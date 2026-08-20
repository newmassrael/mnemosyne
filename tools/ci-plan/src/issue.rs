//! The one door a Rust program in this repository issues a cargo command
//! through.
//!
//! # Why the issuer lives beside the reader
//!
//! [`crate::rust`] answers WHERE this repository's Rust sources spawn cargo, and
//! its whole population is the spelling written here: a law that hunts for
//! `Command::new(..)` can find every spawn but cannot read the one fact a spawn
//! keeps to itself. Two crates — one to issue, one to read — would be two
//! spellings free to drift, and the reader would go quiet the day the issuer
//! moved. One crate, one spelling, and the law is that nothing else spawns
//! cargo at all.
//!
//! # The fact a static reader can never have
//!
//! `locked_resolution_smoke` asks of every cargo command whether it pins the
//! lockfile it resolves, and the answer turns on WHOSE lockfile it is. For the
//! five sources of data this crate reads, the target is written down. For a Rust
//! program it is a value:
//!
//! ```text
//! Command::new(cargo()).args(["metadata", ..]).arg("--manifest-path").arg(manifest)
//! ```
//!
//! and `manifest` is `tools/blind-waits/Cargo.toml` in the gate that walks this
//! repository's workspaces and a directory the test made two lines earlier in
//! the fixture next door. THE WORDS ARE IDENTICAL. No reader of the syntax can
//! separate them, and both readings are wrong for the other site: demanding
//! `--locked` of the fixture rejects a tree that has no lockfile to pin, and
//! excusing the gate leaves five programs quietly rewriting the lockfiles of the
//! workspaces they were built to report on.
//!
//! So the site says which, in the one place that knows: [`Tree`] is an argument,
//! the compiler makes it impossible to omit, and the law reads the declaration
//! and the words together. That is the "a declaration per program, and a way to
//! hold it against what runs" half of the answer Round 1257 wrote down — with
//! the declaration IN the call rather than in a file beside it, because a
//! declaration a program does not have to touch to change what it runs is one
//! that goes stale in silence.
//!
//! # What it does not do
//!
//! It does not add `--locked` for a caller that declared [`Tree::ThisRepository`]
//! and left it out. A builder that quietly repairs the command is a builder
//! whose source no longer says what runs, and this repository's laws are read by
//! people opening the file. The law rejects the pair instead.

use std::ffi::OsStr;
use std::process::Command;

