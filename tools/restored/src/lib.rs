//! What a job STARTED FROM, measured on both sides of its cache restore.
//!
//! A census of what CI compiles is only a measurement if it says what it was
//! taken under, and the loudest thing it is taken under is whether the job's
//! build tree was already there. Round 1099 read two censuses of the same job,
//! called one of them cold, found the counts identical, and deleted a 7.5 GB
//! cache that was saving 426 compilations and ten minutes. Round 1100 put the
//! cache back. The run it had called cold had restored a whole previous
//! generation through `restore-keys`.
//!
//! NOTHING IN THE REPOSITORY COULD HAVE SAID SO, and the reason is worth
//! writing down because it is not laziness:
//!
//! - `actions/cache` declares exactly ONE output, `cache-hit`, and its own
//!   documentation says that a `restore-keys` prefix match returns `false` for
//!   it. The action that did the restoring cannot tell you it restored
//!   anything.
//! - `tools/cache-budget` asks the API which caches are held and when they were
//!   created, and reads a cache created inside this run as one the run built
//!   from nothing — which is true of the KEY and says nothing about the JOB.
//!   `actions/cache` saves whenever the primary key missed, so a job warmed by
//!   a prefix hit saves a new entry and is reported in exactly the words used
//!   for a job that compiled from an empty disk.
//!
//! So the third state — warm from a PREVIOUS generation — is invisible to both
//! instruments, and it is the state that produced the wrong deletion. It is
//! also the ordinary state of this repository's CI: every push that touches
//! `Cargo.lock` or the workflow moves all eight keys at once.
//!
//! THE ANSWER IS ON THE DISK, so this reads the disk. A job measures its cache's
//! declared paths immediately before the restore and again immediately after,
//! and what arrived is the difference. That is an observation and not an
//! inference: no threshold to tune, no API race, and the control for whatever
//! the steps before it happened to leave in `~/.cargo` is the same tree,
//! measured seconds earlier.
//!
//! WHAT IT DOES NOT CLAIM. The bytes that arrived are not the bytes that were
//! USEFUL: cargo decides that per unit, from fingerprints this program does not
//! read, and a prefix hit restores a tree that is warm and stale at once. The
//! question answered here is the one that was got wrong — did this job begin
//! with a tree, or with nothing — and `tools/twice-compiled` is where it is
//! joined to what the job then compiled.

use std::path::{Path, PathBuf};

/// Separates the fields within one line, as in `rustc_log`: the one byte class
/// that cannot appear in a path any real toolchain produces.
pub const FIELD: u8 = 0x1f;

/// The environment variable naming the file this record is written to.
///
/// SPELLED PER JOB IN THE WORKFLOW, exactly as `rustc_log::LOG_VARIABLE` is, and
/// checked against the job's own name by `tools/twice-compiled` — two jobs
/// writing one path is the paste error that turns a census of two jobs into one
/// blob with no duplication in it.
pub const VARIABLE: &str = "MNEMOSYNE_RESTORED";

/// The environment variable carrying `actions/cache`'s own `cache-hit` output.
///
/// TAKEN FROM THE ACTION rather than decided here, because whether the PRIMARY
/// key matched is a fact only the action holds — the disk cannot tell an exact
/// hit from a prefix hit, and this program cannot tell either without being
/// told. The two answers together are what make three states distinguishable.
pub const EXACT_VARIABLE: &str = "MNEMOSYNE_CACHE_HIT";

/// What was under one path at one moment.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Measurement {
    /// Files, symlinks and every other non-directory entry beneath it.
    pub entries: u64,
    /// What those entries weigh.
    pub bytes: u64,
}

/// One cached path, before the restore and after it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Restoration {
    /// The path AS THE WORKFLOW DECLARES IT, `~` and all.
    ///
    /// The spelling is kept rather than the expansion because it is what makes
    /// this record checkable against the `path:` list of the job's cache: two
    /// lists of the same strings, both read off the machine, compared by
    /// `tools/twice-compiled`. Recording `/home/runner/.cargo/registry` would
    /// leave that comparison to a normaliser nobody can test.
    pub path: String,
    /// What was there immediately before the restore step ran.
    pub before: Measurement,
    /// What was there immediately after it.
    pub after: Measurement,
}

