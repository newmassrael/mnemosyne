//! Round 720 — the VERIFIED GUARDS proof, end-to-end through the REAL binary.
//!
//! The owner's design point (the reason guards exist): MNEMOSYNE holds the FULL
//! branching of a place-access guard — the "you have the key" story AND the "you
//! don't" story, as two authored world-lines — while the GAME only evaluates the
//! boolean ("has key?") and follows the branch Mnemosyne already authored. This
//! test WRITES that little novel and PROVES it: a hall adjacent to a vault
//! (a door edge), guarded by a key condition, forking into a got-key world-line
//! (the hero enters the vault) and a no-key world-line (the hero is stuck). It
//! asserts (1) the guard attaches to the real edge and validate-continuity is
//! clean, (2) both world-lines' manuscripts are held and DIFFER, (3) the condition
//! fact cannot be retracted while the guard references it, and (4) a guard on a
//! NON-edge fact is flagged `edge_guard_not_an_edge` — the guard is edge-only and
//! Mnemosyne never evaluates it.

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

/// Seed the registries (kinds / entities / predicates) + the map rule; the facts,
/// branches, and the guard are authored through the CLI below (the real path).
fn write_workspace(ws: &Path) {
    fs::create_dir_all(ws.join("docs/.atomic")).unwrap();
    fs::write(
        ws.join("mnemosyne.toml"),
        "[workspace]\n[continuity]\ncanon_order_path = \"canon-order.json\"\n\
         rules_path = \"narrative-rules.json\"\n",
    )
    .unwrap();
    let atomic = serde_json::json!({
        "schema_version": 32,
        "sections": { "ch-1": {}, "ch-2": {}, "ch-3": {} },
        "changelog_entries": {},
        "frames": { "gt": {} },
        "entity_kinds": { "place": {}, "person": {}, "thing": {} },
        "entities": {
            "hero": { "kind": "person" },
            "hall": { "kind": "place" },
            "vault": { "kind": "place" },
            "iron-key": { "kind": "thing" }
        },
        "predicates": {
            "adjacent": { "object_kind": "entity", "subject_kind": "place",
                          "object_entity_kind": "place" },
            "at-loc": { "object_kind": "entity" },
            "holds": { "object_kind": "entity" }
        },
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
            "edges": [["ch-1", "ch-2"], ["ch-2", "ch-3"]] })
        .to_string(),
    )
    .unwrap();
    // A transition rule so the edge-guard SEMANTIC (EdgeGuardNotAnEdge) is active.
    fs::write(
        ws.join("narrative-rules.json"),
        serde_json::json!({ "schema": "narrative-rules/v1", "rules": [
            { "id": "island-roads", "class": "transition", "predicate": "at-loc",
              "adjacency": "adjacent", "undirected": true }
        ]})
        .to_string(),
    )
    .unwrap();
}

fn add_fact(ws: &Path, args: &[&str]) {
    let mut full = vec!["add-fact"];
    full.extend_from_slice(args);
    ok(ws, &full);
}

