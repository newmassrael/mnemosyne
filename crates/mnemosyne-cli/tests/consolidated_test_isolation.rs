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
//!
//! BOTH DIRECTIONS, since Round 1172. That round folded `mnemosyne-cli`'s
//! seventy-six test binaries into one — the pre-commit item-citation gate, which
//! documents every target `cargo metadata` lists, went from 7m21s to 21s, having
//! already killed Round 1171's commit at a ten-minute timeout. A law that only
//! says which files may not SHARE says nothing about the pile growing back one
//! justified-sounding `[[test]]` at a time, so two more laws are here:
//! `a_separate_test_target_is_one_the_process_or_ci_forces` (an exception is
//! earned by the process or by a CI command that names it, never by preference)
//! and `a_test_target_a_ci_command_names_is_one_that_exists` (the failure
//! consolidation makes possible — a workflow selecting by name a target that has
//! just become a module).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use syn::visit::Visit;

/// Constructs that make a test the owner of something the whole process shares.
/// Each is paired with what it owns, so a failure says WHY rather than only
/// WHICH.
///
/// A name with `::` in it is a PATH SUFFIX (`env::set_var` matches
/// `std::env::set_var` and not `mine::set_var`); a bare one is an identifier.
/// Both are matched against the SYNTAX of a file rather than its text, so a
/// construct named in a comment — or written as a string literal in this very
/// table — is not a use of it.
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

/// Every package this repository tracks a manifest for, as its directory.
///
/// Asked of `ci_plan::tracked_manifests` — the same list the CI gates resolve
/// their commands against — rather than of two directory names. A crate that
/// lives anywhere else (`bench/`, `studio/`, a future `apps/`) joins the
/// population the day its manifest is tracked, which is the property a
/// hardcoded pair of parents cannot have.
fn package_dirs(root: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = ci_plan::tracked_manifests(root)
        .iter()
        .filter_map(|manifest| root.join(manifest).parent().map(Path::to_path_buf))
        .collect();
    found.sort();
    found.dedup();
    found
}

/// Every consolidated test target in the workspace: a `tests/all.rs` under any
/// tracked package. Derived by walking rather than listed here, so a second
/// crate that consolidates is covered the day it does.
fn consolidated_targets(root: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = package_dirs(root)
        .into_iter()
        .map(|dir| dir.join("tests/all.rs"))
        .filter(|candidate| candidate.is_file())
        .collect();
    found.sort();
    found
}

/// One `[[test]]` a manifest declares by hand.
struct DeclaredTest {
    /// The target name — what `cargo test --test <name>` selects.
    name: String,
    /// The file it points at, absolute.
    path: PathBuf,
}

/// What a package's manifest says about its test targets: the `[[test]]`
/// entries it declares, and whether it ALSO lets cargo discover `tests/*.rs`.
///
/// Parsed rather than searched for. The check this replaces asked whether the
/// manifest TEXT contained `tests/<name>.rs`, which a comment mentioning the
/// file satisfies just as well as a declaration — and under `autotests = false`
/// the difference between those two is whether anything runs the tests in it.
fn declared_tests(crate_dir: &Path) -> (Vec<DeclaredTest>, bool) {
    let manifest = crate_dir.join("Cargo.toml");
    let raw = fs::read_to_string(&manifest)
        .unwrap_or_else(|e| panic!("{} is unreadable: {e}", manifest.display()));
    let parsed: toml::Value = toml::from_str(&raw)
        .unwrap_or_else(|e| panic!("{} does not parse as TOML: {e}", manifest.display()));
    let autotests = parsed
        .get("package")
        .and_then(|package| package.get("autotests"))
        .and_then(toml::Value::as_bool)
        .unwrap_or(true);
    let declared = parsed
        .get("test")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .map(|entry| {
            let name = entry
                .get("name")
                .and_then(toml::Value::as_str)
                .unwrap_or_else(|| {
                    panic!("{} declares a [[test]] with no name", manifest.display())
                });
            let path = entry
                .get("path")
                .and_then(toml::Value::as_str)
                .unwrap_or_else(|| {
                    panic!(
                        "{} declares [[test]] `{name}` with no path; this walk will not \
                         guess where cargo would look for it",
                        manifest.display()
                    )
                });
            DeclaredTest {
                name: name.to_string(),
                path: crate_dir.join(path),
            }
        })
        .collect();
    (declared, autotests)
}

