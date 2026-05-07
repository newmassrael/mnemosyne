//! Phase -1A stage 2C §11 provenance marker overhead micro-bench
//! (DESIGN.md §18 line 1847).
//!
//! Two epistemic-row layouts share one RocksDB instance over distinct CFs:
//! - `epistemic_baseline`: minimal record `(kind, name)` (current §4 schema).
//! - `epistemic_provenance`: same record extended with `provenance_kind` enum
//! and `derived_from: u64` (§11 closure marker fields).
//!
//! Two scenarios:
//! 1. **Point query** at random fact_id: read + decode for both layouts.
//! Overhead = (provenance p95) / (baseline p95) - 1. §18 line 1904 trigger
//! fires if overhead ≥ 10 %.
//! 2. **Cascade retraction**: scan for all rows with `derived_from = source`,
//! aggregating ids that would be retracted. The baseline layout has no such
//! field — its "retraction overhead" is informational (no cascade possible
//! without provenance markers); the provenance layout cost is the actual
//! retraction-probe latency.

use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use byteorder::{BigEndian, ByteOrder};
use rocksdb::{
 BlockBasedOptions, Cache, ColumnFamilyDescriptor, DBCompressionType, IteratorMode, Options,
 WriteBatch, DB,
};
use serde::{Deserialize, Serialize};

