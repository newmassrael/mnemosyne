//! The budget law put to populations it can fail on, with no repository and no
//! network — `conclude` takes both sides as arguments precisely so this file can
//! supply them.

use std::collections::{BTreeMap, BTreeSet};

use cache_budget::{range_start, Held, Owner, RangeStart, Refusal, Report, Run, Unheard};
use ci_plan::CacheDeclaration;

/// `conclude` over a run where NOTHING measured what it started from.
///
/// Said once here rather than at each call below, because it is the state every
/// law in this file is about: what STORAGE holds, judged on its own. The laws
/// that need the other instrument — what a job's DISK received — are at the
/// bottom of the file and call the real function with a population.
fn conclude(limit: u64, declared: &[CacheDeclaration], held: &[Held], run: Option<&Run>) -> Report {
    judging(limit, declared, held, run, &BTreeMap::new())
}

/// The same, with what each job's disk said — and with the fixture's horizon.
///
/// EVERY FIXTURE HERE IS WRITTEN IN ONE WORKFLOW AND THAT WORKFLOW COLLECTS, so
/// an owner that left no record in these tests is a job that was silent rather
/// than one this gate could never hear. The laws about the other cases build
/// their own populations, because that is the whole of what they are about.
fn judging(
    limit: u64,
    declared: &[CacheDeclaration],
    held: &[Held],
    run: Option<&Run>,
    started: &BTreeMap<restored::Restore, restored::Warmth>,
) -> Report {
    cache_budget::conclude(limit, declared, held, run, started, &collecting())
}

/// The workflow every fixture below is declared in.
const WORKFLOW: &str = ".github/workflows/w.yml";

/// The workflows that upload an artifact at all — this gate's horizon, as
/// `ci-plan` reads it off the files.
fn collecting() -> BTreeSet<String> {
    [WORKFLOW.to_string()].into_iter().collect()
}

/// A run that started after every cache in these populations was created, so
/// every one of them reads as RESTORED. The budget tests pass `None` instead:
/// they are about arithmetic, not about which run built what.
fn run_after(created: &str, invalidated: &[&str]) -> Run {
    Run {
        workflow: WORKFLOW.to_string(),
        started_at: created.to_string(),
        inputs_changed: invalidated.iter().map(|key| key.to_string()).collect(),
        range: PUSH_RANGE,
    }
}

/// The range these fixtures were judged over. Which one it is changes nothing
/// about the budget arithmetic below — it is carried so that a verdict always
/// says which question it answered.
const PUSH_RANGE: RangeStart = RangeStart::ParentOfHead("a fixture, judged over no repository");

const GB: u64 = 1_000_000_000;
const LIMIT: u64 = 10 * GB;

const TARGET: &[&str] = &["~/.cargo/registry", "target"];
const REGISTRY: &[&str] = &["~/.cargo/registry"];

fn declaration(owner: &str, prefix: &str, paths: &[&str]) -> CacheDeclaration {
    CacheDeclaration {
        source: WORKFLOW.to_string(),
        owner: owner.to_string(),
        // WHERE IN ITS JOB THE CACHE STEP SITS is what lets a reader put a
        // measurement on one side of it or the other. This gate asks nothing
        // about order — `tools/twice-compiled` owns that law — so any position
        // does here.
        index: 1,
        key: format!("{prefix}${{{{ hashFiles('**/Cargo.lock') }}}}"),
        prefix: prefix.to_string(),
        restore_keys: vec![prefix.to_string()],
        paths: paths.iter().map(|path| path.to_string()).collect(),
        hashed: vec!["**/Cargo.lock".to_string()],
    }
}

fn held(key: &str, gb: f64) -> Held {
    held_on(key, gb, "2026-08-08T17:13:25.229538000Z")
}

fn held_on(key: &str, gb: f64, created_at: &str) -> Held {
    Held {
        key: key.to_string(),
        size_in_bytes: (gb * GB as f64) as u64,
        created_at: created_at.to_string(),
    }
}

#[test]
fn a_repository_that_keeps_every_cache_it_declares_passes() {
    let declared = [
        declaration("validate", "Linux-cargo-", TARGET),
        declaration("side", "Linux-cargo-side-", REGISTRY),
    ];
    let held = [
        held("Linux-cargo-abc", 4.0),
        held("Linux-cargo-side-def", 0.06),
    ];
    let report = conclude(LIMIT, &declared, &held, None);
    assert_eq!(report.refusals(), Vec::new());
    assert_eq!(report.absent(), Vec::<String>::new());
}

#[test]
fn caches_that_cannot_all_exist_are_refused_although_the_total_never_exceeds_the_limit() {
    // THE TEST THIS GATE EXISTS FOR, and the one separating it from the gate it
    // would otherwise have been. GitHub deletes caches until the total is under
    // the limit, so the ACTIVE total is bounded by the limit no matter how far
    // over the repository is. Here six jobs each want a whole `target`, two are
    // present at 4 GB and four have been evicted, and the active total is 8 GB —
    // comfortably "under" a 10 GB budget. A gate summing what is present reports
    // this repository as healthy forever while every job in it rebuilds from
    // nothing.
    //
    // Absence is what eviction cannot hide, so absence is what is priced.
    let declared: Vec<CacheDeclaration> = (0..6)
        .map(|n| declaration(&format!("job{n}"), &format!("Linux-cargo-{n}-"), TARGET))
        .collect();
    let held = [
        held("Linux-cargo-0-abc", 4.0),
        held("Linux-cargo-1-abc", 4.0),
    ];

    let report = conclude(LIMIT, &declared, &held, None);
    let present: u64 = held.iter().map(|cache| cache.size_in_bytes).sum();
    assert!(
        present <= LIMIT,
        "the premise of this test: what is PRESENT is inside the budget \
         ({present} <= {LIMIT}) — a gate that summed only this would pass"
    );
    assert_eq!(
        report.demand(),
        Some(24 * GB),
        "and what is DECLARED is not"
    );

    match report.refusals().as_slice() {
        [Refusal::OverBudget { demand, absent, .. }] => {
            assert_eq!(*demand, 24 * GB);
            assert_eq!(
                absent.len(),
                4,
                "and it names which ones are gone: {absent:?}"
            );
        }
        other => panic!("expected one over-budget refusal, got {other:?}"),
    }
}

#[test]
fn an_absent_cache_is_priced_from_the_largest_one_holding_a_subset_of_its_paths() {
    // A cache holding a `target` AND a tool's `target` costs at least what a
    // cache holding that `target` costs. That is a lower bound with a proof
    // rather than a guess, and it is why pricing is not restricted to an
    // identical path list: no two caches in this repository hold quite the same
    // set, so identity would leave every one of them unpriceable.
    //
    // TWO PREFIXES NEITHER OF WHICH NESTS IN THE OTHER, deliberately: this
    // fixture used to spell them `Linux-cargo-` and `Linux-cargo-unrun-`, which
    // is the shape R1123's fallback law refuses — the second holds a path the
    // first never asked for, so the first's `restore-keys` would land it. A
    // fixture declaring the defect a neighbouring law refuses tests both laws by
    // accident and neither on purpose, which is the trap R1104 and R1105 each
    // fell into once.
    let declared = [
        declaration("validate", "Linux-basic-", TARGET),
        declaration(
            "unrun",
            "Linux-unrun-",
            &["~/.cargo/registry", "target", "tools/unrun-tests/target"],
        ),
    ];
    let held = [held("Linux-basic-abc", 9.0)];
    let report = conclude(LIMIT, &declared, &held, None);

    let priced = report
        .rows
        .iter()
        .find(|row| row.prefix == "Linux-unrun-")
        .expect("the absent row");
    let estimate = priced.estimate.as_ref().expect("it is priced");
    assert_eq!(estimate.bytes, 9 * GB);
    assert_eq!(
        estimate.from, "Linux-basic-abc",
        "AND IT NAMES WHERE THE NUMBER CAME FROM — an estimate nobody can trace \
         to a measurement is one nobody can argue with"
    );
    assert!(matches!(
        report.refusals().as_slice(),
        [Refusal::OverBudget { .. }]
    ));
}

