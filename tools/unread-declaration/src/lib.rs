//! The law: every key this repository declares in `.claude/remote-build.toml`
//! is one the program that reads it reports reading, with the value the
//! declaration gives — and every token of that value names something here.
//!
//! # Two halves, and the second one was open for a round
//!
//! The KEY half asks the program. The VALUE half cannot: the program reads
//! `writes` as a list of substrings to look for in a command, and whether such a
//! substring names anything is a question about THIS REPOSITORY, which the
//! program has never seen. So nothing asked it, and the cost was measured:
//! `writes = [".sweep.json"]` was read exactly as declared — the key half was
//! clean — while matching four of the twenty-three sweep manifests this
//! repository tracks. The other nineteen ran on a build machine and the tracked
//! evidence they write stayed there. The same key had already done it once, one
//! key over, as `exclude`, which five repositories declared and nothing read.
//!
//! EXISTENCE IS NOT ENOUGH, WHICH IS THE WHOLE LESSON. `.sweep.json` named
//! something: four files. A law that asked only "does this token match a tracked
//! path" would have passed it. So the value half asks BOTH directions — every
//! token reaches something, and every sweep is reached by some token — and prints
//! the counts, because the difference between four and twenty-three is the defect
//! and only a number says it.
//!
//! # Why the program is asked instead of consulted
//!
//! The declaration is TOML by appearance and is read by line patterns. Those are
//! different languages, and the difference is invisible from either side: a
//! value that moves into a table, gains a decimal point or changes its quoting
//! is still valid TOML and no longer matches. So the comparison here is between
//! two readings of the same bytes — the strict one (`toml`) and the program's
//! own, obtained by asking it — and a gate that carried its own copy of the
//! program's schema would agree with the file while the program disagreed with
//! both.
//!
//! # What this cannot see, said rather than hidden
//!
//! A key inside a table is outside the program's namespace: it extracts
//! top-level keys only, and `[commands]` is read by the skill that drives the
//! program rather than by the program. Those keys are COUNTED AND NAMED in the
//! report and are not judged. A count that is printed is a hole somebody can
//! close; a key silently skipped is a hole that reads as a clean check.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Where a repository's declaration lives, relative to its root.
pub const DECLARATION: &str = ".claude/remote-build.toml";

/// The default program: machine-wide, outside every checkout, which is the
/// asymmetry this gate exists for.
pub const PROGRAM_UNDER_HOME: &str = ".claude/remote-build/bin/bx";

/// The one declared key whose VALUE names things in THIS repository.
///
/// Every other key names something about a MACHINE — `needs` and `packages` are
/// programs and installers on the far side, `min_free_gb` and `peak_gb_per_task`
/// are numbers, `send` is a mode — and nothing in this checkout can say whether
/// those are right. `writes` is different: the declaration's own comment says why,
/// and it is the reason the value half of this law exists at all. The program's
/// built-in vocabulary knows TOOLS (git, cargo and the mutate API are writes
/// whoever runs them); what it cannot know is this repository's own scripts and
/// data, so every token here is a PATH — a substring the program looks for in a
/// command, put there by a file this repository tracks.
pub const WRITES: &str = "writes";

/// One key in the program's namespace, rendered the way the program renders
/// values.
///
/// ONLY TOP-LEVEL KEYS BECOME ONE OF THESE. A key inside a table is not judged,
/// so it needs no rendering — and giving it one meant calling [`render`] on a
/// value nothing compares, which the first draft did with the error dropped into
/// a default. An `Err` nobody reads is a silent fail whether or not the value
/// matters; the shape that cannot have one is not computing it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declared {
    /// The bare key name, as the program would have to spell it.
    pub key: String,
    /// What the declaration MEANS, in the program's rendering (see [`render`]).
    pub rendering: String,
    /// The value's ITEMS, unjoined — one for a scalar, one per element for an
    /// array.
    ///
    /// KEPT BESIDE THE RENDERING RATHER THAN RECOVERED FROM IT, because they
    /// answer different questions and only one of them survives the join. The
    /// rendering is what the PROGRAM compares; the items are what the value
    /// NAMES, and a token containing a space cannot be split back out of the
    /// rendering — so a law that recovered them would silently judge two tokens
    /// where the file wrote one.
    pub items: Vec<String>,
}