impl Restoration {
    /// What the restore put under this path.
    ///
    /// Saturating, because a path that SHRANK across the restore is not a
    /// negative restoration — it is something this program does not model, and
    /// reading it as zero keeps the total honest in the direction that reports
    /// less warmth rather than more.
    pub fn bytes_restored(&self) -> u64 {
        self.after.bytes.saturating_sub(self.before.bytes)
    }
}

/// What one job started from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Restored {
    /// The job id, which is also what the file is named for.
    pub job: String,
    /// `actions/cache`'s `cache-hit`: an EXACT match on the primary key.
    pub exact: bool,
    /// Every path the job's cache declares, in the order it declares them.
    pub paths: Vec<Restoration>,
}

/// The state a job's build tree was in when it started compiling.
///
/// FOUR VARIANTS AND NOT THREE. The fourth is a contradiction between the two
/// instruments — the action says the primary key matched exactly and the disk
/// says nothing arrived — and it is a variant rather than a silent fallback so
/// that every consumer has to say what it does about it. A census is compared
/// against another census by this value; a state nobody handled would be
/// compared as though it were one of the ordinary three.
/// ORDERED so that a refusal naming two of them can be sorted and deduplicated
/// alongside its neighbours. The order carries no meaning of its own — the four
/// states are not a scale, which is why [`Warmth::same_state`] is a predicate
/// rather than a comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Warmth {
    /// The primary key matched, and a tree arrived.
    ExactHit { bytes: u64 },
    /// The primary key MISSED and a previous generation arrived through
    /// `restore-keys` — warm, and stale in whatever moved the key.
    ///
    /// THE STATE THAT WAS INVISIBLE. `actions/cache` reports `cache-hit: false`
    /// here, and it saves a new entry afterwards, so both of the instruments
    /// this repository already had describe it in the words they use for a cold
    /// build.
    PrefixHit { bytes: u64 },
    /// Nothing arrived: this job compiled from an empty tree.
    Nothing,
    /// The action reported an exact hit and the disk did not change.
    HitThatBroughtNothing,
}

impl Warmth {
    /// Did two jobs start in the SAME state — the question one census being
    /// held against another turns on.
    ///
    /// THE VARIANT AND NOT THE SIZE. No two runs restore the same number of
    /// bytes, so comparing the whole value would call every real pair
    /// incomparable and the check would be one nobody could ever satisfy. What
    /// confounded Round 1099 was a variant: a job warmed through `restore-keys`
    /// read as one that had compiled from nothing. The discriminant is taken
    /// from the type rather than spelled out here, so a fifth state cannot be
    /// added and quietly left out of this.
    ///
    /// WHAT IT THEREFORE DOES NOT MODEL is magnitude: two prefix hits of 246 MB
    /// and 27 GB are one state here. That is why a report of a comparison prints
    /// both figures rather than only the verdict — the reader is owed the
    /// numbers this predicate does not look at.
    pub fn same_state(&self, other: &Warmth) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }

    /// One line, for a report that must not be quotable without it.
    pub fn why(&self) -> String {
        match self {
            Warmth::ExactHit { bytes } => format!("exact hit, {} restored", megabytes(*bytes)),
            Warmth::PrefixHit { bytes } => format!(
                "no exact hit, {} restored from an earlier generation",
                megabytes(*bytes)
            ),
            Warmth::Nothing => "NOTHING — it compiled from an empty tree".to_string(),
            Warmth::HitThatBroughtNothing => {
                "the cache reported an exact hit and no bytes arrived".to_string()
            }
        }
    }
}

/// Bytes as a report argues them.
fn megabytes(bytes: u64) -> String {
    format!("{:.0} MB", bytes as f64 / 1e6)
}

