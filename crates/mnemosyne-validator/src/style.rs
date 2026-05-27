//! Style rule layer (T3/T4 — MD-quality validator) — spec ratify carry,
//! production wire (DESIGN.md *Style rule layer (T3/T4 — MD-quality)*
//! sub-section + Stage 1 dogfood depth carry).
//!
//! Layer responsibility: prose property check on parsed design_doc workspace.
//! Parallel to T1/T2 (typed-fact inference) layer in — different validation
//! dimension (prose readability vs typed-fact consistency).
//!
//! Reject power: none. T3 = warn surface (override possible), T4 = info surface
//! (author-checkpoint flag only). signal-4 carry — LLM-eval / cosine
//! similarity / classifier output is forbidden in this layer; only deterministic
//! check kinds (char_count / jaccard / regex / lookup) are admissible.
//!
//! Rule catalog (closed-form, ratify):
//!
//! T3 (warn):
//! - `max_paragraph_length` (default 1000 char) — single paragraph char count
//! - `max_sentence_length` (default 200 char) — single sentence char count
//! - `terminology_consistency` — workspace glossary lookup
//! - `cross_doc_reference_explicit` — cross-doc reference syntax
//!
//! T4 (info):
//! - `boilerplate_repetition_jaccard` (default 0.7) — ChangelogEntry sub_bullets pairwise jaccard
//! - `max_section_body_length` (default 5000 char) — section body char count
//! - `bullet_list_preference` — enumeration pattern detection in run-on paragraphs

