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
//!
//! Round 832 added the fourth answer, and it is the one the others cannot see:
//! when the pinned build is ALREADY INSTALLED, the binary switches to it instead
//! of refusing. Refusal and switching share one code path and one resolved path,
//! so a test that only ever meets an absent build cannot tell "declined to
//! procure" from "never looked".
//!
//! Every test here sets `MN_ROOT` to a temporary directory. Without that the
//! answers depend on what the developer happens to have installed under
//! `~/.local/mn`, which is a suite that passes for reasons outside itself.

use std::path::Path;
use std::process::Command;

use crate::common::link_stub;

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
fn run_in(bin: &str, workspace: &Path, skip: bool, mn_root: &Path) -> std::process::Output {
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
    // Where a pin resolves to, held to the test's own directory so the answer
    // does not depend on this machine's `~/.local/mn`.
    cmd.env("MN_ROOT", mn_root);
    // Never inherit the switch marker: a test that ran under one would read as
    // the loop guard tripping rather than as the case it means to check.
    cmd.env_remove(mnemosyne_config::PIN_EXEC_ENV);
    if skip {
        cmd.env(mnemosyne_config::PIN_SKIP_ENV, "1");
    } else {
        cmd.env_remove(mnemosyne_config::PIN_SKIP_ENV);
    }
    cmd.output().expect("spawn")
}

/// An empty install root — nothing is provisioned under this pin.
fn empty_root() -> tempfile::TempDir {
    tempfile::TempDir::new().expect("tempdir")
}

/// Install a stand-in as the pinned build of `bin`, at the layout
/// `cargo install --root` writes and [`mnemosyne_config::pinned_binary`] reads.
///
/// THE PROGRAM IS TRACKED AND REACHED BY SYMLINK; what varies per case is the
/// two files written beside it (`common::link_stub`, `tests/stubs/pin-stand-in`).
/// A pinned build this test WROTE and the CLI then execs is a file `exec`
/// refuses with `ETXTBSY` while any process holds it open for writing — and the
/// holder is a sibling test's fork rather than this thread (Round 1192).
///
/// `revision` is what the stand-in ANSWERS `--version` with. Round 861 made that
/// load-bearing: the switch asks the installed build which revision it is before
/// handing over, so a stand-in that cannot say is a broken install rather than a
/// pinned one. `None` models exactly that — a build too old to have a parseable
/// `--version`, which is the case the check exists for.
///
/// `body` is shell the stand-in runs for anything else, with the caller's
/// arguments in `"$@"`.
fn install_pinned(root: &Path, pin: &str, bin: &str, revision: Option<&str>, body: &str) {
    let dir = root.join(pin).join("bin");
    link_stub("pin-stand-in", &dir.join(bin));
    let version = match revision {
        Some(rev) => format!("stand-in 0.0.0 ({rev})\n"),
        None => "stand-in with no revision\n".to_string(),
    };
    std::fs::write(dir.join(format!("{bin}.version")), version).expect("the stand-in's version");
    std::fs::write(dir.join(format!("{bin}.body")), format!("{body}\n"))
        .expect("the stand-in's body");
}

