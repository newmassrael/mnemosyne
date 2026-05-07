//! Phase -1A stage 2B direct-impl prototype (DESIGN.md §18 line 1831-1839).
//!
//! Bi-temporal composite key + branch overlay (parent walk) over RocksDB.
//! Toy-grade outside the branching SLA scope per §18 line 1834: this crate
//! exists to measure six branching SLA criteria, not to be a candidate for
//! production use.
//!
//! Key encoding (§4 / §40 codec composition):
//! `branch_id (u64 BE) || entity_id (u64 BE) || valid_from (u64 BE)` — 24 B fixed-width.
//! Lexicographic order ⇒ scan within (branch, entity) prefix is time-ordered.

use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use byteorder::{BigEndian, ByteOrder};
use rocksdb::{
 BlockBasedOptions, Cache, ColumnFamilyDescriptor, DBCompressionType, Direction, IteratorMode,
 Options, ReadOptions, WriteBatch, WriteOptions, DB,
};
use serde::{Deserialize, Serialize};

pub mod concurrent;
pub use concurrent::{
 run_abort_rate, AbortRateReport, ConcurrentStore, WriterStats,
};

pub const KEY_LEN: usize = 24;
pub const BRANCH_ID_OFFSET: usize = 0;
pub const ENTITY_ID_OFFSET: usize = 8;
pub const VALID_FROM_OFFSET: usize = 16;

pub const CF_ENTITIES: &str = "entities";
pub const CF_BRANCH_META: &str = "branch_meta";
pub const CF_ASSETS: &str = "assets";
pub const CF_ASSET_REFS: &str = "asset_refs";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompositeKey {
 pub branch_id: u64,
 pub entity_id: u64,
 pub valid_from: u64,
}

impl CompositeKey {
 pub fn encode(self) -> [u8; KEY_LEN] {
 let mut buf = [0u8; KEY_LEN];
 BigEndian::write_u64(&mut buf[BRANCH_ID_OFFSET..ENTITY_ID_OFFSET], self.branch_id);
 BigEndian::write_u64(&mut buf[ENTITY_ID_OFFSET..VALID_FROM_OFFSET], self.entity_id);
 BigEndian::write_u64(&mut buf[VALID_FROM_OFFSET..KEY_LEN], self.valid_from);
 buf
 }

 pub fn decode(buf: &[u8]) -> Result<Self> {
 if buf.len() != KEY_LEN {
 return Err(anyhow!("composite key wrong length: {} != {}", buf.len(), KEY_LEN));
 }
 Ok(CompositeKey {
 branch_id: BigEndian::read_u64(&buf[BRANCH_ID_OFFSET..ENTITY_ID_OFFSET]),
 entity_id: BigEndian::read_u64(&buf[ENTITY_ID_OFFSET..VALID_FROM_OFFSET]),
 valid_from: BigEndian::read_u64(&buf[VALID_FROM_OFFSET..KEY_LEN]),
 })
 }

 /// Upper-bound key for `(branch, entity, T)` reverse scan: same prefix,
 /// `valid_from = T`. Reverse-iterate from this and stop at the first
 /// matching prefix to find the most recent entry with `valid_from ≤ T`.
 pub fn upper_for_point_query(branch_id: u64, entity_id: u64, time: u64) -> [u8; KEY_LEN] {
 CompositeKey {
 branch_id,
 entity_id,
 valid_from: time,
 }
 .encode()
 }

