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
//!
//! EVERY STAND-IN THIS FILE PUTS ON A PATH IS A TRACKED FILE reached by symlink
//! (`tests/stubs/`), never one this process writes and then runs: `exec` refuses
//! a file some process holds open for writing, and the holder is a sibling
//! test's fork rather than this thread (Round 1192).

use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;

use crate::common::link_stub;

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

/// The stand-in for the three `cargo` commands `pre-push` issues, which CHECKS
/// THE ARGUMENTS of the one this file is about — `tests/stubs/`, with the rest
/// of the reasoning in the file itself.
const CARGO_PRE_PUSH_REPORTER: &str = "cargo-pre-push-reporter";

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
        // The stand-in for `scripts/mn`: it logs the verb it was asked for and
        // REFUSES anything the real CLI does not answer to (Round 900), so a
        // hook calling a misspelled verb fails here rather than passing.
        link_stub("mn", &f.path().join("scripts/mn"));
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

/// A tree that passes fmt and clippy and that the item-citation gate REFUSES,
/// which is the fixture Gate 5a never had.
///
/// The refusal is real rather than shimmed, and every part of it was measured:
/// `cargo clippy --workspace --all-targets -- -D warnings` exits 0 here, and
/// the gate exits 2 saying `declares helper only as a dev-dependency, and
/// cargo's check of its benches produced no library for it`. Cargo omits a
/// package's dev-dependencies from a BENCH target's documentation unit, so the
/// gate has to put them back; a dev-dependency that is a binary produces no
/// library to put back, and the gate stops rather than guessing — a wrong
/// `--extern` does not fail loudly, it resolves citations against the wrong
/// crate in silence.
///
/// WHY IT HAD TO BE THIS SHAPE. The obvious candidate — a workspace with no
/// documentable target — never reaches Gate 5a: `cargo clippy --workspace` on
/// an empty workspace exits 101, so the case would be measuring the clippy gate
/// in front of it. The other obvious one, a package that does not compile, is
/// stopped by that same clippy gate. What is left is a tree that BUILDS and
/// that the citation gate still cannot read.
const CITATION_GATE_REFUSES: &[(&str, &str)] = &[
    (
        "Cargo.toml",
        "[package]\nname = \"fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n\
         [[bench]]\nname = \"measured\"\npath = \"benches/measured.rs\"\nharness = false\n\n\
         [dev-dependencies]\nhelper = { path = \"helper\" }\n\n\
         [workspace]\nmembers = [\"helper\"]\n",
    ),
    (
        "helper/Cargo.toml",
        "[package]\nname = \"helper\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n\
         [[bin]]\nname = \"helper\"\npath = \"src/main.rs\"\n",
    ),
    ("helper/src/main.rs", "fn main() {}\n"),
    ("benches/measured.rs", "fn main() {}\n"),
];

#[test]
fn pre_commit_tells_a_citation_gate_that_could_not_read_from_one_that_found_a_defect() {
    // GATE 5a's OTHER NON-ZERO EXIT, and the one nothing had ever run. The two
    // gates BELOW it in this hook have read three codes since R1183; this one
    // read `if !` and printed `a citation names no item` for both.
    //
    // It is the recorded shape of Z15 arriving at the person committing. A
    // concurrent prune of this repository's one shared build directory left
    // `librocksdb-sys` uncompilable, the citation gate answered 2 and said so,
    // and the caller printed a finding about a citation that does not exist.
    // R1185 repaired the separate-workspace lister and left this line, because
    // the fixture that would have caught it is the one below: a tree that gets
    // past fmt and clippy and still cannot be read.
    let f = Fixture::new();
    for (path, body) in CITATION_GATE_REFUSES {
        f.write(path, body);
    }
    f.generate_lockfile("Cargo.toml");
    f.stage_all();

    let out = f.run_hook("pre-commit", &[], "", &[]);
    assert!(
        !out.status.success(),
        "a tree the citation gate could not read must not pass a commit"
    );
    let err = both_of(&out);
    assert!(
        err.contains("the item-citation gate could not read this tree (exit 2)"),
        "the rejection must say the gate could not judge, and with which code:\n{err}"
    );
    // THE MIRROR, and the whole point of this case. A hook that answered `1`
    // here sends somebody hunting for a broken citation in a tree whose
    // citations are all fine — and there is nothing in this fixture to find.
    assert!(
        !err.contains("a citation names no item"),
        "a tree it could not read is not a tree with a bad citation in it:\n{err}"
    );
    // AND THE GATE'S OWN WORDS ARE ABOVE, which is what the hook's sentence
    // promises the reader will find.
    assert!(
        err.contains("only as a dev-dependency"),
        "the gate's own message must reach the same stream the hook points at:\n{err}"
    );
}