pub const CF_BASELINE: &str = "epistemic_baseline";
pub const CF_PROVENANCE: &str = "epistemic_provenance";

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[repr(u8)]
pub enum ProvenanceKind {
 Explicit = 0,
 Derived = 1,
 Retracted = 2,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BaselineRow {
 pub kind: u8,
 pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProvenanceRow {
 pub kind: u8,
 pub name: String,
 pub provenance: ProvenanceKind,
 pub derived_from: u64,
}

pub struct ProvenanceStore {
 pub db: Arc<DB>,
 _cache: Cache,
}

impl ProvenanceStore {
 pub fn open(path: &Path, block_cache_mib: usize) -> Result<Self> {
 let cache = Cache::new_lru_cache(block_cache_mib * 1024 * 1024);

 let mut block_opts = BlockBasedOptions::default();
 block_opts.set_block_cache(&cache);
 block_opts.set_bloom_filter(10.0, false);
 block_opts.set_block_size(16 * 1024);
 block_opts.set_cache_index_and_filter_blocks(true);

 let mut cf_opts = Options::default();
 cf_opts.set_block_based_table_factory(&block_opts);
 cf_opts.set_compression_type(DBCompressionType::Lz4);

 let cfs = vec![
 ColumnFamilyDescriptor::new(CF_BASELINE, cf_opts.clone()),
 ColumnFamilyDescriptor::new(CF_PROVENANCE, cf_opts),
 ];
 let mut db_opts = Options::default();
 db_opts.create_if_missing(true);
 db_opts.create_missing_column_families(true);
 db_opts.set_max_background_jobs(4);

 let db = DB::open_cf_descriptors(&db_opts, path, cfs)
 .with_context(|| format!("open rocksdb at {}", path.display()))?;
 Ok(Self {
 db: Arc::new(db),
 _cache: cache,
 })
 }

 pub fn put_baseline(&self, fact_id: u64, row: &BaselineRow) -> Result<()> {
 let cf = self
 .db
 .cf_handle(CF_BASELINE)
 .ok_or_else(|| anyhow!("missing CF {}", CF_BASELINE))?;
 let mut key = [0u8; 8];
 BigEndian::write_u64(&mut key, fact_id);
 self.db.put_cf(cf, key, bincode::serialize(row)?)?;
 Ok(())
 }

 pub fn put_provenance(&self, fact_id: u64, row: &ProvenanceRow) -> Result<()> {
 let cf = self
 .db
 .cf_handle(CF_PROVENANCE)
 .ok_or_else(|| anyhow!("missing CF {}", CF_PROVENANCE))?;
 let mut key = [0u8; 8];
 BigEndian::write_u64(&mut key, fact_id);
 self.db.put_cf(cf, key, bincode::serialize(row)?)?;
 Ok(())
 }

 pub fn get_baseline(&self, fact_id: u64) -> Result<Option<BaselineRow>> {
 let cf = self
 .db
 .cf_handle(CF_BASELINE)
 .ok_or_else(|| anyhow!("missing CF {}", CF_BASELINE))?;
 let mut key = [0u8; 8];
 BigEndian::write_u64(&mut key, fact_id);
 match self.db.get_cf(cf, key)? {
 Some(buf) => Ok(Some(bincode::deserialize(&buf)?)),
 None => Ok(None),
 }
 }

 pub fn get_provenance(&self, fact_id: u64) -> Result<Option<ProvenanceRow>> {
 let cf = self
 .db
 .cf_handle(CF_PROVENANCE)
 .ok_or_else(|| anyhow!("missing CF {}", CF_PROVENANCE))?;
 let mut key = [0u8; 8];
 BigEndian::write_u64(&mut key, fact_id);
 match self.db.get_cf(cf, key)? {
 Some(buf) => Ok(Some(bincode::deserialize(&buf)?)),
 None => Ok(None),
 }
 }

 /// Cascade-retraction probe: scan the provenance CF and collect every
 /// fact_id whose `derived_from` matches `source`. Linear in CF size — the
 /// pessimistic measurement. A real implementation would either keep a
 /// secondary `derived_from → derived_id` index (write overhead) or move
 /// derived rows to a separate CF (covered by the §18 line 1904 trigger
 /// fallback path).
 pub fn cascade_scan(&self, source: u64) -> Result<Vec<u64>> {
 let cf = self
 .db
 .cf_handle(CF_PROVENANCE)
 .ok_or_else(|| anyhow!("missing CF {}", CF_PROVENANCE))?;
 let mut out = Vec::new();
 let iter = self.db.iterator_cf(cf, IteratorMode::Start);
 for item in iter {
 let (k, v) = item?;
 if k.len() != 8 {
  continue;
 }
 let row: ProvenanceRow = match bincode::deserialize(&v) {
  Ok(r) => r,
  Err(_) => continue,
 };
 if row.derived_from == source {
  out.push(BigEndian::read_u64(&k));
 }
 }
 Ok(out)
 }

 pub fn flush(&self) -> Result<()> {
 self.db.flush()?;
 Ok(())
 }
}

/// Build a synthetic epistemic fixture: `n_explicit` explicit rows + each one
/// has `derived_per` derived rows pointing back. Both layouts populated.
pub fn populate_fixture(
 store: &ProvenanceStore,
 n_explicit: usize,
 derived_per: usize,
) -> Result<()> {
 let cf_b = store
 .db
 .cf_handle(CF_BASELINE)
 .ok_or_else(|| anyhow!("missing CF {}", CF_BASELINE))?;
 let cf_p = store
 .db
 .cf_handle(CF_PROVENANCE)
 .ok_or_else(|| anyhow!("missing CF {}", CF_PROVENANCE))?;
 let mut batch_b = WriteBatch::default();
 let mut batch_p = WriteBatch::default();
 let mut next_id: u64 = 1;
 for src_idx in 0..n_explicit {
 let src_id = next_id;
 next_id += 1;
 let baseline = BaselineRow {
 kind: 1,
 name: format!("src_{:06}", src_idx),
 };
 let provenance = ProvenanceRow {
 kind: 1,
 name: format!("src_{:06}", src_idx),
 provenance: ProvenanceKind::Explicit,
 derived_from: 0,
 };
 let mut k = [0u8; 8];
 BigEndian::write_u64(&mut k, src_id);
 batch_b.put_cf(cf_b, k, bincode::serialize(&baseline)?);
 batch_p.put_cf(cf_p, k, bincode::serialize(&provenance)?);

 for d in 0..derived_per {
 let did = next_id;
 next_id += 1;
 let baseline = BaselineRow {
  kind: 2,
  name: format!("der_{:06}_{}", src_idx, d),
 };
 let provenance = ProvenanceRow {
  kind: 2,
  name: format!("der_{:06}_{}", src_idx, d),
  provenance: ProvenanceKind::Derived,
  derived_from: src_id,
 };
 let mut k = [0u8; 8];
 BigEndian::write_u64(&mut k, did);
 batch_b.put_cf(cf_b, k, bincode::serialize(&baseline)?);
 batch_p.put_cf(cf_p, k, bincode::serialize(&provenance)?);
 }
 if next_id.is_multiple_of(8192) {
 store.db.write(std::mem::take(&mut batch_b))?;
 store.db.write(std::mem::take(&mut batch_p))?;
 }
 }
 if !batch_b.is_empty() {
 store.db.write(batch_b)?;
 }
 if !batch_p.is_empty() {
 store.db.write(batch_p)?;
 }
 store.flush()?;
 Ok(())
}

#[cfg(test)]
mod tests {
 use super::*;
 use tempfile::TempDir;

 #[test]
 fn baseline_and_provenance_resolve_independently() {
 let dir = TempDir::new().unwrap();
 let store = ProvenanceStore::open(dir.path(), 16).unwrap();
 store
 .put_baseline(
  1,
  &BaselineRow {
  kind: 1,
  name: "alice".into(),
  },
 )
 .unwrap();
 store
 .put_provenance(
  1,
  &ProvenanceRow {
  kind: 1,
  name: "alice".into(),
  provenance: ProvenanceKind::Explicit,
  derived_from: 0,
  },
 )
 .unwrap();
 let b = store.get_baseline(1).unwrap().unwrap();
 let p = store.get_provenance(1).unwrap().unwrap();
 assert_eq!(b.name, "alice");
 assert_eq!(p.name, "alice");
 assert_eq!(p.provenance, ProvenanceKind::Explicit);
 }

 #[test]
 fn cascade_scan_finds_derived_rows() {
 let dir = TempDir::new().unwrap();
 let store = ProvenanceStore::open(dir.path(), 16).unwrap();
 populate_fixture(&store, 5, 3).unwrap();
 // Source 1 (first explicit row) should have 3 derived ids: 2, 3, 4.
 let derived = store.cascade_scan(1).unwrap();
 assert_eq!(derived, vec![2, 3, 4]);
 }
}
