//! Every path this repository's suite reads outside itself is one it declares.
//!
//! # Why a run, and not a reader of sources
//!
//! Round 1227 repaired a law that was true only on the machine that wrote it —
//! it asserted a relationship between two facts, one of which is a sibling
//! checkout, and CI said so one push later. The carry it left named the
//! population it had not looked at: *every case whose fixture reaches outside
//! its own tempdir*. That set cannot be read off the sources. A test that runs
//! `bash scripts/check-side-workspaces.sh` reaches whatever that script's own
//! children reach, and no Rust file in this repository spells those paths; the
//! deepest of them, on this machine, is a `Cargo.toml` inside a sibling
//! repository, opened by cargo, on behalf of a shell script, on behalf of a
//! test. Asking the sources answers "none of them".
//!
//! So the question is asked of a RUN. `strace` reports every path syscall the
//! run made; this reads them, decides for each whether the path is somewhere
//! the run is entitled to be, and attributes what is left to the test binary
//! that caused it.
//!
//! # Nothing is written to disk, and that is measured
//!
//! Round 1228 took the census by writing one file per process (`-ff`) and
//! reported 2.8 GB and 155,072 files as if that were the price of asking. It is
//! not: it was a choice of that round's, and the carry it left called the cost
//! the owner's to weigh. Re-measured the same day, `strace -o "|reader"` hands
//! the trace to a program through a PIPE, and the whole suite then costs
//! **611 MB passing through and ZERO bytes on disk**, with the suite's own
//! verdict unchanged at 2047 passed. What has to be held to the end is the SET
//! OF PATHS REACHED, tens of kilobytes, and never the trace itself.
//!
//! # `%file,%process` — and do not narrow it further
//!
//! Measured, on the one reach this repository already knew about: narrowing to
//! `openat,execve,clone` cuts the trace fivefold and finds **nothing**. The
//! sibling is reached by `newfstatat` — cargo asking whether a directory is
//! THERE, never opening it — so a census that watches opens answers zero, and
//! zero is what a clean tree looks like. `%file` is the requirement.
//!
//! `%process` is the other half and is a requirement for a different reason: a
//! Rust test is a THREAD, its reaches are recorded against its own id, and only
//! the clone lines lead back to the binary. A census that cannot say WHICH law
//! reached out is a number rather than a finding.
//!
//! The one filter that is free is `-e status=successful`: a failed call reached
//! nothing. Measured at 3.07 MB against 1.17 MB for the same run and the same
//! finding — most of what it drops is this machine's `PATH` being walked past
//! directories that do not exist.
//!
//! # What "outside" means
//!
//! Four grounds a run is entitled to stand on, and everything else is a reach:
//!
//!   * the repository, which is what the suite is about;
//!   * its build directory, which on this machine is a symlink OUT of the tree
//!     (`target -> ~/.buildcache/mnemosyne`) — so it has to be named separately
//!     rather than assumed to be `<repo>/target`;
//!   * the fixture root the run was given (`TMPDIR`), where a test's scratch
//!     lives;
//!   * the toolchain and the operating system — the facts every machine that
//!     can build this at all has, and the same ones a hosted runner has.
//!
//! A DIRECTORY ON THE WAY TO ONE OF THOSE IS NOT A REACH. `create_dir_all`
//! walks up to the first parent that exists and stats each one, so a fixture
//! root three levels down makes its own ancestors look like other trees. The
//! question is what a run reached INTO; an ancestor of a place it is allowed to
//! be is not another place.
//!
//! # The descriptor table is MODELLED, because the argument for not modelling
//! it was false
//!
//! R1233. Until this round a path named relative to a directory descriptor was
//! counted and dropped — 590,597 of them in one run, beside that run's 5,033,438
//! lines — and the reason written here was that it costs the census of FILES but
//! not the census of TREES, "because to read a file inside a tree, something
//! must first open the directory, and that open is absolute and is recorded".
//!
//! THAT ARGUMENT IS FALSE, and a two-line trace shows it: `openat(AT_FDCWD,
//! "/repo", O_DIRECTORY) = 3` followed by `openat(3, "../elsewhere/thing")`
//! reaches a tree NO absolute path in the trace ever names. `..` is what breaks
//! it — the descriptor is a place to start walking from, not a floor. The same
//! hole is open one step down: `chdir` moves the working directory and every
//! bare name afterwards is measured from there, which is how `/bin/sh -c 'cd
//! /etc && cat hostname'` reads `/etc/hostname` while the trace says only
//! `openat(AT_FDCWD, "hostname", O_RDONLY)`.
//!
//! So this carries the kernel objects the resolution actually depends on: a
//! FILE TABLE (`fd` → the path it was opened as) and a WORKING DIRECTORY, per
//! process, shared or copied at `clone` exactly as `CLONE_FILES` and `CLONE_FS`
//! say, and with the close-on-exec entries dropped at `execve`.
//!
//! EVERY GAP DEGRADES TO "UNRESOLVED", NEVER TO A WRONG PATH — that is the
//! design invariant, and it is what makes resolution safe to judge. A descriptor
//! this reader never saw opened resolves to nothing and is counted; a `dup2`
//! onto a number whose source is unknown REMOVES that number rather than leaving
//! the old binding under it; an `fchdir` to an unknown descriptor makes the
//! working directory unknown rather than stale. A false finding is the one
//! outcome worse than a missing one (R1232 measured eleven of them in a single
//! hosted run), so the model refuses before it guesses.
//!
//! THIS IS WHY THE TRACE ASKS FOR MORE THAN `%file,%process`. `close`, `dup`,
//! `dup2`, `dup3`, `close_range`, `fchdir` and `fcntl` take no filename, so none
//! of them is in `%file` — and without them a descriptor number can be
//! re-pointed where this reader cannot see it, which is the one way the model
//! could answer WRONG rather than UNKNOWN. They are named individually rather
//! than by taking `%desc`, which would add every `read` and `write` in the run.
//!
//! `fcntl` IS THE ONE NOBODY WOULD HAVE GUESSED, and it was not guessed. The
//! first whole-suite census under this model left 33,792 names unplaced in one
//! binary; the report said which KIND ("under a descriptor never seen opened")
//! and gave three SPELLINGS, and the first of them — `newfstatat(5, "src")` —
//! led to a `find(1)` that had reached descriptor 5 through
//! `fcntl(4, F_DUPFD_CLOEXEC, 3)`. That is how `fdopendir` gets its descriptor,
//! so it is how every tree walk in the run does.
//!
//! What is left unresolved after all of that is counted PER BINARY
//! ([`Census::unresolved`]) rather than as a lump, because a residue nobody can
//! attribute is a number rather than a finding — the same reason the reaches
//! themselves are attributed.
//!
//! A syscall strace could not render whole (`<unfinished ...>`) has no result
//! and is counted in [`Census::unparsed`] rather than dropped. THEY HAPPEN:
//! measured at 82 across the whole root suite — zero on the two smaller runs
//! this was first checked against, which is why the number is printed on every
//! run rather than written down once. A census that silently ignored them would
//! lose parentage, which loses attribution, which is the finding.

