//! `unrun-tests` — every test this repository compiles is one some CI command
//! runs.
//!
//! ```text
//! cargo run -q --manifest-path tools/unrun-tests/Cargo.toml --bin unrun-tests
//! ```
//!
//! The tree it judges is the WORKING DIRECTORY, the same rule
//! `scripts/check-side-workspaces.sh` states: a git hook runs with the
//! repository root as its working directory and may be this repository's gate
//! running over another tree.
//!
//! Exit codes are three answers, not two: `0` the law holds, `1` tests are dark,
//! `2` the gate could not judge. One code for the last two mislabels whichever
//! it did not mean, which is the failure R1078 shipped.

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let root = match std::env::current_dir() {
        Ok(root) => root,
        Err(e) => {
            eprintln!("[unrun-tests] cannot read the working directory: {e}");
            return ExitCode::from(2);
        }
    };
    if !root.join("Cargo.toml").is_file() {
        eprintln!(
            "[unrun-tests] {} has no Cargo.toml — run this from the root of the \
             tree to check, which is where a git hook and CI both start",
            root.display()
        );
        return ExitCode::from(2);
    }

    let report = unrun_tests::run(&root);

    for probe in &report.population_probes {
        println!(
            "[unrun-tests] population {} — {} binaries, {} tests",
            probe.origin,
            probe.binaries.len(),
            probe.listed.len()
        );
    }
    for reason in &report.skipped_workspaces {
        println!("[unrun-tests] not probed (the lister says why): {reason}");
    }
    for probe in &report.ci_probes {
        println!(
            "[unrun-tests] CI {} — `{}` runs {} of the {} it can see",
            probe.origin,
            probe.label,
            probe.executed.len(),
            probe.listed.len()
        );
    }
    println!(
        "[unrun-tests] reach: {} test(s) compiled across {} workspace(s), {} run \
         by {} CI test command(s); {} other cargo command(s) in CI say nothing \
         about tests",
        report.population.len(),
        report.population_probes.len(),
        report.run.len(),
        report.ci_probes.len(),
        report.non_test_commands,
    );

    if let Err(refusals) = report.verdict() {
        for refusal in refusals {
            eprintln!("[unrun-tests] REFUSED: {refusal}");
        }
        eprintln!(
            "[unrun-tests] this gate did not judge. A run that reports no dark \
             tests because it could not look is the failure it exists to \
             prevent."
        );
        return ExitCode::from(2);
    }

    let dark = report.dark();
    if dark.is_empty() {
        println!("[unrun-tests] every test this repository compiles is run by CI");
        return ExitCode::SUCCESS;
    }

    eprintln!(
        "[unrun-tests] {} test(s) exist and no CI command runs them:",
        dark.len()
    );
    let mut by_target: Vec<(PathBuf, Vec<&str>)> = Vec::new();
    for test in &dark {
        match by_target
            .iter_mut()
            .find(|(target, _)| *target == test.target_src)
        {
            Some((_, names)) => names.push(&test.name),
            None => by_target.push((test.target_src.clone(), vec![&test.name])),
        }
    }
    for (target, names) in &by_target {
        eprintln!("    {} — {} test(s):", target.display(), names.len());
        for name in names {
            eprintln!("        {name}");
        }
    }
    eprintln!(
        "[unrun-tests] a test nothing runs is not coverage. Fix: run it from a \
         job (a feature it hides behind needs `--all-features`; an `#[ignore]` \
         needs a command that passes `--ignored`), or delete it."
    );
    ExitCode::from(1)
}
