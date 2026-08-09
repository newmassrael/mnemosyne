//! What the gate refuses, and what it counts.
//!
//! Pinned against records rather than against a build, for the reason
//! `ci-plan`'s own suite is: the branches that matter are the ones this machine
//! does not take. A job whose recorder is unwired, a log from a job that was
//! deleted, two jobs sharing a unit — none of those exist in this repository on
//! the day this is written, and every one of them is what the gate is for.

use std::collections::{BTreeMap, BTreeSet};

use ci_plan::RunStep;
use twice_compiled::{
    declared_jobs, judge, names_its_job, read, read_log, Census, Cost, Invocation, JobLog, Refusal,
    Unit, WRAPPER_VARIABLE,
};

/// A record as `rustc-log` writes it: when the compiler started, how long it
/// ran, then the compiler and its arguments.
fn record(started_at: u64, micros: u64, words: &[&str]) -> String {
    let written = rustc_log::Record {
        started_at,
        micros,
        argv: words.iter().map(|word| word.to_string()).collect(),
    };
    String::from_utf8(rustc_log::encode(&written)).expect("records are utf-8")
}

/// When the fixture clocks start. Any epoch does; a round number makes a failing
/// assertion readable.
const EPOCH: u64 = 1_000_000;

/// The arguments cargo actually passes for one library unit, trimmed to the
/// flags this reader looks at and keeping the two spellings it must handle.
fn compilation(crate_name: &str, metadata: &str, emit: &str) -> Vec<String> {
    vec![
        "/home/runner/.rustup/toolchains/stable/bin/rustc".to_string(),
        "--crate-name".to_string(),
        crate_name.to_string(),
        "--edition=2021".to_string(),
        "src/lib.rs".to_string(),
        "--crate-type".to_string(),
        "lib".to_string(),
        format!("--emit={emit}"),
        "-C".to_string(),
        format!("metadata={metadata}"),
        "--out-dir".to_string(),
        "/home/runner/work/mnemosyne/mnemosyne/target/debug/deps".to_string(),
    ]
}

/// One compilation's record, placed on the clock.
fn compiled(started_at: u64, micros: u64, crate_name: &str, metadata: &str, emit: &str) -> String {
    let argv = compilation(crate_name, metadata, emit);
    let words: Vec<&str> = argv.iter().map(String::as_str).collect();
    record(started_at, micros, &words)
}

/// A job's log, built from units laid END TO END on the clock: one compiler at a
/// time, so the job's window is exactly its work. The tests that are about
/// several compilers at once say so by building their records by hand.
fn log_of(units: &[(&str, &str, &str, u64)]) -> JobLog {
    let mut clock = EPOCH;
    let text: String = units
        .iter()
        .map(|(name, metadata, emit, micros)| {
            let line = compiled(clock, *micros, name, metadata, emit);
            clock += micros;
            line
        })
        .collect();
    read_log(&text)
}

// --- reading one invocation -------------------------------------------------

#[test]
fn a_compilation_is_keyed_by_the_fingerprint_cargo_computed_for_it() {
    let argv = compilation(
        "mnemosyne_core",
        "469b061dbaa2f2d3",
        "dep-info,metadata,link",
    );
    let Invocation::Compilation(unit) = read(&argv) else {
        panic!("that is a compilation: {argv:?}");
    };
    assert_eq!(
        *unit,
        Unit {
            crate_name: "mnemosyne_core".to_string(),
            metadata: "469b061dbaa2f2d3".to_string(),
            emit: "dep-info,metadata,link".to_string(),
            crate_types: vec!["lib".to_string()],
            test: false,
        },
        "the key is cargo's own: the hash it computed from the package, the \
         resolved features, the profile and the compiler's version"
    );
}

