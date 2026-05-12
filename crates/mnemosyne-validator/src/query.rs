//! *Spec query API surface* — production lift.
//!
//! bench prototype (`bench/crates/codegen-prototype/src/query_api.rs`)
//! First round of the production lift. *Spec query API surface* body
//! registered + prerequisite #5 *AI agent dogfood proof* binding source — *AI
//! agent (Claude / future LLM) markdown grep = 0 calls + 0 direct DESIGN.md reads
//! Zero-time + spec-query-API-only entry contract — production
//! data path.
//!
//! ## 4 primitive (closed-form, ratify carry)
//!
//! - [`section_by_id`] — deterministic section_id scan across the workspace (BTreeMap
//! path order) in first match carry, body + line_anchor + decision_status
//! surface.
//! - [`related_sections`] — outbound + inbound 1-hop CrossRef traversal
//! (self doc in + cross-doc form `{path}#§N` tail anchor consistency
//! OPTION H-2 carry).
//! - [`changelog_entries_for_section`] — workspace in all doc in
//! Detects §N citations in changelog_entries fulltext (with boundary checks against longer
//! Blocks false positives on numeric forms `` / ``.
//! - [`workspace_section_id_set`] — full section_id dict across all docs.
//!
//! ## JSON envelope shape (Claude-consumable; body-registered carry)
//!
//! - [`QueryEnvelope`] = section + outbound_refs + inbound_refs +
//! related_changelog_entries unified nested shape.
//! - `serde::Serialize` derive in `serde_json::to_string_pretty` serialize.
//!
//! ## CLI surface
//!
//! - `mnemosyne-cli query [--include-related] [--include-changelog] [--json]`
//! - `mnemosyne-cli query --list-sections`

use crate::atomic::{synthesize_section_body, AtomicChangelogEntry, AtomicStore};
use crate::schema::{ChangelogEntry, CrossRef, DecisionStatus, ParsedDoc, RefKind, Section};
use crate::workspace::Workspace;
use serde::Serialize;
use std::collections::BTreeSet;

// ============================================================================
// Query result views — JSON envelope (Claude consumable shape).
// ============================================================================

/// SectionView — `section_by_id` carry form. Top-level of the JSON envelope.
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

/// RelatedSections — `related_sections` carry form. 1-hop traversal result.
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

/// ChangelogEntryView — `changelog_entries_for_section` carry form.
///
/// atomic-first surface (sub_bullets cascade B). atomic
/// store entry's 5 fields (decision_summary / changes_bullets /
/// verification_bullets / impact_refs / carry_forward_bullets) separate field
/// is directly exposed. The `sub_bullets` field stays stable for -162 legacy entries; extending
/// 244 schema-doc consistency. citation_count = sub_bullets + atomic 5-field
/// summed across fulltext + impact_refs structural matches.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ChangelogEntryView {
 pub entry_id: String,
 pub parent_doc: String,
 pub parent_changelog_entry: Option<String>,
 pub frozen_at_transaction_time: i64,
 pub sub_bullets: Vec<String>,
 /// §N citation count in this entry's body (sub_bullets + atomic 5-field fulltext +
 /// atomic impact_refs are structurally summed.
 pub citation_count: usize,
 /// atomic decision_summary surface (atomic store unregistered entry =
 /// `None`).
 #[serde(default, skip_serializing_if = "Option::is_none")]
 pub atomic_decision_summary: Option<String>,
 /// atomic changes_bullets surface.
 #[serde(default, skip_serializing_if = "Vec::is_empty")]
 pub atomic_changes_bullets: Vec<String>,
 /// atomic verification_bullets surface.
 #[serde(default, skip_serializing_if = "Vec::is_empty")]
 pub atomic_verification_bullets: Vec<String>,
 /// atomic impact_refs surface (target section_id without `§`
 /// prefix). Structural cross-ref shapes are summed directly into citation_count.
 #[serde(default, skip_serializing_if = "Vec::is_empty")]
 pub atomic_impact_refs: Vec<String>,
 /// atomic carry_forward_bullets surface.
 #[serde(default, skip_serializing_if = "Vec::is_empty")]
 pub atomic_carry_forward_bullets: Vec<String>,
}

/// QueryEnvelope — top-level JSON output shape (Claude-consumable).
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

