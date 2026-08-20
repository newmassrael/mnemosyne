//! Report what CI compiled, and refuse if a job that compiles left no record.
//!
//! Three ways in:
//!
//! - `twice-compiled <log directory>` reads logs the jobs of this run already
//!   wrote. This is what runs on a push: the measurement costs nothing beyond
//!   the builds that were happening anyway.
//! - `twice-compiled --replay <scratch>` produces those logs on this machine, by
//!   running each job's `run:` steps in a git worktree of its own. That is the
//!   expensive way, and the one that answers the question without a push.
//! - `twice-compiled compare <earlier> <later>` holds one census against
//!   another, each a `gh run download` directory. It reads no workflow and no
//!   repository — the two may be of different commits — and it REFUSES to total
//!   a pair whose jobs did not begin in the same state, which is the number
//!   Round 1099 quoted when it deleted a cache that was saving ten minutes.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

use ci_plan::issue::{self, Tree};
use ci_plan::RunStep;
use twice_compiled::{
    judge, load, unresolvable, Declared, Entrance, JOB_VARIABLE, WRAPPER_VARIABLE,
};

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let entrance = twice_compiled::read_arguments(&arguments).unwrap_or_else(|why| {
        eprintln!("twice-compiled: {why}");
        std::process::exit(2);
    });
    match entrance {
        // THE ENTRANCE THAT NEEDS NOTHING THIS CHECKOUT HOLDS. Two censuses may
        // be of two different commits, so there is no workflow to read and no
        // repository to be in — what both runs uploaded is enough, because R1101
        // made every census say what it was taken under.
        Entrance::Compare { earlier, later } => hold_against(&earlier, &later),
        judged => judge_a_run(judged),
    }
}

/// Read two censuses and hold them against each other.
///
/// EXIT 1 WHEN THE TOTALS ARE NOT QUOTABLE, which is the verdict rather than an
/// aside: a delta between two runs that began in different states is the number
/// Round 1099 quoted, and it deleted a cache that was saving ten minutes.
fn hold_against(earlier: &str, later: &str) -> ! {
    let read = |directory: &str| {
        twice_compiled::load_collected(Path::new(directory)).unwrap_or_else(|error| {
            eprintln!("twice-compiled: cannot read {directory}: {error}");
            std::process::exit(2);
        })
    };
    let held = twice_compiled::compare(&read(earlier), &read(later));
    print!(
        "{}",
        twice_compiled::render_comparison(&held, earlier, later)
    );
    std::process::exit(if held.totals().is_some() { 0 } else { 1 })
}

/// The gate proper: judge one run's records, or a replay of them.
fn judge_a_run(entrance: Entrance) -> ! {
    let root = std::env::current_dir().expect("a working directory");
    assert!(
        root.join(".github/workflows").is_dir(),
        "run this from the repository root: {} has no .github/workflows",
        root.display()
    );

    let workflow = workflow_path(&root, &entrance);
    let document = ci_plan::load_workflow(&root, &workflow);
    let steps = ci_plan::run_steps(&document);
    // BOTH POPULATIONS, FROM ONE FILE AND ONE READER. The jobs that compile are
    // what the census is of; the jobs that restore a cache are what owes it a
    // record of the state it was taken in, and they are not the same jobs —
    // `tools/cache-budget` asks the second question of this same reader.
    let declared = Declared::of(
        &steps,
        &ci_plan::cache_steps(&document, &workflow),
        &ci_plan::artifact_uploads(&document, &workflow),
    );
    assert!(
        !declared.jobs.is_empty(),
        "{workflow} declares no job with a `run:` step at all — a census over \
         zero jobs is the empty answer that looks like a clean one"
    );

    let mut absent = BTreeSet::new();
    let logs = match &entrance {
        Entrance::Replay { scratch, .. } => replay(&root, &steps, Path::new(scratch), &mut absent),
        Entrance::Logs { directory, .. } => {
            // THE JOB THIS IS RUNNING IN, asked of the runner. Its own build is
            // still in flight while it judges, so it can have no log yet.
            if let Ok(mine) = std::env::var("GITHUB_JOB") {
                absent.insert(mine);
            }
            PathBuf::from(directory)
        }
        Entrance::Compare { .. } => unreachable!("`main` sends a comparison elsewhere"),
    };

    let census = load(&logs, &absent)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", logs.display()));
    // THE WORDS ARE THE LIBRARY'S, for the reason the comparison entrance's are:
    // what a gate SAYS is a decision, and the whole of what a suite can ask of
    // `main` is an exit code. R1125's defect — a lookup that could never hit,
    // printing an absence for every cached job of three green runs — was
    // reachable only by a test that replayed a job in a worktree of its own.
    print!(
        "{}",
        twice_compiled::render(&census, &declared, &absent, &root)
    );

    let refusals = judge(&census, &declared, &absent);
    if refusals.is_empty() {
        println!(
            "\nevery one of the {} job(s) {workflow} declares recorded what it \
             compiled",
            declared.jobs.len() - absent.len()
        );
        std::process::exit(0);
    }
    println!();
    for refusal in &refusals {
        println!("REFUSED {refusal}");
    }
    std::process::exit(1);
}

