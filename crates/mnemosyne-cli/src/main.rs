//! mnemosyne-cli — Phase 0 dogfood entry point.
//!
//! Spec binding: §code-citation-defense (via cmd_validate_code_refs).
//!
//! `validate-workspace` validates the atomic store (the SSOT) store-direct:
//! T1 prose cross-ref orphans, T2 frozen ledger, T3/T4 style, atomic
//! referential closure, publishable/audit divergence, commit-ledger drift.
//! The pre-commit hook + CI workflow invoke this binary as the dogfood gate.

// atomic_cli is exposed via the package library (src/lib.rs); the bin
// reaches it through the lib so both targets share one module instance.
use mnemosyne_cli::atomic_cli;

use std::collections::BTreeSet;
use std::env;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::OnceLock;

use anyhow::{anyhow, bail, Context, Result};

use mnemosyne_atomic::AtomicStore;
use mnemosyne_config::{discover_config, LoadedConfig, SchemaSection, Severity, WorkspaceConfig};
use mnemosyne_query::{
    build_envelope, changelog_entries_for_section, query_term, related_sections_with_atomic,
    section_by_id, TermMode, TermQuery, TermScope,
};
use mnemosyne_style::{
    check_style_atomic, default_ruleset_with_config, StyleSeverity, StyleViolation,
};
use mnemosyne_validate::code_refs::SetEqualityValidator;

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
    let loaded = discover_config(&cwd)?.ok_or_else(|| {
        anyhow!("mnemosyne.toml not found — CWD or ancestor in config file required")
    })?;
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
                } else if backend == mnemosyne_plugin_tree_sitter_cpp::BACKEND_KEY {
                    out.insert(
                        lang.clone(),
                        Box::new(mnemosyne_plugin_tree_sitter_cpp::TreesitterCppResolver),
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
    let prog = args.first().map(String::as_str).unwrap_or("mnemosyne-cli");
    let cmd = args.get(1).ok_or_else(|| {
 anyhow!(
 "usage: {} <validate|validate-workspace|query|add-section|import-sections|import-facts|add-frame|add-branch|add-entity|add-fact|add-fact-conflict|amend-fact|retract-fact|style-check|set-section-intent|set-section-rationale|set-section-inputs|set-section-outputs|set-section-title|set-section-parent-doc|set-section-parent-section|add-section-caveat|set-section-alternatives|set-section-impact-scope|add-section-example|add-section-binding|remove-section-binding|set-section-binding-kind|set-section-coverage-expectation|set-section-verification-expectation|add-confirmation-event|set-section-decision-status|import-epub-excerpts|remove-section|append-changelog-entry|set-changelog-publishable-decision-summary|set-changelog-publishable-changes|set-changelog-publishable-verification|set-changelog-publishable-impact-refs|set-changelog-publishable-carry-forward|redact-term|emit-publishable-override-ledger-draft|add-inventory-entry|set-inventory-status|set-inventory-section-ref|remove-inventory-entry> [args...]",
 prog
 )
 })?;

    match cmd.as_str() {
        "validate-workspace" => cmd_validate_workspace(),
        "query" => cmd_query(prog, &args[2..]),
        "add-section" => atomic_cli::cmd_add_section(&workspace_anchor()?, &args[2..]),
        "import-sections" => atomic_cli::cmd_import_sections(&workspace_anchor()?, &args[2..]),
        // Round 430 — narrative fact primitives (Phase 1A).
        "import-facts" => atomic_cli::cmd_import_facts(&workspace_anchor()?, &args[2..]),
        "add-frame" => atomic_cli::cmd_add_frame(&workspace_anchor()?, &args[2..]),
        "add-branch" => atomic_cli::cmd_add_branch(&workspace_anchor()?, &args[2..]),
        "add-entity" => atomic_cli::cmd_add_entity(&workspace_anchor()?, &args[2..]),
        "add-fact" => atomic_cli::cmd_add_fact(&workspace_anchor()?, &args[2..]),
        "add-fact-conflict" => atomic_cli::cmd_add_fact_conflict(&workspace_anchor()?, &args[2..]),
        "amend-fact" => atomic_cli::cmd_amend_fact(&workspace_anchor()?, &args[2..]),
        "retract-fact" => atomic_cli::cmd_retract_fact(&workspace_anchor()?, &args[2..]),
        "import-epub-anchors" => {
            atomic_cli::cmd_import_epub_anchors(&workspace_anchor()?, &args[2..])
        }
        "style-check" => cmd_style_check(prog, &args[2..]),
        // atomic mutate API surface.
        "set-section-intent" => {
            atomic_cli::cmd_set_section_intent(&workspace_anchor()?, &args[2..])
        }
        "set-section-rationale" => {
            atomic_cli::cmd_set_section_rationale(&workspace_anchor()?, &args[2..])
        }
        "set-section-inputs" => {
            atomic_cli::cmd_set_section_inputs(&workspace_anchor()?, &args[2..])
        }
        "set-section-outputs" => {
            atomic_cli::cmd_set_section_outputs(&workspace_anchor()?, &args[2..])
        }
        // Round 287 — outline setter surface (Phase C).
        "set-section-title" => atomic_cli::cmd_set_section_title(&workspace_anchor()?, &args[2..]),
        "set-section-parent-doc" => {
            atomic_cli::cmd_set_section_parent_doc(&workspace_anchor()?, &args[2..])
        }
        "set-section-parent-section" => {
            atomic_cli::cmd_set_section_parent_section(&workspace_anchor()?, &args[2..])
        }
        "add-section-caveat" => {
            atomic_cli::cmd_add_section_caveat(&workspace_anchor()?, &args[2..])
        }
        "set-section-alternatives" => {
            atomic_cli::cmd_set_section_alternatives(&workspace_anchor()?, &args[2..])
        }
        "set-section-impact-scope" => {
            atomic_cli::cmd_set_section_impact_scope(&workspace_anchor()?, &args[2..])
        }
        "add-section-example" => {
            atomic_cli::cmd_add_section_example(&workspace_anchor()?, &args[2..])
        }
        // Path B (Spec ↔ Code bidirectional binding) substrate — typed
        // trace-link edges (Binding{file, symbol?, kind}).
        "add-section-binding" => {
            atomic_cli::cmd_add_section_binding(&workspace_anchor()?, &args[2..])
        }
        // Section.bindings remove primitive (set-element granularity, kind-agnostic match).
        "remove-section-binding" => {
            atomic_cli::cmd_remove_section_binding(&workspace_anchor()?, &args[2..])
        }
        // Reclassify an existing binding's kind (Stage-B implements→references).
        "set-section-binding-kind" => {
            atomic_cli::cmd_set_section_binding_kind(&workspace_anchor()?, &args[2..])
        }
        // Classify a section's coverage applicability (normative | informative);
        // informative exempts it from the coverage axiom (Round 389).
        "set-section-coverage-expectation" => {
            atomic_cli::cmd_set_section_coverage_expectation(&workspace_anchor()?, &args[2..])
        }
        "set-section-verification-expectation" => {
            atomic_cli::cmd_set_section_verification_expectation(&workspace_anchor()?, &args[2..])
        }
        "add-confirmation-event" => {
            atomic_cli::cmd_add_confirmation_event(&workspace_anchor()?, &args[2..])
        }
        // Round 265 — Stage B freshness substrate. (Round 304 — _atomic suffix
        // dropped; legacy markdown-surgical variant retired with the rest of
        // `mutate.rs`.)
        "set-section-decision-status" => {
            atomic_cli::cmd_set_section_decision_status(&workspace_anchor()?, &args[2..])
        }
        "import-epub-excerpts" => {
            atomic_cli::cmd_import_epub_excerpts(&workspace_anchor()?, &args[2..])
        }
        // Round 267 — section removal (closes Round 266 carry gap).
        "remove-section" => atomic_cli::cmd_remove_section(&workspace_anchor()?, &args[2..]),
        "append-changelog-entry" => {
            atomic_cli::cmd_append_changelog_entry(&workspace_anchor()?, &args[2..])
        }
        // Round 295 — publishable-half setters (audit half stays frozen).
        "set-changelog-publishable-decision-summary" => {
            atomic_cli::cmd_set_changelog_publishable_decision_summary(
                &workspace_anchor()?,
                &args[2..],
            )
        }
        "set-changelog-publishable-changes" => {
            atomic_cli::cmd_set_changelog_publishable_changes(&workspace_anchor()?, &args[2..])
        }
        "set-changelog-publishable-verification" => {
            atomic_cli::cmd_set_changelog_publishable_verification(&workspace_anchor()?, &args[2..])
        }
        "set-changelog-publishable-impact-refs" => {
            atomic_cli::cmd_set_changelog_publishable_impact_refs(&workspace_anchor()?, &args[2..])
        }
        "set-changelog-publishable-carry-forward" => {
            atomic_cli::cmd_set_changelog_publishable_carry_forward(
                &workspace_anchor()?,
                &args[2..],
            )
        }
        // Round 297 — RFC P1 redact_term convenience primitive.
        "redact-term" => atomic_cli::cmd_redact_term(&workspace_anchor()?, &args[2..]),
        // Round 300 — bare-setter ledger draft companion.
        "emit-publishable-override-ledger-draft" => {
            atomic_cli::cmd_emit_publishable_override_ledger_draft(&workspace_anchor()?, &args[2..])
        }
        // Round 274 — Phase 1A inventory mutate primitives.
        "add-inventory-entry" => {
            atomic_cli::cmd_add_inventory_entry(&workspace_anchor()?, &args[2..])
        }
        "set-inventory-status" => {
            atomic_cli::cmd_set_inventory_status(&workspace_anchor()?, &args[2..])
        }
        "set-inventory-section-ref" => {
            atomic_cli::cmd_set_inventory_section_ref(&workspace_anchor()?, &args[2..])
        }
        "remove-inventory-entry" => {
            atomic_cli::cmd_remove_inventory_entry(&workspace_anchor()?, &args[2..])
        }
        // Stage 2 of code-citation defense (Stage 1 = CLAUDE.md
        // rule, carry).
        "validate-code-refs" => cmd_validate_code_refs(&args[2..]),
        "propose-implementations" => cmd_propose_implementations(&args[2..]),
        "report-binding-migration" => cmd_report_binding_migration(&args[2..]),
        "report-coverage" => cmd_report_coverage(&args[2..]),
        "report-confirmation" => cmd_report_confirmation(&args[2..]),
        "validate-confirmation" => cmd_validate_confirmation(&args[2..]),
        // Round 431 — frame-scoped narrative continuity gate (Phase 1A Round B).
        "validate-continuity" => cmd_validate_continuity(&args[2..]),
        // Round 432 — frame-at-T read projection (Phase 1A Round C).
        "report-frame-view" => cmd_report_frame_view(&args[2..]),
        "report-entity" => cmd_report_entity(&args[2..]),
        "validate-verifies-linkage" => cmd_validate_verifies_linkage(&args[2..]),
        "report-excerpt-hash-backfill" => cmd_report_excerpt_hash_backfill(&args[2..]),
        "report-spec-map" => cmd_report_spec_map(&args[2..]),
        "validate-spec-drift" => cmd_validate_spec_drift(&args[2..]),
        "validate-content-drift" => cmd_validate_content_drift(&args[2..]),
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

fn print_help(prog: &str) {
    println!(
        "mnemosyne-cli {} ({}) — Phase 0 design_doc lifecycle (DESIGN §66)",
        env!("CARGO_PKG_VERSION"),
        env!("BUILD_GIT_HASH")
    );
    println!();
    println!("usage:");
    println!(
        " {} validate-workspace 7 markdown doc full validation",
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
    println!("   T3/T4 style rule layer check (Round 129 production wire)");
    println!();
    println!(" --- atomic mutate API (Round 162 production wire, Phase 0f) ---");
    println!(" Field length caps (Round 161 §41 thresholds, surfaced for DX — Round 279 carry):");
    println!(
        "   intent: max 200 chars; each bullet (rationale/inputs/outputs/caveats): max 100 chars"
    );
    println!(" {} add-section --section §<id> --parent-doc <doc-id> --title <text> [--parent §<P>] [--sidecar <path>] [--json]", prog);
    println!(
        " {} import-sections --manifest <path.json> [--sidecar <path>] [--json]",
        prog
    );
    println!(
        " {} import-epub-anchors --anchors <epub-anchor-map.json> [--sidecar <path>] [--json]",
        prog
    );
    println!("   bulk create from a JSON array of {{section_id,parent_doc,title,parent_section?,normative_excerpt?}};");
    println!("   3-way per entry: absent=create / byte-identical=no-op / divergent=reject whole manifest (atomic)");
    println!(
        " {} import-facts --manifest <path.json> [--sidecar <path>] [--json]",
        prog
    );
    println!("   bulk narrative frames + facts (Round 430): manifest = {{frames:[{{frame_id,description?}}],");
    println!("   facts:[{{fact_id,frame,claim,canon_from,canon_to?,evidence[],conflicts_with?,supersedes_in_frame?,quote?}}]}};");
    println!("   one atomic transaction; quote_sha256 computed at write, never caller-supplied");
    println!(
        " {} add-frame --frame <id> [--description <text>] [--sidecar <path>] [--json]",
        prog
    );
    println!(
        " {} add-branch --branch <id> [--description <text>] [--forks-from <branch> --forks-at <section>] [--sidecar <path>] [--json]",
        prog
    );
    println!(
        " {} add-entity --entity <id> [--kind <tag>] [--description <text>] [--sidecar <path>] [--json]",
        prog
    );
    println!(
        " {} report-entity --entity <id> [--sidecar <path>] [--json]",
        prog
    );
    println!(" {} add-fact --fact <id> --frame <f> [--branch <id>] --claim <text> --canon-from <section> [--canon-to <section>] --evidence <sec,sec> [--entities <id,id>] [--conflicts <id,id>] [--supersedes <id>] [--quote <text>] [--sidecar <path>] [--json]", prog);
    println!(
        " {} add-fact-conflict --fact <id> --conflicts-with <id> [--sidecar <path>] [--json]",
        prog
    );
    println!(" {} amend-fact --fact <id> --reason <text> <add-fact flags> [--sidecar <path>] [--json]   (authorial in-place revision; in-world change = --supersedes)", prog);
    println!(
        " {} retract-fact --fact <id> --reason <text> [--sidecar <path>] [--json]",
        prog
    );
    println!(
        "   pairs with remove-section (R267); content fields populate via set-section-* afterwards"
    );
    println!(" {} set-section-intent --section §<N> --intent <text (max 200 chars)> [--sidecar <path>] [--json]", prog);
    println!(" {} set-section-rationale --section §<N> --bullets-file <path (each bullet ≤ 100 chars)> [--sidecar <path>] [--json]", prog);
    println!(" {} set-section-inputs --section §<N> --bullets-file <path (each bullet ≤ 100 chars)> [--sidecar <path>] [--json]", prog);
    println!(" {} set-section-outputs --section §<N> --bullets-file <path (each bullet ≤ 100 chars)> [--sidecar <path>] [--json]", prog);
    println!(
        " {} set-section-title --section §<N> --title <heading text> [--sidecar <path>] [--json]",
        prog
    );
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
 " {} add-section-binding --section §<N> --file <workspace-relative-path> [--symbol <name>] --kind implements|references [--sidecar <path>] [--json]",
 prog
 );
    println!(
        "   Path B typed trace-link binding (implements=«satisfy» / references=«trace»); coverage counts only implements"
    );
    println!(
 " {} remove-section-binding --section §<N> --file <path> [--symbol <name>] --reason <text> [--sidecar <path>] [--json]",
 prog
 );
    println!(
 "   Section.bindings remove primitive (exact (file, symbol) match, kind-agnostic; --reason mandatory)"
 );
    println!(
 " {} set-section-binding-kind --section §<N> --file <path> [--symbol <name>] --kind implements|references --reason <text> [--sidecar <path>] [--json]",
 prog
 );
    println!(
 "   Reclassify an existing binding's kind (Stage-B implements→references; --reason mandatory)"
 );
    println!(
 " {} set-section-coverage-expectation --section §<N> --expectation normative|out_of_scope_here|informational --reason <text> [--sidecar <path>] [--json]",
 prog
 );
    println!(
 " {} set-section-verification-expectation --section §<N> --expectation dedicated|by_construction --reason <text> [--sidecar <path>] [--json]",
 prog
 );
    println!(
 " {} add-confirmation-event --section §<N> [--file <path> --symbol <sym>] --confirmer-kind tool|model --confirmer-id <id> --confirmer-version <v> --method linkage_check|semantic_review|coverage_attestation --verdict confirm|refute --authoring-run <id> --confirming-run <id> --rationale <text> --timestamp <iso> [--spec-sha256 <h>] [--code-sha256 <h>] [--test-sha256 <h>] [--sidecar <path>] [--json]",
 prog
 );
    println!(
 "   Classify coverage applicability; informative exempts the section from the coverage axiom (--reason mandatory)"
 );
    println!(
        " {} import-epub-excerpts --anchors <epub-anchor-map.json> [--sidecar <path>] [--json]",
        prog
    );
    println!(
 "   refresh normative_excerpt.text + text_sha256 from a medium-forge epub-anchor-map/v2; preserves authored anchor_url + source_revision (section must already carry an excerpt)"
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
    println!("   Round 267 section removal (audit-safeguarded; closes Round 266 carry)");
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
    println!();
    println!(" --- code citation defense (Round 255-260, Path B bidirectional) ---");
    println!(
        " {} validate-code-refs [--severity-missing reject|warn|info]\n\
 \x20                       [--severity-binding reject|warn|info]\n\
 \x20                       [--severity-coverage reject|warn|info]\n\
 \x20                       [--severity-verification reject|warn|info]\n\
 \x20                       [--severity-classification reject|warn|info]\n\
 \x20                       [--severity-blanket reject|warn|info]\n\
 \x20                       [--filter-id <entry_id>] [--json]",
        prog
    );
    println!(
 "   Round 256: scan [plugins.set_equality_validator].paths for <entry_id_prefix><digits> citations,"
 );
    println!("   reject those whose entry_id is missing from atomic store changelog_entries");
    println!("   Round 260: §<id> citations cross-checked against AtomicSection.bindings");
    println!("   --severity-missing: Missing + SectionMissing (hallucination class)");
    println!(
 "   --severity-binding (Round 260): CitationUnbound + BindingUnbacked + SymbolMismatch (edge class)"
 );
    println!(
 "   --severity-coverage (Round 385): ImplementationMissing (Active section uncited); inherits --severity-binding when unset"
 );
    println!(
 "   --filter-id (Round 258): restrict to citations of one id; surfaces them as decay (cascade caller use)"
 );
    println!();
    println!(
        " {} propose-implementations [--section §<id>] [--json]",
        prog
    );
    println!(
        "   Path B curation: per (section,file) cite, resolve the enclosing/documented symbol and"
    );
    println!("   emit proposed §<id> binding sets + add-section-binding commands (read-only)");
    println!(" {} report-binding-migration [--json]", prog);
    println!(
        "   v4→v5 surface: list bindings that inherited kind=implements by default (read-only;"
    );
    println!("   empty once the store is at v5 — run before upgrading a pre-v5 store)");
    println!(" {} report-coverage [--json]", prog);
    println!(" {} report-confirmation [--json]", prog);
    println!(
        " {} validate-confirmation [--severity reject|warn|info] [--json]",
        prog
    );
    println!(
        " {} validate-continuity [--order <canon-order.json>] [--severity reject|warn|info] [--sidecar <path>] [--json]",
        prog
    );
    println!("   frame-scoped narrative continuity (Round 431): same-frame overlapping conflict = violation,");
    println!(
        "   cross-frame conflict = data; canon order is a DECLARED partial order, never inferred"
    );
    println!(
        " {} report-frame-view --frame <id> [--branch <id>] [--entity <id>] --at <section> [--order <canon-order.json>] [--sidecar <path>] [--json]",
        prog
    );
    println!(
        "   read-only frame-at-T projection (Round 432): the facts frame F holds at canon point T,"
    );
    println!("   same holds-semantics as the gate; incomparable coordinates surface as `unknown`");
    println!(
        " {} validate-verifies-linkage [--catalog <path>] [--severity reject|warn|info] [--json]",
        prog
    );
    println!(" {} report-excerpt-hash-backfill [--json]", prog);
    println!(
        "   coverage breakdown: implemented / normative-gap / informative-exempt + ratio (read-only)"
    );
    println!(" {} report-spec-map [--json]", prog);
    println!(
        "   unified spec<->fact<->code projection per section: coverage class + spec provenance"
    );
    println!(
        "   (anchor_url/revision) + bindings + drift flag + reverse citation count (read-only L3 view)"
    );
    println!();
    println!(" --- spec-revision drift (RFC-001 UC-1 \"B2\") ---");
    println!(
        " {} validate-spec-drift [--severity reject|warn|info] [--json]",
        prog
    );
    println!("   flag Active Sections whose normative_excerpt.source_revision trails");
    println!("   [workspace.spec_source].revision; Superseded/Removed exempt (partial-migration).");
    println!(
        "   --severity overrides [spec_drift].severity (default warn); reject => exit 1 on drift."
    );
    println!("   no-op (exit 0) when [workspace.spec_source] is absent.");
    println!();
    println!(" --- content-integrity drift (R404 — EPUB-as-content-SSOT) ---");
    println!(
        " {} validate-content-drift [--severity reject|warn|info] [--json]",
        prog
    );
    println!("   offline re-hash of each normative_excerpt.text vs its text_sha256;");
    println!("   a populated hash that no longer matches = drift (cache edited out-of-band).");
    println!(
        "   --severity overrides [content_drift].severity (default reject); reject => exit 1."
    );
    println!("   empty-hash excerpts are unrevalidatable (counted, not drift).");
    println!("   also re-hashes the committed EPUB vs [workspace.spec_source].epub_sha256 when pinned (R405).");
    println!();
    println!(" --- meta (Round 286) ---");
    println!(
        " {} --version | -V | version  print binary version + build hash",
        prog
    );
    println!(" {} --help | -h | help   print this help text", prog);
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
                        .ok_or_else(|| {
                            anyhow!("--scope missing value (all|sections|changelog|inventory)")
                        })?
                        .clone(),
                );
            }
            "--field" => {
                let v = iter.next().ok_or_else(|| {
                    anyhow!("--field missing value (comma-separated field names)")
                })?;
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
                let stripped = mnemosyne_core::strip_section_marker(other).to_string();
                out.section_id = Some(stripped);
            }
        }
    }
    Ok(out)
}