/// Why a file may not share a binary — one clause per reason, empty when it may.
///
/// THE one predicate, because two laws read it in opposite directions: the
/// first rejects a file that shares a binary while owning a process-global, and
/// the second ACCEPTS a separate target whose file does. Two copies is how the
/// two would come to disagree — and a file could then be refused by one law and
/// called an unearned exception by the other at the same time.
fn owns_a_process_global(file: &Mentions, wrappers: &[String]) -> Vec<String> {
    let mut reasons = Vec::new();
    for wrapper in wrappers {
        if file.names(wrapper) {
            reasons.push(format!(
                "it calls `{wrapper}`, which installs a process-global dispatcher \
                 in this crate's own src"
            ));
        }
    }
    for (marker, why) in PROCESS_GLOBALS {
        if file.names(marker) {
            reasons.push(format!("it uses `{marker}` — {why}"));
        }
    }
    reasons
}

/// Every test target a CI command NAMES with `--test`, and where each is named.
///
/// Both readers, because a repository issues cargo two ways: the workflows and
/// the tracked scripts (the hooks among them). A target named in either exists
/// so it can be built ALONE, which is the second of the two reasons a file may
/// keep a target of its own.
fn ci_named_test_targets(root: &Path) -> BTreeMap<String, Vec<String>> {
    let mut named: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for command in ci_plan::workflow_cargo_commands(root)
        .into_iter()
        .chain(ci_plan::script_cargo_commands(root))
    {
        for target in command.values(&["--test"]) {
            named
                .entry(target.to_string())
                .or_default()
                .push(command.origin());
        }
    }
    named
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

/// What one file NAMES, read off its syntax.
///
/// Round 1172 replaced the substring search this walk used to be, and the
/// reason is written three lines up in `PROCESS_GLOBALS`: that table spells
/// `env::set_var` and `set_current_dir` as string literals, so the file holding
/// it matched every marker it hunts and could never be judged by its own law.
/// A parser sees a literal as a literal, a comment as nothing at all, and a
/// path as its segments — which is also what makes "`env::set_var`, not
/// `mine::set_var`" a question that can be asked.
#[derive(Default)]
struct Mentions {
    /// Every identifier the file uses, anywhere.
    idents: BTreeSet<String>,
    /// Every path it writes, segments joined by `::`.
    paths: BTreeSet<String>,
    /// The function bodies currently being visited, innermost last.
    within: Vec<String>,
    /// Per function, the identifiers ITS body names — what the call chain below
    /// walks.
    by_fn: BTreeMap<String, BTreeSet<String>>,
    /// The functions whose own body performs the process-wide install.
    installs: BTreeSet<String>,
}

impl Mentions {
    /// Does this file name `marker`? A marker with `::` is a path SUFFIX; a
    /// bare one is an identifier.
    fn names(&self, marker: &str) -> bool {
        match marker.contains("::") {
            false => self.idents.contains(marker),
            true => self
                .paths
                .iter()
                .any(|path| path == marker || path.ends_with(&format!("::{marker}"))),
        }
    }

    fn note(&mut self, ident: &str) {
        self.idents.insert(ident.to_string());
        if let Some(current) = self.within.last() {
            self.by_fn
                .entry(current.clone())
                .or_default()
                .insert(ident.to_string());
        }
    }

    fn installed_here(&mut self) {
        if let Some(current) = self.within.last() {
            self.installs.insert(current.clone());
        }
    }
}

impl<'ast> Visit<'ast> for Mentions {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        self.within.push(node.sig.ident.to_string());
        syn::visit::visit_item_fn(self, node);
        self.within.pop();
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        self.within.push(node.sig.ident.to_string());
        syn::visit::visit_impl_item_fn(self, node);
        self.within.pop();
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        // `.init()` / `.try_init()` — `tracing_subscriber`'s spelling of the
        // process-wide install. THE PARENS ARE PART OF IT: a method of the same
        // name taking arguments is a different call, and the receiver-less
        // reading is what the textual marker `".init()"` was reaching for.
        let method = node.method.to_string();
        if node.args.is_empty() && (method == "init" || method == "try_init") {
            self.installed_here();
        }
        syn::visit::visit_expr_method_call(self, node);
    }

    fn visit_path(&mut self, node: &'ast syn::Path) {
        let spelled: Vec<String> = node
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect();
        if spelled
            .iter()
            .any(|segment| segment == "set_global_default")
        {
            self.installed_here();
        }
        self.paths.insert(spelled.join("::"));
        syn::visit::visit_path(self, node);
    }

    fn visit_ident(&mut self, node: &'ast syn::Ident) {
        let name = node.to_string();
        self.note(&name);
    }
}

/// One file, read as syntax.
///
/// A file that does not parse STOPS the law rather than reading as clean: this
/// walk is over files the workspace compiles, so a parse failure is the gate
/// breaking, and "no markers found" is exactly what breaking looks like.
fn mentions_of(path: &Path) -> Mentions {
    let text = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("{} is unreadable: {e}", path.display()));
    let parsed = syn::parse_file(&text).unwrap_or_else(|e| {
        panic!(
            "{} does not parse, so this law cannot judge it: {e}",
            path.display()
        )
    });
    let mut found = Mentions::default();
    found.visit_file(&parsed);
    found
}

