//! Projection: live JSON authoring store → canonical Layer-0 facts.
//!
//! This is the medium → canonical bridge for the design_doc adapter: it folds
//! an [`AtomicStore`] into the `mnemosyne-core` fact vocabulary that the RocksDB
//! materialized index consumes (Round 329 — Convergence B). It is the first
//! production code to read the live store and emit canonical facts, wiring the
//! previously-orphaned bitemporal substrate.
//!
//! Section and ChangelogEntry are projected here — their fact shapes were
//! settled by Convergence A/B (Section: R325/R326; ChangelogEntry: R330 dropped
//! the unsourced `appended_at`, leaving `round_number` + `summary`). FrozenList
//! has no atomic representation to project (R327).
//!
//! Scope kept honest to today's single-branch dogfood: every fact is projected
//! on [`MAIN_BRANCH_ID`]. Sections use `valid_from = 0` (one valid-time point);
//! changelog entries use `valid_from = round_number` (the round ordinal is a
//! natural logical clock). The bitemporal valid-time and branch axes populate
//! these slots when branching history is wired (Convergence B/C). The index is
//! rebuildable, so this scheme can evolve without a data migration.

use crate::AtomicStore;
use mnemosyne_core::{ChangelogEntryFact, CrossRefFact, FactKey, SectionFact};
use sha2::{Digest, Sha256};

/// Single-branch dogfood branch id. The branch axis becomes a real input when
/// branching is wired; until then every projected fact lives on this branch.
pub const MAIN_BRANCH_ID: u64 = 0;

/// Valid-time lower bound for a Section's single-snapshot projection. The
/// write side ([`AtomicStore::project_section_facts`]) stamps this on every
/// section's [`FactKey`], so the read side must address rows at the same
/// `valid_from` — exposing it as one `pub` constant keeps the two from
/// drifting (the index reader looks up sections at this exact slot). Real
/// valid-time populates it once bitemporal history exists.
pub const SECTION_VALID_FROM: u64 = 0;

/// Cross-ref kind emitted for an `AtomicSection.impact_scope` edge.
const IMPACT_SCOPE_REF_KIND: &str = "impact_scope";

/// Deterministic numeric entity id for a string `section_id`: the first 8 bytes
/// (big-endian) of its SHA-256 digest. Content-addressable and stable across
/// rebuilds, so the same section always maps to the same composite-key row. The
/// projected [`SectionFact`] keeps the original string id for reverse lookup.
pub fn section_entity_id(section_id: &str) -> u64 {
    let digest = Sha256::digest(section_id.as_bytes());
    let mut head = [0u8; 8];
    head.copy_from_slice(&digest[..8]);
    u64::from_be_bytes(head)
}