#[test]
fn a_missing_build_tree_priced_off_a_registry_cache_does_not_pass() {
    // THE CASE A LOWER BOUND IS NOT ALLOWED TO ANSWER, and it is this
    // repository's own numbers: R1090 measured its registry cache at 0.10 GB and
    // the archive of a built `target` at 3.06 GB, a factor of thirty. So a
    // `target` cache that has been evicted, priced from the registry cache that
    // survived it, reads as 0.10 GB — and the gate would report 0.20 GB of a 10
    // GB budget and pass, on a repository where the job that takes half an hour
    // just lost its cache. That is the exact failure this gate was built for.
    let declared = [
        declaration("validate", "Linux-cargo-", TARGET),
        declaration("msrv", "Linux-cargo-msrv-", REGISTRY),
    ];
    let evicted = conclude(
        LIMIT,
        &declared,
        &[held("Linux-cargo-msrv-abc", 0.10)],
        None,
    );
    assert!(
        evicted.demand().is_some_and(|demand| demand < LIMIT),
        "the premise: the arithmetic says this repository is well inside its \
         budget — {:?}",
        evicted.demand()
    );
    match evicted.refusals().as_slice() {
        [Refusal::Unreached(why)] => assert!(
            why.contains("target"),
            "and it names the path nobody has been observed holding: {why}"
        ),
        other => panic!("expected the lower bound to be refused, got {other:?}"),
    }

    // THE CONTROL, and it is the same repository one cache later: with the build
    // tree present at what this project measured it to be, every row is a reading
    // rather than a bound, and the gate passes.
    let kept = conclude(
        LIMIT,
        &declared,
        &[
            held("Linux-cargo-msrv-abc", 0.10),
            held("Linux-cargo-abc", 3.06),
        ],
        None,
    );
    assert_eq!(kept.refusals(), Vec::new());
    assert_eq!(kept.absent(), Vec::<String>::new());
}

#[test]
fn a_cache_this_run_built_from_nothing_is_a_job_that_was_never_warm() {
    // THE HOLE THE BUDGET LAW LEAVES, and the one Round 1089 had to reach by
    // hand: it read three runs of job durations and concluded `unrun-tests` was
    // never once warm. Every one of those runs ENDED with that cache present, so
    // a gate asking only "is it held" says the repository is fine while the job
    // rebuilds from nothing every time and takes twenty-eight minutes.
    //
    // `actions/cache` saves ONLY when it did not find an exact hit, so a cache
    // created inside this run is that job saying it restored nothing.
    // AND AN ARCHIVE IT COULD HAVE HIT, WHICH R1135 HAD TO PUT IN. This refusal
    // says a cache was available and wasted; with nothing under the prefix
    // predating the run, a cold build was unavoidable and the sentence is not
    // true — which is how it turned main red on a key whose inputs had moved TWO
    // PUSHES earlier, in a run that failed before it could save. The fixture
    // modelled that state while meaning this one.
    let declared = [declaration("unrun", "Linux-cargo-unrun-", TARGET)];
    let rebuilt = [
        held_on(
            "Linux-cargo-unrun-abc",
            3.0,
            "2026-08-09T02:30:00.000000000Z",
        ),
        held_on(
            "Linux-cargo-unrun-was",
            3.0,
            "2026-08-09T01:00:00.000000000Z",
        ),
    ];
    let run = run_after("2026-08-09T02:00:00.000000000Z", &[]);

    let budget_only = conclude(LIMIT, &declared, &rebuilt, None);
    assert_eq!(
        budget_only.refusals(),
        Vec::new(),
        "the premise: judged on the budget alone this repository is spotless — \
         one cache, 3 GB, well inside 10"
    );

    match conclude(LIMIT, &declared, &rebuilt, Some(&run))
        .refusals()
        .as_slice()
    {
        [Refusal::Recreated { prefix, owners, .. }] => {
            assert_eq!(prefix, "Linux-cargo-unrun-");
            assert_eq!(owners.len(), 1, "and it names who paid for it: {owners:?}");
        }
        other => panic!("expected the cold build to be refused, got {other:?}"),
    }
}

#[test]
fn a_cache_built_because_this_commit_moved_what_the_key_hashes_is_not_refused() {
    // THE CONTROL, and it is not a loophole: a dependency change invalidates the
    // key by design, and exactly one cold run is the honest price of it. Refusing
    // that would make every lockfile bump a red build, and a gate that is red for
    // a correct commit is one somebody switches off.
    //
    // The exception is DERIVED, not declared: the key says it hashes
    // `**/Cargo.lock`, git says whether anything matching that moved in this
    // commit, and this function is only told the answer.
    let declared = [declaration("unrun", "Linux-cargo-unrun-", TARGET)];
    let rebuilt = [
        held_on(
            "Linux-cargo-unrun-abc",
            3.0,
            "2026-08-09T02:30:00.000000000Z",
        ),
        held_on(
            "Linux-cargo-unrun-was",
            3.0,
            "2026-08-09T01:00:00.000000000Z",
        ),
    ];
    let bumped = run_after("2026-08-09T02:00:00.000000000Z", &["Linux-cargo-unrun-"]);
    assert_eq!(
        conclude(LIMIT, &declared, &rebuilt, Some(&bumped)).refusals(),
        Vec::new()
    );

    // And the excuse belongs to the key whose inputs moved, not to its
    // neighbours: this repository's side-workspace key hashes a different pair of
    // globs from every other key in the file.
    let two = [
        declaration("unrun", "Linux-cargo-unrun-", TARGET),
        declaration("side", "Linux-cargo-side-", REGISTRY),
    ];
    let both_rebuilt = [
        held_on(
            "Linux-cargo-unrun-abc",
            3.0,
            "2026-08-09T02:30:00.000000000Z",
        ),
        held_on(
            "Linux-cargo-unrun-was",
            3.0,
            "2026-08-09T01:00:00.000000000Z",
        ),
        held_on(
            "Linux-cargo-side-abc",
            0.06,
            "2026-08-09T02:30:00.000000000Z",
        ),
        held_on(
            "Linux-cargo-side-was",
            0.06,
            "2026-08-09T01:00:00.000000000Z",
        ),
    ];
    match conclude(LIMIT, &two, &both_rebuilt, Some(&bumped))
        .refusals()
        .as_slice()
    {
        [Refusal::Recreated { prefix, .. }] => assert_eq!(prefix, "Linux-cargo-side-"),
        other => panic!("expected only the unexcused key to be refused, got {other:?}"),
    }
}

