//! §43 Salsa wire codegen prototype (Round 47 subsequent work scope).
//!
//! Round 47 substantive implementation — CascadeQuerySpec → Salsa `#[salsa::tracked]` function
//! + database trait method emit + dependency graph metadata emit (DESIGN §43
//! *output* dependency-graph auto-visualize carry).
//!
//! Input sources:
//! - Round 43 `entity_indexer::GraphSpec` (`EntityDef` / `RelationDef` typed schema)
//! - Round 45 `cf_wrapper::CfLayout` (storage layer interface, read path in cascade input)
//! - cascade dependency-graph metadata stub (Forge AST export consumer-path mock)
//!
//! output (DESIGN §43 *output* carry):
//! - Rust → Salsa `#[salsa::tracked]` function + Database trait method (production target)
//! - Kotlin / Python → fallback: read-only AST export consumer path (recomputed per call,
//! Round 36 substantive-decision carry — out-of-paradigm scope; *partial break of the 5-language emit contract*
//! cascade_query scope partial break* audit trail)
//! - auto-visualize dependency-graph metadata (§2 cascade preview)
//!
//! Prototype role (Round 42 ratify carry, Round 36 cascade_query out-of-paradigm decision carry):
//! - resides in the bench/codegen-prototype crate (§18 *prototype scope* boundary)
//! - Rust-only emit (Round 36 substantive decision — Salsa is Rust-owned and supports incremental
//! computation; 5-backend byte-identical emit is approximated as out-of-paradigm scope)
//! - Phase 1.5 cascade-gate measurement source as a separate layer (Phase 0 entry,
//! one-time measurement; this round is emit-format validation only.
//! - dev-dep salsa add — candidate for a subsequent round (this round only validates emit format / structure)

use crate::cf_wrapper::CfLayout;
use crate::entity_indexer::GraphSpec;

// ============================================================================
// Cascade query spec — Forge AST stub in cascade_query kind's mnemosyne itself
// codegen input (Round 33.5 responsibility minute- — cascade dependency graph metadata - SCE
// Forge AST export -, Salsa wire codegen itself- mnemosyne -).
// ============================================================================

/// Cascade query spec — DESIGN §43 *input*'s `<query>` block direct carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CascadeQuerySpec {
 /// Query name — Salsa #[tracked] function name.
 pub name: String,
 /// Read dependencies — entity / relation CF in read path.
 /// each element = `(entity_or_relation_name, field_name)`.
 pub reads: Vec<ReadDep>,
 /// Output type name — Salsa tracked function's return type.
 pub output: String,
 /// Invalidation triggers — DESIGN §43 *input*'s `<invalidates-on>` block carry.
 pub triggers: Vec<TriggerSpec>,
 /// CascadeOrdering axis (§39 inter-kind dependency carry; §47 body decision — `global_fifo` default).
 pub ordering: String,
}

/// Read dependency — any CF / field in read - cascade dependency's input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadDep {
 /// Entity / relation name (registered in the GraphSpec).
 pub entity: String,
 /// Field name or `*` (entity full read).
 pub field: String,
}

/// Invalidation trigger — DESIGN §43 *input*'s `<trigger>` element carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriggerSpec {
 /// Event kind (`entity_change` / `relation_change` / `epistemic_change`).
 pub event: String,
 /// Filter expression (referenced_entities / required_knowledge etc.).
 pub filter: String,
}

/// Cascade dependency-graph metadata — read-only Forge AST export consumer path.
/// visualize (Studio Kotlin / CLI Python dependency-graph visualization — *partial break of the 5-language emit contract*
/// partial-break audit trail carry).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CascadeDependencyGraph {
 /// each query's read dependency edge — `(query_name, dep_entity)`.
 pub edges: Vec<(String, String)>,
}

// ============================================================================
// CascadeWireSpec — multiple cascade queries + GraphSpec + CfLayout combined.
// ============================================================================

/// Cascade wire spec — emit input (full).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CascadeWireSpec {
 pub queries: Vec<CascadeQuerySpec>,
}

