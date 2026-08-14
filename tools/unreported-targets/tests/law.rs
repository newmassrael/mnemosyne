//! The law, asked of real cargo runs this test performs — one per answer the
//! gate can give.
//!
//! REAL RUNS RATHER THAN HAND-WRITTEN LOGS, and that is the point of this file
//! against the unit tests next to the code. What this gate claims is a fact
//! about cargo: that a run which stops at the first failing target prints no
//! `Running` line for the ones behind it, and that the artifacts cargo reports
//! for `--no-run` are exactly the executables a full run reaches. A fixture log
//! I wrote myself would prove that this gate reads MY sentence about cargo. So
//! every case here builds a workspace, runs cargo in it, and judges what cargo
//! actually printed.
//!
//! Built OUTSIDE this repository, for the reason its siblings give: a fixture
//! carrying its own `[workspace]` inside the tree would be discovered by
//! `scripts/check-side-workspaces.sh`, which would then lint a workspace whose
//! whole purpose is to be rejected.

use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

use unreported_targets::{Answer, Outcome, Report};

/// A one-package workspace with four test targets and a plain binary.
///
/// THE PLAIN BINARY IS PART OF THE FIXTURE, not decoration. `cargo test` emits
/// two executables for a `[[bin]]` — the binary itself, which no test run ever
/// executes, and its test harness, which every run does. A gate that took every
/// executable cargo reports would call the first one a target that never ran, on
/// every clean tree, forever.
fn fixture(alpha: &str) -> TempDir {
    let at = TempDir::new().expect("tempdir");
    let root = at.path();
    std::fs::create_dir_all(root.join("src")).expect("mkdir src");
    std::fs::create_dir_all(root.join("tests")).expect("mkdir tests");
    std::fs::write(
        root.join("Cargo.toml"),
        "[workspace]\n[package]\nname = \"probe\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\
         publish = false\n",
    )
    .expect("write manifest");
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn two() -> u32 {\n    2\n}\n\n\
         #[cfg(test)]\nmod tests {\n    #[test]\n    fn the_unit_target_runs() {\n        \
         assert_eq!(super::two(), 2);\n    }\n}\n",
    )
    .expect("write lib");
    std::fs::write(
        root.join("src/main.rs"),
        "fn main() {\n    println!(\"{}\", probe::two());\n}\n",
    )
    .expect("write bin");
    std::fs::write(root.join("tests/alpha.rs"), alpha).expect("write alpha");
    std::fs::write(
        root.join("tests/beta.rs"),
        "#[test]\nfn beta_would_pass_if_it_were_asked() {\n    assert_eq!(probe::two(), 2);\n}\n",
    )
    .expect("write beta");
    at
}

/// The target that fails first, and takes the run down with it.
const RED: &str = "#[test]\nfn alpha_is_red() {\n    assert_eq!(probe::two(), 3);\n}\n";
/// The same target, passing.
const GREEN: &str = "#[test]\nfn alpha_is_green() {\n    assert_eq!(probe::two(), 2);\n}\n";

/// The words this test runs, kept once so the log and the judgement cannot
/// drift: the gate is handed the very array that was executed.
fn command(rest: &[&str]) -> Vec<String> {
    let mut words = vec![env!("CARGO").to_string()];
    words.extend(rest.iter().map(|word| (*word).to_string()));
    words
}

/// Run a command in the fixture and keep everything it printed, the way a tee'd
/// verification log does.
///
/// EVERY VARIABLE THE SPAWNED CARGO READS IS SET HERE. `CARGO_TARGET_DIR` keeps
/// the build inside the fixture, so this test never touches the build directory
/// the whole repository shares; `CARGO_TERM_COLOR` is the one input that decides
/// whether the lines this gate parses arrive wrapped in escape sequences, so it
/// is an argument rather than whatever the machine happens to be set to.
fn run_in(at: &Path, words: &[String], colour: &str) -> PathBuf {
    let output = Command::new(&words[0])
        .args(&words[1..])
        .current_dir(at)
        .env("CARGO_TARGET_DIR", at.join("t"))
        .env("CARGO_TERM_COLOR", colour)
        .output()
        .unwrap_or_else(|error| panic!("cannot run {}: {error}", words.join(" ")));
    let log = at.join("run.log");
    let mut text = String::from_utf8_lossy(&output.stderr).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stdout));
    std::fs::write(&log, text).expect("write log");
    log
}

