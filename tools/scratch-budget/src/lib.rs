//! A record this repository writes under `target/` is one something collects.
//!
//! `scripts/gc` is this repository's answer to "reclaim `target/`", and it
//! delegates the whole question to cargo-sweep, which knows what an ARTIFACT is
//! and nothing else. Every record written into that tree was therefore outside
//! any collector: measured 2026-08-14, `cargo sweep --installed --dry-run`
//! answered `Would clean: nothing` on a tree holding 1537 record files and
//! 86 MB across three directories, the oldest of them sixteen days old.
//!
//! THE POLICY IS A BYTE BUDGET, EVICTED OLDEST-FIRST, and the two halves of
//! that need separate defences.
//!
//! A BUDGET, because disk is the harm. This machine's gates fail in the shape
//! of defects when the disk fills — a crate that will not compile, a gate that
//! answers "cannot judge", a citation check reporting findings nobody wrote —
//! and every one of those has been diagnosed here as a full disk wearing a
//! defect's clothes. Bounding bytes bounds that, and it needs no guess about
//! how many days of history somebody will want.
//!
//! OLDEST-FIRST, and this is where the gc's own header has to be answered
//! rather than obeyed. It refuses to sweep by age, because an age window
//! "keeps stale artifacts that happen to be recent and deletes live ones that
//! happen not to be". That is a fact about ARTIFACTS, whose relevance is
//! decided by whether a build needs them — a two-month-old rlib can be exactly
//! what the next `cargo build` links. It is not a fact about RECORDS. Nothing
//! reads a log to build with; a log is worth keeping only while somebody may
//! still want to look at it, and that decays with time and with nothing else.
//! The key that is a guess for one is the right key for the other.
//!
//! AND THE NEWEST FILE IS NEVER REMOVED, so a budget smaller than one record
//! cannot delete the record someone is reading right now. That case is
//! REPORTED rather than obeyed in silence — a collector that cannot meet its
//! budget and says nothing is indistinguishable from one that met it.

use std::cmp::Ordering;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

use serde::Deserialize;

/// The declaration's filename, beside this crate's manifest.
pub const DECLARATION: &str = "scratch.json";

/// Where this program's own declaration lives.
///
/// FROM THE COMPILED-IN CHECKOUT AND NOT FROM THE TREE UNDER COLLECTION. This
/// program is run against a working directory that is frequently NOT its own
/// repository — `scripts/verify.sh` wraps commands in other trees, and this
/// repository's git hooks run over other checkouts — and in every one of those
/// cases the policy is this repository's while the files are that tree's. The
/// same two-tree rule every gate here states.
pub fn declaration_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(DECLARATION)
}

/// One directory of records, and how much disk it may hold.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Collected {
    /// Relative to the tree being collected, and required to name something
    /// under its `target/` — see [`resolve`].
    pub path: PathBuf,
    /// The budget, in mebibytes, because that is the unit a person reasoning
    /// about a log directory thinks in. Held as bytes everywhere below.
    pub budget_mib: u64,
    /// What this directory holds, what it grows at, and what the number buys.
    /// Read by people; declared here so a budget cannot arrive without one.
    pub why: String,
}

impl Collected {
    pub fn budget_bytes(&self) -> u64 {
        self.budget_mib * 1024 * 1024
    }
}

/// The whole policy: every directory of records this repository writes.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Declaration {
    /// The file's own header, in the author's words — declared so that a key
    /// nobody meant cannot hide beside it (the manifest reader's rule).
    #[serde(rename = "_", default)]
    pub prose: Vec<String>,
    pub directories: Vec<Collected>,
}

pub fn read_declaration(path: &Path) -> Result<Declaration, String> {
    let raw =
        fs::read_to_string(path).map_err(|e| format!("{} unreadable: {e}", path.display()))?;
    parse_declaration(&raw, &path.display().to_string())
}

