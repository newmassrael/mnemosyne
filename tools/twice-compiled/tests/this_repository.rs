//! The law against THIS repository's own workflow, which is where the gate has
//! to be non-vacuous rather than merely correct.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use twice_compiled::{judge, Census, Cost, Declared, JobLog, Origin, Refusal, Unit};

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
    Declared::of(
        steps,
        &ci_plan::cache_steps(&document, WORKFLOW),
        &ci_plan::artifact_uploads(&document, WORKFLOW),
    )
}

/// What ONE tracked workflow declares, whichever it is — the same three readers,
/// pointed anywhere.
fn declared_in(path: &str) -> Declared {
    let root = repository_root();
    let document = ci_plan::load_workflow(&root, path);
    Declared::of(
        &ci_plan::run_steps(&document),
        &ci_plan::cache_steps(&document, path),
        &ci_plan::artifact_uploads(&document, path),
    )
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
                driver: "/toolchain/bin/rustc".to_string(),
                origin: twice_compiled::Origin::Tree,
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
        // WHICH CACHE, taken from what this workflow declares rather than
        // spelled again here: a fixture inventing a prefix would be testing that
        // the gate accepts an invented one.
        written.extend_from_slice(&restored::encode_cache(
            declared
                .prefixes
                .get(job)
                .and_then(|declared| declared.first())
                .expect("a job with a cache declares a prefix"),
        ));
        written.extend_from_slice(&restored::encode_at(restored::Side::Before, 1_000_000_000));
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
        written.extend_from_slice(&restored::encode_at(restored::Side::After, 1_030_000_000));
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

#[test]
fn every_cached_job_of_this_workflow_measures_the_restore_around_it() {
    // THE LAW THAT NEEDS NO RUN, held against the live file. What a job started
    // from is the DIFFERENCE between two measurements, so both of them being on
    // one side of the restore is not a smaller reading — it is zero, which is
    // exactly what a job that compiled from an empty tree reports. Round 1099
    // read that shape and deleted a cache that was saving ten minutes a run.
    let declared = declared_jobs(&workflow_steps());
    let refusals = twice_compiled::judge_wiring(&declared);
    assert!(
        refusals.is_empty(),
        "{WORKFLOW}:\n{}",
        refusals
            .iter()
            .map(|refusal| format!("  {refusal}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    // NON-VACUITY, FIRST HALF: the population is not empty. A law over no job
    // returns the same nothing as a law over a file that is right.
    assert!(
        declared.caches_at.len() >= 5,
        "{WORKFLOW} declares caches in {:?} — a wiring law over fewer jobs than \
         this repository has cached jobs is one that stopped reading",
        declared.caches_at.keys().collect::<Vec<_>>()
    );
    // NON-VACUITY, SECOND HALF, AND IT IS THE ONE THAT MATTERS: the law bites on
    // THIS population, not merely on a fixture. Every cached job in the live
    // file is walked, its first measurement moved past its own cache step, and
    // the refusal demanded — so a reader that had quietly stopped comparing
    // indices would turn this red rather than print an empty verdict.
    for job in declared.caches_at.keys().cloned().collect::<Vec<_>>() {
        let mut moved = declared.clone();
        let past = moved.caches_at[&job].iter().max().copied().unwrap_or(0) + 1;
        let steps = moved.jobs.get_mut(&job).expect("a cached job with steps");
        let before = steps
            .iter_mut()
            .find(|step| restored::sides_measured(&step.script).contains(&restored::Side::Before))
            .unwrap_or_else(|| panic!("job `{job}` measures the restore before it happens"));
        before.index = past;
        assert!(
            !twice_compiled::judge_wiring(&moved).is_empty(),
            "moving `{job}`'s first measurement past its cache step is the \
             defect this law exists for, and it was not refused"
        );
    }
}

#[test]
fn every_cargo_home_tree_this_workflow_caches_is_one_the_split_can_name() {
    // WHAT MAKES THE SPLIT ANSWERABLE ABOUT A CACHE. `Origin` reads cargo's own
    // layout, which is a fact about cargo and true in a checkout that caches
    // nothing; what makes "this job compiled 461 crates its cache had fetched"
    // a statement about THIS cache is that the cache carries those trees and no
    // others. A third one added here — `~/.cargo/bin`, a vendor directory —
    // would compile into units the split calls the checkout's, and the fetched
    // share would quietly shrink while every total still added up.
    //
    // ONE SIDE READ OFF THE LIVE FILE, so this goes red when the workflow moves
    // rather than when somebody remembers to come back here.
    let declared = declared_jobs(&workflow_steps());
    let cached: BTreeSet<String> = declared
        .caches
        .values()
        .flatten()
        .filter_map(|path| path.split("/.cargo/").nth(1))
        .map(str::to_string)
        .collect();
    let named: BTreeSet<String> = ["git".to_string(), "registry".to_string()]
        .into_iter()
        .collect();
    assert_eq!(
        cached, named,
        "{WORKFLOW} caches these trees under a cargo home, and `Origin` names \
         those two — teach the split about a third BEFORE caching it, or its \
         compilations are reported as the checkout's"
    );
    // AND THE NAMING IS THE LIVE READING, not a coincidence of spelling: a
    // source in each of the two trees is one this split calls fetched.
    assert!(Origin::of(
        "/home/runner/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/serde-1.0.219/src/lib.rs"
    )
    .fetched());
    assert!(Origin::of(
        "/home/runner/.cargo/git/checkouts/tonic-3a1f6c5d9b2e/9b2ef41/tonic/src/lib.rs"
    )
    .fetched());
}

#[test]
fn this_workflow_caches_one_build_directory_and_not_a_workspaces_own() {
    // ONE `target` FOR THE WHOLE CHECKOUT. `.cargo/config.toml` sets
    // `build.target-dir`, which cargo resolves against the directory holding
    // that file, so all of this repository's workspaces compile into the same
    // place. That is what lets a cache of ONE directory reach the work that
    // used to land in `bench/target` and eleven `tools/*/target` trees — where
    // every one of a run's 4095 fetched compilations was measured to go.
    //
    // A `path:` naming a workspace's own `target` is now a directory nothing
    // writes into: the cache would save an empty tree and the job would read as
    // having had its output held.
    //
    // AND EXACTLY ONE JOB MAY HOLD IT, which is arithmetic and not taste. A
    // cache keyed by a PATH stores one copy per job, this repository's build
    // directory archives to 8.90 GB, and GitHub gives a repository 10. R1112
    // left three jobs each holding it, the demand came to 11.87 GB, and GitHub
    // began evicting least-recently-used — which is the failure whose only
    // symptom is a green job that takes half an hour. The number of copies the
    // budget allows is one, so the law is `== 1`.
    let declared = declared_jobs(&workflow_steps());
    let holders: Vec<&String> = declared
        .caches
        .iter()
        .filter(|(_, paths)| paths.iter().any(|path| path.ends_with("target")))
        .map(|(job, _)| job)
        .collect();
    assert_eq!(
        holders.len(),
        1,
        "{WORKFLOW} has {} job(s) caching a build directory and the budget \
         affords one copy of it: {holders:?}",
        holders.len()
    );
    for path in declared.caches.values().flatten() {
        if path.ends_with("target") {
            assert_eq!(
                path, "target",
                "{WORKFLOW} caches `{path}`, and this checkout compiles nothing \
                 into a workspace's own `target` — see `.cargo/config.toml`"
            );
        }
    }
}

#[test]
fn every_tracked_workflow_wires_its_restore_measurements_or_collects_nothing() {
    // THE LAW REACHED ONE FILE AND THIS REPOSITORY TRACKS TWO. `judge_wiring`
    // needs no census, so nothing about it was ever per-workflow except which
    // file somebody happened to point it at — and the file nobody pointed it at
    // declares a cache. A law whose population is "the file the author
    // remembered" is the shape R777, R783, R1080 and R1082 each closed once.
    let root = repository_root();
    let tracked = ci_plan::workflow_files(&root);
    assert!(
        tracked.len() >= 2,
        "this repository tracks {tracked:?} — a sweep over one file is the \
         population that was already being read"
    );
    let mut cached = 0;
    let mut collecting = 0;
    for path in &tracked {
        let declared = declared_in(path);
        let refusals = twice_compiled::judge_wiring(&declared);
        assert!(
            refusals.is_empty(),
            "{path}:\n{}",
            refusals
                .iter()
                .map(|refusal| format!("  {refusal}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
        cached += declared.caches_at.len();
        collecting += usize::from(declared.collects_records);
    }
    // NON-VACUITY: both sides of the derivation exist in this repository today.
    // If every tracked workflow collected records the exemption would never be
    // exercised, and if none did the law would never bite.
    assert!(
        cached >= 8 && collecting >= 1 && collecting < tracked.len(),
        "{cached} cached job(s) across {} workflow(s), {collecting} of which \
         collect records — this sweep needs both sides to be real",
        tracked.len()
    );
}

#[test]
fn a_workflow_that_collects_nothing_owes_no_restore_record_and_is_refused_for_writing_one() {
    // THE EXEMPTION IS DERIVED FROM THE FILE, and this is what makes it
    // load-bearing rather than a silence. `evidence-replay.yml` declares a
    // cache, measures neither side, and uploads no artifact at all — so the
    // record it would write goes onto a runner that is destroyed when the job
    // ends. Nothing can ever read it.
    let replay = declared_in(".github/workflows/evidence-replay.yml");
    assert!(
        !replay.collects_records,
        "the premise: that workflow uploads nothing"
    );
    assert!(
        !replay.caches_at.is_empty(),
        "and it does declare a cache, which is why the exemption is needed at all"
    );
    assert!(twice_compiled::judge_wiring(&replay).is_empty());

    // AND THE EXEMPTION BITES THE MOMENT THE FILE CHANGES. Told that the same
    // workflow collects records, the same jobs are refused for measuring
    // neither side — so this is a derivation and not a hole.
    let collecting = Declared {
        collects_records: true,
        ..replay.clone()
    };
    assert!(
        !twice_compiled::judge_wiring(&collecting).is_empty(),
        "a workflow that collects records owes one per cached job"
    );

    // THE OTHER DIRECTION, which nothing else in this repository can say: a job
    // measuring a restore where no artifact is uploaded writes a file nobody
    // can read.
    let mut measuring = replay.clone();
    let job = measuring
        .caches_at
        .keys()
        .next()
        .expect("the cached job")
        .clone();
    let mut step = ci_plan::RunStep {
        job: job.clone(),
        index: 0,
        script: format!(
            "./tools/{p}/target/release/{p} before 'target'",
            p = restored::PROGRAM
        ),
        env: std::collections::BTreeMap::new(),
    };
    step.env.insert(
        restored::VARIABLE.to_string(),
        format!("rustc-log/{job}.restored"),
    );
    measuring.jobs.entry(job.clone()).or_default().push(step);
    assert_eq!(
        twice_compiled::judge_wiring(&measuring),
        vec![Refusal::RestoreIsMeasuredWhereNothingCollectsIt { job }]
    );
}
