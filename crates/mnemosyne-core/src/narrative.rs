//! Layer-0 narrative fact model — multi-axis (perspectival) facts.
//!
//! ARCHITECTURE.md sec 1.1 (Round 391): facts are multi-axis — the
//! actual/historical fact and each agent's understood-fact are DISTINCT facts
//! on distinct axes, both true, never cross-validated. A [`NarrativeFact`] is
//! one such fact: a claim held within exactly one epistemic [`Frame`] over a
//! canon-time extent. Frames are sparse and non-privileged (`ground-truth` is
//! a registry entry like any other); the absence of a fact in a frame means
//! *unrecorded*, never *false*.
//!
//! Medium-neutral by construction (ARCHITECTURE.md sec 6 invariant 4): a
//! frame, a claim, canon coordinates (structure-section refs), and evidence
//! refs exist for a novel, a TRPG sourcebook, or a spec consumer alike —
//! nothing fiction-shaped lives here, mirroring how [`crate::SectionSkeleton`]
//! carries only medium-neutral scalars. The canon coordinate is a
//! structure-section id (the medium's discourse order); a story-time axis is a
//! recorded revisit trigger, deliberately not pre-built.
//!
//! Frame-divergence queries ("facts of frame F on branch B at canon point
//! T") are read-side projections. At index-materialization scale the
//! WORLD-LINE branch (Round 433) maps onto the bitemporal/branch KV's branch
//! dimension ([`crate::FactKey`] already carries `branch_id` + `valid_from`
//! for exactly this projection); the epistemic frame is a separate scope key
//! beside it — per-branch ground truth is the `(branch, frame)` composite
//! (design sec 7.9 axis 2 superseded the pre-branch reading that mapped the
//! frame itself onto that dimension). The JSON log stays the SSOT and
//! carries every field the index will ever need.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

/// The default world-line branch every fact belongs to unless authored onto
/// another (design sec 7.9 axis 2: world-line branch ≠ epistemic frame —
/// branch answers "which quest-path/playthrough world", frame answers "who
/// believes it"; per-branch ground truth is the `(branch, frame)` composite).
/// A single-world corpus never names a branch and its store bytes never
/// change.
pub const MAIN_BRANCH: &str = "main";

fn default_branch() -> String {
    MAIN_BRANCH.to_string()
}

fn is_main_branch(branch: &str) -> bool {
    branch == MAIN_BRANCH
}

/// One epistemic frame (registry entry). Keyed by frame id in
/// `AtomicStore.frames`; the id is the value every [`NarrativeFact::frame`]
/// must reference (fail-loud at the mutate primitive).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Frame {
    /// Free-form description of whose epistemic frame this is (e.g. a
    /// character, a faction, the ground-truth axis). Optional prose, not
    /// load-bearing.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

/// One world-line branch (registry entry, Round 436). Keyed by branch id in
/// `AtomicStore.branches`; every non-default [`NarrativeFact::branch`] must
/// reference a registered id (fail-loud at the mutate primitive, symmetric
/// with the frames registry — a write-side typo must not silently create a
/// world). [`MAIN_BRANCH`] is known by construction (it is the default axis
/// value) and is never registered.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Branch {
    /// Free-form description of which quest-path/playthrough world this is.
    /// Optional prose, not load-bearing.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Where this world-line diverged (Round 438). `None` = a standalone
    /// world sharing no history (the pre-fork R433 semantics, preserved
    /// exactly). `Some` = this branch inherits the parent world's facts up
    /// to (and including) the fork point: a fact on an ancestor branch is
    /// visible here iff its `canon_from` is at or before the point where
    /// this lineage departed that ancestor. Immutable after registration
    /// (divergent-reject), and the parent must already be registered — so
    /// fork ancestry is a forest by construction, no cycle is writable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forks_from: Option<BranchFork>,
    /// Incoming world-line merges (Round 532 — convergence / confluence, the
    /// inverse of `forks_from`). Empty = not a confluence (the forest case,
    /// byte-stable). Non-empty = this world-line is the SHARED CONTINUATION
    /// that the listed parents converge INTO; each entry is a [`BranchFork`]
    /// `{branch, at}` naming a parent + the parent's merge coordinate (the
    /// scene on the parent where it joins this continuation). A merge has ≥ 2
    /// parents (a 1-parent "merge" is just a fork). A branch is EITHER a
    /// fork-child (`forks_from`) XOR a confluence (`converges_from`), never
    /// both — enforced at the mutate primitive. The merge is acyclic by the
    /// same forest guard: every parent must already be registered, so a parent
    /// cannot be this branch's own descendant.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub converges_from: Vec<BranchFork>,
}

