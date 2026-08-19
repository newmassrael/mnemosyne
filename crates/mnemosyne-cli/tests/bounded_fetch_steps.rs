//! A step that waits on somebody else's server cannot be allowed to spend the
//! whole job.
//!
//! R1237, and it is R1236's carry paid off. On 2026-08-18 two jobs of one run
//! stalled in the SAME `apt-get` step: `server-features` for 45m09s until its
//! own 45-minute budget killed it, and `validate` for exactly ninety minutes
//! until its own. Between them the run spent over two hours on an archive that
//! never answered, and NOT ONE TEST of this repository ran in either — every
//! step behind the stall was skipped. That same step takes nine to eighteen
//! seconds when the world is answering.
//!
//! R1236 BOUNDED THE SIX `apt-get` STEPS AND THAT WAS A HABIT, which its own
//! carry said in as many words: nothing asked whether the seventh was bounded.
//! The seventh already existed. This law found TEN more the first time it ran —
//! every `rustup show` in the tree, one per job, and the `rustup toolchain
//! install` in `msrv` — because `rustup show` with a `rust-toolchain.toml`
//! DOWNLOADS AND INSTALLS the toolchain when it is not already there. Six of
//! sixteen was what a person's diligence had produced.
//!
//! # The population is asked for, not spelled again
//!
//! THE OBVIOUS WAY TO WRITE THIS IS A WORD LIST — `apt-get`, `rustup`, and the
//! next few somebody might use — and that way is the shape N120 is open about.
//! A word that names nothing in this repository is a hole that stays quiet, and
//! a list that SHRINKS takes the law's population with it while staying green:
//! delete `rustup` from it and fourteen of the sixteen steps below stop being
//! looked at, with nothing red anywhere. Measured, because the first draft of
//! this file did exactly that.
//!
//! `ci_plan` owns that vocabulary — `READ_IN_FULL` is the apt family,
//! `RECOGNISED_NOT_READ` is sixteen more managers, and `read_command` is the
//! reading `tools/undeclared-requirement` has always judged its own law with.
//! R1237 moved it there from that crate so both laws could share one: which
//! manager a step invokes is part of "the one answer to what this repository's
//! CI runs", which is that crate's whole subject.
//!
//! # …and that reading is a LOWER BOUND, which is measured and not assumed
//!
//! `read_command` answers "does this command install a named package", and this
//! law asks something wider: "does this step wait on somebody else's server".
//! The two differ by nine steps in this repository, all of them `rustup show`,
//! and they are nine of the ten this law found unbounded. `rustup show` reads
//! `rust-toolchain.toml` and DOWNLOADS the toolchain it names when the runner
//! does not have it — an install by every property that matters here, and
//! correctly not one by that reading's question, which is about package names it
//! could hold against a declaration.
//!
//! So the population is the union, and the containment is ASSERTED: every step
//! that reading calls an install must be in this law's population. A vocabulary
//! that shrinks past the shared reading is then red rather than quiet, which is
//! the half of N120 that can be closed from here. What remains open is stated in
//! the round's carry — the `rustup` word covers steps the shared reading does
//! not, so deleting it AND its own non-emptiness assertion is two deliberate
//! lines rather than a silent drift.
//!
//! AND AN INSTALLING ACTION IS REFUSED RATHER THAN MISSED, by the same shared
//! vocabulary: `action_installs` reads an action's NAME, which is all there is
//! of somebody else's program, and a `uses:` step whose name says it installs is
//! a refusal here — a requirement moved out of shell cannot slip past by being
//! unreadable.
//!
//! # Two halves
//!
//!   1. Every step that installs from a network service declares a bound.
//!   2. That bound is SMALLER THAN ITS JOB'S. A step bounded at exactly its
//!      job's budget is bounded and prevents nothing — the same forty-five
//!      minutes are spent, with a field filled in.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ci_plan::{
    action_installs, load_workflow, read_command, run_steps, shell_commands, uses_steps,
    workflow_files, InstallCommand, RunStep,
};

/// Where a step is, in the words its own file uses.
type Where = (String, String, usize);

fn place(at: &Where) -> String {
    format!("{} job `{}` step {}", at.0, at.1, at.2)
}

