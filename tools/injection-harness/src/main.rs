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

use serde::Serialize;

/// The process group of the supervisor now running the suite, or 0 when no run
/// is in flight. Global because a signal is delivered to the process, not to
/// whichever call happens to be on the stack.
static SUITE_GROUP: AtomicI32 = AtomicI32::new(0);

/// The interrupt this sweep was asked to stop on, or 0. Read at every step
/// boundary rather than acted on from the signal thread, so the tree has exactly
/// one owner at every moment.
static INTERRUPTED: AtomicI32 = AtomicI32::new(0);

// THE MANIFEST TYPES AND THE ANCHOR LAW ARE THE LIBRARY'S, because a second
// reader of every sweep this repository tracks needs them and a decision in
// `main.rs` has no reader (R1096). See `lib.rs`.
use injection_harness::{
    answers_to, reached, replace_once, Edit, Injection, Manifest, RedSet, Scope, Verb,
};

/// How many test targets one run reached under each name cargo prints.
///
/// A MULTISET AND NOT A SET, and the difference is a whole sweep this tool used
/// to refuse rather than run. Two targets may honestly share a name: `all` is
/// the name of the folded suite in BOTH `mnemosyne-cli` and `mnemosyne-server`,
/// and the line cargo announces a target with carries no package, so
/// `all:tests/all.rs` is what both of them are called. Under a set the pair
/// collapsed to one element, the control came back "82 targets under 81 names",
/// and the harness refused the whole manifest — the repository's widest sweep,
/// stopped by a name it had every right to.
///
/// THE INVARIANT THE REFUSAL WAS PROTECTING IS COMPARABILITY, and uniqueness was
/// its proxy. A count of targets per name is comparable whether or not the names
/// are unique: a run that loses one of the two `all` targets comes back with
/// that name at 1 against the control's 2, which is the drift, said exactly.
/// What uniqueness bought over this is the ability to say WHICH of an
/// indistinguishable pair went missing — and no reading of cargo's output can
/// say that, so the proxy was refusing sweeps for an answer it could not have
/// given either.
type Targets = BTreeMap<String, usize>;

