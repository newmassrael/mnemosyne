//! BOTH WAYS INTO THIS GATE, RUN AS THE PROGRAM A PERSON RUNS.
//!
//! The census has two entrances and they are not watched alike. One reads the
//! logs a CI run already left, costs nothing beyond the builds that were
//! happening anyway, and runs on every push — so a break in it turns main red the
//! same hour. The other REPRODUCES those logs on this machine, running each job's
//! steps in a worktree of its own; it is how the question gets answered without a
//! push, it takes hours over this repository, and nobody runs it twice.
//!
//! What the second one cost, measured rather than supposed: the day the recorder
//! was wired into every job of `mnemosyne-validate.yml`, all nine of them gained
//! `MNEMOSYNE_RUSTC_LOG: ${{ github.workspace }}/…` — an expression the replay
//! REPLACES before running anything, and which the replay read as one only GitHub
//! can resolve. It refused all nine jobs, produced a census of none, and signed
//! off with `every one of the 0 job(s) … recorded what it compiled`. The suite
//! was green the whole time and stayed that way for a round.
//!
//! Neither entrance lived in the library. Both are `main.rs` — the worktree per
//! job, the two variables the replay owns, the log named for the job that wrote
//! it, the workflow read off the runner, the gate's own job dropped from its own
//! reading — and nothing here ran the binary, so none of it had a reader.
//!
//! So the binary is run as a process, over a workflow small enough to be a test,
//! and its EXIT CODE and REPORT are the assertions. That is deliberate: the
//! defect being guarded against was an exit code of 0 under a report of clean
//! zeroes, and a test that called a function would not have been reading the
//! thing that lied.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use rustc_log::Record;
use tempfile::TempDir;
use twice_compiled::{judge, load, Census, Declared, Refusal};

/// The fixture workflow, written into the fixture repository as a TRACKED file,
/// because a replay checks each job out of a commit rather than out of the tree.
const WORKFLOW: &str = ".github/workflows/fixture.yml";

/// A GitHub expression this machine genuinely cannot resolve, in the variable the
/// `msrv` job carries it in.
///
/// THE CONTROL FOR THE WHOLE FILE. The two variables every fixture job spells
/// with `${{ … }}` are ones the replay OWNS and must run anyway; this is one it
/// does not own and must refuse over. A replay that could not tell the two apart
/// would be green on one of these tests and red on the others whichever way round
/// it was wrong.
const UNRESOLVABLE: &str = "      RUSTUP_TOOLCHAIN: ${{ steps.msrv.outputs.version }}\n";

/// The one step a fixture job usually has: it compiles the shared crate.
const BUILDS: &[&str] = &["cargo build --manifest-path crates/shared/Cargo.toml"];

/// A job that measures its own cache restore, the way every cached job of this
/// repository's workflow does.
///
/// THE TWO MEASUREMENTS RUN BACK TO BACK because in a replay there is nothing
/// between them: an `actions/cache` step is a `uses:` step and the replay runs
/// only `run:` ones. That is the point of running this here — a replay restores
/// no cache, so its census is one taken from an empty tree, and the record has
/// to say so rather than leave the reader to remember it.
///
/// IN THE FILE, THOUGH, THE CACHE STEP SITS BETWEEN THEM — see [`CACHES_AT`].
/// The two are not in tension: the workflow declares a restore between the two
/// readings and the replay simply does not perform it, which is exactly why its
/// census reads as taken from nothing.
const MEASURES_A_RESTORE: &[&str] = &[
    // THE INSTRUMENT IS BUILT OUTSIDE THE TREE THIS JOB CACHES. R1118: this
    // fixture used to build it into `target` and then cache `target`, which is
    // the defect the gate now refuses — a restore replaces the binary between
    // the step that builds it and the step that runs it. The fixture declared it
    // and the new law said so on its first run, which is the fourth time in this
    // arc that turning a law on found the fixture wrong rather than the tree.
    "CARGO_TARGET_DIR=instruments cargo build --manifest-path tools/restored/Cargo.toml",
    // THE PREFIX THE FIXTURE'S OWN CACHE KEY RESOLVES TO. The gate checks the
    // record's cache against what the workflow declares, so a fixture naming
    // some other prefix would be declaring the defect it is not testing for.
    "./instruments/debug/restored before --cache 'Linux-fixture-unrun-tests-' 'target'",
    "./instruments/debug/restored after",
    "cargo build --manifest-path crates/shared/Cargo.toml",
];

/// Where the cache step of [`MEASURES_A_RESTORE`] goes in the emitted `steps:`
/// list: after the measurement that opens the record and before the one that
/// closes it.
///
/// A FIXTURE THAT APPENDED IT AFTER BOTH would declare the very defect the gate
/// refuses — two readings on one side of the restore, whose difference is zero
/// and therefore indistinguishable from a job that compiled from an empty tree.
/// The fixture used to do that, and nothing could see it until the steps carried
/// their positions.
const CACHES_AT: usize = 2;

/// A job that installs what this machine already has, compiles, and then fails.
///
/// THREE DECISIONS OF THE REPLAY IN ONE JOB. `rustup show` is one of the two
/// named prefixes a replay does not run, because it installs a toolchain this
/// machine has; the build after it is what the census is made of; and the failing
/// step is the one the replay refuses to treat as fatal, because the compilations
/// it already paid for are already in the log.
const INSTALLS_THEN_FAILS: &[&str] = &[
    "rustup show",
    "cargo build --manifest-path crates/shared/Cargo.toml",
    "exit 3",
];

/// One job of the fixture workflow.
struct Job {
    /// The job id, which is also the name of the log its records belong in.
    name: &'static str,
    /// Extra `env:` lines it carries, indented for the job's `env:` mapping.
    also: &'static str,
    /// The `run:` scripts it is made of, in order.
    steps: &'static [&'static str],
    /// The `actions/cache` step of this job: WHERE it goes among the steps, and
    /// the paths it holds. `None` for a job with no cache.
    ///
    /// A `uses:` STEP AND NOT A NOTE BESIDE THE FIXTURE, because that is what
    /// the gate reads: `ci_plan::cache_steps` parses the same block out of the
    /// fixture that it parses out of this repository's workflow. A fixture that
    /// declared its caches to the test some other way would be asserting about
    /// a reader nothing uses.
    ///
    /// ONE FIELD AND NOT TWO, so that a position without a cache cannot be
    /// written down at all.
    caches: Option<(usize, &'static [&'static str])>,
}

