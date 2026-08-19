//! What this reporter says, and what it refuses to leave unsaid.
//!
//! THE READING HAS ITS OWN FILE (`github.rs`, against recorded bodies). This one
//! is about the sentences: which rows get printed, which get counted, and which
//! of the four verdicts a commit lands on. Both halves are needed and neither
//! substitutes — a reporter that parses GitHub perfectly and prints the wrong
//! sentence is the failure R1125 shipped one gate over, where an oracle matching
//! a SUBSTRING of the failure output agreed with the output it existed to refuse.

use ci_state::{
    annotation_report, one_line, report, verdict, Annotation, Check, Output, Said, Verdict,
};

const SHA: &str = "2d630331b1279e3b7a28985876b53ef0b07fbe77";

/// The census when nothing was superseded — the ordinary case, which is every
/// case here that is not about supersession.
fn census(sha: &str, checks: &[Check]) -> Vec<String> {
    report(sha, checks, &std::collections::BTreeSet::new())
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

/// A red commit is SAID to be red, in the words the hook used to print.
#[test]
fn a_red_commit_is_told_it_is_red_and_told_that_nothing_is_blocking() {
    let checks = [check(1, "validate", Some("failure"), 0)];
    let said = census(SHA, &checks).join("\n");
    assert!(said.contains("is RED"), "{said}");
    assert!(
        said.contains("Not blocking"),
        "the semantics R890 argued for from the history, not a softening of it: {said}"
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

/// A check with its two stamps, which is what a cost is read out of.
fn ran(name: &str, started: &str, completed: Option<&str>) -> Check {
    let mut row = check(1, name, Some("success"), 0);
    row.started_at = Some(started.to_string());
    row.completed_at = completed.map(str::to_string);
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
    let (spent, unread) = ci_state::spent_against_budgets(&checks, &budgets);

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
    let (spent, unread) = ci_state::spent_against_budgets(&checks, &budgets);
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
        },
        ci_state::Spent {
            check: "validate".to_string(),
            took: 2_003,
            budget_minutes: 90,
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
    let retired = ci_state::superseded_checks(&checks, &read);
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

    // THE CHECK THAT NEVER STARTED IS COVERED TOO, and that is why supersession is
    // read per RUN. Two of the three cancelled checks on `74035d7` carried no
    // annotation at all — GitHub had nothing to annotate a job that never began —
    // so a reader asking each check for its own reason would have called one
    // superseded and left the others looking like this commit's failures.
    assert_eq!(
        retired.len(),
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
    let retired = ci_state::superseded_checks(&checks, &read);
    let said = report(SHA, &checks, &retired).join("\n");
    assert!(said.contains("is RED"), "{said}");
    assert!(
        said.contains("1 of the 2 that did not pass were merely superseded"),
        "and it says how much of the red was somebody else's, or a reader cannot \
         tell which check to look at:\n{said}"
    );
    assert_eq!(
        retired.into_iter().collect::<Vec<_>>(),
        vec!["validate".to_string()],
        "the failure in the OTHER run must not be swept up by the retired one"
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