#[test]
fn a_check_of_a_crate_is_not_the_build_of_it() {
    // WHY `--emit` IS IN THE KEY. `uncompiled-sources` runs `cargo check` over
    // the same workspace `validate` runs `cargo test` over, at the same feature
    // resolve. Everything about the two units agrees except what rustc is asked
    // to produce, and calling them one unit would report a duplication that
    // sharing a `target` cannot actually remove.
    let checked = read(&compilation("mnemosyne_core", "469b", "dep-info,metadata"));
    let built = read(&compilation(
        "mnemosyne_core",
        "469b",
        "dep-info,metadata,link",
    ));
    assert_ne!(checked, built);
}

#[test]
fn cargos_questions_to_the_compiler_are_not_compilations() {
    assert_eq!(
        read(&["rustc".to_string(), "-vV".to_string()]),
        Invocation::Probe
    );
    // THE ONE THAT LOOKS LIKE A COMPILATION: cargo's crate-type probe carries a
    // real `--crate-name`, and a reader keying on that alone would count a crate
    // called `___` in every job.
    let probe: Vec<String> = [
        "rustc",
        "-",
        "--crate-name",
        "___",
        "--print=file-names",
        "--crate-type",
        "bin",
    ]
    .iter()
    .map(|word| word.to_string())
    .collect();
    assert_eq!(read(&probe), Invocation::Probe);
}

#[test]
fn a_compilation_with_no_fingerprint_is_counted_rather_than_dropped() {
    // What a build script running `rustc` itself looks like — `autocfg` and its
    // kin. It cannot be keyed, so it cannot be joined; a reader that dropped it
    // would be reporting a total it had quietly narrowed.
    let argv: Vec<String> = ["rustc", "--crate-name", "probe0", "probe0.rs"]
        .iter()
        .map(|word| word.to_string())
        .collect();
    assert_eq!(read(&argv), Invocation::Unkeyed);

    let log = read_log(&record(
        EPOCH,
        400,
        &["rustc", "--crate-name", "probe0", "probe0.rs"],
    ));
    assert_eq!(
        (log.invocations, log.unkeyed, log.compilations()),
        (1, 1, 0)
    );
    assert_eq!(
        (log.micros, log.compiled_micros()),
        (400, 0),
        "its seconds are in the job's total and in no unit's price — a class \
         this reader cannot key is one it must not silently call free either"
    );
}

// --- counting a census ------------------------------------------------------

#[test]
fn what_two_jobs_both_compile_is_what_one_job_would_compile_once() {
    let mut census = Census::default();
    census.jobs.insert(
        "validate".to_string(),
        log_of(&[("serde", "aaa", "link", 30), ("core", "bbb", "link", 100)]),
    );
    census.jobs.insert(
        "unrun-tests".to_string(),
        log_of(&[("serde", "aaa", "link", 20), ("core", "ccc", "link", 100)]),
    );

    assert_eq!(census.paid(), 4, "CI compiles four units across two jobs");
    assert_eq!(
        census.floor(),
        3,
        "one machine sharing a target compiles three"
    );

    let shared = census.shared();
    assert_eq!(
        shared.len(),
        1,
        "exactly one unit is compiled twice: {shared:?}"
    );
    let (unit, jobs) = shared.into_iter().next().expect("one");
    assert_eq!(unit.crate_name, "serde");
    assert_eq!(jobs, vec!["unrun-tests", "validate"]);

    let pair = census
        .pairwise()
        .get(&("unrun-tests", "validate"))
        .copied()
        .expect("the pair is named, because the repair is to merge a pair");
    assert_eq!(pair.units, 1);
    assert_eq!(
        pair.saved_micros, 20,
        "priced at the CHEAPER of the two jobs' figures for it, so the saving \
         reported is the least the merge could win"
    );

    assert_eq!(
        (census.paid_micros(), census.floor_micros()),
        (250, 230),
        "the floor prices the surviving compilation at the DEARER of the two, \
         for the same reason from the other side"
    );
}

