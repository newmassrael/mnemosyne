//! What this gate does, proved on trees built for the purpose.
//!
//! Every mechanism that decides a verdict here needs a repository shaped in a
//! way this one is not: a file no `mod` reaches, a `tests/` entry point the
//! manifest disowns, a target directory holding the record of a unit that no
//! longer exists. Putting any of them into this repository to test the gate
//! would create the very thing the gate rejects, so they live in fixture
//! workspaces built for one assertion and thrown away.
//!
//! They are built OUTSIDE this repository, for two reasons. A fixture carrying
//! its own `[workspace]` inside the tree is discovered by
//! `scripts/check-side-workspaces.sh`, which would then lint a workspace whose
//! whole purpose is to be rejected — and this gate reads `git ls-files`, so a
//! fixture inside the tree would answer with THIS repository's files.
//!
//! The fixtures depend on nothing, which is what keeps this affordable: each is
//! two `cargo check`s over a crate with one or two source files.

use std::collections::BTreeSet;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use ci_plan::SkippedWorkspace;
use uncompiled_sources::{
    check_command, conclude, examine, probe, probe_workspace, tracked_sources, Finding, Refusal,
    Report,
};

/// A throwaway workspace that git tracks, which is what makes it a repository
/// this gate will read at all.
struct Fixture {
    _at: tempfile::TempDir,
    root: PathBuf,
}

impl Fixture {
    /// `files` are written relative to the root; the manifest is one of them.
    fn new(files: &[(&str, &str)]) -> Fixture {
        let at = tempfile::tempdir().expect("a scratch directory");
        // Canonical, because cargo reports a canonical `workspace_root` and the
        // gate strips one path from the other.
        let root = at.path().canonicalize().expect("canonical scratch");
        for (name, body) in files {
            let path = root.join(name);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).expect("create fixture directory");
            }
            std::fs::write(&path, body).expect("write fixture file");
        }
        let fixture = Fixture { _at: at, root };
        fixture.track();
        fixture
    }

    /// `git add` rather than `git commit`: `git ls-files` reads the index, and
    /// committing would need an identity this machine may not have configured.
    fn track(&self) {
        for arguments in [["init", "--quiet"].as_slice(), ["add", "-A"].as_slice()] {
            let status = Command::new("git")
                .args(arguments)
                .current_dir(&self.root)
                .status()
                .expect("git runs");
            assert!(status.success(), "git {arguments:?} failed");
        }
    }

    fn write(&self, name: &str, body: &str) {
        std::fs::write(self.root.join(name), body).expect("rewrite fixture file");
    }

    fn judged(&self) -> Report {
        examine(&self.root, &["Cargo.toml".to_string()], &[])
    }

    fn read(&self) -> BTreeSet<PathBuf> {
        let mut refusals = Vec::new();
        let probes = probe_workspace(&self.root, "Cargo.toml", &mut refusals);
        assert!(refusals.is_empty(), "{refusals:?}");
        probes
            .iter()
            .flat_map(|probe| probe.read.iter().cloned())
            .collect()
    }
}

const MANIFEST: &str = "[workspace]\n\
     \n\
     [package]\n\
     name = \"fixture\"\n\
     version = \"0.1.0\"\n\
     edition = \"2021\"\n";

fn found(report: &Report) -> Vec<&Finding> {
    assert!(
        report.verdict().is_ok(),
        "the gate refused instead of judging: {:?}",
        report.refusals
    );
    report.findings.iter().collect()
}

fn files(report: &Report) -> Vec<&Path> {
    found(report)
        .iter()
        .map(|finding| finding.file.as_path())
        .collect()
}

#[test]
fn a_file_no_module_reaches_is_rust_nothing_compiles() {
    let fixture = Fixture::new(&[
        ("Cargo.toml", MANIFEST),
        ("src/lib.rs", "pub fn reached() {}\n"),
        (
            "src/orphan.rs",
            "#[test]\n\
             fn orphaned() {}\n\
             \n\
             mod inner {\n\
             \x20   #[test]\n\
             \x20   fn nested() {}\n\
             }\n",
        ),
    ]);
    let report = fixture.judged();

    assert_eq!(
        files(&report),
        vec![Path::new("src/orphan.rs")],
        "a file the crate root never declares is compiled by nothing"
    );
    assert_eq!(
        found(&report)[0].tests,
        vec!["orphaned".to_string(), "inner::nested".to_string()],
        "and the tests inside it are the reason this is worse than dead code: \
         R1084's gate cannot see one of them, because there is no build to see \
         it in"
    );
    // NON-VACUITY. If the probe had read nothing at all, every file would be a
    // finding and the assertion above would pass for the wrong reason.
    assert!(
        report.read.contains(Path::new("src/lib.rs")),
        "the probe must have actually compiled the crate: {:?}",
        report.read
    );
}

