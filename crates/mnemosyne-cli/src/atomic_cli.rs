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
//! - `add-section-implementation` — append to Section.implementations
//!
//! - `append-changelog-entry-v2` — atomic-aware changelog append
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

use anyhow::{anyhow, bail, Context, Result};
use mnemosyne_validator::{
 add_inventory_entry, add_section_caveat, add_section_example,
 add_section_implementation, append_changelog_entry_v2,
 code_refs::{scan_inventory_decay, scan_section_decay}, discover_config,
 remove_inventory_entry, remove_section, remove_section_implementation,
 render_changelog_entry, render_section, set_inventory_section_ref, set_inventory_status,
 set_section_alternatives, set_section_decision_status_atomic,
 set_section_impact_scope, set_section_inputs, set_section_intent,
 set_section_outputs, set_section_parent_doc, set_section_parent_section,
 set_section_rationale, set_section_title, AtomicMutateError,
 AtomicMutateReceipt, AtomicStore, DecisionStatus, ExampleBlock,
 InventoryStatus, RejectedAlternative,
};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Resolve sidecar path with the Round 279 precedence chain:
/// 1. Explicit `--sidecar` CLI flag wins absolutely.
/// 2. `[atomic] sidecar_path` from `mnemosyne.toml` (workspace-relative
/// or absolute) when discoverable.
/// 3. Default `<workspace_root>/docs/.atomic/workspace.atomic.json`.
///
/// Closes the tc8-harness dogfood gap where the doc-comment claimed
/// `[atomic] sidecar_path` was configurable but no code path parsed it.
pub fn resolve_sidecar(workspace_root: &Path, sidecar: Option<&str>) -> PathBuf {
 if let Some(p) = sidecar {
 let pb = PathBuf::from(p);
 return if pb.is_absolute() { pb } else { workspace_root.join(pb) };
 }
 if let Ok(Some(loaded)) = discover_config(workspace_root) {
 if let Some(cfg_path) = loaded
 .config
 .atomic
 .as_ref()
 .and_then(|a| a.sidecar_path.as_deref())
 {
 let pb = PathBuf::from(cfg_path);
 return if pb.is_absolute() { pb } else { workspace_root.join(pb) };
 }
 }
 AtomicStore::default_sidecar_path(workspace_root)
}

fn handle_result(
 result: Result<AtomicMutateReceipt, AtomicMutateError>,
 json: bool,
) -> Result<()> {
 match result {
 Ok(r) => {
 print_receipt(&r, json);
 Ok(())
 }
 Err(e) => {
 print_error(&e, json);
 Err(anyhow!("{}", e))
 }
 }
}

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
 let stripped = trimmed.strip_prefix("- ").unwrap_or(trimmed);
 // Format: `<alternative> -- <reason>` or `<alternative> — <reason>`.
 let (alt, reason) = stripped
 .split_once(" — ")
 .or_else(|| stripped.split_once(" -- "))
 .ok_or_else(|| {
  anyhow!(
  "alternatives-file:{}: line format violation — `<alternative> -- <reason>` or ` — ` separator required",
  lineno + 1
  )
 })?;
 out.push(RejectedAlternative {
 alternative: alt.trim().to_string(),
 reason: reason.trim().to_string(),
 });
 }
 Ok(out)
}

/// Parse `--section` or `--section 43` → "43".
fn strip_section_prefix(s: &str) -> String {
 s.strip_prefix('§').unwrap_or(s).to_string()
}

// ============================================================================
// CLI subcommand entry points (each takes args slice = post-subcommand args)
// ============================================================================

pub fn cmd_set_section_intent(workspace_root: &Path, args: &[String]) -> Result<()> {
 let mut section: Option<String> = None;
 let mut intent: Option<String> = None;
 let mut sidecar: Option<String> = None;
 let mut json = false;
 let mut regenerate = true;
 let mut iter = args.iter();
 while let Some(arg) = iter.next() {
 match arg.as_str() {
 "--section" => {
  section = Some(iter.next().ok_or_else(|| anyhow!("--section missing"))?.clone())
 }
 "--intent" => {
  intent = Some(iter.next().ok_or_else(|| anyhow!("--intent missing"))?.clone())
 }
 "--sidecar" => {
  sidecar = Some(iter.next().ok_or_else(|| anyhow!("--sidecar missing"))?.clone())
 }
 "--json" => json = true,
 "--no-regenerate" => regenerate = false,
 other => bail!("unknown flag `{}`", other),
 }
 }
 let section = strip_section_prefix(&section.ok_or_else(|| anyhow!("--section arg required"))?);
 let intent = intent.ok_or_else(|| anyhow!("--intent arg required"))?;
 let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref());
 let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
 finalize_mutate(
 workspace_root,
 set_section_intent(&mut store, &sidecar_path, &section, &intent),
 sidecar.as_deref(),
 regenerate,
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
pub fn cmd_add_section(workspace_root: &Path, args: &[String]) -> Result<()> {
 let mut section: Option<String> = None;
 let mut parent_doc: Option<String> = None;
 let mut title: Option<String> = None;
 let mut parent: Option<String> = None;
 let mut sidecar: Option<String> = None;
 let mut json = false;
 let mut regenerate = true;
 let mut iter = args.iter();
 while let Some(arg) = iter.next() {
 match arg.as_str() {
 "--section" => {
  section = Some(iter.next().ok_or_else(|| anyhow!("--section missing"))?.clone())
 }
 "--parent-doc" => {
  parent_doc =
  Some(iter.next().ok_or_else(|| anyhow!("--parent-doc missing"))?.clone())
 }
 "--title" => {
  title = Some(iter.next().ok_or_else(|| anyhow!("--title missing"))?.clone())
 }
 "--parent" => {
  parent = Some(iter.next().ok_or_else(|| anyhow!("--parent missing"))?.clone())
 }
 "--sidecar" => {
  sidecar = Some(iter.next().ok_or_else(|| anyhow!("--sidecar missing"))?.clone())
 }
 "--json" => json = true,
 "--no-regenerate" => regenerate = false,
 other => bail!("unknown flag `{}`", other),
 }
 }
 let section = strip_section_prefix(&section.ok_or_else(|| anyhow!("--section arg required"))?);
 let parent_doc = parent_doc.ok_or_else(|| anyhow!("--parent-doc arg required"))?;
 let title = title.ok_or_else(|| anyhow!("--title arg required"))?;
 let parent_stripped = parent.as_deref().map(strip_section_prefix);
 let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref());
 let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
 finalize_mutate(
 workspace_root,
 mnemosyne_validator::atomic::add_section(
 &mut store,
 &sidecar_path,
 &section,
 &parent_doc,
 &title,
 parent_stripped.as_deref(),
 ),
 sidecar.as_deref(),
 regenerate,
 json,
 )
}

