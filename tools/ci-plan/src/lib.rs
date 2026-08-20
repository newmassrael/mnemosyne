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
//! - `judged_test_runs` checks that every `cargo test` this repository issues is
//!   issued THROUGH the wrapper that judges what the run covered. A third
//!   question about the same population, and the reason [`CargoCommand`] keeps
//!   the words in FRONT of `cargo` rather than discarding them.
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
//! Three sources, one per place a cargo command in this repository is written:
//!
//! - **Workflows** — [`workflow_files`] lists them from `git ls-files` rather
//!   than a directory walk, so a workflow that is not tracked (and which GitHub
//!   therefore does not run) is not counted as covering anything.
//! - **The workspace lister** — [`workspaces`] runs
//!   `scripts/check-side-workspaces.sh --list`, which is the file CI actually
//!   invokes for every separate in-repo workspace and the only place that knows
//!   which of them this machine can build. Its commands are ASSEMBLED at
//!   runtime, so its output is the only place they are fully spelled — reading
//!   its text would ask whether a variable is spelled `--locked`.
//! - **Tracked shell** — [`script_files`] takes the git hooks and `scripts/`,
//!   which is where the checks a developer gets before a push are written.
//!   R1115 added it: `locked_resolution_smoke` asks whether every cargo command
//!   this repository issues pins the lockfile it resolves, and four of the ones
//!   that did not were in the hooks, where no gate's population reached.
//! - **The build-machine declaration** — [`declared_build_commands`] reads
//!   `[commands]` out of `.claude/remote-build.toml`. R1197 added it for the
//!   reason R1115 added the one above: three cargo commands were written there,
//!   one of them the whole workspace suite, and no law reached any of them.
//! - **Rust's own `.arg()` chains** — [`rust::cargo_commands`], the sixth place
//!   and the first one whose words are COMPUTED rather than written down. R1262
//!   added it; [`rust`] holds why a static walk is still the complete answer to
//!   WHERE, and what it does with the words it cannot finish reading. It is a
//!   module rather than a sixth function beside the others because the reading
//!   is a different KIND — a syntax walk, with a vocabulary of its own for the
//!   parts of a computed command line.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

use yaml_rust2::{Yaml, YamlLoader};

pub mod issue;
pub mod rust;

// --- workflows --------------------------------------------------------------

