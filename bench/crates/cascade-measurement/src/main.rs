//! Phase 1.5 cascade gate full-scale measurement CLI driver.
//!
//! Runs all 3 gates and prints a JSON measurement report on stdout.

use cascade_measurement::{run_full_measurement, BRANCH_COUNT};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "cascade-measure", about = "Phase 1.5 cascade gate measurement")]
struct Args {
 /// Number of branches (default 100, full Phase 1.5 scale).
 #[arg(long, default_value_t = BRANCH_COUNT)]
 branches: usize,

 /// Iterations per branch for gate (iii) deterministic checks.
 #[arg(long, default_value_t = 1000)]
 iter_per_branch: usize,
}

fn main() -> anyhow::Result<()> {
 let args = Args::parse();
 let report = run_full_measurement(args.branches, args.iter_per_branch)?;
 println!("{}", serde_json::to_string_pretty(&report)?);
 if !report.all_gates_pass {
 eprintln!("WARN: not all gates passed; see JSON report for detail");
 std::process::exit(2);
 }
 Ok(())
}
