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
use crate::frontier_axis_closures::closing_verb;
use common::{authored_stores, run, SIDECAR};

/// The verb the ROSTER says closes this axis (Round 1218), asserted to be the
/// one this law is about to call.
///
/// The point is not convenience. `describe-frontier-axes` publishes these verbs
/// to an unattended loop as calls this repository has RUN, and a published claim
/// nothing tests is the drift every table beside an API grows. Reading it here
/// makes the declaration the thing under test: rename the verb in the roster and
/// this law fails, which is the only way the two stay one statement.
fn the_roster_says(at: &Path, field: &str, verb: &str) {
    assert_eq!(
        closing_verb(at, field).as_deref(),
        Some(verb),
        "`describe-frontier-axes` must publish `{verb}` as what closes `{field}` — this law \
         runs that call, and it is the evidence behind the roster's `closes` state"
    );
}

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
        the_roster_says(workspace, "dangling_setups", "add-fact");
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

/// The frontier's whole answer, so a case can require an axis it does not touch
/// to hold still.
fn frontier(workspace: &Path, whose: &str) -> serde_json::Value {
    let out = run(workspace, &["report-authoring-frontier", "--json"]);
    assert!(out.status.success(), "{whose}: the frontier answers");
    serde_json::from_slice(&out.stdout).expect("frontier json")
}

/// One axis of it, as a list of ids.
fn axis(report: &serde_json::Value, key: &str) -> Vec<String> {
    report[key]
        .as_array()
        .unwrap_or_else(|| panic!("`{key}` is a list"))
        .iter()
        .map(|v| v.as_str().unwrap_or_default().to_string())
        .collect()
}

/// The second axis closed, and the one the census below says is non-empty.
///
/// R1216. A zero-fact scene is a registered section no fact anchors — the empty
/// room a loop is supposed to write into. The call it implies is `add-fact` at
/// that coordinate, and this requires it to work on the REAL ones: the scene
/// leaves the list, the two placement axes hold still, and — the part that
/// matters for a loop — closing this axis opens NO item on the payoff axis, so
/// clearing the work-list cannot be a treadmill that refills itself.
#[test]
fn every_zero_fact_scene_is_one_a_single_authored_call_fills() {
    let (stores, _) = authored_stores();
    let mut filled = 0usize;
    let mut stores_asked = 0usize;
    let mut no_frame = Vec::new();
    for store in &stores {
        let workspace = store.ws.path();
        let mut report = frontier(workspace, &store.name);
        let mut empty = axis(&report, "zero_fact_scenes");
        if empty.is_empty() {
            continue;
        }
        // A FACT NEEDS A FRAME, and the frame registry is the store's own. A
        // corpus that registers none cannot be authored into by any call, which
        // is a fact about that corpus rather than about this axis.
        let loaded = AtomicStore::load(&workspace.join(SIDECAR))
            .unwrap_or_else(|e| panic!("{}: the store must load back: {e}", store.name));
        let Some(frame) = loaded.frames.keys().next().map(ToString::to_string) else {
            no_frame.push(store.name.clone());
            continue;
        };
        stores_asked += 1;
        the_roster_says(workspace, "zero_fact_scenes", "add-fact");
        empty.sort();
        for (nth, scene) in empty.iter().enumerate() {
            let before = report.clone();
            let id = format!("frontier-fill-{nth}");
            let out = run(
                workspace,
                &[
                    "add-fact",
                    "--fact",
                    &id,
                    "--frame",
                    &frame,
                    "--claim",
                    "The empty room the frontier named is written into.",
                    "--canon-from",
                    scene,
                    "--evidence",
                    scene,
                ],
            );
            assert!(
                out.status.success(),
                "{}: the call this axis implies was refused at `{scene}` (frame {frame}):\n{}",
                store.name,
                String::from_utf8_lossy(&out.stderr)
            );
            report = frontier(workspace, &store.name);
            let mut expected = axis(&before, "zero_fact_scenes");
            expected.retain(|s| s != scene);
            assert_eq!(
                axis(&report, "zero_fact_scenes"),
                expected,
                "{}: filling `{scene}` had to remove it and leave the rest",
                store.name
            );
            // THE OTHER AXES HOLD STILL. A loop that closed one item and opened
            // another would clear nothing, and the two placement axes are the
            // ones a new fact could plausibly move.
            for other in ["unplaced_scenes", "unordered_scenes"] {
                assert_eq!(
                    axis(&report, other),
                    axis(&before, other),
                    "{}: filling `{scene}` moved `{other}`",
                    store.name
                );
            }
            assert_eq!(
                report["dangling_setups"], before["dangling_setups"],
                "{}: filling `{scene}` opened work on the payoff axis — a work-list that \
                 refills itself is a loop that never ends",
                store.name
            );
            filled += 1;
        }
        assert!(
            axis(&report, "zero_fact_scenes").is_empty(),
            "{}: scenes are still empty after every one the frontier named was filled",
            store.name
        );
    }
    println!(
        "  {} store(s) had empty scenes: {} filled one call each ({} skipped for registering no \
         frame: {:?})",
        stores_asked + no_frame.len(),
        filled,
        no_frame.len(),
        no_frame
    );
    assert!(
        filled > 0,
        "no corpus here holds an empty scene, so this law read an empty population"
    );
}

