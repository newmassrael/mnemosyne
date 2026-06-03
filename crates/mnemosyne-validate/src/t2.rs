//! T2 datalog rules — atomic frozen-ledger append-only enforcement.
//!
//! `frozen_ledger_atomic` compares two `AtomicStore` snapshots (prev = T1,
//! curr = T2, T1 < T2) and enforces the append-only invariant across the
//! atomic Section fields and the audit half of each ChangelogEntry: a
//! set-once field (intent / decision_summary) must not change once set, and
//! each bullet list must satisfy `prev ⊆ curr` (addition OK, removal/modify
//! reject). The bench-era markdown `frozen_ledger_jaccard` (ParsedDoc pair)
//! was removed with the markdown model (R400); the atomic-store axis is the
//! single source of truth.

use mnemosyne_atomic::{AtomicChangelogEntry, AtomicSection, AtomicStore, RejectedAlternative};
use std::collections::BTreeSet;

/// T2 ValidationError — atomic frozen-ledger append-only violations.
#[derive(Debug, Clone, PartialEq)]
pub enum T2ValidationError {
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

// Round 294 — audit-only scope: this function compares the audit half of
// `AtomicChangelogEntry` (decision_summary, changes_bullets,
// verification_bullets, impact_refs, carry_forward_bullets). The
// publishable_* half is intentionally OUT of T2 scope; it is the mutable
// view layer (R295 setters) gated by `[[publishable_override_ledger]]`
// (R296). Adding a publishable_* compare here would re-couple the layers
// and defeat the body-split invariant.
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
    prev: &[mnemosyne_atomic::ExampleBlock],
    curr: &[mnemosyne_atomic::ExampleBlock],
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
        let curr = atomic_store_with_section("39", atomic_section_with("intent", &["r1", "r2"]));
        let errors = frozen_ledger_atomic(&prev, &curr);
        assert!(errors.is_empty(), "append-only atomic bullet must PASS");
    }

    #[test]
    fn atomic_rejects_removed_section_bullet() {
        let prev = atomic_store_with_section("39", atomic_section_with("intent", &["r1", "r2"]));
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
            T2ValidationError::AtomicChangelogFrozen {
                field, entry_id, ..
            } => {
                assert_eq!(*field, "decision_summary");
                assert_eq!(entry_id, "Round 1");
            }
            _ => panic!("expected AtomicChangelogFrozen on decision_summary"),
        }
    }

    #[test]
    fn atomic_rejects_removed_entry_bullet() {
        let prev =
            atomic_store_with_entry("Round 1", atomic_entry_with("decision", &["c1", "c2"], &[]));
        let curr = atomic_store_with_entry("Round 1", atomic_entry_with("decision", &["c1"], &[]));
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
        assert!(
            errors.is_empty(),
            "section deletion is primitive-level concern"
        );
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