/// Parse the round ordinal out of a changelog `entry_id`. Keys are prose of the
/// form `Round <N> — …` (or bare `Round <N>`); the ordinal is the run of digits
/// after the `Round ` prefix. `None` for any key that does not name a round —
/// such entries are not projected (they are not changelog rounds).
///
/// Deriving the ordinal here is the deliberate Convergence-B resolution of the
/// "round number trapped in the prose key" debt: the JSON authoring key stays
/// prose, the index reads a real `u64` at projection time.
pub fn parse_round_number(entry_id: &str) -> Option<u64> {
    let rest = entry_id.strip_prefix("Round ")?;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

impl AtomicStore {
    /// Project every Section into a canonical [`SectionFact`] on `branch_id`.
    pub fn project_section_facts(&self, branch_id: u64) -> Vec<SectionFact> {
        self.sections
            .iter()
            .map(|(section_id, section)| SectionFact {
                key: FactKey {
                    branch_id,
                    entity_id: section_entity_id(section_id),
                    valid_from: SECTION_VALID_FROM,
                },
                section_id: section_id.clone(),
                skeleton: section.skeleton.clone(),
            })
            .collect()
    }

    /// Project every `AtomicSection.impact_scope` edge into a [`CrossRefFact`]
    /// relation (source section → target section). Both endpoints run through
    /// [`section_entity_id`], so a relation's `from`/`to` match the entity ids
    /// of the corresponding [`SectionFact`]s.
    pub fn project_cross_ref_facts(&self, branch_id: u64) -> Vec<CrossRefFact> {
        let mut out = Vec::new();
        for (section_id, section) in &self.sections {
            let from_section = section_entity_id(section_id);
            for target in &section.impact_scope {
                out.push(CrossRefFact {
                    branch_id,
                    from_section,
                    to_section: section_entity_id(target),
                    ref_kind: IMPACT_SCOPE_REF_KIND.to_string(),
                });
            }
        }
        out
    }

    /// Project every round-numbered changelog entry into a [`ChangelogEntryFact`]
    /// on `branch_id`. `round_number` (also the `entity_id` and `valid_from`,
    /// since the ordinal is this entity's identity *and* its logical clock) is
    /// parsed from the prose key; `summary` is the audit `decision_summary`,
    /// falling back to the key itself for legacy entries that predate the
    /// required-summary gate. Entries whose key does not name a round are
    /// skipped (see [`parse_round_number`]).
    pub fn project_changelog_entry_facts(&self, branch_id: u64) -> Vec<ChangelogEntryFact> {
        self.changelog_entries
            .iter()
            .filter_map(|(entry_id, entry)| {
                let round_number = parse_round_number(entry_id)?;
                let summary = entry
                    .decision_summary
                    .clone()
                    .unwrap_or_else(|| entry_id.clone());
                Some(ChangelogEntryFact {
                    key: FactKey {
                        branch_id,
                        entity_id: round_number,
                        valid_from: round_number,
                    },
                    round_number,
                    summary,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AtomicChangelogEntry, AtomicSection};
    use mnemosyne_core::{DecisionStatus, SectionSkeleton};

    fn store_with(sections: Vec<(&str, AtomicSection)>) -> AtomicStore {
        let mut store = AtomicStore::new();
        for (id, section) in sections {
            store.sections.insert(id.to_string(), section);
        }
        store
    }

    fn section(title: &str, impact: &[&str]) -> AtomicSection {
        AtomicSection {
            skeleton: SectionSkeleton {
                title: title.to_string(),
                parent_doc: "docs/DESIGN.md".to_string(),
                parent_section: None,
                decision_status: Some(DecisionStatus::Active),
            },
            impact_scope: impact.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn projects_one_section_fact_per_section() {
        let store = store_with(vec![
            ("alpha", section("Alpha", &[])),
            ("beta", section("Beta", &["alpha"])),
        ]);
        let facts = store.project_section_facts(MAIN_BRANCH_ID);
        assert_eq!(facts.len(), 2);
        let alpha = facts.iter().find(|f| f.section_id == "alpha").unwrap();
        assert_eq!(alpha.key.branch_id, MAIN_BRANCH_ID);
        assert_eq!(alpha.key.entity_id, section_entity_id("alpha"));
        assert_eq!(alpha.key.valid_from, 0);
        assert_eq!(alpha.skeleton.title, "Alpha");
        assert_eq!(alpha.skeleton.decision_status, Some(DecisionStatus::Active));
    }

    #[test]
    fn entity_id_is_deterministic_and_distinct() {
        assert_eq!(section_entity_id("alpha"), section_entity_id("alpha"));
        assert_ne!(section_entity_id("alpha"), section_entity_id("beta"));
    }

    #[test]
    fn cross_ref_endpoints_match_section_entity_ids() {
        let store = store_with(vec![
            ("alpha", section("Alpha", &[])),
            ("beta", section("Beta", &["alpha"])),
        ]);
        let refs = store.project_cross_ref_facts(MAIN_BRANCH_ID);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].from_section, section_entity_id("beta"));
        assert_eq!(refs[0].to_section, section_entity_id("alpha"));
        assert_eq!(refs[0].ref_kind, "impact_scope");
    }

    #[test]
    fn empty_store_projects_nothing() {
        let store = AtomicStore::new();
        assert!(store.project_section_facts(MAIN_BRANCH_ID).is_empty());
        assert!(store.project_cross_ref_facts(MAIN_BRANCH_ID).is_empty());
        assert!(store
            .project_changelog_entry_facts(MAIN_BRANCH_ID)
            .is_empty());
    }

    fn entry(summary: Option<&str>) -> AtomicChangelogEntry {
        AtomicChangelogEntry {
            decision_summary: summary.map(str::to_string),
            ..Default::default()
        }
    }

    fn store_with_entries(entries: Vec<(&str, AtomicChangelogEntry)>) -> AtomicStore {
        let mut store = AtomicStore::new();
        for (id, e) in entries {
            store.changelog_entries.insert(id.to_string(), e);
        }
        store
    }

    #[test]
    fn parse_round_number_forms() {
        assert_eq!(parse_round_number("Round 329 — Convergence B"), Some(329));
        assert_eq!(parse_round_number("Round 81"), Some(81));
        assert_eq!(parse_round_number("MD-DELETION-RATIFY"), None);
        assert_eq!(parse_round_number("Round abc"), None);
    }

    #[test]
    fn projects_changelog_entry_per_round() {
        let store = store_with_entries(vec![
            ("Round 329 — B1", entry(Some("did B1"))),
            ("Round 330 — B2", entry(Some("did B2"))),
        ]);
        let facts = store.project_changelog_entry_facts(MAIN_BRANCH_ID);
        assert_eq!(facts.len(), 2);
        let r330 = facts.iter().find(|f| f.round_number == 330).unwrap();
        assert_eq!(r330.key.branch_id, MAIN_BRANCH_ID);
        assert_eq!(r330.key.entity_id, 330);
        assert_eq!(r330.key.valid_from, 330);
        assert_eq!(r330.summary, "did B2");
    }

    #[test]
    fn changelog_summary_falls_back_to_key() {
        let store = store_with_entries(vec![("Round 250", entry(None))]);
        let facts = store.project_changelog_entry_facts(MAIN_BRANCH_ID);
        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].summary, "Round 250");
    }

    #[test]
    fn non_round_changelog_key_is_skipped() {
        let store = store_with_entries(vec![("legacy-note", entry(Some("x")))]);
        assert!(store
            .project_changelog_entry_facts(MAIN_BRANCH_ID)
            .is_empty());
    }
}
