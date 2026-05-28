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
use mnemosyne_core::DecisionStatus;
use mnemosyne_query::{render_changelog_entry, render_section};

/// Resolve sidecar path with the Round 279 precedence chain:
/// 1. Explicit `--sidecar` CLI flag wins absolutely.
/// 2. `[atomic] sidecar_path` from `mnemosyne.toml` (workspace-relative
///    or absolute) when discoverable.
/// 3. Default `<workspace_root>/docs/.atomic/workspace.atomic.json`.
pub fn resolve_sidecar(workspace_root: &Path, sidecar: Option<&str>) -> PathBuf {
    if let Some(p) = sidecar {
        let pb = PathBuf::from(p);
        return if pb.is_absolute() {
            pb
        } else {
            workspace_root.join(pb)
        };
    }
    if let Ok(Some(loaded)) = discover_config(workspace_root) {
        if let Some(cfg_path) = loaded
            .config
            .atomic
            .as_ref()
            .and_then(|a| a.sidecar_path.as_deref())
        {
            let pb = PathBuf::from(cfg_path);
            return if pb.is_absolute() {
                pb
            } else {
                workspace_root.join(pb)
            };
        }
    }
    AtomicStore::default_sidecar_path(workspace_root)
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
pub fn resolve_output(workspace_root: &Path, output: Option<&str>) -> PathBuf {
    if let Some(p) = output {
        let pb = PathBuf::from(p);
        return if pb.is_absolute() {
            pb
        } else {
            workspace_root.join(pb)
        };
    }
    if let Ok(Some(loaded)) = discover_config(workspace_root) {
        if let Some(cfg_path) = loaded
            .config
            .atomic
            .as_ref()
            .and_then(|a| a.output_path.as_deref())
        {
            let pb = PathBuf::from(cfg_path);
            return if pb.is_absolute() {
                pb
            } else {
                workspace_root.join(pb)
            };
        }
    }
    workspace_root.join("docs/GENERATED.md")
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

    let mut out = String::new();
    out.push_str("# GENERATED.md — atomic store derived view\n\n");
    out.push_str(
        "this file `mnemosyne-cli generate-docs` output — direct no edit. \
  atomic store (`docs/.atomic/workspace.atomic.json`) in mutate \
  primitive (`set-section-*` / `append-changelog-entry`) pass and then \
  re-generate.\n\n",
    );
    let workspace_prefix = format!("{}/", workspace_root.display());
    let source_rel = sidecar_path
        .display()
        .to_string()
        .replacen(&workspace_prefix, "", 1);
    out.push_str(&format!("Source: `{}`\n\n", source_rel));
    out.push_str("---\n\n");

    // Sections — Round 287 outline lift retires the placeholder header.
    // atomic.title / decision_status come from the atomic store directly;
    // full body is synthesized via render_section (intent, rationale, etc.).
    // Pre-backfill sections (empty title) fall back to the section_id as
    // heading text so the surface stays human-parseable.
    if !store.sections.is_empty() {
        out.push_str("## Sections\n\n");
        for (section_id, atomic) in &store.sections {
            let title = if atomic.skeleton.title.is_empty() {
                section_id.as_str()
            } else {
                atomic.skeleton.title.as_str()
            };
            let status = match atomic
                .skeleton
                .decision_status
                .unwrap_or(DecisionStatus::Active)
            {
                DecisionStatus::Active => "active",
                DecisionStatus::Superseded => "superseded",
                DecisionStatus::Removed => "removed",
            };
            let rendered = render_section(section_id, title, status, atomic)
                .map_err(|e| anyhow!("render section {}: {}", section_id, e))?;
            // render_section emits `## §N. title` for top-level depth. The
            // atomic-only sections live under the doc's `## Sections` heading,
            // so demote one level (`##` → `###`) to keep the outline coherent.
            let demoted = rendered.replacen("## §", "### §", 1);
            out.push_str(&demoted);
            out.push('\n');
        }
    }

    // Changelog entries — atomic first carry scope.
    if !store.changelog_entries.is_empty() {
        out.push_str("## Changelog (atomic ledger)\n\n");
        for (entry_id, entry) in &store.changelog_entries {
            let rendered = render_changelog_entry(entry_id, entry)
                .map_err(|e| anyhow!("render entry {}: {}", entry_id, e))?;
            out.push_str(&rendered);
            out.push('\n');
        }
    } else {
        out.push_str("## Changelog (atomic ledger)\n\n");
        out.push_str("(empty — first atomic entry will populate this section.)\n\n");
    }

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
pub fn auto_regenerate(workspace_root: &Path, sidecar: Option<&str>) -> Result<()> {
    let sidecar_path = resolve_sidecar(workspace_root, sidecar);
    let output_path = resolve_output(workspace_root, None);
    let (content, _) = render_atomic_store_to_md(workspace_root, &sidecar_path)?;
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
    workspace_root: &Path,
    section_id_set: &BTreeSet<String>,
) -> Result<AtomicValidationSummary> {
    // Honor `[atomic].sidecar_path` config so the read / validation path
    // sees the same store the mutate path wrote to.
    let sidecar_path = resolve_sidecar(workspace_root, None);
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
    }

    let output_path = resolve_output(workspace_root, None);
    let generated_in_sync = if output_path.exists() {
        let (expected, _) = render_atomic_store_to_md(workspace_root, &sidecar_path)?;
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
