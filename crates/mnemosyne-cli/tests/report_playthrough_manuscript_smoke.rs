//! Round 466 — `report-playthrough-manuscript` verb smoke tests.
//!
//! End-to-end over a workspace store: the composed canon order linearizes
//! into per-world scene walks with declared fact events (begins / expired /
//! superseded-by), the `--world` filter narrows to one manuscript and fails
//! loud on a typo, and the verb is a pure read (exit 0 with unplaced
//! facts — a reading surface, never gated).

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn cli_binary() -> &'static str {
    env!("CARGO_BIN_EXE_mnemosyne-cli")
}

fn run(workspace: &Path, args: &[&str]) -> std::process::Output {
    Command::new(cli_binary())
        .args(args)
        .current_dir(workspace)
        .output()
        .expect("cli exec")
}

fn write_workspace(workspace: &Path) {
    fs::create_dir_all(workspace.join("docs/.atomic")).unwrap();
    fs::write(
        workspace.join("mnemosyne.toml"),
        "[workspace]\n[continuity]\ncanon_order_path = \"canon-order.json\"\n",
    )
    .unwrap();
    let atomic = serde_json::json!({
        "schema_version": 18,
        "sections": { "ch-1": {}, "ch-2": {}, "ch-3": {} },
        "changelog_entries": {},
        "frames": { "gt": {} },
        "narrative_facts": {}
    });
    fs::write(
        workspace.join("docs/.atomic/workspace.atomic.json"),
        serde_json::to_string_pretty(&atomic).unwrap(),
    )
    .unwrap();
    fs::write(
        workspace.join("canon-order.json"),
        serde_json::json!({
            "schema": "canon-order/v1",
            "edges": [["ch-1", "ch-2"], ["ch-2", "ch-3"]]
        })
        .to_string(),
    )
    .unwrap();
}

fn add_fact(workspace: &Path, args: &[&str]) {
    let mut full = vec!["add-fact"];
    full.extend_from_slice(args);
    let out = run(workspace, &full);
    assert!(out.status.success(), "{args:?}: {out:?}");
}

fn manuscript_json(workspace: &Path, extra: &[&str]) -> serde_json::Value {
    let mut args = vec!["report-playthrough-manuscript", "--json"];
    args.extend_from_slice(extra);
    let out = run(workspace, &args);
    assert!(out.status.success(), "{:?}", out);
    serde_json::from_slice(&out.stdout).expect("json output")
}

#[test]
fn scenes_walk_the_order_with_declared_events_per_world() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    add_fact(
        tmp.path(),
        &[
            "--fact",
            "f-gun",
            "--frame",
            "gt",
            "--claim",
            "the revolver hangs on the wall",
            "--canon-from",
            "ch-1",
            "--evidence",
            "ch-1",
        ],
    );
    add_fact(
        tmp.path(),
        &[
            "--fact",
            "f-fired",
            "--frame",
            "gt",
            "--claim",
            "the revolver is fired",
            "--canon-from",
            "ch-3",
            "--evidence",
            "ch-3",
            "--supersedes",
            "f-gun",
        ],
    );
    let out = run(
        tmp.path(),
        &[
            "add-branch",
            "--branch",
            "route",
            "--forks-from",
            "main",
            "--forks-at",
            "ch-2",
        ],
    );
    assert!(out.status.success(), "{out:?}");
    let v = manuscript_json(tmp.path(), &[]);
    // Main walks ch-1..ch-3; the supersession ends f-gun at ch-3 naming
    // the cutting fact; holds-judged count agrees with the delta story.
    let main = &v["worlds"]["main"];
    let scenes = main["scenes"].as_array().unwrap();
    assert_eq!(scenes.len(), 3);
    assert_eq!(scenes[0]["section"], "ch-1");
    assert_eq!(scenes[0]["begins"][0]["fact_id"], "f-gun");
    assert_eq!(scenes[2]["ends"][0]["fact_id"], "f-gun");
    assert_eq!(scenes[2]["ends"][0]["kind"], "superseded");
    assert_eq!(scenes[2]["ends"][0]["by"], "f-fired");
    assert_eq!(scenes[2]["holding_count"], 1);
    // The fork inherits the setup but not the post-fork supersession —
    // its manuscript keeps f-gun holding through ch-3 untouched.
    let route = &v["worlds"]["route"];
    let r_scenes = route["scenes"].as_array().unwrap();
    assert!(r_scenes[2]["ends"].as_array().unwrap().is_empty());
    assert_eq!(r_scenes[2]["holding_count"], 1);
    assert_eq!(r_scenes[2]["begins"].as_array().unwrap().len(), 0);
}

