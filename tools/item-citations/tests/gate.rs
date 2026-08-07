//! The gate run against real cargo workspaces, built for each test and thrown
//! away with it.
//!
//! These are the assertions that say the gate is not vacuous. The unit tests in
//! the library decide verdicts from strings; these ones put a broken citation
//! in a real crate, hand the real binary a real workspace, and read what came
//! back — including the one case the gate exists for, where `cargo doc` on the
//! same workspace reports success while a citation in it names nothing.
//!
//! The fixtures live in a temporary directory OUTSIDE this repository. A
//! fixture crate inside the tree would be found by
//! `scripts/check-side-workspaces.sh`, which discovers every `[workspace]`
//! there is and would then try to lint one whose purpose is to be broken.

use std::path::Path;
use std::process::Command;

use tempfile::TempDir;

struct Run {
    ok: bool,
    stdout: String,
    stderr: String,
}

impl Run {
    fn says(&self, needle: &str) -> bool {
        self.stdout.contains(needle)
    }
}

/// Write a cargo workspace from `(relative path, contents)` pairs.
fn fixture(files: &[(&str, &str)]) -> TempDir {
    let dir = tempfile::tempdir().expect("a temporary directory");
    for (path, contents) in files {
        let full = dir.path().join(path);
        std::fs::create_dir_all(full.parent().expect("a parent")).expect("mkdir");
        std::fs::write(&full, contents).expect("write");
    }
    dir
}

fn gate(workspace: &Path) -> Run {
    let output = Command::new(env!("CARGO_BIN_EXE_item-citations"))
        .args(["--workspace", &workspace.display().to_string()])
        .env("CARGO_TARGET_DIR", workspace.join("target"))
        // The gate must not inherit a caller's rustdoc flags; this asserts that
        // by handing it a hostile one and expecting its own to win.
        .env("RUSTDOCFLAGS", "--this-flag-does-not-exist")
        .output()
        .expect("the gate binary runs");
    Run {
        ok: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// `cargo doc --workspace`, which is the gate this one replaces, run over the
/// same fixture so that what it misses is measured rather than asserted.
fn cargo_doc(workspace: &Path) -> bool {
    Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string()))
        .current_dir(workspace)
        .args([
            "doc",
            "--workspace",
            "--no-deps",
            "--document-private-items",
            "-q",
        ])
        .env("CARGO_TARGET_DIR", workspace.join("target-doc"))
        .env("RUSTDOCFLAGS", "-D rustdoc::broken_intra_doc_links")
        .output()
        .expect("cargo doc runs")
        .status
        .success()
}

const MANIFEST: &str = r#"
[workspace]
[package]
name = "fixture"
version = "0.1.0"
edition = "2021"
"#;

#[test]
fn a_clean_workspace_passes_and_says_how_many_targets_it_reached() {
    let dir = fixture(&[
        ("Cargo.toml", MANIFEST),
        (
            "src/lib.rs",
            "//! A crate whose only citation resolves: [`kept`].\n\
             pub fn kept() {}\n",
        ),
        (
            "tests/smoke.rs",
            "//! Nothing cited here.\n#[test]\nfn passes() {}\n",
        ),
    ]);
    let run = gate(dir.path());
    assert!(run.ok, "stdout:\n{}\nstderr:\n{}", run.stdout, run.stderr);
    assert!(
        run.says("reached 2/2 targets"),
        "the gate must account for both the library and the test target:\n{}",
        run.stdout
    );
}

#[test]
fn a_citation_naming_nothing_in_a_library_is_a_defect() {
    let dir = fixture(&[
        ("Cargo.toml", MANIFEST),
        (
            "src/lib.rs",
            "//! This crate cites [`NoSuchThing`], which it does not have.\n\
             pub fn kept() {}\n",
        ),
    ]);
    let run = gate(dir.path());
    assert!(!run.ok, "the gate must reject:\n{}", run.stdout);
    assert!(run.says("DEFECT"), "{}", run.stdout);
    assert!(run.says("NoSuchThing"), "{}", run.stdout);
}

