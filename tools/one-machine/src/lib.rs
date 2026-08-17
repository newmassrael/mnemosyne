//! A census verdict this repository acts on was given by a machine that is not
//! this one, about the tree that is asking.
//!
//! # Why the machine is the subject
//!
//! `tools/outside-reach` asks what a run reached outside this repository. Its
//! answer is not a property of the source: it is a property of the source AND
//! the machine, and the four defects this repository has caught in it were all
//! found where its author was not sitting — a hosted runner's `HOME`, a gcc
//! driver's spelling of a library path, a `find(1)` reaching a descriptor
//! through `fcntl(F_DUPFD_CLOEXEC)` in a population no workstation run
//! reproduces. A gate like that, verified on one machine, has an untested
//! premise; and the two arms that found the last two defects were scripts a
//! person launched by hand.
//!
//! # What is asked, and of whom
//!
//! Nothing here re-implements a decision another program owns.
//!
//!   * WHERE the census runs is `bx`'s decision — the machine-wide placement
//!     program, which knows the fleet, the locks and the memory budget this
//!     repository declares. It is ASKED (`--explain-choice`, which runs
//!     nothing) rather than second-guessed, and its `BX_FLEET` seam is what
//!     makes "a host that is busy" an argument instead of a wait.
//!   * WHAT the census runs is the declaration's, `[commands] verify` in
//!     `.claude/remote-build.toml` — the same value the laws that read every
//!     cargo command this repository issues already have in their population.
//!   * WHICH SYSCALLS the trace carries is the reader's, asked of
//!     `outside-reach --trace-filter`. R1233 is the argument: the filter was one
//!     call short of the model for as long as the model existed.
//!
//! What is decided HERE is the one question none of them answer: whether the
//! answer in hand came from somewhere else, and is about this tree.
//!
//! # Three answers, and the third is why this exists
//!
//! | code | meaning |
//! |---|---|
//! | 0 | a machine that is not this one took the census of this tree and it was clean |
//! | 1 | it took it and found something |
//! | 2 | NOT JUDGED — no machine has answered about this tree |
//!
//! Every state in the third row reports zero findings under a two-code gate, and
//! zero findings is what a clean tree looks like: nothing was ever dispatched;
//! the placement program would have run it HERE, so the second opinion is the
//! first one again; the answer is about a tree with different bytes in it; the
//! run is still going. Those are four different repairs and they are named
//! apart.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Where this machine records what it dispatched, relative to a repository root.
///
/// UNDER `target/`, which is gitignored and collected: the claim is one file,
/// rewritten by each dispatch rather than accumulated, and `scratch-budget`
/// carries an entry for the directory anyway. A record directory that grows
/// without a collector is a defect this repository has already paid for once.
pub const CLAIM: &str = "target/one-machine/claim";

/// The declaration that says how this repository is verified on a build machine.
pub const DECLARATION: &str = ".claude/remote-build.toml";

/// The placement program, machine-wide and outside every checkout — which is the
/// asymmetry that makes it the right thing to ask rather than to copy.
pub const PROGRAM_UNDER_HOME: &str = ".claude/remote-build/bin/bx";

/// The script the dispatched machine runs, relative to a repository root.
pub const CENSUS_SCRIPT: &str = "scripts/census-elsewhere.sh";

/// The first word of every line this crate writes into, or reads out of, a
/// census log. One tag, so a log that is not ours is not half-read.
pub const TAG: &str = "one-machine";

/// The word the LAUNCHER appends when the run it wrapped has ended.
///
/// Rule 6 of the machine-wide remote-build protocol, and it is the launcher's
/// rather than the script's on purpose: a script that declares its own success
/// is printing a line, and one that dies halfway prints nothing at all. This
/// value is the kernel's, and it survives the script dying.
pub const SENTINEL: &str = "REMOTE_BUILD_EXIT=";

/// Where the dispatched machine writes its log, as the REMOTE shell spells it.
///
/// `$HOME` is left for the far side to expand, deliberately: this program cannot
/// know the remote account, and a path assembled from a local `HOME` would name
/// a directory on the wrong machine. Both the launcher and the fetch spell it
/// this way, so they are one path rather than two that agree today.
#[must_use]
pub fn remote_log(repository_name: &str) -> String {
    format!("$HOME/.remote-build/{TAG}/{repository_name}.log")
}

