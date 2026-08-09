//! Report what CI compiled, and refuse if a job that compiles left no record.
//!
//! Two ways in, one report:
//!
//! - `twice-compiled <log directory>` reads logs the jobs of this run already
//!   wrote. This is what runs on a push: the measurement costs nothing beyond
//!   the builds that were happening anyway.
//! - `twice-compiled --replay <scratch>` produces those logs on this machine, by
//!   running each job's `run:` steps in a git worktree of its own. That is the
//!   expensive way, and the one that answers the question without a push.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use ci_plan::RunStep;
use twice_compiled::{declared_jobs, judge, load, Census, WRAPPER_VARIABLE};

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let root = std::env::current_dir().expect("a working directory");
    assert!(
        root.join(".github/workflows").is_dir(),
        "run this from the repository root: {} has no .github/workflows",
        root.display()
    );

    let workflow = workflow_path(&arguments);
    let steps = ci_plan::run_steps(&ci_plan::load_workflow(&root, &workflow));
    let declared = declared_jobs(&steps);
    assert!(
        !declared.is_empty(),
        "{workflow} declares no job with a `run:` step at all — a census over \
         zero jobs is the empty answer that looks like a clean one"
    );

    let mut absent = BTreeSet::new();
    let logs = match arguments.first().map(String::as_str) {
        Some("--replay") => {
            let scratch = PathBuf::from(
                arguments
                    .get(1)
                    .unwrap_or_else(|| panic!("--replay needs a scratch directory")),
            );
            replay(&root, &steps, &scratch, &mut absent)
        }
        Some(directory) => {
            // THE JOB THIS IS RUNNING IN, asked of the runner. Its own build is
            // still in flight while it judges, so it can have no log yet.
            if let Ok(mine) = std::env::var("GITHUB_JOB") {
                absent.insert(mine);
            }
            PathBuf::from(directory)
        }
        None => panic!("usage: twice-compiled <log directory> | --replay <scratch>"),
    };

    let census = load(&logs, &absent)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", logs.display()));
    report(&census, &declared, &absent);

    let refusals = judge(&census, &declared, &absent);
    if refusals.is_empty() {
        println!(
            "\nevery one of the {} job(s) {workflow} declares recorded what it \
             compiled",
            declared.len() - absent.len()
        );
        return;
    }
    println!();
    for refusal in &refusals {
        println!("REFUSED {refusal}");
    }
    std::process::exit(1);
}

/// `--workflow <path>`, or the one the runner says this job belongs to.
///
/// `GITHUB_WORKFLOW_REF` is `owner/repo/.github/workflows/x.yml@refs/heads/main`
/// — the runner's own answer to which file it is executing, so a workflow
/// renamed tomorrow does not leave a gate pointed at a path that is gone.
fn workflow_path(arguments: &[String]) -> String {
    if let Some(at) = arguments.iter().position(|word| word == "--workflow") {
        return arguments
            .get(at + 1)
            .unwrap_or_else(|| panic!("--workflow needs a path"))
            .clone();
    }
    let reference = std::env::var("GITHUB_WORKFLOW_REF").unwrap_or_else(|_| {
        panic!(
            "no --workflow and no $GITHUB_WORKFLOW_REF — this gate judges the \
             jobs of ONE workflow and will not guess which"
        )
    });
    let path = reference.split('@').next().unwrap_or(&reference);
    let at = path
        .find(".github/")
        .unwrap_or_else(|| panic!("$GITHUB_WORKFLOW_REF={reference} holds no workflow path"));
    path[at..].to_string()
}

