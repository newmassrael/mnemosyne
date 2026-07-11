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
//! **Guardrail B-2 (landed, Round 433):** conflict scoping is decided in
//! ONE place — [`join_world`] (`(frame, world-line)` since the branch axis
//! landed; it superseded the pre-fork `same_scope` predicate in Round 438). Same-frame facts on different world-lines never
//! conflict (cross-branch pairs are data, exactly like cross-frame pairs),
//! and the canon order is branch-relative: the declaration may carry
//! per-branch edge sets (`branches`), each composed with the shared `edges`
//! base — the same quest node can legitimately order differently on two
//! world-lines.
//!
//! **Shared history (Round 438):** a branch registered with `forks_from`
//! INHERITS its ancestor world's facts up to the fork point — visibility is
//! per query world ([`visibility`]): a fact on an ancestor branch is `In`
//! iff its `canon_from` is at or before the point where this lineage
//! departed that ancestor, `Unknown` when the declared order cannot decide
//! (B-1 honesty — surfaced, never gated). Conflicts evaluate in the JOIN
//! world (the deeper branch when one is the other's ancestor; siblings
//! share no world = data), succession may point along the lineage (a fork
//! superseding an inherited belief is in-world change), and a successor
//! ends a predecessor only in worlds where the successor itself is
//! visible — main never sees a fork's revisions. A branch's composed order
//! also inherits every ancestor's edge set. `forks_from = None` keeps the
//! standalone-world semantics exactly.
//!
//! **Typed-claim rule gate (Round 449, design sec 7.12):** a declared
//! `narrative-rules/v1` artifact (consumer vocabulary, never L0; R428
//! authority-input contract with an optional sha256 pin) adds two derived
//! violation classes over the typed subset — `exclusive` (one co-holding
//! value per subject / one holder per object, symmetric non-keyed-leg
//! consistency skip per R443) and `transition` (allowed state steps riding
//! the in-frame succession edge). The gate is the THIRD reader of
//! [`WorldCtx::holds_at`] — point-quantified holds-semantics verbatim, no
//! interval algebra of its own. Rule findings are derivations: re-evaluated
//! fresh each scan, never pinned. Authoring the file is the opt-in;
//! violations ride the existing continuity severity knob.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use mnemosyne_atomic::AtomicStore;
use mnemosyne_config::Severity;
use mnemosyne_core::NarrativeFact;
use serde::{Deserialize, Serialize};

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
// `Serialize` (Round 600) lets the `describe-schema` canon-order drift guard
// pin its prose to these field names; the loader only ever deserializes it.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
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
        Self::from_declaration(
            &CanonOrderFile {
                edges: edges.to_vec(),
                branches: BTreeMap::new(),
            },
            &BTreeMap::new(),
        )
    }

    /// Construct from a declaration + the per-world order composition
    /// ([`world_order_composition`], Rounds 438 + 533): `composition` maps a
    /// branch → the OTHER branches whose declared edge sets compose its order
    /// (its world-line membership in both directions — backward fork ancestors
    /// and forward confluence-suffixes, unified, since the order algebra is
    /// direction-agnostic). A branch's order = closure of (base ∪ every
    /// contributor's declared edges ∪ its own), cycle-checked per composition;
    /// `closure_of` topo-closes the resulting DAG unchanged (a confluence makes
    /// a lineage a DAG, not a chain — the order algebra already handles it).
    /// The order algebra never sees the narrative backward/forward distinction;
    /// that lives in [`Lineage`] (visibility), where it is load-bearing.
    pub fn from_declaration(
        decl: &CanonOrderFile,
        composition: &BTreeMap<String, Vec<String>>,
    ) -> Result<Self, String> {
        let base = closure_of(&decl.edges, "base")?;
        for branch in decl.branches.keys() {
            let branch = branch.trim();
            if branch.is_empty() {
                return Err("canon-order: blank branch id in `branches`".to_string());
            }
            if branch == mnemosyne_core::MAIN_BRANCH {
                return Err(format!(
                    "canon-order: `branches` declares `{branch}` — the base `edges` ARE the \
                     default world-line's order (one way to say it)"
                ));
            }
        }
        let mut branch_reach = BTreeMap::new();
        let all_branches: BTreeSet<&str> = decl
            .branches
            .keys()
            .map(String::as_str)
            .chain(composition.keys().map(String::as_str))
            .collect();
        for branch in all_branches {
            let mut combined = decl.edges.clone();
            for contributor in composition.get(branch).into_iter().flatten() {
                if let Some(edges) = decl.branches.get(contributor) {
                    combined.extend(edges.iter().cloned());
                }
            }
            if let Some(edges) = decl.branches.get(branch) {
                combined.extend(edges.iter().cloned());
            }
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

    /// `node` is named in `branch`'s composed order (Round 488). Catches a fact
    /// whose canon coordinate is positioned in the store's canon but not in its
    /// own branch's world-line — the wrong-branch authoring footgun.
    pub fn names(&self, branch: &str, node: &str) -> bool {
        self.reach_for(branch).contains_key(node)
    }

    /// Branch ids carrying a declared edge set.
    pub fn declared_branches(&self) -> impl Iterator<Item = &str> {
        self.branch_reach.keys().map(String::as_str)
    }

    /// `node` is a maximal node of `branch`'s declared composed order — a
    /// world-line end: named by the governing declaration, no strict
    /// descendant. Sections outside the declaration are isolated
    /// coordinates, never maximal (Round 456 — order semantics live on
    /// the order type, not in its readers).
    pub fn is_maximal(&self, branch: &str, node: &str) -> bool {
        self.reach_for(branch)
            .get(node)
            .is_some_and(BTreeSet::is_empty)
    }

    /// Deterministic topological linearization of `branch`'s composed
    /// order (Round 466, design sec 7.17): every node the governing
    /// declaration names, lexicographically smallest first among nodes
    /// whose declared strict predecessors are all emitted (Kahn over the
    /// closure — the closure of a DAG topo-sorts identically to it).
    /// ONE valid reading of a partial order, never the only one; the
    /// manuscript surfaces the undeclared adjacencies beside it.
    pub fn linearize(&self, branch: &str) -> Vec<String> {
        let reach = self.reach_for(branch);
        let mut pred_count: BTreeMap<&str, usize> = reach.keys().map(|k| (k.as_str(), 0)).collect();
        for descendants in reach.values() {
            for d in descendants {
                *pred_count
                    .get_mut(d.as_str())
                    .expect("closure names every node") += 1;
            }
        }
        let mut ready: BTreeSet<&str> = pred_count
            .iter()
            .filter(|(_, c)| **c == 0)
            .map(|(n, _)| *n)
            .collect();
        let mut out = Vec::with_capacity(pred_count.len());
        while let Some(&n) = ready.iter().next() {
            ready.remove(n);
            out.push(n.to_string());
            for d in &reach[n] {
                let c = pred_count
                    .get_mut(d.as_str())
                    .expect("closure names every node");
                *c -= 1;
                if *c == 0 {
                    ready.insert(d.as_str());
                }
            }
        }
        out
    }
}

/// Authority-input pin check (R428 pattern), shared by the canon-order and
/// narrative-rules loaders (Round 449 — the second declared gate-authority
/// artifact triggered the dedup): a configured pin re-hashes every load and
/// fails LOUDLY on mismatch. `what` names the artifact, `pin_key` the
/// config key to update after a reviewed change.
fn verify_authority_pin(
    bytes: &[u8],
    expected: &str,
    what: &str,
    pin_key: &str,
    path: &Path,
) -> Result<(), String> {
    let actual = mnemosyne_core::sha256_hex(bytes);
    if actual != expected {
        return Err(format!(
            "{what} sha256 mismatch at {}: pinned {expected} but file hashes {actual} — the \
             declaration changed without a re-pin (or was tampered); re-generate, review, \
             and update {pin_key}",
            path.display(),
        ));
    }
    Ok(())
}

/// Load a declared canon order FILE, with the optional sha256 pin (R428
/// pattern: the order is a gate-authority input; a configured pin re-hashes
/// every load and fails LOUDLY on mismatch). Construction into a
/// [`CanonOrder`] happens after the store loads — the per-branch
/// composition needs the fork ancestry (Round 438).
pub fn load_canon_order(
    path: &Path,
    expected_sha256: Option<&str>,
) -> Result<CanonOrderFile, String> {
    let bytes =
        std::fs::read(path).map_err(|e| format!("canon-order read {}: {}", path.display(), e))?;
    if let Some(expected) = expected_sha256 {
        verify_authority_pin(
            &bytes,
            expected,
            "canon-order",
            "[continuity].canon_order_sha256",
            path,
        )?;
    }
    serde_json::from_slice(&bytes)
        .map_err(|e| format!("canon-order parse {}: {}", path.display(), e))
}

/// The `narrative-rules/v1` contract — consumer/medium-adapter declared
/// (Round 449, design sec 7.12). Rule semantics are game/world vocabulary
/// and never enter L0 (ARCHITECTURE sec 6 invariant 4); like canon order,
/// the artifact arrives declared (guardrail B-1) with an optional sha256
/// pin. Authoring the file IS the opt-in — there is no separate severity
/// knob; rule violations ride the existing continuity severity (the R431
/// rationale: a same-frame rule violation is wrong data, never a
/// legitimate intermediate state).
///
/// Deserialization goes through [`NarrativeRulesWire`] with
/// `deny_unknown_fields` (Round 472): the prior `flatten`-based parse was
/// lenient and SILENTLY dropped unknown keys — a transition rule carrying
/// `per` (the S7 authoring miss in the A/B run), an `allowed` leg on an
/// exclusive rule, or a typo'd schema tag all passed unremarked. Those now
/// reject loudly, the same silent-no-op class already closed for the padded
/// predicate (R450) and the unknown `--field` (R468).
#[derive(Debug, Clone, Default)]
pub struct NarrativeRulesFile {
    pub rules: Vec<NarrativeRule>,
}

/// One declared rule. `id` names the rule in findings; `predicate` must be
/// a registered predicate id (predicates are LOAD-BEARING refs — a typo'd
/// predicate would silently escape its rule, the R436 write-side-typo
/// lesson — so the scan boundary fail-louds on an unknown one).
#[derive(Debug, Clone)]
pub struct NarrativeRule {
    pub id: String,
    pub predicate: String,
    pub spec: NarrativeRuleSpec,
}

impl NarrativeRule {
    /// Every predicate id this rule references: the primary `predicate` (the
    /// left operand for an interval rule) plus an interval rule's `right`
    /// operand and predicate-bound (Round 489). The existence check (one site)
    /// fail-louds on any that is not registered, so no ref escapes the typo
    /// guard.
    fn referenced_predicates(&self) -> Vec<&str> {
        let mut refs = vec![self.predicate.as_str()];
        if let NarrativeRuleSpec::Interval { right, bound, .. } = &self.spec {
            refs.push(right.as_str());
            if let IntervalBound::Predicate(p) = bound {
                refs.push(p.as_str());
            }
        }
        refs
    }
}

/// The TWO rule classes (design sec 7.12 — probe-verified sufficient for
/// the named trio: location exclusivity, conservation/custody, state
/// machines).
#[derive(Debug, Clone)]
pub enum NarrativeRuleSpec {
    /// At most one co-holding value per subject (`per: subject` — location
    /// exclusivity) or one holder per object (`per: object` —
    /// conservation/custody) within one (frame × world). The consistency
    /// skip is on the NON-KEYED leg, symmetric (R443 session-review fix):
    /// `per: subject` skips pairs with equal objects (one value restated ≠
    /// two values), `per: object` skips pairs with equal subjects (one
    /// holder restated ≠ two holders).
    Exclusive { per: ExclusiveKey },
    /// Rides the in-frame SUCCESSION edge: successor and predecessor both
    /// typed with the same subject + predicate → `(from, to)` must be a
    /// declared transition. Succession IS the declared adjacency —
    /// "adjacent" over a partial canon order is ill-defined, so the rule
    /// deliberately sees ONLY chained pairs; unchained same-subject pairs
    /// surface as `unchained_state_pairs`, never gated.
    Transition { allowed: Vec<[String; 2]> },
    /// Scalar/arithmetic relation over numeric typed legs (Round 489, design
    /// sec 7.20 — depth-ladder rung 1). The rule's `predicate` is the LEFT
    /// operand; `right` is the second operand; both are scalar predicates
    /// resolved per (frame × world × subject) — so the relation is SAME-SUBJECT
    /// (the measured pull: `codicil ratified-on-day − codicil signed-on-day ≥
    /// codicil min-ratify-gap-days`). The constraint is
    /// `value(left) − value(right)  op  bound`, a pure numeric comparison the
    /// equality/exclusivity gates structurally cannot express. A non-numeric or
    /// ambiguous operand is SURFACED (`interval_unverifiable`), never silently
    /// passed (the R450/R468/R485 no-silent-skip). Cross-subject relations are
    /// a wider shape, deferred (sec 7.20 honest boundary).
    Interval {
        right: String,
        op: IntervalOp,
        bound: IntervalBound,
    },
}

/// The comparison operator of an [`NarrativeRuleSpec::Interval`] rule
/// (Round 489): the closed set of scalar comparisons. `value(left) −
/// value(right)  ⋈op⋈  bound`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntervalOp {
    Ge,
    Le,
    Eq,
    Gt,
    Lt,
}

impl IntervalOp {
    /// Apply the operator to a computed `diff = value(left) − value(right)`
    /// and a `bound`. `true` = the constraint HOLDS (no violation).
    fn holds(self, diff: f64, bound: f64) -> bool {
        match self {
            IntervalOp::Ge => diff >= bound,
            IntervalOp::Le => diff <= bound,
            IntervalOp::Eq => diff == bound,
            IntervalOp::Gt => diff > bound,
            IntervalOp::Lt => diff < bound,
        }
    }

    /// The reporting symbol (findings carry it so a reader sees the relation).
    fn symbol(self) -> &'static str {
        match self {
            IntervalOp::Ge => ">=",
            IntervalOp::Le => "<=",
            IntervalOp::Eq => "==",
            IntervalOp::Gt => ">",
            IntervalOp::Lt => "<",
        }
    }
}

/// The right-hand bound of an [`NarrativeRuleSpec::Interval`] rule (Round
/// 489): a literal constant, or a third scalar predicate resolved on the
/// SAME subject as the operands (the inherited rule fact, e.g.
/// `min-ratify-gap-days`).
#[derive(Debug, Clone, PartialEq)]
pub enum IntervalBound {
    Const(f64),
    Predicate(String),
}

/// Which typed leg an exclusive rule keys on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExclusiveKey {
    Subject,
    Object,
}

impl ExclusiveKey {
    /// The non-keyed leg — where the symmetric consistency skip applies.
    fn other(self) -> Self {
        match self {
            ExclusiveKey::Subject => ExclusiveKey::Object,
            ExclusiveKey::Object => ExclusiveKey::Subject,
        }
    }
}

/// The comparison key of a typed leg under an [`ExclusiveKey`]: the subject
/// entity id, or the object's id/value string. Entity ids and scalar values
/// never collide in practice because a predicate's object shape is fixed by
/// its registered `object_kind` — every object of one predicate has one
/// shape.
fn claim_leg(t: &mnemosyne_core::TypedClaim, leg: ExclusiveKey) -> &str {
    match leg {
        ExclusiveKey::Subject => &t.subject,
        ExclusiveKey::Object => typed_object_key(&t.object),
    }
}

/// The object leg's comparison/report string: the entity id or the scalar
/// value. The rule gate is the only reader today; promote to
/// `mnemosyne-core` if a second one appears.
fn typed_object_key(o: &mnemosyne_core::TypedObject) -> &str {
    match o {
        mnemosyne_core::TypedObject::Entity { id } => id,
        mnemosyne_core::TypedObject::Value { value } => value,
    }
}

/// The schema tag every `narrative-rules` file carries; a present-but-wrong
/// value fails loud (the wrong-version silent-no-op, the same class as an
/// unknown field).
const NARRATIVE_RULES_SCHEMA: &str = "narrative-rules/v1";

/// Wire form of the rules file — flat (no `flatten`) so `deny_unknown_fields`
/// applies; serde forbids that attribute under `flatten`, which is exactly
/// how the lenient parse swallowed unknown keys. `schema` is the version tag
/// and `comment` a free-text annotation slot the dogfood files carry; both
/// are modeled so a THIRD unknown file-level key fails loud.
// `Serialize` (Round 605, review F2): the rules-file wire is a serialization
// contract documented by `schema::describe_schema().narrative_rules_wire`, so —
// like `FactsManifest` / `CanonOrderFile` — it is reflection-pinned: a test
// serializes a fully-populated sample and asserts the describe-schema prose
// names every emitted key, so a serde rename here fails the build until the prose
// is updated (the TEST-guarded tier, not hand-authored tier-3).
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct NarrativeRulesWire {
    #[serde(default)]
    schema: Option<String>,
    #[serde(default)]
    #[allow(dead_code)] // annotation slot: parsed so it is allowed, never read
    comment: Option<String>,
    #[serde(default)]
    rules: Vec<NarrativeRuleWire>,
}

/// Wire form of one rule — flat, `deny_unknown_fields`. `per` and `allowed`
/// are optional here and checked against `class` in
/// [`narrative_rule_from_wire`], so a transition carrying `per` (the S7
/// miss) or an exclusive carrying `allowed` rejects rather than silently
/// dropping the stray leg, and a missing leg is named.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct NarrativeRuleWire {
    id: String,
    predicate: String,
    class: RuleClass,
    #[serde(default)]
    per: Option<ExclusiveKey>,
    #[serde(default)]
    allowed: Option<Vec<[String; 2]>>,
    /// Interval legs (Round 489) — present only for `class: interval`; a
    /// stray one on exclusive/transition rejects (the leg/class coherence
    /// matrix, the R443 lesson).
    #[serde(default)]
    right: Option<String>,
    #[serde(default)]
    op: Option<IntervalOp>,
    #[serde(default)]
    bound: Option<IntervalBoundWire>,
}

/// Wire form of an interval bound — flat, `deny_unknown_fields`, exactly one
/// of `predicate` / `const` set (checked in [`narrative_rule_from_wire`], the
/// explicit-coherence idiom over serde `untagged`).
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct IntervalBoundWire {
    #[serde(default)]
    predicate: Option<String>,
    #[serde(default, rename = "const")]
    constant: Option<f64>,
}

/// The class tag, split from its leg so leg/class coherence is checked
/// explicitly instead of by the lenient `flatten`. `pub(crate)` so the
/// authoring-contract description (`schema::describe_schema`, R587) enumerates
/// the rule classes from THIS enum — an added class breaks its exhaustive match
/// (the single-source drift guard) instead of silently going undescribed.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RuleClass {
    Exclusive,
    Transition,
    Interval,
}

/// Round 605 (review F2): a fully-populated rules-file wire sample serialized to
/// JSON, for the describe-schema reflection guard (`schema.rs`
/// `narrative_rules_wire_prose_names_every_serde_key`). Exercises every serde key
/// — the file-level `schema`/`comment`/`rules`, every rule leg, and the `const`
/// bound rename — so a rename here fails that guard. Ids need not be valid (this
/// is serialization to enumerate keys, not validation).
#[cfg(test)]
pub(crate) fn narrative_rules_wire_sample_json() -> serde_json::Value {
    let sample = NarrativeRulesWire {
        schema: Some("narrative-rules/v1".into()),
        comment: Some("annotation".into()),
        rules: vec![NarrativeRuleWire {
            id: "r".into(),
            predicate: "p".into(),
            class: RuleClass::Interval,
            per: Some(ExclusiveKey::Object),
            allowed: Some(vec![["a".into(), "b".into()]]),
            right: Some("q".into()),
            op: Some(IntervalOp::Ge),
            bound: Some(IntervalBoundWire {
                predicate: Some("z".into()),
                constant: Some(30.0),
            }),
        }],
    };
    serde_json::to_value(sample).expect("serialize rules wire sample")
}

/// Load a declared narrative-rules FILE, with the optional sha256 pin
/// (Round 449; same R428 authority-input contract as the canon order).
/// File-shape validation is here (unknown keys, schema tag, blank/duplicate
/// ids, blank legs, leg/class coherence); registry checks (the predicate
/// must exist) happen at the scan boundary, where the store is in hand.
pub fn load_narrative_rules(
    path: &Path,
    expected_sha256: Option<&str>,
) -> Result<NarrativeRulesFile, String> {
    let bytes = std::fs::read(path)
        .map_err(|e| format!("narrative-rules read {}: {}", path.display(), e))?;
    if let Some(expected) = expected_sha256 {
        verify_authority_pin(
            &bytes,
            expected,
            "narrative-rules",
            "[continuity].rules_sha256",
            path,
        )?;
    }
    let wire: NarrativeRulesWire = serde_json::from_slice(&bytes)
        .map_err(|e| format!("narrative-rules parse {}: {}", path.display(), e))?;
    if let Some(schema) = wire.schema.as_deref() {
        if schema.trim() != NARRATIVE_RULES_SCHEMA {
            return Err(format!(
                "narrative-rules: schema `{}` is not `{NARRATIVE_RULES_SCHEMA}` — the engine \
                 knows only that contract",
                schema.trim()
            ));
        }
    }
    let mut rules: Vec<NarrativeRule> = Vec::with_capacity(wire.rules.len());
    let mut seen_ids: BTreeSet<String> = BTreeSet::new();
    for w in wire.rules {
        let rule = narrative_rule_from_wire(w)?;
        if !seen_ids.insert(rule.id.clone()) {
            return Err(format!(
                "narrative-rules: duplicate rule id `{}` — ids name findings, so they \
                 must be unique",
                rule.id
            ));
        }
        rules.push(rule);
    }
    Ok(NarrativeRulesFile { rules })
}

/// Convert one wire rule to the internal [`NarrativeRule`], checking
/// leg/class coherence and trimming whitespace INTO the stored values
/// (R450: the boundary check and the evaluation both compare exact, so a
/// padded `" alive"` that only a trimmed registry check accepted would match
/// no typed fact and silently disarm its rule).
fn narrative_rule_from_wire(w: NarrativeRuleWire) -> Result<NarrativeRule, String> {
    let id = w.id.trim().to_string();
    if id.is_empty() {
        return Err("narrative-rules: blank rule id".to_string());
    }
    let predicate = w.predicate.trim().to_string();
    if predicate.is_empty() {
        return Err(format!(
            "narrative-rules: rule `{id}` has a blank predicate"
        ));
    }
    let spec = match w.class {
        RuleClass::Exclusive => {
            if w.allowed.is_some() {
                return Err(format!(
                    "narrative-rules: exclusive rule `{id}` carries an `allowed` leg \
                     (that field belongs to a transition rule)"
                ));
            }
            forbid_interval_legs(&id, "exclusive", &w)?;
            let per = w.per.ok_or_else(|| {
                format!("narrative-rules: exclusive rule `{id}` is missing its `per` leg")
            })?;
            NarrativeRuleSpec::Exclusive { per }
        }
        RuleClass::Transition => {
            if w.per.is_some() {
                return Err(format!(
                    "narrative-rules: transition rule `{id}` carries a `per` field \
                     (that leg belongs to an exclusive rule)"
                ));
            }
            forbid_interval_legs(&id, "transition", &w)?;
            let raw = w.allowed.ok_or_else(|| {
                format!("narrative-rules: transition rule `{id}` is missing its `allowed` legs")
            })?;
            let mut allowed: Vec<[String; 2]> = Vec::with_capacity(raw.len());
            for pair in raw {
                let from = pair[0].trim().to_string();
                let to = pair[1].trim().to_string();
                if from.is_empty() || to.is_empty() {
                    return Err(format!(
                        "narrative-rules: rule `{id}` has a blank leg in an allowed \
                         transition pair"
                    ));
                }
                allowed.push([from, to]);
            }
            NarrativeRuleSpec::Transition { allowed }
        }
        RuleClass::Interval => {
            if w.per.is_some() {
                return Err(format!(
                    "narrative-rules: interval rule `{id}` carries a `per` field \
                     (that leg belongs to an exclusive rule)"
                ));
            }
            if w.allowed.is_some() {
                return Err(format!(
                    "narrative-rules: interval rule `{id}` carries an `allowed` leg \
                     (that field belongs to a transition rule)"
                ));
            }
            let right = w
                .right
                .ok_or_else(|| {
                    format!("narrative-rules: interval rule `{id}` is missing its `right` operand")
                })?
                .trim()
                .to_string();
            if right.is_empty() {
                return Err(format!(
                    "narrative-rules: interval rule `{id}` has a blank `right` operand"
                ));
            }
            let op = w.op.ok_or_else(|| {
                format!("narrative-rules: interval rule `{id}` is missing its `op`")
            })?;
            let bound_wire = w.bound.ok_or_else(|| {
                format!("narrative-rules: interval rule `{id}` is missing its `bound`")
            })?;
            let bound = interval_bound_from_wire(&id, bound_wire)?;
            NarrativeRuleSpec::Interval { right, op, bound }
        }
    };
    Ok(NarrativeRule {
        id,
        predicate,
        spec,
    })
}

/// Reject a stray interval leg on a non-interval rule (Round 489) — the
/// leg/class coherence matrix extended to `right` / `op` / `bound`, symmetric
/// to how `per` and `allowed` already reject on the wrong class (R443 lesson).
fn forbid_interval_legs(id: &str, class: &str, w: &NarrativeRuleWire) -> Result<(), String> {
    if w.right.is_some() || w.op.is_some() || w.bound.is_some() {
        return Err(format!(
            "narrative-rules: {class} rule `{id}` carries an interval leg \
             (`right` / `op` / `bound` belong to an interval rule)"
        ));
    }
    Ok(())
}

/// Resolve an interval bound wire to the internal [`IntervalBound`] (Round
/// 489): exactly one of `predicate` / `const`, checked explicitly (the
/// explicit-coherence idiom, not serde `untagged`).
fn interval_bound_from_wire(id: &str, w: IntervalBoundWire) -> Result<IntervalBound, String> {
    match (w.predicate, w.constant) {
        (Some(_), Some(_)) => Err(format!(
            "narrative-rules: interval rule `{id}` bound sets both `predicate` and \
             `const` — exactly one"
        )),
        (None, None) => Err(format!(
            "narrative-rules: interval rule `{id}` bound sets neither `predicate` nor \
             `const` — exactly one"
        )),
        (Some(p), None) => {
            let p = p.trim().to_string();
            if p.is_empty() {
                return Err(format!(
                    "narrative-rules: interval rule `{id}` bound has a blank `predicate`"
                ));
            }
            Ok(IntervalBound::Predicate(p))
        }
        (None, Some(c)) => Ok(IntervalBound::Const(c)),
    }
}

/// One continuity violation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
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
    /// A fact's canon coordinate is named in the store's canon (some branch's
    /// order positions it) but NOT in this fact's OWN branch order — the fact
    /// is on the wrong world-line (Round 488). The silent wrong-branch
    /// authoring error made loud: e.g. a fact defaulting to `main` when the
    /// trunk is a named branch the forks inherit, so its canon node lives
    /// elsewhere and the conflict gate never compares it where it should. A
    /// coordinate named by no branch's order is the orderless/forward-declared
    /// mode and is tolerated (not flagged).
    FactCanonOffBranch {
        fact: String,
        branch: String,
        coord: String,
    },
    /// A fact cites EVIDENCE not reachable at-or-before its own canon
    /// coordinate in its world-line (Round 522, design sec 7.27 Piece B). The
    /// R488 off-branch reachability principle — applied to `canon_from` by
    /// `FactCanonOffBranch` — extended to `evidence` via the SAME
    /// `le(branch, a, b)`. A structural backreference (`evidence`) is an
    /// allusion to an establishing scene; it must be reachable AND prior in
    /// the fact's own branch. Sibling-world-line evidence (no path in this
    /// branch) and a forward reference (in-branch but after the fact) both
    /// fail; spine/prior evidence (reachable in every descendant) passes. As
    /// with `FactCanonOffBranch`, only positioned coordinates are checked —
    /// the orderless/forward-declared mode is tolerated (a fact whose
    /// `canon_from` is unpositioned, or an `evidence` coordinate named by no
    /// branch's order, is not flagged).
    EvidenceUnreachable {
        fact: String,
        branch: String,
        evidence: String,
        canon_from: String,
    },
    /// A fact authored on a CONFLUENCE cites evidence not reachable from one of
    /// the merge's incoming parents (Round 535, the R528-Q3 reconciliation). A
    /// confluence's shared suffix holds in EVERY converging parent world-line
    /// (forward visibility), so its structural dependencies (`evidence`) must be
    /// satisfiable from EACH parent's side of the join — the R488/R522 `le`
    /// reachability checked PER incoming parent (against the fact's own
    /// `canon_from`, which routes through that parent's merge coordinate),
    /// because the confluence's OWN order is prefix-less and cannot connect a
    /// parent's prefix to the suffix. Evidence reachable from only one parent (a
    /// parent-exclusive scene) is an unreconciled cross-merge dependency: the
    /// fact belongs on that parent (a path-dependent continuation) or must cite a
    /// shared establishing scene. Trunk/shared evidence reachable from every
    /// parent passes.
    ConfluenceEvidenceUnreconciled {
        fact: String,
        confluence: String,
        parent: String,
        evidence: String,
        canon_from: String,
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
    /// `supersedes_in_frame` crosses world-lines into a branch that does NOT
    /// inherit the predecessor's belief (Round 433 + 535). A cross-branch
    /// succession is legitimate in exactly two inheritance directions
    /// ([`succession_branch_inherits`]): the predecessor is a BACKWARD fork
    /// ancestor of the successor (a fork revises an inherited belief), or the
    /// successor is a FORWARD confluence-suffix of the predecessor (a merge's
    /// shared continuation reconciles a parent belief at the join, R535 —
    /// bounded, no auto-merge engine). Any OTHER cross-branch edge is a
    /// sibling-world edit (out-of-band; the write path rejects it, the scan
    /// re-checks, fail-loud).
    SuccessionCrossBranch {
        successor: String,
        predecessor: String,
        successor_branch: String,
        predecessor_branch: String,
    },
    /// A recorded edge names a fact that no longer exists (out-of-band
    /// edit; fail-loud).
    ConflictTargetMissing { fact_id: String, target: String },
    /// The target's claim changed since this judgment was recorded (Round
    /// 439): the assertion pinned a different text — re-affirm it (amend
    /// the edge-owning fact restamps its outbound judgments) or retract it.
    /// The pair is still evaluated; the staleness itself is surfaced.
    ConflictEdgeStale {
        fact_id: String,
        target: String,
        stamped_sha256: String,
        current_sha256: String,
    },
    /// `supersedes_in_frame` names a fact that no longer exists.
    SuccessionTargetMissing { fact_id: String, target: String },
    /// Succession edges close a loop (Round 463; out-of-band edit — every
    /// write path rejects this via the shared edge check, the scan
    /// re-checks). A cycle's facts silently never hold anywhere, so this
    /// is a violation, not a count. Reported once per cycle, members in
    /// walk order from the minimum id.
    SuccessionCycle { cycle: Vec<String> },
    /// `pays_off` names a fact that no longer exists (Round 442; out-of-band
    /// edit — the write path rejects this, the scan re-checks, fail-loud).
    /// An evaluable data finding like the conflict/succession variants, not
    /// a store-corruption `Err` (the Round 440 boundary doctrine).
    PayoffTargetMissing { fact_id: String, target: String },
    /// An exclusive rule violated (Round 449, design sec 7.12): two
    /// same-frame typed facts with the rule's predicate agree on the keyed
    /// leg but differ on the non-keyed one, and co-hold at `at` in query
    /// world `branch`. Rule findings are DERIVATIONS — re-evaluated fresh
    /// each scan, never pinned (judgments pin, derivations re-evaluate).
    RuleExclusiveOverlap {
        rule: String,
        predicate: String,
        frame: String,
        branch: String,
        fact_a: String,
        fact_b: String,
        at: String,
    },
    /// A transition rule violated (Round 449): an in-frame succession edge
    /// whose two legs are typed with the same subject + predicate steps
    /// `(from → to)` outside the rule's allowed set.
    RuleTransitionInvalid {
        rule: String,
        predicate: String,
        frame: String,
        subject: String,
        predecessor: String,
        successor: String,
        from: String,
        to: String,
    },
    /// An interval rule violated (Round 489, design sec 7.20): for one subject
    /// in one (frame × world), the numeric relation
    /// `value(left) − value(right)  op  bound` is FALSE. `left` is the rule's
    /// `predicate`; `at` is the left operand's canon coordinate (the point the
    /// relation is evaluated). A pure arithmetic derivation — re-evaluated
    /// fresh each scan, never pinned.
    RuleIntervalViolation {
        rule: String,
        predicate: String,
        right: String,
        op: String,
        frame: String,
        branch: String,
        subject: String,
        left_fact: String,
        right_fact: String,
        /// Authored operand values (the scalar strings), kept faithful; the
        /// numeric comparison happened in the evaluator.
        left_value: String,
        right_value: String,
        bound: String,
        at: String,
    },
}

impl ContinuityViolation {
    /// Whether this is an INTERVAL (timeline) violation, which rides the
    /// separate `interval_severity` class (Round 491) rather than `severity`.
    pub fn is_interval(&self) -> bool {
        matches!(self, ContinuityViolation::RuleIntervalViolation { .. })
    }
}

/// The per-class continuity gate decision (Round 592) — THE single source of the
/// reject policy, shared by `validate-continuity` and `propose-verdict` so a dry
/// run mirrors the real gate EXACTLY (before R592 they diverged: propose-verdict
/// rolled back on store-valid content — a `warn`/`info` store, an interval
/// time-bend with `interval_severity` OFF, or a `[continuity]`-disabled
/// workspace). Structural violations (conflict / off-branch / succession /
/// exclusive / transition) ride `severity`; interval (timeline) violations ride
/// `interval_severity` (OFF by default = surface-not-gate). A class gates only at
/// `reject`; `None` severity = that class is disabled (never gates).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContinuityGateOutcome {
    pub structural_count: usize,
    pub interval_count: usize,
    /// `true` iff a reject-severity class has ≥ 1 violation — the store's own
    /// policy would reject.
    pub gates: bool,
}

/// Evaluate the per-class continuity gate (Round 592). See
/// [`ContinuityGateOutcome`]. `severity` / `interval_severity` are the resolved
/// policy (`None` = that class disabled).
pub fn evaluate_continuity_gate(
    severity: Option<Severity>,
    interval_severity: Option<Severity>,
    violations: &[ContinuityViolation],
) -> ContinuityGateOutcome {
    let interval_count = violations.iter().filter(|v| v.is_interval()).count();
    let structural_count = violations.len() - interval_count;
    let structural_gates = matches!(severity, Some(s) if s.is_reject()) && structural_count > 0;
    let interval_gates =
        matches!(interval_severity, Some(s) if s.is_reject()) && interval_count > 0;
    ContinuityGateOutcome {
        structural_count,
        interval_count,
        gates: structural_gates || interval_gates,
    }
}

/// Scan result — pure data; severity/gating policy belongs to the caller.
#[derive(Debug, Clone, Default, Serialize)]
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
    /// Declared narrative rules evaluated (Round 449; 0 = no rules file =
    /// the gate's pre-Round-449 behavior exactly).
    pub rules: usize,
    /// Distinct exclusive-rule candidate pairs whose canon coordinates the
    /// declared order cannot compare in some world (B-1: surfaced, never
    /// gated — the rule cannot decide them).
    pub rule_unordered_pairs: usize,
    /// Distinct same-frame same-subject typed pairs (per transition rule)
    /// visible together in some query world with NO succession PATH
    /// between them — states the chain never connects, which the
    /// transition rule therefore cannot see. Surfaced as a count, never
    /// gated. Path, not direct edge (Round 452): a correctly chained
    /// A→B→C arc transitively connects (A, C) and must not count.
    /// WORLD-scoped via the shared visibility (the R441 probe finding:
    /// raw branch equality would silently miss fork-inherited pairs),
    /// deduplicated across worlds.
    pub unchained_state_pairs: usize,
    /// Interval-rule operand resolutions (Round 489) that could not be
    /// evaluated: a rule applies to a subject (it has both operands) but an
    /// operand value is non-numeric, or an operand / a predicate-bound
    /// resolves to MORE than one distinct holding value at the evaluation
    /// point (ambiguous). Surfaced as a count, NEVER gated — the data is
    /// absent/unparseable, not contradictory (the R485 `unverifiable` class:
    /// the author types it, then the gate decides). Deduplicated per
    /// (rule, frame, world, subject).
    pub interval_unverifiable: usize,
}

/// Fork lineage of one query world (Rounds 438 + 533): the world's full
/// world-line membership — its BACKWARD fork ancestry (`cut`) and its FORWARD
/// confluence-suffixes (`forward`). Empty for `MAIN_BRANCH`, standalone
/// branches, and pre-fork stores.
#[derive(Debug, Clone, Default)]
pub struct Lineage {
    /// Backward (Round 438): ancestor branch id -> departure point (the
    /// child's `forks_from.at`). A fact on an ancestor is inherited only up to
    /// this point — the parent continues past the fork in its OWN world.
    cut: BTreeMap<String, String>,
    /// Forward (Round 533): confluence branches this world flows INTO past a
    /// merge (`mnemosyne_core::forward_confluences`). A fact authored once on
    /// such a confluence is part of this world-line — unconditionally (the
    /// confluence is the SHARED continuation downstream of every parent; WHEN
    /// it holds is the order's job, not visibility's). The dual asymmetry of
    /// `cut`: backward inheritance is bounded at the fork, forward inheritance
    /// is total past the merge.
    forward: BTreeSet<String>,
}

/// One world's lineage view over THE single world-line graph traversals
/// ([`mnemosyne_core::fork_chain`] backward + [`mnemosyne_core::forward_confluences`]
/// forward, Rounds 440 + 533 — write path, gate, and view share one walk per
/// direction).
pub fn lineage_of(
    branches: &BTreeMap<String, mnemosyne_core::Branch>,
    world: &str,
) -> Result<Lineage, String> {
    Ok(Lineage {
        cut: mnemosyne_core::fork_chain(branches, world)?
            .into_iter()
            .collect(),
        forward: mnemosyne_core::forward_confluences(branches, world)
            .into_iter()
            .collect(),
    })
}

