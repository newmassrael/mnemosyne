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
//! the one side and the fixture's workflow supplies the other. The stub answers
//! in GITHUB'S OWN SHAPE, spelled off the recording in `tests/github.rs`, which
//! is what R1130 bought: while the answer was flattened by a `--jq` expression
//! before this program saw it, a stub could only print rows that expression had
//! already produced, and the two failures below — an answer that stops early,
//! and one that never arrives — could not be posed here at all. The discipline
//! is `crates/mnemosyne-cli/tests/git_hooks_smoke.rs`'s, on the pre-push hook.
//!
//! AND THE STUB RECORDS WHAT IT WAS ASKED. `--paginate` decides whether the
//! answer is the whole storage or its first page, and a stub that ignores its
//! arguments agrees with a gate that stopped asking for it.
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

/// Where the stub writes the words it was called with, one per line.
const ASKED: &str = "asked";

/// The spelling GitHub stamps a cache with, from the recorded answer in
/// `tests/actions-caches.one-page.json`.
///
/// TO THE FRACTION, and not the shorter form a hand-written fixture reaches for:
/// this gate trims a stamp to the second before comparing it to a run's start
/// time, and a fixture spelling it the short way would be testing that trim on
/// input the API never sends.
const CREATED: &str = "2026-08-01T00:00:00.000000000Z";

/// GitHub's answer for a repository holding exactly these caches.
fn page(held: &[(&str, u64)]) -> String {
    page_claiming(held.len() as u64, held)
}

/// The same answer with GitHub's own count said out loud, whatever the rows.
///
/// THE ONE READING WHOSE FAILURE IS QUIET. A body carrying fewer rows than it
/// counts is well-formed and describes a smaller repository, and smaller is the
/// direction that passes a budget.
fn page_claiming(counted: u64, held: &[(&str, u64)]) -> String {
    let rows: Vec<String> = held
        .iter()
        .map(|(key, bytes)| {
            format!(
                "{{\"id\":1,\"ref\":\"refs/heads/main\",\"key\":\"{key}\",\
                 \"last_accessed_at\":\"{CREATED}\",\"created_at\":\"{CREATED}\",\
                 \"size_in_bytes\":{bytes}}}"
            )
        })
        .collect();
    format!(
        "{{\"total_count\":{counted},\"actions_caches\":[{}]}}",
        rows.join(",")
    )
}

/// A repository the gate can be pointed at, and a `gh` that answers for it.
///
/// `answer` is what GitHub says, verbatim — a page built by [`page`], one that
/// stops early, or nothing at all. The three are different states and this gate
/// has to tell them apart.
fn tree(caches: bool, answer: &str) -> tempfile::TempDir {
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

    // WHAT IT WAS ASKED, then what it answers. The question is the library's
    // (`caches_query`) and the file below is how a test reads it back.
    let stub = root.join("stub/gh");
    std::fs::write(
        &stub,
        format!(
            "#!/usr/bin/env bash\n\
             printf '%s\\n' \"$@\" > \"$(dirname \"$0\")/{ASKED}\"\n\
             cat <<'ANSWER'\n{answer}\nANSWER\n"
        ),
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

/// The words the stub was called with, in the order it was handed them.
fn asked(at: &Path) -> Vec<String> {
    std::fs::read_to_string(at.join("stub").join(ASKED))
        .expect("the gate asked `gh` something")
        .lines()
        .map(str::to_string)
        .collect()
}

/// A repository whose one declared cache fits: the law holds, and it says so.
#[test]
fn a_repository_whose_caches_fit_exits_zero_and_says_which_answer_that_is() {
    let at = tree(true, &page(&[(&format!("{PREFIX}abc"), SMALL)]));
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
    let at = tree(true, &page(&[(&format!("{PREFIX}abc"), HUGE)]));
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
    let at = tree(false, &page(&[]));
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

/// An answer that stops early is `2`, and not a repository under its budget.
///
/// THE FAILURE THAT ARRIVES AS GOOD NEWS. Everything else that can go wrong with
/// this read is loud, but a `gh` that stopped paginating prints a well-formed
/// page describing a SMALLER repository — and smaller passes a budget. Here the
/// answer says the storage holds two caches and carries the one that fits, while
/// the one it leaves out is over the limit on its own: a gate that believed it
/// would print the clean sentence and exit `0`.
#[test]
fn an_answer_that_stops_early_is_unjudged_rather_than_a_repository_that_fits() {
    let at = tree(true, &page_claiming(2, &[(&format!("{PREFIX}abc"), SMALL)]));
    let out = gate(at.path());
    assert_eq!(
        code(&out),
        2,
        "a partial read is not a verdict\nstdout:\n{}\nstderr:\n{}",
        stdout(&out),
        stderr(&out)
    );
    assert!(
        stderr(&out).contains("2 caches and 1 arrived"),
        "and it says what it was told and what arrived\n{}",
        stderr(&out)
    );
    // THE MIRROR, and the whole point of the case: the verdict it would have
    // reached on those rows is the clean one.
    assert!(
        !stdout(&out).contains(CLEAN),
        "an answer it could not trust is not one it found clean\n{}",
        stdout(&out)
    );
}

/// A `gh` that prints nothing is `2` — not a repository holding no caches.
///
/// Under the projection R1130 replaced these were ONE answer: an empty stdout
/// was read as a storage of zero bytes, so a `gh` that failed quietly, or a
/// filter that matched nothing, became a repository comfortably inside its
/// budget.
#[test]
fn an_answer_that_never_arrives_is_unjudged_rather_than_an_empty_repository() {
    let at = tree(true, "");
    let out = gate(at.path());
    assert_eq!(
        code(&out),
        2,
        "silence is not a reading\nstdout:\n{}\nstderr:\n{}",
        stdout(&out),
        stderr(&out)
    );
    assert!(
        stderr(&out).contains("printed nothing"),
        "and it says which of the two silences this is\n{}",
        stderr(&out)
    );
    assert!(
        !stdout(&out).contains(CLEAN),
        "and it is not a clean verdict\n{}",
        stdout(&out)
    );
}

/// The question this gate asks GitHub is the one the library writes down.
///
/// `--paginate` IS THE FLAG THAT DECIDES WHETHER THE ANSWER IS THE WHOLE
/// STORAGE, and it is invisible in every other test here: a stub that ignores
/// its arguments answers a gate that stopped asking for it exactly as it answers
/// one that did not. This repository holds more caches than a page carries, so
/// dropping it in production reads eleven caches as three.
#[test]
fn the_gate_asks_github_for_every_page_of_its_cache_storage() {
    let at = tree(true, &page(&[(&format!("{PREFIX}abc"), SMALL)]));
    let out = gate(at.path());
    assert_eq!(code(&out), 0, "stderr:\n{}", stderr(&out));
    let words = asked(at.path());
    // SPELLED OUT HERE, and not read back off `caches_query`. An oracle that
    // compared the question to the constant it came from would agree with every
    // edit to that constant, including the one that drops the flag.
    assert!(
        words.contains(&"--paginate".to_string()),
        "the answer is asked for in full: {words:?}"
    );
    assert!(
        words.contains(&"repos/{owner}/{repo}/actions/caches".to_string()),
        "of the cache endpoint, with `gh`'s own placeholders so this gate never \
         names the repository it judges: {words:?}"
    );
    // AND THE BINARY ASKS WHAT THE LIBRARY WRITES DOWN — that half IS the
    // comparison to the constant, and it is what keeps a second spelling from
    // growing in `main.rs`.
    assert_eq!(
        words,
        cache_budget::caches_query(),
        "the words handed to `gh` are the library's, verbatim"
    );
}
