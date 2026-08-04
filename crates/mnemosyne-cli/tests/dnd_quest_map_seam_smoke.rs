//! The authored map seam, loaded end to end — the fixture that stopped loading.
//!
//! `DisclosureSurface {scene, object}` has two halves. The DERIVED seat, where a
//! locator comes from the fact's own canon coordinate, is exercised by every
//! narrative store. The AUTHORED surface, where a writer names the scene and the
//! object a quest is given at, was authored exactly once — by the blind author of
//! the R562/R566 dnd-quest experiment.
//!
//! That store stopped loading and nobody noticed for ~150 rounds. Three schema
//! removals landed on it one at a time, and the store copies that would have
//! shown it are gitignored, so the tracked inputs could not rebuild it either.
//! Round 857 found it while being the first playable consumer and could only
//! record it: the map seam's end-to-end demonstration had no loadable fixture.
//!
//! Migrating the manifest is half the repair. THIS FILE is the other half — a
//! fixture CI never loads is a fixture that rots again. The provenance of every
//! migrated site is `tests/fixtures/dnd-quest/README.md`.
//!
//! What is asserted is what the ledger already recorded, so a change here is a
//! change to a published claim: R569's honest `unresolved` on `q-main` rather
//! than a scene-proximity rescue, and all 4 authored surfaces resolving on all 4
//! world-lines. Plus one datum no other store supplies — the quest `open`
//! verdict, which the first playable consumer's finished manuscripts never
//! produce.

use std::collections::{BTreeMap, BTreeSet};

mod common;
use common::{dnd_quest_workspace as rebuilt_workspace, json_report, run_ok};

/// The four surfaces the blind author placed on the map: scene, then object.
const AUTHORED_SURFACES: [(&str, &str); 4] = [
    ("sc-02", "reeve-hall"),
    ("sc-05", "lantern-house"),
    ("sc-07", "shrine"),
    ("sc-16", "vault-door"),
];

/// The world-lines the canon order composes.
const WORLD_LINES: [&str; 4] = ["claim", "main", "parley", "shatter"];

/// Each quest and the giving fact its surface hangs off — `q-main` deliberately
/// has none: its completion is split-encoded, and R569 chose an honest
/// `unresolved` over a proximity heuristic that could bleed givings across two
/// quests sharing a scene.
const QUEST_GIVINGS: [(&str, &[&str]); 4] = [
    ("q-delver", &["f-041"]),
    ("q-key", &["f-151"]),
    ("q-main", &[]),
    ("q-reliquary", &["f-060"]),
];

/// Every `(scene, object)` a locator names, over all world-lines.
fn surfaces_of(locators: &serde_json::Value) -> BTreeSet<(String, String)> {
    locators
        .as_array()
        .expect("locators array")
        .iter()
        .filter_map(|l| {
            let object = l.get("object")?.as_str()?;
            let scene = l["scene"].as_str().expect("locator scene");
            Some((scene.to_string(), object.to_string()))
        })
        .collect()
}

#[test]
fn the_migrated_dnd_store_loads_and_passes_the_continuity_gate() {
    let tmp = rebuilt_workspace();
    let stdout = run_ok(tmp.path(), &["validate-continuity"]);
    assert!(
        stdout.contains("violations: 0 (structural=0 interval=0)"),
        "the migration must not have introduced a continuity violation:\n{stdout}"
    );
    assert!(
        stdout.contains("facts=136 order_nodes=56/56 sections"),
        "the rebuild must hold the whole authored store:\n{stdout}"
    );
}

