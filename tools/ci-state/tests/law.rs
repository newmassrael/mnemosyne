//! What this reporter says, and what it refuses to leave unsaid.
//!
//! THE READING HAS ITS OWN FILE (`github.rs`, against recorded bodies). This one
//! is about the sentences: which rows get printed, which get counted, and which
//! of the four verdicts a commit lands on. Both halves are needed and neither
//! substitutes — a reporter that parses GitHub perfectly and prints the wrong
//! sentence is the failure R1125 shipped one gate over, where an oracle matching
//! a SUBSTRING of the failure output agreed with the output it existed to refuse.

use ci_state::{
    annotation_report, one_line, report, verdict, Annotation, Check, Output, Retired, Said, Spent,
    Step, Verdict,
};

const SHA: &str = "2d630331b1279e3b7a28985876b53ef0b07fbe77";

/// The census when nothing was superseded — the ordinary case, which is every
/// case here that is not about supersession.
fn census(sha: &str, checks: &[Check]) -> Vec<String> {
    report(sha, checks, &Retired::default())
}

/// A check with a conclusion, spelled the way GitHub spells one.
fn check(id: u64, name: &str, conclusion: Option<&str>, annotations: u64) -> Check {
    check_in_run(1, id, name, conclusion, annotations)
}

/// The same, in a NAMED RUN — because whether a later push retired a check is a
/// property of the run it belongs to and not of the check.
fn check_in_run(
    run: u64,
    id: u64,
    name: &str,
    conclusion: Option<&str>,
    annotations: u64,
) -> Check {
    Check {
        id,
        name: name.to_string(),
        // The shape GitHub writes for an Actions job, with this row's own id in
        // it — the same equality `github.rs` asserts against the recording.
        details_url: format!("https://github.com/o/r/actions/runs/{run}/job/{id}"),
        head_sha: SHA.to_string(),
        status: if conclusion.is_some() {
            "completed".to_string()
        } else {
            "in_progress".to_string()
        },
        conclusion: conclusion.map(str::to_string),
        // A MINUTE, SO EVERY CASE THAT IS NOT ABOUT COST HAS A READABLE ONE. The
        // cases about what a job took build their own pair; the point here is that
        // no case gets a duration by accident, which is what `None` on both ends
        // would have made every one of them do.
        started_at: Some("2026-08-18T13:40:00Z".to_string()),
        completed_at: Some("2026-08-18T13:41:00Z".to_string()),
        output: Output {
            annotations_count: annotations,
        },
    }
}

fn note(level: &str, message: &str) -> Annotation {
    Annotation {
        annotation_level: level.to_string(),
        message: message.to_string(),
    }
}

/// One annotation as a check reported it (R1238).
fn said(check: &str, level: &str, message: &str) -> Said {
    Said {
        check: check.to_string(),
        annotation: note(level, message),
    }
}

/// The census names every row, and the lines name every row that is not routine.
///
/// BOTH HALVES OR NEITHER IS HONEST. Printing all nine rows on every green push
/// trains a reader to skip the block; printing only the failures says nothing
/// about how much was looked at. The counts are what makes the omission legible,
/// which is the rule the annotation cap below already followed.
#[test]
fn the_census_names_every_row_and_the_lines_name_every_row_that_is_not_success() {
    let checks = [
        check(1, "validate", Some("success"), 0),
        check(
            2,
            "every cache declared is one CI keeps",
            Some("failure"),
            1,
        ),
        check(3, "every compilation is one job's", Some("skipped"), 0),
        check(4, "MSRV", Some("success"), 0),
    ];
    let lines = census(SHA, &checks);
    assert!(
        lines[0].contains("4 check(s)")
            && lines[0].contains("2 success")
            && lines[0].contains("1 failure")
            && lines[0].contains("1 skipped"),
        "the census accounts for every row: {}",
        lines[0]
    );
    let body = lines[1..].join("\n");
    assert!(
        body.contains("every cache declared is one CI keeps") && body.contains("failure"),
        "the failure is named: {body}"
    );
    assert!(
        body.contains("every compilation is one job's"),
        "and so is the skip, which is not routine even though it is not red: {body}"
    );
    assert!(
        !body.contains("validate") && !body.contains("MSRV"),
        "the successes are counted rather than listed: {body}"
    );
}

/// A red commit is SAID to be red, and told that a push ABOUT the red still goes.
///
/// THIS LAW USED TO ASSERT `Not blocking` (R890), and R1297 changed what it
/// asserts because it changed what is true. The old semantics was read off the
/// history — R888 and R889 were both pushes made deliberately while CI was red,
/// to fix it — and it was right about restraint and wrong about knowledge: those
/// two pushes KNEW, and the gate could not tell them from a push that had not
/// looked. So the sentence still promises that fixing a red is not blocked, and
/// now says what makes the difference.
#[test]
fn a_red_commit_is_told_it_is_red_and_that_naming_it_is_what_lets_a_fix_through() {
    let checks = [check(1, "validate", Some("failure"), 0)];
    let said = census(SHA, &checks).join("\n");
    assert!(said.contains("is RED"), "{said}");
    assert!(
        said.contains("Fixing it is itself a push"),
        "a push that is ABOUT the red is still not one to stop: {said}"
    );
    assert!(
        said.contains("says which red"),
        "and what separates it from a push that never looked: {said}"
    );
}

/// A clear commit is not told anything about being red.
///
/// THE CONTROL. A reporter that always printed the warning would be as useless as
/// one that never did, and only this direction says which of the two it is.
#[test]
fn a_clear_commit_is_not_warned_about_anything() {
    let checks = [
        check(1, "validate", Some("success"), 0),
        check(2, "MSRV", Some("neutral"), 0),
    ];
    assert_eq!(verdict(&checks), Verdict::Clear);
    let said = census(SHA, &checks).join("\n");
    assert!(!said.contains("RED"), "{said}");
    assert!(
        said.contains("2 check(s)"),
        "and it still says how much it looked at: {said}"
    );
}

/// A commit whose checks have not finished is neither red nor clear.
///
/// THE THIRD ANSWER THE PROJECTION COULD NOT GIVE. `(.conclusion // "-")` wrote a
/// dash for a check still running and then asked whether any line ended in one of
/// four failing words, so "still running" and "green" were one answer.
#[test]
fn a_commit_still_running_is_neither_red_nor_clear() {
    let checks = [
        check(1, "validate", Some("success"), 0),
        check(2, "MSRV", None, 0),
    ];
    assert_eq!(verdict(&checks), Verdict::Pending);
    let said = census(SHA, &checks).join("\n");
    assert!(!said.contains("RED"), "nothing has failed yet: {said}");
    assert!(
        said.contains("still running"),
        "and the reader is told the answer is not in yet: {said}"
    );
}

/// A failure outweighs an unfinished check.
///
/// THE ORDER MATTERS AND IT IS ASSERTED: a commit with one failed job and one job
/// still going is RED now, and a reporter that answered `Pending` would ask
/// somebody to wait for news that has already arrived.
#[test]
fn a_failure_beside_an_unfinished_check_is_red_now() {
    let checks = [
        check(1, "validate", Some("failure"), 0),
        check(2, "MSRV", None, 0),
    ];
    assert_eq!(verdict(&checks), Verdict::Red);
}

/// A commit nothing ran on says exactly that.
#[test]
fn a_commit_with_no_checks_says_nothing_has_run_on_it() {
    let said = census(SHA, &[]).join("\n");
    assert!(
        said.contains("no CI checks") && said.contains("2d630331"),
        "and it names the commit: {said}"
    );
    assert!(
        !said.contains("RED"),
        "a commit nothing ran on is not a commit that failed: {said}"
    );
}

/// Every line names the commit by its first eight characters, and no more.
#[test]
fn the_commit_is_printed_short_and_it_is_the_commit_asked_about() {
    let said = census(SHA, &[check(1, "validate", Some("success"), 0)]).join("\n");
    assert!(said.contains("2d630331"), "{said}");
    assert!(
        !said.contains(SHA),
        "the whole sha is forty characters of noise in a hook's output: {said}"
    );
}

