//! Which cargo subcommands COMPILE, asked of cargo rather than of the docs.
//!
//! R1272, and it is `locked_resolution_smoke`'s discipline on a second axis. That
//! file measures which subcommands rewrite a lockfile; this one measures which
//! of them produce or read the artifacts `cargo clean` removes, because
//! `tools/stale-artifacts` cleans a run's changed packages BEFORE the run and had
//! no way to ask whether the run was going to meet an artifact at all.
//!
//! THE OBSERVATION IS `cargo clean`'s OWN AXIS. A targeted clean removes a
//! package's `.fingerprint` entries, so "this subcommand made a fingerprint" and
//! "this subcommand has something a clean could have removed" are one question
//! rather than a proxy for one. Watching a build script instead would answer a
//! neighbouring question — R1212 used one for the width measurement — and would
//! be wrong for a subcommand that reads artifacts without running one.
//!
//! WHAT A WRONG ANSWER COSTS, in each direction. Classified as compiling when it
//! does not, a `cargo metadata` gets a clean in front of it that buys nothing and
//! costs whoever compiles next a full rebuild. Classified as not compiling when
//! it does, R743's stale binary survives into the run the freshening exists to
//! make trustworthy — which is why `ci_plan::compiles` answers `None` for a
//! subcommand it does not know and its caller treats that as "assume it does".
//!
//! # The three questions this population asks of an arriving law (R1268)
//!
//! `LAWS_OVER_THIS_POPULATION` names this file, and the list carries three
//! questions the sixth source's shapes put to anything that reads the
//! population. Answered here rather than left for a reader to work out:
//!
//! - A COMMAND A CONDITIONAL SITE CHOSE THE WORDS OF. Every way such a site can
//!   go is in the population, and this law reduces each command to its
//!   SUBCOMMAND and collects them into a set — so a site with four ways
//!   contributes one name if they share it and two if they do not. Neither is a
//!   problem for this law: what it needs is the set of names cargo will be asked
//!   for, and duplicates in that set say nothing.
//! - A COMMAND CARRYING A HOLE. A hole cannot take a word away, so it cannot
//!   remove a subcommand; and a way whose hole sits BEFORE the subcommand is
//!   dropped from the population by `RustSpawn::commands` rather than guessed
//!   at. Every subcommand this law reads is therefore one somebody wrote. It
//!   makes no ABSENCE claim about a command's words at all — its claims are
//!   about what cargo does when asked for a name, measured — so the asymmetry
//!   that governs `lock_verdict` and `decides_its_own_width` does not reach it.
//! - A COMMAND DECLARING IT RUNS OVER A TREE THAT IS NOT THIS REPOSITORY'S. Its
//!   subcommand belongs in this measurement exactly as any other does: whether
//!   `cargo check` leaves a fingerprint is a fact about cargo, not about whose
//!   tree it was pointed at. The declaration changes who owns the lockfile and
//!   changes nothing here.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use ci_plan::issue::{self, Tree};
use ci_plan::{commands_this_repository_issues, compiles};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> sits two levels under the root")
        .to_path_buf()
}

/// The minimum a subcommand needs to do anything at all in the fixture.
///
/// TAKEN FROM `locked_resolution_smoke`'s OWN LIST, which measured them against
/// this same shape of fixture. Where they differ the difference is stated: this
/// experiment does NOT pass `--no-run` to `test` or `bench`, because `--no-run`
/// is what a command says when it wants the artifacts and not the run — exactly
/// the case this table must answer `true` for, and passing it would prove the
/// weaker half by accident.
fn arguments_for(subcommand: &str) -> Vec<&'static str> {
    match subcommand {
        "metadata" => vec!["--format-version", "1"],
        "generate-lockfile" => vec!["--offline"],
        "rustdoc" => vec!["-p", "a", "--lib"],
        "test" | "bench" => vec!["--no-run"],
        "run" => vec!["--bin", "a"],
        "doc" => vec!["--no-deps"],
        "fix" => vec!["--allow-no-vcs", "--lib"],
        "clean" => vec!["-p", "a"],
        "fmt" => vec!["--check"],
        "package" => vec!["--no-verify", "-p", "a"],
        "sweep" => vec!["--stamp"],
        _ => Vec::new(),
    }
}

/// Every `.fingerprint` entry under a build directory — what a targeted clean
/// is able to remove.
fn fingerprints_under(at: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(at) else {
        return 0;
    };
    let mut found = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if path.file_name().is_some_and(|name| name == ".fingerprint") {
            found += std::fs::read_dir(&path).map(Iterator::count).unwrap_or(0);
            continue;
        }
        found += fingerprints_under(&path);
    }
    found
}

/// Run one subcommand against a FRESH build directory and say whether it left
/// anything a clean could remove. `None` when this machine has no such
/// subcommand.
fn compiles_in_fixture(
    fixture: &Path,
    home: &Path,
    subcommand: &str,
    arguments: &[&str],
) -> Option<bool> {
    // A BUILD DIRECTORY PER ARM. Sharing one would let the fingerprints of the
    // previous subcommand answer for this one — the measurement would then say
    // every subcommand after the first compiles, which is a result that looks
    // like a finding.
    let target = home.join("build");
    let _ = std::fs::remove_dir_all(&target);
    let mut cargo = issue::cargo(Tree::MadeByThisRun(
        "the one-package fixture this measurement builds, with a fresh build \
         directory per arm so each subcommand answers only for itself",
    ));
    let out = cargo
        .arg(subcommand)
        .args(arguments)
        .current_dir(fixture)
        .env("CARGO_TARGET_DIR", &target)
        .env("CARGO_HOME", home.join("cargo-home"))
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .output()
        .expect("cargo runs");
    let said = String::from_utf8_lossy(&out.stderr);
    if said.contains("no such command") || said.contains("no such subcommand") {
        return None;
    }
    Some(fingerprints_under(&target) > 0)
}

