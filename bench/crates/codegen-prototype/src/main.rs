//! Round 43 measurement-data producer — runs the §39 entity-indexer prototype and dumps measurement data.
//!
//! This binary is the Round 44 design-round ratify source — validates emit byte-identical
//! + small fixture size + composite key encoding sample.
//!
//! Direct-bench pattern equivalent of Phase -1A's measurement spike — measurement-source output format.

use codegen_prototype::cf_wrapper::{
 cf_wrapper_canonical_set, check_direct_impl_signature_match, default_layout,
 emit_partial_languages, emit_wrapper,
};
use codegen_prototype::closure_runtime::{
 closure_small_fixture, emit_closure_runtime, run_closure_in_memory, InMemoryFact,
 ProvenanceTag,
};
use codegen_prototype::entity_indexer::{
 canonical_identifier_set, decode_composite_key, design_doc_schema_fixture, emit_all_languages,
 emit_rust, encode_composite_key, jaccard_inclusion, sha256_hex, KEY_LEN,
};
use codegen_prototype::markdown_export::{compare_typed_facts, emit_markdown, to_github_anchor};
use codegen_prototype::markdown_import::{
 design_doc_small_fixture as md_design_doc_small_fixture, parse_markdown, parsed_doc_canonical,
 RefKind,
};
use codegen_prototype::salsa_wire::{
 cascade_metadata_canonical_set, design_doc_cascade_fixture, emit_cascade_metadata_all_languages,
 emit_salsa_wire,
};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

