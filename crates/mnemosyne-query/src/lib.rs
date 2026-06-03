//! *Spec query API surface* — production lift.
//!
//! bench prototype (`bench/crates/codegen-prototype/src/query_api.rs`)
//! First round of the production lift. *Spec query API surface* body
//! registered + prerequisite #5 *AI agent dogfood proof* binding source — *AI
//! agent (Claude / future LLM) markdown grep = 0 calls + 0 direct GENERATED.md reads
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

use mnemosyne_atomic::{
    synthesize_section_body, AtomicChangelogEntry, AtomicSection, AtomicStore, InventoryEntry,
    NormativeExcerpt,
};
use mnemosyne_core::DecisionStatus;
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
    /// External-spec mirror anchor (RFC-002 FR-1). `Some` only when this
    /// Section vendors a normative excerpt; the read-path that lets an
    /// agent/reviewer verify code against the exact spec text the
    /// workspace was built against (RFC-001 UC-1 read-path). Omitted from JSON
    /// for ordinary (non-mirror) Sections.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normative_excerpt: Option<NormativeExcerpt>,
    /// Coverage applicability (Round 389). `Some("informative")` only when the
    /// section is exempt from the coverage axiom (prose-only); omitted from
    /// JSON for ordinary `Normative` sections (the default), so the read
    /// surface stays unchanged for unclassified stores.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coverage_expectation: Option<String>,
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

/// `section_by_id` — look up a section in the atomic store (the SSOT).
///
/// Exact-key lookup; not found → `None` (CLI maps to exit 1 + stderr). Prose
/// is synthesized from the atomic fields; outline fields (title / parent_doc /
/// parent_section / decision_status) come straight from the skeleton.
pub fn section_by_id(atomic_store: &AtomicStore, section_id: &str) -> Option<SectionView> {
    atomic_store
        .section(section_id)
        .map(|atomic| build_section_view(section_id, atomic))
}

fn build_section_view(section_id: &str, atomic: &AtomicSection) -> SectionView {
    SectionView {
        section_id: section_id.to_string(),
        parent_doc: atomic.skeleton.parent_doc.clone(),
        parent_section: atomic.skeleton.parent_section.clone(),
        title: atomic.skeleton.title.clone(),
        decision_status: atomic
            .skeleton
            .decision_status
            .unwrap_or(DecisionStatus::Active)
            .as_str()
            .to_string(),
        body: synthesize_section_body(atomic),
        line_anchor: 0,
        normative_excerpt: atomic.normative_excerpt.clone(),
        // Surface only the `Informative` deviation; ordinary Normative
        // sections omit the field so the JSON stays unchanged.
        coverage_expectation: {
            let tag = atomic.coverage_expectation.as_str();
            (tag != "normative").then(|| tag.to_string())
        },
    }
}

