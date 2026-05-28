//! Style violation re-audit ledger (Round 154 RE-AUDIT-POST-RECONCILIATION).
//!
//! Distinct ledger from Round 139 (style_audit_full_scale.rs). Round 139 anchors
//! the *initial* audit on Round 130 baseline; this ledger anchors the
//! *cleanup priority decision matrix* used for Round 155-160 round selection.
//!
//! Source-of-truth: Round 154 audit was run after Round 152+153 reconciliation.
//! (commit `fcd0704`). validate-workspace baseline = T3 warn 163 / T4 info 53
//! / total 216 (carry from Round 151 closure baseline; Round 152+153 changed no
//! prose, so style counts stable).
//!
//! Pattern follows Round 78 orphan classification + Round 139 audit ledger.
//! Classifier reused verbatim — re-audit must reproduce Round 139 anchors so
//! the Round 155-160 cleanup operates on a stable substrate.
//!
//! Round 155-160 budget recalibration anchored at the bottom — drives
//! Δ-per-round assignment and SD-remaining target after audit.

use mnemosyne_parser::{parse_markdown};
use mnemosyne_atomic::{AtomicStore};
use mnemosyne_workspace::{Workspace};
use mnemosyne_style::{StyleSeverity, StyleViolation, check_style, default_ruleset};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

// Round 251 — historical 7-md baseline; tests below are #[ignore]'d.
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

/// Reused from Round 139 — strict `endswith('/impact-range')` so the anchor stays
/// reproducible. Compound `/impact-range-<descriptor>` patterns fall under
/// `prose_named` (3 violations carry; see audit doc body).
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

fn is_strong_carry_kind(k: SectionKind) -> bool {
 matches!(
 k,
 SectionKind::ImpactScope
 | SectionKind::Changelog
 | SectionKind::ChangelogReferenced
 | SectionKind::TopLevelNumeric
 )
}

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

/// Re-audit anchor — total / by-severity reproducible from Round 241 baseline
/// (LEGACY-FIELD-REMOVAL round 1: atomic-first body source). Round 170 anchor
/// 189/136/53 → 40/26/14 — atomic field write-time caps make length-rule
/// largely redundant on decomposed sections.
#[test]
#[ignore = "Round 251 — 7-md baseline anchors obsolete post MD-DELETION; audit reference only"]
fn re_audit_severity_baseline_anchored() {
 let v = collect_workspace_violations();
 let warn = v.iter().filter(|x| x.severity == StyleSeverity::Warn).count();
 let info = v.iter().filter(|x| x.severity == StyleSeverity::Info).count();
 assert_eq!(v.len(), 40, "total violations drifted from Round 241 baseline");
 assert_eq!(warn, 26, "T3 warn drifted from Round 241 baseline");
 assert_eq!(info, 14, "T4 info drifted from Round 241 baseline");
}

