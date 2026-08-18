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
use std::process::{Child, Command, Output};
use std::time::{Duration, Instant};

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

    /// The wrapper as this case will invoke it, with `VERIFY_LOCKS_HELD` set to
    /// whatever the case is about. ONE builder for both the waiting and the
    /// non-waiting form, so a case that watches the wrapper while it runs cannot
    /// be watching a differently-configured one.
    fn invocation(&self, locks_held: Option<&str>, command: &[&str]) -> Command {
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
        invocation
    }

    /// Run the wrapper here and wait for it.
    fn run(&self, locks_held: Option<&str>, command: &[&str]) -> Output {
        self.invocation(locks_held, command)
            .output()
            .expect("the wrapper runs")
    }

    /// Start the wrapper without waiting for it — the only way to ask anything
    /// about the state of the world WHILE it is still running.
    fn start(&self, locks_held: Option<&str>, command: &[&str]) -> Child {
        self.invocation(locks_held, command)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("the wrapper starts")
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

/// Ask, without waiting longer than it takes to be sure, whether the lock at
/// `path` can be taken right now. `flock -n` is the whole question: it belongs
/// to the OPEN FILE DESCRIPTION, so it answers about every process holding a
/// copy of one and not merely about the one that opened it.
fn lock_is_free(path: &str) -> bool {
    Command::new("flock")
        .args(["-n", path, "-c", "true"])
        .status()
        .expect("flock(1), which the wrapper itself needs")
        .success()
}

/// And the other direction, which is NOT its negation. A lock file that does not
/// exist yet cannot be taken either — `flock` fails to open it — so a plain
/// `!lock_is_free` would call a tree where nothing has happened yet "locked",
/// and a case waiting for the wrapper to take the lock would stop waiting before
/// the wrapper had started.
fn lock_is_held(path: &str) -> bool {
    Path::new(path).exists() && !lock_is_free(path)
}

fn wait_until(condition: impl Fn() -> bool, budget: Duration, what: &str) {
    let deadline = Instant::now() + budget;
    while Instant::now() < deadline {
        if condition() {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!(
        "{what} did not happen within {} second(s)",
        budget.as_secs()
    );
}

#[test]
fn the_lock_is_held_while_the_wrapper_runs_and_by_nothing_once_it_returns() {
    // THE FOURTH LINK, AND THE ONE A ROUND PAID FOR. An flock is held by the
    // open file description, which the three cases above rely on — and a
    // descriptor is COPIED INTO EVERY CHILD unless something closes it. The
    // wrapper opened the lock on fd 9 and then ran the command with that
    // descriptor still in its table, so anything the command left behind went on
    // holding this repository's build lock for as long as it lived. The wrapper
    // exits, the lock does not.
    //
    // MEASURED 2026-08-18: a case in `tools/one-machine` ended a child whose
    // background `sleep 600` survived it, the side-workspace gate ran that suite,
    // and the `git push` that followed stopped at `acquiring build lock` until
    // the leftover was found with `fuser` and killed by hand. Nothing in the
    // suite was red; the lock was simply held by a process nobody was looking at.
    //
    // ⚠ AND IT ASKS BOTH HALVES, because the second one alone is ALSO satisfied
    // by a wrapper that never took the lock at all — serialisation silently off
    // is the R743 corruption this lock exists to prevent, and "the lock is free"
    // is exactly what that looks like from outside. So the case waits for the
    // lock to be TAKEN while the wrapped command is still running, and only then
    // asks who holds it after the wrapper has gone.
    let tree = Tree::new();
    let finish = tree.dir.path().join("let-the-wrapper-finish");
    let gate = tree.dir.path().join("let-the-leftover-go");
    let gone = tree.dir.path().join("the-leftover-ended");

    // A LEFTOVER OF THE SHAPE THAT REALLY HAPPENS, in two respects. Its stdio is
    // redirected away, because a leftover holding the wrapper's stdout would
    // instead keep `tee` from ever reaching end-of-file — measured, 4002 ms
    // against 2 ms — which is a hang rather than a held lock, and would prove
    // nothing at all about this. And it ends on a CONDITION THIS CASE HOLDS
    // rather than after a fixed sleep, so the case that proves a leftover cannot
    // hold the lock does not itself become one. The directory test is the panic
    // path: `TempDir` removes it while unwinding, and both loops notice within
    // one asking interval.
    let until = |file: &Path| {
        format!(
            "while [ -d {} ] && [ ! -f {} ]; do sleep 0.05; done",
            tree.dir.path().display(),
            file.display()
        )
    };
    let left_behind = format!("{}; : > {}", until(&gate), gone.display());
    let command = format!("{{ {left_behind}; }} >/dev/null 2>&1 & {}", until(&finish));
    let running = tree.start(None, &["bash", "-c", &command]);

    // THE FIRST HALF: while the wrapped command is still going, this tree's
    // build lock is TAKEN. Without this the case passes against a wrapper that
    // locks nothing.
    wait_until(
        || lock_is_held(&tree.lock()),
        Duration::from_secs(30),
        "the wrapper taking this tree's build lock",
    );

    std::fs::write(&finish, "").expect("let the wrapper finish");
    let out = running
        .wait_with_output()
        .expect("the wrapper, which this case has let finish");
    assert!(
        out.status.success(),
        "the wrapper has to have RETURNED cleanly for the second half to be about \
         the lock at all:\n{}",
        said(&out)
    );

    // THE SECOND HALF, read before letting the leftover go, so what is asserted
    // is the state the next `git push` would have found.
    let free = lock_is_free(&tree.lock());
    std::fs::write(&gate, "").expect("let the leftover end");
    wait_until(
        || gone.exists(),
        Duration::from_secs(30),
        "the process this case left behind ending",
    );
    assert!(
        free,
        "the wrapper returned while a process it started still held the build \
         lock ({}). Every later run in this tree — the next round's suite, the \
         next `git push`'s hook — waits at `acquiring build lock` for a process \
         nothing is waiting on.",
        tree.lock()
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
