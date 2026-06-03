//! Sidecar path-resolution + atomic-store referential validation. The atomic
//! store is the single validated SSOT; there is no rendered `GENERATED.md`
//! derivation here (the store→markdown render path was removed once the store
//! became the directly-validated artifact). `validate_atomic_store` is the
//! shared referential-closure check consumed by validate-workspace.
//!
//! Moved here from `mnemosyne-cli/src/atomic_cli.rs` (R319) so both
//! binaries depend on one orchestration crate rather than mcp linking the
//! CLI binary's library half.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use mnemosyne_atomic::AtomicStore;
use mnemosyne_config::discover_config;

/// Resolve the workspace root from any directory inside the workspace.
///
/// `anchor` is a discovery start (the directory a command was invoked from,
/// or the directory holding `mnemosyne.toml`); the returned path is the
/// config-declared `[workspace] root` (or the config dir when unset). All
/// workspace-relative paths — sidecar, output, doc, citation, implementation
/// — resolve against THIS root, never against `anchor`, so a ledger rooted
/// in a subdirectory (`[workspace] root = "../../.."`) still resolves code
/// paths repo-relative. When no config is discoverable the anchor itself is
/// the root (the built-in-default / test path); a malformed config fails loud.
pub fn workspace_root_from(anchor: &Path) -> Result<PathBuf> {
    Ok(discover_config(anchor)?
        .map(|l| l.workspace_root)
        .unwrap_or_else(|| anchor.to_path_buf()))
}

/// Resolve sidecar path with the Round 279 precedence chain:
/// 1. Explicit `--sidecar` CLI flag wins absolutely.
/// 2. `[atomic] sidecar_path` from `mnemosyne.toml` (workspace-relative
///    or absolute) when discoverable.
/// 3. Default `<workspace_root>/docs/.atomic/workspace.atomic.json`.
///
/// Workspace-relative paths join the config-declared `[workspace] root`, not
/// `anchor` — see [`workspace_root_from`].
pub fn resolve_sidecar(anchor: &Path, sidecar: Option<&str>) -> Result<PathBuf> {
    // Explicit override short-circuits before discovery — a malformed
    // `mnemosyne.toml` must not block an explicitly-pathed resolve. A
    // relative override joins the anchor directly (the dir the command was
    // invoked against).
    if let Some(p) = sidecar {
        let pb = PathBuf::from(p);
        return Ok(if pb.is_absolute() {
            pb
        } else {
            anchor.join(pb)
        });
    }
    // No override: a malformed config propagates loud rather than silently
    // falling back to the default (R356/R359 corrupt-store sweep). The
    // `[atomic]` / default paths join the config-declared root, not anchor.
    let loaded = discover_config(anchor)?;
    let root = loaded
        .as_ref()
        .map(|l| l.workspace_root.as_path())
        .unwrap_or(anchor);
    if let Some(cfg_path) = loaded
        .as_ref()
        .and_then(|l| l.config.atomic.as_ref())
        .and_then(|a| a.sidecar_path.as_deref())
    {
        let pb = PathBuf::from(cfg_path);
        return Ok(if pb.is_absolute() { pb } else { root.join(pb) });
    }
    Ok(AtomicStore::default_sidecar_path(root))
}

/// Atomic-first validation summary — shape consumed by validate-workspace.
#[derive(Debug, Clone)]
pub struct AtomicValidationSummary {
    pub entries: usize,
    pub sections: usize,
    /// `(entry_id, target_section_id)` pairs whose target is NOT in the
    /// supplied workspace section id set.
    pub orphan_entry_refs: Vec<(String, String)>,
    /// `(section_id, target_section_id)` pairs whose target is NOT in the
    /// supplied workspace section id set.
    pub orphan_section_refs: Vec<(String, String)>,
}