#[test]
fn world_filter_narrows_and_fails_loud_and_unplaced_never_gates() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    // ch-3 is a section but the order below names only ch-1 -> ch-2:
    // rewrite the declaration to leave the fact's coordinate unplaced.
    fs::write(
        tmp.path().join("canon-order.json"),
        serde_json::json!({
            "schema": "canon-order/v1",
            "edges": [["ch-1", "ch-2"]]
        })
        .to_string(),
    )
    .unwrap();
    add_fact(
        tmp.path(),
        &[
            "--fact",
            "f-out",
            "--frame",
            "gt",
            "--claim",
            "an event the order never places",
            "--canon-from",
            "ch-3",
            "--evidence",
            "ch-3",
        ],
    );
    // Unplaced fact: a report finding, exit 0 (reading surface, no gate).
    let v = manuscript_json(tmp.path(), &["--world", "main"]);
    assert_eq!(v["worlds"].as_object().unwrap().len(), 1);
    let main = &v["worlds"]["main"];
    assert_eq!(main["unplaced_facts"][0]["fact_id"], "f-out");
    assert_eq!(main["unplaced_facts"][0]["field"], "canon_from");
    assert!(main["sections_off_road"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("ch-3")));
    // A typo'd world fails loud instead of reading as an empty manuscript.
    let out = run(
        tmp.path(),
        &["report-playthrough-manuscript", "--world", "nope"],
    );
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("branch registry"));
}

/// Round 734 — the confluence follow-on from the R733 review, PROVEN BY TEST
/// (not by reading source). The review saw a pre-fork trunk fact read
/// `undecidable` under `--world <confluence>` and doubted describe-schema's
/// "the path-independent trunk survives" a merge. This locks in the real
/// behaviour: the trunk fact SURVIVES through the shared confluence suffix in
/// EVERY real playthrough (main + the two forks); the default dump enumerates
/// only those playthroughs (a confluence is not a standalone world —
/// `query_worlds`); and an explicit `--world <confluence>` renders the
/// prefix-less FRAGMENT for inspection, marking the pre-merge trunk
/// `undecidable` (B-1 honest) rather than silently dropping it. A
/// path-dependent sibling fact is the negative control — it survives its OWN
/// playthrough's suffix but is dropped from the sibling's at the merge
/// intersect, so the trunk assertion is not vacuous.
fn write_confluence_workspace(workspace: &Path) {
    fs::create_dir_all(workspace.join("docs/.atomic")).unwrap();
    fs::write(
        workspace.join("mnemosyne.toml"),
        "[workspace]\n[continuity]\ncanon_order_path = \"canon-order.json\"\n",
    )
    .unwrap();
    let atomic = serde_json::json!({
        "schema_version": 37,
        "sections": { "prologue": {}, "fork": {}, "t1": {}, "k1": {}, "merge": {}, "end": {} },
        "changelog_entries": {},
        "frames": { "gt": {} },
        "entity_kinds": { "thing": {} },
        "entities": { "crown": { "kind": "thing" }, "dagger": { "kind": "thing" } },
        "narrative_facts": {}
    });
    fs::write(
        workspace.join("docs/.atomic/workspace.atomic.json"),
        serde_json::to_string_pretty(&atomic).unwrap(),
    )
    .unwrap();
    // main trunk prologue -> fork; thief/knight fork at `fork` and rejoin at
    // `merge`; conf continues the shared suffix merge -> end.
    fs::write(
        workspace.join("canon-order.json"),
        serde_json::json!({ "schema": "canon-order/v1",
        "edges": [["prologue", "fork"]],
        "branches": {
            "thief":  [["fork", "t1"], ["t1", "merge"]],
            "knight": [["fork", "k1"], ["k1", "merge"]],
            "conf":   [["merge", "end"]]
        }})
        .to_string(),
    )
    .unwrap();
}

