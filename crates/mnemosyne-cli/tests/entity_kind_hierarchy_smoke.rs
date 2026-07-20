//! Round 732 (DEBT-M) / Round 738 (DAG) — the entity-kind inheritance graph
//! through the CLI.
//!
//! The durable proof (not an ephemeral dogfood — the R689/R699 discipline) that
//! `add-entity-kind --parent` is wired end-to-end through the REAL binary AND
//! that the ancestor-closure scope closes the R732-measured expressiveness gap:
//! "a weapon is a kind of thing" — a `thing`-scoped predicate ACCEPTS a `weapon`
//! (which a flat registry rejected), a `weapon`-scoped predicate still REJECTS a
//! bare `thing` (directional), and a self / unregistered parent is rejected at
//! write time. R738 adds the MULTIPLE-INHERITANCE proof: a `magic-sword`
//! declared with a REPEATED `--parent` (`weapon` AND `magic`) satisfies BOTH a
//! weapon-scoped and a magic-scoped rule at once — the capability a single-parent
//! tree could not express. Both parents are visible in the raw SSOT JSON.

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

fn read_store(ws: &Path) -> serde_json::Value {
    serde_json::from_str(
        &fs::read_to_string(ws.join("docs/.atomic/workspace.atomic.json")).unwrap(),
    )
    .unwrap()
}

fn write_workspace(workspace: &Path) {
    fs::create_dir_all(workspace.join("docs/.atomic")).unwrap();
    fs::write(workspace.join("mnemosyne.toml"), "[workspace]\n").unwrap();
    let atomic = serde_json::json!({
        "schema_version": 15,
        "sections": { "ch-1": {} },
        "changelog_entries": {},
        "frames": { "gt": {} }
    });
    fs::write(
        workspace.join("docs/.atomic/workspace.atomic.json"),
        serde_json::to_string_pretty(&atomic).unwrap(),
    )
    .unwrap();
}

