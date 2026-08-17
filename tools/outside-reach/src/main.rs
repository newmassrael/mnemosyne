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
        at: Where::Home(".config"),
        why: "the other half of git's configuration search — `~/.config/git` — \
              and the directory it stats on the way there, which is the shape a \
              hosted runner reported. The row is the DIRECTORY rather than the \
              file, and the cost of that is stated: anything else under \
              `~/.config` a run reads is excused with it. Same nine binaries and \
              the same measured answer as the row above",
        only_where_the_tree_exists: false,
    },
    // R1232 — THE TWO BELOW ARE THE HOSTED RUNNER'S OWN HOME, and this is the
    // first census that ever ran there. Neither is on a workstation, which is
    // why `only_where_the_tree_exists` is true for both and why they will read
    // as `DECLARED BUT NOT REACHED HERE` in every local run.
    DeclaredReach {
        at: Where::Home(".dotnet"),
        why: "the GitHub-hosted runner image puts `~/.dotnet/tools` on `PATH`, \
              so a program lookup stats it and the directory above it. Reached \
              by the cargo driver and by the `all` suite; nothing in this \
              repository asks for .NET",
        only_where_the_tree_exists: true,
    },
    DeclaredReach {
        at: Where::Home("work/_temp"),
        why: "`actions/checkout` persists credentials by writing a git config \
              into the runner's scratch and pointing git at it, so EVERY git \
              invocation on a runner reads it — measured: the `all` suite, \
              `evidence_replay_smoke`, `reading` and the cargo driver all did. \
              This is `RUNNER_TEMP`, written relative to HOME because that is \
              what this table can resolve without an environment read; if the \
              runner's layout moves, this row goes unexercised and the reach \
              comes back as a finding, which is the loud failure rather than the \
              quiet one. It excuses the whole scratch root, and that is the cost",
        only_where_the_tree_exists: true,
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
        "usage: strace -f -qq \\\n\
         \x20         -e trace=%file,%process,close,close_range,dup,dup2,dup3,fchdir,fcntl \\\n\
         \x20         -e status=successful \\\n\
         \x20         -o \"|outside-reach --repo <path> --build <path> --fixture <path> \
         --cwd <path>\" <command…>\n\
         \n\
         Reads the trace on stdin. Nothing is written to disk.\n\
         \n\
         --repo        the repository the run is about\n\
         --build       its build directory (a symlink out of the tree, here)\n\
         --fixture     the TMPDIR the run was given\n\
         --cwd         the directory the traced command was launched from, which \
         is the one fact\n\
         \x20             about a run its own syscalls never state; without it a \
         bare name is\n\
         \x20             unresolvable rather than wrong\n\
         --report      print the census and return 0 whatever it found\n\
         --verdict-of  exit with the status recorded in a file by an earlier run \
         of this program"
    );
    ExitCode::from(2)
}