#[test]
fn a_job_that_compiles_one_unit_twice_is_reported_as_doing_so() {
    // A job holding two `target` directories — the root one and a tool
    // workspace's — pays for a shared dependency in both. That is duplication
    // inside one job, and merging jobs cannot remove it, so it is counted
    // separately rather than folded into the cross-job number.
    let log = log_of(&[("serde", "aaa", "link", 60), ("serde", "aaa", "link", 40)]);
    assert_eq!(
        (log.compilations(), log.units.len(), log.repeats()),
        (2, 1, 1)
    );
    assert_eq!(
        (
            log.compiled_micros(),
            log.units.values().next().unwrap().each()
        ),
        (100, 50),
        "one unit, two compilations, and the price of one of them is the pair's \
         average — the record does not say which of the two was the slow one"
    );
}

#[test]
fn the_window_is_read_off_the_clocks_and_not_off_the_order_of_the_lines() {
    // RECORDS ARE APPENDED WHEN A COMPILER EXITS, and cargo runs as many as the
    // machine has cores, so the earliest start routinely arrives last: the long
    // compilation that began the job finishes after the short ones that began
    // later. A reader taking the first and last LINES would read a window
    // shorter than the job's, every time, and every job would look busier than
    // it was.
    let text = format!(
        "{}{}",
        compiled(EPOCH + 500, 100, "quick", "bbb", "link"),
        compiled(EPOCH, 900, "slow", "aaa", "link"),
    );
    let log = read_log(&text);
    assert_eq!(
        log.window(),
        Some((EPOCH, EPOCH + 900)),
        "the window is the earliest start to the latest exit"
    );
    assert_eq!(log.span_micros(), 900);
    assert_eq!(
        log.compiled_micros(),
        1000,
        "and the work is larger than the window, because two compilers overlapped"
    );
}

#[test]
fn the_part_of_a_window_with_no_compiler_alive_is_measured_and_not_assumed() {
    // WHAT MERGING CANNOT REACH INSIDE ONE JOB. A window holds the minutes a
    // suite spends running the binaries it just built, and those do not get
    // shorter because the job compiled less. Assumed away, they are scaled with
    // the compiling and the estimate promises time that is not there; assumed
    // to be large, every merge looks pointless. They are the union of the
    // intervals subtracted from the window, which the records already say.
    let text = format!(
        "{}{}{}",
        // Two compilers overlapping: busy from 0 to 150, not 250.
        compiled(EPOCH, 100, "first", "aaa", "link"),
        compiled(EPOCH + 50, 100, "second", "bbb", "link"),
        // Then nothing at all until 500 — a suite running.
        compiled(EPOCH + 500, 100, "third", "ccc", "link"),
    );
    let log = read_log(&text);
    assert_eq!(log.span_micros(), 600);
    assert_eq!(
        log.busy_micros(),
        250,
        "the UNION of the intervals: 0..150 and 500..600. Summing them would \
         report 300 µs of a 600 µs window busy, which is a job busier than the \
         clock allows"
    );
    assert_eq!(log.idle_micros(), 350);
    assert_eq!(
        log.compiled_micros(),
        300,
        "and the work is larger than the busy time, because two of them ran at \
         the same moment"
    );
}

