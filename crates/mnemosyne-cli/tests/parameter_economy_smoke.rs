//! Round 728 design → Round 729 build (DEBT-K) — the numeric parameter economy
//! (meter substrate half: `parameters` registry + `parameter_deltas`) through the
//! CLI.
//!
//! The durable proof (not an ephemeral dogfood — the R689/R699 discipline) that
//! `add-parameter` / `add-parameter-delta` / `remove-parameter-delta` are wired
//! end-to-end through the REAL binary: a meter is registered, SIGNED per-beat
//! deltas attach to real facts (both signs, the axis edge_cost's n>0 forbids), an
//! unregistered parameter / zero delta / missing fact are rejected, and
//! `retract-fact` cascade-drops a beat's deltas so none dangles. The threshold
//! GATE (the choice half, gaps 1+2) is R730.

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

fn add_beat(ws: &Path, id: &str) -> std::process::Output {
    run(
        ws,
        &[
            "add-fact",
            "--fact",
            id,
            "--frame",
            "gt",
            "--claim",
            "a beat",
            "--canon-from",
            "ch-1",
            "--evidence",
            "ch-1",
        ],
    )
}

#[test]
fn parameter_economy_meter_substrate_end_to_end() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    let ws = tmp.path();

    assert!(add_beat(ws, "f-gift").status.success());
    assert!(add_beat(ws, "f-insult").status.success());

    // A delta on an UNREGISTERED parameter rejects (invariant 4).
    let out = run(
        ws,
        &[
            "add-parameter-delta",
            "--fact",
            "f-gift",
            "--parameter",
            "affection",
            "--delta",
            "2",
        ],
    );
    assert!(!out.status.success(), "unregistered parameter must reject");
    assert!(stderr(&out).contains("not registered"), "{}", stderr(&out));

    // Register the meter.
    assert!(run(
        ws,
        &[
            "add-parameter",
            "--parameter",
            "affection",
            "--description",
            "the meter"
        ]
    )
    .status
    .success());
    assert_eq!(
        read_store(ws)["parameters"]["affection"]["description"],
        "the meter"
    );

    // Missing fact rejects; zero delta rejects.
    let out = run(
        ws,
        &[
            "add-parameter-delta",
            "--fact",
            "f-gone",
            "--parameter",
            "affection",
            "--delta",
            "1",
        ],
    );
    assert!(!out.status.success(), "missing fact must reject");
    assert!(stderr(&out).contains("not present"), "{}", stderr(&out));

    let out = run(
        ws,
        &[
            "add-parameter-delta",
            "--fact",
            "f-gift",
            "--parameter",
            "affection",
            "--delta",
            "0",
        ],
    );
    assert!(!out.status.success(), "zero delta must reject");
    assert!(stderr(&out).contains("no-op"), "{}", stderr(&out));

    // Valid POSITIVE and NEGATIVE deltas land (the signed axis).
    assert!(run(
        ws,
        &[
            "add-parameter-delta",
            "--fact",
            "f-gift",
            "--parameter",
            "affection",
            "--delta",
            "2"
        ]
    )
    .status
    .success());
    assert!(run(
        ws,
        &[
            "add-parameter-delta",
            "--fact",
            "f-insult",
            "--parameter",
            "affection",
            "--delta",
            "-1"
        ]
    )
    .status
    .success());
    let store = read_store(ws);
    assert_eq!(store["parameter_deltas"]["f-gift"]["affection"], 2);
    assert_eq!(store["parameter_deltas"]["f-insult"]["affection"], -1);

    // A2: a DIVERGENT delta on the same (fact, parameter) rejects.
    let out = run(
        ws,
        &[
            "add-parameter-delta",
            "--fact",
            "f-gift",
            "--parameter",
            "affection",
            "--delta",
            "3",
        ],
    );
    assert!(!out.status.success(), "divergent delta must reject");
    assert!(stderr(&out).contains("DIVERGENT"), "{}", stderr(&out));

    // retract-fact cascade-drops the beat's deltas.
    assert!(run(
        ws,
        &["retract-fact", "--fact", "f-gift", "--reason", "beat cut"]
    )
    .status
    .success());
    let store = read_store(ws);
    assert!(
        store["parameter_deltas"].get("f-gift").is_none(),
        "the deltas must cascade-drop with the beat: {}",
        store["parameter_deltas"]
    );
    // The other beat's delta is untouched, and the store validates clean.
    assert_eq!(
        read_store(ws)["parameter_deltas"]["f-insult"]["affection"],
        -1
    );
    assert!(
        run(ws, &["validate-workspace"]).status.success(),
        "the store validates clean with the meter economy"
    );
}

/// Round 729 — `remove-parameter-delta` through the REAL binary: drops ONE
/// (fact, parameter) delta; the beat key survives while another delta remains and
/// is dropped when the last goes; a remove with nothing to drop fails loud.
#[test]
fn remove_parameter_delta_end_to_end() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    let ws = tmp.path();

    assert!(add_beat(ws, "f-gift").status.success());
    for p in ["affection", "trust"] {
        assert!(run(ws, &["add-parameter", "--parameter", p])
            .status
            .success());
    }
    // One beat moves TWO meters.
    assert!(run(
        ws,
        &[
            "add-parameter-delta",
            "--fact",
            "f-gift",
            "--parameter",
            "affection",
            "--delta",
            "2"
        ]
    )
    .status
    .success());
    assert!(run(
        ws,
        &[
            "add-parameter-delta",
            "--fact",
            "f-gift",
            "--parameter",
            "trust",
            "--delta",
            "1"
        ]
    )
    .status
    .success());

    // Remove-absent fails loud.
    let out = run(
        ws,
        &[
            "remove-parameter-delta",
            "--fact",
            "f-gift",
            "--parameter",
            "gold",
        ],
    );
    assert!(!out.status.success(), "remove-absent must fail loud");
    assert!(stderr(&out).contains("no delta"), "{}", stderr(&out));

    // Drop one: the beat key stays (trust remains).
    assert!(run(
        ws,
        &[
            "remove-parameter-delta",
            "--fact",
            "f-gift",
            "--parameter",
            "affection"
        ]
    )
    .status
    .success());
    let store = read_store(ws);
    assert!(store["parameter_deltas"]["f-gift"]
        .get("affection")
        .is_none());
    assert_eq!(store["parameter_deltas"]["f-gift"]["trust"], 1);

    // Drop the last: the beat KEY is dropped (no vacuous empty map).
    assert!(run(
        ws,
        &[
            "remove-parameter-delta",
            "--fact",
            "f-gift",
            "--parameter",
            "trust"
        ]
    )
    .status
    .success());
    assert!(
        read_store(ws)["parameter_deltas"].get("f-gift").is_none(),
        "the emptied beat key is dropped"
    );
}
