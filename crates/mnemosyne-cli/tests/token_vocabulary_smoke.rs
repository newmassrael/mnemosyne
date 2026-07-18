//! Round 705 — the closed token vocabulary, through the CLI.
//!
//! The durable proof (not an ephemeral dogfood — the R689/R699 discipline) that
//! the `token` object shape is wired end-to-end through the REAL binary: the
//! declaration guards fire (`object_kind=token` needs a non-empty vocabulary; a
//! non-token kind cannot carry one), a fact with an IN-vocabulary token lands, a
//! token OUTSIDE the closed set is rejected at write time naming it, and a
//! `set-predicate` WIDEN then admits the new token — the R658 escape hatch.

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

fn add_life_fact(ws: &Path, id: &str, token: &str) -> std::process::Output {
    run(
        ws,
        &[
            "add-fact",
            "--fact",
            id,
            "--frame",
            "gt",
            "--claim",
            "lucy's state",
            "--canon-from",
            "ch-1",
            "--evidence",
            "ch-1",
            "--entities",
            "lucy",
            "--typed-subject",
            "lucy",
            "--typed-predicate",
            "life",
            "--typed-object-token",
            token,
        ],
    )
}

#[test]
fn token_vocabulary_gate_end_to_end() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    let ws = tmp.path();

    let out = run(ws, &["add-entity", "--entity", "lucy", "--description", ""]);
    assert!(out.status.success(), "add-entity: {out:?}");

    // Declaration guard: object_kind=token with NO vocabulary re-opens the
    // free-text hole → rejected.
    let out = run(
        ws,
        &[
            "add-predicate",
            "--predicate",
            "life",
            "--object-kind",
            "token",
            "--description",
            "state",
        ],
    );
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("non-empty object_tokens"),
        "{}",
        stderr(&out)
    );

    // Declaration guard: object_tokens on a non-token kind is a dead field.
    let out = run(
        ws,
        &[
            "add-predicate",
            "--predicate",
            "at",
            "--object-kind",
            "entity",
            "--object-tokens",
            "here,there",
            "--description",
            "x",
        ],
    );
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("cannot carry an object_tokens"),
        "{}",
        stderr(&out)
    );

    // The closed vocabulary lands.
    let out = run(
        ws,
        &[
            "add-predicate",
            "--predicate",
            "life",
            "--object-kind",
            "token",
            "--object-tokens",
            "alive,dead",
            "--description",
            "life state",
        ],
    );
    assert!(out.status.success(), "{out:?}");

    // In-vocabulary token: accepted at write time.
    let out = add_life_fact(ws, "f-1", "alive");
    assert!(out.status.success(), "in-vocab token should land: {out:?}");

    // Out-of-vocabulary token: rejected, naming the offending token.
    let out = add_life_fact(ws, "f-2", "undead");
    assert!(!out.status.success(), "out-of-vocab token must reject");
    assert!(
        stderr(&out).contains("not in its declared vocabulary"),
        "{}",
        stderr(&out)
    );

    // Widen the closed set via set-predicate (the R658 escape hatch), then the
    // formerly-rejected token lands.
    let out = run(
        ws,
        &[
            "set-predicate",
            "--predicate",
            "life",
            "--object-kind",
            "token",
            "--object-tokens",
            "alive,dead,undead",
            "--description",
            "life state",
        ],
    );
    assert!(out.status.success(), "widen should land: {out:?}");
    let out = add_life_fact(ws, "f-3", "undead");
    assert!(
        out.status.success(),
        "after widening, undead should land: {out:?}"
    );
}