/// Every workflow file a repository tracks, sorted, or why it could not be asked.
///
/// THE FALLIBLE HALF, FOR CALLERS THAT ARE NOT LAWS. [`workflow_files`] asserts
/// the list is non-empty, and that assertion is right for a law — a check over
/// zero workflows is the empty answer that looks like a clean one. It is wrong
/// for a REPORTER, which may be standing in a directory that has none and must
/// say so rather than die: `ci-state` runs in whatever tree a push is happening
/// in, and its entrance cases stand in a throwaway directory on purpose.
pub fn tracked_workflow_files(root: &Path) -> Result<Vec<String>, String> {
    let out = Command::new("git")
        .args(["ls-files", ".github/workflows"])
        .current_dir(root)
        .output()
        .map_err(|why| format!("git ls-files in {} — {why}", root.display()))?;
    if !out.status.success() {
        return Err(format!(
            "git ls-files in {} failed — {}",
            root.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let mut files: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_string)
        .collect();
    files.sort();
    Ok(files)
}

/// Every workflow file this repository tracks, sorted.
pub fn workflow_files(root: &Path) -> Vec<String> {
    let files = tracked_workflow_files(root).expect("git ls-files runs");
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

/// The same, for a caller that must not die on somebody else's file.
///
/// A REPORTER IS NOT A LAW, which is the whole of the difference: a workflow that
/// will not parse is one GitHub silently does not run, and a LAW should stop
/// dead at it. A reporter run inside a push has to name it and carry on, because
/// exiting on it would take out the reading of everything else.
pub fn read_workflow(root: &Path, path: &str) -> Result<Yaml, String> {
    let raw = std::fs::read_to_string(root.join(path))
        .map_err(|why| format!("{path} could not be read — {why}"))?;
    let docs = YamlLoader::load_from_str(&raw)
        .map_err(|why| format!("{path} is not parseable YAML — GitHub would not run it: {why}"))?;
    match docs.len() {
        1 => Ok(docs.into_iter().next().expect("one document")),
        other => Err(format!("{path} holds {other} YAML documents, not one")),
    }
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
    /// Where it sits in its job's `steps:` list, counting every step and not
    /// only the ones that `run:`.
    ///
    /// A JOB'S STEPS ARE AN ORDERED LIST AND THIS IS THE ONLY PLACE THAT SAYS SO.
    /// [`run_steps`] and [`cache_steps`] read the same list twice and each
    /// returns its own population, so without a shared coordinate a caller
    /// holding both can say WHICH steps a job has and never WHICH CAME FIRST.
    /// R1102 wired two measuring steps around a cache restore — the difference
    /// between them is what a job started from — and could not assert from the
    /// file that they bracket the restore at all; a workflow with both of them
    /// on the same side of it measures nothing, reports every job as having
    /// started from an empty tree, and is a file nobody's gate reads as wrong.
    /// Counting every step is what makes the two numbers comparable: an index
    /// among `run:` steps only would say a `run:` step precedes a cache step it
    /// in fact follows.
    pub index: usize,
    /// The shell of the `run:` key.
    pub script: String,
    /// Workflow `env:`, overlaid by the job's, overlaid by the step's — GitHub's
    /// own precedence, resolved here so no caller has to know there are three
    /// places to look.
    pub env: BTreeMap<String, String>,
    /// The step's own `timeout-minutes`, VERBATIM, or `None` when it declares
    /// none (R1236's carry).
    ///
    /// KEPT AS WRITTEN AND NOT PARSED TO A NUMBER. GitHub accepts an expression
    /// here — `${{ env.BUDGET }}` is a declaration — and a reader that parsed to
    /// an integer would answer `None` for it, which is the same word it answers
    /// for a step nobody bounded at all. Those are opposite facts: one is a
    /// bound this reader cannot evaluate, the other is no bound. A caller that
    /// wants the number asks for it and handles the third answer.
    pub timeout: Option<String>,
    /// Its JOB's `timeout-minutes`, verbatim, or `None` when the job declares
    /// none (GitHub then applies its own 360-minute default).
    ///
    /// ON THE STEP, because the only question worth asking about a step's bound
    /// is whether it is SMALLER THAN THE JOB'S. A step bounded at exactly its
    /// job's budget is bounded and cannot prevent anything — it is the same
    /// forty-five minutes spent, with a checkbox ticked. Carrying the job's
    /// number here is what lets one law ask both halves without a second walk
    /// that could disagree with this one about which job a step is in.
    pub job_timeout: Option<String>,
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
        let job_timeout = scalar(&job["timeout-minutes"]);
        for (index, step) in job_steps.iter().enumerate() {
            if let Some(script) = step["run"].as_str() {
                let mut env = job_env.clone();
                env.extend(env_at(step));
                steps.push(RunStep {
                    job: name.clone(),
                    index,
                    script: script.to_string(),
                    env,
                    // `scalar` AND NOT `as_i64`: `timeout-minutes: 5` is an
                    // integer to a YAML parser, `"5"` is a string and
                    // `${{ … }}` is a string too, and all three are the file
                    // declaring a bound. The same function already carries
                    // `CARGO_INCREMENTAL: 0` across that distinction one field
                    // up.
                    timeout: scalar(&step["timeout-minutes"]),
                    job_timeout: job_timeout.clone(),
                });
            }
        }
    }
    steps
}

// --- what a step installs ---------------------------------------------------
//
// MOVED HERE IN R1237, FROM `tools/undeclared-requirement`, AND THE MOVE WAS
// FORCED TWICE OVER. This crate's subject is "the one answer to what this
// repository's CI runs", and which package manager a step invokes is exactly
// that — but the reading grew up next to the law that first needed it, which
// asks whether a package CI installs is named in the build-machine declaration.
// R1237 wrote a SECOND law, in the root suite, over the same population: a step
// that waits on somebody else's server must be bounded. Two readings of "what
// installs" would have been two vocabularies, and the one nobody read would be
// the one that silently shrank.
//
// AND THE FIRST WAY OF SHARING IT WAS MEASURED AND UNDONE. Making that crate a
// dev-dependency of `mnemosyne-cli` made it a member of the root workspace —
// cargo will not have it both ways (`error: multiple workspace roots found in
// the same workspace`) — and `tool_pin_smoke`, a law over every binary of the
// root workspace, then correctly demanded that a `tools/` gate answer `--version`
// and declare how it opens a workspace. A protocol dragged in by where a TEST
// wanted to link is the wrong reason for a binary to change, so the vocabulary
// came here instead and that crate re-exports it.

/// Package managers whose install command names its packages in words a reader
/// can understand in full.
pub const READ_IN_FULL: &[&str] = &["apt-get", "apt", "aptitude"];

/// Managers recognised as installing something whose package names this does not
/// read.
///
/// They are recognised so they can be COUNTED AND NAMED rather than skipped: a
/// step a reader does not understand is a hole in its population, and the report
/// is where a hole belongs. Reading them would mean joining a language's package
/// names against a declaration written in apt's, which is a table, and a table is
/// the thing that goes stale silently.
pub const RECOGNISED_NOT_READ: &[&str] = &[
    "apk", "brew", "cargo", "choco", "dnf", "gem", "go", "npm", "pacman", "pip", "pip3", "rustup",
    "snap", "winget", "yum", "zypper",
];

/// Words in an action's name that say it installs something onto the runner.
///
/// A WEAK READING, and it is the strongest one available: an action is somebody
/// else's program and this repository holds none of its bytes. What the name says
/// is all there is, so the reading is generous — anything matching refuses rather
/// than passes — and an action whose name says nothing about installing is a
/// limit a caller records rather than a claim made here.
pub const INSTALLING_ACTION_WORDS: &[&str] = &["setup", "install", "toolchain", "apt"];

/// Flags an apt-family install takes that consume no word of their own.
///
/// ANY OTHER FLAG IS A REFUSAL. `-t bookworm-backports` takes the next word, and
/// a reader that did not know it would call that word a package — a name that is
/// then reported as undeclared, sending somebody to add a release codename to a
/// list of packages. Guessing which flags take an argument is how a reader starts
/// answering about things nobody installed.
pub const VALUELESS_FLAGS: &[&str] = &[
    "--allow-change-held-packages",
    "--allow-downgrades",
    "--assume-yes",
    "--fix-broken",
    "--fix-missing",
    "--install-recommends",
    "--no-install-recommends",
    "--only-upgrade",
    "--quiet",
    "--reinstall",
    "--yes",
    "-f",
    "-q",
    "-qq",
    "-y",
];

/// Characters that make a word something other than a package name.
const NOT_A_NAME: &[char] = &[
    '$', '`', '*', '?', '(', ')', '{', '}', '[', ']', '<', '>', '\\',
];

/// Strip the words a shell may put in front of a command without changing which
/// program it runs.
///
/// `sudo` and its own flags only. A flag that TAKES a word (`sudo -u root …`)
/// leaves a word that is not the manager, and the caller's detection then finds
/// the manager somewhere other than the head and refuses — which is the intended
/// end of that road.
fn without_privilege(words: &[String]) -> &[String] {
    let Some(first) = words.first() else {
        return words;
    };
    if first != "sudo" {
        return words;
    }
    let mut at = 1;
    while words.get(at).is_some_and(|word| word.starts_with('-')) {
        at += 1;
    }
    &words[at..]
}

/// What one command of a `run:` step installs.
///
/// `InstallCommand` AND NOT `Command`, which is what it was called in the crate
/// it came from: this one already holds `CargoCommand` and `DeclaredCommand`,
/// and a bare `Command` beside them would read as the general case of both while
/// being neither. The crate it moved from keeps its own spelling through a
/// rename at the re-export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallCommand {
    /// Installs nothing.
    Nothing,
    /// An install whose packages this read in full.
    Read {
        manager: String,
        packages: Vec<String>,
    },
    /// An install this recognised and did not read.
    Recognised { what: String },
    /// Install-shaped and unreadable — a refusal, with the reason.
    Refused { why: String },
}

/// Read one command's words for the install it performs.
///
/// # The order of the questions
///
/// The apt family is asked about FIRST and by membership rather than by head
/// position, so `env … apt-get install x` reaches the refusal instead of falling
/// through to "installs nothing". The head is then required to be that manager:
/// everything else is a wrapper this cannot see through.
#[must_use]
pub fn read_command(words: &[String]) -> InstallCommand {
    let body = without_privilege(words);
    let Some(head) = body.first().map(String::as_str) else {
        return InstallCommand::Nothing;
    };
    let apt_at = body
        .iter()
        .position(|word| READ_IN_FULL.contains(&word.as_str()));
    if let Some(at) = apt_at {
        // `apt-get update` is not an install, and neither is a bare `apt`.
        if !body.iter().any(|word| word == "install") {
            return InstallCommand::Nothing;
        }
        if at != 0 {
            return InstallCommand::Refused {
                why: format!(
                    "`{}` installs packages and something else is in front of it, so what it \
                     installs cannot be read from these words",
                    body[at]
                ),
            };
        }
        return read_apt(body);
    }
    if RECOGNISED_NOT_READ.contains(&head) && body.iter().any(|word| word == "install") {
        return InstallCommand::Recognised {
            what: head.to_owned(),
        };
    }
    InstallCommand::Nothing
}

/// Read an apt-family install whose head is the manager.
fn read_apt(body: &[String]) -> InstallCommand {
    let manager = body[0].clone();
    let at = body
        .iter()
        .position(|word| word == "install")
        .expect("the caller established that `install` is one of these words");
    // Everything between the manager and the subcommand must be a flag, or the
    // command is doing something this reading does not describe.
    for word in &body[1..at] {
        if !word.starts_with('-') {
            return InstallCommand::Refused {
                why: format!(
                    "`{word}` sits between `{manager}` and `install`, so this is not the plain \
                     install form this reads"
                ),
            };
        }
    }
    let mut packages = Vec::new();
    for word in &body[at + 1..] {
        if word.starts_with('-') {
            // `--opt=value` carries its value, so it swallows no word.
            if VALUELESS_FLAGS.contains(&word.as_str()) || word.contains('=') {
                continue;
            }
            return InstallCommand::Refused {
                why: format!(
                    "`{word}` is a flag this reader does not know, and a flag that takes the \
                     next word would make that word look like a package"
                ),
            };
        }
        if word.contains(NOT_A_NAME) {
            return InstallCommand::Refused {
                why: format!(
                    "`{word}` is not a package name a file can be held against — the shell \
                     decides what it is at run time"
                ),
            };
        }
        packages.push(word.clone());
    }
    if packages.is_empty() {
        return InstallCommand::Refused {
            why: format!("`{manager} install` with no package named is a form this cannot judge"),
        };
    }
    InstallCommand::Read { manager, packages }
}

/// Whether an action's name says it installs something onto the runner.
#[must_use]
pub fn action_installs(action: &str) -> bool {
    let name = action.to_ascii_lowercase();
    INSTALLING_ACTION_WORDS
        .iter()
        .any(|word| name.contains(word))
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
    /// Where it sits in its job's `steps:` list — the SAME counting
    /// [`RunStep::index`] uses, which is what lets a caller holding both
    /// populations put one before the other.
    pub index: usize,
    /// The `key:` exactly as written, expressions and all.
    pub key: String,
    /// The key with `${{ runner.os }}` resolved from the job's own `runs-on`,
    /// truncated at the first expression that cannot be resolved without running
    /// the job — `Linux-cargo-unrun-`.
    pub prefix: String,
    /// The `restore-keys:` the step declares, `${{ runner.os }}` resolved.
    ///
    /// THE SAME IDENTITY, SPELLED A SECOND TIME BY A HUMAN, and until R1116
    /// nothing read it. [`prefix`](Self::prefix) is DERIVED from the key and is
    /// what every gate in this repository joins on; `restore-keys` is what
    /// GitHub actually falls back to. A workflow where those two disagree is one
    /// where every joining gate is answering about a key GitHub never tries.
    ///
    /// IT IS ALSO THE PREMISE OF EVERY MEASUREMENT IN THE CACHING ARC. Because
    /// each key ends in a `hashFiles` of the lockfiles AND of the workflow
    /// itself, every edit to this file moves all eight primary keys at once —
    /// and the run stays warm anyway, through this fallback. Run 31337461298
    /// changed the workflow and five of seven jobs reported `no exact hit`
    /// against 221, 246, 281, 756 and 32063 MB that arrived regardless. R1115
    /// wrote the opposite into the ledger as a carry — that an edit costs a cold
    /// run — while editing the very file whose comment records the measurement.
    /// A property load-bearing for an arc of measurements was declared in a
    /// field nobody read, so nothing could contradict a sentence about it.
    pub restore_keys: Vec<String>,
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
    /// The step's own `name:`, which is the ONE thing that joins this
    /// declaration to what a run actually did with it.
    ///
    /// WHAT SAVES AN ARCHIVE IS A STEP, NOT A RUN. `actions/cache` writes in its
    /// post step, which GitHub reports as `Post <this name>` inside the job that
    /// held it, with a conclusion of its own — so "the archive for this key was
    /// last saved at commit X" is a question about that step and nothing else.
    /// R1178 bounded the interval with the RUN's conclusion instead, and R1207
    /// measured what that costs on this repository's own history: over 100 runs
    /// and 9 declarations, 134 of the 900 bounds were OLDER than the truth
    /// (a failed run whose cache job had already saved) and 16 more had no bound
    /// at all where the step had one.
    ///
    /// A CACHE STEP WITHOUT A NAME IS A REFUSAL rather than a guess. GitHub then
    /// names it after the action, so two unnamed cache steps in one job are
    /// indistinguishable in the answer — and a join that picks whichever came
    /// first would bound one key's interval with another key's save.
    pub step: String,
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
        for (index, step) in steps.iter().enumerate() {
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
            // THE JOIN AXIS, refused rather than defaulted. GitHub reports the
            // save as `Post <name>`, so an unnamed cache step is one no reader
            // can tell from another unnamed cache step in the same job.
            let Some(step_name) = step["name"]
                .as_str()
                .map(str::trim)
                .filter(|n| !n.is_empty())
            else {
                panic!(
                    "{source}: job `{owner}` caches with no step name, so nothing can join \
                     its archive to the run that saved it — GitHub reports the save as \
                     `Post <the step's name>`"
                );
            };
            let paths: Vec<String> = step["with"]["path"]
                .as_str()
                .unwrap_or_else(|| panic!("{source}: job `{owner}` caches no path"))
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_string)
                .collect();
            let restore_keys: Vec<String> = step["with"]["restore-keys"]
                .as_str()
                .unwrap_or_default()
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(|line| line.replace("${{ runner.os }}", os))
                .collect();
            out.push(CacheDeclaration {
                source: source.to_string(),
                owner: owner.clone(),
                index,
                key: key.to_string(),
                prefix: key_prefix(key, os),
                paths,
                hashed: hashed_globs(key),
                restore_keys,
                step: step_name.to_string(),
            });
        }
    }
    // THE JOIN HAS TO BE UNIQUE WITHIN A WORKFLOW, or it is silently about the
    // wrong archive. A run's jobs page is asked as one document, so two cache
    // steps sharing a name — which GitHub allows, in different jobs of one file —
    // would answer "yes, that archive was written" for EITHER of them, and the
    // key whose job was skipped would be bounded by its sibling's save. Refusing
    // an unnamed step is only half the rule; this is the other half, and it costs
    // one sort over a list this repository has nine entries in.
    let mut named: Vec<&str> = out.iter().map(|cache| cache.step.as_str()).collect();
    named.sort_unstable();
    if let Some(shared) = named.windows(2).find(|pair| pair[0] == pair[1]) {
        panic!(
            "{source}: two cache steps are both named `{}`, so GitHub reports one \
             `Post {}` for two archives and no reader can tell which was written",
            shared[0], shared[0]
        );
    }
    out
}

