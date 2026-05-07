//! Atomic mutate CLI subcommands — DESIGN §15 spec mutate API atomic scope
//! (Round 161 §15 reframe ratify, Round 162 production wire).
//!
//! 9 subcommands cover the 8 atomic Section primitives + 1 atomic ChangelogEntry primitive:
//! - `set-section-intent` — set Section.intent (1-3 sentence summary)
//! - `set-section-rationale` — set Section.rationale_bullets (list)
//! - `set-section-inputs` — set Section.inputs_bullets
//! - `set-section-outputs` — set Section.outputs_bullets
//! - `add-section-caveat` — append to Section.caveats_bullets
//! - `set-section-alternatives` — set Section.alternatives_rejected
//! - `set-section-impact-scope` — set Section.impact_scope (cross-ref list)
//! - `add-section-example` — append to Section.examples (code block)
//! - `append-changelog-entry-v2` — atomic-aware changelog append
//! (decision_summary + changes + verification + impact + carry_forward)
//!
//! Each subcommand:
//! 1. Loads `AtomicStore` from sidecar JSON (default `docs/.atomic/
//! workspace.atomic.json`, configurable via `--sidecar <path>`).
//! 2. Invokes the relevant mutate primitive (T3 threshold validation).
//! 3. Persists the store atomically (temp + rename, Round 124 pattern).
//! 4. Prints `AtomicMutateReceipt` (text or `--json`).
//!
//! permission boundary: production crate atomic scope only — DESIGN.md / ROADMAP.md
//! / 6-doc scope — 0 mutations. Round 19 frozen ledger consistency (legacy body /
//! sub_bullets field preserved).

use anyhow::{anyhow, bail, Context, Result};
use mnemosyne_validator::{
 add_section_caveat, add_section_example, append_changelog_entry_v2,
 render_changelog_entry, set_section_alternatives, set_section_impact_scope,
 set_section_inputs, set_section_intent, set_section_outputs,
 set_section_rationale, AtomicMutateError, AtomicMutateReceipt, AtomicStore,
 ExampleBlock, RejectedAlternative,
};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Resolve sidecar path: explicit `--sidecar` flag wins, else default
/// `<workspace_root>/docs/.atomic/workspace.atomic.json`.
fn resolve_sidecar(workspace_root: &Path, sidecar: Option<&str>) -> PathBuf {
 match sidecar {
 Some(p) => {
 let pb = PathBuf::from(p);
 if pb.is_absolute() {
  pb
 } else {
  workspace_root.join(pb)
 }
 }
 None => AtomicStore::default_sidecar_path(workspace_root),
 }
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

/// Parse `--section §43` or `--section 43` → "43".
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

/// Render the atomic store at `sidecar_path` to a deterministic markdown
/// string. Side-effect free — this is the read-only render path used by
/// both `generate-docs` (writes the bytes) and `verify-generated` (compares
/// the bytes).
///
/// Round 168 — extracted from the original cmd_generate_docs body so the
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

 // Sections — Round 164+ migration scope. Round 168 wire: render path is
 // present, but title / decision_status come from the parsed default_doc
 // workspace (cross-ref shift wire). Section atomic decomposition
 // migration (Round 164+) populates this map; until then the loop is
 // empty and emits nothing.
 if !store.sections.is_empty() {
 out.push_str("## Sections\n\n");
 for (section_id, _atomic) in &store.sections {
 // Atomic-only fallback header — title / decision_status fetch
 // from workspace lands with section migration (Round 164+) so
 // the cross-ref shift wire is testable end-to-end at that point.
 out.push_str(&format!(
  "### §{} (atomic-only — title from workspace pending Round 164+)\n\n",
  section_id
 ));
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

fn resolve_output(workspace_root: &Path, output: Option<&str>) -> PathBuf {
 match output {
 Some(p) => {
 let pb = PathBuf::from(p);
 if pb.is_absolute() {
  pb
 } else {
  workspace_root.join(pb)
 }
 }
 None => workspace_root.join("docs/GENERATED.md"),
 }
}

/// `generate-docs` subcommand — render atomic store → GENERATED.md.
///
/// Round 163 forward-wire: from this round, the atomic store is the primary changelog
/// ledger scope (legacy DESIGN.md Changelog stays frozen — Round 19 consistency).
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
/// Round 168 — pre-commit hook entry point. Exit 0 = sync, exit 1 = stale.
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
/// (Round 169 dogfood-switch ratify).
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
/// validate-workspace (Round 169) and audit ledgers (Round 167) to share a
/// single audit definition.
pub fn validate_atomic_store(
 workspace_root: &Path,
 section_id_set: &std::collections::BTreeSet<String>,
) -> Result<AtomicValidationSummary> {
 let sidecar_path = AtomicStore::default_sidecar_path(workspace_root);
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
/// auto-update behavior single-sourced (Round 168 ratify).
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
