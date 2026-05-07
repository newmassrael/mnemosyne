//! Phase -1A stage 2C §44 row-per-encounter cascade granularity micro-bench
//! (DESIGN.md §18 line 1846).
//!
//! Two layouts for the meta-agent encounter table:
//! - **Blob**: one row per `(run_id, agent_id)`, value = bincode-encoded
//! `BTreeMap<fact_id, count>`. Append = read-modify-write of the whole map.
//! - **Normalized**: one row per `(run_id, agent_id, fact_id)`, value = count.
//! Append = single put; per-fact retraction touches one row.
//!
//! Measurements:
//! - Write throughput at the §18-scale workload (5 runs × 100 facts × 100
//! agent = 50 000 encounters).
//! - Cascade scope: how many rows the §47 run-boundary cascade touches when
//! invalidating one `(run, agent)` slice. Blob = 1 row but coarse (drop
//! everything for that slice). Normalized = N_facts rows but each is a
//! distinct invalidation handle.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use byteorder::{BigEndian, ByteOrder};
use rocksdb::{
 BlockBasedOptions, Cache, ColumnFamilyDescriptor, DBCompressionType, Direction, IteratorMode,
 Options, WriteBatch, DB,
};

pub const CF_ENC_BLOB: &str = "encounter_blob";
pub const CF_ENC_NORM: &str = "encounter_norm";

pub struct EncounterStore {
 pub db: Arc<DB>,
 _cache: Cache,
}

