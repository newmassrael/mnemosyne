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
        Ok(Self {
            env: Arc::new(env),
            entities,
        })
    }

    pub fn seed_hot_set(&self, branch_id: u64, entity_ids: &[u64]) -> Result<()> {
        let mut wtxn = self.env.write_txn()?;
        for &eid in entity_ids {
            let key = CompositeKey {
                branch_id,
                entity_id: eid,
                valid_from: 0,
            }
            .encode();
            let rec = EntityRecord {
                kind: 0,
                name: format!("hot_{}", eid),
            };
            self.entities
                .put(&mut wtxn, &key, &bincode::serialize(&rec)?)?;
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

impl ThroughputReport {
    /// This run as the shared contention rule reads it (Round 1075).
    ///
    /// The mapping lives here because only this type knows which of its
    /// counters is the contention one. LMDB's writer mutex means it should
    /// stay at zero by construction — which is a claim about the STORE, and is
    /// asserted separately from the guarantees that hold for any backend.
    pub fn writer_run(&self, requested: Duration) -> workload_gen::contention::WriterRun {
        workload_gen::contention::WriterRun {
            writers_requested: self.num_writers,
            commits: self.total_commits,
            contended: self.total_conflicts,
            per_writer: self
                .per_writer
                .iter()
                .map(|w| (w.commits, w.conflicts))
                .collect(),
            elapsed_ms: self.duration_ms,
            requested_ms: requested.as_millis() as u64,
        }
    }
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
                let key = CompositeKey {
                    branch_id,
                    entity_id: eid,
                    valid_from: 0,
                }
                .encode();

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
            Ok(WriterStats {
                writer_id,
                commits,
                conflicts,
            })
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

    /// THE REFUTER FOR EVERYTHING THE SHARED RULE CLAIMS (Round 1075).
    ///
    /// One writer is the limiting case of the schedule a loaded machine is
    /// allowed to produce. It asks the SAME `assert_guarantees` the contended
    /// run does, so a claim needing writers to collide fails here rather than
    /// on whichever CI runner is slow enough first.
    #[test]
    fn a_solo_writer_meets_every_guarantee_and_conflicts_with_nobody() {
        let dir = TempDir::new().unwrap();
        let store = ConcurrentStore::open(dir.path()).unwrap();
        let hot: Vec<u64> = (1..=16).collect();
        store.seed_hot_set(0, &hot).unwrap();
        let asked = Duration::from_millis(200);
        let report = run_throughput(&store, 0, &hot, 1, asked).unwrap();
        let run = report.writer_run(asked);
        println!("{}", run.observed());
        run.assert_guarantees();
        assert_eq!(
            report.total_conflicts, 0,
            "a lone writer contends with nobody"
        );
    }

    /// A COMMITTED WRITE IS THE ONE A LATER READ SEES — asserted where it is a
    /// fact rather than an outcome (Round 1075).
    ///
    /// Every test here used to carry `total_commits > 0` over a timed run. That
    /// is a claim about the runner, weaker in degree than the conflict floor
    /// Round 1073 removed but the same in kind: a writer thread the OS starts
    /// after the stop flag is set contributes no iterations. What it was
    /// reaching for — that a write commits and is readable — needs no timer.
    #[test]
    fn a_committed_write_is_what_the_next_read_returns() {
        let dir = TempDir::new().unwrap();
        let store = ConcurrentStore::open(dir.path()).unwrap();
        store.seed_hot_set(0, &[1]).unwrap();
        let key = CompositeKey {
            branch_id: 0,
            entity_id: 1,
            valid_from: 0,
        }
        .encode();
        let written = bincode::serialize(&EntityRecord {
            kind: 0,
            name: "committed under the writer mutex".to_string(),
        })
        .unwrap();

        let mut wtxn = store.env.write_txn().unwrap();
        store.entities.put(&mut wtxn, &key, &written).unwrap();
        wtxn.commit().unwrap();

        let rtxn = store.env.read_txn().unwrap();
        assert_eq!(
            store.entities.get(&rtxn, &key).unwrap(),
            Some(&written[..]),
            "a committed write is not what the next read returns"
        );
    }

    /// The contended run, asserted on what it GUARANTEES, plus the one claim
    /// that is about LMDB rather than about the machine: the env-wide writer
    /// mutex means there is no logical conflict path at all, so zero is a
    /// property of the store here and holds under every schedule.
    #[test]
    fn many_writers_serialize_on_the_writer_mutex() {
        let dir = TempDir::new().unwrap();
        let store = ConcurrentStore::open(dir.path()).unwrap();
        let hot: Vec<u64> = (1..=4).collect();
        store.seed_hot_set(0, &hot).unwrap();
        let asked = Duration::from_millis(500);
        let report = run_throughput(&store, 0, &hot, 8, asked).unwrap();
        let run = report.writer_run(asked);
        println!(
            "4 hot keys, {} — {:.1} commits/s",
            run.observed(),
            report.commits_per_sec
        );
        run.assert_guarantees();
        assert_eq!(
            report.total_conflicts, 0,
            "LMDB serialises writers on an env-wide mutex, so a logical \
             conflict has no path to occur"
        );
    }
}
