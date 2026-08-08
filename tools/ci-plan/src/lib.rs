//! THE one answer to what this repository's CI runs.
//!
//! Three gates ask that question and they must be asking it the same way:
//!
//! - `evidence_replay_smoke` checks that any job able to run its gates checks
//!   out full history.
//! - `feature_coverage_smoke` checks that every feature this workspace declares
//!   is compiled by somebody.
//! - `tools/unrun-tests` checks that every test this repository compiles is one
//!   some CI command runs.
//!
//! A second loader is a second answer, free to drift from the first — the shape
//! R777, R783, R1080 and R1082 each closed one level at a time, where a list
//! restated the tree and then went quietly stale. R1083 is the same lesson at
//! its sharpest: a gate re-derived "which separate workspaces can be asked on
//! this machine" instead of asking the script that already knew, and turned
//! main red on a runner that has no sibling `pinion` checkout. So everything
//! here is either read from a tracked file or asked of the program that owns
//! the answer, and nothing is restated.
//!
//! Two sources, one per thing CI is made of:
//!
//! - **Workflows** — [`workflow_files`] lists them from `git ls-files` rather
//!   than a directory walk, so a workflow that is not tracked (and which GitHub
//!   therefore does not run) is not counted as covering anything.
//! - **The workspace lister** — [`workspaces`] runs
//!   `scripts/check-side-workspaces.sh --list`, which is the file CI actually
//!   invokes for every separate in-repo workspace and the only place that knows
//!   which of them this machine can build.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use yaml_rust2::{Yaml, YamlLoader};

// --- workflows --------------------------------------------------------------

/// Every workflow file this repository tracks, sorted.
pub fn workflow_files(root: &Path) -> Vec<String> {
    let out = Command::new("git")
        .args(["ls-files", ".github/workflows"])
        .current_dir(root)
        .output()
        .expect("git ls-files runs");
    assert!(
        out.status.success(),
        "git ls-files failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let mut files: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_string)
        .collect();
    files.sort();
    assert!(
        !files.is_empty(),
        "this repository tracks no workflow at all — a check over zero of them \
         is the empty answer that looks like a clean one"
    );
    files
}

/// Parse one workflow, failing loud rather than skipping: an unparseable
/// workflow is one GitHub silently does not run, and R583 lost an unknown
/// stretch of CI to exactly that.
pub fn load_workflow(root: &Path, path: &str) -> Yaml {
    let raw = std::fs::read_to_string(root.join(path)).expect("read workflow");
    let docs = YamlLoader::load_from_str(&raw).unwrap_or_else(|e| {
        panic!("{path} is not parseable YAML — GitHub would silently not run it: {e}")
    });
    assert_eq!(docs.len(), 1, "{path}: expected exactly one YAML document");
    docs.into_iter().next().expect("one document")
}

/// Every `run:` script in every job of one workflow, with the job's name.
pub fn run_steps(doc: &Yaml) -> Vec<(String, String)> {
    let mut steps = Vec::new();
    let Some(jobs) = doc["jobs"].as_hash() else {
        return steps;
    };
    for (name, job) in jobs {
        let name = name.as_str().unwrap_or("<unnamed>").to_string();
        let Some(job_steps) = job["steps"].as_vec() else {
            continue;
        };
        for step in job_steps {
            if let Some(script) = step["run"].as_str() {
                steps.push((name.clone(), script.to_string()));
            }
        }
    }
    steps
}

// --- cargo commands ---------------------------------------------------------

/// One cargo invocation, as CI writes it.
///
/// The split at a bare `--` is why this is a type rather than a word list:
/// everything left of it is cargo's and everything right of it is the test
/// harness's. A gate that wants to ask a command what it would run has to
/// append to the RIGHT side (`-- --list`) while leaving the left side exactly
/// as CI has it, and guessing which side a flag belongs on is how a gate starts
/// answering about a command nobody runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CargoCommand {
    /// Where it is written — a workflow path, or the workspace lister.
    pub source: String,
    /// The job or workspace it belongs to.
    pub owner: String,
    /// Words from `cargo` up to (not including) a bare `--`, `cargo` first.
    pub cargo_args: Vec<String>,
    /// Words after the first bare `--`.
    pub harness_args: Vec<String>,
}

