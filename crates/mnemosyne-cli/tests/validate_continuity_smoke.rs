//! Round 431 — `validate-continuity` verb smoke tests.
//!
//! End-to-end over a v12 store with narrative facts: cross-frame conflict =
//! data (exit 0), same-frame overlap = violation (configured reject → exit
//! 1, `--severity warn` → exit 0), disabled when the `[continuity]` table is
//! absent, `--json` contract keys, declared-order pin mismatch fails loud.

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

/// Workspace: chapters ch-1..ch-3 (linear declared order), seward-frame fact
/// f-illness [ch-1..ch-2], `vampire_frame`-frame fact f-vampire [ch-2..),
/// with a recorded conflict edge — the design sec 7.7 acceptance shape
/// (cross-frame with "van-helsing", same-frame with "seward").
fn write_workspace_with_frames(workspace: &Path, continuity_table: &str, vampire_frame: &str) {
    fs::create_dir_all(workspace.join("docs/.atomic")).unwrap();
    fs::write(
        workspace.join("mnemosyne.toml"),
        format!("[workspace]\n{continuity_table}"),
    )
    .unwrap();
    let atomic = serde_json::json!({
        "schema_version": 12,
        "sections": { "ch-1": {}, "ch-2": {}, "ch-3": {} },
        "changelog_entries": {},
        "frames": { "seward": {}, "van-helsing": {} },
        "narrative_facts": {
            "f-illness": {
                "frame": "seward",
                "claim": "Lucy suffers from an unexplained illness",
                "canon_from": "ch-1", "canon_to": "ch-2",
                "evidence": ["ch-1"],
                "conflicts_with": ["f-vampire"]
            },
            "f-vampire": {
                "frame": vampire_frame,
                "claim": "Lucy is preyed upon by a vampire",
                "canon_from": "ch-2",
                "evidence": ["ch-2"]
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

fn write_workspace(workspace: &Path, continuity_table: &str) {
    write_workspace_with_frames(workspace, continuity_table, "van-helsing");
}

#[test]
fn cross_frame_conflict_is_data_exit_zero() {
    let tmp = TempDir::new().unwrap();
    write_workspace(
        tmp.path(),
        "[continuity]\ncanon_order_path = \"canon-order.json\"\n",
    );
    let out = run(tmp.path(), &["validate-continuity", "--json"]);
    assert!(out.status.success(), "{:?}", out);
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json output");
    assert_eq!(v["severity"], "reject");
    assert_eq!(v["violation_count"], 0);
    assert_eq!(v["cross_scope_pairs"], 1);
    assert_eq!(v["conflict_pairs_checked"], 1);
    assert_eq!(v["order_nodes"], 3);
}

#[test]
fn same_frame_overlap_rejects_and_warn_passes() {
    let tmp = TempDir::new().unwrap();
    // The vampire claim in seward's OWN frame: the same frame holds both
    // claims on an overlapping window.
    write_workspace_with_frames(
        tmp.path(),
        "[continuity]\ncanon_order_path = \"canon-order.json\"\n",
        "seward",
    );
    let out = run(tmp.path(), &["validate-continuity", "--json"]);
    assert!(!out.status.success(), "reject severity must exit 1");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json output");
    assert_eq!(v["violation_count"], 1);
    assert_eq!(v["violations"][0]["kind"], "frame_conflict_overlap");
    assert_eq!(v["violations"][0]["at"], "ch-2");
    // Same scan under --severity warn: reported but not gated.
    let out = run(tmp.path(), &["validate-continuity", "--severity", "warn"]);
    assert!(out.status.success(), "{:?}", out);
}

#[test]
fn disabled_without_table_and_scan_still_reports() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path(), "");
    let out = run(tmp.path(), &["validate-continuity"]);
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("disabled"), "{stdout}");
    assert!(stdout.contains("conflict_pairs=1"), "{stdout}");
}

#[test]
fn order_pin_mismatch_fails_loud_and_override_bypasses() {
    let tmp = TempDir::new().unwrap();
    write_workspace(
        tmp.path(),
        "[continuity]\ncanon_order_path = \"canon-order.json\"\n\
         canon_order_sha256 = \"0000000000000000000000000000000000000000000000000000000000000000\"\n",
    );
    let out = run(tmp.path(), &["validate-continuity"]);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("sha256 mismatch"), "{stderr}");
    // --order bypasses the pin by design (the pin claims nothing about a
    // different file — the R428 --catalog rule).
    let out = run(
        tmp.path(),
        &["validate-continuity", "--order", "canon-order.json"],
    );
    assert!(out.status.success(), "{:?}", out);
}

#[test]
fn undeclared_order_surfaces_unordered_never_gates() {
    let tmp = TempDir::new().unwrap();
    // Table present (reject) but NO canon_order_path: same-frame conflict on
    // distinct coordinates is not comparable -> unordered count, exit 0.
    write_workspace_with_frames(tmp.path(), "[continuity]\n", "seward");
    let out = run(tmp.path(), &["validate-continuity", "--json"]);
    assert!(out.status.success(), "{:?}", out);
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json output");
    assert_eq!(v["violation_count"], 0);
    assert_eq!(v["unordered_pairs"], 1);
}
