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

pub mod render;
pub use render::*;

use mnemosyne_atomic::{
    synthesize_section_body, AtomicChangelogEntry, AtomicSection, AtomicStore, InventoryEntry,
};
use mnemosyne_schema::{ChangelogEntry, CrossRef, ParsedDoc, RefKind, Section};
use mnemosyne_core::DecisionStatus;
use mnemosyne_workspace::Workspace;
use serde::Serialize;
use std::collections::BTreeSet;
use thiserror::Error;

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
 //
 // Round 287 — uses atomic outline fields directly (title / parent_doc /
 // parent_section). The legacy `ATOMIC_ONLY_PARENT_DOC` sentinel and the
 // intent→title fallback are retired now that the atomic store carries
 // the closed-form Section shape. Outline fields default to empty strings
 // for pre-backfill sections; callers see honest data rather than
 // synthesized placeholders.
 if let Some(atomic) = atomic_store.section(section_id) {
 let synthetic_section = Section {
 section_id: section_id.to_string(),
 parent_doc: atomic.parent_doc.clone(),
 parent_section: atomic.parent_section.clone(),
 title: atomic.title.clone(),
 decision_status: atomic.decision_status.unwrap_or(DecisionStatus::Active),
 // Synthetic section is built directly from the atomic store, so
 // the lookup key is the atomic id verbatim.
 atomic_section_id: Some(section_id.to_string()),
 };
 let synthetic_doc = ParsedDoc::default();
 return Some(build_section_view(
 &atomic.parent_doc,
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
 // atomic-first body source. Lookup goes through `AtomicStore::resolve`,
 // which honours the parser's `atomic_section_id` bridge (the bare
 // heading `§<token>` slot) so nested `### §<id>` headings emitted
 // under `## Sections` find their atomic counterpart instead of silently
 // falling back to the raw markdown body.
 let atomic = atomic_store.resolve(section);
 let body = if let Some(atomic) = atomic {
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
 let resolved_status = atomic
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
// Round 292 — `query_term` read primitive (literal/regex search).
//
// Pure read over atomic store. Pattern matches against text-typed atomic
// fields across Section (title, intent, bullets, struct sub-fields),
// ChangelogEntry (decision_summary + 4 bullet lists), and Inventory (source,
// reason). Replaces external `grep` over generated artifacts with a
// store-aware search that knows field provenance — required substrate for
// the deferred `redact_term` mutate primitive, also useful standalone.
//
// Out of scope (v1):
//   - Legacy parser-side `sub_bullets` (handled by separate parser query).
//   - Structural pointer fields (parent_doc, parent_section) — these are
//     indexed elsewhere; scanning them as text would conflate identifiers
//     with content.
//   - Enum fields (decision_status, inventory status) — single-token,
//     better surfaced via dedicated list queries.
// ============================================================================

/// Search mode for [`query_term`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TermMode {
    /// Literal substring match; regex meta-characters not interpreted.
    Literal,
    /// Regex match (compiled via the `regex` crate).
    Regex,
}

/// Which entity kinds [`query_term`] should scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TermScope {
    All,
    Sections,
    ChangelogEntries,
    Inventory,
}

/// Entity kind a [`TermHit`] originated from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TermTargetKind {
    Section,
    ChangelogEntry,
    Inventory,
}

/// Read-time query for [`query_term`].
#[derive(Debug, Clone)]
pub struct TermQuery {
    pub pattern: String,
    pub mode: TermMode,
    pub case_insensitive: bool,
    pub scope: TermScope,
    /// Restrict scan to a set of field names (e.g. `{"intent", "decision_summary"}`).
    /// `None` = scan every text-typed field in scope.
    /// Field names use the base name (no `[i]` index suffix); for struct
    /// sub-fields, the *containing list* name applies (e.g. `"alternatives_rejected"`
    /// covers both `.alternative` and `.reason` sub-paths).
    pub field_filter: Option<BTreeSet<String>>,
}

/// One match returned by [`query_term`].
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TermHit {
    pub target_kind: TermTargetKind,
    pub target_id: String,
    /// Field path inside the entity. Examples: `"intent"`,
    /// `"rationale_bullets[2]"`, `"alternatives_rejected[0].reason"`,
    /// `"examples[1].code"`, `"implementations[0].file"`.
    pub field_path: String,
    /// Full text of the matched field (or bullet / struct sub-field).
    /// Not truncated — the caller decides how to display.
    pub line_context: String,
}

/// Failure modes for [`query_term`]. Only one path can fail today (regex
/// compile); kept as an enum for forward compatibility.
#[derive(Debug, Error)]
pub enum QueryTermError {
    #[error("invalid regex pattern: {0}")]
    InvalidRegex(#[from] regex::Error),
}

/// `query_term` — literal/regex scan over the atomic store.
///
/// Returns hits in deterministic order: target_kind variant order ×
/// `BTreeMap` key order × field declaration order × bullet index order.
/// Pure read — `store` is not modified.
pub fn query_term(
    store: &AtomicStore,
    q: &TermQuery,
) -> Result<Vec<TermHit>, QueryTermError> {
    let matcher = Matcher::build(&q.pattern, q.mode, q.case_insensitive)?;
    let mut hits = Vec::new();

    if matches!(q.scope, TermScope::All | TermScope::Sections) {
        for (id, section) in &store.sections {
            scan_section(id, section, &matcher, q.field_filter.as_ref(), &mut hits);
        }
    }
    if matches!(q.scope, TermScope::All | TermScope::ChangelogEntries) {
        for (id, entry) in &store.changelog_entries {
            scan_changelog_entry(id, entry, &matcher, q.field_filter.as_ref(), &mut hits);
        }
    }
    if matches!(q.scope, TermScope::All | TermScope::Inventory) {
        for (id, inv) in &store.inventory_entries {
            scan_inventory_entry(id, inv, &matcher, q.field_filter.as_ref(), &mut hits);
        }
    }

    Ok(hits)
}

enum Matcher {
    Literal {
        needle_lower: String,
        case_insensitive: bool,
        needle_raw: String,
    },
    Regex(regex::Regex),
}

impl Matcher {
    fn build(
        pattern: &str,
        mode: TermMode,
        case_insensitive: bool,
    ) -> Result<Self, QueryTermError> {
        match mode {
            TermMode::Literal => Ok(Matcher::Literal {
                needle_lower: pattern.to_lowercase(),
                case_insensitive,
                needle_raw: pattern.to_string(),
            }),
            TermMode::Regex => {
                let re = regex::RegexBuilder::new(pattern)
                    .case_insensitive(case_insensitive)
                    .build()?;
                Ok(Matcher::Regex(re))
            }
        }
    }

    fn is_match(&self, hay: &str) -> bool {
        match self {
            Matcher::Literal {
                needle_lower,
                case_insensitive,
                needle_raw,
            } => {
                if *case_insensitive {
                    hay.to_lowercase().contains(needle_lower.as_str())
                } else {
                    hay.contains(needle_raw.as_str())
                }
            }
            Matcher::Regex(re) => re.is_match(hay),
        }
    }
}

fn field_allowed(filter: Option<&BTreeSet<String>>, field: &str) -> bool {
    filter.is_none_or(|s| s.contains(field))
}

fn push_simple_hit(
    target_kind: TermTargetKind,
    target_id: &str,
    field_path: String,
    line_context: &str,
    m: &Matcher,
    out: &mut Vec<TermHit>,
) {
    if m.is_match(line_context) {
        out.push(TermHit {
            target_kind,
            target_id: target_id.to_string(),
            field_path,
            line_context: line_context.to_string(),
        });
    }
}

fn push_bullets(
    target_kind: TermTargetKind,
    target_id: &str,
    field: &'static str,
    bullets: &[String],
    m: &Matcher,
    filter: Option<&BTreeSet<String>>,
    out: &mut Vec<TermHit>,
) {
    if !field_allowed(filter, field) {
        return;
    }
    for (i, b) in bullets.iter().enumerate() {
        if m.is_match(b) {
            out.push(TermHit {
                target_kind,
                target_id: target_id.to_string(),
                field_path: format!("{}[{}]", field, i),
                line_context: b.clone(),
            });
        }
    }
}

fn scan_section(
    section_id: &str,
    s: &AtomicSection,
    m: &Matcher,
    filter: Option<&BTreeSet<String>>,
    out: &mut Vec<TermHit>,
) {
    if field_allowed(filter, "title") && !s.title.is_empty() {
        push_simple_hit(
            TermTargetKind::Section,
            section_id,
            "title".to_string(),
            &s.title,
            m,
            out,
        );
    }
    if field_allowed(filter, "intent") {
        if let Some(intent) = s.intent.as_deref() {
            push_simple_hit(
                TermTargetKind::Section,
                section_id,
                "intent".to_string(),
                intent,
                m,
                out,
            );
        }
    }
    push_bullets(
        TermTargetKind::Section,
        section_id,
        "rationale_bullets",
        &s.rationale_bullets,
        m,
        filter,
        out,
    );
    push_bullets(
        TermTargetKind::Section,
        section_id,
        "inputs_bullets",
        &s.inputs_bullets,
        m,
        filter,
        out,
    );
    push_bullets(
        TermTargetKind::Section,
        section_id,
        "outputs_bullets",
        &s.outputs_bullets,
        m,
        filter,
        out,
    );
    push_bullets(
        TermTargetKind::Section,
        section_id,
        "caveats_bullets",
        &s.caveats_bullets,
        m,
        filter,
        out,
    );
    push_bullets(
        TermTargetKind::Section,
        section_id,
        "impact_scope",
        &s.impact_scope,
        m,
        filter,
        out,
    );
    if field_allowed(filter, "alternatives_rejected") {
        for (i, alt) in s.alternatives_rejected.iter().enumerate() {
            if m.is_match(&alt.alternative) {
                out.push(TermHit {
                    target_kind: TermTargetKind::Section,
                    target_id: section_id.to_string(),
                    field_path: format!("alternatives_rejected[{}].alternative", i),
                    line_context: alt.alternative.clone(),
                });
            }
            if m.is_match(&alt.reason) {
                out.push(TermHit {
                    target_kind: TermTargetKind::Section,
                    target_id: section_id.to_string(),
                    field_path: format!("alternatives_rejected[{}].reason", i),
                    line_context: alt.reason.clone(),
                });
            }
        }
    }
    if field_allowed(filter, "examples") {
        for (i, ex) in s.examples.iter().enumerate() {
            if m.is_match(&ex.code) {
                out.push(TermHit {
                    target_kind: TermTargetKind::Section,
                    target_id: section_id.to_string(),
                    field_path: format!("examples[{}].code", i),
                    line_context: ex.code.clone(),
                });
            }
        }
    }
    if field_allowed(filter, "implementations") {
        for (i, im) in s.implementations.iter().enumerate() {
            if m.is_match(&im.file) {
                out.push(TermHit {
                    target_kind: TermTargetKind::Section,
                    target_id: section_id.to_string(),
                    field_path: format!("implementations[{}].file", i),
                    line_context: im.file.clone(),
                });
            }
            if let Some(sym) = im.symbol.as_deref() {
                if m.is_match(sym) {
                    out.push(TermHit {
                        target_kind: TermTargetKind::Section,
                        target_id: section_id.to_string(),
                        field_path: format!("implementations[{}].symbol", i),
                        line_context: sym.to_string(),
                    });
                }
            }
        }
    }
}

fn scan_changelog_entry(
    entry_id: &str,
    e: &AtomicChangelogEntry,
    m: &Matcher,
    filter: Option<&BTreeSet<String>>,
    out: &mut Vec<TermHit>,
) {
    if field_allowed(filter, "decision_summary") {
        if let Some(s) = e.decision_summary.as_deref() {
            push_simple_hit(
                TermTargetKind::ChangelogEntry,
                entry_id,
                "decision_summary".to_string(),
                s,
                m,
                out,
            );
        }
    }
    push_bullets(
        TermTargetKind::ChangelogEntry,
        entry_id,
        "changes_bullets",
        &e.changes_bullets,
        m,
        filter,
        out,
    );
    push_bullets(
        TermTargetKind::ChangelogEntry,
        entry_id,
        "verification_bullets",
        &e.verification_bullets,
        m,
        filter,
        out,
    );
    push_bullets(
        TermTargetKind::ChangelogEntry,
        entry_id,
        "impact_refs",
        &e.impact_refs,
        m,
        filter,
        out,
    );
    push_bullets(
        TermTargetKind::ChangelogEntry,
        entry_id,
        "carry_forward_bullets",
        &e.carry_forward_bullets,
        m,
        filter,
        out,
    );
}

fn scan_inventory_entry(
    inv_id: &str,
    inv: &InventoryEntry,
    m: &Matcher,
    filter: Option<&BTreeSet<String>>,
    out: &mut Vec<TermHit>,
) {
    if field_allowed(filter, "source") {
        if let Some(s) = inv.source.as_deref() {
            push_simple_hit(
                TermTargetKind::Inventory,
                inv_id,
                "source".to_string(),
                s,
                m,
                out,
            );
        }
    }
    if field_allowed(filter, "reason") {
        if let Some(r) = inv.reason.as_deref() {
            push_simple_hit(
                TermTargetKind::Inventory,
                inv_id,
                "reason".to_string(),
                r,
                m,
                out,
            );
        }
    }
}

// ============================================================================
// Tests — small fixture (bench prototype equivalent test set).
// ============================================================================

#[cfg(test)]
mod tests {
 use super::*;
 use mnemosyne_parser::{design_doc_small_fixture, parse_markdown};

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
 ..Default::default()
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
 ..Default::default()
 };
 assert_eq!(count_citations(&entry, Some(&atomic), "§43", "43"), 1);
 assert_eq!(count_citations(&entry, Some(&atomic), "§99", "99"), 0);
 }

 #[test]
 fn section_by_id_atomic_first_body_source() {
 // atomic-first body: atomic store in section is present
 // synthesize_section_body result SectionView.body authoritative source.
 use mnemosyne_atomic::AtomicSection;
 let mut ws = Workspace::mnemosyne();
 let mut doc = ParsedDoc::default();
 doc.sections.push(Section {
 section_id: "999".to_string(),
 parent_doc: "docs/DESIGN.md".to_string(),
 parent_section: None,
 title: "Test".to_string(),
 decision_status: DecisionStatus::Active,
 atomic_section_id: None,
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
 atomic_section_id: None,
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
 //
 // Round 287 — atomic-only surface now uses real outline fields
 // (title / parent_doc / parent_section) instead of the legacy
 // ATOMIC_ONLY_PARENT_DOC sentinel + intent→title fallback.
 use mnemosyne_atomic::AtomicSection;
 let ws = Workspace::mnemosyne();
 let mut store = AtomicStore::default();
 store.sections.insert(
 "777".to_string(),
 AtomicSection {
  title: "Atomic-only Test".to_string(),
  parent_doc: "docs/GENERATED.md".to_string(),
  intent: Some("atomic-only test".to_string()),
  ..Default::default()
 },
 );
 let view = section_by_id(&ws, &store, "777").expect("§777 atomic-only");
 assert_eq!(view.parent_doc, "docs/GENERATED.md");
 assert_eq!(view.title, "Atomic-only Test");
 assert!(view.body.contains("atomic-only test"));
 }

 #[test]
 fn section_by_id_atomic_decision_status_overrides_parser_default() {
 // Round 265 — atomic store's decision_status field, when Some(_),
 // overrides the parser's hardcoded Active. Verifies both code paths:
 // (1) markdown-backed section + atomic override, (2) atomic-only section
 // with explicit Superseded.
 use mnemosyne_atomic::AtomicSection;

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
 atomic_section_id: None,
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
 atomic_section_id: None,
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
  ..Default::default()
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
  ..Default::default()
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
  ..Default::default()
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

 // ========================================================================
 // Round 292 — query_term test suite.
 // ========================================================================

 use mnemosyne_atomic::{
 AtomicChangelogEntry, AtomicSection, ExampleBlock, Implementation,
 InventoryEntry, RejectedAlternative,
 };
 use mnemosyne_core::InventoryStatus;

 fn store_with_one_section(id: &str, s: AtomicSection) -> AtomicStore {
 let mut store = AtomicStore::default();
 store.sections.insert(id.to_string(), s);
 store
 }

 fn store_with_one_entry(id: &str, e: AtomicChangelogEntry) -> AtomicStore {
 let mut store = AtomicStore::default();
 store.changelog_entries.insert(id.to_string(), e);
 store
 }

 fn literal_q(pattern: &str) -> TermQuery {
 TermQuery {
 pattern: pattern.to_string(),
 mode: TermMode::Literal,
 case_insensitive: false,
 scope: TermScope::All,
 field_filter: None,
 }
 }

 #[test]
 fn query_term_literal_matches_section_intent() {
 let section = AtomicSection {
 intent: Some("Tracks the foo subsystem".to_string()),
 ..Default::default()
 };
 let store = store_with_one_section("42", section);
 let hits = query_term(&store, &literal_q("foo")).expect("ok");
 assert_eq!(hits.len(), 1);
 assert_eq!(hits[0].target_kind, TermTargetKind::Section);
 assert_eq!(hits[0].target_id, "42");
 assert_eq!(hits[0].field_path, "intent");
 }

 #[test]
 fn query_term_literal_is_case_sensitive_by_default() {
 let section = AtomicSection {
 intent: Some("Tracks the Foo subsystem".to_string()),
 ..Default::default()
 };
 let store = store_with_one_section("42", section);
 let hits = query_term(&store, &literal_q("foo")).expect("ok");
 assert!(hits.is_empty(), "Foo != foo without case_insensitive");
 }

 #[test]
 fn query_term_case_insensitive_toggle() {
 let section = AtomicSection {
 intent: Some("Tracks the Foo subsystem".to_string()),
 ..Default::default()
 };
 let store = store_with_one_section("42", section);
 let q = TermQuery {
 case_insensitive: true,
 ..literal_q("foo")
 };
 let hits = query_term(&store, &q).expect("ok");
 assert_eq!(hits.len(), 1);
 }

 #[test]
 fn query_term_regex_compiles_and_matches() {
 let section = AtomicSection {
 intent: Some("token abc123 leaked".to_string()),
 ..Default::default()
 };
 let store = store_with_one_section("42", section);
 let q = TermQuery {
 pattern: r"abc\d+".to_string(),
 mode: TermMode::Regex,
 case_insensitive: false,
 scope: TermScope::All,
 field_filter: None,
 };
 let hits = query_term(&store, &q).expect("ok");
 assert_eq!(hits.len(), 1);
 assert_eq!(hits[0].field_path, "intent");
 }

 #[test]
 fn query_term_invalid_regex_surfaces_error() {
 let store = AtomicStore::default();
 let q = TermQuery {
 pattern: "[unterminated".to_string(),
 mode: TermMode::Regex,
 case_insensitive: false,
 scope: TermScope::All,
 field_filter: None,
 };
 assert!(matches!(
 query_term(&store, &q),
 Err(QueryTermError::InvalidRegex(_))
 ));
 }

 #[test]
 fn query_term_indexes_bullets() {
 let section = AtomicSection {
 rationale_bullets: vec![
 "first bullet".to_string(),
 "second bullet contains secret".to_string(),
 "third bullet".to_string(),
 ],
 ..Default::default()
 };
 let store = store_with_one_section("42", section);
 let hits = query_term(&store, &literal_q("secret")).expect("ok");
 assert_eq!(hits.len(), 1);
 assert_eq!(hits[0].field_path, "rationale_bullets[1]");
 }

 #[test]
 fn query_term_scans_alternatives_sub_fields() {
 let section = AtomicSection {
 alternatives_rejected: vec![RejectedAlternative {
 alternative: "use mock DB".to_string(),
 reason: "secret would still leak via tests".to_string(),
 }],
 ..Default::default()
 };
 let store = store_with_one_section("42", section);
 let hits = query_term(&store, &literal_q("secret")).expect("ok");
 assert_eq!(hits.len(), 1);
 assert_eq!(hits[0].field_path, "alternatives_rejected[0].reason");
 }

 #[test]
 fn query_term_scans_examples_code() {
 let section = AtomicSection {
 examples: vec![ExampleBlock {
 language: "rust".to_string(),
 code: "let api_key = \"secret-xyz\";".to_string(),
 }],
 ..Default::default()
 };
 let store = store_with_one_section("42", section);
 let hits = query_term(&store, &literal_q("api_key")).expect("ok");
 assert_eq!(hits.len(), 1);
 assert_eq!(hits[0].field_path, "examples[0].code");
 }

 #[test]
 fn query_term_scans_implementations() {
 let section = AtomicSection {
 implementations: vec![Implementation {
 file: "src/secret/handler.rs".to_string(),
 symbol: Some("fn redact_secret".to_string()),
 }],
 ..Default::default()
 };
 let store = store_with_one_section("42", section);
 let hits = query_term(&store, &literal_q("secret")).expect("ok");
 assert_eq!(hits.len(), 2);
 assert_eq!(hits[0].field_path, "implementations[0].file");
 assert_eq!(hits[1].field_path, "implementations[0].symbol");
 }

 #[test]
 fn query_term_scans_changelog_entry_summary_and_bullets() {
 let entry = AtomicChangelogEntry {
 decision_summary: Some("redact secret tokens from logs".to_string()),
 changes_bullets: vec!["scrub secret env vars".to_string()],
 verification_bullets: vec!["no secret in audit output".to_string()],
 impact_refs: vec!["secret-handling".to_string()],
 carry_forward_bullets: vec!["nothing".to_string()],
 ..Default::default()
 };
 let store = store_with_one_entry("Round 99", entry);
 let hits = query_term(&store, &literal_q("secret")).expect("ok");
 assert_eq!(hits.len(), 4);
 let paths: Vec<&str> = hits.iter().map(|h| h.field_path.as_str()).collect();
 assert!(paths.contains(&"decision_summary"));
 assert!(paths.contains(&"changes_bullets[0]"));
 assert!(paths.contains(&"verification_bullets[0]"));
 assert!(paths.contains(&"impact_refs[0]"));
 }

 #[test]
 fn query_term_scope_filter_excludes_other_kinds() {
 let mut store = AtomicStore::default();
 store.sections.insert(
 "42".to_string(),
 AtomicSection {
 intent: Some("secret intent".to_string()),
 ..Default::default()
 },
 );
 store.changelog_entries.insert(
 "Round 1".to_string(),
 AtomicChangelogEntry {
 decision_summary: Some("secret summary".to_string()),
 ..Default::default()
 },
 );
 let q = TermQuery {
 scope: TermScope::ChangelogEntries,
 ..literal_q("secret")
 };
 let hits = query_term(&store, &q).expect("ok");
 assert_eq!(hits.len(), 1);
 assert_eq!(hits[0].target_kind, TermTargetKind::ChangelogEntry);
 }

 #[test]
 fn query_term_field_filter_restricts_to_named_fields() {
 let section = AtomicSection {
 intent: Some("the secret".to_string()),
 rationale_bullets: vec!["another secret".to_string()],
 ..Default::default()
 };
 let store = store_with_one_section("42", section);
 let mut filter = BTreeSet::new();
 filter.insert("intent".to_string());
 let q = TermQuery {
 field_filter: Some(filter),
 ..literal_q("secret")
 };
 let hits = query_term(&store, &q).expect("ok");
 assert_eq!(hits.len(), 1);
 assert_eq!(hits[0].field_path, "intent");
 }

 #[test]
 fn query_term_scans_inventory_text_fields() {
 let mut store = AtomicStore::default();
 store.inventory_entries.insert(
 "ARP_07".to_string(),
 InventoryEntry {
 status: InventoryStatus::Active,
 section_ref: None,
 source: Some("PDF p.42 internal-doc XYZ".to_string()),
 reason: None,
 },
 );
 let hits = query_term(&store, &literal_q("XYZ")).expect("ok");
 assert_eq!(hits.len(), 1);
 assert_eq!(hits[0].target_kind, TermTargetKind::Inventory);
 assert_eq!(hits[0].field_path, "source");
 }

 #[test]
 fn query_term_empty_store_returns_empty_hits() {
 let store = AtomicStore::default();
 let hits = query_term(&store, &literal_q("anything")).expect("ok");
 assert!(hits.is_empty());
 }

 #[test]
 fn query_term_no_match_returns_empty() {
 let section = AtomicSection {
 intent: Some("clean text".to_string()),
 ..Default::default()
 };
 let store = store_with_one_section("42", section);
 let hits = query_term(&store, &literal_q("absent")).expect("ok");
 assert!(hits.is_empty());
 }

 #[test]
 fn query_term_deterministic_ordering_across_kinds() {
 // Round 292 contract: section_id BTreeMap order × kind-variant order.
 let mut store = AtomicStore::default();
 store.sections.insert(
 "b-section".to_string(),
 AtomicSection {
 intent: Some("X".to_string()),
 ..Default::default()
 },
 );
 store.sections.insert(
 "a-section".to_string(),
 AtomicSection {
 intent: Some("X".to_string()),
 ..Default::default()
 },
 );
 store.changelog_entries.insert(
 "Round 2".to_string(),
 AtomicChangelogEntry {
 decision_summary: Some("X".to_string()),
 ..Default::default()
 },
 );
 let hits = query_term(&store, &literal_q("X")).expect("ok");
 assert_eq!(hits.len(), 3);
 assert_eq!(hits[0].target_id, "a-section");
 assert_eq!(hits[1].target_id, "b-section");
 assert_eq!(hits[2].target_id, "Round 2");
 }
}
