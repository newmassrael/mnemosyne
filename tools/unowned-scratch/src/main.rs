//! `unowned-scratch --workspace <manifest>` — run the law over one workspace
//! and say what it reached.
//!
//! Exit codes are three rather than two, the contract the gates in this
//! repository share:
//!
//! | code | meaning |
//! |---|---|
//! | 0 | judged, and every reach into the shared temp root names the process |
//! | 1 | judged, and these do not |
//! | 2 | NOT judged — part of the tree could not be read, so there is no opinion |

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
                    eprintln!("unowned-scratch: --workspace needs a path to a Cargo.toml");
                    return ExitCode::from(2);
                }
            },
            "--help" | "-h" => {
                println!("usage: unowned-scratch --workspace <path/to/Cargo.toml>");
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("unowned-scratch: unknown argument {other}");
                return ExitCode::from(2);
            }
        }
    }
    let Some(manifest) = manifest else {
        eprintln!("usage: unowned-scratch --workspace <path/to/Cargo.toml>");
        return ExitCode::from(2);
    };

    let report = match unowned_scratch::run(&manifest) {
        Ok(report) => report,
        Err(message) => {
            eprintln!("unowned-scratch: {message}");
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

    println!("[unowned-scratch] workspace {}", root.display());
    println!(
        "[unowned-scratch] parsed {} .rs — {} in a nested workspace (checked by pointing this at \
         ITS manifest), {} under target/, {} directory symlink(s) not followed{}",
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
    // THE POPULATION, so a zero is a measurement. A tree with no reach into the
    // shared root and a tree the walk never opened print the same finding count
    // and must not print the same line.
    println!(
        "[unowned-scratch] {} function(s) build a path from the shared temp root; {} of them name \
         the process",
        report.reaching, report.owning,
    );

    if let Err(refusal) = report.verdict() {
        eprintln!("[unowned-scratch] NO VERDICT — {refusal}");
        return ExitCode::from(2);
    }

    if report.reaching == 0 {
        // A complete answer rather than a refusal, said in words so it cannot be
        // read as "checked and clean".
        println!(
            "[unowned-scratch] nothing here reaches the shared temp root — the law has nothing \
             to apply to, which is not the same as a clean check"
        );
        return ExitCode::SUCCESS;
    }

    if report.findings.is_empty() {
        println!(
            "[unowned-scratch] every path built from the shared temp root names the process that \
             owns it"
        );
        return ExitCode::SUCCESS;
    }

    for finding in &report.findings {
        println!(
            "[unowned-scratch] DEFECT {} — at {}:{}",
            finding,
            show(&finding.file),
            finding.line
        );
        println!(
            "                  why: `temp_dir()` is the MACHINE's directory, so a fixed name \
             under it is one path for every run — and a run that removes it removes another \
             run's"
        );
        println!(
            "                  fix: put `std::process::id()` in the name, as the sibling crates \
             do — it is not a lock or a retry, it is the coordinate the path was missing"
        );
    }
    eprintln!(
        "[unowned-scratch] {} path(s) into the shared temp root name no owner",
        report.findings.len(),
    );
    ExitCode::from(1)
}