/// Where the placement program would run a command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placement {
    /// Whether it chose a machine other than this one.
    pub remote: bool,
    /// The alias it chose, when it chose one.
    pub host: Option<String>,
    /// Its own sentence about why — carried through rather than re-derived, so
    /// a refusal here quotes the program that decided instead of guessing.
    pub why: String,
}

/// Read what the placement program answered when asked where a command goes.
///
/// # Errors
///
/// When the output carries no `where=` line, which is what a program without
/// the seam looks like — a refusal rather than an empty answer.
pub fn read_placement(stdout: &str) -> Result<Placement, String> {
    let mut placement: Option<Placement> = None;
    let mut why = String::new();
    for line in stdout.lines() {
        if let Some(rest) = line.strip_prefix("why=") {
            why = rest.trim().to_owned();
        }
        if !line.starts_with("where=") {
            continue;
        }
        let mut remote = false;
        let mut host = None;
        for field in line.split_whitespace() {
            if let Some(value) = field.strip_prefix("where=") {
                remote = value == "remote";
            }
            if let Some(value) = field.strip_prefix("host=") {
                if !value.is_empty() {
                    host = Some(value.to_owned());
                }
            }
        }
        placement = Some(Placement {
            remote,
            host,
            why: String::new(),
        });
    }
    match placement {
        Some(mut found) => {
            found.why = why;
            Ok(found)
        }
        None => Err("it printed no `where=` line — this program does not have \
             `--explain-choice`, so where it would put a command cannot be asked \
             without running one"
            .to_owned()),
    }
}

/// What this machine dispatched, and where the answer will be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    /// The alias the placement program chose.
    pub host: String,
    /// The log path, as the REMOTE shell spells it.
    pub log: String,
    /// The tree that was sent, as [`fingerprint`] names it.
    pub fingerprint: String,
    /// When, in seconds since the epoch — a number rather than a rendering,
    /// because the only question asked of it is how long ago.
    pub launched: u64,
}

impl Claim {
    /// The claim as the one line this crate writes and reads.
    #[must_use]
    pub fn render(&self) -> String {
        format!(
            "{TAG} claim host={} log={} fingerprint={} launched={}\n",
            self.host, self.log, self.fingerprint, self.launched
        )
    }

    /// Read a claim back.
    ///
    /// # Errors
    ///
    /// When the text carries no claim line, or one missing a field. A claim
    /// this cannot read is NOT an absent one: absent means nothing was ever
    /// dispatched, unreadable means something was and this cannot say what.
    pub fn read(text: &str) -> Result<Self, String> {
        let line = text
            .lines()
            .find(|line| line.starts_with(&format!("{TAG} claim ")))
            .ok_or_else(|| format!("no `{TAG} claim` line in it"))?;
        let fields = fields_of(line);
        let get = |name: &str| -> Result<String, String> {
            fields
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.clone())
                .ok_or_else(|| format!("the claim line names no `{name}`"))
        };
        let launched = get("launched")?;
        Ok(Self {
            host: get("host")?,
            log: get("log")?,
            fingerprint: get("fingerprint")?,
            launched: launched
                .parse()
                .map_err(|_| format!("`launched={launched}` is not a number of seconds"))?,
        })
    }
}

/// What the machine that took the census said about itself before taking it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    /// That machine's own name for itself — the field this whole gate turns on.
    pub host: String,
    /// The tree it was standing in, as [`fingerprint`] names it.
    pub fingerprint: String,
    /// When it started, in seconds since the epoch.
    pub started: u64,
}

impl Header {
    /// The header as the one line this crate writes and reads.
    #[must_use]
    pub fn render(&self) -> String {
        format!(
            "{TAG} header host={} fingerprint={} started={}",
            self.host, self.fingerprint, self.started
        )
    }
}

fn fields_of(line: &str) -> Vec<(String, String)> {
    line.split_whitespace()
        .filter_map(|field| {
            field
                .split_once('=')
                .map(|(key, value)| (key.to_owned(), value.to_owned()))
        })
        .collect()
}

/// Everything a log says, read once so the judgement below is a decision rather
/// than a scan.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Answer {
    /// The header, when the log carries one.
    pub header: Option<Header>,
    /// What the suite under the census exited with, when the log says.
    pub suite: Option<i64>,
    /// What the census reader exited with, when the log says.
    pub census: Option<i64>,
    /// The launcher's sentinel — the kernel's word that the run has ENDED.
    pub sentinel: Option<i64>,
}

