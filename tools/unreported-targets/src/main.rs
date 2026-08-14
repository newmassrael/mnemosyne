//! `unreported-targets --log <path> -- <the command that produced it>` — say
//! whether that run's verdict covered every test target it compiled.
//!
//! Exit codes are three rather than two, the contract the gates in this
//! repository share:
//!
//! | code | meaning |
//! |---|---|
//! | 0 | judged, and every test target the command compiled reported a result |
//! | 1 | judged, and these compiled and never ran |
//! | 2 | NOT judged — the run did not print what it covered, or the log and the command are not about the same run |

use std::path::PathBuf;
use std::process::ExitCode;

use unreported_targets::{Answer, Report};

fn main() -> ExitCode {
    let mut log: Option<PathBuf> = None;
    let mut at = PathBuf::from(".");
    let mut command: Vec<String> = Vec::new();
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--log" => match arguments.next() {
                Some(path) => log = Some(PathBuf::from(path)),
                None => {
                    eprintln!("unreported-targets: --log needs the path of the run's output");
                    return ExitCode::from(2);
                }
            },
            "--at" => match arguments.next() {
                Some(path) => at = PathBuf::from(path),
                None => {
                    eprintln!("unreported-targets: --at needs the directory the run happened in");
                    return ExitCode::from(2);
                }
            },
            "--help" | "-h" => {
                println!("usage: unreported-targets --log <path> [--at <dir>] -- <command...>");
                return ExitCode::SUCCESS;
            }
            "--" => {
                command.extend(arguments.by_ref());
                break;
            }
            other => {
                eprintln!("unreported-targets: unknown argument {other}");
                return ExitCode::from(2);
            }
        }
    }
    let Some(log) = log else {
        eprintln!("usage: unreported-targets --log <path> [--at <dir>] -- <command...>");
        return ExitCode::from(2);
    };
    if command.is_empty() {
        eprintln!(
            "unreported-targets: the command that produced {} has to be given after `--`, \
             because what a run was meant to cover is a property of the words that ran it",
            log.display()
        );
        return ExitCode::from(2);
    }

    let report = match unreported_targets::run(&log, &command, &at) {
        Ok(report) => report,
        Err(message) => {
            eprintln!("unreported-targets: {message}");
            return ExitCode::from(2);
        }
    };
    let answer = report.answer();
    say(&report);
    ExitCode::from(u8::try_from(answer.exit_code()).unwrap_or(2))
}

/// EVERY RUN PRINTS ITS NUMBERS, including the clean ones. A gate that speaks
/// only when it has a finding leaves "nothing was wrong" and "nothing was read"
/// wearing the same silence — and this gate's population comes from a second
/// cargo invocation, which is exactly the kind of thing that can come back empty
/// without anybody noticing.
///
/// WHAT is said, and in what order, is `report_lines`, which has a test. This
/// function only puts the prefix on it and picks the stream.
fn say(report: &Report) {
    for line in unreported_targets::report_lines(report) {
        println!("[unreported-targets] {line}");
    }
    match report.answer() {
        Answer::Clean => {}
        Answer::Finding => {
            eprintln!(
                "[unreported-targets] this run's verdict does not cover the target(s) above. \
                 A suite that stops at the first failure reports a smaller number than the \
                 truth and somebody fixes to it — re-run with `--no-fail-fast`, or repair \
                 what stopped the run from reaching them."
            );
        }
        Answer::CouldNotJudge => {
            eprintln!(
                "[unreported-targets] no opinion about this run. That is NOT the same as a \
                 clean one: nothing here says the targets were covered."
            );
        }
    }
}
