//! The harness put to a tree it can break — the properties it exists to hold,
//! asserted against a fake suite rather than against `cargo test`.
//!
//! The suite is a manifest field precisely so this file can supply one: a shell
//! script that prints a canned cargo log and whose verdict depends on whether
//! the injection landed. A harness that could only be tested by running the real
//! suite would be tested once and then trusted.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_injection-harness")
}

/// A tree with one source file and a suite that goes red exactly when that file
/// no longer says `HEALTHY`.
///
/// The suite EXITS NON-ZERO when it goes red, as a real one does. That is not
/// decoration: the harness now holds the exit code and the failure list to each
/// other, so a fixture that always exited 0 would be a fixture no real suite
/// behaves like, and every test built on it would pass for the wrong reason.
fn tree(root: &Path, source: &str) -> PathBuf {
    fs::create_dir_all(root.join("logs")).expect("mkdir");
    fs::write(root.join("src.txt"), source).expect("write source");
    let suite = root.join("suite.sh");
    fs::write(
        &suite,
        // THE PER-TEST LINES ARE PART OF WHAT A SUITE PRINTS, and leaving them
        // out made this fixture unlike the thing it stands for in a way that
        // mattered: the pre-flight that asks whether a plan's names still exist
        // reads exactly those lines, so a fixture without them exercises the
        // branch where the harness declines to judge instead of the one where
        // it judges.
        "#!/bin/sh\n\
         if grep -q HEALTHY src.txt; then\n\
         printf 'test the_law ... ok\\ntest the_other ... ok\\n\\ntest result: ok. 2 passed; 0 failed; 0 ignored\\n'\n\
         else\n\
         printf 'test the_law ... FAILED\\ntest the_other ... ok\\n\\nfailures:\\n    the_law\\n\\ntest result: FAILED. 1 passed; 1 failed; 0 ignored\\n'\n\
         exit 1\n\
         fi\n",
    )
    .expect("write suite");
    make_runnable(&suite);
    suite
}

/// A suite whose log and whose exit code are supplied SEPARATELY for each of the
/// two tree states, so a run can be made to say one thing in its log and another
/// in its status — which is the disagreement under test.
fn split_verdict_suite(
    root: &Path,
    healthy: (&str, i32),
    broken: (&str, i32),
    source: &str,
) -> PathBuf {
    fs::create_dir_all(root.join("logs")).expect("mkdir");
    fs::write(root.join("src.txt"), source).expect("write source");
    let suite = root.join("suite.sh");
    let (healthy_log, healthy_exit) = healthy;
    let (broken_log, broken_exit) = broken;
    fs::write(
        &suite,
        format!(
            "#!/bin/sh\n\
             if grep -q HEALTHY src.txt; then\n\
             printf '{healthy_log}'\n\
             exit {healthy_exit}\n\
             else\n\
             printf '{broken_log}'\n\
             exit {broken_exit}\n\
             fi\n"
        ),
    )
    .expect("write suite");
    make_runnable(&suite);
    suite
}

/// A cargo-shaped log with no failing test in it.
const ALL_GREEN: &str = "test result: ok. 2 passed; 0 failed; 0 ignored\\n";

/// A cargo-shaped log that names one failing test.
const ONE_RED: &str =
    "failures:\\n    the_law\\n\\ntest result: FAILED. 1 passed; 1 failed; 0 ignored\\n";

fn make_runnable(suite: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(suite, fs::Permissions::from_mode(0o755)).expect("chmod");
    }
}

/// Run a program this process wrote a moment ago, past the kernel's window for
/// saying it is still being written.
///
/// WHY THIS IS NOT A FLAKE TO SHRUG AT. `ETXTBSY` on exec means some process
/// holds the file open for writing, and after `fs::copy` returns this one does
/// not. Another does: a sibling test thread that forks between our write and our
/// exec hands its child a copy of the whole descriptor table, and until that
/// child reaches its own `exec` — which is where `O_CLOEXEC` finally closes our
/// descriptor — the kernel is right that somebody has the file open. The window
/// is microseconds wide and CI is where it gets hit, which is exactly the shape
/// that reads as a gate being unreliable rather than a gate being right.
///
/// A CONDITION AND NOT A SLEEP. This retries on that one error kind and on
/// nothing else, so a program that is genuinely missing or not executable fails
/// on the first attempt with its own message. The bound exists so that a real
/// permanent `ETXTBSY` ends as a failure rather than a hang.
fn run_a_freshly_written_program(program: &Path, argument: &Path) -> std::process::Output {
    for _ in 0..1_000 {
        match Command::new(program).arg(argument).output() {
            Err(busy) if busy.kind() == std::io::ErrorKind::ExecutableFileBusy => continue,
            other => return other.expect("the copied tool runs"),
        }
    }
    panic!(
        "{} was reported as still being written on every attempt — that is no \
         longer another thread's fork-and-exec window",
        program.display()
    )
}

fn manifest(root: &Path, injections: serde_json::Value) -> PathBuf {
    manifest_body(
        root,
        injections,
        serde_json::Value::Null,
        serde_json::Value::Null,
    )
}

fn manifest_with(
    root: &Path,
    injections: serde_json::Value,
    min_free_mb: serde_json::Value,
) -> PathBuf {
    manifest_body(root, injections, min_free_mb, serde_json::Value::Null)
}

/// A manifest claiming its `expect_red` lists name the WHOLE red set.
fn manifest_claiming_every_red(root: &Path, injections: serde_json::Value) -> PathBuf {
    manifest_body(
        root,
        injections,
        serde_json::Value::Null,
        serde_json::json!("exhaustive"),
    )
}

/// The one place a fixture manifest is written, so the optional fields cannot
/// come to differ between the builders above.
fn manifest_body(
    root: &Path,
    injections: serde_json::Value,
    min_free_mb: serde_json::Value,
    red_set: serde_json::Value,
) -> PathBuf {
    let path = root.join("manifest.json");
    let mut body = serde_json::json!({
        "repo": root,
        "test_command": [root.join("suite.sh")],
        "logs": root.join("logs"),
        "injections": injections,
    });
    if !min_free_mb.is_null() {
        body["min_free_mb"] = min_free_mb;
    }
    if !red_set.is_null() {
        body["red_set"] = red_set;
    }
    fs::write(&path, serde_json::to_string(&body).expect("json")).expect("write manifest");
    path
}

