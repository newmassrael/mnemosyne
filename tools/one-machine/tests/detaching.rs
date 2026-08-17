//! The launcher returns before the run it starts ends, and the run's verdict is
//! the kernel's.
//!
//! MEASURED, AND THE FIRST DRAFT WAS WRONG IN THE DIRECTION THAT LOOKS LIKE
//! WORKING. It read `mkdir … && L=… && setsid … > "$L" 2>&1 < /dev/null &`, and
//! `&` there backgrounds the whole `&&` LIST: the subshell running it keeps the
//! transport's stdout as its own fd 1 and then waits for the census. On the
//! fleet this was written for, that dispatch returned in 565 seconds instead of
//! 23 — a correct census, a correct verdict, and a git hook that does not come
//! back. Nothing about the answer was wrong, which is why only the clock said so.
//!
//! So this case is about DESCRIPTORS and not about the answer. It gives the
//! launcher a command that will not end until this case lets it, and asks
//! whether the caller is free while that is still true.

use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use one_machine::{launcher, run_bounded, SENTINEL};

/// How long the caller may take to come back while the run it started is still
/// blocked. It is the BOUND ON A HANG rather than an expectation: the launcher
/// returns in milliseconds when it is right, and never when it is wrong, so
/// anything between those says the same thing.
const RETURNING: Duration = Duration::from_secs(30);

/// How long to wait for the sentinel once the run has been let go, asked as a
/// condition rather than slept through.
const SENTINEL_ARRIVING: Duration = Duration::from_secs(30);

/// How often that condition is asked.
const ASKING: Duration = Duration::from_millis(50);

fn wait_for(condition: impl Fn() -> bool, budget: Duration, what: &str) {
    let deadline = Instant::now() + budget;
    while Instant::now() < deadline {
        if condition() {
            return;
        }
        std::thread::sleep(ASKING);
    }
    panic!(
        "{what} did not happen within {} second(s)",
        budget.as_secs()
    );
}

fn log_says(log: &Path, word: &str) -> bool {
    std::fs::read_to_string(log).is_ok_and(|text| text.contains(word))
}

/// Whatever else happens, the run this case started is let go.
///
/// ⚠ ON DROP AND NOT AT THE END OF THE CASE, because the case can end by
/// PANICKING — and it does exactly that when the launcher is broken, which is
/// the state the injection sweep deliberately puts this file in. A run detached
/// into its own session outlives the test binary, so a case that let go only on
/// success would leave one waiting process behind per red arm, on a machine
/// several repositories share.
struct LetGo(std::path::PathBuf);

impl Drop for LetGo {
    fn drop(&mut self) {
        let _ = std::fs::write(&self.0, "");
    }
}

