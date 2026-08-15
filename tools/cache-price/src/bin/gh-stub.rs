//! A `gh` that answers the two endpoints this program asks for, out of files on
//! disk — and CHECKS THE REQUEST as well as answering it.
//!
//! A stub that answers whatever it is asked lets the program hit the wrong
//! endpoint and still be believed, so anything asked wrongly is appended to
//! `GH_STUB_LOG` and the test asserts that file is empty.
//!
//! # Why this is a cargo-built program rather than a script the test writes
//!
//! Round 1192: a file this process wrote and then ran cannot be exec'd while any
//! process holds it open for writing, and the holder is a SIBLING test's fork
//! rather than this thread. The failure is `ETXTBSY`, it arrives only when
//! something else is running, and it reads as a flake in whichever crate was
//! unlucky. Cargo builds this before any test starts, the fixture reaches it by
//! SYMLINK, and what varies per run is the environment below — data cannot be
//! busy.
//!
//! It also removes the hazard the shell version had to work around: that stub
//! resolved `bash` and `cat` through a `PATH` its own test had emptied, so both
//! were written as absolute paths. A binary has no interpreter to find.
//!
//! | variable | what it names |
//! |---|---|
//! | `GH_STUB_LOG` | where a wrongly-asked request is recorded |
//! | `GH_STUB_WANTED` | the sample size the program was given, which its request must ask for |
//! | `GH_STUB_RUNS` | the recorded body for the run list |
//! | `GH_STUB_JOBS` | the recorded body for the job list |

use std::io::Write as _;
use std::process::ExitCode;

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let asked = arguments.join(" ");

    if arguments.first().map(String::as_str) != Some("api") {
        violate(&format!("gh was not asked for the api: {asked}"));
    }

    // ORDERED, as the shell `case` it replaces was: the run list is looked for
    // first and the job list only if that did not match.
    if let Some(after) = asked.split_once("/actions/workflows/") {
        if after.1.contains("/runs?per_page=") {
            let wanted = std::env::var("GH_STUB_WANTED").unwrap_or_default();
            if !asked.contains(&format!("per_page={wanted}")) {
                violate(&format!(
                    "the run list asked for a different sample than the program was given: {asked}"
                ));
            }
            return answer_with("GH_STUB_RUNS");
        }
    }
    if asked.contains("--paginate") && asked.ends_with("/jobs") {
        return answer_with("GH_STUB_JOBS");
    }

    violate(&format!("gh hit an unexpected endpoint: {asked}"));
    ExitCode::from(1)
}

/// Record a request that broke the contract. The test reads this file.
///
/// A MISSING LOG IS FATAL rather than silent: a stub whose violations went
/// nowhere would let every contract breach pass as a clean run, which is the
/// failure this file exists to prevent.
fn violate(what: &str) {
    let Ok(log) = std::env::var("GH_STUB_LOG") else {
        eprintln!("gh-stub: GH_STUB_LOG is not set, so a contract violation has nowhere to go");
        std::process::exit(3);
    };
    let mut file = match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log)
    {
        Ok(file) => file,
        Err(why) => {
            eprintln!("gh-stub: cannot record a violation in {log}: {why}");
            std::process::exit(3);
        }
    };
    if let Err(why) = writeln!(file, "{what}") {
        eprintln!("gh-stub: cannot record a violation in {log}: {why}");
        std::process::exit(3);
    }
}

/// Hand over the recorded body named by `variable`, byte for byte.
fn answer_with(variable: &str) -> ExitCode {
    let Ok(path) = std::env::var(variable) else {
        eprintln!("gh-stub: {variable} names no recording");
        return ExitCode::from(1);
    };
    match std::fs::read(&path) {
        Ok(bytes) => {
            if let Err(why) = std::io::stdout().write_all(&bytes) {
                eprintln!("gh-stub: cannot hand over {path}: {why}");
                return ExitCode::from(1);
            }
            ExitCode::SUCCESS
        }
        Err(why) => {
            eprintln!("gh-stub: cannot read {path}: {why}");
            ExitCode::from(1)
        }
    }
}