/// The functions in a consolidated target's OWN crate that install a
/// process-global — directly, or by calling something that does, to a FIXED
/// POINT. Named so a test file that merely calls one is caught.
///
/// This is the repair for the hole Round 1150 fell through: the denylist reads
/// the test file, and `grpc_otlp_smoke.rs` says only
/// `init_otlp_tracing_subscriber(...)` while the `.try_init()` sits away in
/// `src/grpc.rs`. Derived from the source rather than listed here, so the next
/// wrapper is covered the day it is written.
///
/// ONE HOP WAS NOT ENOUGH, and Round 1172 has the case rather than the worry.
/// `init_otlp_tracing_subscriber` installs nothing itself — its whole body is
/// `init_otlp_tracing_subscriber_with_config(…)`, one further hop — so the
/// one-hop reading returned only the second name, and the two test files that
/// call the first were invisible to the law that exists for them. The limit had
/// been written down as known for four rounds; what gave it a case was asking
/// the OTHER direction (which separate targets are earned), where those two came
/// back unjustified.
///
/// The chain follows PRIVATE hops as well as public ones — a wrapper whose
/// middle link is private is the same defect one step further out of sight.
fn global_installing_fns(target: &Path) -> Vec<String> {
    let Some(src) = target
        .parent()
        .and_then(Path::parent)
        .map(|c| c.join("src"))
    else {
        return Vec::new();
    };
    let mut by_fn: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut installs: BTreeSet<String> = BTreeSet::new();
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
            let found = mentions_of(&path);
            installs.extend(found.installs);
            for (name, named) in found.by_fn {
                by_fn.entry(name).or_default().extend(named);
            }
        }
    }
    // THE FIXED POINT: a function whose body names an installer is one.
    loop {
        let reached: Vec<String> = by_fn
            .iter()
            .filter(|(name, _)| !installs.contains(name.as_str()))
            .filter(|(name, named)| {
                installs
                    .iter()
                    .any(|installer| installer != *name && named.contains(installer))
            })
            .map(|(name, _)| name.clone())
            .collect();
        if reached.is_empty() {
            break;
        }
        installs.extend(reached);
    }
    installs.into_iter().collect()
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
            let body = mentions_of(&file);
            scanned += 1;
            for reason in owns_a_process_global(&body, &wrappers) {
                violations.push(format!(
                    "{} shares a binary and {reason}. Give it its own [[test]] target \
                     instead of including it in {}",
                    file.strip_prefix(&root).unwrap_or(&file).display(),
                    target.strip_prefix(&root).unwrap_or(target).display()
                ));
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
        let (declared, autotests) = declared_tests(crate_dir);
        // A CONSOLIDATED CRATE HAS AUTO-DISCOVERY OFF, and the two halves of
        // that are what make the rest of this law mean anything: with it on,
        // every file here is built twice — once inside the shared binary and
        // once on its own — which is the cost the consolidation removes, and
        // the orphan check below would be judging a question cargo does not ask.
        assert!(
            !autotests,
            "{} holds a consolidated {} while its manifest still lets cargo \
             discover tests/*.rs — every included file is being built twice",
            crate_dir.display(),
            target.display()
        );
        // NON-VACUITY, per crate: the manifest must declare the consolidated
        // target itself. Under `autotests = false` an undeclared `all.rs` is a
        // binary NOBODY BUILDS, and every walk below would then be reading an
        // empty population as a clean one.
        assert!(
            declared.iter().any(|test| test.path == *target),
            "{} exists but {}'s manifest declares no [[test]] pointing at it — \
             with auto-discovery off, nothing builds or runs it",
            target.display(),
            crate_dir.display()
        );
        let declared_files: BTreeSet<String> = declared
            .iter()
            .filter_map(|test| {
                test.path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
            })
            .collect();
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
            if included.contains(&name) || declared_files.contains(&name) {
                continue;
            }
            orphans.push(name);
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

/// A test target of its own is one the PROCESS or CI forces — never a
/// preference.
///
/// The law above says which files may not SHARE a binary. It says nothing about
/// the other direction, and that direction is how the pile grows back: a file
/// declared as its own `[[test]]` for no reason costs a link and a rustdoc unit
/// on every commit, and reads exactly like one that had to be. Round 1172
/// folded seventy-six of this crate's test binaries into one and measured what
/// they cost — the pre-commit item-citation gate went from 7m21s to 21s, having
/// killed Round 1171's commit at a ten-minute timeout — so the question this law
/// asks is the one that keeps that measurement true.
///
/// TWO REASONS, and they are the only two:
///
///   1. The PROCESS forces it — [`owns_a_process_global`], the same predicate
///      the law above rejects an included file with. What one law refuses to
///      let share is what the other accepts as a target of its own.
///   2. CI NAMES it with `--test`, which makes it a target that exists to be
///      built alone. Folding it in would make that command build every other
///      file in the crate to run one.
#[test]
fn a_separate_test_target_is_one_the_process_or_ci_forces() {
    let root = workspace_root();
    let targets = consolidated_targets(&root);
    assert!(
        !targets.is_empty(),
        "no consolidated test target found under {} — either none exists \
         (delete this test) or the convention moved (fix the walk)",
        root.display()
    );
    let named_by_ci = ci_named_test_targets(&root);

    let mut exceptions = 0usize;
    let mut unearned: Vec<String> = Vec::new();
    let mut earned: Vec<String> = Vec::new();
    for target in &targets {
        let crate_dir = target
            .parent()
            .and_then(Path::parent)
            .expect("tests/ sits in the crate");
        let wrappers = global_installing_fns(target);
        let (declared, _) = declared_tests(crate_dir);
        for test in declared.iter().filter(|test| test.path != *target) {
            exceptions += 1;
            let body = mentions_of(&test.path);
            let shown = test
                .path
                .strip_prefix(&root)
                .unwrap_or(&test.path)
                .display();
            if let Some(reason) = owns_a_process_global(&body, &wrappers).first() {
                earned.push(format!("{shown} — the process forces it: {reason}"));
                continue;
            }
            if let Some(origins) = named_by_ci.get(&test.name) {
                earned.push(format!(
                    "{shown} — named by {} with `--test {}`",
                    origins.join(", "),
                    test.name
                ));
                continue;
            }
            unearned.push(format!(
                "{shown} is its own [[test]] target `{}`, and neither reason holds: \
                 it touches nothing the process owns, and no CI command names it \
                 with `--test`. Include it in {} instead",
                test.name,
                target.strip_prefix(&root).unwrap_or(target).display()
            ));
        }
    }
    for reason in &earned {
        println!("[separate target] {reason}");
    }
    // NON-VACUITY is per-crate and lives in the law above (a consolidated
    // `all.rs` must be a declared target, or the parse read nothing). Here the
    // honest statement is the COUNT: a repository with no exceptions left is a
    // repository this law has nothing to say about, which is a success rather
    // than a reason to fail.
    println!(
        "[separate target] {exceptions} exception(s), {} earned",
        earned.len()
    );
    assert!(
        unearned.is_empty(),
        "{} test target(s) of their own that nothing forces:\n  {}",
        unearned.len(),
        unearned.join("\n  ")
    );
}

/// A `--test <name>` a CI command writes names a target that EXISTS.
///
/// This is the failure consolidation makes possible and nothing else catches:
/// fold a file into the shared binary while a workflow still selects it by
/// name, and `cargo test --test <name>` fails with "no test target named" — in
/// a scheduled job that may not run for a week. The job is red for a reason
/// that has nothing to do with the code it was testing.
///
/// The population is every `--test` the workflows and tracked scripts write;
/// the oracle is what the named package's manifest declares, plus what cargo
/// would auto-discover where auto-discovery is still on.
#[test]
fn a_test_target_a_ci_command_names_is_one_that_exists() {
    let root = workspace_root();
    let named = ci_named_test_targets(&root);
    assert!(
        !named.is_empty(),
        "no CI command in this repository selects a test target with `--test` — \
         either none does any more (delete this test) or the reader stopped \
         seeing them (fix the walk)"
    );
    let dirs = package_dirs(&root);
    let mut missing: Vec<String> = Vec::new();
    for (name, origins) in &named {
        let exists = dirs.iter().any(|dir| {
            let (declared, autotests) = declared_tests(dir);
            declared.iter().any(|test| test.name == *name)
                || (autotests
                    && (dir.join(format!("tests/{name}.rs")).is_file()
                        || dir.join(format!("tests/{name}/main.rs")).is_file()))
        });
        if !exists {
            missing.push(format!(
                "`--test {name}` in {} names no test target in this repository",
                origins.join(", ")
            ));
        }
    }
    // The COUNTS, not a verdict: a line that says "all resolved" printed before
    // the assertion below has decided is a report claiming an answer it does
    // not have yet, which is the shape this repository keeps finding in its own
    // gates.
    println!(
        "[ci-named target] {} name(s) selected by CI, {} of them unresolved",
        named.len(),
        missing.len()
    );
    assert!(
        missing.is_empty(),
        "{} CI command(s) select a test target that does not exist:\n  {}",
        missing.len(),
        missing.join("\n  ")
    );
}