/// The hole this gate was built to close, pinned from both sides.
///
/// A package whose binary shares its library's name has its binary DROPPED by
/// `cargo doc`: the two would write documentation to the same place, so cargo
/// documents the library alone and says nothing about it. Round 1077's gate was
/// `cargo doc --workspace`, and that is how `crates/mnemosyne-cli/src/main.rs`
/// carried a citation to a `DisclosureReveal` that exists nowhere while the
/// gate reported exit 0.
#[test]
fn a_binary_shadowed_by_its_library_is_reached_here_and_not_by_cargo_doc() {
    let dir = fixture(&[
        (
            "Cargo.toml",
            r#"
[workspace]
[package]
name = "fixture"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "fixture"
path = "src/main.rs"
"#,
        ),
        ("src/lib.rs", "//! Clean library.\npub fn kept() {}\n"),
        (
            "src/main.rs",
            // The binary USES its library, which is the shape that showed the
            // extern repair for test targets must not be applied here: cargo
            // already wires a binary to its package's library, and a second
            // `--extern` for it is E0464 and documents nothing.
            "//! The binary cites [`OnlyInTheBinary`], which does not exist.\n\
             use fixture::kept;\nfn main() { kept(); }\n",
        ),
    ]);

    assert!(
        cargo_doc(dir.path()),
        "the gate this one replaces must be GREEN here — if cargo doc ever \
         documents a shadowed binary, this test is measuring the wrong hole"
    );

    let run = gate(dir.path());
    assert!(
        !run.ok,
        "the shadowed binary must be reached and rejected:\n{}",
        run.stdout
    );
    assert!(run.says("OnlyInTheBinary"), "{}", run.stdout);
    assert!(run.says("Bin fixture"), "{}", run.stdout);
}

/// The other half of the same hole: `cargo doc` documents no test target at
/// all, and Round 1076 shipped a citation to a name that did not exist inside
/// one.
#[test]
fn a_citation_naming_nothing_in_a_test_target_is_reached_here_and_not_by_cargo_doc() {
    let dir = fixture(&[
        ("Cargo.toml", MANIFEST),
        ("src/lib.rs", "//! Clean library.\npub fn kept() {}\n"),
        (
            "tests/smoke.rs",
            "//! This test target cites [`OnlyInTheTest`], a helper it does not have.\n\
             #[test]\nfn passes() {}\n",
        ),
    ]);

    assert!(
        cargo_doc(dir.path()),
        "cargo doc must be green here — it documents no test target"
    );

    let run = gate(dir.path());
    assert!(!run.ok, "the test target must be rejected:\n{}", run.stdout);
    assert!(run.says("OnlyInTheTest"), "{}", run.stdout);
    assert!(run.says("Test smoke"), "{}", run.stdout);
}

/// The second authority, and the proof that it excuses a NAME rather than a
/// TARGET: two citations in one test target, one naming a test that exists and
/// one naming a test that does not, and only the second is a defect.
#[test]
fn the_harness_excuses_a_cited_test_and_convicts_a_cited_name_that_is_no_test() {
    let excused = fixture(&[
        ("Cargo.toml", MANIFEST),
        ("src/lib.rs", "//! Clean library.\n"),
        (
            "tests/smoke.rs",
            "//! The law next to it: [`a_real_test`] is what proves this.\n\
             #[test]\nfn a_real_test() {}\n",
        ),
    ]);
    let run = gate(excused.path());
    assert!(
        run.ok,
        "a citation to a sibling test must pass:\n{}\n{}",
        run.stdout, run.stderr
    );
    assert!(run.says("EXCUSED"), "{}", run.stdout);
    assert!(run.says("a_real_test"), "{}", run.stdout);
    assert!(
        run.says("reached 2/2 targets"),
        "an excused target is still a reached one:\n{}",
        run.stdout
    );

    let convicted = fixture(&[
        ("Cargo.toml", MANIFEST),
        ("src/lib.rs", "//! Clean library.\n"),
        (
            "tests/smoke.rs",
            "//! Two citations: [`a_real_test`] and [`a_test_that_was_renamed`].\n\
             #[test]\nfn a_real_test() {}\n",
        ),
    ]);
    let run = gate(convicted.path());
    assert!(
        !run.ok,
        "a citation to a test that does not exist must NOT be excused:\n{}",
        run.stdout
    );
    assert!(run.says("a_test_that_was_renamed"), "{}", run.stdout);
    assert!(
        !run.stdout.contains("DEFECT")
            || !run
                .stdout
                .lines()
                .any(|l| l.contains("DEFECT") && l.contains("`a_real_test`")),
        "the real test must not be convicted alongside it:\n{}",
        run.stdout
    );
}

