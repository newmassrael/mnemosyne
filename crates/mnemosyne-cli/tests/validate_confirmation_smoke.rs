//! R419 — `validate-confirmation` gate smoke test.
//!
//! (i) Without a severity the gate is DISABLED (exit 0, opt-in). (ii) With
//! `--severity reject`, a Normative + Dedicated `verifies` binding that has no
//! Confirmed claim fails (exit 1). (iii) The same binding with a tool
//! linkage-check + an independent semantic confirm passes (exit 0).

use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn cli() -> &'static str {
    env!("CARGO_BIN_EXE_mnemosyne-cli")
}

fn claim() -> serde_json::Value {
    serde_json::json!({ "kind": "verifies_binding", "section_id": "sec", "file": "t.h", "symbol": "f" })
}

fn write_workspace(ws: &Path, confirmed: bool) {
    fs::create_dir_all(ws.join("docs/.atomic")).unwrap();
    fs::write(
        ws.join("mnemosyne.toml"),
        "[workspace]\n[schema]\nentry_id_prefix = \"Round \"\n",
    )
    .unwrap();
    let mut store = serde_json::json!({
        "schema_version": 10,
        "sections": { "sec": {
            "title": "T", "parent_doc": "d",
            "bindings": [{ "file": "t.h", "symbol": "f", "kind": "verifies" }]
        }},
        "changelog_entries": {}
    });
    if confirmed {
        store["confirmation_events"] = serde_json::json!({
            "e1": {
                "claim": claim(),
                "confirmer": { "kind": "tool", "id": "linkchk", "version": "1" },
                "method": "linkage_check",
                "authoring_run": "runA", "confirming_run": "runTool",
                "verdict": "confirm", "rationale": "r", "timestamp": "t"
            },
            "e2": {
                "claim": claim(),
                "confirmer": { "kind": "model", "id": "claude", "version": "1" },
                "method": "semantic_review",
                "authoring_run": "runA", "confirming_run": "runB",
                "verdict": "confirm", "rationale": "r", "timestamp": "t"
            }
        });
    }
    fs::write(
        ws.join("docs/.atomic/workspace.atomic.json"),
        serde_json::to_string_pretty(&store).unwrap(),
    )
    .unwrap();
}

fn run(ws: &Path, args: &[&str]) -> std::process::Output {
    Command::new(cli())
        .current_dir(ws)
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn gate_disabled_without_severity() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path(), false);
    let out = run(tmp.path(), &["validate-confirmation"]);
    assert!(out.status.success(), "disabled gate exits 0");
    assert!(String::from_utf8_lossy(&out.stdout).contains("disabled"));
}

#[test]
fn gate_rejects_unconfirmed_binding() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path(), false);
    let out = run(
        tmp.path(),
        &["validate-confirmation", "--severity", "reject"],
    );
    assert!(
        !out.status.success(),
        "an unconfirmed verifies binding must fail under reject"
    );
}

#[test]
fn gate_passes_confirmed_binding() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path(), true);
    let out = run(
        tmp.path(),
        &["validate-confirmation", "--severity", "reject"],
    );
    assert!(
        out.status.success(),
        "a confirmed binding passes; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}
