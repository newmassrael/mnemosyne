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
#[serde(deny_unknown_fields)]
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

    /// `[tool]` — which Mnemosyne revision may act on this workspace (Round
    /// 825). Opt-in: absent means unpinned, which is every workspace that
    /// existed before this section did.
    #[serde(default)]
    pub tool: Option<ToolSection>,
}

/// `[tool]` table — the tool pin (Round 825).
///
/// Cargo pins a consumer's LIBRARY dependencies through `Cargo.lock` and pins
/// nothing about a binary they invoke by name, so a consumer's gates run with
/// whatever tool happens to be resolvable. This is the declaration that closes
/// that: the workspace states which revision of Mnemosyne is allowed to act on
/// it, and every Mnemosyne binary refuses while carrying a different stamp.
///
/// It lives in `mnemosyne.toml` rather than beside whatever launches the tool
/// because that file is the consumer's own, versioned with the store — and
/// because the host config that launches an MCP server is not ours to change,
/// while `--workspace` already points here.
///
/// This does NOT overlap `schema_version`: that guards the shape of what is
/// written and is enforced by the store's monotone guard, while this guards the
/// judgement that produced it. Round 821 and Round 822 changed no schema and
/// changed what `validate-continuity` reports.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct ToolSection {
    /// The Mnemosyne git revision this workspace is validated by — any prefix
    /// of at least seven hex characters, git's own abbreviation floor.
    #[serde(default)]
    pub pin: Option<String>,
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
#[serde(deny_unknown_fields)]
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
/// that arise when a section is removed from the store after a prior
/// `Round N` entry has cited it. The frozen-ledger invariant blocks
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
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
pub struct SetEqualityValidatorConfig {
    /// Workspace-relative paths to scan recursively. Each entry may be a
    /// file or directory. Hidden directories (`.git/`, `.mnemosyne/`),
    /// `target/`, and `node_modules/` are always skipped (build artifacts
    /// and vendored deps shouldn't carry author-written citations).
    #[serde(default)]
    pub paths: Vec<String>,

    /// Rust trees deliberately left OUT of the citation gate (Round 783).
    ///
    /// `paths` says what is scanned; this says what is knowingly not, and
    /// `validate-workspace` fails on any `.rs` that is in neither — so a tree
    /// merely absent from the config is loud, while a tree someone wrote down
    /// stays quiet. That inversion is the point: Round 777 fixed a scan list
    /// that had drifted from the tree in silence, and `paths` alone is the same
    /// shape of claim one level up.
    ///
    /// An entry matching no file is reported as stale, the same rule the
    /// orphan-ledger axis already applies to a row whose orphan resolved.
    ///
    /// # This does NOT narrow what the gate reads (Round 860)
    ///
    /// `paths` is the only thing that decides which files are read. An entry
    /// here naming a subtree that a `paths` entry already covers matches real
    /// files, reports no stale exclusion, and changes nothing the gate does —
    /// config that looks like it works. Reported from the field by a consumer
    /// whose `paths` enrolled a parent directory holding build output: they
    /// added four exclusion prefixes, the counts did not move, and only a diff
    /// of the numbers showed it. `validate-workspace` now names that overlap as
    /// an advisory; the repair is to narrow `paths`.
    ///
    /// Empty by default: the check costs an adopter nothing until they declare
    /// something, and a workspace that has declared nothing is told about every
    /// unscanned tree it has rather than silently passing.
    #[serde(default)]
    pub scan_exclusions: Vec<String>,

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

    /// Severity for the prose-fact-assertion violation (`ProseFactAssertion`,
    /// structured-fact SSOT): a code comment that RESTATES a structured fact —
    /// a relation/status assertion verb (`supersedes`, `decided in`,
    /// `deferred to`, ...) adjacent to a `§<id>` citation — instead of merely
    /// POINTING to the section. Such facts have a single store home
    /// (`decision_status` / `superseded_by` / bindings, authored via the mutate
    /// API); prose must only project them. `None` = the axis is OFF (opt-in,
    /// like `severity_verification`). Recommended `warn` first to measure the
    /// existing-comment backlog, then escalate to `reject`. See
    /// claudedocs/structured-fact-ssot-design.md.
    #[serde(default)]
    pub severity_prose_fact_assertion: Option<Severity>,

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

