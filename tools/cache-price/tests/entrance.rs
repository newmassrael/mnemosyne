//! The program as a PROCESS — its arguments, its two streams, and its exit code.
//!
//! WHY A PROCESS AND NOT A CALL (R1096): a decision that lives in `main.rs` has no
//! reader, and R1129 measured three gates in this repository whose `main.rs`
//! documented an exit code nothing was running. What the library answers is held
//! in `github.rs` and `law.rs`; what is left here is whether the numbers the
//! library computes actually come out of this binary, and whether the code it
//! exits with is the one its own doc comment claims.
//!
//! `gh` IS STUBBED BY PATH AND ANSWERS WITH THE RECORDINGS, unfiltered — the same
//! discipline `tools/ci-state` adopted in R1136 after two `gh -q` expressions in a
//! hook were measured silent.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

/// Answers the two endpoints this program asks for, out of files on disk.
///
/// ABSOLUTE PATHS INSIDE: `PATH` is the stub's directory and nothing else, so
/// "gh is not installed" is a state these tests can PRODUCE rather than assert
/// about in prose — and a stub resolving its own interpreter through `PATH` would
/// then fail for a reason about the test.
const GH_STUB: &str = r#"#!/bin/bash
violate() { echo "$1" >> "$GH_STUB_LOG"; }
[[ "${1:-}" == "api" ]] || violate "gh was not asked for the api: $*"
case "$*" in
    *"/actions/workflows/"*"/runs?per_page="*)
        [[ "$*" == *"per_page=$GH_STUB_WANTED"* ]] \
            || violate "the run list asked for a different sample than the program was given: $*"
        exec /bin/cat "$GH_STUB_RUNS"
        ;;
    *"--paginate"*"/jobs")
        exec /bin/cat "$GH_STUB_JOBS"
        ;;
    *)
        violate "gh hit an unexpected endpoint: $*"
        exit 1
        ;;
esac
"#;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(name)
}

fn write_exec(path: &Path, body: &str) {
    fs::write(path, body).expect("write");
    let mut perms = fs::metadata(path).expect("stat").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).expect("chmod");
}

struct Stub {
    dir: TempDir,
    with_gh: bool,
}

impl Stub {
    fn recording() -> Self {
        let stub = Stub {
            dir: TempDir::new().expect("tempdir"),
            with_gh: true,
        };
        write_exec(&stub.dir.path().join("gh"), GH_STUB);
        stub
    }

    fn without_gh() -> Self {
        Stub {
            dir: TempDir::new().expect("tempdir"),
            with_gh: false,
        }
    }

    fn run(&self, arguments: &[&str]) -> Output {
        let log = self.dir.path().join("gh-contract-violations.log");
        let out = Command::new(env!("CARGO_BIN_EXE_cache-price"))
            .args(arguments)
            .current_dir(self.dir.path())
            .env("PATH", self.dir.path())
            .env("GH_STUB_LOG", &log)
            .env("GH_STUB_WANTED", arguments.get(1).copied().unwrap_or(""))
            .env("GH_STUB_RUNS", fixture("runs.json"))
            .env("GH_STUB_JOBS", fixture("jobs.green.json"))
            .output()
            .expect("the program runs");
        if self.with_gh {
            let asked_wrongly = fs::read_to_string(&log).unwrap_or_default();
            assert!(
                asked_wrongly.is_empty(),
                "the program asked `gh` for something else than it says it does:\n{asked_wrongly}"
            );
        }
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

/// The recordings come out of this binary as the prices they carry.
///
/// END TO END: the stub hands over the bytes GitHub sent, the reading and the
/// pairing run, and the numbers reach stdout. `144` and `113` are the build
/// directory's restore and save in the recorded run — the two seconds figures this
/// whole round turns on.
#[test]
fn the_recorded_sample_is_priced_and_printed() {
    let out = Stub::recording().run(&["mnemosyne-validate.yml", "3"]);
    let said = said(&out);
    assert_eq!(out.status.code(), Some(0), "{said}");
    assert!(said.contains("3 run(s)"), "{said}");
    assert!(
        said.contains("every test compiled is one CI runs :: cargo (unrun tests, build directory)"),
        "the cache this round is about is named: {said}"
    );
    assert!(
        said.contains("max=144") && said.contains("max=113"),
        "and its restore and save are printed: {said}"
    );
    assert!(
        said.contains("whole-run cache overhead"),
        "and so is what a run spends on caches altogether: {said}"
    );
    assert!(
        said.contains("WHAT THIS DOES NOT SAY"),
        "with the half it cannot measure stated rather than implied: {said}"
    );
}

/// A `gh` that is not on this machine is reported, and the program exits 2.
///
/// IT PRICES NOTHING WITHOUT AN ANSWER, which is why this is not the reporting
/// exit `tools/ci-state` uses: that program's job is to say what it can see, and
/// this one's is to produce a measurement. A measurement that could not be taken
/// is not a measurement of zero.
#[test]
fn without_gh_there_is_no_measurement_and_the_program_says_so() {
    let out = Stub::without_gh().run(&["mnemosyne-validate.yml", "3"]);
    let said = said(&out);
    assert_eq!(out.status.code(), Some(2), "{said}");
    assert!(said.contains("not installed"), "{said}");
}

/// Asked for a sample it cannot take, this program exits 2 rather than printing an
/// empty table.
#[test]
fn a_sample_that_cannot_be_taken_exits_two() {
    for arguments in [
        vec![],
        vec!["mnemosyne-validate.yml"],
        vec!["mnemosyne-validate.yml", "3", "extra"],
        vec!["mnemosyne-validate.yml", "some"],
        vec!["mnemosyne-validate.yml", "0"],
    ] {
        let out = Stub::recording().run(&arguments);
        assert_eq!(
            out.status.code(),
            Some(2),
            "for {arguments:?}: {}",
            said(&out)
        );
    }
}
