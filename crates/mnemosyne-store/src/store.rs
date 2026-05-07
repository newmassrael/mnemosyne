//! `MnemosyneStore` — `rocksdb::DB` wrapper over the §4 ten-CF schema.
//!
//! Provides per-CF put / get / iter_branch / iter_branch_entity / write_batch
//! against the 24 B BE composite key. The prototype `cf_wrapper.rs::emit_wrapper`
//! emitted these as source strings; here they bind to real RocksDB calls.

use crate::cf_layout::{cf_descriptors, CfId};
use crate::error::{Result, StoreError};
use crate::key_codec::{
 decode_composite_key, encode_branch_entity_prefix, encode_composite_key, KEY_LEN,
};
use crate::migration::MigrationMeta;
use rocksdb::{Direction, IteratorMode, Options, WriteBatch, DB};
use std::path::Path;
use std::sync::Arc;

/// Mnemosyne-store handle. Holds an `Arc<DB>` so multiple typed callers
/// (entity wrapper, relation wrapper, audit appender, ...) can share one DB
/// instance without cloning RocksDB resources.
#[derive(Clone)]
pub struct MnemosyneStore {
 db: Arc<DB>,
}

impl MnemosyneStore {
 /// Open or create the store at `path`. Creates the eleven CFs (10 user-facing
 /// + `migration_meta`) on first open, then seeds `migration_meta` with the
 /// spec-default schema versions.
 pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
 let mut db_opts = Options::default();
 db_opts.create_if_missing(true);
 db_opts.create_missing_column_families(true);

 let db = DB::open_cf_descriptors(&db_opts, path.as_ref(), cf_descriptors())?;
 let store = Self { db: Arc::new(db) };
 MigrationMeta::seed_all(&store.db)?;
 Ok(store)
 }

 /// Direct access to the underlying DB — used by typed wrapper layers
 /// (mnemosyne-core / mnemosyne-cascade) that share key encoding rules.
 pub fn db(&self) -> &Arc<DB> {
 &self.db
 }

 fn cf<'a>(&'a self, id: CfId) -> Result<&'a rocksdb::ColumnFamily> {
 self.db
 .cf_handle(id.name())
 .ok_or(StoreError::MissingCf(id.name()))
 }

 /// `cf.put(encode(branch_id, entity_id, valid_from), value)`.
 pub fn put(
 &self,
 cf: CfId,
 branch_id: u64,
 entity_id: u64,
 valid_from: u64,
 value: &[u8],
 ) -> Result<()> {
 let cf_handle = self.cf(cf)?;
 let key = encode_composite_key(branch_id, entity_id, valid_from);
 self.db.put_cf(&cf_handle, key, value)?;
 Ok(())
 }

 pub fn get(
 &self,
 cf: CfId,
 branch_id: u64,
 entity_id: u64,
 valid_from: u64,
 ) -> Result<Option<Vec<u8>>> {
 let cf_handle = self.cf(cf)?;
 let key = encode_composite_key(branch_id, entity_id, valid_from);
 Ok(self.db.get_cf(&cf_handle, key)?)
 }

 /// Prefix scan over `(branch_id, entity_id)` returning `(valid_from, value)` pairs
 /// in lex/numeric order.
 pub fn iter_branch_entity(
 &self,
 cf: CfId,
 branch_id: u64,
 entity_id: u64,
 ) -> Result<Vec<(u64, Vec<u8>)>> {
 let cf_handle = self.cf(cf)?;
 let prefix = encode_branch_entity_prefix(branch_id, entity_id);
 let iter = self
 .db
 .iterator_cf(&cf_handle, IteratorMode::From(&prefix, Direction::Forward));
 let mut out = Vec::new();
 for item in iter {
 let (k, v) = item?;
 if k.len() != KEY_LEN || k[..16] != prefix {
  break;
 }
 let (_, _, valid_from) = decode_composite_key(&k)?;
 out.push((valid_from, v.into_vec()));
 }
 Ok(out)
 }

 /// Round 113 — callback-based streaming variant. Each (valid_from, value)
 /// pair is delivered to the callback as it is decoded from RocksDB,
 /// avoiding the materialization of the whole branch+entity range into
 /// a single `Vec`. The callback returns `true` to continue or `false`
 /// to stop early; iteration ends on either.
 ///
 /// Memory profile: at most one rocksdb iterator step (key + value) is
 /// resident at a time, plus whatever the callback retains. Used by
 /// the audit subscription tail loop (`AuditAppender::iter_from_streaming`)
 /// to forward records per-record to the gRPC channel without ever
 /// holding the full audit log in memory.
 pub fn iter_branch_entity_streaming<F>(
 &self,
 cf: CfId,
 branch_id: u64,
 entity_id: u64,
 mut callback: F,
 ) -> Result<()>
 where
 F: FnMut(u64, Vec<u8>) -> bool,
 {
 let cf_handle = self.cf(cf)?;
 let prefix = encode_branch_entity_prefix(branch_id, entity_id);
 let iter = self
 .db
 .iterator_cf(&cf_handle, IteratorMode::From(&prefix, Direction::Forward));
 for item in iter {
 let (k, v) = item?;
 if k.len() != KEY_LEN || k[..16] != prefix {
  break;
 }
 let (_, _, valid_from) = decode_composite_key(&k)?;
 if !callback(valid_from, v.into_vec()) {
  break;
 }
 }
 Ok(())
 }

 /// WriteBatch atomic group — prototype `cf_wrapper.rs::write_batch` actual
 /// binding. All entries land in the same CF in one atomic write.
 pub fn write_batch(&self, cf: CfId, entries: &[(u64, u64, u64, Vec<u8>)]) -> Result<()> {
 let cf_handle = self.cf(cf)?;
 let mut batch = WriteBatch::default();
 for (b, e, v, val) in entries {
 let key = encode_composite_key(*b, *e, *v);
 batch.put_cf(&cf_handle, key, val);
 }
 self.db.write(batch)?;
 Ok(())
 }

 /// Multi-CF atomic write. Each entry: `(cf_id, branch_id, entity_id, valid_from, value)`.
 /// Used by mnemosyne-server proposal handler when one logical commit spans
 /// `entities` + `audit` + `temporal_index`.
 pub fn write_batch_multi_cf(&self, entries: &[(CfId, u64, u64, u64, Vec<u8>)]) -> Result<()> {
 let mut batch = WriteBatch::default();
 for (cf, b, e, v, val) in entries {
 let cf_handle = self.cf(*cf)?;
 let key = encode_composite_key(*b, *e, *v);
 batch.put_cf(&cf_handle, key, val);
 }
 self.db.write(batch)?;
 Ok(())
 }
}

