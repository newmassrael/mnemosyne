//! Every gap the frontier hands a loop is one a single authored call closes.
//!
//! `report-authoring-frontier` calls itself "every coverage gap an unattended
//! loop pulls its next work from" (R589), and two laws already hold its answer
//! against the reads beside it: `frontier_dangling_agreement` pins that it
//! carries exactly the roads `report-payoff-coverage` calls non-empty, and
//! `frontier_playable_agreement` pins what it counts per scene. Both ask
//! whether the work-list is CONSISTENT. Neither asks whether it is ACTIONABLE.
//!
//! That is the loop's inner step, and it is the one an unattended run lives or
//! dies on: an item nobody can close is a loop that spins on it forever, and
//! from outside it looks exactly like a loop that is working. R442's smoke
//! shows a setup dangling until `--pays-off` lands, on a store built for the
//! purpose — three facts, one road. What no round has asked is whether the
//! REAL items behave that way: the ones a blind extractor left in a 95-section
//! corpus, on a road that forks four ways, where the setup's coordinate and the
//! payoff's have to agree for the credit to land.
//!
//! So this walks the authored population and closes them. For each store, it
//! reads the frontier, and for each dangling setup it authors the payoff the
//! item implies — seated at the setup's OWN frame, branch and coordinate,
//! because that is what the frontier's answer gives a loop to work from — and
//! re-asks. The item must leave the list, every OTHER item must stay exactly as
//! it was, and when the last one is paid the store's dangling must be empty.
//!
//! ⚠ IT MUTATES NOTHING THE REPOSITORY TRACKS. `authored_stores()` imports each
//! corpus into a temporary workspace, and the payoffs land there. The corpora
//! are experiment evidence — R1191 sealed six of those documents and R1203
//! refused a rebuild for exactly this reason — so the bytes on disk are read
//! and never written.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use mnemosyne_atomic::AtomicStore;

use crate::common;
use common::{authored_stores, run, SIDECAR};

