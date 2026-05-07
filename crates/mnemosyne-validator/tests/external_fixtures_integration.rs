//! Round 147 — external project fixture integration test.
//!
//! Three synthetic external-project fixtures verify that the same
//! Mnemosyne codebase + a project-specific `mnemosyne.toml` produces
//! correct round-trip / cross-doc reclassify / changelog parsing under
//! convention sets that differ from the Mnemosyne self-application:
//!
//! - **fixture A (ADR-style)**: `docs/adr/ADR-0001.md` + `ADR-0002.md`,
//! `entry_id_prefix = "ADR-"`, `changelog_titles = ["Decisions"]`.
//! - **fixture B (English README + ARCHITECTURE)**: 2-doc project with
//! heading-anchor convention, `changelog_titles = ["Changelog"]`,
//! no entry prefix (numbered ledger disabled).
//! - **fixture C (Japanese spec)**: `locale = "ja"` placeholder + `。`
//! in body prose; verifies the parser does not mangle CJK content.
//!
//! All three exercise `discover_config` + `Workspace::from_config` +
//! `parse_markdown_with_schema` end-to-end — the same path the CLI takes
//! for the Mnemosyne self-application. This is the empirical
//! "zero-decoding" proof for Round 151 closure: the codebase is unchanged
//! between fixtures; only the TOML differs.

use mnemosyne_validator::{
 discover_config, parse_markdown_with_schema,
 validator::cross_ref_orphan_reject_with_workspace, SchemaSection, Workspace,
};
use std::fs;
use tempfile::TempDir;

#[test]
fn fixture_a_adr_style() {
 let tmp = TempDir::new().unwrap();
 let root = tmp.path();
 fs::create_dir_all(root.join("docs/adr")).unwrap();

 fs::write(
 root.join("mnemosyne.toml"),
 r#"
[workspace]
docs = ["docs/adr/ADR-0001.md", "docs/adr/ADR-0002.md", "README.md"]
default_doc = "docs/adr/ADR-0001.md"

[schema]
changelog_titles = ["Decisions"]
entry_id_prefix = "ADR-"
anchor_convention = "adr_id"
medium_name = "adr"
"#,
 )
 .unwrap();

 fs::write(
 root.join("docs/adr/ADR-0001.md"),
 r#"# ADR 0001 — Authentication

## 1. Choice

JWT tokens with HMAC-SHA256.

## Decisions

- ADR-0001: Choose JWT over session cookie.
"#,
 )
 .unwrap();
 fs::write(
 root.join("docs/adr/ADR-0002.md"),
 r#"# ADR 0002 — Key rotation

## 1. Policy

Rotate keys monthly.

## Decisions

- ADR-0002: Rotate signing keys monthly. See §1 for the JWT base.
"#,
 )
 .unwrap();
 fs::write(
 root.join("README.md"),
 "# Project\n\n## Summary\n\nADR registry — see decisions.\n",
 )
 .unwrap();

 let loaded = discover_config(root).unwrap().expect("config");
 let schema = loaded
 .config
 .schema
 .clone()
 .unwrap_or_else(SchemaSection::mnemosyne_preset);
 let mut ws = Workspace::from_config(&loaded);
 for path in loaded.doc_paths() {
 let abs = loaded.doc_abs_path(path);
 let content = fs::read_to_string(&abs).unwrap();
 let parsed = parse_markdown_with_schema(&content, path, &schema);
 ws.insert(path.to_string(), parsed);
 }

 // Verify ADR entries captured under the "Decisions" heading via the
 // ADR- prefix. Each doc has a single ADR entry.
 let adr_1 = ws.docs.get("docs/adr/ADR-0001.md").unwrap();
 let adr_2 = ws.docs.get("docs/adr/ADR-0002.md").unwrap();
 let ids_1: Vec<&str> = adr_1
 .changelog_entries
 .iter()
 .map(|e| e.entry_id.as_str())
 .collect();
 let ids_2: Vec<&str> = adr_2
 .changelog_entries
 .iter()
 .map(|e| e.entry_id.as_str())
 .collect();
 assert_eq!(ids_1, vec!["ADR-0001"]);
 assert_eq!(ids_2, vec!["ADR-0002"]);

 // Verify cross-doc reclassify: ADR-0002 references §1 which exists
 // intra-doc (its own §1) — no orphan.
 let orphans = cross_ref_orphan_reject_with_workspace(adr_2, &ws);
 assert!(
 orphans.is_empty(),
 "ADR-0002 §1 must intra-doc resolve, got: {:?}",
 orphans
 );
}