fn cmd_query(prog: &str, args: &[String]) -> Result<()> {
    let qargs = parse_query_args(args)?;
    let root = workspace_anchor()?;
    let atomic_store = AtomicStore::load(&mnemosyne_ops::cascade::resolve_sidecar(&root, None)?)
        .map_err(|e| anyhow!("atomic store load: {}", e))?;

    if qargs.list_sections {
        // The canonical section-id set (numeric / `X/Y` ids plus ancestor
        // prefixes) lives in the atomic store, the SSOT.
        let set = atomic_store.atomic_section_id_set();
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
                let status_label = entry.status.as_str();
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
        let entry = atomic_store
            .inventory(&inv_id)
            .ok_or_else(|| anyhow!("inventory_id `{}` not present in atomic store", inv_id))?;
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
            let status_label = entry.status.as_str();
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

    let section_id = qargs
        .section_id
        .ok_or_else(|| anyhow!("section_id arg required — e.g. {} query §43", prog))?;

    if qargs.json && qargs.include_related && qargs.include_changelog {
        let envelope = build_envelope(&atomic_store, &section_id)
            .ok_or_else(|| anyhow!("section_id `{}` not found in store", section_id))?;
        println!("{}", serde_json::to_string_pretty(&envelope)?);
        return Ok(());
    }

    if qargs.json {
        let view = section_by_id(&atomic_store, &section_id)
            .ok_or_else(|| anyhow!("section_id `{}` not found in store", section_id))?;
        println!("{}", serde_json::to_string_pretty(&view)?);
        return Ok(());
    }

    let view = section_by_id(&atomic_store, &section_id)
        .ok_or_else(|| anyhow!("section_id `{}` not found in store", section_id))?;
    println!(
        "§{} ({}#L{}) {}",
        view.section_id, view.parent_doc, view.line_anchor, view.title
    );
    println!("decision_status: {}", view.decision_status);
    if let Some(parent) = &view.parent_section {
        println!("parent_section: §{}", parent);
    }
    if let Some(ne) = &view.normative_excerpt {
        println!();
        println!("--- normative excerpt ({}) ---", ne.source_revision);
        println!("source: {}", ne.anchor_url);
        println!("{}", ne.text);
        println!("--- end normative excerpt ---");
    }
    if !view.body.is_empty() {
        println!();
        println!("--- body ---");
        println!("{}", view.body);
        println!("--- end body ---");
    }

    if qargs.include_related {
        let related = related_sections_with_atomic(&atomic_store, &section_id);
        println!();
        println!("outbound_refs ({}):", related.outbound_refs.len());
        for r in &related.outbound_refs {
            println!(" {} → {} [{}]", r.from_section, r.to_target, r.ref_kind);
        }
        println!();
        println!("inbound_refs ({}):", related.inbound_refs.len());
        for r in &related.inbound_refs {
            println!(
                " {}#§{} → {} [{}]",
                r.from_doc, r.from_section, r.to_target, r.ref_kind
            );
        }
    }

    if qargs.include_changelog {
        let entries = changelog_entries_for_section(&atomic_store, &section_id);
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
    // Single-sourced through mnemosyne-ops (R320): the full T1 / round-trip
    // / style / atomic-ledger / supersede / publishable-divergence pipeline
    // lives in ops::validate_workspace. The CLI keeps only its own stdout
    // rendering + two extra surfaces: atomic decay (display-only) and
    // commit-ledger drift, which gates the exit code at
    // `[commit_ledger].severity = reject` (the R301 default) and is
    // display-only at warn/info (R377).
    let root = workspace_anchor()?;
    let report = mnemosyne_ops::validate_workspace(&root).map_err(|e| anyhow!("{}", e))?;
    print!("{}", report.render_plain());

    print_atomic_decay_surface(&root)?;
    let atomic = AtomicStore::load(&mnemosyne_ops::cascade::resolve_sidecar(&root, None)?)
        .map_err(|e| anyhow!("atomic store load: {}", e))?;
    print_commit_ledger_drift_surface(&root, &atomic)?;

    if report.failed {
        bail!(
            "validate-workspace failed - {}",
            report.failure_reasons.join("; ")
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
    let store = match mnemosyne_atomic::AtomicStore::load(&mnemosyne_ops::cascade::resolve_sidecar(
        root, None,
    )?) {
        Ok(s) => s,
        Err(_) => return Ok(()),
    };
    let mut targets: Vec<&str> = Vec::new();
    for (section_id, section) in &store.sections {
        if matches!(
            section.skeleton.decision_status,
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
        .map_err(|e| anyhow!("scan section decay (§{}): {}", sid, e))?;
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
/// entry; the dogfood fix is to backfill the entry, not silence the gate.
///
/// Round 377 makes the scan multi-workspace-aware: the commit scan is
/// path-scoped to the workspace subtree (see
/// `collect_recent_commit_round_labels`) so a sibling workspace's labels
/// no longer bleed in, and `[commit_ledger].severity` (default `reject`)
/// lets a consumer workspace whose `(R<n>)` labels are not Mnemosyne
/// changelog rounds downgrade the gate to `warn`/`info`.
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
        // Round 377 — `[commit_ledger].severity` gates the exit code.
        // Default `reject` (table absent) preserves the R301 dogfood
        // hard-reject. A multi-workspace consumer whose `(R<n>)` labels are
        // not Mnemosyne changelog rounds sets `warn`/`info`: the missing
        // lines above still print (no silent suppression), but the gate
        // stops failing the exit code. `workspace_config()` is cached and
        // already validated severity ∈ {reject,warn,info} at load.
        let severity = workspace_config()?
            .config
            .commit_ledger
            .as_ref()
            .map(|cl| cl.severity.as_str())
            .unwrap_or("reject");
        if severity == "reject" {
            println!(
 "  hint: backfill via `mnemosyne-cli append-changelog-entry --entry-id \"Round <N> — ...\" \
  --decision <text> --changes-file <path> --verification-file <path> --impact §A,§B \
  --carry-file <path>` (Round 293 backfill flow)"
 );
            bail!(
                "commit↔ledger drift gate: {} cited round(s) missing from atomic store (Round 301)",
                report.missing.len()
            );
        }
        println!(
            "  severity={} — commit↔ledger drift surfaced, not gating this workspace (Round 377)",
            severity
        );
    }
    Ok(())
}

fn collect_recent_commit_round_labels(root: &std::path::Path, max_commits: usize) -> BTreeSet<u32> {
    // Round 377 — path-scope the scan to this workspace's subtree (`-- .`,
    // relative to `root` = the workspace root). In a single-workspace repo
    // the workspace root is the git root, so `.` matches the whole repo and
    // behaviour is unchanged. In a multi-workspace mono-repo a `(R<n>)`
    // label on a commit that only touched a *sibling* workspace no longer
    // bleeds in and false-flags this workspace's ledger as missing it.
    let output = std::process::Command::new("git")
        .args([
            "log",
            &format!("--max-count={}", max_commits),
            "--pretty=%s",
            "--",
            ".",
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

fn collect_ledger_round_numbers(atomic: &mnemosyne_atomic::AtomicStore) -> BTreeSet<u32> {
    // Single round-parse home: mnemosyne_query::round_number (shared with
    // list_changelog) so the ledger scan and the timeline ordering agree.
    atomic
        .changelog_entries
        .keys()
        .filter_map(|k| mnemosyne_query::round_number(k))
        .collect()
}

// ============================================================================
// helpers
// ============================================================================

/// Discovery anchor for ops / mutate / sidecar calls: the directory holding
/// `mnemosyne.toml`. Equal to the resolved workspace root when `[workspace]
/// root` is unset; when set (a ledger rooted above its own directory), ops
/// re-discover the config from this anchor and resolve the true root
/// themselves, so every ops call must receive the anchor, not the resolved
/// root (discovery walks UP and would miss the subdir config otherwise).
fn workspace_anchor() -> Result<PathBuf> {
    let loaded = workspace_config()?;
    Ok(loaded
        .config_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| loaded.workspace_root.clone()))
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

    let root = workspace_anchor()?;
    let style_check_cfg = workspace_config()?;
    let ruleset = default_ruleset_with_config(
        style_check_cfg.config.style.as_ref(),
        style_check_cfg.config.terminology.as_ref(),
    );

    let style_check_atomic =
        AtomicStore::load(&mnemosyne_ops::cascade::resolve_sidecar(&root, None)?)
            .map_err(|e| anyhow!("atomic store load: {}", e))?;
    // Store-direct: findings come from the atomic store (the SSOT) under a
    // stable "atomic-store" label; a `--doc` filter selects that label.
    let label = "atomic-store";
    let all_violations: Vec<StyleViolation> = match &doc_filter {
        Some(filter) if filter != label => Vec::new(),
        _ => check_style_atomic(label, &style_check_atomic, &ruleset),
    };

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
        println!(
            "violations: total={} t3_warn={} t4_info={}",
            filtered.len(),
            t3,
            t4
        );
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
/// v4→v5 migration surface (read-only): when the atomic store on disk is
/// pre-v5 (its bindings carried no `kind` and all defaulted to `implements`
/// at load), print the inferred-default work-list so the migration is NOT a
/// silent blessing — each row is a Stage-B reclassification candidate
/// (flip data/DTO fields to `references` via `set-section-binding-kind`).
/// Returns nothing to review once the store is at v5 (the first save bumps
/// the version and the signal is gone), so run this before/around upgrading
/// a v4 store. Never mutates.
fn cmd_report_binding_migration(args: &[String]) -> Result<()> {
    let mut json = false;
    for a in args {
        match a.as_str() {
            "--json" => json = true,
            other => bail!("unknown flag `{}`", other),
        }
    }
    let loaded = workspace_config()?;
    let root = loaded.workspace_root.clone();
    let anchor = loaded
        .config_path
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| root.clone());
    let atomic_path = mnemosyne_ops::cascade::resolve_sidecar(&anchor, None)?;
    let store = AtomicStore::load(&atomic_path)
        .with_context(|| format!("atomic store load: {}", atomic_path.display()))?;
    match store.kind_migration_report() {
        None => {
            if json {
                println!("{{\"from_schema_version\":null,\"rows\":[]}}");
            } else {
                println!(
                    "store already at current schema (>= v5); no binding-kind migration pending"
                );
            }
        }
        Some(report) => {
            if json {
                let rows: Vec<_> = report
                    .rows
                    .iter()
                    .map(|r| {
                        serde_json::json!({
                            "section_id": r.section_id,
                            "file": r.file,
                            "symbol": r.symbol,
                            "defaulted_kind": r.defaulted_kind.as_str(),
                        })
                    })
                    .collect();
                println!(
                    "{}",
                    serde_json::json!({
                        "from_schema_version": report.from_schema_version,
                        "rows": rows,
                    })
                );
            } else {
                println!(
                    "=== binding-kind migration report (store schema v{} → v5) ===",
                    report.from_schema_version
                );
                println!(
                    "{} binding(s) inherited kind=implements by default — review and reclassify",
                    report.rows.len()
                );
                println!("(flip data/DTO fields to references: set-section-binding-kind --kind references --reason …)");
                for r in &report.rows {
                    match &r.symbol {
                        Some(s) => println!("  §{}  {}:{}", r.section_id, r.file, s),
                        None => println!("  §{}  {}", r.section_id, r.file),
                    }
                }
            }
        }
    }
    Ok(())
}

/// R402 — excerpt-hash backfill work-list: every section whose
/// `normative_excerpt` has an empty `text_sha256` (hand-authored or pre-v8
/// excerpt not yet revalidatable against an EPUB). Re-import via
/// `import-epub-excerpts` to populate the hash. Read-only.
fn cmd_report_excerpt_hash_backfill(args: &[String]) -> Result<()> {
    let mut json = false;
    for a in args {
        match a.as_str() {
            "--json" => json = true,
            other => bail!("unknown flag `{}`", other),
        }
    }
    let loaded = workspace_config()?;
    let anchor = loaded
        .config_path
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| loaded.workspace_root.clone());
    let atomic_path = mnemosyne_ops::cascade::resolve_sidecar(&anchor, None)?;
    let store = AtomicStore::load(&atomic_path)
        .with_context(|| format!("atomic store load: {}", atomic_path.display()))?;
    let report = store.excerpt_hash_backfill_report();
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("=== excerpt-hash backfill report ===");
        println!(
            "{} excerpt(s) lack a text_sha256 — project from an EPUB via import-epub-excerpts",
            report.rows.len()
        );
        for r in &report.rows {
            println!("  §{}  (rev {})", r.section_id, r.source_revision);
        }
    }
    Ok(())
}

/// Positive coverage projection (Round 390): the 3-way breakdown of every
/// section — implemented / normative-gap / informative-exempt — plus the
/// `Removed` tombstones excluded from the denominator, and the coverage
/// ratio. Read-only (no authoritative state of its own); the positive
/// counterpart of the `validate-code-refs` coverage axis, which emits the
/// precise gap list. Mirrors `report-binding-migration`.
fn cmd_report_coverage(args: &[String]) -> Result<()> {
    let mut json = false;
    for a in args {
        match a.as_str() {
            "--json" => json = true,
            other => bail!("unknown flag `{}`", other),
        }
    }
    let loaded = workspace_config()?;
    let root = loaded.workspace_root.clone();
    let anchor = loaded
        .config_path
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| root.clone());
    let atomic_path = mnemosyne_ops::cascade::resolve_sidecar(&anchor, None)?;
    let store = AtomicStore::load(&atomic_path)
        .with_context(|| format!("atomic store load: {}", atomic_path.display()))?;
    let snapshot = mnemosyne_core::AtomicStoreView::snapshot(&store);
    let report = mnemosyne_validate::code_refs::classify_coverage(&snapshot);
    if json {
        println!(
            "{}",
            serde_json::json!({
                "applicable": report.applicable(),
                "implemented_count": report.implemented.len(),
                "normative_gap_count": report.normative_gap.len(),
                "informative_exempt_count": report.informative_exempt.len(),
                "removed_excluded_count": report.removed_excluded.len(),
                "coverage_ratio": report.coverage_ratio(),
                "implemented": report.implemented,
                "normative_gap": report.normative_gap,
                "informative_exempt": report.informative_exempt,
                "removed_excluded": report.removed_excluded,
            })
        );
    } else {
        println!("=== coverage report ===");
        println!("  implemented:        {}", report.implemented.len());
        println!("  normative gap:      {}", report.normative_gap.len());
        println!("  informative exempt: {}", report.informative_exempt.len());
        println!("  removed (excluded): {}", report.removed_excluded.len());
        match report.coverage_ratio() {
            Some(ratio) => println!(
                "  coverage: {:.1}% ({}/{} applicable)",
                ratio * 100.0,
                report.implemented.len(),
                report.applicable()
            ),
            None => println!("  coverage: n/a (0 applicable sections)"),
        }
        if !report.normative_gap.is_empty() {
            println!("normative-gap sections (same set as validate-code-refs impl_missing):");
            for id in &report.normative_gap {
                println!("  §{}", id);
            }
        }
    }
    Ok(())
}

/// R418 — read-only confirmation projection (max-rigor v1). Classifies each
/// claim in the event log as confirmed / proposed / refuted via the v1
/// required-evidence-set, and surfaces the confirmation-debt work-queue (claims
/// not yet confirmed). Pure over the stored events; no new authoritative state.
/// Drift/staleness is out of scope until R419 wires artifact hashing.
fn cmd_report_confirmation(args: &[String]) -> Result<()> {
    use mnemosyne_atomic::{ConfirmationClaim, ConfirmationStatus};
    let mut json = false;
    for a in args {
        match a.as_str() {
            "--json" => json = true,
            other => bail!("unknown flag `{}`", other),
        }
    }
    let loaded = workspace_config()?;
    let root = loaded.workspace_root.clone();
    let anchor = loaded
        .config_path
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| root.clone());
    let atomic_path = mnemosyne_ops::cascade::resolve_sidecar(&anchor, None)?;
    let store = AtomicStore::load(&atomic_path)
        .with_context(|| format!("atomic store load: {}", atomic_path.display()))?;
    let report = mnemosyne_atomic::confirmation_report(&store);
    let count_of = |s: ConfirmationStatus| report.claims.iter().filter(|c| c.status == s).count();
    let confirmed = count_of(ConfirmationStatus::Confirmed);
    let proposed = count_of(ConfirmationStatus::Proposed);
    let refuted = count_of(ConfirmationStatus::Refuted);
    let claim_label = |claim: &ConfirmationClaim| -> String {
        match claim {
            ConfirmationClaim::VerifiesBinding {
                section_id,
                file,
                symbol,
            } => format!(
                "§{} {}{}",
                section_id,
                file,
                symbol
                    .as_deref()
                    .map(|s| format!(":{s}"))
                    .unwrap_or_default()
            ),
            ConfirmationClaim::SectionCompleteness { section_id } => {
                format!("§{} (all-I/O completeness)", section_id)
            }
        }
    };
    if json {
        println!(
            "{}",
            serde_json::json!({
                "total_claims": report.claims.len(),
                "confirmed_count": confirmed,
                "proposed_count": proposed,
                "refuted_count": refuted,
                "debt_count": report.debt().count(),
                "claims": report.claims,
            })
        );
    } else {
        println!("=== confirmation report ===");
        println!("  confirmed: {}", confirmed);
        println!("  proposed:  {}", proposed);
        println!("  refuted:   {}", refuted);
        println!("  debt (not yet confirmed): {}", report.debt().count());
        for c in report.debt() {
            let st = match c.status {
                ConfirmationStatus::Proposed => "proposed",
                ConfirmationStatus::Confirmed => "confirmed",
                ConfirmationStatus::Refuted => "refuted",
                ConfirmationStatus::Stale => "stale",
            };
            println!("  [{}] {}", st, claim_label(&c.claim));
        }
    }
    Ok(())
}

/// R419 — confirmation gate (max-rigor v1). For every Normative + Dedicated
/// section, each `verifies` binding must map to a Confirmed claim, else it is an
/// unconfirmed gap. Opt-in: `--severity` overrides
/// `[plugins.set_equality_validator].severity_confirmation`; unset on both means
/// the gate is disabled (exit 0). `reject` + any gap => exit 1. Layers above the
/// R413 verify axis (verify = a test exists; confirmation = independently
/// re-verified).
fn cmd_validate_confirmation(args: &[String]) -> Result<()> {
    use mnemosyne_atomic::ConfirmationStatus;
    use mnemosyne_config::Severity;
    let mut json = false;
    let mut severity_override: Option<String> = None;
    let mut iter = args.iter();
    while let Some(a) = iter.next() {
        match a.as_str() {
            "--json" => json = true,
            "--severity" => {
                severity_override = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--severity missing"))?
                        .clone(),
                )
            }
            other => bail!("unknown flag `{}`", other),
        }
    }
    let loaded = workspace_config()?;
    let root = loaded.workspace_root.clone();
    let anchor = loaded
        .config_path
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| root.clone());
    let configured = loaded
        .config
        .plugins
        .as_ref()
        .and_then(|p| p.set_equality_validator.as_ref())
        .and_then(|c| c.severity_confirmation);
    let severity = match severity_override {
        Some(s) => Some(
            Severity::from_tag(s.trim())
                .ok_or_else(|| anyhow!("--severity must be `reject`, `warn`, or `info`"))?,
        ),
        None => configured,
    };
    let atomic_path = mnemosyne_ops::cascade::resolve_sidecar(&anchor, None)?;
    let store = AtomicStore::load(&atomic_path)
        .with_context(|| format!("atomic store load: {}", atomic_path.display()))?;
    let snapshot = mnemosyne_core::AtomicStoreView::snapshot(&store);
    // R427 (SCE P3) — when a verifies catalog is configured, its live exact
    // match is an authoritative confirmation branch.
    let catalog = load_verifies_catalog_if_configured(&loaded.config, &root)?;
    let gaps = mnemosyne_validate::confirmation::scan_confirmation_gate(
        &snapshot,
        &store,
        &root,
        catalog.as_ref(),
    );
    let status_str = |s: ConfirmationStatus| match s {
        ConfirmationStatus::Proposed => "proposed",
        ConfirmationStatus::Confirmed => "confirmed",
        ConfirmationStatus::Refuted => "refuted",
        ConfirmationStatus::Stale => "stale",
    };
    if json {
        println!(
            "{}",
            serde_json::json!({
                "severity": severity.map(|s| s.as_str()),
                "unconfirmed_count": gaps.len(),
                "unconfirmed": gaps.iter().map(|g| serde_json::json!({
                    "section_id": g.section_id,
                    "file": g.file,
                    "symbol": g.symbol,
                    "status": status_str(g.status),
                })).collect::<Vec<_>>(),
            })
        );
    } else {
        match severity {
            None => println!("confirmation gate: disabled (severity_confirmation unset)"),
            Some(s) => {
                println!("=== confirmation gate ({}) ===", s.as_str());
                println!("  unconfirmed verifies bindings: {}", gaps.len());
                for g in &gaps {
                    let sym = g
                        .symbol
                        .as_deref()
                        .map(|x| format!(":{x}"))
                        .unwrap_or_default();
                    println!(
                        "  [{}] §{} {}{}",
                        status_str(g.status),
                        g.section_id,
                        g.file,
                        sym
                    );
                }
            }
        }
    }
    if matches!(severity, Some(s) if s.is_reject()) && !gaps.is_empty() {
        std::process::exit(1);
    }
    Ok(())
}

/// Round 431 — frame-scoped narrative continuity gate (`validate-continuity`).
///
/// Severity: `--severity` flag > `[continuity].severity` > disabled (table
/// absent = opt-out; the scan still runs read-only and reports). Order/store
/// resolution is the shared `ops::continuity_scan` path (Round 435 — one
/// resolution chain for CLI and MCP): `--order` bypasses the sha256 pin (the
/// R428 `--catalog` rule); `--sidecar` overrides the store path (narrative
/// facts usually live in non-dogfood stores).
fn cmd_validate_continuity(args: &[String]) -> Result<()> {
    use mnemosyne_config::Severity;
    let mut json = false;
    let mut severity_override: Option<String> = None;
    let mut order_override: Option<String> = None;
    let mut sidecar_override: Option<String> = None;
    let mut iter = args.iter();
    while let Some(a) = iter.next() {
        match a.as_str() {
            "--json" => json = true,
            "--severity" => {
                severity_override = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--severity missing"))?
                        .clone(),
                )
            }
            "--order" => {
                order_override = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--order missing"))?
                        .clone(),
                )
            }
            "--sidecar" => {
                sidecar_override = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            other => bail!("unknown flag `{}`", other),
        }
    }
    let loaded = workspace_config()?;
    let anchor = loaded
        .config_path
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| loaded.workspace_root.clone());
    let mut report = mnemosyne_ops::continuity_scan(
        &anchor,
        sidecar_override.as_deref().map(std::path::Path::new),
        order_override.as_deref(),
    )
    .map_err(|e| anyhow!("{e}"))?;
    if let Some(s) = &severity_override {
        let parsed = Severity::from_tag(s.trim())
            .ok_or_else(|| anyhow!("--severity must be `reject`, `warn`, or `info`"))?;
        report.severity = Some(parsed.as_str().to_string());
    }
    let severity = report.severity.as_deref().and_then(Severity::from_tag);
    if json {
        println!("{}", serde_json::to_string(&report)?);
    } else {
        match severity {
            None => println!("continuity gate: disabled ([continuity] table absent)"),
            Some(s) => println!("=== continuity gate ({}) ===", s.as_str()),
        }
        println!(
            "  facts={} order_nodes={} conflict_pairs={} cross_scope(data)={} unordered={}",
            report.facts,
            report.order_nodes,
            report.conflict_pairs_checked,
            report.cross_scope_pairs,
            report.unordered_pairs
        );
        println!("  violations: {}", report.violation_count);
        for v in &report.violations {
            println!("  {}", serde_json::to_string(v)?);
        }
    }
    if matches!(severity, Some(s) if s.is_reject()) && report.violation_count > 0 {
        std::process::exit(1);
    }
    Ok(())
}

