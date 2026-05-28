//! End-to-end Validator + ErasedValidator two-tier dispatch smoke.
//!
//! Asserts both paths land in production:
//! 1. `Validator::validate(&ctx)` returns typed `Vec<Self::Finding>`
//!    — pattern-match on `CodeRefViolation` variants for assertions.
//! 2. `PluginRegistry::register_validator(Box::new(v))` + retrieve via
//!    `registry.validator(key)` → `ErasedValidator::validate_erased(&ctx)`
//!    returns `Vec<serde_json::Value>` (typed findings serialized at
//!    the object-safe trait boundary).
//!
//! Coverage:
//! - Typed path preserves full enum shape (Citation { citation, kind } /
//!   ImplementationMissing { section_id, decision_status } / etc).
//! - Erased path serializes each finding via the auto-derived
//!   `CodeRefViolation: Serialize` impl and routes through
//!   `ValidatorError::Internal` if serialization ever fails.

use std::collections::BTreeMap;
use std::path::PathBuf;

use mnemosyne_atomic::{AtomicSection, AtomicStore, Implementation};
use mnemosyne_config::SetEqualityValidatorConfig;
use mnemosyne_core::{AtomicStoreView, PluginRegistry, ValidationContext, Validator};
use mnemosyne_validate::code_refs::{CodeRefViolation, SetEqualityValidator, ViolationKind};
use tempfile::TempDir;

fn build_validator(filter_id: Option<String>) -> SetEqualityValidator {
    SetEqualityValidator {
        config: SetEqualityValidatorConfig {
            paths: vec!["src/".into()],
            severity_missing: "reject".into(),
            severity_binding: "reject".into(),
            severity_inventory: "reject".into(),
            comment_only: true,
            inventory_prefixes: vec![],
            external_section_prefixes: vec![],
            external_section_prefixes_bare: vec![],
            inventory_path_prefixes: vec![],
        },
        entry_id_prefix: "Round ".to_string(),
        orphan_ledger: vec![],
        symbol_resolvers: BTreeMap::new(),
        filter_id,
    }
}

fn seed_workspace_for_unbacked_and_missing() -> (TempDir, AtomicStore) {
    let tmp = TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join("src")).unwrap();
    let mut store = AtomicStore::new();
    // sec1 declares an impl at src/missing.rs (no §sec1 cite in that file
    // — but a Round 999 hallucinated cite — triggers ImplementationUnbacked
    // ... actually src/missing.rs DOES cite §sec1, so binding holds for
    // sec1; the unbacked variant covers the other-section case below).
    let sec1 = AtomicSection {
        skeleton: mnemosyne_core::SectionSkeleton {
            title: "Sec One".into(),
            parent_doc: "docs/GENERATED.md".into(),
            ..Default::default()
        },
        implementations: vec![Implementation {
            file: "src/missing.rs".into(),
            symbol: Some("expected_symbol".into()),
        }],
        ..AtomicSection::default()
    };
    store.sections.insert("sec1".into(), sec1);
    // sec2 declares no impls → ImplementationMissing (default-Active).
    let sec2 = AtomicSection {
        skeleton: mnemosyne_core::SectionSkeleton {
            title: "Sec Two".into(),
            parent_doc: "docs/GENERATED.md".into(),
            ..Default::default()
        },
        ..AtomicSection::default()
    };
    store.sections.insert("sec2".into(), sec2);
    // Code file: cites §sec1 (passes binding) + Round 999 (hallucinated).
    std::fs::write(
        tmp.path().join("src/missing.rs"),
        "// §sec1 bound\n// Round 999 hallucinated\n",
    )
    .unwrap();
    (tmp, store)
}

