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
    // THE ANNOTATIONS ARE FETCHED BEFORE THE CENSUS IS PHRASED (R1242), which is a
    // requirement rather than a tidier order. Whether a cancelled run was retired
    // by a LATER PUSH is a fact only an annotation carries, and the sentence that
    // says `RED` or `NO VERDICT` cannot be written without it. The lines still come
    // out in the same order; what moved is when the answer is in hand.
    //
    // A GREEN PUSH STILL MAKES NO EXTRA CALL: the loop asks only about checks whose
    // own row declares annotations, which is the rule it has followed since R893.
    let declared: u64 = checks
        .iter()
        .map(|check| check.output.annotations_count)
        .sum();
    // PAIRED WITH THE CHECK THAT SAID IT (R1238). This loop already knows which
    // check it is asking about — the name was thrown away here, one line down,
    // and getting it back cost three `gh api` calls by hand the day a red commit
    // carried two failing jobs and five flat lines.
    let mut read: Vec<Said> = Vec::new();
    let mut notes: Vec<String> = Vec::new();
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
            Err(why) => notes.push(format!("NOTE {why}")),
        }
    }

    let retired = ci_state::superseded_checks(&checks, &read);
    let mut lines = report(sha, &checks, &retired);

    // WHICH STEP ENDED IT, FOR EVERY CHECK THAT DID NOT PASS (R1236). The
    // per-commit endpoint answers with a conclusion and no steps, and those two
    // words are not enough to attribute anything: `cancelled` after 45 minutes
    // reads as "the change this push is about hung the job", and the run that made
    // this round was `apt-get` stalling three steps in, with every later step —
    // this repository's clippy, its suite, its wrapper — never run at all. That
    // was a second tool's answer for one afternoon; it is this one's now.
    //
    // AND NOT FOR A CHECK A LATER PUSH RETIRED (R1242). Where a retired job got to
    // is true and it is not about this commit: `stopped at step 9 cargo test
    // --workspace, 5 of the 9 after it never ran` reads as a diagnosis until the
    // sentence above it says otherwise, and it costs a call per check to say. The
    // count of what was not asked is PRINTED, because a block that shrinks in
    // silence is how a reader comes to believe there was nothing to see.
    let mut unasked = 0;
    for check in checks.iter().filter(|check| is_failing(check)) {
        if retired.contains(&check.name) {
            unasked += 1;
            continue;
        }
        lines.extend(steps_of(root, check));
    }
    if unasked > 0 {
        lines.push(format!(
            "      not asking where {unasked} retired check(s) stopped — a run a later push \
             ended says nothing about this commit"
        ));
    }

    // THE OTHER HALF OF WHAT CI SAID (R893). A conclusion is one word, and a green
    // run can still be reporting something: the Node 20 runtime deprecation rode
    // in the annotations of every run this repository could see while every
    // conclusion beside them said `success`. Fetched above, because the census
    // needs it; printed here, because this is where a reader looks for it.
    lines.extend(notes);
    lines.extend(ci_state::annotation_report(sha, declared, &read));

    // WHAT EACH JOB COST, AGAINST WHAT IT DECLARED (R1245). The budget is written
    // in the workflow and read by `ci-plan`; what the job took is in the answer
    // already in hand. Neither half is new and nothing joined them, which is how
    // R1229 changed the work of a job's longest step, left the number alone, and
    // learned about it from a cancellation.
    //
    // NO EXTRA CALL: the stamps ride on the check rows this reporter already
    // fetched, and the budgets come off the tracked workflow files.
    //
    // AND THE READING IS THE FALLIBLE ONE, because a reporter is not a law: the
    // asserting reader beside it is right to die where a repository tracks no
    // workflow at all, and this program runs in whatever tree a push happens in.
    //
    // AND THE RETIRED CHECKS ARE HANDED IN (R1260): a run a later push cancelled
    // is nine jobs stamped with one wall clock, and holding that against nine
    // budgets is a number about queueing wearing a number about cost's clothes.
    // `retired` is the set R1242 already computes for the census above.
    let (budgets, unreadable) = ci_plan::readable_job_budgets(root);
    let (spent, mut unread) = ci_state::spent_against_budgets(&checks, &budgets, &retired);
    unread.extend(
        unreadable
            .into_iter()
            .map(|why| ci_state::Unmeasured::Workflow { why }),
    );
    if budgets.is_empty() {
        // A REFUSAL RATHER THAN SILENCE: no workflow read means no job's cost was
        // held against anything, and a block that simply vanished would read as
        // "nothing was close to its budget".
        lines.push(
            "NOT MEASURED no workflow of this repository was readable from here, so no \
             job's cost was held against a budget"
                .to_string(),
        );
        lines.extend(
            unread
                .into_iter()
                .map(|why| format!("  NOT MEASURED {why}")),
        );
    } else {
        lines.extend(ci_state::budget_report(&spent, &unread));
    }

    // AND WHAT IT COST LAST TIME (R1260). The block above is a LEVEL — a share of
    // the budget, on this one commit — and nothing kept it, so asking whether that
    // share is where the job has always sat meant a person holding two screens
    // side by side. This keeps the number and reads the record back.
    //
    // OUTSIDE THE BRANCH ABOVE ON PURPOSE: a tree whose workflows would not read
    // measures nothing new, and the history it already holds is still the answer
    // to what earlier pushes cost. The recording declines itself when there is
    // nothing measured to record.
    lines.extend(ci_state::history::kept_report(
        root,
        sha,
        &checks,
        &spent,
        &Github { root },
    ));
    lines
}

/// GitHub, asked what one job of one commit did step by step.
///
/// THE WHOLE OF WHAT THIS HOLDS IS THE TWO CALLS. Which commits to ask about,
/// what the answer means and every sentence built from it live in
/// [`ci_state::history`], where a test can reach them — the rule this file's own
/// header states and the reason `ci_state::history::StepsOf` is a trait.
///
/// TWO CALLS BECAUSE A COMMIT DOES NOT NAME A JOB. The check rows carry the job
/// in their `details_url`, which is the same route [`steps_of`] takes for the
/// commit being reported on; that one has its rows in hand already and this one
/// is asking about a commit from the record, which may be weeks back.
struct Github<'a> {
    root: &'a Path,
}

impl ci_state::history::StepsOf for Github<'_> {
    fn steps_of(&self, commit: &str, check: &str) -> Result<Vec<ci_state::Step>, String> {
        let answer = gh(self.root, &checks_query(commit))?;
        let checks = checks_in(commit, &answer)?;
        let row = checks
            .iter()
            .find(|row| row.name == check)
            .ok_or_else(|| format!("that commit has no check named `{check}`"))?;
        let job = job_of(&row.details_url).ok_or_else(|| {
            format!(
                "`{check}` on that commit is behind no Actions job — its details are at {}",
                row.details_url
            )
        })?;
        let body = gh(self.root, &steps_query(job))?;
        steps_in(job, &body)
    }
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