/// `section_by_id` — workspace in section_id deterministic scan.
///
/// Lookup priority: first match in BTreeMap path order.
/// not found → `None` (CLI in error code 1 + stderr explicit).
///
/// atomic-first body source carry (sub_bullets cascade D).
/// atomic-store-registered section: prose is synthesized from the 8 atomic fields.
/// SectionView.body, fallback = parsed.bodies (legacy markdown).
pub fn section_by_id(
 workspace: &Workspace,
 atomic_store: &AtomicStore,
 section_id: &str,
) -> Option<SectionView> {
 for (path, doc) in &workspace.docs {
 if let Some(section) = doc.sections.iter().find(|s| s.section_id == section_id) {
 return Some(build_section_view(path, section, doc, atomic_store));
 }
 }
 // atomic-only section surface (markdown missing + atomic_store
 // standalone registered scope, MD-DELETION-RATIFY thereafter sole iteration
 // source path).
 if let Some(atomic) = atomic_store.section(section_id) {
 let synthetic_section = Section {
 section_id: section_id.to_string(),
 parent_doc: ATOMIC_ONLY_PARENT_DOC.to_string(),
 parent_section: None,
 title: atomic
  .intent
  .clone()
  .unwrap_or_else(|| section_id.to_string()),
 decision_status: atomic.decision_status.unwrap_or(DecisionStatus::Active),
 };
 let synthetic_doc = ParsedDoc::default();
 return Some(build_section_view(
 ATOMIC_ONLY_PARENT_DOC,
 &synthetic_section,
 &synthetic_doc,
 atomic_store,
 ));
 }
 None
}