impl Restored {
    /// What the restore put on disk, over every path.
    pub fn bytes_restored(&self) -> u64 {
        self.paths.iter().map(Restoration::bytes_restored).sum()
    }

    /// The state this job started in.
    pub fn warmth(&self) -> Warmth {
        let bytes = self.bytes_restored();
        match (self.exact, bytes) {
            (true, 0) => Warmth::HitThatBroughtNothing,
            (true, bytes) => Warmth::ExactHit { bytes },
            (false, 0) => Warmth::Nothing,
            (false, bytes) => Warmth::PrefixHit { bytes },
        }
    }

    /// The paths this record measured, in the order the workflow declares them.
    pub fn measured(&self) -> Vec<&str> {
        self.paths.iter().map(|one| one.path.as_str()).collect()
    }
}

/// A record this reader will not turn into a measurement.
///
/// A REFUSAL AND NOT A DEFAULT, for the reason every reader in this repository
/// has one: a record half-written by a job that died between the two steps
/// decodes into a job that restored nothing, which is the finding a reader of
/// the census is hunting for. There is no value that means "I do not know", so
/// the absence has to travel as an error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Malformed {
    /// A line whose first field is not one this format defines.
    UnknownLine { line: String },
    /// A field that should be a number is not one.
    NotANumber { line: String },
    /// A line with the wrong number of fields.
    WrongShape { line: String },
    /// No `job` line, or more than one.
    JobIsNotSaidOnce { times: usize },
    /// No `exact` line, or more than one — the file the `after` step finishes
    /// with, so its absence is a job that died before the restore was read.
    ExactIsNotSaidOnce { times: usize },
    /// A path measured before the restore and not after it, or the other way
    /// round.
    PathIsNotOnBothSides { path: String },
    /// A record measuring nothing at all.
    NoPathsAtAll,
    /// `cache-hit` was neither `true` nor `false`.
    ExactIsNotABoolean { value: String },
}

impl std::fmt::Display for Malformed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Malformed::UnknownLine { line } => {
                write!(f, "a line this format does not define: {line:?}")
            }
            Malformed::NotANumber { line } => {
                write!(f, "a measurement that is not a number: {line:?}")
            }
            Malformed::WrongShape { line } => {
                write!(f, "a line with the wrong number of fields: {line:?}")
            }
            Malformed::JobIsNotSaidOnce { times } => write!(
                f,
                "the job is named {times} time(s) and a record is one job's"
            ),
            Malformed::ExactIsNotSaidOnce { times } => write!(
                f,
                "`cache-hit` is said {times} time(s) — the step after the restore \
                 writes it exactly once, so this record was never finished"
            ),
            Malformed::PathIsNotOnBothSides { path } => write!(
                f,
                "`{path}` was measured on one side of the restore only, and what \
                 arrived is the difference between the two"
            ),
            Malformed::NoPathsAtAll => write!(
                f,
                "a record measuring no path at all — a job whose cache declares \
                 nothing does not write one of these"
            ),
            Malformed::ExactIsNotABoolean { value } => write!(
                f,
                "`cache-hit` came through as {value:?}, which is neither `true` \
                 nor `false` — the step that reads it ran without the output it \
                 reads"
            ),
        }
    }
}

/// The `job` line the `before` step opens the record with.
pub fn encode_job(job: &str) -> Vec<u8> {
    line(&["job", job])
}

/// One `before` or `after` line.
pub fn encode_side(side: Side, path: &str, measured: &Measurement) -> Vec<u8> {
    line(&[
        side.word(),
        path,
        &measured.entries.to_string(),
        &measured.bytes.to_string(),
    ])
}

/// The `exact` line the `after` step closes the record with.
pub fn encode_exact(exact: bool) -> Vec<u8> {
    line(&["exact", if exact { "true" } else { "false" }])
}

/// Which side of the restore a measurement is from.
///
/// ORDERED, and the order is the real one: `Before` sorts before `After`, so a
/// reader holding both can ask which came first without a second convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Side {
    Before,
    After,
}

