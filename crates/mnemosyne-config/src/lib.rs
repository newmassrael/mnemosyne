//! Workspace config — `mnemosyne.toml` schema + load + discovery (//! WORKSPACE-CONFIG-ABSTRACTION, Phase 0e generic library extraction).
//!
//! Spec binding: §orphan-ledger (OrphanKind + OrphanLedgerEntry).
//!
//! Phase 0e framing reset: Mnemosyne is *LLM-driven spec infrastructure for
//! any codebase*, not a project-specific tool. The repo root + atomic-store
//! sidecar path an external user authors live in a TOML file rather than
//! hardcoded constants.
//!
//! ## Schema
//!
//! ```toml
//! [workspace]
//! root = "." # optional, default = file's dir
//!
//! [atomic]
//! sidecar_path = "docs/.atomic/workspace.atomic.json" # optional
//! ```
//!
//! ## Discovery
//!
//! `discover_config(start)` walks from `start` upward looking for
//! `mnemosyne.toml` (or `.mnemosyne/config.toml`) — same pattern as git. Returns the
//! parsed config + the directory it was found in (= workspace root).

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Cite-time gate strictness — the canonical `reject | warn | info` vocabulary
/// shared by every reject-class config knob (`severity_missing` /
/// `severity_binding` / `severity_coverage` / `severity_verification` /
/// `severity_inventory`, and the `spec_drift` / `commit_ledger` /
/// `content_drift` gates). `Reject` fails the run (exit 1); `Warn` and `Info`
/// print but pass.
///
/// Lives in `mnemosyne-config` (not `mnemosyne-core`) because severity is a
/// pure config-policy concept — never stored in the atomic store, unlike the
/// domain enums `BindingKind` / `CoverageExpectation`. Parsed ONCE: by serde
/// at config load, and at the CLI `--severity-*` boundary via
/// [`from_tag`](Self::from_tag). This replaces the stringly-typed `String` +
/// the `matches!("reject"|"warn"|"info")` checks that were scattered across
/// the config loader and the CLI. Distinct from the style-tier `StyleSeverity`
/// (`warn | info`, no `reject`). `Reject` is the default — the conservative
/// gate (matches the pre-enum `default_severity_reject`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    #[default]
    Reject,
    Warn,
    Info,
}

impl Severity {
    /// Canonical lowercase label (matches the serde representation).
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Reject => "reject",
            Severity::Warn => "warn",
            Severity::Info => "info",
        }
    }

    /// Parse the canonical lowercase tag ([`Self::as_str`]) back to a value.
    /// `None` for any other string — the single CLI-boundary validation point.
    pub fn from_tag(s: &str) -> Option<Self> {
        match s {
            "reject" => Some(Severity::Reject),
            "warn" => Some(Severity::Warn),
            "info" => Some(Severity::Info),
            _ => None,
        }
    }

    /// Does this severity fail the run (exit 1)?
    pub fn is_reject(self) -> bool {
        matches!(self, Severity::Reject)
    }
}

/// Top-level workspace config schema, mapping 1:1 to TOML tables.
///
/// `[workspace]` is required. `[schema]`, `[style]`, `[terminology]` are
/// optional — when omitted, callers fall back to preset defaults
/// (`mnemosyne_preset` for this codebase, `generic_default` for external
/// generic-markdown users).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceConfig {
    pub workspace: WorkspaceSection,
    #[serde(default)]
    pub schema: Option<SchemaSection>,
    #[serde(default)]
    pub style: Option<StyleSection>,
    #[serde(default)]
    pub terminology: Option<TerminologySection>,
    /// Per-workspace orphan ledger.
    /// OPTION D originally hardcoded ledger entries in mnemosyne-cli's
    /// `KNOWN_STALE_ORPHANS` const — fine for self-application but unusable
    /// for external workspaces that need to register their own legacy
    /// orphans without modifying mnemosyne. This config-based ledger
    /// composes (set-union) with the const ledger; bidirectional set
    /// equality semantics (new orphan / resolved entry drift catch) are
    /// preserved across both sources.
    #[serde(default, rename = "orphan_ledger")]
    pub orphan_ledger: Vec<OrphanLedgerEntry>,
    /// Round 296 — publishable / audit divergence ledger.
    ///
    /// Pairs with the R294 schema split + R295 publishable setters. When an
    /// `AtomicChangelogEntry`'s `publishable_*` half diverges from its
    /// `audit_*` half (the permanent record), validate-workspace rejects the
    /// store unless a matching `[[publishable_override_ledger]]` row
    /// authorizes it with a written `reason` and a `content_hash_after`
    /// anchor that equals the current publishable hash. This is the textbook
    /// audit-trace pattern from R254 orphan_ledger applied to the body-split
    /// axis: divergent state is allowed only when explicitly accounted for.
    #[serde(default, rename = "publishable_override_ledger")]
    pub publishable_override_ledger: Vec<PublishableOverrideLedgerEntry>,
    /// `[plugins.*]` table — plugin substrate config (RFC-003 FR-1/FR-2
    /// landed in R306). Two plugin categories live here:
    /// - `[plugins.set_equality_validator]` — the validator that drives
    ///   code citation refs (set-equality + inventory + external-prefix
    ///   axes). When omitted, the `validate-code-refs` subcommand exits 0
    ///   with a "skipped, no config" log line — 5-min setup promise carry.
    /// - `[plugins.symbol_resolver.<lang>]` — per-language symbol
    ///   resolvers used by RFC-002 FR-3 symbol-level enforcement. When a
    ///   language has no resolver configured, file-only set-equality
    ///   continues to apply for that language (no language is blocked).
    #[serde(default)]
    pub plugins: Option<PluginsSection>,
    /// Round 279 — `[atomic]` table — atomic store sidecar path override.
    ///
    /// Closes the documentation-vs-implementation gap surfaced by the TC8
    /// external dogfood: the docstring on `AtomicStore::default_sidecar_path`
    /// claimed `[atomic] sidecar_path` was configurable, but no struct field
    /// actually parsed it. External users adopting Mnemosyne next to an
    /// existing `docs/` tree can now redirect the sidecar (e.g., to
    /// `doc/.atomic/workspace.atomic.json`) to avoid directory collisions.
    #[serde(default)]
    pub atomic: Option<AtomicConfigSection>,
    /// `[spec_drift]` table — severity policy for the spec-revision
    /// drift scan (RFC-001 UC-1 "B2"). Absent → the scan still runs
    /// whenever `[workspace.spec_source]` is present, at the default
    /// `warn` severity.
    #[serde(default)]
    pub spec_drift: Option<SpecDriftSection>,
    /// `[commit_ledger]` table — severity policy for the commit↔ledger
    /// drift gate (Round 293/301; `validate-workspace`'s commit-subject
    /// round-label scan). Absent → the gate runs at the default `reject`
    /// severity (the R301 dogfood hard-reject). An external consumer
    /// workspace whose `(R<n>)` commit labels are not Mnemosyne changelog
    /// rounds downgrades to `warn`/`info` (Round 377).
    #[serde(default)]
    pub commit_ledger: Option<CommitLedgerSection>,
    /// `[content_drift]` table — severity policy for the content-integrity
    /// scan (R404; `validate-content-drift`'s offline re-hash of each
    /// `normative_excerpt.text` vs its `text_sha256`). Absent → the scan
    /// runs at the default `reject` severity (a cache diverging from its own
    /// hash is corruption, never a legitimate intermediate state).
    #[serde(default)]
    pub content_drift: Option<ContentDriftSection>,
    /// `[verifies_catalog]` table — authoritative test-catalog linkage check
    /// (R426; SCE field-report P2; the `validate-verifies-linkage` subcommand).
    /// Points at a consumer-generated catalog JSON mapping each test artifact
    /// to the section(s) its authoritative metadata declares it targets; every
    /// `verifies` binding is then validated against it deterministically.
    /// Absent → the check is disabled (opt-in).
    #[serde(default)]
    pub verifies_catalog: Option<VerifiesCatalogSection>,
    /// `[continuity]` table — frame-scoped narrative continuity gate (Round
    /// 431; `validate-continuity`). Evaluates recorded conflict edges between
    /// narrative facts: same-frame overlapping contradictions are violations,
    /// cross-frame contradictions are data. Absent → the gate is disabled
    /// (opt-in, the verify-axis pattern: a workspace with no narrative facts
    /// pays no cost).
    #[serde(default)]
    pub continuity: Option<ContinuitySection>,
}

/// `[atomic]` table — atomic store path override (Round 279).
///
/// Overrides the default sidecar (`docs/.atomic/workspace.atomic.json`)
/// path. Relative paths resolve against the workspace root; absolute paths
/// are honored as-is. The CLI `--sidecar` flag wins over this config.
///
/// Type name is `AtomicConfigSection` (not `AtomicSection`) to disambiguate
/// from `atomic::AtomicSection`, which is the typed-fields-per-§ store.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct AtomicConfigSection {
    /// Workspace-relative or absolute sidecar JSON path. `None` (or `[atomic]`
    /// omitted entirely) falls back to the default `docs/.atomic/workspace.atomic.json`.
    #[serde(default)]
    pub sidecar_path: Option<String>,
}

