//! An annotation inside a document cites every inventory id its marker
//! encloses — through the real `validate-code-refs`.
//!
//! Round 1322 read a marker as a prefix followed by the section-path tail
//! class, so `req="REQ-4.2.1 REQ-7"` cited `REQ-4.2.1` and never `REQ-7`, while
//! the adopter's own state-machine documents put two ids in one attribute on
//! seven of twenty-five annotated lines. A marker is now a pair of delimiters
//! and what it encloses is a list: a deprecated id second in its list is
//! reported, an id on the line after its marker is reported on that line, and
//! a marker that never closes is reported instead of read as citing nothing.

use serde_json::Value;
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

/// The declaration every marker test here uses: an XML attribute `req="…"`.
const REQ_ATTRIBUTE: &str = r#"inventory_markers = [{ open = "req=\"", close = "\"" }]"#;

/// A workspace declaring `axis` over `doc/model.scxml`, whose text is
/// `document`, with one active entry (`REQ-4.2.1`) and one deprecated entry
/// (`REQ-7`) registered.
fn workspace(axis: &str, document: &str) -> TempDir {
    let ws = TempDir::new().expect("tempdir");
    fs::write(
        ws.path().join("mnemosyne.toml"),
        format!(
            "[workspace]\n\n[plugins.set_equality_validator]\npaths = [\"doc/\"]\n\
             {axis}\nseverity_inventory = \"reject\"\n"
        ),
    )
    .expect("config");
    for args in [
        &[
            "add-inventory-entry",
            "--id",
            "REQ-4.2.1",
            "--status",
            "active",
        ][..],
        &[
            "add-inventory-entry",
            "--id",
            "REQ-7",
            "--status",
            "deprecated",
            "--reason",
            "superseded",
        ][..],
    ] {
        let out = run(ws.path(), args);
        assert!(
            out.status.success(),
            "fixture: `{}` failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    fs::create_dir_all(ws.path().join("doc")).expect("doc dir");
    fs::write(ws.path().join("doc/model.scxml"), document).expect("document");
    ws
}

/// Whether the gate passed, and every violation it reported as
/// `(kind, entry_id, line)`, sorted.
fn reported(workspace: &Path) -> (bool, Vec<(String, String, u64)>) {
    let out = run(workspace, &["validate-code-refs", "--json"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let report: Value = stdout
        .lines()
        .find(|line| line.starts_with('{'))
        .and_then(|line| serde_json::from_str(line).ok())
        .unwrap_or_else(|| {
            panic!(
                "the gate prints one JSON report: stdout {stdout} stderr {}",
                String::from_utf8_lossy(&out.stderr)
            )
        });
    let mut violations: Vec<(String, String, u64)> = report["violations"]
        .as_array()
        .expect("a violations array")
        .iter()
        .map(|v| {
            (
                v["kind"].as_str().expect("a kind").to_string(),
                v["entry_id"].as_str().expect("an entry_id").to_string(),
                v["line"].as_u64().expect("a line"),
            )
        })
        .collect();
    violations.sort();
    (out.status.success(), violations)
}

#[test]
fn a_marker_cites_every_id_it_encloses() {
    let ws = workspace(
        REQ_ATTRIBUTE,
        "<scxml>\n  <state id=\"idle\" req=\"REQ-4.2.1 REQ-7\"/>\n  \
         <state id=\"new\" req=\"\n    REQ-999\"/>\n</scxml>\n",
    );
    let (passed, violations) = reported(ws.path());
    assert!(
        !passed,
        "a deprecated and a missing citation at reject severity fail the gate: {violations:?}"
    );
    assert_eq!(
        violations,
        vec![
            ("inventory_deprecated".to_string(), "REQ-7".to_string(), 2),
            ("inventory_missing".to_string(), "REQ-999".to_string(), 4),
        ],
        "the deprecated id second in its list and the missing id on the line after its marker \
         are reported where they stand, and the active id is not"
    );
}

/// A marker whose close never follows is reported at the line it opened on —
/// and the deprecated id inside it is not, because an unclosed list has no end
/// and none of its ids is read.
#[test]
fn a_marker_that_never_closes_is_reported() {
    let ws = workspace(
        REQ_ATTRIBUTE,
        "<scxml>\n  <state id=\"idle\" req=\"REQ-7/>\n</scxml>\n",
    );
    let (passed, violations) = reported(ws.path());
    assert!(
        !passed,
        "an unclosed marker at reject severity fails the gate: {violations:?}"
    );
    assert_eq!(
        violations,
        vec![(
            "inventory_marker_unclosed".to_string(),
            "req=\"".to_string(),
            2
        )],
        "the marker is reported where it opened, not read as citing nothing"
    );
}

/// CONTROL: the same annotation under the path axis keeps the attribute's
/// syntax in the id, so even the active entry is reported — the shape a
/// delimited marker exists for.
#[test]
fn the_same_annotation_under_the_path_axis_misses_the_active_entry() {
    let ws = workspace(
        r#"inventory_path_prefixes = ["req=\""]"#,
        "<scxml>\n  <state id=\"idle\" req=\"REQ-4.2.1\"/>\n</scxml>\n",
    );
    let (_, violations) = reported(ws.path());
    assert_eq!(
        violations,
        vec![(
            "inventory_missing".to_string(),
            "req=\"REQ-4.2.1".to_string(),
            2
        )],
        "control: the path axis reads the attribute's syntax into the id"
    );
}
