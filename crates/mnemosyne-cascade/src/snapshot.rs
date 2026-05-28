//! Branch snapshot — typed-fact bundle that the cascade query body reads.
//!
//! this module cascade_query's actual body invariant validation in use-
//! Per-branch typed-fact snapshot wire format + load helper. round
//! Actual body carry — `mnemosyne-store` raw bytes → `mnemosyne-facts` typed
//! facts → this snapshot's read path.
//!
//! ## Wire format
//!
//! `serde_json` serialize (mnemosyne-facts/cascade are both workspace deps, already registered).
//! Deterministic encoding — BTreeMap sort order preserved as the Salsa input's cache key.
//! `Vec<u8>` byte-equal match guarantees a memoize hit.
//!
//! ## Phase 0 entity-kind partition contract
//!
//! Section / ChangelogEntry / FrozenList all share the `entities` CF — fact-kind
//! cannot identify itself by byte-shape alone (encode_value's length-prefix is coincidental).
//! match possible). Phase 0 contract — callers must partition entity_id explicitly (any
//! any entity_id is recognized regardless of fact kind). Phase 1.5+ entity_id namespace allocation
//! the policy explicitly enables an auto-dispatch entry path.

use mnemosyne_facts::{
    ChangelogEntryFact, CrossRefFact, FrozenListFact, PersistError, SectionFact, TypedFactStore,
};
use mnemosyne_store::MnemosyneStore;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// `BranchSnapshotData` — per-branch typed-fact bundle that the cascade query
/// body reads. The Salsa input field is `Vec<u8>` and serializes as the cache key.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BranchSnapshotData {
    pub sections: Vec<SectionFact>,
    pub changelog_entries: Vec<ChangelogEntryFact>,
    pub frozen_lists: Vec<FrozenListFact>,
    pub cross_refs: Vec<CrossRefFact>,
}

/// `BranchEntityPartition` — Phase 0 caller-supplied entity_id partition per
/// fact kind. `load_from_store` reads each kind via the appropriate
/// `TypedFactStore::get_X` accessor.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BranchEntityPartition {
    /// Section facts to load — each entry is `(entity_id, valid_from)`.
    pub sections: Vec<(u64, u64)>,
    pub changelog_entries: Vec<(u64, u64)>,
    pub frozen_lists: Vec<(u64, u64)>,
    /// Cross-ref relations — each entry is `(from_section, to_section)`.
    pub cross_refs: Vec<(u64, u64)>,
}

