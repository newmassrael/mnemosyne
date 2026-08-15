//! The program as a PROCESS — its arguments, its two streams, and its exit code.
//!
//! WHY A PROCESS AND NOT A CALL. R1096: a decision that lives in `main.rs` has no
//! reader, and R1127 to R1129 measured three gates in this repository whose
//! `main.rs` documented an exit code nothing was running. What the library
//! answers is held in `github.rs` and `law.rs`; what is left here is exactly the
//! part those cannot reach — whether the sentences the library composes actually
//! come out of this binary, and whether the code it exits with is the one its own
//! doc comment claims.
//!
//! `gh` IS STUBBED BY PATH AND THE STUB RETURNS THE RECORDINGS, unfiltered. That
//! is the difference from the hook suite this replaces: its stub returned lines
//! `gh -q` had already flattened, so nothing between the hook and GitHub was ever
//! executed, and two renamed fields left it at 14 passed / 0 failed. Here the stub
//! hands over the same bytes GitHub sent and the reading runs for real.
//!
//! The stubs are `src/bin/gh-stub.rs` and `src/bin/gh-unreachable.rs`, programs
//! cargo builds and this fixture SYMLINKS into place: R1192's rule, that nothing
//! here writes a file it then runs.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

const SHA: &str = "2d630331b1279e3b7a28985876b53ef0b07fbe77";

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(name)
}

/// Put one of this crate's stub programs on `at` under the name `gh`.
///
/// A SYMLINK TO A BINARY CARGO BUILT, never a file this process writes (R1192).
/// `exec` on a file some process holds open for writing fails with `ETXTBSY`,
/// and the holder is a sibling test's fork rather than this thread — so the
/// scripts this used to write were correct alone and a flake the moment anything
/// else was running. `src/bin/gh-stub.rs` answers the recordings and checks what
/// it was asked; `src/bin/gh-unreachable.rs` is a `gh` that is installed and
/// fails.
fn link_gh(at: &Path, program: &str) {
    std::os::unix::fs::symlink(program, at).expect("link the gh stub");
}

/// A directory holding one `gh`, and the log of anything it was asked wrongly.
struct Stub {
    dir: TempDir,
}

impl Stub {
    /// A `gh` that answers with the recorded bodies.
    fn recording() -> Self {
        let stub = Stub {
            dir: TempDir::new().expect("tempdir"),
        };
        link_gh(&stub.dir.path().join("gh"), env!("CARGO_BIN_EXE_gh-stub"));
        stub
    }

    /// A `gh` that is installed and fails.
    fn unreachable() -> Self {
        let stub = Stub {
            dir: TempDir::new().expect("tempdir"),
        };
        link_gh(
            &stub.dir.path().join("gh"),
            env!("CARGO_BIN_EXE_gh-unreachable"),
        );
        stub
    }

    /// A directory with no `gh` in it at all.
    fn absent() -> Self {
        Stub {
            dir: TempDir::new().expect("tempdir"),
        }
    }

    fn log(&self) -> PathBuf {
        self.dir.path().join("gh-contract-violations.log")
    }

    /// Run the reporter with this stub as the whole of `PATH`.
    fn run(&self, arguments: &[&str], checks: &Path) -> Output {
        let out = Command::new(env!("CARGO_BIN_EXE_ci-state"))
            .args(arguments)
            .current_dir(self.dir.path())
            // THE WHOLE OF `PATH`, so "gh is not installed" is a state this test
            // can actually produce rather than one it asserts about in prose.
            .env("PATH", self.dir.path())
            .env("GH_STUB_LOG", self.log())
            .env("GH_STUB_SHA", SHA)
            .env("GH_STUB_CHECK", "93478488570")
            .env("GH_STUB_CHECKS", checks)
            .env("GH_STUB_ANNOTATIONS", fixture("annotations.json"))
            .output()
            .expect("the reporter runs");
        let asked_wrongly = fs::read_to_string(self.log()).unwrap_or_default();
        assert!(
            asked_wrongly.is_empty(),
            "the reporter asked `gh` for something else than it says it does:\n{asked_wrongly}"
        );
        out
    }
}

