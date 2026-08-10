//! Ask both sides, print what was reached, and only then judge any of it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

use cache_budget::{conclude, Held, Refusal, Run, DEFAULT_LIMIT_BYTES};
use ci_plan::CacheDeclaration;

fn main() {
    let (root, restored) =
        cache_budget::read_arguments(&std::env::args().skip(1).collect::<Vec<_>>());

    // WHAT EACH JOB'S DISK RECEIVED, when this run collected the records. A
    // directory that was not given is not an empty one: without it this gate
    // still answers the budget question and says the other half was not
    // measured, exactly as it does without a run.
    let started = match &restored {
        Some(directory) => match cache_budget::started_from(Path::new(directory)) {
            Ok(started) => started,
            Err(why) => {
                eprintln!(
                    "cache-budget: {}",
                    Refusal::Unreached(format!("cannot read {directory}: {why}"))
                );
                std::process::exit(2);
            }
        },
        None => BTreeMap::new(),
    };

    // DECLARED — through `ci-plan`, this repository's one reader of what its CI
    // says, so this gate cannot drift from the gates asking the same files what
    // CI RUNS.
    let declared = ci_plan::workflow_cache_declarations(&root);

    // HELD — from the only thing that knows what a cache actually costs.
    let held = match held_caches(&root) {
        Ok(held) => held,
        Err(why) => {
            // A REFUSAL, NOT A PASS, and exit 2 rather than exit 1: "I could not
            // look" and "these break the law" are different answers, and one
            // message for both mislabels whichever it did not mean.
            eprintln!("cache-budget: {}", Refusal::Unreached(why));
            std::process::exit(2);
        }
    };

    // WHO CAN BE HEARD AT ALL, read off the same files. A record is an artifact,
    // an artifact belongs to a run, and a workflow that uploads none leaves
    // nothing behind — so the two halves of this gate's horizon are which
    // workflows collect anything and which workflow this run is of. Both are
    // asked rather than assumed, because the sentence they decide used to name a
    // job as deficient for a limit that was the reader's.
    let collecting = ci_plan::workflows_collecting_artifacts(&root);

    // THE RUN, when there is one to be inside. `GITHUB_RUN_ID` is set by the
    // runner and by nothing else, so a developer's machine gets the budget
    // verdict and is TOLD that the other half was not evaluated — inventing a
    // run here would read every cache in the repository as freshly built.
    let run = match std::env::var("GITHUB_RUN_ID").ok() {
        Some(id) => match run_window(&root, &id, &declared) {
            Ok(run) => Some(run),
            Err(why) => {
                eprintln!("cache-budget: {}", Refusal::Unreached(why));
                std::process::exit(2);
            }
        },
        None => None,
    };

    let report = conclude(
        DEFAULT_LIMIT_BYTES,
        &declared,
        &held,
        run.as_ref(),
        &started,
        &collecting,
    );

    // WHAT WAS REACHED, first and unconditionally. A gate that never opened
    // anything and a gate that found nothing wrong print the same silence.
    //
    // THE WORDS ARE THE LIBRARY'S. What this gate SAYS is a decision, and a
    // decision written here has no reader — the whole of what a suite can ask of
    // `main` is its exit code, which is the shape R1096 paid for.
    print!("{}", cache_budget::render(&report));

    // THEN judge it, and print every refusal. Stopping at the first reports one
    // line of a distribution that is itself the finding.
    let refusals = report.refusals();
    if refusals.is_empty() {
        println!("every cache this repository declares is one it keeps");
        return;
    }
    for refusal in &refusals {
        eprintln!("cache-budget: {refusal}");
    }
    let unreached = refusals
        .iter()
        .any(|refusal| matches!(refusal, Refusal::Unreached(_)));
    std::process::exit(if unreached { 2 } else { 1 });
}

