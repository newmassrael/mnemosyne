//! Style violation audit ledger (Round 139 STYLE-WARN-CARRY-AUDIT).
//!
//! Anchored on the Round 130 measurement baseline (435/383/52) carried by Round 138
//! Tier-mobility ratify. Splits the 435 violations into 3 deterministic
//! categories — ACCEPTABLE_CARRY / SUBSTANTIVELY_DIRTY / HARD_CASE — purely from
//! (rule_id, section_kind) tuples so the classification reproduces exactly across
//! runs without any LLM-eval / sampling judgment.
//!
//! Pattern follows Round 78 orphan classification + Round 130 style measurement
//! ledger. Source-of-truth is this file's anchor counts; drift means either
//! source docs changed (re-anchor with intent) or the classification policy
//! changed (separate spec round).
//!
//! Decision-matrix outcome anchored at the bottom — drives Round 140+ round
//! selection (α / γ / δ / hybrid).

use mnemosyne_validator::{
 check_style, default_ruleset, parse_markdown, AtomicStore, StyleViolation, Workspace,
};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

// Round 251 — workspace collapsed to GENERATED.md alone post 7-md deletion.
// Style audit baselines anchored at Round 144 (7-md numbers) are no longer
// reachable; the audit suite is left as historical record.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd)]
enum SectionKind {
 ImpactScope,
 Changelog,
 ChangelogReferenced,
 TopLevelNumeric,
 NumericSubsection,
 ProseNamed,
}

impl SectionKind {
 fn as_str(self) -> &'static str {
 match self {
 SectionKind::ImpactScope => "impact_scope",
 SectionKind::Changelog => "changelog",
 SectionKind::ChangelogReferenced => "changelog_referenced",
 SectionKind::TopLevelNumeric => "top_level_numeric",
 SectionKind::NumericSubsection => "numeric_subsection",
 SectionKind::ProseNamed => "prose_named",
 }
 }
}

/// Deterministic — section_id text alone determines kind.
fn classify_section(section_id: &str) -> SectionKind {
 if section_id.ends_with("/impact-range") {
 return SectionKind::ImpactScope;
 }
 let lower = section_id.to_ascii_lowercase();
 if lower.contains("changelog") || section_id.contains("change-history") {
 return SectionKind::Changelog;
 }
 if section_id.contains("extension") {
 return SectionKind::ChangelogReferenced;
 }
 let head = section_id.split('/').next().unwrap_or("");
 let is_numeric_head = !head.is_empty()
 && head.chars().all(|c| c.is_ascii_digit() || c == '.');
 if is_numeric_head {
 if section_id.contains('/') {
 return SectionKind::NumericSubsection;
 }
 return SectionKind::TopLevelNumeric;
 }
 SectionKind::ProseNamed
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd)]
enum AuditCategory {
 AcceptableCarry,
 SubstantivelyDirty,
 HardCase,
}

impl AuditCategory {
 fn as_str(self) -> &'static str {
 match self {
 AuditCategory::AcceptableCarry => "ACCEPTABLE_CARRY",
 AuditCategory::SubstantivelyDirty => "SUBSTANTIVELY_DIRTY",
 AuditCategory::HardCase => "HARD_CASE",
 }
 }
}

/// Strong-carry kinds — frozen-feeling section bodies where length-rule edits
/// risk T2 frozen-ledger jaccard drift or carry-trail readability loss.
fn is_strong_carry_kind(k: SectionKind) -> bool {
 matches!(
 k,
 SectionKind::ImpactScope
 | SectionKind::Changelog
 | SectionKind::ChangelogReferenced
 | SectionKind::TopLevelNumeric
 )
}

