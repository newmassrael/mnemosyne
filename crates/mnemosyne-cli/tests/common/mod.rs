//! The dnd-quest authored store, rebuilt the way its runbook does — THE one
//! builder, shared by every test that needs the only blind-authored branching
//! corpus this repo can load.
//!
//! Round 857 found that this fixture had stopped loading and nobody noticed for
//! ~150 rounds; `dnd_quest_map_seam_smoke.rs` exists so CI loads it every run.
//! Round 1031 needed the same store to judge quest prerequisites against real
//! authored roads, and a SECOND copy of the recipe is how the two tests would
//! come to disagree about which store they are talking about — so the recipe
//! lives here and both read it.

#![allow(dead_code)] // each test binary uses a different part of this module

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

pub fn cli_binary() -> &'static str {
    env!("CARGO_BIN_EXE_mnemosyne-cli")
}

pub fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR = <root>/crates/mnemosyne-cli
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repo root is two levels above the crate manifest")
        .to_path_buf()
}

/// The frozen experiment record. Read, never written: it is the blind author's
/// own output and the experiment's sha-pinned evidence.
pub fn audit_dir() -> PathBuf {
    repo_root().join("claudedocs/phase1-dnd-quest-experiment/v3/run/author")
}

pub fn run(workspace: &Path, args: &[&str]) -> std::process::Output {
    Command::new(cli_binary())
        .args(args)
        .current_dir(workspace)
        .output()
        .expect("cli exec")
}

pub fn run_ok(workspace: &Path, args: &[&str]) -> String {
    let out = run(workspace, args);
    assert!(
        out.status.success(),
        "{args:?} failed: {}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    String::from_utf8(out.stdout).expect("cli output is utf-8")
}

pub fn json_report(workspace: &Path, args: &[&str]) -> serde_json::Value {
    let mut full = args.to_vec();
    full.push("--json");
    serde_json::from_str(&run_ok(workspace, &full)).expect("report json")
}

/// The tracked fact manifest — the migrated half of the frozen record.
pub fn dnd_quest_facts() -> serde_json::Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dnd-quest/facts.json");
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("fixture {} unreadable: {e}", path.display()));
    serde_json::from_str(&text).expect("fixture json")
}

/// Rebuild the store the way the experiment's runbook does — fresh seed, then
/// the manifests — with the three unchanged manifests taken from the frozen
/// record and the fact manifest supplied by the caller, so a test can author a
/// DEFECT into the store an author could equally have authored (the manifest is
/// the authoring path, and it validates: a corruption the import rejects is a
/// corruption no author could have shipped).
pub fn dnd_quest_workspace_from(facts: &serde_json::Value) -> TempDir {
    dnd_quest_workspace_try(facts).unwrap_or_else(|e| panic!("the fact manifest must import: {e}"))
}

/// The same recipe, with the fact import's REFUSAL handed back rather than
/// asserted. A walk that corrupts the manifest needs "the write path rejected
/// this" as a VERDICT about the corruption, not as a failure of the walk.
pub fn dnd_quest_workspace_try(facts: &serde_json::Value) -> Result<TempDir, String> {
    for name in ["sections.json", "order.json", "narrative-rules.json"] {
        let src = audit_dir().join(name);
        assert!(
            src.exists(),
            "the frozen record must hold {}",
            src.display()
        );
    }
    corpus_workspace_try(&audit_dir(), facts)
}

/// The same recipe over ANY authored corpus directory this tree tracks. The
/// dnd-quest builder is this one with the frozen record's path: a corpus is a
/// `sections.json` + `order.json` beside a fact manifest, and nothing about the
/// recipe is specific to which author wrote it. `narrative-rules.json` is
/// optional — not every corpus declares rules, and a config naming a file that
/// is not there is a load failure rather than a corpus without rules.
pub fn corpus_workspace_try(dir: &Path, facts: &serde_json::Value) -> Result<TempDir, String> {
    let tmp = TempDir::new().expect("tempdir");
    let ws = tmp.path();
    fs::create_dir_all(ws.join("docs/.atomic")).expect("mkdir");

    let mut rules = false;
    for name in ["sections.json", "order.json", "narrative-rules.json"] {
        let src = dir.join(name);
        if !src.exists() {
            continue;
        }
        fs::copy(&src, ws.join(name)).map_err(|e| format!("copy {}: {e}", src.display()))?;
        rules |= name == "narrative-rules.json";
    }
    fs::write(
        ws.join("facts.json"),
        serde_json::to_string(facts).expect("facts serialize"),
    )
    .expect("write facts");

    fs::write(
        ws.join("mnemosyne.toml"),
        format!(
            "[workspace]\n[continuity]\ncanon_order_path = \"order.json\"\n{}",
            if rules {
                "rules_path = \"narrative-rules.json\"\n"
            } else {
                ""
            }
        ),
    )
    .expect("write config");
    fs::write(
        ws.join("docs/.atomic/workspace.atomic.json"),
        serde_json::json!({
            "schema_version": 23,
            "sections": {},
            "changelog_entries": {}
        })
        .to_string(),
    )
    .expect("write seed");

    for import in [
        ["import-sections", "--manifest", "sections.json"],
        ["import-facts", "--manifest", "facts.json"],
    ] {
        let out = run(ws, &import);
        if !out.status.success() {
            return Err(format!(
                "{}: {}",
                import[0],
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
    }
    Ok(tmp)
}

/// The authored store exactly as the blind author left it.
pub fn dnd_quest_workspace() -> TempDir {
    dnd_quest_workspace_from(&dnd_quest_facts())
}
