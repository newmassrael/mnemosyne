//! Atomic mutate CLI subcommands — spec mutate API atomic scope
//!.
//!
//! Spec binding: §atomic-store-mutate-api.
//!
//! 10 subcommands cover the 9 atomic Section primitives + 1 atomic ChangelogEntry primitive:
//! - `set-section-intent` — set Section.intent (1-3 sentence summary)
//! - `set-section-rationale` — set Section.rationale_bullets (list)
//! - `set-section-inputs` — set Section.inputs_bullets
//! - `set-section-outputs` — set Section.outputs_bullets
//! - `add-section-caveat` — append to Section.caveats_bullets
//! - `set-section-alternatives` — set Section.alternatives_rejected
//! - `set-section-impact-scope` — set Section.impact_scope (cross-ref list)
//! - `add-section-example` — append to Section.examples (code block)
//! - `add-section-binding` — append to Section.bindings
//!
//! - `append-changelog-entry` — atomic-aware changelog append
//! (decision_summary + changes + verification + impact + carry_forward)
//!
//! Each subcommand:
//! 1. Loads `AtomicStore` from sidecar JSON (default `docs/.atomic/
//! workspace.atomic.json`, configurable via `--sidecar <path>`).
//! 2. Invokes the relevant mutate primitive (T3 threshold validation).
//! 3. Persists the store atomically (temp + rename, pattern).
//! 4. Prints `AtomicMutateReceipt` (text or `--json`).
//!
//! permission boundary: production crate atomic scope only — DESIGN.md / ROADMAP.md
//! / 6-doc scope — 0 mutations. frozen ledger consistency (legacy body /
//! sub_bullets field preserved).

use std::fs;
use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};

use crate::CliError;
use mnemosyne_atomic::{
    add_inventory_entry, add_section_binding, add_section_caveat, add_section_example,
    append_changelog_entry, append_confirmation_event, remove_inventory_entry, remove_section,
    remove_section_binding, set_inventory_section_ref, set_inventory_status,
    set_section_alternatives, set_section_binding_kind, set_section_coverage_expectation,
    set_section_decision_status, set_section_impact_scope, set_section_inputs, set_section_intent,
    set_section_outputs, set_section_parent_doc, set_section_parent_section, set_section_rationale,
    set_section_title, set_section_verification_expectation, ArtifactHashes, AtomicMutateError,
    AtomicMutateReceipt, AtomicStore, BindingKind, ChangelogEntryDraft, ConfirmMethod,
    ConfirmationClaim, ConfirmationEvent, Confirmer, ConfirmerKind, ExampleBlock,
    RejectedAlternative, Verdict,
};
use mnemosyne_config::discover_config;
use mnemosyne_core::{
    strip_section_marker, CoverageExpectation, DecisionStatus, InventoryStatus,
    VerificationExpectation,
};
use mnemosyne_ops::cascade::resolve_sidecar;
use mnemosyne_validate::code_refs::{scan_inventory_decay, scan_section_decay};

fn print_receipt(r: &AtomicMutateReceipt, json: bool) {
    if json {
        if let Ok(s) = serde_json::to_string_pretty(r) {
            println!("{}", s);
        }
    } else {
        println!("=== mnemosyne-cli {} ===", r.primitive);
        println!("primitive: {}", r.primitive);
        println!("target_kind: {}", r.target_kind);
        println!("target_id: {}", r.target_id);
        println!("sidecar_path: {}", r.sidecar_path);
        println!("written_bytes: {}", r.written_bytes);
    }
}

fn print_error(e: &AtomicMutateError, json: bool) {
    if json {
        let v = serde_json::json!({
        "kind": match e {
         AtomicMutateError::Validation(_) => "validation",
         AtomicMutateError::NotFound(_) => "not_found",
         AtomicMutateError::FrozenLedger(_) => "frozen_ledger",
         AtomicMutateError::Store(_) => "store",
        },
        "detail": format!("{}", e),
        });
        eprintln!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
    } else {
        eprintln!("=== mnemosyne-cli atomic mutate FAILED ===");
        eprintln!("error: {}", e);
    }
}

/// Read a "bullets" file: one bullet per non-empty line, stripping leading
/// `- ` if present. Empty lines and trailing whitespace are ignored.
fn parse_bullets_file(path: &str) -> Result<Vec<String>> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("bullets-file recovery failed: {}", path))?;
    let bullets: Vec<String> = content
        .lines()
        .map(|l| l.trim_end())
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let s = l.trim_start();
            s.strip_prefix("- ").unwrap_or(s).to_string()
        })
        .collect();
    Ok(bullets)
}

fn parse_alternatives_file(path: &str) -> Result<Vec<RejectedAlternative>> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("alternatives-file recovery failed: {}", path))?;
    let mut out = Vec::new();
    for (lineno, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Format: `<alternative> -- <reason>` or `<alternative> — <reason>`.
        let parsed = RejectedAlternative::parse_line(trimmed).ok_or_else(|| {
            anyhow!(
                "alternatives-file:{}: line format violation — `<alternative> -- <reason>` or ` — ` separator required",
                lineno + 1
            )
        })?;
        out.push(parsed);
    }
    Ok(out)
}

/// Parse `--section` or `--section 43` → "43". Owned-`String` adapter over
/// the canonical [`strip_section_marker`] for CLI arg-parse ergonomics
/// (stored in arg structs, used as a `.map(..)` fn pointer).
fn strip_section_prefix(s: &str) -> String {
    strip_section_marker(s).to_string()
}

// ============================================================================
// CLI subcommand entry points (each takes args slice = post-subcommand args)
// ============================================================================

pub fn cmd_set_section_intent(workspace_root: &Path, args: &[String]) -> Result<(), CliError> {
    let mut section: Option<String> = None;
    let mut intent: Option<String> = None;
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--section" => {
                section = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--section missing"))?
                        .clone(),
                )
            }
            "--intent" => {
                intent = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--intent missing"))?
                        .clone(),
                )
            }
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let section = strip_section_prefix(&section.ok_or_else(|| anyhow!("--section arg required"))?);
    let intent = intent.ok_or_else(|| anyhow!("--intent arg required"))?;
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    finalize_mutate(
        set_section_intent(&mut store, &sidecar_path, &section, &intent),
        json,
    )
}

/// Round 287 / 289 — atomic `add-section` CLI surface (Phase F).
///
/// Replaces the legacy markdown-surgical `add-section` (mutate.rs) with the
/// atomic primitive. Closed-form Section creation: only outline fields
/// (`section_id`, `parent_doc`, `title`, optional `parent_section`); content
/// fields (intent / rationale / etc.) populate via subsequent `set-section-*`
/// calls. The legacy `--body-file` and `--numbered-id` flags are retired —
/// atomic mode has no monolithic body, and section_id is explicit.
pub fn cmd_add_section(workspace_root: &Path, args: &[String]) -> Result<(), CliError> {
    let mut section: Option<String> = None;
    let mut parent_doc: Option<String> = None;
    let mut title: Option<String> = None;
    let mut parent: Option<String> = None;
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--section" => {
                section = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--section missing"))?
                        .clone(),
                )
            }
            "--parent-doc" => {
                parent_doc = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--parent-doc missing"))?
                        .clone(),
                )
            }
            "--title" => {
                title = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--title missing"))?
                        .clone(),
                )
            }
            "--parent" => {
                parent = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--parent missing"))?
                        .clone(),
                )
            }
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let section = strip_section_prefix(&section.ok_or_else(|| anyhow!("--section arg required"))?);
    let parent_doc = parent_doc.ok_or_else(|| anyhow!("--parent-doc arg required"))?;
    let title = title.ok_or_else(|| anyhow!("--title arg required"))?;
    let parent_stripped = parent.as_deref().map(strip_section_prefix);
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    finalize_mutate(
        mnemosyne_atomic::add_section(
            &mut store,
            &sidecar_path,
            &section,
            &parent_doc,
            &title,
            parent_stripped.as_deref(),
        ),
        json,
    )
}

/// RFC-001 UC-1 "A2" — bulk section-create from a JSON manifest, as one
/// atomic transaction. Manifest = a JSON array of
/// `{section_id, parent_doc, title, parent_section?, normative_excerpt?}`.
/// Per-entry 3-way: absent → create / byte-identical → no-op / divergent →
/// reject the WHOLE manifest. One save for the batch (no-op
/// entries don't count; an all-no-op manifest writes nothing). Reuses
/// `add_section`'s in-memory core — the same single section-create write-path.
pub fn cmd_import_sections(workspace_root: &Path, args: &[String]) -> Result<(), CliError> {
    let mut manifest: Option<String> = None;
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--manifest" => {
                manifest = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--manifest missing"))?
                        .clone(),
                )
            }
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let manifest_path = manifest.ok_or_else(|| anyhow!("--manifest <path> arg required"))?;
    let raw = fs::read_to_string(&manifest_path)
        .with_context(|| format!("read manifest {}", manifest_path))?;
    let entries: Vec<mnemosyne_atomic::SectionImport> =
        serde_json::from_str(&raw).with_context(|| {
            format!(
                "parse manifest {} (JSON array of section imports)",
                manifest_path
            )
        })?;
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    finalize_mutate(
        mnemosyne_atomic::import_sections(&mut store, &sidecar_path, &entries),
        json,
    )
}

/// Round 430 — bulk frames + narrative facts from a manifest (one atomic
/// transaction; forward succession refs within the manifest are legal).
pub fn cmd_import_facts(workspace_root: &Path, args: &[String]) -> Result<(), CliError> {
    let mut manifest: Option<String> = None;
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--manifest" => {
                manifest = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--manifest missing"))?
                        .clone(),
                )
            }
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let manifest_path = manifest.ok_or_else(|| anyhow!("--manifest <path> arg required"))?;
    let raw = fs::read_to_string(&manifest_path)
        .with_context(|| format!("read manifest {}", manifest_path))?;
    let parsed: mnemosyne_atomic::FactsManifest =
        serde_json::from_str(&raw).with_context(|| {
            format!(
                "parse manifest {} ({})",
                manifest_path,
                mnemosyne_atomic::FACTS_MANIFEST_SHAPE
            )
        })?;
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    finalize_mutate(
        mnemosyne_atomic::import_facts(&mut store, &sidecar_path, &parsed),
        json,
    )
}

/// Round 430 — register one epistemic frame.
pub fn cmd_add_frame(workspace_root: &Path, args: &[String]) -> Result<(), CliError> {
    let mut frame_id: Option<String> = None;
    let mut description = String::new();
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--frame" => {
                frame_id = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--frame missing"))?
                        .clone(),
                )
            }
            "--description" => {
                description = iter
                    .next()
                    .ok_or_else(|| anyhow!("--description missing"))?
                    .clone()
            }
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let frame_id = frame_id.ok_or_else(|| anyhow!("--frame arg required"))?;
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    finalize_mutate(
        mnemosyne_atomic::add_frame(&mut store, &sidecar_path, &frame_id, &description),
        json,
    )
}