/// Round 432 — frame-at-T read projection (`report-frame-view`): the facts a
/// frame holds at a canon point, over the SAME holds-semantics as the
/// continuity gate (R390 single-predicate discipline). Read-only; order and
/// store resolve through the shared `ops::continuity_frame_view` path
/// (Round 435; `--order` bypasses the pin, `--sidecar` for non-dogfood
/// stores). `--branch` scopes the view to one world-line (Round 433;
/// omitted = the default branch).
fn cmd_report_frame_view(args: &[String]) -> Result<()> {
    let mut json = false;
    let mut frame: Option<String> = None;
    let mut branch: Option<String> = None;
    let mut entity: Option<String> = None;
    let mut at: Option<String> = None;
    let mut order_override: Option<String> = None;
    let mut sidecar_override: Option<String> = None;
    let mut iter = args.iter();
    while let Some(a) = iter.next() {
        match a.as_str() {
            "--json" => json = true,
            "--frame" => {
                frame = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--frame missing"))?
                        .clone(),
                )
            }
            "--branch" => {
                branch = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--branch missing"))?
                        .clone(),
                )
            }
            "--entity" => {
                entity = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--entity missing"))?
                        .clone(),
                )
            }
            "--at" => at = Some(iter.next().ok_or_else(|| anyhow!("--at missing"))?.clone()),
            "--order" => {
                order_override = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--order missing"))?
                        .clone(),
                )
            }
            "--sidecar" => {
                sidecar_override = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            other => bail!("unknown flag `{}`", other),
        }
    }
    let frame = frame.ok_or_else(|| anyhow!("--frame arg required"))?;
    let at = at.ok_or_else(|| anyhow!("--at arg required"))?;
    let loaded = workspace_config()?;
    let anchor = loaded
        .config_path
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| loaded.workspace_root.clone());
    let view = mnemosyne_ops::continuity_frame_view(
        &anchor,
        sidecar_override.as_deref().map(std::path::Path::new),
        &frame,
        branch.as_deref(),
        entity.as_deref(),
        &at,
        order_override.as_deref(),
    )
    .map_err(|e| anyhow!("{e}"))?;
    if json {
        println!("{}", serde_json::to_string(&view)?);
    } else {
        let entity_tag = view
            .entity
            .as_deref()
            .map(|e| format!(" entity `{e}`"))
            .unwrap_or_default();
        println!(
            "=== frame `{}` branch `{}`{} at `{}` ===",
            view.frame, view.branch, entity_tag, view.at
        );
        println!(
            "  holding={} not_holding={} unknown={}",
            view.holding_count,
            view.not_holding,
            view.unknown.len()
        );
        for e in &view.holding {
            let to = e
                .canon_to
                .as_deref()
                .map(|t| format!("..{t}"))
                .unwrap_or_default();
            println!("  [{}{}] {}: {}", e.canon_from, to, e.fact_id, e.claim);
        }
        for u in &view.unknown {
            println!("  [unknown under declared order] {u}");
        }
    }
    Ok(())
}

