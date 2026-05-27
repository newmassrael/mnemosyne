//! Round 306 — RFC-002 FR-3 symbol-level enforcement smoke tests.
//!
//! Verifies the three end-to-end paths of `scan_paths_bidirectional` with
//! a `SymbolResolver` registry attached:
//! 1. **Happy path** — `Implementation.symbol` matches the resolver's
//!    `resolve_symbol_at(file, citation_line)` answer; 0 SymbolMismatch.
//! 2. **Mismatch path** — resolver returns a different symbol from the
//!    one recorded in `§<id>.implementations[file=...]`; one
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

use mnemosyne_plugin::SymbolResolver;
use mnemosyne_plugin_tree_sitter_rust::TreesitterRustResolver;
use mnemosyne_validator::{
    atomic::{add_section, add_section_implementation, AtomicStore},
    code_refs::{scan_paths_bidirectional, CodeRefViolation, ViolationKind},
};
use tempfile::TempDir;

fn rust_resolver_map() -> BTreeMap<String, Box<dyn SymbolResolver>> {
    let mut m: BTreeMap<String, Box<dyn SymbolResolver>> = BTreeMap::new();
    m.insert("rust".into(), Box::new(TreesitterRustResolver));
    m
}

/// Stand up a minimal workspace with one Section that declares an
/// implementation at `src/foo.rs` and an optional `Implementation.symbol`.
/// Writes `src/foo.rs` carrying a `§<section_id>` citation at line 1 that
/// sits over a top-level `fn` named `expected_symbol_at_line_1`.
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
    add_section_implementation(
        &mut store,
        &sidecar,
        "sec1",
        "src/foo.rs",
        expected_symbol_in_spec,
    )
    .unwrap();

    std::fs::create_dir_all(tmp.path().join("src")).unwrap();
    // line 1: fn definition (so the symbol enclosing the cite resolves).
    // line 2: cite inside the body — `resolve_symbol_at(file, 2)` returns
    //         the enclosing fn's name.
    // line 3: body filler. line 4: close brace.
    let src = format!(
        "fn {}() {{\n    // see §sec1\n    let _ = 1;\n}}\n",
        body_symbol_name
    );
    std::fs::write(tmp.path().join("src/foo.rs"), src).unwrap();
    (tmp, store)
}

#[test]
fn happy_path_symbol_matches_no_mismatch_violation() {
    let (tmp, store) = stand_up(Some("alpha"), "alpha");
    let resolvers = rust_resolver_map();
    let v = scan_paths_bidirectional(
        tmp.path(),
        &["src/".to_string()],
        "Round ",
        &store,
        &[],
        None,
        true,
        &[],
        &[],
        &[],
        &[],
        Some(&resolvers),
    )
    .unwrap();
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
    let resolvers = rust_resolver_map();
    let v = scan_paths_bidirectional(
        tmp.path(),
        &["src/".to_string()],
        "Round ",
        &store,
        &[],
        None,
        true,
        &[],
        &[],
        &[],
        &[],
        Some(&resolvers),
    )
    .unwrap();
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
    let v = scan_paths_bidirectional(
        tmp.path(),
        &["src/".to_string()],
        "Round ",
        &store,
        &[],
        None,
        true,
        &[],
        &[],
        &[],
        &[],
        None,
    )
    .unwrap();
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
    let resolvers = rust_resolver_map();
    let v = scan_paths_bidirectional(
        tmp.path(),
        &["src/".to_string()],
        "Round ",
        &store,
        &[],
        None,
        true,
        &[],
        &[],
        &[],
        &[],
        Some(&resolvers),
    )
    .unwrap();
    assert!(v.iter().all(|x| !matches!(
        x,
        CodeRefViolation::Citation {
            kind: ViolationKind::SymbolMismatch,
            ..
        }
    )));
}
