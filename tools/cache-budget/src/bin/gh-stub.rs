//! A `gh` that records what it was asked and answers out of files on disk.
//!
//! # Why this is a cargo-built program rather than a script the fixture writes
//!
//! Round 1192: a file this process wrote and then ran cannot be exec'd while any
//! process holds it open for writing, and the holder is a SIBLING test's fork
//! rather than this thread. The failure is `ETXTBSY`, it arrives only when
//! something else is running, and it reads as a flake in whichever crate was
//! unlucky. Cargo builds this before any test starts and the fixture reaches it
//! by SYMLINK.
//!
//! WHAT THAT MOVED, and it is the whole reason the answers are files now: the
//! script found its own directory with `dirname "$0"`, which works because the
//! kernel hands an interpreter the resolved path. A binary reached through a
//! symlink cannot ask that question of anything — `argv[0]` is the name `execvp`
//! was given and `/proc/self/exe` is the cargo target — so the directory is
//! NAMED, by `GH_STUB_DIR`, and the four answers live in it as data. Which is
//! the better half of the same rule: the behaviour that varies per case is data
//! the program reads, and data cannot be busy.
//!
//! | in `$GH_STUB_DIR` | what it is |
//! |---|---|
//! | `asked` | appended: the words of every call, one per line, a blank line between calls |
//! | `runs.json` | the answer to a workflow's run list |
//! | `jobs.json` | the answer to a run's job list |
//! | `run.json` | the answer about one run |
//! | `caches.json` | the answer about cache storage — the default arm |
//!
//! IT DISPATCHES ON THE ENDPOINT, and that is not decoration: the four answers
//! have nothing in common but their transport, so a stub handing the cache page
//! to a question about a run would agree with a gate that asked for the wrong
//! thing.

use std::io::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let Ok(directory) = std::env::var("GH_STUB_DIR") else {
        eprintln!("gh-stub: GH_STUB_DIR names no directory to answer out of");
        return ExitCode::from(3);
    };
    let directory = PathBuf::from(directory);

    let arguments: Vec<String> = std::env::args().skip(1).collect();
    // WHAT IT WAS ASKED, APPENDED — one block per call, because the gate asks
    // three times inside a run and a record that overwrote itself would show
    // only whichever came last.
    let mut record = String::new();
    for argument in &arguments {
        record.push_str(argument);
        record.push('\n');
    }
    record.push('\n');
    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(directory.join("asked"))
    {
        Ok(mut file) => {
            if let Err(why) = file.write_all(record.as_bytes()) {
                eprintln!("gh-stub: cannot record the call: {why}");
                return ExitCode::from(3);
            }
        }
        Err(why) => {
            eprintln!("gh-stub: cannot record the call: {why}");
            return ExitCode::from(3);
        }
    }

    let asked = arguments.join(" ");
    let answer = if asked.contains("/actions/workflows/") {
        "runs.json"
    } else if asked.contains("/jobs?") {
        "jobs.json"
    } else if asked.contains("/actions/runs/") {
        "run.json"
    } else {
        "caches.json"
    };
    let path = directory.join(answer);
    match std::fs::read(&path) {
        Ok(bytes) => {
            if let Err(why) = std::io::stdout().write_all(&bytes) {
                eprintln!("gh-stub: cannot hand over {}: {why}", path.display());
                return ExitCode::from(1);
            }
            ExitCode::SUCCESS
        }
        Err(why) => {
            eprintln!("gh-stub: cannot read {}: {why}", path.display());
            ExitCode::from(1)
        }
    }
}
