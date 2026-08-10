//! The three answers this gate gives, asked of the PROGRAM.
//!
//! R1104 moved this gate's WORDS into the library so that a suite could read
//! them, and left its EXIT CODES here — `2` when a refusal is `Unreached`, "I
//! could not look", and `1` when the caches themselves break the law. Those are
//! two answers a hook acts on differently, and collapsing them is the failure
//! R1078 named. R1129 measured whether anything would notice: the injection that
//! makes both answers `1` went SILENT across fifty-two tests, because nothing
//! ran the binary. Moving the words was half the repair; this is the other half.
//!
//! `gh` IS STUBBED, WHICH IS THE ONLY WAY TO ASK THIS AT ALL. What GitHub holds
//! for a repository is not something a test can arrange, and the gate's whole
//! job is to hold that against what the workflows declare — so the stub supplies
//! the one side and the fixture's workflow supplies the other. The stub prints
//! the exact rows `held_caches` parses, which is the same discipline
//! `crates/mnemosyne-cli/tests/git_hooks_smoke.rs` uses on the pre-push hook.
//!
//! AND THE RUNNER'S OWN VARIABLES ARE REMOVED RATHER THAN ASSUMED ABSENT. This
//! suite runs on a runner too, where `GITHUB_RUN_ID` is set and would send the
//! gate down a second reading it has no stub for — green here and red there,
//! which is the shape R1119 paid for.

use std::path::Path;
use std::process::{Command, Output};

/// Under the 10 GB budget, and comfortably.
const SMALL: u64 = 1_000_000_000;

/// Over it on its own, which is what makes the finding a finding.
const HUGE: u64 = 20_000_000_000;

/// The job whose cache the fixture declares, and the prefix that follows from
/// it: `${{ runner.os }}` resolves to `Linux`, which is what the gate derives.
const JOB: &str = "build";
const PREFIX: &str = "Linux-fixture-build-";

/// A repository the gate can be pointed at, and a `gh` that answers for it.
///
/// `held` is what GitHub is holding, as `(key, bytes)`. An empty slice is a
/// repository storing nothing, which is a state this gate has to tell apart
/// from one it could not ask about.
fn tree(caches: bool, held: &[(&str, u64)]) -> tempfile::TempDir {
    let at = tempfile::tempdir().expect("a scratch directory");
    let root = at.path();
    std::fs::create_dir_all(root.join(".github/workflows")).expect("fixture workflows");
    std::fs::create_dir_all(root.join("stub")).expect("fixture stub directory");

    let mut workflow = String::from("name: a fixture, and not this repository's CI\njobs:\n");
    workflow.push_str(&format!(
        "  {JOB}:\n    runs-on: ubuntu-latest\n    steps:\n"
    ));
    if caches {
        workflow.push_str(
            "      - uses: actions/cache@v6\n        with:\n          path: |\n            \
             ~/.cargo/registry\n          key: ${{ runner.os }}-fixture-build-\
             ${{ hashFiles('**/Cargo.lock') }}\n",
        );
    }
    workflow.push_str("      - run: cargo test\n");
    std::fs::write(root.join(".github/workflows/ci.yml"), workflow).expect("write the workflow");

    // THE ROWS `held_caches` PARSES, and no more: size, created_at, key,
    // tab-separated, which is what its `--jq` asks GitHub for.
    let mut rows = String::new();
    for (key, bytes) in held {
        rows.push_str(&format!("{bytes}\t2026-08-01T00:00:00Z\t{key}\n"));
    }
    let stub = root.join("stub/gh");
    std::fs::write(
        &stub,
        format!("#!/usr/bin/env bash\ncat <<'ROWS'\n{rows}ROWS\n"),
    )
    .expect("write the gh stub");
    make_runnable(&stub);

    git(root, &["init", "--quiet"]);
    git(root, &["add", "-A"]);
    at
}

fn make_runnable(path: &Path) {
    use std::os::unix::fs::PermissionsExt as _;
    let mut mode = std::fs::metadata(path)
        .expect("stub metadata")
        .permissions();
    mode.set_mode(0o755);
    std::fs::set_permissions(path, mode).expect("make the stub runnable");
}

