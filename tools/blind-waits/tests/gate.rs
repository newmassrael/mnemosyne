//! The gate run against real cargo workspaces, built for each test and thrown
//! away with it.
//!
//! These are the assertions that say the law is not vacuous: each one puts a
//! wait of a known shape into a real workspace, hands the real binary the real
//! manifest, and reads what came back. Both directions every time — the shape
//! the law rejects AND the shape it must not, because a gate that rejects
//! everything and a gate that rejects the right thing pass the same one-sided
//! test.
//!
//! Nothing here is compiled by cargo; the gate parses. That is what makes it
//! affordable to run the fixtures as whole workspaces rather than as strings.
//!
//! The fixtures live in a temporary directory OUTSIDE this repository. A
//! fixture crate inside the tree would be found by
//! `scripts/check-side-workspaces.sh`, which discovers every `[workspace]`
//! there is and would then try to lint one whose purpose is to be rejected.

use std::path::Path;
use std::process::Command;

use tempfile::TempDir;

struct Run {
    code: i32,
    stdout: String,
    stderr: String,
}

impl Run {
    fn says(&self, needle: &str) -> bool {
        self.stdout.contains(needle) || self.stderr.contains(needle)
    }

    /// Every line the gate rejected something on, so a test can count them.
    fn defects(&self) -> Vec<&str> {
        self.stdout
            .lines()
            .filter(|line| line.contains("] DEFECT "))
            .collect()
    }
}

fn fixture(files: &[(&str, &str)]) -> TempDir {
    let dir = tempfile::tempdir().expect("a temporary directory");
    for (path, contents) in files {
        let full = dir.path().join(path);
        std::fs::create_dir_all(full.parent().expect("a parent")).expect("mkdir");
        std::fs::write(&full, contents).expect("write");
    }
    dir
}

