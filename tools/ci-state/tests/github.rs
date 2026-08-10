//! What GitHub actually answers, held against what this reporter reads out of it.
//!
//! THE BODIES HERE ARE RECORDINGS. `tests/check-runs.one-page.json`,
//! `tests/check-runs.paginated.json` and `tests/annotations.json` are what `gh
//! api` printed for THIS repository, byte for byte, on 2026-08-10 — the second
//! with `--paginate` over a page size small enough to make it answer in three.
//! Everything below reads one of those, or a mutation of one that NAMES THE DRIFT
//! it is modelling, because a fixture somebody invented proves what that person
//! believed the API sends.
//!
//! WHY THIS FILE HAD TO EXIST. Until R1136 this reading was three `gh -q`
//! expressions inside `.githooks/pre-push`, and the suite that holds that hook up
//! stubs `gh` with already-filtered lines — its own header said so: "a wrong
//! `--json` field or jq expression in the hook would still pass here". That was
//! measured rather than argued, twice, by breaking one expression and running the
//! suite: `.conclusion` renamed to `.verdict` left it at 14 passed / 0 failed, and
//! so did `annotations_count` renamed to `annotation_count`. With a real `gh` the
//! first makes every conclusion print as a dash — so a RED commit reports clean —
//! and the second makes every commit report no annotations at all.
//!
//! THE COMMIT THESE RECORDINGS ARE OF WAS RED, and that is why it was chosen: a
//! green recording proves a reporter can agree that nothing is wrong.

use ci_state::{annotations_in, checks_in, is_failing, verdict, Verdict};

/// This repository's checks on `2d63033`, as GitHub answered.
const ONE_PAGE: &str = include_str!("check-runs.one-page.json");

/// The same nine checks, the same minute, answered in three pages instead of one.
const PAGINATED: &str = include_str!("check-runs.paginated.json");

/// The annotations of the one check on that commit that carried any.
const ANNOTATIONS: &str = include_str!("annotations.json");

/// The commit all three recordings are about.
const SHA: &str = "2d630331b1279e3b7a28985876b53ef0b07fbe77";

/// How many checks that commit carried, and the one that failed.
const CARRIED: usize = 9;
const FAILED_ID: u64 = 93_478_488_570;
const FAILED_NAME: &str = "every cache declared is one CI keeps";

/// The recorded answer reads as the checks it carries, values and all.
///
/// THE VALUES ARE ASSERTED AND NOT JUST THE COUNT, because the failure this
/// replaces was a projection naming fields by hand: a `conclusion` GitHub renamed
/// arrived as the string `-` and an `annotations_count` it renamed selected
/// nothing, and a test counting rows agrees with both.
#[test]
fn the_recorded_answer_reads_as_the_checks_it_carries() {
    let checks = checks_in(SHA, ONE_PAGE).expect("a real answer is one this reporter can read");
    assert_eq!(
        checks.len(),
        CARRIED,
        "every row GitHub sent, and no others"
    );
    let failed = checks
        .iter()
        .find(|check| check.id == FAILED_ID)
        .expect("the check that failed is in the answer");
    assert_eq!(failed.name, FAILED_NAME);
    assert_eq!(failed.conclusion.as_deref(), Some("failure"));
    assert_eq!(failed.status, "completed");
    assert_eq!(failed.head_sha, SHA);
    assert_eq!(failed.output.annotations_count, 1);
    assert!(is_failing(failed));
    assert_eq!(verdict(&checks), Verdict::Red, "the commit these are of");

    // AND THE FIELDS THIS REPORTER DOES NOT WANT DID NOT STOP IT. The recording
    // carries `app`, `check_suite`, `pull_requests`, `html_url` and more on every
    // row; a reader that refused unknown fields would go red the day GitHub adds
    // one, which is a gate failing for somebody else's work.
    assert!(
        ONE_PAGE.contains("\"check_suite\"") && ONE_PAGE.contains("\"details_url\""),
        "the recording carries fields this reporter ignores — if it stops doing \
         so, this test has stopped proving that ignoring them works"
    );
}

/// A skipped check is not a failing one.
///
/// THE RECORDING IS WHY THIS IS HERE AND NOT AN OPINION: the same commit carries a
/// job GitHub concluded `skipped`, this repository's workflow skips a job whose
/// inputs did not change on every green push, and a reporter that read that as red
/// would be ignored within a day.
#[test]
fn the_skipped_check_in_the_recording_is_not_read_as_a_failure() {
    let checks = checks_in(SHA, ONE_PAGE).expect("a real answer is readable");
    let skipped: Vec<&str> = checks
        .iter()
        .filter(|check| check.conclusion.as_deref() == Some("skipped"))
        .map(|check| check.name.as_str())
        .collect();
    assert_eq!(
        skipped.len(),
        1,
        "the recording carries exactly one skipped job — without it this test \
         proves nothing: {skipped:?}"
    );
    assert!(
        !checks
            .iter()
            .filter(|check| check.conclusion.as_deref() == Some("skipped"))
            .any(is_failing),
        "a skipped job is a job that did not need to run"
    );
}

