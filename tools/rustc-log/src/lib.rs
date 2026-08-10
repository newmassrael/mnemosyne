//! The record `RUSTC_WRAPPER` leaves behind, and the only place its shape is
//! written down.
//!
//! `tools/twice-compiled` asks whether two CI jobs compile the same thing. The
//! population for that question is every `rustc` a job runs, and there is no way
//! to ask cargo for it: a job's cost is not one cargo invocation but a tree of
//! them, because three of this repository's gates are programs that shell out to
//! cargo themselves. `--message-format=json` reaches the outermost call and
//! nothing under it.
//!
//! `RUSTC_WRAPPER` reaches all of them, because it is an environment variable
//! and a child process inherits it. Every compilation in a job — the workspace
//! suite, a gate's own build, and the builds that gate spawns — arrives here.
//!
//! # A record carries a clock
//!
//! A COMPILATION IS NOT A COMPILATION. Counting them answers how much work is
//! done twice and cannot answer what that costs, and the two questions have
//! different answers here: the head of this repository's duplication is
//! `build_script_build`, a crate whose units are among the cheapest it compiles.
//! A ranking by count puts the repair where the rows are, and the money is
//! somewhere else.
//!
//! So every record says WHEN the compiler started and HOW LONG it ran. Both,
//! not one: the durations of a job sum to the work it did, and cargo runs as
//! many compilers as the machine has cores, so that sum is not a wall-clock. The
//! window from the first compiler's start to the last one's exit is, and having
//! the two is what lets a reader say how much of a job's minutes a repair can
//! actually reach.
//!
//! # What it cannot see, named rather than left to be discovered
//!
//! - **A unit that was already fresh.** cargo does not run `rustc` for it, so it
//!   leaves no record. That makes this an instrument for a COLD build, and it is
//!   why the CI wiring gives each job its own log rather than reading one shared
//!   one after the fact.
//! - **`rustdoc`.** Doc tests and `cargo doc` do not go through `RUSTC_WRAPPER`.
//! - **A build script's own `cc`.** `librocksdb-sys` compiles C, and that is
//!   real work no `rustc` record accounts for.
//! - **A compilation that never finished.** The record is written when the
//!   compiler exits, because that is when its duration is known, so a job killed
//!   at its timeout loses whatever was in flight.
//!
//! All four under-count, which is the safe direction for a gate whose finding is
//! "this work is done more than once".

use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// The environment variable naming the file this wrapper appends to.
pub const LOG_VARIABLE: &str = "MNEMOSYNE_RUSTC_LOG";

/// Separates the fields within one record.
///
/// ASCII unit separator, chosen because it is the one byte class that cannot
/// appear in a path or a `--cfg` value that any real toolchain produces — and
/// `rustc_log::encode` refuses rather than escaping if one ever does, because a
/// silently mangled record is a compilation this gate would then miscount.
pub const FIELD: u8 = 0x1f;

/// Separates records.
pub const RECORD: u8 = b'\n';

/// The commit this binary was COMPILED from, baked in at compile time.
///
/// R1119 — WHICH BUILD OF THE INSTRUMENT MEASURED A CENSUS. R1118 found that
/// `unrun-tests` restored its own `target` over the binaries it had just built,
/// so every census that job ever produced was measured by whatever recorder the
/// previous generation's cache happened to hold — and nothing could say so,
/// because the record carried no way to tell one build of the recorder from
/// another. The seconds moved by a factor of four when the fresh one finally ran.
///
/// `option_env!` AND NOT AN ARGUMENT, which is the whole point: a value passed in
/// at run time is the environment's answer, and a substituted binary would read
/// the CURRENT environment and look right. This is fixed when the crate is
/// compiled, so a binary out of a cache answers with the commit it was built
/// from, which is not this run's.
///
/// `None` off a runner — a local build has no commit to name and says so rather
/// than inventing one; "could not tell" is a different answer from "they agree".
pub const BUILT_FROM: Option<&str> = option_env!("GITHUB_SHA");

/// What [`BUILT_FROM`] says, in one word a record can hold.
pub fn built_from() -> &'static str {
    match BUILT_FROM {
        Some(commit) if !commit.is_empty() => commit,
        _ => "local",
    }
}

/// The argument that asks this program what it was built from rather than
/// running a compiler.
///
/// SAFE BECAUSE CARGO NEVER PASSES IT. A `RUSTC_WRAPPER` is invoked as
/// `<wrapper> <rustc> <arguments…>`, and the first word is always a path to a
/// compiler — never a flag.
pub const STAMP_ARGUMENT: &str = "--built-from";