/// One job of a workflow, as `ci-plan` reads one.
fn job(id: &str, shown_as: Option<&str>, timeout: Option<&str>) -> (String, ci_plan::JobBudget) {
    (
        "recorded.yml".to_string(),
        ci_plan::JobBudget {
            id: id.to_string(),
            shown_as: shown_as.map(str::to_string),
            timeout: timeout.map(str::to_string),
        },
    )
}

/// The ordinary commit, where no later push has retired anything.
fn nothing_retired() -> Retired {
    Retired::default()
}

/// A commit's retired checks when a later push ended these runs.
fn retired_by_a_later_push(names: &[&str]) -> Retired {
    Retired {
        by_later_push: names.iter().map(|name| (*name).to_string()).collect(),
        never_ran: std::collections::BTreeSet::new(),
    }
}

/// A commit's retired checks when these jobs were cancelled before a step ran.
fn retired_for_never_running(names: &[&str]) -> Retired {
    Retired {
        by_later_push: std::collections::BTreeSet::new(),
        never_ran: names.iter().map(|name| (*name).to_string()).collect(),
    }
}

/// A check with its two stamps, which is what a cost is read out of.
fn ran(name: &str, started: &str, completed: Option<&str>) -> Check {
    let mut row = check(1, name, Some("success"), 0);
    row.started_at = Some(started.to_string());
    row.completed_at = completed.map(str::to_string);
    row
}

/// The same, ended by a cancellation.
///
/// WHAT A RETIRED CHECK ACTUALLY IS. Both reasons a check says nothing about its
/// commit — a later push ended its run, or it was cancelled in the queue — are
/// cancellations, and the budget block only retires a check that did not pass: a
/// success that shares a name with a retired one is still a measurement. A case
/// about retiring must therefore build a cancelled check, not a successful one
/// marked as retired, which is a state GitHub never produces.
fn cancelled_after(name: &str, started: &str, completed: Option<&str>) -> Check {
    let mut row = ran(name, started, completed);
    row.conclusion = Some("cancelled".to_string());
    row
}

/// The stamp GitHub writes, read as seconds — and anything else refused.
///
/// A CALENDAR IS THE ONE PIECE OF ARITHMETIC HERE THAT CAN BE SUBTLY WRONG, so it
/// is held against values that can be checked by hand: the epoch itself, the day
/// after a leap day, the first second of a year, and the stamp this repository's
/// own recording carries.
#[test]
fn githubs_stamp_reads_as_seconds_and_nothing_else_does() {
    assert_eq!(ci_state::epoch_seconds("1970-01-01T00:00:00Z"), Some(0));
    assert_eq!(
        ci_state::epoch_seconds("1970-01-02T00:00:00Z"),
        Some(86_400)
    );
    assert_eq!(
        ci_state::epoch_seconds("2000-03-01T00:00:00Z"),
        Some(951_868_800),
        "the day after a leap day in a century year that IS a leap year"
    );
    assert_eq!(
        ci_state::epoch_seconds("2024-02-29T12:00:00Z"),
        Some(1_709_208_000),
        "a leap day itself"
    );
    assert_eq!(
        ci_state::epoch_seconds("2026-01-01T00:00:00Z"),
        Some(1_767_225_600),
        "the first second of a year"
    );

    for refused in [
        "2026-08-18T13:40:00",      // no zone
        "2026-08-18T13:40:00.5Z",   // fractional
        "2026-08-18t13:40:00Z",     // lower case
        "2026-08-18T13:40:00+0100", // an offset
        "2026-13-01T00:00:00Z",     // no such month
        "2026-08-18T24:00:00Z",     // no such hour
        "",
    ] {
        assert_eq!(
            ci_state::epoch_seconds(refused),
            None,
            "`{refused}` is not the one shape this reader claims to read, and a \
             plausible number out of it is worse than none"
        );
    }
}

/// A pair of stamps reads as a duration, and an inverted pair reads as nothing.
///
/// GITHUB REALLY WRITES ONE. The recording this crate keeps has a skipped job
/// starting at `14:01:37` and completing at `14:01:36` — a duration clamped to
/// zero there is a number nobody wrote, sitting where a measurement is expected.
#[test]
fn a_pair_of_stamps_is_a_duration_and_an_inverted_pair_is_a_refusal() {
    assert_eq!(
        ci_state::seconds_between("2026-08-18T13:40:53Z", "2026-08-18T14:00:50Z"),
        Some(1_197)
    );
    assert_eq!(
        ci_state::seconds_between("2026-08-10T14:01:37Z", "2026-08-10T14:01:36Z"),
        None,
        "the skipped job in this crate's own recording ends before it starts"
    );
    assert_eq!(
        ci_state::seconds_between("2026-08-18T13:40:53Z", "nonsense"),
        None
    );
}

/// What a job took is held against what its job declares, by the name a check
/// carries — and every way that cannot be done is a kind of its own.
#[test]
fn a_jobs_cost_is_joined_to_its_budget_by_the_name_a_check_carries() {
    let budgets = [
        job("validate", None, Some("90")),
        job(
            "cache-budget",
            Some("every cache declared is one CI keeps"),
            Some("60"),
        ),
        job(
            "expressive",
            Some("an expression"),
            Some("${{ env.BUDGET }}"),
        ),
    ];
    let checks = [
        ran(
            "validate",
            "2026-08-18T13:40:53Z",
            Some("2026-08-18T14:00:50Z"),
        ),
        ran(
            "every cache declared is one CI keeps",
            "2026-08-18T13:40:00Z",
            Some("2026-08-18T14:34:00Z"),
        ),
        ran(
            "an expression",
            "2026-08-18T13:40:00Z",
            Some("2026-08-18T13:41:00Z"),
        ),
        ran(
            "posted by another app",
            "2026-08-18T13:40:00Z",
            Some("2026-08-18T13:41:00Z"),
        ),
        ran("validate", "2026-08-18T13:40:00Z", None),
    ];
    let (spent, unread) = ci_state::spent_against_budgets(&checks, &budgets, &nothing_retired());

    assert_eq!(spent.len(), 2, "{spent:?}");
    assert_eq!(spent[0].took, 1_197);
    assert_eq!(spent[0].percent(), 22, "1197s of 90m");
    assert_eq!(
        spent[1].percent(),
        90,
        "54m of 60m — and this is the one a reader has to be told about"
    );

    let said: Vec<String> = unread.iter().map(ToString::to_string).collect();
    assert!(
        said.iter().any(|why| why.contains("cannot evaluate")),
        "a budget written as an expression is a bound and not a number: {said:?}"
    );
    assert!(
        said.iter()
            .any(|why| why.contains("no job of this repository")),
        "a check no job declares is named rather than dropped: {said:?}"
    );
    assert!(
        said.iter().any(|why| why.contains("has not finished")),
        "and a job still running is a state of the world: {said:?}"
    );
}

