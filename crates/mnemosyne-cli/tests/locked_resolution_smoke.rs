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

use ci_plan::issue::{self, Tree};
use ci_plan::{
    commands_this_repository_issues, lock_verdict, resolves_the_lockfile, tracked_manifests,
    workspaces_this_repository_cannot_pin, CargoCommand, LockVerdict, BUILD_DECLARATION,
};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> sits two levels under the root")
        .to_path_buf()
}

/// Everything this repository issues: the workflows GitHub runs, the shell the
/// hooks and scripts run, the commands the workspace lister declares, the build
/// machine's, and — since Round 1256 — every injection sweep's.
///
/// THE FIFTH SOURCE WAS THE WHOLE POPULATION MISSING. Twenty-six of the
/// thirty-three tracked sweep manifests issued a suite without `--locked`, and
/// nothing had ever asked: a sweep restores exactly the files it edited, so a
/// lockfile its suite rewrote is one it leaves behind, in the same run that
/// reports the tree returned to what it was.
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
/// AND THE SIXTH SOURCE SAYS WHAT IT COULD NOT FINISH READING (R1262). A Rust
/// program's words are computed, so some of them are a hole this walk cannot
/// close — and the hole's SIZE is printed here for the reason R1190 gave one
/// directory over: a limit stated without its size is a limit nobody can weigh.
fn everything_this_repository_issues(root: &Path) -> Vec<CargoCommand> {
    let issued = commands_this_repository_issues(root);
    for skipped in &issued.skipped {
        println!("[locked-resolution] {}", skipped.was_not("judged"));
    }
    println!(
        "[locked-resolution] the sixth source: {} spawn(s) in {} tracked Rust \
         file(s) — {} cargo command(s) read, {} site(s) whose word list depends \
         on the path taken, {} carried (an `.args(..)` this reader cannot \
         count), {} spawn(s) whose program it cannot name",
        issued.rust.spawns,
        issued.rust.files,
        issued.rust.commands.len(),
        issued.rust.conditional.len(),
        issued.rust.carried.len(),
        issued.rust.unplaceable.len()
    );
    for site in &issued.rust.carried {
        println!(
            "[locked-resolution]   carried: {} — {}",
            site.origin(),
            site.reach()
        );
    }
    // A SITE WITH A CONDITIONAL WORD IS EVERY COMMAND IT CAN ISSUE (R1265).
    // Before that it was one command nobody could read, and the law below said
    // nothing about it — which is the same silence as a clean answer. Each way
    // the choices go is a command this repository issues, and each is judged
    // beside the ones written whole.
    let mut ways = 0;
    let mut every_path = Vec::new();
    for site in &issued.rust.conditional {
        let Some(commands) = site.commands() else {
            panic!(
                "{} is filed as a site whose paths can be enumerated and it \
                 could not be: {site:#?}",
                site.origin()
            );
        };
        println!(
            "[locked-resolution]   on {} path(s): {} — {}",
            commands.len(),
            site.origin(),
            site.rendered()
        );
        ways += commands.len();
        every_path.extend(commands);
    }
    println!(
        "[locked-resolution] {} site(s) written with a conditional word issue \
         {ways} command(s) between them, and every one of them is judged below",
        issued.rust.conditional.len()
    );
    let mut all = issued.commands;
    all.extend(every_path);
    all
}

