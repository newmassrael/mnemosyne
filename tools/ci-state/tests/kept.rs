//! What the record keeps, what it refuses to keep, and what it says a trend is.
//!
//! THE HALF THAT COULD BE PLAUSIBLE AND WRONG. A trend is a sentence about
//! numbers a reader cannot check by hand — nobody remembers what `validate` cost
//! six pushes ago, which is the entire reason R1260 wrote any of this down — so a
//! reporter that ordered its record by the wrong key, or drew one job's series
//! through another job's commits, would print a well-formed paragraph nobody
//! could contradict. Every case here is about a way that paragraph can be
//! well-formed and false.

use std::fs;
use std::path::Path;

use ci_state::history::{
    keep, kept_in, kept_report, movements, never_completed, trend_report, Kept, Movement, RECORDS,
};
use ci_state::{Check, Output, Spent};

use tempfile::TempDir;

const SHA: &str = "2d630331b1279e3b7a28985876b53ef0b07fbe77";

/// A job that ran all of its steps — the population a movement is drawn over.
fn spent(check: &str, took: u64, budget_minutes: u64) -> Spent {
    concluded(check, took, budget_minutes, "success")
}

/// A job that ran and stopped, in GitHub's own word for how.
fn concluded(check: &str, took: u64, budget_minutes: u64, conclusion: &str) -> Spent {
    Spent {
        check: check.to_string(),
        took,
        budget_minutes,
        conclusion: conclusion.to_string(),
    }
}

/// A record of one commit, built the way this crate builds one.
fn record(commit: &str, ran_at: &str, jobs: &[Spent]) -> Kept {
    Kept {
        commit: commit.to_string(),
        ran_at: ran_at.to_string(),
        jobs: jobs.to_vec(),
    }
}

/// A check with the two stamps a cost is read out of.
fn ran(name: &str, started: &str) -> Check {
    Check {
        id: 1,
        name: name.to_string(),
        details_url: "https://github.com/o/r/actions/runs/1/job/1".to_string(),
        head_sha: SHA.to_string(),
        status: "completed".to_string(),
        conclusion: Some("success".to_string()),
        started_at: Some(started.to_string()),
        completed_at: Some("2026-08-18T14:00:00Z".to_string()),
        output: Output {
            annotations_count: 0,
        },
    }
}

/// A tree with these records already in it, written through the writer under test.
///
/// SEEDED THROUGH `keep` AND NEVER BY HAND. A fixture that wrote the JSON itself
/// would be a second spelling of the record format, and it would keep passing on
/// the day the writer's spelling changed — which is the failure this repository
/// has now paid for at four levels.
fn tree_holding(records: &[Kept]) -> TempDir {
    let tree = TempDir::new().expect("a tree to record into");
    for one in records {
        keep(tree.path(), one).expect("the record is written");
    }
    tree
}

/// A commit nothing measured is not a row in the history.
///
/// AN EMPTY RECORD WOULD LENGTHEN EVERY COUNT AND SHORTEN NO SERIES: `against 9
/// commit(s) recorded here` reads as nine measurements, and a reader has no way
/// to tell that four of them were pushes made while CI was still running.
#[test]
fn a_commit_no_job_of_which_was_measured_is_not_recorded_at_all() {
    let checks = [ran("validate", "2026-08-18T13:40:00Z")];
    assert_eq!(
        Kept::of(SHA, &checks, &[]),
        None,
        "a commit with no measured job has nothing to contribute to a trend"
    );

    let kept = Kept::of(SHA, &checks, &[spent("validate", 2424, 90)])
        .expect("a commit with a measured job is worth keeping");
    assert_eq!(kept.commit, SHA);
    assert_eq!(kept.jobs, vec![spent("validate", 2424, 90)]);
}

/// The stamp a record is ordered by is the run's, and the earliest one at that.
#[test]
fn a_commits_record_is_stamped_with_the_earliest_start_of_its_own_checks() {
    let checks = [
        ran("late", "2026-08-18T13:59:00Z"),
        ran("early", "2026-08-18T13:40:00Z"),
        ran("later", "2026-08-18T14:10:00Z"),
    ];
    let kept = Kept::of(SHA, &checks, &[spent("early", 60, 90)]).expect("a record");
    assert_eq!(
        kept.ran_at, "2026-08-18T13:40:00Z",
        "a run began when its first job began, not when the job that happens to \
         be measured did"
    );
}

