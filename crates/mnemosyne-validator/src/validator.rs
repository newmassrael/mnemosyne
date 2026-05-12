//! T1 validator — *Phase 0 Validator (T1 standalone behavior, 4 rule)* source binding.
//!
//! 4 rules:
//! 1. **CrossRef orphan reject** — `to_target` missing reject (rule 1)
//! 2. **ChangelogEntry append-only** — existing entry body mutate reject (rule 2)
//! 3. **FrozenList membership delta** — member changes require a new ChangelogEntry attachment (rule 3)
//! 4. **Section decision_status transition** — active → superseded on superseding cross-ref enforced (rule 4)
//!
//! OPTION H-2 adoption carry — rule 1 step (2) lookup:
//! single-doc orphan check failure on [`crate::workspace::Workspace::default_doc_has_section`]
//! reclassify-possibility fallback check. If both are missing, step (3) rejects as orphan.

use crate::schema::{ChangelogEntry, DecisionStatus, FrozenList, ParsedDoc, RefKind, Section};
use crate::workspace::Workspace;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
 /// Rule 1 — CrossRef orphan: to_target is missing from this doc's section_id_set.
 OrphanCrossRef {
 from_section: String,
 to_target: String,
 ref_kind: RefKind,
 },
 /// Rule 2 — ChangelogEntry append-only violation.
 ChangelogMutated {
 entry_id: String,
 prev_sub_bullets: Vec<String>,
 curr_sub_bullets: Vec<String>,
 },
 /// Rule 3 — FrozenList membership delta without new ChangelogEntry attachment.
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
// Rule 1 — cross_ref_orphan_reject (single-doc fallback to step (2)).
// ============================================================================