/// How this repository bounds a step that fetches, in minutes.
///
/// MEASURED, on the last fully green run before this law existed (`7bbaba1`,
/// nine jobs, every one success): the `apt-get` steps took 9 to 18 seconds and
/// the `rustup` steps 8 to 10. Five minutes is roughly seventeen times the
/// slowest of them, and it is the number R1236 already put on the six `apt-get`
/// steps — one number for one failure, rather than a table of budgets nobody
/// can defend one line at a time.
const BOUND: u64 = 5;

/// What GitHub applies to a job that declares no budget of its own.
const GITHUB_DEFAULT: &str = "360";

/// The one fetch this repository issues that installs no NAMED package.
///
/// ONE WORD, AND IT IS HERE FOR A FACT ABOUT THE PROGRAM RATHER THAN A GUESS.
/// `rustup show` prints what is installed — and, given a `rust-toolchain.toml`,
/// INSTALLS the toolchain it names first when the runner has not got it. Every
/// job of this repository runs it, ten steps in all, and every one of them was
/// unbounded until this law existed. `undeclared_requirement` does not name them
/// and should not: its question is which package names to hold against the
/// build-machine declaration, and `show` names none.
const ALSO_FETCHES: &str = "rustup";

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> sits two levels under the root")
        .to_path_buf()
}

/// How a step reads when this law has something to say about it.
fn named(at: &Where, step: &RunStep) -> String {
    let head = step.script.lines().next().unwrap_or_default().trim();
    format!("{} — `{head}`", place(at))
}

/// Does the shared reading call any command of this step an install?
///
/// `Refused` COUNTS. That variant means install-shaped and unreadable — a
/// command whose packages cannot be held against a declaration — and for the
/// question here, "does this wait on somebody else's server", unreadable is not
/// a reason to look away.
fn the_shared_reading_calls_it_an_install(step: &RunStep) -> bool {
    shell_commands(&step.script).iter().any(|words| {
        matches!(
            read_command(words),
            InstallCommand::Read { .. }
                | InstallCommand::Recognised { .. }
                | InstallCommand::Refused { .. }
        )
    })
}

