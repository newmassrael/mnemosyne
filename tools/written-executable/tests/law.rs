//! The law, asked of trees this test builds — one per answer the gate can give,
//! and one per spelling of the shape it is about.
//!
//! Built OUTSIDE this repository, for the reason its siblings give: a fixture
//! carrying its own `[workspace]` inside the tree would be discovered by
//! `scripts/check-side-workspaces.sh`, which would then lint a workspace whose
//! whole purpose is to be rejected.
//!
//! EVERY CASE HAS A CONTROL. A gate that only ever says "defect" is
//! indistinguishable from a broken one, and a gate that only ever says "clean"
//! is indistinguishable from one that did not run.

use std::path::Path;
use std::process::Command;

use tempfile::TempDir;

use written_executable::{Made, Ran};

/// A workspace of one file, whose body is the case.
fn workspace(body: &str) -> TempDir {
    let at = TempDir::new().expect("tempdir");
    let root = at.path();
    std::fs::create_dir_all(root.join("src")).expect("mkdir src");
    std::fs::write(
        root.join("Cargo.toml"),
        "[workspace]\n[package]\nname = \"probe\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write manifest");
    std::fs::write(root.join("src/lib.rs"), body).expect("write source");
    at
}

fn gate(at: &Path) -> written_executable::Report {
    written_executable::run(&at.join("Cargo.toml")).expect("the gate runs over the fixture")
}

/// The shape the gate exists for, in its shortest spelling.
#[test]
fn a_mode_with_the_executable_bit_is_a_finding() {
    let at = workspace(
        "use std::os::unix::fs::PermissionsExt;\n\
         pub fn install(path: &std::path::Path) {\n    \
         std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();\n}",
    );
    let report = gate(at.path());
    assert_eq!(report.verdict(), Ok(()), "{report:?}");
    assert_eq!(report.applying, 1, "{report:?}");
    assert_eq!(report.findings.len(), 1, "{report:?}");
    assert_eq!(report.findings[0].owner, "install", "{report:?}");
    assert_eq!(report.findings[0].made, Made::Mode(0o755), "{report:?}");
}

/// The control. The same tree, one mode bit different, and the population is
/// still 1 — which is what makes the clean answer a measurement.
#[test]
fn the_same_tree_is_clean_when_the_mode_is_not_executable() {
    let at = workspace(
        "use std::os::unix::fs::PermissionsExt;\n\
         pub fn install(path: &std::path::Path) {\n    \
         std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o644)).unwrap();\n}",
    );
    let report = gate(at.path());
    assert_eq!(report.verdict(), Ok(()), "{report:?}");
    assert_eq!(report.applying, 1, "{report:?}");
    assert!(report.findings.is_empty(), "{report:?}");
}

/// THE SPELLING THIS REPOSITORY ACTUALLY USES, and the reason the unit is the
/// function: the mode is bound on one statement and applied on another, so no
/// expression rule reads it.
#[test]
fn a_mode_bound_on_one_statement_and_applied_on_another_is_a_finding() {
    let at = workspace(
        "use std::os::unix::fs::PermissionsExt;\n\
         pub fn write_exec(path: &std::path::Path) {\n    \
         let mut perms = std::fs::metadata(path).unwrap().permissions();\n    \
         perms.set_mode(0o755);\n    \
         std::fs::set_permissions(path, perms).unwrap();\n}",
    );
    let report = gate(at.path());
    assert_eq!(report.verdict(), Ok(()), "{report:?}");
    assert!(!report.findings.is_empty(), "{report:?}");
    assert!(
        report
            .findings
            .iter()
            .all(|f| f.owner == "write_exec" && f.made == Made::Mode(0o755)),
        "{report:?}"
    );
}

/// A mode arriving as a PARAMETER is one this walk cannot read. Answering
/// "clean" there would be answering a question nobody asked, so it refuses.
#[test]
fn a_mode_this_walk_cannot_read_refuses_rather_than_passing() {
    let at = workspace(
        "use std::os::unix::fs::PermissionsExt;\n\
         pub fn install(path: &std::path::Path, mode: u32) {\n    \
         std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();\n}",
    );
    let report = gate(at.path());
    assert!(report.verdict().is_err(), "{report:?}");
    assert_eq!(report.unreadable.len(), 1, "{report:?}");
    assert_eq!(report.unreadable[0].owner, "install", "{report:?}");
    assert!(report.findings.is_empty(), "{report:?}");
}