/// The divergence coordinate of a forked world-line (Round 438): the parent
/// branch and the canon point (structure-section ref) where the child's
/// history departs it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BranchFork {
    /// Parent world-line (`MAIN_BRANCH` or a registered branch).
    pub branch: String,
    /// Canon point of divergence — facts on the parent starting at or
    /// before this point are part of this world's inherited history.
    pub at: String,
}

/// Walk a branch's fork lineage toward the root (Round 440 — THE single
/// fork-chain traversal; the write path, the continuity gate, and the
/// frame-at-T projection all route here so the subtle termination logic
/// cannot drift). Returns `(ancestor, departure_point)` pairs
/// nearest-first. The forest is write-guaranteed (parents pre-exist, forks
/// are immutable after registration); the hop cap fails loud on an
/// out-of-band cyclic edit instead of looping.
pub fn fork_chain(
    branches: &BTreeMap<String, Branch>,
    branch: &str,
) -> Result<Vec<(String, String)>, String> {
    let mut chain = Vec::new();
    let mut current = branch.to_string();
    for _ in 0..=branches.len() {
        let Some(fork) = branches.get(&current).and_then(|b| b.forks_from.clone()) else {
            return Ok(chain);
        };
        chain.push((fork.branch.clone(), fork.at));
        current = fork.branch;
    }
    Err(format!(
        "branch fork ancestry of `{branch}` exceeds the registry size — cyclic out-of-band edit"
    ))
}

/// Walk a world-line's FORWARD confluence-suffix closure (Round 533 — the
/// forward dual of [`fork_chain`]). Given a query world `world`, returns every
/// confluence branch `C` such that `world` is, transitively, one of `C`'s
/// converging parents (`world ∈ C.converges_from`, or `world` reaches such a
/// `C` through a chain of confluences). These are the SHARED continuations
/// `world` flows INTO past a merge — a fact authored once on `C` is part of
/// `world`'s world-line (the inverse of fork inheritance: a fork CHILD inherits
/// its parent's prefix; a converging PARENT inherits the confluence's suffix).
/// Sorted, deduplicated. Termination needs no hop cap: each confluence enters
/// the result set at most once (`BTreeSet` dedup), and only a fresh insert
/// re-expands — so the walk is bounded by the registry size by construction,
/// the dual of `fork_chain`'s linear-chain cap (a chain is not deduplicated, so
/// it needs the explicit guard; a deduplicated frontier cannot loop).
pub fn forward_confluences(branches: &BTreeMap<String, Branch>, world: &str) -> Vec<String> {
    // Reverse adjacency: parent branch -> the confluences converging FROM it.
    // (`converges_from` points child->parents; the forward walk needs
    // parent->children.)
    let mut downstream: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (id, b) in branches {
        for parent in &b.converges_from {
            downstream
                .entry(parent.branch.as_str())
                .or_default()
                .push(id.as_str());
        }
    }
    let mut out: BTreeSet<String> = BTreeSet::new();
    let mut frontier: Vec<String> = vec![world.to_string()];
    while let Some(cur) = frontier.pop() {
        let Some(confluences) = downstream.get(cur.as_str()) else {
            continue;
        };
        for &c in confluences {
            if out.insert(c.to_string()) {
                frontier.push(c.to_string());
            }
        }
    }
    out.into_iter().collect()
}

/// One recorded conflict assertion (Round 439): the judged target plus a
/// content pin of the target's claim AT JUDGMENT TIME. The hash is computed
/// by the mutate primitive, never caller-supplied (the R404 pattern) — so
/// when `amend_fact` later changes the target's claim, the stale pin is
/// detectable offline and the scan demands re-affirmation instead of
/// silently gating on a judgment about text that no longer exists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConflictAssertion {
    /// The fact this claim was judged to contradict.
    pub target: String,
    /// sha256 of the target's `claim` when the judgment was recorded.
    pub target_claim_sha256: String,
}

