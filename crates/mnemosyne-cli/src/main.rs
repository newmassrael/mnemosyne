//! mnemosyne-cli — Phase 0 dogfood entry point.
//!
//! Spec binding: §code-citation-defense (via cmd_validate_code_refs).
//!
//! 3 sub-commands:
//!
//! - `validate <file>` — single doc T1 + intra-doc round-trip validation.
//! - `validate-workspace` — 7 markdown doc workspace lookup + reclassify +
//! round-trip 7/7 mandatory preserved.
//! - `commit <file>` — file validation, then binding through ProposalHandler with audit append
//! (mnemosyne-server embedded API).
//!
//! pre-commit hook + CI workflow this binary invoke with design_doc lifecycle
//! Performs auto-validation — OPTION C Phase 0 dogfood entry source.

mod atomic_cli;

use anyhow::{anyhow, bail, Context, Result};
use mnemosyne_server::{MnemosyneServer, Proposal, ProposalKind};
use mnemosyne_store::MnemosyneStore;
use mnemosyne_validator::{
 add_cross_ref, add_section, append_changelog_entry, check_style,
 code_refs::{scan_paths_bidirectional, CodeRefViolation, ViolationKind},
 compare_typed_facts, default_ruleset_with_config, discover_config,
 emitter::emit_markdown_with_default,
 parse_markdown_with_schema,
 query::{
 build_envelope, changelog_entries_for_section, related_sections_with_atomic,
 section_by_id, workspace_section_id_set,
 },
 schema::{DecisionStatus, RefKind},
 set_section_body, set_section_decision_status,
 style::{StyleSeverity, StyleViolation},
 validator::cross_ref_orphan_reject_with_workspace,
 AtomicStore, LoadedConfig, MutateError, OrphanKind, ParsedDoc, SchemaSection, ValidationError,
 Workspace,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::{Arc, OnceLock};
use std::{env, fs};

/// workspace config (mnemosyne.toml) cached on first lookup.
/// `discover_config` walks upward from CWD looking for `mnemosyne.toml`
/// (or `.mnemosyne/config.toml`); the loaded config provides
/// `workspace.docs` + `workspace.default_doc` + workspace_root.
fn workspace_config() -> Result<&'static LoadedConfig> {
 static CACHE: OnceLock<LoadedConfig> = OnceLock::new();
 if let Some(loaded) = CACHE.get() {
 return Ok(loaded);
 }
 let cwd = env::current_dir().context("CWD lookup")?;
 let loaded = discover_config(&cwd)?
 .ok_or_else(|| anyhow!("mnemosyne.toml not found — CWD or ancestor in config file required"))?;
 let _ = CACHE.set(loaded);
 Ok(CACHE.get().expect("just set"))
}

/// schema config (mnemosyne.toml `[schema]`) cached on first
/// lookup. Falls back to [`SchemaSection::mnemosyne_preset`] when the
/// loaded config omits `[schema]` (back-compat with pre-143 configs).
fn cli_schema() -> Result<&'static SchemaSection> {
 static CACHE: OnceLock<SchemaSection> = OnceLock::new();
 if let Some(s) = CACHE.get() {
 return Ok(s);
 }
 let loaded = workspace_config()?;
 let schema = loaded
 .config
 .schema
 .clone()
 .unwrap_or_else(SchemaSection::mnemosyne_preset);
 let _ = CACHE.set(schema);
 Ok(CACHE.get().expect("just set"))
}

/// Known-stale orphan ledger — type-system escape hatch pattern (TypeScript
/// `@ts-expect-error` / Rust `#[allow(lint, reason)]` equivalent, ratify).
///
/// validate-workspace invariant: the actual orphan set must exactly match this ledger.
/// **set-equal**. bidirectional validation:
/// - actual ∖ ledger ≠ ∅ → new orphan (ledger registered or fix enforced)
/// - ledger ∖ actual ≠ ∅ → some ledger entry was resolved (delete the entry — drift catch)
///
/// This ledger is an information-complete replacement for the baseline counter.
struct KnownStaleOrphan {
 doc: &'static str,
 from_section: &'static str,
 to_target: &'static str,
 /// *Why* remaining -- resolution path requires author ratify.
 reason: &'static str,
 /// this ledger at registered round id.
 tracked_since: &'static str,
}

const KNOWN_STALE_ORPHANS: &[KnownStaleOrphan] = &[];

fn main() -> ExitCode {
 let args: Vec<String> = env::args().collect();
 match run(&args) {
 Ok(()) => ExitCode::SUCCESS,
 Err(e) => {
 eprintln!("error: {:#}", e);
 ExitCode::FAILURE
 }
 }
}

fn run(args: &[String]) -> Result<()> {
 let prog = args
 .first()
 .map(String::as_str)
 .unwrap_or("mnemosyne-cli");
 let cmd = args.get(1).ok_or_else(|| {
 anyhow!(
 "usage: {} <validate|validate-workspace|commit|query|append-changelog-entry|add-section|add-cross-ref|set-section-decision-status|set-section-body|style-check|list-docs|set-section-intent|set-section-rationale|set-section-inputs|set-section-outputs|add-section-caveat|set-section-alternatives|set-section-impact-scope|add-section-example|add-section-implementation|set-section-decision-status-atomic|append-changelog-entry-v2|generate-docs|verify-generated> [args...]",
 prog
 )
 })?;

 match cmd.as_str() {
 "validate" => {
 let file = args
  .get(2)
  .ok_or_else(|| anyhow!("usage: {} validate <file>", prog))?;
 cmd_validate(file)
 }
 "validate-workspace" => cmd_validate_workspace(),
 "commit" => {
 let file = args
  .get(2)
  .ok_or_else(|| anyhow!("usage: {} commit <file>", prog))?;
 cmd_commit(file)
 }
 "query" => cmd_query(prog, &args[2..]),
 "append-changelog-entry" => cmd_append_changelog_entry(prog, &args[2..]),
 "add-section" => cmd_add_section(prog, &args[2..]),
 "add-cross-ref" => cmd_add_cross_ref(prog, &args[2..]),
 "set-section-decision-status" => cmd_set_section_decision_status(prog, &args[2..]),
 "set-section-body" => cmd_set_section_body(prog, &args[2..]),
 "style-check" => cmd_style_check(prog, &args[2..]),
 "list-docs" => cmd_list_docs(),
 // atomic mutate API surface.
 "set-section-intent" => atomic_cli::cmd_set_section_intent(&repo_root()?, &args[2..]),
 "set-section-rationale" => atomic_cli::cmd_set_section_rationale(&repo_root()?, &args[2..]),
 "set-section-inputs" => atomic_cli::cmd_set_section_inputs(&repo_root()?, &args[2..]),
 "set-section-outputs" => atomic_cli::cmd_set_section_outputs(&repo_root()?, &args[2..]),
 "add-section-caveat" => atomic_cli::cmd_add_section_caveat(&repo_root()?, &args[2..]),
 "set-section-alternatives" => {
 atomic_cli::cmd_set_section_alternatives(&repo_root()?, &args[2..])
 }
 "set-section-impact-scope" => {
 atomic_cli::cmd_set_section_impact_scope(&repo_root()?, &args[2..])
 }
 "add-section-example" => atomic_cli::cmd_add_section_example(&repo_root()?, &args[2..]),
 // Path B (Spec ↔ Code bidirectional binding) substrate.
 "add-section-implementation" => {
 atomic_cli::cmd_add_section_implementation(&repo_root()?, &args[2..])
 }
 // Round 265 — Stage B freshness substrate.
 "set-section-decision-status-atomic" => {
 atomic_cli::cmd_set_section_decision_status_atomic(&repo_root()?, &args[2..])
 }
 "append-changelog-entry-v2" => {
 atomic_cli::cmd_append_changelog_entry_v2(&repo_root()?, &args[2..])
 }
 "generate-docs" => atomic_cli::cmd_generate_docs(&repo_root()?, &args[2..]),
 "verify-generated" => atomic_cli::cmd_verify_generated(&repo_root()?, &args[2..]),
 // Stage 2 of code-citation defense (Stage 1 = CLAUDE.md
 // rule, carry).
 "validate-code-refs" => cmd_validate_code_refs(&args[2..]),
 "--help" | "-h" | "help" => {
 print_help(prog);
 Ok(())
 }
 other => bail!("unknown command: {} (run `{} --help`)", other, prog),
 }
}

/// print the configured workspace doc list (one per line) for
/// shell consumers (pre-commit hook, CI scripts, external user automation).
fn cmd_list_docs() -> Result<()> {
 for path in workspace_config()?.doc_paths() {
 println!("{}", path);
 }
 Ok(())
}