use crate::atomic::{AtomicSection, AtomicStore};
use crate::schema::{ChangelogEntry, ParsedDoc, Section};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleTier {
 T3,
 T4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleSeverity {
 Warn,
 Info,
}

impl StyleSeverity {
 pub fn from_tier(t: StyleTier) -> Self {
 match t {
 StyleTier::T3 => StyleSeverity::Warn,
 StyleTier::T4 => StyleSeverity::Info,
 }
 }
}

#[derive(Debug, Clone, PartialEq)]
pub enum StyleThreshold {
 /// Character count cap (length-style rules).
 CharCount(usize),
 /// Jaccard similarity cap (boilerplate detection).
 Jaccard(f64),
 /// Workspace glossary — canonical → variant_set (variant etc. paragraph on warn).
 GlossaryLookup(BTreeMap<String, BTreeSet<String>>),
 /// Cross-doc reference required-form regex tag (placeholder — match against
 /// known doc names without `#§` syntax).
 CrossDocReferenceExplicit,
 /// Enumeration pattern detector (`(i)`/`(ii)`/`1.`/`first/second` etc.).
 EnumerationPattern,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleScope {
 SectionBody,
 ChangelogSubBullets,
 FullDoc,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StyleRule {
 pub rule_id: String,
 pub tier: StyleTier,
 pub threshold: StyleThreshold,
 pub scope: StyleScope,
 pub rationale: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StyleViolation {
 pub rule_id: String,
 pub doc_path: String,
 pub section_id: String,
 pub line_anchor: Option<usize>,
 pub severity: StyleSeverity,
 pub message: String,
 pub suggested_fix: Option<String>,
}

/// Workspace terminology glossary (measurement-driven inventory,
/// conservative carry, config-driven entry point).
///
/// The first inventory listed five entries; the review
/// drops three (`design_doc`/`design doc`, `cascade_query`/`cascade query`,
/// `Forge`/`forge`) because the variants name **different things** rather
/// than misspellings. The Mnemosyne preset retains the unambiguous pair.
///
/// external users override via
/// `mnemosyne.toml::[terminology.glossary]`. This factory returns the
/// Mnemosyne self-application preset; [`glossary_from_config`] takes a
/// `TerminologySection` and produces a parser-shape map.
pub fn workspace_glossary() -> BTreeMap<String, BTreeSet<String>> {
 let mut g = BTreeMap::new();
 let mut put = |canonical: &str, variants: &[&str]| {
 let set: BTreeSet<String> = variants.iter().map(|s| (*s).to_string()).collect();
 g.insert(canonical.to_string(), set);
 };
 put("Salsa", &["salsa"]);
 put("bi-temporal", &["bitemporal"]);
 g
}

/// convert a `TerminologySection` into the parser's glossary
/// shape. Empty config → empty glossary (terminology rule disabled).
pub fn glossary_from_config(
 config: &crate::config::TerminologySection,
) -> BTreeMap<String, BTreeSet<String>> {
 let mut g = BTreeMap::new();
 for (canonical, variants) in &config.glossary {
 let set: BTreeSet<String> = variants.iter().cloned().collect();
 g.insert(canonical.clone(), set);
 }
 g
}

/// config-driven ruleset factory. Per-rule char count thresholds
/// override compile-time defaults via `style.thresholds`; glossary populates
/// from `[terminology.glossary]`. Empty glossary disables the
/// `terminology_consistency` rule's reject power without removing the rule.
pub fn default_ruleset_with_config(
 style: Option<&crate::config::StyleSection>,
 terminology: Option<&crate::config::TerminologySection>,
) -> Vec<StyleRule> {
 let glossary = match terminology {
 Some(t) if !t.glossary.is_empty() => glossary_from_config(t),
 _ => workspace_glossary(),
 };
 let mut rules = default_ruleset_with_glossary(glossary);
 if let Some(s) = style {
 for rule in rules.iter_mut() {
 if let Some(&override_threshold) = s.thresholds.get(&rule.rule_id) {
  if let StyleThreshold::CharCount(_) = rule.threshold {
  rule.threshold = StyleThreshold::CharCount(override_threshold as usize);
  }
 }
 }
 }
 rules
}

/// build the default ruleset with a custom glossary. The
/// glossary argument replaces the Mnemosyne preset (Salsa / bi-temporal);
/// pass an empty BTreeMap to disable terminology checks.
fn default_ruleset_with_glossary(
 glossary: BTreeMap<String, BTreeSet<String>>,
) -> Vec<StyleRule> {
 let mut rules = default_ruleset();
 for rule in rules.iter_mut() {
 if rule.rule_id == "terminology_consistency" {
 rule.threshold = StyleThreshold::GlossaryLookup(glossary.clone());
 }
 }
 rules
}

/// Default ruleset — closed-form catalog (detector tightening
/// + threshold tuning carry: max_sentence_length 200 → 300 char, em-dash
/// subclause in effective length, strong-carry section skip).
/// `terminology_consistency` glossary populated at measurement round.
pub fn default_ruleset() -> Vec<StyleRule> {
 vec![
 StyleRule {
 rule_id: "max_paragraph_length".into(),
 tier: StyleTier::T3,
 threshold: StyleThreshold::CharCount(1000),
 scope: StyleScope::SectionBody,
 rationale: "paragraph single char threshold in run-on prose detect (Round 128 ratify).".into(),
 },
 StyleRule {
 rule_id: "max_sentence_length".into(),
 tier: StyleTier::T3,
 threshold: StyleThreshold::CharCount(300),
 scope: StyleScope::SectionBody,
 rationale: "sentence effective length cap, em-dash splits clauses (Round 140 ratify — bumped 200 -> 300 to fit technical-prose syllable density; em-dash multi-clause subclause detection).".into(),
 },
 StyleRule {
 rule_id: "terminology_consistency".into(),
 tier: StyleTier::T3,
 threshold: StyleThreshold::GlossaryLookup(workspace_glossary()),
 scope: StyleScope::FullDoc,
 rationale: "workspace glossary in consistent notation (Round 130 inventory carry).".into(),
 },
 StyleRule {
 rule_id: "cross_doc_reference_explicit".into(),
 tier: StyleTier::T3,
 threshold: StyleThreshold::CrossDocReferenceExplicit,
 scope: StyleScope::SectionBody,
 rationale: "cross-doc reference format consistency (Round 70 OPTION H-2 carry).".into(),
 },
 StyleRule {
 rule_id: "boilerplate_repetition_jaccard".into(),
 tier: StyleTier::T4,
 threshold: StyleThreshold::Jaccard(0.7),
 scope: StyleScope::ChangelogSubBullets,
 rationale: "ChangelogEntry sub_bullets use jaccard threshold boilerplate detect.".into(),
 },
 StyleRule {
 rule_id: "max_section_body_length".into(),
 tier: StyleTier::T4,
 threshold: StyleThreshold::CharCount(5000),
 scope: StyleScope::SectionBody,
 rationale: "Section.body char count threshold in split recommended (info).".into(),
 },
 StyleRule {
 rule_id: "bullet_list_preference".into(),
 tier: StyleTier::T4,
 threshold: StyleThreshold::EnumerationPattern,
 scope: StyleScope::SectionBody,
 rationale: "paragraph in enumeration pattern detect + bullet transform suggestion.".into(),
 },
 ]
}

/// Strong-carry section — frozen-feeling section bodies where length-rule
/// edits risk T2 frozen-ledger jaccard drift or carry-trail readability loss.
/// Mirrors the audit ledger classifier so
/// production detection skips the same regions the audit identified as
/// HARD_CASE. `terminology_consistency` and `cross_doc_reference_explicit`
/// continue to apply universally — they don't risk semantic drift.
pub fn is_strong_carry_section(section_id: &str) -> bool {
 if section_id.ends_with("/impact-range") {
 return true;
 }
 let lower = section_id.to_ascii_lowercase();
 if lower.contains("changelog") || section_id.contains("change-history") {
 return true;
 }
 let lower_id = section_id.to_ascii_lowercase();
 if lower_id.contains("round-")
 || lower_id.contains("/round-")
 || lower_id.contains("-round-")
 || section_id.contains("extension")
 {
 return true;
 }
 let head = section_id.split('/').next().unwrap_or("");
 let is_top_level_numeric = !section_id.contains('/')
 && !head.is_empty()
 && head.chars().all(|c| c.is_ascii_digit() || c == '.');
 is_top_level_numeric
}

/// True when the rule should skip strong-carry sections. Length rules
/// (`max_paragraph_length` / `max_sentence_length`) skip; terminology +
/// cross-doc reference checks continue to apply.
fn rule_skips_strong_carry(rule_id: &str) -> bool {
 matches!(rule_id, "max_paragraph_length" | "max_sentence_length")
}

/// true when `section_id` is a descendant of (or is) the
/// changelog area. Used by `check_style` to skip rules whose violations
/// inside frozen changelog entries can never be acted on (the entry text
/// (append-only per frozen ledger).
///
/// Pattern: section_id contains the changelog title slug. Matches both
/// the bare `change-history` section and any nested per-entry slug like
/// `<root>/change-history-atomic-ledger/extension-N--...`.
fn is_changelog_area_section(section_id: &str) -> bool {
 let lower = section_id.to_ascii_lowercase();
 lower.contains("changelog") || section_id.contains("change-history")
}

/// true when the rule's violations are unactionable inside
/// frozen changelog entries (atomic store ledger). `terminology_consistency`
/// fires on legacy variant text (`salsa` / `bitemporal`) embedded in
/// historical entries' decision_summary/changes/etc; those entries are
/// frozen by ledger and cannot be retroactively fixed. The
/// canonical-form contract still applies to live spec body via
/// `parsed.sections` outside the changelog area.
fn rule_skips_changelog_area(rule_id: &str) -> bool {
 matches!(rule_id, "terminology_consistency")
}

pub fn check_style(
 doc_path: &str,
 parsed: &ParsedDoc,
 atomic_store: &AtomicStore,
 ruleset: &[StyleRule],
) -> Vec<StyleViolation> {
 let mut out = Vec::new();
 for rule in ruleset {
 match rule.scope {
 StyleScope::SectionBody | StyleScope::FullDoc => {
  for section in &parsed.sections {
  if rule_skips_strong_carry(&rule.rule_id)
  && is_strong_carry_section(&section.section_id)
  {
  continue;
  }
 // skip terminology check on changelog-area
 // sections. Frozen ledger entries (atomic store) cannot
 // be retroactively fixed; flagged variants stay as
 // *historical* text. Live spec body outside changelog
 // still gets full terminology coverage.
  if rule_skips_changelog_area(&rule.rule_id)
  && is_changelog_area_section(&section.section_id)
  {
  continue;
  }
  let body = resolve_section_body(parsed, atomic_store, section);
  if let Some(body) = body {
  check_section_body_rule(
   doc_path,
   section,
   &body,
   parsed.line_anchors.get(&section.section_id).copied(),
   rule,
   &mut out,
  );
  }
  }
 }
 StyleScope::ChangelogSubBullets => {
  for entry in &parsed.changelog_entries {
  check_changelog_entry_rule(doc_path, entry, atomic_store, rule, &mut out);
  }
 }
 }
 }
 out
}

/// Resolve the prose body for a section's style checks. atomic-first source
///: if the atomic store has an entry
/// for this `section`, synthesize a prose body via
/// [`crate::atomic::synthesize_section_prose_body`] (excludes mechanical
/// citation blocks like `implementations` file paths — see that function's
/// doc for the category rationale); otherwise fall back to the legacy
/// `parsed.bodies` map for sections that have not yet been
/// atomic-decomposed. Both branches return `None` when no source exists
/// (decomposed-but-empty atomic section also yields a synthesized empty
/// string `""`, which `check_section_body_rule` treats as a no-op).
///
/// Atomic lookup goes through [`AtomicStore::resolve`], which honours the
/// parser's `atomic_section_id` bridge — the bare heading `§<token>` slot
/// — instead of the parent-prefixed `section_id`. Without that bridge,
/// nested `### §<id>` headings (the renderer's depth-3 layout under
/// `## Sections`) miss the atomic store and silently fall back to the
/// raw markdown body, defeating mechanical-citation exclusions like the
/// `implementations` file-path filter.
fn resolve_section_body(
 parsed: &ParsedDoc,
 atomic_store: &AtomicStore,
 section: &Section,
) -> Option<String> {
 if let Some(atomic) = atomic_store.resolve(section) {
 return Some(synthesize_atomic_body(atomic));
 }
 parsed.bodies.get(&section.section_id).cloned()
}

// Style-check body synthesizer. Uses the prose-only variant so that
// mechanical citation blocks (Section.implementations file paths) do not
// participate in prose rules like `terminology_consistency`. Path-shaped
// identifiers follow Unix/C filesystem conventions (lowercase) regardless
// of the canonical prose form of the same concept (e.g. `dut/...` vs the
// canonical `DUT` glossary form). query.rs continues to use
// [`crate::atomic::synthesize_section_body`] (the full variant) for
// SectionView.body, where downstream consumers want the rendered citations.
fn synthesize_atomic_body(atomic: &AtomicSection) -> String {
 crate::atomic::synthesize_section_prose_body(atomic)
}

fn check_section_body_rule(
 doc_path: &str,
 section: &Section,
 body: &str,
 line_anchor: Option<usize>,
 rule: &StyleRule,
 out: &mut Vec<StyleViolation>,
) {
 match (&rule.threshold, rule.rule_id.as_str()) {
 (StyleThreshold::CharCount(cap), "max_paragraph_length") => {
 for para in split_paragraphs(body) {
  let len = para.chars().count();
  if len > *cap {
  out.push(make_violation(
  rule,
  doc_path,
  &section.section_id,
  line_anchor,
  format!(
   "paragraph length {} > {} (run-on detected — split into bullets or shorter paragraphs)",
   len, cap
  ),
  Some("split into shorter paragraphs or convert to bullet list".into()),
  ));
  }
 }
 }
 (StyleThreshold::CharCount(cap), "max_sentence_length") => {
 for sentence in split_sentences(body) {
  let len = effective_sentence_length(&sentence);
  if len > *cap {
  out.push(make_violation(
  rule,
  doc_path,
  &section.section_id,
  line_anchor,
  format!(
   "sentence effective length {} > {} (split required — em-dash subclause counted independently)",
   len, cap
  ),
  Some("split into shorter sentences (em-dash subclause already counted independently)".into()),
  ));
  }
 }
 }
 (StyleThreshold::CharCount(cap), "max_section_body_length") => {
 let len = body.chars().count();
 if len > *cap {
  out.push(make_violation(
  rule,
  doc_path,
  &section.section_id,
  line_anchor,
  format!(
  "section body length {} > {} (consider splitting into sub-sections)",
  len, cap
  ),
  Some("split into sub-sections via add_section".into()),
  ));
 }
 }
 (StyleThreshold::GlossaryLookup(glossary), "terminology_consistency") => {
 if !glossary.is_empty() {
  for (canonical, variants) in glossary {
  for variant in variants {
  if body.contains(variant.as_str()) && variant != canonical {
   out.push(make_violation(
   rule,
   doc_path,
   &section.section_id,
   line_anchor,
   format!(
   "terminology variant `{}` found — use canonical `{}`",
   variant, canonical
   ),
   Some(format!("replace `{}` with `{}`", variant, canonical)),
   ));
  }
  }
  }
 }
 }
 (StyleThreshold::CrossDocReferenceExplicit, "cross_doc_reference_explicit") => {
 for occ in detect_implicit_cross_doc_references(body, doc_path) {
  out.push(make_violation(
  rule,
  doc_path,
  &section.section_id,
  line_anchor,
  format!(
  "implicit cross-doc reference `{}` — use `{{doc}}#§N` or `§N` form",
  occ
  ),
  Some("rewrite as `{doc}#§N` (Round 70 OPTION H-2 carry)".into()),
  ));
 }
 }
 (StyleThreshold::EnumerationPattern, "bullet_list_preference") => {
 for para in split_paragraphs(body) {
  if has_enumeration_pattern(para) {
  out.push(make_violation(
  rule,
  doc_path,
  &section.section_id,
  line_anchor,
  "enumeration pattern detected in paragraph — convert to bullet list".into(),
  Some("convert `(i)/(ii)/(iii)` or `1./2./3.` enumerations to `- ` bullets".into()),
  ));
  }
 }
 }
 _ => {}
 }
}

fn check_changelog_entry_rule(
 doc_path: &str,
 entry: &ChangelogEntry,
 atomic_store: &AtomicStore,
 rule: &StyleRule,
 out: &mut Vec<StyleViolation>,
) {
 if let (StyleThreshold::Jaccard(cap), "boilerplate_repetition_jaccard") =
 (&rule.threshold, rule.rule_id.as_str())
 {
 // atomic-first source: atomic ChangelogEntry changes_bullets
 // existswhen atomic source in jaccard check. atomic entry missing or
 // changes_bullets when empty legacy `entry.sub_bullets` fallback (LEGACY-
 // FIELD-REMOVAL round 2 carry; sub_bullets field self-carries stable
 // until round 3 explicit removal).
 let (bullets_owned, source_label): (Vec<String>, &'static str) =
 match atomic_store.entry(&entry.entry_id) {
  Some(atomic) if !atomic.changes_bullets.is_empty() => {
  (atomic.changes_bullets.clone(), "changes_bullets")
  }
  _ => (entry.sub_bullets.clone(), "sub_bullets"),
 };
 let bullets: &[String] = &bullets_owned;
 for i in 0..bullets.len() {
 for j in (i + 1)..bullets.len() {
  let sim = word_jaccard(&bullets[i], &bullets[j]);
  if sim >= *cap {
  out.push(make_violation(
  rule,
  doc_path,
  &entry.entry_id,
  None,
  format!(
   "{}[{}] vs {}[{}] jaccard = {:.3} >= {:.3} (boilerplate)",
   source_label, i, source_label, j, sim, cap
  ),
  Some("rewrite one bullet to remove repeated phrasing".into()),
  ));
  }
 }
 }
 }
}

fn make_violation(
 rule: &StyleRule,
 doc_path: &str,
 section_id: &str,
 line_anchor: Option<usize>,
 message: String,
 suggested_fix: Option<String>,
) -> StyleViolation {
 StyleViolation {
 rule_id: rule.rule_id.clone(),
 doc_path: doc_path.to_string(),
 section_id: section_id.to_string(),
 line_anchor,
 severity: StyleSeverity::from_tier(rule.tier),
 message,
 suggested_fix,
 }
}

/// Split body into paragraphs (blank-line delimited). Code-fenced blocks count
/// as a single paragraph (skip — fences are intentionally long).
fn split_paragraphs(body: &str) -> Vec<&str> {
 let mut out = Vec::new();
 let mut start = 0usize;
 let mut prev_blank = true;
 let mut in_code = false;
 let bytes = body.as_bytes();
 let mut line_start = 0usize;
 while line_start <= bytes.len() {
 let line_end = bytes[line_start..]
 .iter()
 .position(|&b| b == b'\n')
 .map(|n| line_start + n)
 .unwrap_or(bytes.len());
 let line = &body[line_start..line_end];
 let trimmed = line.trim();
 if trimmed.starts_with("```") {
 in_code = !in_code;
 }
 if !in_code && trimmed.is_empty() {
 if !prev_blank && start < line_start {
  let para = body[start..line_start].trim();
  if !para.is_empty() && !is_only_code_or_table(para) {
  out.push(&body[start..line_start]);
  }
 }
 start = line_end + 1;
 prev_blank = true;
 } else {
 prev_blank = false;
 }
 if line_end >= bytes.len() {
 break;
 }
 line_start = line_end + 1;
 }
 if start < bytes.len() && !in_code {
 let para = body[start..].trim();
 if !para.is_empty() && !is_only_code_or_table(para) {
 out.push(&body[start..]);
 }
 }
 out
}

/// Suppress paragraph-level checks for table rows / fenced blocks / list-only blocks.
/// (Length rules target prose, not tables or code.)
fn is_only_code_or_table(para: &str) -> bool {
 let mut all_table = true;
 let mut all_list = true;
 let mut empty_lines = 0;
 let mut total = 0;
 for line in para.lines() {
 let t = line.trim_start();
 total += 1;
 if t.is_empty() {
 empty_lines += 1;
 continue;
 }
 if !t.starts_with('|') && !t.starts_with("```") {
 all_table = false;
 }
 if !(t.starts_with("- ") || t.starts_with("* ") || t.starts_with(char::is_numeric)) {
 all_list = false;
 }
 }
 if total == empty_lines {
 return true;
 }
 all_table || all_list
}

/// Sentence splitter — `. ` / `! ` / `? ` followed by a-zA-Z or a Hangul onset.
/// Code fences and table rows are skipped (returned as a single sentence,
/// which the length check then ignores via len cap).
fn split_sentences(body: &str) -> Vec<String> {
 let mut out = Vec::new();
 let mut in_code = false;
 for raw_para in body.split("\n\n") {
 let para = raw_para.trim();
 if para.is_empty() {
 continue;
 }
 if para.starts_with("```") {
 in_code = !in_code;
 continue;
 }
 if in_code || is_only_code_or_table(para) {
 continue;
 }
 let mut current = String::new();
 let mut chars = para.chars().peekable();
 while let Some(ch) = chars.next() {
 current.push(ch);
 if matches!(ch, '.' | '!' | '?') {
  if let Some(&next) = chars.peek() {
  if next == ' ' || next == '\n' {
  let after_space: String = chars.clone().skip(1).take(1).collect();
  let starts_new = after_space
   .chars()
   .next()
   .map(|c| c.is_alphanumeric() || is_korean(c))
   .unwrap_or(false);
  if starts_new {
   out.push(current.trim().to_string());
   current.clear();
  }
  }
  }
 }
 }
 let last = current.trim();
 if !last.is_empty() {
 out.push(last.to_string());
 }
 }
 out
}

fn is_korean(c: char) -> bool {
 matches!(c as u32, 0xAC00..=0xD7AF)
}

/// Effective sentence length for `max_sentence_length` (detector
/// tightening). Korean technical prose chains multiple semantic clauses with
/// em-dashes (`A — B — C`) inside what reads as a single author-intended
/// sentence; counting the whole chain inflates length false-positively. This
/// helper measures the longest em-dash subclause instead, so the rule fires
/// only when an individual clause genuinely exceeds the threshold.
pub fn effective_sentence_length(sentence: &str) -> usize {
 let has_dash = sentence.chars().any(|c| c == '—' || c == '–');
 if !has_dash {
 return sentence.chars().count();
 }
 sentence
 .split(['—', '–'])
 .map(|s| s.trim().chars().count())
 .max()
 .unwrap_or(0)
}

/// Word-level jaccard. Tokens = whitespace split + lowercase + strip punctuation.
fn word_jaccard(a: &str, b: &str) -> f64 {
 let sa: BTreeSet<String> = a
 .split_whitespace()
 .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase())
 .filter(|w| !w.is_empty())
 .collect();
 let sb: BTreeSet<String> = b
 .split_whitespace()
 .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase())
 .filter(|w| !w.is_empty())
 .collect();
 if sa.is_empty() && sb.is_empty() {
 return 0.0;
 }
 let inter = sa.intersection(&sb).count() as f64;
 let union = sa.union(&sb).count() as f64;
 if union == 0.0 {
 0.0
 } else {
 inter / union
 }
}

const KNOWN_DOCS: &[&str] = &[
 "DESIGN.md",
 "ARCHITECTURE.md",
 "ROADMAP.md",
 "VISION.md",
 "CONCEPTS.md",
 "README.md",
 "PRIOR_ART.md",
];

/// Heuristic — informal cross-doc references like "see DESIGN" without
/// `#§N` anchor. Skips the following non-navigational contexts (/// false-positive triage):
/// - inside backticks (already exempt before)
/// - reference to the parser's own doc (self-reference is not cross-doc)
/// - followed by `#` (already anchored)
/// - immediately followed by a Korean nominal particle (eu / e / an / wi / deung / man / do / gwa / wa / eun / neun / i / ga / eul / reul / ro)
/// or by `Round N` source-of-truth annotations — these are noun usages, not
/// navigational pointers
fn detect_implicit_cross_doc_references(body: &str, parent_doc_path: &str) -> Vec<String> {
 let mut hits = Vec::new();
 for doc in KNOWN_DOCS {
 if parent_doc_path.ends_with(doc) {
 continue;
 }
 let mut idx = 0;
 while let Some(pos) = body[idx..].find(doc) {
 let abs = idx + pos;
 let after = &body[abs + doc.len()..];
 let next_char = after.chars().next();
 if next_char == Some('#') {
  idx = abs + doc.len();
  continue;
 }
 if is_inside_backticks(body, abs) {
  idx = abs + doc.len();
  continue;
 }
 if is_followed_by_nominal_context(after) {
  idx = abs + doc.len();
  continue;
 }
 hits.push(doc.to_string());
 idx = abs + doc.len();
 }
 }
 hits
}

/// True if the next non-space token signals noun/source usage rather than
/// navigation. English-only after; Korean particle detection
/// retired (workspace is English-only, AI-consumed; Korean prose is no
/// longer expected anywhere in the validated content).
fn is_followed_by_nominal_context(after: &str) -> bool {
 let t = after.trim_start();
 t.starts_with("extension")
 || t.starts_with("source")
 || t.starts_with("itself")
 || t.starts_with("§")
 || t.starts_with("from")
}

fn is_inside_backticks(body: &str, pos: usize) -> bool {
 let before = &body[..pos];
 let count = before.matches('`').count();
 count % 2 == 1
}

/// Detect `(i)/(ii)/(iii)` or `1./2./3.` or `first/second/third` style enumerations
/// embedded in a single paragraph (≥ 3 markers).
fn has_enumeration_pattern(para: &str) -> bool {
 let count_paren_roman = ["(i)", "(ii)", "(iii)", "(iv)", "(v)"]
 .iter()
 .filter(|m| para.contains(*m))
 .count();
 if count_paren_roman >= 3 {
 return true;
 }
 let mut count_numbered = 0;
 for n in 1..=9 {
 let needle1 = format!("{}.", n);
 let needle2 = format!("{})", n);
 if para.contains(&needle1) || para.contains(&needle2) {
 count_numbered += 1;
 }
 }
 if count_numbered >= 3 {
 return true;
 }
 let count_korean_ord = ["first", "second", "third", "fourth"]
 .iter()
 .filter(|m| para.contains(*m))
 .count();
 count_korean_ord >= 3
}

#[cfg(test)]
mod tests {
 use super::*;

 fn empty_doc() -> ParsedDoc {
 ParsedDoc::default()
 }

 fn empty_store() -> AtomicStore {
 AtomicStore::default()
 }

 #[test]
 fn default_ruleset_has_seven_rules() {
 let rs = default_ruleset();
 assert_eq!(rs.len(), 7);
 let ids: Vec<&str> = rs.iter().map(|r| r.rule_id.as_str()).collect();
 assert!(ids.contains(&"max_paragraph_length"));
 assert!(ids.contains(&"max_sentence_length"));
 assert!(ids.contains(&"terminology_consistency"));
 assert!(ids.contains(&"cross_doc_reference_explicit"));
 assert!(ids.contains(&"boilerplate_repetition_jaccard"));
 assert!(ids.contains(&"max_section_body_length"));
 assert!(ids.contains(&"bullet_list_preference"));
 }

 #[test]
 fn check_style_empty_doc_no_violations() {
 let doc = empty_doc();
 let v = check_style("docs/EMPTY.md", &doc, &empty_store(), &default_ruleset());
 assert_eq!(v.len(), 0);
 }

 #[test]
 fn max_paragraph_length_detects_run_on() {
 let mut doc = ParsedDoc::default();
 // Use a prose_named section so the strong-carry skip does not apply.
 doc.sections.push(Section {
 section_id: "test/prose-section".into(),
 parent_doc: "TEST".into(),
 parent_section: None,
 title: "Test".into(),
 decision_status: mnemosyne_core::DecisionStatus::Active,
 atomic_section_id: None,
 });
 let long = "-".repeat(1500);
 doc.bodies.insert("test/prose-section".into(), long);
 let v = check_style("TEST.md", &doc, &empty_store(), &default_ruleset());
 assert!(v.iter().any(|x| x.rule_id == "max_paragraph_length"));
 }

 #[test]
 fn strong_carry_section_skip_length_rules() {
 let mut doc = ParsedDoc::default();
 // top_level_numeric — strong-carry, length rules must skip.
 doc.sections.push(Section {
 section_id: "43".into(),
 parent_doc: "TEST".into(),
 parent_section: None,
 title: "Test".into(),
 decision_status: mnemosyne_core::DecisionStatus::Active,
 atomic_section_id: None,
 });
 let long = "-".repeat(2000);
 doc.bodies.insert("43".into(), long);
 let v = check_style("TEST.md", &doc, &empty_store(), &default_ruleset());
 assert!(
 !v.iter().any(|x| x.rule_id == "max_paragraph_length"),
 "max_paragraph_length must skip top_level_numeric strong-carry"
 );
 assert!(
 !v.iter().any(|x| x.rule_id == "max_sentence_length"),
 "max_sentence_length must skip top_level_numeric strong-carry"
 );
 }

 #[test]
 fn strong_carry_classifier_anchors() {
 assert!(is_strong_carry_section("41/impact-range"));
 assert!(is_strong_carry_section("voice-taxonomy/change-history"));
 assert!(is_strong_carry_section("15/spec-mutate-api-surface-extension-123-ratify"));
 assert!(is_strong_carry_section("43"));
 assert!(is_strong_carry_section("1.5"));
 assert!(!is_strong_carry_section("test/prose-section"));
 assert!(!is_strong_carry_section("mnemosyne/status"));
 assert!(!is_strong_carry_section("41/rationale"));
 }

 #[test]
 fn effective_sentence_length_em_dash_subclause() {
 // No dash — full length.
 let plain = "-".repeat(150);
 assert_eq!(effective_sentence_length(&plain), 150);

 // Em-dash chains 3 clauses: 100 + 100 + 100 — effective = 100.
 let chained = format!("{} — {} — {}", "-".repeat(100), "a".repeat(100), "b".repeat(100));
 assert_eq!(effective_sentence_length(&chained), 100);

 // En-dash also recognised.
 let en_dash = format!("{}–{}", "-".repeat(50), "a".repeat(50));
 assert_eq!(effective_sentence_length(&en_dash), 50);
 }

 #[test]
 fn max_sentence_length_threshold_300_boundary() {
 let mut doc = ParsedDoc::default();
 doc.sections.push(Section {
 section_id: "test/prose-section".into(),
 parent_doc: "TEST".into(),
 parent_section: None,
 title: "Test".into(),
 decision_status: mnemosyne_core::DecisionStatus::Active,
 atomic_section_id: None,
 });
 // 250-char sentence — under the new 300 cap, no violation (was over 200 cap pre-140).
 let body = format!("{}.", "-".repeat(250));
 doc.bodies.insert("test/prose-section".into(), body);
 let v = check_style("TEST.md", &doc, &empty_store(), &default_ruleset());
 assert!(
 !v.iter().any(|x| x.rule_id == "max_sentence_length"),
 "250-char sentence must not violate the 300-char cap"
 );

 // 350-char sentence — over the cap.
 let mut doc2 = ParsedDoc::default();
 doc2.sections.push(Section {
 section_id: "test/prose-section".into(),
 parent_doc: "TEST".into(),
 parent_section: None,
 title: "Test".into(),
 decision_status: mnemosyne_core::DecisionStatus::Active,
 atomic_section_id: None,
 });
 let body2 = format!("{}.", "-".repeat(350));
 doc2.bodies.insert("test/prose-section".into(), body2);
 let v2 = check_style("TEST.md", &doc2, &empty_store(), &default_ruleset());
 assert!(
 v2.iter().any(|x| x.rule_id == "max_sentence_length"),
 "350-char sentence must violate the 300-char cap"
 );
 }

 #[test]
 fn max_section_body_length_t4_info() {
 let mut doc = ParsedDoc::default();
 doc.sections.push(Section {
 section_id: "1".into(),
 parent_doc: "TEST".into(),
 parent_section: None,
 title: "Test".into(),
 decision_status: mnemosyne_core::DecisionStatus::Active,
 atomic_section_id: None,
 });
 doc.bodies.insert("1".into(), "-".repeat(6000));
 let v = check_style("TEST.md", &doc, &empty_store(), &default_ruleset());
 let body_len_v: Vec<_> = v
 .iter()
 .filter(|x| x.rule_id == "max_section_body_length")
 .collect();
 assert_eq!(body_len_v.len(), 1);
 assert_eq!(body_len_v[0].severity, StyleSeverity::Info);
 }

 #[test]
 fn boilerplate_jaccard_detects_repetition() {
 let mut doc = ParsedDoc::default();
 doc.changelog_entries.push(ChangelogEntry {
 entry_id: "Round 1".into(),
 parent_changelog_entry: None,
 sub_bullets: vec![
  "this round's substantive contribution = round-scope ratify carry".into(),
  "this round's substantive contribution = round-scope ratify pass".into(),
 ],
 frozen_at_transaction_time: 0,
 });
 let v = check_style("TEST.md", &doc, &empty_store(), &default_ruleset());
 assert!(v
 .iter()
 .any(|x| x.rule_id == "boilerplate_repetition_jaccard"));
 }

 #[test]
 fn enumeration_pattern_detection() {
 let para = "Trade-off — (i) closure scope 5-language emit break acknowledged, (ii) Phase entry block framing deprecate, (iii) Tier 5 measurement-pending lock new 1cases registered.";
 assert!(has_enumeration_pattern(para));
 }

 #[test]
 fn glossary_lookup_skipped_when_empty() {
 let mut glossary = BTreeMap::new();
 glossary.insert("Salsa".to_string(), {
 let mut s = BTreeSet::new();
 s.insert("salsa".to_string());
 s
 });
 let rule = StyleRule {
 rule_id: "terminology_consistency".into(),
 tier: StyleTier::T3,
 threshold: StyleThreshold::GlossaryLookup(glossary),
 scope: StyleScope::FullDoc,
 rationale: "test".into(),
 };
 let mut doc = ParsedDoc::default();
 doc.sections.push(Section {
 section_id: "1".into(),
 parent_doc: "TEST".into(),
 parent_section: None,
 title: "Test".into(),
 decision_status: mnemosyne_core::DecisionStatus::Active,
 atomic_section_id: None,
 });
 doc.bodies
 .insert("1".into(), "uses salsa not Salsa".to_string());
 let v = check_style("TEST.md", &doc, &empty_store(), &[rule]);
 assert_eq!(v.len(), 1);
 assert_eq!(v[0].rule_id, "terminology_consistency");
 }

 /// Regression — `terminology_consistency` MUST NOT fire on file paths
 /// in `Section.implementations`. Filesystem paths are mechanical
 /// citations (lowercase by Unix/C convention), not authored prose.
 ///
 /// Scenario mirrors a TC8-style workspace: glossary lists lowercase
 /// variants (`tc8` / `dut` / `someip` / `dhcpv4`); the section has
 /// prose using the canonical forms and `implementations[]` populated
 /// with lowercase filesystem paths. Pre-fix: 4 false positives. Post-
 /// fix: 0.
 #[test]
 fn terminology_consistency_ignores_implementation_paths() {
 use crate::atomic::{AtomicSection, Implementation};
 let mut glossary = BTreeMap::new();
 for (canon, variants) in [
 ("TC8", &["tc8", "Tc8"][..]),
 ("DUT", &["dut", "Dut"][..]),
 ("SOME/IP", &["someip", "SomeIP"][..]),
 ("DHCPv4", &["dhcpv4", "Dhcpv4"][..]),
 ] {
 let set: BTreeSet<String> = variants.iter().map(|s| s.to_string()).collect();
 glossary.insert(canon.to_string(), set);
 }
 let rule = StyleRule {
 rule_id: "terminology_consistency".into(),
 tier: StyleTier::T3,
 threshold: StyleThreshold::GlossaryLookup(glossary),
 scope: StyleScope::FullDoc,
 rationale: "test".into(),
 };

 let mut doc = ParsedDoc::default();
 doc.sections.push(Section {
 section_id: "tc8-harness/4.2".into(),
 parent_doc: "GENERATED.md".into(),
 parent_section: None,
 title: "4.2".into(),
 decision_status: mnemosyne_core::DecisionStatus::Active,
 // Bare-key uniformity: this test pre-dates the atomic_section_id
 // bridge and intentionally collapses parser-key and atomic-key onto
 // the same string, so the atomic-store fallback path covers the
 // lookup. The render→parse roundtrip test is the canonical
 // reproduction of the production nested-key shape.
 atomic_section_id: Some("tc8-harness/4.2".into()),
 });
 // prose uses canonical TC8 forms — no terminology violation expected.
 let mut store = AtomicStore::default();
 let section = AtomicSection {
 title: "4.2".into(),
 parent_doc: "GENERATED.md".into(),
 parent_section: None,
 intent: Some(
 "TC8 §4.2 — auto-seeded TC8-internal sub-section (40 code citations).".into(),
 ),
 implementations: vec![
 Implementation {
 file: "dut/env/smoke-test.sh".into(),
 symbol: None,
 },
 Implementation {
 file: "include/tc8/bpf_group.h".into(),
 symbol: None,
 },
 Implementation {
 file: "src/sce_integration/cases/someip_ets_084.h".into(),
 symbol: None,
 },
 Implementation {
 file: "src/proto/dhcpv4_common.h".into(),
 symbol: None,
 },
 ],
 ..Default::default()
 };
 store.sections.insert("tc8-harness/4.2".into(), section);

 let v = check_style("docs/GENERATED.md", &doc, &store, &[rule]);
 let term_hits: Vec<&StyleViolation> = v
 .iter()
 .filter(|x| x.rule_id == "terminology_consistency")
 .collect();
 assert!(
 term_hits.is_empty(),
 "implementations file paths must not trigger terminology_consistency; got: {:?}",
 term_hits
 );
 }

 /// Companion to [`terminology_consistency_ignores_implementation_paths`]:
 /// the rule MUST still fire when a lowercase variant appears in genuine
 /// authored prose (intent text).
 #[test]
 fn terminology_consistency_still_fires_on_prose_variants() {
 use crate::atomic::AtomicSection;
 let mut glossary = BTreeMap::new();
 let mut variants = BTreeSet::new();
 variants.insert("tc8".to_string());
 glossary.insert("TC8".to_string(), variants);
 let rule = StyleRule {
 rule_id: "terminology_consistency".into(),
 tier: StyleTier::T3,
 threshold: StyleThreshold::GlossaryLookup(glossary),
 scope: StyleScope::FullDoc,
 rationale: "test".into(),
 };

 let mut doc = ParsedDoc::default();
 doc.sections.push(Section {
 section_id: "p/1".into(),
 parent_doc: "GENERATED.md".into(),
 parent_section: None,
 title: "1".into(),
 decision_status: mnemosyne_core::DecisionStatus::Active,
 atomic_section_id: Some("p/1".into()),
 });
 let mut store = AtomicStore::default();
 let section = AtomicSection {
 title: "1".into(),
 parent_doc: "GENERATED.md".into(),
 parent_section: None,
 intent: Some("the tc8 spec defines ...".into()),
 ..Default::default()
 };
 store.sections.insert("p/1".into(), section);

 let v = check_style("docs/GENERATED.md", &doc, &store, &[rule]);
 assert!(v
 .iter()
 .any(|x| x.rule_id == "terminology_consistency"
 && x.message.contains("`tc8`")));
 }

 /// End-to-end roundtrip — render a section via `render_section`,
 /// re-parse the resulting markdown via the real parser, then style-check.
 ///
 /// Reproduces the production GENERATED.md shape that the bare-key uses
 /// in [`terminology_consistency_ignores_implementation_paths`] miss:
 /// the renderer wraps section bodies under `## Sections` and demotes
 /// their headings from `##` to `###`, so the parser builds nested
 /// `section_id` like `<doc-slug>/sections/<atomic-id>` while the atomic
 /// store is keyed by the bare `<atomic-id>`. Pre-bridge: lookup misses,
 /// `parsed.bodies` fallback fires the rule on `**Implementations**:`
 /// file paths. Post-bridge: parser captures the heading's `§<token>`
 /// into `Section.atomic_section_id`, `AtomicStore::resolve` honours it,
 /// and `synthesize_section_prose_body` correctly excludes the impl block.
 #[test]
 fn terminology_consistency_roundtrip_excludes_impl_paths_in_nested_layout() {
 use crate::atomic::{AtomicSection, AtomicStore, Implementation};
 use crate::parser::parse_markdown;
 use crate::render::render_section;

 // Glossary of the same shape as the production failure: lowercase
 // path-shaped tokens that collide with canonical prose forms.
 let mut glossary = BTreeMap::new();
 for (canon, variants) in [
 ("TC8", &["tc8"][..]),
 ("DUT", &["dut"][..]),
 ("SOME/IP", &["someip"][..]),
 ("DHCPv4", &["dhcpv4"][..]),
 ] {
 let set: BTreeSet<String> = variants.iter().map(|s| s.to_string()).collect();
 glossary.insert(canon.to_string(), set);
 }
 let rule = StyleRule {
 rule_id: "terminology_consistency".into(),
 tier: StyleTier::T3,
 threshold: StyleThreshold::GlossaryLookup(glossary),
 scope: StyleScope::FullDoc,
 rationale: "test".into(),
 };

 // Atomic source: prose uses canonical forms, implementations[] holds
 // lowercase filesystem paths that share substrings with the variants.
 let atomic = AtomicSection {
 title: "TC8 harness §4.2".into(),
 parent_doc: "docs/GENERATED.md".into(),
 parent_section: None,
 intent: Some(
 "TC8 §4.2 — auto-seeded TC8-internal sub-section (40 code citations).".into(),
 ),
 implementations: vec![
 Implementation {
 file: "dut/env/smoke-test.sh".into(),
 symbol: None,
 },
 Implementation {
 file: "include/tc8/bpf_group.h".into(),
 symbol: None,
 },
 Implementation {
 file: "src/sce_integration/cases/someip_ets_084.h".into(),
 symbol: None,
 },
 Implementation {
 file: "src/proto/dhcpv4_common.h".into(),
 symbol: None,
 },
 ],
 ..Default::default()
 };

 // Render the section, then wrap it in the production GENERATED.md
 // outer shape: doc-root h1 + `## Sections` parent + the rendered
 // section demoted from `##` to `###` (mirrors atomic_cli.rs's
 // `replacen("## §", "### §", 1)` step in render_atomic_store_to_md).
 let rendered = render_section("4.2", &atomic.title, "active", &atomic).unwrap();
 let demoted = rendered.replacen("## §", "### §", 1);
 let full_md = format!(
 "# GENERATED.md — atomic store derived view\n\n## Sections\n\n{}",
 demoted
 );

 // Real parse path. Parser produces nested section_id
 // `<doc-slug>/sections/4.2`; the bridge field carries bare "4.2".
 let parsed = parse_markdown(&full_md, "docs/GENERATED.md");
 let leaf = parsed
 .sections
 .iter()
 .find(|s| s.atomic_section_id.as_deref() == Some("4.2"))
 .expect("parser must capture §4.2 from heading into atomic_section_id");
 assert_ne!(
 leaf.section_id, "4.2",
 "parser-derived section_id is expected to be the nested form, \
 demonstrating that the bridge is what makes the lookup work"
 );

 // Atomic store keyed by bare "4.2" — the production shape.
 let mut store = AtomicStore::default();
 store.sections.insert("4.2".into(), atomic);

 let v = check_style("docs/GENERATED.md", &parsed, &store, &[rule]);
 let term_hits: Vec<&StyleViolation> = v
 .iter()
 .filter(|x| x.rule_id == "terminology_consistency")
 .collect();
 assert!(
 term_hits.is_empty(),
 "nested-layout roundtrip must not fire terminology_consistency on \
 implementations file paths; got: {:?}",
 term_hits
 );
 }

 #[test]
 fn cross_doc_reference_implicit_detected() {
 let mut doc = ParsedDoc::default();
 doc.sections.push(Section {
 section_id: "1".into(),
 parent_doc: "DESIGN.md".into(),
 parent_section: None,
 title: "Test".into(),
 decision_status: mnemosyne_core::DecisionStatus::Active,
 atomic_section_id: None,
 });
 doc.bodies
 .insert("1".into(), "see ARCHITECTURE.md for layer".into());
 let v = check_style("docs/DESIGN.md", &doc, &empty_store(), &default_ruleset());
 assert!(v.iter().any(|x| x.rule_id == "cross_doc_reference_explicit"));
 }

 #[test]
 fn cross_doc_reference_anchored_allowed() {
 let mut doc = ParsedDoc::default();
 doc.sections.push(Section {
 section_id: "1".into(),
 parent_doc: "DESIGN.md".into(),
 parent_section: None,
 title: "Test".into(),
 decision_status: mnemosyne_core::DecisionStatus::Active,
 atomic_section_id: None,
 });
 doc.bodies
 .insert("1".into(), "see ARCHITECTURE.md#§3 for layer".into());
 let v = check_style("docs/DESIGN.md", &doc, &empty_store(), &default_ruleset());
 assert!(!v.iter().any(|x| x.rule_id == "cross_doc_reference_explicit"));
 }
}
