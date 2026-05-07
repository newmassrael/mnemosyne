//! Phase -1A stage 2D LMDB sanity check (DESIGN.md §18 line 1851).
//!
//! Backend-native LMDB (via `heed` 0.20) implementation that mirrors the
//! direct-impl schema and SLA harness: 24 B fixed-width composite key
//! (`branch_id || entity_id || valid_from`, big-endian) + 4 named databases
//! (`entities` / `branch_meta` / `assets` / `asset_refs`) corresponding 1:1
//! to the direct-impl column families.
//!
//! LMDB's writer model is *single-writer-mutex* (only one `RwTxn` at a time);
//! readers are MVCC-concurrent. The §12 abort-rate measurement is reframed
//! as "concurrent-writer throughput" — LMDB writers serialise on the writer
//! mutex so the headline `commits_per_sec` exposes the queueing cost
//! comparably to the OCC backends.
//!
//! Toy-grade outside this sanity check; the canonical L1 backend remains
//! direct-impl on RocksDB per §18 P5 Decision gate (Round 29).

use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use byteorder::{BigEndian, ByteOrder};
use heed::types::Bytes;
use heed::{Database, Env, EnvOpenOptions};
use serde::{Deserialize, Serialize};

pub mod concurrent;
pub use concurrent::{run_throughput, ConcurrentStore, ThroughputReport, WriterStats};

pub const KEY_LEN: usize = 24;
pub const ASSET_KEY_LEN: usize = 16;
pub const ASSET_REF_KEY_LEN: usize = 16;

pub const DB_ENTITIES: &str = "entities";
pub const DB_BRANCH_META: &str = "branch_meta";
pub const DB_ASSETS: &str = "assets";
pub const DB_ASSET_REFS: &str = "asset_refs";

/// LMDB map size for 200K-asset workload. 4 GiB is comfortable headroom over
/// the measured RocksDB 35 MB / sled 283 MB footprints; we size the map well
/// above expected usage so the writer doesn't trigger MDB_MAP_FULL during
/// population.
pub const DEFAULT_MAP_SIZE: usize = 4 * 1024 * 1024 * 1024;

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

/// Untyped byte-keyed databases — both keys and values are arbitrary byte
/// slices (composite key is encoded outside heed).
type ByteDb = Database<Bytes, Bytes>;

pub struct BranchStore {
 pub env: Arc<Env>,
 pub entities: ByteDb,
 pub branch_meta: ByteDb,
 pub assets: ByteDb,
 pub asset_refs: ByteDb,
}

impl BranchStore {
 pub fn open(path: &Path) -> Result<Self> {
 Self::open_with_map_size(path, DEFAULT_MAP_SIZE)
 }