fn print_help(prog: &str) {
 println!("mnemosyne-cli — Phase 0 design_doc lifecycle (DESIGN §66)");
 println!();
 println!("usage:");
 println!(" {} validate <file> single-doc T1 + round-trip", prog);
 println!(
 " {} validate-workspace 7 markdown doc full validation",
 prog
 );
 println!(
 " {} commit <file>  validate + audit append (proposal handler binding)",
 prog
 );
 println!(
 " {} query §<section_id> [--include-related] [--include-changelog] [--json]",
 prog
 );
 println!(
 " {} query --list-sections workspace full section_id set print",
 prog
 );
 println!(
 " {} append-changelog-entry --doc <doc> --entry-id \"Round N\" --body-file <path> [--title <text>] [--json]",
 prog
 );
 println!(
 "   §15 mutate primitive v1 — *legacy markdown surgical insert* (Round 246 cascade C)"
 );
 println!(
 "   production = `append-changelog-entry-v2` (atomic store, Round 162)"
 );
 println!(
 " {} style-check [--doc <path>] [--severity t3|t4|all] [--json]",
 prog
 );
 println!(
 "   T3/T4 style rule layer check (Round 129 production wire)"
 );
 println!();
 println!(" --- atomic mutate API (Round 162 production wire, Phase 0f) ---");
 println!(" {} set-section-intent --section §<N> --intent <text> [--sidecar <path>] [--json]", prog);
 println!(" {} set-section-rationale --section §<N> --bullets-file <path> [--sidecar <path>] [--json]", prog);
 println!(" {} set-section-inputs --section §<N> --bullets-file <path> [--sidecar <path>] [--json]", prog);
 println!(" {} set-section-outputs --section §<N> --bullets-file <path> [--sidecar <path>] [--json]", prog);
 println!(" {} add-section-caveat --section §<N> --bullet <text> [--sidecar <path>] [--json]", prog);
 println!(
 " {} set-section-alternatives --section §<N> --alternatives-file <path> [--sidecar <path>] [--json]",
 prog
 );
 println!(
 " {} set-section-impact-scope --section §<N> --refs §A,§B,... [--sidecar <path>] [--json]",
 prog
 );
 println!(
 " {} add-section-example --section §<N> --language <lang> --code-file <path> [--sidecar <path>] [--json]",
 prog
 );
 println!(
 " {} add-section-implementation --section §<N> --file <workspace-relative-path> [--symbol <name>] [--sidecar <path>] [--json]",
 prog
 );
 println!(
 "   Round 259 Path B substrate (Spec ↔ Code binding); validator cross-check is Round 260+"
 );
 println!(
 " {} set-section-decision-status-atomic --section §<N> --status active|superseded|removed [--sidecar <path>] [--json]",
 prog
 );
 println!(
 "   Round 265 atomic decision_status setter (Stage B freshness substrate)"
 );
 println!(
 " {} append-changelog-entry-v2 --entry-id \"Round N\" --decision <text> --changes-file <path> --verification-file <path> --impact §A,§B --carry-file <path> [--sidecar <path>] [--json]",
 prog
 );
 println!(" {} generate-docs [--sidecar <path>] [--output <path>]", prog);
 println!(
 "   atomic store → GENERATED.md (default docs/GENERATED.md, Round 163 forward-wire)"
 );
 println!(" {} verify-generated [--sidecar <path>] [--output <path>]", prog);
 println!(
 "   exit 0 if GENERATED.md sync, exit 1 if stale (Round 168 cascade auto-update gate)"
 );
 println!(
 "   atomic mutate subcommands above auto-regenerate GENERATED.md (override: --no-regenerate)"
 );
 println!();
 println!(" --- code citation defense (Round 255-260, Path B bidirectional) ---");
 println!(
 " {} validate-code-refs [--severity-missing reject|warn|info]\n\
 \x20                       [--severity-binding reject|warn|info]\n\
 \x20                       [--filter-id <entry_id>] [--json]",
 prog
 );
 println!(
 "   Round 256: scan [code_refs].paths for <entry_id_prefix><digits> citations,"
 );
 println!(
 "   reject those whose entry_id is missing from atomic store changelog_entries"
 );
 println!(
 "   Round 260: §<id> citations cross-checked against AtomicSection.implementations"
 );
 println!(
 "   --severity-missing: Missing + SectionMissing (hallucination class)"
 );
 println!(
 "   --severity-binding (Round 260): CitationUnbound + ImplementationUnbacked (set-equality class)"
 );
 println!(
 "   --filter-id (Round 258): restrict to citations of one id; surfaces them as decay (cascade caller use)"
 );
}

// ============================================================================
// validate <file> — single doc T1 + round-trip.
// ============================================================================

fn cmd_validate(file: &str) -> Result<()> {
 let abs = PathBuf::from(file)
 .canonicalize()
 .with_context(|| format!("file recovery failed: {}", file))?;
 let rel = repo_relative_path(&abs)?;
 let content = fs::read_to_string(&abs).with_context(|| format!("read {}", abs.display()))?;
 let schema = cli_schema()?;
 let parsed = parse_markdown_with_schema(&content, &rel, schema);

 // Single-doc T1 (intra-doc only) — workspace fallback validate-workspace scope.
 let orphans = mnemosyne_validator::validator::cross_ref_orphan_reject(&parsed);

 // Round-trip — single doc scope, default_doc = None.
 let emitted = emit_markdown_with_default(&parsed, None);
 let reparsed = parse_markdown_with_schema(&emitted, &rel, schema);
 let diff = compare_typed_facts(&parsed, &reparsed);

 println!("=== mnemosyne-cli validate {} ===", rel);
 println!(
 "sections={} changelog={} cross_refs={} orphans={}",
 parsed.sections.len(),
 parsed.changelog_entries.len(),
 parsed.cross_refs.len(),
 orphans.len(),
 );
 print_orphans(&rel, &orphans, 20);
 println!(
 "round-trip mandatory_preserved={} (section_identity={}, changelog_sequence={}, cross_ref_set={})",
 diff.mandatory_preserved,
 diff.section_identity_match,
 diff.changelog_sequence_match,
 diff.cross_ref_set_match,
 );

 if !diff.mandatory_preserved {
 // Round-trip diagnostic — surface the typed-fact diff so authors can
 // pinpoint which section_id / cross_ref tuples drifted between
 // parse → emit → re-parse. Without this dump the only signal is a
 // boolean per dimension, which is insufficient to locate the cause
 // in a real-world doc with hundreds of sections.
 let a_keys: BTreeSet<(String, Option<String>, String)> = parsed
 .sections
 .iter()
 .map(|s| (s.section_id.clone(), s.parent_section.clone(), s.title.clone()))
 .collect();
 let b_keys: BTreeSet<(String, Option<String>, String)> = reparsed
 .sections
 .iter()
 .map(|s| (s.section_id.clone(), s.parent_section.clone(), s.title.clone()))
 .collect();
 if !diff.section_identity_match {
 eprintln!("--- section diff (a-only / b-only, up to 15 each) ---");
 for k in a_keys.difference(&b_keys).take(15) {
 eprintln!("  -A {:?}", k);
 }
 for k in b_keys.difference(&a_keys).take(15) {
 eprintln!("  +B {:?}", k);
 }
 }
 if !diff.cross_ref_set_match {
 let a_cross: BTreeSet<(String, String, String)> = parsed
 .cross_refs
 .iter()
 .map(|c| (c.from_section.clone(), c.to_target.clone(), format!("{:?}", c.ref_kind)))
 .collect();
 let b_cross: BTreeSet<(String, String, String)> = reparsed
 .cross_refs
 .iter()
 .map(|c| (c.from_section.clone(), c.to_target.clone(), format!("{:?}", c.ref_kind)))
 .collect();
 eprintln!("--- cross_ref diff (a-only / b-only, up to 20 each) ---");
 for c in a_cross.difference(&b_cross).take(20) {
 eprintln!("  -A {:?}", c);
 }
 for c in b_cross.difference(&a_cross).take(20) {
 eprintln!("  +B {:?}", c);
 }
 }
 bail!(
 "round-trip mandatory preserved break — sections {}->{} / changelog {}->{} / cross_ref {}->{}",
 diff.section_count_a,
 diff.section_count_b,
 diff.changelog_entry_count_a,
 diff.changelog_entry_count_b,
 diff.cross_ref_count_a,
 diff.cross_ref_count_b,
 );
 }
 // Single-doc orphan cross-doc intent possible — workspace scope validation encourage.
 if !orphans.is_empty() {
 println!(
 "note: single-doc orphan {}cases — cross-doc intent if `validate-workspace` in step (2) reclassify validation",
 orphans.len()
 );
 }
 Ok(())
}

// ============================================================================
// query spec query API surface.
// ============================================================================

#[derive(Debug, Default)]
struct QueryArgs {
 section_id: Option<String>,
 include_related: bool,
 include_changelog: bool,
 json: bool,
 list_sections: bool,
}

