//! Apply an injection, run the suite, read what went red, put the tree back —
//! and refuse, loudly, at every point where doing it by hand has gone wrong.
//!
//! THE SHAPE OF AN INJECTION SWEEP. A contract that nothing can break is not a
//! contract, so this arc proves each one by breaking the thing it is about and
//! showing the contract goes red. That means editing the tree, running the whole
//! suite, and restoring — a loop whose every step has a failure mode that looks
//! exactly like a result:
//!
//! - an edit that matched nothing, or matched twice, produces a run that means
//!   nothing and reads as "the injection did not fire";
//! - a control that was already red makes every later count a subtraction
//!   somebody does in their head;
//! - a restore that leaves the file byte-identical but the timestamp older can
//!   hand the next run a stale artifact, which is how one round reported a
//!   shipping defect that did not exist;
//! - a run that built a different set of targets than the control was compared
//!   against is a smaller number that reads as a cleaner one;
//! - a suite can fail in a way its own log does not name — a target that did not
//!   build, a crash, a signal — and the failure list stays empty, so the run
//!   reads as 0 red, which is what a surface that holds also reads as.
//!
//! So: the edit must match exactly once, the control must be green, the restore
//! is verified by reading the bytes back, each run's target set is compared with
//! the control's, and the exit code is held against the failure list rather than
//! merely recorded beside it. The full log of every run is kept, never filtered
//! — a summary is what this prints, not what it keeps.
//!
//! And it never polls for its own child. A round waited on
//! `pgrep -f "cargo test --workspace"`, which the waiting shell's own command
//! line matches, so the wait could not end; here the child is waited on because
//! this process owns it.
//!
//! OWNERSHIP THROUGH ITS OWN DEATH is the other half of that, and it is not the
//! same half: a sweep that is killed still owns an edited tree and a running
//! suite, and neither goes back on its own. `supervise` is where that lives.

mod supervise;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicI32, Ordering};

use serde::{Deserialize, Serialize};

/// The process group of the supervisor now running the suite, or 0 when no run
/// is in flight. Global because a signal is delivered to the process, not to
/// whichever call happens to be on the stack.
static SUITE_GROUP: AtomicI32 = AtomicI32::new(0);

/// The interrupt this sweep was asked to stop on, or 0. Read at every step
/// boundary rather than acted on from the signal thread, so the tree has exactly
/// one owner at every moment.
static INTERRUPTED: AtomicI32 = AtomicI32::new(0);

/// One textual replacement in one file. `from` must occur EXACTLY once.
#[derive(Debug, Clone, Deserialize, Serialize)]
struct Edit {
    file: String,
    from: String,
    to: String,
}

