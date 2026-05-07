//! §39 entity-relation graph indexer codegen prototype (Round 42, first scope).
//!
//! Round 43 substantive implementation — Forge AST stub (mock) + Rust struct
//! emit + unified 24 B fixed-width BE composite key encoding + §66 design_doc
//! schema fixture (Section / CrossRef / ChangelogEntry / FrozenList).
//!
//! Input spec (Phase -1A stage 2C measurement carry, ROADMAP Phase -1B
//! Source for self-application codegen scope (a):
//! 1. Composite key: branch_id (u64 BE) || entity_id (u64 BE) || valid_from (u64 BE) — 24 B fixed-width
//! 2. Asset refs: row-per-(asset, fact) normalized (478× faster on hot facts)
//! 3. Encounter (§44 meta agent): row-per-encounter (2.27× faster)
//! 4. Epistemic CF separation: explicit CF + derived CF kept separate
//!  (provenance overhead 16.4%, §11 closure separation)
//!
//! Prototype role (Round 42 ratify):
//! - Lives in bench/codegen-prototype (§18 line 1954 *prototype scope* boundary).
//! - Emits Rust core only this round; 5-language emit consistency validation
//! Follow-up round.
//! - SCE Forge §40 Codec multi-component RFC progress is upstream and out of
//! scope for this entry (Round 33 signal-4 carry).

use byteorder::{BigEndian, ByteOrder};
use serde::{Deserialize, Serialize};

/// 24 B fixed-width composite key encoding (Phase -1A stage 2C carry, DESIGN §18 line 1845).
pub const KEY_LEN: usize = 24;
pub const BRANCH_ID_OFFSET: usize = 0;
pub const ENTITY_ID_OFFSET: usize = 8;
pub const VALID_FROM_OFFSET: usize = 16;

// ============================================================================
// Forge AST stub (mock) — once apis/ Forge AST schema stabilizes, this is
// replaced by the real AST consumer path. This round uses a mock to
// validate format consistency only.
// ============================================================================

/// Field type — typed-field expression for the 4 Phase -1A stage 2C decisions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum FieldType {
 /// `u64 BE` encoding (composite key component, 8 B).
 U64BigEndian,
 /// Variable-length UTF-8 string.
 String,
 /// Variable-length byte array.
 Bytes,
 /// Reference to another entity by ID (u64 BE encoded; foreign-key semantics).
 EntityRef { target: String },
 /// Row-per-(asset, fact) normalized layout (asset_refs decision —
 /// stage 2C, DESIGN §18 line 1901).
 NormalizedAssetRefs,
 /// Row-per-encounter layout (§44 meta agent decision — stage 2C,
 /// DESIGN §18 line 1903).
 RowPerEncounter,
}

/// Persistence policy (Round 33.5 fine-resolution mapping carry — driven by
/// the annotation system, distinct from other kinds).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Persistence {
 /// `@persistent` — resides in the explicit CF (epistemic separation;
 /// the *explicit* side per §18 line 1904).
 Persistent,
 /// `@derived` — resides in the derived CF (epistemic separation;
 /// the *derived* side per §11 closure separation).
 Derived,
}

/// Composite key descriptor — §40 Codec multi-component instance (Round 33.5
/// fine-resolution mapping carry). For this prototype only the stage 2C
/// decision `branch_id || entity_id || valid_from` 24 B fixed-width layout is
/// supported.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompositeKey {
 /// 8 B BE prefix — branch overlay (§1 single-writer per branch isolation source).
 pub branch_field: String,
 /// 8 B BE — entity_id (foreign key meaning).
 pub entity_field: String,
 /// 8 B BE — valid_time (bi-temporal lower bound).
 pub valid_from_field: String,
}

impl Default for CompositeKey {
 fn default() -> Self {
 Self {
 branch_field: "branch_id".to_string(),
 entity_field: "entity_id".to_string(),
 valid_from_field: "valid_from".to_string(),
 }
 }
}

/// Field — single entity component.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldDef {
 pub name: String,
 pub ty: FieldType,
 pub nullable: bool,
}

/// Entity — typed-fact node (Round 33.5 fine-resolution mapping carry —
/// §40 Codec multi-component instance; persistence policy is registered
/// via the annotation system, distinct from other kinds).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityDef {
 pub name: String,
 pub fields: Vec<FieldDef>,
 pub key: CompositeKey,
 pub persistence: Persistence,
}

/// Relation — typed edge (directed link between entities).
/// This prototype only supports binary relations (Phase -1B scope).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationDef {
 pub name: String,
 pub from_entity: String,
 pub to_entity: String,
 pub fields: Vec<FieldDef>,
 pub persistence: Persistence,
}

/// Forge AST's mnemosyne itself codegen input — entity/relation graph spec.
/// Once the Forge AST schema stabilizes, replace this type with the real AST consumer.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphSpec {
 pub entities: Vec<EntityDef>,
 pub relations: Vec<RelationDef>,
}

