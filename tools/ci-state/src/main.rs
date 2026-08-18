//! Ask GitHub what CI said about one commit, and print it.
//!
//! WHAT IS LEFT HERE IS THE PART A SUITE CANNOT REACH: a process, its exit
//! status, and its two streams. What to ask for and what the answer means both
//! live in the library, because a decision in `main.rs` has no reader — R1129
//! measured three gates whose `main.rs` carried a decision nothing was running,
//! and R1126 moved a neighbouring reporter's words out for the same reason.
//!
//! TWO EXIT CODES, AND THE SECOND ONE HAS A READER. `0` means a report was
//! printed, INCLUDING the reports "gh is not installed", "gh could not reach
//! GitHub", "GitHub's answer is not a shape I can read" and "the commit is RED".
//! This program reports and never blocks, which is the semantics R890 argued for
//! from the history rather than a softening of it: the two pushes that fixed a
//! red CI were made deliberately while it was red, and a gate would have been
//! wrong both times. `2` means no report could be produced at all — bad usage —
//! and `.githooks/pre-push` prints a different sentence for it, because a check
//! that stays quiet when it cannot answer is indistinguishable from one that
//! answered "fine". There is no `1`: a violation is not one of this program's
//! answers, and inventing the code would invent a caller that acts on it.

use std::path::Path;
use std::process::Command;

use ci_state::{
    annotations_in, annotations_query, checks_in, checks_query, is_failing, job_of, report,
    steps_in, steps_query, stoppage_line, stopped_at, Check, Said, STOPPED_NOWHERE,
};

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let [sha] = arguments.as_slice() else {
        eprintln!(
            "usage: ci-state <sha> — reports what CI said about one commit. Given {} \
             argument(s), there is no commit to report on",
            arguments.len()
        );
        std::process::exit(2);
    };
    let root = match std::env::current_dir() {
        Ok(here) => here,
        Err(why) => {
            eprintln!("ci-state: this reporter has no working directory to ask `gh` from: {why}");
            std::process::exit(2);
        }
    };
    for line in state_of(&root, sha) {
        println!("ci-state: {line}");
    }
}

/// Run `gh` and hand back its output, or say why it could not be asked.
///
/// THE TWO FAILURES ARE DIFFERENT FACTS AND ARE TOLD APART. A `gh` that is not on
/// this machine is a tool nobody installed; a `gh` that exits non-zero reached the
/// point of trying and failed — no network, no credential, a repository it cannot
/// resolve. The hook this replaces printed both, and collapsing them here would
/// send a reader to install something they already have.
fn gh(root: &Path, arguments: &[String]) -> Result<String, String> {
    let out = Command::new("gh")
        .args(arguments)
        .current_dir(root)
        .output()
        .map_err(|why| match why.kind() {
            std::io::ErrorKind::NotFound => "`gh` is not installed on this machine".to_string(),
            _ => format!("`gh` could not be run at all: {why}"),
        })?;
    if !out.status.success() {
        return Err(format!(
            "`gh {}` failed ({}): {}",
            arguments.join(" "),
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Every line this reporter has to say about one commit.
///
/// A REFUSAL IS A LINE AND NOT A SILENCE. Each of the three ways this can fail to
/// find out — `gh` missing, `gh` failing, an answer it cannot read — returns a
/// sentence naming which one it was, and the caller prints it like any other.
fn state_of(root: &Path, sha: &str) -> Vec<String> {
    let answer = match gh(root, &checks_query(sha)) {
        Ok(answer) => answer,
        Err(why) => return vec![format!("NOTE CI state for {sha} is unknown — {why}")],
    };
    let checks = match checks_in(sha, &answer) {
        Ok(checks) => checks,
        Err(why) => return vec![format!("NOTE {why}")],
    };
    let mut lines = report(sha, &checks);

    // WHICH STEP ENDED IT, FOR EVERY CHECK THAT DID NOT PASS (R1236). The
    // per-commit endpoint answers with a conclusion and no steps, and those two
    // words are not enough to attribute anything: `cancelled` after 45 minutes
    // reads as "the change this push is about hung the job", and the run that made
    // this round was `apt-get` stalling three steps in, with every later step —
    // this repository's clippy, its suite, its wrapper — never run at all. That
    // was a second tool's answer for one afternoon; it is this one's now.
    for check in checks.iter().filter(|check| is_failing(check)) {
        lines.extend(steps_of(root, check));
    }

    // THE OTHER HALF OF WHAT CI SAID (R893). A conclusion is one word, and a green
    // run can still be reporting something: the Node 20 runtime deprecation rode
    // in the annotations of every run this repository could see while every
    // conclusion beside them said `success`.
    let declared: u64 = checks
        .iter()
        .map(|check| check.output.annotations_count)
        .sum();
    // PAIRED WITH THE CHECK THAT SAID IT (R1238). This loop already knows which
    // check it is asking about — the name was thrown away here, one line down,
    // and getting it back cost three `gh api` calls by hand the day a red commit
    // carried two failing jobs and five flat lines.
    let mut read: Vec<Said> = Vec::new();
    for check in checks.iter().filter(|c| c.output.annotations_count > 0) {
        match gh(root, &annotations_query(check.id))
            .and_then(|body| annotations_in(check.id, &body))
        {
            Ok(some) => read.extend(some.into_iter().map(|annotation| Said {
                check: check.name.clone(),
                annotation,
            })),
            // NAMED, AND THE REST STILL READ: one check whose annotations cannot
            // be fetched must not take the other checks' annotations down with
            // it, and the shortfall shows up in the "N distinct of D reported"
            // line either way.
            Err(why) => lines.push(format!("NOTE {why}")),
        }
    }
    lines.extend(ci_state::annotation_report(sha, declared, &read));
    lines
}

/// What one failing check's own steps say about where its job stopped.
///
/// EVERY WAY THIS CAN COME BACK EMPTY IS A SENTENCE. A check no Actions job is
/// behind, a `gh` that could not be asked, an answer that would not read, a job
/// whose steps name no stopping point: each returns a line saying which one it
/// was, because the whole value here is that a reader stops guessing — and a
/// reporter that fell silent would hand back exactly the guess it exists to end.
fn steps_of(root: &Path, check: &Check) -> Vec<String> {
    let Some(job) = job_of(&check.details_url) else {
        return vec![format!(
            "      no Actions job behind `{}` — its details are at {}",
            check.name, check.details_url
        )];
    };
    let steps = match gh(root, &steps_query(job)).and_then(|body| steps_in(job, &body)) {
        Ok(steps) => steps,
        Err(why) => return vec![format!("      NOTE {why}")],
    };
    match stopped_at(&steps) {
        Some(stoppage) => vec![format!("      {}", stoppage_line(&stoppage))],
        None => vec![format!("      {STOPPED_NOWHERE}")],
    }
}
