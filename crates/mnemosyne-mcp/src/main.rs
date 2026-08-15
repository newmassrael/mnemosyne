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
    pub section_id: ExistingRef,
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
    pub entry_id: ExistingRef,
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

/// THREE STRUCTS BECAUSE THERE ARE THREE VALUE SPACES.
///
/// These were one `SetSectionTextArgs` whose `text` field carried the doc "New
/// value. For intent: a single sentence, max ~200 chars" — a per-user caveat
/// inside a shared shape, which is the same defect the round before this one
/// removed from the bullet setters, still standing three declarations away. A
/// struct can only say one thing, so it said the intent's rule to the title and
/// the parent doc as well, and named none of them.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetSectionTitleArgs {
    /// Section ID to mutate. Pass `"39"`, not `"§39"`.
    pub section_id: ExistingRef,
    /// Heading text. Free prose, no length rule of its own.
    pub title: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetSectionParentDocArgs {
    /// Section ID to mutate. Pass `"39"`, not `"§39"`.
    pub section_id: ExistingRef,
    /// The owning doc this section re-binds to — an identifier, not prose.
    pub parent_doc: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetSectionIntentArgs {
    /// Section ID to mutate. Pass `"39"`, not `"§39"`.
    pub section_id: ExistingRef,
    /// One sentence stating what the section is for. The store enforces a
    /// length cap and refuses a longer value; this is the only one of the three
    /// section-text arguments with a rule of its own.
    pub intent: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetSectionBulletsArgs {
    pub section_id: ExistingRef,
    /// Ordered list of bullets. Each ≤ 100 chars per T3 default.
    pub bullets: Vec<String>,
}

/// `set_section_alternatives` does NOT share [`SetSectionBulletsArgs`], and the
/// reason is the thing the shared struct was hiding: every other bullet setter
/// takes free prose, while this one took a line with a GRAMMAR in it and split
/// it in the handler. One struct for both meant the schema described the free
/// case and the constrained one identically.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetSectionAlternativesArgs {
    pub section_id: ExistingRef,
    /// Ordered list of rejected alternatives. Replaces the existing list; an
    /// empty list clears it.
    pub alternatives: Vec<RejectedAlternative>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddSectionCaveatArgs {
    pub section_id: ExistingRef,
    /// Single caveat bullet to append.
    pub bullet: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetImpactScopeArgs {
    pub section_id: ExistingRef,
    /// Cross-ref targets without the `§` prefix, e.g. `["39", "61.1"]`.
    pub refs: Vec<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddSectionExampleArgs {
    pub section_id: ExistingRef,
    /// Code-fence language tag (e.g. `"rust"`, `"toml"`).
    pub language: String,
    /// Code body — embedded inside a fenced block. No leading fence.
    pub code: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddSectionBindingArgs {
    /// Section ID without the `§` prefix.
    pub section_id: ExistingRef,
    /// Workspace-relative POSIX file path. No leading `/`, no leading
    /// `./`, no `..` segment, no backslash. The file does not need to
    /// exist at write time — schema records intent.
    pub file: String,
    /// Optional opaque language-agnostic identifier (function / type /
    /// qualified path). Stored as-is; no language-grammar regex applied.
    /// Omit for file-level binding.
    #[serde(default)]
    pub symbol: Option<String>,
    /// Trace-link kind. `implements` = SysML «satisfy»: the symbol fulfills
    /// the section's normative requirement, and it is the only kind counted as
    /// coverage. `references` = SysML «trace»: related to, no fulfillment
    /// claim. `verifies` = SysML «verify»: a test or report artifact
    /// establishes the requirement.
    pub kind: atomic::BindingKind,
}

// Round 287/289 — Section creation + outline setter MCP arg structs.

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddSectionArgs {
    /// Section ID to create. No `§` prefix in the value; use the bare slug
    /// or numbered id (e.g. `"39"`, `"39.1"`, `"my-section"`).
    pub section_id: FreshId,
    /// Owning doc identifier (workspace-relative path or doc id).
    pub parent_doc: String,
    /// Heading title (non-empty).
    pub title: String,
    /// Optional parent section id. Omit for top-level; pass a bare id
    /// (no `§`) to nest under an existing section. The parent must exist
    /// in the atomic store at write time.
    #[serde(default)]
    pub parent_section: Option<ExistingRef>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetSectionParentSectionArgs {
    /// Section being re-parented.
    pub section_id: ExistingRef,
    /// New parent. Pass `Some("<id>")` to nest under that section, or
    /// `None` (omit) to promote to top-level. Self-loop rejected.
    #[serde(default)]
    pub parent_section: Option<ExistingRef>,
}

/// R678 — the section-mutate parity gap the cost-audit found: an MCP agent
/// could add/edit a section but not REMOVE one, nor transition its lifecycle.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RemoveSectionArgs {
    /// Section ID (the `§` prefix is stripped if present).
    pub section_id: ExistingRef,
    /// Mandatory rationale — recorded in the reasoned-mutation ledger
    /// (`report_mutation_reasons`), not discarded.
    pub reason: String,
}

/// R678 — the section lifecycle transition (Active/Superseded/Removed/Open).
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetSectionDecisionStatusArgs {
    /// Section ID (the `§` prefix is stripped if present).
    pub section_id: ExistingRef,
    /// The decision-lifecycle state the section moves to. `superseded`
    /// requires `superseding`; `open` admits `resolving`.
    pub status: mnemosyne_core::DecisionStatus,
    /// Superseding section id — MANDATORY for `superseded`, rejected otherwise.
    #[serde(default)]
    pub superseding: Option<ScannedRef>,
    /// Resolving section id — valid only for `open`, rejected otherwise.
    #[serde(default)]
    pub resolving: Option<ScannedRef>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RemoveSectionBindingArgs {
    /// Section ID without the `§` prefix.
    pub section_id: ExistingRef,
    /// Workspace-relative POSIX file path to remove from the binding set.
    pub file: ExistingRef,
    /// Optional symbol — must exact-match the row to remove. Omit to
    /// target a file-only binding (a row with `symbol = None`). Matching is
    /// kind-agnostic (identity is the `(file, symbol)` pair).
    #[serde(default)]
    pub symbol: Option<String>,
    /// Mandatory rationale — recorded in the reasoned-mutation ledger
    /// (`report_mutation_reasons`), not discarded.
    pub reason: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetSectionBindingKindArgs {
    /// Section ID without the `§` prefix.
    pub section_id: ExistingRef,
    /// Workspace-relative POSIX file path of the existing binding.
    pub file: ExistingRef,
    /// Optional symbol identifying the binding (omit for a file-only row).
    #[serde(default)]
    pub symbol: Option<String>,
    /// The kind the binding is reclassified to.
    pub kind: atomic::BindingKind,
    /// Mandatory rationale — recorded in the reasoned-mutation ledger
    /// (`report_mutation_reasons`), not discarded.
    pub reason: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetSectionCoverageExpectationArgs {
    /// Section ID without the `§` prefix.
    pub section_id: ExistingRef,
    /// Whether the section's coverage axiom applies. `normative` expects an
    /// `implements` binding; `out_of_scope_here` (in the document this ledger
    /// mirrors, not built here — including a deferred or Phase-2 feature) and
    /// `informational` (inherently non-implementable prose) both exempt it.
    pub expectation: atomic::CoverageExpectation,
    /// Mandatory rationale — recorded in the reasoned-mutation ledger
    /// (`report_mutation_reasons`), not discarded.
    pub reason: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetSectionVerificationExpectationArgs {
    /// Section ID without the `§` prefix.
    pub section_id: ExistingRef,
    /// `dedicated` expects a `verifies` binding to a test or report artifact;
    /// `by_construction` has no independently-assertable per-unit oracle and is
    /// exempt from the dedicated-verify gate.
    pub expectation: atomic::VerificationExpectation,
    /// Mandatory rationale — recorded in the reasoned-mutation ledger
    /// (`report_mutation_reasons`), not discarded.
    pub reason: String,
}

/// Round 1024 — the reasoned-mutation ledger read.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReportMutationReasonsArgs {
    /// Narrow to one record's rows — a section, fact, binding or entry id. A
    /// target that no longer exists is the interesting case, not an error:
    /// removals write rows too. Omit for the whole ledger.
    #[serde(default)]
    pub target: Option<String>,
}

/// R417 — confirmation-event MCP args. A `file` present makes it a
/// VerifiesBinding claim, else a SectionCompleteness claim. Enum fields take the
/// snake_case tag. The event_id is derived in-core (not supplied).
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddConfirmationEventArgs {
    /// Claim section ID without the `§` prefix.
    pub section_id: ExistingRef,
    /// Bound file (VerifiesBinding claim). Omit for a SectionCompleteness claim.
    pub file: Option<String>,
    /// Bound symbol (requires `file`).
    pub symbol: Option<String>,
    /// What kind of producer emitted this: `tool` is deterministic and
    /// reproducible, `model` is a fresh-context LLM trusted only as an
    /// independent-set member.
    pub confirmer_kind: atomic::ConfirmerKind,
    pub confirmer_id: FreshId,
    pub confirmer_version: String,
    /// How it was verified — a deterministic linkage check, a fresh-context
    /// semantic review, or an external coverage attestation.
    pub method: atomic::ConfirmMethod,
    /// The verdict this event records. A single refutation blocks regardless
    /// of how many confirmations stand beside it.
    pub verdict: atomic::Verdict,
    /// The run that authored the claim.
    pub authoring_run: String,
    /// The run producing THIS verdict (must differ from `authoring_run`).
    pub confirming_run: String,
    pub rationale: String,
    /// When the confirmer says it checked, as a canonical UTC instant —
    /// EXACTLY `YYYY-MM-DDTHH:MM:SSZ`, no offset and no fractional seconds, so
    /// the append-only ledger sorts chronologically by sorting lexically. Never
    /// generated in-core (determinism), and NOT part of the derived event id: one
    /// verification act records once however many times its clock moves.
    pub timestamp: UtcInstant,
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
    pub frame_id: FreshId,
    /// Optional free-form description (whose epistemic frame this is).
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddBranchArgs {
    /// Branch id — the registry key every non-default fact `branch` must
    /// reference. `main` is known by construction and never registered.
    pub branch_id: FreshId,
    /// Optional free-form description (which quest-path/playthrough world).
    #[serde(default)]
    pub description: String,
    /// Parent world-line this branch diverges from (R438). Give with
    /// `forks_at`; omit both for a standalone world.
    #[serde(default)]
    pub forks_from: Option<ExistingRef>,
    /// Canon point of divergence (structure-section id).
    #[serde(default)]
    pub forks_at: Option<ExistingRef>,
    /// Incoming world-line merges (R532 — convergence / confluence). Each entry
    /// is a parent + its merge coordinate; a confluence has ≥ 2. Mutually
    /// exclusive with `forks_from`/`forks_at`.
    #[serde(default)]
    pub converges_from: Vec<ConvergeEdgeArg>,
}

/// One incoming-merge edge of a confluence branch (R532): the parent
/// world-line + the parent's merge coordinate (structure-section id).
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[schemars(inline)]
pub struct ConvergeEdgeArg {
    pub branch: String,
    pub at: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddEntityArgs {
    /// Entity id — the registry key fact `entities` refs must name.
    pub entity_id: FreshId,
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
    pub kind_id: FreshId,
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
    pub kind_id: ExistingRef,
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
    pub kind_id: ExistingRef,
}

/// Register one unit of measure (Round 706) — the vocabulary a `quantity`
/// object's `unit` refs. Without it an MCP-only agent could not declare a unit,
/// so no Quantity fact could pass the units-registry gate. Mirrors
/// `add_entity_kind`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddUnitArgs {
    /// Unit id — one member of the measurement vocabulary (e.g. day / minute /
    /// metre). Fail-loud: a Quantity whose unit is unregistered rejects.
    pub unit_id: FreshId,
    #[serde(default)]
    pub description: String,
}

/// Register a numeric PARAMETER (Round 729, DEBT-K) — an accumulating meter
/// (affection / karma / gold). Mirrors `add_unit`. Without it an MCP-only agent
/// could not declare a meter, so no parameter_delta could pass the gate.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddParameterArgs {
    /// Parameter id — one member of the meter vocabulary (e.g. affection).
    pub parameter_id: FreshId,
    #[serde(default)]
    pub description: String,
}

/// Attach a SIGNED per-beat delta to a parameter (Round 729, DEBT-K) — a
/// side-table entry keyed by the beat fact id. The consumer accumulates the
/// running sum; Mnemosyne never does.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddParameterDeltaArgs {
    /// The BEAT FACT ID the delta rides (must already exist).
    pub fact_id: ExistingRef,
    /// A REGISTERED parameter (add_parameter first).
    pub parameter: ExistingRef,
    /// The SIGNED delta — non-zero (0 = a no-op beat); both signs legal.
    pub delta: i64,
}

/// Remove one (fact, parameter) delta (Round 729) — the peer of
/// `add_parameter_delta`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RemoveParameterDeltaArgs {
    /// The beat fact id whose delta to drop.
    pub fact_id: ExistingRef,
    /// The parameter whose delta to drop (fail-loud if the beat has none).
    pub parameter: ExistingRef,
}

/// Attach a numeric-value THRESHOLD gate to a CHOICE edge (Round 730, DEBT-K) — a
/// side-table entry keyed by the choice fact id. The gate references the meter
/// DIRECTLY (no boolean proxy); the consumer accumulates the meter and compares.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddParameterGateArgs {
    /// The CHOICE FACT ID the gate rides (must already exist; rides ANY fact — no
    /// map-edge check).
    pub fact_id: ExistingRef,
    /// A REGISTERED parameter (add_parameter first).
    pub parameter: ExistingRef,
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
    pub fact_id: ExistingRef,
}

/// Attach a cost to one map edge (Round 709 → DEBT-J) — a side-table entry keyed
/// by the adjacent fact id, NOT a reified fact (the cost is frame-invariant edge
/// metadata). Mirrors `add_unit`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddEdgeCostArgs {
    /// The ADJACENT FACT ID the cost attaches to (must already exist).
    pub fact_id: ExistingRef,
    /// The cost amount — a POSITIVE integer (e.g. walk minutes; 0/negative = a
    /// free teleport, rejected by G3).
    pub n: i64,
    /// A REGISTERED unit (e.g. minute; add_unit first).
    pub unit: ExistingRef,
}

/// Remove a map edge's cost (Round 711) — the peer of `add_edge_cost`. Drops a
/// stray cost off a non-edge fact without retracting the fact.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RemoveEdgeCostArgs {
    /// The fact id whose edge cost to drop (fail-loud if it has none).
    pub fact_id: ExistingRef,
}

/// Attach a multiset COUNT to a fact (Round 731 → DEBT-L) — a side-table entry
/// keyed by the fact id, value = a positive count. Mirrors `add_edge_cost`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddFactCountArgs {
    /// The FACT ID the count attaches to (must already exist).
    pub fact_id: ExistingRef,
    /// The multiset count — a POSITIVE integer (holds(A,potion) count 5 = A holds
    /// FIVE potions; 0/negative = not holding it, rejected — retract the fact).
    pub count: i64,
}

/// Remove a fact's multiset count (Round 731) — the peer of `add_fact_count`.
/// Drops a stray count off a fact without retracting the fact.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RemoveFactCountArgs {
    /// The fact id whose multiset count to drop (fail-loud if it has none).
    pub fact_id: ExistingRef,
}

/// Attach a place-access guard to one map edge (Round 717 design → Round 720).
/// Keyed by the edge fact id, value = the condition fact id; both must exist.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddEdgeGuardArgs {
    /// The ADJACENT (edge) FACT ID the guard attaches to (must already exist).
    pub fact_id: ExistingRef,
    /// The CONDITION FACT ID the edge requires (must already exist; a dangling-
    /// ref is rejected). Distinct from the edge — an edge cannot guard itself.
    pub condition: ExistingRef,
}

/// Remove a map edge's whole guard set (Round 720) — the peer of `add_edge_guard`.
/// Drops a stray guard off a non-edge fact without retracting the fact.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RemoveEdgeGuardArgs {
    /// The fact id whose edge guard set to drop (fail-loud if it has none).
    pub fact_id: ExistingRef,
}

/// Remove ONE condition from a map edge's guard set (Round 722).
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RemoveEdgeGuardConditionArgs {
    /// The edge fact id whose guard set to edit.
    pub fact_id: ExistingRef,
    /// The condition to drop from the set (fail-loud if absent; the edge key is
    /// deleted when the set empties).
    pub condition: ExistingRef,
}