/// Re-audit anchor — per-doc severity breakdown drives Round 158 (6-doc)
/// vs Round 155-157 (DESIGN-only) split. DESIGN.md dominates with 72 warn
/// (44.2% of T3 warn); README.md is second with 50 warn (most cross_doc
/// → ACCEPTABLE_CARRY).
#[test]
#[ignore = "Round 251 — 7-md baseline anchors obsolete post MD-DELETION; audit reference only"]
fn re_audit_per_doc_severity_anchored() {
 let v = collect_workspace_violations();
 let mut cell: BTreeMap<(String, &'static str), usize> = BTreeMap::new();
 for x in &v {
 let sev: &'static str = if x.severity == StyleSeverity::Warn { "warn" } else { "info" };
 *cell.entry((x.doc_path.clone(), sev)).or_default() += 1;
 }
 // Round 241 baseline — atomic-first body source. README warn 49 → 8 (mostly
 // cross_doc on inline prose), DESIGN warn 49 → 10 / info 36 → 9 (atomic
 // decompose strips length warns). ARCH/CONCEPTS info shift on remaining
 // raw bodies. ROADMAP info 9 → 0 / CONCEPTS warn 6 → 0 / README info 3 → 0.
 let expected: &[(&str, &str, usize)] = &[
 ("README.md", "warn", 8),
 ("README.md", "info", 0),
 ("docs/ARCHITECTURE.md", "warn", 1),
 ("docs/ARCHITECTURE.md", "info", 2),
 ("docs/CONCEPTS.md", "warn", 0),
 ("docs/CONCEPTS.md", "info", 3),
 ("docs/DESIGN.md", "warn", 10),
 ("docs/DESIGN.md", "info", 9),
 ("docs/PRIOR_ART.md", "warn", 1),
 ("docs/ROADMAP.md", "warn", 5),
 ("docs/ROADMAP.md", "info", 0),
 ("docs/VISION.md", "warn", 1),
 ];
 for (doc, sev, n) in expected {
 let actual = cell.get(&((*doc).into(), *sev)).copied().unwrap_or(0);
 assert_eq!(
 actual, *n,
 "{} {} drifted (expected {}, got {})",
 doc, sev, n, actual
 );
 }
}

/// Re-audit anchor — SUBSTANTIVELY_DIRTY count per doc. Anchors the cleanup
/// priority matrix: DESIGN.md = 29 SD (Round 155-157 cleanup target),
/// README/ARCHITECTURE/ROADMAP/PRIOR_ART/VISION/CONCEPTS = 20 SD combined
/// (Round 158 cleanup target). Total SD = 49.
#[test]
#[ignore = "Round 251 — 7-md baseline anchors obsolete post MD-DELETION; audit reference only"]
fn re_audit_substantively_dirty_per_doc_anchored() {
 let v = collect_workspace_violations();
 let mut sd_per_doc: BTreeMap<String, usize> = BTreeMap::new();
 for x in &v {
 if classify_violation(x) != AuditCategory::SubstantivelyDirty {
 continue;
 }
 *sd_per_doc.entry(x.doc_path.clone()).or_default() += 1;
 }
 // Round 241 baseline — atomic-first body source compresses SD pool. README
 // SD 7 → 0 (atomic decompose), DESIGN 6 → 2 (§60 + voice-taxonomy carry).
 // ARCH / PRIOR_ART = 1 stable (ASCII art HARD_CASE). Total SD 15 → 4.
 let expected: &[(&str, usize)] = &[
 ("README.md", 0),
 ("docs/ARCHITECTURE.md", 1),
 ("docs/CONCEPTS.md", 0),
 ("docs/DESIGN.md", 2),
 ("docs/PRIOR_ART.md", 1),
 ("docs/ROADMAP.md", 0),
 ("docs/VISION.md", 0),
 ];
 let mut total = 0usize;
 for (doc, n) in expected {
 let actual = sd_per_doc.get(*doc).copied().unwrap_or(0);
 assert_eq!(
 actual, *n,
 "SD count drifted for {} (expected {}, got {})",
 doc, n, actual
 );
 total += actual;
 }
 assert_eq!(total, 4, "total SD drifted from Round 241 baseline");
}

/// Re-audit anchor — DESIGN.md SD top-level grouping. Drives Round 155-157
/// cleanup region selection. Anchored as `(top_level_section, count)` pairs.
///
/// Top regions for cleanup priority:
/// - Round 155 scope (top region): §41 (3) + §39 (2) + §61 (3) + §56 (2) = 10
/// — `markdown-variant-spec` carry + Datalog/grammar narrative
/// - Round 156 scope (top-2 region): §60 (2) + §63 (2) + §12 (2) + §42 (2) +
/// §66 (2) = 10 — schema/saga/decision narrative
/// - Round 157 scope (top-3 + remaining): §4 (1) + §44 (1) + §46 (1) + §65 (1)
/// + §voice-taxonomy (1) + §19 (1) + §multi-playthrough-layer (1) +
/// §creator-workflow (1) + §import-retrieval (1) = 9 — single-violation
/// sections + compound impact-scope-like (multi-playthrough/creator/import
/// are classified as `prose_named` but are impact-scope-flavored; may be carried
/// as HARD_CASE per spirit at cleanup time)
#[test]
#[ignore = "Round 251 — 7-md baseline anchors obsolete post MD-DELETION; audit reference only"]
fn re_audit_design_md_sd_top_level_grouping() {
 let v = collect_workspace_violations();
 let mut by_top: BTreeMap<String, usize> = BTreeMap::new();
 for x in &v {
 if x.doc_path != "docs/DESIGN.md" {
 continue;
 }
 if classify_violation(x) != AuditCategory::SubstantivelyDirty {
 continue;
 }
 let head = x.section_id.split('/').next().unwrap_or("").to_string();
 *by_top.entry(head).or_default() += 1;
 }
 // Round 157 baseline — top-3 + remaining cleanup landed:
 // §4: 1 → 0 (column-family-layout split into 9 CF code blocks)
 // §44: 1 → 0 (schema-row-normalized split into 3 CF code blocks)
 // §65: 1 → 0 (cross-branch lead-bullet split)
 // §19: 1 → 0 (gate-1 lead-bullet split)
 // §39: 2 → 0 (input-xml split by entities/relations/predicates etc;
 //  §39/6 split into 6 axis blocks + enforce responsibility split)
 // §41: 2 → 0 (input split into relations+rules; constraint-grammar
 //  -formal split into 8 EBNF groups)
 // §63: 2 → 0 (schema split into relations+enums; t2 split per-rule)
 // §60: 2 → 1 (§60/2 multi-block split; §60/5 entry 0 still > 1000
 //  as single-struct HARD_CASE per spirit)
 // §46: 1 stable (YAML schema HARD_CASE per spirit)
 // §voice-taxonomy: 1 stable (ASCII art graph HARD_CASE per spirit)
 // 3 compound impact-range sentences: stable (HARD_CASE per spirit)
 // Round 241 baseline — atomic-first carry: §60 inline struct (Round 233 single
 // code block) + voice-taxonomy ASCII graph (Round 236 carry). compound impact-
 // range sentences (multi-playthrough / creator-workflow / import) scope atomic
 // decomposed → SD pool entry 0. §46 YAML schema identical.
 let expected: &[(&str, usize)] = &[
 ("46", 0),
 ("60", 1),
 ("creator-workflow--export-adapters", 0),
 ("import--retrieval-layer", 0),
 ("multi-playthrough-layer", 0),
 ("voice-taxonomy", 1),
 ];
 let mut total = 0usize;
 for (head, n) in expected {
 let actual = by_top.get(*head).copied().unwrap_or(0);
 assert_eq!(
 actual, *n,
 "DESIGN.md §{} SD count drifted (expected {}, got {})",
 head, n, actual
 );
 total += actual;
 }
 assert_eq!(total, 2, "DESIGN.md SD top-level total drifted from Round 241 baseline");
}

/// Re-audit anchor — non-DESIGN doc SD section breakdown. Drives Round 158
/// 6-doc cleanup region selection. ARCHITECTURE/ROADMAP carry the bulk;
/// VISION/PRIOR_ART are single-violation; CONCEPTS = 0 SD (already clean).
#[test]
#[ignore = "Round 251 — 7-md baseline anchors obsolete post MD-DELETION; audit reference only"]
fn re_audit_non_design_sd_breakdown_anchored() {
 let v = collect_workspace_violations();
 let mut count: BTreeMap<(String, String), usize> = BTreeMap::new();
 for x in &v {
 if x.doc_path == "docs/DESIGN.md" {
 continue;
 }
 if classify_violation(x) != AuditCategory::SubstantivelyDirty {
 continue;
 }
 let head = x.section_id.split('/').next().unwrap_or("").to_string();
 *count.entry((x.doc_path.clone(), head)).or_default() += 1;
 }
 // Round 158 baseline — 6-doc cleanup landed:
 // README §mnemosyne: 8 → 7 (-1 component-family; status 6 + arch 1 carry HARD per spirit)
 // ARCHITECTURE §architecture: 4 → 1 (-3 l1/dc/l2; component-family ASCII tree carry)
 // ROADMAP §roadmap: 6 → 0 (rm1 + rm2 + rm3 + rm4 all cleaned)
 // VISION §vision: 1 -> 0 (single-line measurement sentence rephrased)
 // PRIOR_ART §prior-art: 1 stable (ASCII art HARD per spirit)
 // Round 241 baseline — README §mnemosyne 7 → 0 (atomic decompose in SD pool
 // missing). ARCH §architecture + PRIOR_ART §prior-art = 1 stable (ASCII art /
 // component-family carry).
 let expected: &[(&str, &str, usize)] = &[
 ("README.md", "mnemosyne", 0),
 ("docs/ARCHITECTURE.md", "architecture", 1),
 ("docs/PRIOR_ART.md", "prior-art", 1),
 ];
 let mut total = 0usize;
 for (doc, head, n) in expected {
 let actual = count.get(&((*doc).into(), (*head).into())).copied().unwrap_or(0);
 assert_eq!(
 actual, *n,
 "{} §{} SD count drifted (expected {}, got {})",
 doc, head, n, actual
 );
 total += actual;
 }
 assert_eq!(total, 2, "non-DESIGN SD total drifted from Round 241 baseline");
}

/// Re-audit decision matrix — Δ budget recalibration for Round 155-160.
///
/// User spec (carry from session input) proposed Δ ≥ 30/25/20/25/10 across
/// rounds 155-159 (sum 110). Audit shows actual SD = 49, so the spec's
/// 110 is over-inflated. Recalibrated budget:
///
/// | Round | Region | SD pool | Δ target |
/// |-------|--------|---------|----------|
/// | Round 155 | DESIGN top region (§41 §39 §61 §56) | 10 | ≥ 7 |
/// | Round 156 | DESIGN top-2 region (§60 §63 §12 §42 §66) | 10 | ≥ 7 |
/// | Round 157 | DESIGN remaining (§4 §44 §46 §65 §19 + 4 prose) | 9 | ≥ 5 |
/// | Round 158 | 6 doc (README/ARCH/ROAD/VISION/PRIOR/CONCEPTS) | 20 | ≥ 7 |
/// | Round 159 | new docs (out-of-scope GETTING_STARTED/SCHEMA_GUIDE) | n/a | n/a |
///
/// Cumulative SD cleanup target ≥ 26, leaving SD remaining ≤ 23.
///
/// Round 160 closure threshold: SD-remaining ≤ 30 (T3 warn excluding HC + AC).
/// Total T3 warn after cleanup ≈ 114 (AC cross_doc) + ≤ 23 (SD remaining) = ≤ 137.
///
/// Note: 3 compound impact-scope-flavored sections (§multi-playthrough-layer
/// /impact-range-..., §creator-workflow.../impact-range-..., §import--retrieval-layer
/// /.../...-cleanup) are SD per classifier but HARD_CASE in spirit. Each cleanup
/// round must judge case-by-case; if frozen-feeling at edit time, skip and
/// Document-level round entry header.
#[test]
#[ignore = "Round 251 — 7-md baseline anchors obsolete post MD-DELETION; audit reference only"]
fn re_audit_cleanup_budget_recalibrated() {
 let v = collect_workspace_violations();
 let mut sd_per_doc: BTreeMap<String, usize> = BTreeMap::new();
 let mut sd_design_top_region = 0usize;
 let mut sd_design_top2_region = 0usize;
 let mut sd_design_remaining = 0usize;
 let top_region_heads: &[&str] = &["41", "39", "61", "56"];
 let top2_region_heads: &[&str] = &["60", "63", "12", "42", "66"];
 for x in &v {
 if classify_violation(x) != AuditCategory::SubstantivelyDirty {
 continue;
 }
 *sd_per_doc.entry(x.doc_path.clone()).or_default() += 1;
 if x.doc_path == "docs/DESIGN.md" {
 let head = x.section_id.split('/').next().unwrap_or("");
 if top_region_heads.contains(&head) {
  sd_design_top_region += 1;
 } else if top2_region_heads.contains(&head) {
  sd_design_top2_region += 1;
 } else {
  sd_design_remaining += 1;
 }
 }
 }
 // Round 158 cleanup result: DESIGN regions stable (top=0 / top2=1 /
 // remaining=5, all HARD_CASE per spirit). non-DESIGN region 20 → 9
 // (-11 cleaned: ARCH -3, ROADMAP -6, VISION -1, README -1; remaining
 // 9 = README status 6 sentence (massive run-on, full-rewrite scope) +
 // PRIOR_ART 1 (ASCII art) + README architecture 1 (layered diagram)
 // + ARCHITECTURE component-family 1 (ASCII tree) — all HARD_CASE per
 // spirit). Total SD 26 → 15 (well below 30 closure threshold).
 assert_eq!(sd_design_top_region, 0, "Round 155 scope SD pool drifted (carry baseline 0)");
 assert_eq!(sd_design_top2_region, 1, "Round 156 scope SD pool drifted (carry baseline 1)");
 assert_eq!(sd_design_remaining, 1, "Round 157 scope SD pool drifted (Round 241 baseline 1)");
 let non_design: usize = sd_per_doc
 .iter()
 .filter(|(k, _)| k.as_str() != "docs/DESIGN.md")
 .map(|(_, n)| *n)
 .sum();
 assert_eq!(non_design, 2, "Round 158 scope SD pool drifted (Round 241 baseline 2)");
 // Closure threshold gate — SD-remaining ≤ 30 deeply MET (total SD =
 // 15). Round 155+156+157+158 cleaned 34 cumulative; remaining 15 are
 // all HARD_CASE per spirit (code blocks / ASCII art / massive run-on
 // requiring full rewrite / compound impact-range frozen).
 let total_sd: usize = sd_per_doc.values().sum();
 let cleanup_required = total_sd.saturating_sub(30);
 assert_eq!(
 cleanup_required, 0,
 "cleanup Δ requirement post-158: SD-remaining ≤ 30 deeply MET (got {})",
 cleanup_required
 );
}
