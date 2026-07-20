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

/// World-line MEMBERSHIP of one query world (Round 612) — THE single definition
/// of "whose facts are part of world W, and under what departure bound".
///
/// Maps a branch id to the departure coordinates a fact on that branch must be
/// at-or-before — **all of them**; the constraints CONJOIN. An EMPTY set means
/// the branch belongs to W unconditionally (W's own branch, or a confluence W
/// flows into — the shared continuation downstream of every parent). A branch
/// ABSENT from the map is not part of W's world-line at all.
///
/// The bounds are carried as a SET of coordinates rather than folded into a
/// single `min`, which is what keeps this order-free: nothing here consults the
/// [`crate::Branch`] graph's canon order, so the SAME definition serves the
/// write path (`succession_branch_inherits`) and the read path (the continuity
/// gate's visibility) — the one-invariant discipline. The canon order compares
/// `canon_from` against each bound later, at visibility time.
pub type WorldMembership = BTreeMap<String, BTreeSet<String>>;

/// Compute [`WorldMembership`] for `world` (Round 612 — the series-parallel
/// lattice that replaced the enumerated `cut` / `forward` / `cut_forward`
/// relations of Rounds 438 / 533 / 611).
///
/// The algebra, over the branch DAG:
/// - **own** and every **forward confluence** ([`forward_confluences`]): unbounded.
/// - **W forks from A at `at`**: `membership(A)`, with `at` CONJOINED into every
///   bound — a fork inherits the ancestor's whole COMPOSED walk (its own facts
///   AND whatever an upstream merge displaced onto a confluence), cut at the
///   departure.
/// - **C converges from {P_i at at_i}**: the INTERSECTION over parents of
///   (`membership(P_i)` conjoined with `at_i`) — a confluence continues only what
///   EVERY incoming road agrees on. That is exactly the path-independent trunk
///   prefix: each parent's exclusive middle is missing from some other parent, so
///   the intersection drops it, while the shared pre-merge trunk survives.
///
/// Fork = conjoin a bound. Merge = intersect the parents. The lattice is CLOSED:
/// a fork off a confluence, a fork off a fork off a confluence, nested
/// confluences, and a confluence whose parent is itself a confluence all fall out
/// of these two rules — no further relation is ever needed (the enumerate-one-
/// more-relation trajectory of R438 → R533 → R611 ends here).
///
/// CONJOINING the bounds (rather than keeping only the nearest departure) is what
/// makes a non-monotone fork chain sound: a world that departs its parent at a
/// coordinate BEFORE the parent's own fork point inherits BOTH cuts, so it cannot
/// see anything past the tighter one.
///
/// Acyclic by write-path construction (a parent must pre-exist registration); the
/// recursion stack fails loud on an out-of-band cyclic edit instead of looping.
pub fn world_membership(
    branches: &BTreeMap<String, Branch>,
    world: &str,
) -> Result<WorldMembership, String> {
    let mut memo: BTreeMap<String, WorldMembership> = BTreeMap::new();
    let mut on_stack: BTreeSet<String> = BTreeSet::new();
    membership_of(branches, world, &mut memo, &mut on_stack)
}

fn membership_of(
    branches: &BTreeMap<String, Branch>,
    world: &str,
    memo: &mut BTreeMap<String, WorldMembership>,
    on_stack: &mut BTreeSet<String>,
) -> Result<WorldMembership, String> {
    if let Some(done) = memo.get(world) {
        return Ok(done.clone());
    }
    if !on_stack.insert(world.to_string()) {
        return Err(format!(
            "branch lineage of `{world}` is cyclic — out-of-band edit \
             (the mutate API cannot write a cycle: a parent must pre-exist)"
        ));
    }
    // Own branch, and every confluence this world flows INTO: unconditional.
    let mut out: WorldMembership = BTreeMap::new();
    out.insert(world.to_string(), BTreeSet::new());
    for confluence in forward_confluences(branches, world) {
        out.insert(confluence, BTreeSet::new());
    }
    if let Some(branch) = branches.get(world) {
        // A branch is a fork-child XOR a confluence (write-path enforced), so at
        // most one of the two arms below contributes — an inherited entry can
        // never collide with another inherited entry, only with the
        // unconditional own/forward entries above, which are the most permissive
        // and therefore win (`or_insert`).
        if let Some(fork) = &branch.forks_from {
            let parent = membership_of(branches, &fork.branch, memo, on_stack)?;
            for (id, bounds) in conjoin(parent, &fork.at) {
                out.entry(id).or_insert(bounds);
            }
        }
        let mut merged: Option<WorldMembership> = None;
        for edge in &branch.converges_from {
            let parent = membership_of(branches, &edge.branch, memo, on_stack)?;
            let bounded = conjoin(parent, &edge.at);
            merged = Some(match merged {
                None => bounded,
                Some(acc) => intersect(acc, bounded),
            });
        }
        for (id, bounds) in merged.unwrap_or_default() {
            out.entry(id).or_insert(bounds);
        }
    }
    on_stack.remove(world);
    memo.insert(world.to_string(), out.clone());
    Ok(out)
}

