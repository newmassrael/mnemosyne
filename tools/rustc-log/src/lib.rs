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
//! WHAT IT CANNOT SEE, named rather than left to be discovered:
//!
//! - **A unit that was already fresh.** cargo does not run `rustc` for it, so it
//!   leaves no record. That makes this an instrument for a COLD build, and it is
//!   why the CI wiring gives each job its own log rather than reading one shared
//!   one after the fact.
//! - **`rustdoc`.** Doc tests and `cargo doc` do not go through `RUSTC_WRAPPER`.
//! - **A build script's own `cc`.** `librocksdb-sys` compiles C, and that is
//!   real work no `rustc` record accounts for.
//!
//! All three under-count, which is the safe direction for a gate whose finding
//! is "this work is done more than once".

use std::io::Write;
use std::path::Path;

/// The environment variable naming the file this wrapper appends to.
pub const LOG_VARIABLE: &str = "MNEMOSYNE_RUSTC_LOG";

/// Separates the arguments within one record.
///
/// ASCII unit separator, chosen because it is the one byte class that cannot
/// appear in a path or a `--cfg` value that any real toolchain produces — and
/// [`encode`] refuses rather than escaping if one ever does, because a silently
/// mangled record is a compilation this gate would then miscount.
pub const FIELD: u8 = 0x1f;

/// Separates records.
pub const RECORD: u8 = b'\n';

/// One `rustc` invocation as this wrapper saw it: the compiler's own path first,
/// then its arguments.
///
/// The compiler path is part of the record because it is what tells one
/// toolchain from another. The MSRV job runs a different `rustc` over the same
/// sources, and a reader that dropped the path would have to be told about
/// toolchains by a human instead of reading them off the machine.
pub fn encode(argv: &[String]) -> Vec<u8> {
    assert!(
        !argv.is_empty(),
        "an invocation with no words at all is not one this wrapper was given"
    );
    for word in argv {
        assert!(
            !word.bytes().any(|byte| byte == FIELD || byte == RECORD),
            "the argument {word:?} holds a byte this record format uses as a \
             separator — refusing rather than writing a record that would decode \
             into different arguments than were passed"
        );
    }
    let mut out = Vec::new();
    for (index, word) in argv.iter().enumerate() {
        if index > 0 {
            out.push(FIELD);
        }
        out.extend_from_slice(word.as_bytes());
    }
    out.push(RECORD);
    out
}

/// Read back one record.
pub fn decode(record: &str) -> Vec<String> {
    record.split(FIELD as char).map(str::to_string).collect()
}

/// Every record in a log file, in the order they were appended.
///
/// A trailing partial line would mean a record was torn, which [`append`] is
/// built to make impossible; it is an assertion rather than a silent `filter`
/// because "some records went missing" is exactly the shape of a clean-looking
/// wrong answer.
pub fn decode_all(text: &str) -> Vec<Vec<String>> {
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
pub fn append(log: &Path, argv: &[String]) -> std::io::Result<()> {
    let record = encode(argv);
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
    file.write_all(&record)
}