#[test]
fn the_two_endpoints_spell_a_timestamp_differently_and_are_still_compared_correctly() {
    // MEASURED, NOT ASSUMED, and it is the reason these are not compared as plain
    // strings. GitHub's runs endpoint answers `2026-08-08T22:17:13Z` and its
    // caches endpoint answers `2026-08-08T17:13:25.229538000Z` — both real,
    // copied from this repository's own API responses. Compared byte by byte they
    // order on what follows the seconds, where `.` sits below `Z`, so a cache
    // built a minute INTO a run would read as older than the run and the whole
    // law would quietly never fire.
    let declared = [declaration("unrun", "Linux-cargo-unrun-", TARGET)];
    let run = Run {
        workflow: WORKFLOW.to_string(),
        started_at: "2026-08-09T02:00:00Z".to_string(),
        inputs_changed: BTreeSet::new(),
        range: PUSH_RANGE,
    };

    let after = [
        held_on(
            "Linux-cargo-unrun-abc",
            3.0,
            "2026-08-09T02:00:01.229538000Z",
        ),
        // The generation this refusal is about wasting — R1135.
        held_on(
            "Linux-cargo-unrun-was",
            3.0,
            "2026-08-09T01:00:00.000000000Z",
        ),
    ];
    assert!(
        matches!(
            conclude(LIMIT, &declared, &after, Some(&run))
                .refusals()
                .as_slice(),
            [Refusal::Recreated { .. }]
        ),
        "one second after the run started, in the spelling the caches endpoint \
         uses, is INSIDE the run"
    );

    let before = [held_on(
        "Linux-cargo-unrun-abc",
        3.0,
        "2026-08-09T01:59:59.229538000Z",
    )];
    assert_eq!(
        conclude(LIMIT, &declared, &before, Some(&run)).refusals(),
        Vec::new(),
        "and one second before it is outside"
    );

    // AND SUB-SECOND ORDER DOES NOT DECIDE WHETHER A JOB REBUILT. Today the two
    // spellings differ, and a plain string comparison of them lands on the right
    // answer only by an accident of ASCII — `.` sorts below `Z`, so a
    // nanosecond-stamped cache always reads as older than a whole-second run. The
    // day GitHub gives the runs endpoint fractional seconds, that accident stops
    // holding and this comparison starts deciding a half-hour verdict on
    // four hundred milliseconds. A job does not finish and save a cache inside its
    // run's opening second, so the tie is what the law means.
    let fractional_start = Run {
        workflow: WORKFLOW.to_string(),
        started_at: "2026-08-09T02:00:00.100000000Z".to_string(),
        inputs_changed: BTreeSet::new(),
        range: PUSH_RANGE,
    };
    let same_second = [held_on(
        "Linux-cargo-unrun-abc",
        3.0,
        "2026-08-09T02:00:00.500000000Z",
    )];
    assert_eq!(
        conclude(LIMIT, &declared, &same_second, Some(&fractional_start)).refusals(),
        Vec::new(),
        "four hundred milliseconds is not a cold build"
    );
}

#[test]
fn a_cache_older_than_this_run_is_one_the_run_restored() {
    // The other side of the same clock comparison, and the reason it is a
    // comparison rather than a flag: a cache GitHub already held when this run
    // started is one no job in this run had to build.
    let declared = [declaration("unrun", "Linux-cargo-unrun-", TARGET)];
    let kept = [held_on(
        "Linux-cargo-unrun-abc",
        3.0,
        "2026-08-08T09:00:00.000000000Z",
    )];
    let run = run_after("2026-08-09T02:00:00.000000000Z", &[]);
    assert_eq!(
        conclude(LIMIT, &declared, &kept, Some(&run)).refusals(),
        Vec::new()
    );
}

#[test]
fn without_a_run_the_gate_says_so_rather_than_judging_one() {
    // Run on a developer's machine there is no run to be inside, and a gate that
    // invented one would read every cache in the repository as freshly built and
    // refuse the lot. The budget half still answers; the other half is absent
    // from the verdict and the binary prints that it was.
    let declared = [declaration("unrun", "Linux-cargo-unrun-", TARGET)];
    let rebuilt = [held_on(
        "Linux-cargo-unrun-abc",
        3.0,
        "2026-08-09T02:30:00.000000000Z",
    )];
    let report = conclude(LIMIT, &declared, &rebuilt, None);
    assert_eq!(report.run, None);
    assert_eq!(report.refusals(), Vec::new());
}

#[test]
fn a_registry_only_cache_is_never_priced_as_a_whole_target() {
    // The control for the rule above, and the reason it is a SUBSET rather than
    // an overlap: every cache in this repository holds `~/.cargo/registry`, so
    // pricing from anything sharing a path would price a 66 MB registry cache as
    // a 10 GB build tree and invent demand that is not there — a gate turning
    // red at a repository doing nothing wrong.
    let declared = [
        declaration("unrun", "Linux-cargo-unrun-", TARGET),
        declaration("side", "Linux-cargo-side-", REGISTRY),
    ];
    let held = [held("Linux-cargo-unrun-abc", 9.0)];
    let report = conclude(LIMIT, &declared, &held, None);
    let side = report
        .rows
        .iter()
        .find(|row| row.prefix == "Linux-cargo-side-")
        .expect("the registry-only row");
    assert_eq!(
        side.estimate, None,
        "nothing holding a subset of a registry-only cache has been seen, so its \
         cost is UNKNOWN — and unknown is refused, not rounded up to 9 GB"
    );
    assert!(
        matches!(report.refusals().as_slice(), [Refusal::Unreached(_)]),
        "{:?}",
        report.refusals()
    );
}

#[test]
fn a_prefix_still_matches_after_a_lockfile_bump() {
    // Every key in this repository ends in a hash of the lockfiles, so a
    // dependency bump changes all of them at once. A gate joining on the whole
    // key would call every cache in the repository absent the day after one.
    let declared = [declaration("validate", "Linux-cargo-", TARGET)];
    let held = [held("Linux-cargo-a-brand-new-lockfile-hash", 4.0)];
    let report = conclude(LIMIT, &declared, &held, None);
    assert_eq!(report.absent(), Vec::<String>::new());
    assert_eq!(report.refusals(), Vec::new());
}

/// R1123 — A KEY NOTHING DECLARES IS A COST, AND THE VERDICT IS THE SUM.
///
/// It used to be a refusal of its own, one per key, on the reasoning that such a
/// key "keeps its share of the budget". The harm named there is the BUDGET, and
/// the budget is arithmetic — so these bytes belong INSIDE it. What the
/// categorical refusal cost is measured on this repository: every rename orphans
/// its own archive for the seven days it takes to age out, so the gate refused a
/// tree for making a repair, and R1122 had to pin a real defect it could not
/// close because the only repair for it is a rename.
#[test]
fn a_cache_no_workflow_declares_is_counted_and_not_refused_on_its_own() {
    let declared = [declaration("validate", "Linux-cargo-", TARGET)];
    let held = [
        held("Linux-cargo-abc", 4.0),
        held("Linux-deleted-job-abc", 3.0),
    ];
    let report = conclude(LIMIT, &declared, &held, None);
    assert_eq!(report.held_by_nothing(), 3 * GB);
    assert_eq!(
        report.demand(),
        Some(4 * GB),
        "the declared demand is what this repository ASKS for, and the orphan is \
         not one of its declarations"
    );
    assert_eq!(
        report.refusals(),
        Vec::new(),
        "4 + 3 fits in 10, so nothing here breaks — and only asking both sides \
         could have said so either way"
    );
    // AND IT IS STILL SAID OUT LOUD, key by key and in the total, because a cost
    // nobody can see is one nobody can act on.
    let printed = cache_budget::render(&report);
    assert!(
        printed.contains("Linux-deleted-job-abc") && printed.contains("declared by nothing"),
        "{printed}"
    );
    assert!(
        printed.contains("4.00 GB declared + 3.00 GB declared by nothing = 7.00 GB"),
        "{printed}"
    );
}

/// THE OTHER DIRECTION, AND IT IS THE ONE THE OLD SHAPE COULD NOT REACH: a
/// repository whose declarations fit only because the archives nothing declares
/// were left out of the sum. Under a law that refused each orphan separately
/// this was TWO verdicts, one of them the wrong one — the budget read as healthy.
#[test]
fn declarations_that_fit_only_without_the_orphans_are_over_budget() {
    let declared = [declaration("validate", "Linux-cargo-", TARGET)];
    let held = [
        held("Linux-cargo-abc", 4.0),
        held("Linux-deleted-job-abc", 8.0),
    ];
    let report = conclude(LIMIT, &declared, &held, None);
    assert!(
        report.demand().is_some_and(|demand| demand <= LIMIT),
        "the premise: what this repository DECLARES is inside the budget"
    );
    match report.refusals().as_slice() {
        [Refusal::OverBudget {
            demand, orphaned, ..
        }] => {
            assert_eq!((*demand, *orphaned), (4 * GB, 8 * GB));
            let said = report.refusals()[0].to_string();
            assert!(
                said.contains("8.00 GB held under keys no workflow declares"),
                "the message names which half of the total is nobody's: {said}"
            );
        }
        other => panic!("expected one over-budget refusal, got {other:?}"),
    }
}

