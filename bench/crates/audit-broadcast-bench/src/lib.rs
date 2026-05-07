//! audit-broadcast-bench — sizing audit for `tokio::sync::broadcast` capacity
//! at the synthetic load shapes the Mnemosyne audit appender will see in
//! Production (Round 109 (d)(ii) carry).
//!
//! The audit appender (`crates/mnemosyne-server/src/audit.rs`) backs its live
//! tail-following surface with a `tokio::sync::broadcast::channel<AuditRecord>`
//! whose capacity is set by `AUDIT_BROADCAST_CAPACITY=256` (Round 103). When a
//! subscriber falls more than `capacity` records behind the latest send, the
//! Receiver returns `RecvError::Lagged` and the gRPC tail loop closes the
//! stream (Round 103 default) or surfaces a resume cursor (Round 109
//! `resume_on_lag=true`). The right capacity trades off:
//!
//! * **too small** → benign bursts trigger spurious Lagged → subscribers
//! resubscribe excessively, wasting roundtrips.
//! * **too large** → memory pinned per channel scales linearly with capacity
//! * record size; one server with thousands of audit records held in RAM
//! stalls a slow subscriber.
//!
//! This crate runs synthetic load at a fixed *commit rate* and a fixed
//! *subscriber drain rate* and reports the lag distribution observed at
//! several broadcast capacities. The output is the empirical justification
//! for the production capacity choice; the bench does not embed Mnemosyne's
//! domain types — `String` is enough to make the broadcast semantics
//! observable.
//!
//! See `bin/audit-broadcast-sizing` for the runnable entry point.

use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tokio::time::sleep;

/// One row of the sizing report — every (capacity, commit_rate) cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LagReport {
 pub capacity: usize,
 pub commits_per_second: u64,
 pub drain_per_second: u64,
 pub commit_count: u64,
 /// Total `Lagged` events seen by the slow subscriber while flooding.
 pub lagged_events: u64,
 /// Sum of `missed` counts reported by `RecvError::Lagged`. Records the
 /// total number of skipped records the subscriber would have had to
 /// resubscribe-and-replay if `resume_on_lag=true` (Round 109).
 pub total_missed: u64,
 /// Records the slow subscriber successfully observed (post any Lagged
 /// resets). Adds with `total_missed` to commit_count under steady-state
 /// loss; the equality check is the report's primary internal sanity.
 pub records_seen: u64,
 pub elapsed: Duration,
}

/// Run a synthetic flood through a broadcast channel of `capacity` slots
/// while a single subscriber drains at `drain_per_second`. Commits happen
/// at `commits_per_second`; the harness sleeps between commits to honor
/// the rate, capped at a tight floor so very high rates approximate
/// "as fast as possible" without hammering the wallclock.
pub async fn measure_lag(
 capacity: usize,
 commits_per_second: u64,
 drain_per_second: u64,
 commit_count: u64,
) -> LagReport {
 let (tx, mut rx) = broadcast::channel::<String>(capacity);
 let drain_interval = if drain_per_second == 0 {
 Duration::from_secs(60)
 } else {
 Duration::from_secs_f64(1.0 / drain_per_second as f64)
 };
 let commit_interval = if commits_per_second == 0 {
 Duration::ZERO
 } else {
 Duration::from_secs_f64(1.0 / commits_per_second as f64)
 };

 let drain_handle = tokio::spawn(async move {
 let mut lagged_events: u64 = 0;
 let mut total_missed: u64 = 0;
 let mut records_seen: u64 = 0;
 loop {
 sleep(drain_interval).await;
 match rx.try_recv() {
  Ok(_) => records_seen += 1,
  Err(broadcast::error::TryRecvError::Empty) => continue,
  Err(broadcast::error::TryRecvError::Lagged(n)) => {
  lagged_events += 1;
  total_missed += n;
  }
  Err(broadcast::error::TryRecvError::Closed) => break,
 }
 }
 (lagged_events, total_missed, records_seen)
 });

 let start = Instant::now();
 for i in 1..=commit_count {
 // Per-spec, broadcast::Sender::send returns Err only when no
 // receivers are attached; we always have one, so the result is
 // always Ok. Records are tagged with their txn id so a
 // post-mortem could correlate skipped windows.
 let _ = tx.send(format!("audit-{i}"));
 if !commit_interval.is_zero() {
 sleep(commit_interval).await;
 }
 }
 drop(tx); // closes the broadcast → drain loop exits

 let (lagged_events, total_missed, records_seen) =
 drain_handle.await.expect("drain join");
 LagReport {
 capacity,
 commits_per_second,
 drain_per_second,
 commit_count,
 lagged_events,
 total_missed,
 records_seen,
 elapsed: start.elapsed(),
 }
}

#[cfg(test)]
mod tests {
 use super::*;

 #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
 async fn capacity_eight_overflows_under_fast_commits_slow_drain() {
 // 4-slot broadcast + 100 commits / very slow drain (10 / sec) ⇒
 // total_missed ≈ commit_count - records_seen, lagged_events ≥ 1.
 // The exact numbers depend on scheduler timing; the assertion is
 // that the slow-subscriber loss path fires.
 let report = measure_lag(4, 1_000, 10, 100).await;
 assert!(report.lagged_events >= 1);
 assert!(report.total_missed > 0);
 // Every committed record either showed up via try_recv or was
 // counted in missed (modulo records dropped after channel close).
 assert!(report.records_seen + report.total_missed <= report.commit_count);
 }

 #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
 async fn matched_drain_with_default_capacity_does_not_lag() {
 // Drain at the same rate as commits; capacity 256 (Round 103
 // production default) absorbs scheduler jitter. Lagged should
 // not fire.
 let report = measure_lag(256, 200, 200, 50).await;
 assert_eq!(
 report.lagged_events, 0,
 "matched commit/drain rate must not lag at default capacity"
 );
 }
}
