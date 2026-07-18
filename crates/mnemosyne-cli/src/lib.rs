//! `mnemosyne-cli` library — the bin's arg-parsing + stdout-printing
//! command handlers (`atomic_cli`), shared with integration tests.
//!
//! The reusable in-process orchestration (mutate / query / validate /
//! cascade) lives in the separate `mnemosyne-ops` crate (R319), which both
//! this bin and `mnemosyne-mcp` depend on. The CLI bin keeps only its own
//! I/O concerns here; it does not re-export ops.

pub mod atomic_cli;

/// The CLI's top-level error, threaded from every command up to `main`.
///
/// It carries the ONE decision `main` must make on a failure: print the
/// message, or stay silent because the command already wrote a fully
/// formatted error itself. The two dispositions are:
///
/// - [`CliError::Message`] — `main` prints `error: {:#}` (the full `anyhow`
///   chain) and exits non-zero. This is every ordinary failure; the blanket
///   `From<anyhow::Error>` makes `?` produce it, so command bodies keep using
///   `anyhow` throughout.
/// - [`CliError::AlreadyReported`] — the atomic-mutate path (`finalize_mutate`)
///   already emitted its own formatted error to stderr (the `--json` blob or
///   the `FAILED` header + detail), so `main` only sets the exit code.
///
/// This replaces the Round 684 `AlreadyReported` marker: a `pub` unit-struct
/// error smuggled through `anyhow::Error` and recovered in `main` by
/// `downcast_ref`. The variant is threaded by the compiler end to end, so
/// there is no `pub` marker crossing the module boundary, no `downcast`, and
/// no `Display`/`Error` impl kept alive only to satisfy a bound (the marker's
/// `Display` was dead by design — `main` suppressed it).
pub enum CliError {
    /// `main` prints `error: {:#}` then exits non-zero.
    Message(anyhow::Error),
    /// The command already printed its own formatted error; `main` only sets
    /// the exit code.
    AlreadyReported,
}

impl From<anyhow::Error> for CliError {
    fn from(error: anyhow::Error) -> Self {
        CliError::Message(error)
    }
}