/// `--workflow <path>`, or the one the runner says this job belongs to.
///
/// THE READING IS `ci-plan`'S, and R1107 is why it is not this function's any
/// more. `tools/cache-budget` came to need the same answer — which workflow this
/// run is of decides whose restore records could have been collected in it —
/// and a second cut of `owner/repo/<path>@<ref>` is a second answer free to
/// disagree with this one, which is the shape that crate exists to remove. It
/// also gained the check this had never made: the name is held against the files
/// this repository tracks, so a reference nothing recognises stops the gate
/// instead of pointing it at a path that is gone.
fn workflow_path(root: &Path, entrance: &Entrance) -> String {
    let named = match entrance {
        Entrance::Logs { workflow, .. } | Entrance::Replay { workflow, .. } => workflow.clone(),
        Entrance::Compare { .. } => unreachable!("a comparison reads no workflow"),
    };
    if let Some(path) = named {
        return path;
    }
    ci_plan::workflow_of_reference(
        std::env::var(ci_plan::WORKFLOW_VARIABLE).ok().as_deref(),
        &ci_plan::workflow_files(root),
    )
    .unwrap_or_else(|why| {
        panic!("no --workflow, and {why} — this gate judges the jobs of ONE workflow")
    })
}

/// Steps a replay on this machine must not run, and why.
///
/// NAMED PREFIXES rather than a general judgement about what a step does: these
/// two install what a hosted runner lacks and this machine already has, and
/// every other step runs verbatim. A step that needed something else would fail
/// loudly rather than be silently skipped.
const NOT_OURS: [(&str, &str); 2] = [
    ("sudo", "installs a runner package this machine already has"),
    ("rustup", "installs a toolchain this machine already has"),
];

/// Build the recorder, then run every job's steps in a worktree of its own.
///
/// A WORKTREE PER JOB rather than one checkout with the target directory moved
/// aside, because a job's cost is spread over several `target` directories — the
/// root one, and one per tool workspace a gate shells into. Redirecting them all
/// into one would make cargo share units between them that a runner compiles
/// twice, which is the very number being measured.
///
/// EVERY WORKTREE FROM ONE REVISION, RESOLVED ONCE AND PRINTED. A CI run measures
/// a single commit; a replay takes hours, and `HEAD` is a moving name — a commit
/// landing in the repository while it works silently splits the census across two
/// source trees. That is not hypothetical: a replay of this workflow took its
/// first four jobs from one commit and its fifth from another that arrived
/// mid-run, and nothing in the report said so. The revision is therefore resolved
/// before the first worktree and passed to all of them, and the report says which
/// one it was — a census that cannot name what it measured is one nobody can
/// compare against anything.
fn replay(
    root: &Path,
    steps: &[RunStep],
    scratch: &Path,
    absent: &mut BTreeSet<String>,
) -> PathBuf {
    let revision = resolve_head(root);
    println!("[replay] every job from {revision}, resolved once");
    let wrapper = build_wrapper(root);
    let logs = scratch.join("logs");
    std::fs::create_dir_all(&logs).expect("a log directory");

    let mut by_job: BTreeMap<&str, Vec<&RunStep>> = BTreeMap::new();
    for step in steps {
        by_job.entry(step.job.as_str()).or_default().push(step);
    }

    for (job, steps) in by_job {
        // A REFUSAL RATHER THAN A GUESS. `RUSTUP_TOOLCHAIN: ${{ steps.msrv…}}`
        // is resolved by GitHub from a step this replay does not run, and a job
        // replayed on the wrong toolchain would report its units as shared with
        // every other job — the loudest possible wrong answer.
        if let Some(reason) = unresolvable(&steps) {
            println!("[replay] SKIP {job} — {reason}");
            absent.insert(job.to_string());
            continue;
        }

        let tree = scratch.join("trees").join(job);
        make_worktree(root, &tree, &revision);
        let log = logs.join(format!("{job}.log"));
        let _ = std::fs::remove_file(&log);
        let restore = logs.join(format!("{job}.restored"));
        let _ = std::fs::remove_file(&restore);

        for step in steps {
            let head = step.script.split_whitespace().next().unwrap_or_default();
            if let Some((_, why)) = NOT_OURS.iter().find(|(word, _)| *word == head) {
                println!("[replay] {job}: skipping `{head}` — {why}");
                continue;
            }
            println!("[replay] {job}: {}", step.script.replace('\n', " "));
            // THE TWO VARIABLES THIS REPLAY OWNS, applied last so that whatever
            // the workflow spells for them is replaced. `twice_compiled::
            // unresolvable` skips exactly these names for that reason, and both
            // sides read the one list so they cannot come apart.
            // THE VARIABLES THIS REPLAY OWNS. The last three are what make a
            // replayed census say what it is: a replay runs a job's `run:`
            // steps and never its `uses:` cache step, so nothing is restored,
            // `cache-hit` is honestly `false`, and the two measurements around
            // the absent restore record a job that started from an empty tree —
            // which is exactly what it did.
            let mine = [
                (WRAPPER_VARIABLE, wrapper.as_os_str()),
                (rustc_log::LOG_VARIABLE, log.as_os_str()),
                (restored::VARIABLE, restore.as_os_str()),
                (restored::EXACT_VARIABLE, OsStr::new("false")),
                (JOB_VARIABLE, OsStr::new(job)),
            ];
            debug_assert!(
                mine.iter()
                    .all(|(name, _)| twice_compiled::REPLAY_SETS.contains(name)),
                "a variable set here and not named in REPLAY_SETS is one \
                 `unresolvable` would refuse a job over"
            );
            let status = Command::new("bash")
                .arg("-c")
                .arg(&step.script)
                .envs(&step.env)
                .envs(mine)
                .current_dir(&tree)
                .status()
                .expect("the step runs");
            if !status.success() {
                // NOT FATAL, and named rather than swallowed: the compilations
                // this step paid for already happened and are already in the
                // log, so the census is still the truth about what was built.
                println!("[replay] {job}: exited {status} — its compilations still count");
            }
        }

        drop_worktree(root, &tree);
    }
    logs
}