#[test]
fn a_tests_entry_point_the_manifest_disowns_is_rust_nothing_compiles() {
    // THE SHAPE HH1 IS NAMED FOR. `autotests = false` and a `tests/*.rs` that
    // no `[[test]]` claims: cargo builds no target from it, so R1084's gate
    // finds it in neither its population nor its run set, and the two absences
    // cancel into a clean report.
    let entry_point = "#[test]\nfn never_built() {}\n";
    let disowned = Fixture::new(&[
        ("Cargo.toml", &format!("{MANIFEST}autotests = false\n")),
        ("src/lib.rs", "pub fn reached() {}\n"),
        ("tests/smoke.rs", entry_point),
    ]);
    let report = disowned.judged();
    assert_eq!(
        files(&report),
        vec![Path::new("tests/smoke.rs")],
        "cargo makes no target from it, so nothing compiles it"
    );

    // THE CONTROL, through the same function: the identical file in a manifest
    // that does not disown it. Without this, the assertion above is equally
    // satisfied by a gate that reports every `tests/` file.
    let claimed = Fixture::new(&[
        ("Cargo.toml", MANIFEST),
        ("src/lib.rs", "pub fn reached() {}\n"),
        ("tests/smoke.rs", entry_point),
    ]);
    let control = claimed.judged();
    assert_eq!(
        files(&control),
        Vec::<&Path>::new(),
        "and with the same file under an ordinary manifest there is nothing to \
         report: {:?}",
        control.findings
    );
}

#[test]
fn a_file_only_a_macro_names_is_still_compiled() {
    // WHY THE INSTRUMENT IS DEP-INFO AND NOT A MODULE WALK. Nothing declares
    // this file; `include!` pulls it in through a path built by two macros. A
    // walker following `mod` items reports it as uncompiled, which is a false
    // finding against a file the compiler demonstrably reads — and this
    // repository holds the case for real, in `codegen-prototype`'s golden
    // fixture.
    let fixture = Fixture::new(&[
        ("Cargo.toml", MANIFEST),
        (
            "src/lib.rs",
            "include!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/src/generated.rs\"));\n",
        ),
        ("src/generated.rs", "pub fn generated() {}\n"),
    ]);
    let report = fixture.judged();

    assert_eq!(
        files(&report),
        Vec::<&Path>::new(),
        "the compiler opened it, so something compiles it"
    );
    assert!(
        report.read.contains(Path::new("src/generated.rs")),
        "and it is in the read set by name, not merely absent from the \
         findings: {:?}",
        report.read
    );
}

#[test]
fn a_build_script_is_compiled_too() {
    // Cargo reports a build script's artifact under a name rustc did not write
    // the dep-info under — `build-script-build` against
    // `build_script_build-<hash>` — so the two are joined by the directory they
    // are alone in. Get that wrong and the gate REFUSES rather than passing,
    // which is why this asserts the verdict as well as the read set.
    let fixture = Fixture::new(&[
        ("Cargo.toml", MANIFEST),
        ("src/lib.rs", "pub fn reached() {}\n"),
        (
            "build.rs",
            "fn main() { println!(\"cargo::rerun-if-changed=build.rs\"); }\n",
        ),
    ]);
    let report = fixture.judged();

    assert!(
        report.verdict().is_ok(),
        "a build script cargo built and this gate could not join is a REFUSAL, \
         not a finding: {:?}",
        report.refusals
    );
    assert!(
        report.read.contains(Path::new("build.rs")),
        "a build script is compiled, so it is in the read set: {:?}",
        report.read
    );
    assert_eq!(files(&report), Vec::<&Path>::new());
}

