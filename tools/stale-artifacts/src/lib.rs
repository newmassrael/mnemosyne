//! The freshness pass `scripts/verify.sh` runs before it verifies anything.
//!
//! R743 is the round this exists for. Two overlapping cargo runs corrupted one
//! `target/`'s fingerprint cache, cargo decided a crate was up to date when it
//! was not, and a STALE test binary ran and passed — a deterministic regression
//! that read as a flake for as long as it took to find. The repair has two
//! halves: an flock so two runs cannot overlap, and a forced clean of the
//! packages the working tree has changed so that no artifact of the code under
//! test can survive a corruption that already happened. The lock is prevention;
//! this is RECOVERY, and only one of the two works after the fact.
//!
//! WHAT THIS PROGRAM IS FOR, and it is a defect rather than a feature request.
//! The recovery half lived in eight lines of shell that asked git for the files
//! changed under `crates/` and ran `cargo clean -p <directory name>` for each.
//! Both halves of that are a statement about the ROOT workspace: `crates/` is
//! where its members live, and `cargo clean -p` resolves the workspace of the
//! directory it runs in. This repository has twenty-three more workspaces —
//! `bench/`, `studio/`, every `tools/*` — and `scripts/check-side-workspaces.sh`
//! runs all of their suites through that same wrapper. So the wrapper passed
//! `--no-fresh` for them, with a comment saying why, and R743's recovery half
//! reached none of them: a corrupted artifact of a changed `tools/` crate would
//! be read by the very suite meant to judge it, exactly as in R743, with nothing
//! to say so.
//!
//! WHY THIS IS A PROGRAM AND NOT A LONGER SHELL BRANCH. The workspace to clean
//! in is decided at runtime, so the command would have to be
//! `cargo clean -p "$package" --manifest-path "$manifest"` — and `ci-plan`, the
//! one reader this repository's laws use for the cargo commands it issues,
//! answers `Unreadable` for a `--manifest-path` whose literal tail is not a
//! directory. That is the reader working correctly: a command whose workspace is
//! a variable is one no static gate can place, and `locked_resolution_smoke`
//! goes red rather than guessing. Writing the decision in a program keeps the
//! command's workspace a thing the program KNOWS instead of a thing a gate has
//! to recover from a shell string.
//!
//! THE THREE QUESTIONS IT ANSWERS, each asked of the machine that owns it:
//!
//! 1. WHICH WORKSPACE the run will build — from the command's own
//!    `--manifest-path`, read with `ci_plan::cargo_invocation`, which is the
//!    same reading every other gate here uses. No `--manifest-path` means the
//!    workspace of the tree it runs in, which is what cargo itself does.
//! 2. WHICH PACKAGES that workspace has and where they live — from
//!    `cargo metadata --no-deps`, so a package that moved directory, or one
//!    whose name is not its directory's, is still the right answer. The shell it
//!    replaced used the directory name as the package name and was correct only
//!    because `crates/*` happens to spell them the same.
//! 3. WHAT THE TREE HAS CHANGED — from git, against HEAD, over the WHOLE tree.
//!    Untracked files count: a file the working tree holds and HEAD does not is
//!    a difference from HEAD by every reading except the one that costs a
//!    rebuild.
//!
//! AND `--locked` IS THE COMMAND'S OWN ANSWER. `cargo clean` resolves the
//! workspace it is pointed at, so it can rewrite a lockfile it disagrees with —
//! which is the defect `locked_resolution_smoke` exists for. Whether this
//! repository may pin that workspace is a question `check-side-workspaces.sh`
//! already answers (a workspace whose path dependencies leave this checkout is
//! not ours to pin), and asking it a second time here would be a second answer
//! to it. So the rule is local and needs no second reading: the clean resolves
//! exactly what the wrapped command resolves, so it pins exactly when the
//! wrapped command pins.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use ci_plan::issue::{self, Tree};
use ci_plan::{cargo_invocation, CargoCommand};

