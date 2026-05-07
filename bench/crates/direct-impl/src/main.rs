//! Phase -1A stage 2B direct-impl smoke runner.
//!
//! Loads a serialized workload, populates a fresh RocksDB, applies overlay
//! deltas across non-root branches, and runs each branching SLA scenario for a
//! small sample to verify the prototype works end-to-end. Real percentile
//! measurement runs through the criterion harness (`cargo bench -p direct-impl`).

use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::time::Instant;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use direct_impl::{
 populate_full, run_abort_rate, synthesize_linear_chain, BranchStore, ConcurrentStore,
};
use hdrhistogram::Histogram;
use tempfile::TempDir;
use workload_gen::{generate, Workload};

#[derive(Parser)]
#[command(name = "direct-bench", version, about = "Phase -1A stage 2B direct-impl smoke runner")]
struct Cli {
 #[command(subcommand)]
 cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
 /// End-to-end smoke: generate (or load) workload, populate, run each
 /// scenario for `iters` samples, report p50/p95/p99 from a tiny histogram.
 Smoke {
 #[arg(long)]
 workload: Option<PathBuf>,
 #[arg(long, default_value = "0.01")]
 overlay_pct: f64,
 #[arg(long, default_value_t = 200)]
 iters: usize,
 #[arg(long, default_value_t = 64)]
 block_cache_mib: usize,
 },
 /// Just populate a database into a chosen path (no benchmarks).
 Populate {
 #[arg(long)]
 workload: Option<PathBuf>,
 #[arg(long)]
 out: PathBuf,
 #[arg(long, default_value = "0.01")]
 overlay_pct: f64,
 #[arg(long, default_value_t = 64)]
 block_cache_mib: usize,
 },
 /// §12 transaction abort rate at given writer count + hot-set size.
 AbortRate {
 #[arg(long, default_value_t = 5)]
 writers: usize,
 #[arg(long, default_value_t = 100)]
 hot_set: usize,
 #[arg(long, default_value_t = 10)]
 duration_secs: u64,
 #[arg(long, default_value_t = 16)]
 block_cache_mib: usize,
 },
 /// Cold-cache point query measurement (DESIGN.md §18 line 1832).
 /// Populate, drop the store, reopen with the requested cache size, then
 /// measure depth-20 chain queries on previously-untouched entity ids so
 /// every read pays a cache-miss disk-access cost. `--cache-mib 16` is the
 /// designated cold-floor; `--cache-mib 256` matches the criterion harness
 /// hot baseline.
 ColdCache {
 #[arg(long)]
 workload: Option<PathBuf>,
 #[arg(long, default_value = "0.005")]
 overlay_pct: f64,
 #[arg(long, default_value_t = 16)]
 cache_mib: usize,
 #[arg(long, default_value_t = 200)]
 iters: usize,
 #[arg(long, default_value_t = 20)]
 chain_depth: u32,
 },
 /// §18 line 1944 storage growth — populate the full 200K-asset workload
 /// (entity / fact / asset CF as encoded by the prototype) and report
 /// the on-disk size of the RocksDB directory.
 StorageGrowth {
 #[arg(long)]
 workload: Option<PathBuf>,
 #[arg(long)]
 out: PathBuf,
 #[arg(long, default_value = "0.005")]
 overlay_pct: f64,
 #[arg(long, default_value_t = 64)]
 block_cache_mib: usize,
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
 Cmd::Smoke {
 workload,
 overlay_pct,
 iters,
 block_cache_mib,
 } => smoke(workload.as_deref(), overlay_pct, iters, block_cache_mib),
 Cmd::Populate {
 workload,
 out,
 overlay_pct,
 block_cache_mib,
 } => populate_only(workload.as_deref(), &out, overlay_pct, block_cache_mib),
 Cmd::AbortRate {
 writers,
 hot_set,
 duration_secs,
 block_cache_mib,
 } => abort_rate(writers, hot_set, duration_secs, block_cache_mib),
 Cmd::ColdCache {
 workload,
 overlay_pct,
 cache_mib,
 iters,
 chain_depth,
 } => cold_cache(workload.as_deref(), overlay_pct, cache_mib, iters, chain_depth),
 Cmd::StorageGrowth {
 workload,
 out,
 overlay_pct,
 block_cache_mib,
 } => storage_growth(workload.as_deref(), &out, overlay_pct, block_cache_mib),
 }
}

