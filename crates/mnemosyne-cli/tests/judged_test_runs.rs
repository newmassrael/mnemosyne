//! A suite nobody wrapped is a suite nobody judged.
//!
//! R1196. `tools/unreported-targets` answers whether a completed run's verdict
//! covered every test target that run compiled — the law R1194 built after
//! R1177 shipped three defects behind a first failing target and CI found them a
//! round later. The MECHANISM went into `scripts/verify.sh`, which is where a
//! round's own verification command goes, and that left the law reaching exactly
//! as far as somebody remembering to type the wrapper. Every `cargo test` in the
//! workflows and every separate workspace's suite — the largest commands in the
//! tree, and the ones whose failures reach `main` — were outside it.
//!
//! What blocked closing that is what this round removed: `ci_plan::parse_script`
//! counted a segment only when its FIRST word was `cargo`, so wrapping a command
//! DELETED it from every population built on that crate. It now reads a bare
//! `--` as a hand-over and keeps the carrier, which is the datum the law below
//! reads.
//!
//! Two halves, and the second is what stops the first from being a spelling
//! check: every `cargo test` this repository issues names the wrapper, AND the
//! wrapper it names is the one that runs the coverage gate.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use ci_plan::{
    commands_this_repository_issues, script_cargo_commands, tracked_manifests, CargoCommand,
    ManifestTarget, BUILD_DECLARATION,
};

/// The wrapper, as this repository's own path names it.
const WRAPPER: &str = "scripts/verify.sh";

/// The gate it runs, as a manifest this repository tracks.
const COVERAGE_GATE: &str = "tools/unreported-targets/Cargo.toml";

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> sits two levels under the root")
        .to_path_buf()
}

/// Every cargo command this repository issues, from all four places one can be
/// written — and what the machine asking could not reach.
///
/// ASKED OF `ci-plan`, NOT ASSEMBLED AGAIN (R1228). This function had spelled
/// the four-source walk out a second time, including the subtlety that
/// `scripts/check-side-workspaces.sh` is read from the LISTER'S OUTPUT rather
/// than from its text. `commands_this_repository_issues` is that same walk, and
/// R1212's own note on it says why one copy is the limit: "a second assembly is
/// a second answer". Two of them had already drifted apart in what they SAID —
/// neither said anything, but only one of them could have.
///
/// And the population is machine-conditional, so the part this machine could
/// not reach is printed. On a hosted runner `studio` is not there and its
/// commands are not in the set below; a reader of one run has no other way to
/// tell which machine produced it.
fn everything_this_repository_issues(root: &Path) -> Vec<CargoCommand> {
    let issued = commands_this_repository_issues(root);
    for skipped in &issued.skipped {
        println!("[judged-test-runs] {}", skipped.was_not("judged"));
    }
    issued.commands
}

/// Does something hand this command over, and is the wrapper among them?
///
/// A path rather than a word, because the callers spell it three ways: a
/// workflow writes `./scripts/verify.sh`, the workspace lister expands an
/// absolute path resolved from its own checkout, and a hook would write it
/// relative to the repository root. All three name one file.
///
/// ANYWHERE IN THE CARRIER, NOT ONLY IN FRONT (R1229). This asked for the FIRST
/// carrier and that was a claim about how many programs may stand in front of a
/// suite, which is not what the law is about: what it wants is that the wrapper
/// judges what the run covered, and a program in front of the wrapper does not
/// take that away. The case is real rather than hypothetical — the census this
/// round added runs the workspace suite under `strace`, so the carrier is
/// `strace … ./scripts/verify.sh …` and the wrapper is second. Reading only the
/// first word would have called this repository's largest suite unjudged while
/// `verify.sh` was carrying it exactly as before.
fn judged_for_coverage(command: &CargoCommand) -> bool {
    command
        .carrier
        .iter()
        .any(|program| program == WRAPPER || program.ends_with(&format!("/{WRAPPER}")))
}

