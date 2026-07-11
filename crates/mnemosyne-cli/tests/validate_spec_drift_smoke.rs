//! RFC-001 UC-1 "B2" — `validate-spec-drift` subcommand smoke tests.
//!
//! Test scope:
//! (i) `[workspace.spec_source]` omission → skip mode (exit 0, explicit log)
//! (ii) all Sections at the workspace rev → 0 drift, exit 0
//! (iii) Active Section trailing the workspace rev → drift; default `warn`
//!  exits 0 and lists it; JSON shape carries the contract fields
//! (iv) `--severity reject` on drift → exit 1
//! (v) Superseded Section trailing the rev → exempt (partial-migration)
//! (vi) `[spec_drift] severity = "reject"` in config (no CLI flag) → exit 1
//! (vii) Section without `normative_excerpt` → never drift

use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn cli_binary() -> &'static str {
    env!("CARGO_BIN_EXE_mnemosyne-cli")
}

/// Build a workspace. When `spec_source` is `Some((url, revision))` the
/// `[workspace.spec_source]` table is written; `drift_severity` adds a
/// `[spec_drift]` table. `sections` is the atomic store's `sections` map.
fn write_workspace(
    workspace: &Path,
    spec_source: Option<(&str, &str)>,
    drift_severity: Option<&str>,
    sections: serde_json::Value,
) {
    fs::create_dir_all(workspace.join("docs/.atomic")).unwrap();
    let mut cfg = String::from(
        "[workspace]\n\
 [schema]\nentry_id_prefix = \"Round \"\n",
    );
    if let Some((url, revision)) = spec_source {
        cfg.push_str(&format!(
            "[workspace.spec_source]\nurl = \"{url}\"\nrevision = \"{revision}\"\n"
        ));
    }
    if let Some(sev) = drift_severity {
        cfg.push_str(&format!("[spec_drift]\nseverity = \"{sev}\"\n"));
    }
    fs::write(workspace.join("mnemosyne.toml"), cfg).unwrap();

    let atomic = serde_json::json!({
    "schema_version": 4,
    "sections": sections,
    "changelog_entries": {}
    });
    fs::write(
        workspace.join("docs/.atomic/workspace.atomic.json"),
        serde_json::to_string_pretty(&atomic).unwrap(),
    )
    .unwrap();
    fs::write(workspace.join("docs/GENERATED.md"), "# Stub\n").unwrap();
}

/// One section JSON with a `normative_excerpt` anchored at `source_revision`
/// and an optional `decision_status`.
fn spec_section(source_revision: &str, decision_status: Option<&str>) -> serde_json::Value {
    let mut v = serde_json::json!({
    "title": "Spec section",
    "parent_doc": "docs/GENERATED.md",
    "normative_excerpt": {
    "text": "the normative text",
    "anchor_url": "https://www.w3.org/TR/scxml/#x",
    "source_revision": source_revision
    }
    });
    if let Some(status) = decision_status {
        v["decision_status"] = serde_json::Value::String(status.to_string());
    }
    v
}

fn run_cli(workspace: &Path, args: &[&str]) -> std::process::Output {
    Command::new(cli_binary())
        .args(args)
        .current_dir(workspace)
        .output()
        .expect("cli exec")
}

#[test]
fn case_i_skip_mode_when_spec_source_unconfigured() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path(), None, None, serde_json::json!({}));
    let out = run_cli(tmp.path(), &["validate-spec-drift"]);
    assert!(out.status.success(), "exit code: {:?}", out.status.code());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("skipped") && stdout.contains("spec_source"),
        "stdout: {stdout}"
    );
}

#[test]
fn case_ii_all_sections_current_no_drift() {
    let tmp = TempDir::new().unwrap();
    write_workspace(
        tmp.path(),
        Some(("https://www.w3.org/TR/scxml/", "2024-rec")),
        None,
        serde_json::json!({ "scxml-3.13": spec_section("2024-rec", Some("active")) }),
    );
    let out = run_cli(tmp.path(), &["validate-spec-drift", "--json"]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let parsed: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&out.stdout)).expect("valid JSON");
    assert_eq!(parsed["drift_count"], 0);
}

