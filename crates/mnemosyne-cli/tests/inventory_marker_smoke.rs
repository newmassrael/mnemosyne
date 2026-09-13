//! An annotation inside a document cites an inventory entry by the id after its
//! marker — through the real `validate-code-refs`.
//!
//! The adopter's measurement this closes: with the annotation's prefix
//! registered as a path prefix, the prefix stayed in the id, so all three
//! citations came back missing — the active one included — and the deprecated
//! one was never reported as deprecated. The same workspace under
//! `inventory_marker_prefixes` must resolve the active entry, report the
//! deprecated one and the missing one, and nothing else.

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

/// A workspace whose document annotates three states with `req="…"`: one
/// active entry, one deprecated entry, and one id nothing registers.
fn workspace(axis_key: &str) -> TempDir {
    let ws = TempDir::new().expect("tempdir");
    fs::write(
        ws.path().join("mnemosyne.toml"),
        format!(
            "[workspace]\n\n[plugins.set_equality_validator]\npaths = [\"doc/\"]\n\
             {axis_key} = [\"req=\\\"\"]\nseverity_inventory = \"reject\"\n"
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
    fs::write(
        ws.path().join("doc/model.scxml"),
        "<scxml>\n  <state id=\"idle\" req=\"REQ-4.2.1\"/>\n  \
         <state id=\"old\" req=\"REQ-7\"/>\n  <state id=\"new\" req=\"REQ-999\"/>\n</scxml>\n",
    )
    .expect("document");
    ws
}

#[test]
fn a_marker_prefix_cites_the_id_after_it() {
    let ws = workspace("inventory_marker_prefixes");
    let out = run(ws.path(), &["validate-code-refs", "--json"]);
    let said = String::from_utf8_lossy(&out.stdout);
    assert!(
        !out.status.success(),
        "a deprecated and a missing citation at reject severity must fail the gate: {said}"
    );
    assert!(
        said.contains("REQ-7") && said.contains("REQ-999"),
        "the deprecated and the missing citation are both reported: {said}"
    );
    assert!(
        !said.contains("REQ-4.2.1"),
        "the active entry resolved through its marker is not a violation: {said}"
    );
}

/// CONTROL: the same annotation under the path axis keeps its syntax in the id,
/// so even the active entry is reported — the shape the marker axis exists for.
#[test]
fn the_same_annotation_under_the_path_axis_misses_the_active_entry() {
    let ws = workspace("inventory_path_prefixes");
    let out = run(ws.path(), &["validate-code-refs", "--json"]);
    let said = String::from_utf8_lossy(&out.stdout);
    assert!(
        said.contains("REQ-4.2.1"),
        "control: the path axis should report the active entry, prefix and all: {said}"
    );
}