#[test]
fn every_test_run_this_repository_issues_goes_through_the_wrapper_that_judges_it() {
    let root = repository_root();
    let commands = everything_this_repository_issues(&root);
    let suites: Vec<&CargoCommand> = commands
        .iter()
        .filter(|command| command.subcommand() == Some("test"))
        .collect();

    let unwrapped: Vec<String> = suites
        .iter()
        .filter(|command| !judged_for_coverage(command))
        .map(|command| format!("{} — {}", command.origin(), command.rendered()))
        .collect();
    assert!(
        unwrapped.is_empty(),
        "`cargo test` stops at the first target it cannot get past, so a run's \
         verdict is one target's number and the targets behind it were never \
         asked — `{WRAPPER}` is what holds that log against what cargo says the \
         command compiles, and a command it does not carry is one nothing \
         judges:\n  {}",
        unwrapped.join("\n  ")
    );

    // NON-VACUITY, PER SOURCE. This population is three walks bolted together
    // and the law reads as satisfied by an empty one — which is exactly what a
    // reader that stopped seeing wrapped commands would produce, and the defect
    // this round's `parse_script` change exists to prevent.
    let mut per_source: BTreeSet<&str> = BTreeSet::new();
    for command in &suites {
        per_source.insert(command.source.as_str());
    }
    assert!(
        per_source.contains("scripts/check-side-workspaces.sh"),
        "every separate in-repo workspace's suite is a `cargo test` this \
         repository issues, and a run that saw none of them read nothing: \
         {per_source:?}"
    );
    assert!(
        per_source
            .iter()
            .any(|source| source.starts_with(".github/workflows/")),
        "so is every suite in the workflows: {per_source:?}"
    );
    assert!(
        per_source.contains(BUILD_DECLARATION),
        "and so is the suite a build machine runs — the words are written in a \
         tracked file of this repository, so the law reaches them: {per_source:?}"
    );
    assert!(
        suites.len() >= 20,
        "this repository issues a suite per separate workspace plus the \
         workflows', and {} of them is a walk that stopped — in the direction \
         that reads as compliance",
        suites.len()
    );

    // THE MIRROR: the law is about TEST runs and not about every cargo command,
    // so a repository that wrapped everything would pass it for the wrong
    // reason. `cargo fmt`, `cargo clippy` and `cargo run` compile no test target
    // and have no coverage to judge; the gate calls them vacuous.
    let unwrapped_others = commands
        .iter()
        .filter(|command| command.subcommand() != Some("test"))
        .filter(|command| !judged_for_coverage(command))
        .count();
    assert!(
        unwrapped_others >= 5,
        "a command that runs no test target has no coverage for the wrapper to \
         judge, so this law must not be reading `everything is wrapped`: only \
         {unwrapped_others} of the non-test commands are issued directly"
    );
}

/// A PROGRAM IN FRONT OF THE WRAPPER DOES NOT TAKE THE JUDGING AWAY, and a
/// command with no wrapper at all is still unjudged (R1229).
///
/// The census this round added runs the workspace suite under `strace`, so the
/// carrier became `strace … ./scripts/verify.sh …` and the wrapper stopped
/// being the first word. Both directions are asserted here because loosening a
/// predicate is exactly where a law quietly starts accepting everything: the
/// first case must pass and the second must NOT.
#[test]
fn the_wrapper_is_found_wherever_it_carries_and_a_command_it_does_not_carry_is_not() {
    let carried = CargoCommand {
        source: "a case".to_string(),
        owner: "a case".to_string(),
        carrier: vec![
            "strace".to_string(),
            "-f".to_string(),
            "./scripts/verify.sh".to_string(),
            "--no-fresh".to_string(),
        ],
        cargo_args: ["cargo", "test", "--workspace"]
            .iter()
            .map(|word| (*word).to_string())
            .collect(),
        harness_args: Vec::new(),
        env: Default::default(),
    };
    assert!(
        judged_for_coverage(&carried),
        "the wrapper carries this suite; something standing in front of it does \
         not stop it judging what the run covered"
    );

    let observer_only = CargoCommand {
        carrier: vec!["strace".to_string(), "-f".to_string()],
        ..carried.clone()
    };
    assert!(
        !judged_for_coverage(&observer_only),
        "and a suite carried by something that is NOT the wrapper is still \
         unjudged — a law that answered yes here would accept every command in \
         this repository"
    );
}