/// Cut an inherited membership at a departure coordinate: every bound gains
/// `at`, so an inherited fact must now ALSO be at-or-before the departure.
fn conjoin(membership: WorldMembership, at: &str) -> WorldMembership {
    membership
        .into_iter()
        .map(|(id, mut bounds)| {
            bounds.insert(at.to_string());
            (id, bounds)
        })
        .collect()
}

/// Intersect two incoming roads at a merge: a branch continues past the
/// confluence only if BOTH roads carried it, and it must satisfy BOTH roads'
/// bounds (the constraints conjoin — the union of the two bound sets).
fn intersect(a: WorldMembership, mut b: WorldMembership) -> WorldMembership {
    a.into_iter()
        .filter_map(|(id, mut bounds)| {
            let other = b.remove(&id)?;
            bounds.extend(other);
            Some((id, bounds))
        })
        .collect()
}

/// Walk a world-line's FORWARD confluence-suffix closure (Round 533 — the
/// forward dual of the backward fork walk). Given a query world `world`, returns every
/// confluence branch `C` such that `world` is, transitively, one of `C`'s
/// converging parents (`world ∈ C.converges_from`, or `world` reaches such a
/// `C` through a chain of confluences). These are the SHARED continuations
/// `world` flows INTO past a merge — a fact authored once on `C` is part of
/// `world`'s world-line (the inverse of fork inheritance: a fork CHILD inherits
/// its parent's prefix; a converging PARENT inherits the confluence's suffix).
/// Sorted, deduplicated. Termination needs no hop cap: each confluence enters
/// the result set at most once (`BTreeSet` dedup), and only a fresh insert
/// re-expands — so the walk is bounded by the registry size by construction,
/// the dual of a linear fork-chain's hop cap (a chain is not deduplicated, so
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

/// A world-line reference is "known" iff it is [`MAIN_BRANCH`] (the implicit,
/// never-registered default world) or a registered branch — THE single
/// definition of the "main or registered" guard the write path and every read
/// surface share. Hand-copying `world != MAIN_BRANCH && !branches.contains_key`
/// at a dozen sites is how one forgotten copy false-rejected a `main`-as-
/// confluence-parent store (the R607 boundary bug); routing every site through
/// this predicate makes an Nth site physically unable to drop the exemption.
pub fn is_known_world(branches: &BTreeMap<String, Branch>, world: &str) -> bool {
    world == MAIN_BRANCH || branches.contains_key(world)
}

