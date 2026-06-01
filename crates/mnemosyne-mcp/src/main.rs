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
use mnemosyne_projection::{ProjectionService, ProjectionValidation, RenderProjectionService};
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
pub struct RenderProjectionArgs {
    /// Force a re-sync from the current log before rendering. Since R367 Step 2b
    /// the warm render projection re-syncs after every successful mutate (and
    /// owns the GENERATED.md write), so the default (false) already reflects the
    /// current log. Pass true only to pick up an out-of-band edit (a manual JSON
    /// edit or a separate CLI mutate that did not go through this host).
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
pub struct SetSectionNormativeExcerptArgs {
    /// Section ID without the `§` prefix.
    pub section_id: String,
    /// Vendored normative quote, verbatim. Trailing newline is trimmed
    /// for round-trip stability; leading whitespace preserved.
    pub text: String,
    /// Absolute http(s):// anchor URL pointing at the upstream
    /// section (e.g. `https://www.w3.org/TR/scxml/#event`).
    pub anchor_url: String,
    /// Upstream revision identifier the excerpt was captured at
    /// (Recommendation publication date, editor's-draft date, RFC
    /// number + revision letter, etc.). Free-form string; should
    /// match `[workspace.spec_source].revision` at the time of
    /// anchoring, but per-Section field carries independently for
    /// partially-migrated workspaces.
    pub source_revision: String,
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
    /// Warm read-side render projection (convergence C/D, R345 / R365 Step 2a,
    /// R367 Step 2b). Built from the same log at startup; serves
    /// `render_projection` from the warm `RenderDb` Salsa memo cache,
    /// byte-identical to the cold `generate-docs`. Since 2b it also owns the
    /// write path: every successful mutate incrementally reconciles this
    /// projection (Round 340 analogue — only changed units re-render), recomposes
    /// GENERATED.md, and writes it, superseding the cold `auto_regenerate` in the
    /// warm host (see [`Self::sync_read_models_after_mutate`]).
    render: Arc<Mutex<RenderProjectionService>>,
    #[allow(dead_code)] // populated by #[tool_router] expansion
    tool_router: ToolRouter<Self>,
}

impl MnemosyneServer {
    pub fn new(workspace: PathBuf) -> Result<Self, ops::OpError> {
        let atomic = ops::load_atomic_store(&workspace, None)?;
        let projection = ProjectionService::build(&atomic, atomic::MAIN_BRANCH_ID);
        // `Source:` line value, computed exactly as the cold render does
        // (sidecar path relative to the workspace root).
        let sidecar = ops::cascade::resolve_sidecar(&workspace, None)?;
        let source_rel =
            sidecar
                .display()
                .to_string()
                .replacen(&format!("{}/", workspace.display()), "", 1);
        let render = RenderProjectionService::build(&atomic, &source_rel);
        Ok(Self {
            workspace: Arc::new(workspace),
            projection: Arc::new(Mutex::new(projection)),
            render: Arc::new(Mutex::new(render)),
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

    /// Finish a mutate op: re-sync the warm read models from the just-written
    /// log (recompose + write GENERATED.md through the warm render projection,
    /// then re-sync the warm validation projection), then receipt JSON. A
    /// regenerate/write failure is surfaced as a tool error (the store mutate
    /// already persisted, so a failed regenerate signals an inconsistent cascade
    /// that needs manual intervention — same contract the cold `auto_regenerate`
    /// had). `regenerated` is set true since the warm host owns the regenerate.
    fn finish_mutate(&self, outcome: Result<MutateOutcome, OpError>) -> CallToolResult {
        match outcome {
            Ok(mut o) => {
                if let Err(e) = self.sync_read_models_after_mutate() {
                    return self.op_error(e);
                }
                o.regenerated = true;
                self.tool_json(&o)
            }
            Err(e) => self.op_error(e),
        }
    }

    /// Re-sync both warm read models from the just-written log after a
    /// successful mutate (R367 Step 2b — the warm host owns regeneration, so the
    /// MCP mutate tools run the primitive with `regenerate=false`; the cold
    /// CLI/CI keeps `auto_regenerate`).
    ///
    /// 1. **Render projection (fail-loud):** incrementally reconcile the warm
    ///    `RenderDb` to the new log (Round 340 analogue — only changed units
    ///    re-render), recompose `GENERATED.md` through the single-source builder
    ///    (byte-identical to the cold `generate-docs`), and atomic-write it. A
    ///    failure here is the cascade-output contract the cold `auto_regenerate`
    ///    used to own, so it propagates.
    /// 2. **Validation projection (best-effort cache):** incrementally reconcile
    ///    the warm `FineCascadeDb` from the same in-memory snapshot so the
    ///    default `validate_projection` reflects the current log. This operates
    ///    on the already-loaded store, so it cannot fail.
    ///
    /// The store is loaded once and shared by both. Poisoned locks are recovered
    /// (the projections are rebuildable caches, not authoritative state).
    fn sync_read_models_after_mutate(&self) -> Result<(), OpError> {
        let atomic = ops::load_atomic_store(&self.workspace, None)?;

        // 1. Render projection: reconcile → recompose → write GENERATED.md.
        {
            let mut svc = self
                .render
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            svc.reload(&atomic);
            let content = svc.render();
            let output_path = ops::resolve_output(&self.workspace, None)?;
            ops::write_generated_md(&output_path, &content)
                .map_err(|e| OpError::Other(format!("{:#}", e)))?;
        }

        // 2. Validation projection: best-effort incremental re-sync.
        {
            let mut svc = self
                .projection
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            svc.reload(&atomic);
        }

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

#[tool_router]
impl MnemosyneServer {
    #[tool(
        description = "Run T1 (cross-ref orphan) + T2 (frozen ledger) + round-trip validation across the entire workspace. Returns the metric summary (orphan total / round-trip mandatory / T3 warn / T4 info). Call at session start for the baseline and after every mutation."
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
        description = "Render docs/GENERATED.md via the warm render model and return the markdown (read-only; does NOT write). Byte-identical to generate_docs. Auto-resyncs after every successful mutate; pass refresh=true only to pick up an out-of-band edit (manual JSON edit or separate CLI mutate)."
    )]
    async fn render_projection(&self, args: Parameters<RenderProjectionArgs>) -> CallToolResult {
        let rendered = {
            let mut svc = self
                .render
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if args.0.refresh {
                match ops::load_atomic_store(&self.workspace, None) {
                    Ok(atomic) => svc.reload(&atomic),
                    Err(e) => return self.op_error(e),
                }
            }
            svc.render()
        };
        Self::tool_text(rendered)
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
        description = "Render the atomic store to docs/GENERATED.md (template render → atomic write temp + rename). Cascade auto-update normally invokes this after every successful mutate primitive; call directly only when you need to force-refresh after a manual JSON edit (which you should not do)."
    )]
    async fn generate_docs(&self, _args: Parameters<EmptyArgs>) -> CallToolResult {
        match ops::generate_docs(&self.workspace, None, None) {
            Ok(report) => self.tool_json(&report),
            Err(e) => self.op_error(e),
        }
    }

    #[tool(
        description = "Verify that docs/GENERATED.md byte-equals what would be rendered fresh from the atomic store. Returns in_sync = true/false. Wire into pre-commit hooks to catch drift."
    )]
    async fn verify_generated(&self, _args: Parameters<EmptyArgs>) -> CallToolResult {
        match ops::verify_generated(&self.workspace, None, None) {
            Ok(report) if report.in_sync => self.tool_json(&report),
            Ok(report) => Self::tool_error(format!(
                "STALE — GENERATED.md does not match atomic sidecar\n{}",
                serde_json::to_string_pretty(&report).unwrap_or_default()
            )),
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
        let outcome = run_atomic_mutate(&self.workspace, None, false, |store, path| {
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
        let outcome = run_atomic_mutate(&self.workspace, None, false, |store, path| {
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
        let outcome = run_atomic_mutate(&self.workspace, None, false, |store, path| {
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
        let outcome = run_atomic_mutate(&self.workspace, None, false, |store, path| {
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
        let outcome = run_atomic_mutate(&self.workspace, None, false, |store, path| {
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
        let outcome = run_atomic_mutate(&self.workspace, None, false, |store, path| {
            atomic::set_section_rationale(store, path, &section, &bullets)
        });
        self.finish_mutate(outcome)
    }

    #[tool(description = "Set Section.inputs_bullets. Replaces existing.")]
    async fn set_section_inputs(&self, args: Parameters<SetSectionBulletsArgs>) -> CallToolResult {
        let section = strip_section_marker(&args.0.section_id).to_string();
        let bullets = args.0.bullets.clone();
        let outcome = run_atomic_mutate(&self.workspace, None, false, |store, path| {
            atomic::set_section_inputs(store, path, &section, &bullets)
        });
        self.finish_mutate(outcome)
    }

    #[tool(description = "Set Section.outputs_bullets. Replaces existing.")]
    async fn set_section_outputs(&self, args: Parameters<SetSectionBulletsArgs>) -> CallToolResult {
        let section = strip_section_marker(&args.0.section_id).to_string();
        let bullets = args.0.bullets.clone();
        let outcome = run_atomic_mutate(&self.workspace, None, false, |store, path| {
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
        let outcome = run_atomic_mutate(&self.workspace, None, false, |store, path| {
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
        let outcome = run_atomic_mutate(&self.workspace, None, false, |store, path| {
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
        let outcome = run_atomic_mutate(&self.workspace, None, false, |store, path| {
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
        let outcome = run_atomic_mutate(&self.workspace, None, false, |store, path| {
            atomic::add_section_example(store, path, &section, example)
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "External-spec mirror — anchor a vendored normative excerpt (text + anchor URL + source revision) onto a Section. Use when this Section represents a section of an external standard (W3C / IETF RFC / IEEE / AUTOSAR / etc.) mirrored into the workspace. **Frozen-ledger field**: once set, the excerpt is immutable; spec revision drift is modeled by transitioning this Section to `decision_status = Superseded` and creating a new Section that carries the updated excerpt. `anchor_url` must be an absolute http(s):// URL with a host."
    )]
    async fn set_section_normative_excerpt(
        &self,
        args: Parameters<SetSectionNormativeExcerptArgs>,
    ) -> CallToolResult {
        let section = strip_section_marker(&args.0.section_id).to_string();
        let text = args.0.text.clone();
        let anchor_url = args.0.anchor_url.clone();
        let source_revision = args.0.source_revision.clone();
        let outcome = run_atomic_mutate(&self.workspace, None, false, |store, path| {
            atomic::set_section_normative_excerpt(
                store,
                path,
                &section,
                &text,
                &anchor_url,
                &source_revision,
            )
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
        let outcome = run_atomic_mutate(&self.workspace, None, false, |store, path| {
            let kind = atomic::BindingKind::from_tag(kind_raw.trim()).ok_or_else(|| {
                atomic::AtomicMutateError::Validation(format!(
                    "kind must be `implements` or `references` (got `{}`)",
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
        let outcome = run_atomic_mutate(&self.workspace, None, false, |store, path| {
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
        let outcome = run_atomic_mutate(&self.workspace, None, false, |store, path| {
            let kind = atomic::BindingKind::from_tag(kind_raw.trim()).ok_or_else(|| {
                atomic::AtomicMutateError::Validation(format!(
                    "kind must be `implements` or `references` (got `{}`)",
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
        description = "Append a new ChangelogEntry to the atomic store. entry_id must be strictly monotonic (greater than the last entry's id under the configured schema.entry_id_prefix). All five atomic fields are required."
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
        let outcome = run_atomic_mutate(&self.workspace, None, false, |store, path| {
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
        let outcome = run_atomic_mutate(&self.workspace, None, false, |store, path| {
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
        let outcome = run_atomic_mutate(&self.workspace, None, false, |store, path| {
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
        let outcome = run_atomic_mutate(&self.workspace, None, false, |store, path| {
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
        let outcome = run_atomic_mutate(&self.workspace, None, false, |store, path| {
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
        let outcome = run_atomic_mutate(&self.workspace, None, false, |store, path| {
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
                // The warm host owns regeneration (R367 Step 2b): a non-dry-run
                // redaction mutated the store, so recompose + write GENERATED.md
                // through the warm render projection (fail-loud).
                let regenerated = !report.dry_run;
                if regenerated {
                    if let Err(e) = self.sync_read_models_after_mutate() {
                        return self.op_error(e);
                    }
                }
                let payload = serde_json::json!({
                    "primitive": "redact_term",
                    "dry_run": report.dry_run,
                    "regenerated": regenerated,
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
        let outcome = run_atomic_mutate(&self.workspace, None, false, |store, path| {
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
        let outcome = run_atomic_mutate(&self.workspace, None, false, |store, path| {
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
        let outcome = run_atomic_mutate(&self.workspace, None, false, |store, path| {
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
        let outcome = run_atomic_mutate(&self.workspace, None, false, |store, path| {
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
            Ok(mut o) => {
                if let Err(e) = self.sync_read_models_after_mutate() {
                    return self.op_error(e);
                }
                o.regenerated = true;
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
                                "regenerated": o.regenerated,
                                "cascade_decay_error": format!("{:#}", e),
                            }));
                        }
                    }
                } else {
                    Vec::new()
                };
                self.tool_json(&serde_json::json!({
                    "receipt": o.receipt,
                    "regenerated": o.regenerated,
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
            "NEVER edit docs/GENERATED.md or the atomic JSON directly."
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

#[cfg(test)]
mod tests {
    use super::*;
    use mnemosyne_atomic::{AtomicChangelogEntry, AtomicSection, AtomicStore};
    use std::fs;

    /// R367 Step 2b end-to-end: the warm-host write path must produce a
    /// GENERATED.md byte-identical to the cold `render_atomic_store_to_md`. This
    /// exercises the full seam `MnemosyneServer::sync_read_models_after_mutate`
    /// drives on every mutate (load → incremental render reconcile → recompose →
    /// atomic-write), proving the warm path that supersedes `auto_regenerate`
    /// stays in lockstep with the cold renderer the validate gates compare to.
    #[test]
    fn warm_host_write_path_byte_identical_to_cold_render() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = tmp.path().to_path_buf();

        // Persist a non-trivial store to the resolved sidecar — the state a
        // mutate primitive would have just saved (the warm host runs the
        // primitive with regenerate=false, then this write path).
        let sidecar = ops::cascade::resolve_sidecar(&ws, None).unwrap();
        if let Some(parent) = sidecar.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut store = AtomicStore::new();
        store.sections.insert(
            "1".to_string(),
            AtomicSection {
                intent: Some("warm write path".to_string()),
                rationale_bullets: vec!["incremental reconcile then compose".to_string()],
                ..Default::default()
            },
        );
        let mut e = AtomicChangelogEntry {
            decision_summary: Some("Round 367 render Step 2b".to_string()),
            changes_bullets: vec!["wire warm render into the mutate write path".to_string()],
            ..Default::default()
        };
        e.clone_audit_into_publishable();
        store.changelog_entries.insert("Round 367".to_string(), e);
        store.save(&sidecar).unwrap();

        // Warm host built over that store, then the 2b write path.
        let server = MnemosyneServer::new(ws.clone()).unwrap();
        server.sync_read_models_after_mutate().unwrap();

        // GENERATED.md on disk byte-equals a cold render of the same store.
        let output = ops::resolve_output(&ws, None).unwrap();
        let on_disk = fs::read_to_string(&output).unwrap();
        let (cold, _) = ops::render_atomic_store_to_md(&ws, &sidecar).unwrap();
        assert_eq!(
            on_disk, cold,
            "warm-host write path must match cold render byte-for-byte"
        );
        assert!(on_disk.contains("warm write path"));

        // Second cycle: a REAL change through the host, so the incremental
        // reconcile (not a no-op) drives the write. The warm host must pick up
        // the new content AND stay byte-identical to a cold render of the
        // mutated store — proving the host-driven incremental path, not just the
        // write seam.
        store.sections.get_mut("1").unwrap().intent = Some("edited intent v2".to_string());
        store.save(&sidecar).unwrap();
        server.sync_read_models_after_mutate().unwrap();
        let on_disk_2 = fs::read_to_string(&output).unwrap();
        let (cold_2, _) = ops::render_atomic_store_to_md(&ws, &sidecar).unwrap();
        assert_eq!(
            on_disk_2, cold_2,
            "incremental warm write after a change must still match cold render"
        );
        assert!(on_disk_2.contains("edited intent v2"));
        assert!(!on_disk_2.contains("warm write path"));
    }

    /// R367 fail-loud contract: when the GENERATED.md write fails after a
    /// successful store mutate, `sync_read_models_after_mutate` surfaces the
    /// error (it does NOT silently leave a desynced cascade). Injected by making
    /// the resolved output path a directory, so the temp→rename write fails.
    #[test]
    fn warm_host_write_failure_is_fail_loud() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = tmp.path().to_path_buf();

        let sidecar = ops::cascade::resolve_sidecar(&ws, None).unwrap();
        if let Some(parent) = sidecar.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut store = AtomicStore::new();
        store.sections.insert(
            "1".to_string(),
            AtomicSection {
                intent: Some("fail loud".to_string()),
                ..Default::default()
            },
        );
        store.save(&sidecar).unwrap();

        // Make the GENERATED.md output path a directory → the atomic
        // temp+rename write cannot succeed.
        let output = ops::resolve_output(&ws, None).unwrap();
        fs::create_dir_all(&output).unwrap();

        let server = MnemosyneServer::new(ws.clone()).unwrap();
        let result = server.sync_read_models_after_mutate();
        assert!(
            result.is_err(),
            "a GENERATED.md write failure after a mutate must fail loud, not silently desync"
        );
    }
}