/// Deterministic classifier — (rule_id, section_kind) → AuditCategory. No
/// per-violation sampling; same inputs always produce same outputs.
///
/// Rationale per branch:
/// - `bullet_list_preference` (T4 info) → ACCEPTABLE — informational suggestion
/// only, no action required by rule definition (Round 128 ratify carry).
/// - `cross_doc_reference_explicit` (T3 warn) → ACCEPTABLE — Round 130 detector
/// exempts only `Round N` follow + backticks; MD-link form `[doc](doc.md)`
/// and post-period mentions still hit. Tightening detector = δ rule
/// redesign territory, not migration target.
/// - `max_section_body_length` (T4 info) → HARD_CASE — split needs
/// `add_section` mutate + body redistribution, substantive spec mutation.
/// - `max_paragraph_length` / `max_sentence_length` on strong-carry kinds
/// → HARD_CASE — body edits risk frozen-ledger semantic drift.
/// - `max_paragraph_length` / `max_sentence_length` on numeric_subsection /
/// prose_named → SUBSTANTIVELY_DIRTY — non-frozen prose, cleanable via
/// the atomic body-field setters (set-section-intent / -rationale / etc.)
/// without Round 121 jaccard violation.
fn classify_violation(v: &StyleViolation) -> AuditCategory {
 let skind = classify_section(&v.section_id);
 match v.rule_id.as_str() {
 "bullet_list_preference" => AuditCategory::AcceptableCarry,
 "cross_doc_reference_explicit" => AuditCategory::AcceptableCarry,
 "max_section_body_length" => AuditCategory::HardCase,
 "max_paragraph_length" | "max_sentence_length" => {
 if is_strong_carry_kind(skind) {
  AuditCategory::HardCase
 } else {
  AuditCategory::SubstantivelyDirty
 }
 }
 _ => AuditCategory::AcceptableCarry,
 }
}

#[test]
#[ignore = "Round 251 — 7-md baseline anchors obsolete post MD-DELETION; audit reference only"]
fn audit_classification_baseline_anchored() {
 let v = collect_workspace_violations();
 let mut by_cat: BTreeMap<AuditCategory, usize> = BTreeMap::new();
 for x in &v {
 *by_cat.entry(classify_violation(x)).or_default() += 1;
 }
 // Round 241 baseline — atomic-first body source. AC 158 → 31 / SD 15 → 4 /
 // HC 16 -> 5 (40 total). atomic field write-time threshold pulls length-rules out of scope
 // silent (most cross_doc + max_paragraph only carry).
 assert_eq!(v.len(), 40, "total violations changed (Round 241 baseline 40)");
 assert_eq!(
 by_cat.get(&AuditCategory::AcceptableCarry).copied().unwrap_or(0),
 31,
 "ACCEPTABLE_CARRY count drifted (Round 241 baseline 31)"
 );
 assert_eq!(
 by_cat
 .get(&AuditCategory::SubstantivelyDirty)
 .copied()
 .unwrap_or(0),
 4,
 "SUBSTANTIVELY_DIRTY count drifted (Round 241 baseline 4)"
 );
 assert_eq!(
 by_cat.get(&AuditCategory::HardCase).copied().unwrap_or(0),
 5,
 "HARD_CASE count drifted (Round 241 baseline 5)"
 );
}

#[test]
#[ignore = "Round 251 — 7-md baseline anchors obsolete post MD-DELETION; audit reference only"]
fn audit_per_rule_per_category_anchored() {
 let v = collect_workspace_violations();
 let mut cell: BTreeMap<(String, AuditCategory), usize> = BTreeMap::new();
 for x in &v {
 let cat = classify_violation(x);
 *cell.entry((x.rule_id.clone(), cat)).or_default() += 1;
 }
 // Round 170 baseline — bullet 36 → 37 (+1 ROADMAP Phase 0f closure paragraph
 // enumeration pattern), HARD_CASE +1 from §66 substantive blockquote
 // mutation. cross_doc / max_paragraph / max_sentence assumed stable
 // (re-anchored if drift surfaces).
 // Round 241 baseline — atomic-first body source. bullet AC 37 → 9 /
 // cross_doc AC 121 → 22 / max_paragraph SD 7 → 4 / max_section HC 16 → 5 /
 // max_sentence SD 8 → 0 (sentence rule no longer fires on atomic-bullet-
 // shaped body, em-dash subclause + 300 char effective length cap rarely hit).
 let expected: &[(&str, AuditCategory, usize)] = &[
 ("bullet_list_preference", AuditCategory::AcceptableCarry, 9),
 (
 "cross_doc_reference_explicit",
 AuditCategory::AcceptableCarry,
 22,
 ),
 (
 "max_paragraph_length",
 AuditCategory::SubstantivelyDirty,
 4,
 ),
 ("max_section_body_length", AuditCategory::HardCase, 5),
 (
 "max_sentence_length",
 AuditCategory::SubstantivelyDirty,
 0,
 ),
 ];
 for (rule, cat, n) in expected {
 let actual = cell.get(&((*rule).into(), *cat)).copied().unwrap_or(0);
 assert_eq!(
 actual,
 *n,
 "rule={} cat={} drifted (expected {}, got {})",
 rule,
 cat.as_str(),
 n,
 actual
 );
 }
}

