//! Round 142 — workspace-config abstraction integration test.
//!
//! Two fixtures verify that the config-driven loader produces the same
//! same result as the legacy hardcoded path:
//!
//! 1. **Synthetic 3-doc fixture** — builds a temp `mnemosyne.toml` plus
//! three minimal markdown files, calls `discover_config` from a nested
//! subdir, and verifies the loaded config + workspace match expectation.
//! 2. **Self-application fixture** — points `discover_config` at the real
//! Mnemosyne repo root and verifies the config matches the 7-doc
//! workspace already documented in `docs/DESIGN.md` §66 Stage 1.
//!
//! Both share the same code path the CLI takes — proving the generic loader
//! is the *only* path (no hardcoded fallback consulted in production runs).

use mnemosyne_config::discover_config;
use mnemosyne_parser::parse_markdown;
use mnemosyne_validate::validator::cross_ref_orphan_reject_with_workspace;
use mnemosyne_workspace::Workspace;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

#[test]
fn synthetic_three_doc_fixture_loads_via_config() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    fs::create_dir_all(root.join("docs")).unwrap();

    fs::write(
        root.join("mnemosyne.toml"),
        r#"
[workspace]
docs = ["docs/SPEC.md", "docs/ARCH.md", "README.md"]
default_doc = "docs/SPEC.md"
"#,
    )
    .unwrap();

    fs::write(
        root.join("docs/SPEC.md"),
        "# Spec\n\n## 1. Foundations\n\nBody.\n",
    )
    .unwrap();
    fs::write(
        root.join("docs/ARCH.md"),
        "# Architecture\n\n## A. Overview\n\nSee §1.\n",
    )
    .unwrap();
    fs::write(root.join("README.md"), "# Project\n\nWelcome.\n").unwrap();

    // Discover from a nested subdir to exercise the upward walk.
    let nested = root.join("docs");
    let loaded = discover_config(&nested).unwrap().expect("config found");

    assert_eq!(
        loaded.config.workspace.docs,
        vec!["docs/SPEC.md", "docs/ARCH.md", "README.md"]
    );
    assert_eq!(
        loaded.config.workspace.default_doc.as_deref(),
        Some("docs/SPEC.md")
    );
    assert_eq!(
        loaded.workspace_root.canonicalize().unwrap(),
        root.canonicalize().unwrap()
    );

    // Build a workspace from the config and verify cross-ref resolution
    // routes through the configured default_doc rather than a hardcoded one.
    let mut ws = Workspace::from_config(&loaded);
    for path in loaded.doc_paths() {
        let abs = loaded.doc_abs_path(path);
        let content = fs::read_to_string(&abs).unwrap();
        let parsed = parse_markdown(&content, path);
        ws.insert(path.to_string(), parsed);
    }
    assert_eq!(ws.default_doc.as_deref(), Some("docs/SPEC.md"));
    assert_eq!(ws.docs.len(), 3);

    // ARCH.md's "See §1" must reclassify to docs/SPEC.md#§1 via the configured default_doc.
    let arch_orphans =
        cross_ref_orphan_reject_with_workspace(ws.docs.get("docs/ARCH.md").unwrap(), &ws);
    assert!(
        arch_orphans.is_empty(),
        "§1 reference must reclassify to default_doc, got orphans: {:?}",
        arch_orphans
    );
}

#[test]
fn mnemosyne_self_application_config_matches_canonical_workspace() {
    let root = repo_root();
    let loaded = discover_config(&root)
        .unwrap()
        .expect("repo root must contain mnemosyne.toml");

    // Round 251 — post 7-md deletion the workspace collapses to GENERATED.md
    // alone (atomic store = sole source of truth, GENERATED.md = sole
    // readable artifact). Pre-deletion this enumerated 7 design_doc paths
    // and asserted default_doc = "docs/DESIGN.md".
    let mut docs: Vec<&str> = loaded.doc_paths().collect();
    docs.sort();
    let expected = vec!["docs/GENERATED.md"];
    assert_eq!(docs, expected);

    assert_eq!(
        loaded.config.workspace.default_doc.as_deref(),
        Some("docs/GENERATED.md")
    );
    assert_eq!(
        loaded.workspace_root.canonicalize().unwrap(),
        root.canonicalize().unwrap()
    );
}

#[test]
fn config_driven_workspace_default_doc_drives_cross_ref_resolution() {
    // Verifies the contract: Workspace::from_config + insert + reclassify
    // is the same path the CLI takes. If the config's default_doc changes,
    // cross-doc reclassify follows it — no hardcoded "docs/DESIGN.md" leaks.
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    fs::create_dir_all(root.join("adr")).unwrap();

    fs::write(
        root.join("mnemosyne.toml"),
        r#"
[workspace]
docs = ["adr/0001.md", "README.md"]
default_doc = "adr/0001.md"
"#,
    )
    .unwrap();
    fs::write(
        root.join("adr/0001.md"),
        "# ADR 0001\n\n## 7. Authentication\n\nDecision body.\n",
    )
    .unwrap();
    fs::write(
        root.join("README.md"),
        "# Project\n\n## summary\n\nPer §7 we authenticate via JWT.\n",
    )
    .unwrap();

    let loaded = discover_config(root).unwrap().expect("config");
    let mut ws = Workspace::from_config(&loaded);
    for path in loaded.doc_paths() {
        let content = fs::read_to_string(loaded.doc_abs_path(path)).unwrap();
        let parsed = parse_markdown(&content, path);
        ws.insert(path.to_string(), parsed);
    }

    // README's §7 should reclassify to adr/0001.md#§7 because the config
    // names adr/0001.md as default_doc — proving the config drives the
    // resolver, not any hardcoded fallback.
    let readme = ws.docs.get("README.md").unwrap();
    let orphans = cross_ref_orphan_reject_with_workspace(readme, &ws);
    assert!(
        orphans.is_empty(),
        "§7 must reclassify via configured default_doc, got: {:?}",
        orphans
    );
}