/// One `rustc` invocation as this wrapper saw it.
///
/// The compiler path is part of `rustc_log::Record::argv` because it is what
/// tells one toolchain from another. The MSRV job runs a different `rustc` over
/// the same sources, and a reader that dropped the path would have to be told
/// about toolchains by a human instead of reading them off the machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    /// When the compiler was started, in microseconds since the Unix epoch.
    ///
    /// A WALL CLOCK and not the monotonic one, because its whole purpose is to
    /// be compared against a start recorded by a DIFFERENT process — the dozens
    /// of wrappers a build runs at once — and a monotonic instant has no meaning
    /// outside the process that read it.
    pub started_at: u64,
    /// How long the compiler ran, in microseconds, measured monotonically.
    pub micros: u64,
    /// The compiler's own path first, then the arguments it was given.
    pub argv: Vec<String>,
}

impl Record {
    /// When the compiler exited.
    pub fn ended_at(&self) -> u64 {
        self.started_at.saturating_add(self.micros)
    }
}

/// Now, in microseconds since the Unix epoch.
///
/// A clock set before 1970 reads as 0 rather than refusing: this is a wrapper in
/// front of every compilation, and a build that stops because the machine's
/// clock is odd is a worse outcome than a span this reader cannot use.
pub fn now_micros() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| u64::try_from(since.as_micros()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

/// Write one record: the start, the duration, then the compiler and its
/// arguments.
pub fn encode(record: &Record) -> Vec<u8> {
    assert!(
        !record.argv.is_empty(),
        "an invocation with no words at all is not one this wrapper was given"
    );
    for word in &record.argv {
        assert!(
            !word.bytes().any(|byte| byte == FIELD || byte == RECORD),
            "the argument {word:?} holds a byte this record format uses as a \
             separator — refusing rather than writing a record that would decode \
             into different arguments than were passed"
        );
    }
    let mut out = Vec::new();
    out.extend_from_slice(record.started_at.to_string().as_bytes());
    out.push(FIELD);
    out.extend_from_slice(record.micros.to_string().as_bytes());
    for word in &record.argv {
        out.push(FIELD);
        out.extend_from_slice(word.as_bytes());
    }
    out.push(RECORD);
    out
}

/// Read back one record.
///
/// A FIELD THAT IS NOT A NUMBER IS A PANIC, not a zero and not a skip. The two
/// ways it can happen are a log written by an older wrapper and a record torn by
/// something this format's `O_APPEND` write was supposed to make impossible, and
/// both would otherwise arrive downstream as compilations that took no time at
/// all — free work, which is what a gate reading this is looking for.
pub fn decode(record: &str) -> Record {
    let mut fields = record.split(FIELD as char);
    let started_at = number(fields.next(), "the start", record);
    let micros = number(fields.next(), "the duration", record);
    let argv: Vec<String> = fields.map(str::to_string).collect();
    assert!(
        !argv.is_empty(),
        "a record with a clock and no compiler at all: {record:?}"
    );
    Record {
        started_at,
        micros,
        argv,
    }
}

fn number(field: Option<&str>, which: &str, record: &str) -> u64 {
    let field = field.unwrap_or_else(|| {
        panic!("a record with no field for {which}: {record:?}");
    });
    field.parse().unwrap_or_else(|error| {
        panic!(
            "{which} of a record is {field:?}, which is not microseconds \
             ({error}) — refusing rather than reading a compilation as one that \
             cost nothing: {record:?}"
        );
    })
}

/// Every record in a log file, in the order they were appended.
///
/// A trailing partial line would mean a record was torn, which
/// `rustc_log::append` is built to make impossible; the numeric fields are
/// asserted rather than filtered because "some records went missing" is exactly
/// the shape of a clean-looking wrong answer.
pub fn decode_all(text: &str) -> Vec<Record> {
    let mut out = Vec::new();
    for line in text.split(RECORD as char) {
        if line.is_empty() {
            continue;
        }
        out.push(decode(line));
    }
    out
}

/// Append one record to the log.
///
/// ONE `write` OF THE WHOLE RECORD on a handle opened `O_APPEND`, which is what
/// makes this safe under the dozens of `rustc` processes cargo runs at once: the
/// kernel serialises an append to a regular file against other appends, so two
/// records cannot interleave. `tests/format.rs` runs it from many threads at
/// once and checks every record came back whole, because that guarantee is the
/// one thing here that a comment cannot establish.
pub fn append(log: &Path, record: &Record) -> std::io::Result<()> {
    let bytes = encode(record);
    // THE DIRECTORY IS THIS WRAPPER'S TO MAKE. The alternative is a step in every
    // job that creates it first, which is eight places to forget — and the first
    // `rustc` of a build is cargo's version probe, which runs before cargo has
    // made a single directory of its own.
    if let Some(parent) = log.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log)?;
    file.write_all(&bytes)
}