#[test]
fn case_iii_active_stale_warn_lists_and_exits_zero() {
    let tmp = TempDir::new().unwrap();
    write_workspace(
        tmp.path(),
        Some(("https://www.w3.org/TR/scxml/", "2024-rec")),
        None, // [spec_drift] absent → default warn
        serde_json::json!({ "scxml-3.13": spec_section("2020-rec", Some("active")) }),
    );
    let out = run_cli(tmp.path(), &["validate-spec-drift", "--json"]);
    assert!(
        out.status.success(),
        "default warn should exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let parsed: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&out.stdout)).expect("valid JSON");
    assert_eq!(parsed["primitive"], "validate-spec-drift");
    assert_eq!(parsed["workspace_revision"], "2024-rec");
    assert_eq!(parsed["severity"], "warn");
    assert_eq!(parsed["drift_count"], 1);
    let violations = parsed["violations"].as_array().expect("violations array");
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0]["section_id"], "scxml-3.13");
    assert_eq!(violations[0]["section_revision"], "2020-rec");
    assert_eq!(violations[0]["workspace_revision"], "2024-rec");
    assert_eq!(violations[0]["status"], "drift");
}

#[test]
fn case_iv_active_stale_reject_flag_exits_one() {
    let tmp = TempDir::new().unwrap();
    write_workspace(
        tmp.path(),
        Some(("https://www.w3.org/TR/scxml/", "2024-rec")),
        None,
        serde_json::json!({ "scxml-3.13": spec_section("2020-rec", Some("active")) }),
    );
    let out = run_cli(tmp.path(), &["validate-spec-drift", "--severity", "reject"]);
    assert!(
        !out.status.success(),
        "reject should exit 1; stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("drift") && stderr.contains("2024-rec"),
        "stderr should name the drift + workspace rev; got: {stderr}"
    );
}

#[test]
fn case_v_superseded_stale_is_exempt() {
    let tmp = TempDir::new().unwrap();
    write_workspace(
        tmp.path(),
        Some(("https://www.w3.org/TR/scxml/", "2024-rec")),
        Some("reject"), // even under reject, Superseded must not fire
        serde_json::json!({ "scxml-3.13": spec_section("2020-rec", Some("superseded")) }),
    );
    let out = run_cli(tmp.path(), &["validate-spec-drift", "--json"]);
    assert!(
        out.status.success(),
        "Superseded trailing rev is the partial-migration pattern; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let parsed: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&out.stdout)).expect("valid JSON");
    assert_eq!(parsed["drift_count"], 0);
}

#[test]
fn case_vi_config_severity_reject_gates_without_flag() {
    let tmp = TempDir::new().unwrap();
    write_workspace(
        tmp.path(),
        Some(("https://www.w3.org/TR/scxml/", "2024-rec")),
        Some("reject"), // workspace declares the gate; no CLI flag
        serde_json::json!({ "scxml-3.13": spec_section("2020-rec", Some("active")) }),
    );
    let out = run_cli(tmp.path(), &["validate-spec-drift"]);
    assert!(
        !out.status.success(),
        "config severity=reject should gate; stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
}

#[test]
fn case_vii_section_without_excerpt_never_drifts() {
    let tmp = TempDir::new().unwrap();
    write_workspace(
        tmp.path(),
        Some(("https://www.w3.org/TR/scxml/", "2024-rec")),
        Some("reject"),
        serde_json::json!({
        "ordinary-decision": { "title": "Plain", "parent_doc": "docs/GENERATED.md" }
        }),
    );
    let out = run_cli(tmp.path(), &["validate-spec-drift"]);
    assert!(
        out.status.success(),
        "a Section without normative_excerpt is not a spec mirror; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}