#[test]
fn a_merge_estimate_does_not_shorten_the_time_no_compiler_was_alive() {
    // THE ASSUMPTION THIS ROUND REFUSED TO CARRY. Scaling a whole window by the
    // work removed treats a job's idle minutes as though they compiled, and the
    // two jobs with the most to share in this repository are the two that spend
    // the longest running what they built. Here each job is busy for 100 µs and
    // idle for 900, and merging removes every compilation one of them does: the
    // estimate must still be about 1800 µs, not 900.
    let mut census = Census::default();
    for job in ["validate", "unrun-tests"] {
        let text = format!(
            "{}{}",
            compiled(EPOCH, 100, "serde", "aaa", "link"),
            // A last compilation of the job's own, instant, 900 µs later: the
            // window is long and almost all of it has nothing compiling in it,
            // which is what these two jobs look like in this repository.
            compiled(EPOCH + 1000, 0, "only_mine", job, "link"),
        );
        census.jobs.insert(job.to_string(), read_log(&text));
    }

    let merge = census
        .pairwise()
        .get(&("unrun-tests", "validate"))
        .copied()
        .expect("the two jobs share `serde`");
    assert_eq!((merge.floor_micros, merge.ceiling_micros), (1000, 2000));
    assert_eq!(
        (merge.units, merge.saved_micros),
        (1, 100),
        "merging removes one of the two compilations of `serde`, and all 100 µs \
         of compiling that one did"
    );
    assert_eq!(
        merge.idle_micros, 1800,
        "900 µs of each job's window had no compiler alive in it"
    );
    assert_eq!(
        merge.estimate_micros, 1900,
        "half the pair's 200 µs of compiling survives and the 1800 µs of idle \
         carries across whole. AN ESTIMATE THAT SCALED THE WHOLE WINDOW would \
         have said 2000 x 100/200 = 1000 µs — a merged job half the length of \
         the shorter of the two it replaces, which is not a thing that happens"
    );
}

// --- the law ----------------------------------------------------------------

fn step(job: &str, env: &[(&str, &str)]) -> RunStep {
    RunStep {
        job: job.to_string(),
        script: "cargo test --workspace".to_string(),
        env: env
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect(),
    }
}

fn wired(job: &str) -> RunStep {
    step(
        job,
        &[
            (WRAPPER_VARIABLE, "${{ github.workspace }}/recorder"),
            (rustc_log::LOG_VARIABLE, &format!("/w/rustc-log/{job}.log")),
        ],
    )
}

fn census_of(jobs: &[&str]) -> Census {
    let mut census = Census::default();
    for (index, job) in jobs.iter().enumerate() {
        census.jobs.insert(
            (*job).to_string(),
            log_of(&[
                ("serde", "aaa", "link", 30),
                ("only", &format!("{index}"), "link", 70),
            ]),
        );
    }
    census
}

fn nothing() -> BTreeSet<String> {
    BTreeSet::new()
}

#[test]
fn a_wired_workflow_whose_jobs_all_recorded_is_accepted() {
    let declared = declared_jobs(&[wired("validate"), wired("unrun-tests")]);
    assert!(judge(
        &census_of(&["validate", "unrun-tests"]),
        &declared,
        &nothing()
    )
    .is_empty());
}

#[test]
fn a_job_that_recorded_nothing_is_refused() {
    // THE FAILURE THIS GATE EXISTS FOR. A job whose recorder is unwired builds
    // perfectly and hands the census an absence, and an absence prints the same
    // as a job with nothing to compile.
    let declared = declared_jobs(&[wired("validate"), wired("unrun-tests")]);
    assert_eq!(
        judge(&census_of(&["validate"]), &declared, &nothing()),
        vec![Refusal::JobLeftNoRecord {
            job: "unrun-tests".to_string()
        }]
    );
}

#[test]
fn a_job_whose_compilations_took_no_time_at_all_is_refused() {
    // THE QUIETER WAY AN INSTRUMENT GOES SILENT, and the one the law above
    // cannot reach: the counts are all present, every total adds up, and only
    // the seconds are zero. It is what a job running an OLDER recorder produces
    // — a real state on a runner that restored a cached binary — and the report
    // it makes reads as work that costs nothing, which is exactly the finding
    // that would argue for merging every job in the file.
    //
    // Its control is the test above: the same census with its clocks intact is
    // accepted.
    let declared = declared_jobs(&[wired("validate"), wired("unrun-tests")]);
    let mut census = census_of(&["validate", "unrun-tests"]);
    let untimed = census.jobs.get_mut("unrun-tests").expect("the job");
    for cost in untimed.units.values_mut() {
        cost.micros = 0;
    }
    untimed.micros = 0;

    assert_eq!(
        judge(&census, &declared, &nothing()),
        vec![Refusal::JobRecordedNoTime {
            job: "unrun-tests".to_string()
        }]
    );
}