fn harness(manifest: &Path) -> std::process::Output {
    Command::new(binary())
        .arg(manifest)
        .output()
        .expect("harness runs")
}

/// The same, naming which injections to run.
fn harness_only(manifest: &Path, only: &[&str]) -> std::process::Output {
    let mut command = Command::new(binary());
    command.arg(manifest);
    for name in only {
        command.arg("--only").arg(name);
    }
    command.output().expect("harness runs")
}

/// Two injections, of which the second is the one a case below asks for.
fn two_injections() -> serde_json::Value {
    serde_json::json!([
        {
            "name": "I1",
            "why": "the first, which a scoped run must not touch",
            "edits": [{"file": "src.txt", "from": "HEALTHY", "to": "BROKEN"}],
            "expect_red": ["the_law"],
        },
        {
            "name": "I2",
            "why": "the second, and the one asked for by name",
            // A DIFFERENT ANCHOR ON THE SAME WORD THE SUITE WATCHES. The fake
            // suite goes red exactly when `HEALTHY` is gone, so an injection
            // that edits anything else is one the harness refuses as misaimed —
            // which is what the first draft of this fixture was, and what it was
            // told.
            "edits": [{"file": "src.txt", "from": "HEALTHY here", "to": "BROKEN here"}],
            "expect_red": ["the_law"],
        }
    ])
}

/// One injection can be run by name, and the others are not run at all.
///
/// R1179 — THE PRICE OF PROVING ONE INJECTION USED TO BE THE WHOLE MANIFEST. With
/// no way to name one, asking "does the injection I just wrote reach the test I
/// aimed it at" cost fifty-two runs and twenty minutes, and R1178 paid that once
/// and still shipped a misaimed injection. A verification that expensive gets
/// written down as an intention instead of run.
#[test]
fn one_injection_can_be_asked_for_by_name() {
    let root = tempdir();
    tree(root.path(), "the wire is HEALTHY here\n");
    let path = manifest(root.path(), two_injections());
    let out = harness_only(&path, &["I2"]);
    let report = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "the scoped sweep should pass: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        report.contains("\"I2\""),
        "the named injection ran: {report}"
    );
    assert!(
        !report.contains("\"I1\""),
        "AND THE OTHER DID NOT — a filter that ran everything and reported one \
         would agree with the assertion above: {report}"
    );
    // AND THE REPORT SAYS WHAT IT DID NOT MEASURE, which is the same lesson the
    // report's first field records: a red set is a fact about a population, and
    // one injection of two must not print the shape of a sweep of both.
    assert!(
        report.contains("\"of\": 2"),
        "the report names the scope it ran under: {report}"
    );
}

/// A sweep of everything says so, rather than saying nothing.
///
/// THE CONTROL FOR THE FIELD ABOVE. A `scope` that only appeared under `--only`
/// would leave every unscoped report ambiguous in exactly the way the scoped one
/// was, and a reader could not tell an old report from a full one.
#[test]
fn a_sweep_of_every_injection_says_which_population_that_was() {
    let root = tempdir();
    tree(root.path(), "the wire is HEALTHY here\n");
    let path = manifest(root.path(), two_injections());
    let out = harness(&path);
    let report = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        report.contains("EveryInjection") && report.contains("\"count\": 2"),
        "an unscoped run names its population too: {report}"
    );
}

/// A name that matches no injection is a refusal, not an empty sweep.
///
/// THE "0 MEANS SUSPECT THE INJECTION" RULE, APPLIED TO THE ASKING. A misspelled
/// name that ran nothing would print a clean report and exit 0 — a green verdict
/// about a measurement that never happened, in the one place nobody re-reads.
#[test]
fn a_name_that_matches_no_injection_is_refused_rather_than_run_as_nothing() {
    let root = tempdir();
    tree(root.path(), "the wire is HEALTHY here\n");
    let path = manifest(root.path(), two_injections());
    let out = harness_only(&path, &["I3"]);
    assert!(!out.status.success(), "a misspelling is not a clean sweep");
    let why = String::from_utf8_lossy(&out.stderr);
    assert!(
        why.contains("I3") && why.contains("I1") && why.contains("I2"),
        "and it says what was asked for and what the manifest carries: {why}"
    );
}

#[test]
fn an_injection_that_fires_is_reported_and_the_tree_comes_back() {
    let root = tempdir();
    tree(root.path(), "the wire is HEALTHY here\n");
    let path = manifest(
        root.path(),
        serde_json::json!([{
            "name": "I1",
            "why": "the law is not vacuous",
            "edits": [{"file": "src.txt", "from": "HEALTHY", "to": "BROKEN"}],
            "expect_red": ["the_law"],
        }]),
    );
    let out = harness(&path);
    let report = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "the sweep should pass: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        report.contains("\"the_law\""),
        "the red name is reported: {report}"
    );
    assert_eq!(
        fs::read_to_string(root.path().join("src.txt")).expect("read back"),
        "the wire is HEALTHY here\n",
        "THE TREE COMES BACK — the restore is what makes the next injection a \
         measurement rather than a compound of the ones before it"
    );
}

#[test]
fn an_edit_that_matches_twice_is_refused_before_the_suite_is_asked() {
    let root = tempdir();
    tree(root.path(), "HEALTHY and HEALTHY\n");
    let path = manifest(
        root.path(),
        serde_json::json!([{
            "name": "I1",
            "edits": [{"file": "src.txt", "from": "HEALTHY", "to": "BROKEN"}],
        }]),
    );
    let out = harness(&path);
    assert!(!out.status.success(), "a two-match edit must not run");
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(
        said.contains("occurs 2 times"),
        "and it must say how many: {said}"
    );
    assert!(
        !root.path().join("logs").join("I1.log").exists(),
        "the suite was never asked"
    );
    assert!(
        !root.path().join("logs").join("control.log").exists(),
        "NOR WAS THE CONTROL — which is what this test's name has always \
         claimed. An anchor is a property of the manifest and the tree, and \
         both are known before a single target is built; paying a whole-suite \
         run to find out is paying for nothing"
    );
}

