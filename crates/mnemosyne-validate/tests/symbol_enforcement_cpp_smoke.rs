//! RFC-002 FR-3 symbol-level enforcement smoke tests — C++ backend.
//!
//! Mirror of `symbol_enforcement_smoke.rs` (Rust backend) using the
//! production `tree-sitter-cpp` resolver the CLI wires for the `cpp`
//! language key. Verifies the same three end-to-end paths against a C++
//! source file:
//! 1. **Happy path** — `Implementation.symbol` matches the resolver's
//!    `resolve_symbol_at(file, citation_line)` answer; 0 SymbolMismatch.
//! 2. **Mismatch path** — resolver returns a different symbol from the one
//!    recorded; one `CodeRefViolation::Citation { SymbolMismatch }`.
//! 3. **Opt-out path** — no resolver registered for the file's language;
//!    file-only set-equality applies, 0 SymbolMismatch.

use std::collections::BTreeMap;
use std::path::PathBuf;

use mnemosyne_atomic::{add_section, add_section_binding, AtomicStore, BindingKind};
use mnemosyne_config::SetEqualityValidatorConfig;
use mnemosyne_core::{AtomicStoreView, SymbolResolver};
use mnemosyne_plugin_tree_sitter_cpp::TreesitterCppResolver;
use mnemosyne_validate::code_refs::{CodeRefViolation, SetEqualityValidator, ViolationKind};
use tempfile::TempDir;

fn cpp_resolver_map() -> BTreeMap<String, Box<dyn SymbolResolver>> {
    let mut m: BTreeMap<String, Box<dyn SymbolResolver>> = BTreeMap::new();
    m.insert("cpp".into(), Box::new(TreesitterCppResolver));
    m
}

/// Stand up a minimal workspace with one Section declaring an
/// implementation at `src/foo.cpp` and an optional `Implementation.symbol`.
/// Writes `src/foo.cpp` carrying a `§<section_id>` citation at line 2 that
/// sits inside a top-level `int body_symbol_name() { ... }`.
fn stand_up(
    expected_symbol_in_spec: Option<&str>,
    body_symbol_name: &str,
) -> (TempDir, AtomicStore) {
    let tmp = TempDir::new().unwrap();
    let sidecar = tmp.path().join("docs/.atomic/workspace.atomic.json");
    std::fs::create_dir_all(sidecar.parent().unwrap()).unwrap();
    let mut store = AtomicStore::new();
    add_section(
        &mut store,
        &sidecar,
        "sec1",
        "docs/GENERATED.md",
        "Sec One",
        None,
    )
    .unwrap();
    add_section_binding(
        &mut store,
        &sidecar,
        "sec1",
        "src/foo.cpp",
        expected_symbol_in_spec,
        BindingKind::Implements,
    )
    .unwrap();

    std::fs::create_dir_all(tmp.path().join("src")).unwrap();
    // line 1: function definition; line 2: cite inside the body, so
    // `resolve_symbol_at(file, 2)` returns the enclosing function name.
    let src = format!(
        "int {}() {{\n    // see §sec1\n    return 1;\n}}\n",
        body_symbol_name
    );
    std::fs::write(tmp.path().join("src/foo.cpp"), src).unwrap();
    (tmp, store)
}

fn build_validator(
    symbol_resolvers: BTreeMap<String, Box<dyn SymbolResolver>>,
) -> SetEqualityValidator {
    SetEqualityValidator {
        config: SetEqualityValidatorConfig {
            paths: vec!["src/".to_string()],
            severity_missing: mnemosyne_config::Severity::Reject,
            severity_binding: mnemosyne_config::Severity::Reject,
            severity_coverage: None,
            severity_verification: None,
            severity_confirmation: None,
            severity_classification: None,
            severity_blanket: None,
            severity_prose_fact_assertion: None,
            severity_inventory: mnemosyne_config::Severity::Reject,
            comment_only: true,
            inventory_prefixes: vec![],
            external_section_prefixes: vec![],
            external_section_prefixes_bare: vec![],
            inventory_path_prefixes: vec![],
            section_namespace: None,
        },
        entry_id_prefix: "Round ".to_string(),
        orphan_ledger: vec![],
        symbol_resolvers,
        filter_id: None,
    }
}

fn symbol_mismatches(v: &[CodeRefViolation]) -> Vec<&CodeRefViolation> {
    v.iter()
        .filter(|x| {
            matches!(
                x,
                CodeRefViolation::Citation {
                    kind: ViolationKind::SymbolMismatch,
                    ..
                }
            )
        })
        .collect()
}

#[test]
fn happy_path_symbol_matches_no_mismatch_violation() {
    let (tmp, store) = stand_up(Some("alpha"), "alpha");
    let validator = build_validator(cpp_resolver_map());
    let snapshot = store.snapshot();
    let v = validator.scan(tmp.path(), &snapshot).unwrap();
    let mismatches = symbol_mismatches(&v);
    assert!(
        mismatches.is_empty(),
        "expected 0 SymbolMismatch, got: {:?}",
        mismatches
    );
}

#[test]
fn mismatch_path_surfaces_symbol_mismatch_violation() {
    // Spec says `alpha`, body actually defines `beta` at the cited line.
    let (tmp, store) = stand_up(Some("alpha"), "beta");
    let validator = build_validator(cpp_resolver_map());
    let snapshot = store.snapshot();
    let v = validator.scan(tmp.path(), &snapshot).unwrap();
    let mismatches = symbol_mismatches(&v);
    assert_eq!(
        mismatches.len(),
        1,
        "expected exactly 1 SymbolMismatch; got: {:?}",
        v
    );
    if let Some(CodeRefViolation::Citation { citation, .. }) = mismatches.first() {
        assert_eq!(citation.file, PathBuf::from("src/foo.cpp"));
        assert_eq!(citation.line, 2);
        assert_eq!(citation.entry_id, "§sec1");
    }
}

#[test]
fn opt_out_path_no_resolver_passes_file_only_setequality() {
    // Same setup as the mismatch path, BUT no resolver provided. File-level
    // binding passes; without a resolver the symbol axis is silent.
    let (tmp, store) = stand_up(Some("alpha"), "beta");
    let validator = build_validator(BTreeMap::new());
    let snapshot = store.snapshot();
    let v = validator.scan(tmp.path(), &snapshot).unwrap();
    assert!(
        symbol_mismatches(&v).is_empty(),
        "opt-out (no resolver) must surface 0 SymbolMismatch"
    );
}
