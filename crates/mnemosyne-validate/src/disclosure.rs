//! Disclosure (discourse) layer reports + render-acceptance gates (Round 507,
//! design sec 7.24 — the build of R506 steps 4-6).
//!
//! - [`disclosure_coverage`] — a SURFACE (never gated, the R442
//!   dangling-is-a-todo discipline): per telling, every fact classified
//!   disclosed / hidden-by-design / never-planned.
//! - [`disclosure_leak`] — the premature-leak GATE (R502): a withheld fact
//!   must NOT be re-extractable, and a `first_at`-pinned fact must not be
//!   re-extractable before its pin in the world's discourse order. Matched
//!   to the BLIND RE-EXTRACTED prose store by TYPED (subject, predicate,
//!   object) tuple in a caller-named truth frame — the determinism keystone
//!   that removes R505's manual mapping (AI out of the gate; the comparison
//!   is tuple/coordinate equality over the artifacts).
//! - [`render_fidelity`] — the render↔world-line GATE (R505 input 1): every
//!   re-extracted fact's `canon_from` must stay in the assigned world's
//!   composed order — a coord that is a declaration node of ANOTHER world is
//!   off-path (the prose drifted onto the wrong world-line). The prose analog
//!   of R488 `FactCanonOffBranch`.
//!
//! These two gates operate on TWO stores (the authored plan + the re-extracted
//! prose) — a render-acceptance family distinct from the single-store
//! `validate-workspace` store-integrity gates; disclosure timing is a render
//! property, not a store invariant.

use std::cmp::Ordering;
use std::collections::BTreeSet;

use mnemosyne_atomic::AtomicStore;
use mnemosyne_core::{DisclosureMode, DisclosureReveal};
use serde::Serialize;

use crate::continuity::CanonOrder;