/// AND A RENAME IS EXPRESSIBLE NOW, which is the whole point: the old archive is
/// held, nothing declares it, and the repository is not red for having repaired
/// itself. This is R1122's pinned defect, played forward.
#[test]
fn a_renamed_key_leaves_an_archive_that_does_not_turn_the_tree_red() {
    let declared = [
        declaration("validate", "Linux-cargo-validate-", REGISTRY),
        declaration("unrun", "Linux-cargo-unrun-", &["target"]),
    ];
    let held = [
        // What the rename left behind, under the key `validate` used to use.
        held("Linux-cargo-a-lockfile-hash", 0.15),
        held("Linux-cargo-validate-a-lockfile-hash", 0.15),
        held("Linux-cargo-unrun-a-lockfile-hash", 8.9),
    ];
    let report = conclude(LIMIT, &declared, &held, None);
    assert_eq!(report.held_by_nothing(), (0.15 * GB as f64) as u64);
    assert_eq!(report.refusals(), Vec::new());
    // NON-VACUITY: the old key really is unmatched, rather than being swallowed
    // by the new one's prefix.
    assert_eq!(
        report
            .orphans
            .iter()
            .map(|orphan| orphan.key.as_str())
            .collect::<Vec<_>>(),
        vec!["Linux-cargo-a-lockfile-hash"]
    );
}

#[test]
fn a_gate_that_measured_nothing_does_not_pass() {
    // The house rule, in the one place it decides an exit code: a gate that could
    // not look and a gate that looked and found nothing print the same silence.
    // Declarations with no observation anywhere are UNKNOWN, not acceptable.
    let declared = [declaration("validate", "Linux-cargo-", TARGET)];
    let report = conclude(LIMIT, &declared, &[], None);
    assert_eq!(report.demand(), None);
    assert!(matches!(
        report.refusals().as_slice(),
        [Refusal::Unreached(_)]
    ));
}

#[test]
fn a_repository_declaring_no_cache_at_all_is_unreached_not_clean() {
    // The other end of the same rule, and the one a parser bug produces: a reader
    // that stopped recognising `actions/cache` hands this function an empty list,
    // and an empty list satisfies "every declared cache is present" vacuously.
    let report = conclude(LIMIT, &[], &[held("Linux-cargo-abc", 4.0)], None);
    assert!(matches!(
        report.refusals().as_slice(),
        [Refusal::Unreached(_)]
    ));
}

#[test]
fn a_cache_belongs_to_the_most_specific_key_that_claims_it() {
    // THIS REPOSITORY'S OWN SHAPE, and the reason a plain `starts_with` join is
    // wrong: the oldest key here is `Linux-cargo-`, which is a prefix of
    // `Linux-cargo-unrun-` and of every other key in the file. Assigned to every
    // key it matches, the small general job takes credit for the huge specific
    // job's cache, the specific one looks satisfied, and the gate reports no
    // absences and a third of the real demand — a green verdict built entirely
    // out of double counting.
    let declared = [
        declaration("validate", "Linux-cargo-", TARGET),
        declaration("unrun", "Linux-cargo-unrun-", TARGET),
    ];
    let held = [held("Linux-cargo-unrun-abc", 9.0)];
    let report = conclude(LIMIT, &declared, &held, None);

    let row = |prefix: &str| {
        report
            .rows
            .iter()
            .find(|row| row.prefix == prefix)
            .expect("the row")
            .clone()
    };
    assert_eq!(
        row("Linux-cargo-unrun-").held.map(|h| h.size_in_bytes),
        Some(9 * GB),
        "the specific key owns it"
    );
    assert!(
        row("Linux-cargo-").held.is_none(),
        "and the general one is correctly reported ABSENT — which is the finding"
    );
    assert!(report.orphans.is_empty(), "it is claimed, just not by both");
    assert_eq!(report.demand(), Some(18 * GB), "two targets, not one");
}

#[test]
fn the_newest_generation_under_one_key_is_the_one_judged() {
    // Two lockfile generations can be alive under one prefix at once, and the
    // BIGGER one is usually the older one — which is exactly the case a gate must
    // not refuse. A repository that shrank its caches yesterday would be read at
    // the size it had the day before, and stay red for the seven days the old
    // generation takes to age out: the repair reported as the failure. What the
    // workflows declare is one generation of each key, and that is what is judged.
    let declared = [declaration("validate", "Linux-cargo-", TARGET)];
    let held = [
        held_on("Linux-cargo-old", 9.0, "2026-08-01T00:00:00.000000000Z"),
        held_on("Linux-cargo-new", 4.0, "2026-08-08T00:00:00.000000000Z"),
    ];
    let report = conclude(LIMIT, &declared, &held, None);
    assert_eq!(
        report.rows[0].held.as_ref().map(|cache| cache.key.as_str()),
        Some("Linux-cargo-new")
    );
    assert_eq!(report.demand(), Some(4 * GB));
    assert_eq!(
        report.rows[0]
            .superseded
            .iter()
            .map(|cache| cache.key.as_str())
            .collect::<Vec<_>>(),
        vec!["Linux-cargo-old"],
        "and the one it replaced is REPORTED, because it is bytes GitHub really \
         is holding — judged is not the same as unseen"
    );
    assert!(
        report.orphans.is_empty(),
        "both belong to the one declaration"
    );
    assert_eq!(report.refusals(), Vec::new());
}

#[test]
fn the_order_a_generation_arrives_in_does_not_decide_which_one_is_judged() {
    // The control for the rule above: `gh` returns caches in whatever order the
    // API paginates them, so a reader keeping "the last one seen" would answer
    // differently for the same repository depending on the response.
    let declared = [declaration("validate", "Linux-cargo-", TARGET)];
    let newest = held_on("Linux-cargo-new", 4.0, "2026-08-08T00:00:00.000000000Z");
    let oldest = held_on("Linux-cargo-old", 9.0, "2026-08-01T00:00:00.000000000Z");
    let forwards = conclude(LIMIT, &declared, &[newest.clone(), oldest.clone()], None);
    let backwards = conclude(LIMIT, &declared, &[oldest, newest], None);
    assert_eq!(forwards, backwards);
    assert_eq!(forwards.demand(), Some(4 * GB));
}

#[test]
fn two_jobs_naming_one_key_cost_the_budget_once() {
    // THE KEY IS THE CACHE. Two jobs sharing a key share one entry in GitHub's
    // storage, which is what sharing a key is for; counting per declaration would
    // price it twice and refuse a repository for reading one cache from two
    // places.
    let declared = [
        declaration("writer", "Linux-cargo-shared-", TARGET),
        declaration("reader", "Linux-cargo-shared-", TARGET),
    ];
    let held = [held("Linux-cargo-shared-abc", 6.0)];
    let report = conclude(LIMIT, &declared, &held, None);
    assert_eq!(report.rows.len(), 1, "one key, one row: {:?}", report.rows);
    assert_eq!(report.rows[0].owners.len(), 2, "and both jobs are named");
    assert_eq!(report.demand(), Some(6 * GB));
    assert_eq!(report.refusals(), Vec::new());
}

#[test]
fn two_jobs_disagreeing_about_what_one_key_holds_are_refused() {
    // The hazard the rule above opens, and nothing else in this repository can
    // see it: GitHub stores one archive per key, so what the cache holds depends
    // on which job saved it first, and every job restoring the other spelling
    // silently gets a tree it did not ask for. The workflow reads as though both
    // were true.
    let declared = [
        declaration("writer", "Linux-cargo-shared-", TARGET),
        declaration("reader", "Linux-cargo-shared-", REGISTRY),
    ];
    let held = [held("Linux-cargo-shared-abc", 6.0)];
    let report = conclude(LIMIT, &declared, &held, None);
    match report.refusals().as_slice() {
        [Refusal::Divergent { prefix, owners }] => {
            assert_eq!(prefix, "Linux-cargo-shared-");
            assert_eq!(owners.len(), 2, "{owners:?}");
        }
        other => panic!("expected one divergent-key refusal, got {other:?}"),
    }
    assert_eq!(
        report.rows[0].paths.len(),
        2,
        "and the key is priced at the UNION, which is the loud direction: {:?}",
        report.rows[0].paths
    );
}

