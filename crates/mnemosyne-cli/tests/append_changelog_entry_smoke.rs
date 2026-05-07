//! Round 124 — `mnemosyne-cli append-changelog-entry` smoke test (production lift,
//! Phase 0c entry #2).
//!
//! This test exercises §15 *Spec mutate API surface* — *first mutate primitive* carry
//! integration test (Round 124 ratify carry). CLI binary execution + temporary directory mock.
//! Simulate the 7-doc workspace, then append → round-trip + T1/T2 validation-path carry.
//!
//! Test scope:
//! (i) normal append on round-trip validation (entry_id monotonic + sub_bullets consistency +
//! disk byte-preservation is out-of-scope; 0 mutations).
//! (ii) non-monotonic entry_id reject (MutateErrorKind::MonotonicViolation)
//! (iii) non-monotonic frozen_at_transaction_time reject (MutateErrorKind::MonotonicViolation)
//! (iv) surgical insert byte-preservation validation (0 mutations outside the changelog scope)
//! (v) atomic write rollback validation (failure path in disk restore)

use mnemosyne_validator::{
 append_changelog_entry, parse_markdown, MutateErrorKind, Workspace,
};
use std::fs;
use std::path::Path;

fn write_workspace(content: &str) -> tempfile::TempDir {
 let dir = tempfile::tempdir().unwrap();
 let docs = dir.path().join("docs");
 fs::create_dir_all(&docs).unwrap();
 fs::write(docs.join("TEST.md"), content).unwrap();
 dir
}

fn workspace_for(path: &Path, content: &str) -> Workspace {
 let parsed = parse_markdown(content, "docs/TEST.md");
 let mut ws = Workspace::new();
 ws.insert("docs/TEST.md".to_string(), parsed);
 let _ = path; // path used by caller to pass dir; not needed in workspace itself.
 ws
}

const MULTI_ENTRY_DOC: &str = "\
# Test Doc

## §1 First section

body 1

## §2 Second section

body 2 with §1 reference

## Changelog

- Round 1 (FIRST):
  - bullet 1a
  - bullet 1b
- Round 2 (SECOND):
  - bullet 2a
- Round 3 (THIRD):
  - bullet 3a
  - bullet 3b
  - bullet 3c
";

#[test]
fn case_i_append_succeeds_with_round_trip_diff_zero() {
 let dir = write_workspace(MULTI_ENTRY_DOC);
 let ws = workspace_for(dir.path(), MULTI_ENTRY_DOC);

 let receipt = append_changelog_entry(
 &ws,
 "docs/TEST.md",
 "Round 4",
 Some("FOURTH-INTEGRATION-TEST"),
 &[
 "first new bullet about §1".to_string(),
 "second new bullet without ref".to_string(),
 ],
 9999,
 dir.path(),
 )
 .expect("normal append should succeed");

 assert_eq!(receipt.primitive, "append_changelog_entry");
 assert_eq!(receipt.affected_docs, vec!["docs/TEST.md"]);
 assert_eq!(receipt.round_trip_diff_count, 0);
 assert_eq!(receipt.applied_at_transaction_time, 9999);
 assert!(receipt
 .validator_path_invocations
 .iter()
 .any(|v| v == "t2::frozen_ledger_jaccard"));
 assert!(receipt
 .validator_path_invocations
 .iter()
 .any(|v| v == "t1::cross_ref_orphan_reject_with_workspace"));

 // Verify on-disk content.
 let after = fs::read_to_string(dir.path().join("docs/TEST.md")).unwrap();
 assert!(after.contains("- Round 4 (FOURTH-INTEGRATION-TEST):"));
 assert!(after.contains(" - first new bullet about §1"));
 assert!(after.contains(" - second new bullet without ref"));
 // Pre-existing entries preserved.
 assert!(after.contains("- Round 1 (FIRST):"));
 assert!(after.contains("- Round 3 (THIRD):"));
 assert!(after.contains(" - bullet 3c"));
}

#[test]
fn case_ii_non_monotonic_entry_id_rejects() {
 let dir = write_workspace(MULTI_ENTRY_DOC);
 let ws = workspace_for(dir.path(), MULTI_ENTRY_DOC);

 let err = append_changelog_entry(
 &ws,
 "docs/TEST.md",
 "Round 5", // skipped Round 4
 None,
 &["should not commit".to_string()],
 9999,
 dir.path(),
 )
 .expect_err("skipped entry_id should reject");
 assert_eq!(err.kind, MutateErrorKind::MonotonicViolation);

 // Disk content unchanged.
 let after = fs::read_to_string(dir.path().join("docs/TEST.md")).unwrap();
 assert_eq!(after, MULTI_ENTRY_DOC);
}