#[test]
fn entity_kind_hierarchy_end_to_end() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    let ws = tmp.path();

    // The kind tree: thing ⊃ weapon (a weapon is a kind of thing); character root.
    assert!(run(ws, &["add-entity-kind", "--kind", "thing"])
        .status
        .success());
    let out = run(
        ws,
        &["add-entity-kind", "--kind", "weapon", "--parent", "thing"],
    );
    assert!(out.status.success(), "weapon --parent thing: {out:?}");
    assert!(run(ws, &["add-entity-kind", "--kind", "character"])
        .status
        .success());

    // Write-path guards: a self-parent and an unregistered parent reject.
    let out = run(
        ws,
        &["add-entity-kind", "--kind", "loop", "--parent", "loop"],
    );
    assert!(!out.status.success());
    assert!(stderr(&out).contains("its own parent"), "{}", stderr(&out));
    let out = run(
        ws,
        &["add-entity-kind", "--kind", "orphan", "--parent", "ghost"],
    );
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("not a registered"),
        "{}",
        stderr(&out)
    );

    // The raw SSOT JSON carries the parent link (a set — a one-element array).
    let store = read_store(ws);
    assert_eq!(
        store["entity_kinds"]["weapon"]["parents"],
        serde_json::json!(["thing"]),
        "parents must be stored: {store}"
    );

    // Entities + two predicates: holds scoped to `thing`, wields to `weapon`.
    for (id, kind) in [
        ("hero", "character"),
        ("sword", "weapon"),
        ("shield", "thing"),
    ] {
        assert!(run(ws, &["add-entity", "--entity", id, "--kind", kind])
            .status
            .success());
    }
    assert!(run(
        ws,
        &[
            "add-predicate",
            "--predicate",
            "holds",
            "--object-kind",
            "entity",
            "--subject-kind",
            "character",
            "--object-entity-kind",
            "thing",
            "--description",
            "custody",
        ],
    )
    .status
    .success());
    assert!(run(
        ws,
        &[
            "add-predicate",
            "--predicate",
            "wields",
            "--object-kind",
            "entity",
            "--subject-kind",
            "character",
            "--object-entity-kind",
            "weapon",
            "--description",
            "wielding",
        ],
    )
    .status
    .success());

    let add_fact = |fid: &str, pred: &str, obj: &str| -> std::process::Output {
        run(
            ws,
            &[
                "add-fact",
                "--fact",
                fid,
                "--frame",
                "gt",
                "--canon-from",
                "ch-1",
                "--evidence",
                "ch-1",
                "--entities",
                &format!("hero,{obj}"),
                "--typed-subject",
                "hero",
                "--typed-predicate",
                pred,
                "--typed-object-entity",
                obj,
                "--claim",
                "x",
            ],
        )
    };

    // THE CLOSED GAP: holds(hero, sword) — sword is a `weapon`, weapon ⊂ thing ⇒
    // ACCEPT. A flat registry rejected this exact fact (the R732 measurement).
    let out = add_fact("f-hold", "holds", "sword");
    assert!(
        out.status.success(),
        "holds(hero, sword) must accept via subtree: {out:?}"
    );
    // wields(hero, sword) — weapon == weapon ⇒ ACCEPT.
    assert!(add_fact("f-wield", "wields", "sword").status.success());
    // wields(hero, shield) — a bare `thing` is NOT a subkind of `weapon` ⇒
    // REJECT (the subtree is directional, not symmetric).
    let out = add_fact("f-bad", "wields", "shield");
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("object kind `weapon`"),
        "{}",
        stderr(&out)
    );

    // ── Round 738 (DAG / multiple inheritance) ─────────────────────────────
    // magic-sword is BOTH a weapon (⊂ thing) AND a magic-item — the capability
    // a single-parent tree could not express. Declared with a REPEATED --parent.
    assert!(run(ws, &["add-entity-kind", "--kind", "magic"])
        .status
        .success());
    let out = run(
        ws,
        &[
            "add-entity-kind",
            "--kind",
            "magic-sword",
            "--parent",
            "weapon",
            "--parent",
            "magic",
        ],
    );
    assert!(out.status.success(), "magic-sword two parents: {out:?}");
    // The raw SSOT JSON carries BOTH parents (a BTreeSet ⇒ a sorted array).
    let store = read_store(ws);
    assert_eq!(
        store["entity_kinds"]["magic-sword"]["parents"],
        serde_json::json!(["magic", "weapon"]),
        "both parents stored, sorted: {store}"
    );
    // A magic-scoped predicate + an entity of the doubly-parented kind.
    assert!(run(
        ws,
        &[
            "add-predicate",
            "--predicate",
            "attunes",
            "--object-kind",
            "entity",
            "--subject-kind",
            "character",
            "--object-entity-kind",
            "magic",
            "--description",
            "attunement",
        ],
    )
    .status
    .success());
    assert!(run(
        ws,
        &[
            "add-entity",
            "--entity",
            "excalibur",
            "--kind",
            "magic-sword"
        ]
    )
    .status
    .success());
    // THE MULTIPLE-INHERITANCE GAP: excalibur satisfies BOTH ancestor scopes.
    //  holds   scoped to thing  — magic-sword ⊂ weapon ⊂ thing ⇒ ACCEPT (parent 1).
    assert!(
        add_fact("f-mh", "holds", "excalibur").status.success(),
        "holds(hero, excalibur) via weapon⊂thing must accept"
    );
    //  wields  scoped to weapon — magic-sword ⊂ weapon ⇒ ACCEPT (parent 1).
    assert!(
        add_fact("f-mw", "wields", "excalibur").status.success(),
        "wields(hero, excalibur) via weapon must accept"
    );
    //  attunes scoped to magic  — magic-sword ⊂ magic ⇒ ACCEPT (parent 2, the DAG).
    assert!(
        add_fact("f-ma", "attunes", "excalibur").status.success(),
        "attunes(hero, excalibur) via the SECOND parent must accept"
    );
    // Directional still holds: a plain weapon (sword) is NOT a magic-item.
    let out = add_fact("f-mbad", "attunes", "sword");
    assert!(
        !out.status.success(),
        "attunes(hero, sword) must reject — sword is not ⊂ magic"
    );

    // The store validates clean end to end.
    let out = run(ws, &["validate-continuity"]);
    assert!(
        out.status.success(),
        "validate-continuity: {}",
        stderr(&out)
    );
}

