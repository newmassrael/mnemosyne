//! Round 899 — the hooks gate everything else, and nothing gated them.
//!
//! `.githooks/` carries 515 lines that reject hallucinated citations, malformed
//! commit messages, version-postfix identifiers, unformatted code and lint
//! regressions, and that report the CI state of the commit a push builds on.
//! Round 890 recorded that deleting any block of it would be silent; Round 893
//! added logic to `pre-push` and recorded the same carry again. This file is
//! the missing gate: every branch runs against a throwaway git repo wired to
//! the REAL hook files, so a deleted block turns a case red.
//!
//! The hooks are exercised, never copied. `core.hooksPath` points at this
//! repository's own `.githooks/`, because a copied hook is free to drift from
//! the one that actually runs — the rot Round 873 and Round 897 each found in a
//! fixture nothing loaded.
//!
//! What this CANNOT cover, stated rather than implied:
//!   - the real behaviour of `validate-code-refs` / `validate-workspace`. The
//!     fixture's `scripts/mn` is a stub, so these cases prove the hook CALLS
//!     the verb and honours its exit code, nothing about the verb itself
//!     (those have their own tests).
//!   - `gh`'s own filtering. The stub returns already-filtered lines, exactly
//!     as `gh -q` would, so a wrong `--json` field or jq expression in the hook
//!     would still pass here.
//!   - the real workspace's fmt/clippy state, which CI and the hook itself
//!     cover on the real tree.

use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;

/// Formatted, lint-clean, and free of any version-postfix identifier — the
/// baseline every gate must accept.
const CLEAN_LIB: &str = "pub fn one() -> u8 {\n    1\n}\n";

/// A `.rs` file the COMPILER never sees and the gates DO: a module nothing
/// declares.
///
/// THAT DIFFERENCE IS WHAT MAKES IT USABLE HERE. `cargo fmt` formats what a
/// crate root reaches and clippy compiles the same set, so an unattached file
/// walks past Gates 4, 5 and 5a untouched; `tools/blind-waits` walks the TREE,
/// cannot parse this, and answers `2` — "I could not read enough of this to
/// have an opinion" — which is the one answer no case here had ever produced.
const UNREADABLE_ORPHAN: &str = "fn ( this file is not rust\n";

/// Logs the verb it was asked for, then REFUSES anything the real CLI does not
/// answer to. Round 900: a stub that accepts every argument would let a hook
/// call a misspelled verb and still pass — the gap Round 899 named in a carry
/// instead of closing.
const MN_STUB: &str = r#"#!/usr/bin/env bash
echo "$@" >> "$PWD/mn-calls.log"
case "${1:-}" in
    validate-code-refs|validate-workspace) ;;
    *)
        echo "stub mn: no CLI verb is named '${1:-}'" >&2
        exit 127
        ;;
esac
exit "${MN_STUB_EXIT:-0}"
"#;

/// Answers the three `gh` calls `pre-push` makes, keyed by `GH_STUB_MODE`, and
/// CHECKS THE ARGUMENTS of each. Round 900: returning canned lines without
/// looking at the request meant a hook that asked for the wrong `--json` field
/// or the wrong endpoint still got its answer — Round 899 wrote that down as a
/// carry. The stub cannot evaluate jq, so what it pins is the request contract:
/// the subcommand, the field list, the headSha filter, and the endpoint shape.
/// Violations land in a file the test asserts is empty.
const GH_STUB: &str = r#"#!/usr/bin/env bash
mode="${GH_STUB_MODE:-unreachable}"
violate() { echo "$1" >> "$PWD/gh-contract-violations.log"; }
if [[ "${1:-}" == "run" ]]; then
    [[ "${2:-}" == "list" ]] || violate "gh run subcommand is '${2:-}', not 'list'"
    [[ "$*" == *"--json headSha,name,status,conclusion"* ]] \
        || violate "gh run list asked for the wrong --json fields: $*"
    [[ "$*" == *'headSha=='* ]] \
        || violate "gh run list does not filter on headSha: $*"
fi
if [[ "${1:-}" == "api" ]]; then
    if [[ "${2:-}" == "repos/{owner}/{repo}/commits/"*"/check-runs" ]]; then
        :
    elif [[ "${2:-}" == "repos/{owner}/{repo}/check-runs/"*"/annotations" ]]; then
        :
    else
        violate "gh api hit an unexpected endpoint: ${2:-}"
    fi
