//! A suite nobody wrapped is a suite nobody judged.
//!
//! R1196. `tools/unreported-targets` answers whether a completed run's verdict
//! covered every test target that run compiled — the law R1194 built after
//! R1177 shipped three defects behind a first failing target and CI found them a
//! round later. The MECHANISM went into `scripts/verify.sh`, which is where a
//! round's own verification command goes, and that left the law reaching exactly
//! as far as somebody remembering to type the wrapper. Every `cargo test` in the
//! workflows and every separate workspace's suite — the largest commands in the
//! tree, and the ones whose failures reach `main` — were outside it.
//!
//! What blocked closing that is what this round removed: `ci_plan::parse_script`
//! counted a segment only when its FIRST word was `cargo`, so wrapping a command
//! DELETED it from every population built on that crate. It now reads a bare
//! `--` as a hand-over and keeps the carrier, which is the datum the law below
//! reads.
//!
//! Two halves, and the second is what stops the first from being a spelling
//! check: every `cargo test` this repository issues names the wrapper, AND the
//! wrapper it names is the one that runs the coverage gate.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use ci_plan::{
    declared_build_commands, lister_declared_commands, script_cargo_commands, tracked_manifests,
    workflow_cargo_commands, workspaces, CargoCommand, ManifestTarget, BUILD_DECLARATION,
};

/// The wrapper, as this repository's own path names it.
const WRAPPER: &str = "scripts/verify.sh";

/// The gate it runs, as a manifest this repository tracks.
const COVERAGE_GATE: &str = "tools/unreported-targets/Cargo.toml";

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> sits two levels under the root")
        .to_path_buf()
}

/// Every cargo command this repository issues, from all three places one can be
/// written.
///
/// `scripts/check-side-workspaces.sh` is read from the LISTER'S OUTPUT and not
/// from its text, the same split `locked_resolution_smoke` makes: that script
/// assembles its commands at runtime, so its source says which words exist and
/// only its output says what they expand to.
fn everything_this_repository_issues(root: &Path) -> Vec<CargoCommand> {
    let mut commands = workflow_cargo_commands(root);
    commands.extend(lister_declared_commands(&workspaces(root)));
    commands.extend(
        script_cargo_commands(root)
            .into_iter()
            .filter(|command| command.source != "scripts/check-side-workspaces.sh"),
    );
    // R1197 — a build machine running this repository's suite is a run whose
    // coverage matters exactly as much as CI's; the words are just written
    // somewhere neither the workflows nor the scripts reach.
    commands.extend(declared_build_commands(root));
    commands
}

/// Does something hand this command over, and is that something the wrapper?
///
/// A path rather than a word, because the three callers spell it three ways: a
/// workflow writes `./scripts/verify.sh`, the workspace lister expands an
/// absolute path resolved from its own checkout, and a hook would write it
/// relative to the repository root. All three name one file.
fn judged_for_coverage(command: &CargoCommand) -> bool {
    command
        .carrier
        .first()
        .is_some_and(|program| program == WRAPPER || program.ends_with(&format!("/{WRAPPER}")))
}

#[test]
fn every_test_run_this_repository_issues_goes_through_the_wrapper_that_judges_it() {
    let root = repository_root();
    let commands = everything_this_repository_issues(&root);
    let suites: Vec<&CargoCommand> = commands
        .iter()
        .filter(|command| command.subcommand() == Some("test"))
        .collect();

    let unwrapped: Vec<String> = suites
        .iter()
        .filter(|command| !judged_for_coverage(command))
        .map(|command| format!("{} — {}", command.origin(), command.rendered()))
        .collect();
    assert!(
        unwrapped.is_empty(),
        "`cargo test` stops at the first target it cannot get past, so a run's \
         verdict is one target's number and the targets behind it were never \
         asked — `{WRAPPER}` is what holds that log against what cargo says the \
         command compiles, and a command it does not carry is one nothing \
         judges:\n  {}",
        unwrapped.join("\n  ")
    );

    // NON-VACUITY, PER SOURCE. This population is three walks bolted together
    // and the law reads as satisfied by an empty one — which is exactly what a
    // reader that stopped seeing wrapped commands would produce, and the defect
    // this round's `parse_script` change exists to prevent.
    let mut per_source: BTreeSet<&str> = BTreeSet::new();
    for command in &suites {
        per_source.insert(command.source.as_str());
    }
    assert!(
        per_source.contains("scripts/check-side-workspaces.sh"),
        "every separate in-repo workspace's suite is a `cargo test` this \
         repository issues, and a run that saw none of them read nothing: \
         {per_source:?}"
    );
    assert!(
        per_source
            .iter()
            .any(|source| source.starts_with(".github/workflows/")),
        "so is every suite in the workflows: {per_source:?}"
    );
    assert!(
        per_source.contains(BUILD_DECLARATION),
        "and so is the suite a build machine runs — the words are written in a \
         tracked file of this repository, so the law reaches them: {per_source:?}"
    );
    assert!(
        suites.len() >= 20,
        "this repository issues a suite per separate workspace plus the \
         workflows', and {} of them is a walk that stopped — in the direction \
         that reads as compliance",
        suites.len()
    );

    // THE MIRROR: the law is about TEST runs and not about every cargo command,
    // so a repository that wrapped everything would pass it for the wrong
    // reason. `cargo fmt`, `cargo clippy` and `cargo run` compile no test target
    // and have no coverage to judge; the gate calls them vacuous.
    let unwrapped_others = commands
        .iter()
        .filter(|command| command.subcommand() != Some("test"))
        .filter(|command| !judged_for_coverage(command))
        .count();
    assert!(
        unwrapped_others >= 5,
        "a command that runs no test target has no coverage for the wrapper to \
         judge, so this law must not be reading `everything is wrapped`: only \
         {unwrapped_others} of the non-test commands are issued directly"
    );
}

#[test]
fn the_wrapper_that_law_names_is_the_one_that_runs_the_coverage_gate() {
    // THE HALF THAT TIES THE NAME TO THE MECHANISM. Above is a check on a path
    // spelled in five files; on its own it would go on passing after
    // `scripts/verify.sh` stopped judging anything, which is the shape where a
    // gate keeps its shape and loses its content.
    let root = repository_root();
    let tracked = tracked_manifests(&root);
    let gate_calls: Vec<CargoCommand> = script_cargo_commands(&root)
        .into_iter()
        .filter(|command| command.source == WRAPPER)
        .filter(|command| {
            command.manifest(&tracked) == ManifestTarget::Named(COVERAGE_GATE.to_string())
        })
        .collect();
    assert_eq!(
        gate_calls.len(),
        1,
        "`{WRAPPER}` is named by every test run in this repository BECAUSE it \
         asks `{COVERAGE_GATE}` what the run covered; a copy of it that no \
         longer does is a wrapper the law above cannot tell from this one"
    );
    assert!(
        gate_calls[0].harness_has("--log"),
        "and it hands that gate the run's own log, which is the evidence side of \
         the comparison: {:?}",
        gate_calls[0].harness_args
    );
}
