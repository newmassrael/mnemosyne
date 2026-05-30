//! Smoke test for `generate-docs` CLI subcommand (Round 163 ratify, Phase 0f
//! forward-wire).
//!
//! End-to-end: append-changelog-entry → atomic store sidecar JSON →
//! generate-docs → GENERATED.md output. Runs against an isolated workspace
//! fixture (NOT the real workspace) to avoid mutating committed state.

use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn cli_binary() -> &'static str {
    env!("CARGO_BIN_EXE_mnemosyne-cli")
}

fn write_min_workspace_config(workspace: &std::path::Path) {
    // Minimal mnemosyne.toml — single doc workspace, nothing else.
    let cfg = r#"
[workspace]
docs = ["docs/STUB.md"]
default_doc = "docs/STUB.md"
"#;
    fs::create_dir_all(workspace.join("docs")).unwrap();
    fs::write(workspace.join("mnemosyne.toml"), cfg).unwrap();
    // Stub doc with the bare minimum to parse.
    let stub = "# Stub\n\n## §1. Top\n\nbody.\n";
    fs::write(workspace.join("docs/STUB.md"), stub).unwrap();
}

#[test]
fn generate_docs_emits_minimal_artifact_when_atomic_store_empty() {
    let tmp = TempDir::new().unwrap();
    write_min_workspace_config(tmp.path());

    let out = Command::new(cli_binary())
        .arg("generate-docs")
        .current_dir(tmp.path())
        .output()
        .expect("run mnemosyne-cli generate-docs");
    assert!(
        out.status.success(),
        "generate-docs failed: stdout={}, stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let generated = fs::read_to_string(tmp.path().join("docs/GENERATED.md")).unwrap();
    assert!(generated.contains("# GENERATED.md"));
    assert!(generated.contains("(empty — first atomic entry will populate this section.)"));
}

#[test]
fn generate_docs_emits_changelog_entry_after_append() {
    let tmp = TempDir::new().unwrap();
    write_min_workspace_config(tmp.path());

    // Write the bullet files.
    let changes_path = tmp.path().join("changes.txt");
    fs::write(&changes_path, "first change\nsecond change\n").unwrap();
    let verify_path = tmp.path().join("verify.txt");
    fs::write(&verify_path, "verified\n").unwrap();

    // append-changelog-entry.
    let out = Command::new(cli_binary())
        .args([
            "append-changelog-entry",
            "--entry-id",
            "Round 999",
            "--decision",
            "smoke test summary",
            "--changes-file",
            changes_path.to_str().unwrap(),
            "--verification-file",
            verify_path.to_str().unwrap(),
            "--impact",
            "§1",
        ])
        .current_dir(tmp.path())
        .output()
        .expect("run append-changelog-entry");
    assert!(
        out.status.success(),
        "append failed: stdout={}, stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    // generate-docs.
    let out = Command::new(cli_binary())
        .arg("generate-docs")
        .current_dir(tmp.path())
        .output()
        .expect("run generate-docs");
    assert!(out.status.success());

    let generated = fs::read_to_string(tmp.path().join("docs/GENERATED.md")).unwrap();
    assert!(
        generated.contains("### Round 999 — smoke test summary"),
        "GENERATED.md missing entry header; got: {}",
        generated
    );
    assert!(generated.contains("- first change"));
    assert!(generated.contains("- second change"));
    assert!(generated.contains("- verified"));
    assert!(generated.contains("**Impact**: §1"));
}

#[test]
fn append_changelog_entry_rejects_duplicate() {
    let tmp = TempDir::new().unwrap();
    write_min_workspace_config(tmp.path());

    let changes_path = tmp.path().join("changes.txt");
    fs::write(&changes_path, "x\n").unwrap();
    let verify_path = tmp.path().join("verify.txt");
    fs::write(&verify_path, "v\n").unwrap();

    // First append succeeds.
    let out = Command::new(cli_binary())
        .args([
            "append-changelog-entry",
            "--entry-id",
            "Round 999",
            "--decision",
            "first",
            "--changes-file",
            changes_path.to_str().unwrap(),
            "--verification-file",
            verify_path.to_str().unwrap(),
        ])
        .current_dir(tmp.path())
        .output()
        .expect("run first append");
    assert!(out.status.success());

    // Second append to same id must fail (T2 frozen ledger semantics).
    let out = Command::new(cli_binary())
        .args([
            "append-changelog-entry",
            "--entry-id",
            "Round 999",
            "--decision",
            "second",
            "--changes-file",
            changes_path.to_str().unwrap(),
            "--verification-file",
            verify_path.to_str().unwrap(),
        ])
        .current_dir(tmp.path())
        .output()
        .expect("run second append");
    assert!(
        !out.status.success(),
        "second append must fail; stdout={}, stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("frozen") || stderr.contains("FrozenLedger"),
        "expected frozen-ledger error, got: {}",
        stderr
    );
}

#[test]
fn generate_docs_renders_normative_excerpt_block() {
    // RFC-001 UC-1 §4 read-path — a Section carrying a normative_excerpt
    // renders the excerpt block (rev + verbatim text + source URL) into
    // GENERATED.md. Cold render path.
    let tmp = TempDir::new().unwrap();
    write_min_workspace_config(tmp.path());
    fs::create_dir_all(tmp.path().join("docs/.atomic")).unwrap();
    let atomic = serde_json::json!({
    "schema_version": 4,
    "sections": {
    "scxml-3.13": {
    "title": "Event Descriptors",
    "parent_doc": "docs/GENERATED.md",
    "decision_status": "active",
    "normative_excerpt": {
    "text": "An event descriptor matches the event name verbatim.",
    "anchor_url": "https://www.w3.org/TR/scxml/#event",
    "source_revision": "2024-rec"
    }
    }
    },
    "changelog_entries": {}
    });
    fs::write(
        tmp.path().join("docs/.atomic/workspace.atomic.json"),
        serde_json::to_string_pretty(&atomic).unwrap(),
    )
    .unwrap();

    let out = Command::new(cli_binary())
        .arg("generate-docs")
        .current_dir(tmp.path())
        .output()
        .expect("run generate-docs");
    assert!(
        out.status.success(),
        "generate-docs failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let generated = fs::read_to_string(tmp.path().join("docs/GENERATED.md")).unwrap();
    assert!(
        generated.contains("**Normative excerpt** (2024-rec):"),
        "missing excerpt heading; got:\n{}",
        generated
    );
    assert!(
        generated.contains("An event descriptor matches the event name verbatim."),
        "missing verbatim excerpt text; got:\n{}",
        generated
    );
    assert!(
        generated.contains("**Source**: https://www.w3.org/TR/scxml/#event"),
        "missing source anchor; got:\n{}",
        generated
    );
}
