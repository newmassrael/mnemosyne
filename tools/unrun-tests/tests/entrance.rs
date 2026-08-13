//! The three answers this gate gives, asked of the PROGRAM.
//!
//! `src/main.rs` says it in its own doc comment: the exit codes are three
//! answers and not two — `0` the law holds, `1` tests are dark, `2` the gate
//! could not judge — and one code for the last two mislabels whichever it did
//! not mean, "which is the failure R1078 shipped". That decision lived in
//! `main.rs` and NOTHING RAN THE BINARY, so the defect it names could be put
//! straight back: R1127 measured it by turning the refusal path into a clean
//! pass, and the whole suite stayed green. A decision whose only reader is the
//! process has to be read by running the process — R1096 for the sibling gate,
//! and the same answer here.
//!
//! THE FIXTURES ARE TREES AND NOT CALLS. What decides the verdict is the join
//! of two readings — what the tree compiles and what CI runs — and both come
//! from asking cargo and git about a real directory. So each case here is a
//! git repository holding one dependency-free crate, a workflow, and a stub
//! lister, and the only thing that varies between them is the cargo command
//! the workflow issues. Everything else being equal is what makes the exit
//! code attributable to that one line.

use std::path::Path;
use std::process::{Command, Output};

use unrun_tests::Refusal;

/// The test the fixture crate compiles and that a filtered command leaves out.
///
/// NAMED HERE BECAUSE THE ASSERTIONS READ IT BACK. A dark test is reported by
/// name, and a test that spelled the name a second time could drift from the
/// fixture into agreeing with anything.
const DARK_TEST: &str = "tests::not_run_by_ci";

/// The one the filtered command does run, which is what keeps the dark case
/// from being "CI runs nothing" — a different finding with a different code.
const RUN_TEST: &str = "tests::run_by_ci";