/// Round 436 — register one world-line branch (the frames-registry
/// symmetry; `main` is known by construction and never registered).
pub fn cmd_add_branch(workspace_root: &Path, args: &[String]) -> Result<(), CliError> {
    let mut branch_id: Option<String> = None;
    let mut description = String::new();
    let mut forks_from: Option<String> = None;
    let mut forks_at: Option<String> = None;
    let mut converges: Vec<(String, String)> = Vec::new();
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--branch" => {
                branch_id = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--branch missing"))?
                        .clone(),
                )
            }
            "--description" => {
                description = iter
                    .next()
                    .ok_or_else(|| anyhow!("--description missing"))?
                    .clone()
            }
            "--forks-from" => {
                forks_from = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--forks-from missing"))?
                        .clone(),
                )
            }
            "--forks-at" => {
                forks_at = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--forks-at missing"))?
                        .clone(),
                )
            }
            // Round 532 — repeatable `<parent>=<merge-section>` incoming-merge
            // edge of a confluence world-line.
            "--converges" => {
                let pair = iter.next().ok_or_else(|| anyhow!("--converges missing"))?;
                let (parent, at) = pair.split_once('=').ok_or_else(|| {
                    anyhow!("--converges expects `<parent-branch>=<merge-section>`")
                })?;
                converges.push((parent.to_string(), at.to_string()));
            }
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let branch_id = branch_id.ok_or_else(|| anyhow!("--branch arg required"))?;
    let fork = match (&forks_from, &forks_at) {
        (None, None) => None,
        (Some(p), Some(a)) => Some((p.as_str(), a.as_str())),
        _ => return Err(anyhow!("--forks-from and --forks-at must be given together").into()),
    };
    let converges_from: Vec<(&str, &str)> = converges
        .iter()
        .map(|(p, a)| (p.as_str(), a.as_str()))
        .collect();
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    finalize_mutate(
        mnemosyne_atomic::add_branch(
            &mut store,
            &sidecar_path,
            &branch_id,
            &description,
            fork,
            &converges_from,
        ),
        json,
    )
}

/// Round 437 — register one narrative entity (third registry; the
/// retrieval key for "all facts about X").
pub fn cmd_add_entity(workspace_root: &Path, args: &[String]) -> Result<(), CliError> {
    let mut entity_id: Option<String> = None;
    let mut kind = String::new();
    let mut description = String::new();
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--entity" => {
                entity_id = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--entity missing"))?
                        .clone(),
                )
            }
            "--kind" => {
                kind = iter
                    .next()
                    .ok_or_else(|| anyhow!("--kind missing"))?
                    .clone()
            }
            "--description" => {
                description = iter
                    .next()
                    .ok_or_else(|| anyhow!("--description missing"))?
                    .clone()
            }
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let entity_id = entity_id.ok_or_else(|| anyhow!("--entity arg required"))?;
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    finalize_mutate(
        mnemosyne_atomic::add_entity(&mut store, &sidecar_path, &entity_id, &kind, &description),
        json,
    )
}

/// Register one entity kind — the vocabulary `--kind` refs on `add-entity`.
/// The members are the consumer's; the substrate only enforces that a kind in
/// use was declared (Round 661's machine-slot rule, invariant 4 routing).
pub fn cmd_add_entity_kind(workspace_root: &Path, args: &[String]) -> Result<(), CliError> {
    let mut kind_id: Option<String> = None;
    let mut description = String::new();
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--kind" => {
                kind_id = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--kind missing"))?
                        .clone(),
                )
            }
            "--description" => {
                description = iter
                    .next()
                    .ok_or_else(|| anyhow!("--description missing"))?
                    .clone()
            }
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let kind_id = kind_id.ok_or_else(|| anyhow!("--kind arg required"))?;
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    finalize_mutate(
        mnemosyne_atomic::add_entity_kind(&mut store, &sidecar_path, &kind_id, &description),
        json,
    )
}

/// Round 706 — register one unit of measure (the `quantity` object shape's
/// unit vocabulary). `--unit` mandatory; the members are the consumer's
/// (`day`, `minute`), core never enumerates them (invariant 4).
pub fn cmd_add_unit(workspace_root: &Path, args: &[String]) -> Result<(), CliError> {
    let mut unit_id: Option<String> = None;
    let mut description = String::new();
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--unit" => {
                unit_id = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--unit missing"))?
                        .clone(),
                )
            }
            "--description" => {
                description = iter
                    .next()
                    .ok_or_else(|| anyhow!("--description missing"))?
                    .clone()
            }
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let unit_id = unit_id.ok_or_else(|| anyhow!("--unit arg required"))?;
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    finalize_mutate(
        mnemosyne_atomic::add_unit(&mut store, &sidecar_path, &unit_id, &description),
        json,
    )
}

/// Round 709 → DEBT-J — attach a cost to one map edge (the adjacent fact).
/// `--fact` (the adjacent fact id) + `--n` (positive integer) + `--unit`
/// (registered) mandatory. The cost is edge metadata (a side-table entry), not
/// a fact.
pub fn cmd_add_edge_cost(workspace_root: &Path, args: &[String]) -> Result<(), CliError> {
    let mut fact_id: Option<String> = None;
    let mut n: Option<String> = None;
    let mut unit: Option<String> = None;
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--fact" => {
                fact_id = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--fact missing"))?
                        .clone(),
                )
            }
            "--n" => n = Some(iter.next().ok_or_else(|| anyhow!("--n missing"))?.clone()),
            "--unit" => {
                unit = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--unit missing"))?
                        .clone(),
                )
            }
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let fact_id = fact_id.ok_or_else(|| anyhow!("--fact arg required"))?;
    let n = n
        .ok_or_else(|| anyhow!("--n arg required"))?
        .trim()
        .parse::<i64>()
        .map_err(|_| anyhow!("--n must be an integer"))?;
    let unit = unit.ok_or_else(|| anyhow!("--unit arg required"))?;
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    finalize_mutate(
        mnemosyne_atomic::add_edge_cost(&mut store, &sidecar_path, &fact_id, n, &unit),
        json,
    )
}

/// Round 711 — remove a map edge's cost (the peer of `add-edge-cost`). Drops a
/// stray cost off a NON-edge fact (which `validate-continuity` flags) without
/// retracting the fact, and cleans an out-of-band orphan cost.
pub fn cmd_remove_edge_cost(workspace_root: &Path, args: &[String]) -> Result<(), CliError> {
    let mut fact_id: Option<String> = None;
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--fact" => {
                fact_id = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--fact missing"))?
                        .clone(),
                )
            }
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let fact_id = fact_id.ok_or_else(|| anyhow!("--fact arg required"))?;
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    finalize_mutate(
        mnemosyne_atomic::remove_edge_cost(&mut store, &sidecar_path, &fact_id),
        json,
    )
}

/// Round 717 design → Round 720 — attach a place-access GUARD to one map edge:
/// `--fact` (the adjacent edge fact) REQUIRES `--condition` (the condition fact).
/// Both must exist (a dangling-ref check); the guard is edge metadata (a
/// side-table entry), NEVER evaluated by Mnemosyne (the consumer's job).
pub fn cmd_add_edge_guard(workspace_root: &Path, args: &[String]) -> Result<(), CliError> {
    let mut fact_id: Option<String> = None;
    let mut condition: Option<String> = None;
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--fact" => {
                fact_id = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--fact missing"))?
                        .clone(),
                )
            }
            "--condition" => {
                condition = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--condition missing"))?
                        .clone(),
                )
            }
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let fact_id = fact_id.ok_or_else(|| anyhow!("--fact arg required"))?;
    let condition = condition.ok_or_else(|| anyhow!("--condition arg required"))?;
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    finalize_mutate(
        mnemosyne_atomic::add_edge_guard(&mut store, &sidecar_path, &fact_id, &condition),
        json,
    )
}

/// Round 720 — remove a map edge's guard (the peer of `add-edge-guard`). Drops a
/// stray guard off a NON-edge fact (which `validate-continuity` flags) without
/// retracting the fact, and cleans an out-of-band orphan guard.
pub fn cmd_remove_edge_guard(workspace_root: &Path, args: &[String]) -> Result<(), CliError> {
    let mut fact_id: Option<String> = None;
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--fact" => {
                fact_id = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--fact missing"))?
                        .clone(),
                )
            }
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let fact_id = fact_id.ok_or_else(|| anyhow!("--fact arg required"))?;
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    finalize_mutate(
        mnemosyne_atomic::remove_edge_guard(&mut store, &sidecar_path, &fact_id),
        json,
    )
}

/// Round 446 — register one predicate (fourth registry; load-bearing refs
/// the narrative rules key off). `--object-kind entity|token|quantity|fact`
/// mandatory (Round 708 removed free-text `scalar`).
/// Round 701 — optional `--subject-kind` / `--object-entity-kind` declare the
/// required endpoint entity-kind (registered `entity_kinds`); the write path
/// then rejects a fact whose endpoint is not that kind (the spatial-map gate).
pub fn cmd_add_predicate(workspace_root: &Path, args: &[String]) -> Result<(), CliError> {
    let mut predicate_id: Option<String> = None;
    let mut object_kind: Option<String> = None;
    let mut subject_kind: Option<String> = None;
    let mut object_entity_kind: Option<String> = None;
    let mut object_tokens: Vec<String> = Vec::new();
    let mut description = String::new();
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--predicate" => {
                predicate_id = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--predicate missing"))?
                        .clone(),
                )
            }
            "--object-kind" => {
                object_kind = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--object-kind missing"))?
                        .clone(),
                )
            }
            "--subject-kind" => {
                subject_kind = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--subject-kind missing"))?
                        .clone(),
                )
            }
            "--object-entity-kind" => {
                object_entity_kind = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--object-entity-kind missing"))?
                        .clone(),
                )
            }
            "--object-tokens" => {
                // Comma-separated closed vocabulary (Round 705); required under
                // `--object-kind token`, rejected otherwise (build_predicate).
                object_tokens = iter
                    .next()
                    .ok_or_else(|| anyhow!("--object-tokens missing"))?
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
            "--description" => {
                description = iter
                    .next()
                    .ok_or_else(|| anyhow!("--description missing"))?
                    .clone()
            }
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let predicate_id = predicate_id.ok_or_else(|| anyhow!("--predicate arg required"))?;
    let object_kind = object_kind.ok_or_else(|| anyhow!("--object-kind arg required"))?;
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    finalize_mutate(
        mnemosyne_atomic::add_predicate(
            &mut store,
            &sidecar_path,
            &predicate_id,
            &object_kind,
            subject_kind.as_deref(),
            object_entity_kind.as_deref(),
            &object_tokens,
            &description,
        ),
        json,
    )
}

/// Round 658 — re-type or re-describe an EXISTING predicate. Full replace
/// (PUT), so BOTH `--object-kind` and `--description` are mandatory here even
/// though `add-predicate` defaults the description: omitting one on an update
/// path would wipe it silently, and this primitive exists precisely because
/// silent registry damage had no repair.
pub fn cmd_set_predicate(workspace_root: &Path, args: &[String]) -> Result<(), CliError> {
    let mut predicate_id: Option<String> = None;
    let mut object_kind: Option<String> = None;
    let mut subject_kind: Option<String> = None;
    let mut object_entity_kind: Option<String> = None;
    let mut object_tokens: Vec<String> = Vec::new();
    let mut description: Option<String> = None;
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--predicate" => {
                predicate_id = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--predicate missing"))?
                        .clone(),
                )
            }
            "--object-kind" => {
                object_kind = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--object-kind missing"))?
                        .clone(),
                )
            }
            "--subject-kind" => {
                subject_kind = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--subject-kind missing"))?
                        .clone(),
                )
            }
            "--object-entity-kind" => {
                object_entity_kind = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--object-entity-kind missing"))?
                        .clone(),
                )
            }
            "--object-tokens" => {
                object_tokens = iter
                    .next()
                    .ok_or_else(|| anyhow!("--object-tokens missing"))?
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
            "--description" => {
                description = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--description missing"))?
                        .clone(),
                )
            }
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let predicate_id = predicate_id.ok_or_else(|| anyhow!("--predicate arg required"))?;
    let object_kind = object_kind.ok_or_else(|| anyhow!("--object-kind arg required"))?;
    let description = description.ok_or_else(|| {
        anyhow!("--description arg required (set-predicate is a full replace; state it explicitly)")
    })?;
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    finalize_mutate(
        mnemosyne_atomic::set_predicate(
            &mut store,
            &sidecar_path,
            &predicate_id,
            &object_kind,
            subject_kind.as_deref(),
            object_entity_kind.as_deref(),
            &object_tokens,
            &description,
        ),
        json,
    )
}