/// Per potential query world, the OTHER branches whose declared edge sets
/// compose into its order (Rounds 438 + 533) — the single
/// [`CanonOrder::from_declaration`] composition input. A world's order is the
/// closure of its base ∪ these contributors' edges ∪ its own; the contributors
/// are its world-line membership in BOTH directions: backward fork ancestors
/// (the inherited prefix, [`mnemosyne_core::fork_chain`]) AND forward
/// confluence-suffixes (the shared continuation, [`mnemosyne_core::forward_confluences`]).
/// The order algebra is direction-agnostic — it composes edge sets and
/// `closure_of` topo-closes the resulting DAG — so the two relations unify here
/// into one contributor list; the backward/forward DISTINCTION lives only in
/// [`Lineage`] (visibility), where it is load-bearing. Keyed for `MAIN_BRANCH`
/// and every registered branch (a confluence parent may be `main`); a world
/// with no contributors is omitted (its order is the base, reached via the
/// `reach_for` fallback — byte-stable for a pre-fork/pre-confluence store).
pub fn world_order_composition(
    branches: &BTreeMap<String, mnemosyne_core::Branch>,
) -> Result<BTreeMap<String, Vec<String>>, String> {
    let mut out = BTreeMap::new();
    for world in
        std::iter::once(mnemosyne_core::MAIN_BRANCH.to_string()).chain(branches.keys().cloned())
    {
        let mut contributors: Vec<String> = mnemosyne_core::fork_chain(branches, &world)?
            .into_iter()
            .map(|(ancestor, _)| ancestor)
            .collect();
        contributors.extend(mnemosyne_core::forward_confluences(branches, &world));
        if !contributors.is_empty() {
            out.insert(world, contributors);
        }
    }
    Ok(out)
}

/// Three-state world-visibility of a fact in query world `world` (Rounds
/// 438 + 533, B-1 honest): `In` = part of this world (its own branch, an
/// ancestor branch at-or-before the departure point, or a FORWARD
/// confluence-suffix this world flows into); `Out` = definitively another
/// world; `Unknown` = on an ancestor, but the declared order cannot compare
/// its start to the fork point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Vis {
    In,
    Out,
    Unknown,
}

fn visibility(world: &str, lineage: &Lineage, order: &CanonOrder, fact: &NarrativeFact) -> Vis {
    if fact.branch == world {
        return Vis::In;
    }
    // Forward (Round 533): a fact authored on a confluence this world flows
    // INTO is part of this world-line — unconditionally `In` (no fork-cut
    // bound: the confluence is the shared continuation downstream of every
    // parent). WHEN it holds is the order's job ([`WorldCtx::holds_at`]) — a
    // suffix fact's `canon_from` sits at/after the merge by construction, so
    // it cannot hold before the merge in this world's composed order. No
    // `Unknown` arm: membership is structural, never order-dependent.
    if lineage.forward.contains(&fact.branch) {
        return Vis::In;
    }
    match lineage.cut.get(&fact.branch) {
        None => Vis::Out,
        Some(at) => {
            if order.le(world, &fact.canon_from, at) {
                Vis::In
            } else if order.comparable(world, &fact.canon_from, at) {
                Vis::Out
            } else {
                Vis::Unknown
            }
        }
    }
}

/// B-2 scope resolution — the ONE place conflict scoping is decided:
/// `(frame, world-line)` with fork lineage (Rounds 433 + 438 + 535). Same
/// frame required; the pair's JOIN world (the playthrough where both facts
/// co-exist, so a conflict between them is real) is found in either world-line
/// direction:
/// - BACKWARD (fork): one branch is the other's fork ancestor → the deeper
///   (descendant) branch, which inherits the ancestor's prefix.
/// - FORWARD (confluence, R535): one fact is on a CONFLUENCE the other's branch
///   flows INTO → the PARENT, where the shared suffix is visible
///   ([`Lineage::forward`]) alongside the parent-middle fact. This scopes a
///   cross-merge conflict the backward `cut` cannot see.
///
/// Else there is no shared world — sibling/unrelated world-lines are data, like
/// cross-frame pairs. (The sibling-confluence-common-parent case — two distinct
/// confluences sharing a parent — would need an In-set intersection over the
/// query worlds; it belongs to the deferred series-parallel generalization, not
/// the R528 exclusive-OR diamond, so it stays unscoped here, surfaced as
/// `cross_scope_pairs`, never silent.)
fn join_world<'a>(
    a: &'a NarrativeFact,
    b: &'a NarrativeFact,
    lineages: &BTreeMap<String, Lineage>,
) -> Option<&'a str> {
    if a.frame != b.frame {
        return None;
    }
    if a.branch == b.branch {
        return Some(&a.branch);
    }
    if lineages[&a.branch].cut.contains_key(&b.branch) {
        return Some(&a.branch);
    }
    if lineages[&b.branch].cut.contains_key(&a.branch) {
        return Some(&b.branch);
    }
    // Forward (Round 535): `forward(X)` lists the confluences `X` flows into,
    // so `forward(b.branch).contains(a.branch)` means `a` is on a confluence
    // `b`'s world-line merges into — `a` (the suffix) is visible in `b.branch`
    // (the parent), where `b` lives natively; the join world is that parent.
    if lineages[&b.branch].forward.contains(&a.branch) {
        return Some(&b.branch);
    }
    if lineages[&a.branch].forward.contains(&b.branch) {
        return Some(&a.branch);
    }
    None
}

/// One query world's evaluation context (Round 440): the world id, its
/// fork lineage, the composed order, and the succession index — bundled so
/// the holds-semantics reads as a judgment about a world rather than a
/// seven-argument shuffle.
struct WorldCtx<'a> {
    world: &'a str,
    lineage: &'a Lineage,
    order: &'a CanonOrder,
    successors: &'a BTreeMap<&'a str, Vec<(&'a str, &'a NarrativeFact)>>,
}

impl WorldCtx<'_> {
    fn visibility(&self, fact: &NarrativeFact) -> Vis {
        visibility(self.world, self.lineage, self.order, fact)
    }

    /// Whether `fact` (id `fact_id`) holds at canon point `p` in this world
    /// (Round 438): visible here, started (`canon_from <= p`), not past a
    /// stored `canon_to`, and not yet replaced by an in-frame successor
    /// THAT IS ITSELF VISIBLE here — a fork's revision never ends the
    /// inherited belief in the ancestor's own world. All precedence is
    /// evaluated under this world's composed order.
    ///
    /// THE single holds-semantics — shared by the continuity gate and the
    /// frame-at-T projection ([`frame_view`]) so the two can never drift
    /// (the R390 single-predicate discipline).
    fn holds_at(&self, fact_id: &str, fact: &NarrativeFact, p: &str) -> bool {
        if self.visibility(fact) != Vis::In {
            return false;
        }
        if !self.order.le(self.world, &fact.canon_from, p) {
            return false;
        }
        if let Some(to) = &fact.canon_to {
            if !self.order.le(self.world, p, to) {
                return false;
            }
        }
        if let Some(succ) = self.successors.get(fact_id) {
            if succ.iter().any(|(_, s)| {
                self.visibility(s) == Vis::In && self.order.le(self.world, &s.canon_from, p)
            }) {
                return false;
            }
        }
        true
    }
}

/// Fail-loud store boundary shared by the gate and the view (Rounds 436 +
/// 440, single check — the two read paths cannot drift). Declaration side:
/// every order node must be a section (canon coordinates are structure
/// refs) and every declared per-branch edge set must name a REGISTERED
/// world-line. Fact side (Round 440 — the write path enforces all of this;
/// the scan RE-CHECKS it against out-of-band edits, closing the
/// half-enforced asymmetry where only conflict/succession targets were
/// re-checked): every fact's frame / branch / entity refs must be
/// registered, its canon coordinates and evidence must be sections, and
/// evidence must be non-empty. A store that fails this is corrupt — the
/// semantics below are not evaluable over it, so this is an `Err`, not a
/// violation. (It also guarantees every fact branch has a lineage entry,
/// which is what makes the downstream lineage lookups total.)
fn check_store_boundary(store: &AtomicStore, order: &CanonOrder) -> Result<(), String> {
    for n in order.nodes() {
        if !store.sections.contains_key(n) {
            return Err(format!(
                "canon-order names `{n}`, which is not a section in the store — \
                 canon coordinates are structure refs; fix the declaration"
            ));
        }
    }
    for b in order.declared_branches() {
        if !store.branches.contains_key(b) {
            return Err(format!(
                "canon-order declares an edge set for branch `{b}`, which is not in the \
                 branch registry — register it (add_branch) or fix the declaration"
            ));
        }
    }
    for (id, fact) in &store.narrative_facts {
        if !store.frames.contains_key(&fact.frame) {
            return Err(format!(
                "fact `{id}`: frame `{}` not in the frames registry (out-of-band edit; \
                 the write path enforces this)",
                fact.frame
            ));
        }
        if fact.branch != mnemosyne_core::MAIN_BRANCH && !store.branches.contains_key(&fact.branch)
        {
            return Err(format!(
                "fact `{id}`: branch `{}` not in the branch registry (out-of-band edit; \
                 the write path enforces this)",
                fact.branch
            ));
        }
        for e in &fact.entities {
            if !store.entities.contains_key(e) {
                return Err(format!(
                    "fact `{id}`: entity `{e}` not in the entity registry (out-of-band \
                     edit; the write path enforces this)"
                ));
            }
        }
        if !store.sections.contains_key(&fact.canon_from) {
            return Err(format!(
                "fact `{id}`: canon_from `{}` is not a section (out-of-band edit)",
                fact.canon_from
            ));
        }
        if let Some(to) = &fact.canon_to {
            if !store.sections.contains_key(to) {
                return Err(format!(
                    "fact `{id}`: canon_to `{to}` is not a section (out-of-band edit)"
                ));
            }
        }
        if fact.evidence.is_empty() {
            return Err(format!(
                "fact `{id}`: evidence emptied out-of-band (a claim without provenance \
                 is unauditable)"
            ));
        }
        for e in &fact.evidence {
            if !store.sections.contains_key(e) {
                return Err(format!(
                    "fact `{id}`: evidence `{e}` is not a section (out-of-band edit)"
                ));
            }
        }
    }
    Ok(())
}

/// Lineage per potential query world (main + every registered branch) —
/// THE single construction (Round 465; the scan and the edge-candidates
/// report carried the second copy, the two-copies rule).
fn query_world_lineages(store: &AtomicStore) -> Result<BTreeMap<String, Lineage>, String> {
    let mut lineages: BTreeMap<String, Lineage> = BTreeMap::new();
    lineages.insert(mnemosyne_core::MAIN_BRANCH.to_string(), Lineage::default());
    for branch in store.branches.keys() {
        lineages.insert(branch.clone(), lineage_of(&store.branches, branch)?);
    }
    Ok(lineages)
}

/// The query worlds the per-world surfaces SWEEP (Round 533): `MAIN_BRANCH`
/// plus every NON-confluence registered branch. A confluence is a structural
/// merge node, not a playable world-line — its shared-suffix facts are
/// evaluated WITHIN each parent world (forward visibility, [`Lineage::forward`]),
/// so sweeping the confluence as its OWN world would render a prefix-less
/// fragment and surface false per-world findings (a suffix setup whose payoff
/// lands in a parent middle would read as dangling in the merge's fragment
/// world). Deliberately DISTINCT from [`query_world_lineages`], which keys
/// EVERY branch — `join_world` indexes the lineage map by any fact's branch
/// (a same-branch suffix-suffix pair scopes to the confluence), so the lookup
/// set must be total even though the iteration set is not. Pre-confluence
/// stores: identical to the old `main + every branch` (no branch is a
/// confluence), so the sweep is byte-stable.
fn query_worlds(store: &AtomicStore) -> Vec<&str> {
    std::iter::once(mnemosyne_core::MAIN_BRANCH)
        .chain(
            store
                .branches
                .iter()
                .filter(|(_, b)| b.converges_from.is_empty())
                .map(|(id, _)| id.as_str()),
        )
        .collect()
}

/// The per-world pair space both rule surfaces sweep (Round 452 — the
/// second copy triggered the extraction): for every query world, every
/// same-frame pair of `typed` facts visible together there, visited with
/// that world's evaluation context. Pairs visit in id order (`typed` is
/// id-sorted); a pair visible in several worlds visits once per world —
/// the world is part of the finding. Cross-frame pairs never visit
/// (data, never gated — the North-Star sentence).
fn for_each_world_pair<'a>(
    worlds: &[&'a str],
    lineages: &'a BTreeMap<String, Lineage>,
    order: &'a CanonOrder,
    successors: &'a BTreeMap<&'a str, Vec<(&'a str, &'a NarrativeFact)>>,
    typed: &[(&'a String, &'a NarrativeFact)],
    mut visit: impl FnMut(&WorldCtx<'_>, &'a str, &'a NarrativeFact, &'a str, &'a NarrativeFact),
) {
    for world in worlds {
        let ctx = WorldCtx {
            world,
            lineage: &lineages[*world],
            order,
            successors,
        };
        let vis: Vec<&(&'a String, &'a NarrativeFact)> = typed
            .iter()
            .filter(|(_, f)| ctx.visibility(f) == Vis::In)
            .collect();
        for (i, (aid, a)) in vis.iter().enumerate() {
            for (bid, b) in vis.iter().skip(i + 1) {
                if a.frame != b.frame {
                    continue;
                }
                visit(&ctx, aid.as_str(), a, bid.as_str(), b);
            }
        }
    }
}

/// The R439 judgment-time content pin of a claim's text — delegates to
/// THE one hash encoding, `mnemosyne_core::sha256_hex` (Round 460
/// consolidation: this pin is stamped here and re-checked by the
/// proposals import in mnemosyne-atomic; two implementations of one
/// cross-crate invariant is the half-enforced-invariant class).
fn claim_sha256_hex(claim: &str) -> String {
    mnemosyne_core::sha256_hex(claim.as_bytes())
}

/// In-frame succession index (predecessor id → superseding facts, each
/// with its own id) — the [`WorldCtx::holds_at`] input every reader
/// needs, built one way (Round 456 session review: the third hand-rolled
/// copy triggered the extraction, per the R440/R452 two-copies rule; the
/// id rides along since Round 466 — the manuscript names the cutting
/// successor in its end events).
fn successors_index(
    facts: &BTreeMap<String, NarrativeFact>,
) -> BTreeMap<&str, Vec<(&str, &NarrativeFact)>> {
    let mut successors: BTreeMap<&str, Vec<(&str, &NarrativeFact)>> = BTreeMap::new();
    for (sid, fact) in facts {
        if let Some(t) = &fact.supersedes_in_frame {
            successors
                .entry(t.as_str())
                .or_default()
                .push((sid.as_str(), fact));
        }
    }
    successors
}

/// Every transitive predecessor of `id` along the `supersedes_in_frame`
/// chain (each fact carries at most one backward pointer, so this is a
/// single upward walk). Cycle-guarded: the write path rejects succession
/// cycles, but the scan re-reads out-of-band-edited stores (the Round 440
/// boundary doctrine), so the walk must terminate regardless.
fn succession_ancestors<'a>(
    facts: &'a BTreeMap<String, NarrativeFact>,
    id: &str,
) -> BTreeSet<&'a str> {
    let mut out = BTreeSet::new();
    let mut cur = facts.get(id).and_then(|f| f.supersedes_in_frame.as_deref());
    while let Some(p) = cur {
        if !out.insert(p) {
            break;
        }
        cur = facts.get(p).and_then(|f| f.supersedes_in_frame.as_deref());
    }
    out
}

/// Frame-scoped continuity scan over the narrative facts. Returns `Err` only
/// on a malformed input boundary (an order node that is not a section, a
/// declared branch that is not registered, or a rule naming an unregistered
/// predicate — likely a typo in a declaration; fail loud). All data findings
/// are violations/counts in the report.
///
/// `rules` is the declared `narrative-rules/v1` rule set (Round 449, design
/// sec 7.12) — empty = no rules authored = the recorded-edge gate alone.
/// The rule gate is the THIRD reader of [`WorldCtx::holds_at`] (after the
/// conflict gate and the frame-at-T view): it reuses the point-quantified
/// holds-semantics verbatim, never its own interval algebra (the R441 probe
/// falsified a paper interval model — the half-open successor cut is
/// load-bearing).
/// Parse a scalar typed-leg value as a number (Round 489). Trimmed so a
/// surrounding space is not mistaken for non-numeric; an unparseable value is
/// `None` and surfaces as `interval_unverifiable`, never silently skipped.
fn parse_scalar(value: &str) -> Option<f64> {
    value.trim().parse::<f64>().ok()
}

/// The resolution of an interval operand for one (frame × world × subject) at
/// the evaluation point (Round 489).
enum Operand<'a> {
    /// Exactly one distinct holding value (one or more facts agreeing on it):
    /// the parsed number, the authored value string, and the fact id.
    Value {
        num: f64,
        value: &'a str,
        fact: &'a str,
    },
    /// No holding fact for this (subject, predicate) here — the rule does not
    /// apply on this leg.
    Absent,
    /// A non-numeric value, or two or more DISTINCT holding values (ambiguous)
    /// — surfaced, never silently passed.
    Unverifiable,
}

/// Resolve `predicate` for `subject` in `frame`, among facts HOLDING at `at`
/// in this world (Round 489). The single point-quantified read is
/// [`WorldCtx::holds_at`] — the interval evaluator owns no time semantics of
/// its own (the R441 reader-reuse rule).
fn resolve_operand<'a>(
    facts: &'a BTreeMap<String, NarrativeFact>,
    ctx: &WorldCtx<'_>,
    frame: &str,
    subject: &str,
    predicate: &str,
    at: &str,
) -> Operand<'a> {
    let mut resolved: Option<(f64, &'a str, &'a str)> = None;
    for (gid, g) in facts {
        let Some(gt) = g.typed.as_ref() else { continue };
        if g.frame != frame || gt.subject != subject || gt.predicate != predicate {
            continue;
        }
        if !ctx.holds_at(gid, g, at) {
            continue;
        }
        let raw = typed_object_key(&gt.object);
        let Some(n) = parse_scalar(raw) else {
            return Operand::Unverifiable; // non-numeric operand
        };
        match resolved {
            None => resolved = Some((n, raw, gid.as_str())),
            Some((existing, _, _)) if existing == n => {} // same value restated
            Some(_) => return Operand::Unverifiable,      // distinct values: ambiguous
        }
    }
    match resolved {
        Some((num, value, fact)) => Operand::Value { num, value, fact },
        None => Operand::Absent,
    }
}

/// One interval-rule evaluation for a left-operand fact in a query world, at
/// that fact's canon coordinate (Round 489/490). The shared output of THE
/// single interval evaluator: the continuity gate maps `Violated` to a
/// `RuleIntervalViolation` and counts distinct `Unverifiable` subjects;
/// `report-timeline-gaps` (the read surface) presents all three. So the gate
/// and the report can never drift (R305/R390 single-reader discipline).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IntervalOutcome {
    pub rule: String,
    /// Left operand predicate (the rule's primary `predicate`).
    pub predicate: String,
    pub right: String,
    pub op: String,
    pub frame: String,
    pub world: String,
    pub subject: String,
    pub left_fact: String,
    pub left_value: String,
    /// The left operand's canon coordinate — the evaluation point.
    pub at: String,
    pub verdict: IntervalVerdict,
}

/// The three deterministic interval verdicts (Round 489/490).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum IntervalVerdict {
    /// Both operands and the bound resolved; the relation HELD.
    Satisfied {
        right_fact: String,
        right_value: String,
        bound: String,
    },
    /// Both resolved; the relation FAILED — the gate's violation.
    Violated {
        right_fact: String,
        right_value: String,
        bound: String,
    },
    /// An operand was absent on the right/bound leg, non-numeric, or ambiguous
    /// (>1 distinct holding value) — surfaced, never silently passed.
    Unverifiable { reason: String },
}

/// Resolve the verdict for one left fact (Round 489/490): parse the left
/// value, resolve the `right` operand and the bound at the left fact's canon
/// point, and compare. The scalar parse fail-louds — a non-numeric / absent /
/// ambiguous operand is `Unverifiable` with a reason, never silently skipped.
#[allow(clippy::too_many_arguments)]
fn interval_verdict(
    facts: &BTreeMap<String, NarrativeFact>,
    ctx: &WorldCtx<'_>,
    frame: &str,
    subject: &str,
    left_raw: &str,
    right_pred: &str,
    op: IntervalOp,
    bound: &IntervalBound,
    at: &str,
) -> IntervalVerdict {
    let unver = |reason: String| IntervalVerdict::Unverifiable { reason };
    let Some(left_num) = parse_scalar(left_raw) else {
        return unver(format!("left operand value `{left_raw}` is not numeric"));
    };
    let (right_num, right_value, right_fact) =
        match resolve_operand(facts, ctx, frame, subject, right_pred, at) {
            Operand::Value { num, value, fact } => (num, value.to_string(), fact.to_string()),
            Operand::Absent => {
                return unver(format!("right operand `{right_pred}` has no holding value"))
            }
            Operand::Unverifiable => {
                return unver(format!(
                    "right operand `{right_pred}` is non-numeric or ambiguous"
                ))
            }
        };
    let (bound_num, bound_str) = match bound {
        IntervalBound::Const(c) => (*c, c.to_string()),
        IntervalBound::Predicate(bp) => match resolve_operand(facts, ctx, frame, subject, bp, at) {
            Operand::Value { num, value, .. } => (num, value.to_string()),
            Operand::Absent => return unver(format!("bound `{bp}` has no holding value")),
            Operand::Unverifiable => {
                return unver(format!("bound `{bp}` is non-numeric or ambiguous"))
            }
        },
    };
    let legs = (right_fact, right_value, bound_str);
    if op.holds(left_num - right_num, bound_num) {
        IntervalVerdict::Satisfied {
            right_fact: legs.0,
            right_value: legs.1,
            bound: legs.2,
        }
    } else {
        IntervalVerdict::Violated {
            right_fact: legs.0,
            right_value: legs.1,
            bound: legs.2,
        }
    }
}

/// Evaluate one interval rule across all query worlds (Round 489/490, design
/// sec 7.20 — depth-ladder rung 1). Returns one [`IntervalOutcome`] per
/// (query world × holding left-operand fact), evaluated at the left fact's
/// canon coordinate so the earlier `right`/bound facts are read where the left
/// event lands. THE single interval evaluator — both `scan_continuity` (the
/// gate) and `timeline_gaps` (the read surface) consume these outcomes, so
/// they can never drift (R305/R390).
#[allow(clippy::too_many_arguments)]
fn scan_interval_rule(
    rule_id: &str,
    left_pred: &str,
    right_pred: &str,
    op: IntervalOp,
    bound: &IntervalBound,
    facts: &BTreeMap<String, NarrativeFact>,
    worlds: &[&str],
    lineages: &BTreeMap<String, Lineage>,
    order: &CanonOrder,
    successors: &BTreeMap<&str, Vec<(&str, &NarrativeFact)>>,
) -> Vec<IntervalOutcome> {
    let mut outcomes = Vec::new();
    for world in worlds {
        let ctx = WorldCtx {
            world,
            lineage: &lineages[*world],
            order,
            successors,
        };
        for (lid, lf) in facts {
            let Some(lt) = lf.typed.as_ref() else {
                continue;
            };
            if lt.predicate != left_pred {
                continue;
            }
            if !ctx.holds_at(lid, lf, &lf.canon_from) {
                continue;
            }
            let left_raw = typed_object_key(&lt.object);
            let verdict = interval_verdict(
                facts,
                &ctx,
                &lf.frame,
                &lt.subject,
                left_raw,
                right_pred,
                op,
                bound,
                &lf.canon_from,
            );
            outcomes.push(IntervalOutcome {
                rule: rule_id.to_string(),
                predicate: left_pred.to_string(),
                right: right_pred.to_string(),
                op: op.symbol().to_string(),
                frame: lf.frame.clone(),
                world: ctx.world.to_string(),
                subject: lt.subject.clone(),
                left_fact: lid.clone(),
                left_value: left_raw.to_string(),
                at: lf.canon_from.clone(),
                verdict,
            });
        }
    }
    outcomes
}

/// Every predicate a rule references is a load-bearing ref — the left operand
/// (`rule.predicate`) AND, for an interval rule, its `right` operand and
/// predicate-bound (Round 489). Checked in ONE place so no ref escapes the
/// typo guard, and SHARED by the gate (`scan_continuity`) and the read surface
/// (`timeline_gaps`) so neither can drift to a weaker check (the R436 lesson).
/// EXACT registry compare, deliberately untrimmed (R450): the loader
/// normalizes whitespace into the stored values, so a padded predicate
/// arriving here (a programmatic rule that skipped the loader) fails loud
/// instead of passing a trimmed check while the evaluation compares exact and
/// silently matches nothing.
fn check_rule_predicates(store: &AtomicStore, rules: &[NarrativeRule]) -> Result<(), String> {
    for rule in rules {
        for p in rule.referenced_predicates() {
            if !store.predicates.contains_key(p) {
                return Err(format!(
                    "narrative-rules: rule `{}` names predicate `{p}`, which is not in the \
                     predicate registry — a typo'd predicate would silently escape its rule \
                     (the R436 lesson); register it (add_predicate) or fix the declaration",
                    rule.id
                ));
            }
        }
    }
    Ok(())
}

/// One world's interval outcomes (Round 490). Every query world appears, so a
/// world with no gaps shows an explicit empty list (a clean dashboard).
#[derive(Debug, Clone, Default, Serialize)]
pub struct WorldTimelineGaps {
    pub outcomes: Vec<IntervalOutcome>,
}

/// Whole-store timeline-gap projection (Round 490, design sec 7.20 step 2):
/// the deterministic interval evaluator surfaced as a READ report, never
/// gated. Only `interval` rules contribute; exclusive/transition rules are
/// the continuity gate's, not a timeline surface.
#[derive(Debug, Clone, Default, Serialize)]
pub struct TimelineGapsReport {
    pub worlds: BTreeMap<String, WorldTimelineGaps>,
    /// Interval rules evaluated (0 = no interval rules declared).
    pub interval_rules: usize,
}

/// Run the interval rules as a read projection (Round 490). Same store
/// boundary + predicate-existence checks as the gate, the SAME evaluator
/// (`scan_interval_rule`) — the report is the gate's findings without the
/// gating, grouped per world. Surface-not-gate: no severity, no exit.
pub fn timeline_gaps(
    store: &AtomicStore,
    order: &CanonOrder,
    rules: &[NarrativeRule],
) -> Result<TimelineGapsReport, String> {
    check_store_boundary(store, order)?;
    check_rule_predicates(store, rules)?;
    let facts = &store.narrative_facts;
    let successors = successors_index(facts);
    let lineages = query_world_lineages(store)?;
    let worlds = query_worlds(store);
    let mut report = TimelineGapsReport::default();
    // Every world present, even the clean ones (explicit empty list).
    for w in &worlds {
        report
            .worlds
            .insert((*w).to_string(), WorldTimelineGaps::default());
    }
    for rule in rules {
        if let NarrativeRuleSpec::Interval { right, op, bound } = &rule.spec {
            report.interval_rules += 1;
            let outcomes = scan_interval_rule(
                &rule.id,
                &rule.predicate,
                right,
                *op,
                bound,
                facts,
                &worlds,
                &lineages,
                order,
                &successors,
            );
            for o in outcomes {
                report
                    .worlds
                    .entry(o.world.clone())
                    .or_default()
                    .outcomes
                    .push(o);
            }
        }
    }
    Ok(report)
}