    /// Round 810 — external *ledger* prefixes for the `Round NNN` axis.
    ///
    /// The section axis has carried an external escape hatch since Round 277,
    /// and this one carried none: a `<entry_id_prefix><number>` citation was
    /// always resolved against THIS workspace's changelog, so a consumer that
    /// cites an upstream project's round number in its own code had no way to
    /// say whose ledger it meant, and got a `Missing` — the hallucination
    /// class, at reject severity. Reported by a downstream workspace citing
    /// this project's rounds.
    ///
    /// Citation form is `<PREFIX> <entry_id_prefix><number>`, e.g.
    /// `mnemosyne Round 780`. Only this bare shape exists, and deliberately so:
    /// on the section axis a document needs both a NAME and a NUMBER, while
    /// here the round number IS the citation, so there is nothing for a numeric
    /// sibling key to hold.
    ///
    /// Empty list = the axis is disabled and every `Round NNN` resolves locally
    /// (the pre-Round-810 behavior exactly). Naming the ledger is a TRUE
    /// declaration, not a matcher-appeasing twist — which is why the fix is a
    /// registry rather than a heuristic such as "inside quotes means external".
    #[serde(default)]
    pub external_changelog_prefixes: Vec<String>,

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
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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
// `deny_unknown_fields` (Round 604, continuity-stress-experiment/v1 `surface_gap`):
// a misspelled key here (e.g. `narrative_rules_path` for `rules_path`) was
// SILENTLY IGNORED, leaving the rules unwired and the gate off with no signal —
// the loop had to sweep candidate key names to find the live one. Fail loud
// instead (the CLAUDE.md no-silent-fail rule): an unknown `[continuity]` key now
// rejects at config load. The other config sections are a follow-up (each needs
// its own audit); this closes the key the experiment actually hit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
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
    /// Workspace-relative path to the `narrative-rules/v1` declaration
    /// (Round 449, design sec 7.12) — consumer-vocabulary exclusivity/
    /// transition/interval rules over typed claims. Optional: authoring the
    /// file IS the opt-in. EXCLUSIVE and TRANSITION violations ride `severity`
    /// (no separate knob — a same-frame rule violation of those classes is
    /// wrong data, never a legitimate intermediate state). INTERVAL (timeline)
    /// violations are the exception: they ride `interval_severity` (Round 491),
    /// because a timeline gap can be a legitimate authored time-bend.
    #[serde(default)]
    pub rules_path: Option<String>,
    /// Optional sha256 pin of the narrative-rules file (the same R428
    /// gate-authority-input contract as `canon_order_sha256`). Requires
    /// `rules_path`.
    #[serde(default)]
    pub rules_sha256: Option<String>,
    /// Per-class severity for INTERVAL (timeline) rule violations (Round 491,
    /// design sec 7.20 step 3). `reject` | `warn` | `info`; absent = OFF.
    /// Default OFF (surface-not-gate): an interval violation is surfaced by
    /// the gate and by `report-timeline-gaps`, but gates the exit code only
    /// when set. Distinct from `severity` because — unlike a same-frame
    /// exclusive/transition contradiction — a scalar timeline gap can be an
    /// intentional authored time-bend (games bend time deliberately), so the
    /// author opts in to gating it (the `SpecDriftSection` rationale).
    #[serde(default)]
    pub interval_severity: Option<Severity>,
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

impl LoadedConfig {
    /// The resolved root AND where it came from, for a failure that must not
    /// read as an accusation against the reader's config (Round 835).
    ///
    /// A path alone does not tell a reader whether the tool honoured their
    /// `[workspace] root` or silently used the directory they happened to
    /// stand in — and those two answers demand opposite repairs. A consumer
    /// hit exactly that: `configured scan paths resolve to nothing under
    /// <toml dir>` was read as "your paths are wrong", and the cause was that
    /// the declared root had been ignored. One definition, so a second caller
    /// cannot describe the same provenance a different way.
    #[must_use]
    pub fn root_provenance(&self) -> String {
        match &self.config.workspace.root {
            Some(declared) => format!(
                "{} — from `[workspace] root = \"{declared}\"` in {}",
                self.workspace_root.display(),
                self.config_path.display()
            ),
            None => format!(
                "{} — the directory holding {} ([workspace] root is unset)",
                self.workspace_root.display(),
                self.config_path.display()
            ),
        }
    }
}

/// The identity of the running binary — its `BUILD_GIT_HASH` — registered once
/// at startup so a config load can prove which tool is opening the workspace.
static TOOL_STAMP: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// The environment variable that waives the tool pin.
pub const PIN_SKIP_ENV: &str = "MNEMOSYNE_PIN_SKIP";

/// Declare which Mnemosyne build this process is (Round 826). Call once, first
/// thing in `main`, passing `env!("BUILD_GIT_HASH")`.
///
/// A process has exactly one identity, so this is process-global on purpose
/// rather than threaded through the eighteen call sites that load a config —
/// and threading it would also force `mnemosyne-ops`, a library with no stamp
/// of its own, to carry one.
///
/// FORGETTING TO CALL THIS FAILS CLOSED. An unregistered process meeting a
/// PINNED workspace cannot prove itself and is refused; it is not waved
/// through. That is what made this the chosen enforcement shape: a mistake
/// surfaces as a loud stop rather than a silent pass. An unpinned workspace is
/// unaffected either way.
///
/// Calling it twice with the same value is fine; a second, DIFFERENT value is a
/// bug in the caller (one process, one identity) and is ignored rather than
/// silently replacing the first.
pub fn register_tool_stamp(stamp: &str) {
    let _ = TOOL_STAMP.set(stamp.to_string());
}

/// What this process registered, if anything — for diagnostics and tests.
#[must_use]
pub fn tool_stamp() -> Option<&'static str> {
    TOOL_STAMP.get().map(String::as_str)
}

