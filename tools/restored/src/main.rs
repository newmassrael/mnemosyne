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
        _ => fail(USAGE),
    }
}

const USAGE: &str = "usage: restored before --cache <key prefix> <cached path>… \
                     | restored after — the first runs immediately before ONE \
                     `actions/cache` step and the second immediately after that \
                     same step, and the prefix says which cache the interval \
                     between them is the price of";

/// Open the record and measure every declared path as it is now.
fn before(record: &PathBuf, arguments: &[String]) {
    // WHICH CACHE, TAKEN AS AN ARGUMENT AND NOT DERIVED. R1117: a job may
    // declare more than one cache step, and what a restore COST is the cost of
    // the step it brackets. A record that did not say which cache it is of would
    // be matched to whichever one the reader picked — the shape where a build
    // directory's price is read off a registry's restore, which is the
    // arithmetic R1099 got wrong. The value is the key's resolved prefix, which
    // is what every gate here already joins on, and `tools/twice-compiled`
    // checks it against the one `ci_plan` derives from the workflow.
    let [flag, cache, paths @ ..] = arguments else {
        fail(USAGE)
    };
    if flag != restored::CACHE_FLAG || cache.is_empty() {
        fail(USAGE);
    }
    if paths.is_empty() {
        fail(
            "`restored before` was given no path — the paths are the `path:` \
             list of the cache step this brackets, and a record measuring \
             nothing would read as a job that restored nothing",
        );
    }
    let job = required("GITHUB_JOB");
    let home = required("HOME");
    // TRUNCATED AND NOT APPENDED, so that a runner re-using a workspace cannot
    // leave a previous job's measurements in front of this one's.
    let mut out = Vec::new();
    out.extend_from_slice(&restored::encode_job(&job));
    out.extend_from_slice(&restored::encode_cache(cache));
    // THE CLOCK BEFORE THE MEASURING, and its twin after the `after` step's, so
    // the interval the record carries is the widest one this program can see —
    // the cache step and both readings. Erring wide is the safe direction for a
    // number a reader uses to decide whether a cache is worth its price.
    out.extend_from_slice(&restored::encode_at(Side::Before, restored::now_micros()));
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
    let opened = opened(&text);
    // MEASURED FIRST, because what the disk says is what tells the action's
    // THIRD answer from a miswired step — see below.
    let mut out = Vec::new();
    let mut arrived = 0_u64;
    for (path, before) in &opened {
        let measured = measure(path, &home);
        arrived = arrived.saturating_add(measured.bytes.saturating_sub(before.bytes));
        out.extend_from_slice(&restored::encode_side(Side::After, path, &measured));
        println!(
            "[restored] after   {path}: {} entries, {} bytes",
            measured.entries, measured.bytes
        );
    }
    // THE ACTION'S OWN OUTPUT, AND IT HAS THREE ANSWERS AND NOT TWO.
    // `actions/cache` sets `cache-hit` to `true` on an exact key match, to
    // `false` when a `restore-keys` PREFIX matched, and to the EMPTY STRING when
    // nothing matched at all. Until R1112 moved every key this repository had
    // never had a fully cold run, so the third answer had never arrived and this
    // program refused it — three jobs of that run died here.
    //
    // AN EMPTY VALUE IS ALSO WHAT A MISWIRED STEP READS, and the two are told
    // apart by the DISK rather than by the string: nothing matched means nothing
    // arrived. A step reading another step's output while a cache genuinely
    // restored something is the contradiction, and it is still refused.
    // READ DIRECTLY AND NOT THROUGH `required`, which treats empty as missing —
    // right for every other variable here and wrong for this one, because empty
    // is a VALUE `actions/cache` sets and means. An UNSET variable is still a
    // wiring mistake and is still refused.
    let said = std::env::var(restored::EXACT_VARIABLE).unwrap_or_else(|_| {
        fail(&format!(
            "${} is unset — `actions/cache` always sets it, to `true`, `false` \
             or the empty string, so this step is not reading the action at all",
            restored::EXACT_VARIABLE
        ))
    });
    let exact = match said.as_str() {
        "true" => true,
        "false" => false,
        "" if arrived == 0 => false,
        "" => fail(&format!(
            "${} is empty, which `actions/cache` means as `nothing matched` — \
             and yet {arrived} bytes arrived across the restore. One of those \
             two is reading a different step than the other",
            restored::EXACT_VARIABLE
        )),
        other => fail(&format!(
            "${} is {other:?}, which is none of `true`, `false` or empty — \
             `actions/cache` sets it from the primary key, so this step is \
             reading an output that is not there",
            restored::EXACT_VARIABLE
        )),
    };
    // WHICH BUILD OF EACH INSTRUMENT SURVIVED THE RESTORE. R1119, and the moment
    // is this step's by construction: it runs between the restore and the
    // compiling, which is exactly where a substituted binary would already be on
    // disk and not yet have measured anything. This program answers for itself
    // from a constant baked in at compile time; the recorder is ASKED, by running
    // the one `RUSTC_WRAPPER` names — its own answer, not this program's guess
    // about it.
    out.extend_from_slice(&restored::encode_built_from(
        restored::INSTRUMENTS[0],
        restored::built_from(),
    ));
    out.extend_from_slice(&restored::encode_built_from(
        restored::INSTRUMENTS[1],
        &recorder_built_from(),
    ));
    out.extend_from_slice(&restored::encode_at(Side::After, restored::now_micros()));
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

/// Ask the recorder `RUSTC_WRAPPER` names what IT was built from.
///
/// THE PROGRAM ANSWERS, not this one. A path and a version this program derived
/// would be a second reading of the same thing, and the whole point is that the
/// binary on disk may not be the one this commit built. Running it is the only
/// way to be told by the thing itself.
///
/// A REFUSAL RATHER THAN A BLANK. If the recorder cannot be run, this census is
/// about to be taken by something nobody can name — which is the state R1118
/// found and nothing could report. The one case that is NOT a refusal is the
/// variable being deliberately empty: the instruments' own build sets
/// `RUSTC_WRAPPER: ""` to tell cargo there is no wrapper, and a step running
/// under that is not one whose recorder is missing.
fn recorder_built_from() -> String {
    let wrapper = match std::env::var("RUSTC_WRAPPER") {
        Ok(path) if !path.is_empty() => path,
        _ => return "none".to_string(),
    };
    let out = std::process::Command::new(&wrapper)
        .arg("--built-from")
        .output()
        .unwrap_or_else(|error| {
            fail(&format!(
                "cannot run the recorder at {wrapper} ({error}) — every \
                 compilation of this job is about to go through it, so a census \
                 measured by something this step cannot even start is one nobody \
                 can name"
            ))
        });
    if !out.status.success() {
        fail(&format!(
            "the recorder at {wrapper} refused `--built-from` ({}) — a recorder \
             that cannot say which build it is predates this check, and a census \
             it measures reads as one whose measurer agreed with every other",
            out.status
        ));
    }
    let said = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if said.is_empty() {
        fail(&format!(
            "the recorder at {wrapper} answered `--built-from` with nothing"
        ));
    }
    said
}

/// The paths `restored before` wrote down, in the order it wrote them.
///
/// A HALF-RECORD IS READ HERE TOO. `restored::decode` refuses one, and it is
/// right to: a record with no `exact` line is a job that died before the
/// restore was read. But this step is the one that finishes it, so it reads the
/// `before` lines directly and refuses only on what would make its own
/// measurement wrong.
fn opened(text: &str) -> Vec<(String, Measurement)> {
    let mut paths = Vec::new();
    for line in text.lines() {
        let fields: Vec<&str> = line.split(restored::FIELD as char).collect();
        match fields.as_slice() {
            ["job", _] => {}
            // Written by `before` and read by the gate, not by this step: which
            // cache the interval prices is not a question this measurement asks.
            ["cache", _] => {}
            // The clock the first step wrote. This step adds its own and reads
            // neither: what the interval is FOR is downstream of both.
            ["at", _, _] => {}
            // WHAT WAS THERE BEFORE COMES BACK WITH THE PATH, because whether
            // anything ARRIVED is what tells `actions/cache`'s empty answer from
            // a step reading the wrong output.
            [side, path, entries, bytes] if *side == Side::Before.word() => paths.push((
                (*path).to_string(),
                Measurement {
                    entries: reading(entries, line),
                    bytes: reading(bytes, line),
                },
            )),
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

/// A number this step's predecessor wrote, or a refusal naming the line.
fn reading(field: &str, line: &str) -> u64 {
    field.parse().unwrap_or_else(|_| {
        fail(&format!(
            "the record `restored before` wrote holds a measurement that is not \
             a number: {line:?}"
        ))
    })
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
