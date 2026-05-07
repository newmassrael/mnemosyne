//! Proposal request/response types.
//!
//! `Proposal` carries a single mutation intent. The handler runs it through the
//! 3-tier gate, appends an audit record on accept, and either commits the
//! storage write or returns a typed rejection.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProposalKind {
 /// Create a new entity instance.
 EntityCreate {
 entity_type: String,
 branch_id: u64,
 entity_id: u64,
 valid_from: u64,
 payload: Vec<u8>,
 },
 /// Update existing entity (new valid_from).
 EntityUpdate {
 entity_type: String,
 branch_id: u64,
 entity_id: u64,
 valid_from: u64,
 payload: Vec<u8>,
 },
 /// Append a ChangelogEntry — must be append-only (Tier 1 rule 2 enforced).
 ChangelogAppend {
 branch_id: u64,
 entity_id: u64,
 valid_from: u64,
 payload: Vec<u8>,
 },
 /// Create a CrossRef relation — must resolve to an existing target
 /// (Tier 1 rule 1 cross-ref orphan reject enforced).
 CrossRefCreate {
 branch_id: u64,
 from_section: u64,
 to_section: u64,
 ref_kind: String,
 },
 /// Change FrozenList membership — requires a new ChangelogEntry attachment
 /// (Tier 1 rule 3 membership-delta enforced).
 FrozenListMembershipChange {
 branch_id: u64,
 list_id: u64,
 valid_from: u64,
 attached_changelog_entry_id: u64,
 payload: Vec<u8>,
 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Proposal {
 /// Caller-supplied identifier (idempotency / audit trace).
 pub proposal_id: String,
 pub actor: String,
 pub kind: ProposalKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposalResult {
 pub proposal_id: String,
 pub accepted: bool,
 pub audit_transaction_id: Option<u64>,
 pub rejection_reason: Option<String>,
}

#[cfg(test)]
mod tests {
 use super::*;

 #[test]
 fn proposal_serde_round_trip() {
 let p = Proposal {
 proposal_id: "p-001".to_string(),
 actor: "tester".to_string(),
 kind: ProposalKind::EntityCreate {
  entity_type: "Section".to_string(),
  branch_id: 1,
  entity_id: 42,
  valid_from: 1000,
  payload: b"payload".to_vec(),
 },
 };
 let json = serde_json::to_string(&p).unwrap();
 let back: Proposal = serde_json::from_str(&json).unwrap();
 assert_eq!(p, back);
 }

 #[test]
 fn cross_ref_proposal_carries_targets() {
 let p = Proposal {
 proposal_id: "p-002".to_string(),
 actor: "tester".to_string(),
 kind: ProposalKind::CrossRefCreate {
  branch_id: 1,
  from_section: 66,
  to_section: 39,
  ref_kind: "decision".to_string(),
 },
 };
 if let ProposalKind::CrossRefCreate { from_section, to_section, .. } = &p.kind {
 assert_eq!(*from_section, 66);
 assert_eq!(*to_section, 39);
 } else {
 panic!("expected CrossRefCreate");
 }
 }
}