#[test]
fn a_job_that_recorded_nothing_is_refused_once_and_not_twice() {
    // A job with no compilations has no seconds either. Both laws are true of
    // it, and reporting both would name one silence twice — the second line
    // sending whoever reads it after a clock when the recorder never ran.
    let declared = declared_jobs(&[wired("validate"), wired("unrun-tests")]);
    let refusals = judge(&census_of(&["validate"]), &declared, &nothing());
    assert_eq!(
        refusals,
        vec![Refusal::JobLeftNoRecord {
            job: "unrun-tests".to_string()
        }]
    );
}

#[test]
fn a_job_the_census_cannot_hold_is_not_refused_for_being_absent() {
    // The control for the test above. The gate runs INSIDE the workflow it
    // judges, so its own build is still in flight while it judges; and a local
    // replay cannot run a job whose environment GitHub resolves. Both are named
    // by the caller from the machine, and neither is a finding.
    let declared = declared_jobs(&[wired("validate"), wired("twice-compiled")]);
    let mut absent = BTreeSet::new();
    absent.insert("twice-compiled".to_string());
    assert!(judge(&census_of(&["validate"]), &declared, &absent).is_empty());
}

#[test]
fn a_log_from_a_job_the_workflow_no_longer_declares_is_refused() {
    let declared = declared_jobs(&[wired("validate")]);
    assert_eq!(
        judge(&census_of(&["validate", "deleted"]), &declared, &nothing()),
        vec![Refusal::RecordFromNoJob {
            job: "deleted".to_string()
        }]
    );
}

#[test]
fn a_step_running_without_the_recorder_is_refused_even_when_the_job_recorded() {
    // WHY THIS IS A SEPARATE LAW. A job whose FIRST step is wired and whose
    // second is not still produces a non-empty log, so the count above passes
    // while whatever the second step compiled is missing. The wiring is read off
    // the workflow so that it cannot be removed to make the count go green.
    let declared = declared_jobs(&[wired("validate"), step("validate", &[])]);
    let refusals = judge(&census_of(&["validate"]), &declared, &nothing());
    assert_eq!(
        refusals,
        vec![
            Refusal::StepIsNotRecorded {
                job: "validate".to_string(),
                missing: rustc_log::LOG_VARIABLE.to_string(),
            },
            Refusal::StepIsNotRecorded {
                job: "validate".to_string(),
                missing: WRAPPER_VARIABLE.to_string(),
            },
        ],
        "both variables are named, because either one missing is the same silence"
    );
}

#[test]
fn an_empty_recorder_variable_is_as_missing_as_no_variable_at_all() {
    // Not pedantry: `RUSTC_WRAPPER: ""` is exactly how the recorder's own build
    // step turns itself off, and cargo reads an empty value as unset. A reader
    // checking only for the key's presence would accept a workflow that had
    // switched recording off everywhere.
    let declared = declared_jobs(&[step(
        "validate",
        &[
            (WRAPPER_VARIABLE, ""),
            (rustc_log::LOG_VARIABLE, "/w/rustc-log/validate.log"),
        ],
    )]);
    assert_eq!(
        judge(&census_of(&["validate"]), &declared, &nothing()),
        vec![Refusal::StepIsNotRecorded {
            job: "validate".to_string(),
            missing: WRAPPER_VARIABLE.to_string(),
        }]
    );
}