fn parse_query_args(args: &[String]) -> Result<QueryArgs> {
 let mut out = QueryArgs::default();
 for arg in args {
 match arg.as_str() {
 "--include-related" => out.include_related = true,
 "--include-changelog" => out.include_changelog = true,
 "--json" => out.json = true,
 "--list-sections" => out.list_sections = true,
 other if other.starts_with("--") => bail!("unknown flag `{}`", other),
 other => {
  if out.section_id.is_some() {
  bail!("section_id argument duplicate (already `{}`)", out.section_id.unwrap());
  }
  let stripped = other.strip_prefix('§').unwrap_or(other).to_string();
  out.section_id = Some(stripped);
 }
 }
 }
 Ok(out)
}

fn cmd_query(prog: &str, args: &[String]) -> Result<()> {
 let qargs = parse_query_args(args)?;
 let root = repo_root()?;
 let (ws, _parsed_docs) = load_workspace(&root)?;
 // cascade B — atomic-first citation surface in atomic store load.
 let atomic_store =
 AtomicStore::load(&AtomicStore::default_sidecar_path(&root)).unwrap_or_default();

 if qargs.list_sections {
 // list_sections covers BOTH the markdown-derived workspace
 // sections and the atomic-store-derived sections. Post 7-md deletion
 // the markdown side is GENERATED.md only (slug-form headings), and
 // the canonical numeric/`X/Y` ids live in the atomic store.
 let mut set = workspace_section_id_set(&ws);
 set.extend(atomic_store.atomic_section_id_set());
 for id in &set {
 println!("{}", id);
 }
 eprintln!("# total {} section(s)", set.len());
 return Ok(());
 }

 let section_id = qargs.section_id.ok_or_else(|| {
 anyhow!("section_id arg required — e.g. {} query §43", prog)
 })?;

 if qargs.json && qargs.include_related && qargs.include_changelog {
 let envelope = build_envelope(&ws, &atomic_store, &section_id)
 .ok_or_else(|| anyhow!("section_id `{}` workspace in not found", section_id))?;
 println!("{}", serde_json::to_string_pretty(&envelope)?);
 return Ok(());
 }

 if qargs.json {
 let view = section_by_id(&ws, &atomic_store, &section_id)
 .ok_or_else(|| anyhow!("section_id `{}` workspace in not found", section_id))?;
 println!("{}", serde_json::to_string_pretty(&view)?);
 return Ok(());
 }

 let view = section_by_id(&ws, &atomic_store, &section_id)
 .ok_or_else(|| anyhow!("section_id `{}` workspace in not found", section_id))?;
 println!(
 "§{} ({}#L{}) {}",
 view.section_id, view.parent_doc, view.line_anchor, view.title
 );
 println!("decision_status: {}", view.decision_status);
 if let Some(parent) = &view.parent_section {
 println!("parent_section: §{}", parent);
 }
 if !view.body.is_empty() {
 println!();
 println!("--- body ---");
 println!("{}", view.body);
 println!("--- end body ---");
 }

 if qargs.include_related {
 let related = related_sections_with_atomic(&ws, &atomic_store, &section_id);
 println!();
 println!("outbound_refs ({}):", related.outbound_refs.len());
 for r in &related.outbound_refs {
 println!(" {} → {} [{}]", r.from_section, r.to_target, r.ref_kind);
 }
 println!();
 println!("inbound_refs ({}):", related.inbound_refs.len());
 for r in &related.inbound_refs {
 println!(" {}#§{} → {} [{}]", r.from_doc, r.from_section, r.to_target, r.ref_kind);
 }
 }

 if qargs.include_changelog {
 let entries = changelog_entries_for_section(&ws, &atomic_store, &section_id);
 println!();
 println!("related_changelog_entries ({}):", entries.len());
 for e in &entries {
 // atomic surface exposed: atomic_changes/verification/carry
 // bullets summed + impact_refs structural count.
 let atomic_field_count = e.atomic_decision_summary.is_some() as usize
  + e.atomic_changes_bullets.len()
  + e.atomic_verification_bullets.len()
  + e.atomic_carry_forward_bullets.len()
  + e.atomic_impact_refs.len();
 println!(
  " [{}] {} (txn={}, citations={}, sub_bullets={}, atomic_fields={})",
  e.parent_doc,
  e.entry_id,
  e.frozen_at_transaction_time,
  e.citation_count,
  e.sub_bullets.len(),
  atomic_field_count
 );
 }
 }

 Ok(())
}

// ============================================================================
// append-changelog-entry mutate API surface.
// ============================================================================

#[derive(Debug, Default)]
struct AppendChangelogArgs {
 doc: Option<String>,
 entry_id: Option<String>,
 title: Option<String>,
 body_file: Option<String>,
 json: bool,
 transaction_time: Option<i64>,
}

fn parse_append_changelog_args(args: &[String]) -> Result<AppendChangelogArgs> {
 let mut out = AppendChangelogArgs::default();
 let mut iter = args.iter();
 while let Some(arg) = iter.next() {
 match arg.as_str() {
 "--doc" => {
  out.doc = Some(
  iter.next()
  .ok_or_else(|| anyhow!("--doc argument missing"))?
  .clone(),
  );
 }
 "--entry-id" => {
  out.entry_id = Some(
  iter.next()
  .ok_or_else(|| anyhow!("--entry-id argument missing"))?
  .clone(),
  );
 }
 "--title" => {
  out.title = Some(
  iter.next()
  .ok_or_else(|| anyhow!("--title argument missing"))?
  .clone(),
  );
 }
 "--body-file" => {
  out.body_file = Some(
  iter.next()
  .ok_or_else(|| anyhow!("--body-file argument missing"))?
  .clone(),
  );
 }
 "--transaction-time" => {
  let v = iter
  .next()
  .ok_or_else(|| anyhow!("--transaction-time argument missing"))?;
  out.transaction_time = Some(v.parse::<i64>().context("--transaction-time integer parse failure")?);
 }
 "--json" => out.json = true,
 other => bail!("unknown flag `{}`", other),
 }
 }
 Ok(out)
}

/// Body-file format: each line starting with `- ` begins a new sub_bullet.
/// subsequent line (until next `- ` or EOF) appended to current bullet (joined with single space).
fn parse_body_file(content: &str) -> Result<Vec<String>> {
 let mut bullets: Vec<String> = Vec::new();
 let mut current: Option<String> = None;
 for line in content.lines() {
 let trimmed = line.trim_end();
 if let Some(rest) = trimmed.strip_prefix("- ") {
 if let Some(prev) = current.take() {
  bullets.push(prev);
 }
 current = Some(rest.to_string());
 } else if trimmed.is_empty() {
 // Blank line — preserve as continuation only if a bullet is in progress.
 if let Some(prev) = current.take() {
  bullets.push(prev);
 }
 current = None;
 } else if let Some(c) = current.as_mut() {
 // Continuation — append to current bullet with a space.
 if !c.is_empty() {
  c.push(' ');
 }
 c.push_str(trimmed.trim_start());
 } else {
 bail!(
  "body-file in first non-blank line `- ` prefix enforced — got `{}`",
  line
 );
 }
 }
 if let Some(prev) = current.take() {
 bullets.push(prev);
 }
 if bullets.is_empty() {
 bail!("body-file from sub_bullet missing (file empty or `- ` prefix missing)");
 }
 Ok(bullets)
}

/// *legacy v1 markdown-mutate path* (sub_bullets cascade C
/// marker). Production = [`cmd_append_changelog_entry_v2`] via
/// [`crate::atomic::append_changelog_entry_v2`]. This entry point handles the -162
/// Surgical insert is a dedicated legacy-markdown carry; new entries should use v2.
fn cmd_append_changelog_entry(prog: &str, args: &[String]) -> Result<()> {
 let cargs = parse_append_changelog_args(args)?;
 let doc = cargs.doc.ok_or_else(|| {
 anyhow!(
 "--doc arg required — e.g. {} append-changelog-entry --doc docs/DESIGN.md ...",
 prog
 )
 })?;
 let entry_id = cargs
 .entry_id
 .ok_or_else(|| anyhow!("--entry-id arg required — e.g. --entry-id \"Round 124\""))?;
 let body_file_path = cargs
 .body_file
 .ok_or_else(|| anyhow!("--body-file arg required"))?;

 let root = repo_root()?;
 let body_abs = if Path::new(&body_file_path).is_absolute() {
 PathBuf::from(&body_file_path)
 } else {
 env::current_dir()?.join(&body_file_path)
 };
 let body_content = fs::read_to_string(&body_abs)
 .with_context(|| format!("body-file recovery failed: {}", body_abs.display()))?;
 let sub_bullets = parse_body_file(&body_content)?;

 let (ws, _parsed_docs) = load_workspace(&root)?;

 // Default transaction_time = current Unix seconds (monotonic in practice).
 let txn_time = cargs
 .transaction_time
 .unwrap_or_else(|| current_unix_seconds() as i64);

 let receipt_result = append_changelog_entry(
 &ws,
 &doc,
 &entry_id,
 cargs.title.as_deref(),
 &sub_bullets,
 txn_time,
 &root,
 );

 match receipt_result {
 Ok(receipt) => {
 print_mutate_receipt(&receipt, cargs.json);
 Ok(())
 }
 Err(err) => {
 // Emit error in structured form for AI agent observability.
 print_mutate_error(&err, cargs.json);
 // Convert to anyhow::Error for non-zero exit.
 Err(anyhow!("{}", err))
 }
 }
}