/// What each road still owes, by world.
fn dangling(workspace: &Path, whose: &str) -> BTreeMap<String, BTreeSet<String>> {
    let out = run(workspace, &["report-authoring-frontier", "--json"]);
    assert!(
        out.status.success(),
        "{whose}: the frontier must answer: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|e| panic!("{whose}: the frontier emitted no json ({e})"));
    let mut by_world = BTreeMap::new();
    let Some(worlds) = report["dangling_setups"].as_object() else {
        return by_world;
    };
    for (world, setups) in worlds {
        let ids: BTreeSet<String> = setups
            .as_array()
            .unwrap_or_else(|| panic!("{whose}: `dangling_setups.{world}` is a list"))
            .iter()
            .map(|id| {
                id.as_str()
                    .unwrap_or_else(|| panic!("{whose}: a setup id is a string, got {id}"))
                    .to_string()
            })
            .collect();
        by_world.insert(world.clone(), ids);
    }
    by_world
}

/// Every id the map names, whatever road it sits on.
fn flat(by_world: &BTreeMap<String, BTreeSet<String>>) -> BTreeSet<String> {
    by_world.values().flatten().cloned().collect()
}

/// The map with one setup removed from every road it was on — what the frontier
/// must answer after that setup is paid, and nothing else.
fn without(
    by_world: &BTreeMap<String, BTreeSet<String>>,
    setup: &str,
) -> BTreeMap<String, BTreeSet<String>> {
    by_world
        .iter()
        .map(|(world, ids)| {
            (
                world.clone(),
                ids.iter().filter(|id| *id != setup).cloned().collect(),
            )
        })
        // A ROAD THAT RUNS OUT OF WORK LEAVES THE MAP, which is the omission
        // `frontier_dangling_agreement` pins as the contract: the frontier
        // carries exactly the roads coverage calls non-empty.
        .filter(|(_, ids): &(String, BTreeSet<String>)| !ids.is_empty())
        .collect()
}

#[test]
fn every_gap_the_frontier_names_is_one_a_single_authored_call_closes() {
    let (stores, skipped) = authored_stores();
    let mut stores_with_work = 0usize;
    let mut closed = 0usize;
    let mut roads_emptied = 0usize;
    // TWO COUNTS OF THE SAME BACKLOG, because they are different questions and
    // the ledger has quoted the first as if it were the second. `sightings` is
    // what a walk over the roads sums — a setup visible on three roads is three
    // — and `closed` is how many CALLS it takes to clear them, which is one per
    // FACT however many roads see it.
    let mut sightings = 0usize;

    for store in &stores {
        let workspace = store.ws.path();
        let mut left = dangling(workspace, &store.name);
        if left.is_empty() {
            continue;
        }
        stores_with_work += 1;
        let roads_at_first = left.len();
        sightings += left.values().map(BTreeSet::len).sum::<usize>();

        // THE SETUP'S OWN SEAT is what the frontier's answer gives a loop: the
        // item names a fact, and the fact says which frame holds it, which
        // world-line it is on, and where it starts. A payoff seated anywhere
        // else is a coordinate this law invented.
        let loaded = AtomicStore::load(&workspace.join(SIDECAR))
            .unwrap_or_else(|e| panic!("{}: the store must load back: {e}", store.name));
        // KEYED BY THE STRING THE FRONTIER PRINTS. The store's key is a typed
        // id and the report's is JSON text; joining them through `as_str` is
        // the one place the two spellings meet.
        let facts: BTreeMap<&str, _> = loaded
            .narrative_facts
            .iter()
            .map(|(id, fact)| (id.as_str(), fact))
            .collect();

        let mut order: Vec<String> = flat(&left).into_iter().collect();
        order.sort();
        for (nth, setup) in order.iter().enumerate() {
            let fact = facts.get(setup.as_str()).copied().unwrap_or_else(|| {
                panic!(
                    "{}: the frontier names `{setup}` as work and the store holds no such \
                         fact — a work-list item nothing can act on",
                    store.name
                )
            });
            let payoff = format!("frontier-closure-{nth}");
            let out = run(
                workspace,
                &[
                    "add-fact",
                    "--fact",
                    &payoff,
                    "--frame",
                    fact.frame.as_str(),
                    "--branch",
                    fact.branch.as_str(),
                    "--claim",
                    "The gun the frontier named goes off.",
                    "--canon-from",
                    fact.canon_from.as_str(),
                    "--evidence",
                    fact.canon_from.as_str(),
                    "--pays-off",
                    setup,
                ],
            );
            assert!(
                out.status.success(),
                "{}: the payoff the frontier's item implies was refused for `{setup}` \
                 (frame {}, branch {}, at {}):\n{}",
                store.name,
                fact.frame,
                fact.branch,
                fact.canon_from,
                String::from_utf8_lossy(&out.stderr)
            );

            let expected = without(&left, setup);
            let now = dangling(workspace, &store.name);
            assert_eq!(
                now, expected,
                "{}: paying `{setup}` had to remove it from every road it was on and move \
                 nothing else",
                store.name
            );
            left = now;
            closed += 1;
        }

        assert!(
            left.is_empty(),
            "{}: the frontier still owes work after every item it named was paid: {left:?}",
            store.name
        );
        roads_emptied += roads_at_first;
    }

    println!(
        "  {} store(s) asked ({} could not be loaded), {} of them owed work across {} road(s): \
         {} sighting(s) of {} setup(s), and {} call(s) cleared every one",
        stores.len(),
        skipped.len(),
        stores_with_work,
        roads_emptied,
        sightings,
        closed,
        closed
    );
    assert!(
        sightings >= closed,
        "a setup is seen at least once on the roads that name it: {sightings} sighting(s) of \
         {closed} setup(s)"
    );
    // NON-VACUITY, asserted rather than hoped: this law is about real items, and
    // a population with none would report the same silence as a population that
    // closed cleanly.
    assert!(
        closed > 0,
        "no authored corpus in this tree owes the frontier any work, so nothing here was \
         closed — the law read an empty population"
    );
}

/// THE CONTROL: it is the `--pays-off` that closes the item, not the write.
///
/// Without this, the law above passes on a frontier that clears an item for any
/// fact landing at its coordinate — which would report the loop as closing work
/// it never did. The same seat, the same store, the same call minus the one
/// argument that names the setup.
#[test]
fn a_fact_that_names_no_setup_leaves_the_frontier_exactly_where_it_was() {
    let (stores, _) = authored_stores();
    let mut asked = 0usize;
    for store in &stores {
        let workspace = store.ws.path();
        let before = dangling(workspace, &store.name);
        if before.is_empty() {
            continue;
        }
        let loaded = AtomicStore::load(&workspace.join(SIDECAR))
            .unwrap_or_else(|e| panic!("{}: the store must load back: {e}", store.name));
        let facts: BTreeMap<&str, _> = loaded
            .narrative_facts
            .iter()
            .map(|(id, fact)| (id.as_str(), fact))
            .collect();
        let setup = flat(&before)
            .into_iter()
            .next()
            .expect("a road with work names a setup");
        let fact = facts[setup.as_str()];
        let out = run(
            workspace,
            &[
                "add-fact",
                "--fact",
                "frontier-closure-control",
                "--frame",
                fact.frame.as_str(),
                "--branch",
                fact.branch.as_str(),
                "--claim",
                "A fact at the same seat that pays off nothing.",
                "--canon-from",
                fact.canon_from.as_str(),
                "--evidence",
                fact.canon_from.as_str(),
            ],
        );
        assert!(
            out.status.success(),
            "{}: the control fact must be authorable: {}",
            store.name,
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(
            dangling(workspace, &store.name),
            before,
            "{}: a fact naming no setup moved the work-list, so the list reacts to the write \
             rather than to the payoff",
            store.name
        );
        asked += 1;
        // ONE STORE IS THE WHOLE CLAIM — that the read is not moved by an
        // unrelated write — and every further store pays another import for the
        // same sentence.
        break;
    }
    assert_eq!(
        asked, 1,
        "the control needs one store that owes work, and the population gave none"
    );
}