/// atomic-internal orphan ledger kind.
///
/// introduced `[[orphan_ledger]]` for markdown-body cross-ref
/// orphans. extends the ledger to also cover atomic-internal
/// orphans introduced by dogfood-switch ratify — namely
/// dangling refs in `ChangelogEntry.impact_refs` and `Section.impact_scope`
/// that arise when a doc/section is removed from `workspace.docs` after a
/// prior `Round N` entry has cited it. The frozen-ledger invariant blocks
/// rewriting the prior entry; the orphan ledger absorbs the dangling refs
/// without silencing them. This is the textbook scope-correction path:
/// append a new Round entry recording the scope change, then register the
/// now-dangling atomic refs here with `reason` pointing to that entry.
///
/// adds `CodeCitation` for code-side citation suppression
/// (Path B Spec ↔ Code bidirectional check). Each axis carries one
/// dedicated kind so a bulk register against `CodeCitation`
/// can land without touching the atomic-internal axes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OrphanKind {
    /// Markdown body cross-ref orphan. Existing toml
    /// rows without `kind` parse as this variant via serde default,
    /// preserving behavior.
    MarkdownRef,
    /// ChangelogEntry `impact_refs` orphan. `from` = entry_id
    /// (e.g. `""`); `to` = atomic section_id missing from id_set;
    /// `doc` = `"<atomic-changelog>"` by convention.
    AtomicEntryRef,
    /// Section `impact_scope` orphan. `from` = section_id
    /// authoring the impact_scope; `to` = atomic section_id missing from
    /// id_set; `doc` = `"<atomic-section>"` by convention.
    AtomicSectionRef,
    /// Code-side citation suppression.
    /// `from` = workspace-relative file path containing the citation;
    /// `to` = section_id without leading `§` (or `entry_id` for
    /// Round NNN-shaped suppression, deferred to bulk
    /// register); `doc` = `"<code-citation>"` by convention. Suppresses
    /// `SectionMissing` / `CitationUnbound` / `BindingUnbacked`
    /// when the (from, to) pair matches.
    CodeCitation,
    /// Round 285 — code-side inventory-citation suppression.
    /// Mirrors `CodeCitation` for the Phase 1A inventory axis. `from` =
    /// workspace-relative file path containing the cite; `to` = inventory
    /// id (e.g., `"IPv4_OPTIONS_01"`); `doc` = `"<inventory-citation>"`
    /// by convention. Suppresses `InventoryMissing` /
    /// `InventoryDeprecated` for the (from, to) pair so adopters can
    /// document intentional historical references to deleted-or-
    /// deprecated test-case ids without flipping the cite-time gate off.
    /// `reason` field is the audit-trail record of *why* the suppression
    /// is acceptable.
    InventoryCitation,
}

fn default_orphan_kind() -> OrphanKind {
    OrphanKind::MarkdownRef
}

/// One row of `[[orphan_ledger]]` in `mnemosyne.toml` — a known-stale
/// cross-ref that the workspace explicitly accepts as legacy carry.
///
/// covered markdown-body cross-refs; generalized the
/// ledger to also cover atomic-internal orphans (ChangelogEntry impact_refs
/// + Section impact_scope) via the `kind` field.
///
/// Validate-workspace requires the actual orphan set (per kind) to
/// set-equal the merged ledger (config + const). Adding an entry here
/// suppresses one orphan from "new"; removing an entry whose ref is still
/// broken surfaces it as new again. If an authored ref is later fixed,
/// validate-workspace flags the orphan as "resolved" so the stale entry
/// can be removed from the ledger.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OrphanLedgerEntry {
    /// orphan kind. Default = `MarkdownRef` for backward
    /// compatibility with toml rows.
    #[serde(default = "default_orphan_kind")]
    pub kind: OrphanKind,
    /// Doc path (workspace-relative) of the orphan's source. For
    /// `kind = AtomicEntryRef`, by convention `"<atomic-changelog>"`.
    /// For `kind = AtomicSectionRef`, `"<atomic-section>"`.
    pub doc: String,
    /// Section id (or entry_id for `AtomicEntryRef`) the orphan ref is
    /// authored from (without leading `§`).
    pub from: String,
    /// Section id the orphan ref points to (without leading `§`).
    pub to: String,
    /// Why this orphan is acceptable (target pending authoring,
    /// cross-doc placeholder, scope-correction carry, etc.). Required
    /// field — the orphan is frozen-by-rationale, not silently suppressed.
    pub reason: String,
    /// When the entry was registered (free-form date or round id).
    pub since: String,
}

/// One row of `[[publishable_override_ledger]]` in `mnemosyne.toml` — an
/// authorized divergence between the `publishable_*` half and the `audit_*`
/// half of a single `AtomicChangelogEntry` (R294 body split).
///
/// Validate-workspace gate (R296) walks `changelog_entries`; for each entry
/// where `publishable_matches_audit() == false`, requires at least one row
/// here with matching `target_id` whose `content_hash_after` equals the
/// current publishable hash. Missing or stale rows reject the workspace —
/// mirroring the [`OrphanLedgerEntry`] pattern.
///
/// `kind` is free-form (e.g. `"redaction"`, `"typo"`, `"clarification"`)
/// so workspace policy can categorize divergences without a closed-form
/// enum that would block adoption-time vocabulary expansion.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PublishableOverrideLedgerEntry {
    /// Free-form classification of the divergence. Common values:
    /// `"redaction"` (RFC P1 privacy fix), `"typo"`, `"clarification"`.
    pub kind: String,
    /// `entry_id` (changelog entry key) whose publishable / audit halves
    /// diverge — short form `Round <N>` or long form `Round <N> — title`.
    pub target_id: String,
    /// Field names that diverge (subset of: `publishable_decision_summary`,
    /// `publishable_changes_bullets`, `publishable_verification_bullets`,
    /// `publishable_impact_refs`, `publishable_carry_forward_bullets`).
    /// Currently informational — v1 gate matches at entry granularity, not
    /// per-field. Author-facing audit trace.
    #[serde(default)]
    pub fields: Vec<String>,
    /// Why the divergence is authorized (privacy fix, typo correction, etc.).
    /// Required field — frozen-by-rationale, not silently suppressed.
    pub reason: String,
    /// Round id (or commit hash) where the divergence was applied. Free-form
    /// string for cross-referencing the originating changelog entry.
    pub applied_in: String,
    /// Optional SHA256 anchor of the audit-half hash at divergence time.
    /// Informational trace; not validated (audit half is immutable so this
    /// would only ever fail if the audit invariant itself was breached).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_hash_before: Option<String>,
    /// SHA256 anchor of the publishable-half hash after divergence.
    /// Required: validate-workspace recomputes the current publishable hash
    /// per entry and rejects if no ledger row's `content_hash_after` matches.
    /// This is what makes the ledger forge-resistant — editing publishable_*
    /// without re-anchoring here re-surfaces the rejection.
    pub content_hash_after: String,
}

/// `[plugins.*]` table root — plugin substrate config (RFC-003 FR-1/FR-2
/// land in R306).
///
/// Two plugin categories live here today:
/// - `set_equality_validator` — `ValidatorClass` plugin that drives the
///   code citation refs subcommand. Owns paths + severity + comment_only
///   + inventory + external-prefix axes. Sub-axis splits (separate
///   inventory_validator / external_ref_skipper plugins) are R307+
///   refinements — set_equality_validator is the current monolithic carrier.
/// - `symbol_resolver` — `BindingClass` plugin map keyed by language ID
///   (`rust`, `python`, `go`, …). Per-language transport selection per
///   the RFC-003 transport-abstraction section: `in-process` (Rust trait impl), `mcp` (MCP client),
///   or `cli` (shell-out). Missing language falls through to file-only
///   set-equality — no language is blocked.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct PluginsSection {
    #[serde(default)]
    pub set_equality_validator: Option<SetEqualityValidatorConfig>,
    #[serde(default)]
    pub symbol_resolver: std::collections::BTreeMap<String, SymbolResolverConfig>,
}

/// Per-language symbol resolver config under
/// `[plugins.symbol_resolver.<lang>]`. Transport-tagged enum mirrors
/// `mnemosyne_core::Transport` so config parse failures surface the same
/// variant set as the runtime trait.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "transport", rename_all = "kebab-case")]
pub enum SymbolResolverConfig {
    InProcess {
        backend: String,
    },
    Mcp {
        command: Vec<String>,
    },
    Cli {
        command: Vec<String>,
        #[serde(default)]
        output_parser: Option<String>,
    },
}

