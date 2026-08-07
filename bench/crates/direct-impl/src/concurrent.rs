//! Phase -1A stage 2B §12 transaction abort rate measurement.
//!
//! §18 line 1947-1948 targets: 5-writer commit/abort ratio < 5 %,
//! 15-writer < 15 %. Optimistic-concurrency contention on a small hot set
//! exercises the abort path; sustained writers run for a fixed duration and
//! report (commits, aborts) per writer.

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use rocksdb::{
    BlockBasedOptions, Cache, ColumnFamilyDescriptor, DBCompressionType, OptimisticTransactionDB,
    Options,
};
use serde::{Deserialize, Serialize};

use crate::{CompositeKey, EntityRecord, CF_ENTITIES};

/// `OptimisticTransactionDB`-backed store that can run concurrent writer
/// transactions over the same composite-key namespace as `BranchStore`.
pub struct ConcurrentStore {
    pub db: Arc<OptimisticTransactionDB>,
}

impl ConcurrentStore {
    pub fn open(path: &Path, block_cache_mib: usize) -> Result<Self> {
        let cache = Cache::new_lru_cache(block_cache_mib * 1024 * 1024);

        let mut block_opts = BlockBasedOptions::default();
        block_opts.set_block_cache(&cache);
        block_opts.set_bloom_filter(10.0, false);

        let mut cf_opts = Options::default();
        cf_opts.set_block_based_table_factory(&block_opts);
        cf_opts.set_compression_type(DBCompressionType::Lz4);

        let cfs = vec![ColumnFamilyDescriptor::new(CF_ENTITIES, cf_opts)];

        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);

