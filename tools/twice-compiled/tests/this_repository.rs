//! The law against THIS repository's own workflow, which is where the gate has
//! to be non-vacuous rather than merely correct.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use twice_compiled::{judge, Census, Cost, Declared, JobLog, Refusal, Unit};

/// The workflow this gate judges. One file, because a census can only be taken
/// of jobs that share a run: artifacts do not cross from one workflow run to
/// another, and `evidence-replay.yml` is a separate run on separate machines.
const WORKFLOW: &str = ".github/workflows/mnemosyne-validate.yml";

/// The job that runs this gate.
const GATE: &str = "twice-compiled";

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("this crate lives two directories below the repository root")
        .to_path_buf()
}

fn workflow_steps() -> Vec<ci_plan::RunStep> {
    let root = repository_root();
    ci_plan::run_steps(&ci_plan::load_workflow(&root, WORKFLOW))
}

/// What THIS repository's workflow declares — both populations, from the live
/// file.
fn declared_jobs(steps: &[ci_plan::RunStep]) -> Declared {
    let root = repository_root();
    let document = ci_plan::load_workflow(&root, WORKFLOW);
    Declared::of(steps, &ci_plan::cache_steps(&document, WORKFLOW))
}

/// A census in which every declared job compiled something AND took time doing
/// it, so that the only thing left for [`judge`] to refuse is the WIRING.
/// Separating the two is what lets this test be about the workflow rather than
/// about a build.
fn everybody_compiled(declared: &Declared) -> Census {
    let mut census = Census::default();
    for (index, job) in declared.jobs.keys().enumerate() {
        let mut log = JobLog {
            invocations: 1,
            micros: PINNED.micros,
            intervals: vec![(1_000_000, 1_000_000 + PINNED.micros)],
            ..JobLog::default()
        };
        log.units.insert(
            Unit {
                crate_name: "pinned".to_string(),
                metadata: format!("{index:04x}"),
                emit: "link".to_string(),
                crate_types: vec!["lib".to_string()],
                test: false,
            },
            PINNED,
        );
        census.jobs.insert(job.clone(), log);
    }
    // AND EVERY JOB WITH A CACHE SAID WHAT IT STARTED FROM, for the same reason
    // the compilations are pinned: this fixture exists to leave only the WIRING
    // for the judge to speak about. What is asserted below is that the workflow
    // asks each of these jobs to write the record — not that any run produced
    // one.
    for (job, paths) in &declared.caches {
        let mut written = restored::encode_job(job);
        for path in paths {
            written.extend_from_slice(&restored::encode_side(
                restored::Side::Before,
                path,
                &restored::Measurement::default(),
            ));
        }
        for path in paths {
            written.extend_from_slice(&restored::encode_side(
                restored::Side::After,
                path,
                &restored::Measurement {
                    entries: 1,
                    bytes: 1_000,
                },
            ));
        }
        written.extend_from_slice(&restored::encode_exact(true));
        census.restored.insert(
            job.clone(),
            restored::decode(&String::from_utf8(written).expect("the record is text")),
        );
    }
    census
}

/// What each pinned job compiled: one unit, once, in a time that is not zero.
///
/// NOT ZERO ON PURPOSE. A census whose seconds are all zero is refused, and this
/// fixture exists to leave only the wiring for the judge to speak about.
const PINNED: Cost = Cost {
    times: 1,
    micros: 1_000,
};