#[test]
fn every_binary_refuses_a_workspace_pinned_to_another_revision() {
    // `deadbeef` is hex, seven-plus characters, and is not this build — so it
    // passes the shape check and fails the identity one, which is the case that
    // matters. (A local build is `-dirty` besides, and a dirty build satisfies
    // no pin at all; both paths land in the same refusal.)
    let ws = workspace_with("[workspace]\n\n[tool]\npin = \"deadbeef\"\n");
    let root = empty_root();
    for bin in &pinned_binaries() {
        let out = run_in(bin, ws.path(), false, root.path());
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
    let root = empty_root();
    for bin in &pinned_binaries() {
        let out = run_in(bin, ws.path(), false, root.path());
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
    let root = empty_root();
    // Round 863 — the pinned build is INSTALLED here, which it was not before.
    // The waiver used to sit in front of the only switch; now delegation happens
    // earlier, in a different function, and an empty root would let a waiver that
    // silently hands over pass this test for the wrong reason.
    for bin in &pinned_binaries() {
        install_pinned(
            root.path(),
            "deadbeef",
            bin,
            Some("deadbeef"),
            "echo SWITCHED-TO-PINNED\nexit 0",
        );
    }
    for bin in &pinned_binaries() {
        let out = run_in(bin, ws.path(), true, root.path());
        let err = String::from_utf8_lossy(&out.stderr);
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            err.contains(mnemosyne_config::PIN_SKIP_ENV),
            "{bin} waived the pin without saying so — a silent waiver is how a \
             waiver becomes permanent: {err}"
        );
        assert!(
            !err.contains("pins Mnemosyne `deadbeef`, and"),
            "{bin} refused despite the documented waiver: {err}"
        );
        assert!(
            !stdout.contains("SWITCHED-TO-PINNED"),
            "{bin} handed over under the waiver — a waived run is attributable to \
             no revision, and delegating makes it attributable to one: {stdout}"
        );
    }
}

/// Round 863 — THE FREEZE. The pin is read out of a generic table before the
/// document is validated, so a key this build has never heard of must not stop
/// the hand-off.
///
/// This is the test that keeps the floor from rising again. It is the entire
/// reason the pre-parse read is loose, and a round that couples it back to
/// `WorkspaceConfig` turns this red here rather than turning a consumer's gates
/// red in the field, which is how Round 861 found out last time.
#[test]
fn a_key_this_build_does_not_know_still_reaches_the_pin() {
    // `[from_a_later_round]` is what a NEWER Mnemosyne's section looks like to
    // this build: valid TOML, unknown to `WorkspaceConfig`, fatal to the strict
    // parse. The pin sits behind it in the file as well as behind it in time.
    let ws = workspace_with(
        "[workspace]\n\n[tool]\npin = \"deadbeef\"\n\n[from_a_later_round]\nkey = 1\n",
    );
    let root = empty_root();
    for bin in &pinned_binaries() {
        install_pinned(
            root.path(),
            "deadbeef",
            bin,
            Some("deadbeef"),
            "echo SWITCHED-TO-PINNED\nexit 0",
        );
    }
    for bin in &pinned_binaries() {
        let out = run_in(bin, ws.path(), false, root.path());
        let err = String::from_utf8_lossy(&out.stderr);
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            stdout.contains("SWITCHED-TO-PINNED"),
            "{bin} died on a key it does not know instead of handing the workspace \
             to the build that does\nstdout: {stdout}\nstderr: {err}"
        );
        assert!(
            out.status.success(),
            "{bin} switched but did not carry the replacement's exit code: {err}"
        );
    }
}

/// The other half of the freeze, and the one that can rot quietly: reading the
/// pin loosely must not make an unknown key TOLERATED. With no pin there is
/// nothing to delegate to, so the strict parse is still the answer and it must
/// still be loud.
#[test]
fn an_unknown_key_without_a_pin_is_still_fatal() {
    let ws = workspace_with("[workspace]\n\n[from_a_later_round]\nkey = 1\n");
    let root = empty_root();
    for bin in &pinned_binaries() {
        let out = run_in(bin, ws.path(), false, root.path());
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(
            !out.status.success(),
            "{bin} accepted a key no build of it knows: {err}"
        );
        assert!(
            err.contains("unknown field"),
            "{bin} failed on the unknown key without naming it: {err}"
        );
    }
}