// ============================================================================
// 24 B fixed-width BE composite key encoding (stage 2C carry).
// direct-impl crate's identical pattern — this prototype - codegen emit above this function-
// create are emitted -. This round is generation source.
// ============================================================================

/// 24 B composite key encode — branch_id (u64 BE) || entity_id (u64 BE) || valid_from (u64 BE).
pub fn encode_composite_key(branch_id: u64, entity_id: u64, valid_from: u64) -> [u8; KEY_LEN] {
 let mut buf = [0u8; KEY_LEN];
 BigEndian::write_u64(&mut buf[BRANCH_ID_OFFSET..ENTITY_ID_OFFSET], branch_id);
 BigEndian::write_u64(&mut buf[ENTITY_ID_OFFSET..VALID_FROM_OFFSET], entity_id);
 BigEndian::write_u64(&mut buf[VALID_FROM_OFFSET..KEY_LEN], valid_from);
 buf
}

/// 24 B composite key decode.
pub fn decode_composite_key(buf: &[u8; KEY_LEN]) -> (u64, u64, u64) {
 let branch_id = BigEndian::read_u64(&buf[BRANCH_ID_OFFSET..ENTITY_ID_OFFSET]);
 let entity_id = BigEndian::read_u64(&buf[ENTITY_ID_OFFSET..VALID_FROM_OFFSET]);
 let valid_from = BigEndian::read_u64(&buf[VALID_FROM_OFFSET..KEY_LEN]);
 (branch_id, entity_id, valid_from)
}

// ============================================================================
// Rust struct emit (codegen) — GraphSpec → Rust source string.
// this prototype - deterministic emit guaranteed (identical input → byte-identical output).
// 5-language emit consistency validation- subsequent round (Kotlin / Python / C++ / protobuf).
// ============================================================================

/// Codegen emit result — Rust source code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmittedRust {
 pub source: String,
}

/// GraphSpec → Rust source code emit.
/// Deterministic — preserves entities and relations in registration order from the spec.
pub fn emit_rust(spec: &GraphSpec) -> EmittedRust {
 let mut out = String::new();
 out.push_str("// Auto-generated by codegen-prototype (Round 43, §39 entity-indexer).\n");
 out.push_str("// input source: Forge AST stub. Phase -1A stage 2C carry — 24 B BE composite key.\n");
 out.push_str("\n");
 out.push_str("use byteorder::{BigEndian, ByteOrder};\n");
 out.push_str("\n");
 out.push_str("pub const KEY_LEN: usize = 24;\n");
 out.push_str("\n");

 for entity in &spec.entities {
 emit_entity(&mut out, entity);
 }
 for relation in &spec.relations {
 emit_relation(&mut out, relation);
 }

 EmittedRust { source: out }
}

fn emit_entity(out: &mut String, entity: &EntityDef) {
 let persistence_marker = match entity.persistence {
 Persistence::Persistent => "// @persistent — explicit CF",
 Persistence::Derived => "// @derived — derived CF",
 };
 out.push_str(&format!("{persistence_marker}\n"));
 out.push_str(&format!("#[derive(Debug, Clone)]\n"));
 out.push_str(&format!("pub struct {} {{\n", entity.name));
 out.push_str(&format!(
 " pub {}: u64, // composite key — 8 B BE prefix\n",
 entity.key.branch_field
 ));
 out.push_str(&format!(
 " pub {}: u64, // composite key — 8 B BE entity_id\n",
 entity.key.entity_field
 ));
 out.push_str(&format!(
 " pub {}: u64, // composite key — 8 B BE valid_from\n",
 entity.key.valid_from_field
 ));
 for field in &entity.fields {
 out.push_str(&format!(
 " pub {}: {},\n",
 field.name,
 rust_type_for(&field.ty, field.nullable)
 ));
 }
 out.push_str("}\n");
 out.push_str(&format!("impl {} {{\n", entity.name));
 out.push_str(&format!(
 " pub fn encode_key(&self) -> [u8; KEY_LEN] {{\n"
 ));
 out.push_str(&format!(
 " let mut buf = [0u8; KEY_LEN];\n"
 ));
 out.push_str(&format!(
 " BigEndian::write_u64(&mut buf[0..8], self.{});\n",
 entity.key.branch_field
 ));
 out.push_str(&format!(
 " BigEndian::write_u64(&mut buf[8..16], self.{});\n",
 entity.key.entity_field
 ));
 out.push_str(&format!(
 " BigEndian::write_u64(&mut buf[16..24], self.{});\n",
 entity.key.valid_from_field
 ));
 out.push_str(" buf\n");
 out.push_str(" }\n");
 out.push_str("}\n\n");
}