fn cold_cache(
 workload_path: Option<&std::path::Path>,
 overlay_pct: f64,
 cache_mib: usize,
 iters: usize,
 chain_depth: u32,
) -> Result<()> {
 use rand::SeedableRng;
 use rand_chacha::ChaCha20Rng;

 let workload = load_or_generate(workload_path)?;
 let dir = TempDir::new().context("tempdir")?;
 println!(
 "cold_cache: rocksdb={} cache={} MiB chain_depth={} iters={}",
 dir.path().display(),
 cache_mib,
 chain_depth,
 iters
 );

 // ── populate with default-ish hot cache so populate is fast ──
 {
 let store = BranchStore::open(dir.path(), 128)?;
 let t0 = Instant::now();
 let stats = populate_full(&store, &workload, overlay_pct)?;
 let leaf = synthesize_linear_chain(&store, 1_000_000, chain_depth, "chain_cold")?;
 println!(
 "populate: branches={} root_entities={} overlay_writes={} assets={} asset_refs={} chain_leaf={} in {:?}",
 stats.branches, stats.root_entities, stats.overlay_writes, stats.asset_rows, stats.asset_ref_rows, leaf, t0.elapsed()
 );
 // store / db arc dropped here → caches torn down before reopen
 }

 // ── reopen with the small cache and measure ──
 let store = BranchStore::open(dir.path(), cache_mib)?;
 let leaf = (1_000_000u64) + (chain_depth as u64) - 1; // last id from synth chain

 // Sample entity ids that were *never* touched during populate's batch
 // commits — we want pages read from disk, not from any residual write
 // buffer. We use the highest-numbered slice of entity ids: those land
 // in upper SST levels which are colder.
 let mut entity_ids: Vec<u64> = workload.entities.iter().map(|e| e.id).collect();
 entity_ids.sort_unstable();
 let cold_slice = if entity_ids.len() > 4096 {
 entity_ids[entity_ids.len() - 4096..].to_vec()
 } else {
 entity_ids.clone()
 };
 let mut rng = ChaCha20Rng::seed_from_u64(0xC0_1D_C0_1D_C0_1D_C0_1Du64);
 let mut hist = Histogram::<u64>::new(3).unwrap();
 for _ in 0..iters {
 use rand::Rng as _;
 let eid = cold_slice[rng.gen_range(0..cold_slice.len())];
 let t0 = Instant::now();
 let _ = store.point_query(leaf, eid, 1_000_000)?;
 hist.record(t0.elapsed().as_nanos() as u64).ok();
 }
 let target = match chain_depth {
 1 => "<10ms",
 5 | 10 => "<25ms",
 20 => "<50ms",
 _ => "best-effort",
 };
 print_hist(
 &format!("cold_point_query_depth_{:02}_cache_{}MiB", chain_depth, cache_mib),
 &hist,
 target,
 );
 println!("block cache usage after measurement: {} bytes", store.block_cache_size_bytes());
 Ok(())
}

fn storage_growth(
 workload_path: Option<&std::path::Path>,
 out: &PathBuf,
 overlay_pct: f64,
 block_cache_mib: usize,
) -> Result<()> {
 use std::process::Command;

 let workload = load_or_generate(workload_path)?;
 println!(
 "storage_growth: workload entities={} facts={} branches={} assets={}",
 workload.entities.len(),
 workload.facts.len(),
 workload.branches.len(),
 workload.assets.len()
 );
 if !out.exists() {
 std::fs::create_dir_all(out)?;
 }

 let store = BranchStore::open(out, block_cache_mib)?;
 let t0 = Instant::now();
 let stats = populate_full(&store, &workload, overlay_pct)?;
 let elapsed = t0.elapsed();
 println!(
 "populate: branches={} root_entities={} overlay_writes={} assets={} asset_refs={} in {:?}",
 stats.branches, stats.root_entities, stats.overlay_writes, stats.asset_rows, stats.asset_ref_rows, elapsed
 );

 // Drop the store so RocksDB flushes any in-memory state to disk before
 // we read its size — otherwise du undercounts the WAL / memtable side.
 drop(store);

 let du = Command::new("du")
 .arg("-sh")
 .arg("--apparent-size")
 .arg(out)
 .output()
 .context("invoke du -sh")?;
 let du_text = String::from_utf8_lossy(&du.stdout).trim().to_string();
 let du_real = Command::new("du")
 .arg("-sh")
 .arg(out)
 .output()
 .context("invoke du -sh real")?;
 let du_real_text = String::from_utf8_lossy(&du_real.stdout).trim().to_string();

 println!("\n--- §18 line 1944 storage growth ---");
 println!("apparent size : {}", du_text);
 println!("on-disk size : {}", du_real_text);
 println!(
 "target : < 50 GB (DESIGN.md §3 / §18 line 1944)"
 );
 Ok(())
}