/// THE CONTROL for the axis above: it is the COORDINATE that fills the scene,
/// not the write.
///
/// Without it, that law passes on a frontier that clears an empty scene for any
/// fact landing anywhere — which would report a loop as filling rooms it never
/// entered.
#[test]
fn a_fact_seated_at_a_scene_that_already_has_one_leaves_the_empty_ones_empty() {
    let (stores, _) = authored_stores();
    let mut asked = 0usize;
    for store in &stores {
        let workspace = store.ws.path();
        let report = frontier(workspace, &store.name);
        let empty = axis(&report, "zero_fact_scenes");
        if empty.is_empty() {
            continue;
        }
        // A scene that is NOT on the list — the coverage census names one with
        // facts already anchored.
        let Some(occupied) = report["scene_coverage"]
            .as_array()
            .expect("the census is a list")
            .iter()
            .find(|row| !row["facts"].as_array().map(Vec::is_empty).unwrap_or(true))
            .and_then(|row| row["scene"].as_str())
            .map(ToString::to_string)
        else {
            continue;
        };
        let loaded = AtomicStore::load(&workspace.join(SIDECAR))
            .unwrap_or_else(|e| panic!("{}: the store must load back: {e}", store.name));
        let Some(frame) = loaded.frames.keys().next().map(ToString::to_string) else {
            continue;
        };
        let out = run(
            workspace,
            &[
                "add-fact",
                "--fact",
                "frontier-fill-control",
                "--frame",
                &frame,
                "--claim",
                "A fact in a room that already had one.",
                "--canon-from",
                &occupied,
                "--evidence",
                &occupied,
            ],
        );
        assert!(
            out.status.success(),
            "{}: the control fact must be authorable: {}",
            store.name,
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(
            axis(&frontier(workspace, &store.name), "zero_fact_scenes"),
            empty,
            "{}: a fact seated at `{occupied}` moved the empty-scene list, so the list reacts \
             to the write rather than to the coordinate",
            store.name
        );
        asked += 1;
        break;
    }
    assert_eq!(
        asked, 1,
        "the control needs one store with an empty scene and an occupied one"
    );
}

/// What each axis of the frontier is holding, across the authored population.
///
/// R1216 — R1214 closed ONE of the six axes and said so: "the other five each
/// need their own implied call, and nothing derives that call from the report".
/// Before closing a second one, this counts what is actually there, because an
/// axis no corpus reaches is a claim with no evidence behind it and an axis
/// holding hundreds of items is where a loop's time goes.
#[test]
fn the_frontier_says_how_much_of_each_axis_the_corpora_are_holding() {
    let (stores, skipped) = authored_stores();
    let mut zero_fact = 0usize;
    let mut unplaced = 0usize;
    let mut unordered = 0usize;
    let mut dangling_setups = 0usize;
    let mut map_gaps = 0usize;
    let mut stores_with_zero_fact = 0usize;
    let mut telling_needed = 0usize;
    for store in &stores {
        let out = run(store.ws.path(), &["report-authoring-frontier", "--json"]);
        assert!(out.status.success(), "{}: the frontier answers", store.name);
        let report: serde_json::Value = serde_json::from_slice(&out.stdout).expect("frontier json");
        let len = |key: &str| {
            report[key]
                .as_array()
                .map(Vec::len)
                .unwrap_or_else(|| panic!("{}: `{key}` is a list", store.name))
        };
        let here = len("zero_fact_scenes");
        if here > 0 {
            stores_with_zero_fact += 1;
        }
        zero_fact += here;
        unplaced += len("unplaced_scenes");
        unordered += len("unordered_scenes");
        dangling_setups += flat(&dangling(store.ws.path(), &store.name)).len();
        map_gaps += report["map_frontier"]["total_gaps"]
            .as_u64()
            .unwrap_or_default() as usize;
        // R1217's axis is a per-STORE bool rather than a list, so it is counted
        // in stores. A store is one `add-disclosure-plan` away from it whatever
        // else it is holding.
        if report["telling_needed"]["gap"]
            .as_bool()
            .unwrap_or_else(|| panic!("{}: `telling_needed.gap` is a bool", store.name))
        {
            telling_needed += 1;
        }
    }
    println!(
        "  {} store(s) asked ({} do not load): zero-fact scenes {} (in {} stores), unplaced {}, \
         unordered {}, dangling setups {}, map gaps {}, stores needing a telling {}",
        stores.len(),
        skipped.len(),
        zero_fact,
        stores_with_zero_fact,
        unplaced,
        unordered,
        dangling_setups,
        map_gaps,
        telling_needed
    );
    assert!(
        zero_fact > 0,
        "the axis this round closes has to be non-empty here or the case below proves nothing"
    );
}

/// The third axis closed, and the one that opens a READ rather than filling a
/// list.
///
/// R1217. `telling_needed.gap` says a store carries apparatus only a
/// telling-scoped read projects — a world-line fork, a quest — and declares no
/// telling to project it under. The call it implies is one
/// `add-disclosure-plan`, and this requires it against the real corpora: the
/// gap closes, the loop's gauge drops by exactly one, no other axis moves, and
/// — the part that makes the item worth naming — the seam that refused the
/// store before now answers. A work item whose closure opens nothing is a
/// chore; this one is the door to the runtime.
#[test]
fn every_store_that_needs_a_telling_is_one_a_single_authored_call_opens() {
    let (stores, skipped) = authored_stores();
    let mut satisfied = 0usize;
    let mut seams_opened = 0usize;
    for store in &stores {
        let workspace = store.ws.path();
        let before = frontier(workspace, &store.name);
        if !before["telling_needed"]["gap"]
            .as_bool()
            .unwrap_or_else(|| panic!("{}: `telling_needed.gap` is a bool", store.name))
        {
            continue;
        }
        the_roster_says(workspace, "telling_needed", "add-disclosure-plan");
        // The seam BEFORE, so the sentence below is about a door that was shut.
        // `--telling` is a required argument, so there is no way to ask.
        let shut = run(workspace, &["report-playable-world", "--telling", "r1217"]);
        assert!(
            !shut.status.success(),
            "{}: the store declares no telling, so the playable-world read cannot resolve one \
             — if it answered, this axis is naming a door that is open",
            store.name
        );

        let out = run(
            workspace,
            &[
                "add-disclosure-plan",
                "--telling",
                "r1217",
                "--default-mode",
                "withhold",
            ],
        );
        assert!(
            out.status.success(),
            "{}: the call this axis implies was refused:\n{}",
            store.name,
            String::from_utf8_lossy(&out.stderr)
        );

        let after = frontier(workspace, &store.name);
        assert_eq!(
            after["telling_needed"],
            serde_json::json!({
                "carried": before["telling_needed"]["carried"],
                "gap": false
            }),
            "{}: declaring the telling had to close the gap and leave the apparatus it names \
             exactly as it was",
            store.name
        );
        let gauge = |r: &serde_json::Value| r["total_gaps"].as_u64().expect("a gap count");
        assert_eq!(
            gauge(&before) - gauge(&after),
            1,
            "{}: one call has to take exactly one item off the loop's gauge",
            store.name
        );
        // NOTHING REFILLS, the R1216 cross-axis requirement one axis over: a
        // telling is a discourse declaration and must not create authoring work
        // on any axis that counts the fact base.
        for other in ["zero_fact_scenes", "unplaced_scenes", "unordered_scenes"] {
            assert_eq!(
                axis(&after, other),
                axis(&before, other),
                "{}: declaring a telling moved `{other}`",
                store.name
            );
        }
        assert_eq!(
            after["dangling_setups"], before["dangling_setups"],
            "{}: declaring a telling opened work on the payoff axis",
            store.name
        );

        // AND THE DOOR IS OPEN. This is what separates the item from a chore:
        // the read that refused the store two calls ago now projects it.
        let open = run(workspace, &["report-playable-world", "--telling", "r1217"]);
        assert!(
            open.status.success(),
            "{}: after the one call the frontier's item implies, the playable-world seam still \
             refuses:\n{}",
            store.name,
            String::from_utf8_lossy(&open.stderr)
        );
        seams_opened += 1;
        satisfied += 1;
    }
    println!(
        "  {} store(s) asked ({} do not load): {} needed a telling, one call each satisfied \
         them, and {} playable-world seam(s) that refused before answered after",
        stores.len(),
        skipped.len(),
        satisfied,
        seams_opened
    );
    assert!(
        satisfied > 0,
        "no authored corpus here needs a telling, so this law read an empty population"
    );
}

/// THE CONTROL for the axis above: it is the DECLARATION that closes the item,
/// not any write.
///
/// Without it, that law passes on a frontier that clears the need for any
/// mutation at all — which would report a loop as opening seams it never
/// touched. The same store, a write it can certainly take, and the need must
/// stand exactly where it was.
#[test]
fn a_write_that_declares_no_telling_leaves_the_need_exactly_where_it_was() {
    let (stores, _) = authored_stores();
    let mut asked = 0usize;
    for store in &stores {
        let workspace = store.ws.path();
        let before = frontier(workspace, &store.name);
        if !before["telling_needed"]["gap"].as_bool().unwrap_or(false) {
            continue;
        }
        let loaded = AtomicStore::load(&workspace.join(SIDECAR))
            .unwrap_or_else(|e| panic!("{}: the store must load back: {e}", store.name));
        let (Some(frame), Some(scene)) = (
            loaded.frames.keys().next().map(ToString::to_string),
            loaded.sections.keys().next().map(ToString::to_string),
        ) else {
            continue;
        };
        let out = run(
            workspace,
            &[
                "add-fact",
                "--fact",
                "telling-need-control",
                "--frame",
                &frame,
                "--claim",
                "A fact written into a store that still declares no telling.",
                "--canon-from",
                &scene,
                "--evidence",
                &scene,
            ],
        );
        assert!(
            out.status.success(),
            "{}: the control fact must be authorable: {}",
            store.name,
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(
            frontier(workspace, &store.name)["telling_needed"],
            before["telling_needed"],
            "{}: a write that declares no telling moved the need, so the axis reacts to the \
             write rather than to the declaration",
            store.name
        );
        asked += 1;
        // ONE STORE IS THE WHOLE CLAIM, and every further store pays another
        // import for the same sentence.
        break;
    }
    assert_eq!(
        asked, 1,
        "the control needs one store that needs a telling, and the population gave none"
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