fn build_section_view(
 path: &str,
 section: &Section,
 doc: &ParsedDoc,
 atomic_store: &AtomicStore,
) -> SectionView {
 // atomic-first body source. atomic store in section is present
 // synthesize_section_body result, fallback = parsed.bodies (legacy markdown).
 let body = if let Some(atomic) = atomic_store.section(&section.section_id) {
 synthesize_section_body(atomic)
 } else {
 doc.bodies
 .get(&section.section_id)
 .cloned()
 .unwrap_or_default()
 };
 let line_anchor = doc
 .line_anchors
 .get(&section.section_id)
 .copied()
 .unwrap_or(0);
 // Round 265 — atomic decision_status overrides parser-derived default
 // when present. parser hardcodes Active workspace-wide; the atomic
 // override is the only path to surface Superseded / Removed.
 let resolved_status = atomic_store
 .section(&section.section_id)
 .and_then(|a| a.decision_status)
 .unwrap_or(section.decision_status);
 SectionView {
 section_id: section.section_id.clone(),
 parent_doc: path.to_string(),
 parent_section: section.parent_section.clone(),
 title: section.title.clone(),
 decision_status: decision_status_str(resolved_status).to_string(),
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

/// `related_sections` — workspace full in 1-hop CrossRef traversal.
///
/// outbound = cross_refs whose from_section is this section_id (within self doc).
/// inbound = cross_refs whose to_target is this section_id (scanned across all workspace docs;
/// cross-doc form `{path}#§N` tail anchor strip consistency — 
/// OPTION H-2 carry).
///
/// Markdown-derived signature only. For atomic-store-aware traversal that
/// surfaces `impact_refs` reverse lookup post 7-md deletion, callers should
/// prefer [`related_sections_with_atomic`].
pub fn related_sections(workspace: &Workspace, section_id: &str) -> RelatedSections {
 let mut out = RelatedSections::default();
 for (path, doc) in &workspace.docs {
 for cr in &doc.cross_refs {
 if cr.from_section == section_id {
  out.outbound_refs.push(build_cross_ref_view(path, cr));
 }
 if cross_ref_targets_section(cr, section_id) {
  out.inbound_refs.push(build_cross_ref_view(path, cr));
 }
 }
 }
 out
}

/// atomic-aware variant of [`related_sections`]. Adds two new
/// inbound traversal sources sourced from the atomic store:
///
/// - `entry.impact_refs` — every changelog entry whose impact_refs
/// contains `section_id` becomes a synthetic inbound ref originating
/// from `<atomic-changelog>#<entry_id>`.
/// - `atomic_section.impact_scope` — every atomic section whose
/// impact_scope references `section_id` becomes an inbound ref from
/// `<atomic>#<source_section_id>`.
///
/// Outbound traversal is unchanged (markdown-derived); the function is
/// designed for inbound enrichment when the atomic store carries impact
/// information that legacy markdown cross-refs no longer surface (post
/// MD-DELETION).
pub fn related_sections_with_atomic(
 workspace: &Workspace,
 atomic_store: &AtomicStore,
 section_id: &str,
) -> RelatedSections {
 let mut out = related_sections(workspace, section_id);
 for (entry_id, entry) in &atomic_store.changelog_entries {
 for r in &entry.impact_refs {
 if r == section_id {
  out.inbound_refs.push(CrossRefView {
  from_doc: "<atomic-changelog>".to_string(),
  from_section: entry_id.clone(),
  to_target: section_id.to_string(),
  ref_kind: "decision".to_string(),
  created_at_changelog_entry: Some(entry_id.clone()),
  });
 }
 }
 }
 for (source_section_id, atomic) in &atomic_store.sections {
 for r in &atomic.impact_scope {
 if r == section_id {
  out.inbound_refs.push(CrossRefView {
  from_doc: "<atomic>".to_string(),
  from_section: source_section_id.clone(),
  to_target: section_id.to_string(),
  ref_kind: "decision".to_string(),
  created_at_changelog_entry: None,
  });
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

/// Check whether CrossRef.to_target leaks the section_id.
///
/// match shape:
/// - decision form: bare `§N` → to_target == section_id
/// - cross-doc form: `{path}#§N` or `{path}#anchor-{N}` etc. → tail in §N
/// literal-or-anchor numeric prefix consistency.
fn cross_ref_targets_section(cr: &CrossRef, section_id: &str) -> bool {
 if cr.to_target == section_id {
 return true;
 }
 if let Some((_path, anchor)) = cr.to_target.split_once('#') {
 if anchor == section_id {
 return true;
 }
 if let Some(stripped) = anchor.strip_prefix('§') {
 if stripped == section_id {
  return true;
 }
 }
 }
 false
}

/// `changelog_entries_for_section` — detect §N citations across the full workspace
/// (atomic-first surface, cascade B).
///
/// citation source = this entry's (1) sub_bullets fulltext (legacy carry,
/// -162) + (2) atomic_store entry's 5-field fulltext +
/// (3) atomic impact_refs structural match — boundary check guards against longer numeric
/// false-positive block for forms `` / `` (next byte after the needle —
/// ASCII digit OR `.` (in which case it mismatches).
///
/// Iteration order:
/// - workspace.docs in ChangelogEntry markdown-derived entry first
/// (legacy -162 + atomic-migrated both visible),
/// - then atomic-only entries (markdown absent, atomic-store-standalone),
///. parent_doc = "<atomic>" sentinel,
/// MD-DELETION-RATIFY — sole iteration source thereafter.
pub fn changelog_entries_for_section(
 workspace: &Workspace,
 atomic_store: &AtomicStore,
 section_id: &str,
) -> Vec<ChangelogEntryView> {
 let mut out = Vec::new();
 let needle = format!("§{}", section_id);
 let mut seen_entry_ids: BTreeSet<String> = BTreeSet::new();
 for (path, doc) in &workspace.docs {
 for entry in &doc.changelog_entries {
 let atomic = atomic_store.entry(&entry.entry_id);
 let citation_count = count_citations(entry, atomic, &needle, section_id);
 if citation_count > 0 {
  out.push(build_entry_view(path, entry, atomic, citation_count));
 }
 seen_entry_ids.insert(entry.entry_id.clone());
 }
 }
 // atomic-only entry surface (markdown missing atomic_store standalone).
 for (entry_id, atomic) in &atomic_store.changelog_entries {
 if seen_entry_ids.contains(entry_id) {
 continue;
 }
 let synthetic = ChangelogEntry {
 entry_id: entry_id.clone(),
 parent_changelog_entry: None,
 sub_bullets: Vec::new(),
 frozen_at_transaction_time: 0,
 };
 let citation_count = count_citations(&synthetic, Some(atomic), &needle, section_id);
 if citation_count > 0 {
 out.push(build_entry_view(ATOMIC_ONLY_PARENT_DOC, &synthetic, Some(atomic), citation_count));
 }
 }
 out
}

/// Sentinel `parent_doc` for atomic-only entries.
/// markdown-absent + atomic-store-standalone entries use a placeholder notation in view.parent_doc.
pub const ATOMIC_ONLY_PARENT_DOC: &str = "<atomic>";

fn build_entry_view(
 path: &str,
 entry: &ChangelogEntry,
 atomic: Option<&AtomicChangelogEntry>,
 citation_count: usize,
) -> ChangelogEntryView {
 ChangelogEntryView {
 entry_id: entry.entry_id.clone(),
 parent_doc: path.to_string(),
 parent_changelog_entry: entry.parent_changelog_entry.clone(),
 frozen_at_transaction_time: entry.frozen_at_transaction_time,
 sub_bullets: entry.sub_bullets.clone(),
 citation_count,
 atomic_decision_summary: atomic.and_then(|a| a.decision_summary.clone()),
 atomic_changes_bullets: atomic
 .map(|a| a.changes_bullets.clone())
 .unwrap_or_default(),
 atomic_verification_bullets: atomic
 .map(|a| a.verification_bullets.clone())
 .unwrap_or_default(),
 atomic_impact_refs: atomic.map(|a| a.impact_refs.clone()).unwrap_or_default(),
 atomic_carry_forward_bullets: atomic
 .map(|a| a.carry_forward_bullets.clone())
 .unwrap_or_default(),
 }
}

fn count_citations(
 entry: &ChangelogEntry,
 atomic: Option<&AtomicChangelogEntry>,
 needle: &str,
 section_id: &str,
) -> usize {
 let mut count = 0usize;
 for sub in &entry.sub_bullets {
 count += count_needle_in(sub, needle);
 }
 if let Some(a) = atomic {
 if let Some(decision) = &a.decision_summary {
 count += count_needle_in(decision, needle);
 }
 for b in &a.changes_bullets {
 count += count_needle_in(b, needle);
 }
 for b in &a.verification_bullets {
 count += count_needle_in(b, needle);
 }
 for b in &a.carry_forward_bullets {
 count += count_needle_in(b, needle);
 }
 // structural cross-ref — impact_refs in direct match (entry §N
 // impact target as explicit, 1 iteration count).
 for r in &a.impact_refs {
 if r == section_id {
  count += 1;
 }
 }
 }
 count
}

fn count_needle_in(haystack: &str, needle: &str) -> usize {
 let mut count = 0usize;
 let mut search_start = 0usize;
 while let Some(pos) = haystack[search_start..].find(needle) {
 let abs_pos = search_start + pos;
 let after = abs_pos + needle.len();
 let next_byte = haystack.as_bytes().get(after).copied();
 let is_extended = matches!(next_byte, Some(b) if b.is_ascii_digit() || b == b'.');
 if !is_extended {
 count += 1;
 }
 search_start = after;
 if search_start >= haystack.len() {
 break;
 }
 }
 count
}

/// `workspace_section_id_set` — section_id dict spanning all docs.
pub fn workspace_section_id_set(workspace: &Workspace) -> BTreeSet<String> {
 let mut out = BTreeSet::new();
 for doc in workspace.docs.values() {
 for s in &doc.sections {
 out.insert(s.section_id.clone());
 }
 }
 out
}

/// `build_envelope` — section_by_id + related_sections +
/// changelog_entries_for_section unified envelope (Claude consumable).
///
/// atomic-first surface carry (cascade B). atomic-store-entry-driven.
/// 5-field ChangelogEntryView — `atomic_*` fields exposed, citation_count
/// atomic-field citations are summed.
pub fn build_envelope(
 workspace: &Workspace,
 atomic_store: &AtomicStore,
 section_id: &str,
) -> Option<QueryEnvelope> {
 let section = section_by_id(workspace, atomic_store, section_id)?;
 // atomic-aware traversal (post 7-md deletion the markdown
 // cross_ref graph collapses; impact_refs / impact_scope reverse lookup
 // restores inbound visibility).
 let related = related_sections_with_atomic(workspace, atomic_store, section_id);
 let changelog = changelog_entries_for_section(workspace, atomic_store, section_id);
 Some(QueryEnvelope {
 section,
 outbound_refs: related.outbound_refs,
 inbound_refs: related.inbound_refs,
 related_changelog_entries: changelog,
 })
}

// ============================================================================
// Tests — small fixture (bench prototype equivalent test set).
// ============================================================================

#[cfg(test)]
mod tests {
 use super::*;
 use crate::parser::{design_doc_small_fixture, parse_markdown};

 fn fixture_workspace() -> Workspace {
 let mut ws = Workspace::mnemosyne();
 let doc = parse_markdown(design_doc_small_fixture(), "docs/DESIGN.md");
 ws.insert("docs/DESIGN.md", doc);
 ws
 }

 #[test]
 fn section_by_id_finds_numbered() {
 let ws = fixture_workspace();
 let view = section_by_id(&ws, &AtomicStore::default(), "39").expect("§39 exists");
 assert_eq!(view.section_id, "39");
 assert_eq!(view.title, "Graph schema codegen");
 assert_eq!(view.parent_doc, "docs/DESIGN.md");
 assert_eq!(view.decision_status, "active");
 assert!(view.line_anchor > 0);
 }

 #[test]
 fn section_by_id_returns_none_for_unknown() {
 let ws = fixture_workspace();
 assert!(section_by_id(&ws, &AtomicStore::default(), "999").is_none());
 }

 #[test]
 fn related_sections_outbound_includes_decision_refs() {
 let ws = fixture_workspace();
 let related = related_sections(&ws, "39");
 assert!(related
 .outbound_refs
 .iter()
 .any(|r| r.to_target == "41" && r.ref_kind == "decision"));
 }

 #[test]
 fn related_sections_inbound_includes_external_citation() {
 let ws = fixture_workspace();
 let related_41 = related_sections(&ws, "41");
 assert!(related_41
 .inbound_refs
 .iter()
 .any(|r| r.from_section == "39"));
 }

 #[test]
 fn changelog_entries_for_section_detects_citation() {
 let ws = fixture_workspace();
 let store = AtomicStore::default();
 let entries = changelog_entries_for_section(&ws, &store, "39");
 assert!(entries.iter().any(|e| e.entry_id == "Round 60"));
 }

 #[test]
 fn count_citations_excludes_substring_match() {
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
 assert_eq!(count_citations(&entry, None, "§43", "43"), 1);
 }

 #[test]
 fn count_citations_includes_atomic_fulltext() {
 // atomic-first surface: atomic 5 field fulltext in §N detect.
 let entry = ChangelogEntry {
 entry_id: "Round X".to_string(),
 parent_changelog_entry: None,
 sub_bullets: vec![],
 frozen_at_transaction_time: 1,
 };
 let atomic = AtomicChangelogEntry {
 decision_summary: Some("§43 cited in summary".to_string()),
 changes_bullets: vec!["§43 in changes".to_string()],
 verification_bullets: vec!["§43 in verify".to_string()],
 impact_refs: vec![],
 carry_forward_bullets: vec!["§43 in carry".to_string()],
 };
 assert_eq!(count_citations(&entry, Some(&atomic), "§43", "43"), 4);
 }

 #[test]
 fn count_citations_includes_atomic_impact_refs_structural() {
 // impact_refs structural match (1 iteration count, fulltext distinct).
 let entry = ChangelogEntry {
 entry_id: "Round X".to_string(),
 parent_changelog_entry: None,
 sub_bullets: vec![],
 frozen_at_transaction_time: 1,
 };
 let atomic = AtomicChangelogEntry {
 decision_summary: None,
 changes_bullets: vec![],
 verification_bullets: vec![],
 impact_refs: vec!["43".to_string(), "61".to_string()],
 carry_forward_bullets: vec![],
 };
 assert_eq!(count_citations(&entry, Some(&atomic), "§43", "43"), 1);
 assert_eq!(count_citations(&entry, Some(&atomic), "§99", "99"), 0);
 }

 #[test]
 fn section_by_id_atomic_first_body_source() {
 // atomic-first body: atomic store in section is present
 // synthesize_section_body result SectionView.body authoritative source.
 use crate::atomic::AtomicSection;
 let mut ws = Workspace::mnemosyne();
 let mut doc = ParsedDoc::default();
 doc.sections.push(Section {
 section_id: "999".to_string(),
 parent_doc: "docs/DESIGN.md".to_string(),
 parent_section: None,
 title: "Test".to_string(),
 decision_status: DecisionStatus::Active,
 });
 doc.bodies
 .insert("999".to_string(), "legacy markdown body".to_string());
 ws.insert("docs/DESIGN.md", doc);

 let mut store = AtomicStore::default();
 store.sections.insert(
 "999".to_string(),
 AtomicSection {
  intent: Some("atomic intent".to_string()),
  rationale_bullets: vec!["r1".to_string()],
  ..Default::default()
 },
 );
 let view = section_by_id(&ws, &store, "999").expect("§999 exists");
 // atomic-first: body atomic synthesized result (intent + rationale).
 assert!(view.body.contains("atomic intent"));
 assert!(view.body.contains("- r1"));
 assert!(
 !view.body.contains("legacy markdown body"),
 "atomic-first overrides parsed.bodies"
 );
 }

 #[test]
 fn section_by_id_legacy_body_fallback() {
 // atomic store unregistered section: parsed.bodies fallback carry.
 let mut ws = Workspace::mnemosyne();
 let mut doc = ParsedDoc::default();
 doc.sections.push(Section {
 section_id: "888".to_string(),
 parent_doc: "docs/DESIGN.md".to_string(),
 parent_section: None,
 title: "Test".to_string(),
 decision_status: DecisionStatus::Active,
 });
 doc.bodies
 .insert("888".to_string(), "legacy body content".to_string());
 ws.insert("docs/DESIGN.md", doc);

 let store = AtomicStore::default();
 let view = section_by_id(&ws, &store, "888").expect("§888 exists");
 assert_eq!(view.body, "legacy body content");
 }

 #[test]
 fn section_by_id_atomic_only_section_surface() {
 // markdown missing + atomic store standalone registered section carry.
 // MD-DELETION-RATIFY thereafter sole iteration source path.
 use crate::atomic::AtomicSection;
 let ws = Workspace::mnemosyne();
 let mut store = AtomicStore::default();
 store.sections.insert(
 "777".to_string(),
 AtomicSection {
  intent: Some("atomic-only test".to_string()),
  ..Default::default()
 },
 );
 let view = section_by_id(&ws, &store, "777").expect("§777 atomic-only");
 assert_eq!(view.parent_doc, ATOMIC_ONLY_PARENT_DOC);
 assert!(view.body.contains("atomic-only test"));
 }

 #[test]
 fn section_by_id_atomic_decision_status_overrides_parser_default() {
 // Round 265 — atomic store's decision_status field, when Some(_),
 // overrides the parser's hardcoded Active. Verifies both code paths:
 // (1) markdown-backed section + atomic override, (2) atomic-only section
 // with explicit Superseded.
 use crate::atomic::AtomicSection;

 // Path 1: markdown-backed section with Active parser status, atomic
 // override to Superseded.
 let mut ws = Workspace::mnemosyne();
 let mut doc = ParsedDoc::default();
 doc.sections.push(Section {
 section_id: "555".to_string(),
 parent_doc: "docs/DESIGN.md".to_string(),
 parent_section: None,
 title: "MD-backed".to_string(),
 decision_status: DecisionStatus::Active,
 });
 ws.insert("docs/DESIGN.md", doc);

 let mut store = AtomicStore::default();
 store.sections.insert(
 "555".to_string(),
 AtomicSection {
  decision_status: Some(DecisionStatus::Superseded),
  ..Default::default()
 },
 );
 let view = section_by_id(&ws, &store, "555").expect("§555 exists");
 assert_eq!(
 view.decision_status, "superseded",
 "atomic Some(Superseded) overrides parser-hardcoded Active"
 );

 // Path 2: atomic-only section with explicit Removed.
 let ws2 = Workspace::mnemosyne();
 let mut store2 = AtomicStore::default();
 store2.sections.insert(
 "666".to_string(),
 AtomicSection {
  intent: Some("removed atomic-only".to_string()),
  decision_status: Some(DecisionStatus::Removed),
  ..Default::default()
 },
 );
 let view2 = section_by_id(&ws2, &store2, "666").expect("§666 atomic-only");
 assert_eq!(view2.decision_status, "removed");

 // Path 3: atomic field None (default) — parser status carries through.
 let mut ws3 = Workspace::mnemosyne();
 let mut doc3 = ParsedDoc::default();
 doc3.sections.push(Section {
 section_id: "444".to_string(),
 parent_doc: "docs/DESIGN.md".to_string(),
 parent_section: None,
 title: "no override".to_string(),
 decision_status: DecisionStatus::Active,
 });
 ws3.insert("docs/DESIGN.md", doc3);
 let mut store3 = AtomicStore::default();
 store3.sections.insert(
 "444".to_string(),
 AtomicSection {
  intent: Some("no status override".to_string()),
  decision_status: None,
  ..Default::default()
 },
 );
 let view3 = section_by_id(&ws3, &store3, "444").expect("§444 exists");
 assert_eq!(
 view3.decision_status, "active",
 "atomic None falls back to parser-derived status"
 );
 }

 #[test]
 fn changelog_entries_for_section_surfaces_atomic_only_entry() {
 // markdown missing + atomic_store standalone entry also query carry.
 // MD-DELETION-RATIFY thereafter sole iteration source path.
 let ws = Workspace::mnemosyne();
 let mut store = AtomicStore::default();
 store.changelog_entries.insert(
 "Round 999".to_string(),
 AtomicChangelogEntry {
  decision_summary: Some("atomic-only test".to_string()),
  changes_bullets: vec![],
  verification_bullets: vec![],
  impact_refs: vec!["39".to_string()],
  carry_forward_bullets: vec![],
 },
 );
 let entries = changelog_entries_for_section(&ws, &store, "39");
 assert_eq!(entries.len(), 1);
 assert_eq!(entries[0].entry_id, "Round 999");
 assert_eq!(entries[0].parent_doc, ATOMIC_ONLY_PARENT_DOC);
 assert_eq!(entries[0].citation_count, 1);
 }

 #[test]
 fn changelog_entries_for_section_dedupes_markdown_and_atomic() {
 // markdown observed entry atomic-only dedupe.
 let mut ws = Workspace::mnemosyne();
 let mut doc = ParsedDoc::default();
 doc.changelog_entries.push(ChangelogEntry {
 entry_id: "Round 100".to_string(),
 parent_changelog_entry: None,
 sub_bullets: vec!["§39 cited".to_string()],
 frozen_at_transaction_time: 1,
 });
 ws.insert("docs/DESIGN.md", doc);

 let mut store = AtomicStore::default();
 store.changelog_entries.insert(
 "Round 100".to_string(),
 AtomicChangelogEntry {
  decision_summary: Some("test".to_string()),
  changes_bullets: vec![],
  verification_bullets: vec![],
  impact_refs: vec!["39".to_string()],
  carry_forward_bullets: vec![],
 },
 );
 let entries = changelog_entries_for_section(&ws, &store, "39");
 assert_eq!(entries.len(), 1, "single entry, no dupe");
 assert_eq!(entries[0].parent_doc, "docs/DESIGN.md");
 }

 #[test]
 fn changelog_entries_for_section_surfaces_atomic_fields() {
 // atomic surface field exposed validation.
 let mut ws = Workspace::mnemosyne();
 let mut doc = ParsedDoc::default();
 doc.changelog_entries.push(ChangelogEntry {
 entry_id: "Round 245".to_string(),
 parent_changelog_entry: None,
 sub_bullets: vec![],
 frozen_at_transaction_time: 1,
 });
 ws.insert("docs/DESIGN.md", doc);

 let mut store = AtomicStore::default();
 store.changelog_entries.insert(
 "Round 245".to_string(),
 AtomicChangelogEntry {
  decision_summary: Some("test summary".to_string()),
  changes_bullets: vec!["c1".to_string()],
  verification_bullets: vec!["v1".to_string()],
  impact_refs: vec!["39".to_string()],
  carry_forward_bullets: vec!["carry".to_string()],
 },
 );
 let entries = changelog_entries_for_section(&ws, &store, "39");
 assert_eq!(entries.len(), 1);
 let e = &entries[0];
 assert_eq!(e.atomic_decision_summary.as_deref(), Some("test summary"));
 assert_eq!(e.atomic_changes_bullets, vec!["c1".to_string()]);
 assert_eq!(e.atomic_verification_bullets, vec!["v1".to_string()]);
 assert_eq!(e.atomic_impact_refs, vec!["39".to_string()]);
 assert_eq!(e.atomic_carry_forward_bullets, vec!["carry".to_string()]);
 assert_eq!(e.citation_count, 1);
 }

 #[test]
 fn workspace_section_id_set_contains_known_sections() {
 let ws = fixture_workspace();
 let set = workspace_section_id_set(&ws);
 assert!(set.contains("39"));
 assert!(set.contains("61"));
 }

 #[test]
 fn build_envelope_serializes_to_json() {
 let ws = fixture_workspace();
 let store = AtomicStore::default();
 let env = build_envelope(&ws, &store, "39").expect("§39 exists");
 let json = serde_json::to_string_pretty(&env).expect("serialize");
 assert!(json.contains("\"section_id\": \"39\""));
 assert!(json.contains("\"outbound_refs\""));
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