/// Why a stamp does not satisfy a declared pin. Each variant is a DIFFERENT
/// repair, which is why they are not one "mismatch" (the Round 817 rule: naming
/// one failure when it is another lies about what has to be fixed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinRefusal {
    /// The process never declared what it is, so nothing can be compared.
    Unidentified,
    /// The binary was built from a tree with uncommitted changes, so it
    /// corresponds to no revision at all.
    Dirty { stamp: String },
    /// The binary was built without git, so its revision is unknowable —
    /// distinct from being known and wrong.
    Unknown,
    /// Both are revisions and they are different ones.
    Different { stamp: String, pin: String },
}

impl std::fmt::Display for PinRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unidentified => write!(
                f,
                "this build did not declare which revision it is, so the pin cannot be checked"
            ),
            Self::Dirty { stamp } => write!(
                f,
                "this build is `{stamp}` — built from a tree with uncommitted changes, \
                 so it is not any revision"
            ),
            Self::Unknown => write!(
                f,
                "this build carries no revision (built without git), so the pin cannot be checked"
            ),
            // Round 840 — the pin is NOT restated here. The one caller
            // (`enforce_tool_pin`) already opens with "<config> pins Mnemosyne
            // `<pin>`", so naming it again produced "pins `X`, and this build is
            // `Y`, and the workspace pins `X`" — one sentence saying the same
            // thing twice, joined by a second "and". Reported from the field as
            // reading like two fragments concatenated, which is what it was.
            // The other three variants do not overlap the opener and are unchanged.
            //
            // `pin` stays in the struct: `check_tool_pin` is public and its
            // Err value must carry both sides for a caller that has neither.
            Self::Different { stamp, pin: _ } => {
                write!(f, "this build is `{stamp}`")
            }
        }
    }
}

/// THE tool-pin comparison (Round 825). `Ok(())` when `stamp` satisfies `pin`.
///
/// A stamp is `git describe --always --dirty --abbrev=8`: at least eight hex
/// characters, possibly more, with an optional `-dirty` suffix. A declared pin
/// is hand-written at whatever length its author chose — the first playable
/// consumer writes eight, a Cargo.toml rev is forty — so the comparison is a
/// PREFIX one on the shorter of the two, which is how a human reads two
/// revisions for equality.
///
/// A `-dirty` or `unknown` stamp satisfies NOTHING. A dirty build corresponds to
/// no revision, and telling a pinned consumer otherwise is the exact lie the pin
/// exists to prevent.
pub fn check_tool_pin(pin: &str, stamp: Option<&str>) -> std::result::Result<(), PinRefusal> {
    let Some(stamp) = stamp else {
        return Err(PinRefusal::Unidentified);
    };
    if stamp.ends_with("-dirty") {
        return Err(PinRefusal::Dirty {
            stamp: stamp.to_string(),
        });
    }
    if stamp == "unknown" {
        return Err(PinRefusal::Unknown);
    }
    let shorter = stamp.len().min(pin.len());
    if stamp[..shorter].eq_ignore_ascii_case(&pin[..shorter]) {
        return Ok(());
    }
    Err(PinRefusal::Different {
        stamp: stamp.to_string(),
        pin: pin.to_string(),
    })
}

/// The environment marker recording that a pin switch already happened, so a
/// mis-installed root cannot ping-pong forever (Round 832).
pub const PIN_EXEC_ENV: &str = "MNEMOSYNE_PIN_EXEC";

/// The per-revision install root a pin resolves to — ONE definition, shared by
/// the switch that execs out of it and the refusal that tells you to install
/// into it.
///
/// Two copies of this path would drift, and the drift reads as the tool looking
/// somewhere its own advice never named. `MN_ROOT` overrides the base because
/// SCHEMA_GUIDE's shell recipe already documents that knob; this honours the
/// existing one rather than inventing a second.
#[must_use]
pub fn pinned_root(pin: &str) -> Option<PathBuf> {
    let base = match std::env::var_os("MN_ROOT") {
        Some(v) if !v.is_empty() => PathBuf::from(v),
        _ => PathBuf::from(std::env::var_os("HOME")?).join(".local/mn"),
    };
    Some(base.join(pin))
}