/// Why a cache declaration cannot survive an edit to the workflow it sits in.
///
/// `None` means it can: some declared `restore-key` matches an earlier
/// generation whatever the primary key hashes, so an edit costs an exact miss
/// and nothing else. This is the property every cross-run comparison in the
/// caching arc rests on, and R1116 is where it stopped being a sentence.
///
/// NOT EVERY CACHE MAY BUY IT, and since R1160 that is the other half of a
/// partition rather than an exception. What keeps a run warm across an edit is
/// INHERITING the previous generation, and
/// [`inheritance_without_a_bound`] is where that inheritance has no bound —
/// so the caches holding the build directory answer to that law and the rest
/// answer to this one. The two are applied together in
/// `tools/ci-plan/tests/reading.rs`, each asserting the other's half is not
/// empty.
pub fn survives_an_edit(declaration: &CacheDeclaration) -> Option<String> {
    if declaration.restore_keys.is_empty() {
        return Some(format!(
            "`{}` declares no `restore-keys`, so the only thing GitHub tries is \
             a primary key that hashes this workflow — every edit to it is a \
             cold run for this cache, and no two runs either side of one are \
             comparable",
            declaration.prefix
        ));
    }
    // A fallback that still carries an expression is one that moves when that
    // expression's inputs move — the same cold run, one level down.
    if let Some(hashing) = declaration
        .restore_keys
        .iter()
        .find(|fallback| fallback.contains("${{"))
    {
        return Some(format!(
            "`{hashing}` is a fallback that still carries an expression, so it \
             moves whenever the primary key does and falls back to nothing"
        ));
    }
    // AND IT HAS TO BE THE PREFIX THIS READER USES. Every gate here joins on
    // `prefix`, DERIVED from the key; GitHub falls back to what is WRITTEN in
    // `restore-keys`. Two spellings of one identity that disagree leave the
    // gates answering about a key nothing ever tries — silently, because each
    // spelling looks right on its own.
    if !declaration
        .restore_keys
        .iter()
        .any(|fallback| fallback == &declaration.prefix)
    {
        return Some(format!(
            "this reader joins on `{}` and GitHub falls back to {:?} — two \
             spellings of one identity that do not agree",
            declaration.prefix, declaration.restore_keys
        ));
    }
    None
}

/// Every pair of declared caches where the FIRST one's fallback would restore
/// the SECOND one's archive, bringing paths the first does not ask for.
///
/// `restore-keys` IS A PREFIX MATCH OVER THE WHOLE REPOSITORY'S STORAGE, not
/// over one job's, and nothing in a workflow says otherwise. A cache whose
/// fallback prefix is a prefix of another cache's key falls back onto that
/// cache's archives, and an archive unpacks AS IT WAS STORED — the `path:` list
/// is what a cache SAVES, and has no say in what a restore of somebody else's
/// archive puts on disk.
///
/// SUBSET AND NOT EQUALITY, because the harmless case is the common one. This
/// repository's oldest key, `Linux-cargo-`, is a prefix of every other key in
/// the file, and six of those hold exactly the two cargo-home trees it holds
/// too: falling back onto one of those gets it the paths it asked for out of
/// somebody else's generation, which is what `restore-keys` is FOR. The pair
/// that matters is the one whose archive holds something the first cache never
/// declared, because then the fallback silently lands a tree nothing measured
/// and nothing asked for — and this repository's build directory is 8.90 GB of
/// exactly that.
///
/// R1160 — WHAT IS WRITTEN, NOT WHAT IS DERIVED, and the difference stopped
/// being academic in the same round this sentence was added. Until then every
/// cache here declared `restore-keys: <its own prefix>`, so reading the DERIVED
/// prefix and reading the WRITTEN fallback gave the same answer and the cheaper
/// one was taken. The build-directory cache now declares NO fallback at all, and
/// a reader still joining on its prefix would report a reach that GitHub is never
/// asked to make — a refusal for a reason outside the law, which is the shape
/// R1110 wrote down. So the loop asks each declaration what it actually falls
/// back to, and a declaration that falls back to nothing reaches nothing.
/// Whether the fallbacks a declaration actually WRITES reach a key.
///
/// R1160 — ONE HOME FOR THE JOIN, and reading the written list rather than the
/// derived prefix. Until this round every cache here wrote
/// `restore-keys: <its own prefix>`, so the two readings agreed and the cheaper
/// one was taken; the build-directory cache now writes NONE, and
/// `evidence-replay.yml` writes TWO — one of them `Linux-cargo-`, renamed away
/// from the other workflow at R1123 and still a prefix of the build directory's
/// key. The derived reading answered `Linux-cargo-replay- nests under nothing`
/// and was right about the wrong list. Both this function's callers — the pair
/// walk below and the control beside it in
/// `tools/cache-budget/tests/this_repository.rs` — ask here, because two
/// spellings of one join are two answers free to disagree.
pub fn falls_back_onto(cache: &CacheDeclaration, key: &str) -> bool {
    let written = &cache.restore_keys;
    written.iter().any(|w| key.starts_with(w.as_str()))
}

pub fn fallback_reaches(
    declared: &[CacheDeclaration],
) -> Vec<(&CacheDeclaration, &CacheDeclaration)> {
    let mut out = Vec::new();
    for cache in declared {
        for other in declared {
            if other.prefix == cache.prefix || !falls_back_onto(cache, &other.prefix) {
                continue;
            }
            let held: BTreeSet<&str> = other.paths.iter().map(String::as_str).collect();
            let asked: BTreeSet<&str> = cache.paths.iter().map(String::as_str).collect();
            if !held.is_subset(&asked) {
                out.push((cache, other));
            }
        }
    }
    out
}

/// The directory cargo builds into, as this repository configures it.
///
/// READ RATHER THAN SPELLED, because it is the join
/// [`inheritance_without_a_bound`] makes: a declared cache path is either the
/// build directory or it is not, and the two kinds have different bounds.
/// `.cargo/config.toml` is where this repository says it (`build.target-dir`)
/// and cargo's own default is `target`, so an absent file or an absent key is
/// ANSWERED rather than guessed at. A file that will not parse is refused, for
/// the reason `runner_os` refuses a label it does not know: a wrong answer here
/// reads the build-directory cache as an ordinary cargo-home one, which is the
/// quiet direction.
pub fn build_directory(root: &Path) -> String {
    const CARGO_DEFAULT: &str = "target";
    let path = root.join(".cargo").join("config.toml");
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return CARGO_DEFAULT.to_string();
    };
    let parsed: toml::Value = toml::from_str(&raw)
        .unwrap_or_else(|why| panic!("{} does not parse as TOML: {why}", path.display()));
    match parsed
        .get("build")
        .and_then(|build| build.get("target-dir"))
    {
        Some(dir) => dir
            .as_str()
            .unwrap_or_else(|| {
                panic!(
                    "{}: `build.target-dir` is {dir:?} and not a string",
                    path.display()
                )
            })
            .to_string(),
        None => CARGO_DEFAULT.to_string(),
    }
}

/// Why a cache declaration's fallback costs more every generation, with no bound
/// in this repository's contents.
///
/// `None` means the fallback is bounded, which is what `restore-keys` is FOR.
///
/// A FALLBACK IS INHERITANCE, and this repository's own log is where that stops
/// being a reading of somebody else's documentation. `actions/cache` writes an
/// archive only when the PRIMARY key missed — run 31599893855 printed `Cache hit
/// occurred on the primary key …, not saving cache` for both of `unrun-tests`'
/// caches — so the only run that writes is the run whose key moved, and by then
/// `restore-keys` has already put the PREVIOUS generation's tree on the disk
/// being saved. What gets written is the union of the two.
///
/// FOR A CARGO HOME THAT UNION IS BOUNDED. `~/.cargo/registry` holds one entry
/// per crate version some lockfile has named, and the six caches here holding
/// exactly those trees weigh 0.05–0.14 GB apiece.
///
/// FOR THE BUILD DIRECTORY IT IS NOT. Every generation leaves a full set of
/// artifacts under fresh hashes and nothing removes one, so the archive is every
/// tree this repository has ever built. Measured on run 31599893855: that cache
/// put 37,427 MB on disk, while a clean build of everything the job compiles
/// (`cargo test --workspace --all-features --no-run` plus `tools/unrun-tests`,
/// under the same `CARGO_INCREMENTAL=0` and
/// `CARGO_PROFILE_DEV_DEBUG=line-tables-only`, into an empty tree) is 3.86 GB.
/// NINE TENTHS OF WHAT THAT CACHE CARRIES IS IN NO BUILD AT ALL, and it is what
/// took this repository's declared caches to 10.75 GB against GitHub's 10 GB —
/// 7.83 GB at R1091, 9.95 GB here, with the declared paths never once changing.
pub fn inheritance_without_a_bound(
    declaration: &CacheDeclaration,
    build_directory: &str,
) -> Option<String> {
    if declaration.restore_keys.is_empty() {
        return None;
    }
    if !declaration
        .paths
        .iter()
        .any(|path| path.trim() == build_directory)
    {
        return None;
    }
    Some(format!(
        "`{}` holds the build directory `{build_directory}` and falls back to \
         {:?}, so the run whose primary key moves saves the union of its own \
         tree and every generation before it — a sum nothing in this \
         repository's contents bounds",
        declaration.prefix, declaration.restore_keys
    ))
}

/// One `actions/upload-artifact` step, as a workflow declares it.
///
/// WHAT MAKES A RECORD READABLE. Every record this repository's gates join —
/// what a job compiled, what its cache restore brought — is written to a file on
/// a runner that is destroyed when the job ends, and the only thing that
/// survives is what an upload step collected. So a workflow that uploads nothing
/// produces no record any later job can download, and a measurement written in
/// it is unreadable BY CONSTRUCTION rather than by anybody's choice.
///
/// That is why this is read rather than assumed: it is the derivation behind
/// which jobs owe a restore record at all, and the alternative is a list of
/// exempt workflows kept beside the law, which is the shape R777, R783, R1080
/// and R1082 each closed one level at a time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactUpload {
    /// The workflow file it is written in.
    pub source: String,
    /// The id of the job that runs it.
    pub owner: String,
    /// Where it sits in that job's `steps:` list — the same counting
    /// [`RunStep::index`] uses.
    pub index: usize,
    /// The `path:` entries, in the order written.
    pub paths: Vec<String>,
}

/// The variable a runner names the workflow it is executing in.
pub const WORKFLOW_VARIABLE: &str = "GITHUB_WORKFLOW_REF";