 /// Prefix `branch_id || entity_id` (16 B) — used for prefix-bound iteration
 /// to keep RocksDB from spilling into the next entity.
 pub fn entity_prefix(branch_id: u64, entity_id: u64) -> [u8; 16] {
 let mut buf = [0u8; 16];
 BigEndian::write_u64(&mut buf[..8], branch_id);
 BigEndian::write_u64(&mut buf[8..16], entity_id);
 buf
 }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EntityRecord {
 pub kind: u8,
 pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BranchMeta {
 pub parent: Option<u64>,
 pub depth: u32,
 pub label: String,
}

/// Asset row stored in `CF_ASSETS` keyed by `branch_id (u64 BE) || asset_id
/// (u64 BE)` (16 B). The `facts_referenced` field is *not* stored here — it
/// is normalised into `CF_ASSET_REFS` (row-per (asset, fact)). This matches
/// the §4 normalised decision validated by the schema-bench micro-bench.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssetRow {
 pub content_hash: [u8; 32],
}

pub const ASSET_KEY_LEN: usize = 16;
pub const ASSET_REF_KEY_LEN: usize = 16; // asset_id (u64 BE) || fact_id (u64 BE)

pub fn asset_key(branch_id: u64, asset_id: u64) -> [u8; ASSET_KEY_LEN] {
 let mut buf = [0u8; ASSET_KEY_LEN];
 BigEndian::write_u64(&mut buf[..8], branch_id);
 BigEndian::write_u64(&mut buf[8..16], asset_id);
 buf
}

pub fn asset_ref_key(asset_id: u64, fact_id: u64) -> [u8; ASSET_REF_KEY_LEN] {
 let mut buf = [0u8; ASSET_REF_KEY_LEN];
 BigEndian::write_u64(&mut buf[..8], asset_id);
 BigEndian::write_u64(&mut buf[8..16], fact_id);
 buf
}

pub struct BranchStore {
 pub db: Arc<DB>,
 block_cache: Cache,
}

impl BranchStore {
 pub fn open(path: &Path, block_cache_mib: usize) -> Result<Self> {
 let cache = Cache::new_lru_cache(block_cache_mib * 1024 * 1024);

 let mut block_opts = BlockBasedOptions::default();
 block_opts.set_block_cache(&cache);
 block_opts.set_bloom_filter(10.0, false);
 block_opts.set_block_size(16 * 1024);
 block_opts.set_cache_index_and_filter_blocks(true);
 block_opts.set_pin_l0_filter_and_index_blocks_in_cache(true);

 let mut cf_opts = Options::default();
 cf_opts.set_block_based_table_factory(&block_opts);
 cf_opts.set_compression_type(DBCompressionType::Lz4);

 let cfs = vec![
 ColumnFamilyDescriptor::new(CF_ENTITIES, cf_opts.clone()),
 ColumnFamilyDescriptor::new(CF_BRANCH_META, cf_opts.clone()),
 ColumnFamilyDescriptor::new(CF_ASSETS, cf_opts.clone()),
 ColumnFamilyDescriptor::new(CF_ASSET_REFS, cf_opts),
 ];

 let mut db_opts = Options::default();
 db_opts.create_if_missing(true);
 db_opts.create_missing_column_families(true);
 db_opts.set_max_background_jobs(4);
 db_opts.set_bytes_per_sync(1 << 20);

 let db = DB::open_cf_descriptors(&db_opts, path, cfs)
 .with_context(|| format!("open rocksdb at {}", path.display()))?;
 Ok(BranchStore {
 db: Arc::new(db),
 block_cache: cache,
 })
 }

 pub fn block_cache(&self) -> &Cache {
 &self.block_cache
 }

 pub fn put_branch(&self, branch_id: u64, meta: &BranchMeta) -> Result<()> {
 let cf = self
 .db
 .cf_handle(CF_BRANCH_META)
 .ok_or_else(|| anyhow!("missing CF {}", CF_BRANCH_META))?;
 let mut key = [0u8; 8];
 BigEndian::write_u64(&mut key, branch_id);
 let val = bincode::serialize(meta)?;
 self.db.put_cf(cf, key, val)?;
 Ok(())
 }

 pub fn get_branch(&self, branch_id: u64) -> Result<Option<BranchMeta>> {
 let cf = self
 .db
 .cf_handle(CF_BRANCH_META)
 .ok_or_else(|| anyhow!("missing CF {}", CF_BRANCH_META))?;
 let mut key = [0u8; 8];
 BigEndian::write_u64(&mut key, branch_id);
 match self.db.get_cf(cf, key)? {
 Some(buf) => Ok(Some(bincode::deserialize(&buf)?)),
 None => Ok(None),
 }
 }

 pub fn put_entity(
 &self,
 branch_id: u64,
 entity_id: u64,
 valid_from: u64,
 record: &EntityRecord,
 ) -> Result<()> {
 let cf = self
 .db
 .cf_handle(CF_ENTITIES)
 .ok_or_else(|| anyhow!("missing CF {}", CF_ENTITIES))?;
 let key = CompositeKey {
 branch_id,
 entity_id,
 valid_from,
 }
 .encode();
 let val = bincode::serialize(record)?;
 self.db.put_cf(cf, key, val)?;
 Ok(())
 }