/// A nested function is judged on ITS OWN body. Without the walk stopping at a
/// function boundary, the parent's harmless `0o644` would be reported as
/// executable because of a literal written in a helper inside it.
#[test]
fn a_nested_function_does_not_lend_its_mode_to_the_one_around_it() {
    let at = workspace(
        "use std::os::unix::fs::PermissionsExt;\n\
         pub fn outer(path: &std::path::Path) {\n    \
         std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o644)).unwrap();\n    \
         fn inner(path: &std::path::Path) {\n        \
         std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();\n    }\n    \
         inner(path);\n}",
    );
    let report = gate(at.path());
    assert_eq!(report.verdict(), Ok(()), "{report:?}");
    assert_eq!(report.applying, 2, "{report:?}");
    assert_eq!(report.findings.len(), 1, "{report:?}");
    assert_eq!(report.findings[0].owner, "inner", "{report:?}");
}

/// And the other direction, which is the one that would be a SILENT PASS: a
/// helper that writes an executable inside a parent whose own mode is harmless.
#[test]
fn a_nested_function_does_not_borrow_innocence_from_the_one_around_it() {
    let at = workspace(
        "use std::os::unix::fs::PermissionsExt;\n\
         pub fn outer(path: &std::path::Path) {\n    \
         std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();\n    \
         fn inner(path: &std::path::Path) {\n        \
         std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o644)).unwrap();\n    }\n    \
         inner(path);\n}",
    );
    let report = gate(at.path());
    assert_eq!(report.verdict(), Ok(()), "{report:?}");
    assert_eq!(report.findings.len(), 1, "{report:?}");
    assert_eq!(report.findings[0].owner, "outer", "{report:?}");
}

/// `OpenOptions` creates the file executable in one call, with no `Permissions`
/// anywhere. A rule that knew only `set_permissions` would pass it.
#[test]
fn a_file_created_executable_by_open_options_is_a_finding() {
    let at = workspace(
        "use std::os::unix::fs::OpenOptionsExt;\n\
         pub fn install(path: &std::path::Path) {\n    \
         let _ = std::fs::OpenOptions::new().create(true).write(true).mode(0o755).open(path);\n}",
    );
    let report = gate(at.path());
    assert_eq!(report.verdict(), Ok(()), "{report:?}");
    assert_eq!(report.findings.len(), 1, "{report:?}");
    assert_eq!(report.findings[0].made, Made::Mode(0o755), "{report:?}");
}

/// READING a mode is not applying one. This is the real shape of
/// `git_hooks_smoke::hook`, which asserts that a TRACKED hook is executable —
/// mode getter, `0o111` mask, and no write anywhere. A gate that called this a
/// defect would be a gate people delete.
#[test]
fn asserting_that_a_tracked_file_is_executable_is_not_applying_a_mode() {
    let at = workspace(
        "use std::os::unix::fs::PermissionsExt;\n\
         pub fn hook(path: &std::path::Path) {\n    \
         let mode = std::fs::metadata(path).unwrap().permissions().mode();\n    \
         assert!(mode & 0o111 != 0, \"the tracked hook must be executable\");\n}",
    );
    let report = gate(at.path());
    assert_eq!(report.verdict(), Ok(()), "{report:?}");
    assert_eq!(report.applying, 0, "{report:?}");
    assert!(report.findings.is_empty(), "{report:?}");
}

/// ⚠ `mode` IS NOT A RARE METHOD NAME, and this gate learned that by being run
/// over the whole repository rather than by anyone reasoning about it:
/// `bench/crates/sled-baseline` calls
/// `sled::Config::default().mode(sled::Mode::HighThroughput)`, which has nothing
/// to do with permissions, and the first draft REFUSED that whole workspace over
/// it — a gate that answers "I could not judge this tree" about a tree with no
/// chmod anywhere in it.
///
/// The second fact is the one Rust already requires: a trait method is callable
/// only where the trait is imported.
#[test]
fn a_mode_that_is_not_a_permission_is_not_one() {
    let at = workspace(
        "pub fn open(path: &std::path::Path) {\n    \
         let _ = sled::Config::default().path(path).mode(sled::Mode::HighThroughput);\n}",
    );
    let report = gate(at.path());
    assert_eq!(report.verdict(), Ok(()), "{report:?}");
    assert_eq!(report.applying, 0, "{report:?}");
    assert!(report.unreadable.is_empty(), "{report:?}");
    assert!(report.findings.is_empty(), "{report:?}");
}