/// One narrative entity (registry entry, Round 437 — design sec 7.10 gap 4).
/// Keyed by entity id in `AtomicStore.entities`; every
/// [`NarrativeFact::entities`] ref must name a registered id (fail-loud at
/// the mutate primitive — the frames/branches registry symmetry). The
/// entity is the retrieval key for "all facts about X" (a character's
/// background, a location's lore, an item's chain of custody) and the
/// `entity_id` seat of the convergence-B index key
/// `(branch_id, entity_id, valid_from)`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entity {
    /// Free-form kind tag (consumer-defined values — e.g. a game medium
    /// uses character/location/faction/item; nothing medium-shaped is
    /// enforced here, ARCHITECTURE.md sec 6 invariant 4). Optional.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub kind: String,
    /// Free-form description. Optional prose, not load-bearing.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

/// Declared object shape of a [`Predicate`] (Round 446, design sec 7.12).
/// `Entity` = the object leg names a registered entity (locations, custody
/// targets); `Scalar` = the object leg is a consumer-vocabulary value
/// string (`alive`, `undead` — opaque data, never enumerated here:
/// ARCHITECTURE.md sec 6 invariant 4). The builder checks the typed leg's
/// object against this declaration — a shape mismatch is a write-time
/// reject, not a scan finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PredicateObjectKind {
    Entity,
    Scalar,
}

impl PredicateObjectKind {
    /// Canonical lowercase label (matches the serde representation).
    pub fn as_str(self) -> &'static str {
        match self {
            PredicateObjectKind::Entity => "entity",
            PredicateObjectKind::Scalar => "scalar",
        }
    }

    /// Parse the canonical lowercase tag back to a value. `None` for any
    /// other string (fail-loud at the caller; no silent default).
    pub fn from_tag(s: &str) -> Option<Self> {
        match s {
            "entity" => Some(PredicateObjectKind::Entity),
            "scalar" => Some(PredicateObjectKind::Scalar),
            _ => None,
        }
    }
}

/// One predicate (registry entry, Round 446 — the FOURTH registry, design
/// sec 7.12). Keyed by predicate id in `AtomicStore.predicates`; every
/// [`TypedClaim::predicate`] must reference a registered id. Predicates are
/// LOAD-BEARING refs — narrative rules key off them, so a typo'd predicate
/// would silently escape its rule (the R436 write-side-typo lesson) —
/// hence the same fail-loud registry contract as frames/branches/entities.
/// Contrast: [`Entity::kind`] stays free-form BECAUSE it is not
/// load-bearing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Predicate {
    /// Declared object shape; the builder enforces it on every typed leg.
    pub object_kind: PredicateObjectKind,
    /// Free-form description. Optional prose, not load-bearing.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

/// The object leg of a [`TypedClaim`] — two-shaped by real data (design
/// sec 7.12): locations/custody objects are entities; state values are
/// consumer-vocabulary scalars. Serde-tagged, no stringly union.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TypedObject {
    /// A registered entity id (must also be a member of the owning fact's
    /// `entities` list — the entities list stays THE retrieval key).
    Entity { id: String },
    /// An opaque consumer-vocabulary value (`alive`, `undead`, …). Never
    /// enumerated by the substrate.
    Value { value: String },
}

impl TypedObject {
    /// Resolve the flattened two-field arg surface (CLI `--typed-object-*`
    /// flags, MCP `object_entity`/`object_value` args) into the object
    /// leg — the ONE place the exactly-one rule lives (Round 448 session
    /// review: both surfaces had hand-rolled copies). Shape-vs-predicate
    /// validation stays in the store builder; this is pure arg resolution.
    pub fn from_exclusive_args(
        entity: Option<String>,
        value: Option<String>,
    ) -> Result<Self, String> {
        match (entity, value) {
            (Some(id), None) => Ok(TypedObject::Entity { id }),
            (None, Some(value)) => Ok(TypedObject::Value { value }),
            (Some(_), Some(_)) => Err(
                "typed leg: the entity-shaped and value-shaped object args are mutually \
                 exclusive (give exactly one)"
                    .to_string(),
            ),
            (None, None) => Err(
                "typed leg needs an object: give the entity-shaped or the value-shaped \
                 object arg"
                    .to_string(),
            ),
        }
    }
}

