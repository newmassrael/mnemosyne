//! The declared-map projection — the PLACE-axis sibling of
//! [`PlayableProjection`](crate::PlayableProjection) (the scene walk) and
//! [`QuestProjection`](crate::QuestProjection) (the journal).
//!
//! Reads the store's declared maps (`report-transition-map`, the Round 875
//! projection) and exposes them as a presentation-agnostic place layer: per
//! transition rule the node set, the edges, and for each edge the `adjacency`
//! fact that DECLARES it plus the side-table values keyed by that fact —
//! `edge_costs` (R710) and `edge_guards` (R722/R723). Every road is
//! store-derived; the kernel declares none.
//!
//! # Why this door exists
//!
//! R875 built the READ half of the map axis and measured, before writing any
//! code, that `mnemosyne-engine` — the kernel a consumer links — contained ZERO
//! occurrences of either side-table identifier. The read landed on the CLI and
//! MCP surfaces and stopped there. A live consumer had already written its own
//! map module, opened our store sidecar by hand to get back the costs and guards
//! it had just authored through `add-edge-cost` / `add-edge-guard`, and then
//! moved that parse to a build-time bake; its own comment names both the cost
//! (the store's shape is now bitten in two places on its side, and two will
//! diverge) and the single swap point it is waiting on. This module is that swap
//! point on our side of the seam.
//!
//! # Carriage, not computation
//!
//! Stated here rather than left to be rediscovered, because every line below is
//! a boundary a later round could quietly cross:
//!
//! - **`undirected` is CARRIED, and applied only where the DECLARATION says
//!   to.** [`DeclaredMapView::steps_from`] walks back-edges on an undirected map
//!   and refuses to on a directed one. It reads the flag; it never assumes maps
//!   are two-way (the R697 one-way-state-machine reason, and the reason R875
//!   reports the flag instead of pre-symmetrizing).
//! - **A guard is CARRIED, never evaluated.** Whether a road stands right now is
//!   a question about world state at play time, and answering it here would put
//!   a game rule in the kernel (the R712 layering line).
//! - **A cost is CARRIED in the unit the store registered.** This layer sums
//!   nothing, compares nothing, and computes no route. What a cost MEANS, and
//!   whether two units are even commensurable, is domain knowledge core must not
//!   hold — the invariant-4 line R711 drew when it established that the derived
//!   travel-time read is not ours to build at all. A consumer that wants a
//!   shortest path has the graph to run one over, which is the whole point.
//! - **The map is FLAT.** Each edge carries its declaring fact's frame and
//!   branch and this layer resolves neither, exactly as the gate evaluates the
//!   map (branch-scoped adjacency stays deferred, R696 finding 6 / R875).
//!
//! # Three silences that must not read alike
//!
//! [`MapProjection::transition_rules`] is carried for the reason R875 carries
//! it: with no transition rule there is no declared adjacency predicate, so the
//! store genuinely cannot know which facts are edges, and that is a different
//! answer from a rule whose map has no edges. Likewise
//! [`MapProjection::unattached_costs`] / [`MapProjection::unattached_guards`] —
//! a side-table value keyed by a fact that is no map edge is NAMED, because an
//! authored cost that simply vanishes from a baked map reads as never authored.

use std::path::Path;

use mnemosyne_validate::continuity::TransitionMapReport;

use crate::{EngineError, SceneView};

/// An edge's stored walk cost, as the store holds it. Carried, never summed —
/// see the module's carriage-not-computation boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct EdgeCostView {
    /// The magnitude the author declared.
    pub n: i64,
    /// A ref into the store's `units` registry, checked at write time (R706).
    /// Two edges may carry different units; reconciling them is the consumer's.
    pub unit: String,
}

/// An edge's stored access guard, as the store holds it. Carried, never
/// evaluated (R712) — the kernel does not know whether these conditions hold.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct EdgeGuardView {
    /// The condition facts this road's standing depends on.
    pub conditions: Vec<String>,
    /// `Some(k)` = k-of-N; `None` = require ALL of them (the canonical AND).
    pub threshold: Option<usize>,
}

/// One road of a declared map — the `adjacency` fact that declares it, its two
/// endpoints, the declaring fact's coordinate, and the side-table values keyed
/// by that fact id.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct MapEdgeView {
    /// The `adjacency` fact declaring this step — the key BOTH side tables use,
    /// and this edge's provenance: no road exists here that no fact declares.
    pub fact_id: String,
    /// The endpoint the declaring fact names as subject.
    pub from: String,
    /// The endpoint the declaring fact names as object.
    pub to: String,
    /// The declaring fact's frame. Carried, not filtered on.
    pub frame: String,
    /// The declaring fact's world-line. Carried, not filtered on.
    pub branch: String,
    /// The stored walk cost, absent when none was authored.
    pub cost: Option<EdgeCostView>,
    /// The stored access guard, absent when none was authored.
    pub guard: Option<EdgeGuardView>,
}

