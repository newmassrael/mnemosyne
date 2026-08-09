//! `RUSTC_WRAPPER`: record the invocation, then become `rustc`.
//!
//! cargo runs this as `rustc-log <real rustc> <arguments…>`, so the whole job —
//! including the builds a gate spawns for itself — flows through here. See the
//! crate documentation for what it deliberately cannot see.

use std::path::PathBuf;
use std::process::Command;

use rustc_log::LOG_VARIABLE;

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

    if let Err(error) = rustc_log::append(&log, &argv) {
        eprintln!("rustc-log: cannot append to {}: {error}", log.display());
        std::process::exit(2);
    }

    // BECOME rustc rather than waiting on it: one fewer process alive for the
    // whole of every compilation, and cargo's view of signals and exit codes is
    // exactly what it would be with no wrapper at all.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let error = Command::new(compiler).args(arguments).exec();
        eprintln!("rustc-log: cannot exec {compiler}: {error}");
        std::process::exit(2);
    }

    #[cfg(not(unix))]
    {
        let status = Command::new(compiler)
            .args(arguments)
            .status()
            .unwrap_or_else(|error| panic!("rustc-log: cannot run {compiler}: {error}"));
        std::process::exit(status.code().unwrap_or(1));
    }
}
