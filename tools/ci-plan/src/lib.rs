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
//! - `tools/cache-budget` checks that every cache this repository's CI declares
//!   is one it gets to keep. That is a second question about the same files —
//!   what CI asks to KEEP rather than what it RUNS — and it is answered here for
//!   the reason the others are: a second reader is a second answer.
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
    parse_workflow(&raw, path)
}

/// The same parse, over text rather than a file — so a test can pin a reading
/// rule against a workflow shape this repository does not happen to contain
/// (`reading.rs` explains why those are the branches that matter) without
/// needing its own copy of a YAML library, which would be a second parser.
pub fn parse_workflow(raw: &str, source: &str) -> Yaml {
    let docs = YamlLoader::load_from_str(raw).unwrap_or_else(|e| {
        panic!("{source} is not parseable YAML — GitHub would silently not run it: {e}")
    });
    assert_eq!(
        docs.len(),
        1,
        "{source}: expected exactly one YAML document"
    );
    docs.into_iter().next().expect("one document")
}

/// One `run:` step, with the job it belongs to and the environment it runs in.
///
/// THE ENVIRONMENT IS PART OF THE COMMAND, and this repository holds the two
/// proofs of it. The `msrv` job's `cargo check --workspace` is the same words as
/// `validate`'s half of the same, and what makes it a different build is
/// `RUSTUP_TOOLCHAIN` on the step. R1090 then set `CARGO_PROFILE_DEV_DEBUG` at
/// the top of the file, which changes what every job in it compiles. A reader
/// that took only the words would call those the same command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunStep {
    /// The job id — the spelling `needs:` uses.
    pub job: String,
    /// The shell of the `run:` key.
    pub script: String,
    /// Workflow `env:`, overlaid by the job's, overlaid by the step's — GitHub's
    /// own precedence, resolved here so no caller has to know there are three
    /// places to look.
    pub env: BTreeMap<String, String>,
}

/// How a YAML scalar spells an environment value. `CARGO_INCREMENTAL: 0` is an
/// integer to a YAML parser and a string to a process, and a reader that took
/// only `as_str` would drop it.
fn scalar(value: &Yaml) -> Option<String> {
    match value {
        Yaml::String(text) => Some(text.clone()),
        Yaml::Integer(number) => Some(number.to_string()),
        Yaml::Real(number) => Some(number.clone()),
        Yaml::Boolean(flag) => Some(flag.to_string()),
        _ => None,
    }
}

/// The `env:` mapping at one level, if there is one.
fn env_at(node: &Yaml) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let Some(pairs) = node["env"].as_hash() else {
        return out;
    };
    for (name, value) in pairs {
        let (Some(name), Some(value)) = (name.as_str(), scalar(value)) else {
            continue;
        };
        out.insert(name.to_string(), value);
    }
    out
}

/// Every `run:` step in every job of one workflow.
pub fn run_steps(doc: &Yaml) -> Vec<RunStep> {
    let mut steps = Vec::new();
    let Some(jobs) = doc["jobs"].as_hash() else {
        return steps;
    };
    let workflow_env = env_at(doc);
    for (name, job) in jobs {
        let name = name.as_str().unwrap_or("<unnamed>").to_string();
        let Some(job_steps) = job["steps"].as_vec() else {
            continue;
        };
        let mut job_env = workflow_env.clone();
        job_env.extend(env_at(job));
        for step in job_steps {
            if let Some(script) = step["run"].as_str() {
                let mut env = job_env.clone();
                env.extend(env_at(step));
                steps.push(RunStep {
                    job: name.clone(),
                    script: script.to_string(),
                    env,
                });
            }
        }
    }
    steps
}

// --- caches -----------------------------------------------------------------