#[test]
fn one_feature_resolve_cannot_see_what_the_other_hides() {
    // WHY THERE ARE TWO PROBES. `--all-features` is what makes a file behind a
    // feature part of the population, and it is also what hides a file behind
    // `not(feature)`. One flag cannot do both, so the read set is a union — and
    // this test fails if either half is dropped for cost.
    let fixture = Fixture::new(&[
        (
            "Cargo.toml",
            &format!("{MANIFEST}\n[features]\nextra = []\n"),
        ),
        (
            "src/lib.rs",
            "#[cfg(feature = \"extra\")]\n\
             mod behind_the_feature;\n\
             #[cfg(not(feature = \"extra\"))]\n\
             mod behind_its_absence;\n",
        ),
        ("src/behind_the_feature.rs", "pub fn one() {}\n"),
        ("src/behind_its_absence.rs", "pub fn other() {}\n"),
    ]);

    let mut refusals = Vec::new();
    let all = probe(
        &fixture.root,
        &check_command("Cargo.toml", true),
        &mut refusals,
    )
    .expect("the fixture compiles with every feature");
    let default = probe(
        &fixture.root,
        &check_command("Cargo.toml", false),
        &mut refusals,
    )
    .expect("the fixture compiles with its default features");
    assert!(refusals.is_empty(), "{refusals:?}");

    assert!(
        all.read.contains(Path::new("src/behind_the_feature.rs"))
            && !all.read.contains(Path::new("src/behind_its_absence.rs")),
        "all-features sees the first and hides the second: {:?}",
        all.read
    );
    assert!(
        default
            .read
            .contains(Path::new("src/behind_its_absence.rs"))
            && !default
                .read
                .contains(Path::new("src/behind_the_feature.rs")),
        "and the default resolve is exactly the other way round: {:?}",
        default.read
    );
    assert_eq!(
        files(&fixture.judged()),
        Vec::<&Path>::new(),
        "so the union covers both, and neither is reported"
    );
}

#[test]
fn a_warm_target_directory_does_not_credit_a_unit_that_is_gone() {
    // THE JOIN, AND WHY IT IS NOT OPTIONAL. Cargo never garbage-collects
    // dep-info: a target removed from the graph leaves its record behind,
    // naming every file it used to read. A gate that swept the directory would
    // keep crediting them, and would do so ONLY on a machine with a warm target
    // directory — passing here, failing on a cold runner, or the reverse.
    let fixture = Fixture::new(&[
        ("Cargo.toml", MANIFEST),
        ("src/lib.rs", "pub fn reached() {}\n"),
        ("tests/smoke.rs", "#[test]\nfn built_once() {}\n"),
    ]);
    assert!(
        fixture.read().contains(Path::new("tests/smoke.rs")),
        "while it is a target, it is compiled"
    );

    // Take the target away without taking the file away, and DO NOT clean.
    fixture.write("Cargo.toml", &format!("{MANIFEST}autotests = false\n"));
    let report = fixture.judged();

    // NON-VACUITY: the stale record is still on disk and still names the file.
    // If cargo had cleaned it up, the assertion below would pass without the
    // join ever having been consulted.
    let mut stale = Vec::new();
    dep_info_naming(&fixture.root.join("target"), "tests/smoke.rs", &mut stale);
    assert!(
        !stale.is_empty(),
        "cargo kept no stale dep-info, so this test proves nothing about the \
         join"
    );
    assert_eq!(
        files(&report),
        vec![Path::new("tests/smoke.rs")],
        "the record survives and the unit does not, so the file is uncompiled — \
         {} stale record(s) said otherwise",
        stale.len()
    );
}

/// Every dep-info file under `at` whose text names `needle`.
fn dep_info_naming(at: &Path, needle: &str, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(at) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            dep_info_naming(&path, needle, out);
        } else if path.extension().is_some_and(|extension| extension == "d")
            && std::fs::read_to_string(&path).is_ok_and(|text| text.contains(needle))
        {
            out.push(path);
        }
    }
}

#[test]
fn dep_info_this_gate_cannot_read_is_a_refusal_and_not_an_empty_answer() {
    // The read set is the gate's whole evidence, and "I read nothing" looks
    // exactly like "nothing compiles any of this". Whichever way that is
    // resolved is a decision, and resolving it as a FINDING would report every
    // file in the repository as uncompiled the first time a target directory
    // was unreadable — a headline that hides a permissions problem.
    let fixture = Fixture::new(&[
        ("Cargo.toml", MANIFEST),
        ("src/lib.rs", "pub fn reached() {}\n"),
    ]);
    assert!(
        fixture.read().contains(Path::new("src/lib.rs")),
        "the control: with dep-info readable, the crate root is read"
    );

    let mut records = Vec::new();
    dep_info_naming(&fixture.root.join("target"), "", &mut records);
    assert!(!records.is_empty(), "the build wrote no dep-info to hide");
    for record in &records {
        std::fs::set_permissions(record, std::fs::Permissions::from_mode(0o000))
            .expect("make the record unreadable");
    }

    let mut refusals = Vec::new();
    let probed = probe(
        &fixture.root,
        &check_command("Cargo.toml", true),
        &mut refusals,
    );
    assert!(probed.is_none(), "there is no read set to report");
    assert!(
        matches!(refusals.as_slice(), [Refusal::NoDepInfo { .. }]),
        "cargo says it built something and rustc's record of what that read is \
         unavailable — unknown, not empty: {refusals:?}"
    );

    for record in &records {
        std::fs::set_permissions(record, std::fs::Permissions::from_mode(0o644))
            .expect("put the record back");
    }
}

