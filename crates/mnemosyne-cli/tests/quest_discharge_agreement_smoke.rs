//! Two shipped reads, one store, opposite answers about the same quest (R1037).
//!
//! Round 1037 went looking for the arc's next gate and the blind-authored
//! corpus refused two candidate rules in a row. The second refusal is the one
//! worth keeping: the store it rejected was the AUTHOR'S, and the reason it
//! looked broken is that this repository holds TWO derivations of "the player
//! finished this quest on this road".
//!
//! - `validate-continuity`'s prerequisite walk (R1031) reads the quest's
//!   VISIBLE `completed_by` facts. On the authored corpus it judges `q-main`
//!   discharged on three of the four roads, and names the coordinate.
//! - `report-quest-graph`'s per-world state (R559/R568) reads the R442 payoff
//!   coverage of the quest's GIVING setup — and R569 deliberately binds no
//!   giving when the completion fact carries no `pays_off` of its own (the
//!   split encoding the blind author used). With no giving, the quest reads
//!   `unknown` on every road, forever.
//!
//! So the gate says the main quest is finished at `sc-25c` and the runtime
//! projection says nobody can tell whether it was ever available. Both ship,
//! both exit 0, and the disagreement is about the MAIN quest of the only
//! blind-authored corpus this repository can load.
//!
//! This test states the agreement as a property over every (quest, road) pair
//! the two reads BOTH have an opinion about, so it stays true of a corpus that
//! grows a second `requires` edge rather than of these four rows.

mod common;
use common::{dnd_quest_facts, dnd_quest_workspace_from, json_report};

use std::collections::BTreeMap;

/// The telling the corpus declares — the graph read needs one.
const TELLING: &str = "delve";

#[test]
fn the_gate_and_the_runtime_projection_agree_on_which_roads_finish_a_quest() {
    let ws = dnd_quest_workspace_from(&dnd_quest_facts());

    // What the GATE says: a prerequisite judgement carries `quest_at` exactly
    // when this road discharges the subject quest (`satisfied` / `late`);
    // `inapplicable` is the walk's word for "this road never discharges it".
    let gate = json_report(ws.path(), &["validate-continuity"]);
    let mut discharged: BTreeMap<(String, String), Option<String>> = BTreeMap::new();
    for row in gate["quest_prerequisite_judgements"]
        .as_array()
        .expect("the walk emits one row per (edge, world, discharge)")
    {
        let quest = row["quest"].as_str().expect("quest id").to_string();
        let world = row["world"].as_str().expect("world id").to_string();
        let at = row["quest_at"].as_str().map(str::to_string);
        // A road can discharge a quest at more than one coordinate; any one of
        // them is a discharge, so an existing `Some` never loses to a `None`.
        discharged
            .entry((quest, world))
            .and_modify(|seen| {
                if seen.is_none() {
                    *seen = at.clone();
                }
            })
            .or_insert(at);
    }
    assert!(
        !discharged.is_empty(),
        "the authored corpus declares no prerequisite at all, so this walk has \
         nothing to compare and would pass vacuously"
    );

    // What the RUNTIME PROJECTION says: the quest's derived state per road.
    let graph = json_report(ws.path(), &["report-quest-graph", "--telling", TELLING]);
    let mut state: BTreeMap<(String, String), String> = BTreeMap::new();
    for node in graph["quests"].as_array().expect("quests array") {
        let quest = node["quest_id"].as_str().expect("quest id").to_string();
        for (world, per) in node["per_world"]
            .as_object()
            .expect("per_world object")
            .iter()
        {
            state.insert(
                (quest.clone(), world.clone()),
                per["state"].as_str().expect("state").to_string(),
            );
        }
    }

    // Print the whole table before asserting — a first-violation stop would
    // report one row of a comparison whose value is the distribution.
    println!("(quest, road): gate discharge vs projection state\n");
    let mut disagree: Vec<String> = Vec::new();
    for ((quest, world), at) in &discharged {
        let Some(projected) = state.get(&(quest.clone(), world.clone())) else {
            continue;
        };
        let gate_says = at.as_deref().unwrap_or("(not discharged here)");
        println!("  {quest} / {world}: gate={gate_says} projection={projected}");
        // The gate discharging this road and the projection calling it `done`
        // are the same claim. `open` and `unknown` are both "not finished
        // here", and the gate's `inapplicable` is the same statement.
        if at.is_some() != (projected == "done") {
            disagree.push(format!(
                "{quest} / {world}: the gate says {gate_says} and the quest \
                 graph says `{projected}`"
            ));
        }
    }

    assert_eq!(
        disagree,
        Vec::<String>::new(),
        "one store, two shipped reads, opposite answers about whether a road \
         finishes a quest — a runtime believing either one is being told \
         something the other denies"
    );
}
