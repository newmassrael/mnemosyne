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

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::{Arc, OnceLock};
use std::{env, fs};

use anyhow::{anyhow, bail, Context, Result};
use sha2::{Digest, Sha256};

use mnemosyne_atomic::AtomicStore;
use mnemosyne_config::{discover_config, LoadedConfig, OrphanKind, SchemaSection, WorkspaceConfig};
use mnemosyne_parser::{compare_typed_facts, emit_markdown_with_default, parse_markdown_with_schema};
use mnemosyne_query::{
    build_envelope, changelog_entries_for_section, query_term, related_sections_with_atomic,
    section_by_id, workspace_section_id_set, TermMode, TermQuery, TermScope,
};
use mnemosyne_schema::ParsedDoc;
use mnemosyne_server::{MnemosyneServer, Proposal, ProposalKind};
use mnemosyne_store::MnemosyneStore;
use mnemosyne_style::{check_style, default_ruleset_with_config, StyleSeverity, StyleViolation};
use mnemosyne_validate::{
    code_refs::SetEqualityValidator, validator::cross_ref_orphan_reject_with_workspace,
    ValidationError,
};
use mnemosyne_workspace::Workspace;

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

/// Round 306 — build the SymbolResolver registry from
/// `[plugins.symbol_resolver.<lang>]` config entries.
///
/// Only `transport = "in-process"` entries land production backends in
/// R306. `transport = "mcp"` / `"cli"` parse cleanly (the config enum
/// has those variants per RFC-003 transport-abstraction section) but their backends are not yet
/// wired — callers reaching those variants surface `NotImplemented` at
/// resolve time. Unknown in-process backend names log a stderr warning
/// and are skipped (no plugin = file-only set-equality, no language
/// blocked).
fn build_symbol_resolver_map(
 cfg: &WorkspaceConfig,
) -> std::collections::BTreeMap<String, Box<dyn mnemosyne_core::SymbolResolver>> {
 use mnemosyne_config::SymbolResolverConfig;
 let mut out: std::collections::BTreeMap<String, Box<dyn mnemosyne_core::SymbolResolver>> =
 std::collections::BTreeMap::new();
 let Some(plugins) = cfg.plugins.as_ref() else {
 return out;
 };
 for (lang, resolver_cfg) in &plugins.symbol_resolver {
 match resolver_cfg {
 SymbolResolverConfig::InProcess { backend } => {
 if backend == mnemosyne_plugin_tree_sitter_rust::BACKEND_KEY {
 out.insert(
  lang.clone(),
  Box::new(mnemosyne_plugin_tree_sitter_rust::TreesitterRustResolver),
 );
 } else {
 eprintln!(
  "[plugins.symbol_resolver.{}] unknown in-process backend `{}` — skipped",
  lang, backend
 );
 }
 }
 SymbolResolverConfig::Mcp { command } => {
 // Placeholder McpResolver — registered into the type surface so
 // enforcement passes the call through; resolve_symbol_at returns
 // ResolverError::NotImplemented until R307+ wires real MCP transport.
 out.insert(
  lang.clone(),
  Box::new(mnemosyne_core::McpResolver {
  command: command.clone(),
  }),
 );
 }
 SymbolResolverConfig::Cli {
 command,
 output_parser,
 } => {
 // Placeholder CliResolver — same NotImplemented behavior as McpResolver
 // until R307+ wires shell-out + output_parser.
 out.insert(
  lang.clone(),
  Box::new(mnemosyne_core::CliResolver {
  command: command.clone(),
  output_parser: output_parser.clone(),
  }),
 );
 }
 }
 }
 out
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
 "usage: {} <validate|validate-workspace|commit|query|add-section|style-check|list-docs|set-section-intent|set-section-rationale|set-section-inputs|set-section-outputs|set-section-title|set-section-parent-doc|set-section-parent-section|add-section-caveat|set-section-alternatives|set-section-impact-scope|add-section-example|add-section-implementation|remove-section-implementation|set-section-decision-status|set-section-normative-excerpt|remove-section|append-changelog-entry|set-changelog-publishable-decision-summary|set-changelog-publishable-changes|set-changelog-publishable-verification|set-changelog-publishable-impact-refs|set-changelog-publishable-carry-forward|redact-term|emit-publishable-override-ledger-draft|add-inventory-entry|set-inventory-status|set-inventory-section-ref|remove-inventory-entry|generate-docs|verify-generated> [args...]",
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
 "add-section" => atomic_cli::cmd_add_section(&repo_root()?, &args[2..]),
 "style-check" => cmd_style_check(prog, &args[2..]),
 "list-docs" => cmd_list_docs(),
 // atomic mutate API surface.
 "set-section-intent" => atomic_cli::cmd_set_section_intent(&repo_root()?, &args[2..]),
 "set-section-rationale" => atomic_cli::cmd_set_section_rationale(&repo_root()?, &args[2..]),
 "set-section-inputs" => atomic_cli::cmd_set_section_inputs(&repo_root()?, &args[2..]),
 "set-section-outputs" => atomic_cli::cmd_set_section_outputs(&repo_root()?, &args[2..]),
 // Round 287 — outline setter surface (Phase C).
 "set-section-title" => atomic_cli::cmd_set_section_title(&repo_root()?, &args[2..]),
 "set-section-parent-doc" => {
 atomic_cli::cmd_set_section_parent_doc(&repo_root()?, &args[2..])
 }
 "set-section-parent-section" => {
 atomic_cli::cmd_set_section_parent_section(&repo_root()?, &args[2..])
 }
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
 // Round 283 — Section.implementations remove primitive (set-element granularity).
 "remove-section-implementation" => {
 atomic_cli::cmd_remove_section_implementation(&repo_root()?, &args[2..])
 }
 // Round 265 — Stage B freshness substrate. (Round 304 — _atomic suffix
 // dropped; legacy markdown-surgical variant retired with the rest of
 // `mutate.rs`.)
 "set-section-decision-status" => {
 atomic_cli::cmd_set_section_decision_status(&repo_root()?, &args[2..])
 }
 "set-section-normative-excerpt" => {
 atomic_cli::cmd_set_section_normative_excerpt(&repo_root()?, &args[2..])
 }
 // Round 267 — section removal (closes Round 266 carry gap).
 "remove-section" => atomic_cli::cmd_remove_section(&repo_root()?, &args[2..]),
 "append-changelog-entry" => {
 atomic_cli::cmd_append_changelog_entry(&repo_root()?, &args[2..])
 }
 // Round 295 — publishable-half setters (audit half stays frozen).
 "set-changelog-publishable-decision-summary" => {
 atomic_cli::cmd_set_changelog_publishable_decision_summary(
 &repo_root()?,
 &args[2..],
 )
 }
 "set-changelog-publishable-changes" => {
 atomic_cli::cmd_set_changelog_publishable_changes(&repo_root()?, &args[2..])
 }
 "set-changelog-publishable-verification" => {
 atomic_cli::cmd_set_changelog_publishable_verification(
 &repo_root()?,
 &args[2..],
 )
 }
 "set-changelog-publishable-impact-refs" => {
 atomic_cli::cmd_set_changelog_publishable_impact_refs(
 &repo_root()?,
 &args[2..],
 )
 }
 "set-changelog-publishable-carry-forward" => {
 atomic_cli::cmd_set_changelog_publishable_carry_forward(
 &repo_root()?,
 &args[2..],
 )
 }
 // Round 297 — RFC P1 redact_term convenience primitive.
 "redact-term" => atomic_cli::cmd_redact_term(&repo_root()?, &args[2..]),
 // Round 300 — bare-setter ledger draft companion.
 "emit-publishable-override-ledger-draft" => {
 atomic_cli::cmd_emit_publishable_override_ledger_draft(
 &repo_root()?,
 &args[2..],
 )
 }
 // Round 274 — Phase 1A inventory mutate primitives.
 "add-inventory-entry" => {
 atomic_cli::cmd_add_inventory_entry(&repo_root()?, &args[2..])
 }
 "set-inventory-status" => {
 atomic_cli::cmd_set_inventory_status(&repo_root()?, &args[2..])
 }
 "set-inventory-section-ref" => {
 atomic_cli::cmd_set_inventory_section_ref(&repo_root()?, &args[2..])
 }
 "remove-inventory-entry" => {
 atomic_cli::cmd_remove_inventory_entry(&repo_root()?, &args[2..])
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
 "--version" | "-V" | "version" => {
 println!(
 "mnemosyne-cli {} ({})",
 env!("CARGO_PKG_VERSION"),
 env!("BUILD_GIT_HASH")
 );
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
 println!(
 "mnemosyne-cli {} ({}) — Phase 0 design_doc lifecycle (DESIGN §66)",
 env!("CARGO_PKG_VERSION"),
 env!("BUILD_GIT_HASH")
 );
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
 " {} query --list-inventory [--json] Phase 1A inventory entries (Round 278)",
 prog
 );
 println!(
 " {} query --inventory <ID> [--json] single inventory entry lookup",
 prog
 );
 println!(
 " {} query --term <pattern> [--regex] [--case-insensitive|-i] [--scope all|sections|changelog|inventory] [--field name,name,...] [--json]",
 prog
 );
 println!(
 "   Round 292 — literal/regex search across atomic Section + ChangelogEntry + Inventory fields (preview for redact_term carry)"
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
 println!(" Field length caps (Round 161 §41 thresholds, surfaced for DX — Round 279 carry):");
 println!("   intent: max 200 chars; each bullet (rationale/inputs/outputs/caveats): max 100 chars");
 println!(" {} add-section --section §<id> --parent-doc <doc-id> --title <text> [--parent §<P>] [--sidecar <path>] [--json]", prog);
 println!("   pairs with remove-section (R267); content fields populate via set-section-* afterwards");
 println!(" {} set-section-intent --section §<N> --intent <text (max 200 chars)> [--sidecar <path>] [--json]", prog);
 println!(" {} set-section-rationale --section §<N> --bullets-file <path (each bullet ≤ 100 chars)> [--sidecar <path>] [--json]", prog);
 println!(" {} set-section-inputs --section §<N> --bullets-file <path (each bullet ≤ 100 chars)> [--sidecar <path>] [--json]", prog);
 println!(" {} set-section-outputs --section §<N> --bullets-file <path (each bullet ≤ 100 chars)> [--sidecar <path>] [--json]", prog);
 println!(" {} set-section-title --section §<N> --title <heading text> [--sidecar <path>] [--json]", prog);
 println!(" {} set-section-parent-doc --section §<N> --parent-doc <doc-id> [--sidecar <path>] [--json]", prog);
 println!(" {} set-section-parent-section --section §<N> (--parent §<P> | --no-parent) [--sidecar <path>] [--json]", prog);
 println!(" {} add-section-caveat --section §<N> --bullet <text (max 100 chars)> [--sidecar <path>] [--json]", prog);
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
 " {} remove-section-implementation --section §<N> --file <path> [--symbol <name>] --reason <text> [--sidecar <path>] [--json]",
 prog
 );
 println!(
 "   Round 283 Section.implementations remove primitive (exact (file, symbol) match; --reason mandatory)"
 );
 println!(
 " {} set-section-normative-excerpt --section §<N> --text-file <path> --anchor-url <url> --source-revision <rev> [--sidecar <path>] [--json]",
 prog
 );
 println!(
 "   external-spec mirror anchor — vendored quote + URL + rev; frozen after first set (model spec rev drift by superseding the Section)"
 );
 println!(
 " {} set-section-decision-status --section §<N> --status active|superseded|removed [--superseding §<M>] [--sidecar <path>] [--json]",
 prog
 );
 println!(
 "   atomic decision_status setter (Stage B freshness substrate); --superseding required for --status superseded (T1 rule 4 atomic axis)"
 );
 println!(
 " {} remove-section --section §<N> --reason <text> [--sidecar <path>] [--json]",
 prog
 );
 println!(
 "   Round 267 section removal (audit-safeguarded; closes Round 266 carry)"
 );
 println!(
 " {} append-changelog-entry --entry-id \"Round N\" --decision <text> --changes-file <path> --verification-file <path> --impact §A,§B --carry-file <path> [--sidecar <path>] [--json]",
 prog
 );
 println!();
 println!(" --- Phase 1A inventory mutate API (Round 274) ---");
 println!(
 " {} add-inventory-entry --id <ID> --status active|deprecated|reserved [--section §<N>] [--source <text>] [--reason <text>] [--sidecar <path>] [--json]",
 prog
 );
 println!(
 " {} set-inventory-status --id <ID> --status active|deprecated|reserved [--reason <text>] [--sidecar <path>] [--json]",
 prog
 );
 println!(
 " {} set-inventory-section-ref --id <ID> (--section §<N> | --clear) [--sidecar <path>] [--json]",
 prog
 );
 println!(
 " {} remove-inventory-entry --id <ID> --reason <text> [--sidecar <path>] [--json]",
 prog
 );
 println!(
 "   Round 273 InventoryEntry 5번째 closed-form 엔티티 substrate; cite-time reject (R275) + cascade (R276) carry"
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
 "   Round 256: scan [plugins.set_equality_validator].paths for <entry_id_prefix><digits> citations,"
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
 println!();
 println!(" --- meta (Round 286) ---");
 println!(" {} --version | -V | version  print binary version + build hash", prog);
 println!(" {} --help | -h | help   print this help text", prog);
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
 let orphans = mnemosyne_validate::validator::cross_ref_orphan_reject(&parsed);

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
 // Round 278 — Phase 1A inventory query surface.
 list_inventory: bool,
 inventory_id: Option<String>,
 // Round 292 — query_term primitive (literal/regex search).
 term_pattern: Option<String>,
 term_regex: bool,
 term_case_insensitive: bool,
 term_scope: Option<String>,
 term_fields: Vec<String>,
}

fn parse_query_args(args: &[String]) -> Result<QueryArgs> {
 let mut out = QueryArgs::default();
 let mut iter = args.iter();
 while let Some(arg) = iter.next() {
 match arg.as_str() {
 "--include-related" => out.include_related = true,
 "--include-changelog" => out.include_changelog = true,
 "--json" => out.json = true,
 "--list-sections" => out.list_sections = true,
 "--list-inventory" => out.list_inventory = true,
 "--inventory" => {
 out.inventory_id = Some(
  iter.next()
  .ok_or_else(|| anyhow!("--inventory missing value"))?
  .clone(),
 );
 }
 "--term" => {
 out.term_pattern = Some(
  iter.next()
  .ok_or_else(|| anyhow!("--term missing pattern"))?
  .clone(),
 );
 }
 "--regex" => out.term_regex = true,
 "--case-insensitive" | "-i" => out.term_case_insensitive = true,
 "--scope" => {
 out.term_scope = Some(
  iter.next()
  .ok_or_else(|| anyhow!("--scope missing value (all|sections|changelog|inventory)"))?
  .clone(),
 );
 }
 "--field" => {
 let v = iter
 .next()
 .ok_or_else(|| anyhow!("--field missing value (comma-separated field names)"))?;
 for name in v.split(',') {
  let trimmed = name.trim();
  if !trimmed.is_empty() {
  out.term_fields.push(trimmed.to_string());
  }
 }
 }
 other if other.starts_with("--") => bail!("unknown flag `{}`", other),
 other => {
  if let Some(existing) = &out.section_id {
  bail!("section_id argument duplicate (already `{}`)", existing);
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
 AtomicStore::load(&atomic_cli::resolve_sidecar(&root, None)).unwrap_or_default();

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

 // Round 278 — Phase 1A inventory query surface.
 if qargs.list_inventory {
 if qargs.json {
 let view: Vec<_> = atomic_store
 .inventory_entries
 .iter()
 .map(|(id, e)| {
  serde_json::json!({
  "id": id,
  "status": e.status,
  "section_ref": e.section_ref,
  "source": e.source,
  "reason": e.reason,
  })
 })
 .collect();
 println!("{}", serde_json::to_string_pretty(&view)?);
 } else {
 for (id, entry) in &atomic_store.inventory_entries {
 let status_label = match entry.status {
  mnemosyne_core::InventoryStatus::Active => "active",
  mnemosyne_core::InventoryStatus::Deprecated => "deprecated",
  mnemosyne_core::InventoryStatus::Reserved => "reserved",
 };
 let section_part = entry
  .section_ref
  .as_deref()
  .map(|s| format!(" §{}", s))
  .unwrap_or_default();
 println!("{}\t{}{}", id, status_label, section_part);
 }
 eprintln!(
 "# total {} inventory entry(ies)",
 atomic_store.inventory_entries.len()
 );
 }
 return Ok(());
 }
 if let Some(inv_id) = qargs.inventory_id {
 let entry = atomic_store.inventory(&inv_id).ok_or_else(|| {
 anyhow!("inventory_id `{}` not present in atomic store", inv_id)
 })?;
 if qargs.json {
 let view = serde_json::json!({
 "id": inv_id,
 "status": entry.status,
 "section_ref": entry.section_ref,
 "source": entry.source,
 "reason": entry.reason,
 });
 println!("{}", serde_json::to_string_pretty(&view)?);
 } else {
 let status_label = match entry.status {
 mnemosyne_core::InventoryStatus::Active => "active",
 mnemosyne_core::InventoryStatus::Deprecated => "deprecated",
 mnemosyne_core::InventoryStatus::Reserved => "reserved",
 };
 println!("inventory_id: {}", inv_id);
 println!("status: {}", status_label);
 if let Some(s) = entry.section_ref.as_deref() {
 println!("section_ref: §{}", s);
 }
 if let Some(s) = entry.source.as_deref() {
 println!("source: {}", s);
 }
 if let Some(s) = entry.reason.as_deref() {
 println!("reason: {}", s);
 }
 }
 return Ok(());
 }

 // Round 292 — query_term primitive (literal/regex search across
 // atomic Section + ChangelogEntry + Inventory fields).
 if let Some(pattern) = qargs.term_pattern.as_deref() {
 let scope = match qargs.term_scope.as_deref().unwrap_or("all") {
 "all" => TermScope::All,
 "sections" => TermScope::Sections,
 "changelog" | "changelog-entries" => TermScope::ChangelogEntries,
 "inventory" => TermScope::Inventory,
 other => bail!(
 "--scope must be one of all|sections|changelog|inventory (got `{}`)",
 other
 ),
 };
 let field_filter = if qargs.term_fields.is_empty() {
 None
 } else {
 let set: BTreeSet<String> = qargs.term_fields.iter().cloned().collect();
 Some(set)
 };
 let q = TermQuery {
 pattern: pattern.to_string(),
 mode: if qargs.term_regex {
 TermMode::Regex
 } else {
 TermMode::Literal
 },
 case_insensitive: qargs.term_case_insensitive,
 scope,
 field_filter,
 };
 let hits = query_term(&atomic_store, &q)
 .with_context(|| format!("query_term failed (pattern=`{}`)", pattern))?;
 if qargs.json {
 println!("{}", serde_json::to_string_pretty(&hits)?);
 } else {
 for hit in &hits {
 let kind = match hit.target_kind {
 mnemosyne_query::TermTargetKind::Section => "section",
 mnemosyne_query::TermTargetKind::ChangelogEntry => "entry",
 mnemosyne_query::TermTargetKind::Inventory => "inventory",
 };
 println!(
 "{}\t{}\t{}\t{}",
 kind, hit.target_id, hit.field_path, hit.line_context
 );
 }
 eprintln!("# {} hit(s)", hits.len());
 }
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
 if let Some(spec) = &workspace_config()?.config.workspace.spec_source {
 println!(
 "spec_source: url={} revision={} sha256={} fetched_at={}",
 spec.url,
 spec.revision,
 spec.fetched_sha256.as_deref().unwrap_or("-"),
 spec.fetched_at.as_deref().unwrap_or("-"),
 );
 }
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
 AtomicStore::load(&atomic_cli::resolve_sidecar(&root, None)).unwrap_or_default();
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
 OrphanKind::InventoryCitation => {} // Round 285 — inventory-axis ledger handled by validate-code-refs (axis-symmetric with CodeCitation; set-equality drift detection for both is R286+ carry)
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

 // T1 rule 4 (atomic axis) state gate. Walks the atomic store for sections
 // where decision_status=Some(Superseded) and verifies a superseding cross-
 // ref exists from that section in any parsed doc. State-based, post-
 // condition — complements the parser-pair transition check by catching
 // atomic-only overrides that no markdown prev/curr snapshot would expose.
 // Some(Removed) is tombstone-exempt (asserts finality, not replacement).
 let parsed_docs_refs: Vec<&mnemosyne_schema::ParsedDoc> =
 parsed_docs.iter().map(|(_, doc)| doc).collect();
 let atomic_supersede_errors = mnemosyne_validate::atomic_section_supersede_state_reject(
 &validate_workspace_atomic,
 &parsed_docs_refs,
 );
 if !atomic_supersede_errors.is_empty() {
 for err in &atomic_supersede_errors {
 if let mnemosyne_validate::ValidationError::SupersedeMissingRef {
 section_id, ..
 } = err
 {
 eprintln!(
  "T1 rule 4 (atomic axis): §{} decision_status=Superseded but no \
   superseding cross-ref (Decision|Impl) found from this section",
  section_id
 );
 }
 }
 bail!(
 "T1 rule 4 (atomic axis): {} section(s) marked Superseded without \
  superseding cross-ref — embed §<superseding-id> in the section's \
  intent/rationale/impact_scope via the matching atomic setter, or \
  revert to Active|Removed",
 atomic_supersede_errors.len()
 );
 }

 // workspace-wide cascade decay surface.
 //
 // Iterates atomic_store.sections for decision_status=Some(Superseded|Removed)
 // and runs scan_section_decay per section. Sums citing locations across
 // [plugins.set_equality_validator].paths and prints one informational line. Never fails the
 // gate — matches the Round 266 mutate-time trigger's informational-only
 // semantics. Silent when [plugins.set_equality_validator] is unconfigured.
 print_atomic_decay_surface(&root)?;

 // Round 296 — publishable / audit divergence ledger gate.
 //
 // Walks atomic.changelog_entries; for each entry where
 // `publishable_matches_audit() == false`, requires a matching
 // `[[publishable_override_ledger]]` row whose `target_id` equals the
 // entry id and whose `content_hash_after` equals the current publishable
 // hash. Missing or stale rows reject the workspace. The hash makes the
 // ledger forge-resistant: editing publishable_* without re-anchoring the
 // ledger row re-surfaces the rejection.
 check_publishable_override_ledger(
 &validate_workspace_atomic,
 &validate_workspace_cfg_for_ledger.config.publishable_override_ledger,
 )?;

 // Round 293 — commit↔ledger drift surface.
 //
 // Walks the last 200 git commit subjects, extracts "(R<N>)" / "(Round <N>)"
 // round labels, diffs against round_numbers parsed from atomic ledger
 // entry-id keys ("Round NNN — ..."). missing = cited in commit but absent
 // from ledger (audit-trail hole — R291 was the trigger). Prints one
 // informational line; never fails the gate. Silent when not in a git repo
 // or when no labeled commits found in the scan window.
 print_commit_ledger_drift_surface(&root, &validate_workspace_atomic)?;

 Ok(())
}

/// Round 296 — publishable / audit divergence ledger gate.
///
/// Walks `atomic.changelog_entries`. For each entry where
/// `publishable_matches_audit() == false`, requires a matching
/// `[[publishable_override_ledger]]` row whose `target_id` equals the entry
/// id and whose `content_hash_after` equals the current publishable hash.
/// Pure-ledger carry passes (rows whose target entry no longer diverges,
/// e.g. because publishable_* was reverted to audit_*, are silently
/// inert; that is the correct behavior — drift surfaces only on
/// divergence, not on extra ledger rows).
///
/// Prints one informational line summarizing divergence count and
/// ledger-row count regardless of pass/fail. Bails on first reject.
fn check_publishable_override_ledger(
 atomic: &mnemosyne_atomic::AtomicStore,
 ledger: &[mnemosyne_config::PublishableOverrideLedgerEntry],
) -> Result<()> {
 let divergent_entries: Vec<(&String, &mnemosyne_atomic::AtomicChangelogEntry)> = atomic
 .changelog_entries
 .iter()
 .filter(|(_, e)| !e.publishable_matches_audit())
 .collect();
 println!(
 "publishable / audit divergence: entries={} ledger_rows={}",
 divergent_entries.len(),
 ledger.len()
 );
 if divergent_entries.is_empty() {
 // pure-ledger carry note (informational): rows for entries that no
 // longer diverge are inert — surfaced once so authors can prune.
 let inert: Vec<&str> = ledger
 .iter()
 .filter(|row| {
 atomic
 .changelog_entries
 .get(&row.target_id)
 .map(|e| e.publishable_matches_audit())
 .unwrap_or(true)
 })
 .map(|row| row.target_id.as_str())
 .collect();
 if !inert.is_empty() {
 println!(
 " inert ledger rows ({}): {}",
 inert.len(),
 inert.join(", ")
 );
 }
 return Ok(());
 }
 let mut errors: Vec<String> = Vec::new();
 for (entry_id, entry) in &divergent_entries {
 let current_hash = entry.publishable_hash_hex();
 let matched = ledger.iter().any(|row| {
 row.target_id == **entry_id && row.content_hash_after == current_hash
 });
 if !matched {
 errors.push(format!(
 "  diverged `{}` — current publishable_hash={} (no matching \
  [[publishable_override_ledger]] row)",
 entry_id, current_hash
 ));
 }
 }
 if !errors.is_empty() {
 for e in &errors {
 eprintln!("{}", e);
 }
 bail!(
 "publishable / audit divergence on {} entry(ies) without matching \
  [[publishable_override_ledger]] row — add a row with target_id, \
  reason, applied_in, and content_hash_after = the printed publishable_hash, \
  or revert publishable_* to audit_* (Round 296 body-split gate)",
 errors.len()
 );
 }
 Ok(())
}

/// Round 268 — workspace decay surface report.
///
/// Reads the atomic store, walks all sections with
/// `decision_status = Some(Superseded | Removed)`, and runs
/// `scan_section_decay` against the configured `[plugins.set_equality_validator].paths`. Prints
/// a one-line summary plus a per-section break-down when any decay surfaces.
/// Pure informational — does not affect the validate-workspace exit code.
fn print_atomic_decay_surface(root: &std::path::Path) -> Result<()> {
 let cfg = match workspace_config() {
 Ok(c) => c,
 Err(_) => return Ok(()),
 };
 let code_refs_cfg = match cfg
 .config
 .plugins
 .as_ref()
 .and_then(|p| p.set_equality_validator.as_ref())
 {
 Some(c) if !c.paths.is_empty() => c,
 _ => return Ok(()),
 };
 let store = match mnemosyne_atomic::AtomicStore::load(
 &atomic_cli::resolve_sidecar(root, None),
 ) {
 Ok(s) => s,
 Err(_) => return Ok(()),
 };
 let mut targets: Vec<&str> = Vec::new();
 for (section_id, section) in &store.sections {
 if matches!(
 section.decision_status,
 Some(mnemosyne_core::DecisionStatus::Superseded)
 | Some(mnemosyne_core::DecisionStatus::Removed)
 ) {
 targets.push(section_id.as_str());
 }
 }
 if targets.is_empty() {
 return Ok(());
 }
 let mut total = 0usize;
 let mut per_section: Vec<(&str, usize)> = Vec::new();
 for sid in &targets {
 let hits = mnemosyne_validate::code_refs::scan_section_decay(
 root,
 &code_refs_cfg.paths,
 sid,
 code_refs_cfg.comment_only,
 )
 .unwrap_or_default();
 if !hits.is_empty() {
 per_section.push((sid, hits.len()));
 }
 total += hits.len();
 }
 println!(
 "atomic decay surface: {} citation(s) across {} superseded/removed section(s)",
 total,
 targets.len()
 );
 for (sid, n) in &per_section {
 println!(" §{}: {} citation(s)", sid, n);
 }
 Ok(())
}

/// Round 293 — commit↔ledger drift surface. Round 301 — `missing > 0`
/// promoted from warn-only to hard reject.
///
/// Walks the last `MAX_COMMIT_SCAN` git commit subjects, extracts round
/// labels via the project commit convention `(R<N>)` / `(Round <N>)`, and
/// diffs against round_numbers parsed from atomic ledger entry-id keys
/// (`Round NNN — ...`).
///
/// `missing` (cited in commit but absent from ledger) is the audit-trail
/// hole catch — R291 was the trigger (commit `76581f6` landed without an
/// atomic-store entry between R290 and R292; backfilled in R293). Under
/// R301 the gate refuses to pass when any cited round has no atomic-store
/// entry; the fix is to backfill the entry, not silence the gate.
///
/// Silent when not in a git repo, when git is missing, or when no labeled
/// commits exist in the scan window.
const MAX_COMMIT_SCAN: usize = 200;

fn print_commit_ledger_drift_surface(
 root: &std::path::Path,
 atomic: &mnemosyne_atomic::AtomicStore,
) -> Result<()> {
 let cited = collect_recent_commit_round_labels(root, MAX_COMMIT_SCAN);
 if cited.is_empty() {
 return Ok(());
 }
 let ledger = collect_ledger_round_numbers(atomic);
 let report = mnemosyne_validate::commit_ledger_diff(&cited, &ledger);
 println!(
 "commit↔ledger drift: cited={} / ledger={} / missing={} (last {} commits scanned)",
 report.cited_count,
 report.ledger_count,
 report.missing.len(),
 MAX_COMMIT_SCAN,
 );
 if !report.missing.is_empty() {
 for n in &report.missing {
 println!(
 "  missing R{} — commit subject cites this round but no atomic-store entry exists",
 n
 );
 }
 println!(
 "  hint: backfill via `mnemosyne-cli append-changelog-entry --entry-id \"Round <N> — ...\" \
  --decision <text> --changes-file <path> --verification-file <path> --impact §A,§B \
  --carry-file <path>` (Round 293 backfill flow)"
 );
 // Round 301 — hard reject. The line + per-round missing prints
 // above remain so the diagnostic is preserved before the bail.
 bail!(
 "commit↔ledger drift gate: {} cited round(s) missing from atomic store (Round 301)",
 report.missing.len()
 );
 }
 Ok(())
}

fn collect_recent_commit_round_labels(
 root: &std::path::Path,
 max_commits: usize,
) -> BTreeSet<u32> {
 let output = std::process::Command::new("git")
 .args([
 "log",
 &format!("--max-count={}", max_commits),
 "--pretty=%s",
 ])
 .current_dir(root)
 .output();
 let output = match output {
 Ok(o) if o.status.success() => o,
 _ => return BTreeSet::new(),
 };
 let text = String::from_utf8_lossy(&output.stdout);
 // matches "(R293)" and "(Round 288)" — both forms appear in project history.
 let re = match regex::Regex::new(r"\((?:R|Round )(\d+)\)") {
 Ok(r) => r,
 Err(_) => return BTreeSet::new(),
 };
 let mut set: BTreeSet<u32> = BTreeSet::new();
 for line in text.lines() {
 for cap in re.captures_iter(line) {
 if let Ok(n) = cap[1].parse::<u32>() {
 set.insert(n);
 }
 }
 }
 set
}

fn collect_ledger_round_numbers(
 atomic: &mnemosyne_atomic::AtomicStore,
) -> BTreeSet<u32> {
 let re = match regex::Regex::new(r"^Round (\d+)") {
 Ok(r) => r,
 Err(_) => return BTreeSet::new(),
 };
 let mut set: BTreeSet<u32> = BTreeSet::new();
 for key in atomic.changelog_entries.keys() {
 if let Some(cap) = re.captures(key) {
 if let Ok(n) = cap[1].parse::<u32>() {
 set.insert(n);
 }
 }
 }
 set
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
 AtomicStore::load(&atomic_cli::resolve_sidecar(root, None)).unwrap_or_default();
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
 AtomicStore::load(&atomic_cli::resolve_sidecar(&root, None)).unwrap_or_default();
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
/// `[plugins.set_equality_validator]` omission ⇒ skip (exit 0 with log line) — 5-min setup
/// promise carry for external users who don't cite spec entries in code.
fn cmd_validate_code_refs(args: &[String]) -> Result<()> {
 let mut json = false;
 let mut severity_missing_override: Option<String> = None;
 let mut severity_binding_override: Option<String> = None;
 let mut severity_inventory_override: Option<String> = None;
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
 "--severity-inventory" => {
 severity_inventory_override = Some(
 iter.next()
 .ok_or_else(|| anyhow!("--severity-inventory missing value"))?
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
 let cfg = match loaded
 .config
 .plugins
 .as_ref()
 .and_then(|p| p.set_equality_validator.as_ref())
 {
 Some(c) => c,
 None => {
 if json {
 println!(
 "{}",
 serde_json::json!({
 "primitive": "validate-code-refs",
 "status": "skipped",
 "reason": "[plugins.set_equality_validator] not configured in mnemosyne.toml",
 })
 );
 } else {
 println!("=== mnemosyne-cli validate-code-refs ===");
 println!(
 "skipped — [plugins.set_equality_validator] not configured in mnemosyne.toml \
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
 let severity_inventory = severity_inventory_override
 .as_deref()
 .unwrap_or(&cfg.severity_inventory)
 .to_string();
 if !matches!(severity_inventory.as_str(), "reject" | "warn" | "info") {
 bail!(
 "invalid --severity-inventory `{}` — expected one of: reject | warn | info",
 severity_inventory
 );
 }

 let prefix = cli_schema()?.entry_id_prefix.clone();
 let root = loaded.workspace_root.clone();

 let atomic_path = atomic_cli::resolve_sidecar(&root, None);
 let store = AtomicStore::load(&atomic_path)
 .with_context(|| format!("atomic store load: {}", atomic_path.display()))?;

 // Build the SymbolResolver registry from
 // [plugins.symbol_resolver.<lang>]. Only InProcess transport returns
 // real answers; Mcp/Cli surface ResolverError::NotImplemented at
 // call time until R308+ wires real backends. Unknown in-process
 // backend names log a warning and are skipped.
 let symbol_resolvers = build_symbol_resolver_map(&loaded.config);

 // Dispatch through PluginRegistry — closes the R306 carry item #1
 // (validator-class trait surface reached from production, not just
 // type-level). The SetEqualityValidator owns its config + resolver
 // map + orphan ledger + filter_id, so `Validator::validate(ctx)` is
 // self-contained and ValidationContext stays minimal.
 let validator = SetEqualityValidator {
 config: cfg.clone(),
 entry_id_prefix: prefix.clone(),
 orphan_ledger: loaded.config.orphan_ledger.clone(),
 symbol_resolvers,
 filter_id: filter_id.clone(),
 };
 let mut registry = mnemosyne_core::PluginRegistry::new();
 registry.register_validator("set_equality_validator", Box::new(validator));
 let dispatched = registry
 .validator("set_equality_validator")
 .expect("just registered");
 let store_view: &dyn mnemosyne_core::AtomicStoreView = &store;
 let ctx = mnemosyne_core::ValidationContext {
 workspace_root: &root,
 atomic_sidecar: &atomic_path,
 store: store_view,
 };
 let findings = dispatched
 .validate(&ctx)
 .map_err(|e| anyhow!("SetEqualityValidator dispatch failed: {}", e))?;

 // Per-class counting reads `ValidationFinding.kind` (the validator's
 // sub-kind tag). The kind→count routing matches the
 // pre-dispatch CodeRefViolation arms 1-to-1 — same defect-class
 // bucketing for `severity_missing` / `severity_binding` /
 // `severity_inventory`.
 let mut counts = std::collections::BTreeMap::<&str, usize>::new();
 for f in &findings {
 if let Some(k) = f.kind.as_deref() {
 *counts.entry(k).or_insert(0) += 1;
 }
 }
 let get = |k: &str| counts.get(k).copied().unwrap_or(0);
 let missing_count = get("missing");
 let section_missing_count = get("section_missing");
 let citation_unbound_count = get("citation_unbound");
 let impl_unbacked_count = get("impl_unbacked");
 let decay_count = get("decay");
 let impl_missing_count = get("impl_missing");
 let inventory_missing_count = get("inventory_missing");
 let inventory_deprecated_count = get("inventory_deprecated");
 let symbol_mismatch_count = get("symbol_mismatch");
 let inventory_count = inventory_missing_count + inventory_deprecated_count;
 let hallucination_count = missing_count + section_missing_count;
 // impl_missing bucketed into severity_binding (defect_class = Binding
 // for all three Path B edges). RFC-002 FR-3 SymbolMismatch joins the
 // binding bucket so the existing severity flag governs symbol-axis
 // policy without a new knob.
 let binding_count = citation_unbound_count + impl_unbacked_count + impl_missing_count
 + symbol_mismatch_count;

 if json {
 // Reconstruct the pre-R307 JSON shape from ValidationFinding's
 // universal fields + plugin-specific `extras` (entry_id / symbol /
 // decision_status). External consumers see a byte-identical
 // structure modulo extras ordering.
 let view: Vec<serde_json::Value> = findings
 .iter()
 .map(|f| {
 let mut obj = serde_json::Map::new();
 if let Some(k) = &f.kind {
 obj.insert("kind".into(), serde_json::Value::String(k.clone()));
 }
 if let Some(file) = &f.file {
 obj.insert(
  "file".into(),
  serde_json::Value::String(file.to_string_lossy().into_owned()),
 );
 }
 if let Some(line) = f.line {
 obj.insert("line".into(), serde_json::Value::Number(line.into()));
 }
 if let Some(sid) = &f.section_id {
 obj.insert("section_id".into(), serde_json::Value::String(sid.clone()));
 }
 for (k, v) in &f.extras {
 obj.insert(k.clone(), v.clone());
 }
 serde_json::Value::Object(obj)
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
 "valid_inventory_count": store.inventory_entries.len(),
 "inventory_prefixes": cfg.inventory_prefixes,
 "inventory_path_prefixes": cfg.inventory_path_prefixes,
 "external_section_prefixes": cfg.external_section_prefixes,
 "external_section_prefixes_bare": cfg.external_section_prefixes_bare,
 "missing_count": missing_count,
 "section_missing_count": section_missing_count,
 "citation_unbound_count": citation_unbound_count,
 "impl_unbacked_count": impl_unbacked_count,
 "impl_missing_count": impl_missing_count,
 "decay_count": decay_count,
 "inventory_missing_count": inventory_missing_count,
 "inventory_deprecated_count": inventory_deprecated_count,
 "severity_missing": severity_missing,
 "severity_binding": severity_binding,
 "severity_inventory": severity_inventory,
 "filter_id": filter_id,
 "violations": view,
 })
 );
 } else {
 println!("=== mnemosyne-cli validate-code-refs ===");
 println!(
 "prefix={:?} valid_entries={} valid_sections={} valid_inventory={} scanned_paths={:?}",
 prefix,
 store.changelog_entries.len(),
 store.sections.len(),
 store.inventory_entries.len(),
 cfg.paths
 );
 if !cfg.inventory_prefixes.is_empty() {
 println!("inventory_prefixes={:?} (Round 275 axis)", cfg.inventory_prefixes);
 }
 if !cfg.inventory_path_prefixes.is_empty() {
 println!(
 "inventory_path_prefixes={:?} (Round 302 section-path axis)",
 cfg.inventory_path_prefixes
 );
 }
 if !cfg.external_section_prefixes.is_empty() {
 println!(
 "external_section_prefixes={:?} (Round 277 numeric mode)",
 cfg.external_section_prefixes
 );
 }
 if !cfg.external_section_prefixes_bare.is_empty() {
 println!(
 "external_section_prefixes_bare={:?} (Round 284 doc-name mode)",
 cfg.external_section_prefixes_bare
 );
 }
 if let Some(ref fid) = filter_id {
 println!("filter_id={:?} (Round 258 decay scan mode)", fid);
 }
 println!(
 "violations: total={} missing={} section_missing={} \
 citation_unbound={} impl_unbacked={} impl_missing={} decay={} \
 inv_missing={} inv_deprecated={} \
 (severity_missing={} severity_binding={} severity_inventory={})",
 findings.len(),
 missing_count,
 section_missing_count,
 citation_unbound_count,
 impl_unbacked_count,
 impl_missing_count,
 decay_count,
 inventory_missing_count,
 inventory_deprecated_count,
 severity_missing,
 severity_binding,
 severity_inventory,
 );
 // ValidationFinding.message is pre-formatted by
 // `violation_to_finding` to mirror the pre-R307 TTY shape — render
 // each finding's message line as-is.
 for f in &findings {
 println!(" {}", f.message);
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
 ImplementationMissing={} (severity_binding=reject)",
 binding_count, citation_unbound_count, impl_unbacked_count, impl_missing_count,
 ));
 }
 if inventory_count > 0 && severity_inventory == "reject" {
 reject_msgs.push(format!(
 "{} inventory-axis violation(s) — InventoryMissing={} InventoryDeprecated={} \
 (severity_inventory=reject)",
 inventory_count, inventory_missing_count, inventory_deprecated_count,
 ));
 }
 if !reject_msgs.is_empty() {
 bail!("{}", reject_msgs.join("; "));
 }
 Ok(())
}