fi
if [[ "${1:-}" == "run" ]]; then
    case "$mode" in
        empty) exit 0 ;;
        red) echo "mnemosyne-validate completed failure"; exit 0 ;;
        annotated) echo "mnemosyne-validate completed success"; exit 0 ;;
        *) exit 1 ;;
    esac
fi
if [[ "${1:-}" == "api" && "$mode" == "annotated" ]]; then
    if [[ "${2:-}" == *"/annotations" ]]; then
        echo "warning Node.js 20 actions are deprecated"
        exit 0
    fi
    echo "42 3"
    exit 0
fi
exit 1
"#;

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR = <root>/crates/mnemosyne-cli
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repo root is two levels above the crate manifest")
        .to_path_buf()
}

fn hook(name: &str) -> PathBuf {
    let p = repo_root().join(".githooks").join(name);
    assert!(p.is_file(), "the tracked hook {} must exist", p.display());
    let mode = fs::metadata(&p).expect("stat").permissions().mode();
    assert!(
        mode & 0o111 != 0,
        "the hook {} must be executable — git SILENTLY SKIPS a hook without \
         the bit, which removes the gate with no message at all",
        p.display()
    );
    p
}

fn write_exec(path: &Path, body: &str) {
    fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    fs::write(path, body).expect("write");
    let mut perms = fs::metadata(path).expect("stat").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).expect("chmod");
}

/// A throwaway git repo wired to the real hooks, holding the smallest tree the
/// hooks' cargo gates can run on.
struct Fixture {
    dir: TempDir,
}

impl Fixture {
    fn new() -> Self {
        let f = Fixture {
            dir: TempDir::new().expect("tempdir"),
        };
        f.git(&["init", "-q", "."]);
        f.git(&["config", "user.email", "hooks@test"]);
        f.git(&["config", "user.name", "hooks test"]);
        f.git(&["config", "commit.gpgsign", "false"]);
        f.git(&[
            "config",
            "core.hooksPath",
            repo_root()
                .join(".githooks")
                .to_str()
                .expect("hooks path is utf-8"),
        ]);
        f.write(
            "Cargo.toml",
            "[package]\nname = \"fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[workspace]\n",
        );
        f.write("src/lib.rs", CLEAN_LIB);
        f.write(".gitignore", "mn-calls.log\n/target\n");
        write_exec(&f.path().join("scripts/mn"), MN_STUB);
        f.generate_lockfile("Cargo.toml");
        f.stage_all();
        f
    }

    /// A REPOSITORY HAS A LOCKFILE. R1115 put `--locked` on every cargo command
    /// this repository issues, the hooks included, and a hook is pointed at the
    /// WORKING DIRECTORY — so a fixture without one is a tree the hooks
    /// correctly refuse, and seven tests here said so the moment the flag went
    /// on. R1112 made the same correction for `.cargo/config.toml`: a fixture
    /// that does not look like a repository fails for a reason that is about
    /// the fixture.
    ///
    /// CARGO WRITES IT, not this file. A hand-written lockfile pins a format
    /// version, and the day cargo moves to the next one these tests would fail
    /// for a third reason that is again about the fixture. `--offline` because
    /// these trees have no dependencies and a test must not need a network.
    fn generate_lockfile(&self, manifest: &str) {
        let out = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string()))
            .args([
                "generate-lockfile",
                "--offline",
                "--manifest-path",
                manifest,
            ])
            .current_dir(self.path())
            .env("CARGO_TARGET_DIR", self.path().join("target"))
            .output()
            .expect("cargo exec");
        assert!(
            out.status.success(),
            "cargo generate-lockfile for {manifest} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn write(&self, rel: &str, body: &str) {
        let p = self.path().join(rel);
        fs::create_dir_all(p.parent().expect("parent")).expect("mkdir");
        fs::write(p, body).expect("write");
    }

    /// Every git call carries the fixture's own target dir, so a nested cargo
    /// (the hooks' fmt/clippy gates) can never touch the outer build's — an
    /// inherited `CARGO_TARGET_DIR` would otherwise contend on its lock.
    fn git_allow_failure(&self, args: &[&str]) -> Output {
        Command::new("git")
            .args(args)
            .current_dir(self.path())
            .env("CARGO_TARGET_DIR", self.path().join("target"))
            .output()
            .expect("git exec")
    }

    fn git(&self, args: &[&str]) -> Output {
        let out = self.git_allow_failure(args);
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        out
    }

    fn stage_all(&self) {
        self.git(&["add", "-A"]);
    }

    /// Run one real hook with the fixture as its working tree.
    fn run_hook(&self, name: &str, args: &[&str], stdin: &str, env: &[(&str, &str)]) -> Output {
        let mut cmd = Command::new(hook(name));
        cmd.args(args)
            .current_dir(self.path())
            .env("CARGO_TARGET_DIR", self.path().join("target"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (k, v) in env {
            cmd.env(k, v);
        }
        let mut child = cmd.spawn().expect("hook spawn");
        child
            .stdin
            .as_mut()
            .expect("stdin")
            .write_all(stdin.as_bytes())
            .expect("write stdin");
        child.wait_with_output().expect("hook wait")
    }

    /// Which verbs the stub resolver was asked for, in order.
    fn mn_calls(&self) -> String {
        fs::read_to_string(self.path().join("mn-calls.log")).unwrap_or_default()
    }

    /// Every request `pre-push` made to `gh` that broke the pinned contract.
    fn gh_contract_violations(&self) -> String {
        fs::read_to_string(self.path().join("gh-contract-violations.log")).unwrap_or_default()
    }

    fn commit_msg(&self, message: &str) -> Output {
        let msg_path = self.path().join("COMMIT_EDITMSG_case");
        fs::write(&msg_path, message).expect("write message");
        self.run_hook(
            "commit-msg",
            &[msg_path.to_str().expect("msg path is utf-8")],
            "",
            &[],
        )
    }
}

fn stderr_of(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).to_string()
}

