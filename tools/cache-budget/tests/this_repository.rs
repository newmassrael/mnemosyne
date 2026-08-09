//! The law against THIS repository's own workflows, which is where the gate has
//! to be non-vacuous rather than merely correct.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use cache_budget::{
    conclude, render, Held, RangeStart, Refusal, Run, Unheard, DEFAULT_LIMIT_BYTES,
};
use ci_plan::CacheDeclaration;

/// The job that runs this gate.
const GATE: &str = "cache-budget";

/// The workflow that runs this gate.
const WORKFLOW: &str = ".github/workflows/mnemosyne-validate.yml";

/// The root `target` directory, which is the path six jobs asked to keep a copy
/// of and the reason this gate exists.
const ROOT_TARGET: &str = "target";

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("this crate lives two directories below the repository root")
        .to_path_buf()
}

fn gibibytes(gib: f64) -> u64 {
    (gib * 1024.0 * 1024.0 * 1024.0) as u64
}

/// What GitHub was holding when R1089 measured this repository, key by key.
///
/// PINNED, because it is a measurement and not a fact about today's tree: these
/// three were the only caches alive, and two of them were each bigger than the
/// whole 10 GB budget the eight declared keys were sharing.
fn measured_by_r1089() -> Vec<Held> {
    let held = |key: &str, gib: f64| Held {
        key: key.to_string(),
        size_in_bytes: gibibytes(gib),
        created_at: "2026-08-08T00:00:00.000000000Z".to_string(),
    };
    vec![
        held("Linux-cargo-unrun-0f0f0f", 10.71),
        held("Linux-cargo-0f0f0f", 9.58),
        held("Linux-cargo-side-0f0f0f", 0.06436),
    ]
}

fn declaration(owner: &str, prefix: &str, paths: &[&str]) -> CacheDeclaration {
    CacheDeclaration {
        source: ".github/workflows/mnemosyne-validate.yml".to_string(),
        owner: owner.to_string(),
        // This gate asks what a cache HOLDS, never where its step sits; the law
        // about the order is `tools/twice-compiled`'s.
        index: 1,
        key: format!("{prefix}${{{{ hashFiles('**/Cargo.lock') }}}}"),
        prefix: prefix.to_string(),
        paths: paths.iter().map(|path| path.to_string()).collect(),
        hashed: vec!["**/Cargo.lock".to_string()],
    }
}

const REGISTRY: &[&str] = &["~/.cargo/registry", "~/.cargo/git"];
const REGISTRY_AND_TARGET: &[&str] = &["~/.cargo/registry", "~/.cargo/git", "target"];

/// Every cache key this repository declared on the commit that measured the
/// thrashing — six of the eight holding a whole root `target`.
///
/// PINNED for the same reason `reading.rs` pins the lister's output: it is the
/// control for a state this tree can no longer produce. A test asserting that
/// today's tree is better than the one this gate was written to repair needs the
/// worse one to still exist somewhere, and the frozen ledger says that somewhere
/// is not the workflow file.
fn declared_before_the_repair() -> Vec<CacheDeclaration> {
    vec![
        declaration("validate", "Linux-cargo-", REGISTRY_AND_TARGET),
        declaration(
            "item-citations",
            "Linux-cargo-citations-",
            &[
                "~/.cargo/registry",
                "~/.cargo/git",
                "target",
                "tools/item-citations/target",
            ],
        ),
        declaration("side-workspaces", "Linux-cargo-side-", REGISTRY),
        declaration(
            "server-features",
            "Linux-cargo-serverfeat-",
            REGISTRY_AND_TARGET,
        ),
        declaration(
            "unrun-tests",
            "Linux-cargo-unrun-",
            &[
                "~/.cargo/registry",
                "~/.cargo/git",
                "target",
                "tools/unrun-tests/target",
            ],
        ),
        declaration(
            "uncompiled-sources",
            "Linux-cargo-uncompiled-",
            &[
                "~/.cargo/registry",
                "~/.cargo/git",
                "target",
                "bench/target",
                "tools/uncompiled-sources/target",
            ],
        ),
        declaration("msrv", "Linux-cargo-msrv-", REGISTRY_AND_TARGET),
        declaration("replay", "Linux-cargo-replay-", REGISTRY),
    ]
}