#[test]
fn all_four_authored_surfaces_resolve_on_every_world_line() {
    let tmp = rebuilt_workspace();
    let report = json_report(tmp.path(), &["report-playable-world", "--telling", "delve"]);
    let worlds = report["worlds"].as_object().expect("worlds map");

    let seen: BTreeSet<&str> = worlds.keys().map(String::as_str).collect();
    assert_eq!(
        seen,
        WORLD_LINES.into_iter().collect::<BTreeSet<_>>(),
        "the canon order must compose exactly the authored world-lines"
    );

    let expected: BTreeSet<(String, String)> = AUTHORED_SURFACES
        .iter()
        .map(|(s, o)| (s.to_string(), o.to_string()))
        .collect();

    for (world, body) in worlds {
        assert_eq!(
            surfaces_of(&body["locators"]),
            expected,
            "world `{world}` must seat every authored surface"
        );
        let manuscript = &body["manuscript"];
        for axis in ["unplaced_facts", "undecidable"] {
            let n = manuscript[axis].as_array().expect(axis).len();
            assert_eq!(n, 0, "world `{world}` reports {n} {axis}");
        }
    }
}

#[test]
fn the_quest_graph_reproduces_the_bindings_the_ledger_recorded() {
    let tmp = rebuilt_workspace();
    let report = json_report(tmp.path(), &["report-quest-graph", "--telling", "delve"]);

    let unresolved: Vec<&str> = report["unresolved_quests"]
        .as_array()
        .expect("unresolved array")
        .iter()
        .map(|v| v.as_str().expect("quest id"))
        .collect();
    assert_eq!(
        unresolved,
        ["q-main"],
        "R569 chose an honest unresolved on the split encoding over a heuristic"
    );

    let quests = report["quests"].as_array().expect("quests array");
    let givings: BTreeMap<&str, Vec<&str>> = quests
        .iter()
        .map(|q| {
            let id = q["quest_id"].as_str().expect("quest id");
            let facts = q["giving_facts"]
                .as_array()
                .expect("giving_facts")
                .iter()
                .map(|v| v.as_str().expect("fact id"))
                .collect();
            (id, facts)
        })
        .collect();
    let expected: BTreeMap<&str, Vec<&str>> = QUEST_GIVINGS
        .iter()
        .map(|(id, facts)| (*id, facts.to_vec()))
        .collect();
    assert_eq!(givings, expected, "every quest's giving binding is pinned");

    // A giving-bound quest must be SEATED, on every world-line — that join is
    // the seam this fixture exists to exercise.
    for quest in quests {
        let id = quest["quest_id"].as_str().expect("quest id");
        let surfaces = surfaces_of(&quest["locators"]);
        if givings[id].is_empty() {
            assert!(surfaces.is_empty(), "`{id}` has no giving, so no seat");
            continue;
        }
        assert_eq!(surfaces.len(), 1, "`{id}` is given at exactly one surface");
        let worlds: BTreeSet<&str> = quest["locators"]
            .as_array()
            .expect("locators")
            .iter()
            .map(|l| l["world_line"].as_str().expect("world_line"))
            .collect();
        assert_eq!(
            worlds,
            WORLD_LINES.into_iter().collect::<BTreeSet<_>>(),
            "`{id}` must be seated on every world-line"
        );
    }
}

#[test]
fn the_open_verdict_has_an_instance_here_and_the_census_is_pinned() {
    let tmp = rebuilt_workspace();
    let report = json_report(tmp.path(), &["report-quest-graph", "--telling", "delve"]);

    let mut census: BTreeMap<&str, usize> = BTreeMap::new();
    for quest in report["quests"].as_array().expect("quests array") {
        let per_world = quest["per_world"].as_object().expect("per_world map");
        assert_eq!(per_world.len(), WORLD_LINES.len(), "one verdict per world");
        for body in per_world.values() {
            *census
                .entry(body["state"].as_str().expect("state"))
                .or_default() += 1;
        }
    }

    // `open` is the verdict a quest LOG renders, and the reason this fixture is
    // worth keeping loadable: a store of FINISHED manuscripts yields none.
    assert_eq!(
        census,
        BTreeMap::from([("done", 7), ("open", 5), ("unknown", 4)]),
        "the per-world verdict census over 4 quests x 4 world-lines is pinned"
    );
}
