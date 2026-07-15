//! Every dispatched command is named by `--help` — the discoverability gate.
//!
//! A consumer authoring a 26-chapter playable store reported that Mnemosyne
//! "had no place to record world state / no way to project the player's turn /
//! no open predicate set". All of it already existed. The measured cause was
//! not capability: the CLI's bare-invocation usage string was a SECOND,
//! hand-maintained command list that had drifted to omit every
//! narrative/playable verb — `validate-continuity` (the narrative gate itself),
//! `describe-schema`, `report-playable-world`, `report-quest-graph`,
//! `report-typing-candidates` — while still naming 52 commands, so it read as
//! exhaustive rather than partial. The reader was taught the surface did not
//! exist.
//!
//! That second list is gone (a bare invocation now prints `print_help`), which
//! makes THAT drift structurally impossible. This gate covers the remaining
//! pair: `print_help` is still hand-maintained beside the dispatch `match`, so
//! a new verb can still land dispatched-but-undocumented. Half-enforced
//! discoverability is no discoverability (CLAUDE.md) — a verb the help never
//! names is a verb the next consumer rebuilds in Python.

use std::collections::HashSet;
use std::process::Command;

/// The dispatch `match` in `run()`, verbatim from source at compile time — the
/// SSOT this gate reads. Parsing the source (rather than restating the verb
/// list here) is the point: a restated list would be a third hand-maintained
/// copy, i.e. the very defect under gate.
const MAIN_RS: &str = include_str!("../src/main.rs");

/// Command names the help is not expected to enumerate: the meta-flags, which
/// `print_help` documents as a group rather than per-verb.
const META: &[&str] = &["--help", "-h", "help", "--version", "-V", "version"];

/// Extract every command literal from `run()`'s dispatch `match`, bounded to
/// that match so unrelated `match` arms elsewhere in the file cannot leak in.
fn dispatched_commands() -> Vec<String> {
    let start = MAIN_RS
        .find("match cmd.as_str() {")
        .expect("run() dispatch match must exist");
    let rest = &MAIN_RS[start..];
    let end = rest
        .find("other => bail!")
        .expect("dispatch must end with the unknown-command arm");
    let block = &rest[..end];

    let mut cmds = Vec::new();
    for line in block.lines() {
        let trimmed = line.trim_start();
        // A dispatch arm opens with its string literal; `=>` on the same line.
        if !trimmed.starts_with('"') || !trimmed.contains("=>") {
            continue;
        }
        let head = trimmed.split("=>").next().unwrap_or("");
        for lit in head.split('|') {
            let name = lit.trim().trim_matches('"').trim();
            if name.is_empty() || META.contains(&name) {
                continue;
            }
            cmds.push(name.to_string());
        }
    }
    cmds
}

#[test]
fn help_names_every_dispatched_command() {
    let cmds = dispatched_commands();
    assert!(
        cmds.len() > 40,
        "parser regression: only {} dispatch arms found — the gate would pass \
         vacuously. Found: {:?}",
        cmds.len(),
        cmds
    );

    let out = Command::new(env!("CARGO_BIN_EXE_mnemosyne-cli"))
        .arg("--help")
        .output()
        .expect("run mnemosyne-cli --help");
    assert!(out.status.success(), "--help must exit 0");
    let help = String::from_utf8(out.stdout).expect("--help stdout is utf-8");

    // Token-exact, never substring: `add-fact` must not be satisfied by
    // `add-fact-conflict` appearing in the help.
    let tokens: HashSet<&str> = help.split_whitespace().collect();

    let missing: Vec<&String> = cmds
        .iter()
        .filter(|c| !tokens.contains(c.as_str()))
        .collect();
    assert!(
        missing.is_empty(),
        "these commands dispatch but `--help` never names them — they are \
         invisible to any consumer who does not read the source:\n  {:?}",
        missing
    );
}

#[test]
fn bare_invocation_prints_the_command_list() {
    // The discovery act. It must answer with the real surface, not a curated
    // subset: the narrative verbs below are exactly the ones the drifted usage
    // string omitted, and their absence is what sent a consumer to Python.
    let out = Command::new(env!("CARGO_BIN_EXE_mnemosyne-cli"))
        .output()
        .expect("run mnemosyne-cli with no args");
    let stdout = String::from_utf8(out.stdout).expect("bare stdout is utf-8");

    for verb in [
        "validate-continuity",
        "describe-schema",
        "report-playable-world",
        "report-quest-graph",
        "report-typing-candidates",
    ] {
        assert!(
            stdout.split_whitespace().any(|t| t == verb),
            "a bare invocation must name `{}`; a consumer typing the program \
             name is discovering it, and this list is the whole answer",
            verb
        );
    }
}
