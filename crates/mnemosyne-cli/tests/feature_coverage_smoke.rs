//! Every cargo feature this workspace declares is compiled by somebody.
//!
//! # What this exists because of
//!
//! Round 1081 went looking for waits that end on a clock and found four of
//! them living in test targets that CI HAD NEVER COMPILED. `mnemosyne-server`
//! declares `default = []` and puts its TLS and OTLP surfaces behind `tls` and
//! `otlp`; every test file for them opens with `#![cfg(feature = ...)]`. Under
//! `cargo test --workspace` — what the `validate` job has run since R583 —
//! six integration test targets printed `running 0 tests`. Nine tests, none of
//! which had ever run, and a `sleep(50ms)` under the comment "tiny grace"
//! survives indefinitely when nothing executes it.
//!
//! A feature nobody compiles hides whatever is behind it. That is the whole
//! claim, and it is one a program can check.
//!
//! # How the two sides are asked
//!
//! Neither side is a list somebody typed here:
//!
//! - DECLARED — `cargo metadata --no-deps`, `packages[].features` for every
//!   workspace member. A feature added tomorrow is in this set the day it
//!   exists.
//! - COMPILED — the union of two answers. First, `cargo metadata`'s RESOLVE
//!   (`resolve.nodes[].features`): what the default workspace build actually
//!   turns on, which is how `mnemosyne-atomic/schemars` is covered — nothing
//!   names it, `mnemosyne-mcp` depends on it with the feature enabled and
//!   unification does the rest. Second, this repository's own CI: every
//!   `run:` step of every job of every tracked workflow, read for the cargo
//!   invocations that enable features.
//!
//! Anything declared and in neither is a feature this repository compiles
//! nowhere, and it is a defect rather than a note: R777, R783 and R1080 each
//! closed the same shape one level up, and each found something on its first
//! run because a list that restates the tree drifts from it in silence.

use crate::common;
use ci_plan as ci;
use ci_plan::issue::{self, Tree};

use std::collections::{BTreeMap, BTreeSet};
use std::process::Command;
use std::sync::OnceLock;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Metadata {
    packages: Vec<MetaPackage>,
    workspace_members: Vec<String>,
    #[serde(default)]
    resolve: Option<Resolve>,
}

#[derive(Debug, Deserialize)]
struct MetaPackage {
    id: String,
    name: String,
    manifest_path: std::path::PathBuf,
    #[serde(default)]
    features: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct Resolve {
    nodes: Vec<ResolveNode>,
}

#[derive(Debug, Deserialize)]
struct ResolveNode {
    id: String,
    #[serde(default)]
    features: Vec<String>,
}

/// EVERY workspace this repository has, not only the one CI's main job builds.
///
/// `bench/`, `studio/` and each `tools/*` carry their own `[workspace]` so the
/// root gates never compile them, and that separation is exactly where this
/// repository has twice found a check covering only the root: R1079 put the
/// item-citation gate into all six, R1080 enrolled the two trees the hooks
/// call. A feature declared in one of those and enabled by nobody is the same
/// darkness one directory over.
///
/// WHICH of them can be asked on THIS machine is `scripts/check-side-workspaces.sh
/// --list`'s answer, consumed rather than restated.
///
/// The first version of this test wrote its own discovery and turned main red on
/// the first push: `studio` depends on a sibling `../pinion` checkout, which
/// exists on this project's machine and not on a CI runner, so `cargo metadata`
/// for it dies there. That script had solved exactly this — R1079 named the
/// class ("a gate that fails on somebody else's file is a gate that gets
/// ignored") and gave it the `missing_siblings` walk — and asking it is what
/// keeps the two from drifting, which is R1066's correction for fmt and clippy.
///
/// Reading the lister's output is `ci_plan`'s job, not this file's: R1084 made
/// a third gate ask the same question, and a third parser of the same lines is
/// a third answer.
fn workspaces() -> ci::Workspaces {
    let listed = ci::workspaces(&common::repo_root());
    // The skipped ones are PRINTED, never silently dropped: a workspace nobody
    // could ask about is a fact, and a fact that does not appear in the output
    // is one nobody can act on.
    for skipped in &listed.skipped {
        println!("{}", skipped.was_not("asked"));
    }
    listed
}

fn workspace_manifests() -> Vec<String> {
    workspaces().askable
}

/// The workspaces whose lockfile is not this repository's to pin, asked once.
///
/// A `git` call and a lister run per `cargo metadata` would be three of each for
/// every workspace; the answer is a fact about the checkout and does not move
/// under this test.
fn cannot_be_pinned() -> &'static BTreeSet<String> {
    static ANSWER: OnceLock<BTreeSet<String>> = OnceLock::new();
    ANSWER.get_or_init(|| {
        let found = ci::workspaces_this_repository_cannot_pin(&common::repo_root());
        // PRINTED, because whether this branch is taken depends on the MACHINE:
        // a workspace path-depending on a sibling checkout is skipped entirely
        // where that sibling is absent, so a run on a build machine exercises
        // nothing here and says so by printing an empty set.
        println!(
            "[feature-coverage] {} workspace(s) asked without `--locked`, their \
             lockfile not being this repository's: {found:?}",
            found.len()
        );
        found
    })
}

