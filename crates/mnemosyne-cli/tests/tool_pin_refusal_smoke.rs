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

/// The binaries that open a workspace, and therefore owe this behaviour. A new
/// one belongs in this list on the day it is written.
const PINNED_BINARIES: &[&str] = &["mnemosyne-cli", "mnemosyne-mcp"];

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
    if bin == "mnemosyne-mcp" {
        cmd.arg("--workspace").arg(workspace);
        // The server would otherwise sit on stdio waiting for a client; closing
        // stdin makes a SUCCESSFUL start terminate promptly instead of hanging.
        cmd.stdin(std::process::Stdio::null());
    } else {
        cmd.arg("validate-workspace")
            .arg("--sidecar")
            .arg(workspace.join("store.json"));
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
    for bin in PINNED_BINARIES {
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
    }
}

#[test]
fn an_unpinned_workspace_is_untouched_by_any_binary() {
    // The opt-in half. If this ever fails, the pin has been made mandatory and
    // every consumer that predates it is broken.
    let ws = workspace_with("[workspace]\n");
    for bin in PINNED_BINARIES {
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
    for bin in PINNED_BINARIES {
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