/// A run a later push cancelled is not a cost, and its jobs are counted rather
/// than measured.
///
/// THE NUMBERS HERE ARE A REAL COMMIT OF THIS REPOSITORY. `1ddeff31` carries nine
/// checks, every one of them `cancelled` by the next push, every one of them
/// stamped 11:39:12 to 12:37:16 — ONE wall clock, shared by nine jobs that spent
/// most of it queued. R1242 taught the census to call that no verdict; the block
/// that reads what a job COST was written three rounds later and never learned
/// it, so it held 3484 seconds against a thirty-minute budget and reported
/// `MSRV` at 193% — a job that would have been killed at thirty if it had ever
/// been running. R1260 found it by keeping the numbers, which is the whole
/// argument for keeping them.
#[test]
fn a_run_a_later_push_cancelled_is_counted_rather_than_held_against_a_budget() {
    let budgets = [
        job(
            "msrv",
            Some("MSRV (workspace.package.rust-version)"),
            Some("30"),
        ),
        job("validate", None, Some("90")),
    ];
    let checks = [
        cancelled_after(
            "MSRV (workspace.package.rust-version)",
            "2026-08-19T11:39:12Z",
            Some("2026-08-19T12:37:16Z"),
        ),
        cancelled_after(
            "validate",
            "2026-08-19T11:39:12Z",
            Some("2026-08-19T12:37:16Z"),
        ),
    ];
    let names: Vec<&str> = checks.iter().map(|check| check.name.as_str()).collect();
    let retired = retired_by_a_later_push(&names);

    let (measured, _) = ci_state::spent_against_budgets(&checks, &budgets, &nothing_retired());
    assert_eq!(
        measured.iter().map(Spent::percent).collect::<Vec<_>>(),
        vec![193, 64],
        "the reading this replaces: a wall clock nine jobs shared, held against \
         each of their budgets"
    );

    let (spent, unread) = ci_state::spent_against_budgets(&checks, &budgets, &retired);
    assert!(
        spent.is_empty(),
        "nothing a later push ended is a measurement: {spent:?}"
    );
    let said = ci_state::budget_report(&spent, &unread).join("\n");
    assert!(
        said.contains("2 job(s) were ended by a LATER PUSH"),
        "counted, because a concurrency group cancels a whole run at once and \
         nine lines about the entirely normal is a screen of alarm: {said}"
    );
    assert!(
        !said.contains("closest to its budget"),
        "and no job is named as closest to anything, because none was measured: \
         {said}"
    );
    assert!(
        !said.contains("193"),
        "above all the number itself is gone: {said}"
    );
}

/// A job cancelled while it queued has a clock and no cost.
///
/// MEASURED ON `84dcad2c`: eleven jobs stamped from 11:49 to 13:15 on 2026-09-15,
/// which is 85 minutes of waiting for a runner that never came. Held against their
/// budgets that reads `every test compiled is one CI runs` at 94% of ninety
/// minutes — a number about a queue, in a cost's clothes, and the same shape R1260
/// found for a run a later push ended. They are counted under a sentence of their
/// own because the reason they say nothing is different.
#[test]
fn a_job_cancelled_before_it_ran_is_counted_rather_than_held_against_a_budget() {
    let budgets = [job(
        "compile",
        Some("every test compiled is one CI runs"),
        Some("90"),
    )];
    let checks = [cancelled_after(
        "every test compiled is one CI runs",
        "2026-09-15T11:49:54Z",
        Some("2026-09-15T13:15:22Z"),
    )];

    let (measured, _) = ci_state::spent_against_budgets(&checks, &budgets, &nothing_retired());
    assert_eq!(
        measured.iter().map(Spent::percent).collect::<Vec<_>>(),
        vec![94],
        "the reading this replaces: 85 minutes of queueing held against a 90 minute budget"
    );

    let retired = retired_for_never_running(&["every test compiled is one CI runs"]);
    let (spent, unread) = ci_state::spent_against_budgets(&checks, &budgets, &retired);
    assert!(
        spent.is_empty(),
        "a job that never left the queue is not a measurement: {spent:?}"
    );
    let said = ci_state::budget_report(&spent, &unread).join("\n");
    assert!(
        said.contains("1 job(s) were cancelled before any step of them ran"),
        "counted, under its own sentence: {said}"
    );
    assert!(
        !said.contains("LATER PUSH"),
        "and not under the later push's, because that is not why it says nothing: {said}"
    );
    assert!(
        !said.contains("94"),
        "above all the number itself is gone: {said}"
    );
}

/// A skipped job is not a job that cost nothing.
///
/// THE MOST ORDINARY CASE IN THIS REPOSITORY, and the one R1260's record made
/// matter. The workflow skips a job whose inputs did not change, GitHub stamps
/// such a job's start and completion at the SAME INSTANT, and a reader that
/// subtracted them got zero seconds — which is not a cost, it is the absence of a
/// measurement. Measured on the first full record: 9 of the 23 rows for `every
/// compilation is one job's` were skips, so an endpoint of that job's curve
/// landing on one was a matter of time, and a movement from or to zero is what it
/// would then have printed.
#[test]
fn a_skipped_job_is_not_a_job_that_cost_nothing() {
    let budgets = [job(
        "compile",
        Some("every compilation is one job's"),
        Some("60"),
    )];
    let mut skipped = ran(
        "every compilation is one job's",
        "2026-08-19T05:32:26Z",
        Some("2026-08-19T05:32:26Z"),
    );
    skipped.conclusion = Some("skipped".to_string());

    let (spent, unread) = ci_state::spent_against_budgets(&[skipped], &budgets, &nothing_retired());
    assert!(
        spent.is_empty(),
        "a job that never ran has no duration to hold against a budget: {spent:?}"
    );
    let said = ci_state::budget_report(&spent, &unread).join("\n");
    assert!(
        said.contains("1 job(s) were skipped"),
        "counted, because a green push routinely has two of these: {said}"
    );
    assert!(
        !said.contains("0%"),
        "and the zero is nowhere on the page: {said}"
    );
}

/// What a job took is recorded with GitHub's own word for how it ended.
///
/// THE TWO READERS NEED DIFFERENT POPULATIONS and the record serves both, so the
/// word that tells them apart is kept rather than a flag one of them decided on.
#[test]
fn what_a_job_took_is_kept_beside_how_it_ended() {
    let budgets = [job("validate", None, Some("90"))];
    let mut failed = ran(
        "validate",
        "2026-08-19T05:10:14Z",
        Some("2026-08-19T05:15:45Z"),
    );
    failed.conclusion = Some("failure".to_string());
    let (spent, _) = ci_state::spent_against_budgets(&[failed], &budgets, &nothing_retired());
    assert_eq!(spent.len(), 1, "a job that RAN is measured: {spent:?}");
    assert_eq!(spent[0].took, 331, "the real 331s of `d412b06e`");
    assert_eq!(
        spent[0].conclusion, "failure",
        "and the level line is still entitled to it — what it is not is a point \
         on a cost curve: {spent:?}"
    );
}

/// A check name two workflows both declare is refused rather than joined.
///
/// `ci-plan`'s law makes a name unique WITHIN a workflow and can say nothing
/// across them, and a commit's checks carry no workflow at all — so the only
/// honest answer for a name two files declare is that this reader cannot say
/// which budget is the row's. A number about the wrong job is worse than none.
#[test]
fn a_name_two_workflows_declare_is_a_refusal_rather_than_the_first_one_found() {
    let budgets = [
        job("here", Some("shared"), Some("30")),
        (
            "other.yml".to_string(),
            ci_plan::JobBudget {
                id: "there".to_string(),
                shown_as: Some("shared".to_string()),
                timeout: Some("90".to_string()),
            },
        ),
    ];
    let checks = [ran(
        "shared",
        "2026-08-18T13:40:00Z",
        Some("2026-08-18T13:55:00Z"),
    )];
    let (spent, unread) = ci_state::spent_against_budgets(&checks, &budgets, &nothing_retired());
    assert!(spent.is_empty(), "{spent:?}");
    let said = unread
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        said.contains("recorded.yml") && said.contains("other.yml"),
        "{said}"
    );
}

/// The worst job is printed even when nothing is wrong.
///
/// A COUNT PUBLISHED ONLY ON FAILURE IS A COUNT NOBODY CAN WATCH FALL — the shape
/// R1241 measured one gate over. What this block is FOR is notice, so the number
/// has to be on the screen of an ordinary green push.
#[test]
fn the_closest_job_to_its_budget_is_printed_even_when_every_job_is_fine() {
    let spent = [
        ci_state::Spent {
            check: "quick".to_string(),
            took: 60,
            budget_minutes: 90,
            conclusion: "success".to_string(),
        },
        ci_state::Spent {
            check: "validate".to_string(),
            took: 2_003,
            budget_minutes: 90,
            conclusion: "success".to_string(),
        },
    ];
    let said = ci_state::budget_report(&spent, &[]).join("\n");
    assert!(said.contains("of 2 job(s) measured"), "{said}");
    assert!(
        said.contains("`validate` — 33m23s of 90m (37%)"),
        "the worst one, its clock and its share: {said}"
    );
    assert!(
        !said.contains("and it is close"),
        "nothing here is close, so nothing says it is: {said}"
    );
}