/// Round 287 — outline setter CLI surface. set_section_title sets the
/// heading text on an existing Section (Phase C primitive).
pub fn cmd_set_section_title(workspace_root: &Path, args: &[String]) -> Result<()> {
 let mut section: Option<String> = None;
 let mut title: Option<String> = None;
 let mut sidecar: Option<String> = None;
 let mut json = false;
 let mut regenerate = true;
 let mut iter = args.iter();
 while let Some(arg) = iter.next() {
 match arg.as_str() {
 "--section" => {
  section = Some(iter.next().ok_or_else(|| anyhow!("--section missing"))?.clone())
 }
 "--title" => {
  title = Some(iter.next().ok_or_else(|| anyhow!("--title missing"))?.clone())
 }
 "--sidecar" => {
  sidecar = Some(iter.next().ok_or_else(|| anyhow!("--sidecar missing"))?.clone())
 }
 "--json" => json = true,
 "--no-regenerate" => regenerate = false,
 other => bail!("unknown flag `{}`", other),
 }
 }
 let section = strip_section_prefix(&section.ok_or_else(|| anyhow!("--section arg required"))?);
 let title = title.ok_or_else(|| anyhow!("--title arg required"))?;
 let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref());
 let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
 finalize_mutate(
 workspace_root,
 set_section_title(&mut store, &sidecar_path, &section, &title),
 sidecar.as_deref(),
 regenerate,
 json,
 )
}

/// Round 287 — set Section.parent_doc (doc binding).
pub fn cmd_set_section_parent_doc(workspace_root: &Path, args: &[String]) -> Result<()> {
 let mut section: Option<String> = None;
 let mut parent_doc: Option<String> = None;
 let mut sidecar: Option<String> = None;
 let mut json = false;
 let mut regenerate = true;
 let mut iter = args.iter();
 while let Some(arg) = iter.next() {
 match arg.as_str() {
 "--section" => {
  section = Some(iter.next().ok_or_else(|| anyhow!("--section missing"))?.clone())
 }
 "--parent-doc" => {
  parent_doc =
  Some(iter.next().ok_or_else(|| anyhow!("--parent-doc missing"))?.clone())
 }
 "--sidecar" => {
  sidecar = Some(iter.next().ok_or_else(|| anyhow!("--sidecar missing"))?.clone())
 }
 "--json" => json = true,
 "--no-regenerate" => regenerate = false,
 other => bail!("unknown flag `{}`", other),
 }
 }
 let section = strip_section_prefix(&section.ok_or_else(|| anyhow!("--section arg required"))?);
 let parent_doc = parent_doc.ok_or_else(|| anyhow!("--parent-doc arg required"))?;
 let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref());
 let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
 finalize_mutate(
 workspace_root,
 set_section_parent_doc(&mut store, &sidecar_path, &section, &parent_doc),
 sidecar.as_deref(),
 regenerate,
 json,
 )
}

/// Round 287 — set Section.parent_section (hierarchy binding). Use `--parent
/// <section_id>` to re-parent; use `--no-parent` to promote to top-level.
/// The two flags are mutually exclusive; exactly one is required.
pub fn cmd_set_section_parent_section(workspace_root: &Path, args: &[String]) -> Result<()> {
 let mut section: Option<String> = None;
 let mut parent: Option<String> = None;
 let mut clear_parent = false;
 let mut sidecar: Option<String> = None;
 let mut json = false;
 let mut regenerate = true;
 let mut iter = args.iter();
 while let Some(arg) = iter.next() {
 match arg.as_str() {
 "--section" => {
  section = Some(iter.next().ok_or_else(|| anyhow!("--section missing"))?.clone())
 }
 "--parent" => {
  parent = Some(iter.next().ok_or_else(|| anyhow!("--parent missing"))?.clone())
 }
 "--no-parent" => clear_parent = true,
 "--sidecar" => {
  sidecar = Some(iter.next().ok_or_else(|| anyhow!("--sidecar missing"))?.clone())
 }
 "--json" => json = true,
 "--no-regenerate" => regenerate = false,
 other => bail!("unknown flag `{}`", other),
 }
 }
 let section = strip_section_prefix(&section.ok_or_else(|| anyhow!("--section arg required"))?);
 if parent.is_some() && clear_parent {
 bail!("--parent and --no-parent are mutually exclusive");
 }
 if parent.is_none() && !clear_parent {
 bail!("exactly one of --parent <id> or --no-parent required");
 }
 let parent_stripped = parent.as_deref().map(strip_section_prefix);
 let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref());
 let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
 finalize_mutate(
 workspace_root,
 set_section_parent_section(
 &mut store,
 &sidecar_path,
 &section,
 parent_stripped.as_deref(),
 ),
 sidecar.as_deref(),
 regenerate,
 json,
 )
}

pub fn cmd_set_section_rationale(workspace_root: &Path, args: &[String]) -> Result<()> {
 cmd_set_section_bullets(workspace_root, args, "rationale", |s, p, id, b| {
 set_section_rationale(s, p, id, b)
 })
}

pub fn cmd_set_section_inputs(workspace_root: &Path, args: &[String]) -> Result<()> {
 cmd_set_section_bullets(workspace_root, args, "inputs", |s, p, id, b| {
 set_section_inputs(s, p, id, b)
 })
}

pub fn cmd_set_section_outputs(workspace_root: &Path, args: &[String]) -> Result<()> {
 cmd_set_section_bullets(workspace_root, args, "outputs", |s, p, id, b| {
 set_section_outputs(s, p, id, b)
 })
}

// Round 295 — publishable-half setters for ChangelogEntry. Mutate only
// publishable_*; audit_* is the permanent record and stays untouched.
// `--entry` arg names the changelog entry (must already exist); the audit
// half can only be authored by `append_changelog_entry_v2`.

pub fn cmd_set_changelog_publishable_decision_summary(
 workspace_root: &Path,
 args: &[String],
) -> Result<()> {
 cmd_set_changelog_publishable_string(
 workspace_root,
 args,
 "decision_summary",
 |s, p, id, v| {
 mnemosyne_validator::set_changelog_publishable_decision_summary(s, p, id, v)
 },
 )
}

pub fn cmd_set_changelog_publishable_changes(
 workspace_root: &Path,
 args: &[String],
) -> Result<()> {
 cmd_set_changelog_publishable_bullets(
 workspace_root,
 args,
 "publishable_changes",
 |s, p, id, b| {
 mnemosyne_validator::set_changelog_publishable_changes_bullets(s, p, id, b)
 },
 )
}

pub fn cmd_set_changelog_publishable_verification(
 workspace_root: &Path,
 args: &[String],
) -> Result<()> {
 cmd_set_changelog_publishable_bullets(
 workspace_root,
 args,
 "publishable_verification",
 |s, p, id, b| {
 mnemosyne_validator::set_changelog_publishable_verification_bullets(
 s, p, id, b,
 )
 },
 )
}

pub fn cmd_set_changelog_publishable_impact_refs(
 workspace_root: &Path,
 args: &[String],
) -> Result<()> {
 cmd_set_changelog_publishable_bullets(
 workspace_root,
 args,
 "publishable_impact_refs",
 |s, p, id, b| {
 mnemosyne_validator::set_changelog_publishable_impact_refs(s, p, id, b)
 },
 )
}

