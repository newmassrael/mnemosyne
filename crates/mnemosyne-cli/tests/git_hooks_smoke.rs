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
//!   - what GitHub answers about a commit, and what that answer means. Until
//!     R1136 the hook read it with three `gh -q` expressions and the stub here
//!     returned already-filtered lines, so a wrong `--json` field or jq
//!     expression still passed — measured twice, each time at 14 passed / 0
//!     failed while a real `gh` would have reported a red commit as clean. That
//!     reading is now `tools/ci-state`, gated in its own suite against RECORDED
//!     real bodies with the reporter run as a process. What is left here is the
//!     seam this file owns: that the hook calls it with the commit it means and
//!     passes its words through.
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

/// Stands in for the three `cargo` commands `pre-push` issues, and CHECKS THE
/// ARGUMENTS of the one this file is about.
///
/// R1136 MOVED THE SEAM. The hook used to call `gh` three times and flatten the
/// answers with `-q`; a stub could not evaluate those expressions, so the request
/// contract was all this file could pin and two renamed fields passed it. The
/// reading is now a program (`tools/ci-state`) with its own suite over recorded
/// bodies, and what THIS file owns is the call: the hook must ask that reporter,
/// with a lockfile it may not rewrite, about the commit it says it means — and
/// must pass whatever comes back through to the person pushing.
///
/// `fmt` and `clippy` have their own cases above and are answered blindly here.
/// Violations land in a file the test asserts is empty.
const CARGO_STUB: &str = r#"#!/usr/bin/env bash
violate() { echo "$1" >> "$PWD/reporter-contract-violations.log"; }
if [[ "${1:-}" != "run" ]]; then
    exit 0
fi
# The separate-workspace gate (R1156) runs this repository's own gate programs
# through `cargo run` as well, and they are not this stub's subject — each has
# its own suite. This stub owns exactly ONE of them, the CI reporter.
#
# A RULE, NOT A LIST (R1184). Until this round the others were named one by one,
# and that list went stale the moment a gate was added: R1182 added one, the stub
# read it as "some other program", and the case failed for a reason that has
# nothing to do with the seam it owns. What is asked now is the SHAPE — a
# manifest under this repository's own `tools/` that is not the reporter's is
# some other gate's business — so a gate added to the hook needs no edit here and
# cannot silently fall off. Still by PATH, so the exemption cannot be claimed by
# anything else the hook might call.
case "$*" in
    *"/tools/ci-state/Cargo.toml"*) ;;
    *"/tools/"*"/Cargo.toml"*) exit 0 ;;
esac
[[ "$*" == *"/tools/ci-state/Cargo.toml"* ]] \
    || violate "pre-push ran some other program than the CI reporter: $*"
[[ "$*" == *"--bin ci-state"* ]] \
    || violate "pre-push did not name the reporter binary: $*"
[[ "$*" == *"--locked"* ]] \
    || violate "the reporter resolves freely and may rewrite a lockfile: $*"
[[ "$*" == *"$REPORTER_SHA"* ]] \
    || violate "pre-push asked about some commit other than origin/main: $*"
case "${REPORTER_MODE:-reports}" in
    reports)
        echo "ci-state: CI on ${REPORTER_SHA:0:8} — 9 check(s): 1 failure, 8 success"
        echo "ci-state:   failure — every cache declared is one CI keeps"
        echo "ci-state: ^^ the commit you are building on is RED. Not blocking"
        echo "ci-state: CI annotations on ${REPORTER_SHA:0:8} — 1 distinct of 3 reported:"
        echo "ci-state:   warning Node.js 20 actions are deprecated"
        exit 0
        ;;
    *) exit 2 ;;
