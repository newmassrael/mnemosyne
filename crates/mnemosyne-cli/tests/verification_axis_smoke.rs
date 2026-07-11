//! R413/R414 — verify-axis CLI wiring smoke tests.
//!
//! Locks the end-to-end CLI surface the lib-level gate test cannot reach:
//! (i)   axis is OPT-IN — default run (no `--severity-verification`) emits
//!       `verification_missing_count == 0` and `severity_verification == null`,
//!       so a workspace that does not opt in is unaffected.
//! (ii)  `--severity-verification reject` ENABLES the axis for the run and
//!       rejects (exit 1) on a Normative + Dedicated section with no `verifies`.
//! (iii) `set-section-verification-expectation … by_construction` exempts the
//!       section — the gate then passes (exit 0, count 0).
//!
//! Other severities are pinned to `warn` so only the verification axis drives
//! the exit code (the `gap` section is also an implements-coverage gap).

use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

fn cli_binary() -> &'static str {
    env!("CARGO_BIN_EXE_mnemosyne-cli")
}

fn write_workspace(workspace: &Path) {
    fs::create_dir_all(workspace.join("docs/.atomic")).unwrap();
    fs::create_dir_all(workspace.join("src")).unwrap();
    let cfg = "[workspace]\n\
        [schema]\nentry_id_prefix = \"Round \"\n\
        [plugins.set_equality_validator]\npaths = [\"src/\"]\n";
    fs::write(workspace.join("mnemosyne.toml"), cfg).unwrap();
    let atomic = serde_json::json!({
        "schema_version": 9,
        // Normative (default) + Dedicated (default), zero verifies → a verify gap.
        "sections": {
            "req-a": { "title": "Req A", "parent_doc": "docs/GENERATED.md" }
        },
        "changelog_entries": {}
    });
    fs::write(
        workspace.join("docs/.atomic/workspace.atomic.json"),
        serde_json::to_string_pretty(&atomic).unwrap(),
    )
    .unwrap();
    fs::write(workspace.join("docs/GENERATED.md"), "# Stub\n").unwrap();
}

fn run_cli(workspace: &Path, args: &[&str]) -> Output {
    Command::new(cli_binary())
        .args(args)
        .current_dir(workspace)
        .output()
        .expect("cli exec")
}

/// Run `validate-code-refs --json` with every non-verification axis pinned to
/// `warn`, so only the verification axis can drive the exit code.
fn validate(workspace: &Path, extra: &[&str]) -> (Output, serde_json::Value) {
    let mut args = vec![
        "validate-code-refs",
        "--json",
        "--severity-missing",
        "warn",
        "--severity-binding",
        "warn",
        "--severity-coverage",
        "warn",
    ];
    args.extend_from_slice(extra);
    let out = run_cli(workspace, &args);
    let json: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("validate-code-refs --json parses");
    (out, json)
}

#[test]
fn verify_axis_is_opt_in_and_gates_then_exempts() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());

    // (i) Axis OFF by default: no flag, no config severity_verification.
    let (out, json) = validate(tmp.path(), &[]);
    assert!(
        out.status.success(),
        "default run must not reject (verify axis off): {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        json["verification_missing_count"], 0,
        "axis off must emit zero VerificationMissing"
    );
    assert!(
        json["severity_verification"].is_null(),
        "severity_verification must be null (off) when unset: {json}"
    );

    // (ii) Enable via override at reject → the Dedicated gap rejects (exit 1).
    let (out, json) = validate(tmp.path(), &["--severity-verification", "reject"]);
    assert!(
        !out.status.success(),
        "--severity-verification reject must reject the Dedicated gap"
    );
    assert_eq!(
        json["verification_missing_count"], 1,
        "the one Normative+Dedicated+0-verifies section is the gap: {json}"
    );

    // (iii) Classify ByConstruction → the gate exempts it (exit 0, count 0).
    let set = run_cli(
        tmp.path(),
        &[
            "set-section-verification-expectation",
            "--section",
            "§req-a",
            "--expectation",
            "by_construction",
            "--reason",
            "transcribed pseudocode, holistic coverage",
        ],
    );
    assert!(
        set.status.success(),
        "set-section-verification-expectation failed: {}",
        String::from_utf8_lossy(&set.stderr)
    );
    let (out, json) = validate(tmp.path(), &["--severity-verification", "reject"]);
    assert!(
        out.status.success(),
        "ByConstruction section must be exempt from the verify gate"
    );
    assert_eq!(
        json["verification_missing_count"], 0,
        "ByConstruction exempts the section: {json}"
    );
}
