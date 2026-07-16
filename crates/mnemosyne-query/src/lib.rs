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
    synthesize_section_body, AtomicChangelogEntry, AtomicSection, AtomicStore, Binding,
    InventoryEntry, NormativeExcerpt,
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
    /// Verification class (R413). `Some("by_construction")` only when the
    /// section is exempt from the dedicated-verify gate; omitted for the
    /// default `Dedicated` so the read surface stays unchanged for stores that
    /// do not use the verify axis.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification_expectation: Option<String>,
    /// Spec → Code trace-links (Path B `§<id>.bindings`): the ratified
    /// implementing files/symbols for this section. Surfaced on the
    /// per-section read-path so an agent navigating "work on §X" gets the
    /// authoritative, kind-typed file set inline — without a separate
    /// whole-store `report-spec-map` round-trip (which scales O(total
    /// sections)) or a noisy `grep §<id>` over the tree (raw citations, not
    /// the ratified set). Empty for prose-only sections that have no
    /// binding; omitted from JSON then so the read surface is unchanged.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bindings: Vec<Binding>,
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
        verification_expectation: {
            let tag = atomic.verification_expectation.as_str();
            (tag != "dedicated").then(|| tag.to_string())
        },
        bindings: atomic.bindings.clone(),
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

/// ChangelogLedgerView — [`list_changelog`] carry form. `entries` is
/// ascending round order (oldest first); `total` always reports the full
/// ledger size, so a `limit`-bounded read is never mistaken for the whole
/// ledger (no-silent-caps — Round 470).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ChangelogLedgerView {
    pub total: usize,
    pub entries: Vec<ChangelogEntryView>,
}

/// `list_changelog` — the changelog ledger projected to views, in
/// round-number order. `changelog_entries` is keyed by the prose entry_id
/// (`Round <n><suffix> — …`) and the `BTreeMap` iterates lexicographically,
/// which is NOT creation order — neither the numeric part (digits compare
/// left-to-right) nor the alpha suffix (a base-26 column: `z` then `aa` then
/// `ab` …, where lexicographic order wrongly puts `aa < lq < z`). So this
/// sorts by [`round_order_key`]: numeric round, then the alpha suffix as a
/// bijective base-26 column ordinal, then any sub-step tail — making the
/// timeline chronological (ascending = oldest first; a viewer reverses
/// client-side for newest-first). Entries whose key has no leading
/// `Round <n>` sort last, then by key for stability. `limit` keeps only the
/// LAST n entries (the newest — the session-load read; Round 470, pulled by
/// the ledger's monotonic growth) while `total` stays the full count. Drives
/// the Studio changelog timeline and any full-ledger read; the per-section
/// view is [`changelog_entries_for_section`]. `citation_count` is `0` — it is
/// a per-section relevance metric, not applicable to the whole-ledger
/// projection.
pub fn list_changelog(atomic_store: &AtomicStore, limit: Option<usize>) -> ChangelogLedgerView {
    let mut entries: Vec<ChangelogEntryView> = atomic_store
        .changelog_entries
        .iter()
        .map(|(entry_id, atomic)| build_entry_view(entry_id, atomic, 0))
        .collect();
    entries.sort_by_key(|e| round_order_key(&e.entry_id));
    let total = entries.len();
    if let Some(n) = limit {
        if n < total {
            entries.drain(..total - n);
        }
    }
    ChangelogLedgerView { total, entries }
}

/// Parse the leading `Round <n>` of a changelog entry_id into its round
/// number. The single home for entry_id -> round parsing — used by
/// [`list_changelog`]'s ordering and the CLI's ledger-round scan
/// (`collect_ledger_round_numbers`), so the two cannot drift. `None` when
/// the key does not open with `Round <digits>`.
pub fn round_number(entry_id: &str) -> Option<u32> {
    let rest = entry_id.strip_prefix("Round ")?;
    let end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    rest[..end].parse::<u32>().ok()
}

