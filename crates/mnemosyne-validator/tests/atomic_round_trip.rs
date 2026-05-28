//! Phase 0f atomic typed facts round-trip — Round 162 ratify.
//!
//! Integration test: atomic fields set → template render → output is valid
//! markdown with expected structure. Round-trip invariant (Round 161 §56
//! reframe ratify): same input always produces same output (deterministic).

use mnemosyne_atomic::{AtomicStore, ExampleBlock, RejectedAlternative, add_section_caveat, add_section_example, add_section_implementation, append_changelog_entry, set_section_alternatives, set_section_impact_scope, set_section_inputs, set_section_intent, set_section_outputs, set_section_rationale};
use mnemosyne_validator::{render_changelog_entry, render_section};
use tempfile::TempDir;

#[test]
fn atomic_section_round_trip_full_shape() {
 let tmp = TempDir::new().unwrap();
 let sidecar = tmp.path().join("docs/.atomic/workspace.atomic.json");
 let mut store = AtomicStore::new();

 // Round 287 fail-loud: explicit Section creation before content mutates.
 mnemosyne_atomic::add_section(
 &mut store,
 &sidecar,
 "43",
 "docs/GENERATED.md",
 "cascade_query kind",
 None,
 )
 .unwrap();
 set_section_intent(&mut store, &sidecar, "43", "test intent for §43").unwrap();
 set_section_rationale(
 &mut store,
 &sidecar,
 "43",
 &["reason A".into(), "reason B".into()],
 )
 .unwrap();
 set_section_inputs(&mut store, &sidecar, "43", &["input X".into()]).unwrap();
 set_section_outputs(&mut store, &sidecar, "43", &["output Y".into()]).unwrap();
 add_section_caveat(&mut store, &sidecar, "43", "caveat Z").unwrap();
 set_section_alternatives(
 &mut store,
 &sidecar,
 "43",
 &[RejectedAlternative {
 alternative: "approach A".into(),
 reason: "doesn't scale".into(),
 }],
 )
 .unwrap();
 set_section_impact_scope(&mut store, &sidecar, "43", &["15".into(), "39".into()]).unwrap();
 add_section_example(
 &mut store,
 &sidecar,
 "43",
 ExampleBlock {
 language: "rust".into(),
 code: "fn main() {}".into(),
 },
 )
 .unwrap();
 // Round 259 — Path B binding entries (file + file:symbol shape).
 add_section_implementation(
 &mut store,
 &sidecar,
 "43",
 "crates/mnemosyne-validator/src/atomic.rs",
 Some("AtomicSection"),
 )
 .unwrap();
 add_section_implementation(
 &mut store,
 &sidecar,
 "43",
 "crates/mnemosyne-cli/src/atomic_cli.rs",
 None,
 )
 .unwrap();

 // Re-load and render.
 let loaded = AtomicStore::load(&sidecar).unwrap();
 let atomic_43 = loaded.section("43").expect("§43 must exist");
 let md = render_section("43", "cascade_query kind", "active", atomic_43).unwrap();

 assert!(md.contains("## §43. cascade_query kind"));
 assert!(md.contains("**Intent**: test intent for §43"));
 assert!(md.contains("- reason A"));
 assert!(md.contains("- reason B"));
 assert!(md.contains("- input X"));
 assert!(md.contains("- output Y"));
 assert!(md.contains("- caveat Z"));
 assert!(md.contains("approach A — doesn't scale"));
 assert!(md.contains("**Impact scope**: §15, §39"));
 assert!(md.contains("```rust"));
 assert!(md.contains("fn main() {}"));
 assert!(md.contains("**Implementations**"));
 assert!(md.contains("- crates/mnemosyne-validator/src/atomic.rs:AtomicSection"));
 assert!(md.contains("- crates/mnemosyne-cli/src/atomic_cli.rs"));
}

