//! `RUSTC_WRAPPER`: run the compiler, time it, and write down what it was.
//!
//! cargo runs this as `rustc-log <real rustc> <arguments…>`, so the whole job —
//! including the builds a gate spawns for itself — flows through here. See the
//! crate documentation for what it deliberately cannot see.
//!
//! IT WAITS FOR THE COMPILER RATHER THAN BECOMING IT, which is the one thing
//! here that costs something, and it buys the duration. An `exec` replaces this
//! process, so the only moment left to write a record is before the compiler
//! starts — and a record written then can say what was compiled but never what
//! it cost. Counting compilations was enough to find that 50.3% of this CI's
//! work is done twice; it cannot say whether removing a given half of them is
//! worth a minute of wall-clock, and that is the question the number raised.
//!
//! What waiting costs is one more process alive per compilation, which next to a
//! `rustc` holding a crate graph in memory is nothing, and one more layer for a
//! signal to cross. The exit status is forwarded whole, including the signal
//! that killed the compiler, so cargo's view of a failed build is unchanged.

use std::path::PathBuf;
use std::process::{Command, ExitStatus};
use std::time::Instant;

use rustc_log::{Record, LOG_VARIABLE};

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let Some((compiler, arguments)) = argv.split_first() else {
        eprintln!(
            "rustc-log: cargo runs a `RUSTC_WRAPPER` as `<wrapper> <rustc> \
             <arguments…>` and this one was given no compiler to run"
        );
        std::process::exit(2);
    };

    // A REFUSAL RATHER THAN A QUIET PASS-THROUGH. A wrapper that records nothing
    // when it is misconfigured hands the gate downstream an empty log, and an
    // empty log reads exactly like a job that compiled nothing — the failure
    // this repository keeps meeting, where absence and cleanliness print the
    // same thing.
    let log = match std::env::var(LOG_VARIABLE) {
        Ok(path) if !path.is_empty() => PathBuf::from(path),
        _ => {
            eprintln!(
                "rustc-log: ${LOG_VARIABLE} names no file, so this invocation \
                 would go unrecorded — refusing rather than leaving a gap that \
                 reads as a job with nothing to compile"
            );
            std::process::exit(2);
        }
    };

    // THE WALL CLOCK FOR THE START AND THE MONOTONIC ONE FOR THE LENGTH. The
    // start has to be comparable against starts read by the other wrappers
    // running right now, which only a shared clock can be; the length must not
    // move because something stepped the shared one mid-compilation.
    let started_at = rustc_log::now_micros();
    let elapsing = Instant::now();
    let status = Command::new(compiler)
        .args(arguments)
        .status()
        .unwrap_or_else(|error| panic!("rustc-log: cannot run {compiler}: {error}"));
    let micros = u64::try_from(elapsing.elapsed().as_micros()).unwrap_or(u64::MAX);

    // WRITTEN EVEN WHEN THE COMPILATION FAILED. A build that broke still paid
    // for every `rustc` it ran, and the census is of what CI paid for.
    let record = Record {
        started_at,
        micros,
        argv: argv.clone(),
    };
    if let Err(error) = rustc_log::append(&log, &record) {
        eprintln!("rustc-log: cannot append to {}: {error}", log.display());
        std::process::exit(2);
    }

    std::process::exit(status.code().unwrap_or_else(|| killed_by_a_signal(&status)));
}

/// The exit code for a compiler that no code of its own ended.
///
/// `128 + signal` is the shell's convention and cargo reads it as the failure it
/// is. What matters is that it is NOT zero: a `rustc` the kernel killed —
/// out of memory, most often, on a runner — must not reach cargo wearing the
/// exit code of a compilation that worked.
#[cfg(unix)]
fn killed_by_a_signal(status: &ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt as _;
    128 + status.signal().unwrap_or(0)
}

#[cfg(not(unix))]
fn killed_by_a_signal(_status: &ExitStatus) -> i32 {
    1
}
