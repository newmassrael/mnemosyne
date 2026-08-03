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
use mnemosyne_core::{strip_section_marker, IntervalOp, InventoryStatus};
use mnemosyne_ops::{
    self as ops, run_atomic_mutate, MutateOutcome, OpError, QuerySectionMode, QueryTermInput,
    RedactTermInput, StyleCheckInput,
};
use mnemosyne_projection::{ProjectionService, ProjectionValidation};
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{
        CallToolResult, ListResourcesResult, PaginatedRequestParams, ReadResourceRequestParams,
        ReadResourceResult, Resource, ResourceContents, ServerCapabilities, ServerInfo,
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

// Round 638 — the single-entry changelog read + the citation check.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct QueryChangelogEntryArgs {
    /// The citation to verify and read — a round, e.g. `Round 625`. The
    /// exact stored key (with its title) also resolves.
    pub entry_id: String,
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

/// R678 — the section-mutate parity gap the cost-audit found: an MCP agent
/// could add/edit a section but not REMOVE one, nor transition its lifecycle.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RemoveSectionArgs {
    /// Section ID (the `§` prefix is stripped if present).
    pub section_id: String,
    /// Mandatory rationale recorded on the receipt (audit safeguard).
    pub reason: String,
}

/// R678 — the section lifecycle transition (Active/Superseded/Removed/Open).
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetSectionDecisionStatusArgs {
    /// Section ID (the `§` prefix is stripped if present).
    pub section_id: String,
    /// New status: `active` | `superseded` | `removed` | `open`. Unknown rejects.
    pub status: String,
    /// Superseding section id — MANDATORY for `superseded`, rejected otherwise.
    #[serde(default)]
    pub superseding: Option<String>,
    /// Resolving section id — valid only for `open`, rejected otherwise.
    #[serde(default)]
    pub resolving: Option<String>,
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
    /// One of the three `CoverageExpectation` tags. `"normative"` expects an
    /// `implements` binding; `"out_of_scope_here"` (in the document this ledger
    /// mirrors, not built here — including a deferred or Phase-2 feature) and
    /// `"informational"` (inherently non-implementable prose) both exempt the
    /// section. The refusal message names the current set.
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
    /// Incoming world-line merges (R532 — convergence / confluence). Each entry
    /// is a parent + its merge coordinate; a confluence has ≥ 2. Mutually
    /// exclusive with `forks_from`/`forks_at`.
    #[serde(default)]
    pub converges_from: Vec<ConvergeEdgeArg>,
}

/// One incoming-merge edge of a confluence branch (R532): the parent
/// world-line + the parent's merge coordinate (structure-section id).
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ConvergeEdgeArg {
    pub branch: String,
    pub at: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddEntityArgs {
    /// Entity id — the registry key fact `entities` refs must name.
    pub entity_id: String,
    /// Kind — a REF into the entity-kind registry, not free text (R669):
    /// register it first with `add_entity_kind`. Omitted = unspecified
    /// (allowed); a non-empty typo fails loud (it would route the entity out
    /// of every kind-scoped gate — the R436 write-side-typo lesson).
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub description: String,
}

/// R669 — the entity-kind registry, the vocabulary `AddEntityArgs.kind` refs.
/// The missing MCP half: `add_entity` gates on this registry, but without a
/// register verb an MCP-only agent could never declare a kind (the Phase-0
/// AI-first north star). Mirrors `add_predicate` / `add_entity_kind` (CLI).
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddEntityKindArgs {
    /// Entity-kind id — one member of the vocabulary `add_entity`'s `kind`
    /// refs (e.g. character / place / item). Fail-loud, load-bearing.
    pub kind_id: String,
    /// Direct SUPER-kinds (Round 732 DEBT-M as one, Round 738 as a SET —
    /// multiple inheritance / a DAG) — a `thing`-scoped rule then accepts a
    /// `weapon` when `thing` is reachable upward from `weapon`'s parents, and a
    /// `magic-sword` with parents `["weapon","magic-item"]` satisfies BOTH. Each
    /// must already be registered (parent-first) and not the kind itself.
    /// Omitted / empty = a root kind (a flat registry, unchanged).
    #[serde(default)]
    pub parents: Vec<String>,
    #[serde(default)]
    pub description: String,
}

/// R739 — the parent-mutation half of the entity-kind registry: REPLACE an
/// existing kind's direct super-kinds (add_entity_kind only creates). Without it
/// an MCP-only agent could never re-parent a kind (fix a mis-declared parent,
/// add a second super-kind), leaving only a hand-edit or a banned vN id.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetEntityKindParentsArgs {
    /// The EXISTING entity-kind whose super-kinds to replace (fail-loud if absent).
    pub kind_id: String,
    /// The new direct SUPER-kinds (0..N) — a full replace, not a merge. Each must
    /// be registered and not the kind itself; none may be at-or-below `kind_id`
    /// (that would close a cycle). Empty = root the kind.
    #[serde(default)]
    pub parents: Vec<String>,
}

/// R740 — the remove peer of add_entity_kind. Refuses while the kind is still
/// referenced (an Entity.kind, a child kind's parents, a predicate endpoint),
/// so an MCP-only agent can un-declare a mistaken kind without orphaning a ref.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RemoveEntityKindArgs {
    /// The entity-kind to remove (fail-loud if absent or still referenced).
    pub kind_id: String,
}

/// Register one unit of measure (Round 706) — the vocabulary a `quantity`
/// object's `unit` refs. Without it an MCP-only agent could not declare a unit,
/// so no Quantity fact could pass the units-registry gate. Mirrors
/// `add_entity_kind`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddUnitArgs {
    /// Unit id — one member of the measurement vocabulary (e.g. day / minute /
    /// metre). Fail-loud: a Quantity whose unit is unregistered rejects.
    pub unit_id: String,
    #[serde(default)]
    pub description: String,
}

/// Register a numeric PARAMETER (Round 729, DEBT-K) — an accumulating meter
/// (affection / karma / gold). Mirrors `add_unit`. Without it an MCP-only agent
/// could not declare a meter, so no parameter_delta could pass the gate.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddParameterArgs {
    /// Parameter id — one member of the meter vocabulary (e.g. affection).
    pub parameter_id: String,
    #[serde(default)]
    pub description: String,
}

/// Attach a SIGNED per-beat delta to a parameter (Round 729, DEBT-K) — a
/// side-table entry keyed by the beat fact id. The consumer accumulates the
/// running sum; Mnemosyne never does.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddParameterDeltaArgs {
    /// The BEAT FACT ID the delta rides (must already exist).
    pub fact_id: String,
    /// A REGISTERED parameter (add_parameter first).
    pub parameter: String,
    /// The SIGNED delta — non-zero (0 = a no-op beat); both signs legal.
    pub delta: i64,
}

/// Remove one (fact, parameter) delta (Round 729) — the peer of
/// `add_parameter_delta`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RemoveParameterDeltaArgs {
    /// The beat fact id whose delta to drop.
    pub fact_id: String,
    /// The parameter whose delta to drop (fail-loud if the beat has none).
    pub parameter: String,
}

/// Attach a numeric-value THRESHOLD gate to a CHOICE edge (Round 730, DEBT-K) — a
/// side-table entry keyed by the choice fact id. The gate references the meter
/// DIRECTLY (no boolean proxy); the consumer accumulates the meter and compares.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddParameterGateArgs {
    /// The CHOICE FACT ID the gate rides (must already exist; rides ANY fact — no
    /// map-edge check).
    pub fact_id: String,
    /// A REGISTERED parameter (add_parameter first).
    pub parameter: String,
    /// The comparison operator: ge | le | eq | gt | lt.
    pub op: IntervalOp,
    /// The required accumulated value (signed; 0 / negative legal).
    pub threshold: i64,
}

/// Remove a choice's parameter gate (Round 730) — the peer of
/// `add_parameter_gate`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RemoveParameterGateArgs {
    /// The choice fact id whose gate to drop (fail-loud if it has none).
    pub fact_id: String,
}

/// Attach a cost to one map edge (Round 709 → DEBT-J) — a side-table entry keyed
/// by the adjacent fact id, NOT a reified fact (the cost is frame-invariant edge
/// metadata). Mirrors `add_unit`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddEdgeCostArgs {
    /// The ADJACENT FACT ID the cost attaches to (must already exist).
    pub fact_id: String,
    /// The cost amount — a POSITIVE integer (e.g. walk minutes; 0/negative = a
    /// free teleport, rejected by G3).
    pub n: i64,
    /// A REGISTERED unit (e.g. minute; add_unit first).
    pub unit: String,
}

/// Remove a map edge's cost (Round 711) — the peer of `add_edge_cost`. Drops a
/// stray cost off a non-edge fact without retracting the fact.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RemoveEdgeCostArgs {
    /// The fact id whose edge cost to drop (fail-loud if it has none).
    pub fact_id: String,
}

/// Attach a multiset COUNT to a fact (Round 731 → DEBT-L) — a side-table entry
/// keyed by the fact id, value = a positive count. Mirrors `add_edge_cost`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddFactCountArgs {
    /// The FACT ID the count attaches to (must already exist).
    pub fact_id: String,
    /// The multiset count — a POSITIVE integer (holds(A,potion) count 5 = A holds
    /// FIVE potions; 0/negative = not holding it, rejected — retract the fact).
    pub count: i64,
}

/// Remove a fact's multiset count (Round 731) — the peer of `add_fact_count`.
/// Drops a stray count off a fact without retracting the fact.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RemoveFactCountArgs {
    /// The fact id whose multiset count to drop (fail-loud if it has none).
    pub fact_id: String,
}

/// Attach a place-access guard to one map edge (Round 717 design → Round 720).
/// Keyed by the edge fact id, value = the condition fact id; both must exist.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddEdgeGuardArgs {
    /// The ADJACENT (edge) FACT ID the guard attaches to (must already exist).
    pub fact_id: String,
    /// The CONDITION FACT ID the edge requires (must already exist; a dangling-
    /// ref is rejected). Distinct from the edge — an edge cannot guard itself.
    pub condition: String,
}

/// Remove a map edge's whole guard set (Round 720) — the peer of `add_edge_guard`.
/// Drops a stray guard off a non-edge fact without retracting the fact.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RemoveEdgeGuardArgs {
    /// The fact id whose edge guard set to drop (fail-loud if it has none).
    pub fact_id: String,
}

/// Remove ONE condition from a map edge's guard set (Round 722).
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RemoveEdgeGuardConditionArgs {
    /// The edge fact id whose guard set to edit.
    pub fact_id: String,
    /// The condition to drop from the set (fail-loud if absent; the edge key is
    /// deleted when the set empties).
    pub condition: String,
}

/// Set (or clear) a map edge guard's K-of-N threshold (Round 723).
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetEdgeGuardThresholdArgs {
    /// The edge fact id whose guard threshold to set (must already have a guard).
    pub fact_id: String,
    /// The K-of-N threshold: `Some(k)` = at least k of the conditions (1 <= k <=
    /// len; k == len normalizes to AND); `None` (omit) = clear to AND (require all).
    #[serde(default)]
    pub threshold: Option<usize>,
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
    /// Declared object shape. Round 873 typed this slot: the closed set is now
    /// in the tool's JSON Schema, so a client cannot spell a tag the wire
    /// removed and learn about it only from a write-time reject.
    pub object_kind: mnemosyne_core::PredicateObjectKind,
    /// R701 — required entity-KIND for the subject leg (a registered
    /// `entity_kinds` ref; omit = any). The write path rejects a fact whose
    /// subject entity is not this kind (the spatial-map G1 gate).
    #[serde(default)]
    pub subject_kind: Option<String>,
    /// R701 — required entity-KIND for an entity-shaped object leg (omit =
    /// any). Rejects unless `object_kind=entity` (only an entity object has a kind).
    #[serde(default)]
    pub object_entity_kind: Option<String>,
    /// R705 — the CLOSED object vocabulary. REQUIRED (non-empty) under
    /// `object_kind=token`, REJECTED otherwise. Every `TypedObject::Token`
    /// under this predicate must be a member; a token outside it rejects.
    #[serde(default)]
    pub object_tokens: Vec<String>,
    #[serde(default)]
    pub description: String,
}

/// R658 — the repair half of the predicate registry. Full replace (PUT), so
/// `description` is mandatory here unlike `add_predicate`: omitting it on an
/// update path would wipe it silently.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetPredicateArgs {
    /// Predicate id — must ALREADY be registered (`add_predicate` creates).
    pub predicate_id: String,
    /// New declared object shape. A re-type rejects while any existing use still
    /// holds an object of the old shape. Round 873 typed this slot, so the closed
    /// set travels in the tool's JSON Schema.
    pub object_kind: mnemosyne_core::PredicateObjectKind,
    /// R701 — new required subject entity-KIND (omit = clear; full replace).
    /// A tighten rejects while any existing use's subject is off-kind.
    #[serde(default)]
    pub subject_kind: Option<String>,
    /// R701 — new required object entity-KIND (omit = clear; full replace).
    /// A tighten rejects while any existing use's object is off-kind.
    #[serde(default)]
    pub object_entity_kind: Option<String>,
    /// R705 — new closed object vocabulary (full replace). REQUIRED non-empty
    /// under `object_kind=token`. A tighten (dropping a token an existing use
    /// holds) rejects — extend, never silently narrow.
    #[serde(default)]
    pub object_tokens: Vec<String>,
    /// New description. Mandatory — this is a replace, not a merge.
    pub description: String,
}

/// R658 — remove a predicate from the registry.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RemovePredicateArgs {
    /// Predicate id. Rejects while any typed leg still names it.
    pub predicate_id: String,
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

/// One per-world-line first-reveal trigger (Round 752) — a branch plus its
/// trigger-coordinate SET and an optional K-of-N threshold.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DisclosureRevealArg {
    /// World-line this reveal pins timing for (main or a registered branch).
    pub branch: String,
    /// First-reveal trigger coordinate SET (each a section ref) — a non-linear
    /// reader reveals the fact at the EARLIEST coord reached (first-reached).
    pub coords: Vec<String>,
    /// Optional K-of-N threshold: omit = first-reached (k=1); 2..=len selects the
    /// k-th-earliest (len = last-reached).
    #[serde(default)]
    pub threshold: Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetDisclosureArgs {
    /// Telling id (add_disclosure_plan first).
    pub telling_id: String,
    /// Fact id the override targets (must exist; withhold/first_at need it typed).
    pub fact_id: String,
    /// Disclosure mode: withhold | state | hint | imply.
    pub mode: String,
    /// Per-world-line first-reveal triggers (R752): each a branch + a coord SET +
    /// optional threshold; multiple entries for one branch accumulate.
    #[serde(default)]
    pub first_at: Vec<DisclosureRevealArg>,
    /// Optional diegetic surface scene (section ref the disclosure rides on).
    #[serde(default)]
    pub surface_scene: Option<String>,
    /// Optional diegetic surface object (registered entity id).
    #[serde(default)]
    pub surface_object: Option<String>,
}

/// Round 626 — clear one telling's disclosure decision for one fact.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RemoveDisclosureArgs {
    /// Telling id carrying the decision.
    pub telling_id: String,
    /// Fact id whose decision is cleared (the fact itself is untouched).
    pub fact_id: String,
    /// Why the decision is withdrawn (mandatory — audit-trail safeguard).
    pub reason: String,
}

/// Round 752 — add ONE trigger coordinate to a fact's per-world first-reveal SET
/// (the granular peer of `set_disclosure`, mirroring `add_edge_guard`).
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddDisclosureRevealCoordArgs {
    /// Telling id carrying the override (`set_disclosure` first).
    pub telling_id: String,
    /// Fact id whose reveal the coord attaches to (a first_at pin makes it
    /// gate-targeted, so the fact must carry a typed claim).
    pub fact_id: String,
    /// World-line the reveal pins timing for (main or a registered branch).
    pub branch: String,
    /// The trigger coordinate to ADD to the branch's SET (a registered section).
    pub coord: String,
}

/// Round 752 — remove ONE trigger coordinate from a fact's per-world first-reveal
/// SET (the granular peer of `add_disclosure_reveal_coord`).
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RemoveDisclosureRevealCoordArgs {
    /// Telling id carrying the override.
    pub telling_id: String,
    /// Fact id whose reveal the coord is dropped from.
    pub fact_id: String,
    /// World-line the reveal is pinned on.
    pub branch: String,
    /// The trigger coordinate to DROP from the set (fail-loud if absent; the
    /// branch key is deleted when the set empties — never a vacuous empty trigger).
    pub coord: String,
}

/// Round 752 — set (or clear) a fact's per-world first-reveal K-of-N THRESHOLD
/// (the granular peer of `set_edge_guard_threshold`).
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetDisclosureRevealThresholdArgs {
    /// Telling id carrying the override.
    pub telling_id: String,
    /// Fact id whose reveal threshold to set.
    pub fact_id: String,
    /// World-line whose reveal threshold to set (must already have a reveal).
    pub branch: String,
    /// K-of-N threshold: `k` fires at the k-th-earliest trigger (2 <= k <= len;
    /// k == 1 normalizes to first-reached; k == len = last-reached, kept distinct);
    /// omit/null clears back to first-reached.
    #[serde(default)]
    pub threshold: Option<usize>,
}

/// Transactional batch section authoring (Round 687, retyped Round 690 —
/// DEBT-MCP-MANIFEST-SCHEMA). The R687 form took an opaque `manifest_json`
/// String, so the agent got no schema; Round 690 exposes the ONE atomic DTO
/// (`SectionImport`) directly, giving a real JSON Schema from the single source.
/// `import_facts` takes the `FactsManifest` type itself and needs no wrapper;
/// this wrapper exists only because a JSON-RPC tool arg must be an object, not a
/// bare array.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ImportSectionsArgs {
    /// The sections to create in one atomic transaction (the typed `SectionImport`
    /// shape the CLI `import-sections` manifest also reads).
    pub sections: Vec<atomic::SectionImport>,
}