/// Which TRACKED workflow a runner's [`WORKFLOW_VARIABLE`] names.
///
/// A JOIN AND NOT A PARSE, AND ONE OF THEM RATHER THAN TWO. The runner spells it
/// `owner/repo/.github/workflows/x.yml@refs/heads/main`, and two gates need the
/// middle: the census gate loads that file, and `tools/cache-budget` compares it
/// with [`CacheDeclaration::source`] to decide whose restore records could have
/// been collected in this run at all. Two cuts of one string are two answers
/// free to disagree, which is the shape this crate exists to remove — so the cut
/// lives here, and it is checked against the files this repository actually
/// tracks rather than trusted.
///
/// The check is the part that matters. A path nothing tracks compares unequal to
/// every declaration's source, so a gate that merely cut the string would report
/// every job in the repository as belonging to some other workflow — a verdict
/// wearing the shape of a reading, with nothing behind it.
pub fn workflow_of_reference(
    reference: Option<&str>,
    tracked: &[String],
) -> Result<String, String> {
    let Some(reference) = reference.map(str::trim).filter(|it| !it.is_empty()) else {
        return Err(format!(
            "no {WORKFLOW_VARIABLE} in the environment, so which workflow this is \
             a run of is unknown — and every gate that asks is one that must not \
             guess"
        ));
    };
    // The two leading segments are the repository's and the tail from `@` is the
    // ref; a workflow path holds neither.
    let path = reference
        .splitn(3, '/')
        .nth(2)
        .map(|rest| rest.split('@').next().unwrap_or(rest))
        .unwrap_or_default();
    if !tracked.iter().any(|file| file == path) {
        return Err(format!(
            "{WORKFLOW_VARIABLE} is {reference:?}, whose workflow reads as \
             {path:?} — not one of the {} this repository tracks ({})",
            tracked.len(),
            tracked.join(", ")
        ));
    }
    Ok(path.to_string())
}

/// Does this workflow collect anything a later reader could download?
///
/// THE PREDICATE ITSELF, WITH ONE HOME. Two gates ask it — `tools/twice-compiled`
/// to decide which jobs owe a restore record, `tools/cache-budget` to decide
/// whose silence about a restore is the job's and whose is the gate's — and a
/// second spelling of it is a second answer free to drift from the first. It is
/// deliberately the loosest reading that is still true: a workflow uploading
/// ANYTHING can carry a record out, and matching an upload's `path:` against
/// where a record is written would need `${{ github.workspace }}` resolved,
/// which only GitHub can do.
pub fn collects_artifacts(uploads: &[ArtifactUpload]) -> bool {
    !uploads.is_empty()
}

/// Every tracked workflow that collects an artifact at all, by path.
///
/// The population a reader needs in order to say WHERE a record could have come
/// from. Read from the files for the reason everything else here is: a list of
/// collecting workflows kept beside a law is the shape R777, R783, R1080 and
/// R1082 each closed one level at a time.
pub fn workflows_collecting_artifacts(root: &Path) -> std::collections::BTreeSet<String> {
    let mut out = std::collections::BTreeSet::new();
    for path in workflow_files(root) {
        let doc = load_workflow(root, &path);
        if collects_artifacts(&artifact_uploads(&doc, &path)) {
            out.insert(path);
        }
    }
    out
}

/// Every `actions/upload-artifact` step in every job of one workflow.
pub fn artifact_uploads(doc: &Yaml, source: &str) -> Vec<ArtifactUpload> {
    let mut out = Vec::new();
    let Some(jobs) = doc["jobs"].as_hash() else {
        return out;
    };
    for (name, job) in jobs {
        let owner = name.as_str().unwrap_or("<unnamed>").to_string();
        let Some(steps) = job["steps"].as_vec() else {
            continue;
        };
        for (index, step) in steps.iter().enumerate() {
            let Some(uses) = step["uses"].as_str() else {
                continue;
            };
            if uses.split('@').next().unwrap_or(uses) != "actions/upload-artifact" {
                continue;
            }
            let paths: Vec<String> = step["with"]["path"]
                .as_str()
                .unwrap_or_else(|| panic!("{source}: job `{owner}` uploads no path"))
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_string)
                .collect();
            // EVERY UPLOAD STEP AND NOT THE FIRST: a job may collect more than
            // one thing, and stopping at one is a truncation that reads as a
            // workflow collecting less than it does.
            out.push(ArtifactUpload {
                source: source.to_string(),
                owner: owner.clone(),
                index,
                paths,
            });
        }
    }
    out
}

/// One step that runs somebody else's action rather than a shell command.
///
/// R1210 — the law asking which packages CI installs reads shell, and a step
/// with `uses:` runs no shell at all. Nothing in this crate could name those
/// steps in general (the two readers above each match ONE action by name), so a
/// law built on `run:` steps alone would report a workflow that installs its
/// tools through an action as installing nothing — the empty answer wearing the
/// shape of a clean one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsesStep {
    /// The workflow file it is written in.
    pub source: String,
    /// The id of the job that runs it — the spelling `needs:` uses.
    pub job: String,
    /// Where it sits in that job's `steps:` list — the same counting
    /// [`RunStep::index`] uses.
    pub index: usize,
    /// The whole `uses:` value, version included: what a reader must judge is
    /// the action a maintainer pinned, and the version is part of its identity.
    pub uses: String,
}

impl UsesStep {
    /// The action without its version — `actions/checkout` for
    /// `actions/checkout@v7`.
    #[must_use]
    pub fn action(&self) -> &str {
        self.uses.split('@').next().unwrap_or(&self.uses)
    }
}

