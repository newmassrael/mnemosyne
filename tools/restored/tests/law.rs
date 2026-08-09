//! The record's shape, and the four states a job can start in.
//!
//! THE ORACLE IS THE TYPE. `Warmth` has a variant for each answer the two
//! instruments can jointly give, including the one where they contradict each
//! other, so a state nobody thought about cannot arrive as one of the ordinary
//! three. Every test below drives that function from a record rather than
//! asserting on a string, because the string is a report and the value is what
//! `tools/twice-compiled` compares two censuses by.

use restored::{decode, Malformed, Measurement, Restoration, Restored, Side, Warmth};

/// Assemble a record the way the two steps do, so the tests read what a job
/// would have written rather than a hand-built string.
fn written(job: &str, exact: bool, paths: &[(&str, u64, u64)]) -> String {
    let mut out = restored::encode_job(job);
    for (path, before, _) in paths {
        out.extend_from_slice(&restored::encode_side(
            Side::Before,
            path,
            &Measurement {
                entries: *before / 10,
                bytes: *before,
            },
        ));
    }
    for (path, _, after) in paths {
        out.extend_from_slice(&restored::encode_side(
            Side::After,
            path,
            &Measurement {
                entries: *after / 10,
                bytes: *after,
            },
        ));
    }
    out.extend_from_slice(&restored::encode_exact(exact));
    String::from_utf8(out).expect("the record is text")
}

#[test]
fn a_record_reads_back_as_what_was_written() {
    let text = written(
        "unrun-tests",
        false,
        &[("~/.cargo/registry", 0, 700), ("target", 0, 6_800)],
    );
    let record = decode(&text).expect("a whole record decodes");
    assert_eq!(
        record,
        Restored {
            job: "unrun-tests".to_string(),
            exact: false,
            paths: vec![
                Restoration {
                    path: "~/.cargo/registry".to_string(),
                    before: Measurement {
                        entries: 0,
                        bytes: 0
                    },
                    after: Measurement {
                        entries: 70,
                        bytes: 700
                    },
                },
                Restoration {
                    path: "target".to_string(),
                    before: Measurement {
                        entries: 0,
                        bytes: 0
                    },
                    after: Measurement {
                        entries: 680,
                        bytes: 6_800
                    },
                },
            ],
        }
    );
    assert_eq!(record.measured(), vec!["~/.cargo/registry", "target"]);
}

/// THE STATE ROUND 1099 COULD NOT SEE, and the reason this crate exists: the
/// primary key missed, `actions/cache` reported `cache-hit: false`, and a whole
/// previous generation arrived through `restore-keys`.
#[test]
fn a_missed_key_that_restored_a_previous_generation_is_not_a_cold_job() {
    let text = written("unrun-tests", false, &[("target", 0, 7_466_000_000)]);
    let record = decode(&text).expect("a whole record decodes");
    assert_eq!(
        record.warmth(),
        Warmth::PrefixHit {
            bytes: 7_466_000_000
        }
    );
    // AND IT IS NOT THE SAME VALUE AS A COLD JOB, which is the whole finding:
    // the two states differ in nothing `cache-hit` or the cache API can see,
    // and they differ here.
    let cold = decode(&written("unrun-tests", false, &[("target", 0, 0)])).expect("decodes");
    assert_eq!(cold.warmth(), Warmth::Nothing);
    assert_ne!(record.warmth(), cold.warmth());
}

#[test]
fn an_exact_hit_that_restored_a_tree_is_the_ordinary_warm_job() {
    let record =
        decode(&written("validate", true, &[("~/.cargo/registry", 0, 100)])).expect("decodes");
    assert_eq!(record.warmth(), Warmth::ExactHit { bytes: 100 });
}

/// The two instruments contradicting each other is a state of its own, not a
/// silent fallback into one of the three.
#[test]
fn an_exact_hit_that_brought_nothing_is_neither_warm_nor_cold() {
    let record =
        decode(&written("validate", true, &[("~/.cargo/registry", 40, 40)])).expect("decodes");
    assert_eq!(record.warmth(), Warmth::HitThatBroughtNothing);
}

/// What was already there is not what the restore brought.
///
/// THE CONTROL FOR THE WHOLE MEASUREMENT. The steps before the cache run cargo,
/// so `~/.cargo` is not guaranteed empty; reading the AFTER figure alone would
/// call a cold job warm for whatever those steps left behind.
#[test]
fn only_what_arrived_across_the_restore_counts_as_restored() {
    let record = decode(&written(
        "side-workspaces",
        false,
        &[("~/.cargo/registry", 4_096, 4_096)],
    ))
    .expect("decodes");
    assert_eq!(record.bytes_restored(), 0);
    assert_eq!(record.warmth(), Warmth::Nothing);
}

#[test]
fn a_path_shrinking_across_the_restore_does_not_read_as_negative_warmth() {
    let record = decode(&written("validate", false, &[("target", 900, 100)])).expect("decodes");
    assert_eq!(record.bytes_restored(), 0);
    assert_eq!(record.warmth(), Warmth::Nothing);
}

