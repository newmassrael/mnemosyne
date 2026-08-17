//! The gate for tests that exist and that nothing executes.
//!
//! # The law
//!
//! **Every test this repository compiles is one some CI command runs.**
//!
//! # What it exists because of
//!
//! R1082 closed the level below this one: every cargo feature this repository
//! declares is compiled by somebody. That is a claim about COMPILATION, and
//! compilation is not execution — `--all-features` keeps passing after every
//! test behind those features is deleted. The defect R1081 found was of this
//! kind and was found by hand: six `mnemosyne-server` integration targets
//! printed `running 0 tests` under the workspace suite, nine tests had never
//! run in CI, and four waits that end on a clock had survived indefinitely
//! inside them because nothing executed them. The instrument was free — the
//! suite prints `running 0 tests` on every push — and nobody was counting.
//!
//! Counting that line is not enough to be a rule, and that is what this gate
//! settles. Three binary unittest targets legitimately run zero tests: they
//! contain none. So the rule cannot be "every target runs something", it has to
//! be "every test somebody wrote is run", and the thing that makes it checkable
//! is that BOTH SIDES ARE ASKED OF THE MACHINE:
//!
//! - POPULATION — for every workspace the lister says this machine can build,
//!   `cargo test --workspace --all-features --no-run`, then each test binary's
//!   own `--list`. Cargo says which targets exist; the harness says which tests
//!   are in them. Neither is a list anybody typed.
//! - RUN — every `cargo test` this repository's CI actually issues, re-issued
//!   with `--list` appended to ITS OWN harness arguments. A command is not
//!   modelled, it is asked: whatever `-p`, `--test`, `--features` or `--ignored`
//!   it carries, cargo and the harness resolve them exactly as they do in CI.
//!
//! Anything in the first set and not the second is a test this repository
//! compiles and nobody runs.
//!
//! # Why `--ignored` is a separate probe
//!
//! `--list` lists ignored tests too — measured, not assumed: a binary with 39
//! tests and one `#[ignore]` prints 39 for `--list` and 1 for `--list
//! --ignored`. So the set a command LISTS is not the set it RUNS, and the
//! difference is exactly the ignored ones. A command that itself selects
//! `--ignored` or `--include-ignored` runs what it lists; every other command
//! runs its list minus its ignored list. Without that subtraction an
//! `#[ignore]` nobody ever passes `--ignored` for would read as covered, which
//! is the same silence this gate exists to break.
//!
//! # Why it reports what it reached
//!
//! An empty finding list is what a covered repository looks like AND what a
//! repository nobody probed looks like. R1078 shipped the second while
//! believing the first, so [`Report::verdict`] REFUSES rather than passing when
//! a probe failed to build, when the population is empty, or when CI names no
//! test command at all.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

use ci_plan::CargoCommand;
use serde::Deserialize;

/// One test, identified the way the machine identifies it: the source file of
/// the target it lives in, and the name its harness prints.
///
/// The target's source path rather than its name, because two packages may each
/// have a `tests/smoke.rs` and a bare target name would silently merge them.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TestId {
    /// Target source, relative to the repository root when it is inside it.
    pub target_src: PathBuf,
    /// The test's full name as the harness prints it, `module::path::fn`.
    pub name: String,
}

impl fmt::Display for TestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.target_src.display(), self.name)
    }
}

/// One test binary cargo built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestBinary {
    pub package: String,
    pub kind: String,
    pub name: String,
    pub target_src: PathBuf,
    pub executable: PathBuf,
}

/// What one probe — a population build or a CI command — turned out to hold.
#[derive(Debug, Clone)]
pub struct Probe {
    /// How the probe reads back, for the gate's own output.
    pub label: String,
    /// Where the command is written, for the gate's own output.
    pub origin: String,
    /// Binaries cargo built for it.
    pub binaries: Vec<TestBinary>,
    /// Tests those binaries hold.
    pub listed: BTreeSet<TestId>,
    /// Of those, the ones this probe would actually execute. For a population
    /// probe this is the same as `listed`; for a CI command it is `listed`
    /// minus what the command's own flags leave ignored.
    pub executed: BTreeSet<TestId>,
}