fn emit_relation(out: &mut String, relation: &RelationDef) {
 let persistence_marker = match relation.persistence {
 Persistence::Persistent => "// @persistent — explicit CF",
 Persistence::Derived => "// @derived — derived CF",
 };
 out.push_str(&format!("{persistence_marker}\n"));
 out.push_str(&format!("#[derive(Debug, Clone)]\n"));
 out.push_str(&format!("pub struct {} {{\n", relation.name));
 out.push_str(&format!(
 " pub from: u64, // {} entity_id\n",
 relation.from_entity
 ));
 out.push_str(&format!(
 " pub to: u64, // {} entity_id\n",
 relation.to_entity
 ));
 for field in &relation.fields {
 out.push_str(&format!(
 " pub {}: {},\n",
 field.name,
 rust_type_for(&field.ty, field.nullable)
 ));
 }
 out.push_str("}\n\n");
}

fn rust_type_for(ty: &FieldType, nullable: bool) -> String {
 let base = match ty {
 FieldType::U64BigEndian => "u64".to_string(),
 FieldType::String => "String".to_string(),
 FieldType::Bytes => "Vec<u8>".to_string(),
 FieldType::EntityRef { target } => format!("/* ref: {target} */ u64"),
 FieldType::NormalizedAssetRefs => "Vec<u64> // row-per-(asset, fact) normalized".to_string(),
 FieldType::RowPerEncounter => "Vec<u64> // row-per-encounter (§44)".to_string(),
 };
 if nullable {
 format!("Option<{base}>")
 } else {
 base
 }
}

// ============================================================================
// 5-language emit (Round 52, OPTION B-1) — Kotlin / Python / C++ / protobuf add.
// Round 33.5 minuteresolve mapping carry — mnemosyne itself codegen file---recognize's 5-language emit
// contract (Rust / Kotlin / Python / C++ / protobuf). This round is §39 entity-indexer
//'s 5 language sync.emit + content-addressable sha256 hash introduce + cross-language
// Jaccard consistency validation.
// ============================================================================

/// 5-language unified emit result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmittedMultiLang {
 pub rust: String,
 pub kotlin: String,
 pub python: String,
 pub cpp: String,
 pub protobuf: String,
}

/// SHA-256 hex digest — content-addressable stable hash (DefaultHasher RandomState
/// threshold break, cross-process / cross-run byte-identical validation possible).
pub fn sha256_hex(s: &str) -> String {
 use sha2::{Digest, Sha256};
 let mut hasher = Sha256::new();
 hasher.update(s.as_bytes());
 let digest = hasher.finalize();
 let mut hex = String::with_capacity(64);
 for byte in digest.iter() {
 hex.push_str(&format!("{:02x}", byte));
 }
 hex
}

/// GraphSpec → 5-language emit (Rust / Kotlin / Python / C++ / protobuf).
/// Deterministic — identical input yields byte-identical results across all 5 emits.
pub fn emit_all_languages(spec: &GraphSpec) -> EmittedMultiLang {
 EmittedMultiLang {
 rust: emit_rust(spec).source,
 kotlin: emit_kotlin(spec),
 python: emit_python(spec),
 cpp: emit_cpp(spec),
 protobuf: emit_protobuf(spec),
 }
}

// ─── Kotlin emit ────────────────────────────────────────────────────────────

/// GraphSpec → Kotlin source emit (data class — 5-language consistency validation source).
pub fn emit_kotlin(spec: &GraphSpec) -> String {
 let mut out = String::new();
 out.push_str("// Auto-generated by codegen-prototype (Round 52, §39 entity-indexer Kotlin emit).\n");
 out.push_str("// 5-language emit consistency validation source — Rust core baseline; Kotlin tracks via sync.\n");
 out.push_str("\n");
 out.push_str("package mnemosyne.generated\n");
 out.push_str("\n");
 out.push_str("const val KEY_LEN: Int = 24\n");
 out.push_str("\n");

 for entity in &spec.entities {
 let marker = match entity.persistence {
 Persistence::Persistent => "// @persistent — explicit CF",
 Persistence::Derived => "// @derived — derived CF",
 };
 out.push_str(marker);
 out.push('\n');
 out.push_str(&format!("data class {}(\n", entity.name));
 out.push_str(&format!(
 " val {}: ULong, // composite key — 8 B BE prefix\n",
 entity.key.branch_field
 ));
 out.push_str(&format!(
 " val {}: ULong, // composite key — 8 B BE entity_id\n",
 entity.key.entity_field
 ));
 out.push_str(&format!(
 " val {}: ULong, // composite key — 8 B BE valid_from\n",
 entity.key.valid_from_field
 ));
 for field in &entity.fields {
 out.push_str(&format!(
  " val {}: {},\n",
  field.name,
  kotlin_type_for(&field.ty, field.nullable)
 ));
 }
 out.push_str(")\n\n");
 }
 for relation in &spec.relations {
 let marker = match relation.persistence {
 Persistence::Persistent => "// @persistent — explicit CF",
 Persistence::Derived => "// @derived — derived CF",
 };
 out.push_str(marker);
 out.push('\n');
 out.push_str(&format!("data class {}(\n", relation.name));
 out.push_str(&format!(
 " val from: ULong, // {} entity_id\n",
 relation.from_entity
 ));
 out.push_str(&format!(
 " val to: ULong, // {} entity_id\n",
 relation.to_entity
 ));
 for field in &relation.fields {
 out.push_str(&format!(
  " val {}: {},\n",
  field.name,
  kotlin_type_for(&field.ty, field.nullable)
 ));
 }
 out.push_str(")\n\n");
 }
 out
}

