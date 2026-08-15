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

/// A store with one scene, one fact, and either a declared telling or none.
fn workspace(declare_telling: bool) -> TempDir {
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
    fs::write(
        ws.join("facts.json"),
        format!(
            r#"{{
              "frames":[{{"frame_id":"gt"}}],
              "facts":[
                {{"fact_id":"f0","frame":"gt","claim":"The well is deep",
                  "canon_from":"sc-01","evidence":["sc-01"]}}],
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
    let tmp = workspace(false);
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
    let tmp = workspace(true);
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
