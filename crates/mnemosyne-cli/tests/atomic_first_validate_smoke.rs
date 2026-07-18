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
fn validate_workspace_rejects_unregistered_entity_kind() {
    // R675 — entity-kind integrity is a baseline-gate invariant, not only a
    // validate-continuity boundary check. The write path (`add_entity`) cannot
    // produce an unregistered kind, so the defect state is reached OUT OF BAND:
    // a pre-v24 store whose kinds were never registered, or a hand edit. Here
    // we build a valid store then drop the kind registration from the sidecar,
    // leaving `ent-x`'s kind `place` dangling — validate-workspace must fail.
    let tmp = TempDir::new().unwrap();
    write_min_workspace_config(tmp.path());

    // Valid store: register `place`, then an entity of that kind.
    Command::new(cli_binary())
        .args(["add-entity-kind", "--kind", "place"])
        .current_dir(tmp.path())
        .output()
        .expect("add-entity-kind");
    Command::new(cli_binary())
        .args(["add-entity", "--entity", "ent-x", "--kind", "place"])
        .current_dir(tmp.path())
        .output()
        .expect("add-entity");

    // Out-of-band drift: remove `place` from the entity_kinds registry, leaving
    // the entity's kind dangling (the half-migration a map adopter could hit).
    let sidecar = tmp.path().join("docs/.atomic/workspace.atomic.json");
    let mut store: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&sidecar).unwrap()).unwrap();
    store["entity_kinds"]
        .as_object_mut()
        .unwrap()
        .remove("place");
    fs::write(&sidecar, serde_json::to_string_pretty(&store).unwrap()).unwrap();

    let out = Command::new(cli_binary())
        .arg("validate-workspace")
        .current_dir(tmp.path())
        .output()
        .expect("run validate-workspace");
    assert!(
        !out.status.success(),
        "validate-workspace must reject an unregistered entity kind; stdout={}, stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        combined.contains("store registry integrity")
            && combined.contains("ent-x")
            && combined.contains("place"),
        "expected store registry integrity diagnostic naming ent-x/place; got: {}",
        combined
    );
}

#[test]
fn validate_workspace_passes_with_registered_entity_kind() {
    // The clean counterpart: a registered kind + an entity of that kind must
    // pass, so the R675 gate is not a blanket reject (non-vacuous both ways).
    let tmp = TempDir::new().unwrap();
    write_min_workspace_config(tmp.path());

    Command::new(cli_binary())
        .args(["add-entity-kind", "--kind", "place"])
        .current_dir(tmp.path())
        .output()
        .expect("add-entity-kind");
    Command::new(cli_binary())
        .args(["add-entity", "--entity", "ent-x", "--kind", "place"])
        .current_dir(tmp.path())
        .output()
        .expect("add-entity");

    let out = Command::new(cli_binary())
        .arg("validate-workspace")
        .current_dir(tmp.path())
        .output()
        .expect("run validate-workspace");
    assert!(
        out.status.success(),
        "validate-workspace must pass with a registered kind; stdout={}, stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("store registry integrity: 0 out-of-band violation(s)"),
        "expected clean store registry integrity line; got: {}",
        stdout
    );
}

#[test]
fn report_entity_kind_migration_lists_the_worklist() {
    // R679 — the migration verb the cost-audit found missing: the complete
    // worklist of unregistered kinds a pre-registry / out-of-band store needs.
    let tmp = TempDir::new().unwrap();
    write_min_workspace_config(tmp.path());

    Command::new(cli_binary())
        .args(["add-entity-kind", "--kind", "place"])
        .current_dir(tmp.path())
        .output()
        .expect("add-entity-kind");
    Command::new(cli_binary())
        .args(["add-entity", "--entity", "ent-x", "--kind", "place"])
        .current_dir(tmp.path())
        .output()
        .expect("add-entity");

    // Clean: every in-use kind is registered.
    let clean = Command::new(cli_binary())
        .arg("report-entity-kind-migration")
        .current_dir(tmp.path())
        .output()
        .expect("run report-entity-kind-migration");
    assert!(clean.status.success());
    assert!(
        String::from_utf8_lossy(&clean.stdout).contains("0 unregistered kinds"),
        "expected clean worklist; got: {}",
        String::from_utf8_lossy(&clean.stdout)
    );

    // Out-of-band drift: drop `place` from the registry, leaving ent-x dangling.
    let sidecar = tmp.path().join("docs/.atomic/workspace.atomic.json");
    let mut store: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&sidecar).unwrap()).unwrap();
    store["entity_kinds"]
        .as_object_mut()
        .unwrap()
        .remove("place");
    fs::write(&sidecar, serde_json::to_string_pretty(&store).unwrap()).unwrap();

    let out = Command::new(cli_binary())
        .args(["report-entity-kind-migration", "--json"])
        .current_dir(tmp.path())
        .output()
        .expect("run report-entity-kind-migration");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("\"kind\":\"place\"") && stdout.contains("ent-x"),
        "expected place/ent-x in the worklist; got: {}",
        stdout
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