fn kotlin_type_for(ty: &FieldType, nullable: bool) -> String {
 let base = match ty {
 FieldType::U64BigEndian => "ULong".to_string(),
 FieldType::String => "String".to_string(),
 FieldType::Bytes => "ByteArray".to_string(),
 FieldType::EntityRef { target } => format!("ULong /* ref: {target} */"),
 FieldType::NormalizedAssetRefs => "List<ULong> /* row-per-(asset, fact) normalized */".to_string(),
 FieldType::RowPerEncounter => "List<ULong> /* row-per-encounter (§44) */".to_string(),
 };
 if nullable {
 format!("{base}?")
 } else {
 base
 }
}

// ─── Python emit ────────────────────────────────────────────────────────────

/// GraphSpec → Python source emit (@dataclass — 5-language consistency validation source).
pub fn emit_python(spec: &GraphSpec) -> String {
 let mut out = String::new();
 out.push_str("# Auto-generated by codegen-prototype (Round 52, §39 entity-indexer Python emit).\n");
 out.push_str("# 5-language emit consistency validation source — Rust core baseline; Python tracks via sync.\n");
 out.push_str("\n");
 out.push_str("from dataclasses import dataclass\n");
 out.push_str("from typing import Optional, List\n");
 out.push_str("\n");
 out.push_str("KEY_LEN: int = 24\n");
 out.push_str("\n");

 for entity in &spec.entities {
 let marker = match entity.persistence {
 Persistence::Persistent => "# @persistent — explicit CF",
 Persistence::Derived => "# @derived — derived CF",
 };
 out.push_str(marker);
 out.push('\n');
 out.push_str("@dataclass\n");
 out.push_str(&format!("class {}:\n", entity.name));
 out.push_str(&format!(
 " {}: int # composite key — 8 B BE prefix\n",
 entity.key.branch_field
 ));
 out.push_str(&format!(
 " {}: int # composite key — 8 B BE entity_id\n",
 entity.key.entity_field
 ));
 out.push_str(&format!(
 " {}: int # composite key — 8 B BE valid_from\n",
 entity.key.valid_from_field
 ));
 for field in &entity.fields {
 out.push_str(&format!(
  " {}: {}\n",
  field.name,
  python_type_for(&field.ty, field.nullable)
 ));
 }
 out.push('\n');
 }
 for relation in &spec.relations {
 let marker = match relation.persistence {
 Persistence::Persistent => "# @persistent — explicit CF",
 Persistence::Derived => "# @derived — derived CF",
 };
 out.push_str(marker);
 out.push('\n');
 out.push_str("@dataclass\n");
 out.push_str(&format!("class {}:\n", relation.name));
 out.push_str(&format!(
 " from_: int # {} entity_id\n",
 relation.from_entity
 ));
 out.push_str(&format!(
 " to: int # {} entity_id\n",
 relation.to_entity
 ));
 for field in &relation.fields {
 out.push_str(&format!(
  " {}: {}\n",
  field.name,
  python_type_for(&field.ty, field.nullable)
 ));
 }
 out.push('\n');
 }
 out
}

fn python_type_for(ty: &FieldType, nullable: bool) -> String {
 let base = match ty {
 FieldType::U64BigEndian => "int".to_string(),
 FieldType::String => "str".to_string(),
 FieldType::Bytes => "bytes".to_string(),
 FieldType::EntityRef { target } => format!("int # ref: {target}"),
 FieldType::NormalizedAssetRefs => "List[int] # row-per-(asset, fact) normalized".to_string(),
 FieldType::RowPerEncounter => "List[int] # row-per-encounter (§44)".to_string(),
 };
 if nullable {
 format!("Optional[{base}]")
 } else {
 base
 }
}

// ─── C++ emit ───────────────────────────────────────────────────────────────