fn print_mutate_error(err: &MutateError, json: bool) {
 if json {
 let val = serde_json::json!({
 "primitive": err.primitive,
 "kind": format!("{:?}", err.kind),
 "detail": err.detail,
 });
 eprintln!("{}", serde_json::to_string_pretty(&val).unwrap_or_default());
 } else {
 eprintln!("=== mnemosyne-cli {} FAILED ===", err.primitive);
 eprintln!("primitive: {}", err.primitive);
 eprintln!("kind: {:?}", err.kind);
 eprintln!("detail: {}", err.detail);
 }
}

fn print_mutate_receipt(receipt: &mnemosyne_validator::MutateReceipt, json: bool) {
 let style_summary = compute_post_mutate_style_summary(&receipt.affected_docs);
 if json {
 let mut value = serde_json::to_value(receipt).unwrap_or(serde_json::Value::Null);
 if let serde_json::Value::Object(ref mut map) = value {
 map.insert(
  "post_mutate_style_summary".into(),
  serde_json::to_value(&style_summary).unwrap_or(serde_json::Value::Null),
 );
 }
 if let Ok(s) = serde_json::to_string_pretty(&value) {
 println!("{}", s);
 }
 } else {
 println!("=== mnemosyne-cli {} ===", receipt.primitive);
 println!("primitive: {}", receipt.primitive);
 println!("affected_docs: {:?}", receipt.affected_docs);
 println!("affected_sections: {:?}", receipt.affected_sections);
 for (d, b) in &receipt.written_bytes_per_doc {
 println!("written_bytes[{}]: {}", d, b);
 }
 println!("round_trip_diff_count: {}", receipt.round_trip_diff_count);
 println!("validator_path_invocations:");
 for v in &receipt.validator_path_invocations {
 println!(" - {}", v);
 }
 println!(
 "applied_at_transaction_time: {}",
 receipt.applied_at_transaction_time
 );
 if !style_summary.is_empty() {
 println!("post_mutate_style_summary (warn-only, Round 138 reject activate carry):");
 for (doc, (warn, info)) in &style_summary {
  println!(" {}: T3 warn={} / T4 info={}", doc, warn, info);
 }
 }
 }
}

/// Re-parse the affected docs and run the default style ruleset, returning
/// per-doc (warn_count, info_count). Pure side-effect-free read pass — used
/// to attach a style summary to mutate receipts.
fn compute_post_mutate_style_summary(
 affected_docs: &[String],
) -> std::collections::BTreeMap<String, (usize, usize)> {
 use mnemosyne_validator::check_style;
 let mut out: std::collections::BTreeMap<String, (usize, usize)> = Default::default();
 let root = match repo_root() {
 Ok(p) => p,
 Err(_) => return out,
 };
 let cfg = match workspace_config() {
 Ok(c) => c,
 Err(_) => return out,
 };
 let ruleset = default_ruleset_with_config(
 cfg.config.style.as_ref(),
 cfg.config.terminology.as_ref(),
 );
 let schema = match cli_schema() {
 Ok(s) => s,
 Err(_) => return out,
 };
 let atomic_store =
 AtomicStore::load(&AtomicStore::default_sidecar_path(&root)).unwrap_or_default();
 for doc_path in affected_docs {
 let abs = root.join(doc_path);
 let content = match fs::read_to_string(&abs) {
 Ok(s) => s,
 Err(_) => continue,
 };
 let parsed = parse_markdown_with_schema(&content, doc_path, schema);
 let v = check_style(doc_path, &parsed, &atomic_store, &ruleset);
 let warn = v
 .iter()
 .filter(|x| x.severity == StyleSeverity::Warn)
 .count();
 let info = v
 .iter()
 .filter(|x| x.severity == StyleSeverity::Info)
 .count();
 out.insert(doc_path.clone(), (warn, info));
 }
 out
}

// ---- add-section ----

fn cmd_add_section(prog: &str, args: &[String]) -> Result<()> {
 let mut doc: Option<String> = None;
 let mut parent: Option<String> = None;
 let mut title: Option<String> = None;
 let mut numbered_id: Option<String> = None;
 let mut body_file: Option<String> = None;
 let mut json = false;
 let mut iter = args.iter();
 while let Some(arg) = iter.next() {
 match arg.as_str() {
 "--doc" => doc = Some(iter.next().ok_or_else(|| anyhow!("--doc missing"))?.clone()),
 "--parent" => parent = Some(iter.next().ok_or_else(|| anyhow!("--parent missing"))?.clone()),
 "--title" => title = Some(iter.next().ok_or_else(|| anyhow!("--title missing"))?.clone()),
 "--numbered-id" => {
  numbered_id =
  Some(iter.next().ok_or_else(|| anyhow!("--numbered-id missing"))?.clone())
 }
 "--body-file" => {
  body_file = Some(iter.next().ok_or_else(|| anyhow!("--body-file missing"))?.clone())
 }
 "--json" => json = true,
 other => bail!("unknown flag `{}`", other),
 }
 }
 let doc =
 doc.ok_or_else(|| anyhow!("--doc arg required — e.g. {} add-section --doc docs/DESIGN.md", prog))?;
 let title = title.ok_or_else(|| anyhow!("--title arg required"))?;
 let body = match body_file {
 Some(p) => fs::read_to_string(&p).with_context(|| format!("body-file recovery failed: {}", p))?,
 None => String::new(),
 };

 let parent_strip = parent.as_deref().map(|s| s.strip_prefix('§').unwrap_or(s).to_string());
 let numbered_strip = numbered_id
 .as_deref()
 .map(|s| s.strip_prefix('§').unwrap_or(s).to_string());

 let root = repo_root()?;
 let (ws, _) = load_workspace(&root)?;

 let result = add_section(
 &ws,
 &doc,
 parent_strip.as_deref(),
 &title,
 numbered_strip.as_deref(),
 &body,
 &root,
 );
 handle_mutate_result(result, json)
}

// ---- add-cross-ref ----

fn cmd_add_cross_ref(prog: &str, args: &[String]) -> Result<()> {
 let mut doc: Option<String> = None;
 let mut from_section: Option<String> = None;
 let mut to_target: Option<String> = None;
 let mut kind_str: Option<String> = None;
 let mut json = false;
 let mut iter = args.iter();
 while let Some(arg) = iter.next() {
 match arg.as_str() {
 "--doc" => doc = Some(iter.next().ok_or_else(|| anyhow!("--doc missing"))?.clone()),
 "--from" => {
  from_section = Some(iter.next().ok_or_else(|| anyhow!("--from missing"))?.clone())
 }
 "--to" => to_target = Some(iter.next().ok_or_else(|| anyhow!("--to missing"))?.clone()),
 "--kind" => kind_str = Some(iter.next().ok_or_else(|| anyhow!("--kind missing"))?.clone()),
 "--json" => json = true,
 other => bail!("unknown flag `{}`", other),
 }
 }
 let doc = doc.ok_or_else(|| anyhow!("--doc arg required"))?;
 let from_section = from_section.ok_or_else(|| anyhow!("--from arg required"))?;
 let to_target = to_target.ok_or_else(|| anyhow!("--to arg required"))?;
 let kind_str = kind_str.unwrap_or_else(|| "decision".to_string());

 let kind = match kind_str.as_str() {
 "decision" => RefKind::Decision,
 "impl" => RefKind::Impl,
 "cross_doc" => RefKind::CrossDoc,
 other => bail!(
 "--kind must be `decision` | `impl` | `cross_doc` (got `{}`); usage example: {} add-cross-ref --kind decision",
 other,
 prog
 ),
 };

 let from = from_section.strip_prefix('§').unwrap_or(&from_section).to_string();
 let to = to_target.strip_prefix('§').unwrap_or(&to_target).to_string();

 let root = repo_root()?;
 let (ws, _) = load_workspace(&root)?;

 let result = add_cross_ref(&ws, &doc, &from, &to, kind, &root);
 handle_mutate_result(result, json)
}

// ---- set-section-decision-status ----