#[test]
fn pre_commit_rejects_a_scratch_path_that_names_no_owner() {
    // GATE 5f. `temp_dir()` is the machine's directory, so a fixed name under it
    // is one path for every run — and the code that builds one removes it too,
    // which is how R1175's two overlapping runs deleted each other's fixtures.
    let f = Fixture::new();
    f.write(
        "src/lib.rs",
        "pub fn scratch() -> std::path::PathBuf {\n    \
         std::env::temp_dir().join(\"fixture-with-no-owner\")\n}\n",
    );
    f.stage_all();

    let out = f.run_hook("pre-commit", &[], "", &[]);
    assert!(
        !out.status.success(),
        "a scratch path shared by every run must not pass a commit"
    );
    let err = both_of(&out);
    assert!(
        err.contains("names no owner, so two runs share it"),
        "the rejection must say what was wrong:\n{err}"
    );
    assert!(
        err.contains("`scratch` builds a path from the shared temp root"),
        "the gate's own words must reach the same stream the hook points at:\n{err}"
    );
}

#[test]
fn pre_commit_accepts_a_scratch_path_that_names_the_process() {
    // The control. `Fixture::new`'s own tree reaches no temp root at all, so
    // without this the case above would prove only that the gate can say
    // "defect" — and the arm that has to keep working is the one where a
    // fixture path is written correctly.
    let f = Fixture::new();
    f.write(
        "src/lib.rs",
        "pub fn scratch() -> std::path::PathBuf {\n    \
         std::env::temp_dir().join(format!(\"fixture-{}\", std::process::id()))\n}\n",
    );
    f.stage_all();

    let out = f.run_hook("pre-commit", &[], "", &[]);
    let err = both_of(&out);
    assert!(
        out.status.success(),
        "a scratch path that names its owner must pass a commit:\n{err}"
    );
    assert!(
        err.contains("name the process"),
        "the gate's own verdict must reach the caller:\n{err}"
    );
}

/// A declaration whose every key the build-machine program extracts.
///
/// THE KEYS ARE THE PROGRAM'S OLDEST, not this repository's current ones: a
/// fixture that copied a live declaration would go red the day that declaration
/// was correctly re-measured, which is the defect R1188 found in the gate beside
/// the program and repaired by making the budget an argument.
const READ_DECLARATION: &str = "send = \"tracked\"\nneeds = [\"cargo\"]\npeak_gb_per_task = 2\n";

/// A `HOME` whose machine-wide program answers `--explain-declaration` with
/// fixed text, or — with `None` — one where no such program is installed.
///
/// THE PROGRAM IS BUILT RATHER THAN FOUND, and that is what makes these three
/// cases facts about the hook instead of facts about the machine running them.
/// The first draft used the installed one, passed on this workstation and failed
/// on the build machine, where it is not installed — and CI has no copy either,
/// so two of the three would have been permanent refusals reporting themselves
/// as checks. That is the shape R1188 found in the gate beside the program.
fn home_whose_program_answers(answer: Option<&str>) -> TempDir {
    let home = TempDir::new().expect("tempdir");
    if let Some(answer) = answer {
        // THE ANSWER IS DATA BESIDE THE PROGRAM, which is what lets the program
        // be a tracked file rather than one this test writes and then has the
        // hook run — `tests/stubs/bx-answering` finds it through `$0`.
        let bin = home.path().join(".claude/remote-build/bin");
        link_stub("bx-answering", &bin.join("bx"));
        fs::write(bin.join("answer"), format!("{answer}\n")).expect("the program's answer");
    }
    home
}

