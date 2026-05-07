//! §66 prerequisite #4 — T1 cross-ref validator + audit append-only prototype (Round 68, OPTION A).
//!
//! DESIGN.md §66 *Phase 0 Validator (T1 standalone behavior, 4 rule)* body source of truth carry:
//! 1. **CrossRef orphan reject** — to_target missing on reject
//! 2. **ChangelogEntry append-only** — existing entry body mutate on reject
//! 3. **FrozenList membership delta** — when members change, a new ChangelogEntry must be attached
//! 4. **Section decision_status transition** — active → superseded on superseding section cross-ref enforced
//!
//! Prototype role:
//! 7th module of the bench/codegen-prototype crate (entity_indexer / cf_wrapper /
//! salsa_wire / closure_runtime / markdown_import / markdown_export / **t1_validator**)
//! - single-doc validator (rule 1) + diff validator (rule 2/3/4) separation
//! - small-fixture validation feasibility source — Phase 0 implementation time
//! Reference data layer for the mnemosyne-validator production crate (Round 41 entity-relation
//! Graph-indexer prototype — §39 entity_indexer's reference-data-layer pattern equivalent.

use crate::markdown_import::{
 ChangelogEntry, DecisionStatus, FrozenList, ParsedDoc, RefKind, Section,
};
use std::collections::{BTreeMap, BTreeSet};

// ============================================================================
// ValidationError — T1 validator's reject signal typed enum.
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
 /// Rule 1 — CrossRef orphan: to_target is missing from this doc's section_id_set.
 /// (cross-doc CrossRefs are not rejected by this prototype — external-doc scope validation is a separate layer.)
 OrphanCrossRef {
 from_section: String,
 to_target: String,
 ref_kind: RefKind,
 },
 /// Rule 2 — ChangelogEntry append-only violation: existing entry's sub_bullets / id mutate.
 ChangelogMutated {
 entry_id: String,
 prev_sub_bullets: Vec<String>,
 curr_sub_bullets: Vec<String>,
 },
 /// Rule 3 — FrozenList membership delta without new ChangelogEntry attached.
 FrozenListMembershipDelta {
 list_id: String,
 prev_members: Vec<String>,
 curr_members: Vec<String>,
 new_changelog_entry_attached: bool,
 },
 /// Rule 4 — Section decision_status active → superseded with no superseding CrossRef.
 SupersedeMissingRef {
 section_id: String,
 prev_status: DecisionStatus,
 curr_status: DecisionStatus,
 },
}

// ============================================================================
// Rule 1 — cross_ref_orphan_reject (single-doc validator).
// ============================================================================

/// Validates that CrossRef.to_target exists in this doc's section_id_set.
/// cross-doc CrossRefs (RefKind::CrossDoc) — out of scope for validation in this prototype (external-doc scope).
///
/// DESIGN §66 *Phase 0 Validator (T1)* rule 1 source of truth carry.
pub fn cross_ref_orphan_reject(doc: &ParsedDoc) -> Vec<ValidationError> {
 let section_id_set: BTreeSet<&str> = doc
 .sections
 .iter()
 .map(|s| s.section_id.as_str())
 .collect();

 let mut errors = Vec::new();
 for cr in &doc.cross_refs {
 if cr.ref_kind == RefKind::CrossDoc {
 // cross-doc CrossRef — external doc scope, this single-doc validator validation other.
 continue;
 }
 if !section_id_set.contains(cr.to_target.as_str()) {
 errors.push(ValidationError::OrphanCrossRef {
  from_section: cr.from_section.clone(),
  to_target: cr.to_target.clone(),
  ref_kind: cr.ref_kind,
 });
 }
 }
 errors
}

// ============================================================================
// Rule 2 — changelog_entry_append_only (diff validator).
// ============================================================================

