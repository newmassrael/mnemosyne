//! mnemosyne-mcp — Model Context Protocol server for Mnemosyne.
//!
//! Exposes the production design-doc lifecycle CLI as MCP tools, plus a
//! curated set of concept resources under `mnemosyne://concepts/*` so
//! AI clients can internalize Mnemosyne's semantics before mutating.
//!
//! Transport: stdio. Configure your MCP client with:
//!
//! ```jsonc
//! {
//! "mcpServers": {
//! "mnemosyne": {
//! "command": "mnemosyne-mcp",
//! "args": ["--workspace", "."]
//! }
//! }
//! }
//! ```

mod resources;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use mnemosyne_atomic::{self as atomic, ChangelogEntryDraft, ExampleBlock, RejectedAlternative};
use mnemosyne_core::{strip_section_marker, InventoryStatus};
use mnemosyne_ops::{
    self as ops, run_atomic_mutate, MutateOutcome, OpError, QuerySectionMode, QueryTermInput,
    RedactTermInput, StyleCheckInput,
};
use mnemosyne_projection::{ProjectionService, ProjectionValidation};
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{
        Annotated, CallToolResult, ListResourcesResult, PaginatedRequestParams, RawResource,
        ReadResourceRequestParams, ReadResourceResult, ResourceContents, ServerCapabilities,
        ServerInfo,
    },
    schemars,
    service::{RequestContext, RoleServer},
    tool, tool_handler, tool_router,
    transport::stdio,
    ErrorData as McpError, ServerHandler, ServiceExt,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct EmptyArgs {}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ValidateProjectionArgs {
    /// Force a re-sync from the current log before validating. The warm
    /// projection already re-syncs automatically after every successful mutate
    /// tool (Round 341), so the default (false) is current; pass true only to
    /// pick up an out-of-band log change (e.g. a manual JSON edit or a CLI
    /// mutate run against the same workspace).
    #[serde(default)]
    pub refresh: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct QuerySectionArgs {
    /// Section ID without the leading `§` (e.g. `"39"`, `"39.1"`,
    /// `"changelog"`). Pass `--list-sections` form via `list_sections`
    /// instead.
    pub section_id: String,
    /// Include 1-hop CrossRef neighborhood (outbound + inbound).
    #[serde(default)]
    pub include_related: bool,
    /// Include §N citations from changelog entries.
    #[serde(default)]
    pub include_changelog: bool,
}

// Round 467/470 — whole-ledger changelog listing (R410 read model exposed).
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListChangelogArgs {
    /// Keep only the newest N entries (the returned `total` still reports
    /// the full ledger size — a bounded read is never mistaken for the
    /// whole ledger). Omit for the complete ledger.
    #[serde(default)]
    pub limit: Option<usize>,
}

// Round 292 — query_term read primitive (literal/regex search across the
// atomic store). Pure read; preview substrate for the deferred redact_term
// mutate primitive but useful standalone for verifying a term's footprint
// before mutating.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct QueryTermArgs {
    /// Pattern to search. Literal by default; set `regex = true` to
    /// interpret as a regex (`regex` crate syntax).
    pub pattern: String,
    /// Interpret `pattern` as a regex. Default = literal substring.
    #[serde(default)]
    pub regex: bool,
    /// Case-insensitive match. Default = case-sensitive.
    #[serde(default)]
    pub case_insensitive: bool,
    /// Scope. One of `"all"` (default), `"sections"`, `"changelog"`,
    /// `"inventory"`.
    #[serde(default)]
    pub scope: Option<String>,
    /// Optional field-name whitelist. When non-empty, only hits in the
    /// listed fields are returned. Use base field names: `"intent"`,
    /// `"rationale_bullets"`, `"decision_summary"`,
    /// `"changes_bullets"`, `"alternatives_rejected"`, `"examples"`,
    /// `"bindings"`, `"source"`, `"reason"`, and the identifier
    /// keys `"section_id"` / `"entry_id"` / `"inventory_id"` (Round 467).
    /// Unknown names reject loudly with the scope's valid-field list
    /// (Round 468), never a silent 0-hit result.
    #[serde(default)]
    pub fields: Vec<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct StyleCheckArgs {
    /// Optional doc path relative to workspace root. Omit to check
    /// every doc listed in `mnemosyne.toml`.
    #[serde(default)]
    pub doc: Option<String>,
    /// Severity filter — `"t3"`, `"t4"`, or `"all"` (default).
    #[serde(default)]
    pub severity: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetSectionTextArgs {
    /// Section ID to mutate. Pass `"39"`, not `""`.
    pub section_id: String,
    /// New value. For intent: a single sentence, max ~200 chars.
    pub text: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetSectionBulletsArgs {
    pub section_id: String,
    /// Ordered list of bullets. Each ≤ 100 chars per T3 default.
    pub bullets: Vec<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddSectionCaveatArgs {
    pub section_id: String,
    /// Single caveat bullet to append.
    pub bullet: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetImpactScopeArgs {
    pub section_id: String,
    /// Cross-ref targets without the `§` prefix, e.g. `["39", "61.1"]`.
    pub refs: Vec<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddSectionExampleArgs {
    pub section_id: String,
    /// Code-fence language tag (e.g. `"rust"`, `"toml"`).
    pub language: String,
    /// Code body — embedded inside a fenced block. No leading fence.
    pub code: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddSectionBindingArgs {
    /// Section ID without the `§` prefix.
    pub section_id: String,
    /// Workspace-relative POSIX file path. No leading `/`, no leading
    /// `./`, no `..` segment, no backslash. The file does not need to
    /// exist at write time — schema records intent.
    pub file: String,
    /// Optional opaque language-agnostic identifier (function / type /
    /// qualified path). Stored as-is; no language-grammar regex applied.
    /// Omit for file-level binding.
    #[serde(default)]
    pub symbol: Option<String>,
    /// Trace-link kind: `"implements"` (= SysML «satisfy»; the symbol
    /// fulfills the section's normative requirement; the only kind counted
    /// as coverage) or `"references"` (= SysML «trace»; the symbol relates
    /// to / draws meaning from the section without claiming fulfillment).
    pub kind: String,
}

// Round 287/289 — Section creation + outline setter MCP arg structs.

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddSectionArgs {
    /// Section ID to create. No `§` prefix in the value; use the bare slug
    /// or numbered id (e.g. `"39"`, `"39.1"`, `"my-section"`).
    pub section_id: String,
    /// Owning doc identifier (workspace-relative path or doc id).
    pub parent_doc: String,
    /// Heading title (non-empty).
    pub title: String,
    /// Optional parent section id. Omit for top-level; pass a bare id
    /// (no `§`) to nest under an existing section. The parent must exist
    /// in the atomic store at write time.
    #[serde(default)]
    pub parent_section: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetSectionParentSectionArgs {
    /// Section being re-parented.
    pub section_id: String,
    /// New parent. Pass `Some("<id>")` to nest under that section, or
    /// `None` (omit) to promote to top-level. Self-loop rejected.
    #[serde(default)]
    pub parent_section: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RemoveSectionBindingArgs {
    /// Section ID without the `§` prefix.
    pub section_id: String,
    /// Workspace-relative POSIX file path to remove from the binding set.
    pub file: String,
    /// Optional symbol — must exact-match the row to remove. Omit to
    /// target a file-only binding (a row with `symbol = None`). Matching is
    /// kind-agnostic (identity is the `(file, symbol)` pair).
    #[serde(default)]
    pub symbol: Option<String>,
    /// Mandatory rationale recorded on the receipt (audit safeguard).
    pub reason: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetSectionBindingKindArgs {
    /// Section ID without the `§` prefix.
    pub section_id: String,
    /// Workspace-relative POSIX file path of the existing binding.
    pub file: String,
    /// Optional symbol identifying the binding (omit for a file-only row).
    #[serde(default)]
    pub symbol: Option<String>,
    /// New kind: `"implements"` or `"references"`.
    pub kind: String,
    /// Mandatory rationale recorded on the receipt (audit safeguard).
    pub reason: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetSectionCoverageExpectationArgs {
    /// Section ID without the `§` prefix.
    pub section_id: String,
    /// `"normative"` (expects an implements binding) or `"informative"`
    /// (prose-only, exempt from the coverage axiom).
    pub expectation: String,
    /// Mandatory rationale recorded on the receipt (audit safeguard).
    pub reason: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetSectionVerificationExpectationArgs {
    /// Section ID without the `§` prefix.
    pub section_id: String,
    /// `"dedicated"` (expects a `verifies` binding to a test/report artifact)
    /// or `"by_construction"` (no independently-assertable per-unit oracle,
    /// exempt from the dedicated-verify gate).
    pub expectation: String,
    /// Mandatory rationale recorded on the receipt (audit safeguard).
    pub reason: String,
}

/// R417 — confirmation-event MCP args. A `file` present makes it a
/// VerifiesBinding claim, else a SectionCompleteness claim. Enum fields take the
/// snake_case tag. The event_id is derived in-core (not supplied).
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddConfirmationEventArgs {
    /// Claim section ID without the `§` prefix.
    pub section_id: String,
    /// Bound file (VerifiesBinding claim). Omit for a SectionCompleteness claim.
    pub file: Option<String>,
    /// Bound symbol (requires `file`).
    pub symbol: Option<String>,
    /// `"tool"` (deterministic, reproducible) or `"model"` (fresh-context LLM).
    pub confirmer_kind: String,
    pub confirmer_id: String,
    pub confirmer_version: String,
    /// `"linkage_check"` | `"semantic_review"` | `"coverage_attestation"`.
    pub method: String,
    /// `"confirm"` or `"refute"`.
    pub verdict: String,
    /// The run that authored the claim.
    pub authoring_run: String,
    /// The run producing THIS verdict (must differ from `authoring_run`).
    pub confirming_run: String,
    pub rationale: String,
    /// Caller-supplied timestamp (determinism — never generated in-core).
    pub timestamp: String,
    pub spec_sha256: Option<String>,
    #[serde(default)]
    pub code_sha256: Vec<String>,
    #[serde(default)]
    pub test_sha256: Vec<String>,
}

// Round 435 — narrative authoring MCP arg structs (design sec 7.10 pull 3:
// an authoring AI's interface is MCP, the R127 mutate-gate).

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddFrameArgs {
    /// Frame id — the registry key every fact's `frame` must reference.
    pub frame_id: String,
    /// Optional free-form description (whose epistemic frame this is).
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddBranchArgs {
    /// Branch id — the registry key every non-default fact `branch` must
    /// reference. `main` is known by construction and never registered.
    pub branch_id: String,
    /// Optional free-form description (which quest-path/playthrough world).
    #[serde(default)]
    pub description: String,
    /// Parent world-line this branch diverges from (R438). Give with
    /// `forks_at`; omit both for a standalone world.
    #[serde(default)]
    pub forks_from: Option<String>,
    /// Canon point of divergence (structure-section id).
    #[serde(default)]
    pub forks_at: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddEntityArgs {
    /// Entity id — the registry key fact `entities` refs must name.
    pub entity_id: String,
    /// Free-form kind tag (consumer-defined, e.g. character/location).
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReportEntityArgs {
    /// Entity id to assemble the dossier for.
    pub entity_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddPredicateArgs {
    /// Predicate id — the registry key every TypedClaim predicate must
    /// name. Load-bearing (narrative rules key off it), hence fail-loud.
    pub predicate_id: String,
    /// Declared object shape: `entity` | `scalar`. Unknown tags reject.
    pub object_kind: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddDisclosurePlanArgs {
    /// Telling id — the registry key for this named telling over the fact base.
    pub telling_id: String,
    /// Default disclosure mode: withhold | state | hint | imply. Unknown rejects.
    pub default_mode: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetDisclosureArgs {
    /// Telling id (add_disclosure_plan first).
    pub telling_id: String,
    /// Fact id the override targets (must exist; withhold/first_at need it typed).
    pub fact_id: String,
    /// Disclosure mode: withhold | state | hint | imply.
    pub mode: String,
    /// Per-world-line first-disclosure coordinate: branch id -> section ref.
    #[serde(default)]
    pub first_at: std::collections::BTreeMap<String, String>,
    /// Optional diegetic surface scene (section ref the disclosure rides on).
    #[serde(default)]
    pub surface_scene: Option<String>,
    /// Optional diegetic surface object (registered entity id).
    #[serde(default)]
    pub surface_object: Option<String>,
}

/// The optional typed leg of a fact (R446): the machine-readable
/// subject–predicate–object reading of the prose claim, authored in the
/// same act (never NLP-derived). Give exactly ONE of `object_entity` /
/// `object_value`, matching the predicate's declared `object_kind`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct TypedClaimArgs {
    /// Subject entity id (registered AND listed in the fact's `entities`).
    pub subject: String,
    /// Registered predicate id (`add_predicate` first).
    pub predicate: String,
    /// Entity-shaped object (predicate `object_kind = entity`).
    #[serde(default)]
    pub object_entity: Option<String>,
    /// Scalar object value, consumer vocabulary (`object_kind = scalar`).
    #[serde(default)]
    pub object_value: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddFactArgs {
    pub fact_id: String,
    /// Epistemic frame id (must already be registered — `add_frame` first).
    pub frame: String,
    /// World-line branch. Omit for the default branch (`main`).
    #[serde(default)]
    pub branch: Option<String>,
    /// The claim, per-claim granularity (one atomic assertion).
    pub claim: String,
    /// Structure-section id where the claim starts holding.
    pub canon_from: String,
    /// Explicit canon end for a belief that ends WITHOUT a successor.
    #[serde(default)]
    pub canon_to: Option<String>,
    /// Evidencing structure-section ids (>= 1).
    pub evidence: Vec<String>,
    /// Recorded conflict assertions (existing fact ids).
    #[serde(default)]
    pub conflicts_with: Vec<String>,
    /// In-frame predecessor this claim replaces (same frame + branch).
    #[serde(default)]
    pub supersedes_in_frame: Option<String>,
    /// Optional verbatim quote (sha256 stamped by the primitive).
    #[serde(default)]
    pub quote: Option<String>,
    /// Entity refs (each must be registered — `add_entity` first).
    #[serde(default)]
    pub entities: Vec<String>,
    /// Setup marking: `expected` declares this fact a setup whose payoff
    /// coverage `report_payoff_coverage` classifies. Omit for `unmarked`.
    #[serde(default)]
    pub payoff_expectation: Option<String>,
    /// Setup fact ids this fact pays off (existing facts; unpinned
    /// identity refs).
    #[serde(default)]
    pub pays_off: Vec<String>,
    /// Optional typed leg (R446): subject–predicate–object reading of the
    /// claim. Omit for a prose-only fact (partial coverage is the design).
    #[serde(default)]
    pub typed: Option<TypedClaimArgs>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AmendFactArgs {
    /// The revised fact content (same shape as `add_fact`; `fact_id` names
    /// the existing fact to revise — the id never changes).
    #[serde(flatten)]
    pub fact: AddFactArgs,
    /// Mandatory rationale (audit safeguard).
    pub reason: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RetractFactArgs {
    pub fact_id: String,
    /// Mandatory rationale (audit safeguard).
    pub reason: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddFactConflictArgs {
    pub fact_id: String,
    pub conflicts_with: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ValidateContinuityArgs {
    /// Canon-order declaration path override (workspace-relative; bypasses
    /// the configured sha256 pin — the R428 rule). Omit to use
    /// `[continuity].canon_order_path`.
    #[serde(default)]
    pub order_path: Option<String>,
    /// `narrative-rules/v1` declaration path override (Round 449;
    /// workspace-relative, bypasses the configured sha256 pin). Omit to
    /// use `[continuity].rules_path`.
    #[serde(default)]
    pub rules_path: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReportTimelineGapsArgs {
    /// Canon-order declaration path override (bypasses the pin).
    #[serde(default)]
    pub order_path: Option<String>,
    /// `narrative-rules/v1` declaration path override (Round 490; the
    /// interval rules). Omit to use `[continuity].rules_path`.
    #[serde(default)]
    pub rules_path: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReportFrameViewArgs {
    /// Epistemic frame to project.
    pub frame: String,
    /// World-line branch. Omit for the default branch (`main`).
    #[serde(default)]
    pub branch: Option<String>,
    /// Entity filter (Round 437) — the NPC-context query is frame ×
    /// branch × entity at T. Omit for the whole frame.
    #[serde(default)]
    pub entity: Option<String>,
    /// Canon point (structure-section id).
    pub at: String,
    /// Canon-order declaration path override (bypasses the pin).
    #[serde(default)]
    pub order_path: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReportPayoffCoverageArgs {
    /// Canon-order declaration path override (bypasses the pin).
    #[serde(default)]
    pub order_path: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReportIronyIntervalsArgs {
    /// Canon-order declaration path override (bypasses the pin).
    #[serde(default)]
    pub order_path: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReportPlaythroughManuscriptArgs {
    /// Single-world filter (a registered branch id or `main`); omitted =
    /// every query world. Fail-loud on an unregistered id.
    #[serde(default)]
    pub world: Option<String>,
    /// Canon-order declaration path override (bypasses the pin).
    #[serde(default)]
    pub order_path: Option<String>,
    /// Disclosure telling id (R506 render-brief carrier): annotate each
    /// begins-event with its disclosure decision (mode/first_at/surface) under
    /// the named telling. Fail-loud on a typo'd id.
    #[serde(default)]
    pub telling: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReportForkTreeArgs {
    /// Canon-order declaration path override (bypasses the pin).
    #[serde(default)]
    pub order_path: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ImportTypingProposalsArgs {
    /// Path to a `typing-proposals/v1` JSON artifact (workspace-relative
    /// or absolute).
    pub proposals_path: String,
    /// Validate only — full verdicts, nothing written.
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReportEdgeCandidatesArgs {
    /// Canon-order declaration path override (bypasses the pin).
    #[serde(default)]
    pub order_path: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ImportEdgeProposalsArgs {
    /// Path to an `edge-proposals/v1` JSON artifact (workspace-relative
    /// or absolute).
    pub proposals_path: String,
    /// Validate only — full verdicts, nothing written.
    #[serde(default)]
    pub dry_run: bool,
}

// Round 278 — Phase 1A inventory MCP arg structs.

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct InventoryIdArgs {
    /// Inventory id (e.g. `"ARP_07"`, `"TCP_RETRANSMISSION_TO_04"`).
    pub inventory_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddInventoryEntryArgs {
    /// Stable inventory id. Must be non-empty, no whitespace.
    pub inventory_id: String,
    /// Lifecycle status: `"active"` / `"deprecated"` / `"reserved"`.
    pub status: String,
    /// Optional section binding without leading `§` (e.g. `"4.2.4"`).
    #[serde(default)]
    pub section_ref: Option<String>,
    /// Optional traceability pointer (PDF page ref, JSON row id, etc.).
    #[serde(default)]
    pub source: Option<String>,
    /// Optional rationale (typically used when status starts as
    /// `"deprecated"` — explains the deprecation cause).
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetInventoryStatusArgs {
    pub inventory_id: String,
    /// New status: `"active"` / `"deprecated"` / `"reserved"`.
    pub status: String,
    /// Optional reason. Omit to preserve existing; empty string clears.
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetInventorySectionRefArgs {
    pub inventory_id: String,
    /// New section_ref without `§`. Omit (or pass `null`) AND set
    /// `clear: true` to unset the binding.
    #[serde(default)]
    pub section_ref: Option<String>,
    /// Set to `true` to explicitly unset the section_ref. Exactly one
    /// of `section_ref` or `clear` must be present.
    #[serde(default)]
    pub clear: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RemoveInventoryEntryArgs {
    pub inventory_id: String,
    /// Mandatory rationale recorded in the receipt (audit safeguard).
    pub reason: String,
}

// Round 295 — publishable-half setters. Round 299 — MCP wire so the
// publishable side can be authored without a CLI subprocess. The audit half
// stays write-once via append_changelog_entry; these tools only mutate
// the publishable_* mirror and must be paired with a
// [[publishable_override_ledger]] row (R296 gate, automated by redact_term).

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetChangelogPublishableStringArgs {
    /// Existing entry_id whose publishable_decision_summary will be updated.
    /// NotFound if the entry has not been appended yet.
    pub entry_id: String,
    /// Replacement decision_summary text. The audit half is untouched.
    pub value: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetChangelogPublishableBulletsArgs {
    /// Existing entry_id whose publishable bullet list will be replaced.
    pub entry_id: String,
    /// Replacement bullets in order. Empty vec clears the publishable list
    /// (audit half untouched).
    pub bullets: Vec<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct EmitPublishableOverrideLedgerDraftArgs {
    /// Entry whose current publishable-vs-audit divergence is rendered as
    /// a `[[publishable_override_ledger]]` block. NotFound if entry_id is
    /// absent; returns `in_sync: true` and `ledger_draft: null` when the
    /// publishable half still matches the audit half (nothing to anchor).
    pub entry_id: String,
    /// Audit reason recorded in the draft. Mandatory.
    pub reason: String,
    /// `applied_in` field for the draft (commit ref, PR id, etc.). Mandatory.
    pub applied_in: String,
    /// Override kind label. Defaults to `"redaction"`.
    #[serde(default)]
    pub kind: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RedactTermArgs {
    /// Pattern to search across the publishable half. Literal by default;
    /// set `regex = true` for `regex` crate syntax.
    pub pattern: String,
    /// Replacement string. Substituted verbatim per match.
    pub replacement: String,
    /// Treat `pattern` as a regex. Default = literal substring.
    #[serde(default)]
    pub regex: bool,
    /// Case-insensitive match. Default = case-sensitive.
    #[serde(default)]
    pub case_insensitive: bool,
    /// Field scope. One of `"all"` (default), `"decision_summary"`,
    /// `"changes_bullets"`, `"verification_bullets"`, `"impact_refs"`,
    /// `"carry_forward_bullets"`.
    #[serde(default)]
    pub scope: Option<String>,
    /// Dry-run mode: returns hits + ledger drafts without mutating the
    /// store. Default = false.
    #[serde(default)]
    pub dry_run: bool,
    /// Audit reason recorded in every emitted ledger draft. Mandatory.
    pub reason: String,
    /// `applied_in` field for the ledger draft (commit ref, PR id, etc.).
    pub applied_in: String,
    /// Override kind label for ledger drafts. Defaults to `"redaction"`.
    #[serde(default)]
    pub kind: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AppendChangelogEntryArgs {
    /// Entry id matching `[schema] entry_id_prefix`. Must be strictly
    /// monotonic (greater than the last entry's id).
    pub entry_id: String,
    /// One-sentence headline of the decision.
    pub decision_summary: String,
    /// What concretely changed. File paths, primitives, etc.
    pub changes_bullets: Vec<String>,
    /// How the change was validated (tests, measurements).
    pub verification_bullets: Vec<String>,
    /// Section ids affected (without `§`), e.g. `["39", "66"]`.
    #[serde(default)]
    pub impact_refs: Vec<String>,
    /// Carry-forward items for next round.
    #[serde(default)]
    pub carry_forward_bullets: Vec<String>,
}

#[derive(Clone)]
pub struct MnemosyneServer {
    workspace: Arc<PathBuf>,
    /// Warm read-side projection (convergence C/D Step 1). Built once from the
    /// log at startup and held across tool calls so `validate_projection` serves
    /// from the in-process Salsa memo cache. Shared (not duplicated) across the
    /// router's handler clones.
    projection: Arc<Mutex<ProjectionService>>,
    /// Serializes every mutate tool's load→mutate→save against concurrent
    /// `tools/call` (Round 448 session review): MCP clients may issue
    /// parallel calls, and two unserialized mutates on one store file are a
    /// lost-update race. Held only across the mutate itself; read tools
    /// stay lock-free (they tolerate seeing the pre- or post-state).
    mutate_lock: Arc<Mutex<()>>,
    #[allow(dead_code)] // populated by #[tool_router] expansion
    tool_router: ToolRouter<Self>,
}

impl MnemosyneServer {
    pub fn new(workspace: PathBuf) -> Result<Self, ops::OpError> {
        let atomic = ops::load_atomic_store(&workspace, None)?;
        let projection = ProjectionService::build(&atomic, atomic::MAIN_BRANCH_ID);
        Ok(Self {
            workspace: Arc::new(workspace),
            projection: Arc::new(Mutex::new(projection)),
            mutate_lock: Arc::new(Mutex::new(())),
            tool_router: Self::tool_router(),
        })
    }

    /// THE single mutate-lock acquisition site (Rounds 448 + 460): every
    /// store-writing tool runs inside this guard, whatever its return
    /// shape — a second hand-rolled lock acquisition is how two mutate
    /// paths drift (the half-enforced-invariant class). CLI invocations
    /// are process-per-call and need no lock; cross-PROCESS concurrency
    /// on one store stays the filesystem/git domain.
    fn with_mutate_lock<T>(&self, f: impl FnOnce() -> T) -> T {
        let _guard = self
            .mutate_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        f()
    }

    /// The receipt-shaped mutate entry (Round 448): lock + run the
    /// primitive in-process. Verdict-report mutates (e.g.
    /// `import_typing_proposals`) use [`Self::with_mutate_lock`] directly.
    fn run_mutate<F>(&self, primitive: F) -> Result<ops::MutateOutcome, ops::OpError>
    where
        F: FnOnce(
            &mut atomic::AtomicStore,
            &std::path::Path,
        ) -> Result<atomic::AtomicMutateReceipt, atomic::AtomicMutateError>,
    {
        self.with_mutate_lock(|| run_atomic_mutate(&self.workspace, None, primitive))
    }

    fn tool_text(s: String) -> CallToolResult {
        CallToolResult::success(vec![rmcp::model::Content::text(s)])
    }

    fn tool_error(s: String) -> CallToolResult {
        CallToolResult::error(vec![rmcp::model::Content::text(s)])
    }

    /// Serialize a structured payload to pretty JSON (read ops + receipts).
    fn tool_json<T: Serialize>(&self, value: &T) -> CallToolResult {
        match serde_json::to_string_pretty(value) {
            Ok(s) => Self::tool_text(s),
            Err(e) => Self::tool_error(format!("serialize: {}", e)),
        }
    }

    /// Map an in-process op error to a tool error with workspace context.
    fn op_error(&self, e: OpError) -> CallToolResult {
        Self::tool_error(format!("workspace={}\n{}", self.workspace.display(), e))
    }

    /// Finish a mutate op: re-sync the warm validation projection from the
    /// just-written log, then receipt JSON. The atomic store is the only
    /// authoritative artifact; there is nothing rendered to regenerate.
    fn finish_mutate(&self, outcome: Result<MutateOutcome, OpError>) -> CallToolResult {
        match outcome {
            Ok(o) => {
                if let Err(e) = self.sync_read_models_after_mutate() {
                    return self.op_error(e);
                }
                self.tool_json(&o)
            }
            Err(e) => self.op_error(e),
        }
    }

    /// Re-sync the warm validation projection from the just-written log after a
    /// successful mutate. Incrementally reconciles the warm `FineCascadeDb` from
    /// the in-memory snapshot so `validate_projection` reflects the current log.
    /// Operates on the already-loaded store (rebuildable cache, not authoritative
    /// state); poisoned locks are recovered.
    fn sync_read_models_after_mutate(&self) -> Result<(), OpError> {
        let atomic = ops::load_atomic_store(&self.workspace, None)?;
        let mut svc = self
            .projection
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        svc.reload(&atomic);
        Ok(())
    }
}

/// Render a warm-projection validation result as a plain-text summary.
fn render_projection_validation(v: &ProjectionValidation) -> String {
    let status = |ok: bool| if ok { "ok" } else { "VIOLATIONS" };
    format!(
        "warm projection validate (fine-grained Salsa engine, RocksDB-free)\n\
         section_decision: {} (violations={})\n\
         frozen_membership: {} (violations={})\n\
         overall: {} (total violations={})",
        status(v.section_decision.ok),
        v.section_decision.violation_count,
        status(v.frozen_membership.ok),
        v.frozen_membership.violation_count,
        status(v.ok()),
        v.total_violations(),
    )
}

/// Parse `<alternative> -- <reason>` / `<alternative> — <reason>` bullets
/// into structured rejected-alternative rows. Mirrors the CLI's
/// `parse_alternatives_file`.
fn parse_alternatives(bullets: &[String]) -> Result<Vec<RejectedAlternative>, String> {
    let mut out = Vec::new();
    for (i, raw) in bullets.iter().enumerate() {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parsed = RejectedAlternative::parse_line(trimmed).ok_or_else(|| {
            format!(
                "alternative[{}]: expected `<alternative> -- <reason>` (or ` — ` separator)",
                i
            )
        })?;
        out.push(parsed);
    }
    Ok(out)
}

fn parse_inventory_status(raw: &str) -> Result<InventoryStatus, String> {
    raw.parse::<InventoryStatus>()
        .map_err(|e| format!("status {}", e))
}

/// Map the MCP fact-shaped args onto the atomic `FactImport` (Round 435).
/// All registry/shape invariants live in the shared `build_candidate_fact`
/// path; the flattened typed-object pair resolves through the ONE shared
/// constructor (`TypedObject::from_exclusive_args`, Round 448) — pure
/// shape translation here.
fn fact_import_from(a: &AddFactArgs) -> Result<atomic::FactImport, String> {
    let typed = match &a.typed {
        None => None,
        Some(t) => Some(mnemosyne_core::TypedClaim {
            subject: t.subject.clone(),
            predicate: t.predicate.clone(),
            object: mnemosyne_core::TypedObject::from_exclusive_args(
                t.object_entity.clone(),
                t.object_value.clone(),
            )?,
        }),
    };
    Ok(atomic::FactImport {
        fact_id: a.fact_id.clone(),
        frame: a.frame.clone(),
        branch: a.branch.clone(),
        claim: a.claim.clone(),
        canon_from: a.canon_from.clone(),
        canon_to: a.canon_to.clone(),
        evidence: a.evidence.clone(),
        conflicts_with: a.conflicts_with.clone(),
        supersedes_in_frame: a.supersedes_in_frame.clone(),
        payoff_expectation: a.payoff_expectation.clone(),
        pays_off: a.pays_off.clone(),
        typed,
        quote: a.quote.clone(),
        entities: a.entities.clone(),
    })
}

#[tool_router]
impl MnemosyneServer {
    #[tool(
        description = "Run T1 (prose cross-ref orphan) + T2 (frozen ledger) + T3/T4 style validation store-direct over the atomic store (the SSOT). Returns the metric summary (orphan total / T3 warn / T4 info / atomic orphan refs). Call at session start for the baseline and after every mutation."
    )]
    async fn validate_workspace(&self, _args: Parameters<EmptyArgs>) -> CallToolResult {
        match ops::validate_workspace(&self.workspace) {
            Ok(report) => Self::tool_text(report.render_plain()),
            Err(e) => self.op_error(e),
        }
    }

    #[tool(
        description = "Validate Layer-0 cascade invariants (Section supersession refs + FrozenList membership) via the warm incremental read model. Auto-resyncs after every successful mutate; pass refresh=true only to pick up an out-of-band log change (manual JSON edit or separate CLI mutate). `validate_workspace` is the authoritative cold validator."
    )]
    async fn validate_projection(
        &self,
        args: Parameters<ValidateProjectionArgs>,
    ) -> CallToolResult {
        let report = {
            let mut svc = self
                .projection
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if args.0.refresh {
                match ops::load_atomic_store(&self.workspace, None) {
                    Ok(atomic) => svc.reload(&atomic),
                    Err(e) => return self.op_error(e),
                }
            }
            render_projection_validation(&svc.validate())
        };
        Self::tool_text(report)
    }

    #[tool(
        description = "List every section_id in the workspace (one per line, BTreeMap order). Use this to discover the section topology before authoring §N references."
    )]
    async fn list_sections(&self, _args: Parameters<EmptyArgs>) -> CallToolResult {
        match ops::list_sections(&self.workspace) {
            Ok(report) => {
                let mut out = report.section_ids.join("\n");
                out.push_str(&format!("\n# total {} section(s)", report.total));
                Self::tool_text(out)
            }
            Err(e) => self.op_error(e),
        }
    }

    #[tool(
        description = "List the changelog ledger as JSON {total, entries}, in round-number order (oldest first). `limit` keeps only the newest N entries while `total` reports the full ledger size — pass a small limit for the session-start 'where did the last session leave off' read instead of pulling the whole ledger into context. Per-section history is query_section with include_changelog."
    )]
    async fn list_changelog(&self, args: Parameters<ListChangelogArgs>) -> CallToolResult {
        match ops::list_changelog(&self.workspace, args.0.limit) {
            Ok(view) => self.tool_json(&view),
            Err(e) => self.op_error(e),
        }
    }

    #[tool(
        description = "Look up a single section. Returns the SectionView (atomic fields rendered as JSON). Optionally include 1-hop CrossRef neighborhood and §N citations from changelog entries. Always call this BEFORE mutating a section to verify decision_status and avoid editing strong-carry / Superseded sections."
    )]
    async fn query_section(&self, args: Parameters<QuerySectionArgs>) -> CallToolResult {
        let mode = match (args.0.include_related, args.0.include_changelog) {
            (true, true) => QuerySectionMode::Envelope,
            (true, false) | (false, true) => QuerySectionMode::WithRelated,
            (false, false) => QuerySectionMode::Brief,
        };
        match ops::query_section(&self.workspace, &args.0.section_id, mode) {
            Ok(payload) => self.tool_json(&payload),
            Err(e) => self.op_error(e),
        }
    }

    #[tool(
        description = "Literal/regex search across atomic Section + ChangelogEntry + Inventory text fields, including identifier keys (section_id / entry_id / inventory_id). Returns hits as JSON: target_kind (section|changelog_entry|inventory), target_id, field_path (e.g. `rationale_bullets[2]`), line_context. Read-only. Use before redact_term or before mutating prose, to know which entries cite a term."
    )]
    async fn query_term(&self, args: Parameters<QueryTermArgs>) -> CallToolResult {
        let input = QueryTermInput {
            pattern: args.0.pattern.clone(),
            regex: args.0.regex,
            case_insensitive: args.0.case_insensitive,
            scope: args.0.scope.clone(),
            fields: args.0.fields.clone(),
        };
        match ops::query_term(&self.workspace, &input) {
            Ok(hits) => self.tool_json(&hits),
            Err(e) => self.op_error(e),
        }
    }

    #[tool(
        description = "Run T3/T4 style checks. T3 = warning surface (max_paragraph_length, sentence length, terminology); T4 = info. Reject power is configurable; default = warn-only so existing prose stays valid on day 1."
    )]
    async fn style_check(&self, args: Parameters<StyleCheckArgs>) -> CallToolResult {
        let input = StyleCheckInput {
            doc: args.0.doc.clone(),
            severity: args.0.severity.clone(),
        };
        match ops::style_check(&self.workspace, &input) {
            Ok(report) => self.tool_json(&report),
            Err(e) => self.op_error(e),
        }
    }

    #[tool(
        description = "Create a new Section (outline fields only): `section_id` (no `§` prefix), `parent_doc`, `title`, optional `parent_section`. Content fields (intent, rationale, etc.) populate via subsequent set_section_* / add_section_* calls. Rejects duplicate `section_id` and missing `parent_section`."
    )]
    async fn add_section(&self, args: Parameters<AddSectionArgs>) -> CallToolResult {
        let section = strip_section_marker(&args.0.section_id).to_string();
        let parent_doc = args.0.parent_doc.clone();
        let title = args.0.title.clone();
        let parent = args
            .0
            .parent_section
            .as_deref()
            .map(|p| strip_section_marker(p).to_string());
        let outcome = self.run_mutate(|store, path| {
            atomic::add_section(
                store,
                path,
                &section,
                &parent_doc,
                &title,
                parent.as_deref(),
            )
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Set Section.title (heading text). Section must exist (use add_section to create first)."
    )]
    async fn set_section_title(&self, args: Parameters<SetSectionTextArgs>) -> CallToolResult {
        let section = strip_section_marker(&args.0.section_id).to_string();
        let title = args.0.text.clone();
        let outcome =
            self.run_mutate(|store, path| atomic::set_section_title(store, path, &section, &title));
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Set Section.parent_doc (re-bind section to a different owning doc). Section must exist."
    )]
    async fn set_section_parent_doc(&self, args: Parameters<SetSectionTextArgs>) -> CallToolResult {
        let section = strip_section_marker(&args.0.section_id).to_string();
        let parent_doc = args.0.text.clone();
        let outcome = self.run_mutate(|store, path| {
            atomic::set_section_parent_doc(store, path, &section, &parent_doc)
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Set Section.parent_section (re-parent in hierarchy). Pass `parent_section: Some(\"<id>\")` to nest under another section, or omit / pass null to promote to top-level. Self-loop rejected; missing parent rejected."
    )]
    async fn set_section_parent_section(
        &self,
        args: Parameters<SetSectionParentSectionArgs>,
    ) -> CallToolResult {
        let section = strip_section_marker(&args.0.section_id).to_string();
        let parent = args
            .0
            .parent_section
            .as_deref()
            .map(|p| strip_section_marker(p).to_string());
        let outcome = self.run_mutate(|store, path| {
            atomic::set_section_parent_section(store, path, &section, parent.as_deref())
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Set Section.intent atomic field. The intent is a one-sentence statement of what the section is for. Replaces any previous intent. T1+T2 run pre-write."
    )]
    async fn set_section_intent(&self, args: Parameters<SetSectionTextArgs>) -> CallToolResult {
        let section = strip_section_marker(&args.0.section_id).to_string();
        let intent = args.0.text.clone();
        let outcome = self
            .run_mutate(|store, path| atomic::set_section_intent(store, path, &section, &intent));
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Set Section.rationale_bullets. Replaces existing. Each bullet ≤ 100 chars (T3 default)."
    )]
    async fn set_section_rationale(
        &self,
        args: Parameters<SetSectionBulletsArgs>,
    ) -> CallToolResult {
        let section = strip_section_marker(&args.0.section_id).to_string();
        let bullets = args.0.bullets.clone();
        let outcome = self.run_mutate(|store, path| {
            atomic::set_section_rationale(store, path, &section, &bullets)
        });
        self.finish_mutate(outcome)
    }

    #[tool(description = "Set Section.inputs_bullets. Replaces existing.")]
    async fn set_section_inputs(&self, args: Parameters<SetSectionBulletsArgs>) -> CallToolResult {
        let section = strip_section_marker(&args.0.section_id).to_string();
        let bullets = args.0.bullets.clone();
        let outcome = self
            .run_mutate(|store, path| atomic::set_section_inputs(store, path, &section, &bullets));
        self.finish_mutate(outcome)
    }

    #[tool(description = "Set Section.outputs_bullets. Replaces existing.")]
    async fn set_section_outputs(&self, args: Parameters<SetSectionBulletsArgs>) -> CallToolResult {
        let section = strip_section_marker(&args.0.section_id).to_string();
        let bullets = args.0.bullets.clone();
        let outcome = self
            .run_mutate(|store, path| atomic::set_section_outputs(store, path, &section, &bullets));
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Append a single caveat bullet to Section.caveats_bullets. Append-only — does not replace existing caveats."
    )]
    async fn add_section_caveat(&self, args: Parameters<AddSectionCaveatArgs>) -> CallToolResult {
        let section = strip_section_marker(&args.0.section_id).to_string();
        let bullet = args.0.bullet.clone();
        let outcome = self
            .run_mutate(|store, path| atomic::add_section_caveat(store, path, &section, &bullet));
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Set Section.alternatives_rejected. Replaces existing. Each bullet is `<alternative> -- <reason>`."
    )]
    async fn set_section_alternatives(
        &self,
        args: Parameters<SetSectionBulletsArgs>,
    ) -> CallToolResult {
        let section = strip_section_marker(&args.0.section_id).to_string();
        let alternatives = match parse_alternatives(&args.0.bullets) {
            Ok(a) => a,
            Err(e) => return Self::tool_error(e),
        };
        let outcome = self.run_mutate(|store, path| {
            atomic::set_section_alternatives(store, path, &section, &alternatives)
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Set Section.impact_scope. Each ref is a section_id without the `§` prefix; T1 cross-ref orphan reject runs pre-write so non-existent §N targets fail cleanly."
    )]
    async fn set_section_impact_scope(
        &self,
        args: Parameters<SetImpactScopeArgs>,
    ) -> CallToolResult {
        let section = strip_section_marker(&args.0.section_id).to_string();
        let refs: Vec<String> = args
            .0
            .refs
            .iter()
            .map(|r| strip_section_marker(r).to_string())
            .collect();
        let outcome = self.run_mutate(|store, path| {
            atomic::set_section_impact_scope(store, path, &section, &refs)
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Append a code-fenced example to Section.examples. The code block is rendered with the supplied language tag."
    )]
    async fn add_section_example(&self, args: Parameters<AddSectionExampleArgs>) -> CallToolResult {
        let section = strip_section_marker(&args.0.section_id).to_string();
        let example = ExampleBlock {
            language: args.0.language.clone(),
            code: args.0.code.clone(),
        };
        let outcome = self
            .run_mutate(|store, path| atomic::add_section_example(store, path, &section, example));
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Append a typed (file, symbol?, kind) trace-link binding to Section.bindings. file = workspace-relative POSIX path (no leading `/`, `..`, or `\\`); symbol = optional opaque identifier (function/type/qualified path). kind = `implements` (the symbol fulfills the section's requirement — the only kind counted as coverage) or `references` (related, no fulfillment claim). Duplicate (file, symbol) rejected regardless of kind (use set_section_binding_kind to change kind). File existence not checked here (validate-code-refs does that)."
    )]
    async fn add_section_binding(&self, args: Parameters<AddSectionBindingArgs>) -> CallToolResult {
        let section = strip_section_marker(&args.0.section_id).to_string();
        let file = args.0.file.clone();
        let symbol = args.0.symbol.clone();
        let kind_raw = args.0.kind.clone();
        let outcome = self.run_mutate(|store, path| {
            let kind = atomic::BindingKind::from_tag(kind_raw.trim()).ok_or_else(|| {
                atomic::AtomicMutateError::Validation(format!(
                    "kind must be `implements`, `references`, or `verifies` (got `{}`)",
                    kind_raw
                ))
            })?;
            atomic::add_section_binding(store, path, &section, &file, symbol.as_deref(), kind)
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Remove one `(file, symbol?)` binding from Section.bindings (matches the identity pair regardless of kind). Pass `symbol` to target a symbol-narrowed row, omit it for a file-only row. NotFound when section or binding is absent (no silent no-op). `reason` mandatory — recorded on the receipt. Use to clean stale bindings that validate-code-refs flags as binding_unbacked (don't edit the sidecar JSON directly)."
    )]
    async fn remove_section_binding(
        &self,
        args: Parameters<RemoveSectionBindingArgs>,
    ) -> CallToolResult {
        let section = strip_section_marker(&args.0.section_id).to_string();
        let file = args.0.file.clone();
        let symbol = args.0.symbol.clone();
        let reason = args.0.reason.clone();
        let outcome = self.run_mutate(|store, path| {
            atomic::remove_section_binding(store, path, &section, &file, symbol.as_deref(), &reason)
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Reclassify an existing binding's kind (`implements` ⇄ `references`). Identity is the `(file, symbol?)` pair; the binding must already exist (NotFound otherwise — no silent create). `reason` mandatory. Second write path to Binding.kind alongside add_section_binding; both enforce the same closed kind set."
    )]
    async fn set_section_binding_kind(
        &self,
        args: Parameters<SetSectionBindingKindArgs>,
    ) -> CallToolResult {
        let section = strip_section_marker(&args.0.section_id).to_string();
        let file = args.0.file.clone();
        let symbol = args.0.symbol.clone();
        let kind_raw = args.0.kind.clone();
        let reason = args.0.reason.clone();
        let outcome = self.run_mutate(|store, path| {
            let kind = atomic::BindingKind::from_tag(kind_raw.trim()).ok_or_else(|| {
                atomic::AtomicMutateError::Validation(format!(
                    "kind must be `implements`, `references`, or `verifies` (got `{}`)",
                    kind_raw
                ))
            })?;
            atomic::set_section_binding_kind(
                store,
                path,
                &section,
                &file,
                symbol.as_deref(),
                kind,
                &reason,
            )
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Classify a section's coverage applicability (R421 3-state). `normative` (default) keeps the coverage axiom — a non-removed normative section with zero `implements` bindings is a gap. `out_of_scope_here` (part of the standard but not implemented by this consumer; revisitable) and `informational` (inherently non-implementable prose — terminology / overview) both EXEMPT the section. Second write path to Section.coverage_expectation alongside import_sections; both enforce the same closed value set. `reason` mandatory."
    )]
    async fn set_section_coverage_expectation(
        &self,
        args: Parameters<SetSectionCoverageExpectationArgs>,
    ) -> CallToolResult {
        let section = strip_section_marker(&args.0.section_id).to_string();
        let expectation_raw = args.0.expectation.clone();
        let reason = args.0.reason.clone();
        let outcome = self.run_mutate(|store, path| {
            let expectation = atomic::CoverageExpectation::from_tag(expectation_raw.trim())
                .ok_or_else(|| {
                    atomic::AtomicMutateError::Validation(format!(
                        "expectation must be `normative` or `informative` (got `{}`)",
                        expectation_raw
                    ))
                })?;
            atomic::set_section_coverage_expectation(store, path, &section, expectation, &reason)
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Classify a section's verification expectation (R413). `dedicated` (default) keeps the verify gate — when the verify axis is enabled, a normative + dedicated section with zero `verifies` bindings is a VerificationMissing gap. `by_construction` exempts the section (no independently-assertable per-unit oracle — e.g. transcribed algorithm pseudocode exercised holistically). Orthogonal to coverage_expectation: a by_construction section stays normative for implements-coverage. `reason` mandatory."
    )]
    async fn set_section_verification_expectation(
        &self,
        args: Parameters<SetSectionVerificationExpectationArgs>,
    ) -> CallToolResult {
        let section = strip_section_marker(&args.0.section_id).to_string();
        let expectation_raw = args.0.expectation.clone();
        let reason = args.0.reason.clone();
        let outcome = self.run_mutate(|store, path| {
            let expectation = atomic::VerificationExpectation::from_tag(expectation_raw.trim())
                .ok_or_else(|| {
                    atomic::AtomicMutateError::Validation(format!(
                        "expectation must be `dedicated` or `by_construction` (got `{}`)",
                        expectation_raw
                    ))
                })?;
            atomic::set_section_verification_expectation(
                store,
                path,
                &section,
                expectation,
                &reason,
            )
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Append a confirmation event (R416/R417) — an append-only record that a claim (a `verifies` binding, or a section all-I/O completeness claim) was independently re-verified. The event_id is derived in-core. Enforces self-confirm reject (confirming_run must differ from authoring_run) and R287 fail-loud (the claim section must exist). The core records provenance only; it neither verifies the artifact hashes nor spawns a confirmer. Set `file` for a VerifiesBinding claim, omit for SectionCompleteness. Enum fields take the snake_case tag."
    )]
    async fn add_confirmation_event(
        &self,
        args: Parameters<AddConfirmationEventArgs>,
    ) -> CallToolResult {
        let a = args.0;
        let section = strip_section_marker(&a.section_id).to_string();
        let outcome = self.run_mutate(|store, path| {
            let claim = match &a.file {
                Some(f) => atomic::ConfirmationClaim::VerifiesBinding {
                    section_id: section.clone(),
                    file: f.clone(),
                    symbol: a.symbol.clone(),
                },
                None => {
                    if a.symbol.is_some() {
                        return Err(atomic::AtomicMutateError::Validation(
                            "symbol requires file (a VerifiesBinding claim)".to_string(),
                        ));
                    }
                    atomic::ConfirmationClaim::SectionCompleteness {
                        section_id: section.clone(),
                    }
                }
            };
            let kind =
                atomic::ConfirmerKind::from_tag(a.confirmer_kind.trim()).ok_or_else(|| {
                    atomic::AtomicMutateError::Validation(format!(
                        "confirmer_kind must be `tool` or `model` (got `{}`)",
                        a.confirmer_kind
                    ))
                })?;
            let method = atomic::ConfirmMethod::from_tag(a.method.trim()).ok_or_else(|| {
                atomic::AtomicMutateError::Validation(format!(
                    "method must be linkage_check|semantic_review|coverage_attestation (got `{}`)",
                    a.method
                ))
            })?;
            let verdict = atomic::Verdict::from_tag(a.verdict.trim()).ok_or_else(|| {
                atomic::AtomicMutateError::Validation(format!(
                    "verdict must be `confirm` or `refute` (got `{}`)",
                    a.verdict
                ))
            })?;
            let event = atomic::ConfirmationEvent {
                claim,
                confirmer: atomic::Confirmer {
                    kind,
                    id: a.confirmer_id.clone(),
                    version: a.confirmer_version.clone(),
                },
                method,
                artifact_hashes: atomic::ArtifactHashes {
                    spec_sha256: a.spec_sha256.clone(),
                    code_sha256: a.code_sha256.clone(),
                    test_sha256: a.test_sha256.clone(),
                },
                authoring_run: a.authoring_run.clone(),
                confirming_run: a.confirming_run.clone(),
                verdict,
                rationale: a.rationale.clone(),
                timestamp: a.timestamp.clone(),
            };
            atomic::append_confirmation_event(store, path, event)
        });
        self.finish_mutate(outcome)
    }

    // ── Round 435 — narrative authoring verbs (design sec 7.10 pull 3) ──

    #[tool(
        description = "Register one epistemic frame (R430) — the axis a narrative fact's `frame` must reference. Idempotent on a byte-identical description; a divergent description rejects (no silent overwrite)."
    )]
    async fn add_frame(&self, args: Parameters<AddFrameArgs>) -> CallToolResult {
        let a = args.0;
        let outcome = self
            .run_mutate(|store, path| atomic::add_frame(store, path, &a.frame_id, &a.description));
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Register one world-line branch (R436) — the registry key every non-default fact branch must reference (fail-loud at the write path; `main` never registers). Idempotent on a byte-identical description; divergent rejects."
    )]
    async fn add_branch(&self, args: Parameters<AddBranchArgs>) -> CallToolResult {
        let a = args.0;
        let outcome = self.run_mutate(|store, path| {
            let fork = match (&a.forks_from, &a.forks_at) {
                (None, None) => None,
                (Some(p), Some(at)) => Some((p.as_str(), at.as_str())),
                _ => {
                    return Err(atomic::AtomicMutateError::Validation(
                        "forks_from and forks_at must be given together".to_string(),
                    ));
                }
            };
            atomic::add_branch(store, path, &a.branch_id, &a.description, fork)
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Register one narrative entity (R437) — the retrieval key for entity-scoped verification (a character's background, a location's lore). Fact `entities` refs must name a registered id (fail-loud). Idempotent on identical content; divergent rejects."
    )]
    async fn add_entity(&self, args: Parameters<AddEntityArgs>) -> CallToolResult {
        let a = args.0;
        let outcome = self.run_mutate(|store, path| {
            atomic::add_entity(store, path, &a.entity_id, &a.kind, &a.description)
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Register one predicate (R446) — the 4th registry: TypedClaim predicates are load-bearing refs (narrative rules key off them), so a typo must fail loud, never silently escape its rule. object_kind declares the object leg's shape (entity | scalar); the fact builder enforces it. Idempotent on identical content; divergent rejects."
    )]
    async fn add_predicate(&self, args: Parameters<AddPredicateArgs>) -> CallToolResult {
        let a = args.0;
        let outcome = self.run_mutate(|store, path| {
            atomic::add_predicate(store, path, &a.predicate_id, &a.object_kind, &a.description)
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Register one disclosure (discourse) plan (R506) — a named telling over the fact base: a default_mode (withhold | state | hint | imply, default withhold = the sparse-frame ethos) the per-fact overrides sit on. Many plans over one base = many tellings (Dark-Souls-fragment / classic-mystery / expository-thriller). Idempotent on identical policy; a changed description/default_mode rejects (set_disclosure edits the overrides)."
    )]
    async fn add_disclosure_plan(&self, args: Parameters<AddDisclosurePlanArgs>) -> CallToolResult {
        let a = args.0;
        let outcome = self.run_mutate(|store, path| {
            atomic::add_disclosure_plan(store, path, &a.telling_id, &a.default_mode, &a.description)
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Set one per-fact disclosure override within a telling (R506): mode (withhold | state | hint | imply), per-world-line first_at timing (branch -> section), and an optional diegetic surface (scene + entity). A setter (last-write-wins). Fail-loud refs: telling + fact must exist, first_at branches/coords + surface scene must resolve, surface object must be a registered entity. THE gate-enabling invariant: a withhold mode OR any first_at pin requires the fact to carry a typed claim — the premature-leak render-acceptance gate matches re-extracted prose to the plan by typed tuple, so an untyped target is un-gateable."
    )]
    async fn set_disclosure(&self, args: Parameters<SetDisclosureArgs>) -> CallToolResult {
        let a = args.0;
        let first_at: Vec<(String, String)> = a
            .first_at
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let surface = a
            .surface_scene
            .as_deref()
            .map(|scene| (scene, a.surface_object.as_deref()));
        let outcome = self.run_mutate(|store, path| {
            atomic::set_disclosure(
                store,
                path,
                &a.telling_id,
                &a.fact_id,
                &a.mode,
                &first_at,
                surface,
            )
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Entity dossier (R437, read-only): every fact referencing the entity across all frames and branches — 'all facts about X' for background-vs-narrative verification. The at-a-point projection is report_frame_view with the entity filter."
    )]
    async fn report_entity(&self, args: Parameters<ReportEntityArgs>) -> CallToolResult {
        match ops::entity_dossier(&self.workspace, None, &args.0.entity_id) {
            Ok(d) => self.tool_json(&d),
            Err(e) => self.op_error(e),
        }
    }

    #[tool(
        description = "Create one narrative fact (R430): a claim held in exactly one epistemic frame on one world-line branch over a canon extent, evidenced by structure sections. Frame must be registered; a non-default branch must be registered (add_branch); canon/evidence refs must be sections; divergent re-add rejects — in-world belief change = supersedes_in_frame, authorial correction = amend_fact / retract_fact."
    )]
    async fn add_fact(&self, args: Parameters<AddFactArgs>) -> CallToolResult {
        let outcome = self.run_mutate(|store, path| {
            let entry = fact_import_from(&args.0).map_err(atomic::AtomicMutateError::Validation)?;
            atomic::add_fact(store, path, &entry)
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Record one conflict assertion edge between two existing facts (R430). Contradiction is a recorded semantic judgment, never derived from claim text; the continuity gate evaluates it (frame, branch)-scoped — cross-scope edges are data, never gated."
    )]
    async fn add_fact_conflict(&self, args: Parameters<AddFactConflictArgs>) -> CallToolResult {
        let a = args.0;
        let outcome = self.run_mutate(|store, path| {
            atomic::add_fact_conflict(store, path, &a.fact_id, &a.conflicts_with)
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Authorial in-place revision of an existing fact, keeping its id (R434, axis-4 correction: a typo or wrong coordinate; in-world belief change is supersedes_in_frame instead). Same invariants as add_fact; inbound successors must stay same-(frame, branch). Mandatory reason."
    )]
    async fn amend_fact(&self, args: Parameters<AmendFactArgs>) -> CallToolResult {
        let reason = args.0.reason.clone();
        let outcome = self.run_mutate(|store, path| {
            let entry =
                fact_import_from(&args.0.fact).map_err(atomic::AtomicMutateError::Validation)?;
            atomic::amend_fact(store, path, &entry, &reason)
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Authorial retract of an unreferenced fact (R434). Any inbound conflict edge / succession pointer blocks it fail-loud with the referrer list; the retraction's transaction-time audit is the git history of the log. Mandatory reason."
    )]
    async fn retract_fact(&self, args: Parameters<RetractFactArgs>) -> CallToolResult {
        let a = args.0;
        let outcome =
            self.run_mutate(|store, path| atomic::retract_fact(store, path, &a.fact_id, &a.reason));
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Frame-scoped continuity scan (R431, read-only): same-(frame, branch) conflicting pairs whose derived canon extents co-hold are violations; cross-scope pairs are data. With a declared narrative-rules/v1 artifact (R449) it also derives typed-claim rule findings — exclusive (one co-holding value per subject / one holder per object), transition (allowed state steps on succession edges), and interval (R489: a scalar/arithmetic relation value(left) − value(right) op bound per frame-world-subject) — plus the unchained_state_pairs and interval_unverifiable honesty counts. Interval violations ride a SEPARATE per-class severity (R491, interval_severity, OFF by default — a timeline gap can be an intentional time-bend); structural violations ride severity. Returns the JSON report (both severities, interval_violation_count, counts, violations); gating policy belongs to the caller."
    )]
    async fn validate_continuity(
        &self,
        args: Parameters<ValidateContinuityArgs>,
    ) -> CallToolResult {
        match ops::continuity_scan(
            &self.workspace,
            None,
            args.0.order_path.as_deref(),
            args.0.rules_path.as_deref(),
        ) {
            Ok(report) => self.tool_json(&report),
            Err(e) => self.op_error(e),
        }
    }

    #[tool(
        description = "Frame-at-T read projection (R432): the facts frame F holds on branch B at canon point T, over the SAME holds-semantics as the continuity gate. Three-state honest under the declared partial order: holding / not_holding count / unknown (the declaration cannot decide). Call before writing the next scene to load the in-effect beliefs."
    )]
    async fn report_frame_view(&self, args: Parameters<ReportFrameViewArgs>) -> CallToolResult {
        match ops::continuity_frame_view(
            &self.workspace,
            None,
            &args.0.frame,
            args.0.branch.as_deref(),
            args.0.entity.as_deref(),
            &args.0.at,
            args.0.order_path.as_deref(),
        ) {
            Ok(view) => self.tool_json(&view),
            Err(e) => self.op_error(e),
        }
    }

    #[tool(
        description = "Setup/payoff coverage (R442, read-only): per query world, every setup fact (payoff_expectation=expected) classified paid/dangling against world-visible pays_off edges; unmarked facts exempt. Dangling = the author's todo list, never gated. Honesty counts: payoffs_to_unmarked, payoff_before_setup, unknown."
    )]
    async fn report_payoff_coverage(
        &self,
        args: Parameters<ReportPayoffCoverageArgs>,
    ) -> CallToolResult {
        match ops::payoff_coverage_report(&self.workspace, None, args.0.order_path.as_deref()) {
            Ok(report) => self.tool_json(&report),
            Err(e) => self.op_error(e),
        }
    }

    #[tool(
        description = "Typing-discovery input package (R458, read-only): every untyped narrative fact (claim text + claim_sha256 pin + frame/branch/entities) plus the registered predicate and entity vocabulary, in one call. The contract for typing-proposals/v1 authoring: propose typed legs ONLY from this vocabulary, stamp each proposal with the candidate's claim_sha256. Order-independent."
    )]
    async fn report_typing_candidates(&self, _args: Parameters<EmptyArgs>) -> CallToolResult {
        match ops::typing_candidates_report(&self.workspace, None) {
            Ok(report) => self.tool_json(&report),
            Err(e) => self.op_error(e),
        }
    }

    #[tool(
        description = "Import reviewed typed-leg proposals from a typing-proposals/v1 artifact (R459, mutate): ALL-OR-NOTHING with full per-proposal verdicts (fill-blanks only, claim_sha256 staleness re-checked, predicates/entities validated by the one builder). dry_run=true validates without writing. Returns the verdict report; applied=true only when every proposal accepted on a real run."
    )]
    async fn import_typing_proposals(
        &self,
        args: Parameters<ImportTypingProposalsArgs>,
    ) -> CallToolResult {
        // Verdict-report mutate: same single lock site as every other
        // mutate (Round 460 — with_mutate_lock), report-shaped return.
        match self.with_mutate_lock(|| {
            ops::import_typing_proposals_report(
                &self.workspace,
                None,
                std::path::Path::new(&args.0.proposals_path),
                args.0.dry_run,
            )
        }) {
            Ok(report) => {
                if report.applied {
                    if let Err(e) = self.sync_read_models_after_mutate() {
                        return self.op_error(e);
                    }
                }
                self.tool_json(&report)
            }
            Err(e) => self.op_error(e),
        }
    }

    #[tool(
        description = "Deterministic payoff substantiation (R485, read-only): per query world, each credited setup is classified substantiated (a payoff carries a typed state-change on the setup's same subject+predicate, discharging it) / unsubstantiated (typed setup, no discharging payoff — a hollow payoff, the deterministic analogue of drift) / unverifiable (the setup is untyped, so no discharge is definable — type it via typing-discovery). No LLM: a pure comparison of declared typed legs. Replaces the retired R481 LLM-verdict drift surface (R484 redesign)."
    )]
    async fn report_payoff_substantiation(
        &self,
        args: Parameters<ReportPayoffCoverageArgs>,
    ) -> CallToolResult {
        match ops::payoff_substantiation_report(&self.workspace, None, args.0.order_path.as_deref())
        {
            Ok(report) => self.tool_json(&report),
            Err(e) => self.op_error(e),
        }
    }

    #[tool(
        description = "Timeline-gap projection (R490, read-only, surface-not-gate): the interval-rule evaluator as a READ report. Per query world, each declared interval rule (value(left) - value(right) op bound, op ge/le/eq/gt/lt, bound a const or a same-subject scalar predicate) is evaluated at the left fact's canon point, classified violated / unverifiable (an operand non-numeric, absent on the right/bound leg, or ambiguous — type it) / satisfied. Same narrative-rules artifact as the continuity gate, only interval rules contribute. Deterministic, no LLM; never gates (the gate is validate_continuity under opt-in severity)."
    )]
    async fn report_timeline_gaps(
        &self,
        args: Parameters<ReportTimelineGapsArgs>,
    ) -> CallToolResult {
        match ops::timeline_gaps_report(
            &self.workspace,
            None,
            args.0.order_path.as_deref(),
            args.0.rules_path.as_deref(),
        ) {
            Ok(report) => self.tool_json(&report),
            Err(e) => self.op_error(e),
        }
    }

    #[tool(
        description = "Edge-discovery input package (R462, read-only): every fact row (claim text + claim_sha256 pin + frame/branch/entities + ALL recorded edges) plus deterministic succession-gap hints (same-frame same typed predicate+subject pairs no succession path connects). The contract for edge-proposals/v1 authoring: propose succession/conflict edges between listed facts only, stamp BOTH endpoint claim_sha256 pins, never re-propose a recorded edge."
    )]
    async fn report_edge_candidates(
        &self,
        args: Parameters<ReportEdgeCandidatesArgs>,
    ) -> CallToolResult {
        match ops::edge_candidates_report(&self.workspace, None, args.0.order_path.as_deref()) {
            Ok(report) => self.tool_json(&report),
            Err(e) => self.op_error(e),
        }
    }

    #[tool(
        description = "Import reviewed succession/conflict edge proposals from an edge-proposals/v1 artifact (R463, mutate): ALL-OR-NOTHING with full per-proposal verdicts (fill-blanks only, BOTH endpoint claim_sha256 pins re-checked, in-frame/fork-lineage/cycle invariants ride the shared succession check). dry_run=true validates without writing. applied=true only when every proposal accepted on a real run."
    )]
    async fn import_edge_proposals(
        &self,
        args: Parameters<ImportEdgeProposalsArgs>,
    ) -> CallToolResult {
        // Verdict-report mutate: same single lock site as every other
        // mutate (Round 460 — with_mutate_lock), report-shaped return.
        match self.with_mutate_lock(|| {
            ops::import_edge_proposals_report(
                &self.workspace,
                None,
                std::path::Path::new(&args.0.proposals_path),
                args.0.dry_run,
            )
        }) {
            Ok(report) => {
                if report.applied {
                    if let Err(e) = self.sync_read_models_after_mutate() {
                        return self.op_error(e);
                    }
                }
                self.tool_json(&report)
            }
            Err(e) => self.op_error(e),
        }
    }

    #[tool(
        description = "Dramatic-irony intervals (R455, read-only): per query world, every recorded CROSS-FRAME conflict edge classified as a co-hold window (node set where both ends hold under the one holds-semantics, with starts + open-at-world-line-end flag), windowless, unordered (incomparable starts, R456), or undecidable (B-1). Same-frame edges are the continuity gate's territory (counted, skipped). Craft signal, never gated."
    )]
    async fn report_irony_intervals(
        &self,
        args: Parameters<ReportIronyIntervalsArgs>,
    ) -> CallToolResult {
        match ops::irony_intervals_report(&self.workspace, None, args.0.order_path.as_deref()) {
            Ok(report) => self.tool_json(&report),
            Err(e) => self.op_error(e),
        }
    }

    #[tool(
        description = "Playthrough manuscript (R466, read-only): per query world (or the single `world` filter), the composed canon order's deterministic topological walk with declared fact events placed on each scene — begins, ends (expired / superseded-by), holds-judged holding_count, skeleton title + EPUB locator. Honesty surfaces: undeclared_adjacencies (incomparable emitted neighbors — one valid reading, never the only one), unplaced_facts, undecidable (B-1), sections_outside_order. Reading surface, never gated."
    )]
    async fn report_playthrough_manuscript(
        &self,
        args: Parameters<ReportPlaythroughManuscriptArgs>,
    ) -> CallToolResult {
        match ops::playthrough_manuscript_report(
            &self.workspace,
            None,
            args.0.world.as_deref(),
            args.0.order_path.as_deref(),
            args.0.telling.as_deref(),
        ) {
            Ok(report) => self.tool_json(&report),
            Err(e) => self.op_error(e),
        }
    }

    #[tool(
        description = "Fork tree (R497, read-only): the cross-world choice graph — every registered world-line with its divergence coordinate (parent + fork point + the branch description = the CYOA choice label), the fork point resolved against the parent's composed order (at_placed; false = surfaced in unplaced_fork_points, never dropped). The per-world manuscripts (R466) stitched at the fork points. Fail-loud on a fork whose parent is neither `main` nor registered. Reading surface, never gated."
    )]
    async fn report_fork_tree(&self, args: Parameters<ReportForkTreeArgs>) -> CallToolResult {
        match ops::fork_tree_report(&self.workspace, None, args.0.order_path.as_deref()) {
            Ok(report) => self.tool_json(&report),
            Err(e) => self.op_error(e),
        }
    }

    #[tool(
        description = "Append a new ChangelogEntry to the atomic store. entry_id must start with the configured schema.entry_id_prefix (Round 424 conformance gate; pick the next monotonic id by convention). All five atomic fields are required."
    )]
    async fn append_changelog_entry(
        &self,
        args: Parameters<AppendChangelogEntryArgs>,
    ) -> CallToolResult {
        let entry_id = args.0.entry_id.clone();
        let decision = args.0.decision_summary.clone();
        let changes = args.0.changes_bullets.clone();
        let verify = args.0.verification_bullets.clone();
        let impact: Vec<String> = args
            .0
            .impact_refs
            .iter()
            .map(|r| strip_section_marker(r).to_string())
            .collect();
        let carry = args.0.carry_forward_bullets.clone();
        // Round 424 — append conformance gate policy, resolved through the
        // single shared path (CLI + MCP parity).
        let entry_id_prefix = match ops::workspace_entry_id_prefix(&self.workspace) {
            Ok(p) => p,
            Err(e) => return self.op_error(e),
        };
        let outcome = self.run_mutate(|store, path| {
            atomic::append_changelog_entry(
                store,
                path,
                ChangelogEntryDraft {
                    entry_id: &entry_id,
                    decision_summary: Some(&decision),
                    changes_bullets: &changes,
                    verification_bullets: &verify,
                    impact_refs: &impact,
                    carry_forward_bullets: &carry,
                },
                &entry_id_prefix,
            )
        });
        self.finish_mutate(outcome)
    }

    // Round 299 — publishable-half setters + redact_term MCP wire. The
    // audit half stays write-once via append_changelog_entry; every tool
    // below only mutates publishable_* and must be paired with a
    // [[publishable_override_ledger]] row (R296 gate). redact_term emits
    // the ledger drafts inline; the four bare setters require the caller
    // to author the row separately.

    #[tool(
        description = "Replace the publishable_decision_summary of an existing entry. Mutates the publishable half only — the audit half stays frozen. Pair with a [[publishable_override_ledger]] row, or use redact_term for an automated ledger draft. NotFound if entry_id has not been appended."
    )]
    async fn set_changelog_publishable_decision_summary(
        &self,
        args: Parameters<SetChangelogPublishableStringArgs>,
    ) -> CallToolResult {
        let entry_id = args.0.entry_id.clone();
        let value = args.0.value.clone();
        let outcome = self.run_mutate(|store, path| {
            atomic::set_changelog_publishable_decision_summary(store, path, &entry_id, &value)
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Replace the publishable_changes_bullets of an existing entry. Publishable half only — audit half stays frozen. Pair with a [[publishable_override_ledger]] row, or use redact_term for an automated ledger draft."
    )]
    async fn set_changelog_publishable_changes(
        &self,
        args: Parameters<SetChangelogPublishableBulletsArgs>,
    ) -> CallToolResult {
        let entry_id = args.0.entry_id.clone();
        let bullets = args.0.bullets.clone();
        let outcome = self.run_mutate(|store, path| {
            atomic::set_changelog_publishable_changes_bullets(store, path, &entry_id, &bullets)
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Replace the publishable_verification_bullets of an existing entry. Publishable half only — audit half stays frozen. Pair with a [[publishable_override_ledger]] row, or use redact_term for an automated ledger draft."
    )]
    async fn set_changelog_publishable_verification(
        &self,
        args: Parameters<SetChangelogPublishableBulletsArgs>,
    ) -> CallToolResult {
        let entry_id = args.0.entry_id.clone();
        let bullets = args.0.bullets.clone();
        let outcome = self.run_mutate(|store, path| {
            atomic::set_changelog_publishable_verification_bullets(store, path, &entry_id, &bullets)
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Replace the publishable_impact_refs of an existing entry (bare section ids, no `§`). Publishable half only — audit half stays frozen. Pair with a [[publishable_override_ledger]] row, or use redact_term for an automated ledger draft."
    )]
    async fn set_changelog_publishable_impact_refs(
        &self,
        args: Parameters<SetChangelogPublishableBulletsArgs>,
    ) -> CallToolResult {
        let entry_id = args.0.entry_id.clone();
        let bullets: Vec<String> = args
            .0
            .bullets
            .iter()
            .map(|r| strip_section_marker(r).to_string())
            .collect();
        let outcome = self.run_mutate(|store, path| {
            atomic::set_changelog_publishable_impact_refs(store, path, &entry_id, &bullets)
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Replace the publishable_carry_forward_bullets of an existing entry. Publishable half only — audit half stays frozen. Pair with a [[publishable_override_ledger]] row, or use redact_term for an automated ledger draft."
    )]
    async fn set_changelog_publishable_carry_forward(
        &self,
        args: Parameters<SetChangelogPublishableBulletsArgs>,
    ) -> CallToolResult {
        let entry_id = args.0.entry_id.clone();
        let bullets = args.0.bullets.clone();
        let outcome = self.run_mutate(|store, path| {
            atomic::set_changelog_publishable_carry_forward_bullets(
                store, path, &entry_id, &bullets,
            )
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Scan the publishable half of every ChangelogEntry for `pattern` and substitute `replacement`, emitting ledger drafts so the publishable_override_ledger gate accepts the result. Audit half is never read or written. mode = literal (default) or regex; set case_insensitive for either. scope = all | decision_summary | changes_bullets | verification_bullets | impact_refs | carry_forward_bullets. dry_run = true returns hits + drafts without mutating. reason + applied_in required; kind defaults to \"redaction\". Drafts paste directly into mnemosyne.toml `[[publishable_override_ledger]]`."
    )]
    async fn redact_term(&self, args: Parameters<RedactTermArgs>) -> CallToolResult {
        let input = RedactTermInput {
            pattern: args.0.pattern.clone(),
            replacement: args.0.replacement.clone(),
            regex: args.0.regex,
            case_insensitive: args.0.case_insensitive,
            scope: args.0.scope.clone(),
            dry_run: args.0.dry_run,
            reason: args.0.reason.clone(),
            applied_in: args.0.applied_in.clone(),
            kind: args.0.kind.clone(),
        };
        match ops::redact_term(&self.workspace, None, false, &input) {
            Ok((report, _)) => {
                // A non-dry-run redaction mutated the store, so re-sync the warm
                // validation projection from the just-written log (fail-loud).
                if !report.dry_run {
                    if let Err(e) = self.sync_read_models_after_mutate() {
                        return self.op_error(e);
                    }
                }
                let payload = serde_json::json!({
                    "primitive": "redact_term",
                    "dry_run": report.dry_run,
                    "hits": report
                        .hits
                        .iter()
                        .map(|h| serde_json::json!({
                            "entry_id": h.entry_id,
                            "field": h.field,
                            "index": h.index,
                            "original": h.original,
                            "redacted": h.redacted,
                        }))
                        .collect::<Vec<_>>(),
                    "ledger_drafts": report.ledger_drafts,
                });
                self.tool_json(&payload)
            }
            Err(e) => self.op_error(e),
        }
    }

    #[tool(
        description = "Read-only: render a `[[publishable_override_ledger]]` block for an entry whose publishable half diverges from the audit half, computing the SHA256 anchor against the current publishable state so the row clears the gate. Returns `in_sync: true` / `ledger_draft: null` when nothing has diverged. NotFound if entry_id is absent. Use after mutating via the bare publishable setters when you need a draft to paste."
    )]
    async fn emit_publishable_override_ledger_draft(
        &self,
        args: Parameters<EmitPublishableOverrideLedgerDraftArgs>,
    ) -> CallToolResult {
        match ops::emit_publishable_override_ledger_draft(
            &self.workspace,
            None,
            &args.0.entry_id,
            &args.0.reason,
            &args.0.applied_in,
            args.0.kind.as_deref(),
        ) {
            Ok(draft) => self.tool_json(&serde_json::json!({
                "entry_id": args.0.entry_id,
                "in_sync": draft.is_none(),
                "ledger_draft": draft,
            })),
            Err(e) => self.op_error(e),
        }
    }

    // Round 278 — Phase 1A inventory tool surface.

    #[tool(
        description = "List every inventory entry in the atomic store (id, status, section_ref), in id order."
    )]
    async fn list_inventory(&self, _args: Parameters<EmptyArgs>) -> CallToolResult {
        match ops::list_inventory(&self.workspace) {
            Ok(entries) => self.tool_json(&entries),
            Err(e) => self.op_error(e),
        }
    }

    #[tool(
        description = "Look up a single inventory entry (status / section_ref / source / reason). Call this BEFORE writing an inventory citation in code to verify status (Deprecated → don't cite)."
    )]
    async fn query_inventory(&self, args: Parameters<InventoryIdArgs>) -> CallToolResult {
        match ops::query_inventory(&self.workspace, &args.0.inventory_id) {
            Ok(view) => self.tool_json(&view),
            Err(e) => self.op_error(e),
        }
    }

    #[tool(
        description = "Register a new inventory entry. Duplicate inventory_id rejects. status = active|deprecated|reserved. Registering as deprecated surfaces any pre-existing cite-sites via the mutate-time cascade. section_ref omits the leading §."
    )]
    async fn add_inventory_entry(&self, args: Parameters<AddInventoryEntryArgs>) -> CallToolResult {
        let inventory_id = args.0.inventory_id.clone();
        let status = match parse_inventory_status(&args.0.status) {
            Ok(s) => s,
            Err(e) => return Self::tool_error(e),
        };
        let section_ref = args
            .0
            .section_ref
            .as_deref()
            .map(|s| strip_section_marker(s).to_string());
        let source = args.0.source.clone();
        let reason = args.0.reason.clone();
        let outcome = self.run_mutate(|store, path| {
            atomic::add_inventory_entry(
                store,
                path,
                &inventory_id,
                status,
                section_ref.as_deref(),
                source.as_deref(),
                reason.as_deref(),
            )
        });
        self.finish_inventory_mutate(
            outcome,
            &inventory_id,
            status == InventoryStatus::Deprecated,
        )
    }

    #[tool(
        description = "Update an inventory entry's status. Returns NotFound if the id is not registered. reason: omit to preserve existing; pass empty string to clear; pass non-empty to overwrite. Active→Deprecated transitions invoke the cascade scan."
    )]
    async fn set_inventory_status(
        &self,
        args: Parameters<SetInventoryStatusArgs>,
    ) -> CallToolResult {
        let inventory_id = args.0.inventory_id.clone();
        let status = match parse_inventory_status(&args.0.status) {
            Ok(s) => s,
            Err(e) => return Self::tool_error(e),
        };
        let reason = args.0.reason.clone();
        let outcome = self.run_mutate(|store, path| {
            atomic::set_inventory_status(store, path, &inventory_id, status, reason.as_deref())
        });
        self.finish_inventory_mutate(
            outcome,
            &inventory_id,
            status == InventoryStatus::Deprecated,
        )
    }

    #[tool(
        description = "Update an inventory entry's section_ref binding. Exactly one of section_ref or clear must be supplied. section_ref omits the leading §. NotFound on unregistered ids."
    )]
    async fn set_inventory_section_ref(
        &self,
        args: Parameters<SetInventorySectionRefArgs>,
    ) -> CallToolResult {
        let cleaned: Option<String> = match (&args.0.section_ref, args.0.clear) {
            (Some(s), false) => Some(strip_section_marker(s).to_string()),
            (None, true) => None,
            _ => {
                return Self::tool_error(
                    "exactly one of section_ref or clear must be supplied".to_string(),
                );
            }
        };
        let inventory_id = args.0.inventory_id.clone();
        let outcome = self.run_mutate(|store, path| {
            atomic::set_inventory_section_ref(store, path, &inventory_id, cleaned.as_deref())
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Remove an inventory entry. reason is mandatory (audit safeguard recorded in the receipt). Triggers the cascade scan so any pre-existing cite-sites surface mutate-time as `removed` cascade lines."
    )]
    async fn remove_inventory_entry(
        &self,
        args: Parameters<RemoveInventoryEntryArgs>,
    ) -> CallToolResult {
        let inventory_id = args.0.inventory_id.clone();
        let reason = args.0.reason.clone();
        let outcome = self.run_mutate(|store, path| {
            atomic::remove_inventory_entry(store, path, &inventory_id, &reason)
        });
        self.finish_inventory_mutate(outcome, &inventory_id, true)
    }
}

