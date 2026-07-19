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
use mnemosyne_cli::{atomic_cli, CliError};

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
/// (or `.mnemosyne/config.toml`); the loaded config provides the
/// `workspace.root` override + the `[atomic]` sidecar path + the schema
/// preset (the `workspace.docs`/`default_doc` markdown model was removed R400).
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
        // The atomic-mutate path already wrote its own formatted error (the
        // json blob / `FAILED` header), so `main` only sets the exit code —
        // the detail is not printed twice (Round 684 — DEBT-DOUBLE-STDERR).
        // Every other failure is printed here, exactly once. The disposition
        // is a typed variant threaded up from the command, not a marker
        // recovered by `downcast`.
        Err(CliError::AlreadyReported) => ExitCode::FAILURE,
        Err(CliError::Message(e)) => {
            eprintln!("error: {:#}", e);
            ExitCode::FAILURE
        }
    }
}

/// Per-invocation dispatch context handed to every command's `run` fn.
///
/// It carries the raw argv slice rather than pre-split arguments so that
/// `anchor()` stays *lazy*: resolving the workspace anchor walks the tree
/// looking for `mnemosyne.toml` and fails when there is none, so it must run
/// only inside the closures that actually reach the store — never for
/// `--help`, `--version`, or an unknown verb.
struct Ctx<'a> {
    prog: &'a str,
    args: &'a [String],
}

impl<'a> Ctx<'a> {
    /// The invoked command's own arguments (everything after the verb).
    ///
    /// `run` constructs a `Ctx` only once `args[1]` (the verb) is known to
    /// exist, so `args` always holds at least two elements here and the slice
    /// is in bounds — a bare invocation returns before this point.
    fn rest(&self) -> &'a [String] {
        &self.args[2..]
    }

    /// The workspace anchor directory, resolved on demand.
    fn anchor(&self) -> Result<PathBuf> {
        workspace_anchor()
    }
}

/// A help section header plus the preamble lines that belong to the header
/// itself rather than to any one command under it.
struct Group {
    title: &'static str,
    preamble: &'static [&'static str],
}

/// One command: the verb, its help text, and its behavior — in one place.
///
/// The dispatch `match` and `print_help` used to be two hand-maintained lists
/// of the same 83 verbs, kept in agreement only by a test that parsed this
/// source file to detect their drift. Both are now *derived* from
/// [`COMMANDS`]: dispatch looks a verb up in it, help renders it. They cannot
/// disagree, because there is no second list to disagree with — a verb that is
/// dispatched is documented by construction.
struct Command {
    /// The verb as typed.
    name: &'static str,
    /// Additional verbs dispatching to the same `run` (e.g. `-h` / `help`).
    aliases: &'static [&'static str],
    /// Section this command is listed under; `None` = the leading ungrouped
    /// block. The header prints when the group changes.
    group: Option<&'static Group>,
    /// Print a blank line before this command's first usage line (a
    /// readability break *within* a group).
    blank_before: bool,
    /// Usage forms, each rendered as ` {prog} {usage}`. Multiple entries = one
    /// verb with several call shapes (`query`).
    usage: &'static [&'static str],
    /// Continuation/annotation lines, printed verbatim under the usage forms.
    notes: &'static [&'static str],
    /// The command's behavior.
    run: fn(&Ctx) -> Result<(), CliError>,
}

impl Command {
    /// Does `verb` name this command, by name or by alias?
    fn matches(&self, verb: &str) -> bool {
        self.name == verb || self.aliases.contains(&verb)
    }
}

fn run(args: &[String]) -> Result<(), CliError> {
    let prog = args.first().map(String::as_str).unwrap_or("mnemosyne-cli");
    // A bare invocation is the discovery act — answer it from [`COMMANDS`], the
    // same table dispatch reads. The hand-maintained list that used to live at
    // this line had drifted BOTH ways: it omitted every narrative/playable verb
    // (`validate-continuity`, `describe-schema`, `report-playable-world`, …)
    // while naming 53 verbs — one of which (`validate`) dispatched to nothing.
    // Naming 53 is what made it read as exhaustive rather than partial, so a
    // consumer concluded the narrative surface did not exist and rebuilt it in
    // Python (R620/R621).
    let Some(cmd) = args.get(1) else {
        print_help(prog);
        return Ok(());
    };

    let ctx = Ctx { prog, args };
    match COMMANDS.iter().find(|c| c.matches(cmd)) {
        Some(command) => (command.run)(&ctx),
        None => Err(anyhow!("unknown command: {} (run `{} --help`)", cmd, prog).into()),
    }
}

/// Render [`COMMANDS`] in table order. Every line the help emits comes from
/// the same table dispatch reads, so a new verb is documented the moment it is
/// dispatchable.
fn print_help(prog: &str) {
    println!(
        "mnemosyne-cli {} ({}) — Phase 0 design_doc lifecycle (DESIGN §66)",
        env!("CARGO_PKG_VERSION"),
        env!("BUILD_GIT_HASH")
    );
    println!();
    println!("usage:");

    let mut current: Option<&str> = None;
    for command in COMMANDS {
        let title = command.group.map(|g| g.title);
        if title != current {
            if let Some(group) = command.group {
                println!();
                println!(" --- {} ---", group.title);
                for line in group.preamble {
                    println!("{}", line);
                }
            }
            current = title;
        }
        if command.blank_before {
            println!();
        }
        for usage in command.usage {
            println!(" {} {}", prog, usage);
        }
        for note in command.notes {
            println!("{}", note);
        }
    }
}

static GROUP_ATOMIC_MUTATE: Group = Group {
    title: "atomic mutate API (Round 162 production wire, Phase 0f)",
    preamble: &[
        " Field length caps (Round 161 §41 thresholds, surfaced for DX — Round 279 carry):",
        "   intent: max 200 chars; each bullet (rationale/inputs/outputs/caveats): max 100 chars",
    ],
};

static GROUP_PUBLISHABLE: Group = Group {
    title: "publishable half of the ledger (Round 295/297/300; the audit half stays frozen)",
    preamble: &[],
};

static GROUP_INVENTORY: Group = Group {
    title: "Phase 1A inventory mutate API (Round 274)",
    preamble: &[],
};

static GROUP_CODE_CITATION: Group = Group {
    title: "code citation defense (Round 255-260, Path B bidirectional)",
    preamble: &[],
};

static GROUP_SPEC_DRIFT: Group = Group {
    title: "spec-revision drift (RFC-001 UC-1 \"B2\")",
    preamble: &[],
};

static GROUP_CONTENT_DRIFT: Group = Group {
    title: "content-integrity drift (R404 — EPUB-as-content-SSOT)",
    preamble: &[],
};

static GROUP_META: Group = Group {
    title: "meta (Round 286)",
    preamble: &[],
};

