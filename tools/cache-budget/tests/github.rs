//! What GitHub actually answers, held against what this gate reads out of it.
//!
//! THE BODIES HERE ARE RECORDINGS. `tests/actions-caches.one-page.json` and
//! `tests/actions-caches.paginated.json` are what `gh api
//! repos/{owner}/{repo}/actions/caches` printed for THIS repository, byte for
//! byte, the second with `--paginate` over a page size small enough to make it
//! answer in four. Everything below reads one of those, or a mutation of one
//! that names the drift it is modelling — because a fixture somebody invented
//! proves what that person believed the API sends.
//!
//! WHY THIS FILE HAD TO EXIST. Until R1130 the answer was flattened by a `--jq`
//! expression before this program ever saw it, and that expression is a program
//! in a language nothing here can run: there is no `jq` on this machine, and
//! `gh` needs a network and a credential. So the one seam between this gate and
//! the storage it judges had no oracle of any kind — the injection that renames
//! a field GitHub sends could not be written, let alone go red.
//!
//! AND ONE FAILURE OF THIS READ IS SILENT WHILE THE REST ARE LOUD. A row missing
//! a size does not parse and a body that never arrived is empty, but a read that
//! STOPS EARLY is a well-formed answer describing a smaller repository — and
//! smaller is the direction that passes a budget. `total_count` is the only
//! thing in the answer that can catch it, and the projection threw it away.

use cache_budget::{
    caches_in, candidate_runs, run_started_in, saved_the_archive, workflow_runs_query, PriorRun,
};

/// This repository's cache storage, as GitHub answered on 2026-08-10.
const ONE_PAGE: &str = include_str!("actions-caches.one-page.json");

/// The same storage, the same minute, answered in four pages instead of one.
const PAGINATED: &str = include_str!("actions-caches.paginated.json");

/// The newest cache in both recordings, spelled as GitHub spells it.
const NEWEST_KEY: &str =
    "Linux-cargo-validate-90ceb24c2cf985e258613d038affb1685bf0ab9134a089fb7a8d3958f97b9856";
const NEWEST_BYTES: u64 = 151_791_181;
const NEWEST_CREATED: &str = "2026-08-10T10:14:11.221132000Z";

/// How many caches this repository held when both recordings were taken.
const HELD: usize = 11;

/// The recorded answer reads as the caches it carries, values and all.
///
/// THE VALUES ARE ASSERTED AND NOT JUST THE COUNT, because the failure this
/// replaces was a projection naming fields by hand: a `size_in_bytes` GitHub
/// renamed would have arrived as the string `null` and a `key` it renamed as an
/// empty one, and a test counting rows agrees with both.
#[test]
fn the_recorded_answer_reads_as_the_caches_it_carries() {
    let held = caches_in(ONE_PAGE).expect("a real answer is one this gate can read");
    assert_eq!(held.len(), HELD, "every row GitHub sent, and no others");
    let newest = &held[0];
    assert_eq!(newest.key, NEWEST_KEY);
    assert_eq!(newest.size_in_bytes, NEWEST_BYTES);
    assert_eq!(newest.created_at, NEWEST_CREATED);
    // AND THE FIELDS THIS GATE DOES NOT WANT DID NOT STOP IT. The recording
    // carries `id`, `ref`, `version` and `last_accessed_at` on every row; a
    // reader that refused unknown fields would go red the day GitHub adds one,
    // which is a gate failing for somebody else's work.
    assert!(
        ONE_PAGE.contains("\"last_accessed_at\""),
        "the recording carries fields this gate ignores — if it stops doing so, \
         this test has stopped proving that ignoring them works"
    );
}

/// Four pages of one storage read as the same caches as one page of it.
///
/// TWO RECORDINGS OF THE SAME THING, taken seconds apart, are the control this
/// gate's pagination handling needs: `gh --paginate` prints one JSON object per
/// page rather than one merged document, and a reader that took only the first
/// or concatenated them wrongly disagrees with the single-page answer here.
#[test]
fn four_pages_of_one_storage_read_as_the_same_caches_as_one_page() {
    let paged = caches_in(PAGINATED).expect("a real paginated answer is readable");
    let single = caches_in(ONE_PAGE).expect("a real single-page answer is readable");
    assert_eq!(
        paged, single,
        "the same storage, answered in four pages and in one"
    );
}

