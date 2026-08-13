//! Every environment variable a spawned program reads is one the test that
//! spawns it mentions.
//!
//! WHAT THIS WALKS, and why each half is asked of a different thing.
//!
//! A test that runs a binary is running a second program, and that program does
//! not inherit the test's assumptions — it inherits the MACHINE's environment.
//! So the gate holds two sets against each other:
//!
//!   * what the program READS: every `std::env::var` / `var_os` call in the
//!     binary target, its own package's library, and every LOCAL package those
//!     depend on. Cargo's dependency graph decides that set, not the directory
//!     layout, because a path dependency can live anywhere.
//!   * what the test MENTIONS: every environment name that appears anywhere in
//!     the test target's own sources.
//!
//! MENTION, RATHER THAN CONTROL, and the weaker word is deliberate. What the
//! law wants is that the test SETS or REMOVES the variable, and the first draft
//! of this gate asked exactly that — the arguments of `.env(..)` and
//! `.env_remove(..)`. Measured against this repository, that question could not
//! be answered for the very fixture it was written for: R1181 had just moved
//! `cache-budget`'s environment into a list the spawn loops over, so the
//! arguments are a loop variable and no walk can read them. A gate that refuses
//! the tidiest version of the shape it is enforcing is one people route around.
//!
//! Mention is a NECESSARY condition for control, it is checkable no matter how
//! the fixture is factored, and its failure is never a false alarm: a test whose
//! sources never say `GITHUB_REF_NAME` is certainly not deciding what
//! `GITHUB_REF_NAME` is. It is also weaker than the law in words, so the report
//! prints both numbers — mentioned, and of those, named at an `.env` /
//! `.env_remove` call site — rather than letting the weaker basis pass for the
//! stronger one. R1181's defect fails this test: `GITHUB_REF_NAME` appeared
//! nowhere in that fixture.
//!
//! WHICH TEST SPAWNS WHICH BINARY is read from `env!("CARGO_BIN_EXE_<name>")`,
//! the one spelling cargo checks at compile time. A test that finds its binary
//! some other way is invisible here, and the report says so rather than letting
//! the silence read as coverage.
//!
//! A NAME IS NOT ALWAYS A LITERAL. Constants are resolved from the local
//! sources, and a read whose name is a PARAMETER resolves through the call
//! sites of the function that takes it — `required("MNEMOSYNE_CACHE_HIT")` is a
//! read of that variable however many hops it takes to get there. A name that
//! still does not resolve is a REFUSAL, not a zero, because "I could not read
//! this one" and "there is nothing here" must never arrive as the same answer.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

/// One variable a spawned program reads and its test never mentions.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Finding {
    /// The test target that spawns the program.
    pub test_target: String,
    /// The binary it spawns, by the name `CARGO_BIN_EXE_` gave.
    pub binary: String,
    /// The variable the program reads.
    pub variable: String,
    /// Where the program reads it.
    pub read_in: PathBuf,
    pub line: usize,
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "test `{}` spawns `{}`, which reads `{}`, and never mentions it",
            self.test_target, self.binary, self.variable
        )
    }
}

/// A name the walk found and could not turn into a variable.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Unresolved {
    /// How the call site spells it.
    pub spelled: String,
    pub file: PathBuf,
    pub line: usize,
}

/// What the walk opened, and what it did not. Every `.rs` file under the
/// workspace root lands in exactly one of these counters: a file in none of
/// them is a file this gate silently skipped, which is the failure mode it
/// exists to make impossible.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Coverage {
    /// Files parsed as part of some target's source.
    pub scanned: BTreeSet<PathBuf>,
    /// Files no target's `mod` chain reaches. Cargo does not compile them, so
    /// neither program nor test can read an environment through them.
    pub unreached: BTreeSet<PathBuf>,
    /// Files under a nested directory that declares its own `[workspace]`,
    /// checked by pointing this gate at THAT manifest.
    pub foreign_workspaces: BTreeSet<PathBuf>,
    /// Files under a `target/` directory — cargo's output, not source.
    pub build_artifacts: usize,
    /// Files that failed to parse. Any of these is a refusal.
    pub unparsed: Vec<(PathBuf, String)>,
}

/// Counts the gate states about itself, so a zero is a measurement rather than
/// an absence.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reach {
    /// Binary targets in this workspace.
    pub binaries: usize,
    /// Test and bench targets in this workspace.
    pub test_targets: usize,
    /// Of those, the ones that spawn a binary of this workspace.
    pub spawning_targets: usize,
    /// (binary, variable) pairs the programs read — the denominator the
    /// findings are a numerator of.
    pub reads: usize,
    /// (spawning target, variable) pairs a test mentions at all.
    pub mentioned: usize,
    /// Of those, the ones a test also SETS or REMOVES at a call site this walk
    /// can read — the stronger claim, reported beside the weaker one it is a
    /// subset of.
    pub named: usize,
    /// Spawning targets that name the whole environment with `env_clear`.
    pub clearing_targets: usize,
}

