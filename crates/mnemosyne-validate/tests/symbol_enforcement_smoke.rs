//! RFC-002 FR-3 symbol-level enforcement smoke tests.
//!
//! Verifies the three end-to-end paths of `SetEqualityValidator::scan`
//! with a `SymbolResolver` registry attached:
//! 1. **Happy path** — `Implementation.symbol` matches the resolver's
//!    `resolve_symbol_at(file, citation_line)` answer; 0 SymbolMismatch.
//! 2. **Mismatch path** — resolver returns a different symbol from the
//!    one recorded in `§<id>.bindings[file=...]`; one
//!    `CodeRefViolation::Citation { kind: SymbolMismatch }` surfaces.
//! 3. **Opt-out path** — no resolver registered for the file's language
//!    (or no `SymbolResolver` at all); file-only set-equality applies,
//!    0 SymbolMismatch regardless of `Implementation.symbol` content.
//!
//! Each scenario is built on the existing `code_refs` `tests/` fixture
//! pattern: a workspace with one section, one citation, one
//! implementation entry. Resolver = the production
//! `tree-sitter-rust` backend (the same one the CLI wires).

use std::collections::BTreeMap;
use std::path::PathBuf;

use mnemosyne_atomic::{add_section, add_section_binding, AtomicStore, BindingKind};
use mnemosyne_config::SetEqualityValidatorConfig;
use mnemosyne_core::{AtomicStoreView, SymbolResolver};
use mnemosyne_plugin_tree_sitter_rust::TreesitterRustResolver;
use mnemosyne_validate::code_refs::{CodeRefViolation, SetEqualityValidator, ViolationKind};
use tempfile::TempDir;

fn rust_resolver_map() -> BTreeMap<String, Box<dyn SymbolResolver>> {
    let mut m: BTreeMap<String, Box<dyn SymbolResolver>> = BTreeMap::new();
    m.insert("rust".into(), Box::new(TreesitterRustResolver));
    m
}

/// Stand up a minimal workspace with one Section that declares an
/// implementation at `src/foo.rs` and an optional `Implementation.symbol`.
/// Writes `src/foo.rs` carrying a `§<section_id>` citation at line 2
/// that sits inside a top-level `fn body_symbol_name() { ... }`.
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
        "src/foo.rs",
        expected_symbol_in_spec,
        BindingKind::Implements,
    )
    .unwrap();

    std::fs::create_dir_all(tmp.path().join("src")).unwrap();
    // line 1: fn definition (so the symbol enclosing the cite resolves).
    // line 2: cite inside the body — `resolve_symbol_at(file, 2)` returns
    //         the enclosing fn's name.
    let src = format!(
        "fn {}() {{\n    // see §sec1\n    let _ = 1;\n}}\n",
        body_symbol_name
    );
    std::fs::write(tmp.path().join("src/foo.rs"), src).unwrap();
    (tmp, store)
}

/// Builds a `SetEqualityValidator` matching the pre-R307
/// `scan_paths_bidirectional` invocation shape used by these smoke
/// tests — paths = `src/`, comment_only = true, no orphan ledger / no
/// inventory / no external prefix axes, no filter.
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

#[test]
fn happy_path_symbol_matches_no_mismatch_violation() {
    let (tmp, store) = stand_up(Some("alpha"), "alpha");
    let validator = build_validator(rust_resolver_map());
    let snapshot = store.snapshot();
    let v = validator.scan(tmp.path(), &snapshot).unwrap();
    let mismatches: Vec<_> = v
        .iter()
        .filter(|x| {
            matches!(
                x,
                CodeRefViolation::Citation {
                    kind: ViolationKind::SymbolMismatch,
                    ..
                }
            )
        })
        .collect();
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
    let validator = build_validator(rust_resolver_map());
    let snapshot = store.snapshot();
    let v = validator.scan(tmp.path(), &snapshot).unwrap();
    let mismatches: Vec<_> = v
        .iter()
        .filter(|x| {
            matches!(
                x,
                CodeRefViolation::Citation {
                    kind: ViolationKind::SymbolMismatch,
                    ..
                }
            )
        })
        .collect();
    assert_eq!(
        mismatches.len(),
        1,
        "expected exactly 1 SymbolMismatch; got: {:?}",
        v
    );
    // Verify the violation carries the citation file + line.
    if let Some(CodeRefViolation::Citation { citation, .. }) = mismatches.first() {
        assert_eq!(citation.file, PathBuf::from("src/foo.rs"));
        assert_eq!(citation.line, 2);
        assert_eq!(citation.entry_id, "§sec1");
    }
}