#[test]
fn edge_guard_holds_both_branches_of_the_key_and_the_game_only_needs_the_boolean() {
    let tmp = TempDir::new().unwrap();
    let ws = tmp.path();
    write_workspace(ws);

    // The map: the hall is adjacent to the vault — the DOOR edge (`f-door`).
    add_fact(
        ws,
        &[
            "--fact",
            "f-door",
            "--frame",
            "gt",
            "--claim",
            "a heavy door joins the hall and the vault",
            "--canon-from",
            "ch-1",
            "--evidence",
            "ch-1",
            "--entities",
            "hall,vault",
            "--typed-subject",
            "hall",
            "--typed-predicate",
            "adjacent",
            "--typed-object-entity",
            "vault",
        ],
    );

    // Fork the two world-lines at the door (ch-2).
    ok(
        ws,
        &[
            "add-branch",
            "--branch",
            "got-key",
            "--forks-from",
            "main",
            "--forks-at",
            "ch-2",
        ],
    );
    ok(
        ws,
        &[
            "add-branch",
            "--branch",
            "no-key",
            "--forks-from",
            "main",
            "--forks-at",
            "ch-2",
        ],
    );

    // The CONDITION fact — the hero holds the iron key — authored ONLY on the
    // got-key world-line (that is what "having the key" MEANS: this branch).
    add_fact(
        ws,
        &[
            "--fact",
            "f-has-key",
            "--frame",
            "gt",
            "--branch",
            "got-key",
            "--claim",
            "the hero holds the iron key",
            "--canon-from",
            "ch-2",
            "--evidence",
            "ch-2",
            "--entities",
            "hero,iron-key",
            "--typed-subject",
            "hero",
            "--typed-predicate",
            "holds",
            "--typed-object-entity",
            "iron-key",
        ],
    );
    // got-key OUTCOME: the hero crosses into the vault.
    add_fact(
        ws,
        &[
            "--fact",
            "f-in-vault",
            "--frame",
            "gt",
            "--branch",
            "got-key",
            "--claim",
            "the hero steps into the vault",
            "--canon-from",
            "ch-3",
            "--evidence",
            "ch-3",
            "--entities",
            "hero,vault",
            "--typed-subject",
            "hero",
            "--typed-predicate",
            "at-loc",
            "--typed-object-entity",
            "vault",
        ],
    );
    // no-key OUTCOME: the hero stays in the hall.
    add_fact(
        ws,
        &[
            "--fact",
            "f-stuck",
            "--frame",
            "gt",
            "--branch",
            "no-key",
            "--claim",
            "the hero waits in the hall, the door shut",
            "--canon-from",
            "ch-3",
            "--evidence",
            "ch-3",
            "--entities",
            "hero,hall",
            "--typed-subject",
            "hero",
            "--typed-predicate",
            "at-loc",
            "--typed-object-entity",
            "hall",
        ],
    );

    // Attach the GUARD: the door REQUIRES the key. Both facts exist → accepted.
    ok(
        ws,
        &[
            "add-edge-guard",
            "--fact",
            "f-door",
            "--condition",
            "f-has-key",
        ],
    );

    // (1) validate-continuity is CLEAN — the guard is on a real map edge, so
    // `edge_guard_not_an_edge` does NOT fire; Mnemosyne never evaluates the guard.
    let out = run(
        ws,
        &[
            "validate-continuity",
            "--rules",
            "narrative-rules.json",
            "--json",
        ],
    );
    assert!(
        out.status.success(),
        "a guard on a real edge is clean: {out:?}"
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    assert_eq!(v["violation_count"], 0, "{v}");

    // (2) MNEMOSYNE HOLDS BOTH BRANCHES — the got-key manuscript reaches the
    // vault; the no-key manuscript does not (the hero is stuck). The full
    // branching lives in the store; the game only picks a side.
    let manuscript = |world: &str| -> serde_json::Value {
        let out = ok(
            ws,
            &["report-playthrough-manuscript", "--world", world, "--json"],
        );
        serde_json::from_slice(&out.stdout).expect("json")
    };
    let got = manuscript("got-key");
    let got_facts: Vec<&str> = got["worlds"]["got-key"]["scenes"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|s| s["begins"].as_array().unwrap())
        .map(|b| b["fact_id"].as_str().unwrap())
        .collect();
    assert!(
        got_facts.contains(&"f-has-key"),
        "got-key has the key: {got_facts:?}"
    );
    assert!(
        got_facts.contains(&"f-in-vault"),
        "got-key enters the vault: {got_facts:?}"
    );
    assert!(
        !got_facts.contains(&"f-stuck"),
        "got-key is not stuck: {got_facts:?}"
    );

    let no = manuscript("no-key");
    let no_facts: Vec<&str> = no["worlds"]["no-key"]["scenes"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|s| s["begins"].as_array().unwrap())
        .map(|b| b["fact_id"].as_str().unwrap())
        .collect();
    assert!(
        no_facts.contains(&"f-stuck"),
        "no-key stays in the hall: {no_facts:?}"
    );
    assert!(
        !no_facts.contains(&"f-in-vault"),
        "no-key never enters the vault: {no_facts:?}"
    );
    assert!(
        !no_facts.contains(&"f-has-key"),
        "no-key never has the key: {no_facts:?}"
    );

    // (3) the condition fact cannot be retracted while the guard references it —
    // the guard's integrity is enforced (the R707-block peer, side-table variant).
    let out = run(
        ws,
        &["retract-fact", "--fact", "f-has-key", "--reason", "test"],
    );
    assert!(
        !out.status.success(),
        "a referenced condition cannot be retracted"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("guard CONDITION") && stderr.contains("f-door"),
        "the refusal names the referring edge: {stderr}"
    );
}

#[test]
fn edge_guard_on_a_non_edge_fact_is_flagged() {
    let tmp = TempDir::new().unwrap();
    let ws = tmp.path();
    write_workspace(ws);
    // A non-map fact (predicate `holds`, not the adjacency predicate) + a
    // condition fact.
    add_fact(
        ws,
        &[
            "--fact",
            "f-notedge",
            "--frame",
            "gt",
            "--claim",
            "the hero holds a lantern",
            "--canon-from",
            "ch-1",
            "--evidence",
            "ch-1",
            "--entities",
            "hero,iron-key",
            "--typed-subject",
            "hero",
            "--typed-predicate",
            "holds",
            "--typed-object-entity",
            "iron-key",
        ],
    );
    add_fact(
        ws,
        &[
            "--fact",
            "f-cond",
            "--frame",
            "gt",
            "--claim",
            "some condition",
            "--canon-from",
            "ch-1",
            "--evidence",
            "ch-1",
        ],
    );
    // The store-layer add-edge-guard ACCEPTS it (it cannot know the adjacency
    // predicate without the rules — invariant 4); validate-continuity catches it.
    ok(
        ws,
        &[
            "add-edge-guard",
            "--fact",
            "f-notedge",
            "--condition",
            "f-cond",
        ],
    );
    let out = run(
        ws,
        &[
            "validate-continuity",
            "--rules",
            "narrative-rules.json",
            "--json",
        ],
    );
    assert!(
        !out.status.success(),
        "a guard on a non-edge must gate: {out:?}"
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    let kinds: Vec<&str> = v["violations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["kind"].as_str().unwrap())
        .collect();
    assert!(kinds.contains(&"edge_guard_not_an_edge"), "{v}");
}

/// Round 722 — a MULTI-CONDITION guard (AND set), through the real binary: a door
/// requires BOTH the iron key AND low tide; add-edge-guard twice accumulates the
/// set; retracting EITHER condition is refused; remove-edge-guard-condition drops
/// one and the door still validates clean (the guard is never evaluated). This is
/// the owner's "증거들이 있을 경우에만" — a choice gated on SEVERAL conditions.
#[test]
fn multi_condition_guard_set_end_to_end() {
    let tmp = TempDir::new().unwrap();
    let ws = tmp.path();
    write_workspace(ws);
    add_fact(
        ws,
        &[
            "--fact",
            "f-door",
            "--frame",
            "gt",
            "--claim",
            "a door joins the hall and the vault",
            "--canon-from",
            "ch-1",
            "--evidence",
            "ch-1",
            "--entities",
            "hall,vault",
            "--typed-subject",
            "hall",
            "--typed-predicate",
            "adjacent",
            "--typed-object-entity",
            "vault",
        ],
    );
    // Two conditions: the key and low tide.
    add_fact(
        ws,
        &[
            "--fact",
            "f-key",
            "--frame",
            "gt",
            "--claim",
            "the hero holds the key",
            "--canon-from",
            "ch-1",
            "--evidence",
            "ch-1",
            "--entities",
            "hero,iron-key",
            "--typed-subject",
            "hero",
            "--typed-predicate",
            "holds",
            "--typed-object-entity",
            "iron-key",
        ],
    );
    add_fact(
        ws,
        &[
            "--fact",
            "f-tide",
            "--frame",
            "gt",
            "--claim",
            "the tide is low",
            "--canon-from",
            "ch-1",
            "--evidence",
            "ch-1",
        ],
    );
    // Accumulate the AND set: the door requires BOTH.
    ok(
        ws,
        &["add-edge-guard", "--fact", "f-door", "--condition", "f-key"],
    );
    ok(
        ws,
        &[
            "add-edge-guard",
            "--fact",
            "f-door",
            "--condition",
            "f-tide",
        ],
    );
    // Clean: the guard is on a real edge, never evaluated.
    let out = run(
        ws,
        &[
            "validate-continuity",
            "--rules",
            "narrative-rules.json",
            "--json",
        ],
    );
    assert!(
        out.status.success(),
        "a multi-condition guard on a real edge is clean: {out:?}"
    );

    // Retracting EITHER condition is refused (both are in the set).
    for cond in ["f-key", "f-tide"] {
        let out = run(ws, &["retract-fact", "--fact", cond, "--reason", "test"]);
        assert!(
            !out.status.success(),
            "{cond} is referenced by the guard set"
        );
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("guard CONDITION"),
            "the refusal names the guard: {cond}"
        );
    }
    // Drop ONE condition via the granular remover; the door still validates.
    ok(
        ws,
        &[
            "remove-edge-guard-condition",
            "--fact",
            "f-door",
            "--condition",
            "f-key",
        ],
    );
    let out = run(
        ws,
        &[
            "validate-continuity",
            "--rules",
            "narrative-rules.json",
            "--json",
        ],
    );
    assert!(
        out.status.success(),
        "still clean after dropping one condition: {out:?}"
    );
    // f-key is now retractable (no longer in any guard set).
    let out = run(ws, &["retract-fact", "--fact", "f-key", "--reason", "test"]);
    assert!(
        out.status.success(),
        "the dropped condition retracts: {out:?}"
    );
}
