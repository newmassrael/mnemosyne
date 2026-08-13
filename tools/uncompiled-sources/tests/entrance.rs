//! The three answers this gate gives, asked of the PROGRAM.
//!
//! `src/main.rs` says it in its own doc comment: the exit codes are three
//! answers and not two — `0` the law holds, `1` Rust exists that nothing
//! compiles, `2` the gate could not judge — and one code for the last two
//! mislabels whichever it did not mean, "which is the failure R1078 shipped".
//! That decision lived in `main.rs` and NOTHING RAN THE BINARY. R1128 measured
//! it the way R1127 measured the sibling gate: an injection turning the refusal
//! path into a clean pass, run against the suite as it stood. Eleven injections
//! aimed at the library all fired; that one went SILENT across twenty-five
//! tests. A decision whose only reader is the process has to be read by running
//! the process.
//!
//! THE FIXTURES ARE TREES AND NOT CALLS, and `tests/gate.rs` reaches the same
//! mechanisms by calling `examine` directly. That is the right shape for the
//! mechanisms and the wrong one for this: what the binary decides is which
//! ANSWER a whole reading amounts to, and the pieces of that reading — what git
//! tracks, what cargo compiles, what the lister declines — are only assembled
//! by `run`. So each case here is a git repository the binary is pointed at,
//! and the cases differ in one file apiece.

use std::path::Path;
use std::process::{Command, Output};

use uncompiled_sources::Refusal;

/// The tracked file no target reads, which is the finding this gate is for.
///
/// NAMED ONCE. The assertions read it back out of the report, and a second
/// spelling could drift from the fixture into agreeing with anything.
const ORPHAN: &str = "src/orphan.rs";

const MANIFEST: &str = "[workspace]\n\
     \n\
     [package]\n\
     name = \"fixture\"\n\
     version = \"0.1.0\"\n\
     edition = \"2021\"\n";

/// A tree the binary can be pointed at: one crate, and a lister that answers.
///
/// OUTSIDE THIS REPOSITORY for the reason `tests/gate.rs` states — this gate
/// reads `git ls-files`, so a fixture inside the tree would answer with THIS
/// repository's files — and carrying the lister's side of the contract, because
/// `run` shells into `scripts/check-side-workspaces.sh --list` and refuses a
/// lister that names nothing.
///
/// `tracked` is what separates the third answer from the first two: a tree git
/// holds nothing of is one this gate must refuse rather than call clean.
fn tree(files: &[(&str, &str)], tracked: bool) -> tempfile::TempDir {
    let at = tempfile::tempdir().expect("a scratch directory");
    let root = at.path();
    for (name, body) in files {
        let path = root.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("fixture directory");
        }
        std::fs::write(&path, body).expect("write fixture file");
    }
    std::fs::create_dir_all(root.join("scripts")).expect("fixture scripts");
    std::fs::write(
        root.join("scripts/check-side-workspaces.sh"),
        "#!/usr/bin/env bash\n\
         echo '[side-workspaces] SKIP side this fixture has no side workspace'\n",
    )
    .expect("write the lister stub");

    git(root, &["init", "--quiet"]);
    if tracked {
        // `git add` rather than `git commit`: `git ls-files` reads the index,
        // and committing would need an identity this machine may not have.
        git(root, &["add", "-A"]);
    }
    at
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
    Command::new(env!("CARGO_BIN_EXE_uncompiled-sources"))
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

/// The sentence printed on exactly one of the three paths.
const CLEAN: &str = "every Rust file this repository tracks is one cargo compiles";

/// A tree whose every tracked file is compiled: the law holds, and it says so.
#[test]
fn a_tree_cargo_compiles_all_of_exits_zero_and_says_which_answer_that_is() {
    let at = tree(
        &[
            ("Cargo.toml", MANIFEST),
            ("src/lib.rs", "pub fn reached() {}\n"),
        ],
        true,
    );
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
    // THE MIRROR. That sentence is printed on ONE path, and the two failing
    // paths are what this case exists to be told apart from.
    assert!(
        !stderr(&out).contains("REFUSED"),
        "a tree it judged cleanly refuses nothing\n{}",
        stderr(&out)
    );
}

/// A tracked file no target reads: a finding, code `1`, and it is named.
#[test]
fn a_tree_holding_rust_nothing_compiles_exits_one_and_names_the_file() {
    let at = tree(
        &[
            ("Cargo.toml", MANIFEST),
            ("src/lib.rs", "pub fn reached() {}\n"),
            (ORPHAN, "#[test]\nfn orphaned() {}\n"),
        ],
        true,
    );
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
        said.contains(ORPHAN),
        "a file no compiler reads is worth nothing unnamed\n{said}"
    );
    // AND THE TEST INSIDE IT, which is the reason the finding matters: source no
    // compiler reads has no opinion held about it, so a test in there is one no
    // gate can see.
    assert!(
        said.contains("orphaned"),
        "and the tests it hides are what the finding is about\n{said}"
    );
    // THE CONTROL: the file that IS compiled must not be reported, or this would
    // pass for a gate that reported every file it tracks.
    assert!(
        !said.contains("src/lib.rs"),
        "and the compiled file is not among them\n{said}"
    );
    assert!(
        !stdout(&out).contains(CLEAN),
        "and the clean sentence belongs to the other answer\n{}",
        stdout(&out)
    );
}

/// A tree git holds nothing of: NOT clean — unjudged.
///
/// THIS IS THE ONE R1078 GOT WRONG, and the one unreachable until this file
/// existed. "No file is uncompiled" and "nothing was read" print the same
/// silence, and the third code is the whole of what keeps them apart. The oracle
/// is the exit code AND the refusal in the words the type prints for itself, so
/// a rewording moves the assertion with it.
#[test]
fn a_tree_git_tracks_nothing_of_is_unjudged_rather_than_clean() {
    let at = tree(
        &[
            ("Cargo.toml", MANIFEST),
            ("src/lib.rs", "pub fn reached() {}\n"),
        ],
        false,
    );
    let out = gate(at.path());
    assert_eq!(
        code(&out),
        2,
        "a gate that could not look must not answer 1 or 0\nstdout:\n{}\nstderr:\n{}",
        stdout(&out),
        stderr(&out)
    );
    assert!(
        stderr(&out).contains(&Refusal::NothingTracked.to_string()),
        "and it says WHY it could not judge\n{}",
        stderr(&out)
    );
    assert!(
        !stdout(&out).contains(CLEAN),
        "a tree it did not read is not a tree it found clean\n{}",
        stdout(&out)
    );
}

/// And a directory that is not a cargo tree at all is the same third answer.
///
/// CHEAP AND FIRST: it is taken before any probe, so it is the one case that
/// costs no compiler. It is here because the code is the same `2` for a
/// different reason — a reader shown only the case above could conclude the code
/// means "git tracks no Rust".
#[test]
fn a_directory_that_is_not_a_cargo_tree_is_unjudged_too() {
    let at = tempfile::tempdir().expect("a scratch directory");
    let out = gate(at.path());
    assert_eq!(code(&out), 2, "stderr:\n{}", stderr(&out));
    assert!(
        stderr(&out).contains("has no Cargo.toml"),
        "and names what it wanted to find\n{}",
        stderr(&out)
    );
}