pub fn cmd_set_changelog_publishable_carry_forward(
 workspace_root: &Path,
 args: &[String],
) -> Result<()> {
 cmd_set_changelog_publishable_bullets(
 workspace_root,
 args,
 "publishable_carry_forward",
 |s, p, id, b| {
 mnemosyne_validator::set_changelog_publishable_carry_forward_bullets(
 s, p, id, b,
 )
 },
 )
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
) -> Result<()> {
 let mut entry: Option<String> = None;
 let mut value: Option<String> = None;
 let mut sidecar: Option<String> = None;
 let mut json = false;
 let mut regenerate = true;
 let mut iter = args.iter();
 while let Some(arg) = iter.next() {
 match arg.as_str() {
 "--entry" => {
  entry = Some(iter.next().ok_or_else(|| anyhow!("--entry missing"))?.clone())
 }
 "--value" => {
  value = Some(iter.next().ok_or_else(|| anyhow!("--value missing"))?.clone())
 }
 "--sidecar" => {
  sidecar = Some(iter.next().ok_or_else(|| anyhow!("--sidecar missing"))?.clone())
 }
 "--json" => json = true,
 "--no-regenerate" => regenerate = false,
 other => bail!("unknown flag `{}`", other),
 }
 }
 let entry = entry.ok_or_else(|| anyhow!("--entry arg required ({} scope)", field))?;
 let value = value.ok_or_else(|| anyhow!("--value arg required ({} scope)", field))?;
 let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref());
 let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
 finalize_mutate(
 workspace_root,
 primitive(&mut store, &sidecar_path, &entry, &value),
 sidecar.as_deref(),
 regenerate,
 json,
 )
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
) -> Result<()> {
 let mut entry: Option<String> = None;
 let mut bullets_file: Option<String> = None;
 let mut sidecar: Option<String> = None;
 let mut json = false;
 let mut regenerate = true;
 let mut iter = args.iter();
 while let Some(arg) = iter.next() {
 match arg.as_str() {
 "--entry" => {
  entry = Some(iter.next().ok_or_else(|| anyhow!("--entry missing"))?.clone())
 }
 "--bullets-file" => {
  bullets_file = Some(
  iter.next()
  .ok_or_else(|| anyhow!("--bullets-file missing"))?
  .clone(),
  )
 }
 "--sidecar" => {
  sidecar = Some(iter.next().ok_or_else(|| anyhow!("--sidecar missing"))?.clone())
 }
 "--json" => json = true,
 "--no-regenerate" => regenerate = false,
 other => bail!("unknown flag `{}`", other),
 }
 }
 let entry = entry.ok_or_else(|| anyhow!("--entry arg required ({} scope)", field))?;
 let bullets_path =
 bullets_file.ok_or_else(|| anyhow!("--bullets-file arg required ({} scope)", field))?;
 let bullets = parse_bullets_file(&bullets_path)?;
 let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref());
 let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
 finalize_mutate(
 workspace_root,
 primitive(&mut store, &sidecar_path, &entry, &bullets),
 sidecar.as_deref(),
 regenerate,
 json,
 )
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
) -> Result<()> {
 let mut section: Option<String> = None;
 let mut bullets_file: Option<String> = None;
 let mut sidecar: Option<String> = None;
 let mut json = false;
 let mut regenerate = true;
 let mut iter = args.iter();
 while let Some(arg) = iter.next() {
 match arg.as_str() {
 "--section" => {
  section = Some(iter.next().ok_or_else(|| anyhow!("--section missing"))?.clone())
 }
 "--bullets-file" => {
  bullets_file = Some(
  iter.next()
  .ok_or_else(|| anyhow!("--bullets-file missing"))?
  .clone(),
  )
 }
 "--sidecar" => {
  sidecar = Some(iter.next().ok_or_else(|| anyhow!("--sidecar missing"))?.clone())
 }
 "--json" => json = true,
 "--no-regenerate" => regenerate = false,
 other => bail!("unknown flag `{}`", other),
 }
 }
 let section = strip_section_prefix(&section.ok_or_else(|| anyhow!("--section arg required"))?);
 let bullets_path =
 bullets_file.ok_or_else(|| anyhow!("--bullets-file arg required ({} scope)", field))?;
 let bullets = parse_bullets_file(&bullets_path)?;
 let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref());
 let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
 finalize_mutate(
 workspace_root,
 primitive(&mut store, &sidecar_path, &section, &bullets),
 sidecar.as_deref(),
 regenerate,
 json,
 )
}

pub fn cmd_add_section_caveat(workspace_root: &Path, args: &[String]) -> Result<()> {
 let mut section: Option<String> = None;
 let mut bullet: Option<String> = None;
 let mut sidecar: Option<String> = None;
 let mut json = false;
 let mut regenerate = true;
 let mut iter = args.iter();
 while let Some(arg) = iter.next() {
 match arg.as_str() {
 "--section" => {
  section = Some(iter.next().ok_or_else(|| anyhow!("--section missing"))?.clone())
 }
 "--bullet" => {
  bullet = Some(iter.next().ok_or_else(|| anyhow!("--bullet missing"))?.clone())
 }
 "--sidecar" => {
  sidecar = Some(iter.next().ok_or_else(|| anyhow!("--sidecar missing"))?.clone())
 }
 "--json" => json = true,
 "--no-regenerate" => regenerate = false,
 other => bail!("unknown flag `{}`", other),
 }
 }
 let section = strip_section_prefix(&section.ok_or_else(|| anyhow!("--section arg required"))?);
 let bullet = bullet.ok_or_else(|| anyhow!("--bullet arg required"))?;
 let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref());
 let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
 finalize_mutate(
 workspace_root,
 add_section_caveat(&mut store, &sidecar_path, &section, &bullet),
 sidecar.as_deref(),
 regenerate,
 json,
 )
}

pub fn cmd_set_section_alternatives(workspace_root: &Path, args: &[String]) -> Result<()> {
 let mut section: Option<String> = None;
 let mut alternatives_file: Option<String> = None;
 let mut sidecar: Option<String> = None;
 let mut json = false;
 let mut regenerate = true;
 let mut iter = args.iter();
 while let Some(arg) = iter.next() {
 match arg.as_str() {
 "--section" => {
  section = Some(iter.next().ok_or_else(|| anyhow!("--section missing"))?.clone())
 }
 "--alternatives-file" => {
  alternatives_file = Some(
  iter.next()
  .ok_or_else(|| anyhow!("--alternatives-file missing"))?
  .clone(),
  )
 }
 "--sidecar" => {
  sidecar = Some(iter.next().ok_or_else(|| anyhow!("--sidecar missing"))?.clone())
 }
 "--json" => json = true,
 "--no-regenerate" => regenerate = false,
 other => bail!("unknown flag `{}`", other),
 }
 }
 let section = strip_section_prefix(&section.ok_or_else(|| anyhow!("--section arg required"))?);
 let path =
 alternatives_file.ok_or_else(|| anyhow!("--alternatives-file arg required"))?;
 let alts = parse_alternatives_file(&path)?;
 let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref());
 let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
 finalize_mutate(
 workspace_root,
 set_section_alternatives(&mut store, &sidecar_path, &section, &alts),
 sidecar.as_deref(),
 regenerate,
 json,
 )
}

