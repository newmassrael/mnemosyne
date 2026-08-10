//! Price this repository's CI caches over the runs it has already paid for.
//!
//! WHAT IS LEFT HERE is the part a suite cannot reach: a process, its exit
//! status, and its two streams. What to ask GitHub and what the answer means are
//! the library's, because a decision in `main.rs` has no reader (R1096, and R1129
//! measured three gates in this repository where that was literally true).
//!
//! `0` when a report was printed, `2` when no report could be produced at all.
//! There is no `1`: this program judges nothing, it prices — and inventing a
//! verdict code would invent a caller that acts on it.

use std::path::Path;
use std::process::Command;

use cache_price::{by_cache, jobs_in, jobs_query, prices_in, runs_in, runs_query, spread, Price};

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let [workflow, wanted] = arguments.as_slice() else {
        eprintln!(
            "usage: cache-price <workflow-file> <runs> — prices the caches of the last <runs> \
             runs of that workflow. Given {} argument(s), there is nothing to price",
            arguments.len()
        );
        std::process::exit(2);
    };
    let Ok(wanted) = wanted.parse::<usize>() else {
        eprintln!("cache-price: `{wanted}` is not a number of runs");
        std::process::exit(2);
    };
    if wanted == 0 {
        eprintln!("cache-price: a sample of no runs prices nothing");
        std::process::exit(2);
    }
    let root = match std::env::current_dir() {
        Ok(here) => here,
        Err(why) => {
            eprintln!("cache-price: no working directory to ask `gh` from: {why}");
            std::process::exit(2);
        }
    };
    if let Err(why) = report(&root, workflow, wanted) {
        eprintln!("cache-price: {why}");
        std::process::exit(2);
    }
}

/// Run `gh` and hand back its output, or say why it could not be asked.
fn gh(root: &Path, arguments: &[String]) -> Result<String, String> {
    let out = Command::new("gh")
        .args(arguments)
        .current_dir(root)
        .output()
        .map_err(|why| match why.kind() {
            std::io::ErrorKind::NotFound => "`gh` is not installed on this machine".to_string(),
            _ => format!("`gh` could not be run at all: {why}"),
        })?;
    if !out.status.success() {
        return Err(format!(
            "`gh {}` failed ({}): {}",
            arguments.join(" "),
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Price every cache of every completed run in the sample, and print it.
fn report(root: &Path, workflow: &str, wanted: usize) -> Result<(), String> {
    let runs = runs_in(wanted, &gh(root, &runs_query(workflow, wanted))?)?;
    println!("cache-price: {} run(s) of {workflow}", runs.len());

    let mut priced: Vec<Price> = Vec::new();
    let mut per_run: Vec<(u64, u64)> = Vec::new();
    // A RUN THAT HAS NOT FINISHED IS NOT A CHEAP RUN. Its steps carry a start and
    // no end, and taking it would price a cache at whatever it had spent so far —
    // so it is left out BY NAME and counted, because a sample that quietly shrank
    // is one that looks tighter than it is.
    let mut unfinished: Vec<u64> = Vec::new();
    for run in &runs {
        if run.conclusion.is_none() {
            unfinished.push(run.id);
            continue;
        }
        let jobs = jobs_in(run.id, &gh(root, &jobs_query(run.id))?)?;
        let prices = prices_in(&jobs)?;
        let spent: u64 = prices.iter().filter_map(Price::total).sum();
        per_run.push((run.id, spent));
        priced.extend(prices);
    }
    if !unfinished.is_empty() {
        println!(
            "cache-price:   {} run(s) had not finished and were left out: {unfinished:?}",
            unfinished.len()
        );
    }
    if priced.is_empty() {
        println!("cache-price: no cache was priced — this sample paid for none");
        return Ok(());
    }

    println!("cache-price: seconds per cache, over the sample (n / min / median / max / total)");
    for ((job, cache), rows) in by_cache(&priced) {
        let restores: Vec<u64> = rows.iter().filter_map(|row| row.restore).collect();
        let saves: Vec<u64> = rows.iter().filter_map(|row| row.save).collect();
        println!("  {job} :: {cache}");
        for (what, values) in [("restore", &restores), ("save", &saves)] {
            match spread(values) {
                Some(seen) => println!(
                    "    {what:<8} n={:<3} min={:<6} median={:<6} max={:<6} total={}",
                    seen.count, seen.min, seen.median, seen.max, seen.total
                ),
                // NOT A ZERO. A step nothing ran is not a step that cost nothing,
                // and this whole measurement exists because a missing term was
                // read as one.
                None => println!("    {what:<8} never ran in this sample"),
            }
        }
    }

    if let Some(seen) = spread(
        &per_run
            .iter()
            .map(|(_, spent)| *spent)
            .collect::<Vec<u64>>(),
    ) {
        println!(
            "cache-price: whole-run cache overhead, seconds — n={} min={} median={} max={} total={}",
            seen.count, seen.min, seen.median, seen.max, seen.total
        );
    }
    println!(
        "cache-price: WHAT THIS DOES NOT SAY — what a cache SAVED is compile seconds that did \
         not happen, which no timing of a run that used it can contain. The cold observations \
         live in the ledger; the derivation belongs there with them."
    );
    Ok(())
}
