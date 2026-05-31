//! Cascade / render / sidecar orchestration — the side-effecting bridge
//! between the atomic store and the derived `GENERATED.md` artifact, plus
//! the sidecar/output path-resolution chain. Shared by the CLI bin
//! (`generate-docs` / `verify-generated` / every mutate's auto-regenerate)
//! and the MCP server (in-process mutate + validate).
//!
//! Moved here from `mnemosyne-cli/src/atomic_cli.rs` (R319) so both
//! binaries depend on one orchestration crate rather than mcp linking the
//! CLI binary's library half.

use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use mnemosyne_atomic::AtomicStore;
use mnemosyne_config::discover_config;
use mnemosyne_query::{
    compose_generated_md, render_changelog_entry, render_section, section_heading,
};

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

/// Resolve cascade output path with the Round 279 precedence chain:
/// 1. Explicit `--output` CLI flag wins absolutely.
/// 2. `[atomic] output_path` from `mnemosyne.toml`, if set.
/// 3. Built-in default `<workspace_root>/docs/GENERATED.md`.
///
/// `[workspace] docs[0]` is *not* consulted — docs[0] is the parse target
/// (markdown the validator reads), while this is the cascade write target
/// (atomic store → md). Keeping them independent prevents a first mutate
/// from clobbering hand-authored content in docs[0].
pub fn resolve_output(anchor: &Path, output: Option<&str>) -> Result<PathBuf> {
    // Explicit override short-circuits before discovery (see resolve_sidecar);
    // relative override joins the anchor.
    if let Some(p) = output {
        let pb = PathBuf::from(p);
        return Ok(if pb.is_absolute() {
            pb
        } else {
            anchor.join(pb)
        });
    }
    // No override: fail loud on malformed config; `[atomic]` / default paths
    // join the config-declared root, not `anchor` (see workspace_root_from).
    let loaded = discover_config(anchor)?;
    let root = loaded
        .as_ref()
        .map(|l| l.workspace_root.as_path())
        .unwrap_or(anchor);
    if let Some(cfg_path) = loaded
        .as_ref()
        .and_then(|l| l.config.atomic.as_ref())
        .and_then(|a| a.output_path.as_deref())
    {
        let pb = PathBuf::from(cfg_path);
        return Ok(if pb.is_absolute() { pb } else { root.join(pb) });
    }
    Ok(root.join("docs/GENERATED.md"))
}

/// Render the atomic store at `sidecar_path` to a deterministic markdown
/// string. Side-effect free — the read-only render path shared by
/// `generate-docs` (writes the bytes) and `verify-generated` (compares
/// the bytes). `Source:` line uses a path relative to `workspace_root` so
/// the output is portable across checkouts.
pub fn render_atomic_store_to_md(
    workspace_root: &Path,
    sidecar_path: &Path,
) -> Result<(String, AtomicStore)> {
    let store = AtomicStore::load(sidecar_path).map_err(|e| anyhow!("{}", e))?;

    let workspace_prefix = format!("{}/", workspace_root.display());
    let source_rel = sidecar_path
        .display()
        .to_string()
        .replacen(&workspace_prefix, "", 1);

    // Render each unit, then hand the raw blocks to the single-source document
    // composer (R345 Decision 4 — the warm `RenderDb` Tier-2 composition calls
    // the same builder, so the format cannot drift). Sections: title /
    // decision_status come from the skeleton; pre-backfill sections (empty
    // title) fall back to the section_id so the heading stays parseable.
    let mut section_blocks = Vec::with_capacity(store.sections.len());
    for (section_id, atomic) in &store.sections {
        let (title, status) = section_heading(section_id, atomic);
        section_blocks.push(
            render_section(section_id, title, status, atomic)
                .map_err(|e| anyhow!("render section {}: {}", section_id, e))?,
        );
    }
    let mut entry_blocks = Vec::with_capacity(store.changelog_entries.len());
    for (entry_id, entry) in &store.changelog_entries {
        entry_blocks.push(
            render_changelog_entry(entry_id, entry)
                .map_err(|e| anyhow!("render entry {}: {}", entry_id, e))?,
        );
    }

    let out = compose_generated_md(&source_rel, &section_blocks, &entry_blocks);
    Ok((out, store))
}

/// Atomic-write the rendered content to `output_path` (temp + rename).
pub fn write_generated_md(output_path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = output_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }
    let tmp_path = output_path.with_extension("md.tmp");
    {
        let mut tmp = fs::File::create(&tmp_path)
            .with_context(|| format!("create {}", tmp_path.display()))?;
        tmp.write_all(content.as_bytes())?;
        tmp.sync_all()?;
    }
    fs::rename(&tmp_path, output_path)
        .with_context(|| format!("rename to {}", output_path.display()))?;
    Ok(())
}

/// Auto-regenerate GENERATED.md after a successful atomic mutate. Default
/// behavior of every atomic mutate (overridable). Errors are propagated —
/// a regenerate failure after a successful mutate signals the cascade is in
/// an inconsistent state and needs manual intervention.
pub fn auto_regenerate(anchor: &Path, sidecar: Option<&str>) -> Result<()> {
    let root = workspace_root_from(anchor)?;
    let sidecar_path = resolve_sidecar(anchor, sidecar)?;
    let output_path = resolve_output(anchor, None)?;
    let (content, _) = render_atomic_store_to_md(&root, &sidecar_path)?;
    write_generated_md(&output_path, &content)?;
    Ok(())
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
    /// True iff GENERATED.md byte-equals the freshly rendered output of
    /// the atomic store.
    pub generated_in_sync: bool,
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
    // start; render below uses the config-declared root so the rendered
    // `Source:` line matches the committed GENERATED.md (sync comparison).
    let workspace_root = workspace_root_from(anchor)?;
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

    let output_path = resolve_output(anchor, None)?;
    let generated_in_sync = if output_path.exists() {
        let (expected, _) = render_atomic_store_to_md(&workspace_root, &sidecar_path)?;
        let actual = fs::read_to_string(&output_path)
            .with_context(|| format!("read {}", output_path.display()))?;
        expected == actual
    } else {
        // Empty store + missing GENERATED.md = trivially in sync.
        store.changelog_entries.is_empty() && store.sections.is_empty()
    };

    Ok(AtomicValidationSummary {
        entries: store.changelog_entries.len(),
        sections: store.sections.len(),
        orphan_entry_refs,
        orphan_section_refs,
        generated_in_sync,
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