fn judge(at: &Path, log: &Path, words: &[String]) -> Report {
    unreported_targets::run(log, words, at).expect("the gate judges the run")
}

fn judged(report: &Report) -> &unreported_targets::Judged {
    match &report.outcome {
        Outcome::Judged(judged) => judged,
        other => panic!("expected a judged run, got {other:?}"),
    }
}

/// THE SHAPE THIS GATE EXISTS FOR. `cargo test` stops at the first failing
/// target; `tests/beta.rs` is never asked, and the summary a reader sees says
/// one target failed.
#[test]
fn a_target_that_hid_behind_a_first_targets_red_is_named() {
    let at = fixture(RED);
    let words = command(&["test"]);
    let log = run_in(at.path(), &words, "never");

    let text = std::fs::read_to_string(&log).expect("read log");
    assert!(
        !text.contains("tests/beta.rs"),
        "the fixture is only a fixture if cargo really did stop early:\n{text}"
    );

    let report = judge(at.path(), &log, &words);
    assert_eq!(report.answer(), Answer::Finding, "{report:#?}");
    let judged = judged(&report);
    assert_eq!(judged.compiled.len(), 4, "{judged:#?}");
    // THREE OF THE FOUR RAN, and the count is written out because getting it
    // wrong is how this case would stop being about anything: the lib's unit
    // target and the BIN's test harness both run before `tests/alpha.rs` does,
    // and cargo stops after alpha's failure. Doc-tests are never reached at all
    // (`doc_tests` is 0 here and 1 in the run that carries on), which is a
    // second thing a first-failure run silently drops.
    assert_eq!(judged.reported.files.len(), 3, "{judged:#?}");
    assert_eq!(judged.reported.doc_tests, 0, "{judged:#?}");
    let names: Vec<&str> = judged
        .unreported
        .iter()
        .map(|target| target.name.as_str())
        .collect();
    assert_eq!(names, vec!["beta"], "{judged:#?}");
    assert!(judged.unexplained.is_empty(), "{judged:#?}");
}

/// The control, without which the case above proves only that this gate can say
/// "finding", which every broken gate also does. Same tree, same red target —
/// only the flag that makes cargo carry on differs.
#[test]
fn the_same_red_tree_is_clean_once_the_run_covers_every_target() {
    let at = fixture(RED);
    let words = command(&["test", "--no-fail-fast"]);
    let log = run_in(at.path(), &words, "never");

    let report = judge(at.path(), &log, &words);
    assert_eq!(report.answer(), Answer::Clean, "{report:#?}");
    let judged = judged(&report);
    assert_eq!(judged.compiled.len(), 4, "{judged:#?}");
    assert_eq!(judged.reported.files.len(), 4, "{judged:#?}");
    assert_eq!(judged.reported.doc_tests, 1, "{judged:#?}");
    assert!(judged.unreported.is_empty(), "{judged:#?}");
}

/// A wholly green run is clean too — the case that would pass even if this gate
/// only ever looked at the exit status, and is here so the pair above cannot be
/// read as "red means finding".
#[test]
fn a_green_run_covers_its_targets() {
    let at = fixture(GREEN);
    let words = command(&["test"]);
    let log = run_in(at.path(), &words, "never");

    let report = judge(at.path(), &log, &words);
    assert_eq!(report.answer(), Answer::Clean, "{report:#?}");
    assert_eq!(judged(&report).reported.files.len(), 4);
}

