//! The law: every package this repository's CI installs is one its
//! build-machine declaration names.
//!
//! # Why CI is the witness
//!
//! "Everything the far side needs" has no answer without a baseline. A build
//! machine also needs a shell, a linker and a C compiler, and none of those
//! belong in a per-repository declaration. CI is the one far side this
//! repository specifies COMPLETELY: it starts from a stock runner image and
//! every departure from it is written out as a step. So what CI installs is
//! exactly the set this repository has already discovered a bare machine lacks
//! — and the declaration is where that set is supposed to live.
//!
//! # Generous detection, strict reading
//!
//! The two halves are deliberately asymmetric. Anything shaped like an install
//! is DETECTED, including forms this cannot read; only the forms it can read in
//! full are JUDGED; and a detected form it cannot read is a refusal rather than
//! a skip. A gate that skipped what it could not parse would answer "nothing
//! installed" for `env DEBIAN_FRONTEND=noninteractive apt-get install -y x`,
//! and zero findings is what a clean tree looks like.
//!
//! # What it cannot see, said rather than hidden
//!
//! A tool the runner image ships and a bare host does not is invisible to this
//! witness: no step installs it, so nothing here can know it is required.
//! `libclang-common-18-dev` is exactly that — declared, needed by a bare Ubuntu
//! host, installed by no job. [`Report::never_installed`] names those, because a
//! hole somebody can read is one somebody can close.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Where a repository's build-machine declaration lives, relative to its root.
pub const DECLARATION: &str = ".claude/remote-build.toml";

/// The keys of that declaration which NAME something the far side must be given.
///
/// BOTH, and not `packages` alone. The two answer different questions — `needs`
/// is "can it run" and `packages` is "what do I install" — but the law here is
/// that the requirement is NAMED, not that it is named under a particular key.
/// Demanding one key would force a second spelling of a name that happens to be
/// the same word in both vocabularies, and this repository's whole subject is
/// what a second spelling costs. Which key matched is reported.
pub const REQUIREMENT_KEYS: &[&str] = &["needs", "packages"];

/// Package managers whose install command names its packages in words this
/// reader understands in full.
pub const READ_IN_FULL: &[&str] = &["apt-get", "apt", "aptitude"];

/// Managers recognised as installing something whose package names this does not
/// read.
///
/// They are recognised so they can be COUNTED AND NAMED rather than skipped: a
/// step this reader does not understand is a hole in the law's population, and
/// the report is where a hole belongs. Judging them would mean joining a
/// language's package names against a declaration written in apt's, which is a
/// table, and a table is the thing that goes stale silently.
pub const RECOGNISED_NOT_READ: &[&str] = &[
    "apk", "brew", "cargo", "choco", "dnf", "gem", "go", "npm", "pacman", "pip", "pip3", "rustup",
    "snap", "winget", "yum", "zypper",
];

/// Words in an action's name that say it installs something onto the runner.
///
/// A WEAK READING, and it is the strongest one available: an action is somebody
/// else's program and this repository holds none of its bytes. What the name
/// says is all there is, so the reading is generous — anything matching refuses
/// rather than passes — and an action whose name says nothing about installing
/// is a limit recorded in [`Report`] rather than a claim made here.
pub const INSTALLING_ACTION_WORDS: &[&str] = &["setup", "install", "toolchain", "apt"];

/// Flags an apt-family install takes that consume no word of their own.
///
/// ANY OTHER FLAG IS A REFUSAL. `-t bookworm-backports` takes the next word, and
/// a reader that did not know it would call that word a package — a name that is
/// then reported as undeclared, sending somebody to add a release codename to a
/// list of packages. Guessing which flags take an argument is how a reader
/// starts answering about things nobody installed.
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

/// Where a step is, in the words its own file uses.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Site {
    /// The workflow file.
    pub source: String,
    /// The job id — the spelling `needs:` uses.
    pub job: String,
    /// Where the step sits in its job's `steps:` list, counting every step.
    pub index: usize,
}

impl std::fmt::Display for Site {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} job `{}` step {}",
            self.source, self.job, self.index
        )
    }
}

/// One install command this reader understood in full.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Install {
    pub site: Site,
    /// The manager, as the command spells it.
    pub manager: String,
    /// The packages it installs, in the order written.
    pub packages: Vec<String>,
}

/// One install this reader recognised and did not read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unread {
    pub site: Site,
    /// What was recognised — a manager's name, or the action's.
    pub what: String,
    /// The command or `uses:` value, for the person reading the report.
    pub written: String,
}

/// A package CI installs that the declaration does not name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub package: String,
    /// Every place it is installed, so a repair sees the whole of it.
    pub sites: Vec<Site>,
}

