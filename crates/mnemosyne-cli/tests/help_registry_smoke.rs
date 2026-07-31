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

/// The verb set this binary dispatches, read back out of `--help` — which the
/// tests above pin as the whole surface. Guarded against vacuity: a parse that
/// silently yielded a handful would make the contract check below pass on
/// nothing.
fn dispatched_verbs(help: &str) -> std::collections::BTreeSet<String> {
    let verbs: std::collections::BTreeSet<String> = help
        .lines()
        .filter_map(|line| {
            let mut it = line.split_whitespace();
            let prog = it.next()?;
            let verb = it.next()?;
            (prog.ends_with("mnemosyne-cli")
                && verb.starts_with(|c: char| c.is_ascii_lowercase())
                && verb.chars().all(|c| c.is_ascii_lowercase() || c == '-'))
            .then(|| verb.to_string())
        })
        .collect();
    assert!(
        verbs.len() > 40,
        "parsed only {} verbs out of --help — the parse broke, and a broken parse \
         would let the contract check below pass by finding nothing to check",
        verbs.len()
    );
    verbs
}

/// Round 907 — every VERB the authoring contract hands its reader must be a verb
/// this binary dispatches.
///
/// This file's own header records the failure it is guarding: a second,
/// hand-maintained command list drifted, and a consumer was taught that five
/// present capabilities did not exist. `describe-schema` is a third list one
/// layer up — it does not enumerate commands, but it names them inside prose an
/// author follows literally ("declare it via `add-unit` first",
/// "`report-authoring-frontier` reports those scenes"), and nothing checked that
/// those names still dispatch. A renamed verb would leave the contract sending
/// authors at a command that answers `unknown command`.
///
/// Measured before it was written (R907): 30 distinct verb names across the
/// contract, 0 of them missing. So this starts GREEN — it is prevention, not a
/// repair, and the honest claim is that it holds a currently-true property, not
/// that it found something.
///
/// SCOPE, stated because the neighbouring drift was NOT of this kind: this
/// checks that a NAME resolves, never that the prose around it describes what
/// the verb does. R906 fixed a `containment` description that had named no dead
/// identifier at all — it described a superseded MODEL in live vocabulary. No
/// name-existence check reaches that, and pretending otherwise would be the
/// green-and-hollow class.
#[test]
fn every_verb_the_authoring_contract_names_is_dispatched() {
    let help = bin().arg("--help").output().expect("run --help");
    let help = String::from_utf8(help.stdout).expect("help is utf-8");
    let dispatched = dispatched_verbs(&help);

    let contract = bin()
        .args(["describe-schema", "--json"])
        .output()
        .expect("run describe-schema --json");
    assert!(
        contract.status.success(),
        "describe-schema --json must exit 0"
    );
    let contract: serde_json::Value =
        serde_json::from_slice(&contract.stdout).expect("the contract is JSON");

    // Every string the contract serializes is prose an author reads.
    let mut prose = Vec::new();
    collect_strings(&contract, &mut prose);
    assert!(
        prose.len() > 100,
        "walked only {} strings of the contract — the walk broke",
        prose.len()
    );

    // Verb-shaped tokens: the imperative prefixes this CLI actually uses.
    let prefixes = [
        "report-",
        "validate-",
        "import-",
        "add-",
        "set-",
        "remove-",
        "propose-",
        "describe-",
        "emit-",
        "redact-",
    ];
    let mut named: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for s in &prose {
        for raw in s.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-')) {
            let token = raw.trim_matches('-');
            if prefixes.iter().any(|p| token.starts_with(p)) && token.len() > 4 {
                named.insert(token.to_string());
            }
        }
    }
    assert!(
        named.len() > 20,
        "found only {} verb names in the contract — the extraction broke, and an \
         extraction that finds nothing asserts nothing",
        named.len()
    );

    let missing: Vec<&String> = named.iter().filter(|v| !dispatched.contains(*v)).collect();
    assert!(
        missing.is_empty(),
        "the authoring contract names {} verb(s) this binary does not dispatch: {:?} — \
         an author following the contract would be answered `unknown command`",
        missing.len(),
        missing
    );
}

/// Every string value in a serialized contract, recursively.
fn collect_strings(v: &serde_json::Value, out: &mut Vec<String>) {
    match v {
        serde_json::Value::String(s) => out.push(s.clone()),
        serde_json::Value::Array(items) => items.iter().for_each(|i| collect_strings(i, out)),
        serde_json::Value::Object(map) => map.values().for_each(|i| collect_strings(i, out)),
        _ => {}
    }
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