use std::cell::RefCell;
use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// The places a run is entitled to stand on.
#[derive(Debug, Clone, Default)]
pub struct Ground {
    /// Trees the run owns: the repository, its build directory, its fixture
    /// root. An ancestor of one of these is not a reach.
    pub owned: Vec<PathBuf>,
    /// The toolchain and the operating system. A prefix — and an ancestor of
    /// one of these is NOT excused, because `/home/<user>` sits above
    /// `~/.cargo` and excusing it would hide every dotfile beside it.
    pub toolchain: Vec<PathBuf>,
}

impl Ground {
    /// Is this path somewhere the run is entitled to be?
    #[must_use]
    pub fn holds(&self, path: &Path) -> bool {
        self.owned
            .iter()
            .any(|owned| path.starts_with(owned) || owned.starts_with(path))
            || self.toolchain.iter().any(|known| path.starts_with(known))
    }
}

/// What one traced run reached, keyed by the test binary that caused it.
#[derive(Debug, Clone, Default)]
pub struct Census {
    /// Processes and threads the trace holds.
    pub processes: usize,
    /// Lines read, so a census over an empty stream cannot look like a census
    /// over a clean run.
    pub lines: usize,
    /// Paths a syscall named relative to a descriptor or to the working
    /// directory — the whole of them, resolved or not.
    pub relative: usize,
    /// Test binary → what this model could not place, and WHAT KIND.
    ///
    /// The honest residue, attributed rather than lumped: a descriptor opened
    /// before the trace began, one handed over a socket, one whose `clone` line
    /// `strace` could not render whole. A run with a large residue is a run this
    /// census saw less of than its other numbers suggest, and the binary that
    /// carries it is where to look.
    pub unresolved: BTreeMap<String, Unplaced>,
    /// Lines that look like a syscall and could not be read whole.
    pub unparsed: usize,
    /// Test binary → the paths it reached outside the ground.
    pub reaches: BTreeMap<String, BTreeSet<PathBuf>>,
}

/// What a name this model could not place was measured FROM.
///
/// THREE KINDS, BECAUSE THEY ARE THREE DIFFERENT PIECES OF WORK. A residue of
/// descriptors nobody saw opened is closed by tracing what opens them; one of
/// unnamed working directories is closed by learning where a process stood; one
/// of arguments this reader could not read is closed by reading them. Reported
/// as one number they are a blind spot somebody has to re-measure before they
/// can act, which is the shape this repository keeps deleting — and the first
/// whole-suite run under the file table produced 391,418 of them in a single
/// test binary, a number that says nothing at all about which of the three it is.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Unplaced {
    /// Named against a working directory this reader cannot name.
    pub working: usize,
    /// Named under a descriptor it never saw opened.
    pub descriptor: usize,
    /// Named after an argument it could read as neither `AT_FDCWD`, a
    /// descriptor, nor an absent one.
    pub unreadable: usize,
    /// A FEW OF THEM, SPELLED AS THE TRACE SPELLED THEM — because a kind and a
    /// count still do not say WHICH CALL, and the difference decides whether a
    /// residue is a syscall nobody taught this reader or a descriptor genuinely
    /// beyond the trace. Held to three: this is a diagnosis, not a second census,
    /// and it must not grow with the run.
    pub examples: Vec<String>,
}

/// How many spellings of an unplaced name one binary carries into the report.
const EXAMPLES_KEPT: usize = 3;

impl Unplaced {
    /// All three together.
    #[must_use]
    pub fn total(&self) -> usize {
        self.working + self.descriptor + self.unreadable
    }

