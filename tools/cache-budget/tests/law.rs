//! The budget law put to populations it can fail on, with no repository and no
//! network — `conclude` takes both sides as arguments precisely so this file can
//! supply them.

use std::collections::BTreeSet;

use cache_budget::{conclude, Held, Refusal, Run};
use ci_plan::CacheDeclaration;

/// A run that started after every cache in these populations was created, so
/// every one of them reads as RESTORED. The budget tests pass `None` instead:
/// they are about arithmetic, not about which run built what.
fn run_after(created: &str, invalidated: &[&str]) -> Run {
    Run {
        started_at: created.to_string(),
        inputs_changed: invalidated.iter().map(|key| key.to_string()).collect(),
    }
}

const GB: u64 = 1_000_000_000;
const LIMIT: u64 = 10 * GB;

const TARGET: &[&str] = &["~/.cargo/registry", "target"];
const REGISTRY: &[&str] = &["~/.cargo/registry"];

fn declaration(owner: &str, prefix: &str, paths: &[&str]) -> CacheDeclaration {
    CacheDeclaration {
        source: ".github/workflows/w.yml".to_string(),
        owner: owner.to_string(),
        key: format!("{prefix}${{{{ hashFiles('**/Cargo.lock') }}}}"),
        prefix: prefix.to_string(),
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
    let declared = [
        declaration("validate", "Linux-cargo-", TARGET),
        declaration(
            "unrun",
            "Linux-cargo-unrun-",
            &["~/.cargo/registry", "target", "tools/unrun-tests/target"],
        ),
    ];
    let held = [held("Linux-cargo-abc", 9.0)];
    let report = conclude(LIMIT, &declared, &held, None);

    let priced = report
        .rows
        .iter()
        .find(|row| row.prefix == "Linux-cargo-unrun-")
        .expect("the absent row");
    let estimate = priced.estimate.as_ref().expect("it is priced");
    assert_eq!(estimate.bytes, 9 * GB);
    assert_eq!(
        estimate.from, "Linux-cargo-abc",
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
    let declared = [declaration("unrun", "Linux-cargo-unrun-", TARGET)];
    let rebuilt = [held_on(
        "Linux-cargo-unrun-abc",
        3.0,
        "2026-08-09T02:30:00.000000000Z",
    )];
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
    let rebuilt = [held_on(
        "Linux-cargo-unrun-abc",
        3.0,
        "2026-08-09T02:30:00.000000000Z",
    )];
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
            "Linux-cargo-side-abc",
            0.06,
            "2026-08-09T02:30:00.000000000Z",
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
        started_at: "2026-08-09T02:00:00Z".to_string(),
        inputs_changed: BTreeSet::new(),
    };

    let after = [held_on(
        "Linux-cargo-unrun-abc",
        3.0,
        "2026-08-09T02:00:01.229538000Z",
    )];
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
        started_at: "2026-08-09T02:00:00.100000000Z".to_string(),
        inputs_changed: BTreeSet::new(),
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

#[test]
fn a_cache_no_workflow_declares_is_a_refusal() {
    // A key outlives the job that wrote it and keeps its share of the budget.
    // Only asking both sides finds it: the workflows do not mention it and the
    // API cannot know it is unwanted.
    let declared = [declaration("validate", "Linux-cargo-", TARGET)];
    let held = [
        held("Linux-cargo-abc", 4.0),
        held("Linux-deleted-job-abc", 3.0),
    ];
    let report = conclude(LIMIT, &declared, &held, None);
    match report.refusals().as_slice() {
        [Refusal::Orphan { key, size_in_bytes }] => {
            assert_eq!(key, "Linux-deleted-job-abc");
            assert_eq!(*size_in_bytes, 3 * GB);
        }
        other => panic!("expected one orphan, got {other:?}"),
    }
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