/// GraphSpec → C++ header emit (struct — 5-language consistency validation source).
pub fn emit_cpp(spec: &GraphSpec) -> String {
 let mut out = String::new();
 out.push_str("// Auto-generated by codegen-prototype (Round 52, §39 entity-indexer C++ emit).\n");
 out.push_str("// 5-language emit consistency validation source — Rust core baseline; C++ tracks via sync.\n");
 out.push_str("\n");
 out.push_str("#pragma once\n");
 out.push_str("#include <cstdint>\n");
 out.push_str("#include <string>\n");
 out.push_str("#include <vector>\n");
 out.push_str("#include <optional>\n");
 out.push_str("\n");
 out.push_str("namespace mnemosyne::generated {\n");
 out.push_str("\n");
 out.push_str("constexpr std::size_t KEY_LEN = 24;\n");
 out.push_str("\n");

 for entity in &spec.entities {
 let marker = match entity.persistence {
 Persistence::Persistent => "// @persistent — explicit CF",
 Persistence::Derived => "// @derived — derived CF",
 };
 out.push_str(marker);
 out.push('\n');
 out.push_str(&format!("struct {} {{\n", entity.name));
 out.push_str(&format!(
 " uint64_t {}; // composite key — 8 B BE prefix\n",
 entity.key.branch_field
 ));
 out.push_str(&format!(
 " uint64_t {}; // composite key — 8 B BE entity_id\n",
 entity.key.entity_field
 ));
 out.push_str(&format!(
 " uint64_t {}; // composite key — 8 B BE valid_from\n",
 entity.key.valid_from_field
 ));
 for field in &entity.fields {
 out.push_str(&format!(
  " {} {};\n",
  cpp_type_for(&field.ty, field.nullable),
  field.name
 ));
 }
 out.push_str("};\n\n");
 }
 for relation in &spec.relations {
 let marker = match relation.persistence {
 Persistence::Persistent => "// @persistent — explicit CF",
 Persistence::Derived => "// @derived — derived CF",
 };
 out.push_str(marker);
 out.push('\n');
 out.push_str(&format!("struct {} {{\n", relation.name));
 out.push_str(&format!(
 " uint64_t from; // {} entity_id\n",
 relation.from_entity
 ));
 out.push_str(&format!(
 " uint64_t to; // {} entity_id\n",
 relation.to_entity
 ));
 for field in &relation.fields {
 out.push_str(&format!(
  " {} {};\n",
  cpp_type_for(&field.ty, field.nullable),
  field.name
 ));
 }
 out.push_str("};\n\n");
 }
 out.push_str("} // namespace mnemosyne::generated\n");
 out
}

fn cpp_type_for(ty: &FieldType, nullable: bool) -> String {
 let base = match ty {
 FieldType::U64BigEndian => "uint64_t".to_string(),
 FieldType::String => "std::string".to_string(),
 FieldType::Bytes => "std::vector<uint8_t>".to_string(),
 FieldType::EntityRef { target } => format!("uint64_t /* ref: {target} */"),
 FieldType::NormalizedAssetRefs => "std::vector<uint64_t> /* row-per-(asset, fact) normalized */".to_string(),
 FieldType::RowPerEncounter => "std::vector<uint64_t> /* row-per-encounter (§44) */".to_string(),
 };
 if nullable {
 format!("std::optional<{base}>")
 } else {
 base
 }
}

// ─── Protobuf emit ──────────────────────────────────────────────────────────

/// GraphSpec → Protobuf .proto emit — 5-language consistency validation source — wire format-table).
pub fn emit_protobuf(spec: &GraphSpec) -> String {
 let mut out = String::new();
 out.push_str("// Auto-generated by codegen-prototype (Round 52, §39 entity-indexer protobuf emit).\n");
 out.push_str("// 5-language emit consistency validation source — Rust core baseline; protobuf tracks via sync.\n");
 out.push_str("\n");
 out.push_str("syntax = \"proto3\";\n");
 out.push_str("\n");
 out.push_str("package mnemosyne.generated;\n");
 out.push_str("\n");
 out.push_str("// KEY_LEN = 24 (composite key's wire size, external const)\n");
 out.push_str("\n");

 for entity in &spec.entities {
 let marker = match entity.persistence {
 Persistence::Persistent => "// @persistent — explicit CF",
 Persistence::Derived => "// @derived — derived CF",
 };
 out.push_str(marker);
 out.push('\n');
 out.push_str(&format!("message {} {{\n", entity.name));
 let mut idx = 1u32;
 out.push_str(&format!(
 " uint64 {} = {}; // composite key — 8 B BE prefix\n",
 entity.key.branch_field, idx
 ));
 idx += 1;
 out.push_str(&format!(
 " uint64 {} = {}; // composite key — 8 B BE entity_id\n",
 entity.key.entity_field, idx
 ));
 idx += 1;
 out.push_str(&format!(
 " uint64 {} = {}; // composite key — 8 B BE valid_from\n",
 entity.key.valid_from_field, idx
 ));
 idx += 1;
 for field in &entity.fields {
 out.push_str(&format!(
  " {} {} = {};\n",
  proto_type_prefix(&field.ty, field.nullable),
  field.name,
  idx
 ));
 idx += 1;
 }
 out.push_str("}\n\n");
 }
 for relation in &spec.relations {
 let marker = match relation.persistence {
 Persistence::Persistent => "// @persistent — explicit CF",
 Persistence::Derived => "// @derived — derived CF",
 };
 out.push_str(marker);
 out.push('\n');
 out.push_str(&format!("message {} {{\n", relation.name));
 let mut idx = 1u32;
 out.push_str(&format!(
 " uint64 from = {}; // {} entity_id\n",
 idx, relation.from_entity
 ));
 idx += 1;
 out.push_str(&format!(
 " uint64 to = {}; // {} entity_id\n",
 idx, relation.to_entity
 ));
 idx += 1;
 for field in &relation.fields {
 out.push_str(&format!(
  " {} {} = {};\n",
  proto_type_prefix(&field.ty, field.nullable),
  field.name,
  idx
 ));
 idx += 1;
 }
 out.push_str("}\n\n");
 }
 out
}

