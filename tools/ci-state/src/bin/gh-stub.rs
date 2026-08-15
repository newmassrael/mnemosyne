//! A `gh` that answers the two endpoints this reporter asks for, out of files on
//! disk — and CHECKS THE REQUEST as well as answering it.
//!
//! A stub that answers whatever it is asked lets the program hit the wrong
//! endpoint and still be believed (R1132's correction to the hook suite's own
//! stub), so anything asked wrongly is appended to `GH_STUB_LOG` and the test
//! asserts that file is empty.
//!
//! # Why this is a cargo-built program rather than a script the test writes
//!
//! Round 1192: a file this process wrote and then ran cannot be exec'd while any
//! process holds it open for writing, and the holder is a SIBLING test's fork
//! rather than this thread. The failure is `ETXTBSY`, it arrives only when
//! something else is running, and it reads as a flake in whichever crate was
//! unlucky. Cargo builds this before any test starts, the fixture reaches it by
//! SYMLINK, and what varies per run is the environment below.
//!
//! It also settles what the shell version had to work around by hand. `PATH` is
//! the stub's directory and NOTHING ELSE — that is the point of the fixture, so
//! "gh is not installed" is a state the tests can produce rather than assert
//! about — and the script therefore had to spell `bash` and `cat` as absolute
//! paths, having failed on its first run when `env` could not find `bash`. A
//! binary has no interpreter to find.
//!
//! | variable | what it names |
//! |---|---|
//! | `GH_STUB_LOG` | where a wrongly-asked request is recorded |
//! | `GH_STUB_SHA` | the commit the check-run request must name |
//! | `GH_STUB_CHECK` | the check the annotation request must name |
//! | `GH_STUB_CHECKS` | the recorded body for the check-run list |
//! | `GH_STUB_ANNOTATIONS` | the recorded body for the annotation list |

use std::io::Write as _;
use std::process::ExitCode;

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let asked = arguments.join(" ");

    if arguments.first().map(String::as_str) != Some("api") {
        violate(&format!("gh was not asked for the api: {asked}"));
    }

    // ORDERED, as the shell `case` it replaces was.
    if asked.contains("--paginate") && asked.ends_with("/check-runs") {
        let sha = std::env::var("GH_STUB_SHA").unwrap_or_default();
        if !asked.contains(&format!("commits/{sha}/check-runs")) {
            violate(&format!(
                "the check-run request does not name the commit: {asked}"
            ));
        }
        return answer_with("GH_STUB_CHECKS");
    }
    if let Some((_, after)) = asked.split_once("/check-runs/") {
        if after.contains("/annotations") {
            let check = std::env::var("GH_STUB_CHECK").unwrap_or_default();
            if !asked.contains(&format!("check-runs/{check}/annotations")) {
                violate(&format!(
                    "the annotation request names the wrong check: {asked}"
                ));
            }
            return answer_with("GH_STUB_ANNOTATIONS");
        }
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

/// Hand over the recorded body named by `variable`, byte for byte — unfiltered,
/// which is the whole difference from the hook suite this replaced.
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
