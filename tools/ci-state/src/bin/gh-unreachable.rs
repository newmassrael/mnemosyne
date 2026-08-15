//! A `gh` that is INSTALLED and cannot answer — no network, no credential.
//!
//! The third state of the world this reporter has to tell apart: gh absent, gh
//! present and failing, gh present and answering. A separate program rather than
//! a mode of `gh-stub`, because a fixture that selects behaviour by an
//! environment variable makes every case read the branch it is not in.
//!
//! Cargo builds it and the fixture reaches it by SYMLINK, for the reason
//! `gh-stub.rs` gives at length: nothing here writes a file it then runs.

fn main() -> std::process::ExitCode {
    eprintln!("could not resolve host");
    std::process::ExitCode::from(1)
}