/// The text of a declaration, judged.
///
/// SEPARATE FROM THE READ so the law's control can hand this one with a hole in
/// it without writing a file — and so that the emptiness refusal below is
/// exercised by something other than a corrupted checkout.
pub fn parse_declaration(raw: &str, whence: &str) -> Result<Declaration, String> {
    let declaration: Declaration = serde_json::from_str(raw)
        .map_err(|e| format!("{whence} is not a scratch declaration: {e}"))?;
    if declaration.directories.is_empty() {
        return Err(format!(
            "{whence} declares no directory at all, so this collector would run \
             over nothing and report success for it"
        ));
    }
    Ok(declaration)
}

/// A path with `.` and `..` resolved textually, and nothing asked of the disk.
///
/// LEXICAL RATHER THAN `canonicalize`, for the reason the manifest reader gives
/// one directory over: a record directory is created by the program that writes
/// into it, so a reader that refused a path for not existing yet would be a
/// reader about nothing. `std::path::absolute` makes a path absolute and
/// deliberately leaves `..` in place, and every declared log directory in this
/// repository is spelled with one (`../../target/injection-logs`), so both
/// sides of any comparison have to come through here or two spellings of one
/// directory will not match.
pub fn normalise(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                // Only a NAMED component can be undone. Popping past a root, or
                // past a leading `..` in a relative path, would invent a path
                // the caller did not write.
                if out
                    .components()
                    .next_back()
                    .is_some_and(|last| matches!(last, Component::Normal(_)))
                {
                    out.pop();
                } else {
                    out.push(part);
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Where a declared directory lands in a given tree, refusing anything that is
/// not INSIDE a build directory of that tree.
///
/// THE GUARD IS THE WHOLE REASON THIS IS A FUNCTION. Everything below deletes
/// files, and the only thing standing between a mistyped declaration and a
/// source directory is this check. A directory named `target` is the one thing
/// this repository treats as cycled scratch — every one of them is named by a
/// `.gitignore` (the root's own, and one in each separate workspace under
/// `tools/`), and everything in one is rewritten by whatever wrote it. Nothing
/// outside them has that property, so nothing outside them is collectable,
/// whatever a declaration says.
///
/// AT ANY DEPTH, WHICH THE LAW IN `tests/declared.rs` FOUND ON ITS FIRST RUN.
/// This was `<tree>/target` alone, on the assumption that one build directory
/// serves the whole repository — which is true of cargo's ARTIFACTS here
/// (`.cargo/config.toml` points every workspace at the root `target`) and is
/// not true of its RECORDS: three sweeps write their logs beside the workspace
/// they test, `tools/injection-harness/target/self-check-logs` among them, 19
/// files of them on this machine. A guard that refused those would have forced
/// them out of the declaration, which is the state that made them invisible.
///
/// AND NEVER THE BUILD DIRECTORY ITSELF. `"path": "target"` would hand this
/// collector the artifacts of the whole workspace with a budget of a few
/// mebibytes, which is `rm -rf target` written as data. A declared path has to
/// name something INSIDE one.
pub fn resolve(tree: &Path, declared: &Path) -> Result<PathBuf, String> {
    let tree =
        normalise(&std::path::absolute(tree).map_err(|e| format!("{}: {e}", tree.display()))?);
    let joined = if declared.is_absolute() {
        declared.to_path_buf()
    } else {
        tree.join(declared)
    };
    let resolved = normalise(&joined);
    let refuse = |why: &str| {
        Err(format!(
            "{} resolves to {}, which is {why} — a collector deletes files, and \
             the only directories this repository is entitled to delete from are \
             the build directories it rebuilds",
            declared.display(),
            resolved.display()
        ))
    };
    let Ok(inside) = resolved.strip_prefix(&tree) else {
        return refuse(&format!("outside {}", tree.display()));
    };
    let parts: Vec<Component> = inside.components().collect();
    match parts.iter().position(|part| part.as_os_str() == "target") {
        Some(at) if at + 1 < parts.len() => Ok(resolved),
        Some(_) => refuse("a build directory itself and not a directory of records inside one"),
        None => refuse("not inside any build directory"),
    }
}

/// One record file: what it costs and when it was last written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub path: PathBuf,
    pub bytes: u64,
    pub modified: SystemTime,
}

/// What a record directory holds, and what could not be read as a record.
#[derive(Debug, Default)]
pub struct Survey {
    pub entries: Vec<Entry>,
    /// Symbolic links, which are counted and never followed or removed: a link
    /// is a name for a file that may live anywhere, and a collector that
    /// followed one would be deleting outside the directory it was pointed at.
    pub links: usize,
    /// Files whose metadata would not read. Named rather than skipped — an
    /// unreadable file is a byte cost this survey cannot account for, and a
    /// total that quietly omits it is the wrong number in the direction that
    /// reads as "within budget".
    pub unreadable: Vec<String>,
}

impl Survey {
    pub fn bytes(&self) -> u64 {
        self.entries.iter().map(|entry| entry.bytes).sum()
    }
}

/// Every regular file under a directory, recursively.
///
/// RECURSIVE, THOUGH EVERY DIRECTORY THIS COLLECTS IS FLAT TODAY. A collector
/// that read only the top level would answer "within budget" about a directory
/// whose whole growth had moved one level down, and that answer is the one that
/// reads as clean.
///
/// A DIRECTORY THAT DOES NOT EXIST IS NOT AN ERROR: it is a directory nothing
/// has written to yet, which is the state of every one of these on a fresh
/// checkout and on every CI runner.
pub fn survey(directory: &Path) -> Result<Survey, String> {
    let mut found = Survey::default();
    let mut queue = vec![directory.to_path_buf()];
    while let Some(at) = queue.pop() {
        let entries = match fs::read_dir(&at) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(format!("{} unreadable: {e}", at.display())),
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    found.unreadable.push(format!("{}: {e}", at.display()));
                    continue;
                }
            };
            let path = entry.path();
            // `symlink_metadata`, so a link is recognised as one rather than as
            // whatever it points at.
            let meta = match fs::symlink_metadata(&path) {
                Ok(meta) => meta,
                Err(e) => {
                    found.unreadable.push(format!("{}: {e}", path.display()));
                    continue;
                }
            };
            if meta.is_symlink() {
                found.links += 1;
            } else if meta.is_dir() {
                queue.push(path);
            } else {
                match meta.modified() {
                    Ok(modified) => found.entries.push(Entry {
                        path,
                        bytes: meta.len(),
                        modified,
                    }),
                    Err(e) => found.unreadable.push(format!("{}: {e}", path.display())),
                }
            }
        }
    }
    Ok(found)
}