/// Single-doc rule 1 — bench prototype carry.
///
/// Validates that CrossRef.to_target exists in this doc's section_id_set.
/// cross-doc CrossRefs (RefKind::CrossDoc) are out of scope for this single-doc validation (external-doc).
pub fn cross_ref_orphan_reject(doc: &ParsedDoc) -> Vec<ValidationError> {
 let section_id_set: BTreeSet<&str> = doc
 .sections
 .iter()
 .map(|s| s.section_id.as_str())
 .collect();

 let mut errors = Vec::new();
 for cr in &doc.cross_refs {
 if cr.ref_kind == RefKind::CrossDoc {
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

/// Workspace-aware rule 1 — OPTION H-2 adoption lookup priority 3 step
/// + atomic-first step (2.5).
///
/// step (1) intra-doc → step (2) workspace.default_doc fallback →
/// step (2.5) atomic store fallback →
/// step (3) reject.
///
/// step (2) PASS one cross_ref reject not done — workspace.reclassify_cross_refs -
/// In a subsequent pass, ref_kind is reclassified to CrossDoc, preserving round-trip equivalence.
/// step (2.5) — atomic store as the sole source of truth when markdown re-parse
/// validates a missing `to_target` directly against the atomic store.
pub fn cross_ref_orphan_reject_with_workspace(
 doc: &ParsedDoc,
 workspace: &Workspace,
) -> Vec<ValidationError> {
 let section_id_set: BTreeSet<&str> = doc
 .sections
 .iter()
 .map(|s| s.section_id.as_str())
 .collect();
 // Last-segment alias set: a section_id like "2/2.1" is also resolvable
 // by its trailing "2.1" segment. Lets authors write `` for nested
 // numbered sections without spelling out the full parent path.
 let last_segment_set: BTreeSet<&str> = doc
 .sections
 .iter()
 .filter_map(|s| s.section_id.rsplit_once('/').map(|(_, last)| last))
 .collect();

 let mut errors = Vec::new();
 for cr in &doc.cross_refs {
 if cr.ref_kind == RefKind::CrossDoc {
 continue;
 }
 // Step (1): intra-doc priority.
 if section_id_set.contains(cr.to_target.as_str()) {
 continue;
 }
 // Step (1.5): intra-doc last-segment alias for nested numbered sections.
 if last_segment_set.contains(cr.to_target.as_str()) {
 continue;
 }
 // Step (2): workspace default_doc fallback.
 if workspace.default_doc_has_section(&cr.to_target) {
 continue;
 }
 // Step (2.5): atomic store fallback.
 if workspace.atomic_has_section(&cr.to_target) {
 continue;
 }
 // Step (3): three places all missing — reject.
 errors.push(ValidationError::OrphanCrossRef {
 from_section: cr.from_section.clone(),
 to_target: cr.to_target.clone(),
 ref_kind: cr.ref_kind,
 });
 }
 errors
}

// ============================================================================
// Rule 2 — changelog_entry_append_only.
// ============================================================================

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
// Rule 3 — frozen_list_membership_delta.
// ============================================================================

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
 let new_changelog_attached = curr_changelog_ids
 .difference(&prev_changelog_ids)
 .next()
 .is_some();

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
// Rule 4 — section_decision_status_transition.
// ============================================================================

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

#[cfg(test)]
mod tests {
 use super::*;
 use crate::schema::{ChangelogEntry, CrossRef, FrozenList, LockKind, Section};

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
 ..Default::default()
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

 // ── Rule 1: cross_ref_orphan_reject (single-doc) ─────────────────────

 #[test]
 fn rule1_orphan_reject_detects_missing_target() {
 let doc = make_doc(
 vec![sec("1", None, "S1", DecisionStatus::Active)],
 vec![],
 vec![],
 vec![CrossRef {
  from_section: "1".to_string(),
  to_target: "99".to_string(),
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
 assert!(errors.is_empty());
 }

 // ── Rule 1 step (2) — workspace-aware lookup priority ─

 #[test]
 fn rule1_step_2_default_doc_fallback_passes() {
 // self doc missing + workspace default_doc in exists → step (2) PASS.
 let mut ws = Workspace::mnemosyne();
 let design = make_doc(
 vec![sec("39", None, "Graph schema", DecisionStatus::Active)],
 vec![],
 vec![],
 vec![],
 );
 ws.insert("docs/GENERATED.md", design);

 let other_doc = make_doc(
 vec![sec("l1", None, "Layer 1", DecisionStatus::Active)],
 vec![],
 vec![],
 vec![CrossRef {
  from_section: "l1".to_string(),
  to_target: "39".to_string(),
  ref_kind: RefKind::Decision,
  created_at_changelog_entry: None,
 }],
 );

 let errors = cross_ref_orphan_reject_with_workspace(&other_doc, &ws);
 assert!(errors.is_empty(), "step (2) default-doc fallback must PASS");
 }

 #[test]
 fn rule1_step_3_both_missing_orphan_reject() {
 // self doc + default_doc all missing → step (3) reject.
 let mut ws = Workspace::mnemosyne();
 let design = make_doc(
 vec![sec("39", None, "Graph schema", DecisionStatus::Active)],
 vec![],
 vec![],
 vec![],
 );
 ws.insert("docs/GENERATED.md", design);

 let other_doc = make_doc(
 vec![sec("l1", None, "Layer 1", DecisionStatus::Active)],
 vec![],
 vec![],
 vec![CrossRef {
  from_section: "l1".to_string(),
  to_target: "999".to_string(),
  ref_kind: RefKind::Decision,
  created_at_changelog_entry: None,
 }],
 );

 let errors = cross_ref_orphan_reject_with_workspace(&other_doc, &ws);
 assert_eq!(errors.len(), 1);
 }

 #[test]
 fn rule1_step_1_intra_doc_priority_over_default_doc() {
 // self doc + default_doc all exists → step (1) intra-doc priority PASS.
 let mut ws = Workspace::mnemosyne();
 let design = make_doc(
 vec![sec("39", None, "Graph schema", DecisionStatus::Active)],
 vec![],
 vec![],
 vec![],
 );
 ws.insert("docs/GENERATED.md", design);

 let same_id_doc = make_doc(
 vec![
  sec("39", None, "Local §39", DecisionStatus::Active),
  sec("l1", None, "Layer 1", DecisionStatus::Active),
 ],
 vec![],
 vec![],
 vec![CrossRef {
  from_section: "l1".to_string(),
  to_target: "39".to_string(),
  ref_kind: RefKind::Decision,
  created_at_changelog_entry: None,
 }],
 );

 let errors = cross_ref_orphan_reject_with_workspace(&same_id_doc, &ws);
 assert!(errors.is_empty(), "step (1) intra-doc priority must PASS");
 }

 // ── Rule 2 ──────────────────────────────────────────────────────────

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
  sub_bullets: vec!["MUTATED sub-bullet".to_string()],
  frozen_at_transaction_time: 1,
 }],
 vec![],
 vec![],
 );
 let errors = changelog_entry_append_only(&prev, &curr);
 assert_eq!(errors.len(), 1);
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
  entry_id: "Round 2".to_string(),
  parent_changelog_entry: None,
  sub_bullets: vec!["new entry body".to_string()],
  frozen_at_transaction_time: 2,
  },
 ],
 vec![],
 vec![],
 );
 let errors = changelog_entry_append_only(&prev, &curr);
 assert!(errors.is_empty());
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
 let curr = make_doc(vec![], vec![], vec![], vec![]);
 let errors = changelog_entry_append_only(&prev, &curr);
 assert_eq!(errors.len(), 1);
 }

 // ── Rule 3 ──────────────────────────────────────────────────────────

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
  entry_id: "Round 1".to_string(),
  parent_changelog_entry: None,
  sub_bullets: vec![],
  frozen_at_transaction_time: 1,
 }],
 vec![FrozenList {
  list_id: "ten_cf_list".to_string(),
  created_at_changelog_entry: "Round 1".to_string(),
  members: vec!["a".to_string(), "b".to_string(), "c".to_string()],
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
  entry_id: "Round 2".to_string(),
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
 assert!(errors.is_empty());
 }

 // ── Rule 4 ──────────────────────────────────────────────────────────

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
  DecisionStatus::Superseded,
 )],
 vec![],
 vec![],
 vec![],
 );
 let errors = section_decision_status_transition(&prev, &curr);
 assert_eq!(errors.len(), 1);
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
  to_target: "56".to_string(),
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
