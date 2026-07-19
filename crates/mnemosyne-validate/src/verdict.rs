//! Actionable violations (Round 588, R585 debt item 2) — the machine-repairable
//! shape of a gate finding.
//!
//! An external generate-gate-repair agent needs a violation it can repair
//! PROGRAMMATICALLY, not a prose line it must parse: `rule` (a stable machine
//! id) + `locus` (which fact / field / coordinate) + `expected` (what should
//! hold) + `repair_hint` (the authoring action) + a one-line `message`. This
//! module owns that shape and the mapping from the structured
//! [`ContinuityViolation`] into it. The mapping is an EXHAUSTIVE match — an
//! added continuity variant fails to compile here, so a new gate finding can
//! never reach the loop as an un-actionable blob (the describe-schema drift
//! guard, applied to violations).
//!
//! Shape (write-time) violations do not originate as a structured enum — the
//! atomic mutate primitive fails fast with a carefully-worded `Validation`
//! string that already names the fact/field and its repair — so those are
//! carried verbatim via [`ActionableViolation::shape`] rather than
//! reverse-parsed into fields (the no-NLP discipline).

use serde::Serialize;

use crate::continuity::ContinuityViolation;

/// Where a violation is anchored — the fact(s), and the field / scope
/// coordinates that apply. Sparse: only the coordinates a given rule implicates
/// are populated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct ViolationLocus {
    /// The fact id(s) at fault — the primary one first.
    pub facts: Vec<String>,
    /// The entity id(s) at fault, when the violation is anchored on entities
    /// rather than a single fact (Round 702) — e.g. a graph-level map violation
    /// naming the unreachable places. Empty for fact-anchored violations.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entities: Vec<String>,
    /// The fact FIELD implicated, when a single field is (`evidence`,
    /// `supersedes_in_frame`, `pays_off`, `branch`, `canon_to`, `typed`, …).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    /// The epistemic frame, when scoped.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame: Option<String>,
    /// The world-line branch, when scoped.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// The canon coordinate the violation occurs at, when applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
}

/// One gate finding an agent can repair without parsing prose (R588).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ActionableViolation {
    /// Which gate produced it: `shape` (a write-time invariant) or
    /// `continuity` (the cross-fact gate).
    pub source: &'static str,
    /// The stable machine rule id — the [`ContinuityViolation`] `kind`, or
    /// `shape-invariant`.
    pub rule: String,
    /// Where the violation is anchored.
    pub locus: ViolationLocus,
    /// What the substrate expected to hold.
    pub expected: String,
    /// The authoring action that repairs it.
    pub repair_hint: String,
    /// A one-line human summary.
    pub message: String,
}

impl ActionableViolation {
    /// A write-time shape violation, carried verbatim from the atomic mutate
    /// primitive's fail-fast `Validation` message (which already names the fact
    /// and its repair). Locus is intentionally coarse — the message is the
    /// actionable payload; reverse-parsing the string into fields would be the
    /// NLP guessing the substrate forbids.
    pub fn shape(message: String) -> Self {
        ActionableViolation {
            source: "shape",
            rule: "shape-invariant".to_string(),
            locus: ViolationLocus::default(),
            expected: "the batch must satisfy every write-time invariant \
                       (see `describe-schema` invariants)"
                .to_string(),
            repair_hint: "fix the fact/field named in the message and re-propose".to_string(),
            message,
        }
    }
}

/// First 8 hex chars of a sha256 pin, for a compact stale-judgment message.
fn short_sha(s: &str) -> &str {
    s.get(..8).unwrap_or(s)
}