/// Resolve a [`DisclosureReveal`] DECLARATION to its effective first-reveal pin
/// on `world` (Round 752): the k-th-EARLIEST trigger coordinate by `world`'s
/// canon order, k = `threshold.unwrap_or(1)` (FIRST-reached by default). The ONE
/// order-aware resolver of a reveal — the leak gate compares against this pin
/// exactly as it did the single ordinal before R752 (core stores the order-free
/// declaration; this validate-layer helper applies the order, per the layering
/// split). Returns `None` when the reveal's coords do NOT form a CHAIN in this
/// world's order (incomparable or world-absent coords), or when fewer than k are
/// present: the k-th-earliest is then UNDEFINED, so the gate surfaces the matches
/// as unordered rather than inventing a false pin (an honest None, never a
/// guessed early-leak verdict). Pinion resolves first-reached against the
/// player's actual non-linear path at runtime from the same declaration.
pub fn resolve_reveal_pin(
    reveal: &DisclosureReveal,
    world: &str,
    order: &CanonOrder,
) -> Option<String> {
    let k = reveal.threshold.unwrap_or(1);
    if k == 0 {
        return None; // defensive — the write path normalizes Some(0) away
    }
    let mut coords: Vec<&str> = reveal.coords.iter().map(String::as_str).collect();
    if coords.len() < k {
        return None;
    }
    coords.sort_by(|a, b| {
        if a == b {
            Ordering::Equal
        } else if order.le(world, a, b) {
            Ordering::Less
        } else if order.le(world, b, a) {
            Ordering::Greater
        } else {
            // Incomparable — a deterministic id tiebreak keeps the sort total;
            // the chain check below rejects the (ambiguous) ordering as None.
            a.cmp(b)
        }
    });
    // The coords must form a CHAIN (each precedes the next) for the k-th-earliest
    // to be well-defined; a non-chain has no definite k-th trigger.
    if coords.windows(2).all(|w| order.le(world, w[0], w[1])) {
        Some(coords[k - 1].to_string())
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Step 4 — disclosure coverage (SURFACE, never gated).
// ---------------------------------------------------------------------------

/// Per-telling coverage classification (Round 507) — the THIRD coverage
/// instance after the spec axiom (R389/R390) and payoff (R442), same
/// dangling-is-a-todo discipline: `never_planned` is the author's todo list,
/// never a gate reject.
#[derive(Debug, Clone, Serialize)]
pub struct DisclosureCoverageReport {
    pub telling: String,
    pub facts: usize,
    /// Effective mode ≠ withhold (an override state/hint/imply, or a
    /// non-withhold plan default with no override).
    pub disclosed: usize,
    /// An explicit `withhold` override — the author DECIDED to hide it.
    pub hidden_by_design: usize,
    /// No override under a withhold-default telling — withheld by default, no
    /// explicit decision (the todo signal). Sorted, never gated.
    pub never_planned: Vec<String>,
}

/// Classify every fact under a telling (Round 507). Order-independent (a mode
/// is one decision, not per-world); fails loud on a typo'd telling.
pub fn disclosure_coverage(
    store: &AtomicStore,
    telling: &str,
) -> Result<DisclosureCoverageReport, String> {
    let plan = store.disclosure_plans.get(telling).ok_or_else(|| {
        format!("telling `{telling}` not present in the disclosure_plans registry (fail-loud)")
    })?;
    let mut disclosed = 0;
    let mut hidden_by_design = 0;
    let mut never_planned = Vec::new();
    for id in store.narrative_facts.keys() {
        // The single resolver (Round 510) — coverage cannot drift from the
        // carrier on the override-vs-default rule.
        match plan.effective_mode(id) {
            (DisclosureMode::Withhold, true) => hidden_by_design += 1,
            (DisclosureMode::Withhold, false) => never_planned.push(id.clone()),
            (_, _) => disclosed += 1,
        }
    }
    Ok(DisclosureCoverageReport {
        telling: telling.to_string(),
        facts: store.narrative_facts.len(),
        disclosed,
        hidden_by_design,
        never_planned,
    })
}

// ---------------------------------------------------------------------------
// Step 5 — premature-leak gate (R502), cross-store, typed-tuple matched.
// ---------------------------------------------------------------------------

/// The kind of premature-leak finding (Round 510 — a typed enum, not a
/// stringly field, matching the codebase's serde-tagged-enum convention).
/// `Withhold` = a `withhold`-mode fact re-extracted at all; `Early` = a
/// `first_at`-pinned fact re-extractable strictly before its pin; `Unordered`
/// = matched at a coord incomparable to the pin (an honesty surface, not a
/// verdict — carried in the report's `unordered`, never `leaks`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LeakKind {
    Withhold,
    Early,
    Unordered,
}

impl LeakKind {
    /// Canonical lowercase label (matches the serde representation).
    pub fn as_str(self) -> &'static str {
        match self {
            LeakKind::Withhold => "withhold",
            LeakKind::Early => "early",
            LeakKind::Unordered => "unordered",
        }
    }
}

/// One premature-leak finding (Round 507).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DisclosureLeak {
    /// The authored plan-targeted fact (withhold or first_at).
    pub fact_id: String,
    /// What kind of leak (or honesty surface) this is.
    pub kind: LeakKind,
    /// The matched re-extracted fact id (truth-frame, same typed tuple).
    pub reextracted_id: String,
    /// The matched fact's re-extracted discourse coordinate.
    pub coord: String,
    /// The RESOLVED first_at pin the gate compared against — the k-th-earliest
    /// trigger of the world's [`DisclosureReveal`] by its canon order (Round
    /// 752). `early` / `unordered` only; `None` when the reveal had no definite
    /// pin in this world (a non-chain trigger set surfaced as `unordered`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_at: Option<String>,
}

