//! A key the store does not model is refused at the wire, and the refusal
//! leaves the store as it found it.
//!
//! The adopter's measurement this closes, reproduced here step for step: an
//! inventory entry carrying an undeclared `"modality"` answered
//! `query --inventory` without it, and an unrelated `set-inventory-status` then
//! exited 0 and wrote the store back WITHOUT it. A refusal inside `load` is only
//! half of the property — the half a caller sees is that no command reads past
//! the key and no command writes over it, so these run the real binary.
//!
//! `modality` itself became a field in Round 1321, so the key injected here is
//! one the entry still does not model.

use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

const STORE: &str = "docs/.atomic/workspace.atomic.json";

fn cli_binary() -> &'static str {
    env!("CARGO_BIN_EXE_mnemosyne-cli")
}

fn run(workspace: &Path, args: &[&str]) -> std::process::Output {
    Command::new(cli_binary())
        .args(args)
        .current_dir(workspace)
        .output()
        .expect("cli exec")
}

/// A workspace holding one active inventory entry, written by the CLI itself.
fn workspace_with_one_entry() -> TempDir {
    let ws = TempDir::new().unwrap();
    fs::write(ws.path().join("mnemosyne.toml"), "[workspace]\n").unwrap();
    let add = run(
        ws.path(),
        &["add-inventory-entry", "--id", "REQ-1", "--status", "active"],
    );
    assert!(
        add.status.success(),
        "fixture: add-inventory-entry must succeed; stderr={}",
        String::from_utf8_lossy(&add.stderr)
    );
    ws
}

#[test]
fn an_undeclared_key_is_refused_by_the_read_and_survives_the_write() {
    let ws = workspace_with_one_entry();
    let store_path = ws.path().join(STORE);
    let mut store: serde_json::Value =
        serde_json::from_slice(&fs::read(&store_path).unwrap()).unwrap();
    store["inventory_entries"]["REQ-1"]
        .as_object_mut()
        .expect("fixture: REQ-1 is an object")
        .insert(
            "acceptance_criteria".to_string(),
            serde_json::json!("responds within two seconds"),
        );
    let injected = serde_json::to_vec_pretty(&store).unwrap();
    fs::write(&store_path, &injected).unwrap();

    let query = run(ws.path(), &["query", "--inventory", "REQ-1", "--json"]);
    let said = String::from_utf8_lossy(&query.stderr);
    assert!(
        !query.status.success(),
        "a read must not answer past a key it does not model; stdout={}",
        String::from_utf8_lossy(&query.stdout)
    );
    assert!(
        said.contains("unknown field `acceptance_criteria`")
            && said.contains("inventory_entries.REQ-1"),
        "the refusal names the key and the entry it sits on: {said}"
    );

    let write = run(
        ws.path(),
        &[
            "set-inventory-status",
            "--id",
            "REQ-1",
            "--status",
            "reserved",
            "--reason",
            "probe",
            "--json",
        ],
    );
    assert!(
        !write.status.success(),
        "an unrelated write must not succeed over a key it would erase; stdout={}",
        String::from_utf8_lossy(&write.stdout)
    );
    assert_eq!(
        fs::read(&store_path).unwrap(),
        injected,
        "the refused write left the store byte-identical, key included"
    );
}

/// CONTROL: the same write on the same entry, with nothing injected, succeeds —
/// so the refusal above is about the key and not about the command.
#[test]
fn the_same_write_without_the_key_succeeds() {
    let ws = workspace_with_one_entry();
    let write = run(
        ws.path(),
        &[
            "set-inventory-status",
            "--id",
            "REQ-1",
            "--status",
            "reserved",
            "--reason",
            "probe",
            "--json",
        ],
    );
    assert!(
        write.status.success(),
        "control: stderr={}",
        String::from_utf8_lossy(&write.stderr)
    );
}
