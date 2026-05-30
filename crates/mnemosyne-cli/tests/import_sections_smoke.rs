//! RFC-001 UC-1 "A2" — `import-sections` bulk-create subcommand smoke tests.
//!
//! End-to-end: a JSON manifest → `import-sections --manifest` → atomic store
//! + GENERATED.md. Covers create, inline normative_excerpt render, idempotent
//! re-run (no-op), and divergent reject.

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

#[test]
fn import_creates_sections_and_renders_excerpt() {
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
    let gen = fs::read_to_string(tmp.path().join("docs/GENERATED.md")).unwrap();
    assert!(
        gen.contains("§scxml-3.13"),
        "GENERATED.md missing section; {gen}"
    );
    assert!(gen.contains("§scxml-5.10"));
    assert!(
        gen.contains("**Normative excerpt** (2024-rec):"),
        "inline excerpt not rendered; {gen}"
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