fn metadata(manifest: &str, with_deps: bool) -> Metadata {
    // `--locked` EXCEPT OVER A LOCKFILE THIS REPOSITORY DOES NOT OWN, and the
    // exception is R1263's. The arm that asks for dependencies RESOLVES, so
    // without the flag a lockfile that disagreed with its manifests would be
    // rewritten by this test rather than reported, and the suite reading the
    // tree afterwards would read the repair. But `studio` path-depends on a
    // sibling checkout: its `Cargo.lock` is gitignored and moves whenever that
    // sibling moves its own pins, so `--locked` there is a gate another
    // repository's commit turns red — `LockVerdict::PinsWhatItDoesNotOwn`, which
    // the earlier comment here reasoned past by asking whether the MANIFEST was
    // tracked. It is; the lockfile is not.
    let directory = manifest.strip_suffix("/Cargo.toml").unwrap_or_default();
    let mut command = issue::cargo(Tree::PinnedWhenItIsOurs(
        "every workspace this repository tracks is asked, and one of them \
         resolves against a sibling checkout — so the flag below is present \
         exactly where the lockfile is this repository's",
    ));
    command
        .args(["metadata", "--format-version", "1"])
        .arg("--manifest-path")
        .arg(common::repo_root().join(manifest))
        .current_dir(common::repo_root());
    if !cannot_be_pinned().contains(directory) {
        command.arg("--locked");
    }
    if !with_deps {
        command.arg("--no-deps");
    }
    let out = command.output().expect("cargo metadata runs");
    assert!(
        out.status.success(),
        "cargo metadata failed for {manifest}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("cargo metadata is the JSON this expects")
}

/// `<package>/<feature>` for every feature every workspace member declares,
/// minus `default` (which is on by definition) and minus the implicit
/// `dep:`-style features cargo synthesises for optional dependencies, which are
/// not surfaces anybody writes `#[cfg(feature = ...)]` against.
fn declared() -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for manifest in workspace_manifests() {
        out.extend(declared_in(&manifest));
    }
    out
}

/// What ONE workspace's members declare.
fn declared_in(manifest: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    {
        let meta = metadata(manifest, false);
        let members: BTreeSet<&String> = meta.workspace_members.iter().collect();
        for package in &meta.packages {
            if !members.contains(&package.id) {
                continue;
            }
            for feature in package.features.keys() {
                if feature == "default" {
                    continue;
                }
                out.insert(format!("{}/{}", package.name, feature));
            }
        }
    }
    out
}

/// `<package>/<feature>` for everything the DEFAULT workspace build turns on.
/// Cargo answers this; deriving it from the manifests by hand is re-writing
/// feature unification, which is the part nobody gets right twice.
fn enabled_by_the_default_build() -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for manifest in workspace_manifests() {
        let meta = metadata(&manifest, true);
        let members: BTreeSet<&String> = meta.workspace_members.iter().collect();
        let name_of: BTreeMap<&String, &String> =
            meta.packages.iter().map(|p| (&p.id, &p.name)).collect();
        let resolve = meta
            .resolve
            .expect("`cargo metadata` with deps carries a resolve");
        for node in &resolve.nodes {
            if !members.contains(&node.id) {
                continue;
            }
            let Some(name) = name_of.get(&node.id) else {
                continue;
            };
            for feature in &node.features {
                if feature == "default" {
                    continue;
                }
                out.insert(format!("{name}/{feature}"));
            }
        }
    }
    out
}

/// `<package>/<feature>` for everything this repository's own CI compiles,
/// read out of the workflows rather than remembered.
///
/// Two shapes are understood, because two are what the workflows use:
/// `--all-features` (with `-p <pkg>`, every feature of that package; without
/// it, every feature of every member) and `--features a,b` / `--features
/// pkg/feat`.
fn enabled_by_ci() -> BTreeMap<String, String> {
    let root = common::repo_root();
    let declared = declared();
    let root_declared = declared_in("Cargo.toml");
    let mut out = BTreeMap::new();
    for command in ci::workflow_cargo_commands(&root) {
        let where_it_runs = command.origin();
        for feature in features_enabled(&command, &declared, &root_declared) {
            out.entry(feature).or_insert_with(|| where_it_runs.clone());
        }
    }
    out
}

