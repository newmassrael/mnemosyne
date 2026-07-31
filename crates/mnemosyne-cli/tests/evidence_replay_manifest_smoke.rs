//! Round 882 — every tracked experiment manifest declares the revision that
//! can read it, and the declaration cannot drift from git history.
//!
//! The design this enforces, decided across R878–R881: a record is a triple —
//! the INPUT, the REVISION that produced it, and (later) the EXPECTED OUTPUT.
//! Re-checking runs the declared revision against the untouched input. The
//! input is never edited to suit a later tool, because a kit's
//! `deterministic_pins` is a pre-committed claim about what the BLIND AUTHOR
//! produced, and editing that output until it imports would re-establish the
//! pin by editing the thing the pin measures (the R469 contamination bound).
//! R880 proved the revision half is feasible — a June revision still builds and
//! still reads its own bytes. R881 proved the comparison is sound. This round
//! writes the revision down.
//!
//! Why this is a TEST and not a `validate-workspace` check: it is a property of
//! THIS repo's evidence, not of the Mnemosyne product. A consumer has no
//! `claudedocs/phase1-*` kits, and exporting a repo-local rule into the
//! validator every consumer runs would be the wrong home for it.
//!
//! Deliberately NOT asserted here: that the declared revision still BUILDS, or
//! that replaying reproduces anything. That is `pinned_revision_rechecks_
//! evidence_smoke` (R880), which compiles a second workspace and is `#[ignore]`d
//! for it. This gate is the cheap always-on half: the declaration exists, it
//! resolves, and it says where it came from.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The manifest filenames a replay unit can name. Kept as a closed list so a
/// kit that invents a new one is a loud failure rather than a silent omission —
/// the census that preceded this round got its count wrong TWICE by keying on
/// filenames it had guessed rather than on a declared set.
const FACTS_NAMES: &[&str] = &[
    "facts.json",
    "facts.norm.json",
    "facts-manifest.json",
    "facts-base.json",
    "facts-preconditions.json",
    "facts-extracted.json",
    "facts-inferred.json",
    "contrast-manifest.json",
    "render-reextracted-facts.json",
    "A-facts.json",
    "B-facts.json",
];

const PROVENANCE_KINDS: &[&str] = &["derived-upper-bound", "declared-at-run"];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repo root is two levels above the crate manifest")
        .to_path_buf()
}

fn git(args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(repo_root())
        .output()
        .expect("git exec");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).expect("git output is utf-8")
}

fn tracked_evidence_files() -> Vec<String> {
    git(&["ls-files", "claudedocs/phase1-*"])
        .lines()
        .map(str::to_string)
        .collect()
}

struct Entry {
    unit: String,
    sections: String,
    facts: String,
    role: String,
    revision: String,
    provenance: String,
}

fn declared_entries() -> Vec<Entry> {
    let root = repo_root();
    let mut out = Vec::new();
    for file in tracked_evidence_files() {
        if !file.ends_with("/replay.json") {
            continue;
        }
        let unit = file.trim_end_matches("/replay.json").to_string();
        let doc: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(root.join(&file)).expect("read replay"))
                .unwrap_or_else(|e| panic!("{file} is not JSON: {e}"));
        assert_eq!(doc["schema"], "kit-replay/v1", "{file}");
        let provenance = doc["revision_provenance"]
            .as_str()
            .unwrap_or_else(|| panic!("{file} declares no revision_provenance"))
            .to_string();
        assert!(
            PROVENANCE_KINDS.contains(&provenance.as_str()),
            "{file}: unknown revision_provenance `{provenance}` — a record whose \
             pin does not say where it came from cannot be weighed"
        );
        for m in doc["manifests"]
            .as_array()
            .unwrap_or_else(|| panic!("{file} declares no manifests"))
        {
            out.push(Entry {
                unit: unit.clone(),
                sections: m["sections"].as_str().expect("sections path").to_string(),
                facts: m["facts"].as_str().expect("facts path").to_string(),
                role: m["role"].as_str().expect("role").to_string(),
                revision: m["revision"].as_str().expect("revision").to_string(),
                provenance: provenance.clone(),
            });
        }
    }
    out
}