/// A job in the warning band gets a line of its own, and unfinished jobs are
/// counted rather than listed.
#[test]
fn a_job_near_its_budget_is_named_and_the_unfinished_are_counted() {
    let spent = [ci_state::Spent {
        check: "validate".to_string(),
        took: 4_500,
        budget_minutes: 90,
        conclusion: "success".to_string(),
    }];
    let unread = [
        ci_state::Unmeasured::NotFinished {
            check: "one".to_string(),
        },
        ci_state::Unmeasured::NotFinished {
            check: "two".to_string(),
        },
        ci_state::Unmeasured::NoSuchJob {
            check: "somebody else's".to_string(),
        },
    ];
    let said = ci_state::budget_report(&spent, &unread).join("\n");
    assert!(
        said.contains("(83%)") && said.contains("and it is close"),
        "83% is over the warning band, so the job gets its own line: {said}"
    );
    assert!(
        said.contains("2 job(s) have not finished"),
        "the ordinary state of a commit a push builds on is COUNTED, not six lines \
         of alarm: {said}"
    );
    assert!(
        !said.contains("`one`") && !said.contains("`two`"),
        "and counted means not also listed: {said}"
    );
    assert!(
        said.contains("NOT MEASURED") && said.contains("somebody else's"),
        "while every other kind is still named: {said}"
    );
}

/// GitHub's own sentence when a later push retires a run in flight.
const RETIRED: &str =
    "Canceling since a higher priority waiting request for mnemosyne-validate-refs/heads/main \
     exists";

/// A run a LATER PUSH cancelled is not this commit's failure.
///
/// MEASURED ON `74035d7` (2026-08-19). Three checks cancelled, one of them
/// twenty-seven minutes into `cargo test --workspace`, and the reason was the NEXT
/// push on the same ref. Every fact the reporter printed pointed at this commit —
/// `cancelled`, the step it stopped at, how many steps never ran — and the commit
/// had done nothing. What it needed was NO VERDICT.
#[test]
fn a_run_a_later_push_retired_is_no_verdict_rather_than_red() {
    let checks = [
        check_in_run(7, 1, "validate", Some("cancelled"), 1),
        check_in_run(7, 2, "every compilation is one job's", Some("cancelled"), 0),
    ];
    let read = vec![said("validate", "failure", RETIRED)];
    let retired = Retired {
        by_later_push: ci_state::superseded_checks(&checks, &read),
        ..Retired::default()
    };
    let said = report(SHA, &checks, &retired).join("\n");
    assert!(
        said.contains("NO VERDICT") && said.contains("LATER PUSH"),
        "{said}"
    );
    assert!(
        !said.contains("is RED"),
        "a commit whose run was retired by the next push is not a red commit, and \
         calling it one sends a reader to look for a defect that is not there:\n{said}"
    );
    assert!(
        said.contains("Read the later commit's run"),
        "and the advice is the one that is TRUE for this reason: the answer is on the \
         later commit, which is not what a job that never ran gets told:\n{said}"
    );

    // THE CHECK THAT NEVER STARTED IS COVERED TOO, and that is why supersession is
    // read per RUN. Two of the three cancelled checks on `74035d7` carried no
    // annotation at all — GitHub had nothing to annotate a job that never began —
    // so a reader asking each check for its own reason would have called one
    // superseded and left the others looking like this commit's failures.
    assert_eq!(
        retired.by_later_push.len(),
        2,
        "both checks of the retired run are retired, including the one with no \
         annotation of its own: {retired:?}"
    );
}

/// And a genuine failure beside a retired one is still red.
///
/// ⚠ THE OTHER HALF, without which the case above is satisfied by a reporter that
/// has simply stopped saying `RED`. Supersession belongs to a RUN, so a commit
/// carrying two runs can have one retired and one that really failed, and the
/// reader has to be told which is which rather than told the softer of the two.
#[test]
fn a_failure_in_another_run_is_still_red_beside_a_retired_one() {
    let checks = [
        check_in_run(7, 1, "validate", Some("cancelled"), 1),
        check_in_run(9, 2, "evidence replay", Some("failure"), 0),
    ];
    let read = vec![said("validate", "failure", RETIRED)];
    let retired = Retired {
        by_later_push: ci_state::superseded_checks(&checks, &read),
        ..Retired::default()
    };
    let said = report(SHA, &checks, &retired).join("\n");
    assert!(said.contains("is RED"), "{said}");
    assert!(
        said.contains("1 of the 2 that did not pass were merely superseded"),
        "and it says how much of the red was somebody else's, or a reader cannot \
         tell which check to look at:\n{said}"
    );
    assert_eq!(
        retired.by_later_push.into_iter().collect::<Vec<_>>(),
        vec!["validate".to_string()],
        "the failure in the OTHER run must not be swept up by the retired one"
    );
}

/// A step as GitHub spells one, with only the fields this reader depends on.
fn step(number: u64, name: &str, conclusion: Option<&str>) -> Step {
    Step {
        name: name.to_string(),
        number,
        status: if conclusion.is_some() {
            "completed".to_string()
        } else {
            "queued".to_string()
        },
        conclusion: conclusion.map(str::to_string),
        started_at: None,
        completed_at: None,
    }
}

/// A job cancelled while it queued ran nothing, and a job that ran something did.
///
/// MEASURED ON `84dcad2c` (2026-09-15): eleven jobs cancelled after 85 minutes
/// waiting for a runner, every one of them answering `steps: []` and a runner of
/// null or empty. The report called the commit RED and the push was refused over
/// eleven failures none of which had begun.
///
/// ⚠ THE LINE IS "A STEP CARRIES A CONCLUSION OTHER THAN `skipped`", and each side
/// of it is held here. `Set up job` ending `cancelled` is R1236's case — a runner
/// picked the job up and something stopped it, which is about THIS commit — so it
/// must stay a failure, and a reader that called every empty-looking job unrun
/// would have deleted that distinction.
#[test]
fn a_job_ran_nothing_exactly_when_no_step_of_it_has_a_conclusion() {
    assert!(
        ci_state::ran_nothing(&[]),
        "the recorded shape: a job cancelled in the queue answers an empty list"
    );
    assert!(
        ci_state::ran_nothing(&[
            step(1, "Set up job", Some("skipped")),
            step(2, "Run tests", None),
        ]),
        "steps nobody reached are not work: skipped and unfinished steps ran nothing"
    );
    assert!(
        !ci_state::ran_nothing(&[step(1, "Set up job", Some("cancelled"))]),
        "a runner picked it up and it was stopped in the first step, which is R1236's \
         case and a fact about this commit"
    );
    assert!(
        !ci_state::ran_nothing(&[
            step(1, "Set up job", Some("success")),
            step(2, "Run tests", Some("skipped")),
        ]),
        "one step that succeeded is a job that ran, whatever followed it"
    );
}

/// The recorded body of one of those jobs reads as a job that ran nothing.
///
/// AGAINST THE REAL ANSWER AND NOT A TYPED ONE, because the whole classification
/// stands on GitHub continuing to spell a queued-and-cancelled job as an empty step
/// list; this is the body `gh api` returned for job 104396410160 on 2026-10-01.
#[test]
fn the_recorded_body_of_a_job_cancelled_in_the_queue_ran_nothing() {
    let body = include_str!("job.never-started.json");
    let steps = ci_state::steps_in(104_396_410_160, body).expect("the recorded body reads");
    assert!(steps.is_empty(), "{steps:?}");
    assert!(ci_state::ran_nothing(&steps));
}