fn abort_rate(
 writers: usize,
 hot_set_size: usize,
 duration_secs: u64,
 block_cache_mib: usize,
) -> Result<()> {
 println!("=== Phase -1A stage 2B §12 transaction abort rate ===");
 println!(
 "writers={} hot_set={} duration={}s",
 writers, hot_set_size, duration_secs
 );
 let dir = TempDir::new()?;
 println!("rocksdb path: {}", dir.path().display());
 let store = ConcurrentStore::open(dir.path(), block_cache_mib)?;
 let hot_set: Vec<u64> = (1..=hot_set_size as u64).collect();
 store.seed_hot_set(0, &hot_set)?;

 let report = run_abort_rate(
 &store,
 0,
 &hot_set,
 writers,
 std::time::Duration::from_secs(duration_secs),
 )?;
 println!(
 "\nresult: commits={} aborts={} attempts={} abort_rate={:.4} commits/s={:.1}",
 report.total_commits,
 report.total_aborts,
 report.total_commits + report.total_aborts,
 report.abort_rate,
 report.commits_per_sec
 );
 let target_pct = match writers {
 0..=5 => Some(0.05),
 6..=15 => Some(0.15),
 _ => None,
 };
 match target_pct {
 Some(t) if report.abort_rate <= t => {
 println!(
  "§18 line 1947-1948 trigger: PASS (abort_rate {:.4} <= target {:.4})",
  report.abort_rate, t
 )
 }
 Some(t) => println!(
 "§18 line 1947-1948 trigger: FAIL (abort_rate {:.4} > target {:.4})",
 report.abort_rate, t
 ),
 None => println!("writer count out of spec range — informational only"),
 }
 println!("\nper-writer:");
 for w in &report.per_writer {
 println!(
 " writer {:>3}: commits={:>8} aborts={:>8}",
 w.writer_id, w.commits, w.aborts
 );
 }
 Ok(())
}

fn load_or_generate(workload: Option<&std::path::Path>) -> Result<Workload> {
 if let Some(path) = workload {
 let f = File::open(path).with_context(|| format!("open {}", path.display()))?;
 let w: Workload = bincode::deserialize_from(BufReader::new(f))
 .with_context(|| format!("deserialize {}", path.display()))?;
 Ok(w)
 } else {
 Ok(generate(&workload_gen::default_config()))
 }
}

fn populate_only(
 workload_path: Option<&std::path::Path>,
 out: &PathBuf,
 overlay_pct: f64,
 block_cache_mib: usize,
) -> Result<()> {
 let workload = load_or_generate(workload_path)?;
 println!(
 "workload: {} entities, {} branches, {} assets",
 workload.entities.len(),
 workload.branches.len(),
 workload.assets.len()
 );
 if !out.exists() {
 std::fs::create_dir_all(out)?;
 }
 let store = BranchStore::open(out, block_cache_mib)?;
 let t0 = Instant::now();
 let stats = populate_full(&store, &workload, overlay_pct)?;
 let elapsed = t0.elapsed();
 println!(
 "populated: branches={} root_entities={} overlay_writes={} assets={} asset_refs={} in {:?}",
 stats.branches, stats.root_entities, stats.overlay_writes, stats.asset_rows, stats.asset_ref_rows, elapsed
 );
 Ok(())
}