#[test]
fn an_edit_that_matches_nothing_is_refused_too() {
    let root = tempdir();
    tree(root.path(), "the wire is HEALTHY here\n");
    let path = manifest(
        root.path(),
        serde_json::json!([{
            "name": "I1",
            "edits": [{"file": "src.txt", "from": "ABSENT", "to": "BROKEN"}],
        }]),
    );
    let out = harness(&path);
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("occurs 0 times"),
        "a replacement that landed nowhere produces a run whose silence reads \
         as `the injection did not fire`"
    );
    assert!(
        !root.path().join("logs").join("control.log").exists(),
        "and nothing was built to find that out"
    );
}

#[test]
fn a_typo_in_the_last_injection_costs_no_run_at_all() {
    // THE CASE THE PRE-FLIGHT EXISTS FOR, and the one a single-injection
    // manifest cannot show: the anchors are checked where `apply` calls for
    // them, which is after the control AND after every injection ahead of this
    // one. A nine-injection plan whose last anchor has a typo therefore spends
    // the control plus eight whole-suite runs — tens of minutes each on this
    // machine — to arrive at a message about a string, having measured nothing
    // and having edited the tree eight times on the way.
    //
    // So the assertion is not "it refused" — it refused before too — but WHEN:
    // no log exists at all, which means no target was built.
    let root = tempdir();
    tree(root.path(), "the wire is HEALTHY here\n");
    let path = manifest(
        root.path(),
        serde_json::json!([
            {
                "name": "I1",
                "edits": [{"file": "src.txt", "from": "HEALTHY", "to": "BROKEN"}],
                "expect_red": ["the_law"],
            },
            {
                "name": "I2",
                "edits": [{"file": "src.txt", "from": "ABSENT", "to": "BROKEN"}],
            },
        ]),
    );
    let out = harness(&path);
    assert!(!out.status.success());
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(
        said.lines()
            .any(|line| line.contains("I2") && line.contains("occurs 0 times")),
        "ONE LINE names both which injection cannot apply and why, because a \
         manifest with nine of them is where this matters. Asserted per line \
         rather than over the whole of stderr: the sweep also announces each \
         injection by name as it starts one, so `contains(\"I2\")` anywhere is \
         satisfied by a progress line and would pass a build that dropped the \
         name from the refusal itself: {said}"
    );
    assert!(
        !root.path().join("logs").join("control.log").exists()
            && !root.path().join("logs").join("I1.log").exists(),
        "and NOTHING ran — not the control, and not the injection ahead of the \
         broken one, which is the whole cost this check removes"
    );
    assert_eq!(
        fs::read_to_string(root.path().join("src.txt")).expect("read back"),
        "the wire is HEALTHY here\n",
        "the tree was never edited either"
    );
}

#[test]
fn an_injection_whose_second_edit_rewrites_its_first_is_allowed_through() {
    // THE CONTROL GROUP FOR THE PRE-FLIGHT, and the reason it is a dry run
    // rather than a count against the pristine bytes. Edits inside one injection
    // are sequential, so an anchor that exists only because the edit before it
    // wrote it occurs ZERO times in the file on disk. A pre-flight that counted
    // against the snapshot would refuse this manifest for a reason outside its
    // own law — the KK6 defect, where a gate rejects exactly what it was aimed
    // at and tells the author the evidence is untrustworthy.
    let root = tempdir();
    tree(root.path(), "the wire is HEALTHY here\n");
    let path = manifest(
        root.path(),
        serde_json::json!([{
            "name": "I1",
            "edits": [
                {"file": "src.txt", "from": "HEALTHY", "to": "MIDWAY"},
                {"file": "src.txt", "from": "MIDWAY", "to": "BROKEN"},
            ],
            "expect_red": ["the_law"],
        }]),
    );
    let out = harness(&path);
    assert!(
        out.status.success(),
        "a chained pair of edits is a legitimate injection: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        root.path().join("logs").join("I1.log").exists(),
        "and it actually ran"
    );
    assert_eq!(
        fs::read_to_string(root.path().join("src.txt")).expect("read back"),
        "the wire is HEALTHY here\n",
        "and the tree came back"
    );
}

#[test]
fn a_red_control_stops_the_sweep() {
    let root = tempdir();
    // The tree starts broken, so the control itself is red.
    tree(root.path(), "the wire is BROKEN already\n");
    let path = manifest(
        root.path(),
        serde_json::json!([{
            "name": "I1",
            "edits": [{"file": "src.txt", "from": "BROKEN", "to": "WORSE"}],
        }]),
    );
    let out = harness(&path);
    assert!(
        !out.status.success(),
        "a sweep from a red tree measures nothing"
    );
    assert!(String::from_utf8_lossy(&out.stderr).contains("control is 1 red"));
}

#[test]
fn a_plan_aimed_at_a_test_that_no_longer_exists_stops_the_sweep_at_the_control() {
    // THE DECAY ONE LEVEL IN FROM R1183's. That round asked whether a sweep's
    // command still names a SUITE that exists and found three that did not; this
    // is whether its plan still names TESTS inside that suite. Both leave every
    // anchor applying perfectly to a proof aimed at nothing.
    //
    // It is caught at the CONTROL, which is the whole point: the same staleness
    // reaches the report as `missed` — the word a misaimed injection also gets —
    // and only after the suite has run once per injection.
    let root = tempdir();
    tree(root.path(), "the wire is HEALTHY here\n");
    let path = manifest(
        root.path(),
        serde_json::json!([{
            "name": "I1",
            "edits": [{"file": "src.txt", "from": "HEALTHY", "to": "BROKEN"}],
            "expect_red": ["the_law", "the_law_under_its_old_name"],
        }]),
    );
    let out = harness(&path);
    assert!(
        !out.status.success(),
        "a plan addressed at a test that does not exist proves nothing"
    );
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("I1: the_law_under_its_old_name"),
        "the refusal must name the injection and the expectation:\n{err}"
    );
    // AND NOT THE ONE THAT IS STILL THERE, which is what makes this a reader of
    // the names rather than a reader of the list's length.
    assert!(
        !err.contains("I1: the_law\n"),
        "an expectation that still names a test is not a finding:\n{err}"
    );
    // THE COST IT SAVES IS THE POINT: no injection was applied at all.
    assert!(
        !err.contains("applying"),
        "the sweep must stop before it edits the tree:\n{err}"
    );
}