/// The Round 832 answer: an installed pin is USED, not refused.
///
/// The stand-in is a shell script rather than a real Mnemosyne build because
/// what is under test is the switch, not the switched-to: a script that prints
/// a sentinel and exits 0 proves the exec happened, reached the right path, and
/// carried the arguments — none of which a second copy of the real binary would
/// show more clearly.
#[test]
fn an_installed_pinned_build_is_used_instead_of_refused() {
    let ws = workspace_with("[workspace]\n\n[tool]\npin = \"deadbeef\"\n");
    let root = empty_root();
    for bin in &pinned_binaries() {
        install_pinned(
            root.path(),
            "deadbeef",
            bin,
            Some("deadbeef"),
            "echo SWITCHED-TO-PINNED\nexit 0",
        );
    }
    for bin in &pinned_binaries() {
        let out = run_in(bin, ws.path(), false, root.path());
        let err = String::from_utf8_lossy(&out.stderr);
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            stdout.contains("SWITCHED-TO-PINNED"),
            "{bin} did not run the installed pinned build\nstdout: {stdout}\nstderr: {err}"
        );
        assert!(
            out.status.success(),
            "{bin} switched but did not carry the replacement's exit code\nstderr: {err}"
        );
        assert!(
            !err.contains("pins Mnemosyne `deadbeef`, and"),
            "{bin} refused a pin it could have satisfied by switching: {err}"
        );
        // Loud, and on stderr — an MCP server speaks its protocol on stdout, so
        // a note there would corrupt the stream of the very thing being fixed.
        assert!(
            err.contains("switching to the pinned build"),
            "{bin} became a different tool without saying so: {err}"
        );
        // Round 861 — the note and the refusal share one `PinRefusal` Display,
        // and each caller supplies its own sentence around it. Round 840 gave
        // `Different` a subject of its own to fix the refusal, which made THIS
        // caller's own subject a duplicate: `note: this build this build is ...`.
        // The two callers cannot be checked by one assertion, so this is the
        // note's own.
        assert!(
            !err.contains("this build this build"),
            "{bin}'s switch note doubles its subject — the note and the refusal \
             each supply one: {err}"
        );
    }
}

/// Round 861 — the pin path is a NAMING convention, so the build sitting there
/// is a claim, and the claim is checked BEFORE the hand-off.
///
/// The loop guard checks it after, which works only when the replacement is new
/// enough to re-check a pin at all: a build older than `[tool]` dies at TOML
/// parse instead, and that message blames the reader's config for what is a
/// broken install. Asking first is the only place the right answer can be given.
#[test]
fn a_pinned_path_holding_another_revision_is_refused_before_the_hand_off() {
    let ws = workspace_with("[workspace]\n\n[tool]\npin = \"deadbeef\"\n");
    let root = empty_root();
    for bin in &pinned_binaries() {
        install_pinned(
            root.path(),
            "deadbeef",
            bin,
            // Reports a revision, and it is not the one the directory names.
            Some("cafef00d"),
            "echo RAN-THE-WRONG-BUILD\nexit 0",
        );
    }
    for bin in &pinned_binaries() {
        let out = run_in(bin, ws.path(), false, root.path());
        let err = String::from_utf8_lossy(&out.stderr);
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            !out.status.success(),
            "{bin} handed over to a build that is not the revision its path names\nstderr: {err}"
        );
        // THE POINT: it never ran. A check that only reports afterwards is the
        // one already there, and it is the one that cannot reach an old build.
        assert!(
            !stdout.contains("RAN-THE-WRONG-BUILD"),
            "{bin} exec'd the mis-installed build and only then complained: {stdout}"
        );
        assert!(
            err.contains("cafef00d") && err.contains("deadbeef"),
            "{bin} must name BOTH what the path claims and what is actually there, \
             or the reader cannot tell which one to fix: {err}"
        );
        assert!(
            err.contains("cargo install --git"),
            "{bin} reported a broken install without the line that repairs it: {err}"
        );
    }
}

/// The same check when the installed build will not answer at all — a build
/// older than `--version` itself, or something that is not a Mnemosyne binary.
/// Unverifiable is refused, for the reason Round 826 refuses an `unknown` stamp:
/// not knowing is not the same as knowing it is right.
#[test]
fn a_pinned_path_holding_a_build_that_names_no_revision_is_refused() {
    let ws = workspace_with("[workspace]\n\n[tool]\npin = \"deadbeef\"\n");
    let root = empty_root();
    for bin in &pinned_binaries() {
        install_pinned(
            root.path(),
            "deadbeef",
            bin,
            None,
            "echo RAN-AN-UNVERIFIABLE-BUILD\nexit 0",
        );
    }
    for bin in &pinned_binaries() {
        let out = run_in(bin, ws.path(), false, root.path());
        let err = String::from_utf8_lossy(&out.stderr);
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            !out.status.success(),
            "{bin} handed over to a build that would not say what it is\nstderr: {err}"
        );
        assert!(
            !stdout.contains("RAN-AN-UNVERIFIABLE-BUILD"),
            "{bin} exec'd a build it could not verify: {stdout}"
        );
        assert!(
            err.contains("reports no revision at all"),
            "{bin} must separate `cannot be checked` from `checked and wrong` — \
             they are different repairs: {err}"
        );
    }
}

