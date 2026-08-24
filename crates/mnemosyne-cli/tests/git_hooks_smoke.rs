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

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use ci_plan::issue::{self, Tree};
use tempfile::TempDir;

use crate::common::link_stub;

/// Formatted, lint-clean, and free of any version-postfix identifier — the
/// baseline every gate must accept.
///
/// AND IT HOLDS A TEST (R1230), for the reason the lockfile, the separate
/// workspace and the workflow above it are here: `pre-push` now runs
/// `tools/unrun-tests`, whose whole job is to refuse a tree it could not read a
/// population out of. A crate with no test at all is such a tree — the gate
/// answers "not one workspace holds a test", correctly — so a fixture without
/// one is a tree the hooks rightly refuse, and the refusal would be about the
/// fixture rather than about the case.
const CLEAN_LIB: &str = "pub fn one() -> u8 {\n    1\n}\n\
                         \n\
                         #[cfg(test)]\n\
                         mod tests {\n\
                         \x20   #[test]\n\
                         \x20   fn one_is_one() {\n\
                         \x20       assert_eq!(super::one(), 1);\n\
                         \x20   }\n\
                         }\n";

/// The stand-in for the workspace lister, IN THE FIXTURE TREE — the file itself
/// carries why a fixture needs one and why the real script cannot be it.
const SIDE_WORKSPACE_LISTER: &str = "side-workspace-lister";

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
        // AND A REPOSITORY ANSWERS WHICH WORKSPACES IT HAS. Since R1230
        // `pre-push` runs `tools/unrun-tests`, and that gate asks the lister OF
        // THE TREE IT IS JUDGING for its population — the real script is gate
        // 4's, resolved from the hook's checkout, which is a different call.
        link_stub(
            SIDE_WORKSPACE_LISTER,
            &f.path().join("scripts/check-side-workspaces.sh"),
        );
        // A REPOSITORY HAS A WORKFLOW. Since R1210 `pre-commit` holds what CI
        // installs against what the build-machine declaration names, and the
        // population comes from `git ls-files .github/workflows`: a tree with
        // none is one the shared loader refuses, by the same non-vacuity rule
        // that made the lockfile and the separate workspace baselines above.
        //
        // IT INSTALLS NOTHING, which is a reading rather than a refusal — the
        // witness looked and this runner needed nothing added — and it is the
        // one shape that keeps the baseline out of the gate's subject. A
        // fixture that installed something would need a declaration naming it,
        // and a declaration in the baseline would send Gate 5e to the real
        // machine-wide program in every case here, which is exactly what R1189
        // built the stub to avoid.
        f.write(
            ".github/workflows/ci.yml",
            "jobs:\n  build:\n    steps:\n      - run: cargo test --workspace --locked\n",
        );
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
        let out = issue::cargo(Tree::MadeByThisRun(
            "a fixture repository this test wrote a moment ago, whose lockfile \
             is being created here for the first time",
        ))
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

/// WHAT LEFT THE COMMIT PATH IS RUN SOMEWHERE, AND THIS ASKS THE WORKFLOW
/// RATHER THAN THE COMMENT THAT SAYS SO.
///
/// R1284 took three gates that compile or document the whole tree off the commit
/// path because the hook could not finish a commit on a contended workstation.
/// Two of them were sent to the hosted workflow, and the sentence recording that
/// is prose in `.githooks/pre-commit` — the exact shape this repository has been
/// caught by before, most recently in R1276, where a design decision rested on a
/// claim about a gate that nobody had asked the machine about.
///
/// SO BOTH HALVES ARE ASKED. The workflow must declare a job that RUNS something
/// for each of them — a job with no `run:` step declares no work — and the hook
/// must not still be running them, or the move would be a sentence rather than a
/// change. The second half is what makes this a law about placement instead of a
/// list of job names.
#[test]
fn every_gate_the_commit_hook_stopped_running_is_one_the_hosted_workflow_runs() {
    let root = repo_root();
    let workflow = "mnemosyne-validate.yml";
    let raw = fs::read_to_string(root.join(".github/workflows").join(workflow))
        .expect("the workflow this repository pushes to");
    let doc = ci_plan::parse_workflow(&raw, workflow);
    let jobs: std::collections::BTreeSet<String> = ci_plan::run_steps(&doc)
        .into_iter()
        .map(|step| step.job)
        .collect();
    println!("[hooks] {workflow} declares work in {jobs:?}");

    let hook = fs::read_to_string(root.join(".githooks/pre-commit")).expect("the commit hook");
    // COMMENTS ARE PROSE ABOUT A GATE AND NOT ONE, and this hook's own record of
    // where each gate went names both of them.
    let runs: String = hook
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");

    for (gate, job) in [
        ("item-citations", "item-citations"),
        ("check-side-workspaces.sh", "side-workspaces"),
    ] {
        assert!(
            jobs.contains(job),
            "`{gate}` left the commit hook for the hosted `{job}` job, and \
             {workflow} declares work in {jobs:?} — a gate that left one place \
             for nowhere is not a move"
        );
        assert!(
            !runs.contains(gate),
            "`{gate}` is named in the hosted workflow AND still invoked by the \
             commit hook, so the sentence saying it moved is wrong about the \
             file it is written in"
        );
    }
}