/// `[plugins.set_equality_validator]` — the citation-refs validator plugin
/// config (in-place rename from the pre-R306 `[code_refs]` table; no semantic
/// change, only namespace shift onto the RFC-003 plugin substrate).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SetEqualityValidatorConfig {
    /// Workspace-relative paths to scan recursively. Each entry may be a
    /// file or directory. Hidden directories (`.git/`, `.mnemosyne/`),
    /// `target/`, and `node_modules/` are always skipped (build artifacts
    /// and vendored deps shouldn't carry author-written citations).
    #[serde(default)]
    pub paths: Vec<String>,

    /// Severity for hallucination-class violations:
    /// - `Missing` — Round NNN entry_id not in `changelog_entries`
    /// - `SectionMissing` — §<id> not in atomic section_id set
    /// Recognized values: `"reject"` (default) / `"warn"` / `"info"`.
    #[serde(default = "default_severity_reject")]
    pub severity_missing: Severity,

    /// severity for binding-class violations (Path B Spec ↔
    /// Code bidirectional set-equality):
    /// - `CitationUnbound` — code cites §X but file not in §X.bindings
    /// - `BindingUnbacked` — §X.bindings names file F but F
    /// has no §X citation
    /// - `SymbolMismatch` — a cite's resolved symbol is not in §X's
    /// registered symbol set for that file
    /// Recognized values: `"reject"` (default) / `"warn"` / `"info"`.
    #[serde(default = "default_severity_reject")]
    pub severity_binding: Severity,

    /// severity for the coverage-class violation, split out from
    /// `severity_binding`. Round 269 added `ImplementationMissing` but
    /// bucketed it under `severity_binding` (C1, YAGNI) and carried the
    /// split decision pending empirical evidence from external workspace
    /// adoption; spec-mirror adoption — where most sections are prose and
    /// legitimately uncited, so coverage enforcement is inappropriate — is
    /// that evidence:
    /// - `ImplementationMissing` — an Active section has zero implementations
    /// When unset (`None`), inherits `severity_binding` so pre-split
    /// configs and the implementation-ledger default are unchanged.
    /// Recognized values: `"reject"` / `"warn"` / `"info"`.
    #[serde(default)]
    pub severity_coverage: Option<Severity>,

    /// Severity for the verification-axis violation (`VerificationMissing`,
    /// R413): a `Normative` + `Dedicated` section with zero `verifies`
    /// bindings. UNLIKE `severity_coverage` (which inherits `severity_binding`
    /// when unset), `None` here means the verify axis is DISABLED — no
    /// `VerificationMissing` is emitted at all. The verify axis is opt-in: it
    /// is a per-project commitment to requirement→test-evidence traceability
    /// (e.g. a conformance-ledger consumer), not a universal axiom like
    /// implements-coverage, so a workspace that does not register `verifies`
    /// bindings pays no cost and sees no noise. Set to `"reject"` / `"warn"` /
    /// `"info"` to enable the gate at that strictness.
    #[serde(default)]
    pub severity_verification: Option<Severity>,

    /// Severity for the confirmation-gate violation (`ConfirmationUnconfirmed`,
    /// R419): a `Normative` + `Dedicated` section whose `verifies` binding is not
    /// yet `Confirmed` (the v1 required-evidence-set unmet, or an open refute).
    /// Like `severity_verification`, `None` means the confirmation gate is
    /// DISABLED — fully opt-in, so a workspace that does not run independent
    /// confirmation pays no cost. Layers ON TOP of the verify axis: verify checks
    /// that a test exists; confirmation checks the test was independently
    /// re-verified. Set to `"reject"` / `"warn"` / `"info"` to enable.
    #[serde(default)]
    pub severity_confirmation: Option<Severity>,

    /// Severity for the coverage-invariant violation (`MisclassifiedCoverage`,
    /// R423): an EXEMPT section (`OutOfScopeHere` | `Informational`) that carries
    /// an `implements` or `verifies` binding — design sec 6's
    /// `has-implements/verifies ⟹ Normative` rule. Either the section is
    /// mislabeled (should be Normative) or the binding is wrong. `None` = the
    /// invariant gate is OFF (opt-in, like `severity_verification`). The 3-state
    /// `coverage_expectation` enum alone cannot catch this — the enum adds the
    /// label, this gate enforces label↔binding consistency.
    #[serde(default)]
    pub severity_classification: Option<Severity>,

    /// Severity for the blanket-binding violation (`BlanketVerifies`, R425,
    /// SCE field-report P1): one test artifact (`file`, `symbol`) carrying
    /// `verifies` bindings on MORE THAN ONE section. A conformance test almost
    /// always verifies one section; N>1 is the blanket-binding smell that let
    /// 84/126 semantically-wrong bindings stay structurally green in the SCE
    /// episode. `None` = the detector is OFF (opt-in). Recommended `warn` —
    /// a genuine multi-target test is tolerable noise (no opt-out annotation
    /// in v1, YAGNI).
    #[serde(default)]
    pub severity_blanket: Option<Severity>,

    /// comment-only filtering toggle. When `true` (default),
    /// the citation extractor only sees text inside language comments
    /// (`//`, `/* */`, `#`); string-literal contents and code identifiers
    /// are stripped out, eliminating the dominant false-positive surface
    /// from test fixtures and inline string data. Unknown file extensions
    /// fall through to whole-text scan regardless of this flag.
    ///
    /// Set to `false` to restore the whole-text scan (back-compat
    /// for users whose citation discipline relies on non-comment markers).
    #[serde(default = "default_comment_only")]
    pub comment_only: bool,

    /// Round 275 — Inventory citation axis (Phase 1A).
    ///
    /// Each prefix opens an inventory ID citation match (e.g., `"ARP_"`,
    /// `"TCP_"`); the scanner walks `<prefix>[A-Z0-9_]+` tokens and looks them
    /// up in `AtomicStore.inventory_entries`. Multiple prefixes are scanned in
    /// parallel — TC8 has 8 categories, ISO/ETSI test specs typically have
    /// similar prefix families. Empty `Vec` = axis disabled (5-min setup
    /// promise carry; users without inventory cites pay no cost).
    ///
    /// Citation existence is *required* — missing ID → `InventoryMissing`.
    /// `Deprecated` status → `InventoryDeprecated`. `Active` / `Reserved`
    /// statuses pass silently. The atomic store is the cite-time SSOT;
    /// external PDF/JSON sources sync into it via the mutate API.
    #[serde(default)]
    pub inventory_prefixes: Vec<String>,

    /// Severity for inventory-axis violations (`InventoryMissing` /
    /// `InventoryDeprecated`). Recognized values: `"reject"` (default) /
    /// `"warn"` / `"info"`. Mirrors `severity_missing` / `severity_binding`
    /// — the cite-time gate's strictness is a per-project knob.
    #[serde(default = "default_severity_reject")]
    pub severity_inventory: Severity,

    /// Round 277 — External-standard section-citation prefixes (Phase 1A P1).
    ///
    /// Each entry is a single-token prefix word (no whitespace) — e.g.,
    /// `"RFC"`, `"IEEE"`, `"ISO/IEC"`. When a `§<id>` citation is preceded
    /// (on the same line) by `<prefix> <digits>(.<digits>)*` + whitespace,
    /// the citation is treated as an *external standard reference*
    /// (`RFC 2131 §3.5`, `IEEE 802.3 §2.4`, `ISO/IEC 14882 §1.5`) and
    /// skipped — neither `SectionMissing` nor `CitationUnbound` fires.
    ///
    /// Empty `Vec` = external-skip disabled (back-compat default; the
    /// existing single-prefix `§<id>` extractor is preserved verbatim).
    ///
    /// Multi-token prefixes (e.g., `"ETSI TS"`) are not v1 — only the last
    /// non-whitespace token before the numeric is consulted. Workaround for
    /// rare ETSI/3GPP citations: register the *trailing* token of the prefix
    /// (e.g., `"TS"` for `"ETSI TS 102 ..."`), accepting a slightly looser
    /// match.
    #[serde(default)]
    pub external_section_prefixes: Vec<String>,

    /// Round 284 — External-standard *doc-name* prefixes (Phase 1A P1).
    ///
    /// Separate axis for standards identified by document *short name*
    /// rather than numeric document number — AUTOSAR family
    /// (`"TR_SOMEIP"`, `"SOMEIPSD"`, `"SWS_SD"`), 3GPP / ETSI doc-name
    /// references, etc. Citation form is `<PREFIX> §<id>` (no numeric
    /// between prefix and sigil): e.g., `// TR_SOMEIP §6.7.4.2.4`.
    ///
    /// Kept distinct from `external_section_prefixes` (numeric mode) so
    /// users *explicitly opt into* the bare form per prefix — guards
    /// against generic-sounding tokens (`"AUTOSAR"`) silently skipping
    /// internal `§<id>` citations on prose lines that happen to mention
    /// the standard name. Same prefix may be registered in both axes if
    /// the standard supports both citation forms; matching tries both.
    ///
    /// Empty list = bare-prefix axis disabled. Existing
    /// `external_section_prefixes` users (R277 / R281) are unaffected —
    /// the numeric-mode key keeps its meaning.
    #[serde(default)]
    pub external_section_prefixes_bare: Vec<String>,

    /// Inventory citation prefixes with *section-path* tail shape
    /// (Phase 0 hardening, RFC-002 FR-4 narrow extension).
    ///
    /// Companion axis to `inventory_prefixes` for external-spec mirror
    /// adopters whose citation tail uses section-path characters
    /// (`A-Za-z0-9./-_`) instead of the opaque-ID shape that R275
    /// codified (`[A-Z0-9_]+ ending in digit`). Citation form:
    /// `<prefix><tail>` where `<tail>` matches `[A-Za-z0-9./-_]+` with
    /// no digit-terminus requirement.
    ///
    /// Use case: W3C SCXML / IETF RFC / IEEE / AUTOSAR mirror. An adopter
    /// registers `inventory_path_prefixes = ["W3C SCXML "]` and a W3C
    /// SCXML section like `3.13` gets registered as `InventoryEntry { id
    /// = "W3C SCXML 3.13", … }` in the atomic store. Citations of the
    /// form `// W3C SCXML 3.13` in code resolve against the inventory
    /// axis without forcing a mass cite migration to backslash-sigil form.
    ///
    /// Resolution target is the same `InventoryEntry` store as
    /// `inventory_prefixes` — they are two tail-shape axes that feed the
    /// same lifecycle (active / deprecated / reserved). `severity_inventory`
    /// applies to both. Orphan-ledger suppression via
    /// `[[orphan_ledger]] kind = "inventory_citation"` covers both.
    ///
    /// Empty list = path-shape axis disabled. Existing `inventory_prefixes`
    /// users (R275) are unaffected — the opaque-ID-shape key keeps its
    /// meaning. A prefix may be registered in both axes if the standard
    /// supports both citation forms; matching tries the path-shape axis
    /// after the opaque-ID axis (longest-prefix-first ordering within
    /// each axis is preserved).
    #[serde(default)]
    pub inventory_path_prefixes: Vec<String>,

    /// Section-ID namespace scope for this workspace's `§<id>` axis.
    ///
    /// A `§<id>` citation's namespace is the segment of `<id>` before the
    /// first `-` (or the whole id when it has no `-`). When this field is
    /// set, only citations whose namespace segment is *exactly* equal to it
    /// are validated against the atomic section-id set; citations in any
    /// other namespace are treated as out of this workspace's jurisdiction
    /// and skipped entirely (neither `SectionMissing` nor `CitationUnbound`,
    /// and no bidirectional binding record).
    ///
    /// This is what lets a single source file cite more than one external
    /// spec — `§scxml-6.4` (W3C SCXML) and `§mesh-16.7` (a different
    /// ledger) in the same comment — with each workspace gating only its
    /// own namespace. The namespace lives in the citation token itself, not
    /// in surrounding prose, so it is independent of the R277/R284
    /// preceding-word external-skip axes (which still apply on top).
    ///
    /// `None` (omitted) = no scoping: every `§<id>` is checked, exactly as
    /// before this field existed (100% back-compatible — workspaces with
    /// kebab/slash ids like `§atomic-store/changelog-…` are unaffected).
    ///
    /// Exact-segment match, not prefix: namespace `"scxml"` validates
    /// `§scxml-6.4` and skips `§scxmlfoo-1` (segment `scxmlfoo` ≠ `scxml`)
    /// and `§mesh-16.7`. An empty string is rejected at config load — an
    /// empty namespace is almost certainly an authoring error.
    #[serde(default)]
    pub section_namespace: Option<String>,
}

