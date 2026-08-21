//! The gate for Rust that exists and that nothing compiles.
//!
//! # The law
//!
//! **Every Rust file this repository tracks is one cargo compiles.**
//!
//! # What it exists because of
//!
//! R1084 gated the level below: every test this repository COMPILES is one some
//! CI command runs. Both of its sides are asked of the machine, and both of them
//! start from the same place — a build. A test cargo never compiles is therefore
//! absent from its population AND from its run set at once, and two absences
//! that cancel produce no finding. The gate reported `dark 0` and the report was
//! true; the question it answers is just narrower than it reads.
//!
//! That shape is not hypothetical. R1084 found it inside itself: doc-tests have
//! no test binary, so cargo's artifact record never mentions one, so they were
//! missing symmetrically and the verdict was a confident zero. It was found by
//! chasing a thirteen-test disagreement between two instruments, never by
//! reading the output. This crate is the second instrument for the level above,
//! turned into a law.
//!
//! # Both sides are asked of the machine
//!
//! - TRACKED — `git ls-files '*.rs'`. Not a directory walk: a file git does not
//!   track is not this repository's, and it is also not one a CI checkout has.
//! - COMPILED — for every workspace the lister says this machine can build,
//!   `cargo check --workspace --all-targets`, once with `--all-features` and
//!   once with the default set. Then the files each compilation READ, out of
//!   rustc's own dep-info rather than a module resolution this gate wrote.
//!
//! Anything tracked and not read is Rust that exists and that nothing compiles.
//!
//! # Why dep-info and not a module walk
//!
//! Following `mod` declarations from each target root is the obvious way to
//! decide which files a crate is made of, and it is a MODEL of what rustc does:
//! it has to re-implement `#[path]`, `mod.rs` versus `name.rs`, `cfg` and
//! `include!`. This repository already holds the case that breaks it —
//! `bench/crates/codegen-prototype/tests/fixtures/salsa_wire_emit.rs` is a file
//! no `mod` names, pulled in by `include!(concat!(env!("CARGO_MANIFEST_DIR"),
//! …))`, a path no walker resolves without evaluating macros. rustc records it,
//! because rustc opened it. Dep-info is the compiler's own answer to exactly
//! this question and there is no second implementation to drift.
//!
//! # Why two feature resolves
//!
//! `--all-features` turns every feature ON, which is what makes a file behind
//! `#[cfg(feature = "x")] mod x;` part of the population. It is also what hides
//! a file behind `#[cfg(not(feature = "x"))]`, and one flag cannot do both. So
//! the read set is the union of two resolves, all-features and default, and the
//! limit that survives is stated rather than left implied: a file visible ONLY
//! under some third combination of mutually exclusive features is in neither,
//! and enumerating combinations is exponential.
//!
//! # Why it reports what it reached
//!
//! An empty finding list is what a fully compiled repository looks like AND what
//! a repository nobody probed looks like. R1078 shipped the second while
//! believing the first, so [`Report::verdict`] REFUSES rather than passing when
//! a probe did not compile, when an artifact cargo reported has no dep-info this
//! gate could find, or when a target root cargo NAMES is absent from what the
//! same build was measured to have read. That last one is the self-check that
//! makes the rest trustworthy: cargo states the roots independently of rustc
//! stating the reads, and a join that quietly drops units disagrees with it.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

use ci_plan::issue::{self, Tree};
use ci_plan::CargoCommand;
use serde::Deserialize;

/// Tracked Rust files this repository deliberately does not compile, each with
/// the reason it does not.
///
/// IT IS EMPTY, and that is the state to keep it in. It exists for the reason
/// `scripts/check-side-workspaces.sh` keeps an empty `ungated` map: a total rule
/// with no way to write down a defended exception is a rule somebody eventually
/// routes around instead of arguing with. An entry here should read like a
/// decision a person made and can defend in review — not like a failure
/// somebody stepped around — and it is printed on every run so that adding one
/// is visible forever after.
///
/// It is this repository's table and not the gate's: [`conclude`] takes the
/// table it is to honour as an argument, so that what an exception DOES is
/// something a test can aim at. Read out of the `const` by the function it
/// steers, the branch below it was reachable only by editing this line — a
/// calculation with no test, hidden by the table being empty rather than by the
/// code being subtle.
///
/// An entry stops excusing anything the moment its file is compiled or leaves
/// the repository, and the gate REFUSES rather than passing then
/// ([`Refusal::StaleDeclaration`]): a dead exception is invisible in a printed
/// list of live ones, and this table is reviewed by being printed.
pub const DECLARED: &[(&str, &str)] = &[];