/// One injection: what it breaks, and what the sweep expects to go red.
#[derive(Debug, Clone, Deserialize, Serialize)]
struct Injection {
    name: String,
    /// What this injection is FOR, in the author's words — carried into the
    /// report so a number is never read without the claim it is evidence for.
    #[serde(default)]
    why: String,
    edits: Vec<Edit>,
    /// Test names this injection is expected to turn red. Empty means "say what
    /// went red and judge nothing", which is honest for an exploratory sweep;
    /// naming them makes the harness itself fail when the sweep does not reach
    /// what it was aimed at (the "0 means suspect the injection" rule).
    #[serde(default)]
    expect_red: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Manifest {
    /// The tree to edit and run in.
    repo: PathBuf,
    /// The suite, argv-style. Kept in the manifest rather than assumed, because
    /// a harness that hardcodes `cargo test` cannot be tested without running
    /// one.
    test_command: Vec<String>,
    /// Where the full logs go. One file per run, never truncated.
    logs: PathBuf,
    /// Refuse to start a run when less than this much memory is available.
    ///
    /// The standing rule on this machine is to re-check occupancy BEFORE EVERY
    /// BUILD, because other checkouts share the RAM and a measurement that runs
    /// the machine out of memory is not a measurement. Eight rounds running, the
    /// re-check happened at the start of a session and before the big sweeps and
    /// not before every build — which is what a person does and a program need
    /// not.
    #[serde(default)]
    min_free_mb: Option<u64>,
    injections: Vec<Injection>,
}

/// What one run of the suite said.
#[derive(Debug, Clone, Default, Serialize)]
struct Run {
    passed: usize,
    failed: usize,
    /// Every `test result:` line, which is one per target the run reached.
    targets: usize,
    /// WHICH targets, by the name cargo prints for each one. A count alone
    /// cannot tell a run that lost a target from one that lost a target and
    /// gained another, and the whole point of the drift check is that a run
    /// covering something else than the control is not comparable to it.
    reached: BTreeSet<String>,
    /// The names in the `failures:` lists, deduplicated.
    red: BTreeSet<String>,
    /// What the suite exited with — `None` when a signal killed it before it
    /// could exit at all. Held against `red` by `verdict_disagreement`, because
    /// a status that is only recorded is a status that judges nothing.
    exit_code: Option<i32>,
    log: PathBuf,
}

/// Whether one expected name is among the tests that went red.
///
/// A harness prints a test by the path its target reaches it through —
/// `read_agreement_population::the_walk` for a `#[test]` inside a module — and
/// the person writing a plan writes down the name they gave the function. Held
/// against each other as plain strings, a sweep in which every injection landed
/// exactly where it was aimed comes back "aimed at X and did not reach it" for
/// all of it. That happened, on six injections and forty minutes of suite runs,
/// and the message it prints is the strongest one this tool has: a misaimed
/// injection. A refusal a gate makes for a reason outside its own law is the
/// same defect as a gate that does not fire.
///
/// So a name matches its own suffix at a MODULE BOUNDARY, and nowhere else:
/// `a::b::name` answers to `name` and to `b::name`, and `other_name` does not
/// answer to `name`. Not a substring test — that would let `judges` match
/// `the_walk_judges_nothing` and quietly credit an injection with a red it did
/// not cause.
fn reached(fired: &BTreeSet<String>, expected: &str) -> bool {
    fired.iter().any(|red| {
        red == expected
            || red
                .strip_suffix(expected)
                .is_some_and(|prefix| prefix.ends_with("::"))
    })
}

/// The two halves of a run's verdict — the status the suite exited with, and the
/// names its log listed — must tell the same story.
///
/// Each half is blind where the other sees. A failure list is written by the
/// test harness inside the suite, so it is empty when the suite never got that
/// far: a target that failed to compile, a crash, a signal. An exit code is one
/// number for the whole run, so it says nothing about WHICH test failed and
/// cannot be compared with the control's. Recorded side by side and never
/// compared, the pair is worth exactly the weaker of the two — and the weaker
/// one is silent in the case that matters, where a run that did not finish is
/// read as a surface that held.
fn verdict_disagreement(run: &Run) -> Option<String> {
    let exited = match run.exit_code {
        Some(code) => format!("exited {code}"),
        None => "was killed by a signal, exiting nothing".to_string(),
    };
    if run.exit_code == Some(0) && !run.red.is_empty() {
        return Some(format!(
            "the run {exited} and its log names {} failing test(s) {:?} — either \
             the log was read into names nothing actually failed under, or the \
             suite swallowed a failure it reported; a red count under a green \
             exit is fiction either way",
            run.red.len(),
            run.red
        ));
    }
    if run.exit_code != Some(0) && run.red.is_empty() {
        return Some(format!(
            "the run {exited} and named no failing test — the suite failed in a \
             way its own log does not name (a target that did not build, a \
             crash, a signal), and 0 red out of a run that did not finish is not \
             a clean surface"
        ));
    }
    None
}

#[derive(Debug, Serialize)]
struct Report {
    control: Run,
    injections: Vec<InjectionResult>,
}

#[derive(Debug, Serialize)]
struct InjectionResult {
    name: String,
    why: String,
    run: Run,
    /// Red here and not in the control — the injection's own effect.
    fired: BTreeSet<String>,
    /// Expected red that did not go red.
    missed: BTreeSet<String>,
    /// Targets the control reached and this run did not, and the reverse.
    targets_missing: BTreeSet<String>,
    targets_extra: BTreeSet<String>,
    /// The count difference, kept for a suite whose output names no targets.
    target_drift: i64,
}

fn main() {
    // `--supervise <index|-> -- argv…` is this binary re-exec'd as the owner of
    // one suite run. It never returns: it exits AS the suite did.
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.first().map(String::as_str) == Some("--supervise") {
        let index = match argv.get(1).map(String::as_str) {
            Some("-") | None => None,
            Some(path) => Some(PathBuf::from(path)),
        };
        let command: Vec<String> = argv
            .iter()
            .skip_while(|argument| argument.as_str() != "--")
            .skip(1)
            .cloned()
            .collect();
        supervise::supervise(index, &command);
    }
    if let Err(problem) = run() {
        eprintln!("injection-harness: {problem}");
        std::process::exit(1);
    }
}

/// A manifest's path as an absolute one, resolved against where the sweep was
/// started rather than against wherever a child is about to be run.
///
/// Lexical rather than `canonicalize`, which requires the path to exist: the log
/// directory is created after this, and a gate that refused a manifest for
/// naming a directory it is about to make would be a gate about nothing.
fn absolute(path: &Path) -> Result<PathBuf, String> {
    std::path::absolute(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// The interrupt this sweep has been asked to stop on, if one has arrived.
fn interrupted() -> Option<i32> {
    match INTERRUPTED.load(Ordering::SeqCst) {
        0 => None,
        signal => Some(signal),
    }
}

/// The private state one sweep owns on disk: the copy of this binary that
/// supervises every run, and the originals whoever is still alive restores the
/// tree from.
struct SweepFiles {
    supervisor: PathBuf,
    originals_index: PathBuf,
}

/// Removes the originals when the sweep ends under its own control.
///
/// What is left behind is precisely the signal that it did NOT: the next sweep
/// reads them and says whether the tree still holds an injection.
struct OriginalsGuard {
    logs: PathBuf,
}

impl Drop for OriginalsGuard {
    fn drop(&mut self) {
        if let Err(problem) = supervise::clear_originals(&self.logs) {
            eprintln!("injection-harness: the originals could not be cleared: {problem}");
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let manifest_path = args
        .next()
        .ok_or("usage: injection-harness <manifest.json> [--control-only]")?;
    let control_only = args.any(|flag| flag == "--control-only");

    let mut manifest: Manifest = serde_json::from_str(
        &fs::read_to_string(&manifest_path)
            .map_err(|e| format!("{manifest_path} unreadable: {e}"))?,
    )
    .map_err(|e| format!("{manifest_path} is not a manifest: {e}"))?;
    // EVERY PATH IS MADE ABSOLUTE HERE, ONCE. A manifest names its tree and its
    // logs relative to wherever it is run from — the tracked ones say `../..`
    // and `target/injection-logs` — and the suite is started with the tree as
    // its working directory. A relative path handed across that change of
    // directory resolves somewhere else entirely: the first real sweep under the
    // supervisor died with `No such file or directory`, and a relative backup
    // path would have restored the tree into a directory beside it.
    manifest.repo = absolute(&manifest.repo)?;
    manifest.logs = absolute(&manifest.logs)?;
    if manifest.test_command.is_empty() {
        return Err("the manifest names no test command".to_string());
    }
    fs::create_dir_all(&manifest.logs).map_err(|e| format!("{}: {e}", manifest.logs.display()))?;

    // THE SNAPSHOT, taken before anything is asked of the tree: every file any
    // injection touches, as bytes. The restore is compared against this rather
    // than against a hash of it, so "restored" is a fact about the content and
    // not about a digest agreeing with itself.
    let mut snapshot: BTreeMap<PathBuf, Vec<u8>> = BTreeMap::new();
    for injection in &manifest.injections {
        for edit in &injection.edits {
            let path = manifest.repo.join(&edit.file);
            if snapshot.contains_key(&path) {
                continue;
            }
            let bytes =
                fs::read(&path).map_err(|e| format!("{} unreadable: {e}", path.display()))?;
            snapshot.insert(path, bytes);
        }
    }

    // WHAT A PREVIOUS SWEEP LEFT BEHIND, before this one writes its own. A sweep
    // that ended under its own control removes these; finding them means one
    // died holding an edited tree, and the tree itself is asked whether that
    // edit is still in it.
    let stale = supervise::index_path(&manifest.logs);
    if stale.exists() {
        let originals = supervise::read_originals(&stale)?;
        if supervise::owner_alive(originals.owner) {
            return Err(format!(
                "a sweep is already running in this tree (pid {}, originals in \
                 {}) — two sweeps here would edit the same files and read each \
                 other's injections as their own baseline",
                originals.owner,
                stale.display()
            ));
        }
        let injected = supervise::still_injected(&originals)?;
        if !injected.is_empty() {
            return Err(format!(
                "a sweep died holding this tree: {} left originals in {}, and \
                 {} of them no longer hold their pre-sweep bytes ({:?}). A \
                 sweep started here would measure that injection as its \
                 baseline. Compare and put them back, then remove {}",
                stale.display(),
                manifest.logs.display(),
                injected.len(),
                injected,
                stale.display()
            ));
        }
        eprintln!(
            "[originals] a previous sweep left {} original(s) behind and the \
             tree matches every one — clearing them",
            originals.files.len()
        );
        supervise::clear_originals(&manifest.logs)?;
    }

    // THE ORIGINALS GO TO DISK, because the process holding them in memory is
    // the process that may be killed. From here the supervisor of whatever run
    // is in flight can put the tree back without us.
    let originals_index = supervise::write_originals(&manifest.logs, &snapshot)?;
    let _originals = OriginalsGuard {
        logs: manifest.logs.clone(),
    };
    // AND A COPY OF THIS BINARY, taken before the first run: a sweep may be
    // aimed at the tree that builds it, and the suite would then replace the
    // program that is running the sweep.
    let files = SweepFiles {
        supervisor: supervise::copy_self(&manifest.logs)?,
        originals_index,
    };

    // ONE THREAD OWNS THE INTERRUPTS, and it only records and kills — the tree
    // is put back by the main thread, which is the only one that ever writes to
    // it. Blocking must happen before the thread exists, since the mask is
    // inherited.
    supervise::block_interrupts()?;
    std::thread::spawn(|| {
        let signal = supervise::wait_for_interrupt();
        INTERRUPTED.store(signal, Ordering::SeqCst);
        // SIGTERM to the supervisor, which kills the suite outright and puts the
        // tree back if we no longer can.
        supervise::kill_group(SUITE_GROUP.load(Ordering::SeqCst), libc::SIGTERM);
    });

    // THE CONTROL, and it is a gate rather than a first data point. A sweep that
    // starts on a red tree makes every later count a subtraction done in
    // somebody's head, which Round 1053 did and had to redo.
    eprintln!("[control] {}", manifest.test_command.join(" "));
    let control = execute(&manifest, "control", &files);
    if let Some(signal) = interrupted() {
        restore(&snapshot)?;
        return Err(stopped(signal));
    }
    let control = control?;
    eprintln!(
        "[control] {} passed, {} failed, {} targets",
        control.passed, control.failed, control.targets
    );
    if control.failed > 0 || !control.red.is_empty() {
        return Err(format!(
            "the control is {} red before any injection — a sweep from here \
             measures nothing: {:?}",
            control.failed, control.red
        ));
    }
    if control.targets == 0 {
        return Err("the control reached no test target at all".to_string());
    }
    if let Some(disagreement) = verdict_disagreement(&control) {
        return Err(format!("the control cannot be a baseline: {disagreement}"));
    }
    // THE NAMES MUST IDENTIFY THE TARGETS, or the set comparison below is
    // quietly weaker than the count it replaced. This found its own first
    // instance: three crates in this repository run unit tests for a library
    // AND a binary under one stem, so 151 targets came out as 148 names until
    // what cargo was RUNNING went into the name.
    if !control.reached.is_empty() && control.reached.len() != control.targets {
        return Err(format!(
            "the control ran {} targets under {} distinct names — {} pair(s) \
             share a name, and a drift check cannot see a pair it cannot tell \
             apart",
            control.targets,
            control.reached.len(),
            control.targets - control.reached.len()
        ));
    }
    if control_only {
        println!("{}", serde_json::to_string_pretty(&control).map_err(err)?);
        return Ok(());
    }

    let mut results = Vec::new();
    for injection in &manifest.injections {
        if let Some(signal) = interrupted() {
            return Err(stopped(signal));
        }
        eprintln!(
            "[{}] applying {} edit(s)",
            injection.name,
            injection.edits.len()
        );
        apply(&manifest.repo, &injection.edits)?;
        // THE TREE IS OURS UNTIL THE RESTORE. Round 1054 edited a file while a
        // driver held the snapshot and the restore silently reverted that edit;
        // nothing here yields between apply and restore.
        let run = execute(&manifest, &injection.name, &files);
        restore(&snapshot)?;
        if let Some(signal) = interrupted() {
            return Err(stopped(signal));
        }
        let run = run?;

        let fired: BTreeSet<String> = run.red.difference(&control.red).cloned().collect();
        let missed: BTreeSet<String> = injection
            .expect_red
            .iter()
            .filter(|name| !reached(&fired, name))
            .cloned()
            .collect();
        let drift = run.targets as i64 - control.targets as i64;
        let missing: BTreeSet<String> = control.reached.difference(&run.reached).cloned().collect();
        let extra: BTreeSet<String> = run.reached.difference(&control.reached).cloned().collect();
        eprintln!(
            "[{}] {} red ({} targets, {} missing, {} extra)",
            injection.name,
            fired.len(),
            run.targets,
            missing.len(),
            extra.len(),
        );
        results.push(InjectionResult {
            name: injection.name.clone(),
            why: injection.why.clone(),
            run,
            fired,
            missed,
            targets_missing: missing,
            targets_extra: extra,
            target_drift: drift,
        });
    }

    // Print the table BEFORE judging it: a first-violation stop reports one line
    // of a distribution that is the finding.
    println!(
        "{}",
        serde_json::to_string_pretty(&Report {
            control: control.clone(),
            injections: results.iter().map(clone_result).collect()
        })
        .map_err(err)?
    );

    let mut broken: Vec<String> = Vec::new();
    for result in &results {
        // WHETHER THE RUN HAPPENED, before anything about what it found: a red
        // count read off a suite that never finished is not a smaller finding,
        // it is not a finding.
        if let Some(disagreement) = verdict_disagreement(&result.run) {
            broken.push(format!("{}: {}", result.name, disagreement));
        }
        // BY NAME where the suite names its targets, and by count where it does
        // not — a run that lost one target and gained another has a drift of 0
        // and is not comparable to the control at all.
        if !result.targets_missing.is_empty() || !result.targets_extra.is_empty() {
            broken.push(format!(
                "{}: did not reach {:?} and reached {:?} the control did not — a \
                 run over a different set is a smaller number, not a cleaner one",
                result.name, result.targets_missing, result.targets_extra
            ));
        } else if control.reached.is_empty() && result.target_drift != 0 {
            broken.push(format!(
                "{}: reached {} targets against the control's {}, and this suite \
                 names none of them so the count is all there is",
                result.name, result.run.targets, control.targets
            ));
        }
        if !result.missed.is_empty() {
            broken.push(format!(
                "{}: aimed at {:?} and did not reach it — a sweep that comes \
                 back 0 is a misaimed injection until something says otherwise",
                result.name, result.missed
            ));
        }
    }
    if !broken.is_empty() {
        return Err(broken.join("\n  "));
    }
    Ok(())
}

fn clone_result(result: &InjectionResult) -> InjectionResult {
    InjectionResult {
        name: result.name.clone(),
        why: result.why.clone(),
        run: result.run.clone(),
        fired: result.fired.clone(),
        missed: result.missed.clone(),
        targets_missing: result.targets_missing.clone(),
        targets_extra: result.targets_extra.clone(),
        target_drift: result.target_drift,
    }
}

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// Apply every edit, each of which must match EXACTLY once.
///
/// A replacement that matched nothing produces a run whose silence reads as "the
/// injection did not fire", and one that matched twice produces a change nobody
/// described. Both are refused before the suite is asked anything.
fn apply(repo: &Path, edits: &[Edit]) -> Result<(), String> {
    for edit in edits {
        let path = repo.join(&edit.file);
        let text =
            fs::read_to_string(&path).map_err(|e| format!("{} unreadable: {e}", path.display()))?;
        let hits = text.matches(&edit.from).count();
        if hits != 1 {
            return Err(format!(
                "{} : the text to replace occurs {hits} times, not once",
                edit.file
            ));
        }
        fs::write(&path, text.replacen(&edit.from, &edit.to, 1))
            .map_err(|e| format!("{} unwritable: {e}", path.display()))?;
    }
    Ok(())
}

/// Put every touched file back, and READ IT BACK to say so.
///
/// Writing fresh bytes also gives the file a new mtime, which is the half that
/// Round 1050 lost: a restore that preserved the timestamp let a stale build
/// artifact answer the next measurement, and the round wrote up a shipping
/// defect that was not there.
fn restore(snapshot: &BTreeMap<PathBuf, Vec<u8>>) -> Result<(), String> {
    for (path, bytes) in snapshot {
        fs::write(path, bytes).map_err(|e| format!("{} unwritable: {e}", path.display()))?;
        let back = fs::read(path).map_err(|e| format!("{} unreadable: {e}", path.display()))?;
        if &back != bytes {
            return Err(format!(
                "{} did not come back to what it was — the tree is now in a \
                 state no measurement here describes",
                path.display()
            ));
        }
    }
    Ok(())
}

/// How much memory this machine says is available right now, in MiB.
///
/// `MemAvailable` rather than `MemFree`: the page cache is reclaimable and a
/// build is entitled to it, so `MemFree` reads near zero on a healthy machine
/// and would refuse every run.
fn available_mb() -> Result<u64, String> {
    let meminfo = fs::read_to_string("/proc/meminfo")
        .map_err(|e| format!("cannot read /proc/meminfo to check occupancy: {e}"))?;
    meminfo
        .lines()
        .find_map(|line| {
            let kb = line.strip_prefix("MemAvailable:")?;
            kb.split_whitespace().next()?.parse::<u64>().ok()
        })
        .map(|kb| kb / 1024)
        .ok_or_else(|| "/proc/meminfo names no MemAvailable".to_string())
}

/// What a sweep says when it is stopped, and what it has already done about it.
fn stopped(signal: i32) -> String {
    format!(
        "stopped by {} — the suite was killed and the tree put back; nothing \
         here was measured",
        supervise::signal_name(signal)
    )
}

/// Run the suite once, keeping the whole log and returning what it said.
fn execute(manifest: &Manifest, label: &str, files: &SweepFiles) -> Result<Run, String> {
    if let Some(floor) = manifest.min_free_mb {
        let free = available_mb()?;
        if free < floor {
            return Err(format!(
                "{free} MiB available and the manifest asks for {floor} before a \
                 build — this machine is shared with other checkouts, and a run \
                 that starts here measures the machine rather than the tree"
            ));
        }
        eprintln!("[{label}] {free} MiB available (floor {floor})");
    }
    let log = manifest.logs.join(format!("{label}.log"));
    let file = fs::File::create(&log).map_err(|e| format!("{}: {e}", log.display()))?;
    let errors = file
        .try_clone()
        .map_err(|e| format!("{}: {e}", log.display()))?;
    // The child is WAITED ON, not polled for. A round polled with a pattern the
    // waiting shell's own command line matched, and the wait could not end.
    //
    // And it is not the suite: it is a supervisor that leads its own process
    // group and dies when this process dies, so the suite cannot outlive the
    // sweep that started it. Round 1061 killed a harness and the `cargo test`
    // under it kept writing into this very log file.
    let mut command = supervise::supervised_command(
        &files.supervisor,
        Some(&files.originals_index),
        &manifest.test_command,
    )?;
    let mut child = command
        .current_dir(&manifest.repo)
        .stdout(Stdio::from(file))
        .stderr(Stdio::from(errors))
        .spawn()
        .map_err(|e| format!("{:?}: {e}", manifest.test_command))?;
    let group = child.id() as i32;
    SUITE_GROUP.store(group, Ordering::SeqCst);
    // An interrupt that landed between the spawn and the store would have found
    // no group to signal, so the group asks for itself.
    if interrupted().is_some() {
        supervise::kill_group(group, libc::SIGTERM);
    }
    let status = child
        .wait()
        .map_err(|e| format!("{:?}: {e}", manifest.test_command))?;
    SUITE_GROUP.store(0, Ordering::SeqCst);
    let text = fs::read_to_string(&log).map_err(|e| format!("{}: {e}", log.display()))?;
    let mut run = summarize(&text);
    run.exit_code = status.code();
    run.log = log;
    Ok(run)
}

/// The name cargo gives one test target, off the line that announces it.
///
/// `Running unittests src/lib.rs (target/debug/deps/foo-9a3f...)` is one target
/// and `Doc-tests foo` is another. The trailing hash is dropped because it moves
/// whenever the crate is rebuilt, which is every injection — a name that changes
/// for a reason that is not about coverage would report drift on every run.
///
/// THE BINARY STEM ALONE IS NOT UNIQUE, and this repository is where that shows:
/// a crate with both a library and a binary runs its unit tests twice under one
/// stem, so `mnemosyne_cli`, `mnemosyne_index` and `mnemosyne_render` each named
/// two targets and 151 targets came out as 148 names. A set that collapses three
/// pairs is three pairs a drift check cannot see, so what cargo was RUNNING —
/// `unittests src/lib.rs` against `unittests src/main.rs` — goes in the name.
fn target_name(line: &str) -> Option<String> {
    let line = line.trim_start();
    if let Some(crate_name) = line.strip_prefix("Doc-tests ") {
        return Some(format!("doc:{}", crate_name.trim()));
    }
    let (what, inside) = line.strip_prefix("Running ")?.rsplit_once('(')?;
    let binary = inside.trim_end_matches(')').rsplit('/').next()?;
    let stem = binary.rsplit_once('-').map_or(binary, |(name, _hash)| name);
    Some(format!("{stem}:{}", what.trim()))
}

/// What a cargo-test log says: the per-target totals, WHICH targets, and the
/// names in the `failures:` lists.
///
/// Parsed from the whole log rather than a filtered view of it — a pipeline that
/// filters before counting is how one round lost an exit code to `tail`.
fn summarize(text: &str) -> Run {
    let mut run = Run::default();
    let mut in_failures = false;
    for line in text.lines() {
        if let Some(name) = target_name(line) {
            run.reached.insert(name);
            continue;
        }
        if let Some(rest) = line.strip_prefix("test result:") {
            run.targets += 1;
            let mut fields = rest.split_whitespace();
            // `ok. 12 passed; 0 failed; ...` / `FAILED. 12 passed; 2 failed; ...`
            let _verdict = fields.next();
            let counts: Vec<&str> = rest.split_whitespace().collect();
            for (index, word) in counts.iter().enumerate() {
                let value = || {
                    counts
                        .get(index.wrapping_sub(1))
                        .and_then(|n| n.parse().ok())
                };
                match *word {
                    "passed;" => run.passed += value().unwrap_or(0),
                    "failed;" => run.failed += value().unwrap_or(0),
                    _ => {}
                }
            }
            in_failures = false;
            continue;
        }
        if line.trim_end() == "failures:" {
            in_failures = true;
            continue;
        }
        if in_failures {
            // A failures LIST is indented names; the failures DETAIL blocks that
            // precede it start with `---- name stdout ----`, so only the list
            // shape is taken and everything else ends the block.
            let name = line.strip_prefix("    ").unwrap_or_default();
            if !name.is_empty() && !name.starts_with(' ') && !name.contains(' ') {
                run.red.insert(name.to_string());
            } else if line.trim().is_empty() {
                continue;
            } else {
                in_failures = false;
            }
        }
    }
    run
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_test_answers_to_its_own_name_and_not_to_a_name_it_merely_contains() {
        // THE REFUSAL THIS TOOL MADE FOR A REASON OUTSIDE ITS OWN LAW. A plan
        // names the `#[test]` function; a harness prints the path its target
        // reaches it through. Compared as plain strings, six injections that
        // each landed exactly where they were aimed came back as six misaimed
        // injections — the loudest verdict this tool has, spent on nothing.
        let fired = BTreeSet::from([
            "read_agreement_population::the_walk".to_string(),
            "plain".to_string(),
        ]);
        assert!(
            reached(&fired, "the_walk"),
            "the name a plan is written with is the name the function has"
        );
        assert!(
            reached(&fired, "read_agreement_population::the_walk"),
            "and the path the harness prints is still itself"
        );
        assert!(reached(&fired, "plain"), "an unqualified red is unchanged");

        // AND THE OTHER DIRECTION, which is why this is a suffix at a module
        // boundary rather than a substring: crediting an injection with a red
        // it did not cause is how a sweep says a contract is alive when the
        // thing that went red was its neighbour.
        assert!(
            !reached(&fired, "walk"),
            "`the_walk` is not the test called `walk` — a substring match would \
             credit this injection with somebody else's failure"
        );
        assert!(
            !reached(&fired, "population::the_walk"),
            "half a module segment is not a module path"
        );
        assert!(!reached(&fired, "the_walk_that_is_not_this_one"));
    }

    #[test]
    fn a_log_is_read_for_its_totals_and_its_names() {
        let log = "\
running 2 tests
test alpha ... ok
test beta ... FAILED

failures:

---- beta stdout ----
thread 'beta' panicked at src/lib.rs:1:1:
boom

failures:
    beta

test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out

running 1 test
test gamma ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
";
        let run = summarize(log);
        assert_eq!((run.passed, run.failed, run.targets), (2, 1, 2));
        assert_eq!(run.red, BTreeSet::from(["beta".to_string()]));
    }

    #[test]
    fn a_target_is_named_by_what_cargo_announces_and_not_by_its_hash() {
        let log = "\
     Running unittests src/main.rs (target/debug/deps/counted_without_naming-996bc06531259b19)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

   Doc-tests mnemosyne-ops
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
";
        let run = summarize(log);
        assert_eq!(
            run.reached,
            BTreeSet::from([
                "counted_without_naming:unittests src/main.rs".to_string(),
                "doc:mnemosyne-ops".to_string()
            ]),
            "the hash moves on every rebuild, so a name that carried it would \
             report drift on every injection"
        );
        assert_eq!(run.targets, 2);
    }

    #[test]
    fn one_crate_running_two_target_kinds_is_two_names() {
        // A crate with a library AND a binary runs its unit tests twice under
        // one binary stem. Three such crates in this repository turned 151
        // targets into 148 names, and three collapsed pairs are three pairs the
        // drift check could not have seen.
        let log = "\
     Running unittests src/lib.rs (target/debug/deps/mnemosyne_cli-aa11)
test result: ok. 1 passed; 0 failed
     Running unittests src/main.rs (target/debug/deps/mnemosyne_cli-bb22)
test result: ok. 1 passed; 0 failed
";
        let run = summarize(log);
        assert_eq!(run.reached.len(), 2, "{:?}", run.reached);
        assert_eq!(run.reached.len(), run.targets);
    }

    #[test]
    fn a_lost_target_and_a_gained_one_do_not_cancel() {
        // The count is 2 either way; the SET is what says these two runs are
        // not comparable. This is the whole reason the check is by name.
        let control = summarize(
            "     Running unittests src/lib.rs (target/debug/deps/alpha-1)
test result: ok. 1 passed; 0 failed
     Running unittests src/lib.rs (target/debug/deps/beta-2)
test result: ok. 1 passed; 0 failed
",
        );
        let after = summarize(
            "     Running unittests src/lib.rs (target/debug/deps/alpha-9)
test result: ok. 1 passed; 0 failed
     Running unittests src/lib.rs (target/debug/deps/gamma-3)
test result: ok. 1 passed; 0 failed
",
        );
        assert_eq!(control.targets, after.targets);
        assert_eq!(
            after
                .reached
                .difference(&control.reached)
                .collect::<Vec<_>>(),
            vec!["gamma:unittests src/lib.rs"]
        );
        assert_eq!(
            control
                .reached
                .difference(&after.reached)
                .collect::<Vec<_>>(),
            vec!["beta:unittests src/lib.rs"]
        );
    }

    #[test]
    fn a_suite_killed_by_a_signal_exits_nothing_and_that_is_a_disagreement() {
        // The arm no fake suite in the integration tests can reach by exiting,
        // because it is the arm where nothing exits: a signal takes the suite
        // down mid-run and leaves a log whose failure list is empty for the one
        // reason that has nothing to do with the code under it.
        let killed = Run {
            exit_code: None,
            ..Run::default()
        };
        let said = verdict_disagreement(&killed).expect("a signalled run is not a green run");
        assert!(said.contains("killed by a signal"), "{said}");
    }

    #[test]
    fn a_run_that_exited_zero_over_no_reds_is_the_only_agreement() {
        // The other half of the gate, and it needs saying: a check that only
        // ever objects is indistinguishable from a check that always objects.
        let clean = Run {
            exit_code: Some(0),
            ..Run::default()
        };
        assert!(verdict_disagreement(&clean).is_none());
        let red = Run {
            exit_code: Some(101),
            red: BTreeSet::from(["the_law".to_string()]),
            ..Run::default()
        };
        assert!(
            verdict_disagreement(&red).is_none(),
            "a suite that exits non-zero BECAUSE a test failed is the ordinary \
             case, and the law must not object to it"
        );
    }

    #[test]
    fn a_detail_block_is_not_a_list_of_names() {
        // The `---- name stdout ----` header and the panic text sit under a
        // `failures:` line too; taking them as names is how a hand-written grep
        // reported nine corpus paths as failing tests.
        let log = "\
failures:

---- the_walk stdout ----
28 authored stores, 0 mismatches
  MISMATCH claudedocs/phase1-x/v1/run/stage-a

failures:
    the_walk

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
";
        assert_eq!(summarize(log).red, BTreeSet::from(["the_walk".to_string()]));
    }
}