impl Side {
    /// The word this side is written as, and the one it is read back from —
    /// ONE spelling, so the writer and the reader cannot drift.
    pub fn word(&self) -> &'static str {
        match self {
            Side::Before => "before",
            Side::After => "after",
        }
    }

    /// Both sides, in the order a job takes them.
    pub const BOTH: [Side; 2] = [Side::Before, Side::After];
}

/// This program's own name, which is what every spelling of it ends in.
///
/// KEPT HERE BECAUSE THIS CRATE OWNS THE SPELLING, the same reason
/// [`Side::word`] is: a gate asking which step of a job measures the restore has
/// to recognise the command, and a copy of the path in that gate is a second
/// thing to keep in step with the workflow.
///
/// THE NAME AND NOT THE PATH, because there is more than one path and both are
/// real. The workflow runs `./target/release/restored`; the local replay builds
/// this crate without `--release` and runs the debug binary. A
/// constant holding one of those recognises one entrance and is blind to the
/// other — which is a gate that reports a job as measuring nothing when it
/// measures perfectly well.
pub const PROGRAM: &str = "restored";

/// Which sides of the restore one `run:` step measures, in the order it does.
///
/// EMPTY FOR EVERY OTHER STEP. The word has to BE this program or END IN it
/// after a `/`, which is what keeps the step that BUILDS it out of the answer:
/// that one names `tools/restored/Cargo.toml`, and a reader matching the
/// substring would call it a measurement — leaving a job with one it never takes
/// and no way to be refused for taking none.
///
/// Empty as well for an invocation whose next word this program does not define:
/// that is a step which exits 1 the moment it runs, and reading it as a
/// measurement would let a job whose wiring is broken pass a check about where
/// its measurements sit.
pub fn sides_measured(script: &str) -> Vec<Side> {
    let words: Vec<&str> = script.split_whitespace().collect();
    let mut sides = Vec::new();
    for pair in words.windows(2) {
        let [word, next] = pair else { continue };
        let invoked = *word == PROGRAM
            || word
                .rsplit_once('/')
                .is_some_and(|(_, name)| name == PROGRAM);
        if !invoked {
            continue;
        }
        if let Some(side) = Side::BOTH.into_iter().find(|side| side.word() == *next) {
            sides.push(side);
        }
    }
    sides
}

fn line(fields: &[&str]) -> Vec<u8> {
    for field in fields {
        assert!(
            !field.bytes().any(|byte| byte == FIELD || byte == b'\n'),
            "the field {field:?} holds a byte this format uses as a separator — \
             refusing rather than writing a record that reads back as different \
             fields than were written"
        );
    }
    let mut out = fields.join(&(FIELD as char).to_string()).into_bytes();
    out.push(b'\n');
    out
}

