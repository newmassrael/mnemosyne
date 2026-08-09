//! The reading rules, pinned against strings rather than against the tree.
//!
//! Both sides of every gate built on this crate come from these two parses, and
//! the branches that matter most are the ones THIS MACHINE NEVER TAKES: a `SKIP`
//! line only appears where a sibling checkout is absent, which is a CI runner
//! and not the machine this is written on. R1082's gate had no such branch and
//! turned main red on its first push. Pinned text is how a branch nobody here
//! can execute still has a control.

use ci_plan::{
    cache_steps, job_needs, lister_cargo_commands, parse_lister, parse_script, parse_workflow,
    run_steps, CacheDeclaration, CargoCommand,
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
      - uses: actions/cache/restore@v6
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
      - uses: actions/cache@v6
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
      - uses: actions/cache@v6
        with:
          path: target
          key: ${{ runner.os }}-cargo-
"#,
    );
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].prefix, "Linux-cargo-");
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

fn repository_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("this crate lives two directories below the repository root")
        .to_path_buf()
}

/// The lister's output as `scripts/check-side-workspaces.sh --list` prints it,
/// including the skip this machine cannot produce.
const LISTED: &str = "[side-workspaces] CHECKABLE bench\n\
     [side-workspaces] SUITE bench cargo test --manifest-path bench/Cargo.toml --locked --no-fail-fast\n\
     [side-workspaces] SKIP studio — its path dependencies leave this \
     repository and are not on this machine: ../pinion/crates/pinion-a11y\n\
     [side-workspaces] CHECKABLE tools/item-citations\n\
     [side-workspaces] SUITE tools/item-citations cargo test --manifest-path tools/item-citations/Cargo.toml --locked --no-fail-fast\n\
     [side-workspaces] checked 2 (bench tools/item-citations), skipped 1 (studio)\n";

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
        listed.suites.keys().collect::<Vec<_>>(),
        vec!["bench", "tools/item-citations"],
        "only the checkable ones have a suite: {:?}",
        listed.suites
    );
    let commands = lister_cargo_commands(&listed);
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

#[test]
fn a_shell_line_is_split_where_cargo_stops_and_the_harness_starts() {
    let read = |script: &str| {
        parse_script(script)
            .into_iter()
            .map(|(cargo_args, harness_args)| CargoCommand {
                source: "pinned".to_string(),
                owner: "pinned".to_string(),
                cargo_args,
                harness_args,
                env: Default::default(),
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