/// Why the gate declined to give a verdict at all.
///
/// Separate from "tests found dark", because the two demand different actions
/// and look identical in an exit code alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// A probe's build did not succeed, so part of the tree is unread.
    BuildFailed { label: String, message: String },
    /// A probe built nothing that holds a test, so it says nothing.
    NothingBuilt { label: String },
    /// A test binary refused to list its tests.
    ListFailed { label: String, message: String },
    /// No workspace held a single test — cargo's answer changed shape, or the
    /// gate is pointed at the wrong tree.
    EmptyPopulation,
    /// This repository's CI issues no `cargo test` at all, which cannot be true
    /// and means the workflow side read nothing.
    NoTestCommands,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::BuildFailed { label, message } => write!(
                f,
                "`{label}` did not build, so this gate read only part of the \
                 tree — a clean answer and an answer about nothing are the same \
                 answer, so this is not one:\n{message}"
            ),
            Refusal::NothingBuilt { label } => write!(
                f,
                "`{label}` built no test binary at all, so whatever it covers is \
                 unknown rather than empty"
            ),
            Refusal::ListFailed { label, message } => write!(
                f,
                "a test binary of `{label}` would not list its tests: {message}"
            ),
            Refusal::EmptyPopulation => f.write_str(
                "not one workspace holds a test — either cargo's answer changed \
                 shape or this gate is pointed at the wrong tree",
            ),
            Refusal::NoTestCommands => f.write_str(
                "no `cargo test` was found in any workflow or in the workspace \
                 lister, which cannot be true while this repository's suite runs \
                 on every push — the CI side is not reading what it thinks",
            ),
        }
    }
}

/// The gate's answer.
///
/// `Default` is here so the two answers it carries — "could I judge" and "what
/// did I find" — can be tested without a build. They are the whole verdict and
/// they had no test until an injection sweep went looking for one to break.
#[derive(Debug, Clone, Default)]
pub struct Report {
    pub root: PathBuf,
    /// The population probes, one per workspace this machine can build.
    pub population_probes: Vec<Probe>,
    /// Workspaces the lister declined, with the reason it gave.
    pub skipped_workspaces: Vec<ci_plan::SkippedWorkspace>,
    /// The CI commands that run tests, probed.
    pub ci_probes: Vec<Probe>,
    /// Cargo commands CI issues that are not `cargo test`, counted so that a
    /// zero here is visible rather than assumed.
    pub non_test_commands: usize,
    /// Every test this repository compiles.
    pub population: BTreeSet<TestId>,
    /// Every test some CI command executes, with one command that does.
    pub run: BTreeMap<TestId, String>,
    pub refusals: Vec<Refusal>,
}

impl Report {
    /// `Ok(())` when the gate could judge, `Err` when it could not. Findings are
    /// read separately: a report can be judgeable AND defective.
    pub fn verdict(&self) -> Result<(), &[Refusal]> {
        if self.refusals.is_empty() {
            Ok(())
        } else {
            Err(&self.refusals)
        }
    }

    /// Tests this repository compiles that no CI command runs.
    pub fn dark(&self) -> Vec<&TestId> {
        self.population
            .iter()
            .filter(|test| !self.run.contains_key(*test))
            .collect()
    }
}

// --- asking cargo -----------------------------------------------------------

fn cargo() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string())
}

