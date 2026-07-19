//! Round 431 — `validate-continuity` verb smoke tests.
//!
//! End-to-end over a v12 store with narrative facts: cross-frame conflict =
//! data (exit 0), same-frame overlap = violation (configured reject → exit
//! 1, `--severity warn` → exit 0), disabled when the `[continuity]` table is
//! absent, `--json` contract keys, declared-order pin mismatch fails loud.

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

/// Workspace: chapters ch-1..ch-3 (linear declared order), seward-frame fact
/// f-illness [ch-1..ch-2], `vampire_frame`-frame fact f-vampire [ch-2..),
/// with a recorded conflict edge — the design sec 7.7 acceptance shape
/// (cross-frame with "van-helsing", same-frame with "seward").
fn write_workspace_with_frames(workspace: &Path, continuity_table: &str, vampire_frame: &str) {
    fs::create_dir_all(workspace.join("docs/.atomic")).unwrap();
    fs::write(
        workspace.join("mnemosyne.toml"),
        format!("[workspace]\n{continuity_table}"),
    )
    .unwrap();
    let atomic = serde_json::json!({
        "schema_version": 17,
        "sections": { "ch-1": {}, "ch-2": {}, "ch-3": {} },
        "changelog_entries": {},
        "frames": { "seward": {}, "van-helsing": {} },
        "narrative_facts": {
            "f-illness": {
                "frame": "seward",
                "claim": "Lucy suffers from an unexplained illness",
                "canon_from": "ch-1", "canon_to": "ch-2",
                "evidence": ["ch-1"]
            },
            "f-vampire": {
                "frame": vampire_frame,
                "claim": "Lucy is preyed upon by a vampire",
                "canon_from": "ch-2",
                "evidence": ["ch-2"]
            }
        }
    });
    fs::write(
        workspace.join("docs/.atomic/workspace.atomic.json"),
        serde_json::to_string_pretty(&atomic).unwrap(),
    )
    .unwrap();
    // The conflict edge is recorded through the REAL primitive (R439: the
    // judgment-time claim pin is computed at write, never hand-written).
    let out = run(
        workspace,
        &[
            "add-fact-conflict",
            "--fact",
            "f-illness",
            "--conflicts-with",
            "f-vampire",
        ],
    );
    assert!(out.status.success(), "{:?}", out);
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

fn write_workspace(workspace: &Path, continuity_table: &str) {
    write_workspace_with_frames(workspace, continuity_table, "van-helsing");
}

#[test]
fn cross_frame_conflict_is_data_exit_zero() {
    let tmp = TempDir::new().unwrap();
    write_workspace(
        tmp.path(),
        "[continuity]\ncanon_order_path = \"canon-order.json\"\n",
    );
    let out = run(tmp.path(), &["validate-continuity", "--json"]);
    assert!(out.status.success(), "{:?}", out);
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json output");
    assert_eq!(v["severity"], "reject");
    assert_eq!(v["violation_count"], 0);
    assert_eq!(v["cross_scope_pairs"], 1);
    assert_eq!(v["conflict_pairs_checked"], 1);
    assert_eq!(v["order_nodes"], 3);
}

#[test]
fn same_frame_overlap_rejects_and_warn_passes() {
    let tmp = TempDir::new().unwrap();
    // The vampire claim in seward's OWN frame: the same frame holds both
    // claims on an overlapping window.
    write_workspace_with_frames(
        tmp.path(),
        "[continuity]\ncanon_order_path = \"canon-order.json\"\n",
        "seward",
    );
    let out = run(tmp.path(), &["validate-continuity", "--json"]);
    assert!(!out.status.success(), "reject severity must exit 1");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json output");
    assert_eq!(v["violation_count"], 1);
    assert_eq!(v["violations"][0]["kind"], "frame_conflict_overlap");
    assert_eq!(v["violations"][0]["at"], "ch-2");
    // Same scan under --severity warn: reported but not gated.
    let out = run(tmp.path(), &["validate-continuity", "--severity", "warn"]);
    assert!(out.status.success(), "{:?}", out);
}

#[test]
fn disabled_without_table_and_scan_still_reports() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path(), "");
    let out = run(tmp.path(), &["validate-continuity"]);
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("disabled"), "{stdout}");
    assert!(stdout.contains("conflict_pairs=1"), "{stdout}");
}

#[test]
fn order_pin_mismatch_fails_loud_and_override_bypasses() {
    let tmp = TempDir::new().unwrap();
    write_workspace(
        tmp.path(),
        "[continuity]\ncanon_order_path = \"canon-order.json\"\n\
         canon_order_sha256 = \"0000000000000000000000000000000000000000000000000000000000000000\"\n",
    );
    let out = run(tmp.path(), &["validate-continuity"]);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("sha256 mismatch"), "{stderr}");
    // --order bypasses the pin by design (the pin claims nothing about a
    // different file — the R428 --catalog rule).
    let out = run(
        tmp.path(),
        &["validate-continuity", "--order", "canon-order.json"],
    );
    assert!(out.status.success(), "{:?}", out);
}