/// One tracked Rust file no compilation read.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Finding {
    /// Path relative to the repository root.
    pub file: PathBuf,
    /// The `#[test]` functions in it, module-qualified as the harness would
    /// print them if anything compiled the file.
    ///
    /// This is what makes the finding worse rather than what makes it a
    /// finding: an uncompiled file is a defect whether or not it holds tests,
    /// and a test inside one is additionally invisible to R1084's gate, which
    /// can only see what a build produced.
    pub tests: Vec<String>,
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.file.display())?;
        match self.tests.len() {
            0 => Ok(()),
            n => write!(f, " — and {n} test(s) inside it that nothing can run"),
        }
    }
}

/// Why the gate declined to give a verdict at all.
///
/// Separate from "files found uncompiled", because the two demand different
/// actions and look identical in an exit code alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// A probe's compilation did not succeed, so part of the tree is unread.
    CheckFailed { label: String, message: String },
    /// Cargo reported building something this repository owns and the gate
    /// found no dep-info for it, so the files that unit read are unknown rather
    /// than absent.
    NoDepInfo { label: String, artifact: PathBuf },
    /// Cargo NAMES a target whose own root source is missing from what the same
    /// build was measured to have read. Two of cargo's answers disagree, and a
    /// gate cannot pick between them.
    TargetRootUnread { label: String, target_src: PathBuf },
    /// `git ls-files` named no Rust at all — the gate is pointed at something
    /// that is not this repository, or git answered nothing.
    NothingTracked,
    /// A [`DECLARED`] exception excuses nothing: the file it names is compiled
    /// now, or this repository no longer tracks it.
    StaleDeclaration { file: PathBuf, because: String },
    /// A candidate file would not parse, so what is inside it is unknown.
    Unparsed { file: PathBuf, message: String },
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::CheckFailed { label, message } => write!(
                f,
                "`{label}` did not compile, so this gate read only part of the \
                 tree — a clean answer and an answer about nothing are the same \
                 answer, so this is not one:\n{message}"
            ),
            Refusal::NoDepInfo { label, artifact } => write!(
                f,
                "`{label}` built {} and this gate found no dep-info for it, so \
                 the files that unit read are unknown rather than none",
                artifact.display()
            ),
            Refusal::TargetRootUnread { label, target_src } => write!(
                f,
                "`{label}` names a target rooted at {} and nothing in the same \
                 build's dep-info mentions that file — cargo's two answers \
                 disagree, and this gate is the wrong place to pick a winner",
                target_src.display()
            ),
            Refusal::NothingTracked => f.write_str(
                "`git ls-files` named no Rust file at all — this is not the \
                 repository this gate is for, or git answered nothing",
            ),
            Refusal::StaleDeclaration { file, because } => write!(
                f,
                "{} is declared deliberately uncompiled and {because}, so the \
                 exception excuses nothing — a table that is reviewed by being \
                 printed cannot show an entry that no longer applies, and the \
                 law this run enforced is not the one the table describes. \
                 Fix: delete the entry.",
                file.display()
            ),
            Refusal::Unparsed { file, message } => write!(
                f,
                "{} would not parse, so whether it holds tests is unknown: \
                 {message}",
                file.display()
            ),
        }
    }
}

/// What one compilation turned out to have read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Probe {
    /// How the command reads back, for the gate's own output.
    pub label: String,
    /// Units cargo reported building, this repository's and its dependencies'.
    pub artifacts: usize,
    /// Dep-info files the gate joined those units to.
    pub dep_info: usize,
    /// Tracked-shaped Rust files inside the repository that this compilation
    /// read, relative to the repository root.
    pub read: BTreeSet<PathBuf>,
}