/// Round 658 — remove a predicate from the registry. Rejects while any typed
/// leg still names it.
pub fn cmd_remove_predicate(workspace_root: &Path, args: &[String]) -> Result<(), CliError> {
    let mut predicate_id: Option<String> = None;
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--predicate" => {
                predicate_id = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--predicate missing"))?
                        .clone(),
                )
            }
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let predicate_id = predicate_id.ok_or_else(|| anyhow!("--predicate arg required"))?;
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    finalize_mutate(
        mnemosyne_atomic::remove_predicate(&mut store, &sidecar_path, &predicate_id),
        json,
    )
}

/// Round 506 — register one disclosure (discourse) plan: a named telling over
/// the fact base. `--default-mode withhold|state|hint|imply` mandatory (no
/// silent default for a load-bearing policy).
pub fn cmd_add_disclosure_plan(workspace_root: &Path, args: &[String]) -> Result<(), CliError> {
    let mut telling_id: Option<String> = None;
    let mut default_mode: Option<String> = None;
    let mut description = String::new();
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--telling" => {
                telling_id = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--telling missing"))?
                        .clone(),
                )
            }
            "--default-mode" => {
                default_mode = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--default-mode missing"))?
                        .clone(),
                )
            }
            "--description" => {
                description = iter
                    .next()
                    .ok_or_else(|| anyhow!("--description missing"))?
                    .clone()
            }
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let telling_id = telling_id.ok_or_else(|| anyhow!("--telling arg required"))?;
    let default_mode = default_mode.ok_or_else(|| anyhow!("--default-mode arg required"))?;
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    finalize_mutate(
        mnemosyne_atomic::add_disclosure_plan(
            &mut store,
            &sidecar_path,
            &telling_id,
            &default_mode,
            &description,
        ),
        json,
    )
}

/// Round 506 — set one per-fact disclosure override within a telling.
/// `--first-at <branch>=<coord>` is repeatable (per world-line timing);
/// `--surface <scene>[,<object>]` is optional (the diegetic carrier).
pub fn cmd_set_disclosure(workspace_root: &Path, args: &[String]) -> Result<(), CliError> {
    let mut telling_id: Option<String> = None;
    let mut fact_id: Option<String> = None;
    let mut mode: Option<String> = None;
    let mut first_at: Vec<(String, String)> = Vec::new();
    let mut surface_scene: Option<String> = None;
    let mut surface_object: Option<String> = None;
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--telling" => {
                telling_id = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--telling missing"))?
                        .clone(),
                )
            }
            "--fact" => {
                fact_id = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--fact missing"))?
                        .clone(),
                )
            }
            "--mode" => {
                mode = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--mode missing"))?
                        .clone(),
                )
            }
            "--first-at" => {
                let raw = iter.next().ok_or_else(|| anyhow!("--first-at missing"))?;
                let (branch, coord) = raw
                    .split_once('=')
                    .ok_or_else(|| anyhow!("--first-at format: <branch>=<coord>"))?;
                first_at.push((branch.to_string(), coord.to_string()));
            }
            "--surface" => {
                let raw = iter.next().ok_or_else(|| anyhow!("--surface missing"))?;
                match raw.split_once(',') {
                    Some((scene, object)) => {
                        surface_scene = Some(scene.to_string());
                        surface_object = Some(object.to_string());
                    }
                    None => surface_scene = Some(raw.clone()),
                }
            }
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let telling_id = telling_id.ok_or_else(|| anyhow!("--telling arg required"))?;
    let fact_id = fact_id.ok_or_else(|| anyhow!("--fact arg required"))?;
    let mode = mode.ok_or_else(|| anyhow!("--mode arg required"))?;
    let surface = surface_scene
        .as_deref()
        .map(|scene| (scene, surface_object.as_deref()));
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    finalize_mutate(
        mnemosyne_atomic::set_disclosure(
            &mut store,
            &sidecar_path,
            mnemosyne_atomic::DisclosureDecision {
                telling_id: &telling_id,
                fact_id: &fact_id,
                mode: &mode,
                first_at: &first_at,
                surface,
            },
        ),
        json,
    )
}

/// Round 626 — clear ONE telling's disclosure decision for one fact: the
/// escape hatch the R626 retract/amend guards require (a guard that says
/// "clear the decision first" with no way to clear it is a trap, not an
/// invariant).
pub fn cmd_remove_disclosure(workspace_root: &Path, args: &[String]) -> Result<(), CliError> {
    let mut telling_id: Option<String> = None;
    let mut fact_id: Option<String> = None;
    let mut reason: Option<String> = None;
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--telling" => {
                telling_id = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--telling missing"))?
                        .clone(),
                )
            }
            "--fact" => {
                fact_id = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--fact missing"))?
                        .clone(),
                )
            }
            "--reason" => {
                reason = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--reason missing"))?
                        .clone(),
                )
            }
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let telling_id = telling_id.ok_or_else(|| anyhow!("--telling arg required"))?;
    let fact_id = fact_id.ok_or_else(|| anyhow!("--fact arg required"))?;
    let reason = reason.ok_or_else(|| anyhow!("--reason arg required"))?;
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    finalize_mutate(
        mnemosyne_atomic::remove_disclosure(
            &mut store,
            &sidecar_path,
            &telling_id,
            &fact_id,
            &reason,
        ),
        json,
    )
}

/// Round 430 — create one narrative fact (same shared build path as
/// `import-facts`; cross-fact refs must already exist in the store).
/// Parsed flag set shared by `add-fact` and `amend-fact` (Round 434: the two
/// verbs describe the same fact shape; only the primitive differs).
struct FactVerbArgs {
    entry: mnemosyne_atomic::FactImport,
    sidecar: Option<String>,
    json: bool,
    reason: Option<String>,
}

fn parse_fact_verb_args(args: &[String], accept_reason: bool) -> Result<FactVerbArgs> {
    let mut out = FactVerbArgs {
        entry: mnemosyne_atomic::FactImport {
            entities: vec![],
            fact_id: String::new(),
            frame: String::new(),
            branch: None,
            claim: String::new(),
            canon_from: String::new(),
            canon_to: None,
            evidence: vec![],
            conflicts_with: vec![],
            supersedes_in_frame: None,
            payoff_expectation: None,
            pays_off: vec![],
            typed: None,
            quote: None,
        },
        sidecar: None,
        json: false,
        reason: None,
    };
    let csv = |raw: &str| -> Vec<String> {
        raw.split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect()
    };
    let mut typed_subject: Option<String> = None;
    let mut typed_predicate: Option<String> = None;
    let mut typed_object_entity: Option<String> = None;
    let mut typed_object_token: Option<String> = None;
    let mut typed_object_quantity_n: Option<String> = None;
    let mut typed_object_quantity_unit: Option<String> = None;
    let mut typed_object_fact: Option<String> = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--fact" => {
                out.entry.fact_id = iter
                    .next()
                    .ok_or_else(|| anyhow!("--fact missing"))?
                    .clone()
            }
            "--frame" => {
                out.entry.frame = iter
                    .next()
                    .ok_or_else(|| anyhow!("--frame missing"))?
                    .clone()
            }
            "--branch" => {
                out.entry.branch = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--branch missing"))?
                        .clone(),
                )
            }
            "--claim" => {
                out.entry.claim = iter
                    .next()
                    .ok_or_else(|| anyhow!("--claim missing"))?
                    .clone()
            }
            "--canon-from" => {
                out.entry.canon_from = iter
                    .next()
                    .ok_or_else(|| anyhow!("--canon-from missing"))?
                    .clone()
            }
            "--canon-to" => {
                out.entry.canon_to = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--canon-to missing"))?
                        .clone(),
                )
            }
            "--evidence" => {
                out.entry.evidence =
                    csv(iter.next().ok_or_else(|| anyhow!("--evidence missing"))?)
            }
            "--conflicts" => {
                out.entry.conflicts_with =
                    csv(iter.next().ok_or_else(|| anyhow!("--conflicts missing"))?)
            }
            "--supersedes" => {
                out.entry.supersedes_in_frame = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--supersedes missing"))?
                        .clone(),
                )
            }
            "--quote" => {
                out.entry.quote = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--quote missing"))?
                        .clone(),
                )
            }
            "--payoff-expectation" => {
                out.entry.payoff_expectation = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--payoff-expectation missing"))?
                        .clone(),
                )
            }
            "--pays-off" => {
                out.entry.pays_off =
                    csv(iter.next().ok_or_else(|| anyhow!("--pays-off missing"))?)
            }
            "--entities" => {
                out.entry.entities =
                    csv(iter.next().ok_or_else(|| anyhow!("--entities missing"))?)
            }
            "--typed-subject" => {
                typed_subject = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--typed-subject missing"))?
                        .clone(),
                )
            }
            "--typed-predicate" => {
                typed_predicate = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--typed-predicate missing"))?
                        .clone(),
                )
            }
            "--typed-object-entity" => {
                typed_object_entity = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--typed-object-entity missing"))?
                        .clone(),
                )
            }
            "--typed-object-token" => {
                typed_object_token = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--typed-object-token missing"))?
                        .clone(),
                )
            }
            "--typed-object-quantity-n" => {
                typed_object_quantity_n = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--typed-object-quantity-n missing"))?
                        .clone(),
                )
            }
            "--typed-object-quantity-unit" => {
                typed_object_quantity_unit = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--typed-object-quantity-unit missing"))?
                        .clone(),
                )
            }
            "--typed-object-fact" => {
                typed_object_fact = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--typed-object-fact missing"))?
                        .clone(),
                )
            }
            "--reason" if accept_reason => {
                out.reason = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--reason missing"))?
                        .clone(),
                )
            }
            "--sidecar" => {
                out.sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => out.json = true,
            other => bail!("unknown flag `{}`", other),
        }
    }
    // Pair the two quantity flags into one candidate (Round 706) — both or
    // neither, since a Quantity object carries n AND unit; the number is parsed
    // fail-loud (an exact integer, never f64).
    let typed_object_quantity: Option<(i64, String)> =
        match (typed_object_quantity_n, typed_object_quantity_unit) {
            (Some(n), Some(unit)) => {
                let n = n.trim().parse::<i64>().map_err(|_| {
                    anyhow!("--typed-object-quantity-n must be an integer (got `{n}`)")
                })?;
                Some((n, unit))
            }
            (None, None) => None,
            _ => bail!(
                "--typed-object-quantity-n and --typed-object-quantity-unit must be given \
                 together (a quantity object is a number + a unit)"
            ),
        };
    // Assemble the optional typed leg (Round 446): all-or-nothing —
    // subject + predicate + exactly ONE object shape. Shape/registry
    // validation lives in the shared builder, not here.
    out.entry.typed = match (
        typed_subject,
        typed_predicate,
        typed_object_entity,
        typed_object_token,
        typed_object_quantity,
        typed_object_fact,
    ) {
        (None, None, None, None, None, None) => None,
        (
            Some(subject),
            Some(predicate),
            object_entity,
            object_token,
            object_quantity,
            object_fact,
        ) => {
            let object = mnemosyne_core::TypedObject::from_exclusive_args(
                object_entity,
                object_token,
                object_quantity,
                object_fact,
            )
            .map_err(|e| anyhow!("{e}"))?;
            Some(mnemosyne_core::TypedClaim {
                subject,
                predicate,
                object,
            })
        }
        _ => bail!(
            "typed leg is all-or-nothing: --typed-subject + --typed-predicate + one of \
             --typed-object-entity | --typed-object-token | \
             (--typed-object-quantity-n + --typed-object-quantity-unit) | --typed-object-fact"
        ),
    };
    Ok(out)
}

