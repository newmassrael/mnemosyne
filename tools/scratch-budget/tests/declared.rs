//! EVERY DIRECTORY THIS REPOSITORY DECLARES IT WRITES RECORDS INTO IS ONE
//! SOMETHING COLLECTS.
//!
//! The collector beside this file bounds the directories `scratch.json` names.
//! That leaves the question this law exists for: is that list the same list as
//! the one the repository actually writes into? Nothing connected the two until
//! R1199, and the gap is what N45 was — `target/verify-logs` grew for sixteen
//! days and 764 files before anybody looked, and the reason nobody looked is
//! that a directory with no collector is indistinguishable from one whose
//! collector has nothing to do.
//!
//! THE POPULATION IS ASKED OF THE PROGRAMS, NOT COPIED FROM THEM. Two kinds of
//! program here write records:
//!
//!   - every injection sweep, which declares its log directory in a `logs`
//!     field the harness resolves against the manifest's own directory; and
//!   - `scripts/verify.sh`, which writes one log per verification and is asked
//!     directly (`--print-logdir`) rather than read.
//!
//! A LIST WRITTEN HERE WOULD BE A THIRD SPELLING and would go stale in the
//! direction that reads as clean — the shape this repository has now paid for
//! at four different levels (R777, R783, R1080, R1082) and once more in the
//! debt this law closes.
//!
//! THE OTHER DIRECTION IS DELIBERATELY NOT A LAW. A declared directory that no
//! program in this repository writes into is not a defect: `target/bx-logs` is
//! written by `bx`, a machine-global tool that lives outside this checkout,
//! prunes only the REMOTE tree it builds in, and leaves one of these
//! directories in every repository it is invoked from. The declaration is
//! entitled to be a superset; it is not entitled to be a subset.

use std::path::{Path, PathBuf};
use std::process::Command;

use scratch_budget::{parse_declaration, read_declaration, uncovered};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("this crate lives two directories below the repository root")
        .to_path_buf()
}

