//! The reading rules, pinned against strings rather than against the tree.
//!
//! Both sides of every gate built on this crate come from these two parses, and
//! the branches that matter most are the ones THIS MACHINE NEVER TAKES: a `SKIP`
//! line only appears where a sibling checkout is absent, which is a CI runner
//! and not the machine this is written on. R1082's gate had no such branch and
//! turned main red on its first push. Pinned text is how a branch nobody here
//! can execute still has a control.

use std::collections::BTreeSet;

use ci_plan::{
    cache_steps, job_needs, lister_declared_commands, lister_suite_commands, lock_verdict,
    parse_lister, parse_script, parse_workflow, run_steps, CacheDeclaration, CargoCommand,
    IssuedCommands, LockVerdict, Ownership,
};

/// A workflow with two cached jobs, one of them registry-only.
const TWO_CACHES: &str = r#"
jobs:
  validate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - name: Cache cargo
        uses: actions/cache@v6
        with:
          path: |
            ~/.cargo/registry
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
          restore-keys: |
            ${{ runner.os }}-cargo-
      - run: cargo test --workspace
  side:
    runs-on: ubuntu-latest
    needs: validate
    steps:
      - name: Cache cargo (side)
        uses: actions/cache@v6
        with:
          path: |
            ~/.cargo/registry
          key: ${{ runner.os }}-cargo-side-${{ hashFiles('tools/*/Cargo.lock') }}
"#;

fn caches_of(yaml: &str) -> Vec<CacheDeclaration> {
    cache_steps(&ci_plan::parse_workflow(yaml, "w.yml"), "w.yml")
}

#[test]
fn a_cache_key_is_read_down_to_the_prefix_a_restore_matches_on() {
    let found = caches_of(TWO_CACHES);
    assert_eq!(found.len(), 2, "{found:?}");
    // `${{ runner.os }}` resolved from the job's OWN `runs-on`, and everything
    // from the lockfile hash onwards dropped: that hash changes on every
    // dependency bump, so a reader joining on the whole key would call every
    // cache in the repository missing the day after one.
    assert_eq!(found[0].prefix, "Linux-cargo-");
    assert_eq!(found[1].prefix, "Linux-cargo-side-");
    // And the key as written is kept beside it, because that is what a person
    // greps for.
    assert!(found[0].key.contains("hashFiles"), "{:?}", found[0].key);
    assert_eq!(
        found[0].owner, "validate",
        "the job id, as `needs:` spells it"
    );
}

#[test]
fn the_paths_are_read_because_they_are_what_a_cache_costs() {
    let found = caches_of(TWO_CACHES);
    assert_eq!(found[0].paths, vec!["~/.cargo/registry", "target"]);
    assert_eq!(found[1].paths, vec!["~/.cargo/registry"]);
}

#[test]
fn a_key_says_which_files_would_legitimately_invalidate_it() {
    // A cache this run had to build from nothing is a job that paid for a cold
    // build — except when the thing the key hashes actually moved, and then one
    // cold run is the price of a dependency change. That exception is DERIVED
    // from the key rather than assumed, and it is a different question per key:
    // this repository's side-workspace key hashes two globs and none of them is
    // the `**/Cargo.lock` every other key hashes.
    let found = caches_of(TWO_CACHES);
    assert_eq!(found[0].hashed, vec!["**/Cargo.lock"]);
    assert_eq!(found[1].hashed, vec!["tools/*/Cargo.lock"]);

    assert_eq!(
        ci_plan::hashed_globs(
            "${{ runner.os }}-cargo-side-${{ hashFiles('bench/Cargo.lock', 'tools/*/Cargo.lock') }}"
        ),
        vec!["bench/Cargo.lock", "tools/*/Cargo.lock"],
        "several arguments to one call are several globs"
    );
    assert!(
        ci_plan::hashed_globs("${{ runner.os }}-cargo-shared-").is_empty(),
        "a key that hashes nothing can never be excused for having been rebuilt"
    );
    assert!(
        ci_plan::hashed_globs("${{ runner.os }}-${{ hashFiles(env.LOCKS) }}").is_empty(),
        "AND AN ARGUMENT THAT IS NOT A PLAIN LITERAL IS NOT GUESSED AT — reading \
         it wrong would excuse a cold build that nothing justifies, so an \
         unreadable input excuses nothing, which is the strict direction"
    );
}

#[test]
fn a_restore_only_step_is_still_a_declaration() {
    // A job that only restores still depends on that key surviving, which is the
    // whole subject. Reading `actions/cache` alone would count it as asking for
    // nothing.
    let found = caches_of(
        r#"
jobs:
  reader:
    runs-on: ubuntu-latest
    steps:
      - name: Cache target (restore only)
        uses: actions/cache/restore@v6
        with:
          path: target
          key: ${{ runner.os }}-cargo-shared-
"#,
    );
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].prefix, "Linux-cargo-shared-");
}

#[test]
fn a_step_that_is_not_a_cache_is_not_read_as_one() {
    assert!(caches_of(
        r#"
jobs:
  plain:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - run: cargo test
"#,
    )
    .is_empty());
}

#[test]
#[should_panic(expected = "runner.os")]
fn a_runner_this_reader_cannot_name_refuses_instead_of_guessing() {
    // The prefix is the identity every later comparison joins on. Guessing it for
    // an unknown label reports every cache as absent — a finding-shaped wrong
    // answer, which is worse than stopping.
    caches_of(
        r#"
jobs:
  exotic:
    runs-on: freebsd-14
    steps:
      - name: Cache target
        uses: actions/cache@v6
        with:
          path: target
          key: ${{ runner.os }}-cargo-
"#,
    );
}

#[test]
fn a_job_with_no_cache_is_never_asked_for_a_runner_this_reader_must_refuse() {
    // The control for the test above, and it is not symmetry for its own sake:
    // resolving `runner.os` for every job rather than every CACHING job would
    // make one exotic uncached job unreadable for the whole workflow, and the
    // gate would report zero declarations — which reads as a tidy repository.
    let found = caches_of(
        r#"
jobs:
  exotic:
    runs-on: freebsd-14
    steps:
      - run: uname -a
  ordinary:
    runs-on: ubuntu-latest
    steps:
      - name: Cache target
        uses: actions/cache@v6
        with:
          path: target
          key: ${{ runner.os }}-cargo-
"#,
    );
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].prefix, "Linux-cargo-");
    assert_eq!(
        found[0].step, "Cache target",
        "and the step's own name, which is what joins it to the run that wrote \
         its archive (R1207)"
    );
}

/// A cache step with no name of its own is a REFUSAL, not a declaration nobody
/// can trace.
///
/// GITHUB NAMES THE SAVE AFTER THE STEP — `Post <name>` — so an unnamed cache
/// step is reported as `Post Run actions/cache@v6`, which two unnamed steps in
/// one job share. R1207 made the interval a key is judged over the history of
/// THAT key's archive, and a join that picked whichever came first would bound
/// one cache's interval with another cache's save. Refusing is the only answer
/// that is not silently about the wrong archive.
/// Two cache steps of one workflow sharing a name is a REFUSAL too.
///
/// THE OTHER HALF OF THE SAME RULE. Refusing an unnamed step keeps the join from
/// being absent; this keeps it from being AMBIGUOUS. A run's jobs page is one
/// document, so a reader asking "did `Post Cache cargo` conclude success" cannot
/// tell two jobs apart — and the key whose job was skipped would be bounded by
/// its sibling's save, which is a wrong interval wearing the shape of a right
/// one. GitHub allows the duplicate; this repository does not.
#[test]
#[should_panic(expected = "two cache steps are both named `Cache cargo`")]
fn two_cache_steps_of_one_workflow_sharing_a_name_are_refused() {
    caches_of(
        r#"
jobs:
  one:
    runs-on: ubuntu-latest
    steps:
      - name: Cache cargo
        uses: actions/cache@v6
        with:
          path: target
          key: ${{ runner.os }}-cargo-one-
  two:
    runs-on: ubuntu-latest
    steps:
      - name: Cache cargo
        uses: actions/cache@v6
        with:
          path: target
          key: ${{ runner.os }}-cargo-two-
"#,
    );
}

#[test]
#[should_panic(expected = "caches with no step name")]
fn a_cache_step_with_no_name_of_its_own_is_refused() {
    caches_of(
        r#"
jobs:
  anonymous:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/cache@v6
        with:
          path: target
          key: ${{ runner.os }}-cargo-
"#,
    );
}

#[test]
fn what_a_job_waits_for_is_read_in_both_spellings_github_allows() {
    // A single id and a list are one thing. A reader knowing only the list form
    // answers "waits for nothing" for the single form — the same class of defect
    // as reading `--flag value` but not `--flag=value`.
    let needs = job_needs(&ci_plan::parse_workflow(TWO_CACHES, "w.yml"));
    assert_eq!(needs.get("side"), Some(&vec!["validate".to_string()]));
    assert_eq!(
        needs.get("validate"),
        Some(&Vec::new()),
        "a job that waits for nothing is present with an empty list, not absent \
         — a gate asking `needs` for an unknown job must be able to tell \
         \"waits for nothing\" from \"there is no such job\""
    );

    let listed = ci_plan::parse_workflow(
        r#"
jobs:
  last:
    runs-on: ubuntu-latest
    needs: [one, two]
    steps:
      - run: true
"#,
        "w.yml",
    );
    assert_eq!(
        job_needs(&listed).get("last"),
        Some(&vec!["one".to_string(), "two".to_string()])
    );
}

#[test]
fn this_repository_declares_the_caches_its_jobs_are_slow_without() {
    // Against the real tree, asserting REACH rather than a fixed list: pinning
    // the count would make this test the second list that goes stale, which is
    // the shape this crate exists to remove.
    let root = repository_root();
    let declared = ci_plan::workflow_cache_declarations(&root);
    assert!(
        declared.len() >= 2,
        "this repository's CI compiles Rust in more than one job, so it cannot \
         declare fewer than two caches — an empty answer here is the reader \
         failing, not the repository being tidy"
    );
    assert!(
        declared
            .iter()
            .all(|d| !d.prefix.is_empty() && !d.paths.is_empty()),
        "every declaration carries a prefix to join on and the paths that say \
         what it costs: {declared:?}"
    );
    // Every workflow declaring a cache is one `workflow_files` tracked, so an
    // untracked workflow — which GitHub does not run — cannot contribute.
    let tracked = ci_plan::workflow_files(&root);
    assert!(declared.iter().all(|d| tracked.contains(&d.source)));
}

/// One cache step, so the three ways a fallback fails can each be taken. This
/// repository has none of them — which is exactly why they are written here and
/// not left to the tree to demonstrate.
fn one_cache(key: &str, restore_keys: &str) -> CacheDeclaration {
    one_cache_holding("target", key, restore_keys)
}

/// The same, with the `path:` list said out loud — because since R1160 the
/// declaration's paths decide which law it answers to, and a fixture that always
/// holds `target` can only ever exercise one side of that.
fn one_cache_holding(paths: &str, key: &str, restore_keys: &str) -> CacheDeclaration {
    let with_restore = if restore_keys.is_empty() {
        String::new()
    } else {
        format!("          restore-keys: |\n            {restore_keys}\n")
    };
    let held: String = paths
        .lines()
        .map(|path| format!("            {}\n", path.trim()))
        .collect();
    let yaml = format!(
        "jobs:\n  j:\n    runs-on: ubuntu-latest\n    steps:\n      - name: Cache j\n\
         \x20       uses: actions/cache@v6\n\
         \x20       with:\n          path: |\n{held}          key: {key}\n{with_restore}"
    );
    let declared = cache_steps(&parse_workflow(&yaml, "fixture"), "fixture");
    assert_eq!(declared.len(), 1, "{declared:?}");
    declared.into_iter().next().expect("one declaration")
}