// ============================================================================
// Salsa wire emit — CascadeWireSpec → Rust source.
// this prototype - Salsa runtime semantics combined (#[salsa::tracked] + Database trait).
// ============================================================================

/// Codegen emit result — Salsa wire Rust source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmittedSalsa {
 pub source: String,
 /// Generated `#[salsa::tracked]` function count.
 pub tracked_fn_count: usize,
 /// Generated Database trait method count.
 pub db_method_count: usize,
 /// Cascade dependency graph edge count (Studio / CLI visualize metadata).
 pub dep_edge_count: usize,
}

/// CascadeWireSpec + GraphSpec + CfLayout → Salsa wire Rust source emit.
/// Deterministic — preserves the registered query order.
pub fn emit_salsa_wire(
 spec: &CascadeWireSpec,
 graph: &GraphSpec,
 layout: &CfLayout,
) -> EmittedSalsa {
 let mut out = String::new();
 out.push_str("// Auto-generated by codegen-prototype (Round 47, §43 salsa_wire).\n");
 out.push_str("// input source: CascadeWireSpec + GraphSpec + CfLayout. Salsa Rust-only emit.\n");
 out.push_str("// Round 36 cascade_query out-of-paradigm decision carry — partial break of the 5-language emit contract\n");
 out.push_str("// cascade_query scope partial break audit trail (Studio Kotlin / CLI Python of\n");
 out.push_str("// dependency graph visualize- AST export consumer in read-only path).\n");
 out.push_str("\n");

 // Cascade root input — tracked function's Salsa struct 2nd arg -er..
 // (Round 49 OPTION A in Salsa 0.26 actual compile validation result — primitive u64 -
 // SalsaStructInDb not.mplementation, root input - as wrap then branch_id getter in read.)
 out.push_str("// ─── Cascade root input (branch identity Salsa wrap) ───────────────\n");
 out.push_str("#[salsa::input]\n");
 out.push_str("pub struct CascadeBranch {\n");
 out.push_str(" pub branch_id: u64,\n");
 out.push_str("}\n\n");

 // Salsa input struct emit per entity (input --- source).
 out.push_str("// ─── Salsa input structs (entity per input source) ──────────────────\n");
 for entity in &graph.entities {
 out.push_str("#[salsa::input]\n");
 out.push_str(&format!("pub struct {}Input {{\n", entity.name));
 out.push_str(" pub branch_id: u64,\n");
 out.push_str(" pub entity_id: u64,\n");
 out.push_str(" pub valid_from: u64,\n");
 out.push_str(" pub payload: Vec<u8>,\n");
 out.push_str("}\n\n");
 }

 // Database trait — db_method_count --ic.
 // (Round 49: user trait extending salsa::Database in `#[salsa::db]` - zalsa
 // downcaster etc. plumbing - auto give- — Salsa 0.26 actual compile validation.)
 let mut db_method_count = 0usize;
 out.push_str("// ─── Database trait (mnemosyne cascade DB surface) ─────────────────────\n");
 out.push_str("#[salsa::db]\n");
 out.push_str("pub trait CascadeDb: salsa::Database {\n");
 for query in &spec.queries {
 out.push_str(&format!(
 " fn {}(&self, branch: CascadeBranch) -> {};\n",
 query.name, query.output
 ));
 db_method_count += 1;
 }
 out.push_str("}\n\n");

 // Tracked function per query.
 let mut tracked_fn_count = 0usize;
 let mut dep_edges: Vec<(String, String)> = Vec::new();
 out.push_str("// ─── Salsa #[tracked] cascade query functions ──────────────────────\n");
 for query in &spec.queries {
 emit_tracked_function(&mut out, query, layout);
 tracked_fn_count += 1;
 for read in &query.reads {
 dep_edges.push((query.name.clone(), read.entity.clone()));
 }
 }

 // Dependency graph metadata (DESIGN §43 *output* — dependency graph auto visualize).
 out.push_str("// ─── Cascade dependency graph metadata (Studio/CLI visualize source) ──\n");
 out.push_str("// Forge AST export consumer path read-only visualization (Round 36 out-of-paradigm)\n");
 out.push_str("// scope — decision carry — 5-language emit contract's cascade_query scope partial break).\n");
 out.push_str("pub fn cascade_dependency_edges() -> &'static [(&'static str, &'static str)] {\n");
 out.push_str(" &[\n");
 for (q, dep) in &dep_edges {
 out.push_str(&format!(" (\"{}\", \"{}\"),\n", q, dep));
 }
 out.push_str(" ]\n");
 out.push_str("}\n\n");

 // CascadeOrdering axis emit (§39 inter-kind dependency carry).
 out.push_str("// ─── CascadeOrdering axis consumption (§39 inter-kind dependency) ──\n");
 out.push_str("pub fn cascade_orderings() -> &'static [(&'static str, &'static str)] {\n");
 out.push_str(" &[\n");
 for query in &spec.queries {
 out.push_str(&format!(
 " (\"{}\", \"{}\"),\n",
 query.name, query.ordering
 ));
 }
 out.push_str(" ]\n");
 out.push_str("}\n");

 EmittedSalsa {
 source: out,
 tracked_fn_count,
 db_method_count,
 dep_edge_count: dep_edges.len(),
 }
}

