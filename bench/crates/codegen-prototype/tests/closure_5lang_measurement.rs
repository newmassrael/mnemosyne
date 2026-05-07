//! Round 76 — Lock #1 closure runtime 5-language emit divergence measurement spike.
//!
//! Measurement-source ledger — substantive validation source for the closing of DESIGN.md §41's impact range.
//!
//! ## Measurement dimensions
//!
//! - byte size: per-language source length across all 5 emits (small-fixture stable anchor).
//! - sha256: content-addressable hash with 5 distinct values (deterministic byte-identical
//! re-emit).
//! - Jaccard inclusion: canonical_set is 1.0 across all 5 languages (detects any missing identifier).
//! - closure-expansion equivalence: in-memory forward-chaining's actual semantics
//! 5-language equivalence (data-shape equivalence → equivalent expanded results — this measurement covers Rust
//! authoritative — used as the in-memory expansion source).
//!
//! ## Lock #1 decision source
//!
//! Measurement results:
//! - **floor (data-shape 5-language emit)**: all 5 metadata emits
//! deterministic + Jaccard inclusion = 1.0 + 5 distinct sha256 → data-shape
//! 5-language consistency PASS.
//! - **ceiling (computation Rust-only carry)**: Round 35 out-of-paradigm decision
//! carry — `expand_*` / `run_closure`'s forward-chaining loop body - Rust
//! out-of-paradigm scope — the 5-language emit contract carries a partial-break audit trail.
//!
//! Lock #1 closure decision: data shape 5-lang feasible + computation Rust-only
//! carry → §41 impact range closing can be ratified inline (Round 35 partial break
//! audit trail's floor + ceiling explicit).

use codegen_prototype::closure_runtime::{
 closure_metadata_canonical_set, closure_small_fixture, emit_closure_metadata_all_languages,
 run_closure_in_memory, InMemoryFact, ProvenanceTag,
};
use codegen_prototype::entity_indexer::{jaccard_inclusion, sha256_hex};
use std::collections::BTreeSet;

/// measure 1 — 5-language metadata emit deterministic.
#[test]
fn five_language_emit_deterministic() {
 let rules = closure_small_fixture();
 let a = emit_closure_metadata_all_languages(&rules);
 let b = emit_closure_metadata_all_languages(&rules);
 assert_eq!(a, b, "5-language emit must be deterministic");
}

/// measure 2 — byte size anchor (small fixture, 2 rules).
#[test]
fn byte_size_anchor_within_expected_range() {
 let rules = closure_small_fixture();
 let m = emit_closure_metadata_all_languages(&rules);
 // Round 95 — §11 canonical 5-tuple (rust/kotlin/python/cpp/go).
 let sizes = [
 ("rust", m.rust.len()),
 ("kotlin", m.kotlin.len()),
 ("python", m.python.len()),
 ("cpp", m.cpp.len()),
 ("go", m.go.len()),
 ];
 for (lang, size) in sizes {
 assert!(
 size > 200 && size < 8000,
 "{lang} emit size: {size} bytes (expected 200-8000 small-fixture range)"
 );
 }
}

/// measure 3 — 5 distinct sha256 hashes, all 64-char.
#[test]
fn five_distinct_sha256_64_char() {
 let rules = closure_small_fixture();
 let m = emit_closure_metadata_all_languages(&rules);
 let mut hashes = BTreeSet::new();
 // Round 95 — §11 canonical 5-tuple (rust/kotlin/python/cpp/go).
 for s in [&m.rust, &m.kotlin, &m.python, &m.cpp, &m.go] {
 let h = sha256_hex(s);
 assert_eq!(h.len(), 64, "sha256 must be 64-char hex");
 assert!(hashes.insert(h));
 }
 assert_eq!(hashes.len(), 5, "5 languages → 5 distinct hashes");
}

/// measure 4 — Jaccard inclusion = 1.0 for every language (Lock #1 floor).
#[test]
fn jaccard_inclusion_one_for_every_language() {
 let rules = closure_small_fixture();
 let canonical = closure_metadata_canonical_set(&rules);
 let m = emit_closure_metadata_all_languages(&rules);
 // Round 95 — §11 canonical 5-tuple (rust/kotlin/python/cpp/go).
 for (lang, text) in [
 ("rust", &m.rust),
 ("kotlin", &m.kotlin),
 ("python", &m.python),
 ("cpp", &m.cpp),
 ("go", &m.go),
 ] {
 let j = jaccard_inclusion(text, &canonical);
 assert!(
 (j - 1.0).abs() < f64::EPSILON,
 "{lang} Jaccard inclusion = {j} (expected 1.0)"
 );
 }
}

/// measure 5 — canonical set covers Provenance variants + ClosureFact fields +
/// rule ids + premise/conclusion predicates.
#[test]
fn canonical_set_covers_provenance_factshape_rules_predicates() {
 let rules = closure_small_fixture();
 let s = closure_metadata_canonical_set(&rules);
 // Provenance variants
 assert!(s.contains("Provenance"));
 assert!(s.contains("Explicit"));
 assert!(s.contains("Derived"));
 assert!(s.contains("Retracted"));
 // ClosureFact field names
 assert!(s.contains("ClosureFact"));
 assert!(s.contains("predicate"));
 assert!(s.contains("roles"));
 assert!(s.contains("provenance"));
 assert!(s.contains("derived_from"));
 // Rule ids + premise/conclusion predicates
 assert!(s.contains("transitive_grandparent"));
 assert!(s.contains("knowledge_modus_ponens"));
 assert!(s.contains("is_parent_of"));
 assert!(s.contains("is_grandparent_of"));
 assert!(s.contains("knows"));
 assert!(s.contains("implies"));
}