pub fn scan_continuity(
    store: &AtomicStore,
    order: &CanonOrder,
    rules: &[NarrativeRule],
) -> Result<ContinuityReport, String> {
    check_store_boundary(store, order)?;
    check_rule_predicates(store, rules)?;
    let facts = &store.narrative_facts;
    let successors = successors_index(facts);
    let lineages = query_world_lineages(store)?;
    let mut report = ContinuityReport {
        facts: facts.len(),
        order_nodes: order.node_count(),
        ..Default::default()
    };
    // Canon-coordinate integrity (Round 488): a fact's canon coordinate must be
    // a node in its OWN branch's composed order. A coordinate the store's canon
    // positions ELSEWHERE (some branch names it) but that is absent from this
    // fact's branch order means the fact sits on the wrong world-line — the
    // silent wrong-branch error (e.g. defaulting to `main` when the trunk is a
    // named branch the forks inherit), which then keeps the conflict gate from
    // ever comparing it where it belongs. A coordinate named by NO branch's
    // order is the orderless/forward-declared mode, tolerated unchanged.
    let positioned: BTreeSet<&str> = order.nodes().collect();
    for (id, fact) in facts {
        for coord in std::iter::once(&fact.canon_from).chain(fact.canon_to.as_ref()) {
            if positioned.contains(coord.as_str()) && !order.names(&fact.branch, coord) {
                report
                    .violations
                    .push(ContinuityViolation::FactCanonOffBranch {
                        fact: id.clone(),
                        branch: fact.branch.clone(),
                        coord: coord.clone(),
                    });
            }
        }
    }
    // Evidence reachability (Round 522, design sec 7.27 Piece B): a
    // backreference cited in `evidence` must be reachable AT-OR-BEFORE the
    // fact's own coordinate in its world-line — the R488 off-branch principle
    // (canon_from) extended to evidence via the SAME `le`. Sibling-branch
    // evidence (no path in this branch) and a forward reference (in-branch but
    // after the fact) both fail; spine/prior evidence passes. Only positioned
    // coordinates are checked: an unpositioned `canon_from` is the
    // orderless/forward-declared mode (tolerated whole, matching
    // FactCanonOffBranch), and an unpositioned evidence coordinate is the same
    // orderless tolerance per reference.
    for (id, fact) in facts {
        if !positioned.contains(fact.canon_from.as_str()) {
            continue;
        }
        // The world-line(s) the evidence must be reachable in. The normal case
        // is the fact's OWN branch (R522). A fact authored on a CONFLUENCE is a
        // merge's shared suffix — it holds in EVERY incoming parent (forward
        // visibility), and the confluence's own order is prefix-less (it cannot
        // connect a parent's prefix to the suffix), so its dependencies are
        // checked against each PARENT's order instead (Round 535). The upper
        // bound stays the fact's own `canon_from` (which sits in the suffix,
        // downstream of the merge in each parent's composed order) — NOT the
        // merge coordinate, so suffix-internal evidence (a shared scene before
        // this fact) reaches correctly, while a parent-exclusive scene fails in
        // every sibling parent.
        let confluence_parents = store
            .branches
            .get(&fact.branch)
            .filter(|b| !b.converges_from.is_empty())
            .map(|b| b.converges_from.as_slice());
        for e in &fact.evidence {
            if !positioned.contains(e.as_str()) {
                continue;
            }
            match confluence_parents {
                None => {
                    if !order.le(&fact.branch, e, &fact.canon_from) {
                        report
                            .violations
                            .push(ContinuityViolation::EvidenceUnreachable {
                                fact: id.clone(),
                                branch: fact.branch.clone(),
                                evidence: e.clone(),
                                canon_from: fact.canon_from.clone(),
                            });
                    }
                }
                Some(parents) => {
                    for parent in parents {
                        if !order.le(&parent.branch, e, &fact.canon_from) {
                            report.violations.push(
                                ContinuityViolation::ConfluenceEvidenceUnreconciled {
                                    fact: id.clone(),
                                    confluence: fact.branch.clone(),
                                    parent: parent.branch.clone(),
                                    evidence: e.clone(),
                                    canon_from: fact.canon_from.clone(),
                                },
                            );
                        }
                    }
                }
            }
        }
    }
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
                Some(t)
                    if t.branch != s.branch
                        && !mnemosyne_core::succession_branch_inherits(
                            &store.branches,
                            &s.branch,
                            &t.branch,
                        )? =>
                {
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
    // Succession-cycle detection (Round 463): every write path rejects
    // cycles since the shared edge check landed, but the scan re-reads
    // out-of-band-edited stores (the Round 440 boundary doctrine). A
    // cycle's facts silently never hold anywhere (each derives the
    // other's end) — the exact silent-broken-state the R461 probe found.
    // Reported once per cycle, anchored at its minimum member id.
    for (sid, s) in facts {
        // A fact is ON a cycle exactly when it appears among its own
        // transitive predecessors — THE existing cycle-guarded walk
        // (`succession_ancestors`), not a second hand-rolled one.
        if s.supersedes_in_frame.is_none()
            || !succession_ancestors(facts, sid).contains(sid.as_str())
        {
            continue;
        }
        let mut cycle = vec![sid.clone()];
        let mut cur = s.supersedes_in_frame.as_deref().expect("checked above");
        while cur != sid {
            cycle.push(cur.to_string());
            // Total: the membership test above walked these exact edges.
            cur = facts[cur].supersedes_in_frame.as_deref().expect("walked");
        }
        if cycle.iter().min().map(String::as_str) == Some(sid.as_str()) {
            report
                .violations
                .push(ContinuityViolation::SuccessionCycle { cycle });
        }
    }
    // Payoff edge integrity (Round 442): identity refs re-checked against
    // out-of-band edits, exactly like conflict/succession targets.
    for (aid, a) in facts {
        for target in &a.pays_off {
            if !facts.contains_key(target) {
                report
                    .violations
                    .push(ContinuityViolation::PayoffTargetMissing {
                        fact_id: aid.clone(),
                        target: target.clone(),
                    });
            }
        }
    }
    // Distinct recorded conflict pairs (edges are read symmetrically).
    let mut pairs: BTreeSet<(String, String)> = BTreeSet::new();
    for (aid, a) in facts {
        for assertion in &a.conflicts_with {
            let target = &assertion.target;
            let Some(t) = facts.get(target) else {
                report
                    .violations
                    .push(ContinuityViolation::ConflictTargetMissing {
                        fact_id: aid.clone(),
                        target: target.clone(),
                    });
                continue;
            };
            // Judgment-time content pin (Round 439): a target claim that
            // changed since the assertion = stale judgment, surfaced.
            let current = claim_sha256_hex(&t.claim);
            if current != assertion.target_claim_sha256 {
                report
                    .violations
                    .push(ContinuityViolation::ConflictEdgeStale {
                        fact_id: aid.clone(),
                        target: target.clone(),
                        stamped_sha256: assertion.target_claim_sha256.clone(),
                        current_sha256: current,
                    });
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
        let Some(world) = join_world(a, b, &lineages) else {
            report.cross_scope_pairs += 1;
            continue;
        };
        let world = world.to_string();
        // Total by the Round 440 boundary: every fact branch is registered,
        // and `lineages` covers main + every registered branch.
        let ctx = WorldCtx {
            world: &world,
            lineage: &lineages[&world],
            order,
            successors: &successors,
        };
        let co_hold = store
            .sections
            .keys()
            .find(|p| ctx.holds_at(aid, a, p) && ctx.holds_at(bid, b, p));
        match co_hold {
            Some(p) => report
                .violations
                .push(ContinuityViolation::FrameConflictOverlap {
                    frame: a.frame.clone(),
                    branch: world.clone(),
                    fact_a: aid.clone(),
                    fact_b: bid.clone(),
                    at: p.clone(),
                }),
            None => {
                if !order.comparable(&world, &a.canon_from, &b.canon_from) {
                    report.unordered_pairs += 1;
                }
            }
        }
    }
    // Typed-claim rule gate (Round 449, design sec 7.12) — derivations over
    // the typed subset, evaluated per query world (main + every registered
    // branch, the R441 probe's executable model): cross-frame pairs and
    // sibling-world pairs are data by construction (a fact invisible in the
    // query world never holds there). A pair violating in several worlds is
    // reported per world — the world is part of the finding.
    //
    // Two scoping models coexist in this scan, deliberately (Round 452):
    // RECORDED conflict edges evaluate once per edge in the pair's join
    // world (B-2 — the edge is the finding's identity), while DERIVED rule
    // findings sweep every query world (the payoff-coverage shape — a
    // derived pair exists only relative to a world). One holds-semantics
    // under both: `WorldCtx::holds_at`.
    report.rules = rules.len();
    let worlds = query_worlds(store);
    for rule in rules {
        let typed: Vec<(&String, &NarrativeFact)> = facts
            .iter()
            .filter(|(_, f)| {
                f.typed
                    .as_ref()
                    .is_some_and(|t| t.predicate == rule.predicate)
            })
            .collect();
        match &rule.spec {
            NarrativeRuleSpec::Exclusive { per } => {
                let mut unordered: BTreeSet<(&str, &str)> = BTreeSet::new();
                for_each_world_pair(
                    &worlds,
                    &lineages,
                    order,
                    &successors,
                    &typed,
                    |ctx, aid, a, bid, b| {
                        let (ta, tb) = (a.typed.as_ref().unwrap(), b.typed.as_ref().unwrap());
                        if claim_leg(ta, *per) != claim_leg(tb, *per) {
                            return; // different keyed legs — no exclusivity claim
                        }
                        if claim_leg(ta, per.other()) == claim_leg(tb, per.other()) {
                            // The non-keyed leg agrees — a restated fact is
                            // exclusivity-consistent, not gated (R443: symmetric,
                            // both `per` directions).
                            return;
                        }
                        let co_hold = store
                            .sections
                            .keys()
                            .find(|p| ctx.holds_at(aid, a, p) && ctx.holds_at(bid, b, p));
                        match co_hold {
                            Some(p) => {
                                report
                                    .violations
                                    .push(ContinuityViolation::RuleExclusiveOverlap {
                                        rule: rule.id.clone(),
                                        predicate: rule.predicate.clone(),
                                        frame: a.frame.clone(),
                                        branch: ctx.world.to_string(),
                                        fact_a: aid.to_string(),
                                        fact_b: bid.to_string(),
                                        at: p.clone(),
                                    })
                            }
                            None => {
                                if !order.comparable(ctx.world, &a.canon_from, &b.canon_from) {
                                    unordered.insert((aid, bid));
                                }
                            }
                        }
                    },
                );
                report.rule_unordered_pairs += unordered.len();
            }
            NarrativeRuleSpec::Transition { allowed } => {
                let allowed: BTreeSet<(&str, &str)> = allowed
                    .iter()
                    .map(|pair| (pair[0].as_str(), pair[1].as_str()))
                    .collect();
                // The gated half: every typed succession edge with this
                // predicate and one subject must step inside `allowed`.
                // The edge itself is the scope — the write path already
                // confines succession to one frame and one world-line.
                for (sid, s) in &typed {
                    let st = s.typed.as_ref().unwrap();
                    let Some(pid) = &s.supersedes_in_frame else {
                        continue;
                    };
                    // A missing predecessor is already surfaced as
                    // SuccessionTargetMissing; an untyped or
                    // other-predicate/subject predecessor is outside this
                    // rule (partial coverage is the design).
                    let Some(pt) = facts.get(pid).and_then(|p| p.typed.as_ref()) else {
                        continue;
                    };
                    if pt.predicate != rule.predicate || pt.subject != st.subject {
                        continue;
                    }
                    let (from, to) = (typed_object_key(&pt.object), typed_object_key(&st.object));
                    if !allowed.contains(&(from, to)) {
                        report
                            .violations
                            .push(ContinuityViolation::RuleTransitionInvalid {
                                rule: rule.id.clone(),
                                predicate: rule.predicate.clone(),
                                frame: s.frame.clone(),
                                subject: st.subject.clone(),
                                predecessor: pid.clone(),
                                successor: (*sid).clone(),
                                from: from.to_string(),
                                to: to.to_string(),
                            });
                    }
                }
                // The honesty half: same-frame same-subject typed pairs
                // visible together in some world with NO succession PATH
                // between them — states the chain never connects, which the
                // transition rule therefore cannot see (surfaced count,
                // never gated). Path, not direct edge (Round 452 session
                // review): a correctly chained A→B→C arc transitively
                // connects (A, C) — each hop was checked, so counting the
                // pair as "unchained" was a false signal on correct data
                // (falsified live: a chained 4-step arc reported 3).
                // World-scoped via visibility (the R441 probe finding) and
                // deduplicated across worlds.
                let ancestors: BTreeMap<&str, BTreeSet<&str>> = typed
                    .iter()
                    .map(|(id, _)| (id.as_str(), succession_ancestors(facts, id)))
                    .collect();
                let mut seen: BTreeSet<(&str, &str)> = BTreeSet::new();
                for_each_world_pair(
                    &worlds,
                    &lineages,
                    order,
                    &successors,
                    &typed,
                    |_, aid, a, bid, b| {
                        if a.typed.as_ref().unwrap().subject != b.typed.as_ref().unwrap().subject {
                            return;
                        }
                        if ancestors[aid].contains(bid) || ancestors[bid].contains(aid) {
                            return; // connected through the succession chain
                        }
                        seen.insert((aid, bid));
                    },
                );
                report.unchained_state_pairs += seen.len();
            }
            NarrativeRuleSpec::Interval { right, op, bound } => {
                let outcomes = scan_interval_rule(
                    &rule.id,
                    &rule.predicate,
                    right,
                    *op,
                    bound,
                    facts,
                    &worlds,
                    &lineages,
                    order,
                    &successors,
                );
                // Gate adapter: a Violated outcome gates; Unverifiable surfaces
                // as a count deduplicated per (frame, world, subject) — several
                // left facts for one subject must not multiply-count.
                let mut unverifiable: BTreeSet<(&str, &str, &str)> = BTreeSet::new();
                for o in &outcomes {
                    match &o.verdict {
                        IntervalVerdict::Violated {
                            right_fact,
                            right_value,
                            bound,
                        } => report
                            .violations
                            .push(ContinuityViolation::RuleIntervalViolation {
                                rule: o.rule.clone(),
                                predicate: o.predicate.clone(),
                                right: o.right.clone(),
                                op: o.op.clone(),
                                frame: o.frame.clone(),
                                branch: o.world.clone(),
                                subject: o.subject.clone(),
                                left_fact: o.left_fact.clone(),
                                right_fact: right_fact.clone(),
                                left_value: o.left_value.clone(),
                                right_value: right_value.clone(),
                                bound: bound.clone(),
                                at: o.at.clone(),
                            }),
                        IntervalVerdict::Unverifiable { .. } => {
                            unverifiable.insert((&o.frame, &o.world, &o.subject));
                        }
                        IntervalVerdict::Satisfied { .. } => {}
                    }
                }
                report.interval_unverifiable += unverifiable.len();
            }
        }
    }
    Ok(report)
}

/// One fact currently in effect in a frame view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FrameViewEntry {
    pub fact_id: String,
    pub claim: String,
    pub entities: Vec<String>,
    pub canon_from: String,
    pub canon_to: Option<String>,
    pub evidence: Vec<String>,
    /// Typed leg (Round 446), surfaced verbatim when authored.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub typed: Option<mnemosyne_core::TypedClaim>,
    pub quote: Option<String>,
}

/// The frame-at-T projection result (Round 432). Three-state honest under a
/// partial order (B-1): a fact is `holding`, definitively `not_holding`
/// (counted), or `unknown` — some canon coordinate involved is not
/// comparable to the query point, so the declaration cannot decide. Scoped
/// to one world-line (`branch`, Round 433) — a view never mixes branches.
#[derive(Debug, Clone, Default, Serialize)]
pub struct FrameView {
    pub frame: String,
    pub branch: String,
    pub at: String,
    /// Entity filter applied (Round 437), `None` = unfiltered.
    pub entity: Option<String>,
    pub holding: Vec<FrameViewEntry>,
    pub not_holding: usize,
    pub unknown: Vec<String>,
}

/// "Facts of frame F in world-line B at canon point T" — the read
/// projection over the SAME `holds_at` semantics the continuity gate uses
/// (R390 single-predicate discipline: gate and view cannot drift). The
/// world includes inherited history (Round 438): facts on ancestor
/// branches up to each fork point are part of this view; a fork's own
/// revisions never leak back into the ancestor's view. Fail-loud
/// boundaries: the frame must be registered, the branch must be
/// `MAIN_BRANCH` or registered (Round 436), an `entity` filter must be
/// registered (Round 437 — the NPC-context query is frame × branch ×
/// entity at T), the query point must be a section, and the order
/// declaration must pass the shared store boundary.
pub fn frame_view(
    store: &AtomicStore,
    order: &CanonOrder,
    frame: &str,
    branch: &str,
    entity: Option<&str>,
    at: &str,
) -> Result<FrameView, String> {
    check_store_boundary(store, order)?;
    if !store.frames.contains_key(frame) {
        return Err(format!(
            "frame `{frame}` not present in the frames registry (fail-loud)"
        ));
    }
    if branch != mnemosyne_core::MAIN_BRANCH && !store.branches.contains_key(branch) {
        return Err(format!(
            "branch `{branch}` not present in the branch registry (fail-loud — a typo'd \
             branch must not read as an empty world)"
        ));
    }
    if let Some(e) = entity {
        if !store.entities.contains_key(e) {
            return Err(format!(
                "entity `{e}` not present in the entity registry (fail-loud — a typo'd \
                 entity must not read as an empty dossier)"
            ));
        }
    }
    if !store.sections.contains_key(at) {
        return Err(format!(
            "query point `{at}` not present as a section (canon coordinates are structure refs)"
        ));
    }
    let facts = &store.narrative_facts;
    let successors = successors_index(facts);
    let lineage = lineage_of(&store.branches, branch)?;
    let ctx = WorldCtx {
        world: branch,
        lineage: &lineage,
        order,
        successors: &successors,
    };
    let mut view = FrameView {
        frame: frame.to_string(),
        branch: branch.to_string(),
        at: at.to_string(),
        entity: entity.map(str::to_string),
        ..Default::default()
    };
    for (id, fact) in facts {
        if fact.frame != frame {
            continue;
        }
        if let Some(e) = entity {
            if !fact.entities.iter().any(|x| x == e) {
                continue;
            }
        }
        // World membership (Round 438): own branch, or inherited from an
        // ancestor up to the fork point. Definitively other worlds drop out
        // silently; an undecidable fork comparison is honest `unknown`.
        let vis = ctx.visibility(fact);
        if vis == Vis::Out {
            continue;
        }
        if vis == Vis::Unknown {
            view.unknown.push(id.clone());
            continue;
        }
        if ctx.holds_at(id, fact, at) {
            view.holding.push(FrameViewEntry {
                fact_id: id.clone(),
                claim: fact.claim.clone(),
                entities: fact.entities.clone(),
                canon_from: fact.canon_from.clone(),
                canon_to: fact.canon_to.clone(),
                evidence: fact.evidence.clone(),
                typed: fact.typed.clone(),
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
            .any(|(_, s)| ctx.visibility(s) == Vis::In && order.le(branch, &s.canon_from, at));
        if from_unknown || (to_unknown && !succ_cut) {
            view.unknown.push(id.clone());
        } else {
            view.not_holding += 1;
        }
    }
    Ok(view)
}

/// One payoff edge reference surfaced by the coverage report (Round 442).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PayoffEdgeRef {
    pub payoff: String,
    pub setup: String,
}

/// One paid setup with the in-world payoffs that credit it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PaidSetup {
    pub setup: String,
    pub payoffs: Vec<String>,
}

/// Per-world payoff coverage (Round 442) — the R390 3-way classification
/// on the discourse axis: a visible setup with a visible payoff is `paid`,
/// without one it is `dangling` (the author's todo list — a report
/// finding, deliberately never a gate reject: a WIP story has dangling
/// setups by definition), and unmarked facts are `exempt` (counted, not
/// listed). Honesty counts ride along: `payoffs_to_unmarked` (a payoff
/// aimed at a fact nobody marked as a setup — often a forgotten marking),
/// `payoff_before_setup` (legal mystery/flashback structure, surfaced
/// never gated), and `unknown` (world visibility undecidable under the
/// declared order — B-1, mirroring the frame view).
#[derive(Debug, Clone, Default, Serialize)]
pub struct WorldPayoffCoverage {
    pub paid: Vec<PaidSetup>,
    pub dangling: Vec<String>,
    pub exempt: usize,
    pub payoffs_to_unmarked: Vec<PayoffEdgeRef>,
    pub payoff_before_setup: Vec<PayoffEdgeRef>,
    pub unknown: Vec<String>,
}

/// Setup/payoff coverage over every query world (Round 442). Pure read
/// projection — severity/gating policy deliberately does not exist for
/// dangling setups.
#[derive(Debug, Clone, Default, Serialize)]
pub struct PayoffCoverageReport {
    pub worlds: BTreeMap<String, WorldPayoffCoverage>,
    pub facts: usize,
    /// Distinct facts marked `expected`, store-wide (before world scoping).
    pub setups_total: usize,
    /// Recorded payoff edges that credited a setup in NO world (Round 443
    /// session review): both endpoints exist, but no query world ever sees
    /// them together — e.g. a payoff on one sibling branch naming a setup
    /// on another. The dangling list shows the symptom in the setup's
    /// world; this surfaces the dead edge itself (the author thinks the
    /// gun was paid; no world agrees). An edge is exempted from this list
    /// only by a world where it COULD still credit (Round 447 — see
    /// `undecidable_edges`); a world where either endpoint is Out is
    /// decided and exempts nothing.
    pub uncredited_edges: Vec<PayoffEdgeRef>,
    /// Edges that credited nowhere AND met a could-credit undecidable
    /// world — both endpoints In/Unknown with at least one Unknown, so the
    /// declared order cannot decide their fate there (Round 447, the R445
    /// Detroit Finding 3 fix). Surfaced instead of silently withdrawn
    /// (no-silent-caps): under parallel protagonist chains the old blanket
    /// withdrawal masked genuinely dead cross-chain edges behind Unknowns
    /// in unrelated forks. B-1 honesty either way: these are *undecided*,
    /// never listed as definitively dead.
    pub undecidable_edges: Vec<PayoffEdgeRef>,
}

impl PayoffCoverageReport {
    /// The per-world dangling setups, keeping only worlds with ≥ 1 dangling
    /// (Round 600 SSOT). The ONE home for the "dangling setups by world"
    /// projection — both `propose-verdict`'s dry run (R599) and the authoring
    /// frontier (R589) surface it, and had copy-pasted the transform.
    pub fn dangling_by_world(&self) -> BTreeMap<String, Vec<String>> {
        self.worlds
            .iter()
            .filter(|(_, w)| !w.dangling.is_empty())
            .map(|(world, w)| (world.clone(), w.dangling.clone()))
            .collect()
    }
}

/// Classify setup/payoff coverage per world (Round 442). WORLD-scoped via
/// the shared [`visibility`] semantics — an inherited setup dangles on a
/// fork until that world-line itself pays it (each playthrough resolves
/// its own guns; forking early surfaces all the narrative debt the new
/// world inherits), and a fork's payoff never credits the ancestor's
/// world. Payoff edges cross FRAMES freely (setup/payoff is a
/// discourse-structure relation, not an epistemic judgment) but never
/// cross worlds (an edge whose other end is not visible here is inert in
/// this world's classification). Facts with undecidable visibility are
/// surfaced as `unknown`, never classified (B-1).
pub fn payoff_coverage(
    store: &AtomicStore,
    order: &CanonOrder,
) -> Result<PayoffCoverageReport, String> {
    check_store_boundary(store, order)?;
    let facts = &store.narrative_facts;
    let mut report = PayoffCoverageReport {
        facts: facts.len(),
        setups_total: facts
            .values()
            .filter(|f| f.payoff_expectation == mnemosyne_core::PayoffExpectation::Expected)
            .count(),
        ..Default::default()
    };
    // Every recorded edge with an existing target (a missing target is the
    // scan's finding, not the report's). Edges drain from this set as some
    // world credits them; what remains either surfaces as definitively
    // dead or, when a could-credit world was undecidable, as undecidable
    // (Rounds 443 + 447).
    let mut never_credited: BTreeSet<(String, String)> = facts
        .iter()
        .flat_map(|(pid, p)| {
            p.pays_off
                .iter()
                .filter(|t| facts.contains_key(*t))
                .map(|t| (pid.clone(), t.clone()))
        })
        .collect();
    let mut undecidable: BTreeSet<(String, String)> = BTreeSet::new();
    let worlds: Vec<String> = query_worlds(store)
        .into_iter()
        .map(str::to_string)
        .collect();
    for world in worlds {
        let lineage = lineage_of(&store.branches, &world)?;
        let mut vis_by_id: BTreeMap<&str, Vis> = BTreeMap::new();
        let mut visible: BTreeMap<&str, &NarrativeFact> = BTreeMap::new();
        let mut unknown: Vec<String> = Vec::new();
        for (id, fact) in facts {
            let vis = visibility(&world, &lineage, order, fact);
            vis_by_id.insert(id.as_str(), vis);
            match vis {
                Vis::In => {
                    visible.insert(id.as_str(), fact);
                }
                Vis::Unknown => unknown.push(id.clone()),
                Vis::Out => {}
            }
        }
        // Round 447 (the R445 Detroit Finding 3 fix): an Unknown endpoint
        // suspends the dead-edge verdict ONLY in a world where the edge
        // could actually credit — both endpoints In/Unknown. A world where
        // either endpoint is Out is DECIDED (the edge cannot credit there)
        // regardless of any Unknown: the pre-fix blanket withdrawal let an
        // Unknown in an unrelated fork (parallel protagonist chains make
        // every cross-chain fact Unknown there) silently mask genuinely
        // dead edges. Suspended edges surface as `undecidable_edges`, not
        // as definitively dead (B-1 honesty, no silent caps).
        for (pid, p) in facts {
            for target in &p.pays_off {
                // An edge endpoint outside the fact map (a missing target)
                // is the scan's finding, never in `never_credited` — Out
                // is the honest default here.
                let endpoint = |id: &str| vis_by_id.get(id).copied().unwrap_or(Vis::Out);
                let could_credit_undecided = matches!(
                    (endpoint(pid), endpoint(target.as_str())),
                    (Vis::In, Vis::Unknown)
                        | (Vis::Unknown, Vis::In)
                        | (Vis::Unknown, Vis::Unknown)
                );
                if could_credit_undecided {
                    undecidable.insert((pid.clone(), target.clone()));
                }
            }
        }
        let mut cov = WorldPayoffCoverage {
            unknown,
            ..Default::default()
        };
        // In-world payoff index: setup id -> crediting payoff ids. Edges
        // whose target is not visible here are inert (cross-world edge),
        // except the honesty counts below.
        let mut paid_by: BTreeMap<&str, Vec<String>> = BTreeMap::new();
        for (pid, p) in &visible {
            for target in &p.pays_off {
                let Some(t) = visible.get(target.as_str()) else {
                    continue;
                };
                paid_by
                    .entry(target.as_str())
                    .or_default()
                    .push((*pid).to_string());
                never_credited.remove(&((*pid).to_string(), target.clone()));
                if t.payoff_expectation != mnemosyne_core::PayoffExpectation::Expected {
                    cov.payoffs_to_unmarked.push(PayoffEdgeRef {
                        payoff: (*pid).to_string(),
                        setup: target.clone(),
                    });
                }
                if p.canon_from != t.canon_from && order.le(&world, &p.canon_from, &t.canon_from) {
                    cov.payoff_before_setup.push(PayoffEdgeRef {
                        payoff: (*pid).to_string(),
                        setup: target.clone(),
                    });
                }
            }
        }
        for (id, fact) in &visible {
            if fact.payoff_expectation != mnemosyne_core::PayoffExpectation::Expected {
                cov.exempt += 1;
                continue;
            }
            match paid_by.get(id) {
                Some(payoffs) => cov.paid.push(PaidSetup {
                    setup: (*id).to_string(),
                    payoffs: payoffs.clone(),
                }),
                None => cov.dangling.push((*id).to_string()),
            }
        }
        report.worlds.insert(world, cov);
    }
    let (undecided, dead): (Vec<_>, Vec<_>) = never_credited
        .into_iter()
        .partition(|e| undecidable.contains(e));
    report.uncredited_edges = dead
        .into_iter()
        .map(|(payoff, setup)| PayoffEdgeRef { payoff, setup })
        .collect();
    report.undecidable_edges = undecided
        .into_iter()
        .map(|(payoff, setup)| PayoffEdgeRef { payoff, setup })
        .collect();
    Ok(report)
}

/// Per-world deterministic payoff SUBSTANTIATION (Round 485). Refines
/// [`payoff_coverage`]'s `paid` set — a credited setup is `substantiated` only
/// when a typed state-change actually discharges it, never on the bare
/// existence of a `pays_off` edge. NO model judgment: the verdict is a
/// deterministic comparison of declared typed legs (R484 — the all-deterministic
/// redesign that replaced the R481 LLM-verdict drift surface). Three outcomes
/// for each credited setup:
/// - `substantiated`: the setup carries a typed state `(subject, predicate,
///   V0)` AND ≥ 1 of its visible payoff facts carries a typed leg on the SAME
///   `(subject, predicate)` with a different value (a state change discharging
///   the setup). `payoffs` lists only the discharging facts.
/// - `unsubstantiated`: the setup is typed but NO crediting payoff carries that
///   discharging state-change — a hollow payoff (the edge exists, the typed
///   backing does not). This is the deterministic analogue of the drift R481
///   chased with an LLM.
/// - `unverifiable`: the typed data needed to check a discharge is absent —
///   the setup has no typed state, OR every crediting payoff is prose-only.
///   Surfaced, never silently passed. The honest boundary: an untyped payoff
///   chain cannot be machine-verified; the author types it (the
///   typing-discovery pull) and the gate then decides. This is the dominant
///   class on a prose-first store and is the correct deterministic statement,
///   not a failure.
#[derive(Debug, Clone, Default, Serialize)]
pub struct WorldPayoffSubstantiation {
    pub substantiated: Vec<PaidSetup>,
    pub unsubstantiated: Vec<PaidSetup>,
    pub unverifiable: Vec<PaidSetup>,
}

/// Whole-store payoff substantiation (Round 485). Pure read projection over the
/// declared typed structure — no LLM, re-runnable, deterministic.
#[derive(Debug, Clone, Default, Serialize)]
pub struct PayoffSubstantiationReport {
    pub worlds: BTreeMap<String, WorldPayoffSubstantiation>,
    /// Distinct facts marked `expected`, store-wide (pass-through from coverage).
    pub setups_total: usize,
}

/// Does any crediting payoff fact carry a typed leg that discharges the setup's
/// typed state — same subject and predicate, different value? Deterministic.
fn discharging_payoffs(
    facts: &BTreeMap<String, NarrativeFact>,
    setup_typed: &mnemosyne_core::TypedClaim,
    payoffs: &[String],
) -> Vec<String> {
    let v0 = typed_object_key(&setup_typed.object);
    payoffs
        .iter()
        .filter(|pid| {
            facts
                .get(*pid)
                .and_then(|f| f.typed.as_ref())
                .is_some_and(|t| {
                    t.subject == setup_typed.subject
                        && t.predicate == setup_typed.predicate
                        && typed_object_key(&t.object) != v0
                })
        })
        .cloned()
        .collect()
}

/// Classify every credited setup as substantiated / unsubstantiated /
/// unverifiable, per world (Round 485). Reuses [`payoff_coverage`] for the
/// world-scoped paid set, then applies the deterministic typed-discharge rule.
pub fn payoff_substantiation(
    store: &AtomicStore,
    order: &CanonOrder,
) -> Result<PayoffSubstantiationReport, String> {
    let coverage = payoff_coverage(store, order)?;
    let facts = &store.narrative_facts;
    let mut report = PayoffSubstantiationReport {
        setups_total: coverage.setups_total,
        ..Default::default()
    };
    for (world, cov) in &coverage.worlds {
        let mut w = WorldPayoffSubstantiation::default();
        for paid in &cov.paid {
            // The setup is present (it was just credited by payoff_coverage).
            let setup_typed = facts.get(&paid.setup).and_then(|f| f.typed.as_ref());
            match setup_typed {
                // Setup carries no typed state -> a discharge is undefinable.
                None => w.unverifiable.push(paid.clone()),
                Some(ts) => {
                    let any_typed_payoff = paid
                        .payoffs
                        .iter()
                        .any(|pid| facts.get(pid).is_some_and(|f| f.typed.is_some()));
                    if !any_typed_payoff {
                        // Setup typed, but every crediting payoff is prose-only:
                        // the discharge cannot be checked deterministically.
                        w.unverifiable.push(paid.clone());
                    } else {
                        let discharging = discharging_payoffs(facts, ts, &paid.payoffs);
                        if discharging.is_empty() {
                            // Typed payoff(s) exist but none changes the setup's
                            // typed state — a hollow payoff.
                            w.unsubstantiated.push(paid.clone());
                        } else {
                            w.substantiated.push(PaidSetup {
                                setup: paid.setup.clone(),
                                payoffs: discharging,
                            });
                        }
                    }
                }
            }
        }
        report.worlds.insert(world.clone(), w);
    }
    Ok(report)
}

/// One recorded cross-frame conflict edge (read symmetrically; endpoints
/// id-ordered like the gate's pair key).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IronyEdgeRef {
    pub fact_a: String,
    pub fact_b: String,
}

/// One dramatic-irony window (Round 455, design sec 7.14): the canon
/// region of a query world where both ends of a recorded CROSS-FRAME
/// conflict edge are simultaneously in effect. Deliberately a node SET,
/// not a span — under a partial (DAG) order the co-hold region need not
/// be contiguous, and a (from, to) pair would lie about that.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IronyWindow {
    pub fact_a: String,
    pub fact_b: String,
    pub frame_a: String,
    pub frame_b: String,
    /// Sections where both endpoints hold, sorted.
    pub nodes: Vec<String>,
    /// Minimal co-hold nodes under this world's composed order (where the
    /// window opens; several when the region starts on incomparable nodes).
    pub starts: Vec<String>,
    /// The window contains a maximal node of this world's declared
    /// composed order — the divergence is never resolved on this
    /// world-line (the R454 headline insight, "the belief never closes").
    pub open: bool,
}

/// Per-world irony classification (Round 455). `windowless` = both
/// endpoints visible here, comparable starts, never co-holding (the
/// belief genuinely never overlaps the truth in this world — data, e.g. a
/// belief corrected before the truth lands); `unordered` = no co-hold AND
/// the declared order cannot compare the starts (Round 456 — the gate's
/// `unordered_pairs` idiom mirrored: an incomparable pair is *undeclared*,
/// not resolved, and calling it windowless would overstate — under a
/// richer order declaration it may be a window); `undecidable` = an
/// endpoint with `Unknown` visibility (B-1, never classified). An edge
/// with an `Out` endpoint is not this world's business and reports where
/// it IS visible.
#[derive(Debug, Clone, Default, Serialize)]
pub struct WorldIrony {
    pub windows: Vec<IronyWindow>,
    pub windowless: Vec<IronyEdgeRef>,
    pub unordered: Vec<IronyEdgeRef>,
    pub undecidable: Vec<IronyEdgeRef>,
}

/// Dramatic-irony intervals over every query world (Round 455) — pure
/// read projection, never gated (irony is craft signal, not defect).
#[derive(Debug, Clone, Default, Serialize)]
pub struct IronyIntervalsReport {
    pub worlds: BTreeMap<String, WorldIrony>,
    pub facts: usize,
    /// Distinct recorded cross-frame conflict pairs (the report's input).
    pub cross_frame_edges: usize,
    /// Distinct same-frame pairs skipped — the continuity gate's
    /// territory (`frame_conflict_overlap`), surfaced so the split is
    /// never silent.
    pub same_frame_edges: usize,
}

/// Derive dramatic-irony windows (Round 455, design sec 7.14): for every
/// query world (main + every registered branch — the derived-finding
/// scoping of the R452 pin: a cross-frame edge has no join world by
/// construction, so the window exists only relative to a world's
/// visibility), every recorded cross-frame conflict edge with both
/// endpoints visible classifies as a window (the co-hold node set under
/// [`WorldCtx::holds_at`] — its 4th reader, no interval algebra of its
/// own), windowless, or unordered (incomparable starts — Round 456).
/// Missing conflict targets are the scan's finding
/// (`ConflictTargetMissing`), not the report's — mirrored from the
/// payoff-coverage precedent.
pub fn irony_intervals(
    store: &AtomicStore,
    order: &CanonOrder,
) -> Result<IronyIntervalsReport, String> {
    check_store_boundary(store, order)?;
    let facts = &store.narrative_facts;
    let successors = successors_index(facts);
    // Distinct recorded pairs with existing endpoints, id-ordered (the
    // gate's pair key), split by frame locus.
    let mut cross: BTreeSet<(&str, &str)> = BTreeSet::new();
    let mut same: BTreeSet<(&str, &str)> = BTreeSet::new();
    for (aid, a) in facts {
        for assertion in &a.conflicts_with {
            let Some(t) = facts.get(&assertion.target) else {
                continue;
            };
            let key = if aid.as_str() < assertion.target.as_str() {
                (aid.as_str(), assertion.target.as_str())
            } else {
                (assertion.target.as_str(), aid.as_str())
            };
            if a.frame == t.frame {
                same.insert(key);
            } else {
                cross.insert(key);
            }
        }
    }
    let mut report = IronyIntervalsReport {
        facts: facts.len(),
        cross_frame_edges: cross.len(),
        same_frame_edges: same.len(),
        ..Default::default()
    };
    let worlds: Vec<String> = query_worlds(store)
        .into_iter()
        .map(str::to_string)
        .collect();
    for world in worlds {
        let lineage = lineage_of(&store.branches, &world)?;
        let ctx = WorldCtx {
            world: &world,
            lineage: &lineage,
            order,
            successors: &successors,
        };
        let mut out = WorldIrony::default();
        for (aid, bid) in &cross {
            let (a, b) = (&facts[*aid], &facts[*bid]);
            let (va, vb) = (ctx.visibility(a), ctx.visibility(b));
            if va == Vis::Out || vb == Vis::Out {
                continue; // not this world's business — reports where visible
            }
            let edge = IronyEdgeRef {
                fact_a: (*aid).to_string(),
                fact_b: (*bid).to_string(),
            };
            if va == Vis::Unknown || vb == Vis::Unknown {
                out.undecidable.push(edge);
                continue;
            }
            let nodes: Vec<String> = store
                .sections
                .keys()
                .filter(|p| ctx.holds_at(aid, a, p) && ctx.holds_at(bid, b, p))
                .cloned()
                .collect();
            if nodes.is_empty() {
                // The gate's idiom (Round 456): incomparable starts mean
                // the declaration cannot order the pair — surfaced as
                // unordered, never asserted resolved.
                if order.comparable(&world, &a.canon_from, &b.canon_from) {
                    out.windowless.push(edge);
                } else {
                    out.unordered.push(edge);
                }
                continue;
            }
            let starts: Vec<String> = nodes
                .iter()
                .filter(|n| !nodes.iter().any(|m| m != *n && order.le(&world, m, n)))
                .cloned()
                .collect();
            let open = nodes.iter().any(|n| order.is_maximal(&world, n));
            out.windows.push(IronyWindow {
                fact_a: (*aid).to_string(),
                fact_b: (*bid).to_string(),
                frame_a: a.frame.clone(),
                frame_b: b.frame.clone(),
                nodes,
                starts,
                open,
            });
        }
        report.worlds.insert(world, out);
    }
    Ok(report)
}

/// The render-brief disclosure decision for a fact under a telling (Round
/// 506, design sec 7.24) — attached to a begins-event only when the
/// `--telling` carrier is given. `mode` = the effective disclosure mode (the
/// per-fact override, or the plan's default); `first_at` = the reader's
/// first-learn coordinate for THIS world (the override's per-world-line pin,
/// `None` when defaulted or unpinned for this world — distinct from the fact's
/// `canon_from` = when it is TRUE); `surface` = the diegetic carrier. Craft
/// guidance for the LLM render step (Layer B), NEVER gated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FactDisclosure {
    /// The effective disclosure mode — serializes as its snake_case tag
    /// (`withhold`/`state`/`hint`/`imply`); a typed enum, not a stringly field
    /// (Round 510).
    pub mode: mnemosyne_core::DisclosureMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub surface: Option<mnemosyne_core::DisclosureSurface>,
}

/// One fact event in a playthrough scene (Round 466, design sec 7.17) —
/// the [`FrameViewEntry`] mirror + the frame label: the manuscript is
/// world-scoped, so frame is data on the event (a renderer splits
/// reader-knowledge from character-belief without a second query).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ManuscriptFactEvent {
    pub fact_id: String,
    pub frame: String,
    pub claim: String,
    pub entities: Vec<String>,
    pub canon_from: String,
    pub canon_to: Option<String>,
    pub evidence: Vec<String>,
    /// Typed leg (Round 446), surfaced verbatim when authored.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub typed: Option<mnemosyne_core::TypedClaim>,
    pub quote: Option<String>,
    /// Render-brief disclosure decision under the `--telling` carrier (Round
    /// 506) — `None` unless a telling is given; the craft-bearing input the
    /// bare fact list lacked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disclosure: Option<FactDisclosure>,
}

/// Why a fact's effect ends at a scene (Round 466) — two DECLARED kinds
/// with distinct semantics, surfaced verbatim (no derived algebra):
/// `Expired` = `canon_to` equals the scene node (the fact still holds AT
/// it, through it — this is its last scene); `Superseded` = a visible
/// successor's `canon_from` equals the scene node (the replaced fact no
/// longer holds FROM it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ManuscriptEndKind {
    Expired,
    Superseded,
}

/// One end event in a playthrough scene (Round 466).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ManuscriptEndEvent {
    pub fact_id: String,
    pub frame: String,
    pub kind: ManuscriptEndKind,
    /// The cutting successor (`Superseded` only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub by: Option<String>,
}

/// One scene of a world's manuscript (Round 466): the order node, its
/// skeleton title, the EPUB pointer verbatim when authored (the
/// renderer's prose source — facts alone are a wireframe; prose stays in
/// the content-SSOT), the declared fact events, and the holds-judged
/// count (the delta story and the holds semantics cross-check each
/// other — a delta reconstruction that disagrees with the count has hit
/// an unplaced coordinate, never a second semantics).
#[derive(Debug, Clone, Serialize)]
pub struct ManuscriptScene {
    pub section: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub epub_locator: Option<mnemosyne_atomic::EpubLocator>,
    pub begins: Vec<ManuscriptFactEvent>,
    pub ends: Vec<ManuscriptEndEvent>,
    pub holding_count: usize,
}

/// A visible fact the manuscript cannot place (Round 466, B-1): the named
/// coordinate is a section, but this world's composed order never names
/// it, so no scene carries the event — surfaced, never silently dropped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ManuscriptUnplacedFact {
    pub fact_id: String,
    /// Which declared field points outside the order: `canon_from`,
    /// `canon_to`, or `successor_canon_from`.
    pub field: String,
    pub coordinate: String,
    /// The cutting successor (`successor_canon_from` only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub successor: Option<String>,
}

/// One world's linear manuscript (Round 466, design sec 7.17).
#[derive(Debug, Clone, Default, Serialize)]
pub struct WorldManuscript {
    pub scenes: Vec<ManuscriptScene>,
    /// Adjacent emitted pairs the composed order cannot compare — the
    /// linearization is ONE valid reading; a rendering may reorder inside
    /// such an adjacency freely (the 7.14 span lesson carried to
    /// sequences: silently totalizing an incomparable pair would lie).
    pub undeclared_adjacencies: Vec<[String; 2]>,
    pub unplaced_facts: Vec<ManuscriptUnplacedFact>,
    /// `Vis::Unknown` facts (B-1) — never placed, never counted holding.
    pub undecidable: Vec<String>,
    /// Store sections this world's declaration never names — isolated
    /// coordinates (Round 456), never scenes.
    pub sections_outside_order: Vec<String>,
}

/// Playthrough manuscripts over query worlds (Round 466) — pure read
/// projection, never gated (a manuscript is a reading surface, not a
/// defect detector).
#[derive(Debug, Clone, Default, Serialize)]
pub struct PlaythroughManuscriptReport {
    pub worlds: BTreeMap<String, WorldManuscript>,
    pub facts: usize,
}

/// Linearize query worlds into readable scene sequences (Round 466,
/// design sec 7.17): per world (main + every registered branch, or the
/// `world` filter — fail-loud on an unregistered id, the [`frame_view`]
/// branch-check idiom), the composed order's deterministic topological
/// walk with declared fact events placed on it. `begins`/`ends` are
/// exact-match declared coordinates; `holding_count` is judged by
/// [`WorldCtx::holds_at`] VERBATIM (its 5th reader — one semantics, no
/// drift). Everything the walk cannot place is surfaced (B-1, no silent
/// caps).
pub fn playthrough_manuscript(
    store: &AtomicStore,
    order: &CanonOrder,
    world: Option<&str>,
    telling: Option<&str>,
) -> Result<PlaythroughManuscriptReport, String> {
    check_store_boundary(store, order)?;
    if let Some(w) = world {
        if w != mnemosyne_core::MAIN_BRANCH && !store.branches.contains_key(w) {
            return Err(format!(
                "world `{w}` not present in the branch registry (fail-loud — a typo'd \
                 world must not read as an empty manuscript)"
            ));
        }
    }
    // Round 506 — the render-brief disclosure carrier: resolve the named
    // telling ONCE (fail-loud on a typo, the registry ethos — a missing telling
    // must not silently render with no disclosure plan). `None` = no carrier,
    // every begins-event's `disclosure` stays `None` (byte-stable output).
    let plan = match telling {
        Some(t) => Some(store.disclosure_plans.get(t).ok_or_else(|| {
            format!(
                "telling `{t}` not present in the disclosure_plans registry (fail-loud — \
                 a typo'd telling must not silently render with no disclosure plan)"
            )
        })?),
        None => None,
    };
    let facts = &store.narrative_facts;
    let successors = successors_index(facts);
    let mut report = PlaythroughManuscriptReport {
        facts: facts.len(),
        ..Default::default()
    };
    // Explicit `--world` renders any registered branch (incl. a confluence
    // fragment, for inspection); the default dump sweeps the PLAYTHROUGHS only
    // (Round 533 `query_worlds` — a confluence's shared suffix already renders
    // WITHIN each parent's manuscript via forward visibility, so it is not also
    // a standalone world).
    let worlds: Vec<String> = match world {
        Some(w) => vec![w.to_string()],
        None => query_worlds(store)
            .into_iter()
            .map(str::to_string)
            .collect(),
    };
    for world in worlds {
        let lineage = lineage_of(&store.branches, &world)?;
        let ctx = WorldCtx {
            world: &world,
            lineage: &lineage,
            order,
            successors: &successors,
        };
        let sequence = order.linearize(&world);
        let node_set: BTreeSet<&str> = sequence.iter().map(String::as_str).collect();
        let mut out = WorldManuscript {
            undeclared_adjacencies: sequence
                .windows(2)
                .filter(|w| !order.comparable(&world, &w[0], &w[1]))
                .map(|w| [w[0].clone(), w[1].clone()])
                .collect(),
            sections_outside_order: store
                .sections
                .keys()
                .filter(|s| !node_set.contains(s.as_str()))
                .cloned()
                .collect(),
            ..Default::default()
        };
        // Visibility split + placement honesty, one pass (facts iterate
        // id-sorted, so every surface below is deterministic).
        for (id, fact) in facts {
            match ctx.visibility(fact) {
                Vis::Out => continue,
                Vis::Unknown => {
                    out.undecidable.push(id.clone());
                    continue;
                }
                Vis::In => {}
            }
            if !node_set.contains(fact.canon_from.as_str()) {
                out.unplaced_facts.push(ManuscriptUnplacedFact {
                    fact_id: id.clone(),
                    field: "canon_from".to_string(),
                    coordinate: fact.canon_from.clone(),
                    successor: None,
                });
            }
            if let Some(to) = &fact.canon_to {
                if !node_set.contains(to.as_str()) {
                    out.unplaced_facts.push(ManuscriptUnplacedFact {
                        fact_id: id.clone(),
                        field: "canon_to".to_string(),
                        coordinate: to.clone(),
                        successor: None,
                    });
                }
            }
            for (sid, s) in successors.get(id.as_str()).into_iter().flatten() {
                if ctx.visibility(s) == Vis::In && !node_set.contains(s.canon_from.as_str()) {
                    out.unplaced_facts.push(ManuscriptUnplacedFact {
                        fact_id: id.clone(),
                        field: "successor_canon_from".to_string(),
                        coordinate: s.canon_from.clone(),
                        successor: Some((*sid).to_string()),
                    });
                }
            }
        }
        for node in &sequence {
            let mut scene = ManuscriptScene {
                section: node.clone(),
                title: store
                    .sections
                    .get(node)
                    .map(|s| s.skeleton.title.clone())
                    .unwrap_or_default(),
                epub_locator: store
                    .sections
                    .get(node)
                    .and_then(|s| s.epub_locator.clone()),
                begins: Vec::new(),
                ends: Vec::new(),
                holding_count: 0,
            };
            for (id, fact) in facts {
                if ctx.visibility(fact) != Vis::In {
                    continue;
                }
                if fact.canon_from == *node {
                    scene.begins.push(ManuscriptFactEvent {
                        fact_id: id.clone(),
                        frame: fact.frame.clone(),
                        claim: fact.claim.clone(),
                        entities: fact.entities.clone(),
                        canon_from: fact.canon_from.clone(),
                        canon_to: fact.canon_to.clone(),
                        evidence: fact.evidence.clone(),
                        typed: fact.typed.clone(),
                        quote: fact.quote.clone(),
                        disclosure: plan.map(|p| resolve_fact_disclosure(p, &world, id)),
                    });
                }
                if fact.canon_to.as_deref() == Some(node.as_str()) {
                    scene.ends.push(ManuscriptEndEvent {
                        fact_id: id.clone(),
                        frame: fact.frame.clone(),
                        kind: ManuscriptEndKind::Expired,
                        by: None,
                    });
                }
                for (sid, s) in successors.get(id.as_str()).into_iter().flatten() {
                    if ctx.visibility(s) == Vis::In && s.canon_from == *node {
                        scene.ends.push(ManuscriptEndEvent {
                            fact_id: id.clone(),
                            frame: fact.frame.clone(),
                            kind: ManuscriptEndKind::Superseded,
                            by: Some((*sid).to_string()),
                        });
                    }
                }
                if ctx.holds_at(id, fact, node) {
                    scene.holding_count += 1;
                }
            }
            out.scenes.push(scene);
        }
        report.worlds.insert(world, out);
    }
    Ok(report)
}

