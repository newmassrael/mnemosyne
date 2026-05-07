//! Phase -1A stage 2D sled sanity check (DESIGN.md §18 line 1851).
//!
//! Backend-native sled implementation that mirrors the direct-impl schema and
//! SLA harness: 24 B fixed-width composite key (`branch_id || entity_id ||
//! valid_from`, big-endian) + 4 named trees (`entities` / `branch_meta` /
//! `assets` / `asset_refs`) corresponding 1:1 to the direct-impl column
//! families. The §12 abort-rate measurement is reframed as
//! "concurrent-writer throughput" because sled's CAS semantics differ from
//! RocksDB's OptimisticTransactionDB but produce comparable commits/s.
//!
//! Toy-grade outside this sanity check; the canonical L1 backend remains
//! direct-impl on RocksDB per §18 P5 Decision gate (Round 29).

use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use byteorder::{BigEndian, ByteOrder};
use serde::{Deserialize, Serialize};
use sled::{Db, IVec, Tree};

pub mod concurrent;
pub use concurrent::{run_throughput, ConcurrentStore, ThroughputReport, WriterStats};

pub const KEY_LEN: usize = 24;
pub const ASSET_KEY_LEN: usize = 16;
pub const ASSET_REF_KEY_LEN: usize = 16;

pub const TREE_ENTITIES: &str = "entities";
pub const TREE_BRANCH_META: &str = "branch_meta";
pub const TREE_ASSETS: &str = "assets";
pub const TREE_ASSET_REFS: &str = "asset_refs";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompositeKey {
 pub branch_id: u64,
 pub entity_id: u64,
 pub valid_from: u64,
}

impl CompositeKey {
 pub fn encode(self) -> [u8; KEY_LEN] {
 let mut buf = [0u8; KEY_LEN];
 BigEndian::write_u64(&mut buf[0..8], self.branch_id);
 BigEndian::write_u64(&mut buf[8..16], self.entity_id);
 BigEndian::write_u64(&mut buf[16..24], self.valid_from);
 buf
 }

 pub fn decode(buf: &[u8]) -> Result<Self> {
 if buf.len() != KEY_LEN {
 return Err(anyhow!("composite key wrong length: {} != {}", buf.len(), KEY_LEN));
 }
 Ok(CompositeKey {
 branch_id: BigEndian::read_u64(&buf[0..8]),
 entity_id: BigEndian::read_u64(&buf[8..16]),
 valid_from: BigEndian::read_u64(&buf[16..24]),
 })
 }

