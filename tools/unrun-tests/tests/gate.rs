//! What this gate does, proved on trees built for the purpose.
//!
//! The two mechanisms that decide every verdict are the ones a real repository
//! cannot exercise on demand: the `#[ignore]` subtraction needs an ignored test
//! to subtract, and `--all-features` being load-bearing needs a test that hides
//! behind a feature. Adding either to this repository to test the gate would
//! create the very thing the gate rejects — an `#[ignore]` nobody runs — so both
//! live in fixture crates compiled for one assertion and thrown away.
//!
//! The fixtures are tiny and depend on nothing, which is what keeps this
//! affordable: the gate's own workspace is compiled by the side-workspaces job,
//! and a test here that built the root workspace would put that job's whole cost
//! into it a second time.

use std::path::{Path, PathBuf};

use ci_plan::CargoCommand;
use unrun_tests::{population_command, probe, Refusal, Report, TestId};

/// A crate with one plain test, one `#[ignore]`d test, and one behind a feature.
fn fixture(at: &Path) -> PathBuf {
    let src = at.join("src");
    std::fs::create_dir_all(&src).expect("create fixture src");
    std::fs::write(
        at.join("Cargo.toml"),
        "[workspace]\n\
         \n\
         [package]\n\
         name = \"fixture\"\n\
         version = \"0.1.0\"\n\
         edition = \"2021\"\n\
         \n\
         [features]\n\
         extra = []\n",
    )
    .expect("write fixture manifest");
    std::fs::write(
        src.join("lib.rs"),
        "/// A doc-test has no test binary at all, so it is the one shape the\n\
         /// artifact record cannot report.\n\
         ///\n\
         /// ```\n\
         /// assert_eq!(1 + 1, 2);\n\
         /// ```\n\
         pub fn documented() {}\n\
         \n\
         #[cfg(test)]\n\
         mod tests {\n\
         \x20   #[test]\n\
         \x20   fn plain() {}\n\
         \n\
         \x20   #[test]\n\
         \x20   #[ignore]\n\
         \x20   fn only_when_asked_for() {}\n\
         \n\
         \x20   #[cfg(feature = \"extra\")]\n\
         \x20   #[test]\n\
         \x20   fn only_with_the_feature() {}\n\
         }\n",
    )
    .expect("write fixture source");
    at.join("Cargo.toml")
}

fn command(manifest: &Path, extra: &[&str], harness: &[&str]) -> CargoCommand {
    let mut cargo_args = vec![
        "cargo".to_string(),
        "test".to_string(),
        "--manifest-path".to_string(),
        manifest.display().to_string(),
    ];
    cargo_args.extend(extra.iter().map(|word| (*word).to_string()));
    CargoCommand {
        source: "tests/gate.rs".to_string(),
        owner: "fixture".to_string(),
        cargo_args,
        harness_args: harness.iter().map(|word| (*word).to_string()).collect(),
        env: Default::default(),
    }
}

fn names(tests: &std::collections::BTreeSet<TestId>) -> Vec<&str> {
    let mut out: Vec<&str> = tests.iter().map(|test| test.name.as_str()).collect();
    out.sort_unstable();
    out
}

fn scratch(name: &str) -> PathBuf {
    let at = std::env::temp_dir().join(format!("unrun-tests-{name}"));
    let _ = std::fs::remove_dir_all(&at);
    at
}

#[test]
fn a_command_runs_what_it_lists_minus_what_it_leaves_ignored() {
    let at = scratch("ignored");
    let manifest = fixture(&at);
    let mut refusals = Vec::new();
    let found = probe(Path::new("/"), &command(&manifest, &[], &[]), &mut refusals)
        .expect("the fixture builds");
    assert!(refusals.is_empty(), "{refusals:?}");

    assert_eq!(
        names(&found.listed),
        vec![
            "src/lib.rs - documented (line 4)",
            "tests::only_when_asked_for",
            "tests::plain",
        ],
        "`--list` prints the ignored test too, which is why listing is not \
         enough to know what a command runs"
    );
    assert_eq!(
        names(&found.executed),
        vec!["src/lib.rs - documented (line 4)", "tests::plain"],
        "and the ignored one is not run, so it must not be credited"
    );
    // NON-VACUITY. If these were equal the subtraction would never have fired
    // and this test would pass while proving nothing.
    assert_ne!(
        found.listed, found.executed,
        "the subtraction did not happen at all"
    );
    let _ = std::fs::remove_dir_all(&at);
}

