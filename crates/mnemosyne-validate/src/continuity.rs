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
//! **Guardrail B-2 (landed, Round 433):** the conflict scope key is the
//! `same_scope` predicate below — `(frame, branch)` since the world-line
//! branch axis landed. Same-frame facts on different world-lines never
//! conflict (cross-branch pairs are data, exactly like cross-frame pairs),
//! and the canon order is branch-relative: the declaration may carry
//! per-branch edge sets (`branches`), each composed with the shared `edges`
//! base — the same quest node can legitimately order differently on two
//! world-lines.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use mnemosyne_atomic::AtomicStore;
use mnemosyne_core::NarrativeFact;
use serde::Deserialize;

/// The `canon-order/v1` contract — consumer/medium-adapter generated
/// (guardrail B-1: an explicit declaration, e.g. a chapter chain for a
/// linear novel, a quest DAG for a game). Extra JSON fields are ignored
/// (lenient, the epub-anchor-map precedent).
///
/// `branches` (Round 433, guardrail B-2) declares per-world-line edge sets:
/// each branch's order = the shared `edges` base composed with its own
/// edges. Branch-relative order is the point — the same quest node can
/// legitimately precede X on one world-line and follow it on another, which
/// a single global DAG cannot express (it would be a cycle). A branch
/// absent from `branches` orders by the base alone.
#[derive(Debug, Clone, Deserialize)]
pub struct CanonOrderFile {
    #[serde(default)]
    pub edges: Vec<[String; 2]>,
    #[serde(default)]
    pub branches: BTreeMap<String, Vec<[String; 2]>>,
}

/// node -> strict descendants (transitive closure of the edges); a node
/// reaching itself = cycle = no order, fail loud. `label` names the edge set
/// in errors (the base or a branch).
fn closure_of(
    edges: &[[String; 2]],
    label: &str,
) -> Result<BTreeMap<String, BTreeSet<String>>, String> {
    let mut adj: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for e in edges {
        let (a, b) = (e[0].trim(), e[1].trim());
        if a.is_empty() || b.is_empty() {
            return Err(format!("canon-order ({label}): blank node in an edge"));
        }
        if a == b {
            return Err(format!("canon-order ({label}): self-edge `{a}` (a cycle)"));
        }
        adj.entry(a).or_default().push(b);
        adj.entry(b).or_default();
    }
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
                "canon-order ({label}): cycle through `{start}` — a cyclic declaration is no order"
            ));
        }
        reach.insert(
            start.to_string(),
            seen.into_iter().map(str::to_string).collect(),
        );
    }
    Ok(reach)
}

/// Reachability over the declared partial order: `le(branch, a, b)` =
/// `a == b` or a declared path `a -> b` under that branch's order (its own
/// edges composed with the base; undeclared branch = base alone). Cycles are
/// rejected at construction — per edge set, base ∪ branch combined (an order
/// with a cycle is no order — fail loud).
#[derive(Debug, Clone)]
pub struct CanonOrder {
    /// Closure of the shared `edges` base.
    base: BTreeMap<String, BTreeSet<String>>,
    /// Per-branch closure of (base ∪ branch edges), keyed by branch id.
    branch_reach: BTreeMap<String, BTreeMap<String, BTreeSet<String>>>,
}

impl CanonOrder {
    /// No declaration: equality is the only comparability.
    pub fn empty() -> Self {
        Self {
            base: BTreeMap::new(),
            branch_reach: BTreeMap::new(),
        }
    }

    /// Base-only order (no per-branch edge sets) — every branch orders by it.
    pub fn from_edges(edges: &[[String; 2]]) -> Result<Self, String> {
        Self::from_declaration(&CanonOrderFile {
            edges: edges.to_vec(),
            branches: BTreeMap::new(),
        })
    }

    pub fn from_declaration(decl: &CanonOrderFile) -> Result<Self, String> {
        let base = closure_of(&decl.edges, "base")?;
        let mut branch_reach = BTreeMap::new();
        for (branch, edges) in &decl.branches {
            let branch = branch.trim();
            if branch.is_empty() {
                return Err("canon-order: blank branch id in `branches`".to_string());
            }
            let mut combined = decl.edges.clone();
            combined.extend(edges.iter().cloned());
            branch_reach.insert(
                branch.to_string(),
                closure_of(&combined, &format!("branch `{branch}`"))?,
            );
        }
        Ok(Self { base, branch_reach })
    }

