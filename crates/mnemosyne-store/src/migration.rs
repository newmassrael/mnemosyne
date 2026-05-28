//! Migration metadata — per-CF schema_version tracking persisted in the
//! `migration_meta` CF.
//!
//! Key layout: `MIGRATION_META_KEY_PREFIX` (4 B `"smv:"` literal) || cf_name
//! (variable). Value: u32 BE schema_version.
//!
//! Bench prototype `bench/codegen-prototype/src/cf_wrapper.rs::emit_migration_stub`
//! emitted three helpers (`cf_versions`, `scaffold_new_cf`, `bump_version`) as a
//! source-string scaffold. The production binding actually persists those
//! versions to a CF and exposes the same operations against a live DB.

use crate::cf_layout::{CfId, ALL_CFS};
use crate::error::{Result, StoreError};
use byteorder::{BigEndian, ByteOrder};
use rocksdb::DB;

/// Key prefix for migration-meta entries (4 ASCII bytes).
pub const MIGRATION_META_KEY_PREFIX: &[u8; 4] = b"smv:";

#[derive(Debug, Clone)]
pub struct MigrationMeta;

impl MigrationMeta {
    fn key_for(cf_name: &str) -> Vec<u8> {
        let mut k = Vec::with_capacity(MIGRATION_META_KEY_PREFIX.len() + cf_name.len());
        k.extend_from_slice(MIGRATION_META_KEY_PREFIX);
        k.extend_from_slice(cf_name.as_bytes());
        k
    }

    /// Persist the spec-default `schema_version` for every registered CF.
    /// Idempotent — subsequent calls overwrite with the current spec value.
    pub fn seed_all(db: &DB) -> Result<()> {
        let cf = db
            .cf_handle(CfId::MigrationMeta.name())
            .ok_or(StoreError::MissingCf("migration_meta"))?;
        for meta in ALL_CFS {
            let key = Self::key_for(meta.name());
            let mut value = [0u8; 4];
            BigEndian::write_u32(&mut value, meta.schema_version);
            db.put_cf(&cf, &key, value)?;
        }
        Ok(())
    }

    pub fn read_version(db: &DB, cf: CfId) -> Result<Option<u32>> {
        let cf_handle = db
            .cf_handle(CfId::MigrationMeta.name())
            .ok_or(StoreError::MissingCf("migration_meta"))?;
        let key = Self::key_for(cf.name());
        match db.get_cf(&cf_handle, key)? {
            Some(bytes) if bytes.len() == 4 => Ok(Some(BigEndian::read_u32(&bytes))),
            Some(_) => Err(StoreError::KeyLength {
                expected: 4,
                got: 0,
            }),
            None => Ok(None),
        }
    }

    pub fn bump_version(db: &DB, cf: CfId, new_version: u32) -> Result<()> {
        let cf_handle = db
            .cf_handle(CfId::MigrationMeta.name())
            .ok_or(StoreError::MissingCf("migration_meta"))?;
        let key = Self::key_for(cf.name());
        let mut value = [0u8; 4];
        BigEndian::write_u32(&mut value, new_version);
        db.put_cf(&cf_handle, &key, value)?;
        Ok(())
    }

    /// Validate the persisted schema_version against the spec-default.
    /// Returns `SchemaVersionMismatch` if the persisted value diverges.
    pub fn validate_against_spec(db: &DB, cf: CfId) -> Result<()> {
        let expected = ALL_CFS
            .iter()
            .find(|m| m.id == cf)
            .expect("ALL_CFS covers every CfId")
            .schema_version;
        let stored = Self::read_version(db, cf)?.unwrap_or(0);
        if stored != expected {
            return Err(StoreError::SchemaVersionMismatch {
                cf: cf.name(),
                stored,
                expected,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_format_has_prefix_and_cf_name() {
        let k = MigrationMeta::key_for("entities");
        assert_eq!(&k[..4], MIGRATION_META_KEY_PREFIX);
        assert_eq!(&k[4..], b"entities");
    }
}