/// One `actions/cache` step, as a workflow declares it.
///
/// The KEY IS KEPT TWICE on purpose. `key` is what the workflow says and what a
/// person greps for. `prefix` is what a restore actually matches on: every key in
/// this repository ends in a `hashFiles` of the lockfiles, so a dependency bump
/// changes all of them at once, and a reader joining on the whole key would call
/// every cache in the repository missing the day after one. `restore-keys` is the
/// workflows' own statement that the prefix is the identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheDeclaration {
    /// The workflow file it is written in.
    pub source: String,
    /// The id of the job that runs it — the same spelling `needs:` uses.
    pub owner: String,
    /// The `key:` exactly as written, expressions and all.
    pub key: String,
    /// The key with `${{ runner.os }}` resolved from the job's own `runs-on`,
    /// truncated at the first expression that cannot be resolved without running
    /// the job — `Linux-cargo-unrun-`.
    pub prefix: String,
    /// The `path:` entries, in the order written. What a cache HOLDS is what it
    /// costs, so this is what lets one cache's size be reasoned about from
    /// another's.
    pub paths: Vec<String>,
    /// The file globs the key's `hashFiles(…)` calls name, in the order written.
    ///
    /// THE KEY SAYS WHAT WOULD LEGITIMATELY INVALIDATE IT. A cache that this run
    /// had to build from nothing is a job that paid for a cold build, which is
    /// the cost the whole budget exists to avoid — EXCEPT when the thing the key
    /// hashes actually moved, and then one cold run is simply the price of a
    /// dependency change. Reading the globs off the declaration is how that
    /// exception is derived rather than assumed: `side-workspaces` hashes
    /// `bench/Cargo.lock` and `tools/*/Cargo.lock` while every other key here
    /// hashes `**/Cargo.lock`, so "the lockfiles changed" is a different question
    /// per key and only the key can answer it.
    pub hashed: Vec<String>,
}

/// What GitHub sets `runner.os` to for a `runs-on` label.
///
/// A REFUSAL RATHER THAN A GUESS for a label this does not know. The prefix is
/// the identity every later join is made on, so a wrong one reports every cache
/// in the repository as absent — the loudest possible wrong answer, wearing the
/// shape of a finding.
fn runner_os(runs_on: &str) -> String {
    let label = runs_on.trim();
    match label.split('-').next().unwrap_or(label) {
        "ubuntu" => "Linux".to_string(),
        "windows" => "Windows".to_string(),
        "macos" => "macOS".to_string(),
        _ => panic!(
            "`runs-on: {label}` is a runner this reader has no `runner.os` for — \
             the cache key prefix is what every later comparison joins on, so \
             guessing it would report every cache in the repository as missing"
        ),
    }
}

/// Every glob a key's `hashFiles(…)` calls name.
///
/// A hand-written scan rather than a grammar, and the shape it reads is the one
/// GitHub documents: `hashFiles('a', 'b')`, single-quoted, comma-separated. An
/// argument that is not a plain quoted literal is NOT guessed at — it is left
/// out, and a key whose inputs cannot be read simply excuses nothing, which is
/// the strict direction.
pub fn hashed_globs(key: &str) -> Vec<String> {
    let mut globs = Vec::new();
    let mut rest = key;
    while let Some(at) = rest.find("hashFiles(") {
        rest = &rest[at + "hashFiles(".len()..];
        let Some(end) = rest.find(')') else { break };
        let arguments = &rest[..end];
        rest = &rest[end..];
        for argument in arguments.split(',') {
            let argument = argument.trim();
            let unquoted = argument
                .strip_prefix('\'')
                .and_then(|inner| inner.strip_suffix('\''))
                .or_else(|| {
                    argument
                        .strip_prefix('"')
                        .and_then(|inner| inner.strip_suffix('"'))
                });
            if let Some(glob) = unquoted {
                if !glob.is_empty() {
                    globs.push(glob.to_string());
                }
            }
        }
    }
    globs
}

/// The literal head of a key: everything before the first `${{ … }}` that is not
/// `runner.os`.
fn key_prefix(key: &str, os: &str) -> String {
    let resolved = key.replace("${{ runner.os }}", os);
    match resolved.find("${{") {
        Some(at) => resolved[..at].to_string(),
        None => resolved,
    }
}