/// Optional machine-readable leg of a [`NarrativeFact`] (Round 446, design
/// sec 7.12): subject–predicate–object, binary only (n-ary = recorded
/// revisit trigger). AUTHORED in the same act as the prose claim, never
/// NLP-derived (guardrail B-1 applied to typing). The prose `claim` stays
/// required and primary; partial coverage is the design — the
/// deterministic rule gate (Round B) covers the typed subset, recorded
/// conflict edges and the future LLM-discovery adapter cover the rest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypedClaim {
    /// Registered entity id; must be a member of the owning fact's
    /// `entities` list (a typed leg never silently widens the retrieval
    /// key).
    pub subject: String,
    /// Registered predicate id (`AtomicStore.predicates` key).
    pub predicate: String,
    /// Object leg; its shape must match the predicate's declared
    /// [`PredicateObjectKind`].
    pub object: TypedObject,
}

/// Whether a fact is a narrative SETUP expecting a later payoff (Round 442
/// — the narrative mirror of the spec side's [`crate::CoverageExpectation`]
/// axis: a declared expectation plus a read-only coverage classification).
/// `Unmarked` is the default and means the author has not marked the fact —
/// absence of marking is *unrecorded*, never an assertion that the fact is
/// not a setup (the sparse-frame ethos applied to the discourse axis).
/// `Expected` is Chekhov's gun: a payoff should become visible in every
/// world-line where the setup is visible; until then the setup is DANGLING
/// — a report finding (the author's todo list), deliberately never a gate
/// reject (a WIP story has dangling setups by definition).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PayoffExpectation {
    #[default]
    Unmarked,
    Expected,
}

impl PayoffExpectation {
    /// Canonical lowercase label (matches the serde representation).
    pub fn as_str(self) -> &'static str {
        match self {
            PayoffExpectation::Unmarked => "unmarked",
            PayoffExpectation::Expected => "expected",
        }
    }

    /// Parse the canonical lowercase tag ([`Self::as_str`]) back to a
    /// value. `None` for any other string. Mirrors
    /// [`crate::CoverageExpectation::from_tag`].
    pub fn from_tag(s: &str) -> Option<Self> {
        match s {
            "unmarked" => Some(PayoffExpectation::Unmarked),
            "expected" => Some(PayoffExpectation::Expected),
            _ => None,
        }
    }
}

fn payoff_unmarked(p: &PayoffExpectation) -> bool {
    *p == PayoffExpectation::Unmarked
}

