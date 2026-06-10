//! R426 — `validate-verifies-linkage` smoke tests (SCE field-report P2 + P5).
//!
//! (i)   opt-in: no `[verifies_catalog]` and no `--catalog` → disabled, exit 0.
//! (ii)  a cross-section mismatch rejects (exit 1) and is classified `cross`;
//!       a finer-than-declared binding is classified `finer_than_declared`
//!       (the P5 granularity lint); an uncataloged artifact is counted, never
//!       gating.
//! (iii) a fully-matching ledger passes (exit 0, mismatch_count 0).

use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

fn cli() -> &'static str {
    env!("CARGO_BIN_EXE_mnemosyne-cli")
}

fn write_workspace(ws: &Path, bound_section: &str) {
    fs::create_dir_all(ws.join("docs/.atomic")).unwrap();
    fs::write(
        ws.join("mnemosyne.toml"),
        "[workspace]\n[schema]\nentry_id_prefix = \"Round \"\n",
    )
    .unwrap();
    let atomic = serde_json::json!({
        "schema_version": 11,
        "sections": {
            bound_section: { "title": "T", "parent_doc": "d",
                "bindings": [{ "file": "t/Test215.h", "kind": "verifies" }] },
            "5.1": { "title": "U", "parent_doc": "d",
                "bindings": [{ "file": "t/Test999.h", "kind": "verifies" }] }
        },
        "changelog_entries": {}
    });
    fs::write(
        ws.join("docs/.atomic/workspace.atomic.json"),
        serde_json::to_string_pretty(&atomic).unwrap(),
    )
    .unwrap();
    // Catalog declares Test215 → 6.4 only; Test999 is uncataloged.
    let catalog = serde_json::json!({
        "schema": "verifies-catalog/v1",
        "entries": [ { "file": "t/Test215.h", "section_ids": ["6.4"] } ]
    });
    fs::write(
        ws.join("verifies-catalog.json"),
        serde_json::to_string_pretty(&catalog).unwrap(),
    )
    .unwrap();
}

fn run(ws: &Path, args: &[&str]) -> (Output, Option<serde_json::Value>) {
    let out = Command::new(cli())
        .args(args)
        .current_dir(ws)
        .output()
        .unwrap();
    let json = serde_json::from_slice(&out.stdout).ok();
    (out, json)
}

#[test]
fn disabled_without_catalog() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path(), "6.4");
    let (out, json) = run(tmp.path(), &["validate-verifies-linkage", "--json"]);
    assert!(out.status.success(), "no catalog configured → exit 0");
    assert_eq!(json.unwrap()["enabled"], false);
}

#[test]
fn cross_mismatch_rejects_and_uncataloged_never_gates() {
    let tmp = TempDir::new().unwrap();
    // Test215 bound to 3.13 while the catalog declares 6.4 → cross mismatch.
    write_workspace(tmp.path(), "3.13");
    let (out, json) = run(
        tmp.path(),
        &[
            "validate-verifies-linkage",
            "--catalog",
            "verifies-catalog.json",
            "--json",
        ],
    );
    assert!(
        !out.status.success(),
        "cross mismatch under reject → exit 1"
    );
    let j = json.unwrap();
    assert_eq!(j["mismatch_count"], 1);
    assert_eq!(j["mismatches"][0]["kind"], "cross");
    assert_eq!(j["uncataloged"], 1, "Test999 counted, not gating");
}

#[test]
fn finer_than_declared_is_the_granularity_lint() {
    let tmp = TempDir::new().unwrap();
    // Bound to 6.4.1, declared 6.4 → the blanket-enabling granularity claim.
    write_workspace(tmp.path(), "6.4.1");
    let (_out, json) = run(
        tmp.path(),
        &[
            "validate-verifies-linkage",
            "--catalog",
            "verifies-catalog.json",
            "--severity",
            "warn",
            "--json",
        ],
    );
    let j = json.unwrap();
    assert_eq!(j["mismatch_count"], 1);
    assert_eq!(j["mismatches"][0]["kind"], "finer_than_declared");
}

#[test]
fn exact_match_passes() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path(), "6.4");
    let (out, json) = run(
        tmp.path(),
        &[
            "validate-verifies-linkage",
            "--catalog",
            "verifies-catalog.json",
            "--json",
        ],
    );
    assert!(
        out.status.success(),
        "exact match passes; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(json.unwrap()["mismatch_count"], 0);
}
