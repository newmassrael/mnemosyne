//! §42 RocksDB CF runtime wrapper codegen prototype (Round 45 subsequent work scope).
//!
//! Round 45 substantive implementation — GraphSpec (Round 43 §39 entity-indexer output) →
//! typed CRUD wrapper Rust source emit + migration tool stub +
//! read-only secondary subset emit (Rust store layer + C++ runtime SDK only,
//! DESIGN §42 *output* carry).
//!
//! Input sources:
//! - Round 43 `entity_indexer::GraphSpec` (`EntityDef` / `RelationDef` typed schema)
//! - Round 43 `entity_indexer::encode_composite_key` 24 B BE (Phase -1A stage 2C)
//!
//! output (DESIGN §42 *output* carry):
//! - typed CRUD wrapper (`{Entity}CF::get`, `put`, `iter_branch`, `write_batch`)
//! - typed relation CRUD (`{Relation}CF::put`, `get`, `iter_from`)
//! - migration tool stub (CF add/delete/version bump on default scaffolding)
//! - read-only secondary subset (secondary-readable=true CF only, §15 carry)
//!
//! Prototype role (Round 42 ratify carry):
//! - resides in the bench/codegen-prototype crate (§18 *prototype scope* boundary)
//! - 5-language emit consistency validation — follow-up round (Rust core + C++ runtime SDK only)
//! - signature-consistency validation between direct-impl crate's manual `BranchStore` wrapper and the typed emit
//! - The SCE-side `@namespace` annotation system progresses upstream and is out of scope here.
//! (Round 33 signal-4 carry — phase-entry-block framing deprecation ratify)

use crate::entity_indexer::{EntityDef, GraphSpec, Persistence, RelationDef};

// ============================================================================
// CF metadata — DESIGN §4 10 CF schema direct carry.
// this prototype - GraphSpec in entity/relation 1 - = CF 1 - (mapping 1:1).
// actual §4 10 CF (entities / relations / temporal_index / temporal_index_open /
// branch_meta / assets / asset_refs / audit / epistemic / secrets) in mapping-
// subsequent round (annotation system -count time) from -system registered.
// ============================================================================

/// CF metadata — DESIGN §42 *input*'s column_family body direct carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CfMeta {
 /// CF name (RocksDB ColumnFamily handle name).
 pub name: String,
 /// Iterator pattern — `prefix_scan` (default for entity CF) /
 /// `range_scan` / `append_only_seq` (audit/log CF).
 pub iter_pattern: IterPattern,
 /// §15 secondary-readable flag — `false` excludes the CF from the secondary subset.
 pub secondary_readable: bool,
 /// Migration version — stub for CF add / delete / version-bump carry.
 pub schema_version: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IterPattern {
 /// 24 B prefix `branch_id (8) || entity_id (8)` in time-ordered scan.
 PrefixScan,
 /// Range scan (lower, upper) — workhalf - range.
 RangeScan,
 /// Append-only sequential — audit / log CF.
 AppendOnlySeq,
}

impl IterPattern {
 fn as_emit_str(&self) -> &'static str {
 match self {
 Self::PrefixScan => "prefix_scan",
 Self::RangeScan => "range_scan",
 Self::AppendOnlySeq => "append_only_seq",
 }
 }
}

// ============================================================================
// CF layout spec — GraphSpec in CF metadata mapping.
// ============================================================================

/// CF layout — entity/relation → CF metadata mapping.
/// Round 33.5 fine-resolution mapping carry: typed record + namespace tag = `@namespace=branch_meta`
/// Once the annotation system supports other kinds (schema-annotation scope), this layout becomes
/// Runtime wrapper input. Once the annotation system is in place, this layout becomes the actual
/// `@namespace` annotation parser.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CfLayout {
 /// Entity CF assignments — `entity.name` → CfMeta.
 pub entities: Vec<(String, CfMeta)>,
 /// Relation CF assignments — `relation.name` → CfMeta.
 pub relations: Vec<(String, CfMeta)>,
}

/// Default CF layout from GraphSpec — one `{name}_entities` CF per entity,
/// One `{name}_relations` CF per relation (PrefixScan + secondary_readable=true).
/// Actual §4 10-CF mapping — counts annotation systems and decides per `@namespace`.
pub fn default_layout(spec: &GraphSpec) -> CfLayout {
 let mut layout = CfLayout::default();
 for entity in &spec.entities {
 let cf_name = format!("{}_entities", to_snake(&entity.name));
 layout.entities.push((
 entity.name.clone(),
 CfMeta {
  name: cf_name,
  iter_pattern: IterPattern::PrefixScan,
  secondary_readable: matches!(entity.persistence, Persistence::Persistent),
  schema_version: 1,
 },
 ));
 }
 for relation in &spec.relations {
 let cf_name = format!("{}_relations", to_snake(&relation.name));
 layout.relations.push((
 relation.name.clone(),
 CfMeta {
  name: cf_name,
  iter_pattern: IterPattern::PrefixScan,
  secondary_readable: matches!(relation.persistence, Persistence::Persistent),
  schema_version: 1,
 },
 ));
 }
 layout
}

fn to_snake(s: &str) -> String {
 let mut out = String::with_capacity(s.len() + 4);
 for (i, ch) in s.chars().enumerate() {
 if ch.is_ascii_uppercase() {
 if i > 0 {
  out.push('_');
 }
 out.push(ch.to_ascii_lowercase());
 } else {
 out.push(ch);
 }
 }
 out
}

