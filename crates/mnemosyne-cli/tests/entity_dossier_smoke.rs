//! Round 437 — `add-entity` / `report-entity` / `--entity` filter smoke.
//!
//! End-to-end: register an entity, author a fact carrying it, read the
//! dossier and the entity-filtered frame view; typo'd entity fails loud on
//! both reads; unregistered ref fails loud on the write.

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
        "schema_version": 15,
        "sections": { "ch-1": {}, "ch-2": {} },
        "changelog_entries": {},
        "frames": { "jonathan": {} }
    });
    fs::write(
        workspace.join("docs/.atomic/workspace.atomic.json"),
        serde_json::to_string_pretty(&atomic).unwrap(),
    )
    .unwrap();
}

#[test]
fn entity_axis_end_to_end() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    // Unregistered entity ref on the write path fails loud.
    let out = run(
        tmp.path(),
        &[
            "add-fact",
            "--fact",
            "f-castle",
            "--frame",
            "jonathan",
            "--claim",
            "the count never eats",
            "--canon-from",
            "ch-1",
            "--evidence",
            "ch-1",
            "--entities",
            "dracula",
        ],
    );
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("entity registry"));
    // The kind is a registry ref too: naming one nobody declared rejects, and
    // says so, before the entity exists.
    let out = run(
        tmp.path(),
        &[
            "add-entity",
            "--entity",
            "dracula",
            "--kind",
            "character",
            "--description",
            "the count",
        ],
    );
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("not a registered entity kind"));
    // Declare the vocabulary, then the write lands.
    let out = run(
        tmp.path(),
        &[
            "add-entity-kind",
            "--kind",
            "character",
            "--description",
            "a person in the story",
        ],
    );
    assert!(out.status.success(), "{:?}", out);
    let out = run(
        tmp.path(),
        &[
            "add-entity",
            "--entity",
            "dracula",
            "--kind",
            "character",
            "--description",
            "the count",
        ],
    );
    assert!(out.status.success(), "{:?}", out);
    let out = run(
        tmp.path(),
        &[
            "add-fact",
            "--fact",
            "f-castle",
            "--frame",
            "jonathan",
            "--claim",
            "the count never eats",
            "--canon-from",
            "ch-1",
            "--evidence",
            "ch-1",
            "--entities",
            "dracula",
        ],
    );
    assert!(out.status.success(), "{:?}", out);
    // Dossier: all facts about the entity.
    let out = run(
        tmp.path(),
        &["report-entity", "--entity", "dracula", "--json"],
    );
    assert!(out.status.success(), "{:?}", out);
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["kind"], "character");
    assert_eq!(v["fact_count"], 1);
    assert_eq!(v["facts"][0]["fact_id"], "f-castle");
    // Entity-filtered frame view.
    let out = run(
        tmp.path(),
        &[
            "report-frame-view",
            "--frame",
            "jonathan",
            "--entity",
            "dracula",
            "--at",
            "ch-2",
            "--json",
        ],
    );
    assert!(out.status.success(), "{:?}", out);
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["entity"], "dracula");
    // No declared order: ch-1 vs ch-2 incomparable — surfaced as unknown
    // (B-1 honesty), never silently absent.
    assert_eq!(v["holding_count"], 0);
    assert!(v["unknown"]
        .as_array()
        .unwrap()
        .iter()
        .any(|u| u == "f-castle"));
    // Typo'd entity fails loud on the dossier read.
    let out = run(tmp.path(), &["report-entity", "--entity", "dracual"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("entity registry"));
}

/// Round 736 — the dossier is CONTENT-COMPLETE: EntityFactRow echoes the R731
/// multiset count AND the verbatim quote (parity with report-frame-view /
/// manuscript, the R735 principle applied to the third fact-echoing surface),
/// through the REAL binary. A fact with neither shows a null quote and no
/// `count` key — opt-in, never an implicit 1.
#[test]
fn dossier_echoes_the_count_and_quote() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    let ws = tmp.path();
    for e in ["char-a", "potion"] {
        assert!(run(ws, &["add-entity", "--entity", e]).status.success());
    }
    assert!(run(
        ws,
        &[
            "add-predicate",
            "--predicate",
            "holds",
            "--object-kind",
            "entity"
        ]
    )
    .status
    .success());
    // A custody fact carrying a verbatim quote.
    assert!(run(
        ws,
        &[
            "add-fact",
            "--fact",
            "f-hold",
            "--frame",
            "jonathan",
            "--claim",
            "char-a holds a potion",
            "--canon-from",
            "ch-1",
            "--evidence",
            "ch-1",
            "--entities",
            "char-a,potion",
            "--typed-subject",
            "char-a",
            "--typed-predicate",
            "holds",
            "--typed-object-entity",
            "potion",
            "--quote",
            "five vials clinked in the bag",
        ],
    )
    .status
    .success());
    assert!(
        run(ws, &["add-fact-count", "--fact", "f-hold", "--count", "5"])
            .status
            .success()
    );
    // A plain fact on the same entity: no count, no quote (the negative control).
    assert!(run(
        ws,
        &[
            "add-fact",
            "--fact",
            "f-plain",
            "--frame",
            "jonathan",
            "--claim",
            "char-a waits",
            "--canon-from",
            "ch-1",
            "--evidence",
            "ch-1",
            "--entities",
            "char-a",
        ],
    )
    .status
    .success());

    let out = run(ws, &["report-entity", "--entity", "char-a", "--json"]);
    assert!(out.status.success(), "{out:?}");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let rows = v["facts"].as_array().unwrap();
    let row = |id: &str| rows.iter().find(|f| f["fact_id"] == id).unwrap();
    assert_eq!(
        row("f-hold")["count"],
        5,
        "the count rides the dossier row: {}",
        row("f-hold")
    );
    assert_eq!(
        row("f-hold")["quote"],
        "five vials clinked in the bag",
        "the quote rides the dossier row: {}",
        row("f-hold")
    );
    // NON-VACUITY: the plain fact has no count key and a null quote.
    assert!(
        row("f-plain").get("count").is_none(),
        "no authored count = no key (opt-in): {}",
        row("f-plain")
    );
    assert!(
        row("f-plain")["quote"].is_null(),
        "no authored quote = null: {}",
        row("f-plain")
    );
}
