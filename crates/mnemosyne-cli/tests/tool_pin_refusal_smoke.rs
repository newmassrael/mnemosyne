//! Round 826 — every Mnemosyne binary refuses a workspace pinned to another
//! revision, checked on the BINARIES rather than on the rule they share.
//!
//! The rule itself is unit-tested in `mnemosyne-config`. What that cannot see is
//! whether a given binary ever registers its identity: `register_tool_stamp` is
//! one line in one `main`, and the enforcement shape was chosen precisely
//! because forgetting it fails CLOSED — an unidentified process is refused, not
//! waved through. Fail-closed keeps a forgotten registration from being silently
//! unsafe, but only a test like this notices that it was forgotten at all.
//!
//! So this walks each shipped binary through the same three answers: refuse a
//! foreign pin with a non-zero exit, pass an unpinned workspace untouched (the
//! opt-in half — every consumer that predates the pin), and honour the documented
//! waiver while saying out loud that it did.

use std::path::Path;
use std::process::Command;

/// Binaries that do NOT open a workspace, each with the reason it is out.
///
/// The complement is DERIVED from cargo, never listed: `PINNED_BINARIES` below
/// asks cargo for every binary in this workspace and subtracts these. Round 826
/// hand-listed the enforcing side instead and the list was wrong within one
/// round — `mnemosyne-render` takes a workspace as its first argument, was never
/// named, and could not open a pinned workspace at all. That is the Round 783
/// lesson repeating: a list restates the tree and then drifts from it in
/// silence, because a binary merely absent looks exactly like one deliberately
/// left out. Inverting it makes absence loud and only a written-down name quiet.
const NOT_WORKSPACE_BINARIES: &[(&str, &str)] = &[
    (
        "mnemosyne-index",
        "an admin driver for the RocksDB index: every path is an explicit \
         argument (--atomic, --index) and it never discovers a workspace config",
    ),
    (
        "projection_stack_probe",
        "a measurement probe inside mnemosyne-engine-build, not a shipped tool",
    ),
];

/// Every binary this workspace builds, minus the declared non-workspace ones —
/// asked of cargo so a new binary is covered the day it exists.
fn pinned_binaries() -> Vec<String> {
    let out = Command::new(env!("CARGO"))
        .args([
            "metadata",
            "--format-version",
            "1",
            "--no-deps",
            "--manifest-path",
            concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"),
        ])
        .output()
        .expect("cargo metadata");
    assert!(out.status.success(), "cargo metadata failed");
    let meta: serde_json::Value = serde_json::from_slice(&out.stdout).expect("metadata json");
    let mut all: Vec<String> = Vec::new();
    for pkg in meta["packages"].as_array().expect("packages") {
        for target in pkg["targets"].as_array().expect("targets") {
            let is_bin = target["kind"]
                .as_array()
                .is_some_and(|k| k.iter().any(|v| v == "bin"));
            if is_bin {
                all.push(target["name"].as_str().expect("target name").to_string());
            }
        }
    }
    // A declared exclusion that no longer matches anything is STALE and is
    // reported, the Round 783 rule — otherwise the list quietly becomes folklore.
    for (name, _) in NOT_WORKSPACE_BINARIES {
        assert!(
            all.iter().any(|b| b == name),
            "`{name}` is declared as a non-workspace binary but this workspace \
             builds no such binary — a stale exclusion hides whatever replaced it"
        );
    }
    all.retain(|b| !NOT_WORKSPACE_BINARIES.iter().any(|(n, _)| n == b));
    assert!(
        !all.is_empty(),
        "no binaries to check — the derivation is broken"
    );
    all
}

fn workspace_with(toml: &str) -> tempfile::TempDir {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    std::fs::write(tmp.path().join("mnemosyne.toml"), toml).expect("write config");
    tmp
}