pub fn cmd_add_fact(workspace_root: &Path, args: &[String]) -> Result<(), CliError> {
    let parsed = parse_fact_verb_args(args, false)?;
    let sidecar_path = resolve_sidecar(workspace_root, parsed.sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    finalize_mutate(
        mnemosyne_atomic::add_fact(&mut store, &sidecar_path, &parsed.entry),
        parsed.json,
    )
}

/// Round 434 — authorial in-place revision of an existing fact (the
/// author-correction path; in-world belief change stays `--supersedes`).
pub fn cmd_amend_fact(workspace_root: &Path, args: &[String]) -> Result<(), CliError> {
    let parsed = parse_fact_verb_args(args, true)?;
    let reason = parsed
        .reason
        .ok_or_else(|| anyhow!("--reason arg required"))?;
    let sidecar_path = resolve_sidecar(workspace_root, parsed.sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    finalize_mutate(
        mnemosyne_atomic::amend_fact(&mut store, &sidecar_path, &parsed.entry, &reason),
        parsed.json,
    )
}

/// Round 434 — authorial retract of an unreferenced fact (fail-loud on
/// inbound refs; the retraction's transaction-time audit is git history).
pub fn cmd_retract_fact(workspace_root: &Path, args: &[String]) -> Result<(), CliError> {
    let mut fact_id: Option<String> = None;
    let mut reason: Option<String> = None;
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--fact" => {
                fact_id = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--fact missing"))?
                        .clone(),
                )
            }
            "--reason" => {
                reason = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--reason missing"))?
                        .clone(),
                )
            }
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let fact_id = fact_id.ok_or_else(|| anyhow!("--fact arg required"))?;
    let reason = reason.ok_or_else(|| anyhow!("--reason arg required"))?;
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    finalize_mutate(
        mnemosyne_atomic::retract_fact(&mut store, &sidecar_path, &fact_id, &reason),
        json,
    )
}

/// Round 430 — record one conflict assertion edge between two existing facts.
pub fn cmd_add_fact_conflict(workspace_root: &Path, args: &[String]) -> Result<(), CliError> {
    let mut fact_id: Option<String> = None;
    let mut other: Option<String> = None;
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--fact" => {
                fact_id = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--fact missing"))?
                        .clone(),
                )
            }
            "--conflicts-with" => {
                other = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--conflicts-with missing"))?
                        .clone(),
                )
            }
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other_flag => return Err(anyhow!("unknown flag `{}`", other_flag).into()),
        }
    }
    let fact_id = fact_id.ok_or_else(|| anyhow!("--fact arg required"))?;
    let other = other.ok_or_else(|| anyhow!("--conflicts-with arg required"))?;
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    finalize_mutate(
        mnemosyne_atomic::add_fact_conflict(&mut store, &sidecar_path, &fact_id, &other),
        json,
    )
}

/// R393 — ingest a medium-forge `epub-anchor-map/v1` file, setting each
/// matching Section's `epub_locator` (EPUB-SSOT pointer). One save; ids absent
/// from the store are reported as a note, not an error.
pub fn cmd_import_epub_anchors(workspace_root: &Path, args: &[String]) -> Result<(), CliError> {
    #[derive(serde::Deserialize)]
    struct AnchorEntry {
        id: String,
        locator: mnemosyne_atomic::EpubLocator,
    }
    #[derive(serde::Deserialize)]
    struct AnchorMap {
        anchors: Vec<AnchorEntry>,
    }
    let mut anchors_path: Option<String> = None;
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--anchors" => {
                anchors_path = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--anchors missing"))?
                        .clone(),
                )
            }
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let path = anchors_path.ok_or_else(|| anyhow!("--anchors <path> arg required"))?;
    let raw = fs::read_to_string(&path).with_context(|| format!("read anchors {}", path))?;
    let map: AnchorMap = serde_json::from_str(&raw)
        .with_context(|| format!("parse {} (epub-anchor-map/v1)", path))?;
    let pairs: Vec<(String, mnemosyne_atomic::EpubLocator)> =
        map.anchors.into_iter().map(|a| (a.id, a.locator)).collect();
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    let outcome = mnemosyne_atomic::import_epub_anchors(&mut store, &sidecar_path, &pairs);
    if let Ok((_, unmatched)) = &outcome {
        if !unmatched.is_empty() {
            eprintln!(
                "note: {} anchor id(s) matched no section in the store",
                unmatched.len()
            );
        }
    }
    finalize_mutate(outcome.map(|(receipt, _)| receipt), json)
}

/// Round 287 — outline setter CLI surface. set_section_title sets the
/// heading text on an existing Section (Phase C primitive).
pub fn cmd_set_section_title(workspace_root: &Path, args: &[String]) -> Result<(), CliError> {
    let mut section: Option<String> = None;
    let mut title: Option<String> = None;
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--section" => {
                section = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--section missing"))?
                        .clone(),
                )
            }
            "--title" => {
                title = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--title missing"))?
                        .clone(),
                )
            }
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let section = strip_section_prefix(&section.ok_or_else(|| anyhow!("--section arg required"))?);
    let title = title.ok_or_else(|| anyhow!("--title arg required"))?;
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    finalize_mutate(
        set_section_title(&mut store, &sidecar_path, &section, &title),
        json,
    )
}

/// Round 287 — set Section.parent_doc (doc binding).
pub fn cmd_set_section_parent_doc(workspace_root: &Path, args: &[String]) -> Result<(), CliError> {
    let mut section: Option<String> = None;
    let mut parent_doc: Option<String> = None;
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--section" => {
                section = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--section missing"))?
                        .clone(),
                )
            }
            "--parent-doc" => {
                parent_doc = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--parent-doc missing"))?
                        .clone(),
                )
            }
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let section = strip_section_prefix(&section.ok_or_else(|| anyhow!("--section arg required"))?);
    let parent_doc = parent_doc.ok_or_else(|| anyhow!("--parent-doc arg required"))?;
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    finalize_mutate(
        set_section_parent_doc(&mut store, &sidecar_path, &section, &parent_doc),
        json,
    )
}

/// Round 287 — set Section.parent_section (hierarchy binding). Use `--parent
/// <section_id>` to re-parent; use `--no-parent` to promote to top-level.
/// The two flags are mutually exclusive; exactly one is required.
pub fn cmd_set_section_parent_section(
    workspace_root: &Path,
    args: &[String],
) -> Result<(), CliError> {
    let mut section: Option<String> = None;
    let mut parent: Option<String> = None;
    let mut clear_parent = false;
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--section" => {
                section = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--section missing"))?
                        .clone(),
                )
            }
            "--parent" => {
                parent = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--parent missing"))?
                        .clone(),
                )
            }
            "--no-parent" => clear_parent = true,
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let section = strip_section_prefix(&section.ok_or_else(|| anyhow!("--section arg required"))?);
    if parent.is_some() && clear_parent {
        return Err(anyhow!("--parent and --no-parent are mutually exclusive").into());
    }
    if parent.is_none() && !clear_parent {
        return Err(anyhow!("exactly one of --parent <id> or --no-parent required").into());
    }
    let parent_stripped = parent.as_deref().map(strip_section_prefix);
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    finalize_mutate(
        set_section_parent_section(
            &mut store,
            &sidecar_path,
            &section,
            parent_stripped.as_deref(),
        ),
        json,
    )
}

pub fn cmd_set_section_rationale(workspace_root: &Path, args: &[String]) -> Result<(), CliError> {
    cmd_set_section_bullets(workspace_root, args, "rationale", |s, p, id, b| {
        set_section_rationale(s, p, id, b)
    })
}

pub fn cmd_set_section_inputs(workspace_root: &Path, args: &[String]) -> Result<(), CliError> {
    cmd_set_section_bullets(workspace_root, args, "inputs", |s, p, id, b| {
        set_section_inputs(s, p, id, b)
    })
}

pub fn cmd_set_section_outputs(workspace_root: &Path, args: &[String]) -> Result<(), CliError> {
    cmd_set_section_bullets(workspace_root, args, "outputs", |s, p, id, b| {
        set_section_outputs(s, p, id, b)
    })
}

// Round 295 — publishable-half setters for ChangelogEntry. Mutate only
// publishable_*; audit_* is the permanent record and stays untouched.
// `--entry` arg names the changelog entry (must already exist); the audit
// half can only be authored by `append_changelog_entry`.

pub fn cmd_set_changelog_publishable_decision_summary(
    workspace_root: &Path,
    args: &[String],
) -> Result<(), CliError> {
    cmd_set_changelog_publishable_string(
        workspace_root,
        args,
        "decision_summary",
        mnemosyne_atomic::set_changelog_publishable_decision_summary,
    )
}

pub fn cmd_set_changelog_publishable_changes(
    workspace_root: &Path,
    args: &[String],
) -> Result<(), CliError> {
    cmd_set_changelog_publishable_bullets(
        workspace_root,
        args,
        "publishable_changes",
        mnemosyne_atomic::set_changelog_publishable_changes_bullets,
    )
}

pub fn cmd_set_changelog_publishable_verification(
    workspace_root: &Path,
    args: &[String],
) -> Result<(), CliError> {
    cmd_set_changelog_publishable_bullets(
        workspace_root,
        args,
        "publishable_verification",
        mnemosyne_atomic::set_changelog_publishable_verification_bullets,
    )
}

pub fn cmd_set_changelog_publishable_impact_refs(
    workspace_root: &Path,
    args: &[String],
) -> Result<(), CliError> {
    cmd_set_changelog_publishable_bullets(
        workspace_root,
        args,
        "publishable_impact_refs",
        mnemosyne_atomic::set_changelog_publishable_impact_refs,
    )
}

pub fn cmd_set_changelog_publishable_carry_forward(
    workspace_root: &Path,
    args: &[String],
) -> Result<(), CliError> {
    cmd_set_changelog_publishable_bullets(
        workspace_root,
        args,
        "publishable_carry_forward",
        mnemosyne_atomic::set_changelog_publishable_carry_forward_bullets,
    )
}

/// Round 300 — emit a `[[publishable_override_ledger]]` block for an
/// entry whose publishable half currently diverges from the audit half.
/// Read-only: never writes the sidecar. Pairs with the R295 bare setters
/// so callers who did not use `redact-term` can still obtain a draft to
/// paste into `mnemosyne.toml`.
pub fn cmd_emit_publishable_override_ledger_draft(
    workspace_root: &Path,
    args: &[String],
) -> Result<(), CliError> {
    let mut entry: Option<String> = None;
    let mut reason: Option<String> = None;
    let mut applied_in: Option<String> = None;
    let mut kind = "redaction".to_string();
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(a) = iter.next() {
        match a.as_str() {
            "--entry" => {
                entry = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--entry missing"))?
                        .clone(),
                )
            }
            "--reason" => {
                reason = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--reason missing"))?
                        .clone(),
                )
            }
            "--applied-in" => {
                applied_in = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--applied-in missing"))?
                        .clone(),
                )
            }
            "--kind" => {
                kind = iter
                    .next()
                    .ok_or_else(|| anyhow!("--kind missing"))?
                    .clone()
            }
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let entry = entry.ok_or_else(|| anyhow!("--entry arg required"))?;
    let reason = reason.ok_or_else(|| anyhow!("--reason arg required"))?;
    let applied_in = applied_in.ok_or_else(|| anyhow!("--applied-in arg required"))?;
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    let draft = mnemosyne_atomic::emit_publishable_override_ledger_draft(
        &store,
        &entry,
        &reason,
        &applied_in,
        &kind,
    )
    .map_err(|e| anyhow!("{}", e))?;
    if json {
        let payload = serde_json::json!({
        "primitive": "emit_publishable_override_ledger_draft",
        "entry_id": entry,
        "in_sync": draft.is_none(),
        "ledger_draft": draft,
        });
        println!("{}", payload);
    } else {
        println!("=== mnemosyne-cli emit_publishable_override_ledger_draft ===");
        match draft {
            Some(ref s) => {
                println!("entry: {}", entry);
                println!("status: divergent (paste the block below into mnemosyne.toml)\n");
                print!("{}", s);
            }
            None => {
                println!("entry: {}", entry);
                println!("status: in sync — no ledger row required");
            }
        }
    }
    Ok(())
}