 pub fn write_entities_batch(
 &self,
 entries: impl IntoIterator<Item = (CompositeKey, EntityRecord)>,
 ) -> Result<()> {
 let cf = self
 .db
 .cf_handle(CF_ENTITIES)
 .ok_or_else(|| anyhow!("missing CF {}", CF_ENTITIES))?;
 let mut batch = WriteBatch::default();
 for (key, rec) in entries {
 let buf = bincode::serialize(&rec)?;
 batch.put_cf(cf, key.encode(), buf);
 }
 let mut opts = WriteOptions::default();
 opts.set_sync(false);
 self.db.write_opt(batch, &opts)?;
 Ok(())
 }

 /// Walk the branch chain from `branch_id` to the root and return the most
 /// recent entry for `entity_id` with `valid_from ≤ time` found in any
 /// branch on the chain. Returns `None` if no chain branch has a matching
 /// entry.
 pub fn point_query(
 &self,
 branch_id: u64,
 entity_id: u64,
 time: u64,
 ) -> Result<Option<EntityRecord>> {
 let entities_cf = self
 .db
 .cf_handle(CF_ENTITIES)
 .ok_or_else(|| anyhow!("missing CF {}", CF_ENTITIES))?;

 let mut current = Some(branch_id);
 while let Some(b) = current {
 let upper = CompositeKey::upper_for_point_query(b, entity_id, time);
 let prefix = CompositeKey::entity_prefix(b, entity_id);
 let mut iter = self.db.iterator_cf(
  entities_cf,
  IteratorMode::From(&upper, Direction::Reverse),
 );
 if let Some(item) = iter.next() {
  let (k, v) = item?;
  if k.len() == KEY_LEN && k[..16] == prefix {
  let key = CompositeKey::decode(&k)?;
  if key.valid_from <= time {
  let rec: EntityRecord = bincode::deserialize(&v)?;
  return Ok(Some(rec));
  }
  }
 }
 // Fall through to parent.
 let parent_meta = self.get_branch(b)?;
 current = parent_meta.and_then(|m| m.parent);
 }
 Ok(None)
 }

 /// Return depth of the chain from `branch_id` to the root (root depth = 0).
 pub fn chain_depth(&self, branch_id: u64) -> Result<u32> {
 match self.get_branch(branch_id)? {
 Some(meta) => Ok(meta.depth),
 None => Err(anyhow!("branch {} not found", branch_id)),
 }
 }

 /// Cross-branch diff: snapshot of `branch_a` and `branch_b` at `time` for
 /// every entity in `entity_ids`; return the set of entity ids whose
 /// resolved record differs (by serialized bytes) between the two branches.
 pub fn cross_branch_diff(
 &self,
 branch_a: u64,
 branch_b: u64,
 entity_ids: &[u64],
 time: u64,
 ) -> Result<Vec<u64>> {
 let mut diff = Vec::new();
 for &eid in entity_ids {
 let a = self.point_query(branch_a, eid, time)?;
 let b = self.point_query(branch_b, eid, time)?;
 if !records_eq(&a, &b) {
  diff.push(eid);
 }
 }
 Ok(diff)
 }

 /// Drop the block cache (best-effort cache cold simulation). RocksDB
 /// internally references the same `Cache`; constructing a fresh cache
 /// requires reopening the DB. For cache-cold measurement, prefer accessing
 /// previously-untouched keys instead of relying on this.
 pub fn block_cache_size_bytes(&self) -> usize {
 self.block_cache.get_usage()
 }

 /// Flatten a branch into the root by materialising every entity reachable
 /// via overlay walk into a fresh branch with parent = root. The original
 /// branch is left intact; the returned id is the new flat branch.
 pub fn flatten_branch(
 &self,
 source_branch: u64,
 entity_ids: &[u64],
 time: u64,
 new_branch_id: u64,
 new_label: String,
 ) -> Result<()> {
 self.put_branch(
 new_branch_id,
 &BranchMeta {
  parent: Some(0),
  depth: 1,
  label: new_label,
 },
 )?;
 let entities_cf = self
 .db
 .cf_handle(CF_ENTITIES)
 .ok_or_else(|| anyhow!("missing CF {}", CF_ENTITIES))?;
 let mut batch = WriteBatch::default();
 let mut written = 0usize;
 for &eid in entity_ids {
 if let Some(rec) = self.point_query(source_branch, eid, time)? {
  let key = CompositeKey {
  branch_id: new_branch_id,
  entity_id: eid,
  valid_from: time,
  }
  .encode();
  batch.put_cf(entities_cf, key, bincode::serialize(&rec)?);
  written += 1;
  if written.is_multiple_of(8192) {
  let staged = std::mem::take(&mut batch);
  self.db.write(staged)?;
  }
 }
 }
 if !batch.is_empty() {
 self.db.write(batch)?;
 }
 Ok(())
 }