/// Every tracked facts manifest is declared exactly once, and both halves of
/// each declared pair exist. The coverage direction is the load-bearing one:
/// this round exists because a manifest nobody had enumerated was found by
/// accident, twice.
#[test]
fn every_tracked_facts_manifest_is_declared_exactly_once() {
    let root = repo_root();
    let entries = declared_entries();
    assert!(
        !entries.is_empty(),
        "no replay declarations found at all — this gate would pass vacuously"
    );

    let mut declared: BTreeSet<String> = BTreeSet::new();
    for e in &entries {
        let facts = format!("{}/{}", e.unit, e.facts);
        let sections = format!("{}/{}", e.unit, e.sections);
        let facts = normalize(&facts);
        let sections = normalize(&sections);
        assert!(
            root.join(&facts).is_file(),
            "{}: declares a facts manifest that is not there: {facts}",
            e.unit
        );
        assert!(
            root.join(&sections).is_file(),
            "{}: declares a sections manifest that is not there: {sections}",
            e.unit
        );
        assert!(
            declared.insert(facts.clone()),
            "{facts} is declared by more than one replay unit"
        );
    }

    let present: BTreeSet<String> = tracked_evidence_files()
        .into_iter()
        .filter(|f| {
            Path::new(f)
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| FACTS_NAMES.contains(&n))
        })
        .collect();
    let missing: Vec<&String> = present.difference(&declared).collect();
    assert!(
        missing.is_empty(),
        "tracked facts manifests with no replay declaration — the class this \
         round exists to close: {missing:#?}"
    );
    let phantom: Vec<&String> = declared.difference(&present).collect();
    assert!(
        phantom.is_empty(),
        "declared manifests that are not tracked, or whose name is outside \
         FACTS_NAMES: {phantom:#?}"
    );
}

/// Resolve `a/b/../c` without touching the filesystem — the declarations are
/// relative to their unit directory and some point back up out of it.
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

/// Every declared revision is a real commit, and a DERIVED one still equals
/// what the derivation yields. A pin that has quietly drifted from history
/// would send a future re-check to the wrong tree and look fine doing it.
#[test]
fn every_declared_revision_resolves_and_derived_ones_still_match() {
    let entries = declared_entries();
    assert!(!entries.is_empty(), "no declarations — nothing asserted");
    let mut checked_derived = 0usize;
    for e in &entries {
        assert_eq!(
            e.revision.len(),
            40,
            "{}: not a full sha: {}",
            e.unit,
            e.revision
        );
        let ok = Command::new("git")
            .args(["cat-file", "-e", &format!("{}^{{commit}}", e.revision)])
            .current_dir(repo_root())
            .output()
            .expect("git exec")
            .status
            .success();
        assert!(ok, "{}: revision does not resolve: {}", e.unit, e.revision);

        if e.provenance == "derived-upper-bound" {
            let facts = normalize(&format!("{}/{}", e.unit, e.facts));
            let derived = git(&["log", "--diff-filter=A", "--format=%H", "-1", "--", &facts])
                .trim()
                .to_string();
            assert_eq!(
                derived, e.revision,
                "{facts}: the declared pin no longer matches what the derivation \
                 yields — one of the two is wrong and a re-check would use the \
                 wrong tree"
            );
            checked_derived += 1;
        }
    }
    // Non-vacuity: the derivation comparison is the substantive half, so at
    // least one entry must actually have taken it.
    assert!(
        checked_derived > 0,
        "no derived pin was re-derived — the comparison above ran on nothing"
    );
    println!(
        "{} declarations, {checked_derived} re-derived",
        entries.len()
    );
}

/// A manifest the record marks as raw agent output is NOT a replay input. The
/// distinction is recorded because one kit ships both its agent's raw output and
/// the normalized form, and a census that could not tell them apart reported a
/// working arm as broken.
#[test]
fn roles_are_from_the_declared_vocabulary_and_both_are_used() {
    let entries = declared_entries();
    let roles: BTreeSet<&str> = entries.iter().map(|e| e.role.as_str()).collect();
    for r in &roles {
        assert!(
            ["replay-input", "raw-agent-output"].contains(r),
            "unknown role `{r}`"
        );
    }
    assert!(
        roles.contains("replay-input") && roles.contains("raw-agent-output"),
        "both roles must be exercised or the distinction is untested: {roles:?}"
    );
}