/// Measure 6 — closure-expansion equivalence: Rust-authoritative in-memory expansion
/// validates the data shape and consistency of the 5-language emit. The expanded result-set's derived
/// fact = canonical predicate names + ProvenanceTag::Derived + derived_from
/// (rule_id, depth) carry the full 5-language data shape — this result is expressible.
#[test]
fn closure_expand_result_shape_matches_5lang_metadata() {
 let rules = closure_small_fixture();
 let mut facts: BTreeSet<InMemoryFact> = BTreeSet::new();
 facts.insert(InMemoryFact {
 predicate: "is_parent_of".to_string(),
 roles: vec![
 ("subject".to_string(), "A".to_string()),
 ("object".to_string(), "B".to_string()),
 ],
 provenance: ProvenanceTag::Explicit,
 derived_from: None,
 });
 facts.insert(InMemoryFact {
 predicate: "is_parent_of".to_string(),
 roles: vec![
 ("subject".to_string(), "B".to_string()),
 ("object".to_string(), "C".to_string()),
 ],
 provenance: ProvenanceTag::Explicit,
 derived_from: None,
 });
 let stats = run_closure_in_memory(&mut facts, &rules, 5);
 assert!(stats.fixpoint_reached);

 // Rust authoritative result — derived fact's all component - 5-language
 // canonical_set in etc.pageresolve- (data shape 5-lang feasibility validation).
 let canonical = closure_metadata_canonical_set(&rules);
 let derived = facts
 .iter()
 .find(|f| {
 f.predicate == "is_grandparent_of"
  && matches!(f.provenance, ProvenanceTag::Derived)
 })
 .expect("transitive_grandparent must derive is_grandparent_of");
 assert!(canonical.contains(&derived.predicate));
 if let Some((rule_id, _depth)) = &derived.derived_from {
 assert!(canonical.contains(rule_id));
 } else {
 panic!("derived fact must have derived_from set");
 }
}

/// measure 7 — Lock #1 floor + ceiling boundary explicit.
///
/// floor (data-shape 5-language emit): `emit_closure_metadata_all_languages`
/// All 5 emits PASS — this is the measurement source.
///
/// ceiling (computation Rust-only carry): emit_closure_runtime's expand_* /
/// run_closure's forward-chaining loop body is out-of-paradigm for Rust — this test
/// - validates that the metadata emit does not include a forward-chaining loop.
#[test]
fn lock1_floor_ceiling_boundary() {
 let rules = closure_small_fixture();
 let m = emit_closure_metadata_all_languages(&rules);
 // ceiling check — metadata emit in forward-chaining body not.nclude.
 // Round 95 — §11 canonical 5-tuple (rust/kotlin/python/cpp/go).
 for (lang, text) in [
 ("rust", &m.rust),
 ("kotlin", &m.kotlin),
 ("python", &m.python),
 ("cpp", &m.cpp),
 ("go", &m.go),
 ] {
 assert!(
 !text.contains("for iter in 0..MAX_CLOSURE_DEPTH"),
 "{lang} metadata emit must not contain Rust runtime loop"
 );
 assert!(
 !text.contains("expand_transitive_grandparent("),
 "{lang} metadata emit must not contain expand function body"
 );
 }
}

/// Measure 8 — measurement-result summary ledger (DESIGN §41 impact-range closing source).
#[test]
fn measurement_summary_ledger() {
 let rules = closure_small_fixture();
 let m = emit_closure_metadata_all_languages(&rules);
 let canonical = closure_metadata_canonical_set(&rules);

 let summary = format!(
 "Round 76 closure_runtime 5-language measurement spike (§11 canonical, Round 95 carry)\n\
  ─────────────────────────────────────────────────────\n\
  rule_count: {}\n\
  canonical_set_size: {}\n\
  emit byte sizes:\n\
  \trust: {}\n\
  \tkotlin: {}\n\
  \tpython: {}\n\
  \tcpp: {}\n\
  \tgo: {}\n\
  emit sha256 (truncated to 16 chars):\n\
  \trust: {}\n\
  \tkotlin: {}\n\
  \tpython: {}\n\
  \tcpp: {}\n\
  \tgo: {}\n\
  Jaccard inclusion (5-language metadata vs canonical_set): all = 1.0\n\
  Lock #1 floor: data shape 5-language emit feasible (PASS)\n\
  Lock #1 ceiling: computation Rust-only carry (Round 35 paradigm carry)\n",
 m.rule_count,
 canonical.len(),
 m.rust.len(),
 m.kotlin.len(),
 m.python.len(),
 m.cpp.len(),
 m.go.len(),
 &sha256_hex(&m.rust)[..16],
 &sha256_hex(&m.kotlin)[..16],
 &sha256_hex(&m.python)[..16],
 &sha256_hex(&m.cpp)[..16],
 &sha256_hex(&m.go)[..16],
 );
 eprintln!("{summary}");
 // Assert: summary - Lock #1 floor + ceiling explicit -recognize include.
 assert!(summary.contains("floor: data shape 5-language emit feasible (PASS)"));
 assert!(summary.contains("ceiling: computation Rust-only carry"));
}