/// Premature-leak gate report (Round 507).
#[derive(Debug, Clone, Serialize)]
pub struct DisclosureLeakReport {
    pub telling: String,
    pub world: String,
    pub truth_frame: String,
    /// Plan-targeted facts checked for this world (withhold or first_at[world];
    /// all carry a typed claim by the set_disclosure invariant).
    pub targeted: usize,
    /// The gate failures: withheld facts that appear, or facts re-extractable
    /// before their `first_at`. Empty = PASS.
    pub leaks: Vec<DisclosureLeak>,
    /// A first_at fact matched at a coord INCOMPARABLE to its pin in this
    /// world's order (B-1 honesty — surfaced, not a leak verdict).
    pub unordered: Vec<DisclosureLeak>,
    /// first_at-pinned facts with NO truth-frame match (not disclosed in the
    /// prose at all — a coverage note, not a leak).
    pub unmatched: Vec<String>,
    /// Re-extracted facts in `truth_frame` carrying a typed claim — the
    /// universe this gate matches against (Round 510, the F5 vacuous-pass
    /// guard).
    pub truth_frame_typed_facts: usize,
    /// Of those, how many use a subject AND predicate the AUTHORED store
    /// registers — the shared-vocabulary count. `targeted > 0` with
    /// `vocabulary_shared == 0` means the re-extraction used foreign ids (or
    /// has no typed truth-frame facts), so a `leaks == 0` result is VACUOUS,
    /// not a clean pass — the CLI gate fails loud on it (no silent pass).
    pub vocabulary_shared: usize,
}

/// Run the premature-leak gate (Round 507, R502). For each plan-targeted fact
/// (an override that is `withhold`, or carries a `first_at` for `world`), match
/// the BLIND RE-EXTRACTED store's `truth_frame` facts by typed tuple: a
/// withheld fact that matches is a leak; a `first_at` fact whose match sits
/// strictly before the pin in `world`'s order is a leak. The targeted facts
/// are guaranteed typed (the set_disclosure invariant); a bypassed-invariant
/// untyped target fails loud. Deterministic — AI out of the gate.
pub fn disclosure_leak(
    authored: &AtomicStore,
    reextracted: &AtomicStore,
    order: &CanonOrder,
    telling: &str,
    world: &str,
    truth_frame: &str,
) -> Result<DisclosureLeakReport, String> {
    let plan = authored.disclosure_plans.get(telling).ok_or_else(|| {
        format!("telling `{telling}` not present in the disclosure_plans registry (fail-loud)")
    })?;
    // F5 vacuous-pass guard (Round 510): measure the re-extraction's
    // truth-frame typed universe and how much of it shares the authored
    // vocabulary. A withheld-fact "no match" and a foreign-vocabulary "no
    // match" are indistinguishable by leak count alone — this surfaces the
    // difference so a blind gate (foreign ids ⇒ matches nothing ⇒ leaks=0)
    // cannot read as a clean pass.
    let mut truth_frame_typed_facts = 0usize;
    let mut vocabulary_shared = 0usize;
    for g in reextracted.narrative_facts.values() {
        if g.frame != truth_frame {
            continue;
        }
        let Some(t) = g.typed.as_ref() else {
            continue;
        };
        truth_frame_typed_facts += 1;
        if authored.entities.contains_key(&t.subject)
            && authored.predicates.contains_key(&t.predicate)
        {
            vocabulary_shared += 1;
        }
    }
    let mut report = DisclosureLeakReport {
        telling: telling.to_string(),
        world: world.to_string(),
        truth_frame: truth_frame.to_string(),
        targeted: 0,
        leaks: Vec::new(),
        unordered: Vec::new(),
        unmatched: Vec::new(),
        truth_frame_typed_facts,
        vocabulary_shared,
    };
    for (fact_id, ov) in &plan.overrides {
        let is_withhold = ov.mode == DisclosureMode::Withhold;
        let reveal = ov.first_at.get(world);
        if !is_withhold && reveal.is_none() {
            continue; // not targeted for this world-line
        }
        report.targeted += 1;
        let typed = match authored
            .narrative_facts
            .get(fact_id)
            .and_then(|f| f.typed.as_ref())
        {
            Some(t) => t,
            None => {
                return Err(format!(
                    "disclosure_leak: targeted fact `{fact_id}` has no typed claim — \
                     un-gateable (the set_disclosure typed invariant was bypassed)"
                ));
            }
        };
        let matches: Vec<(&String, &str)> = reextracted
            .narrative_facts
            .iter()
            .filter(|(_, g)| g.frame == truth_frame && g.typed.as_ref() == Some(typed))
            .map(|(gid, g)| (gid, g.canon_from.as_str()))
            .collect();
        if is_withhold {
            for (gid, coord) in matches {
                report.leaks.push(DisclosureLeak {
                    fact_id: fact_id.clone(),
                    kind: LeakKind::Withhold,
                    reextracted_id: gid.clone(),
                    coord: coord.to_string(),
                    first_at: None,
                });
            }
            continue;
        }
        let reveal = reveal.expect("targeted non-withhold has a reveal");
        // Resolve the reveal DECLARATION to its effective pin: the k-th-EARLIEST
        // trigger by this world's order (R752 first-reached-of-a-set). A reveal
        // whose coords do not form a chain in this world (or fewer than k
        // present) has no definite pin — its matches surface as unordered
        // honesty (never a false early-leak), not compared against a guessed pin.
        let pin = resolve_reveal_pin(reveal, world, order);
        if matches.is_empty() {
            report.unmatched.push(fact_id.clone());
        }
        for (gid, coord) in matches {
            let Some(pin) = pin.as_deref() else {
                report.unordered.push(DisclosureLeak {
                    fact_id: fact_id.clone(),
                    kind: LeakKind::Unordered,
                    reextracted_id: gid.clone(),
                    coord: coord.to_string(),
                    first_at: None,
                });
                continue;
            };
            if coord == pin {
                continue; // at the pin = on time
            }
            if order.le(world, coord, pin) {
                // coord <= pin and coord != pin => strictly before => leak.
                report.leaks.push(DisclosureLeak {
                    fact_id: fact_id.clone(),
                    kind: LeakKind::Early,
                    reextracted_id: gid.clone(),
                    coord: coord.to_string(),
                    first_at: Some(pin.to_string()),
                });
            } else if !order.le(world, pin, coord) {
                // neither direction => incomparable honesty surface (B-1).
                report.unordered.push(DisclosureLeak {
                    fact_id: fact_id.clone(),
                    kind: LeakKind::Unordered,
                    reextracted_id: gid.clone(),
                    coord: coord.to_string(),
                    first_at: Some(pin.to_string()),
                });
            }
            // else coord strictly after pin => on time.
        }
    }
    Ok(report)
}

