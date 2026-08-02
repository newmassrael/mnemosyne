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

/// What [`set_role`] changed on one record.
#[derive(Debug, Default)]
pub struct Reassigned {
    /// `record :: unit-relative path :: old -> new` for each entry rewritten.
    pub changed: Vec<String>,
}

/// What a reassignment writes onto an input besides its role (Round 974).
#[derive(Debug, Default)]
pub struct RoleDetail<'a> {
    /// The command that prints these bytes: verb first, then its arguments.
    pub reproduced_by: &'a [String],
    /// The replay whose workspace that command runs in.
    pub reproduced_after: Option<&'a str>,
    /// Why no single command can print these bytes.
    pub unreproducible: Option<&'a str>,
    /// The kit's own declared inputs the command reads beside the store.
    pub reproduced_with: &'a [String],
    /// The exit status the recorded run ended with, when it was not zero.
    pub reproduced_exit: Option<i64>,
    /// The ids the recorded run's store held that this kit's replay does not
    /// create, when THAT is what stops a command from existing (Round 975).
    /// State a store difference here rather than in the excuse's prose, where
    /// nothing settles it.
    pub store_surplus: &'a [String],
}

/// Rewrite the role of inputs a record ALREADY declares, and nothing else.
///
/// Round 973 needed sixteen entries moved from `run-artifact` to
/// `reproduced-output`, and Round 953 named hand-editing the record as the
/// place errors hide — a mistyped path in a 468-entry array reads as a
/// declaration that resolves to nothing. So the edit is a verb: it fails on a
/// path the record does not declare, rather than adding one, because adding is
/// [`run`]'s job and silently creating an entry here would defeat the gate that
/// requires each path exactly once.
///
/// The role name is NOT validated against a vocabulary here. There is one home
/// for that vocabulary — the gate — and a second copy in this workspace would
/// be a second thing to keep in step. A role this tool cannot spell is rejected
/// by `cargo test` on the next run, which is where every other record literal
/// is decided too.
pub fn set_role(
    record: &str,
    paths: &[String],
    role: &str,
    detail: &RoleDetail<'_>,
) -> HResult<Reassigned> {
    let raw = read_file(record)?;
    let mut doc: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("{record} is not JSON: {e}"))?;
    let inputs = doc
        .get_mut("inputs")
        .and_then(|v| v.as_array_mut())
        .ok_or_else(|| format!("{record} declares no `inputs` array"))?;

    let mut out = Reassigned::default();
    for want in paths {
        let entry = inputs
            .iter_mut()
            .find(|i| i.get("path").and_then(|p| p.as_str()) == Some(want.as_str()))
            .ok_or_else(|| {
                format!(
                    "{record} declares no input at `{want}` — this verb rewrites \
                     declarations, it does not create them; run \
                     `declare-run-tree` first if the file is undeclared"
                )
            })?;
        let was = entry
            .get("role")
            .and_then(|r| r.as_str())
            .unwrap_or("<none>")
            .to_string();
        let obj = entry
            .as_object_mut()
            .ok_or_else(|| format!("{record}: the input at `{want}` is not an object"))?;
        obj.insert("role".to_string(), serde_json::json!(role));
        // Every optional field is written or REMOVED on each call, never left
        // over from a previous one: a role that no longer claims a command must
        // not keep the command, which would name something nothing runs and
        // read as checked.
        for (key, value) in [
            (
                "reproduced_by",
                (!detail.reproduced_by.is_empty()).then(|| serde_json::json!(detail.reproduced_by)),
            ),
            (
                "reproduced_after",
                detail.reproduced_after.map(|a| serde_json::json!(a)),
            ),
            (
                "unreproducible",
                detail.unreproducible.map(|u| serde_json::json!(u)),
            ),
            (
                "reproduced_with",
                (!detail.reproduced_with.is_empty())
                    .then(|| serde_json::json!(detail.reproduced_with)),
            ),
            (
                "reproduced_exit",
                detail.reproduced_exit.map(|e| serde_json::json!(e)),
            ),
            (
                "store_surplus",
                (!detail.store_surplus.is_empty()).then(|| serde_json::json!(detail.store_surplus)),
            ),
        ] {
            match value {
                Some(v) => {
                    obj.insert(key.to_string(), v);
                }
                None => {
                    obj.remove(key);
                }
            }
        }
        out.changed
            .push(format!("{record} :: {want} :: {was} -> {role}"));
    }

    let mut rendered = serde_json::to_string_pretty(&doc)
        .map_err(|e| format!("{record}: cannot render the updated record: {e}"))?;
    rendered.push('\n');
    write_file(record, &rendered)?;
    Ok(out)
}

/// Point a declared replay at the configuration its run used.
///
/// Round 974 needed this because a report transcript reproduces only in a
/// workspace that holds the author's `mnemosyne.toml` and the files it names —
/// `rules_path`, `canon_order_path` — and the replay's `config` field is the
/// existing mechanism for putting them there. Measured before it was written:
/// seeding the author's config alongside the manifests does NOT move the store
/// digest for any of the three k-of-n stages, so declaring it here leaves every
/// `expected_store_sha256` in the record standing.
pub fn set_replay_config(record: &str, replay: &str, config: &str) -> HResult<()> {
    let raw = read_file(record)?;
    let mut doc: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("{record} is not JSON: {e}"))?;
    let replays = doc
        .get_mut("replays")
        .and_then(|v| v.as_array_mut())
        .ok_or_else(|| format!("{record} declares no `replays` array"))?;
    let entry = replays
        .iter_mut()
        .find(|r| r.get("name").and_then(|n| n.as_str()) == Some(replay))
        .ok_or_else(|| format!("{record} declares no replay named `{replay}`"))?;
    entry
        .as_object_mut()
        .ok_or_else(|| format!("{record}: the replay `{replay}` is not an object"))?
        .insert("config".to_string(), serde_json::json!(config));

    let mut rendered = serde_json::to_string_pretty(&doc)
        .map_err(|e| format!("{record}: cannot render the updated record: {e}"))?;
    rendered.push('\n');
    write_file(record, &rendered)
}

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

    /// A path the record does not declare is refused rather than created. This
    /// is the whole reason the reassignment is a verb: a mistyped path in a
    /// 468-entry array would otherwise land as a NEW declaration that resolves
    /// to nothing, and the coverage gate would then report the real file as
    /// undeclared while the typo sat there looking like a record.
    #[test]
    fn reassigning_an_undeclared_path_is_an_error_and_writes_nothing() {
        let dir = std::env::temp_dir().join("mn-set-role-undeclared");
        std::fs::create_dir_all(&dir).expect("mkdir");
        let record = dir.join("replay.json");
        let before = r#"{"inputs":[{"path":"run/contract.txt","role":"run-artifact"}]}"#;
        std::fs::write(&record, before).expect("seed record");
        let path = record.to_string_lossy().into_owned();

        let verb = ["describe-schema".to_string()];
        let err = set_role(
            &path,
            &["run/contrakt.txt".to_string()],
            "reproduced-output",
            &RoleDetail {
                reproduced_by: &verb,
                ..RoleDetail::default()
            },
        )
        .expect_err("an undeclared path must not be accepted");
        assert!(err.contains("declares no input"), "unhelpful error: {err}");
        assert_eq!(
            std::fs::read_to_string(&record).expect("read back"),
            before,
            "a refused reassignment must leave the record untouched"
        );
        std::fs::remove_dir_all(&dir).ok();
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