fn emit_tracked_function(out: &mut String, query: &CascadeQuerySpec, _layout: &CfLayout) {
 // Trigger filters as a comment block — invalidation consistency surface.
 out.push_str(&format!("/// Cascade query `{}` — output {}.\n", query.name, query.output));
 out.push_str(&format!("/// Ordering: {} (§39 CascadeOrdering axis).\n", query.ordering));
 if !query.triggers.is_empty() {
 out.push_str("/// Invalidation triggers:\n");
 for trigger in &query.triggers {
 out.push_str(&format!(
  "/// - event: {}, filter: {}\n",
  trigger.event, trigger.filter
 ));
 }
 }
 if !query.reads.is_empty() {
 out.push_str("/// Read dependencies:\n");
 for read in &query.reads {
 out.push_str(&format!("/// - {}.{}\n", read.entity, read.field));
 }
 }
 out.push_str("#[salsa::tracked]\n");
 out.push_str(&format!(
 "pub fn {}<'db>(db: &'db dyn CascadeDb, branch: CascadeBranch) -> {} {{\n",
 query.name, query.output
 ));
 out.push_str(" // Read inputs from cascade DB — generated read path.\n");
 out.push_str(" let _branch_id = branch.branch_id(db);\n");
 for read in &query.reads {
 out.push_str(&format!(
 " let _ = (db, \"{}.{}\"); // read dep: {}.{}\n",
 read.entity, read.field, read.entity, read.field
 ));
 }
 out.push_str(&format!(
 " {}::default() // codegen stub — actual cascade body- subsequent round\n",
 query.output
 ));
 out.push_str("}\n\n");
}

// ============================================================================
// 5-language cascade dependency graph metadata emit (Round 54, OPTION B-3).
//
// Round 36 paradigm carry — Salsa runtime itself- Rust-only (cascade_query scope
// paradigm other decision). *cascade dependency-graph metadata* are read-only
// visualization source and 5-language emit possible — Studio Kotlin / CLI Python /
// C++ runtime SDK's cascade preview affected_asset_count output source.
//
// This emit scope (cascade_dependency_edges + cascade_orderings) two formal data items
// only 5-language sync.emit. Salsa #[salsa::input] / #[salsa::tracked] /
// CascadeDb trait etc. runtime semantics - Rust-only carry.
// ============================================================================

/// Cascade metadata 5-language emit result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmittedCascadeMetadata {
 pub rust: String,
 pub kotlin: String,
 pub python: String,
 pub cpp: String,
 pub protobuf: String,
 pub edge_count: usize,
 pub query_count: usize,
}