    /// Count one, by what it was measured from, and keep a few spellings.
    fn note(&mut self, base: Base, syscall: &str, named: &str) {
        match base {
            Base::Working => self.working += 1,
            Base::Descriptor(_) => self.descriptor += 1,
            Base::Unknown => self.unreadable += 1,
        }
        if self.examples.len() < EXAMPLES_KEPT {
            let from = match base {
                Base::Working => "AT_FDCWD".to_string(),
                Base::Descriptor(fd) => fd.to_string(),
                Base::Unknown => "?".to_string(),
            };
            // A path can be any length and this is a report line, so the name is
            // cut where it stops being a clue.
            let named: String = named.chars().take(60).collect();
            self.examples
                .push(format!("{syscall}({from}, \"{named}\")"));
        }
    }

    /// Take another process's counts and, while there is room, its spellings.
    fn absorb(&mut self, other: &Self) {
        self.working += other.working;
        self.descriptor += other.descriptor;
        self.unreadable += other.unreadable;
        for example in &other.examples {
            if self.examples.len() >= EXAMPLES_KEPT {
                break;
            }
            if !self.examples.contains(example) {
                self.examples.push(example.clone());
            }
        }
    }
}

impl Census {
    /// How many relative names went unplaced, over every binary.
    #[must_use]
    pub fn unresolved_total(&self) -> usize {
        self.unresolved.values().map(Unplaced::total).sum()
    }

    /// The trees reached, and which binaries reached each. A census of FILES
    /// answers "how much" where the question is "what".
    #[must_use]
    pub fn trees(&self, depth: usize) -> BTreeMap<PathBuf, BTreeSet<String>> {
        let mut out: BTreeMap<PathBuf, BTreeSet<String>> = BTreeMap::new();
        for (target, paths) in &self.reaches {
            for path in paths {
                out.entry(tree_of(path, depth))
                    .or_default()
                    .insert(target.clone());
            }
        }
        out
    }
}

/// The same file, spelled the one way this gate compares.
///
/// `/../lib/gcc/x86_64-linux-gnu/14/crtbegin.o` and
/// `/lib/gcc/x86_64-linux-gnu/14/crtbegin.o` are one file, and the first is what
/// a gcc driver actually writes: it builds search paths by concatenation and
/// leaves the `..` in. The first hosted run that carried this census reported
/// ELEVEN of its nineteen findings in that shape, every one of them inside the
/// toolchain the ground already names — because `starts_with("/lib")` is false
/// for a path whose first component after the root is `..`.
///
/// THE GATE WAS DISAGREEING WITH ITSELF ABOUT THE SAME PATH, which is how it was
/// found: [`tree_of`] drops non-`Normal` components, so the report printed
/// `REACH /lib/gcc` in one list and `(the cargo driver itself) reached
/// /../lib/gcc` in the other. One of those two readings had to be wrong for
/// both to be about one file.
///
/// LEXICAL, AND THAT IS A DECISION WITH A COST. Resolving `..` without asking
/// the filesystem is wrong exactly when a component is a SYMLINK, and this
/// reader cannot ask: a trace is historical, the run is over, and the machine it
/// ran on may not be this one. The alternative is to compare the unnormalised
/// spelling, which is what produced eleven findings about the toolchain — a
/// false finding is worse than a normalisation that is right about every path
/// with no symlinked parent.
#[must_use]
pub fn normalise(path: &Path) -> PathBuf {
    let mut out = PathBuf::from("/");
    for component in path.components() {
        match component {
            std::path::Component::Normal(name) => out.push(name),
            // `/..` IS `/`. Popping at the root is a no-op rather than an error,
            // which is what the kernel does with the same path.
            std::path::Component::ParentDir => {
                out.pop();
            }
            // `.` and the root itself add nothing; a prefix cannot occur here.
            _ => {}
        }
    }
    out
}

/// The first `depth` components of a path, which is the tree it is in.
#[must_use]
pub fn tree_of(path: &Path, depth: usize) -> PathBuf {
    let mut out = PathBuf::from("/");
    for component in path
        .components()
        .filter_map(|c| match c {
            std::path::Component::Normal(name) => Some(name),
            _ => None,
        })
        .take(depth)
    {
        out.push(component);
    }
    out
}

/// WHERE a declared tree is, said in terms no machine owns.
///
/// R1229 — AND THIS GATE HAD THE DISEASE IT WAS BUILT TO FIND. The first draft
/// wrote `/home/coin/pinion` and `/home/coin/.gitconfig` into the table, which
/// is true on the machine that wrote it and false on every other one: a hosted
/// runner's home is `/home/runner`, so the row for git's own configuration
/// would have matched nothing there and the census would have called git's
/// config an undeclared reach — a red on CI, from a gate whose entire subject
/// is claims that only hold where they were written. It was caught by asking
/// where this table would resolve on a runner, which is the question Round 1227
/// paid to learn to ask.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Where {
    /// Beside the repository — a sibling checkout. Resolved against the
    /// repository's own parent, so it follows the tree to any machine.
    Sibling(&'static str),
    /// Under the home of whoever is running, resolved against `HOME`.
    Home(&'static str),
}

impl Where {
    /// Resolve against the machine actually asking.
    #[must_use]
    pub fn resolve(self, repo: &Path, home: &Path) -> PathBuf {
        match self {
            Self::Sibling(name) => repo
                .parent()
                .map_or_else(|| PathBuf::from("/").join(name), |up| up.join(name)),
            Self::Home(under) => home.join(under),
        }
    }
}

/// A tree this repository's suite is allowed to read, and why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeclaredReach {
    /// Where the tree is, in terms that survive the move to another machine.
    pub at: Where,
    /// Why a law here reads it — a row somebody has to defend in review.
    pub why: &'static str,
    /// Whether the tree is on every machine that runs this suite.
    ///
    /// THE TWO ANSWERS ARE KEPT APART, for the reason Round 1115 kept "not
    /// ours" apart from "not on this machine": a declared reach that is absent
    /// here is not a stale row, and a gate that treated it as one would delete
    /// the sibling's row the first time it ran on a hosted runner.
    pub only_where_the_tree_exists: bool,
}