/// Whether an in-frame succession edge whose successor and predecessor sit on
/// DIFFERENT world-lines is legitimate (Rounds 438 + 535 + 612) — THE single
/// definition of cross-branch succession legitimacy, called by BOTH enforcement
/// points (the write path [`check_succession_edge`] in mnemosyne-atomic and the
/// out-of-band scan re-check in mnemosyne-validate's continuity gate), so the
/// two cannot drift (the multi-write-path-one-invariant discipline).
///
/// Succession is DIRECTIONAL — the successor must lie DOWNSTREAM of the
/// predecessor in the world-line flow — so it reads [`world_membership`] from
/// both ends, and each direction means something different:
///
/// THE DIRECTION DISCRIMINATOR: in [`world_membership`], a NON-EMPTY bound set
/// means UPSTREAM and an EMPTY one means own-or-DOWNSTREAM. That is exact, not a
/// heuristic — every backward step (a fork's departure, a merge's join) CONJOINS
/// at least one coordinate, so an inherited-prefix entry always carries a bound,
/// while the two unconditional entries are precisely the world's own branch and
/// the confluences it flows INTO.
///
/// - **BACKWARD (a fork revising an inherited belief, R438):** the predecessor is
///   UPSTREAM of the successor — it sits in the successor's membership WITH a
///   bound. Round 612 reads that off the lattice, which is what closes the R611
///   hole: a fork off a confluence-chain trunk now has the displaced trunk branch
///   in its membership, so the supersede that revises a belief it can SEE is
///   finally legal. (Before, the read path showed the fact holding in the
///   divergent world while this predicate refused the revision — the gate reported
///   a contradiction and then forbade the only sanctioned way to resolve it.)
/// - **FORWARD (a merge reconciling a parent's belief at the join, R535):** the
///   successor is a CONFLUENCE the predecessor flows INTO — it sits in the
///   PREDECESSOR's membership with an EMPTY bound set.
///
/// Both arms therefore require the successor to lie DOWNSTREAM. Revising UPSTREAM
/// is the leak-back R438 forbids (an ancestor must never see its fork's revision;
/// a parent must never rewrite the merge it flows into), and sideways between
/// siblings is not succession at all — divergence is data.
///
/// Graph-level only, deliberately: it authorizes the EDGE; it does not evaluate
/// the departure bound (that needs the canon order, which lives with the reader).
pub fn succession_branch_inherits(
    branches: &BTreeMap<String, Branch>,
    successor_branch: &str,
    predecessor_branch: &str,
) -> Result<bool, String> {
    if successor_branch == predecessor_branch {
        return Ok(true);
    }
    if world_membership(branches, successor_branch)?
        .get(predecessor_branch)
        .is_some_and(|bounds| !bounds.is_empty())
    {
        return Ok(true);
    }
    Ok(world_membership(branches, predecessor_branch)?
        .get(successor_branch)
        .is_some_and(BTreeSet::is_empty))
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
    /// Registered kind ref — a key of `AtomicStore.entity_kinds`, NOT free
    /// text. Optional (empty = unspecified); a NON-empty value must resolve,
    /// fail-loud at the mutate primitive AND at the scan boundary. The
    /// vocabulary itself stays the consumer's (a game medium registers
    /// character/place/item; a spec medium registers something else):
    /// the substrate enforces THAT the kind is registered, never WHICH kinds
    /// exist (ARCHITECTURE.md sec 6 invariant 4 — routing, not prohibition).
    ///
    /// This was a free-text `String` until the machine-slot rule (Round 661)
    /// reached it: every other identity here — frames, branches, entities,
    /// predicates, sections — is registered and fail-loud, and Round 661
    /// counted the entity's ID as registered while missing the kind INSIDE
    /// the record. Measured on the live corpus before closing: 5 distinct
    /// kinds over 109 entities, every one filled, zero typos — the set was
    /// already closed in practice, so registration costs the author nothing
    /// and buys the spatial gate a question it can actually ask.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub kind: String,
    /// Free-form description. Optional prose, not load-bearing.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

/// One registered entity kind (registry entry). Keyed by kind id in
/// `AtomicStore.entity_kinds`; every non-empty [`Entity::kind`] must name a
/// key here — the frames/branches/entities/predicates registry symmetry
/// (Round 436's write-side-typo lesson applied to the last slot that lacked
/// it: a typo'd kind would silently answer "not a place" and route the
/// entity out of every kind-scoped gate).
///
/// The registry holds the vocabulary the CONSUMER declares. Core never
/// enumerates the members — there is no `Place` variant here and there must
/// never be one (invariant 4).
///
/// `parents` (Round 732 DEBT-M as a single-parent tree, generalised to a DAG
/// in Round 738) is the SET of this kind's direct SUPER-kinds (registered
/// `entity_kinds` refs), forming a directed ACYCLIC graph — MULTIPLE
/// inheritance (the Inform "a weapon is a kind of thing" model, extended so a
/// `magic-sword` can be BOTH a `weapon` AND a `magic-item`). A predicate's
/// endpoint-kind gate (Round 701) then scopes over the whole ANCESTOR CLOSURE:
/// a `thing`-scoped rule accepts a `weapon` because `thing` is reachable
/// upward from `weapon.parents`. An empty set ⇒ a root kind ⇒ the ancestor
/// closure is the singleton `{self}` ⇒ identical to a flat registry (the
/// backward-compat line: a pre-R738 store's lone `parent` migrates to a
/// one-element `parents`, Round 738 load migration v37→v38).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityKind {
    /// This kind's direct super-kinds (registered `entity_kinds` refs) — a SET
    /// (0..N), because a kind may specialise more than one super-kind (a DAG,
    /// not a tree). Empty for a root kind. Fail-loud at the write path
    /// (`add_entity_kind`: every parent registered + none self) AND at the scan
    /// boundary (`entity_kind_parent_violations`: every parent registered + the
    /// whole graph acyclic) — the registry-symmetry parity. `BTreeSet` dedups
    /// and orders deterministically (a byte-stable on-disk shape).
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub parents: BTreeSet<String>,
    /// Free-form description. Optional prose, not load-bearing.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

/// A registered unit of measure (Round 706) — the consumer's vocabulary for
/// the [`TypedObject::Quantity`] amount slot, keyed by unit id in
/// `AtomicStore.units`. Every `Quantity.unit` must reference a key here
/// (fail-loud at the mutate primitives AND at the scan boundary — the
/// entity_kinds symmetry). The registry holds WHICH units the consumer
/// declares (`day`, `minute`, `metre`); core never enumerates the members —
/// there is no `Day` variant and there must never be one (invariant 4, the
/// R700 place-kind lesson applied one axis over). A bare unit string would
/// reintroduce the drift defect one level down (`min`/`minute`/`분`); the
/// registry keeps the set the consumer's while the substrate enforces THAT the
/// unit is registered.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Unit {
    /// Free-form description. Optional prose, not load-bearing.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

/// A registered numeric PARAMETER (Round 728 design → Round 729 build, DEBT-K)
/// — the consumer's vocabulary for an accumulating meter (`affection`, `karma`,
/// `gold`, an RPG stat), keyed by parameter id in `AtomicStore.parameters`.
/// Every `parameter_deltas` reference (and `parameter_gates`, R730) must name a
/// key here (fail-loud at the mutate primitives AND at the scan boundary — the
/// `units` symmetry). The registry holds WHICH meters the consumer declares; core never
/// enumerates the members — there is no `Affection` / `Karma` variant and there
/// must never be one (invariant 4, the R700 place-kind / R706 unit lesson one
/// axis over). A bare parameter string would reintroduce the drift defect
/// (`affection` / `affinity` / `호감도`); the registry keeps the set the
/// consumer's while the substrate enforces THAT the parameter is registered.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Parameter {
    /// Free-form description. Optional prose, not load-bearing.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

/// A numeric comparison operator (Round 489 interval rule; LIFTED to core in
/// Round 730 so the [`ParameterGate`] threshold and the
/// `NarrativeRuleSpec::Interval` bound share ONE operator — a numeric comparison
/// is a core domain concept, not a validate-only one). The closed set of scalar
/// comparisons: `value ⋈op⋈ bound`. Mnemosyne STORES `op` on a gate and NEVER
/// evaluates it (the R712 layering line); [`holds`](IntervalOp::holds) is the
/// interval rule's tool AND the consumer's, never called by Mnemosyne on a gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum IntervalOp {
    Ge,
    Le,
    Eq,
    Gt,
    Lt,
}