// ---------------------------------------------------------------- commit-msg

#[test]
fn commit_msg_accepts_the_house_format_and_names_every_violation() {
    let f = Fixture::new();

    let long_subject = format!("docs(narrative): {}", "a".repeat(56));
    assert_eq!(long_subject.len(), 73, "the case must exceed the 72 cap");
    let long_bullet = format!("- {}", "b".repeat(71));

    // (message, expected reject reason) — None = must be accepted.
    let cases: Vec<(String, Option<&str>)> = vec![
        (
            "docs(narrative): a clean subject\n\n- one bullet\n- a second bullet\n".into(),
            None,
        ),
        ("Merge branch 'topic' into main\n".into(), None),
        (
            "# a git instruction line\ndocs(narrative): comments are stripped\n\n- one bullet\n"
                .into(),
            None,
        ),
        ("".into(), Some("empty message")),
        (
            "wip: not a declared type\n".into(),
            Some("subject must match"),
        ),
        (format!("{long_subject}\n"), Some("73 bytes (max 72)")),
        (
            "docs(narrative): a trailing period.\n".into(),
            Some("must not end with a period"),
        ),
        (
            "docs(narrative): body on line two\n- a bullet\n".into(),
            Some("Line 2: must be blank"),
        ),
        (
            "docs(narrative): prose in the body\n\nthis is a prose paragraph\n".into(),
            Some("bullets only, no prose"),
        ),
        (
            "docs(narrative): a wrapped bullet\n\n- a bullet that\n  wraps onto a second line\n"
                .into(),
            Some("indented continuation"),
        ),
        (
            "docs(narrative): a blank inside the body\n\n- one bullet\n\n- another bullet\n".into(),
            Some("blank line inside body"),
        ),
        (
            "docs(narrative): four bullets\n\n- one\n- two\n- three\n- four\n".into(),
            Some("4 bullets (max 3)"),
        ),
        (
            format!("docs(narrative): an over-long bullet\n\n{long_bullet}\n"),
            Some("73 bytes (max 72)"),
        ),
        (
            "docs(narrative): a forbidden trailer\n\n- one bullet\n\nCo-Authored-By: someone\n"
                .into(),
            Some("Co-Authored-By"),
        ),
        (
            "docs(narrative): a generated trailer\n\n- Generated with Claude Code\n".into(),
            Some("Generated with Claude Code"),
        ),
        (
            "docs(narrative): an emoji sneaks in\n\n- shipped \u{1F680}\n".into(),
            Some("Emoji detected"),
        ),
        (
            "docs(narrative): korean in the log\n\n- 한글 본문\n".into(),
            Some("Non-English"),
        ),
    ];

    for (message, expected) in cases {
        let out = f.commit_msg(&message);
        let err = stderr_of(&out);
        let first = message.lines().next().unwrap_or("<empty>");
        match expected {
            None => assert!(
                out.status.success(),
                "commit-msg must ACCEPT `{first}`, but it rejected:\n{err}"
            ),
            Some(reason) => {
                assert!(
                    !out.status.success(),
                    "commit-msg must REJECT `{first}` for `{reason}`, but it accepted"
                );
                assert!(
                    err.contains(reason),
                    "commit-msg rejected `{first}` for the wrong reason \
                     (wanted `{reason}`):\n{err}"
                );
            }
        }
    }

    // The typographic whitelist is the counterpart of the English-only rule:
    // without this the Korean case above would also pass with the rule deleted.
    let out = f.commit_msg("docs(narrative): typography stays legal\n\n- sec 4.7 \u{2192} ok\n");
    assert!(
        out.status.success(),
        "the whitelisted arrow must be accepted:\n{}",
        stderr_of(&out)
    );
}

