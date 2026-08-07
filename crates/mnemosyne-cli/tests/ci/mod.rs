//! THE one reader of this repository's own GitHub workflows.
//!
//! Two test targets ask questions of them and they must be asking of the same
//! files, parsed the same way: `evidence_replay_smoke` checks that any job able
//! to run its gates checks out full history, and `feature_coverage_smoke`
//! checks that every feature this workspace declares is compiled by somebody. A
//! second loader is a second answer to "which workflows are there", free to
//! drift from the first — the shape R777, R783 and R1080 each closed one level
//! at a time, where a list restated the tree and then went quietly stale.
//!
//! The file list comes from `git ls-files` rather than from a directory walk,
//! so a workflow that is not tracked — and which GitHub therefore does not run
//! — is not counted as covering anything.

#![allow(dead_code)] // each test binary asks a different question of these

use std::path::Path;
use std::process::Command;

use yaml_rust2::{Yaml, YamlLoader};

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
    let docs = YamlLoader::load_from_str(&raw).unwrap_or_else(|e| {
        panic!("{path} is not parseable YAML — GitHub would silently not run it: {e}")
    });
    assert_eq!(docs.len(), 1, "{path}: expected exactly one YAML document");
    docs.into_iter().next().expect("one document")
}

/// Every `run:` script in every job of one workflow, with the job's name.
pub fn run_steps(doc: &Yaml) -> Vec<(String, String)> {
    let mut steps = Vec::new();
    let Some(jobs) = doc["jobs"].as_hash() else {
        return steps;
    };
    for (name, job) in jobs {
        let name = name.as_str().unwrap_or("<unnamed>").to_string();
        let Some(job_steps) = job["steps"].as_vec() else {
            continue;
        };
        for step in job_steps {
            if let Some(script) = step["run"].as_str() {
                steps.push((name.clone(), script.to_string()));
            }
        }
    }
    steps
}
