//! The reading rules, pinned against strings rather than against the tree.
//!
//! Both sides of every gate built on this crate come from these two parses, and
//! the branches that matter most are the ones THIS MACHINE NEVER TAKES: a `SKIP`
//! line only appears where a sibling checkout is absent, which is a CI runner
//! and not the machine this is written on. R1082's gate had no such branch and
//! turned main red on its first push. Pinned text is how a branch nobody here
//! can execute still has a control.

use ci_plan::{lister_cargo_commands, parse_lister, parse_script, CargoCommand};

/// The lister's output as `scripts/check-side-workspaces.sh --list` prints it,
/// including the skip this machine cannot produce.
const LISTED: &str = "[side-workspaces] CHECKABLE bench\n\
     [side-workspaces] SUITE bench cargo test --manifest-path bench/Cargo.toml --locked --no-fail-fast\n\
     [side-workspaces] SKIP studio — its path dependencies leave this \
     repository and are not on this machine: ../pinion/crates/pinion-a11y\n\
     [side-workspaces] CHECKABLE tools/item-citations\n\
     [side-workspaces] SUITE tools/item-citations cargo test --manifest-path tools/item-citations/Cargo.toml --locked --no-fail-fast\n\
     [side-workspaces] checked 2 (bench tools/item-citations), skipped 1 (studio)\n";

#[test]
fn the_lister_is_read_for_what_it_can_and_cannot_be_asked() {
    let listed = parse_lister(LISTED);
    assert_eq!(
        listed.askable,
        vec![
            "Cargo.toml".to_string(),
            "bench/Cargo.toml".to_string(),
            "tools/item-citations/Cargo.toml".to_string(),
        ],
        "the root is always asked; a skipped workspace is not"
    );
    assert_eq!(listed.skipped.len(), 1, "{:?}", listed.skipped);
    assert!(
        listed.skipped[0].starts_with("studio "),
        "the skip carries the workspace AND the reason, so the print says why: \
         {:?}",
        listed.skipped
    );
    assert!(
        !listed.askable.iter().any(|m| m.starts_with("studio")),
        "a workspace the lister could not check must not be asked anyway: {:?}",
        listed.askable
    );
}

#[test]
fn a_skipped_workspace_contributes_no_suite_to_run() {
    let listed = parse_lister(LISTED);
    assert_eq!(
        listed.suites.keys().collect::<Vec<_>>(),
        vec!["bench", "tools/item-citations"],
        "only the checkable ones have a suite: {:?}",
        listed.suites
    );
    let commands = lister_cargo_commands(&listed);
    assert_eq!(commands.len(), 2, "{commands:?}");
    assert!(
        commands
            .iter()
            .all(|command| command.subcommand() == Some("test")),
        "the lister's suite is a cargo test: {commands:?}"
    );
    assert_eq!(
        commands[0].value(&["--manifest-path"]),
        Some("bench/Cargo.toml"),
        "the manifest the suite is pointed at is readable off the command"
    );
    assert!(
        commands
            .iter()
            .all(|command| command.harness_args.is_empty()),
        "the lister passes the harness nothing, so it runs every test the \
         command's targets hold: {commands:?}"
    );
}

#[test]
fn a_shell_line_is_split_where_cargo_stops_and_the_harness_starts() {
    let read = |script: &str| {
        parse_script(script)
            .into_iter()
            .map(|(cargo_args, harness_args)| CargoCommand {
                source: "pinned".to_string(),
                owner: "pinned".to_string(),
                cargo_args,
                harness_args,
            })
            .collect::<Vec<_>>()
    };

    let plain = read("cargo test --workspace --locked");
    assert_eq!(plain.len(), 1);
    assert!(
        plain[0].harness_args.is_empty(),
        "no bare `--` means the harness was passed nothing"
    );

    let split = read("cargo test -p mnemosyne-cli --test evidence_replay_smoke --locked -- --ignored --nocapture");
    assert_eq!(split.len(), 1);
    assert_eq!(split[0].value(&["--test"]), Some("evidence_replay_smoke"));
    assert_eq!(
        split[0].harness_args,
        vec!["--ignored".to_string(), "--nocapture".to_string()],
        "everything past the bare `--` is the harness's, and a gate appending \
         `--list` has to append THERE"
    );
    assert!(
        split[0].harness_has("--ignored"),
        "which tests such a command runs is a question about its harness side"
    );
    assert!(
        !split[0].has("--ignored"),
        "and not about cargo's — the two sides must not be searched as one"
    );

    assert!(
        read("sudo apt-get update && sudo apt-get install -y protobuf-compiler").is_empty(),
        "a step with no cargo in it holds no cargo command"
    );
    assert_eq!(
        read("rustup show && cargo test --workspace").len(),
        1,
        "an operator ends one command and starts the next"
    );
    assert!(
        read("cargo run -q --manifest-path tools/item-citations/Cargo.toml --bin item-citations -- --workspace Cargo.toml")
            .first()
            .is_some_and(|command| command.subcommand() == Some("run")),
        "a path holding the word Cargo.toml is not the start of a second command"
    );

    let joined = read("cargo test --features=server/otlp,server/tls --test=smoke");
    assert_eq!(joined.len(), 1);
    assert_eq!(
        joined[0].value(&["--features"]),
        Some("server/otlp,server/tls"),
        "`--flag=value` is the same flag as `--flag value`; reading only the \
         second answers ABSENT for the first, which R1082 carried as a limit"
    );
    assert!(
        joined[0].has("--test"),
        "and a narrowing selector written that way still narrows: {:?}",
        joined[0].cargo_args
    );

    let folded = read("cargo clippy -p mnemosyne-server --all-features \\\n  --all-targets --locked -- -D warnings");
    assert_eq!(
        folded.len(),
        1,
        "a trailing backslash continues the command"
    );
    assert!(
        folded[0].has("--all-targets"),
        "and the continued half is part of it: {folded:?}"
    );
}