// ---------------------------------------------------------------------------
// Step 6 — render↔world-line fidelity gate (R505 input 1).
// ---------------------------------------------------------------------------

/// One off-path / unplaced re-extracted fact (Round 507).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RenderPathFact {
    pub fact_id: String,
    pub coord: String,
}

/// Render↔world-line fidelity report (Round 507, R505 input 1 — the prose
/// analog of R488 `FactCanonOffBranch`).
#[derive(Debug, Clone, Serialize)]
pub struct RenderFidelityReport {
    pub world: String,
    pub reextracted_facts: usize,
    /// Re-extracted facts whose `canon_from` is a declaration node of ANOTHER
    /// world but NOT of this world's composed order — the prose drifted onto a
    /// different world-line. The FAIL signal. Empty = on the assigned path.
    pub off_path: Vec<RenderPathFact>,
    /// Re-extracted facts whose `canon_from` is not a declaration node at all
    /// (the extractor's coordinate is unplaceable — honesty surface).
    pub unplaced: Vec<RenderPathFact>,
    /// True iff some re-extracted coord is a maximal node of this world (the
    /// prose reached the assigned world-line's ending).
    pub reached_terminal: bool,
}

/// Run the render↔world-line fidelity gate (Round 507). Every re-extracted
/// fact's `canon_from` must be named in `world`'s composed order; a coord that
/// is a declaration node of a DIFFERENT world is off-path (the R504 footgun: a
/// file labeled one ending that delivered another). `world` validity is the
/// caller's guard (the ops wrapper checks the branch registry).
pub fn render_fidelity(
    reextracted: &AtomicStore,
    order: &CanonOrder,
    world: &str,
) -> RenderFidelityReport {
    let nodes: BTreeSet<&str> = order.nodes().collect();
    let mut report = RenderFidelityReport {
        world: world.to_string(),
        reextracted_facts: reextracted.narrative_facts.len(),
        off_path: Vec::new(),
        unplaced: Vec::new(),
        reached_terminal: false,
    };
    for (id, g) in &reextracted.narrative_facts {
        let coord = g.canon_from.as_str();
        if order.names(world, coord) {
            if order.is_maximal(world, coord) {
                report.reached_terminal = true;
            }
        } else if nodes.contains(coord) {
            report.off_path.push(RenderPathFact {
                fact_id: id.clone(),
                coord: coord.to_string(),
            });
        } else {
            report.unplaced.push(RenderPathFact {
                fact_id: id.clone(),
                coord: coord.to_string(),
            });
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::continuity::CanonOrderFile;
    use mnemosyne_core::{
        DisclosureOverride, DisclosurePlan, Entity, NarrativeFact, PayoffExpectation, Predicate,
        PredicateObjectKind, TypedClaim, TypedObject, MAIN_BRANCH,
    };
    use std::collections::BTreeMap;

    /// Register the `pike`/`did` vocabulary the leak fixtures type against, so
    /// the F5 vocabulary-overlap signal is meaningful (Round 510).
    fn register_vocab(store: &mut AtomicStore) {
        store.entities.insert("pike".to_string(), Entity::default());
        store.predicates.insert(
            "did".to_string(),
            Predicate {
                // Round 708 — the free-text scalar shape was removed; `did` is a
                // token predicate whose vocabulary is the states the fixtures use.
                object_kind: PredicateObjectKind::Token,
                subject_kind: None,
                object_entity_kind: None,
                object_tokens: ["climbed", "fell", "spoke"]
                    .into_iter()
                    .map(String::from)
                    .collect(),
                description: String::new(),
            },
        );
    }

    fn typed(subject: &str, value: &str) -> TypedClaim {
        TypedClaim {
            subject: subject.to_string(),
            predicate: "did".to_string(),
            object: TypedObject::Token {
                token: value.to_string(),
            },
        }
    }

    fn nf(frame: &str, canon_from: &str, typed: Option<TypedClaim>) -> NarrativeFact {
        NarrativeFact {
            frame: frame.to_string(),
            branch: MAIN_BRANCH.to_string(),
            entities: vec![],
            claim: "c".to_string(),
            canon_from: canon_from.to_string(),
            canon_to: None,
            evidence: vec![canon_from.to_string()],
            conflicts_with: vec![],
            supersedes_in_frame: None,
            payoff_expectation: PayoffExpectation::Unmarked,
            typed,
            pays_off: vec![],
            quote: None,
            quote_sha256: None,
        }
    }

    fn ov(mode: DisclosureMode, first_at: &[(&str, &str)]) -> DisclosureOverride {
        // A single-coord first-reached trigger per branch (the common case); the
        // R752 multi-coord + threshold triggers are built directly in their test.
        DisclosureOverride {
            mode,
            first_at: first_at
                .iter()
                .map(|(b, c)| {
                    (
                        b.to_string(),
                        DisclosureReveal {
                            coords: BTreeSet::from([c.to_string()]),
                            threshold: None,
                        },
                    )
                })
                .collect(),
            surface: None,
        }
    }

    /// A per-branch reveal with an explicit trigger SET + threshold (Round 752).
    fn reveal_ov(
        mode: DisclosureMode,
        branch: &str,
        coords: &[&str],
        threshold: Option<usize>,
    ) -> DisclosureOverride {
        let mut first_at = BTreeMap::new();
        first_at.insert(
            branch.to_string(),
            DisclosureReveal {
                coords: coords.iter().map(|c| c.to_string()).collect(),
                threshold,
            },
        );
        DisclosureOverride {
            mode,
            first_at,
            surface: None,
        }
    }

    fn plan(
        default_mode: DisclosureMode,
        overrides: BTreeMap<String, DisclosureOverride>,
    ) -> DisclosurePlan {
        DisclosurePlan {
            description: String::new(),
            default_mode,
            overrides,
        }
    }

    #[test]
    fn coverage_classifies_disclosed_hidden_neverplanned() {
        let mut store = AtomicStore::new();
        store.narrative_facts.insert(
            "f-state".into(),
            nf("gt", "ch-1", Some(typed("pike", "climbed"))),
        );
        store.narrative_facts.insert(
            "f-hide".into(),
            nf("gt", "ch-1", Some(typed("pike", "fell"))),
        );
        store
            .narrative_facts
            .insert("f-bare".into(), nf("gt", "ch-1", None));
        let mut overrides = BTreeMap::new();
        overrides.insert("f-state".to_string(), ov(DisclosureMode::State, &[]));
        overrides.insert("f-hide".to_string(), ov(DisclosureMode::Withhold, &[]));
        store
            .disclosure_plans
            .insert("t".into(), plan(DisclosureMode::Withhold, overrides));
        let r = disclosure_coverage(&store, "t").unwrap();
        assert_eq!(r.facts, 3);
        assert_eq!(r.disclosed, 1);
        assert_eq!(r.hidden_by_design, 1);
        assert_eq!(r.never_planned, vec!["f-bare".to_string()]);
        assert!(disclosure_coverage(&store, "nope").is_err());
    }

    #[test]
    fn leak_gate_catches_withhold_and_early_passes_clean_and_belief() {
        let mut authored = AtomicStore::new();
        register_vocab(&mut authored);
        authored
            .narrative_facts
            .insert("w".into(), nf("gt", "ch-1", Some(typed("pike", "climbed"))));
        authored
            .narrative_facts
            .insert("e".into(), nf("gt", "ch-1", Some(typed("pike", "fell"))));
        let mut overrides = BTreeMap::new();
        overrides.insert("w".to_string(), ov(DisclosureMode::Withhold, &[]));
        overrides.insert(
            "e".to_string(),
            ov(DisclosureMode::State, &[("main", "ch-3")]),
        );
        authored
            .disclosure_plans
            .insert("t".into(), plan(DisclosureMode::Withhold, overrides));
        let order = CanonOrder::from_edges(&[
            ["ch-1".into(), "ch-2".into()],
            ["ch-2".into(), "ch-3".into()],
        ])
        .unwrap();

        // CLEAN: withheld fact absent; "fell" disclosed at its pin (on time).
        let mut clean = AtomicStore::new();
        clean
            .narrative_facts
            .insert("x".into(), nf("gt", "ch-3", Some(typed("pike", "fell"))));
        let r = disclosure_leak(&authored, &clean, &order, "t", "main", "gt").unwrap();
        assert_eq!(r.targeted, 2);
        assert!(r.leaks.is_empty(), "{:?}", r.leaks);

        // LEAKY: withheld "climbed" appears; "fell" appears before its pin.
        let mut leaky = AtomicStore::new();
        leaky
            .narrative_facts
            .insert("a".into(), nf("gt", "ch-2", Some(typed("pike", "climbed"))));
        leaky
            .narrative_facts
            .insert("b".into(), nf("gt", "ch-1", Some(typed("pike", "fell"))));
        let r = disclosure_leak(&authored, &leaky, &order, "t", "main", "gt").unwrap();
        assert_eq!(r.leaks.len(), 2);
        assert!(r
            .leaks
            .iter()
            .any(|l| l.kind == LeakKind::Withhold && l.fact_id == "w"));
        assert!(r
            .leaks
            .iter()
            .any(|l| l.kind == LeakKind::Early && l.fact_id == "e"));

        // A belief-frame appearance is NOT a leak (truth_frame = gt only).
        let mut belief = AtomicStore::new();
        belief.narrative_facts.insert(
            "c".into(),
            nf("hale", "ch-1", Some(typed("pike", "climbed"))),
        );
        let r = disclosure_leak(&authored, &belief, &order, "t", "main", "gt").unwrap();
        assert!(
            r.leaks.is_empty(),
            "belief-frame is not the reader's established truth"
        );
    }

    /// Round 752 — the leak gate resolves a reveal TRIGGER SET to its k-th-
    /// earliest pin by the world's order, not a single ordinal. NON-VACUITY: the
    /// trigger set {x-2, b-4} has canon-earliest x-2 and canon-latest b-4, with
    /// LEXICAL order REVERSED (b-4 < x-2) — so a naive "first coord in the set"
    /// picks the wrong end. k=1 (first-reached) pins x-2: a match before x-2
    /// leaks, one BETWEEN x-2 and b-4 does NOT. k=2 pins b-4 (the 2nd/last): the
    /// same between-match now leaks. Reverting to single-coord fails both halves.
    #[test]
    fn leak_gate_resolves_kth_earliest_of_a_reveal_set() {
        let order = CanonOrder::from_edges(&[
            ["r-1".into(), "x-2".into()],
            ["x-2".into(), "m-3".into()],
            ["m-3".into(), "b-4".into()],
        ])
        .unwrap();
        let build = |threshold: Option<usize>| {
            let mut authored = AtomicStore::new();
            register_vocab(&mut authored);
            authored
                .narrative_facts
                .insert("e".into(), nf("gt", "x-2", Some(typed("pike", "climbed"))));
            let mut overrides = BTreeMap::new();
            overrides.insert(
                "e".to_string(),
                reveal_ov(DisclosureMode::State, "main", &["x-2", "b-4"], threshold),
            );
            authored
                .disclosure_plans
                .insert("t".into(), plan(DisclosureMode::Withhold, overrides));
            authored
        };
        let match_at = |coord: &str| {
            let mut store = AtomicStore::new();
            store
                .narrative_facts
                .insert("g".into(), nf("gt", coord, Some(typed("pike", "climbed"))));
            store
        };

        // FIRST-REACHED (k=1): the effective pin is x-2 (the canon-EARLIEST).
        let authored = build(None);
        let r = disclosure_leak(&authored, &match_at("r-1"), &order, "t", "main", "gt").unwrap();
        assert_eq!(
            r.leaks.len(),
            1,
            "a match before the first-reached trigger leaks"
        );
        assert_eq!(r.leaks[0].kind, LeakKind::Early);
        assert_eq!(
            r.leaks[0].first_at.as_deref(),
            Some("x-2"),
            "the resolved pin is the canon-earliest of the set, not the lexical-first"
        );
        let r = disclosure_leak(&authored, &match_at("m-3"), &order, "t", "main", "gt").unwrap();
        assert!(
            r.leaks.is_empty(),
            "a match AFTER the first-reached trigger (between x-2 and b-4) is on time: {:?}",
            r.leaks
        );

        // THRESHOLD 2 (k=2): the effective pin is b-4 (the 2nd-earliest = last).
        let authored = build(Some(2));
        let r = disclosure_leak(&authored, &match_at("m-3"), &order, "t", "main", "gt").unwrap();
        assert_eq!(
            r.leaks.len(),
            1,
            "k=2 pins the 2nd-earliest (b-4), so the between-match is early: {:?}",
            r.leaks
        );
        assert_eq!(r.leaks[0].first_at.as_deref(), Some("b-4"));
    }

    /// Round 510 (F5) — the vacuous-pass guard distinguishes a genuine clean
    /// run from a foreign-vocabulary blind run: both show leaks==0, but the
    /// blind run shares no vocabulary (vocabulary_shared==0) so the CLI gate can
    /// fail it loud rather than read it as clean (no silent pass).
    #[test]
    fn leak_gate_surfaces_vacuous_pass_on_foreign_vocabulary() {
        let mut authored = AtomicStore::new();
        register_vocab(&mut authored);
        authored
            .narrative_facts
            .insert("w".into(), nf("gt", "ch-1", Some(typed("pike", "climbed"))));
        let mut overrides = BTreeMap::new();
        overrides.insert("w".to_string(), ov(DisclosureMode::Withhold, &[]));
        authored
            .disclosure_plans
            .insert("t".into(), plan(DisclosureMode::Withhold, overrides));
        let order = CanonOrder::from_edges(&[["ch-1".into(), "ch-2".into()]]).unwrap();

        // FOREIGN vocabulary: the re-extraction typed an unregistered subject —
        // 0 matches LOOKS clean, but vocabulary_shared==0 marks it vacuous.
        let mut foreign = AtomicStore::new();
        foreign.narrative_facts.insert(
            "g".into(),
            nf("gt", "ch-2", Some(typed("STRANGER", "climbed"))),
        );
        let r = disclosure_leak(&authored, &foreign, &order, "t", "main", "gt").unwrap();
        assert_eq!(r.targeted, 1);
        assert!(r.leaks.is_empty());
        assert_eq!(r.truth_frame_typed_facts, 1);
        assert_eq!(r.vocabulary_shared, 0, "foreign id ⇒ no shared vocabulary");

        // SHARED vocabulary, genuinely clean: the withheld fact is absent, a
        // different shared-vocab fact present ⇒ a real clean pass.
        let mut shared = AtomicStore::new();
        shared
            .narrative_facts
            .insert("g".into(), nf("gt", "ch-2", Some(typed("pike", "spoke"))));
        let r = disclosure_leak(&authored, &shared, &order, "t", "main", "gt").unwrap();
        assert!(r.leaks.is_empty());
        assert_eq!(r.vocabulary_shared, 1, "shared vocab ⇒ a real clean pass");
    }

    #[test]
    fn fidelity_gate_catches_off_path_and_unplaced() {
        let decl = CanonOrderFile {
            edges: vec![["ch-1".to_string(), "ch-2".to_string()]],
            branches: BTreeMap::from([
                (
                    "route".to_string(),
                    vec![["ch-2".to_string(), "r-1".to_string()]],
                ),
                (
                    "other".to_string(),
                    vec![["ch-2".to_string(), "b-1".to_string()]],
                ),
            ]),
            ..Default::default()
        };
        // Round 614 — `route` and `other` are FORKS of the trunk at ch-2 (as this
        // fixture always meant). The ROAD axis makes fork-vs-standalone load-bearing:
        // a fork rides the trunk in to its fork point, a standalone does not.
        let fork_at_ch2 = || mnemosyne_core::Branch {
            forks_from: Some(mnemosyne_core::BranchFork {
                branch: mnemosyne_core::MAIN_BRANCH.to_string(),
                at: "ch-2".to_string(),
            }),
            ..Default::default()
        };
        let branches = BTreeMap::from([
            ("route".to_string(), fork_at_ch2()),
            ("other".to_string(), fork_at_ch2()),
        ]);
        let order = CanonOrder::from_declaration(&decl, &branches).unwrap();

        // ON-PATH: route prose visits ch-1 then r-1 (route's terminal).
        let mut on = AtomicStore::new();
        on.narrative_facts
            .insert("p".into(), nf("gt", "ch-1", None));
        on.narrative_facts.insert("q".into(), nf("gt", "r-1", None));
        let r = render_fidelity(&on, &order, "route");
        assert!(r.off_path.is_empty());
        assert!(r.reached_terminal, "r-1 is route's maximal node");

        // OFF-PATH: a fact at b-1 (the OTHER world's node) in route = drift.
        let mut off = AtomicStore::new();
        off.narrative_facts
            .insert("p".into(), nf("gt", "ch-1", None));
        off.narrative_facts
            .insert("bad".into(), nf("gt", "b-1", None));
        let r = render_fidelity(&off, &order, "route");
        assert_eq!(r.off_path.len(), 1);
        assert_eq!(r.off_path[0].coord, "b-1");

        // UNPLACED: an invented coordinate not named by any world.
        let mut un = AtomicStore::new();
        un.narrative_facts
            .insert("ghost".into(), nf("gt", "zzz", None));
        let r = render_fidelity(&un, &order, "route");
        assert_eq!(r.unplaced.len(), 1);
    }
}