/// Every `uses:` step in every job of one workflow.
pub fn uses_steps(doc: &Yaml, source: &str) -> Vec<UsesStep> {
    let mut out = Vec::new();
    let Some(jobs) = doc["jobs"].as_hash() else {
        return out;
    };
    for (name, job) in jobs {
        let name = name.as_str().unwrap_or("<unnamed>").to_string();
        let Some(steps) = job["steps"].as_vec() else {
            continue;
        };
        for (index, step) in steps.iter().enumerate() {
            if let Some(uses) = step["uses"].as_str() {
                out.push(UsesStep {
                    source: source.to_string(),
                    job: name.clone(),
                    index,
                    uses: uses.to_string(),
                });
            }
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

/// One job, as the two things a reader holding it to a budget needs.
///
/// A JOB'S NAME ON GITHUB IS NOT ITS ID. `needs:` spells the id; a check run on a
/// commit carries the `name:` the job declares, and only falls back to the id
/// when it declares none — this repository's `validate` is the one job that does.
/// Anything joining what a job WAS ALLOWED TO TAKE to what it TOOK has to cross
/// that gap, and a reader that returned only ids would leave every caller to
/// re-derive it from the same file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobBudget {
    /// The job id — the spelling `needs:` uses.
    pub id: String,
    /// The `name:` GitHub shows, when the job declares one.
    pub shown_as: Option<String>,
    /// Its `timeout-minutes`, VERBATIM, or `None` when it declares none — in
    /// which case GitHub applies its own 360-minute default, which is a budget
    /// nobody in this repository chose.
    ///
    /// KEPT AS WRITTEN, for [`RunStep::timeout`]'s reason: an expression is a
    /// declaration this reader cannot evaluate, and parsing to a number would
    /// answer `None` for it — the same word it answers for a job nobody bounded.
    pub timeout: Option<String>,
}

impl JobBudget {
    /// What a check run on a commit is called: the declared name, or the id.
    #[must_use]
    pub fn check_name(&self) -> &str {
        self.shown_as.as_deref().unwrap_or(&self.id)
    }
}

/// Every job one workflow declares, with what it is allowed to take.
pub fn job_budgets(doc: &Yaml) -> Vec<JobBudget> {
    let Some(jobs) = doc["jobs"].as_hash() else {
        return Vec::new();
    };
    jobs.iter()
        .map(|(name, job)| JobBudget {
            id: name.as_str().unwrap_or("<unnamed>").to_string(),
            shown_as: scalar(&job["name"]),
            timeout: scalar(&job["timeout-minutes"]),
        })
        .collect()
}

/// Every job this repository's workflows declare, workflow by workflow.
///
/// THE PAIR IS KEPT, because a job id is unique within its file and a CHECK NAME
/// is what a commit's answer carries — so a caller joining the two needs to say
/// which file a name came from when two files disagree.
pub fn workflow_job_budgets(root: &Path) -> Vec<(String, JobBudget)> {
    let mut out = Vec::new();
    for path in workflow_files(root) {
        let doc = load_workflow(root, &path);
        for job in job_budgets(&doc) {
            out.push((path.clone(), job));
        }
    }
    out
}

/// The same, for a caller that must not die where there are no workflows.
///
/// A FILE THAT WILL NOT PARSE IS NAMED AND THE REST ARE STILL READ, which is the
/// reporter's half of the rule beside [`load_workflow`]: one unreadable workflow
/// must not take the budgets of every other one down with it, and the caller
/// prints what could not be read rather than answering as though it were empty.
pub fn readable_job_budgets(root: &Path) -> (Vec<(String, JobBudget)>, Vec<String>) {
    let mut out = Vec::new();
    let mut unread = Vec::new();
    let files = match tracked_workflow_files(root) {
        Ok(files) => files,
        Err(why) => return (out, vec![why]),
    };
    for path in files {
        match read_workflow(root, &path) {
            Ok(doc) => {
                for job in job_budgets(&doc) {
                    out.push((path.clone(), job));
                }
            }
            Err(why) => unread.push(why),
        }
    }
    (out, unread)
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
    /// The program that HANDS THIS COMMAND OVER, and its own arguments — empty
    /// when the command is issued directly.
    ///
    /// R1196. A wrapper is not decoration: `scripts/verify.sh -- cargo test …`
    /// is the same cargo command with something around it that judges what the
    /// run covered, and which of the two a line is cannot be recovered from
    /// [`cargo_args`](Self::cargo_args) — the wrapper's words are gone by then.
    /// So the carrier is KEPT rather than discarded, and the law that every
    /// `cargo test` this repository issues is judged for coverage has a datum to
    /// read. Without it that law would have to re-read the shell a second time,
    /// which is the second-spelling shape this crate exists to remove.
    pub carrier: Vec<String>,
    /// Words from `cargo` up to (not including) a bare `--`, `cargo` first.
    pub cargo_args: Vec<String>,
    /// Words after the first bare `--`.
    pub harness_args: Vec<String>,
    /// The environment the command runs in — see [`RunStep::env`]. Empty for a
    /// command a gate builds for itself rather than reads out of a workflow.
    pub env: BTreeMap<String, String>,
    /// Whose lockfile this command resolves, WHEN THE SITE SAID SO.
    ///
    /// R1262, and it exists because the sixth source can say what the other five
    /// cannot. For a command written as data the answer is READ off the manifest
    /// path — `--manifest-path tools/x/Cargo.toml` names the workspace, and
    /// [`CargoCommand::manifest`] is the one reader of that. For a command a Rust
    /// program assembles, the path is a value: the same three words appear in the
    /// gate that walks this repository's workspaces and in the fixture that made
    /// a throwaway one two lines earlier, and no reading of the syntax separates
    /// them. So that site declares it ([`issue::Tree`]) and the declaration
    /// arrives here.
    ///
    /// `None` is not "unknown" — it is "read it off the manifest", which is the
    /// right answer for every source whose words are written down.
    pub declared: Option<rust::Declared>,
    /// How many of this command's words stand for a list of UNKNOWN LENGTH.
    ///
    /// R1266, and it exists because of what a hole can and cannot do: a hole
    /// cannot take a word away, so a flag spelled beside one is definitely
    /// there — and it may hold a word nobody wrote down, so a flag's ABSENCE
    /// cannot be claimed. Zero for every command written as data, where the words
    /// are all there is.
    ///
    /// `--locked` in `item-citations`' assembled argv is the case: the flag is a
    /// literal one statement above the spawn and the `extend` beside it is a
    /// runtime list, so [`lock_verdict`] can say the command pins and could never
    /// have said it did not.
    pub uncounted: usize,
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

    /// EVERY value of `--flag value` / `--flag=value` on cargo's side, for a
    /// flag cargo lets a command REPEAT.
    ///
    /// [`CargoCommand::value`] answers with the first, which is the right
    /// reading for a flag that can only be given once (`--manifest-path`) and
    /// the wrong one for `--test` / `--bin` / `--features`, where a command
    /// naming three targets and a reader seeing one is a half-answer that reads
    /// like a whole one.
    ///
    /// A flag whose next word is another flag names nothing here rather than
    /// naming that flag: `cargo test --test --locked` is a command cargo would
    /// refuse, and a gate that recorded `--locked` as a target name would go on
    /// to report a target that does not exist for a reason that is not the
    /// caller's.
    pub fn values(&self, names: &[&str]) -> Vec<&str> {
        let mut found = Vec::new();
        for (index, word) in self.cargo_args.iter().enumerate() {
            if names.contains(&word.as_str()) {
                match self.cargo_args.get(index + 1) {
                    Some(next) if !next.starts_with('-') => found.push(next.as_str()),
                    _ => {}
                }
                continue;
            }
            if let Some((head, tail)) = word.split_once('=') {
                if names.contains(&head) {
                    found.push(tail);
                }
            }
        }
        found
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

    /// Which workspace's lockfile this command resolves.
    ///
    /// A command with no `--manifest-path` resolves the tree it is run in, and
    /// every caller of this crate runs its commands from the repository root.
    pub fn manifest(&self, tracked: &[String]) -> ManifestTarget {
        let Some(written) = self.value(&["--manifest-path"]) else {
            return ManifestTarget::Root;
        };
        // A path a shell assembles at runtime still NAMES a manifest, as long as
        // the name is in the literal part: `"$program_root/tools/x/Cargo.toml"`
        // is one manifest whatever `$program_root` turns out to be. What it is
        // not allowed to be is ambiguous — `"$ws/Cargo.toml"` has a literal tail
        // too, and that tail is `Cargo.toml`, which every workspace in the
        // repository has one of. So a tail under a variable must carry a
        // directory of its own; `Cargo.toml` alone names the root workspace when
        // it is the whole path and names nothing when something precedes it.
        let literal = written
            .rsplit_once('$')
            .map(|(_, after)| after.split_once('/').map(|(_, tail)| tail).unwrap_or(""));
        let (needle, must_be_qualified) = match literal {
            Some(tail) => (tail, true),
            None => (written, false),
        };
        if must_be_qualified && !needle.contains('/') {
            return ManifestTarget::Unreadable(written.to_string());
        }
        // THE DEEPEST MATCH WINS, and this is not a tie-break — it is the whole
        // reading. `Cargo.toml` is itself a tracked manifest (the root one) and
        // it is a suffix of every other, so a walk that demanded exactly one
        // match would call every path in this repository ambiguous. Two tracked
        // manifests cannot both be proper suffixes of the same string at the
        // same length, so the longest is unique.
        tracked
            .iter()
            .filter(|candidate| {
                needle == candidate.as_str() || needle.ends_with(&format!("/{candidate}"))
            })
            .max_by_key(|candidate| candidate.len())
            .map_or_else(
                || ManifestTarget::Unreadable(written.to_string()),
                |found| ManifestTarget::Named(found.clone()),
            )
    }
}

/// Which manifest a command was pointed at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestTarget {
    /// No `--manifest-path`: the workspace of the directory it runs in.
    Root,
    /// A tracked manifest, named by the literal tail of what was written.
    Named(String),
    /// Written in a way this reader cannot resolve to one tracked manifest.
    /// NOT a pass — a gate that cannot say which workspace a command resolves
    /// cannot say whether that workspace's lockfile is one this repository
    /// pins, and reporting it as compliant is the empty answer that reads like
    /// a clean one.
    Unreadable(String),
}

/// Does this cargo subcommand REWRITE a lockfile it disagrees with?
///
/// `None` for a subcommand this table does not know. Every answer here is
/// measured against cargo itself by `locked_resolution_smoke`, which builds a
/// workspace whose lockfile names a version its manifest does not and runs each
/// of these to see which files move — so this is a recording of an experiment
/// rather than a reading of the documentation, and a cargo release that changes
/// one of them turns that test red instead of quietly widening the hole.
///
/// The two `false`s are the interesting ones: `fmt` and `sweep` both REFUSE
/// `--locked` outright, so requiring the flag everywhere would be a law no
/// correct command could satisfy.
///
/// THE SUBCOMMAND IS NOT ALWAYS THE WHOLE QUESTION, which is why every judge
/// asks [`resolves_the_lockfile_of`] and not this. See it for the one flag that
/// changes the answer.
pub fn resolves_the_lockfile(subcommand: &str) -> Option<bool> {
    match subcommand {
        "bench" | "build" | "check" | "clean" | "clippy" | "doc" | "fix" | "generate-lockfile"
        | "metadata" | "package" | "run" | "rustc" | "rustdoc" | "test" | "tree" => Some(true),
        "fmt" | "sweep" => Some(false),
        _ => None,
    }
}

/// Does THIS COMMAND resolve the lockfile of the workspace it points at?
///
/// R1262, and the reason it is a second function rather than a wider table: the
/// answer depends on one flag, and the fact was ALREADY MEASURED in this
/// repository and never reached the judge.
///
/// `locked_resolution_smoke` has pinned, since the round that wrote it, that
/// `cargo metadata --no-deps` never resolves — "the cheap check that is not
/// one", asserted against cargo in the same fixture as everything else here.
/// [`resolves_the_lockfile`] answers about `metadata` all the same, because a
/// table keyed on the subcommand cannot say it. Nothing had noticed, because no
/// command written as DATA in this repository is a `metadata --no-deps`: the
/// sixth source is the first to bring one, and it brings seven.
///
/// What that would have cost, had this round trusted the table: seven `--locked`
/// flags added to commands the repository's own measurement says the flag can do
/// nothing for — a law demanding a word with no effect, which is the shape of a
/// gate people learn to satisfy rather than read.
pub fn resolves_the_lockfile_of(command: &CargoCommand) -> Option<bool> {
    let subcommand = command.subcommand()?;
    if subcommand == "metadata" && command.has("--no-deps") {
        return Some(false);
    }
    resolves_the_lockfile(subcommand)
}

/// Does this command RUN tests, rather than merely ask cargo about them?
///
/// R1266, and it is the same shape as [`resolves_the_lockfile_of`] one function
/// up: THE SUBCOMMAND IS NOT THE WHOLE QUESTION. `cargo test` with `--list`
/// after the bare `--` prints the names a harness knows and runs nothing;
/// `--no-run` builds the binaries and runs nothing. Both are `cargo test` to a
/// reader keyed on the subcommand alone, and the law that every test RUN goes
/// through the coverage wrapper has no business with either — a listing has no
/// coverage to hold a log against.
///
/// WHY IT IS HERE AND NOT IN THE LAW. Two programs already assemble a
/// `cargo test … -- --list` for exactly this reason (`unrun-tests` to enumerate
/// what a suite holds, `item-citations` to ask a harness for the names rustdoc
/// cannot see), so the distinction is a fact about the command and this crate is
/// where facts about commands live. It surfaced when R1266 made the second of
/// those readable: it walked into the wrapper law's population, which had never
/// met a listing, and the law was right to refuse a command it could not place.
#[must_use]
pub fn runs_tests(command: &CargoCommand) -> bool {
    command.subcommand() == Some("test")
        && !command.has("--no-run")
        && !command.harness_args.iter().any(|word| word == "--list")
}

/// Every `Cargo.toml` this repository tracks, sorted.
pub fn tracked_manifests(root: &Path) -> Vec<String> {
    let mut found = tracked_files(root, &["ls-files", "*Cargo.toml"]);
    found.sort();
    assert!(
        !found.is_empty(),
        "this repository tracks no Cargo.toml at all — the empty answer that \
         looks like a clean one"
    );
    found
}

/// Every tracked shell file that can issue a cargo command: the git hooks and
/// `scripts/`. Read from `git ls-files` for [`workflow_files`]'s reason — an
/// untracked script is not one a fresh clone runs.
pub fn script_files(root: &Path) -> Vec<String> {
    let mut found = tracked_files(root, &["ls-files", ".githooks", "scripts"]);
    found.retain(|path| {
        let head = std::fs::read_to_string(root.join(path)).unwrap_or_default();
        head.starts_with("#!")
            && head
                .lines()
                .next()
                .is_some_and(|line| line.contains("bash"))
    });
    found.sort();
    assert!(
        !found.is_empty(),
        "this repository tracks no shell script at all — the empty answer that \
         looks like a clean one"
    );
    found
}

fn tracked_files(root: &Path, args: &[&str]) -> Vec<String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .expect("git ls-files runs");
    assert!(
        out.status.success(),
        "git ls-files failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_string)
        .collect()
}

/// What one cargo command says about the lockfile it resolves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LockVerdict {
    /// It resolves a workspace this repository pins, and it says `--locked`.
    Pinned,
    /// It cannot rewrite a lockfile, so the flag would be wrong (and `fmt` and
    /// `sweep` both reject it).
    ResolvesNothing,
    /// It resolves a workspace this repository does not pin, and correctly does
    /// not say `--locked` — the flag there is a gate that another repository's
    /// commit turns red.
    NotOursToPin,
    /// THE DEFECT: it resolves a workspace this repository pins, without the
    /// flag, so a lockfile that disagrees with its manifests is REWRITTEN
    /// instead of reported — and every check after it in the same run reads the
    /// repaired file.
    RepairsWhatItShouldReport,
    /// THE MIRROR: `--locked` on a workspace whose resolution belongs to
    /// another tree, which fails on a commit nobody here made.
    PinsWhatItDoesNotOwn,
    /// This reader could not say which workspace the command resolves, or what
    /// its subcommand does to a lockfile. Not a pass.
    Unreadable(String),
}

/// Judge one command. `foreign` holds the workspace directories the lister
/// reported as resolving against trees outside this repository.
pub fn lock_verdict(
    command: &CargoCommand,
    tracked: &[String],
    foreign: &BTreeSet<String>,
) -> LockVerdict {
    let Some(subcommand) = command.subcommand() else {
        return LockVerdict::Unreadable("no subcommand at all".to_string());
    };
    match resolves_the_lockfile_of(command) {
        None => {
            return LockVerdict::Unreadable(format!(
                "`cargo {subcommand}` is a subcommand nobody has measured against a \
                 disagreeing lockfile"
            ))
        }
        Some(false) => return LockVerdict::ResolvesNothing,
        Some(true) => {}
    }
    // THE DECLARATION WINS WHERE THERE IS ONE, and it is not a shortcut past the
    // reading below — it is the only answer available. R1262: a command a Rust
    // program assembles points at a manifest that is a VALUE, so the reading
    // below answers `Unreadable` for the gate walking this repository's own
    // workspaces exactly as it does for the fixture next door. The site is the
    // only party that knows which, so the site says.
    let ours = match &command.declared {
        Some(rust::Declared::ThisRepository) => true,
        Some(rust::Declared::MadeByThisRun(_)) => false,
        // THE ARM THAT DECLINED TO NAME THE TREE OWES THE MOST. Getting here
        // means the command resolves — the check above returned otherwise — so
        // the site is running something that can rewrite a lockfile over a
        // workspace nobody named. That is not a pass and it is not a hole in
        // this reader: it is a declaration the words contradict.
        Some(rust::Declared::WhereverTheCallerPoints(why)) => {
            return LockVerdict::Unreadable(format!(
                "declared as running wherever the caller points ({why}), and yet \
                 `cargo {subcommand}` resolves — a command that can rewrite a \
                 lockfile has to say whose, or pin it whosever it is"
            ))
        }
        // THE OTHER WAY TO PAY FOR DECLINING TO NAME A TREE. This arm asks to be
        // judged as though every tree were one to pin, which is the strictest
        // reading available and the only one that is safe when the answer is not
        // known: `--locked` over a tree nobody here owns fails loudly, and over
        // one we do own it is exactly the flag the law wants. So `ours` is true,
        // and the pair below decides the verdict as it does for any other
        // command.
        Some(rust::Declared::PinnedWhereverItPoints(_)) => true,
        // THE FLAG IS THE ANSWER, BECAUSE THE SITE SAID IT WOULD BE. This arm
        // declares that `--locked` is present exactly on the paths where the
        // tree turns out to be one of ours, so reading ownership off the flag is
        // reading the declaration rather than assuming it. Both ways then pass,
        // WHICH IS WHY THE TEETH ARE SOMEWHERE ELSE: the site owes a `--locked`
        // that is CONDITIONAL, held by `a_site_that_pins_when_the_tree_is_ours…`
        // over the site rather than over the command, because a command is one
        // path and the claim is about two.
        Some(rust::Declared::PinnedWhenItIsOurs(_)) => command.has("--locked"),
        Some(rust::Declared::Unreadable(written)) => {
            return LockVerdict::Unreadable(format!(
                "the tree it runs over is declared as `{written}`, which this \
                 reader cannot read"
            ))
        }
        None => {
            let manifest = match command.manifest(tracked) {
                ManifestTarget::Root => "Cargo.toml".to_string(),
                ManifestTarget::Named(path) => path,
                ManifestTarget::Unreadable(written) => {
                    return LockVerdict::Unreadable(format!(
                        "`--manifest-path {written}` does not name one manifest this \
                         repository tracks"
                    ))
                }
            };
            !foreign.iter().any(|directory| {
                manifest == format!("{directory}/Cargo.toml")
                    || manifest.starts_with(&format!("{directory}/"))
            })
        }
    };
    match (ours, command.has("--locked")) {
        (true, true) => LockVerdict::Pinned,
        (false, true) => LockVerdict::PinsWhatItDoesNotOwn,
        // A HOLE CANNOT TAKE A WORD AWAY, WHICH IS WHY THE TWO ARMS ABOVE ARE
        // SAFE AND THESE TWO ARE NOT (R1266). The flag being THERE is a fact the
        // words carry whatever else the command holds; the flag being ABSENT is
        // a claim about words this reader could not count, and a list of unknown
        // length may hold it. So the verdict is refused rather than guessed —
        // `Unreadable` is this repository's "not a pass", and it stays that.
        (_, false) if command.uncounted > 0 => LockVerdict::Unreadable(format!(
            "`--locked` is not among the words this reader can name, and {} of \
             them stand for a list of unknown length that may hold it",
            command.uncounted
        )),
        (true, false) => LockVerdict::RepairsWhatItShouldReport,
        (false, false) => LockVerdict::NotOursToPin,
    }
}

/// Every cargo invocation in every tracked shell script.
pub fn script_cargo_commands(root: &Path) -> Vec<CargoCommand> {
    let mut out = Vec::new();
    for path in script_files(root) {
        let text = std::fs::read_to_string(root.join(&path)).expect("read script");
        for found in parse_script(&text) {
            out.push(CargoCommand {
                source: path.clone(),
                owner: path.clone(),
                carrier: found.carrier,
                cargo_args: found.cargo_args,
                harness_args: found.harness_args,
                env: BTreeMap::new(),
                declared: None,
                uncounted: 0,
            });
        }
    }
    out
}

/// Where this repository declares how a build machine verifies it.
pub const BUILD_DECLARATION: &str = ".claude/remote-build.toml";

/// Every cargo invocation this repository's build-machine declaration issues.
///
/// THE FOURTH PLACE A CARGO COMMAND IS WRITTEN, and until R1197 it was in no
/// law's population at all. `[commands]` holds the words a build machine runs to
/// verify this repository — the same suite CI runs, issued from somewhere else —
/// and they had neither `--locked` nor a coverage judgement while every other
/// command in the tree had both.
///
/// WHY NOTHING REACHED THEM: the program that consumes this file answers about
/// TOP-LEVEL keys only, so `tools/unread-declaration` prints these three as
/// outside its namespace and judges nothing about them. That is a true statement
/// about THAT reader and it was mistaken here for a limit on every reader. These
/// are cargo commands written in a tracked file of this repository, and this
/// repository's own laws are exactly the ones that can be asked about them.
///
/// NOT ADDED TO THE `unrun-tests` POPULATION, and that is a boundary rather than
/// an oversight. Its law is that every test this repository compiles is one some
/// CI command runs; a build machine is not CI, and folding its suite into the
/// RUN set would let a test that only ever runs there read as covered.
pub fn declared_build_commands(root: &Path) -> Vec<CargoCommand> {
    let path = root.join(BUILD_DECLARATION);
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    let parsed: toml::Value = toml::from_str(&raw)
        .unwrap_or_else(|why| panic!("{} does not parse as TOML: {why}", path.display()));
    let Some(commands) = parsed.get("commands").and_then(toml::Value::as_table) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (role, written) in commands {
        let Some(line) = written.as_str() else {
            panic!("{BUILD_DECLARATION}: `commands.{role}` is {written:?} and not a string");
        };
        for found in parse_script(line) {
            out.push(CargoCommand {
                source: BUILD_DECLARATION.to_string(),
                owner: role.clone(),
                carrier: found.carrier,
                cargo_args: found.cargo_args,
                harness_args: found.harness_args,
                env: BTreeMap::new(),
                declared: None,
                uncounted: 0,
            });
        }
    }
    out
}

/// Every tracked injection-sweep manifest.
///
/// The convention is the substring `sweep.json`, and it is the same one the
/// build-machine declaration uses to decide that running a sweep WRITES this
/// tree (R1241). One convention, now three laws.
pub fn sweep_manifests(root: &Path) -> Vec<String> {
    let mut found = tracked_files(root, &["ls-files"]);
    found.retain(|path| path.ends_with("sweep.json") && !path.ends_with(".firings.json"));
    found.sort();
    found
}

/// Every cargo invocation this repository's injection sweeps issue.
///
/// THE FIFTH PLACE A CARGO COMMAND IS WRITTEN, and until Round 1256 it was in no
/// law's population — the same shape the build declaration was in until R1197,
/// and the fourth time this repository has found one. Each tracked
/// `*sweep.json` carries a `test_command` as an ARRAY OF WORDS: the suite the
/// injection harness runs against an edited tree, once for the control and once
/// per injection.
///
/// WHY IT MATTERS MORE HERE THAN ANYWHERE ELSE, and the reason is the harness's
/// own contract rather than tidiness. A sweep EDITS the tree, runs, and puts it
/// back — restoring exactly the files it edited. A cargo command without
/// `--locked` that finds a lockfile disagreeing with its manifests REWRITES it,
/// and that file is not one the harness edited, so it is not one the harness
/// restores. The sweep would then leave behind a change nobody made and nobody
/// reverts, in the same run that claims the tree was returned to what it was.
///
/// NO SHELL PARSING: the array is already the words `cargo_invocation` takes,
/// which is why this reader is a dozen lines and not a second parser.
pub fn sweep_cargo_commands(root: &Path) -> Vec<CargoCommand> {
    let mut out = Vec::new();
    for path in sweep_manifests(root) {
        let raw = std::fs::read_to_string(root.join(&path)).expect("read sweep manifest");
        let doc: serde_json::Value = serde_json::from_str(&raw)
            .unwrap_or_else(|why| panic!("{path} does not parse as JSON: {why}"));
        let Some(written) = doc.get("test_command") else {
            panic!("{path} reads as a sweep manifest and declares no `test_command`");
        };
        let words: Vec<String> = written
            .as_array()
            .unwrap_or_else(|| panic!("{path}: `test_command` is {written} and not an array"))
            .iter()
            .map(|word| {
                word.as_str()
                    .unwrap_or_else(|| panic!("{path}: `test_command` holds a non-string {word}"))
                    .to_string()
            })
            .collect();
        let Some(found) = cargo_invocation(&words) else {
            panic!("{path}: `test_command` is not a cargo command — {words:?}");
        };
        out.push(CargoCommand {
            source: path.clone(),
            owner: path.clone(),
            carrier: found.carrier,
            cargo_args: found.cargo_args,
            harness_args: found.harness_args,
            env: BTreeMap::new(),
            declared: None,
            uncounted: 0,
        });
    }
    out
}

/// Every cargo command this repository issues ON THE MACHINE ASKING, and the
/// workspaces that machine could not reach.
///
/// # Both halves, because the population is machine-conditional
///
/// R1228 measured what this type is for. The lister answers with the
/// workspaces present HERE, so on a workstation holding the sibling checkout
/// this population is `studio`'s eight commands larger than it is on a hosted
/// runner — and until this round the function handed back the commands alone
/// and dropped the reason. Three laws read it that way (`locked_resolution_
/// smoke`, `build_width`, `judged_test_runs`) and not one of them could say
/// what it had not judged, while the three laws that take the lister's answer
/// directly all say it. The difference was not the authors' care: it was that
/// this shape had nowhere to put the second fact.
///
/// Returning it means a caller RECEIVES what it could not reach, which the
/// compiler enforces, and a caller that then says nothing about it is a
/// decision somebody made rather than a fact nobody was handed.
#[derive(Debug, Clone, Default)]
pub struct IssuedCommands {
    /// The commands, from all six sources.
    pub commands: Vec<CargoCommand>,
    /// The workspaces the lister declined on this machine, with its reasons.
    /// Empty is a real answer — it says every separate workspace was reachable
    /// here — and is not the same as never having asked.
    pub skipped: Vec<SkippedWorkspace>,
    /// What the SIXTH source read and could not finish reading — see
    /// [`rust::RustSpawns`]. Carried for the reason `skipped` is: a law that
    /// receives the residue can say its size, and one that never sees it cannot
    /// say anything at all.
    pub rust: rust::RustSpawns,
}

impl IssuedCommands {
    /// The lister's answer decides BOTH halves, so they are taken from it
    /// together. A constructor rather than two field writes at the call site:
    /// the pairing is the invariant, and a caller that could build one half
    /// without the other is a caller that eventually does.
    #[must_use]
    pub fn from_lister(listed: &Workspaces) -> Self {
        Self {
            commands: lister_declared_commands(listed),
            skipped: listed.skipped.clone(),
            rust: rust::RustSpawns::default(),
        }
    }

    /// Add commands from a source that is not the lister — a source that says
    /// nothing about which workspaces this machine has.
    pub fn extend(&mut self, more: impl IntoIterator<Item = CargoCommand>) {
        self.commands.extend(more);
    }
}

/// EVERY cargo command this repository issues, from all five sources.
///
/// ONE ASSEMBLY, because more than one law asks about the same population and a
/// second assembly is a second answer: `locked_resolution_smoke` asks whether
/// each one pins its lockfile and R1212's `build_width` asks whether any of them
/// decides how wide it runs. The one subtlety belongs here rather than in each
/// caller — `scripts/check-side-workspaces.sh` is dropped from the SHELL source
/// because the lister reports the same commands assembled, and reading its text
/// as well would ask twice about words that are `"${locked[@]}"` on the page.
///
/// THE ORDER IS THE LISTER'S FIRST since R1228, because the lister's answer is
/// what carries the second half and taking both from it together is what makes
/// the pairing a constructor rather than two writes. No law reads this
/// population by position — they tally it by source or scan it whole — and the
/// change is written here rather than left for somebody to discover.
pub fn commands_this_repository_issues(root: &Path) -> IssuedCommands {
    let mut issued = IssuedCommands::from_lister(&workspaces(root));
    issued.extend(workflow_cargo_commands(root));
    issued.extend(
        script_cargo_commands(root)
            .into_iter()
            .filter(|command| command.source != "scripts/check-side-workspaces.sh"),
    );
    issued.extend(declared_build_commands(root));
    issued.extend(sweep_cargo_commands(root));
    // THE SIXTH SOURCE, and the only one that hands back a residue of its own:
    // what a Rust program spells, this reader reads, and what it computes is
    // counted rather than guessed at (R1262).
    let read = rust::cargo_commands(root);
    issued.extend(read.commands.clone());
    issued.rust = read;
    issued
}

/// The words in one command that decide how wide it runs, if it decides at all.
///
/// R1212 — THE WIDTH IS THE RUNNER'S DECISION, and a command that writes one
/// takes it away from every place a scheduler can set one. Measured on this
/// machine's cargo, three arms with a fresh build directory each: this tree's
/// `[build] jobs = 4` gives a build script `NUM_JOBS=4`; `CARGO_BUILD_JOBS=8`
/// against that same config gives 8; and `-j 2` against that same variable gives
/// 2. So a flag beats the environment, which is how the build machine is sized —
/// `bx` exports `CARGO_BUILD_JOBS` and `RUST_TEST_THREADS` per host, measured at
/// 31 on a 32-core one where this tree's config says 4.
///
/// Both sides are read. `--jobs` bounds compiling; `--test-threads` after the
/// bare `--` bounds running, and this repository's memory lives on that side.
#[must_use]
pub fn decides_its_own_width(command: &CargoCommand) -> Vec<String> {
    let mut found = Vec::new();
    for word in &command.cargo_args {
        // `-j`, `-j4`, `--jobs`, `--jobs=4` — one flag in four spellings, and a
        // reader that knew one of them would answer "absent" for the others.
        let short_with_value = word.starts_with("-j")
            && word.len() > 2
            && word[2..].chars().all(|c| c.is_ascii_digit());
        if word == "-j" || word == "--jobs" || word.starts_with("--jobs=") || short_with_value {
            found.push(word.clone());
        }
    }
    for word in &command.harness_args {
        if word == "--test-threads" || word.starts_with("--test-threads=") {
            found.push(word.clone());
        }
    }
    found
}

/// Every cargo invocation in every job of every tracked workflow.
pub fn workflow_cargo_commands(root: &Path) -> Vec<CargoCommand> {
    let mut out = Vec::new();
    for path in workflow_files(root) {
        let doc = load_workflow(root, &path);
        for step in run_steps(&doc) {
            for found in parse_script(&step.script) {
                out.push(CargoCommand {
                    source: path.clone(),
                    owner: step.job.clone(),
                    carrier: found.carrier,
                    cargo_args: found.cargo_args,
                    harness_args: found.harness_args,
                    env: step.env.clone(),
                    declared: None,
                    uncounted: 0,
                });
            }
        }
    }
    out
}

/// The cargo invocation a list of shell words ultimately runs, with whatever
/// hands it over kept beside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    /// The wrapper and its own arguments, empty when cargo is the first word.
    pub carrier: Vec<String>,
    /// Words from `cargo` up to (not including) its bare `--`, `cargo` first.
    pub cargo_args: Vec<String>,
    /// Words after that bare `--`.
    pub harness_args: Vec<String>,
}