fn smoke(
 workload_path: Option<&std::path::Path>,
 overlay_pct: f64,
 iters: usize,
 block_cache_mib: usize,
) -> Result<()> {
 let workload = load_or_generate(workload_path)?;
 println!(
 "workload: {} entities, {} facts, {} branches, {} assets",
 workload.entities.len(),
 workload.facts.len(),
 workload.branches.len(),
 workload.assets.len()
 );

 let dir = TempDir::new().context("tempdir")?;
 println!("rocksdb path: {}", dir.path().display());
 let store = BranchStore::open(dir.path(), block_cache_mib)?;

 let t0 = Instant::now();
 let stats = populate_full(&store, &workload, overlay_pct)?;
 let pop_elapsed = t0.elapsed();
 println!(
 "populate: branches={} root_entities={} overlay_writes={} assets={} asset_refs={} in {:?}",
 stats.branches, stats.root_entities, stats.overlay_writes, stats.asset_rows, stats.asset_ref_rows, pop_elapsed
 );

 // Synthesize linear chains for chain-depth scenarios beyond workload max.
 let chain_depths = [1u32, 5, 10, 20, 50];
 let mut chain_leaves = Vec::new();
 let mut next_id = 1_000_000u64;
 for d in chain_depths {
 let leaf = synthesize_linear_chain(&store, next_id, d, "chain")?;
 chain_leaves.push((d, leaf));
 next_id += d as u64 + 1;
 }
 println!(
 "synth chains: {:?}",
 chain_leaves.iter().map(|(d, l)| (*d, *l)).collect::<Vec<_>>()
 );

 // Pre-compute candidate entity ids and branch ids from workload.
 let entity_ids: Vec<u64> = workload.entities.iter().map(|e| e.id).collect();
 let non_root_branches: Vec<u64> = workload
 .branches
 .iter()
 .filter(|b| b.id != 0)
 .map(|b| b.id)
 .collect();

 println!("\n--- scenarios (iters = {}) ---\n", iters);
 measure_branch_creation(&store, iters, &mut next_id)?;
 measure_point_query_workload(&store, &workload, iters, &entity_ids, &non_root_branches)?;
 measure_point_query_chain_depths(&store, &chain_leaves, iters, &entity_ids)?;
 measure_cross_branch_diff(
 &store,
 iters.min(50), // diff is O(entity_ids) — keep small
 &workload,
 &non_root_branches,
 &entity_ids,
 )?;
 measure_flatten_recovery(&store, &chain_leaves, iters, &entity_ids, &mut next_id)?;
 println!("\nsmoke: PASS");
 Ok(())
}

fn measure_branch_creation(store: &BranchStore, iters: usize, next_id: &mut u64) -> Result<()> {
 let mut hist = Histogram::<u64>::new(3).unwrap();
 for _ in 0..iters {
 let id = *next_id;
 *next_id += 1;
 let t0 = Instant::now();
 store.put_branch(
 id,
 &direct_impl::BranchMeta {
  parent: Some(0),
  depth: 1,
  label: format!("create_{}", id),
 },
 )?;
 let ns = t0.elapsed().as_nanos() as u64;
 hist.record(ns).ok();
 }
 print_hist("branch_creation", &hist, "<100ms");
 Ok(())
}

fn measure_point_query_workload(
 store: &BranchStore,
 workload: &Workload,
 iters: usize,
 entity_ids: &[u64],
 non_root_branches: &[u64],
) -> Result<()> {
 use rand::SeedableRng;
 use rand_chacha::ChaCha20Rng;
 let mut rng = ChaCha20Rng::seed_from_u64(0x70_1A_70_1A_70_1A_70_1A);
 let mut hist = Histogram::<u64>::new(3).unwrap();
 let _ = workload;
 for _ in 0..iters {
 let bid = pick(non_root_branches, &mut rng);
 let eid = pick(entity_ids, &mut rng);
 let t0 = Instant::now();
 let _ = store.point_query(bid, eid, 1_000_000)?;
 let ns = t0.elapsed().as_nanos() as u64;
 hist.record(ns).ok();
 }
 print_hist("point_query_workload", &hist, "<10ms (root)");
 Ok(())
}