// ---------------------------------------------------------------- pre-commit

#[test]
fn pre_commit_gates_on_the_citation_verbs_exit_code() {
    let f = Fixture::new();
    // A non-.rs change, so this case reaches ONLY the citation gate.
    f.write("NOTES.md", "a line\n");
    f.stage_all();

    let accepted = f.run_hook("pre-commit", &[], "", &[("MN_STUB_EXIT", "0")]);
    assert!(
        accepted.status.success(),
        "a passing citation gate must let the commit through:\n{}",
        stderr_of(&accepted)
    );

    let rejected = f.run_hook("pre-commit", &[], "", &[("MN_STUB_EXIT", "1")]);
    assert!(
        !rejected.status.success(),
        "a failing citation gate must reject the commit"
    );
    let err = stderr_of(&rejected);
    assert!(
        err.contains("code citation violations"),
        "the rejection must name the citation gate:\n{err}"
    );
    assert!(
        f.mn_calls().contains("validate-code-refs"),
        "the hook must actually invoke validate-code-refs, called: {:?}",
        f.mn_calls()
    );
}

#[test]
fn pre_commit_runs_the_workspace_gate_only_when_the_sidecar_is_staged() {
    // Without the sidecar staged.
    let bare = Fixture::new();
    bare.write("NOTES.md", "a line\n");
    bare.stage_all();
    let out = bare.run_hook("pre-commit", &[], "", &[]);
    assert!(out.status.success(), "{}", stderr_of(&out));
    assert!(
        !bare.mn_calls().contains("validate-workspace"),
        "the workspace gate must NOT run when the sidecar is unstaged, called: {:?}",
        bare.mn_calls()
    );

    // With it staged — the same tree, one file different. This pair is the
    // discriminator: asserting only the positive would pass with the `if`
    // deleted.
    let staged = Fixture::new();
    staged.write(
        "docs/.atomic/workspace.atomic.json",
        "{\"schema_version\":23,\"sections\":{},\"changelog_entries\":{}}",
    );
    staged.stage_all();
    let out = staged.run_hook("pre-commit", &[], "", &[]);
    assert!(out.status.success(), "{}", stderr_of(&out));
    assert!(
        staged.mn_calls().contains("validate-workspace"),
        "staging the sidecar must run the workspace gate, called: {:?}",
        staged.mn_calls()
    );
}

#[test]
fn pre_commit_rejects_unformatted_rust() {
    let f = Fixture::new();
    // Unformatted AND reachable from the crate root: rustfmt only reads files
    // the module tree reaches, so an orphan file would pass and prove nothing.
    f.write("src/lib.rs", "pub fn one()->u8{1}\n");
    f.stage_all();

    let out = f.run_hook("pre-commit", &[], "", &[]);
    assert!(!out.status.success(), "unformatted code must be rejected");
    let err = stderr_of(&out);
    assert!(
        err.contains("unformatted code"),
        "the rejection must name the format gate:\n{err}"
    );
}

#[test]
fn pre_commit_rejects_lint_dirty_rust() {
    let f = Fixture::new();
    // rustfmt-clean but clippy-dirty (`needless_return`), so this case reaches
    // the clippy gate rather than stopping at the format gate before it.
    f.write(
        "src/lib.rs",
        "pub fn one() -> u8 {\n    let x = 1;\n    return x;\n}\n",
    );
    f.stage_all();

    let out = f.run_hook("pre-commit", &[], "", &[]);
    assert!(
        !out.status.success(),
        "a clippy warning must be denied under -D warnings"
    );
    let err = stderr_of(&out);
    assert!(
        err.contains("cargo clippy"),
        "the run must have reached the clippy gate:\n{err}"
    );
    assert!(
        !err.contains("unformatted code"),
        "this case must not be stopped by the format gate:\n{err}"
    );
}