/// What collecting a directory would do, decided and not yet done.
#[derive(Debug, Default)]
pub struct Plan {
    /// Oldest first, which is the order they would be removed in.
    pub remove: Vec<Entry>,
    /// What the directory holds now.
    pub total: u64,
    /// What it would hold afterwards.
    pub kept: u64,
    /// Whether the budget is still exceeded once everything removable is gone —
    /// which happens when the NEWEST record alone is bigger than the budget.
    pub still_over: bool,
}

/// Which records to remove so a directory fits its budget.
///
/// PURE, AND THE COLLECTOR BELOW DRIVES IT. The cases that prove this rule
/// works run the same function the real collection does; a control that
/// re-implemented the ordering would prove only that a second spelling can
/// sort.
///
/// THE ORDER IS (mtime, path) AND THE SECOND HALF IS NOT DECORATION. Records
/// written in the same second are common — one verification writes several
/// through the side-workspace gate — and a sort that left them in the order the
/// filesystem happened to hand them over would remove a different set on each
/// run over the same directory.
pub fn plan(mut entries: Vec<Entry>, budget: u64) -> Plan {
    entries.sort_by(|a, b| match a.modified.cmp(&b.modified) {
        Ordering::Equal => a.path.cmp(&b.path),
        other => other,
    });
    let total: u64 = entries.iter().map(|entry| entry.bytes).sum();
    let mut kept = total;
    let mut remove = Vec::new();
    // `len() - 1`: THE NEWEST RECORD IS NEVER REMOVED. A budget below the size
    // of one file would otherwise delete the log of the run that is printing
    // this very line, and a collector is not entitled to that.
    for entry in entries.iter().take(entries.len().saturating_sub(1)) {
        if kept <= budget {
            break;
        }
        kept -= entry.bytes;
        remove.push(entry.clone());
    }
    Plan {
        remove,
        total,
        kept,
        still_over: kept > budget,
    }
}

