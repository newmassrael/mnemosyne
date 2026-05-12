//! T2 datalog rules — / *T2 add* binding source (ratify).
//!
//! Phase 0b entry #6 — ratify carry. Bootstrap stages' T2 datalog
//! rule's *frozen-ledger-jaccard rule single-item* Stage 3 → Stage 1 pull-in
//! then Phase 0 entry. Remaining T2 rules (body-ref vs. frozen-list jaccard, etc.)
//! Phase 1B follow-up — introduced after the entry round.
//!
//! ## frozen_ledger_jaccard rule
//!
//! **input**: two ParsedDoc snapshot — `prev` (transaction_time T1) +
//! `curr` (transaction_time T2, T1 < T2). For ChangelogEntries with the same entry_id,
//! sub_bullets — set comparison between the two.
//!
//! **rule**: `jaccard(prev.sub_bullets, curr.sub_bullets) >= 1.0`'s *asymmetric
//! form* — `prev.sub_bullets ⊆ curr.sub_bullets` (T1's sub_bullets ⊆ T2's,
//! preserved as-is, append-only). T1 set ⊆ T2 set ⇒ PASS; otherwise threshold-driven
//! reject (frozen-ledger violation).
//!
//! **violation**: a sub_bullet present in T1 is partially modified in T2 (added item = allow,
//! removed or modified item = violation).
//!
//! ## Difference vs T1 rule 2
//!
//! T1 rule 2 (`changelog_entry_append_only`) = sequence equality enforced
//! (Vec equality, reordering also reject). T2 rule (`frozen_ledger_jaccard`) =
//! set inclusion enforced (T1 ⊆ T2, sub_bullets *add* allowed — append-only
//! meaning consistency). T2 is *slightly more lenient* than T1 (sub_bullets — additions allowed; removal
//! block); datalog rule format auto-enforced.

use crate::atomic::{AtomicChangelogEntry, AtomicSection, AtomicStore, RejectedAlternative};
use crate::schema::{ChangelogEntry, ParsedDoc};
use std::collections::{BTreeMap, BTreeSet};

/// T2 ValidationError — `frozen_ledger_jaccard` rule violation.
///
/// `Eq` derive missing — `f64` IEEE 754 NaN in partial-only ordering at per
/// `Eq` not implemented (Rust trait bound). PartialEq is the only derived trait — used for assertions / pattern matches.
/// matching uses the PartialEq path.
#[derive(Debug, Clone, PartialEq)]
pub enum T2ValidationError {
 /// Partial modification of T1's sub_bullets (remove or modify) — frozen-ledger violation.
 /// `missing_in_curr` = exists in prev but not in curr's sub_bullets set.
 FrozenLedgerJaccardViolation {
 entry_id: String,
 prev_sub_bullets_count: usize,
 curr_sub_bullets_count: usize,
 missing_in_curr: Vec<String>,
 jaccard_asymmetric: f64,
 },
 /// — atomic Section field append-only violation.
 /// `field` = "intent" / "rationale_bullets" /... etc. any atomic 8 field
 /// Explicit out-of-scope violation. `missing_in_curr` = exists in prev but not in curr.
 /// no entries (intent: compare prev and curr values).
 AtomicSectionFrozen {
 section_id: String,
 field: &'static str,
 missing_in_curr: Vec<String>,
 },
 /// — atomic ChangelogEntry field append-only violation.
 /// 5 field (decision_summary / changes_bullets / verification_bullets /
 /// impact_refs / carry_forward_bullets) — registers a `prev ⊆ curr` violation when violated.
 AtomicChangelogFrozen {
 entry_id: String,
 field: &'static str,
 missing_in_curr: Vec<String>,
 },
}