/// What one run of the freshness pass decided.
///
/// THREE OUTCOMES AND TWO EXIT CODES, which is not the three-code contract the
/// judging gates here share — deliberately, because this program is not a judge.
/// It ACTS. There is no state of the world it can find that is "a finding": a
/// tree with nothing changed is a tree with nothing to clean, and that is the
/// pass succeeding. What it can do is fail to know, and that is the code the
/// caller has to fail on, because a freshness pass that did not run leaves
/// exactly the artifact it exists to remove.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    /// There is nothing for this pass to do, and the reason it is nothing.
    ///
    /// A REASON RATHER THAN AN EMPTY ANSWER. "The command builds no workspace"
    /// and "the workspace has no changed package" are different states of the
    /// world, and a run that printed neither would leave "nothing was wrong" and
    /// "nothing was read" wearing the same silence.
    Nothing(String),
    /// This pass could not be run, and why. NOT a pass with nothing to do.
    Unreadable(String),
    /// These packages of this workspace hold code the tree has changed.
    Freshen(Freshen),
}

/// The packages one run has to clean, and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Freshen {
    /// The manifest `cargo clean` is pointed at — the wrapped command's own
    /// `--manifest-path`, or the tree's root manifest when it has none.
    pub manifest: PathBuf,
    /// Whether the cleans pin the lockfile, which is whether the command does.
    pub locked: bool,
    /// The packages to clean, sorted and unique.
    pub packages: Vec<String>,
    /// How many paths the tree reports changed against HEAD, of which the
    /// packages above are the part this workspace owns. PRINTED EVEN WHEN THE
    /// PACKAGES ARE NONE: a workspace whose files nobody touched and a git that
    /// answered nothing at all produce the same empty list and are not the same
    /// state.
    pub changed: usize,
    /// How many packages of that resolve live in this repository at all, which
    /// is the other half of the same sentence: nothing to clean out of forty is
    /// a working pass, and nothing to clean out of zero is cargo having answered
    /// about nothing.
    pub members: usize,
}

/// What this tree can be asked about what it has changed.
enum Difference {
    /// Paths differing from HEAD, relative to the repository root, which is
    /// also given because it need not be the directory this was asked about.
    Changed { root: PathBuf, paths: Vec<String> },
    /// It is not a git repository, so it has no HEAD to differ from.
    NoRepository,
}

/// One workspace member, as cargo describes it.
struct Member {
    name: String,
    /// The directory holding its manifest, canonicalised.
    directory: PathBuf,
}

