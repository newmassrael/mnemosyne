//! What the gate refuses, and what it counts.
//!
//! Pinned against records rather than against a build, for the reason
//! `ci-plan`'s own suite is: the branches that matter are the ones this machine
//! does not take. A job whose recorder is unwired, a log from a job that was
//! deleted, two jobs sharing a unit — none of those exist in this repository on
//! the day this is written, and every one of them is what the gate is for.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use ci_plan::RunStep;
use twice_compiled::{
    judge, names_its_job, read, read_log, unresolvable, Census, Cost, Declared, Invocation, JobLog,
    Origin, Refusal, Unit, WRAPPER_VARIABLE,
};

/// The jobs of a fixture workflow, NONE of which declares a cache.
///
/// Said once here rather than at each call below, because it is a property of
/// every fixture in this file: the laws in it are about what a job compiled and
/// how that was recorded, and the laws about what a job RESTORED have their own
/// fixtures further down, which declare caches on purpose.
fn declared_jobs(steps: &[RunStep]) -> Declared {
    declared_of(steps, &[])
}

/// `Declared::of` over a workflow that COLLECTS its records.
///
/// THE ORDINARY CASE, AND THE ONE EVERY LAW IN THIS FILE IS ABOUT. A workflow
/// that uploads no artifact leaves nothing behind when its runners are
/// destroyed, so its jobs owe no restore record at all — that derivation has its
/// own tests, and a fixture silently on the wrong side of it would make every
/// law below vacuous.
fn declared_of(steps: &[RunStep], caches: &[ci_plan::CacheDeclaration]) -> Declared {
    Declared::of(steps, caches, &[collecting("validate")])
}

/// One `actions/upload-artifact` step, which is what makes a record readable.
fn collecting(job: &str) -> ci_plan::ArtifactUpload {
    ci_plan::ArtifactUpload {
        source: "fixture.yml".to_string(),
        owner: job.to_string(),
        index: 9,
        paths: vec!["rustc-log/".to_string()],
    }
}

/// A record as `rustc-log` writes it: when the compiler started, how long it
/// ran, then the compiler and its arguments.
fn record(started_at: u64, micros: u64, words: &[&str]) -> String {
    let written = rustc_log::Record {
        started_at,
        micros,
        argv: words.iter().map(|word| word.to_string()).collect(),
    };
    String::from_utf8(rustc_log::encode(&written)).expect("records are utf-8")
}

/// When the fixture clocks start. Any epoch does; a round number makes a failing
/// assertion readable.
const EPOCH: u64 = 1_000_000;

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

/// One compilation's record, placed on the clock.
fn compiled(started_at: u64, micros: u64, crate_name: &str, metadata: &str, emit: &str) -> String {
    let argv = compilation(crate_name, metadata, emit);
    let words: Vec<&str> = argv.iter().map(String::as_str).collect();
    record(started_at, micros, &words)
}