esac
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
        // A REPOSITORY HAS MORE THAN ONE WORKSPACE, and since R1156 `pre-push`
        // gates on the separate ones — whole. The gate refuses a tree it reached
        // NO separate workspace in, by its own non-vacuity rule, so a fixture
        // without one is a tree the hooks correctly refuse: the same correction
        // the missing lockfile above needed, for the same reason. Clean and
        // trivial, so it is a baseline every case can build on rather than a
        // subject; the cases that are ABOUT a separate workspace overwrite it.
        f.write(
            "tools/sub/Cargo.toml",
            "[package]\nname = \"sub\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[workspace]\n",
        );
        f.write("tools/sub/src/lib.rs", "pub fn two() -> u8 {\n    2\n}\n");
        f.generate_lockfile("tools/sub/Cargo.toml");
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

    /// Every way `pre-push` called the CI reporter that broke the pinned
    /// contract.
    fn reporter_contract_violations(&self) -> String {
        fs::read_to_string(self.path().join("reporter-contract-violations.log")).unwrap_or_default()
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

/// Both streams, for a claim about something the hook's CHILD narrated. The
/// hooks write their own lines to stderr, and the gates they call write theirs
/// wherever they write them — a case that reads only stderr can miss a step that
/// ran, which is how the R1156 case first went red over a suite that had in fact
/// executed.
fn both_of(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
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
fn the_side_workspace_gate_tells_an_environment_it_could_not_read_from_one_it_judged() {
    // THE THIRD GATE'S TWO ANSWERS, and the reason they need a reader here more
    // than in either sibling: BOTH ARMS EXIT 1. The lister rejects a workspace
    // whose environment gate found a defect and a workspace whose environment
    // gate could not be read with the SAME CODE, so the only thing carrying the
    // difference is the sentence — and a sentence nothing reads is a sentence
    // that drifts. R1182 added this arm beside the two it was modelled on and
    // left it unread; this is the reader its blind-wait sibling got in R1132.
    //
    // ONE TREE, ONE FILE DIFFERENT. The halves share a fixture, so the only
    // thing that can explain two different answers is the one file that changed
    // between them — and they share a target directory, which is what keeps the
    // second build of the gate under check off the clock.
    let gate = repo_root().join("scripts/check-side-workspaces.sh");
    let f = Fixture::new();
    // The workspace has to get PAST fmt, clippy, the citation gate and the
    // blind-wait gate to reach the arm under test at all: the sibling case's
    // unparsable orphan answers 2 one gate earlier and never arrives here.
    f.write(
        "tools/sub/tests/entrance.rs",
        "#[test]\nfn spawns() {\n    \
         let _ = std::process::Command::new(env!(\"CARGO_BIN_EXE_sub\")).output();\n}\n",
    );
    let lister = || {
        Command::new(&gate)
            .args(["--lint-only", "tools/sub"])
            .current_dir(f.path())
            .env("CARGO_TARGET_DIR", f.path().join("target"))
            .output()
            .expect("the gate runs")
    };

    // A NAME THE WALK CANNOT TURN INTO A VARIABLE — the gate's exit 2, "I could
    // not read enough of this tree to have an opinion".
    f.write(
        "tools/sub/src/main.rs",
        "fn main() {\n    \
         let which = std::env::args().nth(1).unwrap_or_default();\n    \
         let _ = std::env::var(which);\n}\n",
    );
    let out = lister();
    let err = stderr_of(&out);
    assert_eq!(
        out.status.code(),
        Some(1),
        "a workspace the environment gate could not read must not pass:\n{err}"
    );
    assert!(
        err.contains("the named-environment gate could not read tools/sub (exit 2)"),
        "the refusal must name the workspace and the code:\n{err}"
    );
    // THE MIRROR: the sentence the other exit prints, which is the one this
    // branch exists to not be. It names a test that leaves a variable to the
    // machine, and there is no such test here — only a name nobody could read.
    assert!(
        !err.contains("spawns a program whose environment its test"),
        "a workspace it could not read carries no finding about environments:\n{err}"
    );

    // THE SAME TREE with that one file replaced by a program whose variable is
    // perfectly readable and which the spawning test never says — the gate's
    // exit 1, a judged workspace with a defect in it.
    f.write(
        "tools/sub/src/main.rs",
        "fn main() {\n    let _ = std::env::var(\"GITHUB_REF_NAME\");\n}\n",
    );
    let out = lister();
    let err = stderr_of(&out);
    assert_eq!(
        out.status.code(),
        Some(1),
        "a workspace that broke the law is a rejection too:\n{err}"
    );
    assert!(
        err.contains("tools/sub spawns a program whose environment its test leaves to the machine"),
        "the finding must name the workspace it is about:\n{err}"
    );
    assert!(
        !err.contains("the named-environment gate could not read"),
        "a workspace it judged was not one it failed to read:\n{err}"
    );
}

/// A `grep` that has no PCRE at all — the reading the first repair of this
/// defect guessed at, kept because it is the OTHER way the capability can be
/// missing and the hook must answer for both.
const GREP_WITHOUT_PCRE: &str = r#"#!/usr/bin/env bash
for argument in "$@"; do
    case "$argument" in
        -*P*) echo "grep: invalid option -- 'P'" >&2; exit 2 ;;
    esac
done
exec /usr/bin/grep "$@"
"#;

#[test]
fn commit_msg_enforces_its_content_rules_in_a_locale_that_cannot_read_characters() {
    // THE SILENT-FAIL THAT SHIPPED, and the reason it is written as an
    // ENVIRONMENT rather than as a machine. Two of this hook's rules — no emoji,
    // English only — are `grep -P` over Unicode code-point classes, and both
    // ended in `2>/dev/null`, which makes every way of going wrong look like
    // "no match".
    //
    // IT WAS FOUND BY PLACEMENT: this suite passes here and on one build host,
    // and on the other it said `commit-msg must REJECT an emoji, but it
    // accepted`. The cause is not a missing `-P` — that host has one — but a
    // non-UTF-8 LOCALE, in which a four-byte emoji is four characters and no
    // code-point class can match it. Named here, the other machine's condition
    // is reproducible on this one, which is the whole point of naming it.
    let f = Fixture::new();
    let msg_path = f.path().join("COMMIT_EDITMSG_locale");
    fs::write(&msg_path, "docs(narrative): an emoji sneaks in \u{1F600}\n").expect("write message");
    let out = f.run_hook(
        "commit-msg",
        &[msg_path.to_str().expect("msg path is utf-8")],
        "",
        &[("LC_ALL", "C"), ("LANG", "C")],
    );
    let err = stderr_of(&out);
    assert!(
        !out.status.success(),
        "a rule that cannot read the message must not report it clean:\n{err}"
    );
    assert!(
        err.contains("Emoji detected"),
        "and the hook repairs the locale rather than refusing, so the rule it \
         claims to enforce is the one that answers:\n{err}"
    );

    // THE OTHER WAY THE CAPABILITY GOES MISSING, which no locale can repair: a
    // grep with no PCRE at all. Here refusing IS the answer, and it must say
    // what it could not do rather than merely fail.
    let shim = f.path().join("shim-nopcre");
    fs::create_dir_all(&shim).expect("mkdir shim");
    write_exec(&shim.join("grep"), GREP_WITHOUT_PCRE);
    let hobbled = format!(
        "{}:{}",
        shim.to_str().expect("shim path is utf-8"),
        std::env::var("PATH").unwrap_or_default()
    );
    fs::write(&msg_path, "fix(ci): a message that breaks no other rule\n").expect("write message");
    let out = f.run_hook(
        "commit-msg",
        &[msg_path.to_str().expect("msg path is utf-8")],
        "",
        &[("PATH", &hobbled)],
    );
    let err = stderr_of(&out);
    assert!(
        !out.status.success(),
        "a hook that cannot enforce what it claims must not accept:\n{err}"
    );
    assert!(
        err.contains("cannot match a Unicode code-point"),
        "and it must name the capability it lacks:\n{err}"
    );

    // THE CONTROL: the same message through an unhobbled hook is accepted, so
    // neither case above is merely a hook that rejects everything.
    let out = f.commit_msg("fix(ci): a message that breaks no other rule");
    assert!(
        out.status.success(),
        "the message itself is fine:\n{}",
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

/// PRE-PUSH RUNS THE SEPARATE WORKSPACES, AND WHOLE (R1156).
///
/// `--workspace` in the three gates above means the ROOT workspace. The others
/// carry their own `[workspace]` so the root gates never compile them, and until
/// this round the only thing that ran their SUITES was CI: pre-commit reaches
/// them only when a `.rs` INSIDE one is staged, and only for the lint half.
///
/// WHAT THAT COST, measured rather than argued. `tools/injection-harness`'s
/// `sweeps.rs` asks whether every tracked injection still APPLIES, and an anchor
/// is exact text naming a file in the ROOT workspace. Rounds 1151 and 1152
/// rewrote two such files; five root-workspace runs stayed green, no side `.rs`
/// was ever staged so the pre-commit gate never fired, and the red arrived on
/// `origin/main`. This case is the reader that branch was missing.
#[test]
fn pre_push_gates_on_every_separate_workspace_and_names_the_one_that_fails() {
    // The baseline fixture's separate workspace, made unformatted.
    let dirty = Fixture::new();
    dirty.write("tools/sub/src/lib.rs", "pub fn two()->u8{2}\n");
    dirty.stage_all();
    dirty.git(&["commit", "--no-verify", "-q", "-m", "test(fixture): seed"]);
    let sha = head_sha(&dirty);

    let out = dirty.run_hook(
        "pre-push",
        &["origin", "git@example:x"],
        &push_line(&sha),
        &[],
    );
    assert!(
        !out.status.success(),
        "a separate workspace that fails its own gate must block the push:\n{}",
        stderr_of(&out)
    );
    let err = stderr_of(&out);
    assert!(
        err.contains("tools/sub is unformatted"),
        "the block must name the separate workspace:\n{err}"
    );
    assert!(
        err.contains("separate in-repo workspace does not pass its own gate"),
        "and it must be this hook's own refusal, not a message from some other \
         gate the push happened to trip:\n{err}"
    );

    // THE MIRROR, and it is what stops the assertion above from holding for a
    // hook wired to reject every push that has a side workspace at all: the
    // baseline tree, whose separate workspace is clean, passes AND the gate is
    // seen to have run its SUITE on it.
    let clean = Fixture::new();
    clean.git(&["commit", "--no-verify", "-q", "-m", "test(fixture): seed"]);
    let sha = head_sha(&clean);

    let out = clean.run_hook(
        "pre-push",
        &["origin", "git@example:x"],
        &push_line(&sha),
        &[],
    );
    assert!(
        out.status.success(),
        "a clean separate workspace must not block the push:\n{}",
        both_of(&out)
    );
    let both = both_of(&out);
    assert!(
        both.contains("check-side-workspaces.sh"),
        "the hook must say it ran the ONE gate CI runs, not a copy of its \
         commands:\n{both}"
    );
    // AND THE SUITE HALF RAN, which is the half that was missing: `--lint-only`
    // stops after fmt and clippy, and this hook asking for that would rebuild the
    // hole one notch smaller. Read off the GATE'S OWN command line rather than
    // off cargo's wording, and from STDOUT as well as stderr — the gate narrates
    // on stdout, which is why the first version of this case failed while the
    // suite had in fact run.
    assert!(
        both.contains("COMMAND tools/sub suite"),
        "the whole gate includes the separate workspace's SUITE, and that is the \
         half CI alone was carrying:\n{both}"
    );
    assert!(
        !both.contains("--lint-only"),
        "the mirror of the assertion above — the lint-only form must be gone \
         from this call, not merely joined by the suite:\n{both}"
    );
}

#[test]
fn pre_push_carries_the_ci_reporters_words_and_names_it_when_it_cannot_run() {
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

    // AND THEN HEAD MOVES PAST IT, which is what makes "which commit" a question
    // this fixture can answer at all. The hook reports on the commit the push
    // BUILDS ON, and while `origin/main` and `HEAD` are the same commit a hook
    // asking about either one looks identical from here — so the law would pass
    // for a hook that reports on the work being pushed instead of the work it
    // lands on, which is a clean report about a commit nothing has run yet.
    f.write("src/second.rs", CLEAN_LIB);
    f.write(
        "src/lib.rs",
        "pub mod second;\npub fn one() -> u8 {\n    1\n}\n",
    );
    f.stage_all();
    f.git(&[
        "commit",
        "--no-verify",
        "-q",
        "-m",
        "test(fixture): move HEAD on",
    ]);
    let head = head_sha(&f);
    assert_ne!(head, sha, "the fixture must be able to tell the two apart");
    let stdin = push_line(&head);

    // A PATH WHOSE ONLY `cargo` IS THE STUB — that is the whole of what this
    // shim is for. This machine keeps the real one beside `git`, and a hook that
    // found it would compile this repository's reporter into the fixture's
    // target directory: a several-minute case that measures cargo rather than
    // the hook.
    //
    // PREPENDED, not substituted (R1156). It used to REPLACE `PATH` with a
    // directory holding two symlinks, which starved every ordinary tool as a
    // side effect of hiding one — and the moment `pre-push` grew a gate that
    // shells out, the case failed on `dirname`, then on `find` and `sort`
    // inside the gate's script. Those failures said nothing about the seam this
    // case owns. The stub still wins because it comes first, which is the claim.
    let shim = f.path().join("shim");
    fs::create_dir_all(&shim).expect("mkdir shim");
    for tool in ["bash", "git"] {
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
    write_exec(&shim.join("cargo"), CARGO_STUB);
    let hermetic = format!(
        "{}:{}",
        shim.to_str().expect("shim path is utf-8"),
        std::env::var("PATH").unwrap_or_default()
    );

    // WHAT THE REPORTER SAYS REACHES THE PERSON PUSHING, verbatim. The hook's job
    // at this seam is carriage: it neither summarises the report nor decides what
    // in it is worth repeating, and a hook that swallowed the RED line would be
    // the R890 blindness rebuilt one layer up.
    let out = f.run_hook(
        "pre-push",
        &["origin", "git@example:x"],
        &stdin,
        &[("PATH", &hermetic), ("REPORTER_SHA", &sha)],
    );
    let err = stderr_of(&out);
    assert!(
        out.status.success(),
        "the CI report must never block the push:\n{err}"
    );
    assert!(err.contains("is RED"), "the verdict is carried:\n{err}");
    assert!(
        err.contains("every cache declared is one CI keeps"),
        "and so is the row behind it:\n{err}"
    );
    assert!(
        err.contains("1 distinct of 3 reported") && err.contains("Node.js 20"),
        "and the annotation half, which a green conclusion does not silence \
         (R893):\n{err}"
    );

    // A REPORTER THAT COULD NOT REPORT IS SAID OUT LOUD. Exit 2 is the one
    // non-zero code it has, and a hook that passed over it would leave the push
    // looking exactly like one where CI was checked and found fine.
    let out = f.run_hook(
        "pre-push",
        &["origin", "git@example:x"],
        &stdin,
        &[
            ("PATH", &hermetic),
            ("REPORTER_SHA", &sha),
            ("REPORTER_MODE", "cannot"),
        ],
    );
    let err = stderr_of(&out);
    assert!(
        out.status.success(),
        "not being able to look is still not a block:\n{err}"
    );
    assert!(
        err.contains("the CI reporter could not run") && err.contains(&sha[..8]),
        "a reporter that failed must be named, with the commit:\n{err}"
    );

    // Every call the hook made was the one it says it makes: the right program,
    // a lockfile it may not rewrite, and THE COMMIT IT MEANT. A report about
    // another commit parses, prints and is wrong — R1122's shape exactly.
    assert!(
        f.reporter_contract_violations().is_empty(),
        "pre-push broke the reporter contract:\n{}",
        f.reporter_contract_violations()
    );
}