fn keys_holding_the_root_target(declared: &[CacheDeclaration]) -> BTreeSet<String> {
    declared
        .iter()
        .filter(|declaration| declaration.paths.iter().any(|path| path == ROOT_TARGET))
        .map(|declaration| declaration.prefix.clone())
        .collect()
}

#[test]
fn the_tree_this_gate_was_written_to_repair_is_one_it_refuses() {
    // NON-VACUITY, measured rather than argued. Against the caches R1089 found
    // alive, the eight keys that commit declared come to six times a build tree,
    // and the two that survived were each larger than the entire budget the eight
    // were sharing. A gate that cannot fail on THAT is a gate that cannot fail.
    let before = declared_before_the_repair();
    let report = conclude(
        DEFAULT_LIMIT_BYTES,
        &before,
        &measured_by_r1089(),
        None,
        &BTreeMap::new(),
        &ci_plan::workflows_collecting_artifacts(&repository_root()),
    );
    let demand = report
        .demand()
        .expect("every key is priceable from the three");
    assert!(
        demand > 6 * DEFAULT_LIMIT_BYTES,
        "six whole build trees and two registries: {demand}"
    );
    assert!(
        report
            .refusals()
            .iter()
            .any(|refusal| matches!(refusal, Refusal::OverBudget { .. })),
        "{:?}",
        report.refusals()
    );
    assert_eq!(
        keys_holding_the_root_target(&before).len(),
        6,
        "and it is six keys that hold it, which is the arithmetic no round did \
         while adding the fourth, fifth and sixth"
    );
}

#[test]
fn this_repository_now_asks_for_strictly_less_than_that() {
    // The other half of the same measurement, and the one that goes red the day
    // somebody adds a whole-`target` cache back with a locally correct comment
    // about why sharing a key would evict.
    //
    // COUNTED RATHER THAN PRICED, and R1091 is why. This test used to reckon
    // both trees' demand from the caches Round 1089 measured and require today's
    // to be smaller. That comparison stopped meaning anything the moment a key's
    // declared contents changed: the archive stored under `Linux-cargo-` held a
    // whole build tree, the declaration now says it holds a registry, and pricing
    // one against the other prices a registry cache at ten gigabytes. That
    // mismatch is not a flaw in the arithmetic — it IS the defect this round
    // repaired, which is why the keys now hash the workflow that decides what
    // they hold. What survives it is the thing the repair actually did: fewer
    // keys asking to keep a whole build tree, derived from the real file with no
    // policy number anywhere in it.
    let before = declared_before_the_repair();
    let now = ci_plan::workflow_cache_declarations(&repository_root());

    let holders = keys_holding_the_root_target(&now);
    assert!(
        holders.len() < keys_holding_the_root_target(&before).len(),
        "fewer keys hold a whole build tree than did before — no policy number, \
         just the comparison: {holders:?}"
    );
    // R1100 — AND NOT ZERO EITHER, WHICH IS NOW A NUMBER RATHER THAN A SENTENCE.
    // This assertion stood on prose: that this repository's slowest jobs are the
    // ones a cold `target` costs half an hour each. R1099 removed the last such
    // key believing it had measured that prose false — two runs, identical
    // censuses for `unrun-tests` at 773 compiled / 670 distinct / 103 repeated,
    // read as a cache which skipped nothing.
    //
    // BOTH OF THOSE RUNS WERE WARM. The exact key missed in one of them because
    // the workflow had moved, but `restore-keys` matched the prefix and GitHub
    // served the previous generation — `Cache hit for restore-key:
    // Linux-cargo-unrun-…` in that job's own log. Warm against warm is how a
    // cache that works reads as a cache that does nothing.
    //
    // Run 31296883662 ran with the tree out of the declaration, which is the only
    // genuinely cold reading this repository has of it: 1199 compiled / 1006
    // distinct / 193 repeated. The cache is worth 426 compilations and about ten
    // minutes of that job. So the prose was right, and it is replaced here by the
    // measurement rather than restored as prose.
    assert!(
        !holders.is_empty(),
        "at least one key keeps a whole build tree, and this is what it buys: \
         with `unrun-`'s tree in the declaration that job compiles 773 units, \
         without it 1199 (run 31296883662) — 426 compilations and about ten \
         minutes for 7.83 GB. Removing the last one takes a census showing it \
         skips nothing, taken against a job that was COLD and not one that \
         restored a prefix match: {holders:?}"
    );
}