/// Read a census log.
#[must_use]
pub fn read_log(text: &str) -> Answer {
    let mut answer = Answer::default();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix(SENTINEL) {
            answer.sentinel = rest.trim().parse().ok();
            continue;
        }
        // THE SENTINEL IS ALSO LOOKED FOR MID-LINE, because the transport can
        // leave the cursor where the wrapped command left it and the launcher's
        // append then lands on the tail of another line. The remote-build
        // protocol records that exact shape; a reader that only anchors at the
        // start of a line answers "still running" about a run that ended.
        if let Some(at) = line.find(SENTINEL) {
            if let Ok(code) = line[at + SENTINEL.len()..].trim().parse() {
                answer.sentinel = Some(code);
                continue;
            }
        }
        let Some(rest) = line.strip_prefix(&format!("{TAG} ")) else {
            continue;
        };
        let fields = fields_of(rest);
        let value = |name: &str| -> Option<String> {
            fields
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.clone())
        };
        if rest.starts_with("header ") {
            if let (Some(host), Some(fingerprint), Some(started)) =
                (value("host"), value("fingerprint"), value("started"))
            {
                if let Ok(started) = started.parse() {
                    answer.header = Some(Header {
                        host,
                        fingerprint,
                        started,
                    });
                }
            }
        } else if rest.starts_with("suite ") {
            answer.suite = value("exit").and_then(|code| code.parse().ok());
        } else if rest.starts_with("census ") {
            answer.census = value("exit").and_then(|code| code.parse().ok());
        }
    }
    answer
}

/// The words a census log carries besides the run it watched.
///
/// A failure the far side's SUITE reported is part of this gate's answer — the
/// census is a reading of that run — so the markers a suite fails with are here
/// beside the census's own prose.
const FAILURE_MARKERS: &[&str] = &[
    "panicked at",
    "test result: FAILED",
    "error[",
    "error:",
    " FAILED",
];

fn carries_the_answer(line: &str) -> bool {
    line.starts_with(TAG)
        || line.contains("[outside-reach]")
        || line.contains(SENTINEL)
        || FAILURE_MARKERS.iter().any(|marker| line.contains(marker))
}

/// The lines of a census log that carry its answer.
///
/// A LOG IS THOUSANDS OF LINES AND THE ANSWER IS TENS OF THEM. Measured on two
/// real ones: 2,766 lines against 17, and 2,815 against 23 on the run whose suite
/// failed. Both callers of this gate are git hooks, so printing the whole thing
/// does not carry the answer to the person pushing — it buries it under the
/// ordinary output of a passing suite. `scripts/verify.sh` met the same problem
/// and the shape is borrowed from it: the lines that decide, and a sentence
/// saying where the rest still is.
///
/// NOT A SUMMARY, and the distinction is the whole point. Every line returned is
/// the far side's own, unedited and in order; what is left out is named, and it
/// is still on the machine that wrote it, reachable by the command this gate
/// prints. A gate that PARAPHRASED another machine's finding would be the second
/// spelling this repository keeps deleting.
#[must_use]
pub fn carrying_lines(log: &str) -> Vec<&str> {
    log.lines()
        .filter(|line| carries_the_answer(line))
        .collect()
}

/// The census's own count of what it looked at, when the log carries one.
///
/// PRINTED ON A CLEAN VERDICT, because "nothing undeclared" is what a census over
/// an empty trace says too. The number of processes and lines is the far side's
/// own evidence that it looked at anything at all, and one line of it costs
/// nothing beside a verdict somebody is about to trust.
#[must_use]
pub fn scale_of(log: &str) -> Option<&str> {
    log.lines()
        .find(|line| line.contains("[outside-reach]") && line.contains("process(es)"))
}

/// What this gate concluded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// A machine that is not this one took the census of this tree, and it was
    /// clean.
    Elsewhere { host: String, seconds: u64 },
    /// It took it, and something is wrong.
    Finding(String),
    /// There is no opinion here, and this says which of the several ways.
    NotJudged(String),
}

impl Verdict {
    /// The exit status, which is the whole contract with the callers.
    #[must_use]
    pub fn code(&self) -> u8 {
        match self {
            Self::Elsewhere { .. } => 0,
            Self::Finding(_) => 1,
            Self::NotJudged(_) => 2,
        }
    }
}