#[test]
fn what_a_subcommand_leaves_for_a_clean_to_remove_is_asked_of_cargo() {
    let root = repository_root();
    // THE POPULATION IS THE TREE'S, not a list written here. A subcommand this
    // repository stops issuing leaves this measurement with it, and one it
    // starts issuing arrives here without anybody remembering to add it — which
    // is the whole reason `ci_plan` assembles the population once.
    let mut issued: BTreeSet<String> = commands_this_repository_issues(&root)
        .commands
        .iter()
        .filter_map(|command| command.subcommand().map(str::to_string))
        .collect();
    assert!(
        issued.len() >= 4,
        "this repository issues more than four distinct subcommands, so a \
         shorter list is a walk that stopped: {issued:?}"
    );
    // AND EVERY NAME THE TABLE SAYS COMPILES NOTHING, whether this repository
    // issues it or not. That half of the table is what makes a caller SKIP a
    // clean, so an entry nobody measured is a stale artifact surviving into the
    // run the freshening exists to make trustworthy. The other half needs no
    // such sweep: assuming a subcommand compiles is the conservative answer and
    // costs only a rebuild.
    issued.extend(
        ci_plan::COMPILES_NOTHING
            .iter()
            .map(|name| (*name).to_string()),
    );

    let home = tempfile::tempdir().expect("temp dir");
    let fixture = home.path().join("compiling");
    std::fs::create_dir_all(fixture.join("a/src")).expect("fixture tree");
    std::fs::write(
        fixture.join("Cargo.toml"),
        "[workspace]\nmembers = [\"a\"]\nresolver = \"2\"\n",
    )
    .expect("workspace manifest");
    std::fs::write(
        fixture.join("a/Cargo.toml"),
        "[package]\nname = \"a\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("member manifest");
    std::fs::write(fixture.join("a/src/lib.rs"), "pub fn f() {}\n").expect("member source");
    std::fs::write(fixture.join("a/src/main.rs"), "fn main() {}\n").expect("member binary");

    let mut unmeasurable = Vec::new();
    let mut reached = BTreeSet::new();
    let mut measured = 0;
    for subcommand in &issued {
        let arguments = arguments_for(subcommand);
        let Some(left_artifacts) =
            compiles_in_fixture(&fixture, home.path(), subcommand, &arguments)
        else {
            unmeasurable.push(subcommand.clone());
            continue;
        };
        measured += 1;
        reached.insert(subcommand.clone());
        println!(
            "cargo {subcommand}: {} for a clean to remove",
            if left_artifacts {
                "LEFT something"
            } else {
                "left nothing"
            }
        );
        let claimed = compiles(subcommand)
            .unwrap_or_else(|| panic!("`cargo {subcommand}` is issued here and unclassified"));
        assert_eq!(
            left_artifacts,
            claimed,
            "`cargo {subcommand}` was measured leaving {}artifacts a targeted \
             clean removes, and `ci_plan::compiles` says {claimed} — a freshening \
             pass reads that table to decide whether cleaning in front of this \
             command buys anything",
            if left_artifacts { "" } else { "no " }
        );
    }

    assert!(
        measured >= 4,
        "the point of this test is the measurement; {measured} of {} subcommands \
         reached cargo (unmeasurable: {unmeasurable:?})",
        issued.len()
    );
    // A SUBCOMMAND THIS MACHINE HAS NO COPY OF CANNOT BE MEASURED, and the only
    // ones that may be in that state are the third-party ones this repository
    // installs rather than ships. `cargo sweep` is the case and it is named here
    // rather than left to a predicate that would also excuse a built-in going
    // missing — which is the shape of a measurement quietly measuring nothing.
    assert!(
        unmeasurable.iter().all(|subcommand| subcommand == "sweep"),
        "a subcommand this machine cannot run is one whose classification is \
         unchecked here; only the third-party ones may be in that state: \
         {unmeasurable:?}"
    );

    // AND EVERY `false` IN THE TABLE WAS REACHED BY THIS RUN, which is the
    // assertion the loop above cannot make for itself: a name dropped from the
    // population is a name this test never runs and never complains about, so
    // "the sweep happened" has to be checked against the list rather than
    // inferred from the loop having gone round. An unmeasured `false` is a
    // freshening pass deciding to skip on nobody's authority.
    let unswept: Vec<&&str> = ci_plan::COMPILES_NOTHING
        .iter()
        .filter(|name| !reached.contains(**name) && **name != "sweep")
        .collect();
    assert!(
        unswept.is_empty(),
        "{} name(s) the table says compile nothing were never put to cargo by \
         this run: {unswept:?} — and that half of the table is the half that \
         makes a caller SKIP a clean",
        unswept.len()
    );

    // AND BOTH ANSWERS ARE PRESENT IN WHAT WAS MEASURED. A table that answered
    // one way for everything would pass every assertion above while saying
    // nothing, and the direction it would fail towards is the silent one: all
    // `false` means the freshening pass never runs again.
    let (yes, no): (Vec<&String>, Vec<&String>) = issued
        .iter()
        .filter(|subcommand| !unmeasurable.contains(subcommand))
        .partition(|subcommand| compiles(subcommand) == Some(true));
    assert!(
        !yes.is_empty() && !no.is_empty(),
        "the measurement separates this repository's subcommands into those a \
         clean is worth running in front of and those it is not, and one of the \
         two sides is empty: compiles {yes:?}, does not {no:?}"
    );
}