/// COLOUR, END TO END. The unit test next to the code proves the stripper reads
/// a sequence I typed; this one proves cargo's own colouring, on this machine's
/// cargo, does not hide a target that ran.
#[test]
fn a_coloured_run_is_still_read() {
    let at = fixture(GREEN);
    let words = command(&["test"]);
    let log = run_in(at.path(), &words, "always");

    let text = std::fs::read_to_string(&log).expect("read log");
    assert!(
        text.contains('\u{1b}'),
        "this case is only a case if cargo really coloured its output:\n{text}"
    );
    let report = judge(at.path(), &log, &words);
    assert_eq!(report.answer(), Answer::Clean, "{report:#?}");
    assert_eq!(judged(&report).reported.files.len(), 4);
}

/// A log and a command that are not about the same run is a REFUSAL. Here the
/// log is a whole run's and the command names one target, so the log reports
/// three executables that command does not build.
#[test]
fn a_log_from_a_wider_command_is_refused_rather_than_reported() {
    let at = fixture(GREEN);
    let whole = command(&["test"]);
    let log = run_in(at.path(), &whole, "never");

    let narrow = command(&["test", "--test", "alpha"]);
    let report = judge(at.path(), &log, &narrow);
    assert_eq!(report.answer(), Answer::CouldNotJudge, "{report:#?}");
    let judged = judged(&report);
    assert_eq!(judged.compiled.len(), 1, "{judged:#?}");
    assert_eq!(judged.unexplained.len(), 3, "{judged:#?}");
}

/// A QUIET RUN IS REFUSED, AND THE DANGER IS ASSERTED RATHER THAN DESCRIBED:
/// the log of a fully successful `-q` run holds no `Running` line at all, so a
/// gate that read it as evidence would report every target as never having run.
#[test]
fn a_quiet_run_is_refused_because_its_log_says_nothing_about_coverage() {
    let at = fixture(GREEN);
    let words = command(&["test", "-q"]);
    let log = run_in(at.path(), &words, "never");

    let text = std::fs::read_to_string(&log).expect("read log");
    assert!(
        !text.contains("Running "),
        "this case rests on a quiet run printing no Running line:\n{text}"
    );

    let report = judge(at.path(), &log, &words);
    assert_eq!(report.answer(), Answer::CouldNotJudge, "{report:#?}");
    assert!(
        matches!(&report.outcome, Outcome::Unreadable(reason) if reason.contains("quiet")),
        "{report:#?}"
    );
}

/// A command that is not a test run carries no coverage question, and says so.
/// Its log is a real one — this runs the build it judges.
#[test]
fn a_command_that_runs_no_test_target_is_clean_with_its_reason() {
    let at = fixture(GREEN);
    let words = command(&["build"]);
    let log = run_in(at.path(), &words, "never");

    let report = judge(at.path(), &log, &words);
    assert_eq!(report.answer(), Answer::Clean, "{report:#?}");
    assert!(
        matches!(&report.outcome, Outcome::Vacuous(reason) if reason.contains("cargo build")),
        "{report:#?}"
    );
}

/// A TARGET THAT DOES NOT COMPILE LEAVES NOTHING TO ASK. The population comes
/// from cargo, and cargo cannot answer for a tree it cannot build — so this is
/// an error the caller turns into "no verdict", never a tree full of targets
/// that never ran.
#[test]
fn a_tree_that_does_not_compile_is_an_error_rather_than_a_finding() {
    let at = fixture("#[test]\nfn alpha_does_not_compile() {\n    let x: u32 = \"no\";\n}\n");
    let words = command(&["test"]);
    let log = run_in(at.path(), &words, "never");

    let error = unreported_targets::run(&log, &words, at.path())
        .expect_err("a tree that does not compile has no population");
    assert!(
        error.contains("--no-run"),
        "the error names the question that could not be asked: {error}"
    );
}
