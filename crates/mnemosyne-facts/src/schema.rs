//! Schema description types — graph_schema's meta-level shape.
//!
//! `GraphSpec` carries `EntityDef` / `RelationDef` / `FieldDef` / `FieldType` /
//! `Persistence` enums. Single canonical structural representation; the 5-language
//! emit modules (`emit::rust` / `emit::kotlin` /...) consume this and produce the target
//! source.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum FieldType {
    /// `u64 BE` 8-byte composite-key component.
    U64BigEndian,
    /// UTF-8 string.
    String,
    /// Variable-length byte array.
    Bytes,
    /// Foreign-entity reference (u64 BE wire form; target-name carried for codegen).
    EntityRef { target: String },
    /// Row-per-(asset, fact) normalized refs.
    NormalizedAssetRefs,
    /// Row-per-encounter (meta-agent measurement).
    RowPerEncounter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Persistence {
    /// `@persistent` — explicit CF.
    Persistent,
    /// `@derived` — derived CF (epistemic split, closure separation).
    Derived,
}

/// Composite key spec. Phase -1A stage 2C decision: 24 B fixed-width
/// `branch_id (u64 BE) || entity_id (u64 BE) || valid_from (u64 BE)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompositeKey {
    pub branch_field: String,
    pub entity_field: String,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldDef {
    pub name: String,
    pub ty: FieldType,
    pub nullable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityDef {
    pub name: String,
    pub fields: Vec<FieldDef>,
    pub key: CompositeKey,
    pub persistence: Persistence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationDef {
    pub name: String,
    pub from_entity: String,
    pub to_entity: String,
    pub fields: Vec<FieldDef>,
    pub persistence: Persistence,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphSpec {
    pub entities: Vec<EntityDef>,
    pub relations: Vec<RelationDef>,
}