/// CascadeWireSpec → 5-language cascade metadata emit (Rust + Kotlin + Python +
/// C++ + Protobuf). Salsa runtime semantics are Rust-only carry; this function
/// dependency-graph + ordering metadata — emit-only.
pub fn emit_cascade_metadata_all_languages(spec: &CascadeWireSpec) -> EmittedCascadeMetadata {
 // Edge / ordering data extract (deterministic — query registered order preserved).
 let mut edges: Vec<(String, String)> = Vec::new();
 let mut orderings: Vec<(String, String)> = Vec::new();
 for query in &spec.queries {
 for read in &query.reads {
 edges.push((query.name.clone(), read.entity.clone()));
 }
 orderings.push((query.name.clone(), query.ordering.clone()));
 }
 EmittedCascadeMetadata {
 rust: emit_cascade_metadata_rust(&edges, &orderings),
 kotlin: emit_cascade_metadata_kotlin(&edges, &orderings),
 python: emit_cascade_metadata_python(&edges, &orderings),
 cpp: emit_cascade_metadata_cpp(&edges, &orderings),
 protobuf: emit_cascade_metadata_protobuf(&edges, &orderings),
 edge_count: edges.len(),
 query_count: spec.queries.len(),
 }
}

fn emit_cascade_metadata_rust(
 edges: &[(String, String)],
 orderings: &[(String, String)],
) -> String {
 let mut out = String::new();
 out.push_str("// Auto-generated by codegen-prototype (Round 54, §43 cascade metadata Rust emit).\n");
 out.push_str("// 5-language emit: dependency graph + ordering metadata only, Salsa runtime - Rust-only carry.\n");
 out.push_str("\n");
 out.push_str("pub fn cascade_dependency_edges() -> &'static [(&'static str, &'static str)] {\n");
 out.push_str(" &[\n");
 for (q, dep) in edges {
 out.push_str(&format!(" (\"{}\", \"{}\"),\n", q, dep));
 }
 out.push_str(" ]\n");
 out.push_str("}\n\n");
 out.push_str("pub fn cascade_orderings() -> &'static [(&'static str, &'static str)] {\n");
 out.push_str(" &[\n");
 for (q, ord) in orderings {
 out.push_str(&format!(" (\"{}\", \"{}\"),\n", q, ord));
 }
 out.push_str(" ]\n");
 out.push_str("}\n");
 out
}

fn emit_cascade_metadata_kotlin(
 edges: &[(String, String)],
 orderings: &[(String, String)],
) -> String {
 let mut out = String::new();
 out.push_str("// Auto-generated by codegen-prototype (Round 54, §43 cascade metadata Kotlin emit).\n");
 out.push_str("// Studio cascade preview affected_asset_count output source — read-only metadata.\n");
 out.push_str("\n");
 out.push_str("package mnemosyne.generated.cascade\n");
 out.push_str("\n");
 out.push_str("val cascadeDependencyEdges: List<Pair<String, String>> = listOf(\n");
 for (q, dep) in edges {
 out.push_str(&format!(" \"{}\" to \"{}\",\n", q, dep));
 }
 out.push_str(")\n\n");
 out.push_str("val cascadeOrderings: List<Pair<String, String>> = listOf(\n");
 for (q, ord) in orderings {
 out.push_str(&format!(" \"{}\" to \"{}\",\n", q, ord));
 }
 out.push_str(")\n");
 out
}

fn emit_cascade_metadata_python(
 edges: &[(String, String)],
 orderings: &[(String, String)],
) -> String {
 let mut out = String::new();
 out.push_str("# Auto-generated by codegen-prototype (Round 54, §43 cascade metadata Python emit).\n");
 out.push_str("# CLI cascade preview affected_asset_count output source — read-only metadata.\n");
 out.push_str("\n");
 out.push_str("from typing import List, Tuple\n");
 out.push_str("\n");
 out.push_str("cascade_dependency_edges: List[Tuple[str, str]] = [\n");
 for (q, dep) in edges {
 out.push_str(&format!(" (\"{}\", \"{}\"),\n", q, dep));
 }
 out.push_str("]\n\n");
 out.push_str("cascade_orderings: List[Tuple[str, str]] = [\n");
 for (q, ord) in orderings {
 out.push_str(&format!(" (\"{}\", \"{}\"),\n", q, ord));
 }
 out.push_str("]\n");
 out
}