/// What the census says about the declaration.
#[derive(Debug, Clone, Default)]
pub struct Judgement {
    /// Reaches no row covers: `(the test binary, the path)`.
    pub undeclared: Vec<(String, PathBuf)>,
    /// Declared trees this run actually reached, as they resolved here.
    pub exercised: Vec<PathBuf>,
    /// Declared trees this run did NOT reach.
    ///
    /// PRINTED RATHER THAN PASSED OVER, which is Round 1227's lesson applied to
    /// this gate itself: on a runner the sibling is absent, so its row goes
    /// unexercised, and a green run there must not read as evidence about a
    /// reach it never made.
    pub unexercised: Vec<PathBuf>,
}

impl Judgement {
    /// Did every reach this run made have a row?
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.undeclared.is_empty()
    }
}

/// Hold a census against a declaration, on the machine actually asking.
///
/// The rows say WHERE in terms no machine owns; `repo` and `home` are what turn
/// that into paths this run can be held against.
#[must_use]
pub fn judge(census: &Census, declared: &[DeclaredReach], repo: &Path, home: &Path) -> Judgement {
    let resolved: Vec<PathBuf> = declared
        .iter()
        .map(|row| row.at.resolve(repo, home))
        .collect();
    let mut out = Judgement::default();
    let mut seen: BTreeSet<&PathBuf> = BTreeSet::new();
    for (target, paths) in &census.reaches {
        for path in paths {
            match resolved.iter().find(|tree| path.starts_with(tree)) {
                Some(tree) => {
                    seen.insert(tree);
                }
                None => out.undeclared.push((target.clone(), path.clone())),
            }
        }
    }
    for tree in &resolved {
        if seen.contains(tree) {
            out.exercised.push(tree.clone());
        } else {
            out.unexercised.push(tree.clone());
        }
    }
    out
}

/// One line of a `strace -f` stream, split into the thread that made the call
/// and the call itself.
///
/// The `-f` form prefixes every line with the id, which is what makes a single
/// stream as good as one file per process — and better, because it is a stream.
/// Lines that are not syscalls at all (`--- SIGCHLD …`, `+++ exited …`) have no
/// call and are skipped by the caller.
fn split_line(line: &str) -> Option<(u64, &str)> {
    let (id, rest) = line.split_once(' ')?;
    let id = id.parse().ok()?;
    Some((id, rest.trim_start()))
}

/// The `which`-th quoted string on a line, with its escapes stepped over, and
/// where its opening quote is — which is what says WHICH ARGUMENT it is, and so
/// what the argument before it was.
fn quoted(line: &str, which: usize) -> Option<(&str, usize)> {
    let bytes = line.as_bytes();
    let mut at = 0;
    let mut seen = 0;
    while at < bytes.len() {
        if bytes[at] != b'"' {
            at += 1;
            continue;
        }
        let open = at;
        let start = at + 1;
        let mut end = start;
        while end < bytes.len() {
            match bytes[end] {
                b'"' => break,
                b'\\' => end += 2,
                _ => end += 1,
            }
        }
        if end >= bytes.len() {
            return None;
        }
        if seen == which {
            return Some((&line[start..end], open));
        }
        seen += 1;
        at = end + 1;
    }
    None
}

/// Where a path argument is measured from: the argument BEFORE it.
///
/// This is one rule for the whole `…at` family rather than a table of syscalls,
/// and the trace is why it can be: `strace` renders `openat(AT_FDCWD, "x", …)`,
/// `newfstatat(3, "x", …)`, `renameat(4, "a", 5, "b")` and `symlinkat("t", 5,
/// "l")` all the same way — the descriptor is the argument immediately to the
/// left of the name. A call whose path IS its first argument (`chdir("/etc")`,
/// `open("x", …)`) has nothing to its left, and nothing to the left means the
/// working directory, which is again what the kernel does.
///
/// A QUOTED ARGUMENT TO THE LEFT IS NOT A DESCRIPTOR, and that case is real:
/// `symlink("../target", "link")` puts a string there. Anything else — a struct,
/// a flag name this reader does not know — is [`Base::Unknown`], which resolves
/// to nothing rather than to a guess.
fn base_before(call: &str, quote: usize) -> Base {
    let Some(open) = call.find('(') else {
        return Base::Unknown;
    };
    if quote <= open {
        return Base::Unknown;
    }
    let before = call[open + 1..quote].trim_end();
    let last = before
        .trim_end_matches(',')
        .rsplit(',')
        .next()
        .unwrap_or_default()
        .trim();
    if last.is_empty() || last == "AT_FDCWD" || last.starts_with('"') {
        return Base::Working;
    }
    match last.parse::<i32>() {
        Ok(fd) => Base::Descriptor(fd),
        Err(_) => Base::Unknown,
    }
}

