//! Round 390 — `report-coverage` subcommand smoke tests.
//!
//! Read-only positive projection of the coverage axis. Test scope:
//! (i) mixed store → correct 3-way counts + ratio + JSON contract keys
//! (ii) all-Informative store → ratio is `null` (0 applicable), exit 0
//! (iii) read-only: a second run leaves the store byte-identical
//!
//! The JSON key names asserted here are the SCE CI contract — they mirror
//! `validate-spec-drift`'s contract test for the same reason.

use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn cli_binary() -> &'static str {
    env!("CARGO_BIN_EXE_mnemosyne-cli")
}

fn write_workspace(workspace: &Path, sections: serde_json::Value) {
    fs::create_dir_all(workspace.join("docs/.atomic")).unwrap();
    let cfg = "[workspace]\ndocs = [\"docs/GENERATED.md\"]\n\
        default_doc = \"docs/GENERATED.md\"\n[schema]\nentry_id_prefix = \"Round \"\n";
    fs::write(workspace.join("mnemosyne.toml"), cfg).unwrap();
    let atomic = serde_json::json!({
        "schema_version": 6,
        "sections": sections,
        "changelog_entries": {}
    });
    fs::write(
        workspace.join("docs/.atomic/workspace.atomic.json"),
        serde_json::to_string_pretty(&atomic).unwrap(),
    )
    .unwrap();
    fs::write(workspace.join("docs/GENERATED.md"), "# Stub\n").unwrap();
}

fn run_cli(workspace: &Path, args: &[&str]) -> std::process::Output {
    Command::new(cli_binary())
        .args(args)
        .current_dir(workspace)
        .output()
        .expect("cli exec")
}

#[test]
fn mixed_store_reports_three_way_breakdown_and_ratio() {
    let tmp = TempDir::new().unwrap();
    write_workspace(
        tmp.path(),
        serde_json::json!({
            // Normative (default) + implements binding → implemented.
            "bound": {
                "title": "Bound", "parent_doc": "docs/GENERATED.md",
                "bindings": [{"file": "src/foo.rs", "symbol": "Foo", "kind": "implements"}]
            },
            // Normative, zero implements → normative gap.
            "gap": { "title": "Gap", "parent_doc": "docs/GENERATED.md" },
            // Informative → exempt.
            "info": {
                "title": "Terminology", "parent_doc": "docs/GENERATED.md",
                "coverage_expectation": "out_of_scope_here"
            },
            // Removed Normative with no coverage → excluded, not a gap.
            "dead": {
                "title": "Dead", "parent_doc": "docs/GENERATED.md",
                "decision_status": "removed"
            }
        }),
    );
    let out = run_cli(tmp.path(), &["report-coverage", "--json"]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let parsed: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&out.stdout)).expect("valid JSON");
    assert_eq!(parsed["applicable"], 2);
    assert_eq!(parsed["implemented_count"], 1);
    assert_eq!(parsed["normative_gap_count"], 1);
    assert_eq!(parsed["informative_exempt_count"], 1);
    assert_eq!(parsed["removed_excluded_count"], 1);
    assert_eq!(parsed["coverage_ratio"], 0.5);
    assert_eq!(
        parsed["normative_gap"].as_array().unwrap(),
        &vec![serde_json::Value::String("gap".to_string())]
    );
    assert_eq!(
        parsed["implemented"].as_array().unwrap(),
        &vec![serde_json::Value::String("bound".to_string())]
    );
}

#[test]
fn all_informative_store_has_null_ratio() {
    let tmp = TempDir::new().unwrap();
    write_workspace(
        tmp.path(),
        serde_json::json!({
            "intro": {
                "title": "Intro", "parent_doc": "docs/GENERATED.md",
                "coverage_expectation": "out_of_scope_here"
            }
        }),
    );
    let out = run_cli(tmp.path(), &["report-coverage", "--json"]);
    assert!(out.status.success());
    let parsed: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&out.stdout)).expect("valid JSON");
    assert_eq!(parsed["applicable"], 0);
    assert_eq!(parsed["informative_exempt_count"], 1);
    assert!(
        parsed["coverage_ratio"].is_null(),
        "0 applicable sections → ratio null, got {}",
        parsed["coverage_ratio"]
    );
}

#[test]
fn report_coverage_is_read_only() {
    let tmp = TempDir::new().unwrap();
    write_workspace(
        tmp.path(),
        serde_json::json!({ "gap": { "title": "Gap", "parent_doc": "docs/GENERATED.md" } }),
    );
    let store_path = tmp.path().join("docs/.atomic/workspace.atomic.json");
    let before = fs::read(&store_path).unwrap();
    let out = run_cli(tmp.path(), &["report-coverage"]);
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("coverage report"), "stdout: {stdout}");
    let after = fs::read(&store_path).unwrap();
    assert_eq!(before, after, "report-coverage must not mutate the store");
}