/// Where `scripts/verify.sh` writes its logs — asked of the script.
///
/// ASKED RATHER THAN READ. The directory is one shell expansion
/// (`${VERIFY_LOGDIR:-target/verify-logs}`) and a reader here could match it
/// out of the source in a line. That reader would then be a second definition
/// of the wrapper's log directory, right up until somebody changed the default
/// or moved it behind a variable — and it would answer the old path, which is
/// a directory nothing writes to and therefore always within its budget. The
/// program is the specification; this asks it.
fn wrapper_log_directory(root: &Path) -> String {
    let out = Command::new(root.join("scripts/verify.sh"))
        .arg("--print-logdir")
        .current_dir(root)
        .output()
        .expect("the verify wrapper runs");
    assert!(
        out.status.success(),
        "scripts/verify.sh --print-logdir failed ({}): {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    let printed = String::from_utf8_lossy(&out.stdout).trim().to_string();
    assert!(
        !printed.is_empty(),
        "the wrapper printed no log directory at all, so this law would hold \
         over one source fewer without saying so"
    );
    printed
}

/// Every directory this repository's own programs declare they write records
/// into, as `(who says so, where)`.
fn record_directories(root: &Path) -> Vec<(String, PathBuf)> {
    let mut sources: Vec<(String, PathBuf)> = injection_harness::tracked_sweeps(root)
        .expect("the sweeps this repository tracks")
        .into_iter()
        // `logs` comes back ABSOLUTE, resolved against the manifest's own
        // directory by the harness's reader — which is why this law does not
        // need to know that a sweep in `tools/x/` spells it `../../target/…`
        // and one in `.githooks/` spells it `../target/…`.
        .map(|(path, manifest)| (format!("the sweep manifest {path}"), manifest.logs))
        .collect();
    sources.push((
        "scripts/verify.sh".to_string(),
        root.join(wrapper_log_directory(root)),
    ));
    sources
}

#[test]
fn every_directory_this_repository_writes_records_into_is_one_something_collects() {
    let root = repository_root();
    let declaration = read_declaration(&scratch_budget::declaration_path())
        .expect("this repository's scratch declaration");
    let sources = record_directories(&root);

    // NON-VACUITY, IN BOTH OF THE WAYS THIS LAW CAN BE EMPTY. A walk that read
    // no manifest and a repository whose every directory is collected print the
    // same silence — and so does a walk that read every manifest and resolved
    // them all to one directory nobody writes to.
    let sweeps = sources.len() - 1;
    assert!(
        sweeps >= 12,
        "{sweeps} sweep manifest(s) named a log directory, which is fewer than \
         this repository tracks — a law over the wrong population is the empty \
         answer that reads as a clean one: {sources:?}"
    );
    let distinct: std::collections::BTreeSet<&PathBuf> = sources.iter().map(|(_, at)| at).collect();
    assert!(
        distinct.len() >= 2,
        "every source named the same directory ({distinct:?}), so this law is \
         about one path and not about a population"
    );

    let findings = uncovered(&declaration, &root, &sources);
    assert!(
        findings.is_empty(),
        "{} record directory(ies) this repository writes into have no collector. \
         A directory nothing collects grows for as long as the repository is \
         worked in, and it is invisible: `target/` is gitignored, so nothing \
         about it appears in a commit, and the build machine prunes its own copy \
         so nothing about it appears in a remote run either:\n  {}",
        findings.len(),
        findings.join("\n  ")
    );

    println!(
        "[scratch] {} source(s) over {} directory(ies), all collected",
        sources.len(),
        distinct.len()
    );
}

/// THE CONTROL, and the reason the law above is not merely a walk over a
/// repository that happens to be tidy. Its own first run is what proved it can
/// fail — the three directories it names had no collector at all — and that
/// evidence is spent the moment they gained one.
///
/// Driven through the same `uncovered` the law uses. A control that
/// re-implemented the comparison would prove that a second spelling can find a
/// difference, which is the one thing nobody needs to know.
#[test]
fn a_directory_no_entry_names_is_reported_rather_than_passed_over() {
    let tree = Path::new("/repo");
    let declaration = parse_declaration(
        r#"{ "directories": [
             { "path": "target/verify-logs", "budget_mib": 1, "why": "the one that is declared" }
           ] }"#,
        "the control's declaration",
    )
    .expect("the control's declaration reads");

    // THE SPELLING IS THE ONE THE MANIFESTS USE. Every sweep in this repository
    // names its log directory with `..` in it, and a comparison that did not
    // normalise both sides would report the collected directory as uncollected
    // — a law that fails on a repository with no defect is worse than none.
    let findings = uncovered(
        &declaration,
        tree,
        &[
            (
                "the sweep manifest tools/x/injection-sweep.json".to_string(),
                PathBuf::from("/repo/tools/x/../../target/verify-logs"),
            ),
            (
                "the sweep manifest tools/y/injection-sweep.json".to_string(),
                PathBuf::from("/repo/target/injection-logs"),
            ),
        ],
    );
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert!(
        findings[0].contains("target/injection-logs")
            && findings[0].contains("tools/y/injection-sweep.json"),
        "the finding names the directory AND who writes into it, because \
         `something is uncollected` sends a reader looking: {findings:?}"
    );

    // AND A DECLARATION THAT DECLARES NOTHING IS A REFUSAL rather than a
    // collector with an empty list — which would pass every law here in silence.
    let empty = parse_declaration(r#"{ "directories": [] }"#, "an empty declaration")
        .expect_err("an empty declaration is refused");
    assert!(empty.contains("no directory at all"), "{empty}");

    // A KEY NOBODY MEANT CANNOT HIDE BESIDE ONE THAT MATTERS: a typo in
    // `budget_mib` would otherwise be a directory silently collected to some
    // other number.
    let typo = parse_declaration(
        r#"{ "directories": [
             { "path": "target/x", "budget_mib": 1, "why": "y", "budget_mb": 4096 }
           ] }"#,
        "a declaration with a typo",
    )
    .expect_err("an unknown key is refused");
    assert!(typo.contains("budget_mb"), "{typo}");
}
