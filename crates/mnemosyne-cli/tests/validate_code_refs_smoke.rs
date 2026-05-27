//! Round 256-260 — `validate-code-refs` subcommand smoke tests.
//!
//! Test scope:
//! (i) `[plugins.set_equality_validator]` omission → skip mode (exit 0 with explicit log line)
//! (ii) clean codebase (citation present in atomic store) → no violations
//! (iii) hallucinated citation → reject (exit 1) under default severity
//! (iv) hallucinated citation → warn (exit 0) under `--severity-missing warn`
//! (v) identifier-shaped incidental hits (`TestRound254Helper`,
//!  `round_254_helper`) → not flagged (word-boundary carve-out)
//! (vi) JSON output shape (Round 256 fields)
//! (vii) `--filter-id` decay scan (Round 258)
//! (viii) Round 260 — `§<id>` hallucination → SectionMissing (reject)
//! (ix) Round 260 — `§<id>` cite without matching impl entry → CitationUnbound
//! (x) Round 260 — impl entry without code cite → ImplementationUnbacked
//! (xi) Round 260 — `--severity-binding warn` keeps exit 0 on binding violations

use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn cli_binary() -> &'static str {
 env!("CARGO_BIN_EXE_mnemosyne-cli")
}

/// Set up a minimal workspace with one ChangelogEntry (`Round 1`) and
/// optionally a `[plugins.set_equality_validator]` table pointing at `src/`.
fn write_workspace(workspace: &Path, with_code_refs: bool) {
 fs::create_dir_all(workspace.join("docs/.atomic")).unwrap();
 fs::create_dir_all(workspace.join("src")).unwrap();
 let mut cfg = String::from(
 "[workspace]\ndocs = [\"docs/GENERATED.md\"]\ndefault_doc = \"docs/GENERATED.md\"\n\
 [schema]\nentry_id_prefix = \"Round \"\n",
 );
 if with_code_refs {
 cfg.push_str("[plugins.set_equality_validator]\npaths = [\"src/\"]\n");
 }
 fs::write(workspace.join("mnemosyne.toml"), cfg).unwrap();

 // Atomic store with one valid entry: "Round 1".
 let atomic = serde_json::json!({
 "schema_version": 1,
 "sections": {},
 "changelog_entries": {
 "Round 1": {
 "decision_summary": "test entry"
 }
 }
 });
 fs::write(
 workspace.join("docs/.atomic/workspace.atomic.json"),
 serde_json::to_string_pretty(&atomic).unwrap(),
 )
 .unwrap();
 // GENERATED.md stub so default_doc resolves; subcommand doesn't need
 // workspace doc validation but loading config wants it to exist.
 fs::write(workspace.join("docs/GENERATED.md"), "# Stub\n").unwrap();
}

fn run_cli(workspace: &Path, args: &[&str]) -> std::process::Output {
 Command::new(cli_binary())
 .args(args)
 .current_dir(workspace)
 .output()
 .expect("cli exec")
}

#[test]
fn case_i_skip_mode_when_code_refs_unconfigured() {
 let tmp = TempDir::new().unwrap();
 write_workspace(tmp.path(), false);

 let out = run_cli(tmp.path(), &["validate-code-refs"]);
 assert!(out.status.success(), "exit code: {:?}", out.status.code());
 let stdout = String::from_utf8_lossy(&out.stdout);
 assert!(
 stdout.contains("skipped") && stdout.contains("[plugins.set_equality_validator]"),
 "stdout: {}",
 stdout
 );
}

#[test]
fn case_ii_clean_codebase_no_violations() {
 let tmp = TempDir::new().unwrap();
 write_workspace(tmp.path(), true);
 fs::write(
 tmp.path().join("src/lib.rs"),
 "// Round 1 — test entry implementation\nfn main() {}\n",
 )
 .unwrap();

 let out = run_cli(tmp.path(), &["validate-code-refs"]);
 assert!(
 out.status.success(),
 "exit code: {:?}, stderr: {}",
 out.status.code(),
 String::from_utf8_lossy(&out.stderr)
 );
 let stdout = String::from_utf8_lossy(&out.stdout);
 assert!(stdout.contains("missing=0"), "stdout: {}", stdout);
}

#[test]
fn case_iii_hallucinated_citation_rejected() {
 let tmp = TempDir::new().unwrap();
 write_workspace(tmp.path(), true);
 fs::write(
 tmp.path().join("src/lib.rs"),
 "// see Round 999 for hallucinated reference\nfn main() {}\n",
 )
 .unwrap();

 let out = run_cli(tmp.path(), &["validate-code-refs"]);
 assert!(
 !out.status.success(),
 "expected reject; stdout: {}",
 String::from_utf8_lossy(&out.stdout)
 );
 let stderr = String::from_utf8_lossy(&out.stderr);
 assert!(
 stderr.contains("Round 999") || stderr.contains("missing"),
 "stderr should mention the missing citation; got: {}",
 stderr
 );
}