/// And the control: the SAME call in a file that imports the trait supplying
/// `mode` is read as the permission it then is. Without this pair, narrowing the
/// rule could have been narrowing it to nothing.
#[test]
fn the_same_method_is_a_permission_where_the_trait_is_imported() {
    let at = workspace(
        "use std::os::unix::fs::OpenOptionsExt;\n\
         pub fn install(path: &std::path::Path) {\n    \
         let _ = std::fs::OpenOptions::new().create(true).write(true).mode(0o755).open(path);\n}",
    );
    let report = gate(at.path());
    assert_eq!(report.applying, 1, "{report:?}");
    assert_eq!(report.findings.len(), 1, "{report:?}");
}

/// A GLOB BRINGS THE TRAIT IN TOO, and a rule that read only the named import
/// would go quiet on the tidier spelling — the shape R1182 warns gets routed
/// around. The glob that does NOT bring it is here beside it, because a rule
/// that read every `fs` glob would be back to calling any `mode` a permission.
#[test]
fn a_glob_brings_the_trait_in_only_where_the_trait_lives() {
    let unix = workspace(
        "use std::os::unix::fs::*;\n\
         pub fn install(path: &std::path::Path) {\n    \
         let _ = std::fs::OpenOptions::new().create(true).write(true).mode(0o755).open(path);\n}",
    );
    let report = gate(unix.path());
    assert_eq!(report.applying, 1, "{report:?}");
    assert_eq!(report.findings.len(), 1, "{report:?}");

    let ordinary = workspace(
        "use std::fs::*;\n\
         pub fn open(path: &std::path::Path) {\n    \
         let _ = sled::Config::default().path(path).mode(0o755);\n}",
    );
    let report = gate(ordinary.path());
    assert_eq!(report.applying, 0, "{report:?}");
    assert!(report.findings.is_empty(), "{report:?}");
}

/// The same reading spelled through the trait, where the getter names
/// `PermissionsExt` and the SETTER names `OpenOptionsExt`. The trait is what
/// separates them, so both are pinned here.
#[test]
fn the_ufcs_mode_getter_is_not_the_ufcs_mode_setter() {
    let reading = workspace(
        "use std::os::unix::fs::PermissionsExt;\n\
         pub fn readable(path: &std::path::Path) -> u32 {\n    \
         let perms = std::fs::metadata(path).unwrap().permissions();\n    \
         PermissionsExt::mode(&perms) & 0o111\n}",
    );
    let report = gate(reading.path());
    assert_eq!(report.applying, 0, "{report:?}");
    assert!(report.findings.is_empty(), "{report:?}");

    let writing = workspace(
        "use std::os::unix::fs::OpenOptionsExt;\n\
         pub fn install(path: &std::path::Path) {\n    \
         let mut options = std::fs::OpenOptions::new();\n    \
         OpenOptionsExt::mode(&mut options, 0o755);\n    \
         let _ = options.create(true).write(true).open(path);\n}",
    );
    let report = gate(writing.path());
    assert_eq!(report.applying, 1, "{report:?}");
    assert_eq!(report.findings.len(), 1, "{report:?}");
}

/// ⚠ `syn` DOES NOT WALK A MACRO'S BODY, and a chmod wrapped in `assert!` is an
/// ordinary way to write one. Round 1186 met the same blindness from the other
/// side; the tokens are read for exactly this.
#[test]
fn a_chmod_inside_a_macro_still_counts() {
    let at = workspace(
        "use std::os::unix::fs::PermissionsExt;\n\
         pub fn install(path: &std::path::Path) {\n    \
         assert!(std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).is_ok());\n}",
    );
    let report = gate(at.path());
    assert_eq!(report.verdict(), Ok(()), "{report:?}");
    assert!(!report.findings.is_empty(), "{report:?}");
    assert!(
        report.findings.iter().all(|f| f.made == Made::Mode(0o755)),
        "{report:?}"
    );
}

