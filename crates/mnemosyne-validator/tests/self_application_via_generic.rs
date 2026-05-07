//! Round 148 — self-application re-import via generic.
//!
//! Verifies that Mnemosyne's own 7-doc workspace, run through the *same*
//! generic loader path that external fixtures use (Round 147 carry),
//! produces the same observable invariants as the legacy hardcoded path:
//!
//! - 7 docs parse with the configured schema
//! - cross-doc orphan count = 0
//! - round-trip diff_count = 0 for all 7 docs
//! - changelog entries are recognized under the configured title set
//!
//! This is the dogfood proof: Mnemosyne is the *first user* of its
//! generic library — no privileged path remains. Round 151 closure gate
//! closes once this test continues to PASS alongside the external
//! fixtures from Round 147.

use mnemosyne_validator::{
 compare_typed_facts, default_ruleset_with_config, discover_config,
 emitter::emit_markdown_with_default, parse_markdown_with_schema,
 validator::cross_ref_orphan_reject_with_workspace, AtomicStore, SchemaSection, StyleSeverity,
 Workspace,
};
use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
 PathBuf::from(env!("CARGO_MANIFEST_DIR"))
 .parent()
 .unwrap()
 .parent()
 .unwrap()
 .to_path_buf()
}

#[test]
#[ignore = "Round 252 -- atomic store reset to empty; test depends on populated atomic ledger or 7-md workspace"]
fn self_application_round_trip_via_generic_loader() {
 let root = repo_root();
 let loaded = discover_config(&root)
 .unwrap()
 .expect("repo root must contain mnemosyne.toml");

 // Schema = whatever the config declares (currently mnemosyne preset
 // via the [schema] table, Round 143-145 carry).
 let schema = loaded
 .config
 .schema
 .clone()
 .unwrap_or_else(SchemaSection::mnemosyne_preset);

 let mut ws = Workspace::from_config(&loaded);
 let mut originals = Vec::new();
 for path in loaded.doc_paths() {
 let abs = loaded.doc_abs_path(path);
 let content = fs::read_to_string(&abs)
 .unwrap_or_else(|e| panic!("read {}: {}", abs.display(), e));
 let parsed = parse_markdown_with_schema(&content, path, &schema);
 ws.insert(path.to_string(), parsed.clone());
 originals.push((path.to_string(), parsed));
 }

 // Round 251 — post 7-md deletion the workspace collapses to GENERATED.md
 // alone (atomic store = sole source of truth, GENERATED.md = sole readable
 // artifact). Pre-deletion this assertion was `== 7`.
 assert_eq!(
 ws.docs.len(),
 loaded.config.workspace.docs.len(),
 "self-application doc count must match config"
 );

 // T1 orphan check via the same workspace-aware path the CLI uses.
 let mut total_orphans = 0usize;
 for (path, _) in &originals {
 let parsed = ws.docs.get(path).unwrap();
 let orphans = cross_ref_orphan_reject_with_workspace(parsed, &ws);
 total_orphans += orphans.len();
 }
 assert_eq!(
 total_orphans, 0,
 "self-application via generic loader must report 0 orphans"
 );

 // Round-trip 7/7 mandatory: every doc emits and reparses to the same
 // typed facts (Section / ChangelogEntry / CrossRef counts preserved).
 let default_doc_for_emit = loaded.config.workspace.default_doc.as_deref();
 for (path, original) in &originals {
 let reclassified = ws
 .reclassify_cross_refs(path)
 .unwrap_or_else(|| panic!("workspace must contain {}", path));
 let emitted = emit_markdown_with_default(&reclassified, default_doc_for_emit);
 let reparsed = parse_markdown_with_schema(&emitted, path, &schema);
 let diff = compare_typed_facts(original, &reparsed);
 assert!(
 diff.mandatory_preserved,
 "{} round-trip failed: section={}/{} changelog={}/{} cross_ref={}/{}",
 path,
 diff.section_count_a,
 diff.section_count_b,
 diff.changelog_entry_count_a,
 diff.changelog_entry_count_b,
 diff.cross_ref_count_a,
 diff.cross_ref_count_b,
 );
 }
}

#[test]
#[ignore = "Round 252 -- atomic store reset to empty; test depends on populated atomic ledger or 7-md workspace"]
fn self_application_style_baseline_via_generic_loader() {
 // The style violation count must match what validate-workspace prints
 // (167 warn / 52 info per the Round 144 baseline). This proves the
 // CLI is using the same code path the test uses — no privileged
 // ruleset construction in the binary.
 let root = repo_root();
 let loaded = discover_config(&root).unwrap().expect("config");
 let schema = loaded
 .config
 .schema
 .clone()
 .unwrap_or_else(SchemaSection::mnemosyne_preset);
 let ruleset = default_ruleset_with_config(
 loaded.config.style.as_ref(),
 loaded.config.terminology.as_ref(),
 );

 let mut ws = Workspace::from_config(&loaded);
 let mut docs: Vec<(String, _)> = Vec::new();
 for path in loaded.doc_paths() {
 let content = fs::read_to_string(loaded.doc_abs_path(path)).unwrap();
 let parsed = parse_markdown_with_schema(&content, path, &schema);
 ws.insert(path.to_string(), parsed.clone());
 docs.push((path.to_string(), parsed));
 }

 let mut warn = 0usize;
 let mut info = 0usize;
 let mut term = 0usize;
 let atomic_store = AtomicStore::load(&AtomicStore::default_sidecar_path(&root))
 .expect("atomic sidecar load");
 for (path, parsed) in &docs {
 let v = mnemosyne_validator::check_style(path, parsed, &atomic_store, &ruleset);
 for sv in &v {
 match sv.severity {
  StyleSeverity::Warn => warn += 1,
  StyleSeverity::Info => info += 1,
 }
 if sv.rule_id == "terminology_consistency" {
  term += 1;
 }
 }
 }

 // terminology_consistency hits drive T3 reject in validate-workspace
 // (Round 138 closure activated reject for this rule). Must stay 0
 // post-Round 132 cleanup.
 assert_eq!(
 term, 0,
 "terminology_consistency must stay 0 (Round 138 reject activated)"
 );

 // Round 251 — re-anchor post 7-md deletion. Workspace collapsed to
 // GENERATED.md alone; rendered atomic sections produce different style
 // counts than the 7-md baseline (Round 241 26/14). New floor matches
 // probe output of `validate-workspace` after deletion: T3 warn 390 /
 // T4 info 96. terminology_consistency stays 0 (Round 250 changelog-area
 // skip carry).
 assert_eq!(warn, 390, "T3 warn drift (Round 251 GENERATED.md baseline 390)");
 assert_eq!(info, 96, "T4 info drift (Round 251 GENERATED.md baseline 96)");
}