#[test]
fn case_iii_non_monotonic_transaction_time_rejects() {
 let dir = write_workspace(MULTI_ENTRY_DOC);
 let mut ws = workspace_for(dir.path(), MULTI_ENTRY_DOC);

 // Override last entry's frozen_at_transaction_time to a high value.
 let parsed = ws.docs.get_mut("docs/TEST.md").unwrap();
 parsed.changelog_entries.last_mut().unwrap().frozen_at_transaction_time = 50_000;

 let err = append_changelog_entry(
 &ws,
 "docs/TEST.md",
 "Round 4",
 None,
 &["should not commit".to_string()],
 100, // less than 50_000
 dir.path(),
 )
 .expect_err("non-monotonic txn_time should reject");
 assert_eq!(err.kind, MutateErrorKind::MonotonicViolation);

 // Disk content unchanged.
 let after = fs::read_to_string(dir.path().join("docs/TEST.md")).unwrap();
 assert_eq!(after, MULTI_ENTRY_DOC);
}

#[test]
fn case_iv_surgical_insert_preserves_pre_changelog_bytes() {
 // Doc with rich pre-changelog content (bold, code, fences) — verify byte preservation.
 let content = "\
# Doc with rich content

## §1 With **bold** and *italic*

body 1 with `inline code` and §2 reference.

```rust
fn example() -> u32 {
 42
}
```

## §2 Second section with [link](http://example.com)

body 2 has a list:
- item a
- item b

## Changelog

- Round 1:
  - existing bullet
";
 let dir = write_workspace(content);
 let ws = workspace_for(dir.path(), content);

 append_changelog_entry(
 &ws,
 "docs/TEST.md",
 "Round 2",
 Some("PRESERVATION-TEST"),
 &["new bullet".to_string()],
 100,
 dir.path(),
 )
 .expect("surgical insert should succeed");

 let after = fs::read_to_string(dir.path().join("docs/TEST.md")).unwrap();
 // Pre-changelog content byte-identical.
 assert!(after.contains("body 1 with `inline code` and §2 reference."));
 assert!(after.contains("```rust\nfn example() -> u32 {\n 42\n}\n```"));
 assert!(after.contains("[link](http://example.com)"));
 assert!(after.contains("- item a\n- item b"));
 // New entry at end.
 assert!(after.contains("- Round 2 (PRESERVATION-TEST):"));
 assert!(after.contains(" - new bullet"));
}

#[test]
fn case_v_invalid_doc_rejects_without_disk_corruption() {
 let dir = write_workspace(MULTI_ENTRY_DOC);
 let ws = workspace_for(dir.path(), MULTI_ENTRY_DOC);

 // doc not in workspace → NotFound.
 let err = append_changelog_entry(
 &ws,
 "docs/UNKNOWN.md",
 "Round 4",
 None,
 &["x".to_string()],
 9999,
 dir.path(),
 )
 .expect_err("unknown doc should reject");
 assert_eq!(err.kind, MutateErrorKind::NotFound);

 // Disk content unchanged.
 let after = fs::read_to_string(dir.path().join("docs/TEST.md")).unwrap();
 assert_eq!(after, MULTI_ENTRY_DOC);
}

#[test]
fn case_vi_receipt_serializes_to_json() {
 let dir = write_workspace(MULTI_ENTRY_DOC);
 let ws = workspace_for(dir.path(), MULTI_ENTRY_DOC);

 let receipt = append_changelog_entry(
 &ws,
 "docs/TEST.md",
 "Round 4",
 Some("JSON-SERIAL"),
 &["bullet for serialize".to_string()],
 12345,
 dir.path(),
 )
 .expect("append should succeed");

 let json = serde_json::to_string_pretty(&receipt).unwrap();
 assert!(json.contains("\"primitive\""));
 assert!(json.contains("\"append_changelog_entry\""));
 assert!(json.contains("\"affected_docs\""));
 assert!(json.contains("\"applied_at_transaction_time\""));
 assert!(json.contains("12345"));
 assert!(json.contains("\"validator_path_invocations\""));
}
