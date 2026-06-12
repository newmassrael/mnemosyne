//! End-to-end smoke test: drive the built binary the way the orchestrator does,
//! asserting the exit codes that make failures loud. The unit tests cover the
//! transforms; this covers dispatch, arg parsing, and process exit status.

use std::fs;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_experiment-harness");

const STORY: &str = "\
# Belvoir

Preamble, ignored.

---

## sc-01 \u{2014} The Line Goes Down

<!-- SESSION 1 -->
The storm made an island of them.

---

## sc-02 \u{2014} The Locked Room

The consulting room was at the quiet end.

CHOICE: Cendre acts on the ledger.
  A) She confronts the matron. \u{2192} continues sc-21

---

## sc-21 \u{2014} The Confrontation

She laid the ledger on the table.
";

const PLAYTHROUGH: &str = r#"{ "worlds": {
  "confront": { "scenes": [{"section":"sc-01"},{"section":"sc-02"},{"section":"sc-21"}] }
} }"#;

fn workdir(tag: &str) -> std::path::PathBuf {
    let mut d = std::env::temp_dir();
    d.push(format!("eh-cli-smoke-{tag}-{}", std::process::id()));
    fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn assemble_then_shuffle_then_verify_roundtrip() {
    let dir = workdir("roundtrip");
    let story = dir.join("story.md");
    let pt = dir.join("pt.json");
    let map = dir.join("label-map.json");
    fs::write(&story, STORY).unwrap();
    fs::write(&pt, PLAYTHROUGH).unwrap();

    // assemble to stdout
    let out = Command::new(BIN)
        .args([
            "assemble",
            "--story",
            story.to_str().unwrap(),
            "--playthrough",
            pt.to_str().unwrap(),
            "--world",
            "confront",
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "assemble should exit 0");
    let manuscript = String::from_utf8(out.stdout).unwrap();
    assert!(manuscript.contains("## The Line Goes Down"));
    assert!(!manuscript.contains("<!--"));
    assert!(!manuscript.contains("CHOICE:"));
    assert!(!manuscript.contains("sc-01 \u{2014}"));
    // playthrough order, not file order
    assert!(manuscript.find("Confrontation").unwrap() > manuscript.find("Locked Room").unwrap());

    // shuffle -> the seal is the sole stdout line
    let out = Command::new(BIN)
        .args([
            "shuffle",
            "--experiment",
            "belvoir-smoke",
            "--note",
            "reveal later",
            "--out",
            map.to_str().unwrap(),
            "plain",
            "loop",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let seal = String::from_utf8(out.stdout).unwrap().trim().to_string();
    assert_eq!(seal.len(), 64);

    // verify-seal with the right hash -> MATCH, exit 0
    let ok = Command::new(BIN)
        .args([
            "verify-seal",
            "--map",
            map.to_str().unwrap(),
            "--sha256",
            &seal,
        ])
        .output()
        .unwrap();
    assert!(ok.status.success(), "correct seal should exit 0");
    assert!(String::from_utf8(ok.stdout).unwrap().starts_with("MATCH"));

    // verify-seal with a wrong hash -> MISMATCH, exit 1
    let bad = Command::new(BIN)
        .args([
            "verify-seal",
            "--map",
            map.to_str().unwrap(),
            "--sha256",
            &"0".repeat(64),
        ])
        .output()
        .unwrap();
    assert_eq!(bad.status.code(), Some(1), "tampered seal must exit 1");

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn missing_scene_in_order_exits_loud() {
    let dir = workdir("missing");
    let story = dir.join("story.md");
    let pt = dir.join("pt.json");
    fs::write(&story, STORY).unwrap();
    fs::write(
        &pt,
        r#"{ "worlds": { "confront": { "scenes": [{"section":"sc-404"}] } } }"#,
    )
    .unwrap();

    let out = Command::new(BIN)
        .args([
            "assemble",
            "--story",
            story.to_str().unwrap(),
            "--playthrough",
            pt.to_str().unwrap(),
            "--world",
            "confront",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "missing scene must fail loud");
    assert!(String::from_utf8(out.stderr).unwrap().contains("sc-404"));

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn unexpected_flag_exits_loud() {
    // All required flags present plus a stray one: the stray is caught at
    // finish() before any file is touched.
    let out = Command::new(BIN)
        .args([
            "assemble",
            "--story",
            "/dev/null",
            "--playthrough",
            "/dev/null",
            "--world",
            "w",
            "--nope",
            "y",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8(out.stderr)
        .unwrap()
        .contains("unexpected argument"));
}

#[test]
fn missing_required_flag_exits_loud() {
    let out = Command::new(BIN).args(["assemble"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8(out.stderr)
        .unwrap()
        .contains("missing required flag"));
}
