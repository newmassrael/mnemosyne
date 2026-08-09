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
    declared_jobs, judge, names_its_job, read, read_log, Census, Invocation, JobLog, Refusal, Unit,
    WRAPPER_VARIABLE,
};

/// A record as `rustc-log` writes it: the compiler first, then its arguments.
fn record(words: &[&str]) -> String {
    let argv: Vec<String> = words.iter().map(|word| word.to_string()).collect();
    String::from_utf8(rustc_log::encode(&argv)).expect("records are utf-8")
}

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

fn log_of(units: &[(&str, &str, &str)]) -> JobLog {
    let text: String = units
        .iter()
        .map(|(name, metadata, emit)| {
            let argv = compilation(name, metadata, emit);
            let words: Vec<&str> = argv.iter().map(String::as_str).collect();
            record(&words)
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

    let log = read_log(&record(&["rustc", "--crate-name", "probe0", "probe0.rs"]));
    assert_eq!(
        (log.invocations, log.unkeyed, log.compilations()),
        (1, 1, 0)
    );
}

// --- counting a census ------------------------------------------------------

#[test]
fn what_two_jobs_both_compile_is_what_one_job_would_compile_once() {
    let mut census = Census::default();
    census.jobs.insert(
        "validate".to_string(),
        log_of(&[("serde", "aaa", "link"), ("core", "bbb", "link")]),
    );
    census.jobs.insert(
        "unrun-tests".to_string(),
        log_of(&[("serde", "aaa", "link"), ("core", "ccc", "link")]),
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

    assert_eq!(
        census.pairwise().get(&("unrun-tests", "validate")),
        Some(&1),
        "and the pair is named, because the repair is to merge a pair"
    );
}

#[test]
fn a_job_that_compiles_one_unit_twice_is_reported_as_doing_so() {
    // A job holding two `target` directories — the root one and a tool
    // workspace's — pays for a shared dependency in both. That is duplication
    // inside one job, and merging jobs cannot remove it, so it is counted
    // separately rather than folded into the cross-job number.
    let log = log_of(&[("serde", "aaa", "link"), ("serde", "aaa", "link")]);
    assert_eq!(
        (log.compilations(), log.units.len(), log.repeats()),
        (2, 1, 1)
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
                ("serde", "aaa", "link"),
                ("only", &format!("{index}"), "link"),
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
            ("serde", "aaa", "link"),
            ("serde", "bbb", "link"),
            // AND ONCE MORE INSIDE THE SAME JOB — a job holding two `target`
            // directories. The first version of this test had no such row, and
            // the breakdown it checked silently dropped repeats: the report
            // printed `272 duplicated` on one line and `269 surplus` on the
            // next, from real data, and only the real data showed it.
            ("serde", "bbb", "link"),
            ("mine", "ccc", "link"),
        ]),
    );
    census.jobs.insert(
        "unrun-tests".to_string(),
        log_of(&[
            ("serde", "aaa", "link"),
            ("serde", "bbb", "link"),
            ("mine", "ddd", "link"),
        ]),
    );

    let surplus = census.surplus_by_crate();
    assert_eq!(
        surplus.get("serde"),
        Some(&3),
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
        surplus.values().sum::<usize>(),
        census.paid() - census.floor(),
        "the breakdown adds up to the total it breaks down"
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
            .map(|(name, metadata, emit)| {
                let argv = compilation(name, metadata, emit);
                let words: Vec<&str> = argv.iter().map(String::as_str).collect();
                record(&words)
            })
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
        log_of(&[("serde", "aaa", "link"), ("serde", "aaa", "link")]),
    );
    census.jobs.insert(
        "unrun-tests".to_string(),
        log_of(&[("serde", "aaa", "link")]),
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
}