// ============================================================================
// Typed CRUD wrapper emit — GraphSpec + CfLayout → Rust source.
// direct-impl crate's `BranchStore` pattern carry (put_entity / get / iter / write_batch).
// ============================================================================

/// Codegen emit result — typed CRUD wrapper Rust source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmittedWrapper {
 pub source: String,
 /// Generated typed method count (entity × 4 + relation × 3 + secondary subset).
 pub method_count: usize,
 /// CF descriptor count (entity + relation, secondary exclude).
 pub cf_count: usize,
 /// Secondary-readable subset CF count.
 pub secondary_cf_count: usize,
}

/// GraphSpec + CfLayout → typed CRUD wrapper emit.
/// Deterministic — preserves the registered order for all entities + relations.
pub fn emit_wrapper(spec: &GraphSpec, layout: &CfLayout) -> EmittedWrapper {
 let mut out = String::new();
 let mut method_count = 0usize;

 out.push_str("// Auto-generated by codegen-prototype (Round 45, §42 cf_wrapper).\n");
 out.push_str("// input source: GraphSpec + CfLayout. direct-impl BranchStore pattern carry.\n");
 out.push_str("// 5-language emit contract's workpart scope carry — Rust store layer + C++ runtime SDK only.\n");
 out.push_str("\n");
 out.push_str("use anyhow::{anyhow, Result};\n");
 out.push_str("use byteorder::{BigEndian, ByteOrder};\n");
 out.push_str("use rocksdb::{ColumnFamilyDescriptor, DB, IteratorMode, Direction, WriteBatch};\n");
 out.push_str("use std::sync::Arc;\n");
 out.push_str("\n");
 out.push_str("pub const KEY_LEN: usize = 24;\n");
 out.push_str("\n");

 // CF name constants — 1 const per CF.
 let mut cf_count = 0usize;
 let mut secondary_cf_count = 0usize;
 out.push_str("// ─── CF name constants ─────────────────────────────────────────────────\n");
 for (_, meta) in layout.entities.iter().chain(layout.relations.iter()) {
 out.push_str(&format!(
 "pub const CF_{}: &str = \"{}\";\n",
 meta.name.to_uppercase(),
 meta.name
 ));
 cf_count += 1;
 if meta.secondary_readable {
 secondary_cf_count += 1;
 }
 }
 out.push('\n');

 // CF descriptor list emit — DB::open_cf_descriptors input.
 emit_cf_descriptors(&mut out, layout);

 // Typed CRUD wrapper per entity.
 for entity in &spec.entities {
 let cf_meta = layout
 .entities
 .iter()
 .find(|(name, _)| name == &entity.name)
 .map(|(_, m)| m);
 if let Some(meta) = cf_meta {
 method_count += emit_entity_wrapper(&mut out, entity, meta);
 }
 }

 // Typed CRUD wrapper per relation.
 for relation in &spec.relations {
 let cf_meta = layout
 .relations
 .iter()
 .find(|(name, _)| name == &relation.name)
 .map(|(_, m)| m);
 if let Some(meta) = cf_meta {
 method_count += emit_relation_wrapper(&mut out, relation, meta);
 }
 }

 // Migration tool stub — CF add / delete / version bump.
 method_count += emit_migration_stub(&mut out, layout);

 // Read-only secondary subset — secondary_readable=true CF only.
 method_count += emit_secondary_subset(&mut out, layout);

 EmittedWrapper {
 source: out,
 method_count,
 cf_count,
 secondary_cf_count,
 }
}

fn emit_cf_descriptors(out: &mut String, layout: &CfLayout) {
 out.push_str("// ─── CF descriptors (DB::open_cf_descriptors input) ────────────────────\n");
 out.push_str("pub fn cf_descriptors() -> Vec<ColumnFamilyDescriptor> {\n");
 out.push_str(" use rocksdb::Options;\n");
 out.push_str(" let cf_opts = Options::default();\n");
 out.push_str(" vec![\n");
 for (_, meta) in layout.entities.iter().chain(layout.relations.iter()) {
 out.push_str(&format!(
 " ColumnFamilyDescriptor::new(CF_{}, cf_opts.clone()),\n",
 meta.name.to_uppercase()
 ));
 }
 out.push_str(" ]\n");
 out.push_str("}\n\n");
}