#[test]
fn pre_commit_rejects_a_test_that_waits_on_a_clock() {
    // Gate 5c. R1073 turned main red with an assertion whose subject was the
    // scheduler rather than the store, and R1081 found four more of the shape
    // in the tree that runs on every push. The fixture's test sleeps and then
    // asserts — nothing re-checks anything, so its green is a claim that this
    // machine got there in time.
    //
    // rustfmt-clean and clippy-clean on purpose, so the case REACHES 5c instead
    // of stopping at Gate 4 or 5 in front of it.
    let f = Fixture::new();
    f.write(
        "src/lib.rs",
        "pub fn one() -> u8 {\n    1\n}\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn \
         the_other_thread_got_there() {\n        \
         std::thread::sleep(std::time::Duration::from_millis(300));\n        \
         assert_eq!(super::one(), 1);\n    }\n}\n",
    );
    f.stage_all();

    let out = f.run_hook("pre-commit", &[], "", &[]);
    assert!(
        !out.status.success(),
        "a wait no condition ends must be rejected"
    );
    let err = stderr_of(&out);
    assert!(
        err.contains("commit rejected — a test waits on a clock"),
        "the rejection must name the blind-wait gate:\n{err}"
    );
    // The hook announces every gate it RUNS, so the presence of `cargo clippy`
    // in this stream says nothing about who rejected — only a `commit rejected`
    // line does. The first version of this assertion read the announcement as a
    // verdict and went red on a passing run.
    assert!(
        !err.contains("unformatted code"),
        "this case must not be stopped by the format gate in front of it:\n{err}"
    );
}

#[test]
fn pre_commit_tells_a_gate_that_could_not_read_apart_from_one_that_found_a_defect() {
    // GATE 5c's OTHER NON-ZERO EXIT, and the one nothing here ran. `1` is
    // "these sites break the law" and `2` is "I could not read enough of this
    // tree to have an opinion"; the hook prints a different sentence for each,
    // and its own comment records what one message for both cost — a workspace
    // that could not be read was reported as a wait on a clock. The case above
    // covers `1` alone, so a hook that collapsed the two stayed green.
    //
    // The orphan is the lever, for the reason `UNREADABLE_ORPHAN` gives: every
    // gate in front of 5c compiles what a crate root reaches, and this is not
    // reachable from one.
    let f = Fixture::new();
    f.write("src/orphan.rs", UNREADABLE_ORPHAN);
    f.stage_all();

    let out = f.run_hook("pre-commit", &[], "", &[]);
    assert!(
        !out.status.success(),
        "a gate that reached no verdict must not pass a commit"
    );
    let err = stderr_of(&out);
    assert!(
        err.contains("the blind-wait gate could not read this tree (exit 2)"),
        "the rejection must say the gate could not judge, and with which code:\n{err}"
    );
    // THE MIRROR, and the whole point of this case: the sentence belonging to
    // the other non-zero exit. A hook that answered `1` here would send somebody
    // hunting for a `sleep` in a tree that has none.
    assert!(
        !err.contains("a test waits on a clock"),
        "a tree it could not read is not a tree with a blind wait in it:\n{err}"
    );
    // AND THE GATE'S OWN WORDS ARE ABOVE, which is exactly what the hook's
    // sentence promises the reader will find.
    assert!(
        err.contains("NO VERDICT"),
        "the gate's own message must reach the same stream the hook points at:\n{err}"
    );
}