fn cmd_set_changelog_publishable_string(
    workspace_root: &Path,
    args: &[String],
    field: &str,
    primitive: impl Fn(
        &mut AtomicStore,
        &Path,
        &str,
        &str,
    ) -> Result<AtomicMutateReceipt, AtomicMutateError>,
) -> Result<(), CliError> {
    let mut entry: Option<String> = None;
    let mut value: Option<String> = None;
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--entry" => {
                entry = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--entry missing"))?
                        .clone(),
                )
            }
            "--value" => {
                value = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--value missing"))?
                        .clone(),
                )
            }
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let entry = entry.ok_or_else(|| anyhow!("--entry arg required ({} scope)", field))?;
    let value = value.ok_or_else(|| anyhow!("--value arg required ({} scope)", field))?;
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    finalize_mutate(primitive(&mut store, &sidecar_path, &entry, &value), json)
}

fn cmd_set_changelog_publishable_bullets(
    workspace_root: &Path,
    args: &[String],
    field: &str,
    primitive: impl Fn(
        &mut AtomicStore,
        &Path,
        &str,
        &[String],
    ) -> Result<AtomicMutateReceipt, AtomicMutateError>,
) -> Result<(), CliError> {
    let mut entry: Option<String> = None;
    let mut bullets_file: Option<String> = None;
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--entry" => {
                entry = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--entry missing"))?
                        .clone(),
                )
            }
            "--bullets-file" => {
                bullets_file = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--bullets-file missing"))?
                        .clone(),
                )
            }
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let entry = entry.ok_or_else(|| anyhow!("--entry arg required ({} scope)", field))?;
    let bullets_path =
        bullets_file.ok_or_else(|| anyhow!("--bullets-file arg required ({} scope)", field))?;
    let bullets = parse_bullets_file(&bullets_path)?;
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    finalize_mutate(primitive(&mut store, &sidecar_path, &entry, &bullets), json)
}

fn cmd_set_section_bullets(
    workspace_root: &Path,
    args: &[String],
    field: &str,
    primitive: impl Fn(
        &mut AtomicStore,
        &Path,
        &str,
        &[String],
    ) -> Result<AtomicMutateReceipt, AtomicMutateError>,
) -> Result<(), CliError> {
    let mut section: Option<String> = None;
    let mut bullets_file: Option<String> = None;
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--section" => {
                section = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--section missing"))?
                        .clone(),
                )
            }
            "--bullets-file" => {
                bullets_file = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--bullets-file missing"))?
                        .clone(),
                )
            }
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let section = strip_section_prefix(&section.ok_or_else(|| anyhow!("--section arg required"))?);
    let bullets_path =
        bullets_file.ok_or_else(|| anyhow!("--bullets-file arg required ({} scope)", field))?;
    let bullets = parse_bullets_file(&bullets_path)?;
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    finalize_mutate(
        primitive(&mut store, &sidecar_path, &section, &bullets),
        json,
    )
}

pub fn cmd_add_section_caveat(workspace_root: &Path, args: &[String]) -> Result<(), CliError> {
    let mut section: Option<String> = None;
    let mut bullet: Option<String> = None;
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--section" => {
                section = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--section missing"))?
                        .clone(),
                )
            }
            "--bullet" => {
                bullet = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--bullet missing"))?
                        .clone(),
                )
            }
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let section = strip_section_prefix(&section.ok_or_else(|| anyhow!("--section arg required"))?);
    let bullet = bullet.ok_or_else(|| anyhow!("--bullet arg required"))?;
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    finalize_mutate(
        add_section_caveat(&mut store, &sidecar_path, &section, &bullet),
        json,
    )
}

pub fn cmd_set_section_alternatives(
    workspace_root: &Path,
    args: &[String],
) -> Result<(), CliError> {
    let mut section: Option<String> = None;
    let mut alternatives_file: Option<String> = None;
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--section" => {
                section = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--section missing"))?
                        .clone(),
                )
            }
            "--alternatives-file" => {
                alternatives_file = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--alternatives-file missing"))?
                        .clone(),
                )
            }
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let section = strip_section_prefix(&section.ok_or_else(|| anyhow!("--section arg required"))?);
    let path = alternatives_file.ok_or_else(|| anyhow!("--alternatives-file arg required"))?;
    let alts = parse_alternatives_file(&path)?;
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    finalize_mutate(
        set_section_alternatives(&mut store, &sidecar_path, &section, &alts),
        json,
    )
}

pub fn cmd_set_section_impact_scope(
    workspace_root: &Path,
    args: &[String],
) -> Result<(), CliError> {
    let mut section: Option<String> = None;
    let mut refs_csv: Option<String> = None;
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--section" => {
                section = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--section missing"))?
                        .clone(),
                )
            }
            "--refs" => {
                refs_csv = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--refs missing"))?
                        .clone(),
                )
            }
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let section = strip_section_prefix(&section.ok_or_else(|| anyhow!("--section arg required"))?);
    let refs_csv =
        refs_csv.ok_or_else(|| anyhow!("--refs arg required — e.g. --refs '15,39,41'"))?;
    let refs: Vec<String> = refs_csv
        .split(',')
        .map(|r| strip_section_prefix(r.trim()))
        .filter(|r| !r.is_empty())
        .collect();
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    finalize_mutate(
        set_section_impact_scope(&mut store, &sidecar_path, &section, &refs),
        json,
    )
}

pub fn cmd_add_section_example(workspace_root: &Path, args: &[String]) -> Result<(), CliError> {
    let mut section: Option<String> = None;
    let mut language: Option<String> = None;
    let mut code_file: Option<String> = None;
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--section" => {
                section = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--section missing"))?
                        .clone(),
                )
            }
            "--language" => {
                language = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--language missing"))?
                        .clone(),
                )
            }
            "--code-file" => {
                code_file = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--code-file missing"))?
                        .clone(),
                )
            }
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let section = strip_section_prefix(&section.ok_or_else(|| anyhow!("--section arg required"))?);
    let language = language.unwrap_or_default();
    let code_file = code_file.ok_or_else(|| anyhow!("--code-file arg required"))?;
    let code = fs::read_to_string(&code_file)
        .with_context(|| format!("code-file recovery failed: {}", code_file))?;
    let example = ExampleBlock { language, code };
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    finalize_mutate(
        add_section_example(&mut store, &sidecar_path, &section, example),
        json,
    )
}

/// Path B (Spec ↔ Code bidirectional binding) substrate.
///
/// Parse a `--kind` flag value into a [`BindingKind`].
fn parse_binding_kind(raw: &str) -> Result<BindingKind> {
    BindingKind::from_tag(raw.trim()).ok_or_else(|| {
        anyhow!(
            "--kind must be `implements`, `references`, or `verifies` (got `{}`)",
            raw
        )
    })
}

fn parse_coverage_expectation(raw: &str) -> Result<CoverageExpectation> {
    CoverageExpectation::from_tag(raw.trim()).ok_or_else(|| {
        anyhow!(
            "--expectation must be `normative`, `out_of_scope_here`, or \
             `informational` (got `{}`)",
            raw
        )
    })
}

fn parse_verification_expectation(raw: &str) -> Result<VerificationExpectation> {
    VerificationExpectation::from_tag(raw.trim()).ok_or_else(|| {
        anyhow!(
            "--expectation must be `dedicated` or `by_construction` (got `{}`)",
            raw
        )
    })
}

/// Append a `(file, symbol?, kind)` typed trace-link binding to
/// `Section.bindings`. File path is workspace-relative POSIX shape; symbol
/// is opaque (no language grammar regex); `--kind` is required and explicit
/// (`implements` = «satisfy» / `references` = «trace»). Set semantics:
/// duplicate `(file, symbol)` rejected at write time regardless of kind
/// (use `set-section-binding-kind` to change an existing binding's kind).
pub fn cmd_add_section_binding(workspace_root: &Path, args: &[String]) -> Result<(), CliError> {
    let mut section: Option<String> = None;
    let mut file: Option<String> = None;
    let mut symbol: Option<String> = None;
    let mut kind: Option<String> = None;
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--section" => {
                section = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--section missing"))?
                        .clone(),
                )
            }
            "--file" => {
                file = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--file missing"))?
                        .clone(),
                )
            }
            "--symbol" => {
                symbol = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--symbol missing"))?
                        .clone(),
                )
            }
            "--kind" => {
                kind = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--kind missing"))?
                        .clone(),
                )
            }
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let section = strip_section_prefix(&section.ok_or_else(|| anyhow!("--section arg required"))?);
    let file =
        file.ok_or_else(|| anyhow!("--file arg required (workspace-relative POSIX path)"))?;
    let kind = parse_binding_kind(
        &kind.ok_or_else(|| anyhow!("--kind arg required (`implements` or `references`)"))?,
    )?;
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    finalize_mutate(
        add_section_binding(
            &mut store,
            &sidecar_path,
            &section,
            &file,
            symbol.as_deref(),
            kind,
        ),
        json,
    )
}

/// `remove-section-binding` CLI surface.
///
/// `--section §<id> --file <path> [--symbol <name>] --reason <text> [--sidecar <path>] [--json]`
///
/// Removes one `(file, symbol?)` binding from `Section.bindings` (matches on
/// the identity pair regardless of kind). Errors with NotFound when the
/// section or the specific binding is absent. `--reason` mandatory —
/// recorded on the mutate receipt for audit symmetry with `remove-section`
/// (R267) / `remove-inventory-entry` (R274).
pub fn cmd_remove_section_binding(workspace_root: &Path, args: &[String]) -> Result<(), CliError> {
    let mut section: Option<String> = None;
    let mut file: Option<String> = None;
    let mut symbol: Option<String> = None;
    let mut reason: Option<String> = None;
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--section" => {
                section = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--section missing"))?
                        .clone(),
                )
            }
            "--file" => {
                file = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--file missing"))?
                        .clone(),
                )
            }
            "--symbol" => {
                symbol = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--symbol missing"))?
                        .clone(),
                )
            }
            "--reason" => {
                reason = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--reason missing"))?
                        .clone(),
                )
            }
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let section = strip_section_prefix(&section.ok_or_else(|| anyhow!("--section arg required"))?);
    let file =
        file.ok_or_else(|| anyhow!("--file arg required (workspace-relative POSIX path)"))?;
    let reason = reason.ok_or_else(|| anyhow!("--reason arg required (audit safeguard)"))?;
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    finalize_mutate(
        remove_section_binding(
            &mut store,
            &sidecar_path,
            &section,
            &file,
            symbol.as_deref(),
            &reason,
        ),
        json,
    )
}

/// `set-section-binding-kind` CLI surface — reclassify an existing binding.
///
/// `--section §<id> --file <path> [--symbol <name>] --kind implements|references --reason <text> [--sidecar <path>] [--json]`
///
/// Second write path to `Binding.kind` (alongside `add-section-binding
/// --kind`); the binding must already exist. `--reason` mandatory
/// (auditable reclassification). This is the Stage-B reclassification verb
/// (`implements → references` for data/DTO fields).
pub fn cmd_set_section_binding_kind(
    workspace_root: &Path,
    args: &[String],
) -> Result<(), CliError> {
    let mut section: Option<String> = None;
    let mut file: Option<String> = None;
    let mut symbol: Option<String> = None;
    let mut kind: Option<String> = None;
    let mut reason: Option<String> = None;
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--section" => {
                section = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--section missing"))?
                        .clone(),
                )
            }
            "--file" => {
                file = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--file missing"))?
                        .clone(),
                )
            }
            "--symbol" => {
                symbol = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--symbol missing"))?
                        .clone(),
                )
            }
            "--kind" => {
                kind = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--kind missing"))?
                        .clone(),
                )
            }
            "--reason" => {
                reason = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--reason missing"))?
                        .clone(),
                )
            }
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let section = strip_section_prefix(&section.ok_or_else(|| anyhow!("--section arg required"))?);
    let file =
        file.ok_or_else(|| anyhow!("--file arg required (workspace-relative POSIX path)"))?;
    let kind = parse_binding_kind(
        &kind.ok_or_else(|| anyhow!("--kind arg required (`implements` or `references`)"))?,
    )?;
    let reason = reason.ok_or_else(|| anyhow!("--reason arg required (audit safeguard)"))?;
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    finalize_mutate(
        set_section_binding_kind(
            &mut store,
            &sidecar_path,
            &section,
            &file,
            symbol.as_deref(),
            kind,
            &reason,
        ),
        json,
    )
}

