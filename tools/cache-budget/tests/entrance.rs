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

/// One real run of this repository's CI, as GitHub answered for it.
///
/// A RECORDING, for the reason `tests/github.rs` gives at length: what this gate
/// does with a run's start time is only worth as much as the spelling that start
/// time really arrives in, and this endpoint answers to the SECOND while the
/// cache endpoint answers to the fraction.
const RUN_ANSWER: &str = include_str!("actions-run.json");

/// That run's id, and when GitHub says it began — both read off the recording.
const RUN_ID: &str = "31376754536";
const RUN_STARTED: &str = "2026-08-10T09:54:23Z";

/// How a runner names the workflow it is executing, pointed at the fixture's.
const WORKFLOW_REF: &str = "owner/fixture/.github/workflows/ci.yml@refs/heads/main";

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
    page_created(
        counted,
        &held
            .iter()
            .map(|(key, bytes)| (*key, *bytes, CREATED))
            .collect::<Vec<_>>(),
    )
}

/// The same, with each archive's own creation stamp.
///
/// WHEN AN ARCHIVE WAS CREATED IS WHAT SAYS WHETHER A JOB REBUILT: one stamped
/// inside the run is a key that missed and was saved again, and one stamped before
/// it is the generation that was there to be restored. Both are needed to pose the
/// refusal that turned this repository's `main` red.
fn page_created(counted: u64, held: &[(&str, u64, &str)]) -> String {
    let rows: Vec<String> = held
        .iter()
        .map(|(key, bytes, created)| {
            format!(
                "{{\"id\":1,\"ref\":\"refs/heads/main\",\"key\":\"{key}\",\
                 \"last_accessed_at\":\"{created}\",\"created_at\":\"{created}\",\
                 \"size_in_bytes\":{bytes}}}"
            )
        })
        .collect();
    format!(
        "{{\"total_count\":{counted},\"actions_caches\":[{}]}}",
        rows.join(",")
    )
}

/// What the workflow-runs endpoint answers about the fixture's own workflow.
///
/// THE INTERVAL A KEY IS JUDGED OVER COMES FROM HERE. A key is asked for when the
/// workflow declaring it runs, so the last such run is what bounds the interval a
/// missed key can be explained over — and the commit that run was of has to be a
/// commit of the fixture's OWN history, which is why this is resolved after the
/// history exists rather than written as a constant.
enum Runs {
    /// A page carrying no run at all — a workflow nothing has run yet, and the
    /// state in which this gate narrows to the push range and says so.
    NoneYet,
    /// The workflow last ran successfully at `HEAD~n` of the fixture's history.
    LastRanAtHeadMinus(usize),
}

/// When the fixture's last run of its own workflow started — before the run being
/// judged, which is what makes it an earlier observation.
const LAST_RAN_AT: &str = "2026-08-09T00:00:00Z";

/// GitHub's answer about a workflow's runs, in the shape `tests/github.rs`
/// records from the real endpoint.
fn runs_page(runs: &[(&str, &str, &str)]) -> String {
    let rows: Vec<String> = runs
        .iter()
        .map(|(sha, conclusion, started)| {
            format!(
                "{{\"id\":7,\"head_branch\":\"main\",\"head_sha\":\"{sha}\",\
                 \"status\":\"completed\",\"conclusion\":\"{conclusion}\",\
                 \"run_started_at\":\"{started}\"}}"
            )
        })
        .collect();
    format!(
        "{{\"total_count\":{},\"workflow_runs\":[{}]}}",
        runs.len(),
        rows.join(",")
    )
}