/// An answer that stops early is a read that failed — not a smaller repository.
///
/// THE FIXTURE IS REAL AND SO IS THE FAILURE IT MODELS: this is the first page
/// of the recorded paginated answer, sliced at the byte the stream ends it on,
/// which is EXACTLY what a `gh` without `--paginate` prints. It says the
/// repository holds eleven caches and carries three, and three of eleven caches
/// weigh a third of the budget. Nothing about it is malformed.
#[test]
fn an_answer_that_stops_early_is_a_read_that_failed_and_not_a_smaller_repository() {
    let why = caches_in(first_page_of(PAGINATED))
        .expect_err("a body carrying fewer rows than it counts is not a verdict");
    assert!(
        why.contains("11") && why.contains("3"),
        "and it says what it was told and what arrived: {why}"
    );
}

/// A repository holding no caches says so, and that is a reading rather than a
/// refusal.
///
/// THE CONTROL FOR THE TWO ABOVE. Every other case here is a failure, and a
/// reader that refused whenever it ended up with no rows would be refusing the
/// honest answer a fresh repository gives.
#[test]
fn a_repository_holding_no_caches_says_so_and_is_read_as_none() {
    let held = caches_in(r#"{"total_count":0,"actions_caches":[]}"#)
        .expect("a page saying there is nothing is an answer, not a failure");
    assert!(held.is_empty(), "and it holds nothing: {held:?}");
}

/// Nothing printed is not a repository holding nothing.
///
/// THE TWO USED TO BE ONE ANSWER. Under the projection this replaces, a `gh`
/// whose filter matched nothing and a `gh` that printed nothing both arrived as
/// an empty stdout and were read as a repository storing zero bytes.
#[test]
fn nothing_printed_is_not_a_repository_holding_nothing() {
    let why = caches_in("").expect_err("an empty answer is not a repository");
    assert!(
        why.contains("printed nothing"),
        "and it says which of the two it is: {why}"
    );
}

/// A field GitHub renames is a refusal, and not a zero.
///
/// THE DRIFT THIS SEAM IS FOR, applied to a real body: with the size gone, a
/// reader that shrugged would price every cache in the repository at nothing and
/// report a storage sitting on its limit as using none of it.
#[test]
fn a_size_this_gate_cannot_find_is_a_refusal_rather_than_a_zero() {
    let renamed = ONE_PAGE.replace("\"size_in_bytes\"", "\"bytes\"");
    let why = caches_in(&renamed).expect_err("a row with no size is not a cache of size zero");
    // THE READER'S OWN WORDS, and not this gate's summary of them: the wrapper
    // names all five fields whatever goes wrong, so an oracle reading only that
    // would agree with a body that failed for any other reason.
    assert!(
        why.contains("missing field `size_in_bytes`"),
        "and it names the field GitHub stopped sending: {why}"
    );
}

/// Pages that disagree about the count are storage moving underneath the read.
///
/// A cache saved by another job of the same run lands between two pages, and the
/// rows either side of it are then a sample rather than a census. Summing them
/// produces a total that is nobody's.
#[test]
fn pages_that_disagree_about_the_count_are_refused() {
    let moved = format!(
        "{ONE_PAGE}{}",
        ONE_PAGE.replace("\"total_count\":11", "\"total_count\":12")
    );
    let why = caches_in(&moved).expect_err("two answers about one storage is not a total");
    assert!(
        why.contains("11") && why.contains("12"),
        "and it says both counts: {why}"
    );
}

/// The one spelling of a timestamp this gate's ordering assumes it will get.
///
/// `Held::created_at` says in its own doc comment that string comparison is
/// sound "only because the API returns one fixed-width UTC spelling for every
/// entry" — a claim about the world with, until now, no reader. It is also NOT
/// the spelling a run's start time comes in, which is why the comparison against
/// a run is made to the second; a fixture writing the shorter one would be
/// testing the ordering on inputs the API never produces.
#[test]
fn every_recorded_stamp_carries_the_one_spelling_the_ordering_assumes() {
    let held = caches_in(ONE_PAGE).expect("a real answer is readable");
    let widths: std::collections::BTreeSet<usize> =
        held.iter().map(|cache| cache.created_at.len()).collect();
    assert_eq!(
        widths.len(),
        1,
        "one width for every entry, or comparing them as strings orders them by \
         how they were printed: {widths:?}"
    );
    assert!(
        held.iter()
            .all(|cache| cache.created_at.ends_with('Z') && cache.created_at.contains('.')),
        "UTC, and to the fraction — the spelling `to_the_second` exists to trim: {:?}",
        held.first().map(|cache| &cache.created_at)
    );
}

/// The other endpoint: one run of this repository's CI, as GitHub answered.
const RUN: &str = include_str!("actions-run.json");

/// The run that recording is of, and when GitHub says it began.
const RUN_ID: &str = "31376754536";
const RUN_STARTED: &str = "2026-08-10T09:54:23Z";

/// The recorded run says when it began, in that endpoint's own spelling.
#[test]
fn the_recorded_run_says_when_it_started() {
    let started = run_started_in(RUN_ID, RUN).expect("a real run answer is one this gate can read");
    assert_eq!(started, RUN_STARTED);
}

/// AND IT IS NOT THE SPELLING THE OTHER ENDPOINT USES.
///
/// This is the premise the whole restore verdict rests on: a cache created
/// before the run began is one the run restored, and the two timestamps being
/// compared come from two endpoints. `law.rs` proves the comparison survives the
/// difference; what nothing proved until now is that the difference is REAL, and
/// a fixture writing both in one spelling would have agreed with a comparison
/// that could not survive it.
#[test]
fn the_two_endpoints_really_do_spell_a_timestamp_differently() {
    let started = run_started_in(RUN_ID, RUN).expect("a real run answer is readable");
    let held = caches_in(ONE_PAGE).expect("a real cache answer is readable");
    let created = &held[0].created_at;
    assert!(
        !started.contains('.'),
        "the runs endpoint answers to the second: {started}"
    );
    assert!(
        created.contains('.'),
        "and the cache endpoint to the fraction: {created}"
    );
    assert_ne!(
        started.len(),
        created.len(),
        "two widths, which is what makes comparing them whole a mistake"
    );
}

/// A run whose start time GitHub does not send is a refusal, not a zero time.
///
/// EVERY CACHE IN THE REPOSITORY IS NEWER THAN NOTHING, so a blank read here
/// reports all of them as archives this run built from scratch — a page of
/// findings about jobs that were in fact warm.
#[test]
fn a_run_answer_with_no_start_time_is_a_refusal_rather_than_a_zero_time() {
    let renamed = RUN.replace("\"run_started_at\"", "\"began\"");
    let why = run_started_in(RUN_ID, &renamed).expect_err("a run with no start time is not a time");
    assert!(
        why.contains("missing field `run_started_at`"),
        "and it names the field GitHub stopped sending: {why}"
    );
}

/// An empty stamp is the same refusal, and it names the run.
#[test]
fn a_run_that_reports_an_empty_start_time_is_refused_by_name() {
    let blank = RUN.replace(RUN_STARTED, "");
    let why = run_started_in(RUN_ID, &blank).expect_err("an empty stamp is not a start time");
    assert!(
        why.contains(RUN_ID),
        "and it says which run answered that way: {why}"
    );
}

/// Nothing printed about a run is not a run that started at no time.
#[test]
fn a_run_answer_that_never_arrives_is_a_refusal() {
    let why = run_started_in(RUN_ID, "").expect_err("silence is not an answer");
    assert!(
        why.contains(RUN_ID) && why.contains("read that failed"),
        "and it says which run it could not read about: {why}"
    );
}

/// The third endpoint: the newest runs of one workflow of this repository, as
/// GitHub answered on 2026-08-13.
///
/// AN ABRIDGED RECORDING, AND THE ABRIDGEMENT IS NAMED. It is the answer to
/// `gh api "repos/{owner}/{repo}/actions/workflows/mnemosyne-validate.yml/runs\
/// ?branch=main&per_page=2"` with each row's `actor`, `triggering_actor`,
/// `repository` and `head_repository` objects dropped — four copies of one user
/// and one repository per row, 24 KB of the 25, none of it named by this reader.
/// Every value that remains is verbatim, which is what a recording is FOR here:
/// this endpoint stamps a run to the SECOND while the cache endpoint stamps an
/// archive to the fraction, and the whole restore verdict is a comparison between
/// the two.
///
/// AND ITS NEWEST ROW IS A REAL FAILURE — run 31695396997, the run this reader
/// exists because of. R1178 read that as "saved nothing" and excluded it; R1207
/// measured the claim and it is false. A run's conclusion is a fact about every
/// job in it, and a failed run routinely contains cache jobs that finished and
/// wrote their archives — 122 such saves inside the 19 red runs of this
/// repository's newest hundred. So `conclusion` is no longer read here at all,
/// and what decides is `actions-run-jobs.json` below.
const RUNS: &str = include_str!("actions-workflow-runs.json");

/// What GitHub answered about the JOBS of one run, byte for byte.
///
/// UNABRIDGED, AND ITS SHA256 IS `5877bc0aacab2d42c7b36fd1f08400e7e82f06d40374dd17e8fea35524bd3ebe`
/// — the answer to `gh api "repos/{owner}/{repo}/actions/runs/31782330835/jobs\
/// ?per_page=100"` with nothing removed, because every field this reader is
/// wrong about is one it would otherwise have invented.
///
/// THIS ONE RECORDING CARRIES BOTH HALVES OF R1207's LAW, which is why it is the
/// one kept. Run 31782330835 CONCLUDED FAILURE, and inside it:
///
///   - seven cache jobs finished and their `Post Cache …` steps concluded
///     `success` — archives really written, at a commit R1178's reader passed
///     over, which is the leniency this round removes;
///   - the job that failed (`every test compiled is one CI runs`) has its two
///     `Post Cache …` steps at `skipped` — archives NOT written, in the same run.
///
/// So the answer differs BETWEEN KEYS OF ONE RUN, and no reading at run level can
/// be right about both. That is the measurement that made the bound per cache.
const JOBS: &str = include_str!("actions-run-jobs.json");

/// The run that recording is of.
const JOBS_RUN: u64 = 31_782_330_835;

/// A cache whose archive that run really wrote, and one whose it did not.
const SAVED_STEP: &str = "Cache cargo";
const SKIPPED_STEP: &str = "Cache cargo (unrun tests, cargo home)";

/// The workflow that recording is of, and the two runs in it.
const WORKFLOW: &str = ".github/workflows/mnemosyne-validate.yml";
const HEAD: &str = "4c07d64056ce169d4f1cb879f4fd0eb724aff782";
const PRIOR: &str = "75b9bd4c8a80e175c188f5f3563355f03f2c613c";
const PRIOR_STARTED: &str = "2026-08-13T08:46:43Z";

/// When the run being judged began — and it is the SAME STAMP the newest recorded
/// row carries, because they are the same run.
const THIS_RUN_STARTED: &str = "2026-08-13T11:24:58Z";

/// A commit no run in the recording was of.
const ELSEWHERE: &str = "0000000000000000000000000000000000000000";

/// Late enough that both recorded runs are earlier than it.
const LATER: &str = "2026-08-13T12:00:00Z";

/// The recorded answer names every run that could have left an archive, newest
/// first.
///
/// THE QUESTION THIS ENDPOINT IS ASKED, and the answer that repairs run
/// 31695396997: the interval a cache key can be judged over runs from the last
/// time that key's archive was written, not from the commits one push carried.
/// This endpoint narrows the candidates; the jobs endpoint decides among them.
#[test]
fn the_recorded_answer_names_every_run_that_could_have_left_an_archive() {
    let both = candidate_runs(WORKFLOW, RUNS, LATER, ELSEWHERE)
        .expect("a real answer is one this gate can read");
    assert_eq!(
        both.iter().map(|run| run.sha.as_str()).collect::<Vec<_>>(),
        vec![HEAD, PRIOR],
        "newest first, and the FAILED newest row is a candidate — R1207: its cache \
         jobs may have written their archives before the run went red"
    );
    assert_eq!(
        both[0].id, 31_695_396_997,
        "with the id the jobs endpoint takes"
    );
    assert_eq!(both[1].started_at, PRIOR_STARTED);

    let one = candidate_runs(WORKFLOW, RUNS, THIS_RUN_STARTED, HEAD)
        .expect("a real answer is one this gate can read");
    assert_eq!(
        one.iter().map(|run| run.sha.as_str()).collect::<Vec<_>>(),
        vec![PRIOR],
        "and asked as the run of HEAD would ask, only the earlier one remains"
    );
}

/// A FAILED run that wrote the archive bounds the interval, and a step that was
/// skipped in that same run does not.
///
/// THIS IS THE WHOLE OF R1207, AND ONE RECORDING PROVES BOTH HALVES. Run
/// 31782330835 concluded FAILURE. Inside it `Post Cache cargo` concluded
/// `success` — an archive really written at that commit, which R1178's reader
/// passed over — while `Post Cache cargo (unrun tests, cargo home)`, in the job
/// that failed, concluded `skipped`. Two keys, one run, opposite answers: no
/// reading at run level can be right about both, and that is why the bound moved
/// from the run to the step.
#[test]
fn a_failed_run_that_wrote_the_archive_bounds_the_interval_and_a_skipped_save_does_not() {
    let run = PriorRun {
        id: JOBS_RUN,
        sha: HEAD.to_string(),
        started_at: PRIOR_STARTED.to_string(),
    };
    assert!(
        saved_the_archive(&run, SAVED_STEP, JOBS).expect("a real answer is readable"),
        "`{SAVED_STEP}` wrote its archive in a run that concluded failure"
    );
    assert!(
        !saved_the_archive(&run, SKIPPED_STEP, JOBS).expect("the same answer is readable"),
        "`{SKIPPED_STEP}` did not, in the SAME run — which is what makes the \
         per-step read load-bearing rather than decoration"
    );
    assert!(
        !saved_the_archive(
            &run,
            "Cache cargo (a key this workflow does not declare)",
            JOBS
        )
        .expect("readable"),
        "and a step the run never held saved nothing"
    );
}

/// A jobs page GitHub truncated is a refusal, not a run that saved nothing.
///
/// "I WAS NOT SENT THAT JOB" AND "THAT JOB DID NOT SAVE" ARE DIFFERENT ANSWERS,
/// and folding the first into the second widens every interval it bounds — the
/// silent-lenient direction this round exists to remove. `total_count` is the
/// only thing in the answer that can catch it.
#[test]
fn a_truncated_jobs_page_is_a_refusal_rather_than_a_run_that_saved_nothing() {
    let run = PriorRun {
        id: JOBS_RUN,
        sha: HEAD.to_string(),
        started_at: PRIOR_STARTED.to_string(),
    };
    let claims_more = JOBS.replacen("\"total_count\":9", "\"total_count\":10", 1);
    assert_ne!(claims_more, JOBS, "the recording really says nine");
    let why = saved_the_archive(&run, SAVED_STEP, &claims_more)
        .expect_err("a page that did not arrive whole has no verdict");
    assert!(
        why.contains("9") && why.contains("10"),
        "and it says what it was promised and what it got: {why}"
    );
}

/// An unreadable or absent answer about a run's jobs is a refusal that says so.
#[test]
fn an_answer_about_a_runs_jobs_this_gate_cannot_read_is_a_refusal() {
    let run = PriorRun {
        id: JOBS_RUN,
        sha: HEAD.to_string(),
        started_at: PRIOR_STARTED.to_string(),
    };
    let silent =
        saved_the_archive(&run, SAVED_STEP, "  ").expect_err("silence is not a run with no jobs");
    assert!(
        silent.contains(&JOBS_RUN.to_string()),
        "and it names the run it could not read: {silent}"
    );
    // A RENAMED `steps` IS THE DANGEROUS ONE, and it is why the field is an
    // `Option` read against `status` rather than a `#[serde(default)]`: defaulted
    // to empty it would answer "that step did not save" about every job, which is
    // a well-formed lie in the widening direction.
    let renamed = JOBS.replace("\"steps\":", "\"stages\":");
    let why = saved_the_archive(&run, SAVED_STEP, &renamed)
        .expect_err("a field GitHub renamed is not a run that saved nothing");
    assert!(
        why.contains("no step list") && why.contains("validate"),
        "and it names the job it could not read: {why}"
    );
    // THE MIRROR, AND IT CANNOT COME FROM THE RECORDING: a completed run holds no
    // queued job, so the state that legitimately carries no step list has to be
    // written out. A job GitHub has not started says nothing about any archive,
    // and answering "it did not save" is the right reading there — the refusal
    // above is about a job that says it FINISHED and sent no steps.
    let queued = "{\"total_count\":1,\"jobs\":[{\"name\":\"one this run has not started\",\
                  \"status\":\"queued\"}]}";
    assert!(
        !saved_the_archive(&run, SAVED_STEP, queued).expect("a job that has not run is readable"),
        "a queued job saved nothing, and saying so is not a refusal"
    );
}

/// A run of the commit being judged cannot bound its own interval.
///
/// THE RULE THAT MAKES THE REPAIR WORK. The run whose miss is being judged is a
/// run of `HEAD`, and an interval of `HEAD..HEAD` answers "nothing moved" for
/// every key in the repository — which is the narrow-range failure R1095 paid for
/// once already, arriving by a different door.
#[test]
fn a_run_of_the_commit_being_judged_cannot_bound_its_own_interval() {
    let excluded = candidate_runs(WORKFLOW, RUNS, LATER, HEAD).expect("readable");
    assert_eq!(
        excluded
            .iter()
            .map(|run| run.sha.as_str())
            .collect::<Vec<_>>(),
        vec![PRIOR],
        "the run of HEAD is passed over"
    );
    let included = candidate_runs(WORKFLOW, RUNS, LATER, ELSEWHERE).expect("readable");
    assert_eq!(
        included[0].sha, HEAD,
        "and it is passed over for being HEAD's rather than for anything else"
    );
}

/// Two workflows triggered by one push start in the same second, and neither is
/// earlier than the other.
///
/// MEASURED, NOT SUPPOSED: `mnemosyne-validate` and `evidence-replay` both report
/// `2026-08-13T11:24:58Z` for the push that made this reader necessary. So "before
/// this run" has to be strict — a sibling run of the same push has observed
/// nothing this run has not, and treating it as the interval's start would answer
/// every question with `HEAD..HEAD`.
#[test]
fn a_run_that_started_in_the_same_second_is_not_an_earlier_run() {
    let tied = candidate_runs(WORKFLOW, RUNS, THIS_RUN_STARTED, ELSEWHERE).expect("readable");
    assert_eq!(
        tied.iter().map(|run| run.sha.as_str()).collect::<Vec<_>>(),
        vec![PRIOR],
        "the tie is not earlier"
    );
    let after =
        candidate_runs(WORKFLOW, RUNS, "2026-08-13T11:24:59Z", ELSEWHERE).expect("readable");
    assert_eq!(
        after[0].sha, HEAD,
        "and one second later it is earlier — which is what makes the comparison \
         strict rather than accidentally right"
    );
}

/// A page carrying no run this gate can use is a READING, and not a refusal.
///
/// A workflow that has never run is a repository this gate cannot bound an
/// interval for — and the caller narrows to the push range and prints why.
/// Refusing here would turn a first-ever run of a new workflow into a red `main`.
#[test]
fn a_page_with_no_usable_run_is_a_reading_rather_than_a_refusal() {
    let empty = "{\"total_count\":515,\"workflow_runs\":[]}";
    let answer = candidate_runs(WORKFLOW, empty, LATER, ELSEWHERE)
        .expect("a page carrying no rows is an answer");
    assert_eq!(answer, Vec::new());
}

/// A commit GitHub stops sending is a refusal, and not a run at no commit.
#[test]
fn a_head_sha_this_gate_cannot_find_is_a_refusal_rather_than_a_run_at_no_commit() {
    let renamed = RUNS.replace("\"head_sha\"", "\"sha\"");
    let why = candidate_runs(WORKFLOW, &renamed, LATER, ELSEWHERE)
        .expect_err("a row with no commit is not a run at no commit");
    assert!(
        why.contains("missing field `head_sha`"),
        "and it names the field GitHub stopped sending: {why}"
    );
}

/// An empty commit or an empty stamp is a refusal that says what it needs.
///
/// AN INTERVAL STARTING AT NOTHING EXCUSES EVERYTHING. `git diff "" HEAD` is not
/// a question with an answer, and reading a blank as "the beginning of history"
/// would excuse every cache in the repository.
#[test]
fn a_run_with_an_empty_commit_is_refused_by_name() {
    let blank = RUNS.replace(PRIOR, "");
    let why = candidate_runs(WORKFLOW, &blank, LATER, ELSEWHERE)
        .expect_err("an empty sha is not a commit");
    assert!(
        why.contains(WORKFLOW),
        "and it says whose runs it could not read: {why}"
    );
}

/// Nothing printed about a workflow is not a workflow that never ran.
#[test]
fn nothing_printed_about_a_workflow_is_not_a_workflow_that_never_ran() {
    let why = candidate_runs(WORKFLOW, "", LATER, ELSEWHERE).expect_err("silence is not an answer");
    assert!(
        why.contains(WORKFLOW) && why.contains("printed nothing"),
        "and it says which of the two silences it is: {why}"
    );
}

/// Every recorded run stamp carries the one spelling the ordering assumes.
///
/// The newest qualifying run is chosen by COMPARING these stamps, so one row
/// spelled differently from another would order them by how they were printed.
#[test]
fn every_recorded_run_stamp_carries_the_one_spelling_the_ordering_assumes() {
    let stamps: Vec<&str> = RUNS
        .match_indices("\"run_started_at\": \"")
        .map(|(at, needle)| {
            let rest = &RUNS[at + needle.len()..];
            &rest[..rest.find('"').expect("a closing quote")]
        })
        .collect();
    assert_eq!(
        stamps.len(),
        2,
        "the recording carries two runs: {stamps:?}"
    );
    let widths: std::collections::BTreeSet<usize> = stamps.iter().map(|it| it.len()).collect();
    assert_eq!(
        widths.len(),
        1,
        "one width, or the ordering is by print: {widths:?}"
    );
    assert!(
        stamps
            .iter()
            .all(|it| it.ends_with('Z') && !it.contains('.')),
        "UTC and to the second — the spelling a cache's `created_at` does NOT use: {stamps:?}"
    );
}

/// The question names the workflow by the identity the endpoint takes.
///
/// A PATH IS A 404 AND A 404 IS A REFUSAL. `ci-plan` spells a workflow as the file
/// it read — `.github/workflows/mnemosyne-validate.yml` — and this endpoint
/// addresses one by NAME. Deriving that here is what keeps every caller from
/// spelling it a second time.
#[test]
fn the_question_names_the_workflow_by_the_identity_the_endpoint_takes() {
    let words = workflow_runs_query(WORKFLOW, Some("main"));
    let asked = words.join(" ");
    assert!(
        asked.contains("workflows/mnemosyne-validate.yml/runs"),
        "the file name, not the path it was read from: {asked}"
    );
    assert!(
        !asked.contains(".github/workflows/mnemosyne-validate.yml/runs"),
        "and not the path, which this endpoint answers 404 to: {asked}"
    );
    assert!(
        asked.contains("branch=main"),
        "scoped to the ref whose storage holds the archives: {asked}"
    );
    assert!(
        asked.contains("per_page="),
        "and it asks for the newest page rather than paginating a run history: {asked}"
    );
}

/// With no branch to name, the question carries no branch.
///
/// A `pull_request` names no branch this endpoint accepts, and `branch=` with
/// nothing after it is a filter matching nothing — which would arrive as a
/// workflow that has never run.
#[test]
fn with_no_branch_to_name_the_question_carries_no_branch() {
    for absent in [None, Some(""), Some("  ")] {
        let asked = workflow_runs_query(WORKFLOW, absent).join(" ");
        assert!(!asked.contains("branch="), "{absent:?} -> {asked}");
    }
}

/// The bytes of the first page of a concatenated answer, sliced where the stream
/// ends it.
///
/// ASKED OF THE PARSER rather than found by searching for `}{`: a key or a
/// timestamp may hold any two characters, and a reader that split on them would
/// be a second, worse parser standing between this test and the recording.
fn first_page_of(body: &str) -> &str {
    let mut pages = serde_json::Deserializer::from_str(body).into_iter::<serde_json::Value>();
    pages
        .next()
        .expect("the recording has a first page")
        .expect("and it is JSON");
    &body[..pages.byte_offset()]
}