/// Everything the judgement is a function of.
#[derive(Debug, Clone)]
pub struct Seen {
    /// What this machine dispatched, when it dispatched anything. `Err` is a
    /// claim that exists and could not be read, which is not the same as none.
    pub claim: Option<Result<Claim, String>>,
    /// The log from the machine that was dispatched to, or why it could not be
    /// fetched.
    pub log: Result<String, String>,
    /// The fingerprint of the tree asking, NOW.
    pub here: String,
    /// This machine's own name for itself.
    pub this_host: String,
}

/// Decide whether a census answer in hand is one this repository may act on.
///
/// # The order of the checks is the order of the repairs
///
/// Each arm below is reached only when the ones above it passed, so the sentence
/// a caller reads names the FIRST thing that is missing rather than the last.
/// Identity before freshness before completion: being told "that census is about
/// another tree" when the census was taken on this very machine would send the
/// reader to re-dispatch, and re-dispatching does not fix a gate that is its own
/// second opinion.
#[must_use]
pub fn judge(seen: &Seen) -> Verdict {
    let claim = match &seen.claim {
        None => {
            return Verdict::NotJudged(format!(
                "no machine has been asked about this tree — there is no {CLAIM} here, so \
                 the only census of it is the one taken where this gate was written"
            ))
        }
        Some(Err(why)) => {
            return Verdict::NotJudged(format!(
                "a dispatch was recorded and cannot be read ({why}) — something was sent \
                 and this cannot say to where, which is not the same as nothing having been"
            ))
        }
        Some(Ok(claim)) => claim,
    };
    let log = match &seen.log {
        Err(why) => {
            return Verdict::NotJudged(format!(
                "the census log on `{}` could not be read — {why}. The run may be gone, the \
                 machine may be, or the transport is",
                claim.host
            ))
        }
        Ok(text) => text,
    };
    let answer = read_log(log);
    let Some(header) = answer.header else {
        return Verdict::NotJudged(format!(
            "`{}` answered with {} byte(s) that carry no `{TAG} header` line — this is not a \
             census of ours, so nothing in it is evidence about this tree",
            claim.host,
            log.len()
        ));
    };
    // THE MACHINE, FIRST AND BEFORE ANYTHING ELSE. This is the whole premise:
    // an answer from the machine that wrote the gate is the answer the gate
    // already had. It is compared against the HEADER rather than against the
    // claim's alias, because an alias is a name in an ssh config and two of them
    // can point at one host.
    if header.host.eq_ignore_ascii_case(&seen.this_host) {
        return Verdict::NotJudged(format!(
            "the census in hand was taken on `{}`, which is THIS machine — a gate cannot be \
             its own second opinion, and every defect this one has had was found somewhere \
             its author was not",
            header.host
        ));
    }
    // THE TREE THAT ANSWERED IS THE TREE THAT WAS SENT. A disagreement here is
    // the transport's, not the round's: the bytes that arrived are not the bytes
    // that left, and re-dispatching a tree whose copy differs would answer about
    // the copy again.
    if header.fingerprint != claim.fingerprint {
        return Verdict::NotJudged(format!(
            "`{}` took the census of a tree this machine did not send it: it read {}, the \
             dispatch recorded {}. What arrived is not what left",
            claim.host, header.fingerprint, claim.fingerprint
        ));
    }
    if header.fingerprint != seen.here {
        return Verdict::NotJudged(format!(
            "that census is about another tree — it read {}, this tree is {} now. A verdict \
             about bytes that have changed is not a verdict about these ones",
            header.fingerprint, seen.here
        ));
    }
    let Some(sentinel) = answer.sentinel else {
        return Verdict::NotJudged(format!(
            "the census on `{}` has not ended: its log carries no `{SENTINEL}` line, which is \
             the only thing that says a detached run is over. Started {} second(s) before it \
             was read; ask again",
            claim.host,
            started_ago(header.started)
        ));
    };
    // A SUITE THAT FAILED THERE IS A FINDING OF ITS OWN, and it is read before
    // the census's verdict because the census is a reading of THAT RUN: a suite
    // that died early reached less, and a census over less is clean for a reason
    // that has nothing to do with reaches.
    if answer.suite != Some(0) {
        return Verdict::Finding(format!(
            "the suite under the census failed on `{}` (exit {}) — this repository does not \
             pass on a machine that is not this one, and the census over that run read a \
             shorter run than a green one would have",
            claim.host,
            answer
                .suite
                .map_or_else(|| "unrecorded".to_owned(), |code| code.to_string())
        ));
    }
    match answer.census {
        Some(0) if sentinel == 0 => Verdict::Elsewhere {
            host: header.host,
            seconds: started_ago(header.started),
        },
        Some(1) => Verdict::Finding(format!(
            "the census on `{}` found reaches outside this repository that nothing declares — \
             its own report is in the log",
            claim.host
        )),
        Some(code) => Verdict::NotJudged(format!(
            "the census on `{}` exited {code} — it could not judge that run, and a census that \
             refused is not a census that passed",
            claim.host
        )),
        None => Verdict::NotJudged(format!(
            "the run on `{}` ended ({SENTINEL}{sentinel}) and its log records no census verdict \
             at all — the reader never wrote one, so nothing here is evidence",
            claim.host
        )),
    }
}

