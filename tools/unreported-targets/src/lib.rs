//! Every test target a run compiled is one that run reported a result for.
//!
//! WHAT HIDES, AND WHY A GREEN DOES NOT SAY SO. `cargo test` stops at the first
//! target that fails. The targets behind it are never asked, and the summary a
//! reader sees — one target's failure count — is the whole of what is printed.
//! Fixing to that number and re-running gives a suite that passes because the
//! first red was repaired, not because the tree is clean.
//!
//! This is not a hypothetical. `scripts/check-side-workspaces.sh` says so in a
//! comment at the line where it passes `--no-fail-fast`: on its first run it
//! reported that `bench` had 6 failures, and the 6 were one target's — there
//! were 18. R1177 then shipped three defects that CI found a round later, and
//! the carry it left is the reason this program exists: `--no-fail-fast` is
//! DISCIPLINE, not a mechanism, and nothing in this tree made a local run cover
//! every target the way CI does.
//!
//! THE POPULATION IS ASKED OF CARGO, NOT WRITTEN DOWN. The set of test targets
//! a command runs is a thing only cargo knows — it is the command's package
//! selection, its features, its `--test`/`--lib` filters and its manifest, all
//! at once. So this gate re-issues THE SAME WORDS with `--no-run
//! --message-format=json` appended and reads the artifacts back. A hand-kept
//! list of "the targets this repository has" would answer a different question
//! and would rot the first time somebody adds a file.
//!
//! WHICH ARTIFACTS, measured rather than assumed. A package with a `[[bin]]`
//! makes cargo emit TWO executables for that target: the plain binary
//! (`profile.test == false`, which no test run ever executes) and its test
//! harness (`profile.test == true`, which gets a `Running` line). Taking every
//! artifact with an `executable` would report the plain binary as a target that
//! never ran, on every clean run, forever. `profile.test` is the discriminator
//! and the fixtures in `tests/law.rs` hold both shapes.
//!
//! IDENTITY IS THE EXECUTABLE'S FILE NAME, hash and all, because that is the one
//! string both sides print. Cargo's `Running` line gives a path relative to the
//! working directory; the JSON gives an absolute one; the file name in between
//! carries the metadata hash, so two targets of the same name in different
//! packages are still two entries.
//!
//! DOC-TESTS ARE OUTSIDE THIS LAW AND ARE COUNTED ANYWAY. `--no-run` compiles no
//! doc-test and reports no artifact for one, so their population cannot be asked
//! of the same program; deriving it would mean re-implementing cargo's target
//! selection, which is a second spelling of the rule cargo already owns. What
//! this gate can do is print how many `Doc-tests` lines the log held, so a
//! reader is never left thinking the number covers them.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

/// What this gate answers about one completed run.
///
/// THREE, and the third one is the point — the contract the gates in this
/// repository share. "Every target reported" and "I could not tell what this run
/// covered" are different answers, and a caller handed one bit prints the
/// finding it knows about a run that has none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// Every test target the command compiled reported a result — or the
    /// command runs no test target at all, which is the same coverage.
    Clean,
    /// At least one target compiled and never ran. The run's verdict, whatever
    /// it said, does not cover them.
    Finding,
    /// No verdict: the run did not print what it covered, or the log and the
    /// command do not explain each other. NOT a finding.
    CouldNotJudge,
}

impl Answer {
    /// The process exit code, which is the whole of what a shell caller sees.
    ///
    /// One home for these numbers, so a gate that starts answering 1 where it
    /// means 2 cannot pass its own suite by having both sides changed to agree.
    #[must_use]
    pub fn exit_code(self) -> i32 {
        match self {
            Answer::Clean => 0,
            Answer::Finding => 1,
            Answer::CouldNotJudge => 2,
        }
    }
}

/// One test executable cargo compiled for the command under judgement.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CompiledTarget {
    /// The package that owns it.
    pub package: String,
    /// Cargo's own word for what the target is: `test`, `lib`, `bin`, `bench`.
    pub kind: String,
    /// The target's name — what a `--test <name>` selector spells.
    pub name: String,
    /// The executable's file name, hash and all. THE IDENTITY.
    pub file: String,
}