impl MnemosyneServer {
    /// Finish an inventory mutate that may trigger the R276 decay cascade.
    /// On success, when `run_cascade` is set (Deprecated transition or
    /// removal), scan for now-stale cite-sites and append them to the
    /// JSON payload (parity with the CLI's stderr cascade lines).
    fn finish_inventory_mutate(
        &self,
        outcome: Result<MutateOutcome, OpError>,
        inventory_id: &str,
        run_cascade: bool,
    ) -> CallToolResult {
        match outcome {
            Ok(o) => {
                if let Err(e) = self.sync_read_models_after_mutate() {
                    return self.op_error(e);
                }
                let decay = if run_cascade {
                    match ops::inventory_decay_scan(&self.workspace, inventory_id) {
                        Ok(hits) => hits
                            .into_iter()
                            .map(|c| {
                                serde_json::json!({
                                    "file": c.file.display().to_string(),
                                    "line": c.line,
                                    "entry_id": c.entry_id,
                                })
                            })
                            .collect::<Vec<_>>(),
                        // The mutate already persisted; surface the scan
                        // failure explicitly rather than a misleading empty
                        // decay set (fail-loud without falsely failing the
                        // mutate).
                        Err(e) => {
                            return self.tool_json(&serde_json::json!({
                                "receipt": o.receipt,
                                "cascade_decay_error": format!("{:#}", e),
                            }));
                        }
                    }
                } else {
                    Vec::new()
                };
                self.tool_json(&serde_json::json!({
                    "receipt": o.receipt,
                    "cascade_decay_hits": decay,
                }))
            }
            Err(e) => self.op_error(e),
        }
    }
}