/// Round 437 — entity dossier (`report-entity`): every fact referencing the
/// entity, across all frames and branches — the raw authoring-time view
/// ("all facts about X"). The frame-at-T projection with an entity filter
/// is `report-frame-view --entity`.
fn cmd_report_entity(args: &[String]) -> Result<()> {
    let mut json = false;
    let mut entity: Option<String> = None;
    let mut sidecar_override: Option<String> = None;
    let mut iter = args.iter();
    while let Some(a) = iter.next() {
        match a.as_str() {
            "--json" => json = true,
            "--entity" => {
                entity = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--entity missing"))?
                        .clone(),
                )
            }
            "--sidecar" => {
                sidecar_override = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            other => bail!("unknown flag `{}`", other),
        }
    }
    let entity = entity.ok_or_else(|| anyhow!("--entity arg required"))?;
    let loaded = workspace_config()?;
    let anchor = loaded
        .config_path
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| loaded.workspace_root.clone());
    let dossier = mnemosyne_ops::entity_dossier(
        &anchor,
        sidecar_override.as_deref().map(std::path::Path::new),
        &entity,
    )
    .map_err(|e| anyhow!("{e}"))?;
    if json {
        println!("{}", serde_json::to_string(&dossier)?);
    } else {
        let kind_tag = if dossier.kind.is_empty() {
            String::new()
        } else {
            format!(" ({})", dossier.kind)
        };
        println!(
            "=== entity `{}`{} — {} fact(s) ===",
            dossier.entity_id, kind_tag, dossier.fact_count
        );
        if !dossier.description.is_empty() {
            println!("  {}", dossier.description);
        }
        for f in &dossier.facts {
            let to = f
                .canon_to
                .as_deref()
                .map(|t| format!("..{t}"))
                .unwrap_or_default();
            println!(
                "  [{}{}] {} (frame {} / branch {}): {}",
                f.canon_from, to, f.fact_id, f.frame, f.branch, f.claim
            );
        }
    }
    Ok(())
}

