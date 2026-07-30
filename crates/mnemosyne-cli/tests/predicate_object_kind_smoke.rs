//! `--object-kind` at the argv boundary — the one surface a bad tag survives to.
//!
//! Round 873 typed `object_kind` end to end: the primitives take
//! `PredicateObjectKind`, and the manifest and MCP wires deserialize into it. A
//! tag that is not a shape therefore cannot reach `add_predicate` any more — it
//! is unrepresentable rather than rejected, which is the better state ONLY if the
//! rejection is still asserted where a bad tag can actually arrive.
//!
//! For a human or an agent at a shell, that place is argv. So this file holds the
//! half of `predicate_registry_and_typed_roundtrip`'s tag-parse coverage that
//! moved out of `mnemosyne-atomic`: the reject, its DERIVED vocabulary, the trim
//! argv gets and JSON does not, and the parity between the two verbs that read
//! the flag.

use std::fs;
use std::path::Path;
use std::process::Command;

use tempfile::TempDir;

fn cli_binary() -> &'static str {
    env!("CARGO_BIN_EXE_mnemosyne-cli")
}

fn workspace() -> TempDir {
    let tmp = TempDir::new().expect("tempdir");
    fs::create_dir_all(tmp.path().join("docs/.atomic")).expect("mkdir");
    fs::write(tmp.path().join("mnemosyne.toml"), "[workspace]\n").expect("config");
    fs::write(
        tmp.path().join("docs/.atomic/workspace.atomic.json"),
        r#"{"schema_version":23,"sections":{},"changelog_entries":{}}"#,
    )
    .expect("seed");
    tmp
}

fn run(ws: &Path, args: &[&str]) -> std::process::Output {
    Command::new(cli_binary())
        .args(args)
        .current_dir(ws)
        .output()
        .expect("cli exec")
}

/// Both verbs that read `--object-kind`, so a fix to one cannot drift from the
/// other — the flag is one datum with two readers.
const VERBS: [&str; 2] = ["add-predicate", "set-predicate"];

#[test]
fn a_tag_that_is_not_a_shape_is_refused_with_the_derived_vocabulary() {
    let tmp = workspace();
    for verb in VERBS {
        let out = run(
            tmp.path(),
            &[verb, "--predicate", "alive", "--object-kind", "boolean"],
        );
        assert!(!out.status.success(), "{verb} accepted `boolean`");
        let err = String::from_utf8_lossy(&out.stderr);
        for tag in ["entity", "token", "quantity", "fact"] {
            assert!(
                err.contains(tag),
                "{verb}: the message must name `{tag}`:\n{err}"
            );
        }
        // The vocabulary is DERIVED from the enum, so the shape Round 708 removed
        // must not appear — `build_predicate`'s hand-written "expected one of"
        // still offered `scalar` until this round.
        assert!(
            !err.contains("scalar"),
            "{verb}: a removed shape is still advertised:\n{err}"
        );
        assert!(err.contains("boolean"), "{verb}: got:\n{err}");
    }
}

#[test]
fn the_flag_is_required_and_says_so_rather_than_defaulting() {
    let tmp = workspace();
    for verb in VERBS {
        let out = run(tmp.path(), &[verb, "--predicate", "alive"]);
        assert!(!out.status.success(), "{verb} defaulted a missing shape");
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(
            err.contains("--object-kind"),
            "{verb}: got:\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[test]
fn argv_trims_a_padded_tag_and_still_refuses_a_miscased_one() {
    let tmp = workspace();
    // A shell hands over padding; a padded legal tag is the same declaration.
    let out = run(
        tmp.path(),
        &[
            "add-predicate",
            "--predicate",
            "near",
            "--object-kind",
            "  entity  ",
        ],
    );
    assert!(
        out.status.success(),
        "argv must trim: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    // Case is NOT normalised: the tag is a wire token, and `Entity` is a typo
    // rather than a synonym. Asserting this keeps the trim from widening.
    let out = run(
        tmp.path(),
        &[
            "add-predicate",
            "--predicate",
            "touches",
            "--object-kind",
            "Entity",
        ],
    );
    assert!(!out.status.success(), "a miscased tag must not be accepted");
}