/// Where the pinned build of `bin` is expected to live. `cargo install --root R`
/// writes `R/bin/<name>`, so this is that layout read back.
#[must_use]
pub fn pinned_binary(pin: &str, bin: &str) -> Option<PathBuf> {
    Some(pinned_root(pin)?.join("bin").join(bin))
}

/// Which refusals a switch could repair.
///
/// [`PinRefusal::Unidentified`] is NOT a wrong revision — it is this build
/// failing to say what it is, which no installed binary repairs. Launching a
/// second process there would hide a defect in the first, and Round 826 chose
/// fail-closed for exactly that case. The other three all mean "this is not the
/// pinned revision", which is precisely what another binary fixes.
#[must_use]
pub fn switch_can_repair(refusal: &PinRefusal) -> bool {
    !matches!(refusal, PinRefusal::Unidentified)
}

/// The binary this process is, for both the switch target and the refusal.
fn running_binary_name() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "mnemosyne-cli".to_string())
}

/// Ask the build at `path` which revision it is, by the one surface every
/// Mnemosyne binary carries (`--version`, Round 286) — returning `None` when it
/// will not say.
///
/// This is deliberately the CHEAPEST question available and not a config-reading
/// one: a build that predates a key in the workspace cannot run any command that
/// opens a config, and `--version` is the sole survivor (measured: on a
/// pre-`[tool]` binary, `validate-workspace`, `validate-code-refs` and `query`
/// all exit 1 at the parse error while `--version` exits 0).
///
/// The revision is the parenthesised tail of `<name> <semver> (<revision>)`,
/// read from the LAST parenthesis so a name that ever gains one of its own does
/// not shift the answer.
#[cfg(unix)]
fn installed_revision(path: &Path) -> Option<String> {
    let out = std::process::Command::new(path)
        .arg("--version")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8(out.stdout).ok()?;
    let open = text.rfind('(')?;
    let close = open + text[open..].find(')')?;
    let rev = text[open + 1..close].trim();
    (!rev.is_empty()).then(|| rev.to_string())
}

/// Replace this process with the pinned build when it is ALREADY INSTALLED
/// (Round 832).
///
/// This is use, not procurement, and the distinction is the whole design.
/// Resolving a revision that is already on disk costs one `exec` and reaches no
/// network; downloading and running another version of oneself is a
/// supply-chain hazard and stays refused — an absent build falls through to the
/// refusal that says how to install it.
///
/// `exec` REPLACES the image rather than spawning a child, which is what keeps
/// an MCP server's stdio pipes, the caller's cwd, and the exit code intact. On
/// success this therefore does not return at all.
///
/// Returns `Ok(())` when no switch was possible — the caller then refuses as
/// before. Returns `Err` when a switch was already attempted for this pin and
/// did not take, which means the build installed at that path is not the
/// revision the path claims, and must not be quiet.
#[cfg(unix)]
fn switch_to_pinned(pin: &str, refusal: &PinRefusal) -> Result<()> {
    use std::io::Write;
    use std::os::unix::process::CommandExt;

    if !switch_can_repair(refusal) {
        return Ok(());
    }
    let bin = running_binary_name();
    let Some(target) = pinned_binary(pin, &bin) else {
        return Ok(());
    };

    // THE LOOP GUARD. After an exec the replacement runs with the same argv and
    // the same cwd, so it discovers the same config and reads the same pin — a
    // second mismatch can therefore only mean the binary sitting at that path is
    // not the revision the path names. That is a broken install, and exec'ing it
    // again would spin forever instead of saying so.
    if std::env::var_os(PIN_EXEC_ENV).is_some_and(|prev| prev == *std::ffi::OsStr::new(pin)) {
        anyhow::bail!(
            "already switched to `{}` for pin `{pin}`, and {refusal}.\n  \
             the build installed there is not the revision that path names — reinstall it:\n    \
             cargo install --git https://github.com/newmassrael/mnemosyne --rev {pin} --locked \\\n      \
             {bin} --root {}",
            target.display(),
            pinned_root(pin).unwrap_or_default().display()
        );
    }
    if !target.is_file() {
        // Never procures. The refusal downstream carries the install line.
        return Ok(());
    }

    // VERIFY THE TARGET BEFORE HANDING OVER (Round 861). `$MN_ROOT/<pin>/bin` is
    // a directory-NAMING convention, and until now nothing checked that the
    // build sitting there is the revision the directory claims. The loop guard
    // above does catch it — but only AFTER the exec, and only when the
    // replacement is new enough to re-check a pin at all. A build older than
    // `[tool]` instead dies at TOML parse, so the same broken install reports
    // itself as the reader's config being wrong. Measured both ways: with a
    // current build at the path the guard names the broken install correctly,
    // and with a pre-`[tool]` build at the same path the only message is
    // `mnemosyne.toml parse failed`.
    //
    // Asking first is the only place the right answer can still be given, and
    // `--version` is the one question every Mnemosyne binary answers (Round 286)
    // AND the only command that still works on a build too old to parse the
    // config — which is precisely the build this check exists to catch.
    let bin_at_target = installed_revision(&target);
    let verified = match &bin_at_target {
        Some(rev) => check_tool_pin(pin, Some(rev)),
        // Unverifiable is refused for the same reason Round 826 refuses an
        // `unknown` stamp: not knowing is not the same as knowing it is right.
        None => Err(PinRefusal::Unknown),
    };
    if let Err(there) = verified {
        anyhow::bail!(
            "{} is where pin `{pin}` resolves, but the build installed there is not that \
             revision — {}.\n  \
             the path names a revision; only reinstalling makes it true:\n    \
             cargo install --git https://github.com/newmassrael/mnemosyne --rev {pin} --locked \\\n      \
             {bin} --root {}",
            target.display(),
            match &bin_at_target {
                Some(_) => there.to_string(),
                None => "it reports no revision at all, so it cannot be checked (a build older \
                         than `--version` itself, or not a Mnemosyne binary)"
                    .to_string(),
            },
            pinned_root(pin).unwrap_or_default().display()
        );
    }

    // stderr, never stdout: an MCP server speaks its protocol on stdout and a
    // note there would corrupt the stream. Loud either way — a tool quietly
    // becoming a different tool is the surprise this must not be.
    //
    // Round 861 — no "this build" prefix: every switchable refusal's Display
    // already opens with its own subject, and Round 840 rewrote `Different` to
    // supply one without noticing this sibling caller supplied another. All
    // three switchable variants printed `note: this build this build ...`.
    eprintln!(
        "note: {refusal}; switching to the pinned build at {}",
        target.display()
    );
    // `exec` discards whatever this process has buffered and not written.
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();

    let err = std::process::Command::new(&target)
        .args(std::env::args_os().skip(1))
        .env(PIN_EXEC_ENV, pin)
        .exec();
    // `exec` returns ONLY on failure.
    Err(anyhow::anyhow!(
        "the pinned build at {} could not be run: {err}",
        target.display()
    ))
}