/// R427 — load the `[verifies_catalog]` catalog when configured; `None` when
/// the table is absent (the catalog branch is opt-in). A CONFIGURED catalog
/// that fails to load is an error (fail loud), not a silent skip.
fn load_verifies_catalog_if_configured(
    config: &mnemosyne_config::WorkspaceConfig,
    root: &std::path::Path,
) -> Result<Option<mnemosyne_validate::verifies_linkage::VerifiesCatalog>> {
    match config.verifies_catalog.as_ref() {
        None => Ok(None),
        Some(c) => mnemosyne_validate::verifies_linkage::load_catalog(
            &root.join(&c.path),
            c.sha256.as_deref(),
        )
        .map(Some)
        .map_err(|e| anyhow!("{}", e)),
    }
}

/// R426 — authoritative test-catalog linkage check (SCE field-report P2 + the
/// P5 granularity lint). Every `verifies` binding must match the consumer-
/// generated catalog's declared target section(s) for that test artifact.
/// Opt-in: `--catalog` overrides `[verifies_catalog].path`; absent on both =
/// disabled (exit 0). `reject` + any mismatch => exit 1; uncataloged artifacts
/// are a count, never gating (a partial catalog is legitimate).
fn cmd_validate_verifies_linkage(args: &[String]) -> Result<()> {
    use mnemosyne_config::Severity;
    use mnemosyne_validate::verifies_linkage::{load_catalog, scan_verifies_linkage};
    let mut json = false;
    let mut catalog_override: Option<String> = None;
    let mut severity_override: Option<String> = None;
    let mut iter = args.iter();
    while let Some(a) = iter.next() {
        match a.as_str() {
            "--json" => json = true,
            "--catalog" => {
                catalog_override = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--catalog missing"))?
                        .clone(),
                )
            }
            "--severity" => {
                severity_override = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--severity missing"))?
                        .clone(),
                )
            }
            other => bail!("unknown flag `{}`", other),
        }
    }
    let loaded = workspace_config()?;
    let root = loaded.workspace_root.clone();
    let anchor = loaded
        .config_path
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| root.clone());
    let cfg_section = loaded.config.verifies_catalog.as_ref();
    let catalog_path = catalog_override
        .clone()
        .or_else(|| cfg_section.map(|c| c.path.clone()));
    let Some(catalog_rel) = catalog_path else {
        if json {
            println!("{}", serde_json::json!({ "enabled": false }));
        } else {
            println!("verifies-linkage: disabled ([verifies_catalog] unset, no --catalog)");
        }
        return Ok(());
    };
    let severity = match severity_override {
        Some(s) => Severity::from_tag(s.trim())
            .ok_or_else(|| anyhow!("--severity must be `reject`, `warn`, or `info`"))?,
        None => cfg_section.map(|c| c.severity).unwrap_or(Severity::Reject),
    };
    let catalog_abs = root.join(&catalog_rel);
    // The sha256 pin applies only to the CONFIG path — a `--catalog` override
    // points at a different file the pin makes no claim about (R428).
    let pin = if catalog_override.is_some() {
        None
    } else {
        cfg_section.and_then(|c| c.sha256.as_deref())
    };
    let catalog = load_catalog(&catalog_abs, pin).map_err(|e| anyhow!("{}", e))?;
    let atomic_path = mnemosyne_ops::cascade::resolve_sidecar(&anchor, None)?;
    let store = AtomicStore::load(&atomic_path)
        .with_context(|| format!("atomic store load: {}", atomic_path.display()))?;
    let snapshot = mnemosyne_core::AtomicStoreView::snapshot(&store);
    let report = scan_verifies_linkage(&snapshot, &catalog);
    if json {
        println!(
            "{}",
            serde_json::json!({
                "enabled": true,
                "catalog": catalog_rel,
                "severity": severity.as_str(),
                "examined": report.examined,
                "uncataloged": report.uncataloged,
                "mismatch_count": report.mismatches.len(),
                "mismatches": report.mismatches.iter().map(|m| serde_json::json!({
                    "section_id": m.section_id,
                    "file": m.file,
                    "symbol": m.symbol,
                    "declared": m.declared,
                    "kind": m.kind.as_str(),
                })).collect::<Vec<_>>(),
            })
        );
    } else {
        println!("=== verifies-linkage ({}) ===", severity.as_str());
        println!(
            "  examined={} mismatches={} uncataloged={}",
            report.examined,
            report.mismatches.len(),
            report.uncataloged
        );
        for m in &report.mismatches {
            let sym = m
                .symbol
                .as_deref()
                .map(|s| format!(":{s}"))
                .unwrap_or_default();
            println!(
                "  [{}] {}{} bound to section {} but catalog declares {:?}",
                m.kind.as_str(),
                m.file,
                sym,
                m.section_id,
                m.declared
            );
        }
    }
    if severity.is_reject() && !report.mismatches.is_empty() {
        std::process::exit(1);
    }
    Ok(())
}

