//! A gate that resolves freely repairs the evidence the next gate reads.
//!
//! R1115. `scripts/check-side-workspaces.sh` runs five checks over each separate
//! workspace, in this order: fmt, clippy, item citations, blind waits, suite.
//! Only the last said `--locked`. A cargo command without that flag REWRITES a
//! lockfile that disagrees with its manifests instead of failing on it, so the
//! clippy two steps earlier repaired the file the suite was going to check, and
//! the one `--locked` in the gate was structurally unable to fail. What that
//! cost, measured rather than argued: `studio/Cargo.lock` was stale in the tree
//! for an unknown number of rounds, and the drift surfaced only as a dirty
//! working tree that was nearly swept into an unrelated commit.
//!
//! Three things are asserted here, and the first is the one the other two rest
//! on — every claim about what cargo does to a lockfile is MEASURED against
//! cargo, in a workspace built for the purpose, rather than read out of the
//! documentation.
//!
//! 1. Which subcommands rewrite a disagreeing lockfile, and which reject
//!    `--locked` outright, is asked of the cargo on this machine.
//! 2. Every cargo command this repository issues over a workspace it can pin
//!    says `--locked`; every one over a workspace it cannot does not.
//! 3. A workspace this repository cannot pin does not track a lockfile — it has
//!    nowhere to keep a file every gate run rewrites.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use ci_plan::{
    commands_this_repository_issues, lock_verdict, resolves_the_lockfile, tracked_manifests,
    workspaces, CargoCommand, LockVerdict, Ownership, BUILD_DECLARATION,
};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> sits two levels under the root")
        .to_path_buf()
}

/// Everything this repository issues: the workflows GitHub runs, the shell the
/// hooks and scripts run, the commands the workspace lister declares, and the
/// build machine's.
///
/// THE THIRD SOURCE IS NOT A DUPLICATE OF THE SECOND. The lister's commands are
/// assembled at runtime — `cargo clippy --manifest-path "$ws/Cargo.toml"
/// "${locked[@]}"` — so its shell says which words exist and only its output
/// says what they expand to. Reading its text instead would ask whether a
/// variable is spelled `--locked`.
///
/// R1212 moved the assembly into `ci-plan`: a second law now asks about the same
/// population, and two assemblies are two answers.
///
/// AND WHAT THIS MACHINE COULD NOT REACH IS SAID HERE (R1228). The lister's
/// answer is the population's third source, so on a hosted runner it is eight
/// commands smaller than it is on a workstation holding the sibling checkout.
/// The per-source floors below survive that — they are floors — but a reader of
/// one run could not tell the two machines apart, and this law's whole subject
/// is a command nobody looked at.
fn everything_this_repository_issues(root: &Path) -> Vec<CargoCommand> {
    let issued = commands_this_repository_issues(root);
    for skipped in &issued.skipped {
        println!("[locked-resolution] {}", skipped.was_not("judged"));
    }
    issued.commands
}

fn workspaces_this_repository_cannot_pin(root: &Path) -> BTreeSet<String> {
    workspaces(root)
        .ownership
        .into_iter()
        .filter(|(_, ownership)| matches!(ownership, Ownership::Foreign(_)))
        .map(|(directory, _)| directory)
        .collect()
}

#[test]
fn every_command_this_repository_issues_pins_the_lockfiles_it_can() {
    let root = repository_root();
    let tracked = tracked_manifests(&root);
    let foreign = workspaces_this_repository_cannot_pin(&root);
    let commands = everything_this_repository_issues(&root);

    let mut tally: BTreeMap<String, usize> = BTreeMap::new();
    let mut broken = Vec::new();
    for command in &commands {
        let verdict = lock_verdict(command, &tracked, &foreign);
        *tally
            .entry(
                format!("{verdict:?}")
                    .split('(')
                    .next()
                    .unwrap()
                    .to_string(),
            )
            .or_default() += 1;
        match verdict {
            LockVerdict::Pinned | LockVerdict::ResolvesNothing | LockVerdict::NotOursToPin => {}
            other => broken.push(format!(
                "{} — {} => {other:?}",
                command.origin(),
                command.rendered()
            )),
        }
    }

    assert!(
        broken.is_empty(),
        "a command that resolves without `--locked` rewrites a lockfile that \
         disagrees with its manifests, so the check after it reads a repaired \
         file and the gate cannot fail:\n  {}\ntally: {tally:?}",
        broken.join("\n  ")
    );

    // NON-VACUITY, AND IT IS PER SOURCE. A total is the wrong instrument here:
    // this population is three walks bolted together, and dropping one of them
    // takes the count down without taking it to zero — a change that deletes
    // every hook command still leaves fifty workflow commands and reads as a
    // clean run. So each source has to be seen to have arrived.
    let mut per_source: BTreeMap<&str, usize> = BTreeMap::new();
    for command in &commands {
        let kind = if command.source.starts_with(".github/workflows/") {
            "a workflow"
        } else if command.source == "scripts/check-side-workspaces.sh" {
            "the workspace lister"
        } else if command.source == BUILD_DECLARATION {
            "the build-machine declaration"
        } else {
            "a tracked script"
        };
        *per_source.entry(kind).or_default() += 1;
    }
    // The build machine declares three commands and the others are walks, so its
    // floor is its own — but it is still a floor, and a walk that stopped
    // reading it would take this to zero rather than to two.
    assert!(
        per_source
            .get("the build-machine declaration")
            .copied()
            .unwrap_or(0)
            >= 3,
        "the build-machine declaration issues this repository's own suite, and a \
         run that found under three of its commands stopped reading: {per_source:?}"
    );
    for kind in ["a workflow", "the workspace lister", "a tracked script"] {
        assert!(
            per_source.get(kind).copied().unwrap_or(0) >= 5,
            "{kind} issues cargo commands in this repository, so a walk that \
             found under five of them stopped reading — and it stops in the \
             direction that reads as compliance: {per_source:?}"
        );
    }
    assert!(
        tally.contains_key("ResolvesNothing"),
        "`cargo fmt` is issued by the hooks and the lister and REJECTS \
         `--locked`, so a run with none of it under the law is one where the \
         exempt arm was never taken and the law is `--locked` everywhere: \
         {tally:?}"
    );
}

