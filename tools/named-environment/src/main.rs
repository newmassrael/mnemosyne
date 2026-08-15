//! `named-environment --workspace <manifest>` — run the law over one workspace
//! and say what it reached.
//!
//! Exit codes are three rather than two on purpose:
//!
//! | code | meaning |
//! |---|---|
//! | 0 | judged, and the workspace obeys the law |
//! | 1 | judged, and these tests do not name what they spawn |
//! | 2 | NOT judged — the gate could not read enough of the tree to have an opinion |
//!
//! The third is the one this family of gates keeps paying for: a run that never
//! happened reports zero findings, and zero findings is what a clean tree looks
//! like.

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut manifest: Option<PathBuf> = None;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--workspace" => match arguments.next() {
                Some(path) => manifest = Some(PathBuf::from(path)),
                None => {
                    eprintln!("named-environment: --workspace needs a path to a Cargo.toml");
                    return ExitCode::from(2);
                }
            },
            "--help" | "-h" => {
                println!("usage: named-environment --workspace <path/to/Cargo.toml>");
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("named-environment: unknown argument {other}");
                return ExitCode::from(2);
            }
        }
    }
    let Some(manifest) = manifest else {
        eprintln!("usage: named-environment --workspace <path/to/Cargo.toml>");
        return ExitCode::from(2);
    };

    let report = match named_environment::run(&manifest) {
        Ok(report) => report,
        Err(message) => {
            eprintln!("named-environment: {message}");
            return ExitCode::from(2);
        }
    };

    let root = report.workspace_root.clone();
    let show = |path: &std::path::Path| -> String {
        path.strip_prefix(&root)
            .unwrap_or(path)
            .display()
            .to_string()
    };

    println!(
        "[named-environment] workspace {}",
        report.workspace_root.display()
    );
    println!(
        "[named-environment] compiled {} .rs — {} no target reaches, {} in a nested workspace \
         (checked by pointing this at ITS manifest), {} under target/",
        report.coverage.scanned.len(),
        report.coverage.unreached.len(),
        report.coverage.foreign_workspaces.len(),
        report.coverage.build_artifacts,
    );
    // BOTH BASES, which the module doc has promised since R1182 and which this
    // sentence printed only half of until R1211: the verdict rests on MENTION,
    // and a report that showed only the stronger number could not be read for
    // how far apart they are — which is the whole of the debt that closed here.
    println!(
        "[named-environment] {} binary(ies) reading {} variable(s); {} of {} test target(s) spawn \
         one, mentioning {} of them and SETTING {} (and {} clear the environment whole)",
        report.reach.binaries,
        report.reach.reads,
        report.reach.spawning_targets,
        report.reach.test_targets,
        report.reach.mentioned,
        report.reach.named,
        report.reach.clearing_targets,
    );
    // AND WHAT THE DISTANCE BETWEEN THEM IS MADE OF. A site the walk cannot
    // resolve contributed to neither number before R1211, so a fixture
    // controlling its whole environment through an unreadable shape looked
    // exactly like one controlling nothing.
    if report.unread_control.is_empty() {
        println!(
            "[named-environment] every .env / .env_remove here names a variable this walk resolved"
        );
    } else {
        println!(
            "[named-environment] {} .env / .env_remove site(s) name something this walk cannot \
             resolve, so what they set is NOT counted above:",
            report.unread_control.len()
        );
        for site in &report.unread_control {
            println!(
                "[named-environment]   `{}` {} at {}:{}",
                site.test_target,
                site.spelled,
                show(&site.file),
                site.line,
            );
        }
    }
    // THE POPULATION THE ATTRIBUTION IS A FRACTION OF (R1190). A gate that
    // prints only what it judged cannot be told from one that judged everything.
    println!(
        "[named-environment] {} Command::new site(s) in test targets — {} spelled \
         env!(\"CARGO_BIN_EXE_…\") and judged, {} pointed at something this walk cannot name",
        report.reach.spawn_sites,
        report.reach.sites_attributed,
        report.other_spawns.len(),
    );
    for spawn in &report.other_spawns {
        if spawn.instead_of.is_some() {
            continue;
        }
        println!(
            "[named-environment] NOT JUDGED `{}` spawns {} at {}:{}",
            spawn.test_target,
            spawn.spelled,
            show(&spawn.file),
            spawn.line,
        );
    }
    for (binary, variables) in &report.read_by {
        if variables.is_empty() {
            continue;
        }
        println!(
            "[named-environment] `{binary}` reads {}",
            variables
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    if let Err(refusal) = report.verdict() {
        eprintln!("[named-environment] NO VERDICT — {refusal}");
        return ExitCode::from(2);
    }

    // BEFORE THE "NOTHING TO APPLY TO" ARM, and that order is load-bearing: a
    // workspace whose ONLY spawns are unchecked ones reaches no law at all, so
    // an early return would print "nothing here" over the very defect that made
    // it true (R1190).
    let unchecked = report.spawns_by_a_name_the_machine_answers();
    for spawn in &unchecked {
        let Some(instead) = &spawn.instead_of else {
            continue;
        };
        println!(
            "[named-environment] DEFECT test `{}` spawns {} — at {}:{}",
            spawn.test_target,
            spawn.spelled,
            show(&spawn.file),
            spawn.line,
        );
        println!(
            "                    why: PATH decides which program that is, so a machine with a \
             different one runs a different test — and a failure there reads as a finding about \
             this repository"
        );
        println!("                    fix: name it — {instead}");
    }

    if !report.found_a_spawn() && unchecked.is_empty() {
        // A complete answer rather than a refusal: the law is about tests that
        // spawn a program, and there are none here. Said in words so it cannot
        // be read as "checked and clean".
        println!(
            "[named-environment] no test in this workspace spawns one of its binaries — the law \
             has nothing here to apply to, which is not the same as a clean check"
        );
        return ExitCode::SUCCESS;
    }

    if report.findings.is_empty() && unchecked.is_empty() {
        println!(
            "[named-environment] every variable the spawned programs read is named by the test \
             that spawns them"
        );
        // WHICH BASIS THAT VERDICT RESTS ON. Zero here means the law was
        // enforced as it is written — the test SETS or REMOVES it — everywhere
        // it applied. A number means some target holds a `.env` this walk could
        // not read, and was held to the weaker question instead.
        if report.reach.targets_on_mention == 0 {
            println!(
                "[named-environment] and every one of them at an .env / .env_remove call site, \
                 which is the law as it is written"
            );
        } else {
            println!(
                "[named-environment] {} of them on MENTION rather than control, because a .env \
                 site named above did not resolve — what those targets set is not known here",
                report.reach.targets_on_mention
            );
        }
        return ExitCode::SUCCESS;
    }

    if !unchecked.is_empty() {
        eprintln!(
            "[named-environment] {} spawn(s) let the machine decide which program runs",
            unchecked.len(),
        );
        if report.findings.is_empty() {
            return ExitCode::from(1);
        }
    }

    for finding in &report.findings {
        println!(
            "[named-environment] DEFECT {} — read at {}:{}",
            finding,
            show(&finding.read_in),
            finding.line,
        );
        println!(
            "                    why: on a machine where `{}` is set, that test runs a different \
             program than it does on one where it is not",
            finding.variable
        );
        println!(
            "                    fix: name it in the test that spawns `{}` — `.env(\"{}\", ..)` \
             for the value the case declares, or `.env_remove(\"{}\")` for its absence",
            finding.binary, finding.variable, finding.variable,
        );
    }
    eprintln!(
        "[named-environment] {} variable(s) a spawned program reads are left to the machine",
        report.findings.len(),
    );
    ExitCode::from(1)
}
