//! `mnemosyne-ops` — shared in-process orchestration consumed by the
//! `mnemosyne-cli` bin and the `mnemosyne-mcp` server.
//!
//! R316 eliminated the MCP→CLI subprocess spawn; R319 extracts the
//! orchestration into this dedicated library so neither binary depends on
//! the other. Both link `mnemosyne-ops` and call typed Rust functions:
//! mutate via [`run_atomic_mutate`], reads via [`query`] / [`validate`] /
//! [`style`], cascade render via [`cascade`]. The bins keep only their own
//! I/O concerns (arg parsing + stdout for the CLI; MCP protocol for the
//! server).

pub mod cascade;
pub mod docs;
pub mod query;
pub mod style;
pub mod validate;

use std::path::{Path, PathBuf};

use mnemosyne_atomic::{AtomicMutateError, AtomicMutateReceipt, AtomicStore};
use serde::Serialize;
use thiserror::Error;

pub use cascade::{
    auto_regenerate, render_atomic_store_to_md, resolve_output, validate_atomic_store,
    write_generated_md, AtomicValidationSummary,
};
pub use docs::{generate_docs, verify_generated, GenerateDocsReport, VerifyGeneratedReport};
pub use query::{
    list_inventory, list_sections, query_inventory, query_section, query_term, InventoryEntryView,
    ListSectionsReport, QuerySectionMode, QueryTermInput,
};
pub use style::{style_check, StyleCheckInput, StyleCheckReport};
pub use validate::{validate_workspace, ValidateWorkspaceReport};