/// Decide what a run of `command` in `at` has to clean before it can be trusted.
pub fn plan(at: &Path, command: &[String]) -> Plan {
    let Some(invocation) = cargo_invocation(command) else {
        return Plan::Nothing(format!(
            "`{}` runs no cargo command, so there is no workspace whose \
             artifacts a stale one could come from",
            command.join(" ")
        ));
    };
    // THE SAME TYPE EVERY OTHER GATE READS A COMMAND THROUGH. `--manifest-path`
    // and `--locked` both have two spellings (`--flag value`, `--flag=value`)
    // and `CargoCommand` is where this repository knows that; a reader written
    // here would answer "absent" for the joined form, which is how a gate
    // silently starts freshening the wrong workspace.
    let read = CargoCommand {
        source: "the wrapped command".to_string(),
        owner: "the run being verified".to_string(),
        carrier: invocation.carrier,
        cargo_args: invocation.cargo_args,
        harness_args: invocation.harness_args,
        env: BTreeMap::new(),
        // The wrapped command's own words, read as written — this is not a site
        // declaring anything about a tree.
        declared: None,
        site: None,
        uncounted: Vec::new(),
    };
    // AND A COMMAND THAT MEETS NO ARTIFACT NEEDS NO CLEAN IN FRONT OF IT
    // (R1272). This pass exists for R743's stale binary — a run reading a
    // compiled artifact older than the source it was built from — and a
    // subcommand that neither produces nor reads one cannot have that problem.
    // Cleaning in front of it buys nothing and costs whoever compiles next a
    // full rebuild of every package the tree has changed.
    //
    // ONLY A MEASURED `false` SKIPS. `ci_plan::compiles` answers `None` for a
    // subcommand nobody measured, and the conservative reading of that is to
    // clean: the price of freshening a command that did not need it is a
    // rebuild, and the price of skipping one that did is the artifact this
    // program exists to remove surviving into the run it was supposed to make
    // trustworthy.
    if let Some(subcommand) = read.subcommand() {
        if ci_plan::compiles(subcommand) == Some(false) {
            return Plan::Nothing(format!(
                "`cargo {subcommand}` leaves nothing a targeted clean removes, \
                 measured against cargo rather than read out of its \
                 documentation, so there is no artifact this run could find stale"
            ));
        }
    }
    let manifest = read
        .value(&["--manifest-path"])
        .map_or_else(|| at.join("Cargo.toml"), |written| at.join(written));
    let locked = read.has("--locked");

    let difference = match difference(at) {
        Ok(found) => found,
        Err(message) => return Plan::Unreadable(message),
    };
    let (root, paths) = match difference {
        Difference::NoRepository => {
            return Plan::Nothing(format!(
                "{} is not a git repository, so nothing in it can be said to \
                 differ from a committed state",
                at.display()
            ))
        }
        Difference::Changed { root, paths } => (root, paths),
    };

    let members = match members(at, &manifest, &root, locked) {
        Ok(found) => found,
        Err(message) => return Plan::Unreadable(message),
    };

    // THE DEEPEST OWNER WINS, for the reason `ci_plan::manifest` gives about
    // suffix matching: a workspace root can itself be a package, so a member
    // directory is a prefix of another member's, and the shallower one owns
    // every file of the deeper one under a first-match reading.
    let mut packages: Vec<String> = Vec::new();
    for path in &paths {
        let absolute = root.join(path);
        let owner = members
            .iter()
            .filter(|member| absolute.starts_with(&member.directory))
            .max_by_key(|member| member.directory.as_os_str().len());
        if let Some(owner) = owner {
            if !packages.contains(&owner.name) {
                packages.push(owner.name.clone());
            }
        }
    }
    packages.sort();
    Plan::Freshen(Freshen {
        manifest,
        locked,
        packages,
        changed: paths.len(),
        members: members.len(),
    })
}