// Round 692 — `add_fact` / `amend_fact` take `atomic::FactImport` directly
// (the ONE fact DTO, JsonSchema via the schemars feature), so the AddFactArgs
// mirror + `fact_import_from` are gone. The typed leg is now the tagged
// `TypedObject` enum ({kind:"entity"|"value"|"token"|"quantity"|"fact", …},
// Round 705/706/707) — stricter than the old object_entity/object_value pair
// (cannot set both or neither) and identical to what `import_facts` already
// exposes (DEBT-… option-1→option-2 sweep). A new variant is auto-exposed here
// via the enum's JsonSchema (the Quantity/Fact variants needed no MCP arg change).

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AmendFactArgs {
    /// The revised fact content (same shape as `add_fact`; `fact_id` names
    /// the existing fact to revise — the id never changes).
    #[serde(flatten)]
    pub fact: atomic::FactImport,
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
    /// PATH RESOLUTION: a relative path is resolved against the WORKSPACE ROOT
    /// (Round 1002), which is the only base an MCP caller can see. An absolute
    /// path is used as given. The CLI resolves its own explicit paths against
    /// the working directory instead (Round 538), because there the caller
    /// chose that directory; here it belongs to whatever host launched the
    /// server.
    /// Canon-order declaration path override (workspace-relative; bypasses
    /// the configured sha256 pin — the R428 rule). Omit to use
    /// `[continuity].canon_order_path`.
    #[serde(default)]
    pub order_path: Option<String>,
    /// PATH RESOLUTION: a relative path is resolved against the WORKSPACE ROOT
    /// (Round 1002), which is the only base an MCP caller can see. An absolute
    /// path is used as given. The CLI resolves its own explicit paths against
    /// the working directory instead (Round 538), because there the caller
    /// chose that directory; here it belongs to whatever host launched the
    /// server.
    /// `narrative-rules/v1` declaration path override (Round 449;
    /// workspace-relative, bypasses the configured sha256 pin). Omit to
    /// use `[continuity].rules_path`.
    #[serde(default)]
    pub rules_path: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ProposeVerdictArgs {
    /// PATH RESOLUTION: a relative path is resolved against the WORKSPACE ROOT
    /// (Round 1002), which is the only base an MCP caller can see. An absolute
    /// path is used as given. The CLI resolves its own explicit paths against
    /// the working directory instead (Round 538), because there the caller
    /// chose that directory; here it belongs to whatever host launched the
    /// server.
    /// Path to the candidate `import-facts` manifest (a JSON object with
    /// `frames`/`branches`/`entities`/`predicates`/`facts`/`disclosure_plans`
    /// arrays). The agent writes the candidate batch to this file, then calls
    /// the tool; the file is only READ (dry run).
    pub manifest_path: String,
    /// PATH RESOLUTION: a relative path is resolved against the WORKSPACE ROOT
    /// (Round 1002), which is the only base an MCP caller can see. An absolute
    /// path is used as given. The CLI resolves its own explicit paths against
    /// the working directory instead (Round 538), because there the caller
    /// chose that directory; here it belongs to whatever host launched the
    /// server.
    /// Canon-order declaration path override (bypasses the pin). Omit to use
    /// `[continuity].canon_order_path`.
    #[serde(default)]
    pub order_path: Option<String>,
    /// PATH RESOLUTION: a relative path is resolved against the WORKSPACE ROOT
    /// (Round 1002), which is the only base an MCP caller can see. An absolute
    /// path is used as given. The CLI resolves its own explicit paths against
    /// the working directory instead (Round 538), because there the caller
    /// chose that directory; here it belongs to whatever host launched the
    /// server.
    /// `narrative-rules/v1` declaration path override. Omit to use
    /// `[continuity].rules_path`.
    #[serde(default)]
    pub rules_path: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReportAuthoringFrontierArgs {
    /// Telling id — enables the quest + disclosure gap sections (unresolved
    /// quests, never-planned disclosures). Omit for the telling-independent
    /// gaps only (zero-fact scenes, per-scene coverage, dangling setups).
    #[serde(default)]
    pub telling: Option<String>,
    /// PATH RESOLUTION: a relative path is resolved against the WORKSPACE ROOT
    /// (Round 1002), which is the only base an MCP caller can see. An absolute
    /// path is used as given. The CLI resolves its own explicit paths against
    /// the working directory instead (Round 538), because there the caller
    /// chose that directory; here it belongs to whatever host launched the
    /// server.
    /// Canon-order declaration path override (bypasses the pin). Omit to use
    /// `[continuity].canon_order_path`.
    #[serde(default)]
    pub order_path: Option<String>,
    /// PATH RESOLUTION: a relative path is resolved against the WORKSPACE ROOT
    /// (Round 1002), which is the only base an MCP caller can see. An absolute
    /// path is used as given. The CLI resolves its own explicit paths against
    /// the working directory instead (Round 538), because there the caller
    /// chose that directory; here it belongs to whatever host launched the
    /// server.
    /// `narrative-rules/v1` declaration path override (Round 891; the
    /// transition rules that declare the map). Omit to use
    /// `[continuity].rules_path`.
    #[serde(default)]
    pub rules_path: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReportTimelineGapsArgs {
    /// PATH RESOLUTION: a relative path is resolved against the WORKSPACE ROOT
    /// (Round 1002), which is the only base an MCP caller can see. An absolute
    /// path is used as given. The CLI resolves its own explicit paths against
    /// the working directory instead (Round 538), because there the caller
    /// chose that directory; here it belongs to whatever host launched the
    /// server.
    /// Canon-order declaration path override (bypasses the pin).
    #[serde(default)]
    pub order_path: Option<String>,
    /// PATH RESOLUTION: a relative path is resolved against the WORKSPACE ROOT
    /// (Round 1002), which is the only base an MCP caller can see. An absolute
    /// path is used as given. The CLI resolves its own explicit paths against
    /// the working directory instead (Round 538), because there the caller
    /// chose that directory; here it belongs to whatever host launched the
    /// server.
    /// `narrative-rules/v1` declaration path override (Round 490; the
    /// interval rules). Omit to use `[continuity].rules_path`.
    #[serde(default)]
    pub rules_path: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReportTransitionMapArgs {
    /// PATH RESOLUTION: a relative path is resolved against the WORKSPACE ROOT
    /// (Round 1002), which is the only base an MCP caller can see. An absolute
    /// path is used as given. The CLI resolves its own explicit paths against
    /// the working directory instead (Round 538), because there the caller
    /// chose that directory; here it belongs to whatever host launched the
    /// server.
    /// `narrative-rules/v1` declaration path override (Round 875; the
    /// transition rules DECLARE which predicate names a map edge). Omit to use
    /// `[continuity].rules_path`. No canon-order override — the map is read
    /// flat, exactly as the gate evaluates it.
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
    /// PATH RESOLUTION: a relative path is resolved against the WORKSPACE ROOT
    /// (Round 1002), which is the only base an MCP caller can see. An absolute
    /// path is used as given. The CLI resolves its own explicit paths against
    /// the working directory instead (Round 538), because there the caller
    /// chose that directory; here it belongs to whatever host launched the
    /// server.
    /// Canon-order declaration path override (bypasses the pin).
    #[serde(default)]
    pub order_path: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReportPayoffCoverageArgs {
    /// PATH RESOLUTION: a relative path is resolved against the WORKSPACE ROOT
    /// (Round 1002), which is the only base an MCP caller can see. An absolute
    /// path is used as given. The CLI resolves its own explicit paths against
    /// the working directory instead (Round 538), because there the caller
    /// chose that directory; here it belongs to whatever host launched the
    /// server.
    /// Canon-order declaration path override (bypasses the pin).
    #[serde(default)]
    pub order_path: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReportIronyIntervalsArgs {
    /// PATH RESOLUTION: a relative path is resolved against the WORKSPACE ROOT
    /// (Round 1002), which is the only base an MCP caller can see. An absolute
    /// path is used as given. The CLI resolves its own explicit paths against
    /// the working directory instead (Round 538), because there the caller
    /// chose that directory; here it belongs to whatever host launched the
    /// server.
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
    /// PATH RESOLUTION: a relative path is resolved against the WORKSPACE ROOT
    /// (Round 1002), which is the only base an MCP caller can see. An absolute
    /// path is used as given. The CLI resolves its own explicit paths against
    /// the working directory instead (Round 538), because there the caller
    /// chose that directory; here it belongs to whatever host launched the
    /// server.
    /// Canon-order declaration path override (bypasses the pin).
    #[serde(default)]
    pub order_path: Option<String>,
    /// Disclosure telling id (R506 render-brief carrier): annotate each
    /// begins-event with its disclosure decision (mode/first_at/surface) under
    /// the named telling. Fail-loud on a typo'd id.
    #[serde(default)]
    pub telling: Option<String>,
    /// Reading-walk prune (R509): keep only each world's content scenes
    /// (begins>0) = the deterministic reading-copy walk.
    #[serde(default)]
    pub reading_walk: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReportForkTreeArgs {
    /// PATH RESOLUTION: a relative path is resolved against the WORKSPACE ROOT
    /// (Round 1002), which is the only base an MCP caller can see. An absolute
    /// path is used as given. The CLI resolves its own explicit paths against
    /// the working directory instead (Round 538), because there the caller
    /// chose that directory; here it belongs to whatever host launched the
    /// server.
    /// Canon-order declaration path override (bypasses the pin).
    #[serde(default)]
    pub order_path: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReportDisclosureCoverageArgs {
    /// Telling id to classify (disclosed / hidden-by-design / never-planned).
    pub telling: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReportPlayableWorldArgs {
    /// Telling id whose disclosure plan resolves the locators (required — a
    /// playable world IS a telling). Fail-loud on a typo'd id.
    pub telling: String,
    /// Single-world filter (a registered branch id or `main`); omitted = every
    /// query world. The fork tree stays full regardless. Fail-loud on an
    /// unregistered id.
    #[serde(default)]
    pub world: Option<String>,
    /// PATH RESOLUTION: a relative path is resolved against the WORKSPACE ROOT
    /// (Round 1002), which is the only base an MCP caller can see. An absolute
    /// path is used as given. The CLI resolves its own explicit paths against
    /// the working directory instead (Round 538), because there the caller
    /// chose that directory; here it belongs to whatever host launched the
    /// server.
    /// Canon-order declaration path override (bypasses the pin).
    #[serde(default)]
    pub order_path: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReportQuestGraphArgs {
    /// Telling id whose disclosure plan resolves the giver-surface locators
    /// (required). Fail-loud on a typo'd id.
    pub telling: String,
    /// Single-world filter (a registered branch id or `main`); omitted = every
    /// query world. The fork tree stays full regardless. Fail-loud on an
    /// unregistered id.
    #[serde(default)]
    pub world: Option<String>,
    /// PATH RESOLUTION: a relative path is resolved against the WORKSPACE ROOT
    /// (Round 1002), which is the only base an MCP caller can see. An absolute
    /// path is used as given. The CLI resolves its own explicit paths against
    /// the working directory instead (Round 538), because there the caller
    /// chose that directory; here it belongs to whatever host launched the
    /// server.
    /// Canon-order declaration path override (bypasses the pin).
    #[serde(default)]
    pub order_path: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DisclosureLeakArgs {
    /// Telling id whose plan is the gate's authored side.
    pub telling: String,
    /// Path to the BLIND RE-EXTRACTED prose store to check.
    pub against: String,
    /// The world-line the re-extracted prose represents.
    pub world: String,
    /// The frame whose re-extracted facts count as reader-established truth.
    pub truth_frame: String,
    /// PATH RESOLUTION: a relative path is resolved against the WORKSPACE ROOT
    /// (Round 1002), which is the only base an MCP caller can see. An absolute
    /// path is used as given. The CLI resolves its own explicit paths against
    /// the working directory instead (Round 538), because there the caller
    /// chose that directory; here it belongs to whatever host launched the
    /// server.
    /// Canon-order declaration path override (bypasses the pin).
    #[serde(default)]
    pub order_path: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RenderFidelityArgs {
    /// Path to the BLIND RE-EXTRACTED prose store to check.
    pub against: String,
    /// The assigned world-line the prose was rendered for.
    pub world: String,
    /// PATH RESOLUTION: a relative path is resolved against the WORKSPACE ROOT
    /// (Round 1002), which is the only base an MCP caller can see. An absolute
    /// path is used as given. The CLI resolves its own explicit paths against
    /// the working directory instead (Round 538), because there the caller
    /// chose that directory; here it belongs to whatever host launched the
    /// server.
    /// Canon-order declaration path override (bypasses the pin).
    #[serde(default)]
    pub order_path: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ImportTypingProposalsArgs {
    /// PATH RESOLUTION: a relative path is resolved against the WORKSPACE ROOT
    /// (Round 1002), which is the only base an MCP caller can see. An absolute
    /// path is used as given. The CLI resolves its own explicit paths against
    /// the working directory instead (Round 538), because there the caller
    /// chose that directory; here it belongs to whatever host launched the
    /// server.
    /// Path to a `typing-proposals/v1` JSON artifact (workspace-relative
    /// or absolute).
    pub proposals_path: String,
    /// Validate only — full verdicts, nothing written.
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReportEdgeCandidatesArgs {
    /// PATH RESOLUTION: a relative path is resolved against the WORKSPACE ROOT
    /// (Round 1002), which is the only base an MCP caller can see. An absolute
    /// path is used as given. The CLI resolves its own explicit paths against
    /// the working directory instead (Round 538), because there the caller
    /// chose that directory; here it belongs to whatever host launched the
    /// server.
    /// Canon-order declaration path override (bypasses the pin).
    #[serde(default)]
    pub order_path: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ImportEdgeProposalsArgs {
    /// PATH RESOLUTION: a relative path is resolved against the WORKSPACE ROOT
    /// (Round 1002), which is the only base an MCP caller can see. An absolute
    /// path is used as given. The CLI resolves its own explicit paths against
    /// the working directory instead (Round 538), because there the caller
    /// chose that directory; here it belongs to whatever host launched the
    /// server.
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
    /// Record what the workspace's recorded population says right now, so a
    /// later round inherits data instead of this entry's sentences (Round 979).
    /// The counts are read from the `[census] report` this workspace declares —
    /// there is deliberately no way to supply them here.
    #[serde(default)]
    pub record_census: bool,
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
        CallToolResult::success(vec![rmcp::model::ContentBlock::text(s)])
    }

    fn tool_error(s: String) -> CallToolResult {
        CallToolResult::error(vec![rmcp::model::ContentBlock::text(s)])
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

/// WHERE AN AGENT'S PATH IS RESOLVED: THE WORKSPACE (Round 1002).
///
/// A relative path is relative to something, and over MCP the only something
/// the caller can see is the workspace it was handed. The process's working
/// directory belongs to whatever host launched this server: an agent cannot
/// read it, cannot choose it, and gets a different one from every host. It was
/// nonetheless the base until this round, inherited from the CLI — where the
/// working directory IS the caller's own choice, the rule Round 538 set for a
/// path typed at a prompt, correct there and only there.
///
/// Round 998 measured the consequence: an order written into the workspace and
/// named by the agent was read from beside this source file. Round 999
/// documented it in the schema, which made the surface honest without making it
/// usable. Round 1000 moved the decision out of the shared library to this one
/// line so it could be changed alone. This is that change.
///
/// An ABSOLUTE path still passes through untouched, so an agent already sending
/// one — as it had to before this — is unaffected.
fn mcp_path(
    workspace: &std::path::Path,
    raw: Option<&str>,
) -> Result<Option<ops::AbsolutePath>, String> {
    let Some(raw) = raw else { return Ok(None) };
    ops::AbsolutePath::resolve(workspace, raw)
        .map(Some)
        .map_err(|e| e.to_string())
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
        description = "Verify and read ONE changelog entry by its `Round NNN` citation. THIS IS THE CITATION CHECK: call it before writing any `Round NNN` into code, a comment, a commit message, or a ledger entry — an error means the round does not exist and the citation must not be written. It resolves either stored key shape (short-form `Round 292`, long-form `Round 293 — <title>`), so never hand-match round numbers against list_changelog's keys yourself. Returns the full ChangelogEntryView (decision_summary + bullets), which is also how to read ONE decision without pulling the whole ledger into context."
    )]
    async fn query_changelog_entry(
        &self,
        args: Parameters<QueryChangelogEntryArgs>,
    ) -> CallToolResult {
        match ops::query::query_changelog_entry(&self.workspace, &args.0.entry_id) {
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
            atomic::set_section_impact_scope(
                store,
                path,
                &section,
                &refs
                    .iter()
                    .map(|s| mnemosyne_core::SectionId::from(s.as_str()))
                    .collect::<Vec<_>>(),
            )
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
        description = "Remove one section from the store (R267). `reason` mandatory — recorded on the receipt. Rejects if any live cross-ref still points at it (orphan guard); NotFound if absent (no silent no-op). The MCP parity for the CLI `remove-section` (R678): an MCP agent could create/edit sections but not remove one. Don't edit the sidecar JSON directly."
    )]
    async fn remove_section(&self, args: Parameters<RemoveSectionArgs>) -> CallToolResult {
        let section = strip_section_marker(&args.0.section_id).to_string();
        let reason = args.0.reason.clone();
        let outcome =
            self.run_mutate(|store, path| atomic::remove_section(store, path, &section, &reason));
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Transition a section's decision lifecycle (R678 parity for CLI set-section-decision-status): `active` | `superseded` | `removed` | `open`. `superseding` is MANDATORY for `superseded` (T1 rule 4 — a replaced decision must name its replacer) and rejected for any other status; `resolving` is valid only for `open` (the expected resolver). All guards are homed in the atomic write path, so this and the CLI enforce the identical invariant set. Absent section NotFound."
    )]
    async fn set_section_decision_status(
        &self,
        args: Parameters<SetSectionDecisionStatusArgs>,
    ) -> CallToolResult {
        let section = strip_section_marker(&args.0.section_id).to_string();
        let status_raw = args.0.status.clone();
        let superseding = args
            .0
            .superseding
            .as_deref()
            .map(|s| strip_section_marker(s).to_string());
        let resolving = args
            .0
            .resolving
            .as_deref()
            .map(|s| strip_section_marker(s).to_string());
        let outcome = self.run_mutate(|store, path| {
            let status = mnemosyne_core::DecisionStatus::from_tag(&status_raw.to_ascii_lowercase())
                .ok_or_else(|| {
                    atomic::AtomicMutateError::Validation(format!(
                        "status must be `active`, `superseded`, `removed`, or `open` (got `{}`)",
                        status_raw
                    ))
                })?;
            atomic::set_section_decision_status(
                store,
                path,
                &section,
                status,
                superseding.as_deref(),
                resolving.as_deref(),
            )
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
        description = "Classify a section's coverage applicability (R421 3-state). `normative` (default) keeps the coverage axiom — a non-removed normative section with zero `implements` bindings is a gap. `out_of_scope_here` (part of the document THIS LEDGER MIRRORS but not implemented by this consumer, which includes a deferred or Phase-2 feature; revisitable) and `informational` (inherently non-implementable prose — terminology / overview) both EXEMPT the section. Second write path to Section.coverage_expectation alongside import_sections; both enforce the same closed value set. `reason` mandatory."
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
                        "expectation must be {} (got `{}`)",
                        atomic::CoverageExpectation::vocabulary(),
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
                        "expectation must be {} (got `{}`)",
                        atomic::VerificationExpectation::vocabulary(),
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
            let converges_from: Vec<(&str, &str)> = a
                .converges_from
                .iter()
                .map(|c| (c.branch.as_str(), c.at.as_str()))
                .collect();
            atomic::add_branch(
                store,
                path,
                &a.branch_id,
                &a.description,
                fork,
                &converges_from,
            )
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Register one entity kind (R669) — the vocabulary `add_entity`'s `kind` refs name (character / place / item). The consumer declares the members; the substrate never enumerates them (ARCHITECTURE.md sec 6 invariant 4) and enforces only that a kind in use was declared. The extension path a closed set requires (R626: a guard without an escape hatch is a trap) — register a new kind the moment authoring needs one. Optional `parents` (R732 DEBT-M as one, R738 as a SET — multiple inheritance) make this kind a SUBKIND of each, so a rule scoped to any ancestor accepts it (a `thing`-scoped predicate admits a `weapon` when `thing` is reachable from `weapon`'s parents; a `magic-sword` with parents `[\"weapon\",\"magic-item\"]` satisfies both); each parent must be registered first and cannot be the kind itself. Idempotent on identical content; divergent rejects. Without this an MCP-only agent could not declare a kind (the Phase-0 AI-first north star)."
    )]
    async fn add_entity_kind(&self, args: Parameters<AddEntityKindArgs>) -> CallToolResult {
        let a = args.0;
        let parents: Vec<&str> = a.parents.iter().map(String::as_str).collect();
        let outcome = self.run_mutate(|store, path| {
            atomic::add_entity_kind(store, path, &a.kind_id, &parents, &a.description)
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Replace an EXISTING entity-kind's direct super-kinds (R739) — the parent-mutation half of the kind registry: add_entity_kind creates a leaf (which can never close a cycle), this re-parents an existing kind, so it is where the multi-node-cycle guard lives. `parents` is a full REPLACE (0..N, not a merge); each must be registered and not the kind itself, and none may be at-or-below the kind (that would close a cycle — a subkind made a super-kind). Empty roots the kind. Absent kind rejects (add creates, this mutates); the identical set is a no-op."
    )]
    async fn set_entity_kind_parents(
        &self,
        args: Parameters<SetEntityKindParentsArgs>,
    ) -> CallToolResult {
        let a = args.0;
        let parents: Vec<&str> = a.parents.iter().map(String::as_str).collect();
        let outcome = self.run_mutate(|store, path| {
            atomic::set_entity_kind_parents(store, path, &a.kind_id, &parents)
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Remove an entity kind from the registry (R740) — the remove peer of add_entity_kind. REFUSES while the kind is still referenced by an entity (Entity.kind), a child kind (a parents link), or a predicate endpoint (subject_kind / object_entity_kind, R701) — removing it would orphan those refs, which the write paths forbid. Absent kind rejects (not an idempotent no-op); a kind naming itself as a parent does not block its own removal."
    )]
    async fn remove_entity_kind(&self, args: Parameters<RemoveEntityKindArgs>) -> CallToolResult {
        let a = args.0;
        let outcome =
            self.run_mutate(|store, path| atomic::remove_entity_kind(store, path, &a.kind_id));
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Register one unit of measure (R706) — the vocabulary a `quantity` typed object's `unit` refs (day / minute / metre). The consumer declares the members; the substrate never enumerates them (invariant 4, the R700 place-kind lesson one axis over) and enforces only that a unit in use was declared — a bare unit string would drift min/minute/분. Register a unit before a Quantity uses it (the R626 escape hatch). Idempotent on identical content; divergent rejects. Without this an MCP-only agent could not declare a unit, so no Quantity fact could pass the units gate."
    )]
    async fn add_unit(&self, args: Parameters<AddUnitArgs>) -> CallToolResult {
        let a = args.0;
        let outcome = self
            .run_mutate(|store, path| atomic::add_unit(store, path, &a.unit_id, &a.description));
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Register a numeric PARAMETER (R729, DEBT-K) — an accumulating meter (affection / karma / gold / an RPG stat). The consumer declares the members; the substrate never enumerates them (invariant 4) and enforces only that a parameter in use was declared — a bare string would drift affection/affinity/호감도. Register a parameter before a delta or gate names it. Idempotent on identical content; divergent rejects. Like units, empty does not pass."
    )]
    async fn add_parameter(&self, args: Parameters<AddParameterArgs>) -> CallToolResult {
        let a = args.0;
        let outcome = self.run_mutate(|store, path| {
            atomic::add_parameter(store, path, &a.parameter_id, &a.description)
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Attach a SIGNED per-beat DELTA to a parameter (R729, DEBT-K) — a side-table entry keyed by the BEAT FACT ID, value = a parameter -> signed delta (+2 a gift, -1 an insult; one beat may move several meters). Fail-loud: the fact must exist, the parameter be registered (add_parameter first), and the delta be NON-ZERO (0 = a no-op beat) — both signs legal (the weighted/negative axis K-of-N cannot express). retract_fact cascade-drops the beat's deltas, so none dangles. Mnemosyne holds the authored delta; it NEVER computes a running sum along a playthrough (the consumer's job — the layering line). A2-consistent per (fact, parameter): identical is a no-op, divergent rejects."
    )]
    async fn add_parameter_delta(&self, args: Parameters<AddParameterDeltaArgs>) -> CallToolResult {
        let a = args.0;
        let outcome = self.run_mutate(|store, path| {
            atomic::add_parameter_delta(store, path, &a.fact_id, &a.parameter, a.delta)
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Remove ONE (fact, parameter) delta (R729) — the peer of add_parameter_delta. Drops the named parameter's delta from the beat; the beat key is deleted when the last delta goes. Fail-loud if the beat has no delta on that parameter."
    )]
    async fn remove_parameter_delta(
        &self,
        args: Parameters<RemoveParameterDeltaArgs>,
    ) -> CallToolResult {
        let a = args.0;
        let outcome = self.run_mutate(|store, path| {
            atomic::remove_parameter_delta(store, path, &a.fact_id, &a.parameter)
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Attach a numeric-value THRESHOLD GATE to a CHOICE edge (R730, DEBT-K) — a side-table entry keyed by the CHOICE FACT ID, value = {parameter, op, threshold} (\"romance route unlocks if affection >= 4\"). The axis K-of-N (edge_guards threshold) cannot express: a signed/weighted meter compared to a threshold. Fail-loud: the fact must exist, the parameter be registered (add_parameter first). Rides ANY real fact — NO map-edge check (a meter-gated route unlock is a narrative branch, not a spatial move). Because the gate references the meter DIRECTLY, the boolean-proxy silent hole is unrepresentable (no disconnected proxy fact to leave stale). op = ge|le|eq|gt|lt; threshold is signed (0/negative legal — satisfiability is the consumer's model, never Mnemosyne's). retract_fact cascade-drops the gate. Mnemosyne holds the declaration; it NEVER accumulates the meter or evaluates whether the gate holds now (the consumer's job — the layering line). A2-consistent: identical is a no-op, divergent rejects."
    )]
    async fn add_parameter_gate(&self, args: Parameters<AddParameterGateArgs>) -> CallToolResult {
        let a = args.0;
        let outcome = self.run_mutate(|store, path| {
            atomic::add_parameter_gate(store, path, &a.fact_id, &a.parameter, a.op, a.threshold)
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Remove a choice's parameter GATE (R730) — the peer of add_parameter_gate (mirrors remove_edge_cost). A gate is subordinate metadata that retract_fact cascade-drops with its fact, but a stray gate must be removable WITHOUT retracting the fact (the author may keep the choice un-gated, or the fact may be referenced so retract refuses it). Also cleans an out-of-band orphan gate. Fail-loud if the fact has no gate."
    )]
    async fn remove_parameter_gate(
        &self,
        args: Parameters<RemoveParameterGateArgs>,
    ) -> CallToolResult {
        let a = args.0;
        let outcome =
            self.run_mutate(|store, path| atomic::remove_parameter_gate(store, path, &a.fact_id));
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Attach a cost to one map EDGE (R709 → DEBT-J) — a side-table entry keyed by the ADJACENT FACT ID (the adjacent(a,b) fact), value = a number + registered unit (the Quantity shape). NOT a reified fact: the cost is frame-invariant edge metadata, so it carries no per-fact frame/branch/evidence. Fail-loud: the fact must exist, n must be POSITIVE (G3 — 0/negative is a free teleport), and the unit must be registered (add_unit first). `retract_fact` cascade-drops the cost when its fact goes, so it never dangles. Idempotent on identical content; divergent rejects."
    )]
    async fn add_edge_cost(&self, args: Parameters<AddEdgeCostArgs>) -> CallToolResult {
        let a = args.0;
        let outcome = self
            .run_mutate(|store, path| atomic::add_edge_cost(store, path, &a.fact_id, a.n, &a.unit));
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Remove a map EDGE's cost (R711) — the peer of add_edge_cost. A cost is subordinate metadata that retract_fact cascade-drops with its edge, but a cost mistakenly attached to a NON-edge fact (which validate-continuity flags as edge_cost_not_an_edge) must be removable WITHOUT retracting the fact (the fact may be legitimate, or referenced so retract refuses it). Also cleans an out-of-band orphan cost. Fail-loud if the fact has no edge cost."
    )]
    async fn remove_edge_cost(&self, args: Parameters<RemoveEdgeCostArgs>) -> CallToolResult {
        let a = args.0;
        let outcome =
            self.run_mutate(|store, path| atomic::remove_edge_cost(store, path, &a.fact_id));
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Attach a multiset COUNT to a fact (R731 → DEBT-L) — a side-table entry keyed by the fact id, value = a POSITIVE count (holds(A, potion) count 5 = A holds FIVE potions). The thing singular holds custody cannot express: a stackable-item quantity tied to the custody edge (the distinct part of DEBT-L; currency is a DEBT-K meter). NOT a reified fact: the count is frame-invariant metadata, a bare int (no unit — the thing counted is the fact's object leg). Fail-loud: the fact must EXIST and count must be POSITIVE (0/negative = not holding it — retract the fact). Rides ANY fact — NO custody-predicate check: anchoring to the per:object Exclusive rule is semantically inverted (a multiset count is meaningful for FUNGIBLE items, which are exactly the ones NOT under exclusivity). retract_fact cascade-drops the count, so it never dangles — the orphaned-count silent hole is unrepresentable. Idempotent on identical content; divergent rejects. Mnemosyne holds the count; it NEVER evaluates the multiset (the consumer's job — the layering line)."
    )]
    async fn add_fact_count(&self, args: Parameters<AddFactCountArgs>) -> CallToolResult {
        let a = args.0;
        let outcome =
            self.run_mutate(|store, path| atomic::add_fact_count(store, path, &a.fact_id, a.count));
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Remove a fact's multiset COUNT (R731) — the peer of add_fact_count. A count is subordinate metadata that retract_fact cascade-drops with its fact, but a count must also be removable WITHOUT retracting the fact (the author may want it kept un-counted, or the fact may be referenced so retract refuses it). Also cleans an out-of-band orphan count. Fail-loud if the fact has no count."
    )]
    async fn remove_fact_count(&self, args: Parameters<RemoveFactCountArgs>) -> CallToolResult {
        let a = args.0;
        let outcome =
            self.run_mutate(|store, path| atomic::remove_fact_count(store, path, &a.fact_id));
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Add a place-access GUARD condition to one map EDGE (R717/721 design → R720/722) — a side-table entry keyed by the ADJACENT (edge) FACT ID, value = the SET of CONDITION fact ids the edge REQUIRES (\"this passage requires the key AND low tide\"). A guard is a SET (AND-semantics): call this N times to add N conditions; OR is authored as MULTIPLE guarded edges to the same target (never a stored boolean expression tree — the layering line). Both facts must EXIST (a per-member dangling-ref check); an edge cannot guard itself. Mnemosyne holds the DECLARATION and integrity-checks only that each resolves — it NEVER evaluates whether the guard holds now (the consumer's playthrough job). retract_fact cascade-drops the whole set when its EDGE goes and REFUSES to retract a CONDITION any guard's set still references. Idempotent on an already-present condition."
    )]
    async fn add_edge_guard(&self, args: Parameters<AddEdgeGuardArgs>) -> CallToolResult {
        let a = args.0;
        let outcome = self.run_mutate(|store, path| {
            atomic::add_edge_guard(store, path, &a.fact_id, &a.condition)
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Remove a map EDGE's WHOLE guard set (R720) — the peer of add_edge_guard. A guard is subordinate metadata that retract_fact cascade-drops with its edge, but a guard mistakenly attached to a NON-edge fact (which validate-continuity flags as edge_guard_not_an_edge) must be removable WITHOUT retracting the fact. Also cleans an out-of-band orphan guard. Fail-loud if the fact has no edge guard. To drop just ONE condition, use remove_edge_guard_condition."
    )]
    async fn remove_edge_guard(&self, args: Parameters<RemoveEdgeGuardArgs>) -> CallToolResult {
        let a = args.0;
        let outcome =
            self.run_mutate(|store, path| atomic::remove_edge_guard(store, path, &a.fact_id));
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Remove ONE condition from a map EDGE's guard SET (R722) — the granular peer of add_edge_guard. Drops the named condition from the edge's set; the edge's key is deleted when the set empties (never a vacuous empty guard). Fail-loud if the edge has no such guard condition."
    )]
    async fn remove_edge_guard_condition(
        &self,
        args: Parameters<RemoveEdgeGuardConditionArgs>,
    ) -> CallToolResult {
        let a = args.0;
        let outcome = self.run_mutate(|store, path| {
            atomic::remove_edge_guard_condition(store, path, &a.fact_id, &a.condition)
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Set (or clear) a map EDGE guard's K-of-N THRESHOLD (R723). threshold=k makes the guard \"at least k of its conditions\" (1<=k<=len; k==len normalizes to AND); omit/null clears to AND (require all). Fail-loud: the edge must have a guard, and k must be in 1..=len (0 is vacuous, >len unsatisfiable). Mnemosyne stores k and checks the range — it NEVER counts how many hold now (the consumer's playthrough job; the layering line). OR is still multiple guarded edges to the same target."
    )]
    async fn set_edge_guard_threshold(
        &self,
        args: Parameters<SetEdgeGuardThresholdArgs>,
    ) -> CallToolResult {
        let a = args.0;
        let outcome = self.run_mutate(|store, path| {
            atomic::set_edge_guard_threshold(store, path, &a.fact_id, a.threshold)
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Register one narrative entity (R437) — the retrieval key for entity-scoped verification (a character's background, a location's lore). Fact `entities` refs must name a registered id (fail-loud). `kind` is a REF into the entity-kind registry (R669) — `add_entity_kind` first. Idempotent on identical content; divergent rejects."
    )]
    async fn add_entity(&self, args: Parameters<AddEntityArgs>) -> CallToolResult {
        let a = args.0;
        let outcome = self.run_mutate(|store, path| {
            atomic::add_entity(store, path, &a.entity_id, &a.kind, &a.description)
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Register one predicate (R446) — the 4th registry: TypedClaim predicates are load-bearing refs (narrative rules key off them), so a typo must fail loud, never silently escape its rule. object_kind declares the object leg's shape: entity | token | quantity | fact (R708 removed the free-text `scalar` shape — every machine-slot object is registered/enumerable now, free text lives only in the prose `claim`). R705 — `token` is a CLOSED, enumerable vocabulary declared in object_tokens (required non-empty under object_kind=token, rejected otherwise); the write path rejects a token outside the set. R706 — `quantity` is a number + a registered unit (units registry, add_unit first). R707 — `fact` references another fact of this store (phase-2 existence check, self-ref rejected, delete-guarded). R701 — optional subject_kind / object_entity_kind (registered entity_kinds refs) require the endpoint entity's kind at write time (the spatial-map gate); object_entity_kind rejects unless object_kind=entity. Idempotent on identical content; divergent rejects."
    )]
    async fn add_predicate(&self, args: Parameters<AddPredicateArgs>) -> CallToolResult {
        let a = args.0;
        let outcome = self.run_mutate(|store, path| {
            atomic::add_predicate(
                store,
                path,
                &a.predicate_id,
                a.object_kind,
                a.subject_kind.as_deref(),
                a.object_entity_kind.as_deref(),
                &a.object_tokens,
                &a.description,
            )
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Re-type or re-describe an EXISTING predicate (R658) — the repair half of the registry: add_predicate could create a state no primitive could undo (a divergent re-declare rejects), leaving only a vN id or a hand-edit, both banned. Full replace, so state description AND any subject_kind / object_entity_kind (R701; omit = clear) AND object_tokens (R705 — the closed vocabulary; required non-empty under object_kind=token, and the extension path: a re-declare that DROPS a token an existing use holds REJECTS, so widen the set, never silently narrow it). A re-declare REJECTS while any existing use fails the new object shape OR the new endpoint kinds OR the new vocabulary (a registry disagreeing with its uses is a silent broken state). Absent predicate rejects — add_predicate creates, this mutates."
    )]
    async fn set_predicate(&self, args: Parameters<SetPredicateArgs>) -> CallToolResult {
        let a = args.0;
        let outcome = self.run_mutate(|store, path| {
            atomic::set_predicate(
                store,
                path,
                &a.predicate_id,
                a.object_kind,
                a.subject_kind.as_deref(),
                a.object_entity_kind.as_deref(),
                &a.object_tokens,
                &a.description,
            )
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Remove a predicate from the registry (R658). REJECTS while any typed leg still names it — removing it would orphan those refs, which the write path forbids. Absent predicate rejects (not an idempotent no-op)."
    )]
    async fn remove_predicate(&self, args: Parameters<RemovePredicateArgs>) -> CallToolResult {
        let a = args.0;
        let outcome =
            self.run_mutate(|store, path| atomic::remove_predicate(store, path, &a.predicate_id));
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
        description = "Set one per-fact disclosure override within a telling (R506/R752): mode (withhold | state | hint | imply), per-world-line first_at timing (each entry {branch, coords[], threshold?} — a first-reached-of-a-SET trigger, coords the trigger set, threshold the optional K-of-N k-th-earliest; omit threshold = first-reached; multiple entries for one branch accumulate), and an optional diegetic surface (scene + entity). A setter (last-write-wins). Fail-loud refs: telling + fact must exist, first_at branches/coords + surface scene must resolve, surface object must be a registered entity. THE gate-enabling invariant: a withhold mode OR any first_at trigger requires the fact to carry a typed claim — the premature-leak render-acceptance gate matches re-extracted prose to the plan by typed tuple, so an untyped target is un-gateable."
    )]
    async fn set_disclosure(&self, args: Parameters<SetDisclosureArgs>) -> CallToolResult {
        let a = args.0;
        let first_at: Vec<atomic::DisclosureRevealImport> = a
            .first_at
            .iter()
            .map(|r| atomic::DisclosureRevealImport {
                branch: r.branch.clone(),
                coords: r.coords.clone(),
                threshold: r.threshold,
            })
            .collect();
        let surface = a
            .surface_scene
            .as_deref()
            .map(|scene| (scene, a.surface_object.as_deref()));
        let outcome = self.run_mutate(|store, path| {
            atomic::set_disclosure(
                store,
                path,
                atomic::DisclosureDecision {
                    telling_id: &a.telling_id,
                    fact_id: &a.fact_id,
                    mode: &a.mode,
                    first_at: &first_at,
                    surface,
                },
            )
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Clear one telling's disclosure decision for one fact (R626). The fact is untouched — a disclosure decision belongs to the TELLING, not the fact (R506). This is the escape hatch the R626 referential guards require: retract_fact refuses to delete a fact an override still points at, and amend_fact refuses to drop the typed leg out from under a withhold/first_at decision, both saying 'clear the decision first' — so clearing must be possible. Fail-loud: the telling and the decision must exist (no silent no-op), reason mandatory. NOT NEUTRAL (R627): the fact then rides the plan's default_mode, which defaults to `withhold` — so clearing a `state` decision flips that fact from told to never-told for that telling. The receipt names the resulting effective mode; if you are clearing only to retract the fact, do both, or the fact is left silently withheld."
    )]
    async fn remove_disclosure(&self, args: Parameters<RemoveDisclosureArgs>) -> CallToolResult {
        let a = args.0;
        let outcome = self.run_mutate(|store, path| {
            atomic::remove_disclosure(store, path, &a.telling_id, &a.fact_id, &a.reason)
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Add ONE first-reveal trigger COORDINATE to a fact's per-world reveal SET (R752) — the granular peer of set_disclosure, mirroring add_edge_guard. Where set_disclosure writes a fact's WHOLE disclosure decision at once, this edits ONE branch's first_at trigger set INCREMENTALLY, keyed by telling + fact + branch, value = the coord (a section ref) added to that branch's SET. A first_at reveal is first-reached-of-a-SET: a non-linear reader reveals the fact at the EARLIEST coord in the set they reach (R751/752). Multiple coords = multiple trigger points; call this N times to grow the set. Fail-loud: the override must already exist (set_disclosure first), the coord must be a registered section, the branch a known world; and because a first_at pin makes the fact gate-targeted, the fact must carry a typed claim (the premature-leak gate matches by typed tuple — the same gate-enabling invariant set_disclosure enforces). The branch's threshold is carried unchanged. Idempotent on an already-present coord."
    )]
    async fn add_disclosure_reveal_coord(
        &self,
        args: Parameters<AddDisclosureRevealCoordArgs>,
    ) -> CallToolResult {
        let a = args.0;
        let outcome = self.run_mutate(|store, path| {
            atomic::add_disclosure_reveal_coord(
                store,
                path,
                &a.telling_id,
                &a.fact_id,
                &a.branch,
                &a.coord,
            )
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Remove ONE first-reveal trigger COORDINATE from a fact's per-world reveal SET (R752) — the granular peer of add_disclosure_reveal_coord, mirroring remove_edge_guard_condition. Drops the named coord from the branch's first_at set; when it was the LAST coord the branch key is deleted (never a vacuous empty trigger). Fail-loud: the override, the branch, and that coord must exist. REFUSES a removal that would leave the branch's K-of-N threshold unsatisfiable (k > remaining) — lower it first with set_disclosure_reveal_threshold. UNLIKE the edge guard, a surviving threshold equal to the new length is KEPT (last-reached is a distinct semantic)."
    )]
    async fn remove_disclosure_reveal_coord(
        &self,
        args: Parameters<RemoveDisclosureRevealCoordArgs>,
    ) -> CallToolResult {
        let a = args.0;
        let outcome = self.run_mutate(|store, path| {
            atomic::remove_disclosure_reveal_coord(
                store,
                path,
                &a.telling_id,
                &a.fact_id,
                &a.branch,
                &a.coord,
            )
        });
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Set (or clear) a fact's per-world first-reveal K-of-N THRESHOLD (R752) — the granular peer of set_edge_guard_threshold, the ONE place k changes. threshold=k makes the branch's reveal fire at the k-th-earliest trigger reached (2<=k<=len; k==1 normalizes to the canonical first-reached; k==len = last-reached, KEPT distinct — unlike the edge guard where k==len collapses to AND); omit/null clears back to first-reached. Fail-loud: the override + the branch reveal must exist, and k must be in range (0 is vacuous, >len unsatisfiable). Mnemosyne checks the range on the DECLARATION and NEVER evaluates which triggers are reached (the R712 layering line — the consumer's playthrough job). set_disclosure sets a fact's whole reveal at once; this edits the threshold incrementally."
    )]
    async fn set_disclosure_reveal_threshold(
        &self,
        args: Parameters<SetDisclosureRevealThresholdArgs>,
    ) -> CallToolResult {
        let a = args.0;
        let outcome = self.run_mutate(|store, path| {
            atomic::set_disclosure_reveal_threshold(
                store,
                path,
                &a.telling_id,
                &a.fact_id,
                &a.branch,
                a.threshold,
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
        description = "Entity-kind migration worklist (R679, read-only): the distinct unregistered entity kinds a store uses, each with the entities naming it — the exact add_entity_kind calls a pre-registry (v23-) or out-of-band store needs. The complete list of the KIND facet, which the validate-workspace failure only samples (R681: the gate covers more than kinds); shares the kind detector the gate uses, so the two cannot disagree on kinds. Empty = every in-use kind is registered."
    )]
    async fn report_entity_kind_migration(&self, _args: Parameters<EmptyArgs>) -> CallToolResult {
        match ops::entity_kind_migration(&self.workspace, None) {
            Ok(r) => self.tool_json(&r),
            Err(e) => self.op_error(e),
        }
    }

    #[tool(
        description = "Parameter-economy read (R730, DEBT-K, read-only): the VISIBLE accumulation surface (gap 3). Per REGISTERED meter, the delta inventory (count, Σ+ = apply-once max reach, Σ- = apply-once min) and the gates that threshold it. NEUTRAL — the Σ is DESCRIPTIVE, NOT a reachability verdict: the consumer applies its OWN accumulation model (grinding / one-shot / clamped), so Mnemosyne never judges whether a gate is reachable (the R712 layering line). Deltas/gates on an unregistered parameter are out-of-band (the validate detectors' job), not this registered-scoped read."
    )]
    async fn report_parameter_economy(&self, _args: Parameters<EmptyArgs>) -> CallToolResult {
        match ops::parameter_economy_report(&self.workspace, None) {
            Ok(r) => self.tool_json(&r),
            Err(e) => self.op_error(e),
        }
    }

    #[tool(
        description = "Binding-kind migration worklist (Round 686, read-only): every code binding that inherited kind=implements by default from a pre-v5 store, pending review as implements vs references. `from_schema_version` is null when the store is already current (no migration; rows empty). Shares the ops report the CLI `report-binding-migration` renders, so the two surfaces cannot disagree. The sibling of report_entity_kind_migration, which was CLI-only until now (DEBT-BINDING-MIGRATION-MCP)."
    )]
    async fn report_binding_migration(&self, _args: Parameters<EmptyArgs>) -> CallToolResult {
        match ops::binding_kind_migration(&self.workspace, None) {
            Ok(r) => self.tool_json(&r),
            Err(e) => self.op_error(e),
        }
    }

    #[tool(
        description = "Create one narrative fact (R430): a claim held in exactly one epistemic frame on one world-line branch over a canon extent, evidenced by structure sections. Frame must be registered; a non-default branch must be registered (add_branch); canon/evidence refs must be sections; divergent re-add rejects — in-world belief change = supersedes_in_frame, authorial correction = amend_fact / retract_fact."
    )]
    async fn add_fact(&self, args: Parameters<atomic::FactImport>) -> CallToolResult {
        let fact = args.0;
        let outcome = self.run_mutate(|store, path| atomic::add_fact(store, path, &fact));
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Transactional batch fact authoring (Round 687, typed Round 690): import a whole FactsManifest — optional frames/branches/entity_kinds/entities/predicates plus the facts — in ONE atomic write. The AI-first way to author a scene: N separate add_* calls are non-atomic, so a mid-sequence failure leaves a partial store; this is all-or-nothing. The manifest is a TYPED argument (real JSON Schema), not an opaque string. Same invariants as add_fact per row; forward succession refs within the manifest are legal."
    )]
    async fn import_facts(&self, args: Parameters<atomic::FactsManifest>) -> CallToolResult {
        let manifest = args.0;
        let outcome = self.run_mutate(|store, path| atomic::import_facts(store, path, &manifest));
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Transactional batch section authoring (Round 687, typed Round 690): create a batch of structure sections — what facts evidence against — in ONE atomic write. All-or-nothing, same as import_facts. `sections` is a TYPED array of SectionImport (real JSON Schema), not an opaque string."
    )]
    async fn import_sections(&self, args: Parameters<ImportSectionsArgs>) -> CallToolResult {
        let sections = args.0.sections;
        let outcome =
            self.run_mutate(|store, path| atomic::import_sections(store, path, &sections));
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
        let AmendFactArgs { fact, reason } = args.0;
        let outcome =
            self.run_mutate(|store, path| atomic::amend_fact(store, path, &fact, &reason));
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
        description = "Frame-scoped continuity scan (R431, read-only): same-(frame, branch) conflicting pairs whose derived canon extents co-hold are violations; cross-scope pairs are data. With a declared narrative-rules/v1 artifact (R449) it also derives typed-claim rule findings — exclusive (one co-holding value per subject / one holder per object), transition (allowed state steps on succession edges), and interval (R489: a scalar/arithmetic relation value(left) − value(right) op bound per frame-world-subject) — plus the unchained_state_pairs, unchained_unreachable_pairs (R916: the subset no ROUTE joins in the hierarchy-augmented map, so no untold journey could cover it — EVERY transition rule since R924, the claim needing no genre and being conservative on a directed rule because the walk symmetrizes) and interval_unverifiable honesty counts, each of which is NULL when no rule of its class was declared (R924: a silence is not a zero). Interval violations ride a SEPARATE per-class severity (R491, interval_severity, OFF by default — a timeline gap can be an intentional time-bend); structural violations ride severity. Returns the JSON report (both severities, interval_violation_count, counts, violations); gating policy belongs to the caller."
    )]
    async fn validate_continuity(
        &self,
        args: Parameters<ValidateContinuityArgs>,
    ) -> CallToolResult {
        match ops::continuity_scan(
            &self.workspace,
            None,
            match mcp_path(&self.workspace, args.0.order_path.as_deref()) {
                Ok(v) => v,
                Err(e) => return Self::tool_error(e),
            }
            .as_ref(),
            match mcp_path(&self.workspace, args.0.rules_path.as_deref()) {
                Ok(v) => v,
                Err(e) => return Self::tool_error(e),
            }
            .as_ref(),
        ) {
            Ok(report) => self.tool_json(&report),
            Err(e) => self.op_error(e),
        }
    }

    #[tool(
        description = "Propose-verdict (R588, DRY RUN): the generate-gate-repair loop's atomic gate. Reads a candidate import-facts manifest from manifest_path, applies it to a THROWAWAY in-memory clone of the store, runs the shape invariants + the continuity gate, and returns verdict=commit|rollback plus actionable violations (each carries rule + locus {facts,field,frame,branch,at} + expected + repair_hint + message). The real store is NEVER written — on commit, apply for real via the import-facts CLI. Deterministic, AI out of the gate. Fail-loud on an unreadable/unparseable manifest."
    )]
    async fn propose_verdict(&self, args: Parameters<ProposeVerdictArgs>) -> CallToolResult {
        // Round 1001 — an agent's manifest path goes through the same wire
        // resolver as every other path it sends. Round 1000 typed the
        // overrides that passed through `ops` and left the ones the wire
        // opens itself, which is the half-cleanup this repository bans:
        // the ambiguity an agent faces is identical either way.
        let manifest_path = match mcp_path(&self.workspace, Some(&args.0.manifest_path)) {
            Ok(p) => p.expect("a Some input yields a Some path"),
            Err(e) => return Self::tool_error(e),
        };
        let raw = match std::fs::read_to_string(manifest_path.as_path()) {
            Ok(r) => r,
            Err(e) => return Self::tool_error(format!("read manifest {manifest_path}: {e}")),
        };
        let manifest = match mnemosyne_atomic::parse_facts_manifest(&raw) {
            Ok(m) => m,
            Err(e) => {
                return Self::tool_error(format!(
                    "parse manifest {} ({}): {e}",
                    args.0.manifest_path,
                    mnemosyne_atomic::FACTS_MANIFEST_SHAPE
                ))
            }
        };
        match ops::propose_verdict(
            &self.workspace,
            None,
            match mcp_path(&self.workspace, args.0.order_path.as_deref()) {
                Ok(v) => v,
                Err(e) => return Self::tool_error(e),
            }
            .as_ref(),
            match mcp_path(&self.workspace, args.0.rules_path.as_deref()) {
                Ok(v) => v,
                Err(e) => return Self::tool_error(e),
            }
            .as_ref(),
            &manifest,
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
            match mcp_path(&self.workspace, args.0.order_path.as_deref()) {
                Ok(v) => v,
                Err(e) => return Self::tool_error(e),
            }
            .as_ref(),
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
        match ops::payoff_coverage_report(
            &self.workspace,
            None,
            match mcp_path(&self.workspace, args.0.order_path.as_deref()) {
                Ok(v) => v,
                Err(e) => return Self::tool_error(e),
            }
            .as_ref(),
        ) {
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
        // Resolved BEFORE the lock closure: a `return` inside a closure
        // leaves the closure, not the tool call.
        let proposals_path = match mcp_path(&self.workspace, Some(&args.0.proposals_path)) {
            Ok(p) => p.expect("a Some input yields a Some path"),
            Err(e) => return Self::tool_error(e),
        };
        match self.with_mutate_lock(|| {
            ops::import_typing_proposals_report(
                &self.workspace,
                None,
                proposals_path.as_path(),
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
        match ops::payoff_substantiation_report(
            &self.workspace,
            None,
            match mcp_path(&self.workspace, args.0.order_path.as_deref()) {
                Ok(v) => v,
                Err(e) => return Self::tool_error(e),
            }
            .as_ref(),
        ) {
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
            match mcp_path(&self.workspace, args.0.order_path.as_deref()) {
                Ok(v) => v,
                Err(e) => return Self::tool_error(e),
            }
            .as_ref(),
            match mcp_path(&self.workspace, args.0.rules_path.as_deref()) {
                Ok(v) => v,
                Err(e) => return Self::tool_error(e),
            }
            .as_ref(),
        ) {
            Ok(report) => self.tool_json(&report),
            Err(e) => self.op_error(e),
        }
    }

    #[tool(
        description = "The declared map (R875, read-only): per transition rule, the map its `adjacency` predicate names — nodes, edges (each the declaring fact id + from/to + the fact's frame/branch), and per edge the STORED side-table values: `cost` {n, registered unit} (R710) and `guard` {conditions, optional k-of-N threshold} (R722/R723). The read half of the map axis: `add_edge_cost` / `add_edge_guard` write these, and until now no read handed them back, so a consumer had to parse the store sidecar itself. Also names what a naive bake would silently lose: self-loops (excluded from edges, as the gate excludes them) and side-table keys on facts that are not an edge of any declared map. Flat and un-scoped, exactly as the continuity gate evaluates the map — `undirected` is reported, never applied, and a guard is never evaluated (the R712 layering line). `transition_rules: 0` means no rule declares an adjacency predicate, which is NOT the same as a map with no edges."
    )]
    async fn report_transition_map(
        &self,
        args: Parameters<ReportTransitionMapArgs>,
    ) -> CallToolResult {
        match ops::transition_map_report(
            &self.workspace,
            None,
            match mcp_path(&self.workspace, args.0.rules_path.as_deref()) {
                Ok(v) => v,
                Err(e) => return Self::tool_error(e),
            }
            .as_ref(),
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
        match ops::edge_candidates_report(
            &self.workspace,
            None,
            match mcp_path(&self.workspace, args.0.order_path.as_deref()) {
                Ok(v) => v,
                Err(e) => return Self::tool_error(e),
            }
            .as_ref(),
        ) {
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
        // Resolved BEFORE the lock closure: a `return` inside a closure
        // leaves the closure, not the tool call.
        let proposals_path = match mcp_path(&self.workspace, Some(&args.0.proposals_path)) {
            Ok(p) => p.expect("a Some input yields a Some path"),
            Err(e) => return Self::tool_error(e),
        };
        match self.with_mutate_lock(|| {
            ops::import_edge_proposals_report(
                &self.workspace,
                None,
                proposals_path.as_path(),
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
        match ops::irony_intervals_report(
            &self.workspace,
            None,
            match mcp_path(&self.workspace, args.0.order_path.as_deref()) {
                Ok(v) => v,
                Err(e) => return Self::tool_error(e),
            }
            .as_ref(),
        ) {
            Ok(report) => self.tool_json(&report),
            Err(e) => self.op_error(e),
        }
    }

    #[tool(
        description = "Playthrough manuscript (R466, read-only): per query world (or the single `world` filter), the composed canon order's deterministic topological walk with declared fact events placed on each scene — begins, ends (expired / superseded-by), holds-judged holding_count, skeleton title + EPUB locator. Honesty surfaces: undeclared_adjacencies (incomparable emitted neighbors — one valid reading, never the only one), unplaced_facts, undecidable (B-1), sections_off_road (scenes belonging to another world-line, or isolated coordinates). Reading surface, never gated."
    )]
    async fn report_playthrough_manuscript(
        &self,
        args: Parameters<ReportPlaythroughManuscriptArgs>,
    ) -> CallToolResult {
        match ops::playthrough_manuscript_report(
            &self.workspace,
            None,
            args.0.world.as_deref(),
            match mcp_path(&self.workspace, args.0.order_path.as_deref()) {
                Ok(v) => v,
                Err(e) => return Self::tool_error(e),
            }
            .as_ref(),
            args.0.telling.as_deref(),
            args.0.reading_walk,
        ) {
            Ok(report) => self.tool_json(&report),
            Err(e) => self.op_error(e),
        }
    }

    #[tool(
        description = "Fork tree (R497, read-only): the cross-world choice graph — every registered world-line with its divergence coordinate (parent + fork point + the branch description = the CYOA choice label), the fork point resolved against the parent's composed order (at_placed; false = surfaced in unplaced_fork_points, never dropped). The per-world manuscripts (R466) stitched at the fork points. `converges` = the merges flowing INTO a world-line; `rejoins` = the confluences it flows OUT into (R836, derived by inverting the merges — a branch that rejoins is not a permanent divergence, and its record alone would not say so). Fail-loud on a fork whose parent is neither `main` nor registered. Reading surface, never gated."
    )]
    async fn report_fork_tree(&self, args: Parameters<ReportForkTreeArgs>) -> CallToolResult {
        match ops::fork_tree_report(
            &self.workspace,
            None,
            match mcp_path(&self.workspace, args.0.order_path.as_deref()) {
                Ok(v) => v,
                Err(e) => return Self::tool_error(e),
            }
            .as_ref(),
        ) {
            Ok(report) => self.tool_json(&report),
            Err(e) => self.op_error(e),
        }
    }

    #[tool(
        description = "Playable world (R556/557, read-only): the map_locator seam a pinion narrative runtime consumes — per telling, the cross-world fork topology (R497) + each world-line's scene walk (R466) + the per-scene disclosure MapLocators (the authored DisclosureSurface resolved to a stable pointer {world_line, scene, scene_ordinal, object, mode, first_at}, no baked geometry = CQRS read-side). first_at is the order-free reveal DECLARATION {coords[], threshold?} (R752) — a first-reached-of-a-SET trigger the runtime resolves against the player's actual non-linear path, not a single baked coordinate. A pure JOIN over the manuscript + fork-tree projections; `world` filters the per-world map (the fork tree stays full). Reading surface, never gated. Fail-loud on a typo'd telling / unregistered world."
    )]
    async fn report_playable_world(
        &self,
        args: Parameters<ReportPlayableWorldArgs>,
    ) -> CallToolResult {
        match ops::playable_world_report(
            &self.workspace,
            None,
            args.0.world.as_deref(),
            match mcp_path(&self.workspace, args.0.order_path.as_deref()) {
                Ok(v) => v,
                Err(e) => return Self::tool_error(e),
            }
            .as_ref(),
            &args.0.telling,
        ) {
            Ok(report) => self.tool_json(&report),
            Err(e) => self.op_error(e),
        }
    }

    #[tool(
        description = "Quest graph (R559/568, read-only): the fact->quest leg a pinion narrative runtime / authoring consumer needs, the sibling of report_playable_world. Per telling, every derived quest (pursues object / requires endpoint / completed_by subject) projected to a QuestNode — objective + actor (pursues) + prerequisites (requires) + giving setups + per-world DERIVED open/done (the R442 payoff coverage: a quest done on one road and open on another) + the completion fact (with discharger) + the giver-surface MapLocator (R557). A pure JOIN over payoff-coverage + playable-world; `world` filters the per-world map (the fork tree stays full). Reading surface, never gated; quest STATE derived per world-line, never stored. Executable quest logic (lifecycle/guards) is SCE/pinion's. Fail-loud on a typo'd telling / unregistered world."
    )]
    async fn report_quest_graph(&self, args: Parameters<ReportQuestGraphArgs>) -> CallToolResult {
        match ops::quest_graph_report(
            &self.workspace,
            None,
            args.0.world.as_deref(),
            match mcp_path(&self.workspace, args.0.order_path.as_deref()) {
                Ok(v) => v,
                Err(e) => return Self::tool_error(e),
            }
            .as_ref(),
            &args.0.telling,
        ) {
            Ok(report) => self.tool_json(&report),
            Err(e) => self.op_error(e),
        }
    }

    #[tool(
        description = "Authoring contract (R587, static): the medium-neutral schema an agent reads to self-serve BEFORE authoring — the registries (frames/branches/entities/predicates/disclosure_plans/sections; declare an id before a fact references it), the narrative-fact shape (required/optional fields), the fixed vocabularies (disclosure_mode, payoff_expectation, predicate_object_kind — the closed enums), the deterministic narrative-rule classes (exclusive/transition/interval), the quest encoding (quests DERIVED from pursues/requires/completed_by roles, no kind marker; completion pays off an Expected setup), and the write-time fail-loud invariants. Store-independent — the contract is fixed; store CONTENTS are query/list_*. No args."
    )]
    async fn describe_schema(&self, _args: Parameters<EmptyArgs>) -> CallToolResult {
        self.tool_json(&ops::describe_schema())
    }

    #[tool(
        description = "Authoring frontier (R589, read-only): the consolidated coverage-gap surface an unattended generate-gate-repair loop pulls its next work from, JOINed from the scattered projections. Always: zero_fact_scenes (sections with no fact anchored) + scene_coverage (facts per section, incl. a derived structural quest-plumbing count, R619) + structural_facts (the flat structural fact-id set to JOIN with each fact's branch, R619) + branch_owned_density (per world-line, own facts over its full traversed road — a divergent world that looks full by inheritance but owns little reads LOW, R617/R619) + dangling_setups (per world-line, R442 Expected facts with no visible payoff) + total_gaps. With telling: unresolved_quests (R568) + never_planned_disclosures (R507, facts never given an explicit disclosure decision). Plus map_frontier (R891): per declared map, the registered places its adjacency predicate's leg kinds admit that are NOT a node of it (places with no way in or out), plus costs/guards keyed to a non-edge. transition_rules 0 is the THIRD state — no adjacency predicate is declared, so the store cannot know which facts are edges — never 'no map work'. Pure read, never gated. Fail-loud on a typo'd telling."
    )]
    async fn report_authoring_frontier(
        &self,
        args: Parameters<ReportAuthoringFrontierArgs>,
    ) -> CallToolResult {
        match ops::authoring_frontier_report(
            &self.workspace,
            None,
            match mcp_path(&self.workspace, args.0.order_path.as_deref()) {
                Ok(v) => v,
                Err(e) => return Self::tool_error(e),
            }
            .as_ref(),
            args.0.telling.as_deref(),
            match mcp_path(&self.workspace, args.0.rules_path.as_deref()) {
                Ok(v) => v,
                Err(e) => return Self::tool_error(e),
            }
            .as_ref(),
        ) {
            Ok(report) => self.tool_json(&report),
            Err(e) => self.op_error(e),
        }
    }

    #[tool(
        description = "Disclosure coverage (R507, read-only): per telling, every fact classified disclosed / hidden-by-design (an explicit withhold override) / never-planned (no override under a withhold-default telling = the author's todo list). A SURFACE (the R442 dangling-is-a-todo discipline), never gated. Fail-loud on a typo'd telling."
    )]
    async fn report_disclosure_coverage(
        &self,
        args: Parameters<ReportDisclosureCoverageArgs>,
    ) -> CallToolResult {
        match ops::disclosure_coverage_report(&self.workspace, None, &args.0.telling) {
            Ok(report) => self.tool_json(&report),
            Err(e) => self.op_error(e),
        }
    }

    #[tool(
        description = "Premature-leak gate (R507/R502): the authored disclosure plan vs a BLIND RE-EXTRACTED prose store (against), matched by typed (subject,predicate,object) tuple in truth_frame for world. A withheld fact that re-extracts, or a first_at-pinned fact re-extractable before its pin in the world's order, is a leak (leaks[] non-empty = FAIL). Deterministic — AI out of the gate. Returns the report; gating policy is the caller's. Fail-loud on typo'd telling / world / truth_frame."
    )]
    async fn validate_disclosure_leak(
        &self,
        args: Parameters<DisclosureLeakArgs>,
    ) -> CallToolResult {
        let a = args.0;
        match ops::disclosure_leak_report(
            &self.workspace,
            None,
            std::path::Path::new(&a.against),
            match mcp_path(&self.workspace, a.order_path.as_deref()) {
                Ok(v) => v,
                Err(e) => return Self::tool_error(e),
            }
            .as_ref(),
            &a.telling,
            &a.world,
            &a.truth_frame,
        ) {
            Ok(report) => self.tool_json(&report),
            Err(e) => self.op_error(e),
        }
    }

    #[tool(
        description = "Render<->world-line fidelity gate (R507/R505): a BLIND RE-EXTRACTED prose store (against) checked against world's composed order — a re-extracted canon_from that is a declaration node of ANOTHER world is off-path (the prose drifted onto the wrong world-line; off_path[] non-empty = FAIL). The prose analog of R488 FactCanonOffBranch. Returns the report; gating policy is the caller's. Fail-loud on a typo'd world."
    )]
    async fn validate_render_fidelity(
        &self,
        args: Parameters<RenderFidelityArgs>,
    ) -> CallToolResult {
        let a = args.0;
        match ops::render_fidelity_report(
            &self.workspace,
            None,
            std::path::Path::new(&a.against),
            match mcp_path(&self.workspace, a.order_path.as_deref()) {
                Ok(v) => v,
                Err(e) => return Self::tool_error(e),
            }
            .as_ref(),
            &a.world,
        ) {
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
        // Round 979 — the same resolver the CLI calls. Two wires into one field
        // must enforce one invariant set, and the way this one does it is by
        // having no second reading of the report to diverge from.
        let population_census = if args.0.record_census {
            match ops::workspace_population_census(&self.workspace) {
                Ok(c) => c,
                Err(e) => return self.op_error(e),
            }
        } else {
            Vec::new()
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
                    impact_refs: &impact
                        .iter()
                        .map(|s| mnemosyne_core::SectionId::from(s.as_str()))
                        .collect::<Vec<_>>(),
                    carry_forward_bullets: &carry,
                    population_census: &population_census,
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
            atomic::set_changelog_publishable_impact_refs(
                store,
                path,
                &entry_id,
                &bullets
                    .iter()
                    .map(|s| mnemosyne_core::SectionId::from(s.as_str()))
                    .collect::<Vec<_>>(),
            )
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
                Resource::new(r.uri, r.name)
                    .with_title(r.title)
                    .with_description(r.description)
                    .with_mime_type("text/markdown")
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

    // Round 826 — say which build this is BEFORE the workspace is opened. If it
    // declares `[tool] pin` and this is not that revision, the server does not
    // start: a gate that cannot answer must not answer, and a server that came
    // up while unusable would report health it does not have.
    mnemosyne_config::register_tool_stamp(env!("BUILD_GIT_HASH"));

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
    use std::collections::BTreeSet;

    /// R680 — the MCP-surface smoke the cost-audit found missing (#4). The R669
    /// blind spot was that a green unit suite NEVER verified a tool was actually
    /// EXPOSED on the MCP router: `add_entity` gated on a registry while
    /// `add_entity_kind` was CLI-only for a whole round, invisible to every
    /// test. This asserts the router routes the authoring tools an AI-first
    /// agent needs — the four added this session plus pre-existing anchors — so
    /// a tool that compiles but is never routed fails HERE, not in an adopter's
    /// hands.
    ///
    /// ROUTING PRESENCE IS NOT THE WHOLE SURFACE CHECK, and this comment used
    /// to say it was: "the tools are thin `run_mutate` wrappers over
    /// CLI-exercised atomic paths". Round 981 found a wrapper whose argument
    /// was never read, and Round 986 measured that neither the compiler nor any
    /// test would have said so. Thin is not the same as faithful. What covers
    /// the rest is [`every_optional_tool_argument_is_declared_exercised_or_not`]
    /// — presence here, behaviour there.
    #[test]
    fn mcp_router_exposes_the_authoring_tools() {
        let router = MnemosyneServer::tool_router();
        for name in [
            "add_entity_kind",                 // R674
            "remove_section",                  // R678
            "set_section_decision_status",     // R678
            "report_entity_kind_migration",    // R679
            "report_binding_migration",        // R686
            "import_facts",                    // R687
            "import_sections",                 // R687
            "add_disclosure_reveal_coord",     // R752 (granular disclosure-reveal parity)
            "remove_disclosure_reveal_coord",  // R752
            "set_disclosure_reveal_threshold", // R752
            "add_entity",                      // pre-existing anchors (non-vacuity)
            "add_fact",
            "report_quest_graph",
        ] {
            assert!(
                router.has_route(name),
                "MCP router does not expose tool `{name}` — an agent cannot call it"
            );
        }
        // Non-vacuity: a non-tool must NOT route, so the check can actually fail.
        assert!(!router.has_route("definitely_not_a_tool"));
    }

    /// THE SECOND WIRE INTO `population_census` IS EXERCISED, NOT INSPECTED
    /// (Round 981).
    ///
    /// Round 979 put a field in the frozen ledger with two write paths, and
    /// checked their parity by READING both wires' source — that a count cannot
    /// be typed into either. A source scan cannot see the thing that actually
    /// breaks a wire: a serde key that never deserializes, or an argument read
    /// into nothing. `record_census` would then be silently false forever, and
    /// the entry an agent appended through MCP would record no census while
    /// saying it had. Nothing in this repository executed an MCP tool, so that
    /// half of the parity claim rested on prose.
    ///
    /// The oracle is the CLI's own answer: the counts must equal what
    /// `ops::workspace_population_census` yields for the same workspace, so this
    /// cannot pass by agreeing with a mistake spelled twice.
    #[tokio::test]
    async fn mcp_append_records_the_census_the_shared_resolver_yields() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let ws = tmp.path();
        std::fs::create_dir_all(ws.join("docs/.atomic")).expect("atomic dir");
        std::fs::write(
            ws.join("mnemosyne.toml"),
            "[workspace]\n[schema]\nentry_id_prefix = \"Round \"\n\
             [census]\nreport = \"census.json\"\n",
        )
        .expect("config");
        let axes = vec![atomic::PopulationCensus {
            axis: "transition rules by `undirected`".to_string(),
            left_label: "undirected".to_string(),
            left: 10,
            right_label: "directed".to_string(),
            right: 27,
        }];
        std::fs::write(
            ws.join("census.json"),
            ops::render_population_census(&axes).expect("render"),
        )
        .expect("report");
        std::fs::write(
            ws.join("docs/.atomic/workspace.atomic.json"),
            format!(
                "{{\"schema_version\":{},\"sections\":{{}},\"changelog_entries\":{{}}}}",
                atomic::CURRENT_SCHEMA_VERSION
            ),
        )
        .expect("store");

        // The argument arrives the way an agent sends it — as JSON — so a serde
        // key that does not deserialize fails here rather than defaulting to
        // false and recording nothing.
        let args: AppendChangelogEntryArgs = serde_json::from_value(serde_json::json!({
            "entry_id": "Round 981",
            "decision_summary": "a round that states a census",
            "changes_bullets": ["changed a thing"],
            "verification_bullets": ["checked the thing"],
            "record_census": true,
        }))
        .expect("the agent-facing shape must carry `record_census`");
        assert!(args.record_census, "the flag deserialized as false");

        let server = MnemosyneServer::new(ws.to_path_buf()).expect("server");
        let result = server.append_changelog_entry(Parameters(args)).await;
        assert!(
            result.is_error != Some(true),
            "the MCP append failed: {:?}",
            result.content
        );

        let store = atomic::AtomicStore::load(&ws.join("docs/.atomic/workspace.atomic.json"))
            .expect("reload");
        let recorded = &store
            .changelog_entries
            .get("Round 981")
            .expect("the entry landed")
            .population_census;
        assert_eq!(
            recorded,
            &ops::workspace_population_census(ws).expect("the CLI's own resolver"),
            "the MCP wire recorded something other than what the shared resolver \
             yields, so the two write paths into this field disagree"
        );

        // Non-vacuity: without the flag the same wire records nothing, so the
        // assertion above is about the flag and not about the field's default.
        let bare: AppendChangelogEntryArgs = serde_json::from_value(serde_json::json!({
            "entry_id": "Round 982",
            "decision_summary": "a round that states none",
            "changes_bullets": ["changed a thing"],
            "verification_bullets": ["checked the thing"],
        }))
        .expect("args without the flag");
        let result = server.append_changelog_entry(Parameters(bare)).await;
        assert!(result.is_error != Some(true), "the second append failed");
        let store = atomic::AtomicStore::load(&ws.join("docs/.atomic/workspace.atomic.json"))
            .expect("reload");
        assert!(
            store
                .changelog_entries
                .get("Round 982")
                .expect("the entry landed")
                .population_census
                .is_empty(),
            "an append that never asked for a census recorded one anyway"
        );
    }

    /// Every (tool, optional argument) pair a differential test proves the
    /// handler actually READS — generated from the same invocations that
    /// generate the tests, so a pair cannot be claimed without one existing.
    /// Growing this list is the point; it may never shrink without the round
    /// that shrinks it saying why.
    /// Pairs proven by a BESPOKE test the `exercised!` macro cannot express —
    /// `record_census` needs a workspace that declares a `[census] report`, and
    /// its oracle is the shared resolver rather than a needle.
    ///
    /// THIS LIST IS WEAKER THAN THE GENERATED ONE AND IS KEPT SHORT FOR THAT
    /// REASON: nothing checks that the named test exists, so it is a claim of
    /// exactly the kind the macro was written to stop making. Each line names
    /// the test that backs it.
    const EXERCISED_BESPOKE: &[(&str, &str)] = &[
        // mcp_append_records_the_census_the_shared_resolver_yields (R981)
        ("append_changelog_entry", "record_census"),
    ];

    /// Every (tool, optional argument) pair that NOTHING proves the handler
    /// reads. Each line is a live instance of the Round 981 defect: an agent
    /// sends the argument, the tool answers success, and whether it did
    /// anything is unknown.
    const UNEXERCISED: &[(&str, &str)] = &[
        ("emit_publishable_override_ledger_draft", "kind"),
        ("import_edge_proposals", "dry_run"),
        ("import_typing_proposals", "dry_run"),
        ("propose_verdict", "order_path"),
        ("propose_verdict", "rules_path"),
        ("query_term", "fields"),
        ("redact_term", "kind"),
        ("redact_term", "scope"),
        ("report_authoring_frontier", "order_path"),
        ("report_authoring_frontier", "rules_path"),
        ("report_authoring_frontier", "telling"),
        ("report_edge_candidates", "order_path"),
        ("report_fork_tree", "order_path"),
        ("report_frame_view", "branch"),
        ("report_frame_view", "entity"),
        ("report_frame_view", "order_path"),
        ("report_irony_intervals", "order_path"),
        ("report_payoff_coverage", "order_path"),
        ("report_payoff_substantiation", "order_path"),
        ("report_playable_world", "world"),
        ("report_playthrough_manuscript", "order_path"),
        ("report_playthrough_manuscript", "reading_walk"),
        ("report_playthrough_manuscript", "telling"),
        ("report_playthrough_manuscript", "world"),
        ("report_quest_graph", "order_path"),
        ("report_quest_graph", "world"),
        ("report_timeline_gaps", "order_path"),
        ("report_timeline_gaps", "rules_path"),
        ("report_transition_map", "rules_path"),
        ("style_check", "doc"),
        ("style_check", "severity"),
        ("validate_continuity", "order_path"),
        ("validate_continuity", "rules_path"),
        ("validate_disclosure_leak", "order_path"),
        ("validate_render_fidelity", "order_path"),
    ];

    /// The optional arguments of every routed tool, taken from the ROUTER'S OWN
    /// SCHEMA — the same bytes an agent is shown.
    fn optional_arguments() -> Vec<(String, String)> {
        let mut out = Vec::new();
        for tool in MnemosyneServer::tool_router().list_all() {
            let required: BTreeSet<&str> = tool
                .input_schema
                .get("required")
                .and_then(|r| r.as_array())
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str())
                .collect();
            for key in tool
                .input_schema
                .get("properties")
                .and_then(|p| p.as_object())
                .into_iter()
                .flatten()
                .map(|(k, _)| k)
            {
                if !required.contains(key.as_str()) {
                    out.push((tool.name.to_string(), key.clone()));
                }
            }
        }
        out.sort();
        out
    }

    /// EVERY PATH AN AGENT CAN SEND SAYS WHAT IT IS RELATIVE TO.
    ///
    /// Round 998 wrote a canon order into the workspace, passed its name, and
    /// got "No such file or directory" against a directory the test never chose:
    /// an explicit path override is resolved against the SERVER PROCESS's
    /// working directory. That is a deliberate rule — Round 538 made an explicit
    /// `--order` CWD-relative to match `--sidecar` and `--manifest`, while the
    /// config-declared path stays workspace-rooted — and it is right for a CLI,
    /// where the working directory is the user's own choice.
    ///
    /// IT WAS NEVER RE-DECIDED FOR MCP, where the working directory belongs to
    /// whatever host launched the server and the agent is told only about the
    /// workspace. Changing the resolution is a decision about the contract and
    /// is not taken here. What IS this side's omission is that the agent-facing
    /// schema never said it, so the one caller who cannot see the working
    /// directory was also the one not told it mattered.
    ///
    /// The rule now lives in the field descriptions, which is what an agent
    /// reads, and this asserts it stays there — a new path argument that does
    /// not say where it resolves fails here rather than in an agent's hands.
    #[test]
    fn every_path_argument_says_what_it_resolves_against() {
        let mut checked = 0usize;
        let mut silent = Vec::new();
        for tool in MnemosyneServer::tool_router().list_all() {
            let Some(props) = tool
                .input_schema
                .get("properties")
                .and_then(|p| p.as_object())
            else {
                continue;
            };
            for (key, schema) in props {
                if !key.ends_with("_path") {
                    continue;
                }
                checked += 1;
                let described = schema
                    .get("description")
                    .and_then(|d| d.as_str())
                    .unwrap_or_default();
                if !described.to_lowercase().contains("workspace root") {
                    silent.push(format!("{}.{key}", tool.name));
                }
            }
        }
        assert!(
            checked > 0,
            "no tool exposes a `*_path` argument, so this gate reads nothing"
        );
        assert!(
            silent.is_empty(),
            "{} of {checked} path argument(s) do not tell an agent what they are \
             relative to. Since Round 1002 that is the workspace root, which is \
             the only base an MCP caller can see: {silent:?}",
            silent.len()
        );
    }

    /// NO AGENT-FACING OPTIONAL ARGUMENT IS UNACCOUNTED FOR, AND THE POPULATION
    /// IS THE ROUTER'S RATHER THAN A LIST SOMEBODY TYPED.
    ///
    /// Round 981 found that `population_census` had two write paths and that the
    /// second had never been executed — a source scan cannot see an argument
    /// that deserializes and is then read into nothing. Round 986 measured how
    /// wide that is and how much of it something else already catches. The
    /// answer to the second half is NOTHING:
    ///
    /// - THE COMPILER DOES NOT CATCH IT. Dropping the read of one field while
    ///   its siblings are still read leaves `cargo clippy -D warnings` at exit
    ///   0. Removing `Debug` from the derive does not change that, because
    ///   `Deserialize` constructs the field and rustc counts that as a use, so
    ///   `dead_code` never fires. The compiler only speaks when the field was
    ///   the handler's ONLY read, and then it complains about `args`.
    /// - NO TEST CATCHES IT. With `query_term` stopped from reading
    ///   `case_insensitive`, the whole suite stays green: an agent asking for a
    ///   case-insensitive search gets a case-sensitive one, reported as success.
    ///
    /// So this is the accounting that makes the population impossible to lose.
    /// A new tool, or a new optional argument on an old one, lands in neither
    /// list and FAILS HERE — it cannot join the silent set by being added. The
    /// two lists are printed on every run, because a check that reports only
    /// violations reads exactly like one that has nothing to report (Round 854).
    ///
    /// It also refutes, in place, the reason the R680 router smoke gave for
    /// stopping at presence: "the tools are thin `run_mutate` wrappers over
    /// CLI-exercised atomic paths". Thin is not the same as faithful, and the
    /// wire Round 981 found broken-by-construction was one of these.
    #[test]
    fn every_optional_tool_argument_is_declared_exercised_or_not() {
        let population = optional_arguments();
        assert!(
            !population.is_empty(),
            "the router exposes no optional argument at all, so this gate is \
             reading nothing and its silence proves nothing"
        );
        assert!(
            !EXERCISED.is_empty(),
            "no pair is claimed exercised, so the accounting has no floor"
        );

        let declared: BTreeSet<(&str, &str)> = EXERCISED
            .iter()
            .chain(EXERCISED_BESPOKE)
            .chain(UNEXERCISED)
            .copied()
            .collect();
        assert_eq!(
            declared.len(),
            EXERCISED.len() + EXERCISED_BESPOKE.len() + UNEXERCISED.len(),
            "a pair is declared twice, so one of the two lists is lying about it"
        );

        let live: BTreeSet<(&str, &str)> = population
            .iter()
            .map(|(t, a)| (t.as_str(), a.as_str()))
            .collect();
        let undeclared: Vec<&(&str, &str)> =
            declared.iter().filter(|p| !live.contains(*p)).collect();
        assert!(
            undeclared.is_empty(),
            "{} declared pair(s) are not optional arguments of any routed tool, \
             so this accounting describes a surface that no longer exists: \
             {undeclared:?}",
            undeclared.len()
        );

        let unaccounted: Vec<&(&str, &str)> =
            live.iter().filter(|p| !declared.contains(*p)).collect();
        assert!(
            unaccounted.is_empty(),
            "{} of {} agent-facing optional argument(s) are in neither list. An \
             argument that nothing proves the handler reads must be named as \
             such, not left to be discovered by an agent whose call quietly did \
             less than it asked for: {unaccounted:?}",
            unaccounted.len(),
            live.len()
        );

        println!(
            "MCP optional arguments: {} exercised / {} unexercised, of {} on the router",
            EXERCISED.len() + EXERCISED_BESPOKE.len(),
            UNEXERCISED.len(),
            live.len()
        );
        for (tool, arg) in UNEXERCISED {
            println!("  unexercised: {tool}.{arg}");
        }
    }

    /// A REFUSED IMPORT LEAVES THE STORE EXACTLY AS IT FOUND IT.
    ///
    /// `import_facts` earns its place in the contract by being all-or-nothing —
    /// "N separate `add_*` calls are non-atomic, so a mid-sequence failure
    /// leaves a partial store". Round 993 proved every one of its ten sections
    /// arrives and wrote in its own carry that nothing tested the atomicity,
    /// down to the shape the test should take. That is a design in a sentence
    /// rather than a test, which is the thing this session keeps finding.
    ///
    /// The manifest here is valid up to its last fact, which names a canon
    /// coordinate that does not exist. A handler that applied sections as it
    /// went would leave the frames, the kinds, the entities, the predicate and
    /// the first fact behind; the assertion is that the store is BYTE-IDENTICAL
    /// to what it was before the call.
    ///
    /// NON-VACUITY: the same manifest with the bad coordinate repaired must
    /// land, so the refusal is about that one row and not about a manifest this
    /// workspace could never accept.
    #[tokio::test]
    async fn a_refused_import_leaves_the_store_untouched() {
        let manifest = |canon_from: &str| {
            serde_json::json!({
                "frames": [{"frame_id": "ground-truth"}],
                "entity_kinds": [{"kind_id": "place"}],
                "entities": [{"entity_id": "p-a", "kind": "place"}],
                "predicates": [{"predicate_id": "condition", "object_kind": "token",
                                "object_tokens": ["lit"], "subject_kind": "place"}],
                "facts": [
                    {"fact_id": "f-good", "frame": "ground-truth", "claim": "the lamp is lit",
                     "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["p-a"]},
                    {"fact_id": "f-last", "frame": "ground-truth", "claim": "the last row",
                     "canon_from": canon_from, "evidence": ["sc-01"], "entities": ["p-a"]},
                ],
            })
        };

        for (what, canon_from, refused) in [
            (
                "a manifest whose last fact names no such scene",
                "sc-nonexistent",
                true,
            ),
            ("the same manifest with that row repaired", "sc-01", false),
        ] {
            let tmp = agent_workspace();
            let ws = tmp.path();
            let server = MnemosyneServer::new(ws.to_path_buf()).expect("server");
            let sections: ImportSectionsArgs = serde_json::from_value(serde_json::json!({
                "sections": [{"section_id": "sc-01", "parent_doc": "spec", "title": "scene one"}],
            }))
            .expect("sections parse");
            assert!(
                server.import_sections(Parameters(sections)).await.is_error != Some(true),
                "seeding the canon coordinate failed"
            );

            let store_path = ws.join("docs/.atomic/workspace.atomic.json");
            let before = std::fs::read_to_string(&store_path).expect("read the store");
            let args: atomic::FactsManifest =
                serde_json::from_value(manifest(canon_from)).expect("manifest parse");
            let result = server.import_facts(Parameters(args)).await;
            let after = std::fs::read_to_string(&store_path).expect("read the store");

            assert_eq!(
                result.is_error == Some(true),
                refused,
                "{what}: the import was {} — {:?}",
                if refused { "accepted" } else { "refused" },
                result.content
            );
            if refused {
                assert_eq!(
                    before, after,
                    "{what}: the store changed. A refused import must leave \
                     nothing behind, and the rows before the bad one are exactly \
                     what a non-atomic handler would have kept"
                );
            } else {
                assert_ne!(
                    before, after,
                    "{what}: the store did not change, so the refusal above says \
                     nothing about atomicity"
                );
            }
        }
    }

    /// Replace `{ws}` with the workspace root everywhere in a value.
    fn substitute_workspace(value: &mut serde_json::Value, root: &std::path::Path) {
        match value {
            serde_json::Value::String(s) => {
                if s.contains("{ws}") {
                    *s = s.replace("{ws}", &root.to_string_lossy());
                }
            }
            serde_json::Value::Array(items) => {
                for item in items {
                    substitute_workspace(item, root);
                }
            }
            serde_json::Value::Object(map) => {
                for (_, v) in map.iter_mut() {
                    substitute_workspace(v, root);
                }
            }
            _ => {}
        }
    }

    /// `import_sections` MAKES THE SAME ALL-OR-NOTHING PROMISE AND KEEPS IT.
    ///
    /// Round 996 wrote the atomicity test for `import_facts` and closed with
    /// "the case above is a template and the second one is a copy with a
    /// different manifest" — a sentence that describes work rather than doing
    /// it, which is the shape this session keeps paying for. It is the copy.
    ///
    /// The manifest's last row names a parent section that does not exist; a
    /// handler that applied rows as it went would leave the first behind.
    #[tokio::test]
    async fn a_refused_section_import_leaves_the_store_untouched() {
        for (what, parent, refused) in [
            (
                "a manifest whose last row names no such parent",
                "no-such-section",
                true,
            ),
            (
                "a manifest whose MIDDLE row names no such parent",
                "middle",
                true,
            ),
            ("the same manifest with that row repaired", "40", false),
        ] {
            let tmp = agent_workspace();
            let ws = tmp.path();
            let server = MnemosyneServer::new(ws.to_path_buf()).expect("server");
            let store_path = ws.join("docs/.atomic/workspace.atomic.json");
            let before = std::fs::read_to_string(&store_path).expect("read the store");

            // Round 1001 — a bad row in the MIDDLE as well as at the end.
            // Round 999 argued the two are the same because both verbs commit
            // through one primitive; this session's own lesson is that an
            // argument about behaviour is not a run of it, and the middle case
            // is the one where a row-at-a-time handler leaves rows on BOTH
            // sides of the failure.
            let sections = if parent == "middle" {
                serde_json::json!([
                    {"section_id": "40", "parent_doc": "spec", "title": "the first"},
                    {"section_id": "41", "parent_doc": "spec", "title": "the middle",
                     "parent_section": "no-such-section"},
                    {"section_id": "42", "parent_doc": "spec", "title": "the last"},
                ])
            } else {
                serde_json::json!([
                    {"section_id": "40", "parent_doc": "spec", "title": "the first"},
                    {"section_id": "41", "parent_doc": "spec", "title": "the last",
                     "parent_section": parent},
                ])
            };
            let args: ImportSectionsArgs =
                serde_json::from_value(serde_json::json!({ "sections": sections }))
                    .expect("manifest parse");
            let result = server.import_sections(Parameters(args)).await;
            let after = std::fs::read_to_string(&store_path).expect("read the store");

            assert_eq!(
                result.is_error == Some(true),
                refused,
                "{what}: the import was {} — {:?}",
                if refused { "accepted" } else { "refused" },
                result.content
            );
            if refused {
                assert_eq!(
                    before, after,
                    "{what}: the store changed. Section 40 is exactly what a \
                     row-at-a-time handler would have left behind"
                );
            } else {
                assert_ne!(
                    before, after,
                    "{what}: the store did not change, so the refusal above says \
                     nothing about atomicity"
                );
            }
        }
    }

    /// THE MANIFEST PATH AN AGENT SENDS IS THE ONE THAT GETS READ.
    ///
    /// Found by injection while closing Round 1000's carry: replacing
    /// `propose_verdict`'s manifest path with a fixed nonsense string left the
    /// whole suite green, because nothing in this repository had ever run that
    /// tool. It is the generate-gate-repair loop's gate — the one an agent is
    /// told to call before writing for real — so a wire that read the wrong
    /// file would hand back a verdict about something the agent never sent.
    ///
    /// Both directions: the manifest the agent named is what the verdict is
    /// about, and a path that names nothing fails loud rather than answering.
    #[tokio::test]
    async fn propose_verdict_reads_the_manifest_the_agent_named() {
        let tmp = agent_workspace();
        let ws = tmp.path();
        let server = MnemosyneServer::new(ws.to_path_buf()).expect("server");
        let sections: ImportSectionsArgs = serde_json::from_value(serde_json::json!({
            "sections": [{"section_id": "sc-01", "parent_doc": "spec", "title": "scene one"}],
        }))
        .expect("sections parse");
        assert!(server.import_sections(Parameters(sections)).await.is_error != Some(true));

        // A manifest whose only fact names a canon coordinate that does not
        // exist: the gate must say so, and saying so proves it read THIS file.
        std::fs::write(
            ws.join("candidate.json"),
            serde_json::json!({
                "frames": [{"frame_id": "ground-truth"}],
                "facts": [{"fact_id": "f-1", "frame": "ground-truth", "claim": "she waits",
                           "canon_from": "sc-nowhere", "evidence": ["sc-01"]}],
            })
            .to_string(),
        )
        .expect("write the candidate manifest");

        let named: ProposeVerdictArgs = serde_json::from_value(serde_json::json!({
            "manifest_path": ws.join("candidate.json").to_string_lossy(),
        }))
        .expect("args parse");
        let said = answer_text(&server.propose_verdict(Parameters(named)).await);
        assert!(
            said.contains("sc-nowhere"),
            "the verdict says nothing about the manifest the agent named, so \
             this tool may be reading some other file: {said}"
        );

        // Non-vacuity in the other direction: a path naming nothing is refused
        // rather than answered, so the assertion above is about WHICH file was
        // read and not about the tool always mentioning its input.
        let missing: ProposeVerdictArgs = serde_json::from_value(serde_json::json!({
            "manifest_path": ws.join("no-such-manifest.json").to_string_lossy(),
        }))
        .expect("args parse");
        let result = server.propose_verdict(Parameters(missing)).await;
        assert_eq!(
            result.is_error,
            Some(true),
            "a manifest path naming nothing was answered instead of refused: {:?}",
            result.content
        );
    }

    /// Everything a tool said, joined — the read-side counterpart of the store
    /// bytes.
    fn answer_text(result: &CallToolResult) -> String {
        result
            .content
            .iter()
            .filter_map(|c| c.as_text().map(|t| t.text.clone()))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// A workspace with an empty store, for driving a tool the way an agent
    /// does.
    fn agent_workspace() -> tempfile::TempDir {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join("docs/.atomic")).expect("atomic dir");
        std::fs::write(tmp.path().join("mnemosyne.toml"), "[workspace]\n").expect("config");
        std::fs::write(
            tmp.path().join("docs/.atomic/workspace.atomic.json"),
            format!(
                "{{\"schema_version\":{},\"sections\":{{}},\"changelog_entries\":{{}}}}",
                atomic::CURRENT_SCHEMA_VERSION
            ),
        )
        .expect("store");
        tmp
    }

    /// THE PATTERN EVERY ENTRY IN `EXERCISED` FOLLOWS: AN OPTIONAL ARGUMENT AN
    /// AGENT SENDS CHANGES WHAT LANDS IN THE STORE.
    ///
    /// DIFFERENTIAL RATHER THAN A SINGLE CALL, and that is the load-bearing
    /// part: a tool that ignores the argument still succeeds and still writes a
    /// record, so asserting on one store proves only that the tool ran. Two
    /// workspaces, identical but for the argument, may not end up with the same
    /// bytes.
    ///
    /// The needle is asserted BESIDE the difference, because the two catch
    /// different failures — a handler that writes the argument into the wrong
    /// place still changes the bytes, and a handler that stamps a timestamp
    /// changes them without reading anything.
    ///
    /// ONE INVOCATION EMITS BOTH THE TEST AND ITS `EXERCISED` ROW, which is the
    /// hole this closes rather than a convenience. Round 986 shipped `EXERCISED`
    /// as a hand-written list, so it was a CLAIM that a test existed and nothing
    /// checked it — a pair could be moved out of the silent set by editing one
    /// line. Now the row cannot exist without the test and the test cannot exist
    /// without the row, and the compiler is what holds the two together.
    macro_rules! exercised {
        ($(
            $test:ident :
            $( {$file:literal = $contents:tt} )*
            $( [$setup:ident($setup_args:ty) $setup_base:tt] )*
            $tool:ident($args:ty) $base:tt . $field:literal = $value:tt seen $needle:literal in $oracle:ident;
        )*) => {
            const EXERCISED: &[(&str, &str)] = &[$((stringify!($tool), $field)),*];
            $(
                #[tokio::test]
                async fn $test() {
                    let mut written = Vec::new();
                    // Whether the tool itself wrote anything, observed rather
                    // than taken from the case's word for it.
                    let mut wrote = false;
                    for with in [false, true] {
                        let tmp = agent_workspace();
                        let mut json = serde_json::json!($base);
                        if with {
                            json[$field] = serde_json::json!($value);
                        }
                        // `{ws}` IN ANY STRING BECOMES THE WORKSPACE ROOT. A
                        // path argument is resolved by the tool against the
                        // SERVER PROCESS's working directory and not against
                        // the workspace it was handed, so a relative literal
                        // here would read a file beside this source rather than
                        // the one the setup wrote.
                        substitute_workspace(&mut json, tmp.path());
                        let args: $args = serde_json::from_value(json)
                            .expect("the agent-facing shape must parse this call");
                        let server =
                            MnemosyneServer::new(tmp.path().to_path_buf()).expect("server");
                        // WHAT THE ARGUMENT NEEDS TO EXIST BEFORE IT CAN BE
                        // SENT. Run in BOTH arms, so the difference the test
                        // asserts is the argument and never the setup.
                        // FILES THE ARGUMENT NAMES OR THE TOOL READS BEHIND
                        // THE STORE — a canon order, a rules artifact, a
                        // proposals file, or a store edited OUT OF BAND, which
                        // is the only thing `refresh` has to pick up. Written
                        // before the setup calls and in both arms.
                        $(
                            std::fs::write(
                                tmp.path().join($file),
                                serde_json::to_string(&serde_json::json!($contents))
                                    .expect("the given file must serialize"),
                            )
                            .unwrap_or_else(|e| panic!("write {}: {e}", $file));
                        )*
                        $(
                            let setup: $setup_args =
                                serde_json::from_value(serde_json::json!($setup_base))
                                    .expect("the setup call's shape must parse");
                            let ready = server.$setup(Parameters(setup)).await;
                            assert!(
                                ready.is_error != Some(true),
                                "the `given` call {} failed, so {} was never \
                                 reached: {:?}",
                                stringify!($setup),
                                stringify!($tool),
                                ready.content
                            );
                        )*
                        let store_path = tmp.path().join("docs/.atomic/workspace.atomic.json");
                        let before_call =
                            std::fs::read_to_string(&store_path).expect("read the store");
                        let result = server.$tool(Parameters(args)).await;
                        // AN ARGUMENT THE TOOL'S OWN CONTRACT MAKES CONDITIONAL
                        // CANNOT BE OMITTED AND STILL LEAVE A VALID CALL — one
                        // half of an exactly-one-of pair has no arm without it.
                        // `in outcome` is that case: the call WITHOUT the
                        // argument must be refused and the call with it must
                        // succeed, so the argument is still what the two arms
                        // differ by.
                        if stringify!($oracle) == "outcome" {
                            assert_eq!(
                                result.is_error == Some(true),
                                !with,
                                "{} is declared `in outcome`, which means the \
                                 call is refused without `{}` and accepted with \
                                 it; at {}={with} it did the opposite: {:?}",
                                stringify!($tool),
                                $field,
                                $field,
                                result.content
                            );
                        } else {
                            assert!(
                                result.is_error != Some(true),
                                "{} failed with {}={with}: {:?}",
                                stringify!($tool),
                                $field,
                                result.content
                            );
                        }
                        wrote |= std::fs::read_to_string(&store_path)
                            .expect("read the store")
                            != before_call;
                        // WHICH ORACLE. A mutating tool is judged by what it
                        // left in the store; a READ tool never touches the
                        // store, so the only thing an argument can change is
                        // the answer, and asking the store about it would
                        // compare two identical files forever.
                        written.push(match stringify!($oracle) {
                            "store" => std::fs::read_to_string(&store_path)
                                .expect("read the store"),
                            "output" | "outcome" => answer_text(&result),
                            other => panic!(
                                "`{other}` is not an oracle this macro knows; \
                                 write `store` or `output`"
                            ),
                        });
                    }
                    // THE DECLARED ORACLE IS CHECKED AGAINST WHAT THE TOOL
                    // ACTUALLY DID (Round 991). Round 989 made the oracle a word
                    // the case author writes and left it unchecked, calling the
                    // wrong choice "loud rather than silent" — but the noise it
                    // makes is a failure that reads as a defect in the HANDLER
                    // when it is a defect in the CASE, which is the most
                    // expensive kind of wrong message. It is observable, so it
                    // is observed.
                    match stringify!($oracle) {
                        "store" => assert!(
                            wrote,
                            "{} is declared `in store` and wrote nothing in \
                             either arm, so this case compares two identical \
                             files and can only ever fail",
                            stringify!($tool)
                        ),
                        // `outcome` is judged on the answer, and the arm that
                        // succeeds may well write; nothing more is asserted
                        // about it than that the two arms differ in acceptance.
                        "outcome" => {}
                        "output" => assert!(
                            !wrote,
                            "{} is declared `in output` and DOES write the \
                             store, so the store is the stronger oracle and \
                             this case is using the weaker one",
                            stringify!($tool)
                        ),
                        other => panic!("`{other}` is not an oracle this macro knows"),
                    }
                    // MORE OCCURRENCES, NOT MERELY PRESENT. A ref-shaped
                    // argument names something the setup already registered, so
                    // `contains` is true in BOTH arms and asserts nothing; the
                    // id appears once for the registration and twice once
                    // something references it. Counting discriminates for both
                    // shapes, where presence discriminates only for fresh prose.
                    let (before, after) = (
                        written[0].matches($needle).count(),
                        written[1].matches($needle).count(),
                    );
                    // DIFFERENT, NOT MORE. An argument may ADD to what is
                    // observed (a description reaching the store) or take away
                    // from it (a `limit` dropping a row), so demanding growth
                    // false-rejects every filter. What has to be true either
                    // way is that the value this case NAMES moved — which is
                    // stricter than the whole-text difference asserted below,
                    // because that one passes on any change at all.
                    // WHAT THE TEST CAN AND CANNOT TELL APART (Round 996).
                    // Round 995 hit a needle that did not move while the
                    // argument HAD reached the store:
                    // `set_edge_guard_threshold` normalises a threshold equal
                    // to the condition count to `None`, so a correct handler
                    // left nothing to count and both arms came out identical.
                    // The message then said "what the agent sent did not reach
                    // it" and sent the reader to the handler. Round 995 called
                    // that "loud rather than silent" and left it; loud is not a
                    // defence when the noise names the wrong file.
                    //
                    // The honest split is narrow. When the two observations
                    // DIFFER, the argument demonstrably arrived and the needle
                    // is simply the wrong thing to watch — that is the case's
                    // defect and is stated as one. When they are IDENTICAL the
                    // test cannot tell an ignored argument from a value the
                    // store declines to represent, so it names both and gives
                    // the two commands that separate them rather than picking.
                    if after == before {
                        assert_eq!(
                            written[0], written[1],
                            "`{}` occurs {before} time(s) in the {} {} produced \
                             WITHOUT `{}` and {after} with it — but the {} DID \
                             change, so the argument arrived and this needle is \
                             not what moved. THE DEFECT IS IN THIS CASE: name a \
                             needle that follows the value",
                            $needle,
                            stringify!($oracle),
                            stringify!($tool),
                            $field,
                            stringify!($oracle)
                        );
                        panic!(
                            "`{}` occurs {before} time(s) in the {} {} produced \
                             WITHOUT `{}` and {after} with it, and the {} is \
                             identical either way. TWO CAUSES LOOK LIKE THIS AND \
                             THIS TEST CANNOT TELL THEM APART: (1) {} ignores \
                             `{}`, or (2) it reads it and the store declines to \
                             represent THIS VALUE — a value equal to a default \
                             is normalised away (Round 995: a K-of-N threshold \
                             equal to the condition count is stored as `None`). \
                             Separate them by running the CLI twin on a scratch \
                             workspace with two different values and seeing \
                             which one survives; if the value is the normalised \
                             one, the defect is in this case",
                            $needle,
                            stringify!($oracle),
                            stringify!($tool),
                            $field,
                            stringify!($oracle),
                            stringify!($tool),
                            $field
                        );
                    }
                    // The whole-observation difference used to be asserted here
                    // beside the needle. It is gone rather than kept: a needle
                    // count that moved implies the observations differ, so the
                    // assertion could not fail and a check that cannot fail is
                    // decoration. The identical-observation case it used to
                    // catch is the branch above, which now says more.
                }
            )*
        };
    }

    exercised! {
        report_playable_world_order_path_reaches_the_answer:
            {"order-a.json" = {"schema": "canon-order/v1", "edges": [["sc-01", "sc-02"]]}}
            {"order-b.json" = {"schema": "canon-order/v1", "edges": [["sc-01", "sc-02"], ["sc-02", "sc-03"]]}}
            [import_sections(ImportSectionsArgs) {"sections": [{"section_id": "sc-01", "parent_doc": "spec", "title": "one"}, {"section_id": "sc-02", "parent_doc": "spec", "title": "two"}, {"section_id": "sc-03", "parent_doc": "spec", "title": "three"}]}]
            [import_facts(atomic::FactsManifest) {"frames": [{"frame_id": "ground-truth"}], "entity_kinds": [{"kind_id": "place"}, {"kind_id": "character"}], "entities": [{"entity_id": "p-a", "kind": "place"}, {"entity_id": "p-b", "kind": "place"}, {"entity_id": "e-her", "kind": "character"}], "predicates": [{"predicate_id": "adjacent", "object_kind": "entity", "subject_kind": "place", "object_entity_kind": "place"}, {"predicate_id": "at", "object_kind": "entity", "subject_kind": "character", "object_entity_kind": "place"}], "facts": [{"fact_id": "f-way", "frame": "ground-truth", "claim": "a way runs", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["p-a", "p-b"], "typed": {"subject": "p-a", "predicate": "adjacent", "object": {"kind": "entity", "id": "p-b"}}}, {"fact_id": "f-at-a", "frame": "ground-truth", "claim": "she is at a", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["e-her", "p-a"], "typed": {"subject": "e-her", "predicate": "at", "object": {"kind": "entity", "id": "p-a"}}}, {"fact_id": "f-at-b", "frame": "ground-truth", "claim": "she is at b", "canon_from": "sc-03", "evidence": ["sc-03"], "entities": ["e-her", "p-b"], "supersedes_in_frame": "f-at-a", "typed": {"subject": "e-her", "predicate": "at", "object": {"kind": "entity", "id": "p-b"}}}], "disclosure_plans": [{"telling_id": "t-quiet", "default_mode": "state"}]}]
            report_playable_world(ReportPlayableWorldArgs) {"telling": "t-quiet", "order_path": "{ws}/order-a.json"}
            ."order_path" = "order-b.json" seen "sc-03" in output;
        query_term_scope_reaches_the_answer:
            [import_sections(ImportSectionsArgs) {"sections": [{"section_id": "40", "parent_doc": "spec", "title": "the section"}, {"section_id": "41", "parent_doc": "spec", "title": "the other"}]}]
            [append_changelog_entry(AppendChangelogEntryArgs) {"entry_id": "Round 1", "decision_summary": "the one who Waits", "changes_bullets": ["a change"], "verification_bullets": ["a check"]}]
            query_term(QueryTermArgs) {"pattern": "Waits"}
            ."scope" = "sections" seen "Round 1" in output;
        query_section_include_related_reaches_the_answer:
            [import_sections(ImportSectionsArgs) {"sections": [{"section_id": "40", "parent_doc": "spec", "title": "the section"}, {"section_id": "41", "parent_doc": "spec", "title": "the other"}]}]
            [set_section_parent_section(SetSectionParentSectionArgs) {"section_id": "41", "parent_section": "40"}]
            query_section(QuerySectionArgs) {"section_id": "40"}
            ."include_related" = true seen "related" in output;
        redact_term_dry_run_reaches_the_answer:
            [append_changelog_entry(AppendChangelogEntryArgs) {"entry_id": "Round 1", "decision_summary": "the one who Waits", "changes_bullets": ["a change"], "verification_bullets": ["a check"]}]
            redact_term(RedactTermArgs) {"pattern": "Waits", "replacement": "waits", "reason": "case", "applied_in": "Round 1"}
            ."dry_run" = true seen "waits" in store;
        redact_term_case_insensitive_reaches_the_answer:
            [append_changelog_entry(AppendChangelogEntryArgs) {"entry_id": "Round 1", "decision_summary": "the one who Waits", "changes_bullets": ["a change"], "verification_bullets": ["a check"]}]
            redact_term(RedactTermArgs) {"pattern": "waits", "replacement": "lingers", "reason": "word", "applied_in": "Round 1", "dry_run": true}
            ."case_insensitive" = true seen "Round 1" in output;
        redact_term_regex_reaches_the_answer:
            [append_changelog_entry(AppendChangelogEntryArgs) {"entry_id": "Round 1", "decision_summary": "the one who Waits", "changes_bullets": ["a change"], "verification_bullets": ["a check"]}]
            redact_term(RedactTermArgs) {"pattern": "W.its", "replacement": "lingers", "reason": "word", "applied_in": "Round 1", "dry_run": true}
            ."regex" = true seen "Round 1" in output;
        validate_projection_refresh_reaches_the_answer:
            {"docs/.atomic/workspace.atomic.json" = {
                "schema_version": atomic::CURRENT_SCHEMA_VERSION,
                "changelog_entries": {},
                "sections": {"40": {
                    "section_id": "40", "parent_doc": "spec", "title": "the section",
                    "decision_status": "superseded"
                }}
            }}
            validate_projection(ValidateProjectionArgs) {}
            ."refresh" = true seen "VIOLATIONS" in output;
        query_term_case_insensitive_reaches_the_answer:
            [append_changelog_entry(AppendChangelogEntryArgs) {"entry_id": "Round 1", "decision_summary": "the one who Waits", "changes_bullets": ["a change"], "verification_bullets": ["a check"]}]
            query_term(QueryTermArgs) {"pattern": "waits"}
            ."case_insensitive" = true seen "Round 1" in output;
        query_term_regex_reaches_the_answer:
            [append_changelog_entry(AppendChangelogEntryArgs) {"entry_id": "Round 1", "decision_summary": "the one who Waits", "changes_bullets": ["a change"], "verification_bullets": ["a check"]}]
            query_term(QueryTermArgs) {"pattern": "W.its"}
            ."regex" = true seen "Round 1" in output;
        query_section_include_changelog_reaches_the_answer:
            [add_section(AddSectionArgs) {"section_id": "40", "parent_doc": "spec", "title": "the section"}]
            [append_changelog_entry(AppendChangelogEntryArgs) {"entry_id": "Round 1", "decision_summary": "a decision naming the section", "changes_bullets": ["a change"], "verification_bullets": ["a check"], "impact_refs": ["40"]}]
            query_section(QuerySectionArgs) {"section_id": "40"}
            ."include_changelog" = true seen "Round 1" in output;
        list_changelog_limit_reaches_the_answer:
            [append_changelog_entry(AppendChangelogEntryArgs) {"entry_id": "Round 1", "decision_summary": "the first", "changes_bullets": ["a change"], "verification_bullets": ["a check"]}]
            [append_changelog_entry(AppendChangelogEntryArgs) {"entry_id": "Round 2", "decision_summary": "the second", "changes_bullets": ["a change"], "verification_bullets": ["a check"]}]
            list_changelog(ListChangelogArgs) {}
            ."limit" = 1 seen "Round 1" in output;
        add_fact_branch_reaches_the_store:
            [add_frame(AddFrameArgs) {"frame_id": "ground-truth"}]
            [add_section(AddSectionArgs) {"section_id": "sc-01", "parent_doc": "spec", "title": "scene one"}]
            [add_branch(AddBranchArgs) {"branch_id": "b-what-if"}]
            add_fact(atomic::FactImport) {"fact_id": "f-1", "frame": "ground-truth", "claim": "she waits", "canon_from": "sc-01", "evidence": ["sc-01"]}
            ."branch" = "b-what-if" seen "b-what-if" in store;
        add_fact_supersedes_in_frame_reaches_the_store:
            [add_frame(AddFrameArgs) {"frame_id": "ground-truth"}]
            [add_section(AddSectionArgs) {"section_id": "sc-01", "parent_doc": "spec", "title": "scene one"}]
            [add_fact(atomic::FactImport) {"fact_id": "f-0", "frame": "ground-truth", "claim": "she arrives", "canon_from": "sc-01", "evidence": ["sc-01"]}]
            add_fact(atomic::FactImport) {"fact_id": "f-1", "frame": "ground-truth", "claim": "she waits", "canon_from": "sc-01", "evidence": ["sc-01"]}
            ."supersedes_in_frame" = "f-0" seen "f-0" in store;
        add_fact_conflicts_with_reaches_the_store:
            [add_frame(AddFrameArgs) {"frame_id": "ground-truth"}]
            [add_section(AddSectionArgs) {"section_id": "sc-01", "parent_doc": "spec", "title": "scene one"}]
            [add_fact(atomic::FactImport) {"fact_id": "f-0", "frame": "ground-truth", "claim": "she arrives", "canon_from": "sc-01", "evidence": ["sc-01"]}]
            add_fact(atomic::FactImport) {"fact_id": "f-1", "frame": "ground-truth", "claim": "she waits", "canon_from": "sc-01", "evidence": ["sc-01"]}
            ."conflicts_with" = ["f-0"] seen "f-0" in store;
        add_fact_payoff_expectation_reaches_the_store:
            [add_frame(AddFrameArgs) {"frame_id": "ground-truth"}]
            [add_section(AddSectionArgs) {"section_id": "sc-01", "parent_doc": "spec", "title": "scene one"}]
            add_fact(atomic::FactImport) {"fact_id": "f-1", "frame": "ground-truth", "claim": "she waits", "canon_from": "sc-01", "evidence": ["sc-01"]}
            ."payoff_expectation" = "expected" seen "expected" in store;
        add_fact_pays_off_reaches_the_store:
            [add_frame(AddFrameArgs) {"frame_id": "ground-truth"}]
            [add_section(AddSectionArgs) {"section_id": "sc-01", "parent_doc": "spec", "title": "scene one"}]
            [add_fact(atomic::FactImport) {"fact_id": "f-0", "frame": "ground-truth", "claim": "she arrives", "canon_from": "sc-01", "evidence": ["sc-01"]}]
            add_fact(atomic::FactImport) {"fact_id": "f-1", "frame": "ground-truth", "claim": "she waits", "canon_from": "sc-01", "evidence": ["sc-01"]}
            ."pays_off" = ["f-0"] seen "f-0" in store;
        amend_fact_canon_to_reaches_the_store:
            [add_frame(AddFrameArgs) {"frame_id": "ground-truth"}]
            [add_section(AddSectionArgs) {"section_id": "sc-01", "parent_doc": "spec", "title": "scene one"}]
            [add_section(AddSectionArgs) {"section_id": "sc-02", "parent_doc": "spec", "title": "scene two"}]
            [add_fact(atomic::FactImport) {"fact_id": "f-1", "frame": "ground-truth", "claim": "she waits", "canon_from": "sc-01", "evidence": ["sc-01"]}]
            amend_fact(AmendFactArgs) {"fact_id": "f-1", "frame": "ground-truth", "claim": "she waits still", "canon_from": "sc-01", "evidence": ["sc-01"], "reason": "revised"}
            ."canon_to" = "sc-02" seen "sc-02" in store;
        amend_fact_payoff_expectation_reaches_the_store:
            [add_frame(AddFrameArgs) {"frame_id": "ground-truth"}]
            [add_section(AddSectionArgs) {"section_id": "sc-01", "parent_doc": "spec", "title": "scene one"}]
            [add_fact(atomic::FactImport) {"fact_id": "f-1", "frame": "ground-truth", "claim": "she waits", "canon_from": "sc-01", "evidence": ["sc-01"]}]
            amend_fact(AmendFactArgs) {"fact_id": "f-1", "frame": "ground-truth", "claim": "she waits still", "canon_from": "sc-01", "evidence": ["sc-01"], "reason": "revised"}
            ."payoff_expectation" = "expected" seen "expected" in store;
        amend_fact_branch_reaches_the_store:
            [add_frame(AddFrameArgs) {"frame_id": "ground-truth"}]
            [add_section(AddSectionArgs) {"section_id": "sc-01", "parent_doc": "spec", "title": "scene one"}]
            [add_branch(AddBranchArgs) {"branch_id": "b-what-if"}]
            [add_fact(atomic::FactImport) {"fact_id": "f-1", "frame": "ground-truth", "claim": "she waits", "canon_from": "sc-01", "evidence": ["sc-01"]}]
            amend_fact(AmendFactArgs) {"fact_id": "f-1", "frame": "ground-truth", "claim": "she waits still", "canon_from": "sc-01", "evidence": ["sc-01"], "reason": "revised"}
            ."branch" = "b-what-if" seen "b-what-if" in store;
        amend_fact_supersedes_in_frame_reaches_the_store:
            [add_frame(AddFrameArgs) {"frame_id": "ground-truth"}]
            [add_section(AddSectionArgs) {"section_id": "sc-01", "parent_doc": "spec", "title": "scene one"}]
            [add_fact(atomic::FactImport) {"fact_id": "f-0", "frame": "ground-truth", "claim": "she arrives", "canon_from": "sc-01", "evidence": ["sc-01"]}]
            [add_fact(atomic::FactImport) {"fact_id": "f-1", "frame": "ground-truth", "claim": "she waits", "canon_from": "sc-01", "evidence": ["sc-01"]}]
            amend_fact(AmendFactArgs) {"fact_id": "f-1", "frame": "ground-truth", "claim": "she waits still", "canon_from": "sc-01", "evidence": ["sc-01"], "reason": "revised"}
            ."supersedes_in_frame" = "f-0" seen "f-0" in store;
        amend_fact_conflicts_with_reaches_the_store:
            [add_frame(AddFrameArgs) {"frame_id": "ground-truth"}]
            [add_section(AddSectionArgs) {"section_id": "sc-01", "parent_doc": "spec", "title": "scene one"}]
            [add_fact(atomic::FactImport) {"fact_id": "f-0", "frame": "ground-truth", "claim": "she arrives", "canon_from": "sc-01", "evidence": ["sc-01"]}]
            [add_fact(atomic::FactImport) {"fact_id": "f-1", "frame": "ground-truth", "claim": "she waits", "canon_from": "sc-01", "evidence": ["sc-01"]}]
            amend_fact(AmendFactArgs) {"fact_id": "f-1", "frame": "ground-truth", "claim": "she waits still", "canon_from": "sc-01", "evidence": ["sc-01"], "reason": "revised"}
            ."conflicts_with" = ["f-0"] seen "f-0" in store;
        amend_fact_pays_off_reaches_the_store:
            [add_frame(AddFrameArgs) {"frame_id": "ground-truth"}]
            [add_section(AddSectionArgs) {"section_id": "sc-01", "parent_doc": "spec", "title": "scene one"}]
            [add_fact(atomic::FactImport) {"fact_id": "f-0", "frame": "ground-truth", "claim": "she arrives", "canon_from": "sc-01", "evidence": ["sc-01"]}]
            [add_fact(atomic::FactImport) {"fact_id": "f-1", "frame": "ground-truth", "claim": "she waits", "canon_from": "sc-01", "evidence": ["sc-01"]}]
            amend_fact(AmendFactArgs) {"fact_id": "f-1", "frame": "ground-truth", "claim": "she waits still", "canon_from": "sc-01", "evidence": ["sc-01"], "reason": "revised"}
            ."pays_off" = ["f-0"] seen "f-0" in store;
        amend_fact_evidence_reaches_the_store:
            [add_frame(AddFrameArgs) {"frame_id": "ground-truth"}]
            [add_section(AddSectionArgs) {"section_id": "sc-01", "parent_doc": "spec", "title": "scene one"}]
            [add_section(AddSectionArgs) {"section_id": "sc-02", "parent_doc": "spec", "title": "scene two"}]
            [add_fact(atomic::FactImport) {"fact_id": "f-1", "frame": "ground-truth", "claim": "she waits", "canon_from": "sc-01", "evidence": ["sc-01"]}]
            amend_fact(AmendFactArgs) {"fact_id": "f-1", "frame": "ground-truth", "claim": "she waits still", "canon_from": "sc-01", "evidence": ["sc-01"], "reason": "revised"}
            ."evidence" = ["sc-01", "sc-02"] seen "sc-02" in store;
        add_fact_typed_reaches_the_store:
            [add_frame(AddFrameArgs) {"frame_id": "ground-truth"}]
            [add_section(AddSectionArgs) {"section_id": "sc-01", "parent_doc": "spec", "title": "scene one"}]
            [add_entity_kind(AddEntityKindArgs) {"kind_id": "character"}]
            [add_entity_kind(AddEntityKindArgs) {"kind_id": "place"}]
            [add_entity(AddEntityArgs) {"entity_id": "e-her", "kind": "character"}]
            [add_entity(AddEntityArgs) {"entity_id": "p-room", "kind": "place"}]
            [add_predicate(AddPredicateArgs) {"predicate_id": "at", "object_kind": "entity", "subject_kind": "character", "object_entity_kind": "place"}]
            add_fact(atomic::FactImport) {"fact_id": "f-1", "frame": "ground-truth", "claim": "she waits", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["e-her", "p-room"]}
            ."typed" = {"subject": "e-her", "predicate": "at", "object": {"kind": "entity", "id": "p-room"}} seen "p-room" in store;
        amend_fact_typed_reaches_the_store:
            [add_frame(AddFrameArgs) {"frame_id": "ground-truth"}]
            [add_section(AddSectionArgs) {"section_id": "sc-01", "parent_doc": "spec", "title": "scene one"}]
            [add_entity_kind(AddEntityKindArgs) {"kind_id": "character"}]
            [add_entity_kind(AddEntityKindArgs) {"kind_id": "place"}]
            [add_entity(AddEntityArgs) {"entity_id": "e-her", "kind": "character"}]
            [add_entity(AddEntityArgs) {"entity_id": "p-room", "kind": "place"}]
            [add_predicate(AddPredicateArgs) {"predicate_id": "at", "object_kind": "entity", "subject_kind": "character", "object_entity_kind": "place"}]
            [add_fact(atomic::FactImport) {"fact_id": "f-1", "frame": "ground-truth", "claim": "she waits", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["e-her", "p-room"]}]
            amend_fact(AmendFactArgs) {"fact_id": "f-1", "frame": "ground-truth", "claim": "she waits still", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["e-her", "p-room"], "reason": "revised"}
            ."typed" = {"subject": "e-her", "predicate": "at", "object": {"kind": "entity", "id": "p-room"}} seen "p-room" in store;
        import_facts_edge_costs_reaches_the_store:
            [import_sections(ImportSectionsArgs) {"sections": [{"section_id": "sc-01", "parent_doc": "spec", "title": "scene one"}]}]
            import_facts(atomic::FactsManifest) {"frames": [{"frame_id": "ground-truth"}], "units": [{"unit_id": "minute"}], "entity_kinds": [{"kind_id": "place"}], "entities": [{"entity_id": "p-a", "kind": "place"}, {"entity_id": "p-b", "kind": "place"}], "predicates": [{"predicate_id": "adjacent", "object_kind": "entity", "subject_kind": "place", "object_entity_kind": "place"}], "facts": [{"fact_id": "f-way", "frame": "ground-truth", "claim": "a way runs from a to b", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["p-a", "p-b"], "typed": {"subject": "p-a", "predicate": "adjacent", "object": {"kind": "entity", "id": "p-b"}}}, {"fact_id": "f-lamp", "frame": "ground-truth", "claim": "the lamp is lit", "canon_from": "sc-01", "evidence": ["sc-01"]}]}
            ."edge_costs" = [{"fact_id": "f-way", "n": 2, "unit": "minute"}] seen "minute" in store;
        import_facts_edge_guards_reaches_the_store:
            [import_sections(ImportSectionsArgs) {"sections": [{"section_id": "sc-01", "parent_doc": "spec", "title": "scene one"}]}]
            import_facts(atomic::FactsManifest) {"frames": [{"frame_id": "ground-truth"}], "units": [{"unit_id": "minute"}], "entity_kinds": [{"kind_id": "place"}], "entities": [{"entity_id": "p-a", "kind": "place"}, {"entity_id": "p-b", "kind": "place"}], "predicates": [{"predicate_id": "adjacent", "object_kind": "entity", "subject_kind": "place", "object_entity_kind": "place"}], "facts": [{"fact_id": "f-way", "frame": "ground-truth", "claim": "a way runs from a to b", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["p-a", "p-b"], "typed": {"subject": "p-a", "predicate": "adjacent", "object": {"kind": "entity", "id": "p-b"}}}, {"fact_id": "f-lamp", "frame": "ground-truth", "claim": "the lamp is lit", "canon_from": "sc-01", "evidence": ["sc-01"]}]}
            ."edge_guards" = [{"fact_id": "f-way", "conditions": ["f-lamp"], "threshold": 1}] seen "f-lamp" in store;
        remove_section_binding_symbol_reaches_the_store:
            [import_sections(ImportSectionsArgs) {"sections": [{"section_id": "40", "parent_doc": "spec", "title": "the section"}, {"section_id": "41", "parent_doc": "spec", "title": "the other"}]}]
            [add_section_binding(AddSectionBindingArgs) {"section_id": "40", "file": "src/lib.rs", "kind": "implements"}]
            [add_section_binding(AddSectionBindingArgs) {"section_id": "40", "file": "src/lib.rs", "symbol": "the_symbol", "kind": "implements"}]
            remove_section_binding(RemoveSectionBindingArgs) {"section_id": "40", "file": "src/lib.rs", "reason": "no longer bound"}
            ."symbol" = "the_symbol" seen "the_symbol" in store;
        set_section_binding_kind_symbol_reaches_the_store:
            [import_sections(ImportSectionsArgs) {"sections": [{"section_id": "40", "parent_doc": "spec", "title": "the section"}, {"section_id": "41", "parent_doc": "spec", "title": "the other"}]}]
            [add_section_binding(AddSectionBindingArgs) {"section_id": "40", "file": "src/lib.rs", "kind": "implements"}]
            [add_section_binding(AddSectionBindingArgs) {"section_id": "40", "file": "src/lib.rs", "symbol": "the_symbol", "kind": "verifies"}]
            set_section_binding_kind(SetSectionBindingKindArgs) {"section_id": "40", "file": "src/lib.rs", "kind": "references", "reason": "re-pointed"}
            ."symbol" = "the_symbol" seen "verifies" in store;
        add_inventory_entry_section_ref_reaches_the_store:
            [import_sections(ImportSectionsArgs) {"sections": [{"section_id": "40", "parent_doc": "spec", "title": "the section"}, {"section_id": "41", "parent_doc": "spec", "title": "the other"}]}]
            add_inventory_entry(AddInventoryEntryArgs) {"inventory_id": "inv-1", "status": "active"}
            ."section_ref" = "40" seen "40" in store;
        set_inventory_section_ref_reaches_the_store:
            [import_sections(ImportSectionsArgs) {"sections": [{"section_id": "40", "parent_doc": "spec", "title": "the section"}, {"section_id": "41", "parent_doc": "spec", "title": "the other"}]}]
            [add_inventory_entry(AddInventoryEntryArgs) {"inventory_id": "inv-1", "status": "active"}]
            set_inventory_section_ref(SetInventorySectionRefArgs) {"inventory_id": "inv-1", "section_ref": "40", "reason": "filed"}
            ."section_ref" = "41" seen "41" in store;
        set_inventory_section_ref_clear_reaches_the_store:
            [import_sections(ImportSectionsArgs) {"sections": [{"section_id": "40", "parent_doc": "spec", "title": "the section"}, {"section_id": "41", "parent_doc": "spec", "title": "the other"}]}]
            [add_inventory_entry(AddInventoryEntryArgs) {"inventory_id": "inv-1", "status": "active", "section_ref": "40"}]
            set_inventory_section_ref(SetInventorySectionRefArgs) {"inventory_id": "inv-1", "reason": "unfiled"}
            ."clear" = true seen "inv-1" in outcome;
        set_section_decision_status_superseding_reaches_the_store:
            [import_sections(ImportSectionsArgs) {"sections": [{"section_id": "40", "parent_doc": "spec", "title": "the section"}, {"section_id": "41", "parent_doc": "spec", "title": "the other"}]}]
            set_section_decision_status(SetSectionDecisionStatusArgs) {"section_id": "40", "status": "Superseded", "reason": "overtaken"}
            ."superseding" = "41" seen "superseding" in outcome;
        add_confirmation_event_file_reaches_the_store:
            [import_sections(ImportSectionsArgs) {"sections": [{"section_id": "40", "parent_doc": "spec", "title": "the section"}]}]
            add_confirmation_event(AddConfirmationEventArgs) {"section_id": "40", "confirmer_kind": "tool", "confirmer_id": "reviewer", "confirmer_version": "1", "method": "semantic_review", "verdict": "confirm", "authoring_run": "run-a", "confirming_run": "run-b", "rationale": "checked it", "timestamp": "2026-08-03T00:00:00Z"}
            ."file" = "src/lib.rs" seen "src/lib.rs" in store;
        add_confirmation_event_symbol_reaches_the_store:
            [import_sections(ImportSectionsArgs) {"sections": [{"section_id": "40", "parent_doc": "spec", "title": "the section"}]}]
            add_confirmation_event(AddConfirmationEventArgs) {"section_id": "40", "file": "src/lib.rs", "confirmer_kind": "tool", "confirmer_id": "reviewer", "confirmer_version": "1", "method": "semantic_review", "verdict": "confirm", "authoring_run": "run-a", "confirming_run": "run-b", "rationale": "checked it", "timestamp": "2026-08-03T00:00:00Z"}
            ."symbol" = "the_symbol" seen "the_symbol" in store;
        add_confirmation_event_code_sha256_reaches_the_store:
            [import_sections(ImportSectionsArgs) {"sections": [{"section_id": "40", "parent_doc": "spec", "title": "the section"}]}]
            add_confirmation_event(AddConfirmationEventArgs) {"section_id": "40", "file": "src/lib.rs", "confirmer_kind": "tool", "confirmer_id": "reviewer", "confirmer_version": "1", "method": "semantic_review", "verdict": "confirm", "authoring_run": "run-a", "confirming_run": "run-b", "rationale": "checked it", "timestamp": "2026-08-03T00:00:00Z"}
            ."code_sha256" = ["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"] seen "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" in store;
        add_confirmation_event_test_sha256_reaches_the_store:
            [import_sections(ImportSectionsArgs) {"sections": [{"section_id": "40", "parent_doc": "spec", "title": "the section"}]}]
            add_confirmation_event(AddConfirmationEventArgs) {"section_id": "40", "file": "src/lib.rs", "confirmer_kind": "tool", "confirmer_id": "reviewer", "confirmer_version": "1", "method": "semantic_review", "verdict": "confirm", "authoring_run": "run-a", "confirming_run": "run-b", "rationale": "checked it", "timestamp": "2026-08-03T00:00:00Z"}
            ."test_sha256" = ["bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"] seen "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb" in store;
        add_confirmation_event_spec_sha256_reaches_the_store:
            [import_sections(ImportSectionsArgs) {"sections": [{"section_id": "40", "parent_doc": "spec", "title": "the section"}]}]
            add_confirmation_event(AddConfirmationEventArgs) {"section_id": "40", "file": "src/lib.rs", "confirmer_kind": "tool", "confirmer_id": "reviewer", "confirmer_version": "1", "method": "semantic_review", "verdict": "confirm", "authoring_run": "run-a", "confirming_run": "run-b", "rationale": "checked it", "timestamp": "2026-08-03T00:00:00Z"}
            ."spec_sha256" = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc" seen "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc" in store;
        set_edge_guard_threshold_reaches_the_store:
            [import_sections(ImportSectionsArgs) {"sections": [{"section_id": "sc-01", "parent_doc": "spec", "title": "scene one"}, {"section_id": "sc-02", "parent_doc": "spec", "title": "scene two"}]}]
            [import_facts(atomic::FactsManifest) {"frames": [{"frame_id": "ground-truth"}], "units": [{"unit_id": "minute"}], "entity_kinds": [{"kind_id": "place"}], "entities": [{"entity_id": "p-a", "kind": "place"}, {"entity_id": "p-b", "kind": "place"}], "predicates": [{"predicate_id": "adjacent", "object_kind": "entity", "subject_kind": "place", "object_entity_kind": "place"}, {"predicate_id": "condition", "object_kind": "token", "object_tokens": ["lit"], "subject_kind": "place"}], "facts": [{"fact_id": "f-way", "frame": "ground-truth", "claim": "a way runs", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["p-a", "p-b"], "typed": {"subject": "p-a", "predicate": "adjacent", "object": {"kind": "entity", "id": "p-b"}}}, {"fact_id": "f-lamp", "frame": "ground-truth", "claim": "the lamp is lit", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["p-a"], "typed": {"subject": "p-a", "predicate": "condition", "object": {"kind": "token", "token": "lit"}}}, {"fact_id": "f-door", "frame": "ground-truth", "claim": "the door is open", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["p-b"], "typed": {"subject": "p-b", "predicate": "condition", "object": {"kind": "token", "token": "lit"}}}], "disclosure_plans": [{"telling_id": "t-quiet", "default_mode": "state"}], "edge_guards": [{"fact_id": "f-way", "conditions": ["f-lamp", "f-door"], "threshold": 1}]}]
            set_edge_guard_threshold(SetEdgeGuardThresholdArgs) {"fact_id": "f-way"}
            ."threshold" = 1 seen "threshold" in store;
        set_disclosure_surface_scene_reaches_the_store:
            [import_sections(ImportSectionsArgs) {"sections": [{"section_id": "sc-01", "parent_doc": "spec", "title": "scene one"}, {"section_id": "sc-02", "parent_doc": "spec", "title": "scene two"}]}]
            [import_facts(atomic::FactsManifest) {"frames": [{"frame_id": "ground-truth"}], "units": [{"unit_id": "minute"}], "entity_kinds": [{"kind_id": "place"}], "entities": [{"entity_id": "p-a", "kind": "place"}, {"entity_id": "p-b", "kind": "place"}], "predicates": [{"predicate_id": "adjacent", "object_kind": "entity", "subject_kind": "place", "object_entity_kind": "place"}, {"predicate_id": "condition", "object_kind": "token", "object_tokens": ["lit"], "subject_kind": "place"}], "facts": [{"fact_id": "f-way", "frame": "ground-truth", "claim": "a way runs", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["p-a", "p-b"], "typed": {"subject": "p-a", "predicate": "adjacent", "object": {"kind": "entity", "id": "p-b"}}}, {"fact_id": "f-lamp", "frame": "ground-truth", "claim": "the lamp is lit", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["p-a"], "typed": {"subject": "p-a", "predicate": "condition", "object": {"kind": "token", "token": "lit"}}}, {"fact_id": "f-door", "frame": "ground-truth", "claim": "the door is open", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["p-b"], "typed": {"subject": "p-b", "predicate": "condition", "object": {"kind": "token", "token": "lit"}}}], "disclosure_plans": [{"telling_id": "t-quiet", "default_mode": "state"}], "edge_guards": [{"fact_id": "f-way", "conditions": ["f-lamp", "f-door"], "threshold": 1}]}]
            set_disclosure(SetDisclosureArgs) {"telling_id": "t-quiet", "fact_id": "f-lamp", "mode": "withhold"}
            ."surface_scene" = "sc-02" seen "sc-02" in store;
        set_disclosure_surface_object_reaches_the_store:
            [import_sections(ImportSectionsArgs) {"sections": [{"section_id": "sc-01", "parent_doc": "spec", "title": "scene one"}, {"section_id": "sc-02", "parent_doc": "spec", "title": "scene two"}]}]
            [import_facts(atomic::FactsManifest) {"frames": [{"frame_id": "ground-truth"}], "units": [{"unit_id": "minute"}], "entity_kinds": [{"kind_id": "place"}], "entities": [{"entity_id": "p-a", "kind": "place"}, {"entity_id": "p-b", "kind": "place"}], "predicates": [{"predicate_id": "adjacent", "object_kind": "entity", "subject_kind": "place", "object_entity_kind": "place"}, {"predicate_id": "condition", "object_kind": "token", "object_tokens": ["lit"], "subject_kind": "place"}], "facts": [{"fact_id": "f-way", "frame": "ground-truth", "claim": "a way runs", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["p-a", "p-b"], "typed": {"subject": "p-a", "predicate": "adjacent", "object": {"kind": "entity", "id": "p-b"}}}, {"fact_id": "f-lamp", "frame": "ground-truth", "claim": "the lamp is lit", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["p-a"], "typed": {"subject": "p-a", "predicate": "condition", "object": {"kind": "token", "token": "lit"}}}, {"fact_id": "f-door", "frame": "ground-truth", "claim": "the door is open", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["p-b"], "typed": {"subject": "p-b", "predicate": "condition", "object": {"kind": "token", "token": "lit"}}}], "disclosure_plans": [{"telling_id": "t-quiet", "default_mode": "state"}], "edge_guards": [{"fact_id": "f-way", "conditions": ["f-lamp", "f-door"], "threshold": 1}]}]
            set_disclosure(SetDisclosureArgs) {"telling_id": "t-quiet", "fact_id": "f-lamp", "mode": "withhold", "surface_scene": "sc-02"}
            ."surface_object" = "p-b" seen "p-b" in store;
        set_disclosure_first_at_reaches_the_store:
            [import_sections(ImportSectionsArgs) {"sections": [{"section_id": "sc-01", "parent_doc": "spec", "title": "scene one"}, {"section_id": "sc-02", "parent_doc": "spec", "title": "scene two"}]}]
            [import_facts(atomic::FactsManifest) {"frames": [{"frame_id": "ground-truth"}], "units": [{"unit_id": "minute"}], "entity_kinds": [{"kind_id": "place"}], "entities": [{"entity_id": "p-a", "kind": "place"}, {"entity_id": "p-b", "kind": "place"}], "predicates": [{"predicate_id": "adjacent", "object_kind": "entity", "subject_kind": "place", "object_entity_kind": "place"}, {"predicate_id": "condition", "object_kind": "token", "object_tokens": ["lit"], "subject_kind": "place"}], "facts": [{"fact_id": "f-way", "frame": "ground-truth", "claim": "a way runs", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["p-a", "p-b"], "typed": {"subject": "p-a", "predicate": "adjacent", "object": {"kind": "entity", "id": "p-b"}}}, {"fact_id": "f-lamp", "frame": "ground-truth", "claim": "the lamp is lit", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["p-a"], "typed": {"subject": "p-a", "predicate": "condition", "object": {"kind": "token", "token": "lit"}}}, {"fact_id": "f-door", "frame": "ground-truth", "claim": "the door is open", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["p-b"], "typed": {"subject": "p-b", "predicate": "condition", "object": {"kind": "token", "token": "lit"}}}], "disclosure_plans": [{"telling_id": "t-quiet", "default_mode": "state"}], "edge_guards": [{"fact_id": "f-way", "conditions": ["f-lamp", "f-door"], "threshold": 1}]}]
            set_disclosure(SetDisclosureArgs) {"telling_id": "t-quiet", "fact_id": "f-lamp", "mode": "withhold"}
            ."first_at" = [{"branch": "main", "coords": ["sc-02"]}] seen "sc-02" in store;
        set_disclosure_reveal_threshold_reaches_the_store:
            [import_sections(ImportSectionsArgs) {"sections": [{"section_id": "sc-01", "parent_doc": "spec", "title": "scene one"}, {"section_id": "sc-02", "parent_doc": "spec", "title": "scene two"}]}]
            [import_facts(atomic::FactsManifest) {"frames": [{"frame_id": "ground-truth"}], "units": [{"unit_id": "minute"}], "entity_kinds": [{"kind_id": "place"}], "entities": [{"entity_id": "p-a", "kind": "place"}, {"entity_id": "p-b", "kind": "place"}], "predicates": [{"predicate_id": "adjacent", "object_kind": "entity", "subject_kind": "place", "object_entity_kind": "place"}, {"predicate_id": "condition", "object_kind": "token", "object_tokens": ["lit"], "subject_kind": "place"}], "facts": [{"fact_id": "f-way", "frame": "ground-truth", "claim": "a way runs", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["p-a", "p-b"], "typed": {"subject": "p-a", "predicate": "adjacent", "object": {"kind": "entity", "id": "p-b"}}}, {"fact_id": "f-lamp", "frame": "ground-truth", "claim": "the lamp is lit", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["p-a"], "typed": {"subject": "p-a", "predicate": "condition", "object": {"kind": "token", "token": "lit"}}}, {"fact_id": "f-door", "frame": "ground-truth", "claim": "the door is open", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["p-b"], "typed": {"subject": "p-b", "predicate": "condition", "object": {"kind": "token", "token": "lit"}}}], "disclosure_plans": [{"telling_id": "t-quiet", "default_mode": "state"}], "edge_guards": [{"fact_id": "f-way", "conditions": ["f-lamp", "f-door"], "threshold": 1}]}]
            [set_disclosure(SetDisclosureArgs) {"telling_id": "t-quiet", "fact_id": "f-lamp", "mode": "withhold", "first_at": [{"branch": "main", "coords": ["sc-01", "sc-02"]}]}]
            set_disclosure_reveal_threshold(SetDisclosureRevealThresholdArgs) {"telling_id": "t-quiet", "fact_id": "f-lamp", "branch": "main"}
            ."threshold" = 2 seen "threshold" in store;
        add_branch_converges_from_reaches_the_store:
            [import_sections(ImportSectionsArgs) {"sections": [{"section_id": "40", "parent_doc": "spec", "title": "the section"}, {"section_id": "41", "parent_doc": "spec", "title": "the other"}, {"section_id": "sc-01", "parent_doc": "spec", "title": "scene one"}]}]
            [add_branch(AddBranchArgs) {"branch_id": "b-other", "forks_from": "main", "forks_at": "sc-01"}]
            [add_branch(AddBranchArgs) {"branch_id": "b-third", "forks_from": "main", "forks_at": "sc-01"}]
            add_branch(AddBranchArgs) {"branch_id": "b-what-if"}
            ."converges_from" = [{"branch": "b-other", "at": "sc-01"}, {"branch": "b-third", "at": "sc-01"}] seen "b-third" in store;
        set_section_decision_status_resolving_reaches_the_store:
            [import_sections(ImportSectionsArgs) {"sections": [{"section_id": "40", "parent_doc": "spec", "title": "the section"}, {"section_id": "41", "parent_doc": "spec", "title": "the other"}, {"section_id": "sc-01", "parent_doc": "spec", "title": "scene one"}]}]
            set_section_decision_status(SetSectionDecisionStatusArgs) {"section_id": "40", "status": "Open", "reason": "reopened"}
            ."resolving" = "41" seen "41" in store;
        add_branch_description_reaches_the_store:
            add_branch(AddBranchArgs) {"branch_id": "b-what-if"}
            ."description" = "the road not taken" seen "the road not taken" in store;
        add_branch_forks_from_reaches_the_store:
            [import_sections(ImportSectionsArgs) {"sections": [{"section_id": "sc-01", "parent_doc": "spec", "title": "scene one"}]}]
            [add_branch(AddBranchArgs) {"branch_id": "b-other", "forks_from": "main", "forks_at": "sc-01"}]
            add_branch(AddBranchArgs) {"branch_id": "b-what-if", "forks_from": "main", "forks_at": "sc-01"}
            ."forks_from" = "b-other" seen "b-other" in store;
        add_branch_forks_at_reaches_the_store:
            [import_sections(ImportSectionsArgs) {"sections": [{"section_id": "sc-01", "parent_doc": "spec", "title": "scene one"}, {"section_id": "sc-02", "parent_doc": "spec", "title": "scene two"}]}]
            add_branch(AddBranchArgs) {"branch_id": "b-what-if", "forks_from": "main", "forks_at": "sc-01"}
            ."forks_at" = "sc-02" seen "sc-02" in store;
        add_predicate_object_entity_kind_reaches_the_store:
            [add_entity_kind(AddEntityKindArgs) {"kind_id": "place"}]
            add_predicate(AddPredicateArgs) {"predicate_id": "at", "object_kind": "entity"}
            ."object_entity_kind" = "place" seen "place" in store;
        add_predicate_object_tokens_reaches_the_store:
            add_predicate(AddPredicateArgs) {"predicate_id": "does", "object_kind": "token", "object_tokens": ["waits"]}
            ."object_tokens" = ["waits", "sleeps"] seen "sleeps" in store;
        set_predicate_subject_kind_reaches_the_store:
            [add_entity_kind(AddEntityKindArgs) {"kind_id": "place"}]
            [add_predicate(AddPredicateArgs) {"predicate_id": "does", "object_kind": "token", "object_tokens": ["waits"]}]
            set_predicate(SetPredicateArgs) {"predicate_id": "does", "object_kind": "token", "object_tokens": ["waits"], "description": "what a person does"}
            ."subject_kind" = "place" seen "place" in store;
        set_predicate_object_tokens_reaches_the_store:
            [add_predicate(AddPredicateArgs) {"predicate_id": "does", "object_kind": "token", "object_tokens": ["waits"]}]
            set_predicate(SetPredicateArgs) {"predicate_id": "does", "object_kind": "token", "object_tokens": ["waits"], "description": "what a person does"}
            ."object_tokens" = ["waits", "sleeps"] seen "sleeps" in store;
        set_predicate_object_entity_kind_reaches_the_store:
            [add_entity_kind(AddEntityKindArgs) {"kind_id": "place"}]
            [add_predicate(AddPredicateArgs) {"predicate_id": "at", "object_kind": "entity"}]
            set_predicate(SetPredicateArgs) {"predicate_id": "at", "object_kind": "entity", "description": "where a thing is"}
            ."object_entity_kind" = "place" seen "place" in store;
        import_facts_frames_reaches_the_store:
            import_facts(atomic::FactsManifest) {"units": [{"unit_id": "u-base"}]}
            ."frames" = [{"frame_id": "ground-truth"}] seen "ground-truth" in store;
        import_facts_units_reaches_the_store:
            import_facts(atomic::FactsManifest) {"frames": [{"frame_id": "ground-truth"}]}
            ."units" = [{"unit_id": "day"}] seen "day" in store;
        import_facts_branches_reaches_the_store:
            import_facts(atomic::FactsManifest) {"frames": [{"frame_id": "ground-truth"}]}
            ."branches" = [{"branch_id": "b-what-if"}] seen "b-what-if" in store;
        import_facts_entity_kinds_reaches_the_store:
            import_facts(atomic::FactsManifest) {"frames": [{"frame_id": "ground-truth"}]}
            ."entity_kinds" = [{"kind_id": "place"}] seen "place" in store;
        import_facts_entities_reaches_the_store:
            import_facts(atomic::FactsManifest) {"frames": [{"frame_id": "ground-truth"}]}
            ."entities" = [{"entity_id": "e-her"}] seen "e-her" in store;
        import_facts_predicates_reaches_the_store:
            import_facts(atomic::FactsManifest) {"frames": [{"frame_id": "ground-truth"}]}
            ."predicates" = [{"predicate_id": "does", "object_kind": "token", "object_tokens": ["waits"]}] seen "does" in store;
        import_facts_disclosure_plans_reaches_the_store:
            import_facts(atomic::FactsManifest) {"frames": [{"frame_id": "ground-truth"}]}
            ."disclosure_plans" = [{"telling_id": "t-quiet", "default_mode": "state"}] seen "t-quiet" in store;
        import_facts_facts_reaches_the_store:
            [import_sections(ImportSectionsArgs) {"sections": [{"section_id": "sc-01", "parent_doc": "spec", "title": "scene one"}]}]
            import_facts(atomic::FactsManifest) {"frames": [{"frame_id": "ground-truth"}]}
            ."facts" = [{"fact_id": "f-1", "frame": "ground-truth", "claim": "she waits", "canon_from": "sc-01", "evidence": ["sc-01"]}] seen "f-1" in store;
        add_fact_quote_reaches_the_store:
            [add_frame(AddFrameArgs) {"frame_id": "ground-truth"}]
            [add_section(AddSectionArgs) {"section_id": "sc-01", "parent_doc": "spec", "title": "scene one"}]
            add_fact(atomic::FactImport) {"fact_id": "f-1", "frame": "ground-truth", "claim": "she waits", "canon_from": "sc-01", "evidence": ["sc-01"]}
            ."quote" = "she waits by the door" seen "she waits by the door" in store;
        add_fact_evidence_reaches_the_store:
            [add_frame(AddFrameArgs) {"frame_id": "ground-truth"}]
            [add_section(AddSectionArgs) {"section_id": "sc-01", "parent_doc": "spec", "title": "scene one"}]
            [add_section(AddSectionArgs) {"section_id": "sc-02", "parent_doc": "spec", "title": "scene two"}]
            add_fact(atomic::FactImport) {"fact_id": "f-1", "frame": "ground-truth", "claim": "she waits", "canon_from": "sc-01", "evidence": ["sc-01"]}
            ."evidence" = ["sc-01", "sc-02"] seen "sc-02" in store;
        add_fact_entities_reaches_the_store:
            [add_frame(AddFrameArgs) {"frame_id": "ground-truth"}]
            [add_section(AddSectionArgs) {"section_id": "sc-01", "parent_doc": "spec", "title": "scene one"}]
            [add_entity(AddEntityArgs) {"entity_id": "e-her"}]
            add_fact(atomic::FactImport) {"fact_id": "f-1", "frame": "ground-truth", "claim": "she waits", "canon_from": "sc-01", "evidence": ["sc-01"]}
            ."entities" = ["e-her"] seen "e-her" in store;
        add_fact_canon_to_reaches_the_store:
            [add_frame(AddFrameArgs) {"frame_id": "ground-truth"}]
            [add_section(AddSectionArgs) {"section_id": "sc-01", "parent_doc": "spec", "title": "scene one"}]
            [add_section(AddSectionArgs) {"section_id": "sc-02", "parent_doc": "spec", "title": "scene two"}]
            add_fact(atomic::FactImport) {"fact_id": "f-1", "frame": "ground-truth", "claim": "she waits", "canon_from": "sc-01", "evidence": ["sc-01"]}
            ."canon_to" = "sc-02" seen "sc-02" in store;
        amend_fact_quote_reaches_the_store:
            [add_frame(AddFrameArgs) {"frame_id": "ground-truth"}]
            [add_section(AddSectionArgs) {"section_id": "sc-01", "parent_doc": "spec", "title": "scene one"}]
            [add_fact(atomic::FactImport) {"fact_id": "f-1", "frame": "ground-truth", "claim": "she waits", "canon_from": "sc-01", "evidence": ["sc-01"]}]
            amend_fact(AmendFactArgs) {"fact_id": "f-1", "frame": "ground-truth", "claim": "she waits still", "canon_from": "sc-01", "evidence": ["sc-01"], "reason": "revised"}
            ."quote" = "she waits by the door" seen "she waits by the door" in store;
        amend_fact_entities_reaches_the_store:
            [add_frame(AddFrameArgs) {"frame_id": "ground-truth"}]
            [add_section(AddSectionArgs) {"section_id": "sc-01", "parent_doc": "spec", "title": "scene one"}]
            [add_entity(AddEntityArgs) {"entity_id": "e-her"}]
            [add_fact(atomic::FactImport) {"fact_id": "f-1", "frame": "ground-truth", "claim": "she waits", "canon_from": "sc-01", "evidence": ["sc-01"]}]
            amend_fact(AmendFactArgs) {"fact_id": "f-1", "frame": "ground-truth", "claim": "she waits still", "canon_from": "sc-01", "evidence": ["sc-01"], "reason": "revised"}
            ."entities" = ["e-her"] seen "e-her" in store;
        add_entity_description_reaches_the_store:
            add_entity(AddEntityArgs) {"entity_id": "e-her"}
            ."description" = "the one who waits" seen "the one who waits" in store;
        add_entity_kind_description_reaches_the_store:
            add_entity_kind(AddEntityKindArgs) {"kind_id": "place"}
            ."description" = "anywhere a person can be" seen "anywhere a person can be" in store;
        add_frame_description_reaches_the_store:
            add_frame(AddFrameArgs) {"frame_id": "ground-truth"}
            ."description" = "what is so in the town" seen "what is so in the town" in store;
        add_parameter_description_reaches_the_store:
            add_parameter(AddParameterArgs) {"parameter_id": "affection"}
            ."description" = "how warmly she reads him" seen "how warmly she reads him" in store;
        add_unit_description_reaches_the_store:
            add_unit(AddUnitArgs) {"unit_id": "day"}
            ."description" = "a day of the flood" seen "a day of the flood" in store;
        add_entity_kind_reaches_the_store:
            [add_entity_kind(AddEntityKindArgs) {"kind_id": "place"}]
            add_entity(AddEntityArgs) {"entity_id": "p-shrine"}
            ."kind" = "place" seen "place" in store;
        add_entity_kind_parents_reaches_the_store:
            [add_entity_kind(AddEntityKindArgs) {"kind_id": "place"}]
            add_entity_kind(AddEntityKindArgs) {"kind_id": "quarter"}
            ."parents" = ["place"] seen "place" in store;
        add_section_parent_section_reaches_the_store:
            [add_section(AddSectionArgs) {"section_id": "40", "parent_doc": "spec", "title": "the parent"}]
            add_section(AddSectionArgs) {"section_id": "41", "parent_doc": "spec", "title": "the child"}
            ."parent_section" = "40" seen "40" in store;
        add_predicate_description_reaches_the_store:
            add_predicate(AddPredicateArgs) {"predicate_id": "does", "object_kind": "token", "object_tokens": ["waits"]}
            ."description" = "what a person is doing" seen "what a person is doing" in store;
        add_predicate_subject_kind_reaches_the_store:
            [add_entity_kind(AddEntityKindArgs) {"kind_id": "character"}]
            add_predicate(AddPredicateArgs) {"predicate_id": "does", "object_kind": "token", "object_tokens": ["waits"]}
            ."subject_kind" = "character" seen "character" in store;
        add_disclosure_plan_description_reaches_the_store:
            add_disclosure_plan(AddDisclosurePlanArgs) {"telling_id": "default", "default_mode": "state"}
            ."description" = "what the reader is told" seen "what the reader is told" in store;
        add_inventory_entry_reason_reaches_the_store:
            add_inventory_entry(AddInventoryEntryArgs) {"inventory_id": "inv-1", "status": "active"}
            ."reason" = "why it is open" seen "why it is open" in store;
        add_inventory_entry_source_reaches_the_store:
            add_inventory_entry(AddInventoryEntryArgs) {"inventory_id": "inv-1", "status": "active"}
            ."source" = "the review that found it" seen "the review that found it" in store;
        set_inventory_status_reason_reaches_the_store:
            [add_inventory_entry(AddInventoryEntryArgs) {"inventory_id": "inv-1", "status": "active"}]
            set_inventory_status(SetInventoryStatusArgs) {"inventory_id": "inv-1", "status": "deprecated"}
            ."reason" = "closed by the round that found it" seen "closed by the round that found it" in store;
        set_section_parent_section_reaches_the_store:
            [add_section(AddSectionArgs) {"section_id": "40", "parent_doc": "spec", "title": "the parent"}]
            [add_section(AddSectionArgs) {"section_id": "41", "parent_doc": "spec", "title": "the child"}]
            set_section_parent_section(SetSectionParentSectionArgs) {"section_id": "41"}
            ."parent_section" = "40" seen "40" in store;
        add_section_binding_symbol_reaches_the_store:
            [add_section(AddSectionArgs) {"section_id": "40", "parent_doc": "spec", "title": "the parent"}]
            add_section_binding(AddSectionBindingArgs) {"section_id": "40", "file": "src/lib.rs", "kind": "implements"}
            ."symbol" = "the_bound_symbol" seen "the_bound_symbol" in store;
        append_changelog_entry_impact_refs_reaches_the_store:
            append_changelog_entry(AppendChangelogEntryArgs) {"entry_id": "Round 1", "decision_summary": "a decision", "changes_bullets": ["a change"], "verification_bullets": ["a check"]}
            ."impact_refs" = ["the-impacted-ref"] seen "the-impacted-ref" in store;
        append_changelog_entry_carry_forward_reaches_the_store:
            append_changelog_entry(AppendChangelogEntryArgs) {"entry_id": "Round 1", "decision_summary": "a decision", "changes_bullets": ["a change"], "verification_bullets": ["a check"]}
            ."carry_forward_bullets" = ["the carried item"] seen "the carried item" in store;
        set_entity_kind_parents_reaches_the_store:
            [add_entity_kind(AddEntityKindArgs) {"kind_id": "place"}]
            [add_entity_kind(AddEntityKindArgs) {"kind_id": "quarter"}]
            set_entity_kind_parents(SetEntityKindParentsArgs) {"kind_id": "quarter"}
            ."parents" = ["place"] seen "place" in store;
    }

    /// An agent can only call what the schema shows it (Round 981) — the Round
    /// 690 rule applied to the flag that decides whether a census is recorded.
    #[test]
    fn append_changelog_entry_schema_exposes_record_census() {
        let schema = schemars::schema_for!(AppendChangelogEntryArgs);
        let json = serde_json::to_value(&schema).expect("schema serializes");
        let props = json
            .get("properties")
            .and_then(|p| p.as_object())
            .expect("the args schema exposes object properties");
        assert!(
            props.contains_key("record_census"),
            "the agent-facing schema hides `record_census`, so no agent can ask \
             for a census: {:?}",
            props.keys().collect::<Vec<_>>()
        );
    }

    /// Round 690 (DEBT-MCP-MANIFEST-SCHEMA) — PROVE the manifest tool arg is a
    /// TYPED schema, not the R687 opaque `{manifest_json: string}`. Generated
    /// from the ONE atomic type via the feature-gated JsonSchema derive, so the
    /// agent sees every manifest field. This is the layer-correct check the
    /// verification-frame lesson demands (prove the claim, don't assert it).
    #[test]
    fn import_facts_arg_schema_is_typed_not_opaque() {
        let schema = schemars::schema_for!(atomic::FactsManifest);
        let json = serde_json::to_value(&schema).expect("schema serializes");
        let props = json
            .get("properties")
            .and_then(|p| p.as_object())
            .expect("FactsManifest schema exposes object properties, not a bare string");
        for field in [
            "frames",
            "branches",
            "entity_kinds",
            "entities",
            "predicates",
            "facts",
            "disclosure_plans",
        ] {
            assert!(
                props.contains_key(field),
                "manifest schema is missing the `{field}` property"
            );
        }
        // The R687 opaque single-string arg is gone.
        assert!(
            !props.contains_key("manifest_json"),
            "the opaque manifest_json string arg must no longer exist"
        );
    }

    /// Round 692 — add_fact/amend_fact take atomic::FactImport directly (the
    /// AddFactArgs/TypedClaimArgs mirror is gone), so the typed leg is the tagged
    /// TypedObject enum, not the flattened object_entity/object_value pair. PROVE
    /// it at the schema layer.
    #[test]
    fn fact_import_schema_uses_the_typed_object_enum_not_the_flattened_pair() {
        let schema = schemars::schema_for!(atomic::FactImport);
        let json = serde_json::to_string(&schema).expect("schema serializes");
        assert!(
            !json.contains("object_entity") && !json.contains("object_value"),
            "the flattened typed-object pair must be gone from the schema"
        );
        // The TypedObject enum's discriminant tag is present instead.
        assert!(
            json.contains("\"kind\""),
            "the TypedObject `kind` tag must appear in the schema"
        );
    }

    /// Round 691 (DEBT-MCP-INVOKE-SMOKE) — the coverage the "MCP is the store's
    /// real authoring surface" thesis requires and the router test cannot give:
    /// DRIVE the mutate tools through a real MnemosyneServer and assert the store
    /// actually changed, plus the all-or-nothing property AT the wrapper layer
    /// (a divergent import rejects and leaves the store byte-unchanged). This
    /// exercises arg extraction, run_mutate's lock, the atomic primitive, and
    /// finish_mutate's read-model resync — none of which router presence sees.
    #[tokio::test]
    async fn mcp_import_tools_author_the_store_and_reject_divergent() {
        use std::fs;
        let tmp = tempfile::TempDir::new().unwrap();
        let ws = tmp.path();
        fs::create_dir_all(ws.join("docs/.atomic")).unwrap();
        fs::write(ws.join("mnemosyne.toml"), "[workspace]\n").unwrap();
        let sidecar = ws.join("docs/.atomic/workspace.atomic.json");
        atomic::AtomicStore::new().save(&sidecar).unwrap();
        let read_store = || atomic::AtomicStore::load(&sidecar).unwrap();

        let server = MnemosyneServer::new(ws.to_path_buf()).expect("server construct");

        let section = |title: &str| atomic::SectionImport {
            section_id: "sec-a".to_string(),
            parent_doc: "docs/DESIGN.md".to_string(),
            title: title.to_string(),
            parent_section: None,
            normative_excerpt: None,
            coverage_expectation: Default::default(),
        };

        // 1) import_sections creates the section a fact will evidence against.
        let r = server
            .import_sections(Parameters(ImportSectionsArgs {
                sections: vec![section("A")],
            }))
            .await;
        assert!(r.is_error != Some(true), "import_sections failed: {r:?}");
        assert!(
            read_store().sections.contains_key(&"sec-a".into()),
            "sec-a not created"
        );

        // 2) import_facts creates a frame + a fact referencing sec-a, atomically.
        let manifest = atomic::FactsManifest {
            edge_costs: Vec::new(),
            edge_guards: Vec::new(),
            frames: vec![atomic::FrameImport {
                frame_id: "gt".to_string(),
                description: String::new(),
            }],
            branches: vec![],
            entity_kinds: vec![],
            units: vec![],
            entities: vec![],
            predicates: vec![],
            facts: vec![atomic::FactImport {
                fact_id: "f1".to_string(),
                frame: "gt".to_string(),
                branch: None,
                entities: vec![],
                claim: "the count is an eccentric nobleman".to_string(),
                canon_from: "sec-a".to_string(),
                canon_to: None,
                evidence: vec!["sec-a".to_string()],
                conflicts_with: vec![],
                supersedes_in_frame: None,
                payoff_expectation: None,
                pays_off: vec![],
                typed: None,
                quote: None,
            }],
            disclosure_plans: vec![],
        };
        let r = server.import_facts(Parameters(manifest)).await;
        assert!(r.is_error != Some(true), "import_facts failed: {r:?}");
        assert!(
            read_store().narrative_facts.contains_key(&"f1".into()),
            "f1 not created"
        );

        // 3) A divergent section (same id, different title) rejects the WHOLE
        //    import and writes NOTHING — all-or-nothing at the wrapper layer.
        let before = fs::read(&sidecar).unwrap();
        let r = server
            .import_sections(Parameters(ImportSectionsArgs {
                sections: vec![section("DIFFERENT")],
            }))
            .await;
        assert!(
            r.is_error == Some(true),
            "divergent import must be rejected"
        );
        assert_eq!(
            before,
            fs::read(&sidecar).unwrap(),
            "a rejected import must leave the store byte-unchanged"
        );
    }
}