/// Unified spec ↔ fact ↔ code projection (read-only L3 view). Joins, per
/// section: the coverage class (single-sourced through
/// [`mnemosyne_validate::code_refs::classify_coverage`] so the map never drifts
/// from `report-coverage` / `validate-code-refs`), the external-spec provenance
/// (`normative_excerpt` anchor_url + source_revision), the Path B bindings, the
/// spec-revision drift flag (`validate-spec-drift`), and the reverse citation
/// count (how many code sites cite the section). No authoritative state of its
/// own — every field is projected from the atomic store plus a code-citation
/// scan. Feeds spec-map visualization (ToC overlay / coverage / citation
/// density / drift in one graph). Citation data requires
/// `[plugins.set_equality_validator]`; when absent, the rest of the map still
/// projects with zero citation counts.
fn cmd_report_spec_map(args: &[String]) -> Result<()> {
    let mut json = false;
    for a in args {
        match a.as_str() {
            "--json" => json = true,
            other => bail!("unknown flag `{}`", other),
        }
    }
    let loaded = workspace_config()?;
    let root = loaded.workspace_root.clone();
    let anchor = loaded
        .config_path
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| root.clone());
    let atomic_path = mnemosyne_ops::cascade::resolve_sidecar(&anchor, None)?;
    let store = AtomicStore::load(&atomic_path)
        .with_context(|| format!("atomic store load: {}", atomic_path.display()))?;
    let snapshot = mnemosyne_core::AtomicStoreView::snapshot(&store);

    // Coverage class per section — single-sourced through `classify_coverage`.
    let coverage = mnemosyne_validate::code_refs::classify_coverage(&snapshot);
    let mut class_of: std::collections::HashMap<&str, &'static str> =
        std::collections::HashMap::new();
    for id in &coverage.implemented {
        class_of.insert(id.as_str(), "implemented");
    }
    for id in &coverage.normative_gap {
        class_of.insert(id.as_str(), "normative_gap");
    }
    for id in &coverage.informative_exempt {
        class_of.insert(id.as_str(), "informative_exempt");
    }
    for id in &coverage.removed_excluded {
        class_of.insert(id.as_str(), "removed_excluded");
    }

    // Spec-revision drift set — only for external-spec mirror workspaces
    // ([workspace.spec_source] present); absent => no current rev to diff.
    let spec_source = loaded.config.workspace.spec_source.as_ref();
    let drift_ids: BTreeSet<String> = match spec_source {
        Some(s) => mnemosyne_validate::scan_spec_drift(&store, &s.revision)
            .into_iter()
            .map(|v| v.section_id)
            .collect(),
        None => BTreeSet::new(),
    };

    // Reverse citation index (citation density). Optional: requires the
    // set-equality validator plugin; absent => empty index, the rest projects.
    let citation_index = match loaded
        .config
        .plugins
        .as_ref()
        .and_then(|p| p.set_equality_validator.as_ref())
    {
        Some(cfg) => {
            let validator = SetEqualityValidator {
                config: cfg.clone(),
                entry_id_prefix: cli_schema()?.entry_id_prefix.clone(),
                orphan_ledger: loaded.config.orphan_ledger.clone(),
                symbol_resolvers: build_symbol_resolver_map(&loaded.config),
                filter_id: None,
            };
            validator.citation_index(&root, &snapshot)?
        }
        None => std::collections::BTreeMap::new(),
    };

    // Project per-section rows in BTreeMap (section-id sorted) order.
    let mut sections_json: Vec<serde_json::Value> = Vec::with_capacity(store.sections.len());
    for (section_id, sec) in &store.sections {
        let class = class_of
            .get(section_id.as_str())
            .copied()
            .unwrap_or("unknown");
        let status = sec
            .skeleton
            .decision_status
            .map(|s| format!("{:?}", s).to_lowercase())
            .unwrap_or_else(|| "active".to_string());
        let spec = sec.normative_excerpt.as_ref().map(|e| {
            serde_json::json!({
                "anchor_url": e.anchor_url,
                "source_revision": e.source_revision,
            })
        });
        let bindings: Vec<serde_json::Value> = sec
            .bindings
            .iter()
            .map(|b| {
                serde_json::json!({
                    "file": b.file,
                    "symbol": b.symbol,
                    "kind": b.kind.as_str(),
                })
            })
            .collect();
        let sites = citation_index.get(section_id);
        let citation_count = sites.map(Vec::len).unwrap_or(0);
        let cited_from: Vec<serde_json::Value> = sites
            .map(|v| {
                v.iter()
                    .map(|c| serde_json::json!({ "file": c.file, "line": c.line }))
                    .collect()
            })
            .unwrap_or_default();
        sections_json.push(serde_json::json!({
            "section_id": section_id,
            "title": sec.skeleton.title,
            "parent_doc": sec.skeleton.parent_doc,
            "parent_section": sec.skeleton.parent_section,
            "decision_status": status,
            "coverage_class": class,
            "drift": drift_ids.contains(section_id),
            "spec": spec,
            // EPUB-SSOT pointer (R393). Serialized via the EpubLocator struct
            // definition (single source) so the viewer resolves the section's
            // rendered position from this projection, not a 2nd store read;
            // `null` when no EPUB is mirrored, `cfi` omitted when absent.
            "epub_locator": sec.epub_locator,
            "bindings": bindings,
            "citation_count": citation_count,
            "cited_from": cited_from,
        }));
    }

    let with_excerpt = store
        .sections
        .values()
        .filter(|s| s.normative_excerpt.is_some())
        .count();
    let with_locator = store
        .sections
        .values()
        .filter(|s| s.epub_locator.is_some())
        .count();
    let total_cites: usize = citation_index.values().map(Vec::len).sum();

    if json {
        let spec_source_json = spec_source.map(|s| {
            serde_json::json!({
                "url": s.url,
                "revision": s.revision,
                "fetched_sha256": s.fetched_sha256,
                "fetched_at": s.fetched_at,
            })
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "spec_source": spec_source_json,
                "summary": {
                    "total_sections": store.sections.len(),
                    "with_excerpt": with_excerpt,
                    "with_epub_locator": with_locator,
                    "coverage_ratio": coverage.coverage_ratio(),
                    "by_class": {
                        "implemented": coverage.implemented.len(),
                        "normative_gap": coverage.normative_gap.len(),
                        "informative_exempt": coverage.informative_exempt.len(),
                        "removed_excluded": coverage.removed_excluded.len(),
                    },
                    "drifted": drift_ids.len(),
                    "total_citations": total_cites,
                },
                "sections": sections_json,
            }))?
        );
    } else {
        println!("=== spec map ===");
        println!(
            "  sections: {} (with spec excerpt: {}, with EPUB locator: {})",
            store.sections.len(),
            with_excerpt,
            with_locator
        );
        match coverage.coverage_ratio() {
            Some(ratio) => println!(
                "  coverage: {:.1}% ({}/{} applicable)",
                ratio * 100.0,
                coverage.implemented.len(),
                coverage.applicable()
            ),
            None => println!("  coverage: n/a (0 applicable sections)"),
        }
        println!(
            "  by class: implemented={} gap={} informative={} removed={}",
            coverage.implemented.len(),
            coverage.normative_gap.len(),
            coverage.informative_exempt.len(),
            coverage.removed_excluded.len(),
        );
        println!("  spec-revision drift: {}", drift_ids.len());
        println!(
            "  citations: {} across {} section(s)",
            total_cites,
            citation_index.len()
        );
        println!("  (run with --json for the full per-section spec↔fact↔code map)");
    }
    Ok(())
}

/// Path B curation support: emit the proposed `§<id>.bindings`
/// symbol sets derived from current code citations, for maintainer
/// ratification. Read-only — never mutates the store. Pair with
/// `add-section-binding` to register the ratified sets.
fn cmd_propose_implementations(args: &[String]) -> Result<()> {
    let mut json = false;
    let mut section_filter: Option<String> = None;
    let mut iter = args.iter();
    while let Some(a) = iter.next() {
        match a.as_str() {
            "--json" => json = true,
            "--section" => {
                section_filter = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--section missing value"))?
                        .clone(),
                );
            }
            other => bail!("unknown flag `{}`", other),
        }
    }
    // Accept both the sigil-prefixed and bare form for --section.
    let section_filter = section_filter.map(|s| s.trim_start_matches('§').to_string());

    let loaded = workspace_config()?;
    let cfg = match loaded
        .config
        .plugins
        .as_ref()
        .and_then(|p| p.set_equality_validator.as_ref())
    {
        Some(c) => c,
        None => bail!("[plugins.set_equality_validator] not configured in mnemosyne.toml"),
    };

    let prefix = cli_schema()?.entry_id_prefix.clone();
    let root = loaded.workspace_root.clone();
    // Sidecar resolution discovers config from the anchor (the toml's dir),
    // not the resolved root, so a subdir-rooted ledger finds its [atomic].
    let anchor = loaded
        .config_path
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| root.clone());
    let atomic_path = mnemosyne_ops::cascade::resolve_sidecar(&anchor, None)?;
    let store = AtomicStore::load(&atomic_path)
        .with_context(|| format!("atomic store load: {}", atomic_path.display()))?;
    let symbol_resolvers = build_symbol_resolver_map(&loaded.config);
    let validator = SetEqualityValidator {
        config: cfg.clone(),
        entry_id_prefix: prefix,
        orphan_ledger: loaded.config.orphan_ledger.clone(),
        symbol_resolvers,
        filter_id: None,
    };
    let snapshot = mnemosyne_core::AtomicStoreView::snapshot(&store);
    let mut proposals = validator.propose_implementations(&root, &snapshot)?;
    if let Some(ref sec) = section_filter {
        proposals.retain(|p| p.section_id == *sec);
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&proposals)?);
        return Ok(());
    }

    println!("=== mnemosyne-cli propose-implementations ===");
    println!(
        "{} (section,file) proposal(s){}",
        proposals.len(),
        section_filter
            .as_ref()
            .map(|s| format!(" for §{}", s))
            .unwrap_or_default()
    );
    println!("# Review each set as design intent, then run the registration commands below.");
    for p in &proposals {
        let syms: Vec<&str> = p.symbols.iter().map(String::as_str).collect();
        println!(
            "\n§{}  {}  symbols={:?}  unresolved_cites={}",
            p.section_id, p.file, syms, p.unresolved_citations
        );
        if p.symbols.is_empty() {
            println!(
                "  mnemosyne-cli add-section-binding --section §{} --file {} --kind implements",
                p.section_id, p.file
            );
        }
        for s in &p.symbols {
            println!(
                "  mnemosyne-cli add-section-binding --section §{} --file {} --kind implements --symbol {}",
                p.section_id, p.file, s
            );
        }
    }
    Ok(())
}