fn build_wrapper(root: &Path) -> PathBuf {
    let manifest = root.join("tools/rustc-log/Cargo.toml");
    // WHERE IT GOES IS TOLD AND NOT DISCOVERED. This checkout's
    // `.cargo/config.toml` sets `build.target-dir`, which cargo resolves against
    // the directory holding that file — but a replay builds this in a REPOSITORY
    // OF ITS OWN, and a fixture that carries no such file puts the binary under
    // the tool's own workspace instead. A path assumed either way is a guess
    // about somebody else's tree; saying it makes the answer the same in both.
    let build = root.join("target");
    // `--locked` AND THE DECLARATION THAT MAKES IT REQUIRED (R1262). This builds
    // a crate of THIS repository — the program refuses to start anywhere else —
    // so the lockfile it resolves is one this repository tracks, and a free
    // resolve would REWRITE `tools/rustc-log/Cargo.lock` rather than report that
    // it disagreed. A census that repairs the tree it is about to measure is
    // measuring a tree nobody committed.
    let status = issue::cargo(Tree::ThisRepository)
        .args(["build", "--release", "-q", "--locked", "--manifest-path"])
        .arg(&manifest)
        .env("CARGO_TARGET_DIR", &build)
        // The recorder cannot record its own build: it does not exist yet.
        .env(WRAPPER_VARIABLE, "")
        .status()
        .expect("cargo builds the recorder");
    assert!(status.success(), "cannot build the recorder");
    let wrapper = build.join("release/rustc-log");
    assert!(wrapper.is_file(), "no recorder at {}", wrapper.display());
    wrapper
}

/// The commit `HEAD` names right now, as a hash that will not move.
///
/// ASKED ONCE AND OF GIT. `HEAD` is a name for whatever the repository is on, and
/// a replay outlives that: resolving it per job is how a census comes to be of two
/// trees at once.
fn resolve_head(root: &Path) -> String {
    let out = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .expect("git rev-parse runs");
    assert!(
        out.status.success(),
        "cannot resolve HEAD in {}: {}",
        root.display(),
        String::from_utf8_lossy(&out.stderr).trim()
    );
    let revision = String::from_utf8_lossy(&out.stdout).trim().to_string();
    assert!(
        !revision.is_empty(),
        "git resolved HEAD to nothing in {}",
        root.display()
    );
    revision
}

fn make_worktree(root: &Path, tree: &Path, revision: &str) {
    let _ = std::fs::remove_dir_all(tree);
    std::fs::create_dir_all(tree.parent().expect("a parent")).expect("a worktree parent");
    let status = Command::new("git")
        .args(["worktree", "add", "--detach", "--force"])
        .arg(tree)
        .arg(revision)
        .current_dir(root)
        .status()
        .expect("git worktree add runs");
    assert!(
        status.success(),
        "cannot add a worktree at {}",
        tree.display()
    );
}

fn drop_worktree(root: &Path, tree: &Path) {
    let status = Command::new("git")
        .args(["worktree", "remove", "--force"])
        .arg(tree)
        .current_dir(root)
        .status()
        .expect("git worktree remove runs");
    if !status.success() {
        println!(
            "[replay] could not remove {} — remove it by hand",
            tree.display()
        );
    }
}