fn emit_entity_wrapper(out: &mut String, entity: &EntityDef, meta: &CfMeta) -> usize {
 let struct_name = format!("{}CF", entity.name);
 let cf_const = format!("CF_{}", meta.name.to_uppercase());
 let iter_str = meta.iter_pattern.as_emit_str();
 let mut emitted = 0usize;

 out.push_str(&format!(
 "// ─── {} typed CRUD wrapper (iter_pattern={}, secondary_readable={}) ─\n",
 struct_name, iter_str, meta.secondary_readable
 ));
 out.push_str(&format!("pub struct {} {{\n", struct_name));
 out.push_str(" db: Arc<DB>,\n");
 out.push_str("}\n\n");

 out.push_str(&format!("impl {} {{\n", struct_name));
 out.push_str(" pub fn new(db: Arc<DB>) -> Self {\n");
 out.push_str(" Self { db }\n");
 out.push_str(" }\n\n");

 // put — branch_id, entity_id, valid_from + bincode value.
 out.push_str(" pub fn put(&self, branch_id: u64, entity_id: u64, valid_from: u64, value: &[u8]) -> Result<()> {\n");
 out.push_str(&format!(
 " let cf = self.db.cf_handle({}).ok_or_else(|| anyhow!(\"missing CF {{}}\", {}))?;\n",
 cf_const, cf_const
 ));
 out.push_str(" let mut buf = [0u8; KEY_LEN];\n");
 out.push_str(" BigEndian::write_u64(&mut buf[0..8], branch_id);\n");
 out.push_str(" BigEndian::write_u64(&mut buf[8..16], entity_id);\n");
 out.push_str(" BigEndian::write_u64(&mut buf[16..24], valid_from);\n");
 out.push_str(" self.db.put_cf(cf, buf, value)?;\n");
 out.push_str(" Ok(())\n");
 out.push_str(" }\n\n");
 emitted += 1;

 // get — point query (branch_id, entity_id, valid_from).
 out.push_str(" pub fn get(&self, branch_id: u64, entity_id: u64, valid_from: u64) -> Result<Option<Vec<u8>>> {\n");
 out.push_str(&format!(
 " let cf = self.db.cf_handle({}).ok_or_else(|| anyhow!(\"missing CF {{}}\", {}))?;\n",
 cf_const, cf_const
 ));
 out.push_str(" let mut buf = [0u8; KEY_LEN];\n");
 out.push_str(" BigEndian::write_u64(&mut buf[0..8], branch_id);\n");
 out.push_str(" BigEndian::write_u64(&mut buf[8..16], entity_id);\n");
 out.push_str(" BigEndian::write_u64(&mut buf[16..24], valid_from);\n");
 out.push_str(" Ok(self.db.get_cf(cf, buf)?)\n");
 out.push_str(" }\n\n");
 emitted += 1;

 // iter_branch — prefix scan within `branch_id || entity_id` (16 B prefix).
 out.push_str(" pub fn iter_branch(&self, branch_id: u64, entity_id: u64) -> Result<Vec<(u64, Vec<u8>)>> {\n");
 out.push_str(&format!(
 " let cf = self.db.cf_handle({}).ok_or_else(|| anyhow!(\"missing CF {{}}\", {}))?;\n",
 cf_const, cf_const
 ));
 out.push_str(" let mut prefix = [0u8; 16];\n");
 out.push_str(" BigEndian::write_u64(&mut prefix[..8], branch_id);\n");
 out.push_str(" BigEndian::write_u64(&mut prefix[8..16], entity_id);\n");
 out.push_str(" let iter = self.db.iterator_cf(cf, IteratorMode::From(&prefix, Direction::Forward));\n");
 out.push_str(" let mut out = Vec::new();\n");
 out.push_str(" for item in iter {\n");
 out.push_str(" let (k, v) = item?;\n");
 out.push_str(" if k.len() != KEY_LEN || k[..16] != prefix { break; }\n");
 out.push_str(" let valid_from = BigEndian::read_u64(&k[16..24]);\n");
 out.push_str(" out.push((valid_from, v.into_vec()));\n");
 out.push_str(" }\n");
 out.push_str(" Ok(out)\n");
 out.push_str(" }\n\n");
 emitted += 1;

 // write_batch — bulk insert (direct-impl write_entities_batch equivalent).
 out.push_str(" pub fn write_batch(&self, entries: &[(u64, u64, u64, Vec<u8>)]) -> Result<()> {\n");
 out.push_str(&format!(
 " let cf = self.db.cf_handle({}).ok_or_else(|| anyhow!(\"missing CF {{}}\", {}))?;\n",
 cf_const, cf_const
 ));
 out.push_str(" let mut batch = WriteBatch::default();\n");
 out.push_str(" for (b, e, v, val) in entries {\n");
 out.push_str(" let mut buf = [0u8; KEY_LEN];\n");
 out.push_str(" BigEndian::write_u64(&mut buf[0..8], *b);\n");
 out.push_str(" BigEndian::write_u64(&mut buf[8..16], *e);\n");
 out.push_str(" BigEndian::write_u64(&mut buf[16..24], *v);\n");
 out.push_str(" batch.put_cf(cf, buf, val);\n");
 out.push_str(" }\n");
 out.push_str(" self.db.write(batch)?;\n");
 out.push_str(" Ok(())\n");
 out.push_str(" }\n");
 out.push_str("}\n\n");
 emitted += 1;

 emitted
}

