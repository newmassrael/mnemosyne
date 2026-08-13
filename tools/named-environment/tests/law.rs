//! The law, asked of workspaces this test builds — one per answer the gate can
//! give.
//!
//! Built OUTSIDE this repository, for the reason `blind-waits` gives: a fixture
//! carrying its own `[workspace]` inside the tree would be discovered by
//! `scripts/check-side-workspaces.sh`, which would then lint a workspace whose
//! whole purpose is to be rejected.
//!
//! Each fixture is a real cargo workspace with a real binary and a real test,
//! and the gate runs over it exactly as a hook runs it. A fixture that only
//! LOOKED like one would be testing the parser against itself.

use std::path::Path;
use std::process::Command;

use named_environment::{run, Refusal};
use tempfile::TempDir;

/// A workspace with one binary and one test, written from the two bodies given.
fn workspace(binary: &str, test: &str) -> TempDir {
    let at = TempDir::new().expect("tempdir");
    let root = at.path();
    std::fs::create_dir_all(root.join("src")).expect("mkdir src");
    std::fs::create_dir_all(root.join("tests")).expect("mkdir tests");
    std::fs::write(
        root.join("Cargo.toml"),
        // `autotests = false` with the test named explicitly, which is the
        // shape the repository's own consolidated suites use — and the shape
        // that makes a `#[path]` child a MODULE of that target rather than a
        // second target cargo discovers on its own.
        "[workspace]\n\
         [package]\nname = \"probe\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\
         autotests = false\n\
         [[bin]]\nname = \"probe\"\npath = \"src/main.rs\"\n\
         [[test]]\nname = \"entrance\"\npath = \"tests/entrance.rs\"\n",
    )
    .expect("write manifest");
    std::fs::write(root.join("src/main.rs"), binary).expect("write binary");
    std::fs::write(root.join("tests/entrance.rs"), test).expect("write test");
    at
}

fn gate(at: &Path) -> named_environment::Report {
    run(&at.join("Cargo.toml")).expect("the gate runs over the fixture")
}

/// The shape the gate exists for: the program reads a variable, the test that
/// spawns it never says its name.
#[test]
fn a_variable_the_spawning_test_never_says_is_a_finding() {
    let at = workspace(
        "fn main() { let _ = std::env::var(\"GITHUB_REF_NAME\"); }",
        "#[test]\nfn spawns() {\n    \
         let _ = std::process::Command::new(env!(\"CARGO_BIN_EXE_probe\")).output();\n}",
    );
    let report = gate(at.path());
    assert_eq!(report.verdict(), Ok(()), "{report:?}");
    let named: Vec<&str> = report
        .findings
        .iter()
        .map(|finding| finding.variable.as_str())
        .collect();
    assert_eq!(named, ["GITHUB_REF_NAME"], "{report:?}");
    assert_eq!(report.findings[0].test_target, "entrance");
    assert_eq!(report.findings[0].binary, "probe");
}

/// And the same tree with the name said is clean — the control, without which
/// the test above only proves the gate says something.
#[test]
fn the_same_tree_is_clean_once_the_test_names_it() {
    let at = workspace(
        "fn main() { let _ = std::env::var(\"GITHUB_REF_NAME\"); }",
        "#[test]\nfn spawns() {\n    \
         let _ = std::process::Command::new(env!(\"CARGO_BIN_EXE_probe\"))\n        \
         .env_remove(\"GITHUB_REF_NAME\")\n        .output();\n}",
    );
    let report = gate(at.path());
    assert_eq!(report.verdict(), Ok(()), "{report:?}");
    assert!(report.findings.is_empty(), "{report:?}");
    assert_eq!(report.reach.named, 1, "{report:?}");
}

/// A name spelled by a constant is a name. The first draft of the walk this
/// crate grew out of read literals only and reported three of five reads as the
/// whole set — the same disease it was written to cure.
#[test]
fn a_variable_named_by_a_constant_resolves_on_both_sides() {
    let at = workspace(
        "const WHICH: &str = \"MNEMOSYNE_RANGE_FROM\";\n\
         fn main() { let _ = std::env::var(WHICH); }",
        "const WHICH: &str = \"MNEMOSYNE_RANGE_FROM\";\n\
         #[test]\nfn spawns() {\n    \
         let _ = std::process::Command::new(env!(\"CARGO_BIN_EXE_probe\"))\n        \
         .env_remove(WHICH)\n        .output();\n}",
    );
    let report = gate(at.path());
    assert_eq!(report.verdict(), Ok(()), "{report:?}");
    assert!(report.findings.is_empty(), "{report:?}");
    assert!(
        report.read_by["probe"].contains("MNEMOSYNE_RANGE_FROM"),
        "the constant did not resolve on the reading side: {report:?}"
    );
}