#[test]
fn every_step_that_installs_over_the_network_is_bounded_and_by_less_than_its_job() {
    let root = repository_root();
    let files = workflow_files(&root);
    assert!(
        !files.is_empty(),
        "this law reads the workflows and found no file at all — a walk that \
         failed, not a repository with no CI"
    );

    let mut steps: BTreeMap<Where, RunStep> = BTreeMap::new();
    let mut shared: Vec<Where> = Vec::new();
    let mut unreadable_actions: Vec<String> = Vec::new();
    for file in &files {
        let doc = load_workflow(&root, file);
        for step in run_steps(&doc) {
            let at: Where = (file.clone(), step.job.clone(), step.index);
            if the_shared_reading_calls_it_an_install(&step) {
                shared.push(at.clone());
            }
            steps.insert(at, step);
        }
        // AND AN ACTION THAT SAYS IT INSTALLS IS A REFUSAL, on the same shared
        // vocabulary. `uses:` steps take a `timeout-minutes` too, but what an
        // action DOES is somebody else's bytes: this law cannot say a fetch
        // inside one is bounded, so it says it cannot rather than passing.
        for used in uses_steps(&doc, file) {
            if action_installs(used.action()) {
                unreadable_actions.push(format!(
                    "{file} job `{}` step {} — `{}` installs onto the runner, and \
                     what it waits on is not in this repository's bytes",
                    used.job, used.index, used.uses
                ));
            }
        }
    }
    assert!(
        unreadable_actions.is_empty(),
        "these step(s) install through an action, so this law cannot say whether \
         what they wait on is bounded:\n  {}",
        unreadable_actions.join("\n  ")
    );
    assert!(
        !shared.is_empty(),
        "the shared reading found no install anywhere in this repository's \
         workflows, so half this law's population is empty — a reading that \
         failed, not a CI that installs nothing (six `apt-get` steps and one \
         `rustup toolchain install` were there when this was written)"
    );

    // THIS LAW'S OWN POPULATION: what the shared reading named, plus every step
    // that invokes `rustup` at all.
    let mut sites: Vec<(Where, &'static str)> = Vec::new();
    let mut also = 0usize;
    for (at, step) in &steps {
        let named_by_shared = shared.contains(at);
        let fetches = step.script.contains(ALSO_FETCHES);
        if !named_by_shared && !fetches {
            continue;
        }
        if fetches && !named_by_shared {
            also += 1;
        }
        sites.push((
            at.clone(),
            if named_by_shared { "shared" } else { "rustup" },
        ));
    }

    // THE CONTAINMENT, ASSERTED. Every step the shared reading calls an install
    // is one this law looks at; if this population ever shrinks past that
    // reading, it is red here rather than quiet. That is the half of N120 that
    // can be closed from inside a law built on a word.
    let missed: Vec<String> = shared
        .iter()
        .filter(|at| !sites.iter().any(|(mine, _)| mine == *at))
        .map(place)
        .collect();
    assert!(
        missed.is_empty(),
        "the shared installer reading names step(s) this law's own population \
         does not contain, so this law is now looking at less than that reading \
         does:\n  {}",
        missed.join("\n  ")
    );
    assert!(
        also > 0,
        "no step of this repository invokes `{ALSO_FETCHES}` outside what the \
         shared reading already names — which was nine steps when this law was \
         written, one per job. Either toolchains arrive some other way now (and \
         this law must be re-read against that way), or this word stopped \
         matching"
    );

    let mut unbounded = Vec::new();
    let mut not_smaller = Vec::new();
    let mut unreadable = Vec::new();
    let mut counted: BTreeMap<&str, usize> = BTreeMap::new();

    for (at, how) in &sites {
        *counted.entry(how).or_default() += 1;
        let step = steps.get(at).expect("a step this law's own walk produced");
        let Some(bound) = step.timeout.as_deref() else {
            unbounded.push(named(at, step));
            continue;
        };
        // THE SECOND HALF. A bound is only a bound if it is smaller than what it
        // protects; the job's budget is what a stalled step would otherwise
        // spend. Either number may be an expression this law cannot evaluate,
        // and that is REPORTED rather than passed — "I could not tell" and "it
        // is fine" are the two answers this repository keeps confusing.
        let job = step.job_timeout.as_deref().unwrap_or(GITHUB_DEFAULT);
        match (bound.parse::<u64>(), job.parse::<u64>()) {
            (Ok(step_minutes), Ok(job_minutes)) if step_minutes < job_minutes => {}
            (Ok(step_minutes), Ok(job_minutes)) => not_smaller.push(format!(
                "{} — bounded at {step_minutes}m inside a job budgeted {job_minutes}m",
                named(at, step)
            )),
            _ => unreadable.push(format!(
                "{} — step bound `{bound}`, job budget `{job}`",
                named(at, step)
            )),
        }
    }

    // PRINTED EVERY RUN, because a population is the one thing a green law
    // never says out loud. If this number falls, somebody moved an install
    // somewhere this reading does not reach.
    println!(
        "[bounded-fetch] {} step(s) wait on a service outside this run: {} named by \
         the shared installer reading, {} more that invoke `{ALSO_FETCHES}` — every \
         one bounded at under its job's budget",
        sites.len(),
        counted.get("shared").copied().unwrap_or_default(),
        counted.get("rustup").copied().unwrap_or_default(),
    );

    assert!(
        unbounded.is_empty(),
        "these step(s) wait on a service outside this run and declare no bound, \
         so a service that stops answering spends their job's whole budget — \
         which is what two jobs of one run did on 2026-08-18, for over two hours \
         between them, with no test of this repository run in either. Add \
         `timeout-minutes: {BOUND}`:\n  {}",
        unbounded.join("\n  ")
    );
    assert!(
        not_smaller.is_empty(),
        "these step(s) are bounded by their job's own budget or more, which \
         cannot prevent anything — the same minutes are spent and a field is \
         filled in:\n  {}",
        not_smaller.join("\n  ")
    );
    assert!(
        unreadable.is_empty(),
        "these step(s) carry a bound this law cannot evaluate, and that is not \
         the same as a bound it approved:\n  {}",
        unreadable.join("\n  ")
    );
}

/// Round 1251 — this repository's apt policy, and the one place its numbers live.
const APT_POLICY: &str = ".github/apt-retries.conf";

/// Where apt reads dropped-in configuration.
const APT_POLICY_DIR: &str = "/etc/apt/apt.conf.d/";

/// R1251 — EVERY APT INSTALL RUNS UNDER THAT POLICY, AND THE POLICY GOES FIRST.
///
/// R1236 bounded these steps at five minutes after two of them stalled for 45
/// and 90; R1237 widened the bound to every fetching step. Then on 2026-08-19
/// the bound FIRED, on two runs two and a half hours apart: the step timed out
/// at five minutes and the jobs behind it died with `exit code 127` looking for
/// a protoc nothing had installed. A bound turns a hang into a red build sooner
/// and cannot turn it into a green one — what was missing is that apt was asked
/// to try ONCE, because `Acquire::Retries` defaults to 0.
///
/// So the population is every step the shared reading calls an APT install, and
/// each must copy the tracked policy in BEFORE the first apt command of the same
/// step. Order is the whole point: a policy installed afterwards configures the
/// next run of a job that has already failed.
#[test]
fn every_apt_install_runs_under_this_repositorys_apt_policy_first() {
    let root = repository_root();
    assert!(
        root.join(APT_POLICY).is_file(),
        "{APT_POLICY} is what every apt step copies in, and it is not in the tree"
    );

    let mut checked = 0usize;
    let mut findings: Vec<String> = Vec::new();
    for file in workflow_files(&root) {
        let doc = load_workflow(&root, &file);
        for step in run_steps(&doc) {
            let commands = shell_commands(&step.script);
            // The apt family and not every install: `rustup` reads no apt
            // configuration, and a law that demanded this of it would be asking
            // for a line that does nothing.
            let Some(first_apt) = commands.iter().position(|words| {
                matches!(read_command(words), InstallCommand::Read { ref manager, .. }
                    if ci_plan::READ_IN_FULL.contains(&manager.as_str()))
            }) else {
                continue;
            };
            checked += 1;
            let at: Where = (file.clone(), step.job.clone(), step.index);
            let policy = commands.iter().position(|words| {
                words.iter().any(|w| w == APT_POLICY)
                    && words.iter().any(|w| w.starts_with(APT_POLICY_DIR))
            });
            match policy {
                None => findings.push(format!(
                    "{} — installs with apt and never copies {APT_POLICY} into \
                     {APT_POLICY_DIR}, so apt runs with `Acquire::Retries` 0",
                    named(&at, &step)
                )),
                Some(i) if i > first_apt => findings.push(format!(
                    "{} — copies {APT_POLICY} in AFTER its first apt command, \
                     which configures the next run rather than this one",
                    named(&at, &step)
                )),
                Some(_) => {}
            }
        }
    }
    assert!(
        findings.is_empty(),
        "these apt step(s) are asked to try once:\n  {}",
        findings.join("\n  ")
    );
    assert!(
        checked > 0,
        "no apt install was found anywhere in this repository's workflows, so \
         this law judged nothing — a reading that failed, not a CI that installs \
         nothing (six were here when this was written)"
    );
    println!("{checked} apt step(s) run under {APT_POLICY}");
}

/// R1251 — AND THE POLICY IS APT CONFIGURATION THAT SAYS WHAT IT CLAIMS, asked
/// of apt's own parser rather than of a regular expression.
///
/// A file that fails to parse is silently ignored by apt: the copy succeeds, the
/// step runs, and every fetch is back to one attempt with nothing to say so. The
/// two settings are read back out of the dump because a file can parse and
/// declare neither — measured, on a draft that declared one.
#[test]
fn the_apt_policy_parses_as_apt_configuration_and_declares_both_settings() {
    let root = repository_root();
    let path = root.join(APT_POLICY);
    let out = match std::process::Command::new("apt-config")
        .args(["-c", path.to_str().expect("utf-8 path"), "dump"])
        .output()
    {
        Ok(out) => out,
        Err(_) => {
            // NOT A PASS. This machine has no apt, so the claim is unjudged, and
            // saying so is the difference between a clean answer and an answer
            // about nothing. Every runner this repository's CI uses has it.
            println!(
                "NO VERDICT — `apt-config` is not on this machine, so {APT_POLICY} was not parsed"
            );
            return;
        }
    };
    assert!(
        out.status.success(),
        "apt's own parser rejects {APT_POLICY}, which apt would then IGNORE in \
         silence:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let dump = String::from_utf8_lossy(&out.stdout);
    for setting in ["Acquire::Retries \"3\";", "Acquire::http::Timeout \"20\";"] {
        assert!(
            dump.contains(setting),
            "{APT_POLICY} parses and does not declare `{setting}` — the numbers \
             are what the file is for"
        );
    }
}
