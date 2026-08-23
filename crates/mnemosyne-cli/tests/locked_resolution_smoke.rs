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
    // A SITE WITH A CONDITIONAL WORD IS EVERY COMMAND IT CAN ISSUE (R1265),
    // AND THE POPULATION ARRIVES HOLDING THEM (R1268). This function used to
    // expand the sites itself and append the result, which made it the only law
    // of the three reading a whole population — `build_width` and
    // `judged_test_runs` received the same struct twelve commands short and had
    // no way to know. The expansion moved into `commands_this_repository_issues`
    // where the other five sources are assembled; what stays here is the REPORT,
    // and the assertion that a site filed as enumerable is one.
    let mut ways = 0;
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
    }
    println!(
        "[locked-resolution] {} site(s) written with a conditional word issue \
         {ways} command(s) between them, and every one of them is judged below; \
         across every site, {} way(s) are dropped because no table can be keyed \
         on a subcommand behind a hole or decided while the program runs, and {} \
         site(s) have more ways than a report can hold",
        issued.rust.conditional.len(),
        issued.rust.ways_no_table_can_key_on,
        issued.rust.ways_beyond_a_report
    );
    issued.commands
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
/// WHICH SITES THIS LAW REACHES IS NOT ASKED HERE ANY MORE (R1277). R1271 put
/// that half in this test because this arm was the one it had a defect in, and
/// the question turned out to belong to all five —
/// `every_site_that_declares_a_tree_issues_a_command_or_is_named` asks it of
/// every arm at once, against a list of the sites nobody can read. Asking it
/// twice would be two answers to one question.
#[test]
fn a_site_that_pins_when_the_tree_is_ours_says_so_with_a_conditional_flag() {
    let root = repository_root();
    let commands = everything_this_repository_issues(&root);
    // THE SITE IS ASKED THROUGH ITS COMMANDS, because a site whose flag is
    // unconditional issues exactly ONE and that is the whole defect. The key is
    // the SITE the command came from and not its `origin` (R1277): a command
    // read at a call site is attributed to that caller, so one site reached
    // through five callers reads as five, and two spawns in one function read as
    // one. Neither is the number this law is about.
    let mut by_site: BTreeMap<String, (bool, bool)> = BTreeMap::new();
    for command in &commands {
        if matches!(
            command.declared,
            Some(ci_plan::rust::Declared::PinnedWhenItIsOurs(_))
        ) {
            let Some(site) = &command.site else {
                panic!(
                    "{} carries a declaration and names no site: {}",
                    command.origin(),
                    command.rendered()
                );
            };
            let seen = by_site.entry(site.clone()).or_insert((false, false));
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
        // THE FOUR THAT PASS ARE SPELLED, AND THE WILDCARD BELOW IS THE ONLY
        // `match` OVER `LockVerdict` IN THIS REPOSITORY (R1279, measured by
        // adding a variant and finding that nothing failed to build). It refuses
        // what it does not know, which is the direction a wildcard has to point
        // when it is the only one: a new verdict arrives red here and is read
        // rather than accepted.
        match verdict {
            LockVerdict::Pinned
            | LockVerdict::ResolvesNothing
            | LockVerdict::NotOursToPin
            | LockVerdict::PlantedForTheMeasurement => {}
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
        let free = run_in_fixture(
            &fixture,
            home.path(),
            stale,
            subcommand,
            &arguments,
            Pin::Free,
        );
        let (Some(rewrote), Some(refused)) = (
            free,
            run_in_fixture(
                &fixture,
                home.path(),
                stale,
                subcommand,
                &arguments,
                Pin::Locked,
            ),
        ) else {
            unmeasurable.push(subcommand.clone());
            continue;
        };
        measured += 1;
        // THE THIRD ARM (R1280). `--frozen` is documented as `--locked
        // --offline`, and this asks the cargo on this machine whether it acts on
        // a disagreeing lockfile the same way — which is what licenses
        // `ci_plan`'s `is_the_pin` reading it as the pin. A documented
        // equivalence is a claim, and this file does not take claims about cargo
        // on trust.
        let frozen = run_in_fixture(
            &fixture,
            home.path(),
            stale,
            subcommand,
            &arguments,
            Pin::Frozen,
        );
        println!(
            "cargo {subcommand}: free resolve {} the lockfile, `--locked` {} it, \
             `--frozen` {}",
            if rewrote { "REWROTE" } else { "left" },
            if refused { "REFUSED" } else { "accepted" },
            match frozen {
                Some(true) => "REFUSED it",
                Some(false) => "accepted it",
                None => "could not be run",
            }
        );
        assert_eq!(
            frozen,
            Some(refused),
            "`cargo {subcommand} --frozen` and `cargo {subcommand} --locked` \
             have to act the same on a lockfile that disagrees with its \
             manifests, because `--frozen` IS `--locked --offline` — and \
             `ci_plan::is_the_pin` reads both as the pin on the strength of this \
             measurement. If they differ, that reader is wrong for one of them"
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
            Pin::Locked,
        ),
        Some(false),
        "`cargo metadata --no-deps --locked` accepts a stale lockfile, so it is \
         not a freshness check however much it reads like one"
    );
}

/// Which of cargo's positions on re-resolving one arm of this measurement takes.
///
/// R1280, AND THE THIRD ARM IS WHY `is_the_pin` COULD BE FIXED. `--frozen` is
/// documented as `--locked --offline`, and a documented equivalence is a claim —
/// this file's whole discipline is that a claim about what cargo does to a
/// lockfile is MEASURED against the cargo on this machine. So the fixture runs
/// it, and what `lock_verdict` reads as the pin follows the measurement rather
/// than the sentence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pin {
    /// No flag: the arm that shows whether a subcommand REWRITES a lockfile it
    /// disagrees with.
    Free,
    /// `--locked`.
    Locked,
    /// `--frozen`, which cargo documents as `--locked --offline`.
    Frozen,
}

impl Pin {
    /// The word this arm adds, or `None` for the free one.
    fn flag(self) -> Option<&'static str> {
        match self {
            Self::Free => None,
            Self::Locked => Some("--locked"),
            Self::Frozen => Some("--frozen"),
        }
    }
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
    pin: Pin,
) -> Option<bool> {
    let lockfile = fixture.join("Cargo.lock");
    std::fs::write(&lockfile, stale).expect("plant the disagreement");
    // THE ONE PLACE A DISAGREEING LOCKFILE IS DELIBERATE. The fixture is built
    // three functions up, its lockfile is planted on the line above, and all
    // three arms — free, `--locked` and `--frozen` — are run on purpose, which
    // is what makes this a measurement rather than a check.
    let mut cargo = issue::cargo(Tree::PlantedByThisMeasurement(
        "the one-package fixture this measurement builds, whose lockfile is \
         planted disagreeing on purpose, and which this function runs both \
         pinned and free because the answer is the difference",
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
    // SPELLED OUT RATHER THAN HANDED OVER AS A VALUE, and that is what the arm
    // this site declares asks of it: the pin has to be a word READABLE at the
    // spawn and present on some paths only, so `a_site_that_plants_a_lockfile_
    // runs_both_ways` can see the variable vary. A `cargo.arg(flag)` would put
    // the pin behind a runtime word and take that check away.
    if pin == Pin::Locked {
        cargo.arg("--locked");
    }
    if pin == Pin::Frozen {
        cargo.arg("--frozen");
    }
    let out = cargo.output().expect("cargo runs");
    let said = String::from_utf8_lossy(&out.stderr);
    if said.contains("no such command") || said.contains("no such subcommand") {
        return None;
    }
    if let Some(flag) = pin.flag() {
        // The flag itself being rejected is not a refusal of the lockfile — it
        // is the subcommand saying the flag is not its. Both are non-zero exits
        // and reading them as one would call `cargo fmt` a lockfile check.
        if said.contains(&format!("unexpected argument '{flag}'")) {
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

/// Every site that declares a tree and issues no command these laws can judge,
/// with the reason its words cannot be read.
///
/// R1277, AND IT IS THE HALF A GREEN RUN ABOVE DOES NOT COVER. R1271 found that
/// a site whose words stop being readable issues no command, so a law about its
/// arm goes on passing over the sites that remain — and it fixed that for ONE
/// arm by demanding every declaring site be reached. Measured over all five, the
/// demand is false of two of them: some sites hand their subcommand over as a
/// caller's word, or build their word list from a value, and no reading of the
/// syntax finishes them.
///
/// A NUMBER WOULD NOT HAVE DONE. `RustSpawns` already counts what it could not
/// read — `ways_no_table_can_key_on`, `carried`, `ways_beyond_a_report` — and
/// every one of those is a total, so a site leaving reach while another arrives
/// is a total that did not move. This is the names, and a name is what makes the
/// difference between a run that reports and a run that refuses: adding a site
/// here is an edit somebody makes on purpose, with a reason, in a review.
///
/// THE LINE IS DELIBERATELY NOT PART OF THE KEY. A site is `path` + the function
/// holding it, so an edit above it does not redden this list — a list that goes
/// red for an unrelated change is a list people learn to renumber, which is the
/// opposite of what it is for.
///
/// R1278 TOOK FOUR OF THE SEVEN OFF IT, AND THAT IS WHAT THIS LIST IS FOR. Four
/// entries said nearly the same sentence four times — the words are another
/// command's, judged where it is written — and a shape four sites share is a
/// shape that can carry a DECLARATION instead of a listing
/// (`issue::Tree::AlreadyJudgedWhereItIsWritten`), which owes a falsifiable
/// obligation where an entry here owes only a sentence.
///
/// R1279 TOOK A FIFTH OFF IT, AND CORRECTED THE SENTENCE R1278 LEFT HERE. That
/// sentence said the three remaining were one shape — each a measurement fixture
/// whose subcommand is a test's loop variable — and the tree says otherwise: of
/// the three, exactly ONE spells the pin (`run_in_fixture`, on some paths), and
/// the other two spell no pin at all while handing over an `.args(..)` that MAY
/// hold one, which a hole makes unclaimable either way (R1266). So the arm
/// R1279 built for the one — its obligation is that the pin be CONDITIONAL —
/// could not have been given to the other two: nothing here could hold them to
/// it. The observation was written down without being run, which is the habit
/// R1278's own entry names.
///
/// A list nobody is trying to shorten is a list that grows; a list shortened by
/// a sentence nobody checked is worse than one that stayed long.
const DECLARED_AND_UNREADABLE: [(&str, &str, &str); 2] = [
    (
        "crates/mnemosyne-cli/tests/compiling_subcommands.rs",
        "compiles_in_fixture",
        "the subcommand is one of the words its caller hands over, and that \
         caller builds the list while it runs — so no way through this site is \
         one a table of subcommands can be keyed on. It spells no pin, and \
         whether the list it is handed holds one is not a question a hole can \
         answer",
    ),
    (
        "tools/stale-artifacts/tests/pass.rs",
        "in_fixture",
        "both call sites hand over a list they assembled, so the words at the \
         spawn are a hole this reader cannot count — and the same follows: no \
         pin is spelled here and no claim about one is available",
    ),
];

/// The words a relay may not add, because they are the ones a verdict is read
/// from.
///
/// R1278. `lock_verdict` reads a command's subcommand, its `--manifest-path` and
/// its pin, and nothing else — so a relay that adds one of those has stopped
/// relaying and started deciding.
///
/// `--frozen` WAS HERE BEFORE THE VERDICT COULD SEE IT, which is worth recording
/// because it is the order this repository prefers. R1278 forbade it on the
/// reasoning that `--frozen` IS `--locked --offline` while `is_the_pin` matched
/// the literal `--locked` only — so a relay adding it would pin the other site's
/// command in a word the verdict was blind to. R1280 measured the equivalence
/// against cargo and taught `is_the_pin` the second word, so the blindness is
/// gone and this entry now names a word the verdict really does read. `--offline`
/// stays for the other reason: it is NOT the pin, and a relay that adds it is
/// still changing how the other site's command resolves.
const WORDS_A_RELAY_MAY_NOT_ADD: [&str; 4] =
    ["--locked", "--frozen", "--offline", "--manifest-path"];

/// EVERY ARM IS ASKED THE QUESTION R1271 ASKED OF ONE.
///
/// A law about an arm asks its COMMANDS, so a site whose words stop being
/// readable issues none and leaves in silence. R1271 measured that for
/// `PinnedWhenItIsOurs` — `stale-artifacts::apply` had never issued a judged
/// command, and the law over that arm was passing on two of the three sites that
/// declare it. It fixed that arm and left the other four unasked, which is N196:
/// whether they are even the sort of thing to ask the same number of was itself
/// unmeasured.
///
/// Measured, and the answer is NO FOR TWO OF THEM — so the shape of the law is
/// not the equality R1271 used but a PARTITION, in three cells since R1278:
/// every site that declares an arm either issues a command these laws judge, or
/// declares `Tree::AlreadyJudgedWhereItIsWritten` and is judged by
/// `a_site_that_relays_a_judged_command_adds_nothing_that_changes_the_answer`
/// where it stands, or is one of [`DECLARED_AND_UNREADABLE`] with its reason
/// written down. Nothing may be in none of the three, and nothing in two.
///
/// THE THIRD CELL IS NOT A WAY OUT OF THE FIRST, which is the question a new
/// cell has to answer. A relay's subcommand is its caller's word, so it can
/// never be in the first cell; what it CAN do is owe something a program checks,
/// and it owes three things (see that law). The cell that has no obligation is
/// the list, which is why the list is the one this repository is trying to
/// shorten.
///
/// THE ARMS COME FROM THE ENUM (`Declared::arms`), so a seventh one arrives here
/// without anybody deciding to add it — and arrives failing, because an arm no
/// site declares is a law holding over nothing.
#[test]
fn every_site_that_declares_a_tree_issues_a_command_or_is_named() {
    let root = repository_root();
    let issued = ci_plan::commands_this_repository_issues(&root);

    // WHICH SITE, NOT WHICH ORIGIN. `origin` is where a person goes to change
    // the WORDS, which for a command read at a call site is that caller — so
    // `item-citations::cargo`, one site, has five of them. Counting reach in
    // origins would read that as five sites answering for an arm one site
    // declares, and as one site for a function holding two spawns.
    let mut reached: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for command in &issued.commands {
        match (&command.declared, &command.site) {
            (Some(declared), Some(site)) => {
                reached
                    .entry(declared.arm().to_string())
                    .or_default()
                    .insert(site.clone());
            }
            (None, None) => {}
            // THE PAIRING IS A LAW AND NOT A REMARK. Exactly the commands a Rust
            // source's own words produced carry both; a declaration whose site
            // went missing is an arm this law can no longer ask about, and it
            // would go missing as a green.
            (declared, site) => panic!(
                "{} carries declared={declared:?} and site={site:?}, which is \
                 half of a pair: {}",
                command.origin(),
                command.rendered()
            ),
        }
    }

    // The sites this walk could not finish, so an unreached one can be printed
    // with the reason the walk itself gives rather than a reason this test
    // invents.
    let mut unfinished: BTreeMap<String, &ci_plan::rust::RustSpawn> = BTreeMap::new();
    for site in issued.rust.conditional.iter().chain(&issued.rust.carried) {
        unfinished.insert(site.origin(), site);
    }

    let mut found: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
    let mut missing_from_the_walk = Vec::new();
    let mut asked: BTreeSet<&str> = BTreeSet::new();
    for arm in ci_plan::rust::Declared::arms() {
        let name = arm.arm();
        asked.insert(name);
        let declared = issued.rust.declaring.get(name).cloned().unwrap_or_default();
        let empty = BTreeSet::new();
        let hit = reached.get(name).unwrap_or(&empty);
        println!(
            "[locked-resolution] arm {name}: {} site(s) declare it, {} issue a \
             command this population judges",
            declared.len(),
            hit.len()
        );
        if matches!(arm, ci_plan::rust::Declared::Unreadable(_)) {
            // NOT AN ARM A SITE CAN DECLARE. It is this reader failing to read a
            // `Tree` expression, and such a spawn is set aside as
            // `beside_the_door` for `one_door_for_cargo` to refuse — so a
            // declaration counted under it here would mean an unreadable
            // expression had been let through as a declaration.
            assert!(
                declared.is_empty() && hit.is_empty(),
                "`Unreadable` is this reader failing to read a `Tree`, not \
                 something a site declares — {declared:?} / {hit:?}"
            );
            continue;
        }
        assert!(
            !declared.is_empty(),
            "no site in this repository declares `Tree::{name}`, so every law \
             about that arm holds over nothing — the arm is either unnecessary \
             or unreachable, and both are things to know"
        );
        if matches!(
            arm,
            ci_plan::rust::Declared::AlreadyJudgedWhereItIsWritten(_)
        ) {
            // THE SECOND CELL, AND IT IS AN ARM RATHER THAN A LIST (R1278). A
            // relay hands over another command's words, so its subcommand is
            // its caller's and it can never issue a command this population
            // judges. What it can do is owe something falsifiable, which the
            // relay law holds it to over the SITE — including that every site
            // declaring the arm is one that law reached, so this cell cannot
            // quietly grow past it.
            assert!(
                hit.is_empty(),
                "a relay's subcommand is the word its caller hands over, so a \
                 relay site that issued a judged command has words this reader \
                 finished — which means it added a subcommand of its own: {hit:?}"
            );
            continue;
        }
        if matches!(arm, ci_plan::rust::Declared::PlantedByThisMeasurement(_)) {
            // THE THIRD CELL (R1279), AND UNLIKE THE RELAY IT MAY ISSUE
            // COMMANDS. A measurement whose subcommand is a literal enumerates
            // its two paths and both are judged `PlantedForTheMeasurement`; one
            // whose subcommand is a loop variable issues none. Either way what
            // holds the site is `a_site_that_plants_a_lockfile_runs_both_ways`,
            // which asserts every site declaring the arm is one it reached — so
            // this cell cannot grow past that law either.
            continue;
        }
        let invented: Vec<&String> = hit.difference(&declared).collect();
        assert!(
            invented.is_empty(),
            "a command declares `Tree::{name}` and names a site nothing \
             declared it at, so the two halves of the pair were read from \
             different places: {invented:?}"
        );
        for origin in declared.difference(hit) {
            match unfinished.get(origin) {
                Some(site) => found
                    .entry((site.source.clone(), site.owner.clone()))
                    .or_default()
                    .push(format!("{name} — {origin} — {}", site.reach())),
                // A site that issues nothing and is in no bucket is the
                // conservation failure `read_but_unanswered` exists for, one
                // level up: it left every count at once.
                None => missing_from_the_walk.push(format!("{name} — {origin}")),
            }
        }
    }
    assert!(
        missing_from_the_walk.is_empty(),
        "these sites declare a tree, issue no command, and are in neither the \
         conditional nor the carried bucket — they left every number this walk \
         keeps at the same time:\n  {}",
        missing_from_the_walk.join("\n  ")
    );

    // AND THE LIST OF ARMS COVERED WHAT THE WALK ACTUALLY READ. `Declared::arms`
    // is a chain the compiler keeps exhaustive over the ENUM, which is not the
    // same as exhaustive over the tree: a chain that skipped a link would build,
    // and the loop above would simply never ask about that arm — a hole of
    // exactly the shape this whole law exists to close, one level up. So the
    // arms this run asked about are held against the arms sites were found
    // declaring.
    let unasked: Vec<&String> = issued
        .rust
        .declaring
        .keys()
        .chain(reached.keys())
        .filter(|name| !asked.contains(name.as_str()))
        .collect();
    assert!(
        unasked.is_empty(),
        "sites in this repository declare {unasked:?} and `Declared::arms` did \
         not hand that arm over, so nothing above asked anything about it — the \
         chain has a link that skips"
    );

    let doubled: Vec<String> = found
        .iter()
        .filter(|(_, why)| why.len() > 1)
        .map(|((source, owner), why)| format!("{source} `{owner}`: {why:?}"))
        .collect();
    assert!(
        doubled.is_empty(),
        "two spawns in one function declare a tree and neither is readable, so \
         the file-and-function key below no longer names one of them — the list \
         needs the line back, or the function needs splitting:\n  {}",
        doubled.join("\n  ")
    );

    let named: BTreeSet<(String, String)> = DECLARED_AND_UNREADABLE
        .iter()
        .map(|(source, owner, _)| ((*source).to_string(), (*owner).to_string()))
        .collect();
    let unnamed: Vec<String> = found
        .iter()
        .filter(|(key, _)| !named.contains(key))
        .map(|((source, owner), why)| format!("{source} `{owner}` — {}", why.join("; ")))
        .collect();
    assert!(
        unnamed.is_empty(),
        "these sites declare a tree and issue no command any law over this \
         population judges, and nothing says so — which is the silence R1271 \
         found for one arm and this law asks of all of them. Add each to \
         `DECLARED_AND_UNREADABLE` with the reason its words cannot be read, or \
         make them readable:\n  {}",
        unnamed.join("\n  ")
    );
    let stale: Vec<String> = named
        .iter()
        .filter(|key| !found.contains_key(key))
        .map(|(source, owner)| format!("{source} `{owner}`"))
        .collect();
    assert!(
        stale.is_empty(),
        "these are listed as sites nobody can read and they are not — either \
         the site is gone, or its words became readable and the list is now \
         claiming a limit this repository does not have:\n  {}",
        stale.join("\n  ")
    );
}

/// A SITE THAT RELAYS A JUDGED COMMAND OWES THREE THINGS AND ALL OF THEM ARE
/// FALSIFIABLE.
///
/// R1278, and it is what took four entries off [`DECLARED_AND_UNREADABLE`]. Four
/// gates re-issue the command they were asked about — with `--no-run` to build
/// without running, with `--message-format=json` to read cargo's own artifact
/// record, with `--doc -- --list` to ask which doc-tests there are — and until
/// this round every one of them declared `Tree::WhereverTheCallerPoints`, whose
/// obligation is that the command resolve NOTHING. They re-issue `cargo test`
/// and `cargo check`, which resolve. The declaration was FALSE of all four, and
/// what hid it is the very thing R1277's law is about: a relay's subcommand sits
/// behind a hole, so the site issued no command for `lock_verdict` to contradict.
///
/// So the arm they declare now says what they do, and the obligation is about
/// the words the site ADDS rather than the words it passes on:
///
/// 1. THE FIRST WORD IT HANDS OVER IS THE RELAYED LIST. That is what makes the
///    subcommand the other site's rather than this one's, and a relay with no
///    hole at all relays nothing — which this law sees as a site the walk could
///    finish, and refuses in the equality below.
/// 2. NONE OF ITS OWN WORDS IS ONE A VERDICT IS READ FROM
///    ([`WORDS_A_RELAY_MAY_NOT_ADD`]). `--locked` pins what the other site chose
///    not to pin, `--manifest-path` points the command at a different tree, and
///    `--frozen` does the first in a word `is_the_pin` cannot see.
/// 3. AND THE HARNESS SIDE IS NOT THE CARGO SIDE. Words after the first bare
///    `--` go to the test binary and change no resolution, so this stops there
///    rather than refusing `--list`.
///
/// WHAT IT CANNOT CHECK is that the relayed list really is a command this
/// population holds — the semantic ceiling every arm shares.
#[test]
fn a_site_that_relays_a_judged_command_adds_nothing_that_changes_the_answer() {
    let root = repository_root();
    let issued = ci_plan::commands_this_repository_issues(&root);
    let arm = ci_plan::rust::Declared::AlreadyJudgedWhereItIsWritten(String::new());
    let name = arm.arm();
    let declared = issued.rust.declaring.get(name).cloned().unwrap_or_default();
    assert!(
        !declared.is_empty(),
        "no site declares `Tree::{name}`, so this law holds over nothing — the \
         arm is either unnecessary or unreachable, and both are things to know"
    );

    let mut reached = BTreeSet::new();
    let mut broken = Vec::new();
    for site in issued.rust.conditional.iter().chain(&issued.rust.carried) {
        let ci_plan::rust::Program::Cargo(spelled) = &site.program else {
            continue;
        };
        if spelled.arm() != name {
            continue;
        }
        reached.insert(site.origin());
        match site.words.first() {
            Some(ci_plan::rust::Word::Unknown(..)) => {}
            other => broken.push(format!(
                "{} — the first word it hands over is {} and not the relayed \
                 list, so the subcommand is this site's own choice",
                site.origin(),
                other.map_or_else(
                    || "nothing at all".to_string(),
                    |word| format!("`{}`", word.rendered())
                )
            )),
        }
        for word in &site.words {
            let spelled_here = match word {
                ci_plan::rust::Word::Spelled(text) | ci_plan::rust::Word::Sometimes(text, _) => {
                    text.as_str()
                }
                // ONE runtime word is not a flag this site spells, which is the
                // reading `Word::Runtime` carries everywhere else; a hole is the
                // relayed list itself.
                ci_plan::rust::Word::Runtime(_) | ci_plan::rust::Word::Unknown(..) => continue,
            };
            if spelled_here == "--" {
                break;
            }
            if let Some(forbidden) = WORDS_A_RELAY_MAY_NOT_ADD.iter().find(|word| {
                spelled_here == **word || spelled_here.starts_with(&format!("{word}="))
            }) {
                broken.push(format!(
                    "{} — it adds `{spelled_here}`, and `{forbidden}` is a word \
                     the verdict is read from, so this site is deciding rather \
                     than relaying",
                    site.origin()
                ));
            }
        }
    }
    println!(
        "[locked-resolution] {} site(s) relay a command judged where it is \
         written, each handing over a list this reader cannot count and adding \
         only words no verdict is read from",
        reached.len()
    );
    // EVERY SITE THAT DECLARES THE ARM IS ONE THIS LAW REACHED, which is R1271's
    // assertion in the place it belongs. A relay whose words this walk could
    // FINISH is not in either bucket, and it is not in either bucket precisely
    // because it has no hole — obligation 1 broken in the one way that would
    // otherwise leave this law holding over fewer sites in silence.
    assert_eq!(
        reached,
        declared,
        "{} site(s) declare `Tree::{name}` and this law reached {} of them — a \
         site missing here is one whose words this reader could finish, which \
         for a relay means it wrote a word list of its own: {:?}",
        declared.len(),
        reached.len(),
        declared.difference(&reached).collect::<Vec<_>>()
    );
    assert!(
        broken.is_empty(),
        "a site declaring that the words it hands over were judged where they \
         are written may add nothing a verdict is read from, or the verdict at \
         that other site is no longer the verdict here:\n  {}",
        broken.join("\n  ")
    );
}

/// A SITE THAT PLANTS A LOCKFILE OWES BOTH ARMS OF ITS OWN MEASUREMENT.
///
/// R1279. `Tree::PlantedByThisMeasurement` is the one arm where the pin is not
/// forbidden but a VARIABLE, so the thing that keeps it from being an exemption
/// is that the variable has to vary: the site must spell `--locked` on some paths
/// through the function and not others. A measurement that always pins, or never
/// does, is asserting a difference rather than measuring one — R1259's question,
/// asked of the arm that most needs it.
///
/// TWO HALVES, AND THE SECOND IS THE ONE THAT COULD OTHERWISE BE SILENT. A site
/// whose pin is UNCONDITIONAL has words this reader can finish, so it leaves the
/// conditional and carried buckets and a law that only walked them would hold
/// over fewer sites and say nothing — R1271's shape, and the equality against
/// `declaring` is what catches it. The first half is then the words themselves:
/// the pin must be a `Word::Sometimes` rather than a `Word::Spelled`.
///
/// WHY THIS ARM COULD NOT BE HANDED TO THE TWO SIBLING FIXTURES, which is the
/// correction R1279 makes to a sentence R1278 wrote without running it: they
/// spell no pin at all and hand over an `.args(..)` that may hold one, so a hole
/// makes the question unclaimable in both directions (R1266) and nothing here
/// could hold them to anything.
#[test]
fn a_site_that_plants_a_lockfile_runs_both_ways() {
    let root = repository_root();
    let issued = ci_plan::commands_this_repository_issues(&root);
    let arm = ci_plan::rust::Declared::PlantedByThisMeasurement(String::new());
    let name = arm.arm();
    let declared = issued.rust.declaring.get(name).cloned().unwrap_or_default();
    assert!(
        !declared.is_empty(),
        "no site declares `Tree::{name}`, so this law holds over nothing — the \
         arm is either unnecessary or unreachable, and both are things to know"
    );

    let mut reached = BTreeSet::new();
    let mut broken = Vec::new();
    for site in issued.rust.conditional.iter().chain(&issued.rust.carried) {
        let ci_plan::rust::Program::Cargo(spelled) = &site.program else {
            continue;
        };
        if spelled.arm() != name {
            continue;
        }
        reached.insert(site.origin());
        let mut conditional_pin = 0;
        let mut unconditional_pin = 0;
        for word in &site.words {
            match word {
                ci_plan::rust::Word::Sometimes(text, _) if text == "--locked" => {
                    conditional_pin += 1;
                }
                ci_plan::rust::Word::Spelled(text) if text == "--locked" => {
                    unconditional_pin += 1;
                }
                _ => {}
            }
        }
        if unconditional_pin > 0 {
            broken.push(format!(
                "{} — it spells `--locked` on every path, so the pin is not the \
                 variable this arm says it is",
                site.origin()
            ));
        }
        if conditional_pin == 0 {
            broken.push(format!(
                "{} — it spells `--locked` on no path at all, so there is no \
                 free-versus-pinned difference here to measure: {}",
                site.origin(),
                site.rendered()
            ));
        }
    }
    println!(
        "[locked-resolution] {} site(s) plant a lockfile for a measurement, each \
         spelling the pin on some paths through the function and not others",
        reached.len()
    );
    // EVERY SITE THAT DECLARES THE ARM IS ONE THIS LAW REACHED (R1271's
    // assertion, in the place it belongs). A site whose pin is unconditional has
    // words this reader can finish, which takes it out of both buckets — so the
    // half of the obligation that would otherwise be invisible is caught here
    // rather than in the loop above.
    assert_eq!(
        reached,
        declared,
        "{} site(s) declare `Tree::{name}` and this law reached {} of them — a \
         site missing here is one whose words this reader could finish, and for \
         this arm that means no word in them depends on the path taken: {:?}",
        declared.len(),
        reached.len(),
        declared.difference(&reached).collect::<Vec<_>>()
    );
    assert!(
        broken.is_empty(),
        "a site declaring that it planted the lockfile it resolves is claiming \
         the pin is its measurement's variable, and a variable that does not \
         vary is an assertion wearing a measurement's declaration:\n  {}",
        broken.join("\n  ")
    );
}

/// THE ARM THE MEASUREMENT FIXTURE LEFT WOULD HAVE REFUSED ITS PINNED PATH.
///
/// R1279, and it is the same discipline R1278 used one arm over: "the old
/// declaration was false" is a claim, and a claim about a mechanism in this tree
/// can be RUN. `run_in_fixture` declared `Tree::MadeByThisRun`, whose obligation
/// `lock_verdict` enforces as `ours = false` — so a command under it that spells
/// the pin is `PinsWhatItDoesNotOwn`, a refusal. The fixture spells the pin on
/// half its paths on purpose, because that IS the measurement.
///
/// WHY NOTHING EVER REFUSED IT: its subcommand is the loop variable of the test
/// above, so no way through the site is one a table can be keyed on, so no
/// command of its own was ever in the population. False and unreachable at the
/// same time, exactly as R1278 found for the four relays.
#[test]
fn the_arm_the_measurement_left_refuses_the_pinned_half_of_a_measurement() {
    let root = repository_root();
    let tracked = tracked_manifests(&root);
    let foreign = workspaces_this_repository_cannot_pin(&root);
    let commands = everything_this_repository_issues(&root);

    let mut checked = 0;
    for pinned in &commands {
        if pinned.declared.is_some() || !pinned.has("--locked") {
            continue;
        }
        let Some(subcommand) = pinned.subcommand() else {
            continue;
        };
        if resolves_the_lockfile(subcommand) != Some(true) {
            continue;
        }
        let under = |declared: ci_plan::rust::Declared| {
            let mut command = pinned.clone();
            command.declared = Some(declared);
            command.site = Some("a measurement that planted this lockfile".to_string());
            lock_verdict(&command, &tracked, &foreign)
        };
        assert_eq!(
            under(ci_plan::rust::Declared::MadeByThisRun(
                "the obligation this arm carries".to_string()
            )),
            LockVerdict::PinsWhatItDoesNotOwn,
            "`{}` spells the pin, so under `MadeByThisRun` it pins a tree that \
             arm says is not ours — and a measurement's pinned path is exactly \
             that command",
            pinned.rendered()
        );
        assert_eq!(
            under(ci_plan::rust::Declared::PlantedByThisMeasurement(
                "the lockfile is the subject".to_string()
            )),
            LockVerdict::PlantedForTheMeasurement,
            "the arm that says the lockfile is the measurement's own has to \
             accept the pinned path as readily as the free one, and for `{}` it \
             did not",
            pinned.rendered()
        );
        checked += 1;
    }
    println!(
        "[locked-resolution] {checked} pinned command(s) were put under both \
         arms: `MadeByThisRun` refuses every one of them, and the arm the \
         fixture declares now accepts every one"
    );
    assert!(
        checked >= 20,
        "this repository issues far more than twenty pinned resolving cargo \
         commands written as data, and {checked} of them is too few for the \
         comparison above to be about anything"
    );
}

/// THE ARM THOSE FOUR SITES USED TO DECLARE REFUSES THE COMMAND THEY ISSUE, AND
/// THE ONE THEY DECLARE NOW REPRODUCES THE VERDICT ALREADY RECORDED.
///
/// R1278, and it is here because "the old declaration was false" is a claim, and
/// a claim about a mechanism in this tree can be RUN. The relays hand over a
/// command out of this very population — so this takes each command the
/// population holds that RESOLVES, puts it under each of the two declarations in
/// turn, and asks `lock_verdict`.
///
/// - under `WhereverTheCallerPoints`, whose obligation is that the command
///   resolve NOTHING, the verdict is `Unreadable` — "a declaration the words
///   contradict", in that function's own words;
/// - under `AlreadyJudgedWhereItIsWritten` the verdict is EQUAL to the relayed
///   command's own, which is the whole content of the arm.
///
/// WHY THIS COULD NOT FAIL BEFORE, which is the point R1277 was making one level
/// up: a relay's subcommand is its caller's word, so no relay command was ever
/// in the population, so `lock_verdict` was never asked. The declaration was
/// false and unreachable at the same time, and the second is what made the first
/// survive.
#[test]
fn the_arm_a_relay_used_to_declare_contradicts_the_command_a_relay_issues() {
    let root = repository_root();
    let tracked = tracked_manifests(&root);
    let foreign = workspaces_this_repository_cannot_pin(&root);
    let commands = everything_this_repository_issues(&root);

    let mut checked = 0;
    for relayed in &commands {
        // WRITTEN AS DATA, so its own verdict is read off its words — which is
        // exactly the position a relay puts them in.
        if relayed.declared.is_some() {
            continue;
        }
        let Some(subcommand) = relayed.subcommand() else {
            continue;
        };
        if resolves_the_lockfile(subcommand) != Some(true) {
            continue;
        }
        let recorded = lock_verdict(relayed, &tracked, &foreign);
        if matches!(recorded, LockVerdict::Unreadable(_)) {
            // Nothing to compare against: this command has no verdict of its
            // own for a relay to inherit.
            continue;
        }
        let under = |declared: ci_plan::rust::Declared| {
            let mut command = relayed.clone();
            command.declared = Some(declared);
            command.site = Some("a relay of this command".to_string());
            lock_verdict(&command, &tracked, &foreign)
        };
        let as_the_old_arm = under(ci_plan::rust::Declared::WhereverTheCallerPoints(
            "the obligation this arm carries".to_string(),
        ));
        assert!(
            matches!(as_the_old_arm, LockVerdict::Unreadable(_)),
            "`{}` resolves, so a site declaring `WhereverTheCallerPoints` over \
             it is claiming the command resolves nothing — and `lock_verdict` \
             answered {as_the_old_arm:?} rather than refusing it, which would \
             mean the obligation that arm documents is not the one it enforces",
            relayed.rendered()
        );
        let as_the_relay = under(ci_plan::rust::Declared::AlreadyJudgedWhereItIsWritten(
            "the words are another command's".to_string(),
        ));
        assert_eq!(
            as_the_relay,
            recorded,
            "the whole content of the relay arm is that the verdict is the one \
             already recorded where the words are written, and for `{}` the two \
             disagree",
            relayed.rendered()
        );
        checked += 1;
    }
    println!(
        "[locked-resolution] {checked} command(s) this population holds were put \
         under both declarations: the old one refuses every single one of them, \
         the new one reproduces the verdict already recorded"
    );
    // NON-VACUITY, AND IT IS A FLOOR ON THE RESOLVING HALF OF THE POPULATION.
    // Every relay in this repository is handed one of these, so a run that found
    // a handful of them is a run whose answer is about something else.
    assert!(
        checked >= 20,
        "this repository issues far more than twenty resolving cargo commands \
         written as data, and {checked} of them is too few for the comparison \
         above to be about the population a relay is actually handed"
    );
}
