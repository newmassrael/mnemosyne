//! Rounds 430/446 — narrative fact verbs smoke tests.
//!
//! End-to-end over the fact-shaped verbs: `import-facts` (bulk manifest,
//! atomic, forward succession), `add-frame`, `add-fact`, `add-fact-conflict`,
//! and the Round 446 typed-claim surface (`add-predicate` + typed flags).
//! Asserted against the store JSON, including the no-silent-overwrite and
//! fail-loud reject paths.

use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn cli_binary() -> &'static str {
    env!("CARGO_BIN_EXE_mnemosyne-cli")
}

/// Workspace with three chapter sections (canon coordinates are structure
/// refs, so facts need sections to point at).
fn write_workspace(workspace: &Path) {
    fs::create_dir_all(workspace.join("docs/.atomic")).unwrap();
    fs::write(workspace.join("mnemosyne.toml"), "[workspace]\n").unwrap();
    let atomic = serde_json::json!({
        "schema_version": 12,
        "sections": { "ch-1": {}, "ch-2": {}, "ch-3": {} },
        "changelog_entries": {}
    });
    fs::write(
        workspace.join("docs/.atomic/workspace.atomic.json"),
        serde_json::to_string_pretty(&atomic).unwrap(),
    )
    .unwrap();
}

fn run(workspace: &Path, args: &[&str]) -> std::process::Output {
    Command::new(cli_binary())
        .args(args)
        .current_dir(workspace)
        .output()
        .expect("cli exec")
}

fn read_store(workspace: &Path) -> serde_json::Value {
    let raw = fs::read_to_string(workspace.join("docs/.atomic/workspace.atomic.json")).unwrap();
    serde_json::from_str(&raw).unwrap()
}

fn write_manifest(workspace: &Path, value: serde_json::Value) -> String {
    let p = workspace.join("facts-manifest.json");
    fs::write(&p, serde_json::to_string_pretty(&value).unwrap()).unwrap();
    p.to_str().unwrap().to_string()
}

#[test]
fn import_facts_creates_frames_and_facts_with_forward_succession() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    let manifest = write_manifest(
        tmp.path(),
        serde_json::json!({
            "frames": [
                { "frame_id": "jonathan", "description": "Jonathan Harker" },
                { "frame_id": "ground-truth" }
            ],
            "facts": [
                {
                    "fact_id": "f-new", "frame": "jonathan",
                    "claim": "the count is something unnatural",
                    "canon_from": "ch-3",
                    "evidence": ["ch-3"],
                    "supersedes_in_frame": "f-old",
                    "quote": "he crawled face-down the castle wall"
                },
                {
                    "fact_id": "f-old", "frame": "jonathan",
                    "claim": "the count is an eccentric nobleman",
                    "canon_from": "ch-1", "canon_to": "ch-2",
                    "evidence": ["ch-1", "ch-2"]
                }
            ]
        }),
    );
    let out = run(tmp.path(), &["import-facts", "--manifest", &manifest]);
    assert!(out.status.success(), "{:?}", out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("2 frames + 0 branches + 0 entities + 0 predicates + 2 facts created"),
        "{stdout}"
    );
    let store = read_store(tmp.path());
    // CURRENT_SCHEMA_VERSION — bumped to 23 by R532 (Branch.converges_from).
    assert_eq!(store["schema_version"], 23);
    assert_eq!(
        store["narrative_facts"]["f-new"]["supersedes_in_frame"],
        "f-old"
    );
    // quote_sha256 stamped by the primitive.
    assert!(
        store["narrative_facts"]["f-new"]["quote_sha256"]
            .as_str()
            .unwrap()
            .len()
            == 64
    );
    // Stored canon_to on the successor-less end shape.
    assert_eq!(store["narrative_facts"]["f-old"]["canon_to"], "ch-2");
    assert!(store["frames"]["ground-truth"].is_object());
    // Idempotent re-run: no-op.
    let again = run(tmp.path(), &["import-facts", "--manifest", &manifest]);
    assert!(again.status.success());
    let stdout = String::from_utf8_lossy(&again.stdout);
    assert!(
        stdout.contains(
            "0 frames + 0 branches + 0 entities + 0 predicates + 0 facts created, 4 no-op"
        ),
        "{stdout}"
    );
}

#[test]
fn import_facts_rejects_unknown_frame_and_leaves_store_untouched() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    let before = fs::read_to_string(tmp.path().join("docs/.atomic/workspace.atomic.json")).unwrap();
    let manifest = write_manifest(
        tmp.path(),
        serde_json::json!({
            "facts": [{
                "fact_id": "f1", "frame": "nobody",
                "claim": "x", "canon_from": "ch-1", "evidence": ["ch-1"]
            }]
        }),
    );
    let out = run(tmp.path(), &["import-facts", "--manifest", &manifest]);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("frames registry"), "{stderr}");
    let after = fs::read_to_string(tmp.path().join("docs/.atomic/workspace.atomic.json")).unwrap();
    assert_eq!(before, after, "failed import must not touch the store");
}

