//! `mnemosyne-cli` library API — in-process surface for callers that need
//! atomic mutate + query semantics without the subprocess + JSON-parse
//! overhead of the CLI binary.
//!
//! The `mnemosyne-cli` bin is a thin arg-parsing wrapper over this
//! surface; the MCP server (`mnemosyne-mcp`) links against it directly so
//! each `#[tool]` invocation calls a Rust function, not a forked process.
//!
//! R316 split: subprocess spawn pattern in `mnemosyne-mcp` was self-
//! contradictory for a project whose North Star is in-process LLM
//! infrastructure (every tool call paid fork + arg parse + JSON round-trip).
//! This library exposes typed operations that both the CLI bin and the
//! MCP server share, while the bin keeps responsibility for arg parsing
//! and stdout printing.

pub mod atomic_cli;
pub mod ops;

pub use ops::{
    emit_publishable_override_ledger_draft, load_atomic_store, redact_term, resolve_sidecar,
    run_atomic_mutate, MutateOutcome, OpError, RedactTermInput,
};
