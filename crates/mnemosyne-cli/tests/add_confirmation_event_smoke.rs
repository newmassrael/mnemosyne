//! R417 — `add-confirmation-event` subcommand smoke test.
//!
//! End-to-end (CLI → primitive → store): (i) a valid VerifiesBinding confirm
//! lands exactly one event under a derived `evt-` id; (ii) self-confirm (the
//! same run authoring and confirming) rejects; (iii) an unknown enum tag rejects.

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
        "sections": { "sec": { "title": "Test Section", "parent_doc": "docs/spec" } },
        "changelog_entries": {}
    });
    fs::write(
        ws.join("docs/.atomic/workspace.atomic.json"),
        serde_json::to_string_pretty(&store).unwrap(),
    )
    .unwrap();
}

fn base_args(sidecar: &str) -> Vec<String> {
    [
        "add-confirmation-event",
        "--section",
        "sec",
        "--file",
        "tests/Test1.h",
        "--symbol",
        "verify_foo",
        "--confirmer-kind",
        "model",
        "--confirmer-id",
        "claude",
        "--confirmer-version",
        "2026-06",
        "--method",
        "semantic_review",
        "--verdict",
        "confirm",
        "--authoring-run",
        "runA",
        "--confirming-run",
        "runB",
        "--rationale",
        "verifies the bound requirement",
        "--timestamp",
        "2026-06-09T00:00:00Z",
        "--sidecar",
        sidecar,
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

#[test]
fn add_confirmation_event_lands_under_derived_id() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    let sidecar = tmp.path().join("docs/.atomic/workspace.atomic.json");
    let out = Command::new(cli())
        .current_dir(tmp.path())
        .args(base_args(sidecar.to_str().unwrap()))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let store: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&sidecar).unwrap()).unwrap();
    let events = store["confirmation_events"].as_object().unwrap();
    assert_eq!(events.len(), 1, "one event recorded");
    let (id, ev) = events.iter().next().unwrap();
    assert!(id.starts_with("evt-"), "derived id: {id}");
    assert_eq!(ev["verdict"], "confirm");
    assert_eq!(ev["confirmer"]["kind"], "model");
}

#[test]
fn add_confirmation_event_self_confirm_rejected() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    let sidecar = tmp.path().join("docs/.atomic/workspace.atomic.json");
    let mut args = base_args(sidecar.to_str().unwrap());
    let pos = args.iter().position(|a| a == "--confirming-run").unwrap();
    args[pos + 1] = "runA".to_string(); // equal to authoring-run
    let out = Command::new(cli())
        .current_dir(tmp.path())
        .args(&args)
        .output()
        .unwrap();
    assert!(!out.status.success(), "self-confirm must fail");
    assert!(String::from_utf8_lossy(&out.stderr).contains("self-confirm"));
}

#[test]
fn add_confirmation_event_bad_method_rejected() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    let sidecar = tmp.path().join("docs/.atomic/workspace.atomic.json");
    let mut args = base_args(sidecar.to_str().unwrap());
    let pos = args.iter().position(|a| a == "--method").unwrap();
    args[pos + 1] = "bogus".to_string();
    let out = Command::new(cli())
        .current_dir(tmp.path())
        .args(&args)
        .output()
        .unwrap();
    assert!(!out.status.success(), "unknown method tag must fail");
}
