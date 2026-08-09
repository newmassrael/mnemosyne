//! What the wrapper promises: a record that decodes back into the arguments it
//! was given AND what they cost, survives cargo running dozens of compilers at
//! once, is never quietly skipped, and hands cargo the compiler's own verdict.

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use rustc_log::{append, decode, decode_all, encode, now_micros, Record, LOG_VARIABLE};

fn words(list: &[&str]) -> Vec<String> {
    list.iter().map(|word| word.to_string()).collect()
}

/// A record with the clock filled in by hand, for the tests that are about the
/// format rather than about the wrapper.
fn record(started_at: u64, micros: u64, argv: &[&str]) -> Record {
    Record {
        started_at,
        micros,
        argv: words(argv),
    }
}

/// How long the stand-in compiler is told to run.
///
/// Named rather than spelled at the sites below because the two tests that use
/// it are making the SAME claim about the machine, and it is the weakest claim
/// available: a `sleep` runs for at least this long, so an assertion that the
/// recorded duration is at least this cannot fail on a loaded runner. Nothing
/// here asserts an upper bound, which is the assertion that would.
const A_COMPILATION_THIS_LONG: Duration = Duration::from_millis(300);

/// A scratch directory that removes itself, so a failing test leaves nothing
/// behind in the tree it is judging.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let mut path = std::env::temp_dir();
        path.push(format!("rustc-log-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("scratch directory");
        Scratch(path)
    }

    fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// --- the format -------------------------------------------------------------

#[test]
fn a_record_decodes_into_the_arguments_it_was_given_and_what_they_cost() {
    // The shape cargo actually passes: a compiler path, then arguments that
    // contain the two characters a naive `split_whitespace` reader would break
    // on — an `=`-joined value and a quoted one with a space in it.
    let written = record(
        1_754_700_000_000_000,
        4_321_000,
        &[
            "/home/runner/.rustup/toolchains/stable/bin/rustc",
            "--crate-name",
            "mnemosyne_core",
            "--cfg",
            r#"feature="disclosure""#,
            "-C",
            "link-arg=-Wl,-rpath,/a b/c",
        ],
    );
    let encoded = encode(&written);
    let text = String::from_utf8(encoded).expect("records are utf-8");
    let line = text.strip_suffix('\n').expect("a record ends in a newline");
    assert_eq!(decode(line), written);
    assert_eq!(
        written.ended_at(),
        1_754_700_004_321_000,
        "a record says when the compiler exited by saying when it started and \
         how long it ran"
    );
}

#[test]
#[should_panic(expected = "refusing rather than writing a record")]
fn an_argument_holding_a_separator_is_refused_not_escaped() {
    encode(&record(0, 0, &["rustc", "--crate-name\u{1f}smuggled"]));
}

#[test]
#[should_panic(expected = "which is not microseconds")]
fn a_record_whose_first_field_is_not_a_clock_is_refused() {
    // THE OLD FORMAT IS THE CASE THIS IS FOR. Before the record carried a clock
    // its first field was the compiler's path, and a log left over from that
    // wrapper still decodes into plausible-looking arguments — with every
    // compilation costing nothing. Free work is precisely the finding a reader
    // of these logs is hunting for, so the wrong answer here is the one that
    // looks like an argument for merging every job in the file.
    decode("/usr/bin/rustc\u{1f}--crate-name\u{1f}mnemosyne_core");
}

#[test]
fn every_record_survives_many_compilers_appending_at_once() {
    // THE ONE CLAIM A COMMENT CANNOT MAKE. cargo runs as many `rustc` processes
    // as the machine has cores, every one of them appending here, and a torn
    // record is a compilation the gate downstream would either miscount or
    // refuse to parse. Threads rather than processes only because the write path
    // is the same either way — one `write` on an `O_APPEND` handle.
    let scratch = Scratch::new("concurrent");
    let log = scratch.file("rustc.log");

    let writers = 16;
    let each = 64;
    // Long enough that a torn write would be visible: a short record can slip
    // under a buffer boundary by luck.
    let padding = "x".repeat(4096);

    std::thread::scope(|scope| {
        for writer in 0..writers {
            let log = log.clone();
            let padding = padding.clone();
            scope.spawn(move || {
                for index in 0..each {
                    append(
                        &log,
                        &record(
                            writer as u64,
                            index as u64,
                            &["rustc", &format!("{writer}:{index}"), &padding],
                        ),
                    )
                    .expect("append");
                }
            });
        }
    });

    let text = std::fs::read_to_string(&log).expect("read log");
    let records = decode_all(&text);
    assert_eq!(
        records.len(),
        writers * each,
        "every append is one record, and no record is two"
    );
    for written in &records {
        assert_eq!(written.argv.len(), 3, "a torn record: {written:?}");
        assert_eq!(written.argv[0], "rustc");
        assert_eq!(written.argv[2], padding, "a torn record's tail");
        assert_eq!(
            written.argv[1],
            format!("{}:{}", written.started_at, written.micros),
            "a record's clock and its arguments came from the same append"
        );
    }
    let mut seen: Vec<&str> = records
        .iter()
        .map(|written| written.argv[1].as_str())
        .collect();
    seen.sort();
    seen.dedup();
    assert_eq!(seen.len(), writers * each, "a record went missing");
}

// --- the wrapper ------------------------------------------------------------

#[test]
fn the_wrapper_records_the_invocation_and_then_runs_the_compiler() {
    let scratch = Scratch::new("passthrough");
    let log = scratch.file("rustc.log");

    let out = Command::new(env!("CARGO_BIN_EXE_rustc-log"))
        .args(["rustc", "--version"])
        .env(LOG_VARIABLE, &log)
        .output()
        .expect("the wrapper runs");

    assert!(
        out.status.success(),
        "the wrapper must exit as the compiler did: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        String::from_utf8_lossy(&out.stdout).starts_with("rustc "),
        "the wrapper must RUN the compiler, not just record it: {:?}",
        String::from_utf8_lossy(&out.stdout)
    );

    let text = std::fs::read_to_string(&log).expect("the wrapper wrote a log");
    let records = decode_all(&text);
    assert_eq!(records.len(), 1, "one invocation, one record");
    assert_eq!(
        records[0].argv,
        words(&["rustc", "--version"]),
        "the record holds the compiler and its arguments, compiler first"
    );
}

#[test]
fn the_record_says_how_long_the_compiler_ran_and_when() {
    // WHY THE WRAPPER WAITS INSTEAD OF BECOMING THE COMPILER. An `exec` leaves
    // no moment at which the duration is known, and a census that counts
    // compilations without pricing them ranks the repair by where the rows are
    // rather than by where the minutes are.
    //
    // The claim is a LOWER BOUND on the duration and a WINDOW around the start,
    // both of which are true however slow the machine is. An upper bound on the
    // duration is the assertion a loaded runner would break, and there is none.
    let scratch = Scratch::new("duration");
    let log = scratch.file("rustc.log");
    let stand_in = format!("sleep {}", A_COMPILATION_THIS_LONG.as_secs_f64());

    let before = now_micros();
    let out = Command::new(env!("CARGO_BIN_EXE_rustc-log"))
        .args(["sh", "-c", &stand_in])
        .env(LOG_VARIABLE, &log)
        .output()
        .expect("the wrapper runs");
    let after = now_micros();
    assert!(out.status.success(), "the stand-in compiler ran");

    let text = std::fs::read_to_string(&log).expect("the wrapper wrote a log");
    let records = decode_all(&text);
    assert_eq!(records.len(), 1);
    let written = &records[0];

    let least = u64::try_from(A_COMPILATION_THIS_LONG.as_micros()).expect("microseconds");
    assert!(
        written.micros >= least,
        "the compiler ran for at least {least} µs and the record says {} µs",
        written.micros
    );
    // A CONSTANT WOULD PASS THE ASSERTION ABOVE if it were large enough; this is
    // what makes the clock a clock. The compiler cannot have started before this
    // test spawned it, nor finished after this test read the status back.
    assert!(
        written.started_at >= before && written.ended_at() <= after,
        "the record's window {}..{} is not inside the test's {before}..{after}",
        written.started_at,
        written.ended_at()
    );
}

#[test]
fn an_unrecorded_invocation_is_refused_rather_than_passed_through() {
    // The control for the test above: with nowhere to write, the wrapper must
    // NOT quietly become the compiler. A job wired without the variable would
    // otherwise build perfectly and hand the gate an empty log, which reads the
    // same as a job with nothing to compile.
    let out = Command::new(env!("CARGO_BIN_EXE_rustc-log"))
        .args(["rustc", "--version"])
        .env_remove(LOG_VARIABLE)
        .output()
        .expect("the wrapper runs");

    assert!(
        !out.status.success(),
        "an unrecorded build must not succeed"
    );
    assert!(
        String::from_utf8_lossy(&out.stdout).is_empty(),
        "it must not have run the compiler at all"
    );
    assert!(
        String::from_utf8_lossy(&out.stderr).contains(LOG_VARIABLE),
        "it must say which variable is missing"
    );
}

#[test]
fn a_compilation_that_failed_is_reported_as_failed_and_still_recorded() {
    // WHAT WAITING PUTS AT RISK. With `exec`, cargo saw the compiler's own exit
    // status because it WAS the compiler; now a process stands between them and
    // has to hand it over unchanged. A wrapper that swallowed the code would
    // turn every broken build green, which no other gate in this repository
    // would notice.
    let scratch = Scratch::new("failure");
    let log = scratch.file("rustc.log");

    let out = Command::new(env!("CARGO_BIN_EXE_rustc-log"))
        .args(["sh", "-c", "exit 7"])
        .env(LOG_VARIABLE, &log)
        .output()
        .expect("the wrapper runs");

    assert_eq!(
        out.status.code(),
        Some(7),
        "the compiler's exit code is cargo's to read, not this wrapper's to \
         replace"
    );
    let text = std::fs::read_to_string(&log).expect("the wrapper wrote a log");
    assert_eq!(
        decode_all(&text).len(),
        1,
        "a build that broke still paid for the compilation that broke it, and \
         the census is of what CI paid for"
    );
}

#[cfg(unix)]
#[test]
fn a_compiler_the_kernel_killed_is_not_handed_over_as_success() {
    // THE OTHER HALF OF THE STATUS. A `rustc` the kernel kills — out of memory,
    // which is the way a hosted runner does it — exits by signal and has no code
    // of its own. `status.code()` is `None` there, and the obvious
    // `unwrap_or(0)` turns the loudest failure a build can have into a pass.
    let scratch = Scratch::new("signal");
    let log = scratch.file("rustc.log");

    let out = Command::new(env!("CARGO_BIN_EXE_rustc-log"))
        .args(["sh", "-c", "kill -KILL $$"])
        .env(LOG_VARIABLE, &log)
        .output()
        .expect("the wrapper runs");

    assert!(!out.status.success(), "a killed compiler is a failed build");
    assert_eq!(
        out.status.code(),
        Some(128 + 9),
        "and the signal that killed it is named in the code, the shell's way"
    );
}