/// Cargo's side and the harness's, split at the first bare `--`.
fn split_at_the_bare_marker(words: &[String]) -> (Vec<String>, Vec<String>) {
    match words.iter().position(|word| word == "--") {
        Some(at) => (words[..at].to_vec(), words[at + 1..].to_vec()),
        None => (words.to_vec(), Vec::new()),
    }
}

/// Read one command's words for the cargo invocation they run, `None` when they
/// run none.
///
/// A BARE `--` HANDS THE REST OVER, and that is the whole of R1196. Until this
/// round a segment counted only when its first word was `cargo`, so wrapping a
/// suite in `scripts/verify.sh -- cargo test …` — which is what puts a run under
/// the coverage law of `tools/unreported-targets` — DELETED that command from
/// every population built on this crate. The gate asking which tests CI runs
/// would have lost the largest command in the tree and reported the tests behind
/// it as dark; the gate asking which commands pin their lockfile would have
/// stopped seeing it at all, which is the quiet direction.
///
/// So the reading is the general one: everything up to a bare `--` belongs to
/// the program in front, and what follows is a command in its own right. It
/// recurses, because a wrapper may carry a wrapper. Cargo's own `--` is NOT this
/// — a command whose head is already `cargo` is split once and never re-read,
/// or `cargo test -- --nocapture` would go looking for a second command in its
/// harness arguments.
///
/// WHAT IT CANNOT DO IS INVENT ONE. `grep -- cargo file` would read as a cargo
/// command here, and that is deliberate over the alternative of a list of known
/// wrappers: the mistake is LOUD (the judging gates answer `Unreadable` for a
/// subcommand nobody has measured) where a stale list of wrappers is silent.
pub fn cargo_invocation(words: &[String]) -> Option<Invocation> {
    let mut carrier: Vec<String> = Vec::new();
    let mut words = words.to_vec();
    loop {
        // `if ! cargo test …` is a cargo command, and a reader that took the
        // first word literally answers `if`. R1115 found four of them in the
        // git hooks — every one an unlocked resolve, none of them in any
        // gate's population, because the word in front of `cargo` was a
        // shell keyword rather than another command. A keyword is not a
        // carrier: nothing runs it, so it is dropped rather than recorded.
        while words
            .first()
            .is_some_and(|word| SHELL_PREFIXES.contains(&word.as_str()))
        {
            words.remove(0);
        }
        if words.first().map(String::as_str) == Some("cargo") {
            let (cargo_args, harness_args) = split_at_the_bare_marker(&words);
            return Some(Invocation {
                carrier,
                cargo_args,
                harness_args,
            });
        }
        // Not cargo, so this is either a wrapper or not a cargo command at all,
        // and the bare `--` is what tells them apart.
        let at = words.iter().position(|word| word == "--")?;
        carrier.extend(words.drain(..at));
        words.remove(0);
    }
}

