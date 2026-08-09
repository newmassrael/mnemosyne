//! What a sweep IS, and the one law every reader of one obeys.
//!
//! THE MANIFEST TYPES AND THE ANCHOR LAW LIVE HERE RATHER THAN IN `main.rs`
//! BECAUSE A DECISION IN `main.rs` HAS NO READER. R1096 measured what that costs
//! in this repository: the thing that lied was an exit code, and the whole of
//! what a suite could ask of it was that one number. The law below is now
//! askable — of a fixture, and of every sweep this repository tracks.
//!
//! AND THAT SECOND READER IS THE POINT. An injection sweep is a proof, and its
//! anchors are the part of it that decays: they are exact text, and the source
//! they name moves. R1103 inserted one line between the two lines an anchor
//! matched on, and from that commit the census gate's `every-job-owes-a-restore-record`
//! injection applied to nothing. Nothing said so, because the only thing that
//! checks an anchor is a sweep being RUN — and a sweep is run when somebody
//! decides to run it. R1107 found it four rounds later, by running one.
//!
//! A proof that has quietly stopped proving reads exactly like one that holds.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// One textual replacement in one file. `from` must occur EXACTLY once.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Edit {
    pub file: String,
    pub from: String,
    pub to: String,
}

/// One injection: what it breaks, and what the sweep expects to go red.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Injection {
    pub name: String,
    /// What this injection is FOR, in the author's words — carried into the
    /// report so a number is never read without the claim it is evidence for.
    #[serde(default)]
    pub why: String,
    pub edits: Vec<Edit>,
    /// Test names this injection is expected to turn red. Empty means "say what
    /// went red and judge nothing", which is honest for an exploratory sweep;
    /// naming them makes the harness itself fail when the sweep does not reach
    /// what it was aimed at (the "0 means suspect the injection" rule).
    #[serde(default)]
    pub expect_red: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct Manifest {
    /// The tree to edit and run in.
    pub repo: PathBuf,
    /// The suite, argv-style. Kept in the manifest rather than assumed, because
    /// a harness that hardcodes `cargo test` cannot be tested without running
    /// one.
    pub test_command: Vec<String>,
    /// Where the full logs go. One file per run, never truncated.
    pub logs: PathBuf,
    /// Refuse to start a run when less than this much memory is available.
    ///
    /// The standing rule on this machine is to re-check occupancy BEFORE EVERY
    /// BUILD, because other checkouts share the RAM and a measurement that runs
    /// the machine out of memory is not a measurement. Eight rounds running, the
    /// re-check happened at the start of a session and before the big sweeps and
    /// not before every build — which is what a person does and a program need
    /// not.
    #[serde(default)]
    pub min_free_mb: Option<u64>,
    pub injections: Vec<Injection>,
}

/// Read one manifest, with its two paths resolved AGAINST THE MANIFEST'S OWN
/// DIRECTORY.
///
/// THE BASE IS THE ONE A FILE ALWAYS KNOWS. `repo` and `logs` used to be
/// resolved against the process's working directory, which meant a sweep's
/// meaning depended on where somebody stood when they ran it — and where that
/// was lived in the manifest's `_` prose, in two different conventions, readable
/// by nobody but a person. R1108 needed a second reader (a law over every sweep
/// this repository tracks) and there was no answer in the files for it to use.
///
/// Resolving from the manifest's directory makes a sweep runnable from anywhere
/// and its tree derivable from the file, which is the same move `ci-plan` makes
/// for everything it reads. Lexical rather than `canonicalize`, which requires
/// the path to exist: the log directory is created afterwards, and a reader that
/// refused a manifest for naming a directory it is about to make would be a
/// reader about nothing.
pub fn read_manifest(path: &Path) -> Result<Manifest, String> {
    let raw =
        fs::read_to_string(path).map_err(|e| format!("{} unreadable: {e}", path.display()))?;
    let mut manifest: Manifest = serde_json::from_str(&raw)
        .map_err(|e| format!("{} is not a manifest: {e}", path.display()))?;
    let beside = path.parent().unwrap_or(Path::new("."));
    manifest.repo = absolute(&beside.join(&manifest.repo))?;
    manifest.logs = absolute(&beside.join(&manifest.logs))?;
    Ok(manifest)
}

/// A path made absolute lexically, against the process's working directory.
fn absolute(path: &Path) -> Result<PathBuf, String> {
    std::path::absolute(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// One replacement, as the law the dry run and the write BOTH obey: the anchor
/// occurs EXACTLY once, and what comes back is the text with that one occurrence
/// replaced.
///
/// A replacement that matched nothing produces a run whose silence reads as "the
/// injection did not fire", and one that matched twice produces a change nobody
/// described. Two places decide that — the pre-flight, which refuses before any
/// run, and the write, which is what actually edits the tree — and a law only
/// one of them enforced is the shape where the dry run accepts an edit the write
/// refuses, or worse, the reverse.
pub fn replace_once(text: &str, edit: &Edit) -> Result<String, String> {
    let hits = text.matches(&edit.from).count();
    if hits != 1 {
        return Err(format!(
            "{} : the text to replace occurs {hits} times, not once",
            edit.file
        ));
    }
    Ok(text.replacen(&edit.from, &edit.to, 1))
}

/// Every file any injection touches, as bytes — and, in the same pass, a DRY RUN
/// of every injection.
///
/// ONE FUNCTION AND NOT TWO, so that the bytes checked are the bytes `apply`
/// will edit by construction rather than by argument. The pre-flight exists
/// because the only other place an anchor is checked is `apply`, which runs
/// after the control and after every injection before it: a typo in the ninth of
/// nine costs the control plus eight whole-suite runs, on a machine where one of
/// those is tens of minutes, and the sweep then ends having measured nothing and
/// having edited the tree eight times.
///
/// It is a DRY RUN and not a count against the pristine bytes: an injection's
/// second edit may legitimately rewrite what its first one wrote, and a gate
/// that refused that would be refusing for a reason outside its own law.
///
/// TAKING NO SUITE AND RUNNING NOTHING is what lets a test call it over every
/// sweep this repository tracks — the anchors are text, and whether they still
/// apply is a question about text.
pub fn snapshot_and_dry_run(
    repo: &Path,
    injections: &[Injection],
) -> Result<BTreeMap<PathBuf, Vec<u8>>, String> {
    let mut snapshot: BTreeMap<PathBuf, Vec<u8>> = BTreeMap::new();
    for injection in injections {
        let mut staged: BTreeMap<PathBuf, String> = BTreeMap::new();
        for edit in &injection.edits {
            let path = repo.join(&edit.file);
            if !snapshot.contains_key(&path) {
                let bytes =
                    fs::read(&path).map_err(|e| format!("{} unreadable: {e}", path.display()))?;
                snapshot.insert(path.clone(), bytes);
            }
            let text = match staged.remove(&path) {
                Some(edited) => edited,
                None => String::from_utf8(snapshot[&path].clone()).map_err(|_| {
                    format!(
                        "{} is not text, so no replacement in it can be described",
                        path.display()
                    )
                })?,
            };
            let edited = replace_once(&text, edit)
                .map_err(|problem| format!("{}: {problem}", injection.name))?;
            staged.insert(path, edited);
        }
    }
    Ok(snapshot)
}