// --- the range the invalidation question is asked over -----------------------

/// Every commit named below is present in the checkout, which is the case a
/// full-history clone gives.
fn everything_is_here(_sha: &str) -> bool {
    true
}

#[test]
fn the_question_is_asked_over_the_whole_push_and_not_over_its_tip() {
    // THE DEFECT THIS PAIR EXISTS FOR, and it is not hypothetical: two commits
    // went up in one push, the workflow moved in the FIRST of them, and the gate
    // asked `git diff HEAD~1 HEAD`. The tip had touched no hashed input, so
    // eight jobs that had legitimately rebuilt from nothing were reported as a
    // defect and main went red — the gate refusing for a reason outside its own
    // law, which is the same failure as a gate that does not fire.
    let before = "df48a8ddf661fa490e4f6c3a3994787847492452";
    let start = range_start(Some(before), everything_is_here);
    assert_eq!(start, RangeStart::Push(before.to_string()));
    assert_eq!(
        start.rev(),
        before,
        "the diff runs from where the push began"
    );
    assert!(
        start.why().contains("this push carried"),
        "the verdict must name the range it answered over: {}",
        start.why()
    );
}

#[test]
fn a_commit_the_checkout_does_not_hold_narrows_the_range_and_says_so() {
    // A SHALLOW CLONE IS THE SILENT VERSION OF THE SAME DEFECT. The runner names
    // the commit the push started from whether or not the checkout fetched it,
    // and diffing from a revision that is not here makes git fail — so the gate
    // would refuse to judge a repository that is fine. It narrows instead, and
    // the narrowing is printed, because a number from a different question is
    // worse than no number.
    let start = range_start(Some("df48a8ddf661fa490e4f6c3a3994787847492452"), |_| false);
    assert!(matches!(start, RangeStart::ParentOfHead(_)));
    assert_eq!(start.rev(), "HEAD~1");
    assert!(
        start.why().contains("too shallow"),
        "the reason must be the checkout, not the event: {}",
        start.why()
    );
}

#[test]
fn a_push_that_created_the_branch_started_from_nothing() {
    // GitHub spells "there was no previous tip" as all zeros. There is no
    // earlier commit to ask about, so the narrow range is the whole of what
    // there is to see — and it is a decision with a reason rather than a diff
    // against a revision that does not exist.
    let start = range_start(Some("0000000000000000000000000000000000000000"), |_| {
        panic!("all zeros is not a commit to look for")
    });
    assert!(matches!(start, RangeStart::ParentOfHead(_)));
    assert!(
        start.why().contains("created the branch"),
        "{}",
        start.why()
    );
}

#[test]
fn a_pull_request_falls_back_to_the_parent_of_head_which_is_its_base() {
    // NOT A WEAKNESS HERE, A PROPERTY: on `pull_request` the runner checks out a
    // merge commit whose FIRST PARENT is the base branch, so `HEAD~1..HEAD` is
    // exactly the change the pull request proposes. The environment carries no
    // push range there, and this is the case that reaches the fallback in normal
    // operation.
    for absent in [None, Some(""), Some("   ")] {
        let start = range_start(absent, |_| panic!("nothing to look for"));
        assert!(matches!(start, RangeStart::ParentOfHead(_)), "{absent:?}");
        assert_eq!(start.rev(), "HEAD~1");
        assert!(start.why().contains("not a push event"), "{}", start.why());
    }
}

// --- what the job's disk received -------------------------------------------
//
// THE OTHER INSTRUMENT. Everything above judges what STORAGE holds, and that is
// the only thing this gate could see until Round 1101: whether an archive
// exists, when it was created, and — from the fact that `actions/cache` saves
// only on a miss — whether the primary key hit. None of that is what a job
// RECEIVED. `restore-keys` can serve an earlier generation to a job whose key
// missed, and Round 1099 read exactly that state as a cold build and deleted a
// cache that was saving ten minutes. The verdicts below are the join.

/// What a job's disk said after ONE of its restores.
///
/// R1122 — BY JOB AND CACHE, because a record is one cache's: a job declaring
/// two writes two of these and a fixture keyed by the job could only ever hand
/// the gate one of them.
fn started(
    measured: &[(&str, &str, restored::Warmth)],
) -> BTreeMap<restored::Restore, restored::Warmth> {
    measured
        .iter()
        .map(|(job, cache, warmth)| {
            (
                restored::Restore {
                    job: (*job).to_string(),
                    cache: (*cache).to_string(),
                },
                *warmth,
            )
        })
        .collect()
}

/// One key, one owner, one archive — the shape both laws below are about.
fn one_key(created_at: &str) -> (Vec<CacheDeclaration>, Vec<Held>) {
    (
        vec![declaration("unrun", "Linux-cargo-unrun-", TARGET)],
        vec![held_on("Linux-cargo-unrun-abc", 3.0, created_at)],
    )
}

/// The same key with an ARCHIVE UNDER IT FROM BEFORE THE RUN.
///
/// R1135, AND THE FIXTURES THAT NEEDED IT WERE SAYING TWO THINGS AT ONCE: a
/// measured `PrefixHit` of 7.4 GB is a job restoring an archive, and the storage
/// these cases described held none. `Recreated` names a cache that was AVAILABLE
/// and wasted, so the available one has to be in the fixture; the state without
/// it is a different answer and has its own case now.
fn one_key_over_a_generation(created_at: &str) -> (Vec<CacheDeclaration>, Vec<Held>) {
    (
        vec![declaration("unrun", "Linux-cargo-unrun-", TARGET)],
        vec![
            held_on("Linux-cargo-unrun-abc", 3.0, created_at),
            held_on("Linux-cargo-unrun-was", 3.0, BEFORE_THE_RUN),
        ],
    )
}

const BEFORE_THE_RUN: &str = "2026-08-08T10:00:00.000000000Z";
const RUN_STARTED: &str = "2026-08-08T12:00:00Z";
const DURING_THE_RUN: &str = "2026-08-08T12:30:00.000000000Z";

/// The run these fixtures are judged inside — of the workflow they declare their
/// caches in, so an owner that says nothing here is a job that was silent.
fn the_run(invalidated: &[&str]) -> Run {
    Run {
        workflow: WORKFLOW.to_string(),
        started_at: RUN_STARTED.to_string(),
        inputs_changed: invalidated.iter().map(|key| key.to_string()).collect(),
        range: PUSH_RANGE,
    }
}

/// THE SENTENCE THAT WAS FALSE. A missed key with a warm owner is not a cold
/// build, and this refusal used to say it was.
#[test]
fn a_missed_key_whose_owner_was_warm_stops_claiming_it_paid_for_a_cold_build() {
    // A PREFIX HIT IS AN ARCHIVE BEING RESTORED, so the storage this case
    // describes has to hold one — R1135. Without it the fixture measured a job
    // restoring 7.4 GB from a repository that held nothing.
    let (declared, held) = one_key_over_a_generation(DURING_THE_RUN);
    let run = the_run(&[]);
    let warm = started(&[(
        "unrun",
        "Linux-cargo-unrun-",
        restored::Warmth::PrefixHit {
            bytes: 7_466_000_000,
        },
    )]);
    let report = judging(LIMIT, &declared, &held, Some(&run), &warm);
    let refusals = report.refusals();
    match refusals.as_slice() {
        [Refusal::Recreated { started, .. }] => {
            assert_eq!(started.len(), 1, "{started:?}");
            let said = refusals[0].to_string();
            // THE ORACLE IS THE MEASUREMENT, NOT A PHRASE. An earlier spelling of
            // this test looked for the words "earlier generation", and the
            // sentence printed when NOTHING was measured contains them too — so
            // an injection that threw the measurement away came back green. What
            // distinguishes the two is the reading itself, which the type can
            // produce and the test therefore does not spell.
            let measured = restored::Warmth::PrefixHit {
                bytes: 7_466_000_000,
            };
            assert!(
                said.contains(&measured.why()),
                "the message says what the disk received: {said}"
            );
            assert!(
                !said.contains("NOT MEASURED"),
                "and does not claim it was unmeasured when it was: {said}"
            );
            assert!(
                !said.contains("cold build"),
                "and no longer asserts what it cannot see: {said}"
            );
        }
        other => panic!("the key still missed, so it is still a finding: {other:?}"),
    }
}