#[test]
fn a_fallback_that_cannot_serve_an_earlier_generation_is_named() {
    let good = one_cache(
        "${{ runner.os }}-cargo-unrun-${{ hashFiles('**/Cargo.lock') }}",
        "${{ runner.os }}-cargo-unrun-",
    );
    assert_eq!(
        ci_plan::survives_an_edit(&good),
        None,
        "the fallback is the prefix with no expression left in it, which is what \
         keeps a run after an edit warm: {good:?}"
    );

    let none = one_cache(
        "${{ runner.os }}-cargo-unrun-${{ hashFiles('**/Cargo.lock') }}",
        "",
    );
    assert!(
        ci_plan::survives_an_edit(&none).is_some_and(|why| why.contains("no `restore-keys`")),
        "with nothing to fall back to, every edit to the workflow the key hashes \
         is a cold run for this cache"
    );

    let still_hashing = one_cache(
        "${{ runner.os }}-cargo-unrun-${{ hashFiles('**/Cargo.lock') }}",
        "${{ runner.os }}-cargo-unrun-${{ hashFiles('**/Cargo.lock') }}",
    );
    assert!(
        ci_plan::survives_an_edit(&still_hashing).is_some_and(|why| why.contains("expression")),
        "a fallback that still hashes moves whenever the primary key does, so it \
         is the same cold run one level down"
    );

    let disagreeing = one_cache(
        "${{ runner.os }}-cargo-unrun-${{ hashFiles('**/Cargo.lock') }}",
        "${{ runner.os }}-cargo-",
    );
    assert!(
        ci_plan::survives_an_edit(&disagreeing).is_some_and(|why| why.contains("do not agree")),
        "THE QUIET ONE: this restores something, so every job looks warm, while \
         every gate in this repository joins on `Linux-cargo-unrun-` — a key \
         GitHub is never asked for. Both spellings read correctly alone."
    );
}

#[test]
fn every_cache_here_survives_an_edit_to_the_workflow_it_is_written_in() {
    // THE PREMISE OF EVERY CROSS-RUN COMPARISON THIS REPOSITORY MAKES, and it
    // was a sentence until R1116. Each key here ends in a `hashFiles` of the
    // lockfiles AND of the workflow file, so every edit to that file moves all
    // eight primary keys at once. What keeps the runs either side of an edit
    // comparable is `restore-keys` — and nothing read it, so no measurement
    // could contradict a claim about it. R1115 put the opposite into the ledger
    // as a carry (that an edit costs a cold run) while editing the file whose
    // own comment records run 31337461298 measuring otherwise: five of seven
    // jobs reported `no exact hit` and 221, 246, 281, 756 and 32063 MB arrived.
    //
    // R1160 SCOPED IT TO THE CACHES THAT CAN AFFORD IT, and the scope is the
    // finding rather than an exemption. Surviving an edit means inheriting the
    // previous generation, and for the BUILD DIRECTORY that inheritance is the
    // defect this repository went red on: see
    // `no_cache_here_inherits_a_tree_nothing_bounds` below. The two laws are one
    // partition of the same population, so each asserts the other's half is not
    // empty — a scope that quietly grew to cover everything would leave both
    // passing over nothing.
    let root = repository_root();
    let declared = ci_plan::workflow_cache_declarations(&root);
    let build_directory = ci_plan::build_directory(&root);
    let (holds_a_tree, bounded): (Vec<_>, Vec<_>) = declared
        .iter()
        .partition(|d| d.paths.iter().any(|path| path.trim() == build_directory));
    let broken: Vec<String> = bounded
        .iter()
        .filter_map(|d| ci_plan::survives_an_edit(d).map(|why| format!("{} — {why}", d.owner)))
        .collect();
    assert!(
        broken.is_empty(),
        "a cache that cannot survive an edit to its own workflow makes every \
         run either side of one incomparable, which is the licence R1105's law \
         checks and this is what would silently revoke it:\n  {}",
        broken.join("\n  ")
    );
    assert!(
        bounded.len() >= 2 && bounded.iter().all(|d| !d.restore_keys.is_empty()),
        "and the reach is asserted, because a walk that found no declaration \
         passes the loop above without looking at anything: {bounded:?}"
    );
    assert!(
        !holds_a_tree.is_empty(),
        "this law is now HALF a partition, and the other half is empty — either \
         `{build_directory}` stopped being cached, in which case delete the \
         partition and go back to one law, or `build_directory` is reading the \
         wrong name and the tree cache is being judged by the wrong one of the \
         two: {declared:?}"
    );
}

#[test]
fn no_cache_here_inherits_a_tree_nothing_bounds() {
    // WHAT WENT RED, AND THE ARITHMETIC BEHIND IT. Run 31599893855 refused this
    // repository at 10.75 GB of declared caches against GitHub's 10.00 GB, with
    // `Linux-cargo-unrun-` alone at 9.95 GB of it. That key holds `target` AND
    // declared a fallback, and the two together are an accumulator: the archive
    // is only ever written on the run whose primary key moved, and on exactly
    // that run the fallback has already unpacked the previous generation onto
    // the disk about to be saved.
    //
    // MEASURED RATHER THAN REASONED. That cache put 37,427 MB on the runner,
    // while a clean build of everything the job compiles is 3.86 GB. Nine tenths
    // of the budget's largest item is in no build at all.
    let root = repository_root();
    let declared = ci_plan::workflow_cache_declarations(&root);
    let build_directory = ci_plan::build_directory(&root);
    let unbounded: Vec<String> = declared
        .iter()
        .filter_map(|d| {
            ci_plan::inheritance_without_a_bound(d, &build_directory)
                .map(|why| format!("{} — {why}", d.owner))
        })
        .collect();
    assert!(
        unbounded.is_empty(),
        "a cache holding the build directory may not fall back onto an earlier \
         generation of it: what it saves is the union, and the union has no \
         bound in this repository's contents:\n  {}",
        unbounded.join("\n  ")
    );
    assert!(
        declared
            .iter()
            .any(|d| d.paths.iter().any(|path| path.trim() == build_directory)),
        "and the subject is asserted: with no cache holding `{build_directory}` \
         the loop above passes without looking at anything, which is the shape \
         this repository keeps finding in other people's gates: {declared:?}"
    );
}

#[test]
fn a_fallback_onto_a_build_directory_is_named_and_one_onto_a_cargo_home_is_not() {
    // BOTH ARMS, because the law is a partition and a predicate that answered
    // `Some` for everything would satisfy the tree-wide test above by refusing
    // the whole repository.
    let inherits_a_tree = one_cache_holding(
        "target",
        "${{ runner.os }}-cargo-unrun-${{ hashFiles('**/Cargo.lock') }}",
        "${{ runner.os }}-cargo-unrun-",
    );
    assert!(
        ci_plan::inheritance_without_a_bound(&inherits_a_tree, "target")
            .is_some_and(|why| why.contains("union") && why.contains("Linux-cargo-unrun-")),
        "the shape run 31599893855 refused, and the message has to name the key: \
         {inherits_a_tree:?}"
    );

    let owns_its_tree = one_cache_holding(
        "target",
        "${{ runner.os }}-cargo-unrun-${{ hashFiles('**/Cargo.lock') }}",
        "",
    );
    assert_eq!(
        ci_plan::inheritance_without_a_bound(&owns_its_tree, "target"),
        None,
        "with nothing to fall back to, the archive is one clean build's tree and \
         nothing older: {owns_its_tree:?}"
    );

    let cargo_home = one_cache_holding(
        "~/.cargo/registry\n~/.cargo/git",
        "${{ runner.os }}-cargo-validate-${{ hashFiles('**/Cargo.lock') }}",
        "${{ runner.os }}-cargo-validate-",
    );
    assert_eq!(
        ci_plan::inheritance_without_a_bound(&cargo_home, "target"),
        None,
        "THE HALF THAT MUST KEEP INHERITING: a cargo home holds one entry per \
         crate version a lockfile has named, so its union is bounded and the \
         fallback is what keeps a run after an edit warm: {cargo_home:?}"
    );

    // AND THE NAME IS READ, not spelled here twice: a repository building into
    // `build/` would have its tree cache judged as a cargo home by a predicate
    // that hard-coded `target`.
    let elsewhere = one_cache_holding(
        "build",
        "${{ runner.os }}-cargo-unrun-${{ hashFiles('**/Cargo.lock') }}",
        "${{ runner.os }}-cargo-unrun-",
    );
    assert!(
        ci_plan::inheritance_without_a_bound(&elsewhere, "build").is_some(),
        "the build directory is whatever `build.target-dir` says it is"
    );
    assert_eq!(
        ci_plan::inheritance_without_a_bound(&elsewhere, "target"),
        None,
        "and a path that is not this repository's build directory is not judged \
         as one"
    );
}

#[test]
fn the_build_directory_is_read_from_the_file_that_sets_it() {
    // NON-VACUITY IS THE POINT OF THIS TEST. `build_directory` falls back to
    // cargo's own default, which is `target` — the same answer this repository's
    // config gives. So a reader that silently failed to open the file would
    // return the right string for the wrong reason, and every law joining on it
    // would keep passing while reading nothing.
    let root = repository_root();
    let raw = std::fs::read_to_string(root.join(".cargo/config.toml"))
        .expect("this repository tracks .cargo/config.toml");
    assert!(
        raw.contains("target-dir"),
        "the file exists but no longer SETS the directory, so the assertion \
         below would be testing cargo's default rather than this repository's \
         answer — say it out loud there or delete this law"
    );
    assert_eq!(ci_plan::build_directory(&root), "target");
}

fn repository_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("this crate lives two directories below the repository root")
        .to_path_buf()
}

/// The lister's output as `scripts/check-side-workspaces.sh --list` prints it,
/// including the skip this machine cannot produce.
const LISTED: &str = "[side-workspaces] LOCK bench ours\n\
     [side-workspaces] CHECKABLE bench\n\
     [side-workspaces] COMMAND bench clippy cargo clippy --manifest-path bench/Cargo.toml --locked --all-targets -- -D warnings\n\
     [side-workspaces] COMMAND bench suite cargo test --manifest-path bench/Cargo.toml --locked --no-fail-fast\n\
     [side-workspaces] SKIP studio — its path dependencies leave this \
     repository and are not on this machine: ../pinion/crates/pinion-a11y\n\
     [side-workspaces] LOCK tools/item-citations ours\n\
     [side-workspaces] CHECKABLE tools/item-citations\n\
     [side-workspaces] COMMAND tools/item-citations suite cargo test --manifest-path tools/item-citations/Cargo.toml --locked --no-fail-fast\n\
     [side-workspaces] checked 2 (bench tools/item-citations), skipped 1 (studio)\n";

/// The same lister over a machine that HAS the sibling checkout: `studio` is
/// checkable there and still foreign, which is the pair no single predicate can
/// express and the reason R1115 split them.
const LISTED_WITH_THE_SIBLING: &str = "[side-workspaces] LOCK bench ours\n\
     [side-workspaces] CHECKABLE bench\n\
     [side-workspaces] COMMAND bench suite cargo test --manifest-path bench/Cargo.toml --locked --no-fail-fast\n\
     [side-workspaces] LOCK studio foreign — it resolves against trees this \
     repository does not own, so its lockfile is not this repository's to pin: \
     ../../pinion/crates/pinion-core\n\
     [side-workspaces] CHECKABLE studio\n\
     [side-workspaces] COMMAND studio suite cargo test --manifest-path studio/Cargo.toml --no-fail-fast\n\
     [side-workspaces] checked 2 (bench studio), skipped 0 ()\n";