impl IntervalOp {
    /// Apply the operator to a computed `diff` (the interval rule's
    /// `value(left) − value(right)`, or a consumer's accumulated meter) and a
    /// `bound` (the interval bound, or a gate threshold). `true` = the
    /// constraint HOLDS (no violation). `pub` since Round 730 (the lift widened
    /// it for cross-crate use); Mnemosyne calls it only for the interval rule,
    /// never on a stored gate (the R712 layering line — a gate is the consumer's
    /// to evaluate).
    pub fn holds(self, diff: f64, bound: f64) -> bool {
        match self {
            IntervalOp::Ge => diff >= bound,
            IntervalOp::Le => diff <= bound,
            IntervalOp::Eq => diff == bound,
            IntervalOp::Gt => diff > bound,
            IntervalOp::Lt => diff < bound,
        }
    }

    /// The reporting symbol (findings / reports carry it so a reader sees the
    /// relation). `pub` since Round 730 (the lift).
    pub fn symbol(self) -> &'static str {
        match self {
            IntervalOp::Ge => ">=",
            IntervalOp::Le => "<=",
            IntervalOp::Eq => "==",
            IntervalOp::Gt => ">",
            IntervalOp::Lt => "<",
        }
    }
}

/// A numeric-value THRESHOLD gate on a CHOICE edge (Round 728 design → Round 730
/// build, DEBT-K) — the thing K-of-N ([`EdgeGuard`](crate::narrative) threshold,
/// R724) cannot express: a signed/weighted meter compared to a threshold
/// ("romance route unlocks if affection >= 4"). The value type of
/// [`AtomicStore::parameter_gates`](../../mnemosyne_atomic/struct.AtomicStore.html),
/// keyed by the CHOICE edge fact id. `parameter` is a `parameters` registry ref
/// (fail-loud); `op` is the comparison ([`IntervalOp`], shared with the interval
/// rule); `threshold` is the required accumulated value.
///
/// Because the gate references the METER DIRECTLY (not a disconnected boolean
/// proxy fact), the R725 boolean-proxy silent hole is UNREPRESENTABLE — there is
/// no second "sufficient" fact to leave stale when the meter drops (DEBT-K hole
/// 1, closed by make-unrepresentable, not by a detector). Mnemosyne holds the
/// DECLARATION and integrity-checks only that the parameter is registered + the
/// keyed fact exists — it NEVER accumulates the meter along a playthrough or
/// evaluates whether the gate holds now (the consumer's job; the R712 layering
/// line, exactly as `edge_guards` stores conditions the consumer counts). NOT
/// `Default`: a default op/threshold is meaningless (the [`EdgeCost`] no-Default
/// lesson).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct ParameterGate {
    /// The gated meter — a `parameters` registry ref (fail-loud at the write
    /// path AND the scan boundary).
    pub parameter: String,
    /// The comparison operator (`>=` is the measured case; the full
    /// [`IntervalOp`] set is a cheap declaration — karma `<=`, currency `>=`
    /// both occur).
    pub op: IntervalOp,
    /// The required accumulated value the consumer compares the meter against.
    pub threshold: i64,
}

/// The cost of one map EDGE (Round 709 design → DEBT-J build) — a number + a
/// registered unit, the SAME shape as [`TypedObject::Quantity`] but stored as a
/// side-table VALUE (`AtomicStore.edge_costs`, keyed by the adjacent fact id)
/// rather than a reified fact. The edge cost is FRAME-INVARIANT ground truth
/// (owner-invented map minutes, no section evidence, no per-frame/branch
/// variation), so it is edge METADATA, not a perspectival claim — the R709
/// review's decisive reason to pick the side-table over reifying the subject
/// leg. `n` is an EXACT integer (map minutes; f64 avoided, the R706 lesson) and
/// must be POSITIVE (build-map.py's G3 — 0 = a free teleport); `unit` is a ref
/// into the store's `units` registry (fail-loud, never free text — invariant 4).
///
/// No `Default` derive (unlike sibling `Unit`/`Frame`): `EdgeCost::default()`
/// would be `{n:0, unit:""}` — a value the write path REJECTS (n=0 is the G3
/// free-teleport, `""` an unregistered unit), so a `..Default::default()` would
/// materialize a cost the primitive never accepts. `stage_registry_entry` needs
/// only `PartialEq`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EdgeCost {
    /// The cost amount — an exact positive integer (e.g. walk minutes).
    pub n: i64,
    /// A ref into the `units` registry (e.g. `minute`), fail-loud.
    pub unit: String,
}