/// Records come back in the order the runs happened, whatever order they were
/// written in.
///
/// THE KEY IS THE RUN'S OWN STAMP AND NOT THIS FILE'S MTIME, and this case is
/// what makes that a fact rather than a comment. The two records here are written
/// NEWEST FIRST, so the mtime order and the run order disagree — which is exactly
/// the state a refill produces, and under an mtime key it would insert a
/// fortnight-old run as the newest point of every trend.
///
/// AND THE COMMIT NAMES RUN THE OTHER WAY TOO. A fixture whose shas happened to
/// sort into the right order would pass under a comparator that had lost the
/// stamp entirely — the tie-breaker alone would carry it — so the older run here
/// is named `ccccccc` and the newer `aaaaaaa`, and every wrong key fails.
#[test]
fn the_record_is_ordered_by_when_the_run_happened_and_not_by_when_it_was_written() {
    let tree = tree_holding(&[
        record(
            "aaaaaaa",
            "2026-08-18T13:40:00Z",
            &[spent("validate", 60, 90)],
        ),
        record(
            "ccccccc",
            "2026-08-04T09:00:00Z",
            &[spent("validate", 60, 90)],
        ),
    ]);
    let (kept, trouble) = kept_in(tree.path());
    assert!(trouble.is_empty(), "{trouble:?}");
    assert_eq!(
        kept.iter()
            .map(|one| one.commit.as_str())
            .collect::<Vec<_>>(),
        vec!["ccccccc", "aaaaaaa"],
        "the older RUN comes first even though its file was written second and \
         its name sorts last"
    );
}

/// Reporting on one commit twice leaves one record of it.
///
/// A LATER READING IS STRICTLY BETTER INFORMED — the jobs that had not finished
/// have — and a finished job never changes what it took. Two rows would be two
/// points in every series for a commit that ran once.
#[test]
fn reading_one_commit_twice_replaces_its_record_rather_than_adding_a_second() {
    let tree = tree_holding(&[
        record(SHA, "2026-08-18T13:40:00Z", &[spent("validate", 60, 90)]),
        record(
            SHA,
            "2026-08-18T13:40:00Z",
            &[spent("validate", 60, 90), spent("evidence", 120, 30)],
        ),
    ]);
    let (kept, trouble) = kept_in(tree.path());
    assert!(trouble.is_empty(), "{trouble:?}");
    assert_eq!(kept.len(), 1, "one commit is one row: {kept:?}");
    assert_eq!(
        kept[0].jobs.len(),
        2,
        "and the row is the later, better-informed reading"
    );
    let files = fs::read_dir(tree.path().join(RECORDS))
        .expect("the record directory")
        .count();
    assert_eq!(files, 1, "the rename leaves nothing beside the record");
}

/// A commit name that is not a commit names no file at all.
///
/// THE NAME ARRIVES ON `argv` AND A NAME IS DATA. R1259 paid for the general form
/// of this one round earlier, where a manifest field naming a directory was
/// resolved against whoever ran the program and walked a whole home directory:
/// the defect belongs to the unbounded use of the input, not to the input.
#[test]
fn a_commit_name_that_is_not_a_commit_is_refused_rather_than_joined_to_a_path() {
    let tree = TempDir::new().expect("a tree");
    for name in [
        "../../../etc/mnemosyne-ci-state-escaped",
        "..",
        "HEAD",
        "2D630331B1279E3B",
        "ab",
        "",
    ] {
        let why = keep(
            tree.path(),
            &record(name, "2026-08-18T13:40:00Z", &[spent("validate", 60, 90)]),
        )
        .expect_err("a name that is not a commit is refused");
        assert!(
            why.contains("lowercase hex"),
            "the refusal says what a commit name is: {why}"
        );
    }
    assert!(
        !Path::new("/etc/mnemosyne-ci-state-escaped.json").exists(),
        "and nothing was written outside the tree"
    );
    let (kept, trouble) = kept_in(tree.path());
    assert!(
        kept.is_empty() && trouble.is_empty(),
        "{kept:?} {trouble:?}"
    );
}

