//! The command registry's observable contract: discovery answers with the
//! whole surface, and an unknown verb still fails loud.
//!
//! A consumer authoring a 26-chapter playable store reported that Mnemosyne
//! "had no place to record world state / no way to project the player's turn /
//! no open predicate set". All of it already existed. The measured cause was
//! not capability: the CLI carried a SECOND, hand-maintained command list that
//! had drifted to omit every narrative/playable verb — `validate-continuity`
//! (the narrative gate itself), `describe-schema`, `report-playable-world`,
//! `report-quest-graph`, `report-typing-candidates` — while still naming 52
//! commands, so it read as exhaustive rather than partial. The reader was
//! taught the surface did not exist.
//!
//! The gate that used to live here parsed `main.rs` to *detect* drift between
//! the dispatch `match` and `print_help`. Both now derive from one `COMMANDS`
//! table, so that drift is unrepresentable rather than merely detected — a
//! dispatched verb is a documented verb by construction, and the source
//! parser (with its `cmds.len() > 40` vacuity guard and its hard-coded 5-verb
//! sample) has nothing left to check. What remains is the behavior no table
//! can assert about itself: the process boundary.

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mnemosyne-cli"))
}

/// The discovery act answers with the real surface — all of it, not a curated
/// subset. Byte-equality against `--help` is total: it covers every verb in
/// the table at once, which is what the old five-verb sample only approximated.
#[test]
fn bare_invocation_prints_the_whole_help() {
    let bare = bin().output().expect("run mnemosyne-cli with no args");
    let help = bin()
        .arg("--help")
        .output()
        .expect("run mnemosyne-cli --help");

    assert!(help.status.success(), "--help must exit 0");
    assert_eq!(
        bare.stdout, help.stdout,
        "a bare invocation must print exactly `--help`; a consumer typing the \
         program name is discovering it, and this list is the whole answer"
    );
}

/// Discovery is not an error: it exits 0 and says nothing on stderr.
#[test]
fn bare_invocation_succeeds_quietly() {
    let out = bin().output().expect("run mnemosyne-cli with no args");

    assert!(out.status.success(), "a bare invocation must exit 0");
    assert!(
        out.stderr.is_empty(),
        "a bare invocation must not write to stderr, got: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A verb absent from the table is a failure, never a silent no-op.
#[test]
fn unknown_command_fails_loud() {
    let out = bin()
        .arg("no-such-verb")
        .output()
        .expect("run mnemosyne-cli no-such-verb");

    assert!(
        !out.status.success(),
        "an unknown command must exit non-zero"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("unknown command: no-such-verb"),
        "the error must name the offending verb, got: {}",
        stderr
    );
}
