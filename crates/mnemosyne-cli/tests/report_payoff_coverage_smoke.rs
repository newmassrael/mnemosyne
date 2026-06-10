//! Round 442 — `report-payoff-coverage` verb smoke tests.
//!
//! End-to-end over a workspace store: setups authored through `add-fact
//! --payoff-expectation` classify dangling until a `--pays-off` fact lands,
//! the classification is world-scoped (per registered branch), and the
//! verb is a pure read (exit 0 with dangling setups — never gated).

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

fn coverage_json(workspace: &Path) -> serde_json::Value {
    let out = run(workspace, &["report-payoff-coverage", "--json"]);
    assert!(out.status.success(), "{:?}", out);
    serde_json::from_slice(&out.stdout).expect("json output")
}

#[test]
fn setup_dangles_until_paid_and_the_verb_never_gates() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    let out = run(
        tmp.path(),
        &[
            "add-fact",
            "--fact",
            "su-knife",
            "--frame",
            "gt",
            "--claim",
            "Quincey carries his bowie knife",
            "--canon-from",
            "ch-1",
            "--evidence",
            "ch-1",
            "--payoff-expectation",
            "expected",
        ],
    );
    assert!(out.status.success(), "{:?}", out);
    // Dangling setup: a report finding, exit 0 (the author's todo list).
    let v = coverage_json(tmp.path());
    assert_eq!(v["setups_total"], 1);
    assert_eq!(v["worlds"]["main"]["dangling"][0], "su-knife");
    // The payoff flips it.
    let out = run(
        tmp.path(),
        &[
            "add-fact",
            "--fact",
            "p-knife",
            "--frame",
            "gt",
            "--claim",
            "the knife finds the Count's heart",
            "--canon-from",
            "ch-3",
            "--evidence",
            "ch-3",
            "--pays-off",
            "su-knife",
        ],
    );
    assert!(out.status.success(), "{:?}", out);
    let v = coverage_json(tmp.path());
    assert_eq!(v["worlds"]["main"]["paid"][0]["setup"], "su-knife");
    assert_eq!(v["worlds"]["main"]["paid"][0]["payoffs"][0], "p-knife");
    assert!(v["worlds"]["main"]["dangling"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[test]
fn coverage_is_world_scoped_per_registered_branch() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    for args in [
        vec![
            "add-fact",
            "--fact",
            "su",
            "--frame",
            "gt",
            "--claim",
            "the gun on the wall",
            "--canon-from",
            "ch-1",
            "--evidence",
            "ch-1",
            "--payoff-expectation",
            "expected",
        ],
        vec![
            "add-branch",
            "--branch",
            "route",
            "--forks-from",
            "main",
            "--forks-at",
            "ch-2",
        ],
        vec![
            "add-fact",
            "--fact",
            "p-main",
            "--frame",
            "gt",
            "--claim",
            "the gun fires",
            "--canon-from",
            "ch-3",
            "--evidence",
            "ch-3",
            "--pays-off",
            "su",
        ],
    ] {
        let out = run(tmp.path(), &args);
        assert!(out.status.success(), "{args:?}: {out:?}");
    }
    let v = coverage_json(tmp.path());
    // Main pays its own gun; the fork inherits the setup but not the
    // post-fork payoff — it dangles there (each playthrough resolves its
    // own guns).
    assert_eq!(v["worlds"]["main"]["paid"][0]["setup"], "su");
    assert_eq!(v["worlds"]["route"]["dangling"][0], "su");
}

#[test]
fn fail_loud_on_missing_pays_off_target() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    let out = run(
        tmp.path(),
        &[
            "add-fact",
            "--fact",
            "p-orphan",
            "--frame",
            "gt",
            "--claim",
            "pays a setup nobody wrote",
            "--canon-from",
            "ch-1",
            "--evidence",
            "ch-1",
            "--pays-off",
            "never-written",
        ],
    );
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("pays_off"));
}