/// The seam's wire form for a fixture's own declaration.
fn declaration_answer(f: &Fixture, extracts: &[(&str, &str)]) -> String {
    let mut lines = vec![format!(
        "decl-file\t{}\tpresent",
        f.path().join(".claude/remote-build.toml").display()
    )];
    for (key, value) in extracts {
        lines.push(format!("decl\t{key}\t{value}"));
    }
    lines.join("\n")
}

/// `HOME` moves in every declaration case, so cargo is pointed at the real
/// registry in the same breath: the gate is COMPILED by the hook, and a cargo
/// that cannot find its own home would fail these cases one step before the
/// branch under test.
fn home_and_a_working_cargo(home: &Path) -> Vec<(String, String)> {
    let real_home = std::env::var("HOME").expect("HOME is set for the test process");
    vec![
        ("HOME".to_owned(), home.to_str().expect("utf-8").to_owned()),
        (
            "CARGO_HOME".to_owned(),
            std::env::var("CARGO_HOME").unwrap_or_else(|_| format!("{real_home}/.cargo")),
        ),
        (
            "RUSTUP_HOME".to_owned(),
            std::env::var("RUSTUP_HOME").unwrap_or_else(|_| format!("{real_home}/.rustup")),
        ),
    ]
}

fn run_pre_commit_with(f: &Fixture, home: &Path) -> Output {
    let env = home_and_a_working_cargo(home);
    let borrowed: Vec<(&str, &str)> = env
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    f.run_hook("pre-commit", &[], "", &borrowed)
}

#[test]
fn pre_commit_accepts_a_declaration_every_key_of_which_the_program_reads() {
    let f = Fixture::new();
    f.write(".claude/remote-build.toml", READ_DECLARATION);
    f.stage_all();
    let home = home_whose_program_answers(Some(&declaration_answer(
        &f,
        &[
            ("send", "tracked"),
            ("needs", "cargo"),
            ("peak_gb_per_task", "2"),
            ("min_free_gb", ""),
        ],
    )));

    let out = run_pre_commit_with(&f, home.path());
    let err = both_of(&out);
    assert!(
        out.status.success(),
        "a declaration the program reads whole must pass a commit:\n{err}"
    );
    // THE GATE MUST HAVE RUN. Without this the case passes on a hook that
    // dropped the block entirely, which is the failure this file exists for.
    assert!(
        err.contains("every top-level key this repository declares"),
        "the gate's own verdict must reach the caller:\n{err}"
    );
}

#[test]
fn pre_commit_rejects_a_declared_key_the_program_never_reads() {
    // The measured shape, with this case's own key rather than a live one:
    // `exclude` was declared by all five repositories on this machine and
    // extracted by none, and `packages` was the same before it. The program here
    // answers without the key, which is exactly what those programs did.
    let f = Fixture::new();
    f.write(
        ".claude/remote-build.toml",
        "send = \"tracked\"\nan_option_this_case_invented = 1\n",
    );
    f.stage_all();
    let home = home_whose_program_answers(Some(&declaration_answer(
        &f,
        &[("send", "tracked"), ("needs", "")],
    )));

    let out = run_pre_commit_with(&f, home.path());
    assert!(
        !out.status.success(),
        "a key the program never reads must not pass a commit"
    );
    let err = both_of(&out);
    assert!(
        err.contains("a declared key is not read as declared"),
        "the rejection must name what was wrong:\n{err}"
    );
    assert!(
        err.contains("`an_option_this_case_invented` is declared and the program never reads it"),
        "the gate's own words must reach the same stream the hook points at:\n{err}"
    );
}