/// A repository the gate can be pointed at, and a `gh` that answers for it.
///
/// ONE CONSTRUCTOR, and every case says which fields it cares about. The
/// two-argument helper this replaced could not name the run answer or the depth
/// of the history, so the cases that need those would have grown a second one
/// beside it — which is how a fixture ends up with three ways to be built and
/// no way to see what varies between two cases.
///
/// `caches` and `run` are what GitHub says, VERBATIM: a page built by [`page`],
/// one that stops early, or nothing at all. Those are different states and this
/// gate has to tell them apart, so none of them is expressed as a flag.
///
/// TWO ANSWERS, BECAUSE THE BINARY MAKES TWO CALLS. One answer for both
/// endpoints is a stub that agrees with a gate asking the wrong one, which is
/// the same class of hole as a stub that ignores its arguments.
struct Fixture {
    /// Does the workflow declare a cache at all?
    declares: bool,
    /// What the cache endpoint answers.
    caches: String,
    /// What the runs endpoint answers — asked for only inside a run.
    run: String,
    /// What the workflow-runs endpoint answers — asked for only inside a run.
    runs: Runs,
    /// How many commits the checkout holds.
    ///
    /// `HEAD~1` is what this gate diffs from when the runner names no push
    /// range, so ONE commit is a checkout too shallow to answer with — a real
    /// state (`fetch-depth: 1`) that the gate refuses rather than reads as
    /// "nothing changed".
    commits: usize,
    /// Which commits touch the `Cargo.lock` the fixture's cache key hashes,
    /// counted from the first.
    ///
    /// WHERE IN THE HISTORY A DEPENDENCY MOVED is the whole of what separates a
    /// legitimate cache miss from a defect, and which interval you ask over
    /// decides whether you can see it. The default is a history where nothing the
    /// key hashes ever moves, so every case that is not about this says nothing
    /// about it.
    lockfile_moves_at: &'static [usize],
}

impl Default for Fixture {
    fn default() -> Self {
        Fixture {
            declares: true,
            caches: page(&[]),
            run: RUN_ANSWER.to_string(),
            runs: Runs::NoneYet,
            commits: 2,
            lockfile_moves_at: &[],
        }
    }
}

impl Fixture {
    fn build(&self) -> tempfile::TempDir {
        let caches = self.declares;
        let answer = self.caches.as_str();
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
        std::fs::write(root.join(".github/workflows/ci.yml"), workflow)
            .expect("write the workflow");

        git(root, &["init", "--quiet"]);
        git(root, &["add", "-A"]);
        // A HISTORY, BECAUSE THE RUN WINDOW DIFFS ONE. Which keys this run
        // legitimately invalidated is git's answer to the globs the keys name, and
        // the interval it is asked over is `HEAD~1..HEAD` unless the runner said
        // otherwise or the declaring workflow's own last run is known. The identity
        // is passed rather than inherited: a runner has none configured, and a
        // fixture that borrowed this machine's would be green here and red there.
        for step in 0..self.commits {
            std::fs::write(root.join(format!("commit-{step}.txt")), "a change\n")
                .expect("something to commit");
            // AND A DEPENDENCY THAT MOVES WHERE THIS FIXTURE SAYS IT DOES. The key
            // hashes `**/Cargo.lock`, so this is the one file whose movement the
            // gate can excuse a rebuilt cache for.
            if self.lockfile_moves_at.contains(&step) {
                std::fs::write(
                    root.join("Cargo.lock"),
                    format!("# a lockfile as it stood at commit {step}\n"),
                )
                .expect("write the lockfile");
            }
            git(root, &["add", "-A"]);
            git(
                root,
                &[
                    "-c",
                    "user.email=fixture@example.invalid",
                    "-c",
                    "user.name=fixture",
                    "commit",
                    "--quiet",
                    "-m",
                    "a commit this fixture can be diffed from",
                ],
            );
        }

        // WHAT IT WAS ASKED, then what it answers — APPENDED, one block per call,
        // because the binary asks three times inside a run and a record that
        // overwrote itself would show only whichever came last.
        //
        // AND IT DISPATCHES ON THE ENDPOINT. The three answers have nothing in
        // common but their transport; a stub handing the cache page to a question
        // about a run would agree with a gate that asked for the wrong thing.
        //
        // WRITTEN AFTER THE HISTORY, because one of the answers names a commit OF
        // that history: which run last observed a key is only meaningful as a
        // commit this checkout can be diffed from, and a sha invented here would
        // model a checkout too shallow to hold it — a different case, which has its
        // own test.
        let run = self.run.as_str();
        let runs = match self.runs {
            Runs::NoneYet => runs_page(&[]),
            Runs::LastRanAtHeadMinus(back) => runs_page(&[(
                &git_says(root, &["rev-parse", &format!("HEAD~{back}")]),
                "success",
                LAST_RAN_AT,
            )]),
        };
        let stub = root.join("stub/gh");
        std::fs::write(
            &stub,
            format!(
                "#!/usr/bin/env bash\n\
             {{ printf '%s\\n' \"$@\"; echo; }} >> \"$(dirname \"$0\")/{ASKED}\"\n\
             case \"$*\" in\n\
             \x20 *\"/actions/workflows/\"*)\n\
             \x20   cat <<'RUNS_ANSWER'\n{runs}\nRUNS_ANSWER\n\
             \x20   ;;\n\
             \x20 *\"/actions/runs/\"*)\n\
             \x20   cat <<'RUN_ANSWER'\n{run}\nRUN_ANSWER\n\
             \x20   ;;\n\
             \x20 *)\n\
             \x20   cat <<'CACHE_ANSWER'\n{answer}\nCACHE_ANSWER\n\
             \x20   ;;\n\
             esac\n"
            ),
        )
        .expect("write the gh stub");
        make_runnable(&stub);
        at
    }
}

