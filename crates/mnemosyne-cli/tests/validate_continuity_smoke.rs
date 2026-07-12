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
        "entities": { "dracula": { "kind": "character" } },
        "predicates": { "at-location": { "object_kind": "scalar" } },
        "narrative_facts": {
            "l1": {
                "frame": "gt",
                "entities": ["dracula"],
                "claim": "Dracula is at the castle",
                "canon_from": "ch-1",
                "evidence": ["ch-1"],
                "typed": { "subject": "dracula", "predicate": "at-location",
                           "object": { "kind": "value", "value": "castle" } }
            },
            "bad": {
                "frame": "gt",
                "entities": ["dracula"],
                "claim": "Dracula is at Whitby",
                "canon_from": "ch-2",
                "evidence": ["ch-2"],
                "typed": { "subject": "dracula", "predicate": "at-location",
                           "object": { "kind": "value", "value": "whitby" } }
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
    let scalar = |p: &str, v: &str, from: &str| {
        serde_json::json!({
            "frame": "gt", "entities": ["codicil"],
            "claim": format!("{p}={v}"), "canon_from": from, "evidence": [from],
            "typed": {"subject": "codicil", "predicate": p, "object": {"kind": "value", "value": v}}
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
            "min-ratify-gap-days": { "object_kind": "scalar" },
            "signed-on-day": { "object_kind": "scalar" },
            "ratified-on-day": { "object_kind": "scalar" }
        },
        "narrative_facts": {
            "f-rule": scalar("min-ratify-gap-days", "42", "ch-1"),
            "f-sign": scalar("signed-on-day", "10", "ch-1"),
            "f-rat": scalar("ratified-on-day", "15", "ch-2")
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