/// What the log says the run actually reached.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reported {
    /// The file name out of every `Running … (<path>)` line, in the order the
    /// run printed them.
    pub files: Vec<String>,
    /// `Doc-tests <crate>` lines. Outside this law; see the module docs.
    pub doc_tests: usize,
}

/// What asking cargo for the population comes to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Population {
    /// The words to run. The command's own, plus `--no-run
    /// --message-format=json`, minus the ones that would answer a different
    /// question.
    Ask(Vec<String>),
    /// The command runs no test target, so there is nothing for its verdict to
    /// cover. Carries the reason, because a 0 with no sentence behind it is the
    /// shape of a gate that never looked.
    Vacuous(String),
    /// The command runs test targets and does not print which ones it reached.
    Unreadable(String),
}

/// What one run came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// The words that produced the log.
    pub command: Vec<String>,
    /// What judging them came to.
    pub outcome: Outcome,
}

/// The three shapes a judgement of one run takes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Nothing to cover, and why.
    Vacuous(String),
    /// Nothing readable, and why.
    Unreadable(String),
    /// The run was judged.
    Judged(Judged),
}

/// A judged run, with both sides of the comparison kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Judged {
    /// The words this gate asked cargo, printed so a reader can re-run them.
    pub asked: Vec<String>,
    /// Every test executable that command compiles.
    pub compiled: Vec<CompiledTarget>,
    /// What the log said ran.
    pub reported: Reported,
    /// Compiled and never reported. THE FINDING.
    pub unreported: Vec<CompiledTarget>,
    /// Reported and never compiled — a log and a command that do not explain
    /// each other, which is a refusal rather than a finding.
    pub unexplained: Vec<String>,
}

impl Report {
    /// The answer, from what the judgement came to.
    ///
    /// INCOHERENCE OUTRANKS THE FINDING. When the log holds a target this
    /// command does not compile, the two are not about the same run — so the
    /// missing targets are not evidence of anything either, and reporting them
    /// sends a reader hunting a target that was never meant to be there.
    #[must_use]
    pub fn answer(&self) -> Answer {
        match &self.outcome {
            Outcome::Vacuous(_) => Answer::Clean,
            Outcome::Unreadable(_) => Answer::CouldNotJudge,
            Outcome::Judged(judged) => {
                if !judged.unexplained.is_empty() {
                    Answer::CouldNotJudge
                } else if !judged.unreported.is_empty() {
                    Answer::Finding
                } else {
                    Answer::Clean
                }
            }
        }
    }
}

/// How many mismatched names a refusal shows before it stops naming them.
///
/// NOT A SILENT CAP — the line that stops says how many it did not print. The
/// list under a refusal is a SYMPTOM, evidence that the log belongs to another
/// run, and one measured case put 57 names in it: a log from the build machine
/// judged against this checkout, where every metadata hash differs because the
/// package paths do.
const NAMES_SHOWN_UNDER_A_REFUSAL: usize = 10;

/// What one report says, in the order a reader should meet it.
///
/// THE ORDER IS THE PRECEDENCE, and getting it wrong is a defect this round
/// shipped and measured before repairing. Judged against a log from another
/// machine's tree, the first draft printed fifty-seven `NEVER RAN` lines and
/// then said "no opinion about this run" — the exact failure the answer's own
/// documentation warns about, a finding printed about a run that has none. When
/// the log and the command are not about the same run, the targets missing from
/// the log follow from the mismatch and are a COUNT here, never a list of names
/// a reader would go hunting.
///
/// Returned rather than printed so the decision has a test. A gate whose output
/// can only be checked by running the binary and reading it is one whose
/// ordering rots silently.
#[must_use]
pub fn report_lines(report: &Report) -> Vec<String> {
    let mut out = vec![format!("command: {}", report.command.join(" "))];
    match &report.outcome {
        Outcome::Vacuous(reason) => out.push(format!("nothing to cover — {reason}")),
        Outcome::Unreadable(reason) => out.push(format!("NO VERDICT — {reason}")),
        Outcome::Judged(judged) => {
            out.push(format!("asked: {}", judged.asked.join(" ")));
            out.push(format!(
                "{} test target(s) compiled, {} reported a result, {} doc-test run(s) \
                 outside this law",
                judged.compiled.len(),
                judged.reported.files.len(),
                judged.reported.doc_tests,
            ));
            if judged.unexplained.is_empty() {
                // FINDINGS ARE NEVER CAPPED. Each of these is a target a reader
                // has to decide about, and a list that stops early is a smaller
                // number than the truth — which is the very thing this gate was
                // built to stop somebody fixing to.
                for target in &judged.unreported {
                    out.push(format!(
                        "NEVER RAN {} {} of {} ({})",
                        target.kind, target.name, target.package, target.file
                    ));
                }
                return out;
            }
            for file in judged.unexplained.iter().take(NAMES_SHOWN_UNDER_A_REFUSAL) {
                out.push(format!(
                    "the log reports {file}, which this command does not build"
                ));
            }
            if judged.unexplained.len() > NAMES_SHOWN_UNDER_A_REFUSAL {
                out.push(format!(
                    "… and {} more the log reports that this command does not build",
                    judged.unexplained.len() - NAMES_SHOWN_UNDER_A_REFUSAL
                ));
            }
            out.push(format!(
                "so this log is not this command's. {} target(s) it builds are absent \
                 from that log too, which follows from the mismatch and is NOT a finding",
                judged.unreported.len()
            ));
        }
    }
    out
}

