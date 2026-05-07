//! Phase -1A stage 1 workload generator CLI.
//!
//! Subcommands:
//! - `sanity` — full default-scale generation + determinism + scale gate
//! + throughput report (Priority 1 stop-point check).
//! - `generate` — generate workload, write to bincode file.
//! - `describe` — load workload and print summary statistics.
//! - `verify` — load workload, regenerate from its config, compare.

use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;
use std::time::Instant;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use workload_gen::{
 default_config, generate, generate_query_trace, verify_determinism, Workload, WorkloadConfig,
};

#[derive(Parser)]
#[command(name = "workload-gen", version, about = "Phase -1A stage 1 synthetic workload generator")]
struct Cli {
 #[command(subcommand)]
 cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
 /// Run the Priority 1 stop-point gate: generate at default scale, verify
 /// determinism, check scale targets, and report throughput.
 Sanity {
 #[arg(long, default_value_t = default_config().seed)]
 seed: u64,
 },
 /// Generate a workload and serialize it to a bincode file.
 Generate {
 #[arg(long, default_value_t = default_config().seed)]
 seed: u64,
 #[arg(long, default_value = "workload.bin")]
 output: PathBuf,
 #[arg(long, default_value_t = default_config().assets)]
 assets: usize,
 #[arg(long, default_value_t = default_config().facts)]
 facts: usize,
 #[arg(long, default_value_t = default_config().branches)]
 branches: usize,
 #[arg(long, default_value_t = default_config().agents)]
 agents: usize,
 },
 /// Print summary statistics for a serialized workload.
 Describe {
 path: PathBuf,
 },
 /// Reload a workload, regenerate from its embedded config, and compare.
 Verify {
 path: PathBuf,
 },
 /// Emit a deterministic query trace JSON next to the workload.
 QueryTrace {
 workload: PathBuf,
 #[arg(long)]
 output: PathBuf,
 #[arg(long, default_value_t = 10_000)]
 count: usize,
 #[arg(long, default_value_t = 0xDEAD_BEEF_DEAD_BEEFu64)]
 seed: u64,
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
 Cmd::Sanity { seed } => sanity(seed),
 Cmd::Generate {
 seed,
 output,
 assets,
 facts,
 branches,
 agents,
 } => {
 let mut cfg = default_config();
 cfg.seed = seed;
 cfg.assets = assets;
 cfg.facts = facts;
 cfg.branches = branches;
 cfg.agents = agents;
 cmd_generate(&cfg, &output)
 }
 Cmd::Describe { path } => cmd_describe(&path),
 Cmd::Verify { path } => cmd_verify(&path),
 Cmd::QueryTrace {
 workload,
 output,
 count,
 seed,
 } => cmd_query_trace(&workload, &output, count, seed),
 }
}

fn sanity(seed: u64) -> Result<()> {
 let mut cfg = default_config();
 cfg.seed = seed;
 println!("=== Phase -1A stage 1 sanity gate ===");
 println!(
 "config: assets={} facts={} branches={} agents={} entities={} seed=0x{:016x}",
 cfg.assets,
 cfg.facts,
 cfg.branches,
 cfg.agents,
 cfg.entity_dist.total(),
 cfg.seed
 );

 let t0 = Instant::now();
 let w = generate(&cfg);
 let gen_elapsed = t0.elapsed();
 println!("\ngeneration: {:?}", gen_elapsed);

 let report = w.scale_report();
 println!(
 " assets {:>7}/{:<7} ratio {:.3} {}",
 report.assets.actual,
 report.assets.target,
 report.assets.ratio(),
 if report.assets.ok() { "PASS" } else { "FAIL" }
 );
 println!(
 " facts {:>7}/{:<7} ratio {:.3} {}",
 report.facts.actual,
 report.facts.target,
 report.facts.ratio(),
 if report.facts.ok() { "PASS" } else { "FAIL" }
 );
 println!(
 " branches {:>7}/{:<7} ratio {:.3} {}",
 report.branches.actual,
 report.branches.target,
 report.branches.ratio(),
 if report.branches.ok() { "PASS" } else { "FAIL" }
 );
 println!(
 " agents {:>7}/{:<7} ratio {:.3} {}",
 report.agents.actual,
 report.agents.target,
 report.agents.ratio(),
 if report.agents.ok() { "PASS" } else { "FAIL" }
 );
 println!(
 " entities {:>7}/{:<7} ratio {:.3} {}",
 report.entities.actual,
 report.entities.target,
 report.entities.ratio(),
 if report.entities.ok() { "PASS" } else { "FAIL" }
 );
 if !report.all_ok() {
 bail!("scale gate FAIL — workload below 90% target on at least one axis");
 }

 print_branch_tree_stats(&w);
 print_asset_ref_stats(&w);

 let t1 = Instant::now();
 verify_determinism(&cfg, &w).map_err(anyhow::Error::msg)?;
 let det_elapsed = t1.elapsed();
 println!("\ndeterminism: PASS (regen took {:?})", det_elapsed);

 let t2 = Instant::now();
 let trace = generate_query_trace(&w, 10_000, cfg.seed.wrapping_add(1));
 let trace_elapsed = t2.elapsed();
 println!("query trace: {} queries in {:?}", trace.len(), trace_elapsed);

 println!("\n=== Priority 1 sanity: PASS ===");
 Ok(())
}