fn cmd_validate_code_refs(args: &[String]) -> Result<()> {
    let mut json = false;
    let mut severity_missing_override: Option<String> = None;
    let mut severity_binding_override: Option<String> = None;
    let mut severity_coverage_override: Option<String> = None;
    let mut severity_verification_override: Option<String> = None;
    let mut severity_classification_override: Option<String> = None;
    let mut severity_blanket_override: Option<String> = None;
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
            "--severity-coverage" => {
                severity_coverage_override = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--severity-coverage missing value"))?
                        .clone(),
                );
            }
            "--severity-verification" => {
                severity_verification_override = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--severity-verification missing value"))?
                        .clone(),
                );
            }
            "--severity-classification" => {
                severity_classification_override = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--severity-classification missing value"))?
                        .clone(),
                );
            }
            "--severity-blanket" => {
                severity_blanket_override = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--severity-blanket missing value"))?
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

    // Resolve each axis severity: a `--severity-*` flag (parsed once via
    // `Severity::from_tag` — the single CLI validation point) overrides the
    // config `Severity` (already serde-validated at load).
    let parse_sev = |flag: &str, raw: &str| -> Result<Severity> {
        Severity::from_tag(raw.trim()).ok_or_else(|| {
            anyhow!(
                "invalid {} `{}` — expected one of: reject | warn | info",
                flag,
                raw
            )
        })
    };
    let severity_missing = match &severity_missing_override {
        Some(s) => parse_sev("--severity-missing", s)?,
        None => cfg.severity_missing,
    };
    let severity_binding = match &severity_binding_override {
        Some(s) => parse_sev("--severity-binding", s)?,
        None => cfg.severity_binding,
    };
    // severity_coverage (Round 385) inherits severity_binding when unset, so
    // the dogfood (no [plugins.set_equality_validator].severity_coverage) keeps
    // the Round 269 behaviour of coverage gating with the binding severity.
    // Precedence: --severity-coverage flag > config.severity_coverage >
    // resolved severity_binding.
    let severity_coverage = match &severity_coverage_override {
        Some(s) => parse_sev("--severity-coverage", s)?,
        None => cfg.severity_coverage.unwrap_or(severity_binding),
    };
    // Verify axis (R413) is opt-in: `None` = disabled. A CLI override enables
    // it for the run (and is injected into the validator config below so the
    // scan actually emits VerificationMissing); otherwise the config value
    // governs.
    let severity_verification: Option<Severity> = match &severity_verification_override {
        Some(s) => Some(parse_sev("--severity-verification", s)?),
        None => cfg.severity_verification,
    };
    let severity_classification: Option<Severity> = match &severity_classification_override {
        Some(s) => Some(parse_sev("--severity-classification", s)?),
        None => cfg.severity_classification,
    };
    let severity_blanket: Option<Severity> = match &severity_blanket_override {
        Some(s) => Some(parse_sev("--severity-blanket", s)?),
        None => cfg.severity_blanket,
    };
    let severity_inventory = match &severity_inventory_override {
        Some(s) => parse_sev("--severity-inventory", s)?,
        None => cfg.severity_inventory,
    };

    let prefix = cli_schema()?.entry_id_prefix.clone();
    let root = loaded.workspace_root.clone();
    // Sidecar resolution discovers config from the anchor (the toml's dir),
    // not the resolved root, so a subdir-rooted ledger finds its [atomic].
    let anchor = loaded
        .config_path
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| root.clone());

    let atomic_path = mnemosyne_ops::cascade::resolve_sidecar(&anchor, None)?;
    let store = AtomicStore::load(&atomic_path)
        .with_context(|| format!("atomic store load: {}", atomic_path.display()))?;

    // Build the SymbolResolver registry from
    // [plugins.symbol_resolver.<lang>]. Only InProcess transport returns
    // real answers; Mcp/Cli surface ResolverError::NotImplemented at
    // call time until R308+ wires real backends. Unknown in-process
    // backend names log a warning and are skipped.
    let symbol_resolvers = build_symbol_resolver_map(&loaded.config);

    // Call the validator directly with typed return — cli owns the
    // concrete SetEqualityValidator construction so the registry
    // indirection adds no value here. The registry dispatch path is
    // still exercised end-to-end in validator_trait_dispatch.rs as
    // proof that ErasedValidator object-safe dispatch works for
    // dynamic-plugin scenarios.
    // Inject the resolved verify-axis severity so a CLI `--severity-verification`
    // override ENABLES the axis for this run (the scan only emits
    // VerificationMissing when `config.severity_verification.is_some()`).
    let mut validator_cfg = cfg.clone();
    validator_cfg.severity_verification = severity_verification;
    validator_cfg.severity_classification = severity_classification;
    validator_cfg.severity_blanket = severity_blanket;
    let validator = SetEqualityValidator {
        config: validator_cfg,
        entry_id_prefix: prefix.clone(),
        orphan_ledger: loaded.config.orphan_ledger.clone(),
        symbol_resolvers,
        filter_id: filter_id.clone(),
    };
    let store_view: &dyn mnemosyne_core::AtomicStoreView = &store;
    let ctx = mnemosyne_core::ValidationContext {
        workspace_root: &root,
        atomic_sidecar: &atomic_path,
        store: store_view,
    };
    let violations =
        <SetEqualityValidator as mnemosyne_core::Validator>::validate(&validator, &ctx)
            .map_err(|e| anyhow!("SetEqualityValidator dispatch failed: {}", e))?;

    // Per-class counting from typed enum — `CodeRefViolation::kind_tag`
    // is the stable string key shared with `validate-code-refs --json`
    // output. Pattern match is exhaustive at the type level.
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    for v in &violations {
        *counts.entry(v.kind_tag()).or_insert(0) += 1;
    }
    let get = |k: &str| counts.get(k).copied().unwrap_or(0);
    let missing_count = get("missing");
    let section_missing_count = get("section_missing");
    let citation_unbound_count = get("citation_unbound");
    let binding_unbacked_count = get("binding_unbacked");
    let decay_count = get("decay");
    let impl_missing_count = get("impl_missing");
    let verification_missing_count = get("verification_missing");
    let misclassified_coverage_count = get("misclassified_coverage");
    let blanket_verifies_count = get("blanket_verifies");
    // R425 / SCE P4 — standing informational count: verifies bindings whose
    // claims are not yet Confirmed (proposed / refuted / stale). Independent of
    // any severity knob — an existence-green gate that hides a semantic gap
    // breeds complacency, so the gap stays visible by default.
    let unconfirmed_verifies_count = {
        let snapshot = mnemosyne_core::AtomicStoreView::snapshot(&store);
        let catalog = load_verifies_catalog_if_configured(&loaded.config, &root)?;
        mnemosyne_validate::confirmation::scan_confirmation_gate(
            &snapshot,
            &store,
            &root,
            catalog.as_ref(),
        )
        .len()
    };
    let inventory_missing_count = get("inventory_missing");
    let inventory_deprecated_count = get("inventory_deprecated");
    let symbol_mismatch_count = get("symbol_mismatch");
    let inventory_count = inventory_missing_count + inventory_deprecated_count;
    let hallucination_count = missing_count + section_missing_count;
    // Round 385 — coverage split. The binding bucket is the per-edge axis:
    // CitationUnbound + BindingUnbacked (cite ↔ file) + SymbolMismatch
    // (cite ↔ symbol). ImplementationMissing (an Active section with zero
    // implementations) is a *coverage* claim about the section, not an edge,
    // and moves to its own `severity_coverage` axis — spec-mirror sections are
    // mostly prose and legitimately uncited, so coverage must be downgradable
    // independently of binding (the Round 269 deferred split).
    let binding_count = citation_unbound_count + binding_unbacked_count + symbol_mismatch_count;
    let coverage_count = impl_missing_count;

    if json {
        // Flat per-violation shape via `CodeRefViolation::to_cli_json` —
        // the stable CLI JSON contract. The default Serialize derive on
        // `CodeRefViolation` produces a nested tagged form intended for
        // the `ErasedValidator` dispatch boundary; cli uses the flat
        // shape so external consumers see one predictable layout.
        let view: Vec<serde_json::Value> = violations.iter().map(|v| v.to_cli_json()).collect();
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
            "binding_unbacked_count": binding_unbacked_count,
            "impl_missing_count": impl_missing_count,
            "verification_missing_count": verification_missing_count,
            "misclassified_coverage_count": misclassified_coverage_count,
            "blanket_verifies_count": blanket_verifies_count,
            "unconfirmed_verifies": unconfirmed_verifies_count,
            "decay_count": decay_count,
            "inventory_missing_count": inventory_missing_count,
            "inventory_deprecated_count": inventory_deprecated_count,
            "severity_missing": severity_missing,
            "severity_binding": severity_binding,
            "severity_coverage": severity_coverage,
            "severity_verification": severity_verification,
            "severity_classification": severity_classification,
            "severity_blanket": severity_blanket,
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
            println!(
                "inventory_prefixes={:?} (Round 275 axis)",
                cfg.inventory_prefixes
            );
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
 citation_unbound={} binding_unbacked={} impl_missing={} verification_missing={} \
 misclassified_coverage={} blanket_verifies={} decay={} \
 inv_missing={} inv_deprecated={} unconfirmed_verifies={} \
 (severity_missing={} severity_binding={} severity_coverage={} severity_verification={} \
 severity_classification={} severity_blanket={} severity_inventory={})",
            violations.len(),
            missing_count,
            section_missing_count,
            citation_unbound_count,
            binding_unbacked_count,
            impl_missing_count,
            verification_missing_count,
            misclassified_coverage_count,
            blanket_verifies_count,
            decay_count,
            inventory_missing_count,
            inventory_deprecated_count,
            unconfirmed_verifies_count,
            severity_missing.as_str(),
            severity_binding.as_str(),
            severity_coverage.as_str(),
            severity_verification.map(Severity::as_str).unwrap_or("off"),
            severity_classification
                .map(Severity::as_str)
                .unwrap_or("off"),
            severity_blanket.map(Severity::as_str).unwrap_or("off"),
            severity_inventory.as_str(),
        );
        // `CodeRefViolation: Display` renders the legacy TTY shape
        // (`[<kind>] <file>:<line> <entry_id>` for citations, etc.).
        for v in &violations {
            println!(" {}", v);
        }
    }

    // Reject gates by defect class — each class gated by its
    // own severity flag. Decay never rejects (informational).
    let mut reject_msgs: Vec<String> = Vec::new();
    if hallucination_count > 0 && severity_missing.is_reject() {
        reject_msgs.push(format!(
            "{} hallucination-class citation(s) — Missing={} SectionMissing={} \
 (severity_missing=reject)",
            hallucination_count, missing_count, section_missing_count,
        ));
    }
    if binding_count > 0 && severity_binding.is_reject() {
        reject_msgs.push(format!(
            "{} binding-class violation(s) — CitationUnbound={} BindingUnbacked={} \
 SymbolMismatch={} (severity_binding=reject)",
            binding_count, citation_unbound_count, binding_unbacked_count, symbol_mismatch_count,
        ));
    }
    if coverage_count > 0 && severity_coverage.is_reject() {
        reject_msgs.push(format!(
            "{} coverage-class violation(s) — ImplementationMissing={} \
 (severity_coverage=reject)",
            coverage_count, impl_missing_count,
        ));
    }
    if verification_missing_count > 0 && severity_verification == Some(Severity::Reject) {
        reject_msgs.push(format!(
            "{} verification-class violation(s) — VerificationMissing={} \
 (severity_verification=reject)",
            verification_missing_count, verification_missing_count,
        ));
    }
    if misclassified_coverage_count > 0 && severity_classification == Some(Severity::Reject) {
        reject_msgs.push(format!(
            "{} classification-class violation(s) — MisclassifiedCoverage={} \
 (severity_classification=reject)",
            misclassified_coverage_count, misclassified_coverage_count,
        ));
    }
    if blanket_verifies_count > 0 && severity_blanket == Some(Severity::Reject) {
        reject_msgs.push(format!(
            "{} blanket-class violation(s) — BlanketVerifies={} \
 (severity_blanket=reject)",
            blanket_verifies_count, blanket_verifies_count,
        ));
    }
    if inventory_count > 0 && severity_inventory.is_reject() {
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

// ============================================================================
// validate-spec-drift — RFC-001 UC-1 "B2" spec-revision label-drift scan.
// ============================================================================
fn cmd_validate_spec_drift(args: &[String]) -> Result<()> {
    let mut json = false;
    let mut severity_override: Option<String> = None;
    let mut iter = args.iter();
    while let Some(a) = iter.next() {
        match a.as_str() {
            "--json" => json = true,
            "--severity" => {
                severity_override = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--severity missing value"))?
                        .clone(),
                );
            }
            other => bail!("unknown flag `{}`", other),
        }
    }

    let loaded = workspace_config()?;
    // The scan only applies to external-spec mirror workspaces. Absent
    // [workspace.spec_source] => no current rev to diff against => no-op
    // exit 0 (mirrors validate-code-refs' skip-when-unconfigured contract).
    let spec_source = match &loaded.config.workspace.spec_source {
        Some(s) => s,
        None => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                    "primitive": "validate-spec-drift",
                    "status": "skipped",
                    "reason": "[workspace.spec_source] not configured in mnemosyne.toml",
                    })
                );
            } else {
                println!("=== mnemosyne-cli validate-spec-drift ===");
                println!(
                    "skipped — [workspace.spec_source] not configured (no external spec mirror)"
                );
            }
            return Ok(());
        }
    };

    // Severity: --severity flag overrides [spec_drift].severity, which
    // defaults to warn when the table is absent.
    let configured = loaded
        .config
        .spec_drift
        .as_ref()
        .map(|s| s.severity)
        .unwrap_or(Severity::Warn);
    let severity = match &severity_override {
        Some(s) => Severity::from_tag(s.trim()).ok_or_else(|| {
            anyhow!(
                "invalid --severity `{}` — expected one of: reject | warn | info",
                s
            )
        })?,
        None => configured,
    };

    let workspace_revision = spec_source.revision.clone();
    let root = loaded.workspace_root.clone();
    // Sidecar resolution discovers config from the anchor (the toml's dir),
    // not the resolved root, so a subdir-rooted ledger finds its [atomic].
    let anchor = loaded
        .config_path
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| root.clone());
    let atomic_path = mnemosyne_ops::cascade::resolve_sidecar(&anchor, None)?;
    let store = AtomicStore::load(&atomic_path)
        .with_context(|| format!("atomic store load: {}", atomic_path.display()))?;

    let violations = mnemosyne_validate::scan_spec_drift(&store, &workspace_revision);

    if json {
        let view: Vec<serde_json::Value> = violations.iter().map(|v| v.to_cli_json()).collect();
        println!(
            "{}",
            serde_json::json!({
            "primitive": "validate-spec-drift",
            "spec_url": spec_source.url,
            "workspace_revision": workspace_revision,
            "valid_section_count": store.sections.len(),
            "drift_count": violations.len(),
            "severity": severity,
            "violations": view,
            })
        );
    } else {
        println!("=== mnemosyne-cli validate-spec-drift ===");
        println!(
            "spec_url={:?} workspace_revision={:?} sections={} severity={}",
            spec_source.url,
            workspace_revision,
            store.sections.len(),
            severity.as_str(),
        );
        println!("drift: total={}", violations.len());
        // Bare section_id (no `§` sigil) — the CLI never renders a literal
        // section citation the R255 pre-commit hook would scan.
        for v in &violations {
            println!(
                " [drift] {} anchored_rev={:?} workspace_rev={:?}",
                v.section_id, v.section_revision, v.workspace_revision
            );
        }
    }

    // Single configurable axis: reject => exit 1 on any drift. warn/info
    // print the findings and exit 0 (CI decides gating; partial migration
    // is a legitimate intermediate state).
    if !violations.is_empty() && severity.is_reject() {
        bail!(
            "{} spec-revision drift violation(s) — Active Section(s) trailing workspace \
 revision {:?} (severity=reject)",
            violations.len(),
            workspace_revision,
        );
    }
    Ok(())
}