/// What one run of the suite said.
#[derive(Debug, Clone, Default, Serialize)]
struct Run {
    passed: usize,
    failed: usize,
    /// Every `test result:` line, which is one per target the run reached.
    targets: usize,
    /// WHICH targets, by the name cargo prints for each one, and how many ran
    /// under it. A count alone cannot tell a run that lost a target from one
    /// that lost a target and gained another, and the whole point of the drift
    /// check is that a run covering something else than the control is not
    /// comparable to it.
    reached: Targets,
    /// The names in the `failures:` lists, deduplicated.
    red: BTreeSet<String>,
    /// EVERY name the run executed, red or green — the population a plan's
    /// `expect_red` is addressed into. Kept out of the report, which is about
    /// what an injection DID: this is what the control is asked before any
    /// injection runs, and a list of two thousand names in a summary is not a
    /// summary.
    #[serde(skip)]
    ran: BTreeSet<String>,
    /// What the suite exited with — `None` when a signal killed it before it
    /// could exit at all. Held against `red` by `verdict_disagreement`, because
    /// a status that is only recorded is a status that judges nothing.
    exit_code: Option<i32>,
    log: PathBuf,
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

/// Whether the names a run announced ACCOUNT FOR the targets it ran.
///
/// THIS IS THE HONEST REMAINDER OF THE UNIQUENESS REFUSAL. That check compared
/// the number of distinct names with the number of `test result:` lines, and it
/// was firing for two different reasons wearing one message: names that collide
/// (which the multiset above now handles) and targets that were never announced
/// at all. The second is still fatal — a `test result:` with no `Running` line
/// before it is a target the by-name comparison is simply blind to, so the check
/// that replaced the count would be quietly weaker than the count it replaced.
///
/// A suite that announces NOTHING is not this case: that is a suite whose output
/// has no target names in it, the count is all there is, and the drift check
/// says so and falls back to it.
fn unaccounted_targets(run: &Run) -> Option<String> {
    let named: usize = run.reached.values().sum();
    if run.reached.is_empty() || named == run.targets {
        return None;
    }
    Some(format!(
        "it announced {named} target(s) by name and printed {} result(s) — a \
         target on one side of that and not the other is one the by-name \
         comparison cannot see at all, which would make it weaker than the \
         count it replaces",
        run.targets
    ))
}

/// What separates two runs' target coverage, per name.
///
/// Both directions, because they mean different things: a name the injected run
/// reached fewer times ran less than the control did, and one it reached more
/// times ran something the control never measured. Either way the two runs are
/// over different populations and the red counts are not each other's baseline.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
struct TargetGap {
    /// `name → (control, run)` where the run reached it fewer times.
    missing: BTreeMap<String, (usize, usize)>,
    /// `name → (control, run)` where it reached it more times.
    extra: BTreeMap<String, (usize, usize)>,
}

impl TargetGap {
    fn is_empty(&self) -> bool {
        self.missing.is_empty() && self.extra.is_empty()
    }
}

/// The gap between what the control reached and what one run reached.
fn compare_targets(control: &Targets, run: &Targets) -> TargetGap {
    let mut gap = TargetGap::default();
    // EVERY NAME EITHER RUN KNOWS, once — a name only one of them reached is
    // the whole point, and a name both reached must not be judged twice.
    let names: BTreeSet<&String> = control.keys().chain(run.keys()).collect();
    for name in names {
        let (before, after) = (
            control.get(name).copied().unwrap_or(0),
            run.get(name).copied().unwrap_or(0),
        );
        match before.cmp(&after) {
            std::cmp::Ordering::Greater => {
                gap.missing.insert(name.clone(), (before, after));
            }
            std::cmp::Ordering::Less => {
                gap.extra.insert(name.clone(), (before, after));
            }
            std::cmp::Ordering::Equal => {}
        }
    }
    gap
}

#[derive(Debug, Serialize)]
struct Report {
    /// The suite every count below is a fact about, and the first field so a
    /// reader meets it before the numbers.
    ///
    /// A RED SET IS A FACT ABOUT A POPULATION, AND THIS REPORT USED TO OMIT
    /// WHICH. Round 1138 read `fired` off a sweep of its own contract, found one
    /// injection that reddened that contract ALONE, and wrote it down as what
    /// nothing else in the repository guards. The run had been scoped with
    /// `-p mnemosyne-cli`, so the edited crate's own suite never ran and could
    /// not have appeared in any `fired` set — and nothing in this report said
    /// so, because the report carried counts and names and no population at all.
    /// A measurement whose condition is invisible is read as unconditional,
    /// which is the very failure that round's contract exists to state.
    test_command: Vec<String>,
    /// WHICH OF THE MANIFEST'S INJECTIONS THIS RUN MEASURED — the same lesson as
    /// the field above, one axis over. R1179 gave this tool a `--only`, so a
    /// report can now be about four injections of fifty-two; without this field
    /// that report is shaped exactly like the sweep of the whole file, and a
    /// reader would have to know which command produced the document in front of
    /// them to know what it did not measure.
    scope: Scope,
    control: Run,
    injections: Vec<InjectionResult>,
}

/// The one place a report is built, for both the modes that print one.
///
/// TWO CONSTRUCTION SITES WOULD BE TWO WRITE PATHS TO THE SAME FIELD, free to
/// disagree about whether it is filled — and the control-only mode is exactly
/// where a population is easiest to forget, because there are no injections to
/// attribute to it yet.
fn report(
    manifest: &Manifest,
    scope: &Scope,
    control: &Run,
    injections: Vec<InjectionResult>,
) -> Report {
    Report {
        test_command: manifest.test_command.clone(),
        scope: scope.clone(),
        control: control.clone(),
        injections,
    }
}

#[derive(Debug, Clone, Serialize)]
struct InjectionResult {
    name: String,
    why: String,
    run: Run,
    /// Red here and not in the control — the injection's own effect.
    fired: BTreeSet<String>,
    /// Expected red that did not go red.
    missed: BTreeSet<String>,
    /// The other direction: red that answers to no expectation. Always recorded,
    /// judged only where the manifest claims its `expect_red` is the whole set —
    /// because for every other sweep this is the count of guards nobody has
    /// written down yet, which is a finding rather than a failure.
    unexpected: BTreeSet<String>,
    /// Whether this injection took the manifest's own anchors with it, which is
    /// what decides whether `tests_that_read_the_anchors` accounts for anything
    /// here. In the report because a reader who cannot see it cannot tell an
    /// excused red from an unnamed one.
    anchors_gone: bool,
    /// Names the manifest says read its anchors and that did NOT go red while
    /// those anchors were gone — an excuse that has stopped applying.
    mechanism_missed: BTreeSet<String>,
    /// Targets the control reached and this run reached fewer of, and the
    /// reverse — by name, with both counts.
    targets: TargetGap,
    /// The count difference, kept for a suite whose output names no targets.
    target_drift: i64,
    /// The injection AS RUN, carried so the firing record is written from what
    /// this process actually applied rather than from a second read of the
    /// manifest — two readings of one file are two answers free to disagree, and
    /// the tree can move between them.
    #[serde(skip)]
    edits: Vec<Edit>,
    #[serde(skip)]
    expect_red: Vec<String>,
}

/// R1258 — THE FIRST WORD IS READ IN ONE PLACE, and what it may be is
/// [`Verb`]'s to say. Until this round the tool had one unnamed action and the
/// manifest was simply the first argument, with `--supervise` sitting in that
/// same position as a flag; a second action would have had to be either a word
/// that means a verb unless it looks like a path, or a mode flag that changes
/// what the whole invocation is. Both put two meanings in one position, and this
/// file's own laws are largely about readers that cannot tell two things apart.
fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let outcome = match argv.first().map(|word| (word, Verb::of(word))) {
        // `supervise <index|-> -- argv…` is this binary re-exec'd as the owner
        // of one suite run — spelled by `supervise::supervised_command` and by
        // nobody else. It never returns: it exits AS the suite did.
        Some((_, Some(Verb::Supervise))) => {
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
        Some((_, Some(Verb::Sweep))) => run(&argv[1..]),
        Some((_, Some(Verb::Forget))) => forget(&argv[1..]),
        Some((word, None)) => Err(format!(
            "unknown verb {word:?} — usage: injection-harness {}",
            Verb::USAGE
        )),
        None => Err(format!("usage: injection-harness {}", Verb::USAGE)),
    };
    if let Err(problem) = outcome {
        eprintln!("injection-harness: {problem}");
        std::process::exit(1);
    }
}

/// `forget <manifest.json> --injection <name> --because <why>` — record that a
/// row is evidence about an injection this manifest no longer has.
///
/// THE VERB IS THIN AND THE DECISION IS THE LIBRARY'S, for the reason this
/// crate's `lib.rs` opens with: what lives in `main.rs` has no reader but a
/// person. Everything this refuses is refused in
/// [`injection_harness::forget`], where the suite can ask it.
fn forget(argv: &[String]) -> Result<(), String> {
    let mut args = argv.iter();
    let manifest_path = args
        .next()
        .ok_or_else(|| format!("usage: injection-harness {}", Verb::USAGE))?;
    let mut name: Option<String> = None;
    let mut because: Option<String> = None;
    while let Some(word) = args.next() {
        // A FLAG'S VALUE IS NOT A POSITIONAL ARGUMENT — the same rule the sweep
        // verb below reads its own flags by, and for the same reason: a reason
        // that went missing must not be read as the next flag.
        let value = |args: &mut std::slice::Iter<'_, String>| {
            args.next()
                .cloned()
                .ok_or_else(|| format!("{word} takes a value"))
        };
        match word.as_str() {
            "--injection" => name = Some(value(&mut args)?),
            "--because" => because = Some(value(&mut args)?),
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    let name = name.ok_or("forget names the injection: `--injection <name>`")?;
    let because = because.ok_or_else(|| {
        format!(
            "forgetting `{name}` is a decision and `--because <why>` is where it \
             is written down — a row retired without one has thrown away exactly \
             what the law was holding out for"
        )
    })?;
    let retired = injection_harness::forget(Path::new(manifest_path), &name, &because)?;
    // THE RETIRED EVIDENCE IS PRINTED, not merely filed. What is being given up
    // is the interesting part: a reader of this line sees the reds that row was
    // proven to raise, at the moment somebody decides they answer to nothing.
    println!(
        "[record] `{name}` forgotten, because: {}\n\
         [record] what it was proven to redden: {:?}",
        retired.because, retired.was.tests
    );
    Ok(())
}

/// The interrupt this sweep has been asked to stop on, if one has arrived.
fn interrupted() -> Option<i32> {
    match INTERRUPTED.load(Ordering::SeqCst) {
        0 => None,
        signal => Some(signal),
    }
}

/// The private state one sweep owns: the image that supervises every run, and
/// the originals whoever is still alive restores the tree from.
struct SweepFiles {
    /// [`supervise::SUPERVISOR`] — the running image, which is a name for an
    /// inode rather than a file on disk.
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

fn run(argv: &[String]) -> Result<(), String> {
    let mut args = argv.iter().cloned();
    let manifest_path = args
        .next()
        .ok_or_else(|| format!("usage: injection-harness {}", Verb::USAGE))?;
    // A FLAG'S VALUE IS NOT A POSITIONAL ARGUMENT: `--only` consumes the word
    // after it, so a misspelled flag cannot be read as a manifest and a missing
    // value cannot silently swallow `--control-only`.
    let mut control_only = false;
    let mut only: Vec<String> = Vec::new();
    while let Some(word) = args.next() {
        match word.as_str() {
            "--control-only" => control_only = true,
            "--only" => only.push(
                args.next()
                    .ok_or("--only names an injection: `--only <name>`")?,
            ),
            other => return Err(format!("unknown argument {other:?}")),
        }
    }

    // EVERY PATH IS ABSOLUTE BY THE TIME IT ARRIVES, resolved once, in the
    // library, against the MANIFEST'S OWN DIRECTORY. A manifest names its tree
    // and its logs, and the suite is started with the tree as its working
    // directory — a relative path handed across that change of directory
    // resolves somewhere else entirely: the first real sweep under the
    // supervisor died with `No such file or directory`, and a relative backup
    // path would have restored the tree into a directory beside it. Resolving
    // from the file rather than from the caller's cwd is R1108's half: it is
    // what lets a second reader open every tracked sweep without knowing where
    // each was meant to be run from.
    let manifest: Manifest = injection_harness::read_manifest(Path::new(&manifest_path))?;
    if manifest.test_command.is_empty() {
        return Err("the manifest names no test command".to_string());
    }
    fs::create_dir_all(&manifest.logs).map_err(|e| format!("{}: {e}", manifest.logs.display()))?;

    // THE SNAPSHOT, taken before anything is asked of the tree: every file any
    // injection touches, as bytes. The restore is compared against this rather
    // than against a hash of it, so "restored" is a fact about the content and
    // not about a digest agreeing with itself.
    //
    // AND, IN THE SAME PASS, A DRY RUN OF EVERY INJECTION — because the only
    // other place an anchor is checked is `apply`, which runs after the control
    // and after every injection before it. A typo in the ninth of nine costs the
    // control plus eight whole-suite runs, on a machine where one of those is
    // tens of minutes, and the sweep then ends having measured nothing and
    // having edited the tree eight times. The nine-injection plan this tool
    // exists to run is written by hand against two files; that is exactly the
    // shape.
    //
    // It is a DRY RUN and not a count against the pristine bytes: an injection's
    // second edit may legitimately rewrite what its first one wrote, and a gate
    // that refused that would be refusing for a reason outside its own law.
    // Folding it into the snapshot pass is what keeps the two from disagreeing —
    // the bytes checked here are the bytes `apply` will edit, by construction
    // rather than by argument.
    // WHICH OF THEM THIS SWEEP IS, decided before the snapshot: a scoped run
    // takes a copy of the files IT will edit and dry-runs the anchors IT will
    // apply, so what it protects is what it does.
    let (chosen, scope) = injection_harness::select(&manifest.injections, &only)?;
    let chosen: Vec<Injection> = chosen.into_iter().cloned().collect();
    let snapshot = injection_harness::snapshot_and_dry_run(&manifest.repo, &chosen)?;

    // AND WHICH INJECTIONS TAKE THIS MANIFEST'S ANCHORS WITH THEM — asked of
    // every injection the FILE carries and not only of the chosen, because the
    // question is about the tree a reader of those anchors sees, and that reader
    // does not know which of them this run selected.
    //
    // BEFORE THE CONTROL, with the pre-flight, and for the same reason: what it
    // can refuse is a manifest whose expectation cannot fail, and finding that
    // out after the suites have run is finding it out at the price of the whole
    // sweep.
    //
    // REFUSED WHOLE RATHER THAN SCOPED TO `--only`. A loose anchor is a fact
    // about the tree and a narrow run is how somebody repairs it; an expectation
    // the mechanism guarantees is a defect in the FILE, repaired by editing it,
    // and after that edit this refusal is gone. Nothing here blocks its own fix.
    let unfalsifiable = injection_harness::unfalsifiable(&manifest.repo, &manifest)?;
    if !unfalsifiable.is_empty() {
        return Err(format!(
            "{} expectation(s) in {manifest_path} cannot fail. `{}` names the \
             test(s) that read this manifest's anchors, and a red the mechanism \
             raises whatever the code does is not evidence about the code:\n  {}",
            unfalsifiable.len(),
            "tests_that_read_the_anchors",
            unfalsifiable.join("\n  ")
        ));
    }
    let unanchored = injection_harness::unanchored(&manifest.repo, &manifest.injections)?;

    // AND THE EVIDENCE THIS RUN IS ABOUT TO REPLACE IS VOIDED BEFORE THE CONTROL
    // READS IT (R1200). A row proven against a definition the manifest no longer
    // has is evidence about another injection, and R1198's gate says so by going
    // red — inside the suite that IS this repository's own self-check control.
    // Editing a proven injection there deadlocked: the stale row reddened the
    // control, the harness refuses to start on a red control, and the `--only`
    // run that would clear the row could never happen.
    //
    // AFTER THE PRE-FLIGHT, so a manifest that will be refused for a loose
    // anchor loses nothing, and BEFORE the control, which is the whole point:
    // the control's tree must not still hold a claim this run has already
    // decided is void. See `injection_harness::void_stale_evidence` for why
    // removing evidence here is not a way to launder any.
    for name in injection_harness::void_stale_evidence(Path::new(&manifest_path), &chosen)? {
        eprintln!(
            "[record] `{name}` was proven against a different definition — that evidence \
             is void and this run re-proves it"
        );
    }

    // WHAT THE LAST MEASUREMENT SAID, read ONCE and before this run writes
    // anything over it (R1201). Read after the voiding above, so a row this run
    // has already decided is void is not a baseline anything is compared to.
    let baseline = injection_harness::read_firings(&injection_harness::firings_path(Path::new(
        &manifest_path,
    )))?;

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
    // AND THE IMAGE THIS PROCESS IS RUNNING, which is what supervises every run:
    // a sweep may be aimed at the tree that builds it, and the suite would then
    // replace the program running the sweep. `/proc/self/exe` names the inode
    // rather than the path, so it survives that — see `supervise::SUPERVISOR`
    // for why it is not a copy taken here.
    let files = SweepFiles {
        supervisor: PathBuf::from(supervise::SUPERVISOR),
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
        // THE COUNT AND THE NAMES COME FROM DIFFERENT LINES OF THE LOG, so they
        // can disagree — and this message printed both without saying which,
        // which is how "3 red {}" reached a reader who then had to run the whole
        // suite again to learn what had failed. A failure a `failures:` list
        // never names is a real shape (a doc-test's name carries spaces, a
        // target can die before it prints one), so it is SAID rather than shown
        // as an empty set beside a number.
        let named = match control.red.is_empty() {
            true => "and its log names none of them — a doc-test, a target that \
                     did not build, or a crash, none of which a `failures:` list \
                     carries a name for"
                .to_string(),
            false => format!("{:?}", control.red),
        };
        // AND ONE CAUSE OF A RED CONTROL IS THIS RECORD, WHICH ONLY THIS TOOL
        // CAN SEE (R1200). A sweep whose own suite holds the law over firing
        // records goes red on a row that was proven against an edited
        // injection. The run VOIDS the rows it selected before getting here, so
        // what can still be doing it is a row for an injection this run was NOT
        // asked to re-prove — and the reader is then looking at a failing test
        // name with no way to know that the fix is `--only` naming one more.
        // Said only when it is true, so it cannot become the line that is always
        // there.
        let unselected: Vec<Injection> = manifest
            .injections
            .iter()
            .filter(|injection| !chosen.iter().any(|run| run.name == injection.name))
            .cloned()
            .collect();
        let stale = injection_harness::read_firings(&injection_harness::firings_path(Path::new(
            &manifest_path,
        )))
        .ok()
        .flatten()
        .map(|record| injection_harness::stale_evidence(&record, &unselected))
        .unwrap_or_default();
        if !stale.is_empty() {
            eprintln!(
                "[control] this sweep's record still holds evidence about a different \
                 definition of {stale:?}, which this run did not select. If the suite \
                 above holds the law over these records, that is what is red, and the \
                 way through is to re-prove them too: --only {}",
                stale.join(" --only ")
            );
        }
        return Err(format!(
            "the control is {} red before any injection — a sweep from here \
             measures nothing: {named}",
            control.failed
        ));
    }
    if control.targets == 0 {
        return Err("the control reached no test target at all".to_string());
    }
    if let Some(disagreement) = verdict_disagreement(&control) {
        return Err(format!("the control cannot be a baseline: {disagreement}"));
    }
    // THE NAMES MUST ACCOUNT FOR THE TARGETS, or the comparison below is quietly
    // weaker than the count it replaced. Not that they must be UNIQUE: two
    // targets may honestly share a name, and counting them is what keeps the
    // comparison whole (see `Targets`). What cannot be tolerated is a target no
    // name covers at all.
    if let Some(unaccounted) = unaccounted_targets(&control) {
        return Err(format!("the control cannot be a baseline: {unaccounted}"));
    }
    if control_only {
        // THE SAME SHAPE AS A FULL SWEEP'S, with the injections it has none of.
        // A control printed as a bare `Run` was a second output shape carrying
        // counts under no population, and a reader would have had to know which
        // flag produced the file in front of them to know what it measured.
        let report = report(&manifest, &scope, &control, Vec::new());
        println!("{}", serde_json::to_string_pretty(&report).map_err(err)?);
        return Ok(());
    }

    // AND EVERY NAME THE PLAN IS ADDRESSED TO IS ONE THE CONTROL RAN.
    //
    // R1183 asked whether a sweep's command still names a suite that EXISTS and
    // found three that did not. This is the same question one level in — whether
    // the tests it names still exist inside that suite — and until now only one
    // of the two had a reader. An `expect_red` whose test was renamed comes back
    // `missed`, which is the word this tool also prints for an injection that
    // landed somewhere else, and it prints it only AFTER a whole suite run per
    // injection. Asked here it costs nothing: the control has already run, and
    // the names it ran are the harness's own words rather than a second
    // derivation of them.
    //
    // SCOPED TO WHAT THIS RUN WILL EXECUTE, and after the control-only return
    // above so that mode stays what it says it is: a baseline measurement. An
    // `--only` run is a claim about the injections it selects, and refusing it
    // over a stale name in one it was asked to skip would take the narrow mode
    // away from exactly the person repairing that name.
    // AND "I COULD NOT READ THE NAMES" IS NOT "THE NAMES ARE WRONG". A suite is
    // free to print no per-test line — `--quiet` does exactly that — and a run
    // whose log has none gives this check an empty population, in which every
    // expectation looks stale. The two answers are separated here, and the one
    // that is not a verdict is PRINTED rather than passed over: a check that
    // silently declined is indistinguishable from a check that held.
    if control.ran.is_empty() {
        eprintln!(
            "[control] no per-test names in this log, so nothing here says whether the \
             plan's {} expectation(s) still name tests that exist",
            chosen
                .iter()
                .map(|injection| injection.expect_red.len())
                .sum::<usize>()
        );
    } else {
        let unaddressed: Vec<String> = chosen
            .iter()
            .flat_map(|injection| {
                injection
                    .expect_red
                    .iter()
                    .filter(|expected| !reached(&control.ran, expected))
                    .map(|expected| format!("{}: {expected}", injection.name))
            })
            .collect();
        if !unaddressed.is_empty() {
            return Err(format!(
                "{} expectation(s) name a test the control never ran, of {} it did. A name \
                 that no longer exists scores `missed`, which is also what a misaimed \
                 injection scores — and it scores it after a whole suite run:\n  {}",
                unaddressed.len(),
                control.ran.len(),
                unaddressed.join("\n  ")
            ));
        }
    }

    let mut results = Vec::new();
    for injection in &chosen {
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
        // THE REDS THE MECHANISM ACCOUNTS FOR, and only where it does. While
        // this injection is in the tree the manifest's anchors are gone, so
        // whatever reads them is red for that and not for the read the injection
        // removed; where the anchors survive — an injection whose `to` contains
        // its `from` — nothing here is excused and such a red is as unnamed as
        // any other.
        let anchors_gone = unanchored.contains(&injection.name);
        let mechanism: &[String] = if anchors_gone {
            &manifest.tests_that_read_the_anchors
        } else {
            &[]
        };
        let unexpected: BTreeSet<String> = fired
            .iter()
            .filter(|red| {
                !injection
                    .expect_red
                    .iter()
                    .any(|expected| answers_to(red, expected))
                    && !mechanism.iter().any(|reader| answers_to(red, reader))
            })
            .cloned()
            .collect();
        // AND THE EXCUSE IS HELD TO THE SAME STANDARD AS AN EXPECTATION. A name
        // the manifest excuses and that did not go red while the anchors it
        // reads were gone is an excuse that has quietly stopped applying — the
        // way a blanket exemption decays, and the direction that reads as a
        // pass. Named here so the sweep says so instead of counting one fewer
        // unnamed red.
        let mechanism_missed: BTreeSet<String> = mechanism
            .iter()
            .filter(|name| !reached(&fired, name))
            .cloned()
            .collect();
        let drift = run.targets as i64 - control.targets as i64;
        let targets = compare_targets(&control.reached, &run.reached);
        eprintln!(
            "[{}] {} red, {} unnamed ({} targets, {} missing, {} extra)",
            injection.name,
            fired.len(),
            unexpected.len(),
            run.targets,
            targets.missing.len(),
            targets.extra.len(),
        );
        // AND HOW THAT SET COMPARES TO THE LAST TIME ANYONE MEASURED IT (R1201).
        //
        // R1179 left this: under the default claim an unnamed red is reported
        // and judged by nothing, so a repaired injection's red set can GROW in
        // silence. R1178's `a-dependency-bump-…` reddened three tests beyond its
        // own, all of them legitimate, and nothing would have said if a fourth
        // had appeared that was not.
        //
        // WHAT CHANGED SINCE IS THAT THERE IS NOW SOMETHING TO COMPARE WITH.
        // R1198's record keeps the WHOLE red set a run measured, not the named
        // half — so the judgement the weak claim cannot make against a rule can
        // be made against the last measurement. Only for a row whose DEFINITION
        // still matches: R1200 voids the rest before the control, so a row still
        // here is one this comparison is entitled to make.
        //
        // REPORTED AND NOT FAILED, deliberately. A new red under an unchanged
        // injection is most often a guard somebody just wrote, and a sweep that
        // went red for that would be a sweep people stop running. Failing on it
        // is what `red_set: exhaustive` is for, and the manifests that claim it
        // accept exactly that cost.
        if let Some(before) = baseline.as_ref().and_then(|record| {
            record
                .fired
                .get(&injection.name)
                .filter(|row| injection_harness::records_the_same_injection(row, injection))
        }) {
            let then: BTreeSet<String> = before.tests.iter().cloned().collect();
            let appeared: Vec<&String> = fired.difference(&then).collect();
            let gone: Vec<&String> = then.difference(&fired).collect();
            if !appeared.is_empty() || !gone.is_empty() {
                eprintln!(
                    "[{}] its red set has CHANGED since it was proven — {} new {appeared:?}, \
                     {} no longer red {gone:?}. The injection is the same one; what moved is \
                     the tree around it",
                    injection.name,
                    appeared.len(),
                    gone.len(),
                );
            }
        }
        results.push(InjectionResult {
            name: injection.name.clone(),
            why: injection.why.clone(),
            run,
            fired,
            missed,
            unexpected,
            anchors_gone,
            mechanism_missed,
            targets,
            target_drift: drift,
            edits: injection.edits.clone(),
            expect_red: injection.expect_red.clone(),
        });
    }

    // Print the table BEFORE judging it: a first-violation stop reports one line
    // of a distribution that is the finding.
    println!(
        "{}",
        serde_json::to_string_pretty(&report(&manifest, &scope, &control, results.clone()))
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
        // AND WHETHER IT RAN THE SAME TARGETS, by the same law the control was
        // held to: a run whose names do not account for its results is not
        // comparable to anything, and it must be said before the comparison
        // rather than read out of a gap the blindness itself produced.
        if let Some(unaccounted) = unaccounted_targets(&result.run) {
            broken.push(format!("{}: {unaccounted}", result.name));
        }
        // BY NAME where the suite names its targets, and by count where it does
        // not — a run that lost one target and gained another has a drift of 0
        // and is not comparable to the control at all.
        if !result.targets.is_empty() {
            broken.push(format!(
                "{}: reached {:?} fewer times than the control and {:?} more \
                 (name → control, run) — a run over a different set is a \
                 smaller number, not a cleaner one",
                result.name, result.targets.missing, result.targets.extra
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
        // AND THE EXCUSE IN THE SAME BREATH AS THE EXPECTATION, under BOTH
        // claims. `red_set` decides what an unnamed red means; it decides
        // nothing about a name this manifest declared and then did not see,
        // which is a false statement about the mechanism either way.
        if !result.mechanism_missed.is_empty() {
            broken.push(format!(
                "{}: this manifest says {:?} read its anchors, and this injection \
                 replaced them — they did not go red, so what excuses an unnamed \
                 red here is a claim about a mechanism that has moved",
                result.name, result.mechanism_missed
            ));
        }
        // AND THE OTHER DIRECTION, FOR A MANIFEST THAT CLAIMS ITS SET IS WHOLE.
        // `missed` catches a sweep run somewhere narrower than it was measured;
        // this catches one run wider, and a guard that appeared since. Under the
        // default claim the same number is a finding to write down rather than a
        // failure, so it is reported either way and judged only here.
        if manifest.red_set == RedSet::Exhaustive && !result.unexpected.is_empty() {
            broken.push(format!(
                "{}: {} red this manifest does not name {:?} — it declares its \
                 red sets exhaustive, so this is either a guard nobody had \
                 written down or a set that has decayed since it was measured",
                result.name,
                result.unexpected.len(),
                result.unexpected
            ));
        }
    }
    if !broken.is_empty() {
        return Err(broken.join("\n  "));
    }

    // R1198 — AND WHAT FIRED IS WRITTEN DOWN, because a run that proved
    // something and left no trace is a proof somebody has to make again to know
    // it was ever made. This is the ONLY writer of that record: a row a person
    // could type is a row that says nothing, so the gate over these files is
    // worth exactly as much as this being the only way one appears.
    //
    // AFTER THE JUDGEMENT AND NOT BESIDE IT. Everything above decides whether
    // this run is comparable to its control at all — a suite that did not
    // finish, a target set that drifted, an injection that missed what it named
    // — and evidence written from a run that failed any of those would record
    // firings nobody measured.
    //
    // MERGED, NOT REPLACED, and that is what makes `--only` the cheap way to
    // prove one new injection: a scoped run must not erase the rows a wider one
    // established. What it does overwrite is the row for an injection it just
    // ran, which is how a re-measurement lands.
    let whole = matches!(scope, Scope::EveryInjection { .. });
    if let Err(why) = record_firings(&manifest_path, &results, whole) {
        return Err(format!("the run held and its record did not: {why}"));
    }
    Ok(())
}

/// Merge this run's firings into the sweep's record, creating it if needed.
///
/// `whole` is whether this run covered every injection the manifest holds, and
/// it is the only writer of the record's completeness claim — a claim a person
/// could type would make the gate that reads it a formality.
fn record_firings(
    manifest_path: &str,
    results: &[InjectionResult],
    whole: bool,
) -> Result<(), String> {
    let path = injection_harness::firings_path(Path::new(manifest_path));
    let mut record = injection_harness::read_firings(&path)?.unwrap_or_default();
    record.prose = vec![
        "Machine-written by tools/injection-harness. Do not edit by hand: a row \
         somebody typed is evidence of nothing, and the law over this file is \
         worth exactly what its being unforgeable is worth."
            .to_string(),
        "Each row is one injection that WAS RUN and DID redden what it names, \
         kept beside the definition it was proven against — edit the injection \
         and the gate says the evidence is about a different one."
            .to_string(),
    ];
    // A WHOLE RUN CLAIMS COMPLETENESS AND A SCOPED ONE NEVER WITHDRAWS IT. The
    // rows an earlier whole run established are still evidence; what a `--only`
    // run does is replace one of them. The claim going stale — a manifest that
    // gained an injection since — is the gate's to notice, and it can only
    // notice it while the claim is still written down.
    record.complete = record.complete || whole;
    for result in results {
        record.fired.insert(
            result.name.clone(),
            injection_harness::Firing {
                edits: result.edits.clone(),
                expect_red: result.expect_red.clone(),
                tests: result.fired.iter().cloned().collect(),
            },
        );
    }
    injection_harness::write_firings(&path, &record)
}

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// Apply every edit, each of which must match EXACTLY once.
///
/// The pre-flight has already dry-run this whole sequence against the snapshot,
/// so reaching a refusal here means the tree changed under the sweep — which is
/// worth refusing on its own, and is why the check lives on the write path too
/// rather than being trusted to the run that happened minutes ago.
fn apply(repo: &Path, edits: &[Edit]) -> Result<(), String> {
    for edit in edits {
        let path = repo.join(&edit.file);
        let text =
            fs::read_to_string(&path).map_err(|e| format!("{} unreadable: {e}", path.display()))?;
        fs::write(&path, replace_once(&text, edit)?)
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
/// and `Doc-tests foo` is another.
///
/// THE TRAILING HASH IS DROPPED, AND THE REASON THIS SAID FOR YEARS IS FALSE.
/// It read "it moves whenever the crate is rebuilt, which is every injection".
/// Measured across one sweep's control and two injected runs — three whole
/// rebuilds of an edited crate — all four of that tree's hashes were identical.
/// It is cargo's `-C metadata` fingerprint, computed from the package, its
/// features and the profile, and a source edit does not touch any of them. So
/// the hash WOULD have told the two `all` targets below apart.
///
/// It is dropped anyway, for the reasons that survive measuring: it is cargo's
/// internal spelling rather than a fact about coverage, it is unreadable in the
/// one place this name is ever printed — a drift message a person has to act on
/// — and it changes nothing this tool DOES, because the refusal a lost target
/// earns is the same either way. What it costs is stated rather than hidden:
/// without it two targets can share a name, which is why what counts them is a
/// multiset (see `Targets`) and not a set.
///
/// THE BINARY STEM ALONE IS NOT UNIQUE, and this repository is where that shows:
/// a crate with both a library and a binary runs its unit tests twice under one
/// stem, so `mnemosyne_cli`, `mnemosyne_index` and `mnemosyne_render` each named
/// two targets and 151 targets came out as 148 names. What cargo was RUNNING —
/// `unittests src/lib.rs` against `unittests src/main.rs` — therefore goes in
/// the name: it is free information that tells two real targets apart.
///
/// It does NOT make the name unique, and it was never able to. Cargo announces a
/// target with a path relative to its own package and no package at all, so the
/// folded `tests/all.rs` suites of two crates are announced identically. That is
/// why what counts targets is a multiset (see `Targets`) rather than a set: the
/// name is as much identity as the log carries, and the remainder is a count.
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
/// Does this line open one of libtest's captured-output blocks?
///
/// `---- <test name> stdout ----`, and the `stderr` form beside it. What follows
/// is THE SUBJECT'S OWN OUTPUT, replayed because that test failed.
fn opens_captured_output(line: &str) -> bool {
    let line = line.trim_end();
    line.starts_with("---- ") && line.ends_with(" ----")
}

fn summarize(text: &str) -> Run {
    let mut run = Run::default();
    let mut in_failures = false;
    // WHAT A FAILING TEST PRINTED IS NOT WHAT THIS RUN DID (R1230).
    //
    // libtest replays a failed test's captured output under `---- <name> stdout
    // ----`, and in this repository that output is routinely another cargo's:
    // the hook suite drives real `pre-push` over throwaway trees, so an assertion
    // that prints what the hook said prints a nested `Running unittests
    // src/lib.rs (…)` and its `test result:` line with it. Read as this run's own,
    // they are targets that appeared out of nowhere — measured here, a sweep of
    // three injections reported `3 targets, 0 missing, 2 extra` and refused,
    // where every injection had in fact fired exactly as named.
    //
    // IT IS THE SHAPE R1125 PAID FOR ONE TOOL OVER: an oracle that was a
    // substring of the failure output. The repair is the same — read the run's
    // own announcements and skip the block that is somebody else's.
    //
    // WHAT IT CANNOT DO, said rather than implied: the block is left at the
    // `failures:` list libtest writes after the last one, so a subject printing
    // an unindented `failures:` of its own inside its captured output would end
    // the skip early. Nothing here can tell that apart from libtest's — the two
    // are the same bytes in one stream — and the honest boundary is the marker,
    // not a guess about the words after it.
    let mut in_captured_output = false;
    for line in text.lines() {
        if opens_captured_output(line) {
            in_captured_output = true;
            in_failures = false;
            continue;
        }
        if in_captured_output {
            if line.trim_end() == "failures:" {
                in_captured_output = false;
                in_failures = true;
            }
            continue;
        }
        if let Some(name) = target_name(line) {
            *run.reached.entry(name).or_default() += 1;
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
        // EVERY NAME THE RUN EXECUTED, which is the only list of them that is
        // the harness's own words rather than a second derivation of them.
        // `test <name> ... ok` — checked AFTER `test result:` above, which the
        // same prefix would otherwise swallow.
        if let Some(rest) = line.strip_prefix("test ") {
            if let Some((name, _outcome)) = rest.rsplit_once(" ... ") {
                // A `#[should_panic]` TEST IS PRINTED WITH ITS ANNOTATION AND IS
                // NOT NAMED THAT (R1198). libtest writes `test <name> - should
                // panic ... ok`, and a reader taking everything before ` ... `
                // records a name no `expect_red` can ever match — so an
                // injection aimed at such a test scores `missed` forever, and
                // since R1183 the pre-flight refuses the WHOLE manifest for it.
                // Measured: `tools/ci-plan/injection-sweep.json` could not be
                // run whole from the day one of its targets gained the
                // annotation, and every `--only` run went on passing because
                // that check is scoped to what a run selects.
                //
                // THE LITERAL SUFFIX AND NOT ` - `, because a doc-test's own
                // name holds that separator (`src/lib.rs - item (line 41)`) and
                // cutting at it would rename every doc-test in the tree.
                let name = name.strip_suffix(" - should panic").unwrap_or(name);
                run.ran.insert(name.to_string());
                continue;
            }
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
        // AND WHICH NAMES RAN, red or green — the population a plan's
        // expectations are addressed into. `test result:` shares the prefix and
        // is not one of them.
        assert_eq!(
            run.ran,
            BTreeSet::from(["alpha".to_string(), "beta".to_string(), "gamma".to_string()]),
            "every executed name is read, and no line that merely starts with `test `"
        );
    }

    /// R1198 — THE ANNOTATION IS NOT PART OF THE NAME, and reading it as one
    /// made a whole manifest unrunnable for as long as one of its targets
    /// carried it.
    #[test]
    fn a_should_panic_test_is_named_by_its_own_name() {
        let run = summarize(
            "\
running 3 tests
test refuses_a_runner_it_cannot_name - should panic ... ok
test src/lib.rs - documented (line 41) ... ok
test src/lib.rs - panicking (line 7) - should panic ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
",
        );
        assert!(
            run.ran.contains("refuses_a_runner_it_cannot_name"),
            "an `expect_red` names the test, not the annotation libtest prints \
             beside it: {:?}",
            run.ran
        );
        // AND THE SEPARATOR IS NOT CUT WHERE A DOC-TEST'S OWN NAME HOLDS ONE.
        assert!(
            run.ran.contains("src/lib.rs - documented (line 41)")
                && run.ran.contains("src/lib.rs - panicking (line 7)"),
            "a doc-test is named `<file> - <item> (line N)`, so cutting at ` - ` \
             rather than at the literal suffix would rename every one of them: \
             {:?}",
            run.ran
        );
    }

    #[test]
    fn a_name_a_plan_is_addressed_to_is_looked_for_among_the_names_that_ran() {
        // THE DECISION THE PRE-FLIGHT MAKES, over the words a harness actually
        // prints: the module path is part of what it prints, and a plan is
        // written with the function's own name.
        let ran = summarize(
            "\
running 3 tests
test git_hooks_smoke::the_hook_rejects ... ok
test audit::tests::subscribe_receives ... ok
test plain ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
",
        )
        .ran;
        assert!(reached(&ran, "the_hook_rejects"), "{ran:?}");
        assert!(reached(&ran, "audit::tests::subscribe_receives"), "{ran:?}");
        assert!(reached(&ran, "plain"), "{ran:?}");

        // AND THE STALE SHAPES THIS EXISTS TO CATCH: a function renamed, and a
        // MODULE renamed under an expectation that names one. Both leave the
        // sweep's anchors applying perfectly to a plan aimed at nothing.
        assert!(!reached(&ran, "the_hook_rejects_it"), "{ran:?}");
        assert!(
            !reached(&ran, "live_tail::tests::subscribe_receives"),
            "{ran:?}"
        );
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
            Targets::from([
                (
                    "counted_without_naming:unittests src/main.rs".to_string(),
                    1
                ),
                ("doc:mnemosyne-ops".to_string(), 1)
            ]),
            "the hash is cargo's internal spelling of a target, not a fact about \
             what ran; see `target_name` for what dropping it costs and what the \
             measurement said about the reason this file used to give"
        );
        assert_eq!(run.targets, 2);
    }

    /// WHAT A FAILING TEST PRINTED IS NOT WHAT THIS RUN DID (R1230).
    ///
    /// This log is the shape a hook-suite sweep produces: the failing case drives
    /// a real `pre-push` over a throwaway tree and prints what it said, so
    /// ANOTHER cargo's announcements and totals are inside libtest's captured
    /// block. Read as this run's own they are two targets that appeared out of
    /// nowhere, and the harness refuses the sweep for target drift — measured, on
    /// three injections that had each fired exactly as named.
    #[test]
    fn what_a_failing_test_printed_is_not_what_this_run_ran() {
        let log = "\
     Running tests/all.rs (target/debug/deps/all-aa11)

failures:

---- git_hooks_smoke::pre_push_refuses_a_tree stdout ----
thread 'git_hooks_smoke::pre_push_refuses_a_tree' panicked at the gate said:
     Running unittests src/lib.rs (target/debug/deps/sub-bb22)
test tests::the_subjects_own ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
   Doc-tests sub
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

failures:
    git_hooks_smoke::pre_push_refuses_a_tree

test result: FAILED. 32 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
";
        let run = summarize(log);
        assert_eq!(
            run.reached,
            Targets::from([("all:tests/all.rs".to_string(), 1)]),
            "a target the SUBJECT ran is not one this run reached"
        );
        assert_eq!(run.targets, 1, "and neither are its `test result:` lines");
        assert_eq!((run.passed, run.failed), (32, 1), "nor its totals");
        // AND THE THREE ANSWERS THAT MUST SURVIVE THE SKIP: the red name is read
        // off the list AFTER the block, and the subject's own passing test is not
        // recorded as one this run executed.
        assert!(
            run.red.contains("git_hooks_smoke::pre_push_refuses_a_tree"),
            "{:?}",
            run.red
        );
        assert!(
            run.ran.is_empty(),
            "the only `test … ok` line in this log is the SUBJECT'S, and a run \
             that claims to have executed it would let an injection aimed at a \
             name score `missed` against a test it never touched: {:?}",
            run.ran
        );
    }

    #[test]
    fn one_crate_running_two_target_kinds_is_two_names() {
        // A crate with a library AND a binary runs its unit tests twice under
        // one binary stem. Three such crates in this repository turned 151
        // targets into 148 names, and what cargo was RUNNING is what tells the
        // two apart — free information the announce line already carries.
        let log = "\
     Running unittests src/lib.rs (target/debug/deps/mnemosyne_cli-aa11)
test result: ok. 1 passed; 0 failed
     Running unittests src/main.rs (target/debug/deps/mnemosyne_cli-bb22)
test result: ok. 1 passed; 0 failed
";
        let run = summarize(log);
        assert_eq!(run.reached.len(), 2, "{:?}", run.reached);
        assert!(unaccounted_targets(&run).is_none());
    }

    #[test]
    fn two_targets_that_share_a_name_are_counted_and_not_collapsed() {
        // WHAT USED TO STOP THE WIDEST SWEEP THIS REPOSITORY HAS. Cargo
        // announces a test target by a path relative to ITS OWN package, so the
        // folded suites of `mnemosyne-cli` and `mnemosyne-server` are both
        // `Running tests/all.rs (…/deps/all-<hash>)` and there is nothing in the
        // line to tell them apart. Under a set that pair was one element, the
        // control read "2 targets under 1 name", and the harness refused the
        // manifest for a collision it could do nothing about.
        let run = summarize(
            "     Running tests/all.rs (target/debug/deps/all-aa11)
test result: ok. 40 passed; 0 failed
     Running tests/all.rs (target/debug/deps/all-bb22)
test result: ok. 30 passed; 0 failed
",
        );
        assert_eq!(
            run.reached,
            Targets::from([("all:tests/all.rs".to_string(), 2)]),
            "the name is as much identity as the log carries; the rest is a count"
        );
        assert_eq!(run.targets, 2);
        assert!(
            unaccounted_targets(&run).is_none(),
            "two targets under one name are two targets the comparison can see"
        );
    }

    #[test]
    fn a_target_lost_from_a_shared_name_is_still_drift() {
        // And this is why the count is enough: uniqueness bought the ability to
        // say WHICH of an indistinguishable pair went missing, which no reading
        // of this log could give. That one of them did is said exactly.
        let control = summarize(
            "     Running tests/all.rs (target/debug/deps/all-aa11)
test result: ok. 40 passed; 0 failed
     Running tests/all.rs (target/debug/deps/all-bb22)
test result: ok. 30 passed; 0 failed
",
        );
        let after = summarize(
            "     Running tests/all.rs (target/debug/deps/all-cc33)
test result: ok. 40 passed; 0 failed
",
        );
        let gap = compare_targets(&control.reached, &after.reached);
        assert_eq!(
            gap.missing,
            BTreeMap::from([("all:tests/all.rs".to_string(), (2, 1))]),
            "a set would have called these two runs identical coverage"
        );
        assert!(gap.extra.is_empty(), "{gap:?}");
    }

    #[test]
    fn a_lost_target_and_a_gained_one_do_not_cancel() {
        // The count is 2 either way; the NAMES are what say these two runs are
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
        let gap = compare_targets(&control.reached, &after.reached);
        assert_eq!(
            gap.missing,
            BTreeMap::from([("beta:unittests src/lib.rs".to_string(), (1, 0))])
        );
        assert_eq!(
            gap.extra,
            BTreeMap::from([("gamma:unittests src/lib.rs".to_string(), (0, 1))])
        );
    }

    #[test]
    fn a_result_no_line_announced_is_a_target_no_name_covers() {
        // THE HONEST REMAINDER OF THE UNIQUENESS REFUSAL. Names that collide are
        // fine now; a `test result:` with no `Running` line before it is not,
        // because the by-name comparison is blind to exactly that target and
        // would be weaker than the count it replaced.
        let run = summarize(
            "     Running unittests src/lib.rs (target/debug/deps/alpha-1)
test result: ok. 1 passed; 0 failed
test result: ok. 1 passed; 0 failed
",
        );
        let said = unaccounted_targets(&run).expect("one result is covered by no name");
        assert!(said.contains("announced 1 target(s) by name"), "{said}");
        assert!(said.contains("printed 2 result(s)"), "{said}");
    }

    #[test]
    fn a_suite_that_announces_no_target_at_all_is_left_to_its_count() {
        // A suite is free to print no announce lines — the fake suites this
        // tool's own tests run do exactly that. The count is then all there is,
        // and the drift check says so and falls back to it; refusing here would
        // refuse every manifest whose suite is not cargo.
        let run = summarize("test result: ok. 2 passed; 0 failed\n");
        assert_eq!(run.targets, 1);
        assert!(run.reached.is_empty());
        assert!(unaccounted_targets(&run).is_none());
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
