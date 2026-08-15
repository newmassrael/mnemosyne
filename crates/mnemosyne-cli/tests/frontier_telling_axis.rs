//! A store with no telling says so, instead of asking for one it does not have.
//!
//! Round 1215. `report-authoring-frontier` carries three telling-scoped axes —
//! unresolved quests, never-planned disclosures, and disclosures seated before
//! their fact is true — and each of them rendered `(pass --telling)` whenever no
//! telling was passed. That sentence has two readings and the report could not
//! tell them apart:
//!
//!   * the caller did not name a telling this store HAS, and should; or
//!   * the store declares none, so there is nothing to name.
//!
//! Against the authored population the second is the MAJORITY: 32 of the 43
//! corpora that load declare no disclosure plan at all. For those, the
//! instruction cannot be followed — a loop that reads it goes looking for a
//! telling, finds the registry empty, and has been told nothing about why the
//! axis is silent. The axis is not un-asked there, it is UNASKABLE, and an
//! absence that reads as an un-asked question is the shape R891 repaired one
//! axis over: "no transition rule ... so no map work can be pulled" is the map's
//! own third state, and the comment beside it says why a store that cannot know
//! its edges must never render like a store whose map is complete.
//!
//! So the telling axes get the same third state, and it lands in the REPORT
//! rather than only in the rendering — `tellings_declared` — because the loop
//! this frontier exists for reads `--json`.

use std::fs;
use std::path::Path;
use std::process::Command;

use tempfile::TempDir;

use crate::common;
use common::authored_stores;

fn cli_binary() -> &'static str {
    env!("CARGO_BIN_EXE_mnemosyne-cli")
}

fn run_ok(workspace: &Path, args: &[&str]) -> String {
    let out = Command::new(cli_binary())
        .args(args)
        .current_dir(workspace)
        .output()
        .expect("cli exec");
    assert!(
        out.status.success(),
        "{args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// What a store carries, as the Round 1217 need-axis reads it: the apparatus a
/// telling-scoped read projects, and which this axis therefore treats as
/// evidence that a telling is missing rather than merely absent.
#[derive(Clone, Copy, PartialEq)]
enum Carries {
    /// One scene, one fact — a flat record. No telling-scoped read is waiting.
    Nothing,
    /// A second world-line: the fork topology `report-playable-world` walks,
    /// and it takes `--telling` as a REQUIRED argument.
    AWorldLine,
    /// A quest, as the `quest_ids` kernel derives one — a typed `pursues` leg
    /// from an actor to a quest entity. `report-quest-graph` requires a telling
    /// too, and reaches the same refusal one layer down.
    AQuest,
}

/// A store with one scene, one fact, the named apparatus, and either a declared
/// telling or none.
fn workspace(carries: Carries, declare_telling: bool) -> TempDir {
    let tmp = TempDir::new().expect("tempdir");
    let ws = tmp.path();
    fs::create_dir_all(ws.join("docs/.atomic")).expect("mkdir");
    fs::write(ws.join("mnemosyne.toml"), "[workspace]\n").expect("config");
    fs::write(
        ws.join("docs/.atomic/workspace.atomic.json"),
        r#"{"schema_version":23,"sections":{},"changelog_entries":{}}"#,
    )
    .expect("seed");
    fs::write(
        ws.join("sections.json"),
        r#"[{"section_id":"sc-01","parent_doc":"d","title":"The well",
             "coverage_expectation":"informational"}]"#,
    )
    .expect("sections");
    let plans = if declare_telling {
        r#"[{"telling_id":"t-one","default_mode":"withhold","overrides":[]}]"#
    } else {
        "[]"
    };
    // The apparatus rides the SAME manifest as everything else, because that is
    // the authoring path an author takes: what the axis reads has to be a thing
    // an author writes, not a thing a test reaches around them to install.
    let (branches, entities, predicates, quest_fact) = match carries {
        Carries::Nothing => ("[]", "[]", "[]", String::new()),
        Carries::AWorldLine => (
            r#"[{"branch_id":"road-b","forks_from":"main","forks_at":"sc-01"}]"#,
            "[]",
            "[]",
            String::new(),
        ),
        Carries::AQuest => (
            "[]",
            r#"[{"entity_id":"e-scout"},{"entity_id":"q-relic"}]"#,
            r#"[{"predicate_id":"pursues","object_kind":"entity"}]"#,
            r#",
                {"fact_id":"f-quest","frame":"gt","claim":"The scout takes the errand",
                 "canon_from":"sc-01","evidence":["sc-01"],
                 "entities":["e-scout","q-relic"],
                 "typed":{"subject":"e-scout","predicate":"pursues",
                          "object":{"kind":"entity","id":"q-relic"}}}"#
                .to_string(),
        ),
    };
    fs::write(
        ws.join("facts.json"),
        format!(
            r#"{{
              "frames":[{{"frame_id":"gt"}}],
              "branches":{branches},
              "entities":{entities},
              "predicates":{predicates},
              "facts":[
                {{"fact_id":"f0","frame":"gt","claim":"The well is deep",
                  "canon_from":"sc-01","evidence":["sc-01"]}}{quest_fact}],
              "disclosure_plans":{plans}
            }}"#
        ),
    )
    .expect("facts");
    run_ok(ws, &["import-sections", "--manifest", "sections.json"]);
    run_ok(ws, &["import-facts", "--manifest", "facts.json"]);
    tmp
}

