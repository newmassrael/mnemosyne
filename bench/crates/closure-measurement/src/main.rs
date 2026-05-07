//! Lock #1 closure runtime 5-language emit divergence measurement CLI driver
//! (Round 93). Runs floor / ceiling / pairwise Jaccard measurement and prints
//! a JSON measurement report on stdout.

use clap::Parser;
use closure_measurement::{run_full_measurement, FIXTURE_COUNT};

#[derive(Parser, Debug)]
#[command(
 name = "closure-measure",
 about = "Lock #1 closure 5-language emit divergence measurement spike"
)]
struct Args {
 /// Number of synthetic closure rule fixtures to measure (default 35,
 /// DESIGN.md §11 target metric).
 #[arg(long, default_value_t = FIXTURE_COUNT)]
 fixtures: usize,
}

fn main() -> anyhow::Result<()> {
 let args = Args::parse();
 let report = run_full_measurement(args.fixtures);
 println!("{}", serde_json::to_string_pretty(&report)?);
 if !report.lock_1_floor_pass {
 eprintln!(
 "WARN: Lock #1 floor failed — data shape 5-language emit not feasible \
  ({}/{} (fixture, backend) runs failed canonical_set inclusion)",
 report.floor.total_runs - report.floor.jaccard_inclusion_pass_count,
 report.floor.total_runs,
 );
 std::process::exit(2);
 }
 Ok(())
}