pub fn cmd_set_section_coverage_expectation(
    workspace_root: &Path,
    args: &[String],
) -> Result<(), CliError> {
    let mut section: Option<String> = None;
    let mut expectation: Option<String> = None;
    let mut reason: Option<String> = None;
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--section" => {
                section = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--section missing"))?
                        .clone(),
                )
            }
            "--expectation" => {
                expectation = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--expectation missing"))?
                        .clone(),
                )
            }
            "--reason" => {
                reason = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--reason missing"))?
                        .clone(),
                )
            }
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let section = strip_section_prefix(&section.ok_or_else(|| anyhow!("--section arg required"))?);
    let expectation =
        parse_coverage_expectation(&expectation.ok_or_else(|| {
            anyhow!("--expectation arg required (`normative` or `informative`)")
        })?)?;
    let reason = reason.ok_or_else(|| anyhow!("--reason arg required (audit safeguard)"))?;
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    finalize_mutate(
        set_section_coverage_expectation(&mut store, &sidecar_path, &section, expectation, &reason),
        json,
    )
}

/// R413 — classify a section's verification expectation
/// (`dedicated` | `by_construction`).
///
/// `--section §<id> --expectation dedicated|by_construction --reason <text>
///   [--sidecar <path>] [--json]`
pub fn cmd_set_section_verification_expectation(
    workspace_root: &Path,
    args: &[String],
) -> Result<(), CliError> {
    let mut section: Option<String> = None;
    let mut expectation: Option<String> = None;
    let mut reason: Option<String> = None;
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--section" => {
                section = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--section missing"))?
                        .clone(),
                )
            }
            "--expectation" => {
                expectation = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--expectation missing"))?
                        .clone(),
                )
            }
            "--reason" => {
                reason = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--reason missing"))?
                        .clone(),
                )
            }
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let section = strip_section_prefix(&section.ok_or_else(|| anyhow!("--section arg required"))?);
    let expectation = parse_verification_expectation(&expectation.ok_or_else(|| {
        anyhow!("--expectation arg required (`dedicated` or `by_construction`)")
    })?)?;
    let reason = reason.ok_or_else(|| anyhow!("--reason arg required (audit safeguard)"))?;
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    finalize_mutate(
        set_section_verification_expectation(
            &mut store,
            &sidecar_path,
            &section,
            expectation,
            &reason,
        ),
        json,
    )
}

/// R417 — confirmation-event CLI surface. Builds a `ConfirmationEvent` from
/// flags and appends it; the `event_id` is derived in-core (never supplied).
/// A `--file` present makes it a `VerifiesBinding` claim, else a
/// `SectionCompleteness` claim. Enum flags take the snake_case tag.
pub fn cmd_add_confirmation_event(workspace_root: &Path, args: &[String]) -> Result<(), CliError> {
    let mut section: Option<String> = None;
    let mut file: Option<String> = None;
    let mut symbol: Option<String> = None;
    let mut confirmer_kind: Option<String> = None;
    let mut confirmer_id: Option<String> = None;
    let mut confirmer_version: Option<String> = None;
    let mut method: Option<String> = None;
    let mut verdict: Option<String> = None;
    let mut authoring_run: Option<String> = None;
    let mut confirming_run: Option<String> = None;
    let mut rationale: Option<String> = None;
    let mut timestamp: Option<String> = None;
    let mut spec_sha256: Option<String> = None;
    let mut code_sha256: Vec<String> = Vec::new();
    let mut test_sha256: Vec<String> = Vec::new();
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--section" => {
                section = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--section missing"))?
                        .clone(),
                )
            }
            "--file" => {
                file = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--file missing"))?
                        .clone(),
                )
            }
            "--symbol" => {
                symbol = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--symbol missing"))?
                        .clone(),
                )
            }
            "--confirmer-kind" => {
                confirmer_kind = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--confirmer-kind missing"))?
                        .clone(),
                )
            }
            "--confirmer-id" => {
                confirmer_id = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--confirmer-id missing"))?
                        .clone(),
                )
            }
            "--confirmer-version" => {
                confirmer_version = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--confirmer-version missing"))?
                        .clone(),
                )
            }
            "--method" => {
                method = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--method missing"))?
                        .clone(),
                )
            }
            "--verdict" => {
                verdict = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--verdict missing"))?
                        .clone(),
                )
            }
            "--authoring-run" => {
                authoring_run = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--authoring-run missing"))?
                        .clone(),
                )
            }
            "--confirming-run" => {
                confirming_run = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--confirming-run missing"))?
                        .clone(),
                )
            }
            "--rationale" => {
                rationale = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--rationale missing"))?
                        .clone(),
                )
            }
            "--timestamp" => {
                timestamp = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--timestamp missing"))?
                        .clone(),
                )
            }
            "--spec-sha256" => {
                spec_sha256 = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--spec-sha256 missing"))?
                        .clone(),
                )
            }
            "--code-sha256" => code_sha256.push(
                iter.next()
                    .ok_or_else(|| anyhow!("--code-sha256 missing"))?
                    .clone(),
            ),
            "--test-sha256" => test_sha256.push(
                iter.next()
                    .ok_or_else(|| anyhow!("--test-sha256 missing"))?
                    .clone(),
            ),
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let section = strip_section_prefix(&section.ok_or_else(|| anyhow!("--section arg required"))?);
    let claim = match file {
        Some(f) => ConfirmationClaim::VerifiesBinding {
            section_id: section,
            file: f,
            symbol,
        },
        None => {
            if symbol.is_some() {
                return Err(anyhow!("--symbol requires --file (a VerifiesBinding claim)").into());
            }
            ConfirmationClaim::SectionCompleteness {
                section_id: section,
            }
        }
    };
    let confirmer = Confirmer {
        kind: ConfirmerKind::from_tag(
            confirmer_kind
                .as_deref()
                .ok_or_else(|| anyhow!("--confirmer-kind arg required (`tool` or `model`)"))?
                .trim(),
        )
        .ok_or_else(|| anyhow!("--confirmer-kind must be `tool` or `model`"))?,
        id: confirmer_id.ok_or_else(|| anyhow!("--confirmer-id arg required"))?,
        version: confirmer_version.ok_or_else(|| anyhow!("--confirmer-version arg required"))?,
    };
    let method = ConfirmMethod::from_tag(
        method
            .as_deref()
            .ok_or_else(|| anyhow!("--method arg required"))?
            .trim(),
    )
    .ok_or_else(|| {
        anyhow!("--method must be `linkage_check`, `semantic_review`, or `coverage_attestation`")
    })?;
    let verdict = Verdict::from_tag(
        verdict
            .as_deref()
            .ok_or_else(|| anyhow!("--verdict arg required"))?
            .trim(),
    )
    .ok_or_else(|| anyhow!("--verdict must be `confirm` or `refute`"))?;
    let event = ConfirmationEvent {
        claim,
        confirmer,
        method,
        artifact_hashes: ArtifactHashes {
            spec_sha256,
            code_sha256,
            test_sha256,
        },
        authoring_run: authoring_run.ok_or_else(|| anyhow!("--authoring-run arg required"))?,
        confirming_run: confirming_run.ok_or_else(|| anyhow!("--confirming-run arg required"))?,
        verdict,
        rationale: rationale.ok_or_else(|| anyhow!("--rationale arg required"))?,
        timestamp: timestamp.ok_or_else(|| anyhow!("--timestamp arg required"))?,
    };
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    finalize_mutate(
        append_confirmation_event(&mut store, &sidecar_path, event),
        json,
    )
}

/// Round 267 — section removal CLI surface.
///
/// `--section §<id> --reason <text> [--sidecar <path>] [--json]`
///
/// Removes a section from the atomic store. Requires `--reason` (audit
/// safeguard). Errors with NotFound when the section_id is absent — no
/// silent no-op, the caller asked for a specific removal.
pub fn cmd_remove_section(workspace_root: &Path, args: &[String]) -> Result<(), CliError> {
    let mut section: Option<String> = None;
    let mut reason: Option<String> = None;
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--section" => {
                section = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--section missing"))?
                        .clone(),
                )
            }
            "--reason" => {
                reason = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--reason missing"))?
                        .clone(),
                )
            }
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let section = strip_section_prefix(&section.ok_or_else(|| anyhow!("--section arg required"))?);
    let reason = reason.ok_or_else(|| anyhow!("--reason arg required (audit safeguard)"))?;
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    finalize_mutate(
        remove_section(&mut store, &sidecar_path, &section, &reason),
        json,
    )
}

/// Round 265 — atomic decision_status setter CLI surface.
///
/// `--section §<id> --status active|superseded|removed|open [--superseding §<M>] [--resolving §<M>] [--sidecar <path>] [--json]`
///
/// Sets `AtomicSection.decision_status` on the atomic store. Stage B
/// freshness substrate — once the atomic store carries non-Active status,
/// downstream tooling (auto-cascade trigger, decay scan) becomes wireable.
pub fn cmd_set_section_decision_status(
    workspace_root: &Path,
    args: &[String],
) -> Result<(), CliError> {
    let mut section: Option<String> = None;
    let mut status_str: Option<String> = None;
    let mut superseding: Option<String> = None;
    let mut resolving: Option<String> = None;
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--section" => {
                section = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--section missing"))?
                        .clone(),
                )
            }
            "--status" => {
                status_str = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--status missing"))?
                        .clone(),
                )
            }
            "--superseding" => {
                superseding = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--superseding missing"))?
                        .clone(),
                )
            }
            "--resolving" => {
                resolving = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--resolving missing"))?
                        .clone(),
                )
            }
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let section = strip_section_prefix(&section.ok_or_else(|| anyhow!("--section arg required"))?);
    let status_raw = status_str
        .ok_or_else(|| anyhow!("--status arg required (active|superseded|removed|open)"))?;
    let new_status =
        DecisionStatus::from_tag(&status_raw.to_ascii_lowercase()).ok_or_else(|| {
            anyhow!(
                "--status `{}` invalid (expected active|superseded|removed|open)",
                status_raw
            )
        })?;
    // T1 rule 4 + the superseding/resolving pointer guards are homed in
    // `atomic::set_section_decision_status` (R678), so the CLI and MCP write
    // paths enforce the identical invariant set — no CLI-only guard for the MCP
    // path to undercut.
    let superseding_strip = superseding.as_deref().map(strip_section_prefix);
    let resolving_strip = resolving.as_deref().map(strip_section_prefix);
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    let mutate_result = set_section_decision_status(
        &mut store,
        &sidecar_path,
        &section,
        new_status,
        superseding_strip.as_deref(),
        resolving_strip.as_deref(),
    );

    // Round 266 — auto-cascade trigger (Stage B freshness). When the new
    // status is Superseded or Removed, run a targeted §<id> decay scan
    // against [plugins.set_equality_validator].paths and surface citing locations to stderr.
    // Informational only — never alters the mutate's success/failure.
    // No-op when [plugins.set_equality_validator] is unconfigured (5-min setup promise carry).
    if mutate_result.is_ok()
        && matches!(
            new_status,
            DecisionStatus::Superseded | DecisionStatus::Removed
        )
    {
        print_section_decay_trigger(workspace_root, &section, new_status);
    }

    finalize_mutate(mutate_result, json)
}