/// A degenerate `adjacent(a, a)`. Named rather than dropped, for the reason
/// R875 names it: the gate excludes a self-loop from the edge set, and an
/// authored fact missing from the edges with no reason given reads as never
/// authored.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct MapSelfLoopView {
    /// The `adjacency` fact declaring the degenerate step.
    pub fact_id: String,
    /// The place the fact names on both sides.
    pub node: String,
}

/// One declared map — a transition rule plus the store facts its `adjacency`
/// predicate names.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct DeclaredMapView {
    /// The transition rule that DECLARES this map.
    pub rule: String,
    /// The predicate that says WHERE A SUBJECT IS — the rule's own, as opposed
    /// to `adjacency`, which says which places are joined. This is what makes
    /// [`MapProjection::places_disclosed_in`] possible without the kernel
    /// guessing which predicate means location.
    pub predicate: String,
    /// The predicate whose facts are this map's edges.
    pub adjacency: String,
    /// Edge symmetry as DECLARED. Read by [`Self::steps_from`]; never assumed.
    pub undirected: bool,
    /// The containment predicate this map's rule declares, when it declares one.
    pub containment: Option<String>,
    /// Every place the map's edges touch, sorted.
    pub nodes: Vec<String>,
    /// Every road, in the read's order.
    pub edges: Vec<MapEdgeView>,
    /// Degenerate self-adjacencies, excluded from `edges` and named here.
    pub self_loops: Vec<MapSelfLoopView>,
}

impl DeclaredMapView {
    /// The roads leadable from `node`, each paired with the place it reaches.
    ///
    /// This is the one navigational question the place axis exists to answer,
    /// and it honours the DECLARATION rather than a hardcoded model of maps: on
    /// an `undirected` map a road declared `b -> a` is walkable from `a`, and on
    /// a directed map it is not. R875 reports the flag for exactly this reason
    /// and declines to pre-symmetrize; this reads the flag it reports.
    ///
    /// No guard is consulted — a road whose guard does not hold is still
    /// returned, carrying its guard, because deciding that is the consumer's
    /// (R712). No ordering by cost, for the same reason nothing here sums one.
    #[must_use]
    pub fn steps_from<'m>(&'m self, node: &str) -> Vec<(&'m MapEdgeView, &'m str)> {
        self.edges
            .iter()
            .filter_map(|e| {
                if e.from == node {
                    Some((e, e.to.as_str()))
                } else if self.undirected && e.to == node {
                    Some((e, e.from.as_str()))
                } else {
                    None
                }
            })
            .collect()
    }
}

/// Every map the store declares — the kernel's PLACE axis.
///
/// Not `Clone`, for the reason [`PlayableProjection`](crate::PlayableProjection)
/// and [`QuestProjection`](crate::QuestProjection) are not (Round 788): the
/// emitter hands back `&'static Self`, so a derived `Clone` would make the
/// compiler's suggested repair a silent deep copy on this axis too.
#[derive(Debug)]
pub struct MapProjection {
    maps: Vec<DeclaredMapView>,
    transition_rules: usize,
    unattached_costs: Vec<String>,
    unattached_guards: Vec<String>,
}

impl MapProjection {
    /// Project the workspace store's declared maps, reading them in process (no
    /// JSON round-trip).
    ///
    /// Takes no telling and no canon order, and that is a property of the axis
    /// rather than an omission: the map has never been canon-ordered and is not
    /// disclosure-scoped, so requiring either would assert a scoping this read
    /// does not perform (R875). `rules_override` names the narrative-rules
    /// artifact when the workspace policy does not pin one — the rule is what
    /// DECLARES the adjacency predicate, which core must not know (invariant 4).
    ///
    /// # Errors
    ///
    /// [`EngineError::Projection`] if the map read fails — an unreadable store,
    /// an unresolvable rules artifact, a rule naming a predicate the store does
    /// not register, or a side-table entry the store-registry boundary rejects.
    pub fn from_workspace(
        workspace_root: &Path,
        rules_override: Option<&str>,
    ) -> Result<Self, EngineError> {
        let report = mnemosyne_ops::transition_map_report(workspace_root, None, rules_override)
            .map_err(|e| EngineError::Projection(e.to_string()))?;
        Ok(Self::from_report(report))
    }

