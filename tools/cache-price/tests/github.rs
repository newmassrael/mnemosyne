//! What GitHub actually answers, held against what this program reads out of it.
//!
//! THE BODIES HERE ARE RECORDINGS: `tests/jobs.green.json` and
//! `tests/jobs.red.json` are what `gh api …/actions/runs/<id>/jobs --paginate`
//! printed for two real runs of this repository — the green one this measurement
//! was started from and the red one before it — and `tests/runs.json` is the
//! workflow's own run list. Everything below reads one of those, or a mutation of
//! one that NAMES THE DRIFT it is modelling.
//!
//! AND THE RECORDINGS TAUGHT THE LAW RATHER THAN CONFIRMING IT. Two of the cases
//! here exist because of something in the bytes that nobody predicted: the runs
//! endpoint's `total_count` is not its row count (it is 499 for a three-row page),
//! and a SKIPPED job reports stamps that run backwards.

use cache_price::{jobs_in, prices_in, runs_in, seconds_between};

/// Run 31432163172 — the green one, nine jobs.
const GREEN: &str = include_str!("jobs.green.json");
const GREEN_ID: u64 = 31_432_163_172;

/// Run 31394095606 — the red one before it, with a job GitHub skipped.
const RED: &str = include_str!("jobs.red.json");
const RED_ID: u64 = 31_394_095_606;

/// The workflow's last three runs, as its run list answered.
const RUNS: &str = include_str!("runs.json");

/// The job that owns two caches, and what they cost it in the green run.
const TWO_CACHE_JOB: &str = "every test compiled is one CI runs";
const BUILD_DIR: &str = "cargo (unrun tests, build directory)";
const CARGO_HOME: &str = "cargo (unrun tests, cargo home)";

/// The recorded run reads as the jobs it carries.
#[test]
fn the_recorded_run_reads_as_the_jobs_it_carries() {
    let jobs = jobs_in(GREEN_ID, GREEN).expect("a real answer is one this program can read");
    assert_eq!(jobs.len(), 9, "every job GitHub sent, and no others");
    let two = jobs
        .iter()
        .find(|job| job.name == TWO_CACHE_JOB)
        .expect("the job that owns two caches");
    assert_eq!(two.conclusion.as_deref(), Some("success"));
    assert!(
        two.steps.len() > 4,
        "and its steps arrived: {}",
        two.steps.len()
    );
}

/// THE PRICE OF THE TWO CACHES IN THAT RUN, to the second, off the recording.
///
/// THESE FOUR NUMBERS ARE THE ROUND. The ledger's arithmetic for GG1b counted a
/// 134 s restore against about 214 s of compiling saved and called the cache worth
/// +81 s. The SAVE was not in it — and here it is 113 s for the build directory,
/// against a 144 s restore. A term paid on most runs and counted on none.
#[test]
fn the_two_caches_of_the_recorded_run_are_priced_to_the_second() {
    let jobs = jobs_in(GREEN_ID, GREEN).expect("a real answer is readable");
    let prices = prices_in(&jobs).expect("real steps come in pairs");
    let build = prices
        .iter()
        .find(|price| price.cache == BUILD_DIR)
        .expect("the build directory cache");
    assert_eq!(build.job, TWO_CACHE_JOB);
    assert_eq!(build.restore, Some(144), "21:05:31 to 21:07:55");
    assert_eq!(build.save, Some(113), "21:11:22 to 21:13:15");
    assert_eq!(build.total(), Some(257));

    let home = prices
        .iter()
        .find(|price| price.cache == CARGO_HOME)
        .expect("the cargo home cache");
    assert_eq!(home.restore, Some(3));
    assert_eq!(home.save, Some(9));
}

/// Seven of that run's nine jobs paid for a cache, and two paid for none.
///
/// THE POPULATION IS ASSERTED, not assumed: a reader that silently found fewer
/// caches would report a cheaper repository, and this is the count the workflow
/// declares.
#[test]
fn the_recorded_run_prices_every_cache_its_jobs_declare() {
    let jobs = jobs_in(GREEN_ID, GREEN).expect("readable");
    let prices = prices_in(&jobs).expect("pairs");
    assert_eq!(prices.len(), 8, "seven jobs, one of which owns two caches");
    let owners: std::collections::BTreeSet<&str> =
        prices.iter().map(|price| price.job.as_str()).collect();
    assert_eq!(owners.len(), 7, "{owners:?}");
    assert!(
        prices.iter().all(|price| price.total().is_some()),
        "every step of a finished run ran"
    );
}

/// A run with a job GitHub SKIPPED is still read, and prices the rest.
///
/// THE SKIPPED JOB IS WHY THIS RECORDING IS HERE. It carries no steps at all and
/// its own stamps run BACKWARDS — `14:01:37` to `14:01:36` — so a reader that
/// priced jobs rather than their cache steps would book 86 399 seconds against it.
#[test]
fn a_run_with_a_skipped_job_is_read_and_the_rest_is_priced() {
    let jobs = jobs_in(RED_ID, RED).expect("a real answer is readable");
    let skipped = jobs
        .iter()
        .find(|job| job.conclusion.as_deref() == Some("skipped"))
        .expect("the recording carries one, which is what makes this a test");
    assert!(skipped.steps.is_empty(), "a skipped job runs no steps");
    let backwards = seconds_between(
        skipped.started_at.as_deref().expect("a stamp"),
        skipped.completed_at.as_deref().expect("a stamp"),
    );
    assert!(
        backwards.is_err(),
        "and its stamps run backwards, which must be a refusal rather than a day: {backwards:?}"
    );
    let prices = prices_in(&jobs).expect("the rest still pairs");
    assert!(!prices.is_empty(), "the caches of the jobs that did run");
}