#[test]
fn an_injection_that_reaches_nothing_it_was_aimed_at_is_a_failure_of_the_sweep() {
    let root = tempdir();
    tree(root.path(), "the wire is HEALTHY here\n");
    let path = manifest(
        root.path(),
        serde_json::json!([{
            "name": "I1",
            // Edits a part of the file the suite does not read, so nothing goes
            // red — the shape of a misaimed injection.
            "edits": [{"file": "src.txt", "from": "here", "to": "there"}],
            "expect_red": ["the_law"],
        }]),
    );
    let out = harness(&path);
    assert!(
        !out.status.success(),
        "0 red against a named expectation is a misaimed injection, not a \
         clean surface"
    );
    assert!(String::from_utf8_lossy(&out.stderr).contains("did not reach"));
}

#[test]
fn the_control_can_be_asked_for_on_its_own() {
    let root = tempdir();
    tree(root.path(), "the wire is HEALTHY here\n");
    let path = manifest(root.path(), serde_json::json!([]));
    let out = Command::new(binary())
        .arg(&path)
        .arg("--control-only")
        .output()
        .expect("harness runs");
    assert!(out.status.success());
    let report = String::from_utf8_lossy(&out.stdout);
    assert!(report.contains("\"passed\": 2"), "{report}");
}

/// Every report names the suite its counts are facts about, in both the modes
/// that print one.
///
/// THE DEFECT THIS IS ABOUT WAS NOT IN THE HARNESS BUT IN WHAT A PERSON DID
/// WITH ITS OUTPUT. Round 1138 read a `fired` set of exactly one off a sweep of
/// its own contract and wrote down what nothing else in the repository guards.
/// The run had been scoped `-p mnemosyne-cli` while the edits landed in
/// `mnemosyne-validate`, so the edited crate's own suite never ran and could not
/// have appeared in any red set. The number was right about the run and wrong
/// about the repository, and the report it was read off carried no population to
/// contradict it.
///
/// THE ORACLE IS THE VALUE AND NOT THE PRESENCE OF A KEY. This fixture's command
/// is a path under a temp directory unique to this run, so a field filled with a
/// plausible constant — `["cargo", "test"]` — fails here, where "the field is
/// non-empty" would pass. And the shape is asserted alongside it, because the
/// control-only mode used to print a bare run object with no `injections` at
/// all: a reader would have had to know which flag produced the file in front of
/// them to know what it measured.
#[test]
fn every_report_names_the_suite_whose_counts_it_carries() {
    let root = tempdir();
    let suite = tree(root.path(), "the wire is HEALTHY here\n");
    let path = manifest(
        root.path(),
        serde_json::json!([{
            "name": "I1",
            "why": "the law is not vacuous",
            "edits": [{"file": "src.txt", "from": "HEALTHY", "to": "BROKEN"}],
            "expect_red": ["the_law"],
        }]),
    );

    let full = harness(&path);
    assert!(
        full.status.success(),
        "{}",
        String::from_utf8_lossy(&full.stderr)
    );
    let control_only = Command::new(binary())
        .arg(&path)
        .arg("--control-only")
        .output()
        .expect("harness runs");
    assert!(
        control_only.status.success(),
        "{}",
        String::from_utf8_lossy(&control_only.stderr)
    );

    let expected = serde_json::json!([suite]);
    for (mode, out, injections) in [
        ("a full sweep", &full, 1),
        ("--control-only", &control_only, 0),
    ] {
        let report: serde_json::Value =
            serde_json::from_slice(&out.stdout).expect("the report is JSON");
        assert_eq!(
            report["test_command"], expected,
            "{mode} printed its counts under {} rather than under the suite that \
             produced them",
            report["test_command"]
        );
        assert_eq!(
            report["injections"].as_array().map(Vec::len),
            Some(injections),
            "{mode} should print the one report shape, holding the injections it \
             has: {}",
            report["injections"]
        );
    }
}

/// A red the manifest does not name fails the sweep exactly where the manifest
/// claims to name them all, and is counted everywhere else.
///
/// THE TWO CLAIMS ANSWER DIFFERENT QUESTIONS AND A SWEEP MAKES ONLY ONE OF THEM.
/// `missed` catches a run narrower than the one a manifest was written against.
/// This is the other direction — a run wider than that one, or a guard somebody
/// added since — and under the default claim it is a finding to write down
/// rather than a failure, because no sweep in this repository but one has ever
/// had its whole red set measured. Round 1139 measured that one: widening its
/// scope from a single crate to the workspace turned 29 reds into 41, and the
/// twelve new ones were invisible for as long as nothing asked.
///
/// BOTH ARMS, BECAUSE A CHECK THAT ONLY EVER REFUSES CANNOT BE TOLD FROM ONE
/// THAT ALWAYS REFUSES. The same suite and the same expectation run under the
/// two claims: one fails and names the red, the other passes and counts it.
#[test]
fn a_red_nobody_described_is_judged_only_where_the_manifest_claims_every_one() {
    let root = tempdir();
    // A suite whose broken state names TWO tests, against a manifest that names
    // one of them. That difference is the whole subject here.
    let two_red = "failures:\\n    the_law\\n    the_other_law\\n\\ntest result: \
                   FAILED. 0 passed; 2 failed; 0 ignored\\n";
    split_verdict_suite(
        root.path(),
        (ALL_GREEN, 0),
        (two_red, 1),
        "the wire is HEALTHY here\n",
    );
    let injections = serde_json::json!([{
        "name": "I1",
        "why": "one read, two guards, and the manifest knows about one",
        "edits": [{"file": "src.txt", "from": "HEALTHY", "to": "BROKEN"}],
        "expect_red": ["the_law"],
    }]);

    let claimed = harness(&manifest_claiming_every_red(
        root.path(),
        injections.clone(),
    ));
    let said = String::from_utf8_lossy(&claimed.stderr);
    assert!(
        !claimed.status.success(),
        "a manifest that claims every red and meets one it did not describe has \
         a decayed set, not a clean sweep: {said}"
    );
    assert!(
        said.contains("the_other_law") && said.contains("does not name"),
        "and it names WHICH red nobody described: {said}"
    );

    let counted = harness(&manifest(root.path(), injections));
    let reported = String::from_utf8_lossy(&counted.stderr);
    assert!(
        counted.status.success(),
        "the default claim is that `expect_red` is a floor, so the same run is a \
         clean sweep: {reported}"
    );
    assert!(
        reported.contains("2 red, 1 unnamed"),
        "and the number is reported either way, because under the weaker claim \
         it is what a next round would go and measure: {reported}"
    );
}