/// The features one command enables.
///
/// `declared` scopes a `-p <pkg> --all-features`; `root_declared` scopes an
/// `--all-features` with no package, because such a command builds THE ROOT
/// WORKSPACE and crediting a separate workspace's features to it would be the
/// gate lying in the generous direction. A shell script's `--manifest-path
/// "$ws/Cargo.toml"` cannot be resolved statically at all, which is why the CI
/// side reads workflow steps and not `scripts/` — a separate workspace that
/// grows a feature is therefore DARK until a job names it, and that is the
/// intended answer rather than a gap.
fn features_enabled(
    command: &ci::CargoCommand,
    declared: &BTreeSet<String>,
    root_declared: &BTreeSet<String>,
) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let package = command.value(&["-p", "--package"]).map(str::to_string);

    if command.has("--all-features") {
        match &package {
            Some(only) => {
                for feature in declared {
                    if feature.split('/').next() == Some(only.as_str()) {
                        out.insert(feature.clone());
                    }
                }
            }
            None => out.extend(root_declared.iter().cloned()),
        }
    }

    if let Some(list) = command.value(&["--features"]) {
        for item in list.split(',') {
            let item = item.trim();
            if item.is_empty() {
                continue;
            }
            if item.contains('/') {
                out.insert(item.to_string());
            } else if let Some(owner) = &package {
                out.insert(format!("{owner}/{item}"));
            }
        }
    }
    out
}

#[test]
fn every_feature_this_workspace_declares_is_compiled_by_somebody() {
    let declared = declared();
    // NON-VACUITY FIRST. A run over an empty declared set would pass while
    // checking nothing, and that is what this gate exists to make impossible
    // one level down.
    assert!(
        !declared.is_empty(),
        "no workspace member declares a feature — either cargo's answer changed \
         shape or this gate is pointed at the wrong tree"
    );

    let by_default = enabled_by_the_default_build();
    let by_ci = enabled_by_ci();

    let mut dark: Vec<&String> = declared
        .iter()
        .filter(|f| !by_default.contains(*f) && !by_ci.contains_key(*f))
        .collect();
    dark.sort();

    println!("declared {}: {declared:?}", declared.len());
    println!(
        "compiled by the default workspace build {}: {:?}",
        by_default.len(),
        by_default
    );
    for (feature, where_it_runs) in &by_ci {
        println!("compiled by CI: {feature} — {where_it_runs}");
    }

    assert!(
        dark.is_empty(),
        "these features are declared and compiled NOWHERE — not by the default \
         workspace build, not by any cargo command in this repository's \
         workflows. Whatever is behind them is invisible to every gate: R1081 \
         found four waits that end on a clock inside six test targets that were \
         dark exactly this way, and nine tests that had never run in CI.\n  \
         {dark:?}\n  fix: run them from a job (`--all-features` for a package \
         is the self-maintaining form), or delete the feature."
    );
}

#[test]
fn the_two_sides_are_asked_of_the_program_and_both_answer() {
    // The reach assertion. A gate whose CI side silently read zero workflows,
    // or whose resolve side came back empty, would report "nothing dark" for a
    // tree it never looked at — R1054's shape, and R1078 shipped it once.
    let by_default = enabled_by_the_default_build();
    let by_ci = enabled_by_ci();

    assert!(
        !by_default.is_empty(),
        "cargo's resolve turned on no member feature at all, which cannot be \
         true while `mnemosyne-mcp` depends on `mnemosyne-core` with `schemars` \
         enabled — the resolve side of this gate is not reading what it thinks"
    );
    assert!(
        !by_ci.is_empty(),
        "no cargo command in any workflow enables any declared feature, which \
         cannot be true while a job runs `-p mnemosyne-server --all-features` — \
         the CI side of this gate is not reading what it thinks"
    );
    assert!(
        by_ci.keys().any(|f| !by_default.contains(f)),
        "every feature CI enables is one the default build already had, so the \
         CI side has never been the reason anything passed — it would pass \
         unread"
    );
}

