//! Round 168 — cascade auto-update wire smoke tests.
//!
//! Verifies the three legs of the cascade auto-update wire:
//! 1. Atomic-mutate CLI auto-regenerates GENERATED.md by default.
//! 2. `--no-regenerate` flag skips the regenerate (batch mode)
//! 3. `verify-generated` subcommand reports sync vs stale via exit code
//!
//! Each test runs in an isolated temp workspace (no shared state).

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
 let stub = "# Stub\n\n## §1. Top\n\nbody.\n";
 fs::write(workspace.join("docs/STUB.md"), stub).unwrap();
}

#[test]
fn append_v2_auto_regenerates_generated_md() {
 let tmp = TempDir::new().unwrap();
 write_min_workspace_config(tmp.path());

 let changes_path = tmp.path().join("changes.txt");
 fs::write(&changes_path, "first change\n").unwrap();
 let verify_path = tmp.path().join("verify.txt");
 fs::write(&verify_path, "v\n").unwrap();

 // Pre-condition: GENERATED.md does not exist yet.
 assert!(
 !tmp.path().join("docs/GENERATED.md").exists(),
 "GENERATED.md must not exist before mutate"
 );

 // append-changelog-entry-v2 without --no-regenerate flag.
 let out = Command::new(cli_binary())
 .args([
 "append-changelog-entry-v2",
 "--entry-id",
 "Round 999",
 "--decision",
 "auto-regen smoke",
 "--changes-file",
 changes_path.to_str().unwrap(),
 "--verification-file",
 verify_path.to_str().unwrap(),
 ])
 .current_dir(tmp.path())
 .output()
 .expect("run append-changelog-entry-v2");
 assert!(
 out.status.success(),
 "append failed: stdout={}, stderr={}",
 String::from_utf8_lossy(&out.stdout),
 String::from_utf8_lossy(&out.stderr)
 );

 // Post-condition: GENERATED.md exists and contains the entry.
 let generated_path = tmp.path().join("docs/GENERATED.md");
 assert!(
 generated_path.exists(),
 "GENERATED.md must exist after mutate (cascade auto-update wire)"
 );
 let generated = fs::read_to_string(&generated_path).unwrap();
 assert!(
 generated.contains("### Round 999 — auto-regen smoke"),
 "GENERATED.md missing entry after auto-regenerate; got: {}",
 generated
 );
}

#[test]
fn append_v2_no_regenerate_skips_generated_md() {
 let tmp = TempDir::new().unwrap();
 write_min_workspace_config(tmp.path());

 let changes_path = tmp.path().join("changes.txt");
 fs::write(&changes_path, "x\n").unwrap();
 let verify_path = tmp.path().join("verify.txt");
 fs::write(&verify_path, "v\n").unwrap();

 let out = Command::new(cli_binary())
 .args([
 "append-changelog-entry-v2",
 "--entry-id",
 "Round 999",
 "--decision",
 "no-regen smoke",
 "--changes-file",
 changes_path.to_str().unwrap(),
 "--verification-file",
 verify_path.to_str().unwrap(),
 "--no-regenerate",
 ])
 .current_dir(tmp.path())
 .output()
 .expect("run append-changelog-entry-v2 --no-regenerate");
 assert!(
 out.status.success(),
 "append failed: stdout={}, stderr={}",
 String::from_utf8_lossy(&out.stdout),
 String::from_utf8_lossy(&out.stderr)
 );

 // Sidecar exists but GENERATED.md does not (skipped regenerate).
 let sidecar_path = tmp.path().join("docs/.atomic/workspace.atomic.json");
 assert!(sidecar_path.exists(), "sidecar must be written");
 let generated_path = tmp.path().join("docs/GENERATED.md");
 assert!(
 !generated_path.exists(),
 "GENERATED.md must not exist when --no-regenerate"
 );
}