/// A key this reader does not know is refused rather than ignored.
///
/// EVERY OPTIONAL FIELD IS A KEY A MANIFEST MAY SIMPLY NOT CARRY, so a
/// misspelled one reads as absent: the manifest runs under the default while its
/// author reads their own spelling and believes the opposite. That is silent in
/// the direction that looks like a pass — `red-set: exhaustive` would leave the
/// sweep making the weaker claim while the file says it makes the stronger one.
#[test]
fn a_key_this_reader_does_not_know_is_refused_rather_than_ignored() {
    let root = tempdir();
    tree(root.path(), "the wire is HEALTHY here\n");
    let path = root.path().join("manifest.json");
    fs::write(
        &path,
        serde_json::to_string(&serde_json::json!({
            "repo": root.path(),
            "test_command": [root.path().join("suite.sh")],
            "logs": root.path().join("logs"),
            "red-set": "exhaustive",
            "injections": [],
        }))
        .expect("json"),
    )
    .expect("write manifest");

    let out = harness(&path);
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "a manifest carrying a key nobody reads is not a manifest: {said}"
    );
    assert!(
        said.contains("is not a manifest") && said.contains("red-set"),
        "and the refusal names the key, so the misspelling is the finding: {said}"
    );
    assert!(
        !root.path().join("logs").join("control.log").exists(),
        "refused before any suite ran"
    );
}

/// An exhaustive claim over an empty expectation is refused before the suites.
///
/// UNDER THE WEAKER CLAIM AN EMPTY `expect_red` IS HONEST — "say what went red
/// and judge nothing" is what an exploratory sweep does. Under this one it says
/// the injection reddens NOTHING, which is the single outcome a sweep exists to
/// detect rather than to declare, and the contradiction should not cost the
/// suites: the whole-workspace sweep this claim was built for is 85 minutes.
#[test]
fn an_exhaustive_manifest_that_names_no_red_is_refused_before_the_run() {
    let root = tempdir();
    tree(root.path(), "the wire is HEALTHY here\n");
    let path = manifest_claiming_every_red(
        root.path(),
        serde_json::json!([{
            "name": "I1",
            "why": "an injection nobody expects anything of",
            "edits": [{"file": "src.txt", "from": "HEALTHY", "to": "BROKEN"}],
        }]),
    );

    let out = harness(&path);
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "an exhaustive set of nothing asserts that breaking the read changes \
         nothing: {said}"
    );
    assert!(
        said.contains("exhaustive") && said.contains("I1"),
        "and the refusal names which injection: {said}"
    );
    assert!(
        !root.path().join("logs").join("control.log").exists(),
        "refused before any suite ran, which is the whole reason it is here and \
         not in the judging at the end"
    );

    // THE CONTROL: the same manifest under the default claim is a legitimate
    // exploratory sweep, and refusing it would be refusing for a reason outside
    // this law.
    let allowed = harness(&manifest(
        root.path(),
        serde_json::json!([{
            "name": "I1",
            "why": "an injection nobody expects anything of",
            "edits": [{"file": "src.txt", "from": "HEALTHY", "to": "BROKEN"}],
        }]),
    ));
    assert!(
        allowed.status.success(),
        "an empty expectation under the weaker claim says what went red and \
         judges nothing: {}",
        String::from_utf8_lossy(&allowed.stderr)
    );
}

#[test]
fn a_machine_with_no_room_is_refused_before_the_build() {
    let root = tempdir();
    tree(root.path(), "the wire is HEALTHY here\n");
    let path = manifest_with(
        root.path(),
        serde_json::json!([]),
        // A floor no machine meets, so the refusal is the thing under test
        // rather than the machine's mood.
        serde_json::json!(1_000_000_000u64),
    );
    let out = harness(&path);
    assert!(
        !out.status.success(),
        "the standing rule is to check occupancy before EVERY build, and eight \
         rounds running a person did it before some of them"
    );
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(
        said.contains("available and the manifest asks for"),
        "{said}"
    );
    assert!(
        !root.path().join("logs").join("control.log").exists(),
        "the suite was never asked"
    );
}

#[test]
fn a_floor_this_machine_clears_lets_the_run_through() {
    // The other half of the gate, and it needs its own test: a check that only
    // ever refuses is indistinguishable from a check that always refuses.
    let root = tempdir();
    tree(root.path(), "the wire is HEALTHY here\n");
    let path = manifest_with(root.path(), serde_json::json!([]), serde_json::json!(0u64));
    let out = Command::new(binary())
        .arg(&path)
        .arg("--control-only")
        .output()
        .expect("harness runs");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("MiB available (floor 0)"),
        "and it says what it read, so a floor nobody meets is told apart from a \
         reading nobody took"
    );
}

#[test]
fn a_control_that_counted_failures_it_cannot_name_says_which_half_is_empty() {
    // THE MESSAGE THAT COST A DIAGNOSIS. The count comes from `test result:`
    // and the names come from a `failures:` list, so they can disagree — a
    // doc-test's name carries spaces and no list names it, a target can die
    // before printing one — and this refusal used to print the number beside an
    // empty set, `3 red {}`, leaving a reader to run the whole suite again to
    // find out what had failed. It is now SAID.
    let root = tempdir();
    let counted_but_unnamed = "test result: FAILED. 1 passed; 3 failed; 0 ignored\\n";
    split_verdict_suite(
        root.path(),
        (counted_but_unnamed, 1),
        (counted_but_unnamed, 1),
        "HEALTHY\n",
    );
    let path = manifest(
        root.path(),
        serde_json::json!([{
            "name": "I1",
            "edits": [{"file": "src.txt", "from": "HEALTHY", "to": "BROKEN"}],
        }]),
    );
    let out = harness(&path);
    assert!(!out.status.success(), "a red control is not a baseline");
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(
        said.contains("the control is 3 red") && said.contains("names none of them"),
        "the refusal must say the count came from one line and no name from another: {said}"
    );
    assert!(
        !said.contains("{}"),
        "an empty set printed beside a number is the shape this replaced: {said}"
    );
}