/// Round 474 — total-order sort key for a changelog entry_id, fixing the
/// round ordering for the bijective base-26 alpha suffix the round labels
/// carry.
/// `round_number` alone groups every `Round 311*` at the same numeric round,
/// so a lexicographic tiebreak on the raw id wrongly orders `311aa < 311lq <
/// 311z` — yet creation order is `311z` (column 26) then `311aa` (27) then
/// `311lq` (329). Returns `(round_num, alpha_column, tail)`:
///
/// - `round_num` — the leading numeric round (`311`); non-`Round <n>` keys
///   (e.g. the stray `test`) get `u32::MAX` so they sort last, matching the
///   prior `unwrap_or(u32::MAX)` placement.
/// - `alpha_column` — the leading lowercase-alpha run read as an Excel-column
///   ordinal (`""`=0, `"a"`=1, `"z"`=26, `"aa"`=27, `"lq"`=329); empty suffix
///   sorts before `"a"` so `Round 311 < Round 311a`.
/// - `tail` — whatever follows the alpha run (a `.1` dotted sub-round, a `-pre`
///   prep step, an `a1` enumerated sub-step), compared lexicographically. All
///   `311az*` share column 52 and stay grouped before `311ba` (column 53).
///
/// This is the single home for entry_id ORDERING; [`round_number`] stays the
/// single home for entry_id -> numeric round (its other caller, the CLI
/// ledger-round scan, wants exactly the numeric part), so the two do not drift.
pub fn round_order_key(entry_id: &str) -> (u32, u64, String) {
    let Some(rest) = entry_id.strip_prefix("Round ") else {
        return (u32::MAX, u64::MAX, entry_id.to_string());
    };
    let digit_end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    let Ok(round_num) = rest[..digit_end].parse::<u32>() else {
        return (u32::MAX, u64::MAX, entry_id.to_string());
    };
    let after_num = &rest[digit_end..];
    let alpha_end = after_num
        .find(|c: char| !c.is_ascii_lowercase())
        .unwrap_or(after_num.len());
    let mut alpha_column: u64 = 0;
    for byte in after_num[..alpha_end].bytes() {
        alpha_column = alpha_column * 26 + u64::from(byte - b'a' + 1);
    }
    (round_num, alpha_column, after_num[alpha_end..].to_string())
}

/// `parent_doc` marker for changelog entry views — entries live in the atomic
/// store, not a markdown doc.
pub const ATOMIC_ONLY_PARENT_DOC: &str = "<atomic>";

/// One changelog entry by its EXACT stored key, projected through the same
/// [`build_entry_view`] the whole-ledger read uses (Round 638). `None` when
/// the key is absent.
///
/// Resolving a `Round NNN` CITATION to that key is a separate concern and
/// lives with the citation rule (`mnemosyne-validate` `code_refs`), because a
/// citation names a number while a key may carry a title; `mnemosyne-ops`
/// composes the two. `citation_count` is `0` — it is a per-section relevance
/// metric, not applicable to a single-entry read (the [`list_changelog`]
/// convention).
pub fn changelog_entry(atomic_store: &AtomicStore, entry_id: &str) -> Option<ChangelogEntryView> {
    atomic_store
        .changelog_entries
        .get(entry_id)
        .map(|atomic| build_entry_view(entry_id, atomic, 0))
}

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
// Round 467 — identifier keys (`section_id` / `entry_id` / `inventory_id`)
// are scanned too, symmetric across all three kinds. The v1 omission made
// an ID-pattern search return 0 hits indistinguishably from "entry absent"
// (silent miss); IDs are exactly what agents grep for at session load.
// Restrict via the same field filter when ID matches are unwanted.
//
// Round 468 — field-filter names are validated against the scope's field
// rosters; an unknown name (typo, or a field outside the scanned kinds)
// rejects loudly instead of silently matching nothing.
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
    /// Names are validated against the scope's rosters (Round 468): a name
    /// no scanned kind knows is `QueryTermError::UnknownField`, never a
    /// silent 0-hit result.
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