/// A record named after one commit and describing another is named, not placed.
///
/// R1122's SHAPE: format perfect, every field present, and the subject wrong. A
/// record copied to a new name is a point this reporter would otherwise place on
/// a commit that never ran it.
#[test]
fn a_record_that_describes_another_commit_than_it_is_named_for_is_refused() {
    let tree = tree_holding(&[record(
        "aaaaaaa",
        "2026-08-18T13:40:00Z",
        &[spent("validate", 60, 90)],
    )]);
    let directory = tree.path().join(RECORDS);
    fs::copy(
        directory.join("aaaaaaa.json"),
        directory.join("ccccccc.json"),
    )
    .expect("a record copied under another name");

    let (kept, trouble) = kept_in(tree.path());
    assert_eq!(kept.len(), 1, "the honest record is still read: {kept:?}");
    assert_eq!(trouble.len(), 1, "{trouble:?}");
    assert!(
        trouble[0].contains("ccccccc.json") && trouble[0].contains("aaaaaaa"),
        "the finding names the file AND the commit it actually describes: {trouble:?}"
    );
}

/// A file that will not read as a record is named, and the rest are still read.
#[test]
fn a_record_that_will_not_read_is_named_and_the_others_are_still_read() {
    let tree = tree_holding(&[
        record(
            "aaaaaaa",
            "2026-08-04T09:00:00Z",
            &[spent("validate", 60, 90)],
        ),
        record(
            "bbbbbbb",
            "2026-08-18T13:40:00Z",
            &[spent("validate", 90, 90)],
        ),
    ]);
    let directory = tree.path().join(RECORDS);
    fs::write(directory.join("ddddddd.json"), "{ not json at all").expect("a corrupted record");
    fs::write(
        directory.join("eeeeeee.json"),
        r#"{"commit":"eeeeeee","ran_at":"whenever","jobs":[]}"#,
    )
    .expect("a record stamped with something that is not a stamp");
    fs::write(
        directory.join("fffffff.json"),
        r#"{"commit":"fffffff","ran_at":"2026-08-18T13:40:00Z","jobs":[],"note":"typo"}"#,
    )
    .expect("a record carrying a key nobody meant");

    let (kept, trouble) = kept_in(tree.path());
    assert_eq!(
        kept.len(),
        2,
        "one bad file does not take the history down with it: {kept:?}"
    );
    assert_eq!(trouble.len(), 3, "{trouble:?}");
    let said = trouble.join("\n");
    assert!(said.contains("ddddddd.json"), "{said}");
    assert!(
        said.contains("eeeeeee.json") && said.contains("whenever"),
        "a record this reader cannot ORDER is one it cannot place, and it says so \
         rather than sorting it somewhere: {said}"
    );
    assert!(
        said.contains("fffffff.json") && said.contains("note"),
        "a key nobody meant, sitting beside one that matters, is a refusal: {said}"
    );
}

/// A tree nothing has reported on yet is a state and not a fault.
#[test]
fn a_tree_with_no_record_at_all_is_no_history_rather_than_an_error() {
    let tree = TempDir::new().expect("a tree");
    let (kept, trouble) = kept_in(tree.path());
    assert!(kept.is_empty(), "{kept:?}");
    assert!(
        trouble.is_empty(),
        "every clone is in this state on its first push: {trouble:?}"
    );
    assert!(
        trend_report(&kept, Some("validate")).is_empty(),
        "and the block above has already said no job was measured"
    );
}

/// Files that are not records are passed over, which is what makes the write safe.
#[test]
fn the_file_a_record_is_renamed_from_is_never_read_as_a_record() {
    let tree = tree_holding(&[record(
        "aaaaaaa",
        "2026-08-18T13:40:00Z",
        &[spent("validate", 60, 90)],
    )]);
    fs::write(
        tree.path().join(RECORDS).join("bbbbbbb.json.4242"),
        "half a record, from a process that died",
    )
    .expect("a partial write");
    let (kept, trouble) = kept_in(tree.path());
    assert_eq!(kept.len(), 1, "{kept:?}");
    assert!(
        trouble.is_empty(),
        "a partial write is not a corrupted record — it is a file the writer \
         renames from, and it never carries the extension a reader looks for: \
         {trouble:?}"
    );
}

