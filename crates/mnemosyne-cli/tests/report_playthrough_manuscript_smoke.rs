//! Round 466 — `report-playthrough-manuscript` verb smoke tests.
//!
//! End-to-end over a workspace store: the composed canon order linearizes
//! into per-world scene walks with declared fact events (begins / expired /
//! superseded-by), the `--world` filter narrows to one manuscript and fails
//! loud on a typo, and the verb is a pure read (exit 0 with unplaced
//! facts — a reading surface, never gated).

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
    assert!(main["sections_outside_order"]
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
