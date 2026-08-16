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
    world: &mnemosyne_core::BranchId,
    order: &CanonOrder,
) -> Option<mnemosyne_core::SectionId> {
    let k = reveal.threshold.unwrap_or(1);
    if k == 0 {
        return None; // defensive — the write path normalizes Some(0) away
    }
    let mut coords: Vec<&mnemosyne_core::SectionId> = reveal.coords.iter().collect();
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
        Some(coords[k - 1].clone())
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
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
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
    /// A `withhold` override that ALSO carries a `first_at` pin — the pin does
    /// nothing, on either surface, and the author almost certainly meant a late
    /// reveal (Round 946). Sorted. Advisory, never gated.
    ///
    /// The projection drops a withheld fact before it can seat a locator, so no
    /// line ever renders at the pin; the premature-leak gate treats ANY match of
    /// a withheld fact as a leak and records `first_at: None`, so the pin does
    /// not move that verdict either. Two blind authors (Round 943) independently
    /// wrote `withhold` + `first_at` meaning "hidden until here, then told", and
    /// the store accepted the coordinate and did nothing with it. `state` with
    /// the same pin is the shape that discloses late.
    ///
    /// NOT a reject: the requirement would fail both recorded corpora, which is
    /// the Round 924 measurement that says speak rather than gate.
    pub inert_reveal_pins: Vec<InertRevealPin>,
}