/// The single command list. Dispatch resolves a verb here; `print_help`
/// renders this same slice in order. Order = help order.
static COMMANDS: &[Command] = &[
    Command {
        name: "validate-workspace",
        aliases: &[],
        group: None,
        blank_before: false,
        usage: &["validate-workspace 7 markdown doc full validation"],
        notes: &[],
        run: |_| cmd_validate_workspace().map_err(CliError::from),
    },
    Command {
        name: "query",
        aliases: &[],
        group: None,
        blank_before: false,
        usage: &[
            "query §<section_id> [--include-related] [--include-changelog] [--json]",
            "query --list-sections workspace full section_id set print",
            "query --list-changelog [--limit N] [--json] changelog ledger in round order, oldest first (Round 467; --limit keeps the newest N beside the honest total, Round 470)",
            "query --list-inventory [--json] Phase 1A inventory entries (Round 278)",
            "query --inventory <ID> [--json] single inventory entry lookup",
            "query --changelog-entry <ID> [--json] single changelog entry lookup — THE citation check (Round 638): resolves both stored key shapes (`Round 292` and `Round 293 — <title>`), exit 1 = hallucinated, do not write it",
            "query --term <pattern> [--regex] [--case-insensitive|-i] [--scope all|sections|changelog|inventory] [--field name,name,...] [--json]",
        ],
        notes: &["   Round 292 — literal/regex search across atomic Section + ChangelogEntry + Inventory fields; identifier keys section_id/entry_id/inventory_id included (Round 467); unknown --field names reject loudly (Round 468)"],
        run: |c| cmd_query(c.prog, c.rest()).map_err(CliError::from),
    },
    Command {
        name: "style-check",
        aliases: &[],
        group: None,
        blank_before: false,
        usage: &["style-check [--doc <path>] [--severity t3|t4|all] [--json]"],
        notes: &["   T3/T4 style rule layer check (Round 129 production wire)"],
        run: |c| cmd_style_check(c.prog, c.rest()).map_err(CliError::from),
    },
    Command {
        name: "add-section",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["add-section --section §<id> --parent-doc <doc-id> --title <text> [--parent §<P>] [--sidecar <path>] [--json]"],
        notes: &[],
        run: |c| atomic_cli::cmd_add_section(&c.anchor()?, c.rest()),
    },
    Command {
        name: "import-sections",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["import-sections --manifest <path.json> [--sidecar <path>] [--json]"],
        notes: &[],
        run: |c| atomic_cli::cmd_import_sections(&c.anchor()?, c.rest()),
    },
    Command {
        name: "import-epub-anchors",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["import-epub-anchors --anchors <epub-anchor-map.json> [--sidecar <path>] [--json]"],
        notes: &[
            "   bulk create from a JSON array of {section_id,parent_doc,title,parent_section?,normative_excerpt?};",
            "   3-way per entry: absent=create / byte-identical=no-op / divergent=reject whole manifest (atomic)",
        ],
        run: |c| atomic_cli::cmd_import_epub_anchors(&c.anchor()?, c.rest()),
    },
    Command {
        name: "import-facts",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["import-facts --manifest <path.json> [--sidecar <path>] [--json]"],
        notes: &[
            "   bulk narrative registries + facts (Round 430/446): manifest = {frames:[{frame_id,description?}], branches:[...],",
            "   entities:[...], predicates:[{predicate_id,object_kind,subject_kind?,object_entity_kind?,description?}],",
            "   facts:[{fact_id,frame,branch?,entities?,claim,canon_from,canon_to?,evidence[],conflicts_with?,supersedes_in_frame?,payoff_expectation?,pays_off?,typed?,quote?}],",
            "   disclosure_plans:[{telling_id,default_mode?,description?,overrides:[{fact_id,mode,first_at?,surface?}]}] (Round 590 all-primitive)};",
            "   one atomic transaction (registries -> facts -> disclosure); quote_sha256 computed at write, never caller-supplied;",
            "   typed = {subject,predicate,object:{kind:entity,id}|{kind:value,value}} (Round 446 typed leg)",
        ],
        run: |c| atomic_cli::cmd_import_facts(&c.anchor()?, c.rest()),
    },
    Command {
        name: "add-frame",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["add-frame --frame <id> [--description <text>] [--sidecar <path>] [--json]"],
        notes: &[],
        run: |c| atomic_cli::cmd_add_frame(&c.anchor()?, c.rest()),
    },
    Command {
        name: "add-branch",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["add-branch --branch <id> [--description <text>] [--forks-from <branch> --forks-at <section>] [--sidecar <path>] [--json]"],
        notes: &[],
        run: |c| atomic_cli::cmd_add_branch(&c.anchor()?, c.rest()),
    },
    Command {
        name: "add-entity",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["add-entity --entity <id> [--kind <registered-kind>] [--description <text>] [--sidecar <path>] [--json]"],
        notes: &[
            "   --kind is a REF into the entity-kind registry, not free text: register it first",
            "   with add-entity-kind. Omitted = unspecified (allowed); a typo = reject",
        ],
        run: |c| atomic_cli::cmd_add_entity(&c.anchor()?, c.rest()),
    },
    Command {
        name: "add-entity-kind",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["add-entity-kind --kind <id> [--description <text>] [--sidecar <path>] [--json]"],
        notes: &[
            "   declares one member of the entity-kind vocabulary add-entity's --kind refs;",
            "   the members are the consumer's (character/place/item/…), never core's",
        ],
        run: |c| atomic_cli::cmd_add_entity_kind(&c.anchor()?, c.rest()),
    },
    Command {
        name: "add-unit",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["add-unit --unit <id> [--description <text>] [--sidecar <path>] [--json]"],
        notes: &[
            "   Round 706 — declares one member of the unit vocabulary a quantity object's",
            "   --typed-object-quantity-unit refs; the members are the consumer's (day/minute/…)",
        ],
        run: |c| atomic_cli::cmd_add_unit(&c.anchor()?, c.rest()),
    },
    Command {
        name: "add-edge-cost",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["add-edge-cost --fact <adjacent-fact-id> --n <positive-int> --unit <registered-unit> [--sidecar <path>] [--json]"],
        notes: &[
            "   Round 709 (DEBT-J) — attach a map edge's cost (side-table, keyed by the",
            "   adjacent fact); n must be positive (G3), unit registered; retract cascade-drops it",
        ],
        run: |c| atomic_cli::cmd_add_edge_cost(&c.anchor()?, c.rest()),
    },
    Command {
        name: "add-predicate",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["add-predicate --predicate <id> --object-kind entity|token|quantity|fact [--subject-kind <entity_kind>] [--object-entity-kind <entity_kind>] [--object-tokens a,b,c] [--description <text>] [--sidecar <path>] [--json]"],
        notes: &["   Round 446 — 4th registry; TypedClaim predicates are load-bearing (rules key off them), fail-loud",
                 "   Round 701 — optional --subject-kind / --object-entity-kind require the endpoint entity's kind at write time (the spatial-map gate)",
                 "   Round 705 — --object-tokens declares a closed vocab (required under --object-kind token); R706 — quantity objects check the unit against the units registry"],
        run: |c| atomic_cli::cmd_add_predicate(&c.anchor()?, c.rest()),
    },
    Command {
        name: "set-predicate",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["set-predicate --predicate <id> --object-kind entity|token|quantity|fact [--subject-kind <entity_kind>] [--object-entity-kind <entity_kind>] [--object-tokens a,b,c] --description <text> [--sidecar <path>] [--json]"],
        notes: &["   Round 658 — re-type/re-describe an EXISTING predicate (full replace); a re-declare rejects while any use fails the new shape or endpoint kinds (R701)"],
        run: |c| atomic_cli::cmd_set_predicate(&c.anchor()?, c.rest()),
    },
    Command {
        name: "remove-predicate",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["remove-predicate --predicate <id> [--sidecar <path>] [--json]"],
        notes: &["   Round 658 — rejects while any typed leg still names it (no orphan)"],
        run: |c| atomic_cli::cmd_remove_predicate(&c.anchor()?, c.rest()),
    },
    Command {
        name: "add-disclosure-plan",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["add-disclosure-plan --telling <id> --default-mode withhold|state|hint|imply [--description <text>] [--sidecar <path>] [--json]"],
        notes: &[],
        run: |c| atomic_cli::cmd_add_disclosure_plan(&c.anchor()?, c.rest()),
    },
    Command {
        name: "set-disclosure",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["set-disclosure --telling <id> --fact <id> --mode withhold|state|hint|imply [--first-at <branch>=<section> ...] [--surface <section>[,<entity>]] [--sidecar <path>] [--json]"],
        notes: &["   Round 506 — disclosure (discourse) layer: a named telling over the fact base; withhold/first_at need a typed fact (gate-matchable)"],
        run: |c| atomic_cli::cmd_set_disclosure(&c.anchor()?, c.rest()),
    },
    Command {
        name: "remove-disclosure",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["remove-disclosure --telling <id> --fact <id> --reason <text> [--sidecar <path>] [--json]"],
        notes: &["   Round 626 — clear one telling's decision for one fact; the fact is untouched (a disclosure decision belongs to the TELLING, R506). The escape hatch the R626 retract/amend guards require — a refusal that says \"clear the decision first\" is a trap unless clearing is possible",
                 "   NOT neutral (Round 627): the fact then rides the plan's default_mode (default `withhold`), so clearing a `state` decision flips it from told to never-told for that telling; the receipt names the resulting effective mode"],
        run: |c| atomic_cli::cmd_remove_disclosure(&c.anchor()?, c.rest()),
    },
    Command {
        name: "report-entity",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["report-entity --entity <id> [--sidecar <path>] [--json]"],
        notes: &[],
        run: |c| cmd_report_entity(c.rest()).map_err(CliError::from),
    },
    Command {
        name: "report-entity-kind-migration",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["report-entity-kind-migration [--sidecar <path>] [--json]"],
        notes: &["   R679 — the worklist for a pre-registry (v23-) or out-of-band store: the distinct unregistered entity kinds in use, each with the add-entity-kind call to make"],
        run: |c| cmd_report_entity_kind_migration(c.rest()).map_err(CliError::from),
    },
    Command {
        name: "report-payoff-coverage",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["report-payoff-coverage [--order <canon-order.json>] [--sidecar <path>] [--json]"],
        notes: &[],
        run: |c| cmd_report_payoff_coverage(c.rest()).map_err(CliError::from),
    },
    Command {
        name: "report-irony-intervals",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["report-irony-intervals [--order <canon-order.json>] [--sidecar <path>] [--json]"],
        notes: &["   Round 455 — cross-frame divergence windows per query world (craft signal, never gated)"],
        run: |c| cmd_report_irony_intervals(c.rest()).map_err(CliError::from),
    },
    Command {
        name: "report-playthrough-manuscript",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["report-playthrough-manuscript [--world <branch>] [--telling <id>] [--reading-walk] [--order <canon-order.json>] [--sidecar <path>] [--json]"],
        notes: &[
            "   --telling (Round 506): annotate each begins-event with its disclosure decision (mode/first_at/surface) = the render-brief carrier",
            "   --reading-walk (Round 509): prune each world to its content scenes (begins>0) = the deterministic reading-copy walk (no hand prune)",
        ],
        run: |c| cmd_report_playthrough_manuscript(c.rest()).map_err(CliError::from),
    },
    Command {
        name: "report-fork-tree",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["report-fork-tree [--order <canon-order.json>] [--sidecar <path>] [--json]"],
        notes: &[],
        run: |c| cmd_report_fork_tree(c.rest()).map_err(CliError::from),
    },
    Command {
        name: "report-playable-world",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["report-playable-world --telling <id> [--world <branch>] [--order <canon-order.json>] [--sidecar <path>] [--json]"],
        notes: &["   Round 556/557 — the map_locator seam: fork topology + per-world scene walk + per-scene disclosure pointers a pinion runtime consumes"],
        run: |c| cmd_report_playable_world(c.rest()).map_err(CliError::from),
    },
    Command {
        name: "report-quest-graph",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["report-quest-graph --telling <id> [--world <branch>] [--order <canon-order.json>] [--sidecar <path>] [--json]"],
        notes: &["   Round 559/568 — the fact->quest leg: per derived quest (pursues/requires/completed_by role), objective + actor + per-world open/done + prerequisites + completion fact + giver locator"],
        run: |c| cmd_report_quest_graph(c.rest()).map_err(CliError::from),
    },
    Command {
        name: "describe-schema",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["describe-schema [--json]"],
        notes: &["   Round 587 — the medium-neutral authoring contract (static): registries + fact shape + fixed vocabularies + rule classes + quest encoding + write-time invariants"],
        run: |c| cmd_describe_schema(c.rest()).map_err(CliError::from),
    },
    Command {
        name: "propose-verdict",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["propose-verdict --manifest <path.json> [--order <path>] [--rules <path>] [--sidecar <path>] [--json]"],
        notes: &["   Round 588 — dry-run gate: apply a candidate batch to a throwaway clone, run shape + continuity gates, emit commit/rollback + actionable violations (exit 1 on rollback; store never written)"],
        run: |c| cmd_propose_verdict(c.rest()).map_err(CliError::from),
    },
    Command {
        name: "report-authoring-frontier",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["report-authoring-frontier [--telling <id>] [--order <path>] [--sidecar <path>] [--json]"],
        notes: &["   Round 589 — the consolidated coverage-gap frontier a loop pulls work from: zero-fact scenes + per-scene coverage + dangling setups + (with --telling) unresolved quests + never-planned disclosures"],
        run: |c| cmd_report_authoring_frontier(c.rest()).map_err(CliError::from),
    },
    Command {
        name: "report-disclosure-coverage",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["report-disclosure-coverage --telling <id> [--sidecar <path>] [--json]"],
        notes: &[],
        run: |c| cmd_report_disclosure_coverage(c.rest()).map_err(CliError::from),
    },
    Command {
        name: "validate-disclosure-leak",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["validate-disclosure-leak --telling <id> --against <reextracted.json> --world <branch> --truth-frame <frame> [--order <path>] [--sidecar <path>] [--json]"],
        notes: &[],
        run: |c| cmd_validate_disclosure_leak(c.rest()).map_err(CliError::from),
    },
    Command {
        name: "validate-render-fidelity",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["validate-render-fidelity --against <reextracted.json> --world <branch> [--order <path>] [--sidecar <path>] [--json]"],
        notes: &[
            "   Round 507 — disclosure render-acceptance gates over a blind re-extracted prose store; leak/fidelity exit non-zero on violation",
            "   Round 466 — per-world linear scene walk with declared fact events (reading surface, never gated)",
        ],
        run: |c| cmd_validate_render_fidelity(c.rest()).map_err(CliError::from),
    },
    Command {
        name: "report-typing-candidates",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["report-typing-candidates [--sidecar <path>] [--json]"],
        notes: &["   Round 458 — typing-discovery input package: untyped facts + claim sha256 + registered vocabulary"],
        run: |c| cmd_report_typing_candidates(c.rest()).map_err(CliError::from),
    },
    Command {
        name: "import-typing-proposals",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["import-typing-proposals --proposals <typing-proposals.json> [--dry-run] [--sidecar <path>] [--json]"],
        notes: &["   Round 459 — all-or-nothing reviewed import of proposed typed legs (claim-sha staleness re-checked, fill-blanks only)"],
        run: |c| cmd_import_typing_proposals(c.rest()).map_err(CliError::from),
    },
    Command {
        name: "report-edge-candidates",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["report-edge-candidates [--order <canon-order.json>] [--sidecar <path>] [--json]"],
        notes: &["   Round 462 — edge-discovery input package: every fact row (claim sha256 + recorded edges) + succession-gap hints"],
        run: |c| cmd_report_edge_candidates(c.rest()).map_err(CliError::from),
    },
    Command {
        name: "import-edge-proposals",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["import-edge-proposals --proposals <edge-proposals.json> [--dry-run] [--sidecar <path>] [--json]"],
        notes: &["   Round 463 — all-or-nothing reviewed import of proposed succession/conflict edges (two-sided claim-sha staleness, fill-blanks, cycle-guarded)"],
        run: |c| cmd_import_edge_proposals(c.rest()).map_err(CliError::from),
    },
    Command {
        name: "report-payoff-substantiation",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["report-payoff-substantiation [--order <canon-order.json>] [--sidecar <path>] [--json]"],
        notes: &["   Round 485 — deterministic payoff substantiation: each credited setup is substantiated (a typed state-change discharges it) / unsubstantiated (typed setup, hollow payoff) / unverifiable (untyped — type it); no LLM"],
        run: |c| cmd_report_payoff_substantiation(c.rest()).map_err(CliError::from),
    },
    Command {
        name: "report-timeline-gaps",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["report-timeline-gaps [--order <canon-order.json>] [--rules <narrative-rules.json>] [--world <branch>] [--sidecar <path>] [--json]"],
        notes: &["   Round 490 — timeline-gap projection (read, never gated): the interval-rule evaluator per world — violated / unverifiable / satisfied scalar relations (value(left) - value(right) op bound)"],
        run: |c| cmd_report_timeline_gaps(c.rest()).map_err(CliError::from),
    },
    Command {
        name: "add-fact",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["add-fact --fact <id> --frame <f> [--branch <id>] --claim <text> --canon-from <section> [--canon-to <section>] --evidence <sec,sec> [--entities <id,id>] [--conflicts <id,id>] [--supersedes <id>] [--payoff-expectation expected] [--pays-off <id,id>] [--typed-subject <entity> --typed-predicate <id> (--typed-object-entity <entity> | --typed-object-token <t> | --typed-object-quantity-n <int> --typed-object-quantity-unit <u> | --typed-object-fact <fact-id>)] [--quote <text>] [--sidecar <path>] [--json]"],
        notes: &["   typed leg (Round 446): optional machine-readable subject-predicate-object reading of the claim, authored with it (never NLP-derived)"],
        run: |c| atomic_cli::cmd_add_fact(&c.anchor()?, c.rest()),
    },
    Command {
        name: "add-fact-conflict",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["add-fact-conflict --fact <id> --conflicts-with <id> [--sidecar <path>] [--json]"],
        notes: &[],
        run: |c| atomic_cli::cmd_add_fact_conflict(&c.anchor()?, c.rest()),
    },
    Command {
        name: "amend-fact",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["amend-fact --fact <id> --reason <text> <add-fact flags> [--sidecar <path>] [--json]   (authorial in-place revision; in-world change = --supersedes)"],
        notes: &[],
        run: |c| atomic_cli::cmd_amend_fact(&c.anchor()?, c.rest()),
    },
    Command {
        name: "retract-fact",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["retract-fact --fact <id> --reason <text> [--sidecar <path>] [--json]"],
        notes: &["   pairs with remove-section (R267); content fields populate via set-section-* afterwards"],
        run: |c| atomic_cli::cmd_retract_fact(&c.anchor()?, c.rest()),
    },
    Command {
        name: "set-section-intent",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["set-section-intent --section §<N> --intent <text (max 200 chars)> [--sidecar <path>] [--json]"],
        notes: &[],
        run: |c| atomic_cli::cmd_set_section_intent(&c.anchor()?, c.rest()),
    },
    Command {
        name: "set-section-rationale",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["set-section-rationale --section §<N> --bullets-file <path (each bullet ≤ 100 chars)> [--sidecar <path>] [--json]"],
        notes: &[],
        run: |c| atomic_cli::cmd_set_section_rationale(&c.anchor()?, c.rest()),
    },
    Command {
        name: "set-section-inputs",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["set-section-inputs --section §<N> --bullets-file <path (each bullet ≤ 100 chars)> [--sidecar <path>] [--json]"],
        notes: &[],
        run: |c| atomic_cli::cmd_set_section_inputs(&c.anchor()?, c.rest()),
    },
    Command {
        name: "set-section-outputs",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["set-section-outputs --section §<N> --bullets-file <path (each bullet ≤ 100 chars)> [--sidecar <path>] [--json]"],
        notes: &[],
        run: |c| atomic_cli::cmd_set_section_outputs(&c.anchor()?, c.rest()),
    },
    Command {
        name: "set-section-title",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["set-section-title --section §<N> --title <heading text> [--sidecar <path>] [--json]"],
        notes: &[],
        run: |c| atomic_cli::cmd_set_section_title(&c.anchor()?, c.rest()),
    },
    Command {
        name: "set-section-parent-doc",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["set-section-parent-doc --section §<N> --parent-doc <doc-id> [--sidecar <path>] [--json]"],
        notes: &[],
        run: |c| atomic_cli::cmd_set_section_parent_doc(&c.anchor()?, c.rest()),
    },
    Command {
        name: "set-section-parent-section",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["set-section-parent-section --section §<N> (--parent §<P> | --no-parent) [--sidecar <path>] [--json]"],
        notes: &[],
        run: |c| atomic_cli::cmd_set_section_parent_section(&c.anchor()?, c.rest()),
    },
    Command {
        name: "add-section-caveat",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["add-section-caveat --section §<N> --bullet <text (max 100 chars)> [--sidecar <path>] [--json]"],
        notes: &[],
        run: |c| atomic_cli::cmd_add_section_caveat(&c.anchor()?, c.rest()),
    },
    Command {
        name: "set-section-alternatives",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["set-section-alternatives --section §<N> --alternatives-file <path> [--sidecar <path>] [--json]"],
        notes: &[],
        run: |c| atomic_cli::cmd_set_section_alternatives(&c.anchor()?, c.rest()),
    },
    Command {
        name: "set-section-impact-scope",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["set-section-impact-scope --section §<N> --refs §A,§B,... [--sidecar <path>] [--json]"],
        notes: &[],
        run: |c| atomic_cli::cmd_set_section_impact_scope(&c.anchor()?, c.rest()),
    },
    Command {
        name: "add-section-example",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["add-section-example --section §<N> --language <lang> --code-file <path> [--sidecar <path>] [--json]"],
        notes: &[],
        run: |c| atomic_cli::cmd_add_section_example(&c.anchor()?, c.rest()),
    },
    Command {
        name: "add-section-binding",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["add-section-binding --section §<N> --file <workspace-relative-path> [--symbol <name>] --kind implements|references [--sidecar <path>] [--json]"],
        notes: &["   Path B typed trace-link binding (implements=«satisfy» / references=«trace»); coverage counts only implements"],
        run: |c| atomic_cli::cmd_add_section_binding(&c.anchor()?, c.rest()),
    },
    Command {
        name: "remove-section-binding",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["remove-section-binding --section §<N> --file <path> [--symbol <name>] --reason <text> [--sidecar <path>] [--json]"],
        notes: &["   Section.bindings remove primitive (exact (file, symbol) match, kind-agnostic; --reason mandatory)"],
        run: |c| atomic_cli::cmd_remove_section_binding(&c.anchor()?, c.rest()),
    },
    Command {
        name: "set-section-binding-kind",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["set-section-binding-kind --section §<N> --file <path> [--symbol <name>] --kind implements|references --reason <text> [--sidecar <path>] [--json]"],
        notes: &["   Reclassify an existing binding's kind (Stage-B implements→references; --reason mandatory)"],
        run: |c| atomic_cli::cmd_set_section_binding_kind(&c.anchor()?, c.rest()),
    },
    Command {
        name: "set-section-coverage-expectation",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["set-section-coverage-expectation --section §<N> --expectation normative|out_of_scope_here|informational --reason <text> [--sidecar <path>] [--json]"],
        notes: &[],
        run: |c| atomic_cli::cmd_set_section_coverage_expectation(&c.anchor()?, c.rest()),
    },
    Command {
        name: "set-section-verification-expectation",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["set-section-verification-expectation --section §<N> --expectation dedicated|by_construction --reason <text> [--sidecar <path>] [--json]"],
        notes: &[],
        run: |c| atomic_cli::cmd_set_section_verification_expectation(&c.anchor()?, c.rest()),
    },
    Command {
        name: "add-confirmation-event",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["add-confirmation-event --section §<N> [--file <path> --symbol <sym>] --confirmer-kind tool|model --confirmer-id <id> --confirmer-version <v> --method linkage_check|semantic_review|coverage_attestation --verdict confirm|refute --authoring-run <id> --confirming-run <id> --rationale <text> --timestamp <iso> [--spec-sha256 <h>] [--code-sha256 <h>] [--test-sha256 <h>] [--sidecar <path>] [--json]"],
        notes: &["   Classify coverage applicability; informative exempts the section from the coverage axiom (--reason mandatory)"],
        run: |c| atomic_cli::cmd_add_confirmation_event(&c.anchor()?, c.rest()),
    },
    Command {
        name: "import-epub-excerpts",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["import-epub-excerpts --anchors <epub-anchor-map.json> [--sidecar <path>] [--json]"],
        notes: &["   refresh normative_excerpt.text + text_sha256 from a medium-forge epub-anchor-map/v2; preserves authored anchor_url + source_revision (section must already carry an excerpt)"],
        run: |c| atomic_cli::cmd_import_epub_excerpts(&c.anchor()?, c.rest()),
    },
    Command {
        name: "set-section-decision-status",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["set-section-decision-status --section §<N> --status active|superseded|removed|open [--superseding §<M>] [--resolving §<M>] [--sidecar <path>] [--json]"],
        notes: &["   atomic decision_status setter (Stage B freshness substrate); --superseding required for --status superseded (T1 rule 4 atomic axis)"],
        run: |c| atomic_cli::cmd_set_section_decision_status(&c.anchor()?, c.rest()),
    },
    Command {
        name: "remove-section",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["remove-section --section §<N> --reason <text> [--sidecar <path>] [--json]"],
        notes: &["   Round 267 section removal (audit-safeguarded; closes Round 266 carry)"],
        run: |c| atomic_cli::cmd_remove_section(&c.anchor()?, c.rest()),
    },
    Command {
        name: "append-changelog-entry",
        aliases: &[],
        group: Some(&GROUP_ATOMIC_MUTATE),
        blank_before: false,
        usage: &["append-changelog-entry --entry-id \"Round N\" --decision <text> --changes-file <path> --verification-file <path> --impact §A,§B --carry-file <path> [--sidecar <path>] [--json]"],
        notes: &[],
        run: |c| atomic_cli::cmd_append_changelog_entry(&c.anchor()?, c.rest()),
    },
    Command {
        name: "set-changelog-publishable-decision-summary",
        aliases: &[],
        group: Some(&GROUP_PUBLISHABLE),
        blank_before: false,
        usage: &["set-changelog-publishable-decision-summary --entry <entry-id> --value <text> [--sidecar <path>] [--json]"],
        notes: &[],
        run: |c| atomic_cli::cmd_set_changelog_publishable_decision_summary(&c.anchor()?, c.rest()),
    },
    Command {
        name: "set-changelog-publishable-changes",
        aliases: &[],
        group: Some(&GROUP_PUBLISHABLE),
        blank_before: false,
        usage: &["set-changelog-publishable-changes --entry <entry-id> --bullets-file <path> [--sidecar <path>] [--json]"],
        notes: &[],
        run: |c| atomic_cli::cmd_set_changelog_publishable_changes(&c.anchor()?, c.rest()),
    },
    Command {
        name: "set-changelog-publishable-verification",
        aliases: &[],
        group: Some(&GROUP_PUBLISHABLE),
        blank_before: false,
        usage: &["set-changelog-publishable-verification --entry <entry-id> --bullets-file <path> [--sidecar <path>] [--json]"],
        notes: &[],
        run: |c| atomic_cli::cmd_set_changelog_publishable_verification(&c.anchor()?, c.rest()),
    },
    Command {
        name: "set-changelog-publishable-impact-refs",
        aliases: &[],
        group: Some(&GROUP_PUBLISHABLE),
        blank_before: false,
        usage: &["set-changelog-publishable-impact-refs --entry <entry-id> --bullets-file <path> [--sidecar <path>] [--json]"],
        notes: &[],
        run: |c| atomic_cli::cmd_set_changelog_publishable_impact_refs(&c.anchor()?, c.rest()),
    },
    Command {
        name: "set-changelog-publishable-carry-forward",
        aliases: &[],
        group: Some(&GROUP_PUBLISHABLE),
        blank_before: false,
        usage: &["set-changelog-publishable-carry-forward --entry <entry-id> --bullets-file <path> [--sidecar <path>] [--json]"],
        notes: &[],
        run: |c| atomic_cli::cmd_set_changelog_publishable_carry_forward(&c.anchor()?, c.rest()),
    },
    Command {
        name: "redact-term",
        aliases: &[],
        group: Some(&GROUP_PUBLISHABLE),
        blank_before: false,
        usage: &["redact-term --pattern <text> --replacement <text> --reason <text> --applied-in <entry-id> [--kind <text>] [--scope all|decision_summary|changes_bullets|verification_bullets|impact_refs|carry_forward_bullets] [--regex] [--case-insensitive|-i] [--dry-run] [--sidecar <path>] [--json]"],
        notes: &["   Round 297 — redact across the publishable half in one call, ledger-recorded"],
        run: |c| atomic_cli::cmd_redact_term(&c.anchor()?, c.rest()),
    },
    Command {
        name: "emit-publishable-override-ledger-draft",
        aliases: &[],
        group: Some(&GROUP_PUBLISHABLE),
        blank_before: false,
        usage: &["emit-publishable-override-ledger-draft --entry <entry-id> --reason <text> --applied-in <entry-id> [--kind <text>] [--sidecar <path>] [--json]"],
        notes: &["   Round 300 — read-only [[publishable_override_ledger]] draft for a diverged entry"],
        run: |c| atomic_cli::cmd_emit_publishable_override_ledger_draft(&c.anchor()?, c.rest()),
    },
    Command {
        name: "add-inventory-entry",
        aliases: &[],
        group: Some(&GROUP_INVENTORY),
        blank_before: false,
        usage: &["add-inventory-entry --id <ID> --status active|deprecated|reserved [--section §<N>] [--source <text>] [--reason <text>] [--sidecar <path>] [--json]"],
        notes: &[],
        run: |c| atomic_cli::cmd_add_inventory_entry(&c.anchor()?, c.rest()),
    },
    Command {
        name: "set-inventory-status",
        aliases: &[],
        group: Some(&GROUP_INVENTORY),
        blank_before: false,
        usage: &["set-inventory-status --id <ID> --status active|deprecated|reserved [--reason <text>] [--sidecar <path>] [--json]"],
        notes: &[],
        run: |c| atomic_cli::cmd_set_inventory_status(&c.anchor()?, c.rest()),
    },
    Command {
        name: "set-inventory-section-ref",
        aliases: &[],
        group: Some(&GROUP_INVENTORY),
        blank_before: false,
        usage: &["set-inventory-section-ref --id <ID> (--section §<N> | --clear) [--sidecar <path>] [--json]"],
        notes: &[],
        run: |c| atomic_cli::cmd_set_inventory_section_ref(&c.anchor()?, c.rest()),
    },
    Command {
        name: "remove-inventory-entry",
        aliases: &[],
        group: Some(&GROUP_INVENTORY),
        blank_before: false,
        usage: &["remove-inventory-entry --id <ID> --reason <text> [--sidecar <path>] [--json]"],
        notes: &["   Round 273 InventoryEntry 5번째 closed-form 엔티티 substrate; cite-time reject (R275) + cascade (R276) carry"],
        run: |c| atomic_cli::cmd_remove_inventory_entry(&c.anchor()?, c.rest()),
    },
    Command {
        name: "validate-code-refs",
        aliases: &[],
        group: Some(&GROUP_CODE_CITATION),
        blank_before: false,
        usage: &["validate-code-refs [--severity-missing reject|warn|info]"],
        notes: &[
            "                        [--severity-binding reject|warn|info]",
            "                        [--severity-coverage reject|warn|info]",
            "                        [--severity-verification reject|warn|info]",
            "                        [--severity-classification reject|warn|info]",
            "                        [--severity-blanket reject|warn|info]",
            "                        [--filter-id <entry_id>] [--json]",
            "   Round 256: scan [plugins.set_equality_validator].paths for <entry_id_prefix><digits> citations,",
            "   reject those whose entry_id is missing from atomic store changelog_entries",
            "   Round 260: §<id> citations cross-checked against AtomicSection.bindings",
            "   --severity-missing: Missing + SectionMissing (hallucination class)",
            "   --severity-binding (Round 260): CitationUnbound + BindingUnbacked + SymbolMismatch (edge class)",
            "   --severity-coverage (Round 385): ImplementationMissing (Active section uncited); inherits --severity-binding when unset",
            "   --filter-id (Round 258): restrict to citations of one id; surfaces them as decay (cascade caller use)",
        ],
        run: |c| cmd_validate_code_refs(c.rest()).map_err(CliError::from),
    },
    Command {
        name: "propose-implementations",
        aliases: &[],
        group: Some(&GROUP_CODE_CITATION),
        blank_before: true,
        usage: &["propose-implementations [--section §<id>] [--json]"],
        notes: &[
            "   Path B curation: per (section,file) cite, resolve the enclosing/documented symbol and",
            "   emit proposed §<id> binding sets + add-section-binding commands (read-only)",
        ],
        run: |c| cmd_propose_implementations(c.rest()).map_err(CliError::from),
    },
    Command {
        name: "report-binding-migration",
        aliases: &[],
        group: Some(&GROUP_CODE_CITATION),
        blank_before: false,
        usage: &["report-binding-migration [--json]"],
        notes: &[
            "   v4→v5 surface: list bindings that inherited kind=implements by default (read-only;",
            "   empty once the store is at v5 — run before upgrading a pre-v5 store)",
        ],
        run: |c| cmd_report_binding_migration(c.rest()).map_err(CliError::from),
    },
    Command {
        name: "report-coverage",
        aliases: &[],
        group: Some(&GROUP_CODE_CITATION),
        blank_before: false,
        usage: &["report-coverage [--json]"],
        notes: &[],
        run: |c| cmd_report_coverage(c.rest()).map_err(CliError::from),
    },
    Command {
        name: "report-confirmation",
        aliases: &[],
        group: Some(&GROUP_CODE_CITATION),
        blank_before: false,
        usage: &["report-confirmation [--json]"],
        notes: &[],
        run: |c| cmd_report_confirmation(c.rest()).map_err(CliError::from),
    },
    Command {
        name: "validate-confirmation",
        aliases: &[],
        group: Some(&GROUP_CODE_CITATION),
        blank_before: false,
        usage: &["validate-confirmation [--severity reject|warn|info] [--json]"],
        notes: &[],
        run: |c| cmd_validate_confirmation(c.rest()).map_err(CliError::from),
    },
    Command {
        name: "validate-continuity",
        aliases: &[],
        group: Some(&GROUP_CODE_CITATION),
        blank_before: false,
        usage: &["validate-continuity [--order <canon-order.json>] [--rules <narrative-rules.json>] [--severity reject|warn|info] [--interval-severity reject|warn|info] [--sidecar <path>] [--json]"],
        notes: &[
            "   frame-scoped narrative continuity (Round 431): same-frame overlapping conflict = violation,",
            "   cross-frame conflict = data; canon order is a DECLARED partial order, never inferred",
        ],
        run: |c| cmd_validate_continuity(c.rest()).map_err(CliError::from),
    },
    Command {
        name: "report-frame-view",
        aliases: &[],
        group: Some(&GROUP_CODE_CITATION),
        blank_before: false,
        usage: &["report-frame-view --frame <id> [--branch <id>] [--entity <id>] --at <section> [--order <canon-order.json>] [--sidecar <path>] [--json]"],
        notes: &[
            "   read-only frame-at-T projection (Round 432): the facts frame F holds at canon point T,",
            "   same holds-semantics as the gate; incomparable coordinates surface as `unknown`",
        ],
        run: |c| cmd_report_frame_view(c.rest()).map_err(CliError::from),
    },
    Command {
        name: "validate-verifies-linkage",
        aliases: &[],
        group: Some(&GROUP_CODE_CITATION),
        blank_before: false,
        usage: &["validate-verifies-linkage [--catalog <path>] [--severity reject|warn|info] [--json]"],
        notes: &[],
        run: |c| cmd_validate_verifies_linkage(c.rest()).map_err(CliError::from),
    },
    Command {
        name: "report-excerpt-hash-backfill",
        aliases: &[],
        group: Some(&GROUP_CODE_CITATION),
        blank_before: false,
        usage: &["report-excerpt-hash-backfill [--json]"],
        notes: &["   coverage breakdown: implemented / normative-gap / informative-exempt + ratio (read-only)"],
        run: |c| cmd_report_excerpt_hash_backfill(c.rest()).map_err(CliError::from),
    },
    Command {
        name: "report-spec-map",
        aliases: &[],
        group: Some(&GROUP_CODE_CITATION),
        blank_before: false,
        usage: &["report-spec-map [--json]"],
        notes: &[
            "   unified spec<->fact<->code projection per section: coverage class + spec provenance",
            "   (anchor_url/revision) + bindings + drift flag + reverse citation count (read-only L3 view)",
        ],
        run: |c| cmd_report_spec_map(c.rest()).map_err(CliError::from),
    },
    Command {
        name: "validate-spec-drift",
        aliases: &[],
        group: Some(&GROUP_SPEC_DRIFT),
        blank_before: false,
        usage: &["validate-spec-drift [--severity reject|warn|info] [--json]"],
        notes: &[
            "   flag Active Sections whose normative_excerpt.source_revision trails",
            "   [workspace.spec_source].revision; Superseded/Removed exempt (partial-migration).",
            "   --severity overrides [spec_drift].severity (default warn); reject => exit 1 on drift.",
            "   no-op (exit 0) when [workspace.spec_source] is absent.",
        ],
        run: |c| cmd_validate_spec_drift(c.rest()).map_err(CliError::from),
    },
    Command {
        name: "validate-content-drift",
        aliases: &[],
        group: Some(&GROUP_CONTENT_DRIFT),
        blank_before: false,
        usage: &["validate-content-drift [--severity reject|warn|info] [--json]"],
        notes: &[
            "   offline re-hash of each normative_excerpt.text vs its text_sha256;",
            "   a populated hash that no longer matches = drift (cache edited out-of-band).",
            "   --severity overrides [content_drift].severity (default reject); reject => exit 1.",
            "   empty-hash excerpts are unrevalidatable (counted, not drift).",
            "   also re-hashes the committed EPUB vs [workspace.spec_source].epub_sha256 when pinned (R405).",
        ],
        run: |c| cmd_validate_content_drift(c.rest()).map_err(CliError::from),
    },
    Command {
        name: "--version",
        aliases: &[
            "-V",
            "version",
        ],
        group: Some(&GROUP_META),
        blank_before: false,
        usage: &["--version | -V | version  print binary version + build hash"],
        notes: &[],
        run: |_| {
            println!(
                "mnemosyne-cli {} ({})",
                env!("CARGO_PKG_VERSION"),
                env!("BUILD_GIT_HASH")
            );
            Ok(())
        },
    },
    Command {
        name: "--help",
        aliases: &[
            "-h",
            "help",
        ],
        group: Some(&GROUP_META),
        blank_before: false,
        usage: &["--help | -h | help   print this help text"],
        notes: &[],
        run: |c| {
            print_help(c.prog);
            Ok(())
        },
    },
];

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
    // Round 467 — whole-ledger changelog listing (R410 read model exposed).
    list_changelog: bool,
    // Round 470 — newest-n bound for --list-changelog (no-silent-caps:
    // the report carries the full total beside the slice).
    changelog_limit: Option<usize>,
    // Round 278 — Phase 1A inventory query surface.
    list_inventory: bool,
    inventory_id: Option<String>,
    changelog_entry_id: Option<String>,
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
            "--list-changelog" => out.list_changelog = true,
            "--limit" => {
                let v = iter
                    .next()
                    .ok_or_else(|| anyhow!("--limit missing value (a positive integer)"))?;
                let n: usize = v
                    .parse()
                    .map_err(|_| anyhow!("--limit expects a positive integer (got `{}`)", v))?;
                out.changelog_limit = Some(n);
            }
            "--list-inventory" => out.list_inventory = true,
            "--inventory" => {
                out.inventory_id = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--inventory missing value"))?
                        .clone(),
                );
            }
            "--changelog-entry" => {
                out.changelog_entry_id = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--changelog-entry missing value (a `Round NNN`)"))?
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
    // Round 470 — query modes are mutually exclusive; a second mode used to
    // be silently outranked by dispatch order (parse-then-ignore, the class
    // the Round 466 --world guard rejects). Mode-scoped flags follow the
    // same rule.
    let modes: Vec<&str> = [
        ("--list-sections", out.list_sections),
        ("--list-changelog", out.list_changelog),
        ("--list-inventory", out.list_inventory),
        ("--inventory", out.inventory_id.is_some()),
        ("--changelog-entry", out.changelog_entry_id.is_some()),
        ("--term", out.term_pattern.is_some()),
        ("§<section_id>", out.section_id.is_some()),
    ]
    .iter()
    .filter(|(_, on)| *on)
    .map(|(name, _)| *name)
    .collect();
    if modes.len() > 1 {
        bail!(
            "query modes are mutually exclusive — pick one of {}",
            modes.join(", ")
        );
    }
    if out.changelog_limit.is_some() && !out.list_changelog {
        bail!("--limit only applies to --list-changelog");
    }
    if out.term_pattern.is_none()
        && (out.term_regex
            || out.term_case_insensitive
            || out.term_scope.is_some()
            || !out.term_fields.is_empty())
    {
        bail!("--regex / --case-insensitive / --scope / --field only apply to --term");
    }
    if out.section_id.is_none() && (out.include_related || out.include_changelog) {
        bail!("--include-related / --include-changelog only apply to a §<section_id> query");
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

    // Round 467 — whole changelog ledger, round-number order (oldest first;
    // tail = latest rounds). Exposes the R410 read model that previously
    // had no CLI surface, which forced ID searches through `--term`.
    if qargs.list_changelog {
        let view = mnemosyne_query::list_changelog(&atomic_store, qargs.changelog_limit);
        if qargs.json {
            println!("{}", serde_json::to_string_pretty(&view)?);
        } else {
            for e in &view.entries {
                println!("{}", e.entry_id);
            }
            if view.entries.len() < view.total {
                eprintln!(
                    "# showing newest {} of {} changelog entry(ies)",
                    view.entries.len(),
                    view.total
                );
            } else {
                eprintln!("# total {} changelog entry(ies)", view.total);
            }
        }
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

    // Round 638 — the single-entry changelog read (DEBT-E), the `--inventory`
    // twin the decision SSOT never had. Resolves EITHER stored key shape
    // through `normalize_entry_citation`, the same resolver the code-refs gate
    // uses — a citation names a number, never the title it cannot know.
    if let Some(cited) = qargs.changelog_entry_id.as_deref() {
        let view = mnemosyne_ops::query::query_changelog_entry(&root, cited)
            .map_err(|e| anyhow!("{}", e))?;
        if qargs.json {
            println!("{}", serde_json::to_string_pretty(&view)?);
        } else {
            println!("entry_id: {}", view.entry_id);
            if let Some(s) = view.atomic_decision_summary.as_deref() {
                println!("\ndecision_summary:\n{}", s);
            }
            for (label, bullets) in [
                ("changes", &view.atomic_changes_bullets),
                ("verification", &view.atomic_verification_bullets),
                ("carry_forward", &view.atomic_carry_forward_bullets),
            ] {
                if !bullets.is_empty() {
                    println!("\n{}:", label);
                    for b in bullets {
                        println!("- {}", b);
                    }
                }
            }
            if !view.atomic_impact_refs.is_empty() {
                println!("\nimpact_refs: {}", view.atomic_impact_refs.join(", "));
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
    print_section_prose_fact_assertion_surface(&atomic)?;

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

/// Structured-fact SSOT — section-prose surface (design sec 12b). The
/// counterpart of the code-comment lint (`validate-code-refs`
/// `severity_prose_fact_assertion`): a section's own prose
/// (`intent`/`rationale`/`caveat`/`inputs`/`outputs`) must POINT to a section,
/// not RESTATE a structured fact about it. Reuses the same axis + verb set, so
/// one knob governs both surfaces (uniform enforcement). OFF unless
/// `severity_prose_fact_assertion` is set; gates the exit code only at `reject`
/// (mirrors the commit-ledger surface's gating pattern).
fn print_section_prose_fact_assertion_surface(
    atomic: &mnemosyne_atomic::AtomicStore,
) -> Result<()> {
    let cfg = match workspace_config() {
        Ok(c) => c,
        Err(_) => return Ok(()),
    };
    let scfg = match cfg
        .config
        .plugins
        .as_ref()
        .and_then(|p| p.set_equality_validator.as_ref())
    {
        Some(c) => c,
        None => return Ok(()),
    };
    let severity = match scfg.severity_prose_fact_assertion {
        Some(s) => s,
        None => return Ok(()), // axis off — no scan, no surface line
    };
    // Reuse the store already loaded by the caller (no redundant load).
    let findings = mnemosyne_validate::code_refs::scan_section_prose_fact_assertions(atomic);
    println!(
        "section prose-fact-assertion: {} finding(s) (severity_prose_fact_assertion={})",
        findings.len(),
        severity.as_str()
    );
    for f in &findings {
        println!(" §{} [{}] prose asserts: {}", f.section_id, f.field, f.verb);
    }
    if !findings.is_empty() && severity == Severity::Reject {
        bail!(
            "{} section prose-fact-assertion violation(s) — section prose restates a \
 store-homed fact; author it via the structured primitive and leave a bare §<id> \
 pointer (severity_prose_fact_assertion=reject)",
            findings.len()
        );
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
            // Round 656 — the remedy must be followable AND true for BOTH
            // classes of `missing`, because they need opposite fixes and the
            // gate cannot tell them apart: it reads commit subjects and
            // resolves them against THIS workspace's ledger, so an upstream's
            // round number (a consumer citing `R643` of the project they
            // adopted) is `missing` forever. Naming only the backfill flow
            // told such a consumer to write a decision they never made into
            // their own ledger — the R627 class (a guard whose sign points
            // away from its own hatch: R377 built `severity`, this hint hid
            // it). Both remedies are named unconditionally; no heuristic
            // guesses which class a number belongs to.
            println!(
                "  hint: if R{n} is THIS workspace's round, backfill it — `mnemosyne-cli \
  append-changelog-entry --entry-id \"Round {n} — ...\" --decision <text> \
  --changes-file <path> --verification-file <path> --impact §A,§B --carry-file <path>` \
  (Round 293 backfill flow).",
                n = report
                    .missing
                    .first()
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "<N>".into())
            );
            println!(
 "  hint: if it is ANOTHER project's round (an upstream you cite), it is NOT yours to \
  backfill — an entry here would record a decision you never made. This gate reads commit \
  SUBJECTS only: keep the token in the commit body instead, or set \
  `[commit_ledger].severity = warn` (Round 377)."
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
    let anchor = loaded
        .config_path
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| loaded.workspace_root.clone());
    // Shared path with the MCP tool of the same name (Round 686) — one report
    // shape, so the two surfaces cannot drift on what the migration lists.
    let report =
        mnemosyne_ops::binding_kind_migration(&anchor, None).map_err(|e| anyhow!("{e}"))?;
    if json {
        println!("{}", serde_json::to_string(&report)?);
        return Ok(());
    }
    match report.from_schema_version {
        None => {
            println!("store already at current schema (>= v5); no binding-kind migration pending");
        }
        Some(from) => {
            println!("=== binding-kind migration report (store schema v{from} → v5) ===");
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
/// facts usually live in non-dogfood stores). `--rules` overrides the
/// declared `narrative-rules/v1` artifact (Round 449; same pin-bypass rule).
fn cmd_validate_continuity(args: &[String]) -> Result<()> {
    use mnemosyne_config::Severity;
    let mut json = false;
    let mut severity_override: Option<String> = None;
    let mut interval_severity_override: Option<String> = None;
    let mut order_override: Option<String> = None;
    let mut rules_override: Option<String> = None;
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
            "--interval-severity" => {
                interval_severity_override = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--interval-severity missing"))?
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
            "--rules" => {
                rules_override = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--rules missing"))?
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
        rules_override.as_deref(),
    )
    .map_err(|e| anyhow!("{e}"))?;
    if let Some(s) = &severity_override {
        let parsed = Severity::from_tag(s.trim())
            .ok_or_else(|| anyhow!("--severity must be `reject`, `warn`, or `info`"))?;
        report.severity = Some(parsed.as_str().to_string());
    }
    if let Some(s) = &interval_severity_override {
        let parsed = Severity::from_tag(s.trim())
            .ok_or_else(|| anyhow!("--interval-severity must be `reject`, `warn`, or `info`"))?;
        report.interval_severity = Some(parsed.as_str().to_string());
    }
    let severity = report.severity.as_deref().and_then(Severity::from_tag);
    let interval_severity = report
        .interval_severity
        .as_deref()
        .and_then(Severity::from_tag);
    // Per-class gating (Round 491), single-sourced through the shared
    // `evaluate_continuity_gate` (Round 592) that `propose-verdict` also uses, so
    // the dry run mirrors this gate exactly. Structural violations ride
    // `severity`; interval (timeline) violations ride `interval_severity` (OFF by
    // default = surface-not-gate).
    let gate = mnemosyne_validate::continuity::evaluate_continuity_gate(
        severity,
        interval_severity,
        &report.violations,
    );
    let structural_count = gate.structural_count;
    if json {
        println!("{}", serde_json::to_string(&report)?);
    } else {
        match severity {
            None => println!("continuity gate: disabled ([continuity] table absent)"),
            Some(s) => println!("=== continuity gate ({}) ===", s.as_str()),
        }
        println!(
            "  facts={} order_nodes={}/{} sections conflict_pairs={} cross_scope(data)={} \
             unordered={}",
            report.facts,
            report.order_nodes,
            report.sections,
            report.conflict_pairs_checked,
            report.cross_scope_pairs,
            report.unordered_pairs
        );
        if report.rules > 0 {
            println!(
                "  rules={} rule_unordered={} unchained_state_pairs={} interval_unverifiable={} interval_severity={}",
                report.rules,
                report.rule_unordered_pairs,
                report.unchained_state_pairs,
                report.interval_unverifiable,
                interval_severity.map_or("off", Severity::as_str)
            );
        } else {
            // Zero-rules NOTICE (Round 664). The count above printed only when
            // NONZERO, so a run with no rules file wired — or a wired one that
            // declares an empty `rules` array — reported a GREEN gate that never
            // said the word `rules` at all. Exactly 0 is the one case where the
            // author most needs to hear it, and it was the one case kept silent:
            // an unmentioned leg reads as a leg that passed. Name the OFF state
            // and hand over the lever, the R491 interval-NOTICE pattern applied
            // to the rules leg itself (R663: a GREEN that measured nothing must
            // say so — silence is what the consumer takes as coverage).
            println!(
                "  NOTICE: 0 narrative rules declared \u{2014} the exclusive / transition / \
                 interval classes evaluated NOTHING; this run checked recorded conflict pairs \
                 only. Declare a `narrative-rules/v1` file and wire it via \
                 [continuity].rules_path (or pass --rules <file>) to turn them on; \
                 `describe-schema` documents the classes and the wire."
            );
        }
        println!(
            "  violations: {} (structural={} interval={})",
            report.violation_count, structural_count, report.interval_violation_count
        );
        for v in &report.violations {
            println!("  {}", serde_json::to_string(v)?);
        }
        // Interval opt-in NOTICE (Round 491): a declared interval rule is
        // surface-only until `interval_severity = reject`. Name the count so a
        // declared-but-ungated timeline rule is loud, not a silent surprise
        // (verify-before-claiming: the OFF default is deliberate — a gap can be
        // an authored time-bend — so this nudges, it does not gate).
        if report.interval_rules > 0 && !matches!(interval_severity, Some(s) if s.is_reject()) {
            println!(
                "  NOTICE: {} interval rule(s) declared but interval_severity is {} \u{2014} \
                 timeline gaps are SURFACED, not gated; set [continuity].interval_severity = \
                 reject to gate them (a gap can be a deliberate time-bend).",
                report.interval_rules,
                interval_severity.map_or("off", Severity::as_str)
            );
        }
        // Road declaration-completeness NOTICE (Round 614). A branch that declares no
        // road segment rides its lineage's road on, so its ENDING is the trunk's. That
        // is correct for a world-line that diverges only in FACTS — and WRONG for a
        // divergent ending whose road was simply never declared, whose terminal gates
        // are then measuring the trunk's ending instead of its own. The substrate
        // cannot tell the two apart, so name the ambiguity and hand over the lever
        // rather than silently picking a reading (the R504 footgun this NOTICE closes).
        if !report.undeclared_roads.is_empty() {
            println!(
                "  NOTICE: {} branch(es) declare no road of their own \u{2014} {}. Each rides \
                 its lineage's road on, so its ENDING is the trunk's. Correct if it diverges \
                 only in FACTS; if it is a DIVERGENT ENDING, declare its road in the \
                 canon-order (\"branches\": {{\"<id>\": [[\"<fork-point>\", \"<its own scene>\"]]}}) \
                 \u{2014} until you do, validate-render-fidelity cannot tell its ending from \
                 the trunk's.",
                report.undeclared_roads.len(),
                report.undeclared_roads.join(", ")
            );
        }
        // Section-side road-completeness NOTICE (Round 667) — the sibling of the
        // R614 branch-side one above: that names a WORLD-LINE with no road of its
        // own, this names SECTIONS no road reaches. Not a gate: road completeness
        // is the author's todo (R442/R596).
        //
        // It exists because the lone `order_nodes` this command printed MISLED a
        // reader into a false census (R663): the number invited the comparison
        // with the registry and did not make it, and the silence read as "the
        // substrate CANNOT make it" — the R620/R638 class, where a probe that
        // answers "absent" for something present is worse than no probe. The
        // denominator is now on the line above; this states the subtraction.
        //
        // It reports the ARITHMETIC only and never re-derives the list:
        // placement has one resolver, `report-authoring-frontier`'s
        // `unplaced_scenes` (Round 667), and this count is exactly that set's
        // size — the reader who follows the pointer finds the same number and
        // the ids behind it (the identity is pinned in that crate's tests). The
        // first cut of this notice pointed at `unordered_scenes` instead and
        // told the reader its counts were "narrower"; on Round 663's own store
        // that read "1 sit on no declared road" over `unordered scenes: none` +
        // three zero-fact scenes, which is this notice's own disease. A pointer
        // must land on the set it counts.
        // GUARDED on a declared order, exactly as its two siblings are guarded
        // (R614 needs a registered branch, R491 a declared interval rule): an
        // order with NO nodes is not an incomplete order, it is a store that
        // never declared one — a SPEC store (facts=0, no scenes, `[continuity]`
        // absent) is the common shape, and Mnemosyne's own reads `0/5`. Unguarded
        // this notice told that store its five spec sections were unrenderable
        // scenes, under a gate the line above declares disabled; SCE's ~370
        // sections are the same shape. A missing order is not this notice's
        // business and is not lost — `report-authoring-frontier` already reports
        // EVERY fact-bearing scene as unordered when no order is declared (R596).
        if report.order_nodes > 0 && report.sections > report.order_nodes {
            println!(
                "  NOTICE: the canon order places {} of {} section(s) \u{2014} {} unplaced. A \
                 store renders only where the order places it, so a fact anchored to an unplaced \
                 section can never be rendered; the section may also be unplaced YET, which is \
                 the author's todo and never gated. Run report-authoring-frontier: `unplaced \
                 scenes` names these {}.",
                report.order_nodes,
                report.sections,
                report.sections - report.order_nodes,
                report.sections - report.order_nodes
            );
        }
    }
    if gate.gates {
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

/// R679 — the entity-kind migration worklist (read-only): the distinct
/// unregistered kinds a store uses, each with the `add-entity-kind` call to
/// make. The complete list the R675 gate failure only samples.
fn cmd_report_entity_kind_migration(args: &[String]) -> Result<()> {
    let mut json = false;
    let mut sidecar_override: Option<String> = None;
    let mut iter = args.iter();
    while let Some(a) = iter.next() {
        match a.as_str() {
            "--json" => json = true,
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
    let report = mnemosyne_ops::entity_kind_migration(
        &anchor,
        sidecar_override.as_deref().map(std::path::Path::new),
    )
    .map_err(|e| anyhow!("{e}"))?;
    if json {
        println!("{}", serde_json::to_string(&report)?);
    } else if report.unregistered_kinds.is_empty() {
        println!("entity-kind migration: 0 unregistered kinds — every in-use kind is registered");
    } else {
        println!(
            "entity-kind migration: {} entity(ies) name {} unregistered kind(s) — register each:",
            report.total_entities,
            report.unregistered_kinds.len()
        );
        for row in &report.unregistered_kinds {
            println!(
                "  add-entity-kind --kind {}   # {} entity(ies): {}",
                row.kind,
                row.entities.len(),
                row.entities.join(", ")
            );
        }
    }
    Ok(())
}

/// Parsed flags shared by the per-world narrative report verbs (Round
/// 456 extraction; struct form since Round 466 — the tuple stopped
/// scaling at the fifth field).
struct NarrativeReportArgs {
    json: bool,
    order_override: Option<String>,
    sidecar_override: Option<String>,
    /// Single-world filter (Round 466) — only verbs that opt in accept it.
    world: Option<String>,
    /// Disclosure telling id (Round 506) — only the manuscript carrier opts in.
    telling: Option<String>,
    /// Reading-walk prune (Round 509) — only the manuscript verb opts in.
    reading_walk: bool,
    anchor: std::path::PathBuf,
}

/// Which opt-in flags a narrative report verb accepts (Round 510 — a named
/// struct replacing three positional bools: a call site reads
/// `NarrativeFlags { world: true, telling: true, .. }`, not `(true, true,
/// false)`). A disallowed flag rejects loudly instead of parsing-then-ignoring.
#[derive(Default, Clone, Copy)]
struct NarrativeFlags {
    world: bool,
    telling: bool,
    reading_walk: bool,
}

/// Shared `--json` / `--order` / `--sidecar` (+ the opt-in flags in
/// [`NarrativeFlags`]) parse + workspace-anchor resolution for the per-world
/// narrative report verbs (Round 456 — the second identical copy triggered the
/// extraction; `validate-continuity` keeps its richer parser). `--world` (R466),
/// `--telling` (R506 render-brief carrier), and `--reading-walk` (R509 begins>0
/// prune) are each opt-in.
fn parse_narrative_report_args(
    args: &[String],
    flags: NarrativeFlags,
) -> Result<NarrativeReportArgs> {
    let mut json = false;
    let mut order_override: Option<String> = None;
    let mut sidecar_override: Option<String> = None;
    let mut world: Option<String> = None;
    let mut telling: Option<String> = None;
    let mut reading_walk = false;
    let mut iter = args.iter();
    while let Some(a) = iter.next() {
        match a.as_str() {
            "--json" => json = true,
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
            "--world" if flags.world => {
                world = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--world missing"))?
                        .clone(),
                )
            }
            "--telling" if flags.telling => {
                telling = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--telling missing"))?
                        .clone(),
                )
            }
            "--reading-walk" if flags.reading_walk => reading_walk = true,
            other => bail!("unknown flag `{}`", other),
        }
    }
    let loaded = workspace_config()?;
    let anchor = loaded
        .config_path
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| loaded.workspace_root.clone());
    Ok(NarrativeReportArgs {
        json,
        order_override,
        sidecar_override,
        world,
        telling,
        reading_walk,
        anchor,
    })
}

/// Round 442 — setup/payoff coverage (`report-payoff-coverage`): per query
/// world, every setup (`payoff_expectation = expected`) classified paid /
/// dangling against the world-visible payoff edges; unmarked facts are
/// exempt. Pure read projection — dangling is the author's todo list,
/// deliberately never gated (a WIP story has dangling setups by
/// definition). Order and store resolve through the shared ops path.
fn cmd_report_payoff_coverage(args: &[String]) -> Result<()> {
    let a = parse_narrative_report_args(args, NarrativeFlags::default())?;
    let json = a.json;
    let report = mnemosyne_ops::payoff_coverage_report(
        &a.anchor,
        a.sidecar_override.as_deref().map(std::path::Path::new),
        a.order_override.as_deref(),
    )
    .map_err(|e| anyhow!("{e}"))?;
    if json {
        println!("{}", serde_json::to_string(&report)?);
    } else {
        println!(
            "=== payoff coverage — {} fact(s), {} setup(s) ===",
            report.facts, report.setups_total
        );
        for e in &report.uncredited_edges {
            println!(
                "  [UNCREDITED EDGE] {} -> {} (credits in no world)",
                e.payoff, e.setup
            );
        }
        for e in &report.undecidable_edges {
            println!(
                "  [UNDECIDABLE EDGE] {} -> {} (a could-credit world cannot decide it under the declared order)",
                e.payoff, e.setup
            );
        }
        for (world, cov) in &report.worlds {
            println!(
                "world `{world}`: paid={} dangling={} exempt={} unknown={}",
                cov.paid.len(),
                cov.dangling.len(),
                cov.exempt,
                cov.unknown.len()
            );
            for p in &cov.paid {
                println!("  [paid] {} <- {}", p.setup, p.payoffs.join(", "));
            }
            for d in &cov.dangling {
                println!("  [DANGLING] {d}");
            }
            for e in &cov.payoffs_to_unmarked {
                println!(
                    "  [payoff->unmarked] {} -> {} (forgotten setup marking?)",
                    e.payoff, e.setup
                );
            }
            for e in &cov.payoff_before_setup {
                println!(
                    "  [payoff-before-setup] {} precedes {} (surfaced, never gated)",
                    e.payoff, e.setup
                );
            }
            for u in &cov.unknown {
                println!("  [unknown under declared order] {u}");
            }
        }
    }
    Ok(())
}

/// Round 455 — dramatic-irony intervals (`report-irony-intervals`, design
/// sec 7.14): per query world, every recorded CROSS-FRAME conflict edge
/// classified as a co-hold window (where both ends are simultaneously in
/// effect under the one holds-semantics), windowless, unordered (Round
/// 456), or undecidable (B-1). Pure read projection — irony is craft
/// signal, deliberately never gated. Order and store resolve through the
/// shared ops path.
fn cmd_report_irony_intervals(args: &[String]) -> Result<()> {
    let a = parse_narrative_report_args(args, NarrativeFlags::default())?;
    let json = a.json;
    let report = mnemosyne_ops::irony_intervals_report(
        &a.anchor,
        a.sidecar_override.as_deref().map(std::path::Path::new),
        a.order_override.as_deref(),
    )
    .map_err(|e| anyhow!("{e}"))?;
    if json {
        println!("{}", serde_json::to_string(&report)?);
    } else {
        println!(
            "=== irony intervals — {} fact(s), {} cross-frame edge(s), {} same-frame (gate territory) ===",
            report.facts, report.cross_frame_edges, report.same_frame_edges
        );
        for (world, irony) in &report.worlds {
            if irony.windows.is_empty()
                && irony.windowless.is_empty()
                && irony.unordered.is_empty()
                && irony.undecidable.is_empty()
            {
                continue;
            }
            println!(
                "world `{world}`: windows={} windowless={} unordered={} undecidable={}",
                irony.windows.len(),
                irony.windowless.len(),
                irony.unordered.len(),
                irony.undecidable.len()
            );
            for w in &irony.windows {
                println!(
                    "  [window{}] {} ({}) vs {} ({}): {} node(s), opens at {}",
                    if w.open {
                        ", OPEN at world-line end"
                    } else {
                        ""
                    },
                    w.fact_a,
                    w.frame_a,
                    w.fact_b,
                    w.frame_b,
                    w.nodes.len(),
                    w.starts.join(", ")
                );
            }
            for e in &irony.windowless {
                println!(
                    "  [windowless] {} vs {} (visible together, never co-hold)",
                    e.fact_a, e.fact_b
                );
            }
            for e in &irony.unordered {
                println!(
                    "  [unordered] {} vs {} (the declared order cannot compare the starts)",
                    e.fact_a, e.fact_b
                );
            }
            for e in &irony.undecidable {
                println!(
                    "  [undecidable under declared order] {} vs {}",
                    e.fact_a, e.fact_b
                );
            }
        }
    }
    Ok(())
}

/// Round 466 — playthrough manuscript (`report-playthrough-manuscript`,
/// design sec 7.17): per query world (or the single `--world` filter), the
/// composed canon order's deterministic topological walk with declared
/// fact events placed on it — begins, ends (expired / superseded), and the
/// holds-judged count per scene. The answer to "a human cannot read the
/// graph": N worlds = N linear manuscripts. Pure read projection — a
/// reading surface, deliberately never gated. Order and store resolve
/// through the shared ops path.
fn cmd_report_playthrough_manuscript(args: &[String]) -> Result<()> {
    let a = parse_narrative_report_args(
        args,
        NarrativeFlags {
            world: true,
            telling: true,
            reading_walk: true,
        },
    )?;
    let report = mnemosyne_ops::playthrough_manuscript_report(
        &a.anchor,
        a.sidecar_override.as_deref().map(std::path::Path::new),
        a.world.as_deref(),
        a.order_override.as_deref(),
        a.telling.as_deref(),
        a.reading_walk,
    )
    .map_err(|e| anyhow!("{e}"))?;
    if a.json {
        println!("{}", serde_json::to_string(&report)?);
        return Ok(());
    }
    println!(
        "=== playthrough manuscript — {} fact(s), {} world(s) ===",
        report.facts,
        report.worlds.len()
    );
    for (world, m) in &report.worlds {
        println!(
            "world `{world}`: {} scene(s), undeclared adjacencies={}, unplaced={}, \
             undecidable={}, off road={}",
            m.scenes.len(),
            m.undeclared_adjacencies.len(),
            m.unplaced_facts.len(),
            m.undecidable.len(),
            m.sections_off_road.len()
        );
        for s in &m.scenes {
            let title = if s.title.is_empty() {
                String::new()
            } else {
                format!(" — {}", s.title)
            };
            println!(
                "  {}{} [begins={} ends={} holding={}]",
                s.section,
                title,
                s.begins.len(),
                s.ends.len(),
                s.holding_count
            );
            for e in &s.begins {
                match &e.disclosure {
                    Some(d) => {
                        let at = d
                            .first_at
                            .as_deref()
                            .map(|c| format!(" first_at={c}"))
                            .unwrap_or_default();
                        let surf = d
                            .surface
                            .as_ref()
                            .map(|s| match &s.object {
                                Some(o) => format!(" via {}/{}", s.scene, o),
                                None => format!(" via {}", s.scene),
                            })
                            .unwrap_or_default();
                        println!(
                            "    + {} ({}) [{}{}{}]: {}",
                            e.fact_id,
                            e.frame,
                            d.mode.as_str(),
                            at,
                            surf,
                            e.claim
                        );
                    }
                    None => println!("    + {} ({}): {}", e.fact_id, e.frame, e.claim),
                }
            }
            for e in &s.ends {
                match &e.by {
                    Some(by) => println!("    - {} ({}): superseded by {}", e.fact_id, e.frame, by),
                    None => println!("    - {} ({}): expires here", e.fact_id, e.frame),
                }
            }
        }
        for adj in &m.undeclared_adjacencies {
            println!(
                "  [undeclared adjacency] {} | {} (one valid reading — the order does not \
                 compare them)",
                adj[0], adj[1]
            );
        }
        for u in &m.unplaced_facts {
            println!(
                "  [unplaced] {} {} -> {}{} (coordinate outside this world's order)",
                u.fact_id,
                u.field,
                u.coordinate,
                u.successor
                    .as_deref()
                    .map(|s| format!(" (successor {s})"))
                    .unwrap_or_default()
            );
        }
        for f in &m.undecidable {
            println!("  [undecidable under declared order] {f}");
        }
    }
    Ok(())
}

/// Round 497 — fork tree (`report-fork-tree`, design sec 7.21): the
/// cross-world CHOICE GRAPH the CYOA renderer assumes — every registered
/// world-line with its divergence coordinate (parent + fork point + the
/// branch description = the CYOA choice label), the fork point resolved
/// against the parent's composed order. Per-world manuscripts (R466) gave N
/// linear readings; this is the tree that stitches them at the fork points.
/// Pure read projection — a reading surface, deliberately never gated. Order
/// and store resolve through the shared ops path.
fn cmd_report_fork_tree(args: &[String]) -> Result<()> {
    let a = parse_narrative_report_args(args, NarrativeFlags::default())?;
    let report = mnemosyne_ops::fork_tree_report(
        &a.anchor,
        a.sidecar_override.as_deref().map(std::path::Path::new),
        a.order_override.as_deref(),
    )
    .map_err(|e| anyhow!("{e}"))?;
    if a.json {
        println!("{}", serde_json::to_string(&report)?);
        return Ok(());
    }
    println!(
        "=== fork tree — {} registered world-line(s), {} unplaced fork point(s) ===",
        report.branch_count,
        report.unplaced_fork_points.len()
    );
    for b in &report.branches {
        match &b.fork {
            // A confluence has no fork edge but is NOT standalone — its merges
            // print below; only a true root world is "standalone" (Round 532).
            None if b.converges.is_empty() => {
                println!("  `{}` (standalone world)", b.branch_id)
            }
            None => {}
            Some(f) => println!(
                "  `{}` forks from `{}` at {}{}",
                b.branch_id,
                f.parent,
                f.at,
                if f.at_placed {
                    ""
                } else {
                    " [UNPLACED — not a node of the parent's order]"
                }
            ),
        }
        // Round 532 — incoming merges (confluence): the join made visible.
        for c in &b.converges {
            println!(
                "  `{}` converges from `{}` at {}{}",
                b.branch_id,
                c.parent,
                c.at,
                if c.at_placed {
                    ""
                } else {
                    " [UNPLACED — not a node of the parent's order]"
                }
            );
        }
        if !b.description.is_empty() {
            println!("      choice: {}", b.description);
        }
    }
    Ok(())
}

/// Round 556/557 — playable world (`report-playable-world`, design sec 7.37):
/// the `map_locator` seam a pinion narrative runtime consumes — per telling,
/// the cross-world fork topology (R497) + each world-line's scene walk (R466) +
/// the per-scene disclosure MapLocators (the R510 resolver). `--telling` is
/// required (a playable world IS a telling); `--world` optionally narrows the
/// per-world map (the fork tree stays full). Pure read projection, never gated.
/// Order and store resolve through the shared ops path.
fn cmd_report_playable_world(args: &[String]) -> Result<()> {
    let a = parse_narrative_report_args(
        args,
        NarrativeFlags {
            world: true,
            telling: true,
            ..Default::default()
        },
    )?;
    let telling = a
        .telling
        .as_deref()
        .ok_or_else(|| anyhow!("--telling arg required"))?;
    let report = mnemosyne_ops::playable_world_report(
        &a.anchor,
        a.sidecar_override.as_deref().map(std::path::Path::new),
        a.world.as_deref(),
        a.order_override.as_deref(),
        telling,
    )
    .map_err(|e| anyhow!("{e}"))?;
    if a.json {
        println!("{}", serde_json::to_string(&report)?);
        return Ok(());
    }
    println!(
        "=== playable world — telling `{}` — {} world(s), {} registered branch(es) ===",
        report.telling,
        report.worlds.len(),
        report.fork_tree.branch_count
    );
    for (world, w) in &report.worlds {
        let m = &w.manuscript;
        println!(
            "world `{world}`: {} scene(s), {} locator(s), undeclared_adjacencies={}, \
             unplaced={}, undecidable={}",
            m.scenes.len(),
            w.locators.len(),
            m.undeclared_adjacencies.len(),
            m.unplaced_facts.len(),
            m.undecidable.len()
        );
        for loc in &w.locators {
            let ord = loc
                .scene_ordinal
                .map(|o| o.to_string())
                .unwrap_or_else(|| "unplaced".to_string());
            let obj = loc
                .object
                .as_deref()
                .map(|o| format!("/{o}"))
                .unwrap_or_default();
            let at = loc
                .first_at
                .as_deref()
                .map(|c| format!(" first_at={c}"))
                .unwrap_or_default();
            println!(
                "  [{}] {} @ {}{} (#{}){}",
                loc.mode.as_str(),
                loc.fact_id,
                loc.scene,
                obj,
                ord,
                at
            );
        }
    }
    Ok(())
}

/// Round 559/568 — quest graph (`report-quest-graph`, design sec 7.38): the
/// fact->quest leg, the sibling of `report-playable-world`. Per telling, every
/// derived quest (pursues/requires/completed_by role) projected to a QuestNode — objective + actor
/// (`pursues`) + prerequisites (`requires`) + giving setups + per-world derived
/// open/done (the R442 payoff coverage) + completion fact + giver-surface
/// locator (R557). A pure JOIN, never gated; quest state DERIVED per world-line.
fn cmd_report_quest_graph(args: &[String]) -> Result<()> {
    let a = parse_narrative_report_args(
        args,
        NarrativeFlags {
            world: true,
            telling: true,
            ..Default::default()
        },
    )?;
    let telling = a
        .telling
        .as_deref()
        .ok_or_else(|| anyhow!("--telling arg required"))?;
    let report = mnemosyne_ops::quest_graph_report(
        &a.anchor,
        a.sidecar_override.as_deref().map(std::path::Path::new),
        a.world.as_deref(),
        a.order_override.as_deref(),
        telling,
    )
    .map_err(|e| anyhow!("{e}"))?;
    if a.json {
        println!("{}", serde_json::to_string(&report)?);
        return Ok(());
    }
    println!(
        "=== quest graph — telling `{}` — {} quest(s), {} world(s), {} registered branch(es) ===",
        report.telling,
        report.quests.len(),
        report.worlds.len(),
        report.fork_tree.branch_count
    );
    if !report.unresolved_quests.is_empty() {
        println!(
            "unresolved (no completed_by anchor): {}",
            report.unresolved_quests.join(", ")
        );
    }
    for q in &report.quests {
        println!("quest `{}`: {}", q.quest_id, q.objective);
        let or_dash = |v: &[String]| {
            if v.is_empty() {
                "—".to_string()
            } else {
                v.join(", ")
            }
        };
        println!(
            "  led by: {} | requires: {} | giving: {}",
            or_dash(&q.actors),
            or_dash(&q.prerequisites),
            or_dash(&q.giving_facts)
        );
        for (world, state) in &q.per_world {
            let detail = state
                .completions
                .iter()
                .map(|c| {
                    let by = c
                        .actor
                        .as_deref()
                        .map(|a| format!(" by {a}"))
                        .unwrap_or_default();
                    format!("{} @ {}{}", c.fact, c.scene, by)
                })
                .collect::<Vec<_>>()
                .join("; ");
            if detail.is_empty() {
                println!("    {world}: {}", state.state.as_str());
            } else {
                println!("    {world}: {} ({detail})", state.state.as_str());
            }
        }
        for loc in &q.locators {
            let ord = loc
                .scene_ordinal
                .map(|o| o.to_string())
                .unwrap_or_else(|| "unplaced".to_string());
            let obj = loc
                .object
                .as_deref()
                .map(|o| format!("/{o}"))
                .unwrap_or_default();
            println!(
                "    giver[{}] {} @ {}{} (#{}) [{}]",
                loc.mode.as_str(),
                loc.fact_id,
                loc.scene,
                obj,
                ord,
                loc.world_line
            );
        }
    }
    Ok(())
}

/// Round 587 — the medium-neutral authoring contract (`describe-schema`, R585
/// debt item 1): the registries, fact shape, fixed vocabularies, rule classes,
/// quest encoding, and write-time invariants an external generate-gate-repair
/// agent reads to self-serve instead of reading source. STATIC and
/// store-independent (the contract is fixed; store CONTENTS are `query` /
/// `list-*`). `--json` emits the full machine form.
fn cmd_describe_schema(args: &[String]) -> Result<()> {
    let mut json = false;
    for arg in args {
        match arg.as_str() {
            "--json" => json = true,
            other => return Err(anyhow!("describe-schema: unexpected arg `{other}`")),
        }
    }
    let c = mnemosyne_ops::describe_schema();
    if json {
        println!("{}", serde_json::to_string_pretty(&c)?);
        return Ok(());
    }
    println!("=== authoring contract (schema v{}) ===", c.schema_version);
    println!("{}", c.overview);
    println!("\n-- registries (declare an id here before a fact references it) --");
    for r in &c.registries {
        let lb = if r.load_bearing {
            " [load-bearing]"
        } else {
            ""
        };
        println!("  {} (key: {}, via {}){}", r.name, r.key, r.add_op, lb);
        println!("    referenced by: {}", r.referenced_by);
        println!("    {}", r.description);
    }
    println!("\n-- fact ({}) --", c.fact.add_op);
    println!("  {}", c.fact.description);
    for f in &c.fact.fields {
        let req = if f.required { "required" } else { "optional" };
        println!("  {} : {} [{}] — {}", f.name, f.ty, req, f.description);
    }
    println!("\n-- typed claim --");
    println!("  {}", c.typed_claim.description);
    println!("  subject: {}", c.typed_claim.subject);
    println!("  predicate: {}", c.typed_claim.predicate);
    for o in &c.typed_claim.object_shapes {
        println!("  object `{}`: {}", o.value, o.description);
    }
    println!("\n-- fixed vocabularies --");
    for v in &c.vocabularies {
        let def = v
            .default
            .map(|d| format!(" (default {d})"))
            .unwrap_or_default();
        println!("  {}{} — applies to {}", v.name, def, v.applies_to);
        for val in &v.values {
            println!("    {} : {}", val.value, val.description);
        }
    }
    println!("\n-- narrative-rule classes (deterministic continuity gate) --");
    for r in &c.narrative_rules {
        println!("  {} — {}", r.class, r.description);
        for p in &r.parameters {
            println!("    {} : {} — {}", p.name, p.ty, p.description);
        }
    }
    println!("\n-- quest encoding (derived from predicate roles, no kind marker) --");
    println!("  {}", c.quest_encoding.description);
    println!("  derivation: {}", c.quest_encoding.derivation);
    for p in &c.quest_encoding.predicates {
        println!("  {} [{}] — {}", p.predicate, p.object_shape, p.role);
    }
    println!("  completion: {}", c.quest_encoding.completion_rule);
    println!("  state: {}", c.quest_encoding.state_derivation);
    println!("\n-- write-time invariants (fail-loud) --");
    for inv in &c.invariants {
        println!("  {} @ {}", inv.name, inv.enforced_at);
        println!("    {}", inv.rule);
    }
    println!("\n-- out-of-band enforcement --");
    println!("  {}", c.invariant_enforcement);
    let w = &c.manifest_wire;
    println!("\n-- manifest wire format ({}) --", w.add_op);
    println!("  {}", w.overview);
    for k in &w.kinds {
        println!("  {}: {}", k.kind, k.json_keys);
    }
    println!("  typed object: {}", w.typed_object_wire);
    println!("\n  worked example (copy and adapt):");
    println!("{}", w.example_json);
    println!("\n-- canon order (required for a renderable store) --");
    println!("  {}", c.canon_order);
    println!("\n-- disclosure encoding (per-road secrets; frontier != leak gate) --");
    println!("  {}", c.disclosure_encoding);
    println!("\n-- narrative rules wire (declare + wire a rule so the gate enforces it) --");
    println!("  {}", c.narrative_rules_wire);
    Ok(())
}

/// Round 588 — propose-verdict (R585 debt item 2): the generate-gate-repair
/// loop's atomic dry-run gate. Apply a candidate `--manifest` to a THROWAWAY
/// clone of the store, run the shape invariants + the continuity gate, and emit
/// commit-or-rollback plus actionable violations (rule + locus + expected +
/// repair hint) AND the per-world dangling setups the batch would leave (Round
/// 599, advisory + non-gating — so a loop sees a structural dangling in the dry
/// run, before it commits). The real store is NEVER written; exit 1 on rollback
/// so a loop can branch on the code. `--order` / `--rules` bypass the pins;
/// `--sidecar` proposes against a non-default base store.
fn cmd_propose_verdict(args: &[String]) -> Result<()> {
    let mut manifest_path: Option<String> = None;
    let mut order_override: Option<String> = None;
    let mut rules_override: Option<String> = None;
    let mut sidecar_override: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(a) = iter.next() {
        match a.as_str() {
            "--manifest" => {
                manifest_path = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--manifest missing"))?
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
            "--rules" => {
                rules_override = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--rules missing"))?
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
            "--json" => json = true,
            other => bail!("unknown flag `{}`", other),
        }
    }
    let manifest_path = manifest_path.ok_or_else(|| anyhow!("--manifest <path> arg required"))?;
    let raw = std::fs::read_to_string(&manifest_path)
        .with_context(|| format!("read manifest {}", manifest_path))?;
    let manifest: mnemosyne_atomic::FactsManifest =
        serde_json::from_str(&raw).with_context(|| {
            format!(
                "parse manifest {} ({})",
                manifest_path,
                mnemosyne_atomic::FACTS_MANIFEST_SHAPE
            )
        })?;
    let loaded = workspace_config()?;
    let anchor = loaded
        .config_path
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| loaded.workspace_root.clone());
    let report = mnemosyne_ops::propose_verdict(
        &anchor,
        sidecar_override.as_deref().map(std::path::Path::new),
        order_override.as_deref(),
        rules_override.as_deref(),
        &manifest,
    )
    .map_err(|e| anyhow!("{e}"))?;
    if json {
        println!("{}", serde_json::to_string(&report)?);
    } else {
        println!("=== propose-verdict: {} ===", report.verdict.as_str());
        println!("would apply: {}", report.applied_summary);
        println!(
            "violations: {} ({} gating at reject severity)",
            report.violation_count, report.gating_violation_count
        );
        for v in &report.violations {
            println!("  [{}] {} — {}", v.source, v.rule, v.message);
            if !v.locus.facts.is_empty() {
                let field = v
                    .locus
                    .field
                    .as_deref()
                    .map(|f| format!(" (field {f})"))
                    .unwrap_or_default();
                println!("    at: {}{}", v.locus.facts.join(", "), field);
            }
            println!("    expected: {}", v.expected);
            println!("    repair: {}", v.repair_hint);
        }
        if report.dangling_setups.is_empty() {
            println!("dangling setups (advisory, non-gating): none");
        } else {
            for (world, facts) in &report.dangling_setups {
                println!(
                    "dangling setups (advisory, non-gating) [{world}] ({}): {}",
                    facts.len(),
                    facts.join(", ")
                );
            }
        }
    }
    if report.verdict == mnemosyne_ops::ProposeVerdict::Rollback {
        std::process::exit(1);
    }
    Ok(())
}

/// Round 589 — authoring frontier (`report-authoring-frontier`, R585 debt item
/// 3): the consolidated coverage-gap surface an unattended loop pulls its next
/// work from — zero-fact scenes + per-scene coverage + per-world dangling setups
/// + (with `--telling`) unresolved quests + never-planned disclosures. A pure
/// read, never gated. `--order` bypasses the pin; `--sidecar` reads a
/// non-default store.
fn cmd_report_authoring_frontier(args: &[String]) -> Result<()> {
    let mut telling: Option<String> = None;
    let mut order_override: Option<String> = None;
    let mut sidecar_override: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(a) = iter.next() {
        match a.as_str() {
            "--telling" => {
                telling = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--telling missing"))?
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
    let report = mnemosyne_ops::authoring_frontier_report(
        &anchor,
        sidecar_override.as_deref().map(std::path::Path::new),
        order_override.as_deref(),
        telling.as_deref(),
    )
    .map_err(|e| anyhow!("{e}"))?;
    if json {
        println!("{}", serde_json::to_string(&report)?);
        return Ok(());
    }
    let telling_label = report.telling.as_deref().unwrap_or("(none)");
    println!(
        "=== authoring frontier — telling {} — {} gap(s) ===",
        telling_label, report.total_gaps
    );
    let list = |label: &str, v: &[String]| {
        if v.is_empty() {
            println!("{label}: none");
        } else {
            println!("{label} ({}): {}", v.len(), v.join(", "));
        }
    };
    list("zero-fact scenes", &report.zero_fact_scenes);
    list("unplaced scenes", &report.unplaced_scenes);
    list("unordered scenes", &report.unordered_scenes);
    if report.dangling_setups.is_empty() {
        println!("dangling setups: none");
    } else {
        for (world, facts) in &report.dangling_setups {
            println!(
                "dangling setups [{world}] ({}): {}",
                facts.len(),
                facts.join(", ")
            );
        }
    }
    match &report.unresolved_quests {
        Some(q) => list("unresolved quests", q),
        None => println!("unresolved quests: (pass --telling)"),
    }
    match &report.never_planned_disclosures {
        Some(d) => list("never-planned disclosures", d),
        None => println!("never-planned disclosures: (pass --telling)"),
    }
    for (world, d) in &report.branch_owned_density {
        let density = match d.density {
            Some(v) => format!("{v:.2}"),
            None => "n/a (no road)".to_string(),
        };
        println!(
            "branch density [{world}]: {} owned fact(s) over {} traversed scene(s) = {}",
            d.owned_facts, d.road_scenes, density
        );
    }
    Ok(())
}

/// Round 507 — disclosure coverage (`report-disclosure-coverage`, design sec
/// 7.24 step 4): per telling, every fact classified disclosed /
/// hidden-by-design / never-planned. A SURFACE (the R442 dangling-is-a-todo
/// discipline) — `never-planned` is the author's todo list, never gated.
fn cmd_report_disclosure_coverage(args: &[String]) -> Result<()> {
    let a = parse_narrative_report_args(
        args,
        NarrativeFlags {
            telling: true,
            ..Default::default()
        },
    )?;
    let telling = a
        .telling
        .as_deref()
        .ok_or_else(|| anyhow!("--telling arg required"))?;
    let report = mnemosyne_ops::disclosure_coverage_report(
        &a.anchor,
        a.sidecar_override.as_deref().map(std::path::Path::new),
        telling,
    )
    .map_err(|e| anyhow!("{e}"))?;
    if a.json {
        println!("{}", serde_json::to_string(&report)?);
        return Ok(());
    }
    println!(
        "=== disclosure coverage — telling `{}` — {} fact(s) ===",
        report.telling, report.facts
    );
    println!(
        "disclosed={} hidden_by_design={} never_planned={}",
        report.disclosed,
        report.hidden_by_design,
        report.never_planned.len()
    );
    for id in &report.never_planned {
        println!("  never-planned: {id}");
    }
    Ok(())
}

/// Resolve the workspace anchor (the config dir) the report verbs hang off —
/// the inline shape `parse_narrative_report_args` uses, extracted for the two
/// render-acceptance gate verbs that carry their own `--against` parser.
fn report_anchor() -> Result<std::path::PathBuf> {
    let loaded = workspace_config()?;
    Ok(loaded
        .config_path
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| loaded.workspace_root.clone()))
}

/// Round 507 — premature-leak gate (`validate-disclosure-leak`, design sec 7.24
/// step 5, R502): the authored plan vs a BLIND RE-EXTRACTED prose store
/// (`--against`), matched by typed tuple in `--truth-frame` for `--world`. A
/// withheld fact that appears, or a fact re-extractable before its first_at, is
/// a leak; the verb exits non-zero on any leak (a gate), the JSON/human report
/// printed first.
fn cmd_validate_disclosure_leak(args: &[String]) -> Result<()> {
    let mut telling: Option<String> = None;
    let mut against: Option<String> = None;
    let mut world: Option<String> = None;
    let mut truth_frame: Option<String> = None;
    let mut order_override: Option<String> = None;
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--telling" => {
                telling = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--telling missing"))?
                        .clone(),
                )
            }
            "--against" => {
                against = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--against missing"))?
                        .clone(),
                )
            }
            "--world" => {
                world = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--world missing"))?
                        .clone(),
                )
            }
            "--truth-frame" => {
                truth_frame = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--truth-frame missing"))?
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
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => bail!("unknown flag `{}`", other),
        }
    }
    let telling = telling.ok_or_else(|| anyhow!("--telling arg required"))?;
    let against = against.ok_or_else(|| anyhow!("--against arg required"))?;
    let world = world.ok_or_else(|| anyhow!("--world arg required"))?;
    let truth_frame = truth_frame.ok_or_else(|| anyhow!("--truth-frame arg required"))?;
    let anchor = report_anchor()?;
    let report = mnemosyne_ops::disclosure_leak_report(
        &anchor,
        sidecar.as_deref().map(std::path::Path::new),
        std::path::Path::new(&against),
        order_override.as_deref(),
        &telling,
        &world,
        &truth_frame,
    )
    .map_err(|e| anyhow!("{e}"))?;
    if json {
        println!("{}", serde_json::to_string(&report)?);
    } else {
        println!(
            "=== disclosure leak — telling `{}` world `{}` truth-frame `{}` — {} targeted ===",
            report.telling, report.world, report.truth_frame, report.targeted
        );
        println!(
            "leaks={} unordered={} unmatched={} truth_frame_typed={} vocabulary_shared={}",
            report.leaks.len(),
            report.unordered.len(),
            report.unmatched.len(),
            report.truth_frame_typed_facts,
            report.vocabulary_shared,
        );
        for l in &report.leaks {
            match &l.first_at {
                Some(fa) => println!(
                    "  LEAK [{}] {} -> {} @{} (first_at {})",
                    l.kind.as_str(),
                    l.fact_id,
                    l.reextracted_id,
                    l.coord,
                    fa
                ),
                None => println!(
                    "  LEAK [{}] {} -> {} @{}",
                    l.kind.as_str(),
                    l.fact_id,
                    l.reextracted_id,
                    l.coord
                ),
            }
        }
        for u in &report.unordered {
            println!(
                "  [unordered] {} -> {} @{}",
                u.fact_id, u.reextracted_id, u.coord
            );
        }
        for u in &report.unmatched {
            println!("  [unmatched] {u}");
        }
    }
    // F5 (Round 510) — a vacuous pass is a LOUD failure, not silent clean: if
    // facts were targeted but the re-extraction shares no vocabulary in the
    // truth frame, the gate matched against nothing (foreign ids / wrong frame)
    // and a `leaks==0` result is meaningless.
    if report.targeted > 0 && report.vocabulary_shared == 0 {
        bail!(
            "disclosure leak gate VACUOUS: {} fact(s) targeted but 0 shared vocabulary in \
             truth-frame `{}` ({} typed fact(s) there) — the re-extraction used foreign ids or \
             the wrong frame; the gate is blind, not clean",
            report.targeted,
            report.truth_frame,
            report.truth_frame_typed_facts
        );
    }
    if !report.leaks.is_empty() {
        bail!(
            "disclosure leak gate FAILED: {} leak(s)",
            report.leaks.len()
        );
    }
    Ok(())
}

/// Round 507 — render↔world-line fidelity gate (`validate-render-fidelity`,
/// design sec 7.24 step 6, R505): a BLIND RE-EXTRACTED prose store
/// (`--against`) checked against `--world`'s composed order — a re-extracted
/// coord that is a declaration node of ANOTHER world is off-path (the prose
/// drifted onto the wrong world-line, the R488 prose analog). Exits non-zero on
/// any off-path fact.
fn cmd_validate_render_fidelity(args: &[String]) -> Result<()> {
    let mut against: Option<String> = None;
    let mut world: Option<String> = None;
    let mut order_override: Option<String> = None;
    let mut sidecar: Option<String> = None;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--against" => {
                against = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--against missing"))?
                        .clone(),
                )
            }
            "--world" => {
                world = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--world missing"))?
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
                sidecar = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--sidecar missing"))?
                        .clone(),
                )
            }
            "--json" => json = true,
            other => bail!("unknown flag `{}`", other),
        }
    }
    let against = against.ok_or_else(|| anyhow!("--against arg required"))?;
    let world = world.ok_or_else(|| anyhow!("--world arg required"))?;
    let anchor = report_anchor()?;
    let report = mnemosyne_ops::render_fidelity_report(
        &anchor,
        sidecar.as_deref().map(std::path::Path::new),
        std::path::Path::new(&against),
        order_override.as_deref(),
        &world,
    )
    .map_err(|e| anyhow!("{e}"))?;
    if json {
        println!("{}", serde_json::to_string(&report)?);
    } else {
        println!(
            "=== render fidelity — world `{}` — {} re-extracted fact(s), reached_terminal={} ===",
            report.world, report.reextracted_facts, report.reached_terminal
        );
        println!(
            "off_path={} unplaced={}",
            report.off_path.len(),
            report.unplaced.len()
        );
        for f in &report.off_path {
            println!(
                "  OFF-PATH {} @{} (not in world `{}`)",
                f.fact_id, f.coord, report.world
            );
        }
        for f in &report.unplaced {
            println!("  [unplaced] {} @{}", f.fact_id, f.coord);
        }
    }
    if !report.off_path.is_empty() {
        bail!(
            "render fidelity gate FAILED: {} off-path fact(s) — the prose drifted off world `{}`",
            report.off_path.len(),
            report.world
        );
    }
    Ok(())
}

/// Round 458 — typing-discovery input package (`report-typing-candidates`,
/// design sec 7.15 Round A): every untyped fact with its claim text + sha256
/// pin + frame/branch/entities, plus the registered predicate and entity
/// vocabulary, in one deterministic call. Order-independent (no `--order`:
/// typing is a property of the fact, not of any canon declaration). The
/// proposer is an LLM agent OUTSIDE the substrate; this verb is its entire
/// input contract.
fn cmd_report_typing_candidates(args: &[String]) -> Result<()> {
    let mut json = false;
    let mut sidecar_override: Option<String> = None;
    let mut iter = args.iter();
    while let Some(a) = iter.next() {
        match a.as_str() {
            "--json" => json = true,
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
    let report = mnemosyne_ops::typing_candidates_report(
        &anchor,
        sidecar_override.as_deref().map(std::path::Path::new),
    )
    .map_err(|e| anyhow!("{e}"))?;
    if json {
        println!("{}", serde_json::to_string(&report)?);
    } else {
        println!(
            "=== typing candidates — {} untyped / {} fact(s) ({} typed) ===",
            report.candidates.len(),
            report.facts,
            report.typed
        );
        for c in &report.candidates {
            println!(
                "  {} (frame {} / branch {} @ {}): {}",
                c.fact_id, c.frame, c.branch, c.canon_from, c.claim
            );
        }
        println!("vocabulary: {} predicate(s)", report.predicates.len());
        for (id, p) in &report.predicates {
            println!("  {} ({:?}): {}", id, p.object_kind, p.description);
        }
        println!("entities: {} registered", report.entities.len());
    }
    Ok(())
}

/// Round 459 — reviewed import of proposed typed legs
/// (`import-typing-proposals`, design sec 7.15 Round B): all-or-nothing
/// with full per-proposal verdicts, `--dry-run` for the review loop.
/// Exit 1 whenever any proposal rejects (both modes — scripts must see
/// it), exit 0 only on a fully-accepted file.
fn cmd_import_typing_proposals(args: &[String]) -> Result<()> {
    let mut json = false;
    let mut dry_run = false;
    let mut proposals: Option<String> = None;
    let mut sidecar_override: Option<String> = None;
    let mut iter = args.iter();
    while let Some(a) = iter.next() {
        match a.as_str() {
            "--json" => json = true,
            "--dry-run" => dry_run = true,
            "--proposals" => {
                proposals = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--proposals missing"))?
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
    let proposals = proposals.ok_or_else(|| anyhow!("--proposals <path> arg required"))?;
    let loaded = workspace_config()?;
    let anchor = loaded
        .config_path
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| loaded.workspace_root.clone());
    let report = mnemosyne_ops::import_typing_proposals_report(
        &anchor,
        sidecar_override.as_deref().map(std::path::Path::new),
        std::path::Path::new(&proposals),
        dry_run,
    )
    .map_err(|e| anyhow!("{e}"))?;
    if json {
        println!("{}", serde_json::to_string(&report)?);
    } else {
        println!(
            "=== typing proposals {} — file sha256 {} ===",
            if report.dry_run {
                "(dry run)"
            } else {
                "import"
            },
            report.file_sha256.get(..16).unwrap_or(&report.file_sha256)
        );
        for v in &report.verdicts {
            println!("  [{}] {}", v.fact, v.verdict);
        }
        println!(
            "accepted={} rejected={} applied={}",
            report.accepted, report.rejected, report.applied
        );
    }
    if report.rejected > 0 {
        std::process::exit(1);
    }
    Ok(())
}

/// Round 485 — deterministic payoff substantiation
/// (`report-payoff-substantiation`): each credited setup is classified
/// substantiated (a payoff carries a typed state-change discharging the setup's
/// typed state) / unsubstantiated (typed setup, no discharging payoff — a hollow
/// payoff) / unverifiable (the setup is untyped, so no discharge is definable —
/// type it). No LLM; pure deterministic comparison of declared typed legs (the
/// R484 redesign that replaced the R481 drift-verdict surface).
fn cmd_report_payoff_substantiation(args: &[String]) -> Result<()> {
    let a = parse_narrative_report_args(args, NarrativeFlags::default())?;
    let report = mnemosyne_ops::payoff_substantiation_report(
        &a.anchor,
        a.sidecar_override.as_deref().map(std::path::Path::new),
        a.order_override.as_deref(),
    )
    .map_err(|e| anyhow!("{e}"))?;
    if a.json {
        println!("{}", serde_json::to_string(&report)?);
    } else {
        println!(
            "=== payoff substantiation — {} setup(s) ===",
            report.setups_total
        );
        for (world, w) in &report.worlds {
            println!(
                "world `{world}`: substantiated={} unsubstantiated={} unverifiable={}",
                w.substantiated.len(),
                w.unsubstantiated.len(),
                w.unverifiable.len()
            );
            for p in &w.substantiated {
                println!("  [substantiated] {} <- {}", p.setup, p.payoffs.join(", "));
            }
            for p in &w.unsubstantiated {
                println!(
                    "  [UNSUBSTANTIATED] {} <- {} (typed setup, no typed state-change discharges it)",
                    p.setup,
                    p.payoffs.join(", ")
                );
            }
            for p in &w.unverifiable {
                println!(
                    "  [unverifiable] {} <- {} (setup untyped — type it to verify)",
                    p.setup,
                    p.payoffs.join(", ")
                );
            }
        }
    }
    Ok(())
}

/// Round 490 — timeline-gap projection (`report-timeline-gaps`, design sec
/// 7.20 step 2): the deterministic interval evaluator surfaced as a READ
/// report, per world, never gated (surface-not-gate). Resolves the same
/// `narrative-rules` artifact as the continuity gate; only `interval` rules
/// contribute. `--world` filters to one world-line.
fn cmd_report_timeline_gaps(args: &[String]) -> Result<()> {
    use mnemosyne_validate::continuity::IntervalVerdict;
    let mut json = false;
    let mut order_override: Option<String> = None;
    let mut rules_override: Option<String> = None;
    let mut sidecar_override: Option<String> = None;
    let mut world_filter: Option<String> = None;
    let mut iter = args.iter();
    while let Some(a) = iter.next() {
        match a.as_str() {
            "--json" => json = true,
            "--order" => {
                order_override = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--order missing"))?
                        .clone(),
                )
            }
            "--rules" => {
                rules_override = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--rules missing"))?
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
            "--world" => {
                world_filter = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--world missing"))?
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
    let report = mnemosyne_ops::timeline_gaps_report(
        &anchor,
        sidecar_override.as_deref().map(std::path::Path::new),
        order_override.as_deref(),
        rules_override.as_deref(),
    )
    .map_err(|e| anyhow!("{e}"))?;
    if json {
        println!("{}", serde_json::to_string(&report)?);
        return Ok(());
    }
    println!(
        "=== timeline gaps — {} interval rule(s) ===",
        report.interval_rules
    );
    for (world, gaps) in &report.worlds {
        if world_filter.as_deref().is_some_and(|f| f != world) {
            continue;
        }
        let mut violated = 0;
        let mut unverifiable = 0;
        let mut satisfied = 0;
        for o in &gaps.outcomes {
            match &o.verdict {
                IntervalVerdict::Violated { .. } => violated += 1,
                IntervalVerdict::Unverifiable { .. } => unverifiable += 1,
                IntervalVerdict::Satisfied { .. } => satisfied += 1,
            }
        }
        println!(
            "world `{world}`: violated={violated} unverifiable={unverifiable} satisfied={satisfied}"
        );
        for o in &gaps.outcomes {
            match &o.verdict {
                IntervalVerdict::Violated {
                    right_value, bound, ..
                } => println!(
                    "  [VIOLATED] {} {}: {}({}) - {}({}) {} {} @{}",
                    o.subject,
                    o.rule,
                    o.predicate,
                    o.left_value,
                    o.right,
                    right_value,
                    o.op,
                    bound,
                    o.at
                ),
                IntervalVerdict::Unverifiable { reason } => println!(
                    "  [unverifiable] {} {}: {} (type it to verify)",
                    o.subject, o.rule, reason
                ),
                IntervalVerdict::Satisfied {
                    right_value, bound, ..
                } => println!(
                    "  [ok] {} {}: {}({}) - {}({}) {} {} @{}",
                    o.subject,
                    o.rule,
                    o.predicate,
                    o.left_value,
                    o.right,
                    right_value,
                    o.op,
                    bound,
                    o.at
                ),
            }
        }
    }
    Ok(())
}

/// Round 463 — reviewed import of proposed succession/conflict edges
/// (`import-edge-proposals`, design sec 7.16 Round B): all-or-nothing
/// with full per-proposal verdicts, `--dry-run` for the review loop.
/// Exit 1 whenever any proposal rejects (both modes — scripts must see
/// it), exit 0 only on a fully-accepted file.
fn cmd_import_edge_proposals(args: &[String]) -> Result<()> {
    let mut json = false;
    let mut dry_run = false;
    let mut proposals: Option<String> = None;
    let mut sidecar_override: Option<String> = None;
    let mut iter = args.iter();
    while let Some(a) = iter.next() {
        match a.as_str() {
            "--json" => json = true,
            "--dry-run" => dry_run = true,
            "--proposals" => {
                proposals = Some(
                    iter.next()
                        .ok_or_else(|| anyhow!("--proposals missing"))?
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
    let proposals = proposals.ok_or_else(|| anyhow!("--proposals <path> arg required"))?;
    let loaded = workspace_config()?;
    let anchor = loaded
        .config_path
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| loaded.workspace_root.clone());
    let report = mnemosyne_ops::import_edge_proposals_report(
        &anchor,
        sidecar_override.as_deref().map(std::path::Path::new),
        std::path::Path::new(&proposals),
        dry_run,
    )
    .map_err(|e| anyhow!("{e}"))?;
    if json {
        println!("{}", serde_json::to_string(&report)?);
    } else {
        println!(
            "=== edge proposals {} — file sha256 {} ===",
            if report.dry_run {
                "(dry run)"
            } else {
                "import"
            },
            report.file_sha256.get(..16).unwrap_or(&report.file_sha256)
        );
        for v in &report.verdicts {
            println!("  [{} {} -> {}] {}", v.kind, v.fact, v.target, v.verdict);
        }
        println!(
            "accepted={} rejected={} applied={}",
            report.accepted, report.rejected, report.applied
        );
    }
    if report.rejected > 0 {
        std::process::exit(1);
    }
    Ok(())
}

/// Round 462 — edge-discovery input package (`report-edge-candidates`,
/// design sec 7.16 Round A): every fact row with claim text + sha256 pin
/// (proposals stamp BOTH endpoints) + every recorded edge, plus the
/// deterministic succession-gap hints. Order-resolved like the other
/// narrative reads — the hints need world visibility; the facts table
/// never degrades.
fn cmd_report_edge_candidates(args: &[String]) -> Result<()> {
    let a = parse_narrative_report_args(args, NarrativeFlags::default())?;
    let json = a.json;
    let report = mnemosyne_ops::edge_candidates_report(
        &a.anchor,
        a.sidecar_override.as_deref().map(std::path::Path::new),
        a.order_override.as_deref(),
    )
    .map_err(|e| anyhow!("{e}"))?;
    if json {
        println!("{}", serde_json::to_string(&report)?);
    } else {
        println!(
            "=== edge candidates — {} fact(s), {} succession edge(s), {} conflict pair(s) ===",
            report.fact_count, report.succession_edges, report.conflict_pairs
        );
        for f in &report.facts {
            let mut edges = Vec::new();
            if let Some(p) = &f.supersedes_in_frame {
                edges.push(format!("supersedes {p}"));
            }
            if !f.conflicts_with.is_empty() {
                edges.push(format!("conflicts {}", f.conflicts_with.join(",")));
            }
            if !f.pays_off.is_empty() {
                edges.push(format!("pays-off {}", f.pays_off.join(",")));
            }
            println!(
                "  {} (frame {} / branch {} @ {}{}): {}{}",
                f.fact_id,
                f.frame,
                f.branch,
                f.canon_from,
                f.canon_to
                    .as_deref()
                    .map(|t| format!("..{t}"))
                    .unwrap_or_default(),
                f.claim,
                if edges.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", edges.join("; "))
                }
            );
        }
        println!("succession gaps: {}", report.succession_gaps.len());
        for g in &report.succession_gaps {
            println!(
                "  {} <-> {} ({} / {}): no succession path connects the pair",
                g.fact_a, g.fact_b, g.predicate, g.subject
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
    // v1 prose-fact-assertion axis (structured-fact SSOT) — config-only
    // severity (no CLI override yet); validator_cfg = cfg.clone() carries it
    // into the scan, so no explicit validator_cfg assignment is needed.
    let severity_prose_fact_assertion: Option<Severity> = cfg.severity_prose_fact_assertion;
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
    let prose_fact_assertion_count = get("prose_fact_assertion");
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
            "prose_fact_assertion_count": prose_fact_assertion_count,
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
 misclassified_coverage={} blanket_verifies={} prose_fact_assertion={} decay={} \
 inv_missing={} inv_deprecated={} unconfirmed_verifies={} \
 (severity_missing={} severity_binding={} severity_coverage={} severity_verification={} \
 severity_classification={} severity_blanket={} severity_prose_fact_assertion={} \
 severity_inventory={})",
            violations.len(),
            missing_count,
            section_missing_count,
            citation_unbound_count,
            binding_unbacked_count,
            impl_missing_count,
            verification_missing_count,
            misclassified_coverage_count,
            blanket_verifies_count,
            prose_fact_assertion_count,
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
            severity_prose_fact_assertion
                .map(Severity::as_str)
                .unwrap_or("off"),
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
    if prose_fact_assertion_count > 0 && severity_prose_fact_assertion == Some(Severity::Reject) {
        reject_msgs.push(format!(
            "{} prose-fact-assertion violation(s) — ProseFactAssertion={} \
 (severity_prose_fact_assertion=reject)",
            prose_fact_assertion_count, prose_fact_assertion_count,
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

// The EPUB-file re-hash against `[workspace.spec_source].epub_sha256`
// (R405) uses THE one hash encoding, `mnemosyne_core::sha256_hex`
// (Round 460 consolidation).

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
                let computed = mnemosyne_core::sha256_hex(&bytes);
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

#[cfg(test)]
mod tests {
    use super::{parse_query_args, COMMANDS};
    use std::collections::HashMap;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    /// Every verb resolves to exactly one command. `run` dispatches by first
    /// match, so a duplicate name or alias would make the second entry
    /// silently unreachable — dead behavior that still documents itself in
    /// `--help`. The table lives here (private to the bin), so this check does
    /// too.
    #[test]
    fn no_verb_is_claimed_twice() {
        let mut owner: HashMap<&str, &str> = HashMap::new();
        for command in COMMANDS {
            for verb in std::iter::once(&command.name).chain(command.aliases) {
                if let Some(first) = owner.insert(verb, command.name) {
                    panic!(
                        "`{}` is claimed by both `{}` and `{}`; the later entry \
                         is unreachable",
                        verb, first, command.name
                    );
                }
            }
        }
    }

    // Round 470 — query modes reject loudly instead of silent dispatch
    // precedence (the Round 466 --world guard class).
    #[test]
    fn parse_query_rejects_two_modes() {
        let err = parse_query_args(&args(&["--list-sections", "--list-changelog"]))
            .expect_err("two modes must reject");
        assert!(err.to_string().contains("mutually exclusive"));
        let err = parse_query_args(&args(&["--term", "x", "39"])).expect_err("term + section");
        assert!(err.to_string().contains("mutually exclusive"));
    }

    #[test]
    fn parse_query_rejects_mode_scoped_flags_without_their_mode() {
        let err = parse_query_args(&args(&["--limit", "5"])).expect_err("limit needs list mode");
        assert!(err.to_string().contains("--list-changelog"));
        let err = parse_query_args(&args(&["--regex"])).expect_err("regex needs term");
        assert!(err.to_string().contains("--term"));
        let err =
            parse_query_args(&args(&["--include-related"])).expect_err("related needs section");
        assert!(err.to_string().contains("section_id"));
    }

    #[test]
    fn parse_query_accepts_each_mode_with_its_flags() {
        let q = parse_query_args(&args(&["--list-changelog", "--limit", "5", "--json"]))
            .expect("valid combo");
        assert!(q.list_changelog);
        assert_eq!(q.changelog_limit, Some(5));
        let q = parse_query_args(&args(&["--term", "x", "--regex", "--scope", "changelog"]))
            .expect("valid term combo");
        assert!(q.term_regex);
        let q = parse_query_args(&args(&["39", "--include-related"])).expect("valid section");
        assert_eq!(q.section_id.as_deref(), Some("39"));
        let err = parse_query_args(&args(&["--limit", "abc", "--list-changelog"]))
            .expect_err("non-numeric limit");
        assert!(err.to_string().contains("positive integer"));
    }
}