#[test]
fn opt_out_path_no_resolver_passes_file_only_setequality() {
    // Same setup as the mismatch path, BUT no resolver provided. R260
    // file-level binding passes (impl.file = src/foo.rs registered), and
    // without a resolver the symbol axis is silent. 0 SymbolMismatch.
    let (tmp, store) = stand_up(Some("alpha"), "beta");
    let validator = build_validator(BTreeMap::new());
    let snapshot = store.snapshot();
    let v = validator.scan(tmp.path(), &snapshot).unwrap();
    let mismatches: Vec<_> = v
        .iter()
        .filter(|x| {
            matches!(
                x,
                CodeRefViolation::Citation {
                    kind: ViolationKind::SymbolMismatch,
                    ..
                }
            )
        })
        .collect();
    assert!(
        mismatches.is_empty(),
        "opt-out (no resolver) must surface 0 SymbolMismatch, got: {:?}",
        mismatches
    );
}

#[test]
fn no_symbol_in_impl_skips_axis_even_with_resolver() {
    // Spec records the implementation entry WITHOUT a symbol field.
    // Resolver registered, but axis is opt-in per (file, symbol) pair —
    // missing symbol = file-only enforcement = 0 SymbolMismatch.
    let (tmp, store) = stand_up(None, "anything");
    let validator = build_validator(rust_resolver_map());
    let snapshot = store.snapshot();
    let v = validator.scan(tmp.path(), &snapshot).unwrap();
    assert!(v.iter().all(|x| !matches!(
        x,
        CodeRefViolation::Citation {
            kind: ViolationKind::SymbolMismatch,
            ..
        }
    )));
}

#[test]
fn set_membership_multiple_symbols_one_file() {
    // A section legitimately realized by more than one symbol in a file:
    // §sec1 records implementation symbols `alpha` and `beta` (two entries,
    // same file). The file cites §sec1 inside `alpha`, `beta`, and an
    // unregistered `gamma`. Set-membership: the alpha/beta cites are bound
    // (member of {alpha, beta}); only the gamma cite drifts → 1 mismatch.
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
        "src/foo.rs",
        Some("alpha"),
        BindingKind::Implements,
    )
    .unwrap();
    add_section_binding(
        &mut store,
        &sidecar,
        "sec1",
        "src/foo.rs",
        Some("beta"),
        BindingKind::Implements,
    )
    .unwrap();

    std::fs::create_dir_all(tmp.path().join("src")).unwrap();
    // Cites sit on lines 2 (alpha), 5 (beta), 8 (gamma).
    let src = "fn alpha() {\n    // see §sec1\n}\nfn beta() {\n    // see §sec1\n}\nfn gamma() {\n    // see §sec1\n}\n";
    std::fs::write(tmp.path().join("src/foo.rs"), src).unwrap();

    let validator = build_validator(rust_resolver_map());
    let snapshot = store.snapshot();
    let v = validator.scan(tmp.path(), &snapshot).unwrap();
    let mismatches: Vec<_> = v
        .iter()
        .filter(|x| {
            matches!(
                x,
                CodeRefViolation::Citation {
                    kind: ViolationKind::SymbolMismatch,
                    ..
                }
            )
        })
        .collect();
    assert_eq!(
        mismatches.len(),
        1,
        "only the unregistered `gamma` cite should mismatch; got: {:?}",
        v
    );
    if let Some(CodeRefViolation::Citation { citation, .. }) = mismatches.first() {
        assert_eq!(citation.line, 8, "the gamma cite is on line 8");
    }
}