/// The gate's answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub workspace_root: PathBuf,
    pub coverage: Coverage,
    pub reach: Reach,
    pub findings: Vec<Finding>,
    pub unresolved: Vec<Unresolved>,
    /// Every variable each binary reads, for a report that can say what it held
    /// rather than only what it rejected.
    pub read_by: BTreeMap<String, BTreeSet<String>>,
}

impl Report {
    /// Whether the gate read enough to have an opinion at all.
    pub fn verdict(&self) -> Result<(), Refusal> {
        if self.coverage.scanned.is_empty() {
            return Err(Refusal::NothingScanned);
        }
        if !self.coverage.unparsed.is_empty() {
            return Err(Refusal::Unparsed(self.coverage.unparsed.clone()));
        }
        if !self.unresolved.is_empty() {
            return Err(Refusal::Unresolved(self.unresolved.clone()));
        }
        Ok(())
    }

    /// Whether this workspace holds the shape the law is about at all.
    pub fn found_a_spawn(&self) -> bool {
        self.reach.spawning_targets > 0
    }
}

/// Why the gate declined to give a verdict.
///
/// Separate from "defects found", because the two demand different actions and
/// look identical in an exit code alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The walk opened no Rust at all.
    NothingScanned,
    /// A file could not be parsed, so part of the tree is unread.
    Unparsed(Vec<(PathBuf, String)>),
    /// A variable name the walk could not turn into a variable. Counting it as
    /// no read is how a stale list survives the gate written to catch it.
    Unresolved(Vec<Unresolved>),
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::NothingScanned => f.write_str(
                "no .rs file was opened — a clean answer and an answer about \
                 nothing are the same answer, so this is not one",
            ),
            Refusal::Unparsed(files) => {
                writeln!(
                    f,
                    "{} file(s) did not parse, so the gate read only part of the tree:",
                    files.len()
                )?;
                for (path, err) in files {
                    writeln!(f, "    {}: {err}", path.display())?;
                }
                Ok(())
            }
            Refusal::Unresolved(names) => {
                writeln!(
                    f,
                    "{} environment read(s) name a variable this walk cannot resolve, \
                     and an unreadable name must not count as no read:",
                    names.len()
                )?;
                for name in names {
                    writeln!(
                        f,
                        "    {}:{} spells it `{}`",
                        name.file.display(),
                        name.line,
                        name.spelled
                    )?;
                }
                Ok(())
            }
        }
    }
}

// --- cargo's answer ---------------------------------------------------------

#[derive(Debug, Deserialize)]
struct Metadata {
    packages: Vec<MetaPackage>,
    workspace_members: Vec<String>,
    workspace_root: PathBuf,
}

#[derive(Debug, Deserialize)]
struct MetaPackage {
    id: String,
    manifest_path: PathBuf,
    targets: Vec<MetaTarget>,
    #[serde(default)]
    dependencies: Vec<MetaDependency>,
}

/// One declared dependency. `path` is present exactly when it is a LOCAL one,
/// which is the only kind whose sources this gate can open.
#[derive(Debug, Deserialize)]
struct MetaDependency {
    path: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct MetaTarget {
    kind: Vec<String>,
    name: String,
    src_path: PathBuf,
}

/// One target cargo builds, reduced to what this gate asks of it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Target {
    /// The directory of the package that owns it — the key every local package
    /// is known by, because a package id is cargo's spelling and a path is the
    /// one thing a declared dependency gives.
    directory: PathBuf,
    kind: String,
    name: String,
    root: PathBuf,
}

/// Ask cargo where the workspace is, what it builds, and which of its packages
/// are LOCAL — sources this gate can open, as opposed to a registry crate whose
/// constants it cannot follow.
/// `--no-deps`, which is the difference between a census and a RESOLUTION.
/// Asking cargo to resolve wants a registry, a lockfile and often a network; the
/// hooks' own fixture trees have none of those, and the sibling gates that
/// survive being run there all ask this way. What resolution would have given —
/// which local packages a binary can read through — is read instead from the
/// `path` of each DECLARED dependency, followed manifest by manifest.
fn metadata(manifest: &Path) -> Result<Metadata, String> {
    let output = Command::new(cargo())
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .arg("--manifest-path")
        .arg(manifest)
        .output()
        .map_err(|e| format!("could not run cargo metadata: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata failed for {}: {}",
            manifest.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("cargo metadata is not the JSON this expects: {e}"))
}

fn cargo() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string())
}

// --- the walk ---------------------------------------------------------------