#[test]
fn the_lister_is_read_for_what_it_can_and_cannot_be_asked() {
    let listed = parse_lister(LISTED);
    assert_eq!(
        listed.askable,
        vec![
            "Cargo.toml".to_string(),
            "bench/Cargo.toml".to_string(),
            "tools/item-citations/Cargo.toml".to_string(),
        ],
        "the root is always asked; a skipped workspace is not"
    );
    assert_eq!(listed.skipped.len(), 1, "{:?}", listed.skipped);
    assert_eq!(
        listed.skipped[0].directory, "studio",
        "the directory is a field, not a prefix of a sentence — a gate deciding \
         whether a FILE is inside a workspace this machine cannot compile needs \
         it without re-splitting the reason: {:?}",
        listed.skipped
    );
    assert!(
        listed.skipped[0].reason.contains("../pinion"),
        "and the reason is kept beside it, so the print still says why: {:?}",
        listed.skipped
    );
    assert!(
        !listed.askable.iter().any(|m| m.starts_with("studio")),
        "a workspace the lister could not check must not be asked anyway: {:?}",
        listed.askable
    );
}

#[test]
fn a_skipped_workspace_contributes_no_suite_to_run() {
    let listed = parse_lister(LISTED);
    assert_eq!(
        listed
            .commands
            .iter()
            .filter(|declared| declared.role == "suite")
            .map(|declared| declared.workspace.as_str())
            .collect::<Vec<_>>(),
        vec!["bench", "tools/item-citations"],
        "only the checkable ones have a suite: {:?}",
        listed.commands
    );
    let commands = lister_suite_commands(&listed);
    assert_eq!(commands.len(), 2, "{commands:?}");
    assert!(
        commands
            .iter()
            .all(|command| command.subcommand() == Some("test")),
        "the lister's suite is a cargo test: {commands:?}"
    );
    assert_eq!(
        commands[0].value(&["--manifest-path"]),
        Some("bench/Cargo.toml"),
        "the manifest the suite is pointed at is readable off the command"
    );
    assert!(
        commands
            .iter()
            .all(|command| command.harness_args.is_empty()),
        "the lister passes the harness nothing, so it runs every test the \
         command's targets hold: {commands:?}"
    );
}

/// THE POPULATION CARRIES WHAT THE MACHINE COULD NOT REACH, AND BOTH MACHINES
/// ARE RUN HERE (R1228).
///
/// The commands a law judges come from the lister, so on a hosted runner they
/// are `studio`'s eight fewer than on a workstation holding the sibling
/// checkout. Until this round `commands_this_repository_issues` handed back the
/// commands alone: three laws read a population whose size is a fact about the
/// machine and not one of them could say so, while the three laws that take the
/// lister's answer directly all say it. The difference was the shape, not the
/// authors.
///
/// BOTH ARMS RUN ON EVERY MACHINE, which is the discipline Round 1227 paid for:
/// a case that only exercises the branch its author's machine is on ships the
/// other one untested. `LISTED` is a runner and `LISTED_WITH_THE_SIBLING` is a
/// workstation, and the assertion below is that the pairing holds in both
/// directions — a skip present and named, and a skip absent with the commands
/// it would have contributed present instead.
#[test]
fn the_commands_a_machine_can_issue_carry_the_workspaces_it_could_not_reach() {
    let runner = IssuedCommands::from_lister(&parse_lister(LISTED));
    assert_eq!(
        runner
            .skipped
            .iter()
            .map(|skipped| skipped.directory.as_str())
            .collect::<Vec<_>>(),
        vec!["studio"],
        "the lister skipped a workspace and the population handed on did not \
         carry it, so a law reading this cannot say what it did not judge: {:?}",
        runner.skipped
    );
    // AND THE SENTENCE NAMES BOTH HALVES. A law prints this and nothing else
    // about the workspace, so a reason that went missing here is a skip a
    // reader cannot act on.
    let said = runner.skipped[0].was_not("judged");
    assert!(
        said.starts_with("not judged (the lister says why): studio "),
        "the sentence must carry the verb of the law saying it and the \
         workspace: {said}"
    );
    assert!(
        said.contains("../pinion"),
        "and the lister's own reason, which is the half that says WHY this \
         machine could not: {said}"
    );
    assert!(
        !runner
            .commands
            .iter()
            .any(|command| command.owner == "studio"),
        "a workspace the lister skipped contributed commands anyway, so the \
         skip is decoration: {:?}",
        runner.commands
    );

    // THE OTHER MACHINE. Same call, a lister that HAS the sibling: nothing is
    // skipped and the commands are there, so the empty `skipped` above is a
    // real answer rather than a field nothing ever fills.
    let workstation = IssuedCommands::from_lister(&parse_lister(LISTED_WITH_THE_SIBLING));
    assert!(
        workstation.skipped.is_empty(),
        "this lister reached every workspace, so nothing should be reported \
         unreachable: {:?}",
        workstation.skipped
    );
    assert!(
        workstation
            .commands
            .iter()
            .any(|command| command.owner == "studio"),
        "the workspace the other machine could not reach is IN the population \
         here, which is the difference the pairing exists to state: {:?}",
        workstation.commands
    );
}

#[test]
fn every_command_the_lister_runs_is_read_and_not_only_the_suite() {
    let listed = parse_lister(LISTED);
    let commands = lister_declared_commands(&listed);
    assert_eq!(
        commands.len(),
        3,
        "the four checks that run BEFORE the suite are the ones that were \
         invisible, so reading only the suite is reading the one command that \
         already carried the flag: {commands:?}"
    );
    let clippy = commands
        .iter()
        .find(|command| command.subcommand() == Some("clippy"))
        .expect("the lister declares its lint command");
    assert!(
        clippy.has("--locked"),
        "and the flag is readable off it: {clippy:?}"
    );
    assert_eq!(
        clippy.harness_args,
        vec!["-D".to_string(), "warnings".to_string()],
        "a declared command splits at the bare `--` like any other: {clippy:?}"
    );
}

#[test]
fn a_workspace_can_be_checkable_here_and_still_not_this_repositorys_to_pin() {
    let listed = parse_lister(LISTED_WITH_THE_SIBLING);
    assert!(
        listed.skipped.is_empty(),
        "this machine has the sibling checkout: {:?}",
        listed.skipped
    );
    assert_eq!(
        listed.ownership.get("bench"),
        Some(&Ownership::Ours),
        "{:?}",
        listed.ownership
    );
    let Some(Ownership::Foreign(reason)) = listed.ownership.get("studio") else {
        panic!(
            "studio resolves against a tree outside this repository whether \
                or not that tree is here: {:?}",
            listed.ownership
        );
    };
    assert!(
        reason.contains("pinion"),
        "and the lister's own words say which dependencies made it so: {reason}"
    );
    let studio = lister_declared_commands(&listed)
        .into_iter()
        .find(|command| command.owner == "studio")
        .expect("studio has a declared command");
    assert!(
        !studio.has("--locked"),
        "a foreign workspace gets no `--locked`, because the commit that would \
         break it is one in another repository: {studio:?}"
    );
}

#[test]
fn a_verdict_this_reader_does_not_know_is_not_read_as_either_answer() {
    let listed = parse_lister(
        "[side-workspaces] CHECKABLE bench\n\
         [side-workspaces] LOCK bench vendored — a third state nobody has written yet\n",
    );
    assert!(
        listed.ownership.is_empty(),
        "an unknown verdict is absent rather than guessed: a reader that folded \
         it into `ours` would demand `--locked`, and one that folded it into \
         `foreign` would stop demanding it — both are answers about a state this \
         reader has never seen: {:?}",
        listed.ownership
    );
}

#[test]
fn a_command_written_behind_a_shell_keyword_is_still_a_command() {
    let parsed = parse_script(
        "if ! cargo test --workspace; then\n  echo no\nfi\n\
         while ! cargo build; do :; done\n",
    );
    assert_eq!(
        parsed.len(),
        2,
        "`if ! cargo …` is a cargo invocation and reading the first word \
         literally answers `if`: {parsed:?}"
    );
    assert_eq!(parsed[0].cargo_args[0], "cargo", "{parsed:?}");
    assert_eq!(parsed[0].cargo_args[1], "test", "{parsed:?}");
    assert_eq!(parsed[1].cargo_args[1], "build", "{parsed:?}");
    assert!(
        parsed.iter().all(|found| found.carrier.is_empty()),
        "a shell keyword is not a program that hands a command over — recording \
         `if` as a carrier would make the law about wrappers read it as one: \
         {parsed:?}"
    );
}

/// R1196 — the reading that lets a suite be wrapped in the thing that judges
/// what it covered without vanishing from every population built on this crate.
#[test]
fn a_command_a_wrapper_carries_is_still_a_command_this_repository_issues() {
    let parsed = parse_script(
        "scripts/verify.sh --no-fresh --label side-bench -- \
         cargo test --manifest-path bench/Cargo.toml --locked -- --nocapture\n",
    );
    assert_eq!(
        parsed.len(),
        1,
        "a wrapper hands the rest over after a bare `--`, and reading only the \
         first word answers `scripts/verify.sh`: {parsed:?}"
    );
    assert_eq!(
        parsed[0].carrier,
        vec![
            "scripts/verify.sh".to_string(),
            "--no-fresh".to_string(),
            "--label".to_string(),
            "side-bench".to_string()
        ],
        "and WHAT carried it is kept, because a law about which runs are judged \
         has nowhere else to read it: {parsed:?}"
    );
    assert_eq!(
        parsed[0].cargo_args,
        vec![
            "cargo".to_string(),
            "test".to_string(),
            "--manifest-path".to_string(),
            "bench/Cargo.toml".to_string(),
            "--locked".to_string()
        ],
        "cargo's side is exactly what it would be unwrapped: {parsed:?}"
    );
    assert_eq!(
        parsed[0].harness_args,
        vec!["--nocapture".to_string()],
        "and the SECOND bare `--` is still cargo's own: {parsed:?}"
    );
}

/// The half that stops the new reading from eating a command's own arguments.
#[test]
fn cargos_own_marker_does_not_start_a_second_command() {
    let parsed = parse_script("cargo test --workspace -- --exact cargo test\n");
    assert_eq!(parsed.len(), 1, "{parsed:?}");
    assert!(
        parsed[0].carrier.is_empty(),
        "cargo is the program here, so nothing carried it: {parsed:?}"
    );
    assert_eq!(
        parsed[0].harness_args,
        vec![
            "--exact".to_string(),
            "cargo".to_string(),
            "test".to_string()
        ],
        "everything past cargo's own `--` is the harness's, whatever words it \
         happens to hold — re-reading it would report a command nobody runs: \
         {parsed:?}"
    );
}

/// R1197 — the fourth place a cargo command is written in this repository.
///
/// Against the TREE rather than pinned text, unlike its neighbours, and that is
/// the point: what makes this reader worth having is that the file it reads is
/// one nothing else did, so a fixture would be asserting about a declaration
/// this repository does not have.
#[test]
fn the_declaration_is_read_for_the_commands_it_issues() {
    let found = ci_plan::declared_build_commands(&repository_root());
    let roles: Vec<&str> = found.iter().map(|command| command.owner.as_str()).collect();
    assert_eq!(
        roles,
        vec!["build", "sweep", "verify"],
        "each declared command is read, and it is read WITH ITS ROLE — a finding \
         that named the file three times would leave a reader to work out which \
         of the three is the defective one: {found:?}"
    );
    assert!(
        found
            .iter()
            .all(|command| command.source == ci_plan::BUILD_DECLARATION),
        "and they all say where they are written: {found:?}"
    );

    let verify = found
        .iter()
        .find(|command| command.owner == "verify")
        .expect("the declaration says how a build machine verifies this tree");
    assert_eq!(
        verify.subcommand(),
        Some("test"),
        "the wrapper in front is read through, so the subcommand is cargo's own \
         and not `scripts/verify.sh`: {verify:?}"
    );
    assert!(
        verify
            .carrier
            .first()
            .is_some_and(|program| program.ends_with("verify.sh")),
        "and what carries it is kept, because that is what the coverage law \
         reads: {verify:?}"
    );

    let sweep = found
        .iter()
        .find(|command| command.owner == "sweep")
        .expect("the declaration says which sweep proves the contracts");
    assert!(
        sweep.carrier.is_empty(),
        "a command issued directly is carried by nothing, so the two cases are \
         distinguishable rather than assumed: {sweep:?}"
    );
    // AND CARGO'S OWN BARE `--` STILL SEPARATES IT FROM WHAT IT PASSES ON.
    // R1258 gave the harness verbs, so what it passes on is a verb and then the
    // manifest — asserted as those two words rather than as a count, because a
    // length is the same number for `sweep <manifest>` and for a header that
    // lost its verb and gained a flag.
    assert_eq!(
        sweep.harness_args.first().map(String::as_str),
        Some("sweep"),
        "the declaration names the verb the harness answers to: {sweep:?}"
    );
    assert!(
        sweep
            .harness_args
            .last()
            .is_some_and(|last| last.ends_with("sweep.json")),
        "and the sweep it proves the contracts with: {sweep:?}"
    );
    assert_eq!(
        sweep.harness_args.len(),
        2,
        "and nothing else — a flag nobody reads back is one that can stop \
         existing in silence: {sweep:?}"
    );
}

