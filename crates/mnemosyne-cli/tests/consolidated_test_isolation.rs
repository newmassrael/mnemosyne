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
        for file in included {
            let body = fs::read_to_string(&file)
                .unwrap_or_else(|e| panic!("{} is declared but unreadable: {e}", file.display()));
            scanned += 1;
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

    // The other half of non-vacuity: the scan reached real files. A `#[path]`
    // list that parsed to nothing would satisfy the assertion above in silence.
    assert!(
        scanned >= 20,
        "only {scanned} included file(s) were scanned across {} target(s); the \
         `#[path]` parse is probably not matching the file's actual spelling",
        targets.len()
    );
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
