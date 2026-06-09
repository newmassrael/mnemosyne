//! R418 — `report-confirmation` read-only projection smoke test.
//!
//! A semantic-only confirm (no deterministic tool linkage-check) classifies as
//! `proposed` and lands on the confirmation-debt queue. Pure projection over the
//! stored events; the event is written directly into the store (no dependency on
//! the `add-confirmation-event` write path).

use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn cli() -> &'static str {
    env!("CARGO_BIN_EXE_mnemosyne-cli")
}

fn write_workspace(ws: &Path) {
    fs::create_dir_all(ws.join("docs/.atomic")).unwrap();
    fs::write(
        ws.join("mnemosyne.toml"),
        "[workspace]\n[schema]\nentry_id_prefix = \"Round \"\n",
    )
    .unwrap();
    let store = serde_json::json!({
        "schema_version": 10,
        "sections": { "sec": { "title": "T", "parent_doc": "d" } },
        "changelog_entries": {},
        "confirmation_events": {
            "evt-1": {
                "claim": { "kind": "verifies_binding", "section_id": "sec", "file": "t.h", "symbol": "f" },
                "confirmer": { "kind": "model", "id": "claude", "version": "v" },
                "method": "semantic_review",
                "authoring_run": "runA",
                "confirming_run": "runB",
                "verdict": "confirm",
                "rationale": "verifies it",
                "timestamp": "2026-06-09T00:00:00Z"
            }
        }
    });
    fs::write(
        ws.join("docs/.atomic/workspace.atomic.json"),
        serde_json::to_string_pretty(&store).unwrap(),
    )
    .unwrap();
}

#[test]
fn report_confirmation_json_classifies_proposed_and_debt() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    let out = Command::new(cli())
        .current_dir(tmp.path())
        .args(["report-confirmation", "--json"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["total_claims"], 1);
    assert_eq!(
        v["proposed_count"], 1,
        "a semantic-only confirm is proposed (no tool linkage)"
    );
    assert_eq!(v["confirmed_count"], 0);
    assert_eq!(v["debt_count"], 1);
}