fn frontier_json(workspace: &Path, args: &[&str]) -> serde_json::Value {
    let mut whole = vec!["report-authoring-frontier", "--json"];
    whole.extend_from_slice(args);
    serde_json::from_str(&run_ok(workspace, &whole)).expect("frontier json")
}

#[test]
fn a_store_with_no_telling_says_so_instead_of_asking_for_one_it_does_not_have() {
    let tmp = workspace(Carries::Nothing, false);
    let report = frontier_json(tmp.path(), &[]);
    assert_eq!(
        report["tellings_declared"],
        serde_json::json!([]),
        "the registry is what says whether there is a telling to pass"
    );
    let human = run_ok(tmp.path(), &["report-authoring-frontier"]);
    assert!(
        human.contains("tellings declared: none — this store has no telling"),
        "the third state gets its own sentence:\n{human}"
    );
    // THE MIRROR, and the whole point: the instruction that cannot be followed
    // must not be printed here.
    assert!(
        !human.contains("(pass --telling)"),
        "a store with no telling must not be told to pass one:\n{human}"
    );
    for axis in [
        "unresolved quests",
        "never-planned disclosures",
        "disclosures seated before truth",
    ] {
        assert!(
            human.contains(&format!("{axis}: (no telling to pass)")),
            "`{axis}` must say which of the two silences it is:\n{human}"
        );
    }
}

#[test]
fn a_store_that_declares_a_telling_names_it_so_a_loop_can_pick_one() {
    let tmp = workspace(Carries::Nothing, true);
    let report = frontier_json(tmp.path(), &[]);
    assert_eq!(
        report["tellings_declared"],
        serde_json::json!(["t-one"]),
        "a loop reading json must not need a second read to learn what to pass"
    );
    let human = run_ok(tmp.path(), &["report-authoring-frontier"]);
    assert!(
        human.contains("tellings declared (1): t-one"),
        "the name is what makes `pass --telling` actionable:\n{human}"
    );
    assert!(
        human.contains("unresolved quests: (pass --telling)"),
        "and here the instruction CAN be followed:\n{human}"
    );

    // Passed, the axes answer — which is what makes the two silences above
    // silences about the ARGUMENT rather than about the store.
    let asked = frontier_json(tmp.path(), &["--telling", "t-one"]);
    assert_eq!(asked["telling"], "t-one");
    for axis in [
        "unresolved_quests",
        "never_planned_disclosures",
        "disclosures_seated_before_truth",
    ] {
        assert!(
            !asked[axis].is_null(),
            "`{axis}` must be present once a telling is given: {asked}"
        );
    }
    assert_eq!(
        asked["tellings_declared"],
        serde_json::json!(["t-one"]),
        "and the registry is reported whether or not one was passed"
    );
}

