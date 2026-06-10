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

use std::collections::BTreeMap;

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