/// The cargo this process should run, pinned to the one that built it when
/// there is one.
///
/// `Command::new("cargo")` lets PATH decide, and PATH is a fact about the
/// machine rather than about this repository — R1190 found two tests in this
/// tree asserting things about a pinned revision through whichever cargo the
/// machine happened to offer.
#[must_use]
pub fn cargo() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string())
}

/// Turn the command that produced a log into the command that answers what it
/// compiled.
///
/// WHAT IS DROPPED AND WHY EACH ONE.
///
/// * everything after a bare `--`: those are the harness's arguments, not
///   cargo's. They decide which TESTS run inside a target and never which
///   targets are built, and passing them to `--no-run` is at best noise.
/// * `--message-format`: this gate needs JSON back, and cargo takes the last
///   one given. Leaving the command's own in would work by argument order,
///   which is a thing that changes.
///
/// AND TWO SHAPES ARE REFUSED RATHER THAN ANSWERED. `--quiet` suppresses the
/// very `Running` lines this gate reads, and a command whose own output was
/// `--message-format=json` printed artifacts where the lines would be: in both
/// the log is silent about coverage, and a silent log must not read as a run
/// that covered nothing.
#[must_use]
pub fn population_command(argv: &[String]) -> Population {
    let mut words = argv.iter().map(String::as_str);
    let Some(program) = words.next() else {
        return Population::Vacuous("the command is empty".to_string());
    };
    if !(program == "cargo" || program.ends_with("/cargo")) {
        return Population::Vacuous(format!("`{program}` is not cargo"));
    }
    // A `+toolchain` word belongs to rustup and sits before the subcommand.
    let mut rest: Vec<&str> = words.collect();
    let mut lead: Vec<&str> = vec![program];
    if rest.first().is_some_and(|word| word.starts_with('+')) {
        lead.push(rest.remove(0));
    }
    match rest.first() {
        Some(&"test") => {}
        Some(other) => {
            return Population::Vacuous(format!("`cargo {other}` runs no test target"));
        }
        None => return Population::Vacuous("`cargo` was given no subcommand".to_string()),
    }

    let cargo_words: Vec<&str> = match rest.iter().position(|word| *word == "--") {
        Some(at) => rest[..at].to_vec(),
        None => rest.clone(),
    };

    let mut kept: Vec<String> = lead.iter().map(|word| (*word).to_string()).collect();
    for word in &cargo_words {
        if *word == "--no-run" {
            return Population::Vacuous("the command asked cargo not to run anything".to_string());
        }
        if *word == "-q" || *word == "--quiet" {
            return Population::Unreadable(
                "the command was quiet, so cargo printed no `Running` line for any target \
                 it reached"
                    .to_string(),
            );
        }
        if *word == "--message-format" || word.starts_with("--message-format=") {
            return Population::Unreadable(
                "the command asked for a machine message format, so its log holds \
                 artifacts where the `Running` lines would be"
                    .to_string(),
            );
        }
        kept.push((*word).to_string());
    }
    kept.push("--no-run".to_string());
    kept.push("--message-format=json".to_string());
    Population::Ask(kept)
}