/// A wrapper may carry a wrapper, and a program with no cargo behind it carries
/// nothing.
#[test]
fn the_reading_follows_every_hop_and_invents_no_command() {
    let nested = parse_script("outer -x -- inner --flag -- cargo build --locked\n");
    assert_eq!(nested.len(), 1, "{nested:?}");
    assert_eq!(
        nested[0].carrier,
        vec![
            "outer".to_string(),
            "-x".to_string(),
            "inner".to_string(),
            "--flag".to_string()
        ],
        "both hops are what handed it over: {nested:?}"
    );
    assert_eq!(nested[0].cargo_args[1], "build", "{nested:?}");
    assert!(
        parse_script("scripts/verify.sh --label x -- ./scripts/check-side-workspaces.sh\n")
            .is_empty(),
        "a wrapper carrying something that is not cargo issues no cargo command"
    );
    assert!(
        parse_script("timeout 60 cargo test --workspace\n").is_empty(),
        "a wrapper that takes its command WITHOUT a bare `--` is not read, and \
         that is the strict direction: the words in front of `cargo` could be \
         anything, and guessing which of them take an argument of their own is \
         how a reader starts answering about a command nobody runs"
    );
}

#[test]
fn a_word_that_merely_starts_with_a_keyword_does_not_hide_a_command() {
    let parsed = parse_script("iffy cargo test\n");
    assert!(
        parsed.is_empty(),
        "`iffy` is a program, not a shell keyword, and the command it runs is \
         its own — skipping it would attribute `cargo test` to a line that does \
         not issue it: {parsed:?}"
    );
}

#[test]
fn one_script_is_split_into_commands_once_for_every_law_that_reads_it() {
    // R1210 — the splitter is public because a second law asks about the same
    // segments (which steps INSTALL something), and the two must not be able to
    // disagree about where one command ends. The assertion is that they are the
    // SAME reading: every cargo command `parse_script` finds is a segment
    // `shell_commands` returns, on a script that puts three commands on two
    // lines with three different operators.
    let script = "sudo apt-get update && sudo apt-get install -y protobuf-compiler\n\
                  cargo test --workspace --locked ; echo done\n";
    let commands = ci_plan::shell_commands(script);
    assert_eq!(
        commands.len(),
        4,
        "four segments, and the empty one an operator leaves behind is not a \
         command: {commands:?}"
    );
    assert_eq!(commands[1][1], "apt-get", "{commands:?}");
    assert_eq!(
        commands[1].last().expect("the package"),
        "protobuf-compiler",
        "the words of a non-cargo command survive whole, which is what the \
         install law reads: {commands:?}"
    );
    let cargo: Vec<Vec<String>> = commands
        .iter()
        .filter(|words| words.first().map(String::as_str) == Some("cargo"))
        .cloned()
        .collect();
    assert_eq!(
        cargo.len(),
        parse_script(script).len(),
        "the two readings find the same cargo commands in the same script — a \
         second splitter is what would let them drift: {commands:?}"
    );
}

#[test]
fn a_step_that_runs_an_action_is_read_on_the_same_coordinate_as_one_that_runs_shell() {
    // R1210 — a law built on `run:` steps alone reports a job that installs its
    // tools through an action as installing nothing. The two readings share
    // `index` (every step counted, not only the ones of one kind), so a caller
    // holding both can say which came first.
    let doc = parse_workflow(
        "jobs:\n  build:\n    steps:\n      - uses: actions/checkout@v7\n      \
         - run: rustup show\n      - uses: arduino/setup-protoc@v3\n",
        "x.yml",
    );
    let uses = ci_plan::uses_steps(&doc, "x.yml");
    assert_eq!(uses.len(), 2, "{uses:?}");
    assert_eq!(uses[0].action(), "actions/checkout", "{uses:?}");
    assert_eq!(uses[0].index, 0, "{uses:?}");
    assert_eq!(uses[1].action(), "arduino/setup-protoc", "{uses:?}");
    assert_eq!(
        uses[1].index, 2,
        "the third step is at 2 whether or not the reader cares about the second \
         — an index among `uses:` steps only would say this one is the second: \
         {uses:?}"
    );
    let runs = run_steps(&doc);
    assert_eq!(runs.len(), 1, "{runs:?}");
    assert_eq!(
        runs[0].index, 1,
        "and the same counting on the other side: {runs:?}"
    );
}

#[test]
fn a_steps_own_bound_and_its_jobs_are_both_read_and_are_told_apart_from_none() {
    // R1237 — the datum R1236's carry named. A step that fetches from a network
    // service and declares no bound of its own can spend the whole job's budget
    // on a third party not answering, which is what two jobs of one run did on
    // 2026-08-18 (45m09s and ninety minutes, in the same `apt-get`). Reading it
    // is the half a law needs, and the JOB's number is here for the same reason:
    // a step bounded at exactly its job's budget is bounded and prevents nothing.
    let doc = parse_workflow(
        "jobs:\n  build:\n    timeout-minutes: 45\n    steps:\n      \
         - run: sudo apt-get install -y protoc\n        timeout-minutes: 5\n      \
         - run: rustup show\n      \
         - run: echo hi\n        timeout-minutes: \"${{ env.BUDGET }}\"\n  \
         loose:\n    steps:\n      - run: cargo build\n",
        "x.yml",
    );
    let steps = run_steps(&doc);
    assert_eq!(steps.len(), 4, "{steps:?}");

    let bounded = &steps[0];
    assert_eq!(bounded.timeout.as_deref(), Some("5"));
    assert_eq!(
        bounded.job_timeout.as_deref(),
        Some("45"),
        "and the job's own budget travels with it, so one law can compare them"
    );

    assert_eq!(
        steps[1].timeout, None,
        "a step nobody bounded says so — this is the state ten steps of this \
         repository were in when the law that reads this was written"
    );
    assert_eq!(steps[1].job_timeout.as_deref(), Some("45"));

    // THE THIRD ANSWER, and the reason this field is not a number. GitHub takes
    // an expression here and that IS a declaration; a reader that parsed to an
    // integer would report it as `None`, which is the word it uses for a step
    // with no bound at all — opposite facts, one spelling.
    assert_eq!(
        steps[2].timeout.as_deref(),
        Some("${{ env.BUDGET }}"),
        "an expression is a bound this reader cannot evaluate, not an absent one"
    );

    assert_eq!(
        steps[3].job_timeout, None,
        "a job that declares no budget says so rather than borrowing another \
         job's — GitHub's own default (360) is not written in the file, and a \
         reader that supplied it would be answering for the file"
    );
}

/// Build a `CargoCommand` from a line, the way a workflow or a script writes it.
fn issued(line: &str) -> CargoCommand {
    let mut words: Vec<String> = line.split_whitespace().map(str::to_string).collect();
    let harness_args = match words.iter().position(|word| word == "--") {
        Some(at) => words.split_off(at).split_off(1),
        None => Vec::new(),
    };
    CargoCommand {
        source: "a fixture".to_string(),
        owner: "a fixture".to_string(),
        carrier: Vec::new(),
        cargo_args: words,
        harness_args,
        env: Default::default(),
        // Written down, so the manifest path is what says whose lockfile it is.
        declared: None,
    }
}

#[test]
fn a_repeatable_flag_is_read_at_every_place_it_is_written() {
    let command = issued("cargo test -p mnemosyne-cli --test alpha --test=beta --locked");
    assert_eq!(
        command.value(&["--test"]),
        Some("alpha"),
        "the single-value reader still answers with the first, which is the \
         right reading for a flag cargo allows only once"
    );
    assert_eq!(
        command.values(&["--test"]),
        vec!["alpha", "beta"],
        "a command naming two targets and a reader seeing one is a half-answer \
         that reads like a whole one — and both spellings are one flag"
    );
    assert_eq!(
        command.values(&["-p", "--package"]),
        vec!["mnemosyne-cli"],
        "a flag written once is one value, not a special case"
    );
    assert!(
        issued("cargo test --test --locked")
            .values(&["--test"])
            .is_empty(),
        "`--test` with a flag after it names no target; recording `--locked` as \
         a target name would report a missing target for a reason that is not \
         the caller's"
    );
    assert!(
        issued("cargo test --workspace")
            .values(&["--test"])
            .is_empty(),
        "a command that names no target must not have one invented for it"
    );
}

const TWO_MANIFESTS: [&str; 2] = ["Cargo.toml", "bench/Cargo.toml"];

fn tracked_pair() -> Vec<String> {
    TWO_MANIFESTS.iter().map(|m| m.to_string()).collect()
}

#[test]
fn a_resolve_without_the_flag_rewrites_the_lockfile_it_should_have_reported() {
    let tracked = tracked_pair();
    let nothing_foreign = BTreeSet::new();
    assert_eq!(
        lock_verdict(
            &issued("cargo clippy --manifest-path bench/Cargo.toml --all-targets"),
            &tracked,
            &nothing_foreign
        ),
        LockVerdict::RepairsWhatItShouldReport,
        "this is the defect the whole law exists for: the lint step repairs the \
         lockfile that the suite two steps later was going to check"
    );
    assert_eq!(
        lock_verdict(
            &issued("cargo clippy --manifest-path bench/Cargo.toml --locked --all-targets"),
            &tracked,
            &nothing_foreign
        ),
        LockVerdict::Pinned,
    );
    assert_eq!(
        lock_verdict(&issued("cargo fmt --check"), &tracked, &nothing_foreign),
        LockVerdict::ResolvesNothing,
        "`cargo fmt` cannot rewrite a lockfile and REJECTS `--locked`, so a law \
         that demanded the flag of everything would be unsatisfiable"
    );
}

#[test]
fn the_flag_is_wrong_on_a_workspace_whose_resolution_belongs_to_another_tree() {
    let tracked = tracked_pair();
    // `bench` stands in for `studio` here: which workspace is foreign is an
    // answer the lister gives, and this is the reading of that answer.
    let foreign = BTreeSet::from(["bench".to_string()]);
    assert_eq!(
        lock_verdict(
            &issued("cargo test --manifest-path bench/Cargo.toml --no-fail-fast"),
            &tracked,
            &foreign
        ),
        LockVerdict::NotOursToPin,
        "its resolution changes when another repository commits — and reading \
         `bench/Cargo.toml` as the root manifest, which is a suffix of it, would \
         report this as a workspace this repository pins"
    );
    assert_eq!(
        lock_verdict(
            &issued("cargo test --manifest-path bench/Cargo.toml --locked"),
            &tracked,
            &foreign
        ),
        LockVerdict::PinsWhatItDoesNotOwn,
        "and pinning it is a gate that goes red for a commit nobody here made"
    );
}