/// Only a `cancelled` job can be one that never ran, and only when its steps were read.
///
/// `startup_failure` also arrives with no steps and is the workflow failing to
/// start, a fact about this commit; `timed_out` ran until its budget ended it. A
/// check whose steps could not be read is not in the set either: not being able to
/// ask is not evidence that nothing ran, and the direction that leaves it red is
/// the loud one.
#[test]
fn only_a_cancelled_job_with_its_steps_read_and_empty_is_one_that_never_ran() {
    let checks = [
        check(1, "queued then cancelled", Some("cancelled"), 0),
        check(2, "stopped in its first step", Some("cancelled"), 0),
        check(3, "workflow failed to start", Some("startup_failure"), 0),
        check(4, "ran out of budget", Some("timed_out"), 0),
        check(5, "steps could not be read", Some("cancelled"), 0),
        check(6, "plain failure", Some("failure"), 0),
    ];
    let mut steps = std::collections::BTreeMap::new();
    steps.insert(1, Vec::new());
    steps.insert(2, vec![step(1, "Set up job", Some("cancelled"))]);
    steps.insert(3, Vec::new());
    steps.insert(4, Vec::new());
    steps.insert(6, Vec::new());

    let never_ran = ci_state::never_ran_checks(&checks, &steps);
    assert_eq!(
        never_ran.into_iter().collect::<Vec<_>>(),
        vec!["queued then cancelled".to_string()]
    );
}

/// Two checks of one name are two jobs, and a name is unrun only if every one was.
///
/// MEASURED ON `84dcad2c`: THREE checks called `replay every kit at its pinned
/// revision` — two successes and one cancelled in the queue — because
/// `evidence-replay` is run again on a commit that already has a run. Steps keyed
/// by name collided, and the report counted THIRTEEN jobs as cancelled before they
/// ran against eleven that were. So the steps are keyed by the check's id, and a
/// name is in the set only when every CANCELLED check of that name ran nothing: a
/// sibling that ran, or one whose steps could not be read, keeps the name red.
#[test]
fn a_name_two_jobs_share_is_unrun_only_when_every_cancelled_one_of_them_is() {
    let checks = [
        check(1, "replay", Some("cancelled"), 0),
        check(2, "replay", Some("success"), 0),
        check(3, "both queued", Some("cancelled"), 0),
        check(4, "both queued", Some("cancelled"), 0),
        check(5, "one ran", Some("cancelled"), 0),
        check(6, "one ran", Some("cancelled"), 0),
        check(7, "one unread", Some("cancelled"), 0),
        check(8, "one unread", Some("cancelled"), 0),
    ];
    let mut steps = std::collections::BTreeMap::new();
    steps.insert(1, Vec::new());
    steps.insert(2, vec![step(1, "Set up job", Some("success"))]);
    steps.insert(3, Vec::new());
    steps.insert(4, Vec::new());
    steps.insert(5, Vec::new());
    steps.insert(6, vec![step(1, "Set up job", Some("cancelled"))]);
    steps.insert(7, Vec::new());

    let never_ran = ci_state::never_ran_checks(&checks, &steps);
    assert_eq!(
        never_ran.into_iter().collect::<Vec<_>>(),
        vec!["both queued".to_string(), "replay".to_string()],
        "the cancelled `replay` ran nothing, and the successful one beside it is not \
         a cancelled check and cannot change that; a name with a sibling that ran, or \
         whose steps were not read, stays red"
    );
}

/// A successful check that shares a name with a retired one is still measured.
///
/// The census marks the NAME, and a name is not a check: asked only by name, the
/// budget block threw away the cost of two jobs that had run and passed because a
/// third of the same name had been cancelled in the queue.
#[test]
fn a_successful_job_beside_a_retired_one_of_its_name_is_still_measured() {
    let budgets = [job("replay", Some("replay"), Some("45"))];
    let mut passed = ran(
        "replay",
        "2026-09-28T10:56:09Z",
        Some("2026-09-28T11:16:09Z"),
    );
    passed.id = 2;
    let mut queued = ran(
        "replay",
        "2026-09-15T11:49:52Z",
        Some("2026-09-15T13:15:22Z"),
    );
    queued.conclusion = Some("cancelled".to_string());
    queued.id = 1;

    let retired = retired_for_never_running(&["replay"]);
    let (spent, unread) = ci_state::spent_against_budgets(&[queued, passed], &budgets, &retired);
    assert_eq!(
        spent.iter().map(Spent::percent).collect::<Vec<_>>(),
        vec![44],
        "the run that passed took 20 of its 45 minutes, and that is still a fact"
    );
    let said = ci_state::budget_report(&spent, &unread).join("\n");
    assert!(
        said.contains("1 job(s) were cancelled before any step of them ran"),
        "and only the cancelled one is counted as never having run:\n{said}"
    );
}

/// A commit whose every failure never ran is NO VERDICT, said with its own reason.
///
/// THE REASON IS NOT THE LATER PUSH'S, and the sentence a reader acts on differs: a
/// run a later push ended has its answer on the later commit, and a job that never
/// left the queue has no answer anywhere. Telling a reader to "read the later
/// commit's run" there sends them looking for a verdict that was never produced.
#[test]
fn a_commit_whose_failures_never_ran_is_no_verdict_and_not_red() {
    let checks = [
        check(1, "every compilation is one job's", Some("cancelled"), 0),
        check(2, "validate", Some("cancelled"), 1),
        check(3, "lint", Some("success"), 0),
    ];
    let retired = retired_for_never_running(&["every compilation is one job's", "validate"]);
    let said = report(SHA, &checks, &retired).join("\n");
    assert!(
        said.contains("NO VERDICT") && said.contains("before any step of them ran"),
        "{said}"
    );
    assert!(
        !said.contains("is RED"),
        "eleven jobs that never began are not a red commit: {said}"
    );
    assert!(
        !said.contains("Read the later commit's run"),
        "and there is no later run to read, so the advice for that case is not given: {said}"
    );
    assert!(
        said.contains("validate (cancelled before any step of it ran)")
            || said.contains("— validate (cancelled before any step of it ran)"),
        "each row says why it carries no verdict: {said}"
    );
}

/// And a real failure beside jobs that never ran is still red.
///
/// ⚠ THE OTHER HALF, without which the case above is satisfied by a reporter that
/// has simply stopped saying `RED`.
#[test]
fn a_failure_beside_jobs_that_never_ran_is_still_red() {
    let checks = [
        check(1, "queued then cancelled", Some("cancelled"), 0),
        check(2, "evidence replay", Some("failure"), 0),
    ];
    let retired = retired_for_never_running(&["queued then cancelled"]);
    let said = report(SHA, &checks, &retired).join("\n");
    assert!(said.contains("is RED"), "{said}");
    assert!(
        said.contains("1 cancelled before any step of them ran"),
        "and it says how much of the red was never a verdict: {said}"
    );
}

/// A RED THAT IS ALL RETIRED IS NOT A VERDICT, so the walk does not stop at it.
///
/// This is the whole point of the change: a commit CI never judged must not be the
/// place the walk concludes that verdicts exist again.
#[test]
fn a_commit_whose_every_failure_is_retired_is_not_judged() {
    let all_queued = [
        check(1, "a", Some("cancelled"), 0),
        check(2, "b", Some("success"), 0),
    ];
    let retired = retired_for_never_running(&["a"]);
    assert!(
        !ci_state::judged(&all_queued, &retired),
        "a commit whose only failure never ran has no verdict to stop at"
    );
    assert!(
        !ci_state::judged(&all_queued, &retired_by_a_later_push(&["a"])),
        "and the same holds for a run a later push ended"
    );
    assert!(
        ci_state::judged(&all_queued, &nothing_retired()),
        "while the same checks with nothing retired are a red, and a red is judged"
    );
    let one_real = [
        check(1, "a", Some("cancelled"), 0),
        check(2, "b", Some("failure"), 0),
    ];
    assert!(
        ci_state::judged(&one_real, &retired_for_never_running(&["a"])),
        "a real failure beside the retired one is a verdict"
    );
    assert!(
        ci_state::judged(&[check(1, "a", Some("success"), 0)], &nothing_retired()),
        "a clear commit is judged"
    );
}

