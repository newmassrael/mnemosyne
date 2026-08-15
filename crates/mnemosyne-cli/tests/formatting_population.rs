//! The command that FIXES formatting covers the population the command that
//! CHECKS it reads.
//!
//! `cargo fmt --all` formats the ROOT workspace, and this repository has twenty:
//! the other nineteen carry their own `[workspace]` so the root's commands never
//! reach them. Until R1206 the fixing side had no command for those at all — a
//! person read the gate's rejection and ran one `cargo fmt --manifest-path` per
//! named manifest, out of a population only the gate knew. Two populations, one
//! of them assembled by hand each time, is the shape this repository has paid
//! for repeatedly: the reader and the writer of one datum drift, and the drift
//! is silent because each half is internally consistent.
//!
//! SO `scripts/fmt.sh` DERIVES ITS POPULATION FROM THE GATE rather than
//! restating it, and this file is what makes that claim checkable instead of
//! merely intended. It holds the two `--list` outputs against each other: a
//! manifest the gate checks and the formatter does not write is a file the
//! repository can reject and cannot fix, and a manifest the formatter writes and
//! the gate does not check is a file nothing is watching.
//!
//! IT RUNS THE PROGRAMS. Reading either script's source for a list of
//! workspaces would be a third spelling of the walk, and the reason there is one
//! walk is that a second one disagrees.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR = <root>/crates/mnemosyne-cli
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the repository root is two levels above this crate's manifest")
        .to_path_buf()
}

/// What one script declares, run from the repository root as both are meant to
/// be. Both streams, because a declaration a script writes to stdout and a
/// refusal it writes to stderr are equally what it said.
fn declared(script: &str) -> String {
    let root = repo_root();
    let out = Command::new(root.join(script))
        .arg("--list")
        .current_dir(&root)
        .output()
        .unwrap_or_else(|why| panic!("{script} runs: {why}"));
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.status.success(),
        "{script} --list must answer, not refuse:\n{said}"
    );
    said
}

/// The manifest a declaration line names, by the token after `--manifest-path`.
fn manifest_of(fields: &[&str]) -> Option<String> {
    let at = fields.iter().position(|word| *word == "--manifest-path")?;
    fields.get(at + 1).map(|path| (*path).to_string())
}

#[test]
fn the_formatter_writes_every_manifest_the_gate_checks() {
    let gate = declared("scripts/check-side-workspaces.sh");
    let fixer = declared("scripts/fmt.sh");

    let mut checked = BTreeSet::new();
    for line in gate.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.first() != Some(&"[side-workspaces]")
            || fields.get(1) != Some(&"COMMAND")
            || fields.get(3) != Some(&"fmt")
        {
            continue;
        }
        // A CHECK THAT IS NOT CHECKING would make the comparison below
        // meaningless in the one direction that matters: the gate would be
        // running a formatter and reading its exit code as a verdict.
        assert!(
            fields.contains(&"--check"),
            "the gate's formatting command must CHECK: {line}"
        );
        checked.insert(manifest_of(&fields).unwrap_or_else(|| {
            panic!("the gate's formatting command must name a manifest: {line}")
        }));
    }

    let mut written = BTreeSet::new();
    let mut writes_the_root = false;
    for line in fixer.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.first() != Some(&"[fmt]") || fields.get(1) != Some(&"COMMAND") {
            continue;
        }
        assert!(
            !fields.contains(&"--check"),
            "the formatter must WRITE, and a command carrying --check writes \
             nothing while reporting exactly like one that did: {line}"
        );
        match manifest_of(&fields) {
            Some(manifest) => {
                written.insert(manifest);
            }
            // `cargo fmt --all` names no manifest because it asks cargo which
            // packages the root workspace has — the one question this must not
            // answer for itself.
            None => writes_the_root |= fields.contains(&"--all"),
        }
    }

    // NON-VACUITY FIRST. Two empty sets are equal, and this repository's own
    // recorded failure mode is a law that holds over nothing reading exactly
    // like one that holds.
    assert!(
        checked.len() >= 20,
        "the gate checks {} manifest(s), which is too few to be this \
         repository's — the comparison below would be about nothing",
        checked.len()
    );
    assert!(
        writes_the_root,
        "the formatter must also format the ROOT workspace, which is the half \
         `cargo fmt --all` covers and the gate deliberately does not:\n{fixer}"
    );

    let unfixable: Vec<&String> = checked.difference(&written).collect();
    assert!(
        unfixable.is_empty(),
        "{} manifest(s) the gate can REJECT and the formatter does not write, \
         so the repository can refuse a file it has no command to fix:\n  {}",
        unfixable.len(),
        unfixable
            .iter()
            .map(|m| m.as_str())
            .collect::<Vec<_>>()
            .join("\n  ")
    );
    let unwatched: Vec<&String> = written.difference(&checked).collect();
    assert!(
        unwatched.is_empty(),
        "{} manifest(s) the formatter writes and the gate never checks, so \
         nothing would notice if the formatter stopped reaching them:\n  {}",
        unwatched.len(),
        unwatched
            .iter()
            .map(|m| m.as_str())
            .collect::<Vec<_>>()
            .join("\n  ")
    );
}