fn gate(workspace: &Path) -> Run {
    let output = Command::new(env!("CARGO_BIN_EXE_blind-waits"))
        .args([
            "--workspace",
            &workspace.join("Cargo.toml").display().to_string(),
        ])
        .env("CARGO_TARGET_DIR", workspace.join("target"))
        .output()
        .expect("the gate binary runs");
    Run {
        code: output
            .status
            .code()
            .expect("the gate exits rather than signals"),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

const MANIFEST: &str = r#"
[workspace]
members = ["waiter"]
"#;

const PACKAGE: &str = r#"
[package]
name = "waiter"
version = "0.0.0"
edition = "2021"
publish = false
"#;

/// A library with nothing in it, so a fixture only has to write the file that
/// carries the shape under test.
const EMPTY_LIB: &str = "pub fn ship() {}\n";

// --- clause one: a wait must end on a condition -----------------------------

#[test]
fn a_sleep_no_loop_re_checks_anything_around_is_a_defect() {
    let dir = fixture(&[
        ("Cargo.toml", MANIFEST),
        ("waiter/Cargo.toml", PACKAGE),
        ("waiter/src/lib.rs", EMPTY_LIB),
        (
            "waiter/tests/settle.rs",
            r#"
use std::time::Duration;

#[test]
fn the_background_thread_got_there() {
    std::thread::sleep(Duration::from_millis(300));
    assert!(true);
}
"#,
        ),
    ]);

    let run = gate(dir.path());
    assert_eq!(run.code, 1, "a blind wait is a defect\n{}", run.stdout);
    assert_eq!(run.defects().len(), 1, "exactly one\n{}", run.stdout);
    assert!(
        run.says("blind wait `std::thread::sleep`"),
        "the gate names the callee it rejected\n{}",
        run.stdout
    );
    assert!(
        run.says("tests/settle.rs:6:5"),
        "and where it is, so it can be opened\n{}",
        run.stdout
    );
}

#[test]
fn the_same_sleep_as_the_retry_step_of_a_poll_is_the_shape_the_law_wants() {
    // The point of the pair: only the SHAPE differs. Same callee, same
    // duration, same file. One is a claim about the machine and the other is
    // not, and a gate that cannot tell them apart is a gate that bans waiting.
    let dir = fixture(&[
        ("Cargo.toml", MANIFEST),
        ("waiter/Cargo.toml", PACKAGE),
        ("waiter/src/lib.rs", EMPTY_LIB),
        (
            "waiter/tests/settle.rs",
            r#"
use std::time::Duration;
use std::time::Instant;

const LIVENESS: Duration = Duration::from_secs(30);

#[test]
fn the_background_thread_got_there() {
    let deadline = Instant::now() + LIVENESS;
    loop {
        if done() {
            break;
        }
        assert!(Instant::now() < deadline, "never happened");
        std::thread::sleep(Duration::from_millis(300));
    }
}

fn done() -> bool {
    true
}
"#,
        ),
    ]);

    let run = gate(dir.path());
    assert_eq!(
        run.code, 0,
        "a poll bounded by a named budget passes\n{}{}",
        run.stdout, run.stderr
    );
    assert!(
        run.says("wait sites, none of them blind or spelled"),
        "and the gate says how many it looked at\n{}",
        run.stdout
    );
}

#[test]
fn a_loop_that_can_only_run_out_of_iterations_does_not_make_a_wait_a_poll() {
    // "Inside a loop" is not the law — ENDING ON A CONDITION is. This body has
    // no `break`, no `return` and no `?`, so it is a blind three hundred
    // milliseconds with a `for` in front of it, and a gate that took loop depth
    // as the test would call it a poll.
    let dir = fixture(&[
        ("Cargo.toml", MANIFEST),
        ("waiter/Cargo.toml", PACKAGE),
        ("waiter/src/lib.rs", EMPTY_LIB),
        (
            "waiter/tests/settle.rs",
            r#"
use std::time::Duration;

#[test]
fn the_background_thread_got_there() {
    for _ in 0..3 {
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(true);
}
"#,
        ),
    ]);

    let run = gate(dir.path());
    assert_eq!(
        run.code, 1,
        "a counted loop is not a condition\n{}",
        run.stdout
    );
    assert!(run.says("blind wait"), "{}", run.stdout);
}

#[test]
fn a_while_head_is_a_condition_so_the_wait_inside_it_ends_on_one() {
    // The counterpart: the same `for` becomes lawful the moment the loop can
    // stop for a reason. Both forms below are what the repaired tests use.
    let dir = fixture(&[
        ("Cargo.toml", MANIFEST),
        ("waiter/Cargo.toml", PACKAGE),
        ("waiter/src/lib.rs", EMPTY_LIB),
        (
            "waiter/tests/settle.rs",
            r#"
use std::time::Duration;

fn done() -> bool {
    true
}

#[test]
fn a_while_head_counts() {
    while !done() {
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn a_break_counts() {
    for _ in 0..3 {
        if done() {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
"#,
        ),
    ]);

    let run = gate(dir.path());
    assert_eq!(run.code, 0, "{}{}", run.stdout, run.stderr);
    assert_eq!(run.defects().len(), 0, "{}", run.stdout);
}

#[test]
fn a_straight_line_yield_is_a_claim_about_the_scheduler_too() {
    // `yield_now()` written straight-line in a test says "the other task got
    // somewhere by now". Nothing checks that it did.
    let dir = fixture(&[
        ("Cargo.toml", MANIFEST),
        ("waiter/Cargo.toml", PACKAGE),
        ("waiter/src/lib.rs", EMPTY_LIB),
        (
            "waiter/tests/settle.rs",
            r#"
#[test]
fn the_other_task_got_there() {
    let _ = tokio::task::yield_now();
    assert!(true);
}
"#,
        ),
    ]);

    let run = gate(dir.path());
    assert_eq!(run.code, 1, "{}", run.stdout);
    assert!(
        run.says("blind wait `tokio::task::yield_now`"),
        "{}",
        run.stdout
    );
}

#[test]
fn a_sleep_in_shipped_code_is_behaviour_and_not_this_law() {
    // Waiting is a real thing for a program to do. The law is about what a
    // TEST claims, so the identical call outside test code must pass — a gate
    // that flags a retry backoff in a library is one nobody can adopt.
    let dir = fixture(&[
        ("Cargo.toml", MANIFEST),
        ("waiter/Cargo.toml", PACKAGE),
        (
            "waiter/src/lib.rs",
            r#"
use std::time::Duration;

pub fn back_off() {
    std::thread::sleep(Duration::from_millis(300));
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_backs_off() {
        super::back_off();
    }
}
"#,
        ),
    ]);

    let run = gate(dir.path());
    assert_eq!(
        run.code, 0,
        "shipped code may sleep\n{}{}",
        run.stdout, run.stderr
    );
    assert_eq!(run.defects().len(), 0, "{}", run.stdout);
}

#[test]
fn a_sleep_inside_a_cfg_test_module_of_a_library_is_test_code() {
    // THE ONE A DIRECTORY LISTING MISSES. This repository's own defect list was
    // assembled by looking at `tests/` and it was short by exactly this shape:
    // `crates/mnemosyne-server/src/audit.rs` carries a wait budget inside a
    // `#[cfg(test)]` module, in a file whose path says `src`.
    let dir = fixture(&[
        ("Cargo.toml", MANIFEST),
        ("waiter/Cargo.toml", PACKAGE),
        (
            "waiter/src/lib.rs",
            r#"
pub fn ship() {}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    #[test]
    fn the_other_thread_got_there() {
        std::thread::sleep(Duration::from_millis(300));
    }
}
"#,
        ),
    ]);

    let run = gate(dir.path());
    assert_eq!(
        run.code, 1,
        "a `#[cfg(test)]` module is test code\n{}",
        run.stdout
    );
    assert!(
        run.says("src/lib.rs:10:9 blind wait"),
        "reported at its own line inside src/\n{}",
        run.stdout
    );
}

#[test]
fn a_sleep_reached_only_through_a_helper_is_still_found_at_the_helper() {
    // A wait hidden one call deep is the same wait. The gate does not chase
    // calls — it does not have to, because the helper's own body is where the
    // straight-line sleep is written.
    let dir = fixture(&[
        ("Cargo.toml", MANIFEST),
        ("waiter/Cargo.toml", PACKAGE),
        ("waiter/src/lib.rs", EMPTY_LIB),
        (
            "waiter/tests/settle.rs",
            r#"
use std::time::Duration;

fn settle() {
    std::thread::sleep(Duration::from_millis(50));
}

#[test]
fn the_background_thread_got_there() {
    settle();
}
"#,
        ),
    ]);

    let run = gate(dir.path());
    assert_eq!(run.code, 1, "{}", run.stdout);
    assert!(run.says("tests/settle.rs:5:5 blind wait"), "{}", run.stdout);
}

// --- clause two: the budget that bounds a wait must be named ----------------

#[test]
fn a_budget_spelled_at_the_wait_site_is_a_defect() {
    let dir = fixture(&[
        ("Cargo.toml", MANIFEST),
        ("waiter/Cargo.toml", PACKAGE),
        ("waiter/src/lib.rs", EMPTY_LIB),
        (
            "waiter/tests/tail.rs",
            r#"
use std::time::Duration;

async fn arrive() {}

#[test]
fn the_push_arrives() {
    let _ = tokio::time::timeout(Duration::from_millis(500), arrive());
}
"#,
        ),
    ]);

    let run = gate(dir.path());
    assert_eq!(run.code, 1, "{}", run.stdout);
    assert!(
        run.says("spelled budget `tokio::time::timeout`"),
        "{}",
        run.stdout
    );
}

#[test]
fn the_same_wait_with_a_named_budget_passes() {
    let dir = fixture(&[
        ("Cargo.toml", MANIFEST),
        ("waiter/Cargo.toml", PACKAGE),
        ("waiter/src/lib.rs", EMPTY_LIB),
        (
            "waiter/tests/tail.rs",
            r#"
use std::time::Duration;

const LIVENESS: Duration = Duration::from_secs(30);

async fn arrive() {}

#[test]
fn the_push_arrives() {
    let _ = tokio::time::timeout(LIVENESS, arrive());
}
"#,
        ),
    ]);

    let run = gate(dir.path());
    assert_eq!(run.code, 0, "{}{}", run.stdout, run.stderr);
    assert_eq!(run.defects().len(), 0, "{}", run.stdout);
}

#[test]
fn a_deadline_spelled_at_the_site_is_the_same_defect_in_another_shape() {
    // `Instant::now() + Duration::from_secs(30)` is a budget. `tools/
    // injection-harness` computes one exactly this way, and clause two is why
    // this round found it: nothing about it looks like `timeout(...)`.
    let dir = fixture(&[
        ("Cargo.toml", MANIFEST),
        ("waiter/Cargo.toml", PACKAGE),
        ("waiter/src/lib.rs", EMPTY_LIB),
        (
            "waiter/tests/tail.rs",
            r#"
use std::time::{Duration, Instant};

#[test]
fn the_push_arrives() {
    let deadline = Instant::now() + Duration::from_secs(30);
    assert!(Instant::now() <= deadline);
}
"#,
        ),
    ]);

    let run = gate(dir.path());
    assert_eq!(run.code, 1, "{}", run.stdout);
    assert!(run.says("spelled budget `Instant::now()"), "{}", run.stdout);
}

// --- reach: what the gate opened, said out loud -----------------------------

#[test]
fn every_rust_file_under_the_root_lands_in_exactly_one_bucket() {
    // The totality rule. A file in none of the buckets is a file the gate
    // skipped without saying so, which is how `scripts/` and `.github/` sat
    // outside the citation gate for twenty rounds.
    let dir = fixture(&[
        ("Cargo.toml", MANIFEST),
        ("waiter/Cargo.toml", PACKAGE),
        ("waiter/src/lib.rs", EMPTY_LIB),
        ("waiter/tests/settle.rs", "#[test]\nfn t() {}\n"),
        // A nested workspace: its files belong to ITS run, not this one.
        ("inner/Cargo.toml", "[workspace]\nmembers = []\n"),
        ("inner/src/lib.rs", EMPTY_LIB),
        // Cargo's output, which is not this repository's source.
        ("target/debug/build/generated.rs", EMPTY_LIB),
    ]);

    let report = blind_waits::run(&dir.path().join("Cargo.toml")).expect("the gate runs");
    let total = count_rust(dir.path());
    assert_eq!(
        report.coverage.scanned.len()
            + report.coverage.foreign_workspaces.len()
            + report.coverage.build_artifacts,
        total,
        "scanned {:?}, foreign {:?}, artifacts {}",
        report.coverage.scanned,
        report.coverage.foreign_workspaces,
        report.coverage.build_artifacts
    );
    assert_eq!(report.coverage.foreign_workspaces.len(), 1);
    assert_eq!(report.coverage.build_artifacts, 1);
}

#[test]
fn a_blind_wait_in_a_nested_workspace_is_not_this_runs_verdict() {
    // The other half of the same rule: counting a foreign file is only honest
    // if pointing the gate at ITS manifest still convicts it. Both runs, one
    // test, so the pair cannot drift.
    let dir = fixture(&[
        ("Cargo.toml", MANIFEST),
        ("waiter/Cargo.toml", PACKAGE),
        ("waiter/src/lib.rs", EMPTY_LIB),
        ("waiter/tests/settle.rs", "#[test]\nfn t() {}\n"),
        ("inner/Cargo.toml", "[workspace]\nmembers = [\"sub\"]\n"),
        ("inner/sub/Cargo.toml", &PACKAGE.replace("waiter", "sub")),
        ("inner/sub/src/lib.rs", EMPTY_LIB),
        (
            "inner/sub/tests/settle.rs",
            "#[test]\nfn t() {\n    std::thread::sleep(std::time::Duration::from_millis(1));\n}\n",
        ),
    ]);

    let outer = gate(dir.path());
    assert_eq!(
        outer.code, 0,
        "the outer run does not judge the inner tree\n{}{}",
        outer.stdout, outer.stderr
    );

    let inner = gate(&dir.path().join("inner"));
    assert_eq!(
        inner.code, 1,
        "and the inner run does\n{}{}",
        inner.stdout, inner.stderr
    );
}

#[test]
fn a_file_no_member_owns_is_scanned_and_said_out_loud() {
    let dir = fixture(&[
        ("Cargo.toml", MANIFEST),
        ("waiter/Cargo.toml", PACKAGE),
        ("waiter/src/lib.rs", EMPTY_LIB),
        ("waiter/tests/settle.rs", "#[test]\nfn t() {}\n"),
        (
            "scripts/tests/helper.rs",
            "#[test]\nfn t() {\n    std::thread::sleep(std::time::Duration::from_millis(1));\n}\n",
        ),
    ]);

    let run = gate(dir.path());
    assert!(
        run.says("unowned scripts/tests/helper.rs"),
        "the gate names what no member claims\n{}",
        run.stdout
    );
    assert_eq!(
        run.code, 1,
        "and judges it anyway — an unowned file is where an unwatched wait \
         lives\n{}{}",
        run.stdout, run.stderr
    );
}

// --- refusals: the answers that are not verdicts ----------------------------

#[test]
fn a_workspace_with_no_test_code_passes_and_says_that_is_why() {
    // R1054's requirement is that "clean" and "nothing was checked" LOOK
    // different — not that the second is a rejection. This gate refused for one
    // run and three of `git_hooks_smoke.rs`'s cases went red: the hook was
    // rejecting commits to fixture trees for a reason outside this law, which
    // is the shape R1079 named as a gate people route around.
    let dir = fixture(&[
        ("Cargo.toml", MANIFEST),
        ("waiter/Cargo.toml", PACKAGE),
        ("waiter/src/lib.rs", EMPTY_LIB),
    ]);

    let run = gate(dir.path());
    assert_eq!(
        run.code, 0,
        "a workspace with no test code satisfies a law about test code\n{}{}",
        run.stdout, run.stderr
    );
    assert!(
        run.says("no test code in this workspace"),
        "and it says so rather than reporting a clean check\n{}",
        run.stdout
    );
    assert!(
        !run.says("none of them blind or spelled"),
        "the clean-check sentence must NOT be what an empty tree prints\n{}",
        run.stdout
    );
    assert!(
        run.says("test code: 0 files"),
        "the reach line carries the same zero\n{}",
        run.stdout
    );
}

#[test]
fn a_file_that_does_not_parse_gets_no_verdict_rather_than_a_pass() {
    let dir = fixture(&[
        ("Cargo.toml", MANIFEST),
        ("waiter/Cargo.toml", PACKAGE),
        ("waiter/src/lib.rs", EMPTY_LIB),
        ("waiter/tests/settle.rs", "#[test]\nfn t() {}\n"),
        ("waiter/tests/broken.rs", "fn t( { this is not rust\n"),
    ]);

    let run = gate(dir.path());
    assert_eq!(run.code, 2, "{}{}", run.stdout, run.stderr);
    assert!(run.says("did not parse"), "{}{}", run.stdout, run.stderr);
}

#[test]
fn a_manifest_that_is_not_there_is_an_error_and_not_an_empty_pass() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let run = gate(dir.path());
    assert_eq!(run.code, 2, "{}{}", run.stdout, run.stderr);
    assert!(
        run.says("cargo metadata failed"),
        "{}{}",
        run.stdout,
        run.stderr
    );
}

// --- what cargo says, pinned ------------------------------------------------

#[test]
fn cargo_calls_a_tests_directory_target_a_test_and_a_benches_one_a_bench() {
    // The census asks cargo which targets are test code rather than deriving it
    // from the directory name, and R1079 is why: a third tool's behaviour is
    // measured, and the measurement is pinned so that the day cargo changes it,
    // THIS goes red and says which assumption moved.
    let dir = fixture(&[
        ("Cargo.toml", MANIFEST),
        (
            "waiter/Cargo.toml",
            &format!("{PACKAGE}\n[[bench]]\nname = \"speed\"\nharness = false\n"),
        ),
        ("waiter/src/lib.rs", EMPTY_LIB),
        ("waiter/tests/settle.rs", "#[test]\nfn t() {}\n"),
        ("waiter/benches/speed.rs", "fn main() {}\n"),
    ]);

    let census = blind_waits::census(&dir.path().join("Cargo.toml")).expect("cargo answers");
    let named: Vec<String> = census
        .test_target_srcs
        .iter()
        .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .collect();
    assert!(
        named.contains(&"settle.rs".to_string()),
        "cargo lists the integration test as a `test` target: {named:?}"
    );
    assert!(
        named.contains(&"speed.rs".to_string()),
        "and the benchmark as a `bench` target: {named:?}"
    );
    assert!(
        !named.contains(&"lib.rs".to_string()),
        "and does not call the library either of them: {named:?}"
    );
}

fn count_rust(dir: &Path) -> usize {
    let mut total = 0;
    for entry in std::fs::read_dir(dir).expect("read_dir").flatten() {
        let path = entry.path();
        if path.is_dir() {
            total += count_rust(&path);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            total += 1;
        }
    }
    total
}
