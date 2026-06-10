//! Round 431 — frame-scoped continuity gate (Phase 1A Round B).
//!
//! Evaluates the RECORDED conflict assertion edges between narrative facts
//! (Round 430) frame-scoped: a same-frame conflicting pair whose canon
//! extents co-hold at some point is a violation; a CROSS-frame conflicting
//! pair is DATA, never gated (the North-Star sentence — frames are never
//! cross-validated — made executable).
//!
//! A fact's effective extent is DERIVED, never stored back: it starts at
//! `canon_from`, is bounded by a stored `canon_to` when present, and ends
//! the moment any in-frame successor (`supersedes_in_frame` pointing at it)
//! begins. A stored `canon_to` that lets the predecessor outlive its
//! successor's start is itself a violation (`SuccessionContradiction`).
//!
//! **Guardrail B-1 (design sec 7.9):** canon order is DECLARED, never
//! inferred. The order relation arrives as a consumer/medium-adapter
//! artifact (`canon-order/v1` edges = a partial order, DAG), the R426
//! verifies-catalog contract pattern. Section-id spelling is never
//! consulted; a pair whose canon coordinates are not comparable under the
//! declared order cannot overlap — surfaced as `unordered_pairs`, never
//! gated. Equality needs no declaration (a point always co-holds with
//! itself), so the gate is meaningful even with no order file.
//!
//! **Guardrail B-2:** the conflict scope key is the `same_scope` predicate
//! below — today `frame`, widening to `(frame, branch)` when the world-line
//! branch axis lands. Key-widening, not a rewrite.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use mnemosyne_atomic::AtomicStore;
use mnemosyne_core::NarrativeFact;
use serde::Deserialize;

/// The `canon-order/v1` contract — consumer/medium-adapter generated
/// (guardrail B-1: an explicit declaration, e.g. a chapter chain for a
/// linear novel, a quest DAG for a game). Extra JSON fields are ignored
/// (lenient, the epub-anchor-map precedent).
#[derive(Debug, Clone, Deserialize)]
pub struct CanonOrderFile {
    #[serde(default)]
    pub edges: Vec<[String; 2]>,
}

/// Reachability over the declared partial order: `le(a, b)` = `a == b` or a
/// declared path `a -> b`. Cycles are rejected at construction (an order
/// with a cycle is no order — fail loud).
#[derive(Debug, Clone)]
pub struct CanonOrder {
    /// node -> strict descendants (transitive closure of the edges).
    reach: BTreeMap<String, BTreeSet<String>>,
}

impl CanonOrder {
    /// No declaration: equality is the only comparability.
    pub fn empty() -> Self {
        Self {
            reach: BTreeMap::new(),
        }
    }

    pub fn from_edges(edges: &[[String; 2]]) -> Result<Self, String> {
        let mut adj: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for e in edges {
            let (a, b) = (e[0].trim(), e[1].trim());
            if a.is_empty() || b.is_empty() {
                return Err("canon-order: blank node in an edge".to_string());
            }
            if a == b {
                return Err(format!("canon-order: self-edge `{a}` (a cycle)"));
            }
            adj.entry(a).or_default().push(b);
            adj.entry(b).or_default();
        }
        // Transitive closure per node (BFS); a node reaching itself = cycle.
        let mut reach: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for &start in adj.keys() {
            let mut seen: BTreeSet<&str> = BTreeSet::new();
            let mut queue: Vec<&str> = adj[start].clone();
            while let Some(n) = queue.pop() {
                if seen.insert(n) {
                    queue.extend(adj.get(n).into_iter().flatten().copied());
                }
            }
            if seen.contains(start) {
                return Err(format!(
                    "canon-order: cycle through `{start}` — a cyclic declaration is no order"
                ));
            }
            reach.insert(
                start.to_string(),
                seen.into_iter().map(str::to_string).collect(),
            );
        }
        Ok(Self { reach })
    }

    /// Declared-or-equal precedence.
    pub fn le(&self, a: &str, b: &str) -> bool {
        a == b || self.reach.get(a).is_some_and(|d| d.contains(b))
    }

