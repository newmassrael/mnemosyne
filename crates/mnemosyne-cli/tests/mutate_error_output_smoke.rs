//! Round 691 (DEBT-DOUBLE-STDERR regression) — the atomic-mutate error prints
//! exactly once, and the `--json` error output stays pure json.
//!
//! R684 fixed the double-print structurally but shipped no test — the
//! cost-no-object review flagged that the CLI crate already has an
//! stderr-asserting subprocess harness. This pins the fix at that layer: a
//! revert to the double-print reddens CI here. The suppression signal is now a
//! typed `CliError` variant `main` matches, not a marker recovered by
//! `downcast_ref`; the behavior this test asserts is invariant across that
//! rework, which is exactly why the test guards it.

use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn run(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_mnemosyne-cli"))
        .args(args)
        .current_dir(dir)
        .output()
        .expect("cli exec")
}

/// A minimal workspace: only `mnemosyne.toml` (no store file — a missing
/// sidecar loads as empty, so the mutate reaches the atomic-mutate error path
/// on the absent section rather than a config/load error).
fn workspace() -> TempDir {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("mnemosyne.toml"), "[workspace]\n").unwrap();
    tmp
}

const FAILING_MUTATE: &[&str] = &[
    "set-section-intent",
    "--section",
    "no-such-section",
    "--intent",
    "x",
];

#[test]
fn atomic_mutate_error_prints_exactly_once() {
    let tmp = workspace();
    let out = run(tmp.path(), FAILING_MUTATE);
    assert!(
        !out.status.success(),
        "the mutate must fail (exit non-zero)"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    let error_lines = stderr.lines().filter(|l| l.starts_with("error:")).count();
    assert_eq!(
        error_lines, 1,
        "the atomic-mutate error must print exactly once, got {error_lines}:\n{stderr}"
    );
}

#[test]
fn atomic_mutate_json_error_is_pure_json() {
    let tmp = workspace();
    let mut args = FAILING_MUTATE.to_vec();
    args.push("--json");
    let out = run(tmp.path(), &args);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.lines().any(|l| l.starts_with("error:")),
        "the --json error output must not carry a trailing non-json `error:` line:\n{stderr}"
    );
    assert!(
        serde_json::from_str::<serde_json::Value>(stderr.trim()).is_ok(),
        "the --json error output must be valid json:\n{stderr}"
    );
}
