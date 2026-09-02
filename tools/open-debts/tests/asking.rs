//! Whether the census ASKS THE WORLD about the names its ledger's retirements
//! give — put to the binary, because that is where the asking lives.
//!
//! `census.rs` puts the library to a ledger and hands it the answers. That is
//! the right shape for the reading rules, and it is exactly the wrong shape for
//! this question: a set of unresolved names built by a test proves the plumbing
//! carries them, and proves NOTHING about whether anybody ever went and looked.
//! The round axis is the case in point — `Attribution.round` has been parsed
//! since the attribution rule was written, carried into every report, and asked
//! of no store at all, so a library test handed `{"R9999"}` would have passed
//! against a program that never resolved a round in its life.
//!
//! SO THE RESOLVER IS A FIXTURE AND NOT A MOCK OF ONE. `--repo` names the
//! repository whose commits and whose store the ledger cites, and this repo's
//! one resolver for "the CLI of this checkout" is `scripts/mn` inside it. A
//! fixture repository carrying its own `scripts/mn` is therefore not an escape
//! hatch bolted on for testing — it is the parameter used as documented, with a
//! resolver that records what it was asked. What it buys is that the argument
//! spelling, the working directory and the exit-code reading are the ones that
//! ship.
//!
//! AND THAT RESOLVER IS A SYMLINK TO A BINARY CARGO BUILT, never a script this
//! test writes. The first version wrote bash and chmod'd it, and the
//! `written-executable` gate refused it by name: `exec` on a file another
//! process holds open for writing fails with `ETXTBSY`, the holder is a
//! concurrent test's fork, and the failure then lands in a crate that did
//! nothing. What varies per case moved out of the program and into
//! `resolver.answers` beside it — see `src/bin/mn-stub.rs`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A directory of this test target's own, named for the case using it.
///
/// `CARGO_TARGET_TMPDIR` RATHER THAN A DEPENDENCY, because this crate has none
/// and a census that pulls in a tree of them to check a ledger is a census
/// nobody will run. It also names its owner, which is what the scratch-path
/// gate asks of every temporary directory in this repository.
///
/// THE PLACE `CARGO_TARGET_TMPDIR` NAMES, BUILT FROM A VARIABLE THAT EXISTS IN
/// BOTH BUILDS. This file is compiled twice by different commands: `cargo test`
/// builds it to run, and `cargo doc` builds it because the item-citation gate
/// documents every target a workspace has. `CARGO_TARGET_TMPDIR` is defined
/// only by the first, and each obvious way of reading it fails somewhere —
/// measured, not reasoned about:
///
///   - `env!` does not compile under the doc build. The target then cannot be
///     documented and that gate refuses to call it clean, because "an unreached
///     target is not a clean one" — it took the whole side-workspace run to 2.
///   - `std::env::var` is `NotPresent` at RUN time, all four cases at once. It
///     is what the compiler's own help suggested and the help is wrong for this
///     variable: cargo passes it to the compiler, not to the test process.
///   - `option_env!(…).expect(…)` compiles in both and clippy denies it
///     (`option_env_unwrap`), rightly: it turns something knowable at compile
///     time into a panic at run time.
///
/// `CARGO_MANIFEST_DIR` is defined by every cargo command, and `target/tmp` is
/// the very directory the missing variable names.
///
/// THE PROCESS IS IN THE PATH, AND THEN THE PATH IS COLLECTED BY IT. Naming the
/// owner is what this repository's scratch gate asks of a shared path, and the
/// reason it asks is not bookkeeping: an owner's name is what lets a later run
/// tell a directory somebody is USING from one whose process is gone. Without
/// the second half the first is a leak, and it was one — measured, two runs of
/// this suite left eight directories and nothing anywhere collects
/// `target/tmp`. So each case reaps its own abandoned predecessors and never a
/// live one: a sibling is removed only when no process holds its number.
fn fixture_root(case: &str) -> PathBuf {
    let scratch = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/tmp");
    reap_abandoned(&scratch, case);
    let root = scratch.join(format!("{case}-{}", std::process::id()));
    if root.exists() {
        fs::remove_dir_all(&root).expect("this run's own earlier fixture can be cleared");
    }
    fs::create_dir_all(&root).expect("the fixture directory can be made");
    root
}

/// Remove this case's fixtures whose owning process no longer exists.
///
/// `/proc/<pid>` IS THE QUESTION, which is the one this platform answers
/// directly — the same platform whose `symlink` this file already needs. A
/// directory whose number is still a process is left alone whatever its age,
/// because "old" is a guess about what somebody is doing and "gone" is a fact.
fn reap_abandoned(scratch: &Path, case: &str) {
    let Ok(entries) = fs::read_dir(scratch) else {
        return;
    };
    let mine = format!("{case}-");
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some(owner) = name.strip_prefix(&mine) else {
            continue;
        };
        if Path::new("/proc").join(owner).exists() {
            continue;
        }
        let _ = fs::remove_dir_all(entry.path());
    }
}

