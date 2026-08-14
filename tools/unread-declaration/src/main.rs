//! `unread-declaration --repo <path>` — hold this repository's declaration
//! against the program that reads it, and say what was reached.
//!
//! Exit codes are three rather than two, the contract the gates in this
//! repository already share:
//!
//! | code | meaning |
//! |---|---|
//! | 0 | judged, and every declared key is read as declared |
//! | 1 | judged, and these keys are not |
//! | 2 | NOT judged — the program could not be asked, so there is no opinion here |
//!
//! The third is the one that keeps being paid for: a check that never ran
//! reports zero findings, and zero findings is what a clean tree looks like.

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut repository: Option<PathBuf> = None;
    let mut program: Option<PathBuf> = None;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--repo" => match arguments.next() {
                Some(path) => repository = Some(PathBuf::from(path)),
                None => {
                    eprintln!("unread-declaration: --repo needs a path to a repository root");
                    return ExitCode::from(2);
                }
            },
            // The same "gate a copy" seam the shell gates beside the program
            // carry: the only way to prove this gate would catch the program
            // dropping a key is to hand it a program that has.
            "--program" => match arguments.next() {
                Some(path) => program = Some(PathBuf::from(path)),
                None => {
                    eprintln!("unread-declaration: --program needs a path to the reading program");
                    return ExitCode::from(2);
                }
            },
            "--help" | "-h" => {
                println!("usage: unread-declaration --repo <path> [--program <path>]");
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("unread-declaration: unknown argument {other}");
                return ExitCode::from(2);
            }
        }
    }
    let Some(repository) = repository else {
        eprintln!("usage: unread-declaration --repo <path> [--program <path>]");
        return ExitCode::from(2);
    };
    let program = match program {
        Some(path) => path,
        None => match unread_declaration::program_under_home() {
            Ok(path) => path,
            Err(message) => {
                eprintln!("[unread-declaration] NO VERDICT — {message}");
                return ExitCode::from(2);
            }
        },
    };

    let report = match unread_declaration::run(&repository, &program) {
        Ok(report) => report,
        Err(message) => {
            eprintln!(
                "[unread-declaration] NO VERDICT — the program that reads {} could not be asked: \
                 {message}",
                unread_declaration::DECLARATION
            );
            return ExitCode::from(2);
        }
    };

    if !report.has_declaration {
        // A complete answer rather than a refusal. Said in words so it cannot be
        // read as "checked and clean".
        println!(
            "[unread-declaration] {} does not exist here — the law has nothing to apply to, \
             which is not the same as a clean check",
            report.declaration.display()
        );
        return ExitCode::SUCCESS;
    }

    println!(
        "[unread-declaration] declaration {}",
        report.declaration.display()
    );
    println!(
        "[unread-declaration] asked `{}` about {} declared key(s); it can read {}",
        program.display(),
        report.top_level.len(),
        report.program_keys.join(", "),
    );
    // THE KEYS NOT ASKED ABOUT ARE NAMED. The program extracts top-level keys
    // only, so a key inside a table is read by whoever drives the program and by
    // nothing this can interrogate. Printing the count and the names is what
    // keeps that hole a hole rather than a pass.
    if report.tabled.is_empty() {
        println!("[unread-declaration] no key sits inside a table, so none went unasked");
    } else {
        println!(
            "[unread-declaration] {} key(s) inside a table are outside the program's namespace \
             and are NOT judged here: {}",
            report.tabled.len(),
            report.tabled.join(", "),
        );
    }

    if report.findings.is_empty() {
        println!(
            "[unread-declaration] every top-level key this repository declares is one the \
             program reads as declared"
        );
        return ExitCode::SUCCESS;
    }

    for finding in &report.findings {
        println!("[unread-declaration] DEFECT {finding}");
        match finding {
            unread_declaration::Finding::Unread { key } => println!(
                "                     why: `{key}` reads as a requirement and imposes none — \
                 the program falls back to its default and nothing anywhere says so\n\
                 \x20                    fix: delete it, or teach the program to read it"
            ),
            unread_declaration::Finding::Disagrees { key, .. } => println!(
                "                     why: the file is TOML and the program reads it with line \
                 patterns; `{key}` is a value the two disagree about, so the run uses a number \
                 nobody wrote\n\
                 \x20                    fix: spell it so both readings agree, or teach the \
                 program the spelling"
            ),
            unread_declaration::Finding::NotTheLanguageItClaims { .. } => println!(
                "                     why: the program's line patterns will still read PARTS of \
                 a file that does not parse, which is worse than a clean failure\n\
                 \x20                    fix: make it parse"
            ),
        }
    }
    eprintln!(
        "[unread-declaration] {} declared key(s) are not read as declared",
        report.findings.len(),
    );
    ExitCode::from(1)
}
