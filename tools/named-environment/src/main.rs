//! `named-environment --workspace <manifest>` — run the law over one workspace
//! and say what it reached.
//!
//! Exit codes are three rather than two on purpose:
//!
//! | code | meaning |
//! |---|---|
//! | 0 | judged, and the workspace obeys the law |
//! | 1 | judged, and these tests do not name what they spawn |
//! | 2 | NOT judged — the gate could not read enough of the tree to have an opinion |
//!
//! The third is the one this family of gates keeps paying for: a run that never
//! happened reports zero findings, and zero findings is what a clean tree looks
//! like.

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut manifest: Option<PathBuf> = None;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--workspace" => match arguments.next() {
                Some(path) => manifest = Some(PathBuf::from(path)),
                None => {
                    eprintln!("named-environment: --workspace needs a path to a Cargo.toml");
                    return ExitCode::from(2);
                }
            },
            "--help" | "-h" => {
                println!("usage: named-environment --workspace <path/to/Cargo.toml>");
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("named-environment: unknown argument {other}");
                return ExitCode::from(2);
            }
        }
    }
    let Some(manifest) = manifest else {
        eprintln!("usage: named-environment --workspace <path/to/Cargo.toml>");
        return ExitCode::from(2);
    };

    let report = match named_environment::run(&manifest) {
        Ok(report) => report,
        Err(message) => {
            eprintln!("named-environment: {message}");
            return ExitCode::from(2);
        }
    };

    let root = report.workspace_root.clone();
    let show = |path: &std::path::Path| -> String {
        path.strip_prefix(&root)
            .unwrap_or(path)
            .display()
            .to_string()
    };

    println!(
        "[named-environment] workspace {}",
        report.workspace_root.display()
    );
    println!(
        "[named-environment] compiled {} .rs — {} no target reaches, {} in a nested workspace \
         (checked by pointing this at ITS manifest), {} under target/",
        report.coverage.scanned.len(),
        report.coverage.unreached.len(),
        report.coverage.foreign_workspaces.len(),
        report.coverage.build_artifacts,
    );
    println!(
        "[named-environment] {} binary(ies) reading {} variable(s); {} of {} test target(s) spawn \
         one, naming {} (and {} clear the environment whole)",
        report.reach.binaries,
        report.reach.reads,
        report.reach.spawning_targets,
        report.reach.test_targets,
        report.reach.named,
        report.reach.clearing_targets,
    );
    for (binary, variables) in &report.read_by {
        if variables.is_empty() {
            continue;
        }
        println!(
            "[named-environment] `{binary}` reads {}",
            variables
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    if let Err(refusal) = report.verdict() {
        eprintln!("[named-environment] NO VERDICT — {refusal}");
        return ExitCode::from(2);
    }

    if !report.found_a_spawn() {
        // A complete answer rather than a refusal: the law is about tests that
        // spawn a program, and there are none here. Said in words so it cannot
        // be read as "checked and clean".
        println!(
            "[named-environment] no test in this workspace spawns one of its binaries — the law \
             has nothing here to apply to, which is not the same as a clean check"
        );
        return ExitCode::SUCCESS;
    }

    if report.findings.is_empty() {
        println!(
            "[named-environment] every variable the spawned programs read is named by the test \
             that spawns them"
        );
        return ExitCode::SUCCESS;
    }

    for finding in &report.findings {
        println!(
            "[named-environment] DEFECT {} — read at {}:{}",
            finding,
            show(&finding.read_in),
            finding.line,
        );
        println!(
            "                    why: on a machine where `{}` is set, that test runs a different \
             program than it does on one where it is not",
            finding.variable
        );
        println!(
            "                    fix: name it in the test that spawns `{}` — `.env(\"{}\", ..)` \
             for the value the case declares, or `.env_remove(\"{}\")` for its absence",
            finding.binary, finding.variable, finding.variable,
        );
    }
    eprintln!(
        "[named-environment] {} variable(s) a spawned program reads are left to the machine",
        report.findings.len(),
    );
    ExitCode::from(1)
}