/// An answer that stops early is a read that failed, not a run with fewer jobs.
#[test]
fn an_answer_that_stops_early_is_a_read_that_failed() {
    let short = GREEN.replace("\"total_count\":9", "\"total_count\":11");
    let why = jobs_in(GREEN_ID, &short).expect_err("a body carrying fewer rows than it counts");
    assert!(
        why.contains("11") && why.contains('9'),
        "and it says what it was told and what arrived: {why}"
    );
}

/// Nothing printed is not a run with no jobs.
#[test]
fn nothing_printed_is_not_a_run_with_no_jobs() {
    let why = jobs_in(GREEN_ID, "").expect_err("silence is not an answer");
    assert!(why.contains("printed nothing"), "{why}");
}

/// A stamp GitHub stops sending is a refusal, and not a step that never ran.
///
/// THE SERDE TRAP, HELD AGAINST A REAL BODY. A derived `Option` field is OPTIONAL,
/// so without the `deserialize_with` on [`cache_price::Step::started_at`] this
/// body would read as a run in which no step ever started — priced at nothing,
/// silently, in the direction that makes every cache look free.
#[test]
fn a_stamp_this_program_cannot_find_is_a_refusal_rather_than_a_step_that_never_ran() {
    let renamed = GREEN.replace("\"started_at\"", "\"began_at\"");
    let why = jobs_in(GREEN_ID, &renamed).expect_err("a step with no start is not a step");
    assert!(
        why.contains("missing field `started_at`"),
        "and it names the field GitHub stopped sending: {why}"
    );
}

/// EACH TYPE REQUIRES ITS OWN STAMPS, asked of each type by itself.
///
/// THESE TWO CASES WERE ADDED BY AN INJECTION THAT CAME BACK 0 RED. The recording
/// carries `started_at` at BOTH levels — on the job and on every step — so the
/// rename above is satisfied by whichever of the two still refuses, and taking the
/// attribute off `Step` alone left it green. A body renaming the field is a real
/// drift and worth its case; it is simply not an oracle for WHICH type stopped
/// requiring it, and only literals aimed one type at a time are.
#[test]
fn a_step_that_does_not_carry_its_stamps_is_refused() {
    let why = serde_json::from_str::<cache_price::Step>(r#"{"name":"x","completed_at":null}"#)
        .expect_err("a step with no `started_at` key is not a step that never started");
    assert!(why.to_string().contains("started_at"), "{why}");
    let why = serde_json::from_str::<cache_price::Step>(r#"{"name":"x","started_at":null}"#)
        .expect_err("nor is one with no `completed_at`");
    assert!(why.to_string().contains("completed_at"), "{why}");
    // AND THE CONTROL: explicit nulls are an answer, and the ONE this program
    // relies on to mean "this step did not run".
    let never_ran = serde_json::from_str::<cache_price::Step>(
        r#"{"name":"x","started_at":null,"completed_at":null}"#,
    )
    .expect("two explicit nulls are a step that did not run");
    assert_eq!((never_ran.started_at, never_ran.completed_at), (None, None));
}

/// The same, for the job level.
#[test]
fn a_job_that_does_not_carry_its_stamps_is_refused() {
    let why =
        serde_json::from_str::<cache_price::Job>(r#"{"name":"x","conclusion":null,"steps":[]}"#)
            .expect_err("a job with no stamps at all");
    assert!(why.to_string().contains("started_at"), "{why}");
}

/// The workflow's run list reads as its rows, and its `total_count` is NOT one.
///
/// THE RECORDING IS THE WHOLE ARGUMENT. This page holds three rows and says
/// `total_count` is 499, because that field counts every run the workflow has ever
/// had. The neighbouring endpoint's identically-named field is the row count, and
/// a program that reused one law for both would refuse every honest page here.
#[test]
fn the_run_list_total_count_is_not_its_row_count() {
    let runs = runs_in(3, RUNS).expect("a real answer is one this program can read");
    assert_eq!(runs.len(), 3);
    assert_eq!(runs[0].id, GREEN_ID);
    assert_eq!(runs[0].conclusion.as_deref(), Some("success"));
    assert_eq!(runs[1].id, RED_ID);
    assert_eq!(runs[1].conclusion.as_deref(), Some("failure"));
    assert!(
        RUNS.contains("\"total_count\":499"),
        "the recording says 499 against three rows — if that stops being true this \
         test has stopped proving what it is about"
    );
}

/// A page shorter than its request, from a workflow that has more, is a short read.
#[test]
fn a_page_shorter_than_its_request_is_refused_when_the_workflow_has_more() {
    let why = runs_in(10, RUNS).expect_err("three rows is not the ten that were asked for");
    assert!(
        why.contains("10") && why.contains('3') && why.contains("499"),
        "and it says all three numbers: {why}"
    );
}

/// A workflow that really has only these runs answers honestly with all of them.
///
/// THE CONTROL for the case above. A reader that refused every short page would
/// refuse a young workflow's whole history.
#[test]
fn a_workflow_with_fewer_runs_than_asked_for_is_read_rather_than_refused() {
    let young = RUNS.replace("\"total_count\":499", "\"total_count\":3");
    let runs = runs_in(10, &young).expect("all the runs there are is an answer");
    assert_eq!(runs.len(), 3);
}
