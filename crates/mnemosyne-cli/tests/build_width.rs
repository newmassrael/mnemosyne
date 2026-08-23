//! How wide a build runs is the runner's decision, not the command's.
//!
//! R1212. `.cargo/config.toml` says `jobs = 4`, and that number is a fact about
//! ONE machine — the 31 GiB workstation whose swap R1076 filled. It travels with
//! the tree to every machine that checks it out, and until this round nothing
//! had asked whether it GOVERNS there. Measured, with a fresh build directory
//! per arm so the build script actually ran:
//!
//! | what was set | `NUM_JOBS` a build script saw |
//! |---|---|
//! | this tree's config alone | 4 |
//! | `CARGO_BUILD_JOBS=8` against that config | 8 |
//! | `-j 2` against that variable | 2 |
//!
//! and on the build machine, where `bx` exports a per-host width: 31, on a host
//! this tree's config would have held to 4. So the config value is a DEFAULT for
//! a machine with no scheduler in front of it, the environment is how a
//! scheduler sizes a host, and a flag on the command line beats both.
//!
//! THAT LAST ONE IS THE LAW HERE. A `--jobs` or a `--test-threads` written into
//! a command this repository issues pins every machine that runs it to a number
//! the author's machine had — the build machine's 31 cores, a runner's 4, a
//! future host's whatever — and it does it silently, because a build that is
//! eight times narrower than it could be looks exactly like a slow build.
//!
//! The population is every cargo command this repository issues, from the same
//! five sources `locked_resolution_smoke` reads, assembled once in `ci-plan`.

use std::path::{Path, PathBuf};

use ci_plan::rust::MayHold;
use ci_plan::{commands_this_repository_issues, decides_its_own_width, CargoCommand, Spelled};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> sits two levels under the root")
        .to_path_buf()
}

/// A command as a line, for the cases that drive the arm directly.
fn issued(line: &str) -> CargoCommand {
    let mut words: Vec<String> = line.split_whitespace().map(str::to_string).collect();
    let harness_args = match words.iter().position(|word| word == "--") {
        Some(at) => words.split_off(at).split_off(1),
        None => Vec::new(),
    };
    CargoCommand {
        source: "a case".to_string(),
        owner: "a case".to_string(),
        carrier: Vec::new(),
        cargo_args: words,
        harness_args,
        env: Default::default(),
        // A line, so whose lockfile it resolves is the manifest path's to say.
        declared: None,
        site: None,
        uncounted: Vec::new(),
    }
}