/// Round 861 — `--version` is the question the switch asks, so every shipped
/// binary owes an answer.
///
/// Round 286 declared it a universal surface and `mnemosyne-render` never had
/// one, which cost nothing until a binary's revision became something another
/// binary ASKS for. The set comes from cargo, so a new binary is covered the day
/// it exists rather than the day someone remembers it.
#[test]
fn every_shipped_binary_answers_which_revision_it_is() {
    for bin in &pinned_binaries() {
        let out = Command::new(env!("CARGO"))
            .args([
                "run",
                "--quiet",
                "--manifest-path",
                concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"),
                "-p",
                bin,
                "--",
                "--version",
            ])
            .output()
            .expect("spawn");
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            out.status.success(),
            "{bin} --version failed, so the switch cannot verify a build of it\n\
             stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        // The parenthesised tail is what the switch reads. Asserting only
        // "exit 0" would pass for a binary that printed nothing at all.
        let open = stdout.rfind('(');
        let revision = open
            .and_then(|o| stdout[o..].find(')').map(|c| stdout[o + 1..o + c].trim()))
            .unwrap_or("");
        assert!(
            !revision.is_empty(),
            "{bin} --version names no revision in parentheses, so a build of it \
             installed under a pin cannot be verified: {stdout}"
        );
    }
}

/// The boundary the switch must NOT cross: an absent build is refused, never
/// fetched. Enforcement and procurement stay separate.
#[test]
fn an_absent_pinned_build_is_refused_and_never_procured() {
    let ws = workspace_with("[workspace]\n\n[tool]\npin = \"deadbeef\"\n");
    let root = empty_root();
    for bin in &pinned_binaries() {
        let out = run_in(bin, ws.path(), false, root.path());
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(
            !out.status.success(),
            "{bin} proceeded with no pinned build installed: {err}"
        );
        assert!(
            err.contains("cargo install --git"),
            "{bin} refused without the line that installs the build it wants: {err}"
        );
        // The refusal must name the place the switch actually looked, or the
        // reader installs into a directory the tool will not read.
        assert!(
            err.contains(&root.path().display().to_string()),
            "{bin}'s remedy names a root other than the one it resolved: {err}"
        );
        assert!(
            root.path().join("deadbeef").metadata().is_err(),
            "{bin} provisioned a build under the pin — enforcement must not procure"
        );
    }
}

/// The loop guard. A build installed under a pin that is not that revision would
/// otherwise exec itself forever; it must stop after one hop and say why.
///
/// Round 861 moved the ordinary case of this OUT of the guard's reach — a build
/// whose `--version` disagrees with its directory is now refused before the
/// hand-off. What the guard still covers is the case that check cannot see: a
/// path that ANSWERS with the pinned revision and then does not honour it. So
/// the stand-in here answers `deadbeef` and execs the real CLI, which is this
/// crate's own binary (`CARGO_BIN_EXE_*`, guaranteed built) because only a build
/// that re-checks a pin can reach the guard at all.
///
/// That is not a contrived shape: a build being replaced between the check and
/// the exec is how the incident this round answers began — a concurrent session
/// reinstalled a binary under a path already in use.
#[test]
fn a_mis_installed_pin_stops_after_one_switch() {
    let ws = workspace_with("[workspace]\n\n[tool]\npin = \"deadbeef\"\n");
    let root = empty_root();
    install_pinned(
        root.path(),
        "deadbeef",
        "mnemosyne-cli",
        Some("deadbeef"),
        &format!("exec {} \"$@\"", env!("CARGO_BIN_EXE_mnemosyne-cli")),
    );

    let out = run_in("mnemosyne-cli", ws.path(), false, root.path());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "a build that is not the revision its path names was accepted: {err}"
    );
    assert!(
        err.contains("already switched to"),
        "the second mismatch was not reported as a broken install, so the \
         reader cannot tell it from a first refusal: {err}"
    );
    // Exactly one hop: a second note would mean the guard let it go round again.
    assert_eq!(
        err.matches("switching to the pinned build").count(),
        1,
        "the switch ran more than once: {err}"
    );
}