/// Read cargo's JSON stream for the test executables it built.
///
/// A line this gate cannot parse is an ERROR rather than a skip: the stream is
/// one message per line by construction, so a line that is not one means the
/// output is not what this gate thinks it is reading.
pub fn compiled_from_json(stream: &str) -> Result<Vec<CompiledTarget>, String> {
    let mut out = Vec::new();
    for line in stream.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let message: serde_json::Value = serde_json::from_str(line)
            .map_err(|error| format!("cargo printed a line that is not JSON: {error}"))?;
        if message.get("reason").and_then(serde_json::Value::as_str) != Some("compiler-artifact") {
            continue;
        }
        let Some(executable) = message
            .get("executable")
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        let is_test = message
            .get("profile")
            .and_then(|profile| profile.get("test"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        if !is_test {
            continue;
        }
        let target = message.get("target");
        let name = target
            .and_then(|target| target.get("name"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("?")
            .to_string();
        let kind = target
            .and_then(|target| target.get("kind"))
            .and_then(serde_json::Value::as_array)
            .and_then(|kinds| kinds.first())
            .and_then(serde_json::Value::as_str)
            .unwrap_or("?")
            .to_string();
        let package = message
            .get("package_id")
            .and_then(serde_json::Value::as_str)
            .map(package_name)
            .unwrap_or_else(|| "?".to_string());
        out.push(CompiledTarget {
            package,
            kind,
            name,
            file: file_name(executable).to_string(),
        });
    }
    out.sort();
    Ok(out)
}

/// The readable half of a cargo package id.
///
/// Cargo has printed two shapes of these and both are still in the wild:
/// `path+file:///w/probe#0.1.0`, where the name is the last path segment, and
/// `path+file:///w/probe#probe@0.1.0`, where it is spelled after the `#`.
#[must_use]
pub fn package_name(id: &str) -> String {
    let (before, after) = id.split_once('#').unwrap_or((id, ""));
    if let Some((name, _version)) = after.split_once('@') {
        return name.to_string();
    }
    if !after.is_empty() && !after.starts_with(|c: char| c.is_ascii_digit()) {
        return after.to_string();
    }
    before.rsplit('/').next().unwrap_or(before).to_string()
}

fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// Read a completed run's own output for the targets it reached.
///
/// COLOUR IS STRIPPED FIRST. Cargo drops colour when its output is a pipe, which
/// is the normal case here — but `CARGO_TERM_COLOR=always` is a thing a machine
/// can be set up with, and a gate that reads `Running` only when it arrives bare
/// answers "nothing ran" on a tree where everything did.
#[must_use]
pub fn reported_from_log(log: &str) -> Reported {
    let mut reported = Reported::default();
    for raw in log.lines() {
        let line = without_ansi(raw);
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("Running ") {
            if let Some(path) = parenthesised(rest) {
                reported.files.push(file_name(path).to_string());
            }
            continue;
        }
        if line.starts_with("Doc-tests ") {
            reported.doc_tests += 1;
        }
    }
    reported
}

/// The path cargo puts in brackets at the end of a `Running` line.
fn parenthesised(rest: &str) -> Option<&str> {
    let rest = rest.trim_end();
    let inner = rest.strip_suffix(')')?;
    let open = inner.rfind('(')?;
    Some(&inner[open + 1..])
}

fn without_ansi(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(character) = chars.next() {
        if character != '\u{1b}' {
            out.push(character);
            continue;
        }
        for terminator in chars.by_ref() {
            if terminator.is_ascii_alphabetic() {
                break;
            }
        }
    }
    out
}

/// Hold the two sides against each other.
///
/// Both directions are returned. A target compiled and never reported is the
/// finding this gate exists for; a target reported and never compiled means the
/// log and the command are not about the same run, which is a different answer
/// and must not be printed as the first one.
#[must_use]
pub fn compare(
    compiled: &[CompiledTarget],
    reported: &Reported,
) -> (Vec<CompiledTarget>, Vec<String>) {
    let ran: BTreeSet<&str> = reported.files.iter().map(String::as_str).collect();
    let built: BTreeMap<&str, &CompiledTarget> = compiled
        .iter()
        .map(|target| (target.file.as_str(), target))
        .collect();
    let unreported = compiled
        .iter()
        .filter(|target| !ran.contains(target.file.as_str()))
        .cloned()
        .collect();
    let unexplained = ran
        .iter()
        .filter(|file| !built.contains_key(*file))
        .map(|file| (*file).to_string())
        .collect();
    (unreported, unexplained)
}

/// Judge one completed run: its log, and the words that produced it.
///
/// `at` is the directory the command ran in, because that is what a relative
/// `--manifest-path` and cargo's own workspace search both resolve against.
pub fn run(log: &Path, argv: &[String], at: &Path) -> Result<Report, String> {
    let text = std::fs::read_to_string(log)
        .map_err(|error| format!("cannot read the run's log at {}: {error}", log.display()))?;
    let command = argv.to_vec();
    let asked = match population_command(argv) {
        Population::Vacuous(reason) => {
            return Ok(Report {
                command,
                outcome: Outcome::Vacuous(reason),
            })
        }
        Population::Unreadable(reason) => {
            return Ok(Report {
                command,
                outcome: Outcome::Unreadable(reason),
            })
        }
        Population::Ask(words) => words,
    };

    let program = if asked[0] == "cargo" {
        cargo()
    } else {
        asked[0].clone()
    };
    let output = Command::new(&program)
        .args(&asked[1..])
        .current_dir(at)
        .output()
        .map_err(|error| format!("cannot run `{}`: {error}", asked.join(" ")))?;
    if !output.status.success() {
        return Err(format!(
            "`{}` did not succeed ({}), so what the run compiled cannot be asked:\n{}",
            asked.join(" "),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim_end(),
        ));
    }
    let compiled = compiled_from_json(&String::from_utf8_lossy(&output.stdout))?;
    let reported = reported_from_log(&text);
    let (unreported, unexplained) = compare(&compiled, &reported);
    Ok(Report {
        command,
        outcome: Outcome::Judged(Judged {
            asked,
            compiled,
            reported,
            unreported,
            unexplained,
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(line: &str) -> Vec<String> {
        line.split_whitespace().map(str::to_string).collect()
    }

    #[test]
    fn a_test_command_becomes_the_question_of_what_it_compiles() {
        assert_eq!(
            population_command(&words("cargo test --workspace --locked")),
            Population::Ask(words(
                "cargo test --workspace --locked --no-run --message-format=json"
            ))
        );
    }

    #[test]
    fn a_toolchain_word_stays_in_front_of_the_subcommand() {
        assert_eq!(
            population_command(&words("cargo +nightly test --lib")),
            Population::Ask(words(
                "cargo +nightly test --lib --no-run --message-format=json"
            ))
        );
    }

    #[test]
    fn harness_arguments_are_not_cargos_and_are_dropped() {
        assert_eq!(
            population_command(&words("cargo test --workspace -- --ignored --nocapture")),
            Population::Ask(words(
                "cargo test --workspace --no-run --message-format=json"
            ))
        );
    }

    #[test]
    fn an_absolute_cargo_is_still_cargo() {
        assert_eq!(
            population_command(&words("/home/x/.cargo/bin/cargo test --lib")),
            Population::Ask(words(
                "/home/x/.cargo/bin/cargo test --lib --no-run --message-format=json"
            ))
        );
    }

    #[test]
    fn a_command_that_is_not_a_test_run_carries_no_coverage_question() {
        for line in [
            "cargo clippy --workspace",
            "cargo run -p mnemosyne-cli",
            "scripts/mn validate-workspace",
        ] {
            assert!(
                matches!(population_command(&words(line)), Population::Vacuous(_)),
                "{line}"
            );
        }
    }

    #[test]
    fn asking_cargo_not_to_run_is_vacuous_rather_than_a_finding() {
        assert!(matches!(
            population_command(&words("cargo test --workspace --no-run")),
            Population::Vacuous(_)
        ));
    }

    /// A quiet run prints no `Running` line, so its log is silent about
    /// coverage — and silence must not read as a run that covered nothing.
    #[test]
    fn a_silent_log_is_refused_rather_than_read_as_empty() {
        for line in [
            "cargo test -q --workspace",
            "cargo test --quiet --workspace",
            "cargo test --workspace --message-format=json",
            "cargo test --workspace --message-format json",
        ] {
            assert!(
                matches!(population_command(&words(line)), Population::Unreadable(_)),
                "{line}"
            );
        }
    }

    #[test]
    fn the_log_gives_up_the_file_name_of_every_target_that_ran() {
        let log = "\
   Compiling probe v0.1.0 (/w/probe)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.33s
     Running unittests src/lib.rs (t/debug/deps/probe-dd205ac96b82505c)
     Running tests/alpha.rs (t/debug/deps/alpha-39d092b6e80606ce)
   Doc-tests probe
";
        let reported = reported_from_log(log);
        assert_eq!(
            reported.files,
            vec![
                "probe-dd205ac96b82505c".to_string(),
                "alpha-39d092b6e80606ce".to_string()
            ]
        );
        assert_eq!(reported.doc_tests, 1);
    }

    #[test]
    fn colour_does_not_hide_a_target_that_ran() {
        let log = "     \u{1b}[0m\u{1b}[0m\u{1b}[1m\u{1b}[32mRunning\u{1b}[0m tests/alpha.rs \
                   (t/debug/deps/alpha-39d092b6e80606ce)";
        assert_eq!(
            reported_from_log(log).files,
            vec!["alpha-39d092b6e80606ce".to_string()]
        );
    }

    #[test]
    fn only_the_artifacts_a_test_run_executes_are_the_population() {
        // The measured shape: a `[[bin]]` yields the plain binary
        // (`profile.test == false`, never run) and its test harness.
        let stream = r#"
{"reason":"compiler-artifact","package_id":"path+file:///w/probe#0.1.0","target":{"kind":["bin"],"name":"probe"},"profile":{"test":false},"executable":"/w/probe/t/debug/probe"}
{"reason":"compiler-artifact","package_id":"path+file:///w/probe#0.1.0","target":{"kind":["bin"],"name":"probe"},"profile":{"test":true},"executable":"/w/probe/t/debug/deps/probe-8b6c970106b4a5f8"}
{"reason":"compiler-artifact","package_id":"path+file:///w/probe#0.1.0","target":{"kind":["lib"],"name":"probe"},"profile":{"test":false},"executable":null}
{"reason":"build-finished","success":true}
"#;
        let compiled = compiled_from_json(stream).expect("the stream parses");
        assert_eq!(compiled.len(), 1, "{compiled:?}");
        assert_eq!(compiled[0].file, "probe-8b6c970106b4a5f8");
        assert_eq!(compiled[0].kind, "bin");
        assert_eq!(compiled[0].package, "probe");
    }

    #[test]
    fn both_package_id_spellings_give_up_the_name() {
        assert_eq!(package_name("path+file:///w/probe#0.1.0"), "probe");
        assert_eq!(package_name("path+file:///w/probe#probe@0.1.0"), "probe");
        assert_eq!(
            package_name("registry+https://github.com/rust-lang/crates.io-index#serde@1.0.0"),
            "serde"
        );
        assert_eq!(
            package_name("path+file:///w/some-crate#other-name"),
            "other-name"
        );
    }

    fn target(file: &str) -> CompiledTarget {
        CompiledTarget {
            package: "probe".to_string(),
            kind: "test".to_string(),
            name: file.split('-').next().unwrap_or(file).to_string(),
            file: file.to_string(),
        }
    }

    fn ran(files: &[&str]) -> Reported {
        Reported {
            files: files.iter().map(|file| (*file).to_string()).collect(),
            doc_tests: 0,
        }
    }

    #[test]
    fn a_target_that_compiled_and_never_ran_is_the_finding() {
        let compiled = vec![target("alpha-1"), target("beta-2")];
        let (unreported, unexplained) = compare(&compiled, &ran(&["alpha-1"]));
        assert_eq!(unreported, vec![target("beta-2")]);
        assert!(unexplained.is_empty());
    }

    #[test]
    fn a_log_naming_a_target_this_command_does_not_build_is_not_about_this_run() {
        let compiled = vec![target("alpha-1")];
        let (unreported, unexplained) = compare(&compiled, &ran(&["alpha-1", "gamma-9"]));
        assert!(unreported.is_empty());
        assert_eq!(unexplained, vec!["gamma-9".to_string()]);
    }

    fn judged(unreported: Vec<CompiledTarget>, unexplained: Vec<String>) -> Report {
        Report {
            command: words("cargo test"),
            outcome: Outcome::Judged(Judged {
                asked: words("cargo test --no-run --message-format=json"),
                compiled: Vec::new(),
                reported: Reported::default(),
                unreported,
                unexplained,
            }),
        }
    }

    #[test]
    fn incoherence_outranks_the_finding() {
        assert_eq!(
            judged(vec![target("beta-2")], vec!["gamma-9".to_string()]).answer(),
            Answer::CouldNotJudge
        );
        assert_eq!(
            judged(vec![target("beta-2")], Vec::new()).answer(),
            Answer::Finding
        );
        assert_eq!(judged(Vec::new(), Vec::new()).answer(), Answer::Clean);
    }

    #[test]
    fn a_vacuous_command_is_clean_and_an_unreadable_one_is_not_judged() {
        let vacuous = Report {
            command: words("cargo clippy"),
            outcome: Outcome::Vacuous("`cargo clippy` runs no test target".to_string()),
        };
        assert_eq!(vacuous.answer(), Answer::Clean);
        let unreadable = Report {
            command: words("cargo test -q"),
            outcome: Outcome::Unreadable("the command was quiet".to_string()),
        };
        assert_eq!(unreadable.answer(), Answer::CouldNotJudge);
    }

    /// THE DEFECT THIS ROUND SHIPPED AND MEASURED. A log from another machine's
    /// tree makes every metadata hash differ, so the command's own targets are
    /// all "missing" from it — and the first draft printed all of them as
    /// findings above the sentence saying it had no opinion.
    #[test]
    fn a_refusal_names_no_finding() {
        let report = judged(
            vec![target("beta-2"), target("gamma-3")],
            vec!["delta-9".to_string()],
        );
        let lines = report_lines(&report);
        assert!(
            !lines.iter().any(|line| line.contains("NEVER RAN")),
            "{lines:#?}"
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("2 target(s) it builds are absent")
                    && line.contains("NOT a finding")),
            "{lines:#?}"
        );
    }

    #[test]
    fn a_refusal_says_how_many_names_it_did_not_print() {
        let unexplained: Vec<String> = (0..57).map(|n| format!("target-{n}")).collect();
        let lines = report_lines(&judged(Vec::new(), unexplained));
        let named = lines
            .iter()
            .filter(|line| line.contains("which this command does not build"))
            .count();
        assert_eq!(named, NAMES_SHOWN_UNDER_A_REFUSAL, "{lines:#?}");
        assert!(
            lines.iter().any(|line| line.contains("… and 47 more")),
            "{lines:#?}"
        );
    }

    /// The other half of the cap, and the reason it is not one number for both:
    /// every finding is a target somebody has to decide about, and a list that
    /// stops early is the smaller-than-the-truth number this gate exists to stop
    /// anyone fixing to.
    #[test]
    fn findings_are_never_capped() {
        let unreported: Vec<CompiledTarget> =
            (0..30).map(|n| target(&format!("target{n}-{n}"))).collect();
        let lines = report_lines(&judged(unreported, Vec::new()));
        assert_eq!(
            lines
                .iter()
                .filter(|line| line.starts_with("NEVER RAN"))
                .count(),
            30,
            "{lines:#?}"
        );
    }

    #[test]
    fn a_clean_run_still_prints_its_numbers() {
        let lines = report_lines(&judged(Vec::new(), Vec::new()));
        assert!(
            lines
                .iter()
                .any(|line| line.contains("test target(s) compiled")),
            "{lines:#?}"
        );
    }

    #[test]
    fn the_three_answers_are_three_codes() {
        let codes: BTreeSet<i32> = [Answer::Clean, Answer::Finding, Answer::CouldNotJudge]
            .iter()
            .map(|answer| answer.exit_code())
            .collect();
        assert_eq!(codes.len(), 3);
    }
}