fn measure_point_query_chain_depths(
 store: &BranchStore,
 chain_leaves: &[(u32, u64)],
 iters: usize,
 entity_ids: &[u64],
) -> Result<()> {
 use rand::SeedableRng;
 use rand_chacha::ChaCha20Rng;
 for &(depth, leaf) in chain_leaves {
 let mut rng = ChaCha20Rng::seed_from_u64(0xCA_FE_BA_BE_DE_AD_BE_EF ^ depth as u64);
 let mut hist = Histogram::<u64>::new(3).unwrap();
 for _ in 0..iters {
 let eid = pick(entity_ids, &mut rng);
 let t0 = Instant::now();
 let _ = store.point_query(leaf, eid, 1_000_000)?;
 let ns = t0.elapsed().as_nanos() as u64;
 hist.record(ns).ok();
 }
 let target = match depth {
 1 => "<10ms",
 5 | 10 => "<25ms",
 20 => "<50ms",
 _ => "best-effort",
 };
 print_hist(&format!("point_query_depth_{:02}", depth), &hist, target);
 }
 Ok(())
}

fn measure_cross_branch_diff(
 store: &BranchStore,
 iters: usize,
 workload: &Workload,
 non_root_branches: &[u64],
 entity_ids: &[u64],
) -> Result<()> {
 use rand::SeedableRng;
 use rand_chacha::ChaCha20Rng;
 let mut rng = ChaCha20Rng::seed_from_u64(0x9001_9001_9001_9001);
 let _ = workload;
 let mut hist = Histogram::<u64>::new(3).unwrap();
 // Diff entity sample size — measure over a fixed 500-entity slice for
 // p95 comparability. Full 16K-entity diff is reported separately.
 let sample: Vec<u64> = entity_ids.iter().copied().take(500).collect();
 for _ in 0..iters {
 let a = pick(non_root_branches, &mut rng);
 let b = pick(non_root_branches, &mut rng);
 let t0 = Instant::now();
 let _ = store.cross_branch_diff(a, b, &sample, 1_000_000)?;
 let ns = t0.elapsed().as_nanos() as u64;
 hist.record(ns).ok();
 }
 print_hist("cross_branch_diff_500_entities", &hist, "<1s");
 Ok(())
}

fn measure_flatten_recovery(
 store: &BranchStore,
 chain_leaves: &[(u32, u64)],
 iters: usize,
 entity_ids: &[u64],
 next_id: &mut u64,
) -> Result<()> {
 let depth_20 = chain_leaves.iter().find(|(d, _)| *d == 20);
 let Some(&(_, leaf)) = depth_20 else {
 bail!("depth-20 chain leaf missing");
 };
 let sample: Vec<u64> = entity_ids.iter().copied().take(500).collect();
 let new_branch = *next_id;
 *next_id += 1;
 let t0 = Instant::now();
 store.flatten_branch(leaf, &sample, 1_000_000, new_branch, "flat".into())?;
 let flatten_elapsed = t0.elapsed();
 println!(
 "flatten depth-20→1 over 500 entities: {:?}",
 flatten_elapsed
 );

 use rand::SeedableRng;
 use rand_chacha::ChaCha20Rng;
 let mut rng = ChaCha20Rng::seed_from_u64(0xF1A7_F1A7);
 let mut hist = Histogram::<u64>::new(3).unwrap();
 for _ in 0..iters {
 let eid = pick(&sample, &mut rng);
 let t0 = Instant::now();
 let _ = store.point_query(new_branch, eid, 1_000_000)?;
 let ns = t0.elapsed().as_nanos() as u64;
 hist.record(ns).ok();
 }
 print_hist("flatten_post_recovery_query", &hist, "best-effort");
 Ok(())
}

fn pick<T: Copy>(slice: &[T], rng: &mut rand_chacha::ChaCha20Rng) -> T {
 use rand::Rng;
 slice[rng.gen_range(0..slice.len())]
}

fn print_hist(label: &str, hist: &Histogram<u64>, target: &str) {
 if hist.is_empty() {
 println!("{}: <no samples>", label);
 return;
 }
 let p50 = hist.value_at_quantile(0.50);
 let p95 = hist.value_at_quantile(0.95);
 let p99 = hist.value_at_quantile(0.99);
 let max = hist.max();
 println!(
 "{:30} n={:>6} p50 {:>10} p95 {:>10} p99 {:>10} max {:>10} target {}",
 label,
 hist.len(),
 fmt_ns(p50),
 fmt_ns(p95),
 fmt_ns(p99),
 fmt_ns(max),
 target
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