impl EncounterStore {
 pub fn open(path: &Path, block_cache_mib: usize) -> Result<Self> {
 let cache = Cache::new_lru_cache(block_cache_mib * 1024 * 1024);

 let mut block_opts = BlockBasedOptions::default();
 block_opts.set_block_cache(&cache);
 block_opts.set_bloom_filter(10.0, false);

 let mut cf_opts = Options::default();
 cf_opts.set_block_based_table_factory(&block_opts);
 cf_opts.set_compression_type(DBCompressionType::Lz4);

 let cfs = vec![
 ColumnFamilyDescriptor::new(CF_ENC_BLOB, cf_opts.clone()),
 ColumnFamilyDescriptor::new(CF_ENC_NORM, cf_opts),
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

 fn blob_key(run: u32, agent: u32) -> [u8; 8] {
 let mut k = [0u8; 8];
 BigEndian::write_u32(&mut k[0..4], run);
 BigEndian::write_u32(&mut k[4..8], agent);
 k
 }

 fn norm_key(run: u32, agent: u32, fact: u32) -> [u8; 12] {
 let mut k = [0u8; 12];
 BigEndian::write_u32(&mut k[0..4], run);
 BigEndian::write_u32(&mut k[4..8], agent);
 BigEndian::write_u32(&mut k[8..12], fact);
 k
 }

 fn norm_prefix(run: u32, agent: u32) -> [u8; 8] {
 Self::blob_key(run, agent)
 }

 /// Append one encounter to the blob layout (read-modify-write).
 pub fn blob_append(&self, run: u32, agent: u32, fact: u32, count: u32) -> Result<()> {
 let cf = self
 .db
 .cf_handle(CF_ENC_BLOB)
 .ok_or_else(|| anyhow!("missing CF {}", CF_ENC_BLOB))?;
 let key = Self::blob_key(run, agent);
 let mut current: BTreeMap<u32, u32> = match self.db.get_cf(cf, key)? {
 Some(buf) => bincode::deserialize(&buf)?,
 None => BTreeMap::new(),
 };
 *current.entry(fact).or_insert(0) += count;
 self.db.put_cf(cf, key, bincode::serialize(&current)?)?;
 Ok(())
 }

 /// Insert one encounter to the normalized layout (single put).
 pub fn norm_insert(&self, run: u32, agent: u32, fact: u32, count: u32) -> Result<()> {
 let cf = self
 .db
 .cf_handle(CF_ENC_NORM)
 .ok_or_else(|| anyhow!("missing CF {}", CF_ENC_NORM))?;
 let key = Self::norm_key(run, agent, fact);
 let mut val = [0u8; 4];
 BigEndian::write_u32(&mut val, count);
 self.db.put_cf(cf, key, val)?;
 Ok(())
 }

 /// Bulk-insert a slice of encounters into the normalized layout.
 pub fn norm_bulk(&self, encounters: &[(u32, u32, u32, u32)]) -> Result<()> {
 let cf = self
 .db
 .cf_handle(CF_ENC_NORM)
 .ok_or_else(|| anyhow!("missing CF {}", CF_ENC_NORM))?;
 let mut batch = WriteBatch::default();
 for &(run, agent, fact, count) in encounters {
 let key = Self::norm_key(run, agent, fact);
 let mut val = [0u8; 4];
 BigEndian::write_u32(&mut val, count);
 batch.put_cf(cf, key, val);
 }
 self.db.write(batch)?;
 Ok(())
 }

 /// Cascade scope on blob layout: 1 row dropped, but the dropped row holds
 /// the entire fact map for `(run, agent)` — coarse granularity.
 pub fn blob_invalidate_scope(&self, run: u32, agent: u32) -> Result<usize> {
 let cf = self
 .db
 .cf_handle(CF_ENC_BLOB)
 .ok_or_else(|| anyhow!("missing CF {}", CF_ENC_BLOB))?;
 let key = Self::blob_key(run, agent);
 match self.db.get_cf(cf, key)? {
 Some(_) => Ok(1),
 None => Ok(0),
 }
 }

 /// Cascade scope on normalized layout: count rows under prefix
 /// `(run, agent, *)`. Each row is an independent invalidation handle.
 pub fn norm_invalidate_scope(&self, run: u32, agent: u32) -> Result<usize> {
 let cf = self
 .db
 .cf_handle(CF_ENC_NORM)
 .ok_or_else(|| anyhow!("missing CF {}", CF_ENC_NORM))?;
 let prefix = Self::norm_prefix(run, agent);
 let mut count = 0usize;
 let iter = self
 .db
 .iterator_cf(cf, IteratorMode::From(&prefix, Direction::Forward));
 for item in iter {
 let (k, _) = item?;
 if k.len() != 12 || k[..8] != prefix {
  break;
 }
 count += 1;
 }
 Ok(count)
 }

 pub fn flush(&self) -> Result<()> {
 self.db.flush()?;
 Ok(())
 }
}

/// Generate a deterministic encounter list of size `n_runs × n_facts ×
/// n_agents`. Counts per encounter are uniform 1..=8.
pub fn generate_encounters(
 n_runs: u32,
 n_facts: u32,
 n_agents: u32,
 seed: u64,
) -> Vec<(u32, u32, u32, u32)> {
 use rand::Rng;
 use rand::SeedableRng;
 use rand_chacha::ChaCha20Rng;
 let mut rng = ChaCha20Rng::seed_from_u64(seed);
 let mut out = Vec::with_capacity((n_runs * n_facts * n_agents) as usize);
 for run in 0..n_runs {
 for agent in 0..n_agents {
 for fact in 0..n_facts {
  let count: u32 = rng.gen_range(1..=8);
  out.push((run, agent, fact, count));
 }
 }
 }
 out
}

#[cfg(test)]
mod tests {
 use super::*;
 use tempfile::TempDir;

 #[test]
 fn blob_and_norm_invalidate_scopes_differ() {
 let dir = TempDir::new().unwrap();
 let store = EncounterStore::open(dir.path(), 16).unwrap();
 let encounters = generate_encounters(2, 5, 3, 1);
 for (run, agent, fact, count) in &encounters {
 store.blob_append(*run, *agent, *fact, *count).unwrap();
 store.norm_insert(*run, *agent, *fact, *count).unwrap();
 }
 // Each (run, agent) holds 5 facts.
 for run in 0..2 {
 for agent in 0..3 {
  assert_eq!(store.blob_invalidate_scope(run, agent).unwrap(), 1);
  assert_eq!(store.norm_invalidate_scope(run, agent).unwrap(), 5);
 }
 }
 }
}