/// A git repository with one crate, one workflow, and a lister that answers.
///
/// TRACKED RATHER THAN MERELY WRITTEN: `ci_plan::workflow_files` asks
/// `git ls-files`, because a workflow nobody committed is not one CI will run.
/// So the fixture stages what it writes, and a file this forgot to add would
/// make the gate say the tree has no workflows at all.
fn tree(name: &str, ci: &str) -> std::path::PathBuf {
    let at = std::env::temp_dir().join(format!(
        "unrun-tests-entrance-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&at);
    std::fs::create_dir_all(at.join("src")).expect("fixture src");
    std::fs::create_dir_all(at.join("scripts")).expect("fixture scripts");
    std::fs::create_dir_all(at.join(".github/workflows")).expect("fixture workflows");

    write(
        &at.join("Cargo.toml"),
        "[workspace]\n\
         \n\
         [package]\n\
         name = \"fixture\"\n\
         version = \"0.1.0\"\n\
         edition = \"2021\"\n",
    );
    write(
        &at.join("src/lib.rs"),
        "#[cfg(test)]\n\
         mod tests {\n\
         \x20   #[test]\n\
         \x20   fn run_by_ci() {}\n\
         \n\
         \x20   #[test]\n\
         \x20   fn not_run_by_ci() {}\n\
         }\n",
    );
    // THE LISTER'S SIDE OF THE CONTRACT, and no more of it. `ci_plan::workspaces`
    // refuses a lister that names nothing, because in the repository this gate
    // guards an empty answer means the reading failed. One SKIP satisfies that
    // without adding a second workspace for the probe to build.
    write(
        &at.join("scripts/check-side-workspaces.sh"),
        "#!/usr/bin/env bash\n\
         echo '[side-workspaces] SKIP side this fixture has no side workspace'\n",
    );
    write(
        &at.join(".github/workflows/ci.yml"),
        &format!(
            "name: ci\n\
             on: [push]\n\
             jobs:\n\
             \x20 build:\n\
             \x20   runs-on: ubuntu-latest\n\
             \x20   steps:\n\
             \x20     - run: {ci}\n"
        ),
    );

    git(&at, &["init", "-q"]);
    git(&at, &["add", "-A"]);
    at
}

fn write(path: &Path, text: &str) {
    std::fs::write(path, text).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
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

/// Run the gate in that tree, exactly as a hook does: from its root.
fn gate(at: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_unrun-tests"))
        .current_dir(at)
        // NAMED RATHER THAN INHERITED (R1182): the gate asks cargo for the
        // workspace census, and which cargo it asks is `$CARGO` — set when a
        // suite runs under cargo and absent when the test binary is run
        // directly, where it would silently fall back to whatever is on PATH.
        .env("CARGO", env!("CARGO"))
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

/// A tree whose CI runs every test it compiles: the law holds, and it says so.
#[test]
fn a_tree_whose_tests_ci_all_runs_exits_zero_and_says_which_answer_that_is() {
    let at = tree("clean", "cargo test");
    let out = gate(&at);
    assert_eq!(
        code(&out),
        0,
        "stdout:\n{}\nstderr:\n{}",
        stdout(&out),
        stderr(&out)
    );
    assert!(
        stdout(&out).contains("every test this repository compiles is run by CI"),
        "the clean answer is said out loud rather than left as an exit code\n{}",
        stdout(&out)
    );
    // AND THE MIRROR, because the sentence above is printed on ONE path and the
    // two failing paths are what this is here to be told apart from.
    assert!(
        !stderr(&out).contains("REFUSED"),
        "a tree it judged cleanly refuses nothing\n{}",
        stderr(&out)
    );
    assert!(
        !stdout(&out).contains(DARK_TEST) && !stderr(&out).contains(DARK_TEST),
        "and names no dark test\n{}\n{}",
        stdout(&out),
        stderr(&out)
    );
    let _ = std::fs::remove_dir_all(&at);
}

/// A tree with a test CI does not run: dark, code `1`, and the test is named.
#[test]
fn a_tree_with_a_test_ci_leaves_out_exits_one_and_names_it() {
    let at = tree("dark", &format!("cargo test -- {RUN_TEST}"));
    let out = gate(&at);
    assert_eq!(
        code(&out),
        1,
        "stdout:\n{}\nstderr:\n{}",
        stdout(&out),
        stderr(&out)
    );
    let said = stderr(&out);
    assert!(
        said.contains(DARK_TEST),
        "a dark test is worth nothing unnamed\n{said}"
    );
    // THE CONTROL: the one CI DOES run must not be reported dark, or this would
    // pass for a gate that simply reported everything.
    assert!(
        !said.contains(RUN_TEST),
        "and the test CI runs is not among them\n{said}"
    );
    assert!(
        !stdout(&out).contains("every test this repository compiles is run by CI"),
        "and the clean sentence belongs to the other answer\n{}",
        stdout(&out)
    );
    let _ = std::fs::remove_dir_all(&at);
}

/// A tree whose CI issues no `cargo test` at all: NOT dark — unjudged.
///
/// THIS IS THE ONE R1078 GOT WRONG, and the one that was unreachable until this
/// file existed. "No test is dark" and "nothing looked" print the same silence,
/// and the gate's whole reason for a third code is to keep them apart. The
/// oracle is the exit code AND the refusal in the words the type prints for
/// itself, so a rewording moves the assertion with it.
#[test]
fn a_tree_whose_ci_runs_no_tests_is_unjudged_rather_than_clean() {
    let at = tree("unjudged", "cargo build");
    let out = gate(&at);
    assert_eq!(
        code(&out),
        2,
        "a gate that could not look must not answer 1 or 0\nstdout:\n{}\nstderr:\n{}",
        stdout(&out),
        stderr(&out)
    );
    assert!(
        stderr(&out).contains(&Refusal::NoTestCommands.to_string()),
        "and it says WHY it could not judge\n{}",
        stderr(&out)
    );
    assert!(
        !stdout(&out).contains("every test this repository compiles is run by CI"),
        "a tree it did not read is not a tree it found clean\n{}",
        stdout(&out)
    );
    let _ = std::fs::remove_dir_all(&at);
}

/// And a directory that is not a cargo tree at all is the same third answer.
///
/// CHEAP AND FIRST: it takes this path before any probe runs, so it is the one
/// case that costs no compiler. It is here because the code is the same `2` and
/// the two reasons are different — a reader who saw only the case above could
/// conclude the code means "CI issues no test command".
#[test]
fn a_directory_that_is_not_a_cargo_tree_is_unjudged_too() {
    let at = std::env::temp_dir().join(format!("unrun-tests-entrance-bare-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&at);
    std::fs::create_dir_all(&at).expect("bare directory");
    let out = gate(&at);
    assert_eq!(code(&out), 2, "stderr:\n{}", stderr(&out));
    assert!(
        stderr(&out).contains("has no Cargo.toml"),
        "and names what it wanted to find\n{}",
        stderr(&out)
    );
    let _ = std::fs::remove_dir_all(&at);
}
