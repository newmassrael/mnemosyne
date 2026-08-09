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

/// Seconds, from the microseconds every number here is measured in.
fn seconds(micros: u64) -> f64 {
    micros as f64 / 1_000_000.0
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
        // THE SECOND LINE IS THE ONE THAT PRICES THE FIRST. `compiling` is the
        // area under every compiler the job ran and is larger than the job; the
        // window is from the first compiler's start to the last one's exit, so
        // their ratio is how many compilers were alive on average. The idle
        // figure is the part of that window with NO compiler in it at all —
        // a suite running what it just built — and it is what says how much of
        // this job removing work can actually reach.
        let window = log.span_micros();
        let busy = log.busy_micros();
        let at_once = if busy == 0 {
            0.0
        } else {
            log.micros as f64 / busy as f64
        };
        println!(
            "  {:<22} {:>8.1} s compiling within a {:>8.1} s window \
             ({at_once:.1} at once while busy, {:.1} s of it idle)",
            "",
            seconds(log.compiled_micros()),
            seconds(window),
            seconds(log.idle_micros()),
        );
    }

    let paid = census.paid();
    let floor = census.floor();
    println!(
        "\n  CI pays for   {paid:>6} compilations across {} job(s)  \
         {:>9.1} s of compiling",
        census.jobs.len(),
        seconds(census.paid_micros()),
    );
    println!(
        "  the floor is  {floor:>6} distinct units{:<26}{:>9.1} s",
        "",
        seconds(census.floor_micros()),
    );
    let duplicated = paid.saturating_sub(floor);
    let share = if paid == 0 {
        0.0
    } else {
        100.0 * duplicated as f64 / paid as f64
    };
    let surplus_micros = census.paid_micros().saturating_sub(census.floor_micros());
    let share_micros = if census.paid_micros() == 0 {
        0.0
    } else {
        100.0 * surplus_micros as f64 / census.paid_micros() as f64
    };
    println!(
        "  duplicated    {duplicated:>6} ({share:.1}% of what CI compiles){:<10}\
         {:>9.1} s ({share_micros:.1}%)",
        "",
        seconds(surplus_micros),
    );
    // THE SPLIT IS THE DECISION. One half exists because the work is spread over
    // jobs and would go away if those jobs were one job; the other half is a job
    // compiling the same unit twice inside itself, which no amount of merging
    // reaches — a job that builds several SEPARATE workspaces holds a `target`
    // per workspace. A single percentage licenses whichever repair was already
    // preferred; these two lines say which one the number is actually about.
    println!(
        "    of which  {:>6} because the work is split across jobs (merging \
         removes){:<1}{:>9.1} s",
        census.shared_between_jobs(),
        "",
        seconds(census.shared_between_jobs_micros()),
    );
    println!(
        "              {:>6} inside one job, over its own separate workspaces \
         (merging does not){:>9.1} s",
        census.repeated_within_jobs(),
        seconds(census.repeated_within_jobs_micros()),
    );

    let pairwise = census.pairwise();
    if pairwise.is_empty() {
        println!("\n  no two jobs compile the same unit");
        return;
    }
    // RANKED BY SECONDS AND NOT BY ROWS. The two orders are different orders,
    // and the round that built this reader had already written the repair list
    // in the row order before it could see the other one.
    println!("\n  what merging a pair would remove, and what it would cost:");
    let mut rows: Vec<((&str, &str), twice_compiled::Merge)> = pairwise.into_iter().collect();
    rows.sort_by_key(|(pair, merge)| (std::cmp::Reverse(merge.saved_micros), *pair));
    for ((left, right), merge) in rows {
        println!("    {left} + {right}");
        println!(
            "      {:>6} compilations  {:>8.1} s of compiling removed",
            merge.units,
            seconds(merge.saved_micros),
        );
        // THE OTHER SIDE OF THE TRADE, printed beside the saving rather than
        // left to whoever acts on it: the two jobs run at the same time today,
        // so the merged one starts from the longer of them and can only grow.
        println!(
            "      window  {:>8.1} s today  ->  {:>8.1} s estimated \
             (never below {:.1} s, never above {:.1} s; {:.1} s of it is idle \
             and scales with nothing)",
            seconds(merge.floor_micros),
            seconds(merge.estimate_micros),
            seconds(merge.floor_micros),
            seconds(merge.ceiling_micros),
            seconds(merge.idle_micros),
        );
    }

    // WHICH REPAIR the number licenses depends on what is in it: duplication in
    // third-party dependencies is answered by a shared compilation cache, and
    // duplication in this repository's own crates is answered by the jobs being
    // one job. Twenty rows because the tail is long and the head is the decision.
    let surplus = census.surplus_by_crate();
    let total: usize = surplus.values().map(|cost| cost.times).sum();
    println!(
        "\n  the {} crate(s) compiled more than once, {total} surplus \
         compilation(s), dearest first:",
        surplus.len()
    );
    // DEAREST AND NOT MOST NUMEROUS. A build script is a row and almost no
    // money; a test binary of this repository's own is the other way round, and
    // which of them heads this list is which repair the number licenses.
    let mut crates: Vec<(&str, twice_compiled::Cost)> = surplus.into_iter().collect();
    crates.sort_by_key(|(name, cost)| (std::cmp::Reverse(cost.micros), *name));
    for (name, cost) in crates.iter().take(20) {
        println!(
            "    {:>6}  {:>8.1} s  {name}",
            cost.times,
            seconds(cost.micros)
        );
    }
    if crates.len() > 20 {
        let times: usize = crates[20..].iter().map(|(_, cost)| cost.times).sum();
        let micros: u64 = crates[20..].iter().map(|(_, cost)| cost.micros).sum();
        println!(
            "    {times:>6}  {:>8.1} s  … and {} more crate(s)",
            seconds(micros),
            crates.len() - 20
        );
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
