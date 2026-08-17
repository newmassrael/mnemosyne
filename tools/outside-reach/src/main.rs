//! Read a `strace -f` stream on stdin and say what the run reached outside
//! this repository — and whether this repository declares it.
//!
//! # Why stdin
//!
//! Because that is what makes the census free. `strace -o "|outside-reach …"`
//! hands the trace to this program through a pipe, so the 611 MB the whole
//! suite produces is never a file: measured, the same run costs ZERO bytes on
//! disk, where the round that built this census first wrote 2.8 GB across
//! 155,072 files and recorded that as the price of asking.
//!
//! # The declaration is here
//!
//! The library walks; this names what this repository is allowed to reach and
//! why, which is the same division `uncompiled-sources` draws between its walk
//! and its exception table. A row is a claim somebody defends in review.

use std::io::{self, BufReader};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use outside_reach::{judge, read_stream, DeclaredReach, Ground, Where};

/// The trees this repository's own suite reaches, and why each one.
///
/// MEASURED, NOT LISTED FROM MEMORY. A syscall census of the whole root suite —
/// 156,692 processes and threads, 3,735,573 lines through the pipe — reached
/// exactly these four outside the repository, its build directory, the fixture
/// root and the toolchain, and every one of them was exercised on the machine
/// that wrote them. Round 1228 took the same census a costlier way and found
/// the same set.
///
/// NOT ONE OF THESE NAMES A MACHINE, and that is load-bearing rather than tidy.
/// The first draft wrote `/home/coin/…` into every row, which is exactly the
/// defect class this gate exists to find: on a hosted runner `HOME` is
/// `/home/runner`, so the git rows would have matched nothing and the census
/// would have reported git's own configuration as an undeclared reach.
const DECLARED: &[DeclaredReach] = &[
    DeclaredReach {
        at: Where::Sibling("pinion"),
        why: "the workspace lister resolves `studio`, whose path dependencies \
              name a sibling checkout; `check-side-workspaces.sh` asks cargo \
              about it and cargo STATS the directories. The largest reach here \
              and the one Round 1227's defect was about",
        only_where_the_tree_exists: true,
    },
    DeclaredReach {
        at: Where::Sibling("belvoir-extraction"),
        why: "`evidence_replay_smoke` re-checks a carried artifact against the \
              source it was carried from, which is where that source is. That \
              law already announces when the source is not on the machine \
              asking — it is the model this gate generalises",
        only_where_the_tree_exists: true,
    },
    DeclaredReach {
        at: Where::Home(".gitconfig"),
        why: "git reads the user's own configuration on every invocation, and \
              nine test binaries run git. Measured to decide NOTHING here: a \
              whole-suite arm with an empty HOME produced zero differences in \
              any test's output",
        only_where_the_tree_exists: false,
    },
    DeclaredReach {
        at: Where::Home(".config/git"),
        why: "the other half of git's configuration search, same nine binaries \
              and the same measured answer",
        only_where_the_tree_exists: false,
    },
];

fn flag(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == name)
        .and_then(|at| args.get(at + 1))
        .cloned()
}