#[test]
fn undeclared_order_surfaces_unordered_never_gates() {
    let tmp = TempDir::new().unwrap();
    // Table present (reject) but NO canon_order_path: same-frame conflict on
    // distinct coordinates is not comparable -> unordered count, exit 0.
    write_workspace_with_frames(tmp.path(), "[continuity]\n", "seward");
    let out = run(tmp.path(), &["validate-continuity", "--json"]);
    assert!(out.status.success(), "{:?}", out);
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json output");
    assert_eq!(v["violation_count"], 0);
    assert_eq!(v["unordered_pairs"], 1);
}

/// Round 449 — narrative-rules workspace: typed location facts with a
/// forgotten succession chain, an exclusive rule declared via
/// `[continuity].rules_path`.
fn write_rules_workspace(workspace: &Path, continuity_table: &str) {
    fs::create_dir_all(workspace.join("docs/.atomic")).unwrap();
    fs::write(
        workspace.join("mnemosyne.toml"),
        format!("[workspace]\n{continuity_table}"),
    )
    .unwrap();
    let atomic = serde_json::json!({
        "schema_version": 19,
        "sections": { "ch-1": {}, "ch-2": {}, "ch-3": {} },
        "changelog_entries": {},
        "frames": { "gt": {} },
        "entity_kinds": { "character": {} },
        "entities": { "dracula": { "kind": "character" } },
        "predicates": { "at-location": { "object_kind": "token", "object_tokens": ["castle", "whitby"] } },
        "narrative_facts": {
            "l1": {
                "frame": "gt",
                "entities": ["dracula"],
                "claim": "Dracula is at the castle",
                "canon_from": "ch-1",
                "evidence": ["ch-1"],
                "typed": { "subject": "dracula", "predicate": "at-location",
                           "object": { "kind": "token", "token": "castle" } }
            },
            "bad": {
                "frame": "gt",
                "entities": ["dracula"],
                "claim": "Dracula is at Whitby",
                "canon_from": "ch-2",
                "evidence": ["ch-2"],
                "typed": { "subject": "dracula", "predicate": "at-location",
                           "object": { "kind": "token", "token": "whitby" } }
            }
        }
    });
    fs::write(
        workspace.join("docs/.atomic/workspace.atomic.json"),
        serde_json::to_string_pretty(&atomic).unwrap(),
    )
    .unwrap();
    fs::write(
        workspace.join("canon-order.json"),
        serde_json::json!({ "edges": [["ch-1", "ch-2"], ["ch-2", "ch-3"]] }).to_string(),
    )
    .unwrap();
    fs::write(
        workspace.join("narrative-rules.json"),
        serde_json::json!({
            "schema": "narrative-rules/v1",
            "rules": [
                { "id": "loc", "class": "exclusive",
                  "predicate": "at-location", "per": "subject" }
            ]
        })
        .to_string(),
    )
    .unwrap();
}

#[test]
fn declared_rules_gate_exclusive_overlap_end_to_end() {
    let tmp = TempDir::new().unwrap();
    write_rules_workspace(
        tmp.path(),
        "[continuity]\ncanon_order_path = \"canon-order.json\"\n\
         rules_path = \"narrative-rules.json\"\n",
    );
    let out = run(tmp.path(), &["validate-continuity", "--json"]);
    assert!(!out.status.success(), "rule violation must gate: {out:?}");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json output");
    assert_eq!(v["rules"], 1);
    assert_eq!(v["violation_count"], 1);
    assert_eq!(v["violations"][0]["kind"], "rule_exclusive_overlap");
    assert_eq!(v["violations"][0]["rule"], "loc");
    assert_eq!(v["violations"][0]["fact_a"], "bad");
    assert_eq!(v["violations"][0]["fact_b"], "l1");
    // No rules declared (table without rules_path) = pre-R449 behavior.
    write_rules_workspace(
        tmp.path(),
        "[continuity]\ncanon_order_path = \"canon-order.json\"\n",
    );
    let out = run(tmp.path(), &["validate-continuity", "--json"]);
    assert!(out.status.success(), "{out:?}");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json output");
    assert_eq!(v["rules"], 0);
    // --rules override picks the file up without config.
    let out = run(
        tmp.path(),
        &["validate-continuity", "--rules", "narrative-rules.json"],
    );
    assert!(!out.status.success(), "override must gate: {out:?}");
}