    /// Index an already-projected map read — the testable core
    /// ([`Self::from_workspace`] is the store-reading wrapper).
    ///
    /// CRATE-PRIVATE from the start, which the two sibling axes reached only by
    /// correction (R771 for the playable axis, R773 for the quest axis): a
    /// [`TransitionMapReport`] is a pub-field struct, so a public constructor
    /// over one would let a downstream crate hand the kernel roads the store
    /// never declared, while this module's opening line promises the opposite.
    /// A consumer supplies a whole projection through [`MapProjectionParts`]
    /// instead, which a build fills from the store.
    #[must_use]
    pub(crate) fn from_report(report: TransitionMapReport) -> Self {
        let TransitionMapReport {
            maps,
            transition_rules,
            unattached_costs,
            unattached_guards,
        } = report;
        Self {
            maps: maps
                .into_iter()
                .map(|m| DeclaredMapView {
                    rule: m.rule,
                    predicate: m.predicate,
                    adjacency: m.adjacency,
                    undirected: m.undirected,
                    containment: m.containment,
                    nodes: m.nodes,
                    edges: m
                        .edges
                        .into_iter()
                        .map(|e| MapEdgeView {
                            fact_id: e.fact_id,
                            from: e.from,
                            to: e.to,
                            frame: e.frame,
                            branch: e.branch,
                            cost: e.cost.map(|c| EdgeCostView {
                                n: c.n,
                                unit: c.unit,
                            }),
                            guard: e.guard.map(|g| EdgeGuardView {
                                conditions: g.conditions,
                                threshold: g.threshold,
                            }),
                        })
                        .collect(),
                    self_loops: m
                        .self_loops
                        .into_iter()
                        .map(|s| MapSelfLoopView {
                            fact_id: s.fact_id,
                            node: s.node,
                        })
                        .collect(),
                })
                .collect(),
            transition_rules,
            unattached_costs,
            unattached_guards,
        }
    }

    /// Every declared map, in rule order.
    #[must_use]
    pub fn maps(&self) -> &[DeclaredMapView] {
        &self.maps
    }

    /// The map a given transition rule declares.
    #[must_use]
    pub fn map(&self, rule: &str) -> Option<&DeclaredMapView> {
        self.maps.iter().find(|m| m.rule == rule)
    }

    /// How many transition rules the store declares. ZERO is a distinct answer
    /// from "a map with no edges" — see the module's three-silences note.
    #[must_use]
    pub fn transition_rules(&self) -> usize {
        self.transition_rules
    }

    /// `edge_costs` keys that are no edge of any declared map. Named, not
    /// dropped.
    #[must_use]
    pub fn unattached_costs(&self) -> &[String] {
        &self.unattached_costs
    }

    /// `edge_guards` keys that are no edge of any declared map. Named, not
    /// dropped.
    #[must_use]
    pub fn unattached_guards(&self) -> &[String] {
        &self.unattached_guards
    }

    /// Where a scene puts people, READ OUT OF WHAT THE TELLING DISCLOSED
    /// (Round 938) — the join that lets a reading surface say "you are here"
    /// without anyone authoring a second coordinate.
    ///
    /// A place is reported when a line the scene DISCLOSED carries the map's
    /// own location predicate and names an entity that is a node of that map.
    /// Nothing is guessed: the predicate comes from the transition rule the
    /// author wrote, and the place-hood test is membership in the node set the
    /// same rule's edges derive.
    ///
    /// # Why it reads the disclosed scene and not the store
    ///
    /// The continuity gate already knows where every subject stands at every
    /// step — that is what a step IS — and using it here would be a LEAK. The
    /// gate reads ground truth; a telling is allowed to withhold that a
    /// character is at the drowned quarter, and a reading surface that showed it
    /// anyway would disclose by the back door what the disclosure plan spent its
    /// whole design withholding. So this asks the scene, and where the telling
    /// says nothing the answer is honestly nothing.
    ///
    /// # What it does NOT answer
    ///
    /// "Where is the PLAYER." This returns every place the scene disclosed
    /// someone at, because the kernel does not know which character the screen
    /// belongs to — that is a consumer's declaration (a live one bakes its own
    /// viewpoint from a `plays` predicate). On authored data the two usually
    /// coincide: 19 of arm D's 20 scenes disclose exactly one place. A scene
    /// cutting between two rooms yields two, in sorted order, and saying so is
    /// better than picking one.
    #[must_use]
    pub fn places_disclosed_in<'m>(&'m self, scene: &SceneView) -> Vec<DisclosedPlace<'m>> {
        let mut found: Vec<DisclosedPlace<'m>> = Vec::new();
        for map in &self.maps {
            for line in &scene.lines {
                if line.typed_predicate() != Some(map.predicate.as_str()) {
                    continue;
                }
                for entity in line.entities() {
                    if !map.nodes.iter().any(|n| n == entity) {
                        continue;
                    }
                    let place = entity.to_string();
                    let fact_id = line.fact_id().to_string();
                    if found
                        .iter()
                        .any(|d| d.map.rule == map.rule && d.place == place)
                    {
                        continue;
                    }
                    found.push(DisclosedPlace {
                        map,
                        place,
                        fact_id,
                    });
                }
            }
        }
        found.sort_by(|a, b| (&a.map.rule, &a.place).cmp(&(&b.map.rule, &b.place)));
        found
    }
}

/// A place a scene disclosed someone at ([`MapProjection::places_disclosed_in`]).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct DisclosedPlace<'m> {
    /// The declared map this place is a node of — ask it for the exits, so a
    /// road has one home rather than a copy here.
    pub map: &'m DeclaredMapView,
    /// The place entity the scene disclosed.
    pub place: String,
    /// The fact that disclosed it. A surface showing this place can name WHY it
    /// is showing it, which is the whole provenance contract: no place on the
    /// screen that no disclosed fact put there.
    pub fact_id: String,
}