#[test]
fn fixture_b_english_readme_plus_arch() {
 let tmp = TempDir::new().unwrap();
 let root = tmp.path();
 fs::create_dir_all(root.join("docs")).unwrap();

 fs::write(
 root.join("mnemosyne.toml"),
 r#"
[workspace]
docs = ["README.md", "docs/ARCHITECTURE.md"]
default_doc = "README.md"

[schema]
changelog_titles = ["Changelog"]
entry_id_prefix = ""
anchor_convention = "heading_slug"
medium_name = "generic"
"#,
 )
 .unwrap();

 fs::write(
 root.join("README.md"),
 r#"# My Project

## Overview

Documentation lives in [ARCHITECTURE](docs/ARCHITECTURE.md).

## Changelog

- v0.1: initial release.
"#,
 )
 .unwrap();
 fs::write(
 root.join("docs/ARCHITECTURE.md"),
 r#"# Architecture

## Components

The system has three components.
"#,
 )
 .unwrap();

 let loaded = discover_config(root).unwrap().expect("config");
 let schema = loaded
 .config
 .schema
 .clone()
 .unwrap_or_else(SchemaSection::generic_default);
 let mut ws = Workspace::from_config(&loaded);
 for path in loaded.doc_paths() {
 let abs = loaded.doc_abs_path(path);
 let content = fs::read_to_string(&abs).unwrap();
 let parsed = parse_markdown_with_schema(&content, path, &schema);
 ws.insert(path.to_string(), parsed);
 }

 // Empty entry_id_prefix → no entries even though "## Changelog" parses
 // as a Section with the right title.
 let readme = ws.docs.get("README.md").unwrap();
 assert!(
 readme.changelog_entries.is_empty(),
 "empty entry_id_prefix disables capture"
 );

 // Both docs round-trip without orphan cross-refs.
 for (path, parsed) in &ws.docs {
 let orphans = cross_ref_orphan_reject_with_workspace(parsed, &ws);
 assert!(
 orphans.is_empty(),
 "{} has orphans: {:?}",
 path,
 orphans
 );
 }
}

#[test]
fn fixture_c_japanese_locale_placeholder() {
 let tmp = TempDir::new().unwrap();
 let root = tmp.path();
 fs::create_dir_all(root.join("docs")).unwrap();

 fs::write(
 root.join("mnemosyne.toml"),
 r#"
[workspace]
docs = ["docs/SPEC.md"]
default_doc = "docs/SPEC.md"

[schema]
changelog_titles = ["更新履歴", "Changelog"]
entry_id_prefix = ""
medium_name = "spec_ja"

[style]
locale = "ja"
"#,
 )
 .unwrap();

 // Japanese body prose using `。` as the sentence terminator. The
 // current parser's split_sentences uses `.` / `!` / `?` so `。` is not
 // a boundary — but the parser must still preserve CJK content
 // verbatim through round-trip. Deeper locale wiring (`。` terminator)
 // is a follow-up round.
 fs::write(
 root.join("docs/SPEC.md"),
 r#"# 仕様書

## 1. 概要

このプロジェクトはMnemosyneを使用しています。すべての仕様はここに記録されます。

## 更新履歴

- v0.1: 初版。
"#,
 )
 .unwrap();

 let loaded = discover_config(root).unwrap().expect("config");
 let schema = loaded
 .config
 .schema
 .clone()
 .unwrap_or_else(SchemaSection::mnemosyne_preset);
 let mut ws = Workspace::from_config(&loaded);
 for path in loaded.doc_paths() {
 let abs = loaded.doc_abs_path(path);
 let content = fs::read_to_string(&abs).unwrap();
 let parsed = parse_markdown_with_schema(&content, path, &schema);
 ws.insert(path.to_string(), parsed);
 }

 // Verify Japanese title carries through to Section.
 let spec = ws.docs.get("docs/SPEC.md").unwrap();
 let section_titles: Vec<&str> = spec.sections.iter().map(|s| s.title.as_str()).collect();
 assert!(
 section_titles.iter().any(|t| t.contains("概要")),
 "Japanese title must carry verbatim, got: {:?}",
 section_titles
 );
 assert!(
 section_titles.iter().any(|t| t == &"更新履歴"),
 "更新履歴 heading must parse as a Section"
 );
}