#[test]
fn both_states_are_real_in_the_corpora_this_repository_ships() {
    // NON-VACUITY OVER THE POPULATION, which is what makes the distinction worth
    // a field: a law whose second state no corpus reaches is a claim with no
    // evidence behind it (the R1041 shape).
    let (stores, skipped) = authored_stores();
    let mut declaring = 0usize;
    let mut without = 0usize;
    for store in &stores {
        let report = frontier_json(store.ws.path(), &[]);
        let tellings = report["tellings_declared"]
            .as_array()
            .unwrap_or_else(|| panic!("{}: the frontier reports the registry", store.name));
        if tellings.is_empty() {
            without += 1;
        } else {
            declaring += 1;
        }
    }
    println!(
        "  {} store(s) asked ({} do not load): {} declare a telling, {} declare none",
        stores.len(),
        skipped.len(),
        declaring,
        without
    );
    assert!(
        declaring > 0 && without > 0,
        "both states have to occur here or the distinction is untested: {declaring} declaring, \
         {without} without"
    );
    assert!(
        without > declaring,
        "the state this round exists for is the majority one, and if that stops being true the \
         sentence about it should be re-read: {without} without, {declaring} declaring"
    );
}

// ==========================================================================
// ROUND 1217 — the axis's FOURTH state: a telling the store's own apparatus
// requires and the store does not have.
//
// R1215 stopped the report from asking for a telling that cannot be passed, and
// left open whether the absence is WORK, on the ground that the answer is what
// each corpus was FOR and this repository does not record it. The store answers
// a narrower question that decides the same thing: what does it CARRY. A fork
// or a quest is apparatus that only a telling-scoped read projects, and both
// those reads take `--telling` as a REQUIRED argument — so the seam is not empty
// for such a store, it cannot be opened, and the work-list said `0 gap(s)`.
// ==========================================================================

/// The gauge, so a case can say the axis moved the loop's work-remaining number
/// rather than only its own field.
///
/// Read as a DIFFERENCE between two stores that differ in one authored thing,
/// never as an absolute: a store with no declared canon order counts every
/// fact-bearing scene as unordered, so an absolute here would be a number about
/// the fixture's order file and would pass while this axis contributed nothing.
fn total_gaps(report: &serde_json::Value) -> u64 {
    report["total_gaps"]
        .as_u64()
        .unwrap_or_else(|| panic!("the frontier counts its gaps: {report}"))
}

#[test]
fn a_flat_record_with_no_telling_is_not_told_it_needs_one() {
    // THE CONTROL, and the reason the axis reads apparatus rather than the
    // absence itself: a store with no fork and no quest has nothing a telling
    // would open, and calling that work would put the R1215 residue back as a
    // false positive on every record store in the tree.
    let tmp = workspace(Carries::Nothing, false);
    let report = frontier_json(tmp.path(), &[]);
    assert_eq!(report["tellings_declared"], serde_json::json!([]));
    assert_eq!(
        report["telling_needed"],
        serde_json::json!({"carried": [], "gap": false}),
        "a flat record is not missing a telling, it simply has none"
    );
    assert_eq!(
        total_gaps(&report),
        total_gaps(&frontier_json(
            workspace(Carries::Nothing, true).path(),
            &[]
        )),
        "and declaring a telling must not move the loop's work-remaining gauge for a store \
         that was never waiting on one"
    );
    let human = run_ok(tmp.path(), &["report-authoring-frontier"]);
    assert!(
        human.contains("telling needed: no — this store forks no world-line and holds no quest"),
        "the third state gets its own sentence here too:\n{human}"
    );
}