    /// Comparable under the declared order (either direction, or equal).
    pub fn comparable(&self, a: &str, b: &str) -> bool {
        self.le(a, b) || self.le(b, a)
    }

    pub fn node_count(&self) -> usize {
        self.reach.len()
    }

    /// Every node named by the declaration (for fail-loud section checks).
    pub fn nodes(&self) -> impl Iterator<Item = &str> {
        self.reach.keys().map(String::as_str)
    }
}

/// Load + construct a declared canon order, with the optional sha256 pin
/// (R428 pattern: the order is a gate-authority input; a configured pin
/// re-hashes every load and fails LOUDLY on mismatch).
pub fn load_canon_order(path: &Path, expected_sha256: Option<&str>) -> Result<CanonOrder, String> {
    let bytes =
        std::fs::read(path).map_err(|e| format!("canon-order read {}: {}", path.display(), e))?;
    if let Some(expected) = expected_sha256 {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(&bytes);
        let actual: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
        if actual != expected {
            return Err(format!(
                "canon-order sha256 mismatch at {}: pinned {} but file hashes {} — the \
                 declaration changed without a re-pin (or was tampered); re-generate, review, \
                 and update [continuity].canon_order_sha256",
                path.display(),
                expected,
                actual
            ));
        }
    }
    let parsed: CanonOrderFile = serde_json::from_slice(&bytes)
        .map_err(|e| format!("canon-order parse {}: {}", path.display(), e))?;
    CanonOrder::from_edges(&parsed.edges)
}

/// One continuity violation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContinuityViolation {
    /// Same-scope conflicting claims co-hold at canon point `at`.
    FrameConflictOverlap {
        frame: String,
        fact_a: String,
        fact_b: String,
        at: String,
    },
    /// A stored `canon_to` lets the predecessor outlive its successor's
    /// start — the stored end contradicts the derived one (design sec 7.3).
    SuccessionContradiction {
        frame: String,
        predecessor: String,
        successor: String,
        stored_to: String,
        successor_from: String,
    },
    /// `supersedes_in_frame` crosses frames (out-of-band edit; the write
    /// path rejects this — the scan re-checks, fail-loud).
    SuccessionCrossFrame {
        successor: String,
        predecessor: String,
        successor_frame: String,
        predecessor_frame: String,
    },
    /// A recorded edge names a fact that no longer exists (out-of-band
    /// edit; fail-loud).
    ConflictTargetMissing { fact_id: String, target: String },
    /// `supersedes_in_frame` names a fact that no longer exists.
    SuccessionTargetMissing { fact_id: String, target: String },
}

/// Scan result — pure data; severity/gating policy belongs to the caller.
#[derive(Debug, Clone, Default)]
pub struct ContinuityReport {
    pub violations: Vec<ContinuityViolation>,
    /// Distinct recorded conflict pairs evaluated.
    pub conflict_pairs_checked: usize,
    /// Conflicting pairs across DIFFERENT frames — data, never gated.
    pub cross_frame_pairs: usize,
    /// Same-scope pairs whose canon coordinates are not comparable under
    /// the declared order (B-1: surfaced, never gated).
    pub unordered_pairs: usize,
    pub facts: usize,
    pub order_nodes: usize,
}

/// B-2 scope predicate — the ONE place conflict scoping is decided. Today:
/// same epistemic frame. When the world-line branch axis lands (design sec
/// 7.9 axis 2), this widens to `(frame, branch)` — same-frame facts on
/// different world-lines never conflict.
fn same_scope(a: &NarrativeFact, b: &NarrativeFact) -> bool {
    a.frame == b.frame
}

