//! Phase -1A stage 2C asset_refs blob vs normalized smoke runner.
//!
//! Builds a Zipf-skewed reference fixture, populates both layouts, then runs
//! per-hot-fact append + read measurements in single-thread mode. Real p95
//! percentile measurement runs through the criterion harness
//! (`cargo bench -p schema-bench`).

use std::path::PathBuf;
use std::time::Instant;

use anyhow::Result;
use clap::{Parser, Subcommand};
use hdrhistogram::Histogram;
use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use schema_bench::{
 generate_skewed_refs, hot_facts, AssetRefsStore,
};
use tempfile::TempDir;

#[derive(Parser)]
#[command(name = "schema-bench", version, about = "Phase -1A stage 2C schema-shape micro-bench runner")]
struct Cli {
 #[command(subcommand)]
 cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
 /// Run asset_refs blob vs normalized smoke (write append + read amp).
 AssetRefs {
 #[arg(long, default_value_t = 5_000)]
 n_facts: usize,
 #[arg(long, default_value_t = 25_000)]
 n_refs: usize,
 #[arg(long, default_value_t = 0.01)]
 hot_pct: f64,
 #[arg(long, default_value_t = 1_000)]
 iters: usize,
 #[arg(long, default_value_t = 7)]
 seed: u64,
 #[arg(long)]
 out: Option<PathBuf>,
 },
 /// Print top-N hot fact reference counts for a given fixture.
 Distribution {
 #[arg(long, default_value_t = 5_000)]
 n_facts: usize,
 #[arg(long, default_value_t = 25_000)]
 n_refs: usize,
 #[arg(long, default_value_t = 7)]
 seed: u64,
 #[arg(long, default_value_t = 20)]
 top: usize,
 },
}

fn main() -> Result<()> {
 tracing_subscriber::fmt()
 .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
 .with_target(false)
 .compact()
 .init();
 let cli = Cli::parse();
 match cli.cmd {
 Cmd::AssetRefs {
 n_facts,
 n_refs,
 hot_pct,
 iters,
 seed,
 out,
 } => asset_refs(n_facts, n_refs, hot_pct, iters, seed, out.as_deref()),
 Cmd::Distribution {
 n_facts,
 n_refs,
 seed,
 top,
 } => distribution(n_facts, n_refs, seed, top),
 }
}

fn distribution(n_facts: usize, n_refs: usize, seed: u64, top: usize) -> Result<()> {
 let refs = generate_skewed_refs(n_facts, n_refs, seed);
 let total: usize = refs.iter().map(|v| v.len()).sum();
 let hot = hot_facts(&refs, 0.01);
 let hot_count: usize = hot.iter().map(|&i| refs[i].len()).sum();
 println!(
 "n_facts={} n_refs={} total_refs={} hot_1pct_facts={} hot_share={:.1}%",
 n_facts,
 n_refs,
 total,
 hot.len(),
 100.0 * hot_count as f64 / total as f64
 );
 let mut sizes: Vec<usize> = refs.iter().map(|v| v.len()).collect();
 sizes.sort_unstable_by(|a, b| b.cmp(a));
 println!("top-{} fact ref counts:", top);
 for (i, s) in sizes.iter().take(top).enumerate() {
 println!(" rank {:>3}: {} refs", i + 1, s);
 }
 Ok(())
}