/// And with nothing measured it says so, rather than falling back to the claim.
#[test]
fn a_missed_key_with_no_record_says_what_it_did_not_measure() {
    let (declared, held) = one_key_over_a_generation(DURING_THE_RUN);
    let run = the_run(&[]);
    let report = judging(LIMIT, &declared, &held, Some(&run), &BTreeMap::new());
    let said = report.refusals()[0].to_string();
    assert!(said.contains("NOT MEASURED"), "{said}");
    assert!(!said.contains("cold build"), "{said}");
}

/// THE FINDING NEITHER INSTRUMENT COULD REACH ALONE: an empty disk while an
/// archive `restore-keys` could have served was already in storage.
#[test]
fn a_job_that_restored_nothing_with_a_generation_held_is_refused() {
    let (declared, held) = one_key(BEFORE_THE_RUN);
    let run = the_run(&[]);
    let cold = started(&[("unrun", "Linux-cargo-unrun-", restored::Warmth::Nothing)]);
    let report = judging(LIMIT, &declared, &held, Some(&run), &cold);
    match report.refusals().as_slice() {
        [Refusal::RestoredNothingWithAGenerationHeld {
            job,
            prefix,
            generation,
        }] => {
            assert_eq!(job, "unrun");
            assert_eq!(prefix, "Linux-cargo-unrun-");
            assert_eq!(generation.key, "Linux-cargo-unrun-abc");
        }
        other => panic!("{other:?}"),
    }
}

/// A KEY THAT MISSED WITH NOTHING UNDER IT TO HIT IS NOT A WASTED CACHE.
///
/// THIS IS THE RED R1135 PAID OFF, and it is the one case where this gate's
/// excuse cannot reach the explanation. `Linux-cargo-side-` had its key moved by
/// a lockfile bump TWO PUSHES back; the run that would have saved the new
/// archive FAILED, and `actions/cache` does not save from a failed job; no older
/// generation survived. So the next run compiled from an empty tree and was told
/// its rebuild was unexplained — while the range this gate diffs for an excuse is
/// THIS push, which cannot hold a change made before it.
///
/// WHAT IS NOT LOST BY DROPPING THE REFUSAL: the report still prints the cold
/// state per owner, and the one actionable cause of a whole prefix being missing
/// — eviction — is refused by the budget law itself, which is where a repository
/// asking for more than 10 GB is caught. Unexcused on purpose: the point is that
/// the verdict no longer depends on an excuse this gate cannot see.
#[test]
fn a_key_that_missed_with_no_archive_under_it_is_not_refused_as_a_wasted_cache() {
    let (declared, held) = one_key(DURING_THE_RUN);
    let run = the_run(&[]);
    let cold = started(&[("unrun", "Linux-cargo-unrun-", restored::Warmth::Nothing)]);
    let report = judging(LIMIT, &declared, &held, Some(&run), &cold);
    assert_eq!(
        report.refusals(),
        Vec::new(),
        "with no generation predating the run there was nothing to restore, so \
         neither refusal has a claim to make"
    );
    // AND THE READING IS STILL PRINTED, which is what keeps this from being a
    // hole: dropping a refusal must not drop the evidence.
    let said = cache_budget::render(&report);
    assert!(
        said.contains(&restored::Warmth::Nothing.why()),
        "the report still says the job compiled from an empty tree:\n{said}"
    );

    // THE CONTROL, in the same case: put the archive back under the prefix and
    // the refusal returns. One `held_on` is the whole difference.
    let (declared, over_a_generation) = one_key_over_a_generation(DURING_THE_RUN);
    let refused = judging(LIMIT, &declared, &over_a_generation, Some(&run), &cold);
    assert!(
        !refused.refusals().is_empty(),
        "an archive that WAS there and was not restored is a finding"
    );
}

/// THE CONTROL, and it is the state a NEW key is in on its first run: nothing
/// was restored because there was nothing to restore.
#[test]
fn a_job_that_restored_nothing_with_nothing_to_restore_is_not_refused_for_it() {
    let (declared, held) = one_key(DURING_THE_RUN);
    // Excused as a key, so the only verdict left available is the new one.
    let run = the_run(&["Linux-cargo-unrun-"]);
    let cold = started(&[("unrun", "Linux-cargo-unrun-", restored::Warmth::Nothing)]);
    let report = judging(LIMIT, &declared, &held, Some(&run), &cold);
    assert_eq!(
        report.refusals(),
        Vec::new(),
        "the only archive under this prefix is the one THIS run saved, so there \
         was nothing for `restore-keys` to fall back to"
    );
}

/// The other control: a warm job is not refused for an empty disk it did not
/// have. Without this the law above would pass for a gate that refused
/// everything.
#[test]
fn a_job_that_was_warm_is_not_refused_for_starting_from_nothing() {
    let (declared, held) = one_key(BEFORE_THE_RUN);
    let run = the_run(&[]);
    for warmth in [
        restored::Warmth::ExactHit { bytes: 1 },
        restored::Warmth::PrefixHit { bytes: 1 },
    ] {
        let report = judging(
            LIMIT,
            &declared,
            &held,
            Some(&run),
            &started(&[("unrun", "Linux-cargo-unrun-", warmth)]),
        );
        assert_eq!(report.refusals(), Vec::new(), "{warmth:?}");
    }
}

/// A SUPERSEDED GENERATION IS STILL ONE `restore-keys` COULD HAVE SERVED, and
/// the newest archive is routinely the one this run just saved.
#[test]
fn the_generation_that_counts_is_the_one_that_predates_the_run() {
    let declared = [declaration("unrun", "Linux-cargo-unrun-", TARGET)];
    let held = [
        held_on("Linux-cargo-unrun-new", 3.0, DURING_THE_RUN),
        held_on("Linux-cargo-unrun-old", 2.0, BEFORE_THE_RUN),
    ];
    let run = the_run(&["Linux-cargo-unrun-"]);
    let report = judging(
        LIMIT,
        &declared,
        &held,
        Some(&run),
        &started(&[("unrun", "Linux-cargo-unrun-", restored::Warmth::Nothing)]),
    );
    match report.refusals().as_slice() {
        [Refusal::RestoredNothingWithAGenerationHeld { generation, .. }] => assert_eq!(
            generation.key, "Linux-cargo-unrun-old",
            "the one this run saved is not one it could have restored"
        ),
        other => panic!("{other:?}"),
    }
}

/// An owner is a join key and not a rendering.
#[test]
fn a_rows_owner_carries_the_job_id_the_records_are_filed_under() {
    let declared = [declaration("unrun", "Linux-cargo-unrun-", TARGET)];
    let report = conclude(
        LIMIT,
        &declared,
        &[held("Linux-cargo-unrun-abc", 3.0)],
        None,
    );
    assert_eq!(
        report.rows[0].owners,
        vec![Owner {
            source: ".github/workflows/w.yml".to_string(),
            job: "unrun".to_string(),
        }]
    );
    assert_eq!(
        report.rows[0].owners[0].to_string(),
        ".github/workflows/w.yml `unrun`",
        "and renders as it always did, so the two are one datum and one view"
    );
}

