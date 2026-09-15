//! An inventory entry says how its requirement is stated and where it is
//! satisfied — through the real binary — and the two writers of each axis refuse
//! the same inputs.
//!
//! The adopter's need, in its own terms: a requirement from a standard is stated
//! as `shall`, `shall not` or `shall within 2000 ms`, and is satisfied here, in
//! another document, in a deployment, or not by this toolchain at all. Two
//! verbs write each axis — `add-inventory-entry` at registration and a
//! `set-inventory-*` peer afterwards — and a field whose writers disagree about
//! what they accept has no invariant, so each refusal below is asked of both.

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

fn accepted(workspace: &Path, args: &[&str]) -> String {
    let out = run(workspace, args);
    assert!(
        out.status.success(),
        "`{}` was refused: {}{}",
        args.join(" "),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn refused(workspace: &Path, args: &[&str]) -> String {
    let out = run(workspace, args);
    assert!(
        !out.status.success(),
        "`{}` was accepted: {}",
        args.join(" "),
        String::from_utf8_lossy(&out.stdout)
    );
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn entry(workspace: &Path, id: &str) -> serde_json::Value {
    serde_json::from_str(&accepted(
        workspace,
        &["query", "--inventory", id, "--json"],
    ))
    .expect("the inventory view is JSON")
}

/// A workspace with the `ms` unit registered, so a bound has something to name.
fn workspace() -> TempDir {
    let ws = TempDir::new().expect("tempdir");
    fs::write(ws.path().join("mnemosyne.toml"), "[workspace]\n").expect("config");
    accepted(ws.path(), &["add-unit", "--unit", "ms"]);
    ws
}

#[test]
fn a_requirement_says_how_it_is_stated_and_where_it_is_satisfied() {
    let ws = workspace();
    accepted(
        ws.path(),
        &[
            "add-inventory-entry",
            "--id",
            "REQ-4.2.1",
            "--status",
            "active",
            "--modality",
            "shall",
            "--within-n",
            "2000",
            "--within-unit",
            "ms",
            "--disposition",
            "out_of_scope",
            "--disposition-reason",
            "a property of the deployment",
        ],
    );
    let view = entry(ws.path(), "REQ-4.2.1");
    assert_eq!(
        view["modality"],
        serde_json::json!({"form": "shall", "within": {"n": 2000, "unit": "ms"}})
    );
    assert_eq!(
        view["disposition"],
        serde_json::json!({"kind": "out_of_scope", "reason": "a property of the deployment"})
    );
    assert_eq!(
        view["status"], "active",
        "scoping a requirement out must not deprecate it"
    );

    accepted(
        ws.path(),
        &[
            "set-inventory-modality",
            "--id",
            "REQ-4.2.1",
            "--modality",
            "shall_not",
        ],
    );
    accepted(
        ws.path(),
        &[
            "set-inventory-disposition",
            "--id",
            "REQ-4.2.1",
            "--disposition",
            "delegated",
            "--to-doc",
            "transport-spec",
            "--to-id",
            "REQ-7",
        ],
    );
    let view = entry(ws.path(), "REQ-4.2.1");
    assert_eq!(view["modality"], serde_json::json!({"form": "shall_not"}));
    assert_eq!(
        view["disposition"],
        serde_json::json!({"kind": "delegated", "to_doc": "transport-spec", "to_id": "REQ-7"})
    );

    accepted(
        ws.path(),
        &["set-inventory-modality", "--id", "REQ-4.2.1", "--clear"],
    );
    accepted(
        ws.path(),
        &["set-inventory-disposition", "--id", "REQ-4.2.1", "--clear"],
    );
    let view = entry(ws.path(), "REQ-4.2.1");
    assert!(
        view["modality"].is_null() && view["disposition"].is_null(),
        "`--clear` left a value behind: {view}"
    );
}

#[test]
fn both_writers_of_each_axis_refuse_the_same_inputs() {
    let ws = workspace();
    accepted(
        ws.path(),
        &["add-inventory-entry", "--id", "REQ-1", "--status", "active"],
    );

    let modality_cases: [(&str, &[&str], &str); 3] = [
        (
            "a bound in an unregistered unit",
            &[
                "--modality",
                "shall",
                "--within-n",
                "5",
                "--within-unit",
                "sec",
            ],
            "`sec`",
        ),
        (
            "a bound that is not positive",
            &[
                "--modality",
                "shall",
                "--within-n",
                "0",
                "--within-unit",
                "ms",
            ],
            "positive",
        ),
        (
            "a form outside the vocabulary",
            &["--modality", "must_maybe"],
            "must_maybe",
        ),
    ];
    for (label, flags, needle) in modality_cases {
        let mut add = vec!["add-inventory-entry", "--id", "REQ-2", "--status", "active"];
        add.extend_from_slice(flags);
        let at_add = refused(ws.path(), &add);
        let mut set = vec!["set-inventory-modality", "--id", "REQ-1"];
        set.extend_from_slice(flags);
        let at_set = refused(ws.path(), &set);
        assert!(
            at_add.contains(needle) && at_set.contains(needle),
            "{label}: both writers must refuse it by name — add said `{at_add}`, set said `{at_set}`"
        );
    }

    let disposition_cases: [(&str, &[&str], &str); 4] = [
        (
            "a payload the kind does not take",
            &["--disposition", "implemented", "--realised-by", "a gateway"],
            "realised_by",
        ),
        (
            "a missing payload",
            &["--disposition", "delegated", "--to-doc", "transport-spec"],
            "to_id",
        ),
        (
            "a blank payload",
            &[
                "--disposition",
                "out_of_scope",
                "--disposition-reason",
                "  ",
            ],
            "reason",
        ),
        (
            "a kind that does not exist",
            &["--disposition", "elsewhere"],
            "elsewhere",
        ),
    ];
    for (label, flags, needle) in disposition_cases {
        let mut add = vec!["add-inventory-entry", "--id", "REQ-2", "--status", "active"];
        add.extend_from_slice(flags);
        let at_add = refused(ws.path(), &add);
        let mut set = vec!["set-inventory-disposition", "--id", "REQ-1"];
        set.extend_from_slice(flags);
        let at_set = refused(ws.path(), &set);
        assert!(
            at_add.contains(needle) && at_set.contains(needle),
            "{label}: both writers must refuse it by name — add said `{at_add}`, set said `{at_set}`"
        );
    }

    // AND NO REFUSAL LEFT ANYTHING BEHIND: the refused registrations did not
    // register, and the refused updates did not touch the entry.
    refused(ws.path(), &["query", "--inventory", "REQ-2", "--json"]);
    let untouched = entry(ws.path(), "REQ-1");
    assert!(
        untouched["modality"].is_null() && untouched["disposition"].is_null(),
        "a refused update was written anyway: {untouched}"
    );
}

/// WHAT THE AXES HOLD, AND WHAT NOTHING HERE CHECKS (Round 1341).
///
/// The adopter who asked for these axes came back with the sharper half of the
/// request: *a disposition wants an arrival check, and "unchecked" must not read
/// as a pass.* `delegated { to_doc, to_id }` names a requirement in another
/// document, a store has no cross-workspace reference, and until this round the
/// limitation lived in a doc comment while the gate printed the same silence for
/// "nothing delegates" and "rows nobody resolved".
#[test]
fn the_report_counts_the_axes_and_names_what_it_does_not_check() {
    let ws = workspace();
    accepted(
        ws.path(),
        &[
            "add-inventory-entry",
            "--id",
            "REQ-1",
            "--status",
            "active",
            "--modality",
            "shall",
            "--disposition",
            "implemented",
        ],
    );
    accepted(
        ws.path(),
        &[
            "add-inventory-entry",
            "--id",
            "REQ-2",
            "--status",
            "active",
            "--disposition",
            "delegated",
            "--to-doc",
            "transport-spec",
            "--to-id",
            "REQ-7",
        ],
    );
    let said = accepted(ws.path(), &["validate-workspace"]);
    let axes = said
        .lines()
        .find(|l| l.starts_with("inventory axes:"))
        .expect("the report says nothing about the inventory axes");
    assert!(
        axes.contains("2 entry(ies)")
            && axes.contains("1 carry a modality")
            && axes.contains("2 a disposition")
            && axes.contains("delegated 1"),
        "the reach does not say what the axes hold: {axes}"
    );
    assert!(
        said.contains("1 delegated row(s), 0 checked by anything here")
            && said.contains("RECORDED, NOT VERIFIED"),
        "a delegated row nothing resolves is reported as though it were fine: {said}"
    );
    assert!(
        said.contains("unchecked: REQ-2 -> transport-spec#REQ-7"),
        "the row nothing verifies is counted but cannot be chased: {said}"
    );
}

/// AND A WORKSPACE THAT DELEGATES NOTHING SAYS SO, rather than printing the
/// silence an unchecked delegation would print. The two states have to be
/// different sentences, which is the whole point of reporting a reach — and at
/// zero population it is the only thing this round can honestly say.
#[test]
fn a_workspace_that_delegates_nothing_says_that_rather_than_nothing() {
    let ws = workspace();
    accepted(
        ws.path(),
        &["add-inventory-entry", "--id", "REQ-1", "--status", "active"],
    );
    let said = accepted(ws.path(), &["validate-workspace"]);
    assert!(
        said.contains("no row delegates, so there is nothing to check yet"),
        "an empty axis prints the same as an unchecked one: {said}"
    );
    assert!(
        !said.contains("unchecked:"),
        "a workspace with no delegation reported one: {said}"
    );
}