/// Failure modes for [`query_term`].
#[derive(Debug, Error)]
pub enum QueryTermError {
    #[error("invalid regex pattern: {0}")]
    InvalidRegex(#[from] regex::Error),
    /// Round 468 — an unknown field name in the filter is a caller error,
    /// not an empty result. Before this, a typo'd `--field` silently
    /// returned 0 hits, indistinguishable from "term absent" — the same
    /// silent-miss class the R467 identifier-key repair closed for IDs.
    #[error("unknown field `{field}` for scope {scope}; valid fields: {valid}")]
    UnknownField {
        field: String,
        scope: &'static str,
        valid: String,
    },
}

/// Field-name rosters per entity kind — the validation vocabulary for
/// [`TermQuery::field_filter`]. Single-sourced with the scanners by a test
/// PAIR, one per direction: `query_term_field_roster_matches_scanners`
/// (every roster name produces a hit — roster ⊆ scanned) and
/// `query_term_every_scanned_field_is_in_its_roster` (every emitted field
/// path's base name is in its roster — scanned ⊆ roster). Either drift
/// direction fails CI.
const SECTION_FIELDS: &[&str] = &[
    "section_id",
    "title",
    "intent",
    "rationale_bullets",
    "inputs_bullets",
    "outputs_bullets",
    "caveats_bullets",
    "impact_scope",
    "alternatives_rejected",
    "examples",
    "bindings",
];
const CHANGELOG_FIELDS: &[&str] = &[
    "entry_id",
    "decision_summary",
    "changes_bullets",
    "verification_bullets",
    "impact_refs",
    "carry_forward_bullets",
];
const INVENTORY_FIELDS: &[&str] = &["inventory_id", "source", "reason"];

/// Validate a field filter against the kinds the scope will scan. A field
/// name outside the scanned kinds' rosters can never produce a hit — that
/// is a caller error surfaced loudly, never an empty result.
fn validate_field_filter(
    scope: TermScope,
    filter: &BTreeSet<String>,
) -> Result<(), QueryTermError> {
    let (rosters, scope_label): (&[&[&str]], &'static str) = match scope {
        TermScope::All => (&[SECTION_FIELDS, CHANGELOG_FIELDS, INVENTORY_FIELDS], "all"),
        TermScope::Sections => (&[SECTION_FIELDS], "sections"),
        TermScope::ChangelogEntries => (&[CHANGELOG_FIELDS], "changelog"),
        TermScope::Inventory => (&[INVENTORY_FIELDS], "inventory"),
    };
    for field in filter {
        let known = rosters.iter().any(|r| r.contains(&field.as_str()));
        if !known {
            let mut valid: Vec<&str> = rosters.iter().flat_map(|r| r.iter().copied()).collect();
            valid.sort_unstable();
            return Err(QueryTermError::UnknownField {
                field: field.clone(),
                scope: scope_label,
                valid: valid.join(", "),
            });
        }
    }
    Ok(())
}

/// `query_term` — literal/regex scan over the atomic store.
///
/// Returns hits in deterministic order: target_kind variant order ×
/// `BTreeMap` key order × field scan order (identifier key first, then
/// declared fields in declaration order) × bullet index order.
/// Pure read — `store` is not modified.
pub fn query_term(store: &AtomicStore, q: &TermQuery) -> Result<Vec<TermHit>, QueryTermError> {
    if let Some(filter) = &q.field_filter {
        validate_field_filter(q.scope, filter)?;
    }
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
    // Round 467 — identifier key scanned like any text field (kind-qualified
    // field name so a shared filter set stays unambiguous across scopes).
    if field_allowed(filter, "section_id") {
        push_simple_hit(
            TermTargetKind::Section,
            section_id,
            "section_id".to_string(),
            section_id,
            m,
            out,
        );
    }
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
    // Round 467 — entry_id (`Round <n> — …`) scanned; a round-number search
    // must find its entry rather than silently missing.
    if field_allowed(filter, "entry_id") {
        push_simple_hit(
            TermTargetKind::ChangelogEntry,
            entry_id,
            "entry_id".to_string(),
            entry_id,
            m,
            out,
        );
    }
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
    // Round 467 — identifier key scanned, symmetric with the other kinds.
    if field_allowed(filter, "inventory_id") {
        push_simple_hit(
            TermTargetKind::Inventory,
            inv_id,
            "inventory_id".to_string(),
            inv_id,
            m,
            out,
        );
    }
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
    fn section_by_id_projects_bindings() {
        let section = AtomicSection {
            skeleton: mnemosyne_core::SectionSkeleton {
                title: "Citation defense".into(),
                parent_doc: "spec".into(),
                ..Default::default()
            },
            bindings: vec![
                Binding {
                    kind: BindingKind::Implements,
                    file: "crates/mnemosyne-validate/src/code_refs.rs".to_string(),
                    symbol: None,
                },
                Binding {
                    kind: BindingKind::References,
                    file: "crates/mnemosyne-cli/src/main.rs".to_string(),
                    symbol: Some("fn cmd_validate_code_refs".to_string()),
                },
            ],
            ..Default::default()
        };
        let store = store_with_one_section("code-citation-defense", section);
        let view = section_by_id(&store, "code-citation-defense").expect("section exists");
        assert_eq!(view.bindings.len(), 2);
        assert_eq!(view.bindings[0].kind, BindingKind::Implements);
        assert_eq!(
            view.bindings[0].file,
            "crates/mnemosyne-validate/src/code_refs.rs"
        );
        assert_eq!(view.bindings[1].kind, BindingKind::References);
        assert_eq!(
            view.bindings[1].symbol.as_deref(),
            Some("fn cmd_validate_code_refs")
        );
    }

    #[test]
    fn section_by_id_omits_bindings_when_absent() {
        // Prose-only section: empty bindings → field skipped in JSON so the
        // read surface is unchanged for stores that do not use Path B.
        let mut store = AtomicStore::default();
        seed_section(&mut store, "39", "Graph schema", "tracks graph schema");
        let view = section_by_id(&store, "39").expect("section 39 exists");
        assert!(view.bindings.is_empty());
        let json = serde_json::to_string(&view).expect("serialize");
        assert!(
            !json.contains("bindings"),
            "empty bindings must be omitted from JSON: {json}"
        );
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
    fn list_changelog_returns_all_entries_in_round_number_order() {
        let mut store = AtomicStore::default();
        for id in ["Round 2", "Round 10", "Round 1"] {
            store.changelog_entries.insert(
                id.into(),
                AtomicChangelogEntry {
                    decision_summary: Some(format!("entry {id}")),
                    ..Default::default()
                },
            );
        }
        let view = list_changelog(&store, None);
        let ids: Vec<&str> = view.entries.iter().map(|v| v.entry_id.as_str()).collect();
        // round-number order, NOT lexicographic: a two-digit round key sorts
        // before a one-digit one as a string, but parses to a larger number.
        assert_eq!(ids, ["Round 1", "Round 2", "Round 10"]);
        assert_eq!(view.total, 3);
    }

    #[test]
    fn list_changelog_limit_keeps_newest_with_honest_total() {
        let mut store = AtomicStore::default();
        for id in ["Round 2", "Round 10", "Round 1"] {
            store
                .changelog_entries
                .insert(id.into(), AtomicChangelogEntry::default());
        }
        let view = list_changelog(&store, Some(2));
        let ids: Vec<&str> = view.entries.iter().map(|v| v.entry_id.as_str()).collect();
        assert_eq!(ids, ["Round 2", "Round 10"], "last n = newest, order kept");
        assert_eq!(
            view.total, 3,
            "total reports the full ledger, not the slice"
        );
        let all = list_changelog(&store, Some(99));
        assert_eq!(all.entries.len(), 3, "limit beyond len = whole ledger");
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

    // --- Round 467: identifier keys are searched fields ---

    #[test]
    fn query_term_matches_changelog_entry_id() {
        let entry = AtomicChangelogEntry {
            decision_summary: Some("nothing relevant".to_string()),
            ..Default::default()
        };
        let store = store_with_one_entry("Round 466 — playthrough manuscript", entry);
        let hits = query_term(&store, &literal_q("Round 466")).expect("ok");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].target_kind, TermTargetKind::ChangelogEntry);
        assert_eq!(hits[0].field_path, "entry_id");
        assert_eq!(hits[0].line_context, "Round 466 — playthrough manuscript");
    }

    #[test]
    fn round_order_key_reads_alpha_suffix_as_base26_column() {
        // The cross-round bug this closes: lexicographic ties put 311aa < 311lq
        // < 311z, but creation order is 311z (col 26) < 311aa (27) < 311lq (329).
        assert!(round_order_key("Round 311z") < round_order_key("Round 311aa"));
        assert!(round_order_key("Round 311aa") < round_order_key("Round 311lq"));
        // empty suffix precedes the first column.
        assert!(round_order_key("Round 311") < round_order_key("Round 311a"));
        // an enumerated sub-step of `a` precedes the `aa` column.
        assert!(round_order_key("Round 311a1") < round_order_key("Round 311aa"));
        // a `-pre` sub-step stays grouped with its column, before the next one.
        assert!(round_order_key("Round 311az-pre") < round_order_key("Round 311ba"));
        // a non-`Round <n>` key (the stray `test`) sorts last.
        assert!(round_order_key("Round 999zz") < round_order_key("test"));
    }

    #[test]
    fn list_changelog_orders_alpha_suffix_by_creation_not_lexicographic() {
        // The BTreeMap iterates keys lexicographically, which is NOT creation
        // order for the base-26 alpha suffix. list_changelog must reflect
        // creation order so a session-load `--limit N` read picks the
        // genuinely-newest entries.
        let mut store = AtomicStore::default();
        for id in [
            "Round 311aa",
            "Round 311z",
            "Round 311lq",
            "Round 311",
            "Round 47",
            "test",
            "Round 311a",
        ] {
            store
                .changelog_entries
                .insert(id.to_string(), AtomicChangelogEntry::default());
        }

        let view = list_changelog(&store, None);
        let order: Vec<&str> = view.entries.iter().map(|e| e.entry_id.as_str()).collect();
        assert_eq!(
            order,
            vec![
                "Round 47",
                "Round 311",
                "Round 311a",
                "Round 311z",
                "Round 311aa",
                "Round 311lq",
                "test",
            ],
        );

        // `--limit` keeps the newest N by creation order. The pre-fix bug
        // returned the lexicographic tail `["Round 311z", "test"]`.
        let limited = list_changelog(&store, Some(2));
        let newest: Vec<&str> = limited
            .entries
            .iter()
            .map(|e| e.entry_id.as_str())
            .collect();
        assert_eq!(newest, vec!["Round 311lq", "test"]);
        assert_eq!(
            limited.total, 7,
            "total reports the full ledger, not the limited slice"
        );
    }

    #[test]
    fn query_term_matches_section_id() {
        let store = store_with_one_section("orphan-ledger", AtomicSection::default());
        let hits = query_term(&store, &literal_q("orphan-ledger")).expect("ok");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].target_kind, TermTargetKind::Section);
        assert_eq!(hits[0].field_path, "section_id");
    }

    #[test]
    fn query_term_matches_inventory_id() {
        let mut store = AtomicStore::default();
        store
            .inventory_entries
            .insert("INV-7".to_string(), InventoryEntry::default());
        let hits = query_term(&store, &literal_q("INV-7")).expect("ok");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].target_kind, TermTargetKind::Inventory);
        assert_eq!(hits[0].field_path, "inventory_id");
    }

    #[test]
    fn query_term_field_filter_can_exclude_identifier_keys() {
        let section = AtomicSection {
            intent: Some("mentions sec-x here".to_string()),
            ..Default::default()
        };
        let store = store_with_one_section("sec-x", section);
        let mut filter = BTreeSet::new();
        filter.insert("intent".to_string());
        let q = TermQuery {
            field_filter: Some(filter),
            ..literal_q("sec-x")
        };
        let hits = query_term(&store, &q).expect("ok");
        assert_eq!(hits.len(), 1, "filter restricts to intent only");
        assert_eq!(hits[0].field_path, "intent");
    }

    // --- Round 468: field-filter names fail loud, single-sourced rosters ---

    #[test]
    fn query_term_unknown_field_rejects() {
        let store = AtomicStore::default();
        let mut filter = BTreeSet::new();
        filter.insert("intnet".to_string());
        let q = TermQuery {
            field_filter: Some(filter),
            ..literal_q("x")
        };
        match query_term(&store, &q) {
            Err(QueryTermError::UnknownField { field, scope, .. }) => {
                assert_eq!(field, "intnet");
                assert_eq!(scope, "all");
            }
            other => panic!("expected UnknownField, got {:?}", other),
        }
    }

    #[test]
    fn query_term_field_outside_scope_rejects() {
        // `intent` is a Section field; under changelog scope it can never
        // hit, so it is a caller error, not an empty result.
        let store = AtomicStore::default();
        let mut filter = BTreeSet::new();
        filter.insert("intent".to_string());
        let q = TermQuery {
            scope: TermScope::ChangelogEntries,
            field_filter: Some(filter),
            ..literal_q("x")
        };
        match query_term(&store, &q) {
            Err(QueryTermError::UnknownField {
                field,
                scope,
                valid,
            }) => {
                assert_eq!(field, "intent");
                assert_eq!(scope, "changelog");
                assert!(valid.contains("entry_id"), "error lists valid names");
            }
            other => panic!("expected UnknownField, got {:?}", other),
        }
    }

    /// Every text field of every kind populated, every value containing the
    /// `zz` marker — the fixture for the two roster<->scanner direction pins.
    fn fully_populated_store() -> AtomicStore {
        let mut store = AtomicStore::default();
        store.sections.insert(
            "zz-sec".to_string(),
            AtomicSection {
                skeleton: mnemosyne_core::SectionSkeleton {
                    title: "zz title".into(),
                    parent_doc: "spec".into(),
                    ..Default::default()
                },
                intent: Some("zz intent".to_string()),
                rationale_bullets: vec!["zz r".to_string()],
                inputs_bullets: vec!["zz i".to_string()],
                outputs_bullets: vec!["zz o".to_string()],
                caveats_bullets: vec!["zz c".to_string()],
                impact_scope: vec!["zz-target".to_string()],
                alternatives_rejected: vec![RejectedAlternative {
                    alternative: "zz alt".to_string(),
                    reason: "zz why".to_string(),
                }],
                examples: vec![ExampleBlock {
                    language: "rust".to_string(),
                    code: "zz code".to_string(),
                }],
                bindings: vec![Binding {
                    kind: BindingKind::Implements,
                    file: "zz/file.rs".to_string(),
                    symbol: Some("zz_sym".to_string()),
                }],
                ..Default::default()
            },
        );
        store.changelog_entries.insert(
            "Round 1 zz".to_string(),
            AtomicChangelogEntry {
                decision_summary: Some("zz summary".to_string()),
                changes_bullets: vec!["zz ch".to_string()],
                verification_bullets: vec!["zz v".to_string()],
                impact_refs: vec!["zz-ref".to_string()],
                carry_forward_bullets: vec!["zz carry".to_string()],
                ..Default::default()
            },
        );
        store.inventory_entries.insert(
            "zz-inv".to_string(),
            InventoryEntry {
                source: Some("zz src".to_string()),
                reason: Some("zz reason".to_string()),
                ..Default::default()
            },
        );
        store
    }

    const ROSTER_CASES: &[(TermScope, &[&str])] = &[
        (TermScope::Sections, SECTION_FIELDS),
        (TermScope::ChangelogEntries, CHANGELOG_FIELDS),
        (TermScope::Inventory, INVENTORY_FIELDS),
    ];

    #[test]
    fn query_term_field_roster_matches_scanners() {
        // Direction 1 (roster ⊆ scanned): every roster name, run as a filter
        // against the fully-populated store, must produce a hit in that very
        // field — a roster name no scanner checks fails here.
        let store = fully_populated_store();
        for (scope, roster) in ROSTER_CASES {
            for field in *roster {
                let mut filter = BTreeSet::new();
                filter.insert(field.to_string());
                let q = TermQuery {
                    scope: *scope,
                    field_filter: Some(filter),
                    ..literal_q("zz")
                };
                let hits = query_term(&store, &q).expect("roster name validates");
                assert!(
                    !hits.is_empty(),
                    "roster field `{}` produced no hit — scanner/roster drift",
                    field
                );
                for h in &hits {
                    assert!(
                        h.field_path.starts_with(field),
                        "hit field_path `{}` outside filtered field `{}`",
                        h.field_path,
                        field
                    );
                }
            }
        }
    }

    #[test]
    fn query_term_every_scanned_field_is_in_its_roster() {
        // Direction 2 (scanned ⊆ roster): an unfiltered scan of the
        // fully-populated store may only emit field paths whose base name the
        // kind's roster knows — a scanner field missing from its roster fails
        // here (R470; the R468 comment had claimed this direction untested).
        let store = fully_populated_store();
        for (scope, roster) in ROSTER_CASES {
            let q = TermQuery {
                scope: *scope,
                ..literal_q("zz")
            };
            let hits = query_term(&store, &q).expect("ok");
            assert!(!hits.is_empty());
            for h in &hits {
                let base = h
                    .field_path
                    .split('[')
                    .next()
                    .expect("split yields at least one part");
                assert!(
                    roster.contains(&base),
                    "scanned field `{}` (base `{}`) missing from its roster",
                    h.field_path,
                    base
                );
            }
        }
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