#[test]
#[ignore = "Round 251 — 7-md baseline anchors obsolete post MD-DELETION; audit reference only"]
fn audit_per_section_kind_anchored() {
 let v = collect_workspace_violations();
 let mut by_skind: BTreeMap<SectionKind, usize> = BTreeMap::new();
 for x in &v {
 *by_skind.entry(classify_section(&x.section_id)).or_default() += 1;
 }
 // Round 161 baseline — changelog_referenced 21 → 29 (+8: Changelog extend
 // 161 entry sub_bullets §N inline literals). other section_kinds stable
 // (reframe blocks treat the cross_doc count as ACCEPTABLE_CARRY scope; this
 // section_kind itself unchanged scope).
 // Round 170 baseline — top_level_numeric 12 → 14 (+2 from §66 closure
 // blockquote: max_section_body_length + cross_doc shift). Other section
 // kinds stable (ROADMAP +1 bullet routes through ChangelogReferenced
 // since slug contains "extension", count carries via 29 → re-anchored).
 // Round 241 baseline — atomic-first body source. ImpactScope 23 → 3 /
 // Changelog 3 → 0 / ChangelogReferenced 29 → 2 / TopLevelNumeric 14 → 7 /
 // NumericSubsection 31 → 7 / ProseNamed 89 → 21. ProseNamed compression
 // dominates (atomic decompose in inline prose scope → bullet block formas
 // transform, length-rule fire not done).
 let expected: &[(SectionKind, usize)] = &[
 (SectionKind::ImpactScope, 3),
 (SectionKind::Changelog, 0),
 (SectionKind::ChangelogReferenced, 2),
 (SectionKind::TopLevelNumeric, 7),
 (SectionKind::NumericSubsection, 7),
 (SectionKind::ProseNamed, 21),
 ];
 for (sk, n) in expected {
 let actual = by_skind.get(sk).copied().unwrap_or(0);
 assert_eq!(
 actual,
 *n,
 "section_kind={} drifted (expected {}, got {})",
 sk.as_str(),
 n,
 actual
 );
 }
}

/// Decision matrix anchor — re-anchored at Round 241 LEGACY-FIELD-REMOVAL
/// round 1 (atomic-first body source). Thresholds carry stable: ≥80%
/// acceptable → α / ≥50% dirty → γ / ≥30% hard → δ / else hybrid.
///
/// Round 241 result: ac=0.775 / dr=0.100 / hc=0.125 — **hybrid path**
/// (sample size for the atomic-first dirty/hard-case scope across the raw markdown body
/// shrinks; AC ratio is slightly below the α threshold (0.80). γ / δ trigger not invoked.
/// In LEGACY-FIELD-REMOVAL follow-up rounds (Round 242+), raw-body remnant counts split as
/// additional cleanup is possible on the α path (carry).
#[test]
#[ignore = "Round 251 — 7-md baseline anchors obsolete post MD-DELETION; audit reference only"]
fn audit_decision_matrix_path_anchored() {
 let v = collect_workspace_violations();
 let total = v.len() as f64;
 let mut by_cat: BTreeMap<AuditCategory, usize> = BTreeMap::new();
 for x in &v {
 *by_cat.entry(classify_violation(x)).or_default() += 1;
 }
 let ac = by_cat.get(&AuditCategory::AcceptableCarry).copied().unwrap_or(0) as f64 / total;
 let dr = by_cat
 .get(&AuditCategory::SubstantivelyDirty)
 .copied()
 .unwrap_or(0) as f64
 / total;
 let hc = by_cat.get(&AuditCategory::HardCase).copied().unwrap_or(0) as f64 / total;

 let alpha_triggered = ac >= 0.80;
 let gamma_triggered = dr >= 0.50;
 let delta_triggered = hc >= 0.30;

 assert!(!alpha_triggered, "α not triggered post-241 (ac={:.3} < 0.80)", ac);
 assert!(!gamma_triggered, "γ must NOT trigger (dr={:.3})", dr);
 assert!(!delta_triggered, "δ must NOT trigger (hc={:.3})", hc);

 // hybrid path active — atomic-first body source in ratios. 0.75 ≤ ac <
 // anchor at 0.80; adds an atomic-decompose round entry on the α carry signal k-category.
 assert!(ac >= 0.75, "hybrid path lower bound (ac={:.3})", ac);
 assert!(ac < 0.80, "hybrid path upper bound (ac={:.3})", ac);
}