/// 1-hop traversal over the atomic store (the SSOT).
///
/// - **outbound** = this section's own `impact_scope` (sections its decision
///   impacts).
/// - **inbound** = every changelog entry whose `impact_refs` contains
///   `section_id` (from `<atomic-changelog>`) and every section whose
///   `impact_scope` references it (from `<atomic>`).
pub fn related_sections_with_atomic(
    atomic_store: &AtomicStore,
    section_id: &str,
) -> RelatedSections {
    let mut out = RelatedSections::default();
    if let Some(section) = atomic_store.section(section_id) {
        for target in &section.impact_scope {
            out.outbound_refs.push(CrossRefView {
                from_doc: "<atomic>".to_string(),
                from_section: section_id.to_string(),
                to_target: target.clone(),
                ref_kind: "decision".to_string(),
                created_at_changelog_entry: None,
            });
        }
    }
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

/// `changelog_entries_for_section` — detect `§N` citations across the atomic
/// store's changelog entries (the SSOT).
///
/// citation source = the entry's 5 audit fields (decision_summary /
/// changes / verification / carry_forward fulltext) + structural
/// `impact_refs` match. The needle boundary check guards against longer
/// numeric false-positives (next byte after the needle being an ASCII digit
/// or `.` mismatches).
pub fn changelog_entries_for_section(
    atomic_store: &AtomicStore,
    section_id: &str,
) -> Vec<ChangelogEntryView> {
    let mut out = Vec::new();
    let needle = format!("§{}", section_id);
    for (entry_id, atomic) in &atomic_store.changelog_entries {
        let citation_count = count_citations(atomic, &needle, section_id);
        if citation_count > 0 {
            out.push(build_entry_view(entry_id, atomic, citation_count));
        }
    }
    out
}

/// `parent_doc` marker for changelog entry views — entries live in the atomic
/// store, not a markdown doc.
pub const ATOMIC_ONLY_PARENT_DOC: &str = "<atomic>";

fn build_entry_view(
    entry_id: &str,
    atomic: &AtomicChangelogEntry,
    citation_count: usize,
) -> ChangelogEntryView {
    ChangelogEntryView {
        entry_id: entry_id.to_string(),
        parent_doc: ATOMIC_ONLY_PARENT_DOC.to_string(),
        parent_changelog_entry: None,
        frozen_at_transaction_time: 0,
        sub_bullets: Vec::new(),
        citation_count,
        atomic_decision_summary: atomic.decision_summary.clone(),
        atomic_changes_bullets: atomic.changes_bullets.clone(),
        atomic_verification_bullets: atomic.verification_bullets.clone(),
        atomic_impact_refs: atomic.impact_refs.clone(),
        atomic_carry_forward_bullets: atomic.carry_forward_bullets.clone(),
    }
}

fn count_citations(atomic: &AtomicChangelogEntry, needle: &str, section_id: &str) -> usize {
    let mut count = 0usize;
    if let Some(decision) = &atomic.decision_summary {
        count += count_needle_in(decision, needle);
    }
    for b in &atomic.changes_bullets {
        count += count_needle_in(b, needle);
    }
    for b in &atomic.verification_bullets {
        count += count_needle_in(b, needle);
    }
    for b in &atomic.carry_forward_bullets {
        count += count_needle_in(b, needle);
    }
    // structural cross-ref — impact_refs direct match (entry §N impact target).
    for r in &atomic.impact_refs {
        if r == section_id {
            count += 1;
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

/// `build_envelope` — section_by_id + related_sections_with_atomic +
/// changelog_entries_for_section unified envelope (Claude-consumable).
pub fn build_envelope(atomic_store: &AtomicStore, section_id: &str) -> Option<QueryEnvelope> {
    let section = section_by_id(atomic_store, section_id)?;
    let related = related_sections_with_atomic(atomic_store, section_id);
    let changelog = changelog_entries_for_section(atomic_store, section_id);
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
    /// `"examples[1].code"`, `"bindings[0].file"`.
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
pub fn query_term(store: &AtomicStore, q: &TermQuery) -> Result<Vec<TermHit>, QueryTermError> {
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
    if field_allowed(filter, "title") && !s.skeleton.title.is_empty() {
        push_simple_hit(
            TermTargetKind::Section,
            section_id,
            "title".to_string(),
            &s.skeleton.title,
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
    if field_allowed(filter, "bindings") {
        for (i, im) in s.bindings.iter().enumerate() {
            if m.is_match(&im.file) {
                out.push(TermHit {
                    target_kind: TermTargetKind::Section,
                    target_id: section_id.to_string(),
                    field_path: format!("bindings[{}].file", i),
                    line_context: im.file.clone(),
                });
            }
            if let Some(sym) = im.symbol.as_deref() {
                if m.is_match(sym) {
                    out.push(TermHit {
                        target_kind: TermTargetKind::Section,
                        target_id: section_id.to_string(),
                        field_path: format!("bindings[{}].symbol", i),
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

    // --- store-direct query primitive tests (R400) ---

    fn seed_section(store: &mut AtomicStore, id: &str, title: &str, intent: &str) {
        store.sections.insert(
            id.to_string(),
            AtomicSection {
                skeleton: mnemosyne_core::SectionSkeleton {
                    title: title.into(),
                    parent_doc: "spec".into(),
                    ..Default::default()
                },
                intent: Some(intent.into()),
                ..Default::default()
            },
        );
    }

    #[test]
    fn section_by_id_reads_atomic_store() {
        let mut store = AtomicStore::default();
        seed_section(&mut store, "39", "Graph schema", "tracks graph schema");
        let view = section_by_id(&store, "39").expect("section 39 exists");
        assert_eq!(view.section_id, "39");
        assert_eq!(view.title, "Graph schema");
        assert_eq!(view.parent_doc, "spec");
        assert_eq!(view.decision_status, "active");
    }

    #[test]
    fn section_by_id_unknown_is_none() {
        let store = AtomicStore::default();
        assert!(section_by_id(&store, "999").is_none());
    }

    #[test]
    fn related_sections_outbound_from_impact_scope_inbound_from_refs() {
        let mut store = AtomicStore::default();
        seed_section(&mut store, "39", "A", "a");
        store.sections.get_mut("39").unwrap().impact_scope = vec!["41".into()];
        seed_section(&mut store, "41", "B", "b");
        store.changelog_entries.insert(
            "Round 1".into(),
            AtomicChangelogEntry {
                decision_summary: Some("touches 41".into()),
                impact_refs: vec!["41".into()],
                ..Default::default()
            },
        );
        let r39 = related_sections_with_atomic(&store, "39");
        assert!(r39.outbound_refs.iter().any(|x| x.to_target == "41"));
        let r41 = related_sections_with_atomic(&store, "41");
        assert!(r41.inbound_refs.iter().any(|x| x.from_section == "39"));
        assert!(r41.inbound_refs.iter().any(|x| x.from_section == "Round 1"));
    }

    #[test]
    fn changelog_entries_for_section_counts_atomic_citations() {
        let m = '\u{a7}'; // section sign U+00A7 (avoid a literal citation in source)
        let mut store = AtomicStore::default();
        store.changelog_entries.insert(
            "Round 60".into(),
            AtomicChangelogEntry {
                decision_summary: Some(format!("supersedes {m}39 decision")),
                impact_refs: vec!["39".into()],
                ..Default::default()
            },
        );
        let entries = changelog_entries_for_section(&store, "39");
        assert!(entries
            .iter()
            .any(|e| e.entry_id == "Round 60" && e.citation_count >= 1));
    }

    #[test]
    fn build_envelope_serializes_to_json() {
        let mut store = AtomicStore::default();
        seed_section(&mut store, "39", "Graph schema", "x");
        let env = build_envelope(&store, "39").expect("section 39 exists");
        let json = serde_json::to_string_pretty(&env).expect("serialize");
        assert!(json.contains("\"section_id\": \"39\""));
        assert!(json.contains("\"outbound_refs\""));
    }

    // ========================================================================
    // Round 292 — query_term test suite.
    // ========================================================================

    use mnemosyne_atomic::{
        AtomicChangelogEntry, AtomicSection, Binding, BindingKind, ExampleBlock, InventoryEntry,
        RejectedAlternative,
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
    fn query_term_scans_bindings() {
        let section = AtomicSection {
            bindings: vec![Binding {
                kind: BindingKind::Implements,
                file: "src/secret/handler.rs".to_string(),
                symbol: Some("fn redact_secret".to_string()),
            }],
            ..Default::default()
        };
        let store = store_with_one_section("42", section);
        let hits = query_term(&store, &literal_q("secret")).expect("ok");
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].field_path, "bindings[0].file");
        assert_eq!(hits[1].field_path, "bindings[0].symbol");
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