/// Print the census. Everything, including the classes this reader cannot key —
/// a gate that prints only its findings cannot be told from one that never ran.
fn report(census: &Census, declared: &BTreeMap<String, Vec<RunStep>>, absent: &BTreeSet<String>) {
    println!("twice-compiled — every compilation CI pays for is one job's\n");
    for job in declared.keys() {
        if absent.contains(job) {
            println!("  {job:<22} not in this census");
            continue;
        }
        let Some(log) = census.jobs.get(job) else {
            println!("  {job:<22} NO LOG");
            continue;
        };
        println!(
            "  {job:<22} {:>6} compiled  {:>6} distinct  {:>5} repeated  \
             {:>5} probes  {:>4} unkeyed",
            log.compilations(),
            log.units.len(),
            log.repeats(),
            log.probes,
            log.unkeyed,
        );
    }

    let paid = census.paid();
    let floor = census.floor();
    println!(
        "\n  CI pays for   {paid:>6} compilations across {} job(s)",
        census.jobs.len()
    );
    println!("  the floor is  {floor:>6} distinct units");
    let duplicated = paid.saturating_sub(floor);
    let share = if paid == 0 {
        0.0
    } else {
        100.0 * duplicated as f64 / paid as f64
    };
    println!("  duplicated    {duplicated:>6} ({share:.1}% of what CI compiles)");
    // THE SPLIT IS THE DECISION. One half exists because the work is spread over
    // jobs and would go away if those jobs were one job; the other half is a job
    // compiling the same unit twice inside itself, which no amount of merging
    // reaches — a job that builds several SEPARATE workspaces holds a `target`
    // per workspace. A single percentage licenses whichever repair was already
    // preferred; these two lines say which one the number is actually about.
    println!(
        "    of which  {:>6} because the work is split across jobs (merging removes)",
        census.shared_between_jobs()
    );
    println!(
        "              {:>6} inside one job, over its own separate workspaces \
         (merging does not)",
        census.repeated_within_jobs()
    );

    let pairwise = census.pairwise();
    if pairwise.is_empty() {
        println!("\n  no two jobs compile the same unit");
        return;
    }
    println!("\n  units two jobs both compile:");
    let mut rows: Vec<((&str, &str), usize)> = pairwise.into_iter().collect();
    rows.sort_by_key(|(pair, count)| (std::cmp::Reverse(*count), *pair));
    for ((left, right), count) in rows {
        println!("    {count:>6}  {left} + {right}");
    }

    // WHICH REPAIR the number licenses depends on what is in it: duplication in
    // third-party dependencies is answered by a shared compilation cache, and
    // duplication in this repository's own crates is answered by the jobs being
    // one job. Twenty rows because the tail is long and the head is the decision.
    let surplus = census.surplus_by_crate();
    let total: usize = surplus.values().sum();
    println!(
        "\n  the {} crate(s) compiled more than once, {total} surplus \
         compilation(s), heaviest first:",
        surplus.len()
    );
    let mut crates: Vec<(&str, usize)> = surplus.into_iter().collect();
    crates.sort_by_key(|(name, count)| (std::cmp::Reverse(*count), *name));
    for (name, count) in crates.iter().take(20) {
        println!("    {count:>6}  {name}");
    }
    if crates.len() > 20 {
        let tail: usize = crates[20..].iter().map(|(_, count)| count).sum();
        println!("    {tail:>6}  … and {} more crate(s)", crates.len() - 20);
    }
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
fn replay(
    root: &Path,
    steps: &[RunStep],
    scratch: &Path,
    absent: &mut BTreeSet<String>,
) -> PathBuf {
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
        make_worktree(root, &tree);
        let log = logs.join(format!("{job}.log"));
        let _ = std::fs::remove_file(&log);

        for step in steps {
            let head = step.script.split_whitespace().next().unwrap_or_default();
            if let Some((_, why)) = NOT_OURS.iter().find(|(word, _)| *word == head) {
                println!("[replay] {job}: skipping `{head}` — {why}");
                continue;
            }
            println!("[replay] {job}: {}", step.script.replace('\n', " "));
            let status = Command::new("bash")
                .arg("-c")
                .arg(&step.script)
                .envs(&step.env)
                .env(rustc_log::LOG_VARIABLE, &log)
                .env(WRAPPER_VARIABLE, &wrapper)
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

/// The first thing in a job that only GitHub can resolve, if there is one.
fn unresolvable(steps: &[&RunStep]) -> Option<String> {
    for step in steps {
        if let Some((name, value)) = step.env.iter().find(|(_, value)| value.contains("${{")) {
            return Some(format!("{name}={value} is resolved by GitHub, not here"));
        }
        if step.script.contains("${{") {
            return Some(format!(
                "a step's own script holds a GitHub expression: {}",
                step.script.replace('\n', " ")
            ));
        }
    }
    None
}

fn build_wrapper(root: &Path) -> PathBuf {
    let manifest = root.join("tools/rustc-log/Cargo.toml");
    let status = Command::new("cargo")
        .args(["build", "--release", "-q", "--manifest-path"])
        .arg(&manifest)
        // The recorder cannot record its own build: it does not exist yet.
        .env(WRAPPER_VARIABLE, "")
        .status()
        .expect("cargo builds the recorder");
    assert!(status.success(), "cannot build the recorder");
    let wrapper = root.join("tools/rustc-log/target/release/rustc-log");
    assert!(wrapper.is_file(), "no recorder at {}", wrapper.display());
    wrapper
}

fn make_worktree(root: &Path, tree: &Path) {
    let _ = std::fs::remove_dir_all(tree);
    std::fs::create_dir_all(tree.parent().expect("a parent")).expect("a worktree parent");
    let status = Command::new("git")
        .args(["worktree", "add", "--detach", "--force"])
        .arg(tree)
        .arg("HEAD")
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
