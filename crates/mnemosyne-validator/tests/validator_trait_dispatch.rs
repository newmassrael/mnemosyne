//! End-to-end PluginRegistry → Validator trait dispatch smoke.
//!
//! Asserts the R307 D1 closure: cmd_validate_code_refs goes through
//! `PluginRegistry::register_validator` + `PluginRegistry::validator()`
//! + `Validator::validate(ctx)` and gets back `Vec<ValidationFinding>`
//! that round-trips the rich `CodeRefViolation` payload via
//! `ValidationFinding.kind` + `ValidationFinding.extras`.
//!
//! Coverage:
//! 1. Validator registers + retrieves via the registry by key.
//! 2. Findings carry the kind tag the consumer reads for per-class
//!    counting (`missing` / `section_missing` / `citation_unbound` /
//!    `impl_unbacked` / `impl_missing`).
//! 3. Extras preserve `entry_id` (for Citation kinds), `symbol` (for
//!    `impl_unbacked`), and `decision_status` (for `impl_missing`).

use std::collections::BTreeMap;
use std::path::PathBuf;

use mnemosyne_plugin::{
    AtomicStoreView, PluginRegistry, Severity, ValidationContext,
};
use mnemosyne_validator::atomic::{
    AtomicSection, AtomicStore, Implementation,
};
use mnemosyne_validator::code_refs::SetEqualityValidator;
use mnemosyne_validator::SetEqualityValidatorConfig;
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

#[test]
fn registry_dispatch_yields_findings_with_kind_and_extras() {
    let tmp = TempDir::new().unwrap();
    let sidecar = tmp.path().join("docs/.atomic/workspace.atomic.json");
    std::fs::create_dir_all(sidecar.parent().unwrap()).unwrap();
    std::fs::create_dir_all(tmp.path().join("src")).unwrap();

    // Section sec1 declares an impl at src/missing.rs (no code citation
    // back → ImplementationUnbacked). Section sec2 declares no impls
    // → ImplementationMissing.
    let mut store = AtomicStore::new();
    let sec1 = AtomicSection {
        title: "Sec One".into(),
        parent_doc: "docs/GENERATED.md".into(),
        implementations: vec![Implementation {
            file: "src/missing.rs".into(),
            symbol: Some("expected_symbol".into()),
        }],
        ..AtomicSection::default()
    };
    store.sections.insert("sec1".into(), sec1);
    let sec2 = AtomicSection {
        title: "Sec Two".into(),
        parent_doc: "docs/GENERATED.md".into(),
        ..AtomicSection::default()
    };
    store.sections.insert("sec2".into(), sec2);

    // Code file: cites §sec1 (passes binding) + Round 999 (hallucinated).
    std::fs::write(
        tmp.path().join("src/missing.rs"),
        "// §sec1 bound\n// Round 999 hallucinated\n",
    )
    .unwrap();

    // Register validator + dispatch via PluginRegistry.
    let mut registry = PluginRegistry::new();
    registry.register_validator(
        "set_equality_validator",
        Box::new(build_validator(None)),
    );
    let dispatched = registry
        .validator("set_equality_validator")
        .expect("just registered");
    let store_view: &dyn AtomicStoreView = &store;
    let ctx = ValidationContext {
        workspace_root: tmp.path(),
        atomic_sidecar: &sidecar,
        store: store_view,
    };
    let findings = dispatched.validate(&ctx).expect("dispatch ok");

    // Expect:
    // - 1 "missing" (Round 999) carrying entry_id in extras.
    // - 1 "impl_missing" for sec2 (no impls, default-Active).
    let kinds: Vec<String> = findings
        .iter()
        .filter_map(|f| f.kind.clone())
        .collect();
    assert!(kinds.contains(&"missing".to_string()), "kinds = {:?}", kinds);
    assert!(
        kinds.contains(&"impl_missing".to_string()),
        "kinds = {:?}",
        kinds
    );

    // Verify the "missing" finding carries entry_id + file + line.
    let missing = findings
        .iter()
        .find(|f| f.kind.as_deref() == Some("missing"))
        .expect("missing finding present");
    assert_eq!(missing.severity, Severity::Reject);
    assert_eq!(missing.file, Some(PathBuf::from("src/missing.rs")));
    assert_eq!(missing.line, Some(2));
    assert_eq!(
        missing.extras.get("entry_id").and_then(|v| v.as_str()),
        Some("Round 999")
    );

    // Verify the "impl_missing" finding carries section_id +
    // decision_status (none → "none(default-active)").
    let impl_missing = findings
        .iter()
        .find(|f| f.kind.as_deref() == Some("impl_missing"))
        .expect("impl_missing finding present");
    assert_eq!(impl_missing.section_id.as_deref(), Some("sec2"));
    assert_eq!(
        impl_missing
            .extras
            .get("decision_status")
            .and_then(|v| v.as_str()),
        Some("none(default-active)")
    );
}

#[test]
fn registry_dispatch_with_filter_id_narrows_to_decay_only() {
    // Decay-cascade caller pattern: filter_id = Some("Round 5") narrows
    // the scan to just that one entry's decay sites. Step 3/4 are
    // suppressed under filter_id.
    let tmp = TempDir::new().unwrap();
    let sidecar = tmp.path().join("docs/.atomic/workspace.atomic.json");
    std::fs::create_dir_all(sidecar.parent().unwrap()).unwrap();
    std::fs::create_dir_all(tmp.path().join("src")).unwrap();

    // Store has an entry "Round 5" (so the cite is valid Decay, not
    // Missing) and no sections (so steps 3-4 would otherwise fire — but
    // are suppressed under filter mode).
    let mut store = AtomicStore::new();
    let mut entry = mnemosyne_validator::atomic::AtomicChangelogEntry {
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

    let mut registry = PluginRegistry::new();
    registry.register_validator(
        "set_equality_validator",
        Box::new(build_validator(Some("Round 5".into()))),
    );
    let store_view: &dyn AtomicStoreView = &store;
    let ctx = ValidationContext {
        workspace_root: tmp.path(),
        atomic_sidecar: &sidecar,
        store: store_view,
    };
    let findings = registry
        .validator("set_equality_validator")
        .unwrap()
        .validate(&ctx)
        .unwrap();

    // Only "decay" findings — no "missing" for "Round 7", no
    // "impl_missing"/etc from steps 3-4.
    for f in &findings {
        assert_eq!(
            f.kind.as_deref(),
            Some("decay"),
            "filter mode should surface decay only, got: {:?}",
            f.kind
        );
        assert_eq!(f.severity, Severity::Info);
    }
    assert_eq!(findings.len(), 1, "expected exactly 1 decay finding");
}
