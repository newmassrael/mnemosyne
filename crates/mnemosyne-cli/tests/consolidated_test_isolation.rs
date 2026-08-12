//! A test that owns a process-global does not share a binary.
//!
//! Round 1148 folded `mnemosyne-server`'s twenty-two integration test files
//! into one target, because each file is its own crate and its own LINK of the
//! RocksDB + tonic graph. The check before that move asked whether any test
//! called `set_current_dir`, `env::set_var` or `env::remove_var` — none did —
//! and MISSED THE ONE THAT MATTERED.
//!
//! `tracing`'s callsite interest cache is process-global even though
//! `subscriber::with_default` is thread-local. A test capturing spans can
//! therefore miss a callsite while another thread in the same process installs
//! a dispatcher. `handler_span_hierarchy_smoke` failed exactly that way, and
//! the numbers are the point: under CPU contention it failed **2 of 30** runs
//! inside the shared binary and **0 of 30** with its own process, then **0 of
//! 100** after being given one back. The four-thread run that had passed
//! before was luck — 0.93^30 is 11%, which is why thirty greens were not an
//! argument and a hundred are.
//!
//! So the law: a file included in a consolidated test target must not touch
//! anything the PROCESS owns. The list below is what this repository has
//! actually been bitten by or can name a mechanism for; it is meant to grow
//! when the next one is found, which is the honest shape for a denylist.
//!
//! WHY A DENYLIST AND NOT A CAPABILITY CHECK. There is no way to ask the
//! compiler "does this test mutate process state" — the property is about what
//! the code reaches at runtime, through dependencies this crate does not see.
//! A named list of constructs, each with a reason, is a claim a reader can
//! check; a clever heuristic is not.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Constructs that make a test the owner of something the whole process shares.
/// Each is paired with what it owns, so a failure says WHY rather than only
/// WHICH.
const PROCESS_GLOBALS: &[(&str, &str)] = &[
    (
        "with_default",
        "installs a tracing dispatcher; the callsite interest cache behind it is \
         process-global (Round 1148 measured 2 failures in 30 runs from this)",
    ),
    (
        "set_global_default",
        "installs the process-wide tracing dispatcher, once, for everyone",
    ),
    (
        "set_current_dir",
        "moves the working directory out from under every other test",
    ),
    (
        "env::set_var",
        "the environment is one table shared by the whole process",
    ),
    (
        "env::remove_var",
        "the environment is one table shared by the whole process",
    ),
    ("global_allocator", "there is one allocator per process"),
];

/// The workspace root, from this test's own location rather than from the
/// working directory a runner happens to use.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name>/ sits two below the workspace root")
        .to_path_buf()
}

/// Every consolidated test target in the workspace: a `tests/all.rs` under any
/// crate. Derived by walking rather than listed here, so a second crate that
/// consolidates is covered the day it does.
fn consolidated_targets(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for parent in ["crates", "tools"] {
        let Ok(entries) = fs::read_dir(root.join(parent)) else {
            continue;
        };
        for entry in entries.flatten() {
            let candidate = entry.path().join("tests/all.rs");
            if candidate.is_file() {
                found.push(candidate);
            }
        }
    }
    found.sort();
    found
}

/// The files a consolidated target pulls in, read off its own `#[path = "…"]`
/// declarations. Not a directory listing: a file sitting beside `all.rs` and
/// NOT declared is exactly the isolated case this law permits.
fn included_files(target: &Path) -> Vec<PathBuf> {
    let source = fs::read_to_string(target).expect("consolidated target is readable");
    let dir = target.parent().expect("a file has a parent");
    source
        .lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("#[path = \"")?;
            let name = rest.strip_suffix("\"]")?;
            Some(dir.join(name))
        })
        .collect()
}

/// Markers that mean "this function installs the process-wide dispatcher".
/// `.init()` and `.try_init()` are `tracing_subscriber`'s spelling of it.
const GLOBAL_INSTALL_MARKERS: &[&str] = &["set_global_default", ".try_init()", ".init()"];

/// The functions in a consolidated target's OWN crate whose bodies install a
/// process-global, named so a test file that merely CALLS one is caught.
///
/// This is the repair for the hole Round 1150 fell through: the denylist reads
/// the test file, and `grpc_otlp_smoke.rs` says only
/// `init_otlp_tracing_subscriber(...)` while the `.try_init()` sits one hop away
/// in `src/grpc.rs`. Derived from the source rather than listed here, so the
/// next wrapper is covered the day it is written — the same reason the target
/// walk above is a walk and not a list.
///
/// Textual and one hop deep, which is what a test can do without a compiler.
/// A wrapper that calls a wrapper is not followed; that is a known limit rather
/// than a claim of completeness.
fn global_installing_fns(target: &Path) -> Vec<String> {
    let Some(src) = target
        .parent()
        .and_then(Path::parent)
        .map(|c| c.join("src"))
    else {
        return Vec::new();
    };
    let mut names = Vec::new();
    let mut stack = vec![src];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            let Ok(body) = fs::read_to_string(&path) else {
                continue;
            };
            // Walk the file once, remembering the most recent `pub fn` name; when
            // an install marker appears, that function is the installer.
            let mut current: Option<String> = None;
            for line in body.lines() {
                let trimmed = line.trim_start();
                if let Some(rest) = trimmed.strip_prefix("pub fn ") {
                    current = rest
                        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                        .next()
                        .filter(|n| !n.is_empty())
                        .map(str::to_string);
                }
                if GLOBAL_INSTALL_MARKERS.iter().any(|m| trimmed.contains(m)) {
                    if let Some(name) = current.take() {
                        names.push(name);
                    }
                }
            }
        }
    }
    names.sort();
    names.dedup();
    names
}