/// The run is read out of the run segment, which is not the job's number.
///
/// R1236 PAID FOR THE NEIGHBOURING CONFUSION: a check's id and its job's id are
/// both numbers in the same answer, and the easy spelling could not tell them
/// apart. The run's number is a third one, and it is the FIRST of the two in the
/// URL rather than the last.
#[test]
fn the_run_is_the_run_segment_and_not_the_job() {
    let url = "https://github.com/o/r/actions/runs/32124678644/job/89012";
    assert_eq!(ci_state::run_of(url), Some(32_124_678_644));
    assert_eq!(ci_state::job_of(url), Some(89_012));
    assert_eq!(
        ci_state::run_of("https://example.test/some/other/app"),
        None,
        "a check no Actions run is behind has no run, which is an answer"
    );
}

/// The same annotation from several jobs is one thing a reader wants to see once.
///
/// AND BOTH NUMBERS ARE PRINTED. GitHub emits one annotation per job, so eight
/// copies of "Node.js 20 actions are deprecated" is one fact — but a line saying
/// only "1 annotation" would understate what the commit reported.
#[test]
fn annotations_are_deduplicated_and_both_numbers_are_printed() {
    let read = vec![
        said("validate", "warning", "Node.js 20 actions are deprecated"),
        said("msrv", "warning", "Node.js 20 actions are deprecated"),
        said("validate", "failure", "Process completed with exit code 1."),
    ];
    let printed = annotation_report(SHA, 3, &read).join("\n");
    assert!(printed.contains("2 distinct of 3 reported"), "{printed}");
    assert_eq!(
        printed.matches("Node.js 20").count(),
        1,
        "the repeated one is printed once: {printed}"
    );
    assert!(printed.contains("exit code 1."), "{printed}");
}

/// Every distinct annotation names the checks that reported it.
///
/// THE WHOLE POINT OF R1238, and the case is the shape that cost this repository
/// an afternoon: two failing jobs, one line carried by BOTH of them and one
/// carried by ONE. Deduplicated without attribution those two read identically,
/// and the reader cannot tell "the change did this" from "this happened to us"
/// without leaving the tool — which is what happened, three `gh api` calls by
/// hand, on `cabcd5c`.
#[test]
fn every_distinct_annotation_names_the_checks_that_said_it() {
    let read = vec![
        said("validate", "failure", "The operation was canceled."),
        said("server-features", "failure", "The operation was canceled."),
        said(
            "validate",
            "failure",
            "Process completed with exit code 127.",
        ),
    ];
    let printed = annotation_report(SHA, 3, &read).join("\n");

    let shared = printed
        .lines()
        .skip_while(|line| !line.contains("was canceled."))
        .nth(1)
        .expect("a line under the shared annotation");
    assert!(
        shared.contains("2 check(s)")
            && shared.contains("validate")
            && shared.contains("server-features"),
        "the line both jobs emitted names both: {printed}"
    );

    let alone = printed
        .lines()
        .skip_while(|line| !line.contains("exit code 127."))
        .nth(1)
        .expect("a line under the lone annotation");
    assert!(
        alone.contains("1 check(s)") && alone.contains("validate"),
        "and the one only `validate` emitted names only it — which is the fact \
         that separates a consequence from a cause: {printed}"
    );
    assert!(
        !alone.contains("server-features"),
        "and it does NOT name the other job, or the attribution says nothing: \
         {printed}"
    );
}

/// More checks than are named under one annotation are counted, never dropped.
///
/// A job name here is a sentence, so nine of them under one line is unreadable —
/// and a cap that said nothing would read as "these are the jobs", which is the
/// failure the distinct cap above already refuses.
#[test]
fn more_checks_than_are_named_under_one_annotation_are_counted() {
    let read: Vec<Said> = (0..7)
        .map(|n| said(&format!("job number {n}"), "warning", "the same thing"))
        .collect();
    let printed = annotation_report(SHA, 7, &read).join("\n");
    assert!(printed.contains("7 check(s)"), "{printed}");
    assert_eq!(
        printed.matches("job number").count(),
        4,
        "four are named: {printed}"
    );
    assert!(
        printed.contains("(+3 more)"),
        "and the other three are counted rather than dropped: {printed}"
    );
}

/// A cap that does not say what it dropped reads as "that was all of them".
#[test]
fn more_annotations_than_are_shown_are_counted_rather_than_dropped() {
    let read: Vec<Said> = (0..14)
        .map(|n| said("validate", "warning", &format!("finding number {n}")))
        .collect();
    let printed = annotation_report(SHA, 14, &read).join("\n");
    assert!(printed.contains("14 distinct of 14 reported"), "{printed}");
    assert_eq!(
        printed
            .lines()
            .filter(|line| line.contains("finding"))
            .count(),
        10,
        "ten are shown: {printed}"
    );
    assert!(
        printed.contains("(+4 distinct not shown)"),
        "and the other four are counted: {printed}"
    );
}

/// A commit that reported annotations none of which could be read says so.
///
/// NOT "no annotations", which is the other answer entirely: one is a quiet
/// commit and the other is a reporter that failed to fetch what the commit said.
#[test]
fn annotations_declared_but_unread_are_not_reported_as_none() {
    let said = annotation_report(SHA, 3, &[]).join("\n");
    assert!(said.contains("3 annotation(s), none readable"), "{said}");
    let quiet = annotation_report(SHA, 0, &[]).join("\n");
    assert!(
        quiet.contains("no CI annotations") && !quiet.contains("none readable"),
        "and a commit with nothing to say is not that: {quiet}"
    );
}

/// An annotation is printed as its level and the first line of its message.
#[test]
fn an_annotation_prints_as_its_level_and_its_first_line() {
    let long = note(
        "failure",
        "error[E0308]: mismatched types\n  --> src/lib.rs:12:5\n   |\n12 | ok\n",
    );
    let line = one_line(&long);
    assert_eq!(line, "failure error[E0308]: mismatched types");
    assert!(
        !line.contains("src/lib.rs"),
        "a whole diagnostic is not a line: {line}"
    );
}

/// A long line is cut by CHARACTERS, and a message that is not ASCII does not
/// take the reporter down with it.
///
/// THE BYTE SLICE THIS REPLACES WOULD PANIC. A reporter that dies on somebody
/// else's error text is worse than one that prints nothing, and a compiler
/// diagnostic quoting source is exactly where a non-ASCII character arrives.
#[test]
fn a_long_message_is_cut_by_characters_and_survives_a_non_ascii_one() {
    let wide = note("warning", &"가".repeat(400));
    let line = one_line(&wide);
    assert_eq!(
        line.chars().count(),
        "warning ".chars().count() + 160,
        "one hundred and sixty characters of message: {line}"
    );
}

// ---------------------------------------------------------------------------
// R1297 — the verdict that has to leave this program in something other than
// prose. Every law below is about a value a caller acts on; the sentences they
// compose are asserted beside them, because a refusal nobody can act on is the
// shape this round exists to end.
// ---------------------------------------------------------------------------

/// "Not finished" is printed as a REFUSAL and not as a row in the tally.
///
/// THE CENSUS THIS REPOSITORY ACTUALLY READ. `3 still running, 4 success, 1
/// failure` puts the one state that is NOT an answer in the same sentence as the
/// answers, and R1295 read exactly such a line, took it for "nothing to act on",
/// and never asked again — the failure it names had been sitting on that commit
/// for eleven minutes by the time it pushed.
#[test]
fn a_commit_that_has_not_finished_is_refused_a_verdict_out_loud() {
    let checks = [
        check(1, "validate", Some("success"), 0),
        check(2, "MSRV", None, 0),
        check(3, "item citations name items", None, 0),
    ];
    assert_eq!(verdict(&checks), Verdict::Pending);
    let said = census(SHA, &checks).join("\n");
    assert!(
        said.contains("NO VERDICT YET on 2d630331"),
        "the absence of a verdict is itself said, and about this commit: {said}"
    );
    assert!(
        said.contains("2 of 3 check(s) have not concluded"),
        "with how much of it is still out: {said}"
    );
    assert!(
        said.contains("Read it again"),
        "and what the reader must do, since nothing else will: {said}"
    );
}

