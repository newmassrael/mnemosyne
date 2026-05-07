//! audit-broadcast-sizing — runnable entry point for the Round 109 (d)(ii)
//! capacity sizing audit. Sweeps three commit rates × three capacities and
//! prints a CSV of the empirical lag distribution. Output is the
//! substantive evidence behind the production
//! `AUDIT_BROADCAST_CAPACITY=256` choice.
//!
//! Run via `cargo run -p audit-broadcast-bench --release` from `bench/`.

use audit_broadcast_bench::measure_lag;

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() {
 println!("capacity,commits_per_second,drain_per_second,commit_count,lagged_events,total_missed,records_seen,elapsed_ms");
 let scenarios = [
 // (capacity, commits/sec, drain/sec, commit_count)
 (64, 100, 1_000, 1_000), // very slow commits / fast drain — no lag
 (64, 1_000, 100, 500), // 10x faster commits than drain — should lag
 (64, 10_000, 1_000, 2_000), // 10x faster, more volume — many lag events
 (256, 100, 1_000, 1_000),
 (256, 1_000, 100, 500),
 (256, 10_000, 1_000, 2_000),
 (1024, 100, 1_000, 1_000),
 (1024, 1_000, 100, 500),
 (1024, 10_000, 1_000, 2_000),
 ];
 for (cap, c_rate, d_rate, n) in scenarios {
 let r = measure_lag(cap, c_rate, d_rate, n).await;
 println!(
 "{},{},{},{},{},{},{},{}",
 r.capacity,
 r.commits_per_second,
 r.drain_per_second,
 r.commit_count,
 r.lagged_events,
 r.total_missed,
 r.records_seen,
 r.elapsed.as_millis()
 );
 }
}