/// Read one job's record.
pub fn decode(text: &str) -> Result<Restored, Malformed> {
    let mut job: Vec<String> = Vec::new();
    let mut exact: Vec<String> = Vec::new();
    // ORDER PRESERVED, because the paths are compared against the `path:` list
    // of the job's cache declaration and that list is ordered. A set would let
    // a record measure the right paths in an order nobody wrote.
    let mut before: Vec<(String, Measurement)> = Vec::new();
    let mut after: Vec<(String, Measurement)> = Vec::new();
    for line in text.lines() {
        if line.is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split(FIELD as char).collect();
        match fields.first().copied() {
            Some("job") => match fields.as_slice() {
                [_, name] => job.push((*name).to_string()),
                _ => return Err(Malformed::WrongShape { line: line.into() }),
            },
            Some("exact") => match fields.as_slice() {
                [_, value] => exact.push((*value).to_string()),
                _ => return Err(Malformed::WrongShape { line: line.into() }),
            },
            Some(word @ ("before" | "after")) => match fields.as_slice() {
                [_, path, entries, bytes] => {
                    let measured = Measurement {
                        entries: number(entries, line)?,
                        bytes: number(bytes, line)?,
                    };
                    let side = if word == "before" {
                        &mut before
                    } else {
                        &mut after
                    };
                    side.push(((*path).to_string(), measured));
                }
                _ => return Err(Malformed::WrongShape { line: line.into() }),
            },
            _ => return Err(Malformed::UnknownLine { line: line.into() }),
        }
    }

    if job.len() != 1 {
        return Err(Malformed::JobIsNotSaidOnce { times: job.len() });
    }
    if exact.len() != 1 {
        return Err(Malformed::ExactIsNotSaidOnce { times: exact.len() });
    }
    let exact = match exact[0].as_str() {
        "true" => true,
        "false" => false,
        other => {
            return Err(Malformed::ExactIsNotABoolean {
                value: other.to_string(),
            })
        }
    };
    if before.is_empty() {
        return Err(Malformed::NoPathsAtAll);
    }
    let mut paths = Vec::new();
    for (path, measured) in &before {
        let matching = after
            .iter()
            .filter(|(other, _)| other == path)
            .collect::<Vec<_>>();
        let [(_, after)] = matching.as_slice() else {
            return Err(Malformed::PathIsNotOnBothSides { path: path.clone() });
        };
        paths.push(Restoration {
            path: path.clone(),
            before: *measured,
            after: *after,
        });
    }
    for (path, _) in &after {
        if !before.iter().any(|(other, _)| other == path) {
            return Err(Malformed::PathIsNotOnBothSides { path: path.clone() });
        }
    }
    Ok(Restored {
        job: job.remove(0),
        exact,
        paths,
    })
}

fn number(field: &str, line: &str) -> Result<u64, Malformed> {
    field.parse().map_err(|_| Malformed::NotANumber {
        line: line.to_string(),
    })
}

/// Does this record's file name belong to this job, and to no other?
///
/// The same law `tools/twice-compiled` puts on the compilation log, for the same
/// reason: the job's name is read off the file, so two jobs writing one path
/// become one record wearing the shape of two.
pub fn names_its_job(path: &str, job: &str) -> bool {
    path.rsplit('/').next() == Some(&format!("{job}.restored"))
}

/// The path a workflow spells, with `~` expanded from the environment's `HOME`.
///
/// `~` IS THE SHELL'S AND THIS IS NOT A SHELL. `actions/cache` expands it
/// itself, so a workflow declares `~/.cargo/registry` and this program is handed
/// that spelling verbatim — expanding it here is what lets the record keep the
/// declared spelling while measuring the real directory.
pub fn expand(path: &str, home: &str) -> PathBuf {
    match path.strip_prefix("~/") {
        Some(rest) => Path::new(home).join(rest),
        None => PathBuf::from(path),
    }
}

/// What is under one path right now.
///
/// AN ABSENT PATH IS ZERO AND NOT AN ERROR: `~/.cargo/git` does not exist until
/// something depends on a git checkout, and a job whose cache declares it is not
/// broken for having none. Anything else that goes wrong while walking — a
/// directory that cannot be read, a file whose metadata cannot be taken —
/// travels back as an error, because a walk that quietly stopped early reports a
/// warm tree as a cold one.
pub fn measure(path: &Path) -> std::io::Result<Measurement> {
    let mut out = Measurement::default();
    let mut pending = vec![path.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let entries = match std::fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(std::io::Error::new(
                    error.kind(),
                    format!("cannot read {}: {error}", directory.display()),
                ))
            }
        };
        for entry in entries {
            let entry = entry?;
            // SYMLINKS ARE NOT FOLLOWED, which is what keeps this walk finite
            // over a `target` directory and stops one counting the tree twice.
            let metadata = entry.path().symlink_metadata()?;
            if metadata.is_dir() {
                pending.push(entry.path());
            } else {
                out.entries += 1;
                out.bytes = out.bytes.saturating_add(metadata.len());
            }
        }
    }
    Ok(out)
}