#[derive(Debug, Error)]
pub enum SnapshotError {
    #[error("snapshot encode failed: {0}")]
    Encode(serde_json::Error),
    #[error("snapshot decode failed: {0}")]
    Decode(serde_json::Error),
    #[error(transparent)]
    Persist(#[from] PersistError),
    #[error(transparent)]
    Store(#[from] mnemosyne_store::StoreError),
}

impl BranchSnapshotData {
    pub fn new() -> Self {
        Self::default()
    }

    /// Deterministic encoding — identical input bytes for identical snapshot.
    pub fn encode(&self) -> Result<Vec<u8>, SnapshotError> {
        serde_json::to_vec(self).map_err(SnapshotError::Encode)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, SnapshotError> {
        if bytes.is_empty() {
            return Ok(Self::default());
        }
        serde_json::from_slice(bytes).map_err(SnapshotError::Decode)
    }

    /// Total fact count — used for measurement size band classification.
    pub fn fact_count(&self) -> usize {
        self.sections.len()
            + self.changelog_entries.len()
            + self.frozen_lists.len()
            + self.cross_refs.len()
    }

    /// Load typed facts from the underlying store — Phase 0 contract: caller
    /// provides `BranchEntityPartition` listing entity_ids per kind. Iterates
    /// the partition and dispatches to `TypedFactStore::get_X` per kind.
    pub fn load_from_store(
        store: &MnemosyneStore,
        branch_id: u64,
        partition: &BranchEntityPartition,
    ) -> Result<Self, SnapshotError> {
        let typed = TypedFactStore::new(store);
        let mut data = Self::default();
        for (entity_id, valid_from) in &partition.sections {
            if let Some(s) = typed.get_section(branch_id, *entity_id, *valid_from)? {
                data.sections.push(s);
            }
        }
        for (entity_id, valid_from) in &partition.changelog_entries {
            if let Some(c) = typed.get_changelog_entry(branch_id, *entity_id, *valid_from)? {
                data.changelog_entries.push(c);
            }
        }
        for (entity_id, valid_from) in &partition.frozen_lists {
            if let Some(f) = typed.get_frozen_list(branch_id, *entity_id, *valid_from)? {
                data.frozen_lists.push(f);
            }
        }
        for (from_section, to_section) in &partition.cross_refs {
            if let Some(cr) = typed.get_cross_ref(branch_id, *from_section, *to_section)? {
                data.cross_refs.push(cr);
            }
        }
        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mnemosyne_facts::{
        ChangelogEntryFact, CrossRefFact, DecisionStatus, FactKey, FrozenListFact, SectionFact,
        SectionSkeleton, TypedFactStore,
    };
    use tempfile::TempDir;

    fn sample_snapshot() -> BranchSnapshotData {
        BranchSnapshotData {
            sections: vec![SectionFact {
                key: FactKey {
                    branch_id: 1,
                    entity_id: 39,
                    valid_from: 100,
                },
                section_id: "39".into(),
                skeleton: SectionSkeleton {
                    title: "graph_schema".into(),
                    parent_doc: "docs/DESIGN.md".into(),
                    parent_section: None,
                    decision_status: Some(DecisionStatus::Active),
                },
            }],
            changelog_entries: vec![ChangelogEntryFact {
                key: FactKey {
                    branch_id: 1,
                    entity_id: 81,
                    valid_from: 100,
                },
                round_number: 81,
                summary: "Round 81 — cascade body".into(),
            }],
            frozen_lists: vec![FrozenListFact {
                key: FactKey {
                    branch_id: 1,
                    entity_id: 1000,
                    valid_from: 100,
                },
                owner_section: 39,
                frozen_round: 60,
                kind: "release_lock".into(),
            }],
            cross_refs: vec![CrossRefFact {
                branch_id: 1,
                from_section: 66,
                to_section: 39,
                ref_kind: "decision".into(),
            }],
        }
    }

    #[test]
    fn snapshot_round_trip_through_serde() {
        let snap = sample_snapshot();
        let bytes = snap.encode().unwrap();
        let decoded = BranchSnapshotData::decode(&bytes).unwrap();
        assert_eq!(decoded, snap);
    }

    #[test]
    fn empty_payload_decodes_to_default() {
        let decoded = BranchSnapshotData::decode(&[]).unwrap();
        assert_eq!(decoded, BranchSnapshotData::default());
    }

    #[test]
    fn fact_count_aggregates_all_kinds() {
        let snap = sample_snapshot();
        assert_eq!(snap.fact_count(), 4);
    }

    #[test]
    fn deterministic_encoding() {
        let snap = sample_snapshot();
        let a = snap.encode().unwrap();
        let b = snap.encode().unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn load_from_store_reads_typed_facts_via_partition() {
        let dir = TempDir::new().unwrap();
        let store = MnemosyneStore::open(dir.path()).unwrap();
        let typed = TypedFactStore::new(&store);

        let s = SectionFact {
            key: FactKey {
                branch_id: 1,
                entity_id: 39,
                valid_from: 100,
            },
            section_id: "39".into(),
            skeleton: SectionSkeleton {
                title: "graph_schema".into(),
                parent_doc: "docs/DESIGN.md".into(),
                parent_section: None,
                decision_status: Some(DecisionStatus::Active),
            },
        };
        typed.put_section(&s).unwrap();
        let cr = CrossRefFact {
            branch_id: 1,
            from_section: 66,
            to_section: 39,
            ref_kind: "decision".into(),
        };
        typed.put_cross_ref(&cr).unwrap();

        let partition = BranchEntityPartition {
            sections: vec![(39, 100)],
            cross_refs: vec![(66, 39)],
            ..Default::default()
        };
        let snap = BranchSnapshotData::load_from_store(&store, 1, &partition).unwrap();
        assert_eq!(snap.sections.len(), 1);
        assert_eq!(snap.sections[0], s);
        assert_eq!(snap.cross_refs.len(), 1);
        assert_eq!(snap.cross_refs[0], cr);
    }

    #[test]
    fn load_from_store_isolates_branches() {
        let dir = TempDir::new().unwrap();
        let store = MnemosyneStore::open(dir.path()).unwrap();
        let typed = TypedFactStore::new(&store);
        for branch_id in &[1u64, 2] {
            typed
                .put_section(&SectionFact {
                    key: FactKey {
                        branch_id: *branch_id,
                        entity_id: 39,
                        valid_from: 100,
                    },
                    section_id: format!("{}", branch_id),
                    skeleton: SectionSkeleton {
                        title: "x".into(),
                        parent_doc: "docs/DESIGN.md".into(),
                        parent_section: None,
                        decision_status: Some(DecisionStatus::Active),
                    },
                })
                .unwrap();
        }
        let partition = BranchEntityPartition {
            sections: vec![(39, 100)],
            ..Default::default()
        };
        let snap1 = BranchSnapshotData::load_from_store(&store, 1, &partition).unwrap();
        let snap2 = BranchSnapshotData::load_from_store(&store, 2, &partition).unwrap();
        assert_eq!(snap1.sections.len(), 1);
        assert_eq!(snap2.sections.len(), 1);
        assert_eq!(snap1.sections[0].section_id, "1");
        assert_eq!(snap2.sections[0].section_id, "2");
    }

    #[test]
    fn missing_entries_in_partition_are_skipped() {
        let dir = TempDir::new().unwrap();
        let store = MnemosyneStore::open(dir.path()).unwrap();
        let partition = BranchEntityPartition {
            sections: vec![(99, 100)],
            ..Default::default()
        };
        let snap = BranchSnapshotData::load_from_store(&store, 1, &partition).unwrap();
        assert_eq!(snap.sections.len(), 0);
    }
}
