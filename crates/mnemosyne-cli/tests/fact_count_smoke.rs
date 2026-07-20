//! Round 731 (DEBT-L) — the multiset-count side-table through the CLI.
//!
//! The durable proof (not an ephemeral dogfood — the R689/R699 discipline) that
//! `add-fact-count` / `remove-fact-count` are wired end-to-end through the REAL
//! binary AND that the count closes the R731-measured orphaned-count silent hole:
//! "the potion stack" — a custody fact `holds(char-a, potion)` with count 5 (A
//! holds FIVE potions); a missing fact / non-positive count are rejected; and
//! `retract-fact` CASCADE-DROPS the count, so a count can no longer survive its
//! custody retract (the disconnected-count workaround's silent hole, now
//! unrepresentable).

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

/// Author `holds(char-a, potion)` — a real custody fact (entities + a holds
/// predicate + a typed leg), the shape a count rides.
fn author_custody(ws: &Path) {
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
    assert!(run(
        ws,
        &[
            "add-fact",
            "--fact",
            "f-hold",
            "--frame",
            "gt",
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
        ],
    )
    .status
    .success());
}

#[test]
fn fact_count_the_potion_stack_end_to_end() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    let ws = tmp.path();
    author_custody(ws);

    // A count on a MISSING fact rejects.
    let out = run(ws, &["add-fact-count", "--fact", "f-gone", "--count", "5"]);
    assert!(!out.status.success(), "missing fact must reject");
    assert!(stderr(&out).contains("not present"), "{}", stderr(&out));

    // A non-positive count rejects (0 / negative = not holding it).
    for bad in ["0", "-3"] {
        let out = run(ws, &["add-fact-count", "--fact", "f-hold", "--count", bad]);
        assert!(!out.status.success(), "count {bad} must reject");
        assert!(
            stderr(&out).contains("must be positive"),
            "{}",
            stderr(&out)
        );
    }

    // The count lands: A holds FIVE potions — bound to the custody fact.
    let out = run(ws, &["add-fact-count", "--fact", "f-hold", "--count", "5"]);
    assert!(out.status.success(), "valid count should land: {out:?}");
    assert_eq!(read_store(ws)["fact_counts"]["f-hold"], 5);

    // A2: a DIVERGENT count on the same fact rejects the silent overwrite.
    let out = run(ws, &["add-fact-count", "--fact", "f-hold", "--count", "7"]);
    assert!(!out.status.success(), "divergent count must reject");
    assert!(stderr(&out).contains("DIVERGENT"), "{}", stderr(&out));

    // The store validates clean with the count.
    assert!(
        run(ws, &["validate-workspace"]).status.success(),
        "the store validates clean with the multiset count"
    );

    // THE PROOF: retract the custody fact (char-a used / handed off the potions)
    // and the count CASCADE-DROPS with it. The R731-measured silent hole (a
    // disconnected count fact surviving its custody retract — a phantom stack with
    // no holder) is now UNREPRESENTABLE: the count is keyed BY the custody fact.
    assert!(run(
        ws,
        &[
            "retract-fact",
            "--fact",
            "f-hold",
            "--reason",
            "char-a used the potions"
        ]
    )
    .status
    .success());
    let store = read_store(ws);
    assert!(
        store["fact_counts"].get("f-hold").is_none(),
        "the count must cascade-drop with the custody fact: {}",
        store["fact_counts"]
    );
    assert!(
        run(ws, &["validate-workspace"]).status.success(),
        "clean after the cascade-drop"
    );
}

/// Round 731 — `remove-fact-count` through the REAL binary: it drops a stray count
/// off a fact WITHOUT retracting the fact (the exit `retract-fact` cannot give a
/// referenced or legitimate fact), and a remove with no count to drop fails loud.
#[test]
fn remove_fact_count_drops_the_count_end_to_end() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    let ws = tmp.path();
    author_custody(ws);
    assert!(
        run(ws, &["add-fact-count", "--fact", "f-hold", "--count", "5"])
            .status
            .success()
    );
    assert_eq!(read_store(ws)["fact_counts"]["f-hold"], 5);

    // Remove the count: it goes, but the FACT stays.
    let out = run(ws, &["remove-fact-count", "--fact", "f-hold"]);
    assert!(out.status.success(), "remove should land: {out:?}");
    let store = read_store(ws);
    assert!(
        store["fact_counts"].get("f-hold").is_none(),
        "the stray count is dropped: {}",
        store["fact_counts"]
    );
    assert!(
        store["narrative_facts"].get("f-hold").is_some(),
        "the fact is untouched"
    );

    // Removing a count that is not there fails loud.
    let out = run(ws, &["remove-fact-count", "--fact", "f-hold"]);
    assert!(!out.status.success(), "remove-absent must fail loud");
    assert!(stderr(&out).contains("no fact count"), "{}", stderr(&out));
}