/// prev → curr in existing ChangelogEntry's mutate validation (sub_bullets / parent / frozen_at_tt).
/// New ChangelogEntry add (new entry_id) — PASS; existing entry mutate — reject.
///
/// DESIGN §66 *Phase 0 Validator (T1)* rule 2 source of truth carry.
pub fn changelog_entry_append_only(
 prev: &ParsedDoc,
 curr: &ParsedDoc,
) -> Vec<ValidationError> {
 let prev_by_id: BTreeMap<&str, &ChangelogEntry> = prev
 .changelog_entries
 .iter()
 .map(|e| (e.entry_id.as_str(), e))
 .collect();

 let mut errors = Vec::new();
 for curr_entry in &curr.changelog_entries {
 if let Some(prev_entry) = prev_by_id.get(curr_entry.entry_id.as_str()) {
 if prev_entry.sub_bullets != curr_entry.sub_bullets
  || prev_entry.parent_changelog_entry != curr_entry.parent_changelog_entry
 {
  errors.push(ValidationError::ChangelogMutated {
  entry_id: curr_entry.entry_id.clone(),
  prev_sub_bullets: prev_entry.sub_bullets.clone(),
  curr_sub_bullets: curr_entry.sub_bullets.clone(),
  });
 }
 }
 }
 // prev at exists- entry - curr at if absent = removed (mutate). reject.
 let curr_id_set: BTreeSet<&str> = curr
 .changelog_entries
 .iter()
 .map(|e| e.entry_id.as_str())
 .collect();
 for prev_entry in &prev.changelog_entries {
 if !curr_id_set.contains(prev_entry.entry_id.as_str()) {
 errors.push(ValidationError::ChangelogMutated {
  entry_id: prev_entry.entry_id.clone(),
  prev_sub_bullets: prev_entry.sub_bullets.clone(),
  curr_sub_bullets: Vec::new(),
 });
 }
 }
 errors
}

// ============================================================================
// Rule 3 — frozen_list_membership_delta (diff validator).
// ============================================================================

/// Validates that FrozenList.members changes attach a new ChangelogEntry.
/// this prototype's *attached* definition: PASS iff every prev.changelog_entries entry_id is also present in curr.
/// Change detection — same list_id + member-set comparison.
///
/// DESIGN §66 *Phase 0 Validator (T1)* rule 3 source of truth carry.
pub fn frozen_list_membership_delta(
 prev: &ParsedDoc,
 curr: &ParsedDoc,
) -> Vec<ValidationError> {
 let prev_by_id: BTreeMap<&str, &FrozenList> = prev
 .frozen_lists
 .iter()
 .map(|f| (f.list_id.as_str(), f))
 .collect();

 let prev_changelog_ids: BTreeSet<&str> = prev
 .changelog_entries
 .iter()
 .map(|e| e.entry_id.as_str())
 .collect();
 let curr_changelog_ids: BTreeSet<&str> = curr
 .changelog_entries
 .iter()
 .map(|e| e.entry_id.as_str())
 .collect();
 let new_changelog_attached = !curr_changelog_ids
 .difference(&prev_changelog_ids)
 .next()
 .is_none();

 let mut errors = Vec::new();
 for curr_list in &curr.frozen_lists {
 if let Some(prev_list) = prev_by_id.get(curr_list.list_id.as_str()) {
 if prev_list.members != curr_list.members && !new_changelog_attached {
  errors.push(ValidationError::FrozenListMembershipDelta {
  list_id: curr_list.list_id.clone(),
  prev_members: prev_list.members.clone(),
  curr_members: curr_list.members.clone(),
  new_changelog_entry_attached: false,
  });
 }
 }
 }
 errors
}

// ============================================================================
// Rule 4 — section_decision_status_transition (diff validator).
// ============================================================================

