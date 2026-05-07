//! Cascade query spec types — DESIGN §43 *input*'s `<query>` block carry.
//!
//! Spec types describe cascade queries declaratively. The actual Salsa runtime
//! lives in [`crate::runtime`]; this module captures the query topology used
//! by both the runtime and the metadata layer (5-language read-only emit).

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadDep {
 pub entity: String,
 pub field: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriggerSpec {
 pub event: String,
 pub filter: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CascadeQuerySpec {
 pub name: String,
 pub reads: Vec<ReadDep>,
 pub output: String,
 pub triggers: Vec<TriggerSpec>,
 /// CascadeOrdering axis — §39 inter-kind dependency, §47 default `global_fifo`.
 pub ordering: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CascadeWireSpec {
 pub queries: Vec<CascadeQuerySpec>,
}

/// §66 design_doc cascade fixture — `section_decision_status` and
/// `frozen_list_membership`. Mirror of bench prototype `design_doc_cascade_fixture`.
pub fn design_doc_cascade_fixture() -> CascadeWireSpec {
 CascadeWireSpec {
 queries: vec![
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

#[cfg(test)]
mod tests {
 use super::*;

 #[test]
 fn fixture_has_two_queries() {
 let spec = design_doc_cascade_fixture();
 assert_eq!(spec.queries.len(), 2);
 assert_eq!(spec.queries[0].name, "section_decision_status");
 assert_eq!(spec.queries[1].name, "frozen_list_membership");
 }

 #[test]
 fn fixture_orderings_default_to_global_fifo() {
 let spec = design_doc_cascade_fixture();
 for q in &spec.queries {
 assert_eq!(q.ordering, "global_fifo");
 }
 }
}