/// The measurement everything above rests on.
///
/// One workspace, one package, no dependencies — so this needs no network and
/// no registry — and a lockfile naming a version the manifest does not. Every
/// subcommand this repository issues is run against a fresh copy of that
/// disagreement twice: once free, to see whether it REWRITES the file, and once
/// with `--locked`, to see whether it REFUSES.
#[test]
fn what_a_free_resolve_does_to_a_disagreeing_lockfile_is_asked_of_cargo() {
    let root = repository_root();
    let issued: BTreeSet<String> = everything_this_repository_issues(&root)
        .iter()
        .filter_map(|command| command.subcommand().map(str::to_string))
        .collect();
    assert!(
        issued.len() >= 4,
        "this repository issues more than four distinct subcommands, so a \
         shorter list is a walk that stopped: {issued:?}"
    );

    let home = tempfile::tempdir().expect("temp dir");
    let fixture = home.path().join("pinned");
    std::fs::create_dir_all(fixture.join("a/src")).expect("fixture tree");
    std::fs::write(
        fixture.join("Cargo.toml"),
        "[workspace]\nmembers = [\"a\"]\nresolver = \"2\"\n",
    )
    .expect("workspace manifest");
    std::fs::write(
        fixture.join("a/Cargo.toml"),
        "[package]\nname = \"a\"\nversion = \"0.2.0\"\nedition = \"2021\"\n",
    )
    .expect("member manifest");
    std::fs::write(fixture.join("a/src/lib.rs"), "pub fn f() {}\n").expect("member source");
    std::fs::write(fixture.join("a/src/main.rs"), "fn main() {}\n").expect("member binary");
    // The disagreement: the manifest says 0.2.0 and this says 0.1.0.
    let stale = "version = 4\n\n[[package]]\nname = \"a\"\nversion = \"0.1.0\"\n";

    let mut unmeasurable = Vec::new();
    let mut measured = 0;
    for subcommand in &issued {
        let arguments = arguments_for(subcommand);
        let free = run_in_fixture(&fixture, home.path(), stale, subcommand, &arguments, false);
        let (Some(rewrote), Some(refused)) = (
            free,
            run_in_fixture(&fixture, home.path(), stale, subcommand, &arguments, true),
        ) else {
            unmeasurable.push(subcommand.clone());
            continue;
        };
        measured += 1;
        println!(
            "cargo {subcommand}: free resolve {} the lockfile, `--locked` {} it",
            if rewrote { "REWROTE" } else { "left" },
            if refused { "REFUSED" } else { "accepted" }
        );
        let claimed = resolves_the_lockfile(subcommand)
            .unwrap_or_else(|| panic!("`cargo {subcommand}` is issued here and unclassified"));
        assert_eq!(
            rewrote,
            claimed,
            "`cargo {subcommand}` was measured {}rewriting a lockfile it \
             disagrees with, and `ci_plan::resolves_the_lockfile` says {claimed}",
            if rewrote { "" } else { "not " }
        );
        assert_eq!(
            refused,
            claimed,
            "`cargo {subcommand}` {} `--locked`, so a law that {} the flag there \
             is one no correct command can satisfy",
            if refused {
                "refuses a disagreeing lockfile under"
            } else {
                "does not honour"
            },
            if claimed {
                "did not require"
            } else {
                "required"
            }
        );
    }

    assert!(
        measured >= 4,
        "the point of this test is the measurement; {measured} of {} subcommands \
         reached cargo (unmeasurable: {unmeasurable:?})",
        issued.len()
    );
    assert!(
        unmeasurable
            .iter()
            .all(|subcommand| resolves_the_lockfile(subcommand) == Some(false)),
        "a subcommand this machine cannot run is one whose classification is \
         unchecked here, and the only ones that may be in that state are the \
         third-party ones this repository installs rather than ships: \
         {unmeasurable:?}"
    );

    // AND THE CHEAP CHECK THAT IS NOT ONE. `cargo metadata --no-deps` looks like
    // a fast way to ask whether a lockfile is fresh, and it answers yes on a
    // lockfile that is not: it never resolves, so `--locked` has nothing to
    // refuse. Pinned here so that a later round reaching for it finds out from
    // a red test rather than from a gate that passes everything.
    assert_eq!(
        run_in_fixture(
            &fixture,
            home.path(),
            stale,
            "metadata",
            &["--no-deps", "--format-version", "1"],
            true,
        ),
        Some(false),
        "`cargo metadata --no-deps --locked` accepts a stale lockfile, so it is \
         not a freshness check however much it reads like one"
    );
}