#[test]
fn the_gate_waits_for_every_job_in_its_own_workflow_that_saves_a_cache() {
    // WHAT THIS GATE JUDGES IS THE STATE A RUN LEAVES BEHIND. `actions/cache`
    // writes in a job's post step, so a gate running beside those jobs reads the
    // caches of the PREVIOUS run and calls today's repair yesterday's failure.
    //
    // `needs:` is a list, and a list restating the tree is the shape this
    // repository has closed four times. So it is not restated: the jobs that
    // declare a cache are READ, and any one missing from the wait is a failure
    // here — loudly, at the moment somebody adds the job, rather than as a wrong
    // verdict nobody can see.
    let root = repository_root();
    let mut workflows_with_the_gate = 0;
    for path in ci_plan::workflow_files(&root) {
        let doc = ci_plan::load_workflow(&root, &path);
        let needs = ci_plan::job_needs(&doc);
        let Some(waits_for) = needs.get(GATE) else {
            continue;
        };
        workflows_with_the_gate += 1;
        let declaring: BTreeSet<String> = ci_plan::cache_steps(&doc, &path)
            .into_iter()
            .map(|declaration| declaration.owner)
            .filter(|owner| owner != GATE)
            .collect();
        assert!(
            declaring.len() > 1,
            "{path}: this workflow is where this repository's caches are \
             declared, so reading fewer than two of them is the reader failing"
        );
        for job in &declaring {
            assert!(
                waits_for.iter().any(|waited| waited == job),
                "{path}: `{GATE}` does not wait for `{job}`, which saves a cache \
                 — it would judge that key before this run wrote it"
            );
        }
    }
    assert_eq!(
        workflows_with_the_gate, 1,
        "exactly one workflow runs this gate — zero means the check above looped \
         over nothing and passed, which is the empty answer that looks clean"
    );
}

#[test]
fn the_gates_own_job_is_given_the_range_the_run_covers_and_the_history_to_read_it() {
    // THE WIRING THAT TURNED MAIN RED BY ITS ABSENCE. This gate excuses a cold
    // build when the key's hashed inputs legitimately moved, and it used to ask
    // that of ONE commit. A push carries a range: two commits went up together,
    // the workflow moved in the first, and the tip had touched no hashed input —
    // so eight jobs that correctly rebuilt from nothing were called a defect.
    //
    // Both halves are asserted because either one alone is silent. Without the
    // variable the gate never sees the range; with it but a shallow checkout,
    // the commit it names is not here and the gate quietly narrows back to the
    // range that failed. `fetch-depth: 0` rather than a number, because a push's
    // commit count is unbounded and every fixed depth is a guess.
    let root = repository_root();
    let raw = std::fs::read_to_string(root.join(WORKFLOW)).expect("the workflow is readable");
    let doc = ci_plan::load_workflow(&root, WORKFLOW);
    let job = &doc["jobs"][GATE];
    assert!(!job.is_badvalue(), "{WORKFLOW} declares no job `{GATE}`");

    assert_eq!(
        job["env"][cache_budget::RANGE_VARIABLE].as_str(),
        Some("${{ github.event.before }}"),
        "`{GATE}` must be told the commit this push started from, in \
         ${}: without it the gate asks about the tip commit alone",
        cache_budget::RANGE_VARIABLE
    );

    let checkout = job["steps"][0]["with"]["fetch-depth"].as_i64();
    assert_eq!(
        checkout,
        Some(0),
        "`{GATE}` checks out {checkout:?} commit(s) of history — the commit a \
         push started from is not in a shallow clone, and the gate would narrow \
         to HEAD~1..HEAD, which is exactly the range that reported eight \
         legitimate cold builds as a defect"
    );

    // The variable is the program's, not a string this test agrees with by
    // coincidence: it must be the one the gate actually reads.
    assert!(
        raw.contains(cache_budget::RANGE_VARIABLE),
        "the workflow names no variable this program reads"
    );
}