impl Job {
    fn plain(name: &'static str) -> Job {
        Job {
            name,
            also: "",
            steps: BUILDS,
            caches: None,
        }
    }
}

/// A workflow of jobs that all compile the same crate.
///
/// THE SAME CRATE ON PURPOSE. A census of jobs with nothing in common prints
/// every total it has and holds no finding, and the finding is what this gate is
/// for: the units two jobs both compile are what merging them would remove.
///
/// EVERY JOB SPELLS THE RECORDER'S TWO VARIABLES THE WAY THIS REPOSITORY'S OWN
/// WORKFLOW SPELLS THEM — `${{ github.workspace }}/…`, the exact shape that closed
/// the replay for a round. A simplified fixture would have been green on the day.
fn workflow(jobs: &[Job]) -> String {
    let mut out = String::from("name: a fixture, and not this repository's CI\njobs:\n");
    for job in jobs {
        let name = job.name;
        out.push_str(&format!("  {name}:\n"));
        out.push_str("    runs-on: ubuntu-latest\n");
        out.push_str("    env:\n");
        out.push_str(
            "      RUSTC_WRAPPER: ${{ github.workspace \
             }}/target/release/rustc-log\n",
        );
        out.push_str(&format!(
            "      MNEMOSYNE_RUSTC_LOG: ${{{{ github.workspace }}}}/rustc-log/{name}.log\n"
        ));
        if job.caches.is_some() {
            out.push_str(&format!(
                "      MNEMOSYNE_RESTORED: ${{{{ github.workspace }}}}/rustc-log/{name}.restored\n"
            ));
        }
        out.push_str(job.also);
        // A POSITION PAST THE END WOULD APPEND THE CACHE SILENTLY, leaving both
        // measurements on one side of the restore — the defect the gate refuses,
        // written into the fixture that is supposed to be the control for it.
        if let Some((at, _)) = job.caches {
            assert!(
                at < job.steps.len(),
                "job `{name}` puts its cache step at {at} of {} step(s)",
                job.steps.len()
            );
        }
        out.push_str("    steps:\n");
        // THE CACHE STEP IS WOVEN IN AT ITS POSITION rather than appended, so
        // that the fixture declares the shape a real workflow has.
        for (index, step) in job.steps.iter().enumerate() {
            if let Some((at, paths)) = job.caches {
                if index == at {
                    out.push_str(&cache_step(name, paths));
                }
            }
            out.push_str(&format!("      - run: {step}\n"));
        }
        // WHAT MAKES THE RECORDS READABLE, and a fixture without it declares the
        // defect it is the control for: a job that writes a record in a workflow
        // uploading nothing puts it on a runner that is destroyed when the job
        // ends. This repository's own workflow uploads `rustc-log/` from every
        // job, which is how the gate job gets nine of them.
        out.push_str(&format!(
            "      - uses: actions/upload-artifact@v7\n        with:\n          \
             name: rustc-log-{name}\n          path: rustc-log/\n"
        ));
    }
    out
}

/// One `actions/cache` step over the paths a fixture job holds.
fn cache_step(job: &str, paths: &[&str]) -> String {
    let mut out =
        String::from("      - uses: actions/cache@v6\n        with:\n          path: |\n");
    for path in paths {
        out.push_str(&format!("            {path}\n"));
    }
    out.push_str(&format!(
        "          key: ${{{{ runner.os }}}}-fixture-{job}-\
         ${{{{ hashFiles('**/Cargo.lock') }}}}\n"
    ));
    out
}

/// The recorder's own source, beside this crate because the dependency on it is
/// declared as `path = "../rustc-log"`.
///
/// COPIED INTO THE FIXTURE rather than reached into, because a replay builds the
/// recorder from the root it is given and would otherwise write a release binary
/// into this repository's own tree while the tests run.
fn recorder_source() -> PathBuf {
    sibling("rustc-log")
}

/// A tool workspace beside this one.
fn sibling(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("this crate has a directory beside it")
        .join(name)
}

/// One end-to-end run of the gate: the process, and the trees either side of it.
struct Run {
    /// The process, whole. This gate's verdict is its exit code and its report is
    /// its stdout, and both are what a person running it reads.
    output: Output,
    /// The fixture repository. Held because dropping it deletes the tree.
    root: TempDir,
    /// The scratch. Held for the same reason.
    scratch: TempDir,
}

impl Run {
    fn stdout(&self) -> String {
        String::from_utf8_lossy(&self.output.stdout).into_owned()
    }

    /// Everything the process said, so a failing assertion explains itself with
    /// the run rather than with a boolean.
    fn transcript(&self) -> String {
        format!(
            "exit {:?}\n--- stdout ---\n{}\n--- stderr ---\n{}",
            self.output.status.code(),
            self.stdout(),
            String::from_utf8_lossy(&self.output.stderr),
        )
    }

    fn logs(&self) -> PathBuf {
        self.scratch.path().join("logs")
    }

    /// The census READ BACK OUT OF THE LOGS — a second reading of what the
    /// process reported, and the one a test can assert against.
    fn census(&self) -> Census {
        load(&self.logs(), &BTreeSet::new()).unwrap_or_else(|error| {
            panic!(
                "cannot read {}: {error}\n{}",
                self.logs().display(),
                self.transcript()
            )
        })
    }

    /// What the fixture workflow declares — both populations, read out of the
    /// fixture by the same reader that reads this repository's own workflow.
    fn declared(&self) -> Declared {
        let document = ci_plan::load_workflow(self.root.path(), WORKFLOW);
        Declared::of(
            &ci_plan::run_steps(&document),
            &ci_plan::cache_steps(&document, WORKFLOW),
            &ci_plan::artifact_uploads(&document, WORKFLOW),
        )
    }

    /// Where a record lands if the replay stops setting the log variable for the
    /// steps it runs. Nothing may ever be written here.
    fn escaped(&self) -> PathBuf {
        self.scratch.path().join("escaped.log")
    }
}