/// A commit that IS judged is not told it has no verdict.
///
/// THE CONTROL. A reporter that printed the refusal on every push would be as
/// useless as one that never did, and only this direction says which of the two
/// this is.
#[test]
fn a_finished_commit_is_not_told_its_verdict_is_missing() {
    for checks in [
        vec![check(1, "validate", Some("success"), 0)],
        vec![check(1, "validate", Some("failure"), 0)],
    ] {
        let said = census(SHA, &checks).join("\n");
        assert!(
            !said.contains("NO VERDICT YET"),
            "every check concluded: {said}"
        );
    }
}

/// One red, as this push must spell it: the job, then where it is red.
fn red(sha: &str, job: &str) -> (String, String) {
    (sha.to_string(), job.to_string())
}

/// A red nobody named is refused, and the refusal carries the spelling.
#[test]
fn a_red_with_no_acknowledgement_is_refused_and_told_how_to_pass() {
    let reds = vec![red(SHA, "separate in-repo workspaces")];
    let standing = ci_state::acknowledgement(&reds, None);
    assert_eq!(
        standing,
        ci_state::Acknowledgement::Absent {
            reds: vec!["separate in-repo workspaces@2d630331".to_string()]
        }
    );
    let said = ci_state::refusal(SHA, &standing).join("\n");
    assert!(said.contains("REFUSING this push"), "{said}");
    assert!(
        said.contains("MNEMOSYNE_PUSH_OVER_RED='separate in-repo workspaces@2d630331'"),
        "a gate whose discharge is a guess is a gate people route around: {said}"
    );
}

/// An empty or blank acknowledgement is an absent one.
///
/// THE SHAPE A SHELL PRODUCES BY ACCIDENT. `MNEMOSYNE_PUSH_OVER_RED=` and
/// `MNEMOSYNE_PUSH_OVER_RED="$UNSET"` are what a half-typed command leaves
/// behind, and a gate satisfied by either is satisfied by a typo.
#[test]
fn a_blank_acknowledgement_names_nothing() {
    let reds = vec![red(SHA, "validate")];
    for blank in ["", "   ", " , , "] {
        assert!(
            matches!(
                ci_state::acknowledgement(&reds, Some(blank)),
                ci_state::Acknowledgement::Absent { .. }
                    | ci_state::Acknowledgement::Mismatched { .. }
            ),
            "`{blank}` must not discharge a red"
        );
    }
}

/// Naming exactly the reds discharges it, in any order and with any spacing.
#[test]
fn naming_every_red_and_no_other_lets_the_push_through() {
    let reds = vec![red(SHA, "MSRV"), red(SHA, "validate")];
    assert_eq!(
        ci_state::acknowledgement(&reds, Some(" validate@2d630331 ,MSRV@2d630331 ")),
        ci_state::Acknowledgement::Named,
        "the set is what was read, not the order it was typed in"
    );
    assert!(
        ci_state::refusal(SHA, &ci_state::Acknowledgement::Named).is_empty(),
        "and a discharged gate says nothing"
    );
}

/// Half a red is not a red read: naming one of two still refuses, and says which.
#[test]
fn an_acknowledgement_that_misses_a_red_is_refused_and_names_the_half() {
    let reds = vec![red(SHA, "MSRV"), red(SHA, "validate")];
    let standing = ci_state::acknowledgement(
        &reds,
        Some("validate@2d630331, item citations name items@2d630331"),
    );
    assert_eq!(
        standing,
        ci_state::Acknowledgement::Mismatched {
            missing: vec!["MSRV@2d630331".to_string()],
            invented: vec!["item citations name items@2d630331".to_string()],
        }
    );
    let said = ci_state::refusal(SHA, &standing).join("\n");
    assert!(said.contains("not named, and red: MSRV@2d630331"), "{said}");
    assert!(
        said.contains("named, and not red on this commit: item citations name items@2d630331"),
        "{said}"
    );
}

/// The SAME JOB red on two commits is two names, and naming one is not both.
///
/// THIS IS THE WHOLE OF R1301, and R1300 is what made it reachable: the walk
/// gathers reds across several commits, and until now they arrived as a set of
/// job NAMES. A job red on two commits for two different reasons was one name,
/// so saying it once discharged both — a push had no way to say it had read the
/// one and not the other. Right for the fix-forward case, and a real narrowing
/// everywhere else.
#[test]
fn the_same_job_red_on_two_commits_must_be_named_twice() {
    let older = "0d1c3336aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let reds = vec![
        red(SHA, "separate in-repo workspaces"),
        red(older, "separate in-repo workspaces"),
    ];
    // NAMING THE JOB ONCE IS THE OLD SPELLING, and it must no longer discharge.
    let standing = ci_state::acknowledgement(&reds, Some("separate in-repo workspaces"));
    assert!(
        matches!(standing, ci_state::Acknowledgement::Mismatched { .. }),
        "the job alone names neither commit: {standing:?}"
    );
    // AND NAMING ONE OF THE TWO IS HALF A READING, said as such.
    let half = ci_state::acknowledgement(&reds, Some("separate in-repo workspaces@2d630331"));
    assert_eq!(
        half,
        ci_state::Acknowledgement::Mismatched {
            missing: vec!["separate in-repo workspaces@0d1c3336".to_string()],
            invented: Vec::new(),
        },
        "and the half that is missing is the one on the other commit"
    );
    // THE CONTROL: both, and the push goes.
    assert_eq!(
        ci_state::acknowledgement(
            &reds,
            Some("separate in-repo workspaces@2d630331, separate in-repo workspaces@0d1c3336")
        ),
        ci_state::Acknowledgement::Named,
        "naming both is what reading both looks like"
    );
}

/// A commit that is not red is not asked to acknowledge anything.
#[test]
fn a_commit_with_no_red_has_nothing_to_name() {
    assert_eq!(
        ci_state::acknowledgement(&[], None),
        ci_state::Acknowledgement::NothingToName
    );
    assert!(ci_state::refusal(SHA, &ci_state::Acknowledgement::NothingToName).is_empty());
}

/// A red whose own name holds the separator is a DEAD END, said out loud.
///
/// NOT A PASS, WHICH IS THE POINT. Every way this gate can fail to reach a
/// verdict has to be louder than the verdict, or the way past it is to arrange
/// for it not to know — the exemption-shaped hole this gate exists to close,
/// arriving through its own parser.
#[test]
fn a_red_whose_name_holds_the_separator_can_be_spelled_by_nothing() {
    let reds = vec![red(SHA, "a job, named badly")];
    let standing = ci_state::acknowledgement(&reds, Some("a job, named badly"));
    assert_eq!(
        standing,
        ci_state::Acknowledgement::Unspellable {
            reds: vec!["a job, named badly".to_string()]
        },
        "and it is unspellable even when the value looks right — splitting it \
         yields two names that are neither of them the check"
    );
    let said = ci_state::refusal(SHA, &standing).join("\n");
    assert!(said.contains("REFUSING this push"), "{said}");
    assert!(said.contains("a job, named badly"), "{said}");

    // AND THE SECOND SEPARATOR IS A DEAD END TOO (R1301). The commit is joined
    // to the job with `@`, so a job carrying one is exactly as unspellable as a
    // job carrying a comma — a parser with one guarded end and one open end is
    // not guarded.
    let at = vec![red(SHA, "a job@named badly")];
    assert_eq!(
        ci_state::acknowledgement(&at, Some("a job@named badly@2d630331")),
        ci_state::Acknowledgement::Unspellable {
            reds: vec!["a job@named badly".to_string()]
        },
        "a value that looks right is still ambiguous to the parser that reads it"
    );
}

