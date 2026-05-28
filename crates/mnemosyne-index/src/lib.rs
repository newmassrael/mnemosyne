//! Materialize the RocksDB fact index from the atomic log (Convergence B3).
//!
//! This is the *application service* that wires the previously-orphaned
//! bitemporal substrate: it reads the live authoring log ([`AtomicStore`]),
//! projects it into canonical facts (the `mnemosyne-atomic` projection from
//! Convergence B1/B2), and writes them into the composite-key RocksDB store via
//! [`TypedFactStore`]. It depends *inward* on the adapter + the persistence
//! layer; nothing in those layers depends back on it, so the dependency
//! direction stays correct (the projection engine `mnemosyne-cascade` never has
//! to know about the design_doc adapter).
//!
//! The index is a **derived, rebuildable view** of the log — never an
//! authoritative store (ARCHITECTURE.md anti-drift invariant #2). [`rebuild_index`]
//! is therefore idempotent: re-running it over the same log reproduces the same
//! rows (composite keys are deterministic), so a stale or deleted index is
//! always recoverable by replaying the log.

use mnemosyne_atomic::AtomicStore;
use mnemosyne_facts::{PersistError, TypedFactStore};
use mnemosyne_store::MnemosyneStore;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum IndexError {
    #[error(transparent)]
    Persist(#[from] PersistError),
}

/// Per-kind row counts written by a [`rebuild_index`] pass.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RebuildStats {
    pub sections: usize,
    pub changelog_entries: usize,
    pub cross_refs: usize,
}

impl RebuildStats {
    /// Total rows written across every fact kind.
    pub fn total(&self) -> usize {
        self.sections + self.changelog_entries + self.cross_refs
    }
}

/// Project the atomic log into canonical facts and persist them into the RocksDB
/// index under `branch_id`. Returns the per-kind row counts.
///
/// Idempotent: each fact maps to a deterministic composite key, so re-running
/// over the same log overwrites in place and the index converges to the same
/// state. FrozenList is not projected — it has no atomic representation (R327).
pub fn rebuild_index(
    atomic: &AtomicStore,
    store: &MnemosyneStore,
    branch_id: u64,
) -> Result<RebuildStats, IndexError> {
    let typed = TypedFactStore::new(store);
    let mut stats = RebuildStats::default();

    for fact in atomic.project_section_facts(branch_id) {
        typed.put_section(&fact)?;
        stats.sections += 1;
    }
    for fact in atomic.project_changelog_entry_facts(branch_id) {
        typed.put_changelog_entry(&fact)?;
        stats.changelog_entries += 1;
    }
    for fact in atomic.project_cross_ref_facts(branch_id) {
        typed.put_cross_ref(&fact)?;
        stats.cross_refs += 1;
    }

    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mnemosyne_atomic::{
        section_entity_id, AtomicChangelogEntry, AtomicSection, MAIN_BRANCH_ID,
    };
    use mnemosyne_core::{DecisionStatus, SectionSkeleton};
    use tempfile::TempDir;

    fn sample_atomic() -> AtomicStore {
        let mut atomic = AtomicStore::new();
        atomic.sections.insert(
            "alpha".into(),
            AtomicSection {
                skeleton: SectionSkeleton {
                    title: "Alpha".into(),
                    parent_doc: "docs/DESIGN.md".into(),
                    parent_section: None,
                    decision_status: Some(DecisionStatus::Active),
                },
                ..Default::default()
            },
        );
        atomic.sections.insert(
            "beta".into(),
            AtomicSection {
                skeleton: SectionSkeleton {
                    title: "Beta".into(),
                    parent_doc: "docs/DESIGN.md".into(),
                    parent_section: None,
                    decision_status: None,
                },
                impact_scope: vec!["alpha".into()],
                ..Default::default()
            },
        );
        atomic.changelog_entries.insert(
            "Round 329 — B1".into(),
            AtomicChangelogEntry {
                decision_summary: Some("did b1".into()),
                ..Default::default()
            },
        );
        atomic
    }

    #[test]
    fn rebuild_then_read_back_round_trips() {
        let atomic = sample_atomic();
        let dir = TempDir::new().unwrap();
        let store = MnemosyneStore::open(dir.path()).unwrap();

        let stats = rebuild_index(&atomic, &store, MAIN_BRANCH_ID).unwrap();
        assert_eq!(stats.sections, 2);
        assert_eq!(stats.changelog_entries, 1);
        assert_eq!(stats.cross_refs, 1);
        assert_eq!(stats.total(), 4);

        let typed = TypedFactStore::new(&store);
        let alpha = typed
            .get_section(MAIN_BRANCH_ID, section_entity_id("alpha"), 0)
            .unwrap()
            .expect("alpha section persisted");
        assert_eq!(alpha.section_id, "alpha");
        assert_eq!(alpha.skeleton.title, "Alpha");
        assert_eq!(alpha.skeleton.decision_status, Some(DecisionStatus::Active));

        let entry = typed
            .get_changelog_entry(MAIN_BRANCH_ID, 329, 329)
            .unwrap()
            .expect("changelog entry persisted");
        assert_eq!(entry.round_number, 329);
        assert_eq!(entry.summary, "did b1");

        let cross = typed
            .get_cross_ref(
                MAIN_BRANCH_ID,
                section_entity_id("beta"),
                section_entity_id("alpha"),
            )
            .unwrap()
            .expect("impact_scope cross-ref persisted");
        assert_eq!(cross.ref_kind, "impact_scope");
    }

    #[test]
    fn rebuild_is_idempotent() {
        let atomic = sample_atomic();
        let dir = TempDir::new().unwrap();
        let store = MnemosyneStore::open(dir.path()).unwrap();

        let first = rebuild_index(&atomic, &store, MAIN_BRANCH_ID).unwrap();
        let second = rebuild_index(&atomic, &store, MAIN_BRANCH_ID).unwrap();
        assert_eq!(first, second);

        // Re-reading after the second pass still yields exactly one row per fact.
        let typed = TypedFactStore::new(&store);
        let alpha = typed
            .get_section(MAIN_BRANCH_ID, section_entity_id("alpha"), 0)
            .unwrap();
        assert!(alpha.is_some());
    }

    #[test]
    fn empty_log_rebuilds_to_empty_index() {
        let dir = TempDir::new().unwrap();
        let store = MnemosyneStore::open(dir.path()).unwrap();
        let stats = rebuild_index(&AtomicStore::new(), &store, MAIN_BRANCH_ID).unwrap();
        assert_eq!(stats.total(), 0);
    }
}