/// An [`EdgeCostView`] as plain data — the emit/ingest mirror, for the reason
/// [`QuestCompletionPart`](crate::QuestCompletionPart) is one: the view is
/// `#[non_exhaustive]`, so generated code in another crate cannot construct it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeCostPart {
    /// The magnitude the author declared.
    pub n: i64,
    /// The registered unit it is declared in.
    pub unit: String,
}

/// An [`EdgeGuardView`] as plain data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeGuardPart {
    /// The condition facts this road's standing depends on.
    pub conditions: Vec<String>,
    /// `Some(k)` = k-of-N; `None` = require ALL.
    pub threshold: Option<usize>,
}

/// A [`MapEdgeView`] as plain data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapEdgePart {
    /// The `adjacency` fact declaring this step.
    pub fact_id: String,
    /// The endpoint the declaring fact names as subject.
    pub from: String,
    /// The endpoint the declaring fact names as object.
    pub to: String,
    /// The declaring fact's frame.
    pub frame: String,
    /// The declaring fact's world-line.
    pub branch: String,
    /// The stored walk cost, absent when none was authored.
    pub cost: Option<EdgeCostPart>,
    /// The stored access guard, absent when none was authored.
    pub guard: Option<EdgeGuardPart>,
}

/// A [`MapSelfLoopView`] as plain data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapSelfLoopPart {
    /// The `adjacency` fact declaring the degenerate step.
    pub fact_id: String,
    /// The place the fact names on both sides.
    pub node: String,
}

/// A [`DeclaredMapView`] as plain data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredMapPart {
    /// The transition rule that declares this map.
    pub rule: String,
    /// The predicate that says where a subject is.
    pub predicate: String,
    /// The predicate whose facts are this map's edges.
    pub adjacency: String,
    /// Edge symmetry as declared.
    pub undirected: bool,
    /// The containment predicate the rule declares, when it declares one.
    pub containment: Option<String>,
    /// Every place the map's edges touch, sorted.
    pub nodes: Vec<String>,
    /// Every road, in the read's order.
    pub edges: Vec<MapEdgePart>,
    /// Degenerate self-adjacencies.
    pub self_loops: Vec<MapSelfLoopPart>,
}

/// A whole [`MapProjection`] as plain data — the PLACE-axis sibling of
/// [`ProjectionParts`](crate::ProjectionParts) and
/// [`QuestProjectionParts`](crate::QuestProjectionParts), and the same shape for
/// the same reason.
///
/// **This is a baked-ingestion door**; its contract — what a bake buys, and the
/// fabrication it cannot engineer away — is stated once in
/// [`crate::baked_ingestion`] and this adds nothing to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapProjectionParts {
    /// Every declared map, in rule order.
    pub maps: Vec<DeclaredMapPart>,
    /// How many transition rules the store declares.
    pub transition_rules: usize,
    /// `edge_costs` keys that are no edge of any declared map.
    pub unattached_costs: Vec<String>,
    /// `edge_guards` keys that are no edge of any declared map.
    pub unattached_guards: Vec<String>,
}

impl MapProjection {
    /// Emit this projection as plain data — the bake half of the build-time
    /// seam.
    ///
    /// Deterministic without sorting anything here: the read derives nodes from
    /// a `BTreeSet` and emits maps in rule order and edges in fact-id order, so
    /// this would only be re-sorting sorted input.
    #[must_use]
    pub fn to_parts(&self) -> MapProjectionParts {
        MapProjectionParts {
            maps: self
                .maps
                .iter()
                .map(|m| DeclaredMapPart {
                    rule: m.rule.clone(),
                    predicate: m.predicate.clone(),
                    adjacency: m.adjacency.clone(),
                    undirected: m.undirected,
                    containment: m.containment.clone(),
                    nodes: m.nodes.clone(),
                    edges: m
                        .edges
                        .iter()
                        .map(|e| MapEdgePart {
                            fact_id: e.fact_id.clone(),
                            from: e.from.clone(),
                            to: e.to.clone(),
                            frame: e.frame.clone(),
                            branch: e.branch.clone(),
                            cost: e.cost.as_ref().map(|c| EdgeCostPart {
                                n: c.n,
                                unit: c.unit.clone(),
                            }),
                            guard: e.guard.as_ref().map(|g| EdgeGuardPart {
                                conditions: g.conditions.clone(),
                                threshold: g.threshold,
                            }),
                        })
                        .collect(),
                    self_loops: m
                        .self_loops
                        .iter()
                        .map(|s| MapSelfLoopPart {
                            fact_id: s.fact_id.clone(),
                            node: s.node.clone(),
                        })
                        .collect(),
                })
                .collect(),
            transition_rules: self.transition_rules,
            unattached_costs: self.unattached_costs.clone(),
            unattached_guards: self.unattached_guards.clone(),
        }
    }