/// Declared object shape of a [`Predicate`] (Round 446, design sec 7.12).
/// `Entity` = the object leg names a registered entity (locations, custody
/// targets); `Token` = the object leg is a member of a CLOSED vocabulary
/// DECLARED on the predicate ([`Predicate::object_tokens`], Round 705) — the
/// enumerable replacement for the removed free-text scalar, so the substrate can
/// answer "what values does this predicate take" (the machine slot is never
/// blind). The vocabulary itself stays the consumer's (invariant 4); the
/// substrate enforces only THAT a token is in the declared set. `Quantity`
/// (Round 706) = the object leg is a number + a REGISTERED unit
/// ([`TypedObject::Quantity`]): the amount slot for timeline/measurement facts,
/// with the unit a ref into the store's `units` registry (fail-loud, never free
/// text — the entity_kinds/invariant-4 lesson applied to units, since a game
/// measures `minute`, a legal store `day`; core must not enumerate them).
/// `Fact` (Round 707) = the object leg references another FACT of this store
/// ([`TypedObject::Fact`]) — a typed fact-ref with two-way referential
/// integrity (existence checked in PHASE 2 against store ∪ staged, so a legal
/// same-manifest forward ref is not rejected; the delete path refuses to orphan
/// it). Round 708 REMOVED the free-text `Scalar` shape (with `TypedObject::Value`)
/// — the last unenumerable machine slot; free text now lives ONLY in the fact's
/// prose `claim`, and every typed object is registered/enumerable. The builder
/// checks the typed leg's object against this declaration — a shape or
/// vocabulary mismatch is a write-time reject, not a scan finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PredicateObjectKind {
    Entity,
    Token,
    Quantity,
    Fact,
}