#[test]
fn every_tracked_manifest_is_inside_a_workspace_this_gate_asks() {
    // TOTALITY. Extending the population from "the root workspace" to "every
    // workspace" is only worth anything if nothing falls between them, and a
    // crate in no asked workspace is a crate whose features are dark by
    // construction. This is R1080's rule at the manifest level: a tree merely
    // absent looks exactly like a tree deliberately excluded.
    let root = common::repo_root();
    let listed = workspaces();
    let (manifests, skipped) = (listed.askable, listed.skipped);
    assert!(
        manifests.len() > 1 || !skipped.is_empty(),
        "this repository has separate workspaces; a census that found only the \
         root one is not reading them: {manifests:?}"
    );

    let mut asked: BTreeSet<String> = manifests.iter().cloned().collect();
    for manifest in &manifests {
        let meta = metadata(manifest, false);
        let members: BTreeSet<&String> = meta.workspace_members.iter().collect();
        for package in &meta.packages {
            if !members.contains(&package.id) {
                continue;
            }
            if let Ok(relative) = package.manifest_path.strip_prefix(&root) {
                asked.insert(relative.display().to_string());
            }
        }
    }

    let tracked = Command::new("git")
        .args(["ls-files", "*Cargo.toml"])
        .current_dir(&root)
        .output()
        .expect("git ls-files runs");
    // A manifest under a SKIPPED workspace is accounted for: the lister said,
    // out loud and with a reason, why nobody can ask about it on this machine.
    // That is a different thing from a manifest nobody's list mentions, which
    // is what this test rejects.
    //
    // The directory is a FIELD of what the lister said, not the first word of
    // its sentence: this test split the reason apart to get at it until the
    // gate one level up needed the same thing per file and the split became a
    // second reader of the same line.
    let under_a_skip = |path: &str| {
        skipped
            .iter()
            .any(|ws| path.starts_with(&format!("{}/", ws.directory)))
    };
    let missed: Vec<String> = String::from_utf8_lossy(&tracked.stdout)
        .lines()
        .filter(|path| !asked.contains(*path) && !under_a_skip(path))
        .map(str::to_string)
        .collect();

    println!(
        "{} workspace(s) asked: {manifests:?}; {} skipped with a reason",
        manifests.len(),
        skipped.len()
    );
    assert!(
        missed.is_empty(),
        "these tracked manifests belong to no workspace this gate asks and to \
         no workspace the lister explained away, so whatever features they \
         declare are outside the census:\n  {missed:?}"
    );
}

#[test]
fn a_command_is_read_for_the_features_it_actually_enables() {
    // The parsing rules, pinned against strings rather than against the tree,
    // so that a workflow edit and a parser change cannot both drift into
    // agreement. Each case is a shape this repository's workflows use or could.
    let declared: BTreeSet<String> = ["server/tls", "server/otlp", "core/schemars"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    // Through the SAME reader the live census uses, so a change to how a shell
    // line becomes a cargo command is checked here rather than only in the tree.
    let read = |line: &str| match ci::parse_script(line).into_iter().next() {
        Some(found) => features_enabled(
            &ci::CargoCommand {
                source: "pinned".to_string(),
                owner: "pinned".to_string(),
                carrier: found.carrier,
                cargo_args: found.cargo_args,
                harness_args: found.harness_args,
                env: Default::default(),
                declared: None,
                site: None,
                uncounted: Vec::new(),
            },
            &declared,
            &declared,
        ),
        None => BTreeSet::new(),
    };

    assert_eq!(
        read("cargo test -p server --all-features --locked"),
        ["server/otlp", "server/tls"]
            .iter()
            .map(|s| s.to_string())
            .collect::<BTreeSet<_>>(),
        "`-p` scopes `--all-features` to that package"
    );
    assert_eq!(
        read("cargo test --workspace --all-features"),
        declared,
        "without `-p`, `--all-features` reaches every member"
    );
    assert_eq!(
        read("cargo test --workspace --features server/otlp,server/tls"),
        ["server/otlp", "server/tls"]
            .iter()
            .map(|s| s.to_string())
            .collect::<BTreeSet<_>>(),
        "package-qualified names are taken as written"
    );
    assert_eq!(
        read("cargo test -p server --features otlp"),
        ["server/otlp"]
            .iter()
            .map(|s| s.to_string())
            .collect::<BTreeSet<_>>(),
        "a bare name belongs to the `-p` package"
    );
    assert!(
        read("cargo test --workspace --locked").is_empty(),
        "a command with no feature flag enables no feature"
    );
    assert!(
        read("sudo apt-get install -y protobuf-compiler").is_empty(),
        "a step that is not cargo says nothing about features"
    );
    assert!(
        ci::parse_script("sudo apt-get install -y protobuf-compiler").is_empty(),
        "and it is not a cargo command at all — the emptiness above must come \
         from the reader, not from a command it read and found nothing in"
    );
}