/// One commit is a level, and the report says so instead of drawing a line
/// through one point.
#[test]
fn one_commit_recorded_is_a_level_and_is_told_apart_from_a_trend() {
    let kept = [record(
        SHA,
        "2026-08-18T13:40:00Z",
        &[spent("validate", 60, 90)],
    )];
    let said = trend_report(&kept, Some("validate")).join("\n");
    assert!(
        said.contains("1 commit is recorded here") && said.contains("not yet a trend"),
        "{said}"
    );
    assert!(
        !said.contains("points"),
        "and no movement is claimed from a single point: {said}"
    );
}

/// A job's window is its own commits, not the record's.
///
/// A JOB ADDED LAST WEEK HAS A SHORT HISTORY INSIDE A LONG RECORD. Measuring its
/// movement from the record's oldest commit would measure it from a commit it
/// never ran on, which is a number about nothing that reads like a number about
/// something.
#[test]
fn a_job_is_measured_from_its_own_first_commit_and_not_the_records() {
    let kept = [
        record("aaaaaaa", "2026-08-01T09:00:00Z", &[spent("old", 60, 90)]),
        record("bbbbbbb", "2026-08-08T09:00:00Z", &[spent("old", 60, 90)]),
        record(
            "ccccccc",
            "2026-08-15T09:00:00Z",
            &[spent("old", 60, 90), spent("new", 1800, 90)],
        ),
        record(
            "ddddddd",
            "2026-08-18T09:00:00Z",
            &[spent("old", 60, 90), spent("new", 2700, 90)],
        ),
    ];
    let movements = movements(&kept);
    let recent: &Movement = movements
        .iter()
        .find(|one| one.check == "new")
        .expect("the job that was added later");
    assert_eq!(
        recent.commits, 2,
        "the window is the two commits that ran it: {recent:?}"
    );
    assert_eq!(recent.since, "2026-08-15T09:00:00Z");
    assert_eq!(
        (recent.first.percent(), recent.last.percent()),
        (33, 50),
        "1800s and 2700s of 90m: {recent:?}"
    );
    assert_eq!(recent.points(), 17);
    assert_eq!((recent.low, recent.high), (33, 50));
}

/// The steepest rise is printed even when it is nowhere near a budget.
///
/// THE TWO QUESTIONS THIS FILE SEPARATES. A job flat at forty-four per cent is
/// the level line's subject and it is not news; a job that went from twelve to
/// thirty-one is news, and the level line will never name it because it is not
/// close to anything.
#[test]
fn the_steepest_rise_is_named_even_when_it_is_not_the_job_closest_to_its_budget() {
    let kept = [
        record(
            "aaaaaaa",
            "2026-08-01T09:00:00Z",
            &[spent("validate", 2214, 90), spent("replay", 648, 90)],
        ),
        record(
            "bbbbbbb",
            "2026-08-18T09:00:00Z",
            &[spent("validate", 2376, 90), spent("replay", 1674, 90)],
        ),
    ];
    let said = trend_report(&kept, Some("validate")).join("\n");
    assert!(
        said.contains("against 2 commit(s) recorded here, oldest 2026-08-01T09:00:00Z"),
        "{said}"
    );
    assert!(
        said.contains(
            "`validate` 41% → 44% (+3 points over 2 commit(s) it completed; 41–44% \
                       across them)"
        ),
        "the job the level line named is followed through the record: {said}"
    );
    assert!(
        said.contains("the steepest rise is not that job")
            && said.contains("`replay` 12% → 31% (+19 points"),
        "and the job that is actually moving is named, though nothing about it is \
         close to its budget: {said}"
    );
}

/// When the steepest rise IS the job the level named, it is said once.
#[test]
fn a_job_that_is_both_closest_and_steepest_is_not_printed_twice() {
    let kept = [
        record(
            "aaaaaaa",
            "2026-08-01T09:00:00Z",
            &[spent("validate", 540, 90)],
        ),
        record(
            "bbbbbbb",
            "2026-08-18T09:00:00Z",
            &[spent("validate", 2376, 90)],
        ),
    ];
    let said = trend_report(&kept, Some("validate")).join("\n");
    assert_eq!(
        said.matches("`validate`").count(),
        1,
        "one job, one sentence: {said}"
    );
    assert!(!said.contains("steepest"), "{said}");
}