/// A COPY CARRIES THE SOURCE'S MODE, so copying a program writes one — with no
/// chmod in the function at all. This is `injection-harness`'s supervisor, and a
/// chmod-only law would have called that file clean the moment its redundant
/// `set_permissions` came off.
#[test]
fn copying_a_program_this_function_names_is_a_finding() {
    let at = workspace(
        "pub fn take_a_copy(to: &std::path::Path) {\n    \
         let me = std::env::current_exe().unwrap();\n    \
         std::fs::copy(&me, to).unwrap();\n}",
    );
    let report = gate(at.path());
    assert_eq!(report.verdict(), Ok(()), "{report:?}");
    assert_eq!(report.copying, 1, "{report:?}");
    assert_eq!(report.findings.len(), 1, "{report:?}");
    assert_eq!(
        report.findings[0].made,
        Made::CopyOf("current_exe()".to_owned()),
        "{report:?}"
    );
}

/// A binary cargo built, named the one way cargo checks at compile time.
#[test]
fn copying_a_cargo_built_binary_is_a_finding() {
    let at = workspace(
        "pub fn take_a_copy(to: &std::path::Path) {\n    \
         std::fs::copy(env!(\"CARGO_BIN_EXE_probe\"), to).unwrap();\n}",
    );
    let report = gate(at.path());
    assert_eq!(report.verdict(), Ok(()), "{report:?}");
    assert_eq!(report.findings.len(), 1, "{report:?}");
    assert_eq!(
        report.findings[0].made,
        Made::CopyOf("CARGO_BIN_EXE_probe".to_owned()),
        "{report:?}"
    );
}

/// ONE HOP, AND IT WAS NOT SPECULATIVE. `injection-harness` held exactly this
/// shape while this gate was being written — `fs::copy(binary(), &tool)` beside
/// a `binary()` returning the cargo path — and the gate read it as an ordinary
/// copy and said nothing. A person found it by reading, which is the thing a
/// gate exists to stop being necessary.
#[test]
fn copying_what_a_helper_in_this_file_names_is_a_finding() {
    let at = workspace(
        "fn binary() -> &'static str {\n    env!(\"CARGO_BIN_EXE_probe\")\n}\n\
         pub fn take_a_copy(to: &std::path::Path) {\n    \
         std::fs::copy(binary(), to).unwrap();\n}",
    );
    let report = gate(at.path());
    assert_eq!(report.verdict(), Ok(()), "{report:?}");
    assert_eq!(report.findings.len(), 1, "{report:?}");
    assert_eq!(
        report.findings[0].made,
        Made::CopyOf("what `binary()` returns".to_owned()),
        "{report:?}"
    );
}

/// And the hop STOPS at a helper whose answer depends on its caller: what
/// `program(which)` returns is not decided here, so this is the census rather
/// than a finding. Stated as a case so the limit is a measurement.
#[test]
fn a_helper_that_takes_an_argument_is_not_followed() {
    let at = workspace(
        "fn program(which: &str) -> String {\n    \
         format!(\"{}-{which}\", env!(\"CARGO_BIN_EXE_probe\"))\n}\n\
         pub fn take_a_copy(to: &std::path::Path) {\n    \
         std::fs::copy(program(\"a\"), to).unwrap();\n}",
    );
    let report = gate(at.path());
    assert_eq!(report.verdict(), Ok(()), "{report:?}");
    assert!(report.findings.is_empty(), "{report:?}");
    assert_eq!(report.unnamed_copies.len(), 1, "{report:?}");
}

/// The control, and the limit stated as a test rather than as prose: an ordinary
/// copy is COUNTED and PRINTED, never failed on. This walk cannot read the mode
/// of a file it is not looking at.
#[test]
fn an_ordinary_copy_is_counted_and_not_judged() {
    let at = workspace(
        "pub fn keep(from: &std::path::Path, to: &std::path::Path) {\n    \
         std::fs::copy(from, to).unwrap();\n}",
    );
    let report = gate(at.path());
    assert_eq!(report.verdict(), Ok(()), "{report:?}");
    assert_eq!(report.copying, 1, "{report:?}");
    assert_eq!(report.unnamed_copies.len(), 1, "{report:?}");
    assert!(report.findings.is_empty(), "{report:?}");
}

/// `std::io::copy` moves bytes between two streams and creates no file. Reading
/// it as `fs::copy` would put a finding on every buffered write in the tree.
#[test]
fn copying_between_streams_is_not_copying_a_file() {
    let at = workspace(
        "pub fn drain(r: &mut dyn std::io::Read, w: &mut dyn std::io::Write) {\n    \
         std::io::copy(r, w).unwrap();\n}",
    );
    let report = gate(at.path());
    assert_eq!(report.verdict(), Ok(()), "{report:?}");
    assert_eq!(report.copying, 0, "{report:?}");
    assert!(report.unnamed_copies.is_empty(), "{report:?}");
}