#[test]
fn rules_pin_mismatch_fails_loud_and_override_bypasses() {
    let tmp = TempDir::new().unwrap();
    write_rules_workspace(
        tmp.path(),
        "[continuity]\ncanon_order_path = \"canon-order.json\"\n\
         rules_path = \"narrative-rules.json\"\n\
         rules_sha256 = \"0000000000000000000000000000000000000000000000000000000000000000\"\n",
    );
    let out = run(tmp.path(), &["validate-continuity"]);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("narrative-rules sha256 mismatch"),
        "{stderr}"
    );
    // --rules bypasses the pin (the R428 --catalog rule); the violation
    // itself still gates.
    let out = run(
        tmp.path(),
        &[
            "validate-continuity",
            "--rules",
            "narrative-rules.json",
            "--json",
        ],
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json output");
    assert_eq!(v["violations"][0]["kind"], "rule_exclusive_overlap");
}

/// Round 491 — a single-world store with one INTERVAL violation (a codicil
/// ratified 5 days after signing against a 42-day rule) and no structural
/// violations. The per-class gating: `severity` (reject) does NOT gate the
/// interval violation (surface-not-gate), only `interval_severity` does.
fn write_interval_workspace(workspace: &Path, continuity_table: &str) {
    fs::create_dir_all(workspace.join("docs/.atomic")).unwrap();
    fs::write(
        workspace.join("mnemosyne.toml"),
        format!("[workspace]\n{continuity_table}"),
    )
    .unwrap();
    // Round 708 — the interval operand is a `token` (the free-text scalar shape
    // was removed); the interval evaluator reads the numeric token via parse.
    let tok = |p: &str, v: &str, from: &str| {
        serde_json::json!({
            "frame": "gt", "entities": ["codicil"],
            "claim": format!("{p}={v}"), "canon_from": from, "evidence": [from],
            "typed": {"subject": "codicil", "predicate": p, "object": {"kind": "token", "token": v}}
        })
    };
    let atomic = serde_json::json!({
        "schema_version": 21,
        "sections": { "ch-1": {}, "ch-2": {}, "ch-3": {} },
        "changelog_entries": {}, "inventory_entries": {}, "confirmation_events": {},
        "frames": { "gt": {} },
        "branches": {},
        "entities": { "codicil": {} },
        "predicates": {
            "min-ratify-gap-days": { "object_kind": "token", "object_tokens": ["42"] },
            "signed-on-day": { "object_kind": "token", "object_tokens": ["10"] },
            "ratified-on-day": { "object_kind": "token", "object_tokens": ["15"] }
        },
        "narrative_facts": {
            "f-rule": tok("min-ratify-gap-days", "42", "ch-1"),
            "f-sign": tok("signed-on-day", "10", "ch-1"),
            "f-rat": tok("ratified-on-day", "15", "ch-2")
        }
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
    fs::write(
        workspace.join("narrative-rules.json"),
        serde_json::json!({
            "schema": "narrative-rules/v1",
            "rules": [{
                "id": "ratify-term", "class": "interval",
                "predicate": "ratified-on-day", "right": "signed-on-day",
                "op": "ge", "bound": { "predicate": "min-ratify-gap-days" }
            }]
        })
        .to_string(),
    )
    .unwrap();
}

#[test]
fn interval_violation_is_surface_not_gate_under_severity_but_gates_on_interval_severity() {
    let tmp = TempDir::new().unwrap();
    write_interval_workspace(
        tmp.path(),
        "[continuity]\ncanon_order_path = \"canon-order.json\"\nrules_path = \"narrative-rules.json\"\n",
    );
    // `severity` defaults to reject, but the lone violation is an interval one:
    // surface-not-gate -> exit 0, the violation still reported.
    let out = run(tmp.path(), &["validate-continuity", "--json"]);
    assert!(
        out.status.success(),
        "interval violation must NOT gate under `severity`: {:?}",
        out
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json output");
    assert_eq!(v["violation_count"], 1);
    assert_eq!(v["interval_violation_count"], 1);
    assert_eq!(v["violations"][0]["kind"], "rule_interval_violation");
    // Opt in: `--interval-severity reject` gates it.
    let out = run(
        tmp.path(),
        &["validate-continuity", "--interval-severity", "reject"],
    );
    assert!(
        !out.status.success(),
        "interval violation must gate under `--interval-severity reject`"
    );
    // `warn` surfaces without gating.
    let out = run(
        tmp.path(),
        &["validate-continuity", "--interval-severity", "warn"],
    );
    assert!(out.status.success(), "{:?}", out);
}

/// Round 491 — the interval opt-in NOTICE: with `interval_severity` OFF, a
/// declared interval rule is surface-only, so `validate-continuity` names it
/// aloud (a declared-but-ungated rule must not be a silent surprise). Setting
/// the class to `reject` removes the nudge (the rule now gates).
#[test]
fn interval_severity_off_emits_notice_naming_the_rule_count() {
    let tmp = TempDir::new().unwrap();
    write_interval_workspace(
        tmp.path(),
        "[continuity]\ncanon_order_path = \"canon-order.json\"\nrules_path = \"narrative-rules.json\"\n",
    );
    // Human output, class OFF: the declared interval rule is surface-only, so
    // the CLI names it in a NOTICE instead of leaving it silently ungated.
    let out = run(tmp.path(), &["validate-continuity"]);
    assert!(out.status.success(), "{:?}", out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("NOTICE") && stdout.contains("1 interval rule"),
        "expected an interval opt-in notice, got: {stdout}"
    );
    // Opt in: with `interval_severity reject` the rule gates, so no ungated NOTICE.
    let out = run(
        tmp.path(),
        &["validate-continuity", "--interval-severity", "reject"],
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains("NOTICE"),
        "no ungated-interval notice when interval_severity = reject: {stdout}"
    );
}

/// Round 664 — the zero-rules NOTICE. The rules count printed only when
/// NONZERO, so a run with no rules file wired went GREEN without the word
/// `rules` appearing once: a gate that evaluated NOTHING read exactly like a
/// gate that PASSED. Exactly 0 — the case where the author most needs to hear
/// it — was the one case kept silent. The R663 census found our own consumer
/// hand-building the non-vacuity guard we lacked. Pinned both ways: 0 names
/// itself and hands over the lever, a wired file removes the notice.
#[test]
fn zero_rules_emits_notice_naming_the_off_state() {
    let tmp = TempDir::new().unwrap();
    // The rules file is on disk but NOT wired — the silent-GREEN case.
    write_rules_workspace(
        tmp.path(),
        "[continuity]\ncanon_order_path = \"canon-order.json\"\n",
    );
    let out = run(tmp.path(), &["validate-continuity"]);
    assert!(out.status.success(), "{out:?}");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("NOTICE") && stdout.contains("0 narrative rules declared"),
        "a gate that evaluated no rules must say so, got: {stdout}"
    );
    assert!(
        stdout.contains("rules_path"),
        "the notice must hand over the lever, got: {stdout}"
    );
    // Wire the same file: the rules now run, so the zero-rules notice is gone
    // and the count line speaks instead.
    write_rules_workspace(
        tmp.path(),
        "[continuity]\ncanon_order_path = \"canon-order.json\"\n\
         rules_path = \"narrative-rules.json\"\n",
    );
    let out = run(tmp.path(), &["validate-continuity"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains("0 narrative rules declared"),
        "no zero-rules notice once a rule is wired: {stdout}"
    );
    assert!(
        stdout.contains("rules=1"),
        "the count line prints: {stdout}"
    );
}

/// Round 699 (session-review cleanup) — the COMMITTED CLI proof the R697/R698
/// dogfood only had ephemerally: the store-native map transition gates movement
/// end-to-end through the `validate-continuity` verb. A forward walk over
/// `adjacent` facts passes; a non-adjacent jump gates (exit 1); `undirected`
/// flips whether the reverse of an edge is admitted; the R698 self-loop /
/// reverse-dup integrity fires (exit 1); and a scalar-subject `adjacent` fact
/// is rejected by the REAL write path (the entity-subject requirement the R697
/// changelog named). Places a—b—c, hero walking `at`.
#[test]
fn store_native_map_transition_gates_end_to_end() {
    let tmp = TempDir::new().unwrap();
    let ws = tmp.path();
    fs::create_dir_all(ws.join("docs/.atomic")).unwrap();
    fs::write(
        ws.join("mnemosyne.toml"),
        "[workspace]\n[continuity]\ncanon_order_path = \"canon-order.json\"\n\
         rules_path = \"narrative-rules.json\"\n",
    )
    .unwrap();
    fs::write(
        ws.join("canon-order.json"),
        serde_json::json!({ "edges": [["ch-1", "ch-2"], ["ch-2", "ch-3"]] }).to_string(),
    )
    .unwrap();

    let write_store = |facts: serde_json::Value| {
        let atomic = serde_json::json!({
            "schema_version": 24,
            "sections": { "ch-1": {}, "ch-2": {}, "ch-3": {} },
            "changelog_entries": {},
            "frames": { "gt": {} },
            "entity_kinds": { "place": {}, "character": {} },
            "entities": {
                "p-a": { "kind": "place" }, "p-b": { "kind": "place" },
                "p-c": { "kind": "place" }, "p-d": { "kind": "place" },
                "hero": { "kind": "character" }
            },
            "predicates": {
                "adjacent": { "object_kind": "entity" },
                "at": { "object_kind": "entity" }
            },
            "narrative_facts": facts
        });
        fs::write(
            ws.join("docs/.atomic/workspace.atomic.json"),
            serde_json::to_string_pretty(&atomic).unwrap(),
        )
        .unwrap();
    };
    let write_rules = |undirected: bool| {
        fs::write(
            ws.join("narrative-rules.json"),
            serde_json::json!({
                "schema": "narrative-rules/v1",
                "rules": [ { "id": "roads", "class": "transition", "predicate": "at",
                             "adjacency": "adjacent", "undirected": undirected } ]
            })
            .to_string(),
        )
        .unwrap();
    };
    let edge = |a: &str, b: &str| {
        serde_json::json!({
            "frame": "gt", "entities": [a, b], "claim": format!("{a} borders {b}"),
            "canon_from": "ch-1", "evidence": ["ch-1"],
            "typed": { "subject": a, "predicate": "adjacent", "object": { "kind": "entity", "id": b } }
        })
    };
    let at = |ch: &str, place: &str, prev: Option<&str>| {
        let mut f = serde_json::json!({
            "frame": "gt", "entities": ["hero", place], "claim": format!("hero at {place}"),
            "canon_from": ch, "evidence": [ch],
            "typed": { "subject": "hero", "predicate": "at", "object": { "kind": "entity", "id": place } }
        });
        if let Some(p) = prev {
            f["supersedes_in_frame"] = serde_json::json!(p);
        }
        f
    };
    let kinds = |v: &serde_json::Value| -> Vec<String> {
        v["violations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x["kind"].as_str().unwrap().to_string())
            .collect()
    };

    // A — good walk a→b→c along undirected edges: no violation, exit 0.
    write_rules(true);
    write_store(serde_json::json!({
        "e-ab": edge("p-a", "p-b"), "e-bc": edge("p-b", "p-c"),
        "at-1": at("ch-1", "p-a", None),
        "at-2": at("ch-2", "p-b", Some("at-1")),
        "at-3": at("ch-3", "p-c", Some("at-2")),
    }));
    let out = run(
        ws,
        &["validate-continuity", "--severity", "reject", "--json"],
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    assert!(out.status.success(), "adjacent walk passes: {v}");
    assert_eq!(v["violation_count"], 0);

    // F — a disconnected map: the island {p-c, p-d} is unreachable from
    // {p-a, p-b}. The undirected connectivity gate (G4, R702) flags
    // map_disconnected and exits 1.
    write_rules(true);
    write_store(serde_json::json!({
        "e-ab": edge("p-a", "p-b"), "e-cd": edge("p-c", "p-d"),
    }));
    let out = run(
        ws,
        &["validate-continuity", "--severity", "reject", "--json"],
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    assert!(!out.status.success(), "disconnected map gates: {v}");
    assert!(
        kinds(&v).contains(&"map_disconnected".to_string()),
        "connectivity gate fires: {v}"
    );

    // B — a non-adjacent jump a→c (no a-c edge): gated, exit 1.
    write_store(serde_json::json!({
        "e-ab": edge("p-a", "p-b"), "e-bc": edge("p-b", "p-c"),
        "at-1": at("ch-1", "p-a", None),
        "at-2": at("ch-2", "p-c", Some("at-1")),
    }));
    let out = run(
        ws,
        &["validate-continuity", "--severity", "reject", "--json"],
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    assert!(!out.status.success(), "non-adjacent jump gates: {v}");
    assert!(
        kinds(&v).contains(&"rule_transition_invalid".to_string()),
        "{v}"
    );

    // C — the reverse of an edge: directed REJECTS, undirected ADMITS.
    let reverse_store = serde_json::json!({
        "e-ab": edge("p-a", "p-b"),
        "at-1": at("ch-1", "p-b", None),
        "at-2": at("ch-2", "p-a", Some("at-1")),
    });
    write_store(reverse_store);
    write_rules(false); // directed
    let out = run(
        ws,
        &["validate-continuity", "--severity", "reject", "--json"],
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    assert!(
        !out.status.success(),
        "directed rejects the reverse step: {v}"
    );
    assert!(kinds(&v).contains(&"rule_transition_invalid".to_string()));
    write_rules(true); // undirected
    let out = run(
        ws,
        &["validate-continuity", "--severity", "reject", "--json"],
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    assert!(
        out.status.success(),
        "undirected admits the reverse step: {v}"
    );

    // D — a self-loop + a reverse-duplicate edge: both integrity gates fire.
    write_rules(true);
    let mut selfloop = edge("p-a", "p-a");
    selfloop["entities"] = serde_json::json!(["p-a"]); // one place, listed once
    write_store(serde_json::json!({
        "e-self": selfloop,
        "e-ab": edge("p-a", "p-b"),
        "e-ba": edge("p-b", "p-a"), // reverse dup of e-ab
        "at-1": at("ch-1", "p-a", None),
    }));
    let out = run(
        ws,
        &["validate-continuity", "--severity", "reject", "--json"],
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    assert!(!out.status.success(), "malformed edges gate: {v}");
    let ks = kinds(&v);
    assert!(ks.contains(&"adjacency_self_loop".to_string()), "{v}");
    assert!(
        ks.contains(&"adjacency_reverse_duplicate".to_string()),
        "{v}"
    );

    // E — a scalar-subject `adjacent` fact is rejected by the REAL write path
    // (its subject "alive" is not a registered entity): store-native adjacency
    // needs entity endpoints, the entity-subject requirement (R697 finding A).
    write_store(serde_json::json!({ "at-1": at("ch-1", "p-a", None) }));
    let manifest = ws.join("scalar-edge.json");
    fs::write(
        &manifest,
        serde_json::json!({
            "predicates": [ { "predicate_id": "life-adjacent", "object_kind": "entity" } ],
            "facts": [ {
                "fact_id": "f-scalar-edge", "frame": "gt", "entities": ["alive", "dead"],
                "claim": "alive borders dead", "canon_from": "ch-1", "evidence": ["ch-1"],
                "typed": { "subject": "alive", "predicate": "life-adjacent",
                           "object": { "kind": "entity", "id": "dead" } }
            } ]
        })
        .to_string(),
    )
    .unwrap();
    let out = run(
        ws,
        &[
            "import-facts",
            "--manifest",
            manifest.to_str().unwrap(),
            "--sidecar",
            "docs/.atomic/workspace.atomic.json",
        ],
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "scalar subject must be rejected: {out:?}"
    );
    assert!(
        stderr.contains("alive") && stderr.contains("entity registry"),
        "the reject names the unregistered scalar subject: {stderr}"
    );
}

/// Round 703 (G2) — the store-native map completeness + container invariants
/// gate movement through the REAL `validate-continuity` binary (the R689-twin
/// discipline: a COMMITTED CLI proof, not an ephemeral dogfood). A complete map
/// whose every `place` entity is a node or a container passes (exit 0); adding
/// a `place` entity off the map fires `map_invented_place`, a container walked on
/// as a node fires `map_container_as_node`, and a region holding an off-map place
/// fires `map_contained_off_map` (each exit 1). Places a—b—c with a container
/// `p-region`; `p-a`'s `subject_kind` derives the place kind from the predicate.
#[test]
fn store_native_map_g2_completeness_and_containers_end_to_end() {
    let tmp = TempDir::new().unwrap();
    let ws = tmp.path();
    fs::create_dir_all(ws.join("docs/.atomic")).unwrap();
    fs::write(
        ws.join("mnemosyne.toml"),
        "[workspace]\n[continuity]\ncanon_order_path = \"canon-order.json\"\n\
         rules_path = \"narrative-rules.json\"\n",
    )
    .unwrap();
    fs::write(
        ws.join("canon-order.json"),
        serde_json::json!({ "edges": [["ch-1", "ch-2"]] }).to_string(),
    )
    .unwrap();
    // The rule declares BOTH map legs: `adjacency` = adjacent, `containment` =
    // contains (Round 703). `adjacent` declares `subject_kind = place`, so the
    // completeness check derives the place kind from the predicate (Round 701).
    fs::write(
        ws.join("narrative-rules.json"),
        serde_json::json!({
            "schema": "narrative-rules/v1",
            "rules": [ { "id": "roads", "class": "transition", "predicate": "at",
                         "adjacency": "adjacent", "undirected": true,
                         "containment": "contains" } ]
        })
        .to_string(),
    )
    .unwrap();

    // `extra_entities` / `extra_facts` splice in the per-case defect.
    let write_store = |extra_entities: serde_json::Value, extra_facts: serde_json::Value| {
        let mut entities = serde_json::json!({
            "p-a": { "kind": "place" }, "p-b": { "kind": "place" },
            "p-c": { "kind": "place" }, "p-region": { "kind": "place" },
            "hero": { "kind": "character" }
        });
        for (k, v) in extra_entities.as_object().unwrap() {
            entities[k] = v.clone();
        }
        let mut facts = serde_json::json!({
            "e-ab": { "frame": "gt", "entities": ["p-a", "p-b"], "claim": "a borders b",
                      "canon_from": "ch-1", "evidence": ["ch-1"],
                      "typed": { "subject": "p-a", "predicate": "adjacent",
                                 "object": { "kind": "entity", "id": "p-b" } } },
            "e-bc": { "frame": "gt", "entities": ["p-b", "p-c"], "claim": "b borders c",
                      "canon_from": "ch-1", "evidence": ["ch-1"],
                      "typed": { "subject": "p-b", "predicate": "adjacent",
                                 "object": { "kind": "entity", "id": "p-c" } } },
            "c-ra": { "frame": "gt", "entities": ["p-region", "p-a"], "claim": "region holds a",
                      "canon_from": "ch-1", "evidence": ["ch-1"],
                      "typed": { "subject": "p-region", "predicate": "contains",
                                 "object": { "kind": "entity", "id": "p-a" } } },
            "at-1": { "frame": "gt", "entities": ["hero", "p-a"], "claim": "hero at a",
                      "canon_from": "ch-1", "evidence": ["ch-1"],
                      "typed": { "subject": "hero", "predicate": "at",
                                 "object": { "kind": "entity", "id": "p-a" } } }
        });
        for (k, v) in extra_facts.as_object().unwrap() {
            facts[k] = v.clone();
        }
        let atomic = serde_json::json!({
            "schema_version": 25,
            "sections": { "ch-1": {}, "ch-2": {} },
            "changelog_entries": {},
            "frames": { "gt": {} },
            "entity_kinds": { "place": {}, "character": {} },
            "entities": entities,
            "predicates": {
                "adjacent": { "object_kind": "entity", "subject_kind": "place",
                              "object_entity_kind": "place" },
                "contains": { "object_kind": "entity" },
                "at": { "object_kind": "entity" }
            },
            "narrative_facts": facts
        });
        fs::write(
            ws.join("docs/.atomic/workspace.atomic.json"),
            serde_json::to_string_pretty(&atomic).unwrap(),
        )
        .unwrap();
    };
    let none = || serde_json::json!({});
    let kinds = |v: &serde_json::Value| -> Vec<String> {
        v["violations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x["kind"].as_str().unwrap().to_string())
            .collect()
    };
    let scan = || {
        let out = run(
            ws,
            &["validate-continuity", "--severity", "reject", "--json"],
        );
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
        (out.status.success(), v)
    };

    // Clean: p-a/p-b/p-c nodes, p-region a container of p-a; every place covered.
    write_store(none(), none());
    let (ok, v) = scan();
    assert!(ok, "a complete map + container passes: {v}");
    assert_eq!(v["violation_count"], 0);

    // Invented place: p-ghost is `kind:place` but off the map.
    write_store(
        serde_json::json!({ "p-ghost": { "kind": "place" } }),
        none(),
    );
    let (ok, v) = scan();
    assert!(!ok, "an off-map place gates: {v}");
    assert!(
        kinds(&v).contains(&"map_invented_place".to_string()),
        "completeness fires: {v}"
    );

    // Container leak: p-region (a `contains` subject) also appears in adjacency.
    write_store(
        none(),
        serde_json::json!({
            "e-rc": { "frame": "gt", "entities": ["p-region", "p-c"], "claim": "region borders c",
                      "canon_from": "ch-1", "evidence": ["ch-1"],
                      "typed": { "subject": "p-region", "predicate": "adjacent",
                                 "object": { "kind": "entity", "id": "p-c" } } }
        }),
    );
    let (ok, v) = scan();
    assert!(!ok, "a container-as-node leak gates: {v}");
    assert!(
        kinds(&v).contains(&"map_container_as_node".to_string()),
        "container leak fires: {v}"
    );

    // Contained off-map: p-region contains p-far, which is in no adjacent fact.
    // p-far is unkinded so it is not ALSO flagged as an invented place.
    write_store(
        serde_json::json!({ "p-far": {} }),
        serde_json::json!({
            "c-rf": { "frame": "gt", "entities": ["p-region", "p-far"], "claim": "region holds far",
                      "canon_from": "ch-1", "evidence": ["ch-1"],
                      "typed": { "subject": "p-region", "predicate": "contains",
                                 "object": { "kind": "entity", "id": "p-far" } } }
        }),
    );
    let (ok, v) = scan();
    assert!(!ok, "a contained off-map place gates: {v}");
    let ks = kinds(&v);
    assert!(ks.contains(&"map_contained_off_map".to_string()), "{v}");
    assert!(
        !ks.contains(&"map_invented_place".to_string()),
        "an unkinded contained place is not also an invented place: {v}"
    );
}

/// Round 711 (the R710 LOW-3 deferral) — the edge-cost adjacency SEMANTIC through
/// the REAL binary (the R689/R699 committed-proof discipline). The two layers,
/// end-to-end: the store-layer `add-edge-cost` write path ACCEPTS a cost on ANY
/// existing fact (it cannot know which predicate is the map's adjacency without
/// rules config), and `validate-continuity` — which DOES have the rules — then
/// catches a cost keyed to a non-adjacency fact as `edge_cost_not_an_edge`. A
/// cost on a real `adjacent` edge is clean. Places a—b—c, a stray `loves` fact.
#[test]
fn store_native_map_edge_cost_semantic_end_to_end() {
    let tmp = TempDir::new().unwrap();
    let ws = tmp.path();
    fs::create_dir_all(ws.join("docs/.atomic")).unwrap();
    fs::write(
        ws.join("mnemosyne.toml"),
        "[workspace]\n[continuity]\ncanon_order_path = \"canon-order.json\"\n\
         rules_path = \"narrative-rules.json\"\n",
    )
    .unwrap();
    fs::write(
        ws.join("canon-order.json"),
        serde_json::json!({ "edges": [["ch-1", "ch-2"]] }).to_string(),
    )
    .unwrap();
    fs::write(
        ws.join("narrative-rules.json"),
        serde_json::json!({
            "schema": "narrative-rules/v1",
            "rules": [ { "id": "roads", "class": "transition", "predicate": "at",
                         "adjacency": "adjacent", "undirected": true } ]
        })
        .to_string(),
    )
    .unwrap();
    // A complete connected map a—b—c + a stray non-map `loves` fact. `minute`
    // is registered so `add-edge-cost` (which requires a registered unit) can run.
    let atomic = serde_json::json!({
        "schema_version": 30,
        "sections": { "ch-1": {}, "ch-2": {} },
        "changelog_entries": {},
        "frames": { "gt": {} },
        "entity_kinds": { "place": {} },
        "entities": { "p-a": { "kind": "place" }, "p-b": { "kind": "place" }, "p-c": { "kind": "place" } },
        "units": { "minute": {} },
        "predicates": {
            "adjacent": { "object_kind": "entity", "subject_kind": "place", "object_entity_kind": "place" },
            "at": { "object_kind": "entity" },
            "loves": { "object_kind": "entity" }
        },
        "narrative_facts": {
            "e-ab": { "frame": "gt", "entities": ["p-a", "p-b"], "claim": "a borders b",
                      "canon_from": "ch-1", "evidence": ["ch-1"],
                      "typed": { "subject": "p-a", "predicate": "adjacent",
                                 "object": { "kind": "entity", "id": "p-b" } } },
            "e-bc": { "frame": "gt", "entities": ["p-b", "p-c"], "claim": "b borders c",
                      "canon_from": "ch-1", "evidence": ["ch-1"],
                      "typed": { "subject": "p-b", "predicate": "adjacent",
                                 "object": { "kind": "entity", "id": "p-c" } } },
            "f-loves": { "frame": "gt", "entities": ["p-a", "p-b"], "claim": "a loves b",
                         "canon_from": "ch-1", "evidence": ["ch-1"],
                         "typed": { "subject": "p-a", "predicate": "loves",
                                    "object": { "kind": "entity", "id": "p-b" } } }
        }
    });
    fs::write(
        ws.join("docs/.atomic/workspace.atomic.json"),
        serde_json::to_string_pretty(&atomic).unwrap(),
    )
    .unwrap();

    let add_cost = |fact: &str| {
        run(
            ws,
            &[
                "add-edge-cost",
                "--fact",
                fact,
                "--n",
                "4",
                "--unit",
                "minute",
            ],
        )
    };
    let kinds = |v: &serde_json::Value| -> Vec<String> {
        v["violations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x["kind"].as_str().unwrap().to_string())
            .collect()
    };
    let scan = || {
        let out = run(
            ws,
            &["validate-continuity", "--severity", "reject", "--json"],
        );
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
        (out.status.success(), v)
    };

    // A cost on a real `adjacent` edge: the write path lands it and the map is
    // clean — no edge-cost violation.
    assert!(
        add_cost("e-ab").status.success(),
        "a cost on a real edge lands"
    );
    let (ok, v) = scan();
    assert!(ok, "a cost on a real edge is clean: {v}");
    assert!(!kinds(&v).contains(&"edge_cost_not_an_edge".to_string()));

    // A cost on the non-adjacency `loves` fact: the store-layer write path
    // ACCEPTS it (it cannot know `loves` is not the map's adjacency) — the gap
    // R711 closes at the read layer.
    assert!(
        add_cost("f-loves").status.success(),
        "the write path accepts a cost on any existing fact (the R710 LOW-3 gap)"
    );
    // `validate-continuity`, holding the rules, catches it and gates.
    let (ok, v) = scan();
    assert!(!ok, "a cost on a non-edge gates: {v}");
    assert!(
        kinds(&v).contains(&"edge_cost_not_an_edge".to_string()),
        "the adjacency-semantic gate fires: {v}"
    );
    let hit = v["violations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["kind"] == "edge_cost_not_an_edge")
        .unwrap();
    assert_eq!(hit["fact"], "f-loves", "it names the offending fact: {v}");
    assert_eq!(hit["found"], "loves", "and its actual predicate: {v}");
}
