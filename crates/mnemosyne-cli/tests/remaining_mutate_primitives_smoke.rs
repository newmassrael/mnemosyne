//! Round 125 — remaining mutate primitive smoke test (Phase 0c entry #3).
//!
//! This test exercises §15 *Spec mutate API surface*'s legacy markdown-surgical
//! mutate primitives that remain after Round 287 (add_section retired in
//! Round 289 Phase H — atomic add_section is the new path; legacy
//! markdown-surgical insert is gone).
//!
//! Test scope (per-primitive happy + reject path):
//! add_cross_ref:  positive intra-doc ref + orphan to_target reject
//! set_section_decision_status: Phase 1+ stub validation (status change ValidatorReject)
//! set_section_body:  positive replace + missing section reject

use mnemosyne_validator::{
 add_cross_ref, parse_markdown,
 schema::{DecisionStatus, RefKind},
 set_section_body, set_section_decision_status, MutateErrorKind, Workspace,
};
use std::fs;

fn write_workspace(content: &str) -> tempfile::TempDir {
 let dir = tempfile::tempdir().unwrap();
 let docs = dir.path().join("docs");
 fs::create_dir_all(&docs).unwrap();
 fs::write(docs.join("TEST.md"), content).unwrap();
 dir
}

fn workspace_for(content: &str) -> Workspace {
 let parsed = parse_markdown(content, "docs/TEST.md");
 let mut ws = Workspace::new();
 ws.insert("docs/TEST.md".to_string(), parsed);
 ws
}

const BASE_DOC: &str = "\
# Test Doc

## 1. First section

body 1

## 2. Second section

body 2

## Changelog

- Round 1:
  - bullet a
";

// Round 289 Phase H — legacy markdown-surgical add_section primitive retired.
// Atomic add_section (atomic.rs) tests live in mnemosyne-validator's unit
// suite (add_section_basic_creates_outline_and_persists + 7 siblings).

// ============================================================================
// add_cross_ref
// ============================================================================

#[test]
fn add_cross_ref_intra_doc_decision() {
 let dir = write_workspace(BASE_DOC);
 let ws = workspace_for(BASE_DOC);

 let receipt = add_cross_ref(
 &ws,
 "docs/TEST.md",
 "1", // from §1
 "2", // to §2
 RefKind::Decision,
 dir.path(),
 )
 .expect("intra-doc cross_ref should succeed");
 assert_eq!(receipt.primitive, "add_cross_ref");

 let after = fs::read_to_string(dir.path().join("docs/TEST.md")).unwrap();
 // The reference text should mention §2.
 assert!(after.contains("§2"));
}

#[test]
fn add_cross_ref_rejects_orphan_target() {
 let dir = write_workspace(BASE_DOC);
 let ws = workspace_for(BASE_DOC);

 let err = add_cross_ref(
 &ws,
 "docs/TEST.md",
 "1",
 "999", // orphan
 RefKind::Decision,
 dir.path(),
 )
 .expect_err("orphan target should reject");
 assert_eq!(err.kind, MutateErrorKind::OrphanRejection);

 let after = fs::read_to_string(dir.path().join("docs/TEST.md")).unwrap();
 assert_eq!(after, BASE_DOC);
}

// ============================================================================
// set_section_decision_status (Phase 1+ stub)
// ============================================================================

#[test]
fn set_section_decision_status_phase_1_stub() {
 let dir = write_workspace(BASE_DOC);
 let ws = workspace_for(BASE_DOC);

 let err = set_section_decision_status(
 &ws,
 "docs/TEST.md",
 "1",
 DecisionStatus::Superseded,
 Some("2"),
 dir.path(),
 )
 .expect_err("Phase 1+ stub should return ValidatorReject");
 assert_eq!(err.kind, MutateErrorKind::ValidatorReject);
 assert!(err.detail.contains("Phase 1+"));

 let after = fs::read_to_string(dir.path().join("docs/TEST.md")).unwrap();
 assert_eq!(after, BASE_DOC);
}

#[test]
fn set_section_decision_status_rejects_superseded_without_superseding() {
 let dir = write_workspace(BASE_DOC);
 let ws = workspace_for(BASE_DOC);

 let err = set_section_decision_status(
 &ws,
 "docs/TEST.md",
 "1",
 DecisionStatus::Superseded,
 None,
 dir.path(),
 )
 .expect_err("missing superseding should reject");
 assert_eq!(err.kind, MutateErrorKind::ValidatorReject);
 assert!(err.detail.contains("T1 rule 4") || err.detail.contains("superseding"));
}

// ============================================================================
// set_section_body
// ============================================================================

#[test]
fn set_section_body_replaces_content() {
 let dir = write_workspace(BASE_DOC);
 let ws = workspace_for(BASE_DOC);

 let new_body = "\
NEW REPLACEMENT BODY for section 1.

Multi-paragraph content with §2 reference.
";

 let receipt = set_section_body(&ws, "docs/TEST.md", "1", new_body, dir.path())
 .expect("set_section_body should succeed");
 assert_eq!(receipt.primitive, "set_section_body");
 assert!(receipt.affected_sections.iter().any(|s| s == "1"));

 let after = fs::read_to_string(dir.path().join("docs/TEST.md")).unwrap();
 assert!(after.contains("NEW REPLACEMENT BODY"));
 // Original body should be replaced.
 assert!(!after.contains("body 1\n"));
 // Pre-existing other sections preserved.
 assert!(after.contains("## 2. Second section"));
 assert!(after.contains("body 2"));
 // Changelog preserved.
 assert!(after.contains("- Round 1:"));
}

#[test]
fn set_section_body_rejects_missing_section() {
 let dir = write_workspace(BASE_DOC);
 let ws = workspace_for(BASE_DOC);

 let err = set_section_body(&ws, "docs/TEST.md", "999", "new body", dir.path())
 .expect_err("missing section should reject");
 assert_eq!(err.kind, MutateErrorKind::NotFound);
}

/// Round 129 regression guard — pre-129 `set_section_body` would silently delete
/// nested sub-sections by treating the body as everything down to the next
/// sibling. Verify sub-sections survive an explicit body replace.
#[test]
fn set_section_body_preserves_nested_sub_sections() {
 const NESTED_DOC: &str = "\
# Nested test

## 1. Parent section

parent body line

### Sub A

sub-a body

### Sub B

sub-b body

## 2. Sibling section

sibling body

## Changelog

- Round 1:
  - bullet a
";
 let dir = write_workspace(NESTED_DOC);
 let ws = workspace_for(NESTED_DOC);

 let receipt = set_section_body(&ws, "docs/TEST.md", "1", "REPLACED parent body\n", dir.path())
 .expect("set_section_body on parent should succeed");
 assert!(receipt.affected_sections.iter().any(|s| s == "1"));

 let after = fs::read_to_string(dir.path().join("docs/TEST.md")).unwrap();
 assert!(after.contains("REPLACED parent body"));
 // Sub-sections must still be present.
 assert!(after.contains("### Sub A"), "Sub A heading deleted: {}", after);
 assert!(after.contains("sub-a body"), "Sub A body deleted");
 assert!(after.contains("### Sub B"), "Sub B heading deleted");
 assert!(after.contains("sub-b body"), "Sub B body deleted");
 // Sibling and changelog also preserved.
 assert!(after.contains("## 2. Sibling section"));
 assert!(after.contains("sibling body"));
 assert!(after.contains("- Round 1:"));
 // Original parent prose replaced.
 assert!(!after.contains("parent body line"));
}