/// Map a fact's effective disclosure (the single resolver,
/// [`mnemosyne_core::DisclosurePlan::effective`]) into the carrier's
/// begins-event shape (Round 506; Round 510 routes through the shared resolver
/// so the carrier and the coverage surface cannot drift on the
/// override-vs-default rule).
fn resolve_fact_disclosure(
    plan: &mnemosyne_core::DisclosurePlan,
    world: &str,
    fact_id: &str,
) -> FactDisclosure {
    let effective = plan.effective(fact_id, world);
    FactDisclosure {
        mode: effective.mode,
        first_at: effective.first_at,
        surface: effective.surface,
    }
}

/// A fork's divergence coordinate, resolved against the parent world's
/// composed order (Round 497, design sec 7.21).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ForkTreeEdge {
    /// Parent world-line (`MAIN_BRANCH` or a registered branch).
    pub parent: String,
    /// The canon point of divergence — the CYOA choice-point scene.
    pub at: String,
    /// `at` is a node of the PARENT's composed order ([`CanonOrder::names`],
    /// Round 488) — the scene the assembler hangs the choice on. `false` =
    /// a declaration gap (the parent's order never names the fork point);
    /// the branch id is also listed in `unplaced_fork_points`, never
    /// silently dropped (the R466 `unplaced_facts` idiom).
    pub at_placed: bool,
}

/// One registered world-line in the fork tree (Round 497, design sec 7.21).
/// The CYOA mapping (design sec 10): `branch_id` = a reachable world (save
/// state), the `fork` = the choice point, `description` = the choice label.
/// Pure projection of the stored [`mnemosyne_core::Branch`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ForkTreeBranch {
    pub branch_id: String,
    /// The branch's free-form description — the CYOA choice label for a
    /// forked world, a plain world description for a standalone one; may be
    /// empty.
    pub description: String,
    /// Divergence coordinate (Round 438). `None` = a standalone world
    /// sharing no history (the pre-fork R433 semantics).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fork: Option<ForkTreeEdge>,
    /// Incoming world-line merges (Round 532 — confluence). Empty for a fork
    /// or standalone world; non-empty = the parents that CONVERGE into this
    /// shared continuation, each merge point resolved against the PARENT's
    /// composed order ([`CanonOrder::names`]). This is the edge a fork tree
    /// alone could never show (R531: "convergence is expressed, not
    /// declared") — the merge made structurally visible.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub converges: Vec<ForkTreeEdge>,
}

/// Fork tree over the registered world-lines (Round 497, design sec 7.21) —
/// the cross-world CHOICE GRAPH the CYOA renderer assumes. Per-world
/// manuscripts (R466) gave N linear readings; this is the tree that
/// stitches them at the fork points. Pure read projection, never gated (a
/// choice graph is a reading surface, not a defect detector).
#[derive(Debug, Clone, Default, Serialize)]
pub struct ForkTreeReport {
    /// Every registered branch, branch-id sorted (the `BTreeMap` order;
    /// `MAIN_BRANCH` is the default axis, never registered, so never listed).
    pub branches: Vec<ForkTreeBranch>,
    /// Branch ids whose fork point is not a node of the parent's composed
    /// order — surfaced, never dropped (B-1, the R466 idiom).
    pub unplaced_fork_points: Vec<String>,
    /// Registered branch count (`branches.len()`).
    pub branch_count: usize,
}

/// Project the fork tree over the registered world-lines (Round 497, design
/// sec 7.21): each registered branch's divergence coordinate (parent + fork
/// point + the choice-label description), the fork point resolved against
/// the PARENT world's composed order via [`CanonOrder::names`] (Round 488 —
/// one node-membership semantics, no parallel fork engine; the R441 binding
/// rule). Fail-loud on a fork whose parent is neither [`MAIN_BRANCH`] nor a
/// registered branch (a store-integrity violation the write path forbids —
/// a typo'd parent must not read as a silent root). This guard covers ONLY
/// the dangling-parent case; cycle and self-fork integrity are delegated
/// upstream to the order composition ([`fork_chain`] fails loud on a cyclic
/// registry before `compose_canon_order` hands this verb an order), so this
/// is not a complete registry validator. Pure read projection —
/// `store.branches` unchanged, deliberately never gated.
pub fn fork_tree(store: &AtomicStore, order: &CanonOrder) -> Result<ForkTreeReport, String> {
    check_store_boundary(store, order)?;
    let mut report = ForkTreeReport::default();
    for (branch_id, branch) in &store.branches {
        let fork = match &branch.forks_from {
            None => None,
            Some(f) => {
                if f.branch != mnemosyne_core::MAIN_BRANCH
                    && !store.branches.contains_key(&f.branch)
                {
                    return Err(format!(
                        "branch `{branch_id}` forks from `{}`, which is neither `main` nor a \
                         registered branch — fail-loud (a typo'd parent must not read as a \
                         silent root); fix the registry",
                        f.branch
                    ));
                }
                let at_placed = order.names(&f.branch, &f.at);
                if !at_placed {
                    report.unplaced_fork_points.push(branch_id.clone());
                }
                Some(ForkTreeEdge {
                    parent: f.branch.clone(),
                    at: f.at.clone(),
                    at_placed,
                })
            }
        };
        // Round 532 — the incoming-merge edges of a confluence world-line, the
        // inverse of the fork edge. Same parent-must-be-registered fail-loud as
        // the fork side; each merge coordinate resolved against the PARENT's
        // composed order. An unplaced merge point lands the branch in
        // `unplaced_fork_points` once (a fork XOR confluence, so the two never
        // both push, but several merge edges share one branch — dedup).
        let mut converges = Vec::with_capacity(branch.converges_from.len());
        let mut converge_unplaced = false;
        for edge in &branch.converges_from {
            if edge.branch != mnemosyne_core::MAIN_BRANCH
                && !store.branches.contains_key(&edge.branch)
            {
                return Err(format!(
                    "branch `{branch_id}` converges from `{}`, which is neither `main` nor a \
                     registered branch — fail-loud (a typo'd parent must not read as a silent \
                     root); fix the registry",
                    edge.branch
                ));
            }
            let at_placed = order.names(&edge.branch, &edge.at);
            converge_unplaced |= !at_placed;
            converges.push(ForkTreeEdge {
                parent: edge.branch.clone(),
                at: edge.at.clone(),
                at_placed,
            });
        }
        if converge_unplaced {
            report.unplaced_fork_points.push(branch_id.clone());
        }
        report.branches.push(ForkTreeBranch {
            branch_id: branch_id.clone(),
            description: branch.description.clone(),
            fork,
            converges,
        });
    }
    report.branch_count = report.branches.len();
    Ok(report)
}

/// One disclosure surface resolved to a stable world POINTER (Round 556/557,
/// design sec 7.37) — the `map_locator` seam a pinion narrative runtime
/// consumes. The authored [`mnemosyne_core::DisclosureSurface`] (`scene` +
/// optional `object`) RESOLVED against a world-line's manuscript walk:
/// `scene_ordinal` = the index of `scene` in this world's scene sequence (the
/// stable position pointer; `None` when the surface scene is not a node of this
/// world's walk — surfaced, never dropped, the R466 unplaced idiom). Carries NO
/// baked geometry: pinion dereferences scene -> place, object -> prop, ordinal
/// -> traversal order, mode -> how-surfaced. A PROJECTION, not stored state —
/// every field derives from the manuscript begins-event + the disclosure
/// resolver (R510), so a locator cannot drift from the coverage surface or the
/// `--telling` carrier (one resolver, no second semantics).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MapLocator {
    /// The world-line this pointer belongs to (`MAIN_BRANCH` or a branch id).
    pub world_line: String,
    /// The disclosed fact the locator carries (a `narrative_facts` key) — what
    /// pinion dereferences for content.
    pub fact_id: String,
    /// The scene the disclosure surfaces in (the authored `surface.scene`, a
    /// canon structure-section ref).
    pub scene: String,
    /// Index of `scene` in this world's manuscript walk (`scene_walk`); `None`
    /// when the surface scene is not a node of this world's walk (surfaced, not
    /// silently dropped — the R466 idiom).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene_ordinal: Option<usize>,
    /// The diegetic carrier object the disclosure rides on (the authored
    /// `surface.object`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    /// The effective disclosure mode under the telling (state/hint/imply/
    /// withhold) — pinion honors it when surfacing.
    pub mode: mnemosyne_core::DisclosureMode,
    /// The reader's first-learn coordinate for THIS world (the disclosure
    /// `first_at`; `None` when unpinned — distinct from the fact's `canon_from`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_at: Option<String>,
}

/// One world-line's playable surface (Round 556/557, design sec 7.37): the
/// world's full manuscript (the spatial skeleton the locators point INTO)
/// overlaid with the resolved disclosure [`MapLocator`]s. The manuscript is
/// reused VERBATIM — not a `scene_walk: Vec<String>` re-projection — so the R466
/// B-1 honesty surfaces ride through, never silently dropped (R558 review fix):
/// `undeclared_adjacencies` (the walk is ONE valid linearization of a partial
/// order, not the only one), `unplaced_facts`, `undecidable`,
/// `sections_outside_order`. A [`MapLocator`]'s `scene_ordinal` indexes
/// `manuscript.scenes`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct PlayableWorld {
    /// The world's manuscript (R466) reused verbatim: the ordered scene walk +
    /// the B-1 honesty surfaces. `MapLocator::scene_ordinal` indexes
    /// `manuscript.scenes`; pinion dereferences each scene's section id.
    pub manuscript: WorldManuscript,
    /// The disclosure pointers for this world, in walk order.
    pub locators: Vec<MapLocator>,
}

/// The playable-world projection for one telling (Round 556/557, design sec
/// 7.37) — the single composing READ a pinion narrative runtime consumes,
/// stitching the existing projections so a runtime need not re-derive across
/// three verbs: the cross-world choice graph ([`fork_tree`], R497) + each
/// world-line's scene walk ([`playthrough_manuscript`], R466) + the per-scene
/// disclosure [`MapLocator`]s (the R510 resolver, already on each begins-event).
/// Pure read projection, never gated (a playable surface is a reading surface,
/// not a defect detector) — it adds no traversal and no authoritative state.
#[derive(Debug, Clone, Default, Serialize)]
pub struct PlayableWorldReport {
    /// The telling whose disclosure plan resolved the locators.
    pub telling: String,
    /// The cross-world choice graph (R497) — navigation context, always full
    /// even under a `world` filter (the topology is inherently cross-world).
    pub fork_tree: ForkTreeReport,
    /// Per world-line (or the single `world` filter), the playable surface.
    pub worlds: BTreeMap<String, PlayableWorld>,
}

/// Compose the playable-world projection for `telling` (Round 556/557, design
/// sec 7.37): a PURE JOIN over [`playthrough_manuscript`] (the per-world scene
/// walk with each begins-event's resolved disclosure) and [`fork_tree`] (the
/// choice graph), reshaping every authored disclosure `surface` into a
/// [`MapLocator`] whose `scene_ordinal` is the surface scene's index in the
/// world's walk. `world` filters the per-world map (the fork tree stays full —
/// topology is inherently cross-world). No new traversal, no new state — the
/// locators derive entirely from the two existing projections, so they cannot
/// drift from them. Fails loud through the sub-projections (an unregistered
/// `world`, a typo'd `telling`).
pub fn playable_world(
    store: &AtomicStore,
    order: &CanonOrder,
    world: Option<&str>,
    telling: &str,
) -> Result<PlayableWorldReport, String> {
    let manuscript = playthrough_manuscript(store, order, world, Some(telling))?;
    let fork_tree = fork_tree(store, order)?;
    let mut worlds = BTreeMap::new();
    for (world_id, manuscript_world) in manuscript.worlds {
        // Owned-key index so the borrow ends before the manuscript moves into
        // PlayableWorld (the manuscript is reused verbatim, R558 fix).
        let ordinal: BTreeMap<String, usize> = manuscript_world
            .scenes
            .iter()
            .enumerate()
            .map(|(index, scene)| (scene.section.clone(), index))
            .collect();
        let mut locators = Vec::new();
        for scene in &manuscript_world.scenes {
            for event in &scene.begins {
                let Some(disclosure) = &event.disclosure else {
                    continue;
                };
                let Some(surface) = &disclosure.surface else {
                    continue;
                };
                locators.push(MapLocator {
                    world_line: world_id.clone(),
                    fact_id: event.fact_id.clone(),
                    scene: surface.scene.clone(),
                    scene_ordinal: ordinal.get(surface.scene.as_str()).copied(),
                    object: surface.object.clone(),
                    mode: disclosure.mode,
                    first_at: disclosure.first_at.clone(),
                });
            }
        }
        worlds.insert(
            world_id,
            PlayableWorld {
                manuscript: manuscript_world,
                locators,
            },
        );
    }
    Ok(PlayableWorldReport {
        telling: telling.to_string(),
        fork_tree,
        worlds,
    })
}

/// The R559 quest authoring-contract vocabulary (design sec 7.38). A quest is
/// an `Entity{kind:"quest"}` plus three typed predicates: `pursues` (an actor
/// LEADS a quest), `requires` (a quest is gated by another quest first), and
/// `completed_by` (an actor DISCHARGES a quest on a road — the carrying fact
/// `pays_off` the quest's giving setup). These ids ARE the contract a consumer
/// adopts to author quests this projection can read, not arbitrary magic
/// strings (the R547 authoring-contract-over-existing-primitives pattern);
/// `Entity.kind` is consumer-defined per ARCHITECTURE sec 6 inv4.
pub(crate) const QUEST_ENTITY_KIND: &str = "quest";
pub(crate) const QUEST_PRED_PURSUES: &str = "pursues";
pub(crate) const QUEST_PRED_REQUIRES: &str = "requires";
pub(crate) const QUEST_PRED_COMPLETED_BY: &str = "completed_by";

/// A quest's DERIVED state in one world-line (R559: "quest state DERIVED per
/// world-line, never stored"). Open vs done is read VERBATIM from the R442
/// payoff coverage of the quest's giving fact — paid here = done, dangling here
/// = open, neither (the giving fact is not visible in this world) = unknown
/// (B-1, surfaced not assumed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum QuestState {
    /// A giving fact of this quest is `paid` in this world (R442) — done here.
    Done,
    /// A giving fact dangles in this world (R442) and none is paid — open here.
    Open,
    /// No giving setup of this quest is visible in this world (neither paid nor
    /// dangling) — the quest does not apply on this road. The SAME verdict also
    /// covers an orphan quest (no giving setup bound at all): it reads `unknown`
    /// on EVERY road and is additionally listed in `unresolved_quests`. Read the
    /// two together to tell "not on this road" from "no payoff anchor anywhere".
    Unknown,
}

impl QuestState {
    /// Stable lowercase label (matches the serde rename), for human output.
    pub fn as_str(&self) -> &'static str {
        match self {
            QuestState::Done => "done",
            QuestState::Open => "open",
            QuestState::Unknown => "unknown",
        }
    }
}

/// One fact that discharges a quest's giving setup in a single world-line (R568)
/// — the "completion fact" the R559 `QuestNode` names. Read straight from the
/// R442 paid-setup payoff list (the payoffs crediting a giving setup that is
/// paid here), kept only when the crediting fact carries THIS quest's
/// `completed_by` claim; the `actor` is that claim's named discharger.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QuestCompletion {
    /// The fact that pays off a giving setup in this world (a `narrative_facts`
    /// key) — what pinion dereferences for the completion beat.
    pub fact: String,
    /// That fact's `canon_from` — the scene the quest completes at on this road.
    pub scene: String,
    /// The actor the fact's `completed_by` claim names as the discharger on this
    /// road, when the completing fact carries that claim for THIS quest (`None`
    /// when the payoff fact is untyped or a `completed_by` for another quest).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
}

/// A quest's state in one world-line: the derived open/done verdict plus the
/// completion fact(s) on that road (empty when open). Per-road divergence — a
/// quest done on one terminal and open on another — is exactly two different
/// `QuestWorldState`s, the R559 "derived per world-line" claim made data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QuestWorldState {
    pub state: QuestState,
    /// The completion fact(s) discharging this quest here; empty when open.
    pub completions: Vec<QuestCompletion>,
}

/// One quest in the graph (R559 design sec 7.38, R568 build): the narrative
/// instance of the substrate's universal tracked-obligation pattern, PROJECTED
/// from existing primitives — no new authoritative state. `objective`/`actors`
/// from the `Entity{kind:"quest"}` + its `pursues` claims; `prerequisites` from
/// `requires` claims; `giving_facts` are the `PayoffExpectation::Expected`
/// setups its `completed_by` facts pay off; `per_world` is the R442 open/done of
/// those givings; `locators` are the giver surfaces (R557) resolved under the
/// telling (where the quest is picked up).
#[derive(Debug, Clone, Default, Serialize)]
pub struct QuestNode {
    /// The quest entity id (an `Entity{kind:"quest"}` key).
    pub quest_id: String,
    /// The quest objective — the entity's `description`.
    pub objective: String,
    /// The actor entities that LEAD the quest (`pursues` subjects), sorted.
    pub actors: Vec<String>,
    /// Prerequisite quest ids that must complete first (`requires` objects),
    /// sorted — the declarative gate (R559); the canon order proves the timing.
    pub prerequisites: Vec<String>,
    /// The giving setups this quest opens (`PayoffExpectation::Expected` facts),
    /// sorted — the obligation that dangles while the quest is open. Bound by the
    /// quest's own `completed_by` fact's `pays_off` edge (R559 strict combined);
    /// empty = `unresolved` (no payoff anchor — no completed_by, or it pays off
    /// no Expected setup; surfaced not dropped).
    pub giving_facts: Vec<String>,
    /// Per world-line, the quest's derived state (open/done/unknown) + the
    /// completion fact(s) on that road.
    pub per_world: BTreeMap<String, QuestWorldState>,
    /// The quest-giver surface locators (R557) — a [`MapLocator`] per world where
    /// a giving fact is disclosed at a surface (where the player picks the quest
    /// up). World-then-walk order; empty when no giving fact carries a surface.
    pub locators: Vec<MapLocator>,
}

/// The quest-graph projection for one telling (R559 design sec 7.38, R568 build)
/// — the single composing READ a pinion narrative runtime (or an authoring
/// consumer) needs for the quest layer, the sibling of [`playable_world`]. A
/// PURE JOIN over the existing projections (R558 verbatim reuse, no
/// re-projection): the `Entity{kind:"quest"}` entities + their typed claims, the
/// R442 [`payoff_coverage`] (per-world open/done), and [`playable_world`] (the
/// R497 fork topology + the R557 giver-surface locators). Never gated — a quest
/// graph is a reading surface, not a defect detector; quest STATE is DERIVED per
/// world-line, never stored (R559). Executable quest LOGIC (the runtime
/// lifecycle available/active/done/failed, completion guards, the state machine)
/// is SCE/pinion's, NOT modeled here (the R546/R559 declarative-vs-executable
/// line). Fails loud through the sub-projections (a typo'd telling / world).
#[derive(Debug, Clone, Default, Serialize)]
pub struct QuestGraphReport {
    /// The telling whose disclosure plan resolved the giver locators.
    pub telling: String,
    /// The cross-world choice graph (R497) — navigation context, always full
    /// even under a `world` filter (the topology is inherently cross-world).
    pub fork_tree: ForkTreeReport,
    /// The world-lines covered (every query world, or the single `world`
    /// filter), sorted — the per-world key set every `QuestNode.per_world` uses.
    pub worlds: Vec<String>,
    /// One node per `Entity{kind:"quest"}`, sorted by quest id.
    pub quests: Vec<QuestNode>,
    /// Quest entities whose giving setup could not be bound — no `completed_by`
    /// fact, or its `completed_by` facts pay off no `Expected` setup (R559 strict
    /// combined). The obligation has no payoff anchor (surfaced, not silently
    /// dropped — the R558 lesson). Each still appears in `quests` with empty
    /// `giving_facts` and an all-`unknown` `per_world`.
    pub unresolved_quests: Vec<String>,
}

/// Compose the quest-graph projection for `telling` (R559 design sec 7.38, R568
/// build; R569 strict-combined binding). A PURE JOIN owning the R562 hand-JOIN
/// (quest entities + typed pursues/requires/completed_by × R442 payoff coverage
/// per world × the R557 playable-world surface locators). Reuses
/// [`playable_world`] and [`payoff_coverage`] VERBATIM (R558): the open/done
/// verdicts AND the completion beats are read straight from the R442 paid list —
/// no second visibility pass, nothing re-derived. `world` filters the per-world
/// map (the fork tree stays full — topology is inherently cross-world). No new
/// traversal, no authoritative state — quest state is DERIVED per world-line
/// (R559). Fails loud through the sub-projections.
pub fn quest_graph(
    store: &AtomicStore,
    order: &CanonOrder,
    world: Option<&str>,
    telling: &str,
) -> Result<QuestGraphReport, String> {
    // Reuse the existing projections VERBATIM (R558): playable-world gives the
    // fork topology + per-world giver-surface locators; payoff coverage gives
    // the per-world open/done of every giving setup (R442). No re-derivation.
    let playable = playable_world(store, order, world, telling)?;
    let payoff = payoff_coverage(store, order)?;

    // The reported world set = playable-world's worlds (respects `world`); the
    // fork tree stays full (cross-world topology).
    let worlds: Vec<String> = playable.worlds.keys().cloned().collect();

    let facts = &store.narrative_facts;

    // Index the quest typed claims once (all keyed by quest id). A completion
    // fact is the `completed_by` fact (it names the per-road discharger); a
    // quest's giving SETUP is the `Expected` fact paid off AT a completion scene
    // — the `completed_by` fact's own `pays_off` edge (the contract's intended
    // encoding), or a sibling fact's `pays_off` at the same scene when the
    // author split completion across two facts (a typed `completed_by` plus a
    // separate fact carrying the `pays_off`, as the R562 base did for the main
    // quest). The substrate has NO hard typed "giving fact of quest Q" edge;
    // this is the same inference the R562 hand-JOIN made (an as-built finding,
    // not a hard binding — a completion scene shared by two quests would share
    // givings).
    // (fact id, completing fact, named actor) — one entry per completed_by claim.
    type CompletionEntry<'a> = (&'a str, &'a NarrativeFact, Option<String>);
    let mut actors: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    let mut prereqs: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    // Per quest, its `completed_by` facts.
    let mut completions_of: BTreeMap<&str, Vec<CompletionEntry<'_>>> = BTreeMap::new();
    for (fid, fact) in facts {
        let Some(claim) = &fact.typed else { continue };
        match claim.predicate.as_str() {
            QUEST_PRED_PURSUES => {
                // subject LEADS the object quest.
                if let mnemosyne_core::TypedObject::Entity { id } = &claim.object {
                    actors
                        .entry(id.as_str())
                        .or_default()
                        .insert(claim.subject.clone());
                }
            }
            QUEST_PRED_REQUIRES => {
                // subject quest REQUIRES the object quest first.
                if let mnemosyne_core::TypedObject::Entity { id } = &claim.object {
                    prereqs
                        .entry(claim.subject.as_str())
                        .or_default()
                        .insert(id.clone());
                }
            }
            QUEST_PRED_COMPLETED_BY => {
                // subject quest is discharged by the object actor at this fact.
                let actor = match &claim.object {
                    mnemosyne_core::TypedObject::Entity { id } => Some(id.clone()),
                    mnemosyne_core::TypedObject::Value { value } => Some(value.clone()),
                };
                completions_of
                    .entry(claim.subject.as_str())
                    .or_default()
                    .push((fid.as_str(), fact, actor));
            }
            _ => {}
        }
    }
    // The `Expected` setups. A quest's giving setup is an Expected fact its OWN
    // `completed_by` fact pays off (R559's single contract encoding — the
    // completion fact pays off the giving). No scene-proximity bridge: binding
    // by scene co-location would let two quests completing at one scene share
    // givings (a cross-quest bleed), so it is not done — strict-combined only.
    let expected: BTreeSet<&str> = facts
        .iter()
        .filter(|(_, f)| f.payoff_expectation == mnemosyne_core::PayoffExpectation::Expected)
        .map(|(id, _)| id.as_str())
        .collect();

    let mut quests: Vec<QuestNode> = Vec::new();
    let mut unresolved_quests: Vec<String> = Vec::new();
    for (quest_id, entity) in &store.entities {
        if entity.kind != QUEST_ENTITY_KIND {
            continue;
        }
        let empty_completions = Vec::new();
        let q_completions = completions_of
            .get(quest_id.as_str())
            .unwrap_or(&empty_completions);
        // R559 strict combined binding: the giving setups are the `Expected`
        // facts this quest's OWN `completed_by` facts pay off. An author who
        // splits completion from payoff (a typed `completed_by` with no
        // `pays_off`, the giving edge on a sibling fact) gets an honest
        // `unresolved`, NEVER a scene-proximity-inferred binding.
        let mut q_givings: BTreeSet<String> = BTreeSet::new();
        for (_, fact, _) in q_completions {
            for target in &fact.pays_off {
                if expected.contains(target.as_str()) {
                    q_givings.insert(target.clone());
                }
            }
        }
        // No giving setup bound = the obligation has no payoff anchor (no
        // `completed_by` fact, or none pays off an Expected setup) — surfaced,
        // not silently dropped (R558). Such a quest reads `unknown` everywhere.
        if q_givings.is_empty() {
            unresolved_quests.push(quest_id.clone());
        }
        // This quest's completed_by facts by id → (scene, discharger): used to
        // credit a paid giving's R442 payoff list back to the named discharger.
        let discharger: BTreeMap<&str, (&str, Option<&str>)> = q_completions
            .iter()
            .map(|(fid, fact, actor)| (*fid, (fact.canon_from.as_str(), actor.as_deref())))
            .collect();
        let per_world: BTreeMap<String, QuestWorldState> = worlds
            .iter()
            .map(|w| {
                // R442 payoff coverage is the SINGLE authority for open/done
                // (reused verbatim, not re-derived): a giving setup PAID here =
                // done, DANGLING here = open, neither (not visible on this road)
                // = unknown. The completion beats are the giving's crediting
                // payoffs that carry THIS quest's `completed_by` claim — read
                // straight from the R442 paid list, no second visibility pass.
                let cov = payoff.worlds.get(w);
                let mut completions: Vec<QuestCompletion> = Vec::new();
                let mut paid_here = false;
                let mut dangling_here = false;
                if let Some(c) = cov {
                    for g in &q_givings {
                        if let Some(ps) = c.paid.iter().find(|p| &p.setup == g) {
                            paid_here = true;
                            for payoff_fact in &ps.payoffs {
                                if let Some((scene, actor)) = discharger.get(payoff_fact.as_str()) {
                                    completions.push(QuestCompletion {
                                        fact: payoff_fact.clone(),
                                        scene: (*scene).to_string(),
                                        actor: actor.map(str::to_string),
                                    });
                                }
                            }
                        }
                        if c.dangling.iter().any(|d| d == g) {
                            dangling_here = true;
                        }
                    }
                }
                completions.sort_by(|a, b| a.fact.cmp(&b.fact));
                completions.dedup();
                let state = if paid_here {
                    QuestState::Done
                } else if dangling_here {
                    QuestState::Open
                } else {
                    QuestState::Unknown
                };
                (w.clone(), QuestWorldState { state, completions })
            })
            .collect();
        // Giver-surface locators: the playable-world locators (R557, reused
        // verbatim) whose disclosed fact is one of this quest's givings, in
        // world-then-walk order.
        let mut locators: Vec<MapLocator> = Vec::new();
        for w in &worlds {
            if let Some(pw) = playable.worlds.get(w) {
                for loc in &pw.locators {
                    if q_givings.contains(&loc.fact_id) {
                        locators.push(loc.clone());
                    }
                }
            }
        }
        quests.push(QuestNode {
            quest_id: quest_id.clone(),
            objective: entity.description.clone(),
            actors: actors
                .get(quest_id.as_str())
                .map(|s| s.iter().cloned().collect())
                .unwrap_or_default(),
            prerequisites: prereqs
                .get(quest_id.as_str())
                .map(|s| s.iter().cloned().collect())
                .unwrap_or_default(),
            giving_facts: q_givings.iter().cloned().collect(),
            per_world,
            locators,
        });
    }

    Ok(QuestGraphReport {
        telling: telling.to_string(),
        fork_tree: playable.fork_tree,
        worlds,
        quests,
        unresolved_quests,
    })
}

/// One untyped fact awaiting a typed-leg proposal (Round 458, design sec
/// 7.15 Round A): everything the proposer needs about THIS fact, including
/// the claim text and its sha256 — the R439 judgment-time pin the eventual
/// proposal must stamp (import re-checks it, so a fact amended after
/// proposing fails loud as stale).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TypingCandidate {
    pub fact_id: String,
    pub frame: String,
    pub branch: String,
    pub claim: String,
    pub claim_sha256: String,
    pub canon_from: String,
    pub entities: Vec<String>,
}

/// The typing-discovery input package (Round 458, design sec 7.15): every
/// untyped fact plus the registered vocabulary, in ONE deterministic call —
/// the proposer (an LLM agent outside the substrate) never assembles its
/// own context from N queries and never sees unregistered vocabulary as
/// proposable. Pure read projection; the substrate contains no LLM client.
#[derive(Debug, Clone, Default, Serialize)]
pub struct TypingCandidatesReport {
    /// Untyped facts, id-sorted.
    pub candidates: Vec<TypingCandidate>,
    pub facts: usize,
    /// Already-typed count (context, not work).
    pub typed: usize,
    /// The 4th registry verbatim — the ONLY predicates a proposal may name.
    pub predicates: BTreeMap<String, mnemosyne_core::Predicate>,
    /// The entity registry verbatim — typed subjects/objects must be
    /// registered AND members of the fact's entities list (R446).
    pub entities: BTreeMap<String, mnemosyne_core::Entity>,
}

/// Collect typing candidates (Round 458). Order-independent by design —
/// typing is a property of the fact, not of any canon declaration — so the
/// store boundary runs with the empty order (its declaration-side checks
/// are vacuous; the fact-side out-of-band re-checks still apply, the R440
/// doctrine).
pub fn typing_candidates(store: &AtomicStore) -> Result<TypingCandidatesReport, String> {
    check_store_boundary(store, &CanonOrder::empty())?;
    let facts = &store.narrative_facts;
    let candidates: Vec<TypingCandidate> = facts
        .iter()
        .filter(|(_, f)| f.typed.is_none())
        .map(|(id, f)| TypingCandidate {
            fact_id: id.clone(),
            frame: f.frame.clone(),
            branch: f.branch.clone(),
            claim: f.claim.clone(),
            claim_sha256: claim_sha256_hex(&f.claim),
            canon_from: f.canon_from.clone(),
            entities: f.entities.clone(),
        })
        .collect();
    Ok(TypingCandidatesReport {
        facts: facts.len(),
        typed: facts.len() - candidates.len(),
        candidates,
        predicates: store.predicates.clone(),
        entities: store.entities.clone(),
    })
}

/// One fact row of the edge-discovery input package (Round 462, design sec
/// 7.16 Round A): the claim text with its sha256 (the R439 judgment-time
/// pin, TWO-SIDED for edges — a proposal stamps both endpoints) and every
/// recorded edge, so the proposer never re-proposes existing structure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EdgeCandidateFact {
    pub fact_id: String,
    pub frame: String,
    pub branch: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub entities: Vec<String>,
    pub claim: String,
    pub claim_sha256: String,
    pub canon_from: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canon_to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub typed: Option<mnemosyne_core::TypedClaim>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supersedes_in_frame: Option<String>,
    /// Recorded conflict TARGETS (identity only — staleness of the stored
    /// pins is the scan's territory, not the proposer's).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub conflicts_with: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub pays_off: Vec<String>,
}

/// One deterministic succession-gap hint (Round 462, design sec 7.16): a
/// same-frame pair with the same typed `(predicate, subject)`, co-visible
/// in some world, that no succession PATH connects either way — the
/// rule-free generalization of the `unchained_state_pairs` count, surfaced
/// as PAIRS because the proposer needs the candidates, not a number.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SuccessionGap {
    pub fact_a: String,
    pub fact_b: String,
    pub predicate: String,
    pub subject: String,
}

/// The edge-discovery input package (Round 462, design sec 7.16 Round A):
/// every fact row (claims + pins + all recorded edges) plus the
/// deterministic succession-gap hints, in ONE call. Typed-only hints by
/// construction — untyped facts carry no machine-comparable state key;
/// their candidate surface is the facts table itself (the LLM's reading
/// job). Pure read projection; the substrate contains no LLM client.
#[derive(Debug, Clone, Default, Serialize)]
pub struct EdgeCandidatesReport {
    /// Every fact, id-sorted.
    pub facts: Vec<EdgeCandidateFact>,
    pub fact_count: usize,
    /// Recorded succession edges (context, not work).
    pub succession_edges: usize,
    /// Distinct recorded conflict pairs (edges read symmetrically).
    pub conflict_pairs: usize,
    pub succession_gaps: Vec<SuccessionGap>,
}

