//! Round 709 (DEBT-J) — the map edge-cost side-table through the CLI.
//!
//! The durable proof (not an ephemeral dogfood — the R689/R699 discipline) that
//! `add-edge-cost` is wired end-to-end through the REAL binary: a cost attaches
//! to an existing fact, a missing fact / non-positive n (G3) / unregistered unit
//! are rejected, and `retract-fact` cascade-drops the cost so it never dangles.

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

fn stderr(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn read_store(ws: &Path) -> serde_json::Value {
    serde_json::from_str(
        &fs::read_to_string(ws.join("docs/.atomic/workspace.atomic.json")).unwrap(),
    )
    .unwrap()
}

fn write_workspace(workspace: &Path) {
    fs::create_dir_all(workspace.join("docs/.atomic")).unwrap();
    fs::write(workspace.join("mnemosyne.toml"), "[workspace]\n").unwrap();
    let atomic = serde_json::json!({
        "schema_version": 15,
        "sections": { "ch-1": {} },
        "changelog_entries": {},
        "frames": { "gt": {} }
    });
    fs::write(
        workspace.join("docs/.atomic/workspace.atomic.json"),
        serde_json::to_string_pretty(&atomic).unwrap(),
    )
    .unwrap();
}

fn add_edge_fact(ws: &Path, id: &str) -> std::process::Output {
    run(
        ws,
        &[
            "add-fact",
            "--fact",
            id,
            "--frame",
            "gt",
            "--claim",
            "an edge",
            "--canon-from",
            "ch-1",
            "--evidence",
            "ch-1",
        ],
    )
}

#[test]
fn edge_cost_gate_end_to_end() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    let ws = tmp.path();

    assert!(
        run(ws, &["add-unit", "--unit", "minute", "--description", ""])
            .status
            .success()
    );
    assert!(add_edge_fact(ws, "f-edge").status.success());

    // Missing fact rejects.
    let out = run(
        ws,
        &[
            "add-edge-cost",
            "--fact",
            "f-gone",
            "--n",
            "4",
            "--unit",
            "minute",
        ],
    );
    assert!(!out.status.success(), "missing fact must reject");
    assert!(stderr(&out).contains("not present"), "{}", stderr(&out));

    // Non-positive n rejects (G3).
    let out = run(
        ws,
        &[
            "add-edge-cost",
            "--fact",
            "f-edge",
            "--n",
            "0",
            "--unit",
            "minute",
        ],
    );
    assert!(!out.status.success(), "n=0 must reject");
    assert!(
        stderr(&out).contains("must be positive"),
        "{}",
        stderr(&out)
    );

    // Unregistered unit rejects.
    let out = run(
        ws,
        &[
            "add-edge-cost",
            "--fact",
            "f-edge",
            "--n",
            "4",
            "--unit",
            "fortnight",
        ],
    );
    assert!(!out.status.success(), "unregistered unit must reject");
    assert!(
        stderr(&out).contains("not a registered unit"),
        "{}",
        stderr(&out)
    );

    // Valid cost lands.
    let out = run(
        ws,
        &[
            "add-edge-cost",
            "--fact",
            "f-edge",
            "--n",
            "4",
            "--unit",
            "minute",
        ],
    );
    assert!(out.status.success(), "valid cost should land: {out:?}");
    let store = read_store(ws);
    assert_eq!(store["edge_costs"]["f-edge"]["n"], 4);
    assert_eq!(store["edge_costs"]["f-edge"]["unit"], "minute");

    // retract-fact cascade-drops the cost.
    assert!(run(
        ws,
        &["retract-fact", "--fact", "f-edge", "--reason", "map edit"]
    )
    .status
    .success());
    let store = read_store(ws);
    assert!(
        store["edge_costs"].get("f-edge").is_none(),
        "the cost must cascade-drop with its fact: {}",
        store["edge_costs"]
    );
}

/// Round 711 — `remove-edge-cost` through the REAL binary: it drops a stray cost
/// off a fact WITHOUT retracting the fact (the exit `retract-fact` cannot give a
/// referenced or legitimate non-edge fact), and a remove with no cost to drop
/// fails loud.
#[test]
fn remove_edge_cost_drops_the_cost_end_to_end() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    let ws = tmp.path();

    assert!(
        run(ws, &["add-unit", "--unit", "minute", "--description", ""])
            .status
            .success()
    );
    assert!(add_edge_fact(ws, "f-edge").status.success());
    assert!(run(
        ws,
        &[
            "add-edge-cost",
            "--fact",
            "f-edge",
            "--n",
            "4",
            "--unit",
            "minute"
        ],
    )
    .status
    .success());
    assert_eq!(read_store(ws)["edge_costs"]["f-edge"]["n"], 4);

    // Remove the cost: it goes, but the FACT stays.
    let out = run(ws, &["remove-edge-cost", "--fact", "f-edge"]);
    assert!(out.status.success(), "remove should land: {out:?}");
    let store = read_store(ws);
    assert!(
        store["edge_costs"].get("f-edge").is_none(),
        "the stray cost is dropped: {}",
        store["edge_costs"]
    );
    assert!(
        store["narrative_facts"].get("f-edge").is_some(),
        "the fact is untouched (the exit retract-fact could not give)"
    );

    // Removing a cost that is not there fails loud.
    let out = run(ws, &["remove-edge-cost", "--fact", "f-edge"]);
    assert!(!out.status.success(), "remove-absent must fail loud");
    assert!(stderr(&out).contains("no edge cost"), "{}", stderr(&out));
}