/// What one directory's collection did.
#[derive(Debug)]
pub struct Report {
    pub directory: PathBuf,
    pub budget: u64,
    pub files: usize,
    pub plan: Plan,
    /// Bytes actually removed. Equal to `total - kept` after a real run, and
    /// zero after a dry run — kept separate so the printed line cannot claim a
    /// reclaim that did not happen.
    pub removed_bytes: u64,
    pub removed_files: usize,
    pub links: usize,
    pub unreadable: Vec<String>,
}

/// Bring one directory inside its budget.
///
/// A REMOVAL THAT FAILS IS A FINDING, not a warning to be swallowed: a
/// collector that cannot delete is a directory that keeps growing, which is the
/// exact state this program exists to end, and it would otherwise be invisible
/// behind a report of everything it MEANT to remove.
pub fn collect(directory: &Path, budget: u64, dry_run: bool) -> Result<Report, String> {
    let found = survey(directory)?;
    let files = found.entries.len();
    let plan = plan(found.entries, budget);
    let mut removed_bytes = 0;
    let mut removed_files = 0;
    if !dry_run {
        for entry in &plan.remove {
            fs::remove_file(&entry.path)
                .map_err(|e| format!("{} could not be removed: {e}", entry.path.display()))?;
            removed_bytes += entry.bytes;
            removed_files += 1;
        }
    }
    Ok(Report {
        directory: directory.to_path_buf(),
        budget,
        files,
        plan,
        removed_bytes,
        removed_files,
        links: found.links,
        unreadable: found.unreadable,
    })
}

/// Bytes as a person reads them, one decimal, binary units — the same unit the
/// declaration is written in, so a budget and a measurement can be compared
/// without arithmetic.
pub fn mib(bytes: u64) -> String {
    format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
}

/// Which of the record directories this repository DECLARES have no collector.
///
/// THE DECISION IS FACTORED OUT SO THE LAW AND ITS CONTROL DRIVE THE SAME CODE.
/// `tests/declared.rs` asks the real repository — every injection sweep's `logs`
/// field, and the log directory `scripts/verify.sh` reports for itself — and
/// hands the answers here. Its control hands the same function a declaration
/// with a hole in it. A control that re-implemented the comparison would prove
/// that a second spelling can find a difference, which is the one thing nobody
/// needs to know.
///
/// `sources` is `(who says so, the directory it named)`, and the finding quotes
/// the first half: "some directory is uncollected" sends a reader looking, and
/// "the sweep manifest at X names it" sends them to the file.
pub fn uncovered(
    declaration: &Declaration,
    tree: &Path,
    sources: &[(String, PathBuf)],
) -> Vec<String> {
    let collected: Vec<PathBuf> = declaration
        .directories
        .iter()
        .filter_map(|entry| resolve(tree, &entry.path).ok())
        .collect();
    let mut findings = Vec::new();
    for (who, directory) in sources {
        let resolved = normalise(directory);
        if !collected.contains(&resolved) {
            findings.push(format!(
                "{who} writes records into {} and nothing collects them — the \
                 directory is named by no entry in {DECLARATION}, so it grows \
                 for as long as this repository is worked in",
                resolved.display()
            ));
        }
    }
    findings
}