/// One multi-axis narrative fact: a claim held within exactly one epistemic
/// frame over a canon-time extent, evidenced by structure sections.
///
/// Append-only by genre for IN-WORLD change: a belief that changes is not
/// edited — a successor fact in the same frame records
/// `supersedes_in_frame`, and the predecessor's effective end is DERIVED
/// from the successor's `canon_from` (never written back). The stored
/// `canon_to` is for beliefs that end without a successor. The one
/// exception is AUTHOR-time correction (Round 434, design sec 7.9 axis 4):
/// `amend_fact` / `retract_fact` revise or remove a mis-written claim —
/// that is transaction-time history (the git log of the store), not
/// in-world belief change, and never routes through succession.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NarrativeFact {
    /// Epistemic frame id (registry key in `AtomicStore.frames`). Exactly
    /// one — a believed-fact and the corresponding ground-truth fact are
    /// distinct facts, never one fact with two frames.
    pub frame: String,
    /// World-line branch id (Round 433, design sec 7.9 axis 2). Frames are
    /// sparse epistemic axes; branches are divergent quest-path/playthrough
    /// worlds. Conflict scoping and in-frame succession are both
    /// `(frame, branch)`-scoped: same-frame facts on different world-lines
    /// never conflict, and succession never crosses a branch (divergence is
    /// data, not succession). Serialization skips the default branch so
    /// pre-branch stores stay byte-stable.
    #[serde(default = "default_branch", skip_serializing_if = "is_main_branch")]
    pub branch: String,
    /// Entity ids this claim is about (Round 437) — the retrieval key for
    /// entity-scoped verification ("does this scene contradict X's
    /// background"). Multi-ref by design: a relation involves two or more
    /// entities. Optional — a world-level fact may be about no entity.
    /// Every ref must be registered (fail-loud at the mutate primitive).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entities: Vec<String>,
    /// The claim held in this frame, per-claim granularity (atomic,
    /// falsifiable — one assertion, not an entity dossier).
    pub claim: String,
    /// Canon coordinate where this claim starts holding: a structure-section
    /// id (the medium's discourse order, e.g. a chapter).
    pub canon_from: String,
    /// Explicit canon end for a belief that ends WITHOUT an in-frame
    /// successor. When a successor exists, the effective end derives from
    /// the successor's `canon_from` and this stays `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canon_to: Option<String>,
    /// Structure-section ids evidencing the claim (≥ 1, fail-loud at the
    /// mutate primitive). Multi-ref by design — a claim's evidence usually
    /// spans sections.
    pub evidence: Vec<String>,
    /// Recorded conflict assertions. Contradiction is a semantic judgment,
    /// so edges are recorded — never derived from claim text. The
    /// continuity gate evaluates them world-scoped: same-scope overlapping
    /// conflict = violation; cross-scope conflict = data. Each assertion
    /// pins the TARGET claim it judged (Round 439): an amend of the target
    /// makes the judgment stale, surfaced by the scan — never silently
    /// trusted, never auto-refreshed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conflicts_with: Vec<ConflictAssertion>,
    /// In-frame predecessor this claim replaces (same frame enforced at the
    /// mutate primitive). The mechanism for time-indexed belief change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supersedes_in_frame: Option<String>,
    /// Setup marking (Round 442). `Expected` declares this fact a setup
    /// whose payoff coverage the read-side report classifies per world;
    /// the default `Unmarked` serializes to nothing (pre-payoff stores
    /// stay byte-stable).
    #[serde(default, skip_serializing_if = "payoff_unmarked")]
    pub payoff_expectation: PayoffExpectation,
    /// Optional typed leg (Round 446): the machine-readable
    /// subject–predicate–object reading of `claim`, authored in the same
    /// act as the prose (never NLP-derived). Absence means the claim is
    /// prose-only — partial coverage is the design, not a gap.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub typed: Option<TypedClaim>,
    /// Setup fact ids this fact PAYS OFF (Round 442) — the backward
    /// pointer shape of `supersedes_in_frame` (the setup is written first
    /// and never touched when paid; append-only by genre). A discourse-
    /// structure relation, so it crosses frames and follows world-line
    /// visibility like any fact. Identity refs, deliberately UNPINNED:
    /// like succession they relate fact identities, not wordings (the
    /// Round 439 pin covers judgments about claim text only). Targets
    /// must exist (fail-loud at the mutate primitive; the scan re-checks
    /// out-of-band edits).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pays_off: Vec<String>,
    /// Optional verbatim quote backing the claim (a derived cache of medium
    /// content, EPUB-SSOT symmetric).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quote: Option<String>,
    /// sha256 of `quote`, computed by the mutate primitive at write time
    /// (never caller-supplied) so an out-of-band sidecar edit is detectable
    /// offline — the R404 content-drift pattern.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quote_sha256: Option<String>,
}

/// How a fact reaches the READER under a given telling (Round 506, design sec
/// 7.24 — the disclosure/discourse axis). `Withhold` (the default = the
/// sparse-frame ethos applied to disclosure) means never told: the reader
/// reconstructs it (the Dark-Souls hidden-lore extreme). `State` = told
/// outright; `Hint` = partially signalled; `Imply` = realised via an
/// object/environment (the Dark-Souls item-text). The render-acceptance gate
/// enforces ONLY the exposed/withheld binary + `first_at` timing (R502); the
/// `state`/`hint`/`imply` gradation at-or-after `first_at` is CRAFT
/// (blind-judged), never gated — the four modes are an authoring vocabulary,
/// the gate's half-enforced-invariant guard (CLAUDE.md).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DisclosureMode {
    #[default]
    Withhold,
    State,
    Hint,
    Imply,
}

