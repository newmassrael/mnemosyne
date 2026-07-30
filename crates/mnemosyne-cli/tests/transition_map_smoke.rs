//! Round 875 — the map READ, end-to-end through the REAL binary.
//!
//! The write half of the map axis shipped in R710 (`add-edge-cost`) and
//! R722/R723 (`add-edge-guard`); the read half never did. Measured on the live
//! consumer: it authored 28 edges' worth of costs and guards through those
//! verbs, found no read that handed them back, and opened the store sidecar
//! itself — 1.2 MB parsed per start, later moved to a build-time bake, leaving
//! the store's shape bitten in TWO places on its side. R710 filed the write-only
//! axis as latency awaiting "the derived read"; R711 then established that THAT
//! read (minutes-within-a-tide) is not ours to build at all, because it needs a
//! domain number core must never know. The two are different reads, and this
//! one — plain carriage of what the store already holds — is what was missing.
//!
//! So the proof here is SUFFICIENCY: everything the consumer hand-parsed comes
//! back from one verb, and this test never opens the store file to get it.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn cli_binary() -> &'static str {
    env!("CARGO_BIN_EXE_mnemosyne-cli")
}

fn run(ws: &Path, args: &[&str]) -> std::process::Output {
    Command::new(cli_binary())
        .args(args)
        .current_dir(ws)
        .output()
        .expect("cli exec")
}

fn ok(ws: &Path, args: &[&str]) -> std::process::Output {
    let out = run(ws, args);
    assert!(out.status.success(), "{args:?}: {out:?}");
    out
}

fn json(ws: &Path, args: &[&str]) -> serde_json::Value {
    serde_json::from_slice(&ok(ws, args).stdout).expect("json")
}

/// The registries + the map rule. Facts, costs, and guards are authored through
/// the CLI in each test (the real write path).
fn write_workspace(ws: &Path, rules: serde_json::Value) {
    fs::create_dir_all(ws.join("docs/.atomic")).unwrap();
    fs::write(
        ws.join("mnemosyne.toml"),
        "[workspace]\n[continuity]\ncanon_order_path = \"canon-order.json\"\n\
         rules_path = \"narrative-rules.json\"\n",
    )
    .unwrap();
    let atomic = serde_json::json!({
        "schema_version": 32,
        "sections": { "ch-1": {}, "ch-2": {} },
        "changelog_entries": {},
        "frames": { "gt": {} },
        "entity_kinds": { "place": {}, "person": {} },
        "entities": {
            "hero": { "kind": "person" },
            "well": { "kind": "place" },
            "alley": { "kind": "place" },
            "shop": { "kind": "place" }
        },
        "predicates": {
            "adjacent": { "object_kind": "entity", "subject_kind": "place",
                          "object_entity_kind": "place" },
            "at-loc": { "object_kind": "entity" }
        },
        "units": { "minute": {} },
        "narrative_facts": {}
    });
    fs::write(
        ws.join("docs/.atomic/workspace.atomic.json"),
        serde_json::to_string_pretty(&atomic).unwrap(),
    )
    .unwrap();
    fs::write(
        ws.join("canon-order.json"),
        serde_json::json!({ "schema": "canon-order/v1",
            "edges": [["ch-1", "ch-2"]] })
        .to_string(),
    )
    .unwrap();
    fs::write(ws.join("narrative-rules.json"), rules.to_string()).unwrap();
}

fn map_rule() -> serde_json::Value {
    serde_json::json!({ "schema": "narrative-rules/v1", "rules": [
        { "id": "island-roads", "class": "transition", "predicate": "at-loc",
          "adjacency": "adjacent", "undirected": true }
    ]})
}

fn add_edge(ws: &Path, fact: &str, from: &str, to: &str) {
    ok(
        ws,
        &[
            "add-fact",
            "--fact",
            fact,
            "--frame",
            "gt",
            "--claim",
            "one is next to the other",
            "--canon-from",
            "ch-1",
            "--evidence",
            "ch-1",
            "--entities",
            &format!("{from},{to}"),
            "--typed-subject",
            from,
            "--typed-predicate",
            "adjacent",
            "--typed-object-entity",
            to,
        ],
    );
}