/// Write the fixture repository: the workflow, the crate the jobs compile, the
/// recorder's source, and one commit for `git worktree add … HEAD` to check out.
fn repository(jobs: &[Job]) -> (TempDir, TempDir) {
    let root = tempfile::tempdir().expect("a fixture repository");
    let scratch = tempfile::tempdir().expect("a scratch directory");
    write(&root.path().join(WORKFLOW), &workflow(jobs));
    write(
        &root.path().join("crates/shared/Cargo.toml"),
        "[workspace]\n\n[package]\nname = \"shared\"\nversion = \"0.1.0\"\n\
         edition = \"2021\"\n",
    );
    write(
        &root.path().join("crates/shared/src/lib.rs"),
        "//! One crate, no dependencies, compiled by every job of the fixture.\n\
         pub fn shared() -> u8 {\n    1\n}\n",
    );
    // THE FIXTURE IS SHAPED LIKE THE REPOSITORY IT STANDS FOR. This checkout
    // builds every one of its workspaces into ONE directory, which is why the
    // steps below can name `./target/debug/restored` for a crate whose manifest
    // lives under `tools/`. A fixture without this file puts that binary
    // somewhere else and the step exits 127 — a job that measured nothing, for
    // a reason that has nothing to do with what the test is about.
    write(
        &root.path().join(".cargo/config.toml"),
        "[build]\ntarget-dir = \"target\"\n",
    );
    copy_instrument(&recorder_source(), &root.path().join("tools/rustc-log"));
    copy_instrument(&sibling("restored"), &root.path().join("tools/restored"));
    commit(root.path());
    (root, scratch)
}

/// Run the real binary with `--replay`, which reproduces every job it can.
fn replay(jobs: &[Job]) -> Run {
    let (root, scratch) = repository(jobs);
    // A POISONED VALUE FOR A VARIABLE THE REPLAY OWNS. If it stops applying its
    // own, every record goes here instead of into a job's log — a file the test
    // can find, rather than an absence it would have to explain.
    let output = gate(root.path(), scratch.path())
        .arg("--replay")
        .arg(scratch.path())
        .args(["--workflow", WORKFLOW])
        .output()
        .expect("the gate runs");
    Run {
        output,
        root,
        scratch,
    }
}

/// Run the real binary the way a runner runs it: over a directory of logs the
/// other jobs already wrote, with the workflow read off the runner's own
/// environment rather than from a flag.
fn on_a_runner(jobs: &[Job], recorded: &[&str], own_job: Option<&str>) -> Run {
    let (root, scratch) = repository(jobs);
    let logs = scratch.path().join("logs");
    for job in recorded {
        // ANCHORED TO THE CHECKOUT THE GATE WILL RUN IN, because the coverage
        // reading joins an absolute `--out-dir` against the workflow's relative
        // `path:` and a destination on another machine is one it refuses to
        // read. This harness is where that join is exercised at all.
        record_a_compilation_into(
            &logs.join(format!("{job}.log")),
            &root.path().join("target/debug/deps").display().to_string(),
        );
        // AND ONE CRATE CARGO FETCHED, so every census this harness builds has a
        // row on BOTH sides of the split. A report whose fetched row is only
        // ever zero is one no test can tell from a report that stopped counting.
        record_a_fetched_compilation(&logs.join(format!("{job}.log")));
    }
    let mut gate = gate(root.path(), scratch.path());
    gate.arg(&logs).env(
        "GITHUB_WORKFLOW_REF",
        format!("newmassrael/mnemosyne/{WORKFLOW}@refs/heads/main"),
    );
    match own_job {
        Some(job) => gate.env("GITHUB_JOB", job),
        // EXPLICITLY REMOVED AND NOT MERELY UNSET BY THE TEST. On a runner this
        // suite is itself inside a job, so the variable is in the environment
        // with somebody else's job in it.
        None => gate.env_remove("GITHUB_JOB"),
    };
    let output = gate.output().expect("the gate runs");
    Run {
        output,
        root,
        scratch,
    }
}

fn gate(root: &Path, scratch: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_twice-compiled"));
    command
        .current_dir(root)
        .env(rustc_log::LOG_VARIABLE, scratch.join("escaped.log"));
    command
}

/// One job's log, written through the recorder's own writer rather than by
/// spelling its format a second time here.
///
/// TWO RECORDS OF THE SAME UNIT, at times that are not zero: a job whose
/// compilations took no time at all is refused, and so is one whose log holds
/// nothing. The unit is the same in every job, so a census of two of these has
/// the finding this gate exists to report in it.
///
/// AND IT SAYS WHERE IT WROTE. cargo passes `--out-dir` to every compilation it
/// drives, and a fixture that left it out was a fixture the gate rightly refuses
/// — a destination nothing said is one no coverage can be read from. `TARGET` is
/// spelled relative here because the caller knows the checkout and this does not.
fn record_a_compilation(log: &Path) {
    record_a_compilation_into(log, "target/debug/deps")
}

/// Where the fixtures above write, when the caller has a checkout to anchor to.
fn record_a_compilation_into(log: &Path, out_dir: &str) {
    for start in [1_000_000_u64, 1_200_000] {
        let record = Record {
            started_at: start,
            micros: 150_000,
            argv: [
                "/usr/bin/rustc",
                "--crate-name",
                "shared",
                "--emit=dep-info,link",
                "-C",
                "metadata=57ac1f0b",
                "--crate-type",
                "lib",
                "src/lib.rs",
                "--out-dir",
                out_dir,
            ]
            .iter()
            .map(|word| (*word).to_string())
            .collect(),
        };
        rustc_log::append(log, &record).expect("a record is appended");
    }
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().expect("a parent directory"))
        .expect("a directory for the fixture file");
    std::fs::write(path, text).unwrap_or_else(|error| {
        panic!("cannot write {}: {error}", path.display());
    });
}