#[test]
fn a_store_that_forks_a_world_line_and_declares_no_telling_is_told_it_needs_one() {
    let tmp = workspace(Carries::AWorldLine, false);
    let report = frontier_json(tmp.path(), &[]);
    assert_eq!(
        report["telling_needed"],
        serde_json::json!({"carried": ["world-lines beyond main: 1"], "gap": true}),
        "the fork is what the playable-world read walks, and it is named rather than counted"
    );
    let human = run_ok(tmp.path(), &["report-authoring-frontier"]);
    assert!(
        human.contains("telling needed: yes — this store carries world-lines beyond main: 1"),
        "the sentence names WHAT makes the telling needed:\n{human}"
    );

    // THE SEAM ITSELF, asked rather than asserted: the read this gap is about
    // refuses this store, and that refusal is the whole claim.
    let refused = Command::new(cli_binary())
        .args(["report-playable-world"])
        .current_dir(tmp.path())
        .output()
        .expect("cli exec");
    assert!(
        !refused.status.success(),
        "report-playable-world must refuse a store with no telling to project under; if it \
         stopped requiring one, this axis is measuring a door that is no longer locked"
    );

    // THE INJECTION, in the direction an author would take it: declare the
    // telling and the gap closes, with nothing else moving.
    let with_telling = workspace(Carries::AWorldLine, true);
    let after = frontier_json(with_telling.path(), &[]);
    assert_eq!(
        after["telling_needed"],
        serde_json::json!({"carried": ["world-lines beyond main: 1"], "gap": false}),
        "the apparatus is still carried — what changed is that it can be projected"
    );
    assert_eq!(
        total_gaps(&report) - total_gaps(&after),
        1,
        "declaring the telling has to take EXACTLY one item off the loop's gauge — one call, \
         one item, and nothing else in the report moved with it"
    );
    let human = run_ok(with_telling.path(), &["report-authoring-frontier"]);
    assert!(
        human.contains("telling needed: satisfied"),
        "satisfied is its own state, not silence:\n{human}"
    );
}

#[test]
fn a_store_that_holds_a_quest_and_declares_no_telling_is_told_it_needs_one() {
    // THE SECOND ARM, and it is not the same claim: a quest reaches the refusal
    // through `report-quest-graph`, and the count comes from the `quest_ids`
    // kernel rather than from the branch registry. An axis proven on one arm
    // would be an axis whose other arm nothing has run.
    let tmp = workspace(Carries::AQuest, false);
    let report = frontier_json(tmp.path(), &[]);
    assert_eq!(
        report["telling_needed"],
        serde_json::json!({"carried": ["quests: 1"], "gap": true}),
        "the quest entity is derived, never marked — the R676 kernel is what counts it"
    );

    let refused = Command::new(cli_binary())
        .args(["report-quest-graph"])
        .current_dir(tmp.path())
        .output()
        .expect("cli exec");
    assert!(
        !refused.status.success(),
        "report-quest-graph must refuse a store with no telling: that refusal is what makes \
         a quest evidence of this need"
    );

    let with_telling = workspace(Carries::AQuest, true);
    let after = frontier_json(with_telling.path(), &[]);
    assert_eq!(
        after["telling_needed"],
        serde_json::json!({"carried": ["quests: 1"], "gap": false})
    );
    assert_eq!(
        total_gaps(&report) - total_gaps(&after),
        1,
        "the quest arm has to move the gauge by exactly one too"
    );
}

#[test]
fn the_corpora_this_repository_ships_reach_both_states_of_the_need() {
    // NON-VACUITY OVER THE POPULATION for the fourth state, in the shape the
    // R1041 lesson asks for — and the census the round exists to print: which of
    // the telling-less corpora are flat records, and which are stores a runtime
    // was meant to walk.
    let (stores, skipped) = authored_stores();
    let mut needed = Vec::new();
    let mut flat = Vec::new();
    let mut satisfied = 0usize;
    for store in &stores {
        let report = frontier_json(store.ws.path(), &[]);
        let need = &report["telling_needed"];
        let carried: Vec<String> = need["carried"]
            .as_array()
            .unwrap_or_else(|| panic!("{}: the need names what it read", store.name))
            .iter()
            .map(|v| v.as_str().unwrap_or_default().to_string())
            .collect();
        match (need["gap"].as_bool(), carried.is_empty()) {
            (Some(true), _) => needed.push(format!("{} [{}]", store.name, carried.join(", "))),
            (Some(false), true) => flat.push(store.name.clone()),
            (Some(false), false) => satisfied += 1,
            (None, _) => panic!("{}: `telling_needed.gap` is a bool", store.name),
        }
    }
    println!(
        "  {} store(s) asked ({} do not load): {} need a telling they do not have, {} carry no \
         apparatus that wants one, {} carry apparatus and declare one",
        stores.len(),
        skipped.len(),
        needed.len(),
        flat.len(),
        satisfied
    );
    for name in &needed {
        println!("    needs one: {name}");
    }
    assert!(
        !needed.is_empty() && !flat.is_empty(),
        "both states have to occur in the shipped corpora or this axis is untested against \
         anything an author wrote: {} needing, {} flat",
        needed.len(),
        flat.len()
    );
}
