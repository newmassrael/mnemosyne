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

use ci_state::{annotations_in, annotations_query, checks_in, checks_query, report, Annotation};

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

    // THE OTHER HALF OF WHAT CI SAID (R893). A conclusion is one word, and a green
    // run can still be reporting something: the Node 20 runtime deprecation rode
    // in the annotations of every run this repository could see while every
    // conclusion beside them said `success`.
    let declared: u64 = checks
        .iter()
        .map(|check| check.output.annotations_count)
        .sum();
    let mut read: Vec<Annotation> = Vec::new();
    for check in checks.iter().filter(|c| c.output.annotations_count > 0) {
        match gh(root, &annotations_query(check.id))
            .and_then(|body| annotations_in(check.id, &body))
        {
            Ok(mut some) => read.append(&mut some),
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
