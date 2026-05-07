//! Round 140 detector tightening smoke test (T3-RULE-REDESIGN-DETECTOR-TIGHTENING).
//!
//! Synthesises the *exact shapes* the Round 139 audit identified as false
//! positives and verifies the new detector no longer fires on them, plus
//! regression guards that the parts of detection we intend to keep still work.
//!
//! Three deltas under test:
//! 1. **Strong-carry section skip** — `max_paragraph_length` /
//! `max_sentence_length` skip impact_scope / changelog / changelog_referenced
//! / top_level_numeric sections.
//! 2. **Em-dash subclause** — `A — B — C` measured as max(|A|,|B|,|C|), not
//! |A| + |B| + |C| + dash overhead.
//! 3. **Threshold tuning** — `max_sentence_length` 200 -> 300 char to fit
//! technical-prose syllable density.

use mnemosyne_validator::{check_style, default_ruleset, parse_markdown, AtomicStore};

/// Repeated filler word (8 chars) used to inflate paragraph length without
/// triggering terminology violations.
const FILLER: &str = "lorem ip ";

fn long_paragraph(repeats: usize) -> String {
 FILLER.repeat(repeats)
}

/// Strong-carry §43 with a 600-char single paragraph — pre-140 would emit
/// max_paragraph_length, post-140 must skip.
fn md_top_level_numeric_skip() -> String {
 format!(
 "# Skip test\n\n## 43. cascade_query kind\n\n{}\n",
 long_paragraph(80)
 )
}

/// impact-range sub-section (impact_scope) with a 1500-char single paragraph —
/// pre-140 would emit max_paragraph_length, post-140 must skip.
fn md_impact_scope_skip() -> String {
 format!(
 "# Impact scope skip test\n\n## 12. Concurrency\n\n### impact range\n\n{}\n",
 long_paragraph(180)
 )
}

/// changelog-referenced section (Round-N section_id) with a long paragraph
/// — strong-carry skip applies.
fn md_changelog_referenced_skip() -> String {
 format!(
 "# Changelog referenced skip test\n\n## 41. datalog_rule kind\n\n### Style rule layer (T3/T4 — MD-quality, Round 128 created)\n\n{}\n",
 long_paragraph(60)
 )
}

/// Em-dash chained sentence — 4 clauses × ~100 char each, total ~400 char,
/// effective length max(100, 100, 100, 100) ≈ 100. Pre-140 would emit
/// (length > 200), post-140 must NOT.
fn md_em_dash_no_violation() -> String {
 let clause = FILLER.repeat(12); // ~108 chars
 format!(
 "# Em-dash test\n\n## test\n\n{} — {} — {} — {}.\n",
 clause, clause, clause, clause
 )
}

/// Single 350-char sentence (no em-dash) in a non-strong-carry prose section
/// — must STILL violate (regression guard for the 300-char cap).
fn md_long_sentence_regression() -> String {
 format!(
 "# Regression test\n\n## test/prose-section\n\n{}.\n",
 FILLER.repeat(45)
 )
}

#[test]
fn skip_top_level_numeric_strong_carry() {
 let md = md_top_level_numeric_skip();
 let parsed = parse_markdown(&md, "test.md");
 let v = check_style("test.md", &parsed, &AtomicStore::default(), &default_ruleset());
 let para = v.iter().filter(|x| x.rule_id == "max_paragraph_length").count();
 let sent = v.iter().filter(|x| x.rule_id == "max_sentence_length").count();
 assert_eq!(para, 0, "max_paragraph_length must skip §43 (top_level_numeric)");
 assert_eq!(sent, 0, "max_sentence_length must skip §43 (top_level_numeric)");
}

#[test]
fn skip_impact_scope_section() {
 let md = md_impact_scope_skip();
 let parsed = parse_markdown(&md, "test.md");
 let v = check_style("test.md", &parsed, &AtomicStore::default(), &default_ruleset());
 let para = v.iter().filter(|x| x.rule_id == "max_paragraph_length").count();
 let sent = v.iter().filter(|x| x.rule_id == "max_sentence_length").count();
 assert_eq!(para, 0, "max_paragraph_length must skip §12/impact-range");
 assert_eq!(sent, 0, "max_sentence_length must skip §12/impact-range");
}

#[test]
fn skip_changelog_referenced_section() {
 let md = md_changelog_referenced_skip();
 let parsed = parse_markdown(&md, "test.md");
 let v = check_style("test.md", &parsed, &AtomicStore::default(), &default_ruleset());
 let para = v.iter().filter(|x| x.rule_id == "max_paragraph_length").count();
 let sent = v.iter().filter(|x| x.rule_id == "max_sentence_length").count();
 assert_eq!(para, 0, "max_paragraph_length must skip Round-N section");
 assert_eq!(sent, 0, "max_sentence_length must skip Round-N section");
}

#[test]
fn em_dash_subclause_no_violation() {
 let md = md_em_dash_no_violation();
 let parsed = parse_markdown(&md, "test.md");
 let v = check_style("test.md", &parsed, &AtomicStore::default(), &default_ruleset());
 let sent = v.iter().filter(|x| x.rule_id == "max_sentence_length").count();
 assert_eq!(
 sent, 0,
 "em-dash chained 4×100-char clauses must measure effective 100, not ~400"
 );
}

#[test]
fn long_sentence_regression_guard() {
 let md = md_long_sentence_regression();
 let parsed = parse_markdown(&md, "test.md");
 let v = check_style("test.md", &parsed, &AtomicStore::default(), &default_ruleset());
 let sent: Vec<_> = v.iter().filter(|x| x.rule_id == "max_sentence_length").collect();
 assert_eq!(
 sent.len(),
 1,
 "350-char single sentence in prose_named must still violate"
 );
}

#[test]
fn cross_doc_still_applies_in_strong_carry() {
 // Regression — cross_doc_reference_explicit must NOT skip strong-carry
 // sections. The detector exempts `Round N` follow + backticks but bare
 // doc names elsewhere in a Round-N section still need anchoring.
 let md = r#"# Cross-doc still applies

## 41. datalog_rule kind

### Style rule layer (T3/T4 — MD-quality, Round 128 created)

This scope prerequisite — see ARCHITECTURE.md for layer split.
"#;
 let parsed = parse_markdown(md, "test.md");
 let v = check_style("test.md", &parsed, &AtomicStore::default(), &default_ruleset());
 assert!(
 v.iter().any(|x| x.rule_id == "cross_doc_reference_explicit"),
 "cross_doc_reference_explicit must continue to apply in strong-carry sections"
 );
}
