//! The controls for the collector: every way it may remove a record, and every
//! way it must refuse to.
//!
//! A COLLECTOR'S EVIDENCE CANNOT COME FROM THE TREE IT COLLECTS. Its first run
//! on this repository will remove whatever is over budget that day, which
//! proves it deleted something and nothing about WHICH something — and on a
//! fresh checkout, where the record directories do not exist at all, the same
//! run is a green line over an empty tree. So the rules are driven here, over
//! directories these cases build, where the answer is known before it is asked.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use scratch_budget::{collect, mib, normalise, plan, read_declaration, resolve, survey, Entry};

/// A record of a known size and age, without touching a disk. The plan is a
/// pure function of these three fields and nothing else.
fn record(name: &str, bytes: u64, seconds_old: u64) -> Entry {
    Entry {
        path: PathBuf::from(name),
        bytes,
        modified: SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000 - seconds_old),
    }
}

fn names(entries: &[Entry]) -> Vec<String> {
    entries
        .iter()
        .map(|entry| entry.path.display().to_string())
        .collect()
}

#[test]
fn the_oldest_records_go_until_the_directory_fits() {
    // Four 10-byte records, budget 25: the two oldest have to go, and no more
    // than that — a collector that emptied a directory to make room would be
    // cheaper to write and would throw away the history it exists to bound.
    let plan = plan(
        vec![
            record("d", 10, 1),
            record("a", 10, 400),
            record("c", 10, 100),
            record("b", 10, 300),
        ],
        25,
    );
    assert_eq!(names(&plan.remove), ["a", "b"], "{plan:?}");
    assert_eq!((plan.total, plan.kept), (40, 20), "{plan:?}");
    assert!(!plan.still_over, "{plan:?}");
}

#[test]
fn a_directory_inside_its_budget_loses_nothing() {
    // The state every one of these directories is in most of the time, and the
    // one a collector must be silent about. A rule that trimmed to a file COUNT
    // would delete here; a budget does not.
    let plan = plan(vec![record("a", 10, 400), record("b", 10, 1)], 1024);
    assert!(plan.remove.is_empty(), "{plan:?}");
    assert_eq!((plan.total, plan.kept), (20, 20), "{plan:?}");
    assert!(!plan.still_over, "{plan:?}");
}

#[test]
fn the_newest_record_survives_a_budget_it_cannot_fit_and_the_failure_is_said() {
    // A verification that writes a 50 MB log into a 32 MiB budget is the case
    // where obeying the number means deleting the log of the run printing the
    // line. The newest record is kept, and `still_over` is what stops that
    // being reported as a directory within its budget.
    let plan = plan(vec![record("old", 10, 400), record("huge", 500, 1)], 25);
    assert_eq!(names(&plan.remove), ["old"], "{plan:?}");
    assert_eq!(plan.kept, 500, "{plan:?}");
    assert!(plan.still_over, "{plan:?}");

    // AND WITH ONE RECORD THERE IS NOTHING TO REMOVE AT ALL — the same rule at
    // its edge, where a `len() - 1` written as `len()` would delete the only
    // file in the directory.
    let alone = plan_of_one();
    assert!(alone.remove.is_empty(), "{alone:?}");
    assert!(alone.still_over, "{alone:?}");
}

fn plan_of_one() -> scratch_budget::Plan {
    plan(vec![record("only", 500, 1)], 25)
}

#[test]
fn records_written_in_the_same_second_are_ordered_by_name() {
    // MEASURED SHAPE, NOT A HYPOTHETICAL: one run of
    // `scripts/check-side-workspaces.sh` writes eighteen verify logs, several
    // of them inside the same second, and `read_dir` hands them over in
    // whatever order the filesystem holds. Without the second key, two runs of
    // this collector over one directory would remove different sets.
    let one = plan(
        vec![record("b", 10, 5), record("a", 10, 5), record("c", 10, 5)],
        15,
    );
    let again = plan(
        vec![record("c", 10, 5), record("b", 10, 5), record("a", 10, 5)],
        15,
    );
    assert_eq!(names(&one.remove), ["a", "b"], "{one:?}");
    assert_eq!(names(&one.remove), names(&again.remove), "{again:?}");
}

