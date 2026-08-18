//! The wrapper every test run in this repository now goes through.
//!
//! R1196 put `scripts/verify.sh` in front of the workflows' suites and every
//! separate workspace's, so two of its properties stopped being a developer
//! convenience and became things CI depends on:
//!
//!   1. THE WRAPPED COMMAND'S EXIT STATUS IS WHAT COMES BACK. A wrapper that
//!      returned its own would turn every red suite green, silently, everywhere
//!      at once. `wait` on the process the script started is what makes that
//!      true — it was `PIPESTATUS[0]` until the pipe went away — and nothing
//!      read it.
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

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
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

/// How long a case here waits for something it is not itself the cause of.
///
/// ONE NUMBER FOR EVERY WAIT IN THIS FILE. None of these are measurements — each
/// is "the other side has had every chance", and the only thing a per-site
/// literal adds is a machine assumption written where nobody reviews it.
const WAIT_BUDGET: Duration = Duration::from_secs(30);

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
        WAIT_BUDGET,
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
        WAIT_BUDGET,
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

#[test]
fn a_leftover_holding_the_wrappers_stdout_does_not_keep_the_wrapper_running() {
    // THE SECOND EDGE OF THE SAME LEFTOVER, and it is not the lock. The case
    // above had to redirect its leftover's stdio away to be about the lock at
    // all, and wrote down why: a leftover holding the descriptor it was given
    // for stdout keeps `tee` from ever reaching end of file, so the wrapper does
    // not finish until the leftover does. Measured 2026-08-18 on the same
    // command with and without that redirection: 4002 ms against 2 ms. Nothing
    // is red, nothing is blocked — the verification simply does not come back,
    // which is the shape a person reads as a hung build machine.
    //
    // ⚠ AND IT ASKS THE VACUITY QUESTION THE LOCK CASE TAUGHT. "The wrapper
    // returned" is also true of a run whose leftover had already ended, which
    // proves nothing about waiting; so the leftover ends on a CONDITION THIS
    // CASE HOLDS, and what is asserted is that it was still running at the
    // moment the wrapper came back.
    let tree = Tree::new();
    let started = tree.dir.path().join("the-leftover-started");
    let release = tree.dir.path().join("let-the-leftover-go");
    let ended = tree.dir.path().join("the-leftover-ended");

    // NO REDIRECTION ANYWHERE IN THIS COMMAND, which is the whole of what it is
    // for: the leftover is handed the wrapper's stdout and keeps it. The
    // directory test is the panic path, as above — `TempDir` removes it while
    // unwinding and the loop notices within one asking interval.
    let leftover = format!(
        ": > {started}; while [ -d {tree_dir} ] && [ ! -f {release} ]; do sleep 0.05; done; \
         : > {ended}",
        started = started.display(),
        tree_dir = tree.dir.path().display(),
        release = release.display(),
        ended = ended.display(),
    );
    let mut running = tree.start(None, &["bash", "-c", &format!("{{ {leftover}; }} &")]);

    wait_until(
        || started.exists(),
        WAIT_BUDGET,
        "the process this case leaves behind starting",
    );

    // `try_wait` and not `wait_with_output`: reading the wrapper's output to end
    // of file is the very thing the leftover prevents, so a case that waited for
    // that would hang whether or not the wrapper had returned.
    let deadline = Instant::now() + WAIT_BUDGET;
    let status = loop {
        match running
            .try_wait()
            .expect("asking whether the wrapper has ended")
        {
            Some(status) => break status,
            None if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(50)),
            None => {
                std::fs::write(&release, "").expect("let the leftover end");
                panic!(
                    "the wrapper is still running 30 second(s) after the command it \
                     wrapped exited, because a process that command left behind holds \
                     the descriptor it was given for stdout. Every verification in this \
                     repository goes through this script, so what this looks like from \
                     outside is a suite that never finishes"
                );
            }
        }
    };

    // READ BEFORE THE LEFTOVER IS LET GO, so what is asserted is the state at
    // the moment the wrapper came back.
    let leftover_was_still_running = !ended.exists();
    std::fs::write(&release, "").expect("let the leftover end");
    wait_until(
        || ended.exists(),
        WAIT_BUDGET,
        "the process this case left behind ending",
    );

    assert!(
        status.success(),
        "the wrapper has to have returned the wrapped command's own status for \
         this case to be about when it returned"
    );
    assert!(
        leftover_was_still_running,
        "the leftover had already ended before the wrapper returned, so this case \
         says nothing about whether the wrapper waits for one. It is not a finding \
         about the wrapper — it is this case having stopped being a test"
    );
}