/// Three pages of one commit read as the same checks as one page of it.
///
/// TWO RECORDINGS OF THE SAME THING, taken seconds apart, are the control this
/// reporter's pagination handling needs: `gh --paginate` prints one JSON object
/// per page rather than one merged document, and a reader that took only the
/// first or concatenated them wrongly disagrees with the single-page answer here.
#[test]
fn three_pages_of_one_commit_read_as_the_same_checks_as_one_page() {
    let paged = checks_in(SHA, PAGINATED).expect("a real paginated answer is readable");
    let single = checks_in(SHA, ONE_PAGE).expect("a real single-page answer is readable");
    assert_eq!(
        paged, single,
        "the same commit's checks, answered in three pages and in one"
    );
}

/// An answer that stops early is a read that failed — not a commit with fewer jobs.
///
/// THE FIXTURE IS REAL AND SO IS THE FAILURE IT MODELS: this is the first page of
/// the recorded paginated answer, sliced at the byte the stream ends it on, which
/// is EXACTLY what a `gh` without `--paginate` prints. It says the commit carries
/// nine checks and carries four. Nothing about it is malformed, and the four it
/// carries do not include the one that failed.
#[test]
fn an_answer_that_stops_early_is_a_read_that_failed_and_not_a_shorter_commit() {
    let why = checks_in(SHA, first_page_of(PAGINATED))
        .expect_err("a body carrying fewer rows than it counts is not a report");
    assert!(
        why.contains('9') && why.contains('4'),
        "and it says what it was told and what arrived: {why}"
    );
}

