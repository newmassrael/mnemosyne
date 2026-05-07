//! Phase -1A stage 2D LMDB sanity-check CLI (DESIGN.md §18 line 1851).

use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use clap::{Parser, Subcommand};
use hdrhistogram::Histogram;
use lmdb_baseline::{
 populate_full, run_throughput, synthesize_linear_chain, BranchStore, ConcurrentStore,
 EntityRecord,
};
use tempfile::TempDir;
use workload_gen::{generate, Workload};

#[derive(Parser, Debug)]
#[command(name = "lmdb-bench", version, about = "Phase -1A stage 2D LMDB sanity check")]
struct Cli {
 #[command(subcommand)]
 cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
 Smoke {
 #[arg(long)]
 workload: Option<PathBuf>,
 #[arg(long, default_value_t = 0.005)]
 overlay_pct: f64,
 #[arg(long, default_value_t = 200)]
 iters: usize,
 },
 ColdPointQuery {
 #[arg(long, default_value_t = 20)]
 chain_depth: u32,
 #[arg(long, default_value_t = 200)]
 iters: usize,
 #[arg(long)]
 workload: Option<PathBuf>,
 },
 /// §12 reframed throughput at N concurrent writers (commits/s).
 Throughput {
 #[arg(long)]
 writers: usize,
 #[arg(long, default_value_t = 16)]
 hot_set: usize,
 #[arg(long, default_value_t = 30)]
 duration_secs: u64,
 },
 StorageGrowth {
 #[arg(long)]
 workload: Option<PathBuf>,
 #[arg(long)]
 out: PathBuf,
 #[arg(long, default_value_t = 0.005)]
 overlay_pct: f64,
 },
}