fn said(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// The recorded red commit comes out of this binary as a red commit.
///
/// END TO END over the real bytes: the stub hands over what GitHub sent, the
/// reading runs, and the sentences reach stdout. This is the case the hook suite
/// could not have — its stub answered with lines `gh -q` had already made.
#[test]
fn the_recorded_red_commit_is_reported_as_red_with_its_annotation() {
    let stub = Stub::recording();
    let out = stub.run(&[SHA], &fixture("check-runs.one-page.json"));
    let said = said(&out);
    assert_eq!(
        out.status.code(),
        Some(0),
        "reporting is not failing: {said}"
    );
    assert!(said.contains("9 check(s)"), "{said}");
    assert!(
        said.contains("every cache declared is one CI keeps"),
        "the job that failed is named: {said}"
    );
    assert!(said.contains("is RED"), "{said}");
    assert!(
        said.contains("1 distinct of 1 reported") && said.contains("exit code 1."),
        "and the annotation behind it is printed: {said}"
    );
}

/// Three pages of the same answer report the same commit.
///
/// `--paginate` IS EXERCISED THROUGH THE PROCESS, because the flag is in the
/// library's query and the stream shape only exists once `gh` has printed it.
#[test]
fn the_paginated_recording_reports_the_same_commit() {
    let stub = Stub::recording();
    let out = stub.run(&[SHA], &fixture("check-runs.paginated.json"));
    let said = said(&out);
    assert_eq!(out.status.code(), Some(0), "{said}");
    assert!(
        said.contains("9 check(s)") && said.contains("is RED"),
        "{said}"
    );
}

/// A `gh` that is not on this machine is REPORTED, and reporting is not failing.
#[test]
fn a_gh_that_is_not_installed_is_reported_and_the_reporter_still_exits_zero() {
    let stub = Stub::absent();
    let out = stub.run(&[SHA], &fixture("check-runs.one-page.json"));
    let said = said(&out);
    assert_eq!(
        out.status.code(),
        Some(0),
        "not being able to look is not a violation: {said}"
    );
    assert!(said.contains("not installed"), "{said}");
    assert!(
        said.contains(SHA),
        "and it names the commit it could not report on: {said}"
    );
}

/// A `gh` that cannot reach GitHub is a different sentence from one that is
/// missing.
///
/// COLLAPSING THEM WOULD SEND A READER TO INSTALL SOMETHING THEY ALREADY HAVE.
#[test]
fn a_gh_that_cannot_reach_github_says_so_rather_than_saying_it_is_missing() {
    let stub = Stub::unreachable();
    let out = stub.run(&[SHA], &fixture("check-runs.one-page.json"));
    let said = said(&out);
    assert_eq!(out.status.code(), Some(0), "{said}");
    assert!(said.contains("unknown"), "{said}");
    assert!(
        !said.contains("not installed"),
        "it is installed; it failed: {said}"
    );
}

/// An answer this reporter cannot read reaches stdout instead of being swallowed.
///
/// THE POINT OF THE WHOLE ROUND. A drift in GitHub's shape must arrive as a named
/// refusal in front of the person pushing, and not as a quieter report.
#[test]
fn an_answer_the_reporter_cannot_read_is_printed_rather_than_swallowed() {
    let stub = Stub::recording();
    let short = stub.dir.path().join("short.json");
    fs::write(&short, r#"{"total_count":9,"check_runs":[]}"#).expect("write");
    let out = stub.run(&[SHA], &short);
    let said = said(&out);
    assert_eq!(
        out.status.code(),
        Some(0),
        "a shape it cannot read is still a report: {said}"
    );
    assert!(
        said.contains("9 check(s) and 0 arrived"),
        "and it says what it was told and what came: {said}"
    );
}

/// Asked about no commit, this reporter says so and exits 2.
///
/// THE ONE NON-ZERO CODE IT HAS, and `.githooks/pre-push` prints a different
/// sentence for it. A check that stays quiet when it cannot answer is
/// indistinguishable from one that answered "fine".
#[test]
fn a_reporter_given_no_commit_exits_two_rather_than_reporting_on_nothing() {
    let stub = Stub::recording();
    for arguments in [vec![], vec![SHA, SHA]] {
        let out = stub.run(&arguments, &fixture("check-runs.one-page.json"));
        let said = said(&out);
        assert_eq!(
            out.status.code(),
            Some(2),
            "given {} argument(s): {said}",
            arguments.len()
        );
        assert!(said.contains("usage"), "{said}");
    }
}