/// Validates that an active → superseded transition on Section.decision_status requires a superseding-section CrossRef.
/// PASS iff curr.cross_refs has an entry with from_section = (this section_id from prev) AND ref_kind = decision/impl that resolves.
///
/// DESIGN §66 *Phase 0 Validator (T1)* rule 4 source of truth carry.
pub fn section_decision_status_transition(
 prev: &ParsedDoc,
 curr: &ParsedDoc,
) -> Vec<ValidationError> {
 let prev_by_id: BTreeMap<&str, &Section> = prev
 .sections
 .iter()
 .map(|s| (s.section_id.as_str(), s))
 .collect();

 let mut errors = Vec::new();
 for curr_section in &curr.sections {
 let prev_section = match prev_by_id.get(curr_section.section_id.as_str()) {
 Some(s) => s,
 None => continue,
 };
 if prev_section.decision_status == DecisionStatus::Active
 && curr_section.decision_status == DecisionStatus::Superseded
 {
 // superseding CrossRef = from this section to another, decision/impl kind.
 let has_superseding_ref = curr.cross_refs.iter().any(|cr| {
  cr.from_section == curr_section.section_id
  && (cr.ref_kind == RefKind::Decision || cr.ref_kind == RefKind::Impl)
 });
 if !has_superseding_ref {
  errors.push(ValidationError::SupersedeMissingRef {
  section_id: curr_section.section_id.clone(),
  prev_status: prev_section.decision_status,
  curr_status: curr_section.decision_status,
  });
 }
 }
 }
 errors
}

// ============================================================================
// Tests — small fixture + diff fixture validation.
// ============================================================================

#[cfg(test)]
mod tests {
 use super::*;
 use crate::markdown_import::{ChangelogEntry, CrossRef, FrozenList, LockKind, Section};

 fn make_doc(
 sections: Vec<Section>,
 changelog: Vec<ChangelogEntry>,
 frozen: Vec<FrozenList>,
 cross: Vec<CrossRef>,
 ) -> ParsedDoc {
 ParsedDoc {
 sections,
 changelog_entries: changelog,
 frozen_lists: frozen,
 cross_refs: cross,
 warnings: Vec::new(),
 bodies: std::collections::BTreeMap::new(),
 line_anchors: std::collections::BTreeMap::new(),
 }
 }

 fn sec(id: &str, parent: Option<&str>, title: &str, status: DecisionStatus) -> Section {
 Section {
 section_id: id.to_string(),
 parent_doc: "test.md".to_string(),
 parent_section: parent.map(String::from),
 title: title.to_string(),
 decision_status: status,
 }
 }

 // ── Rule 1: cross_ref_orphan_reject ──────────────────────────────────

 #[test]
 fn rule1_orphan_reject_detects_missing_target() {
 let doc = make_doc(
 vec![sec("1", None, "S1", DecisionStatus::Active)],
 vec![],
 vec![],
 vec![CrossRef {
  from_section: "1".to_string(),
  to_target: "99".to_string(), // orphan — no §99 section
  ref_kind: RefKind::Decision,
  created_at_changelog_entry: None,
 }],
 );
 let errors = cross_ref_orphan_reject(&doc);
 assert_eq!(errors.len(), 1);
 match &errors[0] {
 ValidationError::OrphanCrossRef { to_target, .. } => {
  assert_eq!(to_target, "99");
 }
 _ => panic!("expected OrphanCrossRef"),
 }
 }

 #[test]
 fn rule1_orphan_reject_passes_valid_ref() {
 let doc = make_doc(
 vec![
  sec("1", None, "S1", DecisionStatus::Active),
  sec("2", None, "S2", DecisionStatus::Active),
 ],
 vec![],
 vec![],
 vec![CrossRef {
  from_section: "1".to_string(),
  to_target: "2".to_string(),
  ref_kind: RefKind::Decision,
  created_at_changelog_entry: None,
 }],
 );
 let errors = cross_ref_orphan_reject(&doc);
 assert!(errors.is_empty());
 }

 #[test]
 fn rule1_cross_doc_ref_skipped() {
 // CrossDoc kind — external doc scope, this single-doc validator validation other → PASS.
 let doc = make_doc(
 vec![sec("1", None, "S1", DecisionStatus::Active)],
 vec![],
 vec![],
 vec![CrossRef {
  from_section: "1".to_string(),
  to_target: "ARCHITECTURE.md#l1".to_string(),
  ref_kind: RefKind::CrossDoc,
  created_at_changelog_entry: None,
 }],
 );
 let errors = cross_ref_orphan_reject(&doc);
 assert!(errors.is_empty(), "cross-doc must skip orphan check");
 }