fn print_branch_tree_stats(w: &Workload) {
 let mut depth_hist: Vec<u32> = Vec::new();
 for b in &w.branches {
 let d = b.depth as usize;
 if depth_hist.len() <= d {
 depth_hist.resize(d + 1, 0);
 }
 depth_hist[d] += 1;
 }
 let total: u32 = depth_hist.iter().sum();
 let mean: f64 = w.branches.iter().map(|b| b.depth as f64).sum::<f64>() / total as f64;
 let max = depth_hist.len() as u32 - 1;
 println!(
 "branch tree: {} nodes, mean depth {:.2}, max depth {}",
 total, mean, max
 );
 for (d, count) in depth_hist.iter().enumerate() {
 if *count > 0 {
 println!(" depth {:>2}: {:>4}", d, count);
 }
 }
}

fn print_asset_ref_stats(w: &Workload) {
 let mut min_refs = usize::MAX;
 let mut max_refs = 0;
 let mut total_refs: u64 = 0;
 for a in &w.assets {
 let n = a.facts_referenced.len();
 min_refs = min_refs.min(n);
 max_refs = max_refs.max(n);
 total_refs += n as u64;
 }
 let mean = total_refs as f64 / w.assets.len() as f64;
 println!(
 "asset refs: min {} max {} mean {:.2} total {}",
 min_refs, max_refs, mean, total_refs
 );
}

fn cmd_generate(cfg: &WorkloadConfig, output: &PathBuf) -> Result<()> {
 let t0 = Instant::now();
 let w = generate(cfg);
 let gen_elapsed = t0.elapsed();

 let f = File::create(output).with_context(|| format!("create {}", output.display()))?;
 let mut writer = BufWriter::new(f);
 let t1 = Instant::now();
 bincode::serialize_into(&mut writer, &w).context("bincode serialize workload")?;
 let ser_elapsed = t1.elapsed();
 let report = w.scale_report();
 println!(
 "generated: {} assets, {} facts, {} branches in {:?} (gen) + {:?} (serialize)",
 report.assets.actual,
 report.facts.actual,
 report.branches.actual,
 gen_elapsed,
 ser_elapsed
 );
 if !report.all_ok() {
 bail!("scale gate FAIL on generated workload — see ratios above");
 }
 println!("wrote {}", output.display());
 Ok(())
}

fn cmd_describe(path: &PathBuf) -> Result<()> {
 let w = load_workload(path)?;
 let r = w.scale_report();
 println!("path: {}", path.display());
 println!("protocol_version: {}", w.config.protocol_version);
 println!("seed: 0x{:016x}", w.config.seed);
 println!("scale:");
 println!(" assets {:>7} target {}", r.assets.actual, r.assets.target);
 println!(" facts {:>7} target {}", r.facts.actual, r.facts.target);
 println!(" branches {:>7} target {}", r.branches.actual, r.branches.target);
 println!(" agents {:>7} target {}", r.agents.actual, r.agents.target);
 println!(" entities {:>7} target {}", r.entities.actual, r.entities.target);
 print_branch_tree_stats(&w);
 print_asset_ref_stats(&w);
 Ok(())
}

fn cmd_verify(path: &PathBuf) -> Result<()> {
 let w = load_workload(path)?;
 let regen = generate(&w.config);
 if regen != w {
 bail!("determinism FAIL — regenerated workload diverges from {}", path.display());
 }
 println!("determinism PASS for {}", path.display());
 Ok(())
}

fn cmd_query_trace(
 workload_path: &PathBuf,
 output: &PathBuf,
 count: usize,
 seed: u64,
) -> Result<()> {
 let w = load_workload(workload_path)?;
 let trace = generate_query_trace(&w, count, seed);
 let f = File::create(output).with_context(|| format!("create {}", output.display()))?;
 serde_json::to_writer(BufWriter::new(f), &trace).context("serialize query trace")?;
 println!("wrote {} queries to {}", trace.len(), output.display());
 Ok(())
}

fn load_workload(path: &PathBuf) -> Result<Workload> {
 let f = File::open(path).with_context(|| format!("open {}", path.display()))?;
 let reader = BufReader::new(f);
 let w: Workload = bincode::deserialize_from(reader)
 .with_context(|| format!("bincode deserialize {}", path.display()))?;
 Ok(w)
}