#[test]
fn a_target_root_the_build_never_read_is_a_disagreement_and_not_a_finding() {
    // THE SELF-CHECK. Cargo NAMES its targets; rustc names the files it read.
    // If a unit's artifact joins to some record that does not mention the
    // target's own root, the join matched the wrong thing — and the read set
    // that comes back is large, confident and about another unit. Here the
    // record is tampered with to produce exactly that disagreement, because
    // producing it by breaking the join would only prove the join is broken.
    let fixture = Fixture::new(&[
        ("Cargo.toml", MANIFEST),
        ("src/lib.rs", "pub fn reached() {}\n"),
    ]);
    assert!(
        fixture.read().contains(Path::new("src/lib.rs")),
        "the control: cargo's target root and rustc's record agree"
    );

    let mut records = Vec::new();
    dep_info_naming(&fixture.root.join("target"), "src/lib.rs", &mut records);
    assert!(
        !records.is_empty(),
        "no record names the crate root to tamper with"
    );
    for record in &records {
        let text = std::fs::read_to_string(record).expect("read the record");
        // The TARGET lines are left alone, so every artifact still joins; only
        // what the join says was READ is changed.
        std::fs::write(record, text.replace("src/lib.rs", "src/decoy.rs"))
            .expect("tamper with the record");
    }

    let mut refusals = Vec::new();
    let probed = probe(
        &fixture.root,
        &check_command("Cargo.toml", true),
        &mut refusals,
    );
    assert!(probed.is_none(), "{probed:?}");
    assert!(
        matches!(
            refusals.as_slice(),
            [Refusal::TargetRootUnread { target_src, .. }] if target_src == Path::new("src/lib.rs")
        ),
        "two of the machine's answers disagree, and the gate says so instead of \
         reporting the files the wrong record happened to name: {refusals:?}"
    );
}

#[test]
fn a_probe_that_cannot_compile_refuses_instead_of_reporting_nothing() {
    let fixture = Fixture::new(&[
        ("Cargo.toml", MANIFEST),
        ("src/lib.rs", "fn ( this is not rust\n"),
    ]);
    let mut refusals = Vec::new();
    let probed = probe(
        &fixture.root,
        &check_command("Cargo.toml", true),
        &mut refusals,
    );

    assert!(
        probed.is_none(),
        "a tree that did not compile has no read set to report"
    );
    assert!(
        matches!(refusals.as_slice(), [Refusal::CheckFailed { .. }]),
        "the answer is REFUSED, not clean — a gate that could not compile and a \
         gate that compiled everything print the same empty finding list \
         otherwise: {refusals:?}"
    );

    // And the whole gate carries that through rather than reporting the tree as
    // entirely uncompiled, which is what an unreported refusal would look like.
    let report = fixture.judged();
    assert!(report.verdict().is_err(), "{:?}", report.findings);
}

#[test]
fn the_verdict_and_the_findings_are_two_different_answers() {
    // "I could not judge" and "I judged and found nothing" are the same exit
    // code unless something separates them, and this is where they separate.
    assert!(
        Report::default().verdict().is_ok(),
        "no refusal means the gate judged, whatever it found"
    );
    let refused = Report {
        refusals: vec![Refusal::NothingTracked],
        ..Report::default()
    };
    assert!(
        refused.verdict().is_err(),
        "one refusal is enough: a gate that compiled part of the tree has no \
         verdict about the rest of it"
    );
}

#[test]
fn a_tree_holding_no_rust_is_a_refusal_and_not_a_clean_answer() {
    let empty = conclude(
        Path::new("/nowhere"),
        &BTreeSet::new(),
        &BTreeSet::new(),
        &[],
    );
    assert!(
        matches!(empty.refusals.as_slice(), [Refusal::NothingTracked]),
        "zero tracked files is the shape of a gate pointed at the wrong tree, \
         not of a repository that passes: {:?}",
        empty.refusals
    );
}