/// Validate the atomic store against the supplied workspace section id set.
/// Pure read — no file writes, side effect free. Shared by
/// validate-workspace and audit ledgers as the single audit definition.
pub fn validate_atomic_store(
    anchor: &Path,
    section_id_set: &BTreeSet<String>,
) -> Result<AtomicValidationSummary> {
    // Honor `[atomic].sidecar_path` config so the read / validation path
    // sees the same store the mutate path wrote to. `anchor` is a discovery
    // start.
    let sidecar_path = resolve_sidecar(anchor, None)?;
    let store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;

    let mut orphan_entry_refs = Vec::new();
    for (entry_id, entry) in &store.changelog_entries {
        for r in &entry.impact_refs {
            if !section_id_set.contains(r) {
                orphan_entry_refs.push((entry_id.clone(), r.clone()));
            }
        }
    }
    let mut orphan_section_refs = Vec::new();
    for (section_id, atomic) in &store.sections {
        for r in &atomic.impact_scope {
            if !section_id_set.contains(r) {
                orphan_section_refs.push((section_id.clone(), r.clone()));
            }
        }
        // R344: the supersession forward-pointer is a section cross-ref too —
        // its target must resolve, the existence check set_section_decision_status
        // defers here (R342). Without this, a Superseded section pointing at a
        // phantom §M would pass the supersede-state gate (a decision ref exists)
        // yet dangle.
        if let Some(target) = &atomic.superseded_by {
            if !section_id_set.contains(target) {
                orphan_section_refs.push((section_id.clone(), target.clone()));
            }
        }
    }

    Ok(AtomicValidationSummary {
        entries: store.changelog_entries.len(),
        sections: store.sections.len(),
        orphan_entry_refs,
        orphan_section_refs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Write a workspace whose `mnemosyne.toml` lives in a subdirectory and
    /// declares `[workspace] root = ".."`, so the discovery anchor (the
    /// toml's dir) differs from the resolved root (its parent). Returns
    /// `(tempdir, subdir_anchor, canonical_declared_root)`.
    fn subdir_rooted_workspace(atomic_table: &str) -> (TempDir, PathBuf, PathBuf) {
        let tmp = TempDir::new().unwrap();
        let anchor = tmp.path().join("ledger");
        fs::create_dir_all(&anchor).unwrap();
        fs::write(
            anchor.join("mnemosyne.toml"),
            format!(
                "[workspace]\ndocs = [\"a.md\"]\nroot = \"..\"\n{}",
                atomic_table
            ),
        )
        .unwrap();
        let root = tmp.path().canonicalize().unwrap();
        (tmp, anchor, root)
    }

    #[test]
    fn workspace_root_from_resolves_declared_root_not_anchor() {
        // R386 — the anchor is the subdir holding the toml; the resolved
        // root is its parent (`root = ".."`). Discovery must walk from the
        // anchor, not the resolved root (which would miss the subdir toml).
        let (_tmp, anchor, root) = subdir_rooted_workspace("");
        let got = workspace_root_from(&anchor).unwrap();
        assert_eq!(got.canonicalize().unwrap(), root);
    }

    #[test]
    fn resolve_sidecar_joins_declared_root_not_anchor() {
        // R386 regression — a workspace-relative sidecar resolves against the
        // config-declared `[workspace] root` (the parent), NOT the discovery
        // anchor (the subdir). Pre-R386 this joined the anchor, so a
        // subdir-rooted ledger read the wrong (empty) store.
        let (_tmp, anchor, root) = subdir_rooted_workspace("");
        let got = resolve_sidecar(&anchor, None).unwrap();
        // No [atomic] table → default sidecar under the DECLARED root.
        let expected = AtomicStore::default_sidecar_path(&root);
        assert_eq!(
            got.canonicalize().unwrap_or(got.clone()),
            expected.canonicalize().unwrap_or(expected.clone()),
            "got {} expected under declared root {}",
            got.display(),
            root.display()
        );
        // The resolved sidecar must NOT live under the subdir anchor.
        assert!(
            !got.starts_with(&anchor),
            "sidecar wrongly resolved under the anchor: {}",
            got.display()
        );
    }

    #[test]
    fn resolve_sidecar_explicit_relative_override_joins_anchor() {
        // An explicit `--sidecar` relative override short-circuits discovery
        // and joins the anchor directly (the dir the command ran against).
        let (_tmp, anchor, _root) = subdir_rooted_workspace("");
        let got = resolve_sidecar(&anchor, Some("custom/store.json")).unwrap();
        assert_eq!(got, anchor.join("custom/store.json"));
    }

    #[test]
    fn resolve_sidecar_atomic_relative_joins_declared_root() {
        // `[atomic] sidecar_path` (relative) also joins the declared root.
        let (_tmp, anchor, root) =
            subdir_rooted_workspace("[atomic]\nsidecar_path = \"store/x.json\"\n");
        let got = resolve_sidecar(&anchor, None).unwrap();
        assert_eq!(got, root.join("store/x.json"));
    }

    #[test]
    fn workspace_root_from_no_config_is_anchor() {
        // No discoverable config → the anchor itself is the root (built-in
        // default / test path).
        let tmp = TempDir::new().unwrap();
        let got = workspace_root_from(tmp.path()).unwrap();
        assert_eq!(got, tmp.path());
    }
}
