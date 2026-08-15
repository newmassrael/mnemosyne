//! `undeclared-requirement --repo <path>` — hold what this repository's CI
//! installs against what its build-machine declaration names.
//!
//! Exit codes are three rather than two, the contract the gates in this
//! repository already share:
//!
//! | code | meaning |
//! |---|---|
//! | 0 | judged, and every package CI installs is named in the declaration |
//! | 1 | judged, and these are not |
//! | 2 | NOT judged — the population or the declaration could not be read |
//!
//! A workflow that installs NOTHING is 0 and says so in its own words: the
//! witness looked and the runner needed nothing added. A workflow whose only
//! installs are ones this law cannot read is 2. Those two zeroes are the whole
//! reason the report prints the size of the population it judged.
//!
//! The third is the one that keeps being paid for: a check that never ran
//! reports zero findings, and zero findings is what a clean tree looks like.

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut repository: Option<PathBuf> = None;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--repo" => match arguments.next() {
                Some(path) => repository = Some(PathBuf::from(path)),
                None => {
                    eprintln!("undeclared-requirement: --repo needs a path to a repository root");
                    return ExitCode::from(2);
                }
            },
            "--help" | "-h" => {
                println!("usage: undeclared-requirement --repo <path>");
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("undeclared-requirement: unknown argument {other}");
                return ExitCode::from(2);
            }
        }
    }
    let Some(repository) = repository else {
        eprintln!("usage: undeclared-requirement --repo <path>");
        return ExitCode::from(2);
    };

    let report = match undeclared_requirement::run(&repository) {
        Ok(report) => report,
        Err(message) => {
            eprintln!("[undeclared-requirement] NO VERDICT — {message}");
            return ExitCode::from(2);
        }
    };

    let packages: usize = report
        .installs
        .iter()
        .map(|install| install.packages.len())
        .sum();
    println!(
        "[undeclared-requirement] {} install step(s) name {} package(s); the declaration {} \
         names {} requirement(s)",
        report.installs.len(),
        packages,
        report.declaration.display(),
        report.declared.len(),
    );
    for install in &report.installs {
        println!(
            "[undeclared-requirement]   {} installs {} with `{}`",
            install.site,
            install.packages.join(", "),
            install.manager,
        );
    }
    // WHAT WAS RECOGNISED AND NOT READ IS NAMED, for the reason the sibling gate
    // names the keys it cannot ask about: a count that is printed is a hole
    // somebody can close, and a step silently skipped is a hole that reads as a
    // clean check.
    if report.unread.is_empty() {
        println!(
            "[undeclared-requirement] every install-shaped step in the workflows was read in full"
        );
    } else {
        println!(
            "[undeclared-requirement] {} step(s) install through something this law does not \
             read and are NOT judged:",
            report.unread.len()
        );
        for step in &report.unread {
            println!(
                "[undeclared-requirement]   {} runs `{}` ({})",
                step.site, step.written, step.what
            );
        }
    }
    if !report.never_installed.is_empty() {
        println!(
            "[undeclared-requirement] {} declared requirement(s) are installed by no job, which \
             is NOT a defect — the runner image ships things a bare host does not, and this \
             witness cannot tell that from a stale entry: {}",
            report.never_installed.len(),
            report.never_installed.join(", "),
        );
    }
    if !report.has_declaration {
        println!(
            "[undeclared-requirement] {} does not exist, so nothing here names a requirement \
             at all",
            report.declaration.display()
        );
    }

    if report.installs.is_empty() {
        // A READING AND NOT A REFUSAL — and the two are told apart one branch
        // up: this is reached only when nothing install-shaped went unread, so
        // the witness looked and saw a runner that needed nothing added.
        println!(
            "[undeclared-requirement] no step in any tracked workflow installs anything, so this \
             repository's CI has found nothing a stock machine lacks"
        );
        return ExitCode::SUCCESS;
    }
    if report.findings.is_empty() {
        println!(
            "[undeclared-requirement] every package this repository's CI installs is one the \
             build-machine declaration names"
        );
        return ExitCode::SUCCESS;
    }

    for finding in &report.findings {
        println!("[undeclared-requirement] DEFECT {finding}");
        println!(
            "                     why: CI installs it because a stock machine does not have it, \
             and the build machine is a stock machine — a host that lacks it is chosen, then \
             compiles the whole graph before failing at the one crate that needs it\n\
             \x20                    fix: name `{}` in `packages` (what to install) and, if a \
             program has to RUN, under `needs` as the binary's own name",
            finding.package
        );
    }
    eprintln!(
        "[undeclared-requirement] {} package(s) CI installs are named nowhere in {}",
        report.findings.len(),
        undeclared_requirement::DECLARATION,
    );
    ExitCode::from(1)
}
