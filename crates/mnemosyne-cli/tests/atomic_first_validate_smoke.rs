//! Round 169 — dogfood-switch atomic-first validate-workspace.
//!
//! Verifies the atomic store is now a first-class workspace artifact:
//! validate-workspace surfaces `atomic ledger:` line + bails when atomic
//! invariants are violated (cross-ref orphan, GENERATED.md stale).

use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn cli_binary() -> &'static str {
 env!("CARGO_BIN_EXE_mnemosyne-cli")
}

fn write_min_workspace_config(workspace: &std::path::Path) {
 let cfg = r#"
[workspace]
docs = ["docs/STUB.md"]
default_doc = "docs/STUB.md"
"#;
 fs::create_dir_all(workspace.join("docs")).unwrap();
 fs::write(workspace.join("mnemosyne.toml"), cfg).unwrap();
 let stub = "# Stub\n\n## 1. Top\n\nbody.\n";
 fs::write(workspace.join("docs/STUB.md"), stub).unwrap();
}

#[test]
fn validate_workspace_surfaces_atomic_ledger_line() {
 let tmp = TempDir::new().unwrap();
 write_min_workspace_config(tmp.path());

 // Empty atomic store + no GENERATED.md = trivially in sync.
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
fn validate_workspace_passes_when_atomic_in_sync() {
 let tmp = TempDir::new().unwrap();
 write_min_workspace_config(tmp.path());

 let changes_path = tmp.path().join("changes.txt");
 fs::write(&changes_path, "x\n").unwrap();

 // Append entry (auto-regenerates GENERATED.md per Round 168 wire).
 Command::new(cli_binary())
 .args([
 "append-changelog-entry-v2",
 "--entry-id",
 "Round 999",
 "--decision",
 "atomic-first sync test",
 "--changes-file",
 changes_path.to_str().unwrap(),
 "--impact",
 "1",
 ])
 .current_dir(tmp.path())
 .output()
 .expect("run mutate");

 // validate-workspace must pass — atomic store + GENERATED.md in sync.
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
 assert!(stdout.contains("entries=1"), "expected entries=1; got: {}", stdout);
 assert!(stdout.contains("orphan_refs=0+0"), "expected 0 orphans; got: {}", stdout);
 assert!(stdout.contains("GENERATED.md=sync"), "expected sync; got: {}", stdout);
}

#[test]
fn validate_workspace_rejects_atomic_orphan_ref() {
 let tmp = TempDir::new().unwrap();
 write_min_workspace_config(tmp.path());

 let changes_path = tmp.path().join("changes.txt");
 fs::write(&changes_path, "x\n").unwrap();

 // Append entry with impact_ref to a non-existent section §99.
 Command::new(cli_binary())
 .args([
 "append-changelog-entry-v2",
 "--entry-id",
 "Round 999",
 "--decision",
 "orphan ref test",
 "--changes-file",
 changes_path.to_str().unwrap(),
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
 combined.contains("atomic store cross-ref orphan")
 || combined.contains("orphan_refs=1"),
 "expected atomic orphan diagnostic; got: {}",
 combined
 );
}

#[test]
fn validate_workspace_rejects_stale_generated_md() {
 let tmp = TempDir::new().unwrap();
 write_min_workspace_config(tmp.path());

 let changes_path = tmp.path().join("changes.txt");
 fs::write(&changes_path, "x\n").unwrap();

 // Initial mutate (auto-regen).
 Command::new(cli_binary())
 .args([
 "append-changelog-entry-v2",
 "--entry-id",
 "Round 998",
 "--decision",
 "first",
 "--changes-file",
 changes_path.to_str().unwrap(),
 "--impact",
 "1",
 ])
 .current_dir(tmp.path())
 .output()
 .expect("first mutate");

 // Second mutate with --no-regenerate (sidecar updated, GENERATED stale).
 fs::write(&changes_path, "y\n").unwrap();
 Command::new(cli_binary())
 .args([
 "append-changelog-entry-v2",
 "--entry-id",
 "Round 999",
 "--decision",
 "second-no-regen",
 "--changes-file",
 changes_path.to_str().unwrap(),
 "--impact",
 "1",
 "--no-regenerate",
 ])
 .current_dir(tmp.path())
 .output()
 .expect("second mutate");

 // validate-workspace must fail — GENERATED.md stale.
 let out = Command::new(cli_binary())
 .arg("validate-workspace")
 .current_dir(tmp.path())
 .output()
 .expect("run validate-workspace");
 assert!(
 !out.status.success(),
 "validate-workspace must reject stale GENERATED.md; stdout={}, stderr={}",
 String::from_utf8_lossy(&out.stdout),
 String::from_utf8_lossy(&out.stderr)
 );
 let combined = format!(
 "{}{}",
 String::from_utf8_lossy(&out.stdout),
 String::from_utf8_lossy(&out.stderr)
 );
 assert!(
 combined.contains("GENERATED.md stale") || combined.contains("GENERATED.md=STALE"),
 "expected stale diagnostic; got: {}",
 combined
 );
}