fn default_severity_reject() -> Severity {
    Severity::Reject
}

fn default_severity_warn() -> Severity {
    Severity::Warn
}

fn default_comment_only() -> bool {
    true
}

/// `[style]` table — locale + threshold overrides for T3/T4 style rules
///.
///
/// `locale` selects the sentence-boundary handler (Korean / Japanese /
/// Chinese / English). `thresholds` lets external users override per-rule
/// char count caps without forking the validator.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct StyleSection {
    /// Locale tag for sentence boundary recognition.
    /// Recognized values: `"ko"` (default), `"ja"`, `"zh"`, `"en"`.
    /// Unknown values fall back to `"en"`.
    #[serde(default = "default_locale")]
    pub locale: String,

    /// Per-rule char count overrides. Keys must match StyleRule rule_id
    /// (`"max_sentence_length"`, `"max_paragraph_length"`,
    /// `"max_section_body_length"`). Missing keys fall back to compile-time
    /// defaults.
    #[serde(default)]
    pub thresholds: std::collections::BTreeMap<String, u32>,
}

/// `[terminology]` table — workspace-wide glossary of canonical terms +
/// non-canonical variants the parser should warn about.
///
/// Schema: each `[terminology.glossary]` row maps a canonical form to a
/// list of non-canonical variants. The Mnemosyne preset registers
/// `Salsa`/`salsa` and `bi-temporal`/`bitemporal`; external users add
/// project-specific terms.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TerminologySection {
    /// canonical → list of variants. e.g.
    /// `{ "Salsa": ["salsa"], "bi-temporal": ["bitemporal"] }`.
    #[serde(default)]
    pub glossary: std::collections::BTreeMap<String, Vec<String>>,
}

fn default_locale() -> String {
    "ko".to_string()
}

/// `[schema]` table — markdown-to-entity mapping config.
///
/// The 4 entity types (Section / CrossRef / ChangelogEntry / FrozenList)
/// are fixed primitives; this section configures *which markdown patterns*
/// the parser maps onto them. External users override via
/// `mnemosyne.toml::[schema]`; the Mnemosyne self-application registers
/// its `design_doc` preset here as the first dogfood consumer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SchemaSection {
    /// Heading titles that mark a `ChangelogEntry` container section.
    /// Default = `["Changelog", "Changelog", "changelog"]` (Mnemosyne preset).
    /// Generic markdown users typically set `["Changelog"]`.
    #[serde(default = "default_changelog_titles")]
    pub changelog_titles: Vec<String>,

    /// string prefix that opens a ChangelogEntry top bullet.
    /// Mnemosyne preset = `"Round "`; ADR preset = `"ADR-"`; Round preset =
    /// `"Round "`; Decision preset = `"Decision "`. The parser extracts
    /// digits (with `.` separator) immediately after this prefix as the
    /// numeric portion of `entry_id`; the full entry_id includes the prefix
    /// (e.g., `""`, `"ADR-0042"`).
    #[serde(default = "default_entry_id_prefix")]
    pub entry_id_prefix: String,

    /// anchor convention placeholder. The Mnemosyne preset is
    /// `"section_number"` (legacy `§N` literal). External users can label
    /// their convention here for diagnostics; deeper anchor-pattern wiring
    /// (heading anchor / ADR-NNNN / custom regex parser) is a +
    /// concern and the parser still derives section_id by the legacy rules.
    #[serde(default = "default_anchor_convention")]
    pub anchor_convention: String,

    /// Diagnostic label for this schema (e.g. `"design_doc"`, `"generic"`,
    /// `"adr"`). Carried through MutateReceipt + tracing spans for
    /// Cross-medium debugging. No semantic effect on parsing.
    #[serde(default = "default_medium_name")]
    pub medium_name: String,
}

fn default_changelog_titles() -> Vec<String> {
    vec![
        "Changelog".to_string(),
        "Changelog".to_string(),
        "changelog".to_string(),
    ]
}

fn default_entry_id_prefix() -> String {
    "Round ".to_string()
}

fn default_anchor_convention() -> String {
    "section_number".to_string()
}

fn default_medium_name() -> String {
    "design_doc".to_string()
}

impl SchemaSection {
    /// Mnemosyne self-application preset — design_doc medium with the
    /// existing Changelog / Changelog title set.
    pub fn mnemosyne_preset() -> Self {
        Self {
            changelog_titles: default_changelog_titles(),
            entry_id_prefix: default_entry_id_prefix(),
            anchor_convention: default_anchor_convention(),
            medium_name: "design_doc".to_string(),
        }
    }

    /// Generic markdown preset — only "Changelog" (case-insensitive)
    /// recognized; medium_name = `"generic"`. Use this for an external
    /// project that does not author its own `[schema]` block.
    pub fn generic_default() -> Self {
        Self {
            changelog_titles: vec!["Changelog".to_string(), "changelog".to_string()],
            // Generic markdown rarely numbers changelog entries; an empty
            // prefix means the parser disables numeric entry_id capture.
            entry_id_prefix: String::new(),
            anchor_convention: "heading_slug".to_string(),
            medium_name: "generic".to_string(),
        }
    }