/// Run the gate over the workspace whose manifest this is.
pub fn run(manifest: &Path) -> Result<Report, String> {
    let meta = metadata(manifest)?;
    let members: BTreeSet<&str> = meta.workspace_members.iter().map(String::as_str).collect();

    let mut targets: Vec<Target> = Vec::new();
    for package in meta
        .packages
        .iter()
        .filter(|package| members.contains(package.id.as_str()))
    {
        for target in &package.targets {
            let Some(kind) = target
                .kind
                .iter()
                .find(|kind| matches!(kind.as_str(), "bin" | "lib" | "test" | "bench"))
            else {
                continue;
            };
            targets.push(Target {
                directory: package
                    .manifest_path
                    .parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_default(),
                kind: kind.clone(),
                name: target.name.clone(),
                root: target.src_path.clone(),
            });
        }
    }
    targets.sort();

    let mut coverage = Coverage::default();
    let mut sources: BTreeMap<PathBuf, syn::File> = BTreeMap::new();

    // Every target's file set, by following the `mod` chain cargo compiles.
    let mut files_of: BTreeMap<Target, BTreeSet<PathBuf>> = BTreeMap::new();
    for target in &targets {
        let files = module_tree(&target.root, &mut sources, &mut coverage);
        files_of.insert(target.clone(), files);
    }
    // And the library of every package this workspace's members can reach
    // through a declared path, so a read that happens inside a path dependency
    // on the binary's behalf is still the binary's read.
    let reachable = local_libraries(&meta);
    let mut files_of_lib: BTreeMap<PathBuf, BTreeSet<PathBuf>> = BTreeMap::new();
    for (package, lib) in &reachable.libraries {
        let files = module_tree(lib, &mut sources, &mut coverage);
        files_of_lib.insert(package.clone(), files);
    }

    account_for_the_rest(&meta.workspace_root, &sources, &mut coverage)?;
    coverage.scanned = sources.keys().cloned().collect();

    // Constants, from every source this gate opened. One name can be declared
    // in more than one crate; every value it can have must be named, which is
    // the conservative reading and the only one that cannot under-report.
    let mut constants: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for file in sources.values() {
        collect_constants(file, &mut constants);
    }
    // And what every function is CALLED with, so a read whose name is a
    // parameter resolves through the call sites of the function that takes it.
    let mut arguments: BTreeMap<(String, usize), BTreeSet<String>> = BTreeMap::new();
    for file in sources.values() {
        collect_arguments(file, &constants, &mut arguments);
    }

    let mut reach = Reach::default();
    let mut unresolved: Vec<Unresolved> = Vec::new();
    let mut read_by: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut reads_at: BTreeMap<(String, String), (PathBuf, usize)> = BTreeMap::new();

    for target in targets.iter().filter(|target| target.kind == "bin") {
        reach.binaries += 1;
        // What this program can read: its own sources, plus every local package
        // it depends on (its own library included, through the same graph).
        let mut program: BTreeSet<PathBuf> = files_of.get(target).cloned().unwrap_or_default();
        for directory in reachable.reached_from(&target.directory) {
            if let Some(files) = files_of_lib.get(&directory) {
                program.extend(files.iter().cloned());
            }
        }
        let variables = read_by.entry(target.name.clone()).or_default();
        for path in &program {
            let Some(file) = sources.get(path) else {
                continue;
            };
            for read in environment_reads(file) {
                match resolve(&read, &constants, &arguments) {
                    Some(names) => {
                        for name in names {
                            reads_at
                                .entry((target.name.clone(), name.clone()))
                                .or_insert_with(|| (path.clone(), read.line));
                            variables.insert(name);
                        }
                    }
                    None => unresolved.push(Unresolved {
                        spelled: read.spelled.clone(),
                        file: path.clone(),
                        line: read.line,
                    }),
                }
            }
        }
        reach.reads += variables.len();
    }

    let mut findings = Vec::new();
    for target in targets
        .iter()
        .filter(|target| target.kind == "test" || target.kind == "bench")
    {
        reach.test_targets += 1;
        let files = files_of.get(target).cloned().unwrap_or_default();
        let mut spawns: BTreeSet<String> = BTreeSet::new();
        let mut mentions: BTreeSet<String> = BTreeSet::new();
        let mut names: BTreeSet<String> = BTreeSet::new();
        let mut clears = false;
        for path in &files {
            let Some(file) = sources.get(path) else {
                continue;
            };
            spawns.extend(binaries_spawned(file));
            mentions.extend(names_mentioned(file, &constants));
            let (declared, cleared) = environment_named(file, &constants, &arguments);
            names.extend(declared);
            clears = clears || cleared;
        }
        if spawns.is_empty() {
            continue;
        }
        reach.spawning_targets += 1;
        if clears {
            reach.clearing_targets += 1;
            continue;
        }
        for binary in &spawns {
            let Some(variables) = read_by.get(binary) else {
                continue;
            };
            reach.mentioned += variables.intersection(&mentions).count();
            reach.named += variables.intersection(&names).count();
            for variable in variables.difference(&mentions) {
                let (read_in, line) = reads_at
                    .get(&(binary.clone(), variable.clone()))
                    .cloned()
                    .unwrap_or_default();
                findings.push(Finding {
                    test_target: target.name.clone(),
                    binary: binary.clone(),
                    variable: variable.clone(),
                    read_in,
                    line,
                });
            }
        }
    }

    findings.sort();
    findings.dedup();
    unresolved.sort();
    unresolved.dedup();

    Ok(Report {
        workspace_root: meta.workspace_root,
        coverage,
        reach,
        findings,
        unresolved,
        read_by,
    })
}