/// A job's log, built from units laid END TO END on the clock: one compiler at a
/// time, so the job's window is exactly its work. The tests that are about
/// several compilers at once say so by building their records by hand.
fn log_of(units: &[(&str, &str, &str, u64)]) -> JobLog {
    let mut clock = EPOCH;
    let text: String = units
        .iter()
        .map(|(name, metadata, emit, micros)| {
            let line = compiled(clock, *micros, name, metadata, emit);
            clock += micros;
            line
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
    let Invocation::Compilation(compiled) = read(&argv) else {
        panic!("that is a compilation: {argv:?}");
    };
    assert_eq!(
        compiled.unit,
        Unit {
            crate_name: "mnemosyne_core".to_string(),
            metadata: "469b061dbaa2f2d3".to_string(),
            emit: "dep-info,metadata,link".to_string(),
            crate_types: vec!["lib".to_string()],
            test: false,
            driver: "/home/runner/.rustup/toolchains/stable/bin/rustc".to_string(),
            origin: twice_compiled::Origin::Tree,
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

// --- whose source, and which compiler -----------------------------------------

/// One unit's arguments with the input file named, which `compilation` above
/// does not vary because the laws it serves are about the key.
fn compiling(crate_name: &str, metadata: &str, input: &str) -> Vec<String> {
    vec![
        TOOLCHAIN.to_string(),
        "--crate-name".to_string(),
        crate_name.to_string(),
        "--edition=2021".to_string(),
        input.to_string(),
        "--emit=dep-info,metadata".to_string(),
        "-C".to_string(),
        format!("metadata={metadata}"),
    ]
}

/// The compiler the fixtures above are compiled by.
const TOOLCHAIN: &str = "/home/runner/.rustup/toolchains/1.94.1-x86_64-unknown-linux-gnu/bin/rustc";

/// Where a compilation reading this file is placed.
fn origin_of(input: &str) -> Origin {
    let argv = compiling("serde", "abcd", input);
    let Invocation::Compilation(compiled) = read(&argv) else {
        panic!("that is a compilation: {argv:?}");
    };
    assert_eq!(
        compiled.unit.origin, compiled.into.origin,
        "the two axes read the same source and must not come to two answers"
    );
    compiled.unit.origin
}

#[test]
fn a_crate_cargo_fetched_is_told_from_one_the_checkout_holds() {
    assert_eq!(
        origin_of(
            "/home/runner/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/\
             serde-1.0.219/src/lib.rs"
        ),
        Origin::Registry
    );
    assert_eq!(origin_of("crates/mnemosyne-core/src/lib.rs"), Origin::Tree);
    assert_eq!(
        origin_of("/home/runner/work/mnemosyne/mnemosyne/tools/ci-plan/src/lib.rs"),
        Origin::Tree
    );
    assert!(
        Origin::Registry.fetched() && !Origin::Tree.fetched(),
        "the trees every cache in this workflow carries are the fetched ones, \
         and that predicate is what a reader asks of a cache"
    );
}

#[test]
fn a_git_dependencys_checkout_is_fetched_as_well() {
    assert_eq!(
        origin_of("/home/runner/.cargo/git/checkouts/tonic-3a1f6c5d9b2e/9b2ef41/tonic/src/lib.rs"),
        Origin::Git
    );
    assert!(Origin::Git.fetched());
}

#[test]
fn a_directory_pair_of_ours_spelled_like_cargos_is_not_cargos() {
    // THE CONTROL THAT TELLS THE RULE FROM A SUBSTRING SEARCH. `registry/src`
    // is a pair any repository may have, and a reader that stopped there would
    // report a crate of this repository's own as a fetched dependency — in the
    // direction that answers this instrument's own question for it.
    assert_eq!(
        origin_of("/home/runner/work/mnemosyne/crates/registry/src/lib.rs"),
        Origin::Tree
    );
    // The pair AND an index directory, and still no unpacked crate below it.
    assert_eq!(origin_of("/work/registry/src/vendor/lib.rs"), Origin::Tree);
}

#[test]
fn the_pass_cargo_clippy_makes_is_read_through_the_chain_that_ran_it() {
    // `cargo clippy` sets `RUSTC_WORKSPACE_WRAPPER`, so for every workspace
    // member cargo runs `<recorder> <clippy-driver> <rustc> <arguments…>` and
    // the record carries TWO program paths ahead of the first flag. A reader
    // that took the first word after the recorder's own for the input file
    // finds two free-standing words and can place neither — which is how 153
    // compilations of a real eight-job census went missing from the split.
    let mut argv = vec![
        "/home/runner/.rustup/toolchains/1.94.1-x86_64-unknown-linux-gnu/bin/clippy-driver"
            .to_string(),
        TOOLCHAIN.to_string(),
    ];
    argv.extend(
        compiling(
            "grpc_smoke",
            "d00d",
            "crates/mnemosyne-server/tests/grpc_smoke.rs",
        )
        .into_iter()
        .skip(1),
    );
    let Invocation::Compilation(compiled) = read(&argv) else {
        panic!("a clippy pass is a compilation this CI pays for: {argv:?}");
    };
    assert_eq!(compiled.unit.origin, Origin::Tree);
    assert_eq!(
        compiled.unit.driver,
        "/home/runner/.rustup/toolchains/1.94.1-x86_64-unknown-linux-gnu/bin/clippy-driver",
        "the program the recorder ran is the one that did the work, and it is \
         not the `rustc` it was handed to pass along"
    );
}

#[test]
fn two_compilations_that_differ_only_in_which_compiler_ran_are_two_units() {
    // WHY THE DRIVER IS IN THE KEY, and it is the reason the fields beside it
    // are given: cargo's `-C metadata` hashes the compiler today, and this
    // reader must split rather than merge if a release ever narrows that.
    let source = "crates/mnemosyne-core/src/lib.rs";
    let stable = read(&compiling("mnemosyne_core", "469b", source));
    let mut older = compiling("mnemosyne_core", "469b", source);
    older[0] =
        "/home/runner/.rustup/toolchains/1.88.0-x86_64-unknown-linux-gnu/bin/rustc".to_string();
    assert_ne!(
        stable,
        read(&older),
        "the MSRV job compiles the same sources with another toolchain, and \
         calling those one unit reports a duplication no merge can remove"
    );
}

#[test]
fn a_compilation_with_a_second_free_standing_word_is_not_placed() {
    let mut argv = compiling("serde", "abcd", "crates/mnemosyne-core/src/lib.rs");
    // A flag this reader does not know, in the form that takes a separated
    // value: the value stands where a path should, and nothing says which of
    // the two is the input.
    argv.push("--a-flag-this-reader-does-not-know".to_string());
    argv.push("its-value".to_string());
    let Invocation::Unplaced(what) = read(&argv) else {
        panic!("two free-standing words are not one input: {argv:?}");
    };
    assert_eq!(what.crate_name, "serde");
    assert_eq!(
        what.candidates,
        vec!["crates/mnemosyne-core/src/lib.rs", "its-value"],
        "the words are the whole of the repair — one is the path and the other \
         names the flag missing from this reader's list"
    );
    assert!(what.why().contains("its-value"));
}

#[test]
fn a_compilation_whose_input_a_flag_swallowed_is_not_placed_either() {
    // THE OTHER DIRECTION THE LIST CAN BE WRONG IN, and it is a different
    // shape: no candidate at all rather than too many. Both are refused, which
    // is what makes a list safe to keep in a crate that derives everything else.
    let argv = vec![
        TOOLCHAIN.to_string(),
        "--crate-name".to_string(),
        "serde".to_string(),
        "-C".to_string(),
        "metadata=abcd".to_string(),
        "--sysroot".to_string(),
        "crates/mnemosyne-core/src/lib.rs".to_string(),
    ];
    let Invocation::Unplaced(what) = read(&argv) else {
        panic!("no free-standing word is not one input: {argv:?}");
    };
    assert!(what.candidates.is_empty());
    assert!(
        what.why().contains("ate the input path"),
        "a reader is owed which way it was wrong: {}",
        what.why()
    );
}

#[test]
fn the_split_by_origin_accounts_for_every_compilation_the_job_counted() {
    // THE LAW THAT KEEPS THE SPLIT HONEST. A breakdown that loses a row is a
    // report where the fetched share reads smaller than it is, and nothing in
    // the totals says so — the numbers still add up, against a population one
    // short.
    // THE LAST TWO ROWS ARE ONE UNIT COMPILED TWICE, and they are what makes
    // this test able to fail. A job holding two `target` directories pays for a
    // shared crate in both, so the split must count COMPILATIONS and not the
    // rows of the unit table — and over a fixture where every unit is compiled
    // once those two readings are the same number. The injection that swaps one
    // for the other came back green until this pair was here.
    let sources = [
        ("serde", "0001", "/home/runner/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/serde-1.0.219/src/lib.rs"),
        ("tokio", "0002", "/home/runner/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tokio-1.47.1/src/lib.rs"),
        ("tonic", "0003", "/home/runner/.cargo/git/checkouts/tonic-3a1f6c5d9b2e/9b2ef41/tonic/src/lib.rs"),
        ("mnemosyne_core", "0004", "crates/mnemosyne-core/src/lib.rs"),
        ("mnemosyne_core", "0004", "crates/mnemosyne-core/src/lib.rs"),
    ];
    let mut clock = EPOCH;
    let mut text = String::new();
    for (crate_name, metadata, source) in sources {
        let argv = compiling(crate_name, metadata, source);
        let words: Vec<&str> = argv.iter().map(String::as_str).collect();
        text.push_str(&record(clock, 1_000, &words));
        clock += 1_000;
    }
    let log = read_log(&text);
    assert_eq!(
        (log.compilations(), log.units.len()),
        (5, 4),
        "the fixture must hold a repeat, or counting units and counting \
         compilations are the same number and this test cannot tell them apart"
    );
    let split = log.by_origin();
    assert_eq!(split[&Origin::Registry].times, 2);
    assert_eq!(split[&Origin::Git].times, 1);
    assert_eq!(split[&Origin::Tree].times, 2);
    assert_eq!(
        split.values().map(|cost| cost.times).sum::<usize>(),
        log.compilations(),
        "every compilation the job counted is in exactly one row of the split"
    );
    assert_eq!(
        split.values().map(|cost| cost.micros).sum::<u64>(),
        log.compiled_micros(),
        "and so is every second of it"
    );
    assert_eq!(log.fetched().times, 3);
    assert_eq!(log.fetched().micros, 3_000);
    // AND THE OTHER BREAKDOWN OF THE SAME POPULATION AGREES. `by_origin` folds
    // the unit table and `written` is folded per compilation; nothing about the
    // way either is built makes them agree by construction, and two breakdowns
    // of one population that disagree are one report nobody can act on.
    let mut crossed: BTreeMap<Origin, Cost> = BTreeMap::new();
    for (into, cost) in &log.written {
        crossed.entry(into.origin).or_default().absorb(*cost);
    }
    assert_eq!(crossed, split);
}

/// A job's log built from `(crate, metadata, source, out-dir)` rows, one
/// compilation each, laid end to end on the clock.
fn log_written(rows: &[(&str, &str, &str, &str)]) -> JobLog {
    let mut clock = EPOCH;
    let mut text = String::new();
    for (crate_name, metadata, source, out_dir) in rows {
        let mut argv = compiling(crate_name, metadata, source);
        argv.push("--out-dir".to_string());
        argv.push((*out_dir).to_string());
        let words: Vec<&str> = argv.iter().map(String::as_str).collect();
        text.push_str(&record(clock, 1_000, &words));
        clock += 1_000;
    }
    read_log(&text)
}

const REGISTRY: &str =
    "/home/runner/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/serde-1.0.219/src/lib.rs";

#[test]
fn what_a_cache_could_not_have_spared_is_told_from_what_it_missed() {
    // THE FINDING THIS EXISTS FOR. A job that restored its `target` on an exact
    // hit and still compiled hundreds of fetched crates reads as a cache that
    // failed — until somebody asks WHERE that work was written. This
    // repository's jobs build side workspaces whose `target` directories are in
    // nobody's `path:` list, and no restore of the declared ones could ever
    // have spared a compilation that landed there.
    let log = log_written(&[
        ("serde", "0001", REGISTRY, "/w/repo/target/debug/deps"),
        (
            "core",
            "0002",
            "crates/core/src/lib.rs",
            "/w/repo/target/debug/deps",
        ),
        ("tokio", "0003", REGISTRY, "/w/repo/bench/target/debug/deps"),
        (
            "bench_gen",
            "0004",
            "bench/src/lib.rs",
            "/w/repo/bench/target/debug/deps",
        ),
        (
            "rayon",
            "0005",
            REGISTRY,
            "/w/repo/tools/gate/target/debug/deps",
        ),
    ]);
    let found = log
        .coverage(Path::new("/w/repo"), &["target".to_string()])
        .expect("these destinations are under that checkout");

    let held = &found.held["target"];
    assert_eq!(held[&Origin::Registry].times, 1);
    assert_eq!(held[&Origin::Tree].times, 1);
    assert_eq!(
        found.outside.keys().collect::<Vec<_>>(),
        vec!["/w/repo/bench/target", "/w/repo/tools/gate/target"],
        "the rows name the trees, because the repair is a `path:` line and a \
         reader owed one needs to know which directory to write"
    );
    assert_eq!(
        found.outside["/w/repo/bench/target"][&Origin::Registry].times,
        1
    );
    assert_eq!(
        found.outside["/w/repo/tools/gate/target"][&Origin::Registry].times,
        1
    );
    let counted: usize = found
        .held
        .values()
        .chain(found.outside.values())
        .flat_map(BTreeMap::values)
        .map(|cost| cost.times)
        .sum();
    assert_eq!(
        counted,
        log.compilations(),
        "the two halves are one walk over one population and must sum to it"
    );
}

#[test]
fn two_declared_paths_holding_one_destination_credit_the_deeper() {
    // MAKING THE PATHS ABSOLUTE IS WHAT REMOVES MOST OF THIS QUESTION:
    // `<root>/bench/target` does not begin with `<root>/target` however much
    // the two spellings share, so a job caching sibling trees needs no tie to
    // be broken. What remains is a `path:` list whose entries NEST, where both
    // genuinely hold the destination and only one of them is the answer a
    // reader can act on. The deeper is that one, and without the ordering the
    // row is whichever the workflow happened to list first.
    let log = log_written(&[
        ("serde", "0001", REGISTRY, "/w/repo/target/debug/deps"),
        ("tokio", "0002", REGISTRY, "/w/repo/bench/target/debug/deps"),
    ]);
    let found = log
        .coverage(
            Path::new("/w/repo"),
            &["target".to_string(), "target/debug".to_string()],
        )
        .expect("these destinations are under that checkout");
    assert_eq!(
        found.held.keys().collect::<Vec<_>>(),
        vec!["target/debug"],
        "both hold it and the deeper is the row: {:?}",
        found.held
    );
    assert_eq!(
        found.outside.keys().collect::<Vec<_>>(),
        vec!["/w/repo/bench/target"],
        "and the sibling tree is under neither, which the absolute join settles \
         without any ordering at all"
    );
}

#[test]
fn a_cache_path_a_shell_would_expand_holds_no_compiler_output_here() {
    // `~/.cargo/registry` is every job's first cached path and only a shell
    // expands it. It covers nothing this reader can see, which is right — a
    // cargo home holds sources — and is said rather than left as a silence.
    let log = log_written(&[("serde", "0001", REGISTRY, "/w/repo/target/debug/deps")]);
    let found = log
        .coverage(Path::new("/w/repo"), &["~/.cargo/registry".to_string()])
        .expect("the destination is under that checkout");
    assert!(found.held.is_empty());
    assert_eq!(found.outside["/w/repo/target"][&Origin::Registry].times, 1);
}

#[test]
fn a_census_from_another_machine_is_not_read_as_a_cache_that_reaches_nothing() {
    // THE CATASTROPHIC-LOOKING NUMBER THAT IS ONLY A WRONG ROOT. An `--out-dir`
    // written on a runner begins `/home/runner/work/…`, and no path under a
    // different checkout will ever prefix it — so a reader that answered anyway
    // would report EVERY compilation as work no cache could spare.
    let log = log_written(&[(
        "serde",
        "0001",
        REGISTRY,
        "/home/runner/work/mnemosyne/mnemosyne/target/debug/deps",
    )]);
    assert!(log
        .coverage(Path::new("/home/coin/mnemosyne"), &["target".to_string()])
        .is_none());
    assert!(
        log.coverage(
            Path::new("/home/runner/work/mnemosyne/mnemosyne"),
            &["target".to_string()]
        )
        .is_some(),
        "and the same census read from the machine it was taken on resolves"
    );
}

#[test]
fn a_job_that_wrote_where_nothing_said_is_refused() {
    // A DESTINATION THIS READER NEVER SAW IS ABSENT FROM BOTH HALVES, and both
    // still sum and still print — the job simply reads as having compiled less
    // than it did, in whichever direction happens to matter.
    let mut census = census_of(&["validate", "unrun-tests"]);
    let mut text = record(
        EPOCH,
        500,
        &compiling("adrift", "beef", "crates/core/src/lib.rs")
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
    );
    text.push_str(&record(
        EPOCH + 500,
        500,
        &{
            let mut argv = compiling("placed", "d00d", "crates/core/src/lib.rs");
            argv.push("--out-dir".to_string());
            argv.push("/w/repo/target/debug/deps".to_string());
            argv
        }
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>(),
    ));
    census.jobs.insert("validate".to_string(), read_log(&text));

    let refusals = judge(
        &census,
        &declared_jobs(&[wired("validate"), wired("unrun-tests")]),
        &nothing(),
    );
    assert!(
        refusals.iter().any(|refusal| matches!(
            refusal,
            Refusal::JobWroteWhereNothingSaid { job, compilations }
                if job == "validate" && *compilations == 1
        )),
        "{refusals:?}"
    );
}

#[test]
fn a_job_that_said_where_every_compilation_went_is_not_refused_for_this() {
    // THE CONTROL, through the same judge on the same fixture shape: `census_of`
    // builds its logs from `compiled`, which names an `--out-dir` — so an
    // assertion that the refusal fires is held against a sibling that accepts.
    let refusals = judge(
        &census_of(&["validate", "unrun-tests"]),
        &declared_jobs(&[wired("validate"), wired("unrun-tests")]),
        &nothing(),
    );
    assert!(
        !refusals
            .iter()
            .any(|refusal| matches!(refusal, Refusal::JobWroteWhereNothingSaid { .. })),
        "{refusals:?}"
    );
}

#[test]
fn a_job_that_ran_a_compilation_this_reader_cannot_place_is_refused() {
    // AN EMPTY SPLIT AND AN UNREACHED ONE HAVE THE SAME SHAPE, which is the
    // failure this repository keeps meeting. A job whose fetched row reads zero
    // because nothing was fetched and one whose fetched row reads zero because
    // the reader lost the population print the same line.
    let mut argv = compiling("serde", "abcd", "crates/mnemosyne-core/src/lib.rs");
    argv.push("--a-flag-this-reader-does-not-know".to_string());
    argv.push("its-value".to_string());
    let words: Vec<&str> = argv.iter().map(String::as_str).collect();
    let mut census = census_of(&["validate", "unrun-tests"]);
    let mut text = record(EPOCH, 500, &words);
    text.push_str(&compiled(EPOCH + 500, 500, "placed", "beef", "link"));
    census.jobs.insert("validate".to_string(), read_log(&text));

    let refusals = judge(
        &census,
        &declared_jobs(&[wired("validate"), wired("unrun-tests")]),
        &nothing(),
    );
    let named: Vec<String> = refusals.iter().map(Refusal::to_string).collect();
    assert!(
        refusals.iter().any(|refusal| matches!(
            refusal,
            Refusal::JobRanCompilationsThisReaderCannotPlace { job, compilations, .. }
                if job == "validate" && *compilations == 1
        )),
        "a job whose split rests on a population the reader lost is refused: {named:?}"
    );
    assert!(
        named
            .iter()
            .any(|why| why.contains("`serde`") && why.contains("its-value")),
        "and the refusal names what was lost, so its reader is sent to a crate \
         rather than to a megabyte of records: {named:?}"
    );
}

#[test]
fn a_job_that_placed_every_compilation_is_not_refused_for_this() {
    // THE CONTROL FOR THE REFUSAL ABOVE, and it goes through the same judge on
    // the same fixture shape: an assertion that something is refused says
    // nothing until the same call accepts its sibling.
    let refusals = judge(
        &census_of(&["validate", "unrun-tests"]),
        &declared_jobs(&[wired("validate"), wired("unrun-tests")]),
        &nothing(),
    );
    assert!(
        !refusals.iter().any(|refusal| matches!(
            refusal,
            Refusal::JobRanCompilationsThisReaderCannotPlace { .. }
        )),
        "{refusals:?}"
    );
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

    let log = read_log(&record(
        EPOCH,
        400,
        &["rustc", "--crate-name", "probe0", "probe0.rs"],
    ));
    assert_eq!(
        (log.invocations, log.unkeyed, log.compilations()),
        (1, 1, 0)
    );
    assert_eq!(
        (log.micros, log.compiled_micros()),
        (400, 0),
        "its seconds are in the job's total and in no unit's price — a class \
         this reader cannot key is one it must not silently call free either"
    );
}

// --- counting a census ------------------------------------------------------

#[test]
fn what_two_jobs_both_compile_is_what_one_job_would_compile_once() {
    let mut census = Census::default();
    census.jobs.insert(
        "validate".to_string(),
        log_of(&[("serde", "aaa", "link", 30), ("core", "bbb", "link", 100)]),
    );
    census.jobs.insert(
        "unrun-tests".to_string(),
        log_of(&[("serde", "aaa", "link", 20), ("core", "ccc", "link", 100)]),
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

    let pair = census
        .pairwise()
        .get(&("unrun-tests", "validate"))
        .copied()
        .expect("the pair is named, because the repair is to merge a pair");
    assert_eq!(pair.units, 1);
    assert_eq!(
        pair.saved_micros, 20,
        "priced at the CHEAPER of the two jobs' figures for it, so the saving \
         reported is the least the merge could win"
    );

    assert_eq!(
        (census.paid_micros(), census.floor_micros()),
        (250, 230),
        "the floor prices the surviving compilation at the DEARER of the two, \
         for the same reason from the other side"
    );
}

#[test]
fn a_job_that_compiles_one_unit_twice_is_reported_as_doing_so() {
    // A job holding two `target` directories — the root one and a tool
    // workspace's — pays for a shared dependency in both. That is duplication
    // inside one job, and merging jobs cannot remove it, so it is counted
    // separately rather than folded into the cross-job number.
    let log = log_of(&[("serde", "aaa", "link", 60), ("serde", "aaa", "link", 40)]);
    assert_eq!(
        (log.compilations(), log.units.len(), log.repeats()),
        (2, 1, 1)
    );
    assert_eq!(
        (
            log.compiled_micros(),
            log.units.values().next().unwrap().each()
        ),
        (100, 50),
        "one unit, two compilations, and the price of one of them is the pair's \
         average — the record does not say which of the two was the slow one"
    );
}

#[test]
fn the_window_is_read_off_the_clocks_and_not_off_the_order_of_the_lines() {
    // RECORDS ARE APPENDED WHEN A COMPILER EXITS, and cargo runs as many as the
    // machine has cores, so the earliest start routinely arrives last: the long
    // compilation that began the job finishes after the short ones that began
    // later. A reader taking the first and last LINES would read a window
    // shorter than the job's, every time, and every job would look busier than
    // it was.
    let text = format!(
        "{}{}",
        compiled(EPOCH + 500, 100, "quick", "bbb", "link"),
        compiled(EPOCH, 900, "slow", "aaa", "link"),
    );
    let log = read_log(&text);
    assert_eq!(
        log.window(),
        Some((EPOCH, EPOCH + 900)),
        "the window is the earliest start to the latest exit"
    );
    assert_eq!(log.span_micros(), 900);
    assert_eq!(
        log.compiled_micros(),
        1000,
        "and the work is larger than the window, because two compilers overlapped"
    );
}

#[test]
fn the_part_of_a_window_with_no_compiler_alive_is_measured_and_not_assumed() {
    // WHAT MERGING CANNOT REACH INSIDE ONE JOB. A window holds the minutes a
    // suite spends running the binaries it just built, and those do not get
    // shorter because the job compiled less. Assumed away, they are scaled with
    // the compiling and the estimate promises time that is not there; assumed
    // to be large, every merge looks pointless. They are the union of the
    // intervals subtracted from the window, which the records already say.
    let text = format!(
        "{}{}{}",
        // Two compilers overlapping: busy from 0 to 150, not 250.
        compiled(EPOCH, 100, "first", "aaa", "link"),
        compiled(EPOCH + 50, 100, "second", "bbb", "link"),
        // Then nothing at all until 500 — a suite running.
        compiled(EPOCH + 500, 100, "third", "ccc", "link"),
    );
    let log = read_log(&text);
    assert_eq!(log.span_micros(), 600);
    assert_eq!(
        log.busy_micros(),
        250,
        "the UNION of the intervals: 0..150 and 500..600. Summing them would \
         report 300 µs of a 600 µs window busy, which is a job busier than the \
         clock allows"
    );
    assert_eq!(log.idle_micros(), 350);
    assert_eq!(
        log.compiled_micros(),
        300,
        "and the work is larger than the busy time, because two of them ran at \
         the same moment"
    );
}

#[test]
fn a_merge_estimate_does_not_shorten_the_time_no_compiler_was_alive() {
    // THE ASSUMPTION THIS ROUND REFUSED TO CARRY. Scaling a whole window by the
    // work removed treats a job's idle minutes as though they compiled, and the
    // two jobs with the most to share in this repository are the two that spend
    // the longest running what they built. Here each job is busy for 100 µs and
    // idle for 900, and merging removes every compilation one of them does: the
    // estimate must still be about 1800 µs, not 900.
    let mut census = Census::default();
    for job in ["validate", "unrun-tests"] {
        let text = format!(
            "{}{}",
            compiled(EPOCH, 100, "serde", "aaa", "link"),
            // A last compilation of the job's own, instant, 900 µs later: the
            // window is long and almost all of it has nothing compiling in it,
            // which is what these two jobs look like in this repository.
            compiled(EPOCH + 1000, 0, "only_mine", job, "link"),
        );
        census.jobs.insert(job.to_string(), read_log(&text));
    }

    let merge = census
        .pairwise()
        .get(&("unrun-tests", "validate"))
        .copied()
        .expect("the two jobs share `serde`");
    assert_eq!((merge.floor_micros, merge.ceiling_micros), (1000, 2000));
    assert_eq!(
        (merge.units, merge.saved_micros),
        (1, 100),
        "merging removes one of the two compilations of `serde`, and all 100 µs \
         of compiling that one did"
    );
    assert_eq!(
        merge.idle_micros, 1800,
        "900 µs of each job's window had no compiler alive in it"
    );
    assert_eq!(
        merge.estimate_micros, 1900,
        "half the pair's 200 µs of compiling survives and the 1800 µs of idle \
         carries across whole. AN ESTIMATE THAT SCALED THE WHOLE WINDOW would \
         have said 2000 x 100/200 = 1000 µs — a merged job half the length of \
         the shorter of the two it replaces, which is not a thing that happens"
    );
}

// --- the law ----------------------------------------------------------------

fn step(job: &str, env: &[(&str, &str)]) -> RunStep {
    RunStep {
        job: job.to_string(),
        // WHERE IN THE JOB IT SITS matters only to the laws about the restore
        // measurements, and the fixtures for those set it deliberately.
        index: 0,
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
                ("serde", "aaa", "link", 30),
                ("only", &format!("{index}"), "link", 70),
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
fn a_census_that_reached_fewer_than_two_jobs_is_refused() {
    // THE VACUOUS GREEN THIS GATE ALMOST SHIPPED. A local replay refused all
    // nine jobs of the workflow — for a variable it replaces itself — and the
    // census then printed `0 compilations across 0 job(s)`, every total a clean
    // zero, and signed off with "every one of the 0 job(s) recorded what it
    // compiled". Nothing was wrong with the file; nothing had been measured.
    // The subject here is what TWO jobs both compile, so fewer than two cannot
    // hold a finding and must not print as though it had looked for one.
    let declared = declared_jobs(&[wired("validate"), wired("unrun-tests"), wired("msrv")]);
    let everything: BTreeSet<String> = declared.jobs.keys().cloned().collect();
    assert_eq!(
        judge(&Census::default(), &declared, &everything),
        vec![Refusal::CensusCoversTooFewJobs { covered: 0 }],
        "a census the absent list has swallowed whole"
    );

    let mut all_but_one: BTreeSet<String> = everything.clone();
    all_but_one.remove("validate");
    assert_eq!(
        judge(&census_of(&["validate"]), &declared, &all_but_one),
        vec![Refusal::CensusCoversTooFewJobs { covered: 1 }],
        "and one job has no cross-job duplication BY CONSTRUCTION, so its zero \
         is arithmetic rather than a finding"
    );

    // The control: two covered jobs, and the same census is accepted.
    let mut mine = BTreeSet::new();
    mine.insert("msrv".to_string());
    assert!(judge(&census_of(&["validate", "unrun-tests"]), &declared, &mine).is_empty());
}

#[test]
fn a_job_is_not_skipped_for_a_variable_the_replay_sets_itself() {
    // WHAT CLOSED THE REPLAY. Every job in this workflow carries
    // `MNEMOSYNE_RUSTC_LOG: ${{ github.workspace }}/rustc-log/<job>.log`, which
    // is an expression only GitHub resolves — and which the replay overwrites
    // with a path of its own before running a single step. Reading it anyway
    // refuses every job in the file over a value nobody reads.
    let replayable = step(
        "validate",
        &[
            (WRAPPER_VARIABLE, "${{ github.workspace }}/recorder"),
            (
                rustc_log::LOG_VARIABLE,
                "${{ github.workspace }}/rustc-log/validate.log",
            ),
        ],
    );
    assert_eq!(unresolvable(&[&replayable]), None);

    // Its control, and the case the refusal is actually for: a toolchain
    // resolved from a step this replay does not run. Replayed on the dev
    // toolchain, `msrv`'s units would collide with every other job's.
    let msrv = step(
        "msrv",
        &[
            (WRAPPER_VARIABLE, "/recorder"),
            (rustc_log::LOG_VARIABLE, "/w/rustc-log/msrv.log"),
            ("RUSTUP_TOOLCHAIN", "${{ steps.msrv.outputs.version }}"),
        ],
    );
    assert!(unresolvable(&[&msrv]).is_some_and(|why| why.contains("RUSTUP_TOOLCHAIN")));

    let mut scripted = replayable.clone();
    scripted.script = "cargo test -- ${{ github.sha }}".to_string();
    assert!(unresolvable(&[&scripted]).is_some_and(|why| why.contains("script")));
}

#[test]
fn the_replay_does_not_replay_the_gate_itself() {
    // The gate's job downloads what the others wrote and joins it. Replayed, it
    // would read a directory the replay is still filling AND put the
    // instrument's own build into the reading — the mistake this crate already
    // corrects once, for the log the gate's own job writes on a runner.
    let mut gate = step("twice-compiled", &[]);
    gate.script = format!(
        "cargo run -q --manifest-path {} --bin twice-compiled -- rustc-log",
        twice_compiled::GATE_MANIFEST
    );
    assert!(unresolvable(&[&gate]).is_some_and(|why| why.contains("this gate")));
}

#[test]
fn a_job_whose_compilations_took_no_time_at_all_is_refused() {
    // THE QUIETER WAY AN INSTRUMENT GOES SILENT, and the one the law above
    // cannot reach: the counts are all present, every total adds up, and only
    // the seconds are zero. It is what a job running an OLDER recorder produces
    // — a real state on a runner that restored a cached binary — and the report
    // it makes reads as work that costs nothing, which is exactly the finding
    // that would argue for merging every job in the file.
    //
    // Its control is the test above: the same census with its clocks intact is
    // accepted.
    let declared = declared_jobs(&[wired("validate"), wired("unrun-tests")]);
    let mut census = census_of(&["validate", "unrun-tests"]);
    let untimed = census.jobs.get_mut("unrun-tests").expect("the job");
    for cost in untimed.units.values_mut() {
        cost.micros = 0;
    }
    untimed.micros = 0;

    assert_eq!(
        judge(&census, &declared, &nothing()),
        vec![Refusal::JobRecordedNoTime {
            job: "unrun-tests".to_string()
        }]
    );
}

#[test]
fn a_job_that_recorded_nothing_is_refused_once_and_not_twice() {
    // A job with no compilations has no seconds either. Both laws are true of
    // it, and reporting both would name one silence twice — the second line
    // sending whoever reads it after a clock when the recorder never ran.
    let declared = declared_jobs(&[wired("validate"), wired("unrun-tests")]);
    let refusals = judge(&census_of(&["validate"]), &declared, &nothing());
    assert_eq!(
        refusals,
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
    let declared = declared_jobs(&[
        wired("validate"),
        wired("unrun-tests"),
        wired("twice-compiled"),
    ]);
    let mut absent = BTreeSet::new();
    absent.insert("twice-compiled".to_string());
    assert!(judge(&census_of(&["validate", "unrun-tests"]), &declared, &absent).is_empty());
}

#[test]
fn a_log_from_a_job_the_workflow_no_longer_declares_is_refused() {
    let declared = declared_jobs(&[wired("validate"), wired("unrun-tests")]);
    assert_eq!(
        judge(
            &census_of(&["validate", "unrun-tests", "deleted"]),
            &declared,
            &nothing()
        ),
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
    let declared = declared_jobs(&[
        wired("validate"),
        step("validate", &[]),
        wired("unrun-tests"),
    ]);
    let refusals = judge(
        &census_of(&["validate", "unrun-tests"]),
        &declared,
        &nothing(),
    );
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
    let declared = declared_jobs(&[
        step(
            "validate",
            &[
                (WRAPPER_VARIABLE, ""),
                (rustc_log::LOG_VARIABLE, "/w/rustc-log/validate.log"),
            ],
        ),
        wired("unrun-tests"),
    ]);
    assert_eq!(
        judge(
            &census_of(&["validate", "unrun-tests"]),
            &declared,
            &nothing()
        ),
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
    let declared = declared_jobs(&[
        step(
            "unrun-tests",
            &[
                (WRAPPER_VARIABLE, "/recorder"),
                (rustc_log::LOG_VARIABLE, "/w/rustc-log/validate.log"),
            ],
        ),
        wired("validate"),
    ]);
    assert_eq!(
        judge(
            &census_of(&["unrun-tests", "validate"]),
            &declared,
            &nothing()
        ),
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
    let declared = declared_jobs(&[wired("validate"), step("validate", &[]), wired("msrv")]);
    assert_eq!(
        declared.jobs.keys().collect::<Vec<_>>(),
        vec!["msrv", "validate"],
        "one entry per job"
    );
    assert_eq!(
        declared.jobs["validate"].len(),
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
    let declared = declared_jobs(&[building, wired("validate"), wired("unrun-tests")]);
    assert!(
        judge(
            &census_of(&["validate", "unrun-tests"]),
            &declared,
            &nothing()
        )
        .is_empty(),
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
            ("serde", "aaa", "link", 10),
            ("serde", "bbb", "link", 10),
            // AND ONCE MORE INSIDE THE SAME JOB — a job holding two `target`
            // directories. The first version of this test had no such row, and
            // the breakdown it checked silently dropped repeats: the report
            // printed `272 duplicated` on one line and `269 surplus` on the
            // next, from real data, and only the real data showed it.
            ("serde", "bbb", "link", 10),
            ("mine", "ccc", "link", 400),
        ]),
    );
    census.jobs.insert(
        "unrun-tests".to_string(),
        log_of(&[
            ("serde", "aaa", "link", 10),
            ("serde", "bbb", "link", 10),
            ("mine", "ddd", "link", 400),
        ]),
    );

    let surplus = census.surplus_by_crate();
    assert_eq!(
        surplus.get("serde"),
        Some(&Cost {
            times: 3,
            micros: 30
        }),
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
        surplus.values().map(|cost| cost.times).sum::<usize>(),
        census.paid() - census.floor(),
        "the breakdown adds up to the total it breaks down"
    );
    assert_eq!(
        surplus.values().map(|cost| cost.micros).sum::<u64>(),
        census.paid_micros() - census.floor_micros(),
        "and so does the price of it"
    );
}

#[test]
fn the_most_numerous_surplus_and_the_dearest_are_not_the_same_crate() {
    // THE FINDING THAT PUT A CLOCK IN THE RECORD. The first census this gate
    // took ranked the duplication by rows and put `build_script_build` at the
    // head with 409 surplus compilations — a crate whose units are among the
    // cheapest cargo drives, host-side and nothing to do with the feature
    // resolve the workflow's comments blamed. Ranked by rows the repair is a
    // shared compilation cache; ranked by seconds it can be something else
    // entirely, and until the recorder carried a clock nothing here could tell
    // the two orders apart. This fixture is that shape in miniature.
    let mut census = Census::default();
    for job in ["validate", "unrun-tests"] {
        census.jobs.insert(
            job.to_string(),
            log_of(&[
                ("build_script_build", "s1", "link", 5),
                ("build_script_build", "s2", "link", 5),
                ("build_script_build", "s3", "link", 5),
                ("mnemosyne_store", "aaa", "link", 900),
            ]),
        );
    }

    let surplus = census.surplus_by_crate();
    let mut by_rows: Vec<(&str, Cost)> = surplus.clone().into_iter().collect();
    by_rows.sort_by_key(|(name, cost)| (std::cmp::Reverse(cost.times), *name));
    assert_eq!(
        by_rows.first().map(|(name, cost)| (*name, cost.times)),
        Some(("build_script_build", 3)),
        "by rows, the build scripts head the list"
    );

    let mut by_seconds: Vec<(&str, Cost)> = surplus.into_iter().collect();
    by_seconds.sort_by_key(|(name, cost)| (std::cmp::Reverse(cost.micros), *name));
    assert_eq!(
        by_seconds.first().map(|(name, cost)| (*name, cost.micros)),
        Some(("mnemosyne_store", 900)),
        "by seconds, a single unit of this repository's own outweighs all three \
         — the two rankings name two different repairs"
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
            .map(|(name, metadata, emit)| compiled(EPOCH, 100, name, metadata, emit))
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
        log_of(&[("serde", "aaa", "link", 40), ("serde", "aaa", "link", 40)]),
    );
    census.jobs.insert(
        "unrun-tests".to_string(),
        log_of(&[("serde", "aaa", "link", 40)]),
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

    // AND THE SAME SPLIT IN SECONDS, which is the half of the report a repair
    // is actually chosen on. It is asserted here rather than trusted because the
    // cross-job figure is DEFINED as what is left over: if the two summed halves
    // ever failed to be the whole, this is the line that says so.
    assert_eq!(
        (
            census.paid_micros(),
            census.floor_micros(),
            census.repeated_within_jobs_micros(),
            census.shared_between_jobs_micros(),
        ),
        (120, 40, 40, 40),
    );
    assert_eq!(
        census.shared_between_jobs_micros() + census.repeated_within_jobs_micros(),
        census.paid_micros() - census.floor_micros(),
        "the two halves are the whole surplus in seconds too"
    );
}

#[test]
fn merging_a_pair_states_what_it_removes_and_what_it_spends() {
    // BOTH SIDES OF THE TRADE. `unrun-tests` and `side-workspaces` share more
    // units than any other pair in this repository AND are its two longest jobs,
    // so the pair with the most to save is also the pair with the most to lose:
    // merged, one of them starts where the other stops. A report that named only
    // the saving would be an argument with the cost left out — and this
    // repository has already written that argument down four times.
    let mut census = Census::default();
    census.jobs.insert(
        "validate".to_string(),
        log_of(&[("serde", "aaa", "link", 100), ("core", "bbb", "link", 300)]),
    );
    census.jobs.insert(
        "unrun-tests".to_string(),
        log_of(&[("serde", "aaa", "link", 60), ("core", "ccc", "link", 240)]),
    );

    let merge = census
        .pairwise()
        .get(&("unrun-tests", "validate"))
        .copied()
        .expect("the two jobs share a unit");
    assert_eq!(merge.units, 1);
    assert_eq!(
        merge.saved_micros, 60,
        "the cheaper of the two prices for the unit that stops being compiled"
    );
    // Each fixture job compiles one unit at a time, so its window IS its work:
    // 400 and 300.
    assert_eq!(
        (merge.floor_micros, merge.ceiling_micros),
        (400, 700),
        "merged it cannot be quicker than the longer job it replaces, nor slower \
         than the two of them end to end"
    );
    assert_eq!(
        merge.estimate_micros, 640,
        "and between them, the pair's window scaled by the work that is left: \
         700 × (700 − 60) / 700"
    );
    assert!(
        merge.estimate_micros > merge.floor_micros,
        "MERGING SPENDS WALL-CLOCK. Today these two run beside one another and \
         cost the run {} µs; merged they cost more, and the saving is compute",
        merge.floor_micros
    );
}

// --- what a job started from ------------------------------------------------
//
// A CENSUS IS ALSO A STATE. Every count above is of a COLD build by
// construction — cargo runs no compiler for a unit that is already fresh — so
// the cache state is the units the numbers are in, and two censuses taken in
// different ones are not each other's control. Round 1099 held two of them side
// by side, called the first cold because its keys had moved, and deleted a
// 7.5 GB cache that was saving 426 compilations. Nothing in the repository
// could have contradicted it: `actions/cache` reports `cache-hit: false` for a
// `restore-keys` prefix match, and a job warmed by one still saves a new entry,
// which is what the cache gate reads as a cache built from nothing.

/// Where a cached job's cache sits in its `steps:` list. The measurements are
/// steps [`BEFORE_AT`] and [`AFTER_AT`], which is what makes them its two sides.
/// The commit every fixture's instruments were built from. One value, because
/// agreeing is the ordinary case; the run where they do not is written out where
/// it is tested.
const A_COMMIT: &str = "0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f";

const CACHE_AT: usize = 1;
/// Where the measurement taken before the restore sits.
const BEFORE_AT: usize = 0;
/// Where the measurement taken after it sits.
const AFTER_AT: usize = 2;

/// A cache one job declares, over the paths it names.
fn cache(job: &str, paths: &[&str]) -> ci_plan::CacheDeclaration {
    ci_plan::CacheDeclaration {
        source: "fixture.yml".to_string(),
        owner: job.to_string(),
        index: CACHE_AT,
        key: format!("${{{{ runner.os }}}}-cargo-{job}-abc"),
        prefix: format!("Linux-cargo-{job}-"),
        restore_keys: vec![format!("Linux-cargo-{job}-")],
        paths: paths.iter().map(|path| (*path).to_string()).collect(),
        hashed: vec!["**/Cargo.lock".to_string()],
        // This gate asks where a cache step SITS in its job, never what wrote
        // its archive; the step name is what joins a declaration to the run that
        // did (R1207), and it is distinct per job here for the same reason the
        // prefix is.
        step: format!("Cache cargo ({job})"),
    }
}

/// A step of a job that has a cache: wired for both records.
fn wired_with_cache(job: &str) -> RunStep {
    let mut step = wired(job);
    step.env.insert(
        restored::VARIABLE.to_string(),
        format!("/w/rustc-log/{job}.restored"),
    );
    step
}

/// A step that measures one side of the restore, SPELLED AS THIS REPOSITORY'S
/// WORKFLOW SPELLS IT — the built binary, and the word `restored` itself defines
/// for that side. A fixture that merely called a step "the before one" would
/// pass a law about where the measurements sit while measuring nothing.
fn measuring(job: &str, side: restored::Side, index: usize, paths: &[&str]) -> RunStep {
    measuring_cache(job, &format!("Linux-cargo-{job}-"), side, index, paths)
}

/// The same, naming a cache other than the job's only one — which is what a
/// `--cache` argument copied from the step above it looks like.
fn measuring_cache(
    job: &str,
    cache: &str,
    side: restored::Side,
    index: usize,
    paths: &[&str],
) -> RunStep {
    let mut step = wired_with_cache(job);
    step.index = index;
    // THE `--cache` ARGUMENT IS PART OF THE SPELLING, and a fixture that left it
    // out would be exercising a law about which cache a pair prices against a
    // step that names none.
    step.script = format!(
        "./tools/{program}/target/release/{program} {side}{names}{paths}",
        program = restored::PROGRAM,
        side = side.word(),
        names = match side {
            restored::Side::Before => format!(" {} '{cache}'", restored::CACHE_FLAG),
            restored::Side::After => String::new(),
        },
        paths = paths
            .iter()
            .map(|path| format!(" '{path}'"))
            .collect::<String>()
    );
    step
}

/// Every `run:` step of a job with a cache, laid out AROUND it: the measurement
/// before the restore, the measurement after it, and the work.
fn cached_job(job: &str, paths: &[&str]) -> Vec<RunStep> {
    let mut work = wired_with_cache(job);
    work.index = AFTER_AT + 1;
    vec![
        measuring(job, restored::Side::Before, BEFORE_AT, paths),
        measuring(job, restored::Side::After, AFTER_AT, &[]),
        work,
    ]
}

/// The record such a job leaves, with what arrived under each path.
fn restore_record(job: &str, exact: bool, paths: &[(&str, u64)]) -> String {
    let mut out = restored::encode_job(job);
    // The same prefix `cache()` above declares for this job — the record says
    // WHICH cache its interval is the price of, and the gate checks the two.
    out.extend_from_slice(&restored::encode_cache(&format!("Linux-cargo-{job}-")));
    for instrument in restored::INSTRUMENTS {
        out.extend_from_slice(&restored::encode_built_from(instrument, A_COMMIT));
    }
    out.extend_from_slice(&restored::encode_at(restored::Side::Before, 1_000_000_000));
    for (path, _) in paths {
        out.extend_from_slice(&restored::encode_side(
            restored::Side::Before,
            path,
            &restored::Measurement::default(),
        ));
    }
    for (path, arrived) in paths {
        out.extend_from_slice(&restored::encode_side(
            restored::Side::After,
            path,
            &restored::Measurement {
                entries: 1,
                bytes: *arrived,
            },
        ));
    }
    out.extend_from_slice(&restored::encode_at(restored::Side::After, 1_030_000_000));
    out.extend_from_slice(&restored::encode_exact(exact));
    String::from_utf8(out).expect("the record is text")
}

/// The two jobs both compile and both cache, and both said what they restored.
fn cached_and_said(exact: bool, arrived: u64) -> (Declared, Census) {
    let mut steps = cached_job("validate", &["~/.cargo/registry"]);
    steps.extend(cached_job("unrun-tests", &["~/.cargo/registry", "target"]));
    let declared = declared_of(
        &steps,
        &[
            cache("validate", &["~/.cargo/registry"]),
            cache("unrun-tests", &["~/.cargo/registry", "target"]),
        ],
    );
    let mut census = census_of(&["validate", "unrun-tests"]);
    census.restored.insert(
        "validate.restored".to_string(),
        restored::decode(&restore_record(
            "validate",
            exact,
            &[("~/.cargo/registry", arrived)],
        )),
    );
    census.restored.insert(
        "unrun-tests.restored".to_string(),
        restored::decode(&restore_record(
            "unrun-tests",
            exact,
            &[("~/.cargo/registry", arrived), ("target", arrived)],
        )),
    );
    (declared, census)
}

#[test]
fn a_census_whose_jobs_all_said_what_they_restored_is_accepted() {
    let (declared, census) = cached_and_said(true, 1_000);
    assert!(judge(&census, &declared, &nothing()).is_empty());
}

/// A directory this machine does not have, so the coverage join answers the same
/// way on every machine that runs this suite.
///
/// WHAT IT DECIDES IS WHICH PARAGRAPH the per-job block carries, and neither
/// paragraph is what the laws below read — they read the restore lines, which
/// are printed either way. Naming a real directory would make the report depend
/// on whose checkout ran the suite.
const NOT_A_CHECKOUT: &str = "/nowhere-this-suite-runs";

/// R1126 — THE REPORT SAYS ABOUT A RESTORE WHAT THE RESTORE SAYS ABOUT ITSELF.
///
/// THIS IS R1125's DEFECT AS A LAW. That round's report asked the census for a
/// record by JOB after R1122 keyed it by the record's FILE, so the lookup could
/// never hit: every cached job of three green runs printed `NOT SAID` about a
/// record the census was holding in its hand. Nothing went red, because the only
/// test covering the line asked for the words `started from` — which
/// `started from: NOT SAID` contains too, so the suite agreed with the very
/// output it existed to prevent.
///
/// THE ORACLE IS NOT SPELLED HERE. What the line should say is computed by
/// `started_from`, the one reader of that datum, and this law asks only that the
/// report CONTAINS it. A wording change moves both sides together; a report that
/// stops reading the census moves one of them, which is the failure being kept
/// reachable.
///
/// THE POPULATION IS ASKED OF THE CENSUS, not listed: every restore it holds.
/// A list beside the law would go stale in the direction that reads as a pass.
#[test]
fn every_restore_the_census_holds_is_in_the_report_as_that_restore_reads() {
    let (declared, census) = cached_and_said(true, 1_000);
    let printed = twice_compiled::render(&census, &declared, &nothing(), Path::new(NOT_A_CHECKOUT));
    let held = census.started();
    // THE LAW ASSERTS ITS OWN REACH. An empty population passes every loop
    // below, and a fixture that stopped holding records would read as a clean
    // report rather than as a law that touched nothing.
    assert_eq!(
        held.len(),
        2,
        "the fixture declares a cache on both jobs and both said what they \
         restored: {held:?}"
    );
    for restore in held.keys() {
        let compiling = census.jobs[&restore.job].busy_micros();
        let reads = twice_compiled::started_from(&census, restore, compiling);
        assert!(
            printed.contains(&reads),
            "the report must carry `{reads}`, which is what this restore reads \
             as — printing anything else about a record the census HOLDS is the \
             defect R1125 shipped in three green runs\n{printed}"
        );
    }
}

/// And the reading names WHICH restore it is of, and WHAT that restore brought.
///
/// THE LAW ABOVE IS A CONSISTENCY LAW and cannot ask this: it holds the report
/// against `started_from`, so a wording that dropped the cache from the line
/// would move both sides together and pass. What the line must carry is fixed by
/// the datum instead of by a phrase here — the cache the restore names, and the
/// state the warmth prints for itself — because a job with two caches has two
/// states, and a sentence that gives one without saying which is a sentence a
/// reader has to guess at. Guessing which of a job's restores a number is about
/// is the substitution R1117 split the record to prevent.
#[test]
fn a_restores_line_names_which_cache_it_is_of_and_what_arrived() {
    let (_, census) = cached_and_said(true, 1_000);
    let held = census.started();
    assert_eq!(held.len(), 2, "{held:?}");
    for (restore, warmth) in &held {
        let reads = twice_compiled::started_from(&census, restore, 0);
        assert!(
            reads.contains(&restore.cache),
            "the line must say which cache it is the state of: {reads}"
        );
        assert!(
            reads.contains(&warmth.why()),
            "and what that restore brought, in the words the state prints for \
             itself: {reads}"
        );
    }
}

/// And the reading itself is of the census rather than of the argument.
///
/// THE CONTROL FOR THE LAW ABOVE, and it is what makes that one non-vacuous: a
/// `started_from` that ignored the census and always printed `NOT SAID` would
/// satisfy it, because the report would then contain that sentence too. Held
/// against the same restore in a census that no longer holds the record, the two
/// readings have to differ — a reader that never hits prints the same thing
/// either way, which is exactly what a lookup that cannot hit looks like.
#[test]
fn a_restore_reads_differently_once_the_census_no_longer_holds_its_record() {
    let (_, census) = cached_and_said(true, 1_000);
    let held = census.started();
    assert_eq!(held.len(), 2, "{held:?}");
    for restore in held.keys() {
        let compiling = census.jobs[&restore.job].busy_micros();
        let mut without = census.clone();
        // BY THE KEY THE CENSUS USES, which is the file — the very key R1122
        // moved it to and the report was left behind by.
        without.restored.retain(|_, record| {
            record.as_ref().map(restored::Restored::restore) != Ok(restore.clone())
        });
        assert_eq!(
            without.restored.len(),
            census.restored.len() - 1,
            "{restore:?}"
        );
        assert_ne!(
            twice_compiled::started_from(&census, restore, compiling),
            twice_compiled::started_from(&without, restore, compiling),
            "a record the census holds and one it does not must not read alike"
        );
    }
}

/// THE STATE TRAVELS INTO THE CENSUS as a value, not as a sentence: it is what
/// one census is compared to another by.
#[test]
fn the_three_states_reach_the_census_as_three_different_values() {
    let (_, warm) = cached_and_said(true, 1_000);
    let (_, stale) = cached_and_said(false, 1_000);
    let (_, cold) = cached_and_said(false, 0);
    let the_cache = restored::Restore {
        job: "unrun-tests".to_string(),
        cache: "Linux-cargo-unrun-tests-".to_string(),
    };
    assert_eq!(
        warm.started().get(&the_cache),
        Some(&restored::Warmth::ExactHit { bytes: 2_000 })
    );
    assert_eq!(
        stale.started().get(&the_cache),
        Some(&restored::Warmth::PrefixHit { bytes: 2_000 })
    );
    assert_eq!(
        cold.started().get(&the_cache),
        Some(&restored::Warmth::Nothing)
    );
    // AND THE TWO THAT LOOK ALIKE TO EVERY OTHER INSTRUMENT DIFFER HERE. A
    // prefix hit and a cold build both report `cache-hit: false` and both make
    // `actions/cache` save a new entry.
    assert_ne!(stale.started(), cold.started());
}

/// THE SILENCE LAW. A job with a cache that left no record hands the census a
/// state the reader has to supply, and Round 1099 supplied the wrong one.
#[test]
fn a_job_with_a_cache_that_did_not_say_what_it_restored_is_refused() {
    let (declared, mut census) = cached_and_said(true, 1_000);
    census.restored.remove("unrun-tests.restored");
    assert_eq!(
        judge(&census, &declared, &nothing()),
        vec![Refusal::JobDidNotSayWhatItRestored {
            job: "unrun-tests".to_string(),
            cache: "Linux-cargo-unrun-tests-".to_string(),
        }]
    );
}

/// The control for the law above: the population is the jobs with a CACHE, not
/// the jobs that compile. This repository's two gate jobs compile plenty and
/// cache nothing.
#[test]
fn a_job_with_no_cache_owes_no_record_of_what_it_restored() {
    let mut steps = cached_job("validate", &["~/.cargo/registry"]);
    steps.push(wired("twice-compiled"));
    let declared = declared_of(&steps, &[cache("validate", &["~/.cargo/registry"])]);
    let mut census = census_of(&["validate", "twice-compiled"]);
    census.restored.insert(
        "validate.restored".to_string(),
        restored::decode(&restore_record(
            "validate",
            true,
            &[("~/.cargo/registry", 10)],
        )),
    );
    assert!(judge(&census, &declared, &nothing()).is_empty());
}

/// WHAT MAKES THE LIST BEING WRITTEN TWICE SAFE. The cache's `path:` and the
/// measuring step's arguments are two spellings of one list, and this is the
/// only thing holding them together.
#[test]
fn a_record_measuring_paths_the_cache_does_not_hold_is_refused() {
    let (declared, mut census) = cached_and_said(true, 1_000);
    census.restored.insert(
        "unrun-tests.restored".to_string(),
        restored::decode(&restore_record(
            "unrun-tests",
            true,
            &[("~/.cargo/registry", 1_000)],
        )),
    );
    assert_eq!(
        judge(&census, &declared, &nothing()),
        vec![Refusal::RestoreRecordMeasuredOtherPaths {
            job: "unrun-tests".to_string(),
            cache: "Linux-cargo-unrun-tests-".to_string(),
            measured: vec!["~/.cargo/registry".to_string()],
            declared: vec!["~/.cargo/registry".to_string(), "target".to_string()],
        }],
        "a `target` added to the cache and not to the measurement makes every \
         restore of it read smaller than it was"
    );
}

/// The two instruments disagreeing is not a third reading of the world.
#[test]
fn an_exact_hit_that_brought_nothing_is_refused_rather_than_counted() {
    let (declared, census) = cached_and_said(true, 0);
    assert_eq!(
        judge(&census, &declared, &nothing()),
        vec![
            Refusal::RestoredNothingAfterAnExactHit {
                job: "unrun-tests".to_string(),
                cache: "Linux-cargo-unrun-tests-".to_string(),
            },
            Refusal::RestoredNothingAfterAnExactHit {
                job: "validate".to_string(),
                cache: "Linux-cargo-validate-".to_string(),
            },
        ]
    );
}

/// R1120 — A JOB'S CACHES ARE A LIST OF STEPS, and until this round they were
/// three maps keyed by job that could disagree about how many of them there are.
/// For a job declaring two, they DID: two indices, two prefixes, and ONE merged
/// path list with no way back to which cache held which path. The merge was right
/// while a restore record was a job's; R1117 made a record a cache's, and that is
/// when it became a loss.
#[test]
fn a_job_declaring_two_caches_keeps_them_apart() {
    let mut first = cache("unrun-tests", &["~/.cargo/registry"]);
    first.prefix = "Linux-cargo-unrun-home-".to_string();
    let mut second = cache("unrun-tests", &["target"]);
    second.index = first.index + 2;
    second.prefix = "Linux-cargo-unrun-tree-".to_string();
    let declared = twice_compiled::Declared::of(
        &cached_job("unrun-tests", &["~/.cargo/registry", "target"]),
        &[first.clone(), second.clone()],
        &[collecting("unrun-tests")],
    );

    let held = &declared.caches["unrun-tests"];
    assert_eq!(held.len(), 2, "{held:?}");
    assert_eq!(
        (held[0].prefix.as_str(), held[0].paths.as_slice()),
        (
            "Linux-cargo-unrun-home-",
            &["~/.cargo/registry".to_string()][..]
        ),
        "WHICH CACHE HOLDS WHICH PATH, which the merge destroyed — and it is what \
         attributing a restore's price to one of two caches needs"
    );
    assert_eq!(
        (held[1].prefix.as_str(), held[1].paths.as_slice()),
        ("Linux-cargo-unrun-tree-", &["target".to_string()][..]),
    );

    // AND THE OLD ANSWERS ARE STILL ASKABLE, derived from the one datum rather
    // than kept beside it: the region laws want the merged paths and the
    // per-cache laws want the list, and neither is now a second copy that can
    // drift from the other.
    assert_eq!(
        declared.cached_paths("unrun-tests"),
        vec!["~/.cargo/registry".to_string(), "target".to_string()],
        "first mention first, duplicates dropped"
    );
    assert_eq!(
        declared.prefixes("unrun-tests"),
        vec![
            "Linux-cargo-unrun-home-".to_string(),
            "Linux-cargo-unrun-tree-".to_string()
        ]
    );
}

/// R1119 — WHICH BUILD OF THE INSTRUMENTS MEASURED THIS RUN. R1118 found six
/// jobs measured by the commit's recorder and one by whatever its cache held, and
/// nothing could say so: the seconds moved by a factor of four when the fresh one
/// finally ran. Both directions are asked of the census alone.
#[test]
fn a_run_whose_jobs_were_measured_by_different_builds_is_refused() {
    let (declared, mut census) = cached_and_said(true, 1_000);
    let elsewhere = restore_record(
        "unrun-tests",
        true,
        &[("~/.cargo/registry", 1_000), ("target", 1_000)],
    )
    .replace(A_COMMIT, "beefbeefbeefbeefbeefbeefbeefbeefbeefbeef");
    census
        .restored
        .insert("unrun-tests".to_string(), restored::decode(&elsewhere));
    let refusals = judge(&census, &declared, &nothing());
    assert!(
        refusals.iter().any(|refusal| matches!(
            refusal,
            Refusal::JobsOfOneRunWereMeasuredByDifferentBuilds { .. }
        )),
        "two censuses taken by two builds of the recorder are not in the same \
         units, and summing them prints one number for two measurements: \
         {refusals:?}"
    );
}

#[test]
fn one_jobs_two_instruments_must_agree_about_their_build() {
    let (declared, mut census) = cached_and_said(true, 1_000);
    // The measuring program survived the restore and the recorder did not — the
    // exact shape of R1118, where `restored` was fresh and wrote a line the
    // restored `restored after` could not read.
    let mixed = restore_record(
        "unrun-tests",
        true,
        &[("~/.cargo/registry", 1_000), ("target", 1_000)],
    )
    .replace(
        &String::from_utf8(restored::encode_built_from("recorder", A_COMMIT)).expect("text"),
        &String::from_utf8(restored::encode_built_from("recorder", "0a0a0a")).expect("text"),
    );
    census
        .restored
        .insert("unrun-tests".to_string(), restored::decode(&mixed));
    assert!(
        judge(&census, &declared, &nothing())
            .iter()
            .any(|refusal| matches!(
                refusal,
                Refusal::InstrumentsOfOneJobDisagreeAboutTheirBuild { .. }
            )),
        "one step builds both from one commit, so a difference is one of them \
         having arrived from somewhere else"
    );
}

#[test]
fn a_recorder_deliberately_absent_is_not_a_disagreement() {
    let (declared, mut census) = cached_and_said(true, 1_000);
    // The instruments' own build sets `RUSTC_WRAPPER: ""`, and a step running
    // under that has no recorder to disagree with. Reading `none` as a mismatch
    // would refuse every correct workflow in this repository.
    let no_recorder = restore_record(
        "unrun-tests",
        true,
        &[("~/.cargo/registry", 1_000), ("target", 1_000)],
    )
    .replace(
        &String::from_utf8(restored::encode_built_from("recorder", A_COMMIT)).expect("text"),
        &String::from_utf8(restored::encode_built_from("recorder", "none")).expect("text"),
    );
    census
        .restored
        .insert("unrun-tests".to_string(), restored::decode(&no_recorder));
    assert!(
        !judge(&census, &declared, &nothing())
            .iter()
            .any(|refusal| matches!(
                refusal,
                Refusal::InstrumentsOfOneJobDisagreeAboutTheirBuild { .. }
                    | Refusal::JobsOfOneRunWereMeasuredByDifferentBuilds { .. }
            )),
        "`none` is an absence this workflow arranges, not a substitution"
    );
}

/// R1118 — A RESTORE OVERWRITES, and that is a different axis from where the
/// measurements sit. `unrun-tests` built its instruments into `target`, cached
/// `target`, and restored it after the build: the program the job ran was the one
/// the previous generation stored. The wiring law reads the ORDER of steps and
/// cannot see it — every step was in the right place and the file was wrong.
#[test]
fn a_program_a_job_runs_after_a_restore_may_not_live_under_it() {
    let overwritten = |program: &str, cached: &[&str]| {
        let mut runs = cached_job("unrun-tests", cached);
        runs[0].script = format!("{program} before 'target'");
        runs[1].script = format!("{program} after");
        let declared = twice_compiled::Declared::of(
            &runs,
            &[cache("unrun-tests", cached)],
            &[collecting("unrun-tests")],
        );
        twice_compiled::judge_wiring(&declared)
            .into_iter()
            .filter(|refusal| {
                matches!(refusal, Refusal::ProgramRunAfterARestoreLivesUnderIt { .. })
            })
            .collect::<Vec<_>>()
    };

    assert_eq!(
        overwritten("./target/debug/restored", &["target"]),
        vec![
            Refusal::ProgramRunAfterARestoreLivesUnderIt {
                job: "unrun-tests".to_string(),
                program: "target/debug/restored".to_string(),
                held: "target".to_string(),
            },
            Refusal::ProgramRunAfterARestoreLivesUnderIt {
                job: "unrun-tests".to_string(),
                program: "target/debug/restored".to_string(),
                held: "target".to_string(),
            },
        ],
        "the binary is replaced between the step that builds it and the step \
         that runs it, and BOTH measuring steps name it"
    );

    // THE OTHER DIRECTION, which keeps this from being a law against relative
    // programs: the instrument outside the cached tree is what this repository
    // now does, and it must pass.
    assert!(
        overwritten("./instruments/debug/restored", &["target"]).is_empty(),
        "a program the cache cannot reach is not one the restore replaces"
    );

    // A CACHE OF THE CARGO HOME REACHES NOTHING IN THE CHECKOUT — a refusal
    // naming a file no cache touches is how a gate teaches people to ignore it.
    assert!(
        overwritten("./target/debug/restored", &["~/.cargo/registry"]).is_empty(),
        "`~/.cargo` is not the checkout"
    );
    // BUT IT REACHES WHAT IT HOLDS. A program under the restored cargo home is
    // replaced exactly as one under `target/` is, and this repository's first
    // version of this law filtered `~` paths out — which changed no answer and
    // would have decided this one wrongly. The sweep found the filter dead by
    // removing it and watching nothing go red.
    assert_eq!(
        overwritten("~/.cargo/registry/bin/restored", &["~/.cargo/registry"]).len(),
        2,
        "both measuring steps run a program the restore overwrites"
    );
}

/// R1117 — A RECORD SAYS WHICH CACHE ITS INTERVAL IS THE PRICE OF, and that
/// field is only worth having if something refuses a wrong one. R1116 found the
/// cost of the other shape: `restore-keys` was spelled by hand, derived
/// separately, and compared never, so a sentence about it went years without
/// anything able to contradict it.
#[test]
fn a_record_naming_a_cache_its_job_does_not_declare_is_refused() {
    let (declared, mut census) = cached_and_said(true, 1_000);
    let mut wrong = restore_record(
        "unrun-tests",
        true,
        &[("~/.cargo/registry", 1_000), ("target", 1_000)],
    );
    wrong = wrong.replace("Linux-cargo-unrun-tests-", "Linux-cargo-somebody-elses-");
    census
        .restored
        .insert("unrun-tests.restored".to_string(), restored::decode(&wrong));
    assert_eq!(
        judge(&census, &declared, &nothing()),
        vec![
            // AND THE CACHE IT SHOULD HAVE PRICED IS LEFT WITH NOTHING. Not the
            // same sentence twice: one says a record charges an interval to a
            // cache nobody declares, the other says a declared cache has no
            // price at all, and a reader handed only the first would have to
            // work out for itself which restore went unmeasured.
            Refusal::JobDidNotSayWhatItRestored {
                job: "unrun-tests".to_string(),
                cache: "Linux-cargo-unrun-tests-".to_string(),
            },
            Refusal::RestoreRecordNamesACacheTheJobDoesNotDeclare {
                job: "unrun-tests".to_string(),
                named: "Linux-cargo-somebody-elses-".to_string(),
                declared: vec!["Linux-cargo-unrun-tests-".to_string()],
            },
        ],
        "an interval charged to a cache the job does not have would answer the \
         next reader's question about what a cache costs with somebody else's \
         number, which is worse than having no answer"
    );
}

/// THE PASTE ERROR, caught on the record's side: the file is named for one job
/// by hand and the contents carry the job the runner named.
#[test]
fn a_record_whose_contents_name_another_job_is_refused() {
    let (declared, mut census) = cached_and_said(true, 1_000);
    census.restored.insert(
        "unrun-tests.restored".to_string(),
        restored::decode(&restore_record(
            "validate",
            true,
            &[("~/.cargo/registry", 1_000), ("target", 1_000)],
        )),
    );
    assert_eq!(
        judge(&census, &declared, &nothing()),
        vec![
            Refusal::JobDidNotSayWhatItRestored {
                job: "unrun-tests".to_string(),
                cache: "Linux-cargo-unrun-tests-".to_string(),
            },
            Refusal::RestoreRecordNamesAnotherJob {
                file: "unrun-tests.restored".to_string(),
                said: "validate".to_string(),
            },
        ],
        "and NOT as a second price for `validate`'s restore, which is what a \
         pasted record looks like to a join that reads the contents before \
         checking the name it arrived under"
    );
}

/// And caught on the workflow's side, before any run produces anything.
#[test]
fn a_job_writing_its_state_to_another_jobs_file_is_refused() {
    let mut steps = cached_job("unrun-tests", &["~/.cargo/registry"]);
    for step in &mut steps {
        step.env.insert(
            restored::VARIABLE.to_string(),
            "/w/rustc-log/validate.restored".to_string(),
        );
    }
    steps.extend(cached_job("validate", &["~/.cargo/registry"]));
    let declared = declared_of(
        &steps,
        &[
            cache("validate", &["~/.cargo/registry"]),
            cache("unrun-tests", &["~/.cargo/registry"]),
        ],
    );
    let mut census = census_of(&["validate", "unrun-tests"]);
    for job in ["validate", "unrun-tests"] {
        census.restored.insert(
            format!("{job}.restored"),
            restored::decode(&restore_record(job, true, &[("~/.cargo/registry", 10)])),
        );
    }
    assert_eq!(
        judge(&census, &declared, &nothing()),
        vec![Refusal::RestoreIsNotRecorded {
            job: "unrun-tests".to_string(),
            path: "/w/rustc-log/validate.restored".to_string(),
        }]
    );
}

/// R1122 — THE POPULATION IS THE STEPS THAT MEASURE. The variable used to sit in
/// a job's `env:`, so every step of a cached job carried it and asking all of
/// them cost nothing. A job with TWO caches writes TWO records, so the value
/// cannot be the job's any more, and a law still demanding it of the steps that
/// do the WORK would refuse exactly the workflow this round was for.
#[test]
fn a_measuring_step_that_does_not_say_where_to_write_it_is_refused() {
    let mut steps = cached_job("validate", &["~/.cargo/registry"]);
    let mut stripped = 0;
    for step in &mut steps {
        if !restored::sides_measured(&step.script).is_empty() {
            step.env.remove(restored::VARIABLE);
            stripped += 1;
        }
    }
    assert_eq!(
        stripped, 2,
        "both sides of the pair, so what is left is one \
         record path nobody named rather than a pair that disagrees with itself"
    );
    steps.extend(cached_job("unrun-tests", &["~/.cargo/registry"]));
    let declared = declared_of(
        &steps,
        &[
            cache("validate", &["~/.cargo/registry"]),
            cache("unrun-tests", &["~/.cargo/registry"]),
        ],
    );
    let mut census = census_of(&["validate", "unrun-tests"]);
    for job in ["validate", "unrun-tests"] {
        census.restored.insert(
            format!("{job}.restored"),
            restored::decode(&restore_record(job, true, &[("~/.cargo/registry", 10)])),
        );
    }
    assert_eq!(
        judge(&census, &declared, &nothing()),
        vec![Refusal::RestoreIsNotRecorded {
            job: "validate".to_string(),
            path: String::new(),
        }]
    );
}

/// A HALF-WRITTEN RECORD IS NOT A COLD JOB. The `exact` line is the last thing
/// the second step writes, so a job that died between the restore and the
/// measurement leaves a file that would otherwise decode as one that restored
/// nothing.
#[test]
fn a_record_that_does_not_decode_is_refused_rather_than_read_as_cold() {
    let (declared, mut census) = cached_and_said(true, 1_000);
    let torn = restore_record("unrun-tests", true, &[("~/.cargo/registry", 1_000)]);
    let torn = torn
        .lines()
        .filter(|line| !line.starts_with("exact"))
        .collect::<Vec<_>>()
        .join("\n");
    census
        .restored
        .insert("unrun-tests.restored".to_string(), restored::decode(&torn));
    let refusals = judge(&census, &declared, &nothing());
    // TWO REFUSALS AND THEY ARE NOT THE SAME DEFECT SAID TWICE. The record is
    // unreadable, which is named by its FILE because nothing inside it can be
    // trusted; and the cache it was supposed to price is left with no record at
    // all, which is named by the CACHE. A reader handed only the first would
    // have to work out which restore is now unmeasured.
    assert_eq!(
        refusals,
        vec![
            Refusal::JobDidNotSayWhatItRestored {
                job: "unrun-tests".to_string(),
                cache: "Linux-cargo-unrun-tests-".to_string(),
            },
            Refusal::RestoreRecordIsMalformed {
                file: "unrun-tests.restored".to_string(),
                why: restored::Malformed::ExactIsNotSaidOnce { times: 0 }.to_string(),
            },
        ],
        "{refusals:?}"
    );
}

#[test]
fn a_record_from_a_job_this_workflow_gives_no_cache_is_refused() {
    let (declared, mut census) = cached_and_said(true, 1_000);
    census.restored.insert(
        "deleted.restored".to_string(),
        restored::decode(&restore_record("deleted", true, &[("target", 1)])),
    );
    assert_eq!(
        judge(&census, &declared, &nothing()),
        vec![Refusal::RestoreRecordFromAJobWithNoCache {
            job: "deleted".to_string()
        }]
    );
}

// --- where the two measurements sit -----------------------------------------
//
// WHAT A JOB STARTED FROM IS A DIFFERENCE, and a difference is only a
// measurement if the two readings are taken on opposite sides of the thing being
// measured. Both on one side gives zero, and zero is indistinguishable from the
// answer that matters most — a job that compiled from an empty tree, the state
// Round 1099 misread at the cost of a cache that was saving ten minutes a run.
//
// Round 1102 could only catch that AT RUNTIME (a job reporting an empty tree
// next to a restorable generation is refused), because the two populations came
// out of the workflow with no shared coordinate: `run:` steps and cache steps
// were two unordered lists, and "is this step before that one?" had no answer in
// the file. `ci_plan::RunStep::index` is that coordinate, and these are the laws
// it makes askable.

/// The layout every cached job in this repository has, and the control for all
/// of the refusals below.
#[test]
fn measurements_on_the_two_sides_of_the_restore_are_accepted() {
    let mut steps = cached_job("validate", &["~/.cargo/registry"]);
    steps.extend(cached_job("unrun-tests", &["~/.cargo/registry", "target"]));
    let declared = declared_of(
        &steps,
        &[
            cache("validate", &["~/.cargo/registry"]),
            cache("unrun-tests", &["~/.cargo/registry", "target"]),
        ],
    );
    assert!(twice_compiled::judge_wiring(&declared).is_empty());
    // AND IT IS NOT THE EMPTY ANSWER. A law that reached no job at all also
    // returns nothing, and every refusal below would then be unreachable.
    assert_eq!(
        declared.caches.len(),
        2,
        "two jobs declare a cache and both were judged"
    );
}

/// THE DEFECT THAT LOOKS LIKE A FINDING, caught in the file.
#[test]
fn a_measurement_taken_after_the_restore_it_precedes_is_refused() {
    let mut steps = cached_job("validate", &["~/.cargo/registry"]);
    steps[0].index = CACHE_AT + 10;
    steps.extend(cached_job("unrun-tests", &["~/.cargo/registry"]));
    let declared = declared_of(
        &steps,
        &[
            cache("validate", &["~/.cargo/registry"]),
            cache("unrun-tests", &["~/.cargo/registry"]),
        ],
    );
    assert_eq!(
        twice_compiled::judge_wiring(&declared),
        vec![Refusal::AMeasuringPairBracketsOtherThanOneCache {
            job: "validate".to_string(),
            record: "/w/rustc-log/validate.restored".to_string(),
            caches: 0,
        }],
        "with both readings after the restore the difference is zero, which is \
         exactly what a job that compiled from an empty tree reports — and \
         R1121 names it by the count of caches the pair encloses, which is the \
         same question asked once instead of once per side"
    );
}

/// The same defect from the other end, and it is a separate arm rather than the
/// same one read backwards.
#[test]
fn a_measurement_taken_before_the_restore_it_follows_is_refused() {
    let mut steps = cached_job("validate", &["~/.cargo/registry"]);
    steps[1].index = BEFORE_AT;
    let declared = declared_of(&steps, &[cache("validate", &["~/.cargo/registry"])]);
    assert_eq!(
        twice_compiled::judge_wiring(&declared),
        vec![Refusal::AMeasuringPairBracketsOtherThanOneCache {
            job: "validate".to_string(),
            record: "/w/rustc-log/validate.restored".to_string(),
            caches: 0,
        }]
    );
}

/// A JOB WITH A CACHE AND NO MEASUREMENT AT ALL. Today the census says so an
/// hour later, from a run, by way of the record that never arrived; the file was
/// already wrong when it was written.
#[test]
fn a_cached_job_that_measures_neither_side_is_refused() {
    let declared = declared_of(
        &[wired_with_cache("validate")],
        &[cache("validate", &["~/.cargo/registry"])],
    );
    // R1121 — ONE NAME FOR IT, and it is the count. Before this round the same
    // job produced two refusals, one per missing side; a job that measures no
    // restore at all is one defect and the pair-count says so exactly.
    assert_eq!(
        twice_compiled::judge_wiring(&declared),
        vec![Refusal::AJobMeasuresOtherThanOneRestorePerCache {
            job: "validate".to_string(),
            pairs: 0,
            caches: 1,
        }]
    );
}

/// TWICE IS NOT ONCE. The second reading overwrites the first, and a step that
/// runs the measurement twice is one step — so counting steps rather than
/// invocations would call this a job that measured its restore properly.
#[test]
fn a_side_measured_twice_is_refused() {
    let mut steps = cached_job("validate", &["~/.cargo/registry"]);
    let twice = measuring("validate", restored::Side::Before, BEFORE_AT, &["target"]);
    steps[0].script = format!("{} && {}", steps[0].script, twice.script);
    let declared = declared_of(&steps, &[cache("validate", &["~/.cargo/registry"])]);
    assert_eq!(
        twice_compiled::judge_wiring(&declared),
        vec![Refusal::RestoreSideIsNotMeasuredOnce {
            job: "validate".to_string(),
            record: "/w/rustc-log/validate.restored".to_string(),
            side: restored::Side::Before,
            times: 2,
        }]
    );
}

/// The mirror, read off the workflow: a measurement with nothing between its two
/// halves reports an empty tree for a job that never had one to fill.
#[test]
fn a_job_with_no_cache_that_measures_a_restore_is_refused() {
    let mut steps = cached_job("validate", &["~/.cargo/registry"]);
    steps.extend(cached_job("twice-compiled", &["target"]));
    let declared = declared_of(&steps, &[cache("validate", &["~/.cargo/registry"])]);
    assert_eq!(
        twice_compiled::judge_wiring(&declared),
        vec![Refusal::RestoreIsMeasuredWithNoCache {
            job: "twice-compiled".to_string()
        }]
    );
}

/// R1121 — AND THE POSITIVE SIDE, which is the shape the workflow is about to
/// take: two caches, each with its own pair writing its own record, is CORRECT
/// and must be accepted. Without this the law reads as "one cache per job" and
/// the split it exists to allow would be refused by it.
///
/// IT IS ALSO WHAT MAKES THE GROUPING LOAD-BEARING. An injection that grouped
/// the pairs by JOB instead of by RECORD left every refusal above unchanged —
/// one job, one bucket, same counts — and only a job that is RIGHT tells the two
/// readings apart. The sweep found that oracle missing by coming back empty.
#[test]
fn a_job_with_a_pair_for_each_of_its_two_caches_is_accepted() {
    let mut steps = Vec::new();
    let mut caches = Vec::new();
    for (which, index) in [("home", 0usize), ("tree", 3usize)] {
        for (side, at) in [
            (restored::Side::Before, index),
            (restored::Side::After, index + 2),
        ] {
            let mut step = measuring_cache(
                "unrun-tests",
                &format!("Linux-cargo-unrun-{which}-"),
                side,
                at,
                &[],
            );
            // ITS OWN RECORD, which is the only thing in the file that says
            // which pair a measurement belongs to.
            step.env.insert(
                restored::VARIABLE.to_string(),
                format!("/w/rustc-log/unrun-tests.{which}.restored"),
            );
            steps.push(step);
        }
        let mut declared = cache("unrun-tests", &["target"]);
        declared.index = index + 1;
        declared.prefix = format!("Linux-cargo-unrun-{which}-");
        caches.push(declared);
    }
    let mut work = wired_with_cache("unrun-tests");
    work.index = 6;
    steps.push(work);

    assert_eq!(
        twice_compiled::judge_wiring(&declared_of(&steps, &caches)),
        Vec::new(),
        "each pair encloses exactly one cache step and writes its own record, \
         which is what pricing two caches of one job requires"
    );

    // AND THE ONE MISTAKE NO RECORD CAN REPORT: the pairs keep their own files
    // and their own positions, and the `--cache` argument of the second is
    // copied from the first. Both prefixes are this job's, so the record it
    // writes decodes, names a cache the job really declares, and passes every
    // check that reads a record — while carrying the other restore's interval.
    let swapped: Vec<RunStep> = steps
        .iter()
        .cloned()
        .map(|mut step| {
            step.script = step
                .script
                .replace("Linux-cargo-unrun-tree-", "Linux-cargo-unrun-home-");
            step
        })
        .collect();
    assert_eq!(
        twice_compiled::judge_wiring(&declared_of(&swapped, &caches)),
        vec![Refusal::AMeasuringPairNamesAnotherCache {
            job: "unrun-tests".to_string(),
            record: "/w/rustc-log/unrun-tests.tree.restored".to_string(),
            named: vec!["Linux-cargo-unrun-home-".to_string()],
            brackets: "Linux-cargo-unrun-tree-".to_string(),
        }],
        "the argument is what the record's `cache` line is written from, and \
         nothing downstream can tell one of a job's own prefixes from another"
    );

    // THE THIRD SHAPE: a pair that names NO cache at all. Its `before` step
    // exits 1 the moment it runs, so this never reaches a record — which is
    // exactly why it has to be caught in the file.
    let silent: Vec<RunStep> = steps
        .iter()
        .cloned()
        .map(|mut step| {
            step.script = step.script.replace(
                &format!("{} 'Linux-cargo-unrun-tree-'", restored::CACHE_FLAG),
                "",
            );
            step
        })
        .collect();
    assert_eq!(
        twice_compiled::judge_wiring(&declared_of(&silent, &caches)),
        vec![Refusal::AMeasuringPairNamesAnotherCache {
            job: "unrun-tests".to_string(),
            record: "/w/rustc-log/unrun-tests.tree.restored".to_string(),
            named: Vec::new(),
            brackets: "Linux-cargo-unrun-tree-".to_string(),
        }]
    );
}

/// R1122 — TWO RECORDS FOR ONE RESTORE IS AN OVERWRITE, AND AN OVERWRITE IS
/// SILENT. The join is a map, so the second would simply replace the first and
/// the census would print one price with no sign that another had been offered.
#[test]
fn two_records_for_one_restore_are_refused_rather_than_letting_one_win() {
    let (declared, mut census) = cached_and_said(true, 1_000);
    // The same job, the same cache, a second file — which is what a pair whose
    // record path was copied from its neighbour leaves behind.
    census.restored.insert(
        "unrun-tests.again.restored".to_string(),
        restored::decode(&restore_record(
            "unrun-tests",
            true,
            &[("~/.cargo/registry", 7), ("target", 7)],
        )),
    );
    let refusals = judge(&census, &declared, &nothing());
    assert_eq!(
        refusals,
        vec![Refusal::OneRestoreIsRecordedTwice {
            restore: "job `unrun-tests` cache `Linux-cargo-unrun-tests-`".to_string(),
            // BOTH OF THEM. Which one a map keeps is the order the directory
            // was walked in, so a refusal naming one would send the reader to
            // whichever file the filesystem happened to hand over second.
            files: vec![
                "unrun-tests.again.restored".to_string(),
                "unrun-tests.restored".to_string(),
            ],
            measured: vec![
                "~/.cargo/registry, target".to_string(),
                "~/.cargo/registry, target".to_string(),
            ],
        }],
        "{refusals:?}"
    );
}

/// R1122 — AND A JOB'S TWO RECORDS ARE READ AS TWO. A map keyed by the job holds
/// whichever was read last, which is the registry's five seconds standing in for
/// the build directory's hundred and thirty.
#[test]
fn a_job_that_wrote_one_record_per_cache_has_both_of_them_read() {
    let mut steps = Vec::new();
    let mut caches = Vec::new();
    let mut census = census_of(&["unrun-tests", "twice-compiled"]);
    for (which, index, paths) in [
        ("home", 0usize, &["~/.cargo/registry"][..]),
        ("tree", 3usize, &["target"][..]),
    ] {
        let prefix = format!("Linux-cargo-unrun-{which}-");
        for (side, at) in [
            (restored::Side::Before, index),
            (restored::Side::After, index + 2),
        ] {
            let mut step = measuring_cache("unrun-tests", &prefix, side, at, paths);
            step.env.insert(
                restored::VARIABLE.to_string(),
                format!("/w/rustc-log/unrun-tests.{which}.restored"),
            );
            steps.push(step);
        }
        let mut declared = cache("unrun-tests", paths);
        declared.index = index + 1;
        declared.prefix = prefix.clone();
        caches.push(declared);
        // EACH CACHE'S OWN PATHS AND ITS OWN SIZE, because the whole point of
        // the split is that these two numbers are not one number.
        let mut written = restore_record(
            "unrun-tests",
            true,
            &paths
                .iter()
                .map(|path| (*path, if which == "tree" { 32_000 } else { 700 }))
                .collect::<Vec<_>>(),
        );
        written = written.replace("Linux-cargo-unrun-tests-", &prefix);
        census.restored.insert(
            format!("unrun-tests.{which}.restored"),
            restored::decode(&written),
        );
    }
    steps.push(wired("twice-compiled"));
    let declared = declared_of(&steps, &caches);
    assert!(
        judge(&census, &declared, &nothing()).is_empty(),
        "{:?}",
        judge(&census, &declared, &nothing())
    );
    // AND THE TWO STATES ARE BOTH THERE, with the sizes that tell them apart.
    let started = census.started();
    assert_eq!(started.len(), 2, "{started:?}");
    assert_eq!(
        started.get(&restored::Restore {
            job: "unrun-tests".to_string(),
            cache: "Linux-cargo-unrun-tree-".to_string(),
        }),
        Some(&restored::Warmth::ExactHit { bytes: 32_000 })
    );
    assert_eq!(
        started.get(&restored::Restore {
            job: "unrun-tests".to_string(),
            cache: "Linux-cargo-unrun-home-".to_string(),
        }),
        Some(&restored::Warmth::ExactHit { bytes: 700 })
    );
}

/// R1122 — THE PATHS ARE THE CACHE'S AND NOT THE JOB'S. A record measuring the
/// REGION — every path any of the job's caches holds, which is what the
/// comparison read until this round — prices a restore that never touched half
/// of it, and would have been accepted.
#[test]
fn a_record_measuring_the_whole_region_rather_than_its_own_cache_is_refused() {
    let mut steps = Vec::new();
    let mut caches = Vec::new();
    let mut census = census_of(&["unrun-tests", "twice-compiled"]);
    for (which, index, paths) in [
        ("home", 0usize, &["~/.cargo/registry"][..]),
        ("tree", 3usize, &["target"][..]),
    ] {
        let prefix = format!("Linux-cargo-unrun-{which}-");
        for (side, at) in [
            (restored::Side::Before, index),
            (restored::Side::After, index + 2),
        ] {
            let mut step = measuring_cache("unrun-tests", &prefix, side, at, paths);
            step.env.insert(
                restored::VARIABLE.to_string(),
                format!("/w/rustc-log/unrun-tests.{which}.restored"),
            );
            steps.push(step);
        }
        let mut declared = cache("unrun-tests", paths);
        declared.index = index + 1;
        declared.prefix = prefix.clone();
        caches.push(declared);
        // BOTH RECORDS MEASURE BOTH PATHS — the region, which is what one pair
        // around a job's whole cargo home plus build directory used to write.
        let mut written = restore_record(
            "unrun-tests",
            true,
            &[("~/.cargo/registry", 700), ("target", 32_000)],
        );
        written = written.replace("Linux-cargo-unrun-tests-", &prefix);
        census.restored.insert(
            format!("unrun-tests.{which}.restored"),
            restored::decode(&written),
        );
    }
    steps.push(wired("twice-compiled"));
    assert_eq!(
        judge(&census, &declared_of(&steps, &caches), &nothing()),
        vec![
            Refusal::RestoreRecordMeasuredOtherPaths {
                job: "unrun-tests".to_string(),
                cache: "Linux-cargo-unrun-home-".to_string(),
                measured: vec!["~/.cargo/registry".to_string(), "target".to_string()],
                declared: vec!["~/.cargo/registry".to_string()],
            },
            Refusal::RestoreRecordMeasuredOtherPaths {
                job: "unrun-tests".to_string(),
                cache: "Linux-cargo-unrun-tree-".to_string(),
                measured: vec!["~/.cargo/registry".to_string(), "target".to_string()],
                declared: vec!["target".to_string()],
            },
        ]
    );
}

/// R1122 — AND THE STEPS THAT DO NOT MEASURE OWE NOTHING. The control for the
/// law above, and it is the arm the split needs: a job's work step has no record
/// to name, because a record is one cache's and the job has two.
#[test]
fn a_step_that_measures_nothing_owes_no_record_path() {
    let mut steps = cached_job("validate", &["~/.cargo/registry"]);
    let work = steps
        .last_mut()
        .expect("the job has a step that does the work");
    assert!(
        restored::sides_measured(&work.script).is_empty(),
        "the premise: it is not one of the measuring steps"
    );
    work.env.remove(restored::VARIABLE);
    steps.extend(cached_job("unrun-tests", &["~/.cargo/registry"]));
    let declared = declared_of(
        &steps,
        &[
            cache("validate", &["~/.cargo/registry"]),
            cache("unrun-tests", &["~/.cargo/registry"]),
        ],
    );
    assert_eq!(twice_compiled::judge_wiring(&declared), Vec::new());
}

/// R1121 — A JOB'S SECOND CACHE OWES ITS OWN PAIR. Until this round the record
/// bracketed a REGION and a job with two caches had one pair around both; what
/// arrived was the sum, and the price of one could not be told from the price of
/// the other. Now each cache is measured by the pair that writes ITS record, so
/// a second cache with no pair is a restore nobody priced.
#[test]
fn a_second_cache_with_no_measuring_pair_of_its_own_is_refused() {
    let steps = cached_job("validate", &["~/.cargo/registry", "target"]);
    let mut second = cache("validate", &["target"]);
    second.index = AFTER_AT + 1;
    let declared = declared_of(&steps, &[cache("validate", &["~/.cargo/registry"]), second]);
    assert_eq!(
        twice_compiled::judge_wiring(&declared),
        vec![Refusal::AJobMeasuresOtherThanOneRestorePerCache {
            job: "validate".to_string(),
            pairs: 1,
            caches: 2,
        }],
        "the one pair prices the first cache correctly; the second is a restore \
         whose cost nobody wrote down, while the census reads as complete"
    );
}

/// THE CONTROL FOR WHAT COUNTS AS A MEASUREMENT. The step that BUILDS this
/// program names its crate, and a reader matching the directory rather than the
/// built binary would read that step as a measurement — leaving a job with two
/// of them and no way to be refused for having none.
#[test]
fn the_step_that_builds_the_measuring_program_is_not_a_measurement() {
    let mut steps = cached_job("validate", &["~/.cargo/registry"]);
    let mut building = wired_with_cache("validate");
    building.index = AFTER_AT + 2;
    building.script =
        "cargo build --release -q --manifest-path tools/restored/Cargo.toml".to_string();
    steps.push(building);
    let declared = declared_of(&steps, &[cache("validate", &["~/.cargo/registry"])]);
    assert!(twice_compiled::judge_wiring(&declared).is_empty());
}

/// AND THE WIRING LAWS REACH THE VERDICT A CONSUMER READS. `judge_wiring` is
/// separate because it needs no census; a law nothing calls refuses nothing.
#[test]
fn the_wiring_refusals_reach_the_gates_own_verdict() {
    let (declared, census) = cached_and_said(true, 1_000);
    let mut broken = declared.clone();
    let steps = broken
        .jobs
        .get_mut("validate")
        .expect("the fixture declares it");
    steps[0].index = CACHE_AT + 10;
    let refusals = judge(&census, &broken, &nothing());
    assert!(
        refusals.contains(&Refusal::AMeasuringPairBracketsOtherThanOneCache {
            job: "validate".to_string(),
            record: "/w/rustc-log/validate.restored".to_string(),
            caches: 0,
        }),
        "{refusals:?}"
    );
}

// --- holding one census against another --------------------------------------
//
// EVERY COUNT IN A CENSUS IS OF A COLD BUILD BY CONSTRUCTION — cargo runs no
// compiler for a unit that is already fresh — so what a job began with is the
// UNIT its numbers are in, and a subtraction across two different units is not a
// measurement. Round 1099 made exactly that subtraction: two censuses side by
// side, one of them called cold because its keys had moved, equal counts, and a
// 7.5 GB cache deleted that was saving 426 compilations and ten minutes. Both
// runs had been warmed through `restore-keys`.

/// A census of two jobs, each having compiled `units` units, and each having
/// started in the state named for it.
fn census_started(jobs: &[(&str, usize, Option<restored::Warmth>)]) -> Census {
    let mut census = Census::default();
    for (job, units, warmth) in jobs {
        let built: Vec<(&str, &str, &str, u64)> = (0..*units)
            .map(|n| ("crate", METADATA[n % METADATA.len()], "link", 100))
            .collect();
        census.jobs.insert((*job).to_string(), log_of(&built));
        let Some(warmth) = warmth else { continue };
        let (exact, arrived) = match warmth {
            restored::Warmth::ExactHit { bytes } => (true, *bytes),
            restored::Warmth::PrefixHit { bytes } => (false, *bytes),
            restored::Warmth::Nothing => (false, 0),
            restored::Warmth::HitThatBroughtNothing => (true, 0),
        };
        census.restored.insert(
            format!("{job}.restored"),
            restored::decode(&restore_record(job, exact, &[("target", arrived)])),
        );
    }
    census
}

/// Distinct fingerprints, so a job compiling `n` units compiles `n` DISTINCT
/// ones — the counts below are then about the comparison and not about
/// deduplication.
const METADATA: [&str; 6] = ["a1", "b2", "c3", "d4", "e5", "f6"];

/// Warm from an earlier generation. TWO OF THEM, WITH DIFFERENT SIZES, because
/// no two runs restore the same number of bytes and a fixture that gave them one
/// figure would be green under a comparison nobody could ever satisfy.
const WARM: Option<restored::Warmth> = Some(restored::Warmth::PrefixHit { bytes: 7_000 });
const WARM_AGAIN: Option<restored::Warmth> = Some(restored::Warmth::PrefixHit { bytes: 7_144 });
const COLD: Option<restored::Warmth> = Some(restored::Warmth::Nothing);

#[test]
fn two_censuses_whose_jobs_began_the_same_way_are_a_difference() {
    let held = twice_compiled::compare(
        &census_started(&[("validate", 4, WARM), ("gate", 2, None)]),
        &census_started(&[("validate", 2, WARM_AGAIN), ("gate", 2, None)]),
    );
    assert_eq!(held.incomparable, Vec::new());
    let validate = held.jobs.get("validate").expect("a comparable job");
    assert_eq!(validate.compilations, (4, 2));
    assert_eq!(validate.compilations_changed(), -2);
    assert_eq!(
        validate.started.get("Linux-cargo-validate-"),
        Some(&(
            restored::Warmth::PrefixHit { bytes: 7_000 },
            restored::Warmth::PrefixHit { bytes: 7_144 }
        )),
        "and it carries BOTH start states, because the predicate that let this \
         pair through looks at the variant and not the size"
    );
    // THE SAME SILENCE TWICE IS NOT A DIFFERENCE: a job with no cache always
    // begins with an empty tree, and a comparison that refused those would refuse
    // every pair this repository can take.
    assert_eq!(
        held.jobs.get("gate").map(|delta| delta.started.len()),
        Some(0)
    );
    let (before, after) = held.totals().expect("every job is comparable");
    assert_eq!((before.paid, after.paid), (6, 4));
}

/// THE ROUND 1099 PAIR, and the whole reason this exists.
#[test]
fn a_job_that_began_differently_is_refused_rather_than_subtracted() {
    let held = twice_compiled::compare(
        &census_started(&[("validate", 4, COLD), ("gate", 2, None)]),
        &census_started(&[("validate", 4, WARM), ("gate", 2, None)]),
    );
    assert_eq!(
        held.incomparable,
        vec![twice_compiled::Incomparable::StartedInDifferentStates {
            restore: restored::Restore {
                job: "validate".to_string(),
                cache: "Linux-cargo-validate-".to_string(),
            },
            earlier: restored::Warmth::Nothing,
            later: restored::Warmth::PrefixHit { bytes: 7_000 },
        }]
    );
    assert!(
        !held.jobs.contains_key("validate"),
        "the confounded job is not also reported as a delta"
    );
}

/// AND THE TOTALS GO WITH IT. A total is a sum over a population, so one job in
/// a different state spoils the whole of it — which is the arm that matters,
/// because the totals are what a reader quotes.
#[test]
fn the_totals_are_unreachable_while_any_job_is_incomparable() {
    let held = twice_compiled::compare(
        &census_started(&[("validate", 4, COLD), ("gate", 2, None)]),
        &census_started(&[("validate", 4, WARM), ("gate", 2, None)]),
    );
    assert_eq!(held.totals(), None);
    // The other job IS still a difference — the refusal is about the sum, not
    // about every row.
    assert!(held.jobs.contains_key("gate"));
}

#[test]
fn a_job_that_said_nothing_on_one_side_only_is_refused() {
    let held = twice_compiled::compare(
        &census_started(&[("validate", 4, WARM), ("gate", 2, None)]),
        &census_started(&[("validate", 4, None), ("gate", 2, None)]),
    );
    assert_eq!(
        held.incomparable,
        vec![
            twice_compiled::Incomparable::OnlyOneSideSaidWhatItStartedFrom {
                restore: restored::Restore {
                    job: "validate".to_string(),
                    cache: "Linux-cargo-validate-".to_string(),
                },
                silent: twice_compiled::Side::Later,
            }
        ]
    );
}

#[test]
fn a_job_in_one_census_and_not_the_other_has_nothing_to_be_subtracted_from() {
    let held = twice_compiled::compare(
        &census_started(&[("validate", 4, WARM), ("gate", 2, None)]),
        &census_started(&[("validate", 4, WARM)]),
    );
    assert_eq!(
        held.incomparable,
        vec![twice_compiled::Incomparable::OnlyInOneCensus {
            job: "gate".to_string(),
            missing: twice_compiled::Side::Later,
        }]
    );
    assert_eq!(held.totals(), None);
}

/// THE SIZE IS NOT THE STATE. No two runs restore the same number of bytes, so
/// a comparison that took the whole value would call every real pair
/// incomparable — a check nobody could ever satisfy is a check nobody keeps.
#[test]
fn two_prefix_hits_of_different_sizes_are_one_state() {
    let held = twice_compiled::compare(
        &census_started(&[
            (
                "validate",
                4,
                Some(restored::Warmth::PrefixHit { bytes: 246_000_000 }),
            ),
            ("gate", 1, None),
        ]),
        &census_started(&[
            (
                "validate",
                4,
                Some(restored::Warmth::PrefixHit {
                    bytes: 27_258_000_000,
                }),
            ),
            ("gate", 1, None),
        ]),
    );
    assert_eq!(held.incomparable, Vec::new());
    // AND THE REPORT PRINTS BOTH SIZES, because that is exactly what the
    // predicate did not look at.
    let printed = twice_compiled::render_comparison(&held, "a", "b");
    assert!(
        printed.contains("246 MB") && printed.contains("27258 MB"),
        "{printed}"
    );
}

/// TWO EMPTY DIRECTORIES COMPARE EQUAL, and that reads exactly like two runs
/// that agreed — the same shape every gate in this repository has a line about.
#[test]
fn a_comparison_that_reached_no_job_says_so_rather_than_agreeing() {
    let held = twice_compiled::compare(&Census::default(), &Census::default());
    assert_eq!(
        held.totals(),
        None,
        "ZERO AGAINST ZERO IS NOT A DIFFERENCE OF NONE. Both totals are honestly \
         empty, which is what makes them dangerous: a caller reaching them would \
         read two censuses that agreed perfectly"
    );
    let printed = twice_compiled::render_comparison(&held, "a", "b");
    assert!(printed.contains("NEITHER CENSUS HOLDS A JOB"), "{printed}");
    // AND THE REFUSAL LIST IS EMPTY, which is the trap: nothing is incomparable
    // because nothing was compared, so a `totals()` guarded on that alone signs
    // off on a measurement that did not happen.
    assert_eq!(held.incomparable, Vec::new());
}

// --- the words this gate was given -------------------------------------------

#[test]
fn a_comparison_is_read_as_two_directories_and_no_workflow() {
    let words = |all: &[&str]| {
        twice_compiled::read_arguments(&all.iter().map(|word| word.to_string()).collect::<Vec<_>>())
    };
    assert_eq!(
        words(&["compare", "earlier", "later"]),
        Ok(twice_compiled::Entrance::Compare {
            earlier: "earlier".to_string(),
            later: "later".to_string(),
        })
    );
    // A FLAG CONSUMES ITS OWN VALUE. `--workflow compare` under a reader taking
    // the first word not beginning with `--` would enter the comparison with one
    // directory named `compare`.
    assert!(matches!(
        words(&["--workflow", ".github/workflows/w.yml", "logs"]),
        Ok(twice_compiled::Entrance::Logs { ref directory, ref workflow })
            if directory == "logs" && workflow.as_deref() == Some(".github/workflows/w.yml")
    ));
    assert!(matches!(
        words(&["--replay", "scratch"]),
        Ok(twice_compiled::Entrance::Replay { ref scratch, .. }) if scratch == "scratch"
    ));
    // AND A COMPARISON HANDED A WORKFLOW IS A MISUNDERSTANDING WORTH SAYING:
    // the two censuses may be of two different commits, and a job in one and not
    // the other is part of the answer rather than something to be told.
    assert!(words(&["compare", "a", "b", "--workflow", "w.yml"]).is_err());
    assert!(words(&["compare", "a"]).is_err());
    assert!(words(&["compare", "a", "b", "c"]).is_err());
    assert!(words(&[]).is_err());
    assert!(
        words(&["--wat", "logs"]).is_err(),
        "an unknown flag is a refusal rather than a directory"
    );
}

// --- what the whole family of repairs is worth --------------------------------
//
// R1124 — THE NUMBER THAT CLOSED AN ARC AND THAT NOTHING PRINTED. R1098 measured
// that 75.4% of these jobs' compiling windows had no compiler alive in them, and
// concluded BY HAND that every compile-side repair together was worth at most
// 394.5 seconds of critical path. An arc decision was taken on that figure and
// nothing has re-derived it since — so it could not go stale loudly, and the
// question it settled came back five times under five names because no reader
// could check whether the premise still held.

/// A job whose compilations are laid out at chosen moments, so a fixture can
/// build a window with a chosen amount of idle in it.
fn log_at(placed: &[(u64, u64)]) -> JobLog {
    let text: String = placed
        .iter()
        .enumerate()
        .map(|(index, (start, micros))| {
            compiled(EPOCH + start, *micros, "crate", METADATA[index], "link")
        })
        .collect();
    read_log(&text)
}

#[test]
fn the_ceiling_is_the_busiest_jobs_own_compiling_and_not_the_sum() {
    // THESE JOBS RUN BESIDE EACH OTHER. A repair that removed EVERY compilation
    // shortens the run by at most the busiest job's own compiling — less, if some
    // other job then becomes the longest. Summing them instead would price the
    // whole family at four times what any arrangement of caches can win, which is
    // exactly the work-seconds-against-wall-clock mistake R1120 made in a report.
    let mut census = Census::default();
    census.jobs.insert("small".to_string(), log_at(&[(0, 100)]));
    census
        .jobs
        .insert("busiest".to_string(), log_at(&[(0, 400), (900, 100)]));
    census
        .jobs
        .insert("middling".to_string(), log_at(&[(0, 250)]));

    assert_eq!(
        census.ceiling_micros(),
        500,
        "`busiest` has a compiler alive for 400 + 100 µs of its window"
    );
    assert!(
        census.ceiling_micros() < census.paid_micros(),
        "the sum of what every job compiles is {} µs and is NOT this bound",
        census.paid_micros()
    );
}

#[test]
fn the_idle_share_is_a_share_of_wall_clock_and_not_of_work() {
    // TWO TRUE PERCENTAGES ABOUT ONE RUN, and only one of them says what a repair
    // can reach: the surplus share is of COMPILER-SECONDS, added up over
    // processes that ran beside each other, and this is of the CLOCK.
    let mut census = Census::default();
    // One compiler for 100 µs, then nothing until 1000 — a suite running what it
    // just built, which is the half no compile-side repair touches.
    census
        .jobs
        .insert("suite".to_string(), log_at(&[(0, 100), (900, 100)]));
    assert_eq!(census.window_micros(), 1000);
    assert_eq!(census.idle_micros(), 800);
    assert_eq!(
        census.idle_micros() + census.ceiling_micros(),
        census.window_micros(),
        "for one job the two halves are the whole window, which is what makes \
         the share quotable"
    );
}

#[test]
fn a_census_of_no_jobs_has_no_ceiling_rather_than_a_zero_one() {
    // THE EMPTY ANSWER, said out loud. `max()` over nothing is nothing, and a
    // reader that turned that into 0 would print "no repair is worth anything"
    // for a census that reached no job at all — which is the shape `judge`
    // refuses separately as `CensusCoversTooFewJobs`.
    let census = Census::default();
    assert_eq!(census.ceiling_micros(), 0);
    assert_eq!(census.window_micros(), 0);
    assert_eq!(
        judge(&census, &declared_of(&[], &[]), &nothing())
            .iter()
            .filter(|refusal| matches!(refusal, Refusal::CensusCoversTooFewJobs { .. }))
            .count(),
        1,
        "so the zero never travels as a reading — the census is refused first"
    );
}
