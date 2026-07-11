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

/// Resolve an EXPLICIT CLI path argument (Round 538) — the single source of
/// the rule every CLI path flag follows: a relative path is **CWD-relative**
/// (the universal CLI convention, the principle of least surprise — `cat`,
/// `cp`, `git apply`, `cargo`, and this tool's own `--manifest` all resolve a
/// typed path against the directory the user is standing in), an absolute path
/// is used verbatim. This is deliberately DIFFERENT from a config-DECLARED
/// path (`[atomic] sidecar_path`, `[continuity] canon_order_path`): a config
/// declaration is a project-level artifact and anchors to the workspace root,
/// not the CWD. Shared by `--sidecar` ([`resolve_sidecar`]) and `--order`
/// (`resolve_canon_order_file`) so the two cannot drift to different anchors.
pub fn resolve_explicit_cli_path(cwd: &Path, raw: &str) -> PathBuf {
    let pb = PathBuf::from(raw);
    if pb.is_absolute() {
        pb
    } else {
        cwd.join(pb)
    }
}

/// Resolve sidecar path with the Round 279 precedence chain (Round 538
/// CWD-correct on the explicit override):
/// 1. Explicit `--sidecar` CLI flag wins absolutely — resolved **CWD-relative**
///    (R538: a path typed on the command line is relative to where the user
///    stands; see [`resolve_explicit_cli_path`]).
/// 2. `[atomic] sidecar_path` from `mnemosyne.toml` (workspace-relative or
///    absolute) when discoverable — a config declaration, **workspace-rooted**.
/// 3. Default `<workspace_root>/docs/.atomic/workspace.atomic.json`.
///
/// `anchor` is the config-discovery start (the dir the command ran against, or
/// the dir holding `mnemosyne.toml`); the config/default branches resolve
/// against the discovered `[workspace] root`, never against `anchor` — see
/// [`workspace_root_from`]. The imperative shell: it injects the process CWD
/// (the invocation's defining context) into the pure [`resolve_sidecar_in`].
pub fn resolve_sidecar(anchor: &Path, sidecar: Option<&str>) -> Result<PathBuf> {
    let cwd =
        std::env::current_dir().map_err(|e| anyhow!("CWD lookup for sidecar resolution: {e}"))?;
    resolve_sidecar_in(&cwd, anchor, sidecar)
}

/// The pure core of [`resolve_sidecar`] (Round 538) — `cwd` is injected so the
/// explicit-override branch is deterministic and testable. The explicit
/// override is CWD-relative and never touches `anchor` (a CLI path is not
/// workspace-rooted); only the config/default branches discover + join the
/// workspace root.
fn resolve_sidecar_in(cwd: &Path, anchor: &Path, sidecar: Option<&str>) -> Result<PathBuf> {
    // Explicit override short-circuits before discovery — a malformed
    // `mnemosyne.toml` must not block an explicitly-pathed resolve.
    if let Some(p) = sidecar {
        return Ok(resolve_explicit_cli_path(cwd, p));
    }
    // No override: a malformed config propagates loud rather than silently
    // falling back to the default (R356/R359 corrupt-store sweep). The
    // `[atomic]` / default paths join the config-declared root (project-rooted).
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
            format!("[workspace]\nroot = \"..\"\n{}", atomic_table),
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
    fn resolve_explicit_cli_path_is_cwd_relative_or_absolute() {
        // R538 — the single explicit-CLI-path rule: a relative path is
        // CWD-relative (least surprise), an absolute path passes through.
        let cwd = Path::new("/home/u/run/author");
        assert_eq!(
            resolve_explicit_cli_path(cwd, "store.json"),
            cwd.join("store.json")
        );
        assert_eq!(
            resolve_explicit_cli_path(cwd, "sub/store.json"),
            cwd.join("sub/store.json")
        );
        assert_eq!(
            resolve_explicit_cli_path(cwd, "/abs/store.json"),
            PathBuf::from("/abs/store.json")
        );
    }

    #[test]
    fn resolve_sidecar_in_explicit_relative_override_is_cwd_relative() {
        // R538 — an explicit `--sidecar` relative override resolves against the
        // CWD (the dir the user is standing in), NOT the workspace anchor. This
        // corrects the pre-R538 anchor-join, which silently planted a subdir
        // store at the repo root. The explicit branch short-circuits discovery,
        // so a fake anchor is never consulted.
        let cwd = Path::new("/home/u/run/author");
        let anchor = Path::new("/the/workspace/root");
        let got = resolve_sidecar_in(cwd, anchor, Some("custom/store.json")).unwrap();
        assert_eq!(got, cwd.join("custom/store.json"));
        assert!(
            !got.starts_with(anchor),
            "must not resolve under the anchor"
        );
        // Absolute override passes through unchanged.
        let abs = resolve_sidecar_in(cwd, anchor, Some("/abs/store.json")).unwrap();
        assert_eq!(abs, PathBuf::from("/abs/store.json"));
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