/// That dead end does not exist in THIS repository, and the workflows say so.
///
/// A LIMIT PROVEN ABSENT RATHER THAN ARGUED ABSENT. The law above is right in
/// general and would take this repository hostage if one of its own jobs were
/// named with a comma — so the claim "it cannot happen here" is asked of the
/// tracked workflow files instead of being written in prose beside them.
///
/// BOTH SEPARATORS SINCE R1301. The commit is joined to the job with `@`, so
/// that character became a second way to be unspellable; a law that checked one
/// of two would have gone on printing a green about a question it no longer
/// covered — which is this repository's own definition of a gate that stopped
/// judging.
#[test]
fn no_job_this_repository_declares_is_named_with_either_separator() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root");
    let (budgets, unread) = ci_plan::readable_job_budgets(&root);
    assert!(
        !budgets.is_empty(),
        "this law is vacuous unless some job was read; unread: {unread:?}"
    );
    // WHAT GITHUB SHOWS, which is what a check row is named by and therefore what
    // an acknowledgement has to spell: the `name:` when a job declares one, and
    // its id when it does not.
    for separator in [
        ci_state::ACKNOWLEDGEMENT_SEPARATOR,
        ci_state::ACKNOWLEDGEMENT_AT,
    ] {
        let offending: Vec<&str> = budgets
            .iter()
            .map(|(_, job)| job.shown_as.as_deref().unwrap_or(job.id.as_str()))
            .filter(|name| name.contains(separator))
            .collect();
        assert!(
            offending.is_empty(),
            "a job named with `{separator}` could never be acknowledged, so a red \
             in it would be unpushable: {offending:?}"
        );
    }
}

// ── THE WALK BEHIND THE BASE (R1300) ────────────────────────────────────────

/// A verdict that EXISTS is where the walk stops, and Clear is not that line.
///
/// THE DEBT ROW ASKED FOR A WALK BACK TO A CLEAR COMMIT and measuring refuted
/// it: from `609101f` the nearest all-success commit is TEN back, because this
/// repository pushes every twenty minutes and a run takes thirty to sixty, so
/// nearly every run is cancelled by the next push. Judged is reachable — depth 2
/// at the same moment — and it is the right line anyway, because the hole is
/// about verdicts that did not exist yet.
#[test]
fn the_walk_stops_where_a_verdict_exists_and_not_where_it_is_clean() {
    let clear = [check(1, "validate", Some("success"), 0)];
    let red = [check(1, "validate", Some("failure"), 0)];
    let pending = [check(1, "validate", None, 0)];
    let none = nothing_retired();
    assert!(
        ci_state::judged(&clear, &none),
        "an all-success commit is judged"
    );
    assert!(
        ci_state::judged(&red, &none),
        "and so is a RED one — somebody could read it, which is the whole \
         difference from the tail this walk exists to cover"
    );
    assert!(
        !ci_state::judged(&pending, &none),
        "a commit still running has no verdict to read"
    );
    assert!(
        !ci_state::judged(&[], &none),
        "and neither has one nothing ever ran on"
    );
}

/// A newer green retires an older red, which is what keeps this a gate.
///
/// MEASURED ON THE REAL HISTORY: `separate in-repo workspaces` failed on
/// `c7540f1` and `0d1c333`, was fixed, and ran green on `1eab0c0`. A walk that
/// demanded every red it ever passed would demand those two for ever — and a
/// refusal nobody can discharge is one people learn to bypass.
#[test]
fn a_red_a_later_commit_ran_green_is_not_outstanding() {
    let walk = vec![
        ci_state::Walked {
            sha: "1eab0c05".to_string(),
            checks: vec![check(1, "separate in-repo workspaces", Some("success"), 0)],
            retired: nothing_retired(),
        },
        ci_state::Walked {
            sha: "0d1c3336".to_string(),
            checks: vec![check(2, "separate in-repo workspaces", Some("failure"), 0)],
            retired: nothing_retired(),
        },
    ];
    assert!(
        ci_state::outstanding_reds(&walk).is_empty(),
        "the tree moved past it, and no memory of an acknowledgement was needed"
    );

    // THE CONTROL, AND IT IS THE MUTATION FOR THIS RULE: the same commit with
    // nothing green newer must leave the red outstanding, or the paragraph above
    // is asserting that this function returns nothing.
    let unfixed = vec![walk[1].clone()];
    assert_eq!(
        ci_state::outstanding_reds(&unfixed),
        vec![(
            "0d1c3336".to_string(),
            "separate in-repo workspaces".to_string()
        )],
        "with no newer green sighting the red is this push's to name"
    );
}

/// A commit's own greens do not retire its own reds.
///
/// A JOB CANNOT BE BOTH ON ONE COMMIT, so the sighting that matters is a LATER
/// one; recording this commit's greens before reading its reds would let a
/// ten-job commit with one failure look clean.
#[test]
fn a_commit_does_not_clear_its_own_red_with_its_other_jobs() {
    let walk = vec![ci_state::Walked {
        sha: "0d1c3336".to_string(),
        checks: vec![
            check(1, "validate", Some("success"), 0),
            check(2, "separate in-repo workspaces", Some("failure"), 0),
        ],
        retired: nothing_retired(),
    }];
    assert_eq!(
        ci_state::outstanding_reds(&walk).len(),
        1,
        "nine green jobs do not answer for the tenth"
    );
}

/// And a run a later push retired is not a red the walk carries.
///
/// THE SUBTRACTION IS R1242's AND IT IS READ HERE RATHER THAN RE-DERIVED. Nearly
/// every commit in this repository's recent history carries cancelled checks for
/// exactly this reason; a walk that counted them would refuse every push.
#[test]
fn the_walk_does_not_carry_a_red_a_later_push_retired() {
    let walk = vec![ci_state::Walked {
        sha: "7557cb27".to_string(),
        checks: vec![check(1, "validate", Some("cancelled"), 0)],
        retired: retired_by_a_later_push(&["validate"]),
    }];
    assert!(
        ci_state::outstanding_reds(&walk).is_empty(),
        "a run a later push ended says nothing about this commit"
    );
}

/// A job that never ran is not a red the walk carries, and an older red survives it.
///
/// THE CASE THAT MADE THIS CHANGE, end to end through the library: the base commit
/// is `84dcad2c` with eleven jobs cancelled in the queue and two successes, and
/// behind it a commit where `validate` really failed. The base contributes no red,
/// the walk reaches past it because it is not judged, and the older failure is
/// still the push's to name — stepping over a commit CI never judged must not
/// forgive what is behind it.
#[test]
fn a_red_behind_a_commit_that_never_ran_is_still_outstanding() {
    let base = ci_state::Walked {
        sha: "84dcad2c".to_string(),
        checks: vec![
            check(1, "every compilation is one job's", Some("cancelled"), 0),
            check(2, "lint", Some("success"), 0),
        ],
        retired: retired_for_never_running(&["every compilation is one job's"]),
    };
    assert!(
        !ci_state::judged(&base.checks, &base.retired),
        "the walk does not stop at a commit whose only failure never ran"
    );
    let behind = ci_state::Walked {
        sha: "1eab0c05".to_string(),
        checks: vec![check(3, "validate", Some("failure"), 0)],
        retired: nothing_retired(),
    };
    assert_eq!(
        ci_state::outstanding_reds(&[base.clone(), behind]),
        vec![("1eab0c05".to_string(), "validate".to_string())],
        "the older red is named, and the base's jobs that never ran are not"
    );
    assert!(
        ci_state::outstanding_reds(&[base]).is_empty(),
        "and with nothing behind it there is no red at all: a commit CI never \
         judged has nothing to name"
    );
}
