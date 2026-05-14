//! Round 280 — split-brain regression: read / validation CLI paths
//! must honor `[atomic].sidecar_path` (and `output_path`), matching the
//! mutate / cascade paths that already did since Round 279.
//!
//! Trigger: tc8-harness adopter set `[atomic].sidecar_path = "doc/.atomic/store.json"`
//! in `mnemosyne.toml`. Mutate primitives wrote to the configured path
//! (Round 279 fix), but `validate-workspace`, `query`, `validate-code-refs`,
//! etc. loaded the default `docs/.atomic/workspace.atomic.json` (which
//! didn't exist), reporting `sections=0` + `GENERATED.md=STALE` falsely.

use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn cli_binary() -> &'static str {
 env!("CARGO_BIN_EXE_mnemosyne-cli")
}

/// Write a workspace whose `[atomic].sidecar_path` + `output_path` deviate
/// from defaults, mirroring the tc8-harness adoption layout.
fn write_redirected_workspace(workspace: &std::path::Path) {
 fs::create_dir_all(workspace.join("doc/.atomic")).unwrap();
 fs::create_dir_all(workspace.join("docs/coverage")).unwrap();
 let cfg = r#"
[workspace]
docs = ["docs/coverage/SPEC_COVERAGE.md"]
default_doc = "docs/coverage/SPEC_COVERAGE.md"

[atomic]
sidecar_path = "doc/.atomic/store.json"
output_path = "docs/coverage/SPEC_COVERAGE.md"
"#;
 fs::write(workspace.join("mnemosyne.toml"), cfg).unwrap();
 let stub = "# SPEC_COVERAGE\n\n## Sections\n\n## Changelog\n";
 fs::write(workspace.join("docs/coverage/SPEC_COVERAGE.md"), stub).unwrap();
}

fn run_cli(workspace: &std::path::Path, args: &[&str]) -> std::process::Output {
 Command::new(cli_binary())
 .args(args)
 .current_dir(workspace)
 .output()
 .expect("run mnemosyne-cli")
}

#[test]
fn validate_workspace_honors_atomic_sidecar_override() {
 let tmp = TempDir::new().unwrap();
 write_redirected_workspace(tmp.path());

 // Round 287 fail-loud: seed §sample at the redirected sidecar path
 // (mutate path requires the Section to exist; the test fixture skips
 // the audit-receipted creation path).
 fs::write(
 tmp.path().join("doc/.atomic/store.json"),
 r#"{
  "sections": {"sample": {"title": "Sample", "parent_doc": "docs/coverage/SPEC_COVERAGE.md"}},
  "changelog_entries": {},
  "inventory_entries": {},
  "schema_version": 3
}"#,
 )
 .unwrap();

 // Write a section via the mutate path (Round 279 already honored config here).
 let mutate = run_cli(
 tmp.path(),
 &["set-section-intent", "--section", "§sample", "--intent", "round-280 split-brain regression"],
 );
 assert!(
 mutate.status.success(),
 "mutate must succeed; stderr={}",
 String::from_utf8_lossy(&mutate.stderr)
 );

 // Read path must see the same store.
 let validate = run_cli(tmp.path(), &["validate-workspace"]);
 assert!(
 validate.status.success(),
 "validate-workspace must pass when sidecar override is honored; stdout={}, stderr={}",
 String::from_utf8_lossy(&validate.stdout),
 String::from_utf8_lossy(&validate.stderr)
 );
 let stdout = String::from_utf8_lossy(&validate.stdout);
 assert!(
 stdout.contains("sections=1"),
 "validate-workspace must count the section the mutate wrote; got: {}",
 stdout
 );
 assert!(
 stdout.contains("GENERATED.md=sync"),
 "validate-workspace must not falsely report stale; got: {}",
 stdout
 );
}

#[test]
fn query_list_sections_honors_atomic_sidecar_override() {
 let tmp = TempDir::new().unwrap();
 write_redirected_workspace(tmp.path());

 // Round 287 fail-loud: seed §q-sample at the redirected sidecar path
 // before the mutate. See validate_workspace_honors_atomic_sidecar_override
 // for the same fixture pattern.
 fs::write(
 tmp.path().join("doc/.atomic/store.json"),
 r#"{
  "sections": {"q-sample": {"title": "Q-Sample", "parent_doc": "docs/coverage/SPEC_COVERAGE.md"}},
  "changelog_entries": {},
  "inventory_entries": {},
  "schema_version": 3
}"#,
 )
 .unwrap();

 // Mutate writes to the redirected sidecar.
 run_cli(
 tmp.path(),
 &["set-section-intent", "--section", "§q-sample", "--intent", "split-brain query regression"],
 );

 let query = run_cli(tmp.path(), &["query", "--list-sections"]);
 let stdout = String::from_utf8_lossy(&query.stdout);
 assert!(
 stdout.contains("q-sample"),
 "query --list-sections must see the section the mutate wrote; got: {}",
 stdout
 );
}

#[test]
fn validate_code_refs_honors_atomic_sidecar_override() {
 // Round 280 split-brain coverage for the inventory axis: a TC ID
 // registered via the mutate path must be visible to validate-code-refs.
 let tmp = TempDir::new().unwrap();
 write_redirected_workspace(tmp.path());
 // Add a code_refs scan target with one inventory citation.
 fs::create_dir_all(tmp.path().join("src")).unwrap();
 fs::write(tmp.path().join("src/x.rs"), "// FOO_07 cite\n").unwrap();
 // Update mnemosyne.toml: append [code_refs] with the FOO_ prefix.
 let cfg = r#"
[workspace]
docs = ["docs/coverage/SPEC_COVERAGE.md"]
default_doc = "docs/coverage/SPEC_COVERAGE.md"

[atomic]
sidecar_path = "doc/.atomic/store.json"
output_path = "docs/coverage/SPEC_COVERAGE.md"

[code_refs]
paths = ["src/"]
inventory_prefixes = ["FOO_"]
severity_inventory = "warn"
"#;
 fs::write(tmp.path().join("mnemosyne.toml"), cfg).unwrap();

 // Register FOO_07 as active via mutate (writes to redirected sidecar).
 let mutate = run_cli(
 tmp.path(),
 &["add-inventory-entry", "--id", "FOO_07", "--status", "active", "--no-regenerate"],
 );
 assert!(
 mutate.status.success(),
 "mutate must succeed; stderr={}",
 String::from_utf8_lossy(&mutate.stderr)
 );

 // validate-code-refs must see FOO_07 in the store (loaded via config-aware
 // sidecar resolution); the cite must NOT surface as InventoryMissing.
 let v = run_cli(tmp.path(), &["validate-code-refs"]);
 let stdout = String::from_utf8_lossy(&v.stdout);
 assert!(
 stdout.contains("inv_missing=0"),
 "validate-code-refs must see FOO_07 in the redirected sidecar; got: {}",
 stdout
 );
}