#[test]
fn a_command_that_asks_for_the_ignored_ones_runs_them() {
    let at = scratch("asks");
    let manifest = fixture(&at);
    let mut refusals = Vec::new();
    let found = probe(
        Path::new("/"),
        &command(&manifest, &[], &["--ignored"]),
        &mut refusals,
    )
    .expect("the fixture builds");
    assert!(refusals.is_empty(), "{refusals:?}");

    assert_eq!(
        names(&found.listed),
        vec!["tests::only_when_asked_for"],
        "`--ignored` narrows the listing to the ignored ones"
    );
    assert_eq!(
        names(&found.executed),
        names(&found.listed),
        "and such a command RUNS what it lists — subtracting again here would \
         report a test that runs as one that does not, which is how the \
         evidence-replay job's only test would read as dark"
    );
    let _ = std::fs::remove_dir_all(&at);
}

#[test]
fn the_population_probe_sees_what_a_default_build_hides() {
    let at = scratch("features");
    let manifest = fixture(&at);
    let mut refusals = Vec::new();

    let default = probe(Path::new("/"), &command(&manifest, &[], &[]), &mut refusals)
        .expect("the fixture builds");
    let population = probe(
        Path::new("/"),
        &population_command(&manifest.display().to_string()),
        &mut refusals,
    )
    .expect("the fixture builds with every feature");
    assert!(refusals.is_empty(), "{refusals:?}");

    assert!(
        !names(&default.listed).contains(&"tests::only_with_the_feature"),
        "a test behind a feature is invisible to the build that does not enable \
         it — this is the shape that hid nine `mnemosyne-server` tests"
    );
    assert!(
        names(&population.listed).contains(&"tests::only_with_the_feature"),
        "and the population probe must see it, or the gate's own census is the \
         thing with the hole: {:?}",
        names(&population.listed)
    );
    let _ = std::fs::remove_dir_all(&at);
}

#[test]
fn a_probe_that_cannot_build_refuses_instead_of_reporting_nothing() {
    let at = scratch("broken");
    let manifest = fixture(&at);
    std::fs::write(at.join("src").join("lib.rs"), "fn ( this is not rust")
        .expect("break the fixture");

    let mut refusals = Vec::new();
    let found = probe(Path::new("/"), &command(&manifest, &[], &[]), &mut refusals);
    assert!(
        found.is_none(),
        "a tree that did not compile has no coverage to report"
    );
    assert!(
        matches!(refusals.as_slice(), [Refusal::BuildFailed { .. }]),
        "the answer is REFUSED, not clean — a gate that cannot look and a gate \
         that looked and found nothing print the same silence otherwise: \
         {refusals:?}"
    );
    let _ = std::fs::remove_dir_all(&at);
}

#[test]
fn a_test_with_no_binary_is_still_a_test() {
    // THE HOLE THIS GATE FOUND IN ITSELF. Its first run over this repository
    // reported a population 13 short of what `cargo test -- --list` prints, and
    // the difference was doc-tests: cargo's artifact record never mentions one,
    // so they were missing from the population AND from the run set at once.
    // Symmetric invisibility produces no finding, which is why it needs a test
    // rather than a reading of the output.
    let at = scratch("doc");
    let manifest = fixture(&at);
    let mut refusals = Vec::new();
    let found = probe(Path::new("/"), &command(&manifest, &[], &[]), &mut refusals)
        .expect("the fixture builds");
    assert!(refusals.is_empty(), "{refusals:?}");

    let doc: Vec<&str> = found
        .listed
        .iter()
        .filter(|test| test.name.contains("src/lib.rs - "))
        .map(|test| test.name.as_str())
        .collect();
    assert_eq!(
        doc.len(),
        1,
        "the fixture documents exactly one runnable example: {:?}",
        names(&found.listed)
    );
    assert!(
        found.binaries.iter().all(|binary| binary.kind != "doctest"),
        "and it came from no binary, which is the whole reason it needs its own \
         probe: {:?}",
        found.binaries
    );
    assert!(
        found.executed.iter().any(|test| test.name == doc[0]),
        "a doc-test this command runs must be credited as run"
    );
    let _ = std::fs::remove_dir_all(&at);
}