#[test]
fn the_gate_takes_no_cache_of_its_own() {
    // Stated as a design property in the job's own header, and therefore a thing
    // to assert rather than to write down: a budget gate that eats the budget is
    // the seventh job.
    let root = repository_root();
    assert!(
        ci_plan::workflow_cache_declarations(&root)
            .iter()
            .all(|declaration| declaration.owner != GATE),
        "the job judging the budget declares a cache of its own"
    );
}

#[test]
fn the_gates_own_job_collects_the_records_it_now_judges_against() {
    // R1101 — BOTH HALVES, AGAIN, BECAUSE EITHER ALONE IS SILENT. Without the
    // download step the directory is empty and every job reads as "did not say";
    // without the flag the gate never looks at it and says the other half was
    // not measured. Both are green-looking states, and the verdict they suppress
    // is the one that would have contradicted Round 1099.
    //
    // Read off the file rather than agreed with by hand: the step that runs this
    // gate is found by the manifest it names, so renaming the job or reordering
    // its steps cannot make this pass by accident.
    let root = repository_root();
    let doc = ci_plan::load_workflow(&root, WORKFLOW);
    let steps = doc["jobs"][GATE]["steps"]
        .as_vec()
        .unwrap_or_else(|| panic!("{WORKFLOW} declares no steps for `{GATE}`"));

    let collecting = steps
        .iter()
        .position(|step| {
            step["uses"]
                .as_str()
                .is_some_and(|uses| uses.starts_with("actions/download-artifact"))
        })
        .unwrap_or_else(|| {
            panic!(
                "`{GATE}` downloads no artifact, so the records every job \
                 uploaded are not in its workspace and what each job's disk \
                 received cannot be read"
            )
        });
    assert_eq!(
        steps[collecting]["with"]["path"].as_str(),
        Some(RECORDS),
        "the records land where the flag below points"
    );

    let running = steps
        .iter()
        .position(|step| {
            step["run"]
                .as_str()
                .is_some_and(|script| script.contains("tools/cache-budget/Cargo.toml"))
        })
        .expect("the step that runs this gate");
    let script = steps[running]["run"].as_str().expect("a script");
    assert!(
        script.contains(&format!("--restored {RECORDS}")),
        "`{GATE}` runs without `--restored {RECORDS}`, so it judges storage \
         alone and reports what each job started from as unmeasured: {script}"
    );
    assert!(
        collecting < running,
        "the records are collected before they are read, not after"
    );
}

