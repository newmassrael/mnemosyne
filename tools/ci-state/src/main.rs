//! Ask GitHub what CI said about one commit, and print it.
//!
//! WHAT IS LEFT HERE IS THE PART A SUITE CANNOT REACH: a process, its exit
//! status, and its two streams. What to ask for and what the answer means both
//! live in the library, because a decision in `main.rs` has no reader — R1129
//! measured three gates whose `main.rs` carried a decision nothing was running,
//! and R1126 moved a neighbouring reporter's words out for the same reason.
//!
//! THREE EXIT CODES, AND EACH ONE HAS A READER. `0` means a report was printed
//! and there is nothing to refuse, INCLUDING the reports "gh is not installed",
//! "gh could not reach GitHub" and "GitHub's answer is not a shape I can read":
//! not being able to look is not a violation, and a machine that cannot reach
//! GitHub must still be able to push. `2` means no report could be produced at
//! all — bad usage — and `.githooks/pre-push` prints a different sentence for
//! it, because a check that stays quiet when it cannot answer is
//! indistinguishable from one that answered "fine".
//!
//! `1` MEANS THE COMMIT IS RED AND THE PUSH HAS NOT SAID SO (R1297), and it is
//! new. This program reported and never blocked for six hundred rounds, and the
//! argument for that was read off the history: the two pushes that fixed a red
//! CI (R888, R889) were made deliberately while it was red, and a gate would
//! have been wrong both times. What that argument could not distinguish is
//! KNOWING from NOT KNOWING — and on 2026-09-02 the distinction cost two rounds
//! in a row. `pushG.log` and `pushH.log` of that session both carry this
//! program's `^^ the commit you are building on is RED`, with the failing job,
//! the step that ended it and its annotation; both pushes went out anyway,
//! because a push is judged by its exit status and the verdict was riding on
//! stderr behind a `0`. Naming the red is what R888 and R889 could have done in
//! one keystroke, and what R1295 could not have done at all.
//!
//! SO THE SEMANTICS IS UNCHANGED WHERE IT WAS ARGUED FOR AND SHARPENED WHERE IT
//! WAS NOT: a push that is about a red still goes through, and a push that has
//! not looked at one no longer does.

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
    let said = state_of(&root, sha);
    for line in &said.lines {
        println!("ci-state: {line}");
    }
    // THE VERDICT LEAVES THIS PROGRAM IN THE EXIT STATUS AND NOT ONLY IN THE
    // PROSE (R1297). Which line a reader happens to catch is not a property this
    // program can hold; which code it exited with is.
    if said.refused {
        std::process::exit(1);
    }
}