#[cfg(not(unix))]
fn switch_to_pinned(_pin: &str, _refusal: &PinRefusal) -> Result<()> {
    // No `exec` outside unix; the refusal below still names the install.
    Ok(())
}

/// Enforce a loaded config's `[tool] pin` against this process's stamp.
///
/// Called from every config load, so a pinned workspace cannot be opened by a
/// tool that has not proven itself — see [`register_tool_stamp`] for why the
/// identity is process-global and why forgetting it fails closed.
fn enforce_tool_pin(cfg: &WorkspaceConfig, config_path: &Path) -> Result<()> {
    let Some(pin) = cfg.tool.as_ref().and_then(|t| t.pin.as_deref()) else {
        return Ok(());
    };
    if std::env::var_os(PIN_SKIP_ENV).is_some() {
        // Loud on EVERY use, never once: a silent waiver is how a waiver
        // becomes permanent, and this one disables the guarantee outright.
        eprintln!(
            "warning: {PIN_SKIP_ENV} is set — the tool pin `{pin}` declared by {} is NOT enforced; \
             results from this run are not attributable to that revision",
            config_path.display()
        );
        return Ok(());
    }
    let Err(refusal) = check_tool_pin(pin, tool_stamp()) else {
        return Ok(());
    };
    // Use the pinned build if it is already here. Diverges on success.
    switch_to_pinned(pin, &refusal)?;

    // Name the binary that is actually refusing. Both binaries reach this one
    // message, and a remedy that says `mnemosyne-cli` to someone whose MCP
    // server just failed to start sends them to install the wrong tool.
    let bin = running_binary_name();
    let root =
        pinned_root(pin).map_or_else(|| format!("~/.local/mn/{pin}"), |p| p.display().to_string());
    // Round 840 — say HOW the two are compared. A stamp is
    // `git describe --abbrev=8` while a pin is legal at seven characters, so the
    // two printed side by side can differ in width and never be string-equal
    // even when they match. Reported from the field as a papercut; the fix is to
    // state the rule rather than to truncate the stamp, which would misreport
    // what the build actually is.
    let widths_differ =
        matches!(&refusal, PinRefusal::Different { stamp, .. } if stamp.len() != pin.len());
    let how = if widths_differ {
        "\n  (a pin is matched against the build's revision by PREFIX, on the shorter of the two)"
    } else {
        ""
    };
    Err(anyhow::anyhow!(
        "{} pins Mnemosyne `{pin}`, and {refusal}.{how}\n  \
         install the pinned revision beside the others — it is then used automatically:\n    \
         cargo install --git https://github.com/newmassrael/mnemosyne --rev {pin} --locked \\\n      \
         {bin} --root {root}\n  \
         to run anyway, knowing the result is not attributable to `{pin}`, set {PIN_SKIP_ENV}=1",
        config_path.display()
    ))
}