#[test]
fn case_iv_hallucinated_warn_severity_exits_zero() {
 let tmp = TempDir::new().unwrap();
 write_workspace(tmp.path(), true);
 fs::write(
 tmp.path().join("src/lib.rs"),
 "// see Round 999 for hallucinated reference\nfn main() {}\n",
 )
 .unwrap();

 let out = run_cli(
 tmp.path(),
 &["validate-code-refs", "--severity-missing", "warn"],
 );
 assert!(
 out.status.success(),
 "warn severity should exit 0; stderr: {}",
 String::from_utf8_lossy(&out.stderr)
 );
 let stdout = String::from_utf8_lossy(&out.stdout);
 assert!(
 stdout.contains("Round 999"),
 "stdout should list the violation; got: {}",
 stdout
 );
}

#[test]
fn case_v_identifier_shaped_incidental_hits_not_flagged() {
 let tmp = TempDir::new().unwrap();
 write_workspace(tmp.path(), true);
 fs::write(
 tmp.path().join("src/lib.rs"),
 "struct TestRound254Helper;\n\
 fn round_254_helper() {}\n\
 // Round 1 — actual citation\n",
 )
 .unwrap();

 let out = run_cli(tmp.path(), &["validate-code-refs"]);
 assert!(
 out.status.success(),
 "identifier-shaped hits should not be citations; stderr: {}",
 String::from_utf8_lossy(&out.stderr)
 );
 let stdout = String::from_utf8_lossy(&out.stdout);
 assert!(stdout.contains("missing=0"), "stdout: {}", stdout);
}

#[test]
fn case_vii_filter_id_surfaces_decay_and_skips_others() {
 // Round 258 — when --filter-id is set, citations matching it are
 // reported as decay; non-matching citations (even if missing) are
 // suppressed. This is the cascade caller's read mode after a
 // supersede mutate.
 let tmp = TempDir::new().unwrap();
 write_workspace(tmp.path(), true);
 fs::write(
 tmp.path().join("src/lib.rs"),
 "// Round 1 still in atomic but flagged decay\n\
 // Round 999 hallucinated, but filter excludes\n\
 // Round 1 second occurrence\n",
 )
 .unwrap();

 let out = run_cli(
 tmp.path(),
 &["validate-code-refs", "--filter-id", "Round 1", "--json"],
 );
 assert!(
 out.status.success(),
 "filter-id mode does not reject; stderr: {}",
 String::from_utf8_lossy(&out.stderr)
 );
 let parsed: serde_json::Value =
 serde_json::from_str(&String::from_utf8_lossy(&out.stdout)).expect("valid JSON");
 assert_eq!(parsed["filter_id"], "Round 1");
 assert_eq!(parsed["decay_count"], 2);
 assert_eq!(parsed["missing_count"], 0);
 let violations = parsed["violations"].as_array().expect("violations array");
 assert_eq!(violations.len(), 2);
 for v in violations {
 assert_eq!(v["kind"], "decay");
 assert_eq!(v["entry_id"], "Round 1");
 }
}

#[test]
fn case_vi_json_output_shape() {
 let tmp = TempDir::new().unwrap();
 write_workspace(tmp.path(), true);
 fs::write(
 tmp.path().join("src/lib.rs"),
 "// Round 999 missing\n// Round 1 ok\n",
 )
 .unwrap();

 let out = run_cli(
 tmp.path(),
 &["validate-code-refs", "--severity-missing", "warn", "--json"],
 );
 assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
 let stdout = String::from_utf8_lossy(&out.stdout);
 let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
 assert_eq!(parsed["primitive"], "validate-code-refs");
 assert_eq!(parsed["missing_count"], 1);
 // Round 260 — new count fields present in JSON shape.
 assert!(parsed.get("section_missing_count").is_some());
 assert!(parsed.get("citation_unbound_count").is_some());
 assert!(parsed.get("impl_unbacked_count").is_some());
 assert!(parsed.get("severity_binding").is_some());
 let violations = parsed["violations"].as_array().expect("violations array");
 assert_eq!(violations.len(), 1);
 assert_eq!(violations[0]["entry_id"], "Round 999");
 assert_eq!(violations[0]["kind"], "missing");
}

// ============ Round 260 — Path B bidirectional smoke tests ============