// R1287 — TWO CASES ABOUT THE PUSH HOOK'S WORKSPACE CLIPPY GATE STOOD HERE AND
// LEFT WITH IT. `pre_push_rejects_lint_dirty_rust` (R1284) asserted the gate
// stopped a lint-dirty push; `pre_push_tells_a_wrapper_that_could_not_place_the_gate_from_a_lint_finding`
// (R1285) asserted it told a build machine's REFUSAL apart from a finding, a
// defect measured in the wild the day the routing landed. The gate is now a step
// in the `validate` job — `every_compiling_gate_a_git_hook_runs_is_one_a_hosted_job_runs`
// is what says so and would go red if it were deleted instead of moved — and a
// law asserting a branch that cannot run is scenery, which this file has a
// standing rule against.
//
// THE R1285 DISTINCTION IS NOT RETIRED WITH ITS CASE. `$BX` appeared in no other
// hook, so nothing in this file can still ask it; N232 is the uncounted
// population of every OTHER reader in this repository of a wrapper's exit status,
// and that census is where the law goes next. Written here because a deleted test
// leaves no trace of what it used to prove.

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
/// UNDER A SEPARATE WORKSPACE SINCE R1284, because that is where the citation
/// gate is still run locally: the commit hook no longer calls it at all, and
/// `check-side-workspaces.sh` — which `pre-push` runs whole — has a `citations`
/// phase per separate workspace. The root workspace's own citations are the
/// hosted job's, and that is the widened window this round wrote down.
const CITATION_GATE_REFUSES: &[(&str, &str)] = &[
    (
        "tools/cited/Cargo.toml",
        "[package]\nname = \"fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n\
         [[bench]]\nname = \"measured\"\npath = \"benches/measured.rs\"\nharness = false\n\n\
         [dev-dependencies]\nhelper = { path = \"helper\" }\n\n\
         [workspace]\nmembers = [\"helper\"]\n",
    ),
    (
        "tools/cited/helper/Cargo.toml",
        "[package]\nname = \"helper\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n\
         [[bin]]\nname = \"helper\"\npath = \"src/main.rs\"\n",
    ),
    ("tools/cited/helper/src/main.rs", "fn main() {}\n"),
    ("tools/cited/benches/measured.rs", "fn main() {}\n"),
];

