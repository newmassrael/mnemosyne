//! A declared prerequisite is a promise the road must keep (Round 1031).
//!
//! `QuestNode::prerequisites` has carried a `requires` edge to every consumer
//! since Round 568 under the words "the canon order proves the timing". Nothing
//! proved it. This file is both halves of that finding: the MEASUREMENT that the
//! class was uncovered, and the proof that it now is.
//!
//! The corruption is one an author could commit, not one hand-written into the
//! store: it goes through `import-facts`, the same manifest path the blind
//! author used, and it moves ONE field — the `requires` object of `f-153` from
//! `q-key` to `q-delver`. That single edit turns "the vault needs the warden's
//! key" into "the vault needs the lantern-boy's errand", and the lantern-boy's
//! errand closes at `sc-30p` / `sc-31s` — AFTER the vault does, on the two roads
//! that close it at all, and never on `claim`.
//!
//! The store stays skeleton-clean under it: `import-facts` accepts it (the entity
//! list moves with the leg), `validate-workspace` passes, and both projections a
//! runtime reads — `report-quest-graph` and `report-playable-world` — render it
//! without complaint and hand the runtime a gate no road can open. That is the
//! third destination's class exactly: consistent store, unplayable game.

use std::collections::BTreeSet;
use std::path::Path;

mod common;
use common::{dnd_quest_facts, dnd_quest_workspace, dnd_quest_workspace_from, json_report, run};

/// The blind author's single `requires` edge and the fact that carries it.
const REQUIRES_FACT: &str = "f-153";
const DECLARED_PREREQUISITE: &str = "q-key";
/// A quest whose own discharge is LATER than the dependent's on the two roads
/// that discharge both, and absent on a third — one substitution, three shapes.
const LATE_PREREQUISITE: &str = "q-delver";

/// Re-point the `requires` object, carrying the entities list with it — which
/// the import gate requires, and which is why this is an authorable defect
/// rather than a corrupt file: without the second half `import-facts` rejects it.
fn facts_with_late_prerequisite() -> serde_json::Value {
    let mut facts = dnd_quest_facts();
    let mut applied = 0usize;
    for fact in facts["facts"].as_array_mut().expect("facts array") {
        if fact["fact_id"] != REQUIRES_FACT {
            continue;
        }
        assert_eq!(
            fact["typed"]["object"]["id"], DECLARED_PREREQUISITE,
            "the fixture's `requires` object moved; this injection is describing a store \
             that no longer exists"
        );
        fact["typed"]["object"]["id"] = LATE_PREREQUISITE.into();
        let entities = fact["entities"].as_array_mut().expect("entities array");
        entities.retain(|e| e != DECLARED_PREREQUISITE);
        entities.push(LATE_PREREQUISITE.into());
        applied += 1;
    }
    assert_eq!(
        applied, 1,
        "the injection must apply exactly once — an injection that applied nowhere passes \
         every assertion below for the wrong reason"
    );
    facts
}

fn continuity(workspace: &Path) -> (bool, serde_json::Value) {
    let out = run(workspace, &["validate-continuity", "--json"]);
    let report = serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "continuity json: {e}\n{}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
    });
    (out.status.success(), report)
}

fn verdicts(report: &serde_json::Value) -> Vec<(String, String, String, String)> {
    report["quest_prerequisite_judgements"]
        .as_array()
        .expect("judgements are always populated")
        .iter()
        .map(|j| {
            (
                j["quest"].as_str().expect("quest").to_string(),
                j["prerequisite"]
                    .as_str()
                    .expect("prerequisite")
                    .to_string(),
                j["world"].as_str().expect("world").to_string(),
                j["verdict"].as_str().expect("verdict tag").to_string(),
            )
        })
        .collect()
}

/// The authored store keeps its promise, and the gate SAYS so on every road —
/// the non-vacuity half. A gate whose only evidence is that it stays quiet on a
/// clean store is indistinguishable from one that never ran.
#[test]
fn the_authored_store_keeps_its_one_declared_prerequisite_on_every_road() {
    let tmp = dnd_quest_workspace();
    let (ok, report) = continuity(tmp.path());
    assert!(ok, "the authored store must still pass: {report}");

    let judged = verdicts(&report);
    assert_eq!(
        judged,
        vec![
            // Rows come in the store's world order, the trunk first — `main`
            // never discharges `q-main`, so its declared gate is never opened
            // there and there is nothing to prove.
            (
                "q-main".into(),
                "q-key".into(),
                "main".into(),
                "inapplicable".into()
            ),
            // `claim` / `parley` / `shatter` each discharge `q-main`, and each
            // discharges `q-key` at `sc-17`, up on the shared trunk before the fork.
            (
                "q-main".into(),
                "q-key".into(),
                "claim".into(),
                "satisfied".into()
            ),
            (
                "q-main".into(),
                "q-key".into(),
                "parley".into(),
                "satisfied".into()
            ),
            (
                "q-main".into(),
                "q-key".into(),
                "shatter".into(),
                "satisfied".into()
            ),
        ],
        "the one authored `requires` edge is judged on all four world-lines"
    );
}

