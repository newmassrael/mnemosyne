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
use serde::{Deserialize, Serialize};
use sled::{Db, Tree};

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
        Ok(Self {
            db: Arc::new(db),
            entities,
        })
    }

    pub fn seed_hot_set(&self, branch_id: u64, entity_ids: &[u64]) -> Result<()> {
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

impl ThroughputReport {
    /// This run as the shared contention rule reads it (Round 1075).
    ///
    /// The mapping lives here because only this type knows which of its
    /// counters is the contention one: for sled's CAS loop that is
    /// `total_conflicts`.
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
                let key = CompositeKey {
                    branch_id,
                    entity_id: eid,
                    valid_from: 0,
                }
                .encode();
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

    /// THE REFUTER FOR EVERYTHING THE SHARED RULE CLAIMS (Round 1075).
    ///
    /// One writer is the limiting case of the schedule a loaded machine is
    /// allowed to produce: no overlap at all, so every contention counter is
    /// deterministically zero and every commit count may be. It asks the SAME
    /// `assert_guarantees` the contended run does, so a claim that needs
    /// writers to collide fails here the moment it is added, rather than on
    /// whichever CI runner is slow enough first.
    #[test]
    fn a_solo_writer_meets_every_guarantee_and_collides_with_nobody() {
        let dir = TempDir::new().unwrap();
        let store = ConcurrentStore::open(dir.path(), 16).unwrap();
        let hot: Vec<u64> = (1..=16).collect();
        store.seed_hot_set(0, &hot).unwrap();
        let asked = Duration::from_millis(200);
        let report = run_throughput(&store, 0, &hot, 1, asked).unwrap();
        let run = report.writer_run(asked);
        println!("{}", run.observed());
        run.assert_guarantees();
        assert_eq!(
            report.total_conflicts, 0,
            "a lone writer has nobody to lose a CAS to"
        );
    }

    /// A STALE EXPECTED VALUE IS REJECTED — the property the eight-writer race
    /// was reaching for, asserted where it is a fact rather than an outcome.
    ///
    /// Round 1072: `many_writers_show_some_conflicts` ran 8 writers over 4 hot
    /// keys for 500 ms and required `total_conflicts > 0`, reading a zero as
    /// "CAS detection broken?". It is not: a scheduler that serialises the
    /// writers produces zero conflicts with a perfectly working CAS, and a
    /// loaded CI runner did exactly that — the assertion measured the MACHINE.
    /// What it meant to check is constructible in one thread.
    #[test]
    fn a_stale_expected_value_is_refused_by_cas() {
        let dir = TempDir::new().unwrap();
        let store = ConcurrentStore::open(dir.path(), 16).unwrap();
        store.seed_hot_set(0, &[1]).unwrap();
        let key = CompositeKey {
            branch_id: 0,
            entity_id: 1,
            valid_from: 0,
        }
        .encode();

        let stale = store.entities.get(key).unwrap();
        let moved = bincode::serialize(&EntityRecord {
            kind: 0,
            name: "moved under the reader".to_string(),
        })
        .unwrap();
        store
            .entities
            .compare_and_swap(key, stale.as_ref().map(|v| v.as_ref()), Some(&moved[..]))
            .unwrap()
            .expect("the first CAS holds the value it read");

        let late = bincode::serialize(&EntityRecord {
            kind: 0,
            name: "written from a stale read".to_string(),
        })
        .unwrap();
        let verdict = store
            .entities
            .compare_and_swap(key, stale.as_ref().map(|v| v.as_ref()), Some(&late[..]))
            .unwrap();
        assert!(
            verdict.is_err(),
            "CAS accepted a write whose expected value had already moved"
        );
        assert_eq!(
            store.entities.get(key).unwrap().as_deref(),
            Some(&moved[..]),
            "the refused write must not have landed"
        );
    }

    /// The throughput run itself, asserted on what it GUARANTEES and nothing
    /// else. Round 1075 took `total_commits > 0` off this list: a writer thread
    /// the OS starts after the stop flag is set performs zero iterations, so
    /// "some commit happened in 500 ms" is a claim about the runner, weaker in
    /// degree than the conflict count Round 1073 removed but the same in kind.
    /// That a commit is possible at all is asserted where it is a fact —
    /// [`a_stale_expected_value_is_refused_by_cas`], which commits one.
    #[test]
    fn many_writers_commit_and_every_attempt_is_accounted() {
        let dir = TempDir::new().unwrap();
        let store = ConcurrentStore::open(dir.path(), 16).unwrap();
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
    }
}