/// The gate's answer.
///
/// `Default` is here so the two answers it carries — "could I judge" and "what
/// did I find" — can be exercised without a build.
#[derive(Debug, Clone, Default)]
pub struct Report {
    pub root: PathBuf,
    /// One per workspace per feature resolve.
    pub probes: Vec<Probe>,
    /// Workspaces the lister declined, with the reason it gave.
    pub skipped_workspaces: Vec<ci_plan::SkippedWorkspace>,
    /// Every Rust file this repository tracks.
    pub tracked: BTreeSet<PathBuf>,
    /// Of those, the ones some compilation read.
    pub read: BTreeSet<PathBuf>,
    /// Tracked files inside a workspace the lister declined. Not findings: this
    /// machine cannot compile that workspace at all, so it has nothing to say
    /// about the files in it, and saying it loudly is the difference between a
    /// limit and a lie.
    pub in_skipped_workspace: BTreeSet<PathBuf>,
    /// Tracked files [`DECLARED`] as deliberately uncompiled, with the reason.
    pub declared: Vec<(PathBuf, String)>,
    pub findings: Vec<Finding>,
    pub refusals: Vec<Refusal>,
}

impl Report {
    /// How many TRACKED files the probes read.
    ///
    /// Not `read.len()`, which is every file inside the repository some
    /// compilation opened — including generated Rust under `target/`, which git
    /// tracks none of. Reported beside `tracked.len()` the raw count reads as a
    /// subset of it and is not one: this repository prints 271 tracked against
    /// 375 read, a pair no reader can hold both halves of. The comparison the
    /// verdict is actually made of is this intersection.
    pub fn compiled(&self) -> usize {
        self.tracked.intersection(&self.read).count()
    }

    /// `Ok(())` when the gate could judge, `Err` when it could not. Findings are
    /// read separately: a report can be judgeable AND defective.
    pub fn verdict(&self) -> Result<(), &[Refusal]> {
        if self.refusals.is_empty() {
            Ok(())
        } else {
            Err(&self.refusals)
        }
    }
}

// --- the tracked side ---------------------------------------------------------