/// Copy one instrument's manifest and sources into the fixture.
///
/// The count is asserted: a copy that moved nothing leaves a fixture whose
/// instrument cannot be built, and the replay would then fail for a reason that
/// has nothing to do with what is being tested.
fn copy_instrument(from: &Path, into: &Path) {
    std::fs::create_dir_all(into.join("src")).expect("an instrument directory");
    std::fs::copy(from.join("Cargo.toml"), into.join("Cargo.toml"))
        .expect("the instrument's manifest");
    let mut copied = 0;
    for entry in std::fs::read_dir(from.join("src")).expect("the instrument's sources") {
        let path = entry.expect("a source file").path();
        if path.extension().and_then(|end| end.to_str()) != Some("rs") {
            continue;
        }
        let name = path.file_name().expect("a file name");
        std::fs::copy(&path, into.join("src").join(name)).expect("an instrument source");
        copied += 1;
    }
    assert!(
        copied >= 2,
        "each instrument is a library and a binary; {} source file(s) were \
         copied from {}",
        copied,
        from.display()
    );
}

/// Make the fixture a repository with one commit, which is what `git worktree
/// add … HEAD` needs.
fn commit(root: &Path) {
    git(root, &["init", "--quiet"]);
    git(root, &["add", "-A"]);
    git(
        root,
        &[
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "user.name=fixture",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "--no-verify",
            "-m",
            "the fixture this gate runs over",
        ],
    );
}

