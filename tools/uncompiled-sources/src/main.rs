//! `uncompiled-sources` — every Rust file this repository tracks is one cargo
//! compiles.
//!
//! ```text
//! cargo run -q --manifest-path tools/uncompiled-sources/Cargo.toml \
//!   --bin uncompiled-sources
//! ```
//!
//! The tree it judges is the WORKING DIRECTORY, the same rule
//! `scripts/check-side-workspaces.sh` states: a git hook runs with the
//! repository root as its working directory and may be this repository's gate
//! running over another tree.
//!
//! Exit codes are three answers, not two: `0` the law holds, `1` Rust exists
//! that nothing compiles, `2` the gate could not judge. One code for the last
//! two mislabels whichever it did not mean, which is the failure R1078 shipped.

use std::process::ExitCode;

fn main() -> ExitCode {
    let root = match std::env::current_dir() {
        Ok(root) => root,
        Err(e) => {
            eprintln!("[uncompiled-sources] cannot read the working directory: {e}");
            return ExitCode::from(2);
        }
    };
    if !root.join("Cargo.toml").is_file() {
        eprintln!(
            "[uncompiled-sources] {} has no Cargo.toml — run this from the root \
             of the tree to check, which is where a git hook and CI both start",
            root.display()
        );
        return ExitCode::from(2);
    }

    let report = uncompiled_sources::run(&root);

    for probe in &report.probes {
        println!(
            "[uncompiled-sources] `{}` — {} unit(s), {} dep-info record(s), {} \
             file(s) of this repository read",
            probe.label,
            probe.artifacts,
            probe.dep_info,
            probe.read.len()
        );
    }
    for skipped in &report.skipped_workspaces {
        println!(
            "[uncompiled-sources] {} — {} tracked file(s) under it are outside \
             this machine's answer",
            skipped.was_not("probed"),
            report
                .in_skipped_workspace
                .iter()
                .filter(|file| file.starts_with(&skipped.directory))
                .count()
        );
    }
    // Printed even when empty, because an exception nobody sees is one nobody
    // reviews. This is the line that has to stay one word long.
    println!(
        "[uncompiled-sources] declared as deliberately uncompiled: {}",
        match report.declared.len() {
            0 => "none".to_string(),
            _ => report
                .declared
                .iter()
                .map(|(file, reason)| format!("{} ({reason})", file.display()))
                .collect::<Vec<_>>()
                .join(", "),
        }
    );
    println!(
        "[uncompiled-sources] reach: {} tracked Rust file(s), {} of them \
         compiled by {} probe(s) over {} workspace(s)",
        report.tracked.len(),
        report.compiled(),
        report.probes.len(),
        report.probes.len().div_ceil(2),
    );

    if let Err(refusals) = report.verdict() {
        for refusal in refusals {
            eprintln!("[uncompiled-sources] REFUSED: {refusal}");
        }
        eprintln!(
            "[uncompiled-sources] this gate did not judge. A run that reports no \
             uncompiled source because it could not compile is the failure it \
             exists to prevent."
        );
        return ExitCode::from(2);
    }

    if report.findings.is_empty() {
        println!(
            "[uncompiled-sources] every Rust file this repository tracks is one cargo compiles"
        );
        return ExitCode::SUCCESS;
    }

    let dark_tests: usize = report
        .findings
        .iter()
        .map(|finding| finding.tests.len())
        .sum();
    eprintln!(
        "[uncompiled-sources] {} tracked Rust file(s) that no compilation reads, \
         holding {dark_tests} test(s) no gate can see:",
        report.findings.len()
    );
    for finding in &report.findings {
        eprintln!("    {finding}");
        for test in &finding.tests {
            eprintln!("        {test}");
        }
    }
    eprintln!(
        "[uncompiled-sources] source no compiler reads has no opinion held about \
         it — not the type checker's, not clippy's, and not R1084's gate, which \
         can only see tests a build produced. Fix: give it a target (a `mod`, a \
         `tests/` entry point, an `[[example]]`), or delete it."
    );
    ExitCode::from(1)
}