/// How long ago a stamp is, in seconds, never going backwards.
///
/// Clocks on two machines are two clocks. A remote stamp ahead of this machine's
/// is a fact about the pair rather than about the run, so it saturates at zero
/// instead of printing a negative age nobody can act on.
#[must_use]
pub fn started_ago(started: u64) -> u64 {
    now().saturating_sub(started)
}

/// Seconds since the epoch, or zero if this machine's clock is before it.
#[must_use]
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// This machine's own name for itself, read where the kernel keeps it.
///
/// FROM `/proc`, not from a program on `PATH`. This gate runs from a git hook,
/// and this repository's hook suite runs those hooks on a deliberately hermetic
/// `PATH` holding three programs; a check that vanishes when `hostname` is not
/// reachable is a check that stops existing without saying so. The far side
/// reads the same file through this same function, so the two names are one
/// spelling rather than two that agree on this fleet.
///
/// # Errors
///
/// When the file cannot be read — on which this program has no opinion to give,
/// because the whole question is which machine answered.
pub fn this_host() -> Result<String, String> {
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|text| text.trim().to_owned())
        .map_err(|error| format!("/proc/sys/kernel/hostname could not be read — {error}"))
}

fn git(repository: &Path, arguments: &[&str]) -> Result<Vec<u8>, String> {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(repository)
        .output()
        .map_err(|error| format!("could not run `git {}` — {error}", arguments.join(" ")))?;
    if !output.status.success() {
        return Err(format!(
            "`git {}` exited {} — {}",
            arguments.join(" "),
            output
                .status
                .code()
                .map_or_else(|| "on a signal".to_owned(), |code| code.to_string()),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(output.stdout)
}

/// Name the bytes a dispatch would send, in a way a commit does not change.
///
/// # What it is over
///
/// The placement program sends TRACKED FILES AS THEY STAND IN THE WORKING TREE
/// and nothing else — no untracked file ever leaves this machine. So the two
/// readings below determine exactly that content and nothing more: the index,
/// which git already holds a content hash for, and the patch from the index to
/// the working tree, which carries every modification, deletion, mode change and
/// binary difference the index does not.
///
/// # Why not `HEAD`
///
/// Because a commit must not change this answer. The dispatch happens at commit
/// time, when the content of the round is final; the reading happens at push
/// time, after `HEAD` has moved and not one tracked byte has. A fingerprint
/// carrying `HEAD` would call those two different trees and this gate would
/// never once judge the tree it was built for. The placement program's own
/// fingerprint does carry `HEAD`, correctly — it answers "is the far copy the
/// same checkout", which is a different question from "is this the same content".
///
/// # Errors
///
/// When git cannot be run or answers an error — which is a refusal, not a
/// fingerprint of an empty tree.
pub fn fingerprint(repository: &Path) -> Result<String, String> {
    let index = git(repository, &["ls-files", "--stage"])?;
    // `--binary`, so a changed binary file is a difference rather than the
    // sentence "Binary files differ" — which is the same sentence for every
    // possible content and would make two different trees fingerprint alike.
    let worktree = git(repository, &["diff-files", "--patch", "--binary"])?;
    let mut listing = Vec::with_capacity(index.len() + worktree.len() + 32);
    listing.extend_from_slice(b"index\n");
    listing.extend_from_slice(&index);
    listing.extend_from_slice(b"worktree\n");
    listing.extend_from_slice(&worktree);
    // DIGESTED BY GIT, which is already a hard requirement of everything above
    // and needs no second opinion about what a content hash is.
    let output = Command::new("git")
        .args(["hash-object", "-t", "blob", "--stdin"])
        .current_dir(repository)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .map_err(|error| format!("could not run `git hash-object` — {error}"))
        .and_then(|mut child| {
            use std::io::Write as _;
            child
                .stdin
                .take()
                .ok_or_else(|| "git hash-object took no stdin".to_owned())
                .and_then(|mut stdin| {
                    stdin
                        .write_all(&listing)
                        .map_err(|error| format!("could not write to git hash-object — {error}"))
                })?;
            child
                .wait_with_output()
                .map_err(|error| format!("git hash-object did not finish — {error}"))
        })?;
    if !output.status.success() {
        return Err("`git hash-object` refused the listing".to_owned());
    }
    let digest = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if digest.is_empty() {
        return Err("`git hash-object` printed nothing".to_owned());
    }
    Ok(digest)
}

/// The command this repository declares for verifying itself on a build machine.
///
/// # Errors
///
/// When the declaration is missing, is not the language it claims, or names no
/// such command. Every one of those is a refusal: a census over a command this
/// program invented would be a second definition of what verification is.
pub fn declared_verify_command(repository: &Path) -> Result<String, String> {
    let path = repository.join(DECLARATION);
    let text = std::fs::read_to_string(&path)
        .map_err(|error| format!("{} could not be read — {error}", path.display()))?;
    let parsed: toml::Table = text
        .parse()
        .map_err(|error| format!("{} is not valid TOML — {error}", path.display()))?;
    parsed
        .get("commands")
        .and_then(|commands| commands.get("verify"))
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            format!(
                "{} declares no `[commands] verify` — the census has no command to be a \
                 census OF, and inventing one here would be a second definition of what \
                 verifying this repository means",
                path.display()
            )
        })
}

/// The launcher, as the far side's shell will read it.
///
/// # Why the sentinel is the launcher's line and not the script's
///
/// Rule 6 of the machine-wide remote-build protocol, and it was written after a
/// script printed `EXPERIMENT COMPLETE` having failed all three of its
/// conditions. A script's claim about itself is a line in a log; the value here
/// is the kernel's, and it is still written when the script dies halfway.
///
/// # Why the redirection is on the BRACES and not on the census
///
/// MEASURED, and the first draft had it wrong in a way that looked like working.
/// It read `mkdir … && L=… && setsid … > "$L" 2>&1 < /dev/null &`, where the `&`
/// backgrounds the whole `&&` LIST: the subshell running that list keeps the
/// transport's own stdout as its fd 1, and then waits for the census. The
/// connection therefore stays open for the entire run — 565 seconds against the
/// 23 a dispatch costs — and a launcher that does not return is a git hook that
/// does not return.
///
/// So the group carries the redirection, and the group is what is backgrounded:
/// the subshell's descriptors are the log from its first instruction, and the
/// transport is free the moment it starts. The `mkdir` stands OUTSIDE it and
/// unbackgrounded, because a redirection cannot open a file in a directory that
/// does not exist yet.
///
/// # Why `setsid nohup`, `< /dev/null` and `$0`
///
/// The census outlives the connection that starts it, so it is detached (rule
/// 5). It is given no stdin, because the transport sends the remote half down
/// one — a wrapped command that inherits it eats the rest of the script and the
/// sentinel is never reached, which is a run with no verdict at all rather than
/// a wrong one. And the log path reaches the inner shell as `$0` rather than by
/// interpolation, so there is exactly one level of quoting to be wrong about.
#[must_use]
pub fn launcher(log: &str, script: &str) -> String {
    format!(
        "mkdir -p \"$(dirname \"{log}\")\"; \
         {{ setsid nohup bash -c '{script}; echo \"{SENTINEL}$?\" >> \"$0\"' \"{log}\"; }} \
         > \"{log}\" 2>&1 < /dev/null &"
    )
}

/// How long this gate waits for the program it runs, when no caller says.
///
/// A BOUND IS REQUIRED HERE RATHER THAN PRUDENT, because both callers are git
/// hooks. A dispatch is a tree transfer and a launch — 23 seconds warm on this
/// fleet, measured — and a fetch is one small file over a connection. Neither
/// program bounds itself: the transport under both of them will sit on a host
/// that has stopped answering for as long as the kernel's own timeouts allow,
/// and a commit that cannot be made while a build machine is rebooting is a gate
/// that stops the work it exists to check. This value is what turns that hang
/// into a sentence.
///
/// FOUR TIMES THE MEASUREMENT, deliberately loose: the number that matters is
/// not "how long should this take" — the answer to that is decided elsewhere and
/// is already reported — but "how long before waiting is worse than not knowing".
pub const BUDGET: std::time::Duration = std::time::Duration::from_secs(120);

/// How often the wait below asks whether the condition it ends on has happened.
const ASKING_STEP: std::time::Duration = std::time::Duration::from_millis(50);

/// Run a program, and stop waiting for it when the budget runs out.
///
/// The wait ends on a CONDITION — the child having exited — and the budget that
/// bounds it is named rather than spelled here, which is the shape this
/// repository's own law asks of a wait.
///
/// # Why the output goes to FILES and not to pipes
///
/// Because this program's entire business is starting a run that OUTLIVES the
/// command that starts it, and a pipe is held open by every process that
/// inherited it. `wait_with_output` reads a pipe to end-of-file, so a child that
/// exits while a grandchild it detached still holds the write end leaves this
/// blocked with no bound at all — the budget above would have been measured and
/// then thrown away one line later. Files have no such property: once the child
/// is gone its output is on disk, and whatever else is still writing is somebody
/// else's business.
///
/// # What ending it costs, said rather than hidden
///
/// A dispatch killed mid-transfer leaves a partial copy on the far side. That is
/// repaired by the next dispatch rather than left: the transport compares
/// CONTENT, not timestamps. What must never happen is the other thing, and does
/// not — no claim is written for a dispatch that did not return, so nothing
/// later reads an answer about a tree that never fully arrived.
///
/// # Errors
///
/// When the program cannot be started, or when the budget runs out — which is
/// reported as what it is, a wait this gate ended, not a program that failed.
pub fn run_bounded(
    command: &mut Command,
    budget: std::time::Duration,
) -> Result<std::process::Output, String> {
    // THE PROCESS IS IN THE PATH, and so is a counter: `temp_dir()` is the
    // MACHINE's directory, shared by every process on it, and two threads of one
    // test binary have the same pid. R1175 repaired six paths that had only the
    // first half of that and `tools/unowned-scratch` is the law that keeps them
    // repaired.
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let at = std::env::temp_dir().join(format!(
        "one-machine-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&at).map_err(|error| {
        format!(
            "has nowhere to write its output ({}) — {error}",
            at.display()
        )
    })?;
    let said = at.join("stdout");
    let complained = at.join("stderr");
    let open = |path: &Path| {
        std::fs::File::create(path)
            .map_err(|error| format!("could not open {} — {error}", path.display()))
    };
    let mut child = command
        .stdin(std::process::Stdio::null())
        .stdout(open(&said)?)
        .stderr(open(&complained)?)
        .spawn()
        .map_err(|error| format!("could not be run — {error}"))?;
    let deadline = std::time::Instant::now() + budget;
    let status = loop {
        match child.try_wait() {
            Err(error) => return Err(format!("could not be waited for — {error}")),
            Ok(Some(status)) => break status,
            Ok(None) => {}
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_dir_all(&at);
            return Err(format!(
                "did not finish within {} second(s) and was ended. That budget is \
                 this gate's, not the program's: a hook may not hang on a machine \
                 that has stopped answering",
                budget.as_secs()
            ));
        }
        std::thread::sleep(ASKING_STEP);
    };
    let output = std::process::Output {
        status,
        stdout: std::fs::read(&said).unwrap_or_default(),
        stderr: std::fs::read(&complained).unwrap_or_default(),
    };
    let _ = std::fs::remove_dir_all(&at);
    Ok(output)
}

/// The placement program's path when the caller names none.
///
/// # Errors
///
/// When this account has no home directory to resolve it under.
pub fn program_under_home() -> Result<PathBuf, String> {
    std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(PROGRAM_UNDER_HOME))
        .ok_or_else(|| {
            format!(
                "HOME is not set, so the placement program cannot be found at \
                 ~/{PROGRAM_UNDER_HOME}"
            )
        })
}