/// The local packages this workspace can reach, keyed by package directory:
/// where each one's library lives, and which other local directories it
/// declares a path to.
struct Reachable {
    /// Package directory -> its `lib` target root.
    libraries: BTreeMap<PathBuf, PathBuf>,
    /// Package directory -> the local directories it declares a path to.
    declares: BTreeMap<PathBuf, BTreeSet<PathBuf>>,
}

impl Reachable {
    /// Every local package directory reachable from one, transitively,
    /// including itself — a package reads through its own library too.
    fn reached_from(&self, from: &Path) -> BTreeSet<PathBuf> {
        let mut seen = BTreeSet::from([from.to_path_buf()]);
        let mut queue = VecDeque::from([from.to_path_buf()]);
        while let Some(directory) = queue.pop_front() {
            for next in self.declares.get(&directory).cloned().unwrap_or_default() {
                if seen.insert(next.clone()) {
                    queue.push_back(next);
                }
            }
        }
        seen
    }
}

/// Follow every DECLARED path dependency, manifest by manifest, collecting each
/// local package's library.
///
/// Cargo's resolved graph would answer this in one call and cannot be had here:
/// resolution wants a registry and a lockfile, and the hooks' fixture trees have
/// neither. A declared `path` needs none of that — it is in the manifest — and
/// following it is the same answer for the only packages this gate can open.
fn local_libraries(meta: &Metadata) -> Reachable {
    let mut reachable = Reachable {
        libraries: BTreeMap::new(),
        declares: BTreeMap::new(),
    };
    let mut known: BTreeSet<PathBuf> = BTreeSet::new();
    let mut asked: BTreeSet<PathBuf> = BTreeSet::new();
    let mut pending: VecDeque<PathBuf> = absorb(meta, &mut reachable, &mut known)
        .into_iter()
        .collect();
    while let Some(directory) = pending.pop_front() {
        if known.contains(&directory) || !asked.insert(directory.clone()) {
            continue;
        }
        // A manifest this walk cannot read is a package it cannot open, and the
        // reads inside it are then simply not counted — which the report says
        // by naming what it opened.
        if let Ok(census) = metadata(&directory.join("Cargo.toml")) {
            pending.extend(absorb(&census, &mut reachable, &mut known));
        }
    }
    reachable
}

/// Take one census into the map, and hand back the local directories it names
/// that are still unread.
fn absorb(
    census: &Metadata,
    reachable: &mut Reachable,
    known: &mut BTreeSet<PathBuf>,
) -> Vec<PathBuf> {
    let mut next = Vec::new();
    for package in &census.packages {
        let Some(directory) = package.manifest_path.parent().map(Path::to_path_buf) else {
            continue;
        };
        if !known.insert(directory.clone()) {
            continue;
        }
        if let Some(lib) = package
            .targets
            .iter()
            .find(|target| target.kind.iter().any(|kind| kind == "lib"))
        {
            reachable
                .libraries
                .insert(directory.clone(), lib.src_path.clone());
        }
        for path in package.dependencies.iter().filter_map(|d| d.path.as_ref()) {
            reachable
                .declares
                .entry(directory.clone())
                .or_default()
                .insert(path.clone());
            next.push(path.clone());
        }
    }
    next
}

/// Every file a target compiles, by following its `mod` declarations from the
/// root cargo named — which is the only definition of "this target's source"
/// that agrees with what cargo builds.
fn module_tree(
    root: &Path,
    sources: &mut BTreeMap<PathBuf, syn::File>,
    coverage: &mut Coverage,
) -> BTreeSet<PathBuf> {
    let mut files = BTreeSet::new();
    // The crate root owns the directory it sits in whatever it is called, which
    // an integration test makes visible: `tests/all.rs` is a crate root, and
    // reading it as an ordinary module puts its `#[path]` children under
    // `tests/all/`, where none of them are.
    let mut queue = VecDeque::from([(root.to_path_buf(), true)]);
    while let Some((path, is_crate_root)) = queue.pop_front() {
        if !files.insert(path.clone()) {
            continue;
        }
        if !sources.contains_key(&path) {
            let text = match std::fs::read_to_string(&path) {
                Ok(text) => text,
                Err(e) => {
                    coverage.unparsed.push((path.clone(), e.to_string()));
                    continue;
                }
            };
            match syn::parse_file(&text) {
                Ok(file) => {
                    sources.insert(path.clone(), file);
                }
                Err(e) => {
                    coverage.unparsed.push((path.clone(), e.to_string()));
                    continue;
                }
            }
        }
        let Some(file) = sources.get(&path) else {
            continue;
        };
        for child in child_modules(file, &path, is_crate_root) {
            queue.push_back((child, false));
        }
    }
    files
}