    /// The reach relation governing `branch` — its declared composition, or
    /// the base for an undeclared branch.
    fn reach_for(&self, branch: &str) -> &BTreeMap<String, BTreeSet<String>> {
        self.branch_reach.get(branch).unwrap_or(&self.base)
    }

    /// Declared-or-equal precedence under `branch`'s order.
    pub fn le(&self, branch: &str, a: &str, b: &str) -> bool {
        a == b || self.reach_for(branch).get(a).is_some_and(|d| d.contains(b))
    }

    /// Comparable under `branch`'s declared order (either direction, or equal).
    pub fn comparable(&self, branch: &str, a: &str, b: &str) -> bool {
        self.le(branch, a, b) || self.le(branch, b, a)
    }

    /// Distinct nodes named anywhere in the declaration (base or branches).
    pub fn node_count(&self) -> usize {
        self.nodes().collect::<BTreeSet<_>>().len()
    }

    /// Every node named by the declaration (for fail-loud section checks).
    pub fn nodes(&self) -> impl Iterator<Item = &str> {
        self.base
            .keys()
            .chain(self.branch_reach.values().flat_map(BTreeMap::keys))
            .map(String::as_str)
    }

    /// Branch ids carrying a declared edge set.
    pub fn declared_branches(&self) -> impl Iterator<Item = &str> {
        self.branch_reach.keys().map(String::as_str)
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
    CanonOrder::from_declaration(&parsed)
}

/// One continuity violation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContinuityViolation {
    /// Same-scope conflicting claims co-hold at canon point `at`. Scope =
    /// `(frame, branch)` (Round 433).
    FrameConflictOverlap {
        frame: String,
        branch: String,
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
    /// `supersedes_in_frame` crosses world-lines (Round 433; out-of-band
    /// edit — the write path rejects this, the scan re-checks, fail-loud).
    SuccessionCrossBranch {
        successor: String,
        predecessor: String,
        successor_branch: String,
        predecessor_branch: String,
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
    /// Conflicting pairs across DIFFERENT scopes — a different frame or a
    /// different world-line branch (Round 433) — data, never gated.
    pub cross_scope_pairs: usize,
    /// Same-scope pairs whose canon coordinates are not comparable under
    /// the declared order (B-1: surfaced, never gated).
    pub unordered_pairs: usize,
    pub facts: usize,
    pub order_nodes: usize,
}

/// B-2 scope predicate — the ONE place conflict scoping is decided:
/// `(frame, branch)` since Round 433. Same-frame facts on different
/// world-lines never conflict (branch divergence is data, like frame
/// divergence).
fn same_scope(a: &NarrativeFact, b: &NarrativeFact) -> bool {
    a.frame == b.frame && a.branch == b.branch
}

/// Whether `fact` (id `fact_id`) holds at canon point `p` under the derived
/// extent: started (`canon_from <= p`), not past a stored `canon_to`, and
/// not yet replaced by any in-frame successor. All precedence is evaluated
/// under the fact's OWN branch order (Round 433: canon order is
/// branch-relative; successors are same-scope by write-path invariant).
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
    if !order.le(&fact.branch, &fact.canon_from, p) {
        return false;
    }
    if let Some(to) = &fact.canon_to {
        if !order.le(&fact.branch, p, to) {
            return false;
        }
    }
    if let Some(succ) = successors.get(fact_id) {
        if succ
            .iter()
            .any(|s| order.le(&fact.branch, &s.canon_from, p))
        {
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
                Some(t) if t.branch != s.branch => {
                    report
                        .violations
                        .push(ContinuityViolation::SuccessionCrossBranch {
                            successor: sid.clone(),
                            predecessor: t_id.clone(),
                            successor_branch: s.branch.clone(),
                            predecessor_branch: t.branch.clone(),
                        })
                }
                Some(t) => {
                    if let Some(stored_to) = &t.canon_to {
                        if order.le(&s.branch, &s.canon_from, stored_to) {
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
            report.cross_scope_pairs += 1;
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
                    branch: a.branch.clone(),
                    fact_a: aid.clone(),
                    fact_b: bid.clone(),
                    at: p.clone(),
                }),
            None => {
                if !order.comparable(&a.branch, &a.canon_from, &b.canon_from) {
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
/// comparable to the query point, so the declaration cannot decide. Scoped
/// to one world-line (`branch`, Round 433) — a view never mixes branches.
#[derive(Debug, Clone, Default)]
pub struct FrameView {
    pub frame: String,
    pub branch: String,
    pub at: String,
    pub holding: Vec<FrameViewEntry>,
    pub not_holding: usize,
    pub unknown: Vec<String>,
}

/// "Facts of frame F on branch B at canon point T" — the read projection
/// over the SAME `holds_at` semantics the continuity gate uses (R390
/// single-predicate discipline: gate and view cannot drift). Fail-loud
/// boundaries: the frame must be registered, the query point must be a
/// section, the order declaration must name only sections, and the branch
/// must be KNOWN — `MAIN_BRANCH`, carried by some fact, or declared in the
/// order (there is no branch registry; this derived check is what keeps a
/// typo'd branch from reading as an empty world).
pub fn frame_view(
    store: &AtomicStore,
    order: &CanonOrder,
    frame: &str,
    branch: &str,
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
    let branch_known = branch == mnemosyne_core::MAIN_BRANCH
        || store.narrative_facts.values().any(|f| f.branch == branch)
        || order.declared_branches().any(|b| b == branch);
    if !branch_known {
        return Err(format!(
            "branch `{branch}` unknown — no fact carries it and the canon-order declaration \
             has no edge set for it (a typo'd branch must not read as an empty world)"
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
        branch: branch.to_string(),
        at: at.to_string(),
        ..Default::default()
    };
    for (id, fact) in facts {
        if fact.frame != frame || fact.branch != branch {
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
        let from_unknown = !order.comparable(branch, &fact.canon_from, at);
        let to_unknown = order.le(branch, &fact.canon_from, at)
            && fact
                .canon_to
                .as_ref()
                .is_some_and(|to| !order.comparable(branch, at, to));
        let succ_cut = successors
            .get(id.as_str())
            .into_iter()
            .flatten()
            .any(|s| order.le(branch, &s.canon_from, at));
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
    use mnemosyne_core::MAIN_BRANCH;

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
            branch: None,
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
        assert_eq!(report.cross_scope_pairs, 1);
    }

    #[test]
    fn same_frame_cross_branch_conflict_is_data_never_gated() {
        // B-2 (Round 433): same frame, different world-lines, overlapping
        // extents, recorded conflict — data, zero violations.
        let mut a = fact("f-castle", "jonathan", "ch-1", None);
        a.conflicts_with = vec!["f-ship".to_string()];
        let mut b = fact("f-ship", "jonathan", "ch-1", None);
        b.branch = Some("sea-route".to_string());
        let store = store_with(vec![a, b]);
        let report = scan_continuity(&store, &chain(&["ch-1", "ch-2", "ch-3", "ch-4"])).unwrap();
        assert!(report.violations.is_empty(), "{:?}", report.violations);
        assert_eq!(report.cross_scope_pairs, 1);
    }

    #[test]
    fn per_branch_order_gates_each_world_line_under_its_own_order() {
        // Round 433 + B-1: ch-2 precedes ch-3 on branch `a`, ch-3 precedes
        // ch-2 on branch `b` — inexpressible as one global DAG (cycle). The
        // same fact shapes (2..2 point vs 3..) co-hold only under `b`'s
        // order, so exactly the `b` pair violates.
        let decl = CanonOrderFile {
            edges: vec![],
            branches: BTreeMap::from([
                (
                    "a".to_string(),
                    vec![["ch-2".to_string(), "ch-3".to_string()]],
                ),
                (
                    "b".to_string(),
                    vec![["ch-3".to_string(), "ch-2".to_string()]],
                ),
            ]),
        };
        let order = CanonOrder::from_declaration(&decl).unwrap();
        let mk = |id: &str, branch: &str, from: &str, to: Option<&str>| {
            let mut f = fact(id, "seward", from, to);
            f.branch = Some(branch.to_string());
            f
        };
        let mut fa_a = mk("fa-a", "a", "ch-2", Some("ch-2"));
        fa_a.conflicts_with = vec!["fb-a".to_string()];
        let fb_a = mk("fb-a", "a", "ch-3", None);
        let mut fa_b = mk("fa-b", "b", "ch-2", Some("ch-2"));
        fa_b.conflicts_with = vec!["fb-b".to_string()];
        let fb_b = mk("fb-b", "b", "ch-3", None);
        let store = store_with(vec![fa_a, fb_a, fa_b, fb_b]);
        let report = scan_continuity(&store, &order).unwrap();
        assert_eq!(report.violations.len(), 1, "{:?}", report.violations);
        match &report.violations[0] {
            ContinuityViolation::FrameConflictOverlap {
                branch,
                fact_a,
                fact_b,
                ..
            } => {
                assert_eq!(branch, "b");
                assert_eq!(fact_a, "fa-b");
                assert_eq!(fact_b, "fb-b");
            }
            v => panic!("wrong violation: {v:?}"),
        }
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
        let at2 = frame_view(&store, &order, "jonathan", MAIN_BRANCH, "ch-2").unwrap();
        assert_eq!(
            at2.holding
                .iter()
                .map(|e| e.fact_id.as_str())
                .collect::<Vec<_>>(),
            vec!["f-old"]
        );
        assert_eq!(at2.not_holding, 1);
        let at3 = frame_view(&store, &order, "jonathan", MAIN_BRANCH, "ch-3").unwrap();
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
        let at3 = frame_view(&store, &order, "seward", MAIN_BRANCH, "ch-3").unwrap();
        assert!(at3.holding.is_empty());
        assert_eq!(at3.not_holding, 1);
        // jonathan's fact never appears in seward's view.
        let at1 = frame_view(&store, &order, "seward", MAIN_BRANCH, "ch-1").unwrap();
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
        let view = frame_view(&store, &order, "seward", MAIN_BRANCH, "ch-3").unwrap();
        assert!(view.holding.is_empty());
        assert_eq!(view.unknown, vec!["f-arm".to_string()]);
        assert_eq!(view.not_holding, 0);
    }

    #[test]
    fn frame_view_fail_loud_boundaries() {
        let store = store_with(vec![fact("f1", "seward", "ch-1", None)]);
        let order = chain(&["ch-1", "ch-2"]);
        let err = frame_view(&store, &order, "nobody", MAIN_BRANCH, "ch-1").unwrap_err();
        assert!(err.contains("frames registry"), "{err}");
        let err = frame_view(&store, &order, "seward", MAIN_BRANCH, "ch-99").unwrap_err();
        assert!(err.contains("ch-99"), "{err}");
    }

    #[test]
    fn frame_view_scopes_to_one_world_line() {
        // Round 433: a view never mixes branches — same frame, two
        // world-lines, each view sees only its own.
        let on_main = fact("f-main", "jonathan", "ch-1", None);
        let mut on_route = fact("f-route", "jonathan", "ch-1", None);
        on_route.branch = Some("sea-route".to_string());
        let store = store_with(vec![on_main, on_route]);
        let order = chain(&["ch-1", "ch-2"]);
        let main_view = frame_view(&store, &order, "jonathan", MAIN_BRANCH, "ch-2").unwrap();
        assert_eq!(main_view.holding.len(), 1);
        assert_eq!(main_view.holding[0].fact_id, "f-main");
        assert_eq!(main_view.branch, MAIN_BRANCH);
        let route_view = frame_view(&store, &order, "jonathan", "sea-route", "ch-2").unwrap();
        assert_eq!(route_view.holding.len(), 1);
        assert_eq!(route_view.holding[0].fact_id, "f-route");
        // Unknown branch fails loud — a typo must not read as an empty world.
        let err = frame_view(&store, &order, "jonathan", "sea-rotue", "ch-2").unwrap_err();
        assert!(err.contains("unknown"), "{err}");
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
            branch,
            fact_a,
            fact_b,
            at,
        } = &report.violations[0]
        else {
            panic!("expected overlap");
        };
        let view = frame_view(&store, &order, frame, branch, at).unwrap();
        let held: Vec<&str> = view.holding.iter().map(|e| e.fact_id.as_str()).collect();
        assert!(held.contains(&fact_a.as_str()) && held.contains(&fact_b.as_str()));
    }
}