fn emit_cascade_metadata_cpp(
 edges: &[(String, String)],
 orderings: &[(String, String)],
) -> String {
 let mut out = String::new();
 out.push_str("// Auto-generated by codegen-prototype (Round 54, §43 cascade metadata C++ emit).\n");
 out.push_str("// Runtime SDK cascade preview affected_asset_count output source — read-only metadata.\n");
 out.push_str("\n");
 out.push_str("#pragma once\n");
 out.push_str("#include <array>\n");
 out.push_str("#include <string_view>\n");
 out.push_str("#include <utility>\n");
 out.push_str("\n");
 out.push_str("namespace mnemosyne::cascade {\n");
 out.push_str("\n");
 out.push_str(&format!(
 "constexpr std::array<std::pair<std::string_view, std::string_view>, {}> CASCADE_DEPENDENCY_EDGES = {{{{\n",
 edges.len()
 ));
 for (q, dep) in edges {
 out.push_str(&format!(
 " {{\"{}\", \"{}\"}},\n",
 q, dep
 ));
 }
 out.push_str("}};\n\n");
 out.push_str(&format!(
 "constexpr std::array<std::pair<std::string_view, std::string_view>, {}> CASCADE_ORDERINGS = {{{{\n",
 orderings.len()
 ));
 for (q, ord) in orderings {
 out.push_str(&format!(
 " {{\"{}\", \"{}\"}},\n",
 q, ord
 ));
 }
 out.push_str("}};\n\n");
 out.push_str("} // namespace mnemosyne::cascade\n");
 out
}

fn emit_cascade_metadata_protobuf(
 edges: &[(String, String)],
 orderings: &[(String, String)],
) -> String {
 let mut out = String::new();
 out.push_str("// Auto-generated by codegen-prototype (Round 54, §43 cascade metadata protobuf emit).\n");
 out.push_str("// Wire format spec — runtime AST consumer cascade dependency graph expression.\n");
 out.push_str("\n");
 out.push_str("syntax = \"proto3\";\n");
 out.push_str("\n");
 out.push_str("package mnemosyne.cascade;\n");
 out.push_str("\n");
 out.push_str("message CascadeDependencyEdge {\n");
 out.push_str(" string query_name = 1;\n");
 out.push_str(" string dep_entity = 2;\n");
 out.push_str("}\n\n");
 out.push_str("message CascadeOrdering {\n");
 out.push_str(" string query_name = 1;\n");
 out.push_str(" string ordering = 2;\n");
 out.push_str("}\n\n");
 out.push_str("message CascadeMetadata {\n");
 out.push_str(" repeated CascadeDependencyEdge edges = 1;\n");
 out.push_str(" repeated CascadeOrdering orderings = 2;\n");
 out.push_str("}\n\n");
 // Snapshot of fixture data as a comment block for traceability.
 out.push_str("// ─── Fixture snapshot (auto-generated, comment only — wire data - separate) ──\n");
 for (q, dep) in edges {
 out.push_str(&format!("// edge: {{ query_name: \"{}\", dep_entity: \"{}\" }}\n", q, dep));
 }
 for (q, ord) in orderings {
 out.push_str(&format!("// ordering: {{ query_name: \"{}\", ordering: \"{}\" }}\n", q, ord));
 }
 out
}

/// Cascade metadata canonical identifier set — query name + dep entity + ordering axis.
/// Validates Jaccard inclusion = 1.0 across 5-language emit for the entire set.
pub fn cascade_metadata_canonical_set(spec: &CascadeWireSpec) -> std::collections::BTreeSet<String> {
 let mut set = std::collections::BTreeSet::new();
 for query in &spec.queries {
 set.insert(query.name.clone());
 set.insert(query.ordering.clone());
 for read in &query.reads {
 set.insert(read.entity.clone());
 }
 }
 set
}

// ============================================================================
// §66 design_doc fixture — small cascade query (Round 47 measure source).
// e.g. ChangelogEntry add → Section.decision_status update cascade.
// ============================================================================