/// Nothing rising is a sentence rather than a block that quietly shrank.
#[test]
fn no_job_rising_is_said_out_loud_rather_than_left_as_a_missing_line() {
    let kept = [
        record(
            "aaaaaaa",
            "2026-08-01T09:00:00Z",
            &[spent("validate", 2376, 90)],
        ),
        record(
            "bbbbbbb",
            "2026-08-18T09:00:00Z",
            &[spent("validate", 2214, 90)],
        ),
    ];
    let said = trend_report(&kept, Some("validate")).join("\n");
    assert!(
        said.contains("no job's share of its budget is above where it was first recorded"),
        "a reader who sees only the header cannot tell a fall from a block that \
         was not printed: {said}"
    );
    assert!(
        said.contains("(-3 points"),
        "and the fall itself is shown: {said}"
    );
}

/// A raised budget is not a job that got cheaper, and the sentence says which.
///
/// THE TRAP THE RECORD EXISTS TO MAKE VISIBLE. Raising `timeout-minutes` from 90
/// to 120 drops a job's share from 44% to 33% without making it one second
/// cheaper — and a record that had kept the percentage instead of the two numbers
/// it is computed from could not tell the difference.
#[test]
fn a_budget_that_moved_is_named_rather_than_read_as_a_job_that_got_cheaper() {
    let kept = [
        record(
            "aaaaaaa",
            "2026-08-01T09:00:00Z",
            &[spent("validate", 2400, 90)],
        ),
        record(
            "bbbbbbb",
            "2026-08-18T09:00:00Z",
            &[spent("validate", 2460, 120)],
        ),
    ];
    let movement = movements(&kept)
        .into_iter()
        .find(|one| one.check == "validate")
        .expect("the job");
    assert!(!movement.budget_steady(), "{movement:?}");
    assert_eq!(
        (movement.first.percent(), movement.last.percent()),
        (44, 34),
        "the shares this reporter printed at the time FELL: {movement:?}"
    );
    assert_eq!(
        movement.points(),
        1,
        "and held against one denominator the job got MORE expensive — a trend \
         built from the raw shares would have called this the steepest fall in \
         the repository: {movement:?}"
    );

    let said = trend_report(&kept, Some("validate")).join("\n");
    assert!(
        said.contains("40m00s → 41m00s") && said.contains("2 distinct budget(s) (90m → 120m)"),
        "the duration leads, because it is the half of the comparison a budget \
         cannot move: {said}"
    );
    assert!(
        said.contains("held against the 120m it declares now that is 33% → 34% (+1 points)")
            && said.contains("the shares printed at the time were 44% and 34%"),
        "both numbers are said: the comparable one, and the one a reader will \
         remember seeing: {said}"
    );
    assert!(
        !said.contains("no job's share of its budget is above where it was first recorded"),
        "and a fall produced by a moved denominator is not reported as good news: \
         {said}"
    );
}