#[test]
fn the_launcher_returns_while_the_run_it_started_is_still_going() {
    let at = tempfile::tempdir().expect("a directory to stand in");
    let log = at.path().join("census.log");
    let gate = at.path().join("let-it-finish");
    let _let_go = LetGo(gate.clone());
    // A COMMAND THIS CASE HOLDS THE END OF. Nothing here sleeps for a fixed
    // time and hopes: the run ends when this case creates a file, so "the
    // caller came back first" is an ORDER rather than a race.
    //
    // AND IT ENDS IN A SUBSHELL RATHER THAN A BARE `exit`, which is not a
    // detail: the launcher appends `; echo "<sentinel>$?"` to whatever it is
    // given, so a command that exits the SHELL takes the sentinel with it. The
    // real one is a script — a separate process, whose exit is a status rather
    // than an exit — and this case has to stand in the same relation to the
    // launcher, or it would be measuring a shape nothing uses. (The first draft
    // did not, and spent a run finding out.)
    //
    // AND IT ALSO ENDS WHEN ITS DIRECTORY DOES. The file above is written on the
    // way out even from a panic, but the temporary directory is removed on the
    // way out too, and which of those the loop notices first is a race. A run
    // whose ground has been removed is over by any reading, so it is asked.
    let held = format!(
        "while [ -d {} ] && [ ! -f {} ]; do sleep 0.05; done; ( exit 3 )",
        at.path().display(),
        gate.display()
    );
    let launched = launcher(log.to_str().expect("a path"), &held);

    // THE CALLER IS GIVEN A PIPE, and that is the whole of what this case is
    // about. A transport reads its remote half's output until END OF FILE, and a
    // pipe is at end of file only when every process holding the write end has
    // let go — so "did the caller come back" is not the question. The question is
    // whether the DESCRIPTOR is free while the run it started is still going,
    // and only a reader waiting on end-of-file can ask it. Given files instead,
    // both the right launcher and the wrong one look identical: measured, the
    // first version of this case passed against the very form that hung a real
    // dispatch for 565 seconds.
    let mut child = Command::new("bash")
        .arg("-c")
        .arg(&launched)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("a shell");
    let mut reading = child
        .stdout
        .take()
        .expect("a pipe this case just asked for");
    let (told, freed) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        use std::io::Read as _;
        let mut swallowed = Vec::new();
        let _ = reading.read_to_end(&mut swallowed);
        let _ = told.send(swallowed);
    });
    let started = Instant::now();
    let swallowed = freed.recv_timeout(RETURNING).unwrap_or_else(|_| {
        let _ = child.kill();
        panic!(
            "the transport's own descriptor was still held {} second(s) after \
             the launcher started a run this case has not let finish. That is a \
             connection that stays open for the whole census, and a git hook that \
             does not come back.\n  it was: {launched}",
            RETURNING.as_secs()
        )
    });
    assert!(
        swallowed.is_empty(),
        "and nothing of the run's output came back up it: {}",
        String::from_utf8_lossy(&swallowed)
    );
    assert!(
        !gate.exists(),
        "this case has not let the run finish yet, so the descriptor was freed \
         while it was still going — which is the claim ({}ms)",
        started.elapsed().as_millis()
    );
    assert!(
        !log_says(&log, SENTINEL),
        "and the run has not ended, so nothing may have written a verdict yet"
    );
    let status = child.wait().expect("the launcher itself");
    assert!(status.success(), "the launcher itself must succeed");

    // AND THE VERDICT IS THE KERNEL'S. Rule 6 of this machine's remote-build
    // protocol puts the sentinel in the LAUNCHER rather than in the script,
    // after a script printed `EXPERIMENT COMPLETE` having failed all three of
    // its conditions. `exit 3` above is what the wrapped command really did.
    std::fs::write(&gate, "").expect("let the run finish");
    wait_for(
        || log_says(&log, SENTINEL),
        SENTINEL_ARRIVING,
        "the sentinel",
    );
    let text = std::fs::read_to_string(&log).expect("the log the run wrote");
    assert!(
        text.contains(&format!("{SENTINEL}3")),
        "the sentinel carries the status the kernel gave, not one the script \
         claimed:\n{text}"
    );
}

/// And a program that never returns is ended and said so, rather than waited on.
///
/// This is the arm that makes the bound above real: both callers of this are git
/// hooks, and a transport sitting on a host that has stopped answering would
/// otherwise hold a commit open for as long as the kernel allows.
#[test]
fn a_program_that_does_not_return_is_ended_and_named() {
    let mut command = Command::new("bash");
    command.arg("-c").arg("sleep 600");
    let error = run_bounded(&mut command, Duration::from_secs(1))
        .expect_err("a program that outlives its budget");
    assert!(
        error.contains("did not finish within 1 second(s)"),
        "the budget is named in what it says: {error}"
    );
}

/// A grandchild still holding the output must not extend the wait either — the
/// shape that made the bound above meaningless in the first draft, where the
/// output went to a pipe and reading it to end-of-file was unbounded.
#[test]
fn output_held_open_by_something_the_child_left_behind_does_not_extend_the_wait() {
    let mut command = Command::new("bash");
    // The child is ended at the budget; the `sleep` it left behind still holds
    // whatever it inherited, and this call must come back regardless.
    command.arg("-c").arg("sleep 600 & sleep 600");
    let started = Instant::now();
    let error = run_bounded(&mut command, Duration::from_secs(1))
        .expect_err("a child that outlives its budget");
    assert!(error.contains("did not finish"), "{error}");
    assert!(
        started.elapsed() < RETURNING,
        "the wait was extended by a process this gate never asked about"
    );
}
