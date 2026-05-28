//! §15 spec query API surface prototype (Round 116, OPTION query-api-bench).
//!
//! Round 47-49 prototype-first pattern equivalent — measurement source first, then Round 117
//! Reframing-decision source. This prototype's output (§43 outbound count, §66
//! changelog hit count, cross-doc reclassify consistency) — Round 117 §66 / §60 / §15
//! substantive spec mutation's measurement input.
//!
//! Phase 0b entry #1 — Phase 0 end gate's *AI agent dogfood proof* item
//! prerequisite. *AI agents (Claude / other LLMs) — zero markdown grep +
//! mnemosyne-cli query as the *only spec-entry carry* contract's first prototype.
//!
//! 4 query primitives (DESIGN §39 *Phase 0 design_doc schema closed-form registered*
//! 4 entity/relation in 1-hop traversal):
//! - `section_by_id(workspace, section_id) -> Option<SectionView>` —
//! body / decision_status / parent_doc / parent_section / title /
//! line_anchor carry.
//! - `related_sections(workspace, section_id) -> RelatedSections` —
//! outbound + inbound 1-hop CrossRef traversal (decision/impl/cross_doc).
//! - `changelog_entries_for_section(workspace, section_id) ->
//! `Vec<ChangelogEntryView>` — detect §N citations in entry bodies (sub_bullets
//! fulltext §N literal scan).
//! - `workspace_section_id_set(workspace) -> BTreeSet<String>` — full
//! section_id dict (orphan check auxiliary).
//!
//! Prototype role (Round 116 framing):
//! - 7th module of the bench/codegen-prototype crate (entity_indexer /
//! cf_wrapper / salsa_wire / closure_runtime / markdown_import /
//! markdown_export / t1_validator / query_api).
//! - in-memory typed-fact state is limited (RocksDB persistence is out-of-scope carry,
//! separate follow-up round).
//! - production crate (mnemosyne-validate / mnemosyne-cli) entry is out of scope —
//! Round 120 production lift subsequent.

use crate::markdown_import::{
 ChangelogEntry, CrossRef, DecisionStatus, ParsedDoc, RefKind, Section,
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

// ============================================================================
// Workspace — multi-doc lookup container (bench prototype, in-memory).
// ============================================================================

/// Workspace = (path → ParsedDoc) mapping + workspace default cross-doc target.
///
/// Round 70 OPTION H-2 carry — production `mnemosyne-workspace::Workspace`
/// Bench-prototype equivalent. mnemosyne workspace default = `docs/DESIGN.md`.
#[derive(Debug, Clone, Default)]
pub struct Workspace {
 pub docs: BTreeMap<String, ParsedDoc>,
 pub default_doc: Option<String>,
}

impl Workspace {
 pub const MNEMOSYNE_DEFAULT_DOC: &'static str = "docs/DESIGN.md";

 pub fn new() -> Self {
 Self::default()
 }

 pub fn mnemosyne() -> Self {
 Self {
 docs: BTreeMap::new(),
 default_doc: Some(Self::MNEMOSYNE_DEFAULT_DOC.to_string()),
 }
 }

 pub fn insert(&mut self, path: impl Into<String>, doc: ParsedDoc) {
 self.docs.insert(path.into(), doc);
 }
}

// ============================================================================
// Query result views — JSON envelope (Claude consumable shape).
// ============================================================================

/// SectionView — `section_by_id` return form (top-level of the JSON envelope).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SectionView {
 pub section_id: String,
 pub parent_doc: String,
 pub parent_section: Option<String>,
 pub title: String,
 pub decision_status: String,
 pub body: String,
 pub line_anchor: usize,
}

/// RelatedSections — related_sections carry form. 1-hop traversal result.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct RelatedSections {
 pub outbound_refs: Vec<CrossRefView>,
 pub inbound_refs: Vec<CrossRefView>,
}

/// CrossRefView — RelatedSections's nested cross-ref view.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CrossRefView {
 pub from_doc: String,
 pub from_section: String,
 pub to_target: String,
 pub ref_kind: String,
 pub created_at_changelog_entry: Option<String>,
}