#[test]
fn a_test_that_owns_a_process_global_does_not_share_a_binary() {
    let root = workspace_root();
    let targets = consolidated_targets(&root);

    // NON-VACUITY: this law is about consolidated targets, so it must have
    // found one. Rename `all.rs` and this test says so rather than passing.
    assert!(
        !targets.is_empty(),
        "no consolidated test target found under {}/{{crates,tools}}/*/tests/all.rs — \
         either none exists (delete this test) or the convention moved (fix the walk)",
        root.display()
    );

    let mut scanned = 0usize;
    let mut violations: Vec<String> = Vec::new();
    for target in &targets {
        let included = included_files(target);
        assert!(
            !included.is_empty(),
            "{} declares no `#[path]` module — a consolidated target that includes \
             nothing is either a typo or a leftover",
            target.display()
        );
        // The crate's OWN wrappers, derived rather than listed. A test file that
        // says `init_otlp_tracing_subscriber(...)` contains none of the markers
        // below, and the install happens one hop away in `src/`. Round 1150
        // shipped exactly that and CI caught it.
        let wrappers = global_installing_fns(target);
        for file in included {
            let body = fs::read_to_string(&file)
                .unwrap_or_else(|e| panic!("{} is declared but unreadable: {e}", file.display()));
            scanned += 1;
            for wrapper in &wrappers {
                if body.contains(wrapper.as_str()) {
                    violations.push(format!(
                        "{} calls `{}`, which installs a process-global dispatcher in this \
                         crate's own src. Give it its own [[test]] target instead of \
                         including it in {}",
                        file.strip_prefix(&root).unwrap_or(&file).display(),
                        wrapper,
                        target.strip_prefix(&root).unwrap_or(target).display()
                    ));
                }
            }
            for (marker, why) in PROCESS_GLOBALS {
                if body.contains(marker) {
                    violations.push(format!(
                        "{} uses `{}` — {}. Give it its own [[test]] target instead of \
                         including it in {}",
                        file.strip_prefix(&root).unwrap_or(&file).display(),
                        marker,
                        why,
                        target.strip_prefix(&root).unwrap_or(target).display()
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "{} test file(s) share a binary while owning something the process owns:\n  {}",
        violations.len(),
        violations.join("\n  ")
    );

    // NON-VACUITY, AND A LAW OF ITS OWN: every `tests/*.rs` in the crate is
    // either included here or declared as its own `[[test]]`. Under
    // `autotests = false` a file that is neither is a test NOBODY RUNS, and it
    // reads exactly like a passing one.
    //
    // This replaces a hardcoded `scanned >= 20`, which was wrong the moment four
    // files legitimately moved out — the hand-written number this repository
    // keeps catching elsewhere, written by this gate's own author. Derived, it
    // also catches the `#[path]` parse silently matching nothing, which is what
    // that constant was reaching for.
    for target in &targets {
        let dir = target.parent().expect("a file has a parent");
        let crate_dir = dir.parent().expect("tests/ sits in the crate");
        let manifest = fs::read_to_string(crate_dir.join("Cargo.toml"))
            .unwrap_or_else(|e| panic!("{}: {e}", crate_dir.display()));
        let included: BTreeSet<String> = included_files(target)
            .iter()
            .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .collect();
        let mut orphans = Vec::new();
        for entry in fs::read_dir(dir).expect("tests/ is readable").flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            let name = path
                .file_name()
                .expect("a file has a name")
                .to_string_lossy()
                .into_owned();
            if name == "all.rs" || included.contains(&name) {
                continue;
            }
            if !manifest.contains(&format!("tests/{name}")) {
                orphans.push(name);
            }
        }
        orphans.sort();
        assert!(
            orphans.is_empty(),
            "{} has `autotests = false`, so {} test file(s) are run by NOBODY — they are \
             neither included in {} nor declared as their own [[test]]: {orphans:?}",
            crate_dir.display(),
            orphans.len(),
            target.display()
        );
        assert!(
            !included.is_empty(),
            "{} included nothing — the `#[path]` parse matched no file",
            target.display()
        );
    }
    let _ = scanned;
}

/// The concrete case the law was written from, pinned by name.
///
/// Without this, the law above is satisfied by a consolidated target that
/// happens to include nothing dangerous today, and the specific repair Round
/// 1148 measured could be undone by moving one line.
#[test]
fn the_span_capturing_server_test_is_not_in_the_shared_binary() {
    let root = workspace_root();
    let target = root.join("crates/mnemosyne-server/tests/all.rs");
    if !target.is_file() {
        return; // the consolidation was reverted; the law above still holds.
    }
    let included = included_files(&target);
    assert!(
        included
            .iter()
            .all(|f| !f.ends_with("handler_span_hierarchy_smoke.rs")),
        "handler_span_hierarchy_smoke captures spans and must keep its own \
         process — it failed 2 of 30 runs under contention when it did not"
    );
    // And it must still be a test target at all, rather than dropped.
    let manifest = fs::read_to_string(root.join("crates/mnemosyne-server/Cargo.toml"))
        .expect("server manifest is readable");
    assert!(
        manifest.contains("tests/handler_span_hierarchy_smoke.rs"),
        "the file was removed from the shared binary without being given its own \
         [[test]] entry, so `autotests = false` means nothing runs it"
    );
}
