//! Phase -1A stage 2D §12 reframed throughput measurement (LMDB).
//!
//! LMDB's writer model is *single-writer-mutex*: every `RwTxn::commit` call
//! happens under an env-wide mutex, so N concurrent writer threads serialise
//! on that mutex. Logical OCC conflicts cannot occur — every commit succeeds
//! once the mutex is granted. The cross-backend comparator is
//! `commits_per_sec`; `conflicts` will be ~0 by construction (only system-
//! level errors increment it).

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use heed::types::Bytes;
use heed::{Database, Env, EnvOpenOptions};
use serde::{Deserialize, Serialize};

use crate::{CompositeKey, EntityRecord, DB_ENTITIES, DEFAULT_MAP_SIZE};

type ByteDb = Database<Bytes, Bytes>;

pub struct ConcurrentStore {
 pub env: Arc<Env>,
 pub entities: ByteDb,
}

impl ConcurrentStore {
 pub fn open(path: &Path) -> Result<Self> {
 std::fs::create_dir_all(path)
 .with_context(|| format!("create lmdb dir {}", path.display()))?;
 let env = unsafe {
 EnvOpenOptions::new()
  .map_size(DEFAULT_MAP_SIZE)
  .max_dbs(4)
  .open(path)
 }
 .with_context(|| format!("open lmdb at {}", path.display()))?;
 let mut wtxn = env.write_txn()?;
 let entities: ByteDb = env.create_database(&mut wtxn, Some(DB_ENTITIES))?;
 wtxn.commit()?;
 Ok(Self { env: Arc::new(env), entities })
 }

 pub fn seed_hot_set(&self, branch_id: u64, entity_ids: &[u64]) -> Result<()> {
 let mut wtxn = self.env.write_txn()?;
 for &eid in entity_ids {
 let key = CompositeKey { branch_id, entity_id: eid, valid_from: 0 }.encode();
 let rec = EntityRecord { kind: 0, name: format!("hot_{}", eid) };
 self.entities.put(&mut wtxn, &key, &bincode::serialize(&rec)?)?;
 }
 wtxn.commit()?;
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
 let env = store.env.clone();
 let entities = store.entities;
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

  // Read current value via a short-lived read txn so the write
  // txn's mutex hold time stays minimal.
  let prev_name: String = {
  let rtxn = env.read_txn().map_err(|e| anyhow!(e))?;
  let v = entities.get(&rtxn, &key).map_err(|e| anyhow!(e))?;
  match v {
  Some(buf) => bincode::deserialize::<EntityRecord>(buf)
   .map(|r| r.name)
   .unwrap_or_default(),
  None => String::new(),
  }
  };
  let mut name = prev_name;
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

  let mut wtxn = match env.write_txn() {
  Ok(t) => t,
  Err(_) => {
  conflicts += 1;
  continue;
  }
  };
  if entities.put(&mut wtxn, &key, &new_buf).is_err() {
  conflicts += 1;
  continue;
  }
  if wtxn.commit().is_err() {
  conflicts += 1;
  continue;
  }
  commits += 1;
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
 backend: "lmdb",
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
 let store = ConcurrentStore::open(dir.path()).unwrap();
 let hot: Vec<u64> = (1..=16).collect();
 store.seed_hot_set(0, &hot).unwrap();
 let report = run_throughput(&store, 0, &hot, 1, Duration::from_millis(200)).unwrap();
 assert!(report.total_commits > 0, "no commits in 200ms?");
 assert_eq!(report.total_conflicts, 0, "solo writer should not conflict");
 }

 #[test]
 fn many_writers_serialize() {
 let dir = TempDir::new().unwrap();
 let store = ConcurrentStore::open(dir.path()).unwrap();
 let hot: Vec<u64> = (1..=4).collect();
 store.seed_hot_set(0, &hot).unwrap();
 let report = run_throughput(&store, 0, &hot, 8, Duration::from_millis(500)).unwrap();
 assert!(report.total_commits > 0);
 // LMDB single-writer mutex serialises writers — conflicts should be ~0
 // because there is no logical OCC conflict path.
 assert_eq!(
 report.total_conflicts, 0,
 "LMDB writer mutex should serialize, not produce logical conflicts"
 );
 }
}