#[test]
fn atomic_changelog_entry_round_trip() {
 let tmp = TempDir::new().unwrap();
 let sidecar = tmp.path().join("docs/.atomic/workspace.atomic.json");
 let mut store = AtomicStore::new();

 append_changelog_entry(
 &mut store,
 &sidecar,
 "Round 162",
 Some("template engine + atomic mutate API"),
 &[
 "tera workspace dep added".into(),
 "templates/section.md.tera + changelog_entry.md.tera".into(),
 "9 atomic mutate primitives".into(),
 ],
 &[
 "411 production tests PASS".into(),
 "round-trip render deterministic".into(),
 ],
 &["15".into(), "39".into(), "56".into()],
 &["Round 163 forward-wire next".into()],
 )
 .unwrap();

 let loaded = AtomicStore::load(&sidecar).unwrap();
 let entry = loaded.entry("Round 162").expect("entry must exist");
 let md = render_changelog_entry("Round 162", entry).unwrap();

 assert!(md.contains("### Round 162 — template engine + atomic mutate API"));
 assert!(md.contains("- tera workspace dep added"));
 assert!(md.contains("- 9 atomic mutate primitives"));
 assert!(md.contains("- 411 production tests PASS"));
 assert!(md.contains("**Impact**: §15, §39, §56"));
 assert!(md.contains("- Round 163 forward-wire next"));
}

#[test]
fn atomic_section_render_deterministic_across_loads() {
 let tmp = TempDir::new().unwrap();
 let sidecar = tmp.path().join("workspace.atomic.json");
 let mut store = AtomicStore::new();

 // Round 287 fail-loud: explicit Section creation.
 mnemosyne_atomic::add_section(
 &mut store,
 &sidecar,
 "43",
 "docs/GENERATED.md",
 "test",
 None,
 )
 .unwrap();
 set_section_intent(&mut store, &sidecar, "43", "stable intent").unwrap();
 set_section_rationale(
 &mut store,
 &sidecar,
 "43",
 &["a".into(), "b".into(), "c".into()],
 )
 .unwrap();

 let loaded1 = AtomicStore::load(&sidecar).unwrap();
 let loaded2 = AtomicStore::load(&sidecar).unwrap();

 let r1 = render_section(
 "43",
 "test",
 "active",
 loaded1.section("43").unwrap(),
 )
 .unwrap();
 let r2 = render_section(
 "43",
 "test",
 "active",
 loaded2.section("43").unwrap(),
 )
 .unwrap();

 assert_eq!(r1, r2, "render must be byte-identical across loads");
}

#[test]
fn atomic_section_legacy_carry_unaffected() {
 // Atomic fields are *additive* — they don't mutate the legacy `body` field
 // on Section (which lives outside this store). Verify by setting atomic
 // fields and confirming the sidecar is the only side-effect (no other
 // file path is touched).
 let tmp = TempDir::new().unwrap();
 let sidecar = tmp.path().join("workspace.atomic.json");
 let mut store = AtomicStore::new();

 // Round 287 fail-loud: explicit Section creation.
 mnemosyne_atomic::add_section(
 &mut store,
 &sidecar,
 "43",
 "docs/GENERATED.md",
 "test",
 None,
 )
 .unwrap();
 set_section_intent(&mut store, &sidecar, "43", "atomic-only").unwrap();

 // Sidecar exists, no other files in tmp dir.
 let entries: Vec<_> = std::fs::read_dir(tmp.path())
 .unwrap()
 .filter_map(|r| r.ok())
 .map(|e| e.file_name().to_string_lossy().into_owned())
 .collect();
 assert_eq!(
 entries,
 vec!["workspace.atomic.json"],
 "only sidecar should exist; legacy body / sub_bullets carry on existing markdown files"
 );
}

#[test]
fn atomic_changelog_v2_frozen_after_append() {
 let tmp = TempDir::new().unwrap();
 let sidecar = tmp.path().join("workspace.atomic.json");
 let mut store = AtomicStore::new();

 append_changelog_entry(
 &mut store,
 &sidecar,
 "Round 162",
 Some("first"),
 &["change A".into()],
 &["verify A".into()],
 &[],
 &[],
 )
 .unwrap();

 let result = append_changelog_entry(
 &mut store,
 &sidecar,
 "Round 162",
 Some("attempted overwrite"),
 &[],
 &[],
 &[],
 &[],
 );
 assert!(
 result.is_err(),
 "second append to same entry_id must fail (T2 frozen ledger)"
 );
}