#[test]
fn no_command_this_repository_issues_decides_how_wide_it_runs() {
    let root = repository_root();
    let issued = commands_this_repository_issues(&root);
    let commands = &issued.commands;
    // AND WHAT THIS MACHINE COULD NOT REACH, R1228. The population comes from
    // the lister, so it is `studio`'s eight commands larger on a workstation
    // holding the sibling checkout than it is on a hosted runner. The count
    // below therefore means something different on the two machines, and until
    // this line the printed number changed with nothing beside it saying why —
    // 231 here against 223 on a runner, measured.
    for skipped in &issued.skipped {
        println!("[build-width] {}", skipped.was_not("judged"));
    }
    // NON-VACUITY FIRST. A law over an empty population reports zero findings,
    // and zero findings is what a clean tree looks like.
    assert!(
        commands.len() > 20,
        "this repository issues {} cargo command(s), which is too few to be the \
         population this law is about — the assembly is what broke, not the tree",
        commands.len()
    );
    // WHAT IT JUDGED, by source, because a count with no shape behind it is an
    // alibi: the day one source stops being assembled, this number drops and
    // nothing else says so.
    let mut by_source: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for command in commands {
        *by_source.entry(command.source.as_str()).or_default() += 1;
    }
    println!(
        "[build-width] {} cargo command(s) this repository issues \
         ({} workspace(s) not reachable here and not in that number): {}",
        commands.len(),
        issued.skipped.len(),
        by_source
            .iter()
            .map(|(source, count)| format!("{source} {count}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let mut deciding = Vec::new();
    let mut unanswerable = Vec::new();
    for command in commands {
        match decides_its_own_width(command) {
            Spelled::No => {}
            Spelled::Yes(words) => deciding.push(format!(
                "{} — {} decides its width with {}",
                command.origin(),
                command.rendered(),
                words.join(" ")
            )),
            Spelled::Unreadable(why) => unanswerable.push(format!(
                "{} — {}: {why}",
                command.origin(),
                command.rendered()
            )),
        }
    }
    assert!(
        deciding.is_empty(),
        "a command that writes its own width beats the environment a scheduler \
         sets — measured: `-j 2` wins over `CARGO_BUILD_JOBS=8`, and the build \
         machine is sized entirely through that variable:\n  {}",
        deciding.join("\n  ")
    );
    // THE ANSWER THIS LAW USED TO GIVE SILENTLY, R1269. Two commands in this
    // population hand over a list of unknown length, and until that round they
    // came back "does not decide its own width" — a claim about words this law
    // never read. A hole cannot take a word away but it can hide one, so a
    // width flag could have been in there and nothing would have said so. The
    // site that knows says what its runtime words may hold
    // (`ci_plan::issue::runtime_words`), and a site that says nothing lands
    // here rather than in the clean pile.
    assert!(
        unanswerable.is_empty(),
        "{} command(s) hand over words this law cannot read, so whether they \
         decide their own width is a question and not an answer — the site says \
         what its runtime words may hold, or this law cannot judge it:\n  {}",
        unanswerable.len(),
        unanswerable.join("\n  ")
    );
}

#[test]
fn the_arm_fires_on_every_spelling_of_a_width() {
    // THE SHIM. The assertion above passes on a tree with no such command, which
    // is also what a reader that recognises nothing does. These drive the arm.
    for line in [
        "cargo build -j 4",
        "cargo build -j4",
        "cargo build --jobs 4",
        "cargo build --jobs=4",
        "cargo test --workspace -- --test-threads 1",
        "cargo test --workspace -- --test-threads=1",
    ] {
        assert!(
            matches!(decides_its_own_width(&issued(line)), Spelled::Yes(_)),
            "`{line}` decides its own width"
        );
    }
}

/// A HOLE MAKES THE ABSENCE A QUESTION AND A DECLARATION MAKES IT AN ANSWER.
///
/// R1269, all four arms. PINNED AGAINST FIXTURES BECAUSE THE TREE HOLDS ONLY
/// ONE OF THEM: the two commands in this repository that carry a hole both
/// declare, so an injection aimed at the refusal would come back green and say
/// nothing — the same reason `locked_resolution_smoke`'s own hole fixture is
/// written out rather than found.
#[test]
fn a_width_a_hole_may_hold_is_not_read_as_a_width_the_command_leaves_alone() {
    let with_a_hole = |line: &str, may_hold: MayHold| {
        let mut command = issued(line);
        command.uncounted = vec![may_hold];
        command
    };

    // THE DEFECT ITSELF. An undeclared list of unknown length may hold `--jobs`,
    // so "this command leaves the width alone" is a claim about words nobody
    // read. Before R1269 this answered with an empty list, which the law read as
    // clean.
    assert!(
        matches!(
            decides_its_own_width(&with_a_hole(
                "cargo test --workspace $words",
                MayHold::Anything
            )),
            Spelled::Unreadable(_)
        ),
        "an undeclared hole may hold a width flag, and a law that answered `no` \
         would be answering for words it never had"
    );

    // THE ANSWER. The site said which flags may be in there, none of them is a
    // width, and the absence is now a fact the words support.
    assert_eq!(
        decides_its_own_width(&with_a_hole(
            "cargo test --workspace $selectors",
            MayHold::OnlyThese(vec!["--lib".to_string(), "--test".to_string()])
        )),
        Spelled::No,
        "a declared hole that cannot hold a width leaves the width alone, and \
         saying so is the whole return on declaring"
    );

    // DECLARING MORE COSTS MORE, which is what makes the declaration honest: a
    // site cannot buy silence by widening what it admits to.
    assert!(
        matches!(
            decides_its_own_width(&with_a_hole(
                "cargo test --workspace $words",
                MayHold::OnlyThese(vec!["--jobs".to_string()])
            )),
            Spelled::Unreadable(_)
        ),
        "a site that declares `--jobs` may be among its runtime words has told \
         this law it cannot answer, and that is the declaration working"
    );

    // PRESENCE SURVIVES EVERYTHING. A hole cannot take a word away, declared or
    // not, so a width spelled beside one is spelled.
    assert_eq!(
        decides_its_own_width(&with_a_hole("cargo test -j 4 $words", MayHold::Anything)),
        Spelled::Yes(vec!["-j".to_string()]),
        "a flag spelled beside a hole is spelled"
    );
}

#[test]
fn a_command_that_leaves_the_width_alone_is_not_read_as_deciding_it() {
    // THE CONTROL, and the near misses that make it one: a flag whose name
    // merely starts the same way, and the two commands this repository actually
    // writes most often.
    for line in [
        "cargo test --workspace --locked --no-fail-fast",
        "cargo build --workspace --locked --no-run",
        "cargo clippy --workspace --all-targets -- -D warnings",
        "cargo run -q --locked --manifest-path tools/ci-plan/Cargo.toml",
        "cargo test --workspace -- --nocapture",
        "cargo build --job-server-is-not-a-flag",
    ] {
        assert_eq!(
            decides_its_own_width(&issued(line)),
            Spelled::No,
            "`{line}` leaves the width to whoever runs it"
        );
    }
}