/// `frozen_ledger_jaccard` T2 rule.
///
/// Two ParsedDoc snapshots (prev = T1, curr = T2, T1 < T2) — same entry_id
/// ChangelogEntry in `prev.sub_bullets ⊆ curr.sub_bullets` check.
///
/// carry form:
/// - prev.entry_id missing in curr → ChangelogEntry itself was removed; T1 rule 2
/// `changelog_entry_append_only`) — scoped to entry deletion. This T2 rule
/// Entry deletion is unchecked (T1 rule 2 — reject takes priority).
/// - prev.entry_id exists in curr + prev.sub_bullets ⊄ curr.sub_bullets
/// → `FrozenLedgerJaccardViolation` registered (remove sub_bullets explicit).
/// - prev.sub_bullets ⊆ curr.sub_bullets → PASS (sub_bullets add allow).
pub fn frozen_ledger_jaccard(prev: &ParsedDoc, curr: &ParsedDoc) -> Vec<T2ValidationError> {
 let curr_by_id: BTreeMap<&str, &ChangelogEntry> = curr
 .changelog_entries
 .iter()
 .map(|e| (e.entry_id.as_str(), e))
 .collect();

 let mut errors = Vec::new();
 for prev_entry in &prev.changelog_entries {
 let curr_entry = match curr_by_id.get(prev_entry.entry_id.as_str()) {
 Some(e) => e,
 // entry deletion T1 rule 2 scope — this T2 rule unchecked.
 None => continue,
 };

 let prev_set: BTreeSet<&str> = prev_entry
 .sub_bullets
 .iter()
 .map(String::as_str)
 .collect();
 let curr_set: BTreeSet<&str> = curr_entry
 .sub_bullets
 .iter()
 .map(String::as_str)
 .collect();

 // Asymmetric jaccard: |prev ∩ curr| / |prev|.
 // ≥ 1.0 ↔ prev ⊆ curr (all prev sub_bullet curr in exists).
 let intersection_size = prev_set.intersection(&curr_set).count();
 let prev_size = prev_set.len();

 if prev_size == 0 {
 // empty prev — vacuous PASS (jaccard undefined → 1.0 default).
 continue;
 }

 let jaccard = intersection_size as f64 / prev_size as f64;
 if jaccard < 1.0 {
 // T1 in sub_bullets in-progress T2 in missing item explicit.
 let missing: Vec<String> = prev_set
  .difference(&curr_set)
  .map(|s| s.to_string())
  .collect();
 errors.push(T2ValidationError::FrozenLedgerJaccardViolation {
  entry_id: prev_entry.entry_id.clone(),
  prev_sub_bullets_count: prev_size,
  curr_sub_bullets_count: curr_set.len(),
  missing_in_curr: missing,
  jaccard_asymmetric: jaccard,
 });
 }
 }
 errors
}

/// `frozen_ledger_atomic` — LEGACY-FIELD-REMOVAL round 2 ratify.
///
/// AtomicStore prev/curr snapshot covers the atomic Section 8 fields + atomic
/// Append-only invariant check across ChangelogEntry's 5 atomic fields. ratify
/// (T2 frozen_ledger_jaccard rule extends to atomic fields) production wire.
///
/// **rule** (Section scope, 8 field):
/// - intent (Option<String>): prev.intent.is_some() → curr.intent == prev.intent
/// (set once; subsequent changes rejected). prev = None → curr can be set freely.
/// - 6 bullet list (rationale_bullets, inputs_bullets, outputs_bullets,
/// caveats_bullets, alternatives_rejected, impact_scope, examples):
/// prev set ⊆ curr set (set inclusion, addition OK / removal+modify reject).
///
/// **rule** (ChangelogEntry scope, 5 field):
/// - decision_summary (Option<String>): equality (entire entry frozen scope consistency).
/// - 4 bullet list (changes / verification / impact_refs / carry_forward):
/// prev ⊆ curr.
///
/// **Note**: the atomic-store mutate primitive itself only allows set/append (atomic.rs
/// (the frozen ledger's entire-entry check carries stable). This rule = external mutate
/// (Markdown round-trip etc. is no longer affected by indirect mutation through atomic fields,
/// guaranteedperform audit gate.
pub fn frozen_ledger_atomic(prev: &AtomicStore, curr: &AtomicStore) -> Vec<T2ValidationError> {
 let mut errors = Vec::new();

 for (section_id, prev_section) in &prev.sections {
 let curr_section = match curr.sections.get(section_id) {
 Some(s) => s,
 None => continue, // section itself removed = atomic primitive scope block
 };
 check_atomic_section(section_id, prev_section, curr_section, &mut errors);
 }

 for (entry_id, prev_entry) in &prev.changelog_entries {
 let curr_entry = match curr.changelog_entries.get(entry_id) {
 Some(e) => e,
 None => continue, // entry itself removed = atomic primitive scope block
 };
 check_atomic_entry(entry_id, prev_entry, curr_entry, &mut errors);
 }

 errors
}

