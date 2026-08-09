//! The law against THIS repository's own workflow, which is where the gate has
//! to be non-vacuous rather than merely correct.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use twice_compiled::{declared_jobs, judge, Census, JobLog, Refusal, Unit};

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

/// A census in which every declared job compiled something, so that the only
/// thing left for [`judge`] to refuse is the WIRING. Separating the two is what
/// lets this test be about the workflow rather than about a build.
fn everybody_compiled(declared: &BTreeMap<String, Vec<ci_plan::RunStep>>) -> Census {
    let mut census = Census::default();
    for (index, job) in declared.keys().enumerate() {
        let mut log = JobLog {
            invocations: 1,
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
            1,
        );
        census.jobs.insert(job.clone(), log);
    }
    census
}

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
        declared.len(),
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
        declared.len() > 2,
        "{WORKFLOW} declares {} job(s) with `run:` steps — this gate's whole \
         subject is what TWO of them both compile",
        declared.len()
    );
    assert!(
        declared.contains_key(GATE),
        "the job running this gate is `{GATE}`, and {WORKFLOW} declares {:?}",
        declared.keys().collect::<Vec<_>>()
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