 pub fn open_with_map_size(path: &Path, map_size: usize) -> Result<Self> {
 std::fs::create_dir_all(path)
 .with_context(|| format!("create lmdb dir {}", path.display()))?;
 let env = unsafe {
 EnvOpenOptions::new()
  .map_size(map_size)
  .max_dbs(8)
  .open(path)
 }
 .with_context(|| format!("open lmdb at {}", path.display()))?;

 let mut wtxn = env.write_txn()?;
 let entities: ByteDb = env.create_database(&mut wtxn, Some(DB_ENTITIES))?;
 let branch_meta: ByteDb = env.create_database(&mut wtxn, Some(DB_BRANCH_META))?;
 let assets: ByteDb = env.create_database(&mut wtxn, Some(DB_ASSETS))?;
 let asset_refs: ByteDb = env.create_database(&mut wtxn, Some(DB_ASSET_REFS))?;
 wtxn.commit()?;

 Ok(BranchStore {
 env: Arc::new(env),
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
 let mut wtxn = self.env.write_txn()?;
 self.branch_meta.put(&mut wtxn, &key, &val)?;
 wtxn.commit()?;
 Ok(())
 }

 pub fn get_branch(&self, branch_id: u64) -> Result<Option<BranchMeta>> {
 let mut key = [0u8; 8];
 BigEndian::write_u64(&mut key, branch_id);
 let rtxn = self.env.read_txn()?;
 let val = self.branch_meta.get(&rtxn, &key)?;
 let result = match val {
 Some(buf) => Some(bincode::deserialize(buf)?),
 None => None,
 };
 Ok(result)
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
 let mut wtxn = self.env.write_txn()?;
 self.entities.put(&mut wtxn, &key, &val)?;
 wtxn.commit()?;
 Ok(())
 }

 pub fn write_entities_batch(
 &self,
 entries: impl IntoIterator<Item = (CompositeKey, EntityRecord)>,
 ) -> Result<()> {
 let mut wtxn = self.env.write_txn()?;
 for (k, rec) in entries {
 let key = k.encode();
 let val = bincode::serialize(&rec)?;
 self.entities.put(&mut wtxn, &key, &val)?;
 }
 wtxn.commit()?;
 Ok(())
 }

 /// Walk the branch chain root-ward; on each branch, scan reverse from the
 /// `(branch, entity, time)` upper bound and accept the first key sharing
 /// the `(branch, entity)` prefix.
 pub fn point_query(
 &self,
 branch_id: u64,
 entity_id: u64,
 time: u64,
 ) -> Result<Option<EntityRecord>> {
 let rtxn = self.env.read_txn()?;
 let mut current = Some(branch_id);
 while let Some(b) = current {
 let upper = CompositeKey { branch_id: b, entity_id, valid_from: time }.encode();
 let lower = CompositeKey { branch_id: b, entity_id, valid_from: 0 }.encode();
 let prefix = CompositeKey::entity_prefix(b, entity_id);
 let range = (
  std::ops::Bound::Included(&lower[..]),
  std::ops::Bound::Included(&upper[..]),
 );
 let iter = self.entities.range(&rtxn, &range)?;
 let mut last: Option<(Vec<u8>, Vec<u8>)> = None;
 for item in iter {
  let (k, v) = item?;
  if k.len() != KEY_LEN || &k[..16] != &prefix[..] {
  continue;
  }
  last = Some((k.to_vec(), v.to_vec()));
 }
 if let Some((k, v)) = last {
  let key = CompositeKey::decode(&k)?;
  if key.valid_from <= time {
  let rec: EntityRecord = bincode::deserialize(&v)?;
  return Ok(Some(rec));
  }
 }
 // Need a new read txn to fetch parent meta on the same MVCC view.
 let parent = match self.branch_meta.get(&rtxn, {
  let mut buf = [0u8; 8];
  BigEndian::write_u64(&mut buf, b);
  &buf.to_vec()[..]
 })? {
  Some(buf) => bincode::deserialize::<BranchMeta>(buf)?.parent,
  None => None,
 };
 current = parent;
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

 // Collect resolved records first under read txn so the write txn is
 // brief — LMDB write-mutex throughput depends on writer-txn duration.
 let mut resolved: Vec<(u64, EntityRecord)> = Vec::new();
 for &eid in entity_ids {
 if let Some(rec) = self.point_query(source_branch, eid, time)? {
  resolved.push((eid, rec));
 }
 }

 let mut wtxn = self.env.write_txn()?;
 for (eid, rec) in resolved {
 let key = CompositeKey { branch_id: new_branch_id, entity_id: eid, valid_from: time }
  .encode();
 self.entities.put(&mut wtxn, &key, &bincode::serialize(&rec)?)?;
 }
 wtxn.commit()?;
 Ok(())
 }

 /// Best-effort total on-disk size — LMDB stores the entire env in one
 /// `data.mdb` file plus a `lock.mdb` lock file. Caller should `du -sh`
 /// the env path for ground truth (LMDB allocates the map_size sparsely;
 /// apparent vs actual size diverge).
 pub fn data_file_size(&self, path: &Path) -> Result<u64> {
 let data = path.join("data.mdb");
 Ok(std::fs::metadata(&data)?.len())
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
 // Single-writer txn covering all root entities + all branch_meta rows so
 // population doesn't pay the writer-mutex acquire cost N times.
 let mut wtxn = store.env.write_txn()?;

 {
 let mut key = [0u8; 8];
 BigEndian::write_u64(&mut key, 0);
 let root_meta = BranchMeta { parent: None, depth: 0, label: "root".to_string() };
 store.branch_meta.put(&mut wtxn, &key, &bincode::serialize(&root_meta)?)?;
 }
 for b in &workload.branches {
 if b.id == 0 {
 continue;
 }
 let mut key = [0u8; 8];
 BigEndian::write_u64(&mut key, b.id);
 let meta = BranchMeta {
 parent: b.parent,
 depth: b.depth,
 label: format!("branch_{}", b.id),
 };
 store.branch_meta.put(&mut wtxn, &key, &bincode::serialize(&meta)?)?;
 }
 for e in &workload.entities {
 let key = CompositeKey { branch_id: 0, entity_id: e.id, valid_from: 0 }.encode();
 let rec = EntityRecord { kind: e.kind as u8, name: e.name.clone() };
 store.entities.put(&mut wtxn, &key, &bincode::serialize(&rec)?)?;
 }
 wtxn.commit()?;
 Ok(())
}

pub fn apply_branch_overrides(
 store: &BranchStore,
 workload: &Workload,
 override_pct: f64,
) -> Result<usize> {
 let threshold = (override_pct.clamp(0.0, 1.0) * (u64::MAX as f64)) as u64;
 let mut total_writes = 0usize;
 let mut wtxn = store.env.write_txn()?;
 for b in &workload.branches {
 if b.id == 0 {
 continue;
 }
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
 store.entities.put(&mut wtxn, &key, &bincode::serialize(&rec)?)?;
 total_writes += 1;
 }
 }
 wtxn.commit()?;
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
 let mut wtxn = store.env.write_txn()?;
 for i in 0..depth {
 let id = base_branch_id + i as u64;
 let meta = BranchMeta {
 parent: Some(parent),
 depth: i + 1,
 label: format!("{}_{:02}", label_prefix, i + 1),
 };
 let mut key = [0u8; 8];
 BigEndian::write_u64(&mut key, id);
 store.branch_meta.put(&mut wtxn, &key, &bincode::serialize(&meta)?)?;
 parent = id;
 last = id;
 }
 wtxn.commit()?;
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
 let mut wtxn = store.env.write_txn()?;
 let mut asset_count = 0usize;
 let mut ref_count = 0usize;
 for a in &workload.assets {
 let key = asset_key(a.branch_id, a.id);
 let row = AssetRow { content_hash: a.content_hash };
 store.assets.put(&mut wtxn, &key, &bincode::serialize(&row)?)?;
 asset_count += 1;
 for &fact_id in &a.facts_referenced {
 let rk = asset_ref_key(a.id, fact_id);
 store.asset_refs.put(&mut wtxn, &rk, &[])?;
 ref_count += 1;
 }
 }
 wtxn.commit()?;
 Ok((asset_count, ref_count))
}

pub fn measurement_workload_config() -> WorkloadConfig {
 workload_gen::default_config()
}

#[cfg(test)]
mod tests {
 use super::*;
 use tempfile::TempDir;

 fn open_temp() -> (BranchStore, TempDir) {
 let dir = TempDir::new().unwrap();
 let store = BranchStore::open_with_map_size(dir.path(), 64 * 1024 * 1024).unwrap();
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
