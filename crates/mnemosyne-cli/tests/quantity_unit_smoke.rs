//! Round 706 — the quantity object shape + units registry, through the CLI.
//!
//! The durable proof (not an ephemeral dogfood — the R689/R699 discipline) that
//! the `quantity` object shape is wired end-to-end through the REAL binary: a
//! Quantity fact whose unit is NOT registered is rejected at write time, an
//! `add-unit` then lets the same fact land (the R626 escape hatch), the two
//! quantity flags are all-or-nothing, and a non-integer `n` fails to parse.

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

fn add_signed_fact(ws: &Path, id: &str, n: &str, unit: &str) -> std::process::Output {
    run(
        ws,
        &[
            "add-fact",
            "--fact",
            id,
            "--frame",
            "gt",
            "--claim",
            "codicil signed on day",
            "--canon-from",
            "ch-1",
            "--evidence",
            "ch-1",
            "--entities",
            "codicil",
            "--typed-subject",
            "codicil",
            "--typed-predicate",
            "signed-on-day",
            "--typed-object-quantity-n",
            n,
            "--typed-object-quantity-unit",
            unit,
        ],
    )
}

#[test]
fn quantity_unit_gate_end_to_end() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    let ws = tmp.path();

    let out = run(
        ws,
        &["add-entity", "--entity", "codicil", "--description", ""],
    );
    assert!(out.status.success(), "add-entity: {out:?}");

    // object_tokens on a quantity predicate is a dead field.
    let out = run(
        ws,
        &[
            "add-predicate",
            "--predicate",
            "bad",
            "--object-kind",
            "quantity",
            "--object-tokens",
            "day",
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

    let out = run(
        ws,
        &[
            "add-predicate",
            "--predicate",
            "signed-on-day",
            "--object-kind",
            "quantity",
            "--description",
            "signing day",
        ],
    );
    assert!(out.status.success(), "{out:?}");

    // A Quantity whose unit is NOT registered: rejected at write time, naming it.
    let out = add_signed_fact(ws, "f-1", "10", "day");
    assert!(!out.status.success(), "unregistered unit must reject");
    assert!(
        stderr(&out).contains("not a registered unit"),
        "{}",
        stderr(&out)
    );

    // Register the unit (the R626 escape hatch), then the same fact lands.
    let out = run(
        ws,
        &["add-unit", "--unit", "day", "--description", "calendar day"],
    );
    assert!(out.status.success(), "add-unit: {out:?}");
    let out = add_signed_fact(ws, "f-1", "10", "day");
    assert!(
        out.status.success(),
        "after add-unit, the quantity fact should land: {out:?}"
    );

    // The two quantity flags are all-or-nothing (n without unit).
    let out = run(
        ws,
        &[
            "add-fact",
            "--fact",
            "f-2",
            "--frame",
            "gt",
            "--claim",
            "x",
            "--canon-from",
            "ch-1",
            "--evidence",
            "ch-1",
            "--entities",
            "codicil",
            "--typed-subject",
            "codicil",
            "--typed-predicate",
            "signed-on-day",
            "--typed-object-quantity-n",
            "5",
        ],
    );
    assert!(!out.status.success(), "n without unit must reject");
    assert!(
        stderr(&out).contains("must be given together"),
        "{}",
        stderr(&out)
    );

    // A non-integer n fails to parse (exact integer, never f64).
    let out = add_signed_fact(ws, "f-3", "3.5", "day");
    assert!(!out.status.success(), "non-integer n must reject");
    assert!(
        stderr(&out).contains("must be an integer"),
        "{}",
        stderr(&out)
    );
}