#[test]
fn a_job_recording_to_another_jobs_log_is_refused() {
    // THE PASTE ERROR, and it is silent otherwise: a job copied from another
    // with the log path not changed makes two jobs one blob, and a census of one
    // job reports NO duplication at all — a wrong answer wearing the shape of a
    // clean one. `${{ github.job }}` cannot spell this, because the runner
    // defines that only inside a step and it is empty in a job's `env:`, so the
    // name is written by hand and this is what makes a hand-written name safe.
    let declared = declared_jobs(&[step(
        "unrun-tests",
        &[
            (WRAPPER_VARIABLE, "/recorder"),
            (rustc_log::LOG_VARIABLE, "/w/rustc-log/validate.log"),
        ],
    )]);
    assert_eq!(
        judge(&census_of(&["unrun-tests"]), &declared, &nothing()),
        vec![Refusal::LogIsNotNamedForItsJob {
            job: "unrun-tests".to_string(),
            path: "/w/rustc-log/validate.log".to_string(),
        }]
    );

    assert!(names_its_job("/w/rustc-log/validate.log", "validate"));
    assert!(
        !names_its_job("/w/rustc-log/validate.log", "validate-msrv"),
        "a prefix is not a name — `validate` must not answer for `validate-msrv`"
    );
    assert!(
        !names_its_job("/w/rustc-log/rustc.log", "validate"),
        "and a path naming no job at all is not one job's"
    );
}

#[test]
fn the_jobs_are_read_off_the_workflow_and_not_kept_beside_it() {
    let declared: BTreeMap<_, _> =
        declared_jobs(&[wired("validate"), step("validate", &[]), wired("msrv")]);
    assert_eq!(
        declared.keys().collect::<Vec<_>>(),
        vec!["msrv", "validate"],
        "one entry per job"
    );
    assert_eq!(
        declared["validate"].len(),
        2,
        "and one environment per STEP, because a job is wired step by step"
    );
}

#[test]
fn the_step_that_builds_the_recorder_is_the_one_step_excused_from_using_it() {
    // It cannot use what it is building. The excuse is DERIVED from what the
    // step does — the manifest it names — rather than kept as a job id in a
    // list, so it stays exactly one step wide. Its control is the test above:
    // the same empty variable on a step that builds anything else is refused.
    let mut building = step(
        "validate",
        &[
            (WRAPPER_VARIABLE, ""),
            (rustc_log::LOG_VARIABLE, "/w/rustc-log/validate.log"),
        ],
    );
    building.script = format!(
        "cargo build --release -q --manifest-path {}",
        twice_compiled::RECORDER_MANIFEST
    );
    let declared = declared_jobs(&[building, wired("validate")]);
    assert!(
        judge(&census_of(&["validate"]), &declared, &nothing()).is_empty(),
        "the recorder's own build cannot be recorded by it"
    );
}

#[test]
fn the_surplus_is_reported_per_crate_because_that_is_what_picks_the_repair() {
    // Duplication in a third-party dependency and duplication in this
    // repository's own crate are the same number and different repairs — a
    // shared compilation cache answers the first, and the jobs being one job
    // answers the second. A total with no breakdown behind it licenses whichever
    // repair was already preferred.
    let mut census = Census::default();
    census.jobs.insert(
        "validate".to_string(),
        log_of(&[
            ("serde", "aaa", "link", 10),
            ("serde", "bbb", "link", 10),
            // AND ONCE MORE INSIDE THE SAME JOB — a job holding two `target`
            // directories. The first version of this test had no such row, and
            // the breakdown it checked silently dropped repeats: the report
            // printed `272 duplicated` on one line and `269 surplus` on the
            // next, from real data, and only the real data showed it.
            ("serde", "bbb", "link", 10),
            ("mine", "ccc", "link", 400),
        ]),
    );
    census.jobs.insert(
        "unrun-tests".to_string(),
        log_of(&[
            ("serde", "aaa", "link", 10),
            ("serde", "bbb", "link", 10),
            ("mine", "ddd", "link", 400),
        ]),
    );

    let surplus = census.surplus_by_crate();
    assert_eq!(
        surplus.get("serde"),
        Some(&Cost {
            times: 3,
            micros: 30
        }),
        "both of serde's units are compiled by both jobs, and one of them twice \
         over inside one of them: {surplus:?}"
    );
    assert_eq!(
        surplus.get("mine"),
        None,
        "and a crate each job compiles at a DIFFERENT resolve is not surplus — \
         that is the case `different feature resolve` actually describes"
    );
    assert_eq!(
        surplus.values().map(|cost| cost.times).sum::<usize>(),
        census.paid() - census.floor(),
        "the breakdown adds up to the total it breaks down"
    );
    assert_eq!(
        surplus.values().map(|cost| cost.micros).sum::<u64>(),
        census.paid_micros() - census.floor_micros(),
        "and so does the price of it"
    );
}