#[test]
fn typed_dispatch_yields_typed_findings() {
    let (tmp, store) = seed_workspace_for_unbacked_and_missing();
    let sidecar = tmp.path().join("docs/.atomic/workspace.atomic.json");
    std::fs::create_dir_all(sidecar.parent().unwrap()).unwrap();

    let validator = build_validator(None);
    let store_view: &dyn AtomicStoreView = &store;
    let ctx = ValidationContext {
        workspace_root: tmp.path(),
        atomic_sidecar: &sidecar,
        store: store_view,
    };
    let violations =
        <SetEqualityValidator as Validator>::validate(&validator, &ctx).expect("typed dispatch ok");

    // Expect a Citation { Missing } for Round 999 + an
    // ImplementationMissing for sec2.
    let missing = violations
        .iter()
        .find(|v| {
            matches!(
                v,
                CodeRefViolation::Citation {
                    kind: ViolationKind::Missing,
                    ..
                }
            )
        })
        .expect("missing citation present");
    if let CodeRefViolation::Citation { citation, .. } = missing {
        assert_eq!(citation.entry_id, "Round 999");
        assert_eq!(citation.file, PathBuf::from("src/missing.rs"));
        assert_eq!(citation.line, 2);
    } else {
        panic!("expected Citation variant");
    }

    let impl_missing = violations
        .iter()
        .find(|v| matches!(v, CodeRefViolation::ImplementationMissing { .. }))
        .expect("impl_missing present");
    if let CodeRefViolation::ImplementationMissing {
        section_id,
        decision_status,
    } = impl_missing
    {
        assert_eq!(section_id, "sec2");
        assert!(decision_status.is_none(), "default-Active fallback path");
    } else {
        panic!("expected ImplementationMissing variant");
    }
}

#[test]
fn erased_dispatch_via_registry_serializes_findings_to_json() {
    let (tmp, store) = seed_workspace_for_unbacked_and_missing();
    let sidecar = tmp.path().join("docs/.atomic/workspace.atomic.json");
    std::fs::create_dir_all(sidecar.parent().unwrap()).unwrap();

    let mut registry = PluginRegistry::new();
    registry.register_validator("set_equality_validator", Box::new(build_validator(None)));
    let dispatched = registry
        .validator("set_equality_validator")
        .expect("just registered");
    let store_view: &dyn AtomicStoreView = &store;
    let ctx = ValidationContext {
        workspace_root: tmp.path(),
        atomic_sidecar: &sidecar,
        store: store_view,
    };
    let json_values = dispatched
        .validate_erased(&ctx)
        .expect("erased dispatch ok");

    // Each erased finding round-trips back into CodeRefViolation via
    // the auto-derived serde shape (default externally-tagged enum).
    // Verify the discriminator-style JSON carries the variant names
    // SetEqualityValidator emits.
    let has_citation_missing = json_values.iter().any(|v| {
        v.get("Citation")
            .and_then(|c| c.get("kind"))
            .and_then(|k| k.as_str())
            == Some("Missing")
    });
    let has_impl_missing = json_values
        .iter()
        .any(|v| v.get("ImplementationMissing").is_some());
    assert!(
        has_citation_missing,
        "expected Citation/Missing finding in erased output: {:#?}",
        json_values
    );
    assert!(
        has_impl_missing,
        "expected ImplementationMissing finding in erased output: {:#?}",
        json_values
    );
}

#[test]
fn typed_dispatch_filter_id_narrows_to_decay_only() {
    // Decay-cascade caller pattern: filter_id = Some("Round 5") narrows
    // the scan to just that one entry's decay sites. Step 3/4 are
    // suppressed under filter_id.
    let tmp = TempDir::new().unwrap();
    let sidecar = tmp.path().join("docs/.atomic/workspace.atomic.json");
    std::fs::create_dir_all(sidecar.parent().unwrap()).unwrap();
    std::fs::create_dir_all(tmp.path().join("src")).unwrap();

    let mut store = AtomicStore::new();
    let mut entry = mnemosyne_atomic::AtomicChangelogEntry {
        decision_summary: Some("Round 5 anchor for decay test".into()),
        ..Default::default()
    };
    entry.clone_audit_into_publishable();
    store.changelog_entries.insert("Round 5".into(), entry);

    std::fs::write(
        tmp.path().join("src/a.rs"),
        "// Round 5 decay target\n// Round 7 other\n",
    )
    .unwrap();

    let validator = build_validator(Some("Round 5".into()));
    let store_view: &dyn AtomicStoreView = &store;
    let ctx = ValidationContext {
        workspace_root: tmp.path(),
        atomic_sidecar: &sidecar,
        store: store_view,
    };
    let violations =
        <SetEqualityValidator as Validator>::validate(&validator, &ctx).expect("typed dispatch ok");

    assert_eq!(violations.len(), 1, "expected exactly 1 decay finding");
    let decay = &violations[0];
    assert!(
        matches!(
            decay,
            CodeRefViolation::Citation {
                kind: ViolationKind::Decay,
                ..
            }
        ),
        "filter mode should surface decay only, got: {:?}",
        decay
    );
}