/// Errors surfaced from any op. Thin wrapper that preserves the structured
/// `AtomicMutateError` variant so callers (mcp) can map cleanly to MCP
/// error categories without reparsing strings.
#[derive(Debug, Error)]
pub enum OpError {
    #[error("{0}")]
    Mutate(#[from] AtomicMutateError),
    #[error("redact: {0}")]
    Redact(#[from] mnemosyne_atomic::RedactError),
    #[error("{0}")]
    Other(String),
}

impl From<anyhow::Error> for OpError {
    fn from(e: anyhow::Error) -> Self {
        OpError::Other(format!("{:#}", e))
    }
}

impl From<std::io::Error> for OpError {
    fn from(e: std::io::Error) -> Self {
        OpError::Other(format!("io: {}", e))
    }
}

/// Outcome of a successful atomic mutate. Includes the receipt the
/// primitive produced plus a flag indicating whether the cascade
/// auto-regeneration of `GENERATED.md` ran (it does by default; callers
/// can opt out for batch use cases).
#[derive(Debug, Clone, Serialize)]
pub struct MutateOutcome {
    pub receipt: AtomicMutateReceipt,
    pub regenerated: bool,
}

/// Input to the convenience-form `redact_term` op.
#[derive(Debug, Clone, Serialize)]
pub struct RedactTermInput {
    pub pattern: String,
    pub replacement: String,
    pub regex: bool,
    pub case_insensitive: bool,
    pub scope: Option<String>,
    pub dry_run: bool,
    pub reason: String,
    pub applied_in: String,
    pub kind: Option<String>,
}

/// Resolve the sidecar path with the same precedence chain the CLI uses:
/// explicit override → `[atomic] sidecar_path` config → built-in
/// `<workspace>/docs/.atomic/workspace.atomic.json`.
pub fn resolve_sidecar(workspace_root: &Path, sidecar: Option<&Path>) -> PathBuf {
    match sidecar {
        Some(p) if p.is_absolute() => p.to_path_buf(),
        Some(p) => workspace_root.join(p),
        None => cascade::resolve_sidecar(workspace_root, None),
    }
}

/// Run an atomic mutate primitive in-process: load the store, invoke the
/// supplied closure against it, then (by default) regenerate
/// `GENERATED.md` so cascade stays in sync. Returns a structured outcome
/// instead of printing.
pub fn run_atomic_mutate<F>(
    workspace_root: &Path,
    sidecar: Option<&Path>,
    regenerate: bool,
    primitive: F,
) -> Result<MutateOutcome, OpError>
where
    F: FnOnce(&mut AtomicStore, &Path) -> Result<AtomicMutateReceipt, AtomicMutateError>,
{
    let sidecar_path = resolve_sidecar(workspace_root, sidecar);
    let mut store =
        AtomicStore::load(&sidecar_path).map_err(|e| OpError::Other(format!("{}", e)))?;
    let receipt = primitive(&mut store, &sidecar_path)?;
    if regenerate {
        cascade::auto_regenerate(workspace_root, sidecar_to_str(sidecar).as_deref())
            .map_err(|e| OpError::Other(format!("{:#}", e)))?;
    }
    Ok(MutateOutcome {
        receipt,
        regenerated: regenerate,
    })
}

/// Load the atomic store at the resolved sidecar path. Returns
/// `AtomicStore::default()` when the sidecar file is missing — matches
/// the CLI's tolerant read semantics for fresh workspaces.
pub fn load_atomic_store(workspace_root: &Path, sidecar: Option<&Path>) -> AtomicStore {
    let sidecar_path = resolve_sidecar(workspace_root, sidecar);
    AtomicStore::load(&sidecar_path).unwrap_or_default()
}

fn sidecar_to_str(sidecar: Option<&Path>) -> Option<String> {
    sidecar.map(|p| p.to_string_lossy().into_owned())
}

/// Run the convenience-form redact_term primitive (R297). Mirrors
/// `mnemosyne-cli redact-term` semantics but returns the structured
/// report instead of printing it.
pub fn redact_term(
    workspace_root: &Path,
    sidecar: Option<&Path>,
    regenerate: bool,
    input: &RedactTermInput,
) -> Result<(mnemosyne_atomic::RedactionReport, bool), OpError> {
    use mnemosyne_atomic::{RedactMode, RedactRequest, RedactScope};
    let mode = if input.regex {
        RedactMode::Regex
    } else {
        RedactMode::Literal
    };
    let scope = match input.scope.as_deref().unwrap_or("all") {
        "all" => RedactScope::All,
        "decision_summary" | "publishable_decision_summary" => RedactScope::DecisionSummary,
        "changes_bullets" | "publishable_changes_bullets" => RedactScope::ChangesBullets,
        "verification_bullets" | "publishable_verification_bullets" => {
            RedactScope::VerificationBullets
        }
        "impact_refs" | "publishable_impact_refs" => RedactScope::ImpactRefs,
        "carry_forward_bullets" | "publishable_carry_forward_bullets" => {
            RedactScope::CarryForwardBullets
        }
        other => {
            return Err(OpError::Other(format!(
                "unknown scope `{}` — expected: all | decision_summary | changes_bullets \
                 | verification_bullets | impact_refs | carry_forward_bullets",
                other
            )));
        }
    };
    let req = RedactRequest {
        pattern: input.pattern.clone(),
        replacement: input.replacement.clone(),
        mode,
        case_insensitive: input.case_insensitive,
        scope,
        dry_run: input.dry_run,
        reason: input.reason.clone(),
        applied_in: input.applied_in.clone(),
        kind: input
            .kind
            .clone()
            .unwrap_or_else(|| "redaction".to_string()),
    };
    let sidecar_path = resolve_sidecar(workspace_root, sidecar);
    let mut store =
        AtomicStore::load(&sidecar_path).map_err(|e| OpError::Other(format!("{}", e)))?;
    let report = mnemosyne_atomic::redact_term(&mut store, &sidecar_path, &req)?;
    let did_regenerate = if regenerate && !report.dry_run {
        cascade::auto_regenerate(workspace_root, sidecar_to_str(sidecar).as_deref())
            .map_err(|e| OpError::Other(format!("{:#}", e)))?;
        true
    } else {
        false
    };
    Ok((report, did_regenerate))
}

/// Scan code citations for now-stale references to `inventory_id` —
/// mirrors the CLI's `print_inventory_decay_trigger` cascade (R276) but
/// returns the hits instead of printing to stderr. Empty when the
/// workspace has no `[plugins.set_equality_validator]` inventory config.
pub fn inventory_decay_scan(
    workspace_root: &Path,
    inventory_id: &str,
) -> Vec<mnemosyne_validate::code_refs::Citation> {
    let Ok(Some(loaded)) = mnemosyne_config::discover_config(workspace_root) else {
        return Vec::new();
    };
    let Some(cfg) = loaded
        .config
        .plugins
        .as_ref()
        .and_then(|p| p.set_equality_validator.as_ref())
    else {
        return Vec::new();
    };
    if cfg.paths.is_empty()
        || (cfg.inventory_prefixes.is_empty() && cfg.inventory_path_prefixes.is_empty())
    {
        return Vec::new();
    }
    mnemosyne_validate::code_refs::scan_inventory_decay(
        workspace_root,
        &cfg.paths,
        inventory_id,
        &cfg.inventory_prefixes,
        &cfg.inventory_path_prefixes,
        cfg.comment_only,
    )
    .unwrap_or_default()
}

/// Emit a `[[publishable_override_ledger]]` draft for an entry whose
/// publishable half currently diverges from the audit half (R300).
pub fn emit_publishable_override_ledger_draft(
    workspace_root: &Path,
    sidecar: Option<&Path>,
    entry_id: &str,
    reason: &str,
    applied_in: &str,
    kind: Option<&str>,
) -> Result<Option<String>, OpError> {
    let sidecar_path = resolve_sidecar(workspace_root, sidecar);
    let store = AtomicStore::load(&sidecar_path).map_err(|e| OpError::Other(format!("{}", e)))?;
    let draft = mnemosyne_atomic::emit_publishable_override_ledger_draft(
        &store,
        entry_id,
        reason,
        applied_in,
        kind.unwrap_or("redaction"),
    )?;
    Ok(draft)
}