#[test]
fn pre_commit_gates_a_separate_in_repo_workspace_the_root_gates_miss() {
    // `cargo fmt --all` / `clippy --workspace` only see root members, so a
    // crate carrying its OWN `[workspace]` is invisible to Gates 4 and 5. This
    // is the gate that walks up to the owning workspace root and runs there.
    let dirty = Fixture::new();
    dirty.write(
        "tools/sub/Cargo.toml",
        "[package]\nname = \"sub\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[workspace]\n",
    );
    dirty.write("tools/sub/src/lib.rs", "pub fn two()->u8{2}\n");
    dirty.generate_lockfile("tools/sub/Cargo.toml");
    dirty.stage_all();

    let out = dirty.run_hook("pre-commit", &[], "", &[]);
    assert!(
        !out.status.success(),
        "unformatted code in a separate workspace must be rejected"
    );
    let err = stderr_of(&out);
    assert!(
        err.contains("tools/sub is unformatted"),
        "the rejection must name the separate workspace:\n{err}"
    );
    assert!(
        err.contains("side-workspaces"),
        "and it must come from the ONE gate CI runs too, not from a copy of its \
         two commands that this hook used to carry (R1066):\n{err}"
    );

    // The other direction: the walk-up must not reject a clean one, or the
    // assertion above would hold with the gate wired to always fail.
    let clean = Fixture::new();
    clean.write(
        "tools/sub/Cargo.toml",
        "[package]\nname = \"sub\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[workspace]\n",
    );
    clean.write("tools/sub/src/lib.rs", "pub fn two() -> u8 {\n    2\n}\n");
    clean.generate_lockfile("tools/sub/Cargo.toml");
    clean.stage_all();
    let out = clean.run_hook("pre-commit", &[], "", &[]);
    assert!(
        out.status.success(),
        "a formatted separate workspace must pass:\n{}",
        stderr_of(&out)
    );
    assert!(
        stderr_of(&out).contains("separate workspace 'tools/sub'"),
        "the gate must have actually run on it:\n{}",
        stderr_of(&out)
    );
}

#[test]
fn the_side_workspace_gate_tells_a_gate_it_could_not_read_from_one_that_found_a_defect() {
    // THE SAME TWO ANSWERS, ONE LEVEL DOWN. `scripts/check-side-workspaces.sh`
    // is where a separate workspace's blind-wait gate is run, and it too prints
    // a different sentence for exit 1 and exit 2 — a branch nothing had ever
    // taken. It matters more here than in the hook: the message names the
    // WORKSPACE, so the wrong one sends somebody looking for a `sleep` in
    // whichever crate the gate merely failed to read.
    let f = Fixture::new();
    f.write(
        "tools/sub/Cargo.toml",
        "[package]\nname = \"sub\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[workspace]\n",
    );
    f.write("tools/sub/src/lib.rs", CLEAN_LIB);
    f.write("tools/sub/src/orphan.rs", UNREADABLE_ORPHAN);
    f.generate_lockfile("tools/sub/Cargo.toml");
    f.stage_all();

    let out = f.run_hook("pre-commit", &[], "", &[]);
    assert!(
        !out.status.success(),
        "a separate workspace the gate could not read must not pass"
    );
    let err = stderr_of(&out);
    assert!(
        err.contains("the blind-wait gate could not read tools/sub (exit 2)"),
        "the refusal must name the workspace and the code:\n{err}"
    );
    // THE MIRROR: the sentence the other exit prints, which is the one this
    // branch exists to not be.
    assert!(
        !err.contains("carries a wait that ends on a clock"),
        "a workspace it could not read carries no finding about waits:\n{err}"
    );
}

#[test]
fn the_side_workspace_gate_answers_two_when_it_was_not_started_in_a_tree() {
    // THE LISTER'S OWN TWO CODES, which nothing anywhere reads: the hook and CI
    // both treat any non-zero as a rejection, so the distinction between "a
    // workspace failed its gate" and "I was not run from the root of a tree"
    // survives only as words on a screen — and words with no reader drift. This
    // case is that reader.
    let gate = repo_root().join("scripts/check-side-workspaces.sh");
    let nowhere = TempDir::new().expect("tempdir");
    let out = Command::new(&gate)
        .current_dir(nowhere.path())
        .output()
        .expect("the gate runs");
    assert_eq!(
        out.status.code(),
        Some(2),
        "a gate started outside a tree has no verdict about one:\n{}",
        stderr_of(&out)
    );
    assert!(
        stderr_of(&out).contains("has no Cargo.toml"),
        "and it names what it wanted to find:\n{}",
        stderr_of(&out)
    );

    // THE CONTROL, and the reason the code above means anything: pointed at a
    // real tree with a real defect, the SAME script answers 1. Unformatted
    // rather than unreadable, so nothing here compiles.
    let f = Fixture::new();
    f.write(
        "tools/sub/Cargo.toml",
        "[package]\nname = \"sub\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[workspace]\n",
    );
    f.write("tools/sub/src/lib.rs", "pub fn two()->u8{2}\n");
    f.generate_lockfile("tools/sub/Cargo.toml");
    f.stage_all();
    let out = Command::new(&gate)
        .args(["--lint-only", "tools/sub"])
        .current_dir(f.path())
        .env("CARGO_TARGET_DIR", f.path().join("target"))
        .output()
        .expect("the gate runs");
    assert_eq!(
        out.status.code(),
        Some(1),
        "a workspace that broke a law is a finding, not an unstartable gate:\n{}",
        stderr_of(&out)
    );
}