/// Collect edge candidates (Round 462). Order-resolved like every
/// narrative read: without a declared canon order the gap hints degrade
/// honestly (fork visibility goes unknown, pairs skip) while the facts
/// table stays complete.
pub fn edge_candidates(
    store: &AtomicStore,
    order: &CanonOrder,
) -> Result<EdgeCandidatesReport, String> {
    check_store_boundary(store, order)?;
    let facts = &store.narrative_facts;
    let successors = successors_index(facts);
    let lineages = query_world_lineages(store)?;
    let worlds = query_worlds(store);
    let typed: Vec<(&String, &NarrativeFact)> =
        facts.iter().filter(|(_, f)| f.typed.is_some()).collect();
    let ancestors: BTreeMap<&str, BTreeSet<&str>> = typed
        .iter()
        .map(|(id, _)| (id.as_str(), succession_ancestors(facts, id)))
        .collect();
    // Same-(predicate, subject) pairs no succession path connects — the
    // scan's unchained computation (path not edge, Round 452) swept over
    // ALL typed facts instead of one rule's predicate, deduplicated
    // across worlds exactly like the count.
    let mut gaps: BTreeSet<(&str, &str)> = BTreeSet::new();
    for_each_world_pair(
        &worlds,
        &lineages,
        order,
        &successors,
        &typed,
        |_, aid, a, bid, b| {
            let (ta, tb) = (a.typed.as_ref().unwrap(), b.typed.as_ref().unwrap());
            if ta.predicate != tb.predicate || ta.subject != tb.subject {
                return;
            }
            if ancestors[aid].contains(bid) || ancestors[bid].contains(aid) {
                return; // connected through the succession chain
            }
            gaps.insert((aid, bid));
        },
    );
    let succession_gaps: Vec<SuccessionGap> = gaps
        .into_iter()
        .map(|(aid, bid)| {
            let t = facts[aid].typed.as_ref().unwrap();
            SuccessionGap {
                fact_a: aid.to_string(),
                fact_b: bid.to_string(),
                predicate: t.predicate.clone(),
                subject: t.subject.clone(),
            }
        })
        .collect();
    let mut conflict_pairs: BTreeSet<(&str, &str)> = BTreeSet::new();
    for (aid, a) in facts {
        for c in &a.conflicts_with {
            let key = if aid.as_str() < c.target.as_str() {
                (aid.as_str(), c.target.as_str())
            } else {
                (c.target.as_str(), aid.as_str())
            };
            conflict_pairs.insert(key);
        }
    }
    let rows: Vec<EdgeCandidateFact> = facts
        .iter()
        .map(|(id, f)| EdgeCandidateFact {
            fact_id: id.clone(),
            frame: f.frame.clone(),
            branch: f.branch.clone(),
            entities: f.entities.clone(),
            claim: f.claim.clone(),
            claim_sha256: claim_sha256_hex(&f.claim),
            canon_from: f.canon_from.clone(),
            canon_to: f.canon_to.clone(),
            typed: f.typed.clone(),
            supersedes_in_frame: f.supersedes_in_frame.clone(),
            conflicts_with: f.conflicts_with.iter().map(|c| c.target.clone()).collect(),
            pays_off: f.pays_off.clone(),
        })
        .collect();
    Ok(EdgeCandidatesReport {
        fact_count: rows.len(),
        succession_edges: facts
            .values()
            .filter(|f| f.supersedes_in_frame.is_some())
            .count(),
        conflict_pairs: conflict_pairs.len(),
        facts: rows,
        succession_gaps,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mnemosyne_atomic::{AtomicSection, FactImport, FactsManifest};
    use mnemosyne_core::MAIN_BRANCH;

    /// Round 592 — the single-sourced per-class gate policy (the finding that
    /// propose-verdict diverged from validate-continuity). Structural violations
    /// ride `severity`; interval violations ride `interval_severity` (OFF by
    /// default); a class gates only at `reject`; `None` = the class is disabled.
    #[test]
    fn evaluate_continuity_gate_respects_per_class_severity() {
        let structural = ContinuityViolation::FactCanonOffBranch {
            fact: "f".into(),
            branch: "main".into(),
            coord: "sc".into(),
        };
        let interval = ContinuityViolation::RuleIntervalViolation {
            rule: "r".into(),
            predicate: "p".into(),
            right: "q".into(),
            op: ">=".into(),
            frame: "gt".into(),
            branch: "main".into(),
            subject: "s".into(),
            left_fact: "a".into(),
            right_fact: "b".into(),
            left_value: "1".into(),
            right_value: "2".into(),
            bound: "5".into(),
            at: "sc".into(),
        };
        let g = |sev, isev, v: &[ContinuityViolation]| evaluate_continuity_gate(sev, isev, v);
        let one = std::slice::from_ref::<ContinuityViolation>;

        // Structural rides `severity`.
        assert!(g(Some(Severity::Reject), None, one(&structural)).gates);
        assert!(!g(Some(Severity::Warn), None, one(&structural)).gates);
        // Gate disabled ([continuity] absent) never gates.
        assert!(!g(None, None, one(&structural)).gates);

        // Interval rides `interval_severity` — OFF by default, so a reject-level
        // `severity` must NOT gate an interval time-bend (the R592 fix).
        assert!(!g(Some(Severity::Reject), None, one(&interval)).gates);
        assert!(
            g(
                Some(Severity::Reject),
                Some(Severity::Reject),
                one(&interval)
            )
            .gates
        );

        // Counts split by class; structural presence gates the mixed set.
        let mixed = g(Some(Severity::Reject), None, &[structural, interval]);
        assert_eq!(mixed.structural_count, 1);
        assert_eq!(mixed.interval_count, 1);
        assert!(mixed.gates);
    }

    fn chain(ids: &[&str]) -> CanonOrder {
        let edges: Vec<[String; 2]> = ids
            .windows(2)
            .map(|w| [w[0].to_string(), w[1].to_string()])
            .collect();
        CanonOrder::from_edges(&edges).unwrap()
    }

    fn fact(id: &str, frame: &str, from: &str, to: Option<&str>) -> FactImport {
        FactImport {
            entities: vec![],
            fact_id: id.to_string(),
            frame: frame.to_string(),
            branch: None,
            claim: format!("claim {id}"),
            canon_from: from.to_string(),
            canon_to: to.map(str::to_string),
            evidence: vec![from.to_string()],
            conflicts_with: vec![],
            supersedes_in_frame: None,
            payoff_expectation: None,
            pays_off: vec![],
            typed: None,
            quote: None,
        }
    }

    /// Entity + predicate imports auto-derived from the facts (Round 449
    /// test convenience): every referenced entity id registers, every typed
    /// leg's predicate registers with the object_kind its object shape
    /// implies — the production write path then enforces the same
    /// invariants it always does.
    fn derived_registries(
        facts: &[FactImport],
    ) -> (
        Vec<mnemosyne_atomic::EntityImport>,
        Vec<mnemosyne_atomic::PredicateImport>,
    ) {
        let entities = facts
            .iter()
            .flat_map(|f| f.entities.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|entity_id| mnemosyne_atomic::EntityImport {
                entity_id,
                kind: String::new(),
                description: String::new(),
            })
            .collect();
        let predicates = facts
            .iter()
            .filter_map(|f| f.typed.as_ref())
            .map(|t| {
                (
                    t.predicate.clone(),
                    match t.object {
                        mnemosyne_core::TypedObject::Entity { .. } => "entity",
                        mnemosyne_core::TypedObject::Value { .. } => "scalar",
                    },
                )
            })
            .collect::<BTreeMap<_, _>>()
            .into_iter()
            .map(
                |(predicate_id, object_kind)| mnemosyne_atomic::PredicateImport {
                    predicate_id,
                    object_kind: object_kind.to_string(),
                    description: String::new(),
                },
            )
            .collect();
        (entities, predicates)
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
        let branches = facts
            .iter()
            .filter_map(|f| f.branch.clone())
            .filter(|b| b != MAIN_BRANCH)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|branch_id| mnemosyne_atomic::BranchImport {
                branch_id,
                description: String::new(),
                forks_from: None,
                forks_at: None,
                converges_from: vec![],
            })
            .collect();
        let (entities, predicates) = derived_registries(&facts);
        mnemosyne_atomic::import_facts(
            &mut store,
            &path,
            &FactsManifest {
                disclosure_plans: vec![],
                entities,
                frames,
                branches,
                predicates,
                facts,
            },
        )
        .unwrap();
        store
    }

    #[test]
    fn same_frame_overlapping_conflict_is_a_violation() {
        let mut a = fact("fa", "seward", "ch-1", Some("ch-3"));
        let b = fact("fb", "seward", "ch-2", None);
        a.conflicts_with = vec!["fb".to_string()];
        let store = store_with(vec![a, b]);
        let report =
            scan_continuity(&store, &chain(&["ch-1", "ch-2", "ch-3", "ch-4"]), &[]).unwrap();
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
        let report =
            scan_continuity(&store, &chain(&["ch-1", "ch-2", "ch-3", "ch-4"]), &[]).unwrap();
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
        let report =
            scan_continuity(&store, &chain(&["ch-1", "ch-2", "ch-3", "ch-4"]), &[]).unwrap();
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
        let order = CanonOrder::from_declaration(&decl, &BTreeMap::new()).unwrap();
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
        let report = scan_continuity(&store, &order, &[]).unwrap();
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
        let report =
            scan_continuity(&store, &chain(&["ch-1", "ch-2", "ch-3", "ch-4"]), &[]).unwrap();
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
        let report =
            scan_continuity(&store, &chain(&["ch-1", "ch-2", "ch-3", "ch-4"]), &[]).unwrap();
        assert_eq!(report.violations.len(), 1);
    }

    #[test]
    fn succession_contradiction_stored_to_outlives_successor() {
        let old = fact("f-old", "jonathan", "ch-1", Some("ch-3"));
        let mut new = fact("f-new", "jonathan", "ch-2", None);
        new.supersedes_in_frame = Some("f-old".to_string());
        let store = store_with(vec![old, new]);
        let report =
            scan_continuity(&store, &chain(&["ch-1", "ch-2", "ch-3", "ch-4"]), &[]).unwrap();
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
        let report = scan_continuity(&store, &CanonOrder::empty(), &[]).unwrap();
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
        let report = scan_continuity(&store, &CanonOrder::empty(), &[]).unwrap();
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
        let report = scan_continuity(&store, &order, &[]).unwrap();
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
        let err = scan_continuity(&store, &chain(&["ch-1", "ch-99"]), &[]).unwrap_err();
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
        let at2 = frame_view(&store, &order, "jonathan", MAIN_BRANCH, None, "ch-2").unwrap();
        assert_eq!(
            at2.holding
                .iter()
                .map(|e| e.fact_id.as_str())
                .collect::<Vec<_>>(),
            vec!["f-old"]
        );
        assert_eq!(at2.not_holding, 1);
        let at3 = frame_view(&store, &order, "jonathan", MAIN_BRANCH, None, "ch-3").unwrap();
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
        let at3 = frame_view(&store, &order, "seward", MAIN_BRANCH, None, "ch-3").unwrap();
        assert!(at3.holding.is_empty());
        assert_eq!(at3.not_holding, 1);
        // jonathan's fact never appears in seward's view.
        let at1 = frame_view(&store, &order, "seward", MAIN_BRANCH, None, "ch-1").unwrap();
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
        let view = frame_view(&store, &order, "seward", MAIN_BRANCH, None, "ch-3").unwrap();
        assert!(view.holding.is_empty());
        assert_eq!(view.unknown, vec!["f-arm".to_string()]);
        assert_eq!(view.not_holding, 0);
    }

    #[test]
    fn frame_view_fail_loud_boundaries() {
        let store = store_with(vec![fact("f1", "seward", "ch-1", None)]);
        let order = chain(&["ch-1", "ch-2"]);
        let err = frame_view(&store, &order, "nobody", MAIN_BRANCH, None, "ch-1").unwrap_err();
        assert!(err.contains("frames registry"), "{err}");
        let err = frame_view(&store, &order, "seward", MAIN_BRANCH, None, "ch-99").unwrap_err();
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
        let main_view = frame_view(&store, &order, "jonathan", MAIN_BRANCH, None, "ch-2").unwrap();
        assert_eq!(main_view.holding.len(), 1);
        assert_eq!(main_view.holding[0].fact_id, "f-main");
        assert_eq!(main_view.branch, MAIN_BRANCH);
        let route_view = frame_view(&store, &order, "jonathan", "sea-route", None, "ch-2").unwrap();
        assert_eq!(route_view.holding.len(), 1);
        assert_eq!(route_view.holding[0].fact_id, "f-route");
        // Unknown branch fails loud — a typo must not read as an empty world.
        let err = frame_view(&store, &order, "jonathan", "sea-rotue", None, "ch-2").unwrap_err();
        assert!(err.contains("branch registry"), "{err}");
    }

    #[test]
    fn order_declared_branch_must_be_registered() {
        // Round 436: the declaration is a consumer artifact — an edge set
        // for an unregistered branch is a typo, surfaced loud by gate AND
        // view (shared boundary), never a silently inert order.
        let decl = CanonOrderFile {
            edges: vec![],
            branches: BTreeMap::from([(
                "sea-rotue".to_string(),
                vec![["ch-1".to_string(), "ch-2".to_string()]],
            )]),
        };
        let order = CanonOrder::from_declaration(&decl, &BTreeMap::new()).unwrap();
        let store = store_with(vec![fact("f1", "seward", "ch-1", None)]);
        let err = scan_continuity(&store, &order, &[]).unwrap_err();
        assert!(err.contains("sea-rotue"), "{err}");
        let err = frame_view(&store, &order, "seward", MAIN_BRANCH, None, "ch-1").unwrap_err();
        assert!(err.contains("sea-rotue"), "{err}");
    }

    #[test]
    fn order_cannot_redeclare_the_default_branch() {
        // The base edges ARE the default world-line's order — one way to
        // say it.
        let decl = CanonOrderFile {
            edges: vec![],
            branches: BTreeMap::from([(
                MAIN_BRANCH.to_string(),
                vec![["ch-1".to_string(), "ch-2".to_string()]],
            )]),
        };
        let err = CanonOrder::from_declaration(&decl, &BTreeMap::new()).unwrap_err();
        assert!(err.contains("default world-line"), "{err}");
    }

    #[test]
    fn frame_view_entity_filter_scopes_the_dossier() {
        // Round 437: frame × branch × entity at T — the NPC-context query.
        let mut about_lucy = fact("f-lucy", "seward", "ch-1", None);
        about_lucy.entities = vec!["lucy".to_string()];
        let other = fact("f-other", "seward", "ch-1", None);
        let store = {
            let tmp = tempfile::TempDir::new().unwrap();
            let path = tmp.path().join("s.json");
            let mut st = AtomicStore::new();
            for ch in ["ch-1", "ch-2"] {
                st.sections.insert(ch.to_string(), AtomicSection::default());
            }
            mnemosyne_atomic::import_facts(
                &mut st,
                &path,
                &FactsManifest {
                    disclosure_plans: vec![],
                    frames: vec![mnemosyne_atomic::FrameImport {
                        frame_id: "seward".to_string(),
                        description: String::new(),
                    }],
                    branches: vec![],
                    entities: vec![mnemosyne_atomic::EntityImport {
                        entity_id: "lucy".to_string(),
                        kind: "character".to_string(),
                        description: String::new(),
                    }],
                    predicates: vec![],
                    facts: vec![about_lucy, other],
                },
            )
            .unwrap();
            st
        };
        let order = chain(&["ch-1", "ch-2"]);
        let all = frame_view(&store, &order, "seward", MAIN_BRANCH, None, "ch-2").unwrap();
        assert_eq!(all.holding.len(), 2);
        let filtered =
            frame_view(&store, &order, "seward", MAIN_BRANCH, Some("lucy"), "ch-2").unwrap();
        assert_eq!(filtered.holding.len(), 1);
        assert_eq!(filtered.holding[0].fact_id, "f-lucy");
        assert_eq!(filtered.holding[0].entities, vec!["lucy".to_string()]);
        assert_eq!(filtered.entity.as_deref(), Some("lucy"));
        // Typo'd entity fails loud, never an empty dossier.
        let err =
            frame_view(&store, &order, "seward", MAIN_BRANCH, Some("lucyy"), "ch-2").unwrap_err();
        assert!(err.contains("entity registry"), "{err}");
    }

    /// Round 438 fixture: a store with chapters, frames/branches/entities
    /// derived from the facts PLUS explicit fork declarations.
    fn store_with_forks(
        facts: Vec<FactImport>,
        forks: &[(&str, &str, &str)], // (branch, parent, at)
    ) -> AtomicStore {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("s.json");
        let mut store = AtomicStore::new();
        // ch-* = the main discourse chain; k-* = a parallel (incomparable)
        // chain for the Round 447 cross-chain fixtures.
        for ch in ["ch-1", "ch-2", "ch-3", "ch-4", "k-1", "k-2"] {
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
        let mut branch_ids: BTreeSet<String> = facts
            .iter()
            .filter_map(|f| f.branch.clone())
            .filter(|b| b != MAIN_BRANCH)
            .collect();
        for (b, parent, _) in forks {
            branch_ids.insert(b.to_string());
            if *parent != MAIN_BRANCH {
                branch_ids.insert(parent.to_string());
            }
        }
        // Parents-first: standalone branches, then forks in declaration
        // order (the registry requires parents to pre-exist).
        let mut ordered: Vec<String> = branch_ids
            .iter()
            .filter(|b| !forks.iter().any(|(f, _, _)| f == *b))
            .cloned()
            .collect();
        ordered.extend(
            forks
                .iter()
                .filter(|(b, _, _)| branch_ids.contains(*b))
                .map(|(b, _, _)| b.to_string()),
        );
        let branches = ordered
            .into_iter()
            .map(|branch_id| {
                let fork = forks.iter().find(|(b, _, _)| *b == branch_id);
                mnemosyne_atomic::BranchImport {
                    branch_id,
                    description: String::new(),
                    forks_from: fork.map(|(_, p, _)| p.to_string()),
                    forks_at: fork.map(|(_, _, a)| a.to_string()),
                    converges_from: vec![],
                }
            })
            .collect();
        let (entities, predicates) = derived_registries(&facts);
        mnemosyne_atomic::import_facts(
            &mut store,
            &path,
            &FactsManifest {
                disclosure_plans: vec![],
                frames,
                branches,
                entities,
                predicates,
                facts,
            },
        )
        .unwrap();
        store
    }

    fn branch_fact(id: &str, frame: &str, branch: &str, from: &str) -> FactImport {
        FactImport {
            branch: Some(branch.to_string()),
            ..fact(id, frame, from, None)
        }
    }

    #[test]
    fn fork_inherits_pre_fork_facts_but_not_later_main_facts() {
        // Round 438: route forks from main at ch-2. A main fact from ch-1
        // is part of route's world; a main fact from ch-3 (post-fork) is
        // not — main continued without route.
        let early = fact("f-early", "gt", "ch-1", None);
        let late = fact("f-late", "gt", "ch-3", None);
        let store = store_with_forks(vec![early, late], &[("route", MAIN_BRANCH, "ch-2")]);
        let order = chain(&["ch-1", "ch-2", "ch-3", "ch-4"]);
        let view = frame_view(&store, &order, "gt", "route", None, "ch-3").unwrap();
        let held: Vec<&str> = view.holding.iter().map(|e| e.fact_id.as_str()).collect();
        assert_eq!(held, vec!["f-early"], "unknown={:?}", view.unknown);
        // Main's own view still sees both.
        let main_view = frame_view(&store, &order, "gt", MAIN_BRANCH, None, "ch-3").unwrap();
        assert_eq!(main_view.holding.len(), 2);
    }

    #[test]
    fn fork_conflict_with_inherited_fact_gates_in_the_join_world() {
        // A route fact contradicting an inherited (pre-fork) main fact IS a
        // violation — same frame, one world by ancestry; the report names
        // the join world. Sibling routes never share a world = data.
        let inherited = fact("f-main", "gt", "ch-1", None);
        let mut on_route = branch_fact("f-route", "gt", "route", "ch-3");
        on_route.conflicts_with = vec!["f-main".to_string()];
        let store = store_with_forks(vec![inherited, on_route], &[("route", MAIN_BRANCH, "ch-2")]);
        let order = chain(&["ch-1", "ch-2", "ch-3", "ch-4"]);
        let report = scan_continuity(&store, &order, &[]).unwrap();
        assert_eq!(report.violations.len(), 1, "{:?}", report.violations);
        match &report.violations[0] {
            ContinuityViolation::FrameConflictOverlap { branch, .. } => {
                assert_eq!(branch, "route");
            }
            v => panic!("wrong violation: {v:?}"),
        }
        // Siblings: same shape across two forks of main = cross-scope data.
        let mut on_a = branch_fact("f-a", "gt", "route-a", "ch-3");
        on_a.conflicts_with = vec!["f-b".to_string()];
        let on_b = branch_fact("f-b", "gt", "route-b", "ch-3");
        let store = store_with_forks(
            vec![on_a, on_b],
            &[
                ("route-a", MAIN_BRANCH, "ch-2"),
                ("route-b", MAIN_BRANCH, "ch-2"),
            ],
        );
        let report = scan_continuity(&store, &order, &[]).unwrap();
        assert!(report.violations.is_empty(), "{:?}", report.violations);
        assert_eq!(report.cross_scope_pairs, 1);
    }

    #[test]
    fn fork_succession_revises_inherited_belief_without_leaking_back() {
        // The fork may supersede an inherited belief (in-world change inside
        // ONE world-line); the ancestor's own view never sees the revision.
        let old = fact("f-old", "jonathan", "ch-1", None);
        let mut new = branch_fact("f-new", "jonathan", "route", "ch-3");
        new.supersedes_in_frame = Some("f-old".to_string());
        let store = store_with_forks(vec![old, new], &[("route", MAIN_BRANCH, "ch-2")]);
        let order = chain(&["ch-1", "ch-2", "ch-3", "ch-4"]);
        // No SuccessionCrossBranch: the predecessor is on the lineage.
        let report = scan_continuity(&store, &order, &[]).unwrap();
        assert!(report.violations.is_empty(), "{:?}", report.violations);
        // Route at ch-3: revised belief holds, inherited one derived-closed.
        let route = frame_view(&store, &order, "jonathan", "route", None, "ch-3").unwrap();
        let held: Vec<&str> = route.holding.iter().map(|e| e.fact_id.as_str()).collect();
        assert_eq!(held, vec!["f-new"]);
        // Main at ch-3: the original belief STILL holds — no leak-back.
        let main_view = frame_view(&store, &order, "jonathan", MAIN_BRANCH, None, "ch-3").unwrap();
        let held: Vec<&str> = main_view
            .holding
            .iter()
            .map(|e| e.fact_id.as_str())
            .collect();
        assert_eq!(held, vec!["f-old"]);
    }

    #[test]
    fn fork_visibility_unknown_when_order_cannot_decide() {
        // No declared order: ch-1 vs the fork point ch-2 is incomparable —
        // the inherited fact surfaces as unknown (B-1), never silently out.
        let early = fact("f-early", "gt", "ch-1", None);
        let store = store_with_forks(vec![early], &[("route", MAIN_BRANCH, "ch-2")]);
        let view = frame_view(&store, &CanonOrder::empty(), "gt", "route", None, "ch-2").unwrap();
        assert!(view.holding.is_empty());
        assert_eq!(view.unknown, vec!["f-early".to_string()]);
    }

    #[test]
    fn fork_grandchild_inherits_ancestor_branch_order() {
        // Ancestry order composition: deep forks inherit every ancestor's
        // declared edge set without redeclaration.
        let decl = CanonOrderFile {
            edges: vec![["ch-1".to_string(), "ch-2".to_string()]],
            branches: BTreeMap::from([(
                "route".to_string(),
                vec![["ch-2".to_string(), "ch-3".to_string()]],
            )]),
        };
        let store = store_with_forks(
            vec![branch_fact("f-deep", "gt", "deep", "ch-3")],
            &[("route", MAIN_BRANCH, "ch-2"), ("deep", "route", "ch-3")],
        );
        let composition = world_order_composition(&store.branches).unwrap();
        // No confluence here, so composition = the backward ancestor chain.
        assert_eq!(
            composition["deep"],
            vec!["route".to_string(), MAIN_BRANCH.to_string()]
        );
        let order = CanonOrder::from_declaration(&decl, &composition).unwrap();
        // ch-2 -> ch-3 was declared on `route`; `deep` inherits it.
        assert!(order.le("deep", "ch-2", "ch-3"));
        assert!(!order.le(MAIN_BRANCH, "ch-2", "ch-3"));
        let view = frame_view(&store, &order, "gt", "deep", None, "ch-4").unwrap();
        // f-deep starts at ch-3; ch-3 vs ch-4 undeclared everywhere -> not
        // holding at ch-4 is undecidable => unknown (honesty).
        assert_eq!(view.unknown, vec!["f-deep".to_string()]);
    }

    /// Round 439 — judgment-time content pin: amending the TARGET of a
    /// recorded conflict surfaces the edge as stale; amending the
    /// edge-owning fact restamps its outbound judgments (re-affirmation).
    #[test]
    fn amended_conflict_target_surfaces_stale_edge_until_reaffirmed() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("s.json");
        let mut store = AtomicStore::new();
        for ch in ["ch-1", "ch-2"] {
            store
                .sections
                .insert(ch.to_string(), AtomicSection::default());
        }
        store
            .frames
            .insert("seward".to_string(), mnemosyne_core::Frame::default());
        let target = fact("f-target", "seward", "ch-1", None);
        let mut owner = fact("f-owner", "seward", "ch-2", None);
        owner.conflicts_with = vec!["f-target".to_string()];
        mnemosyne_atomic::add_fact(&mut store, &path, &target).unwrap();
        mnemosyne_atomic::add_fact(&mut store, &path, &owner.clone()).unwrap();
        let order = chain(&["ch-1", "ch-2"]);
        // Fresh stamp: no staleness (the overlap violation itself may fire;
        // filter for the stale kind).
        let stale_count = |report: &ContinuityReport| {
            report
                .violations
                .iter()
                .filter(|v| matches!(v, ContinuityViolation::ConflictEdgeStale { .. }))
                .count()
        };
        let report = scan_continuity(&store, &order, &[]).unwrap();
        assert_eq!(stale_count(&report), 0, "{:?}", report.violations);
        // Amend the target's claim: the recorded judgment pinned other text.
        let revised_target = FactImport {
            claim: "a materially different claim".to_string(),
            ..fact("f-target", "seward", "ch-1", None)
        };
        mnemosyne_atomic::amend_fact(&mut store, &path, &revised_target, "revision").unwrap();
        let report = scan_continuity(&store, &order, &[]).unwrap();
        assert_eq!(stale_count(&report), 1, "{:?}", report.violations);
        // Re-affirm: amend the edge-owning fact (same content) — its
        // outbound judgments restamp against the target's CURRENT claim.
        mnemosyne_atomic::amend_fact(&mut store, &path, &owner, "re-affirm edges").unwrap();
        let report = scan_continuity(&store, &order, &[]).unwrap();
        assert_eq!(stale_count(&report), 0, "{:?}", report.violations);
    }

    /// Round 440 — out-of-band corruption fails LOUD as an `Err` from both
    /// read paths (previously an unregistered fact branch panicked the
    /// scan's lineage lookup; registry/section integrity was only enforced
    /// at the write path).
    #[test]
    fn out_of_band_corruption_errors_instead_of_panicking() {
        let order = chain(&["ch-1", "ch-2"]);
        // Unregistered branch on a fact (hand-edited store).
        let mut store = store_with(vec![fact("f1", "seward", "ch-1", None)]);
        store.narrative_facts.get_mut("f1").unwrap().branch = "ghost".to_string();
        let err = scan_continuity(&store, &order, &[]).unwrap_err();
        assert!(err.contains("branch registry"), "{err}");
        let err = frame_view(&store, &order, "seward", MAIN_BRANCH, None, "ch-1").unwrap_err();
        assert!(err.contains("branch registry"), "{err}");
        // Unregistered frame.
        let mut store = store_with(vec![fact("f1", "seward", "ch-1", None)]);
        store.narrative_facts.get_mut("f1").unwrap().frame = "nobody".to_string();
        let err = scan_continuity(&store, &order, &[]).unwrap_err();
        assert!(err.contains("frames registry"), "{err}");
        // Evidence emptied out-of-band.
        let mut store = store_with(vec![fact("f1", "seward", "ch-1", None)]);
        store
            .narrative_facts
            .get_mut("f1")
            .unwrap()
            .evidence
            .clear();
        let err = scan_continuity(&store, &order, &[]).unwrap_err();
        assert!(err.contains("unauditable"), "{err}");
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
        let report = scan_continuity(&store, &order, &[]).unwrap();
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
        let view = frame_view(&store, &order, frame, branch, None, at).unwrap();
        let held: Vec<&str> = view.holding.iter().map(|e| e.fact_id.as_str()).collect();
        assert!(held.contains(&fact_a.as_str()) && held.contains(&fact_b.as_str()));
    }

    // ========================================================================
    // Setup/payoff coverage (Round 442).
    // ========================================================================

    fn setup_fact(id: &str, frame: &str, from: &str) -> FactImport {
        FactImport {
            payoff_expectation: Some("expected".to_string()),
            ..fact(id, frame, from, None)
        }
    }

    fn payoff_fact(id: &str, frame: &str, from: &str, pays: &[&str]) -> FactImport {
        FactImport {
            pays_off: pays.iter().map(|s| s.to_string()).collect(),
            ..fact(id, frame, from, None)
        }
    }

    /// The 3-way classification on real shapes: paid (multi-payoff,
    /// cross-frame), dangling, exempt; plus the honesty counts
    /// (payoff to an unmarked fact, payoff before its setup).
    #[test]
    fn payoff_coverage_classifies_paid_dangling_exempt() {
        let mut paid_twice = payoff_fact("p-b", "gt", "ch-3", &["su-multi"]);
        paid_twice.frame = "gt".to_string();
        let store = store_with(vec![
            setup_fact("su-multi", "seward", "ch-1"), // cross-frame payoffs
            payoff_fact("p-a", "gt", "ch-2", &["su-multi"]),
            paid_twice,
            setup_fact("su-dangling", "gt", "ch-2"),
            fact("world-state", "gt", "ch-1", None), // exempt
            payoff_fact("p-unmarked", "gt", "ch-3", &["world-state"]),
            payoff_fact("p-early", "gt", "ch-1", &["su-late"]),
            setup_fact("su-late", "gt", "ch-3"),
        ]);
        let order = chain(&["ch-1", "ch-2", "ch-3", "ch-4"]);
        let report = payoff_coverage(&store, &order).unwrap();
        assert_eq!(report.setups_total, 3);
        let main = &report.worlds[MAIN_BRANCH];
        let paid: Vec<&str> = main.paid.iter().map(|p| p.setup.as_str()).collect();
        assert_eq!(paid, vec!["su-late", "su-multi"], "{main:?}");
        assert_eq!(
            main.paid
                .iter()
                .find(|p| p.setup == "su-multi")
                .unwrap()
                .payoffs,
            vec!["p-a".to_string(), "p-b".to_string()],
            "multi-payoff credits every in-world payoff"
        );
        assert_eq!(main.dangling, vec!["su-dangling".to_string()]);
        assert_eq!(main.exempt, 5, "unmarked facts counted, never listed");
        assert_eq!(
            main.payoffs_to_unmarked,
            vec![PayoffEdgeRef {
                payoff: "p-unmarked".to_string(),
                setup: "world-state".to_string(),
            }]
        );
        assert_eq!(
            main.payoff_before_setup,
            vec![PayoffEdgeRef {
                payoff: "p-early".to_string(),
                setup: "su-late".to_string(),
            }],
            "mystery structure surfaced, never gated — su-late still classifies paid"
        );
    }

    /// World scoping (the probe's central finding): a fork inherits the
    /// pre-fork setup but NOT main's post-fork payoff — the inherited setup
    /// dangles on the fork until that world pays it; the fork's payoff
    /// never credits main's world.
    #[test]
    fn payoff_coverage_is_world_scoped_across_forks() {
        let store = store_with_forks(
            vec![
                setup_fact("su", "gt", "ch-1"),
                payoff_fact("p-main", "gt", "ch-3", &["su"]),
            ],
            &[("route", MAIN_BRANCH, "ch-2")],
        );
        let order = chain(&["ch-1", "ch-2", "ch-3", "ch-4"]);
        let report = payoff_coverage(&store, &order).unwrap();
        assert_eq!(
            report.worlds[MAIN_BRANCH].paid[0].setup, "su",
            "main pays its own gun"
        );
        assert_eq!(
            report.worlds["route"].dangling,
            vec!["su".to_string()],
            "inherited setup dangles on the fork — each playthrough resolves its own guns"
        );
        // Paying it ON the fork flips the fork world only.
        let store = store_with_forks(
            vec![
                setup_fact("su", "gt", "ch-1"),
                payoff_fact("p-main", "gt", "ch-3", &["su"]),
                {
                    let mut p = branch_fact("p-route", "gt", "route", "ch-3");
                    p.pays_off = vec!["su".to_string()];
                    p
                },
            ],
            &[("route", MAIN_BRANCH, "ch-2")],
        );
        let report = payoff_coverage(&store, &order).unwrap();
        assert_eq!(report.worlds["route"].paid[0].payoffs, vec!["p-route"]);
        assert_eq!(
            report.worlds[MAIN_BRANCH].paid[0].payoffs,
            vec!["p-main"],
            "the fork's payoff never leaks back into main's classification"
        );
    }

    /// Round 485 — deterministic payoff substantiation. Set a typed leg on a
    /// FactImport (registers the entity + predicate via `derived_registries`).
    fn typed_value(mut f: FactImport, subject: &str, predicate: &str, value: &str) -> FactImport {
        f.entities = vec![subject.to_string()];
        f.typed = Some(mnemosyne_core::TypedClaim {
            subject: subject.to_string(),
            predicate: predicate.to_string(),
            object: mnemosyne_core::TypedObject::Value {
                value: value.to_string(),
            },
        });
        f
    }

    #[test]
    fn payoff_substantiation_classifies_substantiated_unsubstantiated_unverifiable() {
        let store = store_with(vec![
            // typed setup + a typed state-change on the same subject+predicate.
            typed_value(
                setup_fact("su-diary", "gt", "ch-1"),
                "diary",
                "state",
                "sealed",
            ),
            typed_value(
                payoff_fact("p-diary", "gt", "ch-2", &["su-diary"]),
                "diary",
                "state",
                "opened",
            ),
            // typed setup + a TYPED payoff that re-asserts the same value (no
            // change) -> hollow, unsubstantiated.
            typed_value(setup_fact("su-gun", "gt", "ch-1"), "gun", "state", "loaded"),
            typed_value(
                payoff_fact("p-gun", "gt", "ch-2", &["su-gun"]),
                "gun",
                "state",
                "loaded",
            ),
            // typed setup + an UNTYPED (prose-only) payoff -> can't check ->
            // unverifiable.
            typed_value(
                setup_fact("su-safe", "gt", "ch-1"),
                "safe",
                "state",
                "locked",
            ),
            payoff_fact("p-safe", "gt", "ch-2", &["su-safe"]),
            // untyped setup -> unverifiable.
            setup_fact("su-letter", "gt", "ch-1"),
            payoff_fact("p-letter", "gt", "ch-2", &["su-letter"]),
        ]);
        let order = chain(&["ch-1", "ch-2", "ch-3", "ch-4"]);
        let report = payoff_substantiation(&store, &order).unwrap();
        let w = &report.worlds[MAIN_BRANCH];
        let names =
            |v: &[PaidSetup]| -> Vec<String> { v.iter().map(|p| p.setup.clone()).collect() };
        assert_eq!(names(&w.substantiated), vec!["su-diary"]);
        assert_eq!(w.substantiated[0].payoffs, vec!["p-diary".to_string()]);
        assert_eq!(names(&w.unsubstantiated), vec!["su-gun"]);
        let mut unver = names(&w.unverifiable);
        unver.sort();
        assert_eq!(unver, vec!["su-letter".to_string(), "su-safe".to_string()]);
    }

    /// Round 488 — the wrong-branch authoring footgun made loud. A canon
    /// coordinate positioned in some branch's order, but on a fact whose own
    /// branch does not name it, is FactCanonOffBranch. (The R486 acceptance
    /// hit exactly this: a trunk fact defaulted to `main` while the trunk was
    /// the named branch `spine`, so the conflict gate never compared it.)
    #[test]
    fn fact_canon_off_branch_caught_on_branch_clean() {
        let order = CanonOrder::from_declaration(
            &CanonOrderFile {
                edges: vec![],
                branches: BTreeMap::from([(
                    "spine".to_string(),
                    vec![
                        ["ch-1".to_string(), "ch-2".to_string()],
                        ["ch-2".to_string(), "ch-3".to_string()],
                    ],
                )]),
            },
            &BTreeMap::from([("spine".to_string(), vec![])]),
        )
        .unwrap();
        // ch-3 is positioned in `spine`; a fact on `main` (the default) does not
        // name it -> off-branch (the silent wrong-branch error). An anchor fact
        // registers the `spine` branch the order declares.
        let off = store_with(vec![
            FactImport {
                branch: Some("spine".to_string()),
                ..fact("f-anchor", "gt", "ch-1", None)
            },
            fact("f-stray", "gt", "ch-3", None),
        ]);
        let report = scan_continuity(&off, &order, &[]).unwrap();
        assert!(
            report.violations.iter().any(|v| matches!(
                v,
                ContinuityViolation::FactCanonOffBranch { fact, branch, coord }
                    if fact == "f-stray" && branch == MAIN_BRANCH && coord == "ch-3"
            )),
            "off-branch canon coordinate must be caught: {:?}",
            report.violations
        );
        // The same coordinate on `spine` (which names ch-3) is clean.
        let on = store_with(vec![FactImport {
            branch: Some("spine".to_string()),
            ..fact("f-ok", "gt", "ch-3", None)
        }]);
        let report = scan_continuity(&on, &order, &[]).unwrap();
        assert!(
            !report
                .violations
                .iter()
                .any(|v| matches!(v, ContinuityViolation::FactCanonOffBranch { .. })),
            "on-branch coordinate must be clean: {:?}",
            report.violations
        );
    }

    /// Round 522 (design sec 7.27 Piece B) — a STRUCTURAL backreference (an
    /// `evidence` citation) to a sibling world-line's scene is the case-2
    /// defect R520 surfaced: it is the R488 off-branch reachability, now
    /// applied to evidence. Spine/prior evidence (reachable before the fact in
    /// its own branch) is clean; sibling-branch evidence fails.
    #[test]
    fn evidence_off_branch_caught_spine_evidence_clean() {
        // fork at ch-2: `left` = ch-2->ch-3, `right` = ch-2->ch-4.
        let order = CanonOrder::from_declaration(
            &CanonOrderFile {
                edges: vec![["ch-1".to_string(), "ch-2".to_string()]],
                branches: BTreeMap::from([
                    (
                        "left".to_string(),
                        vec![["ch-2".to_string(), "ch-3".to_string()]],
                    ),
                    (
                        "right".to_string(),
                        vec![["ch-2".to_string(), "ch-4".to_string()]],
                    ),
                ]),
            },
            &BTreeMap::from([("left".to_string(), vec![]), ("right".to_string(), vec![])]),
        )
        .unwrap();
        // A fact on `left` whose evidence cites ch-4 — a scene only on the
        // sibling `right` world-line — is an off-branch backreference.
        let off = store_with(vec![
            FactImport {
                branch: Some("left".to_string()),
                evidence: vec!["ch-4".to_string()],
                ..fact("f-cross", "gt", "ch-3", None)
            },
            FactImport {
                branch: Some("right".to_string()),
                ..fact("f-r", "gt", "ch-4", None)
            },
        ]);
        let report = scan_continuity(&off, &order, &[]).unwrap();
        assert!(
            report.violations.iter().any(|v| matches!(
                v,
                ContinuityViolation::EvidenceUnreachable { fact, branch, evidence, .. }
                    if fact == "f-cross" && branch == "left" && evidence == "ch-4"
            )),
            "sibling-branch evidence must be caught: {:?}",
            report.violations
        );
        // Spine evidence (ch-1, reachable before ch-3 on `left`) is clean.
        let on = store_with(vec![
            FactImport {
                branch: Some("left".to_string()),
                evidence: vec!["ch-1".to_string()],
                ..fact("f-ok", "gt", "ch-3", None)
            },
            FactImport {
                branch: Some("right".to_string()),
                ..fact("f-r", "gt", "ch-4", None)
            },
        ]);
        let report = scan_continuity(&on, &order, &[]).unwrap();
        assert!(
            !report
                .violations
                .iter()
                .any(|v| matches!(v, ContinuityViolation::EvidenceUnreachable { .. })),
            "spine evidence must be clean: {:?}",
            report.violations
        );
    }

    /// Round 443 session review: a payoff edge between SIBLING branches
    /// credits in no world — both endpoints exist, no world sees them
    /// together. The dead edge itself surfaces as `uncredited_edges`
    /// (the dangling list only shows the symptom in the setup's world).
    /// An in-world edge never appears there.
    #[test]
    fn sibling_branch_payoff_edge_surfaces_as_uncredited() {
        let store = store_with_forks(
            vec![
                {
                    let mut s = branch_fact("su-a", "gt", "route-a", "ch-3");
                    s.payoff_expectation = Some("expected".to_string());
                    s
                },
                {
                    let mut p = branch_fact("p-b", "gt", "route-b", "ch-3");
                    p.pays_off = vec!["su-a".to_string()];
                    p
                },
            ],
            &[
                ("route-a", MAIN_BRANCH, "ch-2"),
                ("route-b", MAIN_BRANCH, "ch-2"),
            ],
        );
        let order = chain(&["ch-1", "ch-2", "ch-3", "ch-4"]);
        let report = payoff_coverage(&store, &order).unwrap();
        assert_eq!(
            report.uncredited_edges,
            vec![PayoffEdgeRef {
                payoff: "p-b".to_string(),
                setup: "su-a".to_string(),
            }]
        );
        assert_eq!(
            report.worlds["route-a"].dangling,
            vec!["su-a".to_string()],
            "the symptom still shows in the setup's own world"
        );
        // A credited edge never lists.
        let store = store_with(vec![
            setup_fact("su", "gt", "ch-1"),
            payoff_fact("p", "gt", "ch-2", &["su"]),
        ]);
        let report = payoff_coverage(&store, &order).unwrap();
        assert!(report.uncredited_edges.is_empty());
    }

    /// Round 447 (R445 Detroit Finding 3): an Unknown endpoint in a world
    /// where the edge CANNOT credit (other endpoint Out) must not exempt a
    /// dead edge — under parallel protagonist chains the pre-fix blanket
    /// withdrawal masked every dead cross-chain edge behind Unknowns in
    /// unrelated forks.
    #[test]
    fn dead_edge_not_masked_by_unknown_in_non_crediting_world() {
        let store = store_with_forks(
            vec![
                {
                    // Late main-chain setup: Out in the early fork (the
                    // fork departed before it), Unknown in the
                    // parallel-chain fork (incomparable).
                    let mut s = fact("su-late", "gt", "ch-3", None);
                    s.payoff_expectation = Some("expected".to_string());
                    s
                },
                {
                    // Payoff on the early fork: its world never sees the
                    // late setup -> the edge credits in NO world.
                    let mut p = branch_fact("p-early", "gt", "b-early", "ch-2");
                    p.pays_off = vec!["su-late".to_string()];
                    p
                },
            ],
            &[
                ("b-early", MAIN_BRANCH, "ch-1"),
                ("b-k", MAIN_BRANCH, "k-1"),
            ],
        );
        let order = CanonOrder::from_edges(&[
            ["ch-1".to_string(), "ch-2".to_string()],
            ["ch-2".to_string(), "ch-3".to_string()],
            ["ch-3".to_string(), "ch-4".to_string()],
            ["k-1".to_string(), "k-2".to_string()],
        ])
        .unwrap();
        let report = payoff_coverage(&store, &order).unwrap();
        // In b-k the setup is Unknown (cross-chain) but the payoff is Out
        // (sibling fork) — that world is decided; the edge is dead.
        assert_eq!(
            report.uncredited_edges,
            vec![PayoffEdgeRef {
                payoff: "p-early".to_string(),
                setup: "su-late".to_string(),
            }]
        );
        assert!(report.undecidable_edges.is_empty());
    }

    /// Round 447 — the suspension that IS legitimate surfaces instead of
    /// silently draining: payoff In, setup Unknown in the same world
    /// (could credit there if the order were richer) = `undecidable_edges`.
    #[test]
    fn could_credit_unknown_surfaces_as_undecidable_edge() {
        let store = store_with_forks(
            vec![
                {
                    // Parallel-chain setup on main.
                    let mut s = fact("su-k", "gt", "k-1", None);
                    s.payoff_expectation = Some("expected".to_string());
                    s
                },
                {
                    // Payoff on a main-chain fork: in its world the payoff
                    // is In and the cross-chain setup is Unknown.
                    let mut p = branch_fact("p-x", "gt", "b-early", "ch-2");
                    p.pays_off = vec!["su-k".to_string()];
                    p
                },
            ],
            &[("b-early", MAIN_BRANCH, "ch-1")],
        );
        let order = CanonOrder::from_edges(&[
            ["ch-1".to_string(), "ch-2".to_string()],
            ["ch-2".to_string(), "ch-3".to_string()],
            ["k-1".to_string(), "k-2".to_string()],
        ])
        .unwrap();
        let report = payoff_coverage(&store, &order).unwrap();
        assert_eq!(
            report.undecidable_edges,
            vec![PayoffEdgeRef {
                payoff: "p-x".to_string(),
                setup: "su-k".to_string(),
            }]
        );
        assert!(report.uncredited_edges.is_empty());
    }

    /// Out-of-band pays_off target removal = scan violation (the
    /// conflict/succession symmetry), not a store-corruption Err.
    #[test]
    fn scan_recheck_surfaces_missing_payoff_target() {
        let mut store = store_with(vec![
            setup_fact("su", "gt", "ch-1"),
            payoff_fact("p", "gt", "ch-2", &["su"]),
        ]);
        store.narrative_facts.remove("su");
        let order = chain(&["ch-1", "ch-2", "ch-3", "ch-4"]);
        let report = scan_continuity(&store, &order, &[]).unwrap();
        assert!(
            report.violations.iter().any(|v| matches!(
                v,
                ContinuityViolation::PayoffTargetMissing { fact_id, target }
                    if fact_id == "p" && target == "su"
            )),
            "{:?}",
            report.violations
        );
    }

    // --- Round 449: narrative-rules gate (R441 probe mirror) ---------------

    use mnemosyne_core::{TypedClaim, TypedObject};

    /// A typed fact: subject (+ entity-shaped object) ride the entities
    /// list, per the Round 446 invariant (the typed leg never widens the
    /// retrieval key).
    fn typed_fact(
        id: &str,
        frame: &str,
        from: &str,
        subject: &str,
        predicate: &str,
        object: TypedObject,
    ) -> FactImport {
        let mut entities = vec![subject.to_string()];
        if let TypedObject::Entity { id } = &object {
            entities.push(id.clone());
        }
        FactImport {
            entities,
            typed: Some(TypedClaim {
                subject: subject.to_string(),
                predicate: predicate.to_string(),
                object,
            }),
            ..fact(id, frame, from, None)
        }
    }

    fn at(value: &str) -> TypedObject {
        TypedObject::Value {
            value: value.to_string(),
        }
    }

    fn holds(entity: &str) -> TypedObject {
        TypedObject::Entity {
            id: entity.to_string(),
        }
    }

    fn exclusive_rule(id: &str, predicate: &str, per: ExclusiveKey) -> NarrativeRule {
        NarrativeRule {
            id: id.to_string(),
            predicate: predicate.to_string(),
            spec: NarrativeRuleSpec::Exclusive { per },
        }
    }

    fn transition_rule(id: &str, predicate: &str, allowed: &[(&str, &str)]) -> NarrativeRule {
        NarrativeRule {
            id: id.to_string(),
            predicate: predicate.to_string(),
            spec: NarrativeRuleSpec::Transition {
                allowed: allowed
                    .iter()
                    .map(|(a, b)| [a.to_string(), b.to_string()])
                    .collect(),
            },
        }
    }

    /// A correctly chained location arc — including an A→B→A revisit shape —
    /// is green under the exclusive rule (R441 probe 1).
    #[test]
    fn rule_exclusive_chained_arc_is_green() {
        let mut l2 = typed_fact("l2", "gt", "ch-2", "dracula", "at-location", at("ship"));
        l2.supersedes_in_frame = Some("l1".to_string());
        let mut l3 = typed_fact("l3", "gt", "ch-3", "dracula", "at-location", at("castle"));
        l3.supersedes_in_frame = Some("l2".to_string());
        let store = store_with(vec![
            typed_fact("l1", "gt", "ch-1", "dracula", "at-location", at("castle")),
            l2,
            l3,
        ]);
        let order = chain(&["ch-1", "ch-2", "ch-3", "ch-4"]);
        let rules = [exclusive_rule("loc", "at-location", ExclusiveKey::Subject)];
        let report = scan_continuity(&store, &order, &rules).unwrap();
        assert!(report.violations.is_empty(), "{:?}", report.violations);
        assert_eq!(report.rules, 1);
    }

    /// A location fact that forgot the succession chain co-holds with the
    /// still-open predecessor — caught (R441 probe 2). The forgotten chain
    /// becomes a caught overlap: the authoring convention is now a checked
    /// invariant.
    #[test]
    fn rule_exclusive_catches_forgotten_location_chain() {
        let mut l2 = typed_fact("l2", "gt", "ch-2", "dracula", "at-location", at("england"));
        l2.supersedes_in_frame = Some("l1".to_string());
        let store = store_with(vec![
            typed_fact("l1", "gt", "ch-1", "dracula", "at-location", at("castle")),
            l2,
            // No chain: l2 `england` is still open at ch-3.
            typed_fact("bad", "gt", "ch-3", "dracula", "at-location", at("whitby")),
        ]);
        let order = chain(&["ch-1", "ch-2", "ch-3", "ch-4"]);
        let rules = [exclusive_rule("loc", "at-location", ExclusiveKey::Subject)];
        let report = scan_continuity(&store, &order, &rules).unwrap();
        assert!(
            report.violations.iter().any(|v| matches!(
                v,
                ContinuityViolation::RuleExclusiveOverlap { rule, fact_a, fact_b, branch, .. }
                    if rule == "loc" && fact_a == "bad" && fact_b == "l2"
                        && branch == MAIN_BRANCH
            )),
            "{:?}",
            report.violations
        );
    }

    /// Round 485 — Class B: the f-helene "claim contradicts its own cited
    /// evidence" drift (R483) is caught DETERMINISTICALLY by the existing
    /// exclusivity gate once both load-bearing legs are typed. No new mechanism
    /// — the R484 all-deterministic redesign relies on exactly this. Helene's
    /// `name` cannot be both `true-family` and `borrowed` at one canon point.
    #[test]
    fn class_b_contradiction_caught_by_exclusivity_once_typed() {
        let store = store_with(vec![
            typed_fact(
                "helene-claim",
                "gt",
                "ch-2",
                "helene",
                "name",
                at("true-family"),
            ),
            typed_fact(
                "sc06-evidence",
                "gt",
                "ch-1",
                "helene",
                "name",
                at("borrowed"),
            ),
        ]);
        let order = chain(&["ch-1", "ch-2", "ch-3", "ch-4"]);
        let rules = [exclusive_rule("name", "name", ExclusiveKey::Subject)];
        let report = scan_continuity(&store, &order, &rules).unwrap();
        assert!(
            report.violations.iter().any(|v| matches!(
                v,
                ContinuityViolation::RuleExclusiveOverlap { rule, .. } if rule == "name"
            )),
            "the typed name contradiction must surface deterministically: {:?}",
            report.violations
        );
    }

    /// per:subject skips pairs whose OBJECTS agree — a restated location is
    /// one value said twice, not two values (R443 symmetric skip, leg 1).
    #[test]
    fn rule_exclusive_per_subject_skips_restated_value() {
        let store = store_with(vec![
            typed_fact("l1", "gt", "ch-1", "dracula", "at-location", at("castle")),
            typed_fact("dup", "gt", "ch-2", "dracula", "at-location", at("castle")),
        ]);
        let order = chain(&["ch-1", "ch-2", "ch-3", "ch-4"]);
        let rules = [exclusive_rule("loc", "at-location", ExclusiveKey::Subject)];
        let report = scan_continuity(&store, &order, &rules).unwrap();
        assert!(report.violations.is_empty(), "{:?}", report.violations);
    }

    /// per:object catches two holders of one object co-holding (R441 probe
    /// 4 — conservation/custody).
    #[test]
    fn rule_exclusive_per_object_catches_double_custody() {
        let mut c2 = typed_fact("c2", "gt", "ch-2", "mina", "holds", holds("journal"));
        c2.supersedes_in_frame = Some("c1".to_string());
        let store = store_with(vec![
            typed_fact("c1", "gt", "ch-1", "jonathan", "holds", holds("journal")),
            c2,
            // Second holder, no chain: c2 `mina holds` is open from ch-2.
            typed_fact("bad", "gt", "ch-3", "seward", "holds", holds("journal")),
        ]);
        let order = chain(&["ch-1", "ch-2", "ch-3", "ch-4"]);
        let rules = [exclusive_rule("custody", "holds", ExclusiveKey::Object)];
        let report = scan_continuity(&store, &order, &rules).unwrap();
        assert!(
            report.violations.iter().any(|v| matches!(
                v,
                ContinuityViolation::RuleExclusiveOverlap { rule, .. } if rule == "custody"
            )),
            "{:?}",
            report.violations
        );
    }

    /// per:object skips pairs whose SUBJECTS agree — a restated holder is
    /// one holder said twice, not two holders (R443 symmetric skip, leg 2;
    /// the pre-review probe had this direction missing and false-positived
    /// a restated custody fact). The restatement's extent is closed before
    /// the custody transfer — an open extent here would be a GENUINE
    /// two-holder conflict with the later holder (the R443 fixture lesson).
    #[test]
    fn rule_exclusive_per_object_skips_restated_holder() {
        let mut dup = typed_fact("dup", "gt", "ch-2", "jonathan", "holds", holds("journal"));
        dup.canon_to = Some("ch-2".to_string());
        let mut c2 = typed_fact("c2", "gt", "ch-3", "mina", "holds", holds("journal"));
        c2.supersedes_in_frame = Some("c1".to_string());
        let store = store_with(vec![
            typed_fact("c1", "gt", "ch-1", "jonathan", "holds", holds("journal")),
            dup,
            c2,
        ]);
        let order = chain(&["ch-1", "ch-2", "ch-3", "ch-4"]);
        let rules = [exclusive_rule("custody", "holds", ExclusiveKey::Object)];
        let report = scan_continuity(&store, &order, &rules).unwrap();
        assert!(report.violations.is_empty(), "{:?}", report.violations);
    }

    /// Cross-frame same-key pairs are data, never gated (R441 probe 5 —
    /// frames are never cross-validated; the North-Star sentence carries
    /// into the rule class unchanged).
    #[test]
    fn rule_exclusive_cross_frame_pair_is_data() {
        let store = store_with(vec![
            typed_fact("gt", "gt", "ch-1", "dracula", "at-location", at("england")),
            typed_fact(
                "belief",
                "jonathan",
                "ch-1",
                "dracula",
                "at-location",
                at("castle"),
            ),
        ]);
        let order = chain(&["ch-1", "ch-2", "ch-3", "ch-4"]);
        let rules = [exclusive_rule("loc", "at-location", ExclusiveKey::Subject)];
        let report = scan_continuity(&store, &order, &rules).unwrap();
        assert!(report.violations.is_empty(), "{:?}", report.violations);
    }

    /// A transition outside the allowed set, riding the succession edge —
    /// caught at the exact offending pair (R441 probe 3).
    #[test]
    fn rule_transition_catches_disallowed_step() {
        let mut s2 = typed_fact("s2", "gt", "ch-2", "lucy", "life-status", at("dead"));
        s2.supersedes_in_frame = Some("s1".to_string());
        let mut bad = typed_fact("bad", "gt", "ch-3", "lucy", "life-status", at("alive"));
        bad.supersedes_in_frame = Some("s2".to_string());
        let store = store_with(vec![
            typed_fact("s1", "gt", "ch-1", "lucy", "life-status", at("alive")),
            s2,
            bad,
        ]);
        let order = chain(&["ch-1", "ch-2", "ch-3", "ch-4"]);
        let rules = [transition_rule(
            "life",
            "life-status",
            &[("alive", "dead"), ("dead", "undead")],
        )];
        let report = scan_continuity(&store, &order, &rules).unwrap();
        assert_eq!(report.violations.len(), 1, "{:?}", report.violations);
        match &report.violations[0] {
            ContinuityViolation::RuleTransitionInvalid {
                rule,
                predecessor,
                successor,
                from,
                to,
                ..
            } => {
                assert_eq!(rule, "life");
                assert_eq!(predecessor, "s2");
                assert_eq!(successor, "bad");
                assert_eq!(from, "dead");
                assert_eq!(to, "alive");
            }
            v => panic!("wrong violation: {v:?}"),
        }
        // The whole s1→s2→bad arc is succession-connected (path, not
        // direct edge — Round 452): zero unchained pairs on chained data,
        // even with the invalid step (the violation IS the signal there).
        assert_eq!(report.unchained_state_pairs, 0);
    }

    /// Fork world (R441 probe 6): a what-if branch keeps its own state
    /// without colliding with main's post-fork facts, and the unchained
    /// honesty count is WORLD-scoped — the inherited-vs-fork pair (s1 on
    /// main, w1 on the fork) is visible together ONLY in the fork world;
    /// raw branch equality would silently miss it (the probe finding).
    #[test]
    fn rule_fork_world_scoping_and_unchained_count() {
        let mut w1 = typed_fact("w1", "gt", "ch-2", "lucy", "life-status", at("alive"));
        w1.branch = Some("lucy-lives".to_string());
        let store = store_with_forks(
            vec![
                typed_fact("s1", "gt", "ch-1", "lucy", "life-status", at("alive")),
                // Main continues without the fork: lucy dies at ch-2.
                {
                    let mut s2 = typed_fact("s2", "gt", "ch-2", "lucy", "life-status", at("dead"));
                    s2.supersedes_in_frame = Some("s1".to_string());
                    s2
                },
                w1,
            ],
            &[("lucy-lives", MAIN_BRANCH, "ch-1")],
        );
        let order = chain(&["ch-1", "ch-2", "ch-3", "ch-4"]);
        let rules = [transition_rule("life", "life-status", &[("alive", "dead")])];
        let report = scan_continuity(&store, &order, &rules).unwrap();
        // No violation: w1 has no succession edge (fork-vs-main is never a
        // transition), and main's own chain is allowed.
        assert!(report.violations.is_empty(), "{:?}", report.violations);
        // Exactly the (s1, w1) pair surfaces: same frame + subject, both
        // visible only in the lucy-lives world, not chained. (s2, w1)
        // never co-occur in any world; (s1, s2) is chained.
        assert_eq!(report.unchained_state_pairs, 1);
    }

    /// Sibling forks never share a world: each world-line's state facts are
    /// data to the other (B-2 carried into the rule class).
    #[test]
    fn rule_exclusive_sibling_worlds_are_data() {
        let mut a = typed_fact("on-a", "gt", "ch-2", "kara", "at-location", at("highway"));
        a.branch = Some("w-a".to_string());
        let mut b = typed_fact("on-b", "gt", "ch-2", "kara", "at-location", at("motel"));
        b.branch = Some("w-b".to_string());
        let store = store_with_forks(
            vec![a, b],
            &[("w-a", MAIN_BRANCH, "ch-1"), ("w-b", MAIN_BRANCH, "ch-1")],
        );
        let order = chain(&["ch-1", "ch-2", "ch-3", "ch-4"]);
        let rules = [exclusive_rule("loc", "at-location", ExclusiveKey::Subject)];
        let report = scan_continuity(&store, &order, &rules).unwrap();
        assert!(report.violations.is_empty(), "{:?}", report.violations);
    }

    /// An exclusive candidate pair the declared order cannot compare is
    /// surfaced as a count, never gated (B-1).
    #[test]
    fn rule_unordered_pair_surfaced_not_gated() {
        let store = store_with(vec![
            typed_fact("l1", "gt", "ch-1", "dracula", "at-location", at("castle")),
            typed_fact("l2", "gt", "ch-2", "dracula", "at-location", at("england")),
        ]);
        let rules = [exclusive_rule("loc", "at-location", ExclusiveKey::Subject)];
        let report = scan_continuity(&store, &CanonOrder::empty(), &rules).unwrap();
        assert!(report.violations.is_empty(), "{:?}", report.violations);
        assert_eq!(report.rule_unordered_pairs, 1);
    }

    /// A rule naming an unregistered predicate fails LOUD at the scan
    /// boundary — a typo'd predicate must not silently escape its rule
    /// (the R436 write-side-typo lesson applied to the read side).
    #[test]
    fn rule_unknown_predicate_fails_loud() {
        let store = store_with(vec![typed_fact(
            "l1",
            "gt",
            "ch-1",
            "dracula",
            "at-location",
            at("castle"),
        )]);
        let order = chain(&["ch-1", "ch-2", "ch-3", "ch-4"]);
        let rules = [exclusive_rule("loc", "at-locaton", ExclusiveKey::Subject)];
        let err = scan_continuity(&store, &order, &rules).unwrap_err();
        assert!(err.contains("at-locaton"), "{err}");
        assert!(err.contains("predicate registry"), "{err}");
    }

    /// Loader contract: parse + file-shape validation + the sha256 pin
    /// (R428 authority-input rule shared with the canon order).
    #[test]
    fn rules_loader_validates_shape_and_pin() {
        let tmp = tempfile::TempDir::new().unwrap();
        let write = |name: &str, body: &str| {
            let p = tmp.path().join(name);
            std::fs::write(&p, body).unwrap();
            p
        };
        // Happy path: both classes, the canonical `schema` tag and the
        // `comment` annotation slot accepted (Round 472 — strict otherwise).
        let ok = write(
            "ok.json",
            r#"{"schema":"narrative-rules/v1","comment":"dogfood shape","rules":[
                {"id":"loc","class":"exclusive","predicate":"at-location","per":"subject"},
                {"id":"life","class":"transition","predicate":"life-status",
                 "allowed":[["alive","dead"]]}
            ]}"#,
        );
        let file = load_narrative_rules(&ok, None).unwrap();
        assert_eq!(file.rules.len(), 2);
        // Pin mismatch fails loud and names the config key.
        let err = load_narrative_rules(&ok, Some(&"0".repeat(64))).unwrap_err();
        assert!(err.contains("rules_sha256"), "{err}");
        // Matching pin passes.
        let hash = { mnemosyne_core::sha256_hex(&std::fs::read(&ok).unwrap()) };
        assert!(load_narrative_rules(&ok, Some(&hash)).is_ok());
        // Duplicate rule ids reject (ids name findings).
        let dup = write(
            "dup.json",
            r#"{"rules":[
                {"id":"r","class":"exclusive","predicate":"p","per":"subject"},
                {"id":"r","class":"transition","predicate":"q","allowed":[]}
            ]}"#,
        );
        let err = load_narrative_rules(&dup, None).unwrap_err();
        assert!(err.contains("duplicate rule id"), "{err}");
        // Blank id / blank predicate / blank transition leg reject.
        let blank_id = write(
            "blank-id.json",
            r#"{"rules":[{"id":" ","class":"exclusive","predicate":"p","per":"subject"}]}"#,
        );
        assert!(load_narrative_rules(&blank_id, None)
            .unwrap_err()
            .contains("blank rule id"));
        let blank_pred = write(
            "blank-pred.json",
            r#"{"rules":[{"id":"r","class":"exclusive","predicate":"","per":"subject"}]}"#,
        );
        assert!(load_narrative_rules(&blank_pred, None)
            .unwrap_err()
            .contains("blank predicate"));
        let blank_leg = write(
            "blank-leg.json",
            r#"{"rules":[{"id":"r","class":"transition","predicate":"p","allowed":[["a",""]]}]}"#,
        );
        assert!(load_narrative_rules(&blank_leg, None)
            .unwrap_err()
            .contains("blank leg"));
        // An unknown class tag is a parse error (serde-tagged, fail-loud).
        let bad_class = write(
            "bad-class.json",
            r#"{"rules":[{"id":"r","class":"implication","predicate":"p"}]}"#,
        );
        assert!(load_narrative_rules(&bad_class, None)
            .unwrap_err()
            .contains("parse"));
        // An unknown `per` leg is a parse error too.
        let bad_per = write(
            "bad-per.json",
            r#"{"rules":[{"id":"r","class":"exclusive","predicate":"p","per":"verb"}]}"#,
        );
        assert!(load_narrative_rules(&bad_per, None)
            .unwrap_err()
            .contains("parse"));
    }

    /// Round 472 — the loader rejects unknown and class-incoherent keys
    /// loudly instead of dropping them (the lenient `flatten` parse let the
    /// A/B run's transition rule carry a `per` scope that did nothing). The
    /// silent-no-op class already closed for R450 (padded predicate) and
    /// R468 (unknown `--field`).
    #[test]
    fn rules_loader_rejects_unknown_and_incoherent_fields() {
        let tmp = tempfile::TempDir::new().unwrap();
        let write = |name: &str, body: &str| {
            let p = tmp.path().join(name);
            std::fs::write(&p, body).unwrap();
            p
        };
        // The S7 field-proof: a `per` scope on a TRANSITION rule was
        // silently ignored; now it names the misplaced leg.
        let s7 = write(
            "s7.json",
            r#"{"rules":[{"id":"r","class":"transition","predicate":"p",
                "per":"subject","allowed":[["a","b"]]}]}"#,
        );
        let err = load_narrative_rules(&s7, None).unwrap_err();
        assert!(err.contains("transition") && err.contains("per"), "{err}");
        // Symmetric: an `allowed` leg on an EXCLUSIVE rule.
        let stray_allowed = write(
            "stray-allowed.json",
            r#"{"rules":[{"id":"r","class":"exclusive","predicate":"p",
                "per":"subject","allowed":[["a","b"]]}]}"#,
        );
        let err = load_narrative_rules(&stray_allowed, None).unwrap_err();
        assert!(
            err.contains("exclusive") && err.contains("allowed"),
            "{err}"
        );
        // An unknown RULE-level key (not just a misplaced known one).
        let unknown_rule = write(
            "unknown-rule.json",
            r#"{"rules":[{"id":"r","class":"exclusive","predicate":"p",
                "per":"subject","subject":"x"}]}"#,
        );
        assert!(load_narrative_rules(&unknown_rule, None)
            .unwrap_err()
            .contains("parse"));
        // An unknown FILE-level key.
        let unknown_file = write(
            "unknown-file.json",
            r#"{"schema":"narrative-rules/v1","rules":[],"bogus":1}"#,
        );
        assert!(load_narrative_rules(&unknown_file, None)
            .unwrap_err()
            .contains("parse"));
        // A present-but-wrong schema tag (the wrong-version silent-no-op).
        let bad_schema = write(
            "bad-schema.json",
            r#"{"schema":"narrative-rules/v2","rules":[]}"#,
        );
        assert!(load_narrative_rules(&bad_schema, None)
            .unwrap_err()
            .contains("schema"));
        // A missing leg is named, not defaulted.
        let no_per = write(
            "no-per.json",
            r#"{"rules":[{"id":"r","class":"exclusive","predicate":"p"}]}"#,
        );
        assert!(load_narrative_rules(&no_per, None)
            .unwrap_err()
            .contains("missing"));
        let no_allowed = write(
            "no-allowed.json",
            r#"{"rules":[{"id":"r","class":"transition","predicate":"p"}]}"#,
        );
        assert!(load_narrative_rules(&no_allowed, None)
            .unwrap_err()
            .contains("missing"));
    }

    /// The rule gate and the frame view read the SAME holds_at: a fact the
    /// view shows as holding at T is exactly a fact the exclusive rule can
    /// see co-holding at T (the third-reader contract — no drift possible).
    #[test]
    fn rule_gate_and_view_agree_on_co_holding() {
        let mut l2 = typed_fact("l2", "gt", "ch-2", "dracula", "at-location", at("england"));
        l2.supersedes_in_frame = Some("l1".to_string());
        let store = store_with(vec![
            typed_fact("l1", "gt", "ch-1", "dracula", "at-location", at("castle")),
            l2,
            typed_fact("bad", "gt", "ch-3", "dracula", "at-location", at("whitby")),
        ]);
        let order = chain(&["ch-1", "ch-2", "ch-3", "ch-4"]);
        let rules = [exclusive_rule("loc", "at-location", ExclusiveKey::Subject)];
        let report = scan_continuity(&store, &order, &rules).unwrap();
        let at_point = report
            .violations
            .iter()
            .find_map(|v| match v {
                ContinuityViolation::RuleExclusiveOverlap { at, .. } => Some(at.clone()),
                _ => None,
            })
            .expect("overlap expected");
        let view = frame_view(&store, &order, "gt", MAIN_BRANCH, None, &at_point).unwrap();
        let held: BTreeSet<&str> = view.holding.iter().map(|e| e.fact_id.as_str()).collect();
        assert!(held.contains("l2") && held.contains("bad"), "{held:?}");
    }

    /// R450 session review — whitespace normalization: the loader trims
    /// id/predicate/transition legs INTO the stored values, so a padded
    /// declaration still arms its rule (pre-fix it passed the trimmed
    /// boundary check yet matched no typed fact — silently disarmed); a
    /// programmatic rule that skipped the loader fails the EXACT registry
    /// compare loud.
    #[test]
    fn rules_whitespace_normalizes_at_load_and_padded_programmatic_fails_loud() {
        let tmp = tempfile::TempDir::new().unwrap();
        let padded = tmp.path().join("padded.json");
        std::fs::write(
            &padded,
            r#"{"rules":[{"id":" life ","class":"transition","predicate":" life-status",
                "allowed":[[" alive","dead "]]}]}"#,
        )
        .unwrap();
        let file = load_narrative_rules(&padded, None).unwrap();
        assert_eq!(file.rules[0].id, "life");
        assert_eq!(file.rules[0].predicate, "life-status");
        let mut s2 = typed_fact("s2", "gt", "ch-3", "lucy", "life-status", at("undead"));
        s2.supersedes_in_frame = Some("s1".to_string());
        let store = store_with(vec![
            typed_fact("s1", "gt", "ch-1", "lucy", "life-status", at("alive")),
            s2,
        ]);
        let order = chain(&["ch-1", "ch-2", "ch-3", "ch-4"]);
        // The normalized rule is ARMED: alive->undead is outside the
        // normalized allowed set [["alive","dead"]] and must fire.
        let report = scan_continuity(&store, &order, &file.rules).unwrap();
        assert_eq!(report.violations.len(), 1, "{:?}", report.violations);
        // A padded predicate that bypassed the loader: exact compare, loud.
        let err = scan_continuity(
            &store,
            &order,
            &[transition_rule(
                "life",
                " life-status",
                &[("alive", "dead")],
            )],
        )
        .unwrap_err();
        assert!(err.contains("` life-status`"), "{err}");
    }

    /// R450 session review — the per-world reporting contract pinned: a
    /// violating pair inherited by a fork reports in BOTH worlds (the world
    /// is part of the finding; the R441 probe's executable model).
    #[test]
    fn rule_exclusive_violation_reports_per_world_including_inheriting_fork() {
        let store = store_with_forks(
            vec![
                typed_fact("l1", "gt", "ch-1", "dracula", "at-location", at("castle")),
                typed_fact("l2", "gt", "ch-1", "dracula", "at-location", at("whitby")),
            ],
            &[("route", MAIN_BRANCH, "ch-2")],
        );
        let order = chain(&["ch-1", "ch-2", "ch-3", "ch-4"]);
        let rules = [exclusive_rule("loc", "at-location", ExclusiveKey::Subject)];
        let report = scan_continuity(&store, &order, &rules).unwrap();
        let worlds: Vec<&str> = report
            .violations
            .iter()
            .filter_map(|v| match v {
                ContinuityViolation::RuleExclusiveOverlap { branch, .. } => Some(branch.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(worlds, vec![MAIN_BRANCH, "route"], "{worlds:?}");
    }

    /// R452 session review — unchained means NO SUCCESSION PATH, not "no
    /// direct edge": a correct 4-step chain reports zero (the pre-fix
    /// direct-adjacency definition reported its 3 transitive pairs as
    /// "unchained" — a false signal on correct data, falsified live); a
    /// chain through an UNTYPED middle fact still connects its typed
    /// endpoints; only a genuinely unconnected state fact counts.
    #[test]
    fn unchained_counts_path_disconnected_pairs_only() {
        let chain4 = |vals: [&str; 4]| -> Vec<FactImport> {
            vals.iter()
                .enumerate()
                .map(|(i, v)| {
                    let mut f = typed_fact(
                        &format!("s{}", i + 1),
                        "gt",
                        &format!("ch-{}", i + 1),
                        "lucy",
                        "life-status",
                        at(v),
                    );
                    if i > 0 {
                        f.supersedes_in_frame = Some(format!("s{i}"));
                    }
                    f
                })
                .collect()
        };
        let order = chain(&["ch-1", "ch-2", "ch-3", "ch-4"]);
        let rules = [transition_rule(
            "life",
            "life-status",
            &[
                ("alive", "dead"),
                ("dead", "undead"),
                ("undead", "destroyed"),
            ],
        )];
        // Fully chained correct arc: zero unchained, zero violations.
        let store = store_with(chain4(["alive", "dead", "undead", "destroyed"]));
        let report = scan_continuity(&store, &order, &rules).unwrap();
        assert!(report.violations.is_empty(), "{:?}", report.violations);
        assert_eq!(report.unchained_state_pairs, 0);
        // Untyped middle: s1(typed) <- m(untyped) <- s3(typed) still
        // connects the endpoints through the chain (the hops are outside
        // the rule — partial coverage — but the pair is not unchained).
        let mut middle = fact("m", "gt", "ch-2", None);
        middle.entities = vec!["lucy".to_string()];
        middle.supersedes_in_frame = Some("s1".to_string());
        let mut s3 = typed_fact("s3", "gt", "ch-3", "lucy", "life-status", at("dead"));
        s3.supersedes_in_frame = Some("m".to_string());
        let store = store_with(vec![
            typed_fact("s1", "gt", "ch-1", "lucy", "life-status", at("alive")),
            middle,
            s3,
            // Genuinely unconnected same-subject state fact: the only pair
            // class that counts.
            typed_fact("loose", "gt", "ch-4", "lucy", "life-status", at("undead")),
        ]);
        let report = scan_continuity(&store, &order, &rules).unwrap();
        // (s1,s3) path-connected through m -> not counted; (s1,loose),
        // (s3,loose) disconnected -> 2.
        assert_eq!(report.unchained_state_pairs, 2);
    }

    // ---- dramatic-irony intervals (Round 455, design sec 7.14) ----

    /// The R454 spike's headline insight as a regression: an uncorrected
    /// false belief co-holds with the truth all the way to the world-line
    /// end — the window is OPEN.
    #[test]
    fn irony_window_open_at_world_line_end() {
        let truth = fact("ft", "gt", "ch-2", None);
        let mut belief = fact("fb", "daniel", "ch-2", None);
        belief.conflicts_with = vec!["ft".to_string()];
        let store = store_with(vec![belief, truth]);
        let report = irony_intervals(&store, &chain(&["ch-1", "ch-2", "ch-3", "ch-4"])).unwrap();
        assert_eq!(report.cross_frame_edges, 1);
        assert_eq!(report.same_frame_edges, 0);
        let main = &report.worlds[MAIN_BRANCH];
        assert_eq!(main.windows.len(), 1);
        let w = &main.windows[0];
        assert_eq!((w.fact_a.as_str(), w.fact_b.as_str()), ("fb", "ft"));
        assert_eq!((w.frame_a.as_str(), w.frame_b.as_str()), ("daniel", "gt"));
        assert_eq!(w.nodes, ["ch-2", "ch-3", "ch-4"]);
        assert_eq!(w.starts, ["ch-2"]);
        assert!(w.open, "uncorrected divergence must report open");
    }

    /// Succession closes the window (the half-open cut: the superseded
    /// truth stops holding AT its successor's node, so the last co-hold
    /// node is the one before).
    #[test]
    fn irony_window_closed_by_succession() {
        let truth = fact("ft", "gt", "ch-2", None);
        let mut revised = fact("fz", "gt", "ch-3", None);
        revised.supersedes_in_frame = Some("ft".to_string());
        let mut belief = fact("fb", "daniel", "ch-2", None);
        belief.conflicts_with = vec!["ft".to_string()];
        let store = store_with(vec![belief, truth, revised]);
        let report = irony_intervals(&store, &chain(&["ch-1", "ch-2", "ch-3", "ch-4"])).unwrap();
        let w = &report.worlds[MAIN_BRANCH].windows[0];
        assert_eq!(w.nodes, ["ch-2"]);
        assert!(!w.open, "a closed divergence must not report open");
    }

    /// Same-frame edges are the continuity gate's territory
    /// (`frame_conflict_overlap`) — skipped here, surfaced as a count.
    #[test]
    fn irony_skips_same_frame_edges_counted() {
        let a = fact("fa", "gt", "ch-1", Some("ch-2"));
        let mut b = fact("fb", "gt", "ch-3", None);
        b.conflicts_with = vec!["fa".to_string()];
        let store = store_with(vec![a, b]);
        let report = irony_intervals(&store, &chain(&["ch-1", "ch-2", "ch-3", "ch-4"])).unwrap();
        assert_eq!(report.cross_frame_edges, 0);
        assert_eq!(report.same_frame_edges, 1);
        assert!(report.worlds[MAIN_BRANCH].windows.is_empty());
        assert!(report.worlds[MAIN_BRANCH].windowless.is_empty());
    }

    /// A belief corrected BEFORE the truth lands never co-holds with it:
    /// both endpoints visible, no window — surfaced as windowless (data,
    /// not absence).
    #[test]
    fn irony_windowless_when_belief_corrected_first() {
        let mut belief = fact("fb", "daniel", "ch-1", None);
        belief.conflicts_with = vec!["ft".to_string()];
        let mut corrected = fact("fc", "daniel", "ch-2", None);
        corrected.supersedes_in_frame = Some("fb".to_string());
        let truth = fact("ft", "gt", "ch-3", None);
        let store = store_with(vec![belief, corrected, truth]);
        let report = irony_intervals(&store, &chain(&["ch-1", "ch-2", "ch-3", "ch-4"])).unwrap();
        let main = &report.worlds[MAIN_BRANCH];
        assert!(main.windows.is_empty());
        assert_eq!(main.windowless.len(), 1);
        assert_eq!(main.windowless[0].fact_a, "fb");
        assert_eq!(main.windowless[0].fact_b, "ft");
    }

    /// World scoping: an inherited window reports in BOTH the parent
    /// world and the inheriting fork (the world is part of the finding —
    /// the R450 per-world contract); a sibling fork cut BEFORE the facts
    /// has an Out endpoint and is not that world's business.
    #[test]
    fn irony_windows_are_world_scoped() {
        let truth = fact("ft", "gt", "ch-2", None);
        let mut belief = fact("fb", "daniel", "ch-2", None);
        belief.conflicts_with = vec!["ft".to_string()];
        let store = store_with_forks(
            vec![belief, truth],
            &[("w1", MAIN_BRANCH, "ch-2"), ("w2", MAIN_BRANCH, "ch-1")],
        );
        let report = irony_intervals(&store, &chain(&["ch-1", "ch-2", "ch-3", "ch-4"])).unwrap();
        assert_eq!(report.worlds[MAIN_BRANCH].windows.len(), 1);
        assert_eq!(report.worlds["w1"].windows.len(), 1, "inherited window");
        let w2 = &report.worlds["w2"];
        assert!(
            w2.windows.is_empty()
                && w2.windowless.is_empty()
                && w2.unordered.is_empty()
                && w2.undecidable.is_empty(),
            "pre-fork sibling sees neither endpoint"
        );
    }

    /// An endpoint the declared order cannot place against the fork cut
    /// is `Unknown` there — the edge surfaces as undecidable in that
    /// world (B-1), never classified either way.
    #[test]
    fn irony_undecidable_under_incomparable_fork_cut() {
        let order = CanonOrder::from_edges(&[
            ["ch-1".to_string(), "ch-2".to_string()],
            ["ch-2".to_string(), "ch-3".to_string()],
            ["k-1".to_string(), "k-2".to_string()],
        ])
        .unwrap();
        let truth = fact("ft", "gt", "ch-2", None);
        let mut belief = fact("fb", "daniel", "k-1", None);
        belief.conflicts_with = vec!["ft".to_string()];
        let store = store_with_forks(vec![belief, truth], &[("w1", MAIN_BRANCH, "ch-2")]);
        let report = irony_intervals(&store, &order).unwrap();
        // main: both In, but the parallel-chain starts are incomparable —
        // UNORDERED, not windowless (Round 456: the declaration cannot
        // order the pair; asserting "never overlaps" would overstate).
        let main = &report.worlds[MAIN_BRANCH];
        assert_eq!(main.unordered.len(), 1);
        assert!(main.windowless.is_empty());
        // w1: the belief's k-1 start is incomparable with the ch-2 cut.
        let w1 = &report.worlds["w1"];
        assert_eq!(w1.undecidable.len(), 1);
        assert_eq!(w1.undecidable[0].fact_a, "fb");
        assert!(w1.windows.is_empty() && w1.windowless.is_empty());
    }

    // ---- typing candidates (Round 458, design sec 7.15 Round A) ----

    /// The input-package contract: untyped facts only (id-sorted), each
    /// carrying the claim sha256 the eventual proposal must stamp (the
    /// R439 pin), plus the registries verbatim.
    #[test]
    fn typing_candidates_lists_untyped_with_claim_pin_and_vocabulary() {
        let typed = typed_fact("ft", "gt", "ch-1", "lucy", "life-status", at("alive"));
        let plain_b = fact("fb", "gt", "ch-2", None);
        let plain_a = fact("fa", "daniel", "ch-1", None);
        let store = store_with(vec![typed, plain_b, plain_a]);
        let report = typing_candidates(&store).unwrap();
        assert_eq!(report.facts, 3);
        assert_eq!(report.typed, 1);
        let ids: Vec<&str> = report
            .candidates
            .iter()
            .map(|c| c.fact_id.as_str())
            .collect();
        assert_eq!(ids, ["fa", "fb"], "untyped only, id-sorted");
        assert_eq!(
            report.candidates[0].claim_sha256,
            claim_sha256_hex("claim fa"),
            "the R439 pin the proposal must stamp"
        );
        assert!(
            report.predicates.contains_key("life-status"),
            "the 4th registry rides verbatim"
        );
        assert!(report.entities.contains_key("lucy"));
    }

    /// Order-independence is the contract: no canon declaration exists,
    /// the report still runs (boundary's declaration-side checks are
    /// vacuous under the empty order; fact-side re-checks still apply).
    #[test]
    fn typing_candidates_needs_no_canon_order() {
        let store = store_with(vec![fact("fa", "gt", "ch-1", None)]);
        let report = typing_candidates(&store).unwrap();
        assert_eq!(report.candidates.len(), 1);
    }

    /// Sections outside the declared order are isolated coordinates, not
    /// world-line ends: a window held only there must not report open
    /// (Round 456 — pins the `CanonOrder::is_maximal` boundary).
    #[test]
    fn irony_window_on_isolated_section_is_not_open() {
        // ch-4 exists as a section but the order declares only ch-1..ch-3.
        let truth = fact("ft", "gt", "ch-4", None);
        let mut belief = fact("fb", "daniel", "ch-4", None);
        belief.conflicts_with = vec!["ft".to_string()];
        let store = store_with(vec![belief, truth]);
        let report = irony_intervals(&store, &chain(&["ch-1", "ch-2", "ch-3"])).unwrap();
        let w = &report.worlds[MAIN_BRANCH].windows[0];
        assert_eq!(w.nodes, ["ch-4"]);
        assert!(!w.open, "an isolated coordinate is not a world-line end");
    }

    // ---- edge candidates (Round 462, design sec 7.16 Round A) ----

    /// The input-package contract: every fact row carries the claim text +
    /// sha256 pin (two-sided proposal stamping) and EVERY recorded edge —
    /// the proposer must never re-propose existing structure.
    #[test]
    fn edge_candidates_rows_carry_pins_and_all_recorded_edges() {
        let a = fact("fa", "gt", "ch-1", None);
        let mut b = fact("fb", "gt", "ch-2", None);
        b.supersedes_in_frame = Some("fa".to_string());
        let mut c = fact("fc", "gt", "ch-2", None);
        c.conflicts_with = vec!["fa".to_string()];
        c.pays_off = vec!["fa".to_string()];
        let store = store_with(vec![a, b, c]);
        let report = edge_candidates(&store, &chain(&["ch-1", "ch-2"])).unwrap();
        assert_eq!(report.fact_count, 3);
        assert_eq!(report.succession_edges, 1);
        assert_eq!(report.conflict_pairs, 1);
        let ids: Vec<&str> = report.facts.iter().map(|f| f.fact_id.as_str()).collect();
        assert_eq!(ids, ["fa", "fb", "fc"], "id-sorted");
        assert_eq!(
            report.facts[0].claim_sha256,
            claim_sha256_hex("claim fa"),
            "the pin a proposal must stamp"
        );
        assert_eq!(report.facts[1].supersedes_in_frame.as_deref(), Some("fa"));
        assert_eq!(report.facts[2].conflicts_with, ["fa"]);
        assert_eq!(report.facts[2].pays_off, ["fa"]);
    }

    /// The hint contract: a same-frame same-(predicate, subject) pair with
    /// no succession path is a gap; chaining it removes the gap.
    #[test]
    fn succession_gap_detected_then_closed_by_the_edge() {
        let a = typed_fact("fa", "gt", "ch-1", "todd", "life-status", at("alive"));
        let b = typed_fact("fb", "gt", "ch-2", "todd", "life-status", at("dead"));
        let store = store_with(vec![a.clone(), b.clone()]);
        let report = edge_candidates(&store, &chain(&["ch-1", "ch-2"])).unwrap();
        assert_eq!(report.succession_gaps.len(), 1);
        let gap = &report.succession_gaps[0];
        assert_eq!((gap.fact_a.as_str(), gap.fact_b.as_str()), ("fa", "fb"));
        assert_eq!(gap.predicate, "life-status");
        assert_eq!(gap.subject, "todd");
        let mut chained = b;
        chained.supersedes_in_frame = Some("fa".to_string());
        let store = store_with(vec![a, chained]);
        let report = edge_candidates(&store, &chain(&["ch-1", "ch-2"])).unwrap();
        assert!(report.succession_gaps.is_empty(), "the edge closes the gap");
    }

    /// Path, not direct edge (the Round 452 unchained semantics mirrored):
    /// a correctly chained A→B→C arc transitively connects (A, C).
    #[test]
    fn succession_gap_respects_transitive_chains() {
        let a = typed_fact("fa", "gt", "ch-1", "todd", "life-status", at("alive"));
        let mut b = typed_fact("fb", "gt", "ch-2", "todd", "life-status", at("wounded"));
        b.supersedes_in_frame = Some("fa".to_string());
        let mut c = typed_fact("fc", "gt", "ch-3", "todd", "life-status", at("dead"));
        c.supersedes_in_frame = Some("fb".to_string());
        let store = store_with(vec![a, b, c]);
        let report = edge_candidates(&store, &chain(&["ch-1", "ch-2", "ch-3"])).unwrap();
        assert!(report.succession_gaps.is_empty(), "chained arc has no gap");
    }

    /// Cross-frame pairs and different-(predicate, subject) pairs are not
    /// gaps — succession is in-frame, and state comparability needs the
    /// same typed key. Untyped facts never hint (no machine state key).
    #[test]
    fn succession_gap_scope_boundaries() {
        let a = typed_fact("fa", "gt", "ch-1", "todd", "life-status", at("alive"));
        let cross = typed_fact("fb", "kara", "ch-2", "todd", "life-status", at("dead"));
        let other_subj = typed_fact("fc", "gt", "ch-2", "alice", "life-status", at("dead"));
        let other_pred = typed_fact("fd", "gt", "ch-2", "todd", "deviancy", at("deviant"));
        let untyped = fact("fe", "gt", "ch-2", None);
        let store = store_with(vec![a, cross, other_subj, other_pred, untyped]);
        let report = edge_candidates(&store, &chain(&["ch-1", "ch-2"])).unwrap();
        assert!(report.succession_gaps.is_empty());
    }

    /// A pair co-visible in two worlds (fork lineage) hints once — the
    /// dedup the unchained count applies, mirrored.
    #[test]
    fn succession_gap_deduplicated_across_worlds() {
        let a = typed_fact("fa", "gt", "ch-1", "todd", "life-status", at("alive"));
        let b = typed_fact("fb", "gt", "ch-1", "todd", "life-status", at("dead"));
        let store = store_with_forks(vec![a, b], &[("w1", MAIN_BRANCH, "ch-2")]);
        let report = edge_candidates(&store, &chain(&["ch-1", "ch-2"])).unwrap();
        assert_eq!(
            report.succession_gaps.len(),
            1,
            "visible in main AND w1, one hint"
        );
    }

    /// Without a declared canon order the facts table stays complete and
    /// same-branch hints still fire (visibility inside one branch needs no
    /// order); only fork-inheritance visibility degrades.
    #[test]
    fn edge_candidates_facts_complete_without_order() {
        let a = typed_fact("fa", "gt", "ch-1", "todd", "life-status", at("alive"));
        let b = typed_fact("fb", "gt", "ch-2", "todd", "life-status", at("dead"));
        let store = store_with(vec![a, b]);
        let report = edge_candidates(&store, &CanonOrder::empty()).unwrap();
        assert_eq!(report.fact_count, 2, "facts table never degrades");
        assert_eq!(
            report.succession_gaps.len(),
            1,
            "same-branch co-visibility needs no declared order"
        );
    }

    /// An out-of-band-planted succession cycle is a VIOLATION, reported
    /// once per cycle (Round 463 — before the shared write guard landed,
    /// the R461 probe showed a cyclic store scanning at 0 violations while
    /// its facts silently never held anywhere).
    #[test]
    fn out_of_band_succession_cycle_is_a_violation_reported_once() {
        let a = fact("fa", "gt", "ch-1", None);
        let mut b = fact("fb", "gt", "ch-2", None);
        b.supersedes_in_frame = Some("fa".to_string());
        let mut store = store_with(vec![a, b]);
        // Close the loop out-of-band (the write paths reject this).
        store
            .narrative_facts
            .get_mut("fa")
            .unwrap()
            .supersedes_in_frame = Some("fb".to_string());
        let report = scan_continuity(&store, &chain(&["ch-1", "ch-2"]), &[]).unwrap();
        let cycles: Vec<_> = report
            .violations
            .iter()
            .filter(|v| matches!(v, ContinuityViolation::SuccessionCycle { .. }))
            .collect();
        assert_eq!(cycles.len(), 1, "one cycle, one violation: {cycles:?}");
        match cycles[0] {
            ContinuityViolation::SuccessionCycle { cycle } => {
                assert_eq!(cycle, &["fa", "fb"], "anchored at the minimum member");
            }
            _ => unreachable!(),
        }
    }

    // ---- playthrough manuscript (Round 466, design sec 7.17) ----

    /// Deterministic topological walk: a diamond linearizes smallest-first
    /// among ready nodes, and the incomparable emitted neighbors surface
    /// as an undeclared adjacency (one valid reading, never the only one).
    #[test]
    fn manuscript_diamond_linearizes_and_surfaces_undeclared_adjacency() {
        let order = CanonOrder::from_edges(&[
            ["ch-1".to_string(), "ch-2".to_string()],
            ["ch-1".to_string(), "ch-3".to_string()],
            ["ch-2".to_string(), "ch-4".to_string()],
            ["ch-3".to_string(), "ch-4".to_string()],
        ])
        .unwrap();
        assert_eq!(
            order.linearize(MAIN_BRANCH),
            ["ch-1", "ch-2", "ch-3", "ch-4"]
        );
        let store = store_with(vec![fact("fa", "gt", "ch-1", None)]);
        let report = playthrough_manuscript(&store, &order, None, None).unwrap();
        let main = &report.worlds[MAIN_BRANCH];
        assert_eq!(
            main.undeclared_adjacencies,
            vec![["ch-2".to_string(), "ch-3".to_string()]],
            "the diamond's incomparable middle is surfaced, not silently totalized"
        );
        assert_eq!(main.scenes.len(), 4);
    }

    /// Scene events are declared coordinates verbatim: begins at
    /// `canon_from`, expired at `canon_to` (still holding AT it), and a
    /// supersession ends the predecessor at the successor's `canon_from`,
    /// naming the cutting fact. `holding_count` is the holds_at judgment.
    #[test]
    fn manuscript_places_begins_ends_and_holding_counts() {
        let f1 = fact("f1", "gt", "ch-1", None);
        let f2 = fact("f2", "gt", "ch-1", Some("ch-2"));
        let mut f3 = fact("f3", "gt", "ch-3", None);
        f3.supersedes_in_frame = Some("f1".to_string());
        let store = store_with(vec![f1, f2, f3]);
        let order = chain(&["ch-1", "ch-2", "ch-3", "ch-4"]);
        let report = playthrough_manuscript(&store, &order, None, None).unwrap();
        let main = &report.worlds[MAIN_BRANCH];
        assert!(main.unplaced_facts.is_empty() && main.undecidable.is_empty());
        let s = &main.scenes;
        assert_eq!(s[0].section, "ch-1");
        let begins: Vec<&str> = s[0].begins.iter().map(|e| e.fact_id.as_str()).collect();
        assert_eq!(begins, ["f1", "f2"]);
        assert_eq!(s[0].holding_count, 2);
        // ch-2: f2 expires here — it still holds AT ch-2, through it.
        assert_eq!(s[1].ends.len(), 1);
        assert_eq!(s[1].ends[0].fact_id, "f2");
        assert_eq!(s[1].ends[0].kind, ManuscriptEndKind::Expired);
        assert_eq!(s[1].holding_count, 2);
        // ch-3: f3 begins and cuts f1 — the end event names the successor.
        assert_eq!(s[2].begins[0].fact_id, "f3");
        assert_eq!(s[2].ends[0].fact_id, "f1");
        assert_eq!(s[2].ends[0].kind, ManuscriptEndKind::Superseded);
        assert_eq!(s[2].ends[0].by.as_deref(), Some("f3"));
        assert_eq!(s[2].holding_count, 1, "f1 cut, f2 expired — f3 alone");
    }

    /// A visible fact whose coordinate the order never names emits no
    /// event — surfaced as unplaced beside the outside-order sections,
    /// never silently dropped (B-1, no silent caps).
    #[test]
    fn manuscript_surfaces_unplaced_facts_and_outside_order_sections() {
        let store = store_with(vec![fact("f-out", "gt", "ch-3", None)]);
        let order = chain(&["ch-1", "ch-2"]);
        let report = playthrough_manuscript(&store, &order, None, None).unwrap();
        let main = &report.worlds[MAIN_BRANCH];
        assert_eq!(main.scenes.len(), 2);
        assert_eq!(main.unplaced_facts.len(), 1);
        assert_eq!(main.unplaced_facts[0].fact_id, "f-out");
        assert_eq!(main.unplaced_facts[0].field, "canon_from");
        assert_eq!(main.unplaced_facts[0].coordinate, "ch-3");
        assert_eq!(
            main.sections_outside_order,
            ["ch-3".to_string(), "ch-4".to_string()]
        );
        assert!(main.scenes.iter().all(|s| s.begins.is_empty()));
    }

    /// World scoping (Round 438 carried): a fork's revision cuts the
    /// inherited fact in the fork's own manuscript only — the ancestor's
    /// manuscript never sees the fork's end event.
    #[test]
    fn manuscript_fork_supersession_stays_in_the_fork_world() {
        let f_main = fact("f-main", "gt", "ch-1", None);
        let mut f_rev = branch_fact("f-rev", "gt", "route", "ch-3");
        f_rev.supersedes_in_frame = Some("f-main".to_string());
        let store = store_with_forks(vec![f_main, f_rev], &[("route", MAIN_BRANCH, "ch-2")]);
        let order = chain(&["ch-1", "ch-2", "ch-3", "ch-4"]);
        let report = playthrough_manuscript(&store, &order, None, None).unwrap();
        let main = &report.worlds[MAIN_BRANCH];
        assert!(
            main.scenes.iter().all(|s| s.ends.is_empty()),
            "the fork's revision never leaks into the ancestor's manuscript"
        );
        assert!(main.scenes[2].holding_count == 1, "f-main holds on in main");
        let route = &report.worlds["route"];
        let ch3 = route.scenes.iter().find(|s| s.section == "ch-3").unwrap();
        assert_eq!(ch3.ends[0].fact_id, "f-main");
        assert_eq!(ch3.ends[0].by.as_deref(), Some("f-rev"));
        assert_eq!(ch3.begins[0].fact_id, "f-rev");
    }

    /// The `world` filter is the consumption unit (one manuscript per
    /// reading session): a registered id narrows the sweep; a typo fails
    /// loud instead of reading as an empty manuscript.
    #[test]
    fn manuscript_world_filter_narrows_and_fails_loud() {
        let store = store_with_forks(
            vec![fact("fa", "gt", "ch-1", None)],
            &[("route", MAIN_BRANCH, "ch-2")],
        );
        let order = chain(&["ch-1", "ch-2"]);
        let all = playthrough_manuscript(&store, &order, None, None).unwrap();
        assert_eq!(all.worlds.len(), 2);
        let one = playthrough_manuscript(&store, &order, Some("route"), None).unwrap();
        assert_eq!(one.worlds.len(), 1);
        assert!(one.worlds.contains_key("route"));
        let err = playthrough_manuscript(&store, &order, Some("nope"), None).unwrap_err();
        assert!(err.contains("branch registry"), "{err}");
    }

    /// B-1: a fact the fork comparison cannot place is undecidable —
    /// listed, never placed as an event, never counted holding (the
    /// irony-report idiom carried).
    #[test]
    fn manuscript_undecidable_under_incomparable_fork_cut() {
        let order = CanonOrder::from_edges(&[
            ["ch-1".to_string(), "ch-2".to_string()],
            ["k-1".to_string(), "k-2".to_string()],
        ])
        .unwrap();
        let parallel = fact("fk", "gt", "k-1", None);
        let store = store_with_forks(vec![parallel], &[("w1", MAIN_BRANCH, "ch-2")]);
        let report = playthrough_manuscript(&store, &order, None, None).unwrap();
        let w1 = &report.worlds["w1"];
        assert_eq!(w1.undecidable, ["fk"]);
        assert!(w1.scenes.iter().all(|s| s.begins.is_empty()));
        assert!(w1.scenes.iter().all(|s| s.holding_count == 0));
    }

    /// Round 506 — the --telling render-brief carrier: begins-events carry the
    /// per-fact disclosure decision under the named telling (an override wins,
    /// else the plan default); first_at is per-world-line; a missing telling
    /// fails loud; without a telling the field stays None (byte-stable).
    #[test]
    fn manuscript_telling_carrier_annotates_begins() {
        let mut store = store_with_forks(
            vec![
                fact("f-main", "gt", "ch-1", None),
                branch_fact("f-rev", "gt", "route", "ch-3"),
            ],
            &[("route", MAIN_BRANCH, "ch-2")],
        );
        let mut first_at = BTreeMap::new();
        first_at.insert("route".to_string(), "ch-3".to_string());
        let mut overrides = BTreeMap::new();
        overrides.insert(
            "f-main".to_string(),
            mnemosyne_core::DisclosureOverride {
                mode: mnemosyne_core::DisclosureMode::State,
                first_at,
                surface: Some(mnemosyne_core::DisclosureSurface {
                    scene: "ch-2".to_string(),
                    object: Some("clock".to_string()),
                }),
            },
        );
        store.disclosure_plans.insert(
            "t1".to_string(),
            mnemosyne_core::DisclosurePlan {
                description: String::new(),
                default_mode: mnemosyne_core::DisclosureMode::Withhold,
                overrides,
            },
        );
        let order = chain(&["ch-1", "ch-2", "ch-3", "ch-4"]);

        // No telling → no disclosure annotation.
        let plain = playthrough_manuscript(&store, &order, Some(MAIN_BRANCH), None).unwrap();
        let plain_ev = plain.worlds[MAIN_BRANCH]
            .scenes
            .iter()
            .flat_map(|s| &s.begins)
            .find(|e| e.fact_id == "f-main")
            .unwrap();
        assert!(plain_ev.disclosure.is_none());

        // With the telling: f-main overridden (state); no first_at for main;
        // the surface rides through verbatim.
        let main = playthrough_manuscript(&store, &order, Some(MAIN_BRANCH), Some("t1")).unwrap();
        let ev = main.worlds[MAIN_BRANCH]
            .scenes
            .iter()
            .flat_map(|s| &s.begins)
            .find(|e| e.fact_id == "f-main")
            .unwrap();
        let d = ev.disclosure.as_ref().unwrap();
        assert_eq!(d.mode, mnemosyne_core::DisclosureMode::State);
        assert_eq!(d.first_at, None, "no first_at pinned for the main world");
        let surface = d.surface.as_ref().unwrap();
        assert_eq!(surface.scene, "ch-2");
        assert_eq!(surface.object.as_deref(), Some("clock"));

        // The route world resolves f-main's per-world-line first_at, and
        // f-rev (no override) falls to the plan default (withhold).
        let route = playthrough_manuscript(&store, &order, Some("route"), Some("t1")).unwrap();
        let route_begins: Vec<&ManuscriptFactEvent> = route.worlds["route"]
            .scenes
            .iter()
            .flat_map(|s| &s.begins)
            .collect();
        let f_main = route_begins.iter().find(|e| e.fact_id == "f-main").unwrap();
        assert_eq!(
            f_main.disclosure.as_ref().unwrap().first_at.as_deref(),
            Some("ch-3")
        );
        let f_rev = route_begins.iter().find(|e| e.fact_id == "f-rev").unwrap();
        assert_eq!(
            f_rev.disclosure.as_ref().unwrap().mode,
            mnemosyne_core::DisclosureMode::Withhold
        );
        assert_eq!(f_rev.disclosure.as_ref().unwrap().first_at, None);

        // A typo'd telling fails loud (the registry ethos).
        let err = playthrough_manuscript(&store, &order, None, Some("nope")).unwrap_err();
        assert!(err.contains("disclosure_plans registry"), "{err}");
    }

    // ====================================================================
    // Round 556/557 — playable-world projection (the map_locator seam, sec 7.37).
    // ====================================================================

    /// The playable-world JOIN: each authored disclosure `surface` becomes a
    /// per-world [`MapLocator`] whose `scene_ordinal` indexes the world's scene
    /// walk; the fork topology rides along; a fact disclosed without a surface
    /// (the plan default) yields no locator.
    #[test]
    fn playable_world_resolves_surface_locators_per_world() {
        let mut store = store_with_forks(
            vec![
                fact("f-main", "gt", "ch-1", None),
                branch_fact("f-rev", "gt", "route", "ch-3"),
            ],
            &[("route", MAIN_BRANCH, "ch-2")],
        );
        let mut first_at = BTreeMap::new();
        first_at.insert("route".to_string(), "ch-3".to_string());
        let mut overrides = BTreeMap::new();
        overrides.insert(
            "f-main".to_string(),
            mnemosyne_core::DisclosureOverride {
                mode: mnemosyne_core::DisclosureMode::State,
                first_at,
                surface: Some(mnemosyne_core::DisclosureSurface {
                    scene: "ch-2".to_string(),
                    object: Some("clock".to_string()),
                }),
            },
        );
        store.disclosure_plans.insert(
            "t1".to_string(),
            mnemosyne_core::DisclosurePlan {
                description: String::new(),
                default_mode: mnemosyne_core::DisclosureMode::Withhold,
                overrides,
            },
        );
        let order = chain(&["ch-1", "ch-2", "ch-3", "ch-4"]);

        let report = playable_world(&store, &order, None, "t1").unwrap();
        assert_eq!(report.telling, "t1");
        // The fork topology rides along: route forks from main at ch-2.
        assert!(report
            .fork_tree
            .branches
            .iter()
            .any(|b| b.branch_id == "route"));

        // Main world: f-main's surface (ch-2) resolves to ordinal 1 in the walk
        // [ch-1, ch-2, ch-3, ch-4]; no per-world first_at pinned for main.
        let main = &report.worlds[MAIN_BRANCH];
        let walk: Vec<&str> = main
            .manuscript
            .scenes
            .iter()
            .map(|s| s.section.as_str())
            .collect();
        assert_eq!(walk, vec!["ch-1", "ch-2", "ch-3", "ch-4"]);
        assert_eq!(main.locators.len(), 1);
        let loc = &main.locators[0];
        assert_eq!(loc.fact_id, "f-main");
        assert_eq!(loc.scene, "ch-2");
        assert_eq!(loc.scene_ordinal, Some(1));
        assert_eq!(loc.object.as_deref(), Some("clock"));
        assert_eq!(loc.mode, mnemosyne_core::DisclosureMode::State);
        assert_eq!(loc.first_at, None);

        // Route world: f-main is visible (it begins at ch-1, before the ch-2
        // fork), so its locator carries the per-world first_at (ch-3); f-rev
        // falls to the plan default (withhold) with NO surface → no locator.
        let route = &report.worlds["route"];
        assert_eq!(route.locators.len(), 1);
        let route_loc = &route.locators[0];
        assert_eq!(route_loc.fact_id, "f-main");
        assert_eq!(route_loc.first_at.as_deref(), Some("ch-3"));
        assert_eq!(
            route_loc.scene_ordinal,
            route
                .manuscript
                .scenes
                .iter()
                .position(|s| s.section == "ch-2")
        );
    }

    /// R558 review fix: the playable surface reuses the manuscript VERBATIM, so
    /// the R466 B-1 honesty surfaces ride through — a diamond's incomparable
    /// middle is surfaced, never silently totalized into a false linear walk.
    #[test]
    fn playable_world_carries_manuscript_honesty_surfaces() {
        let order = CanonOrder::from_edges(&[
            ["ch-1".to_string(), "ch-2".to_string()],
            ["ch-1".to_string(), "ch-3".to_string()],
            ["ch-2".to_string(), "ch-4".to_string()],
            ["ch-3".to_string(), "ch-4".to_string()],
        ])
        .unwrap();
        let mut store = store_with(vec![fact("fa", "gt", "ch-1", None)]);
        store.disclosure_plans.insert(
            "t1".to_string(),
            mnemosyne_core::DisclosurePlan {
                description: String::new(),
                default_mode: mnemosyne_core::DisclosureMode::Withhold,
                overrides: BTreeMap::new(),
            },
        );
        let report = playable_world(&store, &order, None, "t1").unwrap();
        let main = &report.worlds[MAIN_BRANCH];
        assert_eq!(
            main.manuscript.undeclared_adjacencies,
            vec![["ch-2".to_string(), "ch-3".to_string()]],
            "the diamond's incomparable middle rides through, not silently totalized"
        );
        assert_eq!(main.manuscript.scenes.len(), 4);
    }

    /// A surface scene that is not a node of the world's walk resolves to
    /// `scene_ordinal = None` — surfaced, not silently dropped (the R466 idiom).
    #[test]
    fn playable_world_surfaces_unplaced_ordinal() {
        let mut store = store_with_forks(vec![fact("f-x", "gt", "ch-1", None)], &[]);
        let mut overrides = BTreeMap::new();
        overrides.insert(
            "f-x".to_string(),
            mnemosyne_core::DisclosureOverride {
                mode: mnemosyne_core::DisclosureMode::Hint,
                first_at: BTreeMap::new(),
                surface: Some(mnemosyne_core::DisclosureSurface {
                    scene: "ch-off".to_string(),
                    object: None,
                }),
            },
        );
        store.disclosure_plans.insert(
            "t1".to_string(),
            mnemosyne_core::DisclosurePlan {
                description: String::new(),
                default_mode: mnemosyne_core::DisclosureMode::Withhold,
                overrides,
            },
        );
        let order = chain(&["ch-1", "ch-2"]);

        let report = playable_world(&store, &order, None, "t1").unwrap();
        let main = &report.worlds[MAIN_BRANCH];
        assert_eq!(main.locators.len(), 1);
        assert_eq!(main.locators[0].scene, "ch-off");
        assert_eq!(main.locators[0].scene_ordinal, None);
    }

    // ====================================================================
    // Round 568 — quest graph (the fact→quest projection, design sec 7.38).
    // ====================================================================

    /// A `subject predicate object-entity` typed claim (the R559 quest contract
    /// vocabulary — all three quest predicates take an entity object).
    fn ent_claim(subject: &str, predicate: &str, object: &str) -> mnemosyne_core::TypedClaim {
        mnemosyne_core::TypedClaim {
            subject: subject.to_string(),
            predicate: predicate.to_string(),
            object: mnemosyne_core::TypedObject::Entity {
                id: object.to_string(),
            },
        }
    }

    /// A typed quest fact built from the R559 contract vocabulary.
    fn quest_fact(
        id: &str,
        from: &str,
        branch: Option<&str>,
        entities: &[&str],
        claim: mnemosyne_core::TypedClaim,
        pays_off: &[&str],
    ) -> FactImport {
        FactImport {
            entities: entities.iter().map(|s| s.to_string()).collect(),
            branch: branch.map(str::to_string),
            pays_off: pays_off.iter().map(|s| s.to_string()).collect(),
            typed: Some(claim),
            ..fact(id, "gt", from, None)
        }
    }

    /// A small dnd-shaped fixture: `q-main` (gated by `q-key`, completed only on
    /// the `win` road = per-road divergence), `q-key` (a pre-fork prerequisite,
    /// done on every road), and `q-orphan` (a quest with no `completed_by` =
    /// unresolved). `q-main`'s giving fact carries a giver surface under `t1`.
    fn quest_store() -> AtomicStore {
        let give_main = FactImport {
            payoff_expectation: Some("expected".to_string()),
            ..fact("f-give-main", "gt", "ch-1", None)
        };
        let give_key = FactImport {
            payoff_expectation: Some("expected".to_string()),
            ..fact("f-give-key", "gt", "ch-1", None)
        };
        let pursue_main = quest_fact(
            "f-pursue-main",
            "ch-1",
            None,
            &["hero", "q-main"],
            ent_claim("hero", "pursues", "q-main"),
            &[],
        );
        let pursue_key = quest_fact(
            "f-pursue-key",
            "ch-1",
            None,
            &["rogue", "q-key"],
            ent_claim("rogue", "pursues", "q-key"),
            &[],
        );
        let pursue_orphan = quest_fact(
            "f-pursue-orphan",
            "ch-1",
            None,
            &["hero", "q-orphan"],
            ent_claim("hero", "pursues", "q-orphan"),
            &[],
        );
        let require = quest_fact(
            "f-require",
            "ch-1",
            None,
            &["q-main", "q-key"],
            ent_claim("q-main", "requires", "q-key"),
            &[],
        );
        // q-key discharged pre-fork (ch-1) → done on every road.
        let complete_key = quest_fact(
            "f-complete-key",
            "ch-1",
            None,
            &["q-key", "rogue"],
            ent_claim("q-key", "completed_by", "rogue"),
            &["f-give-key"],
        );
        // q-main discharged only on the `win` road (post-fork ch-3) → open on
        // main, done on win = per-road divergence on data.
        let complete_main = quest_fact(
            "f-complete-main",
            "ch-3",
            Some("win"),
            &["q-main", "wizard"],
            ent_claim("q-main", "completed_by", "wizard"),
            &["f-give-main"],
        );
        let mut store = store_with_forks(
            vec![
                give_main,
                give_key,
                pursue_main,
                pursue_key,
                pursue_orphan,
                require,
                complete_key,
                complete_main,
            ],
            &[("win", MAIN_BRANCH, "ch-2")],
        );
        for (id, desc) in [
            ("q-main", "End the rising"),
            ("q-key", "Recover the warden's key"),
            ("q-orphan", "Find the lost ledger"),
        ] {
            let e = store.entities.get_mut(id).unwrap();
            e.kind = "quest".to_string();
            e.description = desc.to_string();
        }
        let mut overrides = BTreeMap::new();
        overrides.insert(
            "f-give-main".to_string(),
            mnemosyne_core::DisclosureOverride {
                mode: mnemosyne_core::DisclosureMode::State,
                first_at: BTreeMap::new(),
                surface: Some(mnemosyne_core::DisclosureSurface {
                    scene: "ch-1".to_string(),
                    object: Some("reeve-hall".to_string()),
                }),
            },
        );
        store.disclosure_plans.insert(
            "t1".to_string(),
            mnemosyne_core::DisclosurePlan {
                description: String::new(),
                default_mode: mnemosyne_core::DisclosureMode::Withhold,
                overrides,
            },
        );
        store
    }

    /// The quest-graph JOIN: objective/actor/prerequisite/giving from the typed
    /// claims, per-world open/done DERIVED from the R442 payoff coverage (a quest
    /// done on one road and open on another), the completing fact + discharger
    /// named per road, and the giver surface resolved to a per-world locator.
    #[test]
    fn quest_graph_derives_per_road_state_and_locators() {
        let store = quest_store();
        let order = chain(&["ch-1", "ch-2", "ch-3", "ch-4"]);
        let report = quest_graph(&store, &order, None, "t1").unwrap();

        assert_eq!(report.telling, "t1");
        assert_eq!(
            report.worlds,
            vec![MAIN_BRANCH.to_string(), "win".to_string()]
        );
        // The fork topology rides along (always full).
        assert!(report
            .fork_tree
            .branches
            .iter()
            .any(|b| b.branch_id == "win"));
        // Sorted by id: q-key, q-main, q-orphan.
        let ids: Vec<&str> = report.quests.iter().map(|q| q.quest_id.as_str()).collect();
        assert_eq!(ids, vec!["q-key", "q-main", "q-orphan"]);
        assert_eq!(report.unresolved_quests, vec!["q-orphan".to_string()]);

        let main_quest = report
            .quests
            .iter()
            .find(|q| q.quest_id == "q-main")
            .unwrap();
        assert_eq!(main_quest.objective, "End the rising");
        assert_eq!(main_quest.actors, vec!["hero".to_string()]);
        assert_eq!(main_quest.prerequisites, vec!["q-key".to_string()]);
        assert_eq!(main_quest.giving_facts, vec!["f-give-main".to_string()]);
        // Per-road divergence: open on main, done on win.
        assert_eq!(main_quest.per_world[MAIN_BRANCH].state, QuestState::Open);
        assert!(main_quest.per_world[MAIN_BRANCH].completions.is_empty());
        let win = &main_quest.per_world["win"];
        assert_eq!(win.state, QuestState::Done);
        assert_eq!(win.completions.len(), 1);
        assert_eq!(win.completions[0].fact, "f-complete-main");
        assert_eq!(win.completions[0].scene, "ch-3");
        assert_eq!(win.completions[0].actor.as_deref(), Some("wizard"));
        // Giver surface resolves to a locator on each world the giving fact rides.
        assert_eq!(main_quest.locators.len(), 2);
        assert!(main_quest
            .locators
            .iter()
            .all(|l| l.fact_id == "f-give-main" && l.object.as_deref() == Some("reeve-hall")));

        // The pre-fork prerequisite is done on every road.
        let key_quest = report
            .quests
            .iter()
            .find(|q| q.quest_id == "q-key")
            .unwrap();
        assert_eq!(key_quest.actors, vec!["rogue".to_string()]);
        assert!(key_quest.prerequisites.is_empty());
        assert_eq!(key_quest.per_world[MAIN_BRANCH].state, QuestState::Done);
        assert_eq!(key_quest.per_world["win"].state, QuestState::Done);

        // The orphan quest: no giving fact, all-unknown per world (surfaced).
        let orphan = report
            .quests
            .iter()
            .find(|q| q.quest_id == "q-orphan")
            .unwrap();
        assert!(orphan.giving_facts.is_empty());
        assert!(orphan
            .per_world
            .values()
            .all(|s| s.state == QuestState::Unknown));
    }

    /// `--world` scopes every `QuestNode.per_world` to the one road, but the
    /// fork tree stays full (the topology is inherently cross-world).
    #[test]
    fn quest_graph_world_filter_scopes_per_world_keeps_fork_tree_full() {
        let store = quest_store();
        let order = chain(&["ch-1", "ch-2", "ch-3", "ch-4"]);
        let report = quest_graph(&store, &order, Some("win"), "t1").unwrap();

        assert_eq!(report.worlds, vec!["win".to_string()]);
        assert!(report
            .fork_tree
            .branches
            .iter()
            .any(|b| b.branch_id == "win"));
        let main_quest = report
            .quests
            .iter()
            .find(|q| q.quest_id == "q-main")
            .unwrap();
        let keys: Vec<&str> = main_quest.per_world.keys().map(String::as_str).collect();
        assert_eq!(keys, vec!["win"]);
        assert_eq!(main_quest.per_world["win"].state, QuestState::Done);
    }

    /// R569 strict-combined contract: a quest whose `completed_by` fact carries
    /// NO `pays_off`, with a SIBLING fact at the same scene paying off the
    /// Expected giving, binds NOTHING — no scene-proximity rescue. The quest is
    /// surfaced as `unresolved` + all-`unknown`, never silently bound to a
    /// sibling's giving (the cross-quest-bleed the R568 fallback risked).
    #[test]
    fn quest_graph_split_completion_is_unresolved_not_scene_inferred() {
        let give = FactImport {
            payoff_expectation: Some("expected".to_string()),
            ..fact("f-give", "gt", "ch-1", None)
        };
        let pursue = quest_fact(
            "f-pursue",
            "ch-1",
            None,
            &["hero", "q-split"],
            ent_claim("hero", "pursues", "q-split"),
            &[],
        );
        // completed_by WITHOUT a pays_off edge (the split encoding).
        let complete = quest_fact(
            "f-complete",
            "ch-2",
            None,
            &["q-split", "hero"],
            ent_claim("q-split", "completed_by", "hero"),
            &[],
        );
        // a SIBLING at the same scene pays off the giving — the scene-proximity
        // bait the strict binding must NOT take.
        let sibling = FactImport {
            pays_off: vec!["f-give".to_string()],
            ..fact("f-sibling", "gt", "ch-2", None)
        };
        let mut store = store_with_forks(vec![give, pursue, complete, sibling], &[]);
        let e = store.entities.get_mut("q-split").unwrap();
        e.kind = "quest".to_string();
        e.description = "Split completion".to_string();
        store.disclosure_plans.insert(
            "t1".to_string(),
            mnemosyne_core::DisclosurePlan {
                description: String::new(),
                default_mode: mnemosyne_core::DisclosureMode::Withhold,
                overrides: BTreeMap::new(),
            },
        );
        let order = chain(&["ch-1", "ch-2", "ch-3", "ch-4"]);
        let report = quest_graph(&store, &order, None, "t1").unwrap();

        assert_eq!(report.unresolved_quests, vec!["q-split".to_string()]);
        let q = report
            .quests
            .iter()
            .find(|q| q.quest_id == "q-split")
            .unwrap();
        assert!(q.giving_facts.is_empty(), "no scene-proximity rescue");
        assert!(q.per_world.values().all(|s| s.state == QuestState::Unknown));
    }

    // ====================================================================
    // Round 497 — fork tree (the cross-world choice graph, design sec 7.21).
    // ====================================================================

    /// The choice graph: a placed fork (its point is a node of the parent's
    /// order), a standalone world, and an UNPLACED fork (its point is a
    /// section the parent's order never names — surfaced, never dropped, the
    /// R466 idiom).
    #[test]
    fn fork_tree_projects_forks_standalone_and_unplaced() {
        let store = store_with_forks(
            vec![
                fact("f-main", "gt", "ch-1", None),
                branch_fact("f-solo", "gt", "solo", "k-1"),
            ],
            // route forks on the main chain (placed); side forks at k-2,
            // which the ch chain never names (unplaced).
            &[("route", MAIN_BRANCH, "ch-2"), ("side", MAIN_BRANCH, "k-2")],
        );
        let order = chain(&["ch-1", "ch-2", "ch-3", "ch-4"]);
        let report = fork_tree(&store, &order).unwrap();

        assert_eq!(report.branch_count, 3); // route, side, solo (id-sorted)
        let by_id = |id: &str| report.branches.iter().find(|b| b.branch_id == id).unwrap();

        let route = by_id("route").fork.as_ref().unwrap();
        assert_eq!(route.parent, MAIN_BRANCH);
        assert_eq!(route.at, "ch-2");
        assert!(route.at_placed, "ch-2 is a node of main's order");

        let side = by_id("side").fork.as_ref().unwrap();
        assert_eq!(side.at, "k-2");
        assert!(!side.at_placed, "k-2 is not named by the ch chain");
        assert_eq!(report.unplaced_fork_points, ["side"]);

        assert!(
            by_id("solo").fork.is_none(),
            "a standalone world has no fork point"
        );
    }

    /// The branch description is the CYOA choice label — emitted verbatim.
    #[test]
    fn fork_tree_emits_choice_label_description() {
        let mut store = AtomicStore::new();
        for s in ["s1", "s2"] {
            store
                .sections
                .insert(s.to_string(), AtomicSection::default());
        }
        store.branches.insert(
            "alt".to_string(),
            mnemosyne_core::Branch {
                description: "take the side door".to_string(),
                forks_from: Some(mnemosyne_core::BranchFork {
                    branch: MAIN_BRANCH.to_string(),
                    at: "s1".to_string(),
                }),
                converges_from: vec![],
            },
        );
        let order = chain(&["s1", "s2"]);
        let report = fork_tree(&store, &order).unwrap();
        assert_eq!(report.branches[0].description, "take the side door");
        assert!(report.branches[0].fork.as_ref().unwrap().at_placed);
    }

    /// A fork whose parent is neither `main` nor registered fails loud — a
    /// typo'd parent must not read as a silent root (the write path forbids
    /// this; the read surface guards the out-of-band edit).
    #[test]
    fn fork_tree_fails_loud_on_unregistered_parent() {
        let mut store = AtomicStore::new();
        store.branches.insert(
            "child".to_string(),
            mnemosyne_core::Branch {
                description: String::new(),
                forks_from: Some(mnemosyne_core::BranchFork {
                    branch: "ghost".to_string(),
                    at: "s1".to_string(),
                }),
                converges_from: vec![],
            },
        );
        let err = fork_tree(&store, &CanonOrder::empty()).unwrap_err();
        assert!(err.contains("neither `main` nor a registered"), "{err}");
    }

    /// Round 532 — the fork tree SURFACES a confluence's incoming merges, the
    /// edge a fork tree alone could never declare (R531: "convergence is
    /// expressed, not declared"). Two forked world-lines converge into a shared
    /// continuation; the merge is now structurally visible with its parents and
    /// merge coordinates resolved.
    #[test]
    fn fork_tree_surfaces_confluence_merges() {
        let mut store = AtomicStore::new();
        for s in ["s1", "s2", "s3"] {
            store
                .sections
                .insert(s.to_string(), AtomicSection::default());
        }
        for b in ["sluice", "ride"] {
            store.branches.insert(
                b.to_string(),
                mnemosyne_core::Branch {
                    description: String::new(),
                    forks_from: Some(mnemosyne_core::BranchFork {
                        branch: MAIN_BRANCH.to_string(),
                        at: "s1".to_string(),
                    }),
                    converges_from: vec![],
                },
            );
        }
        store.branches.insert(
            "dawn".to_string(),
            mnemosyne_core::Branch {
                description: "the shared dawn".to_string(),
                forks_from: None,
                converges_from: vec![
                    mnemosyne_core::BranchFork {
                        branch: "sluice".to_string(),
                        at: "s2".to_string(),
                    },
                    mnemosyne_core::BranchFork {
                        branch: "ride".to_string(),
                        at: "s2".to_string(),
                    },
                ],
            },
        );
        let order = chain(&["s1", "s2", "s3"]);
        let report = fork_tree(&store, &order).unwrap();
        let dawn = report
            .branches
            .iter()
            .find(|b| b.branch_id == "dawn")
            .unwrap();
        assert!(dawn.fork.is_none(), "a confluence has no single fork edge");
        assert_eq!(dawn.converges.len(), 2, "both incoming merges surfaced");
        let mut parents: Vec<&str> = dawn.converges.iter().map(|e| e.parent.as_str()).collect();
        parents.sort();
        assert_eq!(parents, vec!["ride", "sluice"]);
        // s2 is a node of each parent's composed order (the base chain) — placed.
        assert!(dawn.converges.iter().all(|e| e.at_placed));
        assert!(report.unplaced_fork_points.is_empty());
    }

    /// Round 533 — the Harlow Mill diamond fixture: `sluice` and `ride` fork at
    /// `tr`, run EXCLUSIVE middles (`sl` / `rd`), and CONVERGE into `dawn` — the
    /// shared `rk -> rv` suffix authored ONCE on the confluence (the R531 2x
    /// duplication, gone). `extra` injects pairs for the conflict-scoping test.
    fn diamond_store(extra: Vec<FactImport>) -> AtomicStore {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("s.json");
        let mut store = AtomicStore::new();
        for s in ["tr-0", "tr", "sl", "rd", "rk", "rv"] {
            store
                .sections
                .insert(s.to_string(), AtomicSection::default());
        }
        let on = |id: &str, branch: &str, at: &str| FactImport {
            branch: Some(branch.to_string()),
            ..fact(id, "gt", at, None)
        };
        let mut facts = vec![
            fact("f-trunk", "gt", "tr-0", None),
            fact("f-fork", "gt", "tr", None),
            on("f-sluice", "sluice", "sl"),
            on("f-ride", "ride", "rd"),
            on("f-reckon", "dawn", "rk"),
            on("f-river", "dawn", "rv"),
        ];
        facts.extend(extra);
        let converge = |b: &str, at: &str| mnemosyne_atomic::BranchConvergeImport {
            branch: b.to_string(),
            at: at.to_string(),
        };
        let fork = |id: &str| mnemosyne_atomic::BranchImport {
            branch_id: id.to_string(),
            description: String::new(),
            forks_from: Some(MAIN_BRANCH.to_string()),
            forks_at: Some("tr".to_string()),
            converges_from: vec![],
        };
        mnemosyne_atomic::import_facts(
            &mut store,
            &path,
            &FactsManifest {
                disclosure_plans: vec![],
                frames: vec![mnemosyne_atomic::FrameImport {
                    frame_id: "gt".to_string(),
                    description: String::new(),
                }],
                // Parents-first: the confluence's parents must pre-exist (R532).
                branches: vec![
                    fork("sluice"),
                    fork("ride"),
                    mnemosyne_atomic::BranchImport {
                        branch_id: "dawn".to_string(),
                        description: String::new(),
                        forks_from: None,
                        forks_at: None,
                        converges_from: vec![converge("sluice", "sl"), converge("ride", "rd")],
                    },
                ],
                entities: vec![],
                predicates: vec![],
                facts,
            },
        )
        .unwrap();
        store
    }

    /// The order for the diamond: a 2-node trunk (`tr-0 -> tr`, fork at `tr`),
    /// each parent connecting its last exclusive scene to the merge scene `rk`,
    /// and the shared suffix `rk -> rv` declared ONCE on `dawn`.
    fn diamond_order(store: &AtomicStore) -> CanonOrder {
        let e = |a: &str, b: &str| [a.to_string(), b.to_string()];
        let decl = CanonOrderFile {
            edges: vec![e("tr-0", "tr")],
            branches: BTreeMap::from([
                ("sluice".to_string(), vec![e("tr", "sl"), e("sl", "rk")]),
                ("ride".to_string(), vec![e("tr", "rd"), e("rd", "rk")]),
                ("dawn".to_string(), vec![e("rk", "rv")]),
            ]),
        };
        CanonOrder::from_declaration(&decl, &world_order_composition(&store.branches).unwrap())
            .unwrap()
    }

    /// Round 533 — VISIBILITY: a fact authored ONCE on the confluence holds in
    /// BOTH parent worlds past the merge (forward visibility), and NOT before
    /// (the order, not visibility, bounds the timing). Exclusive middles stay
    /// exclusive; `main` (the pre-fork trunk) never reaches the merge.
    #[test]
    fn confluence_suffix_visible_in_both_parents_after_merge() {
        let store = diamond_store(vec![]);
        let order = diamond_order(&store);
        let holding = |branch: &str, at: &str| -> Vec<String> {
            frame_view(&store, &order, "gt", branch, None, at)
                .unwrap()
                .holding
                .into_iter()
                .map(|entry| entry.fact_id)
                .collect()
        };
        let reckon = "f-reckon".to_string();
        // Shared suffix, authored once on `dawn`, holds in BOTH parents at `rk`.
        assert!(holding("sluice", "rk").contains(&reckon));
        assert!(holding("ride", "rk").contains(&reckon));
        // ...but NOT before the merge in either parent.
        assert!(!holding("sluice", "sl").contains(&reckon));
        // Exclusive middles do not cross — `f-sluice` is its own world's, not
        // `ride`'s — but BOTH share the suffix.
        assert!(holding("sluice", "rk").contains(&"f-sluice".to_string()));
        assert!(!holding("ride", "rk").contains(&"f-sluice".to_string()));
        // `main` is the pre-fork trunk; the shared suffix is downstream of the
        // fork+merge, so a pure-main reading never sees it.
        assert!(!holding("main", "tr").contains(&reckon));
    }

    /// Round 533 — COMPOSITION: the shared suffix authored ONCE renders in EACH
    /// parent's manuscript (the duplication R531 measured is removed), with no
    /// leak across the exclusive middles. The confluence is NOT a standalone
    /// world in the default dump.
    #[test]
    fn confluence_suffix_authored_once_renders_in_both_parent_manuscripts() {
        let store = diamond_store(vec![]);
        let order = diamond_order(&store);
        // Default dump = the PLAYTHROUGHS; `dawn` is a structural merge, not one.
        let dump = playthrough_manuscript(&store, &order, None, None).unwrap();
        let mut worlds: Vec<&str> = dump.worlds.keys().map(String::as_str).collect();
        worlds.sort();
        assert_eq!(worlds, vec!["main", "ride", "sluice"]);
        let begins =
            |report: &PlaythroughManuscriptReport, world: &str, scene: &str| -> Vec<String> {
                report.worlds[world]
                    .scenes
                    .iter()
                    .find(|s| s.section == scene)
                    .map(|s| s.begins.iter().map(|ev| ev.fact_id.clone()).collect())
                    .unwrap_or_default()
            };
        for world in ["sluice", "ride"] {
            let m = playthrough_manuscript(&store, &order, Some(world), None).unwrap();
            assert!(
                begins(&m, world, "rk").contains(&"f-reckon".to_string()),
                "{world} must begin the shared reckoning authored on `dawn`"
            );
            assert!(
                begins(&m, world, "rv").contains(&"f-river".to_string()),
                "{world} must begin the shared river authored on `dawn`"
            );
        }
        // No middle leaks across the exclusive parents.
        let sl = playthrough_manuscript(&store, &order, Some("sluice"), None).unwrap();
        assert!(sl.worlds["sluice"].scenes.iter().all(|s| s.section != "rd"));
        let rd = playthrough_manuscript(&store, &order, Some("ride"), None).unwrap();
        assert!(rd.worlds["ride"].scenes.iter().all(|s| s.section != "sl"));
    }

    /// Round 533 — the conflict gate still SCOPES correctly across a confluence:
    /// the clean diamond has no contradictions (no false off-branch / false
    /// overlap from the new forward edges), and two conflicting facts authored
    /// on the SAME confluence ARE caught (suffix-suffix scopes to `dawn`). The
    /// cross-merge case (a suffix fact vs a parent-MIDDLE fact) is the R534
    /// reconciliation gate, deliberately out of this round.
    #[test]
    fn confluence_conflict_scoping() {
        let clean = diamond_store(vec![]);
        let order = diamond_order(&clean);
        assert!(
            scan_continuity(&clean, &order, &[])
                .unwrap()
                .violations
                .is_empty(),
            "the clean diamond scans without contradictions"
        );
        // Two facts on `dawn` at the merge scene `rk` that conflict co-hold
        // there — caught, scoped to the confluence.
        let mut clash = FactImport {
            branch: Some("dawn".to_string()),
            ..fact("f-reckon2", "gt", "rk", None)
        };
        clash.conflicts_with = vec!["f-reckon".to_string()];
        let store = diamond_store(vec![clash]);
        let report = scan_continuity(&store, &diamond_order(&store), &[]).unwrap();
        assert!(
            report.violations.iter().any(|v| matches!(
                v,
                ContinuityViolation::FrameConflictOverlap { branch, .. } if branch == "dawn"
            )),
            "a suffix-suffix conflict scopes to the confluence: {:?}",
            report.violations
        );
    }

    /// Round 535 — CONFLICT scoping across a confluence: a suffix fact (on the
    /// merge) and a parent-MIDDLE fact that conflict co-hold in the PARENT world
    /// (the suffix is visible there via forward inheritance), so the cross-merge
    /// conflict is caught — the gap R533/R534 left bucketed as
    /// `cross_scope_pairs`. The sibling parent is NOT dragged in.
    #[test]
    fn confluence_cross_merge_conflict_scopes_to_parent() {
        // A suffix fact on `dawn` declared to conflict with `f-sluice` (sluice's
        // exclusive middle); they co-exist only in sluice's playthrough.
        let clash = FactImport {
            branch: Some("dawn".to_string()),
            conflicts_with: vec!["f-sluice".to_string()],
            ..fact("f-merge-clash", "gt", "rv", None)
        };
        let store = diamond_store(vec![clash]);
        let report = scan_continuity(&store, &diamond_order(&store), &[]).unwrap();
        assert!(
            report.violations.iter().any(|v| matches!(
                v,
                ContinuityViolation::FrameConflictOverlap { branch, fact_a, fact_b, .. }
                    if branch == "sluice"
                        && [fact_a.as_str(), fact_b.as_str()].contains(&"f-merge-clash")
                        && [fact_a.as_str(), fact_b.as_str()].contains(&"f-sluice")
            )),
            "a suffix-vs-parent-middle conflict scopes to the parent: {:?}",
            report.violations
        );
        assert!(
            !report.violations.iter().any(|v| matches!(
                v,
                ContinuityViolation::FrameConflictOverlap { branch, .. } if branch == "ride"
            )),
            "the conflict must not leak into the sibling parent: {:?}",
            report.violations
        );
        assert_eq!(
            report.cross_scope_pairs, 0,
            "the cross-merge pair is now scoped, not bucketed"
        );
    }

    /// Round 535 — SUCCESSION reconciliation wired at BOTH enforcement points
    /// (they share `mnemosyne_core::succession_branch_inherits`, unit-tested for
    /// the four directions in mnemosyne-core). A suffix fact may supersede a
    /// parent belief at the merge — accepted by the write path (the import does
    /// not panic) AND clean in the scan. A sibling-world succession inherits in
    /// neither direction — rejected by the write path (it never reaches the
    /// scan).
    #[test]
    fn confluence_suffix_reconciles_parent_belief() {
        // ACCEPTED: suffix `f-reconcile` on `dawn` supersedes `f-sluice`.
        let reconcile = FactImport {
            branch: Some("dawn".to_string()),
            supersedes_in_frame: Some("f-sluice".to_string()),
            ..fact("f-reconcile", "gt", "rk", None)
        };
        let store = diamond_store(vec![reconcile]);
        let report = scan_continuity(&store, &diamond_order(&store), &[]).unwrap();
        assert!(
            !report
                .violations
                .iter()
                .any(|v| matches!(v, ContinuityViolation::SuccessionCrossBranch { .. })),
            "a confluence suffix reconciling a parent belief is allowed: {:?}",
            report.violations
        );

        // REJECTED at the write path: a sibling-world succession (ride
        // superseding a sluice belief) inherits in neither direction.
        let mut store = diamond_store(vec![]);
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("s.json");
        let err = mnemosyne_atomic::import_facts(
            &mut store,
            &path,
            &FactsManifest {
                disclosure_plans: vec![],
                frames: vec![],
                branches: vec![],
                entities: vec![],
                predicates: vec![],
                facts: vec![FactImport {
                    branch: Some("ride".to_string()),
                    supersedes_in_frame: Some("f-sluice".to_string()),
                    ..fact("f-sibling", "gt", "rd", None)
                }],
            },
        )
        .unwrap_err();
        assert!(
            format!("{err:?}").contains("does not inherit"),
            "a sibling-world succession is rejected at the write path: {err:?}"
        );
    }

    /// Round 535 — the per-parent dependency GATE: a suffix fact whose evidence
    /// is reachable from only ONE incoming parent is an unreconciled cross-merge
    /// dependency (flagged against the parent it is NOT reachable from); a suffix
    /// fact citing a shared/trunk scene reachable from EVERY parent is clean. The
    /// clean case also proves the false-positive fix — the confluence's own
    /// prefix-less order cannot connect the trunk to the suffix, so the pre-R535
    /// `le(confluence, …)` would have wrongly flagged it.
    #[test]
    fn confluence_evidence_reconciled_per_parent() {
        // Suffix fact on `dawn` citing `sl` (sluice's EXCLUSIVE middle):
        // reachable from sluice, NOT from ride.
        let only_sluice = FactImport {
            branch: Some("dawn".to_string()),
            evidence: vec!["sl".to_string()],
            ..fact("f-onesided", "gt", "rv", None)
        };
        let store = diamond_store(vec![only_sluice]);
        let report = scan_continuity(&store, &diamond_order(&store), &[]).unwrap();
        assert!(
            report.violations.iter().any(|v| matches!(
                v,
                ContinuityViolation::ConfluenceEvidenceUnreconciled {
                    confluence,
                    parent,
                    evidence,
                    ..
                } if confluence == "dawn" && parent == "ride" && evidence == "sl"
            )),
            "evidence reachable from only one parent is flagged against the other: {:?}",
            report.violations
        );
        assert!(
            !report.violations.iter().any(|v| matches!(
                v,
                ContinuityViolation::ConfluenceEvidenceUnreconciled { parent, .. }
                    if parent == "sluice"
            )),
            "…but NOT against the parent it IS reachable from: {:?}",
            report.violations
        );

        // Suffix fact citing the pre-fork trunk `tr` — reachable from BOTH
        // parents: clean (and the pre-R535 confluence-order check false-flagged).
        let shared = FactImport {
            branch: Some("dawn".to_string()),
            evidence: vec!["tr".to_string()],
            ..fact("f-shared-dep", "gt", "rv", None)
        };
        let store = diamond_store(vec![shared]);
        let report = scan_continuity(&store, &diamond_order(&store), &[]).unwrap();
        assert!(
            !report.violations.iter().any(|v| matches!(
                v,
                ContinuityViolation::ConfluenceEvidenceUnreconciled { .. }
                    | ContinuityViolation::EvidenceUnreachable { .. }
            )),
            "trunk evidence reachable from every parent is clean: {:?}",
            report.violations
        );
    }

    /// The headline nested case (R497 Detroit dogfood, locked as a
    /// regression): `at_placed` resolves against the PARENT's COMPOSED order,
    /// not the base / `main`. `route` forks `main` and declares its own edge
    /// `ch-2 -> k-1`, so `k-1` is named by route's composition but never by
    /// the base; `deep` forks `route` at `k-1` (placed via the parent's
    /// composition) while `side` forks `main` at the SAME `k-1` (unplaced —
    /// base never names it). Without this, a refactor of `reach_for`/`names`
    /// or the `from_declaration` ancestry composition could silently flip
    /// every nested fork to unplaced and the main-parent tests would stay
    /// green.
    #[test]
    fn fork_tree_resolves_nested_parent_composed_order() {
        let store = store_with_forks(
            vec![fact("f-main", "gt", "ch-1", None)],
            &[
                ("route", MAIN_BRANCH, "ch-2"),
                ("side", MAIN_BRANCH, "k-1"),
                ("deep", "route", "k-1"),
            ],
        );
        let decl = CanonOrderFile {
            edges: vec![["ch-1".to_string(), "ch-2".to_string()]],
            branches: BTreeMap::from([(
                "route".to_string(),
                vec![["ch-2".to_string(), "k-1".to_string()]],
            )]),
        };
        let order =
            CanonOrder::from_declaration(&decl, &world_order_composition(&store.branches).unwrap())
                .unwrap();
        let report = fork_tree(&store, &order).unwrap();
        let by_id = |id: &str| report.branches.iter().find(|b| b.branch_id == id).unwrap();

        let deep = by_id("deep").fork.as_ref().unwrap();
        assert_eq!(deep.parent, "route");
        assert!(
            deep.at_placed,
            "k-1 is named by route's COMPOSED order (route's own edge), so the nested fork is placed"
        );
        let side = by_id("side").fork.as_ref().unwrap();
        assert_eq!(side.parent, MAIN_BRANCH);
        assert!(
            !side.at_placed,
            "the SAME node k-1 is not in main's base order — resolution is parent-specific"
        );
        assert_eq!(report.unplaced_fork_points, ["side"]);
    }

    // ====================================================================
    // Round 489 — interval rule (depth-ladder rung 1, design sec 7.20).
    // ====================================================================

    /// A typed scalar fact on a named world-line.
    fn scalar_branch(
        id: &str,
        branch: &str,
        from: &str,
        subject: &str,
        predicate: &str,
        value: &str,
    ) -> FactImport {
        FactImport {
            branch: Some(branch.to_string()),
            ..typed_fact(id, "gt", from, subject, predicate, at(value))
        }
    }

    fn ratify_term() -> NarrativeRule {
        NarrativeRule {
            id: "ratify-term".to_string(),
            predicate: "ratified-on-day".to_string(),
            spec: NarrativeRuleSpec::Interval {
                right: "signed-on-day".to_string(),
                op: IntervalOp::Ge,
                bound: IntervalBound::Predicate("min-ratify-gap-days".to_string()),
            },
        }
    }

    /// The St. Martin Codicil in miniature (the PoC pull, deterministic): the
    /// SAME inherited rule (`min-ratify-gap-days = 42`) is clean in the lawful
    /// world-line (84 − 42 = 42 ≥ 42) and a violation in the hasty one
    /// (31 − 10 = 21 ≥ 42 is false). The fault is WORLD-SPECIFIC — a
    /// cross-predicate magnitude relation no exclusivity gate can express.
    #[test]
    fn interval_gap_violation_is_world_specific() {
        let store = store_with_forks(
            vec![
                // Inherited rule on the trunk (pre-fork ch-1).
                typed_fact(
                    "f-rule",
                    "gt",
                    "ch-1",
                    "codicil",
                    "min-ratify-gap-days",
                    at("42"),
                ),
                scalar_branch(
                    "f-sign-l",
                    "lawful",
                    "ch-3",
                    "codicil",
                    "signed-on-day",
                    "42",
                ),
                scalar_branch(
                    "f-rat-l",
                    "lawful",
                    "ch-4",
                    "codicil",
                    "ratified-on-day",
                    "84",
                ),
                scalar_branch(
                    "f-sign-h",
                    "hasty",
                    "ch-3",
                    "codicil",
                    "signed-on-day",
                    "10",
                ),
                scalar_branch(
                    "f-rat-h",
                    "hasty",
                    "ch-4",
                    "codicil",
                    "ratified-on-day",
                    "31",
                ),
            ],
            &[
                ("lawful", MAIN_BRANCH, "ch-2"),
                ("hasty", MAIN_BRANCH, "ch-2"),
            ],
        );
        let order = chain(&["ch-1", "ch-2", "ch-3", "ch-4"]);
        let report = scan_continuity(&store, &order, &[ratify_term()]).unwrap();
        let intervals: Vec<_> = report
            .violations
            .iter()
            .filter_map(|v| match v {
                ContinuityViolation::RuleIntervalViolation {
                    branch, subject, ..
                } => Some((branch.as_str(), subject.as_str())),
                _ => None,
            })
            .collect();
        assert_eq!(
            intervals,
            vec![("hasty", "codicil")],
            "exactly the hasty world-line violates, lawful is clean: {:?}",
            report.violations
        );
        assert_eq!(report.interval_unverifiable, 0);
    }

    /// A `const` bound (no rule fact): `ratified − signed >= 6` fails on a
    /// 5-day gap, holds on a 6-day one.
    #[test]
    fn interval_const_bound_gates_short_gap() {
        let rule = NarrativeRule {
            id: "min-six".to_string(),
            predicate: "ratified-on-day".to_string(),
            spec: NarrativeRuleSpec::Interval {
                right: "signed-on-day".to_string(),
                op: IntervalOp::Ge,
                bound: IntervalBound::Const(6.0),
            },
        };
        let order = chain(&["ch-1", "ch-2", "ch-3", "ch-4"]);
        let short = store_with(vec![
            typed_fact("s", "gt", "ch-1", "codicil", "signed-on-day", at("10")),
            typed_fact("r", "gt", "ch-2", "codicil", "ratified-on-day", at("15")),
        ]);
        let report = scan_continuity(&short, &order, std::slice::from_ref(&rule)).unwrap();
        assert!(
            report
                .violations
                .iter()
                .any(|v| matches!(v, ContinuityViolation::RuleIntervalViolation { .. })),
            "5-day gap must violate >= 6: {:?}",
            report.violations
        );
        let ok = store_with(vec![
            typed_fact("s", "gt", "ch-1", "codicil", "signed-on-day", at("10")),
            typed_fact("r", "gt", "ch-2", "codicil", "ratified-on-day", at("16")),
        ]);
        let report = scan_continuity(&ok, &order, &[rule]).unwrap();
        assert!(
            report.violations.is_empty(),
            "6-day gap is clean: {:?}",
            report.violations
        );
    }

    /// A non-numeric operand is SURFACED as `interval_unverifiable`, never a
    /// gating violation (the R485 unverifiable class — the author types it,
    /// then the gate decides).
    #[test]
    fn interval_non_numeric_operand_surfaces_not_gates() {
        let rule = NarrativeRule {
            id: "min-six".to_string(),
            predicate: "ratified-on-day".to_string(),
            spec: NarrativeRuleSpec::Interval {
                right: "signed-on-day".to_string(),
                op: IntervalOp::Ge,
                bound: IntervalBound::Const(6.0),
            },
        };
        let store = store_with(vec![
            typed_fact("s", "gt", "ch-1", "codicil", "signed-on-day", at("early")),
            typed_fact("r", "gt", "ch-2", "codicil", "ratified-on-day", at("20")),
        ]);
        let order = chain(&["ch-1", "ch-2", "ch-3", "ch-4"]);
        let report = scan_continuity(&store, &order, &[rule]).unwrap();
        assert!(
            !report
                .violations
                .iter()
                .any(|v| matches!(v, ContinuityViolation::RuleIntervalViolation { .. })),
            "an unparseable operand must not gate: {:?}",
            report.violations
        );
        assert_eq!(report.interval_unverifiable, 1);
    }

    /// Every referenced predicate is a load-bearing ref: an interval rule whose
    /// `right` operand is unregistered fails loud (the R436 typo guard, now
    /// covering the interval legs).
    #[test]
    fn interval_unknown_right_predicate_rejects() {
        let store = store_with(vec![typed_fact(
            "r",
            "gt",
            "ch-1",
            "codicil",
            "ratified-on-day",
            at("20"),
        )]);
        let order = chain(&["ch-1", "ch-2", "ch-3", "ch-4"]);
        let rule = NarrativeRule {
            id: "r".to_string(),
            predicate: "ratified-on-day".to_string(),
            spec: NarrativeRuleSpec::Interval {
                right: "signed-on-day".to_string(), // never registered (no fact uses it)
                op: IntervalOp::Ge,
                bound: IntervalBound::Const(6.0),
            },
        };
        let err = scan_continuity(&store, &order, &[rule]).unwrap_err();
        assert!(
            err.contains("predicate registry") && err.contains("signed-on-day"),
            "{err}"
        );
    }

    /// The loader: a valid interval rule parses; its legs and coherence are
    /// checked (symmetric to the exclusive/transition matrix, R443).
    #[test]
    fn interval_loader_validates_shape_and_coherence() {
        let tmp = tempfile::TempDir::new().unwrap();
        let write = |name: &str, body: &str| {
            let p = tmp.path().join(name);
            std::fs::write(&p, body).unwrap();
            p
        };
        // Valid: predicate-bound.
        let ok = write(
            "ok.json",
            r#"{"rules":[{"id":"t","class":"interval","predicate":"ratified-on-day",
                "right":"signed-on-day","op":"ge","bound":{"predicate":"min-gap"}}]}"#,
        );
        let file = load_narrative_rules(&ok, None).unwrap();
        assert!(matches!(
            file.rules[0].spec,
            NarrativeRuleSpec::Interval {
                op: IntervalOp::Ge,
                ..
            }
        ));
        // Valid: const-bound.
        let konst = write(
            "const.json",
            r#"{"rules":[{"id":"t","class":"interval","predicate":"a","right":"b",
                "op":"lt","bound":{"const":6}}]}"#,
        );
        assert!(load_narrative_rules(&konst, None).is_ok());
        // Missing the `right` operand.
        let no_right = write(
            "no-right.json",
            r#"{"rules":[{"id":"t","class":"interval","predicate":"a","op":"ge",
                "bound":{"const":6}}]}"#,
        );
        assert!(load_narrative_rules(&no_right, None)
            .unwrap_err()
            .contains("right"));
        // Interval carrying a `per` leg (belongs to exclusive).
        let stray_per = write(
            "stray-per.json",
            r#"{"rules":[{"id":"t","class":"interval","predicate":"a","right":"b",
                "op":"ge","bound":{"const":6},"per":"subject"}]}"#,
        );
        let err = load_narrative_rules(&stray_per, None).unwrap_err();
        assert!(err.contains("interval") && err.contains("per"), "{err}");
        // Exclusive carrying an interval leg (symmetric coherence).
        let stray_interval = write(
            "stray-interval.json",
            r#"{"rules":[{"id":"t","class":"exclusive","predicate":"a","per":"subject",
                "right":"b"}]}"#,
        );
        let err = load_narrative_rules(&stray_interval, None).unwrap_err();
        assert!(
            err.contains("exclusive") && err.contains("interval"),
            "{err}"
        );
        // Bound with BOTH predicate and const.
        let both = write(
            "both.json",
            r#"{"rules":[{"id":"t","class":"interval","predicate":"a","right":"b",
                "op":"ge","bound":{"predicate":"c","const":6}}]}"#,
        );
        assert!(load_narrative_rules(&both, None)
            .unwrap_err()
            .contains("exactly one"));
        // Bound with NEITHER.
        let neither = write(
            "neither.json",
            r#"{"rules":[{"id":"t","class":"interval","predicate":"a","right":"b",
                "op":"ge","bound":{}}]}"#,
        );
        assert!(load_narrative_rules(&neither, None)
            .unwrap_err()
            .contains("exactly one"));
        // An unknown op value is a parse error (the closed operator set).
        let bad_op = write(
            "bad-op.json",
            r#"{"rules":[{"id":"t","class":"interval","predicate":"a","right":"b",
                "op":"gte","bound":{"const":6}}]}"#,
        );
        assert!(load_narrative_rules(&bad_op, None)
            .unwrap_err()
            .contains("parse"));
    }

    /// `report-timeline-gaps` groups outcomes per world: every world present
    /// (clean ones explicitly empty), the hasty gap surfaces as Violated, the
    /// lawful gap as Satisfied, main (no left fact) empty.
    #[test]
    fn timeline_gaps_groups_outcomes_per_world() {
        let store = store_with_forks(
            vec![
                typed_fact(
                    "f-rule",
                    "gt",
                    "ch-1",
                    "codicil",
                    "min-ratify-gap-days",
                    at("42"),
                ),
                scalar_branch(
                    "f-sign-l",
                    "lawful",
                    "ch-3",
                    "codicil",
                    "signed-on-day",
                    "42",
                ),
                scalar_branch(
                    "f-rat-l",
                    "lawful",
                    "ch-4",
                    "codicil",
                    "ratified-on-day",
                    "84",
                ),
                scalar_branch(
                    "f-sign-h",
                    "hasty",
                    "ch-3",
                    "codicil",
                    "signed-on-day",
                    "10",
                ),
                scalar_branch(
                    "f-rat-h",
                    "hasty",
                    "ch-4",
                    "codicil",
                    "ratified-on-day",
                    "31",
                ),
            ],
            &[
                ("lawful", MAIN_BRANCH, "ch-2"),
                ("hasty", MAIN_BRANCH, "ch-2"),
            ],
        );
        let order = chain(&["ch-1", "ch-2", "ch-3", "ch-4"]);
        let report = timeline_gaps(&store, &order, &[ratify_term()]).unwrap();
        assert_eq!(report.interval_rules, 1);
        assert!(
            report.worlds.contains_key(MAIN_BRANCH),
            "every world present"
        );
        assert!(
            report.worlds[MAIN_BRANCH].outcomes.is_empty(),
            "main has no left fact"
        );
        let hasty = &report.worlds["hasty"].outcomes;
        assert_eq!(hasty.len(), 1);
        assert!(matches!(hasty[0].verdict, IntervalVerdict::Violated { .. }));
        let lawful = &report.worlds["lawful"].outcomes;
        assert_eq!(lawful.len(), 1);
        assert!(matches!(
            lawful[0].verdict,
            IntervalVerdict::Satisfied { .. }
        ));
    }

    /// Parity (the single-evaluator no-drift property): the gate's interval
    /// violations and the read report's Violated outcomes are the same set —
    /// both consume `scan_interval_rule`, so they cannot diverge (R305/R390).
    #[test]
    fn timeline_gaps_and_gate_agree_on_violations() {
        let store = store_with_forks(
            vec![
                typed_fact(
                    "f-rule",
                    "gt",
                    "ch-1",
                    "codicil",
                    "min-ratify-gap-days",
                    at("42"),
                ),
                scalar_branch(
                    "f-sign-h",
                    "hasty",
                    "ch-3",
                    "codicil",
                    "signed-on-day",
                    "10",
                ),
                scalar_branch(
                    "f-rat-h",
                    "hasty",
                    "ch-4",
                    "codicil",
                    "ratified-on-day",
                    "31",
                ),
            ],
            &[("hasty", MAIN_BRANCH, "ch-2")],
        );
        let order = chain(&["ch-1", "ch-2", "ch-3", "ch-4"]);
        let gate = scan_continuity(&store, &order, &[ratify_term()]).unwrap();
        let read = timeline_gaps(&store, &order, &[ratify_term()]).unwrap();
        let gate_violations = gate
            .violations
            .iter()
            .filter(|v| matches!(v, ContinuityViolation::RuleIntervalViolation { .. }))
            .count();
        let read_violated = read
            .worlds
            .values()
            .flat_map(|w| &w.outcomes)
            .filter(|o| matches!(o.verdict, IntervalVerdict::Violated { .. }))
            .count();
        assert_eq!(gate_violations, 1);
        assert_eq!(
            gate_violations, read_violated,
            "gate and read surface must agree"
        );
    }
}