/// A workspace with nothing to document is refused rather than passed. An
/// empty population and a clean one are the same green otherwise.
#[test]
fn a_workspace_with_no_documentable_target_is_refused() {
    let dir = fixture(&[("Cargo.toml", "[workspace]\nmembers = []\n")]);
    let run = gate(dir.path());
    assert!(
        !run.ok,
        "an empty population must not pass:\n{}",
        run.stdout
    );
    assert!(
        run.stderr.contains("no documentable target") || run.stdout.contains("no documentable"),
        "stdout:\n{}\nstderr:\n{}",
        run.stdout,
        run.stderr
    );
}

/// The hole cargo itself leaves: it hands a test target's documentation unit
/// an `--extern` for every dependency and NONE for the crate under test, so a
/// test that uses its own crate cannot be documented at all. Twenty-nine of
/// this repository's test targets are that shape. Both halves are asserted —
/// that such a target is reached, and that a citation inside it is still read.
#[test]
fn a_test_target_that_uses_its_own_crate_is_reached_and_still_read() {
    let clean = fixture(&[
        ("Cargo.toml", MANIFEST),
        ("src/lib.rs", "//! Clean library.\npub fn kept() {}\n"),
        (
            "tests/smoke.rs",
            "//! Cites its own crate's [`fixture::kept`].\n\
             use fixture::kept;\n#[test]\nfn passes() { kept(); }\n",
        ),
    ]);
    let run = gate(clean.path());
    assert!(
        run.ok,
        "a test target using its own crate must be documentable:\n{}\n{}",
        run.stdout, run.stderr
    );
    assert!(
        run.says("reached 2/2 targets"),
        "and it must be COUNTED, not skipped:\n{}",
        run.stdout
    );

    let broken = fixture(&[
        ("Cargo.toml", MANIFEST),
        ("src/lib.rs", "//! Clean library.\npub fn kept() {}\n"),
        (
            "tests/smoke.rs",
            "//! Cites [`fixture::gone`], which the crate does not have.\n\
             use fixture::kept;\n#[test]\nfn passes() { kept(); }\n",
        ),
    ]);
    let run = gate(broken.path());
    assert!(
        !run.ok,
        "the citation in it must still be read:\n{}",
        run.stdout
    );
    assert!(run.says("DEFECT"), "{}", run.stdout);
    assert!(run.says("fixture::gone"), "{}", run.stdout);
}