/// A name inside a macro body is a name. Syn does not walk macro bodies, and a
/// fixture that builds its environment with `vec![..]` — which is what this
/// repository's tidiest one does — would otherwise read as naming nothing.
#[test]
fn a_variable_named_inside_a_macro_body_still_counts() {
    let at = workspace(
        "fn main() { let _ = std::env::var(\"GITHUB_RUN_ID\"); }",
        "#[test]\nfn spawns() {\n    \
         for (name, value) in vec![(\"GITHUB_RUN_ID\", \"7\")] {\n        \
         let _ = std::process::Command::new(env!(\"CARGO_BIN_EXE_probe\"))\n            \
         .env(name, value)\n            .output();\n    }\n}",
    );
    let report = gate(at.path());
    assert_eq!(report.verdict(), Ok(()), "{report:?}");
    assert!(report.findings.is_empty(), "{report:?}");
}

/// A read whose name is a PARAMETER resolves through the call sites of the
/// function that takes it — `required("X")` is a read of `X`, however many hops
/// it takes to get there.
#[test]
fn a_variable_read_through_a_helper_resolves_to_what_its_callers_pass() {
    let at = workspace(
        "fn required(variable: &str) -> String {\n    \
         std::env::var(variable).unwrap_or_default()\n}\n\
         fn main() { let _ = required(\"MNEMOSYNE_CACHE_HIT\"); }",
        "#[test]\nfn spawns() {\n    \
         let _ = std::process::Command::new(env!(\"CARGO_BIN_EXE_probe\")).output();\n}",
    );
    let report = gate(at.path());
    assert_eq!(report.verdict(), Ok(()), "{report:?}");
    assert_eq!(
        report.findings.first().map(|f| f.variable.as_str()),
        Some("MNEMOSYNE_CACHE_HIT"),
        "the helper's read never resolved: {report:?}"
    );
}

/// A name the walk cannot resolve is a REFUSAL, not a zero. "I could not read
/// this one" and "there is nothing here" must never arrive as the same answer.
#[test]
fn a_name_the_walk_cannot_resolve_refuses_rather_than_counting_nothing() {
    let at = workspace(
        "fn main() {\n    \
         let which = std::env::args().nth(1).unwrap_or_default();\n    \
         let _ = std::env::var(which);\n}",
        "#[test]\nfn spawns() {\n    \
         let _ = std::process::Command::new(env!(\"CARGO_BIN_EXE_probe\")).output();\n}",
    );
    let report = gate(at.path());
    assert!(
        matches!(report.verdict(), Err(Refusal::Unresolved(_))),
        "{report:?}"
    );
    assert!(
        report.findings.is_empty(),
        "a refusal is not a verdict, so it must not carry findings: {report:?}"
    );
}

/// A test that spawns nothing is not this law's business, and the report says
/// so rather than printing the same thing a clean tree prints.
#[test]
fn a_workspace_whose_tests_spawn_nothing_is_answered_not_judged() {
    let at = workspace(
        "fn main() { let _ = std::env::var(\"GITHUB_REF_NAME\"); }",
        "#[test]\nfn reads_nothing() { assert!(true); }",
    );
    let report = gate(at.path());
    assert_eq!(report.verdict(), Ok(()), "{report:?}");
    assert!(!report.found_a_spawn(), "{report:?}");
    assert!(report.findings.is_empty(), "{report:?}");
}

/// Clearing the environment names all of it at once.
#[test]
fn a_test_that_clears_the_environment_has_named_everything() {
    let at = workspace(
        "fn main() { let _ = std::env::var(\"GITHUB_REF_NAME\"); }",
        "#[test]\nfn spawns() {\n    \
         let _ = std::process::Command::new(env!(\"CARGO_BIN_EXE_probe\"))\n        \
         .env_clear()\n        .output();\n}",
    );
    let report = gate(at.path());
    assert_eq!(report.verdict(), Ok(()), "{report:?}");
    assert!(report.findings.is_empty(), "{report:?}");
    assert_eq!(report.reach.clearing_targets, 1, "{report:?}");
}