/// Every command one shell script holds, as words, empty segments dropped.
///
/// A `run:` step is shell, and this reads the part of shell the workflows
/// actually use: continuation lines, and the `&& || ; |` operators that put two
/// commands on one line.
///
/// IT IS PUBLIC BECAUSE A SECOND LAW ASKS ABOUT THE SAME SEGMENTS. R1210 asks
/// which steps INSTALL something onto the machine CI runs on, which is a
/// question about commands that are not cargo's, and the alternative was a
/// second splitter — the exact shape this crate exists to prevent, and the one
/// that would answer differently the first time a workflow put two commands on
/// one line in a way only one of the two readers understood.
pub fn shell_commands(script: &str) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    for line in join_continuations(script) {
        for segment in split_operators(&line) {
            let words = words_of(&segment);
            if !words.is_empty() {
                out.push(words);
            }
        }
    }
    out
}

/// Split one shell script into the cargo invocations it holds.
///
/// Which segments hold a cargo command is [`cargo_invocation`]'s answer — asking
/// whether the word appears anywhere reads `--manifest-path tools/x/Cargo.toml`
/// inside one command as the start of another.
pub fn parse_script(script: &str) -> Vec<Invocation> {
    shell_commands(script)
        .iter()
        .filter_map(|words| cargo_invocation(words))
        .collect()
}