/// The `which`-th argument of a call, read as a number.
///
/// `close_range(3, 4294967295, 0)` is why this is an `i64`: the upper bound is
/// routinely `~0U`, which is not an `i32`, and a reader that failed to parse it
/// would leave every descriptor above the floor bound when the process just
/// closed them all.
fn nth_number(call: &str, which: usize) -> Option<i64> {
    let open = call.find('(')?;
    let args = &call[open + 1..];
    let args = args.split(')').next().unwrap_or(args);
    args.split(',').nth(which)?.trim().parse().ok()
}

/// The descriptor a call returned, when the call is one that produces one.
fn returned_fd(call: &str) -> Option<i32> {
    call[call.rfind(" = ")? + 3..].trim().parse().ok()
}

/// `= 0` / `= 3` succeeded; `= -1 ENOENT (…)` and `= ?` did not.
fn succeeded(call: &str) -> bool {
    match call.rfind(" = ") {
        Some(at) => {
            let tail = call[at + 3..].trim_start();
            !tail.starts_with('-') && !tail.starts_with('?')
        }
        None => false,
    }
}

/// The syscall a line records, if it records one.
fn syscall_of(call: &str) -> Option<&str> {
    let open = call.find('(')?;
    let name = &call[..open];
    (!name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
        .then_some(name)
}

/// The child a clone/fork line produced.
fn spawned(call: &str) -> Option<u64> {
    call[call.rfind(" = ")? + 3..].trim().parse().ok()
}

/// Is this program a cargo test binary — `…/deps/<name>-<hex>`, no extension?
#[must_use]
pub fn test_binary_name(program: &str) -> Option<String> {
    if !program.contains("/deps/") {
        return None;
    }
    let file = program.rsplit('/').next()?;
    if file.contains('.') {
        return None;
    }
    let (name, hash) = file.rsplit_once('-')?;
    (hash.len() >= 8 && hash.chars().all(|c| c.is_ascii_hexdigit())).then(|| name.to_string())
}

/// What a reach is attributed to when nothing above it is a test binary — the
/// cargo process itself, and whatever it runs that is not a test.
pub const THE_DRIVER: &str = "(the cargo driver itself)";

/// What one descriptor names, and whether an `execve` closes it.
#[derive(Debug, Clone)]
struct OpenFile {
    at: PathBuf,
    cloexec: bool,
}

/// The kernel object a `CLONE_FILES` clone SHARES and a `fork` copies.
type Files = Rc<RefCell<HashMap<i32, OpenFile>>>;

/// The kernel object a `CLONE_FS` clone shares — the working directory, unknown
/// until something says otherwise.
type Cwd = Rc<RefCell<Option<PathBuf>>>;

/// Where a relative name is measured from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Base {
    /// The process's working directory — `AT_FDCWD`, or a call whose path is
    /// its first argument.
    Working,
    /// A directory descriptor.
    Descriptor(i32),
    /// An argument this reader could not read as either. Resolving from here
    /// would be a guess, so nothing does.
    Unknown,
}

#[derive(Default)]
struct Traced {
    program: Option<String>,
    parent: Option<u64>,
    outside: BTreeSet<PathBuf>,
    /// Relative names this process made that the model could not place, by what
    /// each was measured from.
    unresolved: Unplaced,
    files: Files,
    cwd: Cwd,
}

impl Traced {
    /// The absolute path a name denotes, or nothing when the model does not
    /// know — which is the only alternative it is allowed to produce.
    fn resolve(&self, base: Base, name: &str) -> Option<PathBuf> {
        if name.starts_with('/') {
            // An absolute name ignores the descriptor, exactly as the kernel
            // does, so a run whose working directory is unknown still has every
            // absolute reach judged.
            return Some(normalise(Path::new(name)));
        }
        let from = match base {
            Base::Working => self.cwd.borrow().clone()?,
            Base::Descriptor(fd) => self.files.borrow().get(&fd)?.at.clone(),
            Base::Unknown => return None,
        };
        Some(normalise(&from.join(name)))
    }
}