/// A directory of records with known ages, on a real disk.
fn scratch(case: &str) -> PathBuf {
    // The process and the case in the name — this repository's own law about a
    // path built from the shared temp root (`tools/unowned-scratch`).
    let at = std::env::temp_dir().join(format!("scratch-budget-{}-{case}", std::process::id()));
    let _ = std::fs::remove_dir_all(&at);
    std::fs::create_dir_all(&at).expect("mkdir");
    at
}

fn write_record(at: &Path, name: &str, bytes: usize, seconds_old: u64) {
    let path = at.join(name);
    std::fs::write(&path, vec![b'x'; bytes]).expect("write record");
    // The mtime is SET rather than waited for: a case that slept to make one
    // file older than another would be a wait on a clock, which this
    // repository's `blind-waits` gate refuses, and it would take a second per
    // record.
    let when = std::fs::FileTimes::new()
        .set_modified(SystemTime::now() - Duration::from_secs(seconds_old));
    std::fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .expect("open record")
        .set_times(when)
        .expect("set mtime");
}

#[test]
fn a_real_directory_is_brought_inside_its_budget_and_a_dry_run_touches_nothing() {
    let at = scratch("collect");
    for (name, age) in [("old", 900), ("middle", 600), ("new", 1)] {
        write_record(&at, name, 1024 * 1024, age);
    }

    // THE DRY RUN FIRST, and over the same directory: a case that dry-ran a
    // fresh copy would not notice a dry run that deletes.
    let dry = collect(&at, 2 * 1024 * 1024, true).expect("survey");
    assert_eq!(dry.plan.remove.len(), 1, "{dry:?}");
    assert_eq!(dry.removed_files, 0, "a dry run removes nothing: {dry:?}");
    assert_eq!(dry.removed_bytes, 0, "{dry:?}");
    assert!(at.join("old").exists(), "and the file is still there");

    let done = collect(&at, 2 * 1024 * 1024, false).expect("collect");
    assert_eq!(done.removed_files, 1, "{done:?}");
    assert_eq!(done.removed_bytes, 1024 * 1024, "{done:?}");
    assert!(!at.join("old").exists(), "the oldest is gone");
    assert!(
        at.join("middle").exists() && at.join("new").exists(),
        "{done:?}"
    );

    // AND RUNNING IT AGAIN IS A NO-OP, which is what makes it safe to call from
    // every verification: the second run finds the directory inside its budget
    // and removes nothing.
    let again = collect(&at, 2 * 1024 * 1024, false).expect("collect");
    assert_eq!(again.removed_files, 0, "{again:?}");
    let _ = std::fs::remove_dir_all(&at);
}

#[test]
fn the_survey_reads_every_depth_and_says_what_it_would_not_follow() {
    let at = scratch("survey");
    std::fs::create_dir_all(at.join("nested/deeper")).expect("mkdir");
    write_record(&at, "top", 10, 1);
    write_record(&at.join("nested"), "one-down", 10, 1);
    write_record(&at.join("nested/deeper"), "two-down", 10, 1);
    // A LINK IS COUNTED AND NEVER FOLLOWED. Following one would let a directory
    // of records reach outside itself, which is the one way this collector
    // could delete a file nobody declared.
    std::os::unix::fs::symlink("/etc/hostname", at.join("elsewhere")).expect("symlink");

    let found = survey(&at).expect("survey");
    assert_eq!(found.entries.len(), 3, "every depth: {found:?}");
    assert_eq!(found.bytes(), 30, "{found:?}");
    assert_eq!(found.links, 1, "{found:?}");
    assert!(found.unreadable.is_empty(), "{found:?}");

    // And a link is not removable either, whatever the budget says.
    let done = collect(&at, 0, false).expect("collect");
    assert_eq!(done.removed_files, 2, "the newest survives: {done:?}");
    assert!(at.join("elsewhere").exists(), "the link is untouched");
    let _ = std::fs::remove_dir_all(&at);

    // A DIRECTORY THAT DOES NOT EXIST IS NOT AN ERROR: it is the state of every
    // one of these on a fresh checkout and on every CI runner, and a collector
    // that refused there would refuse on every clean machine.
    let absent = survey(&at).expect("an unwritten directory surveys as empty");
    assert!(absent.entries.is_empty(), "{absent:?}");
}

