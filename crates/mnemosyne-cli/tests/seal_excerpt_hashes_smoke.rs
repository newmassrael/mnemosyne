//! Round 406 — `seal-excerpt-hashes` subcommand smoke test.
//!
//! Stamps `text_sha256 = sha256(text)` on excerpts whose hash is empty, sealing
//! the already-stored (consumer-authored) text as its own revalidation baseline
//! — the non-EPUB complement of `import-epub-excerpts`. Scope:
//! (i) an empty-hash excerpt is sealed; `text` is untouched, the hash is filled
//! (ii) after sealing, `validate-content-drift` is clean and
//!      `report-excerpt-hash-backfill` shows 0 backlog
//! (iii) a populated hash is left untouched (not re-sealed)

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
        "schema_version": 8,
        "sections": {
            // consumer-authored excerpt, empty hash (the import_sections path)
            "authored": { "title": "Authored", "parent_doc": "docs/spec.epub",
                "normative_excerpt": {
                    "text": "consumer direct-body text",
                    "anchor_url": "https://www.w3.org/TR/scxml/#x",
                    "source_revision": "REC-scxml-20150901",
                    "text_sha256": "" } },
            // already-sealed-but-wrong hash → must be LEFT (drift, not re-sealed)
            "wrong": { "title": "Wrong", "parent_doc": "docs/spec.epub",
                "normative_excerpt": {
                    "text": "tampered text",
                    "anchor_url": "https://www.w3.org/TR/scxml/#y",
                    "source_revision": "REC-scxml-20150901",
                    "text_sha256": "deadbeef" } }
        },
        "changelog_entries": {}
    });
    fs::write(
        ws.join("docs/.atomic/workspace.atomic.json"),
        serde_json::to_string_pretty(&store).unwrap(),
    )
    .unwrap();
}

fn run(ws: &Path, args: &[&str]) -> std::process::Output {
    Command::new(cli())
        .args(args)
        .current_dir(ws)
        .output()
        .expect("cli exec")
}

#[test]
fn seal_fills_empty_hash_and_leaves_wrong_hash() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());

    let out = run(tmp.path(), &["seal-excerpt-hashes"]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("sealed 1 excerpt"),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let reloaded: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(tmp.path().join("docs/.atomic/workspace.atomic.json")).unwrap(),
    )
    .unwrap();
    let authored = &reloaded["sections"]["authored"]["normative_excerpt"];
    // text untouched, hash now populated
    assert_eq!(authored["text"], "consumer direct-body text");
    assert_eq!(authored["text_sha256"].as_str().unwrap().len(), 64);
    // the wrong/populated hash was NOT re-sealed (left as drift)
    assert_eq!(
        reloaded["sections"]["wrong"]["normative_excerpt"]["text_sha256"],
        "deadbeef"
    );

    // report shows only the still-unrevalidatable... none now lack a hash, but
    // the "wrong" one is a drift (populated), so backfill report = 0.
    let report = run(tmp.path(), &["report-excerpt-hash-backfill", "--json"]);
    let rv: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
    assert_eq!(rv["rows"].as_array().unwrap().len(), 0);
}

#[test]
fn sealed_excerpt_passes_content_drift() {
    let tmp = TempDir::new().unwrap();
    // store with ONLY the authored (sealable) excerpt — no tampered one.
    fs::create_dir_all(tmp.path().join("docs/.atomic")).unwrap();
    fs::write(
        tmp.path().join("mnemosyne.toml"),
        "[workspace]\n[schema]\nentry_id_prefix = \"Round \"\n",
    )
    .unwrap();
    let store = serde_json::json!({
        "schema_version": 8,
        "sections": {
            "authored": { "title": "Authored", "parent_doc": "docs/spec.epub",
                "normative_excerpt": {
                    "text": "consumer direct-body text",
                    "anchor_url": "https://www.w3.org/TR/scxml/#x",
                    "source_revision": "REC-scxml-20150901",
                    "text_sha256": "" } }
        },
        "changelog_entries": {}
    });
    fs::write(
        tmp.path().join("docs/.atomic/workspace.atomic.json"),
        serde_json::to_string_pretty(&store).unwrap(),
    )
    .unwrap();

    assert!(run(tmp.path(), &["seal-excerpt-hashes"]).status.success());
    // default severity reject → must exit 0 because the sealed text matches its hash.
    let drift = run(tmp.path(), &["validate-content-drift"]);
    assert!(
        drift.status.success(),
        "sealed excerpt should pass content-drift; stderr: {}",
        String::from_utf8_lossy(&drift.stderr)
    );
}
