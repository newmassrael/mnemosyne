//! Style rule layer integration test (Round 129 production wire).
//!
//! Verifies end-to-end detection on synthetic violations parsed via the real
//! markdown parser path — guards against drift between style.rs and the
//! Section.body / ChangelogEntry.sub_bullets surface.

use mnemosyne_plugin::DecisionStatus;
use mnemosyne_validator::{
 check_style, default_ruleset, parse_markdown, AtomicStore, StyleSeverity,
};

// Round 140 detector tightening — strong-carry skip applies to top_level_numeric
// section_ids, so this fixture uses a prose-named heading to keep the run-on
// detection assertion meaningful.
const MD_RUN_ON: &str = r#"# Run-on test

## Section A

<RUNON>

## Changelog

- Round 1 (TEST):
  - **Round scope.** This round's substantive contribution = round-scope ratify carry
  - **Round scope.** This round's substantive contribution = round-scope ratify pass
"#;

const MD_BULLET: &str = r#"# Bullet test

## 1. Section A

Trade-off — (i) closure scope 5-language emit break acknowledged, (ii) Phase entry block framing deprecate, (iii) Tier 5 measurement-pending lock new 1cases registered. this decision's fallback path explicit secondary.

## Changelog

- Round 1 (TEST):
  - **A.** abc
"#;

const MD_CROSS_DOC_IMPLICIT: &str = r#"# Cross-doc test

## 1. Section A

this scope prerequisite — see ARCHITECTURE.md and ROADMAP.md for details.

## Changelog

- Round 1 (TEST):
  - **A.** abc
"#;

const MD_CROSS_DOC_ANCHORED: &str = r#"# Cross-doc anchored

## 1. Section A

this scope prerequisite — see ARCHITECTURE.md#§3 and ROADMAP.md#§Phase-1A for details.

## Changelog

- Round 1 (TEST):
  - **A.** abc
"#;

#[test]
fn smoke_run_on_paragraph_detected() {
 let para = "-".repeat(1500);
 let md = MD_RUN_ON.replace("<RUNON>", &para);
 let parsed = parse_markdown(&md, "docs/TEST.md");
 assert!(parsed.sections.iter().any(|s| s.section_id == "run-on-test/section-a"));
 let v = check_style("docs/TEST.md", &parsed, &AtomicStore::default(), &default_ruleset());
 assert!(
 v.iter().any(|x| x.rule_id == "max_paragraph_length"),
 "expected max_paragraph_length violation, got: {:?}",
 v
 );
}

#[test]
fn smoke_changelog_boilerplate_detected() {
 let md = MD_RUN_ON.replace("<RUNON>", "short body");
 let parsed = parse_markdown(&md, "docs/TEST.md");
 let v = check_style("docs/TEST.md", &parsed, &AtomicStore::default(), &default_ruleset());
 let boil: Vec<_> = v
 .iter()
 .filter(|x| x.rule_id == "boilerplate_repetition_jaccard")
 .collect();
 assert!(
 !boil.is_empty(),
 "expected boilerplate_repetition_jaccard violation"
 );
 assert_eq!(boil[0].severity, StyleSeverity::Info);
}

#[test]
fn smoke_enumeration_pattern_detected() {
 let parsed = parse_markdown(MD_BULLET, "docs/TEST.md");
 let v = check_style("docs/TEST.md", &parsed, &AtomicStore::default(), &default_ruleset());
 assert!(
 v.iter().any(|x| x.rule_id == "bullet_list_preference"),
 "expected bullet_list_preference violation, got: {:?}",
 v
 );
}

#[test]
fn smoke_cross_doc_implicit_detected() {
 let parsed = parse_markdown(MD_CROSS_DOC_IMPLICIT, "docs/TEST.md");
 let v = check_style("docs/TEST.md", &parsed, &AtomicStore::default(), &default_ruleset());
 assert!(
 v.iter()
 .any(|x| x.rule_id == "cross_doc_reference_explicit"),
 "expected cross_doc_reference_explicit violation, got: {:?}",
 v
 );
}

#[test]
fn smoke_cross_doc_anchored_no_violation() {
 let parsed = parse_markdown(MD_CROSS_DOC_ANCHORED, "docs/TEST.md");
 let v = check_style("docs/TEST.md", &parsed, &AtomicStore::default(), &default_ruleset());
 assert!(
 !v.iter()
 .any(|x| x.rule_id == "cross_doc_reference_explicit"),
 "did not expect cross_doc_reference_explicit, got: {:?}",
 v
 );
}

#[test]
fn smoke_clean_doc_no_t3_violations() {
 let md = "# Clean doc\n\n## 1. Section A\n\nShort body.\n\n## Changelog\n\n- Round 1 (TEST):\n  - **A.** abc\n";
 let parsed = parse_markdown(md, "docs/TEST.md");
 assert!(parsed
 .sections
 .iter()
 .any(|s| s.decision_status == DecisionStatus::Active));
 let v = check_style("docs/TEST.md", &parsed, &AtomicStore::default(), &default_ruleset());
 assert!(
 !v.iter().any(|x| x.severity == StyleSeverity::Warn),
 "expected no T3 warn violations on clean doc, got: {:?}",
 v
 );
}

#[test]
fn smoke_section_body_length_t4_only() {
 let body = "-".repeat(6000);
 let md = format!(
 "# Long body\n\n## 1. Section A\n\n{}\n\n## Changelog\n\n- Round 1 (TEST):\n  - **A.** abc\n",
 body
 );
 let parsed = parse_markdown(&md, "docs/TEST.md");
 let v = check_style("docs/TEST.md", &parsed, &AtomicStore::default(), &default_ruleset());
 let body_len: Vec<_> = v
 .iter()
 .filter(|x| x.rule_id == "max_section_body_length")
 .collect();
 assert_eq!(body_len.len(), 1);
 assert_eq!(body_len[0].severity, StyleSeverity::Info);
}