/// A run that did not complete is no point on a cost curve — and is not dropped.
///
/// THE NUMBERS ARE THIS REPOSITORY'S OWN. `validate` took 331 s on `d412b06e`,
/// where it FAILED, against some 2400 s on the commits either side; and 5415 s on
/// `cabcd5cf`, where its own ninety-minute budget ended it. The first is a
/// time-to-failure and the second is a censored measurement, and a curve through
/// either is a curve through a quantity that is not cost. The second is also the
/// most important number in the record, so the sentence carries how far it got
/// rather than letting the exclusion take it away.
#[test]
fn a_run_that_did_not_complete_is_no_point_on_a_cost_curve_and_is_not_dropped() {
    let kept = [
        record(
            "aaaaaaa",
            "2026-08-17T18:26:06Z",
            &[spent("validate", 2424, 90)],
        ),
        record(
            "bbbbbbb",
            "2026-08-18T02:40:24Z",
            &[concluded("validate", 5415, 90, "cancelled")],
        ),
        record(
            "ccccccc",
            "2026-08-19T05:10:13Z",
            &[concluded("validate", 331, 90, "failure")],
        ),
        record(
            "ddddddd",
            "2026-08-19T23:50:46Z",
            &[spent("validate", 2347, 90)],
        ),
    ];
    let movement = movements(&kept)
        .into_iter()
        .find(|one| one.check == "validate")
        .expect("the job");
    assert_eq!(
        (movement.commits, movement.set_aside),
        (2, 2),
        "two runs completed and two did not: {movement:?}"
    );
    assert_eq!(
        (movement.first.took, movement.last.took),
        (2424, 2347),
        "the ends are the completed ones, and the failure is NOT the newest \
         point — which is the shape that would have printed a collapse: \
         {movement:?}"
    );
    assert_eq!(
        (movement.low, movement.high),
        (43, 44),
        "and the range is over the comparable runs, so the 6% failure does not \
         widen it: {movement:?}"
    );
    assert_eq!(
        movement.reached, 100,
        "while the run its own budget ended is carried as how far it got: \
         {movement:?}"
    );

    let said = trend_report(&kept, Some("validate")).join("\n");
    assert!(
        said.contains(
            "`validate` 44% → 43% (-1 points over 2 commit(s) it completed; 43–44% \
                       across them"
        ),
        "{said}"
    );
    assert!(
        said.contains("2 more did not complete, the longest reaching 100%"),
        "the exclusion is a sentence and the ceiling survives it — a reader who \
         saw only the movement would not know this job has hit its budget: {said}"
    );
}

/// A job that has never completed is named rather than absent.
///
/// `movements` PRODUCES NOTHING FOR IT, which is right — there is no cost curve —
/// and would therefore take it silently out of a block that reads as covering
/// every job in the record. Where the level line has just named that job, "no
/// trend" is itself the news.
#[test]
fn a_job_with_no_completed_run_is_named_rather_than_left_out() {
    let kept = [
        record(
            "aaaaaaa",
            "2026-08-17T18:26:06Z",
            &[
                spent("healthy", 600, 90),
                concluded("broken", 100, 90, "failure"),
                concluded("also-broken", 100, 90, "failure"),
            ],
        ),
        record(
            "bbbbbbb",
            "2026-08-19T23:50:46Z",
            &[
                spent("healthy", 900, 90),
                concluded("broken", 120, 90, "failure"),
                concluded("also-broken", 120, 90, "failure"),
            ],
        ),
    ];
    assert_eq!(
        never_completed(&kept),
        ["also-broken".to_string(), "broken".to_string()]
            .into_iter()
            .collect()
    );

    let said = trend_report(&kept, Some("broken")).join("\n");
    assert!(
        said.contains("`broken` has no completed run in this record"),
        "the job the level line named is answered rather than passed over: {said}"
    );
    assert!(
        said.contains("1 further job(s) in this record have no completed run"),
        "and the ones it did not name are counted, so the block never reads as \
         covering every job in the record: {said}"
    );
    assert!(
        said.contains("`healthy` 11% → 16%"),
        "while the job that does complete still has its curve: {said}"
    );
}

/// A whole directory that stops reading at once is one fact, not thirty.
///
/// MEASURED ON THIS MACHINE while R1261 was written: the shape of a record is a
/// type, so adding one field made every record in the directory unreadable in the
/// same instant — 32 of them, each producing the identical sentence. A block of
/// thirty-two identical lines is one a reader learns to skip, which is the
/// blindness this whole reporter exists to end, arrived at from the other side.
/// The cap is the annotation block's rule one file over: name a few, count the
/// rest, never drop in silence.
#[test]
fn a_directory_that_stopped_reading_at_once_is_one_fact_and_not_thirty() {
    let tree = TempDir::new().expect("a tree");
    let directory = tree.path().join(RECORDS);
    fs::create_dir_all(&directory).expect("the record directory");
    for name in ["aaaaaaa", "bbbbbbb", "ccccccc", "ddddddd", "eeeeeee"] {
        fs::write(
            directory.join(format!("{name}.json")),
            r#"{"commit":"x","ran_at":"2026-08-18T13:40:00Z","jobs":[],"from":"another shape"}"#,
        )
        .expect("a record of a shape this reader does not know");
    }
    let said = kept_report(tree.path(), SHA, &[], &[]).join("\n");
    assert_eq!(
        said.matches("NOTE").count(),
        4,
        "three named and one line counting the rest, not five: {said}"
    );
    assert!(
        said.contains("(+2 more record(s)"),
        "and the count says how many were left out, so the block never reads as \
         though that was all of them: {said}"
    );
    assert!(
        said.contains("a record shape that changed"),
        "the sentence also says what a whole directory failing at once MEANS, \
         because the reader's next move depends on it: {said}"
    );
}