#[test]
fn a_command_narrowed_to_one_target_runs_no_doc_test() {
    // `cargo test --test x --doc` is not a command; asking cargo for doc-tests
    // under a target selector would make the gate refuse on a command CI runs
    // happily. The evidence-replay job's two commands are exactly this shape.
    let at = scratch("narrowed");
    let manifest = fixture(&at);
    let mut refusals = Vec::new();
    let found = probe(
        Path::new("/"),
        &command(&manifest, &["--lib"], &[]),
        &mut refusals,
    )
    .expect("the fixture builds");
    assert!(
        refusals.is_empty(),
        "a narrowed command must not be asked for doc-tests: {refusals:?}"
    );
    assert!(
        !found
            .listed
            .iter()
            .any(|test| test.name.contains("src/lib.rs - ")),
        "and it does not run them, so it must not be credited with them: {:?}",
        names(&found.listed)
    );
    let _ = std::fs::remove_dir_all(&at);
}

#[test]
fn a_package_with_no_library_has_no_doc_tests_and_that_is_an_answer() {
    // `cargo test --doc` on a bin-only package exits non-zero saying there is
    // no library. Reading that as "the gate could not look" made this gate
    // REFUSE over three of this repository's own workspaces — rejecting for a
    // reason outside its own law, which is the shape R1081 shipped and had to
    // repair inside the round. Having no library is a complete answer.
    let at = scratch("binonly");
    let src = at.join("src");
    std::fs::create_dir_all(&src).expect("create fixture src");
    std::fs::write(
        at.join("Cargo.toml"),
        "[workspace]\n\n[package]\nname = \"binonly\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write fixture manifest");
    std::fs::write(
        src.join("main.rs"),
        "/// ```\n/// assert!(true);\n/// ```\nfn main() {}\n\n\
         #[cfg(test)]\nmod tests {\n    #[test]\n    fn plain() {}\n}\n",
    )
    .expect("write fixture source");

    let mut refusals = Vec::new();
    let found = probe(
        Path::new("/"),
        &command(&at.join("Cargo.toml"), &[], &[]),
        &mut refusals,
    )
    .expect("the fixture builds");
    assert!(
        refusals.is_empty(),
        "a bin-only package must not make this gate refuse: {refusals:?}"
    );
    assert_eq!(
        names(&found.listed),
        vec!["tests::plain"],
        "its ordinary tests are still counted, and cargo runs no doc-test for a \
         package with no library — including the example written above `main`"
    );
    let _ = std::fs::remove_dir_all(&at);
}

#[test]
fn the_verdict_and_the_findings_are_two_different_answers() {
    // "I could not judge" and "I judged and found nothing" are the same exit
    // code unless something separates them, and this is where they separate.
    // Both were computed and neither was checked until a sweep went looking for
    // an injection that would turn a test red and found none aimed here.
    let clean = Report::default();
    assert!(
        clean.verdict().is_ok(),
        "no refusal means the gate judged, whatever it found"
    );
    let refused = Report {
        refusals: vec![Refusal::EmptyPopulation],
        ..Report::default()
    };
    assert!(
        refused.verdict().is_err(),
        "one refusal is enough: a gate that read part of the tree has no verdict \
         about the rest of it"
    );

    let compiled = TestId {
        target_src: PathBuf::from("crates/x/tests/smoke.rs"),
        name: "runs_nowhere".to_string(),
    };
    let elsewhere = TestId {
        target_src: PathBuf::from("crates/x/tests/smoke.rs"),
        name: "runs_somewhere".to_string(),
    };
    let found = Report {
        population: [compiled.clone(), elsewhere.clone()].into_iter().collect(),
        run: [(elsewhere, "a job".to_string())].into_iter().collect(),
        ..Report::default()
    };
    assert_eq!(
        found.dark(),
        vec![&compiled],
        "dark is exactly what this repository compiles and no command runs — \
         not what it runs, which is the inversion that would report a covered \
         repository as an uncovered one and pass every other test here"
    );
}

#[test]
fn the_population_probe_asks_for_every_feature() {
    // Pinned against the flag, because it is the whole reason the population is
    // bigger than any one CI build. R1082's job exists for the same flag.
    let built = population_command("bench/Cargo.toml");
    assert!(built.has("--all-features"), "{:?}", built.cargo_args);
    assert!(built.has("--workspace"), "{:?}", built.cargo_args);
    assert_eq!(built.value(&["--manifest-path"]), Some("bench/Cargo.toml"));
    assert!(
        built.harness_args.is_empty(),
        "the population is every test the workspace holds, so the harness is \
         given no filter"
    );
}