pub fn cmd_set_section_impact_scope(workspace_root: &Path, args: &[String]) -> Result<()> {
 let mut section: Option<String> = None;
 let mut refs_csv: Option<String> = None;
 let mut sidecar: Option<String> = None;
 let mut json = false;
 let mut regenerate = true;
 let mut iter = args.iter();
 while let Some(arg) = iter.next() {
 match arg.as_str() {
 "--section" => {
  section = Some(iter.next().ok_or_else(|| anyhow!("--section missing"))?.clone())
 }
 "--refs" => {
  refs_csv = Some(iter.next().ok_or_else(|| anyhow!("--refs missing"))?.clone())
 }
 "--sidecar" => {
  sidecar = Some(iter.next().ok_or_else(|| anyhow!("--sidecar missing"))?.clone())
 }
 "--json" => json = true,
 "--no-regenerate" => regenerate = false,
 other => bail!("unknown flag `{}`", other),
 }
 }
 let section = strip_section_prefix(&section.ok_or_else(|| anyhow!("--section arg required"))?);
 let refs_csv = refs_csv
 .ok_or_else(|| anyhow!("--refs arg required — e.g. --refs '15,39,41'"))?;
 let refs: Vec<String> = refs_csv
 .split(',')
 .map(|r| strip_section_prefix(r.trim()))
 .filter(|r| !r.is_empty())
 .collect();
 let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref());
 let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
 finalize_mutate(
 workspace_root,
 set_section_impact_scope(&mut store, &sidecar_path, &section, &refs),
 sidecar.as_deref(),
 regenerate,
 json,
 )
}

pub fn cmd_add_section_example(workspace_root: &Path, args: &[String]) -> Result<()> {
 let mut section: Option<String> = None;
 let mut language: Option<String> = None;
 let mut code_file: Option<String> = None;
 let mut sidecar: Option<String> = None;
 let mut json = false;
 let mut regenerate = true;
 let mut iter = args.iter();
 while let Some(arg) = iter.next() {
 match arg.as_str() {
 "--section" => {
  section = Some(iter.next().ok_or_else(|| anyhow!("--section missing"))?.clone())
 }
 "--language" => {
  language =
  Some(iter.next().ok_or_else(|| anyhow!("--language missing"))?.clone())
 }
 "--code-file" => {
  code_file = Some(
  iter.next()
  .ok_or_else(|| anyhow!("--code-file missing"))?
  .clone(),
  )
 }
 "--sidecar" => {
  sidecar = Some(iter.next().ok_or_else(|| anyhow!("--sidecar missing"))?.clone())
 }
 "--json" => json = true,
 "--no-regenerate" => regenerate = false,
 other => bail!("unknown flag `{}`", other),
 }
 }
 let section = strip_section_prefix(&section.ok_or_else(|| anyhow!("--section arg required"))?);
 let language = language.unwrap_or_default();
 let code_file = code_file.ok_or_else(|| anyhow!("--code-file arg required"))?;
 let code = fs::read_to_string(&code_file)
 .with_context(|| format!("code-file recovery failed: {}", code_file))?;
 let example = ExampleBlock { language, code };
 let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref());
 let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
 finalize_mutate(
 workspace_root,
 add_section_example(&mut store, &sidecar_path, &section, example),
 sidecar.as_deref(),
 regenerate,
 json,
 )
}

/// Path B (Spec ↔ Code bidirectional binding) substrate.
///
/// Append a `(file, symbol?)` binding entry to `Section.implementations`.
/// File path is workspace-relative POSIX shape; symbol is opaque (no
/// language grammar regex). Set semantics: duplicate `(file, symbol)`
/// rejected at write time.
///
/// Validator extension and section seeding are deferred to +.
pub fn cmd_add_section_implementation(workspace_root: &Path, args: &[String]) -> Result<()> {
 let mut section: Option<String> = None;
 let mut file: Option<String> = None;
 let mut symbol: Option<String> = None;
 let mut sidecar: Option<String> = None;
 let mut json = false;
 let mut regenerate = true;
 let mut iter = args.iter();
 while let Some(arg) = iter.next() {
 match arg.as_str() {
 "--section" => {
  section = Some(iter.next().ok_or_else(|| anyhow!("--section missing"))?.clone())
 }
 "--file" => {
  file = Some(iter.next().ok_or_else(|| anyhow!("--file missing"))?.clone())
 }
 "--symbol" => {
  symbol = Some(iter.next().ok_or_else(|| anyhow!("--symbol missing"))?.clone())
 }
 "--sidecar" => {
  sidecar = Some(iter.next().ok_or_else(|| anyhow!("--sidecar missing"))?.clone())
 }
 "--json" => json = true,
 "--no-regenerate" => regenerate = false,
 other => bail!("unknown flag `{}`", other),
 }
 }
 let section = strip_section_prefix(&section.ok_or_else(|| anyhow!("--section arg required"))?);
 let file = file.ok_or_else(|| anyhow!("--file arg required (workspace-relative POSIX path)"))?;
 let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref());
 let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
 finalize_mutate(
 workspace_root,
 add_section_implementation(&mut store, &sidecar_path, &section, &file, symbol.as_deref()),
 sidecar.as_deref(),
 regenerate,
 json,
 )
}

