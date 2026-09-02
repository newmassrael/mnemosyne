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
//! | `jobs.<id>.json` | the same, for ONE run — used when it exists |
//! | `run.json` | the answer about one run |
//! | `caches.json` | the answer about cache storage — the default arm |
//!
//! IT DISPATCHES ON THE ENDPOINT, and that is not decoration: the four answers
//! have nothing in common but their transport, so a stub handing the cache page
//! to a question about a run would agree with a gate that asked for the wrong
//! thing.
//!
//! AND IT HONOURS `per_page` ON THE RUN LIST, which R1312 found it was not doing.
//! A stub that answers a question it was not asked is a fixture that cannot see
//! the parameter it ignored: the depth of that page decides which runs the gate
//! can bound an interval with, it stood at five for five rounds, and NOTHING in
//! this suite could go red for it — the answer came back whole however small a
//! page the gate asked for. It is scoped to the run list on purpose. The cache
//! endpoint is asked with `--paginate` and has laws of its own about pages that
//! disagree with their count, and the jobs endpoint has a law about a page that
//! stopped early; truncating either here would answer those with this program
//! instead of with their own fixtures.

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
    let runs = asked.contains("/actions/workflows/");
    let path = if runs {
        directory.join("runs.json")
    } else if asked.contains("/jobs?") {
        // PER RUN WHERE THE CASE SAYS SO. One answer for every run is a stub
        // that agrees with a gate reading the same page whichever run it asked
        // about — and telling one run's saves from another's is the whole of
        // what bounds an interval (R1207).
        let named = run_id_in(&asked).map(|id| directory.join(format!("jobs.{id}.json")));
        match named.filter(|path| path.exists()) {
            Some(path) => path,
            None => directory.join("jobs.json"),
        }
    } else if asked.contains("/actions/runs/") {
        directory.join("run.json")
    } else {
        directory.join("caches.json")
    };
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(why) => {
            eprintln!("gh-stub: cannot read {}: {why}", path.display());
            return ExitCode::from(1);
        }
    };
    // AND THE PAGE IS AS DEEP AS IT WAS ASKED TO BE.
    let bytes = match runs.then(|| per_page_in(&asked)).flatten() {
        Some(rows) => match truncated(&bytes, rows) {
            Ok(shorter) => shorter,
            Err(why) => {
                eprintln!("gh-stub: cannot page {}: {why}", path.display());
                return ExitCode::from(1);
            }
        },
        None => bytes,
    };
    if let Err(why) = std::io::stdout().write_all(&bytes) {
        eprintln!("gh-stub: cannot hand over {}: {why}", path.display());
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

/// The run a jobs question is about — `…/actions/runs/<id>/jobs?…`.
fn run_id_in(asked: &str) -> Option<&str> {
    asked
        .split("/actions/runs/")
        .nth(1)?
        .split('/')
        .next()
        .filter(|id| !id.is_empty() && id.chars().all(|digit| digit.is_ascii_digit()))
}

/// How many rows the caller asked for — `?per_page=100`, or `&per_page=100`.
fn per_page_in(asked: &str) -> Option<usize> {
    asked
        .split("per_page=")
        .nth(1)?
        .split(|c: char| !c.is_ascii_digit())
        .next()?
        .parse()
        .ok()
}

/// That answer with its `workflow_runs` cut to the page asked for.
///
/// `total_count` IS LEFT ALONE, because GitHub leaves it alone: it is the whole
/// history's count and not the page's, which is exactly why the reader of this
/// endpoint does not check one against the other.
fn truncated(bytes: &[u8], rows: usize) -> Result<Vec<u8>, String> {
    let mut answer: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|why| format!("not JSON: {why}"))?;
    let Some(runs) = answer
        .get_mut("workflow_runs")
        .and_then(|it| it.as_array_mut())
    else {
        return Ok(bytes.to_vec());
    };
    runs.truncate(rows);
    serde_json::to_vec(&answer).map_err(|why| format!("cannot write it back: {why}"))
}
