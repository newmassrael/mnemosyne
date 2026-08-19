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
    /// Whether this machine's own `PATH` follows the stub's directory.
    ///
    /// OFF FOR EVERY CASE BUT ONE, because `PATH` being the stub's directory and
    /// NOTHING ELSE is what lets a case produce "gh is not installed" rather than
    /// assert about it in prose. The one case that turns it on is about the budget
    /// block, which is read out of the tracked workflow files — and asking which
    /// files a repository tracks means running `git`, a program that case cannot
    /// stub without stubbing the answer it is checking.
    beside_the_machines_programs: bool,
}

impl Stub {
    /// A `gh` that answers with the recorded bodies.
    fn recording() -> Self {
        let stub = Stub {
            dir: TempDir::new().expect("tempdir"),
            beside_the_machines_programs: false,
        };
        link_gh(&stub.dir.path().join("gh"), env!("CARGO_BIN_EXE_gh-stub"));
        stub
    }

    /// The same, with this machine's `PATH` behind the stub's directory — see
    /// [`Stub::beside_the_machines_programs`]. `gh` still resolves to the stub,
    /// because the stub's directory comes first.
    fn recording_beside_git() -> Self {
        let mut stub = Self::recording();
        stub.beside_the_machines_programs = true;
        stub
    }

    /// A `gh` that is installed and fails.
    fn unreachable() -> Self {
        let stub = Stub {
            dir: TempDir::new().expect("tempdir"),
            beside_the_machines_programs: false,
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
            beside_the_machines_programs: false,
        }
    }

    /// What the reporter's `PATH` is, for this stub.
    fn path(&self) -> String {
        let mine = self.dir.path().display().to_string();
        if self.beside_the_machines_programs {
            format!("{mine}:{}", std::env::var("PATH").unwrap_or_default())
        } else {
            mine
        }
    }

    fn log(&self) -> PathBuf {
        self.dir.path().join("gh-contract-violations.log")
    }

    /// Run the reporter over a named job's recorded steps.
    fn run_with_job(&self, arguments: &[&str], checks: &Path, job: &str, body: &Path) -> Output {
        self.invoke(arguments, checks, &fixture("annotations.json"), job, body)
    }

    /// Run the reporter with a named annotations recording — one case is about
    /// what an annotation SAYS rather than about the checks that carry it.
    fn run_with_annotations(&self, arguments: &[&str], checks: &Path, said: &Path) -> Output {
        self.invoke(
            arguments,
            checks,
            said,
            "93478488570",
            &fixture("job.failure.json"),
        )
    }

    /// Run the reporter with this stub as the whole of `PATH`.
    fn run(&self, arguments: &[&str], checks: &Path) -> Output {
        self.invoke(
            arguments,
            checks,
            &fixture("annotations.json"),
            "93478488570",
            &fixture("job.failure.json"),
        )
    }