fn proto_type_prefix(ty: &FieldType, nullable: bool) -> String {
 let base = match ty {
 FieldType::U64BigEndian => "uint64",
 FieldType::String => "string",
 FieldType::Bytes => "bytes",
 FieldType::EntityRef { .. } => "uint64",
 FieldType::NormalizedAssetRefs => "repeated uint64",
 FieldType::RowPerEncounter => "repeated uint64",
 };
 if nullable && !base.starts_with("repeated") {
 format!("optional {base}")
 } else {
 base.to_string()
 }
}

// ─── Cross-language Jaccard consistency validation ────────────────────────────────────

/// GraphSpec's canonical identifier set (entity name + relation name + field name).
/// All 5-language emits validate Jaccard = 1.0 over this set — detects any missing identifier.
pub fn canonical_identifier_set(spec: &GraphSpec) -> std::collections::BTreeSet<String> {
 let mut set = std::collections::BTreeSet::new();
 for entity in &spec.entities {
 set.insert(entity.name.clone());
 set.insert(entity.key.branch_field.clone());
 set.insert(entity.key.entity_field.clone());
 set.insert(entity.key.valid_from_field.clone());
 for field in &entity.fields {
 set.insert(field.name.clone());
 }
 }
 for relation in &spec.relations {
 set.insert(relation.name.clone());
 for field in &relation.fields {
 set.insert(field.name.clone());
 }
 }
 set
}

/// Validates inclusion of the canonical identifiers in the emitted text — every identifier must appear.
/// Jaccard inclusion: intersection covers the full set; union also covers the full set (no missing identifiers).
/// heuristic lower bound — substring match against the identifier string (false positives are trivially possible,
/// This prototype uses unique identifier names → 0 false positives.
pub fn jaccard_inclusion(emit_text: &str, canonical: &std::collections::BTreeSet<String>) -> f64 {
 let included = canonical
 .iter()
 .filter(|id| emit_text.contains(id.as_str()))
 .count();
 if canonical.is_empty() {
 return 1.0;
 }
 included as f64 / canonical.len() as f64
}

// ============================================================================
// §66 design_doc schema fixture — Phase 0 entry block prerequisite #3.
// Section / CrossRef / ChangelogEntry / FrozenList 4 entity/relation.
// ============================================================================

/// §66 design_doc schema fixture — Phase 0 entry block prerequisite #3 source.
/// This fixture is the Round 43 prototype measurement source — small-fixture validation.
pub fn design_doc_schema_fixture() -> GraphSpec {
 GraphSpec {
 entities: vec![
 EntityDef {
  name: "Section".to_string(),
  fields: vec![
  FieldDef {
  name: "doc_path".to_string(),
  ty: FieldType::String,
  nullable: false,
  },
  FieldDef {
  name: "section_id".to_string(),
  ty: FieldType::String,
  nullable: false,
  },
  FieldDef {
  name: "title".to_string(),
  ty: FieldType::String,
  nullable: false,
  },
  FieldDef {
  name: "decision_status".to_string(),
  ty: FieldType::String,
  nullable: false,
  },
  ],
  key: CompositeKey::default(),
  persistence: Persistence::Persistent,
 },
 EntityDef {
  name: "ChangelogEntry".to_string(),
  fields: vec![
  FieldDef {
  name: "round_number".to_string(),
  ty: FieldType::U64BigEndian,
  nullable: false,
  },
  FieldDef {
  name: "summary".to_string(),
  ty: FieldType::String,
  nullable: false,
  },
  FieldDef {
  name: "appended_at".to_string(),
  ty: FieldType::U64BigEndian,
  nullable: false,
  },
  ],
  key: CompositeKey::default(),
  persistence: Persistence::Persistent,
 },
 EntityDef {
  name: "FrozenList".to_string(),
  fields: vec![
  FieldDef {
  name: "owner_section".to_string(),
  ty: FieldType::EntityRef {
   target: "Section".to_string(),
  },
  nullable: false,
  },
  FieldDef {
  name: "frozen_round".to_string(),
  ty: FieldType::U64BigEndian,
  nullable: false,
  },
  FieldDef {
  name: "kind".to_string(),
  ty: FieldType::String,
  nullable: false,
  },
  ],
  key: CompositeKey::default(),
  persistence: Persistence::Persistent,
 },
 ],
 relations: vec![RelationDef {
 name: "CrossRef".to_string(),
 from_entity: "Section".to_string(),
 to_entity: "Section".to_string(),
 fields: vec![FieldDef {
  name: "ref_kind".to_string(),
  ty: FieldType::String,
  nullable: false,
 }],
 persistence: Persistence::Persistent,
 }],
 }
}