fn git(root: &Path, arguments: &[&str]) {
    let out = Command::new("git")
        .args(arguments)
        .current_dir(root)
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {arguments:?} failed in {}: {}",
        root.display(),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn a_replay_reaches_every_job_it_can_and_the_census_holds_what_they_share() {
    // THE TEST THE ROUND BEFORE THIS ONE DID NOT HAVE. Every assertion below was
    // false on a tree whose suite was green: the replay ran no job, wrote no log,
    // read a census of nothing, and exited 0.
    let run = replay(&[Job::plain("one-job"), Job::plain("the-other-job")]);
    assert!(
        run.output.status.success(),
        "a replay of two replayable jobs must produce a census\n{}",
        run.transcript()
    );
    let stdout = run.stdout();
    assert!(
        !stdout.contains("SKIP"),
        "neither job carries anything this machine cannot resolve\n{}",
        run.transcript()
    );
    assert!(
        stdout.contains("every one of the 2 job(s)"),
        "the sign-off says how far the census reached\n{}",
        run.transcript()
    );

    let census = run.census();
    let reached: BTreeSet<&str> = census.jobs.keys().map(String::as_str).collect();
    assert_eq!(
        reached,
        BTreeSet::from(["one-job", "the-other-job"]),
        "one log per job, named for it\n{}",
        run.transcript()
    );
    for (job, log) in &census.jobs {
        assert!(
            !log.units.is_empty(),
            "job `{job}` ran a build and recorded no compilation\n{}",
            run.transcript()
        );
        // THE CLOCK, ASSERTED APART FROM THE COUNT. A recorder that stopped
        // timing leaves counts that all add up and seconds that are all zero,
        // which reads as work that is free.
        assert!(
            log.compiled_micros() > 0,
            "job `{job}` compiled {} unit(s) in no time at all\n{}",
            log.units.len(),
            run.transcript()
        );
        assert!(
            log.span_micros() > 0,
            "job `{job}` has a compiling window of zero\n{}",
            run.transcript()
        );
    }

    // THE FINDING, AND NOT MERELY A NON-EMPTY FILE. Both jobs compile the same
    // crate in worktrees of their own, so the census must say they share it —
    // the number this gate exists to report, produced end to end rather than
    // assembled by a fixture.
    assert!(
        census.shared_between_jobs() > 0,
        "both jobs compile `crates/shared`, so the census must hold a unit they \
         both paid for\n{}",
        run.transcript()
    );
    let refusals: Vec<Refusal> = judge(&census, &run.declared(), &BTreeSet::new());
    assert!(
        refusals.is_empty(),
        "the census this replay produced is refused: {refusals:?}\n{}",
        run.transcript()
    );
    assert!(
        !run.escaped().exists(),
        "a record was written to {} — the replay stopped setting ${} for the \
         steps it runs, so what those steps compiled is outside every job's \
         log\n{}",
        run.escaped().display(),
        rustc_log::LOG_VARIABLE,
        run.transcript()
    );
}

#[test]
fn a_replay_that_reaches_one_job_is_refused_rather_than_reported() {
    // A CENSUS OF ONE JOB IS NOT A SMALL CENSUS. Its subject is what TWO jobs both
    // compile, so one job has no finding available to it — and it prints a clean
    // zero for every total, which is the same output as a repository with no
    // duplication in it at all.
    let run = replay(&[
        Job::plain("one-job"),
        Job {
            name: "the-other-job",
            also: UNRESOLVABLE,
            steps: BUILDS,
            caches: None,
        },
    ]);
    assert_eq!(
        run.output.status.code(),
        Some(1),
        "a replay that could run only one of two jobs must refuse\n{}",
        run.transcript()
    );
    let stdout = run.stdout();
    assert!(
        stdout.contains("[replay] SKIP the-other-job"),
        "the job carrying an expression GitHub resolves is named, not dropped\n{}",
        run.transcript()
    );
    assert!(
        stdout.contains("covers 1 job(s)"),
        "the refusal says how far the census reached\n{}",
        run.transcript()
    );

    // AND THE REPLAY ITSELF STILL WORKED. Without this the assertions above would
    // pass on a tree where nothing runs at all: the refusal is about REACH, and
    // the job it did reach has to be in the logs with its compilations in it.
    let census = run.census();
    let reached: BTreeSet<&str> = census.jobs.keys().map(String::as_str).collect();
    assert_eq!(
        reached,
        BTreeSet::from(["one-job"]),
        "the replayable job runs and records even though the census is refused\n{}",
        run.transcript()
    );
    assert!(
        !census.jobs["one-job"].units.is_empty(),
        "the job that ran compiled nothing\n{}",
        run.transcript()
    );
}

#[test]
fn a_replay_that_reaches_no_job_does_not_sign_off_as_clean() {
    // THE EXACT SHAPE OF THE DEFECT: every job unresolvable, no log written, and a
    // report of zeroes — the state a real replay was in for a round while the
    // suite stayed green. What must not happen is the sign-off.
    let run = replay(&[
        Job {
            name: "one-job",
            also: UNRESOLVABLE,
            steps: BUILDS,
            caches: None,
        },
        Job {
            name: "the-other-job",
            also: UNRESOLVABLE,
            steps: BUILDS,
            caches: None,
        },
    ]);
    assert_eq!(
        run.output.status.code(),
        Some(1),
        "a replay that ran no job at all must refuse\n{}",
        run.transcript()
    );
    let stdout = run.stdout();
    assert!(
        !stdout.contains("recorded what it compiled"),
        "a census of no jobs printed the sign-off of a file in good order\n{}",
        run.transcript()
    );
    assert!(
        stdout.contains("covers 0 job(s)"),
        "the refusal says how far the census reached\n{}",
        run.transcript()
    );
    assert!(
        !run.logs().join("one-job.log").exists(),
        "a skipped job left a log\n{}",
        run.transcript()
    );
}

#[test]
fn a_replay_skips_what_it_cannot_install_and_carries_on_past_what_fails() {
    // TWO DECISIONS OF THE REPLAY THAT HAD NO READER, in one job that carries
    // both. A replay runs a job's steps VERBATIM except for two named prefixes —
    // `sudo` and `rustup`, which install what a hosted runner lacks and this
    // machine already has — and a step that fails is NAMED rather than fatal,
    // because the compilations it already paid for are already in the log and the
    // census is of what CI paid for. Get the first wrong and the replay installs
    // a toolchain over the one it is measuring; get the second wrong and one
    // failing gate throws away every job after it.
    let run = replay(&[
        Job::plain("one-job"),
        Job {
            name: "the-other-job",
            also: "",
            steps: INSTALLS_THEN_FAILS,
            caches: None,
        },
    ]);
    assert!(
        run.output.status.success(),
        "a step that failed is not a census that failed\n{}",
        run.transcript()
    );
    let stdout = run.stdout();
    assert!(
        stdout.contains("skipping `rustup`"),
        "the step this machine does not need is named as skipped, not silently \
         dropped\n{}",
        run.transcript()
    );
    assert!(
        stdout.contains("its compilations still count"),
        "the step that exited 3 is named, and its build is still in the census\n{}",
        run.transcript()
    );
    let census = run.census();
    assert!(
        !census.jobs["the-other-job"].units.is_empty(),
        "the job whose last step failed compiled before it did\n{}",
        run.transcript()
    );
    assert!(
        census.shared_between_jobs() > 0,
        "the failing job still contributes what it shares with the other\n{}",
        run.transcript()
    );
}

#[test]
fn a_replay_takes_every_job_from_one_revision_and_says_which() {
    // A CI RUN MEASURES ONE COMMIT AND A REPLAY TAKES HOURS, so `HEAD` — a name
    // for whatever the repository is on — is not a revision. A commit landing
    // while a replay works moves it, and the census then covers two source trees
    // with nothing in the report saying so. That happened: a replay of this
    // workflow took four jobs from one commit and its fifth from another that
    // arrived mid-run, and the only reason the numbers survived is that the
    // commit touched no manifest.
    let run = replay(&[Job::plain("one-job"), Job::plain("the-other-job")]);
    assert!(run.output.status.success(), "{}", run.transcript());
    let head = String::from_utf8_lossy(
        &Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(run.root.path())
            .output()
            .expect("git resolves the fixture's HEAD")
            .stdout,
    )
    .trim()
    .to_string();
    // ONCE, AND WITH THE HASH IN IT. Once is the law — a line per job would mean
    // the revision is re-read per job, which is the defect itself — and the hash
    // is what makes the census comparable to any other measurement of this
    // repository. A gate that cannot name what it measured is one nobody can put
    // beside anything else.
    let said = format!("[replay] every job from {head}, resolved once");
    assert_eq!(
        run.stdout().matches(&said).count(),
        1,
        "the replay must resolve `{head}` once and say so once\n{}",
        run.transcript()
    );
}

/// The three jobs the runner tests are about: two that leave logs, and the one
/// the gate itself runs in.
fn a_runners_jobs() -> [Job; 3] {
    [
        Job::plain("one-job"),
        Job::plain("the-other-job"),
        Job::plain("the-gate"),
    ]
}

#[test]
fn on_a_runner_the_workflow_is_read_off_the_runner_and_the_gate_leaves_itself_out() {
    // THE OTHER ENTRANCE, and two decisions in it that had no reader. Which
    // workflow this census is of comes from `$GITHUB_WORKFLOW_REF` — the runner's
    // own answer, so a file renamed tomorrow does not leave a gate pointed at a
    // path that is gone. And the job the gate is RUNNING IN is dropped from the
    // census: its build is still in flight while it judges, so it can have no log
    // yet, and counting it would refuse the gate on every push.
    let run = on_a_runner(
        &a_runners_jobs(),
        &["one-job", "the-other-job"],
        Some("the-gate"),
    );
    assert!(
        run.output.status.success(),
        "two jobs recorded and the third is the gate itself\n{}",
        run.transcript()
    );
    let stdout = run.stdout();
    assert!(
        stdout.contains("every one of the 2 job(s)"),
        "three jobs declared, one of them this gate: the census is of two\n{}",
        run.transcript()
    );
    assert!(
        stdout.contains("the-gate") && stdout.contains("not in this census"),
        "the job left out is PRINTED, because a reader cannot tell a gate that \
         skipped a job from one that never saw it\n{}",
        run.transcript()
    );
    // The census is still a census: the two jobs that did record share a unit,
    // which is the finding this entrance exists to report.
    assert!(
        run.census().shared_between_jobs() > 0,
        "the two recorded jobs compiled the same unit\n{}",
        run.transcript()
    );
    // AND THE REPORT SAYS WHOSE CODE IT WAS. Every cache in this workflow
    // carries `~/.cargo/registry` and `~/.cargo/git`, which hold sources; a job
    // that restored them exactly and still compiled crates out of them is the
    // reading this line exists for, and it lives in `main.rs`, where R1096
    // measured what a decision with no reader costs.
    assert!(
        stdout.contains("1 of them fetched by cargo") && stdout.contains("2 from the checkout"),
        "the per-job split is printed with both sides of it\n{}",
        run.transcript()
    );
    // THE TOTALS ROW IS NAMED BY ITS SHARE AND NOT BY ITS WORDS. The coverage
    // rows below spell the same origins, so an assertion that the origin words
    // appear anywhere was satisfied by them the moment they existed — and the
    // injection that removes the totals loop came back green. A percentage is
    // printed by that line and by nothing else here.
    assert!(
        stdout
            .lines()
            .any(|line| line.contains("fetched from a registry") && line.contains("%)")),
        "and so is the total it is a share of\n{}",
        run.transcript()
    );
    // AND WHETHER THE CACHE REACHES WHERE THAT WORK WENT. These fixture jobs
    // declare no cache at all, so every destination is outside it — which is
    // the row that says the work no restore could have spared, and it names
    // the tree because the repair is a `path:` line.
    assert!(
        stdout.contains("WHERE NO PATH OF ITS CACHE REACHES"),
        "the coverage row is printed, and the census is on this machine so it \
         is readable at all\n{}",
        run.transcript()
    );
    assert!(
        !stdout.contains("taken on another machine"),
        "these destinations ARE under the checkout the gate ran in — a reader \
         that said otherwise is joining against the wrong root\n{}",
        run.transcript()
    );
}

#[test]
fn on_a_runner_a_gate_that_did_not_know_its_own_job_would_refuse_every_push() {
    // THE CONTROL FOR THE TEST ABOVE, and the reason that drop is load-bearing
    // rather than tidy. With nothing saying which job this is, the gate's own job
    // is a declared job with no log — the same shape as a job whose recorder was
    // never wired in — and the gate refuses the push it is part of.
    let run = on_a_runner(&a_runners_jobs(), &["one-job", "the-other-job"], None);
    assert_eq!(
        run.output.status.code(),
        Some(1),
        "a gate that counts its own half-built job must refuse it\n{}",
        run.transcript()
    );
    assert!(
        run.stdout()
            .contains("job `the-gate` recorded no compilation"),
        "the refusal names the job, which is what makes it diagnosable\n{}",
        run.transcript()
    );
}

/// THE REPLAY SAYS WHAT ITS CENSUS WAS TAKEN FROM, end to end.
///
/// A replay runs a job's `run:` steps in a worktree and never its cache step, so
/// every census it produces is of a build from nothing. That was true before
/// this round too, and nothing said it: a reader holding a replay's numbers
/// beside a CI run's was comparing a cold tree against a warm one with no line
/// anywhere to notice. The three variables the replay now owns are what put it
/// in the record — the file to write, the job's own name, and `cache-hit`, which
/// is honestly `false` because no key was consulted at all.
#[test]
fn a_replayed_job_records_that_it_started_from_nothing() {
    let run = replay(&[
        Job {
            name: "unrun-tests",
            also: "",
            steps: MEASURES_A_RESTORE,
            caches: Some((CACHES_AT, &["target"])),
        },
        Job::plain("validate"),
    ]);
    assert!(run.output.status.success(), "{}", run.transcript());

    let record = run.logs().join("unrun-tests.restored");
    assert!(
        record.is_file(),
        "the replay set ${} for the steps it ran, so the record is beside the \
         log at {}\n{}",
        restored::VARIABLE,
        record.display(),
        run.transcript()
    );
    let census = run.census();
    let read = census
        .restored
        .get("unrun-tests.restored")
        .expect("the census loaded the record beside the log")
        .as_ref()
        .unwrap_or_else(|why| panic!("{why}\n{}", run.transcript()));
    assert_eq!(
        read.job,
        "unrun-tests",
        "the name came from the runner's own variable, which the replay sets to \
         the job it is running\n{}",
        run.transcript()
    );
    assert_eq!(
        read.measured(),
        vec!["target"],
        "the paths are the ones the first step named\n{}",
        run.transcript()
    );
    assert_eq!(
        read.warmth(),
        restored::Warmth::Nothing,
        "a replay restores nothing, and the two measurements around the absent \
         restore are what say so\n{}",
        run.transcript()
    );
    // AND THE REPORT CARRIES IT, because the number that gets quoted is the one
    // printed, and the number that got quoted is what deleted a working cache.
    assert!(
        run.stdout().contains("started from"),
        "the report says what each job started from\n{}",
        run.transcript()
    );
    assert!(
        run.stdout().contains("is not this one's control"),
        "and says it of the totals, where a reader takes them from\n{}",
        run.transcript()
    );
}

/// The control for the test above: the same gate over a fixture whose jobs
/// declare no cache reports no state and refuses nothing, because the population
/// that owes a record is the jobs with something to restore.
#[test]
fn a_job_with_no_cache_leaves_no_record_and_is_not_refused_for_it() {
    let run = replay(&[Job::plain("validate"), Job::plain("unrun-tests")]);
    assert!(run.output.status.success(), "{}", run.transcript());
    assert!(
        run.census().restored.is_empty(),
        "nothing wrote a restore record: {}",
        run.transcript()
    );
    assert!(
        run.stdout().contains("declares no cache"),
        "and the report says that is what happened, rather than leaving a job \
         with no line at all\n{}",
        run.transcript()
    );
}

// --- the third entrance: one census held against another ---------------------

/// A `gh run download` directory: one subdirectory per artifact, each holding
/// the records of one job. NOT the shape a runner hands the gate, and that is
/// the point — a loader that knew only the flat one reads this as a run in which
/// nothing recorded anything, which prints every total as a clean zero.
fn downloaded(under: &Path, jobs: &[(&str, bool, u64)]) -> PathBuf {
    let directory = under.to_path_buf();
    for (job, exact, arrived) in jobs {
        let artifact = directory.join(format!("rustc-log-{job}"));
        std::fs::create_dir_all(&artifact).expect("the artifact directory");
        record_a_compilation(&artifact.join(format!("{job}.log")));
        let mut written = restored::encode_job(job);
        written.extend_from_slice(&restored::encode_cache(&format!("Linux-fixture-{job}-")));
        // ONE COMMIT FOR EVERY JOB, which is the ordinary case: the instruments
        // are built by one step of each job from the checkout the run is of.
        for instrument in restored::INSTRUMENTS {
            written.extend_from_slice(&restored::encode_built_from(instrument, "0f0f0f"));
        }
        written.extend_from_slice(&restored::encode_at(restored::Side::Before, 1_000_000_000));
        written.extend_from_slice(&restored::encode_side(
            restored::Side::Before,
            "target",
            &restored::Measurement::default(),
        ));
        written.extend_from_slice(&restored::encode_side(
            restored::Side::After,
            "target",
            &restored::Measurement {
                entries: 1,
                bytes: *arrived,
            },
        ));
        written.extend_from_slice(&restored::encode_at(restored::Side::After, 1_030_000_000));
        written.extend_from_slice(&restored::encode_exact(*exact));
        std::fs::write(artifact.join(format!("{job}.restored")), written)
            .expect("the restore record");
    }
    directory
}

#[test]
fn a_census_is_read_out_of_the_shape_gh_run_download_leaves() {
    let scratch = TempDir::new().expect("a scratch directory");
    let collected = downloaded(
        &scratch.path().join("run"),
        &[("validate", false, 7_000), ("unrun-tests", false, 9_000)],
    );
    let census = twice_compiled::load_collected(&collected).expect("the census loads");
    assert_eq!(
        census.jobs.keys().collect::<Vec<_>>(),
        vec!["unrun-tests", "validate"],
        "a loader that read only the top level would find nothing here, and \
         nothing prints as a clean zero"
    );
    assert_eq!(
        census.started().get(&restored::Restore {
            job: "validate".to_string(),
            cache: "Linux-fixture-validate-".to_string(),
        }),
        Some(&restored::Warmth::PrefixHit { bytes: 7_000 }),
        "and the state each restore began in comes with it"
    );
}

#[test]
fn one_job_recorded_in_two_places_is_an_error_rather_than_an_overwrite() {
    // TWO RUNS UNPACKED INTO ONE DIRECTORY. Whichever the walk reached last would
    // silently become the answer, and the walk's order is the filesystem's.
    let scratch = TempDir::new().expect("a scratch directory");
    let collected = downloaded(&scratch.path().join("run"), &[("validate", false, 7_000)]);
    let twice = collected.join("rustc-log-validate-again");
    std::fs::create_dir_all(&twice).expect("the second directory");
    record_a_compilation(&twice.join("validate.log"));
    let why = twice_compiled::load_collected(&collected).expect_err("two records for one job");
    assert!(
        why.to_string().contains("validate") && why.to_string().contains("two places"),
        "{why}"
    );
}

/// Run the third entrance as a PROCESS, because its verdict is an exit code and
/// an exit code is the one thing no library test can be handed.
fn compared(earlier: &Path, later: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_twice-compiled"))
        .args(["compare"])
        .arg(earlier)
        .arg(later)
        // NO REPOSITORY AND NO WORKFLOW REFERENCE, deliberately: this entrance
        // must not need either, and a temporary directory is where that is
        // provable rather than assumed.
        .current_dir(earlier)
        .env_remove("GITHUB_WORKFLOW_REF")
        .env_remove("GITHUB_JOB")
        .output()
        .expect("the gate runs")
}

#[test]
fn two_censuses_taken_in_one_state_compare_and_the_gate_says_so() {
    let scratch = TempDir::new().expect("a scratch directory");
    let earlier = downloaded(
        &scratch.path().join("earlier"),
        &[("validate", false, 7_000)],
    );
    let later = downloaded(&scratch.path().join("later"), &[("validate", false, 9_000)]);
    let out = compared(&earlier, &later);
    let printed = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "both jobs began warm from an earlier generation\n{printed}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(printed.contains("CI paid"), "{printed}");
}

#[test]
fn a_pair_that_began_differently_is_refused_by_the_process_and_not_only_by_a_type() {
    let scratch = TempDir::new().expect("a scratch directory");
    // COLD ON ONE SIDE, WARM ON THE OTHER — the Round 1099 pair, and the counts
    // are deliberately identical, which is exactly what made it convincing.
    let earlier = downloaded(&scratch.path().join("earlier"), &[("validate", false, 0)]);
    let later = downloaded(&scratch.path().join("later"), &[("validate", false, 9_000)]);
    let out = compared(&earlier, &later);
    let printed = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        out.status.code(),
        Some(1),
        "the exit code is the verdict\n{printed}"
    );
    assert!(printed.contains("NO TOTALS"), "{printed}");
    assert!(
        printed.contains("began in different states"),
        "and it names the job and both states\n{printed}"
    );
}