fn make_runnable(path: &Path) {
    use std::os::unix::fs::PermissionsExt as _;
    let mut mode = std::fs::metadata(path)
        .expect("stub metadata")
        .permissions();
    mode.set_mode(0o755);
    std::fs::set_permissions(path, mode).expect("make the stub runnable");
}

/// The same, with git's answer handed back — for the commits a fixture's history
/// only has once it exists.
fn git_says(at: &Path, arguments: &[&str]) -> String {
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
    String::from_utf8_lossy(&out.stdout).trim().to_string()
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

/// Run the gate over that tree from OUTSIDE a run, the developer's case.
fn gate(at: &Path) -> Output {
    run_gate(at, None)
}

/// Run it as a runner does: inside the recorded run, of the fixture's workflow.
///
/// THIS IS THE HALF NOTHING RAN. Clearing `GITHUB_RUN_ID` is what makes a
/// fixture deterministic and it is ALSO what skips the whole run window — when
/// this run began, which keys it invalidated, and which workflow it is of. Those
/// laws have fixtures in `tests/law.rs`; what had no reader was their wiring
/// into the binary, and R1129 named it in its own carry.
fn gate_in_run(at: &Path) -> Output {
    run_gate(at, Some(RUN_ID))
}

fn run_gate(at: &Path, run: Option<&str>) -> Output {
    let path = format!(
        "{}:{}",
        at.join("stub").display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let mut command = Command::new(env!("CARGO_BIN_EXE_cache-budget"));
    command
        .arg(at)
        .current_dir(at)
        .env("PATH", path)
        // NAMED RATHER THAN INHERITED, in both directions: this suite runs on a
        // runner too, where every one of these is already set to that run's
        // values and would send the gate somewhere the stub does not answer for.
        .env_remove(cache_budget::RANGE_VARIABLE);
    match run {
        Some(id) => command
            .env("GITHUB_RUN_ID", id)
            .env(ci_plan::WORKFLOW_VARIABLE, WORKFLOW_REF),
        None => command
            .env_remove("GITHUB_RUN_ID")
            .env_remove(ci_plan::WORKFLOW_VARIABLE),
    };
    command.output().expect("the gate runs")
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

/// Every call the stub took, in order, each as the words it was handed.
///
/// ONE BLOCK PER CALL, split on the blank line the stub writes between them: the
/// binary asks once outside a run and three times inside one — the cache storage,
/// the run it is in, and the runs of each workflow declaring a key — and which
/// question got which answer is the thing these cases are here to pin.
fn asked(at: &Path) -> Vec<Vec<String>> {
    std::fs::read_to_string(at.join("stub").join(ASKED))
        .expect("the gate asked `gh` something")
        .split("\n\n")
        .map(|call| call.lines().map(str::to_string).collect::<Vec<_>>())
        .filter(|call: &Vec<String>| !call.is_empty())
        .collect()
}

/// A repository whose one declared cache fits: the law holds, and it says so.
#[test]
fn a_repository_whose_caches_fit_exits_zero_and_says_which_answer_that_is() {
    let at = Fixture {
        caches: page(&[(&format!("{PREFIX}abc"), SMALL)]),
        ..Fixture::default()
    }
    .build();
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
    let at = Fixture {
        caches: page(&[(&format!("{PREFIX}abc"), HUGE)]),
        ..Fixture::default()
    }
    .build();
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
    let at = Fixture {
        declares: false,
        ..Fixture::default()
    }
    .build();
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
    let at = Fixture {
        caches: page_claiming(2, &[(&format!("{PREFIX}abc"), SMALL)]),
        ..Fixture::default()
    }
    .build();
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
    let at = Fixture {
        caches: String::new(),
        ..Fixture::default()
    }
    .build();
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
    let at = Fixture {
        caches: page(&[(&format!("{PREFIX}abc"), SMALL)]),
        ..Fixture::default()
    }
    .build();
    let out = gate(at.path());
    assert_eq!(code(&out), 0, "stderr:\n{}", stderr(&out));
    let calls = asked(at.path());
    assert_eq!(
        calls.len(),
        1,
        "outside a run there is one question: {calls:?}"
    );
    let words = calls[0].clone();
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

/// INSIDE A RUN, the gate reads the run window and says which run it read.
///
/// THE SECOND HALF OF THIS GATE, and until now nothing ran it. Everything above
/// clears `GITHUB_RUN_ID`, which is what makes a fixture deterministic and is
/// also what skips the run window entirely — when the run began, which keys its
/// commits invalidated, and which workflow it belongs to. The oracle is the
/// report naming that run and its start time, plus the mirror: the sentence the
/// no-run path prints instead.
#[test]
fn inside_a_run_the_gate_reads_the_window_and_names_the_run_it_read() {
    let at = Fixture {
        caches: page(&[(&format!("{PREFIX}abc"), SMALL)]),
        ..Fixture::default()
    }
    .build();
    let out = gate_in_run(at.path());
    assert_eq!(
        code(&out),
        0,
        "stdout:\n{}\nstderr:\n{}",
        stdout(&out),
        stderr(&out)
    );
    assert!(
        stdout(&out).contains(RUN_STARTED),
        "the report says when the run it judged against began\n{}",
        stdout(&out)
    );
    assert!(
        stdout(&out).contains(".github/workflows/ci.yml"),
        "and which workflow that run is of\n{}",
        stdout(&out)
    );
    // THE MIRROR: the sentence printed on the other path, which is what a gate
    // that silently skipped the window would still be saying.
    assert!(
        !stdout(&out).contains("NOT INSIDE A RUN"),
        "a run it read is not a run it skipped\n{}",
        stdout(&out)
    );
    // AND THE QUESTION IT ASKED ABOUT THAT RUN NAMES THE RUN. `runs/{id}` and
    // `runs` are different resources, and a start time belonging to some other
    // run excuses every cache built after it.
    let calls = asked(at.path());
    assert_eq!(
        calls.len(),
        3,
        "inside a run it asks three times: {calls:?}"
    );
    assert_eq!(calls[1], cache_budget::run_query(RUN_ID));
    assert!(
        calls[1].iter().any(|word| word.contains(RUN_ID)),
        "and the run id is in the question: {:?}",
        calls[1]
    );
    // AND THE THIRD QUESTION IS ABOUT THE WORKFLOW THAT DECLARES THE KEY, which is
    // what bounds the interval that key can be judged over. A gate that stopped
    // asking it would judge every key over the commits one push carried — the
    // reading that turned this repository's `main` red on run 31695396997.
    assert_eq!(
        calls[2],
        cache_budget::workflow_runs_query(".github/workflows/ci.yml", None),
        "the words handed to `gh` are the library's, verbatim"
    );
    assert!(
        calls[2]
            .iter()
            .any(|word| word.contains("workflows/ci.yml/runs")),
        "and it names the workflow whose history is wanted: {:?}",
        calls[2]
    );
}

/// The archive stamped inside the run being judged, and the generation that was
/// there to be restored.
///
/// BOTH ARE NEEDED TO POSE THE REFUSAL. A key saved during the run is one whose
/// primary key missed; without an older generation the miss was unavoidable, and
/// the gate says so by not refusing at all.
fn a_key_rebuilt_during_the_run() -> String {
    page_created(
        2,
        &[
            (
                &format!("{PREFIX}now"),
                SMALL,
                "2026-08-10T10:14:11.221132000Z",
            ),
            (&format!("{PREFIX}was"), SMALL, CREATED),
        ],
    )
}

/// A cache rebuilt after its lockfile moved is not a defect — and the interval
/// that shows it is the declaring workflow's own.
///
/// THIS IS RUN 31695396997, WITH REAL GIT AND THE WHOLE BINARY. The history here
/// is the shape that reddened `main`: the lockfile the key hashes moved at a
/// commit that the LAST PUSH did not carry, and the workflow declaring that key —
/// path-filtered, so it does not run on every push — had not run since before it.
/// Asked over its own history the miss is explained; asked over the push alone it
/// is a finding about a repository doing nothing wrong.
///
/// THE CONTROL IS THE SECOND HALF OF THIS CASE, and it is the behaviour that
/// shipped: the same tree, the same storage, the same git, differing only in
/// whether the declaring workflow's last run could be found.
#[test]
fn a_cache_rebuilt_after_its_lockfile_moved_outside_this_push_is_not_a_defect() {
    let at = Fixture {
        caches: a_key_rebuilt_during_the_run(),
        // Three commits: the lockfile moves in the middle one, so `HEAD~1..HEAD` —
        // the interval a push with no range variable is judged over — contains no
        // movement of it at all.
        commits: 3,
        lockfile_moves_at: &[1],
        runs: Runs::LastRanAtHeadMinus(2),
        ..Fixture::default()
    }
    .build();
    let out = gate_in_run(at.path());
    assert_eq!(
        code(&out),
        0,
        "the lockfile moved since that workflow last ran, so the rebuild is the \
         price of a dependency change\nstdout:\n{}\nstderr:\n{}",
        stdout(&out),
        stderr(&out)
    );
    assert!(
        stdout(&out).contains("1 of 1 key(s) had their hashed inputs moved"),
        "and the report says so\n{}",
        stdout(&out)
    );
    assert!(
        stdout(&out).contains("since .github/workflows/ci.yml last ran successfully"),
        "naming the interval it answered over, and whose history that is\n{}",
        stdout(&out)
    );

    // THE CONTROL. With no run of that workflow to be found, the gate narrows to
    // the push — and the same repository is refused for a rebuild it had every
    // right to. Everything else about the two fixtures is identical.
    let at = Fixture {
        caches: a_key_rebuilt_during_the_run(),
        commits: 3,
        lockfile_moves_at: &[1],
        runs: Runs::NoneYet,
        ..Fixture::default()
    }
    .build();
    let out = gate_in_run(at.path());
    assert_eq!(
        code(&out),
        1,
        "over the push alone nothing the key hashes moved\nstdout:\n{}\nstderr:\n{}",
        stdout(&out),
        stderr(&out)
    );
    assert!(
        stderr(&out).contains("SAVED BY THIS RUN"),
        "which is the refusal that turned main red\n{}",
        stderr(&out)
    );
    assert!(
        stdout(&out).contains("no successful run of `.github/workflows/ci.yml`"),
        "and the report says the interval was narrowed and why, rather than \
         printing a number from a question nobody can see\n{}",
        stdout(&out)
    );
}

/// A run GitHub will not say the start time of is `2`, not a run at time zero.
///
/// EVERY CACHE IS NEWER THAN NOTHING, so a blank start time read as a zero makes
/// this gate report every job in the repository as having rebuilt from scratch —
/// a page of findings about jobs that were warm.
#[test]
fn a_run_whose_start_time_is_missing_is_unjudged_rather_than_a_run_at_time_zero() {
    let at = Fixture {
        caches: page(&[(&format!("{PREFIX}abc"), SMALL)]),
        run: RUN_ANSWER.replace(RUN_STARTED, ""),
        ..Fixture::default()
    }
    .build();
    let out = gate_in_run(at.path());
    assert_eq!(
        code(&out),
        2,
        "stdout:\n{}\nstderr:\n{}",
        stdout(&out),
        stderr(&out)
    );
    assert!(
        stderr(&out).contains(RUN_ID) && stderr(&out).contains("no start time"),
        "and it says which run answered that way\n{}",
        stderr(&out)
    );
    assert!(
        !stdout(&out).contains(CLEAN),
        "a window it could not read is not a clean verdict\n{}",
        stdout(&out)
    );
}

/// A checkout too shallow to hold the range is `2`, and not "nothing changed".
///
/// `fetch-depth: 1` IS A REAL RUNNER STATE, and the difference matters: which
/// keys this push legitimately invalidated is what excuses a cache the run
/// rebuilt, so reading an unanswerable diff as an empty one refuses every key
/// the commit had every right to move.
#[test]
fn a_checkout_with_no_parent_commit_is_unjudged_rather_than_one_that_changed_nothing() {
    let at = Fixture {
        caches: page(&[(&format!("{PREFIX}abc"), SMALL)]),
        commits: 1,
        ..Fixture::default()
    }
    .build();
    let out = gate_in_run(at.path());
    assert_eq!(
        code(&out),
        2,
        "stdout:\n{}\nstderr:\n{}",
        stdout(&out),
        stderr(&out)
    );
    assert!(
        stderr(&out).contains("depth 1"),
        "and it names the shallow checkout as the reason\n{}",
        stderr(&out)
    );
}
