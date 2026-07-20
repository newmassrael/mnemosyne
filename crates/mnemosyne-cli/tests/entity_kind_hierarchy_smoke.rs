//! Round 732 (DEBT-M) — the entity-kind inheritance tree through the CLI.
//!
//! The durable proof (not an ephemeral dogfood — the R689/R699 discipline) that
//! `add-entity-kind --parent` is wired end-to-end through the REAL binary AND
//! that the subtree scope closes the R732-measured expressiveness gap: "a weapon
//! is a kind of thing" — a `thing`-scoped predicate ACCEPTS a `weapon` (which a
//! flat registry rejected), a `weapon`-scoped predicate still REJECTS a bare
//! `thing` (the subtree is directional), and a self / unregistered parent is
//! rejected at write time. The `parent` link is visible in the raw SSOT JSON.

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

#[test]
fn entity_kind_hierarchy_end_to_end() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    let ws = tmp.path();

    // The kind tree: thing ⊃ weapon (a weapon is a kind of thing); character root.
    assert!(run(ws, &["add-entity-kind", "--kind", "thing"])
        .status
        .success());
    let out = run(
        ws,
        &["add-entity-kind", "--kind", "weapon", "--parent", "thing"],
    );
    assert!(out.status.success(), "weapon --parent thing: {out:?}");
    assert!(run(ws, &["add-entity-kind", "--kind", "character"])
        .status
        .success());

    // Write-path guards: a self-parent and an unregistered parent reject.
    let out = run(
        ws,
        &["add-entity-kind", "--kind", "loop", "--parent", "loop"],
    );
    assert!(!out.status.success());
    assert!(stderr(&out).contains("its own parent"), "{}", stderr(&out));
    let out = run(
        ws,
        &["add-entity-kind", "--kind", "orphan", "--parent", "ghost"],
    );
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("not a registered"),
        "{}",
        stderr(&out)
    );

    // The raw SSOT JSON carries the parent link.
    let store = read_store(ws);
    assert_eq!(
        store["entity_kinds"]["weapon"]["parent"], "thing",
        "parent must be stored: {store}"
    );

    // Entities + two predicates: holds scoped to `thing`, wields to `weapon`.
    for (id, kind) in [
        ("hero", "character"),
        ("sword", "weapon"),
        ("shield", "thing"),
    ] {
        assert!(run(ws, &["add-entity", "--entity", id, "--kind", kind])
            .status
            .success());
    }
    assert!(run(
        ws,
        &[
            "add-predicate",
            "--predicate",
            "holds",
            "--object-kind",
            "entity",
            "--subject-kind",
            "character",
            "--object-entity-kind",
            "thing",
            "--description",
            "custody",
        ],
    )
    .status
    .success());
    assert!(run(
        ws,
        &[
            "add-predicate",
            "--predicate",
            "wields",
            "--object-kind",
            "entity",
            "--subject-kind",
            "character",
            "--object-entity-kind",
            "weapon",
            "--description",
            "wielding",
        ],
    )
    .status
    .success());

    let add_fact = |fid: &str, pred: &str, obj: &str| -> std::process::Output {
        run(
            ws,
            &[
                "add-fact",
                "--fact",
                fid,
                "--frame",
                "gt",
                "--canon-from",
                "ch-1",
                "--evidence",
                "ch-1",
                "--entities",
                &format!("hero,{obj}"),
                "--typed-subject",
                "hero",
                "--typed-predicate",
                pred,
                "--typed-object-entity",
                obj,
                "--claim",
                "x",
            ],
        )
    };

    // THE CLOSED GAP: holds(hero, sword) — sword is a `weapon`, weapon ⊂ thing ⇒
    // ACCEPT. A flat registry rejected this exact fact (the R732 measurement).
    let out = add_fact("f-hold", "holds", "sword");
    assert!(
        out.status.success(),
        "holds(hero, sword) must accept via subtree: {out:?}"
    );
    // wields(hero, sword) — weapon == weapon ⇒ ACCEPT.
    assert!(add_fact("f-wield", "wields", "sword").status.success());
    // wields(hero, shield) — a bare `thing` is NOT a subkind of `weapon` ⇒
    // REJECT (the subtree is directional, not symmetric).
    let out = add_fact("f-bad", "wields", "shield");
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("object kind `weapon`"),
        "{}",
        stderr(&out)
    );

    // The store validates clean end to end.
    let out = run(ws, &["validate-continuity"]);
    assert!(
        out.status.success(),
        "validate-continuity: {}",
        stderr(&out)
    );
}