/// A share computed from a record cannot take the reporter down with it.
///
/// THESE TWO NUMBERS COME OFF DISK NOW. `took * 100` overflows a `u64` for a
/// duration nobody worked and anybody can type, and a panic here is a reporter
/// that blocks a push — which is the one thing this program's own doc comment
/// says it never does.
#[test]
fn a_duration_nobody_worked_is_a_number_rather_than_a_panic() {
    assert_eq!(spent("absurd", u64::MAX, 1).percent(), u64::MAX);
    assert_eq!(
        spent("absurd", u64::MAX, u64::MAX).percent(),
        1,
        "and nothing SATURATES on the way: a budget clamped to the largest \
         representable number would answer 100% for a job that used a sixtieth \
         of it, which is a wrong number in the alarming direction"
    );
    assert_eq!(spent("none", 600, 0).percent(), 0);

    let kept = [
        record("aaaaaaa", "2026-08-01T09:00:00Z", &[spent("job", 60, 90)]),
        record(
            "bbbbbbb",
            "2026-08-18T09:00:00Z",
            &[spent("job", u64::MAX, 1)],
        ),
    ];
    let movement = &movements(&kept)[0];
    assert_eq!(
        movement.points(),
        i64::MAX,
        "the movement saturates rather than wrapping into a fall: {movement:?}"
    );
    assert!(!trend_report(&kept, Some("job")).is_empty());
}

/// The directory this reporter writes records into is one this repository
/// collects.
///
/// THE LAW LIVES WITH THE WRITER. `scratch-budget`'s own law asks the two kinds
/// of program it can ask — every sweep manifest, which declares `logs`, and
/// `scripts/verify.sh`, which is asked directly — and this reporter is neither of
/// them, so nothing over there would ever have noticed this directory. Its
/// declaration is entitled to be a superset; what it is not entitled to be is a
/// list that misses a directory this repository writes into on every push, which
/// is the state R1199 found six directories in.
///
/// READ AS JSON RATHER THAN THROUGH `scratch_budget`: that crate carries its own
/// `[workspace]`, so a path dependency from here is the `multiple workspace roots`
/// error R1237 paid for twice. One field of a tracked file is what this needs.
#[test]
fn the_directory_this_reporter_records_into_is_one_this_repository_collects() {
    let declaration = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("tools/")
        .join("scratch-budget/scratch.json");
    let raw = fs::read_to_string(&declaration)
        .unwrap_or_else(|why| panic!("{}: {why}", declaration.display()));
    let read: serde_json::Value = serde_json::from_str(&raw).expect("the declaration is JSON");
    let directories = read["directories"]
        .as_array()
        .expect("the declaration names directories");
    assert!(
        directories.len() >= 7,
        "a declaration this law read as nearly empty would pass for the wrong \
         reason: {}",
        directories.len()
    );
    let mine = directories
        .iter()
        .find(|one| one["path"] == RECORDS)
        .unwrap_or_else(|| {
            panic!(
                "`{RECORDS}` is written on every push and no entry of {} collects \
                 it — a directory nothing collects grows for as long as the \
                 repository is worked in, and `target/` is gitignored, so nothing \
                 about it appears in a commit",
                declaration.display()
            )
        });
    assert!(
        mine["budget_mib"].as_u64().is_some_and(|mib| mib > 0),
        "a budget of nothing cannot be met and the collector would report it \
         every run: {mine}"
    );
    assert!(
        mine["why"].as_str().is_some_and(|why| why.len() > 80),
        "and the number is measured beside the entry that carries it, which is \
         how the next reader tells a budget that was chosen from one that was \
         typed: {mine}"
    );
}