fn main() {
 println!("=== Round 43 §39 entity-indexer prototype measure data ===");
 println!();

 // 1. §66 design_doc fixture emit
 let spec = design_doc_schema_fixture();
 let emitted = emit_rust(&spec);
 let size = emitted.source.len();
 let mut hasher = DefaultHasher::new();
 emitted.source.hash(&mut hasher);
 let source_hash = hasher.finish();

 println!("[measure 1] §66 design_doc schema fixture emit:");
 println!(" - entities count: {}", spec.entities.len());
 println!(" - relations count: {}", spec.relations.len());
 println!(" - emitted source size: {} bytes", size);
 println!(" - emitted source hash (DefaultHasher): 0x{:016x}", source_hash);
 println!();

 // 2. emit determinism (re.xecute → identical hash)
 let emitted2 = emit_rust(&spec);
 let mut hasher2 = DefaultHasher::new();
 emitted2.source.hash(&mut hasher2);
 let source_hash2 = hasher2.finish();
 println!(
 "[measure 2] emit determinism: {} (hash {} == {})",
 if source_hash == source_hash2 {
 "PASS"
 } else {
 "FAIL"
 },
 format_args!("0x{:016x}", source_hash),
 format_args!("0x{:016x}", source_hash2)
 );
 println!();

 // 3. 24 B fixed-width BE composite key encoding sample
 let samples: &[(u64, u64, u64)] = &[
 (0, 0, 0),
 (1, 100, 1_000_000_000),
 (u64::MAX / 2, u64::MAX / 2, u64::MAX / 2),
 (u64::MAX, u64::MAX, u64::MAX),
 ];
 println!("[measure 3] 24 B fixed-width BE composite key encoding:");
 for &(b, e, v) in samples {
 let buf = encode_composite_key(b, e, v);
 let (b2, e2, v2) = decode_composite_key(&buf);
 let round_trip = (b, e, v) == (b2, e2, v2);
 println!(
 " - encode({:>20}, {:>20}, {:>20}) → {} ({} B), round-trip: {}",
 b,
 e,
 v,
 hex(&buf),
 buf.len(),
 if round_trip { "PASS" } else { "FAIL" }
 );
 }
 println!();
 println!("[measure 4] KEY_LEN constant: {} (expected 24, DESIGN §18 line 1845 carry)", KEY_LEN);
 println!();

 // 5. emit source first ~480 byte preview (UTF-8 char-boundary safe)
 let preview = char_boundary_prefix(&emitted.source, 480);
 println!("[measure 5] emit source preview (first ~480 bytes):");
 println!("---");
 println!("{preview}");
 println!("---");
 println!();

 println!("=== Round 43 measure data produce complete ===");
 println!("source-of-truth: bench/crates/codegen-prototype/src/entity_indexer.rs");
 println!("design round ratify: Round 44 (previous round handle complete)");
 println!();

 // ────────────────────────────────────────────────────────────────────
 // Round 45 §42 RocksDB CF runtime wrapper prototype measure data
 // ────────────────────────────────────────────────────────────────────
 println!("=== Round 45 §42 cf_wrapper prototype measure data ===");
 println!();

 let layout = default_layout(&spec);
 let wrapper = emit_wrapper(&spec, &layout);
 let mut hasher_w = DefaultHasher::new();
 wrapper.source.hash(&mut hasher_w);
 let wrapper_hash = hasher_w.finish();

 // 1. CF layout mapping data
 println!("[measure 1] CF layout (default_layout, GraphSpec → CfMeta mapping):");
 println!(" - entity CF count: {}", layout.entities.len());
 println!(" - relation CF count: {}", layout.relations.len());
 for (name, meta) in layout.entities.iter().chain(layout.relations.iter()) {
 println!(
 " {} → CF \"{}\" (iter={:?}, secondary={}, version={})",
 name, meta.name, meta.iter_pattern, meta.secondary_readable, meta.schema_version
 );
 }
 println!();

 // 2. typed CRUD wrapper emit
 println!("[measure 2] typed CRUD wrapper emit (Round 45):");
 println!(" - source size: {} bytes", wrapper.source.len());
 println!(" - source hash (DefaultHasher): 0x{:016x}", wrapper_hash);
 println!(" - generated method count: {}", wrapper.method_count);
 println!(" - cf descriptor count: {}", wrapper.cf_count);
 println!(" - secondary-readable subset count: {}", wrapper.secondary_cf_count);
 println!();

 // 3. emit determinism
 let wrapper2 = emit_wrapper(&spec, &layout);
 let mut hasher_w2 = DefaultHasher::new();
 wrapper2.source.hash(&mut hasher_w2);
 let wrapper_hash2 = hasher_w2.finish();
 println!(
 "[measure 3] cf_wrapper emit determinism: {} (hash 0x{:016x} == 0x{:016x})",
 if wrapper_hash == wrapper_hash2 { "PASS" } else { "FAIL" },
 wrapper_hash,
 wrapper_hash2
 );
 println!();

 // 4. direct-impl signature consistency (manual BranchStore and's mapping)
 let sig_check = check_direct_impl_signature_match();
 println!("[measure 4] direct-impl signature consistency (manual BranchStore ↔ codegen wrapper):");
 println!(
 " - codegen emit methods: {:?}",
 sig_check.emitted_methods
 );
 println!(
 " - direct-impl BranchStore methods: {:?}",
 sig_check.direct_impl_methods
 );
 println!(
 " - signature shape match: {}",
 if sig_check.signature_match { "PASS" } else { "FAIL" }
 );
 println!();

 // 5. emitted source preview (first ~480 bytes, UTF-8 char-boundary safe)
 let w_preview = char_boundary_prefix(&wrapper.source, 480);
 println!("[measure 5] cf_wrapper emit source preview (first ~480 bytes):");
 println!("---");
 println!("{w_preview}");
 println!("---");
 println!();

 println!("=== Round 45 measure data produce complete ===");
 println!("source-of-truth: bench/crates/codegen-prototype/src/cf_wrapper.rs");
 println!("design round ratify: Round 46 (previous round handle complete)");
 println!();

 // ────────────────────────────────────────────────────────────────────
 // Round 47 §43 Salsa wire codegen prototype measure data
 // ────────────────────────────────────────────────────────────────────
 println!("=== Round 47 §43 salsa_wire prototype measure data ===");
 println!();

 let cascade = design_doc_cascade_fixture();
 let salsa = emit_salsa_wire(&cascade, &spec, &layout);
 let mut hasher_s = DefaultHasher::new();
 salsa.source.hash(&mut hasher_s);
 let salsa_hash = hasher_s.finish();

 // 1. cascade fixture metadata
 println!("[measure 1] §66 design_doc cascade fixture (small-scale validation):");
 println!(" - cascade query count: {}", cascade.queries.len());
 for q in &cascade.queries {
 println!(
 " {} → {} (ordering={}, reads={}, triggers={})",
 q.name,
 q.output,
 q.ordering,
 q.reads.len(),
 q.triggers.len()
 );
 }
 println!();

 // 2. salsa_wire emit
 println!("[measure 2] Salsa wire emit (Round 47):");
 println!(" - source size: {} bytes", salsa.source.len());
 println!(" - source hash (DefaultHasher): 0x{:016x}", salsa_hash);
 println!(" - tracked_fn count: {}", salsa.tracked_fn_count);
 println!(" - database trait method count: {}", salsa.db_method_count);
 println!(" - cascade dependency edge count: {}", salsa.dep_edge_count);
 println!();

 // 3. emit determinism
 let salsa2 = emit_salsa_wire(&cascade, &spec, &layout);
 let mut hasher_s2 = DefaultHasher::new();
 salsa2.source.hash(&mut hasher_s2);
 let salsa_hash2 = hasher_s2.finish();
 println!(
 "[measure 3] salsa_wire emit determinism: {} (hash 0x{:016x} == 0x{:016x})",
 if salsa_hash == salsa_hash2 { "PASS" } else { "FAIL" },
 salsa_hash,
 salsa_hash2
 );
 println!();

 // 4. small cascade query small-scale validation
 let has_tracked = salsa.source.contains("#[salsa::tracked]");
 let has_db_trait = salsa.source.contains("pub trait CascadeDb: Database");
 let has_dep_graph = salsa.source.contains("pub fn cascade_dependency_edges()");
 let has_ordering = salsa.source.contains("pub fn cascade_orderings()");
 println!("[measure 4] small cascade query validation:");
 println!(" - #[salsa::tracked] emit: {}", if has_tracked { "PASS" } else { "FAIL" });
 println!(" - CascadeDb trait emit: {}", if has_db_trait { "PASS" } else { "FAIL" });
 println!(" - dependency graph metadata emit: {}", if has_dep_graph { "PASS" } else { "FAIL" });
 println!(" - CascadeOrdering axis emit: {}", if has_ordering { "PASS" } else { "FAIL" });
 println!();

 // 5. salsa_wire emit source preview (first ~480 bytes, UTF-8 char-boundary safe)
 let s_preview = char_boundary_prefix(&salsa.source, 480);
 println!("[measure 5] salsa_wire emit source preview (first ~480 bytes):");
 println!("---");
 println!("{s_preview}");
 println!("---");
 println!();

 println!("=== Round 47 measure data produce complete ===");
 println!("source-of-truth: bench/crates/codegen-prototype/src/salsa_wire.rs");
 println!("design round ratify: Round 48 (next round)");
 println!();

 // ────────────────────────────────────────────────────────────────────
 // Round 52 §39 5-language emit + sha256 + cross-language Jaccard measure data
 // ────────────────────────────────────────────────────────────────────
 println!("=== Round 52 §39 5-language emit + sha256 measure data ===");
 println!();

 let multi = emit_all_languages(&spec);
 let h_rust = sha256_hex(&multi.rust);
 let h_kotlin = sha256_hex(&multi.kotlin);
 let h_python = sha256_hex(&multi.python);
 let h_cpp = sha256_hex(&multi.cpp);
 let h_proto = sha256_hex(&multi.protobuf);

 println!("[measure 1] 5-language emit byte size:");
 println!(" - rust: {:>5} bytes", multi.rust.len());
 println!(" - kotlin: {:>5} bytes", multi.kotlin.len());
 println!(" - python: {:>5} bytes", multi.python.len());
 println!(" - cpp: {:>5} bytes", multi.cpp.len());
 println!(" - protobuf: {:>5} bytes", multi.protobuf.len());
 println!();

 println!("[measure 2] sha256 hex digest (content-addressable, cross-process stable):");
 println!(" - rust: {h_rust}");
 println!(" - kotlin: {h_kotlin}");
 println!(" - python: {h_python}");
 println!(" - cpp: {h_cpp}");
 println!(" - protobuf: {h_proto}");
 println!();

 // 3. emit determinism — re.xecute then hash identical
 let multi2 = emit_all_languages(&spec);
 let h_rust2 = sha256_hex(&multi2.rust);
 let h_kotlin2 = sha256_hex(&multi2.kotlin);
 let h_python2 = sha256_hex(&multi2.python);
 let h_cpp2 = sha256_hex(&multi2.cpp);
 let h_proto2 = sha256_hex(&multi2.protobuf);
 let det = h_rust == h_rust2
 && h_kotlin == h_kotlin2
 && h_python == h_python2
 && h_cpp == h_cpp2
 && h_proto == h_proto2;
 println!(
 "[measure 3] 5-language emit determinism: {} (re.xecute 5 hash all identical)",
 if det { "PASS" } else { "FAIL" }
 );
 println!();

 // 4. Cross-language Jaccard inclusion = 1.0
 let canonical = canonical_identifier_set(&spec);
 let j_rust = jaccard_inclusion(&multi.rust, &canonical);
 let j_kotlin = jaccard_inclusion(&multi.kotlin, &canonical);
 let j_python = jaccard_inclusion(&multi.python, &canonical);
 let j_cpp = jaccard_inclusion(&multi.cpp, &canonical);
 let j_proto = jaccard_inclusion(&multi.protobuf, &canonical);
 println!(
 "[measure 4] Cross-language Jaccard inclusion (canonical identifier set size: {}):",
 canonical.len()
 );
 println!(" - rust: {j_rust:.4}");
 println!(" - kotlin: {j_kotlin:.4}");
 println!(" - python: {j_python:.4}");
 println!(" - cpp: {j_cpp:.4}");
 println!(" - protobuf: {j_proto:.4}");
 let j_pass = (j_rust - 1.0).abs() < f64::EPSILON
 && (j_kotlin - 1.0).abs() < f64::EPSILON
 && (j_python - 1.0).abs() < f64::EPSILON
 && (j_cpp - 1.0).abs() < f64::EPSILON
 && (j_proto - 1.0).abs() < f64::EPSILON;
 println!(
 " → ROADMAP *Cross-language conformance fixture* Jaccard = 1.0: {}",
 if j_pass { "PASS" } else { "FAIL" }
 );
 println!();

 // 5. DefaultHasher RandomState threshold break (sha256 stable cross-process)
 let mut h_rust_old = DefaultHasher::new();
 multi.rust.hash(&mut h_rust_old);
 println!(
 "[measure 5] DefaultHasher RandomState threshold break:\n - rust DefaultHasher (process-local): 0x{:016x}\n - rust sha256 (content-addressable): {h_rust}\n → sha256 cross-process stable, DefaultHasher - RandomState in process endmany change possible.",
 h_rust_old.finish()
 );
 println!();

 println!("=== Round 52 measure data produce complete ===");
 println!("source-of-truth: bench/crates/codegen-prototype/src/entity_indexer.rs (5-language emit functions)");
 println!("design round: Round 52 OPTION B-1 (single round, code phase + ratify unified)");
 println!();

 // ────────────────────────────────────────────────────────────────────
 // Round 53 §42 cf_wrapper Rust + C++ partial emit + sha256 measure data
 // (Round 36 out-of-paradigm decision carry — 5-language emit contract's *partial break*)
 // ────────────────────────────────────────────────────────────────────
 println!("=== Round 53 §42 cf_wrapper partial emit (Rust + C++) measure data ===");
 println!();

 let cf_partial = emit_partial_languages(&spec, &layout);
 let h_cf_rust = sha256_hex(&cf_partial.rust);
 let h_cf_cpp = sha256_hex(&cf_partial.cpp);

 println!("[measure 1] partial language emit byte size:");
 println!(" - rust: {:>5} bytes", cf_partial.rust.len());
 println!(" - cpp: {:>5} bytes", cf_partial.cpp.len());
 println!(
 " → partial languages: {:?} (Round 36 out-of-paradigm decision — Kotlin/Python/Protobuf not.mit)",
 cf_partial.partial_languages
 );
 println!();

 println!("[measure 2] sha256 hex digest (content-addressable):");
 println!(" - rust: {h_cf_rust}");
 println!(" - cpp: {h_cf_cpp}");
 println!();

 // 3. partial emit determinism
 let cf_partial2 = emit_partial_languages(&spec, &layout);
 let det_cf = sha256_hex(&cf_partial2.rust) == h_cf_rust && sha256_hex(&cf_partial2.cpp) == h_cf_cpp;
 println!(
 "[measure 3] partial emit determinism: {} (re.xecute 2 hash all identical)",
 if det_cf { "PASS" } else { "FAIL" }
 );
 println!();

 // 4. Partial language Jaccard inclusion = 1.0
 let cf_canonical = cf_wrapper_canonical_set(&spec, &layout);
 let j_cf_rust = jaccard_inclusion(&cf_partial.rust, &cf_canonical);
 let j_cf_cpp = jaccard_inclusion(&cf_partial.cpp, &cf_canonical);
 println!(
 "[measure 4] partial language Jaccard inclusion (canonical set size: {}):",
 cf_canonical.len()
 );
 println!(" - rust: {j_cf_rust:.4}");
 println!(" - cpp: {j_cf_cpp:.4}");
 let cf_pass = (j_cf_rust - 1.0).abs() < f64::EPSILON && (j_cf_cpp - 1.0).abs() < f64::EPSILON;
 println!(
 " → DESIGN §42 *Rationale* *Rust store layer + C++ runtime SDK only emit* contract in partial Jaccard = 1.0: {}",
 if cf_pass { "PASS" } else { "FAIL" }
 );
 println!();

 // 5. C++ readonly subset table if validation
 let cpp_has_readonly_class = cf_partial.cpp.contains("ReadOnlyCF");
 let cpp_has_secondary_array = cf_partial.cpp.contains("SECONDARY_READABLE_CFS");
 let cpp_no_migration = !cf_partial.cpp.contains("MigrationStub");
 println!("[measure 5] C++ runtime SDK partial emit table if validation (DESIGN §42 *output* --th item):");
 println!(
 " - per-CF ReadOnlyCF class emit: {}",
 if cpp_has_readonly_class { "PASS" } else { "FAIL" }
 );
 println!(
 " - SECONDARY_READABLE_CFS array emit: {}",
 if cpp_has_secondary_array { "PASS" } else { "FAIL" }
 );
 println!(
 " - migration stub Rust-only carry (C++ not.nclude): {}",
 if cpp_no_migration { "PASS" } else { "FAIL" }
 );
 println!();

 println!("=== Round 53 measure data produce complete ===");
 println!("source-of-truth: bench/crates/codegen-prototype/src/cf_wrapper.rs (emit_cpp_readonly)");
 println!("design round: Round 53 OPTION B-2 (single round, code phase + ratify unified)");
 println!();

 // ────────────────────────────────────────────────────────────────────
 // Round 54 §43 cascade dependency graph metadata 5-language emit measure data
 // (Salsa runtime Rust-only carry, metadata only 5-language)
 // ────────────────────────────────────────────────────────────────────
 println!("=== Round 54 §43 cascade metadata 5-language emit measure data ===");
 println!();

 let cm = emit_cascade_metadata_all_languages(&cascade);
 let h_cm_rust = sha256_hex(&cm.rust);
 let h_cm_kotlin = sha256_hex(&cm.kotlin);
 let h_cm_python = sha256_hex(&cm.python);
 let h_cm_cpp = sha256_hex(&cm.cpp);
 let h_cm_proto = sha256_hex(&cm.protobuf);

 println!("[measure 1] cascade metadata 5-language emit byte size:");
 println!(" - rust: {:>5} bytes", cm.rust.len());
 println!(" - kotlin: {:>5} bytes", cm.kotlin.len());
 println!(" - python: {:>5} bytes", cm.python.len());
 println!(" - cpp: {:>5} bytes", cm.cpp.len());
 println!(" - protobuf: {:>5} bytes", cm.protobuf.len());
 println!(
 " → edge count {} / query count {} (Salsa runtime Rust-only carry, metadata-only 5-language)",
 cm.edge_count, cm.query_count
 );
 println!();

 println!("[measure 2] sha256 hex digest:");
 println!(" - rust: {h_cm_rust}");
 println!(" - kotlin: {h_cm_kotlin}");
 println!(" - python: {h_cm_python}");
 println!(" - cpp: {h_cm_cpp}");
 println!(" - protobuf: {h_cm_proto}");
 println!();

 // 3. metadata emit determinism
 let cm2 = emit_cascade_metadata_all_languages(&cascade);
 let det_cm = sha256_hex(&cm2.rust) == h_cm_rust
 && sha256_hex(&cm2.kotlin) == h_cm_kotlin
 && sha256_hex(&cm2.python) == h_cm_python
 && sha256_hex(&cm2.cpp) == h_cm_cpp
 && sha256_hex(&cm2.protobuf) == h_cm_proto;
 println!(
 "[measure 3] cascade metadata emit determinism: {} (re.xecute 5 hash all identical)",
 if det_cm { "PASS" } else { "FAIL" }
 );
 println!();

 // 4. Cross-language Jaccard inclusion = 1.0
 let cm_canonical = cascade_metadata_canonical_set(&cascade);
 let j_cm_rust = jaccard_inclusion(&cm.rust, &cm_canonical);
 let j_cm_kotlin = jaccard_inclusion(&cm.kotlin, &cm_canonical);
 let j_cm_python = jaccard_inclusion(&cm.python, &cm_canonical);
 let j_cm_cpp = jaccard_inclusion(&cm.cpp, &cm_canonical);
 let j_cm_proto = jaccard_inclusion(&cm.protobuf, &cm_canonical);
 println!(
 "[measure 4] cascade metadata cross-language Jaccard inclusion (canonical set size: {}):",
 cm_canonical.len()
 );
 println!(" - rust: {j_cm_rust:.4}");
 println!(" - kotlin: {j_cm_kotlin:.4}");
 println!(" - python: {j_cm_python:.4}");
 println!(" - cpp: {j_cm_cpp:.4}");
 println!(" - protobuf: {j_cm_proto:.4}");
 let cm_pass = (j_cm_rust - 1.0).abs() < f64::EPSILON
 && (j_cm_kotlin - 1.0).abs() < f64::EPSILON
 && (j_cm_python - 1.0).abs() < f64::EPSILON
 && (j_cm_cpp - 1.0).abs() < f64::EPSILON
 && (j_cm_proto - 1.0).abs() < f64::EPSILON;
 println!(
 " → cascade metadata 5-language Jaccard = 1.0: {}",
 if cm_pass { "PASS" } else { "FAIL" }
 );
 println!();

 // 5. Salsa runtime Rust-only carry — 4 non-Rust emit in Salsa runtime attribute not.nclude validation
 let runtime_excluded = ["kotlin", "python", "cpp", "protobuf"]
 .iter()
 .zip([&cm.kotlin, &cm.python, &cm.cpp, &cm.protobuf])
 .all(|(_, e)| !e.contains("#[salsa::input]") && !e.contains("#[salsa::tracked]") && !e.contains("CascadeDb"));
 println!(
 "[measure 5] Salsa runtime Rust-only carry boundary validation: {} (Kotlin/Python/C++/Protobuf metadata emit in #[salsa::*] / CascadeDb not.nclude)",
 if runtime_excluded { "PASS" } else { "FAIL" }
 );
 println!();

 println!("=== Round 54 measure data produce complete ===");
 println!("source-of-truth: bench/crates/codegen-prototype/src/salsa_wire.rs (emit_cascade_metadata_all_languages)");
 println!("design round: Round 54 OPTION B-3 (single round, code phase + ratify unified — OPTION B sequence end)");
 println!();

 // ────────────────────────────────────────────────────────────────────
 // Round 56 §41 closure_runtime fallback path Rust-only emit + closure expand measure
 // (Round 35 out-of-paradigm decision carry — Rust-only, 5-language contract partial break)
 // ────────────────────────────────────────────────────────────────────
 println!("=== Round 56 §41 closure_runtime Rust-only emit + closure expand measure data ===");
 println!();

 let cl_rules = closure_small_fixture();
 let cl_emitted = emit_closure_runtime(&cl_rules);
 let h_cl = sha256_hex(&cl_emitted.source);

 println!("[measure 1] §11 small fixture (closure_small_fixture):");
 println!(" - rule count: {}", cl_emitted.rule_count);
 println!(" - max depth: {}", cl_emitted.max_depth);
 for r in &cl_rules {
 println!(
 " {} (depth {}) — {} premises ⇒ {}",
 r.id,
 r.depth,
 r.premises.len(),
 r.conclusion.predicate
 );
 }
 println!();

 println!("[measure 2] closure_runtime emit (Rust-only, Round 35 out-of-paradigm decision):");
 println!(" - source size: {} bytes", cl_emitted.source.len());
 println!(" - sha256: {h_cl}");
 println!();

 // 3. closure_runtime emit determinism
 let cl_emitted2 = emit_closure_runtime(&cl_rules);
 let det_cl = sha256_hex(&cl_emitted2.source) == h_cl;
 println!(
 "[measure 3] closure_runtime emit determinism: {} (re.xecute hash identical)",
 if det_cl { "PASS" } else { "FAIL" }
 );
 println!();

 // 4. closure expand consistency validation — transitive_grandparent + knowledge_modus_ponens
 let mut facts = std::collections::BTreeSet::new();
 // Family lineage: A → B → C → D (3 explicit is_parent_of)
 for (s, o) in [("A", "B"), ("B", "C"), ("C", "D")] {
 facts.insert(InMemoryFact {
 predicate: "is_parent_of".to_string(),
 roles: vec![
  ("subject".to_string(), s.to_string()),
  ("object".to_string(), o.to_string()),
 ],
 provenance: ProvenanceTag::Explicit,
 derived_from: None,
 });
 }
 // Knowledge: alice knows F1, F1 implies G1, G1 implies H1 (chain).
 facts.insert(InMemoryFact {
 predicate: "knows".to_string(),
 roles: vec![
 ("agent".to_string(), "alice".to_string()),
 ("fact".to_string(), "F1".to_string()),
 ],
 provenance: ProvenanceTag::Explicit,
 derived_from: None,
 });
 for (a, c) in [("F1", "G1"), ("G1", "H1")] {
 facts.insert(InMemoryFact {
 predicate: "implies".to_string(),
 roles: vec![
  ("antecedent".to_string(), a.to_string()),
  ("consequent".to_string(), c.to_string()),
 ],
 provenance: ProvenanceTag::Explicit,
 derived_from: None,
 });
 }
 let explicit_count = facts.len();
 let stats = run_closure_in_memory(&mut facts, &cl_rules, 5);
 let derived_count = facts.iter().filter(|f| f.provenance == ProvenanceTag::Derived).count();
 println!("[measure 4] closure expand consistency (small fixture × bounded forward-chaining):");
 println!(" - explicit facts (input): {}", explicit_count);
 println!(" - derived facts (after run): {}", derived_count);
 println!(
 " - iterations:  {} (fixpoint_reached: {})",
 stats.iterations, stats.fixpoint_reached
 );
 println!(" - derived added:  {}", stats.derived_added);
 let has_grandparent_ac = facts.iter().any(|f| {
 f.predicate == "is_grandparent_of"
 && f.roles.iter().any(|(r, v)| r == "subject" && v == "A")
 && f.roles.iter().any(|(r, v)| r == "object" && v == "C")
 });
 let has_alice_h1 = facts.iter().any(|f| {
 f.predicate == "knows"
 && f.roles.iter().any(|(r, v)| r == "agent" && v == "alice")
 && f.roles.iter().any(|(r, v)| r == "fact" && v == "H1")
 });
 println!(
 " - transitive_grandparent (A,C): {}",
 if has_grandparent_ac { "DERIVED" } else { "MISSING" }
 );
 println!(
 " - knowledge chain alice → H1: {}",
 if has_alice_h1 { "DERIVED" } else { "MISSING" }
 );
 println!();

 // 5. Rust-only emit boundary validation (Round 35 out-of-paradigm decision carry)
 let no_kotlin = !cl_emitted.source.contains("data class");
 let no_python = !cl_emitted.source.contains("@dataclass");
 let no_cpp_namespace = !cl_emitted.source.contains("namespace mnemosyne::");
 let no_proto = !cl_emitted.source.contains("message ");
 let rust_marker = cl_emitted.source.contains("Rust-only emit");
 let boundary_pass = no_kotlin && no_python && no_cpp_namespace && no_proto && rust_marker;
 println!("[measure 5] Rust-only emit boundary validation (Round 35 out-of-paradigm decision):");
 println!(" - no Kotlin (data class): {}", if no_kotlin { "PASS" } else { "FAIL" });
 println!(" - no Python (@dataclass): {}", if no_python { "PASS" } else { "FAIL" });
 println!(" - no C++ (namespace mnemosyne): {}", if no_cpp_namespace { "PASS" } else { "FAIL" });
 println!(" - no Protobuf (message): {}", if no_proto { "PASS" } else { "FAIL" });
 println!(" - Rust-only marker comment: {}", if rust_marker { "PASS" } else { "FAIL" });
 println!(
 " → 5-language emit contract's *closure scope partial break* audit trail PASS: {}",
 if boundary_pass { "PASS" } else { "FAIL" }
 );
 println!();

 println!("=== Round 56 measure data produce complete ===");
 println!("source-of-truth: bench/crates/codegen-prototype/src/closure_runtime.rs");
 println!("design round ratify: Round 57 (next round)");
 println!();

 // ────────────────────────────────────────────────────────────────────
 // Round 62 §61 markdown_import prototype — markdown → typed facts parse measure
 // (Round 60 §39 closed-form registered carry, Round 61 framing update carry)
 // ────────────────────────────────────────────────────────────────────
 println!("=== Round 62 §61 markdown_import prototype measure data ===");
 println!();

 let md_fixture = md_design_doc_small_fixture();
 let parsed = parse_markdown(md_fixture, "DESIGN.md");
 let canonical_text = parsed_doc_canonical(&parsed);
 let parsed_hash = sha256_hex(&canonical_text);

 // 1. small fixture metadata
 println!("[measure 1] §61 small fixture (design_doc_small_fixture):");
 println!(" - input markdown size: {} bytes", md_fixture.len());
 println!(" - input markdown lines: {}", md_fixture.lines().count());
 println!();

 // 2. parsed typed facts state count
 println!("[measure 2] parsed typed facts state (DESIGN §39 closed-form 4 entity/relation):");
 println!(" - sections: {}", parsed.sections.len());
 println!(" - changelog_entries: {}", parsed.changelog_entries.len());
 println!(" - frozen_lists: {}", parsed.frozen_lists.len());
 println!(" - cross_refs:  {}", parsed.cross_refs.len());
 println!(" - warnings: {}", parsed.warnings.len());
 println!();

 // 3. parse determinism — re.xecute → identical ParsedDoc
 let parsed2 = parse_markdown(md_fixture, "DESIGN.md");
 let canonical_text2 = parsed_doc_canonical(&parsed2);
 let parsed_hash2 = sha256_hex(&canonical_text2);
 println!(
 "[measure 3] parse determinism: {} (re.xecute ParsedDoc identical + canonical sha256 identical)",
 if parsed == parsed2 && parsed_hash == parsed_hash2 {
 "PASS"
 } else {
 "FAIL"
 }
 );
 println!(" - canonical render size: {} bytes", canonical_text.len());
 println!(" - canonical sha256: {parsed_hash}");
 println!();

 // 4. typed facts shape validation (DESIGN §39 closed-form 4 entity/relation full shape)
 let h1_count = parsed
 .sections
 .iter()
 .filter(|s| s.parent_section.is_none())
 .count();
 let numbered_count = parsed
 .sections
 .iter()
 .filter(|s| s.section_id.chars().next().map_or(false, |c| c.is_ascii_digit()))
 .count();
 let unnumbered_count = parsed.sections.len() - numbered_count - h1_count;
 let cross_doc_refs = parsed
 .cross_refs
 .iter()
 .filter(|c| c.ref_kind == RefKind::CrossDoc)
 .count();
 let decision_refs = parsed
 .cross_refs
 .iter()
 .filter(|c| c.ref_kind == RefKind::Decision)
 .count();
 println!("[measure 4] typed facts shape (DESIGN §61 mapping table 13 row's subset validation):");
 println!(" - h1 doc-root section (parent=None):   {h1_count}");
 println!(" - numbered top-level Section (## N. ...):  {numbered_count}");
 println!(" - unnumbered Section (## Changelog / unnumbered / etc): {unnumbered_count}");
 println!(" - decision-kind CrossRef (§N inline literal): {decision_refs}");
 println!(" - cross_doc-kind CrossRef ([text](other.md#anchor)): {cross_doc_refs}");
 println!();

 // 5. ChangelogEntry append-only + frozen_at_transaction_time monotonic validation
 let tt_monotonic = parsed
 .changelog_entries
 .windows(2)
 .all(|w| w[0].frozen_at_transaction_time < w[1].frozen_at_transaction_time);
 let entry_ids: Vec<&str> = parsed
 .changelog_entries
 .iter()
 .map(|e| e.entry_id.as_str())
 .collect();
 println!("[measure 5] ChangelogEntry append-only semantics (DESIGN §39 closed-form carry):");
 println!(" - entry_id list:  {entry_ids:?}");
 println!(
 " - frozen_at_transaction_time monotonic: {}",
 if tt_monotonic { "PASS" } else { "FAIL" }
 );
 println!(
 " - sub_bullets total count:  {}",
 parsed
 .changelog_entries
 .iter()
 .map(|e| e.sub_bullets.len())
 .sum::<usize>()
 );
 println!();

 // 6. canonical render preview (first ~480 bytes, UTF-8 char-boundary safe)
 let preview = char_boundary_prefix(&canonical_text, 480);
 println!("[measure 6] canonical render preview (first ~480 bytes):");
 println!("---");
 println!("{preview}");
 println!("---");
 println!();

 // 7. Phase 0 prerequisite #1 small fixture parse feasibility validation (Round 61 framing carry)
 let prereq1_pass = !parsed.sections.is_empty()
 && !parsed.changelog_entries.is_empty()
 && !parsed.cross_refs.is_empty();
 println!(
 "[measure 7] Phase 0 prerequisite #1 (§61 markdown import) small fixture parse feasibility validation: {}",
 if prereq1_pass { "PASS" } else { "FAIL" }
 );
 println!(
 " → DESIGN §61 *Phase 0 implementation entry prerequisite framing* (Round 61)'s *small fixture validation feasibility validation source* carry"
 );
 println!();

 println!("=== Round 62 measure data produce complete ===");
 println!("source-of-truth: bench/crates/codegen-prototype/src/markdown_import.rs");
 println!("design round ratify: Round 63 (next round, OPTION E-3)");
 println!();

 // ────────────────────────────────────────────────────────────────────
 // Round 64 §56 markdown_export prototype — typed facts → markdown emit
 // + round-trip integrity validation (parse → emit → re-parse → diff = ∅)
 // (Round 60 §39 closed-form registered carry, Round 61 framing update carry,
 // Round 62 markdown_import's *symmetric sym* in emit-side prototype)
 // ────────────────────────────────────────────────────────────────────
 println!("=== Round 64 §56 markdown_export prototype measure data ===");
 println!();

 let emitted_md = emit_markdown(&parsed);
 let emitted_hash = sha256_hex(&emitted_md);

 // 1. emit metadata
 println!("[measure 1] §56 typed facts → markdown emit (Round 62 ParsedDoc input):");
 println!(" - input ParsedDoc — sections={} / changelog_entries={} / cross_refs={}",
 parsed.sections.len(),
 parsed.changelog_entries.len(),
 parsed.cross_refs.len()
 );
 println!(" - emitted markdown size: {} bytes", emitted_md.len());
 println!(" - emitted markdown lines: {}", emitted_md.lines().count());
 println!(" - emitted sha256: {emitted_hash}");
 println!();

 // 2. emit determinism
 let emitted_md2 = emit_markdown(&parsed);
 let emitted_hash2 = sha256_hex(&emitted_md2);
 println!(
 "[measure 2] markdown_export emit determinism: {} (re.xecute markdown bytes identical + sha256 identical)",
 if emitted_md == emitted_md2 && emitted_hash == emitted_hash2 {
 "PASS"
 } else {
 "FAIL"
 }
 );
 println!();

 // 3. GitHub anchor algorithm sample (DESIGN §56 spec 3 step)
 let anchor_samples: &[(&str, &str)] = &[
 ("Phase 0", "phase-0"),
 ("60. Core / client boundary", "60--core---client-boundary"),
 ("Changelog", "change-history"),
 ("foo_bar-baz", "foo_bar-baz"),
 ];
 println!("[measure 3] GitHub anchor algorithm (DESIGN §56 spec 3 step):");
 let mut anchor_pass = true;
 for (input, expected) in anchor_samples {
 let actual = to_github_anchor(input);
 let ok = actual == *expected;
 if !ok {
 anchor_pass = false;
 }
 println!(
 " - to_github_anchor({:?}) → {:?} (expected {:?}): {}",
 input,
 actual,
 expected,
 if ok { "PASS" } else { "FAIL" }
 );
 }
 println!(
 " → DESIGN §56 GitHub anchor algorithm binding validation: {}",
 if anchor_pass { "PASS" } else { "FAIL" }
 );
 println!();

 // 4. round-trip integrity validation (parse → emit → re-parse → diff = ∅)
 let reparsed = parse_markdown(&emitted_md, "DESIGN.md");
 let diff = compare_typed_facts(&parsed, &reparsed);
 println!("[measure 4] round-trip integrity validation (DESIGN §61 *preserved mandatory dimension* carry):");
 println!(
 " - sections: {} → {} ({})",
 diff.section_count_a,
 diff.section_count_b,
 if diff.section_identity_match { "MATCH" } else { "DIFF" }
 );
 println!(
 " - changelog_entries: {} → {} ({})",
 diff.changelog_entry_count_a,
 diff.changelog_entry_count_b,
 if diff.changelog_sequence_match { "MATCH" } else { "DIFF" }
 );
 println!(
 " - cross_refs:  {} → {} ({})",
 diff.cross_ref_count_a,
 diff.cross_ref_count_b,
 if diff.cross_ref_set_match { "MATCH" } else { "DIFF" }
 );
 println!(
 " → mandatory preserved mandatory dimension (parse → emit → re-parse → diff = ∅): {}",
 if diff.mandatory_preserved { "PASS" } else { "FAIL" }
 );
 println!();

 // 5. emitted markdown preview (first ~480 bytes, UTF-8 char-boundary safe)
 let md_preview = char_boundary_prefix(&emitted_md, 480);
 println!("[measure 5] emitted markdown preview (first ~480 bytes):");
 println!("---");
 println!("{md_preview}");
 println!("---");
 println!();

 // 6. Phase 0 prerequisite #2 small fixture emit + round-trip feasibility validation
 let prereq2_pass = !emitted_md.is_empty() && diff.mandatory_preserved;
 println!(
 "[measure 6] Phase 0 prerequisite #2 (§56 markdown export) small fixture emit + round-trip feasibility validation: {}",
 if prereq2_pass { "PASS" } else { "FAIL" }
 );
 println!(
 " → DESIGN §56 *Phase 0 implementation entry prerequisite framing* (Round 61)'s *small fixture emit + round-trip validation feasibility validation source* carry"
 );
 println!();

 println!("=== Round 64 measure data produce complete ===");
 println!("source-of-truth: bench/crates/codegen-prototype/src/markdown_export.rs");
 println!("design round ratify: Round 65 (next round, OPTION E-5)");
}

fn hex(buf: &[u8]) -> String {
 let mut s = String::with_capacity(buf.len() * 2);
 for b in buf {
 s.push_str(&format!("{:02x}", b));
 }
 s
}

fn char_boundary_prefix(s: &str, max_bytes: usize) -> &str {
 if s.len() <= max_bytes {
 return s;
 }
 let mut end = max_bytes;
 while end > 0 && !s.is_char_boundary(end) {
 end -= 1;
 }
 &s[..end]
}