/// Put the resolver stub where `--repo` says the resolver is.
///
/// A SYMLINK, so nothing here creates a file it then runs.
fn link_resolver(root: &Path) {
    let scripts = root.join("scripts");
    fs::create_dir_all(&scripts).expect("the fixture's scripts directory can be made");
    std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_mn-stub"), scripts.join("mn"))
        .expect("the fixture resolver can be linked");
}

/// A repository whose resolver answers as `rounds` says and writes down every
/// question it was asked.
///
/// THE LOG IS THE POINT. "The gate refused the row" and "the gate asked the
/// store" are different claims, and only the second one distinguishes a census
/// that resolves rounds from one that guessed and happened to be right.
///
/// THE ANSWERS ARE DATA AND THE PROBE IS ONE OF THEM. Writing the readiness
/// answer beside the round answers is what lets one fixture shape serve both the
/// case where the store lacks a round and the case where the resolver cannot
/// speak at all — the two the census must never confuse.
fn repository_answering(root: &Path, rounds: &[(&str, bool)]) -> PathBuf {
    link_resolver(root);
    let mut answers = String::from("probe\t0\n");
    for (round, exists) in rounds {
        answers.push_str(&format!("Round {round}\t{}\n", u8::from(!*exists)));
    }
    fs::write(root.join("resolver.answers"), answers)
        .expect("the fixture's answers can be written");
    root.join("asked.log")
}

/// The census run over `ledger`, against `repo`.
fn census(ledger: &Path, repo: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_open-debts"))
        .arg("--ledger")
        .arg(ledger)
        .arg("--repo")
        .arg(repo)
        .output()
        .expect("the census binary can be run")
}