/// The files the `mod` items of one source declare, resolved the way rustc
/// resolves them.
///
/// TWO BASE DIRECTORIES, not one, and the difference is where the first draft
/// of this walk was wrong in both directions. A plain `mod foo;` inside an
/// ordinary module file looks in `<dir>/<this file's stem>/`; a `#[path = ".."]`
/// on a top-level `mod` is relative to the directory the current file is IN.
/// A crate root — `lib.rs`, `main.rs`, or whatever cargo named as a target's
/// source — owns its own directory for both.
fn child_modules(file: &syn::File, of: &Path, is_crate_root: bool) -> Vec<PathBuf> {
    let Some(here) = of.parent().map(Path::to_path_buf) else {
        return Vec::new();
    };
    let mod_root = matches!(
        of.file_name().and_then(|name| name.to_str()),
        Some("lib.rs") | Some("main.rs") | Some("mod.rs")
    );
    let plain_base = match is_crate_root || mod_root {
        true => here.clone(),
        false => match of.file_stem() {
            Some(stem) => here.join(stem),
            None => return Vec::new(),
        },
    };

    let mut children = Vec::new();
    for item in &file.items {
        let syn::Item::Mod(module) = item else {
            continue;
        };
        if module.content.is_some() {
            // An inline `mod foo { .. }` adds no file of its own.
            continue;
        }
        let declared = module
            .attrs
            .iter()
            .filter(|attribute| attribute.path().is_ident("path"))
            .find_map(|attribute| match &attribute.meta {
                syn::Meta::NameValue(pair) => string_of(&pair.value),
                _ => None,
            });
        match declared {
            Some(relative) => children.push(here.join(relative)),
            None => {
                let name = module.ident.to_string();
                let flat = plain_base.join(format!("{name}.rs"));
                let nested = plain_base.join(&name).join("mod.rs");
                if flat.exists() {
                    children.push(flat);
                } else if nested.exists() {
                    children.push(nested);
                }
            }
        }
    }
    children
}

/// Account for every `.rs` file under the workspace root that no target's `mod`
/// chain reached, so the report can say what it did not open.
fn account_for_the_rest(
    root: &Path,
    sources: &BTreeMap<PathBuf, syn::File>,
    coverage: &mut Coverage,
) -> Result<(), String> {
    let mut stack = VecDeque::from([root.to_path_buf()]);
    while let Some(directory) = stack.pop_front() {
        if directory != root && declares_a_workspace(&directory) {
            collect_into(&directory, &mut coverage.foreign_workspaces)?;
            continue;
        }
        let entries = std::fs::read_dir(&directory)
            .map_err(|e| format!("could not read {}: {e}", directory.display()))?;
        for entry in entries {
            let path = entry
                .map_err(|e| format!("could not read an entry of {}: {e}", directory.display()))?
                .path();
            if path.is_symlink() {
                continue;
            }
            if path.is_dir() {
                if path.file_name().is_some_and(|name| name == "target") {
                    let mut artifacts = BTreeSet::new();
                    collect_into(&path, &mut artifacts)?;
                    coverage.build_artifacts += artifacts.len();
                    continue;
                }
                stack.push_back(path);
                continue;
            }
            if path.extension().is_some_and(|ext| ext == "rs") && !sources.contains_key(&path) {
                coverage.unreached.insert(path);
            }
        }
    }
    Ok(())
}

