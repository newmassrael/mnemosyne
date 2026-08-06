//! What a CORRUPTION is — the population definition's own oracle (Round 1061).
//!
//! [`common::corruptions`] derives the population that three surface walks
//! consume. Those walks measure the SURFACE, so not one of them can say whether
//! the population itself is sound: a leg that fills nothing, an edit that leaves
//! the corpus byte-identical, and two edits sharing one label all produce
//! perfectly reasonable-looking numbers. Round 1055 learned this shape on the
//! record definition — two rejected rules gave identical surface numbers, so the
//! numbers could not say which rule had shipped — and the answer was that a
//! definition carries its own test.
//!
//! Round 1061 widened that population from the fact manifest to the corpus an
//! author actually writes, which is what makes these questions load-bearing: the
//! scene registry and the canon order arrive at the store by two DIFFERENT
//! routes, and only one of them has an import that could refuse anything.

use std::collections::{BTreeMap, BTreeSet};

mod common;
use common::{
    audit_dir, corruptions, dnd_quest_manifests, read_sidecar, workspace_try, Manifests, LEGS,
    SIDECAR,
};

use mnemosyne_atomic::AtomicStore;

/// The legs this corpus cannot fill, and the measurement is the point rather
/// than the number: a leg that silently contributes nothing is shaped exactly
/// like a leg the surface ignores, so the zero is ASSERTED (the R1054 rule).
///
/// `sections.parent_doc` retargets a scene to another document the registry
/// already uses, and this registry uses one: all 56 scenes are filed under
/// `vael-mooren`. A second authored corpus with two documents fills it.
///
/// `facts.canon_to` was the first run's surprise, and it is why this report
/// exists. That leg has been in the derivation since Round 1054 and has never
/// produced a single corruption: NOT ONE of the manifest's 136 facts declares a
/// `canon_to`, so the guard that asks whether a fact HAS the leg has been
/// answering no for every fact, in every round, silently. Nine legs were
/// believed to be carrying the fact side; eight were.
///
/// Both are corpus facts, not surface facts. This constant is how a second
/// corpus announces that it fills one of them.
const LEGS_THIS_CORPUS_CANNOT_FILL: [&str; 2] = ["facts.canon_to", "sections.parent_doc"];

fn population() -> (Manifests, Vec<common::Corruption>) {
    let manifests = dnd_quest_manifests();
    let ws = workspace_try(&manifests, Some(&audit_dir())).expect("the authored corpus must load");
    let store = AtomicStore::load(&ws.path().join(SIDECAR)).expect("the imported store loads");
    let population = corruptions(&store, &manifests);
    (manifests, population)
}

/// The population reports its reach over a CLOSED leg vocabulary, so a leg the
/// corpus cannot fill is a zero on the report rather than an absence from it.
#[test]
fn every_leg_says_how_much_of_the_corpus_it_reached() {
    let (_, population) = population();
    let mut by_leg: BTreeMap<&str, usize> = LEGS.iter().map(|leg| (*leg, 0)).collect();
    for corruption in &population {
        *by_leg
            .get_mut(corruption.leg)
            .unwrap_or_else(|| panic!("`{}` is not in the leg vocabulary", corruption.leg)) += 1;
    }

    println!("{} corruptions over {} legs:", population.len(), LEGS.len());
    for (leg, count) in &by_leg {
        println!("  {count:>4}  {leg}");
    }

    let empty: BTreeSet<&str> = by_leg
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(leg, _)| *leg)
        .collect();
    assert_eq!(
        empty,
        LEGS_THIS_CORPUS_CANNOT_FILL.into_iter().collect(),
        "the legs this corpus fills with nothing are not the ones the walk says \
         it cannot fill — either a leg stopped deriving, or the corpus grew a \
         second value in a role and nobody said so"
    );

    // AND HOW MUCH each one reached, not only which reached nothing. The
    // injection sweep for this round found that half: reverting the composer
    // that judges a road edit doubled two legs and this test stayed green,
    // because it read the zeros and printed the rest. A number a report prints
    // and no assertion consults is the same dead data a walk that records a
    // read failure and never judges it produces.
    assert_eq!(
        by_leg,
        BTreeMap::from([
            ("facts.branch", 17),
            ("facts.canon_from", 17),
            ("facts.canon_to", 0),
            ("facts.evidence", 9),
            ("facts.frame", 17),
            ("facts.payoff_expectation", 3),
            ("facts.pays_off", 6),
            ("facts.typed.object", 11),
            ("facts.typed.predicate", 12),
            ("order.dropped", 27),
            ("order.from", 54),
            ("order.to", 27),
            ("sections.coverage_expectation", 56),
            ("sections.dropped", 56),
            ("sections.parent_doc", 0),
        ]),
        "how far each leg of the corpus this population reaches"
    );
}

/// A member of the population that does not change the corpus is not a
/// corruption. It would travel the whole walk, build a store identical to the
/// baseline's, and report that every read is blind to it.
#[test]
fn every_corruption_changes_the_corpus() {
    let (manifests, population) = population();
    let before = (
        manifests.sections.to_string(),
        manifests.order.to_string(),
        manifests.facts.to_string(),
    );
    for corruption in &population {
        let after = corruption.applied(&manifests);
        let after = (
            after.sections.to_string(),
            after.order.to_string(),
            after.facts.to_string(),
        );
        assert_ne!(
            before,
            after,
            "{} left the corpus byte-identical, so every walk over it would \
             report the surface as blind to an edit the store never received",
            corruption.label()
        );
    }
    println!(
        "{} corruptions, each of which changes the corpus",
        population.len()
    );
}