/// External-spec mirror — anchor a vendored normative excerpt onto a
/// Section (RFC-002 FR-1). Frozen-ledger semantic: the primitive
/// rejects overwrite, so spec revision drift is modeled by superseding
/// the existing Section and creating a new one with the updated
/// excerpt.
///
/// `--text-file` carries multi-line spec quotes verbatim (preserved
/// trailing newline trimmed). `--anchor-url` must be an absolute
/// http(s) URL. `--source-revision` is the upstream rev identifier
/// that was current when the excerpt was captured.
/// R403 — refresh `normative_excerpt.text` (+ `text_sha256`) from a medium-forge
/// `epub-anchor-map/v2`. Per-section text is the EPUB-projected cache; the
/// authored `anchor_url` + `source_revision` are preserved from the existing
/// excerpt (a section must already carry one to be refreshable). Ids that match
/// no refreshable excerpt are reported as a note, not an error. Replaces the
/// deleted hand-authoring `set-section-normative-excerpt` verb.
pub fn cmd_import_epub_excerpts(workspace_root: &Path, args: &[String]) -> Result<(), CliError> {
    #[derive(serde::Deserialize)]
    struct ExcerptEntry {
        id: String,
        #[serde(default)]
        text: Option<String>,
        #[serde(default)]
        text_sha256: Option<String>,
    }
    #[derive(serde::Deserialize)]
    struct ExcerptAnchorMap {
        anchors: Vec<ExcerptEntry>,
    }
    let mut anchors_path: Option<String> = None;
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--anchors" => {
                anchors_path = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--anchors missing"))?
                        .clone(),
                )
            }
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let path = anchors_path.ok_or_else(|| anyhow!("--anchors <path> arg required"))?;
    let raw = fs::read_to_string(&path).with_context(|| format!("read anchors {}", path))?;
    let map: ExcerptAnchorMap = serde_json::from_str(&raw)
        .with_context(|| format!("parse {} (epub-anchor-map/v2)", path))?;
    // Only v2 entries carry text + text_sha256; v1/locator-only entries are skipped.
    let excerpts: Vec<mnemosyne_atomic::ExcerptImport> = map
        .anchors
        .into_iter()
        .filter_map(|a| match (a.text, a.text_sha256) {
            (Some(text), Some(text_sha256)) => Some(mnemosyne_atomic::ExcerptImport {
                section_id: a.id,
                text,
                text_sha256,
            }),
            _ => None,
        })
        .collect();
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    let outcome = mnemosyne_atomic::import_epub_excerpts(&mut store, &sidecar_path, &excerpts);
    if let Ok((_, unmatched)) = &outcome {
        if !unmatched.is_empty() {
            eprintln!(
                "note: {} id(s) matched no refreshable excerpt in the store",
                unmatched.len()
            );
        }
    }
    finalize_mutate(outcome.map(|(receipt, _)| receipt), json)
}

/// Round 266 — mutate-time auto-cascade trigger.
///
/// Runs a §<section_id> decay scan over `[plugins.set_equality_validator].paths` and prints a
/// short report to stderr. Silent no-op when `[plugins.set_equality_validator]` is unconfigured.
/// Errors during config load or scan are logged but never propagated — the
/// mutate's success boundary stays clean.
fn print_section_decay_trigger(
    workspace_root: &Path,
    section_id: &str,
    new_status: DecisionStatus,
) {
    let loaded = match discover_config(workspace_root) {
        Ok(Some(cfg)) => cfg,
        Ok(None) => return,
        Err(e) => {
            eprintln!(
                "[cascade] decay-trigger skipped (config load failed: {})",
                e
            );
            return;
        }
    };
    let code_refs_cfg = match loaded
        .config
        .plugins
        .as_ref()
        .and_then(|p| p.set_equality_validator.as_ref())
    {
        Some(c) if !c.paths.is_empty() => c,
        _ => return,
    };
    let hits = match scan_section_decay(
        workspace_root,
        &code_refs_cfg.paths,
        section_id,
        code_refs_cfg.comment_only,
    ) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("[cascade] decay-trigger scan io error: {}", e);
            return;
        }
    };
    let status_label = new_status.as_str();
    eprintln!(
        "[cascade] §{} → {} — {} citing location(s) in [plugins.set_equality_validator].paths",
        section_id,
        status_label,
        hits.len()
    );
    for c in &hits {
        eprintln!(" {}:{} §{}", c.file.display(), c.line, section_id);
    }
}

/// Round 276 — Inventory mutate-time auto-cascade trigger (Phase 1A).
///
/// Mirrors [`print_section_decay_trigger`] for the inventory axis. Runs a
/// targeted decay scan for `inventory_id` over `[plugins.set_equality_validator].paths` and
/// prints a short stderr report. Silent no-op when `[plugins.set_equality_validator]` is
/// unconfigured or `inventory_prefixes` is empty (axis disabled).
/// Errors during config load or scan are logged but never propagated —
/// the mutate's success boundary stays clean.
///
/// `transition_label` is rendered into the cascade line so the operator
/// sees what kind of transition prompted the cascade:
/// `"deprecated"`, `"removed"`, or `"added(deprecated)"`.
fn print_inventory_decay_trigger(
    workspace_root: &Path,
    inventory_id: &str,
    transition_label: &str,
) {
    let loaded = match discover_config(workspace_root) {
        Ok(Some(cfg)) => cfg,
        Ok(None) => return,
        Err(e) => {
            eprintln!(
                "[cascade] inventory-decay-trigger skipped (config load failed: {})",
                e
            );
            return;
        }
    };
    let code_refs_cfg = match loaded
        .config
        .plugins
        .as_ref()
        .and_then(|p| p.set_equality_validator.as_ref())
    {
        Some(c)
            if !c.paths.is_empty()
                && (!c.inventory_prefixes.is_empty() || !c.inventory_path_prefixes.is_empty()) =>
        {
            c
        }
        _ => return,
    };
    let hits = match scan_inventory_decay(
        workspace_root,
        &code_refs_cfg.paths,
        inventory_id,
        &code_refs_cfg.inventory_prefixes,
        &code_refs_cfg.inventory_path_prefixes,
        code_refs_cfg.comment_only,
    ) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("[cascade] inventory-decay-trigger scan io error: {}", e);
            return;
        }
    };
    eprintln!(
        "[cascade] {} → {} — {} citing location(s) in [plugins.set_equality_validator].paths",
        inventory_id,
        transition_label,
        hits.len()
    );
    for c in &hits {
        eprintln!(" {}:{} {}", c.file.display(), c.line, c.entry_id);
    }
}

/// Complete a mutate primitive call: on success print the receipt; on failure
/// print the formatted error (the `--json` blob or the `FAILED` header +
/// detail) and return [`CliError::AlreadyReported`] so `main` exits non-zero
/// without reprinting. The atomic store is the only artifact, so there is
/// nothing to regenerate.
///
/// This is the ONE place the atomic-mutate path emits an error to stderr; the
/// returned variant — not a marker recovered by `downcast` — is what keeps
/// `main` from printing it a second time.
fn finalize_mutate(
    result: Result<AtomicMutateReceipt, AtomicMutateError>,
    json: bool,
) -> Result<(), CliError> {
    match result {
        Ok(receipt) => {
            print_receipt(&receipt, json);
            Ok(())
        }
        Err(error) => {
            print_error(&error, json);
            Err(CliError::AlreadyReported)
        }
    }
}

pub fn cmd_append_changelog_entry(workspace_root: &Path, args: &[String]) -> Result<(), CliError> {
    let mut entry_id: Option<String> = None;
    let mut decision_summary: Option<String> = None;
    let mut changes_file: Option<String> = None;
    let mut verification_file: Option<String> = None;
    let mut impact_csv: Option<String> = None;
    let mut carry_file: Option<String> = None;
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--entry-id" => {
                entry_id = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--entry-id missing"))?
                        .clone(),
                )
            }
            "--decision" => {
                decision_summary = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--decision missing"))?
                        .clone(),
                )
            }
            "--changes-file" => {
                changes_file = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--changes-file missing"))?
                        .clone(),
                )
            }
            "--verification-file" => {
                verification_file = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--verification-file missing"))?
                        .clone(),
                )
            }
            "--impact" => {
                impact_csv = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--impact missing"))?
                        .clone(),
                )
            }
            "--carry-file" => {
                carry_file = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--carry-file missing"))?
                        .clone(),
                )
            }
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let entry_id = entry_id.ok_or_else(|| anyhow!("--entry-id arg required"))?;
    let changes = changes_file
        .as_deref()
        .map(parse_bullets_file)
        .transpose()?
        .unwrap_or_default();
    let verification = verification_file
        .as_deref()
        .map(parse_bullets_file)
        .transpose()?
        .unwrap_or_default();
    let carry_forward = carry_file
        .as_deref()
        .map(parse_bullets_file)
        .transpose()?
        .unwrap_or_default();
    let impact_refs: Vec<String> = impact_csv
        .as_deref()
        .map(|csv| {
            csv.split(',')
                .map(|r| strip_section_prefix(r.trim()))
                .filter(|r| !r.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    // Round 424 — append conformance gate policy, resolved through the
    // single shared path (CLI + MCP parity).
    let entry_id_prefix =
        mnemosyne_ops::workspace_entry_id_prefix(workspace_root).map_err(|e| anyhow!("{}", e))?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    finalize_mutate(
        append_changelog_entry(
            &mut store,
            &sidecar_path,
            ChangelogEntryDraft {
                entry_id: &entry_id,
                decision_summary: decision_summary.as_deref(),
                changes_bullets: &changes,
                verification_bullets: &verification,
                impact_refs: &impact_refs,
                carry_forward_bullets: &carry_forward,
            },
            &entry_id_prefix,
        ),
        json,
    )
}

// ============================================================================
// Inventory mutate CLI handlers (Round 274, Phase 1A).
// ============================================================================

fn parse_inventory_status(raw: &str) -> Result<InventoryStatus> {
    raw.parse::<InventoryStatus>()
        .map_err(|e| anyhow!("--status {}", e))
}

/// `add-inventory-entry --id <ID> --status active|deprecated|reserved \
///   [--section §<N>] [--source <text>] [--reason <text>] \
///   [--sidecar <path>] [--json]`
pub fn cmd_add_inventory_entry(workspace_root: &Path, args: &[String]) -> Result<(), CliError> {
    let mut inventory_id: Option<String> = None;
    let mut status_str: Option<String> = None;
    let mut section_ref: Option<String> = None;
    let mut source: Option<String> = None;
    let mut reason: Option<String> = None;
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--id" => {
                inventory_id = Some(iter.next().ok_or_else(|| anyhow!("--id missing"))?.clone())
            }
            "--status" => {
                status_str = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--status missing"))?
                        .clone(),
                )
            }
            "--section" => {
                section_ref = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--section missing"))?
                        .clone(),
                )
            }
            "--source" => {
                source = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--source missing"))?
                        .clone(),
                )
            }
            "--reason" => {
                reason = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--reason missing"))?
                        .clone(),
                )
            }
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let inventory_id = inventory_id.ok_or_else(|| anyhow!("--id arg required"))?;
    let status = parse_inventory_status(
        status_str
            .as_deref()
            .ok_or_else(|| anyhow!("--status arg required (active|deprecated|reserved)"))?,
    )?;
    let section_ref_clean = section_ref.as_deref().map(strip_section_prefix);
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    let mutate_result = add_inventory_entry(
        &mut store,
        &sidecar_path,
        &inventory_id,
        status,
        section_ref_clean.as_deref(),
        source.as_deref(),
        reason.as_deref(),
    );

    // Round 276 — cascade trigger when registering an already-Deprecated
    // entry (typical when syncing from an external SSOT where the source
    // row is already retired). Reserved / Active registrations do not
    // trigger — there is nothing yet that could be a stale cite-site.
    if mutate_result.is_ok() && status == InventoryStatus::Deprecated {
        print_inventory_decay_trigger(workspace_root, &inventory_id, "added(deprecated)");
    }

    finalize_mutate(mutate_result, json)
}