fn main() -> Result<()> {
 tracing_subscriber::fmt::try_init().ok();
 let cli = Cli::parse();
 match cli.cmd {
 Cmd::Smoke { workload, overlay_pct, iters } => {
 smoke(workload.as_deref(), overlay_pct, iters)
 }
 Cmd::ColdPointQuery { chain_depth, iters, workload } => {
 cold_point_query(chain_depth, iters, workload.as_deref())
 }
 Cmd::Throughput { writers, hot_set, duration_secs } => {
 throughput(writers, hot_set, duration_secs)
 }
 Cmd::StorageGrowth { workload, out, overlay_pct } => {
 storage_growth(workload.as_deref(), &out, overlay_pct)
 }
 }
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

fn smoke(
 workload_path: Option<&std::path::Path>,
 overlay_pct: f64,
 iters: usize,
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
 println!("lmdb path: {}", dir.path().display());
 let store = BranchStore::open(dir.path())?;

 let t0 = Instant::now();
 let stats = populate_full(&store, &workload, overlay_pct)?;
 let pop_elapsed = t0.elapsed();
 println!(
 "populate: branches={} root_entities={} overlay_writes={} assets={} asset_refs={} in {:?}",
 stats.branches, stats.root_entities, stats.overlay_writes,
 stats.asset_rows, stats.asset_ref_rows, pop_elapsed
 );

 let chain_depths = [1u32, 5, 10, 20, 50];
 let mut chain_leaves = Vec::new();
 let mut next_id = 1_000_000u64;
 for d in chain_depths {
 let leaf = synthesize_linear_chain(&store, next_id, d, "chain")?;
 chain_leaves.push((d, leaf));
 next_id += d as u64 + 1;
 }
 println!("synth chains: {:?}", chain_leaves);

 let entity_ids: Vec<u64> = workload.entities.iter().map(|e| e.id).collect();
 let non_root_branches: Vec<u64> = workload
 .branches
 .iter()
 .filter(|b| b.id != 0)
 .map(|b| b.id)
 .collect();

 println!("\n--- scenarios (iters = {}) ---\n", iters);
 measure_branch_creation(&store, iters, &mut next_id)?;
 measure_point_query_workload(&store, iters, &entity_ids, &non_root_branches)?;
 measure_point_query_chain_depths(&store, &chain_leaves, iters, &entity_ids)?;
 measure_cross_branch_diff(&store, iters.min(50), &non_root_branches, &entity_ids)?;
 measure_flatten_recovery(&store, &chain_leaves, iters, &entity_ids, &mut next_id)?;
 println!("\nsmoke: PASS");
 Ok(())
}

fn cold_point_query(
 chain_depth: u32,
 iters: usize,
 workload_path: Option<&std::path::Path>,
) -> Result<()> {
 let workload = load_or_generate(workload_path)?;
 let dir = TempDir::new()?;
 println!("lmdb path: {}", dir.path().display());
 let store = BranchStore::open(dir.path())?;
 populate_full(&store, &workload, 0.005)?;
 let leaf = synthesize_linear_chain(&store, 7_000_000, chain_depth, "cold_chain")?;
 let entity_ids: Vec<u64> = workload.entities.iter().map(|e| e.id).collect();

 use rand::SeedableRng;
 use rand_chacha::ChaCha20Rng;
 use rand::Rng;
 let mut rng = ChaCha20Rng::seed_from_u64(0xC0_1D_C0_1D ^ chain_depth as u64);
 let mut hist = Histogram::<u64>::new(3).unwrap();
 let cold_iter_pool = entity_ids.len().saturating_sub(iters).max(1);
 let cold_offset = entity_ids.len() - iters.min(entity_ids.len());

 for i in 0..iters {
 let eid = entity_ids[(cold_offset + (i % cold_iter_pool)) % entity_ids.len()];
 let _ = rng.gen::<u32>();
 let t0 = Instant::now();
 let _ = store.point_query(leaf, eid, 1_000_000)?;
 let ns = t0.elapsed().as_nanos() as u64;
 hist.record(ns).ok();
 }
 let target = match chain_depth {
 1 => "<10ms",
 5 | 10 => "<25ms",
 20 => "<50ms",
 _ => "best-effort",
 };
 print_hist(
 &format!("cold_point_query_depth_{:02}", chain_depth),
 &hist,
 target,
 );
 Ok(())
}

fn throughput(writers: usize, hot_set_size: usize, duration_secs: u64) -> Result<()> {
 println!("=== Phase -1A stage 2D §12 reframed throughput (lmdb) ===");
 println!("writers={} hot_set={} duration={}s", writers, hot_set_size, duration_secs);
 let dir = TempDir::new()?;
 println!("lmdb path: {}", dir.path().display());
 let store = ConcurrentStore::open(dir.path())?;
 let hot_set: Vec<u64> = (1..=hot_set_size as u64).collect();
 store.seed_hot_set(0, &hot_set)?;

 let report = run_throughput(&store, 0, &hot_set, writers, Duration::from_secs(duration_secs))?;
 println!(
 "\nresult: backend={} commits={} conflicts={} attempts={} commit_rate={:.4} commits/s={:.1}",
 report.backend,
 report.total_commits,
 report.total_conflicts,
 report.total_commits + report.total_conflicts,
 report.commit_rate,
 report.commits_per_sec
 );
 println!(
 "§12 reframe: backend-comparable headline = commits/s (LMDB writer mutex serialises writers; conflicts ~0 by construction)"
 );
 println!("\nper-writer:");
 for w in &report.per_writer {
 println!(
 " writer {:>3}: commits={:>8} conflicts={:>8}",
 w.writer_id, w.commits, w.conflicts
 );
 }
 Ok(())
}

fn storage_growth(
 workload_path: Option<&std::path::Path>,
 out: &PathBuf,
 overlay_pct: f64,
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

 let store = BranchStore::open(out)?;
 let t0 = Instant::now();
 let stats = populate_full(&store, &workload, overlay_pct)?;
 let elapsed = t0.elapsed();
 println!(
 "populate: branches={} root_entities={} overlay_writes={} assets={} asset_refs={} in {:?}",
 stats.branches, stats.root_entities, stats.overlay_writes,
 stats.asset_rows, stats.asset_ref_rows, elapsed
 );

 let api_size = store.data_file_size(out)?;
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

 println!("\n--- §18 line 1944 storage growth (lmdb) ---");
 println!("apparent size : {} (sparse — LMDB pre-allocates the map)", du_text);
 println!("on-disk size : {} (actual blocks, this is the comparator)", du_real_text);
 println!("data.mdb size : {} bytes ({:.2} MiB)", api_size, api_size as f64 / 1024.0 / 1024.0);
 println!("target : < 50 GB (DESIGN.md §3 / §18 line 1944)");
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
 &lmdb_baseline::BranchMeta {
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
 iters: usize,
 entity_ids: &[u64],
 non_root_branches: &[u64],
) -> Result<()> {
 use rand::SeedableRng;
 use rand_chacha::ChaCha20Rng;
 let mut rng = ChaCha20Rng::seed_from_u64(0x70_1A_70_1A_70_1A_70_1A);
 let mut hist = Histogram::<u64>::new(3).unwrap();
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
 non_root_branches: &[u64],
 entity_ids: &[u64],
) -> Result<()> {
 use rand::SeedableRng;
 use rand_chacha::ChaCha20Rng;
 let mut rng = ChaCha20Rng::seed_from_u64(0x9001_9001_9001_9001);
 let mut hist = Histogram::<u64>::new(3).unwrap();
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
 "flatten 500 entities chain depth 20 → branch {} in {:?}",
 new_branch, flatten_elapsed
 );

 use rand::SeedableRng;
 use rand_chacha::ChaCha20Rng;
 let mut rng = ChaCha20Rng::seed_from_u64(0xF1A7_F1A7_F1A7_F1A7);
 let mut hist = Histogram::<u64>::new(3).unwrap();
 for _ in 0..iters {
 let eid = pick(&sample, &mut rng);
 let t0 = Instant::now();
 let _ = store.point_query(new_branch, eid, 1_000_000)?;
 let ns = t0.elapsed().as_nanos() as u64;
 hist.record(ns).ok();
 }
 print_hist("flatten_post_recovery_point_query", &hist, "best-effort");
 let _ = EntityRecord { kind: 0, name: String::new() };
 Ok(())
}

fn pick<T: Copy>(slice: &[T], rng: &mut rand_chacha::ChaCha20Rng) -> T {
 use rand::Rng;
 slice[rng.gen_range(0..slice.len())]
}

fn print_hist(name: &str, hist: &Histogram<u64>, target: &str) {
 println!(
 "{:32} count={:>6} p50={:>10} p95={:>10} p99={:>10} max={:>10} (target {})",
 name,
 hist.len(),
 fmt_dur_ns(hist.value_at_quantile(0.50)),
 fmt_dur_ns(hist.value_at_quantile(0.95)),
 fmt_dur_ns(hist.value_at_quantile(0.99)),
 fmt_dur_ns(hist.max()),
 target
 );
}

fn fmt_dur_ns(ns: u64) -> String {
 if ns < 10_000 {
 format!("{} ns", ns)
 } else if ns < 10_000_000 {
 format!("{:.2} µs", ns as f64 / 1_000.0)
 } else {
 format!("{:.2} ms", ns as f64 / 1_000_000.0)
 }
}

fn _silence(_: anyhow::Error) {
 let _ = anyhow!("silence");
}
