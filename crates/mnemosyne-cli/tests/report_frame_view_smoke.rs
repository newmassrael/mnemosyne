//! Round 432 — `report-frame-view` verb smoke tests.
//!
//! End-to-end over a v12 store with an in-frame succession: the held belief
//! swaps at the successor's canon point, other frames never leak in,
//! incomparable coordinates surface as `unknown`, and the fail-loud
//! boundaries (unknown frame / non-section query point) error.

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

/// Chapters ch-1..ch-3 declared linear; jonathan's belief f-old [ch-1..)
/// superseded by f-new at ch-3; seward holds an unrelated fact.
fn write_workspace(workspace: &Path) {
    fs::create_dir_all(workspace.join("docs/.atomic")).unwrap();
    fs::write(
        workspace.join("mnemosyne.toml"),
        "[workspace]\n[continuity]\ncanon_order_path = \"canon-order.json\"\n",
    )
    .unwrap();
    let atomic = serde_json::json!({
        "schema_version": 12,
        "sections": { "ch-1": {}, "ch-2": {}, "ch-3": {} },
        "changelog_entries": {},
        "frames": { "jonathan": {}, "seward": {} },
        "narrative_facts": {
            "f-old": {
                "frame": "jonathan",
                "claim": "the count is an eccentric nobleman",
                "canon_from": "ch-1",
                "evidence": ["ch-1"]
            },
            "f-new": {
                "frame": "jonathan",
                "claim": "the count is something unnatural",
                "canon_from": "ch-3",
                "evidence": ["ch-3"],
                "supersedes_in_frame": "f-old"
            },
            "f-other": {
                "frame": "seward",
                "claim": "Renfield is a self-contained case",
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
    fs::write(
        workspace.join("canon-order.json"),
        serde_json::json!({
            "schema": "canon-order/v1",
            "edges": [["ch-1", "ch-2"], ["ch-2", "ch-3"]]
        })
        .to_string(),
    )
    .unwrap();
}

fn view_json(workspace: &Path, frame: &str, at: &str) -> serde_json::Value {
    let out = run(
        workspace,
        &["report-frame-view", "--frame", frame, "--at", at, "--json"],
    );
    assert!(out.status.success(), "{:?}", out);
    serde_json::from_slice(&out.stdout).expect("json output")
}

#[test]
fn succession_swaps_the_held_belief_at_its_canon_point() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    let at2 = view_json(tmp.path(), "jonathan", "ch-2");
    assert_eq!(at2["holding_count"], 1);
    assert_eq!(at2["holding"][0]["fact_id"], "f-old");
    assert_eq!(at2["not_holding"], 1);
    let at3 = view_json(tmp.path(), "jonathan", "ch-3");
    assert_eq!(at3["holding_count"], 1);
    assert_eq!(at3["holding"][0]["fact_id"], "f-new");
    // f-old is definitively ended (derived closure), not unknown.
    assert_eq!(at3["not_holding"], 1);
    assert_eq!(at3["unknown"].as_array().unwrap().len(), 0);
    // Other frames never leak in.
    let seward = view_json(tmp.path(), "seward", "ch-2");
    assert_eq!(seward["holding_count"], 1);
    assert_eq!(seward["holding"][0]["fact_id"], "f-other");
}

#[test]
fn undeclared_coordinates_surface_as_unknown() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    // Bypass the declared order with an EMPTY declaration: ch-1 vs ch-2
    // becomes incomparable, so f-old/f-other are unknown at ch-2, and only
    // the exact-coordinate query stays decidable.
    fs::write(tmp.path().join("empty-order.json"), r#"{"edges": []}"#).unwrap();
    let out = run(
        tmp.path(),
        &[
            "report-frame-view",
            "--frame",
            "jonathan",
            "--at",
            "ch-2",
            "--order",
            "empty-order.json",
            "--json",
        ],
    );
    assert!(out.status.success(), "{:?}", out);
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["holding_count"], 0);
    assert!(v["unknown"]
        .as_array()
        .unwrap()
        .iter()
        .any(|u| u == "f-old"));
}

#[test]
fn fail_loud_on_unknown_frame_or_non_section_point() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    let out = run(
        tmp.path(),
        &["report-frame-view", "--frame", "nobody", "--at", "ch-1"],
    );
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("frames registry"));
    let out = run(
        tmp.path(),
        &["report-frame-view", "--frame", "jonathan", "--at", "ch-99"],
    );
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("ch-99"));
}