/// Round 283 — `remove-section-implementation` CLI surface.
///
/// `--section §<id> --file <path> [--symbol <name>] --reason <text> [--sidecar <path>] [--json]`
///
/// Removes one `(file, symbol?)` binding from `Section.implementations`.
/// Errors with NotFound when the section or the specific binding is
/// absent (exact set-element match; pass `--symbol` to target a
/// symbol-narrowed row, omit it for a file-only row). `--reason`
/// mandatory — recorded on the mutate receipt for audit symmetry with
/// `remove-section` (R267) / `remove-inventory-entry` (R274).
pub fn cmd_remove_section_implementation(
 workspace_root: &Path,
 args: &[String],
) -> Result<()> {
 let mut section: Option<String> = None;
 let mut file: Option<String> = None;
 let mut symbol: Option<String> = None;
 let mut reason: Option<String> = None;
 let mut sidecar: Option<String> = None;
 let mut json = false;
 let mut regenerate = true;
 let mut iter = args.iter();
 while let Some(arg) = iter.next() {
 match arg.as_str() {
 "--section" => {
  section = Some(iter.next().ok_or_else(|| anyhow!("--section missing"))?.clone())
 }
 "--file" => {
  file = Some(iter.next().ok_or_else(|| anyhow!("--file missing"))?.clone())
 }
 "--symbol" => {
  symbol = Some(iter.next().ok_or_else(|| anyhow!("--symbol missing"))?.clone())
 }
 "--reason" => {
  reason = Some(iter.next().ok_or_else(|| anyhow!("--reason missing"))?.clone())
 }
 "--sidecar" => {
  sidecar = Some(iter.next().ok_or_else(|| anyhow!("--sidecar missing"))?.clone())
 }
 "--json" => json = true,
 "--no-regenerate" => regenerate = false,
 other => bail!("unknown flag `{}`", other),
 }
 }
 let section = strip_section_prefix(&section.ok_or_else(|| anyhow!("--section arg required"))?);
 let file = file.ok_or_else(|| anyhow!("--file arg required (workspace-relative POSIX path)"))?;
 let reason = reason.ok_or_else(|| anyhow!("--reason arg required (audit safeguard)"))?;
 let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref());
 let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
 finalize_mutate(
 workspace_root,
 remove_section_implementation(
 &mut store,
 &sidecar_path,
 &section,
 &file,
 symbol.as_deref(),
 &reason,
 ),
 sidecar.as_deref(),
 regenerate,
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
pub fn cmd_remove_section(workspace_root: &Path, args: &[String]) -> Result<()> {
 let mut section: Option<String> = None;
 let mut reason: Option<String> = None;
 let mut sidecar: Option<String> = None;
 let mut json = false;
 let mut regenerate = true;
 let mut iter = args.iter();
 while let Some(arg) = iter.next() {
 match arg.as_str() {
 "--section" => {
  section = Some(iter.next().ok_or_else(|| anyhow!("--section missing"))?.clone())
 }
 "--reason" => {
  reason = Some(iter.next().ok_or_else(|| anyhow!("--reason missing"))?.clone())
 }
 "--sidecar" => {
  sidecar = Some(iter.next().ok_or_else(|| anyhow!("--sidecar missing"))?.clone())
 }
 "--json" => json = true,
 "--no-regenerate" => regenerate = false,
 other => bail!("unknown flag `{}`", other),
 }
 }
 let section = strip_section_prefix(&section.ok_or_else(|| anyhow!("--section arg required"))?);
 let reason = reason.ok_or_else(|| anyhow!("--reason arg required (audit safeguard)"))?;
 let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref());
 let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
 finalize_mutate(
 workspace_root,
 remove_section(&mut store, &sidecar_path, &section, &reason),
 sidecar.as_deref(),
 regenerate,
 json,
 )
}

/// Round 265 — atomic decision_status setter CLI surface.
///
/// `--section §<id> --status active|superseded|removed [--sidecar <path>] [--json]`
///
/// Sets `AtomicSection.decision_status` on the atomic store. Stage B
/// freshness substrate — once the atomic store carries non-Active status,
/// downstream tooling (auto-cascade trigger, decay scan) becomes wireable.
pub fn cmd_set_section_decision_status_atomic(
 workspace_root: &Path,
 args: &[String],
) -> Result<()> {
 let mut section: Option<String> = None;
 let mut status_str: Option<String> = None;
 let mut superseding: Option<String> = None;
 let mut sidecar: Option<String> = None;
 let mut json = false;
 let mut regenerate = true;
 let mut iter = args.iter();
 while let Some(arg) = iter.next() {
 match arg.as_str() {
 "--section" => {
  section = Some(iter.next().ok_or_else(|| anyhow!("--section missing"))?.clone())
 }
 "--status" => {
  status_str = Some(iter.next().ok_or_else(|| anyhow!("--status missing"))?.clone())
 }
 "--superseding" => {
  superseding =
  Some(iter.next().ok_or_else(|| anyhow!("--superseding missing"))?.clone())
 }
 "--sidecar" => {
  sidecar = Some(iter.next().ok_or_else(|| anyhow!("--sidecar missing"))?.clone())
 }
 "--json" => json = true,
 "--no-regenerate" => regenerate = false,
 other => bail!("unknown flag `{}`", other),
 }
 }
 let section = strip_section_prefix(&section.ok_or_else(|| anyhow!("--section arg required"))?);
 let status_raw = status_str
 .ok_or_else(|| anyhow!("--status arg required (active|superseded|removed)"))?;
 let new_status = match status_raw.to_ascii_lowercase().as_str() {
 "active" => DecisionStatus::Active,
 "superseded" => DecisionStatus::Superseded,
 "removed" => DecisionStatus::Removed,
 other => bail!(
 "--status `{}` invalid (expected active|superseded|removed)",
 other
 ),
 };
 // T1 rule 4 (atomic axis): --superseding is mandatory for `--status
 // superseded` and rejected for active|removed (forward-pointer is only
 // meaningful when the section asserts replacement). Symmetric with the
 // markdown-axis CLI at `cmd_set_section_decision_status`.
 if new_status != DecisionStatus::Superseded && superseding.is_some() {
 bail!(
 "--superseding is only valid with `--status superseded` (got `--status {}`)",
 status_raw
 );
 }
 let superseding_strip = superseding
 .as_deref()
 .map(|s| strip_section_prefix(s));
 let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref());
 let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
 let mutate_result = set_section_decision_status_atomic(
 &mut store,
 &sidecar_path,
 &section,
 new_status,
 superseding_strip.as_deref(),
 );

 // Round 266 — auto-cascade trigger (Stage B freshness). When the new
 // status is Superseded or Removed, run a targeted §<id> decay scan
 // against [code_refs].paths and surface citing locations to stderr.
 // Informational only — never alters the mutate's success/failure.
 // No-op when [code_refs] is unconfigured (5-min setup promise carry).
 if mutate_result.is_ok()
 && matches!(
 new_status,
 DecisionStatus::Superseded | DecisionStatus::Removed
 )
 {
 print_section_decay_trigger(workspace_root, &section, new_status);
 }

 finalize_mutate(
 workspace_root,
 mutate_result,
 sidecar.as_deref(),
 regenerate,
 json,
 )
}