fn collect_into(directory: &Path, into: &mut BTreeSet<PathBuf>) -> Result<(), String> {
    let mut stack = VecDeque::from([directory.to_path_buf()]);
    while let Some(directory) = stack.pop_front() {
        let entries = std::fs::read_dir(&directory)
            .map_err(|e| format!("could not read {}: {e}", directory.display()))?;
        for entry in entries {
            let path = entry
                .map_err(|e| format!("could not read an entry of {}: {e}", directory.display()))?
                .path();
            if path.is_symlink() {
                continue;
            }
            if path.is_dir() {
                stack.push_back(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                into.insert(path);
            }
        }
    }
    Ok(())
}

/// The same rule `scripts/check-side-workspaces.sh` discovers workspaces with:
/// a manifest whose own line says `[workspace]`. Kept identical on purpose —
/// two definitions of "is this a separate workspace" drift, and the shell one
/// is what decides which manifests this gate is pointed at.
fn declares_a_workspace(directory: &Path) -> bool {
    std::fs::read_to_string(directory.join("Cargo.toml"))
        .map(|text| text.lines().any(|line| line.trim() == "[workspace]"))
        .unwrap_or(false)
}

// --- the syntax -------------------------------------------------------------

/// One `env::var`-shaped call, as written.
#[derive(Debug, Clone)]
pub struct Read {
    /// The argument expression, rendered for a message.
    pub spelled: String,
    /// A literal name, when that is what it is.
    literal: Option<String>,
    /// The bare identifier the name is spelled with, when it is one: a
    /// constant, or a parameter of the function this read is inside.
    identifier: Option<String>,
    /// The function this read sits in, and where the identifier appears among
    /// its parameters — the two things needed to ask what callers pass.
    parameter_of: Option<(String, usize)>,
    pub line: usize,
}

/// A name, resolved — or `None` when the walk cannot follow it.
fn resolve(
    read: &Read,
    constants: &BTreeMap<String, BTreeSet<String>>,
    arguments: &BTreeMap<(String, usize), BTreeSet<String>>,
) -> Option<Vec<String>> {
    if let Some(literal) = &read.literal {
        return Some(vec![literal.clone()]);
    }
    let name = read.identifier.as_ref()?;
    if let Some(values) = constants.get(name) {
        return Some(values.iter().cloned().collect());
    }
    let (function, position) = read.parameter_of.clone()?;
    arguments
        .get(&(function, position))
        .filter(|values| !values.is_empty())
        .map(|values| values.iter().cloned().collect())
}

/// Every `const NAME: _ = "literal"` a source declares.
fn collect_constants(file: &syn::File, into: &mut BTreeMap<String, BTreeSet<String>>) {
    struct Constants<'a>(&'a mut BTreeMap<String, BTreeSet<String>>);
    impl<'ast> syn::visit::Visit<'ast> for Constants<'_> {
        fn visit_item_const(&mut self, item: &'ast syn::ItemConst) {
            if let Some(text) = string_of(&item.expr) {
                self.0
                    .entry(item.ident.to_string())
                    .or_default()
                    .insert(text);
            }
            syn::visit::visit_item_const(self, item);
        }
    }
    syn::visit::visit_file(&mut Constants(into), file);
}

/// What every plain function call is passed, by name and position, for the
/// arguments this walk can read as a string.
fn collect_arguments(
    file: &syn::File,
    constants: &BTreeMap<String, BTreeSet<String>>,
    into: &mut BTreeMap<(String, usize), BTreeSet<String>>,
) {
    struct Arguments<'a> {
        constants: &'a BTreeMap<String, BTreeSet<String>>,
        into: &'a mut BTreeMap<(String, usize), BTreeSet<String>>,
    }
    impl<'ast> syn::visit::Visit<'ast> for Arguments<'_> {
        fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
            if let syn::Expr::Path(path) = &*call.func {
                if let Some(function) = path.path.segments.last() {
                    let function = function.ident.to_string();
                    for (position, argument) in call.args.iter().enumerate() {
                        for value in strings_in(argument, self.constants) {
                            self.into
                                .entry((function.clone(), position))
                                .or_default()
                                .insert(value);
                        }
                    }
                }
            }
            syn::visit::visit_expr_call(self, call);
        }

        fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
            for expression in macro_expressions(invocation) {
                syn::visit::visit_expr(self, &expression);
            }
            syn::visit::visit_macro(self, invocation);
        }
    }
    syn::visit::visit_file(&mut Arguments { constants, into }, file);
}

/// The strings one argument expression can be, when the walk can say.
fn strings_in(
    expr: &syn::Expr,
    constants: &BTreeMap<String, BTreeSet<String>>,
) -> BTreeSet<String> {
    match expr {
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(text),
            ..
        }) => BTreeSet::from([text.value()]),
        syn::Expr::Path(path) => path
            .path
            .segments
            .last()
            .and_then(|segment| constants.get(&segment.ident.to_string()))
            .cloned()
            .unwrap_or_default(),
        // `&"X"` and `"X".to_string()` are the same name one hop away.
        syn::Expr::Reference(reference) => strings_in(&reference.expr, constants),
        syn::Expr::MethodCall(call) => strings_in(&call.receiver, constants),
        _ => BTreeSet::default(),
    }
}

