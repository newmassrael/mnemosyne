//! Phase -1A stage 2D §12 reframed throughput measurement (sled).
//!
//! §18 line 1947-1948 originally targets RocksDB-OCC abort rate (5w < 5%, 15w
//! < 15%); the comparable cross-backend primitive at the §12 contention point
//! is *successful commit throughput* under N concurrent writers. sled provides
//! lock-free CAS via `compare_and_swap`; each writer executes a
//! read-modify-CAS loop and counts commits / CAS conflicts. Both numbers are
//! reported so a single back-end's apparent abort rate stays auditable, but
//! the cross-backend comparator is `commits_per_sec`.

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use sled::{Db, Tree};
use serde::{Deserialize, Serialize};

use crate::{CompositeKey, EntityRecord, TREE_ENTITIES};

/// Standalone sled DB keyed by the same composite-key namespace as
/// `BranchStore::entities`. Kept distinct so the throughput run uses a fresh
/// DB without interfering with measurement fixtures.
pub struct ConcurrentStore {
 pub db: Arc<Db>,
 pub entities: Tree,
}

impl ConcurrentStore {
 pub fn open(path: &Path, cache_mib: usize) -> Result<Self> {
 let db = sled::Config::default()
 .path(path)
 .cache_capacity((cache_mib as u64) * 1024 * 1024)
 .mode(sled::Mode::HighThroughput)
 .open()
 .with_context(|| format!("open sled db at {}", path.display()))?;
 let entities = db.open_tree(TREE_ENTITIES)?;
 Ok(Self { db: Arc::new(db), entities })
 }