/// A key the declaration states that the program does not read as declared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Finding {
    /// The program has no extractor for this key at all: it is declared into
    /// nothing.
    Unread { key: String },
    /// The program has an extractor and read something else — the shape that
    /// costs the most, because the value is neither absent nor what was written.
    Disagrees {
        key: String,
        declared: String,
        read: String,
    },
    /// The file claims to be TOML and is not. The program's line patterns will
    /// still read parts of it, which is worse than a clean failure.
    NotTheLanguageItClaims { message: String },
    /// A token in a declared value matches nothing this repository tracks, so the
    /// program looks for a substring no command of this repository can contain.
    /// The declaration reads as a requirement and imposes none — which is the
    /// `exclude` key's shape, one key over, deleted in 2026-08-14 after five
    /// repositories had declared it and nothing had ever read it.
    NamesNothingHere { key: String, token: String },
    /// A sweep this repository tracks is matched by no token, so running it is a
    /// command the program will send to a build machine — where it edits a
    /// throwaway copy and leaves the tracked evidence it writes behind.
    ///
    /// THIS IS THE HALF WITH TEETH. Existence alone accepted `.sweep.json`, which
    /// matched four of twenty-three manifests: the token named something, and
    /// nineteen sweeps ran elsewhere anyway.
    EvidenceWouldLandElsewhere { manifest: String },
}

impl std::fmt::Display for Finding {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unread { key } => {
                write!(
                    formatter,
                    "`{key}` is declared and the program never reads it"
                )
            }
            Self::Disagrees {
                key,
                declared,
                read,
            } => write!(
                formatter,
                "`{key}` is declared as `{declared}` and the program reads `{read}`"
            ),
            Self::NotTheLanguageItClaims { message } => {
                write!(formatter, "the declaration is not valid TOML: {message}")
            }
            Self::NamesNothingHere { key, token } => write!(
                formatter,
                "`{key}` declares `{token}` and no file this repository tracks \
                 carries that text, so the program looks for a substring no \
                 command here can contain"
            ),
            Self::EvidenceWouldLandElsewhere { manifest } => write!(
                formatter,
                "`{manifest}` is a sweep this repository tracks and no `{WRITES}` \
                 token names it, so running it goes to a build machine and the \
                 evidence it writes stays there"
            ),
        }
    }
}

/// One token of a declared value, and how much of this repository it reaches.
///
/// THE COUNT IS PRINTED EVERY RUN. A token matching one path and a token matching
/// twenty-three are both "matches something", and the difference between them is
/// the whole of the defect this half was written after: `.sweep.json` reached four
/// of twenty-three sweeps and nothing said so, because nothing was counting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenReach {
    pub key: String,
    pub token: String,
    /// Tracked paths whose text contains the token.
    pub paths: usize,
    /// Tracked sweeps whose path contains the token — the subset with a law over
    /// it, as against the paths above, which only existence can be asked of.
    pub sweeps: usize,
}

/// What the program says it read out of a declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramReport {
    /// The file the program looked at — compared against the one this gate read,
    /// because two programs standing in different directories answer about
    /// different files and agree by accident.
    pub declaration: PathBuf,
    /// Whether the program found that file.
    pub present: bool,
    /// Every key the program has an extractor for, mapped to what it extracted.
    /// Keys that found nothing are present with an empty value ON PURPOSE: a
    /// report of only what was found cannot distinguish "the program does not
    /// know that key" from "it knows it and the spelling did not match".
    pub extracts: BTreeMap<String, String>,
}