/// Every Rust file this repository tracks, relative to its root.
///
/// `git ls-files` rather than a directory walk, for two reasons that point the
/// same way: an untracked file is not this repository's to judge, and it is also
/// not one a CI checkout has, so a gate that walked the disk would answer about
/// a tree that only exists here.
pub fn tracked_sources(root: &Path) -> Result<BTreeSet<PathBuf>, String> {
    let output = Command::new("git")
        .args(["ls-files", "-z", "--", "*.rs"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("could not run git ls-files: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .split('\0')
        .filter(|entry| !entry.is_empty())
        .map(PathBuf::from)
        .collect())
}

// --- the compiled side --------------------------------------------------------

/// The compilation that decides what "cargo compiles this file" means.
///
/// `--all-targets` is load-bearing and pinned by a test: without it cargo builds
/// neither examples nor benches, and `cargo test` does not build examples even
/// when it runs — measured on cargo 1.94, where a full `cargo test` over
/// `codegen-prototype` leaves its one example untouched. Whether the feature
/// flag is added is the caller's, because one command cannot both enable and
/// disable a feature and the read set is the union of the two that can.
pub fn check_command(manifest: &str, all_features: bool) -> CargoCommand {
    let mut cargo_args: Vec<String> = [
        "cargo",
        "check",
        "--manifest-path",
        manifest,
        "--workspace",
        "--all-targets",
    ]
    .iter()
    .map(|word| (*word).to_string())
    .collect();
    if all_features {
        cargo_args.push("--all-features".to_string());
    }
    CargoCommand {
        source: "scripts/check-side-workspaces.sh".to_string(),
        owner: manifest.to_string(),
        // A probe this gate builds for itself is issued directly; nothing hands
        // it over.
        carrier: Vec::new(),
        cargo_args,
        harness_args: Vec::new(),
        env: Default::default(),
        // The manifest is a literal in the words, so the manifest is what says
        // whose lockfile this resolves — the reading every command written as
        // data gets.
        declared: None,
        uncounted: Vec::new(),
    }
}

#[derive(Debug, Deserialize)]
struct Artifact {
    reason: String,
    #[serde(default)]
    package_id: String,
    #[serde(default)]
    target: Option<ArtifactTarget>,
    #[serde(default)]
    filenames: Vec<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct ArtifactTarget {
    kind: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Metadata {
    workspace_root: PathBuf,
    target_directory: PathBuf,
    workspace_members: Vec<String>,
    packages: Vec<MetaPackage>,
}

#[derive(Debug, Deserialize)]
struct MetaPackage {
    id: String,
    targets: Vec<MetaTarget>,
}

#[derive(Debug, Deserialize)]
struct MetaTarget {
    src_path: PathBuf,
}

/// Ask cargo where this workspace keeps its things and which packages are its
/// own. `--no-deps`, because the registry's packages are nobody's to judge here.
fn metadata(root: &Path, manifest: &str) -> Result<Metadata, String> {
    let output = issue::cargo(Tree::WhereverTheCallerPoints(
        "the gate is pointed at a manifest by whoever runs it, this repository's \
         or a fixture's",
    ))
    .args(["metadata", "--format-version", "1", "--no-deps"])
    .arg("--manifest-path")
    .arg(manifest)
    .current_dir(root)
    .output()
    .map_err(|e| format!("could not run cargo metadata: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("cargo metadata is not the JSON this expects: {e}"))
}

/// One dep-info file: what it says was produced, and what was read to produce it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DepInfo {
    /// The file itself, which is how a build script's unit is recognised.
    at: PathBuf,
    /// Everything to the left of a `:` in it.
    produced: Vec<PathBuf>,
    /// Everything to the right.
    read: Vec<PathBuf>,
}

/// Every dep-info file under a target directory.
///
/// The whole tree rather than the two directories cargo happens to use today:
/// unit dep-info lives in `deps/`, a build script's in its own `build/<unit>/`
/// directory, and an example's in `examples/` — three layouts already, and a
/// gate that enumerates them is one cargo can move out from under.
fn dep_info_under(directory: &Path) -> Vec<DepInfo> {
    let mut found = Vec::new();
    collect_dep_info(directory, &mut found);
    found
}

fn collect_dep_info(directory: &Path, out: &mut Vec<DepInfo>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if kind.is_dir() {
            collect_dep_info(&path, out);
            continue;
        }
        if path.extension().is_none_or(|extension| extension != "d") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let mut produced = Vec::new();
        let mut read = Vec::new();
        for line in text.lines() {
            let Some((target, dependencies)) = parse_dep_line(line) else {
                continue;
            };
            produced.push(target);
            read.extend(dependencies);
        }
        if produced.is_empty() {
            continue;
        }
        out.push(DepInfo {
            at: path,
            produced,
            read,
        });
    }
}

/// Split one Makefile-shaped dep-info line into its target and its dependencies.
///
/// The split cannot be on whitespace: rustc escapes a space inside a path as
/// `\ `, and a path holding one would otherwise read as two files, neither of
/// which exists. Lines with a target and no dependencies — dep-info repeats each
/// dependency as its own empty rule — carry nothing this gate has not already
/// read from the line above, so they return `None`.
fn parse_dep_line(line: &str) -> Option<(PathBuf, Vec<PathBuf>)> {
    let mut escaped = false;
    let mut ends_at = None;
    for (index, character) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match character {
            '\\' => escaped = true,
            ':' => {
                // The colon that ends the target is the one followed by
                // whitespace or by nothing; any other is inside a path.
                if line[index + 1..].starts_with(char::is_whitespace) {
                    ends_at = Some(index);
                    break;
                }
            }
            _ => {}
        }
    }
    let at = ends_at?;
    let dependencies = unescaped_words(&line[at + 1..]);
    if dependencies.is_empty() {
        return None;
    }
    Some((
        PathBuf::from(unescape(&line[..at])),
        dependencies.into_iter().map(PathBuf::from).collect(),
    ))
}

fn unescaped_words(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut escaped = false;
    for character in text.chars() {
        if escaped {
            current.push(character);
            escaped = false;
            continue;
        }
        match character {
            '\\' => escaped = true,
            _ if character.is_whitespace() => {
                if !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(character),
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

fn unescape(text: &str) -> String {
    unescaped_words(text).join(" ")
}

/// Run one compilation and read back which of this repository's files it opened.
///
/// The join between "what cargo says it built" and "what rustc says it read" is
/// the load-bearing step, and it is what keeps a warm target directory from
/// answering about a build that no longer happens: dep-info is never garbage
/// collected, so a unit deleted from the graph leaves its record behind, and a
/// gate that swept the directory would credit a file nothing compiles any more.
/// Only dep-info this build's own artifacts join to is counted.
pub fn probe(root: &Path, command: &CargoCommand, refusals: &mut Vec<Refusal>) -> Option<Probe> {
    let label = command.rendered();
    let manifest = command
        .value(&["--manifest-path"])
        .unwrap_or("Cargo.toml")
        .to_string();
    let meta = match metadata(root, &manifest) {
        Ok(meta) => meta,
        Err(message) => {
            refusals.push(Refusal::CheckFailed { label, message });
            return None;
        }
    };

    let output = issue::cargo(Tree::WhereverTheCallerPoints(
        "the words are a command judged where it is WRITTEN — this re-issues \
         them, and whose lockfile they resolve is decided by the manifest they \
         already name",
    ))
    .args(command.cargo_args.iter().skip(1))
    .arg("--message-format=json")
    .current_dir(root)
    .output();
    let output = match output {
        Ok(output) => output,
        Err(e) => {
            refusals.push(Refusal::CheckFailed {
                label,
                message: format!("could not run cargo: {e}"),
            });
            return None;
        }
    };
    if !output.status.success() {
        refusals.push(Refusal::CheckFailed {
            label,
            message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
        return None;
    }

    let members: BTreeSet<&String> = meta.workspace_members.iter().collect();
    let mut artifacts: Vec<(String, bool, Vec<PathBuf>)> = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let Ok(artifact) = serde_json::from_str::<Artifact>(line) else {
            continue;
        };
        if artifact.reason != "compiler-artifact" {
            continue;
        }
        let is_build_script = artifact
            .target
            .as_ref()
            .is_some_and(|target| target.kind.iter().any(|kind| kind == "custom-build"));
        artifacts.push((
            artifact.package_id,
            is_build_script,
            artifact.filenames.clone(),
        ));
    }

    let records = dep_info_under(&meta.target_directory);
    // Two indexes, because cargo writes a unit's outputs and a build script's
    // outputs under different names. A unit's dep-info NAMES the artifact cargo
    // reported. A build script's does not — cargo reports
    // `build/<unit>/build-script-build` and rustc wrote
    // `build/<unit>/build_script_build-<hash>` — but the two are alone together
    // in that directory, which is a fact about the layout cargo maintains and
    // not a name this gate spells.
    let mut by_output: BTreeMap<&Path, Vec<usize>> = BTreeMap::new();
    let mut by_directory: BTreeMap<&Path, Vec<usize>> = BTreeMap::new();
    for (index, record) in records.iter().enumerate() {
        for produced in &record.produced {
            by_output.entry(produced.as_path()).or_default().push(index);
        }
        if let Some(parent) = record.at.parent() {
            by_directory.entry(parent).or_default().push(index);
        }
    }

    let mut joined: BTreeSet<usize> = BTreeSet::new();
    for (package_id, is_build_script, filenames) in &artifacts {
        let mut matched = Vec::new();
        for filename in filenames {
            let found = if *is_build_script {
                filename.parent().and_then(|at| by_directory.get(at))
            } else {
                by_output.get(filename.as_path())
            };
            if let Some(indexes) = found {
                matched.extend(indexes.iter().copied());
            }
        }
        if matched.is_empty() && members.contains(package_id) {
            refusals.push(Refusal::NoDepInfo {
                label: label.clone(),
                artifact: filenames.first().cloned().unwrap_or_default(),
            });
            return None;
        }
        joined.extend(matched);
    }

    let mut read = BTreeSet::new();
    for index in &joined {
        for source in &records[*index].read {
            if source.extension().is_none_or(|extension| extension != "rs") {
                continue;
            }
            // Dep-info spells its dependencies relative to the WORKSPACE root,
            // not to the directory cargo was invoked from — measured, because
            // `bench`'s records say `crates/…` while cargo ran from the
            // repository root two levels up.
            let absolute = if source.is_absolute() {
                source.clone()
            } else {
                meta.workspace_root.join(source)
            };
            if let Ok(inside) = absolute.strip_prefix(root) {
                read.insert(inside.to_path_buf());
            }
        }
    }

    // THE SELF-CHECK. Cargo names its targets in `metadata`; rustc names the
    // files it read in dep-info. They are two answers from two programs, and
    // every target root has to appear in both — a join that silently matched
    // nothing would otherwise report a large, confident, empty read set. It is
    // asked only of the all-features probe: a target gated behind
    // `required-features` is not built by the default resolve, and demanding its
    // root there would make the gate reject for a reason outside its own law.
    if command.has("--all-features") {
        for package in &meta.packages {
            if !members.contains(&package.id) {
                continue;
            }
            for target in &package.targets {
                let Ok(inside) = target.src_path.strip_prefix(root) else {
                    continue;
                };
                if !read.contains(inside) {
                    refusals.push(Refusal::TargetRootUnread {
                        label: label.clone(),
                        target_src: inside.to_path_buf(),
                    });
                    return None;
                }
            }
        }
    }

    Some(Probe {
        label,
        artifacts: artifacts.len(),
        dep_info: joined.len(),
        read,
    })
}

/// Both feature resolves of one workspace.
pub fn probe_workspace(root: &Path, manifest: &str, refusals: &mut Vec<Refusal>) -> Vec<Probe> {
    [true, false]
        .into_iter()
        .filter_map(|all_features| probe(root, &check_command(manifest, all_features), refusals))
        .collect()
}

// --- what is inside a file nothing compiles -----------------------------------

/// The `#[test]` functions in one Rust source, module-qualified.
///
/// An attribute counts when the LAST segment of its path is `test`, which is
/// `#[test]`, `#[tokio::test]` and `#[async_std::test]` alike — the harness
/// prints all three the same way and this gate has no business knowing which
/// runtime a test asked for.
pub fn tests_in(source: &str) -> Result<Vec<String>, String> {
    let file = syn::parse_file(source).map_err(|e| e.to_string())?;
    let mut found = Vec::new();
    collect_tests(&file.items, &mut Vec::new(), &mut found);
    Ok(found)
}

fn collect_tests(items: &[syn::Item], path: &mut Vec<String>, found: &mut Vec<String>) {
    for item in items {
        match item {
            syn::Item::Mod(module) => {
                let Some((_, inner)) = &module.content else {
                    continue;
                };
                path.push(module.ident.to_string());
                collect_tests(inner, path, found);
                path.pop();
            }
            syn::Item::Fn(function) if is_test(&function.attrs) => {
                let mut name = path.join("::");
                if !name.is_empty() {
                    name.push_str("::");
                }
                name.push_str(&function.sig.ident.to_string());
                found.push(name);
            }
            _ => {}
        }
    }
}

fn is_test(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attribute| {
        attribute
            .path()
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "test")
    })
}

// --- the run ------------------------------------------------------------------

/// Compare the two sides and say what is in the first and not the second,
/// excusing the files `declared` deliberately uncompiled.
///
/// Pure, so that what the gate CONCLUDES is testable without a build — the
/// separation R1084 had to add after an injection sweep found its own verdict
/// had no test aimed at it. `declared` is an argument for the same reason one
/// step further on: a policy the function reads out of a `const` steers a
/// branch no caller can vary, so no test can aim at it either. That the table
/// ships empty is what kept it looking covered.
pub fn conclude(
    root: &Path,
    tracked: &BTreeSet<PathBuf>,
    read: &BTreeSet<PathBuf>,
    skipped_workspaces: &[ci_plan::SkippedWorkspace],
    declared: &[(&str, &str)],
) -> Report {
    let mut report = Report {
        root: root.to_path_buf(),
        tracked: tracked.clone(),
        read: read.clone(),
        skipped_workspaces: skipped_workspaces.to_vec(),
        ..Report::default()
    };
    if tracked.is_empty() {
        report.refusals.push(Refusal::NothingTracked);
        return report;
    }

    let excused: BTreeMap<&str, &str> = declared.iter().copied().collect();
    for file in tracked {
        if read.contains(file) {
            continue;
        }
        // The lister's own words for a workspace it declined name a directory,
        // and a file inside one is a file this machine cannot compile at all.
        // Answering "uncompiled" there would make the verdict a fact about which
        // machine ran the gate.
        if skipped_workspaces
            .iter()
            .any(|workspace| file.starts_with(&workspace.directory))
        {
            report.in_skipped_workspace.insert(file.clone());
            continue;
        }
        if let Some(reason) = file.to_str().and_then(|name| excused.get(name)) {
            report.declared.push((file.clone(), (*reason).to_string()));
            continue;
        }
        // WHETHER IT IS A FINDING IS ALREADY DECIDED — it is tracked and no
        // compilation read it. What is INSIDE it is a second question, and a
        // file that will not open or will not parse leaves that one unanswered
        // rather than answering it with zero. Both are recorded: swallowing the
        // finding here would let an unreadable file hide the defect it is.
        let tests = std::fs::read_to_string(root.join(file))
            .map_err(|e| e.to_string())
            .and_then(|source| tests_in(&source));
        let tests = match tests {
            Ok(tests) => tests,
            Err(message) => {
                report.refusals.push(Refusal::Unparsed {
                    file: file.clone(),
                    message,
                });
                Vec::new()
            }
        };
        report.findings.push(Finding {
            file: file.clone(),
            tests,
        });
    }

    // AND THE OTHER DIRECTION. An exception is reviewed by being printed, so an
    // entry that has stopped excusing anything is invisible: it does not appear
    // in the run's declared list, because nothing matched it. The table would
    // then only ever grow, each dead line making the next one easier to add.
    // A workspace the lister declined is left out of this, and not because it
    // is convenient: this machine cannot compile those files at all, so whether
    // their exceptions still apply is not a question it has an answer to, and
    // refusing here would make the gate's verdict a fact about which machine
    // ran it — the asymmetry `in_skipped_workspace` exists to keep out.
    for (name, _) in declared {
        let file = PathBuf::from(name);
        if report.declared.iter().any(|(at, _)| at == &file) {
            continue;
        }
        if skipped_workspaces
            .iter()
            .any(|workspace| file.starts_with(&workspace.directory))
        {
            continue;
        }
        let because = if tracked.contains(&file) {
            "something compiles it now"
        } else {
            "this repository no longer tracks a file by that name"
        };
        report.refusals.push(Refusal::StaleDeclaration {
            file,
            because: because.to_string(),
        });
    }
    report
}

/// Probe these workspaces, then judge the tree against what they read,
/// excusing the files `declared` deliberately uncompiled.
///
/// Separate from [`run`] by exactly what [`run`] goes and asks its environment
/// for — which workspaces this machine has, and which exceptions this
/// repository declares — so that a fixture tree, which has neither, still goes
/// through the whole of the rest of the gate rather than through a second
/// assembly written for tests.
pub fn examine(
    root: &Path,
    manifests: &[String],
    skipped: &[ci_plan::SkippedWorkspace],
    declared: &[(&str, &str)],
) -> Report {
    let mut refusals = Vec::new();
    let mut probes = Vec::new();
    for manifest in manifests {
        probes.extend(probe_workspace(root, manifest, &mut refusals));
    }
    let read: BTreeSet<PathBuf> = probes
        .iter()
        .flat_map(|probe| probe.read.iter().cloned())
        .collect();

    let mut report = match tracked_sources(root) {
        Ok(tracked) => conclude(root, &tracked, &read, skipped, declared),
        Err(message) => {
            refusals.push(Refusal::Unparsed {
                file: PathBuf::from("git ls-files"),
                message,
            });
            Report {
                root: root.to_path_buf(),
                read,
                skipped_workspaces: skipped.to_vec(),
                ..Report::default()
            }
        }
    };
    report.probes = probes;
    // A probe's refusals come first: "I could not compile this workspace" is
    // why the files under it look unread, and printing the consequence above
    // the cause reads as two independent failures.
    refusals.extend(std::mem::take(&mut report.refusals));
    report.refusals = refusals;
    report
}

/// Run the gate over the repository rooted here, under this repository's own
/// exception table.
pub fn run(root: &Path) -> Report {
    let listed = ci_plan::workspaces(root);
    examine(root, &listed.askable, &listed.skipped, DECLARED)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dependency_line_is_split_where_the_target_ends() {
        let (target, dependencies) =
            parse_dep_line("/at/deps/reading-af69.d: tools/ci-plan/tests/reading.rs Cargo.toml")
                .expect("a rule with dependencies");
        assert_eq!(target, PathBuf::from("/at/deps/reading-af69.d"));
        assert_eq!(
            dependencies,
            vec![
                PathBuf::from("tools/ci-plan/tests/reading.rs"),
                PathBuf::from("Cargo.toml"),
            ]
        );
    }

    #[test]
    fn a_path_holding_a_space_is_one_file() {
        // rustc escapes a space rather than quoting, so splitting on whitespace
        // turns one file into two that do not exist — and the gate would then
        // report the real one as never compiled. HH6 named the same class one
        // round earlier in a different transport.
        let (_, dependencies) = parse_dep_line(r"/at/x.d: crates/a\ b/src/lib.rs other.rs")
            .expect("a rule with dependencies");
        assert_eq!(
            dependencies,
            vec![
                PathBuf::from("crates/a b/src/lib.rs"),
                PathBuf::from("other.rs"),
            ]
        );
    }

    #[test]
    fn a_rule_with_no_dependencies_says_nothing_new() {
        // Dep-info repeats every dependency as its own empty rule. Reading those
        // as targets would put source files into the OUTPUT index, where an
        // artifact could join to one and drag in a whole other unit's reads.
        assert_eq!(parse_dep_line("crates/x/src/lib.rs:"), None);
        assert_eq!(parse_dep_line(""), None);
        assert_eq!(
            parse_dep_line("# env-dep:CARGO_BIN_EXE_x=placeholder"),
            None
        );
    }

    #[test]
    fn a_test_is_named_the_way_a_harness_would_name_it() {
        let found = tests_in(
            "#[test]\n\
             fn at_the_root() {}\n\
             \n\
             fn not_a_test() {}\n\
             \n\
             #[should_panic]\n\
             fn attributed_but_not_a_test() {}\n\
             \n\
             mod outer {\n\
             \x20   pub mod inner {\n\
             \x20       #[tokio::test]\n\
             \x20       async fn asynchronous() {}\n\
             \x20   }\n\
             \x20   #[test]\n\
             \x20   fn plain() {}\n\
             }\n",
        )
        .expect("the source parses");
        assert_eq!(
            found,
            vec![
                "at_the_root".to_string(),
                "outer::inner::asynchronous".to_string(),
                "outer::plain".to_string(),
            ],
            "a runtime's test attribute is still a test attribute, and a \
             function with some other attribute is not one"
        );
    }

    #[test]
    fn source_that_does_not_parse_is_not_source_with_no_tests() {
        assert!(
            tests_in("fn ( this is not rust").is_err(),
            "returning an empty list here would report a file full of tests as \
             a file holding none"
        );
    }
}