// ============================================================================
// Tests — small fixture validation (Round 43 measure data source).
// ============================================================================

#[cfg(test)]
mod tests {
 use super::*;

 /// 24 B fixed-width composite key encoding round-trip — Phase -1A stage 2C carry.
 #[test]
 fn composite_key_round_trip() {
 let cases: &[(u64, u64, u64)] = &[
 (0, 0, 0),
 (1, 1, 1),
 (u64::MAX, u64::MAX, u64::MAX),
 (u64::MAX / 2, u64::MAX / 2, u64::MAX / 2),
 (1, 2, 3),
 ];
 for &(b, e, v) in cases {
 let buf = encode_composite_key(b, e, v);
 assert_eq!(buf.len(), KEY_LEN);
 let (b2, e2, v2) = decode_composite_key(&buf);
 assert_eq!((b, e, v), (b2, e2, v2));
 }
 }

 /// 24 B fixed-width prefix scan ordering — bi-temporal scan order's source.
 #[test]
 fn composite_key_lex_order_matches_numeric() {
 let k1 = encode_composite_key(1, 1, 100);
 let k2 = encode_composite_key(1, 1, 200);
 let k3 = encode_composite_key(1, 2, 50);
 let k4 = encode_composite_key(2, 0, 0);
 assert!(k1 < k2);
 assert!(k2 < k3);
 assert!(k3 < k4);
 }

 /// emit_rust deterministic — identical input → byte-identical output.
 #[test]
 fn emit_rust_deterministic() {
 let spec = design_doc_schema_fixture();
 let a = emit_rust(&spec);
 let b = emit_rust(&spec);
 assert_eq!(a, b, "emit_rust - deterministic — identical input → byte-identical");
 }

 /// §66 design_doc fixture emit — 4 entity/relation all emit.
 #[test]
 fn design_doc_fixture_emits_all_kinds() {
 let spec = design_doc_schema_fixture();
 let emitted = emit_rust(&spec);
 assert!(emitted.source.contains("pub struct Section"));
 assert!(emitted.source.contains("pub struct ChangelogEntry"));
 assert!(emitted.source.contains("pub struct FrozenList"));
 assert!(emitted.source.contains("pub struct CrossRef"));
 // 24 B composite key encoding emit position validation
 assert!(emitted.source.contains("KEY_LEN"));
 assert!(emitted.source.contains("BigEndian::write_u64(&mut buf[0..8]"));
 assert!(emitted.source.contains("BigEndian::write_u64(&mut buf[8..16]"));
 assert!(emitted.source.contains("BigEndian::write_u64(&mut buf[16..24]"));
 }

 /// FrozenList → Section EntityRef field — foreign key meaning preserved.
 #[test]
 fn entity_ref_emits_target_comment() {
 let spec = design_doc_schema_fixture();
 let emitted = emit_rust(&spec);
 assert!(
 emitted.source.contains("/* ref: Section */ u64"),
 "FrozenList.owner_section EntityRef - target=Section explicit emit"
 );
 }

 /// Persistence annotation emit — @persistent / @derived marker.
 #[test]
 fn persistence_annotation_emits() {
 let spec = design_doc_schema_fixture();
 let emitted = emit_rust(&spec);
 assert!(emitted.source.contains("@persistent"));
 }

 /// Stability of the emitted source's byte length — small-fixture (4 entity/relation) stable anchor.
 /// This measurement data is the design-round ratify source (Round 44).
 #[test]
 fn fixture_emit_size_stable() {
 let spec = design_doc_schema_fixture();
 let emitted = emit_rust(&spec);
 let size = emitted.source.len();
 assert!(
 size > 500 && size < 4000,
 "small fixture emit size: {} bytes (expected 500-4000 range)",
 size
 );
 }

 // ─── 5-language emit tests (Round 52, OPTION B-1) ──────────────────────

 /// sha256_hex deterministic — identical input → byte-identical 64-char hex digest.
 #[test]
 fn sha256_hex_deterministic() {
 let a = sha256_hex("hello");
 let b = sha256_hex("hello");
 assert_eq!(a, b);
 assert_eq!(a.len(), 64);
 // Known SHA-256 of "hello"
 assert_eq!(a, "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824");
 }