fn cmd_set_section_decision_status(_prog: &str, args: &[String]) -> Result<()> {
 let mut doc: Option<String> = None;
 let mut section: Option<String> = None;
 let mut status: Option<String> = None;
 let mut superseding: Option<String> = None;
 let mut json = false;
 let mut iter = args.iter();
 while let Some(arg) = iter.next() {
 match arg.as_str() {
 "--doc" => doc = Some(iter.next().ok_or_else(|| anyhow!("--doc missing"))?.clone()),
 "--section" => section = Some(iter.next().ok_or_else(|| anyhow!("--section missing"))?.clone()),
 "--status" => status = Some(iter.next().ok_or_else(|| anyhow!("--status missing"))?.clone()),
 "--superseding" => {
  superseding = Some(iter.next().ok_or_else(|| anyhow!("--superseding missing"))?.clone())
 }
 "--json" => json = true,
 other => bail!("unknown flag `{}`", other),
 }
 }
 let doc = doc.ok_or_else(|| anyhow!("--doc arg required"))?;
 let section = section.ok_or_else(|| anyhow!("--section arg required"))?;
 let status = status.ok_or_else(|| anyhow!("--status arg required"))?;

 let new_status = match status.as_str() {
 "active" => DecisionStatus::Active,
 "superseded" => DecisionStatus::Superseded,
 "removed" => DecisionStatus::Removed,
 other => bail!("--status must be `active` | `superseded` | `removed` (got `{}`)", other),
 };

 let section_strip = section.strip_prefix('§').unwrap_or(&section).to_string();
 let superseding_strip = superseding
 .as_deref()
 .map(|s| s.strip_prefix('§').unwrap_or(s).to_string());

 let root = repo_root()?;
 let (ws, _) = load_workspace(&root)?;

 let result = set_section_decision_status(
 &ws,
 &doc,
 &section_strip,
 new_status,
 superseding_strip.as_deref(),
 &root,
 );
 handle_mutate_result(result, json)
}

// ---- set-section-body ----

fn cmd_set_section_body(_prog: &str, args: &[String]) -> Result<()> {
 let mut doc: Option<String> = None;
 let mut section: Option<String> = None;
 let mut body_file: Option<String> = None;
 let mut json = false;
 let mut iter = args.iter();
 while let Some(arg) = iter.next() {
 match arg.as_str() {
 "--doc" => doc = Some(iter.next().ok_or_else(|| anyhow!("--doc missing"))?.clone()),
 "--section" => section = Some(iter.next().ok_or_else(|| anyhow!("--section missing"))?.clone()),
 "--body-file" => {
  body_file = Some(iter.next().ok_or_else(|| anyhow!("--body-file missing"))?.clone())
 }
 "--json" => json = true,
 other => bail!("unknown flag `{}`", other),
 }
 }
 let doc = doc.ok_or_else(|| anyhow!("--doc arg required"))?;
 let section = section.ok_or_else(|| anyhow!("--section arg required"))?;
 let body_file = body_file.ok_or_else(|| anyhow!("--body-file arg required"))?;
 let new_body = fs::read_to_string(&body_file).with_context(|| format!("body-file recovery failed: {}", body_file))?;

 let section_strip = section.strip_prefix('§').unwrap_or(&section).to_string();

 let root = repo_root()?;
 let (ws, _) = load_workspace(&root)?;

 let result = set_section_body(&ws, &doc, &section_strip, &new_body, &root);
 handle_mutate_result(result, json)
}

fn handle_mutate_result(
 result: Result<mnemosyne_validator::MutateReceipt, MutateError>,
 json: bool,
) -> Result<()> {
 match result {
 Ok(receipt) => {
 print_mutate_receipt(&receipt, json);
 Ok(())
 }
 Err(err) => {
 print_mutate_error(&err, json);
 Err(anyhow!("{}", err))
 }
 }
}

// ============================================================================
// validate-workspace — 7 doc full validation.
// ============================================================================