fn check_atomic_section(
 section_id: &str,
 prev: &AtomicSection,
 curr: &AtomicSection,
 errors: &mut Vec<T2ValidationError>,
) {
 if let Some(prev_intent) = prev.intent.as_ref().filter(|s| !s.is_empty()) {
 let curr_intent = curr.intent.as_deref().unwrap_or("");
 if prev_intent.as_str() != curr_intent {
 errors.push(T2ValidationError::AtomicSectionFrozen {
  section_id: section_id.to_string(),
  field: "intent",
  missing_in_curr: vec![prev_intent.clone()],
 });
 }
 }
 push_string_diff(
 section_id,
 "rationale_bullets",
 &prev.rationale_bullets,
 &curr.rationale_bullets,
 errors,
 T2ValidationError::section_frozen,
 );
 push_string_diff(
 section_id,
 "inputs_bullets",
 &prev.inputs_bullets,
 &curr.inputs_bullets,
 errors,
 T2ValidationError::section_frozen,
 );
 push_string_diff(
 section_id,
 "outputs_bullets",
 &prev.outputs_bullets,
 &curr.outputs_bullets,
 errors,
 T2ValidationError::section_frozen,
 );
 push_string_diff(
 section_id,
 "caveats_bullets",
 &prev.caveats_bullets,
 &curr.caveats_bullets,
 errors,
 T2ValidationError::section_frozen,
 );
 push_string_diff(
 section_id,
 "impact_scope",
 &prev.impact_scope,
 &curr.impact_scope,
 errors,
 T2ValidationError::section_frozen,
 );
 push_alternatives_diff(
 section_id,
 &prev.alternatives_rejected,
 &curr.alternatives_rejected,
 errors,
 );
 push_examples_diff(section_id, &prev.examples, &curr.examples, errors);
}

fn check_atomic_entry(
 entry_id: &str,
 prev: &AtomicChangelogEntry,
 curr: &AtomicChangelogEntry,
 errors: &mut Vec<T2ValidationError>,
) {
 if let Some(prev_summary) = prev.decision_summary.as_ref().filter(|s| !s.is_empty()) {
 let curr_summary = curr.decision_summary.as_deref().unwrap_or("");
 if prev_summary.as_str() != curr_summary {
 errors.push(T2ValidationError::AtomicChangelogFrozen {
  entry_id: entry_id.to_string(),
  field: "decision_summary",
  missing_in_curr: vec![prev_summary.clone()],
 });
 }
 }
 push_string_diff(
 entry_id,
 "changes_bullets",
 &prev.changes_bullets,
 &curr.changes_bullets,
 errors,
 T2ValidationError::entry_frozen,
 );
 push_string_diff(
 entry_id,
 "verification_bullets",
 &prev.verification_bullets,
 &curr.verification_bullets,
 errors,
 T2ValidationError::entry_frozen,
 );
 push_string_diff(
 entry_id,
 "impact_refs",
 &prev.impact_refs,
 &curr.impact_refs,
 errors,
 T2ValidationError::entry_frozen,
 );
 push_string_diff(
 entry_id,
 "carry_forward_bullets",
 &prev.carry_forward_bullets,
 &curr.carry_forward_bullets,
 errors,
 T2ValidationError::entry_frozen,
 );
}

impl T2ValidationError {
 fn section_frozen(
 section_id: &str,
 field: &'static str,
 missing: Vec<String>,
 ) -> T2ValidationError {
 T2ValidationError::AtomicSectionFrozen {
 section_id: section_id.to_string(),
 field,
 missing_in_curr: missing,
 }
 }
 fn entry_frozen(
 entry_id: &str,
 field: &'static str,
 missing: Vec<String>,
 ) -> T2ValidationError {
 T2ValidationError::AtomicChangelogFrozen {
 entry_id: entry_id.to_string(),
 field,
 missing_in_curr: missing,
 }
 }
}