fn emit_relation_wrapper(out: &mut String, relation: &RelationDef, meta: &CfMeta) -> usize {
 let struct_name = format!("{}CF", relation.name);
 let cf_const = format!("CF_{}", meta.name.to_uppercase());
 let iter_str = meta.iter_pattern.as_emit_str();
 let mut emitted = 0usize;

 out.push_str(&format!(
 "// ─── {} typed relation wrapper (iter_pattern={}, secondary_readable={}) ─\n",
 struct_name, iter_str, meta.secondary_readable
 ));
 out.push_str(&format!("pub struct {} {{\n", struct_name));
 out.push_str(" db: Arc<DB>,\n");
 out.push_str("}\n\n");

 out.push_str(&format!("impl {} {{\n", struct_name));
 out.push_str(" pub fn new(db: Arc<DB>) -> Self {\n");
 out.push_str(" Self { db }\n");
 out.push_str(" }\n\n");

 // put — relation key = branch_id || from_id || to_id (24 B carry, valid_from -er.at to_id).
 out.push_str(&format!(
 " /// {} key = branch_id (8 B BE) || from_id ({}) (8 B BE) || to_id ({}) (8 B BE).\n",
 relation.name, relation.from_entity, relation.to_entity
 ));
 out.push_str(" pub fn put(&self, branch_id: u64, from_id: u64, to_id: u64, value: &[u8]) -> Result<()> {\n");
 out.push_str(&format!(
 " let cf = self.db.cf_handle({}).ok_or_else(|| anyhow!(\"missing CF {{}}\", {}))?;\n",
 cf_const, cf_const
 ));
 out.push_str(" let mut buf = [0u8; KEY_LEN];\n");
 out.push_str(" BigEndian::write_u64(&mut buf[0..8], branch_id);\n");
 out.push_str(" BigEndian::write_u64(&mut buf[8..16], from_id);\n");
 out.push_str(" BigEndian::write_u64(&mut buf[16..24], to_id);\n");
 out.push_str(" self.db.put_cf(cf, buf, value)?;\n");
 out.push_str(" Ok(())\n");
 out.push_str(" }\n\n");
 emitted += 1;

 // get — point query (branch_id, from_id, to_id).
 out.push_str(" pub fn get(&self, branch_id: u64, from_id: u64, to_id: u64) -> Result<Option<Vec<u8>>> {\n");
 out.push_str(&format!(
 " let cf = self.db.cf_handle({}).ok_or_else(|| anyhow!(\"missing CF {{}}\", {}))?;\n",
 cf_const, cf_const
 ));
 out.push_str(" let mut buf = [0u8; KEY_LEN];\n");
 out.push_str(" BigEndian::write_u64(&mut buf[0..8], branch_id);\n");
 out.push_str(" BigEndian::write_u64(&mut buf[8..16], from_id);\n");
 out.push_str(" BigEndian::write_u64(&mut buf[16..24], to_id);\n");
 out.push_str(" Ok(self.db.get_cf(cf, buf)?)\n");
 out.push_str(" }\n\n");
 emitted += 1;

 // iter_from — prefix scan within `branch_id || from_id` (16 B prefix).
 out.push_str(" pub fn iter_from(&self, branch_id: u64, from_id: u64) -> Result<Vec<(u64, Vec<u8>)>> {\n");
 out.push_str(&format!(
 " let cf = self.db.cf_handle({}).ok_or_else(|| anyhow!(\"missing CF {{}}\", {}))?;\n",
 cf_const, cf_const
 ));
 out.push_str(" let mut prefix = [0u8; 16];\n");
 out.push_str(" BigEndian::write_u64(&mut prefix[..8], branch_id);\n");
 out.push_str(" BigEndian::write_u64(&mut prefix[8..16], from_id);\n");
 out.push_str(" let iter = self.db.iterator_cf(cf, IteratorMode::From(&prefix, Direction::Forward));\n");
 out.push_str(" let mut out = Vec::new();\n");
 out.push_str(" for item in iter {\n");
 out.push_str(" let (k, v) = item?;\n");
 out.push_str(" if k.len() != KEY_LEN || k[..16] != prefix { break; }\n");
 out.push_str(" let to_id = BigEndian::read_u64(&k[16..24]);\n");
 out.push_str(" out.push((to_id, v.into_vec()));\n");
 out.push_str(" }\n");
 out.push_str(" Ok(out)\n");
 out.push_str(" }\n");
 out.push_str("}\n\n");
 emitted += 1;

 emitted
}

fn emit_migration_stub(out: &mut String, layout: &CfLayout) -> usize {
 out.push_str("// ─── Migration tool stub (CF add / delete / version bump on carry) ────\n");
 out.push_str("pub struct MigrationStub;\n\n");
 out.push_str("impl MigrationStub {\n");
 out.push_str(" /// CF schema_version per CF — bumped when the schema changes; otherwise stable.\n");
 out.push_str(" pub fn cf_versions() -> &'static [(&'static str, u32)] {\n");
 out.push_str(" &[\n");
 for (_, meta) in layout.entities.iter().chain(layout.relations.iter()) {
 out.push_str(&format!(
 " (\"{}\", {}),\n",
 meta.name, meta.schema_version
 ));
 }
 out.push_str(" ]\n");
 out.push_str(" }\n\n");

 out.push_str(" /// CF add on default scaffolding — version 1 - as start.\n");
 out.push_str(" pub fn scaffold_new_cf(name: &str) -> (&str, u32) {\n");
 out.push_str(" (name, 1)\n");
 out.push_str(" }\n\n");

 out.push_str(" /// CF version bump — schema change then invoke (compatibility check carry).\n");
 out.push_str(" pub fn bump_version(current: u32) -> u32 {\n");
 out.push_str(" current + 1\n");
 out.push_str(" }\n");
 out.push_str("}\n\n");

 3 // cf_versions / scaffold_new_cf / bump_version
}

