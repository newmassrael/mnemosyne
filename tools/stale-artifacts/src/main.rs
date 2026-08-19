//! `stale-artifacts --at <tree> -- <the command about to run>` — remove the
//! build artifacts of every package that tree has changed, in the workspace
//! that command builds.
//!
//! TWO EXIT CODES RATHER THAN THE THREE THE JUDGING GATES HERE SHARE, and the
//! difference is that this program is not a judge:
//!
//! | code | meaning |
//! |---|---|
//! | 0 | the pass ran — either it cleaned what the tree had changed, or it says why there was nothing to clean |
//! | 2 | the pass did NOT run, and the caller must fail: a verification whose freshness pass did not happen is one that can read exactly the artifact the pass exists to remove |
//!
//! There is no `1`. A tree with nothing changed is not a finding, it is this
//! pass succeeding, and giving that its own code would make a caller choose
//! between failing on a clean tree and ignoring a refusal.

use std::path::PathBuf;
use std::process::ExitCode;

use stale_artifacts::Plan;

fn main() -> ExitCode {
    let mut at = PathBuf::from(".");
    let mut command: Vec<String> = Vec::new();
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--at" => match arguments.next() {
                Some(path) => at = PathBuf::from(path),
                None => {
                    eprintln!("stale-artifacts: --at needs the directory the run will happen in");
                    return ExitCode::from(2);
                }
            },
            "--help" | "-h" => {
                println!("usage: stale-artifacts [--at <dir>] -- <command...>");
                return ExitCode::SUCCESS;
            }
            "--" => {
                command.extend(arguments.by_ref());
                break;
            }
            other => {
                eprintln!("stale-artifacts: unknown argument {other}");
                return ExitCode::from(2);
            }
        }
    }
    if command.is_empty() {
        eprintln!(
            "stale-artifacts: the command about to run has to be given after `--`, \
             because which workspace has to be fresh is a property of that command"
        );
        return ExitCode::from(2);
    }

    let plan = stale_artifacts::plan(&at, &command);
    say(&plan);
    let Plan::Freshen(freshen) = &plan else {
        // `Nothing` is a run of this pass and `Unreadable` is not one. Both have
        // already said which they are, above.
        return match plan {
            Plan::Unreadable(_) => ExitCode::from(2),
            _ => ExitCode::SUCCESS,
        };
    };
    match stale_artifacts::apply(&at, freshen) {
        Ok(ran) => {
            for command in ran {
                println!("[stale-artifacts] ran {command}");
            }
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("[stale-artifacts] NOT RUN — {message}");
            ExitCode::from(2)
        }
    }
}

/// EVERY RUN PRINTS ITS NUMBERS, including the ones with nothing to do. This
/// pass is invisible when it works, its whole population comes from two other
/// programs' answers, and the state it protects against is a rebuild that did
/// not happen — so a silent success and a silent nothing-was-read look the same
/// in the one place anybody looks, which is the log the wrapper keeps.
fn say(plan: &Plan) {
    for line in stale_artifacts::report_lines(plan) {
        println!("[stale-artifacts] {line}");
    }
    if let Plan::Unreadable(_) = plan {
        eprintln!(
            "[stale-artifacts] this pass did not run. That is NOT the same as a \
             tree with nothing to clean: nothing here says an artifact of the \
             code about to be tested is gone."
        );
    }
}
