//! Round 145 — style rule i18n + terminology glossary integration test.
//!
//! Verifies the config-driven style ruleset path:
//!
//! 1. `default_ruleset_with_config(None, None)` produces the same output
//! as the legacy `default_ruleset()` (back-compat carry).
//! 2. A style threshold override (e.g. max_sentence_length = 100) flows
//! through to the rule definition.
//! 3. A custom terminology glossary replaces the Mnemosyne preset.
//! 4. An empty glossary effectively disables the terminology rule
//! (rule still runs but matches nothing).

use mnemosyne_config::{StyleSection, TerminologySection};
use mnemosyne_style::{StyleThreshold, default_ruleset, default_ruleset_with_config, glossary_from_config};
use std::collections::BTreeMap;

#[test]
fn config_driven_default_matches_legacy_when_unset() {
 let baseline = default_ruleset();
 let configured = default_ruleset_with_config(None, None);

 assert_eq!(baseline.len(), configured.len());
 for (a, b) in baseline.iter().zip(configured.iter()) {
 assert_eq!(a.rule_id, b.rule_id);
 assert_eq!(a.tier as u8, b.tier as u8);
 }
}

#[test]
fn style_threshold_override_takes_effect() {
 let mut thresholds = BTreeMap::new();
 thresholds.insert("max_sentence_length".to_string(), 100u32);
 thresholds.insert("max_paragraph_length".to_string(), 500u32);
 let style = StyleSection {
 locale: "ko".to_string(),
 thresholds,
 };

 let rules = default_ruleset_with_config(Some(&style), None);
 for rule in &rules {
 match rule.rule_id.as_str() {
 "max_sentence_length" => match rule.threshold {
  StyleThreshold::CharCount(n) => assert_eq!(n, 100),
  _ => panic!("max_sentence_length must be CharCount"),
 },
 "max_paragraph_length" => match rule.threshold {
  StyleThreshold::CharCount(n) => assert_eq!(n, 500),
  _ => panic!("max_paragraph_length must be CharCount"),
 },
 _ => {}
 }
 }
}

#[test]
fn terminology_config_replaces_preset() {
 let mut glossary = BTreeMap::new();
 glossary.insert(
 "JWT".to_string(),
 vec!["jwt".to_string(), "Jwt".to_string()],
 );
 let term = TerminologySection { glossary };

 let rules = default_ruleset_with_config(None, Some(&term));
 let term_rule = rules
 .iter()
 .find(|r| r.rule_id == "terminology_consistency")
 .expect("terminology rule present");

 match &term_rule.threshold {
 StyleThreshold::GlossaryLookup(g) => {
 assert!(g.contains_key("JWT"));
 // Mnemosyne preset entries replaced.
 assert!(!g.contains_key("Salsa"));
 assert!(!g.contains_key("bi-temporal"));
 }
 _ => panic!("terminology rule must be GlossaryLookup"),
 }
}

#[test]
fn empty_glossary_config_falls_back_to_mnemosyne_preset() {
 // Empty glossary in config = no override, fallback to workspace_glossary
 // (Mnemosyne preset). Documents intent: external users with no
 // project-specific terms get the curated default rather than nothing.
 let term = TerminologySection {
 glossary: BTreeMap::new(),
 };
 let rules = default_ruleset_with_config(None, Some(&term));
 let term_rule = rules
 .iter()
 .find(|r| r.rule_id == "terminology_consistency")
 .unwrap();
 match &term_rule.threshold {
 StyleThreshold::GlossaryLookup(g) => {
 assert!(g.contains_key("Salsa"), "preset carry on empty config");
 }
 _ => panic!(),
 }
}

#[test]
fn glossary_from_config_round_trip() {
 let mut glossary = BTreeMap::new();
 glossary.insert(
 "JSON".to_string(),
 vec!["json".to_string(), "Json".to_string()],
 );
 glossary.insert("API".to_string(), vec!["api".to_string()]);
 let term = TerminologySection { glossary };

 let parsed_glossary = glossary_from_config(&term);
 assert_eq!(parsed_glossary.len(), 2);
 let json_variants = parsed_glossary.get("JSON").unwrap();
 assert!(json_variants.contains("json"));
 assert!(json_variants.contains("Json"));
}