/// What the fixture resolver was asked, one question per line.
fn asked(log: &Path) -> Vec<String> {
    fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

/// A retirement naming a round the store does not have retires nothing.
///
/// THE DEFECT AS IT STOOD. `--repo` was made required so that a row could not be
/// closed against a commit nobody made, on the argument that the arc's
/// termination is reachable by writing a sentence — and the round beside that
/// commit, which is the name this ledger writes most often, went on being
/// believed on sight. `R9999 CLOSED` passed.
#[test]
fn a_retirement_naming_a_round_the_store_does_not_have_retires_nothing() {
    let root = fixture_root("missing_round");
    let log = repository_answering(&root, &[("1300", true), ("9999", false)]);
    let ledger = root.join("ledger.md");
    fs::write(
        &ledger,
        "- **N901**(①) — retired by a round the store has. CLOSED (R1300)\n\
         - **N902**(①) — retired by a round it does not. CLOSED (R9999)\n\
         - **N903**(①) — still open, so the walk has something to print.\n",
    )
    .expect("the fixture ledger can be written");

    let answer = census(&ledger, &root);
    let said = String::from_utf8_lossy(&answer.stdout).to_string();

    let questions = asked(&log);
    assert!(
        questions.iter().any(|q| q.contains("Round 9999")),
        "the store is ASKED about the round a retirement named: {questions:?}"
    );
    assert!(
        questions.iter().any(|q| q.contains("Round 1300")),
        "and about every other one, not just the first: {questions:?}"
    );
    assert!(
        said.contains("N902"),
        "the row whose closure names a round nobody has is NOT retired: {said}"
    );
    assert!(
        !said.contains("N901"),
        "and the one whose round exists stays retired: {said}"
    );
    assert_eq!(
        answer.status.code(),
        Some(1),
        "judged, and rows are open: {said}"
    );
}

/// A resolver that cannot answer at all is `2`, not a ledger full of findings.
///
/// THE ONE RESOLVER RETURNS `1` FOR BOTH "the store does not have it" and "there
/// is no store here", which is a distinction the commit axis gets from git for
/// free and this axis has to buy. Without it, a checkout where the CLI will not
/// build reports every round the ledger ever named as a hallucinated citation —
/// as many findings as the ledger has named rounds, none of them true, printed
/// with the same confidence as a real one. So the resolver is asked a question
/// it must be able to answer BEFORE any round is judged on its silence.
#[test]
fn a_resolver_that_cannot_answer_is_no_verdict_rather_than_a_hundred_findings() {
    let root = fixture_root("mute_resolver");
    link_resolver(&root);
    // NO ANSWERS FILE AT ALL, which is the resolver's own default: everything
    // gets 1, the probe included. That is the shape a checkout where the CLI
    // will not build presents to this census.
    let ledger = root.join("ledger.md");
    fs::write(
        &ledger,
        "- **N904**(①) — retired by a round. CLOSED (R1300)\n\
         - **N905**(①) — still open.\n",
    )
    .expect("the fixture ledger can be written");

    let answer = census(&ledger, &root);
    let said = String::from_utf8_lossy(&answer.stderr).to_string();
    assert_eq!(
        answer.status.code(),
        Some(2),
        "not judged: {said}{}",
        String::from_utf8_lossy(&answer.stdout)
    );
    assert!(
        said.contains("NO VERDICT"),
        "and it says so in the words the rest of this census uses: {said}"
    );
}

/// No resolver where `--repo` says one is, and the census refuses to judge.
///
/// A MISSING TOOL IS NOT AN ABSENT ROUND. The failure this is written against is
/// the ordinary one — a path typed wrong, a checkout without the script — where
/// every `Command` fails to spawn and a reader that treats a failed spawn as
/// "no" turns a typo into a ledger of false accusations.
#[test]
fn a_repository_with_no_resolver_cannot_judge_the_rounds_it_was_given() {
    let root = fixture_root("no_resolver");
    let ledger = root.join("ledger.md");
    fs::write(
        &ledger,
        "- **N906**(①) — retired by a round. CLOSED (R1300)\n\
         - **N907**(①) — still open.\n",
    )
    .expect("the fixture ledger can be written");

    let answer = census(&ledger, &root);
    let said = String::from_utf8_lossy(&answer.stderr).to_string();
    assert_eq!(
        answer.status.code(),
        Some(2),
        "not judged: {said}{}",
        String::from_utf8_lossy(&answer.stdout)
    );
    assert!(
        said.contains("NO VERDICT"),
        "and it says why rather than answering about rounds it never asked: {said}"
    );
}

/// A ledger naming no round at all needs no resolver.
///
/// THE CONTROL FOR THE TWO REFUSALS ABOVE, and the thing that keeps them from
/// being a census that demands a repository to read a text file. It is the same
/// line the commit axis draws: `--repo` is required WHEN the ledger names
/// something to check, and a ledger that names nothing is judged on its own.
#[test]
fn a_ledger_whose_retirements_name_no_round_is_judged_without_asking_anything() {
    let root = fixture_root("nothing_named");
    let ledger = root.join("ledger.md");
    fs::write(
        &ledger,
        "- **N908**(①) — done. CLOSED 2026-08-14\n\
         - **N909**(①) — still open.\n",
    )
    .expect("the fixture ledger can be written");

    let answer = Command::new(env!("CARGO_BIN_EXE_open-debts"))
        .arg("--ledger")
        .arg(&ledger)
        .output()
        .expect("the census binary can be run");
    let said = String::from_utf8_lossy(&answer.stdout).to_string();
    assert_eq!(
        answer.status.code(),
        Some(1),
        "judged, with N909 open: {said}{}",
        String::from_utf8_lossy(&answer.stderr)
    );
    assert!(
        said.contains("N909") && !said.contains("N908"),
        "the day-attributed retirement still retires: {said}"
    );
}

/// Every program this crate has is declared, and one of them is the default.
///
/// THE LAW IS HERE BECAUSE THE DEFECT CAME FROM HERE (Round 1314). The fixture
/// above needed a program cargo builds, so R1313 put one in `src/bin/` — and
/// that alone made `cargo run --manifest-path tools/open-debts/Cargo.toml --
/// --ledger …` stop working, because a crate with two binaries and no
/// `default-run` gives cargo nothing to choose. It is the way the census is run
/// by hand, no script runs it that way, and every suite names its target
/// explicitly, so the whole root run and every separate workspace stayed green
/// over a program that could no longer be started. Measured, not reasoned about:
/// exit 101, `could not determine which binary to run`.
///
/// TWO CLAUSES, AND THE SECOND IS THE ONE THAT CATCHES THE NEXT ONE. Naming a
/// default fixes today; requiring that every file in `src/bin/` be written down
/// in the manifest is what makes the next fixture binary announce itself instead
/// of arriving as a target nobody declared. This crate's whole subject is that a
/// thing nobody wrote down is a thing nobody checked.
#[test]
fn every_program_this_crate_builds_is_declared_and_one_is_the_default() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest =
        fs::read_to_string(root.join("Cargo.toml")).expect("this crate's own manifest can be read");
    let declared: Vec<String> = manifest
        .lines()
        .filter_map(|line| line.trim().strip_prefix("name = "))
        .map(|name| name.trim().trim_matches('"').to_string())
        .collect();
    let default = manifest
        .lines()
        .find_map(|line| line.trim().strip_prefix("default-run = "))
        .map(|name| name.trim().trim_matches('"').to_string());

    // The census is the program, whatever else this crate builds for its tests.
    assert_eq!(
        default.as_deref(),
        Some("open-debts"),
        "a crate with a second binary and no default-run cannot be `cargo run` \
         at all — the manifest read: {manifest}"
    );

    // AND NOTHING IN `src/bin/` ARRIVES UNDECLARED, which is how the second one
    // did. Auto-discovery is what turned a test fixture into a target of this
    // crate without a line anywhere saying so.
    for entry in fs::read_dir(root.join("src/bin")).expect("src/bin can be read") {
        let path = entry.expect("its entries can be read").path();
        if path.extension().and_then(|it| it.to_str()) != Some("rs") {
            continue;
        }
        let program = path
            .file_stem()
            .and_then(|it| it.to_str())
            .expect("a rust file has a stem")
            .to_string();
        assert!(
            declared.contains(&program),
            "`src/bin/{program}.rs` is a program this crate builds and the \
             manifest does not name it: {declared:?}"
        );
    }
}