    /// Ingest baked parts — the read half, with no `Result` for the reason the
    /// sibling doors have none: the store read already ran at bake time.
    ///
    /// **This is a baked-ingestion door**; see [`crate::baked_ingestion`].
    #[must_use]
    pub fn from_parts(parts: MapProjectionParts) -> Self {
        Self {
            maps: parts
                .maps
                .into_iter()
                .map(|m| DeclaredMapView {
                    rule: m.rule,
                    predicate: m.predicate,
                    adjacency: m.adjacency,
                    undirected: m.undirected,
                    containment: m.containment,
                    nodes: m.nodes,
                    edges: m
                        .edges
                        .into_iter()
                        .map(|e| MapEdgeView {
                            fact_id: e.fact_id,
                            from: e.from,
                            to: e.to,
                            frame: e.frame,
                            branch: e.branch,
                            cost: e.cost.map(|c| EdgeCostView {
                                n: c.n,
                                unit: c.unit,
                            }),
                            guard: e.guard.map(|g| EdgeGuardView {
                                conditions: g.conditions,
                                threshold: g.threshold,
                            }),
                        })
                        .collect(),
                    self_loops: m
                        .self_loops
                        .into_iter()
                        .map(|s| MapSelfLoopView {
                            fact_id: s.fact_id,
                            node: s.node,
                        })
                        .collect(),
                })
                .collect(),
            transition_rules: parts.transition_rules,
            unattached_costs: parts.unattached_costs,
            unattached_guards: parts.unattached_guards,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mnemosyne_validate::continuity::{
        TransitionMapCost, TransitionMapEdge, TransitionMapGuard, TransitionMapReport,
        TransitionMapSelfLoop, TransitionMapView,
    };

    fn edge(fact_id: &str, from: &str, to: &str) -> TransitionMapEdge {
        TransitionMapEdge {
            fact_id: fact_id.into(),
            from: from.into(),
            to: to.into(),
            frame: "f-ground".into(),
            branch: "main".into(),
            cost: None,
            guard: None,
        }
    }

    fn view(rule: &str, undirected: bool, edges: Vec<TransitionMapEdge>) -> TransitionMapView {
        let mut nodes: Vec<String> = edges
            .iter()
            .flat_map(|e| [e.from.clone(), e.to.clone()])
            .collect();
        nodes.sort();
        nodes.dedup();
        TransitionMapView {
            rule: rule.into(),
            predicate: "at".into(),
            adjacency: "adjacent".into(),
            undirected,
            containment: None,
            nodes,
            edges,
            self_loops: Vec::new(),
        }
    }

    fn report(maps: Vec<TransitionMapView>) -> TransitionMapReport {
        let transition_rules = maps.len();
        TransitionMapReport {
            maps,
            transition_rules,
            unattached_costs: Vec::new(),
            unattached_guards: Vec::new(),
        }
    }

    /// The one oracle this axis cannot do without, and the one Round 875 nearly
    /// shipped blind: it found every rule in its own suite was `undirected:
    /// true`, so a hardcoded `true` would have passed. The two arms here carry
    /// the SAME edge and differ only in the declared flag, so a `steps_from`
    /// that hardcodes either answer reddens one of them.
    ///
    /// The measured corpora cannot supply this input — all five blind-authored
    /// maps on record are `directed`, so an undirected map has no witness in
    /// authored data and the discriminating pair has to be built.
    #[test]
    fn steps_from_walks_a_back_edge_only_where_the_map_declares_undirected() {
        let edges = vec![edge("f-adj-stair-landing", "stair", "landing")];
        let directed = MapProjection::from_report(report(vec![view("m", false, edges.clone())]));
        let undirected = MapProjection::from_report(report(vec![view("m", true, edges)]));

        let d = directed.map("m").expect("the directed map");
        let u = undirected.map("m").expect("the undirected map");

        // Forward: both walk it, and both name the same declaring fact.
        let d_fwd = d.steps_from("stair");
        let u_fwd = u.steps_from("stair");
        assert_eq!(d_fwd.len(), 1, "directed forward");
        assert_eq!(u_fwd.len(), 1, "undirected forward");
        assert_eq!(d_fwd[0].1, "landing");
        assert_eq!(u_fwd[0].1, "landing");
        assert_eq!(d_fwd[0].0.fact_id, "f-adj-stair-landing");

        // Backward: the declaration decides, and the two answers differ.
        assert!(
            d.steps_from("landing").is_empty(),
            "a directed road is not walkable backwards"
        );
        let u_back = u.steps_from("landing");
        assert_eq!(u_back.len(), 1, "an undirected road is");
        assert_eq!(
            u_back[0].1, "stair",
            "and it reaches the far endpoint, not its own"
        );
    }

    /// Both side-table values survive the kernel's ingestion verbatim, including
    /// the shape Round 875's own injection flattened: a K-of-N threshold. The
    /// two guards differ only in `threshold`, so collapsing `Some(k)` into the
    /// canonical AND reddens exactly here.
    ///
    /// No authored corpus supplies this either: five blind authors wrote maps
    /// and not one used `edge_costs` or `edge_guards`, which is the same
    /// write-only silence Round 875 built the read for, seen from the far side.
    #[test]
    fn a_cost_and_a_k_of_n_guard_reach_the_kernel_unflattened() {
        let mut all = edge("f-adj-a-b", "a", "b");
        all.cost = Some(TransitionMapCost {
            n: 10,
            unit: "unit-minute".into(),
        });
        all.guard = Some(TransitionMapGuard {
            conditions: vec!["f-tide-out".into(), "f-gate-open".into()],
            threshold: None,
        });
        let mut k_of_n = edge("f-adj-b-c", "b", "c");
        k_of_n.guard = Some(TransitionMapGuard {
            conditions: vec!["f-tide-out".into(), "f-gate-open".into()],
            threshold: Some(1),
        });

        let projection = MapProjection::from_report(report(vec![view(
            "m",
            false,
            vec![all.clone(), k_of_n.clone()],
        )]));
        let m = projection.map("m").expect("the map");

        let cost = m.edges[0].cost.as_ref().expect("the authored cost");
        assert_eq!(cost.n, 10);
        assert_eq!(
            cost.unit, "unit-minute",
            "the unit is carried, not dropped — nothing here knows what it means"
        );
        assert!(
            m.edges[1].cost.is_none(),
            "an unauthored cost stays absent rather than defaulting to zero"
        );

        let and_guard = m.edges[0].guard.as_ref().expect("the AND guard");
        assert_eq!(and_guard.threshold, None, "no threshold = require ALL");
        let some_guard = m.edges[1].guard.as_ref().expect("the K-of-N guard");
        assert_eq!(
            some_guard.threshold,
            Some(1),
            "K-of-N must not flatten into the canonical AND"
        );
        assert_eq!(some_guard.conditions.len(), 2);
    }

    /// The two silences the read refuses to let read alike, carried through.
    #[test]
    fn self_loops_and_unattached_side_table_keys_are_named_not_dropped() {
        let mut m = view("m", false, vec![edge("f-adj-a-b", "a", "b")]);
        m.self_loops.push(TransitionMapSelfLoop {
            fact_id: "f-adj-a-a".into(),
            node: "a".into(),
        });
        let mut r = report(vec![m]);
        r.unattached_costs.push("f-not-an-edge".into());
        r.unattached_guards.push("f-also-not-an-edge".into());

        let projection = MapProjection::from_report(r);
        let map = projection.map("m").expect("the map");
        assert_eq!(map.self_loops.len(), 1);
        assert_eq!(map.self_loops[0].fact_id, "f-adj-a-a");
        assert!(
            !map.edges.iter().any(|e| e.fact_id == "f-adj-a-a"),
            "a self-loop is named, and still excluded from the edges"
        );
        assert!(
            map.steps_from("a")
                .iter()
                .all(|(e, _)| e.fact_id != "f-adj-a-a"),
            "and a self-loop is not a step out of its own node"
        );
        assert_eq!(projection.unattached_costs(), ["f-not-an-edge"]);
        assert_eq!(projection.unattached_guards(), ["f-also-not-an-edge"]);
    }

    /// `transition_rules: 0` is a THIRD state, not an empty map: with no rule
    /// there is no declared adjacency predicate, so the store cannot know which
    /// facts are edges at all (invariant 4). A kernel that let the two silences
    /// read alike would tell a consumer "this world has no roads" when the truth
    /// is "you never declared what a road is".
    #[test]
    fn no_rule_and_a_rule_whose_map_is_empty_are_different_answers() {
        let none = MapProjection::from_report(report(Vec::new()));
        let empty = MapProjection::from_report(report(vec![view("m", false, Vec::new())]));

        assert_eq!(none.transition_rules(), 0);
        assert!(none.maps().is_empty());

        assert_eq!(empty.transition_rules(), 1);
        assert_eq!(empty.maps().len(), 1, "the rule is declared");
        assert!(empty.maps()[0].edges.is_empty(), "and its map has no roads");
    }

    /// The bake round-trip loses nothing. The fixture is deliberately the
    /// awkward one — every optional field populated on one edge and absent on
    /// its neighbour, a K-of-N threshold, a self-loop, a containment predicate,
    /// and both unattached lists non-empty — because a round-trip over a
    /// fixture with no optional content passes while dropping every option.
    #[test]
    fn the_bake_round_trip_preserves_every_field_including_the_optional_ones() {
        let mut rich = edge("f-adj-a-b", "a", "b");
        rich.frame = "f-belief".into();
        rich.branch = "shatter".into();
        rich.cost = Some(TransitionMapCost {
            n: 5,
            unit: "unit-minute".into(),
        });
        rich.guard = Some(TransitionMapGuard {
            conditions: vec!["f-tide-out".into(), "f-key-held".into()],
            threshold: Some(1),
        });
        let bare = edge("f-adj-b-c", "b", "c");

        let mut m = view("m", true, vec![rich, bare]);
        m.containment = Some("inside".into());
        m.self_loops.push(TransitionMapSelfLoop {
            fact_id: "f-adj-c-c".into(),
            node: "c".into(),
        });
        let mut r = report(vec![m]);
        r.unattached_costs.push("f-stray-cost".into());
        r.unattached_guards.push("f-stray-guard".into());

        let projection = MapProjection::from_report(r);
        let parts = projection.to_parts();
        let reingested = MapProjection::from_parts(parts.clone());

        assert_eq!(
            reingested.to_parts(),
            parts,
            "a baked map re-ingests to the same parts"
        );

        // Byte-equal parts would also hold if BOTH directions dropped the same
        // field, so the assertions that matter read the far side directly.
        let m = reingested.map("m").expect("the map");
        assert_eq!(m.containment.as_deref(), Some("inside"));
        assert!(m.undirected);
        assert_eq!(m.self_loops.len(), 1);
        assert_eq!(m.edges[0].frame, "f-belief");
        assert_eq!(m.edges[0].branch, "shatter");
        let cost = m.edges[0]
            .cost
            .as_ref()
            .expect("the cost survived the bake");
        assert_eq!((cost.n, cost.unit.as_str()), (5, "unit-minute"));
        let guard = m.edges[0].guard.as_ref().expect("the guard survived");
        assert_eq!(guard.threshold, Some(1));
        assert_eq!(guard.conditions.len(), 2);
        assert!(m.edges[1].cost.is_none(), "and absence survived as absence");
        assert!(m.edges[1].guard.is_none());
        assert_eq!(reingested.unattached_costs(), ["f-stray-cost"]);
        assert_eq!(reingested.unattached_guards(), ["f-stray-guard"]);
    }
}

#[cfg(test)]
mod place_tests {
    use super::*;
    use crate::test_support::{begin, locator, report, scene};
    use crate::{DefaultOverrides, DisclosureMode, PlayableProjection};
    use mnemosyne_core::{TypedClaim, TypedObject};
    use mnemosyne_validate::continuity::{
        ForkTreeReport, TransitionMapEdge, TransitionMapReport, TransitionMapView,
    };

    /// A disclosed `at(subject, place)` fact.
    fn at_fact(
        fact_id: &str,
        subject: &str,
        place: &str,
    ) -> mnemosyne_validate::continuity::ManuscriptFactEvent {
        let mut event = begin(
            fact_id,
            "someone stands there",
            "ground-truth",
            &[subject, place],
        );
        event.typed = Some(TypedClaim {
            subject: subject.into(),
            predicate: "at".into(),
            object: TypedObject::Entity { id: place.into() },
        });
        event
    }

    fn town() -> MapProjection {
        let edges = vec![
            TransitionMapEdge {
                fact_id: "f-adj-market-stair".into(),
                from: "loc-market".into(),
                to: "loc-stair".into(),
                frame: "ground-truth".into(),
                branch: "main".into(),
                cost: None,
                guard: None,
            },
            TransitionMapEdge {
                fact_id: "f-adj-market-shrine".into(),
                from: "loc-market".into(),
                to: "loc-shrine".into(),
                frame: "ground-truth".into(),
                branch: "main".into(),
                cost: None,
                guard: None,
            },
        ];
        let mut nodes: Vec<String> =
            vec!["loc-market".into(), "loc-shrine".into(), "loc-stair".into()];
        nodes.sort();
        MapProjection::from_report(TransitionMapReport {
            maps: vec![TransitionMapView {
                rule: "town-map".into(),
                predicate: "at".into(),
                adjacency: "adjacent".into(),
                undirected: false,
                containment: None,
                nodes,
                edges,
                self_loops: Vec::new(),
            }],
            transition_rules: 1,
            unattached_costs: Vec::new(),
            unattached_guards: Vec::new(),
        })
    }

    /// Build a one-scene projection whose disclosed lines are exactly `facts`.
    fn projected(
        facts: Vec<mnemosyne_validate::continuity::ManuscriptFactEvent>,
    ) -> PlayableProjection {
        let locators = facts
            .iter()
            .map(|f| locator(&f.fact_id, "sc-01", DisclosureMode::State))
            .collect();
        PlayableProjection::from_report(
            report(
                "main",
                vec![scene("sc-01", "The market", facts)],
                locators,
                ForkTreeReport::default(),
            ),
            &DefaultOverrides::default(),
        )
        .expect("the projection")
    }

    fn scene_of(projection: &PlayableProjection) -> crate::SceneView {
        projection.scene("main", "sc-01", &std::collections::HashSet::new())
    }

    /// The join, on the shape authored data actually has: one `at` fact naming a
    /// person and a place, and only the place is a node of the map.
    #[test]
    fn a_scene_is_placed_by_the_disclosed_fact_that_puts_someone_there() {
        let map = town();
        let projection = projected(vec![at_fact("f-at-mirren", "ent-mirren", "loc-market")]);
        let here = map.places_disclosed_in(&scene_of(&projection));

        assert_eq!(here.len(), 1, "one place, not two: {here:?}");
        assert_eq!(here[0].place, "loc-market");
        assert_eq!(
            here[0].fact_id, "f-at-mirren",
            "the place names the fact that disclosed it"
        );
        assert_eq!(here[0].map.rule, "town-map");

        // And the exits come off the same edge list everything else reads.
        let mut exits: Vec<&str> = here[0]
            .map
            .steps_from(&here[0].place)
            .into_iter()
            .map(|(_, to)| to)
            .collect();
        exits.sort_unstable();
        assert_eq!(exits, ["loc-shrine", "loc-stair"]);
    }

    /// THE LEAK GUARD, and the reason this reads the scene rather than the gate.
    ///
    /// The continuity gate knows where everyone stands whether or not the telling
    /// says so. Here the `at` fact exists in the world and is NOT disclosed at
    /// this scene, and the honest answer is nothing — a reading surface that
    /// showed the place would disclose by the back door exactly what the
    /// disclosure plan withheld.
    ///
    /// The pair is what makes it a guard: the same fact, disclosed and withheld,
    /// with the map and the scene otherwise identical.
    #[test]
    fn a_withheld_at_fact_places_nothing_even_though_the_world_knows() {
        let map = town();
        let fact = at_fact("f-at-mirren", "ent-mirren", "loc-market");

        let disclosed = projected(vec![fact.clone()]);
        assert_eq!(
            map.places_disclosed_in(&scene_of(&disclosed)).len(),
            1,
            "the disclosed half must place, or the withheld half proves nothing"
        );

        // Same scene, same fact in the walk, NO locator — the store's additive
        // filter emitted none, so the kernel never sees it as a line.
        let withheld = PlayableProjection::from_report(
            report(
                "main",
                vec![scene("sc-01", "The market", vec![fact])],
                Vec::new(),
                ForkTreeReport::default(),
            ),
            &DefaultOverrides::default(),
        )
        .expect("the projection");
        assert!(
            map.places_disclosed_in(&scene_of(&withheld)).is_empty(),
            "a withheld position must not reach the surface"
        );
    }

    /// The predicate is the RULE's, not a guess. A disclosed fact about the same
    /// person and the same place under a different predicate is not a position.
    #[test]
    fn only_the_rules_own_predicate_places_a_scene() {
        let map = town();
        let mut wrong = at_fact("f-remembers", "ent-mirren", "loc-market");
        wrong.typed = Some(TypedClaim {
            subject: "ent-mirren".into(),
            predicate: "remembers".into(),
            object: TypedObject::Entity {
                id: "loc-market".into(),
            },
        });
        let projection = projected(vec![wrong]);
        assert!(
            map.places_disclosed_in(&scene_of(&projection)).is_empty(),
            "`remembers` is not `at`, however place-shaped its object is"
        );
    }

    /// An `at` fact naming a place the map does not have is not a position
    /// either — place-hood is membership in the declared node set, so the answer
    /// cannot drift from the roads.
    #[test]
    fn an_at_fact_naming_a_place_off_the_map_places_nothing() {
        let map = town();
        let projection = projected(vec![at_fact("f-at-elsewhere", "ent-mirren", "loc-nowhere")]);
        assert!(
            map.places_disclosed_in(&scene_of(&projection)).is_empty(),
            "a place with no road is not a node of this map"
        );
    }

    /// A scene cutting between two rooms yields BOTH, sorted, rather than one
    /// picked quietly — and two people in one room yield one, deduped.
    #[test]
    fn a_scene_in_two_rooms_names_both_and_two_people_in_one_name_it_once() {
        let map = town();
        let cut = projected(vec![
            at_fact("f-at-mirren", "ent-mirren", "loc-shrine"),
            at_fact("f-at-teo", "ent-teo", "loc-market"),
        ]);
        let here = map.places_disclosed_in(&scene_of(&cut));
        assert_eq!(
            here.iter().map(|d| d.place.as_str()).collect::<Vec<_>>(),
            ["loc-market", "loc-shrine"],
            "both, in sorted order"
        );

        let together = projected(vec![
            at_fact("f-at-mirren", "ent-mirren", "loc-market"),
            at_fact("f-at-teo", "ent-teo", "loc-market"),
        ]);
        let here = map.places_disclosed_in(&scene_of(&together));
        assert_eq!(here.len(), 1, "one room, named once: {here:?}");
    }

    /// A store with no transition rule places nothing, and that is the inert case
    /// working rather than the axis failing.
    #[test]
    fn a_store_with_no_declared_map_places_nothing() {
        let none = MapProjection::from_report(TransitionMapReport::default());
        let projection = projected(vec![at_fact("f-at-mirren", "ent-mirren", "loc-market")]);
        assert!(none.places_disclosed_in(&scene_of(&projection)).is_empty());
        assert_eq!(none.transition_rules(), 0);
    }
}