#[test]
fn a_control_that_failed_without_naming_a_test_stops_the_sweep() {
    // THE SHAPE THIS IS ABOUT: a suite can fail in a way its own log does not
    // name — a target that did not build, a crash, a signal — and the failure
    // list stays empty. Read by the list alone that is a green control, and
    // every count taken after it is taken against a run that did not happen.
    let root = tempdir();
    split_verdict_suite(root.path(), (ALL_GREEN, 1), (ALL_GREEN, 1), "HEALTHY\n");
    let path = manifest(
        root.path(),
        serde_json::json!([{
            "name": "I1",
            "edits": [{"file": "src.txt", "from": "HEALTHY", "to": "BROKEN"}],
        }]),
    );
    let out = harness(&path);
    assert!(
        !out.status.success(),
        "a control that exited non-zero is not a green control"
    );
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(
        said.contains("exited 1") && said.contains("named no failing test"),
        "and it must say WHICH half disagreed with which: {said}"
    );
    assert!(
        !root.path().join("logs").join("I1.log").exists(),
        "no injection is run from a control that did not happen"
    );
}

#[test]
fn an_injection_run_that_failed_without_naming_a_test_is_not_a_clean_surface() {
    // The same disagreement one run later, where it is worse: 0 red against an
    // injection reads as "the surface holds", and the run it is read from never
    // finished. Round 1057 ran exactly this — 15 doc-tests failed to build under
    // one injection — and it was caught afterwards by counting targets, not by
    // the run itself.
    let root = tempdir();
    split_verdict_suite(root.path(), (ALL_GREEN, 0), (ALL_GREEN, 1), "HEALTHY\n");
    let path = manifest(
        root.path(),
        serde_json::json!([{
            "name": "I1",
            // No `expect_red`: the misaimed-injection gate must not be what
            // fails this sweep, or the test would pass without the law.
            "edits": [{"file": "src.txt", "from": "HEALTHY", "to": "BROKEN"}],
        }]),
    );
    let out = harness(&path);
    assert!(
        !out.status.success(),
        "0 red out of a run that failed to run is not 0 red"
    );
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(
        said.contains("I1") && said.contains("named no failing test"),
        "{said}"
    );
    assert_eq!(
        fs::read_to_string(root.path().join("src.txt")).expect("read back"),
        "HEALTHY\n",
        "and the tree still comes back — a refusal is not a reason to leave the \
         injection in place"
    );
}

#[test]
fn a_run_that_exited_green_over_a_red_log_stops_the_sweep() {
    // The other direction, and it needs its own test because the two halves can
    // disagree either way: a log that names failing tests under an exit code
    // that says everything passed means either the parse invented the names or
    // the suite swallowed the failure. Both make the red count fiction.
    let root = tempdir();
    split_verdict_suite(root.path(), (ALL_GREEN, 0), (ONE_RED, 0), "HEALTHY\n");
    let path = manifest(
        root.path(),
        serde_json::json!([{
            "name": "I1",
            "edits": [{"file": "src.txt", "from": "HEALTHY", "to": "BROKEN"}],
            "expect_red": ["the_law"],
        }]),
    );
    let out = harness(&path);
    assert!(
        !out.status.success(),
        "an exit code that says nothing failed cannot stand over a log that \
         names what did"
    );
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(
        said.contains("exited 0") && said.contains("the_law"),
        "{said}"
    );
}

