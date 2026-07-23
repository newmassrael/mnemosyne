//! Round 403 — `import-epub-excerpts` subcommand smoke test.
//!
//! Refreshes `normative_excerpt.text` + `text_sha256` from a medium-forge
//! `epub-anchor-map/v2`, preserving the section's authored `anchor_url` +
//! `source_revision`. Scope:
//! (i) a matching entry with a correct hash refreshes text + text_sha256 and
//!     preserves the authored identity fields
//! (ii) an id with no refreshable excerpt (absent section OR no existing
//!      excerpt) is skipped (not an error) and noted on stderr
//! (iii) a locator-only (v1-style, no text) entry is ignored

use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn cli() -> &'static str {
    env!("CARGO_BIN_EXE_mnemosyne-cli")
}

// sha256("new EPUB text"), the refreshed cache's revalidation anchor.
const NEW_TEXT: &str = "new EPUB text";
const NEW_HASH: &str = "30fdf0c37d22d1469a41daff8c3a9559abab9e7dbcb52af9fb831d7e9d7ede4c";

fn write_workspace(ws: &Path) {
    fs::create_dir_all(ws.join("docs/.atomic")).unwrap();
    fs::write(
        ws.join("mnemosyne.toml"),
        "[workspace]\n[schema]\nentry_id_prefix = \"Round \"\n",
    )
    .unwrap();
    let store = serde_json::json!({
        "schema_version": 8,
        "sections": { "scxml-3.13": {
            "title": "Selecting Transitions", "parent_doc": "docs/spec.epub",
            "normative_excerpt": {
                "text": "old text",
                "anchor_url": "https://www.w3.org/TR/scxml/#selecting-transitions",
                "source_revision": "REC-scxml-20150901",
                "text_sha256": ""
            }
        }},
        "changelog_entries": {}
    });
    fs::write(
        ws.join("docs/.atomic/workspace.atomic.json"),
        serde_json::to_string_pretty(&store).unwrap(),
    )
    .unwrap();
}

#[test]
fn import_epub_excerpts_refreshes_text_preserves_identity_and_skips_unmatched() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    let anchors = serde_json::json!({
        "schema": "epub-anchor-map/v2",
        "epub": { "path": "spec.epub", "revision": "REC-scxml-20150901" },
        "anchors": [
            // matching, refreshable
            { "id": "scxml-3.13", "text": NEW_TEXT, "text_sha256": NEW_HASH },
            // present-but-no-excerpt is not in this store; absent id → unmatched
            { "id": "scxml-absent", "text": "x", "text_sha256": "" },
            // v1-style locator-only entry (no text) → ignored
            { "id": "scxml-loc-only", "locator": {
                "spine_href": "OEBPS/spec.xhtml", "fragment": "scxml-loc-only" } }
        ]
    });
    let apath = tmp.path().join("anchors.json");
    fs::write(&apath, serde_json::to_string(&anchors).unwrap()).unwrap();

    let out = Command::new(cli())
        .args(["import-epub-excerpts", "--anchors", apath.to_str().unwrap()])
        .current_dir(tmp.path())
        .output()
        .expect("cli exec");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    // the absent id is reported as a note, not an error
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("matched no refreshable excerpt"),
        "expected unmatched note; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let reloaded: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(tmp.path().join("docs/.atomic/workspace.atomic.json")).unwrap(),
    )
    .unwrap();
    let ne = &reloaded["sections"]["scxml-3.13"]["normative_excerpt"];
    // R759 — the excerpt is now wrapped on the shared ContentExcerpt substrate;
    // the v8 flat fixture is migrated to v42 on load, then refreshed + saved.
    assert_eq!(ne["excerpt"]["text"], NEW_TEXT);
    assert_eq!(ne["excerpt"]["text_sha256"], NEW_HASH);
    // authored identity preserved (store-side, not EPUB-projected)
    assert_eq!(
        ne["anchor_url"],
        "https://www.w3.org/TR/scxml/#selecting-transitions"
    );
    assert_eq!(ne["source_revision"], "REC-scxml-20150901");
    // unmatched id created nothing
    assert!(reloaded["sections"].get("scxml-absent").is_none());
}