#[test]
fn every_cache_owner_in_this_repository_falls_on_one_side_of_this_gates_horizon() {
    // R1107 — THE SWEEP THAT MAKES THE DISTINCTION LOAD-BEARING. This gate joins
    // storage against records, and a record is an artifact of the run that wrote
    // it: the download step in this job collects THIS run's artifacts, whose jobs
    // are this workflow's jobs. `evidence-replay.yml` also declares a cache, so
    // its owner is in this report and its record can never be handed here — and
    // for two rounds the report printed that as `did not say what it started
    // from`, which reads as a job failing to do something it never could.
    //
    // Both populations are asserted alive against the real files. If nothing were
    // beyond the horizon the distinction would be decoration; if nothing were
    // within it the gate would have stopped asking anybody anything, which is the
    // shape an exemption takes when it has swallowed its population.
    let root = repository_root();
    let declared = ci_plan::workflow_cache_declarations(&root);
    let report = conclude(
        DEFAULT_LIMIT_BYTES,
        &declared,
        // No cache list and no records: this law is about who COULD be heard,
        // which is a property of the files and not of what storage holds today.
        &[],
        Some(&Run {
            workflow: WORKFLOW.to_string(),
            started_at: "2026-08-09T00:00:00Z".to_string(),
            inputs_changed: BTreeSet::new(),
            range: RangeStart::ParentOfHead("a fixture, judged over no push"),
        }),
        &BTreeMap::new(),
        &ci_plan::workflows_collecting_artifacts(&root),
    );

    // THE TWO POPULATIONS, SPLIT BY THE FILES AND NOT BY THE FUNCTION UNDER TEST.
    // Which owners exist at all and where they are written is `ci-plan`'s
    // reading; what `unheard` is asked below is whether it puts each of them on
    // the side the files put it on. Deriving the expectation from `unheard`
    // itself would be a test that agrees with anything.
    let collecting = ci_plan::workflows_collecting_artifacts(&root);
    let owners: Vec<_> = report.rows.iter().flat_map(|row| &row.owners).collect();
    let unhearable: Vec<_> = owners
        .iter()
        .filter(|owner| !collecting.contains(&owner.source))
        .collect();
    let within: Vec<_> = owners
        .iter()
        .filter(|owner| collecting.contains(&owner.source) && owner.source == WORKFLOW)
        .collect();
    assert!(
        !unhearable.is_empty(),
        "this repository declares a cache in a workflow that uploads nothing, and \
         that owner is the whole reason this law exists — with none of them the \
         far-reaching reason never fires: {owners:?}"
    );
    assert!(
        within.len() >= 6,
        "and the jobs of this gate's own workflow are the ones whose silence IS \
         theirs — a gate that has stopped asking anybody is what an exemption \
         looks like once it has swallowed its population: {within:?}"
    );

    let printed = render(&report);
    for owner in &unhearable {
        assert_eq!(
            report.unheard(owner),
            Unheard::NothingCollectsIt {
                workflow: owner.source.clone()
            },
            "nothing carries `{}`'s record off its runner, which holds for every \
             reader there will ever be and not merely for this run",
            owner.job
        );
        assert!(
            !printed.contains(&format!("`{}` {}", owner.job, Unheard::ItsOwnSilence)),
            "and it is not named as a job that withheld something\n{printed}"
        );
    }
    // THE CONTROL: the accusation still exists. A gate that had merely stopped
    // making it would pass every assertion above.
    for owner in &within {
        assert_eq!(report.unheard(owner), Unheard::ItsOwnSilence);
        assert!(
            printed.contains(&format!("`{}` {}", owner.job, Unheard::ItsOwnSilence)),
            "`{}` left no record and could have, which is still a gap\n{printed}",
            owner.job
        );
    }

    // AND THE ARM THIS REPOSITORY CANNOT WALK TODAY, asserted as the zero it is.
    // `AnotherWorkflow` needs a second workflow that both declares a cache and
    // uploads an artifact, and there is none — so the arm is exercised in
    // `law.rs` over a built population and nowhere here. Saying the count out
    // loud is the difference between "no case reached it" and "it was never
    // looked for": the day `evidence-replay.yml` starts collecting, this goes red
    // and the reason for its owner legitimately becomes the nearer one.
    assert_eq!(
        owners
            .iter()
            .filter(|owner| matches!(report.unheard(owner), Unheard::AnotherWorkflow { .. }))
            .count(),
        0,
        "no tracked workflow other than {WORKFLOW} both caches and collects"
    );
}

/// Where the records the jobs uploaded are collected — one spelling, asserted on
/// both sides of the step that reads them.
///
/// NOT `rustc-log`, which is this job's OWN record directory and is uploaded as
/// this job's artifact: collecting eight other jobs' records into it would
/// upload all of them again under this job's name.
const RECORDS: &str = "collected";
