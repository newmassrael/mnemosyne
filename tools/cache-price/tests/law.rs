//! The pairing, the clock and the spread — the decisions this program makes once
//! GitHub's answer is in.
//!
//! `github.rs` holds the READING against recorded bodies. This file is about what
//! is done with it, and the two do not substitute: a program that parses GitHub
//! perfectly and pairs the wrong steps prices a cache at half its cost.

use cache_price::{by_cache, prices_in, seconds_between, spread, Job, Price, Step};

fn step(name: &str, from: Option<&str>, to: Option<&str>) -> Step {
    Step {
        name: name.to_string(),
        started_at: from.map(str::to_string),
        completed_at: to.map(str::to_string),
    }
}

fn job(name: &str, steps: Vec<Step>) -> Job {
    Job {
        name: name.to_string(),
        conclusion: Some("success".to_string()),
        started_at: Some("2026-08-10T21:05:00Z".to_string()),
        completed_at: Some("2026-08-10T21:15:00Z".to_string()),
        steps,
    }
}

/// A cache is its restore step AND its save step, paired by name.
#[test]
fn a_cache_is_priced_from_both_halves_of_its_pair() {
    let jobs = [job(
        "builder",
        vec![
            step("Set up job", None, None),
            step(
                "Cache cargo (build)",
                Some("2026-08-10T21:05:31Z"),
                Some("2026-08-10T21:07:55Z"),
            ),
            step("cargo test", None, None),
            step(
                "Post Cache cargo (build)",
                Some("2026-08-10T21:11:22Z"),
                Some("2026-08-10T21:13:15Z"),
            ),
        ],
    )];
    let prices = prices_in(&jobs).expect("a pair is a price");
    assert_eq!(
        prices,
        vec![Price {
            job: "builder".to_string(),
            cache: "cargo (build)".to_string(),
            restore: Some(144),
            save: Some(113),
        }]
    );
    assert_eq!(prices[0].total(), Some(257));
}

/// `Post Cache x` is a SAVE and never a second restore.
///
/// THE PREFIXES OVERLAP AND THAT IS THE TRAP: `Post Cache x` contains `Cache `.
/// A reader that tested the restore prefix first, or tested it with `contains`,
/// files every save as a restore of a cache named `x` — and then the pair has two
/// restores and no save, which reads as a repository whose caches are never
/// written.
#[test]
fn a_post_step_is_the_save_and_not_a_second_restore() {
    let jobs = [job(
        "builder",
        vec![
            step(
                "Cache cargo (build)",
                Some("2026-08-10T21:00:00Z"),
                Some("2026-08-10T21:00:10Z"),
            ),
            step(
                "Post Cache cargo (build)",
                Some("2026-08-10T21:05:00Z"),
                Some("2026-08-10T21:05:20Z"),
            ),
        ],
    )];
    let prices = prices_in(&jobs).expect("one pair");
    assert_eq!(prices.len(), 1, "one cache, not two: {prices:?}");
    assert_eq!(prices[0].cache, "cargo (build)");
    assert_eq!((prices[0].restore, prices[0].save), (Some(10), Some(20)));
}

/// Half a pair is a refusal, from either side.
#[test]
fn half_a_pair_is_refused_from_either_side() {
    let restore_only = [job(
        "builder",
        vec![step(
            "Cache cargo (build)",
            Some("2026-08-10T21:00:00Z"),
            Some("2026-08-10T21:00:10Z"),
        )],
    )];
    let why = prices_in(&restore_only).expect_err("a restore with no save");
    assert!(
        why.contains("never saves it") && why.contains("cargo (build)"),
        "{why}"
    );

    let save_only = [job(
        "builder",
        vec![step(
            "Post Cache cargo (build)",
            Some("2026-08-10T21:00:00Z"),
            Some("2026-08-10T21:00:10Z"),
        )],
    )];
    let why = prices_in(&save_only).expect_err("a save with no restore");
    assert!(why.contains("never restores it"), "{why}");
}

/// Two steps for one cache is a total nobody can defend.
#[test]
fn one_cache_named_twice_in_a_job_is_refused() {
    let jobs = [job(
        "builder",
        vec![
            step(
                "Cache cargo (build)",
                Some("2026-08-10T21:00:00Z"),
                Some("2026-08-10T21:00:10Z"),
            ),
            step(
                "Cache cargo (build)",
                Some("2026-08-10T21:01:00Z"),
                Some("2026-08-10T21:01:30Z"),
            ),
            step(
                "Post Cache cargo (build)",
                Some("2026-08-10T21:05:00Z"),
                Some("2026-08-10T21:05:20Z"),
            ),
        ],
    )];
    let why = prices_in(&jobs).expect_err("two prices for one cache");
    assert!(
        why.contains("two") && why.contains("cargo (build)"),
        "{why}"
    );
}