    fn invoke(
        &self,
        arguments: &[&str],
        checks: &Path,
        annotations: &Path,
        job: &str,
        body: &Path,
    ) -> Output {
        let out = Command::new(env!("CARGO_BIN_EXE_ci-state"))
            .args(arguments)
            .current_dir(self.dir.path())
            // THE WHOLE OF `PATH`, so "gh is not installed" is a state this test
            // can actually produce rather than one it asserts about in prose. One
            // case widens it, and says why where the flag is declared.
            .env("PATH", self.path())
            .env("GH_STUB_LOG", self.log())
            .env("GH_STUB_SHA", SHA)
            .env("GH_STUB_CHECK", "93478488570")
            .env("GH_STUB_CHECKS", checks)
            .env("GH_STUB_ANNOTATIONS", annotations)
            // R1236 — the steps of the one job that failed on this commit. The
            // stub REFUSES a request for any other job, so every case is also
            // asserting that this reporter asks about the job the failing check
            // names and about no other.
            .env("GH_STUB_JOB", job)
            .env("GH_STUB_JOB_BODY", body)
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
    assert!(
        said.contains("stopped at step 6 `every cache this repository declares is one it keeps`"),
        "and the STEP that ended that job is named, which is the fact no check-run \
         row carries (R1236): {said}"
    );
    assert!(
        said.contains("from 1 check(s): every cache declared is one CI keeps"),
        "and the CHECK that said the annotation is named (R1238) — the loop that \
         fetches them knows which one it asked, and used to throw the name away \
         one line later: {said}"
    );
}

/// A run a later push retired reads, through this binary, as NO VERDICT.
///
/// THE HALF THE LIBRARY LAWS CANNOT REACH. `report` is told which checks were
/// superseded; whether the BINARY works that out — fetching the annotations before
/// it phrases the census, and asking `superseded_checks` — is a decision that lives
/// in `main.rs`, and this crate's own rule (R1096) is that such a decision has no
/// reader until something runs the process. Without this case every library law
/// here passes while the binary goes on printing `is RED`.
///
/// THE CHECKS ARE THE RECORDED ONES WITH ONE WORD CHANGED, and the count is
/// asserted so the change cannot silently apply to nothing: `cancelled` is what
/// GitHub writes for a job it retired, and the recording's one failing check
/// becomes that. The annotation beside it is GitHub's own sentence, copied from the
/// run this round was written after (`74035d7`, 2026-08-19).
#[test]
fn a_run_a_later_push_retired_reads_as_no_verdict_through_this_binary() {
    let recorded =
        fs::read_to_string(fixture("check-runs.one-page.json")).expect("the recorded checks");
    let failure = "\"conclusion\":\"failure\"";
    assert_eq!(
        recorded.matches(failure).count(),
        1,
        "this case rewrites the ONE failing conclusion in the recording; a count \
         that is not one means the edit applied to something else, or to nothing"
    );
    let stub = Stub::recording();
    let checks = stub.dir.path().join("check-runs.superseded.json");
    fs::write(
        &checks,
        recorded.replace(failure, "\"conclusion\":\"cancelled\""),
    )
    .expect("the rewritten recording, which is DATA and not a program (R1192)");

    let out = stub.run_with_annotations(&[SHA], &checks, &fixture("annotations.superseded.json"));
    let said = said(&out);
    assert_eq!(
        out.status.code(),
        Some(0),
        "reporting is not failing: {said}"
    );
    assert!(
        said.contains("NO VERDICT") && said.contains("LATER PUSH"),
        "the binary has to work out for itself that the run was retired — the \
         library is told, and nothing else here would notice if main stopped \
         asking:\n{said}"
    );
    assert!(
        !said.contains("is RED"),
        "and it must not say both: a reader acts on one sentence:\n{said}"
    );
    assert!(
        said.contains("(a later push superseded this run)"),
        "and the row itself carries the reason, beside the conclusion that cannot \
         say it:\n{said}"
    );
    assert!(
        !said.contains("stopped at step") && said.contains("not asking where 1 retired check(s)"),
        "and where a RETIRED job got to is not printed — it is true and it is not \
         about this commit, and a reader who sees `stopped at step 6` under a \
         cancelled row reads a diagnosis. What is NOT asked is counted rather than \
         dropped:\n{said}"
    );
}

/// What a job COST, held against what its workflow allows, out of this binary.
///
/// THE JOIN LIVES IN `main.rs` AND NOWHERE ELSE (R1096): the library can be handed
/// checks and budgets and every law about it passes while the binary never asks
/// `ci-plan` for a budget at all. What that would look like is exactly what this
/// reporter printed before this round — a census, annotations, and no idea what
/// anything cost.
///
/// THE WORKFLOW IS WRITTEN AND TRACKED HERE, because the budgets are read out of
/// what a repository TRACKS. `validate` in the recording ran 13:40:53 to 14:00:50,
/// which is 19m57s; against the 90 minutes this fixture declares that is 22%.
#[test]
fn what_a_job_cost_is_held_against_its_budget_through_this_binary() {
    let stub = Stub::recording_beside_git();
    let tree = stub.dir.path();
    fs::create_dir_all(tree.join(".github/workflows")).expect("the workflow directory");
    fs::write(
        tree.join(".github/workflows/recorded.yml"),
        "name: recorded\non: push\njobs:\n  validate:\n    runs-on: ubuntu-latest\n\
         \x20   timeout-minutes: 90\n    steps:\n      - run: 'true'\n",
    )
    .expect("the workflow");
    for argv in [
        vec!["init", "-q", "."],
        vec!["config", "user.email", "ci-state@test"],
        vec!["config", "user.name", "ci-state test"],
        vec!["add", "-A"],
    ] {
        let out = Command::new("git")
            .args(&argv)
            .current_dir(tree)
            .output()
            .expect("git, which is how a repository is asked what it tracks");
        assert!(out.status.success(), "git {argv:?}: {out:?}");
    }

    let out = stub.run(&[SHA], &fixture("check-runs.one-page.json"));
    let said = said(&out);
    assert_eq!(
        out.status.code(),
        Some(0),
        "reporting is not failing: {said}"
    );
    assert!(
        said.contains("the closest to its budget was `validate` — 19m57s of 90m (22%)"),
        "the binary has to ASK for the budget and join it to what the job took; \
         the library being able to do it is not the same fact:\n{said}"
    );
    assert!(
        said.contains("NOT MEASURED") && said.contains("no job of this repository"),
        "and the eight checks this one-job workflow does not declare are NAMED \
         rather than quietly left out of the count:\n{said}"
    );
}

/// The stalled job reads, through this binary, as one this repository never reached.
///
/// THE ROUND'S OWN RED, END TO END. On 2026-08-18 a job of this repository was
/// `cancelled` after forty-five minutes, and from the check-run rows that is
/// indistinguishable from the push's own change hanging it — the reading this
/// session started with, and abandoned only after a SECOND tool was asked for the
/// steps. What comes out of this binary now is the whole attribution: the step was
/// `apt-get`, it ate the job, and the clippy, the suite and the wrapper behind it
/// never ran.
#[test]
fn the_job_that_stalled_before_anything_of_ours_ran_says_so_through_this_binary() {
    let stub = Stub::recording();
    let checks = stub.dir.path().join("cancelled.json");
    // The recorded rows, with the one failing check turned into the cancelled job
    // recorded beside it. THE CHECK ROW IS WHAT DRIFTS AND THE JOB BODY IS REAL:
    // GitHub answers about a commit and about a job at two endpoints, and this
    // case is about the reporter joining them, so only the join is composed here.
    let rows = fs::read_to_string(fixture("check-runs.one-page.json")).expect("the recording");
    fs::write(&checks, rows.replace("\"failure\"", "\"cancelled\"")).expect("write");
    let out = stub.run_with_job(
        &[SHA],
        &checks,
        "93478488570",
        &fixture("job.cancelled.json"),
    );
    let said = said(&out);
    assert_eq!(out.status.code(), Some(0), "{said}");
    assert!(
        said.contains("stopped at step 3 `Install protoc (mnemosyne-server build script)`"),
        "the step that ate the job is named: {said}"
    );
    assert!(
        said.contains("7 of the 10 step(s) after it never ran"),
        "and what never ran is counted, which is what says the change this push \
         carried was never reached: {said}"
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

/// A check no Actions job is behind is SAID to have none, and is not asked about.
///
/// WHY THE JOB IS READ OUT OF `details_url` AND NOT TAKEN FROM `id`. For GitHub
/// Actions those two integers are equal — `github.rs` asserts it against the
/// recording — so on this repository's own data the two spellings are
/// indistinguishable, and a reporter written the easy way would look identical
/// until the day a check arrives that no Actions job is behind. Then `id` names a
/// job belonging to somebody else's numbering, and the steps of whatever comes
/// back would be printed as this check's.
///
/// THE STUB IS TOLD A JOB THIS CASE FORBIDS, so an ask of any kind is a logged
/// contract violation and `run_with_job` fails on it. "It printed the right
/// sentence" and "it never asked" are two claims, and this makes both.
#[test]
fn a_check_no_actions_job_is_behind_is_said_so_rather_than_asked_about() {
    let stub = Stub::recording();
    let checks = stub.dir.path().join("foreign.json");
    let rows = fs::read_to_string(fixture("check-runs.one-page.json")).expect("the recording");
    fs::write(
        &checks,
        rows.replace(
            "https://github.com/newmassrael/mnemosyne/actions/runs/31394095606/job/93478488570",
            "https://audit.example.com/reports/17",
        ),
    )
    .expect("write");
    let out = stub.run_with_job(&[SHA], &checks, "no-job-may-be-asked-about", &checks);
    let said = said(&out);
    assert_eq!(out.status.code(), Some(0), "{said}");
    assert!(
        said.contains("no Actions job behind `every cache declared is one CI keeps`"),
        "the check is named and so is the absence: {said}"
    );
    assert!(
        said.contains("https://audit.example.com/reports/17"),
        "and the reader is sent where the check itself points: {said}"
    );
    assert!(
        !said.contains("stopped at step"),
        "and no other job's steps are printed as this one's: {said}"
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
