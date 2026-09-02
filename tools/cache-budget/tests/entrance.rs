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
//! RECORDING AN ARGUMENT IS NOT THE SAME AS ANSWERING IT, WHICH R1312 PAID FOR.
//! The stub recorded `per_page` and then handed back whatever page the fixture
//! had written, so every case here received the whole answer however small a
//! page the gate asked for — and the depth of that page is what decides which
//! runs a key's interval can be bounded by. It stood at five while this
//! repository's cadence put the run that mattered fifteen rows down, and
//! nothing in this file could go red for it. The stub honours it now, on the
//! run list.
//!
//! AND THE RUNNER'S OWN VARIABLES ARE NAMED RATHER THAN ASSUMED ABSENT. This
//! suite runs on a runner too, where `GITHUB_RUN_ID` is set and would send the
//! gate down a second reading it has no stub for — green here and red there,
//! which is the shape R1119 paid for.
//!
//! THAT RULE WAS RIGHT AND ITS LIST WENT STALE, which is what R1181 pays for.
//! R1178 taught the gate to bound a key's interval by the workflow that declares
//! it, and bounding it needs the BRANCH — so `main.rs` began reading
//! `GITHUB_BASE_REF` and `GITHUB_REF_NAME`, two variables this fixture had never
//! heard of. A developer's shell sets neither, so the suite stayed green here
//! and turned `main` red on run 31703011291, asking GitHub a branch-scoped
//! question the assertion did not expect. The list is now [`environment`], one
//! value both the spawn and `the_fixture_names_every_variable_the_gate_reads`
//! read, and that law fails when the next variable arrives instead of a run
//! four commits later.

use std::collections::{BTreeMap, BTreeSet};
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

/// The branch the fixture's run is of.
///
/// A RUN IS ALWAYS OF A BRANCH, which is why leaving this to the environment was
/// never a neutral choice: it made the fixture describe a runner that does not
/// exist, and the gate's branch-scoped question then went to a stub that had
/// been set up for the unscoped one.
const BRANCH: &str = "main";

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
    /// The workflow last ran at `HEAD~n` of the fixture's history AND that run
    /// wrote the fixture's archive.
    LastWroteTheArchiveAtHeadMinus(usize),
    /// The same run, which did NOT write it — its cache job was skipped, or
    /// failed before its post step.
    ///
    /// A STATE THAT DID NOT EXIST BEFORE R1207, because the bound used to be the
    /// run's own conclusion and a run either counted or did not. It is the case
    /// this repository produces constantly: measured over the newest hundred
    /// runs, 122 archives were written inside runs that concluded failure, and
    /// two caches of one run routinely disagree.
    RanAtHeadMinusAndWroteNothing(usize),
    /// A STREAK OF RUNS THAT WROTE NOTHING, and behind them the one that wrote
    /// the archive — this repository's own shape, and the state R1312 paid for.
    ///
    /// Its concurrency group cancels the run in flight when the next push
    /// arrives, and a cancelled run usually does not reach its post steps — the
    /// run that DID write here is one GitHub calls `cancelled` too, so the
    /// conclusion is not what says whether an archive was written. Measured
    /// 2026-09-02: FOURTEEN consecutive runs wrote no archive at all, and the
    /// run that last wrote `Linux-cargo-validate-` was FIFTEEN back. Whether
    /// this gate can see that far is decided by how deep a page it asks for, and
    /// that number had been five since R1207 with nothing able to go red for it.
    WroteTheArchiveBehindAStreakOf {
        /// How many newer runs reached no post step.
        silent: usize,
        /// How far back in the fixture's own history the writer ran.
        wrote_at_head_minus: usize,
    },
    /// EVERY RUN SILENT, some of them older than the archive being bounded
    /// against — the shape that says where the walk STOPS.
    ///
    /// The run that WROTE an archive started before it, so a candidate older
    /// than that archive cannot be the one being looked for. Both halves are
    /// needed to see it: without the older ones there is nothing the walk could
    /// wastefully ask about, and without a writer missing from the page the walk
    /// would return before reaching them. The verdict is the same either way —
    /// what differs is only how many calls it cost, which is why this shape's
    /// oracle is what `gh` was asked.
    SilentBeforeAndAfterTheArchive {
        /// Runs stamped AFTER the archive was written — all of them have to be
        /// asked about.
        newer: usize,
        /// Runs stamped BEFORE it — the first is the last one worth asking.
        older: usize,
    },
}