/// The sentence serde emits for a key no build of this type knows. Matched as
/// text because the deserializer offers no typed variant for it — and pinned by
/// a test, so a reworded upstream message turns the hint red instead of quietly
/// dropping it.
const UNKNOWN_FIELD_MARKER: &str = "unknown field";

/// What an unknown key earns beyond the parse error: the revision THIS build is,
/// and the rule that makes an unknown key a version symptom rather than only a
/// typo (Round 861).
///
/// `WorkspaceConfig` denies unknown fields on purpose, so a typo fails loud —
/// but the identical error is what a workspace declaring a NEWER key produces on
/// an OLDER binary, and the message names neither revisions nor Mnemosyne. A
/// consumer reported reaching the real cause only by reverse-engineering it:
/// their gates went red 65 minutes after they adopted `[tool] pin`, when a
/// concurrent session put an older binary back on PATH, and every command that
/// reads a config said their config was wrong.
///
/// It cannot help the binary in THAT report — a build too old to know a key is
/// also too old to carry this text — which is exactly why it is written for the
/// key that does not exist yet rather than for `tool`. A hint that special-cased
/// `tool` could only ever ship inside a build that already knows `tool`, and so
/// would never once be printed.
fn unknown_key_hint(rendered: &str) -> String {
    if !rendered.contains(UNKNOWN_FIELD_MARKER) {
        return String::new();
    }
    let who = match tool_stamp() {
        Some(stamp) => format!("this build is `{stamp}`"),
        None => "this build did not declare which revision it is".to_string(),
    };
    format!(
        "\n  {who}, and a workspace's config is readable only by a Mnemosyne at or after the \
         revision that introduced its NEWEST key.\n  \
         so: if the field named above is not a typo, this build is older than this workspace — \
         install a newer one, or remove the field"
    )
}