/// A crate root owns the directory it sits in whatever it is called, and an
/// integration test is where that matters: `tests/all.rs` is a crate root, and
/// reading it as an ordinary module puts its `#[path]` children under
/// `tests/all/`, where none of them are. Measured — the first version of this
/// walk did exactly that and could not read 96 files of this repository.
#[test]
fn a_test_root_that_is_not_called_main_still_owns_its_own_directory() {
    let at = workspace(
        "fn main() { let _ = std::env::var(\"GITHUB_REF_NAME\"); }",
        "#[path = \"spawner.rs\"]\nmod spawner;\n",
    );
    std::fs::write(
        at.path().join("tests/spawner.rs"),
        "#[test]\nfn spawns() {\n    \
         let _ = std::process::Command::new(env!(\"CARGO_BIN_EXE_probe\"))\n        \
         .env_remove(\"GITHUB_REF_NAME\")\n        .output();\n}",
    )
    .expect("write the module the root declares");
    let report = gate(at.path());
    assert_eq!(report.verdict(), Ok(()), "{report:?}");
    assert!(
        report.reach.spawning_targets == 1,
        "the root's `#[path]` child was never opened, so the spawn was invisible: {report:?}"
    );
    assert!(report.findings.is_empty(), "{report:?}");
}

/// The binary's reads include what a LOCAL dependency reads on its behalf: a
/// program does not stop reading the environment because the call is one crate
/// away.
#[test]
fn a_read_inside_a_path_dependency_is_still_the_binarys_read() {
    let at = TempDir::new().expect("tempdir");
    let root = at.path();
    for directory in ["src", "tests", "helper/src"] {
        std::fs::create_dir_all(root.join(directory)).expect("mkdir");
    }
    std::fs::write(
        root.join("Cargo.toml"),
        "[workspace]\n\
         [package]\nname = \"probe\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\
         autotests = false\n\
         [[bin]]\nname = \"probe\"\npath = \"src/main.rs\"\n\
         [[test]]\nname = \"entrance\"\npath = \"tests/entrance.rs\"\n\
         [dependencies]\nhelper = { path = \"helper\" }\n",
    )
    .expect("write manifest");
    std::fs::write(
        root.join("helper/Cargo.toml"),
        "[package]\nname = \"helper\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write helper manifest");
    std::fs::write(
        root.join("helper/src/lib.rs"),
        "pub fn look() -> Option<String> { std::env::var(\"XDG_DATA_HOME\").ok() }",
    )
    .expect("write helper");
    std::fs::write(
        root.join("src/main.rs"),
        "fn main() { let _ = helper::look(); }",
    )
    .expect("write binary");
    std::fs::write(
        root.join("tests/entrance.rs"),
        "#[test]\nfn spawns() {\n    \
         let _ = std::process::Command::new(env!(\"CARGO_BIN_EXE_probe\")).output();\n}",
    )
    .expect("write test");

    let report = gate(root);
    assert_eq!(report.verdict(), Ok(()), "{report:?}");
    assert_eq!(
        report.findings.first().map(|f| f.variable.as_str()),
        Some("XDG_DATA_HOME"),
        "the dependency's read never reached the binary: {report:?}"
    );
}

/// The gate runs as a hook runs it, and answers with the exit code the hook
/// branches on: 0 clean, 1 judged and defective, 2 not judged.
#[test]
fn the_binary_answers_a_hook_in_three_codes() {
    let clean = workspace(
        "fn main() { let _ = std::env::var(\"GITHUB_REF_NAME\"); }",
        "#[test]\nfn spawns() {\n    \
         let _ = std::process::Command::new(env!(\"CARGO_BIN_EXE_probe\"))\n        \
         .env_remove(\"GITHUB_REF_NAME\")\n        .output();\n}",
    );
    let defective = workspace(
        "fn main() { let _ = std::env::var(\"GITHUB_REF_NAME\"); }",
        "#[test]\nfn spawns() {\n    \
         let _ = std::process::Command::new(env!(\"CARGO_BIN_EXE_probe\")).output();\n}",
    );
    let unreadable = workspace(
        "fn main() {\n    \
         let which = std::env::args().nth(1).unwrap_or_default();\n    \
         let _ = std::env::var(which);\n}",
        "#[test]\nfn spawns() {\n    \
         let _ = std::process::Command::new(env!(\"CARGO_BIN_EXE_probe\")).output();\n}",
    );
    for (at, expected) in [(&clean, 0), (&defective, 1), (&unreadable, 2)] {
        let out = Command::new(env!("CARGO_BIN_EXE_named-environment"))
            .args([
                "--workspace",
                &at.path().join("Cargo.toml").display().to_string(),
            ])
            // NAMED RATHER THAN INHERITED, by this gate's own law: it asks
            // cargo for the census, and which cargo it asks is `$CARGO`.
            .env("CARGO", env!("CARGO"))
            .output()
            .expect("the gate binary runs");
        assert_eq!(
            out.status.code(),
            Some(expected),
            "stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
}