/// §66 design_doc small cascade fixture — Phase 0 prerequisite #4's prototype validation source.
pub fn design_doc_cascade_fixture() -> CascadeWireSpec {
 CascadeWireSpec {
 queries: vec![
 // Query 1: section_decision_status — ChangelogEntry append → Section.decision_status update.
 CascadeQuerySpec {
  name: "section_decision_status".to_string(),
  reads: vec![
  ReadDep {
  entity: "Section".to_string(),
  field: "decision_status".to_string(),
  },
  ReadDep {
  entity: "ChangelogEntry".to_string(),
  field: "summary".to_string(),
  },
  ],
  output: "ValidationResult".to_string(),
  triggers: vec![
  TriggerSpec {
  event: "entity_change".to_string(),
  filter: "ChangelogEntry".to_string(),
  },
  TriggerSpec {
  event: "entity_change".to_string(),
  filter: "Section".to_string(),
  },
  ],
  ordering: "global_fifo".to_string(),
 },
 // Query 2: frozen_list_membership — FrozenList membership check (CrossRef cascade).
 CascadeQuerySpec {
  name: "frozen_list_membership".to_string(),
  reads: vec![
  ReadDep {
  entity: "FrozenList".to_string(),
  field: "owner_section".to_string(),
  },
  ReadDep {
  entity: "CrossRef".to_string(),
  field: "ref_kind".to_string(),
  },
  ],
  output: "ValidationResult".to_string(),
  triggers: vec![TriggerSpec {
  event: "relation_change".to_string(),
  filter: "CrossRef".to_string(),
  }],
  ordering: "global_fifo".to_string(),
 },
 ],
 }
}

// ============================================================================
// Tests — small fixture validation (Round 47 measure data source).
// ============================================================================

#[cfg(test)]
mod tests {
 use super::*;
 use crate::cf_wrapper::default_layout;
 use crate::entity_indexer::design_doc_schema_fixture;

 #[test]
 fn cascade_fixture_has_expected_queries() {
 let spec = design_doc_cascade_fixture();
 assert_eq!(spec.queries.len(), 2);
 assert_eq!(spec.queries[0].name, "section_decision_status");
 assert_eq!(spec.queries[1].name, "frozen_list_membership");
 }

 /// emit_salsa_wire deterministic — identical input → byte-identical output.
 #[test]
 fn emit_salsa_wire_deterministic() {
 let cascade = design_doc_cascade_fixture();
 let graph = design_doc_schema_fixture();
 let layout = default_layout(&graph);
 let a = emit_salsa_wire(&cascade, &graph, &layout);
 let b = emit_salsa_wire(&cascade, &graph, &layout);
 assert_eq!(a, b);
 }

 /// Salsa input struct emit — entity per #[salsa::input] struct.
 #[test]
 fn salsa_input_struct_per_entity() {
 let cascade = design_doc_cascade_fixture();
 let graph = design_doc_schema_fixture();
 let layout = default_layout(&graph);
 let emitted = emit_salsa_wire(&cascade, &graph, &layout);
 assert!(emitted.source.contains("#[salsa::input]"));
 assert!(emitted.source.contains("pub struct SectionInput"));
 assert!(emitted.source.contains("pub struct ChangelogEntryInput"));
 assert!(emitted.source.contains("pub struct FrozenListInput"));
 }

 /// Database trait method emit — query per fn signature.
 #[test]
 fn database_trait_emits_method_per_query() {
 let cascade = design_doc_cascade_fixture();
 let graph = design_doc_schema_fixture();
 let layout = default_layout(&graph);
 let emitted = emit_salsa_wire(&cascade, &graph, &layout);
 assert!(emitted.source.contains("#[salsa::db]\npub trait CascadeDb: salsa::Database"));
 assert!(emitted.source.contains("fn section_decision_status(&self, branch: CascadeBranch)"));
 assert!(emitted.source.contains("fn frozen_list_membership(&self, branch: CascadeBranch)"));
 assert_eq!(emitted.db_method_count, cascade.queries.len());
 }

