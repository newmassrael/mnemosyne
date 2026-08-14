//! A program that answers `--explain-declaration` from a file, so the law's
//! arms can be exercised without the installed one.
//!
//! # Why this is BUILT rather than written at run time
//!
//! The first version of these cases wrote a small shell script into a temporary
//! directory and ran it. Under `check-side-workspaces.sh` on the build machine
//! that failed with `Text file busy` (R1192): a file this process has open for
//! writing cannot be executed, and with eleven cases in flight another thread's
//! `fork` inherits that descriptor and holds it across the window in which this
//! thread runs the file. Nothing about the case was wrong; the shape was — an
//! executable a test writes and then executes is state the test does not own,
//! the same family as Round 1175's constant fixture paths.
//!
//! Cargo builds this one before any test starts, so there is no descriptor and
//! no window. What varies per case is DATA: the answer text, in a file named by
//! `UNREAD_DECLARATION_ANSWER`. Data is not executed, so it cannot be busy.
//!
//! With no such variable, or a path that is not there, it prints nothing — which
//! is a real arm of the law rather than an accident: a program that runs, exits
//! zero and says nothing is what an older copy without the seam looks like.

fn main() {
    let Ok(answer) = std::env::var("UNREAD_DECLARATION_ANSWER") else {
        return;
    };
    if let Ok(text) = std::fs::read_to_string(answer) {
        print!("{text}");
    }
}