/// Two edits under one name merge wherever a walk keys its findings by the
/// label — and all three do. A scene sits on several roads, so this is not
/// hypothetical: `sc-22` is the tail of three forks at once.
#[test]
fn every_corruption_has_its_own_name() {
    let (_, population) = population();
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for corruption in &population {
        *seen.entry(corruption.label()).or_default() += 1;
    }
    let shared: Vec<(&String, &usize)> = seen.iter().filter(|(_, n)| **n > 1).collect();
    assert!(
        shared.is_empty(),
        "labels shared by more than one corruption: {shared:?}"
    );
    assert_eq!(seen.len(), population.len());
}

/// A ROAD EDIT LEAVES A ROAD the system will accept. `order.json` passes no
/// import, so a corruption it would refuse is refused by the READS instead —
/// twelve of the thirty shipped verbs at once, with one message.
///
/// This is the rule `swap_entity` serves on the fact side, where a claim
/// retarget carries the entities list because otherwise the import refuses and
/// "a refused corruption is not a move an author could make". Round 1061 paid
/// to learn it twice: choosing an alternative BY NAME gave 110 corruptions that
/// were each a two-cycle back to the road's root (`a cyclic declaration is no
/// order`), and the topological choice that fixed that still detached forks
/// from the trunk in 83 more (`a branch's edge set must ATTACH to the road it
/// rides in on`). Hand-listing invariants is how a population learns the third
/// one, so the judge is the shipped composer.
#[test]
fn a_retargeted_road_is_still_a_road() {
    let (manifests, population) = population();
    let ws = workspace_try(&manifests, Some(&audit_dir())).expect("the authored corpus must load");
    let store = AtomicStore::load(&ws.path().join(SIDECAR)).expect("the imported store loads");
    let mut checked = 0usize;
    for corruption in &population {
        if !corruption.leg.starts_with("order.") {
            continue;
        }
        let after = corruption.applied(&manifests);
        assert!(
            common::composes(&after.order, &store),
            "{} leaves a declaration the canon-order composer refuses, so the \
             corruption measures the refusal and not the edit",
            corruption.label()
        );
        checked += 1;
    }
    // AND WHAT THE COMPOSER TOOK AWAY, named rather than left as a smaller
    // number. Every edge offers three legs, so the sites are known; the gap is
    // the corruptions no author could ship, and in THIS corpus it has one
    // cause. Three forks hang off the trunk's last node, so any trunk step
    // re-pointed or removed detaches all three — which is why the road legs
    // that survive are the forks' own steps, plus the `from` retarget, which
    // moves a scene off the road without breaking the way to the fork.
    let mut edges = manifests.order["edges"]
        .as_array()
        .map_or(0, |road| road.len());
    for road in manifests.order["branches"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(_, road)| road)
    {
        edges += road.as_array().map_or(0, Vec::len);
    }
    let sites = edges * 3;
    println!(
        "{checked} road edits over {sites} road sites ({edges} edges x 3 legs); \
         {} sites have no candidate the composer accepts",
        sites - checked
    );
    assert_eq!(
        (edges, checked),
        (55, 108),
        "the road population moved without the reason moving with it"
    );
}

/// WHICH ROUTE EACH MANIFEST TAKES TO THE SURFACE, asked of the store rather
/// than read off the code.
///
/// `facts.json` and `sections.json` are IMPORTED: an edit to either reaches the
/// surface only by landing in the store, so if the store comes back identical
/// the import dropped the field on the floor — and every walk's silence about
/// that leg would be the import's silence rather than the surface's. This is
/// not a hypothetical failure: `SectionImport` carries `coverage_expectation`
/// as a `#[serde(default)]` field, and a manifest key that stopped being read
/// would deserialize to the default without a word.
///
/// `order.json` is not imported at all — it is read at READ time through the
/// workspace config — so a road edit MUST leave the store untouched. That is
/// the whole reason the walks over this population count read failures by name:
/// for a third of the legs, the store is not where the refusal can happen.
#[test]
fn an_imported_leg_reaches_the_store_and_a_road_leg_goes_around_it() {
    let (manifests, population) = population();
    let baseline_ws =
        workspace_try(&manifests, Some(&audit_dir())).expect("the authored corpus must load");
    let baseline = read_sidecar(baseline_ws.path());

    // One representative per leg — the first, so the choice is the derivation's
    // order and not this test's judgement.
    let mut representative: BTreeMap<&str, &common::Corruption> = BTreeMap::new();
    for corruption in &population {
        representative.entry(corruption.leg).or_insert(corruption);
    }

    for leg in LEGS {
        let Some(corruption) = representative.get(leg) else {
            assert!(
                LEGS_THIS_CORPUS_CANNOT_FILL.contains(leg),
                "`{leg}` filled nothing and is not declared unfillable here"
            );
            continue;
        };
        let built = workspace_try(&corruption.applied(&manifests), Some(&audit_dir()));
        let road = leg.starts_with("order.");
        match built {
            Err(refusal) => {
                assert!(
                    !road,
                    "`{leg}` was refused by an import ({refusal}), but the canon \
                     order passes no import — a road edit cannot be refused there"
                );
                println!("  {leg}: refused by the write path");
            }
            Ok(ws) => {
                let now = read_sidecar(ws.path());
                if road {
                    assert_eq!(
                        baseline,
                        now,
                        "`{}` changed the STORE, so the canon order is reaching \
                         the surface through the import after all and the walks \
                         are counting read failures for a route that no longer \
                         exists",
                        corruption.label()
                    );
                    println!("  {leg}: store untouched, as a read-time manifest must be");
                } else {
                    assert_ne!(
                        baseline,
                        now,
                        "`{}` left the store identical: the import read the \
                         manifest and dropped this field, so every walk that \
                         reports no read moves for it is reporting the import",
                        corruption.label()
                    );
                    println!("  {leg}: store moved");
                }
            }
        }
    }
}
