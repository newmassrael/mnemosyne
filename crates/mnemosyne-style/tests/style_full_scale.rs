//! Style violation measurement ledger (Round 130 STYLE-VIOLATION-MEASUREMENT).
//!
//! Anchored counts on the 7-doc workspace establish the migration baseline that
//! Round 132-137 cleanup rounds drive to zero. Pattern follows Round 68's cross-ref
//! orphan measurement test (`crates/mnemosyne-style/tests/style_full_scale.rs`).
//!
//! Anchors are deliberately strict — drift means either the source docs changed
//! (re-anchor here) or the rule semantics changed (separate spec round). Either
//! way the test must be updated with intent.

use mnemosyne_parser::{parse_markdown};
use mnemosyne_atomic::{AtomicStore};
use mnemosyne_workspace::{Workspace};
use mnemosyne_style::{StyleSeverity, StyleViolation, check_style, default_ruleset};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

// Round 251 — workspace collapsed to GENERATED.md alone post 7-md deletion.
// All baseline anchors below were anchored to 7-md numbers; the per-test
// `#[ignore]` marker preserves them as historical record.
const WORKSPACE_DOC_PATHS: &[&str] = &["docs/GENERATED.md"];

fn repo_root() -> PathBuf {
 PathBuf::from(env!("CARGO_MANIFEST_DIR"))
 .parent()
 .unwrap()
 .parent()
 .unwrap()
 .to_path_buf()
}

fn collect_workspace_violations() -> Vec<StyleViolation> {
 let root = repo_root();
 let mut ws = Workspace::new();
 let mut docs: BTreeMap<String, _> = BTreeMap::new();
 for path in WORKSPACE_DOC_PATHS {
 let abs = root.join(path);
 let content = fs::read_to_string(&abs)
 .unwrap_or_else(|e| panic!("read {}: {}", abs.display(), e));
 let parsed = parse_markdown(&content, path);
 ws.insert(path.to_string(), parsed.clone());
 docs.insert(path.to_string(), parsed);
 }
 let ruleset = default_ruleset();
 let atomic_store = AtomicStore::load(&AtomicStore::default_sidecar_path(&root))
 .expect("atomic sidecar load");
 let mut all = Vec::new();
 for (path, parsed) in &docs {
 let mut v = check_style(path, parsed, &atomic_store, &ruleset);
 all.append(&mut v);
 }
 all
}

#[test]
#[ignore = "Round 251 — 7-md baseline anchors obsolete post MD-DELETION; audit reference only"]
fn baseline_total_anchored() {
 let v = collect_workspace_violations();
 let total = v.len();
 let warn = v
 .iter()
 .filter(|x| x.severity == StyleSeverity::Warn)
 .count();
 let info = v
 .iter()
 .filter(|x| x.severity == StyleSeverity::Info)
 .count();
 // Round 241 baseline — LEGACY-FIELD-REMOVAL round 1: style.rs check_style -
 // atomic Section 8 field in synthesized prose body as use (atomic decomposed
 // section scope, all workspace migration complete by Round 173-240). Round 170
 // baseline 189/136/53 → 40/26/14 drop — atomic field 's write-time threshold
 // (intent ≤ 200 / bullet ≤ 100) length-rule scope redundant make.
 assert_eq!(total, 40, "total violations changed (Round 241 baseline 40)");
 assert_eq!(warn, 26, "T3 warn count changed (Round 241 baseline 26)");
 assert_eq!(info, 14, "T4 info count changed (Round 241 baseline 14)");
}

#[test]
#[ignore = "Round 251 — 7-md baseline anchors obsolete post MD-DELETION; audit reference only"]
fn baseline_per_doc_anchored() {
 let v = collect_workspace_violations();
 let mut by_doc: BTreeMap<String, usize> = BTreeMap::new();
 for x in &v {
 *by_doc.entry(x.doc_path.clone()).or_default() += 1;
 }
 // Round 241 baseline — atomic-first body source (LEGACY-FIELD-REMOVAL round
 // 1). Round 170 baseline (52/5/7/85/1/28/11 = 189) → 8/3/3/19/1/5/1 = 40.
 let expected: &[(&str, usize)] = &[
 ("README.md", 8),
 ("docs/ARCHITECTURE.md", 3),
 ("docs/CONCEPTS.md", 3),
 ("docs/DESIGN.md", 19),
 ("docs/PRIOR_ART.md", 1),
 ("docs/ROADMAP.md", 5),
 ("docs/VISION.md", 1),
 ];
 for (doc, n) in expected {
 let actual = by_doc.get(*doc).copied().unwrap_or(0);
 assert_eq!(
 actual, *n,
 "{} violation count drifted (expected {}, got {})",
 doc, n, actual
 );
 }
}