/// Everything this reporter has to say about one commit, and whether it refuses.
///
/// THE TWO TRAVEL TOGETHER because they are read off the same answer. A caller
/// that had to ask twice would ask `gh` twice, and the second answer can differ
/// from the first — which is how a report and a verdict about "the same" commit
/// come to disagree.
struct Report {
    lines: Vec<String>,
    refused: bool,
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
///
/// AND NONE OF THE THREE REFUSES THE PUSH (R1297). Not being able to look is not
/// a red, and a machine with no `gh` that could never push would be a gate that
/// took the repository hostage. The red it did not see is not lost either: it is
/// still the state of that commit at the next push made from a machine that can
/// ask.
fn state_of(root: &Path, sha: &str) -> Report {
    let unread = |why: String| Report {
        lines: vec![why],
        refused: false,
    };
    let answer = match gh(root, &checks_query(sha)) {
        Ok(answer) => answer,
        Err(why) => return unread(format!("NOTE CI state for {sha} is unknown — {why}")),
    };
    let checks = match checks_in(sha, &answer) {
        Ok(checks) => checks,
        Err(why) => return unread(format!("NOTE {why}")),
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
    let (read, notes) = annotations_of(
        root,
        checks.iter().filter(|c| c.output.annotations_count > 0),
    );

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

    // AND WHICH JOBS HAVE SAID NOTHING AT ALL LATELY (R1304). Every other block
    // here reads a VERDICT; this one reads the absence of one, and the two are
    // not the same question. A job that is cancelled on every push produces no
    // conclusion, so it is not red, so nothing above mentions it — and it is then
    // indistinguishable from a job that keeps passing. R1303 found a law that had
    // FAILED on live code sitting unread on `main` for two rounds behind exactly
    // that.
    //
    // NO `gh` CALL: the record this tree already keeps holds, per commit, the
    // jobs whose cost could be held against a budget — which is to say the jobs
    // that concluded. The population it is subtracted from is what the workflows
    // declare UNCONDITIONALLY, because a job behind a `paths:` filter is supposed
    // to be quiet and reporting it would be a finding nobody can clear.
    let (kept, _) = ci_state::history::kept_in(root);
    let (expected, unread) = ci_plan::jobs_on_every_push(root);
    if expected.is_empty() {
        // A POPULATION THAT CAME BACK EMPTY IS NOT AN ANSWER OF "NONE".
        lines.push(format!(
            "NOT MEASURED which jobs have gone quiet — no workflow of this repository \
             declares a job on every push, as read from here{}",
            match unread.is_empty() {
                true => String::new(),
                false => format!(" ({})", unread.join("; ")),
            }
        ));
    } else {
        let quiet = ci_state::history::quiet_jobs(&kept, &expected, ci_state::history::QUIET_FOR);
        if quiet.is_empty() {
            lines.push(format!(
                "every one of the {} job(s) this repository runs on every push has \
                 concluded within the last {} recorded commit(s)",
                expected.len(),
                ci_state::history::QUIET_FOR
            ));
        } else {
            lines.push(format!(
                "^^ {} of {} job(s) run on every push have concluded on NONE of the \
                 last {} recorded commit(s) — no verdict from them is not the same \
                 as a good one:",
                quiet.len(),
                expected.len(),
                ci_state::history::QUIET_FOR
            ));
            for job in &quiet {
                lines.push(format!("  silent: {job}"));
            }
        }
    }

    // AND LAST, WHETHER THIS PUSH MAY GO OVER WHAT WAS JUST PRINTED (R1297).
    // Read from the SAME `checks` and the SAME `retired` set the census above was
    // phrased from — asking GitHub a second time would let the report and the
    // verdict be about two different answers.
    //
    // THE VARIABLE IS READ HERE AND NOWHERE ELSE, so the one place that decides
    // is the one place that has the reds in hand.
    // AND THE PENDING TAIL BEHIND IT IS WALKED (R1300). Asking only about
    // `origin/main` left the hole R1297 named: a commit that was still PENDING
    // when the next push went over it is never any later push's base, so its run
    // goes red afterwards with no reader at all. The walk stops at the first
    // JUDGED commit, which is where verdicts start existing again.
    let mut walk = vec![ci_state::Walked {
        sha: sha.to_string(),
        checks: checks.clone(),
        superseded: retired.clone(),
    }];
    if !ci_state::judged(&checks) {
        match parents_of(root, sha) {
            Ok(parents) => {
                let mut reached = false;
                for parent in &parents {
                    match lean_state(root, parent) {
                        Ok(step) => {
                            reached = ci_state::judged(&step.checks);
                            walk.push(step);
                            if reached {
                                break;
                            }
                        }
                        // A WALK THAT COULD NOT SEE IS SAID OUT LOUD, never
                        // treated as a walk that saw nothing.
                        Err(why) => {
                            lines.push(format!(
                                "NOTE the walk stopped at {} — {why}",
                                ci_state::short(parent)
                            ));
                            reached = true;
                            break;
                        }
                    }
                }
                if !reached {
                    lines.push(format!(
                        "^^ WALKED {} commit(s) back from {} without reaching one whose \
                         checks concluded. Everything behind that is unasked, and this \
                         gate is not telling you it is fine — it is telling you CI has \
                         not finished a run in that many pushes",
                        walk.len(),
                        ci_state::short(sha)
                    ));
                }
            }
            Err(why) => lines.push(format!("NOTE could not walk back from {sha} — {why}")),
        }
    }
    let outstanding = ci_state::outstanding_reds(&walk);
    if walk.len() > 1 {
        lines.push(format!(
            "walked {} commit(s) to the first judged one; {} red(s) outstanding across them",
            walk.len(),
            outstanding.len()
        ));
        for (at, job) in &outstanding {
            lines.push(format!("  outstanding on {}: {job}", ci_state::short(at)));
        }
    }

    // AND LAST, WHETHER THIS PUSH MAY GO OVER WHAT WAS JUST PRINTED (R1297).
    // Read from the SAME `checks` and the SAME `retired` set the census above was
    // phrased from — asking GitHub a second time would let the report and the
    // verdict be about two different answers.
    //
    // THE VARIABLE IS READ HERE AND NOWHERE ELSE, so the one place that decides
    // is the one place that has the reds in hand.
    //
    // THE COMMIT TRAVELS WITH THE JOB (R1301). R1300 dropped the sha here and
    // handed on job names alone, so one job red on two commits was ONE name and
    // saying it once discharged both — a push had no way to say it had read the
    // one and not the other.
    let mut reds = outstanding.clone();
    reds.sort();
    reds.dedup();
    let given = std::env::var(ci_state::ACKNOWLEDGEMENT).ok();
    let standing = ci_state::acknowledgement(&reds, given.as_deref());
    let refusal = ci_state::refusal(sha, &standing);
    let refused = !refusal.is_empty();
    lines.extend(refusal);
    Report { lines, refused }
}

/// What these checks' annotations say, paired with the check that said it.
///
/// ONE COPY, AND THIS REPOSITORY'S OWN GATE IS WHY IT IS ONE (R1300). The walk
/// needed the same read the base report already did, and writing it twice was
/// caught at the commit: an existing injection anchors on the line that pairs an
/// annotation with the check that said it, and the harness refused a manifest
/// whose anchor had come to match TWICE. Re-anchoring would have made the sweep
/// legal and left two copies of a read that must agree — the two-write-paths
/// shape this project's own `CLAUDE.md` forbids — so the anchor stayed and the
/// duplicate went. The prose here does not spell that line either, for the same
/// reason: quoting an anchor is one of the ways to become a second match.
///
/// PAIRED WITH THE CHECK THAT SAID IT (R1238). The name was thrown away here
/// once, and getting it back cost three `gh api` calls by hand the day a red
/// commit carried two failing jobs and five flat lines.
///
/// NAMED, AND THE REST STILL READ: one check whose annotations cannot be
/// fetched must not take the other checks' annotations down with it, and the
/// shortfall shows up in the "N distinct of D reported" line either way.
fn annotations_of<'a>(
    root: &Path,
    asking: impl Iterator<Item = &'a Check>,
) -> (Vec<Said>, Vec<String>) {
    let mut read: Vec<Said> = Vec::new();
    let mut notes: Vec<String> = Vec::new();
    for check in asking {
        match gh(root, &annotations_query(check.id))
            .and_then(|body| annotations_in(check.id, &body))
        {
            Ok(some) => read.extend(some.into_iter().map(|annotation| Said {
                check: check.name.clone(),
                annotation,
            })),
            Err(why) => notes.push(format!("NOTE {why}")),
        }
    }
    (read, notes)
}

/// How far back this walk will go before it says it could not find out.
///
/// A BOUND THAT EXCUSES NOTHING TODAY, measured rather than picked: the walk from
/// `609101f` reaches a judged commit in 2, and the deepest thing in the recent
/// history — the distance to an all-success commit, which is NOT this walk's
/// stopping rule — is 10. Hitting this is therefore a statement about CI, not
/// about the cap, and it is printed as one.
const WALK_CAP: usize = 40;

/// The commits behind this one, newest first.
fn parents_of(root: &Path, sha: &str) -> Result<Vec<String>, String> {
    let answer = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-list", &format!("--max-count={}", WALK_CAP + 1), sha])
        .output()
        .map_err(|why| format!("git could not be run: {why}"))?;
    if !answer.status.success() {
        return Err(format!(
            "git rev-list refused: {}",
            String::from_utf8_lossy(&answer.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&answer.stdout)
        .lines()
        .skip(1)
        .map(str::to_string)
        .collect())
}

/// What CI said about one commit the walk passed, and nothing more.
///
/// LEAN ON PURPOSE. The base commit gets the whole report — steps, annotations,
/// budgets, trends — because that is what a person is being told about. A commit
/// the walk merely passes needs exactly two facts: did anything fail, and was
/// that failure a run a later push retired. Everything else would multiply the
/// most expensive part of this program by the depth of the walk.
fn lean_state(root: &Path, sha: &str) -> Result<ci_state::Walked, String> {
    let answer = gh(root, &checks_query(sha))?;
    let checks = checks_in(sha, &answer)?;
    // THE ANNOTATIONS ONLY WHERE A FAILURE COULD BE SOMEBODY ELSE'S (R1242). A
    // commit with nothing failing needs no second call, which is what keeps the
    // ordinary walk one request per commit.
    let (read, _notes) = annotations_of(
        root,
        checks
            .iter()
            .filter(|check| is_failing(check) && check.output.annotations_count > 0),
    );
    let superseded = ci_state::superseded_checks(&checks, &read);
    Ok(ci_state::Walked {
        sha: sha.to_string(),
        checks,
        superseded,
    })
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
