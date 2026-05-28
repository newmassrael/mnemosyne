//! Round 143 — schema-as-input integration test.
//!
//! Verifies the parser's schema-aware path (`parse_markdown_with_schema`)
//! produces the right ChangelogEntry recognition under three configurations:
//!
//! 1. **Mnemosyne preset** — round-trip on `docs/DESIGN.md` is byte-stable
//! vs the legacy `parse_markdown` (which forwards to the same preset).
//! 2. **Generic markdown preset** — only "Changelog" / "changelog" titles
//! open a ChangelogEntry container; "Changelog" does not.
//! 3. **Custom override** — author-supplied changelog titles (e.g.
//! `["Decisions"]`) parse correctly without touching production code.

use mnemosyne_config::SchemaSection;
use mnemosyne_parser::{parse_markdown, parse_markdown_with_schema};
use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

#[test]
fn mnemosyne_preset_matches_legacy_parser() {
    // The legacy `parse_markdown` forwards to the schema-aware path with the
    // Mnemosyne preset — the two outputs must be identical for the workspace
    // self-application's round-trip to remain byte-stable.
    //
    // Round 251 — sourced from `docs/GENERATED.md` (the sole readable artifact
    // post 7-md deletion). The 7 source markdowns were deleted; GENERATED.md
    // is the new round-trip fixture (atomic store derived view).
    let path = repo_root().join("docs/GENERATED.md");
    let content = fs::read_to_string(&path).unwrap();

    let legacy = parse_markdown(&content, "docs/GENERATED.md");
    let schema = SchemaSection::mnemosyne_preset();
    let aware = parse_markdown_with_schema(&content, "docs/GENERATED.md", &schema);

    assert_eq!(
        legacy.sections.len(),
        aware.sections.len(),
        "section count must match preset"
    );
    assert_eq!(
        legacy.changelog_entries.len(),
        aware.changelog_entries.len(),
        "changelog entry count must match preset"
    );
    assert_eq!(
        legacy.cross_refs.len(),
        aware.cross_refs.len(),
        "cross_ref count must match preset"
    );
}

#[test]
fn generic_preset_disables_entry_capture() {
    // The generic_default preset omits "Changelog" from changelog_titles
    // AND sets entry_id_prefix = "" — generic markdown rarely has a fixed
    // numeric entry ID convention. Both filters carry: even bullets that
    // would match Mnemosyne's "Round N" pattern are not captured.
    let content = r#"# Spec

## Changelog

- Round 1: under Korean heading, omitted from generic_default titles.

## Changelog

- Round 2: under English heading, but generic_default prefix is empty.
"#;
    let schema = SchemaSection::generic_default();
    let parsed = parse_markdown_with_schema(content, "spec.md", &schema);

    assert_eq!(parsed.sections.len(), 3, "h1 + 2 h2 sections");
    assert!(
        parsed.changelog_entries.is_empty(),
        "generic_default has empty entry_id_prefix → no entries captured"
    );
}

#[test]
fn generic_with_custom_prefix_captures_under_english_only() {
    // External user wanting English-only Changelog headings + Round N
    // bullet pattern: override generic_default with a non-empty prefix.
    let schema = SchemaSection {
        changelog_titles: vec!["Changelog".to_string()],
        entry_id_prefix: "Round ".to_string(),
        anchor_convention: "heading_slug".to_string(),
        medium_name: "generic".to_string(),
    };
    let content = r#"# Spec

## History

- Round 1: under "History" heading, not in custom title set; must skip.

## Changelog

- Round 2: English heading + Round prefix, must capture.
"#;
    let parsed = parse_markdown_with_schema(content, "spec.md", &schema);
    let ids: Vec<&str> = parsed
        .changelog_entries
        .iter()
        .map(|e| e.entry_id.as_str())
        .collect();
    assert_eq!(ids, vec!["Round 2"]);
}

#[test]
fn mnemosyne_preset_recognizes_korean_title() {
    // Mirror of above with Mnemosyne preset — both headings open changelog
    // sections, so both entries are captured.
    let content = r#"# Spec

## Changelog

- Round 1: legacy entry.

## Changelog

- Round 2: english heading.
"#;
    let schema = SchemaSection::mnemosyne_preset();
    let parsed = parse_markdown_with_schema(content, "spec.md", &schema);

    let ids: Vec<&str> = parsed
        .changelog_entries
        .iter()
        .map(|e| e.entry_id.as_str())
        .collect();
    assert_eq!(ids, vec!["Round 1", "Round 2"]);
}

#[test]
fn custom_override_recognizes_alternate_changelog_title() {
    // External user with a non-standard changelog heading. Custom override
    // routes "History" through the same ChangelogEntry parser. Note: bullet
    // recognition (`Round N`) is the legacy hardcode Round 144 will replace;
    // this round verifies *title* dispatch alone.
    let custom = SchemaSection {
        changelog_titles: vec!["History".to_string()],
        medium_name: "knowledge_base".to_string(),
        ..SchemaSection::mnemosyne_preset()
    };

    let content = r#"# Knowledge Base

## History

- Round 5: alternate title still routes the bullet parser.
- Round 6: second entry.
"#;
    let parsed = parse_markdown_with_schema(content, "kb.md", &custom);

    let ids: Vec<&str> = parsed
        .changelog_entries
        .iter()
        .map(|e| e.entry_id.as_str())
        .collect();
    assert_eq!(ids, vec!["Round 5", "Round 6"]);

    // A "Notes" heading in the same doc must NOT trigger because the
    // custom set omits it (case-insensitive `changelog` carry only matches
    // the literal `changelog` token; "Notes" is unrelated).
    let other = r#"## Notes

- Round 7: under Notes heading, custom set skips.
"#;
    let parsed_other = parse_markdown_with_schema(other, "kb.md", &custom);
    assert_eq!(
        parsed_other.changelog_entries.len(),
        0,
        "custom override must not pick up Notes"
    );
}

#[test]
fn custom_override_diagnostic_label_carries() {
    // medium_name has no parser semantics but should round-trip through
    // SchemaSection clone for diagnostics consumers (Round 143 carry).
    let custom = SchemaSection {
        changelog_titles: vec!["History".to_string()],
        medium_name: "knowledge_base".to_string(),
        ..SchemaSection::mnemosyne_preset()
    };
    let cloned = custom.clone();
    assert_eq!(cloned.medium_name, "knowledge_base");
    assert_eq!(cloned.changelog_titles, vec!["History"]);
}