/// Run one subcommand against a fresh copy of the stale lockfile.
///
/// `Some(true)` means it acted on the disagreement — rewrote the file when free,
/// refused when pinned. `None` means this machine could not run it at all.
fn run_in_fixture(
    fixture: &Path,
    home: &Path,
    stale: &str,
    subcommand: &str,
    arguments: &[&str],
    locked: bool,
) -> Option<bool> {
    let lockfile = fixture.join("Cargo.lock");
    std::fs::write(&lockfile, stale).expect("plant the disagreement");
    let mut cargo = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string()));
    cargo
        .arg(subcommand)
        .args(arguments)
        .current_dir(fixture)
        // Its own build directory, so this never touches the tree under test,
        // and its own `CARGO_HOME`, so a subcommand that installs has somewhere
        // to put things that is not the developer's.
        .env("CARGO_TARGET_DIR", home.join("target"))
        .env("CARGO_HOME", home.join("cargo-home"))
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER");
    if locked {
        cargo.arg("--locked");
    }
    let out = cargo.output().expect("cargo runs");
    let said = String::from_utf8_lossy(&out.stderr);
    if said.contains("no such command") || said.contains("no such subcommand") {
        return None;
    }
    if locked {
        // The flag itself being rejected is not a refusal of the lockfile — it
        // is the subcommand saying the flag is not its. Both are non-zero exits
        // and reading them as one would call `cargo fmt` a lockfile check.
        if said.contains("unexpected argument '--locked'") {
            return Some(false);
        }
        return Some(said.contains("cannot update the lock file"));
    }
    Some(std::fs::read_to_string(&lockfile).expect("read back") != stale)
}

/// What each subcommand needs in order to do the least work that still resolves.
fn arguments_for(subcommand: &str) -> Vec<&'static str> {
    match subcommand {
        "metadata" => vec!["--format-version", "1"],
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

#[test]
fn a_workspace_this_repository_cannot_pin_does_not_track_a_lockfile() {
    let root = repository_root();
    let foreign = workspaces_this_repository_cannot_pin(&root);
    assert!(
        !foreign.is_empty(),
        "this repository has a workspace that path-depends on a sibling \
         checkout, and a run that found none is one where the lister did not \
         answer — the law below would then hold over nothing"
    );
    let tracked: BTreeSet<String> = tracked_manifests(&root).into_iter().collect();
    let tracked_locks = Command::new("git")
        .args(["ls-files", "*Cargo.lock"])
        .current_dir(&root)
        .output()
        .expect("git ls-files runs");
    let locks: BTreeSet<String> = String::from_utf8_lossy(&tracked_locks.stdout)
        .lines()
        .map(str::to_string)
        .collect();

    for directory in &foreign {
        assert!(
            !locks.contains(&format!("{directory}/Cargo.lock")),
            "`{directory}` resolves against a tree this repository does not own, \
             so every gate run rewrites its lockfile and a tracked copy is a \
             file that is stale between one commit and the next — the drift it \
             produces shows up as another repository's work in this one's \
             working tree"
        );
    }
    // THE OTHER DIRECTION, so this is not a law that a repository tracking no
    // lockfile at all would pass: a workspace this repository CAN pin keeps one,
    // because that is the file `--locked` reads.
    let mut pinned_workspaces = 0;
    for manifest in &tracked {
        let Some(directory) = manifest.strip_suffix("/Cargo.toml") else {
            continue;
        };
        if foreign.contains(directory) || !locks.contains(&format!("{directory}/Cargo.lock")) {
            continue;
        }
        pinned_workspaces += 1;
    }
    assert!(
        pinned_workspaces >= 10,
        "the separate workspaces this repository can pin all track a lockfile, \
         and {pinned_workspaces} of them is too few for that to have been read"
    );
}
