//! 3-tier gate (T1/T2/T3) — routing matrix carry.
//!
//! - **Tier 1** (semantic): cross-ref orphan reject + append-only enforcement +
//! FrozenList membership-delta + supersede ref enforcement. Routed to
//! `mnemosyne-validate` rules.
//! - **Tier 2** (structural): family tree + resource conservation. Phase 0 stub.
//! stub — extends in Phase 1.
//! - **Tier 3** (convention): warn-only. Phase 0 stub — extends in Phase 1.
//!
//! Tier 1 rejects cause an audit record with `gate_routing_reason="t1_reject"`.
//! Tier 2 rejects cause `t2_reject`. Tier 3 emits warnings without rejecting.

use crate::error::GateTierLabel;
use crate::proposal::{Proposal, ProposalKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateTier {
 Tier1,
 Tier2,
 Tier3,
}

impl GateTier {
 pub fn label(self) -> GateTierLabel {
 match self {
 GateTier::Tier1 => GateTierLabel::Tier1,
 GateTier::Tier2 => GateTierLabel::Tier2,
 GateTier::Tier3 => GateTierLabel::Tier3,
 }
 }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateOutcome {
 Accept {
 warnings: Vec<String>,
 },
 Reject {
 tier: GateTier,
 reason: String,
 },
}

pub trait Tier1Gate {
 fn evaluate(&self, proposal: &Proposal) -> GateOutcome;
}

pub trait Tier2Gate {
 fn evaluate(&self, proposal: &Proposal) -> GateOutcome;
}

pub trait Tier3Gate {
 fn evaluate(&self, proposal: &Proposal) -> Vec<String>;
}

/// Default Tier 1 gate — minimum Phase 0 safety enforcement.
///
/// Phase 0 scope:
/// - `CrossRefCreate`: reject if `from == 0` or `to == 0` (placeholder for
/// full target-resolution; production wires this through workspace lookup).
/// - `ChangelogAppend`: reject if entity_id is reserved (entity_id == 0).
/// - `FrozenListMembershipChange`: reject if `attached_changelog_entry_id == 0`
/// (membership-delta requires a new ChangelogEntry attachment).
/// - others: accept (entity create/update need Tier 2 structural checks).
#[derive(Debug, Clone, Default)]
pub struct DefaultTier1;

impl Tier1Gate for DefaultTier1 {
 fn evaluate(&self, proposal: &Proposal) -> GateOutcome {
 match &proposal.kind {
 ProposalKind::CrossRefCreate {
  from_section,
  to_section,
  ref_kind,
  ..
 } => {
  if *from_section == 0 {
  return GateOutcome::Reject {
  tier: GateTier::Tier1,
  reason: "cross-ref orphan: from_section unresolved (== 0)".to_string(),
  };
  }
  if *to_section == 0 {
  return GateOutcome::Reject {
  tier: GateTier::Tier1,
  reason: "cross-ref orphan: to_section unresolved (== 0)".to_string(),
  };
  }
  if ref_kind.is_empty() {
  return GateOutcome::Reject {
  tier: GateTier::Tier1,
  reason: "cross-ref ref_kind empty".to_string(),
  };
  }
  GateOutcome::Accept { warnings: vec![] }
 }
 ProposalKind::ChangelogAppend { entity_id, .. } => {
  if *entity_id == 0 {
  return GateOutcome::Reject {
  tier: GateTier::Tier1,
  reason: "changelog entry_id reserved (== 0)".to_string(),
  };
  }
  GateOutcome::Accept { warnings: vec![] }
 }
 ProposalKind::FrozenListMembershipChange {
  attached_changelog_entry_id,
  ..
 } => {
  if *attached_changelog_entry_id == 0 {
  return GateOutcome::Reject {
  tier: GateTier::Tier1,
  reason: "frozen-list membership delta missing attached ChangelogEntry"
   .to_string(),
  };
  }
  GateOutcome::Accept { warnings: vec![] }
 }
 ProposalKind::EntityCreate { .. } | ProposalKind::EntityUpdate { .. } => {
  GateOutcome::Accept { warnings: vec![] }
 }
 }
 }
}

/// Default Tier 2 gate — Phase 0 stub returning `Accept`. Phase 1 expands
/// with structural checks (family tree, resource conservation).
#[derive(Debug, Clone, Default)]
pub struct DefaultTier2;

impl Tier2Gate for DefaultTier2 {
 fn evaluate(&self, _proposal: &Proposal) -> GateOutcome {
 GateOutcome::Accept { warnings: vec![] }
 }
}

/// Default Tier 3 gate — Phase 0 stub returning empty warnings. Phase 1
/// expands with convention checks (naming style, comment density).
#[derive(Debug, Clone, Default)]
pub struct DefaultTier3;

impl Tier3Gate for DefaultTier3 {
 fn evaluate(&self, _proposal: &Proposal) -> Vec<String> {
 vec![]
 }
}

#[cfg(test)]
mod tests {
 use super::*;

 fn cross_ref(from: u64, to: u64, kind: &str) -> Proposal {
 Proposal {
 proposal_id: "p-test".to_string(),
 actor: "tester".to_string(),
 kind: ProposalKind::CrossRefCreate {
  branch_id: 1,
  from_section: from,
  to_section: to,
  ref_kind: kind.to_string(),
 },
 }
 }

 #[test]
 fn tier1_rejects_cross_ref_with_zero_from() {
 let r = DefaultTier1.evaluate(&cross_ref(0, 39, "decision"));
 match r {
 GateOutcome::Reject { tier, reason } => {
  assert_eq!(tier, GateTier::Tier1);
  assert!(reason.contains("from_section unresolved"));
 }
 _ => panic!("expected reject"),
 }
 }

 #[test]
 fn tier1_rejects_cross_ref_with_zero_to() {
 let r = DefaultTier1.evaluate(&cross_ref(66, 0, "decision"));
 assert!(matches!(r, GateOutcome::Reject { tier: GateTier::Tier1, .. }));
 }

 #[test]
 fn tier1_rejects_cross_ref_with_empty_ref_kind() {
 let r = DefaultTier1.evaluate(&cross_ref(66, 39, ""));
 assert!(matches!(r, GateOutcome::Reject { tier: GateTier::Tier1, .. }));
 }

 #[test]
 fn tier1_accepts_valid_cross_ref() {
 let r = DefaultTier1.evaluate(&cross_ref(66, 39, "decision"));
 assert!(matches!(r, GateOutcome::Accept { .. }));
 }

 #[test]
 fn tier1_rejects_changelog_with_zero_entity_id() {
 let p = Proposal {
 proposal_id: "p".to_string(),
 actor: "t".to_string(),
 kind: ProposalKind::ChangelogAppend {
  branch_id: 1,
  entity_id: 0,
  valid_from: 100,
  payload: vec![],
 },
 };
 let r = DefaultTier1.evaluate(&p);
 assert!(matches!(r, GateOutcome::Reject { tier: GateTier::Tier1, .. }));
 }

 #[test]
 fn tier1_rejects_membership_delta_without_changelog_attachment() {
 let p = Proposal {
 proposal_id: "p".to_string(),
 actor: "t".to_string(),
 kind: ProposalKind::FrozenListMembershipChange {
  branch_id: 1,
  list_id: 100,
  valid_from: 100,
  attached_changelog_entry_id: 0,
  payload: vec![],
 },
 };
 let r = DefaultTier1.evaluate(&p);
 match r {
 GateOutcome::Reject { tier, reason } => {
  assert_eq!(tier, GateTier::Tier1);
  assert!(reason.contains("missing attached ChangelogEntry"));
 }
 _ => panic!("expected reject"),
 }
 }

 #[test]
 fn tier1_accepts_valid_membership_delta() {
 let p = Proposal {
 proposal_id: "p".to_string(),
 actor: "t".to_string(),
 kind: ProposalKind::FrozenListMembershipChange {
  branch_id: 1,
  list_id: 100,
  valid_from: 100,
  attached_changelog_entry_id: 73,
  payload: vec![1, 2, 3],
 },
 };
 let r = DefaultTier1.evaluate(&p);
 assert!(matches!(r, GateOutcome::Accept { .. }));
 }

 #[test]
 fn tier2_phase0_stub_accepts_all() {
 let p = cross_ref(66, 39, "decision");
 let r = DefaultTier2.evaluate(&p);
 assert!(matches!(r, GateOutcome::Accept { .. }));
 }

 #[test]
 fn tier3_phase0_stub_no_warnings() {
 let p = cross_ref(66, 39, "decision");
 let warnings = DefaultTier3.evaluate(&p);
 assert!(warnings.is_empty());
 }
}
