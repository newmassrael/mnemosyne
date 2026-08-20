//! Every cargo command a Rust program in this repository issues goes through
//! one door.
//!
//! # What the door is for
//!
//! `locked_resolution_smoke` asks of every cargo command whether it pins the
//! lockfile it resolves, and the answer turns on WHOSE lockfile it is. For the
//! five sources written as data — workflows, scripts, the build declaration,
//! sweep manifests, the workspace lister — the target is written down and
//! `ci_plan::CargoCommand::manifest` reads it. For a Rust program it is a value:
//!
//! ```text
//! .args(["metadata", "--no-deps"]).arg("--manifest-path").arg(manifest)
//! ```
//!
//! and `manifest` is this repository's own workspace in the gate that walks them
//! and a directory the test made two lines earlier in the fixture next door. The
//! words are identical. No reading of the syntax separates them, and both
//! readings are wrong for the other site.
//!
//! So the site says which, through `ci_plan::issue::cargo` — and this law is
//! what makes that answerable rather than optional: a `Command::new` naming
//! cargo is a site with no declaration for any law to read, and the population
//! of cargo commands would quietly be the ones that happened to come through.
//!
//! # Why the population is complete even though the words are not
//!
//! Round 1257 wrote down that a Rust program's cargo commands cannot be read
//! statically, and half of that is true: the WORDS are computed. The POPULATION
//! is not — spawning is a syntactic act, written in a tracked file, and
//! `git ls-files` with `syn` enumerates every one exactly. That is the whole
//! reason this is a law and not a record of what somebody happened to run: a
//! program nobody ran writes an empty record, and an empty record is what a
//! clean program and an unexamined one look like alike.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ci_plan::rust::{cargo_commands, Declared};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> sits two levels under the root")
        .to_path_buf()
}