#[test]
fn the_comparison_entrance_refuses_a_workflow_rather_than_ignoring_it() {
    let scratch = TempDir::new().expect("a scratch directory");
    let earlier = downloaded(
        &scratch.path().join("earlier"),
        &[("validate", false, 7_000)],
    );
    let out = Command::new(env!("CARGO_BIN_EXE_twice-compiled"))
        .args(["compare", "a", "b", "--workflow", "w.yml"])
        .current_dir(scratch.path())
        .output()
        .expect("the gate runs");
    assert_eq!(
        out.status.code(),
        Some(2),
        "could-not-look and these-break-the-law are different answers"
    );
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("reads no workflow"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = earlier;
}

/// Append one compilation of a crate cargo fetched, so a census holds a row on
/// each side of the split rather than only the checkout's.
fn record_a_fetched_compilation(log: &Path) {
    let record = Record {
        started_at: 1_400_000,
        micros: 250_000,
        argv: [
            "/usr/bin/rustc",
            "--crate-name",
            "serde",
            "--emit=dep-info,metadata",
            "-C",
            "metadata=9911aabb",
            "--crate-type",
            "lib",
            "/home/runner/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/\
             serde-1.0.219/src/lib.rs",
            "--out-dir",
            "target/debug/deps",
        ]
        .iter()
        .map(|word| (*word).to_string())
        .collect(),
    };
    rustc_log::append(log, &record).expect("a record is appended");
}

#[test]
fn a_comparison_says_whose_code_each_jobs_difference_is_in() {
    // A DELTA WITH NO CAUSE IN IT IS A NUMBER NOBODY CAN ACT ON. The same
    // hundred compilations mean "somebody added code" if they are the
    // checkout's and "a cache brought sources whose compiled form it did not
    // bring" if they are crates cargo fetched, and the count cannot tell those
    // apart.
    let scratch = TempDir::new().expect("a scratch directory");
    let earlier = downloaded(
        &scratch.path().join("earlier"),
        &[("validate", false, 7_000)],
    );
    let later = downloaded(&scratch.path().join("later"), &[("validate", false, 7_000)]);
    record_a_fetched_compilation(&later.join("rustc-log-validate").join("validate.log"));

    let out = compared(&earlier, &later);
    let printed = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{printed}");
    assert!(
        printed.contains("fetched from a registry"),
        "the row names the tree the difference is in\n{printed}"
    );
    assert!(
        printed.contains("in the checkout"),
        "and both sides of the split are printed, not only the one that moved\
         \n{printed}"
    );
    assert!(
        printed.contains("     0 -> 1"),
        "an origin only the later census had is a row whose earlier side is \
         zero, and never a row that is dropped\n{printed}"
    );
}

#[test]
fn a_comparison_says_what_neither_side_could_place() {
    // `compare` READS NO WORKFLOW, so it has nothing to refuse against — what
    // it owes instead is to say how much of each job it is not describing.
    // Without the line, compilations missing from every number above read as
    // compilations that did not happen.
    let scratch = TempDir::new().expect("a scratch directory");
    let earlier = downloaded(
        &scratch.path().join("earlier"),
        &[("validate", false, 7_000)],
    );
    let later = downloaded(&scratch.path().join("later"), &[("validate", false, 7_000)]);
    let unplaceable = Record {
        started_at: 1_600_000,
        micros: 10_000,
        argv: [
            "/usr/bin/rustc",
            "--crate-name",
            "adrift",
            "--emit=link",
            "-C",
            "metadata=1234",
            "--a-flag-this-reader-does-not-know",
            "its-value",
            "src/lib.rs",
        ]
        .iter()
        .map(|word| (*word).to_string())
        .collect(),
    };
    rustc_log::append(
        &later.join("rustc-log-validate").join("validate.log"),
        &unplaceable,
    )
    .expect("a record is appended");

    let printed = String::from_utf8_lossy(&compared(&earlier, &later).stdout).into_owned();
    assert!(
        printed.contains("NOT PLACED: `adrift`"),
        "the line names the crate\n{printed}"
    );
    assert!(
        printed.contains("its-value"),
        "and the words that stood where one path should be\n{printed}"
    );
}

#[test]
fn two_directories_holding_nothing_do_not_compare_as_two_runs_that_agreed() {
    // THE EMPTY ANSWER THAT LOOKS LIKE A CLEAN ONE, at the entrance where it is
    // cheapest to produce: a mistyped path, an artifact pattern that matched
    // nothing, a download that failed. Both totals are honestly zero and every
    // job is honestly comparable, because there is no job.
    let scratch = TempDir::new().expect("a scratch directory");
    let earlier = scratch.path().join("earlier");
    let later = scratch.path().join("later");
    std::fs::create_dir_all(&earlier).expect("the directory");
    std::fs::create_dir_all(&later).expect("the directory");
    let out = compared(&earlier, &later);
    let printed = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        out.status.code(),
        Some(1),
        "the exit code is the verdict, and there was no measurement to sign off \
         on\n{printed}"
    );
    assert!(printed.contains("NEITHER CENSUS HOLDS A JOB"), "{printed}");
}

