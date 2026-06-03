//! Atomic store mutate-primitive integration tests: a Section mutate touches
//! only the sidecar (no other file), and a duplicate changelog append is
//! rejected (frozen ledger).

use mnemosyne_atomic::{
    append_changelog_entry, set_section_intent, AtomicStore, ChangelogEntryDraft,
};
use tempfile::TempDir;

#[test]
fn atomic_section_legacy_carry_unaffected() {
    // Atomic fields are *additive* — they don't mutate the legacy `body` field
    // on Section (which lives outside this store). Verify by setting atomic
    // fields and confirming the sidecar is the only side-effect (no other
    // file path is touched).
    let tmp = TempDir::new().unwrap();
    let sidecar = tmp.path().join("workspace.atomic.json");
    let mut store = AtomicStore::new();

    // Round 287 fail-loud: explicit Section creation.
    mnemosyne_atomic::add_section(
        &mut store,
        &sidecar,
        "43",
        "docs/GENERATED.md",
        "test",
        None,
    )
    .unwrap();
    set_section_intent(&mut store, &sidecar, "43", "atomic-only").unwrap();

    // Sidecar exists, no other files in tmp dir.
    let entries: Vec<_> = std::fs::read_dir(tmp.path())
        .unwrap()
        .filter_map(|r| r.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        entries,
        vec!["workspace.atomic.json"],
        "only sidecar should exist; legacy body / sub_bullets carry on existing markdown files"
    );
}

#[test]
fn atomic_changelog_entry_frozen_on_duplicate_append() {
    let tmp = TempDir::new().unwrap();
    let sidecar = tmp.path().join("workspace.atomic.json");
    let mut store = AtomicStore::new();

    append_changelog_entry(
        &mut store,
        &sidecar,
        ChangelogEntryDraft {
            entry_id: "Round 162",
            decision_summary: Some("first"),
            changes_bullets: &["change A".into()],
            verification_bullets: &["verify A".into()],
            impact_refs: &[],
            carry_forward_bullets: &[],
        },
    )
    .unwrap();

    let result = append_changelog_entry(
        &mut store,
        &sidecar,
        ChangelogEntryDraft {
            entry_id: "Round 162",
            decision_summary: Some("attempted overwrite"),
            changes_bullets: &[],
            verification_bullets: &[],
            impact_refs: &[],
            carry_forward_bullets: &[],
        },
    );
    assert!(
        result.is_err(),
        "second append to same entry_id must fail (T2 frozen ledger)"
    );
}