fn cmd_validate_workspace() -> Result<()> {
 let root = repo_root()?;
 let (ws, parsed_docs) = load_workspace(&root)?;

 // T1 step (1)+(2) in real orphan carry workspace fallback then).
 let mut actual_orphan_keys: BTreeSet<OrphanKey> = BTreeSet::new();
 let mut all_orphan_details: Vec<(String, Vec<ValidationError>)> = Vec::new();
 for (path, parsed) in &parsed_docs {
 let orphans = cross_ref_orphan_reject_with_workspace(parsed, &ws);
 for err in &orphans {
 if let ValidationError::OrphanCrossRef {
  from_section,
  to_target,
  ..
 } = err
 {
  actual_orphan_keys.insert(OrphanKey {
  doc: path.clone(),
  from_section: from_section.clone(),
  to_target: to_target.clone(),
  });
 }
 }
 if !orphans.is_empty() {
 all_orphan_details.push((path.clone(), orphans));
 }
 }

 // Round-trip 7/7 mandatory.
 let default_doc_for_emit = workspace_config()?
 .config
 .workspace
 .default_doc
 .as_deref();
 let schema = cli_schema()?;
 let mut round_trip_pass = 0usize;
 let mut round_trip_fail: Vec<String> = Vec::new();
 for (path, original) in &parsed_docs {
 let reclassified = ws
 .reclassify_cross_refs(path)
 .ok_or_else(|| anyhow!("workspace in {} not loaded — invariant break", path))?;
 let emitted = emit_markdown_with_default(&reclassified, default_doc_for_emit);
 let reparsed = parse_markdown_with_schema(&emitted, path, schema);
 let diff = compare_typed_facts(original, &reparsed);
 if diff.mandatory_preserved {
 round_trip_pass += 1;
 } else {
 round_trip_fail.push(format!(
  " {}: section={}/{} changelog={}/{} cross_ref={}/{}",
  path,
  diff.section_count_a,
  diff.section_count_b,
  diff.changelog_entry_count_a,
  diff.changelog_entry_count_b,
  diff.cross_ref_count_a,
  diff.cross_ref_count_b,
 ));
 }
 }

 // Ledger set-equality (Option D): actual orphan set ⇔ ledger.
 // ledger composes (set-union) from two sources:
 // 1. `KNOWN_STALE_ORPHANS` const, baked into the binary for
 // mnemosyne self-application carry (currently empty).
 // 2. `[[orphan_ledger]]` rows from the workspace's mnemosyne.toml,
 // authored by external workspaces to register their own legacy
 // carry without modifying the binary.
 let mut known_orphan_keys: BTreeSet<OrphanKey> = KNOWN_STALE_ORPHANS
 .iter()
 .map(|k| OrphanKey {
 doc: k.doc.to_string(),
 from_section: k.from_section.to_string(),
 to_target: k.to_target.to_string(),
 })
 .collect();
 let validate_workspace_cfg_for_ledger = workspace_config()?;
 // only kind=MarkdownRef entries compose into the markdown
 // orphan ledger. Atomic-internal kinds (AtomicEntryRef / AtomicSectionRef)
 // are composed into separate atomic-orphan ledger sets below at the
 // atomic store validation step.
 for entry in &validate_workspace_cfg_for_ledger.config.orphan_ledger {
 if entry.kind != OrphanKind::MarkdownRef {
 continue;
 }
 known_orphan_keys.insert(OrphanKey {
 doc: entry.doc.clone(),
 from_section: entry.from.clone(),
 to_target: entry.to.clone(),
 });
 }
 let new_orphans: Vec<&OrphanKey> = actual_orphan_keys
 .difference(&known_orphan_keys)
 .collect();
 let resolved_entries: Vec<&OrphanKey> = known_orphan_keys
 .difference(&actual_orphan_keys)
 .collect();

 println!("=== mnemosyne-cli validate-workspace ===");
 let configured_doc_count = workspace_config()?.config.workspace.docs.len();
 println!("docs={}/{}", parsed_docs.len(), configured_doc_count);
 println!(
 "T1 orphan total={} (ledger={}, new=+{}, resolved=-{})",
 actual_orphan_keys.len(),
 known_orphan_keys.len(),
 new_orphans.len(),
 resolved_entries.len(),
 );
 for (path, orphans) in &all_orphan_details {
 print_orphans(path, orphans, 5);
 }
 if !new_orphans.is_empty() {
 println!("new orphans (ledger registered or fix enforced):");
 for key in &new_orphans {
 println!(
  " + {}: §{} -> §{}",
  key.doc, key.from_section, key.to_target
 );
 }
 }
 if !resolved_entries.is_empty() {
 println!("resolved ledger entries (entry delete enforced — drift catch):");
 for key in &resolved_entries {
 println!(
  " - {}: §{} -> §{}",
  key.doc, key.from_section, key.to_target
 );
 }
 }
 let ledger_entries_count =
 KNOWN_STALE_ORPHANS.len() + validate_workspace_cfg_for_ledger.config.orphan_ledger.len();
 if ledger_entries_count > 0 {
 println!("known-stale ledger:");
 for entry in KNOWN_STALE_ORPHANS {
 println!(
  " [{}] (const) {}: §{} -> §{}",
  entry.tracked_since, entry.doc, entry.from_section, entry.to_target,
 );
 println!(" reason: {}", entry.reason);
 }
 for entry in &validate_workspace_cfg_for_ledger.config.orphan_ledger {
 println!(
  " [{}] (config) {}: §{} -> §{}",
  entry.since, entry.doc, entry.from, entry.to,
 );
 println!(" reason: {}", entry.reason);
 }
 }
 println!(
 "round-trip mandatory={}/{}",
 round_trip_pass,
 parsed_docs.len()
 );
 for line in &round_trip_fail {
 println!("{}", line);
 }

 // style violation summary surface; tier mobility:
 // T3 deterministic rule (`terminology_consistency`) reject is activated;
 // other T3 rules stay as warn (heuristic / subjective threshold) and T4
 // rules stay as info. See *Tier-per response* table for the closed-form
 // matrix.
 // ruleset thresholds + terminology glossary route through
 // mnemosyne.toml when present; mnemosyne_preset is the fallback.
 let validate_workspace_cfg = workspace_config()?;
 let ruleset = default_ruleset_with_config(
 validate_workspace_cfg.config.style.as_ref(),
 validate_workspace_cfg.config.terminology.as_ref(),
 );
 let validate_workspace_atomic =
 AtomicStore::load(&AtomicStore::default_sidecar_path(&root)).unwrap_or_default();
 let mut style_violations: Vec<StyleViolation> = Vec::new();
 for (path, parsed) in &parsed_docs {
 let mut v = check_style(path, parsed, &validate_workspace_atomic, &ruleset);
 style_violations.append(&mut v);
 }
 let terminology_violations: Vec<&StyleViolation> = style_violations
 .iter()
 .filter(|v| v.rule_id == "terminology_consistency")
 .collect();
 let t3_count = style_violations
 .iter()
 .filter(|v| v.severity == StyleSeverity::Warn)
 .count();
 let t4_count = style_violations
 .iter()
 .filter(|v| v.severity == StyleSeverity::Info)
 .count();
 let t3_reject_count = terminology_violations.len();
 let t3_warn_count = t3_count - t3_reject_count;
 println!(
 "style violations: T3 reject={} / T3 warn={} / T4 info={} (Round 138 tier mobility ratify)",
 t3_reject_count, t3_warn_count, t4_count
 );
 if t3_reject_count > 0 {
 println!("T3 reject violations (deterministic rule, terminology_consistency):");
 for v in &terminology_violations {
 println!(
  " - {}: §{} — {}",
  v.doc_path, v.section_id, v.message
 );
 }
 }

 if round_trip_pass != parsed_docs.len() {
 bail!(
 "round-trip 7/7 mandatory preserved break ({}/{} PASS) — Round 67 carry failure",
 round_trip_pass,
 parsed_docs.len()
 );
 }
 if !new_orphans.is_empty() {
 bail!(
 "new orphan {} cases introduced -- ledger registration or fix enforced (Round 80 OPTION D)",
 new_orphans.len()
 );
 }
 if !resolved_entries.is_empty() {
 bail!(
 "ledger in registered orphan {}cases resolved — KNOWN_STALE_ORPHANS entry delete enforced \
  (drift catch, Round 80 OPTION D bidirectional invariant)",
 resolved_entries.len()
 );
 }
 if t3_reject_count > 0 {
 bail!(
 "T3 deterministic violation {}cases — terminology_consistency rule \
  (Round 138 ratify, deterministic check scope rejectpermission activate)",
 t3_reject_count
 );
 }

 // dogfood-switch — atomic store = first-class workspace artifact.
 // Surface entries/sections count + cross-ref orphans + GENERATED.md sync.
 // Bail if atomic invariants violated (atomic ledger now part of the
 // validate-workspace contract, not just opt-in audit tests).
 //
 // atomic-first: orphan resolution uses the union of markdown-
 // derived workspace sections AND atomic store sections. When markdown is
 // the canonical source the two sets coincide; when atomic store becomes
 // sole source-of-truth (post 7-md deletion path) the markdown side may
 // collapse to an empty / GENERATED.md-only set, and the atomic side
 // carries the resolution.
 let mut id_set = workspace_section_id_set(&ws);
 id_set.extend(ws.atomic_id_set.iter().cloned());
 let atomic = atomic_cli::validate_atomic_store(&root, &id_set)?;
 // atomic-internal orphan ledger composition. Compose
 // (from, to) BTreeSets per kind from the same `[[orphan_ledger]]` table
 // that already cover markdown refs. `kind = AtomicEntryRef`
 // covers ChangelogEntry impact_refs; `kind = AtomicSectionRef` covers
 // Section impact_scope. Set-equality drift catch (new / resolved)
 // mirrors the markdown-ref pattern.
 let atomic_entry_actual: BTreeSet<(String, String)> =
 atomic.orphan_entry_refs.iter().cloned().collect();
 let atomic_section_actual: BTreeSet<(String, String)> =
 atomic.orphan_section_refs.iter().cloned().collect();
 let mut atomic_entry_ledger: BTreeSet<(String, String)> = BTreeSet::new();
 let mut atomic_section_ledger: BTreeSet<(String, String)> = BTreeSet::new();
 for entry in &validate_workspace_cfg_for_ledger.config.orphan_ledger {
 match entry.kind {
 OrphanKind::AtomicEntryRef => {
 atomic_entry_ledger.insert((entry.from.clone(), entry.to.clone()));
 }
 OrphanKind::AtomicSectionRef => {
 atomic_section_ledger.insert((entry.from.clone(), entry.to.clone()));
 }
 OrphanKind::MarkdownRef => {} // already composed into known_orphan_keys
 OrphanKind::CodeCitation => {} // Round 260 — code-axis ledger handled by validate-code-refs, not validate-workspace
 }
 }
 let new_atomic_entries: Vec<&(String, String)> = atomic_entry_actual
 .difference(&atomic_entry_ledger)
 .collect();
 let resolved_atomic_entries: Vec<&(String, String)> = atomic_entry_ledger
 .difference(&atomic_entry_actual)
 .collect();
 let new_atomic_sections: Vec<&(String, String)> = atomic_section_actual
 .difference(&atomic_section_ledger)
 .collect();
 let resolved_atomic_sections: Vec<&(String, String)> = atomic_section_ledger
 .difference(&atomic_section_actual)
 .collect();
 println!(
 "atomic ledger: entries={} / sections={} / orphan_refs={}+{} / GENERATED.md={}",
 atomic.entries,
 atomic.sections,
 atomic.orphan_entry_refs.len(),
 atomic.orphan_section_refs.len(),
 if atomic.generated_in_sync { "sync" } else { "STALE" }
 );
 if !atomic.orphan_entry_refs.is_empty() || !atomic_entry_ledger.is_empty() {
 println!(
 "atomic entry orphan_refs: ledger={}, new=+{}, resolved=-{}",
 atomic_entry_ledger.len(),
 new_atomic_entries.len(),
 resolved_atomic_entries.len(),
 );
 for (entry, target) in &atomic.orphan_entry_refs {
 let status =
 if atomic_entry_ledger.contains(&(entry.clone(), target.clone())) {
 "ledgered"
 } else {
 "new"
 };
 println!(" {} {}: §{}", status, entry, target);
 }
 for (entry, target) in &resolved_atomic_entries {
 println!(" resolved- {}: §{}", entry, target);
 }
 }
 if !atomic.orphan_section_refs.is_empty() || !atomic_section_ledger.is_empty() {
 println!(
 "atomic section orphan_refs: ledger={}, new=+{}, resolved=-{}",
 atomic_section_ledger.len(),
 new_atomic_sections.len(),
 resolved_atomic_sections.len(),
 );
 for (section, target) in &atomic.orphan_section_refs {
 let status = if atomic_section_ledger
 .contains(&(section.clone(), target.clone()))
 {
 "ledgered"
 } else {
 "new"
 };
 println!(" {} §{}: §{}", status, section, target);
 }
 for (section, target) in &resolved_atomic_sections {
 println!(" resolved- §{}: §{}", section, target);
 }
 }
 // reject only on un-ledgered new orphans or ledgered-but-
 // fixed (resolved) drift. Pure ledger carry passes. This is the textbook
 // scope-correction path: a Round entry records the scope change, then
 // the dangling refs are registered here with kind=atomic_entry_ref or
 // kind=atomic_section_ref pointing back at that Round in `reason`.
 let atomic_orphan_drift = new_atomic_entries.len()
 + resolved_atomic_entries.len()
 + new_atomic_sections.len()
 + resolved_atomic_sections.len();
 if atomic_orphan_drift > 0 {
 bail!(
 "atomic store cross-ref orphan drift {}cases — register in \
  [[orphan_ledger]] with kind=atomic_entry_ref or \
  kind=atomic_section_ref, or fix the source (Round 254 atomic-internal \
  orphan ledger; Round 169 dogfood-switch carry)",
 atomic_orphan_drift
 );
 }
 if !atomic.generated_in_sync {
 bail!(
 "GENERATED.md stale — atomic store in change then generate-docs non-pass and \
  (Round 168 cascade auto-update gate carry, Round 169 validate-workspace scope)"
 );
 }
 Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct OrphanKey {
 doc: String,
 from_section: String,
 to_target: String,
}

// ============================================================================
// commit <file> — validate + ProposalHandler binding.
// ============================================================================

fn cmd_commit(file: &str) -> Result<()> {
 let abs = PathBuf::from(file)
 .canonicalize()
 .with_context(|| format!("file recovery failed: {}", file))?;
 let rel = repo_relative_path(&abs)?;

 // Phase 1: validate-workspace and identical validation carry (round-trip + T1).
 cmd_validate_workspace().context("commit precondition: validate-workspace")?;

 // Phase 2: this file payload hash → ProposalKind::EntityCreate audit append.
 let content = fs::read_to_string(&abs).with_context(|| format!("read {}", abs.display()))?;
 let mut hasher = Sha256::new();
 hasher.update(rel.as_bytes());
 hasher.update(content.as_bytes());
 let digest = hasher.finalize();

 let store_dir = repo_root()?.join(".mnemosyne/store");
 fs::create_dir_all(&store_dir)
 .with_context(|| format!("store dir create failure: {}", store_dir.display()))?;
 let store = Arc::new(
 MnemosyneStore::open(&store_dir)
 .with_context(|| format!("store open failure: {}", store_dir.display()))?,
 );
 let server = MnemosyneServer::new(store);

 let valid_from = current_unix_seconds();
 let entity_id = stable_entity_id(&rel);
 let proposal = Proposal {
 proposal_id: format!("design-doc-commit-{}-{}", rel, valid_from),
 actor: env::var("USER").unwrap_or_else(|_| "mnemosyne-cli".to_string()),
 kind: ProposalKind::EntityCreate {
 entity_type: "DesignDocCommit".to_string(),
 branch_id: 1,
 entity_id,
 valid_from,
 payload: digest.to_vec(),
 },
 };
 let result = server
 .submit(&proposal)
 .map_err(|e| anyhow!("ProposalHandler submit failure: {:#}", e))?;

 println!("=== mnemosyne-cli commit {} ===", rel);
 println!("proposal_id={}", result.proposal_id);
 println!("accepted={}", result.accepted);
 if let Some(txn) = result.audit_transaction_id {
 println!("audit_transaction_id={}", txn);
 }
 if let Some(reason) = &result.rejection_reason {
 bail!("commit rejected: {}", reason);
 }
 Ok(())
}

// ============================================================================
// helpers
// ============================================================================

fn repo_root() -> Result<PathBuf> {
 // repo root = workspace_root from discovered mnemosyne.toml.
 // The legacy `.git + docs/DESIGN.md` heuristic is replaced by the explicit
 // config-driven workspace root (the config file's dir, or the
 // `[workspace] root` override).
 Ok(workspace_config()?.workspace_root.clone())
}

fn repo_relative_path(abs: &Path) -> Result<String> {
 let root = repo_root()?;
 let rel = abs
 .strip_prefix(&root)
 .with_context(|| format!("{} repo {} external", abs.display(), root.display()))?;
 Ok(rel.to_string_lossy().into_owned())
}

fn load_workspace(root: &Path) -> Result<(Workspace, Vec<(String, ParsedDoc)>)> {
 // workspace.docs + workspace.default_doc come from the
 // discovered config. `root` is the same workspace_root the config picks;
 // we accept it as parameter for callers that already resolved it.
 // schema config (changelog title set + medium_name) routes
 // through `parse_markdown_with_schema`, the production schema-aware path.
 // atomic store derived section_id set is injected into the
 // workspace so that `cross_ref_orphan_reject_with_workspace` step (2.5)
 // can resolve `to_target` against atomic store keys when markdown re-parse
 // (workspace.docs=[GENERATED.md] mode or 7-md deletion path) cannot.
 let loaded = workspace_config()?;
 let schema = cli_schema()?;
 let mut ws = Workspace::from_config(loaded);
 let atomic_for_id_set =
 AtomicStore::load(&AtomicStore::default_sidecar_path(root)).unwrap_or_default();
 ws.set_atomic_id_set(atomic_for_id_set.atomic_section_id_set());
 let doc_paths: Vec<&str> = loaded.doc_paths().collect();
 let mut parsed_docs: Vec<(String, ParsedDoc)> = Vec::with_capacity(doc_paths.len());
 for path in &doc_paths {
 let abs = root.join(path);
 let content =
 fs::read_to_string(&abs).with_context(|| format!("read {}", abs.display()))?;
 let parsed = parse_markdown_with_schema(&content, path, schema);
 ws.insert((*path).to_string(), parsed.clone());
 parsed_docs.push(((*path).to_string(), parsed));
 }
 Ok((ws, parsed_docs))
}

fn print_orphans(path: &str, orphans: &[ValidationError], limit: usize) {
 for err in orphans.iter().take(limit) {
 if let ValidationError::OrphanCrossRef {
 from_section,
 to_target,
 ref_kind,
 } = err
 {
 println!(
  " orphan {}: §{} -> §{} ({:?})",
  path, from_section, to_target, ref_kind
 );
 }
 }
 if orphans.len() > limit {
 println!(" ... +{} more in {}", orphans.len() - limit, path);
 }
}

fn current_unix_seconds() -> u64 {
 std::time::SystemTime::now()
 .duration_since(std::time::UNIX_EPOCH)
 .map(|d| d.as_secs())
 .unwrap_or(0)
}

fn stable_entity_id(rel_path: &str) -> u64 {
 // 8 byte stable digest prefix — same path → same entity_id.
 let mut hasher = Sha256::new();
 hasher.update(b"DesignDocCommit:");
 hasher.update(rel_path.as_bytes());
 let d = hasher.finalize();
 u64::from_be_bytes([d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7]])
}