/// When this run started, and which keys this commit legitimately invalidated.
///
/// BOTH SIDES ASKED OF A MACHINE. The start time is GitHub's own, from the same
/// clock that stamps a cache's `created_at`, so the two are comparable without
/// this program owning a notion of time. Which keys were invalidated is git's
/// answer to the globs the keys themselves name — `:(glob)` pathspec magic rather
/// than a second implementation of glob matching, which would be a second answer
/// free to disagree with the one GitHub used to build the key.
fn run_window(root: &Path, run_id: &str, declared: &[CacheDeclaration]) -> Result<Run, String> {
    let answer = gh(root, &cache_budget::run_query(run_id))?;
    let started_at = cache_budget::run_started_in(run_id, &answer)?;

    // THE RANGE THIS RUN COVERS, not the commit it ends at. A push carries as
    // many commits as it carries, and the one that moved a cache key's hashed
    // inputs is routinely not the tip.
    let range = cache_budget::range_start(
        std::env::var(cache_budget::RANGE_VARIABLE).ok().as_deref(),
        |sha| commit_is_here(root, sha),
    );

    let mut inputs_changed = BTreeSet::new();
    for declaration in declared {
        if declaration.hashed.is_empty() || inputs_changed.contains(&declaration.prefix) {
            continue;
        }
        let mut arguments = vec![
            "diff".to_string(),
            "--name-only".to_string(),
            range.rev().to_string(),
            "HEAD".to_string(),
            "--".to_string(),
        ];
        arguments.extend(
            declaration
                .hashed
                .iter()
                .map(|glob| format!(":(glob){glob}")),
        );
        let out = Command::new("git")
            .args(&arguments)
            .current_dir(root)
            .output()
            .map_err(|e| format!("`git diff` could not be run at all: {e}"))?;
        if !out.status.success() {
            // A REFUSAL, NOT A GUESS. The usual cause is a checkout with no
            // parent commit, and answering "nothing changed" there would refuse
            // every key this run legitimately rebuilt.
            return Err(format!(
                "`git diff {} HEAD` failed ({}), so which keys this run \
                 invalidated is unknown rather than empty — a checkout of depth 1 \
                 has no parent to compare against: {}",
                range.rev(),
                out.status,
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        if !String::from_utf8_lossy(&out.stdout).trim().is_empty() {
            inputs_changed.insert(declaration.prefix.clone());
        }
    }
    // WHICH WORKFLOW THIS RUN IS OF, asked of the runner and checked against the
    // workflows this gate read. It is what decides whose restore records could
    // have been collected here, so a name nothing recognises is a refusal: the
    // alternative is a report explaining every job's silence with a reason it
    // made up.
    let workflow = ci_plan::workflow_of_reference(
        std::env::var(ci_plan::WORKFLOW_VARIABLE).ok().as_deref(),
        &ci_plan::workflow_files(root),
    )?;

    Ok(Run {
        workflow,
        started_at,
        inputs_changed,
        range,
    })
}

/// Is this commit in this checkout at all?
///
/// A shallow clone holds only what it was asked for, and a revision it does not
/// hold makes `git diff` fail — which this gate turns into a refusal to judge.
/// Asked before the diff rather than recovered from afterwards, so the narrow
/// range is a decision with a reason rather than an error swallowed.
fn commit_is_here(root: &Path, sha: &str) -> bool {
    Command::new("git")
        .args(["cat-file", "-e", &format!("{sha}^{{commit}}")])
        .current_dir(root)
        .output()
        .is_ok_and(|out| out.status.success())
}

/// Run `gh` and hand back its output, or say why it could not be asked.
///
/// THE WORDS ARE THE LIBRARY'S — [`cache_budget::caches_query`] and
/// [`cache_budget::run_query`] — and this function knows none of them. What is
/// left here is the part a suite cannot reach: a process, its exit status, and
/// its two streams.
fn gh(root: &Path, arguments: &[String]) -> Result<String, String> {
    let out = Command::new("gh")
        .args(arguments)
        .current_dir(root)
        .output()
        .map_err(|e| format!("`gh` could not be run at all: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "`gh {}` failed ({}): {}",
            arguments.join(" "),
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Every cache this repository holds, from the GitHub API.
///
/// Through `gh` rather than a hand-rolled HTTP call: it is on every hosted runner
/// and on this project's machine, and it already knows how to authenticate in
/// both — one less credential path to get wrong.
///
/// TWO LINES, AND NEITHER OF THEM IS A DECISION. What to ask for and what the
/// answer means both live in the library, because this function is the one place
/// a suite cannot reach: R1126 moved this gate's words there for that reason and
/// R1129 measured what was left behind here going unnoticed. Until R1130 the
/// answer was flattened by a `--jq` expression written on this side of that line,
/// where nothing could ask whether it still named fields GitHub still sends.
fn held_caches(root: &Path) -> Result<Vec<Held>, String> {
    let answer = gh(root, &cache_budget::caches_query())?;
    cache_budget::caches_in(&answer)
}