/// Whose lockfile a cargo command resolves — the fact only the call site has.
///
/// FOUR ARMS AND EACH ONE OWES SOMETHING. Two of them decline to name a tree,
/// because a gate's census is pointed at a manifest by whoever calls it and its
/// author honestly cannot say which — and declining is the HARDEST position
/// rather than the easiest, with two ways to pay for it: resolve nothing at all
/// ([`Tree::WhereverTheCallerPoints`]), or pin whatever you land on
/// ([`Tree::PinnedWhereverItPoints`]). An arm that asked nothing would be an
/// exemption, and R1259 spent a round on what an expectation that cannot fail is
/// worth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tree {
    /// This repository, in any of its workspaces. Its lockfiles are tracked
    /// here, so a resolving subcommand must pin them: a free resolve REWRITES a
    /// lockfile that disagrees with its manifests instead of failing on it, and
    /// every check after it in the same run reads the repaired file.
    ThisRepository,
    /// A tree this run made, or one belonging to somebody else: a fixture
    /// workspace a test wrote, a worktree checked out at another revision, a
    /// sibling checkout. Nothing here pins those, and the reason is carried so a
    /// reader of the report is told rather than left to assume.
    MadeByThisRun(&'static str),
    /// Whichever tree the caller points at — this repository's workspaces on one
    /// call and a fixture the hook tests built on the next. THE OBLIGATION: the
    /// command must resolve NOTHING. `cargo metadata --no-deps` is the shape
    /// this arm is for, and that it resolves nothing is measured rather than
    /// assumed (`locked_resolution_smoke`). A command here that CAN rewrite a
    /// lockfile is refused, because whose lockfile it is has become a question
    /// with an answer nobody wrote down.
    WhereverTheCallerPoints(&'static str),
    /// Whichever tree the caller points at, PINNED WHATEVER IT TURNS OUT TO BE.
    ///
    /// The arm above declines to name a tree and pays by touching no lockfile at
    /// all. This one declines and pays the other way: the command says
    /// `--locked`, so whosever lockfile it meets, it REPORTS a disagreement
    /// instead of repairing it. That is the answer for a gate that has to run a
    /// resolving subcommand — `cargo check` for the artifacts it reads, `cargo
    /// test -- --list` for the names it asks about — over a workspace named at
    /// run time.
    ///
    /// THE COST IS REAL AND IS THE POINT: pointed at a tree with a stale
    /// lockfile, or none, the gate now FAILS where it used to quietly rewrite,
    /// and a gate that rewrites the tree it is judging is the very thing
    /// `locked_resolution_smoke` exists to find. A caller whose trees are
    /// throwaway makes their lockfiles first (`cargo generate-lockfile`), which
    /// is one line and says what it is doing.
    PinnedWhereverItPoints(&'static str),
    /// Whichever tree the caller points at, PINNED EXACTLY WHEN IT TURNS OUT TO
    /// BE ONE OF THIS REPOSITORY'S — a fact the site decides while it runs.
    ///
    /// `feature_coverage_smoke` asks every workspace this repository tracks for
    /// its metadata, and one of them resolves against a sibling checkout: its
    /// lockfile moves when that sibling moves and is not ours to pin. So the
    /// command pins on one path and not the other, and neither
    /// [`Tree::ThisRepository`] nor an arm that declines to name a tree is true
    /// of it — the first is false wherever the flag is absent, which is how
    /// R1265 found this site declaring something it is not.
    ///
    /// THE OBLIGATION IS ON THE SITE AND IT IS FALSIFIABLE: `--locked` must be a
    /// word this site adds ON SOME PATHS. Spelled unconditionally the arm is a
    /// lie about the code (there is no condition), and absent altogether it is a
    /// lie the other way — and a law that only ever agreed with whatever the
    /// site did would be the expectation that cannot fail R1259 spent a round
    /// on. What no program here can check is whether the CONDITION is ownership
    /// rather than something else; that is the semantic ceiling every one of
    /// these arms shares.
    PinnedWhenItIsOurs(&'static str),
}

impl Tree {
    /// The reason a tree is not named as this repository's, or `None` when it is.
    #[must_use]
    pub fn because(&self) -> Option<&'static str> {
        match self {
            Self::ThisRepository => None,
            Self::MadeByThisRun(why)
            | Self::WhereverTheCallerPoints(why)
            | Self::PinnedWhereverItPoints(why)
            | Self::PinnedWhenItIsOurs(why) => Some(why),
        }
    }
}

/// The cargo this process should run, pinned to the one that built it when
/// there is one.
///
/// `Command::new("cargo")` lets PATH decide, and PATH is a fact about the
/// machine rather than about this repository — the R1190 correction. Five
/// programs under `tools/` carried a private copy of this line until R1262 gave
/// them one home; the copies were identical, which is what made the sixth one
/// nobody would have noticed differing.
#[must_use]
pub fn program() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string())
}

/// A cargo command, declaring whose lockfile it is about to resolve.
///
/// The words are the caller's, added with `.arg()` and `.args()` as on any
/// [`Command`] — this returns the program and the declaration, not a second
/// argument vocabulary. [`crate::rust`] reads the chain from here outwards.
///
/// # Panics
///
/// When a caller declares [`Tree::MadeByThisRun`] with a blank reason. A
/// declaration that names no reason is the shape of a caller reaching for the
/// arm that asks nothing of it, and R1258 met the same one in
/// `injection-harness forget --because`: the answer there was to refuse rather
/// than to store an empty string that reads afterwards like an answered
/// question.
#[must_use]
pub fn cargo(tree: Tree) -> Command {
    named_cargo(program(), tree)
}

/// A cargo command whose PROGRAM was handed in rather than read off this
/// process.
///
/// One case, and it is not a loophole: `experiment-harness open-kit` builds a
/// pinned old revision with the cargo the replay NAMES, because which cargo
/// built the evidence is part of what a replay reproduces. The declaration is
/// asked for here exactly as it is next door — what varies is the program, not
/// whether the site says whose lockfile it resolves.
///
/// # Panics
///
/// When a caller declares a tree that is not this repository's with a blank
/// reason. See [`cargo`].
#[must_use]
pub fn named_cargo(program: impl AsRef<OsStr>, tree: Tree) -> Command {
    if let Some(why) = tree.because() {
        assert!(
            !why.trim().is_empty(),
            "a cargo command declared to run over a tree that is not this \
             repository's says WHY — a blank reason is the arm that asks nothing \
             of the caller, taken silently"
        );
    }
    Command::new(program)
}