// ============================================================================
// style-check — T3/T4 style rule layer.
// ============================================================================

fn cmd_style_check(prog: &str, args: &[String]) -> Result<()> {
 let mut doc_filter: Option<String> = None;
 let mut severity_filter = "all".to_string();
 let mut json = false;
 let mut iter = args.iter();
 while let Some(arg) = iter.next() {
 match arg.as_str() {
 "--doc" => {
  doc_filter = Some(iter.next().ok_or_else(|| anyhow!("--doc missing"))?.clone());
 }
 "--severity" => {
  severity_filter = iter
  .next()
  .ok_or_else(|| anyhow!("--severity missing"))?
  .clone();
 }
 "--json" => json = true,
 other => bail!(
  "unknown flag `{}` — usage: {} style-check [--doc <path>] [--severity t3|t4|all] [--json]",
  other,
  prog
 ),
 }
 }

 let root = repo_root()?;
 let (_ws, parsed_docs) = load_workspace(&root)?;
 let style_check_cfg = workspace_config()?;
 let ruleset = default_ruleset_with_config(
 style_check_cfg.config.style.as_ref(),
 style_check_cfg.config.terminology.as_ref(),
 );

 let style_check_atomic =
 AtomicStore::load(&AtomicStore::default_sidecar_path(&root)).unwrap_or_default();
 let mut all_violations: Vec<StyleViolation> = Vec::new();
 for (path, parsed) in &parsed_docs {
 if let Some(filter) = &doc_filter {
 if path != filter {
  continue;
 }
 }
 let mut v = check_style(path, parsed, &style_check_atomic, &ruleset);
 all_violations.append(&mut v);
 }

 let filtered: Vec<&StyleViolation> = all_violations
 .iter()
 .filter(|v| match severity_filter.as_str() {
 "t3" => v.severity == StyleSeverity::Warn,
 "t4" => v.severity == StyleSeverity::Info,
 _ => true,
 })
 .collect();

 if json {
 let view: Vec<_> = filtered
 .iter()
 .map(|v| {
  serde_json::json!({
  "rule_id": v.rule_id,
  "doc_path": v.doc_path,
  "section_id": v.section_id,
  "line_anchor": v.line_anchor,
  "severity": match v.severity {
  StyleSeverity::Warn => "warn",
  StyleSeverity::Info => "info",
  },
  "message": v.message,
  "suggested_fix": v.suggested_fix,
  })
 })
 .collect();
 println!("{}", serde_json::to_string_pretty(&view)?);
 } else {
 println!("=== mnemosyne-cli style-check ===");
 let t3 = filtered
 .iter()
 .filter(|v| v.severity == StyleSeverity::Warn)
 .count();
 let t4 = filtered
 .iter()
 .filter(|v| v.severity == StyleSeverity::Info)
 .count();
 println!("violations: total={} t3_warn={} t4_info={}", filtered.len(), t3, t4);
 let mut per_rule: std::collections::BTreeMap<String, usize> = Default::default();
 for v in &filtered {
 *per_rule.entry(v.rule_id.clone()).or_default() += 1;
 }
 for (rid, count) in &per_rule {
 println!(" {}: {}", rid, count);
 }
 let mut per_doc: std::collections::BTreeMap<String, usize> = Default::default();
 for v in &filtered {
 *per_doc.entry(v.doc_path.clone()).or_default() += 1;
 }
 println!("per-doc:");
 for (doc, count) in &per_doc {
 println!(" {}: {}", doc, count);
 }
 }
 Ok(())
}