impl CargoCommand {
    /// `test`, `run`, `clippy`, … — the first word that is not a flag.
    pub fn subcommand(&self) -> Option<&str> {
        self.cargo_args
            .iter()
            .skip(1)
            .find(|word| !word.starts_with('-'))
            .map(String::as_str)
    }

    /// Is this flag present on cargo's side, in either spelling?
    pub fn has(&self, flag: &str) -> bool {
        self.flag_at(&[flag]).is_some()
    }

    /// Where the first of `names` sits on cargo's side, and the value if it was
    /// written joined. A reader that knows only `--flag value` answers "absent"
    /// for `--flag=value`, and R1082 carried that as a limit whose loud
    /// direction it had chosen. Both spellings are one flag, so both are read
    /// here and neither caller has to know there are two.
    fn flag_at(&self, names: &[&str]) -> Option<(usize, Option<&str>)> {
        self.cargo_args
            .iter()
            .enumerate()
            .find_map(|(index, word)| {
                if names.contains(&word.as_str()) {
                    return Some((index, None));
                }
                let (head, tail) = word.split_once('=')?;
                names.contains(&head).then_some((index, Some(tail)))
            })
    }

    /// Is this flag present on the harness's side?
    pub fn harness_has(&self, flag: &str) -> bool {
        self.harness_args.iter().any(|word| word == flag)
    }

    /// The value of `--flag value` or `--flag=value` on cargo's side, for the
    /// first of `names` that appears.
    pub fn value(&self, names: &[&str]) -> Option<&str> {
        match self.flag_at(names)? {
            (_, Some(joined)) => Some(joined),
            (index, None) => self.cargo_args.get(index + 1).map(String::as_str),
        }
    }

    /// How the command reads back, for a gate's own output.
    pub fn rendered(&self) -> String {
        let mut out = self.cargo_args.join(" ");
        if !self.harness_args.is_empty() {
            out.push_str(" -- ");
            out.push_str(&self.harness_args.join(" "));
        }
        out
    }

    /// Where it comes from, for a gate's own output.
    pub fn origin(&self) -> String {
        format!("{} `{}`", self.source, self.owner)
    }
}

/// Every cargo invocation in every job of every tracked workflow.
pub fn workflow_cargo_commands(root: &Path) -> Vec<CargoCommand> {
    let mut out = Vec::new();
    for path in workflow_files(root) {
        let doc = load_workflow(root, &path);
        for (job, script) in run_steps(&doc) {
            for (cargo_args, harness_args) in parse_script(&script) {
                out.push(CargoCommand {
                    source: path.clone(),
                    owner: job.clone(),
                    cargo_args,
                    harness_args,
                });
            }
        }
    }
    out
}

/// Split one shell script into the cargo invocations it holds.
///
/// A `run:` step is shell, and this reads the part of shell the workflows
/// actually use: continuation lines, and the `&& || ; |` operators that put two
/// commands on one line. A segment counts only when its FIRST word is `cargo` —
/// asking whether the word appears anywhere reads
/// `--manifest-path tools/x/Cargo.toml` inside one command as the start of
/// another.
pub fn parse_script(script: &str) -> Vec<(Vec<String>, Vec<String>)> {
    let mut out = Vec::new();
    for line in join_continuations(script) {
        for segment in split_operators(&line) {
            let words = words_of(&segment);
            if words.first().map(String::as_str) != Some("cargo") {
                continue;
            }
            match words.iter().position(|word| word == "--") {
                Some(at) => out.push((words[..at].to_vec(), words[at + 1..].to_vec())),
                None => out.push((words, Vec::new())),
            }
        }
    }
    out
}

fn words_of(segment: &str) -> Vec<String> {
    segment
        .split_whitespace()
        .map(|word| word.trim_matches(['"', '\'']).to_string())
        .filter(|word| !word.is_empty())
        .collect()
}