/// ChangelogEntryView — changelog_entries_for_section carry form.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ChangelogEntryView {
 pub entry_id: String,
 pub parent_doc: String,
 pub parent_changelog_entry: Option<String>,
 pub frozen_at_transaction_time: i64,
 pub sub_bullets: Vec<String>,
 /// this entry body in §N citation count (sub_bullets fulltext literal scan).
 pub citation_count: usize,
}

/// QueryEnvelope — top-level JSON output shape (Claude-consumable).
///
/// `mnemosyne-cli query §43 --include-related --include-changelog --json` output.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct QueryEnvelope {
 pub section: SectionView,
 pub outbound_refs: Vec<CrossRefView>,
 pub inbound_refs: Vec<CrossRefView>,
 pub related_changelog_entries: Vec<ChangelogEntryView>,
}

// ============================================================================
// Query primitives — 4 functions.
// ============================================================================

/// section_by_id — workspace-wide section_id carry.
///
/// Lookup priority:
/// (1) first match across all workspace docs (deterministic — BTreeMap path order).
/// (2) not found → None.
pub fn section_by_id(workspace: &Workspace, section_id: &str) -> Option<SectionView> {
 for (path, doc) in &workspace.docs {
 if let Some(section) = doc.sections.iter().find(|s| s.section_id == section_id) {
 return Some(build_section_view(path, section, doc));
 }
 }
 None
}

fn build_section_view(path: &str, section: &Section, doc: &ParsedDoc) -> SectionView {
 let body = doc
 .bodies
 .get(&section.section_id)
 .cloned()
 .unwrap_or_default();
 let line_anchor = doc
 .line_anchors
 .get(&section.section_id)
 .copied()
 .unwrap_or(0);
 SectionView {
 section_id: section.section_id.clone(),
 parent_doc: path.to_string(),
 parent_section: section.parent_section.clone(),
 title: section.title.clone(),
 decision_status: decision_status_str(section.decision_status).to_string(),
 body,
 line_anchor,
 }
}

fn decision_status_str(s: DecisionStatus) -> &'static str {
 match s {
 DecisionStatus::Active => "active",
 DecisionStatus::Superseded => "superseded",
 DecisionStatus::Removed => "removed",
 }
}

fn ref_kind_str(k: RefKind) -> &'static str {
 match k {
 RefKind::Decision => "decision",
 RefKind::Impl => "impl",
 RefKind::CrossDoc => "cross_doc",
 }
}

/// related_sections — full 1-hop CrossRef traversal across the workspace.
///
/// outbound = cross_refs whose from_section matches this section_id (within self doc).
/// inbound = cross_refs whose to_target is this section_id (scanned across all workspace docs;
/// cross-doc target `{path}#§N` form also keeps §N as the tail consistently).
pub fn related_sections(workspace: &Workspace, section_id: &str) -> RelatedSections {
 let mut out = RelatedSections::default();
 for (path, doc) in &workspace.docs {
 for cr in &doc.cross_refs {
 if cr.from_section == section_id {
  out.outbound_refs
  .push(build_cross_ref_view(path, cr));
 }
 if cross_ref_targets_section(cr, section_id) {
  out.inbound_refs.push(build_cross_ref_view(path, cr));
 }
 }
 }
 out
}

fn build_cross_ref_view(from_doc: &str, cr: &CrossRef) -> CrossRefView {
 CrossRefView {
 from_doc: from_doc.to_string(),
 from_section: cr.from_section.clone(),
 to_target: cr.to_target.clone(),
 ref_kind: ref_kind_str(cr.ref_kind).to_string(),
 created_at_changelog_entry: cr.created_at_changelog_entry.clone(),
 }
}

/// CrossRef.to_target — section_id branch check.
///
/// match shape:
/// - decision form: bare `§N` → to_target == section_id
/// - cross-doc form: `{path}#§N` or `{path}#anchor-{N}` etc. → tail in §N
/// literal-or-anchor numeric prefix consistency (Round 70 OPTION H-2 carry).
fn cross_ref_targets_section(cr: &CrossRef, section_id: &str) -> bool {
 if cr.to_target == section_id {
 return true;
 }
 if let Some((_path, anchor)) = cr.to_target.split_once('#') {
 if anchor == section_id {
 return true;
 }
 // `#§N` literal anchor (Round 70 OPTION H-2 canonical form).
 if let Some(stripped) = anchor.strip_prefix('§') {
 if stripped == section_id {
  return true;
 }
 }
 }
 false
}

