//! Round 728 design → Round 729 build (meter substrate) + Round 730 build (the
//! CHOICE gate) — the numeric parameter economy (DEBT-K) through the CLI.
//!
//! The durable proof (not an ephemeral dogfood — the R689/R699 discipline) that
//! the meter economy is wired end-to-end through the REAL binary:
//! - R729 substrate — `add-parameter` / `add-parameter-delta` /
//!   `remove-parameter-delta`: a meter is registered, SIGNED per-beat deltas
//!   attach to real facts (both signs, the axis edge_cost's n>0 forbids), an
//!   unregistered parameter / zero delta / missing fact are rejected, and
//!   `retract-fact` cascade-drops a beat's deltas so none dangles.
//! - R730 gate (gaps 1+2) — `add-parameter-gate` / `remove-parameter-gate` /
//!   `report-parameter-economy`: "The Courtship" — a rising affection meter, a
//!   romance choice gated `affection >= 4`; the gate references the LIVE meter
//!   (drop a delta, the reported Σ moves), the boolean-proxy silent hole is
//!   unrepresentable (there is no proxy fact, only the meter), and `retract-fact`
//!   cascade-drops the gate.

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

fn stdout(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
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

/// Round 730 build (DEBT-K, the CHOICE half — gaps 1+2) — "The Courtship" through
/// the REAL binary: a rising affection meter, a romance choice gated
/// `affection >= 4`. The durable proof that `add-parameter-gate` /
/// `remove-parameter-gate` / `report-parameter-economy` are wired end-to-end AND
/// that the gate references the LIVE meter (the R725 boolean-proxy silent hole is
/// unrepresentable — there is no disconnected "sufficient" fact, only the meter).
#[test]
fn the_courtship_parameter_gate_end_to_end() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    let ws = tmp.path();

    // The beats: two gifts (+2 each), one insult (-1), and the romance choice.
    for f in ["f-gift-1", "f-gift-2", "f-insult", "f-romance-choice"] {
        assert!(add_beat(ws, f).status.success());
    }
    assert!(run(ws, &["add-parameter", "--parameter", "affection"])
        .status
        .success());
    for (fact, delta) in [("f-gift-1", "2"), ("f-gift-2", "2"), ("f-insult", "-1")] {
        assert!(run(
            ws,
            &[
                "add-parameter-delta",
                "--fact",
                fact,
                "--parameter",
                "affection",
                "--delta",
                delta,
            ],
        )
        .status
        .success());
    }

    // A gate on an UNREGISTERED parameter rejects (invariant 4).
    let out = run(
        ws,
        &[
            "add-parameter-gate",
            "--fact",
            "f-romance-choice",
            "--parameter",
            "karma",
            "--op",
            "ge",
            "--threshold",
            "4",
        ],
    );
    assert!(!out.status.success(), "unregistered parameter must reject");
    assert!(stderr(&out).contains("not registered"), "{}", stderr(&out));

    // A bogus op rejects; a missing fact rejects.
    let out = run(
        ws,
        &[
            "add-parameter-gate",
            "--fact",
            "f-romance-choice",
            "--parameter",
            "affection",
            "--op",
            "approximately",
            "--threshold",
            "4",
        ],
    );
    assert!(!out.status.success(), "bogus op must reject");
    assert!(stderr(&out).contains("ge|le|eq|gt|lt"), "{}", stderr(&out));

    let out = run(
        ws,
        &[
            "add-parameter-gate",
            "--fact",
            "f-gone",
            "--parameter",
            "affection",
            "--op",
            "ge",
            "--threshold",
            "4",
        ],
    );
    assert!(!out.status.success(), "missing fact must reject");
    assert!(stderr(&out).contains("not present"), "{}", stderr(&out));

    // The gate lands: romance unlocks at affection >= 4.
    assert!(run(
        ws,
        &[
            "add-parameter-gate",
            "--fact",
            "f-romance-choice",
            "--parameter",
            "affection",
            "--op",
            "ge",
            "--threshold",
            "4",
        ],
    )
    .status
    .success());
    let store = read_store(ws);
    let gate = &store["parameter_gates"]["f-romance-choice"];
    // The gate references the METER DIRECTLY — `parameter` IS the meter the deltas
    // move, not a disconnected "sufficient" proxy fact. This is the structural
    // proof the R725 boolean-proxy silent hole is UNREPRESENTABLE.
    assert_eq!(gate["parameter"], "affection");
    assert_eq!(gate["op"], "ge");
    assert_eq!(gate["threshold"], 4);

    // A2: a DIVERGENT gate on the same choice rejects the silent overwrite.
    let out = run(
        ws,
        &[
            "add-parameter-gate",
            "--fact",
            "f-romance-choice",
            "--parameter",
            "affection",
            "--op",
            "ge",
            "--threshold",
            "6",
        ],
    );
    assert!(!out.status.success(), "divergent gate must reject");
    assert!(stderr(&out).contains("DIVERGENT"), "{}", stderr(&out));

    // report-parameter-economy: the meter is VISIBLE with its Σ and its gate.
    let out = run(ws, &["report-parameter-economy", "--json"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let report: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    let meter = report["meters"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["parameter"] == "affection")
        .expect("affection meter in the economy read");
    assert_eq!(meter["delta_count"], 3);
    assert_eq!(meter["sum_positive"], 4, "Σ+ = 2 + 2");
    assert_eq!(meter["sum_negative"], -1, "Σ- = -1");
    let gate_row = &meter["gates"][0];
    assert_eq!(gate_row["fact"], "f-romance-choice");
    assert_eq!(gate_row["op"], ">=");
    assert_eq!(gate_row["threshold"], 4);

    // The gate rides the LIVE meter: drop a +2 gift, and the SAME meter the gate
    // references changes (Σ+ 4 -> 2). Under an apply-once model the gate is now
    // unreachable — but that is the CONSUMER's judgment; Mnemosyne emits no
    // verdict, it just reports the moved Σ. (The R725 disconnected proxy could not
    // move with the value; this one is the value.)
    assert!(run(
        ws,
        &[
            "remove-parameter-delta",
            "--fact",
            "f-gift-2",
            "--parameter",
            "affection"
        ]
    )
    .status
    .success());
    let out = run(ws, &["report-parameter-economy", "--json"]);
    let report: serde_json::Value = serde_json::from_str(&stdout(&out)).unwrap();
    let meter = report["meters"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["parameter"] == "affection")
        .unwrap();
    assert_eq!(meter["delta_count"], 2, "one gift dropped");
    assert_eq!(meter["sum_positive"], 2, "Σ+ moved with the live meter");
    // The gate is untouched by the delta drop (it references the meter, not the beat).
    assert_eq!(meter["gates"][0]["threshold"], 4);

    // The store validates clean throughout.
    assert!(
        run(ws, &["validate-workspace"]).status.success(),
        "the store validates clean with the gated meter economy"
    );

    // retract-fact on the choice cascade-drops the gate.
    assert!(run(
        ws,
        &[
            "retract-fact",
            "--fact",
            "f-romance-choice",
            "--reason",
            "route cut"
        ]
    )
    .status
    .success());
    assert!(
        read_store(ws)["parameter_gates"]
            .get("f-romance-choice")
            .is_none(),
        "the gate must cascade-drop with the choice fact"
    );
    assert!(
        run(ws, &["validate-workspace"]).status.success(),
        "clean after the cascade-drop"
    );
}