/// THE FACT THE SPLIT RESTS ON, asked of cargo rather than declared here.
///
/// [`item_citations::TargetKind::is_wired_to_its_library`] decides which kinds
/// receive the extern repair, and it is a claim about a tool this repository
/// does not own. So every kind is put to cargo directly, from its own verbose
/// output, on one fixture that has all four. If a later cargo changes any of
/// these answers, this test goes red and says which — which is the correct
/// outcome: the repair becomes unnecessary rather than silently wrong.
///
/// The answer this measured, on cargo 1.94.1: a BINARY and an EXAMPLE each
/// receive their package's library; an integration TEST and a BENCH receive
/// nothing. Benches and examples are on opposite sides of that line, which no
/// amount of reasoning about "what a target is" would have produced.
#[test]
fn cargo_hands_a_binary_and_an_example_their_library_and_a_test_and_bench_nothing() {
    let dir = fixture(&[
        (
            "Cargo.toml",
            r#"
[workspace]
[package]
name = "fixture"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "runner"
path = "src/main.rs"

[[bench]]
name = "measured"
path = "benches/measured.rs"
harness = false
"#,
        ),
        ("src/lib.rs", "pub fn kept() {}\n"),
        ("src/main.rs", "use fixture::kept;\nfn main() { kept(); }\n"),
        (
            "tests/smoke.rs",
            "use fixture::kept;\n#[test]\nfn passes() { kept(); }\n",
        ),
        (
            "benches/measured.rs",
            "use fixture::kept;\nfn main() { kept(); }\n",
        ),
        (
            "examples/shown.rs",
            "use fixture::kept;\nfn main() { kept(); }\n",
        ),
    ]);

    let rustdoc_lines = |selector: &str| -> String {
        let output = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string()))
            .current_dir(dir.path())
            .args(["rustdoc", "-v", "-p", "fixture", selector, "--keep-going"])
            .env("CARGO_TARGET_DIR", dir.path().join(format!("t{selector}")))
            .output()
            .expect("cargo runs");
        let all = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        all.lines()
            // `Running \`…\`` only: cargo repeats a failed command verbatim in
            // its error, and counting that as a second invocation would make
            // "exactly one line for this target" fail on every failing target.
            .filter(|line| line.trim_start().starts_with("Running `"))
            .filter(|line| line.contains("--crate-name"))
            .map(str::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    };

    let line_for = |selector: &str, crate_name: &str| -> String {
        let lines = rustdoc_lines(selector);
        let found: Vec<&str> = lines
            .lines()
            .filter(|line| line.contains(&format!("--crate-name {crate_name}")))
            .collect();
        assert_eq!(
            found.len(),
            1,
            "expected exactly one rustdoc line for `{crate_name}` under {selector}:\n{lines}"
        );
        found[0].to_string()
    };

    for (selector, crate_name) in [("--bins", "runner"), ("--examples", "shown")] {
        let line = line_for(selector, crate_name);
        assert!(
            line.contains("--extern fixture="),
            "cargo is expected to hand a {selector} target its package's library — if that \
             has changed, this kind needs the extern repair too:\n{line}"
        );
    }
    for (selector, crate_name) in [("--tests", "smoke"), ("--benches", "measured")] {
        let line = line_for(selector, crate_name);
        assert!(
            !line.contains("--extern fixture="),
            "cargo is expected to hand a {selector} target nothing for its own crate — if \
             this has been fixed, narrow the extern repair rather than keeping it:\n{line}"
        );
    }
}

/// A target rustdoc could not open at all is neither clean nor a citation
/// defect, and the gate says so rather than counting it as either. Without the
/// reach accounting this run reports zero refused citations, which is the exact
/// shape of a clean one.
#[test]
fn a_target_rustdoc_could_not_open_is_reported_rather_than_counted_as_clean() {
    let dir = fixture(&[
        ("Cargo.toml", MANIFEST),
        ("src/lib.rs", "//! Clean library.\npub fn kept() {}\n"),
        (
            "tests/smoke.rs",
            "//! This target does not parse.\nfn broken( {\n",
        ),
    ]);
    let run = gate(dir.path());
    assert!(
        !run.ok,
        "a target that could not be documented must not pass:\n{}",
        run.stdout
    );
    assert!(
        run.says("INDECISIVE") || run.says("UNEXPLAINED"),
        "{}",
        run.stdout
    );
    assert!(
        run.says("reached 1/2 targets"),
        "the reach count is what makes this visible:\n{}",
        run.stdout
    );
}

/// The gate is pointed at a workspace, and being pointed at nothing is an
/// error rather than a default.
#[test]
fn a_manifest_that_is_not_there_is_an_error_not_an_empty_pass() {
    let dir = fixture(&[("unrelated.txt", "")]);
    let run = Run {
        ok: Command::new(env!("CARGO_BIN_EXE_item-citations"))
            .args(["--workspace", &dir.path().display().to_string()])
            .output()
            .expect("runs")
            .status
            .success(),
        stdout: String::new(),
        stderr: String::new(),
    };
    assert!(!run.ok);
}
