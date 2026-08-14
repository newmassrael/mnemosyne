//! The law, asked of trees this test builds — one per answer the gate can give.
//!
//! Built OUTSIDE this repository, for the reason its siblings give: a fixture
//! carrying its own `[workspace]` inside the tree would be discovered by
//! `scripts/check-side-workspaces.sh`, which would then lint a workspace whose
//! whole purpose is to be rejected.

use std::path::Path;
use std::process::Command;

use tempfile::TempDir;

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

fn gate(at: &Path) -> unowned_scratch::Report {
    unowned_scratch::run(&at.join("Cargo.toml")).expect("the gate runs over the fixture")
}

/// The shape the gate exists for: a fixed name under the machine's temp root.
#[test]
fn a_fixed_name_under_the_shared_root_is_a_finding() {
    let at = workspace(
        "pub fn scratch() -> std::path::PathBuf {\n    \
         std::env::temp_dir().join(\"probe-fixture\")\n}",
    );
    let report = gate(at.path());
    assert_eq!(report.verdict(), Ok(()), "{report:?}");
    assert_eq!(report.reaching, 1, "{report:?}");
    assert_eq!(report.owning, 0, "{report:?}");
    assert_eq!(report.findings.len(), 1, "{report:?}");
    assert_eq!(report.findings[0].owner, "scratch", "{report:?}");
}

/// The control, without which the case above proves only that this gate can say
/// "defect", which every broken gate also does.
#[test]
fn the_same_tree_is_clean_once_the_path_names_the_process() {
    let at = workspace(
        "pub fn scratch() -> std::path::PathBuf {\n    \
         std::env::temp_dir().join(format!(\"probe-{}\", std::process::id()))\n}",
    );
    let report = gate(at.path());
    assert_eq!(report.verdict(), Ok(()), "{report:?}");
    assert_eq!(report.reaching, 1, "{report:?}");
    assert_eq!(report.owning, 1, "{report:?}");
    assert!(report.findings.is_empty(), "{report:?}");
}

/// ⚠ THE PID IS INSIDE A MACRO, AND `syn` DOES NOT WALK ONE. Every one of the
/// eleven sites in this repository spells the name inside `format!`, and the
/// first draft of this gate reported all eleven as defects — a gate whose first
/// run is entirely false alarms. The tokens are read for exactly this.
#[test]
fn the_process_named_only_inside_a_macro_still_counts() {
    let at = workspace(
        "pub fn scratch() -> std::path::PathBuf {\n    \
         let mut path = std::env::temp_dir();\n    \
         path.push(format!(\"probe-{}\", std::process::id()));\n    \
         path\n}",
    );
    let report = gate(at.path());
    assert_eq!(report.verdict(), Ok(()), "{report:?}");
    assert_eq!(report.owning, 1, "{report:?}");
    assert!(report.findings.is_empty(), "{report:?}");
}

/// The second spelling in this repository puts the root and the name on
/// different STATEMENTS, which is why the unit is the function rather than the
/// expression chain.
#[test]
fn a_bound_root_named_on_a_later_statement_is_clean() {
    let at = workspace(
        "pub fn scratch() -> std::path::PathBuf {\n    \
         let mut path = std::env::temp_dir();\n    \
         let owner = std::process::id();\n    \
         path.push(format!(\"probe-{owner}\"));\n    \
         path\n}",
    );
    let report = gate(at.path());
    assert_eq!(report.verdict(), Ok(()), "{report:?}");
    assert!(report.findings.is_empty(), "{report:?}");
}

/// A nested function is judged on ITS OWN body, not on the one that contains
/// it. Without the depth-first order a helper's omission would be covered by
/// its parent's correctness.
#[test]
fn a_nested_function_does_not_inherit_its_parents_naming() {
    let at = workspace(
        "pub fn outer() {\n    \
         let _ = std::env::temp_dir().join(format!(\"outer-{}\", std::process::id()));\n    \
         fn inner() -> std::path::PathBuf {\n        \
         std::env::temp_dir().join(\"inner-fixture\")\n    }\n    \
         let _ = inner();\n}",
    );
    let report = gate(at.path());
    assert_eq!(report.verdict(), Ok(()), "{report:?}");
    assert_eq!(report.findings.len(), 1, "{report:?}");
    assert_eq!(report.findings[0].owner, "inner", "{report:?}");
}

/// `id()` on something else is not the process's. A gate that took any `id` for
/// the pid would pass the defect it exists to catch.
#[test]
fn some_other_id_is_not_the_process() {
    let at = workspace(
        "pub struct Thing;\nimpl Thing { pub fn id(&self) -> u32 { 7 } }\n\
         pub fn scratch(t: &Thing) -> std::path::PathBuf {\n    \
         std::env::temp_dir().join(format!(\"probe-{}\", t.id()))\n}",
    );
    let report = gate(at.path());
    assert_eq!(report.verdict(), Ok(()), "{report:?}");
    assert_eq!(report.findings.len(), 1, "{report:?}");
}

/// A tree the law has nothing to apply to says so, rather than printing what a
/// clean tree prints.
#[test]
fn a_tree_that_never_reaches_the_shared_root_is_answered_not_judged() {
    let at = workspace("pub fn add(a: u8, b: u8) -> u8 {\n    a + b\n}");
    let report = gate(at.path());
    assert_eq!(report.verdict(), Ok(()), "{report:?}");
    assert_eq!(report.reaching, 0, "{report:?}");
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
        "pub fn scratch() -> std::path::PathBuf { std::env::temp_dir().join(\"inner\") }",
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
        "pub fn scratch() -> std::path::PathBuf {\n    \
         std::env::temp_dir().join(format!(\"probe-{}\", std::process::id()))\n}",
    );
    let defective = workspace(
        "pub fn scratch() -> std::path::PathBuf {\n    \
         std::env::temp_dir().join(\"probe-fixture\")\n}",
    );
    let unreadable = workspace("pub fn broken( {");
    for (at, expected) in [(&clean, 0), (&defective, 1), (&unreadable, 2)] {
        let out = Command::new(env!("CARGO_BIN_EXE_unowned-scratch"))
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