/// A step that did not run is priced at nothing at all — not at zero.
///
/// A ZERO JOINS THE SAMPLE AND PULLS EVERY SUMMARY DOWN. `None` leaves it out and
/// says so, which is the same distinction this whole round is about.
#[test]
fn a_step_that_did_not_run_is_not_a_step_that_cost_nothing() {
    let jobs = [job(
        "builder",
        vec![
            step("Cache cargo (build)", None, None),
            step("Post Cache cargo (build)", None, None),
        ],
    )];
    let prices = prices_in(&jobs).expect("a pair that did not run is still a pair");
    assert_eq!((prices[0].restore, prices[0].save), (None, None));
    assert_eq!(prices[0].total(), None, "and it contributes no total");
}

/// One stamp and not the other is neither, and is refused.
#[test]
fn a_step_with_one_stamp_is_refused() {
    let jobs = [job(
        "builder",
        vec![
            step("Cache cargo (build)", Some("2026-08-10T21:00:00Z"), None),
            step(
                "Post Cache cargo (build)",
                Some("2026-08-10T21:05:00Z"),
                Some("2026-08-10T21:05:20Z"),
            ),
        ],
    )];
    let why = prices_in(&jobs).expect_err("half a measurement");
    assert!(why.contains("neither a step that ran"), "{why}");
}

/// The clock: a plain difference, one midnight, and a ceiling.
#[test]
fn the_clock_reads_a_difference_and_carries_one_midnight() {
    assert_eq!(
        seconds_between("2026-08-10T21:05:31Z", "2026-08-10T21:07:55Z").expect("plain"),
        144
    );
    assert_eq!(
        seconds_between("2026-08-10T23:59:30Z", "2026-08-11T00:01:00Z").expect("midnight"),
        90,
        "a step that crosses midnight is ninety seconds, not a negative day"
    );
}

/// Stamps that run backwards are refused rather than read as almost a day.
///
/// A SKIPPED JOB REALLY DOES REPORT THEM, which is why this is a case: the
/// recording in `github.rs` carries `14:01:37` to `14:01:36`, and the midnight
/// carry alone turns that one second into 86 399.
#[test]
fn stamps_that_run_backwards_are_refused_rather_than_read_as_a_day() {
    let why = seconds_between("2026-08-10T14:01:37Z", "2026-08-10T14:01:36Z")
        .expect_err("one second the wrong way is not 86399 seconds");
    assert!(
        why.contains("six hours") && why.contains("86399"),
        "and it says what it read: {why}"
    );
}

/// A stamp this program cannot read is a refusal, not a zero duration.
#[test]
fn an_unreadable_stamp_is_a_refusal() {
    // THE LAST TWO WERE ADDED BY AN INJECTION THAT CAME BACK 0 RED. The first
    // four are all refused BEFORE the fields are parsed — three carry no `T` and
    // the fourth is caught by the hour bound — so the parse itself had no oracle,
    // and a reader that answered `0` for a field it could not read passed this
    // case untouched. A stamp with unparseable fields and one with too few of
    // them are what actually reach it.
    for bad in [
        "",
        "2026-08-10",
        "2026-08-10T25:00:00Z",
        "notatime",
        "2026-08-10Tab:cd:efZ",
        "2026-08-10T21:00Z",
    ] {
        let why = seconds_between(bad, "2026-08-10T21:00:00Z")
            .expect_err("an unreadable stamp is not a duration");
        assert!(why.contains("fixed-width UTC"), "for {bad:?}: {why}");
    }
}

/// The spread says how far the noisy term spreads, and counts what it summarised.
#[test]
fn the_spread_reports_what_was_measured_and_how_many() {
    let seen = spread(&[134, 144, 188, 3]).expect("four measurements");
    assert_eq!(
        (seen.count, seen.min, seen.max, seen.total),
        (4, 3, 188, 469)
    );
    assert_eq!(
        seen.median, 134,
        "the lower middle, so it is a measured value"
    );
}

/// A summary of nothing is not a summary of zeroes.
#[test]
fn a_summary_of_no_measurements_is_none_rather_than_zero() {
    assert_eq!(spread(&[]), None);
}

/// Two jobs restoring the same archive pay for it twice.
#[test]
fn one_archive_restored_by_two_jobs_is_two_prices() {
    let prices = [
        Price {
            job: "a".to_string(),
            cache: "cargo (build)".to_string(),
            restore: Some(10),
            save: Some(1),
        },
        Price {
            job: "b".to_string(),
            cache: "cargo (build)".to_string(),
            restore: Some(20),
            save: Some(2),
        },
    ];
    let grouped = by_cache(&prices);
    assert_eq!(
        grouped.len(),
        2,
        "a cache is a job's — merging them would report one price for two costs"
    );
}