        let db = OptimisticTransactionDB::open_cf_descriptors(&db_opts, path, cfs)
            .with_context(|| format!("open OptimisticTransactionDB at {}", path.display()))?;
        Ok(Self { db: Arc::new(db) })
    }

    /// Seed `entity_ids` at branch 0 with an initial value so writer
    /// transactions can read-modify-write against an existing record.
    pub fn seed_hot_set(&self, branch_id: u64, entity_ids: &[u64]) -> Result<()> {
        let cf = self
            .db
            .cf_handle(CF_ENTITIES)
            .ok_or_else(|| anyhow!("missing CF {}", CF_ENTITIES))?;
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
            self.db.put_cf(cf, key, bincode::serialize(&rec)?)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AbortRateReport {
    pub num_writers: usize,
    pub duration_ms: u64,
    pub hot_set_size: usize,
    pub total_commits: u64,
    pub total_aborts: u64,
    pub abort_rate: f64,
    pub commits_per_sec: f64,
    pub per_writer: Vec<WriterStats>,
}

impl AbortRateReport {
    /// This run as the shared contention rule reads it (Round 1075).
    ///
    /// The mapping lives here because only this type knows which of its
    /// counters is the contention one: for an optimistic transaction that is
    /// `total_aborts`.
    pub fn writer_run(&self, requested: Duration) -> workload_gen::contention::WriterRun {
        workload_gen::contention::WriterRun {
            writers_requested: self.num_writers,
            commits: self.total_commits,
            contended: self.total_aborts,
            per_writer: self
                .per_writer
                .iter()
                .map(|w| (w.commits, w.aborts))
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
    pub aborts: u64,
}

/// Run `num_writers` concurrent writers for `duration`, each issuing
/// read-modify-write transactions against random entries in `hot_set`. Returns
/// the aggregated commit/abort counts.
pub fn run_abort_rate(
    store: &ConcurrentStore,
    branch_id: u64,
    hot_set: &[u64],
    num_writers: usize,
    duration: Duration,
) -> Result<AbortRateReport> {
    if hot_set.is_empty() {
        return Err(anyhow!("hot_set is empty"));
    }
    let stop = Arc::new(AtomicBool::new(false));
    let valid_from_seq = Arc::new(AtomicU64::new(1));
    let hot_set: Arc<Vec<u64>> = Arc::new(hot_set.to_vec());
    let cf_name = CF_ENTITIES.to_string();

    let start = Instant::now();
    let mut handles = Vec::with_capacity(num_writers);
    for writer_id in 0..num_writers as u64 {
        let db = store.db.clone();
        let stop = stop.clone();
        let seq = valid_from_seq.clone();
        let hot = hot_set.clone();
        let cf_name = cf_name.clone();
        let h = thread::spawn(move || -> Result<WriterStats> {
            use rand::Rng;
            use rand::SeedableRng;
            use rand_chacha::ChaCha20Rng;
            let mut rng = ChaCha20Rng::seed_from_u64(0xC0CC_C0CC ^ writer_id);
            let cf = db
                .cf_handle(&cf_name)
                .ok_or_else(|| anyhow!("missing CF {}", cf_name))?;
            let mut commits: u64 = 0;
            let mut aborts: u64 = 0;
            while !stop.load(Ordering::Relaxed) {
                let eid = hot[rng.gen_range(0..hot.len())];
                let new_vf = seq.fetch_add(1, Ordering::Relaxed);
                let key = CompositeKey {
                    branch_id,
                    entity_id: eid,
                    valid_from: 0,
                }
                .encode();
                let tx = db.transaction();
                // Read-modify-write under serializable snapshot semantics:
                // reading via the transaction marks the key for conflict
                // detection at commit time.
                let prev = tx.get_for_update_cf(cf, key, true).ok().flatten();
                let mut name = if let Some(buf) = prev {
                    bincode::deserialize::<EntityRecord>(&buf)
                        .map(|r| r.name)
                        .unwrap_or_default()
                } else {
                    String::new()
                };
                name.push_str(&format!("/w{}@{}", writer_id, new_vf));
                if name.len() > 256 {
                    name.truncate(256);
                }
                let rec = EntityRecord { kind: 0, name };
                if let Ok(buf) = bincode::serialize(&rec) {
                    if tx.put_cf(cf, key, buf).is_ok() {
                        match tx.commit() {
                            Ok(_) => commits += 1,
                            Err(_) => aborts += 1,
                        }
                    } else {
                        aborts += 1;
                    }
                } else {
                    aborts += 1;
                }
            }
            Ok(WriterStats {
                writer_id,
                commits,
                aborts,
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
    let total_aborts: u64 = per_writer.iter().map(|s| s.aborts).sum();
    let total_attempts = total_commits + total_aborts;
    let abort_rate = if total_attempts == 0 {
        0.0
    } else {
        total_aborts as f64 / total_attempts as f64
    };
    let commits_per_sec = if elapsed_ms == 0 {
        0.0
    } else {
        total_commits as f64 * 1_000.0 / elapsed_ms as f64
    };
    Ok(AbortRateReport {
        num_writers,
        duration_ms: elapsed_ms,
        hot_set_size: hot_set.len(),
        total_commits,
        total_aborts,
        abort_rate,
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
    /// allowed to produce: no overlap, so every abort counter is
    /// deterministically zero. It asks the SAME `assert_guarantees` the
    /// contended run does, so a claim needing writers to collide fails here
    /// the moment it is added.
    #[test]
    fn a_solo_writer_meets_every_guarantee_and_aborts_against_nobody() {
        let dir = TempDir::new().unwrap();
        let store = ConcurrentStore::open(dir.path(), 16).unwrap();
        let hot: Vec<u64> = (1..=16).collect();
        store.seed_hot_set(0, &hot).unwrap();
        let asked = Duration::from_millis(200);
        let report = run_abort_rate(&store, 0, &hot, 1, asked).unwrap();
        let run = report.writer_run(asked);
        println!("{}", run.observed());
        run.assert_guarantees();
        assert_eq!(
            report.total_aborts, 0,
            "a lone writer has nobody to lose a transaction to"
        );
    }

    /// A READ THAT MOVED UNDER THE TRANSACTION IS REFUSED AT COMMIT — the
    /// property the eight-writer race was reaching for, asserted where it is a
    /// fact rather than an outcome (Round 1075).
    ///
    /// `many_writers_show_some_aborts` ran 8 writers over 4 hot keys for 500 ms
    /// and required `total_aborts > 0`, reading a zero as "race detection
    /// broken?". That is the assertion Round 1073 removed from `sled-baseline`,
    /// in the same words, left standing here: a scheduler that serialises the
    /// writers produces zero aborts from a perfectly working optimistic
    /// transaction, so the green measured the runner. What it meant to check
    /// needs no second thread — read a key for update, let a committed
    /// transaction move it, and require the first commit to be refused.
    #[test]
    fn a_read_that_moved_underneath_is_refused_at_commit() {
        let dir = TempDir::new().unwrap();
        let store = ConcurrentStore::open(dir.path(), 16).unwrap();
        store.seed_hot_set(0, &[1]).unwrap();
        let cf = store.db.cf_handle(CF_ENTITIES).unwrap();
        let key = CompositeKey {
            branch_id: 0,
            entity_id: 1,
            valid_from: 0,
        }
        .encode();

        // `get_for_update` records the key for validation at commit, which is
        // the same tracking the writer loop relies on — the property under
        // test is the one the measurement's own transactions depend on.
        let doomed = store.db.transaction();
        doomed
            .get_for_update_cf(cf, key, true)
            .unwrap()
            .expect("the seeded record is there to be read");

        let mover = store.db.transaction();
        let moved = bincode::serialize(&EntityRecord {
            kind: 0,
            name: "moved under the reader".to_string(),
        })
        .unwrap();
        mover.put_cf(cf, key, &moved).unwrap();
        mover.commit().expect(
            "an uncontended transaction commits, which is also this \
                    test's proof that a commit is possible at all",
        );

        let late = bincode::serialize(&EntityRecord {
            kind: 0,
            name: "written from a read that had already moved".to_string(),
        })
        .unwrap();
        doomed.put_cf(cf, key, &late).unwrap();
        assert!(
            doomed.commit().is_err(),
            "the transaction committed over a key that had moved since it read \
             it, so optimistic conflict detection accepted a lost update"
        );
        assert_eq!(
            store.db.get_cf(cf, key).unwrap().as_deref(),
            Some(&moved[..]),
            "the refused write must not have landed"
        );
    }

    /// The contended run, asserted on what it GUARANTEES and nothing else.
    /// Round 1075 took `total_commits > 0` off that list along with the abort
    /// floor: a writer the OS starts after the stop flag is set contributes no
    /// iterations, so it is a claim about the runner too.
    #[test]
    fn many_writers_commit_and_every_attempt_is_accounted() {
        let dir = TempDir::new().unwrap();
        let store = ConcurrentStore::open(dir.path(), 16).unwrap();
        let hot: Vec<u64> = (1..=4).collect();
        store.seed_hot_set(0, &hot).unwrap();
        let asked = Duration::from_millis(500);
        let report = run_abort_rate(&store, 0, &hot, 8, asked).unwrap();
        let run = report.writer_run(asked);
        println!(
            "4 hot keys, {} — {:.1} commits/s, abort rate {:.3}",
            run.observed(),
            report.commits_per_sec,
            report.abort_rate
        );
        run.assert_guarantees();
    }
}
