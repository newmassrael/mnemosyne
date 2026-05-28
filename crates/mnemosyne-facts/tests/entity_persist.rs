//! Integration test — `TypedFactStore` against actual `mnemosyne-store` RocksDB,
//! Cross-language emit Jaccard — deterministic composite-key encoding.

use mnemosyne_facts::{
    canonical_identifier_set, design_doc_schema_fixture, emit_all_languages, jaccard_inclusion,
    sha256_hex, ChangelogEntryFact, CrossRefFact, DecisionStatus, FactKey, FrozenListFact,
    SectionFact, SectionSkeleton, TypedFactStore,
};
use mnemosyne_store::{encode_composite_key, MnemosyneStore};
use tempfile::TempDir;

fn fresh_store() -> (TempDir, MnemosyneStore) {
    let dir = TempDir::new().unwrap();
    let store = MnemosyneStore::open(dir.path()).unwrap();
    (dir, store)
}

#[test]
fn entity_put_get_round_trip_all_four_kinds() {
    let (_dir, store) = fresh_store();
    let typed = TypedFactStore::new(&store);

    let section = SectionFact {
        key: FactKey {
            branch_id: 1,
            entity_id: 1,
            valid_from: 100,
        },
        section_id: "39".to_string(),
        skeleton: SectionSkeleton {
            title: "Phase 0 design_doc schema".to_string(),
            parent_doc: "docs/DESIGN.md".to_string(),
            parent_section: None,
            decision_status: Some(DecisionStatus::Active),
        },
    };
    typed.put_section(&section).unwrap();
    assert_eq!(
        typed.get_section(1, 1, 100).unwrap().as_ref(),
        Some(&section)
    );

    let changelog = ChangelogEntryFact {
        key: FactKey {
            branch_id: 1,
            entity_id: 73,
            valid_from: 200,
        },
        round_number: 73,
        summary: "OPTION B-2 mnemosyne-store production".to_string(),
    };
    typed.put_changelog_entry(&changelog).unwrap();
    assert_eq!(
        typed.get_changelog_entry(1, 73, 200).unwrap().as_ref(),
        Some(&changelog)
    );

    let frozen_list = FrozenListFact {
        key: FactKey {
            branch_id: 1,
            entity_id: 200,
            valid_from: 300,
        },
        owner_section: 1, // Section entity_id from above
        frozen_round: 60,
        kind: "release_lock".to_string(),
    };
    typed.put_frozen_list(&frozen_list).unwrap();
    assert_eq!(
        typed.get_frozen_list(1, 200, 300).unwrap().as_ref(),
        Some(&frozen_list)
    );

    let cross_ref = CrossRefFact {
        branch_id: 1,
        from_section: 1,
        to_section: 1, // Self-reference for simplicity in this test.
        ref_kind: "decision".to_string(),
    };
    typed.put_cross_ref(&cross_ref).unwrap();
    assert_eq!(
        typed.get_cross_ref(1, 1, 1).unwrap().as_ref(),
        Some(&cross_ref)
    );
}

#[test]
fn composite_key_encoding_deterministic() {
    let a = encode_composite_key(1, 42, 1000);
    let b = encode_composite_key(1, 42, 1000);
    assert_eq!(a, b);
    // 24 B output, big-endian layout means lexicographic order matches numeric order.
    assert_eq!(a.len(), 24);
    let earlier = encode_composite_key(1, 42, 999);
    assert!(earlier < a);
}

#[test]
fn cross_language_emit_jaccard_inclusion_one() {
    let spec = design_doc_schema_fixture();
    let canonical = canonical_identifier_set(&spec);
    let m = emit_all_languages(&spec);
    for (lang, text) in [
        ("rust", &m.rust),
        ("kotlin", &m.kotlin),
        ("python", &m.python),
        ("cpp", &m.cpp),
        ("protobuf", &m.protobuf),
    ] {
        let j = jaccard_inclusion(text, &canonical);
        assert!(
            (j - 1.0).abs() < f64::EPSILON,
            "{lang} Jaccard = {j} (expected 1.0)"
        );
    }
}

#[test]
fn five_language_emit_distinct_sha256() {
    let spec = design_doc_schema_fixture();
    let m = emit_all_languages(&spec);
    let mut hashes = std::collections::BTreeSet::new();
    for s in [&m.rust, &m.kotlin, &m.python, &m.cpp, &m.protobuf] {
        let h = sha256_hex(s);
        assert_eq!(h.len(), 64);
        assert!(hashes.insert(h));
    }
    assert_eq!(hashes.len(), 5);
}

#[test]
fn re_open_preserves_typed_facts() {
    let dir = TempDir::new().unwrap();
    let section = SectionFact {
        key: FactKey {
            branch_id: 1,
            entity_id: 1,
            valid_from: 100,
        },
        section_id: "39".to_string(),
        skeleton: SectionSkeleton {
            title: "Persistent across reopen".to_string(),
            parent_doc: "docs/DESIGN.md".to_string(),
            parent_section: None,
            decision_status: Some(DecisionStatus::Active),
        },
    };
    {
        let store = MnemosyneStore::open(dir.path()).unwrap();
        let typed = TypedFactStore::new(&store);
        typed.put_section(&section).unwrap();
    }
    let store = MnemosyneStore::open(dir.path()).unwrap();
    let typed = TypedFactStore::new(&store);
    let recovered = typed.get_section(1, 1, 100).unwrap();
    assert_eq!(recovered.as_ref(), Some(&section));
}
