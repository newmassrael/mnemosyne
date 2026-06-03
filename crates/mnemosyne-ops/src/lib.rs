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
pub mod query;
pub mod style;
pub mod validate;

use std::path::{Path, PathBuf};

use mnemosyne_atomic::{AtomicMutateError, AtomicMutateReceipt, AtomicStore};
use serde::Serialize;
use thiserror::Error;

pub use cascade::{validate_atomic_store, AtomicValidationSummary};
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

/// Outcome of a successful atomic mutate — the receipt the primitive
/// produced. The atomic store is the only artifact; there is nothing to
/// regenerate.
#[derive(Debug, Clone, Serialize)]
pub struct MutateOutcome {
    pub receipt: AtomicMutateReceipt,
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
/// `<workspace>/docs/.atomic/workspace.atomic.json`. `anchor` is a discovery
/// start; workspace-relative paths join the config-declared `[workspace]
/// root` (see [`cascade::workspace_root_from`]), so this delegates fully to
/// the anchor-aware cascade resolver rather than joining to `anchor`.
pub fn resolve_sidecar(anchor: &Path, sidecar: Option<&Path>) -> anyhow::Result<PathBuf> {
    let s = sidecar.map(|p| p.to_string_lossy().into_owned());
    cascade::resolve_sidecar(anchor, s.as_deref())
}

/// Run an atomic mutate primitive in-process: load the store, invoke the
/// supplied closure against it, and return the receipt. The closure
/// persists the store itself (`save_with_receipt`); the atomic store is the
/// only artifact, so there is nothing further to regenerate.
pub fn run_atomic_mutate<F>(
    workspace_root: &Path,
    sidecar: Option<&Path>,
    primitive: F,
) -> Result<MutateOutcome, OpError>
where
    F: FnOnce(&mut AtomicStore, &Path) -> Result<AtomicMutateReceipt, AtomicMutateError>,
{
    let sidecar_path = resolve_sidecar(workspace_root, sidecar)?;
    let mut store =
        AtomicStore::load(&sidecar_path).map_err(|e| OpError::Other(format!("{}", e)))?;
    let receipt = primitive(&mut store, &sidecar_path)?;
    Ok(MutateOutcome { receipt })
}

/// Load the atomic store at the resolved sidecar path.
///
/// A missing sidecar is NOT an error — `AtomicStore::load` already returns an
/// empty store for a fresh workspace. This propagates only genuine failures
/// (corrupt JSON, IO error, or a newer-than-supported `schema_version`) so a
/// corrupt SSOT fails loud instead of silently reading as empty (the prior
/// `unwrap_or_default` masked corruption as a clean empty store).
pub fn load_atomic_store(
    workspace_root: &Path,
    sidecar: Option<&Path>,
) -> Result<AtomicStore, OpError> {
    let sidecar_path = resolve_sidecar(workspace_root, sidecar)?;
    AtomicStore::load(&sidecar_path).map_err(|e| OpError::Other(format!("{}", e)))
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
    let sidecar_path = resolve_sidecar(workspace_root, sidecar)?;
    let mut store =
        AtomicStore::load(&sidecar_path).map_err(|e| OpError::Other(format!("{}", e)))?;
    let report = mnemosyne_atomic::redact_term(&mut store, &sidecar_path, &req)?;
    // Inert (no GENERATED.md to regenerate); flag removed in the cleanup round.
    let _ = regenerate;
    Ok((report, false))
}

/// Scan code citations for now-stale references to `inventory_id` —
/// mirrors the CLI's `print_inventory_decay_trigger` cascade (R276) but
/// returns the hits instead of printing to stderr. Empty when the
/// workspace has no `[plugins.set_equality_validator]` inventory config.
pub fn inventory_decay_scan(
    workspace_root: &Path,
    inventory_id: &str,
) -> anyhow::Result<Vec<mnemosyne_validate::code_refs::Citation>> {
    // A malformed mnemosyne.toml fails loud (matches the R362 resolver
    // fail-fast); Ok(None) = no config file = nothing to scan.
    let Some(loaded) = mnemosyne_config::discover_config(workspace_root)? else {
        return Ok(Vec::new());
    };
    let Some(cfg) = loaded
        .config
        .plugins
        .as_ref()
        .and_then(|p| p.set_equality_validator.as_ref())
    else {
        return Ok(Vec::new());
    };
    if cfg.paths.is_empty()
        || (cfg.inventory_prefixes.is_empty() && cfg.inventory_path_prefixes.is_empty())
    {
        return Ok(Vec::new());
    }
    // An unreadable scan path fails loud rather than reporting "no decay" —
    // the `scan_section_decay` sibling the R360 fail-loud sweep missed.
    let hits = mnemosyne_validate::code_refs::scan_inventory_decay(
        workspace_root,
        &cfg.paths,
        inventory_id,
        &cfg.inventory_prefixes,
        &cfg.inventory_path_prefixes,
        cfg.comment_only,
    )?;
    Ok(hits)
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
    let sidecar_path = resolve_sidecar(workspace_root, sidecar)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// A fresh workspace with no sidecar file loads as an empty store — a
    /// missing sidecar is a legitimate state, not an error.
    #[test]
    fn load_atomic_store_missing_sidecar_is_empty_not_error() {
        let tmp = TempDir::new().unwrap();
        let store =
            load_atomic_store(tmp.path(), None).expect("missing sidecar must load as empty");
        assert!(store.atomic_section_id_set().is_empty());
    }

    /// A corrupt sidecar must propagate the error, not silently read as an
    /// empty store. Regression for the `unwrap_or_default` that previously
    /// masked corruption (R356).
    #[test]
    fn load_atomic_store_corrupt_sidecar_propagates_error() {
        let tmp = TempDir::new().unwrap();
        let sidecar = AtomicStore::default_sidecar_path(tmp.path());
        std::fs::create_dir_all(sidecar.parent().unwrap()).unwrap();
        std::fs::write(&sidecar, b"{ this is not valid json").unwrap();
        assert!(
            load_atomic_store(tmp.path(), None).is_err(),
            "corrupt sidecar must fail loud, not silently empty"
        );
    }

    /// No config file = nothing to scan = an empty hit set, not an error.
    #[test]
    fn inventory_decay_scan_missing_config_is_empty_not_error() {
        let tmp = TempDir::new().unwrap();
        let hits = inventory_decay_scan(tmp.path(), "X").expect("missing config = empty");
        assert!(hits.is_empty());
    }

    /// A malformed mnemosyne.toml fails loud instead of silently reporting
    /// "no decay" — regression for the R360/R362 sibling swallows the R364
    /// sweep closed (`let Ok(Some) = discover_config` + `unwrap_or_default`).
    #[test]
    fn inventory_decay_scan_malformed_config_fails_loud() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("mnemosyne.toml"), "[plugins\nbad = ").unwrap();
        assert!(
            inventory_decay_scan(tmp.path(), "X").is_err(),
            "malformed config must fail loud, not silently empty"
        );
    }
}
