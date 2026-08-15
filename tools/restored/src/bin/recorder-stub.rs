//! A stand-in for the recorder `RUSTC_WRAPPER` names, which the `after` step asks
//! what it was built from.
//!
//! It answers `--built-from` with a revision nothing else in the suite could have
//! produced, and refuses anything else — that refusal is what makes the case an
//! oracle rather than an echo: the value has to come from the RECORDER, so a
//! program that derived it for itself would print the same string twice and
//! could not notice a substituted binary at all.
//!
//! # Why this is a cargo-built program rather than a script the test writes
//!
//! Round 1192: a file this process wrote and then ran cannot be exec'd while any
//! process holds it open for writing, and the holder is a SIBLING test's fork
//! rather than this thread. The failure is `ETXTBSY`, it arrives only when
//! something else is running, and it reads as a flake in whichever crate was
//! unlucky. Cargo builds this before any test starts and the test names it with
//! `env!("CARGO_BIN_EXE_recorder-stub")`.

use std::process::ExitCode;

/// The answer. `tests/entrances.rs` asserts this exact string, because what the
/// case is about is that the value travelled from HERE — a binary's constants
/// are not reachable from a test, so the two spellings are held together by that
/// assertion failing loudly rather than by a shared name.
const BUILT_FROM: &str = "cafe1234";

fn main() -> ExitCode {
    if std::env::args().nth(1).as_deref() != Some("--built-from") {
        return ExitCode::from(3);
    }
    println!("{BUILT_FROM}");
    ExitCode::SUCCESS
}