    /// ADR-style preset (anchor = `ADR-NNNN`, entries = `ADR-`).
    /// Useful as a sample for external users authoring an `mnemosyne.toml`
    /// against an Architectural Decision Records project.
    pub fn adr_preset() -> Self {
        Self {
            changelog_titles: vec!["Decisions".to_string()],
            entry_id_prefix: "ADR-".to_string(),
            anchor_convention: "adr_id".to_string(),
            medium_name: "adr".to_string(),
        }
    }

    /// Case-sensitive title match against the configured changelog title
    /// set. Matches the parser's existing `is_changelog_title` semantics
    /// for the Mnemosyne preset.
    pub fn is_changelog_title(&self, title: &str) -> bool {
        self.changelog_titles.iter().any(|c| c == title) || title.eq_ignore_ascii_case("changelog")
    }
}

impl Default for SchemaSection {
    fn default() -> Self {
        Self::mnemosyne_preset()
    }
}

/// `[workspace]` table — optional root override (relative paths resolve
/// against the config file's dir unless `root` is set) + external-spec
/// mirror provenance.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceSection {
    /// Workspace root override — relative paths resolve against this when
    /// set, otherwise against the config file's parent dir.
    #[serde(default)]
    pub root: Option<String>,

    /// External-spec mirror provenance (RFC-002 FR-2). Present when this
    /// workspace is vendored against a specific upstream standard
    /// revision (W3C / IETF RFC / IEEE / AUTOSAR / etc.). Per-Section
    /// `normative_excerpt.source_revision` carries the rev that was
    /// current when each Section was anchored; this workspace-level
    /// field carries the *current* rev the workspace is tracking, so
    /// drift detection tooling can diff per-Section rev against the
    /// workspace rev to surface partially-migrated Sections.
    ///
    /// Single `spec_source` per workspace by design — a workspace that
    /// mirrors multiple standards uses one workspace tree per standard
    /// (multi-`mnemosyne.toml` shape, see SCHEMA_GUIDE.md
    /// "External-spec mirror" pattern). RFC-002 FR-5 reject covers the
    /// "bundle multiple namespaces in one workspace" anti-pattern.
    #[serde(default)]
    pub spec_source: Option<SpecSource>,
}

/// External-spec provenance metadata — anchors a workspace to a
/// specific upstream standard + revision (RFC-002 FR-2).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SpecSource {
    /// Canonical URL of the upstream standard (e.g.
    /// `"https://www.w3.org/TR/scxml/"`).
    pub url: String,
    /// Revision identifier the workspace currently tracks. Free-form
    /// (Recommendation publication date, editor's-draft date, RFC
    /// number + revision letter, etc.).
    pub revision: String,
    /// SHA-256 hex of the upstream content as fetched (lowercase, no
    /// `0x` prefix, 64 chars). Provenance anchor for drift detection
    /// — when the upstream rev label is identical but bytes diverge,
    /// the hash mismatch surfaces it.
    #[serde(default)]
    pub fetched_sha256: Option<String>,
    /// ISO-8601 timestamp at which `fetched_sha256` was captured.
    #[serde(default)]
    pub fetched_at: Option<String>,
    /// Workspace-relative POSIX path to the committed, revision-pinned EPUB
    /// (e.g. `docs/.atomic/epub/scxml-REC-20150901.epub`) — the content SSOT
    /// the `normative_excerpt` caches are projected from (R405). Paired with
    /// [`Self::epub_sha256`]: both set, or neither.
    #[serde(default)]
    pub epub_path: Option<String>,
    /// SHA-256 hex (lowercase, 64 chars) of the committed EPUB at
    /// [`Self::epub_path`]. `validate-content-drift` re-hashes the file and
    /// flags a mismatch — the EPUB was swapped/updated and the cached
    /// excerpts must be re-projected (the Layer B trigger). Provenance anchor
    /// for the EPUB-file itself, distinct from per-excerpt `text_sha256`.
    #[serde(default)]
    pub epub_sha256: Option<String>,
}

/// `[spec_drift]` table — policy for the spec-revision drift scan
/// (RFC-001 UC-1 "B2"). Governs the `validate-spec-drift` subcommand,
/// which flags `Active` Sections whose `normative_excerpt.source_revision`
/// trails the workspace `[workspace.spec_source].revision`.
///
/// Drift severity is its own axis, configurable like the code-ref axes
/// (`set_equality_validator.severity_*`). It defaults to `warn` rather
/// than `reject` because partial migration — old-rev `Superseded` +
/// new-rev `Active` Sections coexisting during a rev bump — is a
/// legitimate intermediate state; the consumer escalates to `reject`
/// (CI gate) once migration is meant to be complete.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SpecDriftSection {
    /// `reject` | `warn` | `info`. Default `warn`. Validated at config
    /// load. The `validate-spec-drift --severity` flag overrides it per
    /// run.
    #[serde(default = "default_severity_warn")]
    pub severity: Severity,
}

impl Default for SpecDriftSection {
    fn default() -> Self {
        Self {
            severity: default_severity_warn(),
        }
    }
}

/// `[commit_ledger]` table — policy for the commit↔ledger drift gate
/// (Round 293/301; the commit-subject round-label scan in
/// `validate-workspace`).
///
/// Mirrors [`SpecDriftSection`] but defaults to `reject` rather than
/// `warn`: the gate is a Mnemosyne self-development invariant — every
/// commit citing a changelog round must have a backfilled atomic-store
/// entry (Round 293 trigger, Round 301 hard-reject) — so the dogfood
/// keeps the hard reject. A multi-workspace consumer whose `(R<n>)`
/// commit labels mean something other than a Mnemosyne changelog round
/// (e.g. an adoption-round counter) downgrades to `warn`/`info`; the
/// drift line still prints, it just stops gating the exit code
/// (Round 377).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommitLedgerSection {
    /// `reject` | `warn` | `info`. Default `reject`. Validated at config
    /// load.
    #[serde(default = "default_severity_reject")]
    pub severity: Severity,
}

impl Default for CommitLedgerSection {
    fn default() -> Self {
        Self {
            severity: default_severity_reject(),
        }
    }
}

/// `[content_drift]` table — policy for the content-integrity scan (R404;
/// the `validate-content-drift` subcommand). Re-hashes each
/// `normative_excerpt.text` against its declared `text_sha256` offline and
/// flags any populated hash that no longer matches.
///
/// Mirrors [`SpecDriftSection`] but defaults to `reject` rather than `warn`:
/// `spec_drift` tolerates a rev-label trailing during partial migration (a
/// legitimate intermediate state), whereas a cache whose text no longer
/// matches its own hash was edited out-of-band — corruption, never expected.
/// The `validate-content-drift --severity` flag overrides it per run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContentDriftSection {
    /// `reject` | `warn` | `info`. Default `reject`. Validated at config load.
    #[serde(default = "default_severity_reject")]
    pub severity: Severity,
}

impl Default for ContentDriftSection {
    fn default() -> Self {
        Self {
            severity: default_severity_reject(),
        }
    }
}

/// `[verifies_catalog]` table — policy + location for the authoritative
/// test-catalog linkage check (R426; SCE field-report P2; the
/// `validate-verifies-linkage` subcommand).
///
/// The catalog itself is CONSUMER-GENERATED (e.g. parsed from the W3C
/// `metadata.txt` `specnum` field) — Mnemosyne takes only this neutral
/// contract, never format-specific parsers (sec 2.6: verification is the
/// consumer's; precedent: medium-forge). Defaults to `reject` like
/// `[content_drift]`: a `verifies` binding that contradicts the test's own
/// declared target is a wrong claim, never a legitimate intermediate state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerifiesCatalogSection {
    /// Workspace-relative path to the catalog JSON
    /// (`verifies-catalog/v1`: `{ "entries": [ { "file", "symbol"?, "section_ids" } ] }`).
    pub path: String,
    /// `reject` | `warn` | `info`. Default `reject`. Validated at config load.
    #[serde(default = "default_severity_reject")]
    pub severity: Severity,
    /// Optional sha256 pin of the catalog file (R428; `epub_sha256` symmetry).
    /// When set, every catalog load re-hashes the file and fails LOUDLY on
    /// mismatch — tamper/drift evidence at the Mnemosyne layer. The catalog is
    /// the AUTHORITY input of the R427 catalog-live confirmed branch; with the
    /// pin, it is the last gate input without a hash guard no longer.
    /// Re-pin on every legitimate catalog change (same flow as `epub_sha256`).
    #[serde(default)]
    pub sha256: Option<String>,
}