 // ── Rule 2: changelog_entry_append_only ──────────────────────────────

 #[test]
 fn rule2_append_only_detects_mutation() {
 let prev = make_doc(
 vec![],
 vec![ChangelogEntry {
  entry_id: "Round 1".to_string(),
  parent_changelog_entry: None,
  sub_bullets: vec!["original sub-bullet".to_string()],
  frozen_at_transaction_time: 1,
 }],
 vec![],
 vec![],
 );
 let curr = make_doc(
 vec![],
 vec![ChangelogEntry {
  entry_id: "Round 1".to_string(),
  parent_changelog_entry: None,
  sub_bullets: vec!["MUTATED sub-bullet".to_string()], // mutated
  frozen_at_transaction_time: 1,
 }],
 vec![],
 vec![],
 );
 let errors = changelog_entry_append_only(&prev, &curr);
 assert_eq!(errors.len(), 1);
 match &errors[0] {
 ValidationError::ChangelogMutated { entry_id, .. } => {
  assert_eq!(entry_id, "Round 1");
 }
 _ => panic!("expected ChangelogMutated"),
 }
 }

 #[test]
 fn rule2_append_only_passes_appended_new_entry() {
 let prev = make_doc(
 vec![],
 vec![ChangelogEntry {
  entry_id: "Round 1".to_string(),
  parent_changelog_entry: None,
  sub_bullets: vec!["original".to_string()],
  frozen_at_transaction_time: 1,
 }],
 vec![],
 vec![],
 );
 let curr = make_doc(
 vec![],
 vec![
  ChangelogEntry {
  entry_id: "Round 1".to_string(),
  parent_changelog_entry: None,
  sub_bullets: vec!["original".to_string()],
  frozen_at_transaction_time: 1,
  },
  ChangelogEntry {
  entry_id: "Round 2".to_string(), // new entry appended
  parent_changelog_entry: None,
  sub_bullets: vec!["new entry body".to_string()],
  frozen_at_transaction_time: 2,
  },
 ],
 vec![],
 vec![],
 );
 let errors = changelog_entry_append_only(&prev, &curr);
 assert!(errors.is_empty(), "appended new entry must PASS");
 }

 #[test]
 fn rule2_append_only_detects_removal() {
 let prev = make_doc(
 vec![],
 vec![ChangelogEntry {
  entry_id: "Round 1".to_string(),
  parent_changelog_entry: None,
  sub_bullets: vec!["body".to_string()],
  frozen_at_transaction_time: 1,
 }],
 vec![],
 vec![],
 );
 let curr = make_doc(vec![], vec![], vec![], vec![]); // entry removed
 let errors = changelog_entry_append_only(&prev, &curr);
 assert_eq!(errors.len(), 1);
 }

 // ── Rule 3: frozen_list_membership_delta ─────────────────────────────

 #[test]
 fn rule3_membership_delta_detects_change_without_new_entry() {
 let prev = make_doc(
 vec![],
 vec![ChangelogEntry {
  entry_id: "Round 1".to_string(),
  parent_changelog_entry: None,
  sub_bullets: vec![],
  frozen_at_transaction_time: 1,
 }],
 vec![FrozenList {
  list_id: "ten_cf_list".to_string(),
  created_at_changelog_entry: "Round 1".to_string(),
  members: vec!["a".to_string(), "b".to_string()],
  lock_kind: LockKind::DecisionFreeze,
 }],
 vec![],
 );
 let curr = make_doc(
 vec![],
 vec![ChangelogEntry {
  entry_id: "Round 1".to_string(), // no new entry
  parent_changelog_entry: None,
  sub_bullets: vec![],
  frozen_at_transaction_time: 1,
 }],
 vec![FrozenList {
  list_id: "ten_cf_list".to_string(),
  created_at_changelog_entry: "Round 1".to_string(),
  members: vec![
  "a".to_string(),
  "b".to_string(),
  "c".to_string(), // member added
  ],
  lock_kind: LockKind::DecisionFreeze,
 }],
 vec![],
 );
 let errors = frozen_list_membership_delta(&prev, &curr);
 assert_eq!(errors.len(), 1);
 }