/// THE STRONGEST THING THE WALK CAN SAY: the expression made executable is the
/// one handed to `Command::new`.
#[test]
fn running_the_very_path_it_made_executable_is_the_strong_evidence() {
    let at = workspace(
        "use std::os::unix::fs::PermissionsExt;\n\
         pub fn install_and_run(stub: &std::path::Path) {\n    \
         std::fs::set_permissions(stub, std::fs::Permissions::from_mode(0o755)).unwrap();\n    \
         std::process::Command::new(stub).status().unwrap();\n}",
    );
    let report = gate(at.path());
    assert_eq!(report.findings.len(), 1, "{report:?}");
    assert_eq!(
        report.findings[0].ran,
        Ran::ThePath("stub".to_owned()),
        "{report:?}"
    );
    assert_eq!(report.evidence(), [1, 0, 0, 0], "{report:?}");
}

/// And the three weaker grades, which is the half of R1182's discipline that
/// keeps a weak basis from reading like a strong one. All four are counted so
/// the report can print them side by side.
#[test]
fn the_weaker_grades_of_run_evidence_are_counted_apart() {
    let elsewhere_in_the_function = workspace(
        "use std::os::unix::fs::PermissionsExt;\n\
         pub fn install(stub: &std::path::Path, other: &std::path::Path) {\n    \
         std::fs::set_permissions(stub, std::fs::Permissions::from_mode(0o755)).unwrap();\n    \
         std::process::Command::new(other).status().unwrap();\n}",
    );
    let report = gate(elsewhere_in_the_function.path());
    assert_eq!(report.findings[0].ran, Ran::SomethingHere, "{report:?}");
    assert_eq!(report.evidence(), [0, 1, 0, 0], "{report:?}");

    let elsewhere_in_the_file = workspace(
        "use std::os::unix::fs::PermissionsExt;\n\
         pub fn install(stub: &std::path::Path) {\n    \
         std::fs::set_permissions(stub, std::fs::Permissions::from_mode(0o755)).unwrap();\n}\n\
         pub fn run(what: &std::path::Path) {\n    \
         std::process::Command::new(what).status().unwrap();\n}",
    );
    let report = gate(elsewhere_in_the_file.path());
    assert_eq!(
        report.findings[0].ran,
        Ran::SomethingInTheFile,
        "{report:?}"
    );
    assert_eq!(report.evidence(), [0, 0, 1, 0], "{report:?}");

    let nowhere = workspace(
        "use std::os::unix::fs::PermissionsExt;\n\
         pub fn install(stub: &std::path::Path) {\n    \
         std::fs::set_permissions(stub, std::fs::Permissions::from_mode(0o755)).unwrap();\n}",
    );
    let report = gate(nowhere.path());
    assert_eq!(report.findings[0].ran, Ran::NothingVisible, "{report:?}");
    assert_eq!(report.evidence(), [0, 0, 0, 1], "{report:?}");
}

/// THE REPAIR PASSES, which is the half of a law that decides whether anyone can
/// obey it. A fixture that symlinks a cargo-built binary into place and spawns
/// it creates no executable at all.
#[test]
fn reaching_a_built_program_by_symlink_is_clean() {
    let at = workspace(
        "pub fn stub_on_the_path(shim: &std::path::Path) {\n    \
         std::os::unix::fs::symlink(env!(\"CARGO_BIN_EXE_probe\"), shim.join(\"gh\")).unwrap();\n    \
         std::process::Command::new(shim.join(\"gh\")).status().unwrap();\n}",
    );
    let report = gate(at.path());
    assert_eq!(report.verdict(), Ok(()), "{report:?}");
    assert_eq!(report.applying, 0, "{report:?}");
    assert_eq!(report.copying, 0, "{report:?}");
    assert!(report.findings.is_empty(), "{report:?}");
}

/// A tree the law has nothing to apply to says so, rather than printing what a
/// clean tree prints.
#[test]
fn a_tree_that_applies_no_mode_is_answered_not_judged() {
    let at = workspace("pub fn add(a: u8, b: u8) -> u8 {\n    a + b\n}");
    let report = gate(at.path());
    assert_eq!(report.verdict(), Ok(()), "{report:?}");
    assert_eq!(report.applying, 0, "{report:?}");
    assert_eq!(report.copying, 0, "{report:?}");
    assert!(report.findings.is_empty(), "{report:?}");
}