/// Round 739 — the parent-mutation setter `set-entity-kind-parents` through the
/// REAL binary: it REPLACES an existing kind's super-kinds, and it REJECTS a
/// re-parent that would close a cycle (the guard add-entity-kind never needs).
#[test]
fn set_entity_kind_parents_through_the_cli() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    let ws = tmp.path();

    // thing ⊃ weapon ; magic is a second root.
    assert!(run(ws, &["add-entity-kind", "--kind", "thing"])
        .status
        .success());
    assert!(run(ws, &["add-entity-kind", "--kind", "magic"])
        .status
        .success());
    assert!(run(
        ws,
        &["add-entity-kind", "--kind", "weapon", "--parent", "thing"]
    )
    .status
    .success());

    // REPLACE: give weapon a SECOND parent (thing + magic) via the setter.
    let out = run(
        ws,
        &[
            "set-entity-kind-parents",
            "--kind",
            "weapon",
            "--parent",
            "thing",
            "--parent",
            "magic",
        ],
    );
    assert!(out.status.success(), "re-parent weapon: {out:?}");
    assert_eq!(
        read_store(ws)["entity_kinds"]["weapon"]["parents"],
        serde_json::json!(["magic", "thing"]),
        "the setter replaced the parents set (sorted)"
    );

    // THE R739 GUARD through the binary: making `thing` a child of `weapon`
    // closes thing↔weapon — rejected, and the store is untouched.
    let out = run(
        ws,
        &[
            "set-entity-kind-parents",
            "--kind",
            "thing",
            "--parent",
            "weapon",
        ],
    );
    assert!(!out.status.success(), "cycle re-parent must reject");
    assert!(
        stderr(&out).contains("would create a cycle"),
        "{}",
        stderr(&out)
    );
    assert!(
        read_store(ws)["entity_kinds"]["thing"]
            .get("parents")
            .is_none(),
        "a rejected re-parent leaves thing a root"
    );

    // Empty roots the kind; the store stays clean.
    assert!(run(ws, &["set-entity-kind-parents", "--kind", "weapon"])
        .status
        .success());
    assert!(read_store(ws)["entity_kinds"]["weapon"]
        .get("parents")
        .is_none());
    assert!(run(ws, &["validate-workspace"]).status.success());
}

/// Round 740 — the remove peer `remove-entity-kind` through the REAL binary: it
/// REFUSES while a child kind still names it as a parent (no orphan), and
/// succeeds once the reference is gone; an absent kind rejects.
#[test]
fn remove_entity_kind_through_the_cli() {
    let tmp = TempDir::new().unwrap();
    write_workspace(tmp.path());
    let ws = tmp.path();

    assert!(run(ws, &["add-entity-kind", "--kind", "thing"])
        .status
        .success());
    assert!(run(
        ws,
        &["add-entity-kind", "--kind", "weapon", "--parent", "thing"]
    )
    .status
    .success());

    // Absent kind rejects.
    let out = run(ws, &["remove-entity-kind", "--kind", "ghost"]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("not present"), "{}", stderr(&out));

    // REFUSE: `thing` is still named as a parent by `weapon`.
    let out = run(ws, &["remove-entity-kind", "--kind", "thing"]);
    assert!(
        !out.status.success(),
        "remove of a referenced kind must refuse"
    );
    assert!(
        stderr(&out).contains("naming it as a parent"),
        "{}",
        stderr(&out)
    );
    assert!(
        read_store(ws)["entity_kinds"].get("thing").is_some(),
        "a refused remove leaves the kind in place"
    );

    // Remove the child first, then the parent — both succeed, store stays clean.
    assert!(run(ws, &["remove-entity-kind", "--kind", "weapon"])
        .status
        .success());
    assert!(run(ws, &["remove-entity-kind", "--kind", "thing"])
        .status
        .success());
    let store = read_store(ws);
    assert!(store["entity_kinds"].get("thing").is_none());
    assert!(store["entity_kinds"].get("weapon").is_none());
    assert!(run(ws, &["validate-workspace"]).status.success());
}