/// `set-inventory-status --id <ID> --status active|deprecated|reserved \
///   [--reason <text>] [--sidecar <path>] [--json]`
///
/// `--reason` semantics: omitted = preserve existing; supplied = overwrite
/// (empty string clears).
pub fn cmd_set_inventory_status(workspace_root: &Path, args: &[String]) -> Result<(), CliError> {
    let mut inventory_id: Option<String> = None;
    let mut status_str: Option<String> = None;
    let mut reason: Option<String> = None;
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--id" => {
                inventory_id = Some(iter.next().ok_or_else(|| anyhow!("--id missing"))?.clone())
            }
            "--status" => {
                status_str = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--status missing"))?
                        .clone(),
                )
            }
            "--reason" => {
                reason = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--reason missing"))?
                        .clone(),
                )
            }
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let inventory_id = inventory_id.ok_or_else(|| anyhow!("--id arg required"))?;
    let status = parse_inventory_status(
        status_str
            .as_deref()
            .ok_or_else(|| anyhow!("--status arg required (active|deprecated|reserved)"))?,
    )?;
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    let mutate_result = set_inventory_status(
        &mut store,
        &sidecar_path,
        &inventory_id,
        status,
        reason.as_deref(),
    );

    // Round 276 — cascade trigger on Active/Reserved → Deprecated
    // transition. Deprecated → Active (reactivation) and other
    // non-Deprecated targets do not trigger; the cascade surfaces
    // *stale-cite risk*, not lifecycle audits in general.
    if mutate_result.is_ok() && status == InventoryStatus::Deprecated {
        print_inventory_decay_trigger(workspace_root, &inventory_id, "deprecated");
    }

    finalize_mutate(mutate_result, json)
}

/// `set-inventory-section-ref --id <ID> (--section §<N> | --clear) \
///   [--sidecar <path>] [--json]`
///
/// Exactly one of `--section` or `--clear` is required.
pub fn cmd_set_inventory_section_ref(
    workspace_root: &Path,
    args: &[String],
) -> Result<(), CliError> {
    let mut inventory_id: Option<String> = None;
    let mut section_ref: Option<String> = None;
    let mut clear = false;
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--id" => {
                inventory_id = Some(iter.next().ok_or_else(|| anyhow!("--id missing"))?.clone())
            }
            "--section" => {
                section_ref = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--section missing"))?
                        .clone(),
                )
            }
            "--clear" => clear = true,
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let inventory_id = inventory_id.ok_or_else(|| anyhow!("--id arg required"))?;
    if section_ref.is_some() == clear {
        return Err(anyhow!("exactly one of --section or --clear must be supplied").into());
    }
    let cleaned: Option<String> = section_ref.as_deref().map(strip_section_prefix);
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    finalize_mutate(
        set_inventory_section_ref(&mut store, &sidecar_path, &inventory_id, cleaned.as_deref()),
        json,
    )
}

/// `remove-inventory-entry --id <ID> --reason <text> [--sidecar <path>] [--json]`
pub fn cmd_remove_inventory_entry(workspace_root: &Path, args: &[String]) -> Result<(), CliError> {
    let mut inventory_id: Option<String> = None;
    let mut reason: Option<String> = None;
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--id" => {
                inventory_id = Some(iter.next().ok_or_else(|| anyhow!("--id missing"))?.clone())
            }
            "--reason" => {
                reason = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--reason missing"))?
                        .clone(),
                )
            }
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let inventory_id = inventory_id.ok_or_else(|| anyhow!("--id arg required"))?;
    let reason = reason.ok_or_else(|| anyhow!("--reason arg required (audit safeguard)"))?;
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    let mutate_result = remove_inventory_entry(&mut store, &sidecar_path, &inventory_id, &reason);

    // Round 276 — cascade trigger on every successful remove. The entry
    // ceasing to exist promotes any extant cite to InventoryMissing
    // on the next validate-code-refs run; the cascade surfaces those
    // cites mutate-time so the author can act before pre-commit gates.
    if mutate_result.is_ok() {
        print_inventory_decay_trigger(workspace_root, &inventory_id, "removed");
    }

    finalize_mutate(mutate_result, json)
}

/// Round 297 — `redact-term` CLI subcommand (RFC P1).
///
/// Wraps `mnemosyne_atomic::redact_term`. By default `--dry-run`
/// is **off** — explicit safety contract: a redaction without `--dry-run`
/// mutates publishable_* in place. The output prints both the per-hit
/// summary and the ready-to-paste `[[publishable_override_ledger]]` draft
/// blocks so authors do not hand-author SHA256 anchors.
pub fn cmd_redact_term(workspace_root: &Path, args: &[String]) -> Result<(), CliError> {
    let mut pattern: Option<String> = None;
    let mut replacement: Option<String> = None;
    let mut mode = mnemosyne_atomic::RedactMode::Literal;
    let mut case_insensitive = false;
    let mut scope = mnemosyne_atomic::RedactScope::All;
    let mut dry_run = false;
    let mut reason: Option<String> = None;
    let mut applied_in: Option<String> = None;
    let mut kind = "redaction".to_string();
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(a) = iter.next() {
        match a.as_str() {
            "--pattern" => {
                pattern = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--pattern missing"))?
                        .clone(),
                )
            }
            "--replacement" => {
                replacement = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--replacement missing"))?
                        .clone(),
                )
            }
            "--regex" => mode = mnemosyne_atomic::RedactMode::Regex,
            "-i" | "--case-insensitive" => case_insensitive = true,
            "--scope" => {
                let raw = iter
                    .next()
                    .ok_or_else(|| anyhow!("--scope missing value"))?
                    .clone();
                scope = match raw.as_str() {
                    "all" => mnemosyne_atomic::RedactScope::All,
                    "decision_summary" | "publishable_decision_summary" => {
                        mnemosyne_atomic::RedactScope::DecisionSummary
                    }
                    "changes_bullets" | "publishable_changes_bullets" => {
                        mnemosyne_atomic::RedactScope::ChangesBullets
                    }
                    "verification_bullets" | "publishable_verification_bullets" => {
                        mnemosyne_atomic::RedactScope::VerificationBullets
                    }
                    "impact_refs" | "publishable_impact_refs" => {
                        mnemosyne_atomic::RedactScope::ImpactRefs
                    }
                    "carry_forward_bullets" | "publishable_carry_forward_bullets" => {
                        mnemosyne_atomic::RedactScope::CarryForwardBullets
                    }
                    other => {
                        return Err(anyhow!(
                            "unknown --scope `{}` — expected: all | decision_summary \
                             | changes_bullets | verification_bullets | impact_refs \
                             | carry_forward_bullets",
                            other
                        )
                        .into())
                    }
                };
            }
            "--dry-run" => dry_run = true,
            "--reason" => {
                reason = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--reason missing"))?
                        .clone(),
                )
            }
            "--applied-in" => {
                applied_in = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--applied-in missing"))?
                        .clone(),
                )
            }
            "--kind" => {
                kind = iter
                    .next()
                    .ok_or_else(|| anyhow!("--kind missing"))?
                    .clone()
            }
            "--sidecar" => {
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => return Err(anyhow!("unknown flag `{}`", other).into()),
        }
    }
    let req = mnemosyne_atomic::RedactRequest {
        pattern: pattern.ok_or_else(|| anyhow!("--pattern arg required"))?,
        replacement: replacement.ok_or_else(|| anyhow!("--replacement arg required"))?,
        mode,
        case_insensitive,
        scope,
        dry_run,
        reason: reason.ok_or_else(|| anyhow!("--reason arg required"))?,
        applied_in: applied_in.ok_or_else(|| anyhow!("--applied-in arg required"))?,
        kind,
    };
    let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref())?;
    let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
    let report = mnemosyne_atomic::redact_term(&mut store, &sidecar_path, &req)
        .map_err(|e| anyhow!("{}", e))?;
    if json {
        let payload = serde_json::json!({
        "primitive": "redact_term",
        "dry_run": report.dry_run,
        "hits": report
        .hits
        .iter()
        .map(|h| {
         serde_json::json!({
         "entry_id": h.entry_id,
         "field": h.field,
         "index": h.index,
         "original": h.original,
         "redacted": h.redacted,
         })
        })
        .collect::<Vec<_>>(),
        "ledger_drafts": report.ledger_drafts,
        });
        println!("{}", payload);
    } else {
        println!(
            "=== mnemosyne-cli redact_term ({}) ===",
            if report.dry_run { "dry-run" } else { "applied" }
        );
        println!(
            "hits: {} across {} entry(ies)",
            report.hits.len(),
            report.touched_entries().len()
        );
        for h in &report.hits {
            let loc = match h.index {
                Some(i) => format!("{}[{}]", h.field, i),
                None => h.field.to_string(),
            };
            println!(
                "  {} {}: `{}` -> `{}`",
                h.entry_id, loc, h.original, h.redacted
            );
        }
        if !report.ledger_drafts.is_empty() {
            println!();
            println!("--- [[publishable_override_ledger]] drafts (paste into mnemosyne.toml) ---");
            for draft in &report.ledger_drafts {
                println!("{}", draft);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_toml(root: &Path, body: &str) {
        std::fs::write(root.join("mnemosyne.toml"), body).unwrap();
    }

    // Round 279 Bug #2 — atomic.sidecar_path resolution chain.

    #[test]
    fn resolve_sidecar_cli_flag_overrides_config() {
        let tmp = TempDir::new().unwrap();
        write_toml(
            tmp.path(),
            r#"
[workspace]

[atomic]
sidecar_path = "from-config.json"
"#,
        );
        let resolved = resolve_sidecar(tmp.path(), Some("from-cli.json")).unwrap();
        // R538 — the explicit override wins over the config AND resolves
        // CWD-relative (not the config path, not the anchor): it short-circuits
        // config discovery entirely.
        let cwd = std::env::current_dir().unwrap();
        assert_eq!(resolved, cwd.join("from-cli.json"));
        assert_ne!(resolved, tmp.path().join("from-config.json"));
    }

    #[test]
    fn resolve_sidecar_config_used_when_cli_omitted() {
        let tmp = TempDir::new().unwrap();
        write_toml(
            tmp.path(),
            r#"
[workspace]

[atomic]
sidecar_path = "altdir/custom.atomic.json"
"#,
        );
        let resolved = resolve_sidecar(tmp.path(), None).unwrap();
        assert_eq!(resolved, tmp.path().join("altdir/custom.atomic.json"));
    }

    #[test]
    fn resolve_sidecar_built_in_default_without_config() {
        let tmp = TempDir::new().unwrap();
        let resolved = resolve_sidecar(tmp.path(), None).unwrap();
        assert_eq!(
            resolved,
            tmp.path().join("docs/.atomic/workspace.atomic.json")
        );
    }

    #[test]
    fn resolve_sidecar_absolute_path_passthrough() {
        let tmp = TempDir::new().unwrap();
        let abs = tmp.path().join("absolute/here.json");
        let resolved = resolve_sidecar(tmp.path(), Some(abs.to_str().unwrap())).unwrap();
        assert_eq!(resolved, abs);
    }

    // Round 362 — a malformed `mnemosyne.toml` fails loud instead of
    // silently falling back to the built-in default path (the prior
    // `if let Ok(Some(..)) = discover_config(..)` swallowed the parse Err).
    #[test]
    fn resolve_sidecar_malformed_config_fails_loud() {
        let tmp = TempDir::new().unwrap();
        write_toml(tmp.path(), "[atomic\nsidecar_path = \"x.json\"\n");
        assert!(resolve_sidecar(tmp.path(), None).is_err());
    }

    #[test]
    fn resolve_sidecar_explicit_override_ignores_malformed_config() {
        // The explicit override short-circuits before config discovery, so a
        // malformed config must not block an explicitly-pathed resolve. R538:
        // it resolves CWD-relative.
        let tmp = TempDir::new().unwrap();
        write_toml(tmp.path(), "[atomic\nsidecar_path = \"x.json\"\n");
        let resolved = resolve_sidecar(tmp.path(), Some("from-cli.json")).unwrap();
        assert_eq!(
            resolved,
            std::env::current_dir().unwrap().join("from-cli.json")
        );
    }
}