#[test]
fn a_command_this_reader_cannot_place_is_not_read_as_compliant() {
    let tracked = tracked_pair();
    let nothing_foreign = BTreeSet::new();
    assert!(
        matches!(
            lock_verdict(
                &issued("cargo test --manifest-path $ws/Cargo.toml --locked"),
                &tracked,
                &nothing_foreign
            ),
            LockVerdict::Unreadable(_)
        ),
        "`$ws/Cargo.toml` has a literal tail and it is `Cargo.toml`, which every \
         workspace here has one of — resolving it to the root would say the \
         command pins a workspace it may never touch"
    );
    assert!(
        matches!(
            lock_verdict(
                &issued("cargo frobnicate --workspace"),
                &tracked,
                &nothing_foreign
            ),
            LockVerdict::Unreadable(_)
        ),
        "a subcommand nobody has measured against a disagreeing lockfile is not \
         evidence that it leaves one alone"
    );
    assert_eq!(
        lock_verdict(
            &issued("cargo run --manifest-path $root/bench/Cargo.toml --locked"),
            &tracked,
            &nothing_foreign
        ),
        LockVerdict::Pinned,
        "a variable PREFIX still names one manifest, and refusing that would push \
         the git hooks into spelling paths they must not spell — they run over \
         another checkout than their own"
    );
    assert_eq!(
        lock_verdict(
            &issued("cargo build --workspace --locked"),
            &tracked,
            &nothing_foreign
        ),
        LockVerdict::Pinned,
        "a command with no `--manifest-path` resolves the tree it runs in, which \
         for every caller of this crate is the repository root"
    );
}

#[test]
fn a_shell_line_is_split_where_cargo_stops_and_the_harness_starts() {
    let read = |script: &str| {
        parse_script(script)
            .into_iter()
            .map(|found| CargoCommand {
                source: "pinned".to_string(),
                owner: "pinned".to_string(),
                carrier: found.carrier,
                cargo_args: found.cargo_args,
                harness_args: found.harness_args,
                env: Default::default(),
                declared: None,
            })
            .collect::<Vec<_>>()
    };

    let plain = read("cargo test --workspace --locked");
    assert_eq!(plain.len(), 1);
    assert!(
        plain[0].harness_args.is_empty(),
        "no bare `--` means the harness was passed nothing"
    );

    let split = read("cargo test -p mnemosyne-cli --test evidence_replay_smoke --locked -- --ignored --nocapture");
    assert_eq!(split.len(), 1);
    assert_eq!(split[0].value(&["--test"]), Some("evidence_replay_smoke"));
    assert_eq!(
        split[0].harness_args,
        vec!["--ignored".to_string(), "--nocapture".to_string()],
        "everything past the bare `--` is the harness's, and a gate appending \
         `--list` has to append THERE"
    );
    assert!(
        split[0].harness_has("--ignored"),
        "which tests such a command runs is a question about its harness side"
    );
    assert!(
        !split[0].has("--ignored"),
        "and not about cargo's — the two sides must not be searched as one"
    );

    assert!(
        read("sudo apt-get update && sudo apt-get install -y protobuf-compiler").is_empty(),
        "a step with no cargo in it holds no cargo command"
    );
    assert_eq!(
        read("rustup show && cargo test --workspace").len(),
        1,
        "an operator ends one command and starts the next"
    );
    assert!(
        read("cargo run -q --manifest-path tools/item-citations/Cargo.toml --bin item-citations -- --workspace Cargo.toml")
            .first()
            .is_some_and(|command| command.subcommand() == Some("run")),
        "a path holding the word Cargo.toml is not the start of a second command"
    );

    let joined = read("cargo test --features=server/otlp,server/tls --test=smoke");
    assert_eq!(joined.len(), 1);
    assert_eq!(
        joined[0].value(&["--features"]),
        Some("server/otlp,server/tls"),
        "`--flag=value` is the same flag as `--flag value`; reading only the \
         second answers ABSENT for the first, which R1082 carried as a limit"
    );
    assert!(
        joined[0].has("--test"),
        "and a narrowing selector written that way still narrows: {:?}",
        joined[0].cargo_args
    );

    let folded = read("cargo clippy -p mnemosyne-server --all-features \\\n  --all-targets --locked -- -D warnings");
    assert_eq!(
        folded.len(),
        1,
        "a trailing backslash continues the command"
    );
    assert!(
        folded[0].has("--all-targets"),
        "and the continued half is part of it: {folded:?}"
    );
}

/// A workflow whose environment is written at all three levels GitHub allows.
const THREE_LEVELS: &str = r#"
env:
  CARGO_INCREMENTAL: 0
  CARGO_PROFILE_DEV_DEBUG: line-tables-only
jobs:
  validate:
    runs-on: ubuntu-latest
    steps:
      - name: build
        run: cargo test --workspace
  msrv:
    runs-on: ubuntu-latest
    env:
      CARGO_PROFILE_DEV_DEBUG: "2"
    steps:
      - name: check
        env:
          RUSTUP_TOOLCHAIN: 1.88
        run: cargo check --workspace
      - name: check again
        run: cargo check --workspace --all-targets
"#;

#[test]
fn a_step_carries_the_environment_of_all_three_levels_that_set_it() {
    // WHY THIS IS PART OF A COMMAND. `msrv` runs the same words as `validate`
    // over the same sources, and the only thing that makes it a different build
    // is a variable on the step. R1090 then set the debug level for the whole
    // file, which changes what every job in it compiles. A reader that took only
    // the words would call those the same command.
    let steps = run_steps(&parse_workflow(THREE_LEVELS, "pinned"));
    let of = |job: &str, index: usize| {
        steps
            .iter()
            .filter(|step| step.job == job)
            .nth(index)
            .unwrap_or_else(|| panic!("no step {index} of {job}"))
            .env
            .clone()
    };

    let validate = of("validate", 0);
    assert_eq!(
        validate.get("CARGO_INCREMENTAL").map(String::as_str),
        Some("0"),
        "`CARGO_INCREMENTAL: 0` is an integer to a YAML parser and a string to a \
         process — a reader that took only `as_str` would drop it: {validate:?}"
    );
    assert_eq!(
        validate.get("CARGO_PROFILE_DEV_DEBUG").map(String::as_str),
        Some("line-tables-only"),
        "the workflow's own env reaches a job that sets none"
    );

    let first = of("msrv", 0);
    assert_eq!(
        first.get("CARGO_PROFILE_DEV_DEBUG").map(String::as_str),
        Some("2"),
        "the job's env overrides the workflow's"
    );
    assert_eq!(
        first.get("RUSTUP_TOOLCHAIN").map(String::as_str),
        Some("1.88"),
        "and the step's own env is there too"
    );
    assert_eq!(
        first.get("CARGO_INCREMENTAL").map(String::as_str),
        Some("0"),
        "overriding one variable does not drop the others"
    );

    let second = of("msrv", 1);
    assert_eq!(
        second.get("RUSTUP_TOOLCHAIN"),
        None,
        "a step's env is the STEP's — the next step in the same job does not \
         inherit it, which is exactly how a job can build twice on two \
         toolchains: {second:?}"
    );
}

/// A job whose cache sits between two `run:` steps, with steps that are neither
/// on both sides of it.
const BRACKETED: &str = r#"
jobs:
  validate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - run: measure before
      - name: Cache cargo
        uses: actions/cache@v6
        with:
          path: target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
      - run: measure after
      - uses: actions/upload-artifact@v7
      - run: cargo test --workspace
"#;