/// SHA-256 (lowercase hex) of a byte slice — re-hashes the committed EPUB file
/// against the pinned `[workspace.spec_source].epub_sha256` (R405).
fn sha256_hex_bytes(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(bytes);
    let out = h.finalize();
    let mut s = String::with_capacity(out.len() * 2);
    for b in out {
        use std::fmt::Write;
        let _ = write!(&mut s, "{:02x}", b);
    }
    s
}

// ============================================================================
// validate-content-drift — R404 offline content-integrity scan + R405 EPUB-file
// provenance check.
// ============================================================================
fn cmd_validate_content_drift(args: &[String]) -> Result<()> {
    let mut json = false;
    let mut severity_override: Option<String> = None;
    let mut iter = args.iter();
    while let Some(a) = iter.next() {
        match a.as_str() {
            "--json" => json = true,
            "--severity" => {
                severity_override = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--severity missing value"))?
                        .clone(),
                );
            }
            other => bail!("unknown flag `{}`", other),
        }
    }

    let loaded = workspace_config()?;
    // Severity: --severity flag overrides [content_drift].severity, which
    // defaults to reject when the table is absent.
    let configured = loaded
        .config
        .content_drift
        .as_ref()
        .map(|c| c.severity)
        .unwrap_or(Severity::Reject);
    let severity = match &severity_override {
        Some(s) => Severity::from_tag(s.trim()).ok_or_else(|| {
            anyhow!(
                "invalid --severity `{}` — expected one of: reject | warn | info",
                s
            )
        })?,
        None => configured,
    };

    let root = loaded.workspace_root.clone();
    let anchor = loaded
        .config_path
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| root.clone());
    let atomic_path = mnemosyne_ops::cascade::resolve_sidecar(&anchor, None)?;
    let store = AtomicStore::load(&atomic_path)
        .with_context(|| format!("atomic store load: {}", atomic_path.display()))?;

    let violations = mnemosyne_validate::scan_content_drift(&store);
    // Unrevalidatable (empty-hash) excerpts are NOT drift — single-sourced
    // from the R402 backfill report and surfaced here only for context.
    let unrevalidatable = store.excerpt_hash_backfill_report().rows.len();

    // EPUB-file provenance (R405): when [workspace.spec_source].epub_path +
    // epub_sha256 are pinned, re-hash the committed EPUB offline and compare.
    // A swap/update or a missing file is the more fundamental signal — a
    // drifted EPUB makes every excerpt suspect even when each still matches
    // its own (now stale) text_sha256. Unset (or no spec_source) => skipped.
    let epub = loaded.config.workspace.spec_source.as_ref().and_then(|s| {
        match (&s.epub_path, &s.epub_sha256) {
            (Some(p), Some(pinned)) => Some((p.clone(), pinned.clone())),
            _ => None,
        }
    });
    let mut epub_checked = false;
    let mut epub_drift: Option<String> = None;
    let mut epub_computed: Option<String> = None;
    let mut epub_pinned: Option<String> = None;
    if let Some((rel, pinned)) = epub {
        epub_checked = true;
        epub_pinned = Some(pinned.clone());
        match std::fs::read(root.join(&rel)) {
            Ok(bytes) => {
                let computed = sha256_hex_bytes(&bytes);
                if computed != pinned {
                    epub_drift = Some(format!(
                        "committed EPUB `{rel}` hash diverges from pinned epub_sha256 (swapped/updated)"
                    ));
                }
                epub_computed = Some(computed);
            }
            Err(_) => {
                epub_drift = Some(format!("committed EPUB missing at epub_path `{rel}`"));
            }
        }
    }

    if json {
        let view: Vec<serde_json::Value> = violations.iter().map(|v| v.to_cli_json()).collect();
        let epub_json = if epub_checked {
            serde_json::json!({
                "checked": true,
                "status": if epub_drift.is_some() { "drift" } else { "clean" },
                "reason": epub_drift,
                "computed_sha256": epub_computed,
                "pinned_sha256": epub_pinned,
            })
        } else {
            serde_json::json!({ "checked": false })
        };
        println!(
            "{}",
            serde_json::json!({
            "primitive": "validate-content-drift",
            "section_count": store.sections.len(),
            "drift_count": violations.len(),
            "unrevalidatable_count": unrevalidatable,
            "epub_file": epub_json,
            "severity": severity,
            "violations": view,
            })
        );
    } else {
        println!("=== mnemosyne-cli validate-content-drift ===");
        println!(
            "sections={} severity={} unrevalidatable={}",
            store.sections.len(),
            severity.as_str(),
            unrevalidatable,
        );
        println!("drift: total={}", violations.len());
        // Bare section_id (no `§` sigil) — the CLI never renders a literal
        // section citation the R255 pre-commit hook would scan.
        for v in &violations {
            println!(
                " [drift] {} declared={} computed={}",
                v.section_id, v.declared_sha256, v.computed_sha256
            );
        }
        if epub_checked {
            match &epub_drift {
                Some(reason) => println!(" [epub-drift] {reason}"),
                None => println!("epub_file: clean (committed EPUB matches pinned epub_sha256)"),
            }
        } else {
            println!("epub_file: not pinned (no [workspace.spec_source].epub_path)");
        }
    }

    // reject => exit 1 on any content drift OR EPUB-file drift. warn/info print
    // and exit 0. Unrevalidatable never gates (backfill work-list, not corruption).
    let gated = !violations.is_empty() || epub_drift.is_some();
    if gated && severity.is_reject() {
        bail!(
            "content-integrity drift (severity=reject): {} excerpt(s) mismatched their text_sha256{}",
            violations.len(),
            match &epub_drift {
                Some(r) => format!("; EPUB-file drift: {r}"),
                None => String::new(),
            },
        );
    }
    Ok(())
}
