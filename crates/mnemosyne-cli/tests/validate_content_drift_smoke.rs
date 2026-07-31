//! Round 404 — `validate-content-drift` subcommand smoke test.
//!
//! Offline re-hash of each `normative_excerpt.text` vs its `text_sha256`.
//! Scope:
//! (i) a populated hash that no longer matches the text = drift; default
//!     severity `reject` exits 1
//! (ii) `--severity warn` prints the drift but exits 0
//! (iii) the unrevalidatable (empty-hash) count is surfaced but never gates
//! (iv) `--json` reports drift_count + unrevalidatable_count
//! (v) Round 895 — a store carrying NO excerpt says so instead of rendering
//!     the same empty finding list a fully-checked clean store renders, and a
//!     store that does carry caches reports the reach behind its list

use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn cli() -> &'static str {
    env!("CARGO_BIN_EXE_mnemosyne-cli")
}

// sha256("clean text") — the matching cache stays clean.
const CLEAN_HASH: &str = "ca3e62efb17057490b07a8858d64a1437387841aff81fc7c56bf1114a13a0a6f";
// sha256("original text") recorded on a section whose text was later edited to
// "edited text" out-of-band → drift.
const STALE_HASH: &str = "b61e285b0ad77c1cbe7654c9b8a029e4a2effc33ca7164185bff1beb4b71156d";

fn excerpt(text: &str, hash: &str) -> serde_json::Value {
    serde_json::json!({
        "text": text,
        "anchor_url": "https://www.w3.org/TR/scxml/#x",
        "source_revision": "REC-scxml-20150901",
        "text_sha256": hash,
    })
}

fn write_workspace(ws: &Path) {
    fs::create_dir_all(ws.join("docs/.atomic")).unwrap();
    fs::write(
        ws.join("mnemosyne.toml"),
        "[workspace]\n[schema]\nentry_id_prefix = \"Round \"\n",
    )
    .unwrap();
    let store = serde_json::json!({
        "schema_version": 8,
        "sections": {
            "clean": { "title": "Clean", "parent_doc": "docs/spec.epub",
                "normative_excerpt": excerpt("clean text", CLEAN_HASH) },
            "drifted": { "title": "Drifted", "parent_doc": "docs/spec.epub",
                "normative_excerpt": excerpt("edited text", STALE_HASH) },
            "unrevalidatable": { "title": "Hand-authored", "parent_doc": "docs/spec.epub",
                "normative_excerpt": excerpt("hand authored", "") }
        },
        "changelog_entries": {}
    });
    fs::write(
        ws.join("docs/.atomic/workspace.atomic.json"),
        serde_json::to_string_pretty(&store).unwrap(),
    )
    .unwrap();
}

fn run(ws: &Path, extra: &[&str]) -> std::process::Output {
    let mut a = vec!["validate-content-drift"];
    a.extend_from_slice(extra);
    Command::new(cli())
        .args(a)
        .current_dir(ws)
        .output()
        .expect("cli exec")
}

#[test]
fn content_drift_default_reject_exits_nonzero() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    let out = run(tmp.path(), &[]);
    assert!(
        !out.status.success(),
        "default severity reject must exit 1 on drift; stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("content-integrity drift"),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn content_drift_warn_opts_out_of_gate() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    let out = run(tmp.path(), &["--severity", "warn"]);
    assert!(
        out.status.success(),
        "warn must exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("drift: total=1"), "stdout: {stdout}");
    // unrevalidatable surfaced, not gated
    assert!(stdout.contains("unrevalidatable=1"), "stdout: {stdout}");
}

/// A workspace whose sections carry no excerpt of any kind — the shape R894
/// measured on the dnd-quest corpus (56 sections, not one with prose).
fn write_excerptless_workspace(ws: &Path) {
    fs::create_dir_all(ws.join("docs/.atomic")).unwrap();
    fs::write(
        ws.join("mnemosyne.toml"),
        "[workspace]\n[schema]\nentry_id_prefix = \"Round \"\n",
    )
    .unwrap();
    let store = serde_json::json!({
        "schema_version": 8,
        "sections": {
            "a": { "title": "A", "parent_doc": "docs/spec.epub" },
            "b": { "title": "B", "parent_doc": "docs/spec.epub" }
        },
        "changelog_entries": {}
    });
    fs::write(
        ws.join("docs/.atomic/workspace.atomic.json"),
        serde_json::to_string_pretty(&store).unwrap(),
    )
    .unwrap();
}

#[test]
fn a_store_with_no_excerpts_says_it_compared_nothing() {
    // Round 895 — the two zeros. This store is clean in the sense that nothing
    // drifted, and it is also a store the scan could not check at all; before the
    // census both printed `drift: total=0` and stopped there.
    let tmp = TempDir::new().unwrap();
    write_excerptless_workspace(tmp.path());
    let out = run(tmp.path(), &[]);
    assert!(
        out.status.success(),
        "nothing to check must not gate; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("sections=2 with_excerpt=0"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("0 excerpt(s) examined") && stdout.contains("nothing to check"),
        "the scan must say which zero this is; stdout: {stdout}"
    );
}

#[test]
fn a_store_with_excerpts_reports_the_reach_behind_its_finding_list() {
    // The discriminating partner: the SAME verb on a store that does carry
    // caches must print a reach, not the `nothing to check` sentence. Two of the
    // three excerpts here are hashed; the third is unrevalidatable.
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    let out = run(tmp.path(), &["--severity", "warn"]);
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("sections=3 with_excerpt=3"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("drift: total=1 over 2 excerpt(s) examined"),
        "stdout: {stdout}"
    );
    assert!(
        !stdout.contains("nothing to check"),
        "a store with caches must not claim it compared nothing; stdout: {stdout}"
    );
}

#[test]
fn content_drift_json_reports_counts() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    // warn so the process exits 0 and we can read clean JSON
    let out = run(tmp.path(), &["--json", "--severity", "warn"]);
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["drift_count"], 1);
    assert_eq!(v["unrevalidatable_count"], 1);
    // Round 895 — the reach travels on the machine surface too, or a consumer
    // reading JSON is left with the ambiguity the text surface just lost.
    assert_eq!(v["sections_with_excerpt"], 3);
    assert_eq!(v["examined_count"], 2);
    assert_eq!(v["violations"][0]["section_id"], "drifted");
    assert_eq!(v["violations"][0]["declared_sha256"], STALE_HASH);
}