/// A `withhold` override carrying a `first_at` pin — the shape two blind authors
/// wrote meaning "hidden until here, then told", which the store accepts and no
/// surface reads (Round 946, sharpened in Round 947).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct InertRevealPin {
    pub fact_id: String,
    pub world: String,
    /// The `first_at` coordinate set, joined — what the author pinned.
    pub pin: String,
    /// The seat the author ALREADY wrote (`surface.scene`), when they wrote one.
    ///
    /// Round 947 measured what actually moves a disclosure: the locator's seat is
    /// `surface.scene` when authored and `canon_from` otherwise, and `first_at`
    /// moves NEITHER — it is a premature-leak constraint on re-extracted prose.
    /// So an author who wants "true from scene nine, told at scene twenty" writes
    /// `state` with `surface.scene` at twenty. One recorded author wrote exactly
    /// that seat and lost it, because `withhold` skips the fact before the seat
    /// can be used: flipping that one field to `state` produces the reveal their
    /// sealed report described, at the scene they named, with nothing else
    /// changed. When this is `Some`, the store is one word away from the author's
    /// stated intent, and the advisory says so instead of giving generic advice.
    pub authored_seat: Option<String>,
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
    let mut inert_reveal_pins = Vec::new();
    for (id, ov) in &plan.overrides {
        if ov.mode != DisclosureMode::Withhold {
            continue;
        }
        for (world, reveal) in &ov.first_at {
            inert_reveal_pins.push(InertRevealPin {
                fact_id: id.to_string(),
                world: world.to_string(),
                pin: reveal
                    .coords
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("+"),
                authored_seat: ov.surface.as_ref().map(|s| s.scene.to_string()),
            });
        }
    }
    inert_reveal_pins.sort();
    for id in store.narrative_facts.keys() {
        // The single resolver (Round 510) — coverage cannot drift from the
        // carrier on the override-vs-default rule.
        match plan.effective_mode(id) {
            (DisclosureMode::Withhold, true) => hidden_by_design += 1,
            (DisclosureMode::Withhold, false) => never_planned.push(id.to_string()),
            (_, _) => disclosed += 1,
        }
    }
    Ok(DisclosureCoverageReport {
        telling: telling.to_string(),
        facts: store.narrative_facts.len(),
        disclosed,
        hidden_by_design,
        never_planned,
        inert_reveal_pins,
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
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
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
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
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
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct DisclosureLeakReport {
    pub telling: String,
    pub world: String,
    pub truth_frame: String,
    /// Plan-targeted facts checked for this world (withhold or `first_at[world]`;
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
    world: &mnemosyne_core::BranchId,
    truth_frame: &mnemosyne_core::FrameId,
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
        if &g.frame != truth_frame {
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
        let matches: Vec<(&mnemosyne_core::FactId, &mnemosyne_core::SectionId)> = reextracted
            .narrative_facts
            .iter()
            .filter(|(_, g)| &g.frame == truth_frame && g.typed.as_ref() == Some(typed))
            .map(|(gid, g)| (gid, &g.canon_from))
            .collect();
        if is_withhold {
            for (gid, coord) in matches {
                report.leaks.push(DisclosureLeak {
                    fact_id: fact_id.to_string(),
                    kind: LeakKind::Withhold,
                    reextracted_id: gid.to_string(),
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
            report.unmatched.push(fact_id.to_string());
        }
        for (gid, coord) in matches {
            let Some(pin) = pin.as_ref() else {
                report.unordered.push(DisclosureLeak {
                    fact_id: fact_id.to_string(),
                    kind: LeakKind::Unordered,
                    reextracted_id: gid.to_string(),
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
                    fact_id: fact_id.to_string(),
                    kind: LeakKind::Early,
                    reextracted_id: gid.to_string(),
                    coord: coord.to_string(),
                    first_at: Some(pin.to_string()),
                });
            } else if !order.le(world, pin, coord) {
                // neither direction => incomparable honesty surface (B-1).
                report.unordered.push(DisclosureLeak {
                    fact_id: fact_id.to_string(),
                    kind: LeakKind::Unordered,
                    reextracted_id: gid.to_string(),
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
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct RenderPathFact {
    pub fact_id: String,
    pub coord: String,
}

/// Render↔world-line fidelity report (Round 507, R505 input 1 — the prose
/// analog of R488 `FactCanonOffBranch`).
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
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
    world: &mnemosyne_core::BranchId,
) -> RenderFidelityReport {
    let nodes: BTreeSet<&mnemosyne_core::SectionId> = order.nodes().collect();
    let mut report = RenderFidelityReport {
        world: world.to_string(),
        reextracted_facts: reextracted.narrative_facts.len(),
        off_path: Vec::new(),
        unplaced: Vec::new(),
        reached_terminal: false,
    };
    for (id, g) in &reextracted.narrative_facts {
        let coord = &g.canon_from;
        if order.names(world, coord) {
            if order.is_maximal(world, coord) {
                report.reached_terminal = true;
            }
        } else if nodes.contains(coord) {
            report.off_path.push(RenderPathFact {
                fact_id: id.to_string(),
                coord: coord.to_string(),
            });
        } else {
            report.unplaced.push(RenderPathFact {
                fact_id: id.to_string(),
                coord: coord.to_string(),
            });
        }
    }
    report
}

/// The SINGLE-WORLD PROJECTION of a store — the shape [`render_fidelity`]
/// requires of its `--against` input (Round 1070).
///
/// The gate is single-world BY CONTRACT: it classifies every fact in the store
/// it is handed against ONE world's composed order, so a store spanning several
/// world-lines reads as off-path in bulk. That verdict is about the CALLER, not
/// about the prose — the authored corpus of this repository hands the gate 136
/// facts and draws 57 off-path at `main`, 39 / 38 / 37 at its three forks, with
/// no drift anywhere in it. The textbook fix is not to teach the gate about
/// branch tags (that would muddy its coordinate-based job) but to hand it the
/// world it expects, and this is the operation that does it.
///
/// **The selection is by the fact's DECLARED world; the classification
/// downstream is by its COORDINATE.** That those are two different declarations
/// is the whole reason the gate has anything to say. So the branch axis is
/// resolved through [`mnemosyne_core::world_membership`] — THE definition of
/// which branches are part of a world-line, the same one
/// [`crate::continuity::world_order_composition`] reads — and the departure
/// BOUNDS that membership carries are deliberately NOT applied. Applying them
/// is [`crate::continuity`]'s `visibility`, which decides a fact's membership by
/// comparing its `canon_from` against the bound: that is the gate's own
/// predicate, and a projection built on it would hand the gate back its own
/// answer and report clean forever.
///
/// A fact declared on a member branch but sitting at a coordinate this world
/// does not walk is therefore KEPT, and the gate names it off-path. That is the
/// disagreement the gate exists to find, not noise to filter out on its behalf.
///
/// The world lattice is a separate argument for the same reason
/// [`render_fidelity`] takes a separate [`CanonOrder`]: the store being
/// projected is PROSE, re-extracted, and need not carry the branch registry its
/// author wrote. The authored store is the authority on both.
///
/// Errs on a cyclic branch registry, which is where `world_membership` fails
/// loud rather than looping.
pub fn project_world(
    store: &AtomicStore,
    branches: &std::collections::BTreeMap<mnemosyne_core::BranchId, mnemosyne_core::Branch>,
    world: &mnemosyne_core::BranchId,
) -> Result<AtomicStore, String> {
    let members = mnemosyne_core::world_membership(branches, world)?;
    let mut out = store.clone();
    out.narrative_facts
        .retain(|_, fact| members.contains_key(&fact.branch));
    Ok(out)
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
        store.entities.insert("pike".into(), Entity::default());
        store.predicates.insert(
            "did".into(),
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
            subject: subject.into(),
            predicate: "did".into(),
            object: TypedObject::Token {
                token: value.to_string(),
            },
        }
    }

    fn nf(frame: &str, canon_from: &str, typed: Option<TypedClaim>) -> NarrativeFact {
        NarrativeFact {
            frame: frame.into(),
            branch: MAIN_BRANCH.into(),
            entities: vec![],
            claim: "c".to_string(),
            canon_from: canon_from.into(),
            canon_to: None,
            evidence: vec![mnemosyne_core::EvidenceRef::unreviewed(canon_from)],
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
                        (*b).into(),
                        DisclosureReveal {
                            coords: BTreeSet::from([(*c).into()]),
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
        let mut first_at: BTreeMap<mnemosyne_core::BranchId, DisclosureReveal> = BTreeMap::new();
        first_at.insert(
            branch.into(),
            DisclosureReveal {
                coords: coords.iter().map(|c| (*c).into()).collect(),
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
        overrides: BTreeMap<mnemosyne_core::FactId, DisclosureOverride>,
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
        let mut overrides: BTreeMap<mnemosyne_core::FactId, DisclosureOverride> = BTreeMap::new();
        overrides.insert("f-state".into(), ov(DisclosureMode::State, &[]));
        overrides.insert("f-hide".into(), ov(DisclosureMode::Withhold, &[]));
        store
            .disclosure_plans
            .insert("t".into(), plan(DisclosureMode::Withhold, overrides));
        let r = disclosure_coverage(&store, "t").unwrap();
        assert_eq!(r.facts, 3);
        assert_eq!(r.disclosed, 1);
        assert_eq!(r.hidden_by_design, 1);
        assert_eq!(r.never_planned, vec!["f-bare".to_string()]);
        assert!(disclosure_coverage(&store, "nope").is_err());
        assert!(
            r.inert_reveal_pins.is_empty(),
            "no override here carries a pin at all: {:?}",
            r.inert_reveal_pins
        );
    }

    /// A `withhold` OVERRIDE CARRYING A `first_at` PIN IS NAMED (Round 946).
    ///
    /// The pin does nothing on either surface: a withheld fact seats no locator,
    /// so no line renders at it, and the leak gate reports any match of a
    /// withheld fact as a leak with `first_at: None` regardless. Two blind
    /// authors (Round 943) independently wrote this shape meaning "hidden until
    /// here, then told", and the store took the coordinate and did nothing.
    ///
    /// THE THREE ARMS ARE THE GUARD. A withheld fact WITHOUT a pin and a `state`
    /// fact WITH one must both stay off the list, or "everything is inert" and
    /// "the pinned withholds are inert" are indistinguishable — and the second
    /// arm is the one that matters, because `state` + the same pin is exactly the
    /// shape this advisory tells the author to write instead.
    #[test]
    fn a_withheld_fact_that_also_pins_a_reveal_is_named_as_inert() {
        let mut store = AtomicStore::new();
        for id in ["f-pinned-hide", "f-plain-hide", "f-pinned-state"] {
            store
                .narrative_facts
                .insert(id.into(), nf("gt", "ch-1", Some(typed("pike", id))));
        }
        let mut overrides: BTreeMap<mnemosyne_core::FactId, DisclosureOverride> = BTreeMap::new();
        overrides.insert(
            "f-pinned-hide".into(),
            ov(DisclosureMode::Withhold, &[("main", "ch-3")]),
        );
        overrides.insert("f-plain-hide".into(), ov(DisclosureMode::Withhold, &[]));
        overrides.insert(
            "f-pinned-state".into(),
            ov(DisclosureMode::State, &[("main", "ch-3")]),
        );
        store
            .disclosure_plans
            .insert("t".into(), plan(DisclosureMode::Withhold, overrides));

        let r = disclosure_coverage(&store, "t").unwrap();
        assert_eq!(
            r.inert_reveal_pins,
            vec![InertRevealPin {
                fact_id: "f-pinned-hide".to_string(),
                world: "main".to_string(),
                pin: "ch-3".to_string(),
                authored_seat: None,
            }],
            "only the withheld fact that ALSO pins a reveal is inert"
        );
        // And the classification it already made is untouched — this is an added
        // reading of the same plan, not a re-verdict of it.
        assert_eq!(r.hidden_by_design, 2);
        assert_eq!(r.disclosed, 1);
    }

    /// THE SEAT THE AUTHOR ALREADY WROTE IS REPORTED BACK TO THEM (Round 947).
    ///
    /// `first_at` does not move a disclosure; `surface.scene` does (the locator
    /// seats there, and at `canon_from` when no surface is authored). A recorded
    /// author wrote `withhold` + `first_at` + `surface.scene` at the scene their
    /// sealed report named as the reveal — and `withhold` skipped the fact before
    /// the seat could be used. Flipping that one field to `state` produces the
    /// reveal they described, at the scene they named, with nothing else edited.
    ///
    /// The pair of arms is the guard: with a surface and without, in one plan.
    /// Without the second arm, "the seat is reported" and "a seat is invented for
    /// every row" would look the same.
    #[test]
    fn an_inert_pin_reports_the_seat_its_author_already_wrote() {
        let mut store = AtomicStore::new();
        for id in ["f-seated", "f-unseated"] {
            store
                .narrative_facts
                .insert(id.into(), nf("gt", "ch-1", Some(typed("pike", id))));
        }
        let mut seated = ov(DisclosureMode::Withhold, &[("main", "ch-9")]);
        seated.surface = Some(mnemosyne_core::DisclosureSurface {
            scene: "ch-9".into(),
            object: None,
        });
        let mut overrides: BTreeMap<mnemosyne_core::FactId, DisclosureOverride> = BTreeMap::new();
        overrides.insert("f-seated".into(), seated);
        overrides.insert(
            "f-unseated".into(),
            ov(DisclosureMode::Withhold, &[("main", "ch-9")]),
        );
        store
            .disclosure_plans
            .insert("t".into(), plan(DisclosureMode::Withhold, overrides));

        let r = disclosure_coverage(&store, "t").unwrap();
        let seat_of = |id: &str| {
            r.inert_reveal_pins
                .iter()
                .find(|p| p.fact_id == id)
                .unwrap_or_else(|| panic!("{id} is an inert pin"))
                .authored_seat
                .clone()
        };
        assert_eq!(
            seat_of("f-seated"),
            Some("ch-9".to_string()),
            "the author's own seat is handed back, not re-derived"
        );
        assert_eq!(
            seat_of("f-unseated"),
            None,
            "and no seat is invented for a row that never had one"
        );
    }

    /// THE CONTRACT MAY NOT TEACH THE SHAPE THIS MODULE REPORTS AS INERT (Round 966).
    ///
    /// Round 946 built the advisory and Round 947 sharpened it, and the
    /// `describe-schema` paragraph a blind author lands on went on presenting the
    /// flagged shape as THE idiom — "leave the mode `withhold` and pin `first_at`
    /// for THAT road only … never a non-withhold mode". That is Round 957's
    /// finding one axis over: the document teaching a model the tree had already
    /// retired, in the one paragraph an author reads. It is also the measured
    /// CAUSE of the advisory's own evidence — the two blind authors of Round 943
    /// who wrote `withhold` + `first_at` were reading this paragraph.
    ///
    /// THE ORACLE IS THIS MODULE, NOT A WORD LIST (the Round 907 pattern): both
    /// shapes go through `disclosure_coverage`, and the prose is held to what came
    /// back. If the classifier's verdict ever changes, these arms go red and force
    /// the paragraph to be re-read rather than silently diverging again.
    ///
    /// THE VERDICT WORD IS DERIVED FROM THE REPORT'S OWN FIELD NAME, not typed
    /// here (the Round 963 stem rule): rename `inert_reveal_pins` and the contract
    /// must follow it. Neither derived token — the verdict stem nor the seat field
    /// — appeared anywhere in the pre-repair paragraph, which is what makes this
    /// gate non-vacuous rather than a restatement of today's wording.
    ///
    /// WHAT THIS CANNOT DO, stated rather than implied: prose cannot be prevented
    /// from carrying a bad recipe alongside a good one. This holds the paragraph
    /// to naming the classifier's verdict and the field that actually seats a
    /// line; it cannot decide that every sentence around them is sound.
    #[test]
    fn the_contract_does_not_teach_the_shape_this_module_reports_as_inert() {
        // ARM 1 and ARM 2 in one plan: the shape the paragraph used to recommend,
        // and the shape the advisory recommends instead.
        let mut store = AtomicStore::new();
        for id in ["f-pinned-hide", "f-seated-state"] {
            store
                .narrative_facts
                .insert(id.into(), nf("gt", "ch-1", Some(typed("pike", id))));
        }
        let mut seated = ov(DisclosureMode::State, &[("main", "ch-9")]);
        seated.surface = Some(mnemosyne_core::DisclosureSurface {
            scene: "ch-9".into(),
            object: None,
        });
        let mut overrides: BTreeMap<mnemosyne_core::FactId, DisclosureOverride> = BTreeMap::new();
        overrides.insert(
            "f-pinned-hide".into(),
            ov(DisclosureMode::Withhold, &[("main", "ch-9")]),
        );
        overrides.insert("f-seated-state".into(), seated);
        store
            .disclosure_plans
            .insert("t".into(), plan(DisclosureMode::Withhold, overrides));

        let r = disclosure_coverage(&store, "t").unwrap();
        assert_eq!(
            r.inert_reveal_pins
                .iter()
                .map(|p| p.fact_id.as_str())
                .collect::<Vec<_>>(),
            vec!["f-pinned-hide"],
            "the withhold + first_at shape is the one this module calls inert"
        );
        assert_eq!(
            r.disclosed, 1,
            "and the disclosing mode is the one that reaches the reader"
        );

        // The verdict word, read off the report's own key rather than retyped.
        let json = serde_json::to_value(&r).expect("the report serializes");
        let key = json
            .as_object()
            .expect("a JSON object")
            .keys()
            .find(|k| k.ends_with("_reveal_pins"))
            .expect("the report still carries the reveal-pin roster")
            .clone();
        let verdict = key.split('_').next().expect("a non-empty key").to_string();

        let prose = crate::schema::describe_schema().disclosure_encoding;
        let lower = prose.to_lowercase();
        assert!(
            lower.contains(&verdict),
            "the paragraph must name the verdict `{verdict}` an author will get \
             back from the coverage report, or the report is the first place they \
             learn their reveal does nothing"
        );
        assert!(
            prose.contains("surface.scene"),
            "and it must name the field that actually seats the line, since that \
             is the repair the advisory hands back"
        );
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
        let mut overrides: BTreeMap<mnemosyne_core::FactId, DisclosureOverride> = BTreeMap::new();
        overrides.insert("w".into(), ov(DisclosureMode::Withhold, &[]));
        overrides.insert("e".into(), ov(DisclosureMode::State, &[("main", "ch-3")]));
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
        let r =
            disclosure_leak(&authored, &clean, &order, "t", &"main".into(), &"gt".into()).unwrap();
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
        let r =
            disclosure_leak(&authored, &leaky, &order, "t", &"main".into(), &"gt".into()).unwrap();
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
        let r = disclosure_leak(
            &authored,
            &belief,
            &order,
            "t",
            &"main".into(),
            &"gt".into(),
        )
        .unwrap();
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
            let mut overrides: BTreeMap<mnemosyne_core::FactId, DisclosureOverride> =
                BTreeMap::new();
            overrides.insert(
                "e".into(),
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
        let r = disclosure_leak(
            &authored,
            &match_at("r-1"),
            &order,
            "t",
            &"main".into(),
            &"gt".into(),
        )
        .unwrap();
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
        let r = disclosure_leak(
            &authored,
            &match_at("m-3"),
            &order,
            "t",
            &"main".into(),
            &"gt".into(),
        )
        .unwrap();
        assert!(
            r.leaks.is_empty(),
            "a match AFTER the first-reached trigger (between x-2 and b-4) is on time: {:?}",
            r.leaks
        );

        // THRESHOLD 2 (k=2): the effective pin is b-4 (the 2nd-earliest = last).
        let authored = build(Some(2));
        let r = disclosure_leak(
            &authored,
            &match_at("m-3"),
            &order,
            "t",
            &"main".into(),
            &"gt".into(),
        )
        .unwrap();
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
        let mut overrides: BTreeMap<mnemosyne_core::FactId, DisclosureOverride> = BTreeMap::new();
        overrides.insert("w".into(), ov(DisclosureMode::Withhold, &[]));
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
        let r = disclosure_leak(
            &authored,
            &foreign,
            &order,
            "t",
            &"main".into(),
            &"gt".into(),
        )
        .unwrap();
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
        let r = disclosure_leak(
            &authored,
            &shared,
            &order,
            "t",
            &"main".into(),
            &"gt".into(),
        )
        .unwrap();
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
                branch: mnemosyne_core::MAIN_BRANCH.into(),
                at: "ch-2".into(),
            }),
            ..Default::default()
        };
        let branches: BTreeMap<mnemosyne_core::BranchId, mnemosyne_core::Branch> =
            BTreeMap::from([
                ("route".into(), fork_at_ch2()),
                ("other".into(), fork_at_ch2()),
            ]);
        let order = CanonOrder::from_declaration(&decl, &branches).unwrap();

        // ON-PATH: route prose visits ch-1 then r-1 (route's terminal).
        let mut on = AtomicStore::new();
        on.narrative_facts
            .insert("p".into(), nf("gt", "ch-1", None));
        on.narrative_facts.insert("q".into(), nf("gt", "r-1", None));
        let r = render_fidelity(&on, &order, &"route".into());
        assert!(r.off_path.is_empty());
        assert!(r.reached_terminal, "r-1 is route's maximal node");

        // OFF-PATH: a fact at b-1 (the OTHER world's node) in route = drift.
        let mut off = AtomicStore::new();
        off.narrative_facts
            .insert("p".into(), nf("gt", "ch-1", None));
        off.narrative_facts
            .insert("bad".into(), nf("gt", "b-1", None));
        let r = render_fidelity(&off, &order, &"route".into());
        assert_eq!(r.off_path.len(), 1);
        assert_eq!(r.off_path[0].coord, "b-1");

        // UNPLACED: an invented coordinate not named by any world.
        let mut un = AtomicStore::new();
        un.narrative_facts
            .insert("ghost".into(), nf("gt", "zzz", None));
        let r = render_fidelity(&un, &order, &"route".into());
        assert_eq!(r.unplaced.len(), 1);
    }

    /// A fact on a named world — the branch axis the projection selects on,
    /// which `nf` cannot express because it defaults every fact to the spine.
    fn on_world(world: &str, canon_from: &str) -> NarrativeFact {
        NarrativeFact {
            branch: world.into(),
            ..nf("gt", canon_from, None)
        }
    }

    /// The trunk with two forks off it at `ch-2`, and a fork off one of THOSE —
    /// the shape that separates world-line MEMBERSHIP from "the world or the
    /// spine".
    fn forked_registry() -> BTreeMap<mnemosyne_core::BranchId, mnemosyne_core::Branch> {
        let fork = |from: &str, at: &str| mnemosyne_core::Branch {
            forks_from: Some(mnemosyne_core::BranchFork {
                branch: from.into(),
                at: at.into(),
            }),
            ..Default::default()
        };
        BTreeMap::from([
            ("route".into(), fork(mnemosyne_core::MAIN_BRANCH, "ch-2")),
            ("other".into(), fork(mnemosyne_core::MAIN_BRANCH, "ch-2")),
            ("leaf".into(), fork("route", "r-1")),
        ])
    }

    #[test]
    fn the_projection_is_what_makes_a_many_world_store_askable_at_all() {
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
        let branches = forked_registry();
        let order = CanonOrder::from_declaration(&decl, &branches).unwrap();

        let mut combined = AtomicStore::new();
        combined.narrative_facts.insert(
            "spine".into(),
            on_world(mnemosyne_core::MAIN_BRANCH, "ch-1"),
        );
        combined
            .narrative_facts
            .insert("r".into(), on_world("route", "r-1"));
        combined
            .narrative_facts
            .insert("o".into(), on_world("other", "b-1"));

        // HANDED THE WHOLE THING, the gate reports the sibling world as drift —
        // a verdict about the caller, since nothing in this store disagrees with
        // itself.
        let whole = render_fidelity(&combined, &order, &"route".into());
        assert_eq!(
            whole
                .off_path
                .iter()
                .map(|f| f.fact_id.as_str())
                .collect::<Vec<_>>(),
            ["o"],
            "the sibling world's fact is what a multi-world store draws off-path"
        );

        // PROJECTED, the same store answers clean, about something: the count is
        // the evidence the projection did not simply empty it.
        let projected =
            project_world(&combined, &branches, &"route".into()).expect("acyclic registry");
        let report = render_fidelity(&projected, &order, &"route".into());
        assert_eq!(report.reextracted_facts, 2, "the spine rides in with route");
        assert!(report.off_path.is_empty() && report.unplaced.is_empty());
        assert!(report.reached_terminal, "r-1 is route's maximal node");
    }

    #[test]
    fn the_projection_keeps_the_disagreement_the_gate_exists_to_find() {
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
        let branches = forked_registry();
        let order = CanonOrder::from_declaration(&decl, &branches).unwrap();

        // The drift itself: prose DECLARED on `route` that landed on `other`'s
        // node. Selecting by coordinate — which is what applying the departure
        // bounds would do — drops exactly this fact and reports a clean render.
        let mut drifted = AtomicStore::new();
        drifted
            .narrative_facts
            .insert("drift".into(), on_world("route", "b-1"));

        let projected =
            project_world(&drifted, &branches, &"route".into()).expect("acyclic registry");
        assert!(
            projected.narrative_facts.contains_key(&"drift".into()),
            "a fact whose declared world is a member SURVIVES the projection \
             however far off its coordinate sits — the projection selects on \
             the branch and the gate classifies on the coordinate, and a \
             projection that used the coordinate would answer clean forever"
        );
        let report = render_fidelity(&projected, &order, &"route".into());
        assert_eq!(
            report
                .off_path
                .iter()
                .map(|f| f.fact_id.as_str())
                .collect::<Vec<_>>(),
            ["drift"]
        );
    }

    #[test]
    fn membership_reaches_past_the_world_and_the_spine() {
        let branches = forked_registry();
        let mut store = AtomicStore::new();
        for (id, world) in [
            ("spine", mnemosyne_core::MAIN_BRANCH),
            ("mid", "route"),
            ("tip", "leaf"),
            ("sibling", "other"),
        ] {
            store
                .narrative_facts
                .insert(id.into(), on_world(world, "ch-1"));
        }

        let projected = project_world(&store, &branches, &"leaf".into()).expect("acyclic registry");
        assert_eq!(
            projected
                .narrative_facts
                .keys()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            ["mid", "spine", "tip"],
            "`leaf` forks off `route`, so route's own facts are part of its \
             world-line — a rule reading `this world or main` would drop them"
        );
    }

    #[test]
    fn a_cyclic_registry_is_refused_rather_than_projected() {
        let cycle = |from: &str| mnemosyne_core::Branch {
            forks_from: Some(mnemosyne_core::BranchFork {
                branch: from.into(),
                at: "ch-1".into(),
            }),
            ..Default::default()
        };
        let store = AtomicStore::new();
        let branches = BTreeMap::from([("a".into(), cycle("b")), ("b".into(), cycle("a"))]);
        assert!(
            project_world(&store, &branches, &"a".into()).is_err(),
            "a lineage that cannot be computed is not a projection of nothing"
        );
    }
}