/// changelog_entries_for_section — detects §N citations across the full workspace.
///
/// this entry's sub_bullets fulltext in `§{section_id}` literal scan. citation
/// count = citation_count. Registered-changelog carry across all workspace docs.
pub fn changelog_entries_for_section(
 workspace: &Workspace,
 section_id: &str,
) -> Vec<ChangelogEntryView> {
 let mut out = Vec::new();
 let needle = format!("§{}", section_id);
 for (path, doc) in &workspace.docs {
 for entry in &doc.changelog_entries {
 let citation_count = count_citations(entry, &needle, section_id);
 if citation_count > 0 {
  out.push(ChangelogEntryView {
  entry_id: entry.entry_id.clone(),
  parent_doc: path.to_string(),
  parent_changelog_entry: entry.parent_changelog_entry.clone(),
  frozen_at_transaction_time: entry.frozen_at_transaction_time,
  sub_bullets: entry.sub_bullets.clone(),
  citation_count,
  });
 }
 }
 }
 out
}

/// `§N` literal scan + boundary check (blocks false positives).
///
/// `§43` query — guards against false positives where `§434` or `§43.1` shares the prefix.
/// avoid — if the byte after the needle is an ASCII digit or `.`, skip it (sub-number form
/// `§43.1` is a separate section_id (registered separately).
fn count_citations(entry: &ChangelogEntry, needle: &str, _section_id: &str) -> usize {
 let mut count = 0usize;
 for sub in &entry.sub_bullets {
 let mut search_start = 0usize;
 while let Some(pos) = sub[search_start..].find(needle) {
 let abs_pos = search_start + pos;
 let after = abs_pos + needle.len();
 // boundary check — next char must NOT be digit or `.`.
 let next_byte = sub.as_bytes().get(after).copied();
 let is_extended = matches!(next_byte, Some(b) if b.is_ascii_digit() || b == b'.');
 if !is_extended {
  count += 1;
 }
 search_start = after;
 if search_start >= sub.len() {
  break;
 }
 }
 }
 count
}

/// workspace_section_id_set — full section_id dict across all docs.
///
/// orphan-check auxiliary + spec-query API's *intent section_id carry* primitive.
pub fn workspace_section_id_set(workspace: &Workspace) -> BTreeSet<String> {
 let mut out = BTreeSet::new();
 for doc in workspace.docs.values() {
 for s in &doc.sections {
 out.insert(s.section_id.clone());
 }
 }
 out
}

// ============================================================================
// Envelope builder — JSON output (Claude consumable shape).
// ============================================================================

/// build_envelope — section_by_id + related_sections +
/// changelog_entries_for_section unified envelope.
///
/// `mnemosyne-cli query §N --include-related --include-changelog --json`'s
/// Returns None when the section_id is not found (CLI emits exit code 1 + an explicit stderr message).
pub fn build_envelope(workspace: &Workspace, section_id: &str) -> Option<QueryEnvelope> {
 let section = section_by_id(workspace, section_id)?;
 let related = related_sections(workspace, section_id);
 let changelog = changelog_entries_for_section(workspace, section_id);
 Some(QueryEnvelope {
 section,
 outbound_refs: related.outbound_refs,
 inbound_refs: related.inbound_refs,
 related_changelog_entries: changelog,
 })
}

// ============================================================================
// Tests — small fixture (markdown_import small fixture re.se).
// ============================================================================

#[cfg(test)]
mod tests {
 use super::*;
 use crate::markdown_import::{design_doc_small_fixture, parse_markdown};

 fn fixture_workspace() -> Workspace {
 let mut ws = Workspace::mnemosyne();
 let doc = parse_markdown(design_doc_small_fixture(), "docs/DESIGN.md");
 ws.insert("docs/DESIGN.md", doc);
 ws
 }