impl DisclosureMode {
    /// Canonical lowercase label (matches the serde representation).
    pub fn as_str(self) -> &'static str {
        match self {
            DisclosureMode::Withhold => "withhold",
            DisclosureMode::State => "state",
            DisclosureMode::Hint => "hint",
            DisclosureMode::Imply => "imply",
        }
    }

    /// Parse the canonical lowercase tag ([`Self::as_str`]) back to a value.
    /// `None` for any other string (fail-loud at the caller; no silent
    /// default — the [`PayoffExpectation::from_tag`] pattern).
    pub fn from_tag(s: &str) -> Option<Self> {
        match s {
            "withhold" => Some(DisclosureMode::Withhold),
            "state" => Some(DisclosureMode::State),
            "hint" => Some(DisclosureMode::Hint),
            "imply" => Some(DisclosureMode::Imply),
            _ => None,
        }
    }
}

fn disclosure_mode_is_withhold(m: &DisclosureMode) -> bool {
    *m == DisclosureMode::Withhold
}

/// The scene/object a disclosure rides on (Round 506, design sec 7.24 —
/// resolves the R502 `surface` under-spec, reusing existing ref kinds: no new
/// ref space). `scene` = a structure-section ref (canon space, like
/// [`NarrativeFact::canon_from`]); `object` = an optional registered entity id
/// (the diegetic carrier — the Dark-Souls item that realises an `imply`).
/// STORED for the render-brief carrier but NOT gated (the gate uses `mode` +
/// `first_at` only; `surface` is craft guidance).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DisclosureSurface {
    /// Structure-section ref the disclosure surfaces in.
    pub scene: String,
    /// Optional registered entity id the disclosure rides on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
}

/// One per-fact disclosure decision within a telling (Round 506, design sec
/// 7.24): a sparse override over the plan's `default_mode`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DisclosureOverride {
    /// How this fact reaches the reader under this telling.
    pub mode: DisclosureMode,
    /// The discourse coordinate where the reader first LEARNS this fact, per
    /// WORLD-LINE (resolves the R502 under-spec: reading order differs per
    /// branch — the per-world contract). Keyed by branch id; the value is a
    /// structure-section ref in canon space (the same space as `canon_from`),
    /// distinct from when the fact is TRUE in the fabula. Empty = no timing
    /// pin (a pure `withhold`, or timing left to the fact's own coordinate).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub first_at: BTreeMap<String, String>,
    /// Optional scene/object the disclosure rides on (render-brief craft hint;
    /// NOT gated).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface: Option<DisclosureSurface>,
}

/// A named TELLING over the fact base (Round 506, design sec 7.24 — the
/// disclosure/discourse layer). Keyed by telling id in
/// `AtomicStore.disclosure_plans`. Multiple plans over ONE fact base = many
/// tellings (Dark-Souls-fragment / classic-mystery / expository-thriller) —
/// the North Star "one substrate → many tellings" made concrete. The plan is
/// authored like any data and is NOT a store-integrity invariant (disclosure
/// timing is a RENDER property, checked by the render-acceptance gates over
/// re-extracted prose, not by `validate-workspace`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DisclosurePlan {
    /// Free-form description of this telling. Optional prose, not load-bearing.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// The default mode for any fact WITHOUT an override (default = `Withhold`,
    /// the sparse-frame ethos — only load-bearing facts get an explicit
    /// decision, a de-facto salience filter, R505). Serializes to nothing when
    /// `Withhold` so a dark-souls plan's bytes stay minimal (the
    /// `PayoffExpectation::Unmarked` skip-default precedent).
    #[serde(default, skip_serializing_if = "disclosure_mode_is_withhold")]
    pub default_mode: DisclosureMode,
    /// Sparse per-fact overrides, keyed by fact id (an `AtomicStore.
    /// narrative_facts` key).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub overrides: BTreeMap<String, DisclosureOverride>,
}

/// The effective disclosure of one fact under a telling, for one world-line
/// (Round 510 — THE single resolver of the override-vs-default semantics).
/// Every reader of a plan (the render-brief carrier, the coverage surface, any
/// future consumer) derives its answer here so the override/default
/// interpretation cannot drift across call sites (the CLAUDE.md
/// half-enforced-invariant guard, read-side).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveDisclosure {
    /// The override's mode, or the plan default when no override exists.
    pub mode: DisclosureMode,
    /// `true` iff an explicit per-fact override exists (vs the plan default) —
    /// the coverage `never-planned` (defaulted) vs `disclosed`/`hidden` split.
    pub is_override: bool,
    /// The first-disclosure coordinate for the queried world (`None` when
    /// defaulted, or the override pins no coordinate for this world). Distinct
    /// from when the fact is TRUE in the fabula (`canon_from`).
    pub first_at: Option<String>,
    /// The diegetic surface the disclosure rides on (override-only; render
    /// craft guidance, never gated).
    pub surface: Option<DisclosureSurface>,
}

