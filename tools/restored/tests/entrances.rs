//! THE PROGRAM A WORKFLOW RUNS, RUN AS A PROCESS.
//!
//! This binary has one job and it is done in two invocations with a cache
//! restore between them, so the thing that can go wrong is not arithmetic — it
//! is the wiring: an environment variable a step forgot, a second step that ran
//! without the first, an output read as `false` when it was not there at all.
//! Round 1096 measured what a function-level test misses here: the census gate's
//! `main.rs` held both of its entrances and had no test of any kind, because
//! nobody ran the binary.
//!
//! So the tree is real, the restore is simulated by files ARRIVING between the
//! two invocations — which is what a restore is — and the assertions are the
//! exit code and the record the process left behind.

use std::path::Path;
use std::process::{Command, Output};

use restored::{decode, Warmth};
use tempfile::TempDir;

/// Which cache the fixture's steps bracket — the resolved key prefix, the
/// identity `ci_plan` derives from the workflow and every gate joins on.
const A_CACHE: &str = "Linux-cargo-unrun-";

/// One invocation of the binary, with the environment a workflow step gives it.
struct Step {
    output: Output,
}

impl Step {
    fn stdout(&self) -> String {
        String::from_utf8_lossy(&self.output.stdout).into_owned()
    }

    fn stderr(&self) -> String {
        String::from_utf8_lossy(&self.output.stderr).into_owned()
    }

    fn transcript(&self) -> String {
        format!(
            "exit {:?}\n--- stdout ---\n{}\n--- stderr ---\n{}",
            self.output.status.code(),
            self.stdout(),
            self.stderr()
        )
    }

    fn code(&self) -> i32 {
        self.output.status.code().expect("the process exited")
    }
}

/// The environment a job gives this program, one variable at a time so a test
/// can take exactly one away.
struct Wiring<'a> {
    home: &'a Path,
    record: Option<String>,
    job: Option<&'a str>,
    exact: Option<&'a str>,
}