 pub fn entity_prefix(branch_id: u64, entity_id: u64) -> [u8; 16] {
 let mut buf = [0u8; 16];
 BigEndian::write_u64(&mut buf[0..8], branch_id);
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssetRow {
 pub content_hash: [u8; 32],
}

pub fn asset_key(branch_id: u64, asset_id: u64) -> [u8; ASSET_KEY_LEN] {
 let mut buf = [0u8; ASSET_KEY_LEN];
 BigEndian::write_u64(&mut buf[0..8], branch_id);
 BigEndian::write_u64(&mut buf[8..16], asset_id);
 buf
}

pub fn asset_ref_key(asset_id: u64, fact_id: u64) -> [u8; ASSET_REF_KEY_LEN] {
 let mut buf = [0u8; ASSET_REF_KEY_LEN];
 BigEndian::write_u64(&mut buf[0..8], asset_id);
 BigEndian::write_u64(&mut buf[8..16], fact_id);
 buf
}

pub struct BranchStore {
 pub db: Arc<Db>,
 pub entities: Tree,
 pub branch_meta: Tree,
 pub assets: Tree,
 pub asset_refs: Tree,
}

impl BranchStore {
 /// Open a sled DB at `path` with a fixed-size in-memory cache. The cache
 /// figure mirrors direct-impl's RocksDB block cache so cache-warm vs cold
 /// behaviour stays comparable across backends.
 pub fn open(path: &Path, cache_mib: usize) -> Result<Self> {
 let db = sled::Config::default()
 .path(path)
 .cache_capacity((cache_mib as u64) * 1024 * 1024)
 .mode(sled::Mode::HighThroughput)
 .open()
 .with_context(|| format!("open sled db at {}", path.display()))?;
 let entities = db.open_tree(TREE_ENTITIES)?;
 let branch_meta = db.open_tree(TREE_BRANCH_META)?;
 let assets = db.open_tree(TREE_ASSETS)?;
 let asset_refs = db.open_tree(TREE_ASSET_REFS)?;
 Ok(BranchStore {
 db: Arc::new(db),
 entities,
 branch_meta,
 assets,
 asset_refs,
 })
 }

 pub fn put_branch(&self, branch_id: u64, meta: &BranchMeta) -> Result<()> {
 let mut key = [0u8; 8];
 BigEndian::write_u64(&mut key, branch_id);
 let val = bincode::serialize(meta)?;
 self.branch_meta.insert(key, val)?;
 Ok(())
 }

 pub fn get_branch(&self, branch_id: u64) -> Result<Option<BranchMeta>> {
 let mut key = [0u8; 8];
 BigEndian::write_u64(&mut key, branch_id);
 match self.branch_meta.get(key)? {
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
 let key = CompositeKey { branch_id, entity_id, valid_from }.encode();
 let val = bincode::serialize(record)?;
 self.entities.insert(key, val)?;
 Ok(())
 }

 pub fn write_entities_batch(
 &self,
 entries: impl IntoIterator<Item = (CompositeKey, EntityRecord)>,
 ) -> Result<()> {
 let mut batch = sled::Batch::default();
 for (k, rec) in entries {
 batch.insert(&k.encode()[..], bincode::serialize(&rec)?);
 }
 self.entities.apply_batch(batch)?;
 Ok(())
 }

 /// Walk the branch chain root-ward; for each branch on the chain, find the
 /// most recent entry for `entity_id` with `valid_from ≤ time`. sled's
 /// `range(..=upper).next_back()` mirrors RocksDB's reverse-from-key
 /// iterator.
 pub fn point_query(
 &self,
 branch_id: u64,
 entity_id: u64,
 time: u64,
 ) -> Result<Option<EntityRecord>> {
 let mut current = Some(branch_id);
 while let Some(b) = current {
 let upper = CompositeKey { branch_id: b, entity_id, valid_from: time }.encode();
 let prefix = CompositeKey::entity_prefix(b, entity_id);
 let lower = CompositeKey { branch_id: b, entity_id, valid_from: 0 }.encode();
 let mut iter = self.entities.range::<&[u8], _>(&lower[..]..=&upper[..]);
 if let Some(item) = iter.next_back() {
  let (k, v) = item?;
  if k.len() == KEY_LEN && &k[..16] == &prefix[..] {
  let key = CompositeKey::decode(&k)?;
  if key.valid_from <= time {
  let rec: EntityRecord = bincode::deserialize(&v)?;
  return Ok(Some(rec));
  }
  }
 }
 let parent_meta = self.get_branch(b)?;
 current = parent_meta.and_then(|m| m.parent);
 }
 Ok(None)
 }

 pub fn chain_depth(&self, branch_id: u64) -> Result<u32> {
 match self.get_branch(branch_id)? {
 Some(meta) => Ok(meta.depth),
 None => Err(anyhow!("branch {} not found", branch_id)),
 }
 }

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
 let mut batch = sled::Batch::default();
 let mut written = 0usize;
 for &eid in entity_ids {
 if let Some(rec) = self.point_query(source_branch, eid, time)? {
  let key = CompositeKey { branch_id: new_branch_id, entity_id: eid, valid_from: time }
  .encode();
  batch.insert(&key[..], bincode::serialize(&rec)?);
  written += 1;
  if written.is_multiple_of(8192) {
  let staged = std::mem::take(&mut batch);
  self.entities.apply_batch(staged)?;
  }
 }
 }
 self.entities.apply_batch(batch)?;
 Ok(())
 }

 /// Best-effort total on-disk size for §18 storage growth measurement.
 pub fn size_on_disk(&self) -> Result<u64> {
 Ok(self.db.size_on_disk()?)
 }

 pub fn flush(&self) -> Result<()> {
 self.db.flush()?;
 Ok(())
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

pub fn populate_root(store: &BranchStore, workload: &Workload) -> Result<()> {
 store.put_branch(
 0,
 &BranchMeta { parent: None, depth: 0, label: "root".to_string() },
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
 let mut batch = sled::Batch::default();
 let mut written = 0usize;
 for e in &workload.entities {
 let key = CompositeKey { branch_id: 0, entity_id: e.id, valid_from: 0 }.encode();
 let rec = EntityRecord { kind: e.kind as u8, name: e.name.clone() };
 batch.insert(&key[..], bincode::serialize(&rec)?);
 written += 1;
 if written.is_multiple_of(8192) {
 let staged = std::mem::take(&mut batch);
 store.entities.apply_batch(staged)?;
 }
 }
 store.entities.apply_batch(batch)?;
 Ok(())
}

pub fn apply_branch_overrides(
 store: &BranchStore,
 workload: &Workload,
 override_pct: f64,
) -> Result<usize> {
 let threshold = (override_pct.clamp(0.0, 1.0) * (u64::MAX as f64)) as u64;
 let mut total_writes = 0usize;
 for b in &workload.branches {
 if b.id == 0 {
 continue;
 }
 let mut batch = sled::Batch::default();
 for e in &workload.entities {
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
 batch.insert(&key[..], bincode::serialize(&rec)?);
 total_writes += 1;
 }
 store.entities.apply_batch(batch)?;
 }
 Ok(total_writes)
}

fn mix(a: u64, b: u64) -> u64 {
 let mut x = a.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ b;
 x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
 x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
 x ^ (x >> 31)
}

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

pub fn populate_assets(store: &BranchStore, workload: &Workload) -> Result<(usize, usize)> {
 let mut asset_batch = sled::Batch::default();
 let mut ref_batch = sled::Batch::default();
 let mut asset_count = 0usize;
 let mut ref_count = 0usize;
 for a in &workload.assets {
 let key = asset_key(a.branch_id, a.id);
 let row = AssetRow { content_hash: a.content_hash };
 asset_batch.insert(&key[..], bincode::serialize(&row)?);
 asset_count += 1;
 for &fact_id in &a.facts_referenced {
 let rk = asset_ref_key(a.id, fact_id);
 ref_batch.insert(&rk[..], &[][..]);
 ref_count += 1;
 }
 if asset_count.is_multiple_of(8192) {
 let staged = std::mem::take(&mut asset_batch);
 store.assets.apply_batch(staged)?;
 let staged_refs = std::mem::take(&mut ref_batch);
 store.asset_refs.apply_batch(staged_refs)?;
 }
 }
 store.assets.apply_batch(asset_batch)?;
 store.asset_refs.apply_batch(ref_batch)?;
 Ok((asset_count, ref_count))
}

pub fn measurement_workload_config() -> WorkloadConfig {
 workload_gen::default_config()
}

// Type alias to silence unused-import warning on IVec
type _IVecAlias = IVec;

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
 fn point_query_falls_through_to_root() {
 let (store, _dir) = open_temp();
 store
 .put_branch(0, &BranchMeta { parent: None, depth: 0, label: "root".into() })
 .unwrap();
 store
 .put_branch(1, &BranchMeta { parent: Some(0), depth: 1, label: "b1".into() })
 .unwrap();
 store
 .put_entity(0, 42, 0, &EntityRecord { kind: 1, name: "alice".into() })
 .unwrap();
 let r = store.point_query(1, 42, 100).unwrap().unwrap();
 assert_eq!(r.name, "alice");
 }

 #[test]
 fn point_query_overlay_overrides_root() {
 let (store, _dir) = open_temp();
 store
 .put_branch(0, &BranchMeta { parent: None, depth: 0, label: "root".into() })
 .unwrap();
 store
 .put_branch(1, &BranchMeta { parent: Some(0), depth: 1, label: "b1".into() })
 .unwrap();
 store
 .put_entity(0, 42, 0, &EntityRecord { kind: 1, name: "alice".into() })
 .unwrap();
 store
 .put_entity(1, 42, 10, &EntityRecord { kind: 1, name: "alice@b1".into() })
 .unwrap();
 let r = store.point_query(1, 42, 100).unwrap().unwrap();
 assert_eq!(r.name, "alice@b1");
 let r2 = store.point_query(1, 42, 5).unwrap().unwrap();
 assert_eq!(r2.name, "alice");
 }

 #[test]
 fn point_query_walks_to_chain_depth_5() {
 let (store, _dir) = open_temp();
 store
 .put_branch(0, &BranchMeta { parent: None, depth: 0, label: "root".into() })
 .unwrap();
 let leaf = synthesize_linear_chain(&store, 100, 5, "chain").unwrap();
 store
 .put_entity(0, 7, 0, &EntityRecord { kind: 2, name: "rooty".into() })
 .unwrap();
 let r = store.point_query(leaf, 7, 100).unwrap().unwrap();
 assert_eq!(r.name, "rooty");
 assert!(store.point_query(leaf, 999_999, 100).unwrap().is_none());
 }

 #[test]
 fn cross_branch_diff_detects_override() {
 let (store, _dir) = open_temp();
 store
 .put_branch(0, &BranchMeta { parent: None, depth: 0, label: "root".into() })
 .unwrap();
 store
 .put_branch(1, &BranchMeta { parent: Some(0), depth: 1, label: "b1".into() })
 .unwrap();
 store
 .put_branch(2, &BranchMeta { parent: Some(0), depth: 1, label: "b2".into() })
 .unwrap();
 store
 .put_entity(0, 42, 0, &EntityRecord { kind: 1, name: "alice".into() })
 .unwrap();
 store
 .put_entity(1, 42, 10, &EntityRecord { kind: 1, name: "alice@b1".into() })
 .unwrap();
 let diff = store.cross_branch_diff(1, 2, &[42, 99], 100).unwrap();
 assert_eq!(diff, vec![42]);
 }

 #[test]
 fn flatten_materializes_overlay() {
 let (store, _dir) = open_temp();
 store
 .put_branch(0, &BranchMeta { parent: None, depth: 0, label: "root".into() })
 .unwrap();
 let leaf = synthesize_linear_chain(&store, 100, 5, "chain").unwrap();
 store
 .put_entity(0, 7, 0, &EntityRecord { kind: 2, name: "rooty".into() })
 .unwrap();
 store.flatten_branch(leaf, &[7], 100, 9000, "flat".into()).unwrap();
 let r = store.point_query(9000, 7, 200).unwrap().unwrap();
 assert_eq!(r.name, "rooty");
 assert_eq!(store.chain_depth(9000).unwrap(), 1);
 }
}