#[test]
fn a_directory_outside_a_build_directory_is_refused_however_it_is_spelled() {
    let tree = Path::new("/home/example/repo");
    for inside in [
        "target/verify-logs",
        // AT ANY DEPTH: three of this repository's sweeps write their records
        // beside the separate workspace they test, and the law in
        // `tests/declared.rs` found them by refusing to be told otherwise.
        "tools/injection-harness/target/self-check-logs",
    ] {
        assert_eq!(
            resolve(tree, Path::new(inside)).expect("inside a build directory"),
            PathBuf::from(format!("/home/example/repo/{inside}"))
        );
    }

    // THE WAYS A DECLARATION COULD REACH SOMETHING THIS MAY NOT DELETE, and the
    // reason this is a guard rather than a convention: everything below it
    // deletes.
    for (escape, says) in [
        // Climbing out with `..` — the spelling every sweep manifest in this
        // repository uses for its own log directory, so it is not exotic.
        ("target/../../elsewhere", "outside"),
        // Somewhere else in the tree entirely: a build directory is the only
        // thing here that is rebuilt from source, so it is the only place a
        // lost file is cheap.
        ("crates/mnemosyne-cli/src", "not inside any build directory"),
        // An absolute path, which joins away the tree completely.
        ("/etc", "outside"),
        // A NEAR MISS, because a `starts_with` over strings would pass this and
        // collect a sibling directory: `target-old` is not `target`.
        ("target-old/logs", "not inside any build directory"),
        // AND THE ONE THAT WOULD COST THE MOST: the build directory itself,
        // which is `rm -rf target` written as data.
        ("target", "a build directory itself"),
        ("tools/injection-harness/target", "a build directory itself"),
    ] {
        let why = resolve(tree, Path::new(escape)).expect_err("refused");
        assert!(
            why.contains(says) && why.contains(escape),
            "the refusal names what was declared and why: {why}"
        );
    }
}

#[test]
fn normalising_a_path_resolves_the_dots_and_invents_nothing() {
    // BOTH SIDES OF THE LAW'S COMPARISON COME THROUGH THIS, and every log
    // directory this repository declares is spelled with `..`
    // (`../../target/injection-logs`), so a comparison of unnormalised paths
    // would find no match and report every directory as uncollected.
    assert_eq!(
        normalise(Path::new("/repo/tools/ci-plan/../../target/injection-logs")),
        PathBuf::from("/repo/target/injection-logs")
    );
    assert_eq!(
        normalise(Path::new("/repo/./target/./verify-logs")),
        PathBuf::from("/repo/target/verify-logs")
    );
    // A `..` WITH NOTHING TO UNDO IS KEPT rather than dropped: dropping it
    // would turn a path that reaches above its base into one that does not, and
    // the guard above would then approve it.
    assert_eq!(normalise(Path::new("../out")), PathBuf::from("../out"));
    assert_eq!(normalise(Path::new("/../out")), PathBuf::from("/../out"));
}

#[test]
fn this_repositorys_own_declaration_reads_and_every_entry_is_collectable() {
    // The declaration is data, and data that does not parse is a collector that
    // refuses on every run. Nothing else in the suite reads the real file.
    let declaration =
        read_declaration(&scratch_budget::declaration_path()).expect("the declaration reads");
    assert!(
        declaration.directories.len() >= 6,
        "this repository writes records into six directories — three shared ones \
         under the root build directory and three beside the separate workspaces \
         whose sweeps write them: {declaration:?}"
    );
    assert!(
        !declaration.prose.is_empty(),
        "the policy is stated in the file that carries it: {declaration:?}"
    );
    let tree = Path::new("/anywhere");
    for entry in &declaration.directories {
        resolve(tree, &entry.path).expect("a declared directory is under the tree's target");
        assert!(
            entry.budget_mib > 0,
            "a budget of zero would keep exactly one record: {entry:?}"
        );
        // EVERY BUDGET CARRIES ITS MEASUREMENT. A number without one is a
        // number somebody typed, and the next reader cannot tell the two apart.
        assert!(
            entry.why.contains("MEASURED"),
            "{} declares a budget with no measurement behind it: {}",
            entry.path.display(),
            entry.why
        );
    }
    assert_eq!(mib(3 * 1024 * 1024 / 2), "1.5 MiB");
}