/// A site that pins when the tree is ours OWES A CONDITIONAL FLAG.
///
/// The verdict half of `Tree::PinnedWhenItIsOurs` cannot fail: the arm says the
/// flag is present exactly where the lockfile is this repository's, so
/// `lock_verdict` reads ownership off the flag and both paths pass. R1259 spent
/// a round on what an expectation like that is worth, and the answer is nothing
/// — so the teeth are here, over the SITE rather than over one of its commands.
///
/// A site declaring this arm and spelling `--locked` unconditionally is claiming
/// a condition its code does not have; one spelling it nowhere is claiming the
/// opposite. Both are a declaration the file contradicts, and both are things a
/// program can see. What no program here can see is whether the condition is
/// OWNERSHIP rather than something else — the semantic ceiling every arm shares
/// and the reason this one is not a way around the other four.
#[test]
fn a_site_that_pins_when_the_tree_is_ours_says_so_with_a_conditional_flag() {
    let root = repository_root();
    let commands = everything_this_repository_issues(&root);
    // THE SITE IS ASKED THROUGH ITS COMMANDS, because a site whose flag is
    // unconditional issues exactly ONE and that is the whole defect. Commands
    // from one site share the file and the function that wrote them, which is
    // what `origin` is; a function holding two cargo spawns would be read as one
    // site here, and the claim — that this function's `--locked` is decided
    // while it runs — is still the right one to make about it.
    let mut by_site: BTreeMap<String, (bool, bool)> = BTreeMap::new();
    for command in &commands {
        if matches!(
            command.declared,
            Some(ci_plan::rust::Declared::PinnedWhenItIsOurs(_))
        ) {
            let seen = by_site.entry(command.origin()).or_insert((false, false));
            if command.has("--locked") {
                seen.0 = true;
            } else {
                seen.1 = true;
            }
        }
    }
    assert!(
        !by_site.is_empty(),
        "no command in this repository declares `Tree::PinnedWhenItIsOurs`, so \
         this law holds over nothing — the arm is either unnecessary or \
         unreachable, and both are things to know"
    );
    let broken: Vec<String> = by_site
        .iter()
        .filter(|(_, (pinned, free))| !(*pinned && *free))
        .map(|(origin, (pinned, free))| {
            format!("{origin} — a path that pins: {pinned}, a path that does not: {free}")
        })
        .collect();
    println!(
        "[locked-resolution] {} site(s) pin when the tree is ours, each issuing \
         one command that says `--locked` and one that does not",
        by_site.len()
    );
    assert!(
        broken.is_empty(),
        "a site declaring that it pins WHEN the tree is ours has to leave that \
         decision in its code: a `--locked` spelled on every path claims a \
         condition the file does not have, and one spelled on none claims the \
         reverse — either way the arm would be a way past the other four rather \
         than a fifth answer:\n  {}",
        broken.join("\n  ")
    );
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
        } else if command.source.ends_with("sweep.json") {
            "an injection sweep"
        } else if command.source.ends_with(".rs") {
            "a Rust program's own words"
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
    // ROUND 1256 — AND THE FIFTH SOURCE HAS A FLOOR OF ITS OWN. Every tracked
    // sweep manifest carries exactly one, so this floor is a count of
    // manifests: a walk that stopped reading them would take it toward zero,
    // and every one it stopped reading is a suite that could rewrite a lockfile
    // the sweep will not restore.
    // ROUND 1262 — AND THE SIXTH SOURCE HAS ITS OWN FLOOR. Its population is a
    // syntax walk over every tracked `.rs`, so a walk that stopped reading takes
    // this toward zero while the other five stay exactly as they were.
    for kind in [
        "a workflow",
        "the workspace lister",
        "a tracked script",
        "an injection sweep",
        "a Rust program's own words",
    ] {
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
    // THE ONE PLACE A DISAGREEING LOCKFILE IS DELIBERATE. The fixture is built
    // three functions up, its lockfile is planted on the line above, and both
    // arms — free and `--locked` — are run on purpose, which is what makes this
    // a measurement rather than a check.
    let mut cargo = issue::cargo(Tree::MadeByThisRun(
        "the one-package fixture this measurement builds, whose lockfile is \
         planted disagreeing on purpose",
    ));
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
    // A COMMAND THAT DID NOT RUN LEFT THE LOCKFILE ALONE, and reading that as
    // "this subcommand does not resolve" is how this measurement produced a
    // WRONG answer rather than no answer (R1262). `cargo rustdoc` with no target
    // named refuses to pick between the fixture's library and its binary, exits
    // non-zero, touches nothing — and was recorded as a subcommand `--locked`
    // has nothing to do for. The repair is loud: what a subcommand needs in
    // order to run belongs in `arguments_for`, and until it is there this cannot
    // answer.
    assert!(
        out.status.success(),
        "`cargo {subcommand} {}` did not run in the fixture ({}), so what it \
         does to a disagreeing lockfile was not measured — an unchanged file \
         here says nothing at all. Give it what it needs in `arguments_for`:\n{}",
        arguments.join(" "),
        out.status,
        said.trim()
    );
    Some(std::fs::read_to_string(&lockfile).expect("read back") != stale)
}

/// What each subcommand needs in order to do the least work that still resolves.
fn arguments_for(subcommand: &str) -> Vec<&'static str> {
    match subcommand {
        "metadata" => vec!["--format-version", "1"],
        // R1262 — the sixth source brought these two in. `generate-lockfile`
        // comes from the fixtures that CREATE a lockfile, with `--offline` for
        // the reason those fixtures pass it: a resolve that reached for a
        // registry would be measuring the network.
        "generate-lockfile" => vec!["--offline"],
        // AND `rustdoc` NEEDS A PACKAGE AND A TARGET NAMED. The fixture root is a
        // virtual manifest and its one package has both a library and a binary,
        // so `cargo rustdoc` alone refuses twice over — and each refusal left the
        // lockfile untouched, which this test read as "the subcommand does not
        // resolve". Measured directly against cargo (R1262): it does. Both words
        // came from the assertion below naming the exact refusal.
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