/// A VERDICT WRITTEN TO A FILE IS ONE A STEP EXITS WITH (R1229).
///
/// `strace` returns the status of the command it WRAPPED and says nothing at
/// all about the program on the other end of its `-o "|…"` pipe. So the census
/// this repository runs over its own suite cannot fail the step it rides on:
/// it writes its verdict to a file, and a LATER step is what turns that into
/// the job's answer. That indirection is the whole of the gate's teeth, and it
/// is one word long — a step that reads the file and then `exit 0` looks
/// exactly like one that honours it, in a diff and on a green run alike.
///
/// This repository has paid for reading a wrapper's status instead of the one
/// that matters. The law is therefore not "a census runs" but "what it decided
/// is what the job returns".
#[test]
fn a_census_verdict_written_to_a_file_is_one_a_step_exits_with() {
    let root = repository_root();
    let mut checked = 0;
    for path in ci_plan::workflow_files(&root) {
        let doc = ci_plan::load_workflow(&root, &path);
        let steps = ci_plan::run_steps(&doc);
        // The steps that WRITE a verdict, found by the shape rather than by a
        // name: `echo $? > <something>.rc` is how a status survives a program
        // that will not propagate it.
        // KEYED ON THE FILE'S OWN NAME, not on the directory holding it. Where
        // a workflow puts its scratch is the author's to change — `$RUNNER_TEMP`,
        // `${TMPDIR:-/tmp}`, the checkout root — and a law matching the whole
        // path would go quiet the first time one of them did, which is the
        // silence it exists to prevent.
        let written: Vec<String> = steps
            .iter()
            .flat_map(|step| step.script.split_whitespace().collect::<Vec<_>>())
            .map(|word| word.trim_matches(|c| c == '"' || c == '\\').to_string())
            .filter(|word| word.ends_with(".rc"))
            .filter_map(|word| word.rsplit('/').next().map(str::to_string))
            .collect();
        for verdict in &written {
            checked += 1;
            // SOME step in the same workflow must exit with what that file
            // holds, and there are two ways to do that. Both are read off the
            // step's LAST command, because a step's status is its last command's
            // status and nothing before it decides anything.
            //
            // R1230 WIDENED THIS, and the reason is the reason the law exists.
            // The shell form — `verdict=$(cat …); exit "$verdict"` — could not
            // tell a census that answered `no` from one that was never allowed
            // to answer: the first hosted run carrying the census was cancelled
            // at its budget, `cat` on a missing file left an empty string, and
            // `exit ""` failed with a message about `exit`. So the reading moved
            // into the program that wrote the file, where a missing verdict is
            // the third answer and has a test that RUNS it. A law that accepted
            // only the shell spelling would have rejected the repair.
            //
            // WHAT IS NOT ACCEPTED IS A SWALLOW. `|| true` after either form
            // makes the step green whatever the file holds, and that is the one
            // word this whole law exists to keep out.
            //
            // AND CONSUMING IS NOT WRITING — measured, by injection. The first
            // version of this widening accepted any last command that NAMED the
            // file, and the step that names it first is the one that WRITES it:
            // `run: >` folds a step into a single line, so the census step's
            // whole script is its own last command and it holds
            // `echo $? > "…/outside-reach.rc"`. The swallow injection stayed
            // green against that reading. A word is an input here only when
            // nothing redirects into it.
            let names_as_input = |line: &str| {
                let words: Vec<&str> = line.split_whitespace().collect();
                words.iter().enumerate().any(|(index, word)| {
                    word.contains(verdict.as_str())
                        && !word.starts_with('>')
                        && index
                            .checked_sub(1)
                            .and_then(|before| words.get(before))
                            .is_none_or(|before| !before.ends_with('>'))
                })
            };
            let honoured = steps.iter().any(|step| {
                let last = step
                    .script
                    .lines()
                    .rev()
                    .find(|line| !line.trim().is_empty())
                    .unwrap_or_default()
                    .trim();
                if last.ends_with("|| true") || last.ends_with("; true") {
                    return false;
                }
                // The program form: the last command is HANDED the file, so its
                // status is the step's.
                names_as_input(last)
                    // The shell form: the last command exits with a variable,
                    // and the script is the one that read the file into it.
                    || (last.starts_with("exit \"$") && step.script.contains(verdict.as_str()))
            });
            assert!(
                honoured,
                "{path} writes a verdict to `{verdict}` and no step exits with \
                 it, so whatever wrote it cannot fail this job. A census whose \
                 answer nothing reads is the shape this repository keeps \
                 deleting — and it is one word away from the shape that works"
            );
        }
    }
    // NON-VACUITY. This law is a walk over a shape, and a walk that found no
    // verdict file reports no violations — which is exactly what it would do
    // the day the census step was deleted.
    assert!(
        checked >= 1,
        "no workflow in this repository writes a verdict to a `.rc` file, so \
         this law asserted nothing. If the census was removed, this law goes \
         with it rather than passing over an empty population"
    );
}

#[test]
fn the_wrapper_that_law_names_is_the_one_that_runs_the_coverage_gate() {
    // THE HALF THAT TIES THE NAME TO THE MECHANISM. Above is a check on a path
    // spelled in five files; on its own it would go on passing after
    // `scripts/verify.sh` stopped judging anything, which is the shape where a
    // gate keeps its shape and loses its content.
    let root = repository_root();
    let tracked = tracked_manifests(&root);
    let gate_calls: Vec<CargoCommand> = script_cargo_commands(&root)
        .into_iter()
        .filter(|command| command.source == WRAPPER)
        .filter(|command| {
            command.manifest(&tracked) == ManifestTarget::Named(COVERAGE_GATE.to_string())
        })
        .collect();
    assert_eq!(
        gate_calls.len(),
        1,
        "`{WRAPPER}` is named by every test run in this repository BECAUSE it \
         asks `{COVERAGE_GATE}` what the run covered; a copy of it that no \
         longer does is a wrapper the law above cannot tell from this one"
    );
    assert!(
        gate_calls[0].harness_has("--log"),
        "and it hands that gate the run's own log, which is the evidence side of \
         the comparison: {:?}",
        gate_calls[0].harness_args
    );
}