 /// #[salsa::tracked] function emit — query per tracked fn body.
 #[test]
 fn tracked_function_per_query() {
 let cascade = design_doc_cascade_fixture();
 let graph = design_doc_schema_fixture();
 let layout = default_layout(&graph);
 let emitted = emit_salsa_wire(&cascade, &graph, &layout);
 assert!(emitted.source.contains("#[salsa::tracked]"));
 assert!(emitted.source.contains(
 "pub fn section_decision_status<'db>(db: &'db dyn CascadeDb, branch: CascadeBranch)"
 ));
 assert!(emitted.source.contains(
 "pub fn frozen_list_membership<'db>(db: &'db dyn CascadeDb, branch: CascadeBranch)"
 ));
 assert_eq!(emitted.tracked_fn_count, cascade.queries.len());
 }

 /// CascadeBranch root input emit — wrapped to compile against Salsa 0.26.
 #[test]
 fn cascade_branch_root_input_emits() {
 let cascade = design_doc_cascade_fixture();
 let graph = design_doc_schema_fixture();
 let layout = default_layout(&graph);
 let emitted = emit_salsa_wire(&cascade, &graph, &layout);
 assert!(emitted.source.contains("pub struct CascadeBranch {"));
 assert!(emitted
 .source
 .contains("// ─── Cascade root input (branch identity Salsa wrap)"));
 }

 /// Cascade dependency graph metadata — Studio/CLI visualize source.
 #[test]
 fn dependency_graph_metadata_emits() {
 let cascade = design_doc_cascade_fixture();
 let graph = design_doc_schema_fixture();
 let layout = default_layout(&graph);
 let emitted = emit_salsa_wire(&cascade, &graph, &layout);
 assert!(emitted.source.contains("pub fn cascade_dependency_edges()"));
 assert!(emitted
 .source
 .contains("(\"section_decision_status\", \"Section\")"));
 assert!(emitted
 .source
 .contains("(\"section_decision_status\", \"ChangelogEntry\")"));
 // 2 query × 2 reads (q1) + 2 query × 1 read (q2 has 2 reads)
 // q1: 2 reads → 2 edges, q2: 2 reads → 2 edges, total 4 edges.
 assert_eq!(emitted.dep_edge_count, 4);
 }

 /// CascadeOrdering axis emit — §39 inter-kind dependency carry.
 #[test]
 fn cascade_ordering_emits_per_query() {
 let cascade = design_doc_cascade_fixture();
 let graph = design_doc_schema_fixture();
 let layout = default_layout(&graph);
 let emitted = emit_salsa_wire(&cascade, &graph, &layout);
 assert!(emitted.source.contains("pub fn cascade_orderings()"));
 assert!(emitted
 .source
 .contains("(\"section_decision_status\", \"global_fifo\")"));
 assert!(emitted
 .source
 .contains("(\"frozen_list_membership\", \"global_fifo\")"));
 }

 /// Trigger filter comment block — invalidation consistency surface.
 #[test]
 fn trigger_filters_emit_as_comments() {
 let cascade = design_doc_cascade_fixture();
 let graph = design_doc_schema_fixture();
 let layout = default_layout(&graph);
 let emitted = emit_salsa_wire(&cascade, &graph, &layout);
 assert!(emitted
 .source
 .contains("event: entity_change, filter: ChangelogEntry"));
 assert!(emitted
 .source
 .contains("event: relation_change, filter: CrossRef"));
 }

 /// emitted source byte size — small cascade fixture stable anchor.
 #[test]
 fn fixture_emit_size_stable() {
 let cascade = design_doc_cascade_fixture();
 let graph = design_doc_schema_fixture();
 let layout = default_layout(&graph);
 let emitted = emit_salsa_wire(&cascade, &graph, &layout);
 let size = emitted.source.len();
 assert!(
 size > 1500 && size < 6000,
 "small cascade fixture emit size: {} bytes (expected 1500-6000 range)",
 size
 );
 }

 // ─── §43 cascade metadata 5-language emit tests (Round 54, OPTION B-3) ─