fn push_string_diff(
 target_id: &str,
 field: &'static str,
 prev: &[String],
 curr: &[String],
 errors: &mut Vec<T2ValidationError>,
 make_err: fn(&str, &'static str, Vec<String>) -> T2ValidationError,
) {
 let prev_set: BTreeSet<&str> = prev.iter().map(String::as_str).collect();
 let curr_set: BTreeSet<&str> = curr.iter().map(String::as_str).collect();
 let missing: Vec<String> = prev_set
 .difference(&curr_set)
 .map(|s| s.to_string())
 .collect();
 if !missing.is_empty() {
 errors.push(make_err(target_id, field, missing));
 }
}

fn push_alternatives_diff(
 section_id: &str,
 prev: &[RejectedAlternative],
 curr: &[RejectedAlternative],
 errors: &mut Vec<T2ValidationError>,
) {
 let curr_set: BTreeSet<(&str, &str)> = curr
 .iter()
 .map(|a| (a.alternative.as_str(), a.reason.as_str()))
 .collect();
 let missing: Vec<String> = prev
 .iter()
 .filter(|a| !curr_set.contains(&(a.alternative.as_str(), a.reason.as_str())))
 .map(|a| format!("{} -- {}", a.alternative, a.reason))
 .collect();
 if !missing.is_empty() {
 errors.push(T2ValidationError::AtomicSectionFrozen {
 section_id: section_id.to_string(),
 field: "alternatives_rejected",
 missing_in_curr: missing,
 });
 }
}

fn push_examples_diff(
 section_id: &str,
 prev: &[crate::atomic::ExampleBlock],
 curr: &[crate::atomic::ExampleBlock],
 errors: &mut Vec<T2ValidationError>,
) {
 let curr_set: BTreeSet<(&str, &str)> = curr
 .iter()
 .map(|e| (e.language.as_str(), e.code.as_str()))
 .collect();
 let missing: Vec<String> = prev
 .iter()
 .filter(|e| !curr_set.contains(&(e.language.as_str(), e.code.as_str())))
 .map(|e| format!("```{}\n{}\n```", e.language, e.code))
 .collect();
 if !missing.is_empty() {
 errors.push(T2ValidationError::AtomicSectionFrozen {
 section_id: section_id.to_string(),
 field: "examples",
 missing_in_curr: missing,
 });
 }
}

#[cfg(test)]
mod tests {
 use super::*;
 use crate::schema::ChangelogEntry;

 fn entry(id: &str, bullets: &[&str], txn: i64) -> ChangelogEntry {
 ChangelogEntry {
 entry_id: id.to_string(),
 parent_changelog_entry: None,
 sub_bullets: bullets.iter().map(|s| s.to_string()).collect(),
 frozen_at_transaction_time: txn,
 }
 }

 fn doc(entries: Vec<ChangelogEntry>) -> ParsedDoc {
 ParsedDoc {
 changelog_entries: entries,
 ..Default::default()
 }
 }

 #[test]
 fn jaccard_passes_on_identity() {
 let prev = doc(vec![entry("Round 1", &["a", "b", "c"], 1)]);
 let curr = doc(vec![entry("Round 1", &["a", "b", "c"], 1)]);
 let errors = frozen_ledger_jaccard(&prev, &curr);
 assert!(errors.is_empty(), "identical sub_bullets must PASS");
 }

 #[test]
 fn jaccard_passes_on_appended_bullet() {
 // T2 = T1 ∪ {new} — T1 ⊆ T2, jaccard = 1.0.
 let prev = doc(vec![entry("Round 1", &["a", "b"], 1)]);
 let curr = doc(vec![entry("Round 1", &["a", "b", "c"], 1)]);
 let errors = frozen_ledger_jaccard(&prev, &curr);
 assert!(
 errors.is_empty(),
 "append-only sub_bullets must PASS (T1 ⊆ T2)"
 );
 }

 #[test]
 fn jaccard_rejects_removed_bullet() {
 let prev = doc(vec![entry("Round 1", &["a", "b", "c"], 1)]);
 let curr = doc(vec![entry("Round 1", &["a", "b"], 1)]);
 let errors = frozen_ledger_jaccard(&prev, &curr);
 assert_eq!(errors.len(), 1, "removed bullet must reject");
 if let T2ValidationError::FrozenLedgerJaccardViolation {
 entry_id,
 missing_in_curr,
 jaccard_asymmetric,
 ..
 } = &errors[0]
 {
 assert_eq!(entry_id, "Round 1");
 assert_eq!(missing_in_curr, &vec!["c".to_string()]);
 assert!(*jaccard_asymmetric < 1.0);
 }
 }

 #[test]
 fn jaccard_rejects_modified_bullet() {
 // T2 in "b" → "B" mutation — content drift, T1 ⊄ T2.
 let prev = doc(vec![entry("Round 1", &["a", "b"], 1)]);
 let curr = doc(vec![entry("Round 1", &["a", "B"], 1)]);
 let errors = frozen_ledger_jaccard(&prev, &curr);
 assert_eq!(errors.len(), 1, "modified bullet must reject");
 }

 #[test]
 fn jaccard_skips_entry_deletion() {
 // entry itself removed — T1 rule 2 scope, T2 unchecked.
 let prev = doc(vec![entry("Round 1", &["a"], 1), entry("Round 2", &["b"], 2)]);
 let curr = doc(vec![entry("Round 1", &["a"], 1)]);
 let errors = frozen_ledger_jaccard(&prev, &curr);
 assert!(errors.is_empty(), "entry deletion is T1 rule 2 territory");
 }

 #[test]
 fn jaccard_passes_on_empty_prev() {
 let prev = doc(vec![entry("Round 1", &[], 1)]);
 let curr = doc(vec![entry("Round 1", &["new"], 1)]);
 let errors = frozen_ledger_jaccard(&prev, &curr);
 assert!(errors.is_empty(), "empty prev sub_bullets is vacuous PASS");
 }

 #[test]
 fn jaccard_handles_multiple_entries() {
 let prev = doc(vec![
 entry("Round 1", &["a", "b"], 1),
 entry("Round 2", &["c"], 2),
 ]);
 let curr = doc(vec![
 entry("Round 1", &["a"], 1), // violation
 entry("Round 2", &["c", "d"], 2), // PASS (append)
 ]);
 let errors = frozen_ledger_jaccard(&prev, &curr);
 assert_eq!(errors.len(), 1);
 if let T2ValidationError::FrozenLedgerJaccardViolation { entry_id, .. } = &errors[0] {
 assert_eq!(entry_id, "Round 1");
 }
 }

 #[test]
 fn jaccard_self_check_design_md_passes() {
 // DESIGN.md itself frozen ledger principle in self-check — same doc two time parse
 // on jaccard = 1.0 (vector equality in frozen principle carry).
 use crate::parser::{design_doc_small_fixture, parse_markdown};
 let a = parse_markdown(design_doc_small_fixture(), "DESIGN.md");
 let b = parse_markdown(design_doc_small_fixture(), "DESIGN.md");
 let errors = frozen_ledger_jaccard(&a, &b);
 assert!(
 errors.is_empty(),
 "self-parse in frozen ledger jaccard violation 0 contract PASS"
 );
 }

 // ========================================================================
 // — atomic frozen ledger tests (LEGACY-FIELD-REMOVAL round 2).
 // ========================================================================

 fn atomic_section_with(intent: &str, rationale: &[&str]) -> AtomicSection {
 AtomicSection {
 intent: Some(intent.to_string()),
 rationale_bullets: rationale.iter().map(|s| s.to_string()).collect(),
 ..Default::default()
 }
 }

 fn atomic_entry_with(
 summary: &str,
 changes: &[&str],
 verification: &[&str],
 ) -> AtomicChangelogEntry {
 AtomicChangelogEntry {
 decision_summary: Some(summary.to_string()),
 changes_bullets: changes.iter().map(|s| s.to_string()).collect(),
 verification_bullets: verification.iter().map(|s| s.to_string()).collect(),
 ..Default::default()
 }
 }

 fn atomic_store_with_section(id: &str, s: AtomicSection) -> AtomicStore {
 let mut store = AtomicStore::default();
 store.sections.insert(id.to_string(), s);
 store
 }

 fn atomic_store_with_entry(id: &str, e: AtomicChangelogEntry) -> AtomicStore {
 let mut store = AtomicStore::default();
 store.changelog_entries.insert(id.to_string(), e);
 store
 }

 #[test]
 fn atomic_passes_on_identity() {
 let s = atomic_section_with("intent A", &["r1", "r2"]);
 let prev = atomic_store_with_section("39", s.clone());
 let curr = atomic_store_with_section("39", s);
 let errors = frozen_ledger_atomic(&prev, &curr);
 assert!(errors.is_empty(), "identical atomic must PASS");
 }

 #[test]
 fn atomic_passes_on_appended_bullet() {
 let prev = atomic_store_with_section("39", atomic_section_with("intent", &["r1"]));
 let curr = atomic_store_with_section(
 "39",
 atomic_section_with("intent", &["r1", "r2"]),
 );
 let errors = frozen_ledger_atomic(&prev, &curr);
 assert!(errors.is_empty(), "append-only atomic bullet must PASS");
 }

 #[test]
 fn atomic_rejects_removed_section_bullet() {
 let prev = atomic_store_with_section(
 "39",
 atomic_section_with("intent", &["r1", "r2"]),
 );
 let curr = atomic_store_with_section("39", atomic_section_with("intent", &["r1"]));
 let errors = frozen_ledger_atomic(&prev, &curr);
 assert_eq!(errors.len(), 1);
 match &errors[0] {
 T2ValidationError::AtomicSectionFrozen {
  section_id,
  field,
  missing_in_curr,
 } => {
  assert_eq!(section_id, "39");
  assert_eq!(*field, "rationale_bullets");
  assert_eq!(missing_in_curr, &vec!["r2".to_string()]);
 }
 _ => panic!("expected AtomicSectionFrozen"),
 }
 }

 #[test]
 fn atomic_rejects_intent_mutation() {
 let prev = atomic_store_with_section("39", atomic_section_with("intent A", &[]));
 let curr = atomic_store_with_section("39", atomic_section_with("intent B", &[]));
 let errors = frozen_ledger_atomic(&prev, &curr);
 assert_eq!(errors.len(), 1);
 match &errors[0] {
 T2ValidationError::AtomicSectionFrozen { field, .. } => {
  assert_eq!(*field, "intent");
 }
 _ => panic!("expected AtomicSectionFrozen on intent"),
 }
 }

 #[test]
 fn atomic_rejects_decision_summary_mutation() {
 let prev = atomic_store_with_entry("Round 1", atomic_entry_with("decision A", &[], &[]));
 let curr = atomic_store_with_entry("Round 1", atomic_entry_with("decision B", &[], &[]));
 let errors = frozen_ledger_atomic(&prev, &curr);
 assert_eq!(errors.len(), 1);
 match &errors[0] {
 T2ValidationError::AtomicChangelogFrozen { field, entry_id, .. } => {
  assert_eq!(*field, "decision_summary");
  assert_eq!(entry_id, "Round 1");
 }
 _ => panic!("expected AtomicChangelogFrozen on decision_summary"),
 }
 }

 #[test]
 fn atomic_rejects_removed_entry_bullet() {
 let prev = atomic_store_with_entry(
 "Round 1",
 atomic_entry_with("decision", &["c1", "c2"], &[]),
 );
 let curr = atomic_store_with_entry(
 "Round 1",
 atomic_entry_with("decision", &["c1"], &[]),
 );
 let errors = frozen_ledger_atomic(&prev, &curr);
 assert_eq!(errors.len(), 1);
 match &errors[0] {
 T2ValidationError::AtomicChangelogFrozen {
  field,
  missing_in_curr,
  ..
 } => {
  assert_eq!(*field, "changes_bullets");
  assert_eq!(missing_in_curr, &vec!["c2".to_string()]);
 }
 _ => panic!("expected AtomicChangelogFrozen on changes_bullets"),
 }
 }

 #[test]
 fn atomic_skips_section_deletion() {
 // Section itself removed = atomic primitive scope block (T1 atomic equivalent).
 let prev = atomic_store_with_section("39", atomic_section_with("intent", &[]));
 let curr = AtomicStore::default();
 let errors = frozen_ledger_atomic(&prev, &curr);
 assert!(errors.is_empty(), "section deletion is primitive-level concern");
 }

 #[test]
 fn atomic_passes_on_empty_prev_intent() {
 // prev.intent = None → curr.intent freely set possible.
 let prev = atomic_store_with_section(
 "39",
 AtomicSection {
  intent: None,
  ..Default::default()
 },
 );
 let curr = atomic_store_with_section("39", atomic_section_with("first intent", &[]));
 let errors = frozen_ledger_atomic(&prev, &curr);
 assert!(errors.is_empty(), "empty prev intent allows first-set");
 }
}
