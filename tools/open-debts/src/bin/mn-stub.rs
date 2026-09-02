//! A stand-in for `scripts/mn` that records what it was asked and answers from
//! a file — the fixture resolver `tests/asking.rs` reaches by symlink.
//!
//! WHY A BINARY CARGO BUILDS RATHER THAN A SCRIPT THE TEST WRITES. The first
//! version of that fixture wrote a bash script and chmod'd it 0755, and this
//! repository's `written-executable` gate refused it with the reason: `exec` on
//! a file some process still holds open for writing fails with `ETXTBSY`, the
//! holder is another test's fork rather than this thread, and so the failure
//! lands in a crate that did nothing, only while something else is running. So
//! the program is built once by cargo, reached by a symlink where the name
//! `scripts/mn` is required, and everything that varies per case lives in a data
//! file beside it — data cannot be busy.
//!
//! AND IT FINDS THAT FILE BY ITS WORKING DIRECTORY, which is the one thing a
//! program reached through a symlink can trust: `argv[0]` is the name `execvp`
//! was given and `/proc/self/exe` is the cargo target, neither of which is the
//! fixture. The census runs its resolver with the working directory set to the
//! repository it is asking about — that is the real resolver's contract too, and
//! it is exactly the directory this stub's answers sit in.

use std::fs::OpenOptions;
use std::io::Write;
use std::process::ExitCode;

/// What the census asked, in the form the answers file keys on.
///
/// TWO QUESTIONS AND NOTHING ELSE, because those are the two the census asks: a
/// readiness probe about the changelog as a whole, and one round. Anything else
/// falls through to the default refusal rather than being answered generously —
/// a stub that says yes to a question the real resolver has never seen would let
/// a test pass on a call that could not happen.
fn key_of(arguments: &[String]) -> Option<String> {
    if arguments.first().map(String::as_str) != Some("query") {
        return None;
    }
    match arguments.get(1).map(String::as_str) {
        Some("--list-changelog") => Some("probe".to_string()),
        Some("--changelog-entry") => arguments.get(2).cloned(),
        _ => None,
    }
}

/// The exit code this fixture gives that question, or 1.
///
/// ONE IS THE DEFAULT BECAUSE ONE IS WHAT THE REAL CLI SAYS to everything it
/// cannot answer — "not in the store", "no `mnemosyne.toml`", "the build
/// failed" — which is the very conflation the census's probe exists to survive.
fn answer(answers: &str, key: &str) -> u8 {
    for line in answers.lines() {
        let Some((name, code)) = line.split_once('\t') else {
            continue;
        };
        if name == key {
            return code.trim().parse().unwrap_or(1);
        }
    }
    1
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let here = match std::env::current_dir() {
        Ok(here) => here,
        Err(why) => {
            eprintln!("[mn-stub] no working directory to answer out of: {why}");
            return ExitCode::from(2);
        }
    };
    // THE RECORD IS WRITTEN BEFORE THE ANSWER, so a question this stub refuses
    // is still a question the test can see was asked. "The gate refused the row"
    // and "the gate asked the store" are different claims and only the second
    // one says anybody went and looked.
    match OpenOptions::new()
        .create(true)
        .append(true)
        .open(here.join("asked.log"))
    {
        Ok(mut log) => {
            let _ = writeln!(log, "{}", arguments.join(" "));
        }
        Err(why) => {
            eprintln!("[mn-stub] the question could not be recorded: {why}");
            return ExitCode::from(2);
        }
    }
    let answers = std::fs::read_to_string(here.join("resolver.answers")).unwrap_or_default();
    let Some(key) = key_of(&arguments) else {
        return ExitCode::from(1);
    };
    ExitCode::from(answer(&answers, &key))
}