#[test]
fn set_section_intent_auto_regenerates_generated_md() {
 // Section atomic mutate also auto-regenerates (Round 164+ migration prep).
 // The section render path emits an atomic-only header until section
 // migration lands; this verifies the wire fires for section mutates too.
 //
 // Round 287 fail-loud: set_section_intent now requires the Section to
 // exist in the atomic store first. Seed the sidecar JSON directly
 // (test fixture path) so the smoke test exercises the cascade wire
 // rather than the creation path.
 let tmp = TempDir::new().unwrap();
 write_min_workspace_config(tmp.path());

 let sidecar = tmp.path().join("docs/.atomic/workspace.atomic.json");
 fs::create_dir_all(sidecar.parent().unwrap()).unwrap();
 fs::write(
 &sidecar,
 r#"{
  "sections": {"1": {"title": "Top", "parent_doc": "docs/STUB.md"}},
  "changelog_entries": {},
  "inventory_entries": {},
  "schema_version": 3
}"#,
 )
 .unwrap();

 let out = Command::new(cli_binary())
 .args([
 "set-section-intent",
 "--section",
 "1",
 "--intent",
 "test intent for §1",
 ])
 .current_dir(tmp.path())
 .output()
 .expect("run set-section-intent");
 assert!(
 out.status.success(),
 "set-section-intent failed: stdout={}, stderr={}",
 String::from_utf8_lossy(&out.stdout),
 String::from_utf8_lossy(&out.stderr)
 );

 let generated_path = tmp.path().join("docs/GENERATED.md");
 assert!(
 generated_path.exists(),
 "GENERATED.md must exist after section mutate"
 );
 let generated = fs::read_to_string(&generated_path).unwrap();
 assert!(
 generated.contains("## Sections"),
 "section mutate must trigger Sections heading; got: {}",
 generated
 );
 assert!(
 generated.contains("### §1"),
 "section mutate must render §1 atomic header; got: {}",
 generated
 );
}

#[test]
fn verify_generated_reports_sync_when_in_sync() {
 let tmp = TempDir::new().unwrap();
 write_min_workspace_config(tmp.path());

 let changes_path = tmp.path().join("changes.txt");
 fs::write(&changes_path, "x\n").unwrap();
 let verify_path = tmp.path().join("verify.txt");
 fs::write(&verify_path, "v\n").unwrap();

 // Mutate (auto-regenerates GENERATED.md).
 Command::new(cli_binary())
 .args([
 "append-changelog-entry-v2",
 "--entry-id",
 "Round 999",
 "--decision",
 "verify-sync test",
 "--changes-file",
 changes_path.to_str().unwrap(),
 "--verification-file",
 verify_path.to_str().unwrap(),
 ])
 .current_dir(tmp.path())
 .output()
 .expect("run mutate");

 // verify-generated must succeed (in sync).
 let out = Command::new(cli_binary())
 .arg("verify-generated")
 .current_dir(tmp.path())
 .output()
 .expect("run verify-generated");
 assert!(
 out.status.success(),
 "verify-generated must exit 0 when in sync; stdout={}, stderr={}",
 String::from_utf8_lossy(&out.stdout),
 String::from_utf8_lossy(&out.stderr)
 );
 let stdout = String::from_utf8_lossy(&out.stdout);
 assert!(
 stdout.contains("status: OK (sync)"),
 "verify-generated must print OK; got: {}",
 stdout
 );
}

#[test]
fn verify_generated_reports_stale_after_no_regenerate_mutate() {
 let tmp = TempDir::new().unwrap();
 write_min_workspace_config(tmp.path());

 let changes_path = tmp.path().join("changes.txt");
 fs::write(&changes_path, "x\n").unwrap();
 let verify_path = tmp.path().join("verify.txt");
 fs::write(&verify_path, "v\n").unwrap();

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
 "--verification-file",
 verify_path.to_str().unwrap(),
 ])
 .current_dir(tmp.path())
 .output()
 .expect("first mutate");

 // Second mutate with --no-regenerate (sidecar updated, GENERATED.md stale).
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
 "--verification-file",
 verify_path.to_str().unwrap(),
 "--no-regenerate",
 ])
 .current_dir(tmp.path())
 .output()
 .expect("second mutate --no-regenerate");

 // verify-generated must report stale (sidecar has 998+999, GENERATED has only 998).
 let out = Command::new(cli_binary())
 .arg("verify-generated")
 .current_dir(tmp.path())
 .output()
 .expect("run verify-generated");
 assert!(
 !out.status.success(),
 "verify-generated must exit non-zero when stale; stdout={}, stderr={}",
 String::from_utf8_lossy(&out.stdout),
 String::from_utf8_lossy(&out.stderr)
 );
 let stderr = String::from_utf8_lossy(&out.stderr);
 assert!(
 stderr.contains("STALE"),
 "verify-generated must print STALE; got: {}",
 stderr
 );
}
