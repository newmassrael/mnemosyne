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
fn tree(root: &Path, source: &str) -> PathBuf {
    fs::create_dir_all(root.join("logs")).expect("mkdir");
    fs::write(root.join("src.txt"), source).expect("write source");
    let suite = root.join("suite.sh");
    fs::write(
        &suite,
        "#!/bin/sh\n\
         if grep -q HEALTHY src.txt; then\n\
         printf 'test result: ok. 2 passed; 0 failed; 0 ignored\\n'\n\
         else\n\
         printf 'failures:\\n    the_law\\n\\ntest result: FAILED. 1 passed; 1 failed; 0 ignored\\n'\n\
         fi\n",
    )
    .expect("write suite");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&suite, fs::Permissions::from_mode(0o755)).expect("chmod");
    }
    suite
}

fn manifest(root: &Path, injections: serde_json::Value) -> PathBuf {
    manifest_with(root, injections, serde_json::Value::Null)
}

fn manifest_with(
    root: &Path,
    injections: serde_json::Value,
    min_free_mb: serde_json::Value,
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
    fs::write(&path, serde_json::to_string(&body).expect("json")).expect("write manifest");
    path
}

fn harness(manifest: &Path) -> std::process::Output {
    Command::new(binary())
        .arg(manifest)
        .output()
        .expect("harness runs")
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