/// `[continuity]` table — policy + canon-order declaration for the
/// frame-scoped continuity gate (Round 431; the `validate-continuity`
/// subcommand).
///
/// The canon order is DECLARED, never inferred (design sec 7.9 guardrail
/// B-1): `canon_order_path` points at a consumer/medium-adapter-generated
/// `canon-order/v1` JSON (a partial-order edge list — a chapter chain for a
/// linear novel, a quest DAG for a game). Without a declaration the gate
/// still catches equal-coordinate contradictions (equality needs no order);
/// non-comparable pairs are surfaced as a count, never gated. Defaults to
/// `reject` like `[content_drift]`: a same-frame simultaneous contradiction
/// is wrong data, never a legitimate intermediate state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContinuitySection {
    /// Workspace-relative path to the `canon-order/v1` declaration. Optional:
    /// absent = equality-only comparability.
    #[serde(default)]
    pub canon_order_path: Option<String>,
    /// `reject` | `warn` | `info`. Default `reject`. Validated at config load.
    #[serde(default = "default_severity_reject")]
    pub severity: Severity,
    /// Optional sha256 pin of the canon-order file (R428 symmetry: the order
    /// is a gate-authority input; a configured pin re-hashes every load and
    /// fails LOUDLY on mismatch). Requires `canon_order_path`.
    #[serde(default)]
    pub canon_order_sha256: Option<String>,
}

/// Config discovery + load result.
#[derive(Debug, Clone)]
pub struct LoadedConfig {
    pub config: WorkspaceConfig,
    /// Absolute path to the directory all `docs[].path` resolve against.
    pub workspace_root: PathBuf,
    /// Absolute path to the config file itself (for diagnostics).
    pub config_path: PathBuf,
}

/// Parse a TOML byte slice into a config struct + validate.
pub fn parse_config(content: &str) -> Result<WorkspaceConfig> {
    let cfg: WorkspaceConfig = toml::from_str(content).context("mnemosyne.toml parse failed")?;
    validate(&cfg)?;
    Ok(cfg)
}