#[test]
fn a_step_is_read_with_where_it_sits_in_its_job() {
    // THE TWO POPULATIONS COME OUT OF ONE ORDERED LIST, and until they carried a
    // shared coordinate a caller holding both could say WHICH steps a job has
    // and never WHICH CAME FIRST. A measurement taken on both sides of a cache
    // restore is the difference between them, so "is this step before that one?"
    // is a question the file has to be able to answer — R1102 could only answer
    // it by OBSERVING a run.
    let document = parse_workflow(BRACKETED, "w.yml");
    let steps = run_steps(&document);
    assert_eq!(
        steps
            .iter()
            .map(|step| (step.index, step.script.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (1, "measure before"),
            (3, "measure after"),
            (5, "cargo test --workspace"),
        ],
        "EVERY STEP IS COUNTED, not only the ones that `run:` — an index among \
         `run:` steps alone would make the first of these 0 and the cache 2, \
         and a reader comparing those two numbers would place a step before a \
         cache step it in fact follows"
    );
    let caches = cache_steps(&document, "w.yml");
    assert_eq!(
        caches.iter().map(|cache| cache.index).collect::<Vec<_>>(),
        vec![2],
        "and the cache is counted in the same list, which is what makes the two \
         numbers comparable at all"
    );
    // The property the whole coordinate exists for, read off this fixture.
    assert!(steps[0].index < caches[0].index && caches[0].index < steps[1].index);
}

/// A workflow whose two jobs differ in the one thing that decides whether a
/// record written in them can ever be read.
const ONE_COLLECTS: &str = r#"
jobs:
  validate:
    runs-on: ubuntu-latest
    steps:
      - run: cargo test --workspace
      - name: Keep what this job compiled
        uses: actions/upload-artifact@v7
        with:
          name: rustc-log-validate
          path: |
            rustc-log/
            other/
      - name: And what it measured, which a job may collect separately
        uses: actions/upload-artifact@v7
        with:
          name: restored-validate
          path: restored/
  replay:
    runs-on: ubuntu-latest
    steps:
      - run: cargo test -p mnemosyne-cli --test evidence_replay_smoke
"#;

#[test]
fn what_a_workflow_collects_is_read_because_it_decides_what_can_be_read_back() {
    // EVERY RECORD THESE GATES JOIN goes onto a runner that is destroyed when
    // the job ends, and the only thing that survives is what an upload step
    // collected. So a workflow that uploads nothing produces no record anything
    // can download — which is the derivation behind which jobs owe a restore
    // record at all, and the alternative is a list of exempt workflows kept
    // beside the law.
    let document = parse_workflow(ONE_COLLECTS, "w.yml");
    let uploads = ci_plan::artifact_uploads(&document, "w.yml");
    assert_eq!(
        uploads.len(),
        2,
        "EVERY UPLOAD STEP AND NOT THE FIRST — a job may collect more than one \
         thing, and stopping at one reads a workflow as collecting less than it \
         does: {uploads:?}"
    );
    assert!(uploads.iter().all(|one| one.owner == "validate"));
    assert_eq!(uploads[0].paths, vec!["rustc-log/", "other/"]);
    assert_eq!(uploads[1].paths, vec!["restored/"]);
    assert_eq!(
        (uploads[0].index, uploads[1].index),
        (1, 2),
        "the same counting every other step of this crate uses"
    );

    // AND THIS REPOSITORY HAS BOTH SIDES, which is what keeps the derivation
    // from being a rule about a case that does not occur.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("this crate lives two directories below the root");
    let collecting: Vec<String> = ci_plan::workflows_collecting_artifacts(root)
        .into_iter()
        .collect();
    assert_eq!(
        collecting,
        vec![".github/workflows/mnemosyne-validate.yml"],
        "exactly one tracked workflow collects anything, and the other declares \
         a cache — so both sides of the derivation are live"
    );
    // AND THE PREDICATE HAS ONE HOME. Two gates ask "does this workflow collect
    // anything" — the census gate to decide who owes a restore record, the cache
    // gate to decide whose silence about one is its own — and the sweep above is
    // the same question asked of every tracked file. A second spelling of it is
    // where the two start disagreeing.
    assert!(
        !ci_plan::collects_artifacts(&[]) && ci_plan::collects_artifacts(&uploads),
        "the sweep and the per-workflow answer are one reading"
    );
}

// --- which workflow a runner says it is executing ----------------------------
//
// R1107. Two gates need it: the census gate loads that file, and
// `tools/cache-budget` holds it against each cache declaration's source to
// decide whose restore records could have been collected in this run at all —
// records are artifacts, and artifacts belong to a run. The census gate cut the
// string for itself until this round, which is two answers about which file a
// gate is judging.

/// The workflows a reader of this repository's `.github/workflows` would have.
fn tracked() -> Vec<String> {
    vec![
        ".github/workflows/evidence-replay.yml".to_string(),
        ".github/workflows/mnemosyne-validate.yml".to_string(),
    ]
}

#[test]
fn a_runners_workflow_reference_resolves_to_the_file_it_names() {
    assert_eq!(
        ci_plan::workflow_of_reference(
            Some("newmassrael/mnemosyne/.github/workflows/mnemosyne-validate.yml@refs/heads/main"),
            &tracked()
        ),
        Ok(".github/workflows/mnemosyne-validate.yml".to_string()),
        "the middle of `owner/repo/<path>@<ref>` is the path"
    );
    // A BRANCH NAME MAY HOLD A `/`, and the ref is everything after the FIRST
    // `@` — a workflow path holds neither, which is what makes the cut sound.
    assert_eq!(
        ci_plan::workflow_of_reference(
            Some("o/r/.github/workflows/evidence-replay.yml@refs/heads/feature/x"),
            &tracked()
        ),
        Ok(".github/workflows/evidence-replay.yml".to_string())
    );
}

#[test]
fn a_reference_naming_a_workflow_this_repository_does_not_track_is_refused() {
    // A CUT STRING IS NOT A JOIN KEY, and this is the half the census gate never
    // had. A path nothing tracks compares unequal to every cache declaration's
    // source, so a gate that merely cut would report every job in the repository
    // as belonging to some other workflow — a verdict shaped like a reading with
    // nothing behind it. The census gate's own failure is louder and just as
    // wrong: it would load a file that is not there.
    let refused = ci_plan::workflow_of_reference(
        Some("o/r/.github/workflows/gone.yml@refs/heads/main"),
        &tracked(),
    )
    .expect_err("a workflow nothing tracks");
    // THE PARSED PATH AND NOT THE REFERENCE. The message quotes both, and an
    // injection that stopped skipping `owner/repo/` came back green against an
    // assertion that looked only for `gone.yml` — which is in the input, so it is
    // in the message however badly the input was read. What distinguishes a
    // reader that got there is the path it says it arrived at.
    assert!(
        refused.contains("\".github/workflows/gone.yml\""),
        "the refusal says which workflow it read the reference AS: {refused}"
    );
    assert!(
        refused.contains("mnemosyne-validate.yml"),
        "and it names what IS tracked, so the reader can see the join it failed: \
         {refused}"
    );
}

#[test]
fn a_reference_that_is_not_there_at_all_is_refused_rather_than_guessed() {
    for absent in [None, Some(""), Some("   ")] {
        let refused = ci_plan::workflow_of_reference(absent, &tracked())
            .expect_err("nothing to join on, so there is nothing to answer");
        assert!(
            refused.contains(ci_plan::WORKFLOW_VARIABLE),
            "the refusal names the variable that was not set: {refused}"
        );
    }
}

/// Every job this repository's CI runs says what it is allowed to take.
///
/// THE BUDGET NOBODY CHOSE IS 360 MINUTES. A job with no `timeout-minutes` gets
/// GitHub's default, which is six hours — long enough that a job stuck on
/// somebody else's server burns most of an afternoon before anything says so,
/// and quiet enough that nobody notices it is there. R1229 changed the work of a
/// job's longest step and left its number alone; the first run answered with a
/// cancellation, and nothing between the edit and that answer had an opinion.
///
/// AND EVERY JOB'S CHECK NAME IS ITS OWN, which is the half a reader needs NEXT:
/// what a commit's answer carries is the `name:` a job declares, or its id when
/// it declares none — this repository's `validate` is the one job with no name of
/// its own. Two jobs a commit cannot tell apart are two jobs nothing can hold to
/// their own budgets, and the clash would be silent: GitHub prints both rows
/// under one name and a join keyed on it takes whichever came first.
#[test]
fn every_job_says_what_it_is_allowed_to_take_and_is_called_something_of_its_own() {
    let root = repository_root();
    let jobs = ci_plan::workflow_job_budgets(&root);

    // NON-VACUITY FIRST: a walk that found no jobs holds every job it found to
    // the rule, which is also what it would do the day the `jobs:` key moved.
    assert!(
        jobs.len() > 5,
        "this repository's workflows declare {} job(s), which is a walk that \
         stopped reading rather than a CI that emptied",
        jobs.len()
    );

    let unbounded: Vec<String> = jobs
        .iter()
        .filter(|(_, job)| job.timeout.is_none())
        .map(|(file, job)| format!("{file} job `{}`", job.id))
        .collect();
    assert!(
        unbounded.is_empty(),
        "{} job(s) declare no `timeout-minutes`, so GitHub gives them its own \
         360-minute default — a budget nobody in this repository chose, and one \
         nothing can be held to:\n  {}",
        unbounded.len(),
        unbounded.join("\n  ")
    );

    let mut seen: BTreeSet<(&str, &str)> = BTreeSet::new();
    let mut clashing = Vec::new();
    for (file, job) in &jobs {
        if !seen.insert((file.as_str(), job.check_name())) {
            clashing.push(format!("{file} shows two jobs as `{}`", job.check_name()));
        }
    }
    assert!(
        clashing.is_empty(),
        "{} check name(s) belong to more than one job of the same workflow, so a \
         commit's answer cannot say which job a row is:\n  {}",
        clashing.len(),
        clashing.join("\n  ")
    );
}

// --- the sixth source, pinned against strings -------------------------------

use ci_plan::rust::{spawns_in, Declared, Program, RustSpawn, Word};

fn spawns(text: &str) -> Vec<RustSpawn> {
    spawns_in("a/fixture.rs", text, "a")
}

fn only(text: &str) -> RustSpawn {
    let found = spawns(text);
    assert_eq!(found.len(), 1, "{found:#?}");
    found.into_iter().next().expect("one")
}

#[test]
fn a_cargo_spawn_beside_the_door_is_seen_however_it_names_cargo() {
    // Four spellings, and every one of them was in this tree before R1262.
    for text in [
        r#"fn f() { Command::new("cargo").arg("build"); }"#,
        r#"fn f() { Command::new(cargo()).arg("build"); }
           fn cargo() -> String { std::env::var("CARGO").unwrap() }"#,
        r#"fn f() { Command::new(std::env::var("CARGO").unwrap()).arg("build"); }"#,
        r#"fn f() { let c = the_cargo_running_this(); Command::new(&c).arg("build"); }
           fn the_cargo_running_this() -> String { std::env::var("CARGO").unwrap() }"#,
    ] {
        let site = only(text);
        assert!(
            matches!(site.program, Program::CargoBesideTheDoor(_)),
            "{text}\n{site:#?}"
        );
    }
}

#[test]
fn a_binary_this_workspace_builds_is_not_mistaken_for_cargo() {
    // `CARGO_BIN_EXE_…` holds the letters a reader hunting for `CARGO` matches,
    // and there are more than a hundred of these spawns in this repository: one
    // filed under the wrong program is a law asking a binary about lockfiles.
    let site = only(r#"fn f() { Command::new(env!("CARGO_BIN_EXE_mn")).arg("x"); }"#);
    assert!(matches!(site.program, Program::OurBinary(_)), "{site:#?}");
    // And one hop away, which is how nearly all of them are written.
    let site = only(
        r#"fn f() { Command::new(cli()).arg("x"); }
           fn cli() -> String { env!("CARGO_BIN_EXE_mn").to_string() }"#,
    );
    assert!(matches!(site.program, Program::OurBinary(_)), "{site:#?}");
}

#[test]
fn a_program_named_by_a_parameter_is_not_resolved_through_a_function_of_the_same_name() {
    // THE FIRST RUN'S OWN DEFECT. `Command::new(program)` inside a function
    // taking `program` resolved through `fn program()` in the same file, and the
    // one site that IS the door read as a second one. A bare name is a local or a
    // parameter; only a CALL names a function.
    let site = only(
        r#"fn spawn(program: String) { Command::new(program).arg("x"); }
           fn program() -> String { std::env::var("CARGO").unwrap() }"#,
    );
    assert!(matches!(site.program, Program::Unplaceable(_)), "{site:#?}");
}

#[test]
fn the_declaration_is_read_beside_the_words_and_a_variant_nobody_knows_is_refused() {
    let site = only(r#"fn f() { issue::cargo(Tree::ThisRepository).arg("build"); }"#);
    assert_eq!(site.program, Program::Cargo(Declared::ThisRepository));

    let site = only(r#"fn f() { issue::cargo(Tree::MadeByThisRun("a fixture")).arg("build"); }"#);
    assert_eq!(
        site.program,
        Program::Cargo(Declared::MadeByThisRun("a fixture".to_string()))
    );

    // `named_cargo` takes the declaration SECOND, which is why the reader finds
    // it by what it is rather than by where it sits.
    let site = only(
        r#"fn f() { issue::named_cargo(chosen, Tree::MadeByThisRun("a pin")).arg("build"); }"#,
    );
    assert_eq!(
        site.program,
        Program::Cargo(Declared::MadeByThisRun("a pin".to_string()))
    );

    // A variant this reader does not know, and a declaration decided at runtime,
    // are both `Unreadable` — the direction a new `Tree` arm has to fail in.
    for text in [
        r#"fn f() { issue::cargo(Tree::SomethingNew).arg("build"); }"#,
        r#"fn f() { issue::cargo(whichever).arg("build"); }"#,
    ] {
        let site = only(text);
        assert!(
            matches!(site.program, Program::Cargo(Declared::Unreadable(_))),
            "{text}\n{site:#?}"
        );
    }
}

#[test]
fn the_door_is_the_door_however_its_module_is_spelled() {
    // `use ci_plan::issue::cargo;` and then `cargo(Tree::ThisRepository)` is the
    // same door with one word less. Reading only the qualified path makes that
    // command INVISIBLE rather than refused, which is the one direction this
    // reader exists to rule out — and no site in the tree is written that way
    // today, so only a fixture can hold the case.
    let site = only(r#"fn f() { cargo(Tree::ThisRepository).arg("build"); }"#);
    assert_eq!(site.program, Program::Cargo(Declared::ThisRepository));

    // AND WHAT TELLS IT FROM THE HELPER THIS REPOSITORY USED TO CARRY: that one
    // took no argument. A bare `cargo()` is a program name, not a door.
    let site = only(
        r#"fn f() { Command::new(cargo()).arg("build"); }
           fn cargo() -> String { std::env::var("CARGO").unwrap() }"#,
    );
    assert!(
        matches!(site.program, Program::CargoBesideTheDoor(_)),
        "{site:#?}"
    );
}

#[test]
fn an_argument_added_on_some_paths_only_is_read_as_neither_present_nor_absent() {
    // The reading that decides whether this reader can be trusted with a flag.
    // Present would let a command that pins every other Tuesday read as pinned;
    // absent would raise a false alarm on a command that is fine.
    let site = only(
        r#"fn f(locked: bool) {
               let mut c = issue::cargo(Tree::ThisRepository);
               c.arg("build");
               if locked { c.arg("--locked"); }
           }"#,
    );
    assert_eq!(
        site.words.iter().map(Word::rendered).collect::<Vec<_>>(),
        vec!["build".to_string(), "[--locked]?".to_string()],
        "{site:#?}"
    );
    // AND THE TWO COMMANDS IT ISSUES ARE BOTH ANSWERED. Reading it as one is
    // what forced the choice above; reading it as the set it is dissolves it.
    assert_eq!(
        site.variants(),
        Some(vec![
            vec!["build".to_string()],
            vec!["build".to_string(), "--locked".to_string()],
        ]),
        "{site:#?}"
    );

    // A COMMAND WHOLLY INSIDE ONE BRANCH IS NOT CONDITIONAL IN ITS OWN TERMS.
    // Depth is compared against where the site was opened, so this reads exactly
    // as it would at the top of the function.
    let site = only(
        r#"fn f(yes: bool) {
               if yes { issue::cargo(Tree::ThisRepository).arg("build").arg("--locked"); }
           }"#,
    );
    assert_eq!(
        site.words,
        vec![
            Word::Spelled("build".to_string()),
            Word::Spelled("--locked".to_string())
        ],
        "{site:#?}"
    );
}

#[test]
fn an_array_of_words_keeps_the_flags_a_runtime_value_sits_between() {
    // An `.args([..])` has a KNOWN LENGTH even when its elements do not, and
    // reading it as a hole would throw away every flag written right there.
    let site = only(
        r#"fn f(path: &str) {
               issue::cargo(Tree::ThisRepository)
                   .args(["run", "--locked", "--manifest-path", path]);
           }"#,
    );
    assert_eq!(
        site.words,
        vec![
            Word::Spelled("run".to_string()),
            Word::Spelled("--locked".to_string()),
            Word::Spelled("--manifest-path".to_string()),
            Word::Runtime("$path".to_string()),
        ],
        "{site:#?}"
    );

    // `.args(expr)` is the hole, and it is a hole in the COUNT: a flag may be
    // inside it, so nothing downstream may say the flag list is complete.
    let site =
        only(r#"fn f(words: Vec<String>) { issue::cargo(Tree::ThisRepository).args(&words); }"#);
    assert!(
        matches!(site.words.first(), Some(Word::Unknown(_))),
        "{site:#?}"
    );
}

#[test]
fn the_manifest_directory_cargo_hands_a_crate_is_read_from_the_file_it_is_in() {
    // `env!("CARGO_MANIFEST_DIR")` is not a runtime value to a reader that knows
    // which crate the file belongs to, and three sites in this repository write
    // their manifest path this way. Left unresolved they name no manifest, and a
    // law about lockfiles cannot say whose they resolve.
    let site = spawns_in(
        "crates/mn/tests/case.rs",
        r#"fn f() {
               issue::cargo(Tree::ThisRepository).args([
                   "run",
                   "--manifest-path",
                   concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"),
               ]);
           }"#,
        "crates/mn",
    );
    assert_eq!(
        site[0].words.last(),
        Some(&Word::Spelled("crates/mn/Cargo.toml".to_string())),
        "{site:#?}"
    );
}

#[test]
fn a_helper_that_names_itself_is_answered_rather_than_walked_forever() {
    // A cycle is a hop that never lands, and a walk that met one would hang
    // instead of reporting.
    let site = only(
        r#"fn f() { Command::new(cargo()).arg("build"); }
           fn cargo() -> String { cargo() }"#,
    );
    assert!(
        matches!(site.program, Program::CargoBesideTheDoor(_)),
        "{site:#?}"
    );
}

#[test]
fn a_command_built_over_several_statements_is_one_command() {
    // The tidier spelling of the same words. A reader that only followed chains
    // would report this site as argument-free, which is how a law comes to say a
    // command with no `--locked` in it pins nothing to worry about.
    let site = only(
        r#"fn f() {
               let mut c = issue::cargo(Tree::ThisRepository);
               c.args(["metadata", "--no-deps"]);
               c.arg("--manifest-path").arg("x/Cargo.toml");
           }"#,
    );
    assert_eq!(
        site.words,
        vec![
            Word::Spelled("metadata".to_string()),
            Word::Spelled("--no-deps".to_string()),
            Word::Spelled("--manifest-path".to_string()),
            Word::Spelled("x/Cargo.toml".to_string()),
        ],
        "{site:#?}"
    );
}

#[test]
fn two_functions_holding_a_command_each_do_not_share_one() {
    let found = spawns(
        r#"fn a() { let mut c = issue::cargo(Tree::ThisRepository); c.arg("build"); }
           fn b() { let mut c = issue::cargo(Tree::ThisRepository); c.arg("test"); }"#,
    );
    assert_eq!(found.len(), 2, "{found:#?}");
    assert_eq!(found[0].words, vec![Word::Spelled("build".to_string())]);
    assert_eq!(found[1].words, vec![Word::Spelled("test".to_string())]);
}

// --- a site with a conditional word is a set of commands ---------------------

fn paths_of(text: &str) -> Vec<Vec<String>> {
    only(text).variants().expect("the paths can be enumerated")
}

#[test]
fn words_one_branch_adds_together_are_never_separated() {
    // `if let Some(m) = … { c.arg("--manifest-path").arg(m) }` is TWO words and
    // ONE decision. Read as two decisions it enumerates a command carrying
    // `--manifest-path` with nothing after it — a command nothing runs, judged
    // as though something did.
    assert_eq!(
        paths_of(
            r#"fn f(manifest: Option<&str>) {
                   let mut c = issue::cargo(Tree::ThisRepository);
                   c.arg("metadata");
                   if let Some(m) = manifest { c.arg("--manifest-path").arg(m); }
               }"#,
        ),
        vec![
            vec!["metadata".to_string()],
            vec![
                "metadata".to_string(),
                "--manifest-path".to_string(),
                "$m".to_string(),
            ],
        ],
    );
}