#[test]
fn every_job_this_workflow_runs_records_what_it_compiles() {
    // THE ONE THAT MATTERS. A job added tomorrow without the recorder in its
    // environment builds perfectly and contributes an absence, and an absence
    // prints the same as a job with nothing to compile. Nothing else in this
    // repository can notice that, which is why it is asserted here rather than
    // left to whoever adds the job.
    let declared = declared_jobs(&workflow_steps());
    let refusals: Vec<Refusal> = judge(&everybody_compiled(&declared), &declared, &BTreeSet::new());
    assert!(
        refusals.is_empty(),
        "{WORKFLOW} has {} job(s) and these are not recorded:\n{}",
        declared.jobs.len(),
        refusals
            .iter()
            .map(|refusal| format!("  {refusal}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn this_workflow_has_the_jobs_the_census_is_worth_taking_of() {
    // Non-vacuity. A census over one job has no cross-job duplication BY
    // CONSTRUCTION and would report a clean zero forever.
    let declared = declared_jobs(&workflow_steps());
    assert!(
        declared.jobs.len() > 2,
        "{WORKFLOW} declares {} job(s) with `run:` steps — this gate's whole \
         subject is what TWO of them both compile",
        declared.jobs.len()
    );
    assert!(
        declared.jobs.contains_key(GATE),
        "the job running this gate is `{GATE}`, and {WORKFLOW} declares {:?}",
        declared.jobs.keys().collect::<Vec<_>>()
    );
    // AND THE TWO POPULATIONS ARE NOT THE SAME POPULATION, which is why they
    // travel together but apart. Every job with a cache owes this census a
    // record of what that cache brought; the two gate jobs cache nothing and
    // owe none, and a law that asked them for one would refuse a workflow that
    // is right.
    assert!(
        declared.caches.len() >= 2,
        "{WORKFLOW} declares caches for {:?} — a census whose jobs restore \
         nothing has no cache state to be taken in",
        declared.caches.keys().collect::<Vec<_>>()
    );
    assert!(
        !declared.caches.contains_key(GATE),
        "`{GATE}` takes no cache of its own, and its own suite says so"
    );
}

#[test]
fn the_jobs_a_local_replay_cannot_run_are_exactly_these_three() {
    // THE SECOND ENTRANCE, HELD OPEN BY A TEST THAT READS THE LIVE FILE. The
    // gate has two ways in: the logs a CI run leaves, and a replay of the same
    // jobs on this machine. The day the recorder was wired into all nine jobs,
    // every one of them gained `MNEMOSYNE_RUSTC_LOG: ${{ github.workspace }}/…`
    // — an expression only GitHub resolves, and one the replay REPLACES before
    // running anything. The replay read it anyway and refused all nine, then
    // reported a census of no jobs as a clean file. Nothing noticed for a round,
    // because a replay costs hours and nobody ran it again.
    //
    // So the skip list is asserted against the workflow itself. A job that
    // becomes unreplayable tomorrow turns this red instead of quietly leaving
    // the census.
    let declared = declared_jobs(&workflow_steps());
    let skipped: BTreeSet<&str> = declared
        .jobs
        .iter()
        .filter(|(_, steps)| {
            let borrowed: Vec<&ci_plan::RunStep> = steps.iter().collect();
            twice_compiled::unresolvable(&borrowed).is_some()
        })
        .map(|(job, _)| job.as_str())
        .collect();
    let expected: BTreeSet<&str> = ["cache-budget", "msrv", "twice-compiled"]
        .into_iter()
        .collect();
    assert_eq!(
        skipped, expected,
        "`msrv` reads a toolchain GitHub resolves from a step this machine does \
         not run, `cache-budget` needs a token, and the gate does not replay \
         itself. Every OTHER job must be replayable — six of them are the \
         census, and a census of fewer than two jobs is refused"
    );
    assert!(
        declared.jobs.len() - skipped.len() >= 2,
        "a replay that can run fewer than two jobs cannot produce a census at \
         all: {declared:?}"
    );
}

#[test]
fn the_gate_waits_for_every_job_whose_log_it_reads() {
    // The same constraint `cache-budget` carries, for the same reason and read
    // the same way: a gate running BESIDE the jobs it judges reads an absence
    // for every job still building and calls it a finding. The list is not
    // trusted — it is compared against the jobs the file declares, so a job
    // added tomorrow and forgotten here turns this red instead of turning the
    // gate into a liar.
    let root = repository_root();
    let document = ci_plan::load_workflow(&root, WORKFLOW);
    let needs = ci_plan::job_needs(&document);
    let waited: BTreeSet<&str> = needs
        .get(GATE)
        .unwrap_or_else(|| panic!("{WORKFLOW} declares no job `{GATE}`"))
        .iter()
        .map(String::as_str)
        .collect();
    let others: BTreeSet<&str> = needs
        .keys()
        .map(String::as_str)
        .filter(|job| *job != GATE)
        .collect();
    assert_eq!(
        waited,
        others,
        "`{GATE}` must wait for every other job in {WORKFLOW}; it is missing \
         {:?} and waits for {:?} that are not jobs",
        others.difference(&waited).collect::<Vec<_>>(),
        waited.difference(&others).collect::<Vec<_>>()
    );
}
