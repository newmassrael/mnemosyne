//! `blind-waits --workspace <manifest>` — run the law over one workspace and
//! say what it reached.
//!
//! Exit codes are three rather than two on purpose:
//!
//! | code | meaning |
//! |---|---|
//! | 0 | judged, and the workspace obeys the law |
//! | 1 | judged, and these sites do not |
//! | 2 | NOT judged — the gate could not read enough of the tree to have an opinion |
//!
//! The third is the one R1078 paid for: a gate whose run never happened
//! reported zero findings, and zero findings is what a clean tree looks like.

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
                    eprintln!("blind-waits: --workspace needs a path to a Cargo.toml");
                    return ExitCode::from(2);
                }
            },
            "--help" | "-h" => {
                println!("usage: blind-waits --workspace <path/to/Cargo.toml>");
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("blind-waits: unknown argument {other}");
                return ExitCode::from(2);
            }
        }
    }
    let Some(manifest) = manifest else {
        eprintln!("usage: blind-waits --workspace <path/to/Cargo.toml>");
        return ExitCode::from(2);
    };

    let report = match blind_waits::run(&manifest) {
        Ok(report) => report,
        Err(message) => {
            eprintln!("blind-waits: {message}");
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
        "[blind-waits] workspace {}",
        report.workspace_root.display()
    );
    println!(
        "[blind-waits] scanned {} .rs — {} owned by a member, {} owned by none, \
         {} in a nested workspace (checked by pointing this at ITS manifest), \
         {} under target/",
        report.coverage.scanned.len(),
        report.coverage.scanned.len() - report.coverage.unowned.len(),
        report.coverage.unowned.len(),
        report.coverage.foreign_workspaces.len(),
        report.coverage.build_artifacts,
    );
    println!(
        "[blind-waits] test code: {} files, {} functions, {} wait sites",
        report.reach.test_context_files, report.reach.test_context_fns, report.reach.wait_sites,
    );
    for path in &report.coverage.unowned {
        println!("[blind-waits] unowned {}", show(path));
    }

    if let Err(refusal) = report.verdict() {
        eprintln!("[blind-waits] NO VERDICT — {refusal}");
        return ExitCode::from(2);
    }

    if !report.found_test_code() {
        // A complete answer rather than a refusal: the law is about test code
        // and there is none. Said in words so it cannot be read as "checked and
        // clean" — which is the whole of R1054's requirement, and is what the
        // exit-2 version of this got wrong by rejecting commits for a reason
        // outside its own law.
        println!(
            "[blind-waits] no test code in this workspace — the law has nothing \
             here to apply to, which is not the same as a clean check"
        );
        return ExitCode::SUCCESS;
    }

    if report.findings.is_empty() {
        println!(
            "[blind-waits] {} wait sites, none of them blind or spelled",
            report.reach.wait_sites
        );
        return ExitCode::SUCCESS;
    }

    for finding in &report.findings {
        println!(
            "[blind-waits] DEFECT {}:{}:{} {} `{}`",
            show(&finding.file),
            finding.line,
            finding.column,
            finding.defect,
            finding.callee,
        );
        println!("               why: {}", finding.defect.why());
        println!("               fix: {}", finding.defect.fix());
    }
    eprintln!(
        "[blind-waits] {} of {} wait sites in test code end on a clock rather \
         than on a condition, or spell their budget where nobody reviews it",
        report.findings.len(),
        report.reach.wait_sites,
    );
    ExitCode::from(1)
}