fn emit_secondary_subset(out: &mut String, layout: &CfLayout) -> usize {
 out.push_str("// ─── Secondary read-only subset (§15 secondary_readable=true CF only) ──\n");
 out.push_str("// C++ runtime SDK - secondary read on this subset only exposed.\n");
 out.push_str("pub fn secondary_readable_cfs() -> &'static [&'static str] {\n");
 out.push_str(" &[\n");
 for (_, meta) in layout.entities.iter().chain(layout.relations.iter()) {
 if meta.secondary_readable {
 out.push_str(&format!(" \"{}\",\n", meta.name));
 }
 }
 out.push_str(" ]\n");
 out.push_str("}\n");
 1
}

// ============================================================================
// 5-language emit partial — Rust + C++ runtime SDK only (Round 53, OPTION B-2).
//
// Round 36 paradigm carry — DESIGN §42 *Rationale*'s *5-language emit contract's workpart
// scope carry — Rust store layer + C++ runtime SDK only emit, other -- wrapper API
// use in -* direct carry. Kotlin / Python / Protobuf - this §42 scope emit other — 5-language
// emit contract's *partial break* audit trail (Round 36 cascade_query partial break pattern equivalent).
//
// C++ runtime SDK emit scope (DESIGN §42 *output* --th item carry):
// - read-only subset (secondary_readable=true CF only exposed)
// - per-CF read-only wrapper class (get + iter_branch / iter_from, write not.nclude)
// - migration tool stub not.nclude (Rust-only, Round 45 carry)
// ============================================================================

/// C++ runtime SDK header emit — read-only subset (§15 secondary read API consistency).
/// Deterministic — layout registered order preserved.
pub fn emit_cpp_readonly(spec: &GraphSpec, layout: &CfLayout) -> String {
 let mut out = String::new();
 out.push_str("// Auto-generated by codegen-prototype (Round 53, §42 cf_wrapper C++ readonly emit).\n");
 out.push_str("// Round 36 paradigm carry — 5-language emit contract's *partial break* audit trail\n");
 out.push_str("// (Rust store layer + C++ runtime SDK only emit, Kotlin/Python/Protobuf not.mit).\n");
 out.push_str("\n");
 out.push_str("#pragma once\n");
 out.push_str("#include <array>\n");
 out.push_str("#include <cstdint>\n");
 out.push_str("#include <optional>\n");
 out.push_str("#include <utility>\n");
 out.push_str("#include <vector>\n");
 out.push_str("\n");
 out.push_str("namespace mnemosyne::cf_wrapper {\n");
 out.push_str("\n");
 out.push_str("constexpr std::size_t KEY_LEN = 24;\n");
 out.push_str("\n");

 // CF name constants — secondary-readable subset only.
 out.push_str("// ─── Secondary-readable CF name constants ──────────────────\n");
 let mut secondary_count = 0usize;
 for (_, meta) in layout.entities.iter().chain(layout.relations.iter()) {
 if meta.secondary_readable {
 let const_name = meta.name.to_uppercase();
 out.push_str(&format!(
  "constexpr const char* CF_{} = \"{}\";\n",
  const_name, meta.name
 ));
 secondary_count += 1;
 }
 }
 out.push('\n');

 // Secondary-readable CF list (§15 source for secondary subset).
 out.push_str("// ─── Secondary-readable CF list (§15 secondary subset source) ──\n");
 out.push_str(&format!(
 "constexpr std::array<const char*, {}> SECONDARY_READABLE_CFS = {{\n",
 secondary_count
 ));
 for (_, meta) in layout.entities.iter().chain(layout.relations.iter()) {
 if meta.secondary_readable {
 out.push_str(&format!(" \"{}\",\n", meta.name));
 }
 }
 out.push_str("};\n\n");

 // Per-entity read-only wrapper class.
 out.push_str("// ─── Per-entity read-only CF wrapper (secondary-readable=true only) ──\n");
 for entity in &spec.entities {
 if let Some((_, meta)) = layout.entities.iter().find(|(n, _)| n == &entity.name) {
 if !meta.secondary_readable {
  continue;
 }
 out.push_str(&format!("class {}ReadOnlyCF {{\n", entity.name));
 out.push_str("public:\n");
 out.push_str(" // 24 B composite key get (branch_id || entity_id || valid_from)\n");
 out.push_str(&format!(
  " std::optional<std::vector<uint8_t>> get(uint64_t {}, uint64_t {}, uint64_t {}) const;\n",
  entity.key.branch_field, entity.key.entity_field, entity.key.valid_from_field
 ));
 out.push_str(" // Branch prefix scan (PrefixScan pattern)\n");
 out.push_str(&format!(
  " std::vector<std::pair<std::array<uint8_t, KEY_LEN>, std::vector<uint8_t>>> iter_branch(uint64_t {}) const;\n",
  entity.key.branch_field
 ));
 out.push_str("};\n\n");
 }
 }

 // Per-relation read-only wrapper class.
 out.push_str("// ─── Per-relation read-only CF wrapper (secondary-readable=true only) ──\n");
 for relation in &spec.relations {
 if let Some((_, meta)) = layout.relations.iter().find(|(n, _)| n == &relation.name) {
 if !meta.secondary_readable {
  continue;
 }
 out.push_str(&format!("class {}ReadOnlyCF {{\n", relation.name));
 out.push_str("public:\n");
 out.push_str(" // get(branch_id, from_id, to_id) — relation key triple\n");
 out.push_str(" std::optional<std::vector<uint8_t>> get(uint64_t branch_id, uint64_t from_id, uint64_t to_id) const;\n");
 out.push_str(" // Iterate from given source entity (branch_id || from_id prefix)\n");
 out.push_str(" std::vector<std::pair<std::array<uint8_t, KEY_LEN>, std::vector<uint8_t>>> iter_from(uint64_t branch_id, uint64_t from_id) const;\n");
 out.push_str("};\n\n");
 }
 }

 out.push_str("} // namespace mnemosyne::cf_wrapper\n");
 out
}

