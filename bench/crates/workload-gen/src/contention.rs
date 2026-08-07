//! WHAT A RUN OF CONCURRENT WRITERS GUARANTEES, AND WHAT IT ONLY OBSERVED.
//!
//! Round 1073 repaired a test that measured the machine. `sled-baseline`'s
//! `many_writers_show_some_conflicts` ran 8 writers over 4 hot keys for 500 ms
//! and required `total_conflicts > 0`, reading a zero as "CAS detection
//! broken?". A CI runner produced zero, and it was right to: a scheduler that
//! serialises eight threads produces zero conflicts with a perfectly working
//! compare-and-swap. Contention is an OUTCOME of how the machine happened to
//! interleave, not a property of the store — so the assertion had been passing
//! because the runner had been fast enough, which is the definition of a green
//! that means nothing in particular.
//!
//! That round repaired the one instance and said so: "there is no reason to
//! believe this was the only one; the population of timing-dependent
//! assertions in `bench` has not been swept." Round 1075 swept it and found
//! two more of the same shape — `direct-impl`'s `many_writers_show_some_aborts`
//! required `total_aborts > 0` with the same reasoning written in the same
//! words, and EVERY concurrent test in all three baselines required
//! `total_commits > 0`, which is the same defect one step weaker: a writer
//! thread the OS schedules after the stop flag is set performs zero iterations,
//! so "some commit happened in 200 ms" is a claim about the runner too.
//!
//! SO THE RULE LIVES HERE, ONCE. All three baselines already depend on this
//! crate, so the thing they must agree about does not have to be written three
//! times and drift three ways. [`WriterRun::assert_guarantees`] holds a run to
//! exactly what is true whatever the scheduler did, and each backend maps its
//! own report onto it — which is where the mapping belongs, since only that
//! type knows which of its counters is the contention one.
//!
//! HOW THE RULE IS KEPT HONEST is that every backend asks it of a run with ONE
//! writer as well as of the contended run. A single writer is the limiting
//! case of the schedule a loaded machine is allowed to produce: no overlap at
//! all, so every contention counter is deterministically zero. Any claim that
//! needs writers to collide dies there, in every backend, the moment someone
//! adds it — which is the refuter the original assertions never had.

/// One concurrent-writer run, reduced to the quantities every backend reports.
///
/// Backends differ in what they call the counter for an attempt another writer
/// invalidated — sled and LMDB report `conflicts`, RocksDB's optimistic
/// transactions report `aborts` — and not at all in what it MEANS. It is
/// [`WriterRun::contended`] here, and it is never a guarantee.
#[derive(Clone, Debug)]
pub struct WriterRun {
    /// Writers the caller asked for.
    pub writers_requested: usize,
    /// Attempts that landed, summed over the writers. AN OBSERVATION: a writer
    /// the OS starts after the run's stop flag is set contributes none.
    pub commits: u64,
    /// Attempts the backend refused because another writer had moved the key
    /// underneath. AN OBSERVATION, and the one Round 1073 was written about:
    /// zero is what a serialising scheduler produces from a working store.
    pub contended: u64,
    /// Per writer, `(commits, contended)` — one row per writer that reported.
    pub per_writer: Vec<(u64, u64)>,
    /// How long the run actually took, as the report measured it.
    pub elapsed_ms: u64,
    /// How long the caller asked it to run for.
    pub requested_ms: u64,
}

impl WriterRun {
    /// Every claim this run supports whatever the machine did with it.
    ///
    /// The list is deliberately short, and what is NOT on it is the point:
    /// nothing here requires a commit to have happened, or a collision to have
    /// happened, because neither is something the code under test decides. A
    /// property about the store — that a stale expected value is refused, that
    /// a committed write is readable — is constructible in ONE thread, where it
    /// is a fact rather than an outcome, and that is where each backend asserts
    /// it.
    pub fn assert_guarantees(&self) {
        assert_eq!(
            self.per_writer.len(),
            self.writers_requested,
            "{} writer(s) were asked for and {} reported: the totals are over a \
             different crowd than the run was given",
            self.writers_requested,
            self.per_writer.len()
        );
        let commits: u64 = self.per_writer.iter().map(|(c, _)| c).sum();
        let contended: u64 = self.per_writer.iter().map(|(_, x)| x).sum();
        assert_eq!(
            (commits, contended),
            (self.commits, self.contended),
            "the per-writer rows and the totals disagree, so one of them is \
             counting something the other is not"
        );
        assert!(
            self.elapsed_ms >= self.requested_ms,
            "the run reports {}ms for a {}ms request, which is less time than \
             it was told to take",
            self.elapsed_ms,
            self.requested_ms
        );
    }

    /// What the run OBSERVED, for printing beside the assertions. These are the
    /// numbers the measurement exists to produce; they are reported and never
    /// required.
    pub fn observed(&self) -> String {
        let attempts = self.commits + self.contended;
        format!(
            "{} writer(s) in {}ms: {} commit(s), {} contended, {} attempt(s)",
            self.writers_requested, self.elapsed_ms, self.commits, self.contended, attempts
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(writers_requested: usize, per_writer: Vec<(u64, u64)>) -> WriterRun {
        WriterRun {
            writers_requested,
            commits: per_writer.iter().map(|(c, _)| c).sum(),
            contended: per_writer.iter().map(|(_, x)| x).sum(),
            per_writer,
            elapsed_ms: 200,
            requested_ms: 200,
        }
    }

    /// A run in which nothing collided and nothing committed is a LEGAL run.
    /// This is the whole of what Round 1073 established, held as a test: if
    /// either zero were ever added to the guarantee list, this is what refuses
    /// it, without needing a slow machine to be found first.
    #[test]
    fn a_run_that_never_collided_and_never_committed_is_still_guaranteed() {
        run(1, vec![(0, 0)]).assert_guarantees();
        run(8, vec![(0, 0); 8]).assert_guarantees();
    }

    #[test]
    #[should_panic(expected = "different crowd")]
    fn a_writer_that_did_not_report_is_refused() {
        run(8, vec![(1, 0); 7]).assert_guarantees();
    }

    #[test]
    #[should_panic(expected = "counting something the other is not")]
    fn totals_that_disagree_with_the_rows_are_refused() {
        let mut r = run(2, vec![(3, 1), (4, 2)]);
        r.commits += 1;
        r.assert_guarantees();
    }

    #[test]
    #[should_panic(expected = "less time than it was told to take")]
    fn a_run_shorter_than_it_was_asked_for_is_refused() {
        let mut r = run(1, vec![(5, 0)]);
        r.elapsed_ms = 199;
        r.assert_guarantees();
    }
}