fn usage() -> ExitCode {
    eprintln!(
        "usage: strace -f -qq -e trace=%file,%process -e status=successful \\\n\
         \x20         -o \"|outside-reach --repo <path> --build <path> --fixture <path>\" <command…>\n\
         \n\
         Reads the trace on stdin. Nothing is written to disk.\n\
         \n\
         --repo     the repository the run is about\n\
         --build    its build directory (a symlink out of the tree, here)\n\
         --fixture  the TMPDIR the run was given\n\
         --report   print the census and return 0 whatever it found"
    );
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let (Some(repo), Some(build), Some(fixture)) =
        (flag("--repo"), flag("--build"), flag("--fixture"))
    else {
        return usage();
    };
    // THE TOOLCHAIN AND THE OPERATING SYSTEM, which every machine that can
    // build this has — including a hosted runner. Read from the environment
    // rather than written down: a path spelled here would be this machine's.
    let mut toolchain: Vec<PathBuf> = [
        "/usr", "/lib", "/lib64", "/bin", "/sbin", "/etc", "/proc", "/sys", "/dev", "/run", "/opt",
        "/snap", "/var",
    ]
    .iter()
    .map(PathBuf::from)
    .collect();
    // EACH NAME IS SPELLED AT THE CALL, not looped over, and the gate that
    // insisted is right to. `tools/named-environment` walks what a program
    // READS so a test that spawns it can be held to naming the same set; a
    // loop variable makes that unreadable, and it REFUSED rather than counting
    // an unresolvable read as no read. Four literals cost nothing and keep this
    // program's environment answerable from its source.
    for value in [
        std::env::var("CARGO_HOME"),
        std::env::var("RUSTUP_HOME"),
        std::env::var("SCCACHE_DIR"),
        std::env::var("CCACHE_DIR"),
    ]
    .into_iter()
    .flatten()
    {
        toolchain.push(PathBuf::from(value));
    }
    if let Ok(home) = std::env::var("HOME") {
        for under in [
            ".cargo",
            ".rustup",
            ".cache/sccache",
            ".cache/ccache",
            ".config/sccache",
            ".config/ccache",
        ] {
            toolchain.push(PathBuf::from(&home).join(under));
        }
    }
    let ground = Ground {
        owned: vec![
            PathBuf::from(&repo),
            PathBuf::from(&build),
            PathBuf::from(&fixture),
        ],
        toolchain,
    };

    let census = read_stream(BufReader::new(io::stdin().lock()), &ground);
    println!(
        "[outside-reach] {} process(es), {} line(s), {} path(s) named relative to \
         a descriptor, {} line(s) this reader could not read whole",
        census.processes, census.lines, census.relative, census.unparsed
    );
    // NON-VACUITY, FIRST AND LOUDEST. A census over a trace that never arrived
    // reports no reaches, and no reaches is what a hermetic suite looks like.
    if census.lines == 0 {
        eprintln!(
            "[outside-reach] the trace was EMPTY. That is not a hermetic run, it \
             is a run this gate never saw — check that strace's `-o \"|…\"` names \
             this program and that the command under it actually ran"
        );
        return ExitCode::from(2);
    }

    let home = PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/root".to_string()));
    let verdict = judge(&census, DECLARED, Path::new(&repo), &home);
    for (tree, targets) in census.trees(3) {
        println!(
            "[outside-reach] REACH {} — by {}",
            tree.display(),
            targets.into_iter().collect::<Vec<_>>().join(" ")
        );
    }
    for tree in &verdict.unexercised {
        // THE THIRD STATE, SAID RATHER THAN PASSED OVER: a row that went
        // unexercised here is not a stale row, and a green run is not evidence
        // about a reach it never made.
        println!(
            "[outside-reach] DECLARED BUT NOT REACHED HERE: {} — this run \
             proves nothing about it",
            tree.display()
        );
    }
    println!(
        "[outside-reach] {} declared tree(s), {} reached by this run",
        DECLARED.len(),
        verdict.exercised.len()
    );

    if flag("--report").is_some() || std::env::args().any(|a| a == "--report") {
        return ExitCode::SUCCESS;
    }
    if verdict.is_clean() {
        return ExitCode::SUCCESS;
    }
    eprintln!(
        "[outside-reach] {} reach(es) outside this repository that nothing \
         declares. A test that reads a tree the machine beside it does not have \
         is a test whose answer is about the machine — which is what this gate \
         exists to make visible rather than to forbid. Add a row naming WHY, or \
         stop reaching:",
        verdict.undeclared.len()
    );
    for (target, path) in &verdict.undeclared {
        eprintln!("[outside-reach]   {target} reached {}", path.display());
    }
    ExitCode::from(1)
}