/// Read a `strace -f -e trace=%file,%process,close,close_range,dup,dup2,dup3,
/// fchdir,fcntl -e status=successful` stream.
///
/// THE WHOLE TRACE IS NEVER HELD. Each line is classified and dropped; what
/// survives is the set of paths reached, the parentage needed to attribute them,
/// and the descriptor table each process is holding AT THAT POINT of the stream
/// — which is bounded by what is open at once, not by what was ever opened,
/// because `close` is read. This repository measures the survivors in tens of
/// kilobytes against 611 MB passing through.
///
/// `started_in` is the directory the traced command was launched from, which is
/// the one fact about a run that the run's own syscalls never state. Without it
/// every bare name is unresolvable — honest, and blind to `cd /etc && cat
/// hostname`. It is a parameter rather than the repository path because they are
/// different facts that merely coincide in this repository's wiring, and a model
/// that conflated them would resolve against the wrong directory the first time
/// somebody traced a command from elsewhere. Seeded onto the FIRST process the
/// stream names, and inherited from there by the clone lines.
pub fn read_stream(stream: impl BufRead, ground: &Ground, started_in: Option<&Path>) -> Census {
    let mut traced: HashMap<u64, Traced> = HashMap::new();
    let mut census = Census::default();
    let mut seeded = started_in.is_none();

    for line in stream.lines() {
        let Ok(line) = line else { continue };
        census.lines += 1;
        let Some((id, call)) = split_line(&line) else {
            continue;
        };
        if !seeded {
            seeded = true;
            if let Some(dir) = started_in {
                *traced.entry(id).or_default().cwd.borrow_mut() = Some(normalise(dir));
            }
        }
        // `--- SIGCHLD …` and `+++ exited …` are events rather than calls.
        if call.starts_with("---") || call.starts_with("+++") {
            continue;
        }
        // A CALL SPLIT ACROSS LINES IS CAUGHT BEFORE IT IS DISPATCHED, and that
        // order is the whole of it. The first half of a split call still LOOKS
        // like a syscall — `clone(child_stack=NULL, flags=… <unfinished ...>`
        // parses as `clone` — so a reader that named it first and checked
        // afterwards would hand it to the clone arm, find no return value, and
        // drop it in silence. That is the parentage of a whole subtree gone,
        // which is attribution gone, which is the finding gone. The second half
        // (`<... clone resumed>) = 101`) has no name at all and would have been
        // counted either way; it is the FIRST half that hides.
        if call.contains("<unfinished ...>") || call.starts_with("<...") {
            census.unparsed += 1;
            continue;
        }
        let Some(name) = syscall_of(call) else {
            continue;
        };
        match name {
            "clone" | "clone3" | "fork" | "vfork" => {
                let Some(child) = spawned(call) else { continue };
                // WHAT THE CHILD INHERITS IS WHAT THE FLAGS SAY. `CLONE_FILES`
                // and `CLONE_FS` are the difference between a thread — which
                // sees every descriptor its siblings open AFTER this line — and
                // a `fork`, which sees a copy frozen here. A model that always
                // shared would resolve a forked child's `openat(3, …)` against
                // a file only its parent went on to open; one that always copied
                // would fail to resolve a thread's, which is most of a cargo
                // run.
                let flags = &call[..call.rfind(" = ").unwrap_or(call.len())];
                let shares_files = flags.contains("CLONE_FILES");
                let shares_cwd = flags.contains("CLONE_FS");
                let (parent_files, parent_cwd) = {
                    let parent = traced.entry(id).or_default();
                    (Rc::clone(&parent.files), Rc::clone(&parent.cwd))
                };
                match traced.entry(child) {
                    Entry::Vacant(slot) => {
                        let files = if shares_files {
                            Rc::clone(&parent_files)
                        } else {
                            Rc::new(RefCell::new(parent_files.borrow().clone()))
                        };
                        let cwd = if shares_cwd {
                            Rc::clone(&parent_cwd)
                        } else {
                            Rc::new(RefCell::new(parent_cwd.borrow().clone()))
                        };
                        slot.insert(Traced {
                            parent: Some(id),
                            files,
                            cwd,
                            ..Traced::default()
                        });
                    }
                    // ALREADY SPEAKING BEFORE ITS OWN CLONE LINE ARRIVED, which
                    // is not an oddity but the NORMAL shape of a spawn: a
                    // `vfork` parent is blocked until its child `execve`s, so
                    // the child's `dup2`, `chdir` and `execve` are all printed
                    // BEFORE the parent's `clone3(…) = <pid>` returns. Measured
                    // in this repository's own suite, and it is what the first
                    // whole-suite census under this model was reporting: 433,904
                    // names unplaced, 391,762 of them under descriptors "never
                    // seen opened" that the child had in fact inherited.
                    Entry::Occupied(mut slot) => {
                        let child = slot.get_mut();
                        child.parent = Some(id);
                        adopt(child, &parent_files, &parent_cwd, shares_files, shares_cwd);
                    }
                }
            }
            "execve" | "execveat" => {
                if !succeeded(call) {
                    continue;
                }
                let Some((named, at)) = quoted(call, 0) else {
                    continue;
                };
                let base = base_before(call, at);
                let me = traced.entry(id).or_default();
                if me.program.is_none() {
                    me.program = Some(named.to_string());
                }
                // CLOSE-ON-EXEC IS WHAT THE NAME SAYS. The kernel closes those
                // descriptors here, and a model that kept them would resolve a
                // later `openat(3, …)` against a file this process can no longer
                // see — the model answering WRONG where it is required to answer
                // UNKNOWN.
                me.files.borrow_mut().retain(|_, file| !file.cloexec);
                if !named.starts_with('/') {
                    census.relative += 1;
                }
                // A PROGRAM A RUN EXECUTES IS A FILE IT READ (R1233). This arm
                // took the name and judged nothing, so a binary executed from
                // outside the ground was a reach the census could not report.
                match me.resolve(base, named) {
                    Some(path) => {
                        if !ground.holds(&path) {
                            me.outside.insert(path);
                        }
                    }
                    None => me.unresolved.note(base, name, named),
                }
            }
            // ---------------------------------------------- the file table
            // None of these seven takes a filename, so none is in `%file`; they
            // are asked for by name because without them a descriptor number can
            // be re-pointed where this reader cannot see it.
            "close" => {
                if succeeded(call) {
                    if let Some(fd) = nth_fd(call, 0) {
                        traced.entry(id).or_default().files.borrow_mut().remove(&fd);
                    }
                }
            }
            "close_range" => {
                if succeeded(call) {
                    if let (Some(low), Some(high)) = (nth_number(call, 0), nth_number(call, 1)) {
                        traced
                            .entry(id)
                            .or_default()
                            .files
                            .borrow_mut()
                            .retain(|fd, _| i64::from(*fd) < low || i64::from(*fd) > high);
                    }
                }
            }
            "dup" | "dup2" | "dup3" => {
                if !succeeded(call) {
                    continue;
                }
                let (Some(source), Some(new)) = (nth_fd(call, 0), returned_fd(call)) else {
                    continue;
                };
                // `dup` and `dup2` clear close-on-exec; `dup3` sets it from its
                // own flag argument.
                let cloexec = name == "dup3" && call.contains("O_CLOEXEC");
                duplicate(traced.entry(id).or_default(), source, new, cloexec);
            }
            // `fcntl` IS HOW A DESCRIPTOR IS USUALLY DUPLICATED, which this
            // reader learned from the residue rather than from the manual.
            // `find(1)` — and everything else that walks a tree through
            // `fdopendir` — opens a directory and immediately does
            // `fcntl(4, F_DUPFD_CLOEXEC, 3) = 5`, then names everything under
            // FIVE. Measured: 33,791 of one run's 33,792 unplaced names were
            // that descriptor, and the trace did not ask for `fcntl`.
            //
            // `F_SETFD` is read for the same reason `O_CLOEXEC` is: it is how a
            // descriptor's survival across `execve` is decided after the fact.
            "fcntl" => {
                if !succeeded(call) {
                    continue;
                }
                let Some(source) = nth_fd(call, 0) else {
                    continue;
                };
                let command = call.split(',').nth(1).unwrap_or_default();
                if command.contains("F_DUPFD") {
                    let Some(new) = returned_fd(call) else {
                        continue;
                    };
                    let cloexec = command.contains("F_DUPFD_CLOEXEC");
                    duplicate(traced.entry(id).or_default(), source, new, cloexec);
                } else if command.contains("F_SETFD") {
                    let cloexec = call.contains("FD_CLOEXEC");
                    if let Some(file) = traced
                        .entry(id)
                        .or_default()
                        .files
                        .borrow_mut()
                        .get_mut(&source)
                    {
                        file.cloexec = cloexec;
                    }
                }
            }
            "fchdir" => {
                if !succeeded(call) {
                    continue;
                }
                let me = traced.entry(id).or_default();
                let to = nth_fd(call, 0).and_then(|fd| {
                    let held = me.files.borrow();
                    held.get(&fd).map(|file| file.at.clone())
                });
                // UNKNOWN, NEVER STALE: a working directory this reader cannot
                // name makes every later bare name unresolvable, which is the
                // honest answer. Keeping the old one would make them wrong.
                *me.cwd.borrow_mut() = to;
            }
            _ => {
                if !succeeded(call) {
                    continue;
                }
                // `symlink`'s FIRST argument is the content of the link — a
                // string the run never reaches — and the path it does reach is
                // the second.
                let which = usize::from(matches!(name, "symlink" | "symlinkat"));
                let Some((named, at)) = quoted(call, which) else {
                    continue;
                };
                let base = base_before(call, at);
                let me = traced.entry(id).or_default();
                if !named.starts_with('/') {
                    census.relative += 1;
                }
                // NORMALISED BEFORE IT IS JUDGED, and before it is stored, so
                // the ground check and the report are about the same spelling.
                // R1231's census found eleven toolchain paths reported as
                // reaches for want of this; `resolve` is where it happens now,
                // because a name joined onto a descriptor's path is exactly
                // where `..` arrives.
                let resolved = me.resolve(base, named);
                if name == "chdir" {
                    // Including when it is `None`, which is the same rule as
                    // `fchdir`: a directory this reader cannot name is unknown.
                    *me.cwd.borrow_mut() = resolved.clone();
                } else if name == "getcwd" && !named.ends_with(" (deleted)") {
                    // THE KERNEL ANSWERING THE QUESTION THIS READER CANNOT ASK.
                    // `getcwd` takes no filename — it RETURNS one, into a buffer
                    // `strace` renders as the first quoted argument — and it is
                    // in `%file` already. Any process that asks it says where it
                    // stands, which repairs a working directory whose descent
                    // from the launch directory was broken. Measured: 41,828 of
                    // one whole-suite census's unplaced names were the cargo
                    // driver's, under a directory nothing had named.
                    //
                    // EXCEPT WHEN THE ANSWER IS NOT A PATH. A process whose
                    // working directory has been removed is told
                    // `"/tmp/x (deleted)"`, and a test that deletes its own
                    // fixture while something stands in it produces exactly
                    // that. Taking it would measure every later name from a
                    // directory that does not exist under a name nothing has —
                    // the model answering WRONG where UNKNOWN is required.
                    *me.cwd.borrow_mut() = resolved.clone();
                }
                let Some(path) = resolved else {
                    me.unresolved.note(base, name, named);
                    continue;
                };
                if matches!(name, "open" | "openat" | "openat2" | "creat") {
                    if let Some(fd) = returned_fd(call) {
                        // Read from the flags, which are to the RIGHT of the
                        // name — a FILE whose own name held the word would
                        // otherwise decide whether an `execve` closes it.
                        let cloexec = call[at + 1 + named.len()..].contains("O_CLOEXEC");
                        me.files.borrow_mut().insert(
                            fd,
                            OpenFile {
                                at: path.clone(),
                                cloexec,
                            },
                        );
                    }
                }
                if !ground.holds(&path) {
                    me.outside.insert(path);
                }
            }
        }
    }

    census.processes = traced.len();
    let noted: Vec<u64> = traced
        .iter()
        .filter_map(|(id, one)| {
            (!one.outside.is_empty() || one.unresolved.total() > 0).then_some(*id)
        })
        .collect();
    for id in noted {
        let target = attribute(id, &traced);
        let Some(one) = traced.get(&id) else { continue };
        let (paths, unresolved) = (one.outside.clone(), one.unresolved.clone());
        if !paths.is_empty() {
            census
                .reaches
                .entry(target.clone())
                .or_default()
                .extend(paths);
        }
        if unresolved.total() > 0 {
            census
                .unresolved
                .entry(target)
                .or_default()
                .absorb(&unresolved);
        }
    }
    census
}

