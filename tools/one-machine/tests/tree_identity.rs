//! What a dispatch would send, named so that a commit does not rename it.
//!
//! THE LOAD-BEARING PROPERTY IS THAT A COMMIT CHANGES NOTHING HERE. The dispatch
//! happens at commit time, when the content of a round is final; the reading
//! happens at push time, after `HEAD` has moved and not one tracked byte has. A
//! fingerprint carrying `HEAD` would call those two different trees, and this
//! gate would never once judge the tree it was built for — it would answer "that
//! census is about another tree" at every push, forever, which is a refusal that
//! reads exactly like a gate working.
//!
//! And the second property is that TWO DIRECTORIES WITH THE SAME CONTENT ARE THE
//! SAME TREE. The census is taken in a copy at another path on another machine,
//! and the whole comparison depends on the copy naming itself the same way.

use std::path::Path;
use std::process::Command;

use one_machine::fingerprint;

fn git(at: &Path, arguments: &[&str]) {
    let status = Command::new("git")
        .args(arguments)
        .current_dir(at)
        .status()
        .unwrap_or_else(|error| panic!("git {arguments:?} could not be run — {error}"));
    assert!(status.success(), "git {arguments:?} failed in {at:?}");
}

/// A repository with one tracked file, committed — the state every case starts
/// from.
fn a_repository() -> tempfile::TempDir {
    let tree = tempfile::tempdir().expect("a directory to stand in");
    let at = tree.path();
    git(at, &["init", "--quiet"]);
    git(at, &["config", "user.email", "case@example.invalid"]);
    git(at, &["config", "user.name", "a case"]);
    std::fs::write(at.join("tracked"), "one\n").expect("a tracked file");
    git(at, &["add", "tracked"]);
    git(at, &["commit", "--quiet", "-m", "one"]);
    tree
}

#[test]
fn the_same_tree_names_the_same_bytes_twice() {
    let tree = a_repository();
    let first = fingerprint(tree.path()).expect("a repository");
    let second = fingerprint(tree.path()).expect("a repository");
    assert_eq!(first, second);
    assert!(!first.is_empty());
}

#[test]
fn a_commit_does_not_change_it() {
    let tree = a_repository();
    std::fs::write(tree.path().join("tracked"), "two\n").expect("an edit");
    git(tree.path(), &["add", "tracked"]);
    let before = fingerprint(tree.path()).expect("a repository");
    git(tree.path(), &["commit", "--quiet", "-m", "two"]);
    let after = fingerprint(tree.path()).expect("a repository");
    assert_eq!(
        before, after,
        "the dispatch happens before the commit and the reading after it; a \
         fingerprint that moves with HEAD judges no round at all"
    );
}

#[test]
fn two_directories_holding_the_same_content_are_the_same_tree() {
    let one = a_repository();
    let other = a_repository();
    assert_eq!(
        fingerprint(one.path()).expect("a repository"),
        fingerprint(other.path()).expect("a repository"),
        "the census is taken in a copy at another path on another machine, and \
         the comparison is between what the two copies HOLD"
    );
}

#[test]
fn a_changed_tracked_byte_changes_it() {
    let tree = a_repository();
    let before = fingerprint(tree.path()).expect("a repository");
    std::fs::write(tree.path().join("tracked"), "one changed\n").expect("an edit");
    assert_ne!(before, fingerprint(tree.path()).expect("a repository"));
}

#[test]
fn a_new_tracked_file_changes_it() {
    let tree = a_repository();
    let before = fingerprint(tree.path()).expect("a repository");
    std::fs::write(tree.path().join("second"), "two\n").expect("a file");
    git(tree.path(), &["add", "second"]);
    assert_ne!(before, fingerprint(tree.path()).expect("a repository"));
}

#[test]
fn a_deleted_tracked_file_changes_it() {
    let tree = a_repository();
    let before = fingerprint(tree.path()).expect("a repository");
    std::fs::remove_file(tree.path().join("tracked")).expect("a deletion");
    assert_ne!(
        before,
        fingerprint(tree.path()).expect("a repository"),
        "a file that has left the working tree is a file the dispatch does not send"
    );
}

/// An untracked file never leaves this machine, so it is not part of what the
/// far side is asked about — and a fingerprint that moved with one would refuse
/// every census taken while a scratch file existed.
#[test]
fn an_untracked_file_does_not_change_it() {
    let tree = a_repository();
    let before = fingerprint(tree.path()).expect("a repository");
    std::fs::write(tree.path().join("scratch"), "not sent\n").expect("a file");
    assert_eq!(before, fingerprint(tree.path()).expect("a repository"));
}

/// This repository ships scripts something executes, and git SILENTLY SKIPS a
/// hook without the bit — so a tree that differs only in the executable bit is
/// a different tree, and a fingerprint blind to it would call a broken copy the
/// same as a working one. Measured the hard way in this very round: the first
/// dispatch of `scripts/census-elsewhere.sh` landed without the bit and the far
/// side answered `Permission denied`.
///
/// ⚠ THE BIT IS MOVED THROUGH GIT AND NOT THROUGH `set_permissions`, and that is
/// a requirement rather than a taste: `tools/written-executable` refuses a
/// function that gives a file an executable mode, after R1192 measured what
/// writing-then-executing costs beside ten other crates. Nothing here executes
/// this file, and the law cannot know that — routing round it with an exemption
/// would be worse than asking git, which is the layer that actually decides.
#[cfg(unix)]
#[test]
fn the_executable_bit_is_part_of_what_a_tree_is() {
    let tree = a_repository();
    let before = fingerprint(tree.path()).expect("a repository");
    git(tree.path(), &["update-index", "--chmod=+x", "tracked"]);
    assert_ne!(before, fingerprint(tree.path()).expect("a repository"));
}

/// A binary difference must be a DIFFERENCE, not the sentence "Binary files
/// differ" — which is the same sentence for every possible content and would
/// make two unrelated trees name themselves alike.
#[test]
fn two_different_binary_contents_are_two_trees() {
    let tree = a_repository();
    let blob = tree.path().join("blob.bin");
    std::fs::write(&blob, [0u8, 159, 146, 150, 0, 1, 2]).expect("a binary file");
    git(tree.path(), &["add", "blob.bin"]);
    git(tree.path(), &["commit", "--quiet", "-m", "blob"]);
    let before = fingerprint(tree.path()).expect("a repository");
    std::fs::write(&blob, [0u8, 159, 146, 150, 0, 1, 3]).expect("one byte");
    assert_ne!(before, fingerprint(tree.path()).expect("a repository"));
}

#[test]
fn a_directory_git_cannot_answer_about_is_a_refusal_not_a_fingerprint() {
    let not_a_repository = tempfile::tempdir().expect("a directory");
    let error =
        fingerprint(not_a_repository.path()).expect_err("git has nothing to say about this");
    // THE QUESTION IS NAMED, not which git verb asked it: the claim here is
    // that a tree git cannot answer about produces a REFUSAL rather than the
    // fingerprint of an empty listing, and pinning the verb would make this case
    // red for a reading that has nothing to do with that.
    assert!(
        error.contains("git "),
        "a refusal names the question it could not answer: {error}"
    );
}