/// The expressions inside a macro invocation, when its body is the
/// comma-separated list most of them are.
///
/// SYN DOES NOT WALK MACRO BODIES — they are an opaque token stream — and that
/// blindness is not a corner case here. Measured: `cache-budget`'s fixture
/// builds its environment list with `vec![..]`, so every variable it names sat
/// inside a macro, and the first version of this gate reported four defects
/// against the one fixture in this repository that names its whole environment
/// on purpose. A gate that cannot see the tidiest spelling of the shape it
/// enforces is worse than no gate: it teaches people to write it badly.
fn macro_expressions(invocation: &syn::Macro) -> Vec<syn::Expr> {
    use syn::punctuated::Punctuated;
    invocation
        .parse_body_with(Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated)
        .map(|parsed| parsed.into_iter().collect())
        .unwrap_or_default()
}

/// The string literals in a macro body, whatever shape the body has.
///
/// The fallback for the macros [`macro_expressions`] cannot parse — a format
/// string followed by a matcher, a `matches!` pattern, a custom DSL. A name is
/// still a name when it is spelled inside one.
fn macro_strings(invocation: &syn::Macro) -> Vec<String> {
    fn walk(stream: proc_macro2::TokenStream, into: &mut Vec<String>) {
        for tree in stream {
            match tree {
                proc_macro2::TokenTree::Literal(literal) => {
                    if let Ok(text) = syn::parse_str::<syn::LitStr>(&literal.to_string()) {
                        into.push(text.value());
                    }
                }
                proc_macro2::TokenTree::Group(group) => walk(group.stream(), into),
                _ => {}
            }
        }
    }
    let mut found = Vec::new();
    walk(invocation.tokens.clone(), &mut found);
    found
}

fn string_of(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(text),
            ..
        }) => Some(text.value()),
        _ => None,
    }
}

/// Is this call `std::env::var` / `var_os`, however it is spelled?
///
/// A one-segment `var(..)` is accepted because `use std::env::var` is how half
/// this repository spells it; a longer path must say `env` immediately before,
/// so a local helper called `var` is not mistaken for a read.
fn is_environment_read(path: &syn::Path) -> bool {
    let segments: Vec<String> = path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect();
    match segments.as_slice() {
        [only] => only == "var" || only == "var_os",
        [.., module, last] => (last == "var" || last == "var_os") && module == "env",
        _ => false,
    }
}

fn line_of(span: proc_macro2::Span) -> usize {
    span.start().line
}

/// Every environment read one source performs.
pub fn environment_reads(file: &syn::File) -> Vec<Read> {
    #[derive(Default)]
    struct Reads {
        found: Vec<Read>,
        /// The function being walked, and its parameter names in order.
        enclosing: Option<(String, Vec<String>)>,
    }
    impl Reads {
        fn in_function<F: FnOnce(&mut Self)>(
            &mut self,
            name: String,
            inputs: &syn::punctuated::Punctuated<syn::FnArg, syn::token::Comma>,
            body: F,
        ) {
            let parameters = inputs
                .iter()
                .map(|argument| match argument {
                    syn::FnArg::Typed(typed) => match &*typed.pat {
                        syn::Pat::Ident(ident) => ident.ident.to_string(),
                        _ => String::new(),
                    },
                    syn::FnArg::Receiver(_) => String::new(),
                })
                .collect();
            let outer = self.enclosing.replace((name, parameters));
            body(self);
            self.enclosing = outer;
        }
    }
    impl<'ast> syn::visit::Visit<'ast> for Reads {
        fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
            let name = item.sig.ident.to_string();
            self.in_function(name, &item.sig.inputs, |walk| {
                syn::visit::visit_item_fn(walk, item)
            });
        }

        fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
            let name = item.sig.ident.to_string();
            self.in_function(name, &item.sig.inputs, |walk| {
                syn::visit::visit_impl_item_fn(walk, item)
            });
        }

        fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
            if let syn::Expr::Path(path) = &*call.func {
                if is_environment_read(&path.path) {
                    for argument in &call.args {
                        self.found.push(read_of(argument, self.enclosing.as_ref()));
                    }
                }
            }
            syn::visit::visit_expr_call(self, call);
        }

        fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
            for expression in macro_expressions(invocation) {
                syn::visit::visit_expr(self, &expression);
            }
            syn::visit::visit_macro(self, invocation);
        }
    }
    let mut reads = Reads::default();
    syn::visit::visit_file(&mut reads, file);
    reads.found
}

fn read_of(argument: &syn::Expr, enclosing: Option<&(String, Vec<String>)>) -> Read {
    use syn::spanned::Spanned;
    let line = line_of(argument.span());
    match argument {
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(text),
            ..
        }) => Read {
            spelled: format!("{:?}", text.value()),
            literal: Some(text.value()),
            identifier: None,
            parameter_of: None,
            line,
        },
        syn::Expr::Reference(reference) => read_of(&reference.expr, enclosing),
        syn::Expr::Path(path) => {
            let segments: Vec<String> = path
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect();
            let identifier = segments.last().cloned();
            let parameter_of = identifier.as_ref().and_then(|name| {
                enclosing.and_then(|(function, parameters)| {
                    parameters
                        .iter()
                        .position(|parameter| parameter == name)
                        .map(|position| (function.clone(), position))
                })
            });
            Read {
                spelled: segments.join("::"),
                literal: None,
                identifier,
                parameter_of,
                line,
            }
        }
        other => Read {
            spelled: format!("an expression at line {}", line_of(other.span())),
            literal: None,
            identifier: None,
            parameter_of: None,
            line,
        },
    }
}

