//! Round 481 — `report-drift-candidates` + `import-drift-verdicts` smoke.
//!
//! End-to-end over a workspace store: a payoff fact starts unreviewed (the
//! drift surface), an independent confirm verdict clears it, and a self-confirm
//! verdict is rejected loudly (exit 1) — the load-bearing rule that blocks the
//! false assurance scale-floor R475 exposed.

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
    fs::write(workspace.join("mnemosyne.toml"), "[workspace]\n").unwrap();
    let atomic = serde_json::json!({
        "schema_version": 18,
        "sections": { "ch-1": {}, "ch-3": {} },
        "changelog_entries": {},
        "frames": { "gt": {} },
        "narrative_facts": {}
    });
    fs::write(
        workspace.join("docs/.atomic/workspace.atomic.json"),
        serde_json::to_string_pretty(&atomic).unwrap(),
    )
    .unwrap();
}

/// A workspace with a setup `su` and a payoff `p` (pays_off su, with a quote).
fn seed_payoff(workspace: &Path) {
    write_workspace(workspace);
    let setup = run(
        workspace,
        &[
            "add-fact",
            "--fact",
            "su",
            "--frame",
            "gt",
            "--claim",
            "a brass-locked diary holds the secret",
            "--canon-from",
            "ch-1",
            "--evidence",
            "ch-1",
            "--payoff-expectation",
            "expected",
        ],
    );
    assert!(setup.status.success(), "{setup:?}");
    let pay = run(
        workspace,
        &[
            "add-fact",
            "--fact",
            "p",
            "--frame",
            "gt",
            "--claim",
            "the diary is forced open and names the killer",
            "--canon-from",
            "ch-3",
            "--evidence",
            "ch-3",
            "--pays-off",
            "su",
            "--quote",
            "She forced the brass lock; the page named Brandt.",
        ],
    );
    assert!(pay.status.success(), "{pay:?}");
}

fn drift_json(workspace: &Path) -> serde_json::Value {
    let out = run(workspace, &["report-drift-candidates", "--json"]);
    assert!(out.status.success(), "{out:?}");
    serde_json::from_slice(&out.stdout).expect("json")
}

#[test]
fn payoff_fact_starts_unreviewed_then_an_independent_confirm_clears_it() {
    let tmp = TempDir::new().unwrap();
    seed_payoff(tmp.path());

    let v = drift_json(tmp.path());
    assert_eq!(v["payoff_facts"], 1);
    assert_eq!(v["drifting"], 1);
    assert_eq!(v["candidates"][0]["status"], "unreviewed");
    let claim_sha = v["candidates"][0]["claim_sha256"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(v["candidates"][0]["quote"]
        .as_str()
        .unwrap()
        .contains("brass lock"));

    // An independent confirm (authoring != confirming) clears the drift.
    let verdicts = serde_json::json!({
        "schema": "drift-verdicts/v1",
        "verdicts": [{
            "fact": "p",
            "claim_sha256": claim_sha,
            "verdict": "confirm",
            "rationale": "the quoted prose opens the diary and names the killer",
            "authoring_run": "author-session",
            "confirming_run": "reviewer-session",
            "confirmer_id": "claude-opus-4-8",
            "confirmer_version": "2026-06",
            "timestamp": "2026-06-12T00:00:00Z"
        }]
    });
    let vpath = tmp.path().join("verdicts.json");
    fs::write(&vpath, serde_json::to_string(&verdicts).unwrap()).unwrap();
    let out = run(
        tmp.path(),
        &[
            "import-drift-verdicts",
            "--verdicts",
            "verdicts.json",
            "--json",
        ],
    );
    assert!(out.status.success(), "{out:?}");
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["applied"], true);
    assert_eq!(report["accepted"], 1);

    let v = drift_json(tmp.path());
    assert_eq!(v["candidates"][0]["status"], "reviewed");
    assert_eq!(v["drifting"], 0);
}

#[test]
fn self_confirm_verdict_is_rejected_loudly() {
    let tmp = TempDir::new().unwrap();
    seed_payoff(tmp.path());
    let claim_sha = drift_json(tmp.path())["candidates"][0]["claim_sha256"]
        .as_str()
        .unwrap()
        .to_string();

    let verdicts = serde_json::json!({
        "schema": "drift-verdicts/v1",
        "verdicts": [{
            "fact": "p",
            "claim_sha256": claim_sha,
            "verdict": "confirm",
            "rationale": "rubber-stamping my own claim",
            "authoring_run": "one-and-the-same",
            "confirming_run": "one-and-the-same",
            "confirmer_id": "claude-opus-4-8",
            "confirmer_version": "2026-06",
            "timestamp": "2026-06-12T00:00:00Z"
        }]
    });
    let vpath = tmp.path().join("verdicts.json");
    fs::write(&vpath, serde_json::to_string(&verdicts).unwrap()).unwrap();
    let out = run(
        tmp.path(),
        &["import-drift-verdicts", "--verdicts", "verdicts.json"],
    );
    assert!(!out.status.success(), "self-confirm must exit non-zero");
    assert!(String::from_utf8_lossy(&out.stdout).contains("self-confirm"));
    // The fact stays drifting.
    assert_eq!(drift_json(tmp.path())["drifting"], 1);
}