/// The `which`-th argument of a call, read as a descriptor.
fn nth_fd(call: &str, which: usize) -> Option<i32> {
    nth_number(call, which).and_then(|n| i32::try_from(n).ok())
}

/// Point one descriptor at what another names — or, when the source is unknown,
/// FORGET the target rather than leave the old binding under it.
///
/// `dup2(10, 1) = 1` from a shell that opened descriptor 10 before the trace
/// began re-points 1 at something nothing here can name, and the binding held
/// under 1 is now a lie. This is the difference between the model answering
/// UNKNOWN and answering WRONG.
fn duplicate(me: &mut Traced, source: i32, new: i32, cloexec: bool) {
    let held = me.files.borrow().get(&source).cloned();
    match held {
        Some(mut file) => {
            file.cloexec = cloexec;
            me.files.borrow_mut().insert(new, file);
        }
        None => {
            me.files.borrow_mut().remove(&new);
        }
    }
}

/// Give a process what it inherited, when its `clone` line arrives after it has
/// already spoken.
///
/// WHY THIS IS THE COMMON CASE AND NOT THE ODD ONE: a `vfork`/`posix_spawn`
/// parent is BLOCKED until its child `execve`s, so `strace` prints the child's
/// `dup2`, `chdir` and `execve` before it can print the parent's
/// `clone3(…) = <pid>`. A reader that treated the child's empty table as its own
/// state gives every such process — and, in a test binary, every thread whose
/// clone line loses the race — a table with nothing in it, so each name it
/// afterwards gives under an inherited descriptor is counted as residue. That
/// was 391,762 of one whole-suite census's 433,904 unplaced names.
///
/// ADOPTING IS SAFE, and the reason is the same fact: a parent makes NO other
/// syscall between calling `clone` and that call returning, so the state this
/// reads at the clone line IS the state at the moment of the clone. What the
/// child learned for itself in the meantime is newer, so it wins — a gap is
/// filled, never overwritten.
fn adopt(child: &mut Traced, files: &Files, cwd: &Cwd, shares_files: bool, shares_cwd: bool) {
    // A shared object adopted twice would be borrowed twice below. It cannot
    // happen from one clone line per child, and a gate that panicked on data
    // would be worse than one that did nothing here.
    if !Rc::ptr_eq(&child.files, files) {
        if shares_files {
            // The child's own opens went into the SHARED table all along, so
            // they move there and the child points at it.
            let mine = std::mem::take(&mut *child.files.borrow_mut());
            files.borrow_mut().extend(mine);
            child.files = Rc::clone(files);
        } else {
            // A copy frozen at the clone: only the numbers the child has not
            // bound for itself.
            let theirs = files.borrow().clone();
            let mut mine = child.files.borrow_mut();
            for (fd, file) in theirs {
                mine.entry(fd).or_insert(file);
            }
        }
    }
    if shares_cwd {
        // `CLONE_FS` IS ONE OBJECT AND IT RUNS BOTH WAYS: a `chdir` in either
        // moves both. The child spoke AFTER the clone and the parent was blocked
        // in it, so a directory the child has named is the newer answer for both
        // — taking the parent's here would put the pair back where the child had
        // already left.
        let learned = child.cwd.borrow().clone();
        if learned.is_some() {
            *cwd.borrow_mut() = learned;
        }
        child.cwd = Rc::clone(cwd);
    } else if child.cwd.borrow().is_none() {
        // A copy frozen at the clone: fill the gap, never overwrite.
        let inherited = cwd.borrow().clone();
        *child.cwd.borrow_mut() = inherited;
    }
}

/// The test binary a thread belongs to: the nearest ancestor that is one.
fn attribute(mut id: u64, traced: &HashMap<u64, Traced>) -> String {
    // BOUNDED, because a trace is data and a cycle in it must not hang a gate.
    // The bound is far above any process tree cargo builds.
    for _ in 0..1024 {
        let Some(one) = traced.get(&id) else { break };
        if let Some(name) = one.program.as_deref().and_then(test_binary_name) {
            return name;
        }
        match one.parent {
            Some(up) => id = up,
            None => break,
        }
    }
    THE_DRIVER.to_string()
}