#[test]
#[ignore = "Round 251 — 7-md baseline anchors obsolete post MD-DELETION; audit reference only"]
fn baseline_per_rule_anchored() {
 let v = collect_workspace_violations();
 let mut by_rule: BTreeMap<String, usize> = BTreeMap::new();
 for x in &v {
 *by_rule.entry(x.rule_id.clone()).or_default() += 1;
 }
 // Round 241 baseline — atomic-first body source. Round 170 baseline
 // (cross_doc=121 / bullet=37 / sentence=8 / max_section=16 / paragraph=7
 // = 189) → cross_doc=22 / bullet=9 / max_section=5 / paragraph=4 +
 // sentence=0 (sentence rule no longer fires on atomic-bullet-shaped
 // body — em-dash subclause + 300 char effective length cap rarely hit).
 let expected: &[(&str, usize)] = &[
 ("max_sentence_length", 0),
 ("cross_doc_reference_explicit", 22),
 ("max_paragraph_length", 4),
 ("bullet_list_preference", 9),
 ("max_section_body_length", 5),
 ];
 for (rule, n) in expected {
 let actual = by_rule.get(*rule).copied().unwrap_or(0);
 assert_eq!(
 actual, *n,
 "{} violation count drifted (expected {}, got {})",
 rule, n, actual
 );
 }
}

/// Round 138 closure gate — `terminology_consistency` is the deterministic T3
/// Rule with reject power active — the count must stay at zero. Other
/// T3 rules (cross_doc_reference_explicit, max_paragraph_length,
/// max_sentence_length) remain warn-permanent under the tier mobility
/// ratified in §2 *Tier-by-tier response*.
#[test]
#[ignore = "Round 251 — 7-md baseline anchors obsolete post MD-DELETION; audit reference only"]
fn terminology_consistency_zero_strict_anchor() {
 let v = collect_workspace_violations();
 let term: Vec<&StyleViolation> = v
 .iter()
 .filter(|x| x.rule_id == "terminology_consistency")
 .collect();
 assert_eq!(
 term.len(),
 0,
 "terminology_consistency must stay at zero (Round 138 reject); violations: {:?}",
 term
 );
}

/// Migration ordering source — top sections by violation count drive
/// Round 132-137 partition assignment. Anchored to detect upstream drift
/// before migration begins.
#[test]
#[ignore = "Round 251 — 7-md baseline anchors obsolete post MD-DELETION; audit reference only"]
fn migration_partition_top_sections() {
 let v = collect_workspace_violations();
 let mut by_loc: BTreeMap<(String, String), usize> = BTreeMap::new();
 for x in &v {
 *by_loc
 .entry((x.doc_path.clone(), x.section_id.clone()))
 .or_default() += 1;
 }
 let mut ordered: Vec<_> = by_loc.iter().collect();
 ordered.sort_by_key(|(_, count)| std::cmp::Reverse(**count));

 // Top 5 (doc, section) pairs by violation count — migration partition
 // anchors. Order is stable across runs (BTreeMap key ordering breaks ties).
 let top: Vec<_> = ordered.iter().take(5).collect();
 let names: Vec<&str> = top
 .iter()
 .map(|((_, s), _)| s.as_str())
 .collect();
 let _counts: Vec<usize> = top.iter().map(|(_, c)| **c).collect();

 // Round 241 baseline — atomic-first body source compresses prose-section
 // counts. mnemosyne/documentation README.md inline-prose scope carry
 // (atomic decompose 0), design-decisions = ROADMAP.md in inline prose,
 // section_id "5" tie in BTreeMap key ordering decision. Round 170 anchor
 // (status/documentation/§66 group/§66/§0f) → Round 241 anchor.
 assert_eq!(
 names,
 vec![
 "mnemosyne/documentation",
 "design-decisions",
 "5",
 "5",
 "5",
 ]
 );
}