/// Set (or clear) a map edge guard's K-of-N threshold (Round 723).
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetEdgeGuardThresholdArgs {
    /// The edge fact id whose guard threshold to set (must already have a guard).
    pub fact_id: ExistingRef,
    /// The K-of-N threshold: `Some(k)` = at least k of the conditions (1 <= k <=
    /// len; k == len normalizes to AND); `None` (omit) = clear to AND (require all).
    #[serde(default)]
    pub threshold: Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReportEntityArgs {
    /// Entity id to assemble the dossier for.
    pub entity_id: ExistingRef,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddPredicateArgs {
    /// Predicate id — the registry key every TypedClaim predicate must
    /// name. Load-bearing (narrative rules key off it), hence fail-loud.
    pub predicate_id: FreshId,
    /// Declared object shape. Round 873 typed this slot: the closed set is now
    /// in the tool's JSON Schema, so a client cannot spell a tag the wire
    /// removed and learn about it only from a write-time reject.
    pub object_kind: mnemosyne_core::PredicateObjectKind,
    /// R701 — required entity-KIND for the subject leg (a registered
    /// `entity_kinds` ref; omit = any). The write path rejects a fact whose
    /// subject entity is not this kind (the spatial-map G1 gate).
    #[serde(default)]
    pub subject_kind: Option<ExistingRef>,
    /// R701 — required entity-KIND for an entity-shaped object leg (omit =
    /// any). Rejects unless `object_kind=entity` (only an entity object has a kind).
    #[serde(default)]
    pub object_entity_kind: Option<ExistingRef>,
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
    pub predicate_id: ExistingRef,
    /// New declared object shape. A re-type rejects while any existing use still
    /// holds an object of the old shape. Round 873 typed this slot, so the closed
    /// set travels in the tool's JSON Schema.
    pub object_kind: mnemosyne_core::PredicateObjectKind,
    /// R701 — new required subject entity-KIND (omit = clear; full replace).
    /// A tighten rejects while any existing use's subject is off-kind.
    #[serde(default)]
    pub subject_kind: Option<ExistingRef>,
    /// R701 — new required object entity-KIND (omit = clear; full replace).
    /// A tighten rejects while any existing use's object is off-kind.
    #[serde(default)]
    pub object_entity_kind: Option<ExistingRef>,
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
    pub predicate_id: ExistingRef,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddDisclosurePlanArgs {
    /// Telling id — the registry key for this named telling over the fact base.
    pub telling_id: FreshId,
    /// The mode every fact takes in this telling unless `set_disclosure`
    /// overrides it.
    pub default_mode: mnemosyne_core::DisclosureMode,
    #[serde(default)]
    pub description: String,
}

/// One per-world-line first-reveal trigger (Round 752) — a branch plus its
/// trigger-coordinate SET and an optional K-of-N threshold.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[schemars(inline)]
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
    pub telling_id: ExistingRef,
    /// Fact id the override targets (must exist; withhold/first_at need it typed).
    pub fact_id: ExistingRef,
    /// How this telling exposes this fact. `withhold` = never told (the reader
    /// reconstructs it); `state` = told outright; `hint` = partially
    /// signalled; `imply` = realised through an object or the environment.
    pub mode: mnemosyne_core::DisclosureMode,
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
    pub telling_id: ExistingRef,
    /// Fact id whose decision is cleared (the fact itself is untouched).
    pub fact_id: ExistingRef,
    /// Why the decision is withdrawn (mandatory — audit-trail safeguard).
    pub reason: String,
}

/// Round 752 — add ONE trigger coordinate to a fact's per-world first-reveal SET
/// (the granular peer of `set_disclosure`, mirroring `add_edge_guard`).
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddDisclosureRevealCoordArgs {
    /// Telling id carrying the override (`set_disclosure` first).
    pub telling_id: ExistingRef,
    /// Fact id whose reveal the coord attaches to (a first_at pin makes it
    /// gate-targeted, so the fact must carry a typed claim).
    pub fact_id: ExistingRef,
    /// World-line the reveal pins timing for (main or a registered branch).
    pub branch: ExistingRef,
    /// The trigger coordinate to ADD to the branch's SET (a registered section).
    pub coord: ExistingRef,
}

/// Round 752 — remove ONE trigger coordinate from a fact's per-world first-reveal
/// SET (the granular peer of `add_disclosure_reveal_coord`).
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RemoveDisclosureRevealCoordArgs {
    /// Telling id carrying the override.
    pub telling_id: ExistingRef,
    /// Fact id whose reveal the coord is dropped from.
    pub fact_id: ExistingRef,
    /// World-line the reveal is pinned on.
    pub branch: ExistingRef,
    /// The trigger coordinate to DROP from the set (fail-loud if absent; the
    /// branch key is deleted when the set empties — never a vacuous empty trigger).
    pub coord: ExistingRef,
}

/// Round 752 — set (or clear) a fact's per-world first-reveal K-of-N THRESHOLD
/// (the granular peer of `set_edge_guard_threshold`).
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetDisclosureRevealThresholdArgs {
    /// Telling id carrying the override.
    pub telling_id: ExistingRef,
    /// Fact id whose reveal threshold to set.
    pub fact_id: ExistingRef,
    /// World-line whose reveal threshold to set (must already have a reveal).
    pub branch: ExistingRef,
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
    pub fact_id: ExistingRef,
    /// Mandatory rationale (audit safeguard).
    pub reason: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddFactConflictArgs {
    pub fact_id: ExistingRef,
    pub conflicts_with: ExistingRef,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ValidateContinuityArgs {
    /// Canon-order declaration path override (workspace-relative; bypasses
    /// the configured sha256 pin — the R428 rule). Omit to use
    /// `[continuity].canon_order_path`.
    #[serde(default)]
    pub order_path: Option<AgentPath>,
    /// `narrative-rules/v1` declaration path override (Round 449;
    /// workspace-relative, bypasses the configured sha256 pin). Omit to
    /// use `[continuity].rules_path`.
    #[serde(default)]
    pub rules_path: Option<AgentPath>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ProposeVerdictArgs {
    /// Path to the candidate `import-facts` manifest (a JSON object with
    /// `frames`/`branches`/`entities`/`predicates`/`facts`/`disclosure_plans`
    /// arrays). The agent writes the candidate batch to this file, then calls
    /// the tool; the file is only READ (dry run).
    pub manifest_path: AgentPath,
    /// Canon-order declaration path override (bypasses the pin). Omit to use
    /// `[continuity].canon_order_path`.
    #[serde(default)]
    pub order_path: Option<AgentPath>,
    /// `narrative-rules/v1` declaration path override. Omit to use
    /// `[continuity].rules_path`.
    #[serde(default)]
    pub rules_path: Option<AgentPath>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReportAuthoringFrontierArgs {
    /// Telling id — enables the quest + disclosure gap sections (unresolved
    /// quests, never-planned disclosures). Omit for the telling-independent
    /// gaps only (zero-fact scenes, per-scene coverage, dangling setups).
    #[serde(default)]
    pub telling: Option<ExistingRef>,
    /// Canon-order declaration path override (bypasses the pin). Omit to use
    /// `[continuity].canon_order_path`.
    #[serde(default)]
    pub order_path: Option<AgentPath>,
    /// `narrative-rules/v1` declaration path override (Round 891; the
    /// transition rules that declare the map). Omit to use
    /// `[continuity].rules_path`.
    #[serde(default)]
    pub rules_path: Option<AgentPath>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReportTimelineGapsArgs {
    /// Canon-order declaration path override (bypasses the pin).
    #[serde(default)]
    pub order_path: Option<AgentPath>,
    /// `narrative-rules/v1` declaration path override (Round 490; the
    /// interval rules). Omit to use `[continuity].rules_path`.
    #[serde(default)]
    pub rules_path: Option<AgentPath>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReportTransitionMapArgs {
    /// `narrative-rules/v1` declaration path override (Round 875; the
    /// transition rules DECLARE which predicate names a map edge). Omit to use
    /// `[continuity].rules_path`. No canon-order override — the map is read
    /// flat, exactly as the gate evaluates it.
    #[serde(default)]
    pub rules_path: Option<AgentPath>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReportFrameViewArgs {
    /// Epistemic frame to project.
    pub frame: ExistingRef,
    /// World-line branch. Omit for the default branch (`main`).
    #[serde(default)]
    pub branch: Option<ExistingRef>,
    /// Entity filter (Round 437) — the NPC-context query is frame ×
    /// branch × entity at T. Omit for the whole frame.
    #[serde(default)]
    pub entity: Option<ExistingRef>,
    /// Canon point (structure-section id).
    pub at: ExistingRef,
    /// Canon-order declaration path override (bypasses the pin).
    #[serde(default)]
    pub order_path: Option<AgentPath>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReportPayoffCoverageArgs {
    /// Canon-order declaration path override (bypasses the pin).
    #[serde(default)]
    pub order_path: Option<AgentPath>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReportIronyIntervalsArgs {
    /// Canon-order declaration path override (bypasses the pin).
    #[serde(default)]
    pub order_path: Option<AgentPath>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReportPlaythroughManuscriptArgs {
    /// Single-world filter (a registered branch id or `main`); omitted =
    /// every query world. Fail-loud on an unregistered id.
    #[serde(default)]
    pub world: Option<ExistingRef>,
    /// Canon-order declaration path override (bypasses the pin).
    #[serde(default)]
    pub order_path: Option<AgentPath>,
    /// Disclosure telling id (R506 render-brief carrier): annotate each
    /// begins-event with its disclosure decision (mode/first_at/surface) under
    /// the named telling. Fail-loud on a typo'd id.
    #[serde(default)]
    pub telling: Option<ExistingRef>,
    /// Reading-walk prune (R509): keep only each world's content scenes
    /// (begins>0) = the deterministic reading-copy walk.
    #[serde(default)]
    pub reading_walk: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReportForkTreeArgs {
    /// Canon-order declaration path override (bypasses the pin).
    #[serde(default)]
    pub order_path: Option<AgentPath>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReportDisclosureCoverageArgs {
    /// Telling id to classify (disclosed / hidden-by-design / never-planned).
    pub telling: ExistingRef,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReportPlayableWorldArgs {
    /// Telling id whose disclosure plan resolves the locators (required — a
    /// playable world IS a telling). Fail-loud on a typo'd id.
    pub telling: ExistingRef,
    /// Single-world filter (a registered branch id or `main`); omitted = every
    /// query world. The fork tree stays full regardless. Fail-loud on an
    /// unregistered id.
    #[serde(default)]
    pub world: Option<ExistingRef>,
    /// Canon-order declaration path override (bypasses the pin).
    #[serde(default)]
    pub order_path: Option<AgentPath>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReportQuestGraphArgs {
    /// Telling id whose disclosure plan resolves the giver-surface locators
    /// (required). Fail-loud on a typo'd id.
    pub telling: ExistingRef,
    /// Single-world filter (a registered branch id or `main`); omitted = every
    /// query world. The fork tree stays full regardless. Fail-loud on an
    /// unregistered id.
    #[serde(default)]
    pub world: Option<ExistingRef>,
    /// Canon-order declaration path override (bypasses the pin).
    #[serde(default)]
    pub order_path: Option<AgentPath>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DisclosureLeakArgs {
    /// Telling id whose plan is the gate's authored side.
    pub telling: ExistingRef,
    /// Path to the BLIND RE-EXTRACTED prose store to check.
    pub against: AgentPath,
    /// The world-line the re-extracted prose represents.
    pub world: ExistingRef,
    /// The frame whose re-extracted facts count as reader-established truth.
    pub truth_frame: ExistingRef,
    /// Canon-order declaration path override (bypasses the pin).
    #[serde(default)]
    pub order_path: Option<AgentPath>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RenderFidelityArgs {
    /// Path to the BLIND RE-EXTRACTED prose store to check.
    pub against: AgentPath,
    /// The assigned world-line the prose was rendered for.
    pub world: ExistingRef,
    /// Canon-order declaration path override (bypasses the pin).
    #[serde(default)]
    pub order_path: Option<AgentPath>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ProjectWorldArgs {
    /// The world-line to project onto.
    pub world: ExistingRef,
    /// Where to write the projected store.
    pub out: AgentPath,
    /// The store to project. Omitted = the workspace's own store.
    #[serde(default)]
    pub store: Option<AgentPath>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ImportTypingProposalsArgs {
    /// Path to a `typing-proposals/v1` JSON artifact (workspace-relative
    /// or absolute).
    pub proposals_path: AgentPath,
    /// Validate only — full verdicts, nothing written.
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReportEdgeCandidatesArgs {
    /// Canon-order declaration path override (bypasses the pin).
    #[serde(default)]
    pub order_path: Option<AgentPath>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ImportEdgeProposalsArgs {
    /// Path to an `edge-proposals/v1` JSON artifact (workspace-relative
    /// or absolute).
    pub proposals_path: AgentPath,
    /// Validate only — full verdicts, nothing written.
    #[serde(default)]
    pub dry_run: bool,
}

// Round 278 — Phase 1A inventory MCP arg structs.

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct InventoryIdArgs {
    /// Inventory id (e.g. `"ARP_07"`, `"TCP_RETRANSMISSION_TO_04"`).
    pub inventory_id: ExistingRef,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddInventoryEntryArgs {
    /// Stable inventory id. Must be non-empty, no whitespace.
    pub inventory_id: FreshId,
    /// Lifecycle state the entry is filed under. `deprecated` at creation is
    /// the case `reason` exists for.
    pub status: mnemosyne_core::InventoryStatus,
    /// Optional section binding without leading `§` (e.g. `"4.2.4"`).
    #[serde(default)]
    pub section_ref: Option<ExistingRef>,
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
    pub inventory_id: ExistingRef,
    /// Lifecycle state the entry moves to.
    pub status: mnemosyne_core::InventoryStatus,
    /// Optional reason. Omit to preserve existing; empty string clears.
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetInventorySectionRefArgs {
    pub inventory_id: ExistingRef,
    /// New section_ref without `§`. Omit (or pass `null`) AND set
    /// `clear: true` to unset the binding.
    #[serde(default)]
    pub section_ref: Option<ExistingRef>,
    /// Set to `true` to explicitly unset the section_ref. Exactly one
    /// of `section_ref` or `clear` must be present.
    #[serde(default)]
    pub clear: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RemoveInventoryEntryArgs {
    pub inventory_id: ExistingRef,
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
    pub entry_id: ExistingRef,
    /// Replacement decision_summary text. The audit half is untouched.
    pub value: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetChangelogPublishableBulletsArgs {
    /// Existing entry_id whose publishable bullet list will be replaced.
    pub entry_id: ExistingRef,
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
    pub entry_id: ExistingRef,
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
    pub entry_id: FreshId,
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
    raw: Option<&AgentPath>,
) -> Result<Option<ops::AbsolutePath>, String> {
    let Some(raw) = raw else { return Ok(None) };
    raw.resolve(workspace).map(Some)
}

/// The `format` a workspace-rooted path carries in the agent-facing schema.
///
/// This is what makes the population of path arguments DERIVED rather than
/// guessed. Round 999 wrote the resolution rule into the field descriptions and
/// gated it by asking whether the property's NAME ends in `_path`; `against` is
/// a path that is not called one, so it sat in the same struct as an obedient
/// `order_path`, silent and unresolved, invisible to that gate and to the
/// library gate that exempts this crate by name. A name is a convention. A type
/// is not.
const AGENT_PATH_FORMAT: &str = "mnemosyne-workspace-path";

/// The resolution rule, in the words an agent reads, in ONE place.
///
/// Stamped into every marked property by [`agent_facing_tools`] rather than
/// re-typed into each field's doc comment, which is where it lived until this
/// round — twenty-four copies of one sentence, policed by a gate that could
/// only see the ones whose names it recognised.
const AGENT_PATH_RULE: &str = "PATH RESOLUTION: a relative path is resolved against the WORKSPACE \
     ROOT (Round 1002), which is the only base an MCP caller can see. An \
     absolute path is used as given. The CLI resolves its own explicit paths \
     against the working directory instead (Round 538), because there the \
     caller chose that directory; here it belongs to whatever host launched \
     the server.";

/// A FILESYSTEM PATH AN AGENT SENDS — whose base is not the agent's to choose.
///
/// The type exists so that the base cannot be left unnamed. Its only way out is
/// [`AgentPath::resolve`], which demands the workspace; there is no `as_str`,
/// no `Deref`, and so no way to hand the raw string to `Path::new` and let the
/// OS resolve it against a working directory that belongs to whatever host
/// launched this server. That is not a hypothetical: it is what
/// `validate_disclosure_leak` and `validate_render_fidelity` did with
/// `against`, and because the store loader answers a missing path with an EMPTY
/// store, both gates returned the shape of a clean pass for a file they had
/// never opened.
#[derive(Debug, Clone, Deserialize)]
#[serde(transparent)]
pub struct AgentPath(String);

impl AgentPath {
    /// Resolve against the workspace the server was handed — the one base an
    /// MCP caller can see (Round 1002).
    fn resolve(&self, workspace: &std::path::Path) -> Result<ops::AbsolutePath, String> {
        ops::AbsolutePath::resolve(workspace, &self.0).map_err(|e| e.to_string())
    }
}

impl schemars::JsonSchema for AgentPath {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "AgentPath".into()
    }

    fn schema_id() -> std::borrow::Cow<'static, str> {
        concat!(module_path!(), "::AgentPath").into()
    }

    /// Inlined, so the marker lands on the PROPERTY an agent reads rather than
    /// behind a `$ref` the gate and the client would each have to follow.
    fn inline_schema() -> bool {
        true
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "format": AGENT_PATH_FORMAT,
        })
    }
}

/// The `format` an argument carries when it must NAME SOMETHING THE STORE
/// ALREADY HOLDS.
///
/// Round 1022's probe sweep measured that fifteen such arguments all refuse an
/// unregistered value loudly, and Round 1025 shipped that as the surface's last
/// carry: the sweep would be exactly as green if one of them answered with an
/// EMPTY result instead, because it has no way to know which polarity an
/// argument was meant to have. This round measured the carry rather than
/// repeating it — `query_section` was made to answer `{}` for a section that
/// does not exist, and the whole workspace stayed green at 1727 tests. Nothing
/// anywhere reads the difference between a correct refusal and a fabricated
/// empty answer.
///
/// So the polarity becomes a TYPE, the way Round 1014 made a path one, and the
/// sweep derives its oracle from the mark instead of a later reader trusting
/// prose.
const EXISTING_REF_FORMAT: &str = "mnemosyne-existing-ref";

/// The `format` an argument carries when it is the identifier a NEW record is
/// filed under.
///
/// It is the other half of the same question and not a decoration. Without it,
/// the only way to say "this argument is not an existing reference" is to leave
/// it unmarked, which is also what an argument nobody has classified looks
/// like — and then the bridge below, which asks whether every id-shaped NAME is
/// typed, would have to carry a hand-written list of exemptions. That list is
/// the thing Round 1014 deleted.
const FRESH_ID_FORMAT: &str = "mnemosyne-fresh-id";

/// The contract of an existing reference, in the words an agent reads, in ONE
/// place — stamped in by [`agent_facing_tools`] rather than re-typed per field.
const EXISTING_REF_RULE: &str =
    "EXISTING REFERENCE: this value must already name something the store \
     holds. A value that names nothing is REFUSED and the refusal names the \
     value; it is never answered with an empty or default result, which would \
     be indistinguishable from a real answer about a real thing.";

/// The `format` an argument carries when it names something the store must
/// hold, but is admitted by the CALL and refused at the SCAN boundary.
///
/// It exists because typing the surface found arguments that are references and
/// are nonetheless accepted with a value naming nothing, and the store is right
/// about them: `AtomicSection::superseded_by` and `resolved_by` are projected
/// as cross-refs and resolved by the orphan scan, which the field-classification
/// table records as `Coverage::Projection` — a baseline gate reached by a
/// different route. Marking those `ExistingRef` would have made the type lie
/// about where the refusal happens, and leaving them unmarked would have said
/// nothing at all about a reference an agent can dangle.
const SCANNED_REF_FORMAT: &str = "mnemosyne-scanned-ref";

/// The contract of a scan-checked reference, in one place, stamped the same way.
const SCANNED_REF_RULE: &str =
    "SCANNED REFERENCE: this value must name something the store holds, and \
     unlike an existing reference it is ACCEPTED here and refused at the scan \
     boundary — `validate_workspace` reports it as an atomic orphan ref. The \
     call succeeding is therefore not evidence that the value resolves.";

/// The contract of a fresh identifier, in one place, stamped the same way.
const FRESH_ID_RULE: &str =
    "FRESH IDENTIFIER: this value is the id the new record is filed under. \
     Naming something that does NOT exist is the normal case; what the store \
     refuses is a second record under an id it already holds, so calling twice \
     with one id leaves one record and never two.";

/// A REFERENCE TO SOMETHING THE STORE ALREADY HOLDS.
///
/// The type carries no behaviour of its own, and that is deliberate rather than
/// unfinished. `AgentPath` can forbid its own misuse because resolving a path
/// against the wrong base is a COMPUTATION, and removing `as_str` removes the
/// way to do it. Whether a name is registered is not a computation this crate
/// performs: the lookup belongs to `ops`, and a second membership check here
/// would be two write paths for one invariant — the shape `CLAUDE.md` forbids
/// outright. What a type CAN do is make the polarity travel with the field into
/// the schema, wherever it is declared and whatever it is called, so the sweep
/// that already sends every argument a value naming nothing knows which answer
/// is the correct one.
#[derive(Debug, Clone, Deserialize)]
#[serde(transparent)]
pub struct ExistingRef(String);

impl std::ops::Deref for ExistingRef {
    type Target = str;

    fn deref(&self) -> &str {
        &self.0
    }
}

/// THE IDENTIFIER A NEW RECORD IS FILED UNDER.
#[derive(Debug, Clone, Deserialize)]
#[serde(transparent)]
pub struct FreshId(String);

impl std::ops::Deref for FreshId {
    type Target = str;

    fn deref(&self) -> &str {
        &self.0
    }
}

/// Both markers stamp themselves the way [`AgentPath`] does: inlined onto the
/// property an agent reads, so neither the gate nor a client has to follow a
/// `$ref` to see it.
macro_rules! marker_schema {
    ($($ty:ident => $format:ident;)*) => {$(
        impl schemars::JsonSchema for $ty {
            fn schema_name() -> std::borrow::Cow<'static, str> {
                stringify!($ty).into()
            }

            fn schema_id() -> std::borrow::Cow<'static, str> {
                concat!(module_path!(), "::", stringify!($ty)).into()
            }

            fn inline_schema() -> bool {
                true
            }

            fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
                schemars::json_schema!({
                    "type": "string",
                    "format": $format,
                })
            }
        }
    )*};
}

/// A REFERENCE THE SCAN RESOLVES RATHER THAN THE CALL.
#[derive(Debug, Clone, Deserialize)]
#[serde(transparent)]
pub struct ScannedRef(String);

impl std::ops::Deref for ScannedRef {
    type Target = str;

    fn deref(&self) -> &str {
        &self.0
    }
}

marker_schema! {
    ExistingRef => EXISTING_REF_FORMAT;
    FreshId => FRESH_ID_FORMAT;
    ScannedRef => SCANNED_REF_FORMAT;
}

/// The `format` an argument carries when it is a canonical UTC instant.
const UTC_INSTANT_FORMAT: &str = "mnemosyne-utc-instant";

/// The contract of a canonical instant, in one place, stamped the same way —
/// and honest about where the published `pattern` stops.
const UTC_INSTANT_RULE: &str =
    "CANONICAL UTC INSTANT: the `pattern` on this property states the exact \
     layout the store enforces — no offset and no fractional seconds, so the \
     append-only ledger this feeds sorts chronologically by sorting lexically. \
     The pattern states the LAYOUT ONLY: the store additionally requires a real \
     calendar day and refuses one that is not, so matching the pattern is \
     necessary and not sufficient.";

/// A CANONICAL UTC INSTANT.
///
/// It carries no validation of its own, for the reason [`ExistingRef`] does not:
/// `atomic` owns the check, and a second one here would be two write paths for
/// one invariant. What the type adds is that the SHAPE travels into the schema —
/// as a `pattern` DERIVED from the same layout table the checker walks, so the
/// rule an agent is handed and the rule it is judged by cannot come apart. Round
/// 1023 put the form in one home and gated the two surfaces that STATE it in
/// prose; this is the machine-readable half, which is the half an agent's client
/// can act on.
#[derive(Debug, Clone, Deserialize)]
#[serde(transparent)]
pub struct UtcInstant(String);

impl std::ops::Deref for UtcInstant {
    type Target = str;

    fn deref(&self) -> &str {
        &self.0
    }
}

impl schemars::JsonSchema for UtcInstant {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "UtcInstant".into()
    }

    fn schema_id() -> std::borrow::Cow<'static, str> {
        concat!(module_path!(), "::UtcInstant").into()
    }

    fn inline_schema() -> bool {
        true
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "format": UTC_INSTANT_FORMAT,
            "pattern": atomic::utc_instant_pattern(),
        })
    }
}

/// Every rule an agent-facing marker stamps, in the one table both the stamping
/// and the gate read. A marker added here is stamped and gated by that alone.
const MARKER_RULES: &[(&str, &str)] = &[
    (AGENT_PATH_FORMAT, AGENT_PATH_RULE),
    (EXISTING_REF_FORMAT, EXISTING_REF_RULE),
    (SCANNED_REF_FORMAT, SCANNED_REF_RULE),
    (FRESH_ID_FORMAT, FRESH_ID_RULE),
    (UTC_INSTANT_FORMAT, UTC_INSTANT_RULE),
];

/// THE POLARITY OF A PROPERTY NO FIELD OF THIS CRATE OWNS.
///
/// A type is the better home and is used everywhere it can be, because it
/// travels with the field and an author cannot forget it. Typing the surface
/// found the two arguments where it CANNOT be one, and the reason is worth
/// stating rather than working around: `add_fact` and `amend_fact` are both
/// shaped by `atomic::FactImport` — the ONE fact DTO, shared with the CLI
/// importer — and `fact_id` has OPPOSITE polarity in the two. `add_fact` files
/// a new fact under it; `amend_fact` revises the fact it names. The same field
/// of the same struct is therefore fresh in one tool and a reference in the
/// other, so the polarity is not a property of the FIELD at all. It belongs to
/// the (tool, argument) pair.
///
/// A hand-written table is what Round 1014 deleted, and this one is admitted
/// only on the terms Round 1022 set for an exception: every row is EXECUTED —
/// the sweep probes it exactly as it probes a typed argument, and a row whose
/// tool does not behave as the row claims turns that tool's probe red. What the
/// table cannot do, and a type can, is force a NEW composed argument to declare
/// itself; that one is caught only if its name is id-shaped, by the bridge.
///
/// THE SAME DTO GIVES THE TABLE ITS SECOND REASON, and the round that wrote the
/// paragraph above did not have it: `canon_from` and `frame` are references in
/// BOTH tools, so the polarity is not divided at all — what divides is where the
/// field lives. `FactImport` is `atomic`'s, and `atomic` does not depend on this
/// crate, so no field of it can carry a marker declared here. Those four rows
/// are not an opinion the sweep trusts; they are stamped into the schema by the
/// loop below and then probed like any typed argument, which is how the round
/// that added them found the arguments in the first place: their handlers refuse
/// an unregistered value and their schemas said nothing at all about it, so the
/// only way an agent could learn either rule was to send a wrong value.
const COMPOSED_POLARITY: &[(&str, &str, &str)] = &[
    ("add_fact", "fact_id", FRESH_ID_FORMAT),
    ("amend_fact", "fact_id", EXISTING_REF_FORMAT),
    ("add_fact", "canon_from", EXISTING_REF_FORMAT),
    ("amend_fact", "canon_from", EXISTING_REF_FORMAT),
    ("add_fact", "frame", EXISTING_REF_FORMAT),
    ("amend_fact", "frame", EXISTING_REF_FORMAT),
];

/// The tools an agent is shown, with each marker's rule stamped into every
/// argument whose TYPE carries that marker.
///
/// THE ONE READER OF THE ROUTER for anything agent-facing: `list_tools` and
/// `get_tool` both come through here, and so does the gate, because a rule that
/// two readers apply differently is the half-enforced invariant this codebase
/// forbids.
///
/// Stamping rather than asserting is the point. A gate over hand-written
/// descriptions can only ever report that somebody forgot; generating them
/// means a new path argument is born saying what it resolves against, and the
/// author cannot forget because there is nothing to remember.
fn agent_facing_tools() -> Vec<rmcp::model::Tool> {
    let mut tools = MnemosyneServer::tool_router().list_all();
    for tool in &mut tools {
        let name = tool.name.to_string();
        let schema = std::sync::Arc::make_mut(&mut tool.input_schema);
        let Some(props) = schema.get_mut("properties").and_then(|p| p.as_object_mut()) else {
            continue;
        };
        // A COMPOSED PROPERTY IS MARKED HERE AND THEN STAMPED BELOW BY THE SAME
        // LOOP AS A TYPED ONE, so there is one reader of a marker and not two.
        for (tool_name, arg, format) in COMPOSED_POLARITY {
            if *tool_name != name {
                continue;
            }
            if let Some(property) = props.get_mut(*arg).and_then(|p| p.as_object_mut()) {
                property.insert("format".to_string(), serde_json::Value::from(*format));
            }
        }
        for (_, property) in props.iter_mut() {
            let Some(property) = property.as_object_mut() else {
                continue;
            };
            let marked = property.get("format").and_then(|f| f.as_str());
            let Some((_, rule)) = MARKER_RULES.iter().find(|(f, _)| Some(*f) == marked) else {
                continue;
            };
            let described = property
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or_default();
            let stamped = if described.is_empty() {
                (*rule).to_string()
            } else {
                format!("{described}\n\n{rule}")
            };
            property.insert("description".to_string(), serde_json::Value::from(stamped));
        }
    }
    tools
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
    async fn set_section_title(&self, args: Parameters<SetSectionTitleArgs>) -> CallToolResult {
        let section = strip_section_marker(&args.0.section_id).to_string();
        let title = args.0.title.clone();
        let outcome =
            self.run_mutate(|store, path| atomic::set_section_title(store, path, &section, &title));
        self.finish_mutate(outcome)
    }

    #[tool(
        description = "Set Section.parent_doc (re-bind section to a different owning doc). Section must exist."
    )]
    async fn set_section_parent_doc(
        &self,
        args: Parameters<SetSectionParentDocArgs>,
    ) -> CallToolResult {
        let section = strip_section_marker(&args.0.section_id).to_string();
        let parent_doc = args.0.parent_doc.clone();
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
    async fn set_section_intent(&self, args: Parameters<SetSectionIntentArgs>) -> CallToolResult {
        let section = strip_section_marker(&args.0.section_id).to_string();
        let intent = args.0.intent.clone();
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

    #[tool(description = "Set Section.alternatives_rejected. Replaces existing.")]
    async fn set_section_alternatives(
        &self,
        args: Parameters<SetSectionAlternativesArgs>,
    ) -> CallToolResult {
        let section = strip_section_marker(&args.0.section_id).to_string();
        let alternatives = args.0.alternatives.clone();
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
        let kind = args.0.kind;
        let outcome = self.run_mutate(|store, path| {
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
        let status = args.0.status;
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
        let kind = args.0.kind;
        let reason = args.0.reason.clone();
        let outcome = self.run_mutate(|store, path| {
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
        let expectation = args.0.expectation;
        let reason = args.0.reason.clone();
        let outcome = self.run_mutate(|store, path| {
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
        let expectation = args.0.expectation;
        let reason = args.0.reason.clone();
        let outcome = self.run_mutate(|store, path| {
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
            let (kind, method, verdict) = (a.confirmer_kind, a.method, a.verdict);
            let event = atomic::ConfirmationEvent {
                claim,
                confirmer: atomic::Confirmer {
                    kind,
                    id: a.confirmer_id.to_string(),
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
                timestamp: a.timestamp.to_string(),
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
                (Some(p), Some(at)) => Some((&**p, &**at)),
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
            atomic::add_disclosure_plan(
                store,
                path,
                &a.telling_id,
                a.default_mode.as_str(),
                &a.description,
            )
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
                    mode: a.mode.as_str(),
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
        description = "Reasoned-mutation ledger read (R1024, read-only): WHY the store changed. One append-only row per reasoned mutation — the primitive, what it changed, and the reason the author gave — in the order the acts happened. Ten primitives take a mandatory `reason`; before this ledger every one of them validated it non-blank and then discarded it, so the rationale an agent spent on a change reached nothing and no later reader could ask why the canon moved. `target` narrows to ONE record, INCLUDING one that no longer exists: five of the ten are removals, and a removed thing is exactly what no other read can answer about. `total` is the whole ledger even when `rows` is filtered, so a narrow answer is never readable as the whole of it."
    )]
    async fn report_mutation_reasons(
        &self,
        args: Parameters<ReportMutationReasonsArgs>,
    ) -> CallToolResult {
        match ops::mutation_reason_report(&self.workspace, None, args.0.target.as_deref()) {
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
        description = "Frame-scoped continuity scan (R431, read-only): same-(frame, branch) conflicting pairs whose derived canon extents co-hold are violations; cross-scope pairs are data. With a declared narrative-rules/v1 artifact (R449) it also derives typed-claim rule findings — exclusive (one co-holding value per subject / one holder per object), transition (allowed state steps on succession edges), and interval (R489: a scalar/arithmetic relation value(left) − value(right) op bound per frame-world-subject) — plus the unchained_state_pairs, unchained_unreachable_pairs (R916: the subset no ROUTE joins in the hierarchy-augmented map, so no untold journey could cover it — EVERY transition rule since R924, the claim needing no genre and being conservative on a directed rule because the walk symmetrizes) and interval_unverifiable honesty counts, each of which is NULL when no rule of its class was declared (R924: a silence is not a zero). It also judges every declared QUEST PREREQUISITE against every road (R1031): a `requires` claim promises its object quest is discharged FIRST, so a world-line that discharges the subject must discharge the object too, strictly earlier — `quest_prerequisite_unreachable` names the road and whether it discharges the prerequisite late or never. `quest_prerequisite_judgements` carries every (quest, prerequisite, world, discharge) row including the ones that prove nothing (inapplicable = this road never discharges the quest; unverifiable = the order cannot compare the two coordinates), and is EMPTY when no `requires` claim is declared at all. Interval violations ride a SEPARATE per-class severity (R491, interval_severity, OFF by default — a timeline gap can be an intentional time-bend); structural violations ride severity. Returns the JSON report (both severities, interval_violation_count, counts, violations); gating policy belongs to the caller."
    )]
    async fn validate_continuity(
        &self,
        args: Parameters<ValidateContinuityArgs>,
    ) -> CallToolResult {
        match ops::continuity_scan(
            &self.workspace,
            None,
            match mcp_path(&self.workspace, args.0.order_path.as_ref()) {
                Ok(v) => v,
                Err(e) => return Self::tool_error(e),
            }
            .as_ref(),
            match mcp_path(&self.workspace, args.0.rules_path.as_ref()) {
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
        description = "Propose-verdict (R588, DRY RUN): the generate-gate-repair loop's atomic gate. Reads a candidate import-facts manifest from manifest_path, applies it to a THROWAWAY in-memory clone of the store, runs the shape invariants + the continuity gate, and returns verdict=commit|rollback plus actionable violations. THE TWO CAN DISAGREE: `verdict` reflects only violations your workspace's `[continuity] severity` makes GATING, so a workspace that declares none gets `commit` with `violation_count` non-zero beside it (Round 1004). Read `violations`, not `verdict` alone (each carries rule + locus {facts,field,frame,branch,at} + expected + repair_hint + message). The real store is NEVER written — on commit, apply for real via the import-facts CLI. Deterministic, AI out of the gate. Fail-loud on an unreadable/unparseable manifest."
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
                // The RESOLVED path, matching the read error above: Round 1001
                // found this message naming the argument received rather than
                // the file opened, which is identical in production and
                // misleading in exactly the case where it is wrong.
                return Self::tool_error(format!(
                    "parse manifest {} ({}): {e}",
                    manifest_path,
                    mnemosyne_atomic::FACTS_MANIFEST_SHAPE
                ));
            }
        };
        match ops::propose_verdict(
            &self.workspace,
            None,
            match mcp_path(&self.workspace, args.0.order_path.as_ref()) {
                Ok(v) => v,
                Err(e) => return Self::tool_error(e),
            }
            .as_ref(),
            match mcp_path(&self.workspace, args.0.rules_path.as_ref()) {
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
            match mcp_path(&self.workspace, args.0.order_path.as_ref()) {
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
            match mcp_path(&self.workspace, args.0.order_path.as_ref()) {
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
            match mcp_path(&self.workspace, args.0.order_path.as_ref()) {
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
            match mcp_path(&self.workspace, args.0.order_path.as_ref()) {
                Ok(v) => v,
                Err(e) => return Self::tool_error(e),
            }
            .as_ref(),
            match mcp_path(&self.workspace, args.0.rules_path.as_ref()) {
                Ok(v) => v,
                Err(e) => return Self::tool_error(e),
            }
            .as_ref(),
            // This tool has never advertised a road filter, and `None` is the
            // whole-store read it has always answered (Round 1049 moved the
            // CLI's filter into the projection; it did not add one here).
            None,
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
            match mcp_path(&self.workspace, args.0.rules_path.as_ref()) {
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
            match mcp_path(&self.workspace, args.0.order_path.as_ref()) {
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
            match mcp_path(&self.workspace, args.0.order_path.as_ref()) {
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
            match mcp_path(&self.workspace, args.0.order_path.as_ref()) {
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
            match mcp_path(&self.workspace, args.0.order_path.as_ref()) {
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
            match mcp_path(&self.workspace, args.0.order_path.as_ref()) {
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
        description = "Quest graph (R559/568, read-only): the fact->quest leg a pinion narrative runtime / authoring consumer needs, the sibling of report_playable_world. Per telling, every derived quest (pursues object / requires endpoint / completed_by subject) projected to a QuestNode — objective + actor (pursues) + prerequisites (requires) + giving setups + per-world DERIVED open/done (the R442 payoff coverage: a quest done on one road and open on another) + the completion fact (with discharger) + the giver-surface MapLocator (R557). Plus roads (R1061): per world-line, the scene walk each locator's scene_ordinal INDEXES, read from the manuscript this report already reuses — the pointer and what it points into travel together, as they do in report_playable_world. A pure JOIN over payoff-coverage + playable-world; `world` filters the per-world map (the fork tree stays full). Reading surface, never gated; quest STATE derived per world-line, never stored. Executable quest logic (lifecycle/guards) is SCE/pinion's. Fail-loud on a typo'd telling / unregistered world."
    )]
    async fn report_quest_graph(&self, args: Parameters<ReportQuestGraphArgs>) -> CallToolResult {
        match ops::quest_graph_report(
            &self.workspace,
            None,
            args.0.world.as_deref(),
            match mcp_path(&self.workspace, args.0.order_path.as_ref()) {
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
        description = "Authoring frontier (R589, read-only): the consolidated coverage-gap surface an unattended generate-gate-repair loop pulls its next work from, JOINed from the scattered projections. Always: zero_fact_scenes (sections with no fact anchored) + unplaced_scenes (R667: every section the canon order does not position, content-independent — deliberately NOT in total_gaps, each member is already counted once as zero-fact or unordered) + unordered_scenes (R596: fact-bearing sections the resolved order does not place; with NO order declared every fact-bearing scene is here) + tellings_declared (R1215: every telling the store HAS — empty is the third state, the telling-scoped axes below are unaskable rather than un-asked) + telling_needed (R1217: `carried` = the apparatus only a telling-scoped read projects (world-lines beyond main, quests), `gap` = it is carried and no telling is declared, so the playable-world / quest-graph seam cannot be opened at all; one add-disclosure-plan closes it and it counts in total_gaps) + scene_coverage (the fact ids anchored per section, NAMED not counted since R1053, plus the derived structural quest-plumbing subset of them, R619) + structural_facts (those same structural ids flat, to JOIN with each fact's branch, R619) + branch_owned_density (per world-line, own facts over its full traversed road — a divergent world that looks full by inheritance but owns little reads LOW, R617/R619) + dangling_setups (per world-line, R442 Expected facts with no visible payoff) + total_gaps. With telling: unresolved_quests (R568) + never_planned_disclosures (R507, facts never given an explicit disclosure decision) + disclosures_seated_before_truth (R949, an authored surface.scene earlier on the fact's own road than its canon_from). Plus map_frontier (R891): per declared map, the registered places its adjacency predicate's leg kinds admit that are NOT a node of it (places with no way in or out), plus costs/guards keyed to a non-edge. transition_rules 0 is the THIRD state — no adjacency predicate is declared, so the store cannot know which facts are edges — never 'no map work'. Pure read, never gated. Fail-loud on a typo'd telling."
    )]
    async fn report_authoring_frontier(
        &self,
        args: Parameters<ReportAuthoringFrontierArgs>,
    ) -> CallToolResult {
        match ops::authoring_frontier_report(
            &self.workspace,
            None,
            match mcp_path(&self.workspace, args.0.order_path.as_ref()) {
                Ok(v) => v,
                Err(e) => return Self::tool_error(e),
            }
            .as_ref(),
            args.0.telling.as_deref(),
            match mcp_path(&self.workspace, args.0.rules_path.as_ref()) {
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
        let against = match mcp_path(&self.workspace, Some(&a.against)) {
            Ok(p) => p.expect("a Some input yields a Some path"),
            Err(e) => return Self::tool_error(e),
        };
        match ops::disclosure_leak_report(
            &self.workspace,
            None,
            &against,
            match mcp_path(&self.workspace, a.order_path.as_ref()) {
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
        description = "Emit the SINGLE-WORLD PROJECTION validate_render_fidelity requires of its `against` store (R1070). That gate is single-world by contract, so a store spanning several world-lines draws its siblings off-path in bulk — a verdict about the caller, not the prose. Keeps every fact whose DECLARED world is part of `world`'s world-line (the world_membership lattice) and drops the rest; `store` omitted = the workspace's own store. Selection is by branch so the gate can still disagree on the COORDINATE — projecting by coordinate would hand the gate its own predicate and report clean forever. Fail-loud on a typo'd world."
    )]
    async fn project_world(&self, args: Parameters<ProjectWorldArgs>) -> CallToolResult {
        let a = args.0;
        let out = match mcp_path(&self.workspace, Some(&a.out)) {
            Ok(p) => p.expect("a Some input yields a Some path"),
            Err(e) => return Self::tool_error(e),
        };
        match ops::project_world_store(
            &self.workspace,
            None,
            match mcp_path(&self.workspace, a.store.as_ref()) {
                Ok(v) => v,
                Err(e) => return Self::tool_error(e),
            }
            .as_ref(),
            &a.world,
            &out,
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
        let against = match mcp_path(&self.workspace, Some(&a.against)) {
            Ok(p) => p.expect("a Some input yields a Some path"),
            Err(e) => return Self::tool_error(e),
        };
        match ops::render_fidelity_report(
            &self.workspace,
            None,
            &against,
            match mcp_path(&self.workspace, a.order_path.as_ref()) {
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
                "entry_id": &*args.0.entry_id,
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
        description = "Register a new inventory entry. Duplicate inventory_id rejects. Registering as deprecated surfaces any pre-existing cite-sites via the mutate-time cascade. section_ref omits the leading §."
    )]
    async fn add_inventory_entry(&self, args: Parameters<AddInventoryEntryArgs>) -> CallToolResult {
        let inventory_id = args.0.inventory_id.clone();
        let status = args.0.status;
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
        let status = args.0.status;
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
    /// The tool list an agent is shown. Overridden — the macro would hand back
    /// the router's own schema, which is the one place the path-resolution rule
    /// has not yet been stamped in.
    async fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<rmcp::model::ListToolsResult, rmcp::ErrorData> {
        Ok(rmcp::model::ListToolsResult {
            tools: agent_facing_tools(),
            meta: None,
            next_cursor: None,
        })
    }

    /// One tool, from the SAME reader as the list. A client that fetches a tool
    /// singly must not be told something different from one that lists them —
    /// two readers of one rule with different answers is the half-enforced
    /// invariant this codebase bans.
    fn get_tool(&self, name: &str) -> Option<rmcp::model::Tool> {
        agent_facing_tools().into_iter().find(|t| t.name == name)
    }

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

    /// The frontier tool's description names EVERY field its report emits
    /// (Round 1217).
    ///
    /// The description is the whole of what an agent knows before it calls: the
    /// tool returns JSON with no schema, so a field the sentence omits is a
    /// field nothing tells the agent to read. It is hand-written prose beside a
    /// struct that grows, and it HAD already drifted — Round 1215 added
    /// `tellings_declared`, the field that tells a loop whether the
    /// telling-scoped axes are unaskable rather than un-asked, and the sentence
    /// went on describing a report without it. Nothing was red. This round found
    /// it while adding a second field, which is exactly the moment the next one
    /// would be missed too.
    ///
    /// The field names come FROM the type, so a field added tomorrow joins this
    /// check on its own. SCOPE, stated rather than implied: this holds for ONE
    /// tool. The other report tools' descriptions have no such law, and the
    /// mapping from a tool to the type it returns is a hand-written pair, which
    /// is why this is one pair and not a table pretending to be a rule.
    #[test]
    fn the_frontier_tool_describes_every_field_its_report_emits() {
        let described = agent_facing_tools()
            .into_iter()
            .find(|t| t.name == "report_authoring_frontier")
            .and_then(|t| t.description.clone())
            .expect("the frontier tool is routed and described")
            .to_string();
        let report = serde_json::to_value(ops::AuthoringFrontierReport::default())
            .expect("the report serializes");
        let emitted: Vec<String> = report
            .as_object()
            .expect("the report is a json object")
            .keys()
            .cloned()
            .collect();
        // `telling` is skipped when None and IS in the default, so the emitted
        // set is the always-present half; the conditional fields are named by
        // the same sentence and checked here when the default carries them.
        let missing: Vec<&String> = emitted
            .iter()
            .filter(|key| !described.contains(key.as_str()))
            .collect();
        assert!(
            missing.is_empty(),
            "the frontier tool's description does not name {missing:?} — an agent reading it \
             learns nothing about a field the tool returns. Emitted: {emitted:?}"
        );
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

    /// A RELATIVE `against` NAMES A FILE IN THE WORKSPACE THE AGENT WAS HANDED.
    ///
    /// Round 1002 decided this for every path an agent sends, and Round 1001
    /// swept the wires to make it true. `against` was missed, and the two gates
    /// that should have caught it are both keyed on the NAME `*_path`: the
    /// schema-doc gate skips any property not ending in `_path`, and the
    /// library gate exempts the wire crate by name. `against` is a path that is
    /// not called one, so it was invisible to both while sitting in the same
    /// struct as an `order_path` that obeys the rule.
    ///
    /// The blind store is BUILT BY THE SERVER rather than written as a literal,
    /// so the fixture cannot drift from the store schema.
    ///
    /// NON-VACUITY: the absolute arm must succeed and must report the fact the
    /// file contains, so a failure in the relative arm is about the BASE and
    /// not about a file this workspace could never read.
    #[tokio::test]
    async fn a_relative_against_path_names_a_file_in_the_agents_workspace() {
        let sections = || {
            serde_json::from_value::<ImportSectionsArgs>(serde_json::json!({
                "sections": [
                    {"section_id": "sc-01", "parent_doc": "spec", "title": "one"},
                    {"section_id": "sc-02", "parent_doc": "spec", "title": "two"},
                ],
            }))
            .expect("sections parse")
        };

        // The re-extracted store, produced by the same writer that produces a
        // real one.
        let blind_ws = agent_workspace();
        let blind = MnemosyneServer::new(blind_ws.path().to_path_buf()).expect("server");
        assert!(blind.import_sections(Parameters(sections())).await.is_error != Some(true));
        let facts: atomic::FactsManifest = serde_json::from_value(serde_json::json!({
            "frames": [{"frame_id": "ground-truth"}],
            "entity_kinds": [{"kind_id": "place"}],
            "entities": [{"entity_id": "p-a", "kind": "place"}],
            "facts": [{
                "fact_id": "f-blind", "frame": "ground-truth", "claim": "the prose said so",
                "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["p-a"],
            }],
        }))
        .expect("blind facts parse");
        assert!(blind.import_facts(Parameters(facts)).await.is_error != Some(true));
        let blind_bytes =
            std::fs::read_to_string(blind_ws.path().join("docs/.atomic/workspace.atomic.json"))
                .expect("the blind store");

        let ws = agent_workspace();
        std::fs::write(ws.path().join("blind.json"), &blind_bytes).expect("blind store");
        std::fs::write(
            ws.path().join("order.json"),
            serde_json::json!({"schema": "canon-order/v1", "edges": [["sc-01", "sc-02"]]})
                .to_string(),
        )
        .expect("order");
        let server = MnemosyneServer::new(ws.path().to_path_buf()).expect("server");
        assert!(
            server
                .import_sections(Parameters(sections()))
                .await
                .is_error
                != Some(true)
        );

        let fidelity = |against: String| {
            serde_json::from_value::<RenderFidelityArgs>(serde_json::json!({
                "against": against,
                "world": "main",
                "order_path": "order.json",
            }))
            .expect("args parse")
        };
        let absolute = answer_text(
            &server
                .validate_render_fidelity(Parameters(fidelity(
                    ws.path().join("blind.json").display().to_string(),
                )))
                .await,
        );
        assert!(
            absolute.contains("\"reextracted_facts\": 1"),
            "the absolute arm did not read the one fact the blind store holds, \
             so the relative arm below would be measuring the fixture rather \
             than the base: {absolute}"
        );

        let relative = answer_text(
            &server
                .validate_render_fidelity(Parameters(fidelity("blind.json".to_string())))
                .await,
        );
        assert_eq!(
            relative, absolute,
            "`against: \"blind.json\"` did not reach the file of that name in \
             the workspace the agent was handed. A relative path an agent sends \
             is resolved against the WORKSPACE ROOT (Round 1002) — the server \
             process's working directory belongs to whatever host launched it \
             and the agent can neither see nor choose it"
        );
    }

    /// A BLIND-ACCEPTANCE GATE MUST NOT PASS A FILE THAT IS NOT THERE.
    ///
    /// `AtomicStore::load` answers a MISSING path with an EMPTY STORE, which is
    /// right for the workspace's own sidecar — a store that does not exist yet
    /// is the bootstrap state, and `load_atomic_store_missing_sidecar_is_empty`
    /// pins that. It is the opposite of right for a store the CALLER NAMED:
    /// both render-acceptance gates read `against` that way, so a path that
    /// does not resolve comes back `reextracted_facts: 0`, `off_path: []`,
    /// `leaks: []` — the shape of a clean pass, from a gate that evaluated
    /// nothing. That is the failure the contract already names in its own
    /// words: a gate that evaluated NOTHING must never read the same as a gate
    /// that PASSED.
    ///
    /// The paths here are ABSOLUTE, so this is about the file's absence and not
    /// about which base a relative path resolves against (the sibling defect,
    /// asserted above). Both wires share these two ops functions, so the CLI
    /// twins are covered by the same repair.
    #[tokio::test]
    async fn a_gate_handed_a_store_that_is_not_there_refuses_instead_of_passing() {
        let ws = agent_workspace();
        std::fs::write(
            ws.path().join("order.json"),
            serde_json::json!({"schema": "canon-order/v1", "edges": [["sc-01", "sc-02"]]})
                .to_string(),
        )
        .expect("order");
        let server = MnemosyneServer::new(ws.path().to_path_buf()).expect("server");
        let sections: ImportSectionsArgs = serde_json::from_value(serde_json::json!({
            "sections": [
                {"section_id": "sc-01", "parent_doc": "spec", "title": "one"},
                {"section_id": "sc-02", "parent_doc": "spec", "title": "two"},
            ],
        }))
        .expect("sections parse");
        assert!(server.import_sections(Parameters(sections)).await.is_error != Some(true));
        let facts: atomic::FactsManifest = serde_json::from_value(serde_json::json!({
            "frames": [{"frame_id": "ground-truth"}],
            "disclosure_plans": [{"telling_id": "t-quiet", "default_mode": "state"}],
        }))
        .expect("facts parse");
        assert!(server.import_facts(Parameters(facts)).await.is_error != Some(true));

        let absent = ws.path().join("never-written.json").display().to_string();
        assert!(
            !std::path::Path::new(&absent).exists(),
            "the fixture's premise is that this file does not exist"
        );

        let fidelity: RenderFidelityArgs = serde_json::from_value(serde_json::json!({
            "against": absent, "world": "main", "order_path": "order.json",
        }))
        .expect("args parse");
        let answered = server.validate_render_fidelity(Parameters(fidelity)).await;
        assert_eq!(
            answered.is_error,
            Some(true),
            "the fidelity gate answered a store that is not there instead of \
             refusing it, and the answer an agent gets is the shape of a pass: \
             {}",
            answer_text(&answered)
        );

        let leak: DisclosureLeakArgs = serde_json::from_value(serde_json::json!({
            "telling": "t-quiet", "against": absent, "world": "main",
            "truth_frame": "ground-truth", "order_path": "order.json",
        }))
        .expect("args parse");
        let answered = server.validate_disclosure_leak(Parameters(leak)).await;
        assert_eq!(
            answered.is_error,
            Some(true),
            "the leak gate answered a store that is not there instead of \
             refusing it: {}",
            answer_text(&answered)
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
    ///
    /// IT IS EMPTY, AND IT STAYS RATHER THAN GOING. Round 986 found one pair of
    /// a hundred and twenty-two with anything behind it; every one of them now
    /// has a differential case. An empty list is not a list with no purpose:
    /// the accounting refuses any pair that is in NEITHER list, so this is
    /// where the next optional argument added to any tool lands, by name and
    /// on the round that adds it, instead of joining a silent set. Deleting it
    /// would make that failure impossible to express.
    const UNEXERCISED: &[(&str, &str)] = &[];

    /// The same admission for the REQUIRED half. Generated from this gate's own
    /// failure output rather than transcribed, the way Round 986 produced the
    /// optional list — a hand-copied population is the defect the accounting
    /// exists to stop.
    const UNEXERCISED_REQUIRED: &[(&str, &str)] = &[];

    /// The REQUIRED arguments of every routed tool — the half of the surface
    /// the exercised/unexercised accounting does not cover, counted so the
    /// ratio it prints cannot read as coverage of the whole.
    fn required_arguments() -> Vec<(String, String)> {
        let mut out = Vec::new();
        for tool in agent_facing_tools() {
            for key in tool
                .input_schema
                .get("required")
                .and_then(|r| r.as_array())
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str())
            {
                out.push((tool.name.to_string(), key.to_string()));
            }
        }
        out.sort();
        out
    }

    /// The optional arguments of every routed tool, taken from the ROUTER'S OWN
    /// SCHEMA — the same bytes an agent is shown.
    fn optional_arguments() -> Vec<(String, String)> {
        let mut out = Vec::new();
        for tool in agent_facing_tools() {
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
    /// Round 999 put the rule in the field descriptions and asked, of each
    /// property, whether its NAME ended in `_path`. THE POPULATION WAS A NAMING
    /// CONVENTION, and `against` is a path that is not called one: it sat in
    /// `DisclosureLeakArgs` beside an `order_path` that obeyed the rule, was
    /// never resolved against anything, and this gate counted 22 arguments and
    /// called them all of them. What it could not see, an agent could — every
    /// relative `against` an agent sent landed on the host's working directory,
    /// and because a missing store loads as an EMPTY one, both gates answered
    /// with the shape of a pass.
    ///
    /// So the population is now the TYPE's. `AgentPath` marks itself in the
    /// schema, the marker travels with the field wherever it is declared and
    /// whatever it is called, and the rule is STAMPED IN by
    /// [`agent_facing_tools`] rather than re-typed per field. What is left to
    /// assert is that the stamping reaches every marked property of the surface
    /// an agent actually reads — which is why this walks the same function the
    /// server answers `tools/list` from, and not the raw router.
    #[test]
    fn every_path_argument_says_what_it_resolves_against() {
        let mut checked = 0usize;
        let mut silent = Vec::new();
        let mut by_name = 0usize;
        for tool in agent_facing_tools() {
            let Some(props) = tool
                .input_schema
                .get("properties")
                .and_then(|p| p.as_object())
            else {
                continue;
            };
            for (key, schema) in props {
                if key.ends_with("_path") {
                    by_name += 1;
                }
                if schema.get("format").and_then(|f| f.as_str()) != Some(AGENT_PATH_FORMAT) {
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
            "no argument is typed `AgentPath`, so this gate reads nothing"
        );
        assert!(
            silent.is_empty(),
            "{} of {checked} path argument(s) do not tell an agent what they are \
             relative to. Since Round 1002 that is the workspace root, which is \
             the only base an MCP caller can see: {silent:?}",
            silent.len()
        );
        // THE TWO POPULATIONS, PRINTED SIDE BY SIDE EVERY RUN (Round 854). The
        // gap is the measurement: it is how many paths the old name-matched
        // population could never have reached, and a future path argument that
        // is not named `*_path` widens it rather than hiding in it.
        println!(
            "agent-facing paths: {checked} typed `AgentPath`, of which {by_name} are \
             also named `*_path` — {} would have been invisible to a gate keyed on the name",
            checked.saturating_sub(by_name)
        );
        assert!(
            checked >= by_name,
            "{by_name} argument(s) are named `*_path` but only {checked} are TYPED as one, \
             so a path is being resolved by hand somewhere this gate cannot see it"
        );
    }

    /// The polarity an argument's TYPE declares, or `None` if it declares none.
    fn declared_polarity(tool: &str, arg: &str) -> Option<&'static str> {
        let found = agent_facing_tools().into_iter().find(|t| t.name == tool)?;
        let marked = found
            .input_schema
            .get("properties")
            .and_then(|p| p.as_object())
            .and_then(|p| p.get(arg))
            .and_then(|s| s.get("format"))
            .and_then(|f| f.as_str())?;
        // THE POLARITY MARKERS, AND NOT EVERY MARKER. `AgentPath` is a marker
        // about resolution rather than about existence, so it is deliberately
        // not one of these — and listing them here rather than by exclusion is
        // what stops a marker added later for a third purpose from silently
        // acquiring an oracle written for this one.
        [EXISTING_REF_FORMAT, SCANNED_REF_FORMAT, FRESH_ID_FORMAT]
            .into_iter()
            .find(|f| *f == marked)
    }

    /// The closed set a PROPERTY declares, in either shape the schema writes
    /// one — and reading both is not a convenience.
    ///
    /// THERE ARE THREE SHAPES AND ASSUMING TWO REPORTS THE SURFACE AS OPEN
    /// WHERE IT IS MOST CAREFULLY DESCRIBED.
    ///
    /// A unit-variant enum whose variants carry no doc comment lands as a flat
    /// `enum: [...]`. One where EVERY variant is documented lands as
    /// `oneOf: [{const, description}, ...]`, which is the better shape to hand
    /// an agent because each member arrives with its meaning. And the mixed
    /// case — some variants documented, some not — lands as a `oneOf` whose
    /// branches are of BOTH kinds, the undocumented ones grouped into a single
    /// `enum` branch. That third shape is what `DecisionStatus` takes, and a
    /// reader that knew only the first two called it `composed` and concluded
    /// there was no closed set at all.
    ///
    /// So a branch contributes its `const` or its `enum`, and EVERY branch must
    /// contribute — a `oneOf` with a branch that is neither is a union of
    /// shapes, not a closed value set, and calling it one would be a false
    /// claim about what an agent may send.
    fn closed_set_members(property: &serde_json::Value) -> Option<Vec<String>> {
        fn tags_of(node: &serde_json::Value) -> Option<Vec<String>> {
            if let Some(c) = node.get("const").and_then(|c| c.as_str()) {
                return Some(vec![c.to_string()]);
            }
            let listed = node.get("enum")?.as_array()?;
            let tags: Vec<String> = listed
                .iter()
                .filter_map(|m| m.as_str().map(str::to_string))
                .collect();
            (tags.len() == listed.len() && !tags.is_empty()).then_some(tags)
        }
        if let Some(tags) = tags_of(property) {
            return Some(tags);
        }
        let branches = property.get("oneOf").and_then(|o| o.as_array())?;
        let mut tags: Vec<String> = Vec::new();
        for branch in branches {
            tags.extend(tags_of(branch)?);
        }
        (!tags.is_empty()).then_some(tags)
    }

    /// The property an agent reads for one argument, not a `$ref` beside it:
    /// the vocabulary enums are `schemars(inline)` for the reason `AgentPath`
    /// is (Round 1014) — a set behind a reference is one the client and the
    /// gate must each follow, and a set the client does not follow is a set the
    /// agent still has to guess.
    fn agent_property(tool: &str, arg: &str) -> Option<serde_json::Value> {
        agent_facing_tools()
            .into_iter()
            .find(|t| t.name == tool)?
            .input_schema
            .get("properties")
            .and_then(|p| p.as_object())
            .and_then(|p| p.get(arg))
            .cloned()
    }

    /// The closed set an argument's schema declares, or `None` if its value
    /// space is open.
    fn declared_closed_set(tool: &str, arg: &str) -> Option<Vec<String>> {
        closed_set_members(&agent_property(tool, arg)?)
    }

    /// Every closed vocabulary this workspace declares, with the members it
    /// holds — the table the refusal check below searches.
    ///
    /// Hand-written and EXECUTED, on the terms Round 1022 set for a table: each
    /// row is looked for in every refusal the sweep collects, so a row naming a
    /// vocabulary that no longer exists stops compiling and a row whose members
    /// moved searches the new ones. What it cannot do is notice a vocabulary
    /// declared somewhere and never registered here — the same bridge limit the
    /// `*_id` name gate has, and the reason the type is the primary mechanism
    /// and this is only the tripwire.
    fn registered_vocabularies() -> Vec<(&'static str, Vec<String>)> {
        fn tags<T: Copy>(all: &'static [T], f: fn(T) -> &'static str) -> Vec<String> {
            all.iter().map(|v| f(*v).to_string()).collect()
        }
        vec![
            (
                "BindingKind",
                tags(atomic::BindingKind::ALL, atomic::BindingKind::as_str),
            ),
            (
                "CoverageExpectation",
                tags(
                    atomic::CoverageExpectation::ALL,
                    atomic::CoverageExpectation::as_str,
                ),
            ),
            (
                "VerificationExpectation",
                tags(
                    atomic::VerificationExpectation::ALL,
                    atomic::VerificationExpectation::as_str,
                ),
            ),
            (
                "DecisionStatus",
                tags(
                    mnemosyne_core::DecisionStatus::ALL,
                    mnemosyne_core::DecisionStatus::as_str,
                ),
            ),
            (
                "DisclosureMode",
                tags(
                    mnemosyne_core::DisclosureMode::ALL,
                    mnemosyne_core::DisclosureMode::as_str,
                ),
            ),
            (
                "PayoffExpectation",
                tags(
                    mnemosyne_core::PayoffExpectation::ALL,
                    mnemosyne_core::PayoffExpectation::as_str,
                ),
            ),
            (
                "PredicateObjectKind",
                tags(
                    mnemosyne_core::PredicateObjectKind::ALL,
                    mnemosyne_core::PredicateObjectKind::as_str,
                ),
            ),
            (
                "InventoryStatus",
                tags(
                    mnemosyne_core::InventoryStatus::ALL,
                    mnemosyne_core::InventoryStatus::as_str,
                ),
            ),
            (
                "ConfirmerKind",
                tags(atomic::ConfirmerKind::ALL, atomic::ConfirmerKind::as_str),
            ),
            (
                "ConfirmMethod",
                tags(atomic::ConfirmMethod::ALL, atomic::ConfirmMethod::as_str),
            ),
            (
                "Verdict",
                tags(atomic::Verdict::ALL, atomic::Verdict::as_str),
            ),
        ]
    }

    /// Tool sets that are DELIBERATELY shaped by one argument struct, with the
    /// claim that makes the sharing safe: every tool named here means the SAME
    /// thing by each argument, so one schema describing all of them is honest.
    ///
    /// The table exists because the round before this one found the shape it is
    /// meant to stop, and found it by accident: `set_section_alternatives` took
    /// `SetSectionBulletsArgs` alongside six setters of free prose, while its own
    /// handler split each bullet on a separator and refused what would not split.
    /// The schema said `string` for the free case and the constrained one alike,
    /// because the struct they share can only say one thing. Nothing was watching
    /// for that, and nothing would have been.
    ///
    /// Each row is EXECUTED on the Round 1022 terms: the groups are derived from
    /// the properties the ROUTER exposes, so a row whose tools stopped agreeing —
    /// or a group that appears without a row — reddens on the round that does it.
    const SHARED_ARGS: &[(&str, &[&str])] = &[
        (
            "a section id and an ordered list of free-prose bullets — no member \
             constrains a bullet beyond the T3 length default that applies to \
             all of them",
            &[
                "set_section_rationale",
                "set_section_inputs",
                "set_section_outputs",
            ],
        ),
        (
            "an entry id and an ordered list of free-prose bullets on the \
             publishable half — the four fields differ in what they are ABOUT \
             and not in what a bullet may be",
            &[
                "set_changelog_publishable_carry_forward",
                "set_changelog_publishable_changes",
                "set_changelog_publishable_impact_refs",
                "set_changelog_publishable_verification",
            ],
        ),
        (
            "the same world-line filters over the same authored world — five \
             READ verbs shaped by five separate structs that an agent cannot \
             tell apart, which is the case this gate catches beyond literal \
             struct sharing",
            &[
                "report_edge_candidates",
                "report_fork_tree",
                "report_irony_intervals",
                "report_payoff_coverage",
                "report_payoff_substantiation",
            ],
        ),
    ];

    /// EVERY TOOL SET THAT SHARES AN ARGUMENT SHAPE IS DECLARED.
    ///
    /// Two tools whose property maps are identical are shaped by one struct, or
    /// are structurally indistinguishable to an agent — which for this question
    /// is the same thing, since the schema is all it reads. Sharing is a claim
    /// that the tools MEAN the same by each argument, and the claim was never
    /// written down anywhere, so the one place it was false was found only when
    /// a different gate walked past it.
    #[test]
    fn every_shared_argument_shape_is_declared() {
        let mut groups: std::collections::BTreeMap<String, Vec<String>> =
            std::collections::BTreeMap::new();
        for tool in agent_facing_tools() {
            let Some(props) = tool.input_schema.get("properties") else {
                continue;
            };
            groups
                .entry(props.to_string())
                .or_default()
                .push(tool.name.to_string());
        }
        let shared: Vec<Vec<String>> = groups
            .into_values()
            .filter(|tools| tools.len() > 1)
            .map(|mut tools| {
                tools.sort();
                tools
            })
            .collect();
        let declared: BTreeSet<Vec<String>> = SHARED_ARGS
            .iter()
            .map(|(_, tools)| {
                let mut t: Vec<String> = tools.iter().map(|s| s.to_string()).collect();
                t.sort();
                t
            })
            .collect();
        let found: BTreeSet<Vec<String>> = shared.iter().cloned().collect();
        println!(
            "agent-facing tools sharing an argument shape: {} group(s), {} declared",
            found.len(),
            declared.len()
        );
        assert_eq!(
            found, declared,
            "the tools that share an argument shape are not the ones declared to. \
             A group the router shows and the table does not is a set of tools \
             whose schema cannot distinguish them — which is only safe if they \
             MEAN the same by every argument, and that is the claim the table \
             makes and this compares"
        );
    }

    /// NO AGENT-FACING PROPERTY HIDES ITS SHAPE BEHIND A `$ref`.
    ///
    /// Round 1014 stated the rule for a closed set and Round 1027 applied it to
    /// ten of them: a set behind a reference is one the client and the gate must
    /// each follow, and a set the client does not follow is a set the agent
    /// still has to guess. The rule was never put to the WHOLE surface, and the
    /// round that asked found it the expensive way — `import_sections.sections`
    /// was excepted from the probe sweep as "a shape the schema describes no
    /// value for", and the diagnostic that finally printed what it MET said
    /// `{"items": {"$ref": "#/$defs/SectionImport"}}`. The probe could not build
    /// an element because the element's properties were not there to read; an
    /// agent reading the same property is in exactly that position.
    ///
    /// The check is on the property AND its `items`, because an array is the
    /// shape where the reference hides one level down — which is where it hid.
    #[test]
    fn no_agent_facing_property_hides_its_shape_behind_a_ref() {
        let mut hidden: Vec<String> = Vec::new();
        let mut properties = 0usize;
        for tool in agent_facing_tools() {
            let Some(props) = tool
                .input_schema
                .get("properties")
                .and_then(|p| p.as_object())
            else {
                continue;
            };
            for (key, schema) in props {
                properties += 1;
                let referenced = |node: Option<&serde_json::Value>| {
                    node.and_then(|n| n.get("$ref"))
                        .and_then(|r| r.as_str())
                        .map(str::to_string)
                };
                if let Some(target) =
                    referenced(Some(schema)).or_else(|| referenced(schema.get("items")))
                {
                    hidden.push(format!("{}.{key} -> {target}", tool.name));
                }
            }
        }
        println!(
            "agent-facing properties: {properties} read, {} behind a `$ref`",
            hidden.len()
        );
        assert!(
            hidden.is_empty(),
            "{} agent-facing propert(ies) name a definition instead of showing \
             their shape, so a client that does not follow references shows the \
             agent nothing: {}",
            hidden.len(),
            hidden.join(" | ")
        );
    }

    /// Whether `text` names EVERY member of a set, each as a word.
    ///
    /// As a WORD, because a member is a tag and not a substring: `state` sits
    /// inside `status` and `statement`, and a check that missed that would
    /// report a vocabulary wherever its shortest member happened to appear.
    fn names_every_member(text: &str, members: &[String]) -> bool {
        fn word(text: &str, needle: &str) -> bool {
            let edge = |c: Option<char>| c.is_none_or(|c| !c.is_ascii_alphanumeric() && c != '_');
            text.match_indices(needle).any(|(at, _)| {
                edge(text[..at].chars().next_back())
                    && edge(text[at + needle.len()..].chars().next())
            })
        }
        members.len() > 1 && members.iter().all(|m| word(text, m))
    }

    /// EVERY IDENTIFIER AN AGENT SENDS SAYS WHETHER IT IS LOOKED UP.
    ///
    /// Round 1025 shipped the argument surface with no silent set on either half
    /// and one carry: the probe cannot tell a correct refusal from an empty
    /// answer where a refusal was intended, because it has no way to know which
    /// polarity an argument was meant to have. THAT CARRY WAS MEASURED BEFORE IT
    /// WAS PAID. `query_section` was made to answer `{}` for a section that does
    /// not exist instead of refusing, and the whole workspace stayed green —
    /// 1727 tests, 135 binaries, nothing anywhere reading the difference. An
    /// agent asking about a section that is not there would have been told,
    /// successfully, that it is empty.
    ///
    /// The two markers are mirrors, and each one buys an oracle the sweep can
    /// run rather than a sentence a later reader has to trust:
    ///
    /// - `ExistingRef` — a value naming NOTHING must be refused, and the refusal
    ///   must name the value. This is the class where silence is fatal, because
    ///   "not found" has an empty-shaped answer that looks exactly like a real
    ///   one about a real thing.
    /// - `FreshId` — the same call made TWICE must leave ONE record. Round 1022
    ///   measured that a duplicate `add_*` is a no-op when the content is
    ///   identical and a refusal when it diverges, and recorded that both rules
    ///   were hand-written prose at each site with nothing enforcing either.
    ///
    /// THE POPULATION IS THE TYPE'S, and the bridge below is why both markers
    /// exist rather than only the one that closes the defect. A gate keyed on
    /// the NAME `*_id` is what Round 1014 already showed to be a convention
    /// rather than a population — `against` was a path not called one. Here the
    /// same gap runs the other way too: `coord`, `telling`, `world`,
    /// `truth_frame`, `at`, `frame`, `unit`, `parameter`, `condition` and `file`
    /// are all looked up and none of them is named `*_id`. So the name is used
    /// only as a BRIDGE — every id-shaped name must carry one of the two marks,
    /// which is a set and not a count, and the surplus of typed-but-not-named
    /// arguments is printed as the measure of what a name-keyed gate could never
    /// have reached.
    #[test]
    fn every_identifier_argument_says_whether_it_is_looked_up() {
        let mut typed: BTreeSet<String> = BTreeSet::new();
        let mut silent: Vec<String> = Vec::new();
        let mut untyped_by_name: Vec<String> = Vec::new();
        let mut by_name = 0usize;
        for tool in agent_facing_tools() {
            let Some(props) = tool
                .input_schema
                .get("properties")
                .and_then(|p| p.as_object())
            else {
                continue;
            };
            for (key, schema) in props {
                let named = key == "id" || key.ends_with("_id");
                let polarity = declared_polarity(&tool.name, key);
                if named {
                    by_name += 1;
                }
                let Some(polarity) = polarity else {
                    if named {
                        untyped_by_name.push(format!("{}.{key}", tool.name));
                    }
                    continue;
                };
                typed.insert(format!("{}.{key}", tool.name));
                // THE RULE REACHED THE PROPERTY AN AGENT READS. The stamping is
                // what makes a new marked argument born saying what it is, so a
                // marker whose rule did not land is a marker that says nothing.
                let rule = MARKER_RULES
                    .iter()
                    .find(|(f, _)| *f == polarity)
                    .map(|(_, r)| *r)
                    .expect("a marked format must have a rule");
                let described = schema
                    .get("description")
                    .and_then(|d| d.as_str())
                    .unwrap_or_default();
                if !described.contains(rule) {
                    silent.push(format!("{}.{key}", tool.name));
                }
            }
        }
        assert!(
            !typed.is_empty(),
            "no argument declares a polarity, so this gate reads nothing"
        );
        assert!(
            silent.is_empty(),
            "{} marked argument(s) do not carry their marker's rule in the \
             description an agent reads, so the stamping in `agent_facing_tools` \
             did not reach them: {silent:?}",
            silent.len()
        );
        println!(
            "agent-facing identifiers: {} typed, of which {by_name} argument(s) are \
             named `id`/`*_id` — {} are looked up or minted under a name no \
             `*_id` gate would have reached",
            typed.len(),
            typed.len().saturating_sub(by_name)
        );
        assert!(
            untyped_by_name.is_empty(),
            "{} argument(s) are named `id`/`*_id` and declare no polarity, so \
             whether the store looks them up is stated nowhere the agent or this \
             sweep can read: {untyped_by_name:?}",
            untyped_by_name.len()
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
        account_for(
            "optional",
            optional_arguments(),
            UNEXERCISED,
            &BTreeSet::new(),
        );
    }

    /// THE OTHER HALF, ON THE SAME TERMS. Round 1014's defect lived here and the
    /// optional accounting could only print that it was excluded.
    ///
    /// A REQUIRED ARGUMENT CANNOT BE OMITTED, SO IT CANNOT BE IGNORED THE WAY
    /// ROUND 981's WAS — the call would not parse. What it CAN be is READ WRONG,
    /// and the failure looks like success just as squarely: `against` reached
    /// `Path::new` unresolved, missed the file, and came back `off_path: []`,
    /// `leaks: []` from a gate that had opened nothing.
    ///
    /// Rounds 1017 and 1019 both carried the guess that a differential could not
    /// catch that, "because both arms fail". IT IS FALSE, and the reason is the
    /// same `AtomicStore::load` behaviour that made the bug invisible: a missing
    /// path loaded as an EMPTY store, so both arms SUCCEEDED and answered
    /// identically. Two values that must produce two answers is exactly what
    /// separates them, which is why the population is being named rather than
    /// argued about (`validate_render_fidelity_against_reaches_the_answer` is
    /// the first case, and the injection that reconstructs Round 1014's defect
    /// turns it red).
    #[test]
    fn every_required_tool_argument_is_declared_exercised_or_not() {
        account_for(
            "required",
            required_arguments(),
            UNEXERCISED_REQUIRED,
            &probed_pairs(),
        );
    }

    /// One accounting, run once per half of the surface.
    ///
    /// `EXERCISED` is generated over BOTH halves, because a case proves a
    /// handler reads an argument whether the schema calls that argument required
    /// or optional. It is the ROUTER that decides which half a pair belongs to,
    /// so a pair is checked against the half it actually lives in and is ignored
    /// by the other — a second generated const would need the macro to know a
    /// fact about the schema rather than about the case.
    fn account_for(
        half: &str,
        population: Vec<(String, String)>,
        silent: &[(&str, &str)],
        probed: &BTreeSet<(String, String)>,
    ) {
        assert!(
            !population.is_empty(),
            "the router exposes no {half} argument at all, so this gate is \
             reading nothing and its silence proves nothing"
        );
        assert!(
            !EXERCISED.is_empty(),
            "no pair is claimed exercised, so the accounting has no floor"
        );

        let live: BTreeSet<(&str, &str)> = population
            .iter()
            .map(|(t, a)| (t.as_str(), a.as_str()))
            .collect();
        let claims: Vec<(&str, &str)> = EXERCISED
            .iter()
            .chain(EXERCISED_BESPOKE)
            .chain(silent)
            .copied()
            .collect();

        let mine: Vec<(&str, &str)> = claims
            .iter()
            .copied()
            .filter(|p| live.contains(p))
            .collect();
        let deduped: BTreeSet<(&str, &str)> = mine.iter().copied().collect();
        assert_eq!(
            deduped.len(),
            mine.len(),
            "a pair is declared twice, so one of the two lists is lying about it"
        );

        // A PAIR THE ROUTER DOES NOT EXPOSE AT ALL is a claim about a surface
        // that no longer exists. Belonging to the OTHER half is not that — the
        // other gate owns it — so the exclusion is by half, not by list.
        let other = other_half(half);
        let elsewhere: BTreeSet<(&str, &str)> = other
            .iter()
            .map(|(t, a)| (t.as_str(), a.as_str()))
            .collect();
        let unroutable: Vec<(&str, &str)> = silent
            .iter()
            .copied()
            .filter(|p| !live.contains(p))
            .collect();
        assert!(
            unroutable.is_empty(),
            "{} pair(s) named in the {half} silent set are not {half} arguments \
             of any routed tool, so this accounting describes a surface that no \
             longer exists: {unroutable:?}",
            unroutable.len()
        );
        let nowhere: Vec<(&str, &str)> = claims
            .iter()
            .copied()
            .filter(|p| !live.contains(p) && !elsewhere.contains(p))
            .collect();
        assert!(
            nowhere.is_empty(),
            "{} declared pair(s) are arguments of no routed tool at all: \
             {nowhere:?}",
            nowhere.len()
        );

        // A PAIR THE PROBE SWEEP REACHES IS ACCOUNTED FOR, ON WEAKER TERMS. It
        // is a separate set rather than another list because nobody types it:
        // it is derived from the tools that declare a valid call, so it cannot
        // claim an argument the sweep did not actually probe.
        let swept: BTreeSet<(&str, &str)> = probed
            .iter()
            .map(|(t, a)| (t.as_str(), a.as_str()))
            .filter(|p| live.contains(p))
            .collect();
        let both: Vec<&(&str, &str)> = swept.iter().filter(|p| silent.contains(p)).collect();
        assert!(
            both.is_empty(),
            "{} pair(s) are named unexercised AND reached by the probe sweep. \
             Declaring a valid call for a tool covers every required argument it \
             has, so the admission must go on the round that declares the call: \
             {both:?}",
            both.len()
        );
        let unaccounted: Vec<&(&str, &str)> = live
            .iter()
            .filter(|p| !deduped.contains(*p) && !swept.contains(*p))
            .collect();
        assert!(
            unaccounted.is_empty(),
            "{} of {} agent-facing {half} argument(s) are in neither list. An \
             argument that nothing proves the handler reads must be named as \
             such, not left to be discovered by an agent whose call quietly did \
             less than it asked for: {unaccounted:?}",
            unaccounted.len(),
            live.len()
        );

        // BOTH RATIOS ARE PRINTED EVERY RUN, because a check that reports only
        // violations reads exactly like one that has nothing to report (Round
        // 854) — and because the optional half read as coverage of the whole
        // surface for as long as the required half had no gate to print.
        // THE PROBED COUNT IS PRINTED APART FROM THE DIFFERENTIAL COUNT AND
        // NEVER ADDED TO IT. A probe proves the handler validated the value or
        // left it observable; a differential proves the value the agent sent
        // decided the answer. Summing them would let the weaker evidence be read
        // as the stronger, which is the reading Round 987 caught when four pairs
        // were moved out of the silent set with no test behind them at all.
        // AND AN ARGUMENT DECLARING A CLOSED SET IS ON THE DIFFERENTIAL SIDE,
        // derived from the router rather than declared anywhere. The sweep sends
        // it a second MEMBER and asserts the answer moved, which is the same
        // evidence a hand-written differential gives; counting it as a floor
        // would understate what every run already proves. The claim is executed:
        // if such an argument does not move the answer, its tool's probe is red.
        let closed: BTreeSet<(&str, &str)> = swept
            .iter()
            .copied()
            .filter(|(t, a)| declared_closed_set(t, a).is_some())
            .collect();
        let differential = deduped.len() - silent.iter().filter(|p| live.contains(*p)).count()
            + closed.iter().filter(|p| !deduped.contains(*p)).count();
        println!(
            "MCP {half} arguments, of {} on the router: {differential} proved by \
             differential ({} of them by a second member of a closed set the \
             schema declares), {} probed only (a floor — the value was validated \
             or observable, not shown to decide the answer), {} unexercised",
            live.len(),
            closed.iter().filter(|p| !deduped.contains(*p)).count(),
            swept
                .iter()
                .filter(|p| !deduped.contains(*p) && !closed.contains(*p))
                .count(),
            silent.len(),
        );
        let mut by_type: std::collections::BTreeMap<String, usize> = Default::default();
        for (tool, arg) in silent {
            let ty = declared_type(tool, arg);
            *by_type.entry(ty.clone()).or_default() += 1;
            println!("  unexercised ({half}): {tool}.{arg} : {ty}");
        }
        if !by_type.is_empty() {
            println!(
                "  by declared type: {}",
                by_type
                    .iter()
                    .map(|(t, n)| format!("{t} {n}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }

    /// The JSON type the ROUTER'S OWN SCHEMA declares for one argument, printed
    /// beside every unexercised row.
    ///
    /// Round 1020 shipped the required population with a carry saying it was not
    /// known how many of the 171 could be given two values without new world
    /// content. That is a question about the schema, so the schema answers it
    /// and the program prints the answer instead of a later round guessing: a
    /// `boolean` has two values by construction, and a `string` that names a
    /// registered thing needs two of them to exist first — the `given` line the
    /// exercised cases already carry. Derived rather than tabulated, so it
    /// cannot drift from what an agent is shown.
    fn declared_type(tool: &str, arg: &str) -> String {
        let Some(found) = agent_facing_tools().into_iter().find(|t| t.name == tool) else {
            return "no such tool".to_string();
        };
        let Some(property) = found
            .input_schema
            .get("properties")
            .and_then(|p| p.as_object())
            .and_then(|p| p.get(arg))
        else {
            return "no such property".to_string();
        };
        // A PATH IS CALLED OUT WITHIN ITS TYPE, because that is the class Round
        // 1014's defect belonged to: `against` is a required string that is a
        // path, and being a path is exactly what the two gates keyed on the NAME
        // `*_path` could not see. Counting them says how many more places that
        // same defect could still live.
        if property.get("format").and_then(|f| f.as_str()) == Some(AGENT_PATH_FORMAT) {
            return format!("string({AGENT_PATH_FORMAT})");
        }
        match property.get("type") {
            Some(serde_json::Value::String(s)) => s.clone(),
            // A nullable or union type is written as a list, and an argument
            // whose type comes from a `$ref` or a composition has no `type` key
            // at all; both are reported as they are rather than flattened, since
            // the point is to say what the agent is shown.
            Some(serde_json::Value::Array(items)) => items
                .iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join("|"),
            Some(other) => other.to_string(),
            None => "composed".to_string(),
        }
    }

    /// THE VALUE A REQUIRED ARGUMENT IS PROBED WITH. It names nothing in any
    /// registry, is not a path any fixture writes, and appears in no world — so
    /// a handler that READS the argument has to either refuse it or leave it
    /// somewhere observable, and a handler that ignores it does neither.
    const NOT_REGISTERED: &str = "mn-probe-names-nothing";

    /// The number a required integer is probed with. A fixed value rather than
    /// `base + 1`, because it doubles as the needle counted in the observation.
    const NOT_A_FIXTURE_NUMBER: i64 = 909091;

    /// What to send in place of ONE required argument, DERIVED FROM THE ROUTER'S
    /// OWN SCHEMA rather than written out per argument.
    ///
    /// This is the whole reason the required half costs one declaration per TOOL
    /// instead of one case per ARGUMENT: the schema already says what shape each
    /// argument takes, so the probe value and the needle to count follow from it.
    /// Returns `None` when the schema does not describe a value this can build —
    /// an argument whose type is a `$ref` to a closed enum, or an array of
    /// objects — and those arguments must be named in the tool's `except` list,
    /// where they stay visible in the unexercised population.
    ///
    /// A CLOSED VOCABULARY IS PROBED WITH ANOTHER MEMBER OF IT, which is a
    /// DIFFERENTIAL and not a floor.
    ///
    /// Until this round the ten closed sets on this surface were bare `string`
    /// on the wire and enforced by the handler, so the sentinel parsed, reached
    /// the handler and was refused by name — evidence about the handler, and
    /// the agent learned the set only by guessing wrong. Now the wire takes the
    /// enum: serde refuses a non-member before the handler is reached, which is
    /// a STRUCTURAL proof that no other value gets through, and the question
    /// left over is the one a sentinel could never answer — whether the member
    /// the agent chose decides the answer. So the probe value is a SECOND
    /// MEMBER, and the same derived observation that judges every other
    /// argument judges this one, as a differential.
    fn probe_value(
        tool: &str,
        arg: &str,
        base: &serde_json::Value,
    ) -> Option<(serde_json::Value, String)> {
        let found = agent_facing_tools().into_iter().find(|t| t.name == tool)?;
        let property = found
            .input_schema
            .get("properties")
            .and_then(|p| p.as_object())
            .and_then(|p| p.get(arg))?;
        // THE MEMBER IS PICKED FROM THE SCHEMA, so a variant added to the enum
        // is probed by the same declaration. Any member other than the one the
        // declared call already sends will do; the caller asserts they differ.
        if let Some(members) = closed_set_members(property) {
            let other = members
                .into_iter()
                .find(|m| base.as_str() != Some(m.as_str()))?;
            return Some((serde_json::Value::from(other.clone()), other));
        }
        // AN OPTIONAL ARGUMENT'S TYPE IS WRITTEN AS A UNION WITH `null`, and the
        // probe is about the value the agent sends rather than about the option
        // of omitting it — which the other half of the accounting already
        // covers with a present-vs-absent differential. So the null arm is set
        // aside and what remains is the type probed.
        let declared = match property.get("type") {
            Some(serde_json::Value::String(s)) => s.as_str(),
            Some(serde_json::Value::Array(items)) => items
                .iter()
                .filter_map(|v| v.as_str())
                .find(|t| *t != "null")?,
            _ => return None,
        };
        match declared {
            "string" => Some((
                serde_json::json!(NOT_REGISTERED),
                NOT_REGISTERED.to_string(),
            )),
            "integer" | "number" => Some((
                serde_json::json!(NOT_A_FIXTURE_NUMBER),
                NOT_A_FIXTURE_NUMBER.to_string(),
            )),
            "array"
                if property
                    .get("items")
                    .and_then(|i| i.get("type"))
                    .and_then(|t| t.as_str())
                    == Some("string") =>
            {
                Some((
                    serde_json::json!([NOT_REGISTERED]),
                    NOT_REGISTERED.to_string(),
                ))
            }
            // AN ARRAY OF OBJECTS IS STILL A SHAPE THE SCHEMA DESCRIBES, so it
            // is probed with ONE element whose every required property is the
            // sentinel. Excepting these instead was the cheaper move and it
            // costs the evidence: an argument nothing concludes about is one
            // whose handler could stop reading it and no run would say so. The
            // element is built from the ITEM SCHEMA, so a property added to the
            // element type is sent by the same declaration — and where a
            // required property is not a string, this returns `None` rather than
            // guessing, and the exception must then be earned as usual.
            "array" if property.get("items").is_some() => {
                let items = property.get("items")?;
                if items.get("type").and_then(|t| t.as_str()) != Some("object") {
                    return None;
                }
                let required = items.get("required")?.as_array()?;
                let props = items.get("properties")?.as_object()?;
                let mut element = serde_json::Map::new();
                for key in required {
                    let key = key.as_str()?;
                    let declared = props.get(key)?.get("type")?.as_str()?;
                    if declared != "string" {
                        return None;
                    }
                    element.insert(key.to_string(), serde_json::json!(NOT_REGISTERED));
                }
                (!element.is_empty()).then(|| {
                    (
                        serde_json::Value::Array(vec![serde_json::Value::Object(element)]),
                        NOT_REGISTERED.to_string(),
                    )
                })
            }
            _ => None,
        }
    }

    /// The (tool, required argument) pairs the probe sweep reaches, DERIVED from
    /// the tools that declare a valid call and the router's own required list.
    ///
    /// Nothing is transcribed: declaring one call for a tool covers every
    /// required argument that tool has, and a required argument ADDED to a
    /// declared tool later is covered by the same declaration on the round that
    /// adds it. That is the property a per-argument table cannot have — Round
    /// 986 opened the optional half with 122 rows that each had to be written,
    /// and every new optional argument since has needed a new one.
    fn probed_pairs() -> BTreeSet<(String, String)> {
        let required = required_arguments();
        let mut out = BTreeSet::new();
        for (tool, except) in PROBED {
            for (t, a) in &required {
                if t == tool && !except.contains(&a.as_str()) {
                    out.insert((t.clone(), a.clone()));
                }
            }
        }
        out
    }

    /// The population the OTHER gate owns — so each gate can tell "this pair
    /// moved to the other half" from "this pair is gone".
    fn other_half(half: &str) -> Vec<(String, String)> {
        match half {
            "optional" => required_arguments(),
            "required" => optional_arguments(),
            other => panic!("`{other}` is not a half of the argument surface"),
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

    /// The first line of substance each observation has and the other does not
    /// (Round 1003).
    ///
    /// Line-by-line "first difference" is useless on a pretty-printed report,
    /// where the first disagreement is a brace; what a case author needs is the
    /// first line of CONTENT one side carries alone. Punctuation-only lines are
    /// skipped for that reason.
    ///
    /// A case author who is told only THAT the answer moved has to go and print
    /// both; being told WHAT moved is usually the whole diagnosis.
    fn first_difference(a: &str, b: &str) -> (String, String) {
        let substantive = |l: &&str| l.chars().any(|c| c.is_alphanumeric());
        let unique = |from: &str, other: &str| -> String {
            let theirs: std::collections::BTreeSet<&str> = other.lines().map(str::trim).collect();
            from.lines()
                .filter(substantive)
                .map(str::trim)
                .find(|l| !theirs.contains(l))
                .map_or_else(
                    || "<nothing this side carries alone>".to_string(),
                    |l| l.chars().take(160).collect(),
                )
        };
        (unique(a, b), unique(b, a))
    }

    /// THE VERDICT IS THE CONTRACT AN AGENT ACTS ON.
    ///
    /// Round 1001 gave `propose_verdict` its first test and closed by naming
    /// what that test does NOT cover: "nothing yet asserts the verdict itself —
    /// that a manifest which would violate a continuity rule comes back
    /// `rollback` with the rule named, and a clean one comes back `commit`".
    /// That is a test written in prose, which this session has now paid for
    /// four times.
    ///
    /// The whole point of this verb is that an agent runs it BEFORE writing for
    /// real and believes the answer. A wire that always said `commit` would
    /// pass Round 1001's test — it reads the right file either way.
    #[tokio::test]
    async fn propose_verdict_says_rollback_with_the_rule_and_commit_without() {
        let tmp = agent_workspace();
        let ws = tmp.path();
        // THE VERDICT IS SEVERITY-DEPENDENT AND THAT IS WORTH KNOWING: with no
        // `[continuity] severity`, this manifest comes back `commit` WITH the
        // violation listed — asserted at the end, because an agent that reads
        // only `verdict` on such a workspace would write a beat the map
        // forbids.
        std::fs::write(
            ws.join("mnemosyne.toml"),
            "[workspace]\n[continuity]\nseverity = \"reject\"\ninterval_severity = \"reject\"\n",
        )
        .expect("config");
        let server = MnemosyneServer::new(ws.to_path_buf()).expect("server");
        let sections: ImportSectionsArgs = serde_json::from_value(serde_json::json!({
            "sections": [
                {"section_id": "sc-01", "parent_doc": "spec", "title": "one"},
                {"section_id": "sc-02", "parent_doc": "spec", "title": "two"},
            ],
        }))
        .expect("sections parse");
        assert!(server.import_sections(Parameters(sections)).await.is_error != Some(true));

        // A world with a map: two places, no way between them.
        let world = serde_json::json!({
            "frames": [{"frame_id": "ground-truth"}],
            "entity_kinds": [{"kind_id": "place"}, {"kind_id": "character"}],
            "entities": [
                {"entity_id": "p-a", "kind": "place"},
                {"entity_id": "p-b", "kind": "place"},
                {"entity_id": "e-her", "kind": "character"},
                {"entity_id": "e-him", "kind": "character"},
            ],
            "predicates": [
                {"predicate_id": "adjacent", "object_kind": "entity",
                 "subject_kind": "place", "object_entity_kind": "place"},
                {"predicate_id": "at", "object_kind": "entity",
                 "subject_kind": "character", "object_entity_kind": "place"},
                {"predicate_id": "rose_on_day", "object_kind": "quantity",
                 "subject_kind": "character"},
                {"predicate_id": "fell_on_day", "object_kind": "quantity",
                 "subject_kind": "character"},
            ],
            "units": [{"unit_id": "day"}],
            "facts": [{
                "fact_id": "f-at-a", "frame": "ground-truth", "claim": "she is at a",
                "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["e-her", "p-a"],
                "typed": {"subject": "e-her", "predicate": "at",
                          "object": {"kind": "entity", "id": "p-a"}},
            }],
        });
        let base: atomic::FactsManifest = serde_json::from_value(world).expect("world parse");
        assert!(server.import_facts(Parameters(base)).await.is_error != Some(true));
        std::fs::write(
            ws.join("rules.json"),
            serde_json::json!({
                "schema": "narrative-rules/v1",
                "rules": [{"id": "moves-follow-the-map", "predicate": "at",
                           "class": "transition", "adjacency": "adjacent",
                           "undirected": true},
                          {"id": "falls-after-it-rises", "predicate": "fell_on_day",
                           "class": "interval", "right": "rose_on_day", "op": "ge",
                           "bound": {"const": 1}}],
            })
            .to_string(),
        )
        .expect("rules");
        std::fs::write(
            ws.join("order.json"),
            serde_json::json!({"schema": "canon-order/v1", "edges": [["sc-01", "sc-02"]]})
                .to_string(),
        )
        .expect("order");

        // A step to a place no way reaches — the map rule must refuse it.
        let stepping = serde_json::json!({
            "facts": [{
                "fact_id": "f-at-b", "frame": "ground-truth", "claim": "she is at b",
                "canon_from": "sc-02", "evidence": ["sc-02"], "entities": ["e-her", "p-b"],
                "supersedes_in_frame": "f-at-a",
                "typed": {"subject": "e-her", "predicate": "at",
                          "object": {"kind": "entity", "id": "p-b"}},
            }],
        });
        // A beat that is not a STEP at all: someone else, standing where she
        // already is, superseding nothing. The map has nothing to judge, so a
        // rollback here would mean the refusal above was about something other
        // than the rule.
        let staying = serde_json::json!({
            "facts": [{
                "fact_id": "f-at-him", "frame": "ground-truth", "claim": "he is at a too",
                "canon_from": "sc-02", "evidence": ["sc-02"], "entities": ["e-him", "p-a"],
                "typed": {"subject": "e-him", "predicate": "at",
                          "object": {"kind": "entity", "id": "p-a"}},
            }],
        });

        let verdict_on = |manifest: serde_json::Value, name: &str| {
            std::fs::write(ws.join(name), manifest.to_string()).expect("write candidate");
            serde_json::from_value::<ProposeVerdictArgs>(serde_json::json!({
                "manifest_path": name,
                "order_path": "order.json",
                "rules_path": "rules.json",
            }))
            .expect("args parse")
        };

        // THE TABLE COVERS BOTH OF THE VERB'S SOURCES (Round 1009). A verdict
        // has exactly two: a SHAPE rejection from applying the manifest, and
        // the CONTINUITY scan over the applied clone. The rows below are one
        // shape (an entity the registry never registered) and two continuity
        // families (a transition rule and an interval rule) — so the field is
        // exercised from every path that can reach it, which is a stronger
        // statement than counting rules.
        //
        // Round 1008's carry said the disclosure gates were "the fourth family
        // reaching the same field". They are not: `propose_verdict` runs the
        // shape invariants and `scan_continuity`, and nothing else. That carry
        // was a claim about the world and it was wrong.
        //
        // Each row asserts the NAME its refusal carries, since that is what an
        // agent repairs against, and not just the word `rollback`.
        // A third family: an interval rule whose bound the candidate breaks.
        let backwards = serde_json::json!({
            "facts": [
                {"fact_id": "f-rose", "frame": "ground-truth", "claim": "it rose on day 5",
                 "canon_from": "sc-02", "evidence": ["sc-02"], "entities": ["e-her"],
                 "typed": {"subject": "e-her", "predicate": "rose_on_day",
                           "object": {"kind": "quantity", "n": 5, "unit": "day"}}},
                {"fact_id": "f-fell", "frame": "ground-truth", "claim": "it fell on day 1",
                 "canon_from": "sc-02", "evidence": ["sc-02"], "entities": ["e-her"],
                 "typed": {"subject": "e-her", "predicate": "fell_on_day",
                           "object": {"kind": "quantity", "n": 1, "unit": "day"}}},
            ],
        });
        let unregistered = serde_json::json!({
            "facts": [{
                "fact_id": "f-ghost", "frame": "ground-truth", "claim": "someone unregistered",
                "canon_from": "sc-02", "evidence": ["sc-02"], "entities": ["e-nobody"],
            }],
        });
        let mut sources_seen: std::collections::BTreeSet<String> = Default::default();
        for (what, manifest, name, rule) in [
            (
                "a beat that walks where no way runs",
                stepping,
                "stepping.json",
                "moves-follow-the-map",
            ),
            (
                "a beat naming an entity the registry never registered",
                unregistered,
                "unregistered.json",
                "e-nobody",
            ),
            (
                "a pair of beats an interval rule orders the other way",
                backwards,
                "backwards.json",
                "falls-after-it-rises",
            ),
        ] {
            let refused = answer_text(
                &server
                    .propose_verdict(Parameters(verdict_on(manifest, name)))
                    .await,
            );
            assert!(
                refused.contains("rollback"),
                "{what} came back without `rollback`, so an agent acting on this \
                 verdict would write it for real: {refused}"
            );
            assert!(
                refused.contains(rule),
                "{what} refuses without naming `{rule}`, which is what an agent \
                 repairs against: {refused}"
            );
            // EVERY SOURCE THIS VERB CAN CARRY IS WALKED, and the walk is the
            // TYPE's (Round 1009), not a list in this test: `ViolationSource`
            // enumerates them, so a third one cannot appear without this
            // assertion going red. Round 1008 claimed a third by reading the
            // file and was wrong.
            let answer: serde_json::Value =
                serde_json::from_str(&refused).expect("the verdict is json");
            for v in answer
                .get("violations")
                .and_then(|v| v.as_array())
                .expect("violations")
            {
                let src = v.get("source").and_then(|s| s.as_str()).expect("source");
                sources_seen.insert(src.to_string());
            }
        }
        let expected: std::collections::BTreeSet<String> = ops::ViolationSource::ALL
            .iter()
            .map(|s| s.as_str().to_string())
            .collect();
        assert_eq!(
            sources_seen, expected,
            "the table does not reach every source a verdict can carry. The \
             type is what enumerates them, so a source added there is undone \
             work here rather than a sentence in a carry"
        );

        let accepted = answer_text(
            &server
                .propose_verdict(Parameters(verdict_on(staying, "staying.json")))
                .await,
        );
        assert!(
            accepted.contains("commit"),
            "a beat the map has no objection to did not come back `commit`, so \
             the refusal above says nothing about the rule: {accepted}"
        );
        assert!(
            !accepted.contains("rollback"),
            "the clean manifest came back with a rollback too: {accepted}"
        );

        // THE SHARP EDGE, ASSERTED RATHER THAN LEFT TO BE DISCOVERED: on a
        // workspace that declares no continuity severity, the SAME refused
        // manifest comes back `commit` with the violation listed beside it.
        // An agent that reads `verdict` alone there writes a beat the map
        // forbids, and nothing in the answer is untrue.
        std::fs::write(ws.join("mnemosyne.toml"), "[workspace]\n").expect("config");
        let unset = MnemosyneServer::new(ws.to_path_buf()).expect("server");
        // The SAME file, unrewritten: `verdict_on` writes the manifest, so
        // reusing it here would clobber the very candidate under test.
        let same_candidate: ProposeVerdictArgs = serde_json::from_value(serde_json::json!({
            "manifest_path": "stepping.json",
            "order_path": "order.json",
            "rules_path": "rules.json",
        }))
        .expect("args parse");
        let lenient = answer_text(&unset.propose_verdict(Parameters(same_candidate)).await);
        assert!(
            lenient.contains("\"verdict\": \"commit\"")
                && lenient.contains("rule_transition_invalid"),
            "with no declared severity the refused manifest was expected to come \
             back `commit` WITH its violation listed; if that has changed, this \
             comment and the tool's description both need re-reading: {lenient}"
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
            $( @ $world:ident )?
            $( {$file:literal = $contents:tt} )*
            $( (store $blind:literal = $( [$blind_call:ident($blind_args:ty) $blind_base:tt] )* ) )*
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
                        // A PATH ARGUMENT IS WRITTEN AS A PLAIN RELATIVE NAME,
                        // because Round 1002 made a relative path an agent
                        // sends resolve against the WORKSPACE IT WAS HANDED.
                        // Before that it resolved against the server process's
                        // working directory, which an agent can neither see nor
                        // choose, and these cases carried a `{ws}` substitution
                        // to spell the workspace root out. That helper is gone
                        // rather than kept: it existed only to work around the
                        // rule Round 1002 replaced, and a case still using it
                        // would be proving the old contract.
                        let args: $args = serde_json::from_value(json)
                            .expect("the agent-facing shape must parse this call");
                        let server =
                            MnemosyneServer::new(tmp.path().to_path_buf()).expect("server");
                        // THE NAMED WORLD, established in BOTH arms before
                        // anything this case adds — the difference the test
                        // asserts must be the argument, never the setup.
                        $( $world(&server, tmp.path()).await; )?
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
                        // A STORE FILE THE TOOL READS, BUILT BY A SECOND SERVER
                        // RATHER THAN WRITTEN AS A LITERAL. The two
                        // render-acceptance gates take `against` — a BLIND
                        // re-extracted prose store, whose whole point is that it
                        // was produced independently of the authored one. A JSON
                        // literal could stand in for it only by hand-copying the
                        // store schema into this file, which is the fixture this
                        // repository keeps deleting and which drifts silently the
                        // next time the schema moves. So the case names the CALLS
                        // that build it and a second server writes it, exactly as
                        // Round 1014's bespoke test does by hand.
                        $(
                            {
                                let blind_ws = agent_workspace();
                                let blind = MnemosyneServer::new(
                                    blind_ws.path().to_path_buf(),
                                )
                                .expect("the second server");
                                $(
                                    let seed: $blind_args = serde_json::from_value(
                                        serde_json::json!($blind_base),
                                    )
                                    .expect("the blind store call's shape must parse");
                                    let built = blind.$blind_call(Parameters(seed)).await;
                                    assert!(
                                        built.is_error != Some(true),
                                        "building the blind store `{}` failed at {}, \
                                         so the gate under test would be reading an \
                                         empty one: {:?}",
                                        $blind,
                                        stringify!($blind_call),
                                        built.content
                                    );
                                )*
                                std::fs::copy(
                                    blind_ws.path().join("docs/.atomic/workspace.atomic.json"),
                                    tmp.path().join($blind),
                                )
                                .unwrap_or_else(|e| panic!("seed {}: {e}", $blind));
                            }
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
                        assert!(
                            written[0] == written[1],
                            "`{}` occurs {before} time(s) in the {} {} produced \
                             WITHOUT `{}` and {after} with it — but the {} DID \
                             change, so the argument arrived and this needle is \
                             not what moved. THE DEFECT IS IN THIS CASE: name a \
                             needle that follows the value. WHAT ACTUALLY MOVED, \
                             first difference:\n  without: {}\n  with:    {}",
                            $needle,
                            stringify!($oracle),
                            stringify!($tool),
                            $field,
                            stringify!($oracle),
                            first_difference(&written[0], &written[1]).0,
                            first_difference(&written[0], &written[1]).1
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

    /// ONE VALID CALL PER TOOL, AND EVERY REQUIRED ARGUMENT OF IT IS PROBED.
    ///
    /// WHY THIS EXISTS RATHER THAN 163 MORE `exercised!` CASES. A required
    /// argument cannot be omitted, so the optional half's present-vs-absent
    /// differential has no arm without it; Round 1020 answered that with
    /// value-A-vs-value-B and wrote the first five by hand. Filling the rest that
    /// way is 163 cases, each needing a world, a second value and a needle found
    /// by trial — and each new required argument added afterwards needs another
    /// one. The population is 163 arguments over SEVENTY-SIX tools, so the unit
    /// of work is wrong: what a tool needs is ONE call that is valid, and what
    /// each of its arguments needs follows from the schema.
    ///
    /// WHAT ONE PROBE PROVES. The base call is made once and must SUCCEED — the
    /// floor, without which every probe is refused for the base's reasons and the
    /// sweep would report a handler reading arguments it never saw
    /// (`add_predicate` refused a probe of this shape while measuring, because
    /// the base named `object_kind: token` with no token vocabulary). Then each
    /// required argument in turn is replaced by a value that names NOTHING, and
    /// exactly one of two things must happen:
    ///
    /// - THE CALL IS REFUSED, and the refusal NAMES THE VALUE. The handler
    ///   looked the value up and rejected it, so it read the argument. Naming it
    ///   is what separates this from a refusal for an unrelated reason.
    /// - THE CALL IS ACCEPTED, and the value is OBSERVABLE. Which observation is
    ///   not declared but DERIVED (the Round 991 discipline): if the base call
    ///   wrote the store, the store is the oracle and the sentinel must occur
    ///   there more often than without it; if it wrote nothing, the tool is a
    ///   read and its answer is all there is.
    ///
    /// WHAT IT DOES NOT PROVE, stated because a green sweep beside a shrinking
    /// silent set will otherwise read as the same evidence the hand-written
    /// differentials give. A handler that VALIDATES a value and then ignores it
    /// passes the refusal branch. Round 1019 rejected exactly that weakness in a
    /// hand-written needle — an argument echoed back proves parsing, not
    /// filtering — so this is a FLOOR under the surface and not a substitute for
    /// the differentials, and the accounting prints the two counts separately for
    /// that reason.
    ///
    /// It also cannot tell a correct refusal from an empty answer where a
    /// refusal was intended: measuring found fifteen existing-ref arguments that
    /// all fail loud on an unregistered value, and the sweep would be equally
    /// green if one of them returned an empty dossier instead, because it has no
    /// way to know which polarity an argument was meant to have. That needs the
    /// polarity to be a TYPE, the way Round 1014 made a path one.
    macro_rules! probed {
        ($(
            $test:ident :
            $( @ $world:ident )?
            $( {$file:literal = $contents:tt} )*
            $( (store $blind:literal = $( [$blind_call:ident($blind_args:ty) $blind_base:tt] )* ) )*
            $( [$setup:ident($setup_args:ty) $setup_base:tt] )*
            $tool:ident($args:ty) $base:tt
            $( when $when_arg:literal $when_base:tt )*
            $( except $except:literal )*
            ;
        )*) => {
            /// Every tool with a declared valid call, beside the required
            /// arguments of it the schema describes no probe value for.
            const PROBED: &[(&str, &[&str])] = &[
                $((stringify!($tool), &[$($except),*])),*
            ];
            $(
                #[tokio::test]
                async fn $test() {
                    let tool = stringify!($tool);
                    let excepted: &[&str] = &[$($except),*];
                    let required: Vec<String> = required_arguments()
                        .into_iter()
                        .filter(|(t, _)| t == tool)
                        .map(|(_, a)| a)
                        .collect();
                    assert!(
                        !required.is_empty(),
                        "the router says `{tool}` has no required argument, so \
                         this declaration probes nothing"
                    );
                    // AN EXCEPTION MUST BE A REAL REQUIRED ARGUMENT, or it is a
                    // way to silence a probe by misspelling it.
                    for skip in excepted {
                        assert!(
                            required.iter().any(|a| a == skip),
                            "`{tool}` is declared with `except {skip}`, which is \
                             not a required argument of it"
                        );
                    }
                    // EVERY REQUIRED ARGUMENT IS PROBED, INCLUDING THE EXCEPTED
                    // ONES — an exception says the probe CANNOT CONCLUDE about
                    // this argument, and that is a claim about behaviour, so it
                    // is executed rather than trusted. Round 987's defect was a
                    // list that said a test existed; an unearned exception is the
                    // same list with a different name.
                    // A CLOSED SET GETS ONE ARM PER MEMBER, because "the value
                    // decides the answer" is a claim about a PAIR of members and
                    // one arm can only ever test one pair. Round 995's hazard is
                    // why it matters here rather than being thoroughness: a
                    // member equal to the field's default is normalised away by
                    // the store, so an arm that happens to pick it observes
                    // nothing and says so about the HANDLER. Trying every other
                    // member turns that into what it is — a fact about one
                    // value, not about the argument.
                    let declared_base = serde_json::json!($base);
                    let candidates = |arg: &str| -> Vec<Option<String>> {
                        match declared_closed_set(tool, arg) {
                            Some(members) => members
                                .into_iter()
                                .filter(|m| declared_base[arg].as_str() != Some(m.as_str()))
                                .map(Some)
                                .collect(),
                            None => vec![None],
                        }
                    };
                    let mut arms: Vec<Option<(String, Option<String>)>> = vec![None];
                    arms.extend(required.iter().flat_map(|a| {
                        candidates(a).into_iter().map(|m| Some((a.clone(), m)))
                    }));
                    // AN OPTIONAL ARGUMENT THAT DECLARES A POLARITY IS PROBED
                    // TOO. The coverage accounting owns the two halves and this
                    // sweep only ever reached the required one; polarity is not
                    // a question about required-ness, and an optional reference
                    // that answers empty instead of refusing is the same defect
                    // in the same place. Nothing new is declared for it — the
                    // tool already has a valid call, and the probe ADDS the
                    // argument to it rather than replacing a value.
                    let optional_typed: Vec<String> = optional_arguments()
                        .into_iter()
                        .filter(|(t, a)| t == tool && declared_polarity(t, a).is_some())
                        .map(|(_, a)| a)
                        .collect();
                    arms.extend(optional_typed.iter().flat_map(|a| {
                        candidates(a).into_iter().map(|m| Some((a.clone(), m)))
                    }));
                    let mut base_answer = String::new();
                    let mut base_store = String::new();
                    let mut base_wrote = false;
                    let mut verdicts: Vec<(String, &'static str, String)> = Vec::new();
                    // ONE RECORD OF WHAT EVERY ARM DID, read by two questions.
                    // The coverage accounting owns the REQUIRED half and asks
                    // only about those rows; the polarity check owns every
                    // marked argument and asks about all of them. Keeping two
                    // vectors is how the two early `inconclusive` paths below
                    // came to be counted as required when they were not.
                    let is_required = |arg: &str| required.iter().any(|r| r == arg);
                    // AN ARGUMENT WHOSE VALIDITY DEPENDS ON ANOTHER ARGUMENT
                    // CANNOT BE PROBED FROM ONE BASE CALL. `superseding` is
                    // rejected unless `status` is `superseded` and `resolving`
                    // unless it is `open`, so the two are mutually exclusive and
                    // no single valid call admits both: probing either from the
                    // other's base gets a refusal that never reached the lookup,
                    // which the sweep correctly refuses to read as evidence. So
                    // the declaration may say what ELSE this one arm sends, and
                    // the arm is otherwise the same probe.
                    let when_bases: Vec<(&str, serde_json::Value)> = vec![
                        $(($when_arg, serde_json::json!($when_base))),*
                    ];
                    for (arg, _) in &when_bases {
                        assert!(
                            required.iter().any(|a| a == arg)
                                || optional_typed.iter().any(|a| a == arg),
                            "`{tool}` declares `when {arg}`, which is not an \
                             argument this sweep probes"
                        );
                    }
                    for arm in arms {
                        let tmp = agent_workspace();
                        let server =
                            MnemosyneServer::new(tmp.path().to_path_buf()).expect("server");
                        // THE WORLD AND THE SETUP RUN IN EVERY ARM, so the only
                        // difference between the base and a probe is the one
                        // argument.
                        $( $world(&server, tmp.path()).await; )?
                        $(
                            std::fs::write(
                                tmp.path().join($file),
                                serde_json::to_string(&serde_json::json!($contents))
                                    .expect("the given file must serialize"),
                            )
                            .unwrap_or_else(|e| panic!("write {}: {e}", $file));
                        )*
                        // A BLIND STORE A GATE READS, built by a second server in
                        // every arm — the same shape `exercised!` takes, so a
                        // declaration can move between the two without being
                        // rewritten.
                        $(
                            {
                                let blind_ws = agent_workspace();
                                let blind = MnemosyneServer::new(
                                    blind_ws.path().to_path_buf(),
                                )
                                .expect("the second server");
                                $(
                                    let seed: $blind_args = serde_json::from_value(
                                        serde_json::json!($blind_base),
                                    )
                                    .expect("the blind store call's shape must parse");
                                    let built = blind.$blind_call(Parameters(seed)).await;
                                    assert!(
                                        built.is_error != Some(true),
                                        "building the blind store `{}` failed at {}: {:?}",
                                        $blind,
                                        stringify!($blind_call),
                                        built.content
                                    );
                                )*
                                std::fs::copy(
                                    blind_ws.path().join("docs/.atomic/workspace.atomic.json"),
                                    tmp.path().join($blind),
                                )
                                .unwrap_or_else(|e| panic!("seed {}: {e}", $blind));
                            }
                        )*
                        $(
                            let setup: $setup_args =
                                serde_json::from_value(serde_json::json!($setup_base))
                                    .expect("the setup call's shape must parse");
                            let ready = server.$setup(Parameters(setup)).await;
                            assert!(
                                ready.is_error != Some(true),
                                "the `given` call {} failed, so {tool} was never \
                                 reached: {:?}",
                                stringify!($setup),
                                ready.content
                            );
                        )*
                        let mut json = serde_json::json!($base);
                        let mut needle = String::new();
                        let mut was = String::new();
                        if let Some((arg, member)) = &arm {
                            // The `when` fields land BEFORE the probe value, so
                            // an override may also supply a valid value for the
                            // argument about to be replaced.
                            if let Some((_, extra)) =
                                when_bases.iter().find(|(a, _)| a == arg)
                            {
                                if let (Some(dst), Some(src)) =
                                    (json.as_object_mut(), extra.as_object())
                                {
                                    for (k, v) in src {
                                        dst.insert(k.clone(), v.clone());
                                    }
                                }
                            }
                            // NO PROBE VALUE THE SCHEMA DESCRIBES IS NO VERDICT.
                            // An array of objects is the shape this still
                            // reaches; it is recorded as inconclusive rather
                            // than skipped, so the exception that covers it has
                            // to be earned here.
                            let derived = match member {
                                Some(m) => Some((
                                    serde_json::Value::from(m.clone()),
                                    m.clone(),
                                )),
                                None => probe_value(tool, arg, &json[arg.as_str()]),
                            };
                            let Some((value, n)) = derived else {
                                verdicts.push((
                                    arg.clone(),
                                    "inconclusive",
                                    format!(
                                        "the schema describes no probe value for \
                                         it — declared type `{}`, property {}",
                                        declared_type(tool, arg),
                                        agent_property(tool, arg)
                                            .map(|p| p.to_string())
                                            .unwrap_or_else(|| "absent".to_string())
                                    ),
                                ));
                                continue;
                            };
                            was = json[arg.as_str()].to_string();
                            assert_ne!(
                                json[arg.as_str()], value,
                                "the declared call for `{tool}` already sends the \
                                 probe value for `{arg}`, so its two arms would \
                                 be identical"
                            );
                            json[arg.as_str()] = value;
                            needle = n;
                        }
                        let parsed: Result<$args, _> = serde_json::from_value(json);
                        let args = match (parsed, &arm) {
                            (Ok(args), _) => args,
                            // A PROBE THAT SERDE REJECTS NEVER REACHED THE
                            // HANDLER, so it is evidence about the wire shape
                            // and none at all about the handler.
                            (Err(e), Some((arg, _))) => {
                                verdicts.push((
                                    arg.clone(),
                                    "inconclusive",
                                    format!("the probe value does not deserialize \
                                             ({e}), so it never reaches the handler"),
                                ));
                                continue;
                            }
                            (Err(e), None) => panic!(
                                "the call declared for `{tool}` does not even parse: {e}"
                            ),
                        };
                        let store_path =
                            tmp.path().join("docs/.atomic/workspace.atomic.json");
                        let before = std::fs::read_to_string(&store_path)
                            .expect("read the store");
                        let result = server.$tool(Parameters(args)).await;
                        let after = std::fs::read_to_string(&store_path)
                            .expect("read the store");
                        let answer = answer_text(&result);
                        let Some((arg, _)) = arm else {
                            // THE FLOOR. Every probe below is interpreted against
                            // this call having worked; if it did not, every
                            // refusal that follows is the base's and the sweep
                            // would report reading that never happened.
                            assert!(
                                result.is_error != Some(true),
                                "the call declared for `{tool}` is NOT VALID, so \
                                 every probe of it would be refused for this \
                                 reason rather than for the argument being \
                                 probed: {:?}",
                                result.content
                            );
                            base_answer = answer;
                            base_store = after;
                            base_wrote = base_store != before;
                            // THE MIRROR ORACLE, RUN WHERE THE WORLD IS ALREADY
                            // STANDING. `FreshId` says the id is the one the new
                            // record is filed under, and what that forbids is a
                            // SECOND record under an id the store already holds.
                            // Round 1022 measured the rule — a duplicate `add_*`
                            // is a no-op when the content is identical and a
                            // refusal when it diverges — and recorded that it was
                            // hand-written prose at each site with nothing
                            // enforcing it. Repeating the declared call is the
                            // whole probe: whether it is refused or absorbed is
                            // the primitive's choice, and either way the id must
                            // occur exactly as often as it did after one call.
                            // Counting rather than comparing bytes is what makes
                            // this safe against a store that stamps anything.
                            for arg in required.iter().chain(optional_typed.iter()) {
                                if declared_polarity(tool, arg) != Some(FRESH_ID_FORMAT) {
                                    continue;
                                }
                                let declared = serde_json::json!($base);
                                let Some(value) = declared
                                    .get(arg.as_str())
                                    .and_then(|v| v.as_str())
                                    .map(str::to_string)
                                else {
                                    continue;
                                };
                                let value = value.as_str();
                                let once = base_store.matches(value).count();
                                assert!(
                                    once > 0,
                                    "`{tool}.{arg}` is typed `FreshId`, but the id \
                                     `{value}` the declared call files under is \
                                     nowhere in the store afterwards, so repeating \
                                     the call cannot be counted. Either the record \
                                     is filed under something else and the type is \
                                     wrong, or the call did not write"
                                );
                                let again: $args = serde_json::from_value(declared)
                                    .expect("the declared call's shape must parse");
                                let _ = server.$tool(Parameters(again)).await;
                                let twice = std::fs::read_to_string(&store_path)
                                    .expect("read the store");
                                assert_eq!(
                                    twice.matches(value).count(),
                                    once,
                                    "`{tool}.{arg}` is typed `FreshId`, so calling \
                                     twice with the id `{value}` must leave ONE \
                                     record — the second call is either refused or \
                                     absorbed as a no-op. The store holds it more \
                                     often after the second call, so a duplicate \
                                     was filed under an id the store already had"
                                );
                            }
                            continue;
                        };
                        let verdict = if result.is_error == Some(true) {
                            if answer.contains(&needle) {
                                ("validated", format!("refused it by name: {answer}"))
                            } else {
                                // A REFUSAL THAT DOES NOT NAME THE VALUE may be
                                // for an unrelated reason, so it is not evidence
                                // that this argument was read.
                                (
                                    "inconclusive",
                                    format!(
                                        "refused WITHOUT naming the value, so the \
                                         refusal may be for an unrelated reason: \
                                         {answer}"
                                    ),
                                )
                            }
                        } else if base_wrote {
                            let (b, a) = (
                                base_store.matches(&needle).count(),
                                after.matches(&needle).count(),
                            );
                            // A CLOSED SET COUNTS BOTH WAYS. Both arms send a
                            // VALID member, so the value the base sent is as
                            // countable as the one the probe sent — and it has to
                            // be, because a store omits a field equal to its
                            // default (Round 995): `verification_expectation`
                            // `dedicated` is written as nothing at all, so an arm
                            // that sends it can only be seen by the base's member
                            // DISAPPEARING. Watching one direction called that
                            // "the handler ignored it".
                            let closed = declared_closed_set(tool, &arg).is_some();
                            let was_tag = was.trim_matches('"').to_string();
                            let (wb, wa) = (
                                base_store.matches(&was_tag).count(),
                                after.matches(&was_tag).count(),
                            );
                            if closed && a <= b && wa < wb {
                                (
                                    "stored",
                                    format!(
                                        "the member the base sent ({was}) occurs \
                                         {wb} time(s) without `{needle}` and {wa} \
                                         with it, so the store follows the member \
                                         chosen even where the chosen one is the \
                                         default it omits"
                                    ),
                                )
                            } else if a > b {
                                ("stored", format!("the store holds the probe value {a} time(s)"))
                            } else if after == before {
                                // THE ARGUMENT DECIDED WHETHER ANYTHING HAPPENED
                                // AT ALL. A subtractive or selective argument
                                // never appears in the store — a pattern that
                                // matches nothing leaves no trace of itself — so
                                // counting it says nothing, exactly as Round 989
                                // found for the optional `limit`. What it does
                                // say is that the base call WROTE and this one
                                // did not, and that comparison is safe against
                                // the stamped-timestamp hazard the needle exists
                                // to avoid, because it compares each arm against
                                // ITS OWN before.
                                (
                                    "selected",
                                    format!(
                                        "the base value {was} made the tool write \
                                         and a value naming nothing made it write \
                                         nothing"
                                    ),
                                )
                            } else {
                                // ACCEPTED AND NOWHERE IN THE STORE. Two causes
                                // look identical from here and the sweep cannot
                                // separate them: the handler ignored the
                                // argument, or it validated it and the store
                                // deliberately does not keep it — which
                                // `retract_fact.reason` does, its primitive
                                // saying so outright ("the transaction-time audit
                                // of a retraction is the git history of the log,
                                // so nothing is tombstoned in the store; `reason`
                                // is mandatory as the audit-trail safeguard").
                                (
                                    "inconclusive",
                                    format!(
                                        "ACCEPTED and the store holds it {a} \
                                         time(s) ({b} with {was}) — either the \
                                         handler ignored it, or it is validated \
                                         and deliberately not kept"
                                    ),
                                )
                            }
                        } else {
                            // A READ TOOL LEAVES ONLY ITS ANSWER, and an answer
                            // that differs ONLY by the value echoed back proves
                            // the argument was parsed, not that it was used
                            // (Round 1019). So the echo is removed from both
                            // sides and what remains still has to differ.
                            let mark = "\u{a7}";
                            // An argument the base call OMITS has no echo to
                            // strip from the base's own answer, and `null` is
                            // not a value that was echoed anywhere — replacing
                            // it would edit text the base really did produce.
                            let stripped_base = if was == "null" {
                                base_answer.clone()
                            } else {
                                base_answer.replace(was.trim_matches('"'), mark)
                            };
                            if answer.replace(&needle, mark) != stripped_base {
                                ("answered", "the answer differs beyond the echo".to_string())
                            } else {
                                (
                                    "inconclusive",
                                    "ACCEPTED and the answer differs from the \
                                     base's ONLY by the value echoed back"
                                        .to_string(),
                                )
                            }
                        };
                        // The coverage accounting owns the REQUIRED half only;
                        // the polarity check owns every probed argument, so an
                        // optional reference cannot be marked and then never
                        // asked about.
                        verdicts.push((arg, verdict.0, verdict.1));
                    }
                    // ONE VERDICT PER ARGUMENT, however many members it has.
                    // A closed set ran one arm per member and "the value decides
                    // the answer" is satisfied by ANY pair that differs, so the
                    // strongest arm is the argument's verdict — and when none of
                    // them moved the answer, every member tried is named, since
                    // "no other member changed anything" is the finding and a
                    // single member's silence is not.
                    let rank = |v: &str| match v {
                        "stored" | "answered" | "selected" => 0,
                        "validated" => 1,
                        _ => 2,
                    };
                    let mut collapsed: Vec<(String, &'static str, String)> = Vec::new();
                    for arg in verdicts
                        .iter()
                        .map(|(a, _, _)| a.clone())
                        .collect::<BTreeSet<String>>()
                    {
                        let mut mine: Vec<&(String, &'static str, String)> =
                            verdicts.iter().filter(|(a, _, _)| *a == arg).collect();
                        mine.sort_by_key(|(_, v, _)| rank(v));
                        let (_, verdict, why) = mine[0];
                        let why = if mine.len() > 1 && rank(verdict) == 2 {
                            format!(
                                "no member moved the answer — {}",
                                mine.iter()
                                    .map(|(_, _, w)| w.as_str())
                                    .collect::<Vec<_>>()
                                    .join(" / ")
                            )
                        } else {
                            why.clone()
                        };
                        collapsed.push((arg, verdict, why));
                    }
                    let verdicts = collapsed;
                    // WHAT EVERY ARM DID IS PRINTED BEFORE ANYTHING IS ASSERTED,
                    // so a failure below arrives with the whole tool's picture
                    // rather than one line of it (Round 854).
                    for (arg, verdict, why) in &verdicts {
                        println!("  {tool}.{arg}: {verdict} — {why}");
                    }
                    // A CLOSED SET IS A DIFFERENTIAL, NOT A FLOOR. serde already
                    // refuses every non-member before the handler is reached, so
                    // the probe sends ANOTHER MEMBER and the only honest verdict
                    // is that the answer moved. A refusal here is not the
                    // validation evidence a sentinel's refusal is — it says a
                    // VALID member was rejected, which is either a dependency the
                    // declaration must supply with `when` or a defect.
                    for (arg, verdict, why) in &verdicts {
                        let Some(members) = declared_closed_set(tool, arg) else {
                            continue;
                        };
                        assert!(
                            ["stored", "answered", "selected"].contains(verdict),
                            "`{tool}.{arg}` declares the closed set {members:?}, so \
                             the sweep probes it with a second MEMBER and the \
                             answer must move: {why}. A member the wire admits and \
                             the handler refuses is a dependency on another \
                             argument (declare it with `when`) or a defect"
                        );
                    }
                    // AND NO REFUSAL RE-SPELLS A SET THE SCHEMA HIDES. A handler
                    // that lists a vocabulary in its refusal is enforcing a
                    // closed set the wire type does not carry, so an agent
                    // reading the schema sees `string` and learns the members
                    // only by guessing wrong — the defect this round closed for
                    // ten arguments. The check is on the MEMBERS rather than on
                    // the sentence, because the sentence was not one sentence:
                    // nine sites said "must be `a`, `b`, or `c`" and
                    // `ConfirmMethod` said "a|b|c", and a matcher keyed on the
                    // phrasing missed exactly that one while measuring.
                    for (arg, verdict, why) in &verdicts {
                        if *verdict != "validated" || declared_closed_set(tool, arg).is_some() {
                            continue;
                        }
                        for (name, members) in registered_vocabularies() {
                            assert!(
                                !names_every_member(why, &members),
                                "`{tool}.{arg}` is refused by a message naming \
                                 every member of `{name}` ({members:?}), so the \
                                 handler is enforcing a closed set the SCHEMA \
                                 does not declare — an agent reading the tool \
                                 sees an open string and finds the set by being \
                                 refused. Type the argument with the enum, the \
                                 way the ten Round 1027 moved are: {why}"
                            );
                        }
                    }
                    // AND A REFUSAL IS A VALUE SPACE THE SCHEMA MUST NAME.
                    //
                    // `validated` is the verdict for an argument whose handler
                    // REFUSED a value naming nothing. The refusal is itself the
                    // evidence that the space is constrained — and where the
                    // schema says nothing about it, the only way an agent can
                    // learn the constraint is to send a wrong value and read
                    // the error. That is the shape the round above closed for
                    // ten arguments; this is the general form of it, and it
                    // needs no tripwire table to fall out of date, because the
                    // population is every refusal THE SWEEP ITSELF COLLECTED
                    // and the question is put to the property an agent reads.
                    //
                    // The three answers that count are the three JSON Schema
                    // has: `enum` for a closed set, `format` for a named shape
                    // — the markers stamp theirs, so a typed argument satisfies
                    // this by construction — and `pattern` for a space only a
                    // regex states. EVERY VIOLATION IS COLLECTED before the
                    // assertion, so a failure arrives as the tool's whole list
                    // rather than its first line, and each one carries the
                    // property it MET, since a diagnosis that does not say what
                    // it found is how an axis gets miscounted.
                    let mut mute: Vec<String> = Vec::new();
                    let mut stated: Vec<String> = Vec::new();
                    for (arg, verdict, why) in &verdicts {
                        if *verdict != "validated" {
                            continue;
                        }
                        let property = agent_property(tool, arg)
                            .expect("a probed argument is a property of its tool");
                        let how = if closed_set_members(&property).is_some() {
                            Some("enum")
                        } else if property.get("format").is_some() {
                            Some("format")
                        } else if property.get("pattern").is_some() {
                            Some("pattern")
                        } else {
                            None
                        };
                        match how {
                            Some(how) => stated.push(format!("{arg}: {how}")),
                            None => mute.push(format!(
                                "`{tool}.{arg}` refused it ({why}) and its schema \
                                 states no value space: {property}"
                            )),
                        }
                        // AND THE `enum` ANSWER IS UNREACHABLE HERE, WHICH THE RUN
                        // SAYS RATHER THAN A READER ASSUMING IT EITHER WAY. A
                        // declared closed set is refused by serde before the
                        // handler, so it cannot produce a `validated` verdict —
                        // and the arm above this one already asserts that such an
                        // argument's answer MOVES. If this ever fires, that arm
                        // stopped running: the branch is kept because the three
                        // answers are the classification, not because one of them
                        // is expected.
                        assert!(
                            how != Some("enum"),
                            "`{tool}.{arg}` reached the value-space check with a \
                             declared closed set AND a `validated` verdict. serde \
                             refuses a non-member before the handler, so this \
                             means a VALID member was refused and the assertion \
                             that a closed set must move the answer no longer \
                             runs: {why}"
                        );
                    }
                    println!(
                        "  {tool}: value space stated for {} of {} refused \
                         argument(s){}{}",
                        stated.len(),
                        stated.len() + mute.len(),
                        if stated.is_empty() { "" } else { " — " },
                        stated.join(", ")
                    );
                    assert!(
                        mute.is_empty(),
                        "{} argument(s) of `{tool}` enforce a value space the \
                         schema does not state, so an agent finds the rule only \
                         by being refused: {}",
                        mute.len(),
                        mute.join(" | ")
                    );
                    // A MARK IS AN ORACLE, NOT A LABEL. `ExistingRef` says a
                    // value naming nothing is refused by name, which is exactly
                    // the observation this sweep already makes — so the type
                    // turns a rule that was hand-written prose at each site into
                    // the one thing every run checks.
                    for (arg, verdict, why) in &verdicts {
                        match declared_polarity(tool, arg) {
                            Some(EXISTING_REF_FORMAT) => assert_eq!(
                                *verdict, "validated",
                                "`{tool}.{arg}` is typed `ExistingRef`, so a \
                                 value naming nothing must be REFUSED and the \
                                 refusal must name it. It was not: {why}. Either \
                                 the handler stopped looking the value up — \
                                 which answers an agent about a thing that does \
                                 not exist, in the shape of an answer about one \
                                 that does — or the reference is resolved at the \
                                 scan boundary instead and the type is \
                                 `ScannedRef`"
                            ),
                            Some(SCANNED_REF_FORMAT) => assert_ne!(
                                *verdict, "validated",
                                "`{tool}.{arg}` is typed `ScannedRef`, which \
                                 says the CALL admits a value naming nothing and \
                                 the scan refuses it. The call refused it, so \
                                 the reference is checked here and the type is \
                                 `ExistingRef`: {why}"
                            ),
                            _ => {}
                        }
                    }
                    // AND A SCANNED REFERENCE IS ONLY HONEST IF THE SCAN
                    // ACTUALLY SPEAKS. The classification table records
                    // `superseded_by` as covered by the orphan scan and cites a
                    // measurement for it; this runs that measurement every time,
                    // through the same `validate_workspace` an agent would call,
                    // so "the other gate has it" cannot decay into a sentence
                    // nobody re-checked.
                    let mut unscanned: Vec<String> = Vec::new();
                    for arg in required.iter().chain(optional_typed.iter()) {
                        if declared_polarity(tool, arg) != Some(SCANNED_REF_FORMAT) {
                            continue;
                        }
                        let tmp = agent_workspace();
                        let server =
                            MnemosyneServer::new(tmp.path().to_path_buf()).expect("server");
                        $( $world(&server, tmp.path()).await; )?
                        $(
                            let setup: $setup_args =
                                serde_json::from_value(serde_json::json!($setup_base))
                                    .expect("the setup call's shape must parse");
                            server.$setup(Parameters(setup)).await;
                        )*
                        let clean = answer_text(
                            &server.validate_workspace(Parameters(EmptyArgs {})).await,
                        );
                        let mut json = serde_json::json!($base);
                        if let Some((_, extra)) = when_bases.iter().find(|(a, _)| a == arg) {
                            if let (Some(dst), Some(src)) =
                                (json.as_object_mut(), extra.as_object())
                            {
                                for (k, v) in src {
                                    dst.insert(k.clone(), v.clone());
                                }
                            }
                        }
                        let (value, _) = probe_value(tool, arg, &json[arg.as_str()])
                            .expect("a scanned reference is a string the schema describes");
                        json[arg.as_str()] = value;
                        let args: $args = serde_json::from_value(json)
                            .expect("the probe call's shape must parse");
                        let dangled = server.$tool(Parameters(args)).await;
                        assert!(
                            dangled.is_error != Some(true),
                            "`{tool}.{arg}` is typed `ScannedRef` but the call \
                             refused it: {:?}",
                            dangled.content
                        );
                        let after = answer_text(
                            &server.validate_workspace(Parameters(EmptyArgs {})).await,
                        );
                        if after == clean {
                            unscanned.push(arg.clone());
                        }
                    }
                    // EVERY SCANNED REFERENCE IS ASKED BEFORE ANY IS REPORTED,
                    // because stopping at the first turns a measurement of the
                    // whole tool into one line of it — which is how
                    // `resolved_by` and `superseded_by`, classified identically
                    // and behaving differently, would have looked the same.
                    assert!(
                        unscanned.is_empty(),
                        "{} argument(s) of `{tool}` are typed `ScannedRef` and \
                         NOTHING refuses the value — not the call, which \
                         admitted it, and not the boundary the type points at: \
                         `validate_workspace` says exactly what it said before \
                         the dangling reference was written. {unscanned:?}",
                        unscanned.len()
                    );
                    // THE EXCEPTIONS AND THE INCONCLUSIVE PROBES MUST BE THE SAME
                    // SET. One direction stops an exception from silencing a
                    // probe that would have concluded; the other stops an
                    // inconclusive probe from being counted as coverage. Neither
                    // is a list anyone maintains by hand: the left side is what
                    // this declaration says and the right side is what the tool
                    // did.
                    let inconclusive: BTreeSet<&str> = verdicts
                        .iter()
                        .filter(|(a, v, _)| *v == "inconclusive" && is_required(a))
                        .map(|(a, _, _)| a.as_str())
                        .collect();
                    let claimed: BTreeSet<&str> = excepted.iter().copied().collect();
                    assert_eq!(
                        inconclusive,
                        claimed,
                        "`{tool}`: the probe concluded about a DIFFERENT set of \
                         arguments than this declaration excepts. Every argument \
                         the probe cannot conclude about must be excepted (and \
                         stays named in the unexercised population); an argument \
                         it CAN conclude about must not be, or the exception is \
                         hiding evidence that exists"
                    );
                    println!(
                        "{tool}: {} of {} required argument(s) concluded, {} \
                         excepted; {} marked optional argument(s) probed for \
                         polarity",
                        verdicts.iter().filter(|(a, _, _)| is_required(a)).count()
                            - inconclusive.len(),
                        required.len(),
                        inconclusive.len(),
                        optional_typed.len()
                    );
                }
            )*
        };
    }

    /// A NAMED WORLD — DECLARED ONCE, ESTABLISHED BY EVERY CASE THAT ASKS.
    ///
    /// The same syntax `exercised!` takes for its own setup, so a world is
    /// written the way a case's setup is written and can be lifted into one
    /// without being rephrased.
    ///
    /// It exists because the branch story below was copy-pasted into SIX cases
    /// as five byte-identical lines, and the eighteen remaining path arguments
    /// would each have wanted the same five. That is not only bulk: a world
    /// improved in one copy is a world that now differs from five others, and
    /// what these cases are converging on is a world that looks like what the
    /// north star actually authors. One home for it means improving it improves
    /// every case that stands in it.
    macro_rules! world {
        ($(
            $name:ident :
            $( {$file:literal = $contents:tt} )*
            $( [$setup:ident($setup_args:ty) $setup_base:tt] )*
        ;)*) => {
            $(
                async fn $name(server: &MnemosyneServer, ws: &std::path::Path) {
                    $(
                        std::fs::write(
                            ws.join($file),
                            serde_json::to_string(&serde_json::json!($contents))
                                .expect("the world's file must serialize"),
                        )
                        .unwrap_or_else(|e| panic!("write {}: {e}", $file));
                    )*
                    $(
                        let setup: $setup_args =
                            serde_json::from_value(serde_json::json!($setup_base))
                                .expect("the world's setup call must parse");
                        let ready = server.$setup(Parameters(setup)).await;
                        assert!(
                            ready.is_error != Some(true),
                            "establishing the world `{}` failed at {}, so every \
                             case standing in it is measuring the failure: {:?}",
                            stringify!($name),
                            stringify!($setup),
                            ready.content
                        );
                    )*
                }
            )*
        };
    }

    world! {
        // A BRANCHED STORY, not a fixture: three scenes, a fork at the second,
        // a fact that exists only down the fork, a map, someone who moves along
        // it, and two tellings that disclose differently. Three verbs already
        // share it, and each new report argument that stands here is a line
        // rather than a world.
        //
        // IT ALSO BELIEVES AND IT ALSO WANTS, because five reports had nothing
        // to say without that. Round 1017 read six order-keyed reports as
        // order-INSENSITIVE when what they actually returned was `windows: []`,
        // `setups_total: 0`, `quests: []` — a report with an empty body cannot
        // be moved by ANY argument, and an argument tested against one is
        // untestable rather than unread. So the world gained the two things
        // those reports read: a SECOND FRAME that holds the way shut while
        // ground truth holds it open (the cross-frame divergence irony is
        // measured over), and a QUEST taken up in the first scene and completed
        // in the third (the credited setup payoff substantiation classifies and
        // the reserved `pursues`/`completed_by` roles the quest graph derives
        // from). Both are what the north star authors, not scaffolding for a
        // test.
        branch_story:
            {"order-a.json" = {"schema": "canon-order/v1", "edges": [["sc-01", "sc-02"]]}}
            {"order-b.json" = {"schema": "canon-order/v1", "edges": [["sc-01", "sc-02"], ["sc-02", "sc-03"]]}}
            // THE THIRD ORDER READS THE SAME SCENES IN A DIFFERENT SEQUENCE, and
            // that is a distinct discriminator rather than a third fixture.
            // `order-a` and `order-b` differ by MEMBERSHIP — b admits `sc-03` and
            // a does not — so a report keyed on WHICH SCENES EXIST tells them
            // apart while a report keyed on WHICH CAME FIRST cannot. Round 1017
            // wrote six cases against a-vs-b, watched them come back
            // byte-identical, and deleted them. Against `order-c` the membership
            // is b's and only the sequence moves: `sc-03` is read FIRST, so the
            // fact it establishes (`f-at-b`, which supersedes `f-at-a`) now lands
            // before the fact it supersedes.
            {"order-c.json" = {"schema": "canon-order/v1", "edges": [["sc-03", "sc-01"], ["sc-01", "sc-02"]]}}
            // A ROAD THAT NEVER REACHES THE FORK POINT. `b-alt` forks at
            // `sc-02`, so an order that omits `sc-02` leaves the fork UNPLACED —
            // the one thing the fork tree reads the order for. `order-a` also
            // omits a scene, but not that one.
            {"order-d.json" = {"schema": "canon-order/v1", "edges": [["sc-01", "sc-03"]]}}
            [import_sections(ImportSectionsArgs) {"sections": [{"section_id": "sc-01", "parent_doc": "spec", "title": "one"}, {"section_id": "sc-02", "parent_doc": "spec", "title": "two"}, {"section_id": "sc-03", "parent_doc": "spec", "title": "three"}]}]
            [import_facts(atomic::FactsManifest) {"frames": [{"frame_id": "ground-truth"}, {"frame_id": "she-believes"}], "entity_kinds": [{"kind_id": "place"}, {"kind_id": "character"}, {"kind_id": "quest"}], "entities": [{"entity_id": "p-a", "kind": "place"}, {"entity_id": "p-b", "kind": "place"}, {"entity_id": "e-her", "kind": "character"}, {"entity_id": "q-cross", "kind": "quest"}], "predicates": [{"predicate_id": "adjacent", "object_kind": "entity", "subject_kind": "place", "object_entity_kind": "place"}, {"predicate_id": "at", "object_kind": "entity", "subject_kind": "character", "object_entity_kind": "place"}, {"predicate_id": "pursues", "object_kind": "entity", "subject_kind": "character", "object_entity_kind": "quest"}, {"predicate_id": "completed_by", "object_kind": "entity", "subject_kind": "quest", "object_entity_kind": "character"}], "facts": [{"fact_id": "f-way", "frame": "ground-truth", "claim": "a way runs", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["p-a", "p-b"], "typed": {"subject": "p-a", "predicate": "adjacent", "object": {"kind": "entity", "id": "p-b"}}}, {"fact_id": "f-at-a", "frame": "ground-truth", "claim": "she is at a", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["e-her", "p-a"], "typed": {"subject": "e-her", "predicate": "at", "object": {"kind": "entity", "id": "p-a"}}}, {"fact_id": "f-at-b", "frame": "ground-truth", "claim": "she is at b", "canon_from": "sc-03", "evidence": ["sc-03"], "entities": ["e-her", "p-b"], "supersedes_in_frame": "f-at-a", "typed": {"subject": "e-her", "predicate": "at", "object": {"kind": "entity", "id": "p-b"}}}, {"fact_id": "f-believes-shut", "frame": "she-believes", "claim": "she believes no way runs", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["p-a", "p-b"], "conflicts_with": ["f-way"]}, {"fact_id": "f-quest-given", "frame": "ground-truth", "claim": "she takes up the crossing", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["e-her", "q-cross"], "payoff_expectation": "expected", "typed": {"subject": "e-her", "predicate": "pursues", "object": {"kind": "entity", "id": "q-cross"}}}, {"fact_id": "f-quest-done", "frame": "ground-truth", "claim": "the crossing is made", "canon_from": "sc-03", "evidence": ["sc-03"], "entities": ["q-cross", "e-her"], "pays_off": ["f-quest-given"], "typed": {"subject": "q-cross", "predicate": "completed_by", "object": {"kind": "entity", "id": "e-her"}}}], "disclosure_plans": [{"telling_id": "t-quiet", "default_mode": "state"}, {"telling_id": "t-hidden", "default_mode": "withhold"}], "branches": [{"branch_id": "b-alt", "forks_from": "main", "forks_at": "sc-02"}]}]
            [add_fact(atomic::FactImport) {"fact_id": "f-alt", "frame": "ground-truth", "branch": "b-alt", "claim": "she never left", "canon_from": "sc-03", "evidence": ["sc-03"], "entities": ["e-her", "p-a"], "typed": {"subject": "e-her", "predicate": "at", "object": {"kind": "entity", "id": "p-a"}}}]
        ;
    }

    exercised! {
        report_quest_graph_world_reaches_the_answer:
            @branch_story
            report_quest_graph(ReportQuestGraphArgs) {"telling": "t-quiet", "order_path": "order-b.json"}
            ."world" = "b-alt" seen "main" in output;
        // THE PROJECTION'S THREE ARGUMENTS (Round 1070). `world` is the one that
        // decides the ANSWER rather than where it lands: `b-alt` forks off the
        // spine, so its world-line holds the spine's six facts AND its own,
        // while `main`'s holds six. `out` is judged by the resolved path the
        // report echoes — the R1014 defect was exactly a path argument reaching
        // the filesystem unresolved and the answer reading clean anyway.
        project_world_world_reaches_the_answer:
            @branch_story
            project_world(ProjectWorldArgs) {"world": "main", "out": "projected.json"}
            ."world" = "b-alt" seen "\"kept\": 7" in output;
        project_world_out_reaches_the_answer:
            @branch_story
            project_world(ProjectWorldArgs) {"world": "main", "out": "projected.json"}
            ."out" = "elsewhere.json" seen "elsewhere.json" in output;
        project_world_store_reaches_the_answer:
            @branch_story
            (store "one-fact.json" =
                [import_sections(ImportSectionsArgs) {"sections": [{"section_id": "sc-01", "parent_doc": "spec", "title": "one"}]}]
                [import_facts(atomic::FactsManifest) {"frames": [{"frame_id": "ground-truth"}], "facts": [{"fact_id": "r-lone", "frame": "ground-truth", "claim": "the prose says one thing", "canon_from": "sc-01", "evidence": ["sc-01"]}]}]
            )
            project_world(ProjectWorldArgs) {"world": "main", "out": "projected.json"}
            ."store" = "one-fact.json" seen "\"kept\": 1" in output;
        report_playthrough_manuscript_telling_reaches_the_answer:
            @branch_story
            report_playthrough_manuscript(ReportPlaythroughManuscriptArgs) {"telling": "t-quiet", "order_path": "order-b.json"}
            ."telling" = "t-hidden" seen "withhold" in output;
        report_playthrough_manuscript_order_path_reaches_the_answer:
            @branch_story
            report_playthrough_manuscript(ReportPlaythroughManuscriptArgs) {"telling": "t-quiet", "order_path": "order-a.json"}
            ."order_path" = "order-b.json" seen "sc-03" in output;
        report_playthrough_manuscript_world_reaches_the_answer:
            @branch_story
            report_playthrough_manuscript(ReportPlaythroughManuscriptArgs) {"telling": "t-quiet", "order_path": "order-b.json"}
            ."world" = "b-alt" seen "main" in output;
        report_playthrough_manuscript_reading_walk_reaches_the_answer:
            @branch_story
            report_playthrough_manuscript(ReportPlaythroughManuscriptArgs) {"telling": "t-quiet", "order_path": "order-b.json"}
            ."reading_walk" = true seen "sc-02" in output;
        report_playable_world_world_reaches_the_answer:
            @branch_story
            report_playable_world(ReportPlayableWorldArgs) {"telling": "t-quiet", "order_path": "order-b.json"}
            ."world" = "b-alt" seen "main" in output;
        report_playable_world_order_path_reaches_the_answer:
            @branch_story
            report_playable_world(ReportPlayableWorldArgs) {"telling": "t-quiet", "order_path": "order-a.json"}
            ."order_path" = "order-b.json" seen "sc-03" in output;
        style_check_doc_reaches_the_answer:
            [add_section(AddSectionArgs) {"section_id": "40", "parent_doc": "spec", "title": "the section"}]
            [add_section(AddSectionArgs) {"section_id": "41", "parent_doc": "other", "title": "the other"}]
            [set_section_intent(SetSectionIntentArgs) {"section_id": "41", "intent": "the rest of this is in DESIGN.md and nobody wrote it as a reference"}]
            style_check(StyleCheckArgs) {}
            ."doc" = "spec" seen "cross_doc_reference_explicit" in output;
        query_term_fields_reaches_the_answer:
            [append_changelog_entry(AppendChangelogEntryArgs) {"entry_id": "Round 1", "decision_summary": "the decision names Waits", "changes_bullets": ["a change"], "verification_bullets": ["a check"]}]
            query_term(QueryTermArgs) {"pattern": "Waits"}
            ."fields" = ["changes_bullets"] seen "Round 1" in output;
        redact_term_scope_reaches_the_answer:
            [append_changelog_entry(AppendChangelogEntryArgs) {"entry_id": "Round 1", "decision_summary": "the decision names Waits", "changes_bullets": ["a change"], "verification_bullets": ["a check"]}]
            redact_term(RedactTermArgs) {"pattern": "Waits", "replacement": "lingers", "reason": "word", "applied_in": "Round 1", "dry_run": true}
            ."scope" = "changes_bullets" seen "Round 1" in output;
        redact_term_kind_reaches_the_answer:
            [append_changelog_entry(AppendChangelogEntryArgs) {"entry_id": "Round 1", "decision_summary": "the decision names Waits", "changes_bullets": ["a change"], "verification_bullets": ["a check"]}]
            redact_term(RedactTermArgs) {"pattern": "Waits", "replacement": "lingers", "reason": "word", "applied_in": "Round 1", "dry_run": true}
            ."kind" = "correction" seen "correction" in output;
        emit_publishable_override_ledger_draft_kind_reaches_the_answer:
            [append_changelog_entry(AppendChangelogEntryArgs) {"entry_id": "Round 1", "decision_summary": "the decision names Waits", "changes_bullets": ["a change"], "verification_bullets": ["a check"]}]
            [set_changelog_publishable_decision_summary(SetChangelogPublishableStringArgs) {"entry_id": "Round 1", "value": "the published line names Waits"}]
            emit_publishable_override_ledger_draft(EmitPublishableOverrideLedgerDraftArgs) {"entry_id": "Round 1", "reason": "word", "applied_in": "Round 1", "pattern": "Waits", "replacement": "lingers"}
            ."kind" = "correction" seen "correction" in output;
        style_check_severity_reaches_the_answer:
            [add_section(AddSectionArgs) {"section_id": "40", "parent_doc": "spec", "title": "the section"}]
            style_check(StyleCheckArgs) {}
            ."severity" = "warn" seen "warn" in output;
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
            set_section_decision_status(SetSectionDecisionStatusArgs) {"section_id": "40", "status": "superseded", "reason": "overtaken"}
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
            [import_facts(atomic::FactsManifest) {"frames": [{"frame_id": "ground-truth"}], "units": [{"unit_id": "minute"}], "entity_kinds": [{"kind_id": "place"}], "entities": [{"entity_id": "p-a", "kind": "place"}, {"entity_id": "p-b", "kind": "place"}], "predicates": [{"predicate_id": "adjacent", "object_kind": "entity", "subject_kind": "place", "object_entity_kind": "place"}, {"predicate_id": "condition", "object_kind": "token", "object_tokens": ["lit"], "subject_kind": "place"}], "facts": [{"fact_id": "f-way", "frame": "ground-truth", "claim": "a way runs", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["p-a", "p-b"], "typed": {"subject": "p-a", "predicate": "adjacent", "object": {"kind": "entity", "id": "p-b"}}}, {"fact_id": "f-lamp", "frame": "ground-truth", "claim": "the lamp is lit", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["p-a"], "typed": {"subject": "p-a", "predicate": "condition", "object": {"kind": "token", "token": "lit"}}}, {"fact_id": "f-door", "frame": "ground-truth", "claim": "the door is open", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["p-b"], "typed": {"subject": "p-b", "predicate": "condition", "object": {"kind": "token", "token": "lit"}}}], "disclosure_plans": [{"telling_id": "t-quiet", "default_mode": "state"}, {"telling_id": "t-hidden", "default_mode": "withhold"}], "edge_guards": [{"fact_id": "f-way", "conditions": ["f-lamp", "f-door"], "threshold": 1}]}]
            set_edge_guard_threshold(SetEdgeGuardThresholdArgs) {"fact_id": "f-way"}
            ."threshold" = 1 seen "threshold" in store;
        set_disclosure_surface_scene_reaches_the_store:
            [import_sections(ImportSectionsArgs) {"sections": [{"section_id": "sc-01", "parent_doc": "spec", "title": "scene one"}, {"section_id": "sc-02", "parent_doc": "spec", "title": "scene two"}]}]
            [import_facts(atomic::FactsManifest) {"frames": [{"frame_id": "ground-truth"}], "units": [{"unit_id": "minute"}], "entity_kinds": [{"kind_id": "place"}], "entities": [{"entity_id": "p-a", "kind": "place"}, {"entity_id": "p-b", "kind": "place"}], "predicates": [{"predicate_id": "adjacent", "object_kind": "entity", "subject_kind": "place", "object_entity_kind": "place"}, {"predicate_id": "condition", "object_kind": "token", "object_tokens": ["lit"], "subject_kind": "place"}], "facts": [{"fact_id": "f-way", "frame": "ground-truth", "claim": "a way runs", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["p-a", "p-b"], "typed": {"subject": "p-a", "predicate": "adjacent", "object": {"kind": "entity", "id": "p-b"}}}, {"fact_id": "f-lamp", "frame": "ground-truth", "claim": "the lamp is lit", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["p-a"], "typed": {"subject": "p-a", "predicate": "condition", "object": {"kind": "token", "token": "lit"}}}, {"fact_id": "f-door", "frame": "ground-truth", "claim": "the door is open", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["p-b"], "typed": {"subject": "p-b", "predicate": "condition", "object": {"kind": "token", "token": "lit"}}}], "disclosure_plans": [{"telling_id": "t-quiet", "default_mode": "state"}, {"telling_id": "t-hidden", "default_mode": "withhold"}], "edge_guards": [{"fact_id": "f-way", "conditions": ["f-lamp", "f-door"], "threshold": 1}]}]
            set_disclosure(SetDisclosureArgs) {"telling_id": "t-quiet", "fact_id": "f-lamp", "mode": "withhold"}
            ."surface_scene" = "sc-02" seen "sc-02" in store;
        set_disclosure_surface_object_reaches_the_store:
            [import_sections(ImportSectionsArgs) {"sections": [{"section_id": "sc-01", "parent_doc": "spec", "title": "scene one"}, {"section_id": "sc-02", "parent_doc": "spec", "title": "scene two"}]}]
            [import_facts(atomic::FactsManifest) {"frames": [{"frame_id": "ground-truth"}], "units": [{"unit_id": "minute"}], "entity_kinds": [{"kind_id": "place"}], "entities": [{"entity_id": "p-a", "kind": "place"}, {"entity_id": "p-b", "kind": "place"}], "predicates": [{"predicate_id": "adjacent", "object_kind": "entity", "subject_kind": "place", "object_entity_kind": "place"}, {"predicate_id": "condition", "object_kind": "token", "object_tokens": ["lit"], "subject_kind": "place"}], "facts": [{"fact_id": "f-way", "frame": "ground-truth", "claim": "a way runs", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["p-a", "p-b"], "typed": {"subject": "p-a", "predicate": "adjacent", "object": {"kind": "entity", "id": "p-b"}}}, {"fact_id": "f-lamp", "frame": "ground-truth", "claim": "the lamp is lit", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["p-a"], "typed": {"subject": "p-a", "predicate": "condition", "object": {"kind": "token", "token": "lit"}}}, {"fact_id": "f-door", "frame": "ground-truth", "claim": "the door is open", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["p-b"], "typed": {"subject": "p-b", "predicate": "condition", "object": {"kind": "token", "token": "lit"}}}], "disclosure_plans": [{"telling_id": "t-quiet", "default_mode": "state"}, {"telling_id": "t-hidden", "default_mode": "withhold"}], "edge_guards": [{"fact_id": "f-way", "conditions": ["f-lamp", "f-door"], "threshold": 1}]}]
            set_disclosure(SetDisclosureArgs) {"telling_id": "t-quiet", "fact_id": "f-lamp", "mode": "withhold", "surface_scene": "sc-02"}
            ."surface_object" = "p-b" seen "p-b" in store;
        set_disclosure_first_at_reaches_the_store:
            [import_sections(ImportSectionsArgs) {"sections": [{"section_id": "sc-01", "parent_doc": "spec", "title": "scene one"}, {"section_id": "sc-02", "parent_doc": "spec", "title": "scene two"}]}]
            [import_facts(atomic::FactsManifest) {"frames": [{"frame_id": "ground-truth"}], "units": [{"unit_id": "minute"}], "entity_kinds": [{"kind_id": "place"}], "entities": [{"entity_id": "p-a", "kind": "place"}, {"entity_id": "p-b", "kind": "place"}], "predicates": [{"predicate_id": "adjacent", "object_kind": "entity", "subject_kind": "place", "object_entity_kind": "place"}, {"predicate_id": "condition", "object_kind": "token", "object_tokens": ["lit"], "subject_kind": "place"}], "facts": [{"fact_id": "f-way", "frame": "ground-truth", "claim": "a way runs", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["p-a", "p-b"], "typed": {"subject": "p-a", "predicate": "adjacent", "object": {"kind": "entity", "id": "p-b"}}}, {"fact_id": "f-lamp", "frame": "ground-truth", "claim": "the lamp is lit", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["p-a"], "typed": {"subject": "p-a", "predicate": "condition", "object": {"kind": "token", "token": "lit"}}}, {"fact_id": "f-door", "frame": "ground-truth", "claim": "the door is open", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["p-b"], "typed": {"subject": "p-b", "predicate": "condition", "object": {"kind": "token", "token": "lit"}}}], "disclosure_plans": [{"telling_id": "t-quiet", "default_mode": "state"}, {"telling_id": "t-hidden", "default_mode": "withhold"}], "edge_guards": [{"fact_id": "f-way", "conditions": ["f-lamp", "f-door"], "threshold": 1}]}]
            set_disclosure(SetDisclosureArgs) {"telling_id": "t-quiet", "fact_id": "f-lamp", "mode": "withhold"}
            ."first_at" = [{"branch": "main", "coords": ["sc-02"]}] seen "sc-02" in store;
        set_disclosure_reveal_threshold_reaches_the_store:
            [import_sections(ImportSectionsArgs) {"sections": [{"section_id": "sc-01", "parent_doc": "spec", "title": "scene one"}, {"section_id": "sc-02", "parent_doc": "spec", "title": "scene two"}]}]
            [import_facts(atomic::FactsManifest) {"frames": [{"frame_id": "ground-truth"}], "units": [{"unit_id": "minute"}], "entity_kinds": [{"kind_id": "place"}], "entities": [{"entity_id": "p-a", "kind": "place"}, {"entity_id": "p-b", "kind": "place"}], "predicates": [{"predicate_id": "adjacent", "object_kind": "entity", "subject_kind": "place", "object_entity_kind": "place"}, {"predicate_id": "condition", "object_kind": "token", "object_tokens": ["lit"], "subject_kind": "place"}], "facts": [{"fact_id": "f-way", "frame": "ground-truth", "claim": "a way runs", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["p-a", "p-b"], "typed": {"subject": "p-a", "predicate": "adjacent", "object": {"kind": "entity", "id": "p-b"}}}, {"fact_id": "f-lamp", "frame": "ground-truth", "claim": "the lamp is lit", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["p-a"], "typed": {"subject": "p-a", "predicate": "condition", "object": {"kind": "token", "token": "lit"}}}, {"fact_id": "f-door", "frame": "ground-truth", "claim": "the door is open", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["p-b"], "typed": {"subject": "p-b", "predicate": "condition", "object": {"kind": "token", "token": "lit"}}}], "disclosure_plans": [{"telling_id": "t-quiet", "default_mode": "state"}, {"telling_id": "t-hidden", "default_mode": "withhold"}], "edge_guards": [{"fact_id": "f-way", "conditions": ["f-lamp", "f-door"], "threshold": 1}]}]
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
            set_section_decision_status(SetSectionDecisionStatusArgs) {"section_id": "40", "status": "open", "reason": "reopened"}
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
        report_transition_map_rules_path_reaches_the_answer:
            @branch_story
            {"rules-b.json" = {"schema": "narrative-rules/v1", "rules": [{"id": "moves-follow-the-map", "predicate": "at", "class": "transition", "adjacency": "adjacent", "undirected": true}]}}
            report_transition_map(ReportTransitionMapArgs) {}
            ."rules_path" = "rules-b.json" seen "moves-follow-the-map" in output;
        validate_continuity_rules_path_reaches_the_answer:
            @branch_story
            {"rules-directed.json" = {"schema": "narrative-rules/v1", "rules": [{"id": "one-way-only", "predicate": "at", "class": "transition", "adjacency": "at", "undirected": false}]}}
            validate_continuity(ValidateContinuityArgs) {"order_path": "order-b.json"}
            ."rules_path" = "rules-directed.json" seen "one-way-only" in output;
        propose_verdict_rules_path_reaches_the_answer:
            @branch_story
            {"rules-directed.json" = {"schema": "narrative-rules/v1", "rules": [{"id": "one-way-only", "predicate": "at", "class": "transition", "adjacency": "at", "undirected": false}]}}
            {"return-trip.json" = {"facts": [{"fact_id": "f-back", "frame": "ground-truth", "claim": "she returns", "canon_from": "sc-03", "evidence": ["sc-03"], "entities": ["e-her", "p-a"], "supersedes_in_frame": "f-at-b", "typed": {"subject": "e-her", "predicate": "at", "object": {"kind": "entity", "id": "p-a"}}}]}}
            propose_verdict(ProposeVerdictArgs) {"manifest_path": "return-trip.json", "order_path": "order-b.json"}
            ."rules_path" = "rules-directed.json" seen "one-way-only" in output;
        report_authoring_frontier_rules_path_reaches_the_answer:
            @branch_story
            {"rules-directed.json" = {"schema": "narrative-rules/v1", "rules": [{"id": "one-way-only", "predicate": "at", "class": "transition", "adjacency": "at", "undirected": false}]}}
            report_authoring_frontier(ReportAuthoringFrontierArgs) {"order_path": "order-b.json"}
            ."rules_path" = "rules-directed.json" seen "one-way-only" in output;
        report_timeline_gaps_rules_path_reaches_the_answer:
            @branch_story
            {"rules-interval.json" = {"schema": "narrative-rules/v1", "rules": [{"id": "falls-after-it-rises", "predicate": "fell_on_day", "class": "interval", "right": "rose_on_day", "op": "ge", "bound": {"const": 1}}]}}
            [import_facts(atomic::FactsManifest) {"units": [{"unit_id": "day"}], "predicates": [{"predicate_id": "rose_on_day", "object_kind": "quantity", "subject_kind": "character"}, {"predicate_id": "fell_on_day", "object_kind": "quantity", "subject_kind": "character"}], "facts": [{"fact_id": "f-rose", "frame": "ground-truth", "claim": "it rose on day 5", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["e-her"], "typed": {"subject": "e-her", "predicate": "rose_on_day", "object": {"kind": "quantity", "n": 5, "unit": "day"}}}]}]
            report_timeline_gaps(ReportTimelineGapsArgs) {"order_path": "order-b.json"}
            ."rules_path" = "rules-interval.json" seen "\"interval_rules\": 1" in output;
        report_payoff_coverage_order_path_reaches_the_answer:
            @branch_story
            report_payoff_coverage(ReportPayoffCoverageArgs) {"order_path": "order-a.json"}
            ."order_path" = "order-b.json" seen "\"unknown\": []" in output;
        validate_continuity_order_path_reaches_the_answer:
            @branch_story
            validate_continuity(ValidateContinuityArgs) {"order_path": "order-a.json"}
            // Round 1061 — `order_nodes` NAMES the scenes it places, so the
            // needle that follows the value is one of the names rather than the
            // count that used to stand for them.
            ."order_path" = "order-b.json" seen "\"sc-03\"" in output;
        report_authoring_frontier_order_path_reaches_the_answer:
            @branch_story
            report_authoring_frontier(ReportAuthoringFrontierArgs) {"telling": "t-quiet", "order_path": "order-a.json"}
            // Round 1061 — the frontier now names each world's ROAD, so `sc-03`
            // appears under both orders and no longer follows the value. The
            // placement axis still does: order-b places every scene.
            ."order_path" = "order-b.json" seen "\"unplaced_scenes\": []" in output;
        report_frame_view_order_path_reaches_the_answer:
            @branch_story
            report_frame_view(ReportFrameViewArgs) {"frame": "ground-truth", "at": "sc-01", "order_path": "order-a.json"}
            // Round 1054 — `not_holding` became a LIST, and the two facts it
            // names at `sc-01` are the same two `unknown` holds without the
            // order that admits `sc-03`, so a needle naming either of them
            // occurs once in BOTH answers. The emptiness of the sibling arm is
            // what the argument still moves: with `order-b` nothing is
            // undecided, without it the two facts anchored past the order are.
            ."order_path" = "order-b.json" seen "\"unknown\": []" in output;
        report_fork_tree_order_path_reaches_the_answer:
            @branch_story
            report_fork_tree(ReportForkTreeArgs) {"order_path": "order-b.json"}
            ."order_path" = "order-d.json" seen "\"at_placed\": false" in output;
        report_edge_candidates_order_path_reaches_the_answer:
            @branch_story
            report_edge_candidates(ReportEdgeCandidatesArgs) {"order_path": "order-b.json"}
            ."order_path" = "order-c.json" seen "\"fact_b\": \"f-at-b\"" in output;
        report_irony_intervals_order_path_reaches_the_answer:
            @branch_story
            report_irony_intervals(ReportIronyIntervalsArgs) {"order_path": "order-b.json"}
            ."order_path" = "order-c.json" seen "\"sc-03\"" in output;
        report_payoff_substantiation_order_path_reaches_the_answer:
            @branch_story
            report_payoff_substantiation(ReportPayoffCoverageArgs) {"order_path": "order-b.json"}
            ."order_path" = "order-c.json" seen "\"unsubstantiated\": []" in output;
        report_quest_graph_order_path_reaches_the_answer:
            @branch_story
            report_quest_graph(ReportQuestGraphArgs) {"telling": "t-quiet", "order_path": "order-b.json"}
            ."order_path" = "order-c.json" seen "\"state\": \"open\"" in output;
        report_timeline_gaps_order_path_reaches_the_answer:
            @branch_story
            {"rules-interval.json" = {"schema": "narrative-rules/v1", "rules": [{"id": "falls-after-it-rises", "predicate": "fell_on_day", "class": "interval", "right": "rose_on_day", "op": "ge", "bound": {"const": 1}}]}}
            [import_facts(atomic::FactsManifest) {"units": [{"unit_id": "day"}], "predicates": [{"predicate_id": "rose_on_day", "object_kind": "quantity", "subject_kind": "character"}, {"predicate_id": "fell_on_day", "object_kind": "quantity", "subject_kind": "character"}], "facts": [{"fact_id": "f-rose", "frame": "ground-truth", "claim": "it rose on day 5", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["e-her"], "typed": {"subject": "e-her", "predicate": "rose_on_day", "object": {"kind": "quantity", "n": 5, "unit": "day"}}}, {"fact_id": "f-fell", "frame": "ground-truth", "claim": "it fell on day 3", "canon_from": "sc-03", "evidence": ["sc-03"], "entities": ["e-her"], "typed": {"subject": "e-her", "predicate": "fell_on_day", "object": {"kind": "quantity", "n": 3, "unit": "day"}}}]}]
            report_timeline_gaps(ReportTimelineGapsArgs) {"rules_path": "rules-interval.json", "order_path": "order-b.json"}
            ."order_path" = "order-c.json" seen "\"kind\": \"violated\"" in output;
        report_frame_view_branch_reaches_the_answer:
            @branch_story
            report_frame_view(ReportFrameViewArgs) {"frame": "ground-truth", "at": "sc-03", "order_path": "order-b.json"}
            ."branch" = "b-alt" seen "\"f-alt\"" in output;
        report_frame_view_entity_reaches_the_answer:
            @branch_story
            report_frame_view(ReportFrameViewArgs) {"frame": "ground-truth", "at": "sc-03", "order_path": "order-b.json"}
            ."entity" = "e-her" seen "\"f-way\"" in output;
        report_authoring_frontier_telling_reaches_the_answer:
            @branch_story
            report_authoring_frontier(ReportAuthoringFrontierArgs) {"order_path": "order-b.json"}
            ."telling" = "t-quiet" seen "\"unresolved_quests\"" in output;
        propose_verdict_order_path_reaches_the_answer:
            @branch_story
            {"rules-directed.json" = {"schema": "narrative-rules/v1", "rules": [{"id": "one-way-only", "predicate": "at", "class": "transition", "adjacency": "at", "undirected": false}]}}
            {"return-trip.json" = {"facts": [{"fact_id": "f-back", "frame": "ground-truth", "claim": "she returns", "canon_from": "sc-03", "evidence": ["sc-03"], "entities": ["e-her", "p-a"], "supersedes_in_frame": "f-at-b", "typed": {"subject": "e-her", "predicate": "at", "object": {"kind": "entity", "id": "p-a"}}}]}}
            propose_verdict(ProposeVerdictArgs) {"manifest_path": "return-trip.json", "rules_path": "rules-directed.json", "order_path": "order-b.json"}
            ."order_path" = "order-c.json" seen "\"dangling_setups\": {}" in output;
        // THE `claim_sha256` PINS ARE THE ARTIFACT'S OWN CONTRACT, not a
        // fixture shortcut: a proposal is judged against the claim text the
        // proposer read, and an amend since then must fail loud as stale. They
        // are sha256 of the claim strings the world declares, so editing that
        // prose without editing these makes the import say `stale`, by design.
        import_typing_proposals_dry_run_reaches_the_store:
            @branch_story
            {"typing.json" = {"schema": "typing-proposals/v1", "proposals": [{"fact": "f-way-back", "typed": {"subject": "p-b", "predicate": "adjacent", "object": {"kind": "entity", "id": "p-a"}}, "claim_sha256": "a3e95230db5ab9cd352aa4aacd956c52c02f74020c8be66dcc82bf865b881c53", "rationale": "the prose names both ends of one way, so the typed leg is the other direction"}]}}
            [import_facts(atomic::FactsManifest) {"facts": [{"fact_id": "f-way-back", "frame": "ground-truth", "claim": "the way runs back as well", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["p-b", "p-a"]}]}]
            import_typing_proposals(ImportTypingProposalsArgs) {"proposals_path": "typing.json"}
            ."dry_run" = true seen "\"subject\": \"p-b\"" in store;
        import_edge_proposals_dry_run_reaches_the_store:
            @branch_story
            {"edges.json" = {"schema": "edge-proposals/v1", "conflicts": [{"fact": "f-alt", "target": "f-at-b", "fact_claim_sha256": "934b712828d0c69368eb8082c9251c5190cceb939a964b8b0bed953bb2da7e9f", "target_claim_sha256": "686eb0e864b28d0c6a918e55d5233ec3a18a9e02e030c3538bf55b14e6cfcc21", "rationale": "one places her at a and the other places her at b, in the same frame"}]}}
            import_edge_proposals(ImportEdgeProposalsArgs) {"proposals_path": "edges.json"}
            ."dry_run" = true seen "\"target\": \"f-at-b\"" in store;
        validate_render_fidelity_order_path_reaches_the_answer:
            @branch_story
            (store "blind.json" =
                [import_sections(ImportSectionsArgs) {"sections": [{"section_id": "sc-01", "parent_doc": "spec", "title": "one"}, {"section_id": "sc-02", "parent_doc": "spec", "title": "two"}, {"section_id": "sc-03", "parent_doc": "spec", "title": "three"}]}]
                [import_facts(atomic::FactsManifest) {"frames": [{"frame_id": "ground-truth"}], "entity_kinds": [{"kind_id": "place"}, {"kind_id": "character"}], "entities": [{"entity_id": "p-a", "kind": "place"}, {"entity_id": "p-b", "kind": "place"}, {"entity_id": "e-her", "kind": "character"}], "predicates": [{"predicate_id": "at", "object_kind": "entity", "subject_kind": "character", "object_entity_kind": "place"}], "facts": [{"fact_id": "r-at-a", "frame": "ground-truth", "claim": "the prose puts her at a", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["e-her", "p-a"], "typed": {"subject": "e-her", "predicate": "at", "object": {"kind": "entity", "id": "p-a"}}}, {"fact_id": "r-at-b", "frame": "ground-truth", "claim": "the prose puts her at b", "canon_from": "sc-03", "evidence": ["sc-03"], "entities": ["e-her", "p-b"], "typed": {"subject": "e-her", "predicate": "at", "object": {"kind": "entity", "id": "p-b"}}}]}]
            )
            validate_render_fidelity(RenderFidelityArgs) {"against": "blind.json", "world": "main", "order_path": "order-b.json"}
            ."order_path" = "order-c.json" seen "\"reached_terminal\": false" in output;
        // A REQUIRED ARGUMENT IS JUDGED BY TWO VALUES, WHICH THE MACRO ALREADY
        // EXPRESSES. Every case above differs by an argument being ABSENT or
        // PRESENT, and a required argument has no absent arm — but the base
        // already carries a value and the `=` supplies a second, so the two arms
        // differ by WHICH STORE the gate was pointed at. Round 1014's defect is
        // the reason to want this: `against` is required, so it sat outside the
        // optional accounting entirely while reaching `Path::new` unresolved.
        validate_render_fidelity_against_reaches_the_answer:
            @branch_story
            (store "one-fact.json" =
                [import_sections(ImportSectionsArgs) {"sections": [{"section_id": "sc-01", "parent_doc": "spec", "title": "one"}, {"section_id": "sc-02", "parent_doc": "spec", "title": "two"}, {"section_id": "sc-03", "parent_doc": "spec", "title": "three"}]}]
                [import_facts(atomic::FactsManifest) {"frames": [{"frame_id": "ground-truth"}], "entity_kinds": [{"kind_id": "place"}, {"kind_id": "character"}], "entities": [{"entity_id": "p-a", "kind": "place"}, {"entity_id": "e-her", "kind": "character"}], "predicates": [{"predicate_id": "at", "object_kind": "entity", "subject_kind": "character", "object_entity_kind": "place"}], "facts": [{"fact_id": "r-at-a", "frame": "ground-truth", "claim": "the prose puts her at a", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["e-her", "p-a"], "typed": {"subject": "e-her", "predicate": "at", "object": {"kind": "entity", "id": "p-a"}}}]}]
            )
            (store "two-facts.json" =
                [import_sections(ImportSectionsArgs) {"sections": [{"section_id": "sc-01", "parent_doc": "spec", "title": "one"}, {"section_id": "sc-02", "parent_doc": "spec", "title": "two"}, {"section_id": "sc-03", "parent_doc": "spec", "title": "three"}]}]
                [import_facts(atomic::FactsManifest) {"frames": [{"frame_id": "ground-truth"}], "entity_kinds": [{"kind_id": "place"}, {"kind_id": "character"}], "entities": [{"entity_id": "p-a", "kind": "place"}, {"entity_id": "p-b", "kind": "place"}, {"entity_id": "e-her", "kind": "character"}], "predicates": [{"predicate_id": "at", "object_kind": "entity", "subject_kind": "character", "object_entity_kind": "place"}], "facts": [{"fact_id": "r-at-a", "frame": "ground-truth", "claim": "the prose puts her at a", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["e-her", "p-a"], "typed": {"subject": "e-her", "predicate": "at", "object": {"kind": "entity", "id": "p-a"}}}, {"fact_id": "r-at-b", "frame": "ground-truth", "claim": "the prose puts her at b", "canon_from": "sc-03", "evidence": ["sc-03"], "entities": ["e-her", "p-b"], "typed": {"subject": "e-her", "predicate": "at", "object": {"kind": "entity", "id": "p-b"}}}]}]
            )
            validate_render_fidelity(RenderFidelityArgs) {"against": "one-fact.json", "world": "main", "order_path": "order-b.json"}
            ."against" = "two-facts.json" seen "\"reextracted_facts\": 2" in output;
        // THE FOUR REMAINING REQUIRED PATHS, WHICH ARE THE CLASS ROUND 1014's
        // DEFECT BELONGED TO. `against` is a required string that IS a path, and
        // being a path is exactly what the two gates keyed on the NAME `*_path`
        // could not see; the accounting now prints the declared format, and it
        // said four more arguments sit in that class. Each names a file, so each
        // case names TWO and the answer has to change.
        validate_disclosure_leak_against_reaches_the_answer:
            @branch_story
            (store "revealed-early.json" =
                [import_sections(ImportSectionsArgs) {"sections": [{"section_id": "sc-01", "parent_doc": "spec", "title": "one"}, {"section_id": "sc-02", "parent_doc": "spec", "title": "two"}, {"section_id": "sc-03", "parent_doc": "spec", "title": "three"}]}]
                [import_facts(atomic::FactsManifest) {"frames": [{"frame_id": "ground-truth"}], "entity_kinds": [{"kind_id": "place"}, {"kind_id": "character"}], "entities": [{"entity_id": "p-b", "kind": "place"}, {"entity_id": "e-her", "kind": "character"}], "predicates": [{"predicate_id": "at", "object_kind": "entity", "subject_kind": "character", "object_entity_kind": "place"}], "facts": [{"fact_id": "r-at-b", "frame": "ground-truth", "claim": "the prose already put her at b", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["e-her", "p-b"], "typed": {"subject": "e-her", "predicate": "at", "object": {"kind": "entity", "id": "p-b"}}}]}]
            )
            (store "revealed-on-time.json" =
                [import_sections(ImportSectionsArgs) {"sections": [{"section_id": "sc-01", "parent_doc": "spec", "title": "one"}, {"section_id": "sc-02", "parent_doc": "spec", "title": "two"}, {"section_id": "sc-03", "parent_doc": "spec", "title": "three"}]}]
                [import_facts(atomic::FactsManifest) {"frames": [{"frame_id": "ground-truth"}], "entity_kinds": [{"kind_id": "place"}, {"kind_id": "character"}], "entities": [{"entity_id": "p-b", "kind": "place"}, {"entity_id": "e-her", "kind": "character"}], "predicates": [{"predicate_id": "at", "object_kind": "entity", "subject_kind": "character", "object_entity_kind": "place"}], "facts": [{"fact_id": "r-at-b", "frame": "ground-truth", "claim": "the prose puts her at b where the plan does", "canon_from": "sc-03", "evidence": ["sc-03"], "entities": ["e-her", "p-b"], "typed": {"subject": "e-her", "predicate": "at", "object": {"kind": "entity", "id": "p-b"}}}]}]
            )
            [set_disclosure(SetDisclosureArgs) {"telling_id": "t-quiet", "fact_id": "f-at-b", "mode": "state", "first_at": [{"branch": "main", "coords": ["sc-03"]}]}]
            validate_disclosure_leak(DisclosureLeakArgs) {"telling": "t-quiet", "against": "revealed-early.json", "world": "main", "truth_frame": "ground-truth", "order_path": "order-b.json"}
            ."against" = "revealed-on-time.json" seen "\"leaks\": []" in output;
        propose_verdict_manifest_path_reaches_the_answer:
            @branch_story
            {"rules-directed.json" = {"schema": "narrative-rules/v1", "rules": [{"id": "one-way-only", "predicate": "at", "class": "transition", "adjacency": "at", "undirected": false}]}}
            {"return-trip.json" = {"facts": [{"fact_id": "f-back", "frame": "ground-truth", "claim": "she returns", "canon_from": "sc-03", "evidence": ["sc-03"], "entities": ["e-her", "p-a"], "supersedes_in_frame": "f-at-b", "typed": {"subject": "e-her", "predicate": "at", "object": {"kind": "entity", "id": "p-a"}}}]}}
            {"stays-put.json" = {"facts": [{"fact_id": "f-stays", "frame": "ground-truth", "claim": "she stays where she is", "canon_from": "sc-03", "evidence": ["sc-03"], "entities": ["e-her", "p-b"]}]}}
            propose_verdict(ProposeVerdictArgs) {"manifest_path": "return-trip.json", "rules_path": "rules-directed.json", "order_path": "order-b.json"}
            ."manifest_path" = "stays-put.json" seen "\"violation_count\": 2" in output;
        import_typing_proposals_proposals_path_reaches_the_store:
            @branch_story
            {"types-the-way.json" = {"schema": "typing-proposals/v1", "proposals": [{"fact": "f-way-back", "typed": {"subject": "p-b", "predicate": "adjacent", "object": {"kind": "entity", "id": "p-a"}}, "claim_sha256": "a3e95230db5ab9cd352aa4aacd956c52c02f74020c8be66dcc82bf865b881c53", "rationale": "the prose names both ends of one way"}]}}
            {"types-the-move.json" = {"schema": "typing-proposals/v1", "proposals": [{"fact": "f-she-moved", "typed": {"subject": "e-her", "predicate": "at", "object": {"kind": "entity", "id": "p-b"}}, "claim_sha256": "503b9a85fd31cd820771da6e4477cc77b67cb9c21fa13a74497556b881ac0c45", "rationale": "the prose says where she ends up"}]}}
            [import_facts(atomic::FactsManifest) {"facts": [{"fact_id": "f-way-back", "frame": "ground-truth", "claim": "the way runs back as well", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["p-b", "p-a"]}, {"fact_id": "f-she-moved", "frame": "ground-truth", "claim": "she crossed over", "canon_from": "sc-03", "evidence": ["sc-03"], "entities": ["e-her", "p-b"]}]}]
            import_typing_proposals(ImportTypingProposalsArgs) {"proposals_path": "types-the-way.json"}
            ."proposals_path" = "types-the-move.json" seen "\"subject\": \"p-b\"" in store;
        import_edge_proposals_proposals_path_reaches_the_store:
            @branch_story
            {"alt-conflicts.json" = {"schema": "edge-proposals/v1", "conflicts": [{"fact": "f-alt", "target": "f-at-b", "fact_claim_sha256": "934b712828d0c69368eb8082c9251c5190cceb939a964b8b0bed953bb2da7e9f", "target_claim_sha256": "686eb0e864b28d0c6a918e55d5233ec3a18a9e02e030c3538bf55b14e6cfcc21", "rationale": "one places her at a and the other at b, in the same frame"}]}}
            {"quest-conflicts.json" = {"schema": "edge-proposals/v1", "conflicts": [{"fact": "f-quest-done", "target": "f-alt", "fact_claim_sha256": "2496954133070e3e5795def5eae4528054f8dbdd6713dc16f01e33b4cf7d01b2", "target_claim_sha256": "934b712828d0c69368eb8082c9251c5190cceb939a964b8b0bed953bb2da7e9f", "rationale": "the crossing cannot be made by someone who never left"}]}}
            import_edge_proposals(ImportEdgeProposalsArgs) {"proposals_path": "alt-conflicts.json"}
            ."proposals_path" = "quest-conflicts.json" seen "\"target\": \"f-alt\"" in store;
        // THE LAST ONE THE SWEEP LEFT, AND ITS EXCEPTION WAS EARNED BY THE
        // TOOL'S SEMANTICS RATHER THAN BY A LIMIT. This verb writes nothing: it
        // composes a ledger row for a human to paste, so the reason appearing in
        // its answer IS the effect and the probe correctly called that
        // indistinguishable from an echo. What the probe could not ask is
        // whether the value landed in the reason SLOT — a handler that put
        // `reason` where `kind` goes would pass an echo test and fail this one,
        // because the needle is the TOML key beside its value.
        emit_publishable_override_ledger_draft_reason_reaches_the_answer:
            [append_changelog_entry(AppendChangelogEntryArgs) {"entry_id": "Round 1", "decision_summary": "the decision names Waits", "changes_bullets": ["a change"], "verification_bullets": ["a check"]}]
            [set_changelog_publishable_decision_summary(SetChangelogPublishableStringArgs) {"entry_id": "Round 1", "value": "the published line names Waits"}]
            emit_publishable_override_ledger_draft(EmitPublishableOverrideLedgerDraftArgs) {"entry_id": "Round 1", "reason": "word", "applied_in": "Round 1", "pattern": "Waits", "replacement": "lingers"}
            ."reason" = "the name is a person's" seen "reason = \\\"the name is a person's\\\"" in output;
        // THE FOUR THE PROBE SWEEP CANNOT REACH. Its value comes from the
        // schema, and these four are the shapes the schema describes no value
        // for — three closed enums behind a `$ref`, whose members serde would
        // refuse before the handler saw them, and one array of objects. Each
        // gets what the sweep could not derive: a SECOND VALID VALUE, written by
        // hand, and an answer that has to change.
        add_parameter_gate_op_reaches_the_store:
            @branch_story
            [add_parameter(AddParameterArgs) {"parameter_id": "hope"}]
            add_parameter_gate(AddParameterGateArgs) {"fact_id": "f-way", "parameter": "hope", "threshold": 2, "op": "ge"}
            ."op" = "lt" seen "\"op\": \"lt\"" in store;
        add_predicate_object_kind_reaches_the_store:
            @branch_story
            add_predicate(AddPredicateArgs) {"predicate_id": "counts", "object_kind": "quantity", "subject_kind": "place", "description": "how many"}
            ."object_kind" = "fact" seen "\"object_kind\": \"fact\"" in store;
        set_predicate_object_kind_reaches_the_store:
            @branch_story
            [add_predicate(AddPredicateArgs) {"predicate_id": "counts", "object_kind": "quantity", "subject_kind": "place", "description": "how many"}]
            set_predicate(SetPredicateArgs) {"predicate_id": "counts", "object_kind": "quantity", "description": "how many"}
            ."object_kind" = "fact" seen "\"object_kind\": \"fact\"" in store;
        import_sections_sections_reaches_the_store:
            import_sections(ImportSectionsArgs) {"sections": [{"section_id": "sc-11", "parent_doc": "spec", "title": "eleven"}]}
            ."sections" = [{"section_id": "sc-12", "parent_doc": "spec", "title": "twelve"}] seen "sc-12" in store;
        report_mutation_reasons_target_reaches_the_answer:
            @branch_story
            [add_section(AddSectionArgs) {"section_id": "sc-06", "parent_doc": "spec", "title": "six"}]
            [add_section(AddSectionArgs) {"section_id": "sc-07", "parent_doc": "spec", "title": "seven"}]
            [remove_section(RemoveSectionArgs) {"section_id": "sc-06", "reason": "the sixth scene was cut"}]
            [remove_section(RemoveSectionArgs) {"section_id": "sc-07", "reason": "the seventh scene was cut"}]
            report_mutation_reasons(ReportMutationReasonsArgs) {}
            ."target" = "sc-06" seen "the seventh scene was cut" in output;
        validate_disclosure_leak_order_path_reaches_the_answer:
            @branch_story
            (store "blind.json" =
                [import_sections(ImportSectionsArgs) {"sections": [{"section_id": "sc-01", "parent_doc": "spec", "title": "one"}, {"section_id": "sc-02", "parent_doc": "spec", "title": "two"}, {"section_id": "sc-03", "parent_doc": "spec", "title": "three"}]}]
                [import_facts(atomic::FactsManifest) {"frames": [{"frame_id": "ground-truth"}], "entity_kinds": [{"kind_id": "place"}, {"kind_id": "character"}], "entities": [{"entity_id": "p-b", "kind": "place"}, {"entity_id": "e-her", "kind": "character"}], "predicates": [{"predicate_id": "at", "object_kind": "entity", "subject_kind": "character", "object_entity_kind": "place"}], "facts": [{"fact_id": "r-at-b", "frame": "ground-truth", "claim": "the prose already put her at b", "canon_from": "sc-01", "evidence": ["sc-01"], "entities": ["e-her", "p-b"], "typed": {"subject": "e-her", "predicate": "at", "object": {"kind": "entity", "id": "p-b"}}}]}]
            )
            [set_disclosure(SetDisclosureArgs) {"telling_id": "t-quiet", "fact_id": "f-at-b", "mode": "state", "first_at": [{"branch": "main", "coords": ["sc-03"]}]}]
            validate_disclosure_leak(DisclosureLeakArgs) {"telling": "t-quiet", "against": "blind.json", "world": "main", "truth_frame": "ground-truth", "order_path": "order-c.json"}
            ."order_path" = "order-b.json" seen "\"kind\": \"early\"" in output;
    }

    /// EVERY REQUIRED `reason` AN AGENT SENDS IS IN THE STORE AFTERWARDS, AND
    /// THE POPULATION IS THE ROUTER'S RATHER THAN A LIST SOMEBODY TYPED.
    ///
    /// Round 1022 measured that ten primitives validated a mandatory `reason`
    /// non-blank and then dropped it, so an agent spent its rationale on a field
    /// that reached nothing and a later reader could not ask why the canon
    /// changed. This walks the same schema the probe sweep does, calls every
    /// tool that declares a required `reason`, and asserts the ledger holds it.
    ///
    /// DERIVED, so a tool ADDED later with a `reason` argument is covered by
    /// this on the round that adds it: the population comes from
    /// `required_arguments()`, and a tool this cannot call has to be named here
    /// with why, not silently skipped.
    #[tokio::test]
    async fn every_reason_an_agent_sends_reaches_the_ledger() {
        // The one tool whose `reason` shapes an ANSWER and writes nothing: it
        // emits a draft ledger row for a human to paste, so there is no store
        // change for a reason to accompany.
        const WRITES_NOTHING: &[&str] = &["emit_publishable_override_ledger_draft"];
        let declaring: Vec<String> = required_arguments()
            .into_iter()
            .filter(|(_, a)| a == "reason")
            .map(|(t, _)| t)
            .collect();
        assert!(
            declaring.len() > 1,
            "only {} tool(s) declare a required `reason`, so this gate is reading \
             almost nothing",
            declaring.len()
        );
        let probed: BTreeSet<&str> = PROBED.iter().map(|(t, _)| *t).collect();
        let mut unreached = Vec::new();
        for tool in &declaring {
            if WRITES_NOTHING.contains(&tool.as_str()) {
                continue;
            }
            assert!(
                probed.contains(tool.as_str()),
                "`{tool}` declares a required `reason` and has no probe \
                 declaration, so nothing here can call it"
            );
            unreached.push(tool.clone());
        }
        // The probe sweep already calls each of these with a value naming
        // nothing and requires it to be observable in the store; what this adds
        // is that the observable place is THE LEDGER, by name, rather than
        // anywhere in the file.
        let tmp = agent_workspace();
        let server = MnemosyneServer::new(tmp.path().to_path_buf()).expect("server");
        branch_story(&server, tmp.path()).await;
        let reason = "the ledger has to hold this sentence";
        let added = server
            .add_section(
                serde_json::from_value::<AddSectionArgs>(serde_json::json!({
                    "section_id": "sc-06", "parent_doc": "spec", "title": "six"
                }))
                .map(Parameters)
                .expect("shape parses"),
            )
            .await;
        assert!(added.is_error != Some(true), "adding: {:?}", added.content);
        let removed = server
            .remove_section(
                serde_json::from_value::<RemoveSectionArgs>(serde_json::json!({
                    "section_id": "sc-06", "reason": reason
                }))
                .map(Parameters)
                .expect("shape parses"),
            )
            .await;
        assert!(
            removed.is_error != Some(true),
            "the removal failed: {:?}",
            removed.content
        );
        let store: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(tmp.path().join("docs/.atomic/workspace.atomic.json"))
                .expect("read the store"),
        )
        .expect("the store parses");
        let rows = store["mutation_reasons"]
            .as_array()
            .expect("the ledger is an array")
            .clone();
        assert_eq!(
            rows.len(),
            1,
            "one reasoned mutation should leave one row: {rows:?}"
        );
        assert_eq!(rows[0]["primitive"], "remove_section");
        assert_eq!(rows[0]["target_kind"], "section");
        assert_eq!(rows[0]["target_id"], "sc-06");
        assert_eq!(rows[0]["reason"], reason);
        // THE ROW SURVIVES ITS TARGET. `sc-02` is gone from the sections
        // registry and the row that says why is still here, which is the whole
        // reason the ledger is a sequence beside the registries rather than a
        // field on the record.
        assert!(
            store["sections"].get("sc-06").is_none(),
            "the section should be gone"
        );
        // A SECOND REASONED CHANGE APPENDS RATHER THAN REPLACING — the property
        // a field on the record could not have.
        for (fact, why) in [
            ("f-quest-done", "the crossing was never made"),
            ("f-alt", "the fork is cut"),
        ] {
            let out = server
                .retract_fact(
                    serde_json::from_value::<RetractFactArgs>(serde_json::json!({
                        "fact_id": fact, "reason": why
                    }))
                    .map(Parameters)
                    .expect("shape parses"),
                )
                .await;
            assert!(
                out.is_error != Some(true),
                "retracting {fact}: {:?}",
                out.content
            );
        }
        let store: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(tmp.path().join("docs/.atomic/workspace.atomic.json"))
                .expect("read the store"),
        )
        .expect("the store parses");
        let rows = store["mutation_reasons"].as_array().expect("array").clone();
        assert_eq!(
            rows.len(),
            3,
            "three reasoned mutations, three rows: {rows:?}"
        );
        assert_eq!(
            rows.iter()
                .map(|r| r["reason"].as_str().unwrap_or_default())
                .collect::<Vec<_>>(),
            vec![reason, "the crossing was never made", "the fork is cut"],
            "the ledger is ordered by the order the acts happened"
        );
        // A BLANK REASON IS REFUSED BEFORE ANYTHING MOVES, which is where the
        // check has to be: Round 1024 first put it in the save and four atomic
        // tests said the store had already been mutated in memory.
        let blank = server
            .remove_section(
                serde_json::from_value::<RemoveSectionArgs>(serde_json::json!({
                    "section_id": "sc-03", "reason": "   "
                }))
                .map(Parameters)
                .expect("shape parses"),
            )
            .await;
        assert!(
            blank.is_error == Some(true),
            "a blank reason must be refused"
        );
        assert!(
            answer_text(&blank).contains("reason mandatory"),
            "the refusal must name what is missing: {}",
            answer_text(&blank)
        );
    }

    /// THE SURFACE THAT STATES THE TIMESTAMP CONTRACT SAYS THE FORM THAT IS
    /// ENFORCED, and cannot drift from it — both read the same const.
    ///
    /// IN PROSE AND IN THE MACHINE-READABLE HALF. The prose half is what Round
    /// 1023 shipped, and a client cannot act on it: `pattern` is the field a
    /// generated form, a validating client or a schema-aware agent actually
    /// checks against. The round that added it found this assertion missing by
    /// trying to INJECT the defect — deleting `pattern` from the property
    /// reddened nothing, because the parity test beside it proves the derived
    /// pattern is honest and says nothing about whether the schema carries it.
    #[test]
    fn the_timestamp_schema_states_the_form_the_store_enforces() {
        let tool = agent_facing_tools()
            .into_iter()
            .find(|t| t.name == "add_confirmation_event")
            .expect("the tool must be routed");
        let property = tool
            .input_schema
            .get("properties")
            .and_then(|p| p.as_object())
            .and_then(|p| p.get("timestamp"))
            .cloned()
            .expect("the tool must have a timestamp property");
        assert_eq!(
            property.get("pattern").and_then(|p| p.as_str()),
            Some(atomic::utc_instant_pattern().as_str()),
            "the `timestamp` property does not publish the pattern derived from \
             the layout the store enforces, so the only machine-readable thing \
             an agent has about this value is nothing: {property}"
        );
        let described = property
            .get("description")
            .and_then(|d| d.as_str())
            .unwrap_or_default()
            .to_string();
        assert!(
            described.contains(atomic::UTC_INSTANT_FORM),
            "the `timestamp` field does not show an agent the form the store \
             enforces (`{}`), which is the state Round 1022 found the CLI in — \
             it said `<iso>` and any string was accepted: {described}",
            atomic::UTC_INSTANT_FORM
        );
    }

    /// THE CONFIRMATION LEDGER'S IDEMPOTENCE SURVIVES A RESTAMPED CLOCK, AND ITS
    /// TIME AXIS IS A TIME.
    ///
    /// Round 1022's probe sweep found `timestamp` stored verbatim — the sentinel
    /// `mn-probe-names-nothing` went in and came back out of the store — and the
    /// CLI's own `--timestamp <iso>` had been stating the contract in prose while
    /// nothing held it. Following that found the worse half: the event id was
    /// DERIVED with the clock in it, so the idempotence the primitive documents
    /// ("a re-append of the identical act hashes to the same id and is rejected")
    /// did not hold. Measured before the repair: the same act, restamped one
    /// second later and then given a string that is not a time at all, was
    /// accepted three times and the ledger held three rows.
    ///
    /// Both halves are asserted here rather than in the primitive's own tests,
    /// because this is the wire an agent reaches them through and the argument
    /// that was unchecked is one an agent sends.
    #[tokio::test]
    async fn one_verification_act_records_once_whatever_the_clock_says() {
        let tmp = agent_workspace();
        let server = MnemosyneServer::new(tmp.path().to_path_buf()).expect("server");
        branch_story(&server, tmp.path()).await;
        // THE ACT IS WHAT WAS VERIFIED AND BY WHICH RUN; the clock is only when
        // the confirmer says it looked. Two arms that mean to test the CALENDAR
        // rather than idempotence have to be different acts, which is what
        // `run` names.
        let act = |timestamp: &str, run: &str| {
            serde_json::json!({
                "section_id": "sc-01", "confirmer_kind": "model",
                "confirmer_id": "the author", "confirmer_version": "1",
                "method": "semantic_review", "verdict": "confirm",
                "rationale": "the scene reads as written", "timestamp": timestamp,
                "authoring_run": "run-a", "confirming_run": run
            })
        };
        let call = |json: serde_json::Value| {
            let args: AddConfirmationEventArgs =
                serde_json::from_value(json).expect("the agent-facing shape parses");
            server.add_confirmation_event(Parameters(args))
        };
        let first = call(act("2026-08-04T00:00:00Z", "run-b")).await;
        assert!(
            first.is_error != Some(true),
            "a canonical UTC instant must be accepted: {:?}",
            first.content
        );
        // THE SAME ACT, ONE SECOND LATER. Nothing about what was verified
        // changed, so the ledger must not grow.
        let again = call(act("2026-08-04T00:00:01Z", "run-b")).await;
        assert!(
            again.is_error == Some(true),
            "the same verification act was recorded a SECOND time because the \
             clock moved, so the documented idempotence is keyed on when the \
             confirmer ran rather than on what it confirmed: {:?}",
            again.content
        );
        assert!(
            answer_text(&again).contains("identical act"),
            "the refusal must say it is the same act, or a caller reads it as a \
             different rejection: {}",
            answer_text(&again)
        );
        // A TIME AXIS THAT TAKES ANYTHING IS NOT A TIME AXIS. Each of these is
        // refused for a reason that names what is wrong with it.
        for (timestamp, why) in [
            ("not-a-time-at-all", "20 characters"),
            ("2026-08-04T00:00:00+09:00", "20 characters"),
            ("2026-08-04T00:00:00.5Z", "20 characters"),
            ("2026-13-04T00:00:00Z", "month 13"),
            ("2026-02-30T00:00:00Z", "day 30"),
            ("2026-08-04T24:00:00Z", "hour 24"),
            ("2026-08-04 00:00:00Z", "`T` at position 10"),
        ] {
            let refused = call(act(timestamp, "run-b")).await;
            assert!(
                refused.is_error == Some(true),
                "`{timestamp}` was accepted as a timestamp: {:?}",
                refused.content
            );
            let said = answer_text(&refused);
            assert!(
                said.contains(why),
                "`{timestamp}` was refused without saying `{why}`, so the caller \
                 is not told what to fix: {said}"
            );
        }
        // A LEAP DAY IS A REAL DAY, so the calendar check has to know the year.
        let leap = call(act("2028-02-29T00:00:00Z", "run-c")).await;
        assert!(
            leap.is_error != Some(true),
            "2028 is a leap year and 29 February is a real day: {:?}",
            leap.content
        );
        let not_leap = call(act("2026-02-29T00:00:00Z", "run-d")).await;
        assert!(
            not_leap.is_error == Some(true),
            "2026 is not a leap year, so 29 February is not a day in it"
        );
        // TWO ROWS FOR TWO ACTS. `run-b` recorded once however many times its
        // clock moved, and `run-c` is a second, genuinely distinct verification
        // run — which is exactly the distinction the id keeps now that the clock
        // is out of it.
        let store: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(tmp.path().join("docs/.atomic/workspace.atomic.json"))
                .expect("read the store"),
        )
        .expect("the store parses");
        assert_eq!(
            store["confirmation_events"]
                .as_object()
                .map(|o| o.len())
                .unwrap_or(0),
            2,
            "the ledger should hold one row per verification RUN — `run-b` once \
             despite three clocks, and `run-c` once"
        );
    }

    probed! {
        add_section_probed:
            @branch_story
            add_section(AddSectionArgs) {"section_id": "sc-04", "parent_doc": "spec", "title": "four"};
        set_section_intent_probed:
            @branch_story
            set_section_intent(SetSectionIntentArgs) {"section_id": "sc-01", "intent": "what it is for"};
        set_section_title_probed:
            @branch_story
            set_section_title(SetSectionTitleArgs) {"section_id": "sc-01", "title": "a new title"};
        add_section_caveat_probed:
            @branch_story
            add_section_caveat(AddSectionCaveatArgs) {"section_id": "sc-01", "bullet": "a caveat"};
        add_entity_probed:
            @branch_story
            add_entity(AddEntityArgs) {"entity_id": "e-him", "kind": "character"};
        add_entity_kind_probed:
            @branch_story
            add_entity_kind(AddEntityKindArgs) {"kind_id": "thing"};
        add_frame_probed:
            @branch_story
            add_frame(AddFrameArgs) {"frame_id": "he-believes"};
        add_branch_probed:
            @branch_story
            add_branch(AddBranchArgs) {"branch_id": "b-third", "forks_from": "main", "forks_at": "sc-02"};
        add_fact_count_probed:
            @branch_story
            add_fact_count(AddFactCountArgs) {"fact_id": "f-way", "count": 3};
        retract_fact_probed:
            @branch_story
            retract_fact(RetractFactArgs) {"fact_id": "f-quest-done", "reason": "the crossing was never made"};
        report_entity_probed:
            @branch_story
            report_entity(ReportEntityArgs) {"entity_id": "e-her"};
        query_section_probed:
            @branch_story
            query_section(QuerySectionArgs) {"section_id": "sc-01"};
        report_frame_view_probed:
            @branch_story
            report_frame_view(ReportFrameViewArgs) {"frame": "ground-truth", "at": "sc-03", "order_path": "order-b.json"};
        report_quest_graph_probed:
            @branch_story
            report_quest_graph(ReportQuestGraphArgs) {"telling": "t-quiet", "order_path": "order-b.json"};
        report_disclosure_coverage_probed:
            @branch_story
            report_disclosure_coverage(ReportDisclosureCoverageArgs) {"telling": "t-quiet"};
        set_disclosure_probed:
            @branch_story
            set_disclosure(SetDisclosureArgs) {"telling_id": "t-quiet", "fact_id": "f-at-b", "mode": "state"};
        append_changelog_entry_probed:
            append_changelog_entry(AppendChangelogEntryArgs) {"entry_id": "Round 1", "decision_summary": "the decision", "changes_bullets": ["a change"], "verification_bullets": ["a check"]};
        query_changelog_entry_probed:
            [append_changelog_entry(AppendChangelogEntryArgs) {"entry_id": "Round 1", "decision_summary": "the decision", "changes_bullets": ["a change"], "verification_bullets": ["a check"]}]
            query_changelog_entry(QueryChangelogEntryArgs) {"entry_id": "Round 1"};
        set_section_parent_doc_probed:
            @branch_story
            set_section_parent_doc(SetSectionParentDocArgs) {"section_id": "sc-01", "parent_doc": "other"};
        set_section_rationale_probed:
            @branch_story
            set_section_rationale(SetSectionBulletsArgs) {"section_id": "sc-01", "bullets": ["because of this"]};
        set_section_alternatives_probed:
            @branch_story
            set_section_alternatives(SetSectionAlternativesArgs) {"section_id": "sc-01", "alternatives": [{"alternative": "the other way", "reason": "it costs more"}]};
        set_section_inputs_probed:
            @branch_story
            set_section_inputs(SetSectionBulletsArgs) {"section_id": "sc-01", "bullets": ["what comes in"]};
        set_section_outputs_probed:
            @branch_story
            set_section_outputs(SetSectionBulletsArgs) {"section_id": "sc-01", "bullets": ["what goes out"]};
        set_section_impact_scope_probed:
            @branch_story
            set_section_impact_scope(SetImpactScopeArgs) {"section_id": "sc-01", "refs": ["sc-02"]};
        set_section_parent_section_probed:
            @branch_story
            set_section_parent_section(SetSectionParentSectionArgs) {"section_id": "sc-02", "parent": "sc-01"};
        set_section_decision_status_probed:
            @branch_story
            set_section_decision_status(SetSectionDecisionStatusArgs) {"section_id": "sc-01", "status": "active"}
            when "superseding" {"status": "superseded", "superseding": "sc-02"}
            when "resolving" {"status": "open", "resolving": "sc-02"};
        set_section_coverage_expectation_probed:
            @branch_story
            set_section_coverage_expectation(SetSectionCoverageExpectationArgs) {"section_id": "sc-01", "expectation": "out_of_scope_here", "reason": "the scene is written"};
        set_section_verification_expectation_probed:
            @branch_story
            set_section_verification_expectation(SetSectionVerificationExpectationArgs) {"section_id": "sc-01", "expectation": "by_construction", "reason": "the scene is checked"};
        add_section_example_probed:
            @branch_story
            add_section_example(AddSectionExampleArgs) {"section_id": "sc-01", "code": "the sample", "language": "text"};
        add_section_binding_probed:
            @branch_story
            add_section_binding(AddSectionBindingArgs) {"section_id": "sc-01", "file": "src/lib.rs", "kind": "implements"};
        set_section_binding_kind_probed:
            @branch_story
            [add_section_binding(AddSectionBindingArgs) {"section_id": "sc-01", "file": "src/lib.rs", "kind": "implements"}]
            set_section_binding_kind(SetSectionBindingKindArgs) {"section_id": "sc-01", "file": "src/lib.rs", "kind": "verifies", "reason": "the binding is a check"};
        remove_section_binding_probed:
            @branch_story
            [add_section_binding(AddSectionBindingArgs) {"section_id": "sc-01", "file": "src/lib.rs", "kind": "implements"}]
            remove_section_binding(RemoveSectionBindingArgs) {"section_id": "sc-01", "file": "src/lib.rs", "reason": "the symbol moved"};
        remove_section_probed:
            @branch_story
            [add_section(AddSectionArgs) {"section_id": "sc-05", "parent_doc": "spec", "title": "five"}]
            remove_section(RemoveSectionArgs) {"section_id": "sc-05", "reason": "the scene was cut"};
        set_changelog_publishable_decision_summary_probed:
            [append_changelog_entry(AppendChangelogEntryArgs) {"entry_id": "Round 1", "decision_summary": "the decision", "changes_bullets": ["a change"], "verification_bullets": ["a check"]}]
            set_changelog_publishable_decision_summary(SetChangelogPublishableStringArgs) {"entry_id": "Round 1", "value": "the published line"};
        set_changelog_publishable_changes_probed:
            [append_changelog_entry(AppendChangelogEntryArgs) {"entry_id": "Round 1", "decision_summary": "the decision", "changes_bullets": ["a change"], "verification_bullets": ["a check"]}]
            set_changelog_publishable_changes(SetChangelogPublishableBulletsArgs) {"entry_id": "Round 1", "bullets": ["the published change"]};
        set_changelog_publishable_verification_probed:
            [append_changelog_entry(AppendChangelogEntryArgs) {"entry_id": "Round 1", "decision_summary": "the decision", "changes_bullets": ["a change"], "verification_bullets": ["a check"]}]
            set_changelog_publishable_verification(SetChangelogPublishableBulletsArgs) {"entry_id": "Round 1", "bullets": ["the published check"]};
        set_changelog_publishable_carry_forward_probed:
            [append_changelog_entry(AppendChangelogEntryArgs) {"entry_id": "Round 1", "decision_summary": "the decision", "changes_bullets": ["a change"], "verification_bullets": ["a check"]}]
            set_changelog_publishable_carry_forward(SetChangelogPublishableBulletsArgs) {"entry_id": "Round 1", "bullets": ["the published carry"]};
        set_changelog_publishable_impact_refs_probed:
            [append_changelog_entry(AppendChangelogEntryArgs) {"entry_id": "Round 1", "decision_summary": "the decision", "changes_bullets": ["a change"], "verification_bullets": ["a check"]}]
            set_changelog_publishable_impact_refs(SetChangelogPublishableBulletsArgs) {"entry_id": "Round 1", "bullets": ["sc-01"]};
        query_term_probed:
            [append_changelog_entry(AppendChangelogEntryArgs) {"entry_id": "Round 1", "decision_summary": "the decision names Waits", "changes_bullets": ["a change"], "verification_bullets": ["a check"]}]
            query_term(QueryTermArgs) {"pattern": "Waits"};
        redact_term_probed:
            [append_changelog_entry(AppendChangelogEntryArgs) {"entry_id": "Round 1", "decision_summary": "the decision names Waits", "changes_bullets": ["a change"], "verification_bullets": ["a check"]}]
            redact_term(RedactTermArgs) {"pattern": "Waits", "replacement": "lingers", "reason": "word", "applied_in": "Round 1", "dry_run": false};
        emit_publishable_override_ledger_draft_probed:
            [append_changelog_entry(AppendChangelogEntryArgs) {"entry_id": "Round 1", "decision_summary": "the decision names Waits", "changes_bullets": ["a change"], "verification_bullets": ["a check"]}]
            [set_changelog_publishable_decision_summary(SetChangelogPublishableStringArgs) {"entry_id": "Round 1", "value": "the published line names Waits"}]
            emit_publishable_override_ledger_draft(EmitPublishableOverrideLedgerDraftArgs) {"entry_id": "Round 1", "reason": "word", "applied_in": "Round 1", "pattern": "Waits", "replacement": "lingers"}
            except "reason";
        add_fact_probed:
            @branch_story
            add_fact(atomic::FactImport) {"fact_id": "f-new", "frame": "ground-truth", "claim": "the lamp is lit", "canon_from": "sc-01", "evidence": ["sc-01"]};
        amend_fact_probed:
            @branch_story
            amend_fact(AmendFactArgs) {"fact_id": "f-way", "frame": "ground-truth", "claim": "a way runs, narrow", "canon_from": "sc-01", "evidence": ["sc-01"], "reason": "the prose says narrow"};
        add_fact_conflict_probed:
            @branch_story
            add_fact_conflict(AddFactConflictArgs) {"fact_id": "f-at-a", "conflicts_with": "f-quest-done"};
        remove_fact_count_probed:
            @branch_story
            [add_fact_count(AddFactCountArgs) {"fact_id": "f-way", "count": 3}]
            remove_fact_count(RemoveFactCountArgs) {"fact_id": "f-way"};
        add_unit_probed:
            @branch_story
            add_unit(AddUnitArgs) {"unit_id": "minute"};
        add_edge_cost_probed:
            @branch_story
            [add_unit(AddUnitArgs) {"unit_id": "minute"}]
            add_edge_cost(AddEdgeCostArgs) {"fact_id": "f-way", "n": 5, "unit": "minute"};
        remove_edge_cost_probed:
            @branch_story
            [add_unit(AddUnitArgs) {"unit_id": "minute"}]
            [add_edge_cost(AddEdgeCostArgs) {"fact_id": "f-way", "n": 5, "unit": "minute"}]
            remove_edge_cost(RemoveEdgeCostArgs) {"fact_id": "f-way"};
        add_edge_guard_probed:
            @branch_story
            add_edge_guard(AddEdgeGuardArgs) {"fact_id": "f-way", "condition": "f-quest-given"};
        remove_edge_guard_condition_probed:
            @branch_story
            [add_edge_guard(AddEdgeGuardArgs) {"fact_id": "f-way", "condition": "f-quest-given"}]
            remove_edge_guard_condition(RemoveEdgeGuardConditionArgs) {"fact_id": "f-way", "condition": "f-quest-given"};
        remove_edge_guard_probed:
            @branch_story
            [add_edge_guard(AddEdgeGuardArgs) {"fact_id": "f-way", "condition": "f-quest-given"}]
            remove_edge_guard(RemoveEdgeGuardArgs) {"fact_id": "f-way"};
        set_edge_guard_threshold_probed:
            @branch_story
            [add_edge_guard(AddEdgeGuardArgs) {"fact_id": "f-way", "condition": "f-quest-given"}]
            [add_edge_guard(AddEdgeGuardArgs) {"fact_id": "f-way", "condition": "f-at-a"}]
            set_edge_guard_threshold(SetEdgeGuardThresholdArgs) {"fact_id": "f-way", "threshold": 1};
        add_parameter_probed:
            @branch_story
            add_parameter(AddParameterArgs) {"parameter_id": "hope"};
        add_parameter_delta_probed:
            @branch_story
            [add_parameter(AddParameterArgs) {"parameter_id": "hope"}]
            add_parameter_delta(AddParameterDeltaArgs) {"fact_id": "f-way", "parameter": "hope", "delta": 2};
        remove_parameter_delta_probed:
            @branch_story
            [add_parameter(AddParameterArgs) {"parameter_id": "hope"}]
            [add_parameter_delta(AddParameterDeltaArgs) {"fact_id": "f-way", "parameter": "hope", "delta": 2}]
            remove_parameter_delta(RemoveParameterDeltaArgs) {"fact_id": "f-way", "parameter": "hope"};
        add_parameter_gate_probed:
            @branch_story
            [add_parameter(AddParameterArgs) {"parameter_id": "hope"}]
            add_parameter_gate(AddParameterGateArgs) {"fact_id": "f-way", "parameter": "hope", "threshold": 2, "op": "ge"};
        remove_parameter_gate_probed:
            @branch_story
            [add_parameter(AddParameterArgs) {"parameter_id": "hope"}]
            [add_parameter_gate(AddParameterGateArgs) {"fact_id": "f-way", "parameter": "hope", "threshold": 2, "op": "ge"}]
            remove_parameter_gate(RemoveParameterGateArgs) {"fact_id": "f-way"};
        add_predicate_probed:
            @branch_story
            add_predicate(AddPredicateArgs) {"predicate_id": "beside", "object_kind": "entity", "subject_kind": "place", "object_entity_kind": "place", "description": "one place beside another"}
            when "object_kind" {"object_entity_kind": null, "object_tokens": ["quiet"]};
        set_predicate_probed:
            @branch_story
            // ON A PREDICATE NOTHING USES YET. Re-typing one that facts already
            // hold objects for is refused BY DESIGN, so probing the object shape
            // through `adjacent` measured that guard rather than the argument.
            [add_predicate(AddPredicateArgs) {"predicate_id": "beside", "object_kind": "entity", "subject_kind": "place", "object_entity_kind": "place", "description": "one place beside another"}]
            set_predicate(SetPredicateArgs) {"predicate_id": "beside", "object_kind": "entity", "description": "a way between two places"}
            when "object_kind" {"object_entity_kind": null, "object_tokens": ["quiet"]};
        remove_predicate_probed:
            @branch_story
            [add_predicate(AddPredicateArgs) {"predicate_id": "beside", "object_kind": "entity", "subject_kind": "place", "object_entity_kind": "place", "description": "one place beside another"}]
            remove_predicate(RemovePredicateArgs) {"predicate_id": "beside"};
        remove_entity_kind_probed:
            @branch_story
            [add_entity_kind(AddEntityKindArgs) {"kind_id": "thing"}]
            remove_entity_kind(RemoveEntityKindArgs) {"kind_id": "thing"};
        set_entity_kind_parents_probed:
            @branch_story
            [add_entity_kind(AddEntityKindArgs) {"kind_id": "thing"}]
            set_entity_kind_parents(SetEntityKindParentsArgs) {"kind_id": "place", "parents": ["thing"]};
        add_disclosure_plan_probed:
            @branch_story
            add_disclosure_plan(AddDisclosurePlanArgs) {"telling_id": "t-loud", "default_mode": "hint"};
        add_disclosure_reveal_coord_probed:
            @branch_story
            [set_disclosure(SetDisclosureArgs) {"telling_id": "t-quiet", "fact_id": "f-at-b", "mode": "withhold", "first_at": [{"branch": "main", "coords": ["sc-03"]}]}]
            add_disclosure_reveal_coord(AddDisclosureRevealCoordArgs) {"telling_id": "t-quiet", "fact_id": "f-at-b", "branch": "main", "coord": "sc-02"};
        remove_disclosure_reveal_coord_probed:
            @branch_story
            [set_disclosure(SetDisclosureArgs) {"telling_id": "t-quiet", "fact_id": "f-at-b", "mode": "withhold", "first_at": [{"branch": "main", "coords": ["sc-03", "sc-02"]}]}]
            remove_disclosure_reveal_coord(RemoveDisclosureRevealCoordArgs) {"telling_id": "t-quiet", "fact_id": "f-at-b", "branch": "main", "coord": "sc-02"};
        set_disclosure_reveal_threshold_probed:
            @branch_story
            [set_disclosure(SetDisclosureArgs) {"telling_id": "t-quiet", "fact_id": "f-at-b", "mode": "withhold", "first_at": [{"branch": "main", "coords": ["sc-03", "sc-02"]}]}]
            set_disclosure_reveal_threshold(SetDisclosureRevealThresholdArgs) {"telling_id": "t-quiet", "fact_id": "f-at-b", "branch": "main", "threshold": 1};
        remove_disclosure_probed:
            @branch_story
            [set_disclosure(SetDisclosureArgs) {"telling_id": "t-quiet", "fact_id": "f-at-b", "mode": "state"}]
            remove_disclosure(RemoveDisclosureArgs) {"telling_id": "t-quiet", "fact_id": "f-at-b", "reason": "the telling says it plainly"};
        add_inventory_entry_probed:
            add_inventory_entry(AddInventoryEntryArgs) {"inventory_id": "inv-lamp", "status": "active"};
        set_inventory_status_probed:
            [add_inventory_entry(AddInventoryEntryArgs) {"inventory_id": "inv-lamp", "status": "active"}]
            set_inventory_status(SetInventoryStatusArgs) {"inventory_id": "inv-lamp", "status": "deprecated"};
        set_inventory_section_ref_probed:
            @branch_story
            [add_inventory_entry(AddInventoryEntryArgs) {"inventory_id": "inv-lamp", "status": "active"}]
            set_inventory_section_ref(SetInventorySectionRefArgs) {"inventory_id": "inv-lamp", "section_ref": "sc-01"};
        remove_inventory_entry_probed:
            [add_inventory_entry(AddInventoryEntryArgs) {"inventory_id": "inv-lamp", "status": "active"}]
            remove_inventory_entry(RemoveInventoryEntryArgs) {"inventory_id": "inv-lamp", "reason": "the lamp was never carried"};
        query_inventory_probed:
            [add_inventory_entry(AddInventoryEntryArgs) {"inventory_id": "inv-lamp", "status": "active"}]
            query_inventory(InventoryIdArgs) {"inventory_id": "inv-lamp"};
        report_playable_world_probed:
            @branch_story
            report_playable_world(ReportPlayableWorldArgs) {"telling": "t-quiet", "order_path": "order-b.json"};
        add_confirmation_event_probed:
            @branch_story
            add_confirmation_event(AddConfirmationEventArgs) {"section_id": "sc-01", "confirmer_kind": "model", "confirmer_id": "the author", "confirmer_version": "1", "method": "semantic_review", "verdict": "confirm", "rationale": "the scene reads as written", "timestamp": "2026-08-04T00:00:00Z", "authoring_run": "run-a", "confirming_run": "run-b"};
        validate_disclosure_leak_probed:
            @branch_story
            (store "blind.json" =
                [import_sections(ImportSectionsArgs) {"sections": [{"section_id": "sc-01", "parent_doc": "spec", "title": "one"}, {"section_id": "sc-02", "parent_doc": "spec", "title": "two"}, {"section_id": "sc-03", "parent_doc": "spec", "title": "three"}]}]
                [import_facts(atomic::FactsManifest) {"frames": [{"frame_id": "ground-truth"}], "entity_kinds": [{"kind_id": "place"}, {"kind_id": "character"}], "entities": [{"entity_id": "p-b", "kind": "place"}, {"entity_id": "e-her", "kind": "character"}], "predicates": [{"predicate_id": "at", "object_kind": "entity", "subject_kind": "character", "object_entity_kind": "place"}], "facts": [{"fact_id": "r-at-b", "frame": "ground-truth", "claim": "the prose puts her at b", "canon_from": "sc-03", "evidence": ["sc-03"], "entities": ["e-her", "p-b"], "typed": {"subject": "e-her", "predicate": "at", "object": {"kind": "entity", "id": "p-b"}}}]}]
            )
            [set_disclosure(SetDisclosureArgs) {"telling_id": "t-quiet", "fact_id": "f-at-b", "mode": "state", "first_at": [{"branch": "main", "coords": ["sc-03"]}]}]
            validate_disclosure_leak(DisclosureLeakArgs) {"telling": "t-quiet", "against": "blind.json", "world": "main", "truth_frame": "ground-truth", "order_path": "order-b.json"};
        validate_render_fidelity_probed:
            @branch_story
            (store "blind.json" =
                [import_sections(ImportSectionsArgs) {"sections": [{"section_id": "sc-01", "parent_doc": "spec", "title": "one"}, {"section_id": "sc-02", "parent_doc": "spec", "title": "two"}, {"section_id": "sc-03", "parent_doc": "spec", "title": "three"}]}]
                [import_facts(atomic::FactsManifest) {"frames": [{"frame_id": "ground-truth"}], "entity_kinds": [{"kind_id": "place"}, {"kind_id": "character"}], "entities": [{"entity_id": "p-b", "kind": "place"}, {"entity_id": "e-her", "kind": "character"}], "predicates": [{"predicate_id": "at", "object_kind": "entity", "subject_kind": "character", "object_entity_kind": "place"}], "facts": [{"fact_id": "r-at-b", "frame": "ground-truth", "claim": "the prose puts her at b", "canon_from": "sc-03", "evidence": ["sc-03"], "entities": ["e-her", "p-b"], "typed": {"subject": "e-her", "predicate": "at", "object": {"kind": "entity", "id": "p-b"}}}]}]
            )
            validate_render_fidelity(RenderFidelityArgs) {"against": "blind.json", "world": "main", "order_path": "order-b.json"};
        import_sections_probed:
            import_sections(ImportSectionsArgs) {"sections": [{"section_id": "sc-01", "parent_doc": "spec", "title": "one"}]};
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
