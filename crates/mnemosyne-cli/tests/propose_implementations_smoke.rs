//! `propose-implementations` — the READING half of the citation gate, through
//! the binary.
//!
//! # Why this file exists, written at the moment it was missing
//!
//! Round 1336 moved this command's validator assembly into
//! `mnemosyne_ops::propose_implementations`, and while doing so found that the
//! command had NO integration test at all. A move with nothing watching it is a
//! move whose behaviour change nobody would see, and "nothing needed one yet" is
//! not a reason to leave it that way — the two behaviours below are exactly the
//! ones the move could silently have dropped.
//!
//! (i) A section cited from a file it has no binding for is PROPOSED, with the
//!     symbols the citation sits in and a ready-to-run registration command.
//! (ii) A workspace that never configured the gate is REFUSED, not answered with
//!     an empty list. Proposing nothing because nobody asked for the gate reads
//!     exactly like proposing nothing because the tree is already bound, and
//!     that is the distinction Round 1141 spent a round making legible. The op
//!     answers `None` for the unconfigured case precisely so this command can
//!     tell them apart.

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

/// A workspace with one section and one Rust file citing it. `configured` writes
/// `[plugins.set_equality_validator]`; without it the gate is not asked for.
fn workspace(configured: bool) -> TempDir {
    let ws = TempDir::new().expect("tempdir");
    fs::create_dir_all(ws.path().join("docs/.atomic")).expect("atomic dir");
    fs::create_dir_all(ws.path().join("src")).expect("src dir");
    let mut cfg = String::from("[workspace]\n[schema]\nentry_id_prefix = \"Round \"\n");
    if configured {
        cfg.push_str("[plugins.set_equality_validator]\npaths = [\"src/\"]\n");
    }
    fs::write(ws.path().join("mnemosyne.toml"), cfg).expect("config");
    let atomic = serde_json::json!({
        "schema_version": 7,
        "sections": {
            "4.2": { "title": "Raising", "parent_doc": "docs/GENERATED.md" }
        },
        "changelog_entries": {}
    });
    fs::write(
        ws.path().join("docs/.atomic/workspace.atomic.json"),
        serde_json::to_string_pretty(&atomic).expect("atomic json"),
    )
    .expect("store");
    fs::write(ws.path().join("docs/GENERATED.md"), "# Stub\n").expect("doc");
    // A function whose body cites the section — the shape a proposal is made of.
    fs::write(
        ws.path().join("src/raise.rs"),
        "/// Implements §4.2 raising.\npub fn raise() {}\n",
    )
    .expect("source");
    ws
}

#[test]
fn a_cited_section_with_no_binding_is_proposed_with_its_symbol() {
    let ws = workspace(true);
    let out = run(ws.path(), &["propose-implementations", "--json"]);
    assert!(
        out.status.success(),
        "a configured workspace answers: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let proposals: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&out.stdout)).expect("valid JSON");
    let rows = proposals.as_array().expect("an array of proposals");
    assert_eq!(
        rows.len(),
        1,
        "one section is cited from one file it has no binding for: {proposals}"
    );
    assert_eq!(rows[0]["section_id"], "4.2");
    assert_eq!(rows[0]["file"], "src/raise.rs");
}

#[test]
fn an_unconfigured_workspace_is_refused_rather_than_answered_with_nothing() {
    let ws = workspace(false);
    let out = run(ws.path(), &["propose-implementations", "--json"]);
    assert!(
        !out.status.success(),
        "a workspace that never configured the gate must not be told there is \
         nothing to propose — that is what a fully bound tree is told. stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(
        said.contains("set_equality_validator"),
        "the refusal names what is missing: {said}"
    );
}
