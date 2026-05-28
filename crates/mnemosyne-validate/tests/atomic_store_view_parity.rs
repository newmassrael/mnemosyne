//! `AtomicStoreView` parity smoke — asserts the trait-level snapshot
//! reads back fields *consumers* (SetEqualityValidator) see (changelog
//! ids, section ids with implied parents, per-section impl tuples,
//! inventory status).
//!
//! Lives in `mnemosyne-validator/tests/` because that crate hosts the
//! `impl AtomicStoreView for AtomicStore`. Test exercises the end-to-end
//! round trip (atomic store fields → snapshot → indices the validator
//! consumes).

use mnemosyne_core::{AtomicStoreView, DecisionStatus, InventoryStatus};
use mnemosyne_atomic::{AtomicChangelogEntry, AtomicSection, AtomicStore, Implementation, InventoryEntry};

fn build_store() -> AtomicStore {
    let mut store = AtomicStore::new();

    // Section sec1 — Active by default (decision_status None), one impl
    // with a symbol.
    let sec1 = AtomicSection {
        title: "Sec One".into(),
        parent_doc: "docs/GENERATED.md".into(),
        implementations: vec![Implementation {
            file: "src/foo.rs".into(),
            symbol: Some("foo_symbol".into()),
        }],
        ..AtomicSection::default()
    };
    store.sections.insert("sec1".into(), sec1);

    // Section sec2/sub — parented + superseded; zero impls.
    let sec2sub = AtomicSection {
        title: "Sec Two Sub".into(),
        parent_doc: "docs/GENERATED.md".into(),
        parent_section: Some("sec2".into()),
        decision_status: Some(DecisionStatus::Superseded),
        ..AtomicSection::default()
    };
    store.sections.insert("sec2/sub".into(), sec2sub);

    // Changelog entry.
    let mut entry = AtomicChangelogEntry {
        decision_summary: Some("Test entry — snapshot parity".into()),
        ..AtomicChangelogEntry::default()
    };
    entry.clone_audit_into_publishable();
    store.changelog_entries.insert("Round 999".into(), entry);

    // Inventory: Active + Deprecated.
    store.inventory_entries.insert(
        "INV_ACTIVE_01".into(),
        InventoryEntry {
            status: InventoryStatus::Active,
            section_ref: None,
            source: None,
            reason: None,
        },
    );
    store.inventory_entries.insert(
        "INV_DEPR_01".into(),
        InventoryEntry {
            status: InventoryStatus::Deprecated,
            section_ref: None,
            source: None,
            reason: None,
        },
    );

    store
}

#[test]
fn snapshot_changelog_entry_ids_match_keys() {
    let store = build_store();
    let snapshot = store.snapshot();
    assert!(snapshot.changelog_entry_ids.contains("Round 999"));
    assert_eq!(snapshot.changelog_entry_ids.len(), 1);
}

#[test]
fn snapshot_section_ids_include_implied_parents() {
    let store = build_store();
    let snapshot = store.snapshot();
    assert!(snapshot.section_ids_with_implied_parents.contains("sec1"));
    assert!(snapshot.section_ids_with_implied_parents.contains("sec2/sub"));
    // Implied parent prefix derived from `/` split — atomic_section_id_set
    // parity (R287 carry; mirrored by AtomicStoreView::snapshot).
    assert!(
        snapshot.section_ids_with_implied_parents.contains("sec2"),
        "implied parent `sec2` must be present in snapshot section id set"
    );
}

#[test]
fn snapshot_section_view_carries_implementations_and_status() {
    let store = build_store();
    let snapshot = store.snapshot();

    let sec1 = snapshot.sections.get("sec1").expect("sec1 present");
    assert_eq!(sec1.implementations.len(), 1);
    assert_eq!(sec1.implementations[0].file, "src/foo.rs");
    assert_eq!(sec1.implementations[0].symbol.as_deref(), Some("foo_symbol"));
    // No explicit decision_status set on sec1 → None (consumer applies
    // default-Active fallback at use site).
    assert_eq!(sec1.decision_status, None);

    let sec2 = snapshot.sections.get("sec2/sub").expect("sec2/sub present");
    assert!(sec2.implementations.is_empty());
    assert_eq!(sec2.decision_status, Some(DecisionStatus::Superseded));
}

#[test]
fn snapshot_inventory_carries_status_view() {
    let store = build_store();
    let snapshot = store.snapshot();

    assert_eq!(
        snapshot.inventory.get("INV_ACTIVE_01").copied(),
        Some(InventoryStatus::Active)
    );
    assert_eq!(
        snapshot.inventory.get("INV_DEPR_01").copied(),
        Some(InventoryStatus::Deprecated)
    );
    assert!(!snapshot.inventory.contains_key("INV_UNKNOWN"));
}