impl<'a> Wiring<'a> {
    fn wired(home: &'a Path, record: &Path) -> Wiring<'a> {
        Wiring {
            home,
            record: Some(record.display().to_string()),
            job: Some("unrun-tests"),
            exact: Some("false"),
        }
    }

    fn run(&self, at: &Path, arguments: &[&str]) -> Step {
        let mut command = Command::new(env!("CARGO_BIN_EXE_restored"));
        command.args(arguments).current_dir(at);
        command.env("HOME", self.home);
        // EVERY VARIABLE THIS PROGRAM READS IS SET HERE OR DELIBERATELY NOT, so
        // a test that removes one is removing it from a known state rather than
        // from whatever the machine running the suite happens to export.
        command.env_remove(restored::VARIABLE);
        command.env_remove(restored::EXACT_VARIABLE);
        command.env_remove("GITHUB_JOB");
        if let Some(record) = &self.record {
            command.env(restored::VARIABLE, record);
        }
        if let Some(job) = self.job {
            command.env("GITHUB_JOB", job);
        }
        if let Some(exact) = self.exact {
            command.env(restored::EXACT_VARIABLE, exact);
        }
        Step {
            output: command.output().expect("the program runs"),
        }
    }
}

/// A workspace with a `target` and a `~/.cargo/registry`, both empty, as a
/// runner's is when the steps before the cache have finished.
fn workspace() -> (TempDir, TempDir) {
    let home = tempfile::tempdir().expect("a home");
    let tree = tempfile::tempdir().expect("a workspace");
    (home, tree)
}

/// What a restore does: files appear under a path that had none.
fn restore(root: &Path, under: &str, files: usize, bytes: usize) {
    let directory = root.join(under).join("deps");
    std::fs::create_dir_all(&directory).expect("the restored directory");
    for index in 0..files {
        std::fs::write(directory.join(format!("unit-{index}.rlib")), vec![7; bytes])
            .expect("a restored file");
    }
}

/// Run both steps around a restore, and read back the record.
fn around(
    exact: &str,
    restoring: impl FnOnce(&Path, &Path),
) -> (restored::Restored, Step, Step, TempDir, TempDir) {
    let (home, tree) = workspace();
    let record = tree.path().join("rustc-log/unrun-tests.restored");
    let mut wiring = Wiring::wired(home.path(), &record);
    let before = wiring.run(
        tree.path(),
        &[
            "before",
            "--cache",
            A_CACHE,
            "~/.cargo/registry",
            "~/.cargo/git",
            "target",
        ],
    );
    assert_eq!(before.code(), 0, "{}", before.transcript());
    restoring(home.path(), tree.path());
    wiring.exact = Some(exact);
    let after = wiring.run(tree.path(), &["after"]);
    assert_eq!(after.code(), 0, "{}", after.transcript());
    let text = std::fs::read_to_string(&record).expect("the record");
    let whole = decode(&text).unwrap_or_else(|why| panic!("{why}\n{}", after.transcript()));
    (whole, before, after, home, tree)
}

/// THE STATE THAT WAS INVISIBLE, end to end: the primary key missed, and a
/// previous generation arrived anyway.
#[test]
fn a_missed_key_with_a_tree_arriving_is_recorded_as_a_prefix_hit() {
    let (record, _, after, _home, _tree) = around("false", |home, tree| {
        restore(home, ".cargo/registry", 4, 1_000);
        restore(tree, "target", 6, 2_000);
    });
    assert_eq!(record.job, "unrun-tests");
    assert!(!record.exact);
    assert_eq!(record.bytes_restored(), 4 * 1_000 + 6 * 2_000);
    assert_eq!(
        record.warmth(),
        Warmth::PrefixHit {
            bytes: 4 * 1_000 + 6 * 2_000
        }
    );
    // AND THE JOB'S OWN LOG SAYS IT, because the state a census was taken under
    // has to be readable in the run that took it.
    assert!(
        after.stdout().contains("no exact hit"),
        "{}",
        after.transcript()
    );
}

/// The control for the test above: the same wiring, the same `cache-hit`, and
/// nothing arriving.
#[test]
fn a_missed_key_with_nothing_arriving_is_recorded_as_nothing() {
    let (record, _, after, _home, _tree) = around("false", |_, _| {});
    assert_eq!(record.bytes_restored(), 0);
    assert_eq!(record.warmth(), Warmth::Nothing);
    assert!(
        after.stdout().contains("empty tree"),
        "{}",
        after.transcript()
    );
}

#[test]
fn an_exact_hit_with_a_tree_arriving_is_recorded_as_one() {
    let (record, _, _, _home, _tree) = around("true", |home, _| {
        restore(home, ".cargo/registry", 2, 500);
    });
    assert!(record.exact);
    assert_eq!(record.warmth(), Warmth::ExactHit { bytes: 1_000 });
}

/// The two instruments disagreeing travels as its own state rather than as one
/// of the ordinary three.
#[test]
fn an_exact_hit_with_nothing_arriving_is_recorded_as_the_contradiction() {
    let (record, _, after, _home, _tree) = around("true", |_, _| {});
    assert_eq!(record.warmth(), Warmth::HitThatBroughtNothing);
    assert!(
        after.stdout().contains("no bytes arrived"),
        "{}",
        after.transcript()
    );
}

/// WHAT WAS ALREADY THERE IS NOT WHAT THE RESTORE BROUGHT — measured through the
/// program, over a directory that is non-empty before the cache step runs.
#[test]
fn what_the_steps_before_the_cache_left_behind_is_not_counted_as_restored() {
    let (home, tree) = workspace();
    // The recorder's own build ran before the cache step and left this.
    restore(home.path(), ".cargo/registry", 3, 4_096);
    let record = tree.path().join("rustc-log/unrun-tests.restored");
    let wiring = Wiring::wired(home.path(), &record);
    let before = wiring.run(
        tree.path(),
        &["before", "--cache", A_CACHE, "~/.cargo/registry", "target"],
    );
    assert_eq!(before.code(), 0, "{}", before.transcript());
    let after = wiring.run(tree.path(), &["after"]);
    assert_eq!(after.code(), 0, "{}", after.transcript());
    let whole = decode(&std::fs::read_to_string(&record).expect("the record")).expect("decodes");
    assert_eq!(whole.paths[0].before.bytes, 3 * 4_096);
    assert_eq!(whole.bytes_restored(), 0);
    assert_eq!(whole.warmth(), Warmth::Nothing);
}

/// The second step re-measures what the first wrote down, and is given no path
/// of its own — one spelling of the list, so the difference cannot be between
/// two different things.
#[test]
fn the_second_step_measures_the_paths_the_first_one_named() {
    let (record, _, _, _home, _tree) = around("false", |_, _| {});
    assert_eq!(
        record.measured(),
        vec!["~/.cargo/registry", "~/.cargo/git", "target"]
    );
}

/// A path that does not exist is zero rather than a failure: `~/.cargo/git` is
/// absent until something depends on a git checkout.
#[test]
fn a_cached_path_that_does_not_exist_is_measured_as_nothing_present() {
    let (record, _, _, _home, _tree) = around("false", |_, _| {});
    let git = record
        .paths
        .iter()
        .find(|one| one.path == "~/.cargo/git")
        .expect("the declared path is in the record");
    assert_eq!(git.before, restored::Measurement::default());
    assert_eq!(git.after, restored::Measurement::default());
}

#[test]
fn the_first_step_truncates_whatever_was_there_before_it() {
    let (home, tree) = workspace();
    let record = tree.path().join("rustc-log/unrun-tests.restored");
    std::fs::create_dir_all(record.parent().expect("a parent")).expect("the directory");
    std::fs::write(&record, "job\u{1f}somebody-else\n").expect("a stale record");
    let wiring = Wiring::wired(home.path(), &record);
    let before = wiring.run(tree.path(), &["before", "--cache", A_CACHE, "target"]);
    assert_eq!(before.code(), 0, "{}", before.transcript());
    let after = wiring.run(tree.path(), &["after"]);
    assert_eq!(after.code(), 0, "{}", after.transcript());
    let whole = decode(&std::fs::read_to_string(&record).expect("the record")).expect("decodes");
    assert_eq!(whole.job, "unrun-tests");
}

// --- the wiring, one variable at a time -------------------------------------

#[test]
fn without_the_record_variable_it_refuses_rather_than_choosing_a_path() {
    let (home, tree) = workspace();
    let mut wiring = Wiring::wired(home.path(), Path::new("unused"));
    wiring.record = None;
    let step = wiring.run(tree.path(), &["before", "--cache", A_CACHE, "target"]);
    assert_eq!(step.code(), 1, "{}", step.transcript());
    assert!(
        step.stderr().contains(restored::VARIABLE),
        "{}",
        step.transcript()
    );
}

#[test]
fn without_the_job_it_refuses_rather_than_reading_the_name_off_its_own_file() {
    let (home, tree) = workspace();
    let record = tree.path().join("rustc-log/unrun-tests.restored");
    let mut wiring = Wiring::wired(home.path(), &record);
    wiring.job = None;
    let step = wiring.run(tree.path(), &["before", "--cache", A_CACHE, "target"]);
    assert_eq!(step.code(), 1, "{}", step.transcript());
    assert!(
        step.stderr().contains("GITHUB_JOB"),
        "{}",
        step.transcript()
    );
}

/// THE ONE THAT MATTERS MOST. `cache-hit` is the only thing that can tell an
/// exact hit from a prefix hit, and reading its absence as `false` would report
/// every warm job as the state this record exists to distinguish.
#[test]
fn without_the_cache_hit_output_it_refuses_rather_than_assuming_a_miss() {
    let (home, tree) = workspace();
    let record = tree.path().join("rustc-log/unrun-tests.restored");
    let mut wiring = Wiring::wired(home.path(), &record);
    let before = wiring.run(tree.path(), &["before", "--cache", A_CACHE, "target"]);
    assert_eq!(before.code(), 0, "{}", before.transcript());
    wiring.exact = None;
    let after = wiring.run(tree.path(), &["after"]);
    assert_eq!(after.code(), 1, "{}", after.transcript());
    assert!(
        after.stderr().contains(restored::EXACT_VARIABLE),
        "{}",
        after.transcript()
    );
}

#[test]
fn a_cache_hit_that_is_not_a_boolean_is_refused_by_the_step_that_reads_it() {
    let (home, tree) = workspace();
    let record = tree.path().join("rustc-log/unrun-tests.restored");
    let mut wiring = Wiring::wired(home.path(), &record);
    let before = wiring.run(tree.path(), &["before", "--cache", A_CACHE, "target"]);
    assert_eq!(before.code(), 0, "{}", before.transcript());
    wiring.exact = Some("maybe");
    let after = wiring.run(tree.path(), &["after"]);
    assert_eq!(after.code(), 1, "{}", after.transcript());
}

#[test]
fn nothing_matched_at_all_is_the_actions_third_answer_and_not_a_refusal() {
    // `actions/cache` SETS `cache-hit` TO THE EMPTY STRING WHEN NOTHING MATCHED
    // — `false` means a `restore-keys` prefix matched, which is a different
    // state and the one R1099 misread. Until R1112 moved every key this
    // repository had never had a fully cold run, so this answer had never
    // arrived and the step refused it; three jobs of that run died here.
    let (home, tree) = workspace();
    let record = tree.path().join("rustc-log/unrun-tests.restored");
    let mut wiring = Wiring::wired(home.path(), &record);
    let before = wiring.run(tree.path(), &["before", "--cache", A_CACHE, "target"]);
    assert_eq!(before.code(), 0, "{}", before.transcript());
    wiring.exact = Some("");
    let after = wiring.run(tree.path(), &["after"]);
    assert_eq!(after.code(), 0, "{}", after.transcript());
    let written = std::fs::read_to_string(&record).expect("the record");
    let whole = restored::decode(&written).expect("a cold record decodes");
    assert!(!whole.exact);
    assert_eq!(whole.warmth(), restored::Warmth::Nothing);
}

#[test]
fn nothing_matched_while_something_arrived_is_two_steps_reading_two_caches() {
    // THE CONTRADICTION THE EMPTY VALUE COULD OTHERWISE HIDE. An empty
    // `cache-hit` is also what a step reading the WRONG step's output sees, and
    // the two are told apart by the disk rather than by the string: nothing
    // matched means nothing arrived.
    let (home, tree) = workspace();
    let record = tree.path().join("rustc-log/unrun-tests.restored");
    let mut wiring = Wiring::wired(home.path(), &record);
    let before = wiring.run(tree.path(), &["before", "--cache", A_CACHE, "target"]);
    assert_eq!(before.code(), 0, "{}", before.transcript());
    // A restore happening between the two readings, which is what the cache step
    // does when it matches something.
    std::fs::create_dir_all(tree.path().join("target/debug")).expect("the tree");
    std::fs::write(
        tree.path().join("target/debug/libserde.rlib"),
        vec![0_u8; 4_096],
    )
    .expect("what the cache brought");
    wiring.exact = Some("");
    let after = wiring.run(tree.path(), &["after"]);
    assert_eq!(after.code(), 1, "{}", after.transcript());
    assert!(
        after.stderr().contains("reading a different step"),
        "{}",
        after.transcript()
    );
}

/// R1117 — a record has to say WHICH cache it is of, and the step that opens it
/// is the only thing that knows. A job may declare more than one; a record that
/// did not name its own would be matched to whichever the reader picked, and the
/// price of a 32 GB build directory would be read off a 756 MB registry.
#[test]
fn the_first_step_without_a_cache_to_name_refuses() {
    let (home, tree) = workspace();
    let record = tree.path().join("rustc-log/unrun-tests.restored");
    let wiring = Wiring::wired(home.path(), &record);

    let unnamed = wiring.run(tree.path(), &["before", "target"]);
    assert_eq!(unnamed.code(), 1, "{}", unnamed.transcript());
    assert!(
        unnamed.stderr().contains("--cache"),
        "and it says what is missing: {}",
        unnamed.transcript()
    );

    let empty = wiring.run(tree.path(), &["before", "--cache", "", "target"]);
    assert_eq!(
        empty.code(),
        1,
        "an empty name is not a name — it decodes as a record whose cache \
         matches no declaration, one step later and further from the mistake: {}",
        empty.transcript()
    );
}

/// The second step without the first is a job whose cache step is wired above
/// the measurement rather than between the two — silent, and it would report
/// every job as cold.
#[test]
fn the_second_step_without_the_first_refuses() {
    let (home, tree) = workspace();
    let record = tree.path().join("rustc-log/unrun-tests.restored");
    let wiring = Wiring::wired(home.path(), &record);
    let after = wiring.run(tree.path(), &["after"]);
    assert_eq!(after.code(), 1, "{}", after.transcript());
    assert!(
        after.stderr().contains("did not run"),
        "{}",
        after.transcript()
    );
}

#[test]
fn the_first_step_with_no_path_refuses() {
    let (home, tree) = workspace();
    let record = tree.path().join("rustc-log/unrun-tests.restored");
    let wiring = Wiring::wired(home.path(), &record);
    let step = wiring.run(tree.path(), &["before"]);
    assert_eq!(step.code(), 1, "{}", step.transcript());
}

#[test]
fn a_word_this_program_does_not_have_is_refused_rather_than_defaulted() {
    let (home, tree) = workspace();
    let record = tree.path().join("rustc-log/unrun-tests.restored");
    let wiring = Wiring::wired(home.path(), &record);
    let step = wiring.run(tree.path(), &["during", "target"]);
    assert_eq!(step.code(), 1, "{}", step.transcript());
    assert!(step.stderr().contains("usage"), "{}", step.transcript());
}

// --- the walk ---------------------------------------------------------------

/// A `target` directory is nested and holds symlinks; the walk reaches the one
/// and does not follow the other.
#[test]
fn the_walk_reaches_every_depth_and_does_not_follow_a_symlink() {
    let tree = tempfile::tempdir().expect("a tree");
    let deep = tree.path().join("a/b/c");
    std::fs::create_dir_all(&deep).expect("the depth");
    // TWO FILES UNDER THE SYMLINK'S TARGET AND ONE OUTSIDE IT, so that walking
    // through the link is a DIFFERENT COUNT and not merely a different path. An
    // earlier spelling of this test put one file each side, and following the
    // link then counted the same total by coincidence — an injection that made
    // the walk follow symlinks came back green against it.
    std::fs::write(deep.join("one"), vec![1; 30]).expect("a file");
    std::fs::write(deep.join("two"), vec![1; 30]).expect("a file");
    std::fs::write(tree.path().join("outside"), vec![1; 12]).expect("a file");
    #[cfg(unix)]
    std::os::unix::fs::symlink(tree.path().join("a"), tree.path().join("loop"))
        .expect("a symlink back into the tree");
    let measured = restored::measure(tree.path()).expect("the walk finishes");
    assert_eq!(
        measured.entries,
        if cfg!(unix) { 4 } else { 3 },
        "the three files, and on unix the symlink itself as one entry rather \
         than as the two files it points at: {measured:?}"
    );
    assert!(
        measured.bytes >= 72,
        "the files themselves are counted at every depth: {measured:?}"
    );
}

#[test]
fn a_path_that_is_not_there_measures_as_nothing_rather_than_failing() {
    let tree = tempfile::tempdir().expect("a tree");
    let measured =
        restored::measure(&tree.path().join("never-made")).expect("absent is not an error");
    assert_eq!(measured, restored::Measurement::default());
}