#[tool_handler]
impl ServerHandler for MnemosyneServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
        )
        .with_instructions(concat!(
            "Mnemosyne MCP server. Read mnemosyne://concepts/overview first, ",
            "then anti-patterns + atomic-store + frozen-ledger before any mutation. ",
            "Run validate_workspace to surface the baseline, mutate via typed primitives, ",
            "validate_workspace again to confirm no new T1/T2 violations. ",
            "NEVER edit the atomic store JSON directly — mutate via the typed primitives."
        ))
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        let resources = resources::RESOURCES
            .iter()
            .map(|r| {
                let raw = RawResource::new(r.uri, r.name)
                    .with_title(r.title)
                    .with_description(r.description)
                    .with_mime_type("text/markdown");
                Annotated::new(raw, None)
            })
            .collect();
        Ok(ListResourcesResult {
            resources,
            ..Default::default()
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        match resources::lookup(&request.uri) {
            Some(r) => Ok(ReadResourceResult::new(vec![ResourceContents::text(
                r.body, r.uri,
            )
            .with_mime_type("text/markdown")])),
            None => Err(McpError::resource_not_found(
                "unknown resource uri",
                Some(serde_json::json!({"uri": request.uri})),
            )),
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .with_target(false)
        .init();

    let workspace = parse_workspace_arg()?;
    if !workspace.exists() {
        anyhow::bail!("workspace path does not exist: {}", workspace.display());
    }

    let server = MnemosyneServer::new(workspace)?;
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

fn parse_workspace_arg() -> anyhow::Result<PathBuf> {
    let mut args = std::env::args().skip(1);
    let mut workspace: Option<PathBuf> = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--workspace" | "-w" => {
                workspace = Some(PathBuf::from(args.next().ok_or_else(|| {
                    anyhow::anyhow!("--workspace requires a path argument")
                })?));
            }
            "--help" | "-h" => {
                eprintln!(
                    "mnemosyne-mcp {} ({}) — MCP server for Mnemosyne\n\n\
                     usage: mnemosyne-mcp [--workspace <path>]\n\n\
                     Communicates over stdio. Mutate + query run in-process\n\
                     against the mnemosyne-cli library (no subprocess spawn).\n\
                     If --workspace is omitted, the current directory is used.",
                    env!("CARGO_PKG_VERSION"),
                    env!("BUILD_GIT_HASH"),
                );
                std::process::exit(0);
            }
            "--version" | "-V" => {
                // Round 286 — universal CLI surface. Mirror mnemosyne-cli
                // format. stdout (not stderr) so wrapper scripts can pipe.
                println!(
                    "mnemosyne-mcp {} ({})",
                    env!("CARGO_PKG_VERSION"),
                    env!("BUILD_GIT_HASH")
                );
                std::process::exit(0);
            }
            other => {
                anyhow::bail!("unknown argument: {}", other);
            }
        }
    }
    Ok(workspace.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))))
}