#[test]
fn names_that_do_not_identify_the_targets_stop_the_sweep() {
    // A suite whose two targets announce the same name: the count says 2 and
    // the set says 1, so a run that lost one of them would read as no drift.
    let root = tempdir();
    fs::create_dir_all(root.path().join("logs")).expect("mkdir");
    fs::write(root.path().join("src.txt"), "HEALTHY\n").expect("write source");
    let suite = root.path().join("suite.sh");
    fs::write(
        &suite,
        "#!/bin/sh\n\
         printf '     Running unittests src/lib.rs (target/debug/deps/twin-1)\\n'\n\
         printf 'test result: ok. 1 passed; 0 failed\\n'\n\
         printf '     Running unittests src/lib.rs (target/debug/deps/twin-2)\\n'\n\
         printf 'test result: ok. 1 passed; 0 failed\\n'\n",
    )
    .expect("write suite");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&suite, fs::Permissions::from_mode(0o755)).expect("chmod");
    }
    let path = manifest(root.path(), serde_json::json!([]));
    let out = harness(&path);
    assert!(
        !out.status.success(),
        "a name that identifies nothing is not a name"
    );
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("share a name"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A suite that answers instantly while the tree is healthy and then, once an
/// injection has landed, opens a long window and records the pid of everything
/// it started.
///
/// The two halves are the point: the control must not take a minute, and the
/// injected run must still be running when the harness is killed — which is the
/// only state in which the tree is edited and a suite is live.
fn slow_once_injected(root: &Path, source: &str) -> PathBuf {
    fs::create_dir_all(root.join("logs")).expect("mkdir");
    fs::write(root.join("src.txt"), source).expect("write source");
    let suite = root.join("suite.sh");
    fs::write(
        &suite,
        "#!/bin/sh\n\
         if grep -q HEALTHY src.txt; then\n\
         printf 'test result: ok. 2 passed; 0 failed; 0 ignored\\n'\n\
         exit 0\n\
         fi\n\
         sh -c 'echo $$ > grandchild.pid; exec sleep 120' &\n\
         echo $$ > suite.pid\n\
         while [ ! -s grandchild.pid ]; do sleep 0.05; done\n\
         echo open > started\n\
         sleep 120\n",
    )
    .expect("write suite");
    make_runnable(&suite);
    suite
}

/// THE liveness budget for this file's waits — one decision, in one place.
///
/// Its whole job is to turn "this never happens" from a hang into a failure.
/// No test's green may depend on its value: these waits end when the condition
/// holds, and on a slower machine they simply poll more times. R1081 named it
/// rather than leaving `30` at the site, because a budget spelled where it is
/// used is a claim about the runner that nobody reviews as one.
const LIVENESS: std::time::Duration = std::time::Duration::from_secs(30);

fn until(what: &str, mut condition: impl FnMut() -> bool) {
    let deadline = std::time::Instant::now() + LIVENESS;
    while std::time::Instant::now() < deadline {
        if condition() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("timed out waiting for {what}");
}

fn alive(pid: i32) -> bool {
    // Signal 0 asks the kernel whether the process exists without touching it.
    unsafe { libc::kill(pid, 0) == 0 }
}

fn pid_in(path: &Path) -> i32 {
    fs::read_to_string(path)
        .expect("the suite wrote its pid")
        .trim()
        .parse()
        .expect("a pid")
}

fn signal(pid: u32, signal: i32) {
    assert_eq!(
        unsafe { libc::kill(pid as i32, signal) },
        0,
        "the harness is still there to be signalled"
    );
}

/// Start a sweep, wait until it is mid-injection with the suite live, and hand
/// back the harness, the tree's injected state, and the pids under it.
fn sweep_in_flight(root: &Path) -> (std::process::Child, i32, i32) {
    slow_once_injected(root, "the wire is HEALTHY here\n");
    let path = manifest(
        root,
        serde_json::json!([{
            "name": "I1",
            "edits": [{"file": "src.txt", "from": "HEALTHY", "to": "BROKEN"}],
        }]),
    );
    let harness = Command::new(binary())
        .arg(&path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("harness starts");
    until("the injected run to open its window", || {
        root.join("started").exists()
    });
    assert!(
        fs::read_to_string(root.join("src.txt"))
            .expect("read")
            .contains("BROKEN"),
        "the window under test is the one where the tree IS edited"
    );
    (
        harness,
        pid_in(&root.join("suite.pid")),
        pid_in(&root.join("grandchild.pid")),
    )
}

#[test]
fn a_sweep_that_is_interrupted_puts_the_tree_back_and_takes_the_suite_with_it() {
    let root = tempdir();
    let (mut harness, suite, grandchild) = sweep_in_flight(root.path());
    signal(harness.id(), libc::SIGTERM);
    // THE DEATHS ARE ASKED FOR BEFORE THE HARNESS IS WAITED ON, and that order
    // is the whole oracle. Waiting first lets a sweep that killed nothing pass:
    // it returns when the suite runs out of sleep, by which time the suite is
    // gone of old age and every assertion below holds. The self-check found this
    // by removing the process groups and watching this test stay green.
    until("the suite to die with the sweep", || !alive(suite));
    until("the test binary under it to die too", || !alive(grandchild));
    let status = harness.wait().expect("the harness exits");
    assert!(!status.success(), "a stopped sweep measured nothing");
    assert_eq!(
        fs::read_to_string(root.path().join("src.txt")).expect("read back"),
        "the wire is HEALTHY here\n",
        "THE TREE COMES BACK even though nobody asked the sweep to finish — an \
         injection left in the tree is the next measurement's baseline"
    );
}

#[test]
fn a_sweep_that_is_killed_outright_is_still_survived_by_its_tree() {
    // SIGKILL cannot be caught, so this is the case where nothing the harness
    // could have installed runs. What restores the tree is the supervisor, which
    // asked in advance to be signalled when its parent died.
    let root = tempdir();
    let (mut harness, suite, grandchild) = sweep_in_flight(root.path());
    signal(harness.id(), libc::SIGKILL);
    harness.wait().expect("the harness dies");
    until("the suite to die with the sweep", || !alive(suite));
    until("the test binary under it to die too", || !alive(grandchild));
    until("the supervisor to put the tree back", || {
        fs::read_to_string(root.path().join("src.txt")).unwrap_or_default()
            == "the wire is HEALTHY here\n"
    });

    // And the originals it never got to clear are what tells the next sweep that
    // this one died — with the tree's own answer about whether it was left
    // injected.
    assert!(
        root.path().join("logs").join("sweep").exists(),
        "a sweep that died leaves the evidence that it did"
    );
    let again = harness_run(root.path());
    assert!(
        again.status.success(),
        "the tree matches every original, so the next sweep may proceed: {}",
        String::from_utf8_lossy(&again.stderr)
    );
    assert!(
        String::from_utf8_lossy(&again.stderr).contains("a previous sweep left"),
        "and it says so rather than silently clearing them: {}",
        String::from_utf8_lossy(&again.stderr)
    );
}

#[test]
fn a_tree_left_injected_by_a_dead_sweep_stops_the_next_one() {
    // The loud half of that gate: the originals are there AND the file no longer
    // holds them, which is a tree carrying somebody else's injection. A sweep
    // started here would take that injection as its baseline and report every
    // law it breaks as a law that holds.
    let root = tempdir();
    tree(root.path(), "the wire is BROKEN by a sweep that died\n");
    left_originals(
        root.path(),
        "the wire is HEALTHY here\n",
        a_pid_that_is_gone(),
    );
    let path = manifest(root.path(), serde_json::json!([]));
    let out = harness(&path);
    assert!(
        !out.status.success(),
        "a borrowed injection is not a baseline"
    );
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(
        said.contains("a sweep died holding this tree") && said.contains("src.txt"),
        "and it names the file that is still injected: {said}"
    );
}

#[test]
fn a_manifest_that_names_its_paths_relatively_is_still_run_from_one_place() {
    // THE SHAPE THE TRACKED MANIFESTS USE, and the one every other test here
    // does not: `example.json` says `"repo": "../.."` and
    // `"logs": "target/injection-logs"`. The suite is started with the TREE as
    // its working directory, so a relative path handed across that change
    // resolves somewhere else — the first real sweep under a supervisor died
    // with `No such file or directory` because the supervisor's own path was
    // relative to where the sweep was started, not to where the suite runs.
    // THE SWEEP IS STARTED SOMEWHERE ELSE THAN THE TREE, which is the half that
    // makes the paths bite: this crate's own sweeps run from `tools/…` over a
    // repository two directories up. A fixture whose tree and whose starting
    // directory are the same one resolves every relative path by accident — the
    // first version of this test did exactly that and stayed green when the
    // absolutising was removed.
    let root = tempdir();
    tree(root.path(), "the wire is HEALTHY here\n");
    let from = root.path().join("tool");
    fs::create_dir_all(&from).expect("mkdir");
    let path = from.join("manifest.json");
    fs::write(
        &path,
        serde_json::json!({
            "repo": "..",
            "test_command": ["./suite.sh"],
            "logs": "logs",
            "injections": [{
                "name": "I1",
                "edits": [{"file": "src.txt", "from": "HEALTHY", "to": "BROKEN"}],
                "expect_red": ["the_law"],
            }],
        })
        .to_string(),
    )
    .expect("write manifest");
    // AND IT IS RUN FROM SOMEWHERE THAT IS NEITHER THE TREE NOR THE MANIFEST'S
    // OWN DIRECTORY. R1108 made the base the manifest's directory rather than
    // the caller's working directory, because a second reader — the law over
    // every sweep this repository tracks — cannot know where each was meant to
    // be run from, and that answer was in prose. A fixture started in the
    // manifest's own directory resolves both rules to the same path and cannot
    // tell them apart: this test did exactly that, and stayed green when the
    // base was changed under it.
    let elsewhere = root.path().join("elsewhere");
    fs::create_dir_all(&elsewhere).expect("mkdir");
    let out = Command::new(binary())
        .arg("../tool/manifest.json")
        .current_dir(&elsewhere)
        .output()
        .expect("harness runs");
    assert!(
        out.status.success(),
        "a relative manifest is the tracked shape, and its paths are its own: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("\"the_law\""),
        "and the injection still fires under it"
    );
    assert!(
        from.join("logs").is_dir(),
        "the logs landed beside the manifest, which is what it named — not \
         beside whoever ran it"
    );
}

#[test]
fn a_suite_that_replaces_the_tool_does_not_end_the_sweep() {
    // A sweep may be aimed at the tree that BUILDS it — this crate's own
    // `self-check.json` is exactly that — and the suite then replaces the binary
    // the sweep is executing. `/proc/self/exe` afterwards names a path that no
    // longer exists, and the first self-check ever run died on its SECOND
    // injection with `No such file or directory`. The sweep's own copy is what
    // makes the supervisor the code that started the sweep.
    let root = tempdir();
    fs::create_dir_all(root.path().join("logs")).expect("mkdir");
    fs::write(root.path().join("src.txt"), "one HEALTHY two SOUND\n").expect("write source");
    let tool = root.path().join("harness-copy");
    fs::copy(binary(), &tool).expect("copy the tool");
    make_runnable(&tool);
    let suite = root.path().join("suite.sh");
    fs::write(
        &suite,
        "#!/bin/sh\n\
         rm -f harness-copy\n\
         printf 'test result: ok. 1 passed; 0 failed; 0 ignored\\n'\n",
    )
    .expect("write suite");
    make_runnable(&suite);
    let path = manifest(
        root.path(),
        serde_json::json!([
            {"name": "I1", "edits": [{"file": "src.txt", "from": "HEALTHY", "to": "BROKEN"}]},
            {"name": "I2", "edits": [{"file": "src.txt", "from": "SOUND", "to": "BROKEN"}]},
        ]),
    );
    let out = run_a_freshly_written_program(&tool, &path);
    assert!(
        !tool.exists(),
        "the suite really did replace the binary the sweep was started from"
    );
    assert!(
        out.status.success(),
        "and the sweep still finished: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        root.path().join("logs").join("I2.log").exists(),
        "including the run that came AFTER the tool was gone"
    );
}

#[test]
fn a_sweep_already_running_in_this_tree_stops_a_second_one() {
    // The other reason an originals index is lying there, and it is the opposite
    // reason: not a sweep that died, a sweep that is HERE. Two sweeps in one
    // tree edit the same files and write the same logs, so each reads the
    // other's injection as its own baseline — which is the one thing this tool
    // exists to make impossible.
    let root = tempdir();
    tree(root.path(), "the wire is HEALTHY here\n");
    // The test process itself is the owner nobody can claim is gone.
    left_originals(
        root.path(),
        "the wire is HEALTHY here\n",
        std::process::id() as i32,
    );
    let path = manifest(root.path(), serde_json::json!([]));
    let out = harness(&path);
    assert!(!out.status.success(), "one tree, one sweep");
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(
        said.contains("already running in this tree") && said.contains("pid"),
        "and it names who is holding it: {said}"
    );
}

/// Leave behind what a sweep leaves when it does not finish: the originals, and
/// the pid of whoever is answerable for them.
fn left_originals(root: &Path, was: &str, owner: i32) {
    let originals = root.join("logs").join("sweep");
    fs::create_dir_all(&originals).expect("mkdir");
    fs::write(originals.join("000-src.txt"), was).expect("backup");
    fs::write(
        originals.join("originals.json"),
        serde_json::json!({
            "owner": owner,
            "files": [{
                "repo_file": root.join("src.txt"),
                "backup": originals.join("000-src.txt"),
            }]
        })
        .to_string(),
    )
    .expect("index");
}

/// A pid that certainly belonged to a process and certainly does not now.
fn a_pid_that_is_gone() -> i32 {
    let mut gone = Command::new("/bin/true").spawn().expect("spawn /bin/true");
    let pid = gone.id() as i32;
    gone.wait().expect("reap");
    pid
}

#[test]
fn a_suite_killed_by_a_signal_is_not_a_run_that_finished() {
    // The supervisor forwards the death rather than translating it: a suite that
    // was killed must not arrive as an exit code, because every exit code is a
    // run that got to the end.
    let root = tempdir();
    fs::create_dir_all(root.path().join("logs")).expect("mkdir");
    fs::write(root.path().join("src.txt"), "HEALTHY\n").expect("write source");
    let suite = root.path().join("suite.sh");
    fs::write(
        &suite,
        "#!/bin/sh\n\
         if grep -q HEALTHY src.txt; then\n\
         printf 'test result: ok. 2 passed; 0 failed; 0 ignored\\n'\n\
         exit 0\n\
         fi\n\
         kill -KILL $$\n",
    )
    .expect("write suite");
    make_runnable(&suite);
    let path = manifest(
        root.path(),
        serde_json::json!([{
            "name": "I1",
            "edits": [{"file": "src.txt", "from": "HEALTHY", "to": "BROKEN"}],
        }]),
    );
    let out = harness(&path);
    assert!(!out.status.success());
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(
        said.contains("killed by a signal"),
        "the run that was killed must be reported as one: {said}"
    );
}

fn harness_run(root: &Path) -> std::process::Output {
    let path = manifest(
        root,
        serde_json::json!([{
            "name": "I1",
            "edits": [{"file": "src.txt", "from": "HEALTHY", "to": "BROKEN"}],
        }]),
    );
    Command::new(binary())
        .arg(&path)
        .arg("--control-only")
        .output()
        .expect("harness runs")
}

/// A temp directory that removes itself, without taking a dependency for it.
struct TempDir(PathBuf);

impl TempDir {
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn tempdir() -> TempDir {
    let base = std::env::temp_dir().join(format!(
        "injection-harness-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base).expect("tempdir");
    TempDir(base)
}