/// Write a minimal workspace whose atomic store includes one §<id>
/// section with optional `implementations` entries. `with_code_refs` adds
/// `[plugins.set_equality_validator] paths = ["src/"]`.
fn write_workspace_with_section(
 workspace: &Path,
 with_code_refs: bool,
 section_id: &str,
 impls: &[(&str, Option<&str>)],
) {
 fs::create_dir_all(workspace.join("docs/.atomic")).unwrap();
 fs::create_dir_all(workspace.join("src")).unwrap();
 let mut cfg = String::from(
 "[workspace]\ndocs = [\"docs/GENERATED.md\"]\ndefault_doc = \"docs/GENERATED.md\"\n\
 [schema]\nentry_id_prefix = \"Round \"\n",
 );
 if with_code_refs {
 cfg.push_str("[plugins.set_equality_validator]\npaths = [\"src/\"]\n");
 }
 fs::write(workspace.join("mnemosyne.toml"), cfg).unwrap();
 let impls_json: Vec<_> = impls
 .iter()
 .map(|(f, s)| match s {
 Some(sym) => serde_json::json!({ "file": f, "symbol": sym }),
 None => serde_json::json!({ "file": f }),
 })
 .collect();
 let mut sections = serde_json::Map::new();
 sections.insert(
 section_id.to_string(),
 serde_json::json!({ "implementations": impls_json }),
 );
 let atomic = serde_json::json!({
 "schema_version": 1,
 "sections": sections,
 "changelog_entries": {}
 });
 fs::write(
 workspace.join("docs/.atomic/workspace.atomic.json"),
 serde_json::to_string_pretty(&atomic).unwrap(),
 )
 .unwrap();
 fs::write(workspace.join("docs/GENERATED.md"), "# Stub\n").unwrap();
}

#[test]
fn case_viii_section_missing_rejected_under_default_severity() {
 let tmp = TempDir::new().unwrap();
 write_workspace(tmp.path(), true);
 fs::write(
 tmp.path().join("src/lib.rs"),
 "// see §999 hallucinated section ref\nfn main() {}\n",
 )
 .unwrap();
 let out = run_cli(tmp.path(), &["validate-code-refs"]);
 assert!(
 !out.status.success(),
 "expected reject; stdout: {}",
 String::from_utf8_lossy(&out.stdout)
 );
 let stderr = String::from_utf8_lossy(&out.stderr);
 assert!(
 stderr.contains("SectionMissing") || stderr.contains("section_missing")
 || stderr.contains("hallucination"),
 "stderr should mention SectionMissing class; got: {}",
 stderr
 );
}

#[test]
fn case_ix_citation_unbound_rejected_under_default_binding_severity() {
 // §39.implementations = [src/bar.rs] but src/foo.rs cites §39.
 let tmp = TempDir::new().unwrap();
 write_workspace_with_section(tmp.path(), true, "39", &[("src/bar.rs", None)]);
 fs::write(
 tmp.path().join("src/foo.rs"),
 "// §39 cite from unregistered file\n",
 )
 .unwrap();
 fs::write(
 tmp.path().join("src/bar.rs"),
 "// §39 cite from authoritative file\n",
 )
 .unwrap();
 let out = run_cli(tmp.path(), &["validate-code-refs"]);
 assert!(
 !out.status.success(),
 "expected reject; stdout: {}",
 String::from_utf8_lossy(&out.stdout)
 );
 let stderr = String::from_utf8_lossy(&out.stderr);
 assert!(
 stderr.contains("binding") || stderr.contains("CitationUnbound"),
 "stderr should mention binding class; got: {}",
 stderr
 );
}

#[test]
fn case_x_implementation_unbacked_rejected_under_default_binding_severity() {
 // §39.implementations = [src/foo.rs] but src/foo.rs has no §39 cite.
 let tmp = TempDir::new().unwrap();
 write_workspace_with_section(tmp.path(), true, "39", &[("src/foo.rs", Some("Foo"))]);
 fs::write(
 tmp.path().join("src/foo.rs"),
 "// no spec citation\nfn foo() {}\n",
 )
 .unwrap();
 let out = run_cli(tmp.path(), &["validate-code-refs"]);
 assert!(
 !out.status.success(),
 "expected reject; stdout: {}",
 String::from_utf8_lossy(&out.stdout)
 );
 let stderr = String::from_utf8_lossy(&out.stderr);
 assert!(
 stderr.contains("binding") || stderr.contains("ImplementationUnbacked"),
 "stderr should mention binding class; got: {}",
 stderr
 );
}

#[test]
fn case_xi_severity_binding_warn_keeps_exit_zero() {
 let tmp = TempDir::new().unwrap();
 write_workspace_with_section(tmp.path(), true, "39", &[("src/foo.rs", None)]);
 fs::write(tmp.path().join("src/foo.rs"), "// no cite\n").unwrap();
 let out = run_cli(
 tmp.path(),
 &["validate-code-refs", "--severity-binding", "warn", "--json"],
 );
 assert!(
 out.status.success(),
 "warn severity should exit 0; stderr: {}",
 String::from_utf8_lossy(&out.stderr)
 );
 let parsed: serde_json::Value =
 serde_json::from_str(&String::from_utf8_lossy(&out.stdout)).expect("valid JSON");
 assert_eq!(parsed["impl_unbacked_count"], 1);
 assert_eq!(parsed["severity_binding"], "warn");
 let violations = parsed["violations"].as_array().expect("violations array");
 assert_eq!(violations.len(), 1);
 assert_eq!(violations[0]["kind"], "impl_unbacked");
 assert_eq!(violations[0]["section_id"], "39");
}