#[test]
fn every_cargo_command_a_rust_program_issues_comes_through_the_one_door() {
    let found = cargo_commands(&repository_root());

    // THE REACH FIRST. An empty finding list is what a clean tree looks like AND
    // what a walk that read nothing looks like, and this walk's population is
    // every tracked Rust file in the repository.
    assert!(
        found.files > 300,
        "this repository tracks more than three hundred Rust files and this walk \
         parsed {}, which is a walk that stopped rather than a repository that \
         emptied",
        found.files
    );
    assert!(
        found.spawns > 100,
        "this repository spawns programs from Rust in more than a hundred places \
         and this walk saw {}, so the finding list below is over the wrong \
         population",
        found.spawns
    );

    let beside: Vec<String> = found
        .beside_the_door
        .iter()
        .map(|site| format!("{} — {}", site.origin(), site.rendered()))
        .collect();
    assert!(
        beside.is_empty(),
        "{} cargo spawn(s) do not come through `ci_plan::issue::cargo`, so \
         nothing says whose lockfile they resolve and `locked_resolution_smoke` \
         cannot judge them:\n  {}",
        beside.len(),
        beside.join("\n  ")
    );

    // NON-VACUITY OF THE OTHER HALF: the door has to be in use. A law that only
    // says "no second door" passes on the day the first one is deleted too.
    let through = found.commands.len() + found.conditional.len() + found.carried.len();
    assert!(
        through >= 15,
        "this repository issues cargo from Rust in at least fifteen places and \
         this walk found {through} through the door, which is the shape of a \
         reader that stopped recognising it"
    );

    // EVERY DECLARATION IS ONE THIS READER UNDERSTOOD. A `Tree` arm nobody
    // taught it becomes `Unreadable`, which lands beside the door above — this
    // asserts the other direction, that what did come through was read.
    //
    // OVER EVERY SITE, not only the ones read as ONE command. A site whose word
    // list depends on the path taken declares just as much as any other, and
    // tallying the arms over the readable half alone is how an arm in daily use
    // reads as unreachable: the fifth one is used at exactly one site, and that
    // site is conditional by construction, since a flag decided while it runs is
    // the whole of what the arm says.
    let mut arms: BTreeMap<&str, usize> = BTreeMap::new();
    let declared_at = found
        .commands
        .iter()
        .map(|command| (command.origin(), command.declared.clone()))
        .chain(found.conditional.iter().map(|site| {
            let declared = match &site.program {
                ci_plan::rust::Program::Cargo(declared) => Some(declared.clone()),
                _ => None,
            };
            (site.origin(), declared)
        }));
    for (origin, declared) in declared_at {
        let arm = match &declared {
            Some(Declared::ThisRepository) => "this repository",
            Some(Declared::MadeByThisRun(_)) => "a tree the run made",
            Some(Declared::WhereverTheCallerPoints(_)) => "wherever the caller points",
            Some(Declared::PinnedWhereverItPoints(_)) => "pinned wherever it points",
            Some(Declared::PinnedWhenItIsOurs(_)) => "pinned when it is ours",
            Some(Declared::Unreadable(_)) | None => {
                panic!("{origin} came through the door undeclared")
            }
        };
        *arms.entry(arm).or_default() += 1;
    }
    println!(
        "[one-door] {} command(s) read and {} site(s) whose words depend on the \
         path taken: {arms:?}",
        found.commands.len(),
        found.conditional.len()
    );
    println!(
        "[one-door] {} carried, {} of this workspace's own binaries, {} other \
         program(s) named, {} spawn(s) whose program this reader cannot name",
        found.carried.len(),
        found.our_binaries,
        found.other_programs,
        found.unplaceable.len()
    );
    for site in &found.carried {
        println!("[one-door]   carried: {} — {}", site.origin(), site.reach());
    }

    // NOTHING THAT CAME THROUGH THE DOOR FALLS OUT OF EVERY ANSWER, and this
    // law exists because R1266 broke it while building. When the hop backwards
    // learned to read a caller's local binding, two sites whose callers all
    // handed over lists WITH HOLES stopped being carried — every call site was
    // read — while producing no command either, because the merged words still
    // held a hole. They left every bucket and every number in the same moment,
    // which is what a silent drop looks like from the outside: nothing.
    assert_eq!(
        found.read_but_unanswered, 0,
        "{} call site(s) had their words READ and produced no answer at all — no \
         command in the population and no count naming what stopped them, which \
         is the one state this reader must never reach",
        found.read_but_unanswered
    );
    println!(
        "[one-door] {} way(s) no table can be keyed on, {} site(s) with more \
         ways than a report can hold, 0 call site(s) read and unanswered",
        found.ways_no_table_can_key_on, found.ways_beyond_a_report
    );

    // THE HOP BACKWARDS IS IN USE, and this is what keeps it from going quiet.
    // A wrapper's words are at its call sites, and a reader that stopped
    // following them there would report every one of those commands as carried —
    // which is exactly what this repository looked like before R1263, and
    // nothing said so. The number is small on purpose: it is the count of
    // commands whose words no law could read until the reader went one hop in
    // the other direction.
    assert!(
        found.through_a_wrapper > 0,
        "no cargo command in this repository has its words read at a call site \
         one hop back, so either the wrappers are gone or the reader stopped \
         following them — and a command it cannot finish reading is one no law \
         asks anything of"
    );

    // AND THE PILE OF THINGS NOBODY IS HOLDING TO ANYTHING HAS A DENOMINATOR.
    // Most spawns here run a binary this workspace builds, named through
    // `env!("CARGO_BIN_EXE_…")` — the one spelling cargo checks at compile time.
    // If the reading of that spelling broke, every one of them would land in
    // `unplaceable` instead, and nothing above would notice: the law would still
    // find no second door, because it could no longer place a program at all.
    assert!(
        found.our_binaries > 50,
        "this repository spawns its own binaries from more than fifty places and \
         this walk placed {}, so the spelling cargo checks is no longer being \
         read and the spawns it named are now in the pile this law says nothing \
         about",
        found.our_binaries
    );
    for site in &found.unplaceable {
        println!("[one-door]   unplaceable: {}", site.origin());
    }

    // ALL FIVE ARMS ARE IN USE, and this is the assertion that keeps the three
    // that decline to name a tree outright honest. One owes `lock_verdict` a
    // command that resolves nothing, one owes it `--locked` on every path, and
    // one owes it a `--locked` its own code decides; if any of them ever became
    // the only arm anybody reached for, the rest would be dead letters and
    // nothing here would say so.
    for arm in [
        "this repository",
        "a tree the run made",
        "wherever the caller points",
        "pinned wherever it points",
        "pinned when it is ours",
    ] {
        assert!(
            arms.get(arm).copied().unwrap_or(0) > 0,
            "no cargo command in this repository declares `{arm}`, so that arm is \
             either unnecessary or unreachable and both are things to know: {arms:?}"
        );
    }
}

/// THE DOOR'S OWN SPAWN NEEDS NO EXCEPTION, and finding that out is worth the
/// sentence.
///
/// The door ends in a `Command::new`, and the first draft of this law excused it
/// by name. The excuse turned out to be unreachable: the program it spawns is
/// that function's own PARAMETER, and since a bare name is no longer resolved
/// through a function of the same name, the reader answers `Unplaceable` there
/// rather than "cargo beside the door". The injection written to prove the
/// excuse mattered came back with nothing red, which is how a dead exception in
/// a gate announces itself — and the excuse was deleted rather than kept as a
/// clause nothing exercises.
#[test]
fn the_door_is_not_a_site_this_law_has_to_excuse() {
    let door: Vec<_> = cargo_commands(&repository_root())
        .unplaceable
        .into_iter()
        .filter(|site| site.source.ends_with("ci-plan/src/issue.rs"))
        .collect();
    assert_eq!(
        door.len(),
        1,
        "the door spawns exactly once, and the reader places that spawn by the \
         PARAMETER it is handed — a second one there, or none, means the door has \
         been rewritten and what this law excuses has changed with it: {door:#?}"
    );
}