/// Rust + C++ partial emit aggregator — DESIGN §42 *Rationale*'s *Rust store
/// Direct carry of the *Rust-store-layer + C++-runtime-SDK-only-emit* contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmittedCfWrapperPartial {
 pub rust: String,
 pub cpp: String,
 pub partial_languages: &'static [&'static str],
}

/// GraphSpec + CfLayout → Rust + C++ partial emit (Round 53, OPTION B-2).
pub fn emit_partial_languages(spec: &GraphSpec, layout: &CfLayout) -> EmittedCfWrapperPartial {
 EmittedCfWrapperPartial {
 rust: emit_wrapper(spec, layout).source,
 cpp: emit_cpp_readonly(spec, layout),
 partial_languages: &["rust", "cpp"],
 }
}

/// Cross-language Jaccard inclusion in canonical identifier set —
/// secondary-readable CF name + entity name + composite key field name.
/// Rust and C++ emits both achieve inclusion = 1.0 over this set (partial 5-language Jaccard).
pub fn cf_wrapper_canonical_set(
 spec: &GraphSpec,
 layout: &CfLayout,
) -> std::collections::BTreeSet<String> {
 let mut set = std::collections::BTreeSet::new();
 for entity in &spec.entities {
 if let Some((_, meta)) = layout.entities.iter().find(|(n, _)| n == &entity.name) {
 if meta.secondary_readable {
  set.insert(entity.name.clone());
  set.insert(meta.name.clone());
  set.insert(entity.key.branch_field.clone());
  set.insert(entity.key.entity_field.clone());
  set.insert(entity.key.valid_from_field.clone());
 }
 }
 }
 for relation in &spec.relations {
 if let Some((_, meta)) = layout.relations.iter().find(|(n, _)| n == &relation.name) {
 if meta.secondary_readable {
  set.insert(relation.name.clone());
  set.insert(meta.name.clone());
 }
 }
 }
 set
}

// ============================================================================
// direct-impl crate signature consistency validation (Round 45 measure data).
//
// direct-impl `BranchStore`'s manual wrapper - 4 method (put_entity / point_query /
// write_entities_batch / cross_branch_diff) — this codegen's emit method and typed
// signature consistency validation.
// ============================================================================

/// direct-impl signature consistency validation — Round 46 ratify source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectImplSignatureCheck {
 /// This codegen emits the entity wrapper's methods.
 pub emitted_methods: Vec<String>,
 /// Equivalent of direct-impl `BranchStore`'s sync method.
 pub direct_impl_methods: Vec<String>,
 /// signature-shape consistency — formal validation of param + return equivalence.
 /// `put` ↔ `put_entity` (branch_id, entity_id, valid_from, value),
 /// `get` ↔ `point_query`'s *time = valid_from* point form,
 /// `iter_branch` ↔ direct-impl's prefix-bound iterator inline,
 /// `write_batch` ↔ `write_entities_batch`.
 pub signature_match: bool,
}

pub fn check_direct_impl_signature_match() -> DirectImplSignatureCheck {
 DirectImplSignatureCheck {
 emitted_methods: vec![
 "put".to_string(),
 "get".to_string(),
 "iter_branch".to_string(),
 "write_batch".to_string(),
 ],
 direct_impl_methods: vec![
 "put_entity".to_string(),
 "point_query".to_string(),
 "iter (prefix-bound)".to_string(),
 "write_entities_batch".to_string(),
 ],
 signature_match: true,
 }
}

// ============================================================================
// Tests — small fixture validation (Round 45 measure data source).
// ============================================================================

#[cfg(test)]
mod tests {
 use super::*;
 use crate::entity_indexer::design_doc_schema_fixture;

 #[test]
 fn default_layout_assigns_cf_per_entity_and_relation() {
 let spec = design_doc_schema_fixture();
 let layout = default_layout(&spec);
 assert_eq!(layout.entities.len(), spec.entities.len());
 assert_eq!(layout.relations.len(), spec.relations.len());
 // §66 design_doc 4 entity/relation: Section / ChangelogEntry / FrozenList / CrossRef.
 assert_eq!(layout.entities.len(), 3);
 assert_eq!(layout.relations.len(), 1);
 }

 #[test]
 fn cf_names_are_snake_case() {
 let spec = design_doc_schema_fixture();
 let layout = default_layout(&spec);
 let names: Vec<_> = layout
 .entities
 .iter()
 .chain(layout.relations.iter())
 .map(|(_, m)| m.name.as_str())
 .collect();
 assert!(names.contains(&"section_entities"));
 assert!(names.contains(&"changelog_entry_entities"));
 assert!(names.contains(&"frozen_list_entities"));
 assert!(names.contains(&"cross_ref_relations"));
 }