/// The whole answer, including what was not asked.
#[derive(Debug, Clone)]
pub struct Report {
    pub declaration: PathBuf,
    /// `false` when the repository declares nothing — a complete answer, not a
    /// refusal, and not the same thing as a clean check.
    pub has_declaration: bool,
    /// Keys in the program's namespace, which are the ones the law applies to.
    pub top_level: Vec<Declared>,
    /// Keys inside a table, by their dotted name: counted, named, and not
    /// judged, because the program's namespace is the top level only.
    pub tabled: Vec<String>,
    /// Every key the program can read at all.
    pub program_keys: Vec<String>,
    /// What each token of the value half reaches here, counted rather than
    /// merely found. Empty when the repository declares no [`WRITES`] key.
    pub writes_reach: Vec<TokenReach>,
    /// How many sweeps this repository tracks — the population the value half is
    /// judged against, printed so that a law over an empty set says so.
    pub sweeps_tracked: usize,
    pub findings: Vec<Finding>,
}

/// Render a declared value the way the program renders one.
///
/// The program's list rendering strips quotes and joins on a space, so an array
/// means the space-joined text of its items. Anything this cannot render is an
/// error rather than a guess: a value nobody can compare is a value the law must
/// refuse to have an opinion about.
fn render(value: &toml::Value) -> Result<String, String> {
    Ok(match value {
        toml::Value::String(text) => text.clone(),
        toml::Value::Integer(number) => number.to_string(),
        toml::Value::Float(number) => number.to_string(),
        toml::Value::Boolean(flag) => flag.to_string(),
        toml::Value::Datetime(stamp) => stamp.to_string(),
        toml::Value::Array(items) => {
            let mut rendered = Vec::with_capacity(items.len());
            for item in items {
                if matches!(item, toml::Value::Array(_) | toml::Value::Table(_)) {
                    return Err("a nested array or table has no single rendering".to_owned());
                }
                rendered.push(render(item)?);
            }
            rendered.join(" ")
        }
        toml::Value::Table(_) => return Err("a table has no single rendering".to_owned()),
    })
}

/// The items a declared value holds, unjoined: one for a scalar, one per element
/// for an array.
///
/// Uses [`render`] per item, so what a token IS here is exactly what the program
/// would look for — the two cannot drift, because one calls the other.
fn items_of(value: &toml::Value) -> Result<Vec<String>, String> {
    match value {
        toml::Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(render(item)?);
            }
            Ok(out)
        }
        scalar => Ok(vec![render(scalar)?]),
    }
}