impl DisclosurePlan {
    /// The effective mode of a fact under this telling (world-independent — a
    /// mode is one decision per (fact x telling), not per world). Returns the
    /// override's mode and `true`, else the plan default and `false`. The ONE
    /// place the override-vs-default rule lives.
    pub fn effective_mode(&self, fact_id: &str) -> (DisclosureMode, bool) {
        match self.overrides.get(fact_id) {
            Some(ov) => (ov.mode, true),
            None => (self.default_mode, false),
        }
    }

    /// The full effective disclosure of a fact for one world-line — the mode
    /// ([`Self::effective_mode`]) plus the world's `first_at` pin and the
    /// surface (both override-only). The single resolver the carrier consumes.
    pub fn effective(&self, fact_id: &str, world: &str) -> EffectiveDisclosure {
        let (mode, is_override) = self.effective_mode(fact_id);
        let ov = self.overrides.get(fact_id);
        EffectiveDisclosure {
            mode,
            is_override,
            first_at: ov.and_then(|o| o.first_at.get(world).cloned()),
            surface: ov.and_then(|o| o.surface.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Round 510 — the single disclosure resolver: `effective` and
    /// `effective_mode` agree, an override wins over the default, `first_at` is
    /// per-world-line, and a defaulted fact carries no override data. Every
    /// reader (carrier, coverage) routes here, so they cannot drift.
    #[test]
    fn disclosure_plan_effective_resolver() {
        let mut overrides = BTreeMap::new();
        let mut first_at = BTreeMap::new();
        first_at.insert("w1".to_string(), "ch-3".to_string());
        overrides.insert(
            "shown".to_string(),
            DisclosureOverride {
                mode: DisclosureMode::State,
                first_at,
                surface: Some(DisclosureSurface {
                    scene: "ch-2".to_string(),
                    object: None,
                }),
            },
        );
        let plan = DisclosurePlan {
            description: String::new(),
            default_mode: DisclosureMode::Withhold,
            overrides,
        };

        // Override wins; first_at is per world-line; is_override = true.
        assert_eq!(plan.effective_mode("shown"), (DisclosureMode::State, true));
        let e_w1 = plan.effective("shown", "w1");
        assert_eq!(e_w1.mode, DisclosureMode::State);
        assert!(e_w1.is_override);
        assert_eq!(e_w1.first_at.as_deref(), Some("ch-3"));
        assert!(e_w1.surface.is_some());
        // No pin for another world-line.
        assert_eq!(plan.effective("shown", "w2").first_at, None);

        // Defaulted fact: the plan default, no override data.
        assert_eq!(
            plan.effective_mode("absent"),
            (DisclosureMode::Withhold, false)
        );
        let e_def = plan.effective("absent", "w1");
        assert_eq!(e_def.mode, DisclosureMode::Withhold);
        assert!(!e_def.is_override);
        assert_eq!(e_def.first_at, None);
        assert!(e_def.surface.is_none());

        // Parity: effective().mode is exactly effective_mode().0 (one source).
        for fact in ["shown", "absent"] {
            assert_eq!(plan.effective(fact, "w1").mode, plan.effective_mode(fact).0);
        }
    }

    /// Round 448 — the ONE shared resolution of the flattened typed-object
    /// arg pair (CLI flags / MCP args both route here).
    #[test]
    fn typed_object_exclusive_args_resolution() {
        assert_eq!(
            TypedObject::from_exclusive_args(Some("gun".into()), None).unwrap(),
            TypedObject::Entity { id: "gun".into() }
        );
        assert_eq!(
            TypedObject::from_exclusive_args(None, Some("alive".into())).unwrap(),
            TypedObject::Value {
                value: "alive".into()
            }
        );
        assert!(
            TypedObject::from_exclusive_args(Some("a".into()), Some("b".into()))
                .unwrap_err()
                .contains("mutually exclusive")
        );
        assert!(TypedObject::from_exclusive_args(None, None)
            .unwrap_err()
            .contains("needs an object"));
    }
}