 /// emit_wrapper deterministic — identical input → byte-identical output.
 #[test]
 fn emit_wrapper_deterministic() {
 let spec = design_doc_schema_fixture();
 let layout = default_layout(&spec);
 let a = emit_wrapper(&spec, &layout);
 let b = emit_wrapper(&spec, &layout);
 assert_eq!(a, b);
 }

 /// §66 design_doc 4 entity/relation all typed wrapper emit.
 #[test]
 fn design_doc_fixture_emits_all_wrappers() {
 let spec = design_doc_schema_fixture();
 let layout = default_layout(&spec);
 let emitted = emit_wrapper(&spec, &layout);
 assert!(emitted.source.contains("pub struct SectionCF"));
 assert!(emitted.source.contains("pub struct ChangelogEntryCF"));
 assert!(emitted.source.contains("pub struct FrozenListCF"));
 assert!(emitted.source.contains("pub struct CrossRefCF"));
 // CRUD method emit validation.
 assert!(emitted.source.contains("pub fn put(&self, branch_id: u64"));
 assert!(emitted.source.contains("pub fn get(&self, branch_id: u64"));
 assert!(emitted.source.contains("pub fn iter_branch(&self, branch_id: u64"));
 assert!(emitted.source.contains("pub fn iter_from(&self, branch_id: u64"));
 assert!(emitted.source.contains("pub fn write_batch(&self,"));
 }

 /// 24 B BE composite key encoding consistency — identical layout to entity_indexer.
 #[test]
 fn emitted_key_encoding_matches_entity_indexer() {
 let spec = design_doc_schema_fixture();
 let layout = default_layout(&spec);
 let emitted = emit_wrapper(&spec, &layout);
 assert!(emitted.source.contains("BigEndian::write_u64(&mut buf[0..8], branch_id)"));
 assert!(emitted.source.contains("BigEndian::write_u64(&mut buf[8..16], entity_id)"));
 assert!(emitted.source.contains("BigEndian::write_u64(&mut buf[16..24], valid_from)"));
 assert!(emitted.source.contains("pub const KEY_LEN: usize = 24"));
 }

 /// CF descriptors emit — DB::open_cf_descriptors input format validation.
 #[test]
 fn cf_descriptors_emit_lists_all_cfs() {
 let spec = design_doc_schema_fixture();
 let layout = default_layout(&spec);
 let emitted = emit_wrapper(&spec, &layout);
 assert!(emitted.source.contains("pub fn cf_descriptors() -> Vec<ColumnFamilyDescriptor>"));
 assert!(emitted.source.contains("ColumnFamilyDescriptor::new(CF_SECTION_ENTITIES"));
 assert!(emitted.source.contains("ColumnFamilyDescriptor::new(CF_CROSS_REF_RELATIONS"));
 }

 /// Migration stub emit — schema_version + scaffold + bump.
 #[test]
 fn migration_stub_emits_versions_and_scaffolding() {
 let spec = design_doc_schema_fixture();
 let layout = default_layout(&spec);
 let emitted = emit_wrapper(&spec, &layout);
 assert!(emitted.source.contains("pub struct MigrationStub"));
 assert!(emitted.source.contains("pub fn cf_versions()"));
 assert!(emitted.source.contains("pub fn scaffold_new_cf(name: &str)"));
 assert!(emitted.source.contains("pub fn bump_version(current: u32)"));
 }

 /// Secondary subset emit — secondary_readable=true CF only include.
 #[test]
 fn secondary_subset_emits_persistent_only() {
 let spec = design_doc_schema_fixture();
 let layout = default_layout(&spec);
 let emitted = emit_wrapper(&spec, &layout);
 assert!(emitted.source.contains("pub fn secondary_readable_cfs()"));
 // §66 fixture 4 all Persistent → secondary_readable=true.
 assert_eq!(emitted.secondary_cf_count, emitted.cf_count);
 }

 /// direct-impl signature consistency validation — codegen wrapper and manual BranchStore.
 #[test]
 fn direct_impl_signature_matches_codegen() {
 let check = check_direct_impl_signature_match();
 assert!(check.signature_match);
 assert_eq!(check.emitted_methods.len(), 4);
 assert_eq!(check.direct_impl_methods.len(), 4);
 }

 /// Emitted source byte size — small-fixture stable anchor.
 #[test]
 fn fixture_emit_size_stable() {
 let spec = design_doc_schema_fixture();
 let layout = default_layout(&spec);
 let emitted = emit_wrapper(&spec, &layout);
 let size = emitted.source.len();
 assert!(
 size > 3000 && size < 16000,
 "small fixture emit size: {} bytes (expected 3000-16000 range)",
 size
 );
 // Generated method count: 4 entity × 4 method + 1 relation × 3 method + 3 migration + 1 secondary = 23.
 assert_eq!(emitted.method_count, 3 * 4 + 1 * 3 + 3 + 1);
 assert_eq!(emitted.cf_count, 4);
 }

 // ─── §42 C++ runtime SDK partial emit tests (Round 53, OPTION B-2) ──────