/// The census the gate PRINTS, on both of its arms — a store that declares a
/// prerequisite, and one that declares none.
///
/// MEASURED: deleting the whole line from the CLI left all 1743 tests green.
/// A number the shipping report states and no program reads is a number free to
/// drift from the walk it claims to summarize, and `not-declared` is the half
/// that matters most: a store with no declared gate and a store whose every gate
/// is kept both find nothing, and only one of them was measured.
#[test]
fn the_printed_census_says_what_was_judged_and_says_when_nothing_could_be() {
    let tmp = dnd_quest_workspace();
    let stdout = String::from_utf8(run(tmp.path(), &["validate-continuity"]).stdout)
        .expect("cli output is utf-8");
    assert!(
        stdout.contains(
            "quest prerequisites: 4 judged (satisfied 3 / inapplicable 1 / unverifiable 0)"
        ),
        "the authored store's census must print verbatim:\n{stdout}"
    );

    // The same store with its ONE `requires` fact removed: still four quests,
    // still four roads, and now nothing the class can be asked about.
    let mut facts = dnd_quest_facts();
    let kept = facts["facts"].as_array_mut().expect("facts array");
    let before = kept.len();
    kept.retain(|f| f["fact_id"] != REQUIRES_FACT);
    assert_eq!(
        before - kept.len(),
        1,
        "exactly the one `requires` fact leaves"
    );

    let bare = dnd_quest_workspace_from(&facts);
    let stdout = String::from_utf8(run(bare.path(), &["validate-continuity"]).stdout)
        .expect("cli output is utf-8");
    assert!(
        stdout.contains("quest prerequisites: not-declared judged"),
        "a store with no `requires` claim must say so rather than print a 0 that reads \
         like a pass:\n{stdout}"
    );
}

/// The MEASUREMENT this round started from: with the new gate's own findings
/// removed, the corrupted store is invisible. Every other station passes it.
#[test]
fn every_other_station_ships_the_broken_gate_without_complaint() {
    let tmp = dnd_quest_workspace_from(&facts_with_late_prerequisite());
    let ws = tmp.path();

    // `import-facts` already ran inside the builder and would have panicked —
    // so the authoring path accepts this store. The rest of the roster:
    for verb in [
        vec!["validate-workspace"],
        vec!["report-quest-graph", "--telling", "delve"],
        vec!["report-playable-world", "--telling", "delve"],
    ] {
        let out = run(ws, &verb);
        assert!(
            out.status.success(),
            "{verb:?} must still pass — this test's claim is that ONLY the continuity gate \
             sees this defect:\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    // And the quest projection carries the broken gate to the runtime verbatim,
    // which is how an unplayable world would have shipped.
    let graph = json_report(ws, &["report-quest-graph", "--telling", "delve"]);
    let carried: Vec<&str> = graph["quests"]
        .as_array()
        .expect("quests")
        .iter()
        .filter(|q| q["quest_id"] == "q-main")
        .flat_map(|q| q["prerequisites"].as_array().expect("prerequisites"))
        .map(|p| p.as_str().expect("quest id"))
        .collect();
    assert_eq!(
        carried,
        [LATE_PREREQUISITE],
        "the projection hands the runtime the prerequisite it was told, judging nothing"
    );

    // The continuity gate is the ONE station that rejects it.
    let (ok, report) = continuity(ws);
    assert!(!ok, "the continuity gate must reject: {report}");
}

/// Both violation shapes, from one substitution — and each names the road, the
/// coordinate that discharges the quest, and (for `late`) the coordinate that
/// was supposed to come first.
#[test]
fn the_gate_names_the_road_and_the_two_ways_a_road_breaks_the_promise() {
    let tmp = dnd_quest_workspace_from(&facts_with_late_prerequisite());
    let (_, report) = continuity(tmp.path());

    let found: BTreeSet<(String, String, String, String)> = report["violations"]
        .as_array()
        .expect("violations")
        .iter()
        .filter(|v| v["kind"] == "quest_prerequisite_unreachable")
        .map(|v| {
            (
                v["world"].as_str().expect("world").to_string(),
                v["shape"].as_str().expect("shape").to_string(),
                v["quest_at"].as_str().expect("quest_at").to_string(),
                v["prerequisite_at"]
                    .as_str()
                    .unwrap_or("-none-")
                    .to_string(),
            )
        })
        .collect();
    assert_eq!(
        found,
        BTreeSet::from([
            // `claim` closes the vault and never runs the lantern-boy's errand.
            (
                "claim".into(),
                "never".into(),
                "sc-25c".into(),
                "-none-".into()
            ),
            // `parley` / `shatter` run it, but only after the vault is resolved.
            (
                "parley".into(),
                "late".into(),
                "sc-25p".into(),
                "sc-30p".into()
            ),
            (
                "shatter".into(),
                "late".into(),
                "sc-25s".into(),
                "sc-31s".into()
            ),
        ]),
        "one substitution produces both shapes, each pinned to its own road"
    );

    // `main` never discharges `q-main`, so it is judged and NOT accused: a gate
    // that fired on the trunk would be accusing a road of breaking a promise it
    // never made.
    assert!(
        verdicts(&report).contains(&(
            "q-main".into(),
            LATE_PREREQUISITE.into(),
            "main".into(),
            "inapplicable".into()
        )),
        "the trunk is judged inapplicable, not accused: {report}"
    );
}