#[test]
fn pre_commit_rejects_a_version_postfix_identifier() {
    let f = Fixture::new();
    // The banned identifier is ASSEMBLED, never written: Gate 6 scans added
    // lines in staged `.rs` and covers test data too (CLAUDE.md), so spelling
    // it out here would make this file uncommittable by the very gate it
    // proves — and `--no-verify` is not an answer the project accepts.
    let banned = format!("parse_{}{}", 'v', 2);
    // Formatted and lint-clean, so only Gate 6 can reject it.
    f.write(
        "src/lib.rs",
        &format!("pub fn {banned}() -> u8 {{\n    2\n}}\n"),
    );
    f.stage_all();

    let out = f.run_hook("pre-commit", &[], "", &[]);
    assert!(!out.status.success(), "a vN identifier must be rejected");
    let err = stderr_of(&out);
    assert!(
        err.contains("vN version-postfix identifier"),
        "the rejection must name the vN ban:\n{err}"
    );
    assert!(
        err.contains(&banned),
        "the rejection must quote the offending line:\n{err}"
    );

    // The documented exemption: `_version` is a real version number, not a
    // postfix. Without this the ban could be a bare `_v` match and still pass.
    let ok = Fixture::new();
    ok.write(
        "src/lib.rs",
        "pub const SCHEMA_VERSION: u8 = 23;\npub fn schema_version() -> u8 {\n    SCHEMA_VERSION\n}\n",
    );
    ok.stage_all();
    let out = ok.run_hook("pre-commit", &[], "", &[]);
    assert!(
        out.status.success(),
        "`schema_version` must not trip the vN ban:\n{}",
        stderr_of(&out)
    );
}

#[test]
fn a_real_commit_passes_through_the_hooks_git_itself_invokes() {
    // The other cases invoke the hook files directly. This one proves the
    // `core.hooksPath` wiring the install step asks for actually reaches them.
    let f = Fixture::new();
    let out = f.git_allow_failure(&[
        "commit",
        "-m",
        "test(fixture): a clean commit\n\n- one bullet",
    ]);
    assert!(
        out.status.success(),
        "a clean commit must pass the wired hooks:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        f.mn_calls().contains("validate-code-refs"),
        "git must have run the pre-commit hook, called: {:?}",
        f.mn_calls()
    );

    // And the commit-msg hook is wired too: the same tree, a bad message.
    f.write("NOTES.md", "a line\n");
    f.stage_all();
    let out = f.git_allow_failure(&["commit", "-m", "no type prefix here"]);
    assert!(
        !out.status.success(),
        "git must have run the commit-msg hook and rejected the message"
    );
}

// ------------------------------------------------------------------ pre-push

/// One ref-update line as git feeds it: `<local_ref> <local_sha> <remote_ref>
/// <remote_sha>`. An all-zero LOCAL sha is a deletion.
fn push_line(local_sha: &str) -> String {
    format!(
        "refs/heads/topic {local_sha} refs/heads/topic {}\n",
        "0".repeat(40)
    )
}

fn head_sha(f: &Fixture) -> String {
    String::from_utf8(f.git(&["rev-parse", "HEAD"]).stdout)
        .expect("sha is utf-8")
        .trim()
        .to_string()
}

#[test]
fn pre_push_skips_delete_only_pushes_and_gates_on_the_workspace() {
    let f = Fixture::new();
    f.git(&["commit", "--no-verify", "-q", "-m", "test(fixture): seed"]);
    let sha = head_sha(&f);

    for (label, stdin) in [
        ("a deletion", push_line(&"0".repeat(40))),
        ("no ref updates at all", String::new()),
    ] {
        let out = f.run_hook("pre-push", &["origin", "git@example:x"], &stdin, &[]);
        assert!(out.status.success(), "{label} must skip the gates");
        assert!(
            stderr_of(&out).contains("delete-only push"),
            "{label} must say why it skipped:\n{}",
            stderr_of(&out)
        );
        assert!(
            f.mn_calls().is_empty(),
            "{label} must not run any gate, called: {:?}",
            f.mn_calls()
        );
    }

    // A real push does run the workspace gate, and its exit code is honoured.
    let out = f.run_hook(
        "pre-push",
        &["origin", "git@example:x"],
        &push_line(&sha),
        &[("MN_STUB_EXIT", "1")],
    );
    assert!(
        !out.status.success(),
        "a failing workspace gate must block the push"
    );
    assert!(
        stderr_of(&out).contains("validate-workspace failed"),
        "the block must name the gate:\n{}",
        stderr_of(&out)
    );
}

