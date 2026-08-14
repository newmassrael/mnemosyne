//! Bring this repository's records under `target/` inside their declared
//! budgets, in whichever tree it is pointed at.
//!
//! TWO EXIT CODES, AND THE ABSENT THIRD IS THE POINT. Its sibling gates answer
//! 0 / 1 / 2 because they JUDGE: 1 is "these sites break the law" and 2 is "I
//! could not read enough to have a verdict". This program does not judge — a
//! directory over its budget is not a finding somebody has to fix, it is work
//! this program does. So 0 is "the budgets hold, and here is what it took", and
//! 2 is a REFUSAL: the declaration would not read, a declared path is not under
//! the tree's `target/`, a directory would not survey, or a removal failed. A
//! collector that could not run and exited 0 would leave the growth it exists
//! to bound behind a green line.
//!
//! ITS CALLER IS `scripts/verify.sh`, which runs on every verification in this
//! repository and is therefore the one reader that cannot be forgotten. The gc
//! calls it too, because a person reaching for "reclaim `target/`" should not
//! have to know that the records in it are collected by a different program.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use scratch_budget::{collect, declaration_path, mib, read_declaration, resolve};

fn main() -> ExitCode {
    let mut tree: Option<PathBuf> = None;
    let mut dry_run = false;
    let mut explain = false;
    let mut words = std::env::args().skip(1);
    while let Some(word) = words.next() {
        match word.as_str() {
            "--at" => match words.next() {
                Some(at) => tree = Some(PathBuf::from(at)),
                None => return refuse("--at names the tree to collect in and was given nothing"),
            },
            "--dry-run" => dry_run = true,
            "--explain" => explain = true,
            other => {
                return refuse(&format!(
                    "unknown argument `{other}`\n\
                     usage: scratch-budget --at <tree> [--dry-run] [--explain]"
                ))
            }
        }
    }
    let Some(tree) = tree else {
        return refuse(
            "--at <tree> is required. The tree collected in is NEVER assumed to be this \
             program's own checkout: it is run by a wrapper that wraps commands in other \
             trees, and a default would silently collect the wrong one",
        );
    };

    let declaration = match read_declaration(&declaration_path()) {
        Ok(declaration) => declaration,
        Err(why) => return refuse(&why),
    };

    if explain {
        for line in &declaration.prose {
            println!("[scratch] {line}");
        }
        for entry in &declaration.directories {
            println!(
                "[scratch] {} — {} budget\n           {}",
                entry.path.display(),
                mib(entry.budget_bytes()),
                entry.why
            );
        }
        return ExitCode::SUCCESS;
    }

    // WHAT WAS REMOVED AND WHAT WOULD BE ARE TWO TALLIES, never one. A dry run
    // removes nothing, so a summary built from the real one reads
    // "0 file(s) removed" about a plan holding 212 of them — which is the
    // answer a reader takes as "nothing to collect".
    let mut removed_files = 0usize;
    let mut removed_bytes = 0u64;
    let mut planned_files = 0usize;
    let mut planned_bytes = 0u64;
    let mut kept = 0u64;
    let mut budgeted = 0u64;
    for entry in &declaration.directories {
        let directory = match resolve(&tree, &entry.path) {
            Ok(directory) => directory,
            Err(why) => return refuse(&why),
        };
        let budget = entry.budget_bytes();
        budgeted += budget;
        let report = match collect(&directory, budget, dry_run) {
            Ok(report) => report,
            Err(why) => return refuse(&why),
        };
        kept += report.plan.kept;
        removed_files += report.removed_files;
        removed_bytes += report.removed_bytes;
        planned_files += report.plan.remove.len();
        planned_bytes += report
            .plan
            .remove
            .iter()
            .map(|entry| entry.bytes)
            .sum::<u64>();
        say(&entry.path, &report, dry_run);
    }
    let (files, bytes, verb) = if dry_run {
        (planned_files, planned_bytes, "would be removed")
    } else {
        (removed_files, removed_bytes, "removed")
    };
    println!(
        "[scratch] {} directory(ies): {files} file(s) {verb} ({}), {} kept of {} budgeted",
        declaration.directories.len(),
        mib(bytes),
        mib(kept),
        mib(budgeted)
    );
    ExitCode::SUCCESS
}

/// One line per directory, in the spelling the declaration uses — a reader who
/// wants to change a budget must be able to find the entry from the output.
fn say(declared: &Path, report: &scratch_budget::Report, dry_run: bool) {
    let head = format!(
        "[scratch] {} {} file(s) {}",
        declared.display(),
        report.files,
        mib(report.plan.total)
    );
    if report.plan.remove.is_empty() {
        println!("{head} within its {} budget", mib(report.budget));
    } else {
        let removed: u64 = report.plan.remove.iter().map(|entry| entry.bytes).sum();
        println!(
            "{head} -> {} {} oldest file(s) ({}), {} kept (budget {})",
            if dry_run { "would remove" } else { "removed" },
            report.plan.remove.len(),
            mib(removed),
            mib(report.plan.kept),
            mib(report.budget)
        );
    }
    // THE THREE THINGS A SUMMARY LINE WOULD HIDE, each printed only when it
    // happened, because a collector's output is read on every verification and
    // a line that is always there is a line nobody sees.
    if report.plan.still_over {
        println!(
            "[scratch] {} is still over its budget with everything removable gone — its \
             NEWEST record alone is bigger than {}, and the newest is the one a reader is \
             looking at. Raise the budget or find out what wrote a record that size",
            declared.display(),
            mib(report.budget)
        );
    }
    if report.links > 0 {
        println!(
            "[scratch] {} holds {} symbolic link(s), which are neither followed nor \
             removed: a link names a file that may live anywhere",
            declared.display(),
            report.links
        );
    }
    for why in &report.unreadable {
        println!(
            "[scratch] {} holds something this survey could not read, so its total is \
             short by whatever that costs: {why}",
            declared.display()
        );
    }
}

fn refuse(why: &str) -> ExitCode {
    eprintln!("[scratch] {why}");
    // 2, never 1: see the header. This is "I did not collect", not "somebody
    // has a defect to fix".
    ExitCode::from(2)
}
