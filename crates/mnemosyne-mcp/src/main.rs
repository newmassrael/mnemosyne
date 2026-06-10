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
    /// `"implementations"`, `"source"`, `"reason"`, etc.
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
            tool_router: Self::tool_router(),
        })
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
/// All invariants live in the shared `build_candidate_fact` path — this is
/// pure shape translation.
fn fact_import_from(a: &AddFactArgs) -> atomic::FactImport {
    atomic::FactImport {
        fact_id: a.fact_id.clone(),
        frame: a.frame.clone(),
        branch: a.branch.clone(),
        claim: a.claim.clone(),
        canon_from: a.canon_from.clone(),
        canon_to: a.canon_to.clone(),
        evidence: a.evidence.clone(),
        conflicts_with: a.conflicts_with.clone(),
        supersedes_in_frame: a.supersedes_in_frame.clone(),
        quote: a.quote.clone(),
        entities: a.entities.clone(),
    }
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
        description = "Literal/regex search across atomic Section + ChangelogEntry + Inventory text fields. Returns hits as JSON: target_kind (section|changelog_entry|inventory), target_id, field_path (e.g. `rationale_bullets[2]`), line_context. Read-only. Use before redact_term or before mutating prose, to know which entries cite a term."
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
        let outcome = run_atomic_mutate(&self.workspace, None, |store, path| {
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
        let outcome = run_atomic_mutate(&self.workspace, None, |store, path| {
            atomic::set_section_title(store, path, &section, &title)
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Set Section.parent_doc (re-bind section to a different owning doc). Section must exist."
    )]
    async fn set_section_parent_doc(&self, args: Parameters<SetSectionTextArgs>) -> CallToolResult {
        let section = strip_section_marker(&args.0.section_id).to_string();
        let parent_doc = args.0.text.clone();
        let outcome = run_atomic_mutate(&self.workspace, None, |store, path| {
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
        let outcome = run_atomic_mutate(&self.workspace, None, |store, path| {
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
        let outcome = run_atomic_mutate(&self.workspace, None, |store, path| {
            atomic::set_section_intent(store, path, &section, &intent)
        });
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
        let outcome = run_atomic_mutate(&self.workspace, None, |store, path| {
            atomic::set_section_rationale(store, path, &section, &bullets)
        });
        self.finish_mutate(outcome)
    }

    #[tool(description = "Set Section.inputs_bullets. Replaces existing.")]
    async fn set_section_inputs(&self, args: Parameters<SetSectionBulletsArgs>) -> CallToolResult {
        let section = strip_section_marker(&args.0.section_id).to_string();
        let bullets = args.0.bullets.clone();
        let outcome = run_atomic_mutate(&self.workspace, None, |store, path| {
            atomic::set_section_inputs(store, path, &section, &bullets)
        });
        self.finish_mutate(outcome)
    }

    #[tool(description = "Set Section.outputs_bullets. Replaces existing.")]
    async fn set_section_outputs(&self, args: Parameters<SetSectionBulletsArgs>) -> CallToolResult {
        let section = strip_section_marker(&args.0.section_id).to_string();
        let bullets = args.0.bullets.clone();
        let outcome = run_atomic_mutate(&self.workspace, None, |store, path| {
            atomic::set_section_outputs(store, path, &section, &bullets)
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Append a single caveat bullet to Section.caveats_bullets. Append-only — does not replace existing caveats."
    )]
    async fn add_section_caveat(&self, args: Parameters<AddSectionCaveatArgs>) -> CallToolResult {
        let section = strip_section_marker(&args.0.section_id).to_string();
        let bullet = args.0.bullet.clone();
        let outcome = run_atomic_mutate(&self.workspace, None, |store, path| {
            atomic::add_section_caveat(store, path, &section, &bullet)
        });
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
        let outcome = run_atomic_mutate(&self.workspace, None, |store, path| {
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
        let outcome = run_atomic_mutate(&self.workspace, None, |store, path| {
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
        let outcome = run_atomic_mutate(&self.workspace, None, |store, path| {
            atomic::add_section_example(store, path, &section, example)
        });
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
        let outcome = run_atomic_mutate(&self.workspace, None, |store, path| {
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
        let outcome = run_atomic_mutate(&self.workspace, None, |store, path| {
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
        let outcome = run_atomic_mutate(&self.workspace, None, |store, path| {
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
        let outcome = run_atomic_mutate(&self.workspace, None, |store, path| {
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
        let outcome = run_atomic_mutate(&self.workspace, None, |store, path| {
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
        let outcome = run_atomic_mutate(&self.workspace, None, |store, path| {
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
        let outcome = run_atomic_mutate(&self.workspace, None, |store, path| {
            atomic::add_frame(store, path, &a.frame_id, &a.description)
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Register one world-line branch (R436) — the registry key every non-default fact branch must reference (fail-loud at the write path; `main` never registers). Idempotent on a byte-identical description; divergent rejects."
    )]
    async fn add_branch(&self, args: Parameters<AddBranchArgs>) -> CallToolResult {
        let a = args.0;
        let outcome = run_atomic_mutate(&self.workspace, None, |store, path| {
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
        let outcome = run_atomic_mutate(&self.workspace, None, |store, path| {
            atomic::add_entity(store, path, &a.entity_id, &a.kind, &a.description)
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
        let entry = fact_import_from(&args.0);
        let outcome = run_atomic_mutate(&self.workspace, None, |store, path| {
            atomic::add_fact(store, path, &entry)
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Record one conflict assertion edge between two existing facts (R430). Contradiction is a recorded semantic judgment, never derived from claim text; the continuity gate evaluates it (frame, branch)-scoped — cross-scope edges are data, never gated."
    )]
    async fn add_fact_conflict(&self, args: Parameters<AddFactConflictArgs>) -> CallToolResult {
        let a = args.0;
        let outcome = run_atomic_mutate(&self.workspace, None, |store, path| {
            atomic::add_fact_conflict(store, path, &a.fact_id, &a.conflicts_with)
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Authorial in-place revision of an existing fact, keeping its id (R434, axis-4 correction: a typo or wrong coordinate; in-world belief change is supersedes_in_frame instead). Same invariants as add_fact; inbound successors must stay same-(frame, branch). Mandatory reason."
    )]
    async fn amend_fact(&self, args: Parameters<AmendFactArgs>) -> CallToolResult {
        let entry = fact_import_from(&args.0.fact);
        let reason = args.0.reason.clone();
        let outcome = run_atomic_mutate(&self.workspace, None, |store, path| {
            atomic::amend_fact(store, path, &entry, &reason)
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Authorial retract of an unreferenced fact (R434). Any inbound conflict edge / succession pointer blocks it fail-loud with the referrer list; the retraction's transaction-time audit is the git history of the log. Mandatory reason."
    )]
    async fn retract_fact(&self, args: Parameters<RetractFactArgs>) -> CallToolResult {
        let a = args.0;
        let outcome = run_atomic_mutate(&self.workspace, None, |store, path| {
            atomic::retract_fact(store, path, &a.fact_id, &a.reason)
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Frame-scoped continuity scan (R431, read-only): same-(frame, branch) conflicting pairs whose derived canon extents co-hold are violations; cross-scope pairs are data. Returns the JSON report (configured severity, counts, violations); gating policy belongs to the caller."
    )]
    async fn validate_continuity(
        &self,
        args: Parameters<ValidateContinuityArgs>,
    ) -> CallToolResult {
        match ops::continuity_scan(&self.workspace, None, args.0.order_path.as_deref()) {
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
        let outcome = run_atomic_mutate(&self.workspace, None, |store, path| {
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
        let outcome = run_atomic_mutate(&self.workspace, None, |store, path| {
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
        let outcome = run_atomic_mutate(&self.workspace, None, |store, path| {
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
        let outcome = run_atomic_mutate(&self.workspace, None, |store, path| {
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
        let outcome = run_atomic_mutate(&self.workspace, None, |store, path| {
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
        let outcome = run_atomic_mutate(&self.workspace, None, |store, path| {
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
        let outcome = run_atomic_mutate(&self.workspace, None, |store, path| {
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
        let outcome = run_atomic_mutate(&self.workspace, None, |store, path| {
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
        let outcome = run_atomic_mutate(&self.workspace, None, |store, path| {
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
        let outcome = run_atomic_mutate(&self.workspace, None, |store, path| {
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