 pub fn seed_hot_set(&self, branch_id: u64, entity_ids: &[u64]) -> Result<()> {
 for &eid in entity_ids {
 let key = CompositeKey { branch_id, entity_id: eid, valid_from: 0 }.encode();
 let rec = EntityRecord { kind: 0, name: format!("hot_{}", eid) };
 self.entities.insert(key, bincode::serialize(&rec)?)?;
 }
 Ok(())
 }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThroughputReport {
 pub backend: &'static str,
 pub num_writers: usize,
 pub duration_ms: u64,
 pub hot_set_size: usize,
 pub total_commits: u64,
 pub total_conflicts: u64,
 pub commit_rate: f64,
 pub commits_per_sec: f64,
 pub per_writer: Vec<WriterStats>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WriterStats {
 pub writer_id: u64,
 pub commits: u64,
 pub conflicts: u64,
}

/// Run `num_writers` concurrent writers issuing read-modify-CAS against
/// random `hot_set` entries for `duration`. Each iteration counts as one
/// commit attempt; CAS conflicts count as `conflicts` (the sled equivalent of
/// the OCC abort path). The cross-backend headline is `commits_per_sec`.
pub fn run_throughput(
 store: &ConcurrentStore,
 branch_id: u64,
 hot_set: &[u64],
 num_writers: usize,
 duration: Duration,
) -> Result<ThroughputReport> {
 if hot_set.is_empty() {
 return Err(anyhow!("hot_set is empty"));
 }
 let stop = Arc::new(AtomicBool::new(false));
 let valid_from_seq = Arc::new(AtomicU64::new(1));
 let hot_set: Arc<Vec<u64>> = Arc::new(hot_set.to_vec());

 let start = Instant::now();
 let mut handles = Vec::with_capacity(num_writers);
 for writer_id in 0..num_writers as u64 {
 let entities = store.entities.clone();
 let stop = stop.clone();
 let seq = valid_from_seq.clone();
 let hot = hot_set.clone();
 let h = thread::spawn(move || -> Result<WriterStats> {
 use rand::Rng;
 use rand::SeedableRng;
 use rand_chacha::ChaCha20Rng;
 let mut rng = ChaCha20Rng::seed_from_u64(0xC0CC_C0CC ^ writer_id);
 let mut commits: u64 = 0;
 let mut conflicts: u64 = 0;
 while !stop.load(Ordering::Relaxed) {
  let eid = hot[rng.gen_range(0..hot.len())];
  let new_vf = seq.fetch_add(1, Ordering::Relaxed);
  let key = CompositeKey { branch_id, entity_id: eid, valid_from: 0 }.encode();
  let prev = entities.get(key).ok().flatten();
  let mut name = match prev.as_ref() {
  Some(buf) => bincode::deserialize::<EntityRecord>(buf)
  .map(|r| r.name)
  .unwrap_or_default(),
  None => String::new(),
  };
  name.push_str(&format!("/w{}@{}", writer_id, new_vf));
  if name.len() > 256 {
  name.truncate(256);
  }
  let rec = EntityRecord { kind: 0, name };
  let new_buf = match bincode::serialize(&rec) {
  Ok(b) => b,
  Err(_) => {
  conflicts += 1;
  continue;
  }
  };
  let prev_slice = prev.as_ref().map(|v| v.as_ref());
  match entities.compare_and_swap(key, prev_slice, Some(new_buf.as_slice())) {
  Ok(Ok(())) => commits += 1,
  Ok(Err(_)) => conflicts += 1,
  Err(_) => conflicts += 1,
  }
 }
 Ok(WriterStats { writer_id, commits, conflicts })
 });
 handles.push(h);
 }

 thread::sleep(duration);
 stop.store(true, Ordering::Relaxed);
 let mut per_writer = Vec::with_capacity(num_writers);
 for h in handles {
 match h.join() {
 Ok(Ok(stats)) => per_writer.push(stats),
 Ok(Err(e)) => return Err(e.context("writer thread")),
 Err(_) => return Err(anyhow!("writer thread panicked")),
 }
 }
 let elapsed_ms = start.elapsed().as_millis() as u64;
 let total_commits: u64 = per_writer.iter().map(|s| s.commits).sum();
 let total_conflicts: u64 = per_writer.iter().map(|s| s.conflicts).sum();
 let total_attempts = total_commits + total_conflicts;
 let commit_rate = if total_attempts == 0 {
 0.0
 } else {
 total_commits as f64 / total_attempts as f64
 };
 let commits_per_sec = if elapsed_ms == 0 {
 0.0
 } else {
 total_commits as f64 * 1_000.0 / elapsed_ms as f64
 };
 Ok(ThroughputReport {
 backend: "sled",
 num_writers,
 duration_ms: elapsed_ms,
 hot_set_size: hot_set.len(),
 total_commits,
 total_conflicts,
 commit_rate,
 commits_per_sec,
 per_writer,
 })
}

#[cfg(test)]
mod tests {
 use super::*;
 use tempfile::TempDir;

 #[test]
 fn solo_writer_no_conflicts() {
 let dir = TempDir::new().unwrap();
 let store = ConcurrentStore::open(dir.path(), 16).unwrap();
 let hot: Vec<u64> = (1..=16).collect();
 store.seed_hot_set(0, &hot).unwrap();
 let report = run_throughput(&store, 0, &hot, 1, Duration::from_millis(200)).unwrap();
 assert!(report.total_commits > 0, "no commits in 200ms?");
 assert_eq!(report.total_conflicts, 0, "solo writer should not conflict");
 }

 #[test]
 fn many_writers_show_some_conflicts() {
 let dir = TempDir::new().unwrap();
 let store = ConcurrentStore::open(dir.path(), 16).unwrap();
 let hot: Vec<u64> = (1..=4).collect();
 store.seed_hot_set(0, &hot).unwrap();
 let report = run_throughput(&store, 0, &hot, 8, Duration::from_millis(500)).unwrap();
 assert!(report.total_commits > 0);
 assert!(
 report.total_conflicts > 0,
 "8 writers / 4 hot keys produced zero conflicts in 500 ms — CAS detection broken?"
 );
 }
}