/// Exit with the status an earlier run of this program left in a file.
///
/// # Why this is a program and not four lines of shell
///
/// `strace` returns the status of the command it WRAPPED and says nothing about
/// the program on the other end of its `-o "|…"` pipe, so the census writes its
/// verdict to a file and a later step turns that into the job's answer. That
/// step was shell, and R1230 measured what shell costs here: the suite under the
/// census overran its job's budget, the job was CANCELLED before the reader
/// wrote anything, and `verdict=$(cat …)` on a file that does not exist left an
/// empty string that `exit "$verdict"` turned into a shell error. The step went
/// red — correctly — with a message about `exit` and nothing about the census.
///
/// FROM OUTSIDE, THAT IS INDISTINGUISHABLE FROM THE CENSUS REFUSING, which is
/// the one thing this gate must never be. "The trace was empty so this gate
/// never saw the run" and "the run was killed before this gate could look" are
/// different facts with different repairs, and the first is the answer to a
/// question this repository has open — whether `ptrace` attaches on a hosted
/// runner at all. A step that cannot tell them apart destroys that measurement.
///
/// So it is a program, for the reason R1136 made `tools/ci-state` one: shell in
/// a workflow is the part no test can execute. The three answers this file
/// already gives are the three it gives here — 0 clean, 1 a finding, 2 could not
/// judge — and NO VERDICT AT ALL IS THE THIRD, not the first.
fn verdict_of(path: &Path) -> ExitCode {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) => {
            eprintln!(
                "[outside-reach] no verdict at {} ({e}) — this census never \
                 answered. The run it rides on was killed, or never started it; \
                 whatever this job says about reaches outside the tree, it did \
                 not learn it here",
                path.display()
            );
            return ExitCode::from(2);
        }
    };
    let trimmed = text.trim();
    match trimmed.parse::<u8>() {
        Ok(code) => {
            println!("[outside-reach] verdict={code}");
            ExitCode::from(code)
        }
        Err(_) => {
            eprintln!(
                "[outside-reach] the verdict at {} is `{trimmed}`, which is not a \
                 status. A file that holds something else holds nothing this step \
                 can exit with, and passing over it would report the census as \
                 clean",
                path.display()
            );
            ExitCode::from(2)
        }
    }
}

fn main() -> ExitCode {
    if let Some(path) = flag("--verdict-of") {
        return verdict_of(Path::new(&path));
    }
    let (Some(repo), Some(build), Some(fixture)) =
        (flag("--repo"), flag("--build"), flag("--fixture"))
    else {
        return usage();
    };
    // THE TOOLCHAIN AND THE OPERATING SYSTEM, which every machine that can
    // build this has — including a hosted runner. Read from the environment
    // rather than written down: a path spelled here would be this machine's.
    // `/lib32` AND `/libx32` ARE HERE BECAUSE A CENSUS FOUND THEM, not because
    // somebody remembered multilib (R1232). A gcc driver stats every library
    // root it knows on the way to linking, and these two are siblings of
    // `/lib64` in every sense except that the first version of this list was
    // written from memory. Measured twice: the hosted runner reported `/lib32`
    // among nineteen findings, and a whole-suite census on a build machine
    // reported exactly `/lib32` and `/libx32` once the rest was repaired.
    let mut toolchain: Vec<PathBuf> = [
        "/usr", "/lib", "/lib32", "/lib64", "/libx32", "/bin", "/sbin", "/etc", "/proc", "/sys",
        "/dev", "/run", "/opt", "/snap", "/var",
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

    // THE ONE FACT A RUN'S OWN SYSCALLS NEVER STATE. Optional, and its absence
    // costs resolution rather than correctness: without it every bare name is
    // counted unresolved instead of being measured from the wrong directory.
    let started_in = flag("--cwd").map(PathBuf::from);
    let census = read_stream(
        BufReader::new(io::stdin().lock()),
        &ground,
        started_in.as_deref(),
    );
    let unresolved = census.unresolved_total();
    println!(
        "[outside-reach] {} process(es), {} line(s), {} path(s) named relative to \
         a descriptor or the working directory ({} placed by the file table, {} \
         not), {} line(s) this reader could not read whole",
        census.processes,
        census.lines,
        census.relative,
        census.relative.saturating_sub(unresolved),
        unresolved,
        census.unparsed
    );
    // ATTRIBUTED AND BROKEN OUT, BECAUSE A RESIDUE NOBODY CAN PLACE IS A NUMBER.
    // A binary whose relative names went unresolved is a binary this census saw
    // less of than its other lines suggest; WHICH of the three kinds it is
    // decides what closing it would take, and they are three different pieces of
    // work.
    for (target, unplaced) in &census.unresolved {
        println!(
            "[outside-reach] UNRESOLVED {} name(s) — by {target}: {} under a \
             working directory this reader cannot name, {} under a descriptor it \
             never saw opened, {} after an argument it could not read. e.g. {}",
            unplaced.total(),
            unplaced.working,
            unplaced.descriptor,
            unplaced.unreadable,
            unplaced.examples.join(" · ")
        );
    }
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