#[test]
fn pre_push_reports_every_ci_state_it_can_and_cannot_read() {
    let f = Fixture::new();
    f.git(&["commit", "--no-verify", "-q", "-m", "test(fixture): seed"]);
    let sha = head_sha(&f);
    let stdin = push_line(&sha);

    // No origin/main yet — the branch that cannot look at all.
    let out = f.run_hook("pre-push", &["origin", "git@example:x"], &stdin, &[]);
    assert!(out.status.success(), "reporting must never block the push");
    assert!(
        stderr_of(&out).contains("no local origin/main ref"),
        "a missing ref must say so:\n{}",
        stderr_of(&out)
    );

    f.git(&["update-ref", "refs/remotes/origin/main", &sha]);

    // `gh` missing entirely. Needs a hermetic PATH, since this machine keeps
    // gh in the same directory as git; the stub cargo stands in for the two
    // lint gates, which have their own cases above.
    let shim = f.path().join("shim");
    fs::create_dir_all(&shim).expect("mkdir shim");
    for tool in ["bash", "git", "grep", "sed", "head", "sort"] {
        let real = Command::new("sh")
            .args(["-c", &format!("command -v {tool}")])
            .output()
            .expect("which");
        let real = String::from_utf8(real.stdout).expect("path is utf-8");
        let real = real.trim();
        assert!(
            !real.is_empty(),
            "the hermetic PATH case needs `{tool}` on this machine"
        );
        std::os::unix::fs::symlink(real, shim.join(tool)).expect("symlink");
    }
    write_exec(&shim.join("cargo"), "#!/usr/bin/env bash\nexit 0\n");
    let out = f.run_hook(
        "pre-push",
        &["origin", "git@example:x"],
        &stdin,
        &[("PATH", shim.to_str().expect("shim path is utf-8"))],
    );
    assert!(out.status.success(), "{}", stderr_of(&out));
    assert!(
        stderr_of(&out).contains("gh not installed"),
        "an absent gh must be reported, not passed over:\n{}",
        stderr_of(&out)
    );

    // The three answers a present gh can give.
    let gh_dir = f.path().join("ghbin");
    write_exec(&gh_dir.join("gh"), GH_STUB);
    let path_with_gh = format!(
        "{}:{}",
        gh_dir.to_str().expect("gh dir is utf-8"),
        std::env::var("PATH").unwrap_or_default()
    );

    for (mode, wanted) in [
        ("unreachable", "gh could not reach GitHub"),
        ("empty", "no CI runs recorded"),
        ("red", "is RED"),
        ("annotated", "1 distinct of 3 reported"),
    ] {
        let out = f.run_hook(
            "pre-push",
            &["origin", "git@example:x"],
            &stdin,
            &[("PATH", &path_with_gh), ("GH_STUB_MODE", mode)],
        );
        assert!(
            out.status.success(),
            "the CI report must never block the push (mode {mode}):\n{}",
            stderr_of(&out)
        );
        assert!(
            stderr_of(&out).contains(wanted),
            "mode {mode} must report `{wanted}`:\n{}",
            stderr_of(&out)
        );
    }

    // Every request the hook made was the one it is supposed to make. This is
    // what keeps the canned answers from covering for a hook that asked the
    // wrong question — a wrong `--json` field, a wrong subcommand, a wrong
    // endpoint. The stub cannot judge the jq expression, only the request.
    assert!(
        f.gh_contract_violations().is_empty(),
        "pre-push broke the gh request contract:\n{}",
        f.gh_contract_violations()
    );

    // A green conclusion does not silence the annotation half (Round 893): the
    // `annotated` arm above returns `success`, and the annotation still prints.
    let out = f.run_hook(
        "pre-push",
        &["origin", "git@example:x"],
        &stdin,
        &[("PATH", &path_with_gh), ("GH_STUB_MODE", "annotated")],
    );
    let err = stderr_of(&out);
    assert!(
        err.contains("completed success") && err.contains("Node.js 20"),
        "a green run must still surface what it said:\n{err}"
    );
}
