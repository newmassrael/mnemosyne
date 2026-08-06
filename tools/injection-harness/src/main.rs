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
//!   against is a smaller number that reads as a cleaner one.
//!
//! So: the edit must match exactly once, the control must be green, the restore
//! is verified by reading the bytes back, and each run's target set is compared
//! with the control's. The full log of every run is kept, never filtered — a
//! summary is what this prints, not what it keeps.
//!
//! And it never polls for its own child. A round waited on
//! `pgrep -f "cargo test --workspace"`, which the waiting shell's own command
//! line matches, so the wait could not end; here the child is waited on because
//! this process owns it.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

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
    /// Whether the command itself exited 0.
    exit_ok: bool,
    log: PathBuf,
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
    if let Err(problem) = run() {
        eprintln!("injection-harness: {problem}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let manifest_path = args
        .next()
        .ok_or("usage: injection-harness <manifest.json> [--control-only]")?;
    let control_only = args.any(|flag| flag == "--control-only");

    let manifest: Manifest = serde_json::from_str(
        &fs::read_to_string(&manifest_path)
            .map_err(|e| format!("{manifest_path} unreadable: {e}"))?,
    )
    .map_err(|e| format!("{manifest_path} is not a manifest: {e}"))?;
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

    // THE CONTROL, and it is a gate rather than a first data point. A sweep that
    // starts on a red tree makes every later count a subtraction done in
    // somebody's head, which Round 1053 did and had to redo.
    eprintln!("[control] {}", manifest.test_command.join(" "));
    let control = execute(&manifest, "control")?;
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
        eprintln!(
            "[{}] applying {} edit(s)",
            injection.name,
            injection.edits.len()
        );
        apply(&manifest.repo, &injection.edits)?;
        // THE TREE IS OURS UNTIL THE RESTORE. Round 1054 edited a file while a
        // driver held the snapshot and the restore silently reverted that edit;
        // nothing here yields between apply and restore.
        let run = execute(&manifest, &injection.name);
        restore(&snapshot)?;
        let run = run?;

        let fired: BTreeSet<String> = run.red.difference(&control.red).cloned().collect();
        let missed: BTreeSet<String> = injection
            .expect_red
            .iter()
            .filter(|name| !fired.contains(*name))
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

/// Run the suite once, keeping the whole log and returning what it said.
fn execute(manifest: &Manifest, label: &str) -> Result<Run, String> {
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
    let status = Command::new(&manifest.test_command[0])
        .args(&manifest.test_command[1..])
        .current_dir(&manifest.repo)
        .stdout(Stdio::from(file))
        .stderr(Stdio::from(errors))
        .status()
        .map_err(|e| format!("{:?}: {e}", manifest.test_command))?;
    let text = fs::read_to_string(&log).map_err(|e| format!("{}: {e}", log.display()))?;
    let mut run = summarize(&text);
    run.exit_ok = status.success();
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