/// Words a shell may put in front of a command without making it a different
/// one. `time` and `env` are deliberately absent: both take arguments of their
/// own, so skipping them would read the wrong word as the subcommand.
const SHELL_PREFIXES: &[&str] = &[
    "if", "then", "else", "elif", "while", "until", "do", "!", "{",
];

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

impl SkippedWorkspace {
    /// The one sentence this repository says a workspace was not reached with,
    /// and the verb of the law saying it.
    ///
    /// ONE SPELLING, R1228. Three programs had written this line themselves —
    /// `uncompiled-sources` and `unrun-tests` "not probed", the feature-coverage
    /// law "not asked" — and three more laws over the same population had
    /// written no line at all. A sentence copied three times is a sentence that
    /// drifts three ways, and a reader comparing a run here with a run on a
    /// hosted runner is comparing exactly these lines.
    #[must_use]
    pub fn was_not(&self, verb: &str) -> String {
        format!("not {verb} (the lister says why): {self}")
    }
}

/// Whose resolution a workspace's lockfile records.
///
/// NOT THE SAME QUESTION AS [`SkippedWorkspace`], which is about this machine.
/// A workspace is `Foreign` on every machine including the one that can compile
/// it, because what makes it foreign is a path dependency landing outside this
/// checkout — and a lockfile over somebody else's tree changes when they
/// commit. R1115 separated the two after one predicate served both and the
/// second fact had nowhere to be said: `studio/Cargo.lock` was tracked, stale,
/// and rewritten by every run of the gate that was supposed to check it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ownership {
    /// Every path dependency lands inside this checkout, so this repository can
    /// pin the resolution and every resolving command over it says `--locked`.
    Ours,
    /// It resolves against a tree this repository does not own. The lister's own
    /// words for which dependencies those are.
    Foreign(String),
}

/// One cargo command the lister declares it runs, in the words it runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredCommand {
    /// The workspace directory it is issued for.
    pub workspace: String,
    /// Which of the gate's checks it is — `fmt`, `clippy`, `citations`,
    /// `waits`, `suite`. A ROLE RATHER THAN A GUESS FROM THE SUBCOMMAND: two
    /// roles here are `cargo run` of a different gate, and a reader that
    /// recovered the role from the words would have to know their binaries.
    pub role: String,
    /// `cargo` first, exactly as the shell expands it.
    pub words: Vec<String>,
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
    /// Whose resolution each separate workspace's lockfile records. The root
    /// workspace is not in the lister's population, so it is not in here.
    pub ownership: BTreeMap<String, Ownership>,
    /// Every command the lister declares, in order. ONE LIST RATHER THAN ONE
    /// PER ROLE: the suite used to be the only declared command and the four
    /// that run before it were invisible, which is how four unlocked resolves
    /// sat in front of the one `--locked` that was supposed to catch a stale
    /// lockfile.
    pub commands: Vec<DeclaredCommand>,
}

/// Ask the lister. Panics rather than guessing: a gate that cannot reach the
/// one program holding this answer must not fall back to deriving it, which is
/// exactly the failure R1083 repaired.
pub fn workspaces(root: &Path) -> Workspaces {
    ask_lister(root, &[])
}

/// Ask the lister for the workspaces whose verdict is THIS repository's to give.
///
/// The same answer minus the ones that resolve against a tree this checkout does
/// not own — the lister's `--ours-only`, which it decides from the ownership it
/// already prints (`LOCK <ws> ours|foreign`) rather than from any name written
/// down. R1225 added the flag for `pre-push`, and the reason generalises to
/// every gate a push runs: a workspace whose compilation depends on somebody
/// else's working tree cannot answer a question about this commit. Round 1230
/// is where a second gate needed it.
///
/// NOT A SUBSTITUTE FOR [`workspaces`]: CI has no sibling checkout and must
/// judge the whole population it can reach, so the flag is a caller's choice
/// about what its verdict is ABOUT, never a default.
pub fn workspaces_ours_only(root: &Path) -> Workspaces {
    ask_lister(root, &["--ours-only"])
}

/// The workspace DIRECTORIES this repository cannot pin — the ones resolving
/// against a tree it does not own.
///
/// ONE SPELLING FOR A QUESTION TWO GATES ASK, and the second one asked it wrong.
/// `locked_resolution_smoke` carried this filter privately, and
/// `feature_coverage_smoke` reasoned from the MANIFEST instead — "the manifest is
/// one this repository tracks, so the command may pin" — which is true of the
/// manifest and false of the lockfile. `studio` tracks its manifest here and
/// path-depends on a sibling checkout, so its `Cargo.lock` is gitignored and
/// moves whenever that sibling does: `--locked` over it is a gate somebody
/// else's commit turns red, which is `LockVerdict::PinsWhatItDoesNotOwn` written
/// out by hand. R1263 met it as a red root suite the day the sibling moved its
/// own dependency pin.
#[must_use]
pub fn workspaces_this_repository_cannot_pin(root: &Path) -> BTreeSet<String> {
    workspaces(root)
        .ownership
        .into_iter()
        .filter(|(_, ownership)| matches!(ownership, Ownership::Foreign(_)))
        .map(|(directory, _)| directory)
        .collect()
}

fn ask_lister(root: &Path, extra: &[&str]) -> Workspaces {
    let out = Command::new("bash")
        .arg("scripts/check-side-workspaces.sh")
        .arg("--list")
        .args(extra)
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
        } else if let Some(rest) = line.strip_prefix("[side-workspaces] LOCK ") {
            let rest = rest.trim();
            let Some((workspace, verdict)) = rest.split_once(char::is_whitespace) else {
                continue;
            };
            let verdict = verdict.trim_start();
            // `ours` is one word and `foreign` carries the lister's sentence
            // after it. Anything else is a verdict this reader does not know,
            // and guessing which side of the law it falls on is how a gate
            // reports "compliant" for a state nobody has thought about.
            let ownership = match verdict.split_whitespace().next() {
                Some("ours") => Ownership::Ours,
                Some("foreign") => Ownership::Foreign(
                    verdict
                        .strip_prefix("foreign")
                        .unwrap_or_default()
                        .trim_start_matches([' ', '\u{2014}', '-'])
                        .trim()
                        .to_string(),
                ),
                _ => continue,
            };
            listed.ownership.insert(workspace.to_string(), ownership);
        } else if let Some(rest) = line.strip_prefix("[side-workspaces] COMMAND ") {
            let mut words = words_of(rest);
            if words.len() < 3 {
                continue;
            }
            let workspace = words.remove(0);
            let role = words.remove(0);
            listed.commands.push(DeclaredCommand {
                workspace,
                role,
                words,
            });
        }
    }
    listed.askable.sort();
    listed
}

/// Every command the lister declares, as [`CargoCommand`]s a gate can read.
pub fn lister_declared_commands(listed: &Workspaces) -> Vec<CargoCommand> {
    listed
        .commands
        .iter()
        .map(|declared| {
            // THE SAME READING THE SHELL WALK USES, so a wrapper the lister
            // prints is read the way a wrapper a workflow writes is. A declared
            // command this reader cannot resolve to cargo is kept AS WRITTEN
            // rather than dropped: the gate that judges it then answers
            // `Unreadable`, which is a verdict, and a command missing from the
            // population is not one.
            let (carrier, cargo_args, harness_args) = match cargo_invocation(&declared.words) {
                Some(found) => (found.carrier, found.cargo_args, found.harness_args),
                None => {
                    let (cargo_args, harness_args) = split_at_the_bare_marker(&declared.words);
                    (Vec::new(), cargo_args, harness_args)
                }
            };
            CargoCommand {
                source: "scripts/check-side-workspaces.sh".to_string(),
                owner: declared.workspace.clone(),
                carrier,
                cargo_args,
                harness_args,
                env: BTreeMap::new(),
                declared: None,
                uncounted: 0,
            }
        })
        .collect()
}

/// The subset of those that RUN A SUITE, which is what a gate asking "which
/// tests does CI execute" wants. Filtered from the one list rather than read
/// from a second line in the lister's output, so the two cannot disagree about
/// what the suite command is.
pub fn lister_suite_commands(listed: &Workspaces) -> Vec<CargoCommand> {
    listed
        .commands
        .iter()
        .zip(lister_declared_commands(listed))
        .filter(|(declared, _)| declared.role == "suite")
        .map(|(_, command)| command)
        .collect()
}
