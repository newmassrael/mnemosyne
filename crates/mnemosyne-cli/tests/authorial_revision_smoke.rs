//! Round 434 — `amend-fact` / `retract-fact` verb smoke tests.
//!
//! End-to-end over a seeded store: the authorial typo-fix loop (the sec 7.10
//! probe gap) — amend revises in place keeping the id, retract removes an
//! unreferenced fact, both demand `--reason`, and the divergent add-fact
//! reject now advises the authorial path alongside in-frame supersession.

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
        "schema_version": 13,
        "sections": { "ch-1": {}, "ch-2": {} },
        "changelog_entries": {},
        "frames": { "jonathan": {} },
        "narrative_facts": {
            "f-typo": {
                "frame": "jonathan",
                "claim": "the count is an eccentric noblemna",
                "canon_from": "ch-1",
                "evidence": ["ch-1"]
            }
        }
    });
    fs::write(
        workspace.join("docs/.atomic/workspace.atomic.json"),
        serde_json::to_string_pretty(&atomic).unwrap(),
    )
    .unwrap();
}

fn store_json(workspace: &Path) -> serde_json::Value {
    let raw = fs::read_to_string(workspace.join("docs/.atomic/workspace.atomic.json")).unwrap();
    serde_json::from_str(&raw).unwrap()
}

#[test]
fn amend_fixes_a_typo_in_place_and_requires_reason() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    // Without --reason: rejected before touching the store.
    let out = run(
        tmp.path(),
        &[
            "amend-fact",
            "--fact",
            "f-typo",
            "--frame",
            "jonathan",
            "--claim",
            "the count is an eccentric nobleman",
            "--canon-from",
            "ch-1",
            "--evidence",
            "ch-1",
        ],
    );
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("--reason"));
    // With --reason: the claim is revised, the id stays.
    let out = run(
        tmp.path(),
        &[
            "amend-fact",
            "--fact",
            "f-typo",
            "--frame",
            "jonathan",
            "--claim",
            "the count is an eccentric nobleman",
            "--canon-from",
            "ch-1",
            "--evidence",
            "ch-1",
            "--reason",
            "typo: noblemna",
        ],
    );
    assert!(out.status.success(), "{:?}", out);
    let store = store_json(tmp.path());
    assert_eq!(
        store["narrative_facts"]["f-typo"]["claim"],
        "the count is an eccentric nobleman"
    );
}

#[test]
fn retract_removes_and_divergent_add_advises_both_paths() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    // Divergent re-add: the reject names BOTH revision paths (sec 7.10 fix).
    let out = run(
        tmp.path(),
        &[
            "add-fact",
            "--fact",
            "f-typo",
            "--frame",
            "jonathan",
            "--claim",
            "something divergent",
            "--canon-from",
            "ch-1",
            "--evidence",
            "ch-1",
        ],
    );
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("supersede in-frame") && stderr.contains("amend_fact"),
        "{stderr}"
    );
    // Retract demands a reason, then removes.
    let out = run(tmp.path(), &["retract-fact", "--fact", "f-typo"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("--reason"));
    let out = run(
        tmp.path(),
        &[
            "retract-fact",
            "--fact",
            "f-typo",
            "--reason",
            "authorial slip",
        ],
    );
    assert!(out.status.success(), "{:?}", out);
    let store = store_json(tmp.path());
    assert!(store["narrative_facts"].as_object().unwrap().is_empty());
}
