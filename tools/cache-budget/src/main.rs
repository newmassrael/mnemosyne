//! Ask both sides, print what was reached, and only then judge any of it.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use cache_budget::{conclude, Held, Refusal, Run, DEFAULT_LIMIT_BYTES};
use ci_plan::CacheDeclaration;

fn main() {
    let root: PathBuf = std::env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("."), PathBuf::from);

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

    let report = conclude(DEFAULT_LIMIT_BYTES, &declared, &held, run.as_ref());

    // WHAT WAS REACHED, first and unconditionally. A gate that never opened
    // anything and a gate that found nothing wrong print the same silence.
    println!(
        "{} cache step(s) across this repository's workflows under {} key(s), {} \
         held by GitHub, budget {:.2} GB",
        declared.len(),
        report.rows.len(),
        held.len(),
        DEFAULT_LIMIT_BYTES as f64 / 1e9
    );
    for row in &report.rows {
        let size = match (&row.held, &row.estimate) {
            (Some(held), _) => format!("{:>8.2} GB held", held.size_in_bytes as f64 / 1e9),
            (None, Some(estimate)) => format!(
                "{:>8.2} GB ABSENT, priced from {}",
                estimate.bytes as f64 / 1e9,
                estimate.from
            ),
            (None, None) => "       ? ABSENT, never observed".to_string(),
        };
        println!(
            "  {size}  {}  [{}]  {}",
            row.prefix,
            row.paths.iter().cloned().collect::<Vec<_>>().join(" "),
            row.owners.join(", ")
        );
        // PRINTED THOUGH NOT COUNTED. These are real bytes GitHub is holding, and
        // a gate that judged one generation while silently dropping the others
        // from its output would be reporting a smaller world than it looked at.
        for old in &row.superseded {
            println!(
                "  {:>8.2} GB held under the same key, superseded on {} and aging \
                 out — not counted, because no workflow can stop a lockfile bump \
                 leaving one behind",
                old.size_in_bytes as f64 / 1e9,
                row.held
                    .as_ref()
                    .map_or("—", |newest| newest.created_at.as_str())
            );
        }
    }
    for orphan in &report.orphans {
        println!(
            "  {:>8.2} GB held, declared by nothing: {}",
            orphan.size_in_bytes as f64 / 1e9,
            orphan.key
        );
    }
    match report.demand() {
        Some(demand) => println!("demand {:.2} GB", demand as f64 / 1e9),
        None => println!("demand UNKNOWN — nothing comparable has been observed"),
    }
    // WHETHER THE SECOND HALF WAS EVALUATED AT ALL, said out loud. A gate that
    // silently skipped a law and a gate that found nothing wrong under it print
    // the same clean line otherwise.
    match &report.run {
        Some(run) => println!(
            "run started {}, so a cache created after that is a job that rebuilt; \
             {} key(s) had their hashed inputs moved by this commit{}",
            run.started_at,
            run.inputs_changed.len(),
            if run.inputs_changed.is_empty() {
                String::new()
            } else {
                format!(
                    " ({})",
                    run.inputs_changed
                        .iter()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        ),
        None => println!(
            "NOT INSIDE A RUN (`GITHUB_RUN_ID` unset), so whether these caches were \
             restored or rebuilt was NOT evaluated — only the budget was"
        ),
    }

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
    let started_at = gh(
        root,
        &[
            "api",
            &format!("repos/{{owner}}/{{repo}}/actions/runs/{run_id}"),
            "--jq",
            ".run_started_at",
        ],
    )?
    .trim()
    .to_string();
    if started_at.is_empty() {
        return Err(format!("run {run_id} reports no start time"));
    }

    let mut inputs_changed = BTreeSet::new();
    for declaration in declared {
        if declaration.hashed.is_empty() || inputs_changed.contains(&declaration.prefix) {
            continue;
        }
        let mut arguments = vec![
            "diff".to_string(),
            "--name-only".to_string(),
            "HEAD~1".to_string(),
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
                "`git diff HEAD~1 HEAD` failed ({}), so which keys this commit \
                 invalidated is unknown rather than empty — a checkout of depth 1 \
                 has no parent to compare against: {}",
                out.status,
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        if !String::from_utf8_lossy(&out.stdout).trim().is_empty() {
            inputs_changed.insert(declaration.prefix.clone());
        }
    }
    Ok(Run {
        started_at,
        inputs_changed,
    })
}

/// Run `gh` and hand back its output, or say why it could not be asked.
fn gh(root: &Path, arguments: &[&str]) -> Result<String, String> {
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
/// both — one less credential path to get wrong. `{owner}` and `{repo}` are
/// `gh`'s own placeholders, resolved from the checkout, so this gate never names
/// the repository it is judging.
fn held_caches(root: &Path) -> Result<Vec<Held>, String> {
    let out = Command::new("gh")
        .args([
            "api",
            "--paginate",
            "repos/{owner}/{repo}/actions/caches",
            "--jq",
            r#".actions_caches[] | "\(.size_in_bytes)\t\(.created_at)\t\(.key)""#,
        ])
        .current_dir(root)
        .output()
        .map_err(|e| format!("`gh api` could not be run at all: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "`gh api repos/{{owner}}/{{repo}}/actions/caches` failed ({}): {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut caches = Vec::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let unreadable = || format!("`gh` printed a row this cannot read: {line:?}");
        let (size, rest) = line.split_once('\t').ok_or_else(unreadable)?;
        let (created_at, key) = rest.split_once('\t').ok_or_else(unreadable)?;
        caches.push(Held {
            key: key.to_string(),
            size_in_bytes: size
                .trim()
                .parse()
                .map_err(|e| format!("{size:?} is not a size: {e}"))?,
            created_at: created_at.trim().to_string(),
        });
    }
    Ok(caches)
}