#[test]
fn the_most_numerous_surplus_and_the_dearest_are_not_the_same_crate() {
    // THE FINDING THAT PUT A CLOCK IN THE RECORD. The first census this gate
    // took ranked the duplication by rows and put `build_script_build` at the
    // head with 409 surplus compilations — a crate whose units are among the
    // cheapest cargo drives, host-side and nothing to do with the feature
    // resolve the workflow's comments blamed. Ranked by rows the repair is a
    // shared compilation cache; ranked by seconds it can be something else
    // entirely, and until the recorder carried a clock nothing here could tell
    // the two orders apart. This fixture is that shape in miniature.
    let mut census = Census::default();
    for job in ["validate", "unrun-tests"] {
        census.jobs.insert(
            job.to_string(),
            log_of(&[
                ("build_script_build", "s1", "link", 5),
                ("build_script_build", "s2", "link", 5),
                ("build_script_build", "s3", "link", 5),
                ("mnemosyne_store", "aaa", "link", 900),
            ]),
        );
    }

    let surplus = census.surplus_by_crate();
    let mut by_rows: Vec<(&str, Cost)> = surplus.clone().into_iter().collect();
    by_rows.sort_by_key(|(name, cost)| (std::cmp::Reverse(cost.times), *name));
    assert_eq!(
        by_rows.first().map(|(name, cost)| (*name, cost.times)),
        Some(("build_script_build", 3)),
        "by rows, the build scripts head the list"
    );

    let mut by_seconds: Vec<(&str, Cost)> = surplus.into_iter().collect();
    by_seconds.sort_by_key(|(name, cost)| (std::cmp::Reverse(cost.micros), *name));
    assert_eq!(
        by_seconds.first().map(|(name, cost)| (*name, cost.micros)),
        Some(("mnemosyne_store", 900)),
        "by seconds, a single unit of this repository's own outweighs all three \
         — the two rankings name two different repairs"
    );
}