/// R1122 — TWO RECORDS FOR ONE JOB, OFF A REAL DIRECTORY.
///
/// The loader keys a compilation log by its stem, because a log IS a job's; a
/// restore record is one CACHE's, so a job that declares two writes two files
/// and they cannot both be `<job>.restored`. A loader still keying these by the
/// stem-as-job would hold whichever `read_dir` returned last — one restore's
/// price standing in for the other's, silently, which is the whole reason the
/// record carries a `cache` line at all.
#[test]
fn a_jobs_two_restore_records_are_loaded_as_two() {
    let scratch = TempDir::new().expect("a scratch directory");
    let directory = scratch.path();
    let write = |file: &str, cache: &str, path: &str, arrived: u64| {
        let mut out = restored::encode_job("unrun-tests");
        out.extend_from_slice(&restored::encode_cache(cache));
        for instrument in restored::INSTRUMENTS {
            out.extend_from_slice(&restored::encode_built_from(instrument, "0f0f0f"));
        }
        out.extend_from_slice(&restored::encode_at(restored::Side::Before, 1_000_000_000));
        out.extend_from_slice(&restored::encode_side(
            restored::Side::Before,
            path,
            &restored::Measurement::default(),
        ));
        out.extend_from_slice(&restored::encode_side(
            restored::Side::After,
            path,
            &restored::Measurement {
                entries: 1,
                bytes: arrived,
            },
        ));
        out.extend_from_slice(&restored::encode_at(restored::Side::After, 1_030_000_000));
        out.extend_from_slice(&restored::encode_exact(true));
        std::fs::write(directory.join(file), out).expect("the record");
    };
    write(
        "unrun-tests.cargo-home.restored",
        "Linux-cargo-unruntests-home-",
        "~/.cargo/registry",
        700_000_000,
    );
    write(
        "unrun-tests.build-directory.restored",
        "Linux-cargo-unrun-",
        "target",
        32_000_000_000,
    );
    record_a_compilation(&directory.join("unrun-tests.log"));

    let census = load(directory, &BTreeSet::new()).expect("the census loads");
    assert_eq!(census.restored.len(), 2, "{:?}", census.restored.keys());
    assert_eq!(
        census.started().len(),
        2,
        "and they reach the join as two restores rather than one: {:?}",
        census.started()
    );
    assert_eq!(
        census.started().get(&restored::Restore {
            job: "unrun-tests".to_string(),
            cache: "Linux-cargo-unrun-".to_string(),
        }),
        Some(&restored::Warmth::ExactHit {
            bytes: 32_000_000_000
        }),
        "the build directory's 32 GB, and not the registry's 0.7"
    );

    // AND A JOB DECLARED ABSENT TAKES BOTH OF ITS RECORDS WITH IT. The skip is
    // the gate's own job on a runner, whose build is still happening while it
    // judges — and a nicknamed file would slip past a check that compared the
    // stem to the job name.
    let absent: BTreeSet<String> = ["unrun-tests".to_string()].into_iter().collect();
    let without = load(directory, &absent).expect("the census loads");
    assert!(
        without.restored.is_empty() && without.jobs.is_empty(),
        "{:?}",
        without.restored.keys()
    );
}