#[cfg(test)]
mod tests {
 use super::*;
 use tempfile::TempDir;

 fn fresh_store() -> (TempDir, MnemosyneStore) {
 let dir = TempDir::new().expect("tempdir");
 let store = MnemosyneStore::open(dir.path()).expect("open store");
 (dir, store)
 }

 #[test]
 fn open_creates_eleven_cfs_and_seeds_migration_meta() {
 let (_dir, store) = fresh_store();
 // After open, migration_meta must hold spec-default schema_version=1
 // for every CF.
 for cf in [
 CfId::Entities,
 CfId::Relations,
 CfId::TemporalIndex,
 CfId::TemporalIndexOpen,
 CfId::BranchMeta,
 CfId::Assets,
 CfId::AssetRefs,
 CfId::Audit,
 CfId::Epistemic,
 CfId::Secrets,
 CfId::MigrationMeta,
 ] {
 let v = MigrationMeta::read_version(&store.db, cf).expect("read");
 assert_eq!(v, Some(1), "cf {} schema_version", cf.name());
 }
 }

 #[test]
 fn put_get_round_trip_entities_cf() {
 let (_dir, store) = fresh_store();
 store
 .put(CfId::Entities, 1, 42, 1000, b"value-payload")
 .unwrap();
 let got = store.get(CfId::Entities, 1, 42, 1000).unwrap();
 assert_eq!(got.as_deref(), Some(b"value-payload".as_ref()));
 }

 #[test]
 fn get_returns_none_for_missing_key() {
 let (_dir, store) = fresh_store();
 let got = store.get(CfId::Entities, 1, 42, 1000).unwrap();
 assert!(got.is_none());
 }

