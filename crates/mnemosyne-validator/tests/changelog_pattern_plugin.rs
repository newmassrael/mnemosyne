//! Round 144 — anchor + changelog pattern plug-in integration test.
//!
//! Verifies the parser's schema-driven entry_id_prefix path:
//!
//! 1. **Mnemosyne preset** — `Round N` round-trip is byte-stable.
//! 2. **ADR preset** — `ADR-NNNN` entries captured under `## Decisions`.
//! 3. **Round preset (custom)** — `Round N` entries captured.
//! 4. **Empty prefix** — captures disabled entirely.
//! 5. **Decimal portion** — `Round 33.5` style sub-IDs preserved.

use mnemosyne_config::{SchemaSection};
use mnemosyne_parser::{parse_markdown_with_schema};

#[test]
fn mnemosyne_preset_extracts_round_n() {
 let schema = SchemaSection::mnemosyne_preset();
 let content = r#"# Spec

## Changelog

- Round 60: round body.
- Round 61: another round.
"#;
 let parsed = parse_markdown_with_schema(content, "spec.md", &schema);
 let ids: Vec<&str> = parsed
 .changelog_entries
 .iter()
 .map(|e| e.entry_id.as_str())
 .collect();
 assert_eq!(ids, vec!["Round 60", "Round 61"]);
}

#[test]
fn adr_preset_extracts_adr_nnnn() {
 let schema = SchemaSection::adr_preset();
 let content = r#"# Architecture Decisions

## Decisions

- ADR-0001: Choose JWT over session cookie.
- ADR-0042: Rotate signing keys monthly.
"#;
 let parsed = parse_markdown_with_schema(content, "decisions.md", &schema);
 let ids: Vec<&str> = parsed
 .changelog_entries
 .iter()
 .map(|e| e.entry_id.as_str())
 .collect();
 assert_eq!(ids, vec!["ADR-0001", "ADR-0042"]);
}

#[test]
fn custom_round_prefix_extracts_round_n() {
 // External user with "Round N" entry convention (e.g., a research log).
 let schema = SchemaSection {
 changelog_titles: vec!["Rounds".to_string()],
 entry_id_prefix: "Round ".to_string(),
 anchor_convention: "heading_slug".to_string(),
 medium_name: "research_log".to_string(),
 };
 let content = r#"# Lab Log

## Rounds

- Round 1: initial baseline.
- Round 5: after detector tuning.
"#;
 let parsed = parse_markdown_with_schema(content, "log.md", &schema);
 let ids: Vec<&str> = parsed
 .changelog_entries
 .iter()
 .map(|e| e.entry_id.as_str())
 .collect();
 assert_eq!(ids, vec!["Round 1", "Round 5"]);
}

#[test]
fn empty_prefix_disables_capture() {
 // generic_default carries empty prefix → no entries even if titles match.
 let schema = SchemaSection::generic_default();
 let content = r#"# Spec

## Changelog

- v1.0: shipped.
- v1.1: hot-fix.
"#;
 let parsed = parse_markdown_with_schema(content, "spec.md", &schema);
 assert!(
 parsed.changelog_entries.is_empty(),
 "empty entry_id_prefix must disable capture"
 );
}

#[test]
fn decimal_portion_preserved() {
 // `Round 33.5` sub-rounds are part of the Mnemosyne convention; the
 // decimal must survive prefix stripping + numeric extraction.
 let schema = SchemaSection::mnemosyne_preset();
 let content = r#"## Changelog

- Round 33: parent round.
- Round 33.5: sub-round (Round 33 carry).
"#;
 let parsed = parse_markdown_with_schema(content, "spec.md", &schema);
 let ids: Vec<&str> = parsed
 .changelog_entries
 .iter()
 .map(|e| e.entry_id.as_str())
 .collect();
 assert_eq!(ids, vec!["Round 33", "Round 33.5"]);
}

#[test]
fn entry_prefix_must_match_at_bullet_start() {
 // Bullets that don't open with the configured prefix must NOT be
 // captured (no false positives from prefix appearing mid-bullet).
 let schema = SchemaSection::mnemosyne_preset();
 let content = r#"## Changelog

- not a Round entry, body mentions Round 60 in the middle.
- Round 99: real entry.
"#;
 let parsed = parse_markdown_with_schema(content, "spec.md", &schema);
 let ids: Vec<&str> = parsed
 .changelog_entries
 .iter()
 .map(|e| e.entry_id.as_str())
 .collect();
 assert_eq!(ids, vec!["Round 99"]);
}