#[test]
fn two_arms_of_one_match_never_appear_in_the_same_command() {
    // A `match` picks one. A reading that chose its arms independently would
    // enumerate a command carrying every arm's words at once, and none carrying
    // any — two commands this program cannot issue.
    let paths = paths_of(
        r#"fn f(which: u8) {
               let mut c = issue::cargo(Tree::ThisRepository);
               c.arg("run");
               match which {
                   0 => { c.arg("--first"); }
                   _ => { c.arg("--second"); }
               }
           }"#,
    );
    assert_eq!(
        paths,
        vec![
            vec!["run".to_string(), "--first".to_string()],
            vec!["run".to_string(), "--second".to_string()],
        ],
        "{paths:?}"
    );
}

#[test]
fn an_if_with_an_else_never_takes_neither_arm() {
    let paths = paths_of(
        r#"fn f(pin: bool) {
               let mut c = issue::cargo(Tree::ThisRepository);
               c.arg("build");
               if pin { c.arg("--locked"); } else { c.arg("--offline"); }
           }"#,
    );
    assert_eq!(
        paths,
        vec![
            vec!["build".to_string(), "--locked".to_string()],
            vec!["build".to_string(), "--offline".to_string()],
        ],
        "{paths:?}"
    );
}

#[test]
fn a_word_inside_a_loop_is_one_the_command_may_not_carry() {
    // A body that runs zero times adds nothing, so "neither" is one of the ways
    // this goes — unlike a `match`, where every way is an arm.
    let paths = paths_of(
        r#"fn f(packages: &[&str]) {
               let mut c = issue::cargo(Tree::ThisRepository);
               c.arg("check");
               for p in packages { c.arg("-p").arg(p); }
           }"#,
    );
    assert_eq!(
        paths,
        vec![
            vec!["check".to_string()],
            vec!["check".to_string(), "-p".to_string(), "$p".to_string()],
        ],
        "{paths:?}"
    );
}

#[test]
fn a_word_inside_two_branches_needs_both_of_them() {
    // The inner choice is not free of the outer one: there is no command
    // carrying `--offline` without `--locked` here, and enumerating one would
    // be a verdict about a path the function has no way to take.
    let paths = paths_of(
        r#"fn f(pin: bool, quiet: bool) {
               let mut c = issue::cargo(Tree::ThisRepository);
               c.arg("build");
               if pin {
                   c.arg("--locked");
                   if quiet { c.arg("--offline"); }
               }
           }"#,
    );
    assert_eq!(
        paths,
        vec![
            vec!["build".to_string()],
            vec!["build".to_string(), "--locked".to_string()],
            vec![
                "build".to_string(),
                "--locked".to_string(),
                "--offline".to_string()
            ],
        ],
        "{paths:?}"
    );
}

#[test]
fn a_site_handing_over_a_list_nobody_can_count_has_no_paths_to_enumerate() {
    // A hole admits any NUMBER of words, so there is nothing to enumerate — and
    // answering with the paths of the words beside it would be a command list
    // that leaves out the flag the hole may hold.
    let site = only(
        r#"fn f(argv: &[&str], pin: bool) {
               let mut c = issue::cargo(Tree::ThisRepository);
               c.args(argv);
               if pin { c.arg("--locked"); }
           }"#,
    );
    assert_eq!(site.variants(), None, "{site:#?}");
}

#[test]
fn a_site_with_more_choices_than_a_report_can_hold_says_so_rather_than_cutting_the_list() {
    // Seven independent `if`s are 128 ways, past the cap. A list cut short reads
    // exactly like a complete one, so the answer is that this reader did not
    // enumerate.
    let mut source =
        String::from("fn f(a: bool) {\n    let mut c = issue::cargo(Tree::ThisRepository);\n");
    for at in 0..7 {
        source.push_str(&format!("    if a {{ c.arg(\"--flag{at}\"); }}\n"));
    }
    source.push_str("}\n");
    let site = only(&source);
    assert_eq!(site.variants(), None, "{site:#?}");
}

// --- the hop in the other direction -----------------------------------------

fn carried_by(text: &str) -> RustSpawn {
    let site = only(text);
    assert!(!site.holes.is_empty(), "no hole to follow: {site:#?}");
    site
}

fn words_read(site: &RustSpawn) -> Vec<Vec<String>> {
    site.from_callers
        .iter()
        .filter_map(|caller| caller.words.as_ref())
        .map(|words| words.iter().map(Word::rendered).collect())
        .collect()
}

#[test]
fn a_wrappers_words_are_read_at_the_call_sites_that_wrote_them() {
    // The shape R1262 wrote down as a limit: the words are not at the spawn,
    // they are at every call, and each call is a different cargo command.
    let site = carried_by(
        r#"fn one() { run(&["metadata", "--no-deps"]); }
           fn two() { run(&["check", "--locked"]); }
           fn run(argv: &[&str]) { issue::cargo(Tree::ThisRepository).args(argv); }"#,
    );
    assert_eq!(site.unfollowed, None, "{site:#?}");
    assert!(site.every_call_read(), "{site:#?}");
    assert_eq!(
        words_read(&site),
        vec![
            vec!["metadata".to_string(), "--no-deps".to_string()],
            vec!["check".to_string(), "--locked".to_string()],
        ],
        "{site:#?}"
    );
}

#[test]
fn a_call_site_in_another_file_is_followed_and_reads_its_own_crates_manifest_dir() {
    // THE HOP CROSSES FILES, which is how a wrapper in a library and the
    // literals its callers write are actually laid out — and the literal is read
    // in the CALLER's crate, so `env!("CARGO_MANIFEST_DIR")` there is the
    // caller's directory and not the wrapper's.
    let found = ci_plan::rust::spawns_across(&[
        (
            "tools/gate/src/lib.rs",
            "fn run(argv: &[&str]) { issue::cargo(Tree::ThisRepository).args(argv); }",
            "tools/gate",
        ),
        (
            "crates/user/tests/case.rs",
            r#"fn f() { run(&["metadata", concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml")]); }"#,
            "crates/user",
        ),
    ]);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert_eq!(
        words_read(&found[0]),
        vec![vec![
            "metadata".to_string(),
            "crates/user/Cargo.toml".to_string(),
        ]],
        "{found:#?}"
    );
}

#[test]
fn a_call_site_handing_over_a_value_is_counted_rather_than_guessed_at() {
    // R1190's rule where the limit is now PARTLY gone: one call site read and
    // one not is a different fact from none read, and a site that said
    // "carried" for both said neither.
    let site = carried_by(
        r#"fn one() { run(&["metadata", "--no-deps"]); }
           fn two() { let built = assemble(); run(&built); }
           fn run(argv: &[&str]) { issue::cargo(Tree::ThisRepository).args(argv); }"#,
    );
    assert_eq!(site.from_callers.len(), 2, "{site:#?}");
    assert_eq!(words_read(&site).len(), 1, "{site:#?}");
    assert!(
        site.reach().starts_with("1 of 2 call site(s) read"),
        "{}",
        site.reach()
    );
    assert!(
        !site.every_call_read(),
        "a site one caller did not finish is still carried, however many did: \
         {site:#?}"
    );
}