#[test]
fn pre_commit_tells_a_declaration_gate_that_could_not_ask_from_one_that_found_a_defect() {
    // THE THIRD CODE, at the caller. The program that reads the declaration is
    // machine-wide and outside every checkout, so "it is not installed here" is
    // a state this gate meets in the ordinary course — and a caller that
    // collapsed it into the finding would send somebody hunting for a bad key in
    // a declaration whose keys are all fine.
    let f = Fixture::new();
    f.write(".claude/remote-build.toml", READ_DECLARATION);
    f.stage_all();
    let home = home_whose_program_answers(None);

    let out = run_pre_commit_with(&f, home.path());
    assert!(
        !out.status.success(),
        "a declaration the gate could not ask about must not pass a commit"
    );
    let err = both_of(&out);
    assert!(
        err.contains("the declaration gate could not ask the program that reads it (exit 2)"),
        "the rejection must say it could not judge, and with which code:\n{err}"
    );
    // THE MIRROR, and the whole point of this case.
    assert!(
        !err.contains("a declared key is not read as declared"),
        "a declaration it could not ask about is not one with a bad key in it:\n{err}"
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
        err.contains("UNFORMATTED tools/sub/Cargo.toml"),
        "the rejection must name the manifest to fix, and with it the separate \
         workspace it is in:\n{err}"
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
    // is where a separate workspace's tree-walking gates are run, and they too
    // print a different sentence for exit 1 and exit 2 — a branch nothing had
    // ever taken. It matters more here than in the hook: the message names the
    // WORKSPACE, so the wrong one sends somebody looking for a defect in
    // whichever crate the gate merely failed to read.
    //
    // WHICH gate answers here is a fact about the PHASE ORDER, not about this
    // fixture. An unparsable file is refused by every gate that walks the tree,
    // so the first one to run owns the sentence — it was `blind-waits` until
    // this round put the phases in measured order, and it is now
    // `unowned-scratch`. The comment this case used to carry predicted exactly
    // that ("reordering only moves the hole to whichever gate ends up second"),
    // and the hole is closed rather than moved by
    // `the_side_workspace_gate_tells_every_gate_it_could_not_read_from_one_it_judged`
    // below, which drives every arm through a shim and needs no order at
    // all. What is left HERE is the end-to-end fact that shim cannot show: a
    // real gate, refusing a real tree, through the real hook.
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
        err.contains("the scratch-ownership gate could not read tools/sub (exit 2)"),
        "the refusal must name the workspace and the code:\n{err}"
    );
    // THE MIRROR: the sentence the other exit prints, which is the one this
    // branch exists to not be.
    assert!(
        !err.contains("builds a path under the shared temp root"),
        "a workspace it could not read carries no finding about scratch paths:\n{err}"
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

/// A `cargo` that answers ONE named gate program with a chosen code and zero for
/// everything else, so any arm of the lister can be driven without compiling a
/// tree. `$GATE_UNDER_TEST` is the gate's crate directory, `$GATE_EXIT` the code.
const CARGO_ONE_GATE: &str = "cargo-one-gate";

#[test]
fn the_side_workspace_gate_tells_every_gate_it_could_not_read_from_one_it_judged() {
    // ONE ARM PER GATE, ONE SHAPE, AND NO ORDER CAN HIDE ONE. Each gate the lister
    // runs answers 0 / 1 / 2 — 2 being "I could not read enough of this tree to
    // have an opinion" — and the lister exits 1 for BOTH non-zero codes, so the
    // only thing carrying the difference is the sentence. A sentence nothing
    // reads is a sentence that drifts.
    //
    // WHY A SHIM RATHER THAN FOUR FIXTURES, and this is the whole reason the
    // case exists in this shape: the real refusal of three of these gates is
    // the SAME input, a file that will not parse, so whichever one runs first
    // refuses and the others are never reached. That was true before this round
    // with `blind-waits` first, and it is true after it with `unowned-scratch`
    // first — the phase order moves the hole, it does not close it, exactly as
    // the comment on the case below predicted. Driving the code directly closes
    // it: every arm has a reader, and no arm's reader depends on the order.
    //
    // IT WAS MEASURED, not imagined. A concurrent prune of this repository's one
    // shared build directory deleted artifacts under a running gate;
    // `librocksdb-sys` would not compile, the citation gate answered 2 and said
    // so, and the lister printed `bench carries a citation that names no item`.
    // There is no such citation in `bench`. That is the recorded shape of a
    // remote red that reads as a defect in the tree.
    let gate = repo_root().join("scripts/check-side-workspaces.sh");
    let arms = [
        (
            "item-citations",
            "the item-citation gate could not read tools/sub (exit 2)",
            "tools/sub carries a citation that names no item",
        ),
        (
            "blind-waits",
            "the blind-wait gate could not read tools/sub (exit 2)",
            "tools/sub carries a wait that ends on a clock",
        ),
        (
            "named-environment",
            "the named-environment gate could not read tools/sub (exit 2)",
            "tools/sub spawns a program whose environment its test leaves to the machine",
        ),
        (
            "unowned-scratch",
            "the scratch-ownership gate could not read tools/sub (exit 2)",
            "tools/sub builds a path under the shared temp root that names no owner",
        ),
        (
            "written-executable",
            "the written-executable gate could not read tools/sub (exit 2)",
            "tools/sub creates an executable file, which is a program it then runs",
        ),
    ];
    for (gate_crate, refusal, finding) in arms {
        let f = Fixture::new();
        let shim = f.path().join("shim-cargo");
        link_stub(CARGO_ONE_GATE, &shim.join("cargo"));
        let hermetic = format!(
            "{}:{}",
            shim.to_str().expect("shim path is utf-8"),
            std::env::var("PATH").unwrap_or_default()
        );
        let lister = |code: &str| {
            Command::new(&gate)
                .args(["--lint-only", "tools/sub"])
                .current_dir(f.path())
                .env("PATH", &hermetic)
                .env("GATE_UNDER_TEST", gate_crate)
                .env("GATE_EXIT", code)
                .env("CARGO_TARGET_DIR", f.path().join("target"))
                .output()
                .expect("the gate runs")
        };

        // EXIT 2 — the gate could not read the workspace, so it has no finding.
        let out = lister("2");
        let err = stderr_of(&out);
        assert_eq!(
            out.status.code(),
            Some(1),
            "a workspace {gate_crate} could not read must not pass:\n{err}"
        );
        assert!(
            err.contains(refusal),
            "the refusal must name the workspace and the code:\n{err}"
        );
        assert!(
            !err.contains(finding),
            "a workspace it could not read carries no finding:\n{err}"
        );

        // EXIT 1 — the same tree, judged, with a defect in it.
        let out = lister("1");
        let err = stderr_of(&out);
        assert_eq!(
            out.status.code(),
            Some(1),
            "a workspace that broke {gate_crate}'s law is a rejection too:\n{err}"
        );
        assert!(
            err.contains(finding),
            "the finding must name the workspace it is about:\n{err}"
        );
        assert!(
            !err.contains(refusal),
            "a workspace it judged was not one it failed to read:\n{err}"
        );
    }
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

#[test]
fn the_side_workspace_gate_names_the_workspace_whose_scratch_path_has_no_owner() {
    // THE FOURTH GATE'S FINDING SENTENCE, read here because the lister exits 1
    // for every arm and the sentence is the only thing carrying which one. This
    // arm is where R1193's whole population is judged: the hook's root gate
    // reaches the shared temp root nowhere.
    //
    // ITS REFUSAL SENTENCE HAS A READER NOW, and it is not this case. It used
    // to have none: this gate's only real refusal is a file that will not
    // parse, `blind-waits` and the citation gate ran before it and refused the
    // same input, and reaching it would have meant reordering the chain — which
    // only moves the hole to whichever gate ends up second. That is precisely
    // what happened when the phases were put in measured order, so the hole was
    // closed rather than moved:
    // `the_side_workspace_gate_tells_every_gate_it_could_not_read_from_one_it_judged`
    // drives every arm through a shim and depends on no order at all. What
    // this case owns is the FINDING sentence, over a real tree.
    let gate = repo_root().join("scripts/check-side-workspaces.sh");
    let f = Fixture::new();
    f.write(
        "tools/sub/src/orphan.rs",
        "pub fn scratch() -> std::path::PathBuf {\n    \
         std::env::temp_dir().join(\"fixture-with-no-owner\")\n}\n",
    );
    let out = Command::new(&gate)
        .args(["--lint-only", "tools/sub"])
        .current_dir(f.path())
        .env("CARGO_TARGET_DIR", f.path().join("target"))
        .output()
        .expect("the gate runs");
    let err = stderr_of(&out);
    assert_eq!(
        out.status.code(),
        Some(1),
        "a workspace that broke the law is a rejection:\n{err}"
    );
    assert!(
        err.contains("tools/sub builds a path under the shared temp root that names no owner"),
        "the finding must name the workspace it is about:\n{err}"
    );
    assert!(
        !err.contains("the scratch-ownership gate could not read"),
        "a workspace it judged was not one it failed to read:\n{err}"
    );
}

/// A `cargo` that compiles nothing, records every call it is given, and answers
/// "unformatted" for exactly the manifests whose path says so.
///
/// THE SUBJECT IS THE ORDER, NOT RUSTFMT. What the two cases below have to see
/// is whether an expensive check was reached AT ALL, and a fixture built to make
/// the real clippy run would be measuring cargo instead of the gate.
const CARGO_ORDER_SHIM: &str = "cargo-records-the-order";

/// Run the real gate over a throwaway tree holding one separate workspace per
/// name, in the order named, with a `cargo` that never compiles. Returns what
/// the gate said and every cargo call it made, one per line.
fn side_gate_run(workspaces: &[&str]) -> (Output, String) {
    let f = Fixture::new();
    for ws in workspaces {
        let name = ws.replace('/', "-");
        f.write(
            &format!("{ws}/Cargo.toml"),
            &format!(
                "[package]\nname = \"{name}\"\nversion = \"0.0.0\"\n\
                 edition = \"2021\"\n\n[workspace]\n"
            ),
        );
        f.write(&format!("{ws}/src/lib.rs"), CLEAN_LIB);
    }
    let shim = f.path().join("shim-cargo");
    link_stub(CARGO_ORDER_SHIM, &shim.join("cargo"));
    let calls = f.path().join("cargo-calls.log");
    let hermetic = format!(
        "{}:{}",
        shim.to_str().expect("shim path is utf-8"),
        std::env::var("PATH").unwrap_or_default()
    );
    let out = Command::new(repo_root().join("scripts/check-side-workspaces.sh"))
        .arg("--lint-only")
        .args(workspaces)
        .current_dir(f.path())
        .env("PATH", &hermetic)
        .env("SIDE_GATE_CARGO_LOG", &calls)
        .env("CARGO_TARGET_DIR", f.path().join("target"))
        .output()
        .expect("the gate runs");
    (out, fs::read_to_string(&calls).unwrap_or_default())
}

/// How many times the shim was asked to check formatting.
fn fmt_calls(calls: &str) -> usize {
    calls.lines().filter(|c| c.starts_with("fmt ")).count()
}

#[test]
fn the_side_workspace_gate_asks_the_cheap_law_everywhere_before_any_expensive_one() {
    // MEASURED, NOT FELT. `cargo fmt --check` over all 27 package manifests of
    // this repository's 19 separate workspaces costs 3.4 s and compiles
    // nothing. The checks that used to stand in front of it cost 38.5 s with
    // every artifact already built, and that is the FLOOR: a round that edited
    // code is exactly a round that makes clippy rebuild, and twice in one
    // session (R1200, R1204) the whole of a multi-minute wait bought one
    // sentence about formatting.
    //
    // So the gate is phase-major: no check starts on ANY workspace until the
    // cheaper checks have answered on EVERY workspace. The cost of the cheap
    // law does not depend on what the round changed; the cost of the one behind
    // it has no upper bound at all.
    let (out, calls) = side_gate_run(&["tools/formatted-first", "tools/unformatted-second"]);
    let said = both_of(&out);
    assert_eq!(
        out.status.code(),
        Some(1),
        "an unformatted workspace is a rejection:\n{said}"
    );
    assert!(
        said.contains("tools/unformatted-second"),
        "the refusal must name the workspace it is about:\n{said}"
    );
    // NON-VACUITY: the cheap law was asked of BOTH, so the absence below is an
    // ordering fact and not a gate that stopped early.
    assert_eq!(
        fmt_calls(&calls),
        2,
        "the cheap law must be asked of every workspace, not only the one that \
         failed:\n{calls}"
    );
    assert!(
        !calls.contains("clippy"),
        "an expensive check started somewhere while a cheap one still had an \
         answer to give:\n{calls}"
    );
    // AND THE GATE SAYS SO ITSELF. What one law cost over the whole population
    // is the number the phase order was chosen by, and a number nobody reads
    // drifts — this is that reader, and it pins the POPULATION too: a phase
    // line claiming fewer workspaces than the run has is a law that quietly
    // stopped covering them.
    assert!(
        said.contains("PHASE fmt over 2 workspace(s)"),
        "the gate must report what the cheap law cost over every workspace \
         it covered:\n{said}"
    );
}

#[test]
fn the_side_workspace_gate_names_every_unformatted_manifest_in_one_run() {
    // THE SCRIPT'S OWN DOCTRINE, APPLIED TO THE CHECK IT HAD NEVER APPLIED IT
    // TO. `--no-fail-fast` is on the suite because "a gate that stops at the
    // first failing target reports a smaller number than the truth and somebody
    // fixes to it" — measured on this gate's first run, which reported 6
    // failures in `bench` when there were 18. The formatting check stopped at
    // the first unformatted manifest, so a tree with three of them took three
    // runs to learn that.
    //
    // THIS PHASE CAN AFFORD TOTALITY AND THE ONES BEHIND IT CANNOT, and the
    // difference is the measurement above: finishing it costs 3.4 s over the
    // whole repository, while finishing clippy after a failure costs minutes.
    let (out, calls) = side_gate_run(&["tools/unformatted-first", "tools/unformatted-second"]);
    let said = both_of(&out);
    assert_eq!(
        out.status.code(),
        Some(1),
        "an unformatted workspace is a rejection:\n{said}"
    );
    assert!(
        said.contains("tools/unformatted-first/Cargo.toml"),
        "the first unformatted manifest must be named:\n{said}"
    );
    assert!(
        said.contains("tools/unformatted-second/Cargo.toml"),
        "and so must the second, in the SAME run:\n{said}"
    );
    assert_eq!(
        fmt_calls(&calls),
        2,
        "both had to be asked for both to be named:\n{calls}"
    );
}

/// A `grep` that has no PCRE at all — the reading the first repair of this
/// defect guessed at, kept because it is the OTHER way the capability can be
/// missing and the hook must answer for both.
const GREP_WITHOUT_PCRE: &str = "grep-without-pcre";

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
    link_stub(GREP_WITHOUT_PCRE, &shim.join("grep"));
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
        err.contains("UNFORMATTED tools/sub/Cargo.toml"),
        "the block must name the manifest to fix, and with it the separate \
         workspace it is in:\n{err}"
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
    link_stub(CARGO_PRE_PUSH_REPORTER, &shim.join("cargo"));
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