/// Map one [`ContinuityViolation`] into its actionable shape (R588). EXHAUSTIVE
/// — a new continuity variant will not compile until it is given a
/// rule/locus/expected/repair here.
pub fn continuity_actionable(v: &ContinuityViolation) -> ActionableViolation {
    let action = |rule: &str,
                  locus: ViolationLocus,
                  expected: String,
                  repair_hint: String,
                  message: String| ActionableViolation {
        source: "continuity",
        rule: rule.to_string(),
        locus,
        expected,
        repair_hint,
        message,
    };
    match v {
        ContinuityViolation::FrameConflictOverlap {
            frame,
            branch,
            fact_a,
            fact_b,
            at,
        } => action(
            "frame_conflict_overlap",
            ViolationLocus {
                facts: vec![fact_a.clone(), fact_b.clone()],
                frame: Some(frame.clone()),
                branch: Some(branch.clone()),
                at: Some(at.clone()),
                ..Default::default()
            },
            "two facts recorded as conflicting must not both hold at one canon point in one \
             (frame, branch)"
                .to_string(),
            "supersede or amend one claim, narrow one's canon extent so they no longer co-hold, \
             or remove the conflict edge if they do not actually contradict"
                .to_string(),
            format!(
                "facts `{fact_a}` and `{fact_b}` conflict yet both hold at `{at}` in frame \
                 `{frame}` / branch `{branch}`"
            ),
        ),
        ContinuityViolation::FactCanonOffBranch {
            fact,
            branch,
            coord,
        } => action(
            "fact_canon_off_branch",
            ViolationLocus {
                facts: vec![fact.clone()],
                field: Some("branch".to_string()),
                branch: Some(branch.clone()),
                at: Some(coord.clone()),
                ..Default::default()
            },
            "a fact's canon_from must be positioned in its OWN branch's order".to_string(),
            "set the fact's branch to the world-line whose order positions the coordinate \
             (often the named trunk branch, not the default `main`)"
                .to_string(),
            format!("fact `{fact}` sits at `{coord}`, not in branch `{branch}`'s order — wrong world-line"),
        ),
        ContinuityViolation::EvidenceUnreachable {
            fact,
            branch,
            evidence,
            canon_from,
        } => action(
            "evidence_unreachable",
            ViolationLocus {
                facts: vec![fact.clone()],
                field: Some("evidence".to_string()),
                branch: Some(branch.clone()),
                at: Some(canon_from.clone()),
                ..Default::default()
            },
            "every evidence ref must be reachable at-or-before the fact's canon_from in its own \
             world-line"
                .to_string(),
            "cite an establishing scene prior in this branch, or move the fact to the branch \
             where the evidence is reachable"
                .to_string(),
            format!(
                "fact `{fact}` cites evidence `{evidence}` not reachable by `{canon_from}` in \
                 branch `{branch}`"
            ),
        ),
        ContinuityViolation::ConfluenceEvidenceUnreconciled {
            fact,
            confluence,
            parent,
            evidence,
            canon_from,
        } => action(
            "confluence_evidence_unreconciled",
            ViolationLocus {
                facts: vec![fact.clone()],
                field: Some("evidence".to_string()),
                branch: Some(confluence.clone()),
                at: Some(canon_from.clone()),
                ..Default::default()
            },
            "a fact on a confluence must cite evidence reachable from EVERY converging parent"
                .to_string(),
            "cite a shared establishing scene reachable from all parents, or move the fact onto \
             the parent world-line if it is a path-dependent continuation"
                .to_string(),
            format!(
                "fact `{fact}` on confluence `{confluence}` cites evidence `{evidence}` \
                 unreachable from parent `{parent}`"
            ),
        ),
        ContinuityViolation::SuccessionContradiction {
            frame,
            predecessor,
            successor,
            stored_to,
            successor_from,
        } => action(
            "succession_contradiction",
            ViolationLocus {
                facts: vec![predecessor.clone(), successor.clone()],
                field: Some("canon_to".to_string()),
                frame: Some(frame.clone()),
                ..Default::default()
            },
            "a predecessor's stored canon_to must not outlive its successor's canon_from"
                .to_string(),
            "drop the predecessor's canon_to (the end derives from the successor), or set it \
             at-or-before the successor's start"
                .to_string(),
            format!(
                "in frame `{frame}`, predecessor `{predecessor}` canon_to `{stored_to}` outlives \
                 successor `{successor}` start `{successor_from}`"
            ),
        ),
        ContinuityViolation::SuccessionCrossFrame {
            successor,
            predecessor,
            successor_frame,
            predecessor_frame,
        } => action(
            "succession_cross_frame",
            ViolationLocus {
                facts: vec![successor.clone(), predecessor.clone()],
                field: Some("supersedes_in_frame".to_string()),
                ..Default::default()
            },
            "supersedes_in_frame must stay within one epistemic frame".to_string(),
            "record the successor in the predecessor's frame, or drop the succession edge"
                .to_string(),
            format!(
                "succession `{successor}` -> `{predecessor}` crosses frames (`{successor_frame}` \
                 vs `{predecessor_frame}`)"
            ),
        ),
        ContinuityViolation::SuccessionCrossBranch {
            successor,
            predecessor,
            successor_branch,
            predecessor_branch,
        } => action(
            "succession_cross_branch",
            ViolationLocus {
                facts: vec![successor.clone(), predecessor.clone()],
                field: Some("supersedes_in_frame".to_string()),
                ..Default::default()
            },
            "cross-branch succession is legitimate only along fork/confluence lineage (a fork \
             revising an inherited belief, or a merge reconciling a parent belief)"
                .to_string(),
            "put the successor on a world-line that inherits the predecessor's branch, or drop \
             the edge (a sibling-world edit is not succession)"
                .to_string(),
            format!(
                "succession `{successor}` -> `{predecessor}` crosses to a non-inheriting branch \
                 (`{successor_branch}` vs `{predecessor_branch}`)"
            ),
        ),
        ContinuityViolation::ConflictTargetMissing { fact_id, target } => action(
            "conflict_target_missing",
            ViolationLocus {
                facts: vec![fact_id.clone(), target.clone()],
                field: Some("conflicts_with".to_string()),
                ..Default::default()
            },
            "a conflict edge must name an existing fact".to_string(),
            "remove the dangling conflict edge, or restore the missing target".to_string(),
            format!("fact `{fact_id}` conflict edge names missing fact `{target}`"),
        ),
        ContinuityViolation::ConflictEdgeStale {
            fact_id,
            target,
            stamped_sha256,
            current_sha256,
        } => action(
            "conflict_edge_stale",
            ViolationLocus {
                facts: vec![fact_id.clone(), target.clone()],
                field: Some("conflicts_with".to_string()),
                ..Default::default()
            },
            "a conflict judgment must pin the target's CURRENT claim text".to_string(),
            "re-affirm the judgment (amending the edge-owning fact restamps its edges) or retract \
             it — the target's claim changed since it was judged"
                .to_string(),
            format!(
                "fact `{fact_id}` conflict judgment on `{target}` is stale (pinned {} vs current {})",
                short_sha(stamped_sha256),
                short_sha(current_sha256)
            ),
        ),
        ContinuityViolation::SuccessionTargetMissing { fact_id, target } => action(
            "succession_target_missing",
            ViolationLocus {
                facts: vec![fact_id.clone(), target.clone()],
                field: Some("supersedes_in_frame".to_string()),
                ..Default::default()
            },
            "supersedes_in_frame must name an existing fact".to_string(),
            "remove the dangling succession edge, or restore the missing target".to_string(),
            format!("fact `{fact_id}` supersedes missing fact `{target}`"),
        ),
        ContinuityViolation::SuccessionCycle { cycle } => action(
            "succession_cycle",
            ViolationLocus {
                facts: cycle.clone(),
                field: Some("supersedes_in_frame".to_string()),
                ..Default::default()
            },
            "succession edges must form a chain, never a cycle".to_string(),
            "break the loop — one fact in the cycle must not supersede another member".to_string(),
            format!("succession cycle: {}", cycle.join(" -> ")),
        ),
        ContinuityViolation::PayoffTargetMissing { fact_id, target } => action(
            "payoff_target_missing",
            ViolationLocus {
                facts: vec![fact_id.clone(), target.clone()],
                field: Some("pays_off".to_string()),
                ..Default::default()
            },
            "a pays_off ref must name an existing setup fact".to_string(),
            "remove the dangling pays_off ref, or add the missing setup fact".to_string(),
            format!("fact `{fact_id}` pays off missing setup `{target}`"),
        ),
        ContinuityViolation::RuleExclusiveOverlap {
            rule,
            predicate,
            frame,
            branch,
            fact_a,
            fact_b,
            at,
        } => action(
            "rule_exclusive_overlap",
            ViolationLocus {
                facts: vec![fact_a.clone(), fact_b.clone()],
                entities: Vec::new(),
                field: Some("typed".to_string()),
                frame: Some(frame.clone()),
                branch: Some(branch.clone()),
                at: Some(at.clone()),
            },
            format!(
                "exclusive rule `{rule}` (predicate `{predicate}`): at most one value/holder may \
                 co-hold per (frame, branch)"
            ),
            "supersede one claim so they no longer co-hold, or separate them across world-lines \
             or canon extents"
                .to_string(),
            format!(
                "exclusive rule `{rule}`: facts `{fact_a}` and `{fact_b}` co-hold conflicting \
                 `{predicate}` at `{at}` (frame `{frame}`, branch `{branch}`)"
            ),
        ),
        ContinuityViolation::RuleTransitionInvalid {
            rule,
            predicate,
            frame,
            subject,
            predecessor,
            successor,
            from,
            to,
        } => action(
            "rule_transition_invalid",
            ViolationLocus {
                facts: vec![predecessor.clone(), successor.clone()],
                field: Some("typed".to_string()),
                frame: Some(frame.clone()),
                ..Default::default()
            },
            format!(
                "transition rule `{rule}` (predicate `{predicate}`): `{from}` -> `{to}` is not an \
                 allowed step"
            ),
            "author an intermediate succession through an allowed state, or correct the from/to \
             values"
                .to_string(),
            format!(
                "transition rule `{rule}`: subject `{subject}` steps `{from}` -> `{to}` \
                 (`{predecessor}` -> `{successor}`) outside the allowed set"
            ),
        ),
        ContinuityViolation::RuleIntervalViolation {
            rule,
            predicate,
            right,
            op,
            frame,
            branch,
            subject,
            left_fact,
            right_fact,
            left_value,
            right_value,
            bound,
            at,
        } => action(
            "rule_interval_violation",
            ViolationLocus {
                facts: vec![left_fact.clone(), right_fact.clone()],
                entities: Vec::new(),
                field: Some("typed".to_string()),
                frame: Some(frame.clone()),
                branch: Some(branch.clone()),
                at: Some(at.clone()),
            },
            format!("interval rule `{rule}`: value(`{predicate}`) - value(`{right}`) {op} {bound}"),
            "adjust the operand values (or the bound fact) so the numeric relation holds"
                .to_string(),
            format!(
                "interval rule `{rule}`: subject `{subject}` {left_value} - {right_value} not \
                 {op} {bound} at `{at}` (frame `{frame}`, branch `{branch}`)"
            ),
        ),
        ContinuityViolation::AdjacencySelfLoop {
            rule,
            predicate,
            fact,
            place,
        } => action(
            "adjacency_self_loop",
            ViolationLocus {
                facts: vec![fact.clone()],
                field: Some("typed".to_string()),
                ..Default::default()
            },
            format!(
                "transition rule `{rule}`: an edge in `{predicate}` must have distinct endpoints"
            ),
            "remove the self-adjacency fact, or correct its endpoints to two different places"
                .to_string(),
            format!(
                "transition rule `{rule}`: `{predicate}` fact `{fact}` is a self-loop \
                 `{place}` -> `{place}`"
            ),
        ),
        ContinuityViolation::AdjacencyReverseDuplicate {
            rule,
            predicate,
            fact_a,
            fact_b,
            a,
            b,
        } => action(
            "adjacency_reverse_duplicate",
            ViolationLocus {
                facts: vec![fact_a.clone(), fact_b.clone()],
                field: Some("typed".to_string()),
                ..Default::default()
            },
            format!(
                "undirected transition rule `{rule}`: `{predicate}` must not hold BOTH directions \
                 of an edge (the eval symmetrizes; store the undirected edge once)"
            ),
            "delete one of the two facts — an undirected edge is stored once".to_string(),
            format!(
                "undirected transition rule `{rule}`: `{predicate}` holds both directions of edge \
                 `{a}` <-> `{b}` (facts `{fact_a}`, `{fact_b}`)"
            ),
        ),
        ContinuityViolation::MapDisconnected {
            rule,
            predicate,
            scope,
            reached,
            total,
            unreached,
            frame,
            branch,
        } => action(
            "map_disconnected",
            ViolationLocus {
                entities: unreached.clone(),
                field: Some("typed".to_string()),
                frame: Some(frame.clone()),
                branch: Some(branch.clone()),
                ..Default::default()
            },
            format!(
                "undirected transition rule `{rule}`: every `{predicate}` SCOPE must be a single \
                 connected component — every place reachable from every other within its container"
            ),
            "add an `adjacent` fact linking the unreachable place(s) to the rest of the scope, or \
             move them into the container they belong to"
                .to_string(),
            format!(
                "undirected transition rule `{rule}`: `{predicate}` scope `{}` is disconnected \
                 across the timeline (frame `{frame}` world `{branch}`) — {reached}/{total} \
                 reachable, unreachable: {}",
                if scope.is_empty() { "<root>" } else { scope },
                unreached.join(", ")
            ),
        ),
        ContinuityViolation::MapInventedPlace {
            rule,
            predicate,
            place_kind,
            place,
            frame,
            branch,
        } => action(
            "map_invented_place",
            ViolationLocus {
                entities: vec![place.clone()],
                frame: Some(frame.clone()),
                branch: Some(branch.clone()),
                ..Default::default()
            },
            format!(
                "transition rule `{rule}`: every `{place_kind}` entity must be a node (in a \
                 `{predicate}` fact) or a container (a containment subject)"
            ),
            format!(
                "add a `{predicate}` fact linking this place to the map, mark it a container, or \
                 remove it if it is not a place"
            ),
            format!(
                "transition rule `{rule}`: `{place_kind}` entity `{place}` is off the `{predicate}` \
                 map at every canon point (frame `{frame}` world `{branch}`) — neither a node nor \
                 a container"
            ),
        ),
        ContinuityViolation::AdjacencyCrossScope {
            rule,
            adjacency,
            fact,
            a,
            b,
            scope_a,
            scope_b,
            frame,
            branch,
            at,
        } => action(
            "adjacency_cross_scope",
            ViolationLocus {
                facts: vec![fact.clone()],
                entities: vec![a.clone(), b.clone()],
                field: Some("typed".to_string()),
                frame: Some(frame.clone()),
                branch: Some(branch.clone()),
                at: Some(at.clone()),
            },
            format!(
                "transition rule `{rule}`: an `{adjacency}` edge must connect SIBLINGS (same \
                 direct container) — a place is not directly adjacent to one in a different \
                 container; leave via the container's own edges"
            ),
            "put the edge between siblings, or model the boundary as the container itself being a \
             node in its parent's scope (a portal), not a cross-container edge"
                .to_string(),
            format!(
                "transition rule `{rule}`: `{adjacency}` fact `{fact}` links `{a}` (in `{}`) and \
                 `{b}` (in `{}`) — different containers at `{at}` (frame `{frame}` world `{branch}`)",
                if scope_a.is_empty() { "<root>" } else { scope_a },
                if scope_b.is_empty() { "<root>" } else { scope_b }
            ),
        ),
        ContinuityViolation::MapContainedOffMap {
            rule,
            adjacency,
            containment,
            fact,
            container,
            contained,
            frame,
            branch,
        } => action(
            "map_contained_off_map",
            ViolationLocus {
                facts: vec![fact.clone()],
                entities: vec![contained.clone()],
                field: Some("typed".to_string()),
                frame: Some(frame.clone()),
                branch: Some(branch.clone()),
                ..Default::default()
            },
            format!(
                "transition rule `{rule}`: a `{containment}` object must be a node (appear in an \
                 `{adjacency}` fact) or itself a container"
            ),
            format!(
                "add an `{adjacency}` fact placing the contained place on the map, give it its own \
                 contained members, or remove the `{containment}` fact"
            ),
            format!(
                "transition rule `{rule}`: `{containment}` fact `{fact}` has container \
                 `{container}` holding `{contained}`, which is off the `{adjacency}` map at every \
                 canon point (frame `{frame}` world `{branch}`) — neither a node nor a container"
            ),
        ),
        ContinuityViolation::EdgeCostNotAnEdge {
            fact,
            found,
            expected,
        } => action(
            "edge_cost_not_an_edge",
            ViolationLocus {
                facts: vec![fact.clone()],
                field: Some("typed".to_string()),
                ..Default::default()
            },
            format!(
                "an edge cost belongs only on a map edge — its keyed fact must use an adjacency \
                 predicate ({})",
                expected.join(", ")
            ),
            "drop the stray cost with `remove-edge-cost --fact <id>` (the fact stays), or if the \
             fact IS a map edge, declare its predicate as a transition rule's `adjacency`"
                .to_string(),
            format!(
                "edge-cost fact `{fact}` is not a map edge — its predicate `{}` is not one of \
                 the adjacency predicate(s) {}",
                found.as_deref().unwrap_or("<untyped>"),
                expected.join(", ")
            ),
        ),
        ContinuityViolation::EdgeGuardNotAnEdge {
            fact,
            found,
            expected,
        } => action(
            "edge_guard_not_an_edge",
            ViolationLocus {
                facts: vec![fact.clone()],
                field: Some("typed".to_string()),
                ..Default::default()
            },
            format!(
                "an edge guard belongs only on a map edge — its keyed fact must use an adjacency \
                 predicate ({})",
                expected.join(", ")
            ),
            "drop the stray guard with `remove-edge-guard --fact <id>` (the fact stays), or if \
             the fact IS a map edge, declare its predicate as a transition rule's `adjacency`"
                .to_string(),
            format!(
                "edge-guard fact `{fact}` is not a map edge — its predicate `{}` is not one of \
                 the adjacency predicate(s) {}",
                found.as_deref().unwrap_or("<untyped>"),
                expected.join(", ")
            ),
        ),
        ContinuityViolation::ContainmentMultipleParents {
            predicate,
            frame,
            branch,
            place,
            parents,
            at,
        } => action(
            "containment_multiple_parents",
            ViolationLocus {
                facts: Vec::new(),
                entities: {
                    let mut e = vec![place.clone()];
                    e.extend(parents.clone());
                    e
                },
                field: Some("typed".to_string()),
                frame: Some(frame.clone()),
                branch: Some(branch.clone()),
                at: Some(at.clone()),
            },
            format!(
                "a `{predicate}` place must have at most one direct container at any one canon \
                 point per (frame, world) — the containment relation must be a tree"
            ),
            format!(
                "these containers CO-HOLD at `{at}` — end one before the other begins (a MOVE is \
                 fine, disjoint extents never conflict), or put the differing hierarchy in \
                 another frame; do not delete a still-true fact"
            ),
            format!(
                "`{predicate}` place `{place}` has {} co-holding containers ({}) at `{at}` in \
                 frame `{frame}` world `{branch}`",
                parents.len(),
                parents.join(", ")
            ),
        ),
        ContinuityViolation::ContainmentCycle {
            predicate,
            frame,
            branch,
            cycle,
            at,
        } => action(
            "containment_cycle",
            ViolationLocus {
                facts: Vec::new(),
                entities: cycle.clone(),
                field: Some("typed".to_string()),
                frame: Some(frame.clone()),
                branch: Some(branch.clone()),
                at: Some(at.clone()),
            },
            format!("`{predicate}` facts must form a tree, never a cycle"),
            format!(
                "break the loop at `{at}` — one place in the cycle must not contain an ancestor \
                 there (an early-then-reversed containment across disjoint extents is fine)"
            ),
            format!(
                "`{predicate}` cycle at `{at}` in frame `{frame}` world `{branch}`: {}",
                cycle.join(" -> ")
            ),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every continuity variant maps to a fully-populated actionable violation:
    /// a non-empty rule/expected/repair/message and at least one anchored fact.
    /// This exercises the exhaustive match (the drift guard's positive
    /// assertion) across a representative sample of every field shape.
    #[test]
    fn continuity_actionable_is_fully_populated() {
        let samples = vec![
            ContinuityViolation::FrameConflictOverlap {
                frame: "gt".into(),
                branch: "main".into(),
                fact_a: "f-1".into(),
                fact_b: "f-2".into(),
                at: "sc-3".into(),
            },
            ContinuityViolation::FactCanonOffBranch {
                fact: "f-3".into(),
                branch: "main".into(),
                coord: "sc-9".into(),
            },
            ContinuityViolation::SuccessionCycle {
                cycle: vec!["f-a".into(), "f-b".into(), "f-a".into()],
            },
            ContinuityViolation::PayoffTargetMissing {
                fact_id: "f-4".into(),
                target: "f-gone".into(),
            },
            ContinuityViolation::RuleIntervalViolation {
                rule: "gap".into(),
                predicate: "ratified-on".into(),
                right: "signed-on".into(),
                op: ">=".into(),
                frame: "gt".into(),
                branch: "main".into(),
                subject: "codicil".into(),
                left_fact: "f-5".into(),
                right_fact: "f-6".into(),
                left_value: "3".into(),
                right_value: "1".into(),
                bound: "5".into(),
                at: "sc-7".into(),
            },
            ContinuityViolation::AdjacencySelfLoop {
                rule: "roads".into(),
                predicate: "adjacent".into(),
                fact: "f-7".into(),
                place: "ent-village".into(),
            },
            ContinuityViolation::AdjacencyReverseDuplicate {
                rule: "roads".into(),
                predicate: "adjacent".into(),
                fact_a: "f-8".into(),
                fact_b: "f-9".into(),
                a: "ent-dike".into(),
                b: "ent-village".into(),
            },
            ContinuityViolation::MapDisconnected {
                rule: "roads".into(),
                predicate: "adjacent".into(),
                scope: "ent-island".into(),
                reached: 3,
                total: 5,
                unreached: vec!["ent-island-cove".into(), "ent-lighthouse".into()],
                frame: "gt".into(),
                branch: "main".into(),
            },
            ContinuityViolation::MapInventedPlace {
                rule: "roads".into(),
                predicate: "adjacent".into(),
                place_kind: "place".into(),
                place: "ent-ghost-town".into(),
                frame: "gt".into(),
                branch: "main".into(),
            },
            ContinuityViolation::AdjacencyCrossScope {
                rule: "roads".into(),
                adjacency: "adjacent".into(),
                fact: "f-cs".into(),
                a: "ent-hall".into(),
                b: "ent-quad".into(),
                scope_a: "ent-school".into(),
                scope_b: "".into(),
                frame: "gt".into(),
                branch: "main".into(),
                at: "ch-2".into(),
            },
            ContinuityViolation::MapContainedOffMap {
                rule: "roads".into(),
                adjacency: "adjacent".into(),
                containment: "contains".into(),
                fact: "f-c1".into(),
                container: "ent-island".into(),
                contained: "ent-nowhere".into(),
                frame: "gt".into(),
                branch: "main".into(),
            },
            ContinuityViolation::EdgeCostNotAnEdge {
                fact: "f-loves".into(),
                found: Some("loves".into()),
                expected: vec!["adjacent".into()],
            },
            ContinuityViolation::EdgeGuardNotAnEdge {
                fact: "f-hates".into(),
                found: Some("hates".into()),
                expected: vec!["adjacent".into()],
            },
            ContinuityViolation::ContainmentMultipleParents {
                predicate: "contains".into(),
                frame: "gt".into(),
                branch: "main".into(),
                place: "ent-hall".into(),
                parents: vec!["ent-castle".into(), "ent-keep".into()],
                at: "ch-2".into(),
            },
            ContinuityViolation::ContainmentCycle {
                predicate: "contains".into(),
                frame: "gt".into(),
                branch: "main".into(),
                cycle: vec!["ent-a".into(), "ent-b".into()],
                at: "ch-2".into(),
            },
        ];
        for v in &samples {
            let a = continuity_actionable(v);
            assert_eq!(a.source, "continuity");
            assert!(!a.rule.is_empty(), "empty rule for {v:?}");
            assert!(!a.expected.is_empty(), "empty expected for {v:?}");
            assert!(!a.repair_hint.is_empty(), "empty repair for {v:?}");
            assert!(!a.message.is_empty(), "empty message for {v:?}");
            // A graph-level violation (e.g. MapDisconnected) anchors on entities,
            // not a single fact — accept either.
            assert!(
                !a.locus.facts.is_empty() || !a.locus.entities.is_empty(),
                "no anchored fact or entity for {v:?}"
            );
        }
    }

    /// A shape violation carries the primitive's message verbatim.
    #[test]
    fn shape_violation_carries_message() {
        let a = ActionableViolation::shape("fact `f-1`: frame mandatory (non-empty)".to_string());
        assert_eq!(a.source, "shape");
        assert_eq!(a.rule, "shape-invariant");
        assert!(a.message.contains("frame mandatory"));
        assert!(!a.repair_hint.is_empty());
    }
}
