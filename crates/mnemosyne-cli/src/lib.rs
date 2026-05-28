//! `mnemosyne-cli` library — the bin's arg-parsing + stdout-printing
//! command handlers (`atomic_cli`), shared with integration tests.
//!
//! The reusable in-process orchestration (mutate / query / validate /
//! cascade) lives in the separate `mnemosyne-ops` crate (R319), which both
//! this bin and `mnemosyne-mcp` depend on. The CLI bin keeps only its own
//! I/O concerns here; it does not re-export ops.

pub mod atomic_cli;
