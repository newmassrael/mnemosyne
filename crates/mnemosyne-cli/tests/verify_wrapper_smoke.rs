//! The wrapper every test run in this repository now goes through.
//!
//! R1196 put `scripts/verify.sh` in front of the workflows' suites and every
//! separate workspace's, so two of its properties stopped being a developer
//! convenience and became things CI depends on:
//!
//!   1. THE WRAPPED COMMAND'S EXIT STATUS IS WHAT COMES BACK. A wrapper that
//!      returned its own would turn every red suite green, silently, everywhere
//!      at once. `PIPESTATUS[0]` is what makes that true and nothing read it.
//!   2. THE BUILD LOCK IS RE-ENTRANT ACROSS THE PROCESS TREE. An flock belongs
//!      to the open file description, so a nested run opening the same path
//!      waits for a lock its own ancestor holds and is never woken. That is not
//!      a slow gate, it is a hang — and this round created the nesting on
//!      purpose, because `scripts/check-side-workspaces.sh` now runs each suite
//!      through the wrapper and RULEBOOK asks a round to run THAT script through
//!      the wrapper too.
//!
//! DRIVEN WITH A `cargo` STUB, for the reason `git_hooks_smoke.rs` drives the
//! lister's arms with one: what is under test is the script's own decisions, and
//! a fixture that made the real coverage gate answer would be measuring the gate
//! instead. The stub is PREPENDED to `PATH` rather than replacing it — the
//! script also needs `git`, `flock`, `tee` and `date`, and a hermetic path would
//! be testing which coreutils this machine has. It is a TRACKED stand-in reached
//! by symlink (`tests/stubs/`), never a file this process writes and then runs.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

use crate::common::link_stub;

fn wrapper() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> sits two levels under the root")
        .join("scripts/verify.sh")
}

/// A tree to run in, with a stubbed `cargo` in front of the real `PATH`.
struct Tree {
    dir: TempDir,
    path: String,
}

impl Tree {
    fn new() -> Self {
        let dir = TempDir::new().expect("tempdir");
        let shim = dir.path().join("shim");
        link_stub("cargo-always-succeeds", &shim.join("cargo"));
        let path = format!(
            "{}:{}",
            shim.to_str().expect("the shim path is utf-8"),
            std::env::var("PATH").unwrap_or_default()
        );
        Self { dir, path }
    }

    fn lock(&self) -> String {
        self.dir
            .path()
            .join("target/.verify.lock")
            .to_str()
            .expect("the lock path is utf-8")
            .to_string()
    }

    /// Run the wrapper here, with `VERIFY_LOCKS_HELD` set to whatever this case
    /// is about, over the given command.
    fn run(&self, locks_held: Option<&str>, command: &[&str]) -> Output {
        let mut invocation = Command::new(wrapper());
        invocation
            .args(["--no-fresh", "--label", "a-case", "--"])
            .args(command)
            .current_dir(self.dir.path())
            .env("PATH", &self.path);
        match locks_held {
            Some(value) => invocation.env("VERIFY_LOCKS_HELD", value),
            None => invocation.env_remove("VERIFY_LOCKS_HELD"),
        };
        invocation.output().expect("the wrapper runs")
    }
}

fn said(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

#[test]
fn the_wrapped_commands_own_exit_status_is_what_the_wrapper_returns() {
    let tree = Tree::new();
    let failed = tree.run(None, &["bash", "-c", "exit 7"]);
    assert_eq!(
        failed.status.code(),
        Some(7),
        "CI reads this script's status as the suite's, so a wrapper that \
         returned `tee`'s would report every red suite as green:\n{}",
        said(&failed)
    );

    let passed = tree.run(None, &["bash", "-c", "exit 0"]);
    assert_eq!(
        passed.status.code(),
        Some(0),
        "and the other direction, or the law above would hold for a wrapper \
         that failed everything:\n{}",
        said(&passed)
    );
}

#[test]
fn a_lock_this_process_tree_already_holds_is_not_taken_a_second_time() {
    // THE DEADLOCK THIS ROUND WOULD OTHERWISE HAVE SHIPPED. Asserted on the
    // DECISION the script prints rather than by nesting two real runs: a case
    // that proved it by not hanging would, when broken, hang — and a hanging
    // suite is a timeout somebody reads as infrastructure.
    let tree = Tree::new();
    let lock = tree.lock();
    let out = tree.run(Some(&lock), &["bash", "-c", "exit 0"]);
    let words = said(&out);
    assert!(
        words.contains("already held by this process tree"),
        "an flock is held by the open file description, so re-taking a lock an \
         ancestor holds waits for something that cannot arrive:\n{words}"
    );
    assert!(
        !words.contains("acquiring build lock"),
        "and it must not do BOTH — the sentence is the whole of what says which \
         branch ran:\n{words}"
    );
}

#[test]
fn the_marker_names_the_lock_and_not_merely_that_one_is_held() {
    // The identity half. This repository's gates run over ANOTHER tree as well
    // as their own, so a marker meaning "some lock is held" would skip the lock
    // for a second tree that nothing has locked at all — serialisation quietly
    // off wherever a gate is pointed elsewhere, which is the R743 corruption
    // this lock exists to prevent.
    let tree = Tree::new();
    let out = tree.run(
        Some("/elsewhere/target/.verify.lock"),
        &["bash", "-c", "exit 0"],
    );
    let words = said(&out);
    assert!(
        words.contains("acquiring build lock"),
        "a marker naming somebody else's lock says nothing about this tree's, so \
         this run has to take it:\n{words}"
    );
}

#[test]
fn the_marker_reaches_the_command_the_wrapper_runs() {
    // The third link, and without it the two above are true of a script whose
    // children never learn what it holds. `export` is what carries a lock across
    // the fork, and a nested wrapper reads it from there.
    let tree = Tree::new();
    let out = tree.run(None, &["bash", "-c", "echo MARKER=[$VERIFY_LOCKS_HELD]"]);
    let words = said(&out);
    assert!(
        words.contains(&format!("MARKER=[{}]", tree.lock())),
        "the command a wrapper runs must be able to see which lock is already \
         held, or nesting deadlocks however carefully the parent recorded it:\
         \n{words}"
    );
}
