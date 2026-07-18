//! Round 701 — the predicate endpoint-KIND write-path gate, through the CLI.
//!
//! The durable proof (not an ephemeral dogfood — the R689/R699 discipline)
//! that the arg surface plus the real write path enforce the spatial-map G1: a
//! kind-constrained `adjacent` predicate accepts a place↔place fact and rejects
//! an off-kind endpoint at write time; the declaration guards fire on the
//! predicate creators; and a `set-predicate` tighten over an off-kind use
//! rejects, naming the offender.

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

fn write_workspace(workspace: &Path) {
    fs::create_dir_all(workspace.join("docs/.atomic")).unwrap();
    fs::write(workspace.join("mnemosyne.toml"), "[workspace]\n").unwrap();
    let atomic = serde_json::json!({
        "schema_version": 15,
        "sections": { "ch-1": {}, "ch-2": {} },
        "changelog_entries": {},
        "frames": { "cartography": {} }
    });
    fs::write(
        workspace.join("docs/.atomic/workspace.atomic.json"),
        serde_json::to_string_pretty(&atomic).unwrap(),
    )
    .unwrap();
}

#[test]
fn predicate_endpoint_kind_gate_end_to_end() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    let ws = tmp.path();

    // Vocabulary: two kinds, two places and one thing.
    for (kind, desc) in [("place", "a location"), ("thing", "an object")] {
        let out = run(
            ws,
            &["add-entity-kind", "--kind", kind, "--description", desc],
        );
        assert!(out.status.success(), "add-entity-kind {kind}: {out:?}");
    }
    for (id, kind) in [("cove", "place"), ("dike", "place"), ("stake", "thing")] {
        let out = run(
            ws,
            &[
                "add-entity",
                "--entity",
                id,
                "--kind",
                kind,
                "--description",
                "",
            ],
        );
        assert!(out.status.success(), "add-entity {id}: {out:?}");
    }

    // Declaration guard: a scalar object cannot carry an object-entity-kind.
    let out = run(
        ws,
        &[
            "add-predicate",
            "--predicate",
            "bad",
            "--object-kind",
            "scalar",
            "--object-entity-kind",
            "place",
            "--description",
            "x",
        ],
    );
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("scalar cannot carry"),
        "{}",
        stderr(&out)
    );

    // Declaration guard: an unregistered kind ref fails loud.
    let out = run(
        ws,
        &[
            "add-predicate",
            "--predicate",
            "bad2",
            "--object-kind",
            "entity",
            "--subject-kind",
            "nope",
            "--description",
            "x",
        ],
    );
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("not a registered entity_kind"),
        "{}",
        stderr(&out)
    );

    // The place↔place map predicate lands.
    let out = run(
        ws,
        &[
            "add-predicate",
            "--predicate",
            "adjacent",
            "--object-kind",
            "entity",
            "--subject-kind",
            "place",
            "--object-entity-kind",
            "place",
            "--description",
            "map edge",
        ],
    );
    assert!(out.status.success(), "{out:?}");

    // place ↔ place fact: accepted at write time.
    let out = run(
        ws,
        &[
            "add-fact",
            "--fact",
            "edge-1",
            "--frame",
            "cartography",
            "--claim",
            "cove borders dike",
            "--canon-from",
            "ch-1",
            "--evidence",
            "ch-1",
            "--entities",
            "cove,dike",
            "--typed-subject",
            "cove",
            "--typed-predicate",
            "adjacent",
            "--typed-object-entity",
            "dike",
        ],
    );
    assert!(out.status.success(), "place-place should land: {out:?}");

    // place → thing (off-kind OBJECT): rejected at write time.
    let out = run(
        ws,
        &[
            "add-fact",
            "--fact",
            "edge-2",
            "--frame",
            "cartography",
            "--claim",
            "cove borders stake",
            "--canon-from",
            "ch-1",
            "--evidence",
            "ch-1",
            "--entities",
            "cove,stake",
            "--typed-subject",
            "cove",
            "--typed-predicate",
            "adjacent",
            "--typed-object-entity",
            "stake",
        ],
    );
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("object kind `place`"),
        "{}",
        stderr(&out)
    );

    // thing → place (off-kind SUBJECT): rejected at write time.
    let out = run(
        ws,
        &[
            "add-fact",
            "--fact",
            "edge-3",
            "--frame",
            "cartography",
            "--claim",
            "stake borders dike",
            "--canon-from",
            "ch-1",
            "--evidence",
            "ch-1",
            "--entities",
            "stake,dike",
            "--typed-subject",
            "stake",
            "--typed-predicate",
            "adjacent",
            "--typed-object-entity",
            "dike",
        ],
    );
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("subject kind `place`"),
        "{}",
        stderr(&out)
    );

    // set-predicate tighten over an off-kind use rejects, naming the offender.
    let out = run(
        ws,
        &[
            "add-predicate",
            "--predicate",
            "near",
            "--object-kind",
            "entity",
            "--description",
            "loose",
        ],
    );
    assert!(out.status.success(), "{out:?}");
    let out = run(
        ws,
        &[
            "add-fact",
            "--fact",
            "near-1",
            "--frame",
            "cartography",
            "--claim",
            "cove near stake",
            "--canon-from",
            "ch-1",
            "--evidence",
            "ch-1",
            "--entities",
            "cove,stake",
            "--typed-subject",
            "cove",
            "--typed-predicate",
            "near",
            "--typed-object-entity",
            "stake",
        ],
    );
    assert!(out.status.success(), "{out:?}");
    let out = run(
        ws,
        &[
            "set-predicate",
            "--predicate",
            "near",
            "--object-kind",
            "entity",
            "--object-entity-kind",
            "place",
            "--description",
            "tight",
        ],
    );
    assert!(!out.status.success());
    let e = stderr(&out);
    assert!(e.contains("do not satisfy") && e.contains("near-1"), "{e}");
}