impl std::fmt::Display for Finding {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "CI installs `{}` and the build-machine declaration names it nowhere ({})",
            self.package,
            self.sites
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; ")
        )
    }
}

/// The whole answer, including what was not judged.
#[derive(Debug, Clone)]
pub struct Report {
    pub declaration: PathBuf,
    /// `false` when the repository declares nothing at all — which is not a
    /// clean check, and here not even a complete answer: a repository whose CI
    /// installs something and which declares nothing is the hiding case at its
    /// widest.
    pub has_declaration: bool,
    /// Every name the declaration gives, mapped to the keys it appears under.
    pub declared: BTreeMap<String, Vec<String>>,
    /// The judged population.
    pub installs: Vec<Install>,
    /// Recognised, not judged, and named for it.
    pub unread: Vec<Unread>,
    /// Names the declaration gives that no job installs. NOT a finding: the
    /// runner image ships things a bare host does not, and this witness cannot
    /// tell that from a stale entry.
    pub never_installed: Vec<String>,
    pub findings: Vec<Finding>,
}

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

/// What one command of a `run:` step is, to this law.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
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
pub fn read_command(words: &[String]) -> Command {
    let body = without_privilege(words);
    let Some(head) = body.first().map(String::as_str) else {
        return Command::Nothing;
    };
    let apt_at = body
        .iter()
        .position(|word| READ_IN_FULL.contains(&word.as_str()));
    if let Some(at) = apt_at {
        // `apt-get update` is not an install, and neither is a bare `apt`.
        if !body.iter().any(|word| word == "install") {
            return Command::Nothing;
        }
        if at != 0 {
            return Command::Refused {
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
        return Command::Recognised {
            what: head.to_owned(),
        };
    }
    Command::Nothing
}

/// Read an apt-family install whose head is the manager.
fn read_apt(body: &[String]) -> Command {
    let manager = body[0].clone();
    let at = body
        .iter()
        .position(|word| word == "install")
        .expect("the caller established that `install` is one of these words");
    // Everything between the manager and the subcommand must be a flag, or the
    // command is doing something this reading does not describe.
    for word in &body[1..at] {
        if !word.starts_with('-') {
            return Command::Refused {
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
            return Command::Refused {
                why: format!(
                    "`{word}` is a flag this reader does not know, and a flag that takes the \
                     next word would make that word look like a package"
                ),
            };
        }
        if word.contains(NOT_A_NAME) {
            return Command::Refused {
                why: format!(
                    "`{word}` is not a package name a file can be held against — the shell \
                     decides what it is at run time"
                ),
            };
        }
        packages.push(word.clone());
    }
    if packages.is_empty() {
        return Command::Refused {
            why: format!("`{manager} install` with no package named is a form this cannot judge"),
        };
    }
    Command::Read { manager, packages }
}

/// Whether an action's name says it installs something onto the runner.
#[must_use]
pub fn action_installs(action: &str) -> bool {
    let name = action.to_ascii_lowercase();
    INSTALLING_ACTION_WORDS
        .iter()
        .any(|word| name.contains(word))
}

/// The names one declaration gives, by the key each appears under.
///
/// # Errors
///
/// When the file does not parse, or a requirement key holds anything but a list
/// of strings. Both make the law's other side unknown, and an unknown side is a
/// refusal rather than an empty one: an empty list of declared names turns every
/// install into a finding, which is a loud wrong answer.
pub fn read_declaration(text: &str) -> Result<BTreeMap<String, Vec<String>>, String> {
    let parsed: toml::Table = text.parse().map_err(|error| format!("{error}"))?;
    let mut declared: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for key in REQUIREMENT_KEYS {
        let Some(value) = parsed.get(*key) else {
            continue;
        };
        let Some(items) = value.as_array() else {
            return Err(format!("`{key}` is declared and is not a list"));
        };
        for item in items {
            let Some(name) = item.as_str() else {
                return Err(format!("`{key}` holds an entry that is not a string"));
            };
            declared
                .entry(name.to_owned())
                .or_default()
                .push((*key).to_owned());
        }
    }
    Ok(declared)
}

/// Which of the installed packages the declaration does not name.
#[must_use]
pub fn judge(installs: &[Install], declared: &BTreeMap<String, Vec<String>>) -> Vec<Finding> {
    // ONE FINDING PER PACKAGE AND NOT PER STEP. Six jobs install `protobuf-compiler`
    // here; six findings for one missing line would report the size of the
    // workflow rather than the size of the defect.
    let mut sites: BTreeMap<String, Vec<Site>> = BTreeMap::new();
    for install in installs {
        for package in &install.packages {
            if declared.contains_key(package) {
                continue;
            }
            sites
                .entry(package.clone())
                .or_default()
                .push(install.site.clone());
        }
    }
    sites
        .into_iter()
        .map(|(package, sites)| Finding { package, sites })
        .collect()
}

/// The workflow files, or a refusal in place of the shared loader's panic.
///
/// [`ci_plan::workflow_files`] asserts rather than returns: a repository that
/// tracks no workflow is an empty population, and it is the one loader four
/// other gates share. This law's contract is three exit codes, so the refusal is
/// CONVERTED rather than dropped — an assertion reaching the process as code 101
/// would be read by the hook as "the gate crashed" instead of "the gate could
/// not be asked", and only one of those two is true.
fn workflows(root: &Path) -> Result<Vec<String>, String> {
    std::panic::catch_unwind(|| ci_plan::workflow_files(root)).map_err(|payload| {
        let said = payload
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| payload.downcast_ref::<&str>().copied())
            .unwrap_or("the loader refused without a message");
        format!("the workflow population could not be read — {said}")
    })
}

/// Run the law over one repository.
///
/// # Errors
///
/// Only for the things that make a VERDICT impossible: a population that could
/// not be read, an install-shaped step this cannot read, a population of zero
/// installs (there is then nothing to hold the declaration against), and a
/// declaration whose requirement keys cannot be read. A requirement CI installs
/// and the declaration omits comes back as a finding.
pub fn run(root: &Path) -> Result<Report, String> {
    let mut installs = Vec::new();
    let mut unread = Vec::new();
    let mut refusals = Vec::new();
    for file in workflows(root)? {
        let doc = ci_plan::load_workflow(root, &file);
        for step in ci_plan::run_steps(&doc) {
            let site = Site {
                source: file.clone(),
                job: step.job.clone(),
                index: step.index,
            };
            for words in ci_plan::shell_commands(&step.script) {
                match read_command(&words) {
                    Command::Nothing => {}
                    Command::Read { manager, packages } => installs.push(Install {
                        site: site.clone(),
                        manager,
                        packages,
                    }),
                    Command::Recognised { what } => unread.push(Unread {
                        site: site.clone(),
                        what,
                        written: words.join(" "),
                    }),
                    Command::Refused { why } => {
                        refusals.push(format!("{site}: {why}"));
                    }
                }
            }
        }
        for step in ci_plan::uses_steps(&doc, &file) {
            if !action_installs(step.action()) {
                continue;
            }
            let site = Site {
                source: file.clone(),
                job: step.job.clone(),
                index: step.index,
            };
            // A REFUSAL AND NOT A NOTE. An action that installs a tool is the
            // one way to put a requirement on the far side that this law cannot
            // read at all, so treating it as merely unread would leave the
            // bypass open with a sentence printed under it.
            refusals.push(format!(
                "{site}: `{}` installs something onto the runner and this law reads package \
                 names out of shell, so what it installs cannot be held against the declaration",
                step.uses
            ));
        }
    }
    if !refusals.is_empty() {
        return Err(format!(
            "{} install-shaped step(s) could not be read:\n  {}",
            refusals.len(),
            refusals.join("\n  ")
        ));
    }
    // A POPULATION OF ZERO IS TWO DIFFERENT ANSWERS, and only one of them is a
    // verdict. Workflows that install NOTHING are a reading: the witness looked
    // and the runner needed nothing beyond its image. Workflows whose only
    // installs are ones this law does not read are an absence wearing that
    // reading's clothes — there is something a bare machine lacks and this
    // cannot say what. The first is [`Report`] with no installs; the second is a
    // refusal.
    if installs.is_empty() && !unread.is_empty() {
        return Err(format!(
            "no step in any tracked workflow installs a package this can read, and {} step(s) \
             DO install through something it does not read — so the declaration was held \
             against nothing, which is not the same as a clean check",
            unread.len()
        ));
    }

    let declaration = root.join(DECLARATION);
    let has_declaration = declaration.is_file();
    let declared = if has_declaration {
        let text = std::fs::read_to_string(&declaration)
            .map_err(|error| format!("could not read {} — {error}", declaration.display()))?;
        read_declaration(&text).map_err(|why| {
            format!(
                "{} could not be read for what it requires — {why}. What that file MEANS is \
                 this law's other side; that it is also read by a machine-wide program with \
                 line patterns is `tools/unread-declaration`'s law, which calls this a defect \
                 rather than a refusal",
                declaration.display()
            )
        })?
    } else {
        BTreeMap::new()
    };

    let installed: BTreeSet<&String> = installs
        .iter()
        .flat_map(|install| install.packages.iter())
        .collect();
    let never_installed = declared
        .keys()
        .filter(|name| !installed.contains(name))
        .cloned()
        .collect();
    let findings = judge(&installs, &declared);
    Ok(Report {
        declaration,
        has_declaration,
        declared,
        installs,
        unread,
        never_installed,
        findings,
    })
}