/// A RECORD WITH NO `exact` LINE IS A JOB THAT DIED BETWEEN THE TWO STEPS, and
/// the difference between that and a job that restored nothing is the whole
/// point of the file.
#[test]
fn a_record_the_second_step_never_finished_is_refused_rather_than_read_as_cold() {
    let mut out = restored::encode_job("unrun-tests");
    out.extend_from_slice(&restored::encode_side(
        Side::Before,
        "target",
        &Measurement::default(),
    ));
    let text = String::from_utf8(out).expect("text");
    assert_eq!(
        decode(&text),
        Err(Malformed::ExactIsNotSaidOnce { times: 0 })
    );
}

#[test]
fn a_path_measured_on_one_side_only_is_refused() {
    let mut out = restored::encode_job("validate");
    out.extend_from_slice(&restored::encode_side(
        Side::Before,
        "target",
        &Measurement::default(),
    ));
    out.extend_from_slice(&restored::encode_side(
        Side::After,
        "~/.cargo/registry",
        &Measurement::default(),
    ));
    out.extend_from_slice(&restored::encode_exact(false));
    let text = String::from_utf8(out).expect("text");
    assert_eq!(
        decode(&text),
        Err(Malformed::PathIsNotOnBothSides {
            path: "target".to_string()
        })
    );
}

#[test]
fn a_path_measured_after_and_not_before_is_refused_too() {
    let mut out = restored::encode_job("validate");
    out.extend_from_slice(&restored::encode_side(
        Side::Before,
        "target",
        &Measurement::default(),
    ));
    out.extend_from_slice(&restored::encode_side(
        Side::After,
        "target",
        &Measurement::default(),
    ));
    out.extend_from_slice(&restored::encode_side(
        Side::After,
        "~/.cargo/git",
        &Measurement::default(),
    ));
    out.extend_from_slice(&restored::encode_exact(false));
    let text = String::from_utf8(out).expect("text");
    assert_eq!(
        decode(&text),
        Err(Malformed::PathIsNotOnBothSides {
            path: "~/.cargo/git".to_string()
        })
    );
}

#[test]
fn a_record_naming_two_jobs_is_refused() {
    let text = format!(
        "{}{}",
        written("validate", false, &[("target", 0, 1)]),
        String::from_utf8(restored::encode_job("unrun-tests")).expect("text")
    );
    assert_eq!(decode(&text), Err(Malformed::JobIsNotSaidOnce { times: 2 }));
}

#[test]
fn a_record_with_no_path_at_all_is_refused() {
    let mut out = restored::encode_job("validate");
    out.extend_from_slice(&restored::encode_exact(true));
    let text = String::from_utf8(out).expect("text");
    assert_eq!(decode(&text), Err(Malformed::NoPathsAtAll));
}

#[test]
fn a_cache_hit_that_is_not_a_boolean_is_refused() {
    let text = written("validate", true, &[("target", 0, 1)]).replace("true", "yes");
    assert_eq!(
        decode(&text),
        Err(Malformed::ExactIsNotABoolean {
            value: "yes".to_string()
        })
    );
}

#[test]
fn a_measurement_that_is_not_a_number_is_refused() {
    let text = written("validate", true, &[("target", 0, 10)]).replace('1', "x");
    assert!(
        matches!(decode(&text), Err(Malformed::NotANumber { .. })),
        "{:?}",
        decode(&text)
    );
}

#[test]
fn a_line_this_format_does_not_define_is_refused() {
    let text = format!(
        "{}during\u{1f}target\u{1f}0\u{1f}0\n",
        written("validate", true, &[("target", 0, 1)])
    );
    assert_eq!(
        decode(&text),
        Err(Malformed::UnknownLine {
            line: "during\u{1f}target\u{1f}0\u{1f}0".to_string()
        })
    );
}

#[test]
fn a_line_with_the_wrong_number_of_fields_is_refused() {
    let text = format!("{}job\n", written("validate", true, &[("target", 0, 1)]));
    assert_eq!(
        decode(&text),
        Err(Malformed::WrongShape {
            line: "job".to_string()
        })
    );
}

/// The paste error: a job writing the record of another job.
#[test]
fn a_record_is_named_for_the_job_that_wrote_it() {
    assert!(restored::names_its_job(
        "/home/runner/work/x/x/rustc-log/unrun-tests.restored",
        "unrun-tests"
    ));
    assert!(!restored::names_its_job(
        "/home/runner/work/x/x/rustc-log/validate.restored",
        "unrun-tests"
    ));
    // A log is not a record: the two files sit in one directory and are read by
    // one gate, so the suffix is part of the name.
    assert!(!restored::names_its_job(
        "rustc-log/unrun-tests.log",
        "unrun-tests"
    ));
}

#[test]
fn a_declared_path_keeps_its_spelling_and_is_expanded_only_to_be_measured() {
    assert_eq!(
        restored::expand("~/.cargo/registry", "/home/runner"),
        std::path::Path::new("/home/runner/.cargo/registry")
    );
    assert_eq!(
        restored::expand("target", "/home/runner"),
        std::path::Path::new("target")
    );
    // `~` ONLY AT THE HEAD, and only as a whole component: a directory that
    // happens to start with a tilde is a directory.
    assert_eq!(
        restored::expand("~cargo/registry", "/home/runner"),
        std::path::Path::new("~cargo/registry")
    );
}
