//! `TypedFactStore` — typed put/get bridge between `mnemosyne-facts` typed facts
//! and `mnemosyne-store` raw RocksDB. Each entity kind maps to one CF:
//!
//! - `SectionFact` → `entities`
//! - `ChangelogEntryFact` → `entities` (separate entity_id namespace)
//! - `FrozenListFact` → `entities`
//! - `CrossRefFact` → `relations` (key: branch_id || from_section || to_section)
//!
//! Convention follows : all entity-shaped facts share the `entities`
//! CF, all relations share `relations`. Per-entity-kind discrimination is left
//! to the caller's `entity_id` allocation strategy (Phase 0 implementation
//! concern, not in scope here).

use crate::facts::{ChangelogEntryFact, CrossRefFact, FactCodecError, FrozenListFact, SectionFact};
use mnemosyne_store::{CfId, MnemosyneStore, StoreError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PersistError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Codec(#[from] FactCodecError),
}

pub struct TypedFactStore<'a> {
    store: &'a MnemosyneStore,
}

impl<'a> TypedFactStore<'a> {
    pub fn new(store: &'a MnemosyneStore) -> Self {
        Self { store }
    }

    pub fn put_section(&self, fact: &SectionFact) -> Result<(), PersistError> {
        self.store.put(
            CfId::Entities,
            fact.key.branch_id,
            fact.key.entity_id,
            fact.key.valid_from,
            &fact.encode_value(),
        )?;
        Ok(())
    }

    pub fn get_section(
        &self,
        branch_id: u64,
        entity_id: u64,
        valid_from: u64,
    ) -> Result<Option<SectionFact>, PersistError> {
        match self
            .store
            .get(CfId::Entities, branch_id, entity_id, valid_from)?
        {
            None => Ok(None),
            Some(bytes) => Ok(Some(SectionFact::decode_value(
                branch_id, entity_id, valid_from, &bytes,
            )?)),
        }
    }

    pub fn put_changelog_entry(&self, fact: &ChangelogEntryFact) -> Result<(), PersistError> {
        self.store.put(
            CfId::Entities,
            fact.key.branch_id,
            fact.key.entity_id,
            fact.key.valid_from,
            &fact.encode_value(),
        )?;
        Ok(())
    }

    pub fn get_changelog_entry(
        &self,
        branch_id: u64,
        entity_id: u64,
        valid_from: u64,
    ) -> Result<Option<ChangelogEntryFact>, PersistError> {
        match self
            .store
            .get(CfId::Entities, branch_id, entity_id, valid_from)?
        {
            None => Ok(None),
            Some(bytes) => Ok(Some(ChangelogEntryFact::decode_value(
                branch_id, entity_id, valid_from, &bytes,
            )?)),
        }
    }

    pub fn put_frozen_list(&self, fact: &FrozenListFact) -> Result<(), PersistError> {
        self.store.put(
            CfId::Entities,
            fact.key.branch_id,
            fact.key.entity_id,
            fact.key.valid_from,
            &fact.encode_value(),
        )?;
        Ok(())
    }

    pub fn get_frozen_list(
        &self,
        branch_id: u64,
        entity_id: u64,
        valid_from: u64,
    ) -> Result<Option<FrozenListFact>, PersistError> {
        match self
            .store
            .get(CfId::Entities, branch_id, entity_id, valid_from)?
        {
            None => Ok(None),
            Some(bytes) => Ok(Some(FrozenListFact::decode_value(
                branch_id, entity_id, valid_from, &bytes,
            )?)),
        }
    }

    pub fn put_cross_ref(&self, fact: &CrossRefFact) -> Result<(), PersistError> {
        // Relation key: branch_id || from_section || to_section.
        self.store.put(
            CfId::Relations,
            fact.branch_id,
            fact.from_section,
            fact.to_section,
            &fact.encode_value(),
        )?;
        Ok(())
    }

    pub fn get_cross_ref(
        &self,
        branch_id: u64,
        from_section: u64,
        to_section: u64,
    ) -> Result<Option<CrossRefFact>, PersistError> {
        match self
            .store
            .get(CfId::Relations, branch_id, from_section, to_section)?
        {
            None => Ok(None),
            Some(bytes) => Ok(Some(CrossRefFact::decode_value(
                branch_id,
                from_section,
                to_section,
                &bytes,
            )?)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mnemosyne_core::FactKey;
    use tempfile::TempDir;

    fn fresh_store() -> (TempDir, MnemosyneStore) {
        let dir = TempDir::new().unwrap();
        let store = MnemosyneStore::open(dir.path()).unwrap();
        (dir, store)
    }

    #[test]
    fn section_round_trip_through_store() {
        let (_dir, store) = fresh_store();
        let typed = TypedFactStore::new(&store);
        let fact = SectionFact {
            key: FactKey {
                branch_id: 1,
                entity_id: 42,
                valid_from: 1000,
            },
            doc_path: "docs/DESIGN.md".to_string(),
            section_id: "39".to_string(),
            title: "Phase 0".to_string(),
            decision_status: "Active".to_string(),
        };
        typed.put_section(&fact).unwrap();
        let got = typed.get_section(1, 42, 1000).unwrap();
        assert_eq!(got.as_ref(), Some(&fact));
    }

    #[test]
    fn cross_ref_uses_relations_cf() {
        let (_dir, store) = fresh_store();
        let typed = TypedFactStore::new(&store);
        let fact = CrossRefFact {
            branch_id: 1,
            from_section: 66,
            to_section: 39,
            ref_kind: "decision".to_string(),
        };
        typed.put_cross_ref(&fact).unwrap();
        let got = typed.get_cross_ref(1, 66, 39).unwrap();
        assert_eq!(got.as_ref(), Some(&fact));
        // Same key on `entities` CF must be empty (relations stay in their own CF).
        assert!(store.get(CfId::Entities, 1, 66, 39).unwrap().is_none());
    }

    #[test]
    fn missing_returns_none() {
        let (_dir, store) = fresh_store();
        let typed = TypedFactStore::new(&store);
        assert!(typed.get_section(1, 1, 1).unwrap().is_none());
        assert!(typed.get_changelog_entry(1, 1, 1).unwrap().is_none());
        assert!(typed.get_frozen_list(1, 1, 1).unwrap().is_none());
        assert!(typed.get_cross_ref(1, 1, 1).unwrap().is_none());
    }
}
