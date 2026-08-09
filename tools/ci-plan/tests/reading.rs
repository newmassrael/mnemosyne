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
    LockVerdict, Ownership,
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

/// One cache step, so the three ways a fallback fails can each be taken. This
/// repository has none of them — which is exactly why they are written here and
/// not left to the tree to demonstrate.
fn one_cache(key: &str, restore_keys: &str) -> CacheDeclaration {
    let with_restore = if restore_keys.is_empty() {
        String::new()
    } else {
        format!("          restore-keys: |\n            {restore_keys}\n")
    };
    let yaml = format!(
        "jobs:\n  j:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/cache@v6\n\
         \x20       with:\n          path: |\n            target\n          key: {key}\n{with_restore}"
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
    let root = repository_root();
    let declared = ci_plan::workflow_cache_declarations(&root);
    let broken: Vec<String> = declared
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
        declared.len() >= 2 && declared.iter().all(|d| !d.restore_keys.is_empty()),
        "and the reach is asserted, because a walk that found no declaration \
         passes the loop above without looking at anything: {declared:?}"
    );
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
    assert_eq!(parsed[0].0[0], "cargo", "{parsed:?}");
    assert_eq!(parsed[0].0[1], "test", "{parsed:?}");
    assert_eq!(parsed[1].0[1], "build", "{parsed:?}");
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
        cargo_args: words,
        harness_args,
        env: Default::default(),
    }
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