/// A commit nothing ran on says so, and that is a reading rather than a refusal.
///
/// THE CONTROL FOR THE THREE ABOVE. Every other case here is a failure, and a
/// reader that refused whenever it ended up with no rows would be refusing the
/// honest answer a commit with no workflows gives.
#[test]
fn a_commit_nothing_ran_on_says_so_and_is_read_as_none() {
    let checks = checks_in(SHA, r#"{"total_count":0,"check_runs":[]}"#)
        .expect("a page saying there is nothing is an answer, not a failure");
    assert!(checks.is_empty(), "and it holds nothing: {checks:?}");
    assert_eq!(verdict(&checks), Verdict::Nothing);
}

/// Nothing printed is not a commit nothing ran on.
///
/// THE TWO USED TO BE ONE ANSWER. Under the projection this replaces, a `gh` whose
/// filter matched nothing and a `gh` that printed nothing both arrived as an empty
/// stdout, and the hook printed "no CI runs recorded" over each.
#[test]
fn nothing_printed_is_not_a_commit_nothing_ran_on() {
    let why = checks_in(SHA, "").expect_err("an empty answer is not a commit");
    assert!(
        why.contains("printed nothing"),
        "and it says which of the two it is: {why}"
    );
}

/// A conclusion GitHub stops sending is a refusal, and not a check still running.
///
/// THE DRIFT THIS SEAM IS FOR, and the one a type nearly failed to catch: serde
/// treats a derived `Option` field as OPTIONAL, so without the `deserialize_with`
/// on [`ci_state::Check::conclusion`] this body would read as nine checks that
/// have not concluded — and this reporter would print a red commit as pending.
#[test]
fn a_conclusion_this_reporter_cannot_find_is_a_refusal_rather_than_a_pending_check() {
    let renamed = ONE_PAGE.replace("\"conclusion\"", "\"verdict\"");
    let why = checks_in(SHA, &renamed)
        .expect_err("a row with no conclusion is not a check still running");
    // THE READER'S OWN WORDS, and not this program's summary of them: the wrapper
    // names all six fields whatever goes wrong, so an oracle reading only that
    // would agree with a body that failed for any other reason.
    assert!(
        why.contains("missing field `conclusion`"),
        "and it names the field GitHub stopped sending: {why}"
    );
}

/// A check still running is read as one, and that is the other half.
///
/// THE MUTATION NAMES ITS DRIFT: the failing check's conclusion is set to `null`,
/// which is what GitHub sends while a job is queued or in flight. Without this the
/// test above would be satisfied by a reader that refused every `null` too, and
/// the third answer would exist only in a doc comment.
#[test]
fn a_conclusion_sent_as_null_is_a_check_that_has_not_concluded() {
    let running = ONE_PAGE.replace("\"conclusion\":\"failure\"", "\"conclusion\":null");
    let checks = checks_in(SHA, &running).expect("an explicit null is an answer");
    let pending = checks
        .iter()
        .find(|check| check.id == FAILED_ID)
        .expect("the same row");
    assert_eq!(pending.conclusion, None);
    assert!(
        !is_failing(pending),
        "not concluded is not concluded FAILED"
    );
    assert_eq!(
        verdict(&checks),
        Verdict::Pending,
        "and a commit with an unfinished check is neither red nor clear"
    );
}

/// An annotations count GitHub renames is a refusal, and not zero annotations.
///
/// THE SECOND MEASURED HOLE. Renaming this field in the hook's jq expression left
/// its suite at 14 passed / 0 failed while every commit reported no annotations —
/// the blindness R893 added the call to end.
#[test]
fn an_annotations_count_this_reporter_cannot_find_is_a_refusal_rather_than_a_zero() {
    let renamed = ONE_PAGE.replace("\"annotations_count\"", "\"annotation_count\"");
    let why = checks_in(SHA, &renamed).expect_err("a check with no count is not a quiet check");
    assert!(
        why.contains("missing field `annotations_count`"),
        "and it names the field GitHub stopped sending: {why}"
    );
}

/// A well-formed answer about another commit is refused by name.
///
/// NOTHING ELSE HERE COULD TELL. It parses, its count agrees with its rows, and
/// every field this reporter wants is present — it is simply about a different
/// commit, and printed as this one's it reports somebody else's health. R1122 paid
/// for exactly this shape one gate over: format perfect, subject wrong.
#[test]
fn an_answer_about_another_commit_is_refused_and_says_which() {
    let elsewhere = "0123456789abcdef0123456789abcdef01234567";
    let moved = ONE_PAGE.replace(SHA, elsewhere);
    let why = checks_in(SHA, &moved).expect_err("another commit's checks are not this commit's");
    assert!(
        why.contains(SHA) && why.contains(elsewhere),
        "and it says both commits: {why}"
    );
}

/// Pages that disagree about the count are checks moving underneath the read.
#[test]
fn pages_that_disagree_about_the_count_are_refused() {
    let moved = format!(
        "{ONE_PAGE}{}",
        ONE_PAGE.replace("\"total_count\":9", "\"total_count\":10")
    );
    let why = checks_in(SHA, &moved).expect_err("two answers about one commit is not a report");
    assert!(
        why.contains('9') && why.contains("10"),
        "and it says both counts: {why}"
    );
}

/// The recorded annotation reads as what it says.
#[test]
fn the_recorded_annotation_reads_as_what_it_says() {
    let notes = annotations_in(FAILED_ID, ANNOTATIONS).expect("a real answer is readable");
    assert_eq!(notes.len(), 1, "what GitHub sent for that check");
    assert_eq!(notes[0].annotation_level, "failure");
    assert_eq!(notes[0].message, "Process completed with exit code 1.");
}

/// A check with nothing to say answers with an empty list, and silence does not.
#[test]
fn an_empty_annotation_list_is_an_answer_and_silence_is_not() {
    let none = annotations_in(FAILED_ID, "[]").expect("an empty list is an answer");
    assert!(none.is_empty());
    let why = annotations_in(FAILED_ID, "").expect_err("silence is not an answer");
    assert!(
        why.contains("printed nothing") && why.contains(&FAILED_ID.to_string()),
        "and it says which check it could not read about: {why}"
    );
}

/// An annotation whose level GitHub renames is a refusal, not an unlabelled note.
#[test]
fn an_annotation_level_this_reporter_cannot_find_is_a_refusal() {
    let renamed = ANNOTATIONS.replace("\"annotation_level\"", "\"level\"");
    let why = annotations_in(FAILED_ID, &renamed).expect_err("a note with no level is not a note");
    assert!(
        why.contains("missing field `annotation_level`"),
        "and it names the field GitHub stopped sending: {why}"
    );
}

/// The bytes of the first page of a concatenated answer, sliced where the stream
/// ends it.
///
/// ASKED OF THE PARSER rather than found by searching for `}{`: a check name or a
/// URL may hold any two characters, and a reader that split on them would be a
/// second, worse parser standing between this test and the recording.
fn first_page_of(body: &str) -> &str {
    let mut pages = serde_json::Deserializer::from_str(body).into_iter::<serde_json::Value>();
    pages
        .next()
        .expect("the recording has a first page")
        .expect("and it is JSON");
    &body[..pages.byte_offset()]
}
