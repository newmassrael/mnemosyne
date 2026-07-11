//! Round 393 — `import-epub-anchors` subcommand smoke test.
//!
//! Ingests a medium-forge `epub-anchor-map/v1` file and sets each matching
//! Section's `epub_locator` (EPUB-SSOT pointer). Scope:
//! (i) a matching anchor sets the locator (fragment + cfi) in the store
//! (ii) an anchor id absent from the store is skipped (not an error) and noted

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
        "[workspace]\n\
         [schema]\nentry_id_prefix = \"Round \"\n",
    )
    .unwrap();
    let store = serde_json::json!({
        "schema_version": 7,
        "sections": { "scxml-3.13": {
            "title": "Selecting Transitions", "parent_doc": "docs/GENERATED.md"
        }},
        "changelog_entries": {}
    });
    fs::write(
        ws.join("docs/.atomic/workspace.atomic.json"),
        serde_json::to_string_pretty(&store).unwrap(),
    )
    .unwrap();
    fs::write(ws.join("docs/GENERATED.md"), "# Stub\n").unwrap();
}

#[test]
fn import_epub_anchors_sets_locator_and_skips_unmatched() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    let anchors = serde_json::json!({
        "schema": "epub-anchor-map/v1",
        "epub": { "path": "spec.epub", "revision": "REC-scxml-20150901" },
        "anchors": [
            { "id": "scxml-3.13", "locator": {
                "spine_href": "OEBPS/spec.xhtml", "fragment": "scxml-3.13",
                "cfi": "epubcfi(/6/4!/4)" } },
            { "id": "scxml-absent", "locator": {
                "spine_href": "OEBPS/spec.xhtml", "fragment": "scxml-absent" } }
        ]
    });
    let apath = tmp.path().join("anchors.json");
    fs::write(&apath, serde_json::to_string(&anchors).unwrap()).unwrap();

    let out = Command::new(cli())
        .args(["import-epub-anchors", "--anchors", apath.to_str().unwrap()])
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
        String::from_utf8_lossy(&out.stderr).contains("matched no section"),
        "expected unmatched note; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let reloaded: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(tmp.path().join("docs/.atomic/workspace.atomic.json")).unwrap(),
    )
    .unwrap();
    let loc = &reloaded["sections"]["scxml-3.13"]["epub_locator"];
    assert_eq!(loc["fragment"], "scxml-3.13");
    assert_eq!(loc["spine_href"], "OEBPS/spec.xhtml");
    assert_eq!(loc["cfi"], "epubcfi(/6/4!/4)");
    // unmatched id created nothing
    assert!(reloaded["sections"].get("scxml-absent").is_none());
}
