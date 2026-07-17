//! Round 437 — `add-entity` / `report-entity` / `--entity` filter smoke.
//!
//! End-to-end: register an entity, author a fact carrying it, read the
//! dossier and the entity-filtered frame view; typo'd entity fails loud on
//! both reads; unregistered ref fails loud on the write.

use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

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

fn write_workspace(workspace: &Path) {
    fs::create_dir_all(workspace.join("docs/.atomic")).unwrap();
    fs::write(workspace.join("mnemosyne.toml"), "[workspace]\n").unwrap();
    let atomic = serde_json::json!({
        "schema_version": 15,
        "sections": { "ch-1": {}, "ch-2": {} },
        "changelog_entries": {},
        "frames": { "jonathan": {} }
    });
    fs::write(
        workspace.join("docs/.atomic/workspace.atomic.json"),
        serde_json::to_string_pretty(&atomic).unwrap(),
    )
    .unwrap();
}

#[test]
fn entity_axis_end_to_end() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    // Unregistered entity ref on the write path fails loud.
    let out = run(
        tmp.path(),
        &[
            "add-fact",
            "--fact",
            "f-castle",
            "--frame",
            "jonathan",
            "--claim",
            "the count never eats",
            "--canon-from",
            "ch-1",
            "--evidence",
            "ch-1",
            "--entities",
            "dracula",
        ],
    );
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("entity registry"));
    // The kind is a registry ref too: naming one nobody declared rejects, and
    // says so, before the entity exists.
    let out = run(
        tmp.path(),
        &[
            "add-entity",
            "--entity",
            "dracula",
            "--kind",
            "character",
            "--description",
            "the count",
        ],
    );
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("not a registered entity kind"));
    // Declare the vocabulary, then the write lands.
    let out = run(
        tmp.path(),
        &[
            "add-entity-kind",
            "--kind",
            "character",
            "--description",
            "a person in the story",
        ],
    );
    assert!(out.status.success(), "{:?}", out);
    let out = run(
        tmp.path(),
        &[
            "add-entity",
            "--entity",
            "dracula",
            "--kind",
            "character",
            "--description",
            "the count",
        ],
    );
    assert!(out.status.success(), "{:?}", out);
    let out = run(
        tmp.path(),
        &[
            "add-fact",
            "--fact",
            "f-castle",
            "--frame",
            "jonathan",
            "--claim",
            "the count never eats",
            "--canon-from",
            "ch-1",
            "--evidence",
            "ch-1",
            "--entities",
            "dracula",
        ],
    );
    assert!(out.status.success(), "{:?}", out);
    // Dossier: all facts about the entity.
    let out = run(
        tmp.path(),
        &["report-entity", "--entity", "dracula", "--json"],
    );
    assert!(out.status.success(), "{:?}", out);
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["kind"], "character");
    assert_eq!(v["fact_count"], 1);
    assert_eq!(v["facts"][0]["fact_id"], "f-castle");
    // Entity-filtered frame view.
    let out = run(
        tmp.path(),
        &[
            "report-frame-view",
            "--frame",
            "jonathan",
            "--entity",
            "dracula",
            "--at",
            "ch-2",
            "--json",
        ],
    );
    assert!(out.status.success(), "{:?}", out);
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["entity"], "dracula");
    // No declared order: ch-1 vs ch-2 incomparable — surfaced as unknown
    // (B-1 honesty), never silently absent.
    assert_eq!(v["holding_count"], 0);
    assert!(v["unknown"]
        .as_array()
        .unwrap()
        .iter()
        .any(|u| u == "f-castle"));
    // Typo'd entity fails loud on the dossier read.
    let out = run(tmp.path(), &["report-entity", "--entity", "dracual"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("entity registry"));
}