#[test]
fn a_call_written_inside_a_macro_is_listed_rather_than_missed() {
    // `syn` hands a macro invocation over as TOKENS and does not parse them, so
    // this call is invisible to the walk. Left out, the site would read as one
    // whose every call site was read — a partly-read population reported as a
    // whole one, by a reader that cannot see the part.
    let site = carried_by(
        r#"fn one() { assert!(run(&["metadata"]).is_ok()); }
           fn two() { let _ = run(&["check"]); }
           fn run(argv: &[&str]) -> Result<(), ()> {
               issue::cargo(Tree::ThisRepository).args(argv);
               Ok(())
           }"#,
    );
    assert_eq!(site.unfollowed, None, "{site:#?}");
    assert!(
        !site.every_call_read(),
        "a call inside a macro is a call site nobody read: {site:#?}"
    );
    assert!(
        site.reach().contains("macro"),
        "the sentence has to say which call site it could not read: {}",
        site.reach()
    );
}

#[test]
fn a_method_call_inside_a_macro_is_not_read_as_a_call_of_a_free_function() {
    // `x.run(..)` inside a macro is the receiver's `run`, and the tokens say so
    // with the dot in front of it.
    let site = carried_by(
        r#"fn one() { assert!(gate.run(&["metadata"]).is_ok()); }
           fn run(argv: &[&str]) { issue::cargo(Tree::ThisRepository).args(argv); }"#,
    );
    let why = site.unfollowed.clone().expect("a refusal");
    assert!(
        why.contains("nothing in this repository calls `run`"),
        "{why}"
    );
}

#[test]
fn an_associated_functions_parameters_are_not_followed_to_a_bare_call_of_that_name() {
    // The bare `make(..)` below is SOMEBODY ELSE'S — an import, a macro's
    // expansion, a function this tree does not define at all. An associated
    // function is never called that way (`G::make(..)` is, and a type before the
    // name is refused), so following its parameter would hand the wrapper words
    // that nothing handed it: a command in the population that nothing runs.
    let found = ci_plan::rust::spawns_across(&[
        (
            "a/one.rs",
            r#"struct G;
               impl G {
                   fn make(argv: &[&str]) { issue::cargo(Tree::ThisRepository).args(argv); }
               }"#,
            "a",
        ),
        (
            "a/two.rs",
            r#"use other::make;
               fn caller() { make(&["build"]); }"#,
            "a",
        ),
    ]);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].from_callers.is_empty(), "{found:#?}");
    let why = found[0].unfollowed.clone().expect("a refusal");
    assert!(why.contains("not a parameter"), "{why}");
}

#[test]
fn a_name_that_means_two_functions_is_not_followed_back() {
    // The forward hop's rule pointed backwards: `run` here is two functions of
    // two arguments, so a call of it names neither, and attributing one
    // caller's words to the wrong wrapper would invent a command nothing runs.
    let found = ci_plan::rust::spawns_across(&[
        (
            "a/one.rs",
            r#"fn caller() { run(1, &["metadata"]); }
               fn run(_n: u8, argv: &[&str]) { issue::cargo(Tree::ThisRepository).args(argv); }"#,
            "a",
        ),
        (
            "a/two.rs",
            "fn run(_n: u8, argv: &[&str]) { let _ = argv; }",
            "a",
        ),
    ]);
    assert_eq!(found.len(), 1, "{found:#?}");
    let why = found[0].unfollowed.clone().expect("a refusal");
    assert!(
        why.contains("2 function(s) of 2 argument(s)"),
        "the refusal has to say WHICH refusal it is: {why}"
    );
    assert!(found[0].from_callers.is_empty(), "{found:#?}");
}

#[test]
fn an_associated_function_is_not_a_call_of_a_free_function_of_that_name() {
    // `Command::new(..)`, `Path::new(..)`: a type before the name means the
    // receiver decides which `new` runs, and a reader matching the last segment
    // would hand a wrapper words nobody handed it.
    let site = carried_by(
        r#"fn caller() { let _ = Thing::run(&["build"]); }
           fn run(argv: &[&str]) { issue::cargo(Tree::ThisRepository).args(argv); }"#,
    );
    let why = site.unfollowed.clone().expect("a refusal");
    assert!(why.contains("nothing in this repository calls"), "{why}");
}

#[test]
fn a_hole_that_is_not_a_parameter_is_not_followed_back() {
    // A field of a value the run computed. No call site's literal answers it,
    // and the refusal says which expression it could not read.
    let site = carried_by(
        r#"fn caller() { run(&Asked::default()); }
           fn run(asked: &Asked) { issue::cargo(Tree::ThisRepository).args(&asked.words); }"#,
    );
    let why = site.unfollowed.clone().expect("a refusal");
    assert!(
        why.contains("$asked.words") && why.contains("not a parameter"),
        "{why}"
    );
}

#[test]
fn a_parameter_a_local_has_taken_over_is_not_followed_back() {
    // The name still reads like the parameter and is not it any more. Following
    // it back would answer with words that never reach this spawn.
    let site = carried_by(
        r#"fn caller() { run(&["metadata"]); }
           fn run(argv: &[&str]) {
               let argv = assemble(argv);
               issue::cargo(Tree::ThisRepository).args(argv);
           }"#,
    );
    let why = site.unfollowed.clone().expect("a refusal");
    assert!(why.contains("not a parameter"), "{why}");
}

#[test]
fn a_parameter_a_closure_has_taken_over_is_not_followed_back() {
    let site = carried_by(
        r#"fn caller() { run(&["metadata"]); }
           fn run(argv: &[&str]) {
               each(|argv| { issue::cargo(Tree::ThisRepository).args(argv); });
           }"#,
    );
    let why = site.unfollowed.clone().expect("a refusal");
    assert!(why.contains("not a parameter"), "{why}");
}

#[test]
fn a_word_added_on_some_paths_only_stops_the_hop_rather_than_being_filled_in() {
    // No caller's literal says whether the flag is there, so the site is exactly
    // as unreadable as it was — and the sentence says which of the two holes it
    // is, because they are different pieces of work.
    let site = carried_by(
        r#"fn caller() { run(&["metadata"], true); }
           fn run(argv: &[&str], pin: bool) {
               let mut c = issue::cargo(Tree::ThisRepository);
               c.args(argv);
               if pin { c.arg("--locked"); }
           }"#,
    );
    let why = site.unfollowed.clone().expect("a refusal");
    assert!(
        why.contains("--locked") && why.contains("some paths"),
        "{why}"
    );
}

#[test]
fn a_method_hands_over_no_parameter_this_reader_follows() {
    // `x.run(..)` is a call whose receiver decides which `run`, and this reader
    // enumerates the calls written `run(..)`. So a method's parameters are not
    // followed at all rather than followed to whatever shares the name.
    let site = carried_by(
        r#"struct G;
           impl G {
               fn run(&self, argv: &[&str]) { issue::cargo(Tree::ThisRepository).args(argv); }
           }"#,
    );
    let why = site.unfollowed.clone().expect("a refusal");
    assert!(why.contains("not a parameter"), "{why}");
}

#[test]
fn a_wrapper_nothing_calls_says_that_rather_than_reading_as_clean() {
    let site =
        carried_by(r#"fn run(argv: &[&str]) { issue::cargo(Tree::ThisRepository).args(argv); }"#);
    let why = site.unfollowed.clone().expect("a refusal");
    assert!(
        why.contains("nothing in this repository calls `run`"),
        "{why}"
    );
}

#[test]
fn two_lists_handed_over_are_refused_rather_than_split_between_the_parameters() {
    let site = carried_by(
        r#"fn caller() { run(&["metadata"], &["--no-deps"]); }
           fn run(first: &[&str], second: &[&str]) {
               issue::cargo(Tree::ThisRepository).args(first).args(second);
           }"#,
    );
    let why = site.unfollowed.clone().expect("a refusal");
    assert!(why.contains("2 lists this reader cannot count"), "{why}");
}

#[test]
fn the_words_a_caller_writes_keep_their_place_among_the_sites_own() {
    // The hole is spliced where it sat, so a flag the wrapper adds AFTER the
    // caller's words stays after them — a command whose words were reordered is
    // a different command.
    let site = carried_by(
        r#"fn caller() { run(&["check", "-q"]); }
           fn run(argv: &[&str]) {
               issue::cargo(Tree::ThisRepository)
                   .arg("--offline")
                   .args(argv)
                   .arg("--message-format=json");
           }"#,
    );
    let whole: Vec<Vec<String>> = site
        .from_callers
        .iter()
        .filter_map(|caller| site.words_from(caller))
        .collect();
    assert_eq!(
        whole,
        vec![vec![
            "--offline".to_string(),
            "check".to_string(),
            "-q".to_string(),
            "--message-format=json".to_string(),
        ]],
        "{site:#?}"
    );
}

#[test]
fn a_call_site_writing_its_list_as_a_vec_is_read_like_one_writing_an_array() {
    // `vec![..]` and `[..]` are the same words to cargo, and a reader that knew
    // one of them would report half the call sites as unreadable.
    let site = carried_by(
        r#"fn caller() { run(vec!["metadata", "--no-deps"]); }
           fn run(argv: Vec<&str>) { issue::cargo(Tree::ThisRepository).args(argv); }"#,
    );
    assert_eq!(
        words_read(&site),
        vec![vec!["metadata".to_string(), "--no-deps".to_string()]],
        "{site:#?}"
    );
}

/// The sixth place a cargo command is written, read where it is written.
///
/// A CENSUS BESIDE THE LAW, and the numbers are half of it: the walk answers
/// with its own reach — files parsed, spawns seen — beside its findings, because
/// a finding list alone cannot tell a repository that spawns nothing from a walk
/// that read nothing.
#[test]
fn every_cargo_spawn_in_tracked_rust_is_placed() {
    let root = repository_root();
    let found = ci_plan::rust::cargo_commands(&root);
    println!(
        "[rust-spawns] {} file(s), {} spawn(s): {} read ({} of them at a call \
         site one hop back), {} carried, {} beside the door, {} unplaceable",
        found.files,
        found.spawns,
        found.commands.len(),
        found.through_a_wrapper,
        found.carried.len(),
        found.beside_the_door.len(),
        found.unplaceable.len()
    );
    for command in &found.commands {
        println!(
            "  READ        {} — {} [{}]",
            command.origin(),
            command.rendered(),
            match &command.declared {
                Some(ci_plan::rust::Declared::ThisRepository) => "this repository".to_string(),
                Some(
                    ci_plan::rust::Declared::MadeByThisRun(why)
                    | ci_plan::rust::Declared::WhereverTheCallerPoints(why)
                    | ci_plan::rust::Declared::PinnedWhereverItPoints(why)
                    | ci_plan::rust::Declared::PinnedWhenItIsOurs(why)
                    | ci_plan::rust::Declared::Unreadable(why),
                ) => why.clone(),
                None => "undeclared".to_string(),
            }
        );
    }
    for site in &found.carried {
        println!("  CARRIED     {} — {}", site.origin(), site.rendered());
        // WHY IT IS STILL CARRIED, because "carried" is now several different
        // pieces of work wearing one word: a name that means two things, a
        // conditional flag, a value assembled two lines earlier, a call site
        // nobody wrote a literal at.
        println!("              {}", site.reach());
    }
    for site in &found.beside_the_door {
        println!("  SECOND DOOR {} — {}", site.origin(), site.rendered());
    }
    for site in &found.unplaceable {
        println!("  UNPLACEABLE {} — {}", site.origin(), site.rendered());
    }
    assert!(
        found.files > 100,
        "this repository tracks more than a hundred Rust files, so a walk that \
         parsed {} stopped reading",
        found.files
    );
}