/// Remove the artifacts the plan names, and answer with the commands that did
/// it.
///
/// RETURNED RATHER THAN PRINTED, so what this ran is a value a test can read.
/// A pass whose only record is its own stdout is one whose behaviour can be
/// checked in exactly one way, by running the binary and reading English.
///
/// # Errors
///
/// When a clean fails. That is not a finding about the tree — it is this pass
/// not having happened, and a caller that continued would run the very
/// verification the pass exists to make trustworthy.
pub fn apply(at: &Path, freshen: &Freshen) -> Result<Vec<String>, String> {
    let mut ran = Vec::new();
    for package in &freshen.packages {
        let arguments = clean_arguments(freshen, package);
        let out = issue::cargo(Tree::PinnedWhenItIsOurs(
            "the pass cleans the workspace of the command it was handed, and \
             `clean_arguments` copies that command's own `--locked` — which the \
             side gate set from the ownership it prints, so the flag is here \
             exactly where the lockfile is this repository's",
        ))
        .args(&arguments)
        .current_dir(at)
        .output()
        .map_err(|error| format!("`{}` could not be run: {error}", issue::program()))?;
        let rendered = format!("cargo {}", arguments.join(" "));
        if !out.status.success() {
            return Err(format!(
                "`{rendered}` failed ({}), so the artifacts of a package this \
                 tree has changed are still there:\n{}",
                out.status,
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        ran.push(rendered);
    }
    Ok(ran)
}

/// The words one clean is issued with.
///
/// SEPARATE FROM RUNNING IT, because these words are the whole of what this
/// program does to the tree and a test that had to run cargo to see them would
/// be reading cargo's behaviour instead of this decision.
#[must_use]
pub fn clean_arguments(freshen: &Freshen, package: &str) -> Vec<String> {
    let mut arguments = vec![
        "clean".to_string(),
        "-p".to_string(),
        package.to_string(),
        "--manifest-path".to_string(),
        freshen.manifest.display().to_string(),
    ];
    if freshen.locked {
        arguments.push("--locked".to_string());
    }
    arguments
}

/// What one run says, in the order a reader should meet it.
///
/// Returned rather than printed for the reason the sibling gates give: a
/// program whose output can only be checked by running it and reading it is one
/// whose sentences rot silently.
#[must_use]
pub fn report_lines(plan: &Plan) -> Vec<String> {
    match plan {
        Plan::Nothing(reason) => vec![format!("nothing to freshen — {reason}")],
        Plan::Unreadable(reason) => vec![format!("NOT RUN — {reason}")],
        Plan::Freshen(freshen) => {
            let mut out = vec![format!(
                "{} package(s) of {} changed against HEAD, out of {} that \
                 resolve holds in this repository and {} path(s) the tree \
                 reports changed{}",
                freshen.packages.len(),
                freshen.manifest.display(),
                freshen.members,
                freshen.changed,
                if freshen.locked {
                    " (pinned, because the command it precedes is)"
                } else {
                    " (unpinned, because the command it precedes is)"
                }
            )];
            for package in &freshen.packages {
                out.push(format!(
                    "cleaning {package}, whose artifacts this tree no longer \
                     agrees with"
                ));
            }
            out
        }
    }
}

/// Ask git what this tree holds that HEAD does not.
///
/// TWO QUESTIONS AND NOT ONE. `git diff --name-only HEAD` answers about files
/// git is tracking, and a file added to a crate and not yet committed is not one
/// of them — it changes what that crate compiles to just the same, and a
/// freshness pass that missed it would leave the artifact of a crate that no
/// longer exists in that shape. `git ls-files --others --exclude-standard` is
/// the other half, and it honours `.gitignore`, so `target/` is not in it.
///
/// A TREE WITH NO COMMIT IS STILL ANSWERABLE. `git diff HEAD` fails there, and
/// the honest reading is that everything in such a tree is new: the untracked
/// half alone is the whole answer, so the HEAD question is asked first and its
/// absence is not an error.
fn difference(at: &Path) -> Result<Difference, String> {
    // A GIT THAT SAID NO AND A GIT THAT COULD NOT BE RUN ARE DIFFERENT ANSWERS,
    // and collapsing them is how "this tree is not a repository" comes to mean
    // "this machine has no git". The first is a tree with nothing to compare
    // against; the second is this pass not having happened, which is the
    // outcome the caller must fail on.
    let inside = run_git(at, &["rev-parse", "--is-inside-work-tree"])?;
    if !inside.status.success() || String::from_utf8_lossy(&inside.stdout).trim() != "true" {
        return Ok(Difference::NoRepository);
    }
    let root = PathBuf::from(git(at, &["rev-parse", "--show-toplevel"])?.trim());
    let root = std::fs::canonicalize(&root).map_err(|error| {
        format!(
            "the repository root {} cannot be read: {error}",
            root.display()
        )
    })?;
    let mut paths: Vec<String> = Vec::new();
    if run_git(at, &["rev-parse", "--verify", "--quiet", "HEAD"])?
        .status
        .success()
    {
        paths.extend(lines(&git(at, &["diff", "--name-only", "HEAD"])?));
    }
    paths.extend(lines(&git(
        at,
        &["ls-files", "--others", "--exclude-standard"],
    )?));
    paths.sort();
    paths.dedup();
    Ok(Difference::Changed { root, paths })
}

/// The packages of that resolve which live in THIS repository, and where each
/// lives.
///
/// NOT `--no-deps`, AND THAT IS THE DIFFERENCE BETWEEN A WORKSPACE AND THE CODE
/// A RUN COMPILES. Six workspaces under `tools/` path-depend on `tools/ci-plan`,
/// which is a member of the ROOT workspace and of none of theirs. A pass that
/// asked only for members would report `0 packages changed` for every one of
/// them on a round that edited `ci-plan` — and the suite about to run is the one
/// that compiles it. What this law is about is code THE TREE HAS CHANGED, and a
/// path dependency inside this checkout is exactly that.
///
/// THE BOUNDARY IS THE REPOSITORY AND IT IS ASKED OF GIT, not of a list. Cargo's
/// full resolve names every registry crate too, and `studio` reaches a sibling
/// checkout this repository does not own; neither is code git here reports
/// changed, and neither is ours to clean. So the filter is `inside the git
/// toplevel`, which is the same boundary the changed paths come from.
///
/// `--locked` exactly when the command being verified pins, for the reason in
/// this file's header. Resolving with dependencies is what the command about to
/// run does anyway, so this asks cargo for nothing the next command would not.
fn members(at: &Path, manifest: &Path, root: &Path, locked: bool) -> Result<Vec<Member>, String> {
    let mut arguments = vec![
        "metadata".to_string(),
        "--format-version".to_string(),
        "1".to_string(),
        "--manifest-path".to_string(),
        manifest.display().to_string(),
    ];
    if locked {
        arguments.push("--locked".to_string());
    }
    let out = issue::cargo(Tree::PinnedWhenItIsOurs(
        "the workspace is the one the wrapped command names, and this resolve \
         copies that command's own `--locked` — which the side gate set from the \
         ownership it prints, so the flag is here exactly where the lockfile is \
         this repository's",
    ))
    .args(&arguments)
    .current_dir(at)
    .output()
    .map_err(|error| format!("`{}` could not be run: {error}", issue::program()))?;
    if !out.status.success() {
        return Err(format!(
            "`cargo {}` failed ({}), so which packages that workspace has is \
             not known:\n{}",
            arguments.join(" "),
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let read: serde_json::Value = serde_json::from_slice(&out.stdout)
        .map_err(|error| format!("cargo's own metadata did not parse: {error}"))?;
    let listed = read
        .get("packages")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "cargo's metadata names no `packages` at all".to_string())?;
    let mut found = Vec::new();
    for package in listed {
        let name = package
            .get("name")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "a package in cargo's metadata has no name".to_string())?;
        let written = package
            .get("manifest_path")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| format!("cargo's metadata does not say where {name} lives"))?;
        let directory = Path::new(written)
            .parent()
            .ok_or_else(|| format!("{written} is not a manifest inside a directory"))?;
        let directory = std::fs::canonicalize(directory)
            .map_err(|error| format!("{} cannot be read: {error}", directory.display()))?;
        if !directory.starts_with(root) {
            continue;
        }
        found.push(Member {
            name: name.to_string(),
            directory,
        });
    }
    if found.is_empty() {
        return Err(format!(
            "cargo says {} resolves no package that lives in {}, which is the \
             empty answer that reads like a clean one — a manifest inside this \
             repository always resolves at least itself",
            manifest.display(),
            root.display()
        ));
    }
    Ok(found)
}

/// Run git and hand back whatever it said. `Err` ONLY when git could not be run
/// at all — a non-zero git is an answer and belongs to the caller.
fn run_git(at: &Path, arguments: &[&str]) -> Result<std::process::Output, String> {
    Command::new("git")
        .args(arguments)
        .current_dir(at)
        .output()
        .map_err(|error| format!("git could not be run: {error}"))
}

/// Run git where a non-zero status is a failure of this pass rather than an
/// answer to it.
fn git(at: &Path, arguments: &[&str]) -> Result<String, String> {
    let out = run_git(at, arguments)?;
    if !out.status.success() {
        return Err(format!(
            "`git {}` failed ({}) in {}:\n{}",
            arguments.join(" "),
            out.status,
            at.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn lines(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}