fn asset_refs(
 n_facts: usize,
 n_refs: usize,
 hot_pct: f64,
 iters: usize,
 seed: u64,
 _out: Option<&std::path::Path>,
) -> Result<()> {
 println!("=== Phase -1A stage 2C asset_refs micro-bench ===");
 println!(
 "fixture: n_facts={} n_refs={} hot_pct={:.3} iters={} seed=0x{:x}",
 n_facts, n_refs, hot_pct, iters, seed
 );

 let refs = generate_skewed_refs(n_facts, n_refs, seed);
 let total_refs: usize = refs.iter().map(|v| v.len()).sum();
 let hot = hot_facts(&refs, hot_pct);
 let hot_total: usize = hot.iter().map(|&i| refs[i].len()).sum();
 println!(
 "skew: total_refs={} hot_facts={} hot_share={:.1}%",
 total_refs,
 hot.len(),
 100.0 * hot_total as f64 / total_refs as f64
 );

 let dir = TempDir::new()?;
 println!("rocksdb path: {}", dir.path().display());
 let store = AssetRefsStore::open(dir.path(), 64)?;

 // Bulk populate both layouts from the fixture.
 let t0 = Instant::now();
 for (idx, list) in refs.iter().enumerate() {
 let fact_id = (idx as u64) + 1;
 if !list.is_empty() {
 store.blob_set(fact_id, list)?;
 store.norm_bulk(fact_id, list)?;
 }
 }
 store.flush()?;
 let pop_elapsed = t0.elapsed();
 println!("populate (both layouts): {:?}", pop_elapsed);

 // Pick a single representative hot fact for per-iteration measurement
 // (avoid noise from scanning hot list each iter).
 let hot_fact_idx = hot[0];
 let hot_fact_id = (hot_fact_idx as u64) + 1;
 let baseline_size = refs[hot_fact_idx].len();
 println!(
 "\nrepresentative hot fact: fact_id={} initial_refs={}",
 hot_fact_id, baseline_size
 );

 // ── Write append: blob vs normalized ──
 let mut rng = ChaCha20Rng::seed_from_u64(seed.wrapping_add(1));
 let mut blob_hist = Histogram::<u64>::new(3).unwrap();
 let mut norm_hist = Histogram::<u64>::new(3).unwrap();
 let mut next_asset: u64 = 10_000_000;
 for _ in 0..iters {
 let aid = next_asset;
 next_asset += 1;
 let _ = rng.gen::<u64>(); // burn rng for determinism even though aid is sequential

 let t = Instant::now();
 store.blob_append(hot_fact_id, aid)?;
 blob_hist.record(t.elapsed().as_nanos() as u64).ok();

 let aid2 = next_asset;
 next_asset += 1;
 let t = Instant::now();
 store.norm_insert(hot_fact_id, aid2)?;
 norm_hist.record(t.elapsed().as_nanos() as u64).ok();
 }
 println!("\nwrite append (single hot fact, {} iters):", iters);
 print_hist(" blob_append ", &blob_hist);
 print_hist(" norm_insert ", &norm_hist);
 println!(
 " blob/norm ratio (p50): {:.2}x (p95): {:.2}x",
 blob_hist.value_at_quantile(0.50) as f64 / norm_hist.value_at_quantile(0.50).max(1) as f64,
 blob_hist.value_at_quantile(0.95) as f64 / norm_hist.value_at_quantile(0.95).max(1) as f64
 );

 // ── Read amplification: blob single get vs normalized prefix scan ──
 let mut blob_r = Histogram::<u64>::new(3).unwrap();
 let mut norm_r = Histogram::<u64>::new(3).unwrap();
 for _ in 0..iters {
 let t = Instant::now();
 let _ = store.blob_read(hot_fact_id)?;
 blob_r.record(t.elapsed().as_nanos() as u64).ok();

 let t = Instant::now();
 let _ = store.norm_read(hot_fact_id)?;
 norm_r.record(t.elapsed().as_nanos() as u64).ok();
 }
 println!("\nread (full fact-ref list, {} iters):", iters);
 print_hist(" blob_read ", &blob_r);
 print_hist(" norm_read ", &norm_r);
 println!(
 " blob/norm ratio (p50): {:.2}x (p95): {:.2}x",
 blob_r.value_at_quantile(0.50) as f64 / norm_r.value_at_quantile(0.50).max(1) as f64,
 blob_r.value_at_quantile(0.95) as f64 / norm_r.value_at_quantile(0.95).max(1) as f64
 );

 // ── Verdict per §18 line 1901 trigger ──
 let blob_w95 = blob_hist.value_at_quantile(0.95);
 let norm_w95 = norm_hist.value_at_quantile(0.95);
 let blob_r95 = blob_r.value_at_quantile(0.95);
 let norm_r95 = norm_r.value_at_quantile(0.95);
 println!("\n--- §18 line 1901 trigger ---");
 if norm_w95 >= blob_w95 {
 println!(
 "WRITE: normalized p95 {} >= blob p95 {} → §18 trigger fires (revert to blob)",
 norm_w95, blob_w95
 );
 } else {
 println!(
 "WRITE: normalized p95 {} < blob p95 {} → §4 normalized decision confirmed (factor {:.2}x)",
 norm_w95, blob_w95,
 blob_w95 as f64 / norm_w95.max(1) as f64
 );
 }
 println!(
 "READ : blob p95 {} vs norm p95 {} (factor {:.2}x — read amp informational)",
 blob_r95, norm_r95,
 norm_r95 as f64 / blob_r95.max(1) as f64
 );
 Ok(())
}

fn print_hist(label: &str, hist: &Histogram<u64>) {
 if hist.is_empty() {
 println!("{}<no samples>", label);
 return;
 }
 let p50 = hist.value_at_quantile(0.50);
 let p95 = hist.value_at_quantile(0.95);
 let p99 = hist.value_at_quantile(0.99);
 let max = hist.max();
 println!(
 "{}n={:>6} p50 {:>10} p95 {:>10} p99 {:>10} max {:>10}",
 label,
 hist.len(),
 fmt_ns(p50),
 fmt_ns(p95),
 fmt_ns(p99),
 fmt_ns(max)
 );
}

fn fmt_ns(ns: u64) -> String {
 if ns < 10_000 {
 format!("{} ns", ns)
 } else if ns < 10_000_000 {
 format!("{:.1} µs", ns as f64 / 1_000.0)
 } else if ns < 10_000_000_000 {
 format!("{:.2} ms", ns as f64 / 1_000_000.0)
 } else {
 format!("{:.2} s", ns as f64 / 1_000_000_000.0)
 }
}