/// Every path this repository tracks, from `git ls-files`.
///
/// FROM GIT AND NOT FROM A WALK, the reason the harness gives for its own
/// listing: an untracked stray is not something another checkout has, so a token
/// that names one names nothing anybody else can run — and a walk would call that
/// declaration satisfied.
fn tracked_paths(root: &Path) -> Result<Vec<String>, String> {
    let out = std::process::Command::new("git")
        .arg("ls-files")
        .current_dir(root)
        .output()
        .map_err(|error| format!("git ls-files in {} — {error}", root.display()))?;
    if !out.status.success() {
        return Err(format!(
            "git ls-files in {} failed — {}",
            root.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_owned)
        .collect())
}

/// Judge the VALUE half of the law: every token of [`WRITES`] names something
/// this repository tracks, and every sweep it tracks is named by one of them.
///
/// Returns what each token reaches, how many sweeps the population held, and the
/// findings.
///
/// # Errors
///
/// Only when a POPULATION cannot be obtained — git unavailable, a manifest
/// listing that fails. That is a refusal and not a finding: a law that answers
/// "nothing wrong" because it could not look is the shape this repository keeps
/// paying for.
pub fn judge_values(
    root: &Path,
    top_level: &[Declared],
) -> Result<(Vec<TokenReach>, usize, Vec<Finding>), String> {
    let sweeps: Vec<String> = injection_harness::tracked_sweeps(root)
        .map_err(|why| format!("which sweeps this repository tracks could not be asked — {why}"))?
        .into_iter()
        .map(|(path, _)| path)
        .collect();
    let Some(declared) = top_level.iter().find(|entry| entry.key == WRITES) else {
        // NOT A CLEAN ANSWER AND NOT A REFUSAL EITHER: a repository that declares
        // no `writes` key has no value half to judge, and the count of sweeps is
        // still reported so that a tree with sweeps and no declaration is loud.
        return Ok((Vec::new(), sweeps.len(), Vec::new()));
    };
    let paths = tracked_paths(root)?;

    let mut reach = Vec::with_capacity(declared.items.len());
    let mut findings = Vec::new();
    for token in &declared.items {
        let matched_paths = paths.iter().filter(|path| path.contains(token)).count();
        let matched_sweeps = sweeps.iter().filter(|path| path.contains(token)).count();
        if matched_paths == 0 {
            findings.push(Finding::NamesNothingHere {
                key: declared.key.clone(),
                token: token.clone(),
            });
        }
        reach.push(TokenReach {
            key: declared.key.clone(),
            token: token.clone(),
            paths: matched_paths,
            sweeps: matched_sweeps,
        });
    }

    for manifest in &sweeps {
        if !declared
            .items
            .iter()
            .any(|token| manifest.contains(token.as_str()))
        {
            findings.push(Finding::EvidenceWouldLandElsewhere {
                manifest: manifest.clone(),
            });
        }
    }

    Ok((reach, sweeps.len(), findings))
}

/// Split a declaration into the keys the program could read and the keys inside
/// tables, which it cannot.
///
/// # Errors
///
/// When the file is not valid TOML, or a TOP-LEVEL key holds a value with no
/// single rendering. A value inside a table is never rendered, so it can never
/// raise this.
pub fn read_declaration(text: &str) -> Result<(Vec<Declared>, Vec<String>), String> {
    let parsed: toml::Table = text.parse().map_err(|error| format!("{error}"))?;
    let mut top_level = Vec::new();
    let mut tabled = Vec::new();
    for (key, value) in &parsed {
        match value {
            toml::Value::Table(inner) => {
                for inner_key in inner.keys() {
                    tabled.push(format!("{key}.{inner_key}"));
                }
            }
            other => top_level.push(Declared {
                key: key.clone(),
                rendering: render(other)
                    .map_err(|why| format!("`{key}` cannot be compared — {why}"))?,
                items: items_of(other)
                    .map_err(|why| format!("`{key}` cannot be compared — {why}"))?,
            }),
        }
    }
    Ok((top_level, tabled))
}

/// Parse what the program printed.
///
/// # Errors
///
/// When the output carries no `decl-file` line — which is what a program without
/// the seam looks like, and is a refusal rather than an empty answer.
pub fn read_program_answer(stdout: &str) -> Result<ProgramReport, String> {
    let mut declaration: Option<PathBuf> = None;
    let mut present = false;
    let mut extracts = BTreeMap::new();
    for line in stdout.lines() {
        let mut fields = line.splitn(3, '\t');
        match (fields.next(), fields.next(), fields.next()) {
            (Some("decl-file"), Some(path), Some(state)) => {
                declaration = Some(PathBuf::from(path));
                present = state == "present";
            }
            (Some("decl"), Some(key), Some(value)) => {
                extracts.insert(key.to_owned(), value.to_owned());
            }
            _ => {}
        }
    }
    let Some(declaration) = declaration else {
        return Err(
            "it printed no `decl-file` line — this program does not have \
             `--explain-declaration`, so what it reads cannot be asked"
                .to_owned(),
        );
    };
    if extracts.is_empty() {
        return Err("it named no keys at all, so there is nothing to hold the file against".into());
    }
    Ok(ProgramReport {
        declaration,
        present,
        extracts,
    })
}

/// Ask the program what it read, standing in `directory`.
///
/// # Errors
///
/// When the program cannot be run, exits non-zero, or answers without the seam.
pub fn ask(program: &Path, directory: &Path) -> Result<ProgramReport, String> {
    let output = std::process::Command::new(program)
        .arg("--explain-declaration")
        .current_dir(directory)
        .output()
        .map_err(|error| format!("could not run `{}` — {error}", program.display()))?;
    if !output.status.success() {
        return Err(format!(
            "`{} --explain-declaration` exited {} — {}",
            program.display(),
            output
                .status
                .code()
                .map_or_else(|| "on a signal".to_owned(), |code| code.to_string()),
            String::from_utf8_lossy(&output.stderr).trim(),
        ));
    }
    read_program_answer(&String::from_utf8_lossy(&output.stdout))
}

/// Run the law over one repository.
///
/// # Errors
///
/// Only for the things that make a VERDICT impossible: a program that cannot be
/// run or lacks the seam, and an answer about a different file than the one read
/// here. A defect in the declaration itself comes back as a finding.
pub fn run(repository: &Path, program: &Path) -> Result<Report, String> {
    let declaration = repository.join(DECLARATION);
    if !declaration.is_file() {
        return Ok(Report {
            declaration,
            has_declaration: false,
            top_level: Vec::new(),
            tabled: Vec::new(),
            program_keys: Vec::new(),
            writes_reach: Vec::new(),
            sweeps_tracked: 0,
            findings: Vec::new(),
        });
    }
    let text = std::fs::read_to_string(&declaration)
        .map_err(|error| format!("could not read {} — {error}", declaration.display()))?;

    let answer = ask(program, repository)?;
    // THE PROGRAM MUST BE ANSWERING ABOUT THIS FILE. It finds the declaration
    // from where it was started, so a gate that ran it elsewhere would compare
    // one repository's keys against another's values and agree or disagree for
    // no reason either file could explain.
    let same_file = match (
        answer.declaration.canonicalize(),
        declaration.canonicalize(),
    ) {
        (Ok(theirs), Ok(ours)) => theirs == ours,
        _ => answer.declaration == declaration,
    };
    if !same_file {
        return Err(format!(
            "it answered about {} and the file read here is {}",
            answer.declaration.display(),
            declaration.display()
        ));
    }
    if !answer.present {
        return Err(format!(
            "it reports {} absent and this gate just read it",
            answer.declaration.display()
        ));
    }

    let program_keys: Vec<String> = answer.extracts.keys().cloned().collect();
    let (top_level, tabled) = match read_declaration(&text) {
        Ok(split) => split,
        Err(message) => {
            return Ok(Report {
                declaration,
                has_declaration: true,
                top_level: Vec::new(),
                tabled: Vec::new(),
                program_keys,
                writes_reach: Vec::new(),
                sweeps_tracked: 0,
                findings: vec![Finding::NotTheLanguageItClaims { message }],
            })
        }
    };

    let mut findings = Vec::new();
    for declared in &top_level {
        match answer.extracts.get(&declared.key) {
            None => findings.push(Finding::Unread {
                key: declared.key.clone(),
            }),
            Some(read) if read != &declared.rendering => findings.push(Finding::Disagrees {
                key: declared.key.clone(),
                declared: declared.rendering.clone(),
                read: read.clone(),
            }),
            Some(_) => {}
        }
    }

    // THE VALUE HALF, AFTER THE KEY HALF AND NOT INSTEAD OF IT. A key the program
    // never reads is a requirement that imposes nothing; a key it reads whose
    // VALUE names nothing here is a requirement that imposes something on no file.
    // The second was measured on this very key: `writes = [".sweep.json"]` was
    // read exactly as declared — the key half was clean — and reached four of the
    // twenty-three sweeps it was written to cover.
    let (writes_reach, sweeps_tracked, value_findings) = judge_values(repository, &top_level)?;
    findings.extend(value_findings);

    Ok(Report {
        declaration,
        has_declaration: true,
        top_level,
        tabled,
        program_keys,
        writes_reach,
        sweeps_tracked,
        findings,
    })
}

/// The default program's path, from the machine rather than from a checkout.
///
/// # Errors
///
/// When `HOME` is unset, which is the one case where guessing would point the
/// gate at some other user's tool.
pub fn program_under_home() -> Result<PathBuf, String> {
    let home = std::env::var("HOME")
        .map_err(|_| "HOME is not set, so the machine-wide program cannot be located".to_owned())?;
    Ok(PathBuf::from(home).join(PROGRAM_UNDER_HOME))
}