/// A file the walk could not parse is a REFUSAL, because a tree read in part is
/// a tree whose clean answer means nothing.
#[test]
fn a_file_that_does_not_parse_refuses_rather_than_counting_nothing() {
    let at = workspace("pub fn broken( {");
    let report = gate(at.path());
    assert!(report.verdict().is_err(), "{report:?}");
    assert!(report.findings.is_empty(), "{report:?}");
}

/// A default method on a trait is a body too, and the walk stops at one — so
/// without its own arm in the judging pass, a chmod written there is collected
/// by neither and dropped in silence.
#[test]
fn a_chmod_in_a_trait_default_method_is_judged() {
    let at = workspace(
        "use std::os::unix::fs::PermissionsExt;\n\
         pub trait Installs {\n    \
         fn install(&self, path: &std::path::Path) {\n        \
         std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();\n    }\n}",
    );
    let report = gate(at.path());
    assert_eq!(report.verdict(), Ok(()), "{report:?}");
    assert_eq!(report.applying, 1, "{report:?}");
    assert_eq!(report.findings.len(), 1, "{report:?}");
    assert_eq!(report.findings[0].owner, "install", "{report:?}");
}

/// A nested workspace is somebody else's tree, checked by pointing this gate at
/// ITS manifest — so its defect is not this workspace's finding, and it is
/// COUNTED rather than passed over.
#[test]
fn a_nested_workspace_is_counted_and_left_to_its_own_manifest() {
    let at = workspace("pub fn nothing() {}");
    let inner = at.path().join("tools/inner");
    std::fs::create_dir_all(inner.join("src")).expect("mkdir inner");
    std::fs::write(
        inner.join("Cargo.toml"),
        "[workspace]\n[package]\nname = \"inner\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write inner manifest");
    std::fs::write(
        inner.join("src/lib.rs"),
        "use std::os::unix::fs::PermissionsExt;\n\
         pub fn install(path: &std::path::Path) {\n    \
         std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();\n}",
    )
    .expect("write inner source");

    let outer = gate(at.path());
    assert!(outer.findings.is_empty(), "{outer:?}");
    assert_eq!(outer.coverage.foreign_workspaces, 1, "{outer:?}");

    // And pointed at its own manifest, the same file IS a finding — which is
    // what makes the skip a delegation rather than a hole.
    let judged = gate(&inner);
    assert_eq!(judged.findings.len(), 1, "{judged:?}");
}

/// The gate runs as a hook runs it, and answers with the exit code the hook
/// branches on: 0 clean, 1 judged and defective, 2 not judged.
#[test]
fn the_binary_answers_a_hook_in_three_codes() {
    let clean = workspace(
        "use std::os::unix::fs::PermissionsExt;\n\
         pub fn install(path: &std::path::Path) {\n    \
         std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o644)).unwrap();\n}",
    );
    let defective = workspace(
        "use std::os::unix::fs::PermissionsExt;\n\
         pub fn install(path: &std::path::Path) {\n    \
         std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();\n}",
    );
    let unreadable = workspace("pub fn broken( {");
    for (at, expected) in [(&clean, 0), (&defective, 1), (&unreadable, 2)] {
        let out = Command::new(env!("CARGO_BIN_EXE_written-executable"))
            .args([
                "--workspace",
                &at.path().join("Cargo.toml").display().to_string(),
            ])
            .output()
            .expect("the gate binary runs");
        assert_eq!(
            out.status.code(),
            Some(expected),
            "stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

/// The unreadable mode reaches the hook as exit 2 as well — "I could not read
/// this" and "these sites break the law" are different answers, and a caller
/// that collapses them tells the wrong person to go looking.
#[test]
fn an_unreadable_mode_reaches_a_hook_as_not_judged() {
    let at = workspace(
        "use std::os::unix::fs::PermissionsExt;\n\
         pub fn install(path: &std::path::Path, mode: u32) {\n    \
         std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();\n}",
    );
    let out = Command::new(env!("CARGO_BIN_EXE_written-executable"))
        .args([
            "--workspace",
            &at.path().join("Cargo.toml").display().to_string(),
        ])
        .output()
        .expect("the gate binary runs");
    assert_eq!(
        out.status.code(),
        Some(2),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("cannot read"),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