/// THE ENTRANCE'S ONE DECISION THAT CAN BE ASKED A QUESTION.
///
/// A flag whose value is read as the tree to judge is a gate pointed at a
/// directory with no workflow in it, refusing for a reason that has nothing to
/// do with any cache — and the shape of the mistake is invisible in a workflow
/// file, where `-- --restored rustc-log` looks exactly like what was meant.
#[test]
fn a_flags_value_is_not_the_tree_to_judge() {
    let words = |list: &[&str]| {
        list.iter()
            .map(|word| (*word).to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        cache_budget::read_arguments(&words(&["--restored", "rustc-log"])),
        (std::path::PathBuf::from("."), Some("rustc-log".to_string())),
        "the tree defaults to here, and `rustc-log` is the records"
    );
    assert_eq!(
        cache_budget::read_arguments(&words(&["/some/tree", "--restored", "logs"])),
        (
            std::path::PathBuf::from("/some/tree"),
            Some("logs".to_string())
        )
    );
    assert_eq!(
        cache_budget::read_arguments(&words(&["/some/tree"])),
        (std::path::PathBuf::from("/some/tree"), None),
        "and without the flag the other half is NOT MEASURED rather than empty"
    );
}

// --- what the report says ----------------------------------------------------
//
// THE WORDS ARE A DECISION AND A DECISION NEEDS A READER. The rendering used to
// live in `main.rs`, where the only thing a suite could ask of it was an exit
// code — R1096's lesson, paid for once already in this repository.

/// The state where the two instruments both speak: a key GitHub holds, and a
/// record from the job that declares it.
fn report_with_both_instruments() -> Report {
    let declared = [declaration("unrun-tests", "Linux-cargo-unrun-", TARGET)];
    let caches = [held("Linux-cargo-unrun-abc", 7.83)];
    let started = started(&[(
        "unrun-tests",
        "Linux-cargo-unrun-",
        restored::Warmth::PrefixHit {
            bytes: 27_258_000_000,
        },
    )]);
    judging(LIMIT, &declared, &caches, None, &started)
}

#[test]
fn the_report_says_the_stored_archive_and_the_restored_disk_are_not_one_quantity() {
    // RUN 31307111606 IS WHY. Its report printed `7.83 GB held` and, on the very
    // next line, `27258 MB restored` for the same key — the compressed archive
    // against the tree it expands into, a factor of three and a half. The line
    // below it read 0.15 GB against 246 MB. Both numbers are right and neither
    // says which quantity it is, one line apart, which is where a reader divides.
    let printed = cache_budget::render(&report_with_both_instruments());
    assert!(
        printed.contains("7.83 GB held"),
        "the premise: the archive size is printed\n{printed}"
    );
    assert!(
        printed.contains("27258 MB restored"),
        "the premise: the disk figure is printed too, and it is the larger\n{printed}"
    );
    assert!(
        printed.contains("the archive GitHub stores")
            && printed.contains("what arrived on its disk"),
        "and the report says the two are not the same quantity\n{printed}"
    );
}

#[test]
fn a_report_with_no_record_to_compare_against_does_not_explain_a_comparison() {
    // THE CONTROL, and it is the reason the sentence is conditional. On a run
    // that read no restore record there is no second quantity, and a line about
    // a comparison nobody can make is what teaches a reader to skip lines.
    let declared = [declaration("unrun-tests", "Linux-cargo-unrun-", TARGET)];
    let caches = [held("Linux-cargo-unrun-abc", 7.83)];
    let printed = cache_budget::render(&judging(LIMIT, &declared, &caches, None, &BTreeMap::new()));
    assert!(printed.contains("7.83 GB held"), "{printed}");
    assert!(
        printed.contains(&Unheard::NoRun.to_string()),
        "the absence is still said out loud\n{printed}"
    );
    assert!(
        !printed.contains("the archive GitHub stores"),
        "and nothing explains a comparison this report cannot make\n{printed}"
    );
}

#[test]
fn the_report_counts_the_steps_and_the_held_caches_it_was_reckoned_against() {
    // A REPORT THAT CANNOT SAY HOW FAR IT REACHED prints the same header whether
    // it read eight cache steps or none. The two counts are not `rows.len()`:
    // several steps may share one key, and held caches include the superseded
    // generations and the ones no workflow declares.
    let declared = [
        declaration("validate", "Linux-cargo-", TARGET),
        declaration("side", "Linux-cargo-", TARGET),
        declaration("msrv", "Linux-cargo-msrv-", REGISTRY),
    ];
    let caches = [
        held("Linux-cargo-abc", 4.0),
        held_on("Linux-cargo-old", 4.0, "2026-08-01T00:00:00.000000000Z"),
    ];
    let report = judging(LIMIT, &declared, &caches, None, &BTreeMap::new());
    assert_eq!((report.declared_steps, report.held_caches), (3, 2));
    assert_eq!(report.rows.len(), 2, "three steps under two keys");
    assert!(
        cache_budget::render(&report).contains("3 cache step(s)")
            && cache_budget::render(&report).contains("under 2 key(s), 2 held by GitHub"),
        "{}",
        cache_budget::render(&report)
    );
}

// --- whose silence it is -----------------------------------------------------
//
// R1107. A record is an artifact and an artifact belongs to A RUN, so a cache
// declared in another workflow has an owner this gate cannot ever be handed a
// record for. It printed that as `did not say what it started from` — the
// sentence for a job that could have been heard and was not — and a reader
// acting on it would go and wire a measurement into a workflow that uploads
// nothing, which is precisely the repair R1106 established must NOT be made.
//
// The populations below are the ones a report is built from, so each law is a
// verdict of `Report::unheard` AND the line `render` prints for it: the enum is
// the decision and the sentence is what anybody actually reads.

/// A second workflow, whose caches this repository's other gates also read.
const OTHER_WORKFLOW: &str = ".github/workflows/other.yml";

/// One key declared in `where_`, held, with nothing measured about its owner.
fn silent_owner(where_: &str) -> (Vec<CacheDeclaration>, Vec<Held>) {
    let mut only = declaration("replay", "Linux-cargo-replay-", REGISTRY);
    only.source = where_.to_string();
    (vec![only], vec![held("Linux-cargo-replay-abc", 0.15)])
}

#[test]
fn a_job_that_could_have_been_heard_and_was_not_is_the_one_that_said_nothing() {
    // THE CONTROL FOR ALL THREE BELOW. Without it every law here would pass for a
    // gate that had simply stopped accusing anybody, and the accusation is right
    // exactly once: a job inside this run's own workflow, which collects, and
    // which left no record anyway. That is a gap in the repository.
    let (declared, held) = silent_owner(WORKFLOW);
    let run = the_run(&[]);
    let report = judging(LIMIT, &declared, &held, Some(&run), &BTreeMap::new());
    let owner = &report.rows[0].owners[0];
    assert_eq!(report.unheard(owner), Unheard::ItsOwnSilence);
    assert!(
        cache_budget::render(&report).contains("`replay` did not say what it started from"),
        "{}",
        cache_budget::render(&report)
    );
}

#[test]
fn a_job_whose_workflow_collects_nothing_is_not_one_that_said_nothing() {
    // THE STATE THIS REPOSITORY IS ACTUALLY IN, and the sentence that was wrong
    // for two rounds. `evidence-replay.yml` declares a cache and uploads no
    // artifact, so whatever its job writes is destroyed with its runner — not
    // withheld, unreadable, and unreadable from ANYWHERE rather than merely from
    // inside a `mnemosyne-validate` run.
    let (declared, held) = silent_owner(OTHER_WORKFLOW);
    let run = the_run(&[]);
    let report = cache_budget::conclude(
        LIMIT,
        &declared,
        &held,
        Some(&run),
        &BTreeMap::new(),
        // The other workflow is not in it: nothing there uploads.
        &collecting(),
    );
    let owner = &report.rows[0].owners[0];
    assert_eq!(
        report.unheard(owner),
        Unheard::NothingCollectsIt {
            workflow: OTHER_WORKFLOW.to_string()
        }
    );
    let printed = cache_budget::render(&report);
    assert!(
        !printed.contains("did not say what it started from"),
        "and the job is not named as deficient for this gate's horizon\n{printed}"
    );
    assert!(
        printed.contains(OTHER_WORKFLOW) && printed.contains("destroyed with its runner"),
        "the reason names the file it was read off\n{printed}"
    );
}

#[test]
fn a_job_in_another_workflow_that_does_collect_is_out_of_this_runs_reach() {
    // THE OTHER HALF, and the reason the two are separate verdicts: this one says
    // a cross-run reader COULD be built, and the one above says it could not.
    // Collapsing them would either invite the repair R1106 refused or forbid one
    // that is merely absent.
    let (declared, held) = silent_owner(OTHER_WORKFLOW);
    let run = the_run(&[]);
    let both: BTreeSet<String> = [WORKFLOW.to_string(), OTHER_WORKFLOW.to_string()]
        .into_iter()
        .collect();
    let report =
        cache_budget::conclude(LIMIT, &declared, &held, Some(&run), &BTreeMap::new(), &both);
    let owner = &report.rows[0].owners[0];
    assert_eq!(
        report.unheard(owner),
        Unheard::AnotherWorkflow {
            workflow: OTHER_WORKFLOW.to_string()
        }
    );
    let printed = cache_budget::render(&report);
    assert!(
        !printed.contains("did not say what it started from"),
        "{printed}"
    );
    assert!(
        printed.contains(&format!("run of {WORKFLOW} started")),
        "and the report names what `here` is, once, where the run is described\n{printed}"
    );
}

#[test]
fn the_reason_that_holds_everywhere_is_the_one_printed() {
    // ORDER IS A DECISION. An owner in another workflow that also collects
    // nothing is both, and the two sentences are not equally useful: one says
    // "build a cross-run reader and you will hear it", the other says "there is
    // nothing to hear, ever". The far-reaching one wins, because acting on the
    // weaker one is the wasted repair.
    let (declared, held) = silent_owner(OTHER_WORKFLOW);
    let run = the_run(&[]);
    let report = cache_budget::conclude(
        LIMIT,
        &declared,
        &held,
        Some(&run),
        &BTreeMap::new(),
        &collecting(),
    );
    let owner = &report.rows[0].owners[0];
    assert!(
        matches!(report.unheard(owner), Unheard::NothingCollectsIt { .. }),
        "both are true of it; the one that holds for every reader is said: {:?}",
        report.unheard(owner)
    );
}

#[test]
fn a_record_in_hand_contradicts_a_reading_that_says_nothing_collects_it() {
    // THE ONE ANCHOR OUTSIDE THE READING IT CHECKS. Every sentence above rests on
    // which workflows collect, and a reader that answered "none" would explain
    // all eight owners' silence with a reason nobody wrote — a report entirely
    // self-consistent and entirely wrong, which is the class of defect this whole
    // repair is about. A record actually in hand cannot be argued with.
    let (declared, held) = silent_owner(OTHER_WORKFLOW);
    let heard = started(&[(
        "replay",
        "Linux-cargo-replay-",
        restored::Warmth::PrefixHit { bytes: 246_000_000 },
    )]);
    let report = cache_budget::conclude(LIMIT, &declared, &held, None, &heard, &collecting());
    match report.refusals().as_slice() {
        [Refusal::Unreached(why)] => {
            assert!(why.contains("replay"), "{why}");
            assert!(why.contains("uploads no artifact at all"), "{why}");
        }
        other => panic!("a record was read for an owner this gate reckons unhearable: {other:?}"),
    }
    // And the control: the same record where the reading agrees with it.
    let (mut declared, held) = silent_owner(OTHER_WORKFLOW);
    declared[0].source = WORKFLOW.to_string();
    assert_eq!(
        judging(LIMIT, &declared, &held, None, &heard).refusals(),
        Vec::new()
    );
}

// WHICH WORKFLOW THIS RUN IS OF is `ci-plan`'s reading and so are its laws —
// `tools/ci-plan/tests/reading.rs`. The census gate cut the same
// `owner/repo/<path>@<ref>` for itself until R1107, and two cuts of one string
// are two answers free to disagree about which file a gate is judging.

/// R1123 — THE HARMLESS NESTING, WHICH IS WHAT `restore-keys` IS FOR. A cache
/// whose prefix is a prefix of another's falls back onto that one's archives,
/// and when the inner cache holds only what the outer job asked for, the outer
/// job gets exactly the paths it declared out of somebody else's generation.
/// This repository ran on that for five keys until R1123 renamed the prefix they
/// all nested under, so the branch has no live example left and is asserted here.
#[test]
fn a_nesting_whose_inner_cache_holds_a_subset_is_not_a_finding() {
    let declared = [
        declaration("validate", "Linux-cargo-", REGISTRY),
        declaration("citations", "Linux-cargo-citations-", REGISTRY),
    ];
    let held = [
        held("Linux-cargo-abc", 0.15),
        held("Linux-cargo-citations-abc", 0.06),
    ];
    assert_eq!(
        conclude(LIMIT, &declared, &held, None).refusals(),
        Vec::new(),
        "the inner cache holds exactly what the outer job declares, so falling \
         back onto its archive is the mechanism working"
    );
}

/// AND THE ONE PATH THAT MAKES IT A FINDING. The same nesting, with the inner
/// cache holding a build directory the outer job never declared: `path:` says
/// what a cache SAVES and cannot stop an archive unpacking as it was stored.
#[test]
fn a_nesting_whose_inner_cache_holds_more_is_refused() {
    let declared = [
        declaration("validate", "Linux-cargo-", REGISTRY),
        declaration(
            "unrun",
            "Linux-cargo-unrun-",
            &["~/.cargo/registry", "target"],
        ),
    ];
    let held = [
        held("Linux-cargo-abc", 0.15),
        held("Linux-cargo-unrun-abc", 8.9),
    ];
    match conclude(LIMIT, &declared, &held, None)
        .refusals()
        .as_slice()
    {
        [Refusal::FallbackReachesAnotherCache {
            prefix,
            other,
            holds,
        }] => {
            assert_eq!(prefix, "Linux-cargo-");
            assert_eq!(other, "Linux-cargo-unrun-");
            assert_eq!(
                holds,
                &vec!["target".to_string()],
                "and it names WHAT would land, not merely that something would"
            );
        }
        other => panic!("expected one fallback refusal, got {other:?}"),
    }
}

/// THE DIRECTION IT DOES NOT HOLD IN. A prefix is not symmetric: the inner cache
/// falls back onto nothing of the outer one's, because its own key is longer.
#[test]
fn the_inner_cache_of_a_nesting_reaches_nothing_of_the_outer_ones() {
    // THE TWO HOLD DISJOINT PATHS, deliberately. With one holding a subset of the
    // other the subset test alone answers both directions, so a reader that had
    // lost the direction would agree with this fixture — green on two different
    // readers, which is no control at all.
    let declared = [
        declaration("validate", "Linux-cargo-", REGISTRY),
        declaration("unrun", "Linux-cargo-unrun-", &["target"]),
    ];
    let refusals = conclude(LIMIT, &declared, &[held("Linux-cargo-abc", 0.15)], None).refusals();
    let named: Vec<&String> = refusals
        .iter()
        .filter_map(|refusal| match refusal {
            Refusal::FallbackReachesAnotherCache { prefix, .. } => Some(prefix),
            _ => None,
        })
        .collect();
    assert_eq!(
        named,
        vec!["Linux-cargo-"],
        "only the OUTER key reaches, and a reader comparing the pair without \
         direction would report this twice"
    );
}