#[test]
fn everything_the_wrapped_command_wrote_reaches_both_the_log_and_the_caller() {
    // WHAT THE PIPE USED TO GUARANTEE, asked directly. `tee` copied every byte
    // to both places by construction; a log the command writes itself and a
    // follower echoing it to the caller is two mechanisms, and each has an end
    // it can lose — the follower can start after the first line is written, or
    // stop before the last one is read. Both were measured to be sound, and this
    // is what keeps them so: a wrapper that dropped either end would be showing
    // CI a shorter run than the one that happened, which is the R743 defect this
    // whole script exists for.
    let tree = Tree::new();
    let out = tree.run(
        None,
        &[
            "bash",
            "-c",
            "echo THE-FIRST-LINE; for i in $(seq 1 2000); do echo line$i; done; \
             echo THE-LAST-LINE",
        ],
    );
    let words = said(&out);
    for line in ["THE-FIRST-LINE", "line1", "line2000", "THE-LAST-LINE"] {
        assert!(
            words.contains(line),
            "`{line}` reached the log but not the caller, so what CI prints is a \
             shorter run than the one that happened:\n{words}"
        );
    }

    // ONCE, AND THE COUNT IS THE POINT. The follower is started at the log's
    // CURRENT end rather than its beginning, and this script runs two things
    // into one log — the command and then the coverage gate. A follower that
    // began at byte one would replay everything the command wrote a second time
    // when the gate ran, which for a suite of any size is megabytes of the same
    // output and a reader who cannot tell a re-run from a repeat.
    assert_eq!(
        words.matches("line2000").count(),
        1,
        "the caller was shown the wrapped command's output more than once:\n{words}"
    );

    let named = words
        .lines()
        .find_map(|line| line.strip_prefix("[verify] log: "))
        .expect("the wrapper names the log it is writing");
    let recorded = std::fs::read_to_string(tree.dir.path().join(named))
        .expect("the log the wrapper named exists");
    for line in ["THE-FIRST-LINE", "line1", "line2000", "THE-LAST-LINE"] {
        assert!(
            recorded.contains(line),
            "`{line}` reached the caller but not the log, so the record this \
             repository keeps of the run is not the run:\n{recorded}"
        );
    }
}

#[test]
fn the_wrapped_command_still_reads_the_callers_standard_input() {
    // THE SILENT HALF OF RUNNING THE COMMAND ASYNCHRONOUSLY. A shell with job
    // control off assigns /dev/null to an asynchronous command's standard input
    // before any redirection written on the command itself, so the obvious
    // spelling takes the caller's stdin away from whatever is being verified and
    // says nothing: a command that reads gets end of file instead of input, and
    // a suite that read a fixture from stdin would go red somewhere else
    // entirely. Measured both ways before the wrapper was written this way.
    let tree = Tree::new();
    let mut invocation = tree.invocation(None, &["bash", "-c", "read line; echo GOT=[$line]"]);
    let mut running = invocation
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the wrapper starts");
    running
        .stdin
        .take()
        .expect("the wrapper was given a stdin to pass on")
        .write_all(b"from-the-caller\n")
        .expect("writing to the wrapper's stdin");
    let out = running
        .wait_with_output()
        .expect("the wrapper, whose command reads one line and ends");
    let words = said(&out);
    assert!(
        words.contains("GOT=[from-the-caller]"),
        "the command the wrapper ran did not receive the caller's standard \
         input, so this wrapper is silently a different environment from running \
         the command directly:\n{words}"
    );
}