/// A 64-char lowercase hex SHA-256 string. Shared by the `fetched_sha256` and
/// `epub_sha256` config-load checks (R405).
fn is_lowercase_sha256_hex(s: &str) -> bool {
    s.len() == 64
        && s.chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

fn validate(cfg: &WorkspaceConfig) -> Result<()> {
    if let Some(spec) = &cfg.workspace.spec_source {
        let is_url = spec.url.starts_with("https://") || spec.url.starts_with("http://");
        if !is_url {
            bail!(
  "mnemosyne.toml: `workspace.spec_source.url = {:?}` must be an absolute http(s):// URL",
  spec.url
 );
        }
        if spec.revision.trim().is_empty() {
            bail!("mnemosyne.toml: `workspace.spec_source.revision` must be non-empty");
        }
        if let Some(hash) = &spec.fetched_sha256 {
            if !is_lowercase_sha256_hex(hash) {
                bail!(
  "mnemosyne.toml: `workspace.spec_source.fetched_sha256` must be 64-char lowercase hex (got `{}`)",
  hash
  );
            }
        }
        if let Some(hash) = &spec.epub_sha256 {
            if !is_lowercase_sha256_hex(hash) {
                bail!(
  "mnemosyne.toml: `workspace.spec_source.epub_sha256` must be 64-char lowercase hex (got `{}`)",
  hash
  );
            }
        }
        // epub_path + epub_sha256 are a pair: a path without a hash cannot be
        // checked; a hash without a path has nothing to check (R405).
        if spec.epub_path.is_some() != spec.epub_sha256.is_some() {
            bail!(
                "mnemosyne.toml: `workspace.spec_source.epub_path` and `epub_sha256` must be set together (or neither)"
            );
        }
    }
    if let Some(cat) = &cfg.verifies_catalog {
        if let Some(hash) = &cat.sha256 {
            if !is_lowercase_sha256_hex(hash) {
                bail!(
  "mnemosyne.toml: `verifies_catalog.sha256` must be 64-char lowercase hex (got `{}`)",
  hash
  );
            }
        }
    }
    if let Some(cont) = &cfg.continuity {
        if let Some(hash) = &cont.canon_order_sha256 {
            if cont.canon_order_path.is_none() {
                bail!(
                    "mnemosyne.toml: `continuity.canon_order_sha256` requires `canon_order_path` (a pin with nothing to pin)"
                );
            }
            if !is_lowercase_sha256_hex(hash) {
                bail!(
  "mnemosyne.toml: `continuity.canon_order_sha256` must be 64-char lowercase hex (got `{}`)",
  hash
  );
            }
        }
    }
    // The `spec_drift` / `commit_ledger` / `content_drift` severities are now
    // the `Severity` enum: serde rejects any value outside `reject|warn|info`
    // at deserialization (the single validation point), so the former manual
    // `matches!` checks here are gone.
    if let Some(sev) = cfg
        .plugins
        .as_ref()
        .and_then(|p| p.set_equality_validator.as_ref())
    {
        if let Some(ns) = &sev.section_namespace {
            // An empty namespace is almost certainly an authoring error —
            // fail fast rather than silently scoping every citation out
            // (the `fetched_sha256` load-time strictness precedent).
            if ns.trim().is_empty() {
                bail!(
  "mnemosyne.toml: `plugins.set_equality_validator.section_namespace` must be non-empty when set"
 );
            }
        }
    }
    Ok(())
}

/// Load a config from a known TOML file path. Resolves `workspace_root` from
/// the explicit `[workspace] root` field if set, else from the config file's
/// parent dir.
pub fn load_config(config_path: &Path) -> Result<LoadedConfig> {
    let content = std::fs::read_to_string(config_path)
        .with_context(|| format!("read {}", config_path.display()))?;
    let config = parse_config(&content)?;

    let config_dir = config_path
        .parent()
        .ok_or_else(|| anyhow!("config path {} has no parent", config_path.display()))?
        .to_path_buf();

    let workspace_root = match &config.workspace.root {
        Some(r) => {
            let candidate = config_dir.join(r);
            candidate
                .canonicalize()
                .unwrap_or_else(|_| candidate.clone())
        }
        None => config_dir,
    };

    Ok(LoadedConfig {
        config,
        workspace_root,
        config_path: config_path.to_path_buf(),
    })
}

const PRIMARY_FILENAME: &str = "mnemosyne.toml";
const FALLBACK_FILENAME: &str = ".mnemosyne/config.toml";

/// Walk upward from `start` looking for `mnemosyne.toml` then
/// `.mnemosyne/config.toml`. Returns the first match (load + validate) or
/// `None` if the entire ancestor chain has no config file.
pub fn discover_config(start: &Path) -> Result<Option<LoadedConfig>> {
    let mut cursor = if start.is_absolute() {
        start.to_path_buf()
    } else {
        std::env::current_dir().context("CWD lookup")?.join(start)
    };

    loop {
        for candidate_name in [PRIMARY_FILENAME, FALLBACK_FILENAME] {
            let candidate = cursor.join(candidate_name);
            if candidate.is_file() {
                return Ok(Some(load_config(&candidate)?));
            }
        }
        match cursor.parent() {
            Some(parent) => cursor = parent.to_path_buf(),
            None => return Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn parse_minimal_config() {
        let content = r#"
[workspace]
"#;
        let cfg = parse_config(content).unwrap();
        assert!(cfg.workspace.root.is_none());
    }

    #[test]
    fn parse_full_config() {
        let content = r#"
[workspace]
root = "."
"#;
        let cfg = parse_config(content).unwrap();
        assert_eq!(cfg.workspace.root.as_deref(), Some("."));
    }

    #[test]
    fn parse_spec_source_minimal() {
        let content = r#"
[workspace]
docs = ["docs/spec/scxml.md"]

[workspace.spec_source]
url = "https://www.w3.org/TR/scxml/"
revision = "2015-09-01"
"#;
        let cfg = parse_config(content).unwrap();
        let spec = cfg.workspace.spec_source.expect("spec_source missing");
        assert_eq!(spec.url, "https://www.w3.org/TR/scxml/");
        assert_eq!(spec.revision, "2015-09-01");
        assert!(spec.fetched_sha256.is_none());
        assert!(spec.fetched_at.is_none());
    }

    #[test]
    fn parse_spec_source_full() {
        let content = r#"
[workspace]
docs = ["docs/spec/scxml.md"]

[workspace.spec_source]
url = "https://www.w3.org/TR/scxml/"
revision = "2015-09-01"
fetched_sha256 = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"
fetched_at = "2026-05-27T00:00:00Z"
"#;
        let cfg = parse_config(content).unwrap();
        let spec = cfg.workspace.spec_source.expect("spec_source missing");
        assert_eq!(
            spec.fetched_sha256.as_deref(),
            Some("abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789")
        );
        assert_eq!(spec.fetched_at.as_deref(), Some("2026-05-27T00:00:00Z"));
    }

    #[test]
    fn spec_source_rejects_non_http_url() {
        let content = r#"
[workspace]
docs = ["docs/spec.md"]

[workspace.spec_source]
url = "ftp://example.com/spec"
revision = "2026-01"
"#;
        let err = parse_config(content).unwrap_err();
        assert!(
            err.to_string().contains("absolute http(s):// URL"),
            "expected URL-validation error, got: {}",
            err
        );
    }

    #[test]
    fn spec_source_rejects_blank_revision() {
        let content = r#"
[workspace]
docs = ["docs/spec.md"]

[workspace.spec_source]
url = "https://example.com/spec"
revision = " "
"#;
        let err = parse_config(content).unwrap_err();
        assert!(
            err.to_string().contains("revision"),
            "expected revision-validation error, got: {}",
            err
        );
    }

    #[test]
    fn spec_source_rejects_malformed_sha() {
        let content = r#"
[workspace]
docs = ["docs/spec.md"]

[workspace.spec_source]
url = "https://example.com/spec"
revision = "2026-01"
fetched_sha256 = "ABC123"
"#;
        let err = parse_config(content).unwrap_err();
        assert!(
            err.to_string().contains("fetched_sha256"),
            "expected sha-validation error, got: {}",
            err
        );
    }

    #[test]
    fn spec_source_epub_provenance_accepts_paired() {
        let content = r#"
[workspace]
docs = ["docs/spec.md"]

[workspace.spec_source]
url = "https://example.com/spec"
revision = "2026-01"
epub_path = "docs/.atomic/epub/spec.epub"
epub_sha256 = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"
"#;
        let spec = parse_config(content)
            .unwrap()
            .workspace
            .spec_source
            .unwrap();
        assert_eq!(
            spec.epub_path.as_deref(),
            Some("docs/.atomic/epub/spec.epub")
        );
        assert!(spec.epub_sha256.is_some());
    }

    #[test]
    fn spec_source_epub_rejects_malformed_sha() {
        let content = r#"
[workspace]
docs = ["docs/spec.md"]

[workspace.spec_source]
url = "https://example.com/spec"
revision = "2026-01"
epub_path = "docs/.atomic/epub/spec.epub"
epub_sha256 = "ABC123"
"#;
        let err = parse_config(content).unwrap_err();
        assert!(
            err.to_string().contains("epub_sha256"),
            "expected epub_sha256 validation error, got: {}",
            err
        );
    }

    #[test]
    fn spec_source_epub_rejects_unpaired() {
        // path without hash → reject (cannot be checked).
        let path_only = r#"
[workspace]
docs = ["docs/spec.md"]

[workspace.spec_source]
url = "https://example.com/spec"
revision = "2026-01"
epub_path = "docs/.atomic/epub/spec.epub"
"#;
        let err = parse_config(path_only).unwrap_err();
        assert!(
            err.to_string().contains("set together"),
            "expected pairing error, got: {}",
            err
        );
        // hash without path → also reject (nothing to check).
        let hash_only = r#"
[workspace]
docs = ["docs/spec.md"]

[workspace.spec_source]
url = "https://example.com/spec"
revision = "2026-01"
epub_sha256 = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789"
"#;
        assert!(parse_config(hash_only)
            .unwrap_err()
            .to_string()
            .contains("set together"));
    }

    #[test]
    fn spec_drift_severity_defaults_to_warn() {
        // [spec_drift] absent → None; present with no severity → warn.
        let content = r#"
[workspace]
docs = ["docs/spec.md"]

[spec_drift]
"#;
        let cfg = parse_config(content).unwrap();
        assert_eq!(cfg.spec_drift.unwrap().severity.as_str(), "warn");
    }

    #[test]
    fn spec_drift_rejects_invalid_severity() {
        let content = r#"
[workspace]
docs = ["docs/spec.md"]

[spec_drift]
severity = "block"
"#;
        let err = parse_config(content).unwrap_err();
        let chain = format!("{err:#}");
        assert!(
            chain.contains("unknown variant") && chain.contains("block"),
            "serde must reject the invalid severity value, got: {chain}"
        );
    }

    #[test]
    fn commit_ledger_severity_defaults_to_reject() {
        // [commit_ledger] absent → None; present with no severity → reject
        // (preserves the R301 dogfood hard-reject when the table is omitted
        // or present-but-bare).
        let absent = r#"
[workspace]
docs = ["docs/spec.md"]
"#;
        assert!(parse_config(absent).unwrap().commit_ledger.is_none());

        let bare = r#"
[workspace]
docs = ["docs/spec.md"]

[commit_ledger]
"#;
        let cfg = parse_config(bare).unwrap();
        assert_eq!(cfg.commit_ledger.unwrap().severity.as_str(), "reject");
    }

    #[test]
    fn commit_ledger_accepts_warn_opt_out() {
        // A consumer workspace downgrades the gate to warn.
        let content = r#"
[workspace]
docs = ["docs/spec.md"]

[commit_ledger]
severity = "warn"
"#;
        let cfg = parse_config(content).unwrap();
        assert_eq!(cfg.commit_ledger.unwrap().severity.as_str(), "warn");
    }

    #[test]
    fn commit_ledger_rejects_invalid_severity() {
        let content = r#"
[workspace]
docs = ["docs/spec.md"]

[commit_ledger]
severity = "block"
"#;
        let err = parse_config(content).unwrap_err();
        let chain = format!("{err:#}");
        assert!(
            chain.contains("unknown variant") && chain.contains("block"),
            "serde must reject the invalid severity value, got: {chain}"
        );
    }

    #[test]
    fn verifies_catalog_sha256_rejects_non_hex() {
        // R428 — a malformed pin is a config error at load, not a silent skip.
        let content = r#"
[workspace]
docs = ["docs/spec.md"]

[verifies_catalog]
path = "verifies-catalog.json"
sha256 = "not-a-hash"
"#;
        let err = parse_config(content).unwrap_err();
        assert!(
            format!("{err:#}").contains("64-char lowercase hex"),
            "got: {err:#}"
        );
    }

    #[test]
    fn content_drift_severity_defaults_to_reject() {
        // [content_drift] absent → None; present with no severity → reject
        // (a cache diverging from its hash is corruption, gated by default).
        let absent = r#"
[workspace]
docs = ["docs/spec.md"]
"#;
        assert!(parse_config(absent).unwrap().content_drift.is_none());

        let bare = r#"
[workspace]
docs = ["docs/spec.md"]

[content_drift]
"#;
        let cfg = parse_config(bare).unwrap();
        assert_eq!(cfg.content_drift.unwrap().severity.as_str(), "reject");
    }

    #[test]
    fn content_drift_rejects_invalid_severity() {
        let content = r#"
[workspace]
docs = ["docs/spec.md"]

[content_drift]
severity = "block"
"#;
        let err = parse_config(content).unwrap_err();
        let chain = format!("{err:#}");
        assert!(
            chain.contains("unknown variant") && chain.contains("block"),
            "serde must reject the invalid severity value, got: {chain}"
        );
    }

    #[test]
    fn parse_atomic_sidecar_path() {
        // Round 279 Bug #2 regression — [atomic] sidecar_path must
        // actually parse into the config struct (previously documented
        // but silently ignored by serde).
        let content = r#"
[workspace]
docs = ["docs/GENERATED.md"]
default_doc = "docs/GENERATED.md"

[atomic]
sidecar_path = "doc/.atomic/workspace.atomic.json"
"#;
        let cfg = parse_config(content).unwrap();
        let atomic_cfg = cfg.atomic.expect("[atomic] table missing");
        assert_eq!(
            atomic_cfg.sidecar_path.as_deref(),
            Some("doc/.atomic/workspace.atomic.json")
        );
    }

    #[test]
    fn atomic_section_optional_when_absent() {
        // Back-compat: omitting [atomic] entirely is fine — the field stays
        // None and the default sidecar path applies.
        let content = r#"
[workspace]
docs = ["docs/GENERATED.md"]
default_doc = "docs/GENERATED.md"
"#;
        let cfg = parse_config(content).unwrap();
        assert!(cfg.atomic.is_none());
    }

    #[test]
    fn discover_walks_upward() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let nested = root.join("a/b/c");
        fs::create_dir_all(&nested).unwrap();
        fs::write(root.join("mnemosyne.toml"), "[workspace]\n").unwrap();

        let loaded = discover_config(&nested).unwrap().expect("config found");
        // Workspace root resolves to the config file's dir.
        assert_eq!(
            loaded.workspace_root.canonicalize().unwrap(),
            root.canonicalize().unwrap()
        );
    }

    #[test]
    fn discover_missing_returns_none() {
        let tmp = TempDir::new().unwrap();
        let result = discover_config(tmp.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn discover_prefers_primary_over_fallback() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join(".mnemosyne")).unwrap();
        fs::write(
            tmp.path().join(".mnemosyne/config.toml"),
            "[workspace]\nroot = \"fallback\"\n",
        )
        .unwrap();
        fs::write(
            tmp.path().join("mnemosyne.toml"),
            "[workspace]\nroot = \"primary\"\n",
        )
        .unwrap();

        let loaded = discover_config(tmp.path()).unwrap().unwrap();
        assert_eq!(loaded.config.workspace.root.as_deref(), Some("primary"));
    }

    #[test]
    fn schema_section_parses_when_present() {
        let content = r#"
[workspace]
docs = ["a.md"]

[schema]
changelog_titles = ["Changelog", "Changelog"]
medium_name = "design_doc"
"#;
        let cfg = parse_config(content).unwrap();
        let schema = cfg.schema.expect("schema present");
        assert_eq!(schema.changelog_titles, vec!["Changelog", "Changelog"]);
        assert_eq!(schema.medium_name, "design_doc");
    }

    #[test]
    fn schema_section_omitted_yields_none() {
        let content = "[workspace]\ndocs = [\"a.md\"]\n";
        let cfg = parse_config(content).unwrap();
        assert!(cfg.schema.is_none(), "schema must default to None");
    }

    #[test]
    fn schema_presets_carry_expected_titles() {
        let mnemo = SchemaSection::mnemosyne_preset();
        assert!(mnemo.is_changelog_title("Changelog"));
        assert!(mnemo.is_changelog_title("changelog"));

        let generic = SchemaSection::generic_default();
        assert!(generic.is_changelog_title("Changelog"));
        assert!(generic.is_changelog_title("CHANGELOG"));
    }

    // per-workspace orphan ledger config table (composes with
    // the compile-time KNOWN_STALE_ORPHANS const in mnemosyne-cli). External
    // workspaces author their legacy orphans here instead of patching the
    // const.
    #[test]
    fn orphan_ledger_omitted_yields_empty_vec() {
        let content = "[workspace]\ndocs = [\"a.md\"]\n";
        let cfg = parse_config(content).unwrap();
        assert!(cfg.orphan_ledger.is_empty());
    }

    #[test]
    fn orphan_ledger_array_of_tables_parses() {
        let content = r#"
[workspace]
docs = ["ARCHITECTURE.md"]

[[orphan_ledger]]
doc = "ARCHITECTURE.md"
from = "11/11.5"
to = "6.2.6"
reason = "Cross-doc to RFC §6.2.6, target pending authoring"
since = "2026-05-08"

[[orphan_ledger]]
doc = "ARCHITECTURE.md"
from = "13"
to = "6.2.6"
reason = "Same target as 11/11.5 entry"
since = "2026-05-08"
"#;
        let cfg = parse_config(content).unwrap();
        assert_eq!(cfg.orphan_ledger.len(), 2);
        let first = &cfg.orphan_ledger[0];
        assert_eq!(first.doc, "ARCHITECTURE.md");
        assert_eq!(first.from, "11/11.5");
        assert_eq!(first.to, "6.2.6");
        assert!(first.reason.contains("Cross-doc"));
        assert_eq!(first.since, "2026-05-08");
        // kind defaults to MarkdownRef when omitted ( // backward compatibility).
        assert_eq!(first.kind, OrphanKind::MarkdownRef);
    }

    // atomic-internal orphan ledger kind variants.
    #[test]
    fn orphan_ledger_kind_atomic_entry_ref_parses() {
        let content = r#"
[workspace]
docs = ["a.md"]

[[orphan_ledger]]
kind = "atomic_entry_ref"
doc = "<atomic-changelog>"
from = "Round 1"
to = "missing-section"
reason = "Round 7 scope correction; doc removed from workspace.docs"
since = "Round 7"
"#;
        let cfg = parse_config(content).unwrap();
        assert_eq!(cfg.orphan_ledger.len(), 1);
        let entry = &cfg.orphan_ledger[0];
        assert_eq!(entry.kind, OrphanKind::AtomicEntryRef);
        assert_eq!(entry.doc, "<atomic-changelog>");
        assert_eq!(entry.from, "Round 1");
        assert_eq!(entry.to, "missing-section");
    }

    #[test]
    fn orphan_ledger_kind_atomic_section_ref_parses() {
        let content = r#"
[workspace]
docs = ["a.md"]

[[orphan_ledger]]
kind = "atomic_section_ref"
doc = "<atomic-section>"
from = "some-section"
to = "missing-target"
reason = "scope correction carry"
since = "Round 7"
"#;
        let cfg = parse_config(content).unwrap();
        assert_eq!(cfg.orphan_ledger.len(), 1);
        assert_eq!(cfg.orphan_ledger[0].kind, OrphanKind::AtomicSectionRef);
    }

    #[test]
    fn orphan_ledger_mixed_kinds_parses() {
        let content = r#"
[workspace]
docs = ["a.md"]

[[orphan_ledger]]
doc = "a.md"
from = "1"
to = "2"
reason = "markdown carry"
since = "Round 5"

[[orphan_ledger]]
kind = "atomic_entry_ref"
doc = "<atomic-changelog>"
from = "Round 1"
to = "removed-section"
reason = "scope-correction carry"
since = "Round 7"
"#;
        let cfg = parse_config(content).unwrap();
        assert_eq!(cfg.orphan_ledger.len(), 2);
        assert_eq!(cfg.orphan_ledger[0].kind, OrphanKind::MarkdownRef);
        assert_eq!(cfg.orphan_ledger[1].kind, OrphanKind::AtomicEntryRef);
    }

    #[test]
    fn orphan_ledger_kind_unknown_variant_rejected() {
        let content = r#"
[workspace]
docs = ["a.md"]

[[orphan_ledger]]
kind = "bogus_kind"
doc = "a.md"
from = "1"
to = "2"
reason = "test"
since = "Round 5"
"#;
        let err = parse_config(content).unwrap_err();
        let chain = format!("{:#}", err);
        assert!(
            chain.contains("kind") || chain.contains("variant"),
            "unknown-kind error should mention the field/variant; full chain: {}",
            chain
        );
    }

    #[test]
    fn orphan_ledger_missing_required_field_rejected() {
        // `reason` is required — silent suppression is not allowed. The
        // anyhow context wraps the serde error, so check the full chain.
        let content = r#"
[workspace]
docs = ["a.md"]

[[orphan_ledger]]
doc = "a.md"
from = "1"
to = "2"
since = "2026-05-08"
"#;
        let err = parse_config(content).unwrap_err();
        let chain = format!("{:#}", err);
        assert!(
            chain.contains("reason"),
            "missing-reason error should mention the field; full chain: {}",
            chain
        );
    }

    #[test]
    fn root_override_resolves_relative() {
        let tmp = TempDir::new().unwrap();
        let nested = tmp.path().join("subdir");
        fs::create_dir_all(&nested).unwrap();
        fs::write(
            nested.join("mnemosyne.toml"),
            "[workspace]\ndocs = [\"a.md\"]\nroot = \"..\"\n",
        )
        .unwrap();

        let loaded = load_config(&nested.join("mnemosyne.toml")).unwrap();
        assert_eq!(
            loaded.workspace_root.canonicalize().unwrap(),
            tmp.path().canonicalize().unwrap()
        );
    }

    #[test]
    fn set_equality_validator_empty_namespace_rejected() {
        let content = r#"
[workspace]
docs = ["docs/spec/scxml.md"]

[plugins.set_equality_validator]
section_namespace = ""
"#;
        let err = parse_config(content).unwrap_err();
        assert!(
            err.to_string().contains("section_namespace"),
            "expected section_namespace-validation error, got: {}",
            err
        );
    }

    #[test]
    fn set_equality_validator_namespace_accepted() {
        let content = r#"
[workspace]
docs = ["docs/spec/scxml.md"]

[plugins.set_equality_validator]
section_namespace = "scxml"
"#;
        let cfg = parse_config(content).unwrap();
        let sev = cfg
            .plugins
            .and_then(|p| p.set_equality_validator)
            .expect("set_equality_validator missing");
        assert_eq!(sev.section_namespace.as_deref(), Some("scxml"));
    }
    #[test]
    fn continuity_section_parses_with_defaults() {
        let cfg = parse_config(
            r#"
[workspace]

[continuity]
canon_order_path = "canon-order.json"
"#,
        )
        .unwrap();
        let cont = cfg.continuity.unwrap();
        assert_eq!(cont.canon_order_path.as_deref(), Some("canon-order.json"));
        assert!(cont.severity.is_reject());
        assert!(cont.canon_order_sha256.is_none());
    }

    #[test]
    fn continuity_sha256_requires_path_and_hex() {
        let err = parse_config(
            r#"
[workspace]

[continuity]
canon_order_sha256 = "0000000000000000000000000000000000000000000000000000000000000000"
"#,
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("requires `canon_order_path`"),
            "{err}"
        );
        let err = parse_config(
            r#"
[workspace]

[continuity]
canon_order_path = "canon-order.json"
canon_order_sha256 = "NOT-HEX"
"#,
        )
        .unwrap_err();
        assert!(err.to_string().contains("lowercase hex"), "{err}");
    }
}