/// Run `bin` so that it must OPEN the workspace — `--workspace` for the server,
/// a workspace-reading subcommand from that directory for the CLI.
///
/// Through `cargo run`, not a path into `target/`, and that is the whole reason
/// this helper has a doc comment. Cargo builds this crate's binary before the
/// test and knows nothing about the server in the next crate, so a path lookup
/// finds whatever `target/` happens to hold — an older build, or nothing at all
/// once `scripts/verify.sh --fresh` has cleaned that crate. Both happened here:
/// the first run of this test failed against a message the working tree had
/// already fixed, and the first full-suite run failed with "was not built".
///
/// Asking cargo makes staleness and absence alike impossible, for the same
/// reason `scripts/mn` asks cargo rather than comparing mtimes: freshness is
/// cargo's question, and a second answer to it is free to be wrong.
fn run_in(bin: &str, workspace: &Path, skip: bool) -> std::process::Output {
    let mut cmd = Command::new(env!("CARGO"));
    // `--manifest-path` so cargo builds HERE while the child runs THERE: the
    // CLI discovers its workspace by walking up from its own directory, so
    // running it from the repo would have it find this repo's config instead of
    // the pinned one under test — which is a pass that proves nothing.
    cmd.args([
        "run",
        "--quiet",
        "--manifest-path",
        concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"),
        "-p",
        bin,
        "--",
    ]);
    // HOW each binary is made to open a workspace. The SET is derived from
    // cargo; only the invocation is written down, and an unknown name panics
    // rather than being skipped — a new binary must be taught, loudly, instead
    // of silently passing a test that never ran it.
    match bin {
        "mnemosyne-cli" => {
            cmd.arg("validate-workspace");
        }
        "mnemosyne-mcp" => {
            cmd.arg("--workspace").arg(workspace);
            // The server would otherwise sit on stdio waiting for a client;
            // closing stdin makes a SUCCESSFUL start terminate promptly.
            cmd.stdin(std::process::Stdio::null());
        }
        "mnemosyne-render" => {
            cmd.arg(workspace).arg("reader");
        }
        other => panic!(
            "{other} is a workspace binary with no invocation here — add how it \
             opens a workspace, or declare it in NOT_WORKSPACE_BINARIES with a reason"
        ),
    }
    cmd.current_dir(workspace);
    if skip {
        cmd.env(mnemosyne_config::PIN_SKIP_ENV, "1");
    } else {
        cmd.env_remove(mnemosyne_config::PIN_SKIP_ENV);
    }
    cmd.output().expect("spawn")
}

#[test]
fn every_binary_refuses_a_workspace_pinned_to_another_revision() {
    // `deadbeef` is hex, seven-plus characters, and is not this build — so it
    // passes the shape check and fails the identity one, which is the case that
    // matters. (A local build is `-dirty` besides, and a dirty build satisfies
    // no pin at all; both paths land in the same refusal.)
    let ws = workspace_with("[workspace]\n\n[tool]\npin = \"deadbeef\"\n");
    for bin in &pinned_binaries() {
        let out = run_in(bin, ws.path(), false);
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(
            !out.status.success(),
            "{bin} ran against a workspace pinned to another revision\nstderr: {err}"
        );
        assert!(
            err.contains("deadbeef"),
            "{bin} refused without naming the pin, so the reader cannot act on it: {err}"
        );
        assert!(
            err.contains(bin),
            "{bin}'s remedy must name {bin}, not whichever binary the message was written for: {err}"
        );
        // THE REGISTRATION IS CHECKED HERE, and this assertion is the reason the
        // test earns its cost. Refusal alone proves nothing about it: a binary
        // that never calls `register_tool_stamp` is refused too — that is
        // fail-closed working — so "it refused" is satisfied by the broken case
        // as well as the correct one. An injection removing the registration
        // from `mnemosyne-render` reported ZERO failures against the earlier
        // version of this file, which is how the gap was found rather than
        // reasoned about. The REASON separates them: an identified build says
        // which revision it is, an unregistered one cannot.
        assert!(
            !err.contains("did not declare which revision it is"),
            "{bin} never registered its own revision — it refuses everything, \
             including the pin it actually satisfies: {err}"
        );
        assert!(
            err.contains("this build is `"),
            "{bin}'s refusal must state the revision it IS, or the reader cannot \
             tell a wrong tool from an unidentified one: {err}"
        );
    }
}

#[test]
fn an_unpinned_workspace_is_untouched_by_any_binary() {
    // The opt-in half. If this ever fails, the pin has been made mandatory and
    // every consumer that predates it is broken.
    let ws = workspace_with("[workspace]\n");
    for bin in &pinned_binaries() {
        let out = run_in(bin, ws.path(), false);
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(
            !err.contains("pins Mnemosyne"),
            "{bin} applied a pin to a workspace that declares none: {err}"
        );
    }
}

#[test]
fn the_waiver_is_honoured_and_never_silent() {
    let ws = workspace_with("[workspace]\n\n[tool]\npin = \"deadbeef\"\n");
    for bin in &pinned_binaries() {
        let out = run_in(bin, ws.path(), true);
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(
            err.contains(mnemosyne_config::PIN_SKIP_ENV),
            "{bin} waived the pin without saying so — a silent waiver is how a \
             waiver becomes permanent: {err}"
        );
        assert!(
            !err.contains("pins Mnemosyne `deadbeef`, and"),
            "{bin} refused despite the documented waiver: {err}"
        );
    }
}