/// Round 266 — mutate-time auto-cascade trigger.
///
/// Runs a §<section_id> decay scan over `[code_refs].paths` and prints a
/// short report to stderr. Silent no-op when `[code_refs]` is unconfigured.
/// Errors during config load or scan are logged but never propagated — the
/// mutate's success boundary stays clean.
fn print_section_decay_trigger(workspace_root: &Path, section_id: &str, new_status: DecisionStatus) {
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
 let code_refs_cfg = match loaded.config.code_refs.as_ref() {
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
 let status_label = match new_status {
 DecisionStatus::Active => "active",
 DecisionStatus::Superseded => "superseded",
 DecisionStatus::Removed => "removed",
 };
 eprintln!(
 "[cascade] §{} → {} — {} citing location(s) in [code_refs].paths",
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
/// targeted decay scan for `inventory_id` over `[code_refs].paths` and
/// prints a short stderr report. Silent no-op when `[code_refs]` is
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
 let code_refs_cfg = match loaded.config.code_refs.as_ref() {
 Some(c) if !c.paths.is_empty() && !c.inventory_prefixes.is_empty() => c,
 _ => return,
 };
 let hits = match scan_inventory_decay(
 workspace_root,
 &code_refs_cfg.paths,
 inventory_id,
 &code_refs_cfg.inventory_prefixes,
 code_refs_cfg.comment_only,
 ) {
 Ok(h) => h,
 Err(e) => {
 eprintln!("[cascade] inventory-decay-trigger scan io error: {}", e);
 return;
 }
 };
 eprintln!(
 "[cascade] {} → {} — {} citing location(s) in [code_refs].paths",
 inventory_id,
 transition_label,
 hits.len()
 );
 for c in &hits {
 eprintln!(" {}:{} {}", c.file.display(), c.line, c.entry_id);
 }
}

/// Render the atomic store at `sidecar_path` to a deterministic markdown
/// string. Side-effect free — this is the read-only render path used by
/// both `generate-docs` (writes the bytes) and `verify-generated` (compares
/// the bytes).
///
/// extracted from the original cmd_generate_docs body so the
/// cascade auto-update wire (atomic mutate → regenerate, pre-commit sync
/// check) can share the single render path. `Source:` line uses a path
/// relative to `workspace_root` so the output is portable across checkouts.
fn render_atomic_store_to_md(
 workspace_root: &Path,
 sidecar_path: &Path,
) -> Result<(String, AtomicStore)> {
 let store = AtomicStore::load(sidecar_path).map_err(|e| anyhow!("{}", e))?;

 let mut out = String::new();
 out.push_str("# GENERATED.md — atomic store derived view\n\n");
 out.push_str(
 "this file `mnemosyne-cli generate-docs` output — direct no edit. \
  atomic store (`docs/.atomic/workspace.atomic.json`) in mutate \
  primitive (`set-section-*` / `append-changelog-entry-v2`) pass and then \
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
 let title = if atomic.title.is_empty() {
  section_id.as_str()
 } else {
  atomic.title.as_str()
 };
 let status = match atomic.decision_status.unwrap_or(DecisionStatus::Active) {
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
fn write_generated_md(output_path: &Path, content: &str) -> Result<()> {
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

/// Resolve cascade output path with the Round 279 precedence chain:
/// 1. Explicit `--output` CLI flag wins absolutely.
/// 2. `[atomic] output_path` from `mnemosyne.toml`, if set.
/// 3. Built-in default `<workspace_root>/docs/GENERATED.md`.
///
/// Closes the tc8-harness silent-drift dogfood gap by exposing an explicit
/// `[atomic] output_path` knob. `[workspace] docs[0]` is *not* consulted —
/// docs[0] is the parse target (markdown the validator reads), while this
/// is the cascade write target (atomic store → md). Keeping them
/// independent prevents a first mutate from clobbering hand-authored
/// content in docs[0].
pub fn resolve_output(workspace_root: &Path, output: Option<&str>) -> PathBuf {
 if let Some(p) = output {
 let pb = PathBuf::from(p);
 return if pb.is_absolute() { pb } else { workspace_root.join(pb) };
 }
 if let Ok(Some(loaded)) = discover_config(workspace_root) {
 if let Some(cfg_path) = loaded
 .config
 .atomic
 .as_ref()
 .and_then(|a| a.output_path.as_deref())
 {
 let pb = PathBuf::from(cfg_path);
 return if pb.is_absolute() { pb } else { workspace_root.join(pb) };
 }
 }
 workspace_root.join("docs/GENERATED.md")
}

/// `generate-docs` subcommand — render atomic store → GENERATED.md.
///
/// forward-wire: from this round, the atomic store is the primary changelog
/// ledger scope (legacy DESIGN.md Changelog stays frozen — consistency).
/// Output path = `<workspace_root>/docs/GENERATED.md` (default, configurable
/// via `--output <path>`).
pub fn cmd_generate_docs(workspace_root: &Path, args: &[String]) -> Result<()> {
 let mut sidecar: Option<String> = None;
 let mut output: Option<String> = None;
 let mut iter = args.iter();
 while let Some(arg) = iter.next() {
 match arg.as_str() {
 "--sidecar" => {
  sidecar = Some(iter.next().ok_or_else(|| anyhow!("--sidecar missing"))?.clone())
 }
 "--output" => {
  output = Some(iter.next().ok_or_else(|| anyhow!("--output missing"))?.clone())
 }
 other => bail!("unknown flag `{}`", other),
 }
 }
 let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref());
 let output_path = resolve_output(workspace_root, output.as_deref());

 let (content, store) = render_atomic_store_to_md(workspace_root, &sidecar_path)?;
 write_generated_md(&output_path, &content)?;

 println!("=== mnemosyne-cli generate-docs ===");
 println!("sidecar: {}", sidecar_path.display());
 println!("output: {}", output_path.display());
 println!("sections rendered: {}", store.sections.len());
 println!("changelog entries rendered: {}", store.changelog_entries.len());
 println!("written_bytes: {}", content.len());
 Ok(())
}

/// `verify-generated` subcommand — verify GENERATED.md matches what
/// generate-docs would produce from the current sidecar (read-only).
///
/// pre-commit hook entry point. Exit 0 = sync, exit 1 = stale.
/// Caller (script / CI) inspects the exit code; stderr prints a one-line
/// hint if stale.
pub fn cmd_verify_generated(workspace_root: &Path, args: &[String]) -> Result<()> {
 let mut sidecar: Option<String> = None;
 let mut output: Option<String> = None;
 let mut iter = args.iter();
 while let Some(arg) = iter.next() {
 match arg.as_str() {
 "--sidecar" => {
  sidecar = Some(iter.next().ok_or_else(|| anyhow!("--sidecar missing"))?.clone())
 }
 "--output" => {
  output = Some(iter.next().ok_or_else(|| anyhow!("--output missing"))?.clone())
 }
 other => bail!("unknown flag `{}`", other),
 }
 }
 let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref());
 let output_path = resolve_output(workspace_root, output.as_deref());

 let (expected, _) = render_atomic_store_to_md(workspace_root, &sidecar_path)?;
 let actual = fs::read_to_string(&output_path)
 .with_context(|| format!("GENERATED.md recovery failed: {}", output_path.display()))?;

 if expected == actual {
 println!("=== mnemosyne-cli verify-generated ===");
 println!("sidecar: {}", sidecar_path.display());
 println!("output: {}", output_path.display());
 println!("status: OK (sync)");
 Ok(())
 } else {
 eprintln!("=== mnemosyne-cli verify-generated ===");
 eprintln!("sidecar: {}", sidecar_path.display());
 eprintln!("output: {}", output_path.display());
 eprintln!("status: STALE — GENERATED.md does not match atomic sidecar");
 eprintln!(
 "hint: run `mnemosyne-cli generate-docs` then stage the updated GENERATED.md"
 );
 bail!("verify-generated: GENERATED.md is stale (Round 168 cascade auto-update gate)")
 }
}

/// Atomic-first validation summary — shape consumed by validate-workspace
///.
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

/// Validate the atomic store against the supplied workspace section id
/// set. Pure read — no file writes, side effect free. Used by
/// validate-workspace and audit ledgers to share a
/// single audit definition.
pub fn validate_atomic_store(
 workspace_root: &Path,
 section_id_set: &std::collections::BTreeSet<String>,
) -> Result<AtomicValidationSummary> {
 // Round 280 — honor `[atomic].sidecar_path` config so the read /
 // validation path sees the same store the mutate path wrote to.
 // Previously the default path was hardcoded, which created a split-
 // brain when an external workspace redirected the sidecar.
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
 // Empty store + missing GENERATED.md = trivially in sync (nothing to derive).
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

/// Auto-regenerate GENERATED.md after a successful atomic mutate. Default
/// behavior of every atomic mutate CLI subcommand (overridable via
/// `--no-regenerate`). Errors are propagated — a regenerate failure after
/// a successful mutate aborts the command, signalling that the cascade is
/// in an inconsistent state and needs manual intervention.
fn auto_regenerate(workspace_root: &Path, sidecar: Option<&str>) -> Result<()> {
 let sidecar_path = resolve_sidecar(workspace_root, sidecar);
 let output_path = resolve_output(workspace_root, None);
 let (content, _) = render_atomic_store_to_md(workspace_root, &sidecar_path)?;
 write_generated_md(&output_path, &content)?;
 Ok(())
}

/// Wrap a mutate primitive call: print the receipt (or error), then auto-
/// regenerate GENERATED.md if `regenerate` is true. Each atomic mutate
/// CLI subcommand routes through this finalizer to keep the cascade
/// auto-update behavior single-sourced.
fn finalize_mutate(
 workspace_root: &Path,
 result: Result<AtomicMutateReceipt, AtomicMutateError>,
 sidecar: Option<&str>,
 regenerate: bool,
 json: bool,
) -> Result<()> {
 handle_result(result, json)?;
 if regenerate {
 auto_regenerate(workspace_root, sidecar)?;
 }
 Ok(())
}

pub fn cmd_append_changelog_entry_v2(workspace_root: &Path, args: &[String]) -> Result<()> {
 let mut entry_id: Option<String> = None;
 let mut decision_summary: Option<String> = None;
 let mut changes_file: Option<String> = None;
 let mut verification_file: Option<String> = None;
 let mut impact_csv: Option<String> = None;
 let mut carry_file: Option<String> = None;
 let mut sidecar: Option<String> = None;
 let mut json = false;
 let mut regenerate = true;
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
  sidecar = Some(iter.next().ok_or_else(|| anyhow!("--sidecar missing"))?.clone())
 }
 "--json" => json = true,
 "--no-regenerate" => regenerate = false,
 other => bail!("unknown flag `{}`", other),
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
 let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref());
 let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
 finalize_mutate(
 workspace_root,
 append_changelog_entry_v2(
 &mut store,
 &sidecar_path,
 &entry_id,
 decision_summary.as_deref(),
 &changes,
 &verification,
 &impact_refs,
 &carry_forward,
 ),
 sidecar.as_deref(),
 regenerate,
 json,
 )
}

// ============================================================================
// Inventory mutate CLI handlers (Round 274, Phase 1A).
// ============================================================================

fn parse_inventory_status(raw: &str) -> Result<InventoryStatus> {
 match raw.to_ascii_lowercase().as_str() {
 "active" => Ok(InventoryStatus::Active),
 "deprecated" => Ok(InventoryStatus::Deprecated),
 "reserved" => Ok(InventoryStatus::Reserved),
 other => bail!(
 "--status `{}` invalid (expected active|deprecated|reserved)",
 other
 ),
 }
}

/// `add-inventory-entry --id <ID> --status active|deprecated|reserved \
///   [--section §<N>] [--source <text>] [--reason <text>] \
///   [--sidecar <path>] [--json]`
pub fn cmd_add_inventory_entry(workspace_root: &Path, args: &[String]) -> Result<()> {
 let mut inventory_id: Option<String> = None;
 let mut status_str: Option<String> = None;
 let mut section_ref: Option<String> = None;
 let mut source: Option<String> = None;
 let mut reason: Option<String> = None;
 let mut sidecar: Option<String> = None;
 let mut json = false;
 let mut regenerate = true;
 let mut iter = args.iter();
 while let Some(arg) = iter.next() {
 match arg.as_str() {
 "--id" => {
  inventory_id = Some(iter.next().ok_or_else(|| anyhow!("--id missing"))?.clone())
 }
 "--status" => {
  status_str = Some(iter.next().ok_or_else(|| anyhow!("--status missing"))?.clone())
 }
 "--section" => {
  section_ref = Some(iter.next().ok_or_else(|| anyhow!("--section missing"))?.clone())
 }
 "--source" => {
  source = Some(iter.next().ok_or_else(|| anyhow!("--source missing"))?.clone())
 }
 "--reason" => {
  reason = Some(iter.next().ok_or_else(|| anyhow!("--reason missing"))?.clone())
 }
 "--sidecar" => {
  sidecar = Some(iter.next().ok_or_else(|| anyhow!("--sidecar missing"))?.clone())
 }
 "--json" => json = true,
 "--no-regenerate" => regenerate = false,
 other => bail!("unknown flag `{}`", other),
 }
 }
 let inventory_id = inventory_id.ok_or_else(|| anyhow!("--id arg required"))?;
 let status = parse_inventory_status(
 status_str
 .as_deref()
 .ok_or_else(|| anyhow!("--status arg required (active|deprecated|reserved)"))?,
 )?;
 let section_ref_clean = section_ref.as_deref().map(strip_section_prefix);
 let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref());
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

 finalize_mutate(
 workspace_root,
 mutate_result,
 sidecar.as_deref(),
 regenerate,
 json,
 )
}

/// `set-inventory-status --id <ID> --status active|deprecated|reserved \
///   [--reason <text>] [--sidecar <path>] [--json]`
///
/// `--reason` semantics: omitted = preserve existing; supplied = overwrite
/// (empty string clears).
pub fn cmd_set_inventory_status(workspace_root: &Path, args: &[String]) -> Result<()> {
 let mut inventory_id: Option<String> = None;
 let mut status_str: Option<String> = None;
 let mut reason: Option<String> = None;
 let mut sidecar: Option<String> = None;
 let mut json = false;
 let mut regenerate = true;
 let mut iter = args.iter();
 while let Some(arg) = iter.next() {
 match arg.as_str() {
 "--id" => {
  inventory_id = Some(iter.next().ok_or_else(|| anyhow!("--id missing"))?.clone())
 }
 "--status" => {
  status_str = Some(iter.next().ok_or_else(|| anyhow!("--status missing"))?.clone())
 }
 "--reason" => {
  reason = Some(iter.next().ok_or_else(|| anyhow!("--reason missing"))?.clone())
 }
 "--sidecar" => {
  sidecar = Some(iter.next().ok_or_else(|| anyhow!("--sidecar missing"))?.clone())
 }
 "--json" => json = true,
 "--no-regenerate" => regenerate = false,
 other => bail!("unknown flag `{}`", other),
 }
 }
 let inventory_id = inventory_id.ok_or_else(|| anyhow!("--id arg required"))?;
 let status = parse_inventory_status(
 status_str
 .as_deref()
 .ok_or_else(|| anyhow!("--status arg required (active|deprecated|reserved)"))?,
 )?;
 let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref());
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

 finalize_mutate(
 workspace_root,
 mutate_result,
 sidecar.as_deref(),
 regenerate,
 json,
 )
}

/// `set-inventory-section-ref --id <ID> (--section §<N> | --clear) \
///   [--sidecar <path>] [--json]`
///
/// Exactly one of `--section` or `--clear` is required.
pub fn cmd_set_inventory_section_ref(workspace_root: &Path, args: &[String]) -> Result<()> {
 let mut inventory_id: Option<String> = None;
 let mut section_ref: Option<String> = None;
 let mut clear = false;
 let mut sidecar: Option<String> = None;
 let mut json = false;
 let mut regenerate = true;
 let mut iter = args.iter();
 while let Some(arg) = iter.next() {
 match arg.as_str() {
 "--id" => {
  inventory_id = Some(iter.next().ok_or_else(|| anyhow!("--id missing"))?.clone())
 }
 "--section" => {
  section_ref = Some(iter.next().ok_or_else(|| anyhow!("--section missing"))?.clone())
 }
 "--clear" => clear = true,
 "--sidecar" => {
  sidecar = Some(iter.next().ok_or_else(|| anyhow!("--sidecar missing"))?.clone())
 }
 "--json" => json = true,
 "--no-regenerate" => regenerate = false,
 other => bail!("unknown flag `{}`", other),
 }
 }
 let inventory_id = inventory_id.ok_or_else(|| anyhow!("--id arg required"))?;
 if section_ref.is_some() == clear {
 bail!("exactly one of --section or --clear must be supplied");
 }
 let cleaned: Option<String> = section_ref.as_deref().map(strip_section_prefix);
 let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref());
 let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
 finalize_mutate(
 workspace_root,
 set_inventory_section_ref(&mut store, &sidecar_path, &inventory_id, cleaned.as_deref()),
 sidecar.as_deref(),
 regenerate,
 json,
 )
}

