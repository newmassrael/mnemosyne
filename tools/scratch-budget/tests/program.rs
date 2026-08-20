//! The program itself: what it prints, what it removes, and what it refuses.
//!
//! A DECISION IN `main.rs` HAS NO READER — R1096's finding, and the reason this
//! file exists rather than a longer comment there. The rules below are all
//! decisions that live in the entry point: which of the two tallies a summary
//! line reports, which exit code a refusal carries, and whether asking for an
//! explanation touches anything. Every one of them is invisible to the library
//! cases beside this file.
//!
//! THE FIRST OF THEM WAS ALREADY WRONG WHEN THIS WAS WRITTEN. The dry run
//! printed `would remove 0 file(s) removed (0.0 MiB)` over a plan holding 212
//! files and 14 MiB, because the summary was built from the tally that a dry run
//! deliberately leaves at zero. A reader takes that line as "nothing to
//! collect", which is the exact sentence this whole round exists to stop being
//! said about a directory that is growing.

use std::path::PathBuf;
use std::process::Command;

const MIB: usize = 1024 * 1024;

fn collector() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_scratch-budget"));
    // WHICH CARGO, ABSENT ON PURPOSE (R1262). This collector reaches `ci-plan`,
    // whose one door to a cargo command reads `CARGO` to pin the cargo that built
    // the process — so the program these cases drive now READS a variable none of
    // them uses, and R1211's law is right to ask them to say which. Removed
    // rather than set: this collector sweeps directories and runs no cargo.
    command.env_remove("CARGO");
    command
}

/// A tree with a record directory in it, OUTSIDE this repository and named for
/// the process that owns it (`tools/unowned-scratch`'s law).
fn tree(case: &str) -> PathBuf {
    let at = std::env::temp_dir().join(format!("scratch-budget-run-{}-{case}", std::process::id()));
    let _ = std::fs::remove_dir_all(&at);
    at
}

fn output(command: &mut Command) -> (i32, String) {
    let out = command.output().expect("the collector runs");
    let printed = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (
        out.status.code().expect("an exit code, not a signal"),
        printed,
    )
}

/// The budget the declaration gives the wrapper's log directory, and the
/// directory it gives it to — READ rather than repeated, so that changing a
/// number in the declaration does not silently make this case test nothing.
fn wrapper_records() -> (PathBuf, u64) {
    let declaration = scratch_budget::read_declaration(&scratch_budget::declaration_path())
        .expect("the declaration reads");
    let entry = declaration
        .directories
        .into_iter()
        .find(|entry| entry.path.ends_with("verify-logs"))
        .expect("the wrapper's log directory is declared");
    assert!(
        entry.budget_mib < 64,
        "this case builds a directory one mebibyte at a time to get over the \
         budget, and {} MiB of them is more than a test should write. Raise the \
         budget and this case needs a different shape",
        entry.budget_mib
    );
    (entry.path, entry.budget_mib)
}

#[test]
fn a_dry_run_reports_the_plan_it_did_not_carry_out_and_a_real_one_carries_it_out() {
    let at = tree("summary");
    let (declared, budget_mib) = wrapper_records();
    let records = at.join(&declared);
    std::fs::create_dir_all(&records).expect("mkdir");
    // TWO MEBIBYTES OVER, so exactly two records have to go: a case that went
    // wildly over would pass whether the collector stopped at the budget or
    // emptied the directory.
    let over = budget_mib + 2;
    for index in 0..over {
        std::fs::write(records.join(format!("{index:04}.log")), vec![b'x'; MIB])
            .expect("write record");
    }

    let (code, said) = output(collector().arg("--at").arg(&at).arg("--dry-run"));
    assert_eq!(code, 0, "{said}");
    assert!(
        said.contains("2 file(s) would be removed (2.0 MiB)"),
        "a dry run reports the plan it did NOT carry out: {said}"
    );
    assert_eq!(
        std::fs::read_dir(&records).expect("read").count(),
        over as usize,
        "and removes nothing"
    );

    let (code, said) = output(collector().arg("--at").arg(&at));
    assert_eq!(code, 0, "{said}");
    assert!(
        said.contains("2 file(s) removed (2.0 MiB)"),
        "a real run reports what it did: {said}"
    );
    assert!(
        !said.contains("would"),
        "and does not hedge about it: {said}"
    );
    assert_eq!(
        std::fs::read_dir(&records).expect("read").count(),
        budget_mib as usize,
        "the directory is inside its budget"
    );
    assert!(
        !records.join("0000.log").exists()
            && !records.join("0001.log").exists()
            && records.join("0002.log").exists(),
        "and it was the two OLDEST that went, in the order they were written"
    );

    // AND AGAIN IS A NO-OP, which is what makes it safe on every verification.
    let (code, said) = output(collector().arg("--at").arg(&at));
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("0 file(s) removed"), "{said}");
    let _ = std::fs::remove_dir_all(&at);
}

#[test]
fn a_tree_with_no_records_in_it_is_a_green_run_and_not_a_refusal() {
    // EVERY CI RUNNER IS THIS CASE, and so is every fresh checkout: the record
    // directories do not exist at all. A collector that refused here would fail
    // every verification on a clean machine.
    let at = tree("empty");
    std::fs::create_dir_all(at.join("target")).expect("mkdir");
    let (code, said) = output(collector().arg("--at").arg(&at));
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("0 file(s) removed"), "{said}");
    let _ = std::fs::remove_dir_all(&at);
}

#[test]
fn every_refusal_carries_two_and_says_which_tree_it_would_have_collected_in() {
    // 2 AND NEVER 1. Its sibling gates answer 1 for "these sites break the law";
    // this program does not judge, so 1 would say a defect exists where there is
    // only a collector that did not run. `scripts/verify.sh` reads these codes.
    for arguments in [vec![], vec!["--dry-run"]] {
        let (code, said) = output(collector().args(&arguments));
        assert_eq!(code, 2, "{said}");
        assert!(said.contains("--at"), "{said}");
    }
    let (code, said) = output(collector().args(["--at", "/tmp", "--collect-everything"]));
    assert_eq!(code, 2, "{said}");
    assert!(
        said.contains("--collect-everything") && said.contains("usage"),
        "an unknown argument is named, not ignored: {said}"
    );
    let (code, said) = output(collector().arg("--at"));
    assert_eq!(code, 2, "{said}");
    assert!(said.contains("given nothing"), "{said}");
}

#[test]
fn the_policy_can_be_asked_of_the_program_and_asking_collects_nothing() {
    let at = tree("explain");
    let (declared, _) = wrapper_records();
    let records = at.join(&declared);
    std::fs::create_dir_all(&records).expect("mkdir");
    std::fs::write(records.join("kept.log"), vec![b'x'; MIB]).expect("write record");

    let (code, said) = output(collector().arg("--at").arg(&at).arg("--explain"));
    assert_eq!(code, 0, "{said}");
    // EVERY DECLARED DIRECTORY, WITH ITS REASON. A policy a person can only read
    // by opening a JSON file beside a crate they have never heard of is a policy
    // that gets guessed at instead.
    let declaration = scratch_budget::read_declaration(&scratch_budget::declaration_path())
        .expect("the declaration reads");
    for entry in &declaration.directories {
        assert!(
            said.contains(&entry.path.display().to_string()) && said.contains(&entry.why),
            "{} is not in what the program says it collects: {said}",
            entry.path.display()
        );
    }
    assert!(
        records.join("kept.log").exists(),
        "asking a program what it would do must not make it do it"
    );
    let _ = std::fs::remove_dir_all(&at);
}