/// Whether `fact` (id `fact_id`) holds at canon point `p` under the derived
/// extent: started (`canon_from <= p`), not past a stored `canon_to`, and
/// not yet replaced by any in-frame successor.
///
/// THE single holds-semantics — shared by the continuity gate and the
/// frame-at-T projection ([`frame_view`]) so the two can never drift (the
/// R390 single-predicate discipline).
fn holds_at(
    fact_id: &str,
    fact: &NarrativeFact,
    p: &str,
    order: &CanonOrder,
    successors: &BTreeMap<&str, Vec<&NarrativeFact>>,
) -> bool {
    if !order.le(&fact.canon_from, p) {
        return false;
    }
    if let Some(to) = &fact.canon_to {
        if !order.le(p, to) {
            return false;
        }
    }
    if let Some(succ) = successors.get(fact_id) {
        if succ.iter().any(|s| order.le(&s.canon_from, p)) {
            return false;
        }
    }
    true
}

/// Frame-scoped continuity scan over the narrative facts. Returns `Err` only
/// on a malformed input boundary (an order node that is not a section —
/// likely a typo in the declaration; fail loud). All data findings are
/// violations/counts in the report.
pub fn scan_continuity(
    store: &AtomicStore,
    order: &CanonOrder,
) -> Result<ContinuityReport, String> {
    for n in order.nodes() {
        if !store.sections.contains_key(n) {
            return Err(format!(
                "canon-order names `{n}`, which is not a section in the store — \
                 canon coordinates are structure refs; fix the declaration"
            ));
        }
    }
    let facts = &store.narrative_facts;
    let mut successors: BTreeMap<&str, Vec<&NarrativeFact>> = BTreeMap::new();
    for fact in facts.values() {
        if let Some(t) = &fact.supersedes_in_frame {
            successors.entry(t.as_str()).or_default().push(fact);
        }
    }
    let mut report = ContinuityReport {
        facts: facts.len(),
        order_nodes: order.node_count(),
        ..Default::default()
    };
    // Succession integrity (derived-extent preconditions).
    for (sid, s) in facts {
        if let Some(t_id) = &s.supersedes_in_frame {
            match facts.get(t_id) {
                None => report
                    .violations
                    .push(ContinuityViolation::SuccessionTargetMissing {
                        fact_id: sid.clone(),
                        target: t_id.clone(),
                    }),
                Some(t) if t.frame != s.frame => {
                    report
                        .violations
                        .push(ContinuityViolation::SuccessionCrossFrame {
                            successor: sid.clone(),
                            predecessor: t_id.clone(),
                            successor_frame: s.frame.clone(),
                            predecessor_frame: t.frame.clone(),
                        })
                }
                Some(t) => {
                    if let Some(stored_to) = &t.canon_to {
                        if order.le(&s.canon_from, stored_to) {
                            report
                                .violations
                                .push(ContinuityViolation::SuccessionContradiction {
                                    frame: s.frame.clone(),
                                    predecessor: t_id.clone(),
                                    successor: sid.clone(),
                                    stored_to: stored_to.clone(),
                                    successor_from: s.canon_from.clone(),
                                });
                        }
                    }
                }
            }
        }
    }
    // Distinct recorded conflict pairs (edges are read symmetrically).
    let mut pairs: BTreeSet<(String, String)> = BTreeSet::new();
    for (aid, a) in facts {
        for target in &a.conflicts_with {
            if !facts.contains_key(target) {
                report
                    .violations
                    .push(ContinuityViolation::ConflictTargetMissing {
                        fact_id: aid.clone(),
                        target: target.clone(),
                    });
                continue;
            }
            let key = if aid < target {
                (aid.clone(), target.clone())
            } else {
                (target.clone(), aid.clone())
            };
            pairs.insert(key);
        }
    }
    report.conflict_pairs_checked = pairs.len();
    for (aid, bid) in &pairs {
        let (a, b) = (&facts[aid], &facts[bid]);
        if !same_scope(a, b) {
            report.cross_frame_pairs += 1;
            continue;
        }
        let co_hold = store.sections.keys().find(|p| {
            holds_at(aid, a, p, order, &successors) && holds_at(bid, b, p, order, &successors)
        });
        match co_hold {
            Some(p) => report
                .violations
                .push(ContinuityViolation::FrameConflictOverlap {
                    frame: a.frame.clone(),
                    fact_a: aid.clone(),
                    fact_b: bid.clone(),
                    at: p.clone(),
                }),
            None => {
                if !order.comparable(&a.canon_from, &b.canon_from) {
                    report.unordered_pairs += 1;
                }
            }
        }
    }
    Ok(report)
}