#[test]
fn pre_fork_trunk_survives_the_confluence_suffix_in_every_playthrough() {
    let tmp = TempDir::new().unwrap();
    let ws = tmp.path();
    write_confluence_workspace(ws);
    // thief + knight fork off main at `fork`; conf converges from BOTH at `merge`.
    for args in [
        &[
            "add-branch",
            "--branch",
            "thief",
            "--forks-from",
            "main",
            "--forks-at",
            "fork",
        ][..],
        &[
            "add-branch",
            "--branch",
            "knight",
            "--forks-from",
            "main",
            "--forks-at",
            "fork",
        ][..],
        &[
            "add-branch",
            "--branch",
            "conf",
            "--converges",
            "thief=merge",
            "--converges",
            "knight=merge",
        ][..],
    ] {
        let out = run(ws, args);
        assert!(out.status.success(), "{args:?}: {out:?}");
    }
    // f-pre: a PATH-INDEPENDENT pre-fork trunk fact on main at prologue.
    add_fact(
        ws,
        &[
            "--fact",
            "f-pre",
            "--frame",
            "gt",
            "--claim",
            "the crown sits in the vault",
            "--canon-from",
            "prologue",
            "--evidence",
            "prologue",
            "--entities",
            "crown",
        ],
    );
    // f-thief: a PATH-DEPENDENT fact on the thief branch at t1 (the negative control).
    add_fact(
        ws,
        &[
            "--fact",
            "f-thief",
            "--frame",
            "gt",
            "--branch",
            "thief",
            "--claim",
            "the thief pockets a dagger",
            "--canon-from",
            "t1",
            "--evidence",
            "t1",
            "--entities",
            "dagger",
        ],
    );

    let all_begins = |v: &serde_json::Value, world: &str| -> Vec<String> {
        v["worlds"][world]["scenes"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|s| s["begins"].as_array().unwrap())
            .map(|b| b["fact_id"].as_str().unwrap().to_string())
            .collect()
    };
    let holding_at = |v: &serde_json::Value, world: &str, section: &str| -> u64 {
        v["worlds"][world]["scenes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["section"] == section)
            .unwrap_or_else(|| panic!("no scene {section} in {world}"))["holding_count"]
            .as_u64()
            .unwrap()
    };

    // (1) The default dump enumerates only the PLAYTHROUGHS — main + the two
    // forks — NOT the confluence (a converges branch is not a standalone world;
    // its shared suffix renders within each parent).
    let all = manuscript_json(ws, &[]);
    let worlds: BTreeSet<&str> = all["worlds"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        worlds,
        BTreeSet::from(["main", "thief", "knight"]),
        "a confluence is not a standalone world: {worlds:?}"
    );

    // (2) THE THIEF PLAYTHROUGH: the trunk fact AND the thief's own fact both
    // survive through the shared confluence suffix (merge, end).
    let thief = manuscript_json(ws, &["--world", "thief"]);
    let thief_begins = all_begins(&thief, "thief");
    assert!(thief_begins.contains(&"f-pre".to_string()));
    assert!(thief_begins.contains(&"f-thief".to_string()));
    assert_eq!(
        holding_at(&thief, "thief", "merge"),
        2,
        "both hold at the merge"
    );
    assert_eq!(
        holding_at(&thief, "thief", "end"),
        2,
        "both survive the confluence suffix in the thief playthrough"
    );

    // (3) THE KNIGHT PLAYTHROUGH: only the TRUNK survives the suffix; the
    // sibling thief's path-dependent fact is dropped at the intersect (the
    // negative control — the trunk-survival assertion is not vacuous).
    let knight = manuscript_json(ws, &["--world", "knight"]);
    let knight_begins = all_begins(&knight, "knight");
    assert!(knight_begins.contains(&"f-pre".to_string()));
    assert!(
        !knight_begins.contains(&"f-thief".to_string()),
        "the sibling's path-dependent fact is not in the knight playthrough: {knight_begins:?}"
    );
    assert_eq!(
        holding_at(&knight, "knight", "merge"),
        1,
        "only the trunk at the merge"
    );
    assert_eq!(
        holding_at(&knight, "knight", "end"),
        1,
        "the trunk survives the confluence suffix in the knight playthrough too"
    );

    // (4) THE CONFLUENCE FRAGMENT (explicit --world conf): the prefix-less
    // fragment cannot place the pre-merge trunk, so it marks it `undecidable`
    // (B-1 honest) — it does NOT silently drop it, and does NOT show it holding.
    let conf = manuscript_json(ws, &["--world", "conf"]);
    let undecidable: Vec<&str> = conf["worlds"]["conf"]["undecidable"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap())
        .collect();
    assert!(
        undecidable.contains(&"f-pre"),
        "the fragment marks the pre-merge trunk undecidable, not silently dropped: {undecidable:?}"
    );
    assert!(
        !all_begins(&conf, "conf").contains(&"f-pre".to_string()),
        "the prefix-less fragment does not falsely show the trunk holding"
    );
}
