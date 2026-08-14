//! The law: every key this repository declares in `.claude/remote-build.toml`
//! is one the program that reads it reports reading, with the value the
//! declaration gives.
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
        }
    }
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

    Ok(Report {
        declaration,
        has_declaration: true,
        top_level,
        tabled,
        program_keys,
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