#[test]
fn the_gates_own_job_is_left_out_of_the_numbers_and_not_only_out_of_the_verdict() {
    // THE INSTRUMENT MUST NOT BE IN ITS OWN READING. The gate's job runs
    // `cargo run` on this crate with the recorder active, so its own log — of a
    // build still in progress — is sitting in the directory beside the ones it
    // downloaded. Skipping it in the verdict while counting it in the totals
    // would add compilations that exist only because the measurement does.
    let scratch = std::env::temp_dir().join(format!("twice-compiled-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).expect("scratch");
    let write = |job: &str, units: &[(&str, &str, &str)]| {
        let text: String = units
            .iter()
            .map(|(name, metadata, emit)| compiled(EPOCH, 100, name, metadata, emit))
            .collect();
        std::fs::write(scratch.join(format!("{job}.log")), text).expect("write log");
    };
    write("validate", &[("serde", "aaa", "link")]);
    write("twice-compiled", &[("yaml_rust2", "zzz", "link")]);

    let mut absent = BTreeSet::new();
    absent.insert("twice-compiled".to_string());
    let census = twice_compiled::load(&scratch, &absent).expect("load");
    let _ = std::fs::remove_dir_all(&scratch);

    assert_eq!(
        census.jobs.keys().collect::<Vec<_>>(),
        vec!["validate"],
        "the gate's own log is not part of the census"
    );
    assert_eq!(census.paid(), 1, "nor of what CI is said to pay for");
}

#[test]
fn the_surplus_splits_into_the_half_merging_reaches_and_the_half_it_does_not() {
    // A SINGLE PERCENTAGE LICENSES WHICHEVER REPAIR WAS ALREADY PREFERRED. Two
    // jobs emitting the same fingerprint are resolving identically, so one job
    // with one `target` compiles it once — that half merging removes. A job that
    // builds several SEPARATE workspaces holds a `target` per workspace and
    // compiles a shared dependency in each, on one runner under one cache key —
    // that half merging cannot touch, and it is the larger one in this
    // repository's `side-workspaces`.
    let mut census = Census::default();
    census.jobs.insert(
        "validate".to_string(),
        log_of(&[("serde", "aaa", "link", 40), ("serde", "aaa", "link", 40)]),
    );
    census.jobs.insert(
        "unrun-tests".to_string(),
        log_of(&[("serde", "aaa", "link", 40)]),
    );

    assert_eq!(census.paid(), 3);
    assert_eq!(census.floor(), 1);
    assert_eq!(
        census.repeated_within_jobs(),
        1,
        "`validate` compiled it twice by itself"
    );
    assert_eq!(
        census.shared_between_jobs(),
        1,
        "and the two jobs compiled it once each"
    );
    assert_eq!(
        census.shared_between_jobs() + census.repeated_within_jobs(),
        census.paid() - census.floor(),
        "the two halves are the whole surplus and nothing else"
    );

    // AND THE SAME SPLIT IN SECONDS, which is the half of the report a repair
    // is actually chosen on. It is asserted here rather than trusted because the
    // cross-job figure is DEFINED as what is left over: if the two summed halves
    // ever failed to be the whole, this is the line that says so.
    assert_eq!(
        (
            census.paid_micros(),
            census.floor_micros(),
            census.repeated_within_jobs_micros(),
            census.shared_between_jobs_micros(),
        ),
        (120, 40, 40, 40),
    );
    assert_eq!(
        census.shared_between_jobs_micros() + census.repeated_within_jobs_micros(),
        census.paid_micros() - census.floor_micros(),
        "the two halves are the whole surplus in seconds too"
    );
}

#[test]
fn merging_a_pair_states_what_it_removes_and_what_it_spends() {
    // BOTH SIDES OF THE TRADE. `unrun-tests` and `side-workspaces` share more
    // units than any other pair in this repository AND are its two longest jobs,
    // so the pair with the most to save is also the pair with the most to lose:
    // merged, one of them starts where the other stops. A report that named only
    // the saving would be an argument with the cost left out — and this
    // repository has already written that argument down four times.
    let mut census = Census::default();
    census.jobs.insert(
        "validate".to_string(),
        log_of(&[("serde", "aaa", "link", 100), ("core", "bbb", "link", 300)]),
    );
    census.jobs.insert(
        "unrun-tests".to_string(),
        log_of(&[("serde", "aaa", "link", 60), ("core", "ccc", "link", 240)]),
    );

    let merge = census
        .pairwise()
        .get(&("unrun-tests", "validate"))
        .copied()
        .expect("the two jobs share a unit");
    assert_eq!(merge.units, 1);
    assert_eq!(
        merge.saved_micros, 60,
        "the cheaper of the two prices for the unit that stops being compiled"
    );
    // Each fixture job compiles one unit at a time, so its window IS its work:
    // 400 and 300.
    assert_eq!(
        (merge.floor_micros, merge.ceiling_micros),
        (400, 700),
        "merged it cannot be quicker than the longer job it replaces, nor slower \
         than the two of them end to end"
    );
    assert_eq!(
        merge.estimate_micros, 640,
        "and between them, the pair's window scaled by the work that is left: \
         700 × (700 − 60) / 700"
    );
    assert!(
        merge.estimate_micros > merge.floor_micros,
        "MERGING SPENDS WALL-CLOCK. Today these two run beside one another and \
         cost the run {} µs; merged they cost more, and the saving is compute",
        merge.floor_micros
    );
}