#[test]
fn a_file_in_a_workspace_this_machine_cannot_compile_is_not_a_finding() {
    // HH5's asymmetry, kept out of the verdict. `studio` depends on a sibling
    // checkout that exists here and on no hosted runner, so the lister declines
    // it there. Reporting its files as uncompiled would make the verdict a fact
    // about WHICH MACHINE ran the gate.
    let tracked: BTreeSet<PathBuf> = ["studio/src/lib.rs", "crates/x/src/lib.rs"]
        .iter()
        .map(PathBuf::from)
        .collect();
    let read: BTreeSet<PathBuf> = [PathBuf::from("crates/x/src/lib.rs")].into_iter().collect();

    let skipped = [SkippedWorkspace {
        directory: "studio".to_string(),
        reason: "its path dependencies leave this repository".to_string(),
    }];
    let declined = conclude(Path::new("/nowhere"), &tracked, &read, &skipped);
    assert_eq!(declined.findings, Vec::new(), "{:?}", declined.findings);
    assert_eq!(
        declined.in_skipped_workspace,
        [PathBuf::from("studio/src/lib.rs")].into_iter().collect(),
        "it is counted and printed, because a limit nobody sees is a lie"
    );

    // THE CONTROL, through the same function: with nothing declined, the same
    // unread file IS a finding. Otherwise this test passes for a gate that
    // reports nothing at all.
    let checked = conclude(Path::new("/nowhere"), &tracked, &read, &[]);
    assert_eq!(
        checked
            .findings
            .iter()
            .map(|finding| finding.file.as_path())
            .collect::<Vec<_>>(),
        vec![Path::new("studio/src/lib.rs")],
        "{:?}",
        checked.findings
    );
}

#[test]
fn a_candidate_that_does_not_parse_refuses_rather_than_reporting_no_tests() {
    let fixture = Fixture::new(&[
        ("Cargo.toml", MANIFEST),
        ("src/lib.rs", "pub fn reached() {}\n"),
        ("src/orphan.rs", "fn ( this is not rust\n"),
    ]);
    let report = fixture.judged();
    assert!(
        matches!(report.refusals.as_slice(), [Refusal::Unparsed { file, .. }] if file == Path::new("src/orphan.rs")),
        "an uncompiled file that will not parse holds an UNKNOWN number of \
         tests, and reporting zero would understate it: {:?}",
        report.refusals
    );
    assert_eq!(
        report
            .findings
            .iter()
            .map(|finding| finding.file.as_path())
            .collect::<Vec<_>>(),
        vec![Path::new("src/orphan.rs")],
        "and it is STILL a finding — whether anything compiles it was decided \
         by two sets, not by whether this gate could read it, so an unreadable \
         file must not be able to hide the defect it is: {:?}",
        report.findings
    );
}

#[test]
fn git_is_what_says_which_files_are_this_repositorys() {
    // An untracked file is not this repository's to judge, and a CI checkout
    // does not have it either. A gate that walked the disk would report a file
    // that only exists on the machine it ran on.
    let fixture = Fixture::new(&[
        ("Cargo.toml", MANIFEST),
        ("src/lib.rs", "pub fn reached() {}\n"),
    ]);
    std::fs::write(
        fixture.root.join("src/untracked.rs"),
        "#[test]\nfn x() {}\n",
    )
    .expect("write an untracked file");

    let tracked = tracked_sources(&fixture.root).expect("git ls-files answers");
    assert_eq!(
        tracked,
        [PathBuf::from("src/lib.rs")].into_iter().collect(),
        "git's index is the population, not the directory listing"
    );
    assert_eq!(files(&fixture.judged()), Vec::<&Path>::new());
}

#[test]
fn the_probe_asks_for_every_target() {
    // Pinned against the flags, because they are the whole definition of what
    // "cargo compiles this file" means here. `cargo test` does not build
    // examples even when it runs them nowhere — measured on cargo 1.94 against
    // `codegen-prototype`, whose one example no command in this repository had
    // ever compiled until `--all-targets` was asked for.
    let all = check_command("bench/Cargo.toml", true);
    assert!(all.has("--all-targets"), "{:?}", all.cargo_args);
    assert!(all.has("--all-features"), "{:?}", all.cargo_args);
    assert!(all.has("--workspace"), "{:?}", all.cargo_args);
    assert_eq!(all.subcommand(), Some("check"));
    assert_eq!(all.value(&["--manifest-path"]), Some("bench/Cargo.toml"));

    let default = check_command("bench/Cargo.toml", false);
    assert!(default.has("--all-targets"), "{:?}", default.cargo_args);
    assert!(
        !default.has("--all-features"),
        "the second resolve is the one that sees `not(feature)`: {:?}",
        default.cargo_args
    );
}