/// When the archive [`Runs::SilentBeforeAndAfterTheArchive`] bounds against was
/// written — between the two halves of that fixture's run history.
const ARCHIVE_WRITTEN: &str = "2026-08-05T00:00:00.000000000Z";

/// When the fixture's last run of its own workflow started — before the run being
/// judged, which is what makes it an earlier observation.
const LAST_RAN_AT: &str = "2026-08-09T00:00:00Z";

/// GitHub's answer about a workflow's runs, in the shape `tests/github.rs`
/// records from the real endpoint.
fn runs_page(runs: &[(&str, &str, &str)]) -> String {
    let rows: Vec<String> = runs
        .iter()
        .enumerate()
        .map(|(row, (sha, conclusion, started))| {
            // AN ID PER ROW, so a case can give one run's jobs a different
            // answer from another's. One id for the whole page was enough while
            // every fixture held a single run, and it made the streak below
            // unwritable: every candidate would have reported the same save.
            let id = 7 + row;
            format!(
                "{{\"id\":{id},\"head_branch\":\"main\",\"head_sha\":\"{sha}\",\
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

/// The cache step this fixture's workflow declares, and therefore the `Post …`
/// step GitHub would report for it.
const FIXTURE_STEP: &str = "Cache cargo (fixture)";

/// GitHub's answer about a run's JOBS, in the shape `tests/actions-run-jobs.json`
/// records from the real endpoint.
///
/// THE ANSWER THAT DECIDES A BOUND SINCE R1207. What writes an archive is the
/// post step of the job holding the cache, so `wrote` is the whole difference
/// between a run that bounds an interval and one that does not — and the two
/// pages differ in exactly one word, which is what makes the case below a
/// control rather than two unrelated fixtures.
fn jobs_page(wrote: bool) -> String {
    let conclusion = if wrote { "success" } else { "skipped" };
    format!(
        "{{\"total_count\":1,\"jobs\":[{{\"name\":\"the fixture's only job\",\
         \"status\":\"completed\",\"steps\":[\
         {{\"name\":\"{FIXTURE_STEP}\",\"status\":\"completed\",\"conclusion\":\"success\"}},\
         {{\"name\":\"Post {FIXTURE_STEP}\",\"status\":\"completed\",\
         \"conclusion\":\"{conclusion}\"}}]}}]}}"
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
            // NAMED, because the save GitHub reports is `Post <this name>` and
            // that name is the only join between a declaration and the run that
            // wrote its archive (R1207). `ci-plan` refuses an unnamed cache
            // step, so a fixture without one is a tree this gate correctly will
            // not read — which is how this line came to be written.
            workflow.push_str(
                "      - name: Cache cargo (fixture)\n        uses: actions/cache@v6\n        \
             with:\n          path: |\n            \
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
        let at_head_minus = |back: usize| {
            runs_page(&[(
                &git_says(root, &["rev-parse", &format!("HEAD~{back}")]),
                "success",
                LAST_RAN_AT,
            )])
        };
        // AND, WHERE THE CASE NEEDS THEM, ONE JOBS ANSWER PER RUN. The ids are
        // `runs_page`'s own — row 0 is 7 — so a case that says "these wrote
        // nothing and that one wrote the archive" says it about the same rows
        // GitHub's answer carries.
        let mut per_run: Vec<(u64, String)> = Vec::new();
        let (runs, jobs) = match self.runs {
            Runs::NoneYet => (runs_page(&[]), jobs_page(true)),
            Runs::LastWroteTheArchiveAtHeadMinus(back) => (at_head_minus(back), jobs_page(true)),
            Runs::RanAtHeadMinusAndWroteNothing(back) => (at_head_minus(back), jobs_page(false)),
            Runs::WroteTheArchiveBehindAStreakOf {
                silent,
                wrote_at_head_minus,
            } => {
                let cancelled = git_says(root, &["rev-parse", "HEAD~1"]);
                let writer = git_says(root, &["rev-parse", &format!("HEAD~{wrote_at_head_minus}")]);
                let mut rows: Vec<(String, String, String)> = (0..silent)
                    .map(|step| {
                        (
                            cancelled.clone(),
                            "cancelled".to_string(),
                            // NEWEST FIRST AND ALL OF THEM AFTER THE WRITER, so
                            // the walk has to go past every one of them to reach
                            // the run that bounds the interval.
                            format!("2026-08-09T{:02}:00:00Z", 12 - step),
                        )
                    })
                    .collect();
                rows.push((writer, "success".to_string(), LAST_RAN_AT.to_string()));
                for (row, _) in rows.iter().enumerate() {
                    per_run.push((7 + row as u64, jobs_page(row == silent)));
                }
                let page = runs_page(
                    &rows
                        .iter()
                        .map(|(sha, conclusion, started)| {
                            (sha.as_str(), conclusion.as_str(), started.as_str())
                        })
                        .collect::<Vec<_>>(),
                );
                (page, jobs_page(false))
            }
            Runs::SilentBeforeAndAfterTheArchive { newer, older } => {
                let sha = git_says(root, &["rev-parse", "HEAD~1"]);
                let rows: Vec<(String, String, String)> = (0..newer)
                    .map(|step| format!("2026-08-09T{:02}:00:00Z", newer - step))
                    .chain((0..older).map(|step| format!("2026-08-04T{:02}:00:00Z", older - step)))
                    .map(|started| (sha.clone(), "cancelled".to_string(), started))
                    .collect();
                for (row, _) in rows.iter().enumerate() {
                    per_run.push((7 + row as u64, jobs_page(false)));
                }
                let page = runs_page(
                    &rows
                        .iter()
                        .map(|(sha, conclusion, started)| {
                            (sha.as_str(), conclusion.as_str(), started.as_str())
                        })
                        .collect::<Vec<_>>(),
                );
                (page, jobs_page(false))
            }
        };
        for (id, body) in &per_run {
            std::fs::write(root.join("stub").join(format!("jobs.{id}.json")), body)
                .expect("the stub's answer about one run's jobs");
        }
        // THE ANSWERS ARE DATA, and the program that hands them over is one cargo
        // built (R1192). This used to interpolate all four bodies into a shell
        // script and chmod it, which is a file this process writes and then runs
        // — the shape that fails with `ETXTBSY` whenever a sibling test forks
        // between the write and the exec. `src/bin/gh-stub.rs` dispatches on the
        // endpoint and reads these; nothing here is executable.
        for (name, body) in [
            ("runs.json", runs),
            ("jobs.json", jobs),
            ("run.json", run.to_owned()),
            ("caches.json", answer.to_owned()),
        ] {
            std::fs::write(root.join("stub").join(name), body).expect("the stub's answer");
        }
        link_gh(&root.join("stub/gh"));
        at
    }
}

/// Put the `gh` stub on this path.
///
/// A SYMLINK TO A BINARY CARGO BUILT, never a file this process writes. See
/// `src/bin/gh-stub.rs` for the mechanism and for why the answers moved out of
/// the program and into files beside it.
fn link_gh(at: &Path) {
    std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_gh-stub"), at).expect("link the gh stub");
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

/// EVERY environment variable the gate reads, and what this fixture says about
/// it — one value, so the spawn and the law that checks it cannot disagree.
///
/// `None` means REMOVED, which is a declaration too: a variable this suite does
/// not name is one the machine underneath gets to answer, and that machine is a
/// runner as often as it is a developer's shell.
fn environment(at: &Path, run: Option<&str>) -> Vec<(String, Option<String>)> {
    vec![
        // WHERE THE `gh` STUB ANSWERS OUT OF (R1192). It is named here rather
        // than worked out by the stub because a program reached through a
        // symlink cannot find the directory it was reached from: `argv[0]` is
        // the name `execvp` was given and `/proc/self/exe` is the cargo target.
        // It belongs in THIS list because the law below walks every source in
        // this crate, and the stub inherits the environment the gate is handed —
        // so a variable it reads is one this fixture must name for exactly the
        // reason the gate's own are.
        (
            "GH_STUB_DIR".to_string(),
            Some(at.join("stub").display().to_string()),
        ),
        // The range a caller may pin. Never inherited: a runner sets it.
        (cache_budget::RANGE_VARIABLE.to_string(), None),
        // Which run this is, and of which workflow — the pair that decides
        // whether the gate reads the run window at all.
        ("GITHUB_RUN_ID".to_string(), run.map(str::to_string)),
        (
            ci_plan::WORKFLOW_VARIABLE.to_string(),
            run.map(|_| WORKFLOW_REF.to_string()),
        ),
        // WHICH BRANCH, which is what bounds the interval a key is judged over
        // (R1178). A push run sets `GITHUB_REF_NAME` and leaves
        // `GITHUB_BASE_REF` empty; a pull-request run sets both, and the gate
        // prefers the base. This fixture is the push case, stated rather than
        // inherited — the inherited version is what turned `main` red.
        (
            "GITHUB_REF_NAME".to_string(),
            run.map(|_| BRANCH.to_string()),
        ),
        ("GITHUB_BASE_REF".to_string(), None),
        // WHICH CARGO, ABSENT ON PURPOSE (R1262). This gate links `ci-plan`, and
        // that crate's one door to a cargo command reads `CARGO` to pin the cargo
        // that built the process — so from this fixture's point of view the gate
        // and its stub now READ a variable neither of them uses, and R1211's law
        // is right to ask for it. Removed rather than set: nothing under this test
        // runs cargo, and a value here would be a claim about a program that is
        // never started.
        ("CARGO".to_string(), None),
    ]
}

fn run_gate(at: &Path, run: Option<&str>) -> Output {
    let path = format!(
        "{}:{}",
        at.join("stub").display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let mut command = Command::new(env!("CARGO_BIN_EXE_cache-budget"));
    command.arg(at).current_dir(at).env("PATH", path);
    for (name, value) in environment(at, run) {
        match value {
            Some(value) => command.env(name, value),
            None => command.env_remove(name),
        };
    }
    command.output().expect("the gate runs")
}

/// The fixture names every variable the gate reads — asked of `main.rs`'s
/// SYNTAX, not of anybody's memory of it.
///
/// This is the law the stale list needed. `environment` above is a hand-written
/// set, and a hand-written set of "what the program reads" is exactly the thing
/// that goes quietly out of date: R1178 added two reads and nothing said so,
/// because on a developer's machine an unnamed variable is absent anyway and the
/// suite agrees with the fixture by luck. Here the question is put to the source
/// the binary is compiled from, so the next read arrives with a failing test
/// attached rather than with a red `main`.
#[test]
fn the_fixture_names_every_variable_the_gate_reads() {
    // The NAMES are what this law is about, so the tree the values would point
    // at is irrelevant here and this crate's own directory serves — a real path
    // rather than an invented one, so nothing in it reads as a claim.
    let anywhere = Path::new(env!("CARGO_MANIFEST_DIR"));
    let named: BTreeSet<String> = [None, Some(RUN_ID)]
        .into_iter()
        .flat_map(|run| environment(anywhere, run))
        .map(|(name, _)| name)
        .collect();
    let read = variables_read(Path::new(env!("CARGO_MANIFEST_DIR")).join("src"));
    println!("  the gate reads {read:?}; the fixture names {named:?}");
    assert!(
        read.len() > 1,
        "a gate that reads one environment variable or none is a source this walk failed to \
         parse, not a program with no environment: {read:?}"
    );
    let unnamed: Vec<&String> = read.difference(&named).collect();
    assert!(
        unnamed.is_empty(),
        "the gate reads {unnamed:?}, which this fixture neither sets nor removes — so the machine \
         underneath answers, and a runner answers differently from a shell"
    );
}

/// The names of environment variables this gate reads that are spelled by a
/// constant BELONGING TO ANOTHER CRATE, which this walk cannot follow.
///
/// The value is taken from the constant itself rather than retyped, so a rename
/// upstream is a compile error here and not a silently smaller set. The KEY is
/// how the call site spells it; a read through a constant this table does not
/// hold fails the walk loudly rather than passing as nothing.
const IMPORTED_NAMES: [(&str, &str); 1] = [("WORKFLOW_VARIABLE", ci_plan::WORKFLOW_VARIABLE)];

/// Every environment variable a `std::env::var`-shaped call reads, over every
/// Rust file in a directory tree.
///
/// Read through `syn` rather than by matching text, the R1172 discipline: a
/// program's own grammar is the only reader that cannot be fooled by a name
/// inside a comment or a doc example.
///
/// A READ IS NOT ALWAYS A LITERAL, and that is the half a first draft of this
/// walk missed: two of the five variables in play are spelled by constants, and
/// a walk that collected string literals alone reported three reads and called
/// the list complete. Constants declared in these same files are resolved from
/// them; one that comes from another crate resolves through [`IMPORTED_NAMES`];
/// anything else stops the walk, because "I could not read this one" and "there
/// is nothing here" must not arrive as the same answer.
fn variables_read(root: std::path::PathBuf) -> BTreeSet<String> {
    #[derive(Default)]
    struct Reads {
        /// The argument of each `var` call, as written.
        arguments: Vec<syn::Expr>,
        /// `const NAME: _ = "literal"` declared in these files.
        constants: BTreeMap<String, String>,
    }
    impl<'ast> syn::visit::Visit<'ast> for Reads {
        fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
            if let syn::Expr::Path(path) = &*call.func {
                let called = path
                    .path
                    .segments
                    .last()
                    .map(|segment| segment.ident.to_string())
                    .unwrap_or_default();
                if called == "var" || called == "var_os" {
                    self.arguments.extend(call.args.iter().cloned());
                }
            }
            syn::visit::visit_expr_call(self, call);
        }

        fn visit_item_const(&mut self, item: &'ast syn::ItemConst) {
            if let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(text),
                ..
            }) = &*item.expr
            {
                self.constants.insert(item.ident.to_string(), text.value());
            }
            syn::visit::visit_item_const(self, item);
        }
    }

    let mut sources: Vec<std::path::PathBuf> = std::fs::read_dir(&root)
        .unwrap_or_else(|e| panic!("the gate's sources are at {}: {e}", root.display()))
        .map(|entry| entry.expect("a directory entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
        .collect();
    sources.sort();
    assert!(
        !sources.is_empty(),
        "no Rust source under {} — this walk found nothing to read",
        root.display()
    );

    let mut reads = Reads::default();
    for source in &sources {
        let text = std::fs::read_to_string(source).expect("a source file reads");
        let parsed = syn::parse_file(&text)
            .unwrap_or_else(|e| panic!("{} does not parse: {e}", source.display()));
        syn::visit::Visit::visit_file(&mut reads, &parsed);
    }

    let imported: BTreeMap<&str, &str> = IMPORTED_NAMES.into_iter().collect();
    reads
        .arguments
        .iter()
        .map(|argument| match argument {
            syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(text),
                ..
            }) => text.value(),
            syn::Expr::Path(path) => {
                let last = path
                    .path
                    .segments
                    .last()
                    .map(|segment| segment.ident.to_string())
                    .unwrap_or_default();
                reads
                    .constants
                    .get(&last)
                    .cloned()
                    .or_else(|| imported.get(last.as_str()).map(|value| value.to_string()))
                    .unwrap_or_else(|| {
                        panic!(
                            "the gate reads an environment variable named by `{last}`, and this \
                             walk can resolve neither a constant in its own sources nor an entry \
                             in IMPORTED_NAMES — resolve it there rather than letting an \
                             unreadable name count as no read"
                        )
                    })
            }
            other => panic!(
                "the gate names an environment variable with an expression this walk cannot \
                 resolve ({}); a name computed at runtime cannot be held against a fixture list",
                quote_of(other)
            ),
        })
        .collect()
}

/// A syn expression as source text, for a message that has to name what it
/// could not read.
fn quote_of(expr: &syn::Expr) -> String {
    use syn::spanned::Spanned;
    format!("{:?}", expr.span())
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
        cache_budget::workflow_runs_query(".github/workflows/ci.yml", Some(BRANCH)),
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
        runs: Runs::LastWroteTheArchiveAtHeadMinus(2),
        ..Fixture::default()
    }
    .build();
    let out = gate_in_run(at.path());
    assert_eq!(
        code(&out),
        0,
        "the lockfile moved since that archive was last written, so the rebuild is \
         the price of a dependency change\nstdout:\n{}\nstderr:\n{}",
        stdout(&out),
        stderr(&out)
    );
    assert!(
        stdout(&out).contains("1 of 1 key(s) had their hashed inputs moved"),
        "and the report says so\n{}",
        stdout(&out)
    );
    assert!(
        stdout(&out).contains(
            "since `Cache cargo (fixture)` in .github/workflows/ci.yml last wrote its archive"
        ),
        "naming the interval it answered over, and whose history that is — since R1207 \
         that is the CACHE's and not the workflow's\n{}",
        stdout(&out)
    );

    // THE CONTROL. With no run of that workflow to be found, the interval this
    // key's archive could have seen is not bounded — and R1312 is the round that
    // stopped substituting a narrower one for it. Everything else about the two
    // fixtures is identical.
    //
    // WHAT THIS ARM ASSERTED UNTIL THEN WAS THE FALSE RED ITSELF: exit `1`, a
    // `Recreated` refusal, the sentence `SAVED BY THIS RUN` — the gate telling a
    // repository that a rebuild the push range simply could not see was a
    // defect. The repair does not make the arm green; it moves it from the
    // verdict that blames the repository to the one that reports the gate's own
    // horizon, which is exit `2`.
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
        2,
        "with no run to bound the interval the gate could not look, which is not \
         the same answer as a repository that rebuilt a cache for no \
         reason\nstdout:\n{}\nstderr:\n{}",
        stdout(&out),
        stderr(&out)
    );
    assert!(
        !stderr(&out).contains("SAVED BY THIS RUN"),
        "and it does NOT reach the refusal that turned main red\n{}",
        stderr(&out)
    );
    assert!(
        stderr(&out).contains("UNKNOWN rather than wrong"),
        "it names the key it could not judge and says which of the two answers \
         that is\n{}",
        stderr(&out)
    );
    assert!(
        stdout(&out).contains("over NO INTERVAL") && stdout(&out).contains(&format!("{PREFIX}was")),
        "and the report names the archive whose writing bounds the interval \
         nobody reached, rather than printing a number from a question nobody \
         can see\n{}",
        stdout(&out)
    );
}

/// A run that HAPPENED but did not write the archive does not bound the interval.
///
/// R1207, END TO END, AND THE TWO FIXTURES DIFFER IN ONE WORD. The tree, the
/// storage, the git history and GitHub's answer about the RUNS are identical to
/// the case above; the only difference is that the jobs endpoint reports the
/// save step as `skipped` rather than `success`. A reader that bounds by the
/// run's own conclusion cannot tell those apart, and this repository produces
/// the difference constantly — 122 archives written inside runs that concluded
/// failure over the newest hundred, and 134 of 900 measured bounds older than
/// the truth.
///
/// THE DIRECTION IT MOVES IS THE POINT. Bounding with a run that saved nothing
/// starts the interval too EARLY, and an interval that starts too early excuses
/// a miss whose inputs moved before the archive was actually written — leniency,
/// in a gate whose whole job is to notice.
#[test]
fn a_run_that_wrote_no_archive_does_not_bound_the_interval() {
    let wrote_nothing = Fixture {
        caches: a_key_rebuilt_during_the_run(),
        commits: 3,
        lockfile_moves_at: &[1],
        runs: Runs::RanAtHeadMinusAndWroteNothing(2),
        ..Fixture::default()
    }
    .build();
    let out = gate_in_run(wrote_nothing.path());
    assert_eq!(
        code(&out),
        2,
        "the run is there and the archive is not, so nothing bounds the interval \
         and the miss is UNJUDGED — which R1312 separated from the miss that is \
         unexplained\nstdout:\n{}\nstderr:\n{}",
        stdout(&out),
        stderr(&out)
    );
    assert!(
        stdout(&out).contains("wrote the archive of `Cache cargo (fixture)`"),
        "and the report names the cache whose history it could not find, rather \
         than the workflow's\n{}",
        stdout(&out)
    );
    assert!(
        stdout(&out).contains("1 examined walking back"),
        "and how many runs it asked before saying so — a count of zero and a count \
         of one are different states\n{}",
        stdout(&out)
    );
}

/// A streak of runs that wrote nothing does not put the interval out of reach.
///
/// THIS IS RUN 33607225800, AND IT IS THE FALSE RED R1312 PAID FOR. This
/// repository's workflow declares `cancel-in-progress`, so a push that arrives
/// while a run is in flight cancels it — and a cancelled run usually does not
/// reach its post steps, so it writes no archive. Under a round cadence shorter
/// than a run, that is not an occasional row to skip: measured on 2026-09-02,
/// FOURTEEN consecutive runs wrote no archive at all, and the run that last
/// wrote `Linux-cargo-validate-` was FIFTEEN back — itself a run GitHub calls
/// `cancelled`. The gate asked for a page of FIVE, found no writer in it,
/// substituted the push range, and refused eight archives as unexplained
/// rebuilds — while `.github/workflows/mnemosyne-validate.yml`, which every one
/// of those keys names in its own `hashFiles`, had moved three commits earlier.
///
/// WHAT MADE IT INVISIBLE HERE WAS THE FIXTURE'S OWN `gh`. It ignored
/// `per_page`, so every case in this file received whatever page the fixture
/// wrote however small a page the gate asked for — the parameter that decides
/// this could not be observed, and the constant sat at five for five rounds
/// with nothing able to go red for it. The stub honours it now, which is what
/// makes the streak below a measurement rather than a description.
#[test]
fn a_streak_of_runs_that_wrote_nothing_does_not_put_the_interval_out_of_reach() {
    // SIX SILENT RUNS, one more than the page this gate used to ask for, and the
    // writer behind them. The lockfile moved at the commit that writer ran
    // BEFORE, so the interval since it wrote the archive holds the movement and
    // the push range does not — the same two-interval shape as the case above,
    // with the streak as the only thing standing between them.
    let at = Fixture {
        caches: a_key_rebuilt_during_the_run(),
        commits: 4,
        lockfile_moves_at: &[2],
        runs: Runs::WroteTheArchiveBehindAStreakOf {
            silent: 6,
            wrote_at_head_minus: 3,
        },
        ..Fixture::default()
    }
    .build();
    let out = gate_in_run(at.path());
    assert_eq!(
        code(&out),
        0,
        "the walk goes past every run that wrote nothing and bounds the interval \
         with the one that did, where the lockfile's movement is\nstdout:\n{}\nstderr:\n{}",
        stdout(&out),
        stderr(&out)
    );
    assert!(
        stdout(&out).contains("1 of 1 key(s) had their hashed inputs moved"),
        "and the rebuild is the price of a dependency change\n{}",
        stdout(&out)
    );
    assert!(
        stdout(&out).contains("last wrote its archive") && !stdout(&out).contains("NO INTERVAL"),
        "bounded by the run that wrote the archive, not left unbounded\n{}",
        stdout(&out)
    );
}

/// The walk stops at the archive it is bounding against, not at every run.
///
/// THE OTHER HALF OF THE PAGE R1312 WIDENED. A page of a hundred is a hundred
/// runs whose jobs could be asked about one call each, and this gate used to be
/// kept cheap by a page of five — a number that bought its cost by being wrong.
/// What bounds the cost honestly is the evidence: the run that WROTE an archive
/// started before it, so a candidate older than that archive cannot be the one
/// being looked for and every call past it buys nothing.
///
/// THE ORACLE IS WHAT `gh` WAS ASKED, which is the only place the saving is
/// visible: the verdict is the same either way, and a walk that paid for ninety
/// more calls would be green and slow. The first fixture written for this law
/// had a writer inside the page, so the walk returned before it could overrun
/// anything and the injection aimed here came back 0 red — a law about a stop,
/// posed over a history with nothing to stop at.
#[test]
fn the_walk_stops_at_the_archive_it_is_bounding_against() {
    let at = Fixture {
        // THREE RUNS EITHER SIDE OF THE ARCHIVE, and none of them wrote it. All
        // three newer ones have to be asked about; the first of the older ones
        // settles it, and the two behind it are the calls a walk bounded by a
        // page size rather than by evidence would still pay for.
        caches: page_created(
            2,
            &[
                (
                    &format!("{PREFIX}now"),
                    SMALL,
                    "2026-08-10T10:14:11.221132000Z",
                ),
                (&format!("{PREFIX}was"), SMALL, ARCHIVE_WRITTEN),
            ],
        ),
        commits: 3,
        runs: Runs::SilentBeforeAndAfterTheArchive { newer: 3, older: 3 },
        ..Fixture::default()
    }
    .build();
    let out = gate_in_run(at.path());
    assert_eq!(
        code(&out),
        2,
        "no run wrote the archive, so the interval is unbounded either \
         way\nstdout:\n{}\nstderr:\n{}",
        stdout(&out),
        stderr(&out)
    );
    let asked = std::fs::read_to_string(at.path().join("stub/asked")).expect("the stub recorded");
    let jobs_asked = asked.lines().filter(|line| line.contains("/jobs?")).count();
    assert_eq!(
        jobs_asked, 4,
        "three runs newer than the archive and the FIRST one older, and not a \
         call past it — a candidate that started before an archive cannot be the \
         run that wrote it:\n{asked}"
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
///
/// THE STORAGE HERE HOLDS ONE ARCHIVE AND IT IS NEWER THAN THE RUN, which R1312
/// made load-bearing rather than incidental. A key with an OLDER generation is
/// asked over the interval since that generation was written, and where no run
/// bounds it the answer is that the gate could not look — a refusal that never
/// reaches git, so a fixture built that way would exit `2` for the wrong reason
/// and this law would pass without the shallow checkout being the cause. With no
/// generation to bound against, the push range is the honest interval, the diff
/// IS attempted, and it is the checkout that refuses it.
#[test]
fn a_checkout_with_no_parent_commit_is_unjudged_rather_than_one_that_changed_nothing() {
    let at = Fixture {
        caches: page_created(
            1,
            &[(
                &format!("{PREFIX}abc"),
                SMALL,
                "2026-08-10T10:14:11.221132000Z",
            )],
        ),
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
