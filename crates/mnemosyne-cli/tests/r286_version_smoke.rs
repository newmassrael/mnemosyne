//! Round 286 — `--version` / `-V` / `version` surface smoke.
//!
//! Verifies the universal CLI version surface: three trigger forms,
//! `<name> <semver> (<git-describe>)` format, build hash embedded
//! at compile time via `build.rs`. The format MUST mirror
//! rustc/cargo so adopters can identify which round/commit produced
//! the binary without resorting to `mtime` or `strings | grep`.

use std::process::Command;

fn cli_binary() -> &'static str {
    env!("CARGO_BIN_EXE_mnemosyne-cli")
}

fn run_cli(args: &[&str]) -> std::process::Output {
    Command::new(cli_binary())
        .args(args)
        .output()
        .expect("run mnemosyne-cli")
}

/// Match `mnemosyne-cli <semver> (<hash>)` where `<hash>` may carry the
/// `-dirty` suffix or be the literal `unknown` (tarball install case).
fn assert_version_line(stdout: &str, prog: &str) {
    let trimmed = stdout.trim();
    assert!(
        trimmed.starts_with(&format!("{} ", prog)),
        "version line must start with `{} `; got: {:?}",
        prog,
        trimmed
    );
    assert!(
        trimmed.contains(" ("),
        "version line must contain ` (` separator before hash; got: {:?}",
        trimmed
    );
    assert!(
        trimmed.ends_with(')'),
        "version line must end with `)`; got: {:?}",
        trimmed
    );
}

#[test]
fn cli_long_flag_version() {
    let out = run_cli(&["--version"]);
    assert!(
        out.status.success(),
        "--version must exit 0; stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_version_line(&String::from_utf8_lossy(&out.stdout), "mnemosyne-cli");
}

#[test]
fn cli_short_flag_version() {
    let out = run_cli(&["-V"]);
    assert!(out.status.success(), "-V must exit 0");
    assert_version_line(&String::from_utf8_lossy(&out.stdout), "mnemosyne-cli");
}

#[test]
fn cli_subcommand_version() {
    let out = run_cli(&["version"]);
    assert!(out.status.success(), "`version` subcmd must exit 0");
    assert_version_line(&String::from_utf8_lossy(&out.stdout), "mnemosyne-cli");
}

#[test]
fn cli_help_first_line_carries_version() {
    // Round 286 — help heading must also embed version so a single
    // `--help` call answers both "what is this" and "which build".
    let out = run_cli(&["--help"]);
    assert!(out.status.success());
    let first_line = String::from_utf8_lossy(&out.stdout)
        .lines()
        .next()
        .unwrap_or("")
        .to_string();
    assert!(
        first_line.starts_with("mnemosyne-cli ") && first_line.contains(" ("),
        "help heading must carry `mnemosyne-cli <semver> (<hash>)`; got: {:?}",
        first_line
    );
}

#[test]
fn cli_three_forms_yield_identical_output() {
    // --version / -V / version all hit the same arm — identical bytes.
    let a = String::from_utf8_lossy(&run_cli(&["--version"]).stdout).into_owned();
    let b = String::from_utf8_lossy(&run_cli(&["-V"]).stdout).into_owned();
    let c = String::from_utf8_lossy(&run_cli(&["version"]).stdout).into_owned();
    assert_eq!(a, b, "--version vs -V output differs");
    assert_eq!(a, c, "--version vs `version` output differs");
}