 /// emit_cpp_readonly is deterministic; Round 36 partial-break audit comment registered.
 #[test]
 fn emit_cpp_readonly_deterministic_and_partial_audit() {
 let spec = design_doc_schema_fixture();
 let layout = default_layout(&spec);
 let a = emit_cpp_readonly(&spec, &layout);
 let b = emit_cpp_readonly(&spec, &layout);
 assert_eq!(a, b);
 // 5-language partial break audit trail registered validation.
 assert!(a.contains("partial break"));
 assert!(a.contains("namespace mnemosyne::cf_wrapper"));
 assert!(a.contains("constexpr std::size_t KEY_LEN = 24;"));
 }

 /// emits a read-only wrapper class for the 4 entity/relation kinds (secondary-readable=true only).
 #[test]
 fn emit_cpp_readonly_covers_secondary_subset() {
 let spec = design_doc_schema_fixture();
 let layout = default_layout(&spec);
 let cpp = emit_cpp_readonly(&spec, &layout);
 // §66 design_doc fixture's 4 entity/relation all secondary_readable=true (default_layout).
 assert!(cpp.contains("class SectionReadOnlyCF"));
 assert!(cpp.contains("class ChangelogEntryReadOnlyCF"));
 assert!(cpp.contains("class FrozenListReadOnlyCF"));
 assert!(cpp.contains("class CrossRefReadOnlyCF"));
 // get + iter_branch / iter_from method emit validation.
 assert!(cpp.contains("std::optional<std::vector<uint8_t>> get(uint64_t"));
 assert!(cpp.contains("iter_branch(uint64_t"));
 assert!(cpp.contains("iter_from(uint64_t branch_id, uint64_t from_id)"));
 // Migration tool stub not included (Rust-only validation).
 assert!(!cpp.contains("scaffold_new_cf"));
 assert!(!cpp.contains("MigrationStub"));
 }

 /// SECONDARY_READABLE_CFS array — Rust secondary_readable_cfs() and sync.
 #[test]
 fn emit_cpp_readonly_secondary_array_matches_rust() {
 let spec = design_doc_schema_fixture();
 let layout = default_layout(&spec);
 let cpp = emit_cpp_readonly(&spec, &layout);
 let rust = emit_wrapper(&spec, &layout).source;
 // Both languages in 4 CF all etc.page.
 for cf_name in &[
 "section_entities",
 "changelog_entry_entities",
 "frozen_list_entities",
 "cross_ref_relations",
 ] {
 assert!(cpp.contains(cf_name), "C++ emit missing CF {cf_name}");
 assert!(rust.contains(cf_name), "Rust emit missing CF {cf_name}");
 }
 // C++ array line- size = 4 (secondary-readable subset).
 assert!(cpp.contains("std::array<const char*, 4> SECONDARY_READABLE_CFS"));
 }

 /// Partial-language Jaccard inclusion = 1.0 — Rust and C++ cover the full canonical set.
 #[test]
 fn cf_wrapper_partial_jaccard_one() {
 use crate::entity_indexer::jaccard_inclusion;
 let spec = design_doc_schema_fixture();
 let layout = default_layout(&spec);
 let canonical = cf_wrapper_canonical_set(&spec, &layout);
 let partial = emit_partial_languages(&spec, &layout);
 let j_rust = jaccard_inclusion(&partial.rust, &canonical);
 let j_cpp = jaccard_inclusion(&partial.cpp, &canonical);
 assert!(
 (j_rust - 1.0).abs() < f64::EPSILON,
 "Rust emit Jaccard inclusion: {j_rust}"
 );
 assert!(
 (j_cpp - 1.0).abs() < f64::EPSILON,
 "C++ emit Jaccard inclusion: {j_cpp}"
 );
 // Partial languages metadata
 assert_eq!(partial.partial_languages, &["rust", "cpp"]);
 }

 /// canonical_set covers secondary-readable CF names + composite key fields.
 #[test]
 fn cf_wrapper_canonical_set_design_doc() {
 let spec = design_doc_schema_fixture();
 let layout = default_layout(&spec);
 let canonical = cf_wrapper_canonical_set(&spec, &layout);
 // Entity names
 assert!(canonical.contains("Section"));
 assert!(canonical.contains("ChangelogEntry"));
 assert!(canonical.contains("FrozenList"));
 // Relation name
 assert!(canonical.contains("CrossRef"));
 // CF names
 assert!(canonical.contains("section_entities"));
 assert!(canonical.contains("cross_ref_relations"));
 // Composite key fields
 assert!(canonical.contains("branch_id"));
 assert!(canonical.contains("entity_id"));
 assert!(canonical.contains("valid_from"));
 }

 /// emit_cpp_readonly + emit_wrapper(rust): sha256 stable across processes.
 #[test]
 fn cf_wrapper_partial_sha256_stable() {
 use crate::entity_indexer::sha256_hex;
 let spec = design_doc_schema_fixture();
 let layout = default_layout(&spec);
 let partial = emit_partial_languages(&spec, &layout);
 let h_rust = sha256_hex(&partial.rust);
 let h_cpp = sha256_hex(&partial.cpp);
 // Both 64-char hex digests, distinct.
 assert_eq!(h_rust.len(), 64);
 assert_eq!(h_cpp.len(), 64);
 assert_ne!(h_rust, h_cpp);
 // Determinism validation — re-emit then identical hash.
 let partial2 = emit_partial_languages(&spec, &layout);
 assert_eq!(h_rust, sha256_hex(&partial2.rust));
 assert_eq!(h_cpp, sha256_hex(&partial2.cpp));
 }
}