fn git(at: &Path, arguments: &[&str]) {
    let out = Command::new("git")
        .args(arguments)
        .current_dir(at)
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {arguments:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Run the gate over that tree, with the stub ahead of anything else on `PATH`.
fn gate(at: &Path) -> Output {
    let path = format!(
        "{}:{}",
        at.join("stub").display(),
        std::env::var("PATH").unwrap_or_default()
    );
    Command::new(env!("CARGO_BIN_EXE_cache-budget"))
        .arg(at)
        .current_dir(at)
        .env("PATH", path)
        // THE RUNNER'S OWN, removed rather than assumed absent: this suite runs
        // on a runner too, and every one of these sends the gate somewhere the
        // stub does not answer for.
        .env_remove("GITHUB_RUN_ID")
        .env_remove(ci_plan::WORKFLOW_VARIABLE)
        .env_remove(cache_budget::RANGE_VARIABLE)
        .output()
        .expect("the gate runs")
}

fn code(out: &Output) -> i32 {
    out.status
        .code()
        .expect("the gate exits rather than signals")
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// The sentence printed on exactly one of the three paths.
const CLEAN: &str = "every cache this repository declares is one it keeps";

/// A repository whose one declared cache fits: the law holds, and it says so.
#[test]
fn a_repository_whose_caches_fit_exits_zero_and_says_which_answer_that_is() {
    let at = tree(true, &[(&format!("{PREFIX}abc"), SMALL)]);
    let out = gate(at.path());
    assert_eq!(
        code(&out),
        0,
        "stdout:\n{}\nstderr:\n{}",
        stdout(&out),
        stderr(&out)
    );
    assert!(
        stdout(&out).contains(CLEAN),
        "the clean answer is said out loud rather than left as an exit code\n{}",
        stdout(&out)
    );
    // AND WHAT IT READ, which is printed FIRST and unconditionally for the
    // reason the library's own comment gives: a gate that never opened anything
    // and a gate that found nothing wrong print the same silence otherwise. On
    // a clean run — the kind nobody reads closely — this is the whole evidence.
    assert!(
        stdout(&out).contains(PREFIX),
        "the report names the cache it judged\n{}",
        stdout(&out)
    );
    // THE MIRROR: that sentence is printed on ONE path, and the two failing
    // paths are what this case exists to be told apart from.
    assert!(
        !stderr(&out).contains("cache-budget:"),
        "a repository it judged cleanly refuses nothing\n{}",
        stderr(&out)
    );
}

/// Caches that do not fit: a finding about the repository, and code `1`.
///
/// NOT `2`. This gate looked, and what it saw was over budget — GitHub will
/// delete these least-recently-used-first and every job restoring one rebuilds
/// from nothing. A hook told `2` here would read it as "the gate is broken".
#[test]
fn a_repository_over_its_budget_is_a_finding_and_not_an_unreachable_gate() {
    let at = tree(true, &[(&format!("{PREFIX}abc"), HUGE)]);
    let out = gate(at.path());
    assert_eq!(
        code(&out),
        1,
        "stdout:\n{}\nstderr:\n{}",
        stdout(&out),
        stderr(&out)
    );
    let said = stderr(&out);
    assert!(
        said.contains("against a 10.00 GB budget"),
        "and it says what the finding is\n{said}"
    );
    assert!(
        !stdout(&out).contains(CLEAN),
        "and the clean sentence belongs to the other answer\n{}",
        stdout(&out)
    );
}

/// A repository whose workflows declare no cache at all: NOT clean — unjudged.
///
/// THIS IS THE ANSWER THE INJECTION COLLAPSED. An empty answer here is the
/// reader failing rather than the repository being tidy, and it is reported as
/// `Unreached` for exactly that reason. The oracle is the exit code AND the
/// refusal in the words the type prints for itself, so a rewording moves the
/// assertion with it.
#[test]
fn a_repository_declaring_no_cache_is_unjudged_rather_than_clean() {
    let at = tree(false, &[]);
    let out = gate(at.path());
    assert_eq!(
        code(&out),
        2,
        "a gate that could not look must not answer 1 or 0\nstdout:\n{}\nstderr:\n{}",
        stdout(&out),
        stderr(&out)
    );
    assert!(
        stderr(&out).contains("this gate reached nothing it could judge"),
        "and it says it could not judge, in the words the refusal prints\n{}",
        stderr(&out)
    );
    assert!(
        !stdout(&out).contains(CLEAN),
        "a repository it did not read is not one it found clean\n{}",
        stdout(&out)
    );
}