/// A trailing `\` continues the command on the next line.
fn join_continuations(script: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut pending = String::new();
    for line in script.lines() {
        let trimmed = line.trim();
        if let Some(head) = trimmed.strip_suffix('\\') {
            pending.push_str(head);
            pending.push(' ');
            continue;
        }
        pending.push_str(trimmed);
        lines.push(std::mem::take(&mut pending));
    }
    if !pending.is_empty() {
        lines.push(pending);
    }
    lines
}

/// The shell operators that end one command and start the next.
fn split_operators(line: &str) -> Vec<String> {
    let mut segments = vec![String::new()];
    let characters: Vec<char> = line.chars().collect();
    let mut index = 0;
    while index < characters.len() {
        let pair = (
            characters[index],
            characters.get(index + 1).copied().unwrap_or(' '),
        );
        let doubled = matches!(pair, ('&', '&') | ('|', '|'));
        if doubled || matches!(pair.0, ';' | '|' | '&') {
            segments.push(String::new());
            index += if doubled { 2 } else { 1 };
            continue;
        }
        segments
            .last_mut()
            .expect("there is always a current segment")
            .push(pair.0);
        index += 1;
    }
    segments
}

// --- the workspace lister ---------------------------------------------------

/// What `scripts/check-side-workspaces.sh --list` says.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Workspaces {
    /// Manifests that can be asked about on this machine, root first. The root
    /// workspace is not in the lister's own population by construction — that
    /// script exists to reach the ones the root gates never compile — so it is
    /// added here and is never skipped.
    pub askable: Vec<String>,
    /// Workspaces the lister declined, each with the reason it printed. On a
    /// hosted runner `studio` is one of these: its path dependencies name a
    /// sibling checkout no runner has.
    pub skipped: Vec<String>,
    /// The suite command the lister runs for each askable separate workspace,
    /// keyed by its directory. The root workspace has none here — its suite is
    /// written in a workflow, not in the lister.
    pub suites: BTreeMap<String, Vec<String>>,
}

/// Ask the lister. Panics rather than guessing: a gate that cannot reach the
/// one program holding this answer must not fall back to deriving it, which is
/// exactly the failure R1083 repaired.
pub fn workspaces(root: &Path) -> Workspaces {
    let out = Command::new("bash")
        .arg("scripts/check-side-workspaces.sh")
        .arg("--list")
        .current_dir(root)
        .output()
        .expect("check-side-workspaces.sh runs");
    assert!(
        out.status.success(),
        "check-side-workspaces.sh --list failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8_lossy(&out.stdout);
    let listed = parse_lister(&text);
    assert!(
        listed.askable.len() > 1 || !listed.skipped.is_empty(),
        "the workspace lister named nothing at all — this repository has \
         separate workspaces, so an empty answer is the gate not reading:\n{text}"
    );
    listed
}

/// Read the lister's output. Pinned against strings on purpose, so that a
/// change to what the script prints and a change to what reads it cannot both
/// drift into agreeing about nothing.
pub fn parse_lister(text: &str) -> Workspaces {
    let mut listed = Workspaces {
        askable: vec!["Cargo.toml".to_string()],
        ..Workspaces::default()
    };
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("[side-workspaces] CHECKABLE ") {
            listed.askable.push(format!("{}/Cargo.toml", rest.trim()));
        } else if let Some(rest) = line.strip_prefix("[side-workspaces] SKIP ") {
            listed.skipped.push(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("[side-workspaces] SUITE ") {
            let mut words = words_of(rest);
            if words.is_empty() {
                continue;
            }
            let workspace = words.remove(0);
            listed.suites.insert(workspace, words);
        }
    }
    listed.askable.sort();
    listed
}

/// The suite commands the lister runs, as [`CargoCommand`]s a gate can re-issue.
pub fn lister_cargo_commands(listed: &Workspaces) -> Vec<CargoCommand> {
    let mut out = Vec::new();
    for (workspace, words) in &listed.suites {
        let (cargo_args, harness_args) = match words.iter().position(|word| word == "--") {
            Some(at) => (words[..at].to_vec(), words[at + 1..].to_vec()),
            None => (words.clone(), Vec::new()),
        };
        out.push(CargoCommand {
            source: "scripts/check-side-workspaces.sh".to_string(),
            owner: workspace.clone(),
            cargo_args,
            harness_args,
        });
    }
    out
}