#[derive(Debug, Deserialize)]
struct Artifact {
    reason: String,
    #[serde(default)]
    package_id: String,
    #[serde(default)]
    target: Option<ArtifactTarget>,
    #[serde(default)]
    profile: Option<ArtifactProfile>,
    #[serde(default)]
    executable: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct ArtifactTarget {
    kind: Vec<String>,
    name: String,
    src_path: PathBuf,
}

#[derive(Debug, Deserialize)]
struct ArtifactProfile {
    test: bool,
}

/// Build a cargo command's test binaries WITHOUT running them, and read which
/// they are out of cargo's own artifact record rather than out of a path this
/// gate derived. R1078 established that as the way to know what a gate reached.
pub fn build_test_binaries(root: &Path, cargo_args: &[String]) -> Result<Vec<TestBinary>, String> {
    let mut command = Command::new(cargo());
    command
        .args(cargo_args.iter().skip(1))
        .arg("--no-run")
        .arg("--message-format=json")
        .current_dir(root);
    let output = command
        .output()
        .map_err(|e| format!("could not run cargo: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let mut binaries = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let Ok(artifact) = serde_json::from_str::<Artifact>(line) else {
            continue;
        };
        if artifact.reason != "compiler-artifact" {
            continue;
        }
        let (Some(target), Some(profile), Some(executable)) =
            (artifact.target, artifact.profile, artifact.executable)
        else {
            continue;
        };
        if !profile.test {
            continue;
        }
        binaries.push(TestBinary {
            package: package_name(&artifact.package_id),
            kind: target.kind.first().cloned().unwrap_or_default(),
            name: target.name,
            target_src: relative(root, &target.src_path),
            executable,
        });
    }
    binaries.sort_by(|a, b| (&a.target_src, &a.name).cmp(&(&b.target_src, &b.name)));
    binaries.dedup();
    Ok(binaries)
}

/// `path+file:///…/crates/mnemosyne-cli#0.1.0` and
/// `path+file:///…#mnemosyne-cli@0.1.0` are both shapes cargo has used; the
/// name is only ever wanted for the gate's own printing, so the whole id is
/// kept when neither shape reads.
fn package_name(id: &str) -> String {
    let tail = id.rsplit('#').next().unwrap_or(id);
    if let Some((name, _)) = tail.split_once('@') {
        return name.to_string();
    }
    if tail.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        // `…/mnemosyne-cli#0.1.0` — the name is the last path component.
        let head = id.split('#').next().unwrap_or(id);
        return head.rsplit('/').next().unwrap_or(head).to_string();
    }
    tail.to_string()
}

fn relative(root: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(root).unwrap_or(path).to_path_buf()
}