/// Every `actions/cache` step in every job of one workflow.
///
/// `actions/cache/restore` counts too: a job that only restores still declares a
/// dependence on that key surviving, which is the very thing being judged.
/// `actions/cache/save` does not appear in this repository and would be read the
/// same way if it did.
pub fn cache_steps(doc: &Yaml, source: &str) -> Vec<CacheDeclaration> {
    let mut out = Vec::new();
    let Some(jobs) = doc["jobs"].as_hash() else {
        return out;
    };
    for (name, job) in jobs {
        let owner = name.as_str().unwrap_or("<unnamed>").to_string();
        let Some(steps) = job["steps"].as_vec() else {
            continue;
        };
        let mut os = None;
        for step in steps {
            let Some(uses) = step["uses"].as_str() else {
                continue;
            };
            let action = uses.split('@').next().unwrap_or(uses);
            if action != "actions/cache" && action != "actions/cache/restore" {
                continue;
            }
            // Resolved LAZILY, so a job with no cache step is never asked for a
            // `runs-on` this reader would have to refuse.
            let os = os.get_or_insert_with(|| {
                runner_os(job["runs-on"].as_str().unwrap_or_else(|| {
                    panic!("{source}: job `{owner}` caches but declares no `runs-on`")
                }))
            });
            let Some(key) = step["with"]["key"].as_str() else {
                panic!("{source}: job `{owner}` caches with no key at all");
            };
            let paths: Vec<String> = step["with"]["path"]
                .as_str()
                .unwrap_or_else(|| panic!("{source}: job `{owner}` caches no path"))
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_string)
                .collect();
            out.push(CacheDeclaration {
                source: source.to_string(),
                owner: owner.clone(),
                key: key.to_string(),
                prefix: key_prefix(key, os),
                paths,
                hashed: hashed_globs(key),
            });
        }
    }
    out
}

/// Every cache declared by every tracked workflow.
pub fn workflow_cache_declarations(root: &Path) -> Vec<CacheDeclaration> {
    let mut out = Vec::new();
    for path in workflow_files(root) {
        let doc = load_workflow(root, &path);
        out.extend(cache_steps(&doc, &path));
    }
    out
}

/// What each job of one workflow waits for, by job id.
///
/// `needs:` is written either as one job id or as a list of them, and both
/// spellings are one thing — a reader that knew only the list form would answer
/// "waits for nothing" for the single form, which is the same class of defect as
/// R1082's `--flag=value`.
pub fn job_needs(doc: &Yaml) -> BTreeMap<String, Vec<String>> {
    let mut out = BTreeMap::new();
    let Some(jobs) = doc["jobs"].as_hash() else {
        return out;
    };
    for (name, job) in jobs {
        let id = name.as_str().unwrap_or("<unnamed>").to_string();
        let needs = match &job["needs"] {
            Yaml::String(one) => vec![one.clone()],
            Yaml::Array(many) => many
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect(),
            _ => Vec::new(),
        };
        out.insert(id, needs);
    }
    out
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
    /// The environment the command runs in — see [`RunStep::env`]. Empty for a
    /// command a gate builds for itself rather than reads out of a workflow.
    pub env: BTreeMap<String, String>,
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
        for step in run_steps(&doc) {
            for (cargo_args, harness_args) in parse_script(&step.script) {
                out.push(CargoCommand {
                    source: path.clone(),
                    owner: step.job.clone(),
                    cargo_args,
                    harness_args,
                    env: step.env.clone(),
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

/// A workspace the lister declined, and why.
///
/// The two halves are separate fields rather than one printed sentence because
/// they answer different questions and one of them is machine-readable: the
/// reason is for a person reading the run, and the directory is what a gate
/// needs in order to say "this machine cannot compile anything under here" — a
/// judgement R1084's successor makes per file. Recovering the directory by
/// splitting the sentence is the kind of re-derivation this crate exists to
/// remove.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedWorkspace {
    /// Its directory, relative to the repository root.
    pub directory: String,
    /// The lister's own words for why it was not checked.
    pub reason: String,
}

impl std::fmt::Display for SkippedWorkspace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.directory, self.reason)
    }
}

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
    pub skipped: Vec<SkippedWorkspace>,
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
            // `SKIP <ws> — <reason>`: the directory is the first word, the same
            // shape `SUITE <ws> <command…>` already relies on.
            let rest = rest.trim();
            let (directory, reason) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
            listed.skipped.push(SkippedWorkspace {
                directory: directory.to_string(),
                reason: reason.trim_start().to_string(),
            });
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
            env: BTreeMap::new(),
        });
    }
    out
}