/// The round's claim: every datum the consumer hand-parsed out of the store —
/// the edge's declaring fact, its endpoints, its walk cost, its guard set and
/// K-of-N threshold — comes back from ONE read verb. This test authors the map
/// through the CLI and then reads it back through the CLI; it never opens
/// `workspace.atomic.json`, which is exactly the coupling the read removes.
#[test]
fn every_authored_cost_and_guard_reads_back_without_opening_the_store() {
    let tmp = TempDir::new().unwrap();
    let ws = tmp.path();
    write_workspace(ws, map_rule());

    add_edge(ws, "f-well-alley", "well", "alley");
    add_edge(ws, "f-alley-shop", "alley", "shop");
    // Two condition facts so a K-of-N guard has an N to be a K of.
    for (fid, claim) in [
        ("f-tide-out", "the tide is out"),
        ("f-lamp", "a lamp burns"),
    ] {
        ok(
            ws,
            &[
                "add-fact",
                "--fact",
                fid,
                "--frame",
                "gt",
                "--claim",
                claim,
                "--canon-from",
                "ch-1",
                "--evidence",
                "ch-1",
            ],
        );
    }
    ok(
        ws,
        &[
            "add-edge-cost",
            "--fact",
            "f-well-alley",
            "--n",
            "5",
            "--unit",
            "minute",
        ],
    );
    ok(
        ws,
        &[
            "add-edge-cost",
            "--fact",
            "f-alley-shop",
            "--n",
            "10",
            "--unit",
            "minute",
        ],
    );
    for cond in ["f-tide-out", "f-lamp"] {
        ok(
            ws,
            &[
                "add-edge-guard",
                "--fact",
                "f-well-alley",
                "--condition",
                cond,
            ],
        );
    }
    ok(
        ws,
        &[
            "set-edge-guard-threshold",
            "--fact",
            "f-well-alley",
            "--threshold",
            "1",
        ],
    );

    let report = json(ws, &["report-transition-map", "--json"]);
    assert_eq!(report["transition_rules"], 1, "{report}");
    let maps = report["maps"].as_array().unwrap();
    assert_eq!(maps.len(), 1, "{report}");
    let map = &maps[0];
    assert_eq!(map["rule"], "island-roads");
    assert_eq!(map["adjacency"], "adjacent");
    // Carried from the RULE — a consumer symmetrizes because the declaration
    // says to, not because it assumed maps are two-way (R697).
    assert_eq!(map["undirected"], true, "{map}");
    let nodes: Vec<&str> = map["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n.as_str().unwrap())
        .collect();
    assert_eq!(nodes, ["alley", "shop", "well"], "{map}");

    // The whole point: (edge, endpoints, cost) round-trips. The presence check
    // is separate so a dropped cost fails SAYING SO, not as an unwrap panic
    // three fields into a format string.
    for e in map["edges"].as_array().unwrap() {
        assert!(
            e.get("cost").is_some(),
            "an authored cost did not come back on its edge: {e}"
        );
    }
    let edges: BTreeSet<String> = map["edges"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            format!(
                "{}:{}->{}={} {}",
                e["fact_id"].as_str().unwrap(),
                e["from"].as_str().unwrap(),
                e["to"].as_str().unwrap(),
                e["cost"]["n"],
                e["cost"]["unit"].as_str().unwrap()
            )
        })
        .collect();
    assert_eq!(
        edges,
        BTreeSet::from([
            "f-well-alley:well->alley=5 minute".to_string(),
            "f-alley-shop:alley->shop=10 minute".to_string(),
        ]),
        "{map}"
    );

    let guarded = map["edges"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["fact_id"] == "f-well-alley")
        .unwrap();
    let conditions: Vec<&str> = guarded["guard"]["conditions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c.as_str().unwrap())
        .collect();
    assert_eq!(conditions, ["f-lamp", "f-tide-out"], "{guarded}");
    // K-of-N survives the read — a guard flattened to a bare AND would be a
    // DIFFERENT declaration, and the consumer evaluates it (R712 layering).
    assert_eq!(guarded["guard"]["threshold"], 1, "{guarded}");
    // The unguarded edge says so by omission, not by an empty guard object.
    let plain = map["edges"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["fact_id"] == "f-alley-shop")
        .unwrap();
    assert!(plain.get("guard").is_none(), "{plain}");

    // Nothing stray in a clean map — so the two lists below are not always-full.
    assert_eq!(report["unattached_costs"].as_array().unwrap().len(), 0);
    assert_eq!(report["unattached_guards"].as_array().unwrap().len(), 0);
}

/// What a naive bake would silently lose. A self-loop is NOT an edge (the gate
/// excludes it, so the read must too) and a cost keyed to a fact that is not an
/// edge of any declared map is not carried on any edge — but neither may simply
/// VANISH from the report: an authored fact absent with no reason given reads as
/// "never authored". Both are named, and the gate is asked about the same store
/// so the read cannot quietly disagree with it.
#[test]
fn a_self_loop_and_a_cost_on_a_non_edge_are_named_not_dropped() {
    let tmp = TempDir::new().unwrap();
    let ws = tmp.path();
    write_workspace(ws, map_rule());

    add_edge(ws, "f-well-alley", "well", "alley");
    // The self-loop is on a place NO real edge touches, so "a self-loop place
    // is still ON the map" is a claim this fixture can actually break: `shop`
    // reaches `nodes` through the self-loop or not at all. It lists its one
    // place ONCE (a duplicate entities ref rejects).
    ok(
        ws,
        &[
            "add-fact",
            "--fact",
            "f-shop-shop",
            "--frame",
            "gt",
            "--claim",
            "the shop is next to itself",
            "--canon-from",
            "ch-1",
            "--evidence",
            "ch-1",
            "--entities",
            "shop",
            "--typed-subject",
            "shop",
            "--typed-predicate",
            "adjacent",
            "--typed-object-entity",
            "shop",
        ],
    );
    // A fact that is not an edge at all, then a cost keyed to it.
    ok(
        ws,
        &[
            "add-fact",
            "--fact",
            "f-hero-walks",
            "--frame",
            "gt",
            "--claim",
            "the hero is at the well",
            "--canon-from",
            "ch-1",
            "--evidence",
            "ch-1",
            "--entities",
            "hero,well",
            "--typed-subject",
            "hero",
            "--typed-predicate",
            "at-loc",
            "--typed-object-entity",
            "well",
        ],
    );
    ok(
        ws,
        &[
            "add-edge-cost",
            "--fact",
            "f-hero-walks",
            "--n",
            "3",
            "--unit",
            "minute",
        ],
    );

    let report = json(ws, &["report-transition-map", "--json"]);
    let map = &report["maps"][0];
    let edge_ids: Vec<&str> = map["edges"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["fact_id"].as_str().unwrap())
        .collect();
    assert_eq!(edge_ids, ["f-well-alley"], "{map}");
    let loops = map["self_loops"].as_array().unwrap();
    assert_eq!(loops.len(), 1, "{map}");
    assert_eq!(loops[0]["fact_id"], "f-shop-shop");
    assert_eq!(loops[0]["node"], "shop");
    // A self-loop place is still ON the map (G2, R703): `shop` has no real
    // edge, so it is here only because the raw fact named it.
    let nodes: Vec<&str> = map["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n.as_str().unwrap())
        .collect();
    assert_eq!(nodes, ["alley", "shop", "well"], "{map}");
    let stray: Vec<&str> = report["unattached_costs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c.as_str().unwrap())
        .collect();
    assert_eq!(stray, ["f-hero-walks"], "{report}");

    // The gate, asked about the SAME store, names the same two things — the
    // read is the gate's findings without the gating, not a second opinion.
    let out = run(
        ws,
        &[
            "validate-continuity",
            "--rules",
            "narrative-rules.json",
            "--json",
        ],
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    let kinds: Vec<&str> = v["violations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["kind"].as_str().unwrap())
        .collect();
    assert!(kinds.contains(&"adjacency_self_loop"), "{v}");
    assert!(kinds.contains(&"edge_cost_not_an_edge"), "{v}");
}

/// The symmetry the read reports is the RULE's, not an assumption. Most
/// transition rules are one-way state machines and a blanket symmetrize would
/// admit resurrection (R697), so `undirected` is a declaration a consumer must
/// be handed rather than guess. Without a DIRECTED fixture the other test's
/// `undirected == true` would pass just as well against a hardcoded `true` —
/// this is the input that tells the two apart.
#[test]
fn a_directed_map_is_reported_directed() {
    let tmp = TempDir::new().unwrap();
    let ws = tmp.path();
    write_workspace(
        ws,
        serde_json::json!({ "schema": "narrative-rules/v1", "rules": [
            { "id": "one-way-stair", "class": "transition", "predicate": "at-loc",
              "adjacency": "adjacent", "undirected": false }
        ]}),
    );
    add_edge(ws, "f-well-alley", "well", "alley");
    let report = json(ws, &["report-transition-map", "--json"]);
    let map = &report["maps"][0];
    assert_eq!(map["undirected"], false, "{map}");
    // The edge is carried ONE way — the read never symmetrizes for the caller.
    let edges = map["edges"].as_array().unwrap();
    assert_eq!(edges.len(), 1, "{map}");
    assert_eq!(edges[0]["from"], "well");
    assert_eq!(edges[0]["to"], "alley");
    let human = String::from_utf8(ok(ws, &["report-transition-map"]).stdout).unwrap();
    assert!(human.contains("directed"), "{human}");
    assert!(!human.contains("undirected"), "{human}");
}

/// "No map is declared" and "the declared map has no edges" are different
/// answers and must not print alike (the R864 three-state discipline). With no
/// transition rule there IS no adjacency predicate, so the store genuinely
/// cannot know which facts are edges — invariant 4, the same reason the gate
/// goes inert. The fixture holds BOTH cases so neither assertion passes on an
/// absence.
#[test]
fn no_declared_map_is_a_different_answer_than_a_map_with_no_edges() {
    // (a) A rules file with no transition rule at all.
    let tmp = TempDir::new().unwrap();
    let ws = tmp.path();
    write_workspace(
        ws,
        serde_json::json!({ "schema": "narrative-rules/v1", "rules": [] }),
    );
    // An adjacency fact EXISTS — so an empty report cannot be blamed on an
    // empty store. It is unread because nothing declares it an edge.
    add_edge(ws, "f-well-alley", "well", "alley");
    let undeclared = json(ws, &["report-transition-map", "--json"]);
    assert_eq!(undeclared["transition_rules"], 0, "{undeclared}");
    assert_eq!(undeclared["maps"].as_array().unwrap().len(), 0);
    let human = String::from_utf8(ok(ws, &["report-transition-map"]).stdout).unwrap();
    assert!(
        human.contains("no transition rule declares an adjacency predicate"),
        "{human}"
    );

    // (b) The rule is declared; the store holds no adjacency fact.
    let tmp2 = TempDir::new().unwrap();
    let ws2 = tmp2.path();
    write_workspace(ws2, map_rule());
    let empty = json(ws2, &["report-transition-map", "--json"]);
    assert_eq!(empty["transition_rules"], 1, "{empty}");
    let maps = empty["maps"].as_array().unwrap();
    assert_eq!(maps.len(), 1, "{empty}");
    assert_eq!(maps[0]["edges"].as_array().unwrap().len(), 0);
    assert_eq!(maps[0]["nodes"].as_array().unwrap().len(), 0);
    let human2 = String::from_utf8(ok(ws2, &["report-transition-map"]).stdout).unwrap();
    assert!(
        !human2.contains("no transition rule declares an adjacency predicate"),
        "a declared-but-empty map must not print the no-rule sentence: {human2}"
    );
}