 #[test]
 fn iter_branch_entity_returns_time_ordered() {
 let (_dir, store) = fresh_store();
 // Seed three valid_from values for (branch=1, entity=42), plus an
 // unrelated entity that should not appear.
 for v in &[100u64, 200, 300] {
 store
  .put(CfId::Entities, 1, 42, *v, format!("v{}", v).as_bytes())
  .unwrap();
 }
 store.put(CfId::Entities, 1, 99, 100, b"other").unwrap();
 let scanned = store.iter_branch_entity(CfId::Entities, 1, 42).unwrap();
 assert_eq!(
 scanned,
 vec![
  (100u64, b"v100".to_vec()),
  (200u64, b"v200".to_vec()),
  (300u64, b"v300".to_vec()),
 ]
 );
 }

 #[test]
 fn iter_branch_entity_stops_at_prefix_boundary() {
 let (_dir, store) = fresh_store();
 store.put(CfId::Entities, 1, 42, 100, b"a").unwrap();
 store.put(CfId::Entities, 1, 43, 100, b"b").unwrap();
 store.put(CfId::Entities, 2, 42, 100, b"c").unwrap();
 let scanned = store.iter_branch_entity(CfId::Entities, 1, 42).unwrap();
 assert_eq!(scanned.len(), 1);
 assert_eq!(scanned[0], (100u64, b"a".to_vec()));
 }

 #[test]
 fn write_batch_atomic_group() {
 let (_dir, store) = fresh_store();
 let entries = vec![
 (1u64, 1u64, 100u64, b"a".to_vec()),
 (1, 1, 200, b"b".to_vec()),
 (1, 2, 100, b"c".to_vec()),
 ];
 store.write_batch(CfId::Relations, &entries).unwrap();
 assert_eq!(
 store.get(CfId::Relations, 1, 1, 100).unwrap().as_deref(),
 Some(b"a".as_ref())
 );
 assert_eq!(
 store.get(CfId::Relations, 1, 1, 200).unwrap().as_deref(),
 Some(b"b".as_ref())
 );
 assert_eq!(
 store.get(CfId::Relations, 1, 2, 100).unwrap().as_deref(),
 Some(b"c".as_ref())
 );
 }

 #[test]
 fn multi_cf_atomic_write() {
 let (_dir, store) = fresh_store();
 let entries = vec![
 (CfId::Entities, 1, 1, 100, b"e".to_vec()),
 (CfId::Audit, 1, 1, 100, b"a".to_vec()),
 (CfId::TemporalIndex, 1, 1, 100, b"t".to_vec()),
 ];
 store.write_batch_multi_cf(&entries).unwrap();
 assert_eq!(
 store.get(CfId::Entities, 1, 1, 100).unwrap().as_deref(),
 Some(b"e".as_ref())
 );
 assert_eq!(
 store.get(CfId::Audit, 1, 1, 100).unwrap().as_deref(),
 Some(b"a".as_ref())
 );
 assert_eq!(
 store.get(CfId::TemporalIndex, 1, 1, 100).unwrap().as_deref(),
 Some(b"t".as_ref())
 );
 }

 #[test]
 fn audit_cf_writable_but_blocked_from_secondary_metadata() {
 // Writing to audit is allowed (it's the append-only target).
 // Secondary read access is enforced by the metadata flag, not the wrapper API.
 let (_dir, store) = fresh_store();
 store.put(CfId::Audit, 1, 1, 100, b"audit-record").unwrap();
 assert_eq!(
 store.get(CfId::Audit, 1, 1, 100).unwrap().as_deref(),
 Some(b"audit-record".as_ref())
 );
 assert!(!crate::cf_layout::meta(CfId::Audit).secondary_readable);
 }

 #[test]
 fn schema_version_validate_against_spec_passes_after_seed() {
 let (_dir, store) = fresh_store();
 for cf in [CfId::Entities, CfId::Relations, CfId::Audit] {
 MigrationMeta::validate_against_spec(&store.db, cf).expect("validate");
 }
 }

 #[test]
 fn schema_version_bump_then_validate_diverges() {
 let (_dir, store) = fresh_store();
 MigrationMeta::bump_version(&store.db, CfId::Entities, 7).unwrap();
 let err = MigrationMeta::validate_against_spec(&store.db, CfId::Entities)
 .expect_err("should mismatch");
 match err {
 StoreError::SchemaVersionMismatch {
  cf,
  stored,
  expected,
 } => {
  assert_eq!(cf, "entities");
  assert_eq!(stored, 7);
  assert_eq!(expected, 1);
 }
 other => panic!("unexpected error: {other:?}"),
 }
 }
}