/// Parse a TOML byte slice into a config struct + validate.
pub fn parse_config(content: &str) -> Result<WorkspaceConfig> {
    let cfg: WorkspaceConfig = match toml::from_str(content) {
        Ok(cfg) => cfg,
        // Built rather than `context`ed: anyhow prints an added context BEFORE
        // its cause, and this belongs after the error it explains.
        Err(err) => {
            let rendered = err.to_string();
            bail!(
                "mnemosyne.toml parse failed: {rendered}{}",
                unknown_key_hint(&rendered)
            )
        }
    };
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

/// One gate-authority pin/path pair check (Round 452 — the second
/// `[continuity]` pin made it a pattern): a pin requires its path (a pin
/// with nothing to pin is a config mistake) and must be canonical hex.
fn check_authority_pin_pair(
    pin: &Option<String>,
    path_present: bool,
    pin_key: &str,
    path_key: &str,
) -> Result<()> {
    if let Some(hash) = pin {
        if !path_present {
            bail!("mnemosyne.toml: `{pin_key}` requires `{path_key}` (a pin with nothing to pin)");
        }
        if !is_lowercase_sha256_hex(hash) {
            bail!("mnemosyne.toml: `{pin_key}` must be 64-char lowercase hex (got `{hash}`)");
        }
    }
    Ok(())
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
        check_authority_pin_pair(
            &cont.canon_order_sha256,
            cont.canon_order_path.is_some(),
            "continuity.canon_order_sha256",
            "canon_order_path",
        )?;
        check_authority_pin_pair(
            &cont.rules_sha256,
            cont.rules_path.is_some(),
            "continuity.rules_sha256",
            "rules_path",
        )?;
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
    // Round 826 — the tool pin's SHAPE, checked at load like every other pin in
    // this file. Seven is git's own abbreviation floor; below it a prefix stops
    // naming one commit, and a pin that matches several revisions pins none.
    if let Some(pin) = cfg.tool.as_ref().and_then(|t| t.pin.as_deref()) {
        if pin.len() < 7 || !pin.chars().all(|c| c.is_ascii_hexdigit()) {
            bail!(
                "mnemosyne.toml: `tool.pin` must be at least 7 hex characters of a git revision \
                 (got `{pin}`)"
            );
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
    // Round 826 — the tool pin is enforced HERE, the one place a workspace is
    // opened, so every one of the eighteen call sites that reach a config is
    // covered by construction rather than by each remembering to ask.
    // `discover_config` routes through this too.
    enforce_tool_pin(&config, config_path)?;

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

[spec_drift]
"#;
        let cfg = parse_config(content).unwrap();
        assert_eq!(cfg.spec_drift.unwrap().severity.as_str(), "warn");
    }

    #[test]
    fn spec_drift_rejects_invalid_severity() {
        let content = r#"
[workspace]

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
"#;
        assert!(parse_config(absent).unwrap().commit_ledger.is_none());

        let bare = r#"
[workspace]

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
"#;
        assert!(parse_config(absent).unwrap().content_drift.is_none());

        let bare = r#"
[workspace]

[content_drift]
"#;
        let cfg = parse_config(bare).unwrap();
        assert_eq!(cfg.content_drift.unwrap().severity.as_str(), "reject");
    }

    #[test]
    fn content_drift_rejects_invalid_severity() {
        let content = r#"
[workspace]

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
        let content = "[workspace]\n";
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
        let content = "[workspace]\n";
        let cfg = parse_config(content).unwrap();
        assert!(cfg.orphan_ledger.is_empty());
    }

    #[test]
    fn orphan_ledger_array_of_tables_parses() {
        let content = r#"
[workspace]

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
            "[workspace]\nroot = \"..\"\n",
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
        // interval_severity is OFF by default (surface-not-gate, Round 491).
        assert!(cont.interval_severity.is_none());
    }

    #[test]
    fn continuity_interval_severity_parses() {
        let cfg = parse_config(
            r#"
[workspace]

[continuity]
interval_severity = "warn"
"#,
        )
        .unwrap();
        assert_eq!(
            cfg.continuity.unwrap().interval_severity,
            Some(Severity::Warn)
        );
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

    #[test]
    fn continuity_rules_path_parses_and_pin_requires_path_and_hex() {
        // Round 449: narrative-rules declaration, same R428 pin contract.
        let cfg = parse_config(
            r#"
[workspace]

[continuity]
canon_order_path = "canon-order.json"
rules_path = "narrative-rules.json"
"#,
        )
        .unwrap();
        let cont = cfg.continuity.unwrap();
        assert_eq!(cont.rules_path.as_deref(), Some("narrative-rules.json"));
        assert!(cont.rules_sha256.is_none());
        let err = parse_config(
            r#"
[workspace]

[continuity]
rules_sha256 = "0000000000000000000000000000000000000000000000000000000000000000"
"#,
        )
        .unwrap_err();
        assert!(err.to_string().contains("requires `rules_path`"), "{err}");
        let err = parse_config(
            r#"
[workspace]

[continuity]
rules_path = "narrative-rules.json"
rules_sha256 = "NOT-HEX"
"#,
        )
        .unwrap_err();
        assert!(err.to_string().contains("lowercase hex"), "{err}");
    }

    #[test]
    fn continuity_rejects_an_unknown_key_fail_loud() {
        // Round 604 (continuity-stress-experiment/v1 `surface_gap`): a misspelled
        // key (here `narrative_rules_path` for `rules_path`) must REJECT, not be
        // silently ignored — the footgun the experiment hit (the loop had to
        // sweep candidate names because the wrong key wired nothing with no error).
        let res = parse_config(
            r#"
[workspace]

[continuity]
narrative_rules_path = "narrative-rules.json"
"#,
        );
        assert!(
            res.is_err(),
            "an unknown [continuity] key must fail loud, got: {res:?}"
        );
        // anyhow wraps the serde error under a `.context()`; the underlying
        // "unknown field `narrative_rules_path`" is in the chain (alternate `{:#}`).
        let err = format!("{:#}", res.unwrap_err());
        assert!(
            err.contains("narrative_rules_path") || err.contains("unknown field"),
            "an unknown [continuity] key must fail loud: {err}"
        );
        // A correctly-spelled key still parses (the fix does not over-reject).
        let ok = parse_config(
            "[workspace]\n\n[continuity]\ncanon_order_path = \"canon-order.json\"\nrules_path = \"narrative-rules.json\"\n",
        );
        assert!(ok.is_ok(), "correct keys must still parse: {ok:?}");
    }

    /// Round 861 — an unknown key is ALSO a version symptom, and the error said
    /// only "unknown field". A consumer reached the real cause (their binary was
    /// older than their config) by reverse-engineering it.
    ///
    /// This test is also what keeps the hint attached: it is bound to serde's
    /// wording by one string, so an upstream rewording turns this red instead of
    /// dropping the hint in silence.
    #[test]
    fn an_unknown_key_says_the_build_may_be_older_than_the_workspace() {
        // A key no build of this type knows — which is the shape a NEWER
        // Mnemosyne's section has when an OLDER binary meets it.
        let err = format!(
            "{:#}",
            parse_config("[workspace]\n\n[from_a_later_round]\nkey = 1\n").unwrap_err()
        );
        assert!(
            err.contains(UNKNOWN_FIELD_MARKER),
            "serde no longer says `{UNKNOWN_FIELD_MARKER}`, so the hint is attached \
             to nothing — reattach it: {err}"
        );
        assert!(
            err.contains("older than this workspace"),
            "an unknown key must offer the version reading, not only the typo one: {err}"
        );
        // NON-VACUOUS, and this half is the one that can rot quietly: a
        // wrong-TYPED value is also a parse failure and must NOT collect the
        // hint. Telling someone whose value is a number that their tool is old
        // sends them to reinstall a binary that was never the problem.
        let typed = format!(
            "{:#}",
            parse_config("[workspace]\n\n[tool]\npin = 7\n").unwrap_err()
        );
        assert!(
            !typed.contains("older than this workspace"),
            "a wrong-typed value collected the version hint: {typed}"
        );
    }

    /// Round 826 — the tool-pin comparison, across every way a stamp can fail to
    /// be a revision. The refusals are separate variants because each is a
    /// different repair, so the test reads them rather than only `is_err`.
    #[test]
    fn a_stamp_satisfies_a_pin_only_by_being_that_revision() {
        // A pin is written at whatever length its author chose and a stamp is
        // at least eight characters, so the comparison is a prefix one on the
        // shorter side — in BOTH directions.
        assert!(check_tool_pin("75eddce5", Some("75eddce58b3c")).is_ok());
        assert!(check_tool_pin("75eddce58b3c10532b2e", Some("75eddce5")).is_ok());
        assert!(
            check_tool_pin("75EDDCE5", Some("75eddce5")).is_ok(),
            "hex case"
        );
        // A DIRTY build is not any revision, which is the whole reason the pin
        // exists — the shared slot held exactly this for a whole session.
        assert_eq!(
            check_tool_pin("75eddce5", Some("75eddce5-dirty")),
            Err(PinRefusal::Dirty {
                stamp: "75eddce5-dirty".to_string()
            }),
            "a dirty build must not satisfy the pin it was built from"
        );
        // Built without git: unknowable, which is not the same as known-wrong.
        assert_eq!(
            check_tool_pin("75eddce5", Some("unknown")),
            Err(PinRefusal::Unknown)
        );
        // FAIL-CLOSED: a process that never said what it is cannot be waved
        // through. This is the property that chose this enforcement shape.
        assert_eq!(
            check_tool_pin("75eddce5", None),
            Err(PinRefusal::Unidentified)
        );
        assert!(matches!(
            check_tool_pin("75eddce5", Some("11d703bf")),
            Err(PinRefusal::Different { .. })
        ));
    }

    /// A pin too short to name one commit is a config error, not a loose match.
    #[test]
    fn a_pin_must_be_hex_and_long_enough_to_name_one_commit() {
        assert!(parse_config("[workspace]\n\n[tool]\npin = \"75eddce5\"\n").is_ok());
        for bad in ["75edd", "nothexx", ""] {
            let err = parse_config(&format!("[workspace]\n\n[tool]\npin = \"{bad}\"\n"))
                .expect_err("must reject {bad}");
            assert!(
                format!("{err:#}").contains("tool.pin"),
                "the message must name the key: {err:#}"
            );
        }
        // No [tool] at all is the OPT-IN half: every workspace that predates
        // this section keeps working untouched.
        assert!(parse_config("[workspace]\n").is_ok());
    }

    /// Round 832 — a switch repairs a WRONG revision, never an unidentified
    /// build. The three that mean "this is not the pin" are fixed by running the
    /// one that is; `Unidentified` means this build never said what it is, which
    /// is a defect in the build itself and which launching another process would
    /// hide rather than fix.
    #[test]
    fn only_a_wrong_revision_is_repairable_by_switching() {
        assert!(!switch_can_repair(&PinRefusal::Unidentified));
        assert!(switch_can_repair(&PinRefusal::Unknown));
        assert!(switch_can_repair(&PinRefusal::Dirty {
            stamp: "31153157-dirty".into()
        }));
        assert!(switch_can_repair(&PinRefusal::Different {
            stamp: "11d703bf".into(),
            pin: "75eddce5".into()
        }));
    }

    // The resolved path — that it is the layout `cargo install --root` writes,
    // and that the refusal's install line names the same directory the switch
    // looked in — is asserted in `tool_pin_smoke.rs` instead. It depends on
    // process environment, and setting that here would race the other tests in
    // this binary; the smoke test sets it on a CHILD, where it is hermetic.
}