 /// 5-language cascade metadata emit deterministic + all 64-char sha256.
 #[test]
 fn emit_cascade_metadata_5_lang_deterministic() {
 use crate::entity_indexer::sha256_hex;
 let cascade = design_doc_cascade_fixture();
 let a = emit_cascade_metadata_all_languages(&cascade);
 let b = emit_cascade_metadata_all_languages(&cascade);
 assert_eq!(a, b);
 // 5 distinct sha256 hex digest
 let mut hashes = std::collections::BTreeSet::new();
 hashes.insert(sha256_hex(&a.rust));
 hashes.insert(sha256_hex(&a.kotlin));
 hashes.insert(sha256_hex(&a.python));
 hashes.insert(sha256_hex(&a.cpp));
 hashes.insert(sha256_hex(&a.protobuf));
 assert_eq!(hashes.len(), 5);
 for h in &hashes {
 assert_eq!(h.len(), 64);
 }
 }

 /// Validates all 5 emits — covers cascade query names + dep entity names + ordering keys.
 #[test]
 fn emit_cascade_metadata_5_lang_covers_queries() {
 let cascade = design_doc_cascade_fixture();
 let m = emit_cascade_metadata_all_languages(&cascade);
 for emit in &[&m.rust, &m.kotlin, &m.python, &m.cpp, &m.protobuf] {
 assert!(
  emit.contains("section_decision_status"),
  "section_decision_status missing in emit"
 );
 assert!(
  emit.contains("frozen_list_membership"),
  "frozen_list_membership missing in emit"
 );
 assert!(emit.contains("Section"));
 assert!(emit.contains("CrossRef"));
 }
 assert_eq!(m.edge_count, 4);
 assert_eq!(m.query_count, 2);
 }

 /// Cross-language Jaccard inclusion = 1.0 — all 5 emits cover the full canonical set.
 #[test]
 fn cascade_metadata_5_lang_jaccard_one() {
 use crate::entity_indexer::jaccard_inclusion;
 let cascade = design_doc_cascade_fixture();
 let canonical = cascade_metadata_canonical_set(&cascade);
 let m = emit_cascade_metadata_all_languages(&cascade);
 for (lang, emit) in &[
 ("rust", &m.rust),
 ("kotlin", &m.kotlin),
 ("python", &m.python),
 ("cpp", &m.cpp),
 ("protobuf", &m.protobuf),
 ] {
 let score = jaccard_inclusion(emit, &canonical);
 assert!(
  (score - 1.0).abs() < f64::EPSILON,
  "{lang} cascade metadata Jaccard inclusion = {score} (expected 1.0)"
 );
 }
 }

 /// canonical set validation — 2 query name + 4 dep entity + 1 ordering axis.
 #[test]
 fn cascade_metadata_canonical_set_covers_fixture() {
 let cascade = design_doc_cascade_fixture();
 let s = cascade_metadata_canonical_set(&cascade);
 assert!(s.contains("section_decision_status"));
 assert!(s.contains("frozen_list_membership"));
 assert!(s.contains("Section"));
 assert!(s.contains("ChangelogEntry"));
 assert!(s.contains("FrozenList"));
 assert!(s.contains("CrossRef"));
 assert!(s.contains("global_fifo"));
 }

 /// Salsa runtime itself stays Rust-only — Round 36 out-of-paradigm decision.
 /// This test validates that the cascade-metadata emit does not include #[salsa::input/tracked/db].
 /// (metadata-only emit boundary, clearly delimited).
 #[test]
 fn cascade_metadata_excludes_salsa_runtime() {
 let cascade = design_doc_cascade_fixture();
 let m = emit_cascade_metadata_all_languages(&cascade);
 for emit in &[&m.kotlin, &m.python, &m.cpp, &m.protobuf] {
 // 5-language metadata emit in Salsa runtime attribute not.nclude
 assert!(
  !emit.contains("#[salsa::input]"),
  "5-language metadata emit - Salsa runtime include (paradigm boundary violation)"
 );
 assert!(
  !emit.contains("#[salsa::tracked]"),
  "5-language metadata emit - #[salsa::tracked] include (paradigm boundary violation)"
 );
 assert!(!emit.contains("CascadeDb"));
 }
 // Rust metadata emit also runtime API not.nclude (metadata-only).
 assert!(!m.rust.contains("#[salsa::input]"));
 assert!(!m.rust.contains("#[salsa::tracked]"));
 }
}