 /// Emit a fresh `ReadOptions` with prefix bound for entity prefix scan.
 /// Useful for scenarios that want to bound an iterator within one entity.
 pub fn entity_read_opts(_branch_id: u64, _entity_id: u64) -> ReadOptions {
 // Prefix extractor is configured on the column family, not at read
 // time; this stub exists for future tuning of iterator bounds.
 ReadOptions::default()
 }
}

fn records_eq(a: &Option<EntityRecord>, b: &Option<EntityRecord>) -> bool {
 match (a, b) {
 (None, None) => true,
 (Some(x), Some(y)) => x.kind == y.kind && x.name == y.name,
 _ => false,
 }
}

// ─── Workload population ─────────────────────────────────────────────────────

use workload_gen::{Workload, WorkloadConfig};

/// Populate root branch (id 0) with every entity and every non-root branch
/// metadata. Per-branch entity overrides (overlay deltas) are added by
/// [`apply_branch_overrides`] for measurement scenarios that need to exercise
/// chain walks.
pub fn populate_root(store: &BranchStore, workload: &Workload) -> Result<()> {
 store.put_branch(
 0,
 &BranchMeta {
 parent: None,
 depth: 0,
 label: "root".to_string(),
 },
 )?;
 for b in &workload.branches {
 if b.id == 0 {
 continue;
 }
 store.put_branch(
 b.id,
 &BranchMeta {
  parent: b.parent,
  depth: b.depth,
  label: format!("branch_{}", b.id),
 },
 )?;
 }
 let cf = store
 .db
 .cf_handle(CF_ENTITIES)
 .ok_or_else(|| anyhow!("missing CF {}", CF_ENTITIES))?;
 let mut batch = WriteBatch::default();
 let mut written = 0usize;
 for e in &workload.entities {
 let key = CompositeKey {
 branch_id: 0,
 entity_id: e.id,
 valid_from: 0,
 }
 .encode();
 let rec = EntityRecord {
 kind: e.kind as u8,
 name: e.name.clone(),
 };
 batch.put_cf(cf, key, bincode::serialize(&rec)?);
 written += 1;
 if written.is_multiple_of(8192) {
 let staged = std::mem::take(&mut batch);
 store.db.write(staged)?;
 }
 }
 if !batch.is_empty() {
 store.db.write(batch)?;
 }
 Ok(())
}

/// For every non-root branch, override a sample of entities (~`override_pct`
/// percent) with a synthetic name change. This produces overlay deltas so that
/// chain walks actually exercise the parent-walk path.
pub fn apply_branch_overrides(
 store: &BranchStore,
 workload: &Workload,
 override_pct: f64,
) -> Result<usize> {
 let cf = store
 .db
 .cf_handle(CF_ENTITIES)
 .ok_or_else(|| anyhow!("missing CF {}", CF_ENTITIES))?;
 let threshold = (override_pct.clamp(0.0, 1.0) * (u64::MAX as f64)) as u64;
 let mut total_writes = 0usize;
 for b in &workload.branches {
 if b.id == 0 {
 continue;
 }
 let mut batch = WriteBatch::default();
 for e in &workload.entities {
 // Deterministic per-(branch, entity) decision via splitmix-style hash.
 if mix(b.id, e.id) >= threshold {
  continue;
 }
 let key = CompositeKey {
  branch_id: b.id,
  entity_id: e.id,
  valid_from: (b.depth as u64) * 1000,
 }
 .encode();
 let rec = EntityRecord {
  kind: e.kind as u8,
  name: format!("{}@b{}", e.name, b.id),
 };
 batch.put_cf(cf, key, bincode::serialize(&rec)?);
 total_writes += 1;
 }
 if !batch.is_empty() {
 store.db.write(batch)?;
 }
 }
 Ok(total_writes)
}

fn mix(a: u64, b: u64) -> u64 {
 let mut x = a.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ b;
 x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
 x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
 x ^ (x >> 31)
}

/// Construct a synthetic linear chain of `depth` branches anchored at root,
/// returning the leaf branch id. Used for chain-depth-specific point query
/// measurement (workload's natural max depth is ≈ 28).
pub fn synthesize_linear_chain(
 store: &BranchStore,
 base_branch_id: u64,
 depth: u32,
 label_prefix: &str,
) -> Result<u64> {
 let mut parent = 0u64;
 let mut last = 0u64;
 for i in 0..depth {
 let id = base_branch_id + i as u64;
 let meta = BranchMeta {
 parent: Some(parent),
 depth: i + 1,
 label: format!("{}_{:02}", label_prefix, i + 1),
 };
 store.put_branch(id, &meta)?;
 parent = id;
 last = id;
 }
 Ok(last)
}

#[derive(Clone, Debug)]
pub struct PopulationStats {
 pub branches: usize,
 pub root_entities: usize,
 pub overlay_writes: usize,
 pub asset_rows: usize,
 pub asset_ref_rows: usize,
}

pub fn populate_full(
 store: &BranchStore,
 workload: &Workload,
 overlay_pct: f64,
) -> Result<PopulationStats> {
 populate_root(store, workload)?;
 let overlay_writes = apply_branch_overrides(store, workload, overlay_pct)?;
 let (asset_rows, asset_ref_rows) = populate_assets(store, workload)?;
 Ok(PopulationStats {
 branches: workload.branches.len(),
 root_entities: workload.entities.len(),
 overlay_writes,
 asset_rows,
 asset_ref_rows,
 })
}

/// Populate `CF_ASSETS` (row per (branch, asset)) and `CF_ASSET_REFS`
/// (normalised row-per (asset, fact)). The split matches the §4 normalised
/// schema decision validated by the schema-bench micro-bench: hot-fact
/// reads scan a 16 B prefix without touching the asset blob.
pub fn populate_assets(store: &BranchStore, workload: &Workload) -> Result<(usize, usize)> {
 let assets_cf = store
 .db
 .cf_handle(CF_ASSETS)
 .ok_or_else(|| anyhow!("missing CF {}", CF_ASSETS))?;
 let refs_cf = store
 .db
 .cf_handle(CF_ASSET_REFS)
 .ok_or_else(|| anyhow!("missing CF {}", CF_ASSET_REFS))?;

 let mut asset_batch = WriteBatch::default();
 let mut asset_count = 0usize;
 let mut ref_count = 0usize;
 for a in &workload.assets {
 let key = asset_key(a.branch_id, a.id);
 let row = AssetRow { content_hash: a.content_hash };
 asset_batch.put_cf(assets_cf, key, bincode::serialize(&row)?);
 asset_count += 1;
 for &fact_id in &a.facts_referenced {
 let rk = asset_ref_key(a.id, fact_id);
 asset_batch.put_cf(refs_cf, rk, []);
 ref_count += 1;
 }
 if asset_count.is_multiple_of(8192) {
 let staged = std::mem::take(&mut asset_batch);
 store.db.write(staged)?;
 }
 }
 if !asset_batch.is_empty() {
 store.db.write(asset_batch)?;
 }
 Ok((asset_count, ref_count))
}

/// Default workload config used by the measurement CLI / benches. Mirrors
/// `workload_gen::default_config()` so that all stages share the same fixture.
pub fn measurement_workload_config() -> WorkloadConfig {
 workload_gen::default_config()
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
 use super::*;
 use tempfile::TempDir;

 fn open_temp() -> (BranchStore, TempDir) {
 let dir = TempDir::new().unwrap();
 let store = BranchStore::open(dir.path(), 16).unwrap();
 (store, dir)
 }

 #[test]
 fn key_roundtrip() {
 let k = CompositeKey {
 branch_id: 0xDEAD_BEEF,
 entity_id: 0x1234_5678,
 valid_from: 0xABCD,
 };
 let buf = k.encode();
 let back = CompositeKey::decode(&buf).unwrap();
 assert_eq!(k, back);
 }

 #[test]
 fn key_orders_lexicographically() {
 let a = CompositeKey {
 branch_id: 1,
 entity_id: 1,
 valid_from: 5,
 }
 .encode();
 let b = CompositeKey {
 branch_id: 1,
 entity_id: 1,
 valid_from: 6,
 }
 .encode();
 assert!(a < b);
 let c = CompositeKey {
 branch_id: 1,
 entity_id: 2,
 valid_from: 0,
 }
 .encode();
 assert!(b < c);
 }

 #[test]
 fn point_query_falls_through_to_root() {
 let (store, _dir) = open_temp();
 store
 .put_branch(
  0,
  &BranchMeta {
  parent: None,
  depth: 0,
  label: "root".into(),
  },
 )
 .unwrap();
 store
 .put_branch(
  1,
  &BranchMeta {
  parent: Some(0),
  depth: 1,
  label: "b1".into(),
  },
 )
 .unwrap();
 store
 .put_entity(
  0,
  42,
  0,
  &EntityRecord {
  kind: 1,
  name: "alice".into(),
  },
 )
 .unwrap();
 // Branch 1 has nothing for entity 42 → fall through to root.
 let r = store.point_query(1, 42, 100).unwrap().unwrap();
 assert_eq!(r.name, "alice");
 }

 #[test]
 fn point_query_overlay_overrides_root() {
 let (store, _dir) = open_temp();
 store
 .put_branch(
  0,
  &BranchMeta {
  parent: None,
  depth: 0,
  label: "root".into(),
  },
 )
 .unwrap();
 store
 .put_branch(
  1,
  &BranchMeta {
  parent: Some(0),
  depth: 1,
  label: "b1".into(),
  },
 )
 .unwrap();
 store
 .put_entity(
  0,
  42,
  0,
  &EntityRecord {
  kind: 1,
  name: "alice".into(),
  },
 )
 .unwrap();
 store
 .put_entity(
  1,
  42,
  10,
  &EntityRecord {
  kind: 1,
  name: "alice@b1".into(),
  },
 )
 .unwrap();
 let r = store.point_query(1, 42, 100).unwrap().unwrap();
 assert_eq!(r.name, "alice@b1");
 // At time 5, b1 has nothing yet → fall through to root.
 let r2 = store.point_query(1, 42, 5).unwrap().unwrap();
 assert_eq!(r2.name, "alice");
 }

 #[test]
 fn point_query_walks_to_chain_depth_5() {
 let (store, _dir) = open_temp();
 store
 .put_branch(
  0,
  &BranchMeta {
  parent: None,
  depth: 0,
  label: "root".into(),
  },
 )
 .unwrap();
 let leaf = synthesize_linear_chain(&store, 100, 5, "chain").unwrap();
 store
 .put_entity(
  0,
  7,
  0,
  &EntityRecord {
  kind: 2,
  name: "rooty".into(),
  },
 )
 .unwrap();
 let r = store.point_query(leaf, 7, 100).unwrap().unwrap();
 assert_eq!(r.name, "rooty");
 // No entry for unknown entity.
 assert!(store.point_query(leaf, 999_999, 100).unwrap().is_none());
 }

 #[test]
 fn cross_branch_diff_detects_override() {
 let (store, _dir) = open_temp();
 store
 .put_branch(
  0,
  &BranchMeta {
  parent: None,
  depth: 0,
  label: "root".into(),
  },
 )
 .unwrap();
 store
 .put_branch(
  1,
  &BranchMeta {
  parent: Some(0),
  depth: 1,
  label: "b1".into(),
  },
 )
 .unwrap();
 store
 .put_branch(
  2,
  &BranchMeta {
  parent: Some(0),
  depth: 1,
  label: "b2".into(),
  },
 )
 .unwrap();
 store
 .put_entity(
  0,
  42,
  0,
  &EntityRecord {
  kind: 1,
  name: "alice".into(),
  },
 )
 .unwrap();
 store
 .put_entity(
  1,
  42,
  10,
  &EntityRecord {
  kind: 1,
  name: "alice@b1".into(),
  },
 )
 .unwrap();
 let diff = store.cross_branch_diff(1, 2, &[42, 99], 100).unwrap();
 assert_eq!(diff, vec![42]);
 }

 #[test]
 fn flatten_materializes_overlay() {
 let (store, _dir) = open_temp();
 store
 .put_branch(
  0,
  &BranchMeta {
  parent: None,
  depth: 0,
  label: "root".into(),
  },
 )
 .unwrap();
 let leaf = synthesize_linear_chain(&store, 100, 5, "chain").unwrap();
 store
 .put_entity(
  0,
  7,
  0,
  &EntityRecord {
  kind: 2,
  name: "rooty".into(),
  },
 )
 .unwrap();
 store.flatten_branch(leaf, &[7], 100, 9000, "flat".into()).unwrap();
 let r = store.point_query(9000, 7, 200).unwrap().unwrap();
 assert_eq!(r.name, "rooty");
 // Flattened branch has depth 1 (parent = root).
 assert_eq!(store.chain_depth(9000).unwrap(), 1);
 }
}
