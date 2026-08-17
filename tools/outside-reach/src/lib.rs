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
//! # What this cannot see, said rather than hidden
//!
//! A path named RELATIVE to a directory file descriptor is recorded as the name
//! alone, and resolving it would need the descriptor table this trace does not
//! carry. That is not a hole in the census of TREES, which is what the
//! declaration is about — to read a file inside a tree, something must first
//! open the directory, and that open is absolute and is recorded. It is a hole
//! in the count of FILES, and [`Census::relative`] carries it so a reader is
//! never shown a total that quietly omitted some.
//!
//! A syscall strace could not render whole (`<unfinished ...>`) has no result
//! and is counted in [`Census::unparsed`] rather than dropped. THEY HAPPEN:
//! measured at 82 across the whole root suite — zero on the two smaller runs
//! this was first checked against, which is why the number is printed on every
//! run rather than written down once. A census that silently ignored them would
//! lose parentage, which loses attribution, which is the finding.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::BufRead;
use std::path::{Path, PathBuf};

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
    /// Paths a syscall named relative to a directory descriptor.
    pub relative: usize,
    /// Lines that look like a syscall and could not be read whole.
    pub unparsed: usize,
    /// Test binary → the paths it reached outside the ground.
    pub reaches: BTreeMap<String, BTreeSet<PathBuf>>,
}

impl Census {
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

/// The first quoted string on a line, with its escapes stepped over.
fn first_quoted(line: &str) -> Option<&str> {
    let start = line.find('"')? + 1;
    let bytes = line.as_bytes();
    let mut at = start;
    while at < bytes.len() {
        match bytes[at] {
            b'"' => return Some(&line[start..at]),
            b'\\' => at += 2,
            _ => at += 1,
        }
    }
    None
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

#[derive(Default)]
struct Traced {
    program: Option<String>,
    parent: Option<u64>,
    outside: BTreeSet<PathBuf>,
}

/// Read a `strace -f -e trace=%file,%process -e status=successful` stream.
///
/// THE WHOLE TRACE IS NEVER HELD. Each line is classified and dropped; what
/// survives is the set of paths reached and the parentage needed to attribute
/// them, which this repository measures in tens of kilobytes against 611 MB
/// passing through.
pub fn read_stream(stream: impl BufRead, ground: &Ground) -> Census {
    let mut traced: HashMap<u64, Traced> = HashMap::new();
    let mut census = Census::default();

    for line in stream.lines() {
        let Ok(line) = line else { continue };
        census.lines += 1;
        let Some((id, call)) = split_line(&line) else {
            continue;
        };
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
                if let Some(child) = spawned(call) {
                    traced.entry(child).or_default().parent = Some(id);
                }
            }
            "execve" | "execveat" => {
                if succeeded(call) {
                    let program = first_quoted(call).map(str::to_string);
                    let me = traced.entry(id).or_default();
                    if me.program.is_none() {
                        me.program = program;
                    }
                }
            }
            _ => {
                if !succeeded(call) {
                    continue;
                }
                match first_quoted(call) {
                    Some(named) if named.starts_with('/') => {
                        // NORMALISED BEFORE IT IS JUDGED, and before it is
                        // stored, so the ground check and the report are about
                        // the same spelling. R1231's census found eleven
                        // toolchain paths reported as reaches for want of this.
                        let named = normalise(Path::new(named));
                        if !ground.holds(&named) {
                            traced.entry(id).or_default().outside.insert(named);
                        }
                    }
                    Some(named) if !named.is_empty() => census.relative += 1,
                    _ => {}
                }
            }
        }
    }

    census.processes = traced.len();
    let reached: Vec<u64> = traced
        .iter()
        .filter_map(|(id, one)| (!one.outside.is_empty()).then_some(*id))
        .collect();
    for id in reached {
        let target = attribute(id, &traced);
        let paths = traced
            .get(&id)
            .map(|one| one.outside.clone())
            .unwrap_or_default();
        census.reaches.entry(target).or_default().extend(paths);
    }
    census
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