#[test]
fn pre_push_tells_a_citation_gate_that_could_not_read_from_one_that_found_a_defect() {
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
    f.generate_lockfile("tools/cited/Cargo.toml");
    f.stage_all();
    f.git(&["commit", "--no-verify", "-q", "-m", "test(fixture): cited"]);
    let sha = head_sha(&f);

    let out = f.run_hook(
        "pre-push",
        &["origin", "git@example:x"],
        &push_line(&sha),
        &[],
    );
    assert!(
        !out.status.success(),
        "a tree the citation gate could not read must not pass a push"
    );
    let err = both_of(&out);
    assert!(
        err.contains("the item-citation gate could not read tools/cited (exit 2)"),
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

/// A workflow that installs one package, the way this repository's own does.
const WORKFLOW_INSTALLING: &str = "jobs:\n  build:\n    steps:\n      \
                                   - run: sudo apt-get update && sudo apt-get install -y jq\n";

/// The stub program's answer for a declaration that names `jq`.
///
/// Gate 5e runs BEFORE 5h and would reject first if the program disagreed with
/// the file, so a case about what CI installs has to keep the other declaration
/// gate satisfied — with the built stub rather than the installed program, for
/// the reason the cases above use it.
fn answers_for(f: &Fixture, packages: &str) -> TempDir {
    home_whose_program_answers(Some(&declaration_answer(
        f,
        &[("send", "tracked"), ("packages", packages), ("needs", "")],
    )))
}

#[test]
fn pre_commit_accepts_a_workflow_whose_installs_the_declaration_names() {
    let f = Fixture::new();
    f.write(".github/workflows/ci.yml", WORKFLOW_INSTALLING);
    f.write(
        ".claude/remote-build.toml",
        "send = \"tracked\"\npackages = [\"jq\"]\n",
    );
    f.stage_all();
    let home = answers_for(&f, "jq");

    let out = run_pre_commit_with(&f, home.path());
    let err = both_of(&out);
    assert!(
        out.status.success(),
        "a package CI installs and the declaration names must pass a commit:\n{err}"
    );
    // THE GATE MUST HAVE RUN. Without this the case passes on a hook that
    // dropped the block entirely, which is the failure this file exists for.
    assert!(
        err.contains("every package this repository's CI installs is one the build-machine"),
        "the gate's own verdict must reach the caller:\n{err}"
    );
}

#[test]
fn pre_commit_rejects_a_package_ci_installs_that_the_declaration_never_names() {
    // THE MEASURED SHAPE. `protobuf-compiler` was installed by six jobs of this
    // repository's workflow from 2026-07-31 and named in the declaration from
    // 2026-08-14; in between, a build host was chosen without it and compiled
    // 269 crates before saying so.
    let f = Fixture::new();
    f.write(".github/workflows/ci.yml", WORKFLOW_INSTALLING);
    f.write(
        ".claude/remote-build.toml",
        "send = \"tracked\"\npackages = [\"libclang-common-18-dev\"]\n",
    );
    f.stage_all();
    let home = answers_for(&f, "libclang-common-18-dev");

    let out = run_pre_commit_with(&f, home.path());
    assert!(
        !out.status.success(),
        "a package the far side is never told about must not pass a commit"
    );
    let err = both_of(&out);
    assert!(
        err.contains("CI installs something the build-machine declaration does not name"),
        "the rejection must name what was wrong:\n{err}"
    );
    assert!(
        err.contains("CI installs `jq` and the build-machine declaration names it nowhere"),
        "the gate's own words must reach the same stream the hook points at:\n{err}"
    );
}

#[test]
fn pre_commit_tells_a_requirement_gate_that_could_not_judge_from_one_that_found_a_defect() {
    // THE THIRD CODE, at the caller. An action installs onto the runner without
    // writing a package name anywhere this law can read, so "there is a
    // requirement here and I cannot say what it is" is a state this gate meets
    // in the ordinary course — and a caller that collapsed it into the finding
    // would send somebody hunting for a package nobody named.
    let f = Fixture::new();
    f.write(
        ".github/workflows/ci.yml",
        "jobs:\n  build:\n    steps:\n      - uses: arduino/setup-protoc@v3\n      \
         - run: sudo apt-get install -y jq\n",
    );
    f.write(
        ".claude/remote-build.toml",
        "send = \"tracked\"\npackages = [\"jq\"]\n",
    );
    f.stage_all();
    let home = answers_for(&f, "jq");

    let out = run_pre_commit_with(&f, home.path());
    assert!(
        !out.status.success(),
        "an install this law cannot read must not pass a commit"
    );
    let err = both_of(&out);
    assert!(
        err.contains("the requirement gate could not judge what CI installs (exit 2)"),
        "the rejection must say it could not judge, and with which code:\n{err}"
    );
    // THE MIRROR, and the whole point of this case.
    assert!(
        !err.contains("CI installs something the build-machine declaration does not name"),
        "a population it could not read is not a package nobody declared:\n{err}"
    );
}

// THE WALK-UP GATE IS GONE AND SO IS ITS LAW (R1284). A `.rs` staged inside a
// separate workspace used to make the commit hook walk up to that workspace's
// root and run the lint half of `check-side-workspaces.sh` there. What replaced
// it is not a weaker check but a wider one: `pre_push_gates_on_every_separate_
// workspace_and_names_the_one_that_fails` below runs that script WHOLE over
// EVERY separate workspace, not the ones a particular commit happened to touch,
// and the hosted `separate in-repo workspaces` job does the same. A law asserting
// the walk-up would now be asserting a mechanism that does not exist, which is
// worse than no law: it goes red for the right reason, gets deleted, and takes
// the coverage question with it.

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
    // THROUGH THE PUSH HOOK SINCE R1284, which is where the script runs now —
    // and it runs WHOLE there rather than `--lint-only`, so this case reaches
    // the same phase by a shorter argument than it used to.
    f.git(&["commit", "--no-verify", "-q", "-m", "test(fixture): sub"]);
    let sha = head_sha(&f);

    let out = f.run_hook(
        "pre-push",
        &["origin", "git@example:x"],
        &push_line(&sha),
        &[],
    );
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

/// `--ours-only` DROPS EXACTLY THE WORKSPACES THIS REPOSITORY DOES NOT OWN, AND
/// THE PUSH HOOK IS WHY IT EXISTS (Round 1225).
///
/// `pre-push` asks whether the commit about to be published breaks THIS
/// repository. `studio` path-depends on the sibling `pinion` checkout, so its
/// gate also answers whether somebody else's working tree compiles right now —
/// and on 2026-08-16 it did not, so three finished rounds could not be pushed
/// for a reason nothing here could fix.
///
/// TWO FACTS, EACH EXECUTED, because a flag that is passed and does nothing and
/// a flag that works but is never passed fail the same way from a distance:
/// the gate's `--list` answers differently with and without the flag, and the
/// hook's own text carries it. `--list` rather than a full run because what is
/// under test is WHICH workspaces are chosen, and choosing is what `--list`
/// prints — a full run would spend minutes compiling to re-answer it.
///
/// THE POPULATION IS NOT WRITTEN DOWN HERE EITHER. The test does not name
/// `studio`; it asks the gate which workspaces it calls foreign and requires
/// the flag to drop those and keep the rest. A second sibling-dependent
/// workspace changes both answers together.
#[test]
fn ours_only_drops_the_workspaces_this_repository_does_not_own() {
    let gate = repo_root().join("scripts/check-side-workspaces.sh");
    let list = |flags: &[&str]| -> String {
        let out = Command::new(&gate)
            .args(flags)
            .current_dir(repo_root())
            .output()
            .expect("the gate runs");
        assert!(
            out.status.success(),
            "the lister failed with {flags:?}:\n{}",
            stderr_of(&out)
        );
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    let named = |text: &str, marker: &str| -> Vec<String> {
        text.lines()
            .filter_map(|line| line.strip_prefix(&format!("[side-workspaces] {marker} ")))
            .map(|rest| {
                rest.split_whitespace()
                    .next()
                    .unwrap_or_default()
                    .to_string()
            })
            .filter(|name| !name.is_empty())
            .collect()
    };

    let whole = list(&["--list"]);
    // The gate's OWN answer about ownership, read rather than restated.
    let foreign: Vec<String> = whole
        .lines()
        .filter_map(|line| line.strip_prefix("[side-workspaces] LOCK "))
        .filter_map(|rest| {
            let mut parts = rest.split_whitespace();
            let name = parts.next()?;
            (parts.next()? == "foreign").then(|| name.to_string())
        })
        .collect();
    assert!(
        !foreign.is_empty(),
        "no workspace in this tree resolves against a foreign one, so this flag \
         has nothing to drop and the case proves nothing. If that is now true, \
         the flag and this test are both dead and should go: {whole}"
    );
    // TWO ANSWERS THE GATE KEEPS APART AND THIS CASE MUST TOO (Round 1227).
    // "not ours" and "not on this machine" are different facts — Round 1115
    // separated them and the gate says so in two different SKIP lines. A
    // workspace can be BOTH, and on a CI runner `studio` is: it resolves
    // against the sibling checkout AND that checkout is not there. The plain
    // lister already drops it for the second reason, so there is nothing left
    // for `--ours-only` to drop and nothing this machine can learn about the
    // flag from it. Round 1225 asserted every foreign workspace was offered by
    // the plain lister, which is true here and false on a runner, and CI said so
    // — a claim about the machine asking, which is the shape this repository
    // has now paid for several times.
    let checkable = named(&whole, "CHECKABLE");
    let droppable: Vec<&String> = foreign.iter().filter(|f| checkable.contains(f)).collect();

    let ours = list(&["--list", "--ours-only"]);
    let kept = named(&ours, "CHECKABLE");
    let expected: Vec<&String> = checkable.iter().filter(|c| !foreign.contains(c)).collect();
    assert_eq!(
        kept.iter().collect::<Vec<_>>(),
        expected,
        "`--ours-only` changed the set by more than the foreign workspaces — it \
         must drop those and NOTHING else"
    );

    if droppable.is_empty() {
        // THE THIRD STATE, SAID RATHER THAN PASSED OVER. This machine holds no
        // foreign checkout, so the flag has nothing to drop here and the
        // assertion above only witnesses that it is INERT when there is nothing
        // to drop — which is a real claim, and not the one the flag exists for.
        // The drop itself is exercised where the sibling checkout lives, which
        // is the same machine the gate can check `studio` on at all.
        println!(
            "[ours-only] this machine holds no foreign workspace the plain lister \
             offers, so the DROP is unexercised here; what is checked is that the \
             flag changes nothing when there is nothing to change. Foreign: \
             {foreign:?}"
        );
        return;
    }
    for name in droppable {
        assert!(
            !kept.contains(name),
            "`--ours-only` still checks `{name}`, which resolves against a tree \
             this repository does not own"
        );
        // The skip is LOUD: a caller must be able to tell "checked everything"
        // from "checked what it could", which is the whole reason the closing
        // line names what it skipped.
        assert!(
            ours.contains(&format!("SKIP {name} — --ours-only")),
            "`--ours-only` dropped `{name}` without saying so; a green run that \
             quietly checked less is the failure this gate exists to prevent"
        );
    }

    // AND THE HOOK PASSES IT. Without this half the flag could be correct and
    // unreached; with only this half it could be passed and inert.
    let hook = std::fs::read_to_string(repo_root().join(".githooks/pre-push"))
        .expect("the pre-push hook is tracked");
    assert!(
        hook.contains("\"$side_gate\" --ours-only"),
        "`pre-push` does not run the side gate with `--ours-only`, so a sibling \
         repository's working tree can still block a push from this one"
    );
}

/// WHERE THE SIBLING IS NOT THERE, `--ours-only` CHANGES NOTHING — AND THAT IS
/// THE BRANCH CI RUNS (Round 1227).
///
/// Round 1225's case asserted every FOREIGN workspace was one the plain lister
/// offered to check. That is true on a machine holding the sibling checkout and
/// false on a runner, where the gate has already skipped it for the OTHER
/// reason — "not on this machine" — which Round 1115 separated from "not ours"
/// precisely because they are different facts. CI is what said so, one push
/// after the flag landed.
///
/// The repaired case above takes whichever branch its machine is on, and BOTH
/// machines this repository builds on hold the sibling — so the absent branch
/// would ship unexercised, which is the shape that just cost a red. This runs
/// the gate over a tree that HAS a foreign workspace and NOT the tree it points
/// at, which is the runner's condition made local and deterministic.
#[test]
fn ours_only_is_inert_where_the_foreign_tree_is_not_on_this_machine() {
    let gate = repo_root().join("scripts/check-side-workspaces.sh");
    let tmp = TempDir::new().expect("tempdir");
    // A root so the gate agrees it was started in a tree, and one separate
    // workspace whose path dependency leaves it — pointing at a sibling that
    // does not exist, the way `studio` points at one that does not exist on a
    // runner.
    std::fs::write(
        tmp.path().join("Cargo.toml"),
        "[package]\nname = \"fixture-root\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[workspace]\n",
    )
    .expect("root manifest");
    std::fs::create_dir_all(tmp.path().join("src")).expect("src");
    std::fs::write(tmp.path().join("src/lib.rs"), "").expect("root lib");
    std::fs::create_dir_all(tmp.path().join("viewer/src")).expect("viewer");
    std::fs::write(
        tmp.path().join("viewer/Cargo.toml"),
        "[workspace]\n\n[package]\nname = \"viewer\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n\
         [dependencies]\nabsent-sibling = { path = \"../../not-here/crates/absent-sibling\" }\n",
    )
    .expect("viewer manifest");
    std::fs::write(tmp.path().join("viewer/src/lib.rs"), "").expect("viewer lib");
    // A SECOND WORKSPACE THAT IS OURS, because the gate refuses a run that
    // reached nothing — "a green run that checked nothing is the failure it
    // exists to prevent" — and with only the absent one there is nothing left.
    // It is also what makes the comparison below mean something: the flag must
    // leave THIS one alone while the other is already gone.
    std::fs::create_dir_all(tmp.path().join("tool/src")).expect("tool");
    std::fs::write(
        tmp.path().join("tool/Cargo.toml"),
        "[workspace]\n\n[package]\nname = \"tool\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    )
    .expect("tool manifest");
    std::fs::write(tmp.path().join("tool/src/lib.rs"), "").expect("tool lib");

    let list = |flags: &[&str]| -> String {
        let out = Command::new(&gate)
            .args(flags)
            .current_dir(tmp.path())
            .output()
            .expect("the gate runs");
        assert!(
            out.status.success(),
            "the lister failed with {flags:?}:\n{}",
            stderr_of(&out)
        );
        String::from_utf8_lossy(&out.stdout).into_owned()
    };

    let whole = list(&["--list"]);
    // THE GATE ALREADY DROPS IT, for absence rather than for ownership — and it
    // says which, because the two sentences are different.
    assert!(
        whole.contains("SKIP viewer — its path dependencies leave this repository"),
        "the gate did not skip a workspace pointing at a tree that is not here, \
         so this fixture is not the runner's condition:\n{whole}"
    );
    assert!(
        whole.contains("LOCK viewer foreign"),
        "the gate did not call it foreign, so the ownership answer this flag \
         consumes is not being produced:\n{whole}"
    );
    let ours = list(&["--list", "--ours-only"]);
    assert!(
        !ours.contains("SKIP viewer — --ours-only"),
        "`--ours-only` claimed the drop for a workspace the gate had already \
         skipped for absence — two reasons for one skip, and a reader cannot \
         tell which machine it is on:\n{ours}"
    );
    // INERT, not merely quiet: the set of workspaces offered is identical, so a
    // runner's verdict is the same with the flag as without it.
    let offered = |text: &str| -> Vec<String> {
        text.lines()
            .filter(|l| l.starts_with("[side-workspaces] CHECKABLE "))
            .map(|l| l.to_string())
            .collect()
    };
    assert_eq!(
        offered(&ours),
        offered(&whole),
        "`--ours-only` changed what a machine WITHOUT the sibling checks; there \
         is nothing there for it to drop, so it must change nothing"
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
const GIT_WITHOUT_A_STAGED_LIST: &str = "git-that-cannot-list-what-is-staged";
const GIT_WITHOUT_A_STAGED_DIFF: &str = "git-that-cannot-read-the-staged-diff";

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
    let err = stderr_of(&out);
    // THE HOOK'S OWN WORDS ARE THE EVIDENCE AND THIS ASSERTION USED TO THROW
    // THEM AWAY (R1280). It went red once inside the full suite and passed alone,
    // and there was nothing in the failure to say WHY the hook had accepted —
    // whether Gate 6 ran and found nothing, or the gates never ran because the
    // staged list came back empty. The hook prints a line per gate it enters, so
    // the answer is in what it said.
    assert!(
        !out.status.success(),
        "a vN identifier must be rejected, and the hook accepted it — what it \
         printed says which gates it entered:\n{err}"
    );
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

/// A COMMIT GATE THAT CANNOT SEE WHAT IS STAGED MUST NOT ACCEPT.
///
/// R1281, and the defect it closes is a single `|| true`. Four of this hook's
/// gates are keyed on the staged file list, and that list was read with a clause
/// that turned a FAILED read into an EMPTY one — so an unreadable index looked
/// exactly like a repository with nothing staged, every one of those gates found
/// nothing to check, and the hook exited 0 on a commit no rule had been applied
/// to. `git diff --cached --name-only` returns non-zero only on error (an empty
/// index is a zero exit with no output), so that clause could never have masked
/// anything but the failure it was masking.
///
/// THE SHAPE IS THIS REPOSITORY'S OLDEST ONE and it has now been found in both
/// hooks: `commit-msg` ended its Unicode rules in `2>/dev/null`, which made
/// every way of going wrong look like "no match", and it took a second machine
/// to see it. Here the stand-in is what makes the condition reproducible on this
/// one — narrow on purpose, because a `git` that failed at everything would abort
/// the hook under `set -e` and a non-zero exit would prove nothing.
#[test]
fn pre_commit_refuses_when_it_cannot_read_what_is_staged() {
    let f = Fixture::new();
    // The same banned identifier the case above uses, assembled rather than
    // written for the same reason: Gate 6 scans this file too.
    let banned = format!("parse_{}{}", 'v', 2);
    f.write(
        "src/lib.rs",
        &format!("pub fn {banned}() -> u8 {{\n    2\n}}\n"),
    );
    f.stage_all();

    let shim = f.path().join("shim-blind-git");
    link_stub(GIT_WITHOUT_A_STAGED_LIST, &shim.join("git"));
    let hobbled = format!(
        "{}:{}",
        shim.to_str().expect("shim path is utf-8"),
        std::env::var("PATH").unwrap_or_default()
    );
    let out = f.run_hook("pre-commit", &[], "", &[("PATH", &hobbled)]);
    let err = stderr_of(&out);
    assert!(
        !out.status.success(),
        "the staged list could not be read, so no gate below it ran — and a hook \
         that accepts because it could not look is worse than no hook, because \
         it reports a clean answer:\n{err}"
    );
    assert!(
        err.contains("the staged list could not be read"),
        "and it must say what it could not do rather than merely fail, so the \
         next reader is not left deciding between an empty index and a broken \
         one:\n{err}"
    );

    // AND THE SAME QUESTION ONE GATE DEEPER (R1283). R1281 repaired the line it
    // was looking at; nine lines below it the vN ban read the staged DIFF
    // through a `|| true` that had to be there — `grep` answers 1 when it finds
    // nothing, which is the ordinary case — and under `set -o pipefail` that one
    // clause also swallowed a failed `git diff`, leaving the search empty and the
    // gate passing. One clause cannot tell "no hits" from "could not look", and
    // a census of this repository's shell found this was the only one of eight
    // such clauses that could not.
    let deeper = f.path().join("shim-blind-diff");
    link_stub(GIT_WITHOUT_A_STAGED_DIFF, &deeper.join("git"));
    let hobbled = format!(
        "{}:{}",
        deeper.to_str().expect("shim path is utf-8"),
        std::env::var("PATH").unwrap_or_default()
    );
    let out = f.run_hook("pre-commit", &[], "", &[("PATH", &hobbled)]);
    let err = stderr_of(&out);
    assert!(
        !out.status.success() && err.contains("the staged diff could not be read"),
        "the vN ban is a search over the staged diff, and a diff it could not \
         read is not one with no hits in it:\n{err}"
    );

    // THE CONTROL, and it is the half that keeps the assertions above from being
    // satisfied by a hook that refuses everything: the same fixture through an
    // unhobbled hook is rejected BY GATE 6, naming the identifier. Refusing for
    // the right reason and refusing for any reason are different facts.
    let out = f.run_hook("pre-commit", &[], "", &[]);
    let err = stderr_of(&out);
    assert!(
        !out.status.success() && err.contains("vN version-postfix identifier"),
        "with a git that can answer, the same tree is refused by the gate whose \
         subject it is:\n{err}"
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

/// A tree whose only test outside the baseline is one nothing runs.
///
/// `#[ignore]` rather than a feature or a filter, because it is the cheapest
/// dark test there is: `--list` prints it, `--list --ignored` prints it too, and
/// a command that passes neither runs its list minus its ignored list. The name
/// is read back by the assertions, so a fixture that stopped holding it could
/// not pass by spelling the name a second time.
const DARK_LIB: &str = "pub fn one() -> u8 {\n    1\n}\n\
                        \n\
                        #[cfg(test)]\n\
                        mod tests {\n\
                        \x20   #[test]\n\
                        \x20   fn one_is_one() {\n\
                        \x20       assert_eq!(super::one(), 1);\n\
                        \x20   }\n\
                        \n\
                        \x20   #[test]\n\
                        \x20   #[ignore]\n\
                        \x20   fn nothing_runs_this() {}\n\
                        }\n";

const DARK_TEST: &str = "tests::nothing_runs_this";

/// A PUSH DOES NOT PUBLISH A TEST NOTHING RUNS (Round 1230).
///
/// `tools/unrun-tests` had run on CI and nowhere else since it was written, and
/// the cost of that is on the record rather than argued: R1193 wrote a
/// ```` ```ignore ```` fence, which makes a doc-test no command in this
/// repository runs; every local gate was green, the push went out, and the
/// runner turned `origin/main` red. R1195 paid it. This case is the reader that
/// branch was missing — the same repair R1156 made for the separate workspaces'
/// suites and R890 for CI state.
///
/// THREE ARMS, because this gate answers THREE things and a hook that read two
/// of them would publish the third as a pass:
///   - a dark test is a refusal, and the test is named,
///   - a gate that could not judge is a refusal too, and says which it was,
///   - and the mirror: the same tree whose CI DOES run that test is pushed, with
///     the gate's own clean sentence in the output — which is what stops the
///     first arm from holding for a hook that rejects every push.
///
/// The three trees differ in ONE line each — the workflow's command, and the
/// lister's mode — so the verdict is attributable to it.
#[test]
fn pre_push_refuses_a_tree_that_compiles_a_test_no_ci_command_runs() {
    let dark = Fixture::new();
    dark.write("src/lib.rs", DARK_LIB);
    dark.stage_all();
    dark.git(&["commit", "--no-verify", "-q", "-m", "test(fixture): seed"]);
    let out = dark.run_hook(
        "pre-push",
        &["origin", "git@example:x"],
        &push_line(&head_sha(&dark)),
        &[],
    );
    let both = both_of(&out);
    assert!(
        !out.status.success(),
        "a test this push would publish and nothing runs must block it:\n{both}"
    );
    assert!(
        both.contains(DARK_TEST),
        "and the block is worth nothing without the test's name:\n{both}"
    );
    assert!(
        both.contains("compiles a test no CI command runs"),
        "and it must be this hook's own refusal, not a message from some other \
         gate the push happened to trip:\n{both}"
    );

    // THE SECOND ANSWER. A gate that could not read the tree has not found it
    // clean, and the two look identical in an exit code alone — which is the
    // failure `tools/unrun-tests` was given a third code for.
    let unjudged = Fixture::new();
    unjudged.git(&["commit", "--no-verify", "-q", "-m", "test(fixture): seed"]);
    let out = unjudged.run_hook(
        "pre-push",
        &["origin", "git@example:x"],
        &push_line(&head_sha(&unjudged)),
        &[("LISTER_MODE", "offers-what-is-not-there")],
    );
    let both = both_of(&out);
    assert!(
        !out.status.success(),
        "a gate that could not judge is not a gate that passed:\n{both}"
    );
    assert!(
        both.contains("could not judge this tree (exit 2)"),
        "and the hook says WHICH of the two refusals it is, with the code:\n{both}"
    );
    assert!(
        !both.contains("compiles a test no CI command runs"),
        "the two answers must not be reported as each other:\n{both}"
    );

    // THE MIRROR. The same dark tree, and one CI command that selects the
    // ignored tests — so nothing is dark and the push goes.
    let covered = Fixture::new();
    covered.write("src/lib.rs", DARK_LIB);
    covered.write(
        ".github/workflows/ci.yml",
        "jobs:\n  build:\n    steps:\n      - run: cargo test --workspace --locked\n\
         \x20     - run: cargo test --workspace --locked -- --ignored\n",
    );
    covered.stage_all();
    covered.git(&["commit", "--no-verify", "-q", "-m", "test(fixture): seed"]);
    let out = covered.run_hook(
        "pre-push",
        &["origin", "git@example:x"],
        &push_line(&head_sha(&covered)),
        &[],
    );
    let both = both_of(&out);
    assert!(
        out.status.success(),
        "a tree whose CI runs every test it compiles must be pushable:\n{both}"
    );
    assert!(
        both.contains("every test this repository compiles is run by CI"),
        "and the gate must be SEEN to have run — a pass that skipped it looks \
         the same from here:\n{both}"
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

/// The census of this tree is put on a machine that is not this one, and a fleet
/// that cannot take it does not stop the work (R1234).
///
/// WHAT THIS CASE OWNS IS THE CALL AND THE CARRIAGE, the same division the CI
/// reporter's case draws: whether a dispatch is legitimate, whether an answer is
/// stale, and whether the machine that answered was this one are all decided in
/// `tools/one-machine`, against stub placement programs and stub transports, and
/// nothing here re-asks them. What lives HERE is that the hook calls that
/// program with the tree it means, prints whatever it says, and does not turn a
/// busy build machine into a commit nobody can make.
///
/// The fixture declares no `[commands] verify`, so the program refuses — which
/// is the state this gate must survive, not an accident of the fixture. A
/// repository that says nothing about how it is verified on a build machine has
/// no census to dispatch, and a hook that failed there would make this gate cost
/// every commit in every tree the hooks are run over.
#[test]
fn pre_commit_dispatches_the_census_elsewhere_and_a_fleet_that_cannot_take_it_blocks_nothing() {
    let f = Fixture::new();
    let out = f.run_hook("pre-commit", &[], "", &[]);
    let err = stderr_of(&out);
    assert!(
        out.status.success(),
        "a second machine that cannot be reached is not this repository's defect:\n{err}"
    );
    assert!(
        err.contains("on a machine that is not this one"),
        "the gate must announce itself, or a gate that stopped running looks \
         exactly like one that found nothing:\n{err}"
    );
    assert!(
        err.contains("no second machine has this tree"),
        "and it must say that it did NOT get one — silence here is the shape \
         every defect this lane exists for wore:\n{err}"
    );
}

/// And what that machine found reaches the person pushing, without blocking them.
#[test]
fn pre_push_reports_what_a_second_machine_found_and_never_blocks() {
    let f = Fixture::new();
    f.git(&["commit", "--no-verify", "-q", "-m", "test(fixture): seed"]);
    let sha = head_sha(&f);
    let stdin = push_line(&sha);
    let out = f.run_hook("pre-push", &["origin", "git@example:x"], &stdin, &[]);
    let err = stderr_of(&out);
    assert!(
        out.status.success(),
        "R888 and R889 were both pushes made deliberately while a gate was red, \
         to fix it; this one blocks for the same reason gate 6 does not:\n{err}"
    );
    assert!(
        err.contains("what a machine that is not this one found here"),
        "the gate announces itself:\n{err}"
    );
    assert!(
        err.contains("no second machine has judged this tree")
            || err.contains("no machine has been asked"),
        "a tree nothing was dispatched for must SAY so — that state reports zero \
         findings, and zero findings is what a clean tree looks like:\n{err}"
    );
}

/// The compiling gates a hook runs that a hosted runner CANNOT, and why.
///
/// R1287. Every other entry in the census below is either hosted or a debt; these
/// two are neither, and the difference is not about cost. Both ask about the
/// FLEET THIS PUSH IS LEAVING FROM rather than about the tree, so a runner asking
/// them would be asking a question with no subject — and a job that always
/// answers "nothing to report" is the shape this repository keeps finding under
/// the name "a green that means nobody looked".
///
/// A LIST RATHER THAN A COMMENT, for the reason `ci_plan::LAWS_OVER_THIS_POPULATION`
/// is one: a sentence stays true by luck. This is read by the test, each row is
/// checked to still name a gate a hook issues, and adding a row is an edit
/// somebody makes on purpose.
const CANNOT_LEAVE_THIS_MACHINE: [(&str, &str, &str); 2] = [
    (
        "run",
        "tools/one-machine/Cargo.toml",
        "it dispatches THIS tree to a second machine and reads what that machine \
         found; a runner has no second machine, and the census it would take is \
         of itself",
    ),
    (
        "run",
        "tools/ci-state/Cargo.toml",
        "it reports what the HOSTED RUN of the commit being built on concluded, \
         which is the one thing a push cannot learn from the run it is about to \
         start; inside that run it would be reading itself",
    ),
];

/// EVERY COMPILING GATE A GIT HOOK RUNS IS ONE A HOSTED JOB RUNS TOO (R1287).
///
/// THE OWNER'S WORD IS THE OCCASION AND THE MEASUREMENT IS THE POINT. "Move the
/// heavy things to CI" is a placement instruction, and placement is only
/// decidable against a fact nobody in this repository held: which of the gates a
/// hook makes this machine pay for are ALSO paid by a hosted runner. A gate run
/// in both places is the same work twice and the local copy is the one competing
/// for the cores; a gate run in ONE place cannot be moved without being lost,
/// and the difference between those two is invisible from either file alone.
///
/// COMPILING, because that is the axis that costs. [`ci_plan::compiles`] is a
/// measured table rather than a guess, and `cargo fmt` sits on the other side of
/// it for the reason `pre-push` states beside its own: it parses rather than
/// builds, so it costs a fraction of a transfer. What a push waits on is what
/// compiles the workspace.
///
/// THE KEY IS THE SUBCOMMAND, THE WORKSPACE AND THE SCOPE, NOT THE ARGV. Two
/// commands that clippy the whole root workspace are the same gate whether or
/// not one of them spells `--locked`; two that clippy different workspaces — or
/// the same one at different widths — are not the same gate at all. Dropping the
/// scope is not a theoretical worry: it is the form this test shipped in first,
/// and it is recorded as an injection rather than only as this sentence.
///
/// WHAT A FAILURE HERE MEANS, both ways round. A hook-only compiling gate is
/// either one that needs a hosted job — and then this test is the ask — or one
/// that genuinely cannot leave this machine, and that second answer belongs in
/// [`CANNOT_LEAVE_THIS_MACHINE`], where a reader meets it and an author has to
/// edit it. The absence of a job is what `pre-push`'s workspace clippy was for
/// its whole life, and nothing could tell that from a decision.
#[test]
fn every_compiling_gate_a_git_hook_runs_is_one_a_hosted_job_runs() {
    let root = repo_root();
    let issued = ci_plan::commands_this_repository_issues(&root);
    let tracked = ci_plan::tracked_files(&root, &["ls-files"]);

    // THE MANIFEST IS ONLY HALF OF "WHICH GATE", AND THE FIRST FORM OF THIS TEST
    // FOUND OUT BY BEING WRONG. `cargo clippy --workspace` in `pre-push` and
    // `cargo clippy -p mnemosyne-server --all-features` in the `server-features`
    // job both carry no `--manifest-path`, so both resolve to the root — and a
    // key of subcommand-and-manifest read them as ONE gate and reported the push
    // hook's workspace clippy as already hosted. It is not: nothing on a runner
    // clippies this workspace, which is the debt this test was written to ask
    // about, and the first key answered `covered` for it.
    //
    // SO THE SCOPE IS PART OF THE KEY: what a command COMPILES is decided by
    // `--workspace` / `-p` / `--all-targets`, not by which manifest it resolves.
    // Asked through `spells_a_flag`, which answers in three, because a command
    // assembled by a Rust program can carry a list this cannot see — and a scope
    // read off half a command line is the same false equality one layer down.
    let scope = |command: &ci_plan::CargoCommand| -> String {
        let mut parts: Vec<String> = Vec::new();
        for (flag, like) in [
            ("--workspace", &["--workspace", "--all"][..]),
            ("--all-targets", &["--all-targets"][..]),
        ] {
            match command.spells_a_flag(ci_plan::Side::Cargo, &|word: &str| like.contains(&word)) {
                ci_plan::Spelled::Yes(_) => parts.push(flag.to_string()),
                ci_plan::Spelled::No => {}
                ci_plan::Spelled::Unreadable(why) => parts.push(format!("<{flag}? {why}>")),
            }
        }
        let mut packages: Vec<&str> = command.values(&["-p", "--package"]);
        packages.sort_unstable();
        packages.dedup();
        for package in packages {
            parts.push(format!("-p {package}"));
        }
        if parts.is_empty() {
            "<the default package>".to_string()
        } else {
            parts.join(" ")
        }
    };

    let key = |command: &ci_plan::CargoCommand| -> Option<(String, String, String)> {
        let sub = command.subcommand()?.to_string();
        if ci_plan::compiles(&sub) != Some(true) {
            return None;
        }
        let over = match command.manifest(&tracked) {
            ci_plan::ManifestTarget::Root => "<root workspace>".to_string(),
            ci_plan::ManifestTarget::Named(path) => path,
            ci_plan::ManifestTarget::Unreadable(why) => format!("<unreadable: {why}>"),
        };
        Some((sub, over, scope(command)))
    };

    let mut hooks: BTreeMap<(String, String, String), Vec<String>> = BTreeMap::new();
    let mut hosted: BTreeSet<(String, String, String)> = BTreeSet::new();
    for command in &issued.commands {
        let Some(k) = key(command) else { continue };
        if command.source.starts_with(".githooks/") {
            hooks.entry(k).or_default().push(command.origin());
        } else if command.source.starts_with(".github/workflows/") {
            hosted.insert(k);
        }
    }
    assert!(
        !hooks.is_empty(),
        "no git hook issues a compiling command at all — the empty answer that \
         looks like a clean one"
    );
    println!("[venue] the hooks compile: {hooks:#?}");
    println!("[venue] the hosted jobs compile: {hosted:#?}");

    // THE SCOPE IN THE KEY IS LOAD-BEARING, AND THIS IS WHAT MAKES THAT
    // FALSIFIABLE. Dropping it can only ever LOOSEN the comparison — more
    // commands match, fewer orphans — so no injection over the orphan assertion
    // below can catch its absence: a blanked scope leaves that half green while
    // silently answering "covered" for gates nothing covers. That is not a
    // hypothetical. It is what the first form of this test did to `pre-push`'s
    // workspace clippy, whose twin was `server-features`' ONE-PACKAGE clippy over
    // the same manifest, and the answer would have licensed deleting the gate.
    //
    // SO THE PROPERTY IS ASSERTED DIRECTLY: this repository issues at least one
    // pair of commands that share a subcommand and a workspace and differ ONLY in
    // what they compile of it. While such a pair exists, a key that ignores the
    // scope is a key that reports two different gates as one — and if the pair
    // ever stops existing, this line is the notice that the third component has
    // stopped earning its place here rather than something to quietly keep.
    let mut scopes_by_gate: BTreeMap<(&String, &String), BTreeSet<&String>> = BTreeMap::new();
    for (sub, over, scope) in hooks.keys().chain(hosted.iter()) {
        scopes_by_gate.entry((sub, over)).or_default().insert(scope);
    }
    let split: Vec<String> = scopes_by_gate
        .iter()
        .filter(|(_, scopes)| scopes.len() > 1)
        .map(|((sub, over), scopes)| format!("cargo {sub} over {over}: {scopes:?}"))
        .collect();
    assert!(
        !split.is_empty(),
        "the key's third component is the SCOPE, and nothing in this tree \
         exercises it: no two commands share a subcommand and a workspace while \
         compiling different amounts of it. A key that ignores the scope reports \
         two gates as one and can only ever under-report orphans, so its absence \
         cannot be caught by the assertion below — this is the only place that \
         asks"
    );
    println!(
        "[venue] the scope separates {} gate(s) a coarser key would merge:\n  {}",
        split.len(),
        split.join("\n  ")
    );

    // THE OTHER HALF OF THE SAME MEASUREMENT, REPORTED RATHER THAN ASSERTED. A
    // gate in both places is the same work twice, and the local copy is the one
    // paid for on a workstation that is also building three sibling repositories.
    // It is NOT automatically wrong — a cheap parse-level gate is worth having at
    // the commit, where it costs a second and saves a round trip — so this prints
    // the list and leaves the judgement to whoever reads it. Turning it into an
    // assertion would be inventing a rule nobody stated.
    let twice: Vec<String> = hooks
        .iter()
        .filter(|(k, _)| hosted.contains(*k))
        .map(|((sub, over, scope), sites)| format!("cargo {sub} ({scope}) over {over} — {sites:?}"))
        .collect();
    println!(
        "[venue] paid in BOTH places, {} of them:\n  {}",
        twice.len(),
        twice.join("\n  ")
    );

    let excused: BTreeSet<(String, String)> = CANNOT_LEAVE_THIS_MACHINE
        .iter()
        .map(|(sub, over, _)| ((*sub).to_string(), (*over).to_string()))
        .collect();
    let orphans: Vec<String> = hooks
        .iter()
        .filter(|(k, _)| !hosted.contains(*k))
        .filter(|((sub, over, _), _)| !excused.contains(&(sub.clone(), over.clone())))
        .map(|((sub, over, scope), sites)| format!("cargo {sub} ({scope}) over {over} — {sites:?}"))
        .collect();

    // AN EXCUSE FOR A GATE THAT IS NOT THERE IS AN EXCUSE NOBODY WILL DELETE.
    // The list below is prose with teeth only while each row names a command a
    // hook actually issues; a row that outlives its gate reads as a considered
    // exemption and is a leftover. Checked here rather than trusted, because the
    // whole point of the list is that somebody has to edit it.
    for (sub, over, why) in CANNOT_LEAVE_THIS_MACHINE {
        assert!(
            hooks.keys().any(|(s, o, _)| s == sub && o == over),
            "`cargo {sub}` over {over} is excused from needing a hosted job \
             because \"{why}\" — and no git hook issues it any more, so the \
             excuse outlived the gate"
        );
    }
    assert!(
        orphans.is_empty(),
        "these compiling gates are paid by every push on this workstation and by \
         no hosted job, so they cannot be MOVED to CI — moving them would lose \
         them:\n  {}\nEither give each one a job in .github/workflows, or write \
         here why it cannot leave this machine.",
        orphans.join("\n  ")
    );
}