/// `remove-inventory-entry --id <ID> --reason <text> [--sidecar <path>] [--json]`
pub fn cmd_remove_inventory_entry(workspace_root: &Path, args: &[String]) -> Result<()> {
 let mut inventory_id: Option<String> = None;
 let mut reason: Option<String> = None;
 let mut sidecar: Option<String> = None;
 let mut json = false;
 let mut regenerate = true;
 let mut iter = args.iter();
 while let Some(arg) = iter.next() {
 match arg.as_str() {
 "--id" => {
  inventory_id = Some(iter.next().ok_or_else(|| anyhow!("--id missing"))?.clone())
 }
 "--reason" => {
  reason = Some(iter.next().ok_or_else(|| anyhow!("--reason missing"))?.clone())
 }
 "--sidecar" => {
  sidecar = Some(iter.next().ok_or_else(|| anyhow!("--sidecar missing"))?.clone())
 }
 "--json" => json = true,
 "--no-regenerate" => regenerate = false,
 other => bail!("unknown flag `{}`", other),
 }
 }
 let inventory_id = inventory_id.ok_or_else(|| anyhow!("--id arg required"))?;
 let reason = reason.ok_or_else(|| anyhow!("--reason arg required (audit safeguard)"))?;
 let sidecar_path = resolve_sidecar(workspace_root, sidecar.as_deref());
 let mut store = AtomicStore::load(&sidecar_path).map_err(|e| anyhow!("{}", e))?;
 let mutate_result = remove_inventory_entry(&mut store, &sidecar_path, &inventory_id, &reason);

 // Round 276 — cascade trigger on every successful remove. The entry
 // ceasing to exist promotes any extant cite to InventoryMissing
 // on the next validate-code-refs run; the cascade surfaces those
 // cites mutate-time so the author can act before pre-commit gates.
 if mutate_result.is_ok() {
 print_inventory_decay_trigger(workspace_root, &inventory_id, "removed");
 }

 finalize_mutate(
 workspace_root,
 mutate_result,
 sidecar.as_deref(),
 regenerate,
 json,
 )
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
docs = ["docs/GENERATED.md"]
default_doc = "docs/GENERATED.md"

[atomic]
sidecar_path = "from-config.json"
"#,
 );
 let resolved = resolve_sidecar(tmp.path(), Some("from-cli.json"));
 assert_eq!(resolved, tmp.path().join("from-cli.json"));
 }

 #[test]
 fn resolve_sidecar_config_used_when_cli_omitted() {
 let tmp = TempDir::new().unwrap();
 write_toml(
 tmp.path(),
 r#"
[workspace]
docs = ["docs/GENERATED.md"]
default_doc = "docs/GENERATED.md"

[atomic]
sidecar_path = "altdir/custom.atomic.json"
"#,
 );
 let resolved = resolve_sidecar(tmp.path(), None);
 assert_eq!(resolved, tmp.path().join("altdir/custom.atomic.json"));
 }

 #[test]
 fn resolve_sidecar_built_in_default_without_config() {
 let tmp = TempDir::new().unwrap();
 let resolved = resolve_sidecar(tmp.path(), None);
 assert_eq!(
 resolved,
 tmp.path().join("docs/.atomic/workspace.atomic.json")
 );
 }

 #[test]
 fn resolve_sidecar_absolute_path_passthrough() {
 let tmp = TempDir::new().unwrap();
 let abs = tmp.path().join("absolute/here.json");
 let resolved = resolve_sidecar(tmp.path(), Some(abs.to_str().unwrap()));
 assert_eq!(resolved, abs);
 }

 // Round 279 Bug #3 — cascade output_path resolution chain.

 #[test]
 fn resolve_output_explicit_cli_flag_wins() {
 let tmp = TempDir::new().unwrap();
 write_toml(
 tmp.path(),
 r#"
[workspace]
docs = ["docs/coverage/X.md"]
default_doc = "docs/coverage/X.md"

[atomic]
output_path = "ignored-by-cli.md"
"#,
 );
 let resolved = resolve_output(tmp.path(), Some("manual/output.md"));
 assert_eq!(resolved, tmp.path().join("manual/output.md"));
 }

 #[test]
 fn resolve_output_atomic_output_path_used_when_cli_omitted() {
 let tmp = TempDir::new().unwrap();
 write_toml(
 tmp.path(),
 r#"
[workspace]
docs = ["docs/coverage/SPEC_COVERAGE.md"]
default_doc = "docs/coverage/SPEC_COVERAGE.md"

[atomic]
output_path = "docs/coverage/SPEC_COVERAGE.md"
"#,
 );
 let resolved = resolve_output(tmp.path(), None);
 assert_eq!(resolved, tmp.path().join("docs/coverage/SPEC_COVERAGE.md"));
 }

 #[test]
 fn resolve_output_ignores_workspace_docs_first() {
 // Round 279 design — [workspace] docs[0] is the parse target, NOT
 // the cascade write target. Setting docs[0] without [atomic]
 // output_path must NOT redirect cascade output (would clobber
 // hand-authored content).
 let tmp = TempDir::new().unwrap();
 write_toml(
 tmp.path(),
 r#"
[workspace]
docs = ["docs/HAND_AUTHORED.md"]
default_doc = "docs/HAND_AUTHORED.md"
"#,
 );
 let resolved = resolve_output(tmp.path(), None);
 assert_eq!(resolved, tmp.path().join("docs/GENERATED.md"));
 }

 #[test]
 fn resolve_output_built_in_default_without_config() {
 let tmp = TempDir::new().unwrap();
 let resolved = resolve_output(tmp.path(), None);
 assert_eq!(resolved, tmp.path().join("docs/GENERATED.md"));
 }
}
