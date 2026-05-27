//! design_doc closed-form schema fixture — Section / ChangelogEntry /
//! FrozenList / CrossRef. Single canonical instance used by tests and downstream
//! crates (mnemosyne-cascade for cascade-query input shape).

use crate::schema::{
 CompositeKey, EntityDef, FieldDef, FieldType, GraphSpec, Persistence, RelationDef,
};

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

#[cfg(test)]
mod tests {
 use super::*;

 #[test]
 fn fixture_has_three_entities_and_one_relation() {
 let spec = design_doc_schema_fixture();
 assert_eq!(spec.entities.len(), 3);
 assert_eq!(spec.relations.len(), 1);
 }

 #[test]
 fn fixture_entity_names_match_design_md() {
 let spec = design_doc_schema_fixture();
 let names: Vec<&str> = spec.entities.iter().map(|e| e.name.as_str()).collect();
 assert_eq!(names, vec!["Section", "ChangelogEntry", "FrozenList"]);
 assert_eq!(spec.relations[0].name, "CrossRef");
 }
}
