//! Write down what this job's cache restore actually put on disk.
//!
//! Two invocations, either side of the `actions/cache` step, and the paths are
//! spelled ONCE:
//!
//! ```text
//! restored before ~/.cargo/registry ~/.cargo/git target
//! …the cache step…
//! restored after
//! ```
//!
//! `after` re-measures the paths `before` wrote down rather than being handed
//! them again, because the two measurements are only a difference if they are of
//! the same thing — a second spelling of the list is a second thing to keep in
//! step, and the difference would go quietly wrong rather than loudly.
//!
//! Both write to `$MNEMOSYNE_RESTORED`, which the workflow spells per job beside
//! the compilation log, so what a job compiled and what it started from are
//! uploaded together and joined by `tools/twice-compiled`.

use std::io::Write;
use std::path::PathBuf;

use restored::{Measurement, Side};

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let record = PathBuf::from(required(restored::VARIABLE));
    match arguments.first().map(String::as_str) {
        Some("before") => before(&record, &arguments[1..]),
        Some("after") => after(&record),
        _ => fail(
            "usage: restored before <cached path>… | restored after — the first \
             runs immediately before the `actions/cache` step and the second \
             immediately after it",
        ),
    }
}

/// Open the record and measure every declared path as it is now.
fn before(record: &PathBuf, paths: &[String]) {
    if paths.is_empty() {
        fail(
            "`restored before` was given no path — the paths are the `path:` \
             list of this job's cache, and a record measuring nothing would read \
             as a job that restored nothing",
        );
    }
    let job = required("GITHUB_JOB");
    let home = required("HOME");
    // TRUNCATED AND NOT APPENDED, so that a runner re-using a workspace cannot
    // leave a previous job's measurements in front of this one's.
    let mut out = Vec::new();
    out.extend_from_slice(&restored::encode_job(&job));
    for path in paths {
        let measured = measure(path, &home);
        out.extend_from_slice(&restored::encode_side(Side::Before, path, &measured));
        println!(
            "[restored] before  {path}: {} entries, {} bytes",
            measured.entries, measured.bytes
        );
    }
    write(record, &out, false);
}

/// Measure the same paths again, and close the record with the action's own
/// answer about the primary key.
fn after(record: &PathBuf) {
    let home = required("HOME");
    let text = std::fs::read_to_string(record).unwrap_or_else(|error| {
        fail(&format!(
            "cannot read {} ({error}) — `restored after` re-measures what \
             `restored before` wrote down, so a missing record means the step \
             before the cache did not run",
            record.display()
        ))
    });
    let paths = opened(&text);
    // THE ACTION'S OWN OUTPUT, and an unreadable one is a refusal rather than a
    // `false`: `cache-hit` is the only thing that can tell an exact hit from a
    // prefix hit, and reading its absence as "no exact hit" would report every
    // warm job as the state this whole record exists to distinguish.
    let exact = match required(restored::EXACT_VARIABLE).as_str() {
        "true" => true,
        "false" => false,
        other => fail(&format!(
            "${} is {other:?}, which is neither `true` nor `false` — \
             `actions/cache` sets it from the primary key, so this step is \
             reading an output that is not there",
            restored::EXACT_VARIABLE
        )),
    };
    let mut out = Vec::new();
    for path in &paths {
        let measured = measure(path, &home);
        out.extend_from_slice(&restored::encode_side(Side::After, path, &measured));
        println!(
            "[restored] after   {path}: {} entries, {} bytes",
            measured.entries, measured.bytes
        );
    }
    out.extend_from_slice(&restored::encode_exact(exact));
    write(record, &out, true);

    // READ BACK, so that a record this job cannot decode fails HERE, in the job
    // that wrote it, rather than in the gate that joins nine of them an hour
    // later. Its verdict is printed for the same reason `tools/cache-budget`
    // prints one: the state a census was taken under has to be in the job's own
    // log, where a person reading a single run can see it.
    let text = std::fs::read_to_string(record).expect("the record just written");
    match restored::decode(&text) {
        Ok(whole) => println!("[restored] this job started from: {}", whole.warmth().why()),
        Err(why) => fail(&format!(
            "the record this job just wrote does not decode: {why}"
        )),
    }
}

/// The paths `restored before` wrote down, in the order it wrote them.
///
/// A HALF-RECORD IS READ HERE TOO. `restored::decode` refuses one, and it is
/// right to: a record with no `exact` line is a job that died before the
/// restore was read. But this step is the one that finishes it, so it reads the
/// `before` lines directly and refuses only on what would make its own
/// measurement wrong.
fn opened(text: &str) -> Vec<String> {
    let mut paths = Vec::new();
    for line in text.lines() {
        let fields: Vec<&str> = line.split(restored::FIELD as char).collect();
        match fields.as_slice() {
            ["job", _] => {}
            [side, path, _, _] if *side == Side::Before.word() => paths.push((*path).to_string()),
            _ => fail(&format!(
                "the record `restored before` wrote holds a line this step \
                 cannot read: {line:?}"
            )),
        }
    }
    if paths.is_empty() {
        fail(
            "the record `restored before` wrote names no path — the two \
             measurements are a difference and there is nothing to difference",
        );
    }
    paths
}

fn measure(path: &str, home: &str) -> Measurement {
    let expanded = restored::expand(path, home);
    restored::measure(&expanded).unwrap_or_else(|error| {
        fail(&format!(
            "cannot measure {path} ({}): {error} — a walk that stopped early \
             reports a warm tree as a cold one",
            expanded.display()
        ))
    })
}

fn write(record: &PathBuf, bytes: &[u8], append: bool) {
    if let Some(parent) = record.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).unwrap_or_else(|error| {
                fail(&format!("cannot make {}: {error}", parent.display()))
            });
        }
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(append)
        .write(true)
        .truncate(!append)
        .open(record)
        .unwrap_or_else(|error| fail(&format!("cannot open {}: {error}", record.display())));
    file.write_all(bytes)
        .unwrap_or_else(|error| fail(&format!("cannot write {}: {error}", record.display())));
}

fn required(variable: &str) -> String {
    match std::env::var(variable) {
        Ok(value) if !value.is_empty() => value,
        _ => fail(&format!(
            "${variable} is unset or empty, and this program will not guess it"
        )),
    }
}

/// Say what is wrong and stop.
///
/// EXIT 1 RATHER THAN A PANIC, because every one of these is a wiring mistake in
/// a workflow and the person reading it is looking at a job log, not a
/// backtrace.
fn fail(why: &str) -> ! {
    eprintln!("restored: {why}");
    std::process::exit(1)
}
