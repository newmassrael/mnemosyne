//! `written-executable --workspace <manifest>` — run the law over one workspace
//! and say what it reached.
//!
//! Exit codes are three rather than two, the contract the gates in this
//! repository share:
//!
//! | code | meaning |
//! |---|---|
//! | 0 | judged, and nothing here creates an executable file |
//! | 1 | judged, and these functions do |
//! | 2 | NOT judged — part of the tree could not be read, so there is no opinion |
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
                    eprintln!("written-executable: --workspace needs a path to a Cargo.toml");
                    return ExitCode::from(2);
                }
            },
            "--help" | "-h" => {
                println!("usage: written-executable --workspace <path/to/Cargo.toml>");
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("written-executable: unknown argument {other}");
                return ExitCode::from(2);
            }
        }
    }
    let Some(manifest) = manifest else {
        eprintln!("usage: written-executable --workspace <path/to/Cargo.toml>");
        return ExitCode::from(2);
    };

    let report = match written_executable::run(&manifest) {
        Ok(report) => report,
        Err(message) => {
            eprintln!("written-executable: {message}");
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

    println!("[written-executable] workspace {}", root.display());
    println!(
        "[written-executable] parsed {} .rs — {} in a nested workspace (checked by pointing this \
         at ITS manifest), {} under target/, {} directory symlink(s) not followed{}",
        report.coverage.scanned.len(),
        report.coverage.foreign_workspaces,
        report.coverage.build_artifacts,
        report.coverage.symlinks_not_followed.len(),
        if report.coverage.symlinks_not_followed.is_empty() {
            String::new()
        } else {
            format!(
                " ({})",
                report
                    .coverage
                    .symlinks_not_followed
                    .iter()
                    .map(|p| show(p))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        },
    );
    // THE TWO POPULATIONS THE FINDINGS ARE A FRACTION OF, so a zero is a
    // measurement. A tree that applies no mode at all and a tree the walk never
    // opened print the same finding count and must not print the same line.
    println!(
        "[written-executable] {} function(s) apply a permission mode, {} call fs::copy",
        report.applying, report.copying,
    );
    // STRONG EVIDENCE AND WEAK, SIDE BY SIDE (R1182). The verdict rests on
    // neither — the executable bit is the hazard whoever runs it — but a reader
    // who is told only the total cannot tell a proven "writes and runs" from a
    // "writes, and something somewhere runs something".
    let [the_path, here, in_the_file, invisible] = report.evidence();
    println!(
        "[written-executable] of {} finding(s): {} run the very path they made executable, {} \
         spawn something in the same function, {} sit in a file that spawns something, {} show no \
         spawn at all",
        report.findings.len(),
        the_path,
        here,
        in_the_file,
        invisible,
    );
    // NOT JUDGED, AND PRINTED. A copy carries the source's mode, so a copy of an
    // executable IS a written executable — and this walk cannot read the mode of
    // a file it is not looking at. Failing on every copy would be a gate nobody
    // could keep green; saying nothing would be the disease R1174 named.
    for site in &report.unnamed_copies {
        println!(
            "[written-executable] NOT JUDGED `{}` copies a file at {}:{} — if its source is a \
             program, this writes one",
            site.owner,
            show(&site.file),
            site.line,
        );
    }

    if let Err(refusal) = report.verdict() {
        eprintln!("[written-executable] NO VERDICT — {refusal}");
        return ExitCode::from(2);
    }

    if report.applying == 0 && report.copying == 0 {
        // A complete answer rather than a refusal, said in words so it cannot be
        // read as "checked and clean".
        println!(
            "[written-executable] nothing here applies a permission mode or copies a file — the \
             law has nothing to apply to, which is not the same as a clean check"
        );
        return ExitCode::SUCCESS;
    }

    if report.findings.is_empty() {
        println!(
            "[written-executable] no function here creates an executable file — every program \
             that runs was built by cargo or is tracked by git"
        );
        return ExitCode::SUCCESS;
    }

    for finding in &report.findings {
        println!(
            "[written-executable] DEFECT {} — at {}:{}",
            finding,
            show(&finding.file),
            finding.line
        );
        println!(
            "                     why: `exec` on a file some process holds open for writing fails \
             with ETXTBSY, and the holder is another test's fork rather than this thread — so the \
             failure arrives in a crate that did nothing, only while something else is running"
        );
        println!(
            "                     fix: do not write the program. Let cargo build it \
             (`env!(\"CARGO_BIN_EXE_…\")`), reach it by SYMLINK where a name is required, and put \
             what varies per case in a data file it reads — data cannot be busy"
        );
    }
    eprintln!(
        "[written-executable] {} function(s) create a file this repository then runs",
        report.findings.len(),
    );
    ExitCode::from(1)
}