/// Ask one test binary which tests it holds, under the harness arguments the
/// command carries. `--list` prints every test INCLUDING the ignored ones, so
/// what this returns is what the command can see, not yet what it runs.
pub fn list_tests(
    binary: &TestBinary,
    harness_args: &[String],
) -> Result<BTreeSet<String>, String> {
    let output = Command::new(&binary.executable)
        .args(harness_args)
        .arg("--list")
        .output()
        .map_err(|e| format!("could not run {}: {e}", binary.executable.display()))?;
    if !output.status.success() {
        return Err(format!(
            "{} exited {}: {}",
            binary.executable.display(),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.strip_suffix(": test"))
        .map(str::to_string)
        .collect())
}

/// Does this command select the ignored tests itself?
fn selects_ignored(command: &CargoCommand) -> bool {
    command.harness_has("--ignored") || command.harness_has("--include-ignored")
}

/// Target selectors, any of which narrows a `cargo test` to something that is
/// not a doc-test. `--all-targets` is one of them: it means lib, bins, tests,
/// benches and examples, and cargo documents that it does NOT include doc-tests.
const NARROWS_AWAY_FROM_DOC: &[&str] = &[
    "--test",
    "--tests",
    "--lib",
    "--bin",
    "--bins",
    "--example",
    "--examples",
    "--bench",
    "--benches",
    "--all-targets",
];

/// Does this command run the doc-tests?
///
/// It matters because a doc-test is a test with NO test binary — cargo hands
/// the examples to rustdoc — so the artifact record every other probe here
/// reads says nothing about them. This repository has sixteen, and the first
/// version of this gate could not see one of them: its population and its run
/// set were both short by the same amount, which is the shape where a hole
/// hides because it is symmetric.
fn runs_doc_tests(command: &CargoCommand) -> bool {
    !NARROWS_AWAY_FROM_DOC
        .iter()
        .any(|selector| command.has(selector))
}

/// The directory of the workspace a command builds, relative to the repository
/// root. Doc-test names are printed relative to their own workspace root, so
/// this is what makes `bench`'s `crates/x/src/lib.rs` and the root's distinct.
fn workspace_prefix(command: &CargoCommand) -> PathBuf {
    command
        .value(&["--manifest-path"])
        .map(Path::new)
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_default()
}

#[derive(Debug, Deserialize)]
struct Metadata {
    packages: Vec<MetaPackage>,
    workspace_members: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct MetaPackage {
    id: String,
    name: String,
    targets: Vec<MetaTarget>,
}

#[derive(Debug, Deserialize)]
struct MetaTarget {
    kind: Vec<String>,
}

/// Is there a library in what this command builds?
///
/// `cargo test --doc` on a package with no library exits non-zero saying so,
/// and reading that as a refusal makes this gate reject for a reason outside
/// its own law — the shape R1081 shipped and repaired within the round. A
/// bin-only package having no doc-tests is a COMPLETE answer, so the question
/// is asked BEFORE the probe, of `cargo metadata` rather than of an error
/// message: a string cargo is free to reword is not an authority.
fn has_library(root: &Path, command: &CargoCommand) -> Result<bool, String> {
    let mut invocation = Command::new(cargo());
    invocation
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(root);
    if let Some(manifest) = command.value(&["--manifest-path"]) {
        invocation.arg("--manifest-path").arg(manifest);
    }
    let output = invocation
        .output()
        .map_err(|e| format!("could not run cargo metadata: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let meta: Metadata = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("cargo metadata is not the JSON this expects: {e}"))?;
    let members: BTreeSet<&String> = meta.workspace_members.iter().collect();
    let only = command.value(&["-p", "--package"]);
    Ok(meta
        .packages
        .iter()
        .filter(|package| members.contains(&package.id))
        .filter(|package| only.is_none_or(|name| name == package.name))
        .flat_map(|package| package.targets.iter())
        .any(|target| {
            target
                .kind
                .iter()
                .any(|kind| kind == "lib" || kind == "rlib" || kind == "proc-macro")
        }))
}

/// Ask cargo which doc-tests one command holds. Unlike every other probe here
/// this one cannot go through a binary, so it reads cargo's own listing.
fn list_doc_tests(
    root: &Path,
    command: &CargoCommand,
    extra_harness: &[&str],
) -> Result<BTreeSet<TestId>, String> {
    let mut invocation = Command::new(cargo());
    invocation
        .args(command.cargo_args.iter().skip(1))
        .arg("--doc")
        .arg("--")
        .args(&command.harness_args)
        .args(extra_harness)
        .arg("--list")
        .current_dir(root);
    let output = invocation
        .output()
        .map_err(|e| format!("could not run cargo: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let prefix = workspace_prefix(command);
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.strip_suffix(": test"))
        .map(|name| {
            // `crates/x/src/lib.rs - item (line 41)` — the source is the part
            // before the separator, and the whole string is the test's name.
            let source = name.split(" - ").next().unwrap_or(name).trim();
            TestId {
                target_src: prefix.join(source),
                name: name.to_string(),
            }
        })
        .collect())
}

/// Build one command's binaries and ask each what it lists and what it runs.
pub fn probe(root: &Path, command: &CargoCommand, refusals: &mut Vec<Refusal>) -> Option<Probe> {
    let label = command.rendered();
    let binaries = match build_test_binaries(root, &command.cargo_args) {
        Ok(binaries) => binaries,
        Err(message) => {
            refusals.push(Refusal::BuildFailed {
                label: label.clone(),
                message,
            });
            return None;
        }
    };
    if binaries.is_empty() {
        refusals.push(Refusal::NothingBuilt {
            label: label.clone(),
        });
        return None;
    }

    let mut listed = BTreeSet::new();
    let mut ignored = BTreeSet::new();
    for binary in &binaries {
        let names = match list_tests(binary, &command.harness_args) {
            Ok(names) => names,
            Err(message) => {
                refusals.push(Refusal::ListFailed {
                    label: label.clone(),
                    message,
                });
                return None;
            }
        };
        for name in names {
            listed.insert(TestId {
                target_src: binary.target_src.clone(),
                name,
            });
        }
        if selects_ignored(command) {
            continue;
        }
        // THE SUBTRACTION. Everything this binary would skip for being
        // `#[ignore]`d under exactly these arguments.
        let mut ignored_args = command.harness_args.clone();
        ignored_args.push("--ignored".to_string());
        let names = match list_tests(binary, &ignored_args) {
            Ok(names) => names,
            Err(message) => {
                refusals.push(Refusal::ListFailed {
                    label: label.clone(),
                    message,
                });
                return None;
            }
        };
        for name in names {
            ignored.insert(TestId {
                target_src: binary.target_src.clone(),
                name,
            });
        }
    }

    // THE TESTS WITH NO BINARY. Everything above reads cargo's artifact record,
    // which never mentions a doc-test, so they are asked for separately or they
    // are in neither side of this gate at once — invisible symmetrically, which
    // is a hole that cannot show up as a finding.
    let library = match has_library(root, command) {
        Ok(library) => library,
        Err(message) => {
            refusals.push(Refusal::BuildFailed {
                label: label.clone(),
                message,
            });
            return None;
        }
    };
    if runs_doc_tests(command) && library {
        match list_doc_tests(root, command, &[]) {
            Ok(names) => listed.extend(names),
            Err(message) => {
                refusals.push(Refusal::ListFailed {
                    label: label.clone(),
                    message,
                });
                return None;
            }
        }
        if !selects_ignored(command) {
            match list_doc_tests(root, command, &["--ignored"]) {
                Ok(names) => ignored.extend(names),
                Err(message) => {
                    refusals.push(Refusal::ListFailed {
                        label: label.clone(),
                        message,
                    });
                    return None;
                }
            }
        }
    }

    let executed = listed.difference(&ignored).cloned().collect();
    Some(Probe {
        label,
        origin: command.origin(),
        binaries,
        listed,
        executed,
    })
}

// --- the run ----------------------------------------------------------------

/// The population probe for one workspace: everything it can compile.
///
/// `--all-features` is the load-bearing flag — it is what makes a test hiding
/// behind a feature part of the population instead of part of the silence — so
/// it is pinned by a test rather than left to a reader to notice.
pub fn population_command(manifest: &str) -> CargoCommand {
    CargoCommand {
        source: "scripts/check-side-workspaces.sh".to_string(),
        owner: manifest.to_string(),
        cargo_args: [
            "cargo",
            "test",
            "--manifest-path",
            manifest,
            "--workspace",
            "--all-features",
        ]
        .iter()
        .map(|word| (*word).to_string())
        .collect(),
        // A probe this gate builds for itself is issued directly; nothing hands
        // it over.
        carrier: Vec::new(),
        harness_args: Vec::new(),
        env: Default::default(),
    }
}

/// Which workspaces a run draws its population from.
///
/// # Why a caller gets to choose
///
/// The population is "every workspace the lister says this machine can build",
/// and on the machine that holds the sibling checkout that set includes one
/// whose compilation is somebody else's working tree. CI never sees it — no
/// runner has the sibling — so the choice only exists locally, and it is a
/// choice about what the verdict is ABOUT rather than a way to check less:
/// [`Population::Ours`] is what `pre-push` asks, because a push publishes THIS
/// repository's commit and cannot be blocked by whether another repository
/// compiles at this instant (R1225's reason, one gate over).
///
/// Either way what was left out is NAMED — the lister's own sentence, printed
/// through [`ci_plan::SkippedWorkspace::was_not`] — so a green run never means
/// "and something was quietly not looked at".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Population {
    /// Every workspace the lister says this machine can build. What CI runs.
    Whole,
    /// Only the ones whose resolution this repository owns.
    Ours,
}

/// Run the gate over the repository rooted here.
pub fn run(root: &Path, population_of: Population) -> Report {
    let mut refusals = Vec::new();

    let listed = match population_of {
        Population::Whole => ci_plan::workspaces(root),
        Population::Ours => ci_plan::workspaces_ours_only(root),
    };
    let mut population_probes = Vec::new();
    let mut population: BTreeSet<TestId> = BTreeSet::new();
    for manifest in &listed.askable {
        if let Some(found) = probe(root, &population_command(manifest), &mut refusals) {
            population.extend(found.listed.iter().cloned());
            population_probes.push(found);
        }
    }

    let mut commands = ci_plan::workflow_cargo_commands(root);
    commands.extend(ci_plan::lister_suite_commands(&listed));
    let non_test_commands = commands
        .iter()
        .filter(|command| command.subcommand() != Some("test"))
        .count();

    let mut ci_probes = Vec::new();
    let mut run: BTreeMap<TestId, String> = BTreeMap::new();
    for command in commands.iter().filter(|c| c.subcommand() == Some("test")) {
        let Some(found) = probe(root, command, &mut refusals) else {
            continue;
        };
        // A CI command sees tests the population probe may not — a
        // `#[cfg(not(feature = ...))]` test is invisible under
        // `--all-features` — so both sides feed the population.
        population.extend(found.listed.iter().cloned());
        for test in &found.executed {
            run.entry(test.clone())
                .or_insert_with(|| found.origin.clone());
        }
        ci_probes.push(found);
    }

    if population.is_empty() {
        refusals.push(Refusal::EmptyPopulation);
    }
    if ci_probes.is_empty() {
        refusals.push(Refusal::NoTestCommands);
    }

    Report {
        root: root.to_path_buf(),
        population_probes,
        skipped_workspaces: listed.skipped,
        ci_probes,
        non_test_commands,
        population,
        run,
        refusals,
    }
}