 /// Kotlin emit deterministic + Section/ChangelogEntry/FrozenList/CrossRef emit.
 #[test]
 fn emit_kotlin_covers_all_kinds() {
 let spec = design_doc_schema_fixture();
 let a = emit_kotlin(&spec);
 let b = emit_kotlin(&spec);
 assert_eq!(a, b);
 assert!(a.contains("data class Section("));
 assert!(a.contains("data class ChangelogEntry("));
 assert!(a.contains("data class FrozenList("));
 assert!(a.contains("data class CrossRef("));
 }

 /// Python emit is deterministic — emits 4 entity/relation @dataclass blocks.
 #[test]
 fn emit_python_covers_all_kinds() {
 let spec = design_doc_schema_fixture();
 let a = emit_python(&spec);
 let b = emit_python(&spec);
 assert_eq!(a, b);
 assert!(a.contains("class Section:"));
 assert!(a.contains("class ChangelogEntry:"));
 assert!(a.contains("class FrozenList:"));
 assert!(a.contains("class CrossRef:"));
 assert!(a.contains("@dataclass"));
 }

 /// C++ emit deterministic + 4 struct emit + namespace.
 #[test]
 fn emit_cpp_covers_all_kinds() {
 let spec = design_doc_schema_fixture();
 let a = emit_cpp(&spec);
 let b = emit_cpp(&spec);
 assert_eq!(a, b);
 assert!(a.contains("namespace mnemosyne::generated"));
 assert!(a.contains("struct Section {"));
 assert!(a.contains("struct ChangelogEntry {"));
 assert!(a.contains("struct FrozenList {"));
 assert!(a.contains("struct CrossRef {"));
 }

 /// Protobuf emit is deterministic — emits 4 messages with tag indices.
 #[test]
 fn emit_protobuf_covers_all_kinds() {
 let spec = design_doc_schema_fixture();
 let a = emit_protobuf(&spec);
 let b = emit_protobuf(&spec);
 assert_eq!(a, b);
 assert!(a.contains("message Section {"));
 assert!(a.contains("message ChangelogEntry {"));
 assert!(a.contains("message FrozenList {"));
 assert!(a.contains("message CrossRef {"));
 assert!(a.contains("uint64 branch_id = 1;"));
 assert!(a.contains("uint64 entity_id = 2;"));
 }

 /// emit_all_languages — all 5 emits are deterministic.
 #[test]
 fn emit_all_languages_deterministic() {
 let spec = design_doc_schema_fixture();
 let a = emit_all_languages(&spec);
 let b = emit_all_languages(&spec);
 assert_eq!(a, b);
 }

 /// All 5 language sha256 hex digests are stable cross-process.
 #[test]
 fn cross_language_sha256_stable() {
 let spec = design_doc_schema_fixture();
 let m = emit_all_languages(&spec);
 // each emit's sha256 - stable — DefaultHasher RandomState threshold break validation.
 let h_rust = sha256_hex(&m.rust);
 let h_kotlin = sha256_hex(&m.kotlin);
 let h_python = sha256_hex(&m.python);
 let h_cpp = sha256_hex(&m.cpp);
 let h_proto = sha256_hex(&m.protobuf);
 // 5 hex digest all 64-char + distinct (from as other source code in distinct hash).
 for h in [&h_rust, &h_kotlin, &h_python, &h_cpp, &h_proto] {
 assert_eq!(h.len(), 64);
 }
 let mut all = std::collections::BTreeSet::new();
 all.insert(h_rust);
 all.insert(h_kotlin);
 all.insert(h_python);
 all.insert(h_cpp);
 all.insert(h_proto);
 assert_eq!(all.len(), 5, "5 distinct language emit → 5 distinct sha256");
 }

 /// Cross-language Jaccard inclusion = 1.0 — all canonical identifiers covered by all 5 emits.
 /// ROADMAP *Cross-language conformance fixture*'s Jaccard = 1.0 consistency validation.
 #[test]
 fn cross_language_jaccard_inclusion_one() {
 let spec = design_doc_schema_fixture();
 let canonical = canonical_identifier_set(&spec);
 let m = emit_all_languages(&spec);
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
  "{lang} emit Jaccard inclusion = {score} (expected 1.0)"
 );
 }
 }

 /// canonical_identifier_set — validates that every entity / relation / field is registered.
 #[test]
 fn canonical_identifier_set_covers_design_doc() {
 let spec = design_doc_schema_fixture();
 let s = canonical_identifier_set(&spec);
 // 4 kind names
 assert!(s.contains("Section"));
 assert!(s.contains("ChangelogEntry"));
 assert!(s.contains("FrozenList"));
 assert!(s.contains("CrossRef"));
 // composite key fields (branch_id / entity_id / valid_from)
 assert!(s.contains("branch_id"));
 assert!(s.contains("entity_id"));
 assert!(s.contains("valid_from"));
 // sample entity fields
 assert!(s.contains("doc_path"));
 assert!(s.contains("decision_status"));
 }
}