 #[test]
 fn rule3_membership_delta_passes_with_new_changelog_entry() {
 let prev = make_doc(
 vec![],
 vec![ChangelogEntry {
  entry_id: "Round 1".to_string(),
  parent_changelog_entry: None,
  sub_bullets: vec![],
  frozen_at_transaction_time: 1,
 }],
 vec![FrozenList {
  list_id: "ten_cf_list".to_string(),
  created_at_changelog_entry: "Round 1".to_string(),
  members: vec!["a".to_string()],
  lock_kind: LockKind::DecisionFreeze,
 }],
 vec![],
 );
 let curr = make_doc(
 vec![],
 vec![
  ChangelogEntry {
  entry_id: "Round 1".to_string(),
  parent_changelog_entry: None,
  sub_bullets: vec![],
  frozen_at_transaction_time: 1,
  },
  ChangelogEntry {
  entry_id: "Round 2".to_string(), // new entry
  parent_changelog_entry: None,
  sub_bullets: vec!["FrozenList delta record".to_string()],
  frozen_at_transaction_time: 2,
  },
 ],
 vec![FrozenList {
  list_id: "ten_cf_list".to_string(),
  created_at_changelog_entry: "Round 1".to_string(),
  members: vec!["a".to_string(), "b".to_string()],
  lock_kind: LockKind::DecisionFreeze,
 }],
 vec![],
 );
 let errors = frozen_list_membership_delta(&prev, &curr);
 assert!(errors.is_empty(), "new ChangelogEntry attached → PASS");
 }

 // ── Rule 4: section_decision_status_transition ───────────────────────

 #[test]
 fn rule4_supersede_detects_missing_ref() {
 let prev = make_doc(
 vec![sec("15", None, "Game runtime SDK", DecisionStatus::Active)],
 vec![],
 vec![],
 vec![],
 );
 let curr = make_doc(
 vec![sec(
  "15",
  None,
  "Game runtime SDK",
  DecisionStatus::Superseded, // active → superseded
 )],
 vec![],
 vec![],
 vec![], // no superseding CrossRef from §15
 );
 let errors = section_decision_status_transition(&prev, &curr);
 assert_eq!(errors.len(), 1);
 match &errors[0] {
 ValidationError::SupersedeMissingRef { section_id, .. } => {
  assert_eq!(section_id, "15");
 }
 _ => panic!("expected SupersedeMissingRef"),
 }
 }

 #[test]
 fn rule4_supersede_passes_with_superseding_ref() {
 let prev = make_doc(
 vec![sec("15", None, "Game runtime", DecisionStatus::Active)],
 vec![],
 vec![],
 vec![],
 );
 let curr = make_doc(
 vec![sec("15", None, "Game runtime", DecisionStatus::Superseded)],
 vec![],
 vec![],
 vec![CrossRef {
  from_section: "15".to_string(),
  to_target: "56".to_string(), // superseded by §56
  ref_kind: RefKind::Decision,
  created_at_changelog_entry: None,
 }],
 );
 let errors = section_decision_status_transition(&prev, &curr);
 assert!(errors.is_empty());
 }

 #[test]
 fn rule4_active_to_active_no_check() {
 let prev = make_doc(
 vec![sec("1", None, "S", DecisionStatus::Active)],
 vec![],
 vec![],
 vec![],
 );
 let curr = make_doc(
 vec![sec("1", None, "S", DecisionStatus::Active)],
 vec![],
 vec![],
 vec![],
 );
 let errors = section_decision_status_transition(&prev, &curr);
 assert!(errors.is_empty());
 }
}