/// One fact currently in effect in a frame view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameViewEntry {
    pub fact_id: String,
    pub claim: String,
    pub canon_from: String,
    pub canon_to: Option<String>,
    pub evidence: Vec<String>,
    pub quote: Option<String>,
}

/// The frame-at-T projection result (Round 432). Three-state honest under a
/// partial order (B-1): a fact is `holding`, definitively `not_holding`
/// (counted), or `unknown` — some canon coordinate involved is not
/// comparable to the query point, so the declaration cannot decide.
#[derive(Debug, Clone, Default)]
pub struct FrameView {
    pub frame: String,
    pub at: String,
    pub holding: Vec<FrameViewEntry>,
    pub not_holding: usize,
    pub unknown: Vec<String>,
}

/// "Facts of frame F at canon point T" — the read projection over the SAME
/// `holds_at` semantics the continuity gate uses (R390 single-predicate
/// discipline: gate and view cannot drift). Fail-loud boundaries: the frame
/// must be registered, the query point must be a section, and the order
/// declaration must name only sections.
pub fn frame_view(
    store: &AtomicStore,
    order: &CanonOrder,
    frame: &str,
    at: &str,
) -> Result<FrameView, String> {
    for n in order.nodes() {
        if !store.sections.contains_key(n) {
            return Err(format!(
                "canon-order names `{n}`, which is not a section in the store — \
                 canon coordinates are structure refs; fix the declaration"
            ));
        }
    }
    if !store.frames.contains_key(frame) {
        return Err(format!(
            "frame `{frame}` not present in the frames registry (fail-loud)"
        ));
    }
    if !store.sections.contains_key(at) {
        return Err(format!(
            "query point `{at}` not present as a section (canon coordinates are structure refs)"
        ));
    }
    let facts = &store.narrative_facts;
    let mut successors: BTreeMap<&str, Vec<&NarrativeFact>> = BTreeMap::new();
    for fact in facts.values() {
        if let Some(t) = &fact.supersedes_in_frame {
            successors.entry(t.as_str()).or_default().push(fact);
        }
    }
    let mut view = FrameView {
        frame: frame.to_string(),
        at: at.to_string(),
        ..Default::default()
    };
    for (id, fact) in facts {
        if fact.frame != frame {
            continue;
        }
        if holds_at(id, fact, at, order, &successors) {
            view.holding.push(FrameViewEntry {
                fact_id: id.clone(),
                claim: fact.claim.clone(),
                canon_from: fact.canon_from.clone(),
                canon_to: fact.canon_to.clone(),
                evidence: fact.evidence.clone(),
                quote: fact.quote.clone(),
            });
            continue;
        }
        // Not holding — definitive vs unknown (B-1 honesty): if a coordinate
        // the verdict depended on is not comparable to `at`, the declared
        // order cannot actually decide it.
        let from_unknown = !order.comparable(&fact.canon_from, at);
        let to_unknown = order.le(&fact.canon_from, at)
            && fact
                .canon_to
                .as_ref()
                .is_some_and(|to| !order.comparable(at, to));
        let succ_cut = successors
            .get(id.as_str())
            .into_iter()
            .flatten()
            .any(|s| order.le(&s.canon_from, at));
        if from_unknown || (to_unknown && !succ_cut) {
            view.unknown.push(id.clone());
        } else {
            view.not_holding += 1;
        }
    }
    Ok(view)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mnemosyne_atomic::{AtomicSection, FactImport, FactsManifest};

    fn chain(ids: &[&str]) -> CanonOrder {
        let edges: Vec<[String; 2]> = ids
            .windows(2)
            .map(|w| [w[0].to_string(), w[1].to_string()])
            .collect();
        CanonOrder::from_edges(&edges).unwrap()
    }

    fn fact(id: &str, frame: &str, from: &str, to: Option<&str>) -> FactImport {
        FactImport {
            fact_id: id.to_string(),
            frame: frame.to_string(),
            claim: format!("claim {id}"),
            canon_from: from.to_string(),
            canon_to: to.map(str::to_string),
            evidence: vec![from.to_string()],
            conflicts_with: vec![],
            supersedes_in_frame: None,
            quote: None,
        }
    }

    /// Store with sections ch-1..ch-4 and the given facts, built through the
    /// REAL import primitive (same invariants as production writes).
    fn store_with(facts: Vec<FactImport>) -> AtomicStore {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("s.json");
        let mut store = AtomicStore::new();
        for ch in ["ch-1", "ch-2", "ch-3", "ch-4"] {
            store
                .sections
                .insert(ch.to_string(), AtomicSection::default());
        }
        let frames = facts
            .iter()
            .map(|f| f.frame.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|frame_id| mnemosyne_atomic::FrameImport {
                frame_id,
                description: String::new(),
            })
            .collect();
        mnemosyne_atomic::import_facts(&mut store, &path, &FactsManifest { frames, facts })
            .unwrap();
        store
    }

    #[test]
    fn same_frame_overlapping_conflict_is_a_violation() {
        let mut a = fact("fa", "seward", "ch-1", Some("ch-3"));
        let b = fact("fb", "seward", "ch-2", None);
        a.conflicts_with = vec!["fb".to_string()];
        let store = store_with(vec![a, b]);
        let report = scan_continuity(&store, &chain(&["ch-1", "ch-2", "ch-3", "ch-4"])).unwrap();
        assert_eq!(report.conflict_pairs_checked, 1);
        assert_eq!(report.violations.len(), 1);
        match &report.violations[0] {
            ContinuityViolation::FrameConflictOverlap { frame, at, .. } => {
                assert_eq!(frame, "seward");
                assert!(at == "ch-2" || at == "ch-3");
            }
            v => panic!("wrong violation: {v:?}"),
        }
    }

    #[test]
    fn cross_frame_conflict_is_data_never_gated() {
        // Acceptance shape 1 (design sec 7.7): seward vs van-helsing on the
        // same canon window — recorded conflict, ZERO violations.
        let mut a = fact("f-illness", "seward", "ch-1", Some("ch-3"));
        let b = fact("f-vampire", "van-helsing", "ch-2", None);
        a.conflicts_with = vec!["f-vampire".to_string()];
        let store = store_with(vec![a, b]);
        let report = scan_continuity(&store, &chain(&["ch-1", "ch-2", "ch-3", "ch-4"])).unwrap();
        assert!(report.violations.is_empty());
        assert_eq!(report.cross_frame_pairs, 1);
    }

    #[test]
    fn derived_closure_from_succession_prevents_overlap() {
        // Acceptance shape 3: predecessor has NO stored end; the successor's
        // start ends it. A conflict against a post-succession fact never
        // co-holds.
        let old = fact("f-old", "jonathan", "ch-1", None);
        let mut new = fact("f-new", "jonathan", "ch-3", None);
        new.supersedes_in_frame = Some("f-old".to_string());
        let mut late = fact("f-late", "jonathan", "ch-3", None);
        late.conflicts_with = vec!["f-old".to_string()];
        let store = store_with(vec![old, new, late]);
        let report = scan_continuity(&store, &chain(&["ch-1", "ch-2", "ch-3", "ch-4"])).unwrap();
        assert!(
            report.violations.is_empty(),
            "f-old is derived-closed at ch-3: {:?}",
            report.violations
        );
        // Without the successor the same pair DOES overlap (control).
        let old = fact("f-old", "jonathan", "ch-1", None);
        let mut late = fact("f-late", "jonathan", "ch-3", None);
        late.conflicts_with = vec!["f-old".to_string()];
        let store = store_with(vec![old, late]);
        let report = scan_continuity(&store, &chain(&["ch-1", "ch-2", "ch-3", "ch-4"])).unwrap();
        assert_eq!(report.violations.len(), 1);
    }

    #[test]
    fn succession_contradiction_stored_to_outlives_successor() {
        let old = fact("f-old", "jonathan", "ch-1", Some("ch-3"));
        let mut new = fact("f-new", "jonathan", "ch-2", None);
        new.supersedes_in_frame = Some("f-old".to_string());
        let store = store_with(vec![old, new]);
        let report = scan_continuity(&store, &chain(&["ch-1", "ch-2", "ch-3", "ch-4"])).unwrap();
        assert_eq!(report.violations.len(), 1);
        assert!(matches!(
            &report.violations[0],
            ContinuityViolation::SuccessionContradiction { stored_to, successor_from, .. }
                if stored_to == "ch-3" && successor_from == "ch-2"
        ));
    }

    #[test]
    fn undeclared_order_makes_pairs_unordered_not_violations() {
        // B-1: no declaration + distinct canon_from = not comparable —
        // surfaced as unordered, never gated.
        let mut a = fact("fa", "seward", "ch-1", None);
        let b = fact("fb", "seward", "ch-2", None);
        a.conflicts_with = vec!["fb".to_string()];
        let store = store_with(vec![a, b]);
        let report = scan_continuity(&store, &CanonOrder::empty()).unwrap();
        assert!(report.violations.is_empty());
        assert_eq!(report.unordered_pairs, 1);
    }

    #[test]
    fn equal_canon_from_needs_no_declaration() {
        // Equality is order-free comparability: same start co-holds.
        let mut a = fact("fa", "seward", "ch-2", None);
        let b = fact("fb", "seward", "ch-2", None);
        a.conflicts_with = vec!["fb".to_string()];
        let store = store_with(vec![a, b]);
        let report = scan_continuity(&store, &CanonOrder::empty()).unwrap();
        assert_eq!(report.violations.len(), 1);
        assert_eq!(report.unordered_pairs, 0);
    }

    #[test]
    fn dag_incomparable_branches_do_not_overlap() {
        // B-1 quest-DAG shape: ch-1 -> ch-2 and ch-1 -> ch-3 (ch-2 vs ch-3
        // incomparable). Conflicting facts on the two arms: no violation,
        // surfaced unordered.
        let order = CanonOrder::from_edges(&[
            ["ch-1".to_string(), "ch-2".to_string()],
            ["ch-1".to_string(), "ch-3".to_string()],
        ])
        .unwrap();
        let mut a = fact("fa", "seward", "ch-2", None);
        let b = fact("fb", "seward", "ch-3", None);
        a.conflicts_with = vec!["fb".to_string()];
        let store = store_with(vec![a, b]);
        let report = scan_continuity(&store, &order).unwrap();
        assert!(report.violations.is_empty());
        assert_eq!(report.unordered_pairs, 1);
    }

    #[test]
    fn cyclic_declaration_rejected_loud() {
        let err = CanonOrder::from_edges(&[
            ["a".to_string(), "b".to_string()],
            ["b".to_string(), "a".to_string()],
        ])
        .unwrap_err();
        assert!(err.contains("cycle"), "{err}");
    }

    #[test]
    fn order_node_must_be_a_section() {
        let store = store_with(vec![]);
        let err = scan_continuity(&store, &chain(&["ch-1", "ch-99"])).unwrap_err();
        assert!(err.contains("ch-99"), "{err}");
    }
    // ── frame_view (Round 432) ──────────────────────────────────────────

    #[test]
    fn frame_view_succession_swaps_the_held_belief() {
        // jonathan at ch-2: f-old holds; at ch-3: f-new (derived closure).
        let old = fact("f-old", "jonathan", "ch-1", None);
        let mut new = fact("f-new", "jonathan", "ch-3", None);
        new.supersedes_in_frame = Some("f-old".to_string());
        let store = store_with(vec![old, new]);
        let order = chain(&["ch-1", "ch-2", "ch-3", "ch-4"]);
        let at2 = frame_view(&store, &order, "jonathan", "ch-2").unwrap();
        assert_eq!(
            at2.holding
                .iter()
                .map(|e| e.fact_id.as_str())
                .collect::<Vec<_>>(),
            vec!["f-old"]
        );
        assert_eq!(at2.not_holding, 1);
        let at3 = frame_view(&store, &order, "jonathan", "ch-3").unwrap();
        assert_eq!(
            at3.holding
                .iter()
                .map(|e| e.fact_id.as_str())
                .collect::<Vec<_>>(),
            vec!["f-new"]
        );
        assert_eq!(at3.not_holding, 1);
        assert!(at3.unknown.is_empty());
    }

    #[test]
    fn frame_view_stored_to_ends_and_other_frames_excluded() {
        let bounded = fact("f-b", "seward", "ch-1", Some("ch-2"));
        let other = fact("f-x", "jonathan", "ch-1", None);
        let store = store_with(vec![bounded, other]);
        let order = chain(&["ch-1", "ch-2", "ch-3", "ch-4"]);
        let at3 = frame_view(&store, &order, "seward", "ch-3").unwrap();
        assert!(at3.holding.is_empty());
        assert_eq!(at3.not_holding, 1);
        // jonathan's fact never appears in seward's view.
        let at1 = frame_view(&store, &order, "seward", "ch-1").unwrap();
        assert_eq!(at1.holding.len(), 1);
        assert_eq!(at1.holding[0].fact_id, "f-b");
    }

    #[test]
    fn frame_view_incomparable_is_unknown_not_absent() {
        // Quest-DAG: ch-1 -> ch-2, ch-1 -> ch-3. A fact on the ch-2 arm
        // queried at ch-3 is UNKNOWN (B-1), not silently "not holding".
        let order = CanonOrder::from_edges(&[
            ["ch-1".to_string(), "ch-2".to_string()],
            ["ch-1".to_string(), "ch-3".to_string()],
        ])
        .unwrap();
        let store = store_with(vec![fact("f-arm", "seward", "ch-2", None)]);
        let view = frame_view(&store, &order, "seward", "ch-3").unwrap();
        assert!(view.holding.is_empty());
        assert_eq!(view.unknown, vec!["f-arm".to_string()]);
        assert_eq!(view.not_holding, 0);
    }

    #[test]
    fn frame_view_fail_loud_boundaries() {
        let store = store_with(vec![fact("f1", "seward", "ch-1", None)]);
        let order = chain(&["ch-1", "ch-2"]);
        let err = frame_view(&store, &order, "nobody", "ch-1").unwrap_err();
        assert!(err.contains("frames registry"), "{err}");
        let err = frame_view(&store, &order, "seward", "ch-99").unwrap_err();
        assert!(err.contains("ch-99"), "{err}");
    }

    /// R390-style consistency lock: the gate and the view share holds_at —
    /// any FrameConflictOverlap the gate reports at point `at` MUST show
    /// both facts holding in that frame's view at `at`.
    #[test]
    fn gate_and_view_agree_on_co_holding() {
        let mut a = fact("fa", "seward", "ch-1", Some("ch-3"));
        let b = fact("fb", "seward", "ch-2", None);
        a.conflicts_with = vec!["fb".to_string()];
        let store = store_with(vec![a, b]);
        let order = chain(&["ch-1", "ch-2", "ch-3", "ch-4"]);
        let report = scan_continuity(&store, &order).unwrap();
        let ContinuityViolation::FrameConflictOverlap {
            frame,
            fact_a,
            fact_b,
            at,
        } = &report.violations[0]
        else {
            panic!("expected overlap");
        };
        let view = frame_view(&store, &order, frame, at).unwrap();
        let held: Vec<&str> = view.holding.iter().map(|e| e.fact_id.as_str()).collect();
        assert!(held.contains(&fact_a.as_str()) && held.contains(&fact_b.as_str()));
    }
}
