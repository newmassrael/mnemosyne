//! The map axis reaches the work-list — Round 891, end to end through the CLI.
//!
//! `report-authoring-frontier` (R589) calls itself "every coverage gap an
//! unattended loop pulls its next work from". The map axis was built after it —
//! R697 edges, R710 costs, R722 guards — and read at R875, and no round reached
//! back to the JOIN. So a store could register places with no way between them,
//! and the loop's work source would report its other gaps and stay silent on
//! this one: an absence that reads as health.
//!
//! What this file holds is the half a unit test cannot: that the frontier VERB
//! resolves the rules artifact and carries the axis, and that the no-rule case
//! gets its own sentence rather than rendering like a finished map. The
//! derivation itself is gated in `mnemosyne-validate` (the kind filter, the
//! R738 subkind, the negative control).

use std::fs;
use std::path::Path;
use std::process::Command;

use tempfile::TempDir;

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
        "{args:?} failed:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    String::from_utf8(out.stdout).expect("cli output is utf-8")
}

/// A two-place map plus a place it never reaches, seeded through the real
/// import verbs. `declare_map` chooses whether the transition rule exists —
/// the discriminating input for the three-state report.
fn workspace(declare_map: bool) -> TempDir {
    let tmp = TempDir::new().expect("tempdir");
    let ws = tmp.path();
    fs::create_dir_all(ws.join("docs/.atomic")).expect("mkdir");
    fs::write(
        ws.join("mnemosyne.toml"),
        "[workspace]\n[continuity]\nrules_path = \"narrative-rules.json\"\n",
    )
    .expect("config");
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

    let rules = if declare_map {
        r#"{"schema":"narrative-rules/v1","rules":[
             {"id":"roads","class":"transition","predicate":"pred-at",
              "adjacency":"adjacent","undirected":true}]}"#
    } else {
        r#"{"schema":"narrative-rules/v1","rules":[]}"#
    };
    fs::write(ws.join("narrative-rules.json"), rules).expect("rules");

    fs::write(
        ws.join("facts.json"),
        r#"{
          "frames":[{"frame_id":"gt"}],
          "branches":[],
          "entity_kinds":[{"kind_id":"place"},{"kind_id":"person"}],
          "entities":[
            {"entity_id":"ent-well","kind":"place"},
            {"entity_id":"ent-alley","kind":"place"},
            {"entity_id":"ent-vault","kind":"place"},
            {"entity_id":"ent-jiun","kind":"person"}],
          "predicates":[
            {"predicate_id":"pred-at","object_kind":"entity"},
            {"predicate_id":"adjacent","object_kind":"entity",
             "subject_kind":"place","object_entity_kind":"place"}],
          "facts":[
            {"fact_id":"p0","frame":"gt","claim":"Jiun stands at the well",
             "canon_from":"sc-01","evidence":["sc-01"],
             "entities":["ent-jiun","ent-well"],
             "typed":{"subject":"ent-jiun","predicate":"pred-at",
                      "object":{"kind":"entity","id":"ent-well"}}},
            {"fact_id":"e-well-alley","frame":"gt","claim":"The well opens on the alley",
             "canon_from":"sc-01","evidence":["sc-01"],
             "entities":["ent-well","ent-alley"],
             "typed":{"subject":"ent-well","predicate":"adjacent",
                      "object":{"kind":"entity","id":"ent-alley"}}}],
          "disclosure_plans":[]
        }"#,
    )
    .expect("facts");

    run_ok(ws, &["import-sections", "--manifest", "sections.json"]);
    run_ok(ws, &["import-facts", "--manifest", "facts.json"]);
    tmp
}

/// The verb carries the axis: a registered place no edge reaches arrives as
/// work, and it counts toward the gauge the loop reads.
#[test]
fn the_frontier_verb_pulls_the_unconnected_place_off_the_map() {
    let tmp = workspace(true);
    let report: serde_json::Value = serde_json::from_str(&run_ok(
        tmp.path(),
        &["report-authoring-frontier", "--json"],
    ))
    .expect("frontier json");
    let map = &report["map_frontier"];

    assert_eq!(map["transition_rules"], 1);
    assert_eq!(
        map["maps"][0]["unconnected_places"],
        serde_json::json!(["ent-vault"]),
        "the place the map never reaches is the work; the person is not a place \
         and the two connected places are not work: {map}"
    );
    // The gauge the loop actually reads must MOVE, or carrying the axis in the
    // payload while leaving the count behind is the same silence one field over.
    assert_eq!(map["total_gaps"], 1);
    // EXACT, not `>= 1`: this workspace declares no canon order, so `sc-01` is
    // already an unordered scene and a `>= 1` here would hold with the map gap
    // dropped entirely — the assertion would be true about the wrong number.
    assert_eq!(
        report["total_gaps"], 2,
        "one unordered scene plus the one map gap: {report}"
    );

    let human = run_ok(tmp.path(), &["report-authoring-frontier"]);
    assert!(
        human.contains("unconnected places [roads] (1): ent-vault"),
        "the human render must name the rule and the place:\n{human}"
    );
}

/// The third state. With no transition rule the store cannot know which facts
/// are edges (invariant 4), and that must not render as a finished map — the
/// R864 discipline, and the reason the axis stayed invisible for 300 rounds.
#[test]
fn a_store_with_no_transition_rule_says_so_instead_of_reporting_no_work() {
    let tmp = workspace(false);
    let human = run_ok(tmp.path(), &["report-authoring-frontier"]);
    assert!(
        human.contains("map: no transition rule declares an adjacency predicate"),
        "the inert case needs its own sentence:\n{human}"
    );
    assert!(
        !human.contains("unconnected places"),
        "with no map declared there is no place set to report on:\n{human}"
    );
    let report: serde_json::Value = serde_json::from_str(&run_ok(
        tmp.path(),
        &["report-authoring-frontier", "--json"],
    ))
    .expect("frontier json");
    assert_eq!(report["map_frontier"]["transition_rules"], 0);
    assert_eq!(report["map_frontier"]["maps"], serde_json::json!([]));
}