// ============================================================================
// validate-code-refs — Stage 2 code-citation defense.
// ============================================================================

/// scan configured code paths for `<entry_id_prefix><digits>`
/// citations and reject those whose target entry_id is missing from the
/// atomic store `changelog_entries` map.
///
/// Stage 1 of the 3-stage defense is the agent-time CLAUDE.md
/// rule; this subcommand is the validator-time gate. wires it
/// into the pre-commit hook; wires the supersede cascade
/// trigger.
///
/// `[code_refs]` omission ⇒ skip (exit 0 with log line) — 5-min setup
/// promise carry for external users who don't cite spec entries in code.
fn cmd_validate_code_refs(args: &[String]) -> Result<()> {
 let mut json = false;
 let mut severity_missing_override: Option<String> = None;
 let mut severity_binding_override: Option<String> = None;
 // explicit decay filter (cascade caller restricts the scan
 // to citations of one entry_id, e.g. an entry that just transitioned
 // Active → Superseded).
 let mut filter_id: Option<String> = None;
 let mut iter = args.iter();
 while let Some(a) = iter.next() {
 match a.as_str() {
 "--json" => json = true,
 "--severity-missing" => {
 severity_missing_override = Some(
 iter.next()
 .ok_or_else(|| anyhow!("--severity-missing missing value"))?
 .clone(),
 );
 }
 "--severity-binding" => {
 severity_binding_override = Some(
 iter.next()
 .ok_or_else(|| anyhow!("--severity-binding missing value"))?
 .clone(),
 );
 }
 "--filter-id" => {
 filter_id = Some(
 iter.next()
 .ok_or_else(|| anyhow!("--filter-id missing value"))?
 .clone(),
 );
 }
 other => bail!("unknown flag `{}`", other),
 }
 }

 let loaded = workspace_config()?;
 let cfg = match &loaded.config.code_refs {
 Some(c) => c,
 None => {
 if json {
 println!(
 "{}",
 serde_json::json!({
 "primitive": "validate-code-refs",
 "status": "skipped",
 "reason": "[code_refs] not configured in mnemosyne.toml",
 })
 );
 } else {
 println!("=== mnemosyne-cli validate-code-refs ===");
 println!(
 "skipped — [code_refs] not configured in mnemosyne.toml \
 (5-min setup promise carry — Round 256)"
 );
 }
 return Ok(());
 }
 };

 let severity_missing = severity_missing_override
 .as_deref()
 .unwrap_or(&cfg.severity_missing)
 .to_string();
 if !matches!(severity_missing.as_str(), "reject" | "warn" | "info") {
 bail!(
 "invalid --severity-missing `{}` — expected one of: reject | warn | info",
 severity_missing
 );
 }
 let severity_binding = severity_binding_override
 .as_deref()
 .unwrap_or(&cfg.severity_binding)
 .to_string();
 if !matches!(severity_binding.as_str(), "reject" | "warn" | "info") {
 bail!(
 "invalid --severity-binding `{}` — expected one of: reject | warn | info",
 severity_binding
 );
 }

 let prefix = cli_schema()?.entry_id_prefix.clone();
 let root = loaded.workspace_root.clone();

 let atomic_path = AtomicStore::default_sidecar_path(&root);
 let store = AtomicStore::load(&atomic_path)
 .with_context(|| format!("atomic store load: {}", atomic_path.display()))?;

 let violations = scan_paths_bidirectional(
 &root,
 &cfg.paths,
 &prefix,
 &store,
 &loaded.config.orphan_ledger,
 filter_id.as_deref(),
 cfg.comment_only,
 )
 .context("scan_paths_bidirectional failed")?;

 let mut counts = [0usize; 5]; // missing / section_missing / citation_unbound / impl_unbacked / decay
 for v in &violations {
 match v {
 CodeRefViolation::Citation { kind, .. } => match kind {
 ViolationKind::Missing => counts[0] += 1,
 ViolationKind::SectionMissing => counts[1] += 1,
 ViolationKind::CitationUnbound => counts[2] += 1,
 ViolationKind::Decay => counts[4] += 1,
 },
 CodeRefViolation::ImplementationUnbacked { .. } => counts[3] += 1,
 }
 }
 let [missing_count, section_missing_count, citation_unbound_count, impl_unbacked_count, decay_count] =
 counts;
 let hallucination_count = missing_count + section_missing_count;
 let binding_count = citation_unbound_count + impl_unbacked_count;

 if json {
 let view: Vec<_> = violations
 .iter()
 .map(|v| match v {
 CodeRefViolation::Citation { citation, .. } => serde_json::json!({
 "kind": v.kind_tag(),
 "file": citation.file.to_string_lossy(),
 "line": citation.line,
 "entry_id": citation.entry_id,
 }),
 CodeRefViolation::ImplementationUnbacked {
 section_id,
 file,
 symbol,
 } => serde_json::json!({
 "kind": v.kind_tag(),
 "file": file.to_string_lossy(),
 "section_id": section_id,
 "symbol": symbol,
 }),
 })
 .collect();
 let valid_entry_count = store.changelog_entries.len();
 println!(
 "{}",
 serde_json::json!({
 "primitive": "validate-code-refs",
 "scanned_paths": cfg.paths,
 "valid_entry_count": valid_entry_count,
 "valid_section_count": store.sections.len(),
 "missing_count": missing_count,
 "section_missing_count": section_missing_count,
 "citation_unbound_count": citation_unbound_count,
 "impl_unbacked_count": impl_unbacked_count,
 "decay_count": decay_count,
 "severity_missing": severity_missing,
 "severity_binding": severity_binding,
 "filter_id": filter_id,
 "violations": view,
 })
 );
 } else {
 println!("=== mnemosyne-cli validate-code-refs ===");
 println!(
 "prefix={:?} valid_entries={} valid_sections={} scanned_paths={:?}",
 prefix,
 store.changelog_entries.len(),
 store.sections.len(),
 cfg.paths
 );
 if let Some(ref fid) = filter_id {
 println!("filter_id={:?} (Round 258 decay scan mode)", fid);
 }
 println!(
 "violations: total={} missing={} section_missing={} \
 citation_unbound={} impl_unbacked={} decay={} \
 (severity_missing={} severity_binding={})",
 violations.len(),
 missing_count,
 section_missing_count,
 citation_unbound_count,
 impl_unbacked_count,
 decay_count,
 severity_missing,
 severity_binding,
 );
 for v in &violations {
 match v {
 CodeRefViolation::Citation { citation, .. } => println!(
 " [{}] {}:{} {}",
 v.kind_tag(),
 citation.file.to_string_lossy(),
 citation.line,
 citation.entry_id,
 ),
 CodeRefViolation::ImplementationUnbacked {
 section_id,
 file,
 symbol,
 } => println!(
 " [{}] {}:<no-cite> §{}{}",
 v.kind_tag(),
 file.to_string_lossy(),
 section_id,
 symbol
 .as_deref()
 .map(|s| format!(" ({})", s))
 .unwrap_or_default(),
 ),
 }
 }
 }

 // Reject gates by defect class — each class gated by its
 // own severity flag. Decay never rejects (informational).
 let mut reject_msgs: Vec<String> = Vec::new();
 if hallucination_count > 0 && severity_missing == "reject" {
 reject_msgs.push(format!(
 "{} hallucination-class citation(s) — Missing={} SectionMissing={} \
 (severity_missing=reject)",
 hallucination_count, missing_count, section_missing_count,
 ));
 }
 if binding_count > 0 && severity_binding == "reject" {
 reject_msgs.push(format!(
 "{} binding-class violation(s) — CitationUnbound={} ImplementationUnbacked={} \
 (severity_binding=reject)",
 binding_count, citation_unbound_count, impl_unbacked_count,
 ));
 }
 if !reject_msgs.is_empty() {
 bail!("{}", reject_msgs.join("; "));
 }
 Ok(())
}
