//! R425 — blanket-binding detector + unconfirmed-verifies surface smoke tests
//! (SCE field-report P1 + P4).
//!
//! (i)   detector is OPT-IN — default run emits `blanket_verifies_count == 0`
//!       and `severity_blanket == null` even with a blanket binding present.
//! (ii)  `--severity-blanket reject` flags one artifact verifies-bound to two
//!       sections and exits 1.
//! (iii) `unconfirmed_verifies` appears in default JSON output independent of
//!       any severity knob (the P4 anti-complacency line) — here 2: both
//!       verifies bindings are bound but unconfirmed.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

fn cli() -> &'static str {
    env!("CARGO_BIN_EXE_mnemosyne-cli")
}

fn write_workspace(ws: &Path) {
    fs::create_dir_all(ws.join("docs/.atomic")).unwrap();
    fs::create_dir_all(ws.join("src")).unwrap();
    let cfg = "[workspace]\n[schema]\nentry_id_prefix = \"Round \"\n\
        [plugins.set_equality_validator]\npaths = [\"src/\"]\n";
    fs::write(ws.join("mnemosyne.toml"), cfg).unwrap();
    // One test artifact verifies-bound to TWO sibling sections = blanket.
    let atomic = serde_json::json!({
        "schema_version": 11,
        "sections": {
            "6.4.1": { "title": "A", "parent_doc": "d",
                "bindings": [{ "file": "t/Test215.h", "symbol": "t215", "kind": "verifies" }] },
            "6.4.2": { "title": "B", "parent_doc": "d",
                "bindings": [{ "file": "t/Test215.h", "symbol": "t215", "kind": "verifies" }] }
        },
        "changelog_entries": {}
    });
    fs::write(
        ws.join("docs/.atomic/workspace.atomic.json"),
        serde_json::to_string_pretty(&atomic).unwrap(),
    )
    .unwrap();
}

fn validate(ws: &Path, extra: &[&str]) -> (Output, serde_json::Value) {
    let mut args = vec![
        "validate-code-refs",
        "--json",
        "--severity-coverage",
        "warn",
    ];
    args.extend_from_slice(extra);
    let out = Command::new(cli())
        .args(&args)
        .current_dir(ws)
        .output()
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|_| panic!("stdout not json: {}", String::from_utf8_lossy(&out.stdout)));
    (out, json)
}

#[test]
fn detector_off_by_default_and_unconfirmed_surfaced() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    let (out, json) = validate(tmp.path(), &[]);
    assert!(
        out.status.success(),
        "default run passes; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(json["blanket_verifies_count"], 0, "opt-in: off by default");
    assert_eq!(json["severity_blanket"], serde_json::Value::Null);
    // P4 — the bound-but-unconfirmed gap is visible WITHOUT any opt-in.
    assert_eq!(
        json["unconfirmed_verifies"], 2,
        "both verifies bindings are unconfirmed"
    );
}

#[test]
fn detector_rejects_blanket_binding() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    let (out, json) = validate(tmp.path(), &["--severity-blanket", "reject"]);
    assert!(!out.status.success(), "blanket binding must reject");
    assert_eq!(
        json["blanket_verifies_count"], 1,
        "one artifact flagged once"
    );
}
