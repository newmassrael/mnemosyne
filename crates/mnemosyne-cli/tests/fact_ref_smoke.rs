//! Round 707 — the Fact object shape (typed fact-ref) through the CLI.
//!
//! The durable proof (not an ephemeral dogfood — the R689/R699 discipline) that
//! the `fact` object shape is wired end-to-end through the REAL binary: a
//! Fact-ref to a MISSING fact rejects in phase 2, a self-reference rejects, a
//! ref to an existing fact lands, and `retract-fact` refuses to orphan a fact
//! still referenced as another fact's typed object (the R625 delete-path guard).

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

fn add_plain_fact(ws: &Path, id: &str) -> std::process::Output {
    run(
        ws,
        &[
            "add-fact",
            "--fact",
            id,
            "--frame",
            "gt",
            "--claim",
            "a plain fact",
            "--canon-from",
            "ch-1",
            "--evidence",
            "ch-1",
        ],
    )
}

fn add_ref_fact(ws: &Path, id: &str, target: &str) -> std::process::Output {
    run(
        ws,
        &[
            "add-fact",
            "--fact",
            id,
            "--frame",
            "gt",
            "--claim",
            "a fact that references another",
            "--canon-from",
            "ch-1",
            "--evidence",
            "ch-1",
            "--entities",
            "mina",
            "--typed-subject",
            "mina",
            "--typed-predicate",
            "opened-by",
            "--typed-object-fact",
            target,
        ],
    )
}

#[test]
fn fact_ref_gate_end_to_end() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    let ws = tmp.path();

    assert!(
        run(ws, &["add-entity", "--entity", "mina", "--description", ""])
            .status
            .success()
    );
    assert!(run(
        ws,
        &[
            "add-predicate",
            "--predicate",
            "opened-by",
            "--object-kind",
            "fact",
            "--description",
            "which fact opened this",
        ],
    )
    .status
    .success());

    // A Fact-ref to a MISSING fact rejects in phase 2, naming it.
    let out = add_ref_fact(ws, "f-open", "f-gone");
    assert!(!out.status.success(), "missing target must reject");
    assert!(stderr(&out).contains("not present"), "{}", stderr(&out));

    // A self-reference rejects.
    let out = add_ref_fact(ws, "f-self", "f-self");
    assert!(!out.status.success(), "self-ref must reject");
    assert!(
        stderr(&out).contains("references itself"),
        "{}",
        stderr(&out)
    );

    // With the target present, the ref lands.
    assert!(
        add_plain_fact(ws, "f-sluice").status.success(),
        "seed target"
    );
    let out = add_ref_fact(ws, "f-open", "f-sluice");
    assert!(
        out.status.success(),
        "ref to existing fact should land: {out:?}"
    );

    // retract-fact refuses to orphan a fact referenced as a typed object.
    let out = run(
        ws,
        &["retract-fact", "--fact", "f-sluice", "--reason", "slip"],
    );
    assert!(!out.status.success(), "referenced fact must be blocked");
    assert!(
        stderr(&out).contains("f-open") && stderr(&out).contains("typed object (fact)"),
        "{}",
        stderr(&out)
    );

    // Retract the referrer first, then the target goes cleanly.
    assert!(run(
        ws,
        &["retract-fact", "--fact", "f-open", "--reason", "drop ref"]
    )
    .status
    .success());
    assert!(run(
        ws,
        &["retract-fact", "--fact", "f-sluice", "--reason", "slip"]
    )
    .status
    .success());
}