 #[test]
 fn section_by_id_finds_numbered() {
 let ws = fixture_workspace();
 let view = section_by_id(&ws, "39").expect("§39 must exist");
 assert_eq!(view.section_id, "39");
 assert_eq!(view.title, "Graph schema codegen");
 assert_eq!(view.parent_doc, "docs/DESIGN.md");
 assert_eq!(view.decision_status, "active");
 assert!(view.line_anchor > 0, "line_anchor must be 1-indexed");
 }

 #[test]
 fn section_by_id_returns_none_for_unknown() {
 let ws = fixture_workspace();
 assert!(section_by_id(&ws, "999").is_none());
 }

 #[test]
 fn section_view_body_contains_inline_text() {
 let ws = fixture_workspace();
 let view = section_by_id(&ws, "39").expect("§39 exists");
 assert!(
 view.body.contains("§39") || view.body.contains("graph_schema"),
 "body must contain inline text from §39"
 );
 }

 #[test]
 fn related_sections_outbound_includes_decision_refs() {
 let ws = fixture_workspace();
 let related = related_sections(&ws, "39");
 // §39 body in §39 self citation + §41 citation emit.
 assert!(
 related
  .outbound_refs
  .iter()
  .any(|r| r.to_target == "41" && r.ref_kind == "decision"),
 "expected §41 outbound from §39"
 );
 }

 #[test]
 fn related_sections_inbound_includes_external_citation() {
 let ws = fixture_workspace();
 // §41 body in not.itation BUT §39 body in `§41` citation → §41 inbound in §39 registered.
 let related_41 = related_sections(&ws, "41");
 assert!(
 related_41
  .inbound_refs
  .iter()
  .any(|r| r.from_section == "39"),
 "§41 inbound must include §39 (small fixture §39 body in §41 citation)"
 );
 }

 #[test]
 fn changelog_entries_for_section_detects_citation() {
 let ws = fixture_workspace();
 // Round 60 sub_bullets in §39 + §66 citation.
 let entries_39 = changelog_entries_for_section(&ws, "39");
 assert!(
 entries_39.iter().any(|e| e.entry_id == "Round 60"),
 "Round 60 must cite §39"
 );
 }

 #[test]
 fn changelog_citation_count_excludes_substring_match() {
 // §43 in query on §434 same longer literal in false positive block.
 // synthesized fixture (small fixture in §434 missing, this test - boundary
 // logic itself validation).
 let entry = ChangelogEntry {
 entry_id: "Round X".to_string(),
 parent_changelog_entry: None,
 sub_bullets: vec![
  "§43 cited".to_string(),
  "§434 not cited (extended)".to_string(),
  "§43.1 not cited (subnumber)".to_string(),
 ],
 frozen_at_transaction_time: 1,
 };
 let needle = "§43";
 let count = count_citations(&entry, needle, "43");
 assert_eq!(count, 1, "only bare §43 citation must count");
 }

 #[test]
 fn workspace_section_id_set_is_sorted_and_unique() {
 let ws = fixture_workspace();
 let set = workspace_section_id_set(&ws);
 assert!(set.contains("39"));
 assert!(set.contains("61"));
 assert!(set.len() >= 4, "small fixture has at least 4 sections");
 }

 #[test]
 fn build_envelope_serializes_to_json() {
 let ws = fixture_workspace();
 let env = build_envelope(&ws, "39").expect("§39 exists");
 let json = serde_json::to_string_pretty(&env).expect("serialize");
 assert!(json.contains("\"section_id\": \"39\""));
 assert!(json.contains("\"outbound_refs\""));
 assert!(json.contains("\"related_changelog_entries\""));
 }

 #[test]
 fn build_envelope_returns_none_for_unknown() {
 let ws = fixture_workspace();
 assert!(build_envelope(&ws, "999").is_none());
 }

 #[test]
 fn cross_ref_targets_section_handles_cross_doc_anchor() {
 let cr = CrossRef {
 from_section: "x".to_string(),
 to_target: "docs/DESIGN.md#§39".to_string(),
 ref_kind: RefKind::CrossDoc,
 created_at_changelog_entry: None,
 };
 assert!(cross_ref_targets_section(&cr, "39"));
 assert!(!cross_ref_targets_section(&cr, "41"));
 }
}
