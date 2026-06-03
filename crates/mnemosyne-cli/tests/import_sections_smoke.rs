//! RFC-001 UC-1 "A2" — `import-sections` bulk-create subcommand smoke tests.
//!
//! End-to-end: a JSON manifest → `import-sections --manifest` → atomic store.
//! Covers create, inline normative_excerpt, sigil-strip, idempotent re-run
//! (no-op), and divergent reject — all asserted against the store JSON.

use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn cli_binary() -> &'static str {
    env!("CARGO_BIN_EXE_mnemosyne-cli")
}

fn write_workspace(workspace: &Path) {
    fs::create_dir_all(workspace.join("docs/.atomic")).unwrap();
    let cfg = "[workspace]\ndocs = [\"docs/GENERATED.md\"]\n\
 default_doc = \"docs/GENERATED.md\"\n";
    fs::write(workspace.join("mnemosyne.toml"), cfg).unwrap();
    let atomic = serde_json::json!({
    "schema_version": 4, "sections": {}, "changelog_entries": {}
    });
    fs::write(
        workspace.join("docs/.atomic/workspace.atomic.json"),
        serde_json::to_string_pretty(&atomic).unwrap(),
    )
    .unwrap();
    fs::write(workspace.join("docs/GENERATED.md"), "# Stub\n").unwrap();
}

fn run(workspace: &Path, args: &[&str]) -> std::process::Output {
    Command::new(cli_binary())
        .args(args)
        .current_dir(workspace)
        .output()
        .expect("cli exec")
}

fn write_manifest(workspace: &Path, value: serde_json::Value) -> String {
    let p = workspace.join("manifest.json");
    fs::write(&p, serde_json::to_string_pretty(&value).unwrap()).unwrap();
    p.to_str().unwrap().to_string()
}

/// Read the atomic store JSON the import wrote.
fn read_store(workspace: &Path) -> serde_json::Value {
    let raw = fs::read_to_string(workspace.join("docs/.atomic/workspace.atomic.json")).unwrap();
    serde_json::from_str(&raw).unwrap()
}

#[test]
fn import_creates_sections_and_excerpt() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    let manifest = write_manifest(
        tmp.path(),
        serde_json::json!([
        { "section_id": "scxml-3.13", "parent_doc": "docs/GENERATED.md",
          "title": "Event Descriptors",
          "normative_excerpt": {
          "text": "An event descriptor matches the event name verbatim.",
          "anchor_url": "https://www.w3.org/TR/scxml/#event",
          "source_revision": "2024-rec" } },
        { "section_id": "scxml-5.10", "parent_doc": "docs/GENERATED.md",
          "title": "Datamodel" }
        ]),
    );
    let out = run(tmp.path(), &["import-sections", "--manifest", &manifest]);
    assert!(
        out.status.success(),
        "import failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let store = read_store(tmp.path());
    let sections = &store["sections"];
    assert!(
        sections.get("scxml-3.13").is_some(),
        "store missing scxml-3.13; {store}"
    );
    assert!(sections.get("scxml-5.10").is_some());
    assert_eq!(
        sections["scxml-3.13"]["normative_excerpt"]["source_revision"], "2024-rec",
        "inline excerpt not stored; {store}"
    );
}

#[test]
fn import_is_idempotent_on_rerun() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    let manifest = write_manifest(
        tmp.path(),
        serde_json::json!([
        { "section_id": "scxml-3.13", "parent_doc": "docs/GENERATED.md", "title": "Event Descriptors" }
        ]),
    );
    assert!(
        run(tmp.path(), &["import-sections", "--manifest", &manifest])
            .status
            .success()
    );
    // Re-run with the same manifest → all no-op, exit 0.
    let out = run(
        tmp.path(),
        &["import-sections", "--manifest", &manifest, "--json"],
    );
    assert!(
        out.status.success(),
        "idempotent re-run must exit 0: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("0 created"),
        "re-run should be no-op; {stdout}"
    );
}

#[test]
fn import_strips_section_sigil_no_double() {
    // SCE-found bug: a citation-form manifest (`§scxml-1`) must be stored under
    // the bare key `scxml-1`, never the sigil-prefixed `§scxml-1`.
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    let manifest = write_manifest(
        tmp.path(),
        serde_json::json!([
        { "section_id": "§scxml-1", "parent_doc": "docs/GENERATED.md", "title": "A" }
        ]),
    );
    assert!(
        run(tmp.path(), &["import-sections", "--manifest", &manifest])
            .status
            .success()
    );
    let store = read_store(tmp.path());
    let sections = &store["sections"];
    assert!(
        sections.get("scxml-1").is_some(),
        "sigil not stripped to bare key; {store}"
    );
    assert!(
        sections.get("§scxml-1").is_none(),
        "sigil-prefixed key leaked into store; {store}"
    );
}

#[test]
fn import_rejects_divergent_section() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    let m1 = write_manifest(
        tmp.path(),
        serde_json::json!([
        { "section_id": "scxml-3.13", "parent_doc": "docs/GENERATED.md", "title": "Original" }
        ]),
    );
    assert!(run(tmp.path(), &["import-sections", "--manifest", &m1])
        .status
        .success());
    // Same id, different title → divergent → reject (exit 1).
    let p = tmp.path().join("manifest2.json");
    fs::write(
        &p,
        serde_json::to_string_pretty(&serde_json::json!([
        { "section_id": "scxml-3.13", "parent_doc": "docs/GENERATED.md", "title": "Changed" }
        ]))
        .unwrap(),
    )
    .unwrap();
    let out = run(
        tmp.path(),
        &["import-sections", "--manifest", p.to_str().unwrap()],
    );
    assert!(
        !out.status.success(),
        "divergent import must exit 1: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("DIVERGENT"),
        "stderr should name the divergence: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}
