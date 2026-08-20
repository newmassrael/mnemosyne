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

use ci_plan::{commands_this_repository_issues, decides_its_own_width, CargoCommand};

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
        uncounted: 0,
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
    let deciding: Vec<String> = commands
        .iter()
        .filter_map(|command| {
            let words = decides_its_own_width(command);
            (!words.is_empty()).then(|| {
                format!(
                    "{} — {} decides its width with {}",
                    command.origin(),
                    command.rendered(),
                    words.join(" ")
                )
            })
        })
        .collect();
    assert!(
        deciding.is_empty(),
        "a command that writes its own width beats the environment a scheduler \
         sets — measured: `-j 2` wins over `CARGO_BUILD_JOBS=8`, and the build \
         machine is sized entirely through that variable:\n  {}",
        deciding.join("\n  ")
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
        let found = decides_its_own_width(&issued(line));
        assert!(!found.is_empty(), "`{line}` decides its own width");
    }
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
        assert!(
            decides_its_own_width(&issued(line)).is_empty(),
            "`{line}` leaves the width to whoever runs it"
        );
    }
}
