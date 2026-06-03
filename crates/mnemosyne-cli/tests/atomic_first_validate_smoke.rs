//! Round 169 — dogfood-switch atomic-first validate-workspace.
//!
//! Verifies the atomic store is now a first-class workspace artifact:
//! validate-workspace surfaces `atomic ledger:` line + bails when atomic
//! invariants are violated (cross-ref orphan, superseded_by orphan).

use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn cli_binary() -> &'static str {
    env!("CARGO_BIN_EXE_mnemosyne-cli")
}

fn write_min_workspace_config(workspace: &std::path::Path) {
    // Store-only workspace (R400): no markdown docs. The atomic sidecar
    // (docs/.atomic/workspace.atomic.json) is created on first mutate.
    fs::create_dir_all(workspace.join("docs")).unwrap();
    fs::write(workspace.join("mnemosyne.toml"), "[workspace]\n").unwrap();
}

/// Create section `id` in the store so changelog impact_refs / superseded_by
/// pointers can resolve against it.
fn add_section(workspace: &std::path::Path, id: &str) {
    Command::new(cli_binary())
        .args([
            "add-section",
            "--section",
            id,
            "--parent-doc",
            "spec",
            "--title",
            "Top",
        ])
        .current_dir(workspace)
        .output()
        .expect("add-section");
}

#[test]
fn validate_workspace_surfaces_atomic_ledger_line() {
    let tmp = TempDir::new().unwrap();
    write_min_workspace_config(tmp.path());

    // Empty atomic store = trivially clean (no orphans, no violations).
    let out = Command::new(cli_binary())
        .arg("validate-workspace")
        .current_dir(tmp.path())
        .output()
        .expect("run validate-workspace");
    assert!(
        out.status.success(),
        "validate-workspace failed: stdout={}, stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("atomic ledger:"),
        "validate-workspace missing atomic ledger surface line; got: {}",
        stdout
    );
    assert!(
        stdout.contains("entries=0"),
        "empty atomic store must report entries=0; got: {}",
        stdout
    );
}

#[test]
fn validate_workspace_passes_on_clean_store() {
    let tmp = TempDir::new().unwrap();
    write_min_workspace_config(tmp.path());

    let changes_path = tmp.path().join("changes.txt");
    fs::write(&changes_path, "x\n").unwrap();
    let verify_path = tmp.path().join("verify.txt");
    fs::write(&verify_path, "v\n").unwrap();

    // §1 must exist in the store for the entry's impact_ref to resolve.
    add_section(tmp.path(), "1");

    // Append entry impacting §1.
    Command::new(cli_binary())
        .args([
            "append-changelog-entry",
            "--entry-id",
            "Round 999",
            "--decision",
            "atomic-first sync test",
            "--changes-file",
            changes_path.to_str().unwrap(),
            "--verification-file",
            verify_path.to_str().unwrap(),
            "--impact",
            "1",
        ])
        .current_dir(tmp.path())
        .output()
        .expect("run mutate");

    // validate-workspace must pass — store is clean (impact_ref §1 resolves).
    let out = Command::new(cli_binary())
        .arg("validate-workspace")
        .current_dir(tmp.path())
        .output()
        .expect("run validate-workspace");
    assert!(
        out.status.success(),
        "validate-workspace must pass when atomic in sync; stdout={}, stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("entries=1"),
        "expected entries=1; got: {}",
        stdout
    );
    assert!(
        stdout.contains("orphan_refs=0+0"),
        "expected 0 orphans; got: {}",
        stdout
    );
}

#[test]
fn validate_workspace_rejects_atomic_orphan_ref() {
    let tmp = TempDir::new().unwrap();
    write_min_workspace_config(tmp.path());

    let changes_path = tmp.path().join("changes.txt");
    fs::write(&changes_path, "x\n").unwrap();
    let verify_path = tmp.path().join("verify.txt");
    fs::write(&verify_path, "v\n").unwrap();

    // Append entry with impact_ref to a non-existent section §99.
    Command::new(cli_binary())
        .args([
            "append-changelog-entry",
            "--entry-id",
            "Round 999",
            "--decision",
            "orphan ref test",
            "--changes-file",
            changes_path.to_str().unwrap(),
            "--verification-file",
            verify_path.to_str().unwrap(),
            "--impact",
            "99",
        ])
        .current_dir(tmp.path())
        .output()
        .expect("run mutate");

    // validate-workspace must fail — §99 not in workspace.
    let out = Command::new(cli_binary())
        .arg("validate-workspace")
        .current_dir(tmp.path())
        .output()
        .expect("run validate-workspace");
    assert!(
        !out.status.success(),
        "validate-workspace must reject atomic orphan; stdout={}, stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        combined.contains("atomic store cross-ref orphan") || combined.contains("orphan_refs=1"),
        "expected atomic orphan diagnostic; got: {}",
        combined
    );
}

#[test]
fn validate_workspace_rejects_superseded_by_orphan() {
    // R344: a Superseded section's superseded_by forward-pointer is a section
    // cross-ref whose target must resolve. §1 superseded by a non-existent §99
    // satisfies the supersede-state gate (a pointer is set) but must be caught
    // as an orphan — the existence check the setter defers to validate-workspace.
    let tmp = TempDir::new().unwrap();
    write_min_workspace_config(tmp.path());

    Command::new(cli_binary())
        .args([
            "add-section",
            "--section",
            "1",
            "--parent-doc",
            "docs/STUB.md",
            "--title",
            "Top",
        ])
        .current_dir(tmp.path())
        .output()
        .expect("add-section");
    Command::new(cli_binary())
        .args([
            "set-section-decision-status",
            "--section",
            "1",
            "--status",
            "superseded",
            "--superseding",
            "99",
        ])
        .current_dir(tmp.path())
        .output()
        .expect("set-section-decision-status");

    let out = Command::new(cli_binary())
        .arg("validate-workspace")
        .current_dir(tmp.path())
        .output()
        .expect("run validate-workspace");
    assert!(
        !out.status.success(),
        "validate-workspace must reject a superseded_by orphan; stdout={}, stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        combined.contains("atomic orphan") || combined.contains("orphan_refs=0+1"),
        "expected superseded_by orphan diagnostic; got: {}",
        combined
    );
}