#[test]
fn add_frame_add_fact_add_conflict_end_to_end() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    let ok = run(
        tmp.path(),
        &[
            "add-frame",
            "--frame",
            "seward",
            "--description",
            "Dr Seward",
        ],
    );
    assert!(ok.status.success(), "{:?}", ok);
    let ok = run(tmp.path(), &["add-frame", "--frame", "van-helsing"]);
    assert!(ok.status.success());
    let ok = run(
        tmp.path(),
        &[
            "add-fact",
            "--fact",
            "f-illness",
            "--frame",
            "seward",
            "--claim",
            "Lucy suffers from an unexplained illness",
            "--canon-from",
            "ch-1",
            "--canon-to",
            "ch-2",
            "--evidence",
            "ch-1,ch-2",
        ],
    );
    assert!(ok.status.success(), "{:?}", ok);
    let ok = run(
        tmp.path(),
        &[
            "add-fact",
            "--fact",
            "f-vampire",
            "--frame",
            "van-helsing",
            "--claim",
            "Lucy is preyed upon by a vampire",
            "--canon-from",
            "ch-2",
            "--evidence",
            "ch-2",
        ],
    );
    assert!(ok.status.success(), "{:?}", ok);
    let ok = run(
        tmp.path(),
        &[
            "add-fact-conflict",
            "--fact",
            "f-illness",
            "--conflicts-with",
            "f-vampire",
        ],
    );
    assert!(ok.status.success(), "{:?}", ok);
    let store = read_store(tmp.path());
    assert_eq!(
        store["narrative_facts"]["f-illness"]["conflicts_with"][0]["target"],
        "f-vampire"
    );
    // Judgment-time claim pin stamped by the primitive (R439).
    assert_eq!(
        store["narrative_facts"]["f-illness"]["conflicts_with"][0]["target_claim_sha256"]
            .as_str()
            .unwrap()
            .len(),
        64
    );
    // Duplicate edge (either direction) rejects as already-recorded.
    let dup = run(
        tmp.path(),
        &[
            "add-fact-conflict",
            "--fact",
            "f-vampire",
            "--conflicts-with",
            "f-illness",
        ],
    );
    assert!(!dup.status.success());
    let stderr = String::from_utf8_lossy(&dup.stderr);
    assert!(stderr.contains("already recorded"), "{stderr}");
}

#[test]
fn add_fact_rejects_cross_frame_succession() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    for frame in ["a", "b"] {
        assert!(run(tmp.path(), &["add-frame", "--frame", frame])
            .status
            .success());
    }
    assert!(run(
        tmp.path(),
        &[
            "add-fact",
            "--fact",
            "f1",
            "--frame",
            "a",
            "--claim",
            "x",
            "--canon-from",
            "ch-1",
            "--evidence",
            "ch-1",
        ],
    )
    .status
    .success());
    let out = run(
        tmp.path(),
        &[
            "add-fact",
            "--fact",
            "f2",
            "--frame",
            "b",
            "--claim",
            "y",
            "--canon-from",
            "ch-2",
            "--evidence",
            "ch-2",
            "--supersedes",
            "f1",
        ],
    );
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("in-frame succession only"), "{stderr}");
}

/// Round 446 — typed-claim verbs end-to-end: `add-predicate` (4th
/// registry, object_kind fail-loud) and the typed flags on `add-fact`
/// (all-or-nothing leg, registry + shape enforcement via the shared
/// builder).
#[test]
fn add_predicate_and_typed_fact_end_to_end() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    assert!(run(tmp.path(), &["add-frame", "--frame", "gt"])
        .status
        .success());
    assert!(run(tmp.path(), &["add-entity", "--entity", "kara"])
        .status
        .success());
    // Unknown object_kind rejects (no silent default).
    let bad = run(
        tmp.path(),
        &[
            "add-predicate",
            "--predicate",
            "alive",
            "--object-kind",
            "boolean",
        ],
    );
    assert!(!bad.status.success());
    assert!(String::from_utf8_lossy(&bad.stderr).contains("unknown object_kind"));
    assert!(run(
        tmp.path(),
        &[
            "add-predicate",
            "--predicate",
            "alive",
            "--object-kind",
            "scalar",
            "--description",
            "life state",
        ],
    )
    .status
    .success());
    // Incomplete typed leg rejects at the arg surface.
    let partial = run(
        tmp.path(),
        &[
            "add-fact",
            "--fact",
            "f1",
            "--frame",
            "gt",
            "--claim",
            "Kara is operational",
            "--canon-from",
            "ch-1",
            "--evidence",
            "ch-1",
            "--entities",
            "kara",
            "--typed-subject",
            "kara",
        ],
    );
    assert!(!partial.status.success());
    assert!(String::from_utf8_lossy(&partial.stderr).contains("all-or-nothing"));
    // Full typed leg lands and round-trips.
    let ok = run(
        tmp.path(),
        &[
            "add-fact",
            "--fact",
            "f1",
            "--frame",
            "gt",
            "--claim",
            "Kara is operational",
            "--canon-from",
            "ch-1",
            "--evidence",
            "ch-1",
            "--entities",
            "kara",
            "--typed-subject",
            "kara",
            "--typed-predicate",
            "alive",
            "--typed-object-value",
            "operational",
        ],
    );
    assert!(ok.status.success(), "{:?}", ok);
    let store = read_store(tmp.path());
    assert_eq!(store["narrative_facts"]["f1"]["typed"]["subject"], "kara");
    assert_eq!(
        store["narrative_facts"]["f1"]["typed"]["object"]["kind"],
        "value"
    );
    assert_eq!(store["predicates"]["alive"]["object_kind"], "scalar");
}