/// The binaries one test source spawns, by the `CARGO_BIN_EXE_` spelling cargo
/// checks at compile time.
pub fn binaries_spawned(file: &syn::File) -> BTreeSet<String> {
    const PREFIX: &str = "CARGO_BIN_EXE_";
    struct Spawns(BTreeSet<String>);
    impl<'ast> syn::visit::Visit<'ast> for Spawns {
        fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
            if invocation.path.is_ident("env") {
                if let Ok(text) = invocation.parse_body::<syn::LitStr>() {
                    if let Some(name) = text.value().strip_prefix(PREFIX) {
                        self.0.insert(name.to_string());
                    }
                }
            }
            syn::visit::visit_macro(self, invocation);
        }
    }
    let mut spawns = Spawns(BTreeSet::new());
    syn::visit::visit_file(&mut spawns, file);
    spawns.0
}

/// Every name one test source says at all — its string literals, and the values
/// of the constants it refers to.
///
/// This is what the verdict rests on, and the module doc says why the weaker
/// question is the answerable one. It cannot be defeated by how a fixture is
/// factored, and a variable a test never says is one it is certainly not
/// deciding.
pub fn names_mentioned(
    file: &syn::File,
    constants: &BTreeMap<String, BTreeSet<String>>,
) -> BTreeSet<String> {
    struct Mentions<'a> {
        constants: &'a BTreeMap<String, BTreeSet<String>>,
        found: BTreeSet<String>,
    }
    impl<'ast> syn::visit::Visit<'ast> for Mentions<'_> {
        fn visit_lit_str(&mut self, text: &'ast syn::LitStr) {
            self.found.insert(text.value());
        }

        fn visit_path(&mut self, path: &'ast syn::Path) {
            if let Some(segment) = path.segments.last() {
                if let Some(values) = self.constants.get(&segment.ident.to_string()) {
                    self.found.extend(values.iter().cloned());
                }
            }
            syn::visit::visit_path(self, path);
        }

        fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
            for expression in macro_expressions(invocation) {
                syn::visit::visit_expr(self, &expression);
            }
            // And the bodies no expression grammar fits, so a name spelled
            // inside `assert!(.., "..{}..", X)` is still a name said.
            self.found.extend(macro_strings(invocation));
            syn::visit::visit_macro(self, invocation);
        }
    }
    let mut mentions = Mentions {
        constants,
        found: BTreeSet::new(),
    };
    syn::visit::visit_file(&mut mentions, file);
    mentions.found
}

/// What one test source SETS or REMOVES, and whether it clears the environment
/// whole — the stronger claim, reported beside the mention the verdict uses.
pub fn environment_named(
    file: &syn::File,
    constants: &BTreeMap<String, BTreeSet<String>>,
    arguments: &BTreeMap<(String, usize), BTreeSet<String>>,
) -> (BTreeSet<String>, bool) {
    struct Named<'a> {
        constants: &'a BTreeMap<String, BTreeSet<String>>,
        arguments: &'a BTreeMap<(String, usize), BTreeSet<String>>,
        found: BTreeSet<String>,
        clears: bool,
    }
    impl<'ast> syn::visit::Visit<'ast> for Named<'_> {
        fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
            match call.method.to_string().as_str() {
                "env" | "env_remove" => {
                    if let Some(first) = call.args.first() {
                        self.found.extend(strings_in(first, self.constants));
                        // A name handed in as a variable resolves the same way
                        // a read does: through what the enclosing helper is
                        // called with.
                        if let syn::Expr::Path(path) = first {
                            if let Some(last) = path.path.segments.last() {
                                let name = last.ident.to_string();
                                for ((_, _), values) in self
                                    .arguments
                                    .iter()
                                    .filter(|((function, _), _)| function == &name)
                                {
                                    self.found.extend(values.iter().cloned());
                                }
                            }
                        }
                    }
                }
                "env_clear" => self.clears = true,
                _ => {}
            }
            syn::visit::visit_expr_method_call(self, call);
        }

        fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
            for expression in macro_expressions(invocation) {
                syn::visit::visit_expr(self, &expression);
            }
            syn::visit::visit_macro(self, invocation);
        }
    }
    let mut named = Named {
        constants,
        arguments,
        found: BTreeSet::new(),
        clears: false,
    };
    syn::visit::visit_file(&mut named, file);
    (named.found, named.clears)
}
