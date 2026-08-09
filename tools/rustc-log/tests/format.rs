//! What the wrapper promises: a record that decodes back into the arguments it
//! was given, survives cargo running dozens of compilers at once, and is never
//! quietly skipped.

use std::path::PathBuf;
use std::process::Command;

use rustc_log::{append, decode, decode_all, encode, LOG_VARIABLE};

fn words(list: &[&str]) -> Vec<String> {
    list.iter().map(|word| word.to_string()).collect()
}

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

#[test]
fn a_record_decodes_into_the_arguments_it_was_given() {
    // The shape cargo actually passes: a compiler path, then arguments that
    // contain the two characters a naive `split_whitespace` reader would break
    // on — an `=`-joined value and a quoted one with a space in it.
    let argv = words(&[
        "/home/runner/.rustup/toolchains/stable/bin/rustc",
        "--crate-name",
        "mnemosyne_core",
        "--cfg",
        r#"feature="disclosure""#,
        "-C",
        "link-arg=-Wl,-rpath,/a b/c",
    ]);
    let encoded = encode(&argv);
    let text = String::from_utf8(encoded).expect("records are utf-8");
    let line = text.strip_suffix('\n').expect("a record ends in a newline");
    assert_eq!(decode(line), argv);
}

#[test]
#[should_panic(expected = "refusing rather than writing a record")]
fn an_argument_holding_a_separator_is_refused_not_escaped() {
    encode(&words(&["rustc", "--crate-name\u{1f}smuggled"]));
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
                        &words(&["rustc", &format!("{writer}:{index}"), &padding]),
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
    for record in &records {
        assert_eq!(record.len(), 3, "a torn record: {record:?}");
        assert_eq!(record[0], "rustc");
        assert_eq!(record[2], padding, "a torn record's tail");
    }
    let mut seen: Vec<&str> = records.iter().map(|record| record[1].as_str()).collect();
    seen.sort();
    seen.dedup();
    assert_eq!(seen.len(), writers * each, "a record went missing");
}

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
        "the wrapper must BECOME the compiler, not just record it: {:?}",
        String::from_utf8_lossy(&out.stdout)
    );

    let text = std::fs::read_to_string(&log).expect("the wrapper wrote a log");
    assert_eq!(
        decode_all(&text),
        vec![words(&["rustc", "--version"])],
        "the record holds the compiler and its arguments, compiler first"
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
