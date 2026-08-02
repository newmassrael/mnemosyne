//! Declaring a kit's run tree into its replay record.
//!
//! Round 952 gave every DECLARED input a digest a gate re-checks, and Round 953
//! measured how far that reached: 552 tracked files live under kit `run/` trees
//! and 127 of them were declared. The other 425 — the manuscripts the judges
//! read, the judges' own reports, the label map that made them blind, the
//! briefs the authors were handed, the logs the run emitted — were pinned by
//! nothing.
//!
//! This walks the gap the way the gate does, FROM THE FILESYSTEM, and writes
//! the missing entries. It claims only what it can establish: `run-artifact`
//! says the file is part of this kit's frozen run tree, not who produced it. A
//! kit that establishes provenance uses the sharper roles, and this tool never
//! touches an entry that already exists.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

use crate::util::{read_file, write_file, HResult};

/// The role this tool writes. The vocabulary lives in the gate that enforces
/// it; this is the one member a mechanical walk is entitled to claim.
const RUN_ARTIFACT: &str = "run-artifact";

/// What one call to [`run`] added.
#[derive(Debug, Default)]
pub struct Declared {
    /// `record :: unit-relative path` for each entry written.
    pub added: Vec<String>,
    /// Artifacts that were already declared, by any record.
    pub already: usize,
}

fn git(root: &Path, args: &[&str]) -> HResult<String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|e| format!("git {args:?}: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    String::from_utf8(out.stdout).map_err(|e| format!("git {args:?} output is not utf-8: {e}"))
}

/// Resolve `a/b/../c` textually — declared paths are relative to their unit
/// directory and some point back up out of it.
fn normalize(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "." | "" => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

/// The record that owns a path: the nearest ancestor directory holding a
/// `replay.json`. Nearest, not outermost, because kits nest.
fn owning_unit(path: &str, units: &BTreeSet<String>) -> Option<String> {
    let mut dir = Path::new(path).parent();
    while let Some(d) = dir {
        let candidate = d.to_string_lossy().into_owned();
        if units.contains(&candidate) {
            return Some(candidate);
        }
        dir = d.parent();
    }
    None
}

fn declared_paths(unit: &str, doc: &serde_json::Value) -> HResult<Vec<String>> {
    let inputs = doc
        .get("inputs")
        .and_then(|v| v.as_array())
        .ok_or_else(|| format!("{unit}/replay.json declares no `inputs` array"))?;
    inputs
        .iter()
        .map(|i| {
            i.get("path")
                .and_then(|v| v.as_str())
                .map(|p| normalize(&format!("{unit}/{p}")))
                .ok_or_else(|| format!("{unit}/replay.json: an input declares no `path`"))
        })
        .collect()
}

/// Write a `run-artifact` entry for every tracked run-tree file the given
/// records own and do not yet declare.
///
/// Ownership is computed against EVERY tracked record, not just the ones named
/// on the command line, so passing one record cannot silently claim a nested
/// kit's tree. An artifact already declared anywhere is left alone — the gate
/// requires each path to be declared exactly once, and re-declaring it here
/// would break that in the act of trying to satisfy it.
pub fn run(records: &[String]) -> HResult<Declared> {
    let root_raw = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|e| format!("git rev-parse: {e}"))?;
    if !root_raw.status.success() {
        return Err("not inside a git work tree".to_string());
    }
    let root = Path::new(
        std::str::from_utf8(&root_raw.stdout)
            .map_err(|e| format!("repo root is not utf-8: {e}"))?
            .trim(),
    )
    .to_path_buf();

    let tracked: Vec<String> = git(&root, &["ls-files", "claudedocs/phase1-*"])?
        .lines()
        .map(str::to_string)
        .collect();
    let units: BTreeSet<String> = tracked
        .iter()
        .filter(|f| f.ends_with("/replay.json"))
        .map(|f| f.trim_end_matches("/replay.json").to_string())
        .collect();
    if units.is_empty() {
        return Err("no tracked kit records found — nothing to declare into".to_string());
    }

    // Every path any record already declares, so an entry is never written
    // twice and a nested kit's file is never claimed by two units.
    let mut already: BTreeSet<String> = BTreeSet::new();
    for unit in &units {
        let path = root.join(unit).join("replay.json");
        let raw = read_file(path.to_str().ok_or("record path is not utf-8")?)?;
        let doc: serde_json::Value = serde_json::from_str(&raw)
            .map_err(|e| format!("{unit}/replay.json is not JSON: {e}"))?;
        already.extend(declared_paths(unit, &doc)?);
    }

    // The gap, grouped by the record that owns it.
    let mut missing: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for file in tracked.iter().filter(|f| f.contains("/run/")) {
        if already.contains(file) {
            continue;
        }
        let Some(unit) = owning_unit(file, &units) else {
            return Err(format!(
                "{file} sits under no kit record, so nothing can declare it — \
                 write the record first"
            ));
        };
        missing.entry(unit).or_default().push(file.clone());
    }

    let mut out = Declared {
        already: already.len(),
        ..Declared::default()
    };
    for record in records {
        let unit = Path::new(record)
            .parent()
            .ok_or_else(|| format!("{record} has no parent directory"))?
            .to_string_lossy()
            .into_owned();
        let unit = unit
            .strip_prefix(&format!("{}/", root.display()))
            .unwrap_or(&unit)
            .to_string();
        if !units.contains(&unit) {
            return Err(format!("{record} is not a tracked kit record"));
        }
        let Some(files) = missing.get(&unit) else {
            continue;
        };

        let raw = read_file(record)?;
        let mut doc: serde_json::Value =
            serde_json::from_str(&raw).map_err(|e| format!("{record} is not JSON: {e}"))?;
        let inputs = doc
            .get_mut("inputs")
            .and_then(|v| v.as_array_mut())
            .ok_or_else(|| format!("{record} declares no `inputs` array"))?;
        for file in files {
            let rel = file
                .strip_prefix(&format!("{unit}/"))
                .ok_or_else(|| format!("{file} is not under {unit}"))?;
            inputs.push(serde_json::json!({ "path": rel, "role": RUN_ARTIFACT }));
            out.added.push(format!("{record} :: {rel}"));
        }
        let mut rendered = serde_json::to_string_pretty(&doc)
            .map_err(|e| format!("{record}: cannot render the updated record: {e}"))?;
        rendered.push('\n');
        write_file(record, &rendered)?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_resolves_parent_segments() {
        assert_eq!(normalize("a/b/../c"), "a/c");
        assert_eq!(normalize("a/./b"), "a/b");
        assert_eq!(normalize("kit/v1/../shared/x.json"), "kit/shared/x.json");
    }

    /// Nearest ancestor, not outermost: a nested kit's own record must win, or
    /// its artifacts would be claimed by the record above it.
    #[test]
    fn ownership_is_the_nearest_record_above_a_path() {
        let units: BTreeSet<String> = ["kit", "kit/v3"].iter().map(|s| s.to_string()).collect();
        assert_eq!(
            owning_unit("kit/v3/run/story.md", &units).as_deref(),
            Some("kit/v3")
        );
        assert_eq!(
            owning_unit("kit/run/story.md", &units).as_deref(),
            Some("kit")
        );
        assert_eq!(owning_unit("other/run/story.md", &units), None);
    }
}