impl PredicateObjectKind {
    /// Canonical lowercase label (matches the serde representation).
    pub fn as_str(self) -> &'static str {
        match self {
            PredicateObjectKind::Entity => "entity",
            PredicateObjectKind::Token => "token",
            PredicateObjectKind::Quantity => "quantity",
            PredicateObjectKind::Fact => "fact",
        }
    }

    /// Parse the canonical lowercase tag back to a value. `None` for any
    /// other string (fail-loud at the caller; no silent default).
    pub fn from_tag(s: &str) -> Option<Self> {
        match s {
            "entity" => Some(PredicateObjectKind::Entity),
            "token" => Some(PredicateObjectKind::Token),
            "quantity" => Some(PredicateObjectKind::Quantity),
            "fact" => Some(PredicateObjectKind::Fact),
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
/// [`Entity::kind`] is a registered ref (Round 661); when a predicate names a
/// `subject_kind` / `object_entity_kind` (Round 701) it becomes load-bearing —
/// the write-path endpoint gate matches an endpoint entity's kind against it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Predicate {
    /// Declared object shape; the builder enforces it on every typed leg.
    pub object_kind: PredicateObjectKind,
    /// Round 701 — required entity-KIND for the SUBJECT leg (a registered
    /// `entity_kinds` ref; `None` = any entity). The subject is always an
    /// entity ([`TypedClaim::subject`]), so there is no shape combination to
    /// guard. When `Some`, the write path (`build_typed_claim`) rejects a fact
    /// whose subject entity is not that kind — the spatial-map G1 gate lives
    /// here (an `adjacent` predicate declares `subject_kind = place`), enforced
    /// at write time rather than by a scan.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject_kind: Option<String>,
    /// Round 701 — required entity-KIND for an ENTITY-shaped OBJECT leg (a
    /// registered `entity_kinds` ref; `None` = any). Only meaningful when
    /// `object_kind = Entity`; the builder REJECTS `Some` under any non-entity
    /// kind (a token / quantity / fact object has no entity kind), so the
    /// nonsensical pairing is unreachable through the mutate API.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_entity_kind: Option<String>,
    /// Round 705 — the CLOSED object vocabulary, meaningful ONLY when
    /// `object_kind = Token`. Every [`TypedObject::Token`] under this predicate
    /// must be a member (the write path rejects a token outside the set — the
    /// R436 typo guard one level down). Non-empty is REQUIRED for a `Token`
    /// predicate and REJECTED for any other kind (`build_predicate`), so an
    /// empty-vocab token slot — the free-text hole re-opened — is unrepresentable.
    /// A `BTreeSet` so membership is O(log n), duplicates collapse, and the
    /// serialized order is deterministic. Extended mid-authoring via
    /// `set_predicate` (R658 — a closed set with an escape hatch, not a trap).
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub object_tokens: BTreeSet<String>,
    /// Free-form description. Optional prose, not load-bearing.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

/// The object leg of a [`TypedClaim`] — shaped by real data (design sec 7.12):
/// locations/custody objects are entities; state values are consumer-vocabulary
/// tokens (a closed declared set, Round 705). Round 708 removed the free-text
/// `Value` variant — EVERY machine-slot object is now registered/enumerable
/// (Entity / Token / Quantity / Fact); free text lives ONLY in the prose
/// `claim`. Serde-tagged, no stringly union.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TypedObject {
    /// A registered entity id (must also be a member of the owning fact's
    /// `entities` list — the entities list stays THE retrieval key).
    Entity { id: String },
    /// A member of the predicate's CLOSED, declared vocabulary
    /// ([`Predicate::object_tokens`], Round 705) — the enumerable replacement for
    /// the removed free-text `Value`: the substrate CAN enumerate the legal set,
    /// so self-extraction is not blind. The write path rejects a token outside
    /// the declared set.
    Token { token: String },
    /// A number + a REGISTERED unit (Round 706 — the amount slot for
    /// timeline/measurement facts, e.g. `signed-on-day = Quantity{10, day}`).
    /// `n` is an EXACT integer (all measured live uses are integer counts;
    /// `f64` is deliberately avoided — it reintroduces `10` vs `10.0`
    /// serialization and equality fuzz for what are day/minute counts). `unit`
    /// is a ref into the store's `units` registry (fail-loud, never free text —
    /// invariant 4); the write path rejects an unregistered unit. The interval
    /// evaluator (R489) reads `n` for its arithmetic.
    Quantity { n: i64, unit: String },
    /// A reference to another FACT of this store (Round 707 — `opened_by = f-*`
    /// and the reification class). Distinct from `Entity`: the referent is a
    /// fact, not an entity, so existence is checked in PHASE 2 against
    /// store ∪ staged (a legal same-manifest forward ref must not reject), and
    /// the delete path refuses to orphan it — symmetric with
    /// `conflicts_with` / `pays_off`, NOT the phase-1 registry facets. A fact
    /// may not reference itself.
    Fact { id: String },
}

impl TypedObject {
    /// Resolve the flattened CLI arg surface (`--typed-object-entity` /
    /// `--typed-object-token` / `--typed-object-quantity-*` / `--typed-object-fact`)
    /// into the object leg — the exactly-one rule lives here (Round 448). The MCP
    /// surface does NOT route here: since Round 692 it accepts the `TypedObject`
    /// enum directly through its JsonSchema (a new variant is auto-exposed), so
    /// the enum's own shape is the forcing function there. Shape-vs-predicate
    /// validation stays in the store builder; this is pure arg resolution.
    ///
    /// The `token` (Round 705) / `quantity` (Round 706) / `fact` (Round 707)
    /// parameters are each the arity change the R660 oracle
    /// (`every_declared_object_kind_is_satisfiable_from_the_arg_surface`) forced
    /// when its `PredicateObjectKind` variant was added — the arity change breaks
    /// every CLI call site so the flag cannot be left unwired (the R625/R659
    /// half-wired-green defense). Round 708 removed the `value` parameter with the
    /// free-text `Value` shape.
    pub fn from_exclusive_args(
        entity: Option<String>,
        token: Option<String>,
        quantity: Option<(i64, String)>,
        fact: Option<String>,
    ) -> Result<Self, String> {
        let candidates: Vec<TypedObject> = [
            entity.map(|id| TypedObject::Entity { id }),
            token.map(|token| TypedObject::Token { token }),
            quantity.map(|(n, unit)| TypedObject::Quantity { n, unit }),
            fact.map(|id| TypedObject::Fact { id }),
        ]
        .into_iter()
        .flatten()
        .collect();
        match candidates.len() {
            1 => Ok(candidates.into_iter().next().unwrap()),
            0 => Err(
                "typed leg needs an object: give exactly one of the entity / token / \
                 quantity / fact object args"
                    .to_string(),
            ),
            _ => Err(
                "typed leg: the entity / token / quantity / fact object args are \
                 mutually exclusive (give exactly one)"
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
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
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

    /// The single "main or registered" world-ref guard: `main` is known even
    /// though it is never in the registry, a registered branch is known, an
    /// unknown id is not — and an empty registry still knows `main`.
    #[test]
    fn is_known_world_covers_main_and_registered_only() {
        let mut branches = BTreeMap::new();
        assert!(is_known_world(&branches, MAIN_BRANCH));
        assert!(!is_known_world(&branches, "braid"));
        branches.insert("braid".to_string(), Branch::default());
        assert!(is_known_world(&branches, "braid"));
        assert!(is_known_world(&branches, MAIN_BRANCH));
        assert!(!is_known_world(&branches, "ghost"));
    }

    /// Round 535 — the cross-branch succession legitimacy predicate, the SINGLE
    /// definition both enforcement points share (write path + scan re-check), so
    /// it IS the multi-write-path parity guarantee. A confluence diamond:
    /// `main → {sluice, ride}` (fork at `tr`) → `dawn` (the merge).
    #[test]
    fn succession_inherits_in_both_lineage_directions() {
        let fork = |at: &str| BranchFork {
            branch: MAIN_BRANCH.to_string(),
            at: at.to_string(),
        };
        let converge = |b: &str, at: &str| BranchFork {
            branch: b.to_string(),
            at: at.to_string(),
        };
        let mut branches = BTreeMap::new();
        branches.insert(
            "sluice".to_string(),
            Branch {
                forks_from: Some(fork("tr")),
                ..Branch::default()
            },
        );
        branches.insert(
            "ride".to_string(),
            Branch {
                forks_from: Some(fork("tr")),
                ..Branch::default()
            },
        );
        branches.insert(
            "dawn".to_string(),
            Branch {
                converges_from: vec![converge("sluice", "sl"), converge("ride", "rd")],
                ..Branch::default()
            },
        );
        let inherits =
            |succ: &str, pred: &str| succession_branch_inherits(&branches, succ, pred).unwrap();
        // BACKWARD: a fork inherits its ancestor's belief (R438).
        assert!(inherits("sluice", MAIN_BRANCH));
        // FORWARD: the merge reconciles a parent's belief (R535).
        assert!(inherits("dawn", "sluice"));
        assert!(inherits("dawn", "ride"));
        // EQUAL: trivially inherits.
        assert!(inherits("sluice", "sluice"));
        // SIBLING: ride does not inherit sluice's belief (neither direction).
        assert!(!inherits("ride", "sluice"));
        // DIRECTION matters: a parent does NOT inherit the merge's belief
        // (succession flows parent → merge, never merge → parent).
        assert!(!inherits("sluice", "dawn"));
    }

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
            TypedObject::from_exclusive_args(Some("gun".into()), None, None, None).unwrap(),
            TypedObject::Entity { id: "gun".into() }
        );
        assert_eq!(
            TypedObject::from_exclusive_args(None, Some("dead".into()), None, None).unwrap(),
            TypedObject::Token {
                token: "dead".into()
            }
        );
        assert_eq!(
            TypedObject::from_exclusive_args(None, None, Some((10, "day".into())), None).unwrap(),
            TypedObject::Quantity {
                n: 10,
                unit: "day".into()
            }
        );
        assert_eq!(
            TypedObject::from_exclusive_args(None, None, None, Some("f-1".into())).unwrap(),
            TypedObject::Fact { id: "f-1".into() }
        );
        assert!(
            TypedObject::from_exclusive_args(Some("a".into()), Some("c".into()), None, None)
                .unwrap_err()
                .contains("mutually exclusive")
        );
        assert!(TypedObject::from_exclusive_args(
            None,
            None,
            Some((1, "u".into())),
            Some("f-1".into())
        )
        .unwrap_err()
        .contains("mutually exclusive"));
        assert!(TypedObject::from_exclusive_args(None, None, None, None)
            .unwrap_err()
            .contains("needs an object"));
    }

    /// A subway-braid trunk: `main` + `braid1` (fork at s1) reconverge into the
    /// confluence `weave1` at s2; `braid2` forks OFF THE CONFLUENCE at s3; and a
    /// divergent `ending` forks off `main` at s3, downstream of the merge.
    fn braid_chain() -> BTreeMap<String, Branch> {
        let fork = |from: &str, at: &str| BranchFork {
            branch: from.to_string(),
            at: at.to_string(),
        };
        BTreeMap::from([
            (
                "braid1".to_string(),
                Branch {
                    forks_from: Some(fork(MAIN_BRANCH, "s1")),
                    ..Branch::default()
                },
            ),
            (
                "weave1".to_string(),
                Branch {
                    converges_from: vec![fork(MAIN_BRANCH, "s2"), fork("braid1", "s2")],
                    ..Branch::default()
                },
            ),
            (
                "braid2".to_string(),
                Branch {
                    forks_from: Some(fork("weave1", "s3")),
                    ..Branch::default()
                },
            ),
            (
                "ending".to_string(),
                Branch {
                    forks_from: Some(fork(MAIN_BRANCH, "s3")),
                    ..Branch::default()
                },
            ),
        ])
    }

    /// Round 612 — MERGE = INTERSECT. A confluence continues only what EVERY
    /// incoming road carried: the path-independent trunk prefix survives (bounded
    /// at the merge coordinates), while each parent's EXCLUSIVE middle is dropped
    /// (it is missing from the other parent, so the intersection removes it). This
    /// is what the enumerated relations never expressed — pre-R612 a confluence's
    /// membership was empty of its parents entirely, so a confluence world saw a
    /// prefix-less fragment and a fork off it (below) lost the trunk outright.
    #[test]
    fn confluence_membership_is_the_intersection_of_its_parents() {
        let b = braid_chain();
        let weave = world_membership(&b, "weave1").unwrap();
        assert!(weave["weave1"].is_empty(), "own branch is unbounded");
        // `main` survives the merge — but BOUNDED by both roads' cuts, which is
        // exactly what excludes main's own exclusive middle downstream of s1.
        assert_eq!(
            weave["main"],
            BTreeSet::from(["s1".to_string(), "s2".to_string()]),
            "the shared trunk is inherited, conjoined with BOTH roads' bounds"
        );
        // Neither parent's EXCLUSIVE identity crosses the merge.
        assert!(!weave.contains_key("braid1"), "braid1's road is not shared");
        assert!(!weave.contains_key("ending"), "a sibling never crosses");
    }

    /// Round 612 — FORK = CONJOIN A BOUND, and it composes with the merge. A fork
    /// off a CONFLUENCE (`braid2`) inherits the confluence's whole membership —
    /// including the pre-merge trunk the merge carried through — cut at its own
    /// departure. Pre-R612 `fork_chain` terminated at a confluence (it has no
    /// `forks_from`), so `braid2` lost the entire pre-merge trunk: the same class
    /// of bug as MNEMO-GAP-003, one level up, and it is the SECOND link of every
    /// subway-braid chain.
    #[test]
    fn fork_off_a_confluence_inherits_the_pre_merge_trunk() {
        let b = braid_chain();
        let braid2 = world_membership(&b, "braid2").unwrap();
        assert!(braid2["braid2"].is_empty());
        assert!(
            braid2.contains_key("weave1"),
            "the confluence it forked off is a member"
        );
        assert_eq!(
            braid2["main"],
            BTreeSet::from(["s1".to_string(), "s2".to_string(), "s3".to_string()]),
            "the pre-merge trunk rides THROUGH the confluence, conjoined with the fork cut"
        );
        assert!(!braid2.contains_key("braid1"), "the other road stays out");
    }

    /// Round 612 — CONJOINING (not min-ing) the bounds is what makes a NON-MONOTONE
    /// fork chain sound: `early` departs `late` at s1, but `late` itself only
    /// departed `main` at s4. `early` must satisfy BOTH cuts, so a main fact at s2
    /// (past s1 but before s4) is correctly excluded. Keeping only the nearest
    /// departure — the pre-R612 shape — leaked the whole s1..s4 span into `early`.
    #[test]
    fn non_monotone_fork_chain_conjoins_every_cut() {
        let fork = |from: &str, at: &str| BranchFork {
            branch: from.to_string(),
            at: at.to_string(),
        };
        let b = BTreeMap::from([
            (
                "late".to_string(),
                Branch {
                    forks_from: Some(fork(MAIN_BRANCH, "s4")),
                    ..Branch::default()
                },
            ),
            (
                "early".to_string(),
                Branch {
                    forks_from: Some(fork("late", "s1")),
                    ..Branch::default()
                },
            ),
        ]);
        assert_eq!(
            world_membership(&b, "early").unwrap()["main"],
            BTreeSet::from(["s1".to_string(), "s4".to_string()]),
            "BOTH departures bind — the order then enforces the tighter one"
        );
    }

    /// Round 612 — succession is DIRECTIONAL, and both directions now read the one
    /// membership. A divergent ending may revise a trunk belief a merge DISPLACED
    /// onto the confluence (the R611 hole: the gate reported the contradiction and
    /// then forbade the only sanctioned fix); a confluence may still reconcile a
    /// parent's belief at the merge (R535); and a fork's revision may NEVER leak
    /// back up into its ancestor, nor sideways between siblings.
    #[test]
    fn succession_follows_the_membership_in_both_directions_only_downstream() {
        let b = braid_chain();
        let inherits = |succ: &str, pred: &str| succession_branch_inherits(&b, succ, pred).unwrap();
        // BACKWARD, through a merge (the R611 hole this closes).
        assert!(
            inherits("ending", "weave1"),
            "a divergent ending revises the displaced trunk belief it can SEE"
        );
        assert!(inherits("ending", MAIN_BRANCH), "and the plain trunk");
        // FORWARD (R535): the merge reconciles a parent's belief at the join.
        assert!(inherits("weave1", "braid1"));
        assert!(inherits("weave1", MAIN_BRANCH));
        // NEVER upstream — a fork's revision must not leak back into its ancestor.
        assert!(!inherits(MAIN_BRANCH, "ending"));
        assert!(!inherits("weave1", "braid2"));
        // NEVER sideways — divergence is data, not succession.
        assert!(!inherits("braid1", "ending"));
        assert!(!inherits("ending", "braid1"));
    }

    /// Round 612 — the lattice fails LOUD on an out-of-band cyclic edit instead of
    /// looping (the mutate API cannot write one: a parent must pre-exist).
    #[test]
    fn cyclic_branch_lineage_fails_loud() {
        let fork = |from: &str| BranchFork {
            branch: from.to_string(),
            at: "s1".to_string(),
        };
        let b = BTreeMap::from([
            (
                "a".to_string(),
                Branch {
                    forks_from: Some(fork("b")),
                    ..Branch::default()
                },
            ),
            (
                "b".to_string(),
                Branch {
                    forks_from: Some(fork("a")),
                    ..Branch::default()
                },
            ),
        ]);
        assert!(world_membership(&b, "a").unwrap_err().contains("cyclic"));
    }
}
