//! Atomic typed fields store — Section / ChangelogEntry atomic
//! decomposition.
//!
//! Spec binding: §atomic-store-mutate-api, §code-citation-defense/bidirectional-binding.
//!
//! Phase 0e wired the *input axis* (markdown → typed facts via
//! generic loader). Phase 0f wires the *output axis*: atomic typed fields →
//! template render → MD bytes. The atomic store is the new authoritative
//! source for new content; legacy `body` /
//! `sub_bullets` field is carried stable on existing entries (frozen
//! ledger-166 migration multi-session scope).
//!
//! Storage: sidecar JSON file (default `docs/.atomic/workspace.atomic.json`),
//! workspace-wide single store keyed by `section_id` / `entry_id`.
//! Persistence is atomic write (temp + rename) following the same pattern as
//! the markdown mutate primitives.
//!
//! API surface:
//! - Section atomic: `set_section_intent` / `set_section_rationale` /
//! `set_section_inputs` / `set_section_outputs` / `add_section_caveat` /
//! `set_section_alternatives` / `set_section_impact_scope` /
//! `add_section_example` / `add_section_binding`
//! - ChangelogEntry atomic: `append_changelog_entry`
//!
//! `Section.bindings` lands as the substrate for Path B
//! of the code-citation defense (Spec ↔ Code bidirectional binding). The
//! atomic store records "this section is implemented at file:symbol";
//! cross-checks code citations against the spec's authoritative
//! binding (set-equality, the OPTION D pattern lifted from cross-
//! ref orphan reject). Schema + mutate primitive only — validator
//! extension and section seeding are deferred to later rounds.

pub mod project;
pub mod redact;
pub use project::{section_entity_id, MAIN_BRANCH_ID, SECTION_VALID_FROM};
pub use redact::*;

use mnemosyne_core::{
    sha256_hex, strip_section_marker, Branch, BranchFork, ConflictAssertion, DecisionStatus,
    DisclosureMode, DisclosureOverride, DisclosurePlan, DisclosureSurface, Entity, Frame,
    InventoryStatus, NarrativeFact, PayoffExpectation, Predicate, PredicateObjectKind, TypedClaim,
    TypedObject,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Section atomic typed fields.
///
/// Default = all empty / None. legacy `body` field (Section.body via parser
/// `bodies` map carries stable — atomic fields are additive only.
///
/// Round 287 — outline lift (title-from-workspace-pending carry closure). `title` / `parent_doc` /
/// `parent_section` 3 fields added so AtomicSection mirrors schema.rs::Section's
/// closed-form 5-field shape (`section_id` is the AtomicStore.sections map key).
/// Pre-Round 287 sections deserialize with empty `title` / `parent_doc` and
/// `parent_section = None` via `#[serde(default)]`; Phase I backfill migration
/// populates them from workspace markdown-derived Section data. Post-migration
/// invariant: every AtomicSection has non-empty `title` + non-empty `parent_doc`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtomicSection {
    /// Layer-0 canonical scalar skeleton (Round 325; scoped to scalars in
    /// Round 326): the medium-neutral attributes (`title` / `parent_doc` /
    /// `parent_section` / `decision_status`) lifted into `mnemosyne-core`.
    /// `#[serde(flatten)]` keeps the skeleton fields inline in the JSON, so
    /// the on-disk authoring shape is byte-identical to the pre-split layout.
    /// Placed first so flattened skeleton fields serialize ahead of the
    /// design_doc content below, matching the historical field order.
    #[serde(flatten)]
    pub skeleton: mnemosyne_core::SectionSkeleton,
    /// 1-3 sentence summary. T3 style threshold: ≤ 200 char.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
    /// Preserved decision list. T3 style threshold: each bullet ≤ 100 chars.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rationale_bullets: Vec<String>,
    /// input list.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs_bullets: Vec<String>,
    /// output list.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outputs_bullets: Vec<String>,
    /// threshold list. T3 style threshold: each bullet ≤ 100 char.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub caveats_bullets: Vec<String>,
    /// rejected option + reason pairs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternatives_rejected: Vec<RejectedAlternative>,
    /// Outbound cross-ref list (target section_id without the `§` prefix).
    /// Adapter-local (Round 326): cross-refs are *not* part of the shared
    /// Layer-0 skeleton because the index represents them as `CrossRefFact`
    /// relations, not an inline array. The JSON log adapter stores them inline
    /// here; index projection (convergence B) reads this and emits CrossRefFacts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub impact_scope: Vec<String>,
    /// Supersession forward-pointer — the section_id (without the `§` prefix)
    /// of the decision that replaced this one. `Some` iff
    /// `decision_status == Superseded`; the pairing is enforced by the single
    /// write path [`set_section_decision_status`] (R342). Adapter-local like
    /// [`Self::impact_scope`]: cross-refs stay out of the Layer-0 skeleton, so
    /// this lives on the JSON log adapter and is projected to a
    /// `decision`-kind `CrossRefFact` at index build (`project_cross_ref_facts`).
    /// Storing it structurally is what lets the warm read-side projection (R339)
    /// see the supersession relation from the store instead of re-parsed markdown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub superseded_by: Option<String>,
    /// Resolution forward-pointer — the section_id (without `§`) of the section
    /// expected to RESOLVE this open question. `Some` only when
    /// `decision_status == Open`, and OPTIONAL there (an open question may not
    /// yet know its resolver). Symmetric with [`Self::superseded_by`]: each
    /// lifecycle forward-pointer is its own field with its own invariant (no
    /// general relation bag — keeps invariant enforcement un-shared per the
    /// CLAUDE.md half-enforced-invariant guard). Set by the single write path
    /// [`set_section_decision_status`]; projected to a `resolved_by`-kind
    /// `CrossRefFact` (orphan-checked like supersession). The structured-fact
    /// SSOT home for "deferred to §Y" prose
    /// (claudedocs/structured-fact-ssot-design.md sec 12a / sec 6).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_by: Option<String>,
    /// code/config block list. T3 style threshold: code block itself exempt.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub examples: Vec<ExampleBlock>,
    /// Path B (Spec ↔ Code bidirectional binding) substrate.
    /// Set of `(file, symbol?, kind)` typed trace-link edges. The
    /// bidirectional cross-check defends code citations against this set
    /// (presence, any kind); coverage counts only `kind == Implements`.
    /// Duplicate `(file, symbol)` rejected at write time (set semantics on
    /// the identity pair; `kind` is a mutable attribute via
    /// [`set_section_binding_kind`], not part of the identity).
    ///
    /// `#[serde(alias = "implementations")]` is the v4→v5 migration reader:
    /// a pre-v5 store's JSON key was `implementations`, and this alias maps
    /// it onto `bindings` at load; the next save rewrites the key. This is
    /// migration substrate (permitted by no-legacy-carry, which bans
    /// *compat* carries, not migration code), and it is **load-bearing for
    /// data safety** — without it a v4 store's entire binding set would
    /// deserialize empty (the field `#[serde(default)]`s to `[]`) and the
    /// next save would erase it. It is the sole guard because `AtomicSection`
    /// uses `#[serde(flatten)]`, which is incompatible with
    /// `deny_unknown_fields`; the happy-path migration test pins that the
    /// alias fires. Keep until a v4 store cannot exist anywhere (not provable
    /// while external consumers hold old stores), then drop with a tracked
    /// round — do not remove on aesthetic grounds.
    #[serde(
        default,
        alias = "implementations",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub bindings: Vec<Binding>,

    /// Coverage applicability (Path B). `Normative` (default) keeps the
    /// Round 269 coverage axiom — a non-`Removed` `Normative` section with
    /// zero `implements` bindings is a coverage gap. `Informative` exempts
    /// this section: it is prose-only (terminology / overview / references)
    /// with nothing to implement here. Adapter-local beside `bindings` rather
    /// than in the L0 skeleton because coverage applicability is not
    /// medium-neutral. Skipped from JSON when `Normative` so an unclassified
    /// store is byte-identical on disk and renders only the `Informative`
    /// deviation.
    #[serde(default, skip_serializing_if = "is_normative_coverage")]
    pub coverage_expectation: mnemosyne_core::CoverageExpectation,

    /// Verification class (R413) — which kind of evidence a `Normative` section
    /// expects, orthogonal to `coverage_expectation`. Adapter-local beside
    /// `bindings`. Skipped from JSON when `Dedicated` (the default) so an
    /// unclassified store is byte-identical and only the `ByConstruction`
    /// deviation is persisted. Consulted by the `VerificationMissing` gate only
    /// when `coverage_expectation == Normative` and the verify axis is enabled
    /// (`[plugins.set_equality_validator].severity_verification` set).
    #[serde(default, skip_serializing_if = "is_dedicated_verification")]
    pub verification_expectation: mnemosyne_core::VerificationExpectation,

    /// External-spec mirror — vendored normative quote anchored to this
    /// Section (RFC-002 FR-1). When `Some`, the Section represents a
    /// section of an external standard (W3C / IETF RFC / IEEE / AUTOSAR /
    /// …) mirrored into this workspace; the embedded text + anchor URL +
    /// source revision pin let reviewers verify code citations against
    /// the exact spec text the workspace was built against.
    ///
    /// **EPUB-projected cache (R403)**: `text` is a derived cache of the
    /// committed EPUB, not a frozen authored value — it may be overwritten by
    /// re-projecting from a fresh EPUB extraction. The authored identity fields
    /// (`anchor_url`, `source_revision`) pin which upstream section + revision
    /// the excerpt belongs to; they are store-side metadata, not EPUB content,
    /// so [`import_epub_excerpts`] preserves them and refreshes only `text` +
    /// `text_sha256`. Spec revision drift across a *different* revision is still
    /// modeled as `Section.decision_status = Superseded` + a new Section (R265),
    /// keeping the audit trail honest about which rev each excerpt mirrors.
    ///
    /// `None` (default) on every Section that does not mirror an external
    /// standard. The `[workspace.spec_source]` config (FR-2) names the
    /// upstream the entire workspace tracks; per-Section `source_revision`
    /// is the rev that was current when this Section's excerpt was
    /// anchored, so partially-migrated workspaces can carry old + new
    /// rev side-by-side via `Active` + `Superseded` Sections.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normative_excerpt: Option<NormativeExcerpt>,

    /// EPUB-SSOT locator (R393) — where this Section lives in the workspace's
    /// normalized EPUB. Set by `import-epub-anchors` from the medium-forge
    /// `epub-anchor-map/v1` output. Adapter-local (EPUB-specific, not L0). A
    /// derived, mutable pointer; `None` when no EPUB is mirrored. Not rendered
    /// to GENERATED.md (machine pointer, not human content) so round-trip is
    /// unaffected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub epub_locator: Option<EpubLocator>,
}

/// serde `skip_serializing_if` predicate for [`AtomicSection::coverage_expectation`]:
/// the default `Normative` is omitted from the JSON log so unclassified stores
/// stay byte-identical and only the `Informative` deviation is persisted.
fn is_normative_coverage(c: &mnemosyne_core::CoverageExpectation) -> bool {
    matches!(c, mnemosyne_core::CoverageExpectation::Normative)
}

/// serde `skip_serializing_if` predicate for [`AtomicSection::verification_expectation`]:
/// the default `Dedicated` is omitted from the JSON log so unclassified stores
/// stay byte-identical and only the `ByConstruction` deviation is persisted.
fn is_dedicated_verification(v: &mnemosyne_core::VerificationExpectation) -> bool {
    matches!(v, mnemosyne_core::VerificationExpectation::Dedicated)
}

/// Vendored quote from an external normative source — embedded into an
/// [`AtomicSection`] so reviewers can verify code citations without
/// fetching the upstream HTML.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NormativeExcerpt {
    /// The normative text as it appeared at `source_revision`. Preserved
    /// verbatim — leading/trailing whitespace and newline-only edits are
    /// the only sanitization (mutate primitive trims trailing newline
    /// to keep round-trip render stable).
    pub text: String,
    /// Direct anchor URL (e.g. `https://www.w3.org/TR/scxml/#event` for
    /// a W3C SCXML section like 3.13). The mutate primitive validates that the string
    /// parses as an absolute URL (scheme + host) — anchor fragment
    /// optional, query string allowed.
    pub anchor_url: String,
    /// Revision identifier from the upstream spec the excerpt was
    /// captured at. Free-form string (Recommendation publication date,
    /// editor's-draft date, RFC number + revision letter, etc.) — the
    /// workspace's `[workspace.spec_source].revision` is the *current*
    /// rev; this field is the rev *this specific Section* was anchored
    /// at, so partially-migrated workspaces stay coherent under spec
    /// rev bumps.
    pub source_revision: String,
    /// SHA-256 (hex) of `text` as emitted by the EPUB extractor
    /// (`medium-forge` `epub-anchor-map/v2`). `text` is a *derived cache* of
    /// the committed EPUB; this hash lets `scan_content_drift` re-hash the
    /// cached string offline and detect drift without re-extracting. Empty
    /// = unrevalidatable (hand-authored or pre-v8 excerpt not yet imported
    /// from an EPUB); surfaced by `report-excerpt-hash-backfill`. Schema v8.
    #[serde(default)]
    pub text_sha256: String,
}

impl NormativeExcerpt {
    /// SHA-256 (hex) recomputed from the stored `text` — the value
    /// `text_sha256` is expected to equal. The offline revalidation anchor
    /// (R404): the mutate API guarantees this equality at write time, so a
    /// later divergence means the cache was edited out-of-band.
    pub fn recompute_text_sha256(&self) -> String {
        sha256_hex(self.text.as_bytes())
    }

    /// Whether the declared `text_sha256` still matches the stored `text`.
    /// `None` when the hash is empty (unrevalidatable — never imported from an
    /// EPUB; owned by `report-excerpt-hash-backfill`, not treated as drift).
    /// `Some(false)` is a content-integrity failure (`scan_content_drift`).
    pub fn text_sha256_matches(&self) -> Option<bool> {
        if self.text_sha256.is_empty() {
            None
        } else {
            Some(self.recompute_text_sha256() == self.text_sha256)
        }
    }
}

/// EPUB-SSOT locator (R393) — where this Section lives inside the workspace's
/// normalized EPUB (produced by the `medium-forge` HTML backend, contract
/// `epub-anchor-map/v1`). The EPUB is the content SSOT; this pointer lets a
/// reader/viewer resolve the Section's position to overlay facts on the
/// rendered spec. Mutable (unlike the frozen `normative_excerpt`): re-importing
/// an updated anchor map overwrites it, since it is a derived pointer, not an
/// authored audit value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpubLocator {
    /// EPUB spine document holding this Section (e.g. `OEBPS/spec.xhtml`).
    pub spine_href: String,
    /// Element `id` within that document — equals the `section_id`, the join
    /// key the medium-forge backend stamps onto the Section's container.
    pub fragment: String,
    /// Canonical Fragment Identifier for precise location. Optional: the
    /// fragment alone resolves the Section; CFI is for sub-element precision.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cfi: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RejectedAlternative {
    pub alternative: String,
    pub reason: String,
}

impl RejectedAlternative {
    /// Parse one `<alternative> -- <reason>` line (or the ` — ` em-dash
    /// separator). A leading `- ` bullet marker and surrounding whitespace are
    /// trimmed. Returns `None` when no recognized separator is present — the
    /// caller supplies the contextual error (bullet index vs file line number).
    ///
    /// Sole parser shared by the CLI `--alternatives-file` and MCP bullet
    /// surfaces (Round 358 DRY).
    pub fn parse_line(raw: &str) -> Option<RejectedAlternative> {
        let trimmed = raw.trim();
        let stripped = trimmed.strip_prefix("- ").unwrap_or(trimmed);
        let (alt, reason) = stripped
            .split_once(" — ")
            .or_else(|| stripped.split_once(" -- "))?;
        Some(RejectedAlternative {
            alternative: alt.trim().to_string(),
            reason: reason.trim().to_string(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExampleBlock {
    /// Language tag for fenced code block (`rust` / `toml` / `markdown` / etc).
    pub language: String,
    pub code: String,
}

// Trace-link claim strength on a Path B binding. Canonical enum lives in
// L0 core (mirrors DecisionStatus); re-exported here so the on-disk
// `Binding.kind` field and the mutate primitives share one type with the
// validator/view layer — no adapter, no duplicate enum (R309 pattern).
pub use mnemosyne_core::BindingKind;
pub use mnemosyne_core::CoverageExpectation;
pub use mnemosyne_core::VerificationExpectation;

/// serde default for [`Binding::kind`]: pre-v5 stores have no `kind` field;
/// every legacy binding was an implicit implementation claim (coverage
/// counted all bindings before the split), so defaulting to `Implements`
/// is behavior-preserving. The defaulted bindings are surfaced (not
/// silently blessed) by [`AtomicStore::kind_migration_report`], which the
/// CLI `report-binding-migration` verb prints; it is readable only while
/// `schema_version < 5` (before the first save bumps the version), so an
/// operator upgrading a v4 store runs that verb before/around the upgrade.
fn binding_kind_implements_default() -> BindingKind {
    BindingKind::Implements
}

/// Path B binding entry (Spec → Code), a typed trace-link edge.
///
/// `file` = workspace-relative POSIX path (no leading `/`, no `..` segment,
/// no backslash; validated at write time by [`add_section_binding`]).
/// `symbol` = optional opaque language-agnostic identifier (function /
/// type / qualified path); when present, narrows the binding from "this
/// file" to "this symbol within this file". Stored opaquely — the spec
/// layer does not encode language grammar; the bidirectional cross-check
/// operates on the strings as-is.
/// `kind` = the trace-link claim strength ([`BindingKind`]); always written
/// explicitly. `#[serde(default)]` only so pre-v5 stores (which have no
/// `kind`) still deserialize during the load migration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Binding {
    pub file: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(default = "binding_kind_implements_default")]
    pub kind: BindingKind,
}

/// One row of the v4→v5 [`AtomicStore::kind_migration_report`]: a binding
/// that inherited `kind = Implements` from a pre-v5 store, pending Stage-B
/// classification (`implements` vs `references`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KindMigrationRow {
    pub section_id: String,
    pub file: String,
    pub symbol: Option<String>,
    pub defaulted_kind: BindingKind,
}

/// The v4→v5 migration work-list (see [`AtomicStore::kind_migration_report`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KindMigrationReport {
    /// On-disk schema version the store was loaded from (< 5).
    pub from_schema_version: u32,
    pub rows: Vec<KindMigrationRow>,
}

/// One Section whose `normative_excerpt.text_sha256` is empty — its `text`
/// is not yet revalidatable against an EPUB (R402, v7→v8 backfill).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExcerptHashBackfillRow {
    pub section_id: String,
    pub source_revision: String,
}

/// The v7→v8 excerpt-hash backfill work-list
/// (see [`AtomicStore::excerpt_hash_backfill_report`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExcerptHashBackfillReport {
    pub rows: Vec<ExcerptHashBackfillRow>,
}

/// ChangelogEntry atomic typed fields.
///
/// Round 294 — schema_version 4 splits the body into two parallel layers:
///
/// - **audit_*** fields: frozen after first commit (T2 jaccard scope). The
///   permanent record. Mutate API never modifies these post-append; the
///   primitive boundary rejects any attempt.
/// - **publishable_*** fields: mutable view layer. Default = audit clone at
///   append time. R295 introduces publishable setters; R296 introduces
///   `[[publishable_override_ledger]]` so that publishable_* != audit_*
///   transitions require an explicit reason and content_hash anchor.
///   read projections surface publishable_*. CQRS / read-write split pattern.
///
/// Migration: v3 → v4 loader (`AtomicStore::load`) clones audit_* into
/// publishable_* per entry; v4 stores keep them independent (intended
/// divergence for redaction / typo fix without losing the audit record).
///
/// Backward compat: `decision_summary` / `changes_bullets` /
/// `verification_bullets` / `impact_refs` / `carry_forward_bullets` are kept
/// as the **audit** half (no rename → existing JSON loads unchanged); the
/// publishable half is opt-in via the new `publishable_*` keys.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtomicChangelogEntry {
    /// Audit half — 1 sentence headline. Frozen after first commit (T2 scope).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changes_bullets: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verification_bullets: Vec<String>,
    /// cross-ref list (target section_id without `§` prefix). Audit half.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub impact_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub carry_forward_bullets: Vec<String>,

    /// Round 294 — publishable half. Mutable view layer; default = audit
    /// clone at append time. T2 jaccard does NOT compare these. R295
    /// introduces publishable setters; R296 wires
    /// `[[publishable_override_ledger]]` so divergence from the audit half
    /// requires an explicit reason and content_hash anchor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publishable_decision_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub publishable_changes_bullets: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub publishable_verification_bullets: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub publishable_impact_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub publishable_carry_forward_bullets: Vec<String>,
}

impl AtomicChangelogEntry {
    /// Round 294 — clone the audit half into the publishable half. Used by
    /// the v3→v4 loader migration (so existing entries get default-equal
    /// publishable views) and by `append_changelog_entry` (so newly
    /// authored entries default to audit-equal publishable shape).
    ///
    /// Idempotent: calling twice produces the same result. Safe to call when
    /// publishable fields are already non-empty — the audit half always
    /// wins (audit is the source of truth at append time; later setters
    /// diverge the publishable half deliberately).
    pub fn clone_audit_into_publishable(&mut self) {
        self.publishable_decision_summary = self.decision_summary.clone();
        self.publishable_changes_bullets = self.changes_bullets.clone();
        self.publishable_verification_bullets = self.verification_bullets.clone();
        self.publishable_impact_refs = self.impact_refs.clone();
        self.publishable_carry_forward_bullets = self.carry_forward_bullets.clone();
    }

    /// Round 294 — true when publishable_* matches audit_* across all 5
    /// fields. Used by validate-workspace (R296) to detect intentional
    /// divergences and require a `[[publishable_override_ledger]]` entry.
    pub fn publishable_matches_audit(&self) -> bool {
        self.publishable_decision_summary == self.decision_summary
            && self.publishable_changes_bullets == self.changes_bullets
            && self.publishable_verification_bullets == self.verification_bullets
            && self.publishable_impact_refs == self.impact_refs
            && self.publishable_carry_forward_bullets == self.carry_forward_bullets
    }

    /// Round 300 — enumerate publishable_* fields that diverge from their
    /// audit_* counterpart, in `format_ledger_row` order so emitted ledger
    /// drafts read deterministically. Returns empty Vec when in sync.
    pub fn divergent_publishable_fields(&self) -> Vec<&'static str> {
        let mut out = Vec::with_capacity(5);
        if self.publishable_decision_summary != self.decision_summary {
            out.push("publishable_decision_summary");
        }
        if self.publishable_changes_bullets != self.changes_bullets {
            out.push("publishable_changes_bullets");
        }
        if self.publishable_verification_bullets != self.verification_bullets {
            out.push("publishable_verification_bullets");
        }
        if self.publishable_impact_refs != self.impact_refs {
            out.push("publishable_impact_refs");
        }
        if self.publishable_carry_forward_bullets != self.carry_forward_bullets {
            out.push("publishable_carry_forward_bullets");
        }
        out
    }

    /// Round 296 — SHA256 of the publishable half, hex-encoded.
    ///
    /// Computes a deterministic content hash over the 5 publishable_*
    /// fields by serializing them as JSON (BTreeMap-key ordering already
    /// applies inside Vec<String>; serde_json preserves struct field order
    /// from the explicit Serialize impl below). The hash is the anchor
    /// stored in `[[publishable_override_ledger]].content_hash_after`;
    /// validate-workspace recomputes it per entry and rejects any divergent
    /// entry whose hash does not match a ledger row.
    ///
    /// Mutating publishable_* without re-anchoring the ledger row produces
    /// a hash mismatch — the ledger is forge-resistant by construction.
    pub fn publishable_hash_hex(&self) -> String {
        // Inline shape (not the full struct, only the publishable half) so
        // that adding new audit fields later does not silently invalidate
        // every prior content_hash_after anchor.
        let payload = serde_json::json!({
        "publishable_decision_summary": self.publishable_decision_summary,
        "publishable_changes_bullets": self.publishable_changes_bullets,
        "publishable_verification_bullets": self.publishable_verification_bullets,
        "publishable_impact_refs": self.publishable_impact_refs,
        "publishable_carry_forward_bullets": self.publishable_carry_forward_bullets,
        });
        sha256_hex(
            &serde_json::to_vec(&payload)
                .expect("serializing owned publishable strings to JSON is infallible"),
        )
    }

    /// Round 296 — SHA256 of the audit half, hex-encoded. Optional
    /// content_hash_before anchor in the ledger; informational since the
    /// audit half is immutable post-append.
    pub fn audit_hash_hex(&self) -> String {
        let payload = serde_json::json!({
        "decision_summary": self.decision_summary,
        "changes_bullets": self.changes_bullets,
        "verification_bullets": self.verification_bullets,
        "impact_refs": self.impact_refs,
        "carry_forward_bullets": self.carry_forward_bullets,
        });
        sha256_hex(
            &serde_json::to_vec(&payload)
                .expect("serializing owned audit strings to JSON is infallible"),
        )
    }
}

/// Atomic inventory entry — Phase 1A 5th closed-form entity (Round 273).
///
/// Schema rationale: external-dogfood projects (TC8 harness as the seeding
/// case) cite stable IDs in code (`ARP_07`, `TCP_RETRANSMISSION_TO_04`) whose
/// lifecycle (`Active` / `Deprecated` / `Reserved`) and section binding
/// (`§4.2.4`) must be validated cite-time. `ChangelogEntry` does not fit —
/// audit-trail genre with body and `frozen_ledger_jaccard` T2 protection —
/// while inventory entries have no body (often license-blocked from import)
/// and demand a different reject vocabulary.
///
/// Field shape kept minimal; mutate primitives (`add_inventory_entry`,
/// `set_inventory_status`, etc.) land in Round 274. Validator cite-time
/// axis (T1 inventory existence + Deprecated reject) lands in Round 275.
///
/// Key (in `AtomicStore.inventory_entries`) = the inventory ID itself
/// (e.g., `"ARP_07"`); not duplicated in the struct, mirroring the
/// `AtomicSection` / `AtomicChangelogEntry` convention.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventoryEntry {
    /// Lifecycle status. Default = Active.
    #[serde(default)]
    pub status: InventoryStatus,
    /// Optional Section binding (section_id without leading `§`).
    /// `None` = orphan inventory entry (later rounds may surface as T4 info).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section_ref: Option<String>,
    /// Optional traceability pointer to the upstream SSOT row (PDF page ref,
    /// `case_inventory.json` row id, requirements DB key). Opaque string —
    /// no shape validation; supports humans tracing back to the source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Optional deprecation rationale. Surfaced in cite-time reject messages
    /// when `status = Deprecated` (Round 275 wiring).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Workspace-wide atomic store. Keys = canonical `section_id` / `entry_id` /
/// `inventory_id`. On-disk shape = single JSON file at
/// `docs/.atomic/workspace.atomic.json` by default; Round 279 wires
/// `[atomic] sidecar_path` in `mnemosyne.toml` to override (CLI
/// `--sidecar` flag still wins over config when both are present).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtomicStore {
    #[serde(default)]
    pub sections: BTreeMap<String, AtomicSection>,
    #[serde(default)]
    pub changelog_entries: BTreeMap<String, AtomicChangelogEntry>,
    /// Phase 1A 5th entity — inventory entries (Round 273). Keyed by the
    /// inventory ID (e.g., `"ARP_07"`); empty on schema-version-1 stores
    /// via `#[serde(default)]`.
    #[serde(default)]
    pub inventory_entries: BTreeMap<String, InventoryEntry>,
    /// Max-rigor confirmation events (R416) — append-only audit records, keyed
    /// by a caller-supplied `event_id`. Mirrors `changelog_entries`: a top-level
    /// collection (NOT nested on a section/binding) so it shares the audit-trail
    /// genre and never bloats a section. Empty on pre-v10 stores via
    /// `#[serde(default)]`.
    #[serde(default)]
    pub confirmation_events: BTreeMap<String, ConfirmationEvent>,
    /// Epistemic-frame registry (Round 430) — keyed by frame id;
    /// `ground-truth` is a non-privileged entry like any other. Every
    /// `NarrativeFact.frame` must reference a key here (fail-loud at the
    /// mutate primitives). Empty on pre-v12 stores via `#[serde(default)]`.
    #[serde(default)]
    pub frames: BTreeMap<String, Frame>,
    /// World-line branch registry (Round 436) — keyed by branch id. Every
    /// non-default `NarrativeFact.branch` must reference a key here
    /// (fail-loud at the mutate primitives, symmetric with `frames` — a
    /// write-side typo must never silently create a world). `MAIN_BRANCH`
    /// is known by construction and never registered. Empty on pre-v14
    /// stores via `#[serde(default)]`.
    #[serde(default)]
    pub branches: BTreeMap<String, Branch>,
    /// Narrative entity registry (Round 437) — keyed by entity id. Every
    /// `NarrativeFact.entities` ref must name a key here (fail-loud at the
    /// mutate primitives; frames/branches symmetry). The retrieval key for
    /// entity-scoped verification and the convergence-B `entity_id` seat.
    /// Empty on pre-v15 stores via `#[serde(default)]`.
    #[serde(default)]
    pub entities: BTreeMap<String, Entity>,
    /// Predicate registry (Round 446) — the FOURTH registry, keyed by
    /// predicate id. Every `TypedClaim.predicate` must name a key here
    /// (fail-loud at the mutate primitives). Predicates are load-bearing
    /// (narrative rules key off them), hence registry, not free-form.
    /// Empty on pre-v19 stores via `#[serde(default)]`.
    #[serde(default)]
    pub predicates: BTreeMap<String, Predicate>,
    /// Multi-axis narrative facts (Round 430) — append-only perspectival
    /// claims, keyed by fact id. Top-level (the confirmation_events
    /// placement pattern): never nested on a section, no frozen-ledger
    /// contact. Empty on pre-v12 stores via `#[serde(default)]`.
    #[serde(default)]
    pub narrative_facts: BTreeMap<String, NarrativeFact>,
    /// Disclosure (discourse) plans (Round 506, design sec 7.24) — keyed by
    /// telling id. Each plan is a named telling over the fact base: a default
    /// disclosure mode + sparse per-fact overrides selecting which facts the
    /// reader is told, when, in what mode. Top-level (the registry placement
    /// pattern): one fact base, many tellings. NOT a store-integrity invariant
    /// (disclosure timing is a render property gated over re-extracted prose).
    /// Empty on pre-v22 stores via `#[serde(default)]`.
    #[serde(default)]
    pub disclosure_plans: BTreeMap<String, DisclosurePlan>,
    /// Schema version — bump on breaking shape change.
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
}

fn default_schema_version() -> u32 {
    1
}

// ============================================================================
// Max-rigor confirmation events (R416 — design `claudedocs/
// max-rigor-verification-design.md` sec 12). Append-only audit records that a
// claim (a `Verifies` binding, or a section all-I/O completeness claim) was
// independently re-verified. Events are the SSOT; status / count / `confirmed?`
// are PROJECTIONS computed later (R418), never stored. The core only records
// provenance — fresh-ness is a producer property, not a store invariant
// (design sec 4.6). All vocab lives here in `mnemosyne-atomic` (an audit-store
// concept with no medium-neutral consumer yet — no core lift, per YAGNI).
// ============================================================================

/// The claim a confirmation event is about (design sec 7 claim-key).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConfirmationClaim {
    /// A `Verifies` binding ("does this test verify this requirement?"), keyed
    /// by `(section_id, file, symbol)` — `kind` is implicitly `Verifies`.
    VerifiesBinding {
        section_id: String,
        file: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        symbol: Option<String>,
    },
    /// A section-level all-I/O completeness claim ("the verifies-set covers
    /// every normative I/O behavior of this section").
    SectionCompleteness { section_id: String },
}

impl ConfirmationClaim {
    /// The section this claim is about — both variants carry one. Used by
    /// [`append_confirmation_event`] to enforce the R287 fail-loud rule: the
    /// section must already exist before a claim about it is recorded.
    pub fn section_id(&self) -> &str {
        match self {
            ConfirmationClaim::VerifiesBinding { section_id, .. } => section_id,
            ConfirmationClaim::SectionCompleteness { section_id } => section_id,
        }
    }
}

/// What KIND of producer emitted a confirmation (design sec 4.1 provenance).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfirmerKind {
    /// A deterministic tool (reproducible — CI re-runs it; truly
    /// un-hand-authorable, design sec 4.6).
    Tool,
    /// A fresh-context LLM confirmer (non-deterministic; trusted only as an
    /// independent-set member, design sec 4.6).
    Model,
}

impl ConfirmerKind {
    /// Canonical lowercase label (matches the serde representation).
    pub fn as_str(self) -> &'static str {
        match self {
            ConfirmerKind::Tool => "tool",
            ConfirmerKind::Model => "model",
        }
    }

    /// Parse the canonical tag back to a value; `None` otherwise.
    pub fn from_tag(s: &str) -> Option<Self> {
        match s {
            "tool" => Some(ConfirmerKind::Tool),
            "model" => Some(ConfirmerKind::Model),
            _ => None,
        }
    }
}

/// The verification method recorded on the event (design sec 4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfirmMethod {
    /// Deterministic: the verifying test exercises the bound symbol.
    LinkageCheck,
    /// Fresh-context LLM semantic judgement.
    SemanticReview,
    /// A coverage attestation (an external coverage tool's result).
    CoverageAttestation,
}

impl ConfirmMethod {
    /// Canonical lowercase label (matches the serde representation).
    pub fn as_str(self) -> &'static str {
        match self {
            ConfirmMethod::LinkageCheck => "linkage_check",
            ConfirmMethod::SemanticReview => "semantic_review",
            ConfirmMethod::CoverageAttestation => "coverage_attestation",
        }
    }

    /// Parse the canonical tag back to a value; `None` otherwise.
    pub fn from_tag(s: &str) -> Option<Self> {
        match s {
            "linkage_check" => Some(ConfirmMethod::LinkageCheck),
            "semantic_review" => Some(ConfirmMethod::SemanticReview),
            "coverage_attestation" => Some(ConfirmMethod::CoverageAttestation),
            _ => None,
        }
    }
}

/// The verdict an event records (design sec 4.1). A single `Refute` blocks
/// regardless of confirmations (design sec 8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Confirm,
    Refute,
}

impl Verdict {
    /// Canonical lowercase label (matches the serde representation).
    pub fn as_str(self) -> &'static str {
        match self {
            Verdict::Confirm => "confirm",
            Verdict::Refute => "refute",
        }
    }

    /// Parse the canonical tag back to a value; `None` otherwise.
    pub fn from_tag(s: &str) -> Option<Self> {
        match s {
            "confirm" => Some(Verdict::Confirm),
            "refute" => Some(Verdict::Refute),
            _ => None,
        }
    }
}

/// Who/what produced a confirmation (design sec 4.1 `confirmer`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Confirmer {
    pub kind: ConfirmerKind,
    pub id: String,
    pub version: String,
}

/// Hashes of the artifacts the event was checked against (design sec 4.4). When
/// any drifts, the event stops being `valid` and drops out of `confirmed?`
/// (computed later, R418). `spec_sha256` reuses R404 `text_sha256`; the
/// code/test hashes are collected by the outside producer (design sec 4.6 — the
/// core never reads the files).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactHashes {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spec_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub code_sha256: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub test_sha256: Vec<String>,
}

/// One immutable confirmation / refutation event (design sec 4.1) — the SSOT for
/// "what confirmations happened." Everything derived (status, count,
/// `confirmed?`) is a projection computed later, never stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfirmationEvent {
    pub claim: ConfirmationClaim,
    pub confirmer: Confirmer,
    pub method: ConfirmMethod,
    #[serde(default)]
    pub artifact_hashes: ArtifactHashes,
    /// The run that AUTHORED the claim.
    pub authoring_run: String,
    /// The run that produced THIS verdict — must differ from `authoring_run`
    /// (self-confirm reject, design sec 4.7).
    pub confirming_run: String,
    pub verdict: Verdict,
    pub rationale: String,
    /// Caller-supplied (determinism — never generated in-core, design sec 4.1).
    pub timestamp: String,
}

/// Confirmation status of a claim — a PROJECTION over the event log (R418),
/// never stored (design sec 4.5). `Refuted` wins (one credible refute blocks,
/// design sec 8); else `Confirmed` iff the v1 required-evidence-set is met among
/// the VALID events; else `Stale` if the claim had a confirm that drifted out of
/// validity (R420); else `Proposed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfirmationStatus {
    Proposed,
    Confirmed,
    Refuted,
    /// Had a confirm that is no longer current — its `artifact_hashes` diverged
    /// from the live artifacts (R420 drift). Distinct from `Proposed` (never had
    /// evidence): a `Stale` claim WAS confirmed and now demands re-confirmation.
    Stale,
}

/// Per-claim confirmation projection (R418/R420). Counts are over the VALID
/// events (those passing the drift predicate); `stale_count` is the events
/// dropped by drift.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ClaimConfirmation {
    pub claim: ConfirmationClaim,
    pub status: ConfirmationStatus,
    pub confirm_count: usize,
    pub refute_count: usize,
    /// Events dropped by the drift predicate (R420) — no longer current.
    pub stale_count: usize,
    /// A deterministic tool `linkage_check` Confirm exists (among valid events).
    pub has_tool_linkage: bool,
    /// Distinct `confirming_run` among valid `semantic_review` Confirms
    /// (independence count; self-confirm is already impossible — append rejects).
    pub independent_semantic: usize,
}

/// Whole-store confirmation projection (R418).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConfirmationReport {
    pub claims: Vec<ClaimConfirmation>,
}

impl ConfirmationReport {
    /// The confirmation-debt work-queue (design sec 4.5): every claim not yet
    /// `Confirmed` (`Proposed`, `Refuted`, or `Stale`).
    pub fn debt(&self) -> impl Iterator<Item = &ClaimConfirmation> {
        self.claims
            .iter()
            .filter(|c| c.status != ConfirmationStatus::Confirmed)
    }
}

/// Build the confirmation projection — PURE over the event log, nothing stored.
/// Drift-unaware: every stored event counts as current. Equivalent to
/// [`confirmation_report_with`] with an always-valid predicate.
pub fn confirmation_report(store: &AtomicStore) -> ConfirmationReport {
    confirmation_report_with(store, |_| true)
}

/// Build the confirmation projection with a caller-supplied VALIDITY predicate
/// (R420). `is_valid(event)` decides whether an event is still current; the
/// drift check itself (re-hashing spec / code / test artifacts) lives in the
/// outer validate layer so the core stays file-free (design sec 4.6). Groups
/// events by claim, then classifies each via the v1 required-evidence-set
/// (design sec 4.2 decision A) over the VALID events: a deterministic tool
/// `linkage_check` Confirm AND ≥ 1 independent `semantic_review` Confirm, zero
/// valid refutations. One valid `Refute` blocks (design sec 8). A claim whose
/// only confirms drifted out becomes `Stale` (was confirmed, now demands
/// re-confirmation) rather than `Proposed`.
pub fn confirmation_report_with<F: Fn(&ConfirmationEvent) -> bool>(
    store: &AtomicStore,
    is_valid: F,
) -> ConfirmationReport {
    let mut by_claim: BTreeMap<ConfirmationClaim, Vec<&ConfirmationEvent>> = BTreeMap::new();
    for ev in store.confirmation_events.values() {
        by_claim.entry(ev.claim.clone()).or_default().push(ev);
    }
    let mut claims = Vec::new();
    for (claim, all) in by_claim {
        let valid: Vec<&ConfirmationEvent> = all.iter().copied().filter(|e| is_valid(e)).collect();
        let stale_count = all.len() - valid.len();
        let confirm_count = valid
            .iter()
            .filter(|e| e.verdict == Verdict::Confirm)
            .count();
        let refute_count = valid
            .iter()
            .filter(|e| e.verdict == Verdict::Refute)
            .count();
        let has_tool_linkage = valid
            .iter()
            .any(|e| e.verdict == Verdict::Confirm && e.method == ConfirmMethod::LinkageCheck);
        let mut sem_runs: Vec<&str> = valid
            .iter()
            .filter(|e| {
                e.verdict == Verdict::Confirm
                    && e.method == ConfirmMethod::SemanticReview
                    && e.confirming_run != e.authoring_run
            })
            .map(|e| e.confirming_run.as_str())
            .collect();
        sem_runs.sort_unstable();
        sem_runs.dedup();
        let independent_semantic = sem_runs.len();
        // A confirm that drifted out of the valid set: the claim HAD evidence
        // that is no longer current (R420) — `Stale`, not `Proposed`.
        let invalid_confirm = all
            .iter()
            .copied()
            .any(|e| e.verdict == Verdict::Confirm && !is_valid(e));
        let status = if refute_count > 0 {
            ConfirmationStatus::Refuted
        } else if has_tool_linkage && independent_semantic >= 1 {
            ConfirmationStatus::Confirmed
        } else if invalid_confirm {
            ConfirmationStatus::Stale
        } else {
            ConfirmationStatus::Proposed
        };
        claims.push(ClaimConfirmation {
            claim,
            status,
            confirm_count,
            refute_count,
            stale_count,
            has_tool_linkage,
            independent_semantic,
        });
    }
    ConfirmationReport { claims }
}

#[derive(Debug, Error)]
pub enum AtomicStoreError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json parse: {0}")]
    Json(#[from] serde_json::Error),
    #[error("schema version mismatch: store={store} expected ≤ {expected}")]
    SchemaVersionMismatch { store: u32, expected: u32 },
}

// Schema version 2 (Round 273): Phase 1A entry — adds AtomicStore.inventory_entries.
// Schema version 3 (Round 287): outline lift — adds AtomicSection.title /
// .parent_doc / .parent_section (title-from-workspace-pending carry closure). Pre-v3 sections
// deserialize with empty title/parent_doc + parent_section=None via
// #[serde(default)]; Phase I backfill migration populates them from
// workspace markdown-derived Section data.
// Schema version 4 (Round 294): publishable / audit body split on
// AtomicChangelogEntry. Pre-v4 entries deserialize with empty publishable_*
// fields via #[serde(default)]; the v3→v4 migration in `AtomicStore::load`
// clones audit_* into publishable_* per entry so the default render shape
// stays byte-identical until R295 setters explicitly diverge them.
//
// Schema version 5: Path B `Section.implementations[] = {file, symbol}`
// became `Section.bindings[] = {file, symbol, kind}` (typed trace-link
// edge; BindingKind = Implements | References). Unlike v3→v4 (a *content
// transform* that must run imperatively in `load`), v4→v5 is a pure
// field-rename + new-field-default, which serde expresses idiomatically and
// declaratively — so there is deliberately NO `schema_version < 5` arm in
// `load`: the renamed `bindings` field reads the old `implementations` JSON
// key via #[serde(alias)] and each legacy binding (no `kind` on disk)
// defaults to `Implements` via #[serde(default)]. Behavior-preserving,
// because coverage counted every binding before the split. The inferred
// defaults are NOT silently blessed: `AtomicStore::kind_migration_report`
// (surfaced by the CLI `report-binding-migration` verb) lists them while
// `schema_version < 5`, i.e. before the first save bumps the version.
//
// Load is back-compat across all versions ≤ CURRENT: version-N stores
// deserialize with newer fields defaulted; the next save rewrites
// schema_version to CURRENT.
// v5→v6 adds `AtomicSection.coverage_expectation` (Normative | Informative).
// Like v4→v5 (and unlike v3→v4's content transform), this is a pure new-field
// default, expressed declaratively: a pre-v6 store has no `coverage_expectation`
// key, so serde `#[serde(default)]` fills `Normative` — which preserves the
// Round 269 coverage axiom exactly (every section expected coverage before the
// split). So there is deliberately NO `schema_version < 6` arm in `load`. The
// default is the conservative no-op, not a silently-blessed claim, so it needs
// no migration report (contrast v4→v5's `kind = Implements` default, which was
// a reviewable claim surfaced by `report-binding-migration`).
// v6→v7 adds `AtomicSection.epub_locator` (EPUB-SSOT pointer, R393). Same
// declarative new-field-default pattern: a pre-v7 store has no `epub_locator`
// key, serde `#[serde(default)]` fills `None` (no EPUB mirrored) — byte-identical
// on disk, no behavior change. So there is deliberately NO `schema_version < 7`
// arm in `load`. The locator is a derived pointer (set by `import-epub-anchors`),
// not an authored value, so no migration report is needed.
// v7→v8 adds `NormativeExcerpt.text_sha256` (R402). Same declarative
// new-field-default pattern: a pre-v8 excerpt has no `text_sha256` key, serde
// `#[serde(default)]` fills "" — byte-identical behavior, no `schema_version < 8`
// arm. Unlike `epub_locator`, an empty hash IS a reviewable gap (the excerpt's
// `text` is not yet revalidatable against an EPUB), so it is surfaced by
// `excerpt_hash_backfill_report` / `report-excerpt-hash-backfill` — a
// schema-independent work-list (the gap persists across saves until the excerpt
// is re-imported from an EPUB via `import_epub_excerpts`).
// v8→v9 adds `AtomicSection.verification_expectation` (Dedicated | ByConstruction,
// R413). Same declarative new-field-default pattern as v5→v6 coverage_expectation:
// a pre-v9 store has no `verification_expectation` key, serde `#[serde(default)]`
// fills `Dedicated` — but because the VerificationMissing gate is OFF unless
// `severity_verification` is explicitly configured, an unclassified store gates
// identically to before (no verify violations). So there is deliberately NO
// `schema_version < 9` arm in `load`, and no migration report is needed.
// v9→v10 adds `AtomicStore.confirmation_events` (max-rigor confirmation
// subsystem, R416) — a top-level append-only collection mirroring
// `changelog_entries`. Same declarative new-field-default pattern: a pre-v10
// store has no `confirmation_events` key, serde `#[serde(default)]` fills an
// empty map — no behavior change (nothing reads the events until the R418
// predicate / R419 gate land, and that gate is opt-in). So there is deliberately
// NO `schema_version < 10` arm in `load`, and no migration report is needed.
// v10→v11 widens `AtomicSection.coverage_expectation` from 2-state
// (Normative | Informative) to 3-state (Normative | OutOfScopeHere |
// Informational, R421). The `informative` alias was REMOVED (R422 clean break):
// a store still carrying that tag fails to load LOUDLY (an unknown enum tag
// errors — no silent drop), so a consumer migrates `informative` →
// `out_of_scope_here` deliberately before bumping. New 3-state stores gate
// identically to the old 2-state (both OutOfScopeHere and Informational leave the
// coverage axiom, exactly as Informative did). SCE's 50 `informative` sections
// migrate this way on rev bump.
// v11→v12 adds `AtomicStore.frames` + `AtomicStore.narrative_facts` (Phase 1A
// narrative fact entity, Round 430) — two top-level collections mirroring the
// v9→v10 confirmation_events placement. Same declarative new-field-default
// pattern: a pre-v12 store has no `frames` / `narrative_facts` keys, serde
// `#[serde(default)]` fills empty maps — no behavior change (nothing reads
// them until the continuity gate lands, and that gate is opt-in). So there is
// deliberately NO `schema_version < 12` arm in `load`, and no migration
// report is needed.
// v12→v13 adds `NarrativeFact.branch` (world-line branch axis, Round 433 —
// design sec 7.9 axis 2). Declarative serde default: a pre-v13 fact has no
// `branch` key, serde fills `MAIN_BRANCH`, and serialization skips the
// default — a single-world store round-trips byte-identical. Conflict
// scoping and succession widen from `frame` to `(frame, branch)` (guardrail
// B-2 key-widening); a store that never names a branch gates exactly as
// before. So there is deliberately NO `schema_version < 13` arm in `load`,
// and no migration report is needed.
// v13→v14 adds `AtomicStore.branches` (world-line branch registry, Round
// 436) — the frames-registry symmetry the R433 minimal pin deferred: branch
// refs now fail loud at the write path (`MAIN_BRANCH` ∪ registry) instead of
// free-form strings, closing the write-side-typo-creates-a-world gap the
// session review surfaced. Same declarative new-field-default pattern: a
// pre-v14 store has no `branches` key, serde fills an empty map, and a
// single-world store (every fact on the default branch) loads and gates
// exactly as before. So there is deliberately NO `schema_version < 14` arm
// in `load`, and no migration report is needed.
// v14→v15 adds `AtomicStore.entities` + `NarrativeFact.entities` (narrative
// entity axis, Round 437 — design sec 7.10 gap 4, pulled live by the
// AAA/pinion consumer: entity-scoped verification needs a retrieval key).
// Same declarative new-field-default pattern: pre-v15 stores load with an
// empty registry and entity-less facts, and a fact that names no entity
// serializes no `entities` key — byte-stable round-trip. So there is
// deliberately NO `schema_version < 15` arm in `load`, and no migration
// report is needed.
// v15→v16 adds `Branch.forks_from` (world-line fork point, Round 438 — the
// shared-history half of the branch axis the R433 minimal pin deferred,
// surfaced as session-review tension 1: without it a branching story lost
// its pre-divergence facts on the branch view). `None` = standalone world
// (pre-fork semantics preserved exactly); ancestry is a forest by
// construction (parent must already be registered; fork is immutable after
// registration). Declarative serde default — pre-v16 stores load with
// fork-less branches and gate identically. So there is deliberately NO
// `schema_version < 16` arm in `load`, and no migration report is needed.
// v16→v17 changes `NarrativeFact.conflicts_with` from bare target ids to
// [`ConflictAssertion`] rows pinning the target's claim sha256 at judgment
// time (Round 439 — session-review tension 2: an amend of the target must
// not leave recorded semantic judgments silently trusted). CLEAN BREAK, no
// compat shim (pre-release rule): no committed consumer store carries a
// conflict edge yet, and an old-shape store fails to load LOUDLY (string
// where a struct is expected) rather than silently dropping the pin. The
// hash is computed by the primitives, never caller-supplied (R404), and is
// NEVER auto-refreshed — `scan_continuity` surfaces a stale pin as
// `ConflictEdgeStale`, and re-affirmation = amending the edge-owning fact
// (its outbound judgments restamp as the amender's fresh assertions).
// v18→v19 adds `AtomicStore.predicates` (the 4th registry) and
// `NarrativeFact.typed` (the optional TypedClaim leg, Round 446 — design
// sec 7.12 step 2: the machine-readable subject–predicate–object reading
// authored in the same act as the prose claim, never NLP-derived). Both
// declarative serde defaults (empty map / `None`, skip-serialized) — every
// pre-v19 store loads unchanged and stays byte-stable. So there is
// deliberately NO `schema_version < 19` arm in `load`, and no migration
// report is needed.
// v19→v20 added the `ConfirmationClaim::FactEvidence { fact_id }` variant (the
// R481 LLM-verdict drift target). v20→v21 REMOVES it (Round 485 — the
// all-deterministic redesign R484: R483's blind acceptance falsified the
// LLM-verdict approach, drift moved to the deterministic typed-substantiation
// scan, and no-legacy-carry retires the dead variant in the same change). No
// canonical store ever carried a `fact_evidence` event (the dogfood store's
// only such events lived in a throwaway grading copy), so the removal loses no
// data; a v21 store has no `fact_evidence` events and the monotonic bump
// records the variant's retirement. No migration arm needed.
// v21→v22 adds `AtomicStore.disclosure_plans` (the disclosure/discourse layer,
// Round 506 — design sec 7.24): a top-level registry of named tellings over the
// fact base, mirroring the v9→v10 confirmation_events / v11→v12 narrative_facts
// placement. Same declarative new-field-default pattern: a pre-v22 store has no
// `disclosure_plans` key, serde `#[serde(default)]` fills an empty map — no
// behavior change (nothing reads the plans until the `--telling` carrier + the
// render-acceptance gates run, and those are out-of-band render-loop tools, not
// validate-workspace). So there is deliberately NO `schema_version < 22` arm in
// `load`, and no migration report is needed.
/// The store schema generation the current binary writes and validates
/// against (bumped on a breaking shape change). Public so the medium-neutral
/// authoring contract (`describe-schema`, R587) can report which generation it
/// describes.
pub const CURRENT_SCHEMA_VERSION: u32 = 23;
const DEFAULT_SIDECAR_REL: &str = "docs/.atomic/workspace.atomic.json";

impl AtomicStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Default sidecar path relative to workspace root.
    pub fn default_sidecar_path(workspace_root: &Path) -> PathBuf {
        workspace_root.join(DEFAULT_SIDECAR_REL)
    }

    /// Load from sidecar JSON. Returns empty store if file missing.
    ///
    /// Round 294 — v3→v4 migration: stores written under schema_version ≤ 3
    /// have no `publishable_*` fields. Clone audit_* into publishable_* per
    /// entry so the render shape stays byte-identical until R295 setters
    /// explicitly diverge them. v4+ stores keep the two halves independent
    /// (intended divergence after redaction / typo fix).
    pub fn load(path: &Path) -> Result<Self, AtomicStoreError> {
        if !path.exists() {
            return Ok(Self::new());
        }
        let bytes = fs::read(path)?;
        let mut store: AtomicStore = serde_json::from_slice(&bytes)?;
        if store.schema_version > CURRENT_SCHEMA_VERSION {
            return Err(AtomicStoreError::SchemaVersionMismatch {
                store: store.schema_version,
                expected: CURRENT_SCHEMA_VERSION,
            });
        }
        if store.schema_version < 4 {
            for entry in store.changelog_entries.values_mut() {
                entry.clone_audit_into_publishable();
            }
        }
        Ok(store)
    }

    /// v4→v5 migration report: when the store was loaded from a pre-v5
    /// schema, every binding inherited `kind = Implements` by default (no
    /// `kind` existed on disk). This lists each such binding so the inherited
    /// claim is a reviewable work-list — the Stage-B reclassification input —
    /// rather than a silent blessing of unverified `implements`.
    ///
    /// Returns `None` once the store is at the current schema (the first save
    /// rewrites `schema_version` to CURRENT), so this must be read in the same
    /// session that loads the pre-v5 store. Ordered by `(section_id, file,
    /// symbol)` for stable output.
    pub fn kind_migration_report(&self) -> Option<KindMigrationReport> {
        if self.schema_version >= 5 {
            return None;
        }
        let mut rows: Vec<KindMigrationRow> = Vec::new();
        for (section_id, section) in &self.sections {
            for binding in &section.bindings {
                rows.push(KindMigrationRow {
                    section_id: section_id.clone(),
                    file: binding.file.clone(),
                    symbol: binding.symbol.clone(),
                    defaulted_kind: binding.kind,
                });
            }
        }
        rows.sort_by(|a, b| {
            a.section_id
                .cmp(&b.section_id)
                .then_with(|| a.file.cmp(&b.file))
                .then_with(|| a.symbol.cmp(&b.symbol))
        });
        Some(KindMigrationReport {
            from_schema_version: self.schema_version,
            rows,
        })
    }

    /// v7→v8 backfill work-list: every Section whose `normative_excerpt` has an
    /// empty `text_sha256` — hand-authored or pre-v8 excerpts whose `text` is
    /// not yet revalidatable against an EPUB. Re-importing via
    /// `import_epub_excerpts` populates the hash and clears the row. Unlike
    /// [`kind_migration_report`] this is schema-independent: the gap is a real
    /// empty field that persists across saves, not a defaulted-then-blessed
    /// claim. Ordered by `section_id` for stable output.
    pub fn excerpt_hash_backfill_report(&self) -> ExcerptHashBackfillReport {
        let mut rows: Vec<ExcerptHashBackfillRow> = self
            .sections
            .iter()
            .filter_map(|(section_id, section)| {
                section.normative_excerpt.as_ref().and_then(|ne| {
                    ne.text_sha256.is_empty().then(|| ExcerptHashBackfillRow {
                        section_id: section_id.clone(),
                        source_revision: ne.source_revision.clone(),
                    })
                })
            })
            .collect();
        rows.sort_by(|a, b| a.section_id.cmp(&b.section_id));
        ExcerptHashBackfillReport { rows }
    }

    /// Atomic save (temp + rename). Creates parent dir as needed.
    pub fn save(&self, path: &Path) -> Result<(), AtomicStoreError> {
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }
        let mut store = self.clone();
        store.schema_version = CURRENT_SCHEMA_VERSION;
        let json = serde_json::to_vec_pretty(&store)?;
        let tmp_path = path.with_extension("json.tmp");
        {
            let mut tmp = fs::File::create(&tmp_path)?;
            tmp.write_all(&json)?;
            tmp.write_all(b"\n")?;
            tmp.sync_all()?;
        }
        fs::rename(&tmp_path, path)?;
        Ok(())
    }

    pub fn section(&self, section_id: &str) -> Option<&AtomicSection> {
        self.sections.get(section_id)
    }

    pub fn entry(&self, entry_id: &str) -> Option<&AtomicChangelogEntry> {
        self.changelog_entries.get(entry_id)
    }

    /// atomic-derived section_id set (MD-DELETION-RATIFY foundation).
    ///
    /// Returns workspace-wide section_id set sourced from atomic store keys
    /// only — no markdown parsing required. Production use case: T1 cross-ref
    /// Orphan check + atomic-store cross-ref resolution when 7 source MD files
    /// are deleted.
    ///
    /// Parallel to [`crate::query::workspace_section_id_set`] which sources
    /// from `Workspace.docs.values().sections` (markdown-derived). When the
    /// atomic store is the sole source of truth (paradigm shift
    /// complete), this becomes the canonical section_id set.
    ///
    /// also yields *implied parent prefixes* derived from `/`
    /// path components in keys (e.g. key `architecture/layer/l0` implies
    /// parent ids `architecture` and `architecture/layer`). This covers
    /// heading-only roots that were intentionally skipped during atomic
    /// decompose (ARCHITECTURE.md / VISION.md / PRIOR_ART.md h1 roots) but
    /// are still legitimate cross-ref targets.
    pub fn atomic_section_id_set(&self) -> std::collections::BTreeSet<String> {
        let mut set: std::collections::BTreeSet<String> = self.sections.keys().cloned().collect();
        for key in self.sections.keys() {
            let mut start = 0usize;
            while let Some(idx) = key[start..].find('/') {
                let abs = start + idx;
                set.insert(key[..abs].to_string());
                start = abs + 1;
            }
        }
        set
    }

    /// Read-only inventory entry lookup (Round 273, Phase 1A).
    ///
    /// Returns `Some(&InventoryEntry)` when the inventory ID is registered,
    /// `None` otherwise. Mutate access (registration / status transition /
    /// removal) lands as named primitives in Round 274 — there is
    /// deliberately no `inventory_mut` / `inventory_entry_mut` helper here,
    /// so cite-time existence checks cannot accidentally auto-register an
    /// ID by lookup side-effect.
    pub fn inventory(&self, inventory_id: &str) -> Option<&InventoryEntry> {
        self.inventory_entries.get(inventory_id)
    }

    /// Inventory ID set for cite-time existence checks (Round 273, Phase 1A).
    ///
    /// Parallel to [`Self::atomic_section_id_set`]. Inventory IDs are flat
    /// strings (no `/` parent-prefix derivation) because the genre is stable
    /// external IDs without parent-child hierarchy. The validator's inventory
    /// citation axis (Round 275) consults this set in O(log n) membership
    /// checks per scanned cite token.
    pub fn atomic_inventory_id_set(&self) -> std::collections::BTreeSet<String> {
        self.inventory_entries.keys().cloned().collect()
    }
}

/// AtomicStoreView impl — substrate read surface used by Validator
/// plugins. Lives on `AtomicStore` so any caller that already holds a
/// store can pass `&store as &dyn AtomicStoreView` into a
/// ValidationContext.
impl mnemosyne_core::AtomicStoreView for AtomicStore {
    fn snapshot(&self) -> mnemosyne_core::AtomicSnapshot {
        let changelog_entry_ids: std::collections::BTreeSet<String> =
            self.changelog_entries.keys().cloned().collect();

        let section_ids_with_implied_parents = self.atomic_section_id_set();

        // R309 textbook unification: SectionView.decision_status now carries
        // the canonical DecisionStatus (lifted to mnemosyne-core); no
        // adapter layer between schema and view types.
        let sections: BTreeMap<String, mnemosyne_core::SectionView> = self
            .sections
            .iter()
            .map(|(sid, sec)| {
                let bindings = sec
                    .bindings
                    .iter()
                    .map(|b| mnemosyne_core::BindingRef {
                        file: b.file.clone(),
                        symbol: b.symbol.clone(),
                        kind: b.kind,
                    })
                    .collect();
                (
                    sid.clone(),
                    mnemosyne_core::SectionView {
                        bindings,
                        decision_status: sec.skeleton.decision_status,
                        coverage_expectation: sec.coverage_expectation,
                        verification_expectation: sec.verification_expectation,
                    },
                )
            })
            .collect();

        let inventory: BTreeMap<String, InventoryStatus> = self
            .inventory_entries
            .iter()
            .map(|(id, e)| (id.clone(), e.status))
            .collect();

        mnemosyne_core::AtomicSnapshot {
            changelog_entry_ids,
            section_ids_with_implied_parents,
            sections,
            inventory,
        }
    }
}

/// Render an [`AtomicSection`] into a paragraph-separated prose string
///.
///
/// Used by `query.rs::build_section_view` (atomic-first body source — the
/// SectionView consumer wants the full body including mechanical citations
/// rendered for human / agent inspection).
///
/// Style-check callers MUST use [`synthesize_section_prose_body`] instead,
/// which omits mechanical citation blocks (file paths) that are not
/// authored prose. See that function's doc for the category rationale.
///
/// Bullet blocks render with `- ` prefixes (so `is_only_code_or_table`
/// filters them out of paragraph-length checks); examples render as fenced
/// code blocks (skipped by detectors).
pub fn synthesize_section_body(atomic: &AtomicSection) -> String {
    synthesize_section_body_inner(atomic, true)
}

/// Style-check variant of [`synthesize_section_body`].
///
/// Identical to `synthesize_section_body` except that the `implementations`
/// block (file paths emitted by the renderer for cross-citation) is
/// excluded. Implementations entries are mechanical, filesystem-shaped
/// identifiers — by Unix/C convention they are lowercase (`dut/`,
/// `include/tc8/`, `someip_*.h`) regardless of the canonical prose form
/// of the same concept (DUT / TC8 / SOME/IP). Running
/// `terminology_consistency` (or any other prose-targeted rule) over them
/// is a category error: the glossary expresses "when an author writes the
/// dut received..., suggest the DUT received..." — it was never meant to
/// police filesystem paths.
///
/// Used by `style.rs::resolve_section_body` for all section-body style
/// rules. `impact_scope` (section-id cross-refs) is retained — those IDs
/// are URL-slug shaped and may legitimately contain glossary variants
/// authored as prose. `examples` (fenced code) is retained — code examples
/// are illustrative spec content; if an example uses wrong terminology in
/// a comment, the rule should still flag it.
pub fn synthesize_section_prose_body(atomic: &AtomicSection) -> String {
    synthesize_section_body_inner(atomic, false)
}

fn synthesize_section_body_inner(atomic: &AtomicSection, include_implementations: bool) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(intent) = atomic.intent.as_ref().filter(|s| !s.is_empty()) {
        parts.push(intent.clone());
    }
    let push_bullet_block = |parts: &mut Vec<String>, bullets: &[String]| {
        if bullets.is_empty() {
            return;
        }
        let block: Vec<String> = bullets.iter().map(|b| format!("- {}", b)).collect();
        parts.push(block.join("\n"));
    };
    push_bullet_block(&mut parts, &atomic.rationale_bullets);
    push_bullet_block(&mut parts, &atomic.inputs_bullets);
    push_bullet_block(&mut parts, &atomic.outputs_bullets);
    push_bullet_block(&mut parts, &atomic.caveats_bullets);
    if !atomic.alternatives_rejected.is_empty() {
        let block: Vec<String> = atomic
            .alternatives_rejected
            .iter()
            .map(|a| format!("- {} -- {}", a.alternative, a.reason))
            .collect();
        parts.push(block.join("\n"));
    }
    if !atomic.impact_scope.is_empty() {
        let block: Vec<String> = atomic
            .impact_scope
            .iter()
            .map(|s| format!("- §{}", s))
            .collect();
        parts.push(block.join("\n"));
    }
    if include_implementations && !atomic.bindings.is_empty() {
        let block: Vec<String> = atomic
            .bindings
            .iter()
            .map(|b| match &b.symbol {
                Some(s) => format!("- {}:{}", b.file, s),
                None => format!("- {}", b.file),
            })
            .collect();
        parts.push(block.join("\n"));
    }
    for ex in &atomic.examples {
        parts.push(format!("```{}\n{}\n```", ex.language, ex.code));
    }
    parts.join("\n\n")
}

/// Mutate primitive error.
#[derive(Debug, Error)]
pub enum AtomicMutateError {
    #[error("validation: {0}")]
    Validation(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("frozen ledger: {0}")]
    FrozenLedger(String),
    #[error("store: {0}")]
    Store(#[from] AtomicStoreError),
}

/// Mutate primitive receipt — minimal shape for atomic mutations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AtomicMutateReceipt {
    pub primitive: String,
    pub target_kind: &'static str,
    pub target_id: String,
    pub sidecar_path: String,
    pub written_bytes: usize,
}

// ============================================================================
// Section atomic mutate primitives.
// ============================================================================

const MAX_INTENT_CHAR: usize = 200;
const MAX_BULLET_CHAR: usize = 100;

fn check_intent_len(text: &str) -> Result<(), AtomicMutateError> {
    if text.chars().count() > MAX_INTENT_CHAR {
        return Err(AtomicMutateError::Validation(format!(
            "intent length {} > MAX_INTENT_CHAR {} (Round 161 §41 threshold)",
            text.chars().count(),
            MAX_INTENT_CHAR
        )));
    }
    Ok(())
}

fn check_bullet_len(text: &str, field: &str) -> Result<(), AtomicMutateError> {
    if text.chars().count() > MAX_BULLET_CHAR {
        return Err(AtomicMutateError::Validation(format!(
            "{} bullet length {} > MAX_BULLET_CHAR {} (Round 161 §41 threshold)",
            field,
            text.chars().count(),
            MAX_BULLET_CHAR
        )));
    }
    Ok(())
}

// Round 298 — close the silent-accept hole on `append_changelog_entry`:
// previously entry-id alone with no decision/changes/verification body could
// land a record-less row into the frozen ledger. The primitive now refuses
// at the boundary, which covers CLI and MCP wires equally.
fn check_changelog_entry_required(
    decision_summary: Option<&str>,
    changes_bullets: &[String],
    verification_bullets: &[String],
    impact_refs: &[String],
    carry_forward_bullets: &[String],
) -> Result<(), AtomicMutateError> {
    let summary = decision_summary.ok_or_else(|| {
        AtomicMutateError::Validation(
            "decision_summary required (Round 298 silent-accept gate)".to_string(),
        )
    })?;
    if summary.trim().is_empty() {
        return Err(AtomicMutateError::Validation(
            "decision_summary blank (Round 298 silent-accept gate)".to_string(),
        ));
    }
    check_required_bullets("changes_bullets", changes_bullets)?;
    check_required_bullets("verification_bullets", verification_bullets)?;
    check_optional_bullets("impact_refs", impact_refs)?;
    check_optional_bullets("carry_forward_bullets", carry_forward_bullets)?;
    Ok(())
}

fn check_required_bullets(field: &str, bullets: &[String]) -> Result<(), AtomicMutateError> {
    if bullets.is_empty() {
        return Err(AtomicMutateError::Validation(format!(
            "{} requires at least one non-blank bullet (Round 298 silent-accept gate)",
            field
        )));
    }
    check_optional_bullets(field, bullets)
}

fn check_optional_bullets(field: &str, bullets: &[String]) -> Result<(), AtomicMutateError> {
    for (i, b) in bullets.iter().enumerate() {
        if b.trim().is_empty() {
            return Err(AtomicMutateError::Validation(format!(
                "{}[{}] is blank (Round 298 silent-accept gate)",
                field, i
            )));
        }
    }
    Ok(())
}

fn save_with_receipt(
    store: &AtomicStore,
    sidecar_path: &Path,
    primitive: &str,
    target_kind: &'static str,
    target_id: &str,
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
    store.save(sidecar_path)?;
    let written = fs::metadata(sidecar_path)
        .map(|m| m.len() as usize)
        .unwrap_or(0);
    Ok(AtomicMutateReceipt {
        primitive: primitive.to_string(),
        target_kind,
        target_id: target_id.to_string(),
        sidecar_path: sidecar_path.display().to_string(),
        written_bytes: written,
    })
}

/// Round 287 — fail-loud Section lookup for mutate primitives.
///
/// Returns `NotFound` when `section_id` is absent. Closes the pre-R287
/// silent-create footgun: every set_section_* / add_section_* primitive now
/// requires the Section to exist (created via `add_section`). Creation and
/// population are explicitly separated — matches the rest of the atomic API.
fn section_mut_strict<'a>(
    store: &'a mut AtomicStore,
    section_id: &str,
) -> Result<&'a mut AtomicSection, AtomicMutateError> {
    store.sections.get_mut(section_id).ok_or_else(|| {
        AtomicMutateError::NotFound(format!(
            "section_id `{}` not present in atomic store (use add_section to create it first)",
            section_id
        ))
    })
}

pub fn set_section_intent(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    section_id: &str,
    intent: &str,
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
    check_intent_len(intent)?;
    section_mut_strict(store, section_id)?.intent = Some(intent.to_string());
    save_with_receipt(
        store,
        sidecar_path,
        "set_section_intent",
        "section",
        section_id,
    )
}

pub fn set_section_rationale(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    section_id: &str,
    bullets: &[String],
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
    for b in bullets {
        check_bullet_len(b, "rationale")?;
    }
    section_mut_strict(store, section_id)?.rationale_bullets = bullets.to_vec();
    save_with_receipt(
        store,
        sidecar_path,
        "set_section_rationale",
        "section",
        section_id,
    )
}

pub fn set_section_inputs(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    section_id: &str,
    bullets: &[String],
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
    for b in bullets {
        check_bullet_len(b, "inputs")?;
    }
    section_mut_strict(store, section_id)?.inputs_bullets = bullets.to_vec();
    save_with_receipt(
        store,
        sidecar_path,
        "set_section_inputs",
        "section",
        section_id,
    )
}

pub fn set_section_outputs(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    section_id: &str,
    bullets: &[String],
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
    for b in bullets {
        check_bullet_len(b, "outputs")?;
    }
    section_mut_strict(store, section_id)?.outputs_bullets = bullets.to_vec();
    save_with_receipt(
        store,
        sidecar_path,
        "set_section_outputs",
        "section",
        section_id,
    )
}

pub fn add_section_caveat(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    section_id: &str,
    bullet: &str,
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
    check_bullet_len(bullet, "caveats")?;
    section_mut_strict(store, section_id)?
        .caveats_bullets
        .push(bullet.to_string());
    save_with_receipt(
        store,
        sidecar_path,
        "add_section_caveat",
        "section",
        section_id,
    )
}

pub fn set_section_alternatives(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    section_id: &str,
    alternatives: &[RejectedAlternative],
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
    section_mut_strict(store, section_id)?.alternatives_rejected = alternatives.to_vec();
    save_with_receipt(
        store,
        sidecar_path,
        "set_section_alternatives",
        "section",
        section_id,
    )
}

pub fn set_section_impact_scope(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    section_id: &str,
    refs: &[String],
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
    section_mut_strict(store, section_id)?.impact_scope = refs.to_vec();
    save_with_receipt(
        store,
        sidecar_path,
        "set_section_impact_scope",
        "section",
        section_id,
    )
}

pub fn add_section_example(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    section_id: &str,
    example: ExampleBlock,
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
    section_mut_strict(store, section_id)?
        .examples
        .push(example);
    save_with_receipt(
        store,
        sidecar_path,
        "add_section_example",
        "section",
        section_id,
    )
}

/// Path B binding entry append.
///
/// Validation at the trust boundary (data integrity only — language
/// grammar belongs in 's cross-check):
/// - `file`: non-empty after trim, workspace-relative POSIX shape (reject
/// leading `/`, leading `./`, `..` segment, `\`, internal `//`,
/// trailing `/`). File existence is *not* checked — schema records
/// intent; consumption-time check is 's concern.
/// - `symbol`: when `Some`, non-empty after trim, no whitespace edges,
/// no internal newline. Opaque otherwise (no language regex).
///
/// Set semantics: duplicate `(file, symbol)` returns Validation error
/// (fail-loud > silent dedup; the data model is a set of bindings).
/// Existing entries are append-only — no remove/replace primitive
/// exists in this round (frozen-ledger doctrine for
/// atomic fields).
/// Round 267 — atomic section removal primitive.
///
/// Removes a section entry from `AtomicStore.sections` entirely. Closes the
/// gap exposed by Round 266 cleanup (CLAUDE.md override grant path) where
/// authoring loops touching wrong section_ids had no clean self-cleanup
/// route short of direct JSON edit.
///
/// `reason` is mandatory and recorded as the receipt's primitive payload —
/// the audit safeguard for an otherwise-destructive operation. The atomic
/// store is the audit trail; git history of the sidecar JSON preserves the
/// prior state regardless, but the receipt makes the *intent* explicit.
///
/// Returns `NotFound` when the section_id is absent (no silent no-op — the
/// caller asked to remove something specific). No referential-integrity
/// check (cross_refs / impact_scope pointing at the removed id) — that's
/// validate-workspace's job, not the atomic primitive's.
pub fn remove_section(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    section_id: &str,
    reason: &str,
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
    if reason.trim().is_empty() {
        return Err(AtomicMutateError::Validation(
            "remove_section: --reason mandatory (audit-trail safeguard)".to_string(),
        ));
    }
    if store.sections.remove(section_id).is_none() {
        return Err(AtomicMutateError::NotFound(format!(
            "section_id `{}` not present in atomic store",
            section_id
        )));
    }
    save_with_receipt(store, sidecar_path, "remove_section", "section", section_id)
}

/// Round 287 — atomic section creation primitive.
///
/// Pairs with `remove_section` (Round 267): symmetric add/remove on the atomic
/// `sections` map. Closes the outline-lift carry — Sections created
/// via this path carry their own `title` / `parent_doc` / `parent_section`,
/// retiring the `ATOMIC_ONLY_PARENT_DOC` sentinel and the
/// `intent → title` fallback that query.rs synthesized for atomic-only
/// sections.
///
/// Single-responsibility: create the outline shell only. Content fields
/// (`intent` / `rationale_bullets` / etc.) start at their `Default` values;
/// subsequent `set_section_*` / `add_section_*` calls populate them. This
/// matches the rest of the atomic API surface (one primitive, one mutation).
///
/// Validations (all `AtomicMutateError::Validation` except parent NotFound):
/// - `section_id` non-empty after trim
/// - `parent_doc` non-empty after trim
/// - `title` non-empty after trim
/// - `section_id` not already present in store (uniqueness — fail loud over
///   silent overwrite; the pre-R287 silent-create footgun is closed by
///   routing all creation through this primitive)
/// - `parent_section`, when `Some(_)`, must be non-empty and exist in store
///   (referential integrity at write time)
///
/// `decision_status` is initialized to `Some(Active)` — newly created sections
/// are *explicitly* Active, distinct from Round 269's `None = parser default`
/// case which only applies to pre-251 carry sections. Subsequent transitions
/// route through `set_section_decision_status`.
/// Validate + construct a new `AtomicSection` candidate WITHOUT inserting or
/// saving. The single section-create write-path: [`add_section`] (single,
/// + 1 save) and [`import_sections`] (bulk, N inserts + 1 save) both shape a
/// new Section here, so the validation + field-construction invariants live
/// in exactly one place (CLAUDE.md single-write-path rule). Returns the
/// trimmed `section_id` plus the constructed Section.
///
/// Does NOT check for a pre-existing `section_id` — duplicate handling is the
/// caller's policy (`add_section` hard-rejects; `import_sections` runs the
/// 3-way absent/identical/divergent classification). `store` is read-only
/// here (the parent-section existence check), so the bulk caller may insert
/// earlier manifest entries before building a later child that parents to one.
///
/// `normative_excerpt` (`(text, anchor_url, source_revision, text_sha256)`)
/// anchors the excerpt INLINE at create via the shared
/// [`build_normative_excerpt`] validator (the same validator the EPUB
/// re-projection path [`import_epub_excerpts`] uses, so the two write paths
/// cannot drift on the invariant set).
fn build_candidate_section(
    store: &AtomicStore,
    section_id: &str,
    parent_doc: &str,
    title: &str,
    parent_section: Option<&str>,
    normative_excerpt: Option<(&str, &str, &str, &str)>,
    coverage_expectation: mnemosyne_core::CoverageExpectation,
) -> Result<(String, AtomicSection), AtomicMutateError> {
    // Strip a leading `§` citation sigil so store keys stay bare, regardless of
    // caller. The CLI/MCP boundaries already strip for the set_section_* paths;
    // doing it here in the one shared section-create core makes `add_section`
    // AND `import_sections` symmetric — a citation-form manifest entry (a
    // section_id carrying a leading sigil) can no longer slip through
    // `import_sections` and render with a doubled sigil.
    let section_id_t = strip_section_marker(section_id.trim()).trim();
    let parent_doc_t = parent_doc.trim();
    let title_t = title.trim();

    if section_id_t.is_empty() {
        return Err(AtomicMutateError::Validation(
            "section_id mandatory (non-empty after trim)".to_string(),
        ));
    }
    if parent_doc_t.is_empty() {
        return Err(AtomicMutateError::Validation(
            "parent_doc mandatory (non-empty after trim)".to_string(),
        ));
    }
    if title_t.is_empty() {
        return Err(AtomicMutateError::Validation(
            "title mandatory (non-empty after trim)".to_string(),
        ));
    }
    let parent_section_norm = if let Some(parent) = parent_section {
        let parent_t = strip_section_marker(parent.trim()).trim();
        if parent_t.is_empty() {
            return Err(AtomicMutateError::Validation(
                "parent_section must be None or non-empty".to_string(),
            ));
        }
        if !store.sections.contains_key(parent_t) {
            return Err(AtomicMutateError::NotFound(format!(
                "parent_section `{}` not present in atomic store",
                parent_t
            )));
        }
        Some(parent_t.to_string())
    } else {
        None
    };
    let excerpt =
        match normative_excerpt {
            Some((text, anchor_url, source_revision, text_sha256)) => Some(
                build_normative_excerpt(text, anchor_url, source_revision, text_sha256)?,
            ),
            None => None,
        };
    let section = AtomicSection {
        skeleton: mnemosyne_core::SectionSkeleton {
            title: title_t.to_string(),
            parent_doc: parent_doc_t.to_string(),
            parent_section: parent_section_norm,
            decision_status: Some(DecisionStatus::Active),
        },
        normative_excerpt: excerpt,
        coverage_expectation,
        ..Default::default()
    };
    Ok((section_id_t.to_string(), section))
}

pub fn add_section(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    section_id: &str,
    parent_doc: &str,
    title: &str,
    parent_section: Option<&str>,
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
    let (section_id_t, section) = build_candidate_section(
        store,
        section_id,
        parent_doc,
        title,
        parent_section,
        None,
        // add_section creates Normative (the default); classification is a
        // post-create concern via set_section_coverage_expectation. Only the
        // bulk import path takes a per-entry classification.
        mnemosyne_core::CoverageExpectation::Normative,
    )?;
    if store.sections.contains_key(&section_id_t) {
        return Err(AtomicMutateError::Validation(format!(
 "add_section: section_id `{}` already exists in atomic store (use set_section_* primitives to mutate)",
 section_id_t
 )));
    }
    store.sections.insert(section_id_t.clone(), section);
    save_with_receipt(store, sidecar_path, "add_section", "section", &section_id_t)
}

/// One manifest entry for the bulk [`import_sections`] primitive. Deserialized
/// from a JSON array. `normative_excerpt` (optional) anchors the excerpt
/// inline at create — the bulk path's reason to exist (a section's
/// frozen-anchor moment IS its creation, per RFC-002 FR-1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SectionImport {
    pub section_id: String,
    pub parent_doc: String,
    pub title: String,
    #[serde(default)]
    pub parent_section: Option<String>,
    #[serde(default)]
    pub normative_excerpt: Option<NormativeExcerpt>,
    /// Coverage applicability at create (default `Normative`). Lets a bulk
    /// import classify prose-only sections (`Informative`) in the same
    /// manifest that creates them — e.g. an external-spec mirror marking its
    /// terminology / overview sections exempt from the coverage axiom.
    #[serde(default)]
    pub coverage_expectation: mnemosyne_core::CoverageExpectation,
}

/// Bulk section-create from a manifest, as one atomic transaction (RFC-001
/// UC-1 "A2"). Reuses [`build_candidate_section`] per entry — the SAME
/// section-create write-path as [`add_section`], never a bespoke second one
/// (CLAUDE.md half-enforced-invariant rule).
///
/// Per-entry 3-way classification:
/// - **absent** (`section_id` not yet in the store) → create;
/// - **byte-identical** (the would-be section equals the existing one) →
///   no-op skip (idempotent re-run);
/// - **divergent** (present but different) → reject the WHOLE manifest (no
///   silent overwrite — supersede + re-create to revise).
///
/// **Atomicity**: all in-memory mutations happen first; the single
/// `save_with_receipt` runs only after every entry classifies cleanly. Any
/// rejection returns `Err` before the save, so the caller's
/// `run_atomic_mutate` never persists a partially-applied manifest (it loaded
/// the store fresh and discards it on error). Manifest order matters: a child
/// that parents to an earlier manifest entry must follow it (the parent is
/// inserted by then).
///
/// A no-op-only manifest (nothing absent) does NOT save — `written_bytes` is
/// 0 and the receipt reports `0 created`.
pub fn import_sections(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    manifest: &[SectionImport],
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
    let mut created = 0usize;
    let mut no_op = 0usize;
    for (idx, entry) in manifest.iter().enumerate() {
        let excerpt = entry.normative_excerpt.as_ref().map(|n| {
            (
                n.text.as_str(),
                n.anchor_url.as_str(),
                n.source_revision.as_str(),
                n.text_sha256.as_str(),
            )
        });
        let (section_id_t, candidate) = build_candidate_section(
            store,
            &entry.section_id,
            &entry.parent_doc,
            &entry.title,
            entry.parent_section.as_deref(),
            excerpt,
            entry.coverage_expectation,
        )
        .map_err(|e| {
            AtomicMutateError::Validation(format!("import_sections: manifest entry {idx}: {e}"))
        })?;
        // Compute the 3-way verdict without holding a borrow across the insert.
        let verdict = store
            .sections
            .get(&section_id_t)
            .map(|existing| *existing == candidate);
        match verdict {
            None => {
                store.sections.insert(section_id_t, candidate);
                created += 1;
            }
            Some(true) => no_op += 1,
            Some(false) => {
                return Err(AtomicMutateError::Validation(format!(
                    "import_sections: manifest entry {idx} section_id `{section_id_t}` already \
 exists with DIVERGENT content — refusing silent overwrite (supersede + re-create to revise)"
                )));
            }
        }
    }
    let summary = format!("{created} created, {no_op} no-op");
    if created == 0 {
        // Idempotent no-op: nothing absent → nothing to persist.
        return Ok(AtomicMutateReceipt {
            primitive: "import_sections".to_string(),
            target_kind: "section",
            target_id: summary,
            sidecar_path: sidecar_path.display().to_string(),
            written_bytes: 0,
        });
    }
    save_with_receipt(store, sidecar_path, "import_sections", "section", &summary)
}

/// Round 287 — Section.title setter (outline mutate axis).
///
/// In-place rename of an existing Section's heading title. Validates
/// non-empty after trim. Section must exist (fail-loud — use `add_section`
/// to create first).
pub fn set_section_title(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    section_id: &str,
    title: &str,
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
    let title_t = title.trim();
    if title_t.is_empty() {
        return Err(AtomicMutateError::Validation(
            "set_section_title: title mandatory (non-empty after trim)".to_string(),
        ));
    }
    section_mut_strict(store, section_id)?.skeleton.title = title_t.to_string();
    save_with_receipt(
        store,
        sidecar_path,
        "set_section_title",
        "section",
        section_id,
    )
}

/// Round 287 — Section.parent_doc setter (outline mutate axis).
///
/// Re-binds an existing Section to a different owning doc. Validates
/// non-empty after trim. Section must exist (fail-loud). Doc identifier
/// shape (workspace.toml doc list membership) is NOT enforced here —
/// validate-workspace's job, not the atomic primitive's.
pub fn set_section_parent_doc(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    section_id: &str,
    parent_doc: &str,
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
    let pd_t = parent_doc.trim();
    if pd_t.is_empty() {
        return Err(AtomicMutateError::Validation(
            "set_section_parent_doc: parent_doc mandatory (non-empty after trim)".to_string(),
        ));
    }
    section_mut_strict(store, section_id)?.skeleton.parent_doc = pd_t.to_string();
    save_with_receipt(
        store,
        sidecar_path,
        "set_section_parent_doc",
        "section",
        section_id,
    )
}

/// Round 287 — Section.parent_section setter (outline mutate axis).
///
/// Re-parents a Section under a different hierarchy node (or promotes it to
/// top-level by passing `None`). Validations:
/// - Section being mutated must exist (fail-loud)
/// - When `Some(parent)`, parent must be non-empty and exist in store
///   (referential integrity at write time)
/// - Cannot set parent_section == section_id (immediate self-loop)
///
/// Deep cycle detection (A → B → C → A) is NOT performed here — that's
/// validate-workspace's territory (T1-class structural axis). This primitive
/// rejects only the trivial self-loop; deeper inconsistencies surface at
/// validation pass.
pub fn set_section_parent_section(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    section_id: &str,
    parent_section: Option<&str>,
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
    // Validate target Section exists first (fail-loud).
    if !store.sections.contains_key(section_id) {
        return Err(AtomicMutateError::NotFound(format!(
            "section_id `{}` not present in atomic store (use add_section to create it first)",
            section_id
        )));
    }
    let parent_norm = match parent_section {
        Some(p) => {
            let p_t = p.trim();
            if p_t.is_empty() {
                return Err(AtomicMutateError::Validation(
                    "set_section_parent_section: parent_section must be None or non-empty"
                        .to_string(),
                ));
            }
            if p_t == section_id {
                return Err(AtomicMutateError::Validation(format!(
  "set_section_parent_section: parent_section `{}` cannot equal section_id (self-loop)",
  p_t
 )));
            }
            if !store.sections.contains_key(p_t) {
                return Err(AtomicMutateError::NotFound(format!(
                    "set_section_parent_section: parent_section `{}` not present in atomic store",
                    p_t
                )));
            }
            Some(p_t.to_string())
        }
        None => None,
    };
    // Unwrap is safe: contains_key confirmed above and no intervening mutation.
    section_mut_strict(store, section_id)?
        .skeleton
        .parent_section = parent_norm;
    save_with_receipt(
        store,
        sidecar_path,
        "set_section_parent_section",
        "section",
        section_id,
    )
}

/// Atomic decision_status setter (Stage B freshness substrate).
///
/// Sets `AtomicSection.decision_status` to `Some(new_status)`. Idempotent
/// at the value level (re-setting the same status is a no-op write); always
/// persists to keep mutate semantics uniform with the other primitives.
///
/// T1 rule 4 author-time guard: when `new_status == Superseded`, the
/// `superseding` argument is mandatory — Superseded by definition forward-
/// points to a replacement decision, and accepting `None` would permit a
/// semantically-incoherent state (replaced, but no replacement recorded).
/// `Removed` is tombstone-exempt (asserts finality, not replacement).
///
/// On `Superseded` the `superseding` target is stored structurally in
/// `AtomicSection.superseded_by` (the single write path that keeps the
/// `decision_status`/`superseded_by` pair coherent, R342); a transition to
/// `Active`/`Removed` clears it so no stale forward-pointer survives. This
/// guard does not validate that the named superseding section_id exists in
/// the atomic store — cross-ref orphan checking is T1 rule 1's territory,
/// picked up by `validate-workspace`. The caller normalizes the `§` prefix
/// (uniform with [`set_section_impact_scope`]); the stored id is the bare
/// section_id so `project_cross_ref_facts` can address it by entity id.
pub fn set_section_decision_status(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    section_id: &str,
    new_status: DecisionStatus,
    superseding: Option<&str>,
    resolving: Option<&str>,
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
    if new_status == DecisionStatus::Superseded && superseding.is_none() {
        return Err(AtomicMutateError::Validation(
 "(T1 rule 4, atomic axis): superseding section_id mandatory for active → superseded transition".to_string(),
 ));
    }
    {
        let section = section_mut_strict(store, section_id)?;
        section.skeleton.decision_status = Some(new_status);
        section.superseded_by = match new_status {
            DecisionStatus::Superseded => superseding.map(str::to_string),
            // Open poses an undecided question, so like Active/Removed it
            // carries no superseding pointer (Open → Active/Removed on resolve).
            DecisionStatus::Active | DecisionStatus::Removed | DecisionStatus::Open => None,
        };
        // Symmetric resolution forward-pointer — set only while Open, cleared on
        // every other transition (Open → Active/Removed/Superseded drops it).
        section.resolved_by = match new_status {
            DecisionStatus::Open => resolving.map(str::to_string),
            DecisionStatus::Active | DecisionStatus::Removed | DecisionStatus::Superseded => None,
        };
    }
    save_with_receipt(
        store,
        sidecar_path,
        "set_section_decision_status",
        "section",
        section_id,
    )
}

// Lowercase-hex sha256: THE one implementation is `mnemosyne_core::
// sha256_hex` (Round 460 consolidation — the claim pin is stamped in one
// crate and re-checked in another; two implementations of one invariant
// is the half-enforced-invariant class). The stale "external-spec
// mirror" doc that sat here described the R403-deleted
// set-section-normative-excerpt primitive and is gone with it.

/// Validate the four normative-excerpt fields and build the value.
///
/// The single validator + constructor for a `NormativeExcerpt`. Both write
/// paths route through it — the bulk [`import_sections`] inline-at-create path
/// and the EPUB re-projection path [`import_epub_excerpts`] — so the two paths
/// cannot drift on the invariant set (CLAUDE.md half-enforced-invariant rule;
/// the R305 paste-error class). A field-parity test feeds the same edge inputs
/// through both paths.
///
/// Validates:
/// - `text` non-empty (trimmed).
/// - `source_revision` non-empty (trimmed).
/// - `anchor_url` parses as absolute URL (scheme `http`/`https` + host).
/// - `text_sha256`, when non-empty, equals `sha256(stored text)` — the stored
///   text is the EPUB-projected cache and the hash is its revalidation anchor
///   (R403). Empty `text_sha256` = unrevalidatable (hand-authored or pre-v8,
///   surfaced by `report-excerpt-hash-backfill`); it is accepted as-is.
fn build_normative_excerpt(
    text: &str,
    anchor_url: &str,
    source_revision: &str,
    text_sha256: &str,
) -> Result<NormativeExcerpt, AtomicMutateError> {
    if text.trim().is_empty() {
        return Err(AtomicMutateError::Validation(
            "normative_excerpt text blank or whitespace-only".to_string(),
        ));
    }
    if source_revision.trim().is_empty() {
        return Err(AtomicMutateError::Validation(
            "normative_excerpt source_revision blank or whitespace-only".to_string(),
        ));
    }
    // Lightweight URL validation — full RFC 3986 parser is out of scope;
    // require `http://` or `https://` + non-empty host segment.
    let is_url = anchor_url
        .strip_prefix("https://")
        .or_else(|| anchor_url.strip_prefix("http://"))
        .map(|rest| {
            let host_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
            !rest[..host_end].is_empty()
        })
        .unwrap_or(false);
    if !is_url {
        return Err(AtomicMutateError::Validation(format!(
            "normative_excerpt anchor_url `{}` must be an absolute http(s):// URL with a host",
            anchor_url
        )));
    }
    // Trailing newline is trimmed for stable storage; the hash anchors the
    // *stored* string so revalidation re-hashes exactly what is on disk.
    let stored_text = text.trim_end_matches('\n').to_string();
    let hash = text_sha256.trim();
    if !hash.is_empty() {
        let computed = sha256_hex(stored_text.as_bytes());
        if computed != hash {
            return Err(AtomicMutateError::Validation(format!(
                "normative_excerpt text_sha256 mismatch: declared `{hash}` != sha256(text) `{computed}` — the cached text does not match its EPUB-extracted hash"
            )));
        }
    }
    Ok(NormativeExcerpt {
        text: stored_text,
        anchor_url: anchor_url.to_string(),
        source_revision: source_revision.to_string(),
        text_sha256: hash.to_string(),
    })
}

pub fn add_section_binding(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    section_id: &str,
    file: &str,
    symbol: Option<&str>,
    kind: BindingKind,
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
    let file_clean = validate_binding_file(file)?;
    let symbol_clean = match symbol {
        Some(s) => Some(validate_binding_symbol(s)?),
        None => None,
    };
    let candidate = Binding {
        file: file_clean,
        symbol: symbol_clean,
        kind,
    };
    let section = section_mut_strict(store, section_id)?;
    // Identity is the (file, symbol) pair; `kind` is a mutable attribute
    // (set_section_binding_kind), so a duplicate is detected on identity
    // regardless of kind.
    if section
        .bindings
        .iter()
        .any(|b| b.file == candidate.file && b.symbol == candidate.symbol)
    {
        return Err(AtomicMutateError::Validation(format!(
 "binding `{}{}` already present on §{} (set semantics on (file, symbol) — use set_section_binding_kind to change its kind)",
 candidate.file,
 candidate
 .symbol
 .as_deref()
 .map(|s| format!(":{}", s))
 .unwrap_or_default(),
 section_id,
 )));
    }
    section.bindings.push(candidate);
    save_with_receipt(
        store,
        sidecar_path,
        "add_section_binding",
        "section",
        section_id,
    )
}

/// Round 283 — remove one (file, symbol?) implementation binding from a
/// Section.
///
/// Section.bindings carries current-truth semantics (R259
/// bidirectional binding + R269 ImplementationMissing axiom), so stale
/// rows from code refactor / citation cleanup must be removable. The
/// long-term audit trail lives in (a) the `reason` recorded on the
/// receipt, and (b) the sidecar JSON's git history — Mnemosyne's
/// standard "atomic store = current state, git = history" stance.
///
/// Matching is exact on the `(file, symbol)` pair — set element
/// identity. Pass `Some(symbol)` to remove a symbol-narrowed binding;
/// pass `None` to remove a file-only binding. `file` 가 같아도
/// `symbol` 변종이 다르면 별 row 이며 영향 없음.
///
/// Errors:
/// - `Validation`: `file` shape violation, empty `reason`, or other
/// input validation (mirrors `add_section_binding`).
/// - `NotFound`: `section_id` absent, or the `(file, symbol)` tuple
/// is not registered on the section (no silent no-op — caller asked
/// to remove a specific binding).
///
/// Symmetric with [`add_section_binding`]; `--reason` mandatory
/// mirrors [`remove_section`] (Round 267) and `remove_inventory_entry`
/// (Round 274). Matches on the `(file, symbol)` identity pair regardless of
/// `kind`.
pub fn remove_section_binding(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    section_id: &str,
    file: &str,
    symbol: Option<&str>,
    reason: &str,
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
    if reason.trim().is_empty() {
        return Err(AtomicMutateError::Validation(
            "remove_section_binding: --reason mandatory (audit-trail safeguard)".to_string(),
        ));
    }
    let file_clean = validate_binding_file(file)?;
    let symbol_clean = match symbol {
        Some(s) => Some(validate_binding_symbol(s)?),
        None => None,
    };
    let section = match store.sections.get_mut(section_id) {
        Some(s) => s,
        None => {
            return Err(AtomicMutateError::NotFound(format!(
                "section_id `{}` not present in atomic store",
                section_id
            )));
        }
    };
    let pos = match section
        .bindings
        .iter()
        .position(|b| b.file == file_clean && b.symbol == symbol_clean)
    {
        Some(p) => p,
        None => {
            return Err(AtomicMutateError::NotFound(format!(
                "binding `{}{}` not registered on §{}",
                file_clean,
                symbol_clean
                    .as_deref()
                    .map(|s| format!(":{}", s))
                    .unwrap_or_default(),
                section_id
            )));
        }
    };
    section.bindings.remove(pos);
    save_with_receipt(
        store,
        sidecar_path,
        "remove_section_binding",
        "section",
        section_id,
    )
}

/// Reclassify the `kind` of an existing binding (R295/R305 pattern: a second
/// write path to the same atomic field). Identity is the `(file, symbol)`
/// pair; the binding must already exist (NotFound otherwise — no silent
/// create). `--reason` mandatory (auditable reclassification, mirrors
/// `remove_section_binding`).
///
/// Invariant parity: this and [`add_section_binding`] are the two write
/// paths to `Binding.kind`. `BindingKind` is a closed enum, so both accept
/// exactly the same kind set by construction; the parity test in this crate
/// pins that they never diverge (CLAUDE.md half-enforced-invariant rule).
pub fn set_section_binding_kind(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    section_id: &str,
    file: &str,
    symbol: Option<&str>,
    kind: BindingKind,
    reason: &str,
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
    if reason.trim().is_empty() {
        return Err(AtomicMutateError::Validation(
            "set_section_binding_kind: --reason mandatory (audit-trail safeguard)".to_string(),
        ));
    }
    let file_clean = validate_binding_file(file)?;
    let symbol_clean = match symbol {
        Some(s) => Some(validate_binding_symbol(s)?),
        None => None,
    };
    let section = match store.sections.get_mut(section_id) {
        Some(s) => s,
        None => {
            return Err(AtomicMutateError::NotFound(format!(
                "section_id `{}` not present in atomic store",
                section_id
            )));
        }
    };
    let binding = match section
        .bindings
        .iter_mut()
        .find(|b| b.file == file_clean && b.symbol == symbol_clean)
    {
        Some(b) => b,
        None => {
            return Err(AtomicMutateError::NotFound(format!(
                "binding `{}{}` not registered on §{}",
                file_clean,
                symbol_clean
                    .as_deref()
                    .map(|s| format!(":{}", s))
                    .unwrap_or_default(),
                section_id
            )));
        }
    };
    binding.kind = kind;
    save_with_receipt(
        store,
        sidecar_path,
        "set_section_binding_kind",
        "section",
        section_id,
    )
}

/// Set a Section's coverage applicability (`Normative` | `Informative`) in
/// place. `Informative` exempts the section from the Round 269 coverage axiom
/// (terminology / overview / references — prose-only, nothing to implement
/// here); `Normative` (the default) keeps the axiom. `reason` is a mandatory
/// audit-trail safeguard, mirroring [`set_section_binding_kind`].
///
/// Invariant parity: this and [`import_sections`] are the two write paths to
/// `AtomicSection.coverage_expectation`. `CoverageExpectation` is a closed
/// enum, so both accept exactly the same value set by construction; the parity
/// test in this crate pins that they never diverge (CLAUDE.md
/// half-enforced-invariant rule).
pub fn set_section_coverage_expectation(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    section_id: &str,
    expectation: mnemosyne_core::CoverageExpectation,
    reason: &str,
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
    if reason.trim().is_empty() {
        return Err(AtomicMutateError::Validation(
            "set_section_coverage_expectation: --reason mandatory (audit-trail safeguard)"
                .to_string(),
        ));
    }
    let section = match store.sections.get_mut(section_id) {
        Some(s) => s,
        None => {
            return Err(AtomicMutateError::NotFound(format!(
                "section_id `{}` not present in atomic store",
                section_id
            )));
        }
    };
    section.coverage_expectation = expectation;
    save_with_receipt(
        store,
        sidecar_path,
        "set_section_coverage_expectation",
        "section",
        section_id,
    )
}

/// Classify a section's verification expectation (`Dedicated` | `ByConstruction`,
/// R413). Mirrors [`set_section_coverage_expectation`]: `reason` mandatory
/// (audit-trail safeguard), section must exist. Orthogonal to the coverage
/// axis — a `ByConstruction` section stays `Normative` for implements-coverage.
pub fn set_section_verification_expectation(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    section_id: &str,
    expectation: mnemosyne_core::VerificationExpectation,
    reason: &str,
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
    if reason.trim().is_empty() {
        return Err(AtomicMutateError::Validation(
            "set_section_verification_expectation: --reason mandatory (audit-trail safeguard)"
                .to_string(),
        ));
    }
    let section = match store.sections.get_mut(section_id) {
        Some(s) => s,
        None => {
            return Err(AtomicMutateError::NotFound(format!(
                "section_id `{}` not present in atomic store",
                section_id
            )));
        }
    };
    section.verification_expectation = expectation;
    save_with_receipt(
        store,
        sidecar_path,
        "set_section_verification_expectation",
        "section",
        section_id,
    )
}

/// R393 — bulk-set EPUB-SSOT locators from a medium-forge `epub-anchor-map/v1`.
/// Each `(section_id, locator)` whose section exists has its `epub_locator`
/// overwritten; ids absent from the store are returned as `unmatched` (the
/// caller decides whether that is an error). One in-memory pass + one save
/// (single write path, like `import_sections`) — not N saves. The locator is a
/// derived pointer, so overwrite is allowed (no frozen-ledger gate).
pub fn import_epub_anchors(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    anchors: &[(String, EpubLocator)],
) -> Result<(AtomicMutateReceipt, Vec<String>), AtomicMutateError> {
    let mut applied = 0usize;
    let mut unmatched = Vec::new();
    for (section_id, locator) in anchors {
        match store.sections.get_mut(section_id) {
            Some(section) => {
                section.epub_locator = Some(locator.clone());
                applied += 1;
            }
            None => unmatched.push(section_id.clone()),
        }
    }
    if applied == 0 {
        return Err(AtomicMutateError::NotFound(
            "import_epub_anchors: no anchor matched a section in the store".to_string(),
        ));
    }
    let receipt = save_with_receipt(
        store,
        sidecar_path,
        "import_epub_anchors",
        "anchors",
        &applied.to_string(),
    )?;
    Ok((receipt, unmatched))
}

/// One entry for the bulk [`import_epub_excerpts`] primitive: the EPUB-extracted
/// text cache + its SHA-256 for a Section, as emitted per-anchor by medium-forge
/// `epub-anchor-map/v2`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExcerptImport {
    pub section_id: String,
    pub text: String,
    pub text_sha256: String,
}

/// R403 — refresh the EPUB-projected `normative_excerpt.text` cache (+ its
/// `text_sha256`) from a medium-forge `epub-anchor-map/v2`.
///
/// The upstream identity fields (`anchor_url`, `source_revision`) are store-side
/// authored metadata, NOT projected from the EPUB, so they are preserved from
/// the existing excerpt; only the content cache + hash refresh. Consequently a
/// Section must already carry a `normative_excerpt` (authored via
/// [`import_sections`] inline) for its text to be refreshable — the EPUB map has
/// no anchor_url to construct one from scratch. Sections absent from the store
/// OR lacking an existing excerpt are returned as `unmatched` (the caller
/// decides whether that is an error).
///
/// Each refresh routes through [`build_normative_excerpt`] — the SAME validator
/// as the [`import_sections`] inline path — which verifies
/// `sha256(text) == text_sha256` (CLAUDE.md half-enforced-invariant rule; a
/// field-parity test pins it). One in-memory pass + one save (single write path,
/// like [`import_epub_anchors`]). The text cache is derived, so overwrite is
/// allowed (the frozen-ledger gate was removed in R403). A hash mismatch on any
/// entry returns `Err` before the save, so `run_atomic_mutate` never persists a
/// partially-applied import.
pub fn import_epub_excerpts(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    excerpts: &[ExcerptImport],
) -> Result<(AtomicMutateReceipt, Vec<String>), AtomicMutateError> {
    let mut applied = 0usize;
    let mut unmatched = Vec::new();
    for e in excerpts {
        // Read the authored identity off the existing excerpt (owned clone so
        // the immutable borrow ends before the mutable get_mut below).
        let prev_meta = store
            .sections
            .get(&e.section_id)
            .and_then(|s| s.normative_excerpt.as_ref())
            .map(|ne| (ne.anchor_url.clone(), ne.source_revision.clone()));
        let Some((anchor_url, source_revision)) = prev_meta else {
            unmatched.push(e.section_id.clone());
            continue;
        };
        let refreshed =
            build_normative_excerpt(&e.text, &anchor_url, &source_revision, &e.text_sha256)?;
        store
            .sections
            .get_mut(&e.section_id)
            .expect("section present: prev_meta resolved from it")
            .normative_excerpt = Some(refreshed);
        applied += 1;
    }
    if applied == 0 {
        return Err(AtomicMutateError::NotFound(
            "import_epub_excerpts: no entry matched a refreshable excerpt in the store".to_string(),
        ));
    }
    let receipt = save_with_receipt(
        store,
        sidecar_path,
        "import_epub_excerpts",
        "excerpts",
        &applied.to_string(),
    )?;
    Ok((receipt, unmatched))
}

fn validate_binding_file(raw: &str) -> Result<String, AtomicMutateError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AtomicMutateError::Validation(
            "binding file: must be non-empty".to_string(),
        ));
    }
    if trimmed != raw {
        return Err(AtomicMutateError::Validation(format!(
            "binding file: leading or trailing whitespace not allowed (`{}`)",
            raw
        )));
    }
    if trimmed.starts_with('/') {
        return Err(AtomicMutateError::Validation(format!(
            "binding file: must be workspace-relative (no leading `/`): `{}`",
            trimmed
        )));
    }
    if trimmed.starts_with("./") {
        return Err(AtomicMutateError::Validation(format!(
            "binding file: drop leading `./` for canonical form (`{}`)",
            trimmed
        )));
    }
    if trimmed.contains('\\') {
        return Err(AtomicMutateError::Validation(format!(
            "binding file: backslash not allowed (workspace paths are POSIX): `{}`",
            trimmed
        )));
    }
    if trimmed.contains("//") {
        return Err(AtomicMutateError::Validation(format!(
            "binding file: collapse internal `//` (`{}`)",
            trimmed
        )));
    }
    if trimmed.ends_with('/') {
        return Err(AtomicMutateError::Validation(format!(
            "binding file: trailing `/` not allowed (must point at a file, not a dir): `{}`",
            trimmed
        )));
    }
    for seg in trimmed.split('/') {
        if seg == ".." {
            return Err(AtomicMutateError::Validation(format!(
                "binding file: `..` segment not allowed (no traversal in normalized paths): `{}`",
                trimmed
            )));
        }
    }
    Ok(trimmed.to_string())
}

fn validate_binding_symbol(raw: &str) -> Result<String, AtomicMutateError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AtomicMutateError::Validation(
 "binding symbol: must be non-empty when supplied (omit the field for file-level binding)".to_string(),
 ));
    }
    if trimmed != raw {
        return Err(AtomicMutateError::Validation(format!(
            "binding symbol: leading or trailing whitespace not allowed (`{}`)",
            raw
        )));
    }
    if trimmed.contains('\n') || trimmed.contains('\r') {
        return Err(AtomicMutateError::Validation(format!(
            "binding symbol: newline not allowed (`{:?}`)",
            raw
        )));
    }
    Ok(trimmed.to_string())
}

// ============================================================================
// ChangelogEntry atomic mutate primitive.
// ============================================================================

/// Audit-half fields for a new changelog entry. Bundles the five
/// `AtomicChangelogEntry` audit fields + `entry_id` so the append
/// primitive takes one named struct instead of 8 positional args.
///
/// Named fields close a latent bug class: `changes_bullets`,
/// `verification_bullets`, `impact_refs`, and `carry_forward_bullets` are
/// all `&[String]`, so positional calls could silently transpose them. All
/// fields borrow; [`append_changelog_entry`] clones into the store.
#[derive(Debug, Clone, Copy)]
pub struct ChangelogEntryDraft<'a> {
    /// Strictly-monotonic entry id (e.g. `"Round 316"`).
    pub entry_id: &'a str,
    /// One-sentence headline. Required (Round 298 silent-accept gate).
    pub decision_summary: Option<&'a str>,
    /// What concretely changed. ≥ 1 non-blank bullet required.
    pub changes_bullets: &'a [String],
    /// How the change was verified. ≥ 1 non-blank bullet required.
    pub verification_bullets: &'a [String],
    /// Affected section ids (no `§` prefix).
    pub impact_refs: &'a [String],
    /// Carry-forward items for the next round.
    pub carry_forward_bullets: &'a [String],
}

/// `append_changelog_entry` primitive — atomic-aware changelog append.
///
/// Frozen ledger semantics: once committed,
/// existing fields cannot be modified or removed (T2 jaccard); subsequent
/// mutations to the same `entry_id` are rejected via FrozenLedger error.
///
/// `entry_id_prefix` is the workspace's configured
/// `schema.entry_id_prefix`. A non-empty prefix gates the append:
/// `entry_id` must start with it. An empty prefix disables the gate
/// (generic-preset semantics — numeric entry_id capture is off there
/// too). Round 424.
pub fn append_changelog_entry(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    draft: ChangelogEntryDraft<'_>,
    entry_id_prefix: &str,
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
    let ChangelogEntryDraft {
        entry_id,
        decision_summary,
        changes_bullets,
        verification_bullets,
        impact_refs,
        carry_forward_bullets,
    } = draft;
    if entry_id.trim().is_empty() {
        return Err(AtomicMutateError::Validation(
            "entry_id blank (Round 298 silent-accept gate)".to_string(),
        ));
    }
    if store.changelog_entries.contains_key(entry_id) {
        return Err(AtomicMutateError::FrozenLedger(format!(
            "entry_id `{}` already exists in atomic store; mutations to existing \
  entries are forbidden (Round 161 §41 frozen ledger)",
            entry_id
        )));
    }
    // Round 424 — entry_id_prefix conformance gate at the primitive
    // boundary (the same shared CLI / MCP enforcement surface as the
    // Round 298 gates). Runs after the FrozenLedger duplicate check so
    // frozen reject wins; existing non-conforming entries stay frozen
    // history, only NEW appends are gated.
    if !entry_id_prefix.is_empty() && !entry_id.starts_with(entry_id_prefix) {
        return Err(AtomicMutateError::Validation(format!(
            "entry_id `{}` does not start with configured schema.entry_id_prefix \
  `{}` (Round 424 conformance gate; set entry_id_prefix = \"\" to disable)",
            entry_id, entry_id_prefix
        )));
    }
    // Round 298 — required-field gate at the primitive boundary so CLI / MCC
    // / future wires share the same enforcement surface. Frozen-ledger reject
    // wins over field validation (existing FrozenLedger test passes empty
    // body intentionally).
    check_changelog_entry_required(
        decision_summary,
        changes_bullets,
        verification_bullets,
        impact_refs,
        carry_forward_bullets,
    )?;
    // Round 294 — initialize publishable_* = audit_* clone. The two halves
    // diverge later via R295 publishable setters (paired with the R296
    // [[publishable_override_ledger]] gate). Default-equal at append time so
    // the publishable read-view matches the audit half until an explicit redact.
    let mut entry = AtomicChangelogEntry {
        decision_summary: decision_summary.map(str::to_string),
        changes_bullets: changes_bullets.to_vec(),
        verification_bullets: verification_bullets.to_vec(),
        impact_refs: impact_refs.to_vec(),
        carry_forward_bullets: carry_forward_bullets.to_vec(),
        ..Default::default()
    };
    entry.clone_audit_into_publishable();
    store.changelog_entries.insert(entry_id.to_string(), entry);
    save_with_receipt(
        store,
        sidecar_path,
        "append_changelog_entry",
        "changelog_entry",
        entry_id,
    )
}

/// `append_confirmation_event` primitive (R416; R417 derives the id). Append-only
/// confirmation record. The `event_id` is DERIVED deterministically from the
/// verification act ([`derive_confirmation_event_id`]) — callers never supply it,
/// so the CLI and MCP write paths cannot mint inconsistent keys. A re-append of
/// the identical act hashes to the same id and is rejected (idempotent
/// append-only). Enforces the self-confirm reject invariant (design sec 4.7):
/// `confirming_run` must differ from `authoring_run`. The core does NOT verify the
/// artifact hashes or spawn any confirmer — it records the producer's claimed
/// provenance (design sec 4.6); the `confirmed?` predicate and the opt-in gate
/// land later (R418 / R419).
pub fn append_confirmation_event(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    event: ConfirmationEvent,
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
    if event.authoring_run.trim().is_empty() || event.confirming_run.trim().is_empty() {
        return Err(AtomicMutateError::Validation(
            "authoring_run / confirming_run must be non-blank (provenance, sec 4.1)".to_string(),
        ));
    }
    // Self-confirm reject — the cheapest, machine-checkable slice of independence
    // (design sec 4.7). The store cannot prove fresh-ness, but it CAN reject a run
    // confirming its own claim.
    if event.authoring_run == event.confirming_run {
        return Err(AtomicMutateError::Validation(format!(
            "self-confirm rejected: confirming_run `{}` must differ from authoring_run \
             (independence, design sec 4.7)",
            event.confirming_run
        )));
    }
    if event.rationale.trim().is_empty() {
        return Err(AtomicMutateError::Validation(
            "rationale must be non-blank (esp. for Refute, sec 4.1)".to_string(),
        ));
    }
    // R287 fail-loud: a claim about a non-existent section is a silent footgun.
    // Only section existence is checked here; binding existence + the
    // `Verifies` kind are evaluated by the `confirmed?` predicate (R418), which
    // reads the live binding graph at query time rather than freezing it here.
    let claim_section = event.claim.section_id();
    if !store.sections.contains_key(claim_section) {
        return Err(AtomicMutateError::NotFound(format!(
            "claim section_id `{}` not present in atomic store (use add_section first; \
             R287 fail-loud)",
            claim_section
        )));
    }
    let event_id = derive_confirmation_event_id(&event);
    if store.confirmation_events.contains_key(&event_id) {
        return Err(AtomicMutateError::FrozenLedger(format!(
            "confirmation already recorded (identical act, idempotent): `{}`",
            event_id
        )));
    }
    store.confirmation_events.insert(event_id.clone(), event);
    save_with_receipt(
        store,
        sidecar_path,
        "append_confirmation_event",
        "confirmation_event",
        &event_id,
    )
}

/// Deterministic `event_id` for a confirmation event (R417) — the SINGLE source
/// of the id rule, shared by the CLI and MCP write paths so neither can mint an
/// inconsistent key. Hashes the verification *act*: claim + confirmer + method +
/// verdict + both runs + timestamp. `rationale` and `artifact_hashes` are
/// EXCLUDED — they are payload of the act, not its identity. Two identical acts
/// collide (idempotent re-append rejects); distinct independent confirmations
/// (a different `confirming_run` / `timestamp`) get distinct ids and accumulate.
fn derive_confirmation_event_id(event: &ConfirmationEvent) -> String {
    let (kind, section, file, symbol) = match &event.claim {
        ConfirmationClaim::VerifiesBinding {
            section_id,
            file,
            symbol,
        } => (
            "verifies_binding",
            section_id.as_str(),
            file.as_str(),
            symbol.as_deref().unwrap_or(""),
        ),
        ConfirmationClaim::SectionCompleteness { section_id } => {
            ("section_completeness", section_id.as_str(), "", "")
        }
    };
    // Unit-separator join so distinct field tuples can never alias.
    let canonical = [
        kind,
        section,
        file,
        symbol,
        event.confirmer.kind.as_str(),
        event.confirmer.id.as_str(),
        event.method.as_str(),
        event.verdict.as_str(),
        event.authoring_run.as_str(),
        event.confirming_run.as_str(),
        event.timestamp.as_str(),
    ]
    .join("\u{1f}");
    format!("evt-{}", &sha256_hex(canonical.as_bytes())[..16])
}

// ============================================================================
// ChangelogEntry publishable-half setters (Round 295).
//
// The 5 setters below mutate **only** the `publishable_*` half of an
// `AtomicChangelogEntry`. The audit half (`decision_summary`,
// `changes_bullets`, `verification_bullets`, `impact_refs`,
// `carry_forward_bullets`) is untouched — that is the permanent record and
// `append_changelog_entry` is the only path that writes it (with frozen
// ledger reject on second-attempt). After R296 wires
// `[[publishable_override_ledger]]`, calling these setters without an
// accompanying ledger entry will surface as a validate-workspace reject.
//
// `entry_mut_strict` is the fail-loud changelog-entry lookup for the
// publishable setters: they require the entry to exist first (created via
// `append_changelog_entry`) because they cannot author the audit half.
// ============================================================================

fn entry_mut_strict<'a>(
    store: &'a mut AtomicStore,
    entry_id: &str,
) -> Result<&'a mut AtomicChangelogEntry, AtomicMutateError> {
    store.changelog_entries.get_mut(entry_id).ok_or_else(|| {
        AtomicMutateError::NotFound(format!(
            "entry_id `{}` not present in atomic store (use append_changelog_entry \
  to create it first)",
            entry_id
        ))
    })
}

pub fn set_changelog_publishable_decision_summary(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    entry_id: &str,
    summary: &str,
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
    entry_mut_strict(store, entry_id)?.publishable_decision_summary = Some(summary.to_string());
    save_with_receipt(
        store,
        sidecar_path,
        "set_changelog_publishable_decision_summary",
        "changelog_entry",
        entry_id,
    )
}

pub fn set_changelog_publishable_changes_bullets(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    entry_id: &str,
    bullets: &[String],
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
    entry_mut_strict(store, entry_id)?.publishable_changes_bullets = bullets.to_vec();
    save_with_receipt(
        store,
        sidecar_path,
        "set_changelog_publishable_changes_bullets",
        "changelog_entry",
        entry_id,
    )
}

pub fn set_changelog_publishable_verification_bullets(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    entry_id: &str,
    bullets: &[String],
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
    entry_mut_strict(store, entry_id)?.publishable_verification_bullets = bullets.to_vec();
    save_with_receipt(
        store,
        sidecar_path,
        "set_changelog_publishable_verification_bullets",
        "changelog_entry",
        entry_id,
    )
}

pub fn set_changelog_publishable_impact_refs(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    entry_id: &str,
    refs: &[String],
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
    entry_mut_strict(store, entry_id)?.publishable_impact_refs = refs.to_vec();
    save_with_receipt(
        store,
        sidecar_path,
        "set_changelog_publishable_impact_refs",
        "changelog_entry",
        entry_id,
    )
}

pub fn set_changelog_publishable_carry_forward_bullets(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    entry_id: &str,
    bullets: &[String],
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
    entry_mut_strict(store, entry_id)?.publishable_carry_forward_bullets = bullets.to_vec();
    save_with_receipt(
        store,
        sidecar_path,
        "set_changelog_publishable_carry_forward_bullets",
        "changelog_entry",
        entry_id,
    )
}

/// Round 300 — emit a `[[publishable_override_ledger]]` block for a single
/// entry whose publishable half currently diverges from the audit half.
/// Read-only: never mutates the store. Returns `Ok(None)` when the entry
/// is in sync (nothing to anchor).
///
/// Mirrors the inline ledger-draft block that `redact_term` produces, but
/// for callers that authored their divergence via the bare R295
/// publishable setters and now need a draft to paste into
/// `mnemosyne.toml`. The hash matches what `validate-workspace` will
/// compute against the post-mutation publishable half, so the resulting
/// ledger row clears the R296 gate without manual SHA256 work.
pub fn emit_publishable_override_ledger_draft(
    store: &AtomicStore,
    entry_id: &str,
    reason: &str,
    applied_in: &str,
    kind: &str,
) -> Result<Option<String>, AtomicMutateError> {
    let entry = store.changelog_entries.get(entry_id).ok_or_else(|| {
        AtomicMutateError::NotFound(format!(
            "entry_id `{}` not present in atomic store",
            entry_id
        ))
    })?;
    if entry.publishable_matches_audit() {
        return Ok(None);
    }
    let fields: Vec<String> = entry
        .divergent_publishable_fields()
        .into_iter()
        .map(|f| f.to_string())
        .collect();
    let before = entry.audit_hash_hex();
    let after = entry.publishable_hash_hex();
    Ok(Some(crate::redact::format_ledger_row(
        kind, entry_id, &fields, reason, applied_in, &before, &after,
    )))
}

// ============================================================================
// Inventory atomic mutate primitives (Round 274, Phase 1A).
// ============================================================================

/// Validate an inventory ID against the trust-boundary shape rules:
/// non-empty after trim, no surrounding whitespace, no internal whitespace,
/// no embedded newlines. Returns the (already-known-canonical) input on
/// success — callers should pass the trimmed form they intend to store.
fn validate_inventory_id(raw: &str) -> Result<&str, AtomicMutateError> {
    if raw.trim().is_empty() {
        return Err(AtomicMutateError::Validation(
            "inventory_id: must be non-empty".to_string(),
        ));
    }
    if raw.trim() != raw {
        return Err(AtomicMutateError::Validation(format!(
            "inventory_id: leading or trailing whitespace not allowed (`{}`)",
            raw
        )));
    }
    if raw.chars().any(char::is_whitespace) {
        return Err(AtomicMutateError::Validation(format!(
            "inventory_id: internal whitespace not allowed (`{}`)",
            raw
        )));
    }
    Ok(raw)
}

/// Validate a `section_ref` value: strip nothing (callers pass canonical
/// form already — no leading `§`, no whitespace edges). The CLI surface
/// performs the `§` strip before reaching this layer.
fn validate_section_ref_input(raw: &str) -> Result<&str, AtomicMutateError> {
    if raw.trim().is_empty() {
        return Err(AtomicMutateError::Validation(
            "section_ref: must be non-empty (pass None to unset)".to_string(),
        ));
    }
    if raw.trim() != raw {
        return Err(AtomicMutateError::Validation(format!(
            "section_ref: leading or trailing whitespace not allowed (`{}`)",
            raw
        )));
    }
    if raw.starts_with('§') {
        return Err(AtomicMutateError::Validation(format!(
            "section_ref: drop leading `§` for canonical form (`{}`)",
            raw
        )));
    }
    Ok(raw)
}

/// Register a new inventory entry (Round 274).
///
/// `inventory_id` must be unique within `AtomicStore.inventory_entries` —
/// duplicate registration returns `Validation` error (fail-loud; explicit
/// register-then-update is cleaner than implicit upsert when the genre is
/// stable external IDs). To change an existing entry, use the dedicated
/// `set_inventory_*` primitives.
///
/// `section_ref` / `source` / `reason` are all optional and pass through
/// after a trust-boundary shape check (no whitespace edges, no embedded
/// leading `§` in `section_ref`). Empty strings reject — callers should
/// pass `None` to indicate "no value".
pub fn add_inventory_entry(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    inventory_id: &str,
    status: InventoryStatus,
    section_ref: Option<&str>,
    source: Option<&str>,
    reason: Option<&str>,
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
    validate_inventory_id(inventory_id)?;
    if store.inventory_entries.contains_key(inventory_id) {
        return Err(AtomicMutateError::Validation(format!(
 "inventory_id `{}` already registered (use set_inventory_status / set_inventory_section_ref / remove_inventory_entry to mutate)",
 inventory_id
 )));
    }
    let section_ref_clean = match section_ref {
        Some(s) => Some(validate_section_ref_input(s)?.to_string()),
        None => None,
    };
    let source_clean = match source {
        Some(s) if !s.trim().is_empty() => Some(s.to_string()),
        _ => None,
    };
    let reason_clean = match reason {
        Some(s) if !s.trim().is_empty() => Some(s.to_string()),
        _ => None,
    };
    let entry = InventoryEntry {
        status,
        section_ref: section_ref_clean,
        source: source_clean,
        reason: reason_clean,
    };
    store
        .inventory_entries
        .insert(inventory_id.to_string(), entry);
    save_with_receipt(
        store,
        sidecar_path,
        "add_inventory_entry",
        "inventory_entry",
        inventory_id,
    )
}

/// Update an inventory entry's `status` (Round 274).
///
/// Returns `NotFound` when the ID is not registered (no silent create —
/// status mutation on a non-existent entry is a caller mistake worth
/// surfacing). `reason` is `Option<&str>`: `None` leaves the existing
/// reason untouched, `Some("")` clears it, `Some(non_empty)` overwrites.
///
/// All transitions accepted at this layer (Active ↔ Deprecated ↔ Reserved).
/// Cite-time reject semantics (Deprecated triggers reject) is the validator
/// inventory axis's responsibility (Round 275 carry). Cascade-on-status-
/// transition is Round 276 carry.
pub fn set_inventory_status(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    inventory_id: &str,
    new_status: InventoryStatus,
    reason: Option<&str>,
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
    validate_inventory_id(inventory_id)?;
    let entry = store
        .inventory_entries
        .get_mut(inventory_id)
        .ok_or_else(|| {
            AtomicMutateError::NotFound(format!(
                "inventory_id `{}` not present in atomic store",
                inventory_id
            ))
        })?;
    entry.status = new_status;
    if let Some(r) = reason {
        entry.reason = if r.trim().is_empty() {
            None
        } else {
            Some(r.to_string())
        };
    }
    save_with_receipt(
        store,
        sidecar_path,
        "set_inventory_status",
        "inventory_entry",
        inventory_id,
    )
}

/// Update an inventory entry's `section_ref` (Round 274).
///
/// Returns `NotFound` when the ID is not registered. `section_ref` is
/// `Option<&str>`: `None` unsets the binding, `Some(non_empty)` overwrites
/// after a shape check (leading `§` rejected — callers strip before this
/// layer, matching the existing CLI convention).
///
/// No referential-integrity check (`section_ref` points at a section_id
/// that actually exists) — that's `validate-workspace`'s job, not the
/// atomic primitive's. Mirrors the `set_section_decision_status`
/// stance on `superseding` (no cross-store existence enforcement here).
pub fn set_inventory_section_ref(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    inventory_id: &str,
    section_ref: Option<&str>,
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
    validate_inventory_id(inventory_id)?;
    let cleaned = match section_ref {
        Some(s) => Some(validate_section_ref_input(s)?.to_string()),
        None => None,
    };
    let entry = store
        .inventory_entries
        .get_mut(inventory_id)
        .ok_or_else(|| {
            AtomicMutateError::NotFound(format!(
                "inventory_id `{}` not present in atomic store",
                inventory_id
            ))
        })?;
    entry.section_ref = cleaned;
    save_with_receipt(
        store,
        sidecar_path,
        "set_inventory_section_ref",
        "inventory_entry",
        inventory_id,
    )
}

/// Remove an inventory entry (Round 274).
///
/// `reason` mandatory — audit-trail safeguard mirroring `remove_section`
/// (Round 267). Returns `NotFound` when the ID is absent. The receipt
/// records the primitive name and target_id; the reason itself lands in
/// the git history of the sidecar JSON (the atomic store *is* the audit
/// trail, so the diff carries the human-readable explanation).
pub fn remove_inventory_entry(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    inventory_id: &str,
    reason: &str,
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
    validate_inventory_id(inventory_id)?;
    if reason.trim().is_empty() {
        return Err(AtomicMutateError::Validation(
            "remove_inventory_entry: --reason mandatory (audit-trail safeguard)".to_string(),
        ));
    }
    if store.inventory_entries.remove(inventory_id).is_none() {
        return Err(AtomicMutateError::NotFound(format!(
            "inventory_id `{}` not present in atomic store",
            inventory_id
        )));
    }
    save_with_receipt(
        store,
        sidecar_path,
        "remove_inventory_entry",
        "inventory_entry",
        inventory_id,
    )
}

// ============================================================================
// Narrative fact mutate primitives (Phase 1A, Round 430).
//
// Multi-axis perspectival facts (ARCHITECTURE.md sec 1.1): a claim held in
// exactly one epistemic frame over a canon-time extent. Append-only genre:
// belief change = a successor fact carrying `supersedes_in_frame`, never an
// edit of the predecessor. BOTH write paths (`add_fact` / `import_facts`)
// route `build_candidate_fact` — the field-invariant-parity rule (R305): one
// closed invariant set, no half-enforced second path.
// ============================================================================

/// One frame entry in the [`FactsManifest`] (and the `add_frame` shape).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameImport {
    pub frame_id: String,
    #[serde(default)]
    pub description: String,
}

/// One branch entry in the [`FactsManifest`] (and the `add_branch` shape,
/// Round 436).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchImport {
    pub branch_id: String,
    #[serde(default)]
    pub description: String,
    /// Parent world-line (Round 438). Both fork fields or neither; the
    /// parent must already be registered (earlier in this manifest is fine
    /// — branches land sequentially, parents first).
    #[serde(default)]
    pub forks_from: Option<String>,
    /// Canon point of divergence (structure-section ref).
    #[serde(default)]
    pub forks_at: Option<String>,
    /// Incoming world-line merges (Round 532 — convergence / confluence).
    /// Each entry is `{branch, at}` (a parent + its merge coordinate); a
    /// confluence has ≥ 2. Mutually exclusive with `forks_from`/`forks_at`.
    /// Parents must already be registered (earlier in this manifest).
    #[serde(default)]
    pub converges_from: Vec<BranchConvergeImport>,
}

/// One incoming-merge edge in the import manifest (Round 532) — the authoring
/// face of a confluence parent edge: the parent world-line + the parent's
/// merge coordinate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchConvergeImport {
    pub branch: String,
    pub at: String,
}

/// One fact entry in the import manifest — the authoring face of
/// [`NarrativeFact`] plus its map key. `quote_sha256` is deliberately NOT
/// accepted from the caller: the primitive computes it from `quote` at write
/// time, so a stored hash can never start out wrong (offline drift detection
/// then owns divergence — the R404 content-drift pattern).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactImport {
    pub fact_id: String,
    pub frame: String,
    /// World-line branch (Round 433). Omitted = `MAIN_BRANCH`; present must
    /// be non-empty after trim (omit it instead of blanking it).
    #[serde(default)]
    pub branch: Option<String>,
    /// Entity refs (Round 437). Each must name a registered entity.
    #[serde(default)]
    pub entities: Vec<String>,
    pub claim: String,
    pub canon_from: String,
    #[serde(default)]
    pub canon_to: Option<String>,
    #[serde(default)]
    pub evidence: Vec<String>,
    #[serde(default)]
    pub conflicts_with: Vec<String>,
    #[serde(default)]
    pub supersedes_in_frame: Option<String>,
    /// Setup marking (Round 442): the canonical lowercase
    /// [`PayoffExpectation`] tag (`expected`); omitted = `unmarked`.
    /// Unknown tags reject (fail-loud, no silent default).
    #[serde(default)]
    pub payoff_expectation: Option<String>,
    /// Setup fact ids this fact pays off (Round 442). Identity refs,
    /// unpinned (the R439 pin covers claim-text judgments only).
    #[serde(default)]
    pub pays_off: Vec<String>,
    /// Optional typed leg (Round 446): subject–predicate–object reading of
    /// the prose claim, validated against the predicate/entity registries
    /// by the shared builder.
    #[serde(default)]
    pub typed: Option<TypedClaim>,
    #[serde(default)]
    pub quote: Option<String>,
}

/// One entity entry in the [`FactsManifest`] (and the `add_entity` shape,
/// Round 437).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityImport {
    pub entity_id: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub description: String,
}

/// One predicate entry in the [`FactsManifest`] (and the `add_predicate`
/// shape, Round 446). `object_kind` is the canonical lowercase tag
/// (`entity` | `scalar`) — unknown tags reject (fail-loud, no silent
/// default).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredicateImport {
    pub predicate_id: String,
    pub object_kind: String,
    #[serde(default)]
    pub description: String,
}

/// One diegetic surface in a [`DisclosureOverrideImport`] (Round 590) — the
/// flat manifest form of [`DisclosureSurface`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisclosureSurfaceImport {
    pub scene: String,
    #[serde(default)]
    pub object: Option<String>,
}

/// One per-fact disclosure override in a [`DisclosurePlanImport`] (Round 590) —
/// the manifest form of a `set-disclosure` decision. Applied through the SAME
/// [`apply_disclosure_override`] the standalone setter uses (write-path parity).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisclosureOverrideImport {
    pub fact_id: String,
    /// Disclosure mode tag (`withhold`/`state`/`hint`/`imply`); parsed fail-loud.
    pub mode: String,
    /// Per-world-line `first_at` pins as `[branch, coord]` pairs.
    #[serde(default)]
    pub first_at: Vec<[String; 2]>,
    #[serde(default)]
    pub surface: Option<DisclosureSurfaceImport>,
}

/// One disclosure plan (telling) in the [`FactsManifest`] (Round 590) — the plan
/// policy plus its per-fact overrides. Applied AFTER facts (registries → facts →
/// disclosure), so an override can reference a same-manifest fact and satisfy
/// the typed-fact invariant. Uses the SAME `apply_disclosure_plan` /
/// `apply_disclosure_override` cores as the standalone primitives.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisclosurePlanImport {
    pub telling_id: String,
    /// Default disclosure mode tag; omitted = `withhold` (the plan default).
    #[serde(default)]
    pub default_mode: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub overrides: Vec<DisclosureOverrideImport>,
}

/// `import-facts` manifest (Round 590 — the all-primitive form): frames +
/// branches + entities + predicates + facts + disclosure plans created in ONE
/// atomic transaction. Ordered so later kinds reference earlier ones —
/// registries first, then facts, then disclosure (whose overrides reference the
/// facts). Quests need no dedicated kind: a quest is an `Entity{kind:"quest"}`
/// (an `entities` entry) plus `pursues`/`requires`/`completed_by` typed
/// `facts`, so it is authored through the existing kinds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactsManifest {
    #[serde(default)]
    pub frames: Vec<FrameImport>,
    #[serde(default)]
    pub branches: Vec<BranchImport>,
    #[serde(default)]
    pub entities: Vec<EntityImport>,
    #[serde(default)]
    pub predicates: Vec<PredicateImport>,
    #[serde(default)]
    pub facts: Vec<FactImport>,
    #[serde(default)]
    pub disclosure_plans: Vec<DisclosurePlanImport>,
}

/// The one-line human description of a [`FactsManifest`]'s shape (Round 592) —
/// single-sourced so the CLI/MCP parse hints cannot drift apart (they had, at
/// R590: some still said "frames + facts", one omitted `disclosure_plans`).
pub const FACTS_MANIFEST_SHAPE: &str =
    "a JSON object with frames / branches / entities / predicates / facts / disclosure_plans arrays";

/// Single shared fact builder/validator — both write paths route here
/// (R305 parity). Enforces the scalar invariants:
/// - `fact_id` / `frame` / `claim` / `canon_from` non-empty after trim;
/// - `frame` must exist in the frames registry (fail-loud, R287 lesson);
/// - `branch`, when present and not `MAIN_BRANCH`, must exist in the branch
///   registry (Round 436 — frames-registry symmetry);
/// - every `entities` ref must exist in the entity registry, no blanks, no
///   duplicates (Round 437);
/// - `canon_from` / `canon_to` / every `evidence` ref must name an existing
///   section (canon coordinates ARE structure refs, design sec 7.3/7.5);
/// - `evidence` >= 1 (a claim without provenance is unauditable);
/// - no self-reference in `conflicts_with` / `supersedes_in_frame` /
///   `pays_off`, no duplicate `pays_off` refs, `payoff_expectation` tag
///   must parse (Round 442);
/// - `quote_sha256` computed here, never caller-supplied.
///
/// Cross-FACT refs (`conflicts_with` / `supersedes_in_frame` / `pays_off`
/// targets) are
/// validated by the caller against its own visibility set — store ∪ manifest
/// for `import_facts` (forward refs within one manifest are legal), store
/// for `add_fact`.
fn build_candidate_fact(
    store: &AtomicStore,
    entry: &FactImport,
) -> Result<(String, NarrativeFact), String> {
    let fact_id = entry.fact_id.trim();
    if fact_id.is_empty() {
        return Err("fact_id mandatory (non-empty after trim)".to_string());
    }
    let frame = entry.frame.trim();
    if frame.is_empty() {
        return Err(format!("fact `{fact_id}`: frame mandatory (non-empty)"));
    }
    if !store.frames.contains_key(frame) {
        return Err(format!(
            "fact `{fact_id}`: frame `{frame}` not present in the frames registry \
             (add_frame / manifest frames[] first; fail-loud)"
        ));
    }
    let branch = match entry.branch.as_deref().map(str::trim) {
        None => mnemosyne_core::MAIN_BRANCH.to_string(),
        Some("") => {
            return Err(format!(
                "fact `{fact_id}`: branch must be non-empty when present (omit it for `{}`)",
                mnemosyne_core::MAIN_BRANCH
            ));
        }
        Some(b) => {
            if !mnemosyne_core::is_known_world(&store.branches, b) {
                return Err(format!(
                    "fact `{fact_id}`: branch `{b}` not present in the branch registry \
                     (add_branch / manifest branches[] first; fail-loud — a typo'd branch \
                     must not silently create a world)"
                ));
            }
            b.to_string()
        }
    };
    let mut entities = Vec::with_capacity(entry.entities.len());
    for e in &entry.entities {
        let e = e.trim();
        if e.is_empty() {
            return Err(format!("fact `{fact_id}`: blank entity ref"));
        }
        if !store.entities.contains_key(e) {
            return Err(format!(
                "fact `{fact_id}`: entity `{e}` not present in the entity registry \
                 (add_entity / manifest entities[] first; fail-loud)"
            ));
        }
        if entities.iter().any(|x| x == e) {
            return Err(format!("fact `{fact_id}`: duplicate entity ref `{e}`"));
        }
        entities.push(e.to_string());
    }
    let claim = entry.claim.trim();
    if claim.is_empty() {
        return Err(format!("fact `{fact_id}`: claim mandatory (non-empty)"));
    }
    let canon_from = entry.canon_from.trim();
    if canon_from.is_empty() {
        return Err(format!(
            "fact `{fact_id}`: canon_from mandatory (non-empty)"
        ));
    }
    if !store.sections.contains_key(canon_from) {
        return Err(format!(
            "fact `{fact_id}`: canon_from `{canon_from}` not present as a section \
             (canon coordinates are structure-section refs)"
        ));
    }
    let canon_to = match entry.canon_to.as_deref().map(str::trim) {
        None => None,
        Some("") => {
            return Err(format!(
                "fact `{fact_id}`: canon_to must be non-empty when present (omit it instead)"
            ));
        }
        Some(c) => {
            if !store.sections.contains_key(c) {
                return Err(format!(
                    "fact `{fact_id}`: canon_to `{c}` not present as a section"
                ));
            }
            Some(c.to_string())
        }
    };
    if entry.evidence.is_empty() {
        return Err(format!(
            "fact `{fact_id}`: evidence mandatory (>= 1 structure-section ref; \
             a claim without provenance is unauditable)"
        ));
    }
    let mut evidence = Vec::with_capacity(entry.evidence.len());
    for e in &entry.evidence {
        let e = e.trim();
        if e.is_empty() {
            return Err(format!("fact `{fact_id}`: blank evidence ref"));
        }
        if !store.sections.contains_key(e) {
            return Err(format!(
                "fact `{fact_id}`: evidence `{e}` not present as a section"
            ));
        }
        evidence.push(e.to_string());
    }
    let mut conflicts_with = Vec::with_capacity(entry.conflicts_with.len());
    for c in &entry.conflicts_with {
        let c = c.trim();
        if c.is_empty() {
            return Err(format!("fact `{fact_id}`: blank conflicts_with ref"));
        }
        if c == fact_id {
            return Err(format!(
                "fact `{fact_id}`: conflicts_with itself — a fact cannot contradict itself"
            ));
        }
        // Stamped by `validate_and_stamp_fact_refs` once the target is
        // known to exist; never caller-supplied (R404).
        conflicts_with.push(ConflictAssertion {
            target: c.to_string(),
            target_claim_sha256: String::new(),
        });
    }
    let supersedes_in_frame = match entry.supersedes_in_frame.as_deref().map(str::trim) {
        None => None,
        Some("") => {
            return Err(format!(
                "fact `{fact_id}`: supersedes_in_frame must be non-empty when present"
            ));
        }
        Some(s) if s == fact_id => {
            return Err(format!(
                "fact `{fact_id}`: supersedes_in_frame itself — succession needs a predecessor"
            ));
        }
        Some(s) => Some(s.to_string()),
    };
    let payoff_expectation = match entry.payoff_expectation.as_deref().map(str::trim) {
        None => PayoffExpectation::default(),
        Some(tag) => PayoffExpectation::from_tag(tag).ok_or_else(|| {
            format!(
                "fact `{fact_id}`: unknown payoff_expectation `{tag}` \
                 (expected one of: unmarked, expected)"
            )
        })?,
    };
    let mut pays_off = Vec::with_capacity(entry.pays_off.len());
    for t in &entry.pays_off {
        let t = t.trim();
        if t.is_empty() {
            return Err(format!("fact `{fact_id}`: blank pays_off ref"));
        }
        if t == fact_id {
            return Err(format!(
                "fact `{fact_id}`: pays_off itself — a payoff resolves an earlier setup"
            ));
        }
        if pays_off.iter().any(|x| x == t) {
            return Err(format!("fact `{fact_id}`: duplicate pays_off ref `{t}`"));
        }
        pays_off.push(t.to_string());
    }
    let (quote, quote_sha256) = match entry.quote.as_deref().map(str::trim) {
        None => (None, None),
        Some("") => {
            return Err(format!(
                "fact `{fact_id}`: quote must be non-empty when present (omit it instead)"
            ));
        }
        Some(q) => (Some(q.to_string()), Some(sha256_hex(q.as_bytes()))),
    };
    let typed = match &entry.typed {
        None => None,
        Some(t) => Some(build_typed_claim(store, fact_id, t, &entities)?),
    };
    Ok((
        fact_id.to_string(),
        NarrativeFact {
            frame: frame.to_string(),
            branch,
            entities,
            claim: claim.to_string(),
            canon_from: canon_from.to_string(),
            canon_to,
            evidence,
            conflicts_with,
            supersedes_in_frame,
            payoff_expectation,
            pays_off,
            typed,
            quote,
            quote_sha256,
        },
    ))
}

/// Validate + shape one typed leg against the store (Round 446, design
/// sec 7.12) — the ONE place typed-claim invariants live, shared by both
/// fact write paths via [`build_candidate_fact`] (R305 parity):
/// - subject must be a REGISTERED entity AND a member of the fact's
///   `entities` list (the entities list stays THE retrieval key; a typed
///   leg never silently widens it);
/// - predicate must be a registered predicate id (load-bearing ref —
///   rules key off it);
/// - the object leg's shape must match the predicate's declared
///   `object_kind`; an entity-shaped object obeys the same
///   registered-and-listed rule as the subject; a scalar value must be
///   non-empty (opaque consumer vocabulary, never enumerated here).
fn build_typed_claim(
    store: &AtomicStore,
    fact_id: &str,
    t: &TypedClaim,
    fact_entities: &[String],
) -> Result<TypedClaim, String> {
    let check_entity_leg = |leg: &str, id: &str| -> Result<String, String> {
        let id = id.trim();
        if id.is_empty() {
            return Err(format!(
                "fact `{fact_id}`: typed {leg} mandatory (non-empty)"
            ));
        }
        if !store.entities.contains_key(id) {
            return Err(format!(
                "fact `{fact_id}`: typed {leg} `{id}` not present in the entity registry \
                 (add_entity / manifest entities[] first; fail-loud)"
            ));
        }
        if !fact_entities.iter().any(|e| e == id) {
            return Err(format!(
                "fact `{fact_id}`: typed {leg} `{id}` is not a member of the fact's \
                 entities list — the entities list stays THE retrieval key; list it there too"
            ));
        }
        Ok(id.to_string())
    };
    let subject = check_entity_leg("subject", &t.subject)?;
    let predicate = t.predicate.trim();
    if predicate.is_empty() {
        return Err(format!(
            "fact `{fact_id}`: typed predicate mandatory (non-empty)"
        ));
    }
    let Some(decl) = store.predicates.get(predicate) else {
        return Err(format!(
            "fact `{fact_id}`: typed predicate `{predicate}` not present in the predicate \
             registry (add_predicate / manifest predicates[] first; fail-loud — rules key \
             off predicate ids, a typo must not silently escape its rule)"
        ));
    };
    let object = match (&t.object, decl.object_kind) {
        (TypedObject::Entity { id }, PredicateObjectKind::Entity) => TypedObject::Entity {
            id: check_entity_leg("object", id)?,
        },
        (TypedObject::Value { value }, PredicateObjectKind::Scalar) => {
            let value = value.trim();
            if value.is_empty() {
                return Err(format!(
                    "fact `{fact_id}`: typed object value mandatory (non-empty)"
                ));
            }
            TypedObject::Value {
                value: value.to_string(),
            }
        }
        (TypedObject::Entity { .. }, PredicateObjectKind::Scalar) => {
            return Err(format!(
                "fact `{fact_id}`: predicate `{predicate}` declares object_kind=scalar but \
                 the typed object is an entity — shape mismatch (fix the leg or the declaration)"
            ));
        }
        (TypedObject::Value { .. }, PredicateObjectKind::Entity) => {
            return Err(format!(
                "fact `{fact_id}`: predicate `{predicate}` declares object_kind=entity but \
                 the typed object is a scalar value — shape mismatch (fix the leg or the declaration)"
            ));
        }
    };
    Ok(TypedClaim {
        subject,
        predicate: predicate.to_string(),
        object,
    })
}

/// THE closed invariant set of one succession edge (Round 463 extraction —
/// every write path that can produce `successor --supersedes--> target`
/// routes here: `add_fact` / `import_facts` / `amend_fact` via
/// [`validate_and_stamp_fact_refs`], and [`import_edge_proposals`]
/// directly; the R305/R446 one-builder-site rule applied to edges):
/// the target exists, succession is in-frame (cross-frame disagreement is
/// data, design sec 7.3), the target's branch is one this world-line INHERITS
/// (its own, a fork ancestor, or a confluence it merges into —
/// [`mnemosyne_core::succession_branch_inherits`], Rounds 438 + 535), and the
/// resulting chain is ACYCLIC.
///
/// The cycle walk closes a hole verified live in Round 461: `add_fact` is
/// cycle-safe only by construction (the target must pre-exist), but
/// `amend_fact` retargeting and `import_facts` forward refs could both
/// close an A⇄B loop with exit 0 — after which the cycle's facts silently
/// never hold anywhere (each derives the other's end). `staged_edges` is
/// the outbound overlay for edges not yet in `visible` (the edge-proposals
/// import validates jointly — two proposals must not close what each alone
/// would not); the candidate edge itself is overlaid internally.
fn check_succession_edge(
    fact_id: &str,
    frame: &str,
    branch: &str,
    target: &str,
    visible: &BTreeMap<String, NarrativeFact>,
    branches: &BTreeMap<String, Branch>,
    staged_edges: &BTreeMap<String, String>,
) -> Result<(), String> {
    match visible.get(target) {
        None => {
            return Err(format!(
                "fact `{fact_id}`: supersedes_in_frame `{target}` not present \
                 (succession needs an existing predecessor; fail-loud)"
            ));
        }
        Some(t) if t.frame != frame => {
            return Err(format!(
                "fact `{fact_id}` (frame `{frame}`): supersedes_in_frame `{target}` lives in \
                 frame `{}` — in-frame succession only (cross-frame disagreement is \
                 data, not succession)",
                t.frame
            ));
        }
        Some(t)
            if t.branch != branch
                && !mnemosyne_core::succession_branch_inherits(branches, branch, &t.branch)? =>
        {
            return Err(format!(
                "fact `{fact_id}` (branch `{branch}`): supersedes_in_frame `{target}` lives on \
                 branch `{}`, whose belief this world-line does not inherit — succession \
                 crosses world-lines only by inheritance (a fork inheriting an ancestor's \
                 belief, or a confluence reconciling a parent's at the merge), never between \
                 siblings (divergence is data, not succession)",
                t.branch
            ));
        }
        Some(_) => {}
    }
    // Acyclicity: walk the outbound chain from the candidate edge. Each
    // fact carries at most one outbound pointer, so the walk is linear; a
    // revisit means the candidate closes (or runs into) a loop — reject
    // loud either way (a pre-existing loop is out-of-band corruption).
    let outbound = |id: &str| -> Option<String> {
        if id == fact_id {
            Some(target.to_string())
        } else if let Some(staged) = staged_edges.get(id) {
            Some(staged.clone())
        } else {
            visible.get(id).and_then(|f| f.supersedes_in_frame.clone())
        }
    };
    let mut visited: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    visited.insert(fact_id.to_string());
    let mut cur = outbound(fact_id);
    while let Some(next) = cur {
        if !visited.insert(next.clone()) {
            return Err(format!(
                "fact `{fact_id}`: supersedes_in_frame `{target}` closes a succession \
                 cycle through `{next}` — a fact cannot (transitively) supersede \
                 itself; in-world change is a NEW successor fact, never a loop"
            ));
        }
        cur = outbound(&next);
    }
    Ok(())
}

/// Whether a conflict edge between `a` and `b` is already recorded on
/// EITHER side (edges are read symmetrically — the Round 463 shared
/// predicate both conflict write paths consult).
fn conflict_edge_recorded(facts: &BTreeMap<String, NarrativeFact>, a: &str, b: &str) -> bool {
    facts
        .get(a)
        .is_some_and(|f| f.conflicts_with.iter().any(|c| c.target == b))
        || facts
            .get(b)
            .is_some_and(|f| f.conflicts_with.iter().any(|c| c.target == a))
}

/// Cross-fact ref check + judgment stamping for one fact against a
/// visibility set (the caller's store ∪ manifest view). Conflict targets
/// must exist, and each assertion is STAMPED with the target's current
/// claim sha256 (Round 439 — judgment-time content pin, computed here and
/// never caller-supplied). `pays_off` targets must exist but stay
/// UNPINNED (Round 442 — like succession they relate fact identities,
/// not wordings). `supersedes_in_frame` rides the full shared invariant
/// set ([`check_succession_edge`]): in-frame, fork-lineage branch, and
/// acyclic chain.
fn validate_and_stamp_fact_refs(
    fact_id: &str,
    fact: &mut NarrativeFact,
    visible: &BTreeMap<String, NarrativeFact>,
    branches: &BTreeMap<String, Branch>,
) -> Result<(), String> {
    for c in &mut fact.conflicts_with {
        let Some(target) = visible.get(&c.target) else {
            return Err(format!(
                "fact `{fact_id}`: conflicts_with `{}` not present \
                 (a recorded assertion needs an existing target; fail-loud)",
                c.target
            ));
        };
        c.target_claim_sha256 = sha256_hex(target.claim.as_bytes());
    }
    for target in &fact.pays_off {
        if !visible.contains_key(target) {
            return Err(format!(
                "fact `{fact_id}`: pays_off `{target}` not present \
                 (a payoff resolves an existing setup; fail-loud). Identity ref, \
                 deliberately unpinned — like succession it relates fact \
                 identities, not wordings"
            ));
        }
    }
    if let Some(target) = &fact.supersedes_in_frame {
        check_succession_edge(
            fact_id,
            &fact.frame,
            &fact.branch,
            target,
            visible,
            branches,
            &BTreeMap::new(),
        )?;
    }
    Ok(())
}

/// Register one epistemic frame. A2-consistent verdicts: absent → create,
/// byte-identical → idempotent no-op (written_bytes 0), divergent → reject
/// (no silent overwrite; description revision = a future setter,
/// consumer-pull).
/// THE shared registry staging path (Round 446 — the 4th registry fired
/// the R440 dedup carry: six hand-rolled copies of this shape, across the
/// standalone primitives and the manifest loops, collapsed here).
/// A2-consistent 3-way verdict: absent → create (`Ok(true)`),
/// byte-identical → idempotent no-op (`Ok(false)`), divergent → reject
/// (never a silent overwrite). `context` names the caller for the message
/// (`add_frame` / `import_facts: manifest frame 3`); `kind` is the
/// registry-entry kind. Ids arrive pre-trimmed (callers trim once and
/// reuse for receipts); registry-specific prechecks (the `MAIN_BRANCH`
/// reject, fork shaping, object_kind parsing) stay with their callers —
/// this helper owns only the verdict every registry shares.
fn stage_registry_entry<T: PartialEq>(
    map: &mut BTreeMap<String, T>,
    context: &str,
    kind: &str,
    id: &str,
    candidate: T,
) -> Result<bool, String> {
    if id.is_empty() {
        return Err(format!(
            "{context}: {kind}_id mandatory (non-empty after trim)"
        ));
    }
    match map.get(id) {
        None => {
            map.insert(id.to_string(), candidate);
            Ok(true)
        }
        Some(existing) if *existing == candidate => Ok(false),
        Some(_) => Err(format!(
            "{context}: {kind} `{id}` already exists with DIVERGENT content — \
             refusing silent overwrite"
        )),
    }
}

/// Receipt half of the standalone registry primitives: save on create,
/// zero-byte no-op receipt otherwise.
fn registry_receipt(
    store: &AtomicStore,
    sidecar_path: &Path,
    primitive: &str,
    target_kind: &'static str,
    id: &str,
    created: bool,
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
    if created {
        save_with_receipt(store, sidecar_path, primitive, target_kind, id)
    } else {
        Ok(AtomicMutateReceipt {
            primitive: primitive.to_string(),
            target_kind,
            target_id: format!("{id} (no-op)"),
            sidecar_path: sidecar_path.display().to_string(),
            written_bytes: 0,
        })
    }
}

pub fn add_frame(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    frame_id: &str,
    description: &str,
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
    let id = frame_id.trim().to_string();
    let candidate = Frame {
        description: description.trim().to_string(),
    };
    let created = stage_registry_entry(&mut store.frames, "add_frame", "frame", &id, candidate)
        .map_err(AtomicMutateError::Validation)?;
    registry_receipt(store, sidecar_path, "add_frame", "frame", &id, created)
}

/// Register one world-line branch (Round 436 — the frames-registry symmetry:
/// every non-default fact branch must reference a registered id, so a typo'd
/// branch fails loud at the write path instead of silently creating a
/// world). `MAIN_BRANCH` is known by construction and rejects registration
/// (one way to say the default). A2-consistent verdicts: absent → create,
/// byte-identical → idempotent no-op, divergent → reject.
/// Validate + shape one fork declaration against the store (Round 438) —
/// the ONE place fork invariants live, shared by `add_branch` and the
/// manifest path. Parent must be `MAIN_BRANCH` or already registered (and
/// not the branch itself); the divergence point must be a section. Because
/// the parent must pre-exist and the fork is immutable after registration,
/// ancestry is a forest by construction.
fn build_branch_fork(
    store: &AtomicStore,
    branch_id: &str,
    forks_from: Option<(&str, &str)>,
) -> Result<Option<BranchFork>, String> {
    let Some((parent, at)) = forks_from else {
        return Ok(None);
    };
    let parent = parent.trim();
    let at = at.trim();
    if parent.is_empty() || at.is_empty() {
        return Err(format!(
            "branch `{branch_id}`: forks_from needs both a parent branch and a canon point"
        ));
    }
    if parent == branch_id {
        return Err(format!("branch `{branch_id}`: cannot fork from itself"));
    }
    if !mnemosyne_core::is_known_world(&store.branches, parent) {
        return Err(format!(
            "branch `{branch_id}`: fork parent `{parent}` not present in the branch \
             registry (register parents first; fail-loud)"
        ));
    }
    if !store.sections.contains_key(at) {
        return Err(format!(
            "branch `{branch_id}`: fork point `{at}` not present as a section \
             (canon coordinates are structure refs)"
        ));
    }
    Ok(Some(BranchFork {
        branch: parent.to_string(),
        at: at.to_string(),
    }))
}

/// Validate the incoming-merge edges of a confluence branch (Round 532 — the
/// `converges_from` analog of [`build_branch_fork`]). Each `(parent, at)` is
/// shaped exactly like a fork edge — same parent-exists + canon-point + blank +
/// self-reference checks — but a confluence has ≥ 2 DISTINCT parents (a
/// 1-parent merge is just a fork). Empty input = not a confluence.
fn build_branch_converges(
    store: &AtomicStore,
    branch_id: &str,
    converges_from: &[(&str, &str)],
) -> Result<Vec<BranchFork>, String> {
    if converges_from.is_empty() {
        return Ok(Vec::new());
    }
    if converges_from.len() < 2 {
        return Err(format!(
            "branch `{branch_id}`: a confluence merges ≥ 2 parent world-lines; \
             converges_from names only 1 (a 1-parent merge is just a fork)"
        ));
    }
    let mut out = Vec::with_capacity(converges_from.len());
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for &(parent, at) in converges_from {
        let parent = parent.trim();
        let at = at.trim();
        if parent.is_empty() || at.is_empty() {
            return Err(format!(
                "branch `{branch_id}`: converges_from needs both a parent branch and a canon point"
            ));
        }
        if parent == branch_id {
            return Err(format!("branch `{branch_id}`: cannot converge from itself"));
        }
        if !mnemosyne_core::is_known_world(&store.branches, parent) {
            return Err(format!(
                "branch `{branch_id}`: converge parent `{parent}` not present in the branch \
                 registry (register parents first; fail-loud)"
            ));
        }
        if !store.sections.contains_key(at) {
            return Err(format!(
                "branch `{branch_id}`: converge point `{at}` not present as a section \
                 (canon coordinates are structure refs)"
            ));
        }
        if !seen.insert(parent.to_string()) {
            return Err(format!(
                "branch `{branch_id}`: converges_from names parent `{parent}` more than once"
            ));
        }
        out.push(BranchFork {
            branch: parent.to_string(),
            at: at.to_string(),
        });
    }
    Ok(out)
}

/// Build a [`Branch`] candidate, enforcing the fork-XOR-confluence rule (Round
/// 532): a world-line is either a fork-child (`forks_from`) or a confluence
/// (`converges_from`), never both. Shared by [`add_branch`] and `import_facts`.
fn build_branch_candidate(
    store: &AtomicStore,
    branch_id: &str,
    description: &str,
    forks_from: Option<(&str, &str)>,
    converges_from: &[(&str, &str)],
) -> Result<Branch, String> {
    if forks_from.is_some() && !converges_from.is_empty() {
        return Err(format!(
            "branch `{branch_id}`: a world-line is either a fork-child (forks_from) or a \
             confluence (converges_from), never both"
        ));
    }
    Ok(Branch {
        description: description.trim().to_string(),
        forks_from: build_branch_fork(store, branch_id, forks_from)?,
        converges_from: build_branch_converges(store, branch_id, converges_from)?,
    })
}

pub fn add_branch(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    branch_id: &str,
    description: &str,
    forks_from: Option<(&str, &str)>,
    converges_from: &[(&str, &str)],
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
    let id = branch_id.trim();
    // Fail on a blank id BEFORE fork shaping (Round 448 session review:
    // the staging helper also rejects it, but only after build_branch_fork
    // would have emitted a blank-named fork message — fail with the
    // precise cause first).
    if id.is_empty() {
        return Err(AtomicMutateError::Validation(
            "add_branch: branch_id mandatory (non-empty after trim)".to_string(),
        ));
    }
    if id == mnemosyne_core::MAIN_BRANCH {
        return Err(AtomicMutateError::Validation(format!(
            "add_branch: `{id}` is the default world-line — known by construction, \
             never registered"
        )));
    }
    let candidate = build_branch_candidate(store, id, description, forks_from, converges_from)
        .map_err(AtomicMutateError::Validation)?;
    let id = id.to_string();
    let created = stage_registry_entry(&mut store.branches, "add_branch", "branch", &id, candidate)
        .map_err(AtomicMutateError::Validation)?;
    registry_receipt(store, sidecar_path, "add_branch", "branch", &id, created)
}

/// Register one narrative entity (Round 437 — the third registry, after
/// frames and branches: every `NarrativeFact.entities` ref must name a
/// registered id, so a typo'd entity fails loud instead of silently
/// splitting a dossier). A2-consistent verdicts: absent → create,
/// byte-identical → idempotent no-op, divergent → reject.
pub fn add_entity(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    entity_id: &str,
    kind: &str,
    description: &str,
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
    let id = entity_id.trim().to_string();
    let candidate = Entity {
        kind: kind.trim().to_string(),
        description: description.trim().to_string(),
    };
    let created = stage_registry_entry(&mut store.entities, "add_entity", "entity", &id, candidate)
        .map_err(AtomicMutateError::Validation)?;
    registry_receipt(store, sidecar_path, "add_entity", "entity", &id, created)
}

/// Parse one predicate declaration into its registry value (Round 446) —
/// the ONE place the `object_kind` tag is interpreted, shared by
/// `add_predicate` and the manifest path (R305 parity). Unknown tags
/// reject; there is no silent default for a load-bearing declaration.
fn build_predicate(
    context: &str,
    object_kind: &str,
    description: &str,
) -> Result<Predicate, String> {
    let tag = object_kind.trim();
    let object_kind = PredicateObjectKind::from_tag(tag).ok_or_else(|| {
        format!("{context}: unknown object_kind `{tag}` (expected one of: entity, scalar)")
    })?;
    Ok(Predicate {
        object_kind,
        description: description.trim().to_string(),
    })
}

/// Register one predicate (Round 446 — the FOURTH registry, design sec
/// 7.12): every `TypedClaim.predicate` must reference a registered id.
/// Predicates are load-bearing (narrative rules key off them; a typo'd
/// predicate would silently escape its rule — the R436 write-side-typo
/// lesson), hence the same fail-loud registry contract as
/// frames/branches/entities. A2-consistent verdicts via the shared
/// staging path.
pub fn add_predicate(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    predicate_id: &str,
    object_kind: &str,
    description: &str,
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
    let id = predicate_id.trim().to_string();
    let candidate = build_predicate("add_predicate", object_kind, description)
        .map_err(AtomicMutateError::Validation)?;
    let created = stage_registry_entry(
        &mut store.predicates,
        "add_predicate",
        "predicate",
        &id,
        candidate,
    )
    .map_err(AtomicMutateError::Validation)?;
    registry_receipt(
        store,
        sidecar_path,
        "add_predicate",
        "predicate",
        &id,
        created,
    )
}

/// Register one disclosure (discourse) plan — a named telling over the fact
/// base (Round 506, design sec 7.24): a default mode (policy) the per-fact
/// overrides ([`set_disclosure`]) sit on top of. The registry symmetry
/// (frames/branches/entities/predicates), but with one difference: the plan is
/// MUTATED after registration (overrides are added), so the idempotency check
/// compares ONLY the policy `(description, default_mode)` — a re-add after
/// `set_disclosure` populated overrides is still a clean no-op, while a changed
/// policy fails loud. `default_mode` parses through the fail-loud tag (no silent
/// default for a load-bearing policy).
pub fn add_disclosure_plan(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    telling_id: &str,
    default_mode: &str,
    description: &str,
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
    let (id, created) = apply_disclosure_plan(store, telling_id, default_mode, description)?;
    registry_receipt(
        store,
        sidecar_path,
        "add_disclosure_plan",
        "disclosure_plan",
        &id,
        created,
    )
}

/// Register a disclosure plan in an IN-MEMORY store WITHOUT persisting (Round
/// 590) — the shared core of [`add_disclosure_plan`] and the all-primitive
/// manifest apply, so both write paths enforce the SAME policy-divergence
/// invariant (the multi-write-path parity discipline). Returns the trimmed
/// telling id + whether it was created (a matching re-add is a no-op).
pub(crate) fn apply_disclosure_plan(
    store: &mut AtomicStore,
    telling_id: &str,
    default_mode: &str,
    description: &str,
) -> Result<(String, bool), AtomicMutateError> {
    let id = telling_id.trim().to_string();
    if id.is_empty() {
        return Err(AtomicMutateError::Validation(
            "add_disclosure_plan: telling_id mandatory (non-empty after trim)".to_string(),
        ));
    }
    let mode = DisclosureMode::from_tag(default_mode.trim()).ok_or_else(|| {
        AtomicMutateError::Validation(format!(
            "add_disclosure_plan: unknown default_mode `{}` (expected one of: \
             withhold, state, hint, imply)",
            default_mode.trim()
        ))
    })?;
    let description = description.trim().to_string();
    let created = match store.disclosure_plans.get(&id) {
        None => {
            store.disclosure_plans.insert(
                id.clone(),
                DisclosurePlan {
                    description,
                    default_mode: mode,
                    overrides: BTreeMap::new(),
                },
            );
            true
        }
        Some(existing) if existing.description == description && existing.default_mode == mode => {
            false
        }
        Some(_) => {
            return Err(AtomicMutateError::Validation(format!(
                "add_disclosure_plan: telling `{id}` already exists with DIVERGENT policy \
                 (description/default_mode) — refusing silent overwrite (set-disclosure \
                 edits the per-fact overrides; re-adding may not change the policy)"
            )));
        }
    };
    Ok((id, created))
}

/// The authored inputs to [`set_disclosure`] (Round 510 — bundled so the
/// primitive takes store + sidecar + one decision, not seven positional args;
/// the [`ChangelogEntryDraft`] precedent: borrowed fields, the caller owns
/// them).
pub struct DisclosureDecision<'a> {
    pub telling_id: &'a str,
    pub fact_id: &'a str,
    /// Disclosure mode tag (`withhold`/`state`/`hint`/`imply`); parsed
    /// fail-loud.
    pub mode: &'a str,
    /// Per-world-line `first_at` pins as `(branch, coord)` pairs.
    pub first_at: &'a [(String, String)],
    /// Optional `(scene, object?)` diegetic surface.
    pub surface: Option<(&'a str, Option<&'a str>)>,
}

/// Set one per-fact disclosure override within a telling (Round 506, design sec
/// 7.24): how a fact reaches the reader, when (per world-line), and on what
/// surface. A setter (last-write-wins on the override — authoring iteration,
/// not the append-only audit genre). Fail-loud refs: the telling and the fact
/// must exist, each `first_at` branch must be registered (or `MAIN_BRANCH`),
/// each `first_at` coord + the surface scene must be a section, the surface
/// object must be a registered entity. THE gate-enabling invariant: a
/// `withhold` mode OR any `first_at` timing pin requires the targeted fact to
/// carry a typed claim — the premature-leak render-acceptance gate matches the
/// re-extracted prose to the plan by typed (subject, predicate, object) tuple,
/// so a disclosure decision on an untyped fact would be deterministically
/// un-gateable (R506: the determinism keystone).
pub fn set_disclosure(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    decision: DisclosureDecision<'_>,
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
    let telling = decision.telling_id.trim().to_string();
    let fact = decision.fact_id.trim().to_string();
    apply_disclosure_override(store, decision)?;
    save_with_receipt(
        store,
        sidecar_path,
        "set_disclosure",
        "disclosure_plan",
        &format!("{telling}/{fact}"),
    )
}

/// Set one per-fact disclosure override in an IN-MEMORY store WITHOUT persisting
/// (Round 590) — the shared core of [`set_disclosure`] and the all-primitive
/// manifest apply, so both write paths enforce the SAME fail-loud refs + the
/// gate-enabling typed-fact invariant (the multi-write-path parity discipline;
/// a manifest override must never bypass a check the standalone setter runs).
pub(crate) fn apply_disclosure_override(
    store: &mut AtomicStore,
    decision: DisclosureDecision<'_>,
) -> Result<bool, AtomicMutateError> {
    let DisclosureDecision {
        telling_id,
        fact_id,
        mode,
        first_at,
        surface,
    } = decision;
    let telling = telling_id.trim();
    let fact = fact_id.trim();
    if !store.disclosure_plans.contains_key(telling) {
        return Err(AtomicMutateError::Validation(format!(
            "set_disclosure: telling `{telling}` not present in the disclosure_plans \
             registry (add-disclosure-plan first)"
        )));
    }
    let fact_is_typed = match store.narrative_facts.get(fact) {
        Some(f) => f.typed.is_some(),
        None => {
            return Err(AtomicMutateError::Validation(format!(
                "set_disclosure: fact `{fact}` not present in narrative_facts"
            )));
        }
    };
    let mode = DisclosureMode::from_tag(mode.trim()).ok_or_else(|| {
        AtomicMutateError::Validation(format!(
            "set_disclosure: unknown mode `{}` (expected one of: withhold, state, hint, imply)",
            mode.trim()
        ))
    })?;
    let has_first_at = !first_at.is_empty();
    if (mode == DisclosureMode::Withhold || has_first_at) && !fact_is_typed {
        return Err(AtomicMutateError::Validation(format!(
            "set_disclosure: fact `{fact}` has no typed claim, but a withhold/first_at \
             disclosure decision is deterministically un-gateable without one (the \
             premature-leak gate matches by typed tuple — author a typed leg first)"
        )));
    }
    let mut first_at_map: BTreeMap<String, String> = BTreeMap::new();
    for (branch, coord) in first_at {
        let branch = branch.trim();
        let coord = coord.trim();
        if branch.is_empty() || coord.is_empty() {
            return Err(AtomicMutateError::Validation(
                "set_disclosure: each first_at needs branch=coord (both non-empty)".to_string(),
            ));
        }
        if !mnemosyne_core::is_known_world(&store.branches, branch) {
            return Err(AtomicMutateError::Validation(format!(
                "set_disclosure: first_at branch `{branch}` not present in the branch registry"
            )));
        }
        if !store.sections.contains_key(coord) {
            return Err(AtomicMutateError::Validation(format!(
                "set_disclosure: first_at coord `{coord}` not present as a section \
                 (canon coordinates are structure refs)"
            )));
        }
        if first_at_map
            .insert(branch.to_string(), coord.to_string())
            .is_some()
        {
            return Err(AtomicMutateError::Validation(format!(
                "set_disclosure: duplicate first_at branch `{branch}`"
            )));
        }
    }
    let surface = match surface {
        None => None,
        Some((scene, object)) => {
            let scene = scene.trim();
            if scene.is_empty() {
                return Err(AtomicMutateError::Validation(
                    "set_disclosure: surface needs a scene ref".to_string(),
                ));
            }
            if !store.sections.contains_key(scene) {
                return Err(AtomicMutateError::Validation(format!(
                    "set_disclosure: surface scene `{scene}` not present as a section"
                )));
            }
            let object = match object {
                Some(o) if !o.trim().is_empty() => {
                    let o = o.trim();
                    if !store.entities.contains_key(o) {
                        return Err(AtomicMutateError::Validation(format!(
                            "set_disclosure: surface object `{o}` not present in the entity registry"
                        )));
                    }
                    Some(o.to_string())
                }
                _ => None,
            };
            Some(DisclosureSurface {
                scene: scene.to_string(),
                object,
            })
        }
    };
    let plan = store
        .disclosure_plans
        .get_mut(telling)
        .expect("telling presence checked above");
    let new_override = DisclosureOverride {
        mode,
        first_at: first_at_map,
        surface,
    };
    // Whether the stored override actually changed — a re-set of the identical
    // decision is a no-op, so a manifest re-import stays byte-stable (the
    // standalone `set_disclosure` persists unconditionally, its own contract).
    let changed = plan.overrides.get(fact) != Some(&new_override);
    plan.overrides.insert(fact.to_string(), new_override);
    Ok(changed)
}

/// Divergent-overwrite reject message, shared by both create paths (Round
/// 440 nuance on the R434 advice): when the ONLY divergence is the
/// judgment pins — a conflict target's claim was amended since this exact
/// content was recorded — say so, because the fix is re-affirmation, not a
/// content change.
fn divergent_fact_message(
    primitive: &str,
    fact_id: &str,
    existing: &NarrativeFact,
    candidate: &NarrativeFact,
) -> String {
    let unpin = |f: &NarrativeFact| {
        let mut f = f.clone();
        for c in &mut f.conflicts_with {
            c.target_claim_sha256 = String::new();
        }
        f
    };
    if unpin(existing) == unpin(candidate) {
        format!(
            "{primitive}: fact `{fact_id}` matches the stored content except for STALE \
             judgment pins — a conflict target's claim was amended since this judgment \
             was recorded; re-affirm via amend_fact (it restamps outbound judgments)"
        )
    } else {
        format!(
            "{primitive}: fact `{fact_id}` already exists with DIVERGENT content — \
             refusing silent overwrite (in-world belief change: supersede in-frame; \
             authorial correction: amend_fact / retract_fact)"
        )
    }
}

/// Create one narrative fact. Routes the SAME builder as `import_facts`
/// (R305 parity); cross-fact refs resolve against the store only (a single
/// add cannot forward-reference). 3-way verdict mirrors `import_sections`:
/// absent → create, byte-identical → no-op, divergent → reject (never a
/// silent overwrite — in-world belief change is in-frame supersession;
/// authorial correction is `amend_fact` / `retract_fact`, Round 434).
pub fn add_fact(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    entry: &FactImport,
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
    let (fact_id, mut candidate) =
        build_candidate_fact(store, entry).map_err(AtomicMutateError::Validation)?;
    validate_and_stamp_fact_refs(
        &fact_id,
        &mut candidate,
        &store.narrative_facts,
        &store.branches,
    )
    .map_err(AtomicMutateError::Validation)?;
    match store.narrative_facts.get(&fact_id) {
        Some(existing) if *existing == candidate => Ok(AtomicMutateReceipt {
            primitive: "add_fact".to_string(),
            target_kind: "narrative_fact",
            target_id: format!("{fact_id} (no-op)"),
            sidecar_path: sidecar_path.display().to_string(),
            written_bytes: 0,
        }),
        Some(existing) => Err(AtomicMutateError::Validation(divergent_fact_message(
            "add_fact", &fact_id, existing, &candidate,
        ))),
        None => {
            store.narrative_facts.insert(fact_id.clone(), candidate);
            save_with_receipt(store, sidecar_path, "add_fact", "narrative_fact", &fact_id)
        }
    }
}

/// The outcome of applying a [`FactsManifest`] to an in-memory store (Round
/// 587): the human summary line plus whether anything actually changed (a
/// pure no-op re-application persists nothing). Returned by
/// [`apply_facts_manifest`]; [`import_facts`] turns it into a receipt.
#[derive(Debug, Clone)]
pub struct FactsApplyOutcome {
    /// The `N frames + … + N facts created, N no-op` summary.
    pub summary: String,
    /// `true` iff at least one registry entry or fact was created (persist
    /// only when changed — the byte-stable no-op contract).
    pub changed: bool,
}

/// Apply a manifest to an IN-MEMORY store WITHOUT persisting (Round 588) — the
/// shared apply core of [`import_facts`] and the dry-run `propose-verdict`
/// transaction. Registries land first so same-manifest facts can reference
/// them; cross-fact refs + judgment pins are checked/stamped AFTER all facts
/// stage (forward references within one manifest are legal). Any rejection
/// returns `Err` fail-fast — on a failure the store may be left partially
/// mutated, so a dry run applies to a throwaway clone and `import_facts`
/// persists only on `Ok`. This function performs NO I/O.
pub fn apply_facts_manifest(
    store: &mut AtomicStore,
    manifest: &FactsManifest,
) -> Result<FactsApplyOutcome, AtomicMutateError> {
    let mut frames_created = 0usize;
    let mut branches_created = 0usize;
    let mut facts_created = 0usize;
    let mut no_op = 0usize;
    for (idx, f) in manifest.frames.iter().enumerate() {
        let candidate = Frame {
            description: f.description.trim().to_string(),
        };
        let created = stage_registry_entry(
            &mut store.frames,
            &format!("import_facts: manifest frame {idx}"),
            "frame",
            f.frame_id.trim(),
            candidate,
        )
        .map_err(AtomicMutateError::Validation)?;
        if created {
            frames_created += 1;
        } else {
            no_op += 1;
        }
    }
    for (idx, b) in manifest.branches.iter().enumerate() {
        let id = b.branch_id.trim();
        // Blank-id fail-fast before fork shaping (Round 448 — the
        // add_branch symmetry; the staging helper's check would fire too
        // late to name the precise cause).
        if id.is_empty() {
            return Err(AtomicMutateError::Validation(format!(
                "import_facts: manifest branch {idx}: branch_id mandatory (non-empty after trim)"
            )));
        }
        if id == mnemosyne_core::MAIN_BRANCH {
            return Err(AtomicMutateError::Validation(format!(
                "import_facts: manifest branch {idx}: `{id}` is the default world-line — \
                 known by construction, never registered"
            )));
        }
        let fork_pair = match (&b.forks_from, &b.forks_at) {
            (None, None) => None,
            (Some(p), Some(a)) => Some((p.as_str(), a.as_str())),
            _ => {
                return Err(AtomicMutateError::Validation(format!(
                    "import_facts: manifest branch {idx} `{id}`: forks_from and forks_at \
                     must be declared together"
                )));
            }
        };
        let converge_pairs: Vec<(&str, &str)> = b
            .converges_from
            .iter()
            .map(|c| (c.branch.as_str(), c.at.as_str()))
            .collect();
        let candidate =
            build_branch_candidate(store, id, &b.description, fork_pair, &converge_pairs)
                .map_err(AtomicMutateError::Validation)?;
        let created = stage_registry_entry(
            &mut store.branches,
            &format!("import_facts: manifest branch {idx}"),
            "branch",
            id,
            candidate,
        )
        .map_err(AtomicMutateError::Validation)?;
        if created {
            branches_created += 1;
        } else {
            no_op += 1;
        }
    }
    let mut entities_created = 0usize;
    for (idx, e) in manifest.entities.iter().enumerate() {
        let candidate = Entity {
            kind: e.kind.trim().to_string(),
            description: e.description.trim().to_string(),
        };
        let created = stage_registry_entry(
            &mut store.entities,
            &format!("import_facts: manifest entity {idx}"),
            "entity",
            e.entity_id.trim(),
            candidate,
        )
        .map_err(AtomicMutateError::Validation)?;
        if created {
            entities_created += 1;
        } else {
            no_op += 1;
        }
    }
    let mut predicates_created = 0usize;
    for (idx, p) in manifest.predicates.iter().enumerate() {
        let context = format!("import_facts: manifest predicate {idx}");
        let candidate = build_predicate(&context, &p.object_kind, &p.description)
            .map_err(AtomicMutateError::Validation)?;
        let created = stage_registry_entry(
            &mut store.predicates,
            &context,
            "predicate",
            p.predicate_id.trim(),
            candidate,
        )
        .map_err(AtomicMutateError::Validation)?;
        if created {
            predicates_created += 1;
        } else {
            no_op += 1;
        }
    }
    let mut staged: Vec<(String, NarrativeFact)> = Vec::with_capacity(manifest.facts.len());
    for (idx, entry) in manifest.facts.iter().enumerate() {
        let (fact_id, candidate) = build_candidate_fact(store, entry).map_err(|e| {
            AtomicMutateError::Validation(format!("import_facts: manifest fact {idx}: {e}"))
        })?;
        if staged.iter().any(|(sid, _)| *sid == fact_id) {
            return Err(AtomicMutateError::Validation(format!(
                "import_facts: manifest fact {idx}: duplicate fact_id `{fact_id}` in manifest"
            )));
        }
        staged.push((fact_id, candidate));
    }
    // Refs validate + judgment pins stamp BEFORE the create/no-op verdicts
    // (Round 440 parity fix: `add_fact` verdicts on a stamped candidate, so
    // import must too — otherwise an idempotent re-import of a manifest
    // with conflict edges false-diverges on empty pins). Visibility =
    // store ∪ staged, so forward refs within one manifest stay legal and a
    // pin records the target's claim as staged.
    let mut visible = store.narrative_facts.clone();
    for (fact_id, candidate) in &staged {
        visible.insert(fact_id.clone(), candidate.clone());
    }
    for (fact_id, candidate) in &mut staged {
        validate_and_stamp_fact_refs(fact_id, candidate, &visible, &store.branches)
            .map_err(AtomicMutateError::Validation)?;
    }
    for (fact_id, candidate) in staged {
        match store.narrative_facts.get(&fact_id) {
            None => {
                store.narrative_facts.insert(fact_id, candidate);
                facts_created += 1;
            }
            Some(existing) if *existing == candidate => no_op += 1,
            Some(existing) => {
                return Err(AtomicMutateError::Validation(divergent_fact_message(
                    "import_facts",
                    &fact_id,
                    existing,
                    &candidate,
                )));
            }
        }
    }
    // Disclosure plans LAST (Round 590) — an override references a fact + needs
    // the typed-fact invariant, so facts must already be staged. Same cores as
    // the standalone add-disclosure-plan / set-disclosure (write-path parity).
    let mut disclosure_plans_created = 0usize;
    let mut disclosure_overrides_set = 0usize;
    for (idx, plan) in manifest.disclosure_plans.iter().enumerate() {
        let default_mode = plan.default_mode.as_deref().unwrap_or("withhold");
        let (_, created) =
            apply_disclosure_plan(store, &plan.telling_id, default_mode, &plan.description)
                .map_err(|e| {
                    AtomicMutateError::Validation(format!(
                        "import_facts: manifest disclosure_plan {idx}: {e}"
                    ))
                })?;
        if created {
            disclosure_plans_created += 1;
        } else {
            no_op += 1;
        }
        for (ov_idx, ov) in plan.overrides.iter().enumerate() {
            let first_at: Vec<(String, String)> = ov
                .first_at
                .iter()
                .map(|[b, c]| (b.clone(), c.clone()))
                .collect();
            let surface = ov
                .surface
                .as_ref()
                .map(|s| (s.scene.as_str(), s.object.as_deref()));
            let changed = apply_disclosure_override(
                store,
                DisclosureDecision {
                    telling_id: &plan.telling_id,
                    fact_id: &ov.fact_id,
                    mode: &ov.mode,
                    first_at: &first_at,
                    surface,
                },
            )
            .map_err(|e| {
                AtomicMutateError::Validation(format!(
                    "import_facts: manifest disclosure_plan {idx} override {ov_idx}: {e}"
                ))
            })?;
            if changed {
                disclosure_overrides_set += 1;
            } else {
                no_op += 1;
            }
        }
    }
    let summary = format!(
        "{frames_created} frames + {branches_created} branches + {entities_created} entities \
         + {predicates_created} predicates + {facts_created} facts + {disclosure_plans_created} \
         disclosure-plans + {disclosure_overrides_set} disclosure-overrides created, {no_op} no-op"
    );
    let changed = frames_created != 0
        || branches_created != 0
        || entities_created != 0
        || predicates_created != 0
        || facts_created != 0
        || disclosure_plans_created != 0
        || disclosure_overrides_set != 0;
    Ok(FactsApplyOutcome { summary, changed })
}

/// Bulk frames + branches + entities + predicates + facts create from a
/// manifest, as one atomic transaction (the `import_sections` A2 pattern) —
/// [`apply_facts_manifest`] then a single persist. Any rejection returns `Err`
/// before the save, so the on-disk store is untouched; a pure no-op
/// re-application writes nothing (`written_bytes: 0`, the byte-stable
/// idempotency contract).
pub fn import_facts(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    manifest: &FactsManifest,
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
    let outcome = apply_facts_manifest(store, manifest)?;
    if !outcome.changed {
        return Ok(AtomicMutateReceipt {
            primitive: "import_facts".to_string(),
            target_kind: "narrative_fact",
            target_id: outcome.summary,
            sidecar_path: sidecar_path.display().to_string(),
            written_bytes: 0,
        });
    }
    save_with_receipt(
        store,
        sidecar_path,
        "import_facts",
        "narrative_fact",
        &outcome.summary,
    )
}

/// Append one conflict assertion edge between two existing facts. The edge
/// is a RECORDED semantic judgment (contradiction cannot be derived from
/// claim text), stored on `fact_id` and read symmetrically by projections.
/// An edge already present on either side rejects as already-recorded (the
/// confirmation-event idempotency precedent).
pub fn add_fact_conflict(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    fact_id: &str,
    conflicts_with: &str,
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
    let id = fact_id.trim();
    let other = conflicts_with.trim();
    if id.is_empty() || other.is_empty() {
        return Err(AtomicMutateError::Validation(
            "add_fact_conflict: fact_id and conflicts_with both mandatory".to_string(),
        ));
    }
    if id == other {
        return Err(AtomicMutateError::Validation(
            "add_fact_conflict: a fact cannot conflict with itself".to_string(),
        ));
    }
    if !store.narrative_facts.contains_key(id) {
        return Err(AtomicMutateError::NotFound(format!(
            "fact_id `{id}` not present in atomic store"
        )));
    }
    if !store.narrative_facts.contains_key(other) {
        return Err(AtomicMutateError::NotFound(format!(
            "conflicts_with `{other}` not present in atomic store"
        )));
    }
    if conflict_edge_recorded(&store.narrative_facts, id, other) {
        return Err(AtomicMutateError::FrozenLedger(format!(
            "conflict edge `{id}` <-> `{other}` already recorded (idempotent)"
        )));
    }
    let stamp = sha256_hex(store.narrative_facts[other].claim.as_bytes());
    store
        .narrative_facts
        .get_mut(id)
        .expect("checked above")
        .conflicts_with
        .push(ConflictAssertion {
            target: other.to_string(),
            target_claim_sha256: stamp,
        });
    save_with_receipt(
        store,
        sidecar_path,
        "add_fact_conflict",
        "narrative_fact",
        &format!("{id} -> {other}"),
    )
}

/// Inbound references to `fact_id` from every OTHER fact (conflict edges and
/// succession pointers). Shared by [`retract_fact`] (any inbound ref blocks
/// the retract) and [`amend_fact`] (inbound successors must stay same-scope).
fn inbound_fact_refs<'a>(
    facts: &'a BTreeMap<String, NarrativeFact>,
    fact_id: &str,
) -> Vec<(&'a String, &'a NarrativeFact, &'static str)> {
    let mut refs = Vec::new();
    for (other_id, other) in facts {
        if other_id == fact_id {
            continue;
        }
        if other.conflicts_with.iter().any(|c| c.target == fact_id) {
            refs.push((other_id, other, "conflicts_with"));
        }
        if other.supersedes_in_frame.as_deref() == Some(fact_id) {
            refs.push((other_id, other, "supersedes_in_frame"));
        }
        if other.pays_off.iter().any(|t| t == fact_id) {
            refs.push((other_id, other, "pays_off"));
        }
    }
    refs
}

/// Round 434 — authorial retract (design sec 7.9 axis 4). Removes a fact the
/// AUTHOR no longer asserts — distinct from in-frame supersession, which is
/// an IN-WORLD belief change and leaves the predecessor in the log. The
/// transaction-time audit of a retraction is the git history of the log
/// (R330: transaction time = commit time), so nothing is tombstoned in the
/// store; `reason` is mandatory as the audit-trail safeguard (the
/// `remove_section` precedent).
///
/// Fail-loud referential integrity: a fact referenced by any other fact
/// (conflict edge or succession pointer) cannot be retracted — retract or
/// amend the referrers first, so the scan invariants never see a dangling
/// target.
pub fn retract_fact(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    fact_id: &str,
    reason: &str,
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
    if reason.trim().is_empty() {
        return Err(AtomicMutateError::Validation(
            "retract_fact: --reason mandatory (audit-trail safeguard)".to_string(),
        ));
    }
    let id = fact_id.trim();
    if !store.narrative_facts.contains_key(id) {
        return Err(AtomicMutateError::NotFound(format!(
            "fact_id `{id}` not present in atomic store"
        )));
    }
    let referrers = inbound_fact_refs(&store.narrative_facts, id);
    if !referrers.is_empty() {
        let listing = referrers
            .iter()
            .map(|(rid, _, via)| format!("`{rid}` (via {via})"))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(AtomicMutateError::Validation(format!(
            "retract_fact: fact `{id}` is referenced by {listing} — retract or amend the \
             referrers first (a recorded assertion never dangles)"
        )));
    }
    store.narrative_facts.remove(id);
    save_with_receipt(store, sidecar_path, "retract_fact", "narrative_fact", id)
}

/// Round 434 — authorial amend (design sec 7.9 axis 4): replace a fact's
/// content in place, keeping its id. This is the AUTHOR-correction path (a
/// typo, a wrong coordinate) — in-world belief change stays in-frame
/// supersession, never amend. Routes the SAME builder as `add_fact` /
/// `import_facts` (R305 parity: one closed invariant set, `quote_sha256`
/// restamped here, never caller-supplied); the authoring-time audit of what
/// changed is the git history of the log (R330).
///
/// Fail-loud boundaries: the fact must exist (creation is `add_fact`), and
/// inbound successors must remain same-scope — an amend that moves the fact
/// to another frame or branch while something supersedes it would corrupt
/// the succession invariant the write paths enforce.
pub fn amend_fact(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    entry: &FactImport,
    reason: &str,
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
    if reason.trim().is_empty() {
        return Err(AtomicMutateError::Validation(
            "amend_fact: --reason mandatory (audit-trail safeguard)".to_string(),
        ));
    }
    let (fact_id, mut candidate) =
        build_candidate_fact(store, entry).map_err(AtomicMutateError::Validation)?;
    if !store.narrative_facts.contains_key(&fact_id) {
        return Err(AtomicMutateError::NotFound(format!(
            "fact_id `{fact_id}` not present in atomic store (amend revises an existing \
             fact; add_fact creates)"
        )));
    }
    // Outbound judgments restamp here — an amend is the amender's fresh
    // assertion of its own edges (inbound edges pointing AT this fact go
    // stale instead, surfaced by the scan).
    validate_and_stamp_fact_refs(
        &fact_id,
        &mut candidate,
        &store.narrative_facts,
        &store.branches,
    )
    .map_err(AtomicMutateError::Validation)?;
    for (rid, referrer, via) in inbound_fact_refs(&store.narrative_facts, &fact_id) {
        if via == "supersedes_in_frame"
            && (referrer.frame != candidate.frame || referrer.branch != candidate.branch)
        {
            return Err(AtomicMutateError::Validation(format!(
                "amend_fact: fact `{fact_id}` is superseded by `{rid}` in scope (frame `{}`, \
                 branch `{}`) — an amend cannot move it to (frame `{}`, branch `{}`) \
                 (succession is same-scope; amend the successor first)",
                referrer.frame, referrer.branch, candidate.frame, candidate.branch
            )));
        }
    }
    if store.narrative_facts[&fact_id] == candidate {
        return Ok(AtomicMutateReceipt {
            primitive: "amend_fact".to_string(),
            target_kind: "narrative_fact",
            target_id: format!("{fact_id} (no-op)"),
            sidecar_path: sidecar_path.display().to_string(),
            written_bytes: 0,
        });
    }
    store.narrative_facts.insert(fact_id.clone(), candidate);
    save_with_receipt(
        store,
        sidecar_path,
        "amend_fact",
        "narrative_fact",
        &fact_id,
    )
}

// ============================================================================
// Typing-proposals import (Round 459, design sec 7.15 Round B).
// ============================================================================

/// One proposed typed leg for an existing untyped fact — authored OUTSIDE
/// the substrate (an LLM agent reading `report-typing-candidates`) and
/// quarantined in this reviewable artifact; it enters the store only
/// through [`import_typing_proposals`], the declared import act (B-1:
/// "never NLP-inferred" precisely means "never SILENTLY NLP-inferred").
/// `deny_unknown_fields`: a new artifact carries no lenient-parse legacy —
/// a typo'd key fails loud, never a silently dropped proposal field.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypingProposal {
    /// Target fact id (must exist and be untyped — fill-blanks only).
    pub fact: String,
    /// The proposed leg, validated by THE one builder at import.
    pub typed: TypedClaim,
    /// sha256 of the claim text the proposer interpreted (the R439
    /// judgment-time pin re-targeted): import re-checks, so a fact
    /// amended after proposing fails loud as stale.
    pub claim_sha256: String,
    /// Prose justification for the reviewer — the reviewable substance
    /// (deliberately no confidence score, the Goodhart guard).
    pub rationale: String,
}

/// The `typing-proposals/v1` artifact (design sec 7.15).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypingProposalsFile {
    /// Must be exactly `typing-proposals/v1`.
    pub schema: String,
    /// Optional free-form note from the proposer.
    #[serde(default)]
    pub comment: String,
    pub proposals: Vec<TypingProposal>,
}

/// Load + shape-check a `typing-proposals/v1` file; returns the parsed
/// artifact with the file content's sha256 (the audit anchor the import
/// receipt carries).
pub fn load_typing_proposals(path: &Path) -> Result<(TypingProposalsFile, String), String> {
    let raw = fs::read_to_string(path)
        .map_err(|e| format!("typing-proposals: cannot read `{}`: {e}", path.display()))?;
    let file: TypingProposalsFile = serde_json::from_str(&raw)
        .map_err(|e| format!("typing-proposals: `{}` does not parse: {e}", path.display()))?;
    if file.schema != "typing-proposals/v1" {
        return Err(format!(
            "typing-proposals: schema `{}` is not `typing-proposals/v1` (fail-loud — \
             an unknown schema must not half-apply)",
            file.schema
        ));
    }
    if file.proposals.is_empty() {
        return Err("typing-proposals: empty proposals list (nothing to import)".to_string());
    }
    Ok((file, sha256_hex(raw.as_bytes())))
}

/// One per-proposal verdict (full list always surfaced — no silent caps).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TypingProposalVerdict {
    pub fact: String,
    /// `accepted`, or the reject reason verbatim.
    pub verdict: String,
}

/// The import outcome both wires emit. `applied` is true only when this
/// was a real run AND every proposal accepted (all-or-nothing: the file
/// is the reviewed artifact — make it fully valid; no half-applied state).
#[derive(Debug, Clone, Serialize)]
pub struct TypingImportReport {
    /// sha256 of the proposals file content (audit anchor).
    pub file_sha256: String,
    pub verdicts: Vec<TypingProposalVerdict>,
    pub accepted: usize,
    pub rejected: usize,
    pub dry_run: bool,
    pub applied: bool,
    pub written_bytes: usize,
}

/// Import typed legs from a reviewed `typing-proposals/v1` artifact
/// (Round 459, design sec 7.15 Round B). ALL-OR-NOTHING with full
/// per-proposal verdicts; `dry_run` runs the identical validation and
/// writes nothing. Every typed invariant rides [`build_typed_claim`] —
/// the ONE builder site both fact write paths share (R305/R446 parity);
/// this path adds ZERO new invariant sites. Fill-blanks only: a proposal
/// targeting an already-typed fact rejects (overwrite is manual author
/// territory). The R439 staleness pin re-checks per proposal: a claim
/// amended after proposing rejects loud instead of silently mis-typing
/// revised prose.
pub fn import_typing_proposals(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    file: &TypingProposalsFile,
    file_sha256: &str,
    dry_run: bool,
) -> Result<TypingImportReport, AtomicMutateError> {
    let mut seen: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    let mut verdicts: Vec<TypingProposalVerdict> = Vec::new();
    let mut built: Vec<(String, TypedClaim)> = Vec::new();
    for p in &file.proposals {
        let fact_id = p.fact.trim();
        let verdict = (|| -> Result<TypedClaim, String> {
            if fact_id.is_empty() {
                return Err("fact id mandatory (non-empty)".to_string());
            }
            if !seen.insert(fact_id) {
                return Err(format!(
                    "duplicate proposal for fact `{fact_id}` in one file — ambiguous, \
                     keep exactly one"
                ));
            }
            let Some(fact) = store.narrative_facts.get(fact_id) else {
                return Err(format!(
                    "fact `{fact_id}` not present in atomic store (proposals target \
                     existing facts; add_fact creates)"
                ));
            };
            if fact.typed.is_some() {
                return Err(format!(
                    "fact `{fact_id}` already carries a typed leg — fill-blanks only \
                     (overwrite is manual author territory: amend-fact)"
                ));
            }
            check_claim_pin(&store.narrative_facts, fact_id, &p.claim_sha256, "fact")?;
            if p.rationale.trim().is_empty() {
                return Err(format!(
                    "fact `{fact_id}`: rationale mandatory (the reviewable substance)"
                ));
            }
            build_typed_claim(store, fact_id, &p.typed, &fact.entities)
        })();
        match verdict {
            Ok(leg) => {
                verdicts.push(TypingProposalVerdict {
                    fact: fact_id.to_string(),
                    verdict: "accepted".to_string(),
                });
                built.push((fact_id.to_string(), leg));
            }
            Err(reason) => verdicts.push(TypingProposalVerdict {
                fact: fact_id.to_string(),
                verdict: reason,
            }),
        }
    }
    let accepted = built.len();
    let rejected = verdicts.len() - accepted;
    let apply = !dry_run && rejected == 0;
    let mut written = 0;
    if apply {
        for (fact_id, leg) in built {
            // Unwrap is total: every id was validated present above and
            // nothing mutates the map in between.
            store.narrative_facts.get_mut(&fact_id).unwrap().typed = Some(leg);
        }
        let receipt = save_with_receipt(
            store,
            sidecar_path,
            "import_typing_proposals",
            "narrative_fact",
            &format!(
                "typing-proposals {} ({} leg(s))",
                file_sha256.get(..16).unwrap_or(file_sha256),
                accepted
            ),
        )?;
        written = receipt.written_bytes;
    }
    Ok(TypingImportReport {
        file_sha256: file_sha256.to_string(),
        verdicts,
        accepted,
        rejected,
        dry_run,
        applied: apply,
        written_bytes: written,
    })
}

// ============================================================================
// Edge-proposals import (Round 463, design sec 7.16 Round B).
// ============================================================================

/// One proposed succession edge: `successor --supersedes--> predecessor`,
/// authored OUTSIDE the substrate and quarantined here (the sec 7.15
/// pattern). The R439 judgment-time pin goes TWO-SIDED: an edge judgment
/// interprets two claim texts, so the proposal stamps both and import
/// re-checks both — either fact amended after proposing fails loud as
/// stale. `deny_unknown_fields`: a stray key (e.g. a confidence score the
/// Goodhart guard bans) fails loud, never silently dropped.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SuccessionProposal {
    /// The fact gaining the backward pointer (must currently carry none —
    /// fill-blanks only; retargeting is manual amend territory).
    pub successor: String,
    /// The predecessor it supersedes in-frame.
    pub predecessor: String,
    /// sha256 of the successor's claim as interpreted.
    pub successor_claim_sha256: String,
    /// sha256 of the predecessor's claim as interpreted.
    pub predecessor_claim_sha256: String,
    /// Prose justification for the reviewer (the reviewable substance).
    pub rationale: String,
}

/// One proposed conflict edge (recorded semantic judgment, stored on
/// `fact` and read symmetrically) — both endpoint claims pinned, like
/// [`SuccessionProposal`].
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConflictProposal {
    /// The fact the edge is stored on.
    pub fact: String,
    /// The fact it is judged to contradict.
    pub target: String,
    /// sha256 of `fact`'s claim as interpreted.
    pub fact_claim_sha256: String,
    /// sha256 of `target`'s claim as interpreted.
    pub target_claim_sha256: String,
    /// Prose justification for the reviewer (the reviewable substance).
    pub rationale: String,
}

/// The `edge-proposals/v1` artifact (design sec 7.16). As-built deviation
/// from the R461 "kind-tagged" wording, declared in Round 463: serde does
/// not support `deny_unknown_fields` on internally tagged enums, and
/// fail-loud parsing outranks the cosmetic tag (a silently-dropped
/// `confidence` key would defeat the Goodhart guard) — so the two kinds
/// are two typed arrays, each entry strictly parsed.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EdgeProposalsFile {
    /// Must be exactly `edge-proposals/v1`.
    pub schema: String,
    /// Optional free-form note from the proposer.
    #[serde(default)]
    pub comment: String,
    #[serde(default)]
    pub succession: Vec<SuccessionProposal>,
    #[serde(default)]
    pub conflicts: Vec<ConflictProposal>,
}

/// Load + shape-check an `edge-proposals/v1` file; returns the parsed
/// artifact with the file content's sha256 (the audit anchor the import
/// receipt carries).
pub fn load_edge_proposals(path: &Path) -> Result<(EdgeProposalsFile, String), String> {
    let raw = fs::read_to_string(path)
        .map_err(|e| format!("edge-proposals: cannot read `{}`: {e}", path.display()))?;
    let file: EdgeProposalsFile = serde_json::from_str(&raw)
        .map_err(|e| format!("edge-proposals: `{}` does not parse: {e}", path.display()))?;
    if file.schema != "edge-proposals/v1" {
        return Err(format!(
            "edge-proposals: schema `{}` is not `edge-proposals/v1` (fail-loud — \
             an unknown schema must not half-apply)",
            file.schema
        ));
    }
    if file.succession.is_empty() && file.conflicts.is_empty() {
        return Err("edge-proposals: no proposals (nothing to import)".to_string());
    }
    Ok((file, sha256_hex(raw.as_bytes())))
}

/// One per-proposal verdict (full list always surfaced — no silent caps).
/// `kind` = `succession` | `conflict`; for succession `fact` is the
/// successor and `target` the predecessor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EdgeProposalVerdict {
    pub kind: &'static str,
    pub fact: String,
    pub target: String,
    /// `accepted`, or the reject reason verbatim.
    pub verdict: String,
}

/// The import outcome both wires emit (the [`TypingImportReport`] shape).
#[derive(Debug, Clone, Serialize)]
pub struct EdgeImportReport {
    /// sha256 of the proposals file content (audit anchor).
    pub file_sha256: String,
    pub verdicts: Vec<EdgeProposalVerdict>,
    pub accepted: usize,
    pub rejected: usize,
    pub dry_run: bool,
    pub applied: bool,
    pub written_bytes: usize,
}

/// Re-check one proposal-side claim pin against the current store text —
/// THE R439 staleness re-check both proposal imports share (Round 465
/// dedup: typing carried an inline copy; R460 unified the hash ENCODING
/// and this unifies the re-check itself). `side` names the endpoint for
/// the message (`fact` / `successor` / `predecessor` / `target`).
fn check_claim_pin(
    facts: &BTreeMap<String, NarrativeFact>,
    fact_id: &str,
    stamped: &str,
    side: &str,
) -> Result<(), String> {
    let current = sha256_hex(facts[fact_id].claim.as_bytes());
    if current != stamped {
        return Err(format!(
            "stale proposal: {side} `{fact_id}` claim sha256 is `{current}` but the \
             proposal interpreted `{stamped}` — the claim changed after proposing; \
             re-run discovery against the current text"
        ));
    }
    Ok(())
}

/// Import succession + conflict edges from a reviewed `edge-proposals/v1`
/// artifact (Round 463, design sec 7.16 Round B). ALL-OR-NOTHING with full
/// per-proposal verdicts; `dry_run` runs the identical validation and
/// writes nothing. Succession invariants ride [`check_succession_edge`] —
/// the ONE site every succession write path shares — with the staged-edge
/// overlay, so two proposals cannot jointly close a cycle each alone would
/// not. Conflict invariants ride the same predicates as
/// [`add_fact_conflict`]. Fill-blanks only; both endpoint pins re-checked
/// per proposal (a claim amended after proposing rejects loud).
pub fn import_edge_proposals(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    file: &EdgeProposalsFile,
    file_sha256: &str,
    dry_run: bool,
) -> Result<EdgeImportReport, AtomicMutateError> {
    let facts = &store.narrative_facts;
    let mut verdicts: Vec<EdgeProposalVerdict> = Vec::new();
    // Every successor seen, ACCEPTED OR NOT (the R459 rule: a duplicate is
    // ambiguous regardless of either verdict).
    let mut seen_successors: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    // Accepted succession edges, successor -> predecessor (the joint-
    // validation overlay AND the apply list).
    let mut staged_succession: BTreeMap<String, String> = BTreeMap::new();
    for p in &file.succession {
        let successor = p.successor.trim();
        let predecessor = p.predecessor.trim();
        let verdict = (|| -> Result<(), String> {
            if successor.is_empty() || predecessor.is_empty() {
                return Err("successor and predecessor both mandatory (non-empty)".to_string());
            }
            if !seen_successors.insert(successor.to_string()) {
                return Err(format!(
                    "duplicate proposal for successor `{successor}` in one file — \
                     ambiguous, keep exactly one"
                ));
            }
            let Some(s) = facts.get(successor) else {
                return Err(format!(
                    "successor `{successor}` not present in atomic store (proposals \
                     target existing facts; add_fact creates)"
                ));
            };
            if s.supersedes_in_frame.is_some() {
                return Err(format!(
                    "successor `{successor}` already carries a succession pointer — \
                     fill-blanks only (retargeting is manual author territory: \
                     amend-fact)"
                ));
            }
            if !facts.contains_key(predecessor) {
                return Err(format!(
                    "predecessor `{predecessor}` not present in atomic store"
                ));
            }
            check_claim_pin(facts, successor, &p.successor_claim_sha256, "successor")?;
            check_claim_pin(
                facts,
                predecessor,
                &p.predecessor_claim_sha256,
                "predecessor",
            )?;
            if p.rationale.trim().is_empty() {
                return Err("rationale mandatory (the reviewable substance)".to_string());
            }
            check_succession_edge(
                successor,
                &s.frame,
                &s.branch,
                predecessor,
                facts,
                &store.branches,
                &staged_succession,
            )?;
            staged_succession.insert(successor.to_string(), predecessor.to_string());
            Ok(())
        })();
        verdicts.push(EdgeProposalVerdict {
            kind: "succession",
            fact: successor.to_string(),
            target: predecessor.to_string(),
            verdict: verdict.map_or_else(|reason| reason, |()| "accepted".to_string()),
        });
    }
    // Accepted conflict pairs, canonical order (dedup within the file).
    let mut staged_conflicts: std::collections::BTreeSet<(String, String)> =
        std::collections::BTreeSet::new();
    let mut conflict_applies: Vec<(String, String)> = Vec::new();
    for p in &file.conflicts {
        let fact_id = p.fact.trim();
        let target = p.target.trim();
        let verdict = (|| -> Result<(), String> {
            if fact_id.is_empty() || target.is_empty() {
                return Err("fact and target both mandatory (non-empty)".to_string());
            }
            if fact_id == target {
                return Err("a fact cannot conflict with itself".to_string());
            }
            for (id, side) in [(fact_id, "fact"), (target, "target")] {
                if !facts.contains_key(id) {
                    return Err(format!("{side} `{id}` not present in atomic store"));
                }
            }
            let key = if fact_id < target {
                (fact_id.to_string(), target.to_string())
            } else {
                (target.to_string(), fact_id.to_string())
            };
            // Seen regardless of verdict (the R459 duplicate rule).
            if !staged_conflicts.insert(key) {
                return Err(format!(
                    "duplicate proposal for conflict pair `{fact_id}` <-> `{target}` \
                     in one file — ambiguous, keep exactly one"
                ));
            }
            if conflict_edge_recorded(facts, fact_id, target) {
                return Err(format!(
                    "conflict edge `{fact_id}` <-> `{target}` already recorded — \
                     never re-propose existing structure"
                ));
            }
            check_claim_pin(facts, fact_id, &p.fact_claim_sha256, "fact")?;
            check_claim_pin(facts, target, &p.target_claim_sha256, "target")?;
            if p.rationale.trim().is_empty() {
                return Err("rationale mandatory (the reviewable substance)".to_string());
            }
            conflict_applies.push((fact_id.to_string(), target.to_string()));
            Ok(())
        })();
        verdicts.push(EdgeProposalVerdict {
            kind: "conflict",
            fact: fact_id.to_string(),
            target: target.to_string(),
            verdict: verdict.map_or_else(|reason| reason, |()| "accepted".to_string()),
        });
    }
    let accepted = staged_succession.len() + conflict_applies.len();
    let rejected = verdicts.len() - accepted;
    let apply = !dry_run && rejected == 0;
    let mut written = 0;
    if apply {
        let succession_count = staged_succession.len();
        let conflict_count = conflict_applies.len();
        for (successor, predecessor) in staged_succession {
            // Unwrap is total: validated present above, nothing mutates
            // the map in between.
            store
                .narrative_facts
                .get_mut(&successor)
                .unwrap()
                .supersedes_in_frame = Some(predecessor);
        }
        for (fact_id, target) in conflict_applies {
            // The write-time stamp equals the verified pin by construction
            // (the pin was checked against the current claim above); it is
            // still COMPUTED here, never copied from the proposal — the
            // R439 never-caller-supplied rule.
            let stamp = sha256_hex(store.narrative_facts[&target].claim.as_bytes());
            store
                .narrative_facts
                .get_mut(&fact_id)
                .unwrap()
                .conflicts_with
                .push(ConflictAssertion {
                    target,
                    target_claim_sha256: stamp,
                });
        }
        let receipt = save_with_receipt(
            store,
            sidecar_path,
            "import_edge_proposals",
            "narrative_fact",
            &format!(
                "edge-proposals {} ({} succession + {} conflict edge(s))",
                file_sha256.get(..16).unwrap_or(file_sha256),
                succession_count,
                conflict_count
            ),
        )?;
        written = receipt.written_bytes;
    }
    Ok(EdgeImportReport {
        file_sha256: file_sha256.to_string(),
        verdicts,
        accepted,
        rejected,
        dry_run,
        applied: apply,
        written_bytes: written,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn rejected_alternative_parse_line_separators_and_bullet() {
        // Em-dash and double-hyphen separators, with/without a bullet marker.
        let em = RejectedAlternative::parse_line("- foo — because bar").unwrap();
        assert_eq!(em.alternative, "foo");
        assert_eq!(em.reason, "because bar");
        let dh = RejectedAlternative::parse_line("foo -- because bar").unwrap();
        assert_eq!(dh, em);
        // No recognized separator → None (caller supplies the contextual error).
        assert!(RejectedAlternative::parse_line("no separator here").is_none());
    }

    /// Round 287 — test fixture helper. Direct sections.insert (bypasses
    /// audit-receipt path) to seed a Section so content-axis primitives can
    /// be exercised. Production code routes Section creation through
    /// `add_section`; tests use this helper to keep setup boilerplate down.
    fn seed_section(store: &mut AtomicStore, section_id: &str) {
        store
            .sections
            .insert(section_id.to_string(), AtomicSection::default());
    }

    // R416 — confirmation-event fixture. Claim targets section "sec".
    fn sample_event(authoring: &str, confirming: &str) -> ConfirmationEvent {
        ConfirmationEvent {
            claim: ConfirmationClaim::VerifiesBinding {
                section_id: "sec".to_string(),
                file: "tests/w3c/Test1.h".to_string(),
                symbol: Some("verify_foo".to_string()),
            },
            confirmer: Confirmer {
                kind: ConfirmerKind::Model,
                id: "claude-opus-4-8".to_string(),
                version: "2026-06".to_string(),
            },
            method: ConfirmMethod::SemanticReview,
            artifact_hashes: ArtifactHashes::default(),
            authoring_run: authoring.to_string(),
            confirming_run: confirming.to_string(),
            verdict: Verdict::Confirm,
            rationale: "the test verifies the bound requirement".to_string(),
            timestamp: "2026-06-09T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn confirmation_event_round_trips() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_section(&mut store, "sec");
        let receipt =
            append_confirmation_event(&mut store, &path, sample_event("runA", "runB")).unwrap();
        assert_eq!(receipt.target_kind, "confirmation_event");
        // The event_id is DERIVED (R417); the receipt carries it.
        assert!(receipt.target_id.starts_with("evt-"));
        let reloaded = AtomicStore::load(&path).unwrap();
        assert_eq!(reloaded.schema_version, CURRENT_SCHEMA_VERSION);
        let ev = &reloaded.confirmation_events[&receipt.target_id];
        assert_eq!(ev.verdict, Verdict::Confirm);
        assert_eq!(ev.confirmer.kind, ConfirmerKind::Model);
        assert_eq!(ev.method, ConfirmMethod::SemanticReview);
        match &ev.claim {
            ConfirmationClaim::VerifiesBinding { section_id, .. } => {
                assert_eq!(section_id, "sec")
            }
            _ => panic!("wrong claim variant"),
        }
    }

    #[test]
    fn confirmation_event_self_confirm_rejected() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_section(&mut store, "sec");
        let err =
            append_confirmation_event(&mut store, &path, sample_event("runA", "runA")).unwrap_err();
        assert!(
            matches!(err, AtomicMutateError::Validation(_)),
            "self-confirm (authoring_run == confirming_run) must reject (sec 4.7)"
        );
    }

    #[test]
    fn confirmation_event_idempotent_reappend_rejected() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_section(&mut store, "sec");
        // Same verification act twice → same derived id → append-only reject.
        append_confirmation_event(&mut store, &path, sample_event("runA", "runB")).unwrap();
        let err =
            append_confirmation_event(&mut store, &path, sample_event("runA", "runB")).unwrap_err();
        assert!(
            matches!(err, AtomicMutateError::FrozenLedger(_)),
            "an identical act must reject (idempotent append-only)"
        );
        // A DISTINCT confirming_run is a different act → accepted (accumulates).
        append_confirmation_event(&mut store, &path, sample_event("runA", "runC")).unwrap();
        assert_eq!(store.confirmation_events.len(), 2);
    }

    #[test]
    fn confirmation_event_unknown_section_rejected() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        // No seed_section → the claim's section_id is absent.
        let err =
            append_confirmation_event(&mut store, &path, sample_event("runA", "runB")).unwrap_err();
        assert!(
            matches!(err, AtomicMutateError::NotFound(_)),
            "R287 fail-loud: a claim about an unknown section must reject"
        );
    }

    #[test]
    fn confirmation_event_blank_rationale_rejected() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_section(&mut store, "sec");
        let mut ev = sample_event("runA", "runB");
        ev.rationale = "   ".to_string();
        let err = append_confirmation_event(&mut store, &path, ev).unwrap_err();
        assert!(
            matches!(err, AtomicMutateError::Validation(_)),
            "blank rationale must reject (sec 4.1)"
        );
    }

    #[test]
    fn confirmation_events_default_empty_on_legacy_store() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let legacy_json = r#"{ "sections": {}, "changelog_entries": {}, "schema_version": 9 }"#;
        std::fs::write(&path, legacy_json).unwrap();
        let loaded = AtomicStore::load(&path).unwrap();
        assert!(
            loaded.confirmation_events.is_empty(),
            "a missing confirmation_events key must default to empty"
        );
        assert_eq!(loaded.schema_version, 9, "version preserved on load");
        loaded.save(&path).unwrap();
        let reloaded = AtomicStore::load(&path).unwrap();
        assert_eq!(
            reloaded.schema_version, CURRENT_SCHEMA_VERSION,
            "save bumps the store to v10"
        );
    }

    #[test]
    fn confirmation_report_classifies_proposed_confirmed_refuted() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_section(&mut store, "sec");

        // One semantic confirm only → Proposed (no tool linkage yet).
        append_confirmation_event(&mut store, &path, sample_event("runA", "runB")).unwrap();
        let rep = confirmation_report(&store);
        assert_eq!(rep.claims.len(), 1);
        assert_eq!(rep.claims[0].status, ConfirmationStatus::Proposed);
        assert_eq!(rep.claims[0].independent_semantic, 1);

        // Add a deterministic tool linkage_check confirm → Confirmed.
        let mut tool = sample_event("runA", "runTool");
        tool.method = ConfirmMethod::LinkageCheck;
        tool.confirmer.kind = ConfirmerKind::Tool;
        append_confirmation_event(&mut store, &path, tool).unwrap();
        let rep = confirmation_report(&store);
        assert_eq!(rep.claims[0].status, ConfirmationStatus::Confirmed);
        assert!(rep.claims[0].has_tool_linkage);

        // A single refute blocks regardless (design sec 8).
        let mut refute = sample_event("runA", "runRefuter");
        refute.verdict = Verdict::Refute;
        refute.rationale = "the test does not exercise the requirement".to_string();
        append_confirmation_event(&mut store, &path, refute).unwrap();
        let rep = confirmation_report(&store);
        assert_eq!(rep.claims[0].status, ConfirmationStatus::Refuted);
    }

    #[test]
    fn confirmation_report_debt_excludes_confirmed() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_section(&mut store, "sec");
        // A Proposed claim (semantic only) is on the debt queue.
        append_confirmation_event(&mut store, &path, sample_event("runA", "runB")).unwrap();
        assert_eq!(confirmation_report(&store).debt().count(), 1);
    }

    #[test]
    fn confirmation_report_with_drift_marks_stale() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_section(&mut store, "sec");
        let mut tool = sample_event("runA", "runTool");
        tool.method = ConfirmMethod::LinkageCheck;
        tool.confirmer.kind = ConfirmerKind::Tool;
        append_confirmation_event(&mut store, &path, tool).unwrap();
        append_confirmation_event(&mut store, &path, sample_event("runA", "runB")).unwrap();
        // All events valid → Confirmed.
        assert_eq!(
            confirmation_report(&store).claims[0].status,
            ConfirmationStatus::Confirmed
        );
        // Drift everything (R420): the confirms drop out → Stale, not Proposed.
        let rep = confirmation_report_with(&store, |_| false);
        assert_eq!(rep.claims[0].status, ConfirmationStatus::Stale);
        assert_eq!(rep.claims[0].stale_count, 2);
        assert_eq!(rep.claims[0].confirm_count, 0, "valid confirms excluded");
    }

    #[test]
    fn verification_expectation_round_trips_and_skips_default() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_section(&mut store, "dedicated");
        seed_section(&mut store, "bycon");
        // The set persists the whole store (default `dedicated` section included).
        set_section_verification_expectation(
            &mut store,
            &path,
            "bycon",
            mnemosyne_core::VerificationExpectation::ByConstruction,
            "transcribed pseudocode, holistic coverage",
        )
        .unwrap();
        let reloaded = AtomicStore::load(&path).unwrap();
        assert_eq!(
            reloaded.sections["bycon"].verification_expectation,
            mnemosyne_core::VerificationExpectation::ByConstruction,
            "ByConstruction must round-trip"
        );
        assert_eq!(
            reloaded.sections["dedicated"].verification_expectation,
            mnemosyne_core::VerificationExpectation::Dedicated,
            "default Dedicated must reload from an omitted key"
        );
        // Default `Dedicated` is skipped on disk; `ByConstruction` is persisted.
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(
            raw.contains("by_construction"),
            "ByConstruction must persist on disk: {raw}"
        );
    }

    #[test]
    fn save_load_round_trip() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_section(&mut store, "43");
        set_section_intent(&mut store, &path, "43", "test intent").unwrap();
        let loaded = AtomicStore::load(&path).unwrap();
        assert_eq!(
            loaded.section("43").unwrap().intent.as_deref(),
            Some("test intent")
        );
    }

    #[test]
    fn intent_threshold_rejects() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        let too_long = "x".repeat(MAX_INTENT_CHAR + 1);
        let err = set_section_intent(&mut store, &path, "43", &too_long).unwrap_err();
        match err {
            AtomicMutateError::Validation(_) => {}
            other => panic!("expected Validation, got {:?}", other),
        }
    }

    #[test]
    fn rationale_bullet_threshold_rejects() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        let too_long = vec!["x".repeat(MAX_BULLET_CHAR + 1)];
        let err = set_section_rationale(&mut store, &path, "43", &too_long).unwrap_err();
        match err {
            AtomicMutateError::Validation(_) => {}
            other => panic!("expected Validation, got {:?}", other),
        }
    }

    #[test]
    fn changelog_entry_frozen_after_append() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        append_changelog_entry(
            &mut store,
            &path,
            ChangelogEntryDraft {
                entry_id: "Round 162",
                decision_summary: Some("test summary"),
                changes_bullets: &["change 1".into()],
                verification_bullets: &["verify 1".into()],
                impact_refs: &["43".into()],
                carry_forward_bullets: &["carry 1".into()],
            },
            "Round ",
        )
        .unwrap();
        let err = append_changelog_entry(
            &mut store,
            &path,
            ChangelogEntryDraft {
                entry_id: "Round 162",
                decision_summary: Some("attempted overwrite"),
                changes_bullets: &[],
                verification_bullets: &[],
                impact_refs: &[],
                carry_forward_bullets: &[],
            },
            "Round ",
        )
        .unwrap_err();
        match err {
            AtomicMutateError::FrozenLedger(_) => {}
            other => panic!("expected FrozenLedger, got {:?}", other),
        }
    }

    #[test]
    fn empty_store_load_when_missing() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("no_such.json");
        let store = AtomicStore::load(&path).unwrap();
        assert!(store.sections.is_empty());
        assert!(store.changelog_entries.is_empty());
        assert!(store.inventory_entries.is_empty());
    }

    #[test]
    fn schema_version_1_store_loads_with_empty_inventory() {
        // Back-compat: a store written under schema-version 1 (pre-Round 273)
        // has no inventory_entries field. Load must succeed; the field defaults
        // to empty via #[serde(default)]; subsequent save rewrites to v2.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let legacy_json = r#"{
 "sections": {},
 "changelog_entries": {},
 "schema_version": 1
 }"#;
        std::fs::write(&path, legacy_json).unwrap();
        let loaded = AtomicStore::load(&path).unwrap();
        assert!(loaded.inventory_entries.is_empty());
        assert_eq!(loaded.schema_version, 1);
        loaded.save(&path).unwrap();
        let reloaded = AtomicStore::load(&path).unwrap();
        assert_eq!(reloaded.schema_version, CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn schema_version_2_store_loads_with_empty_outline_fields() {
        // Round 287 back-compat: a v2 store (pre-outline-lift) deserializes
        // with empty AtomicSection.title / .parent_doc + parent_section = None.
        // Next save rewrites to v3.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let schema_version_2_json = r#"{
 "sections": {
 "39": {
 "intent": "old-shape section without outline"
 }
 },
 "changelog_entries": {},
 "inventory_entries": {},
 "schema_version": 2
 }"#;
        std::fs::write(&path, schema_version_2_json).unwrap();
        let loaded = AtomicStore::load(&path).unwrap();
        let s = loaded.sections.get("39").expect("§39 present");
        assert_eq!(s.skeleton.title, "", "title defaults to empty pre-backfill");
        assert_eq!(
            s.skeleton.parent_doc, "",
            "parent_doc defaults to empty pre-backfill"
        );
        assert_eq!(
            s.skeleton.parent_section, None,
            "parent_section defaults to None"
        );
        assert_eq!(
            s.intent.as_deref(),
            Some("old-shape section without outline")
        );
        loaded.save(&path).unwrap();
        let reloaded = AtomicStore::load(&path).unwrap();
        assert_eq!(reloaded.schema_version, CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn schema_version_3_clones_audit_into_publishable_on_load() {
        // Round 294 v3→v4 migration: a v3 store has audit_* fields populated
        // but no publishable_* fields. Loading must clone audit_* into
        // publishable_* per entry so the render shape stays byte-identical.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let schema_version_3_json = r#"{
 "sections": {},
 "changelog_entries": {
 "Round 200": {
 "decision_summary": "v3 entry summary",
 "changes_bullets": ["v3 change A", "v3 change B"],
 "verification_bullets": ["v3 verify A"],
 "impact_refs": ["43"],
 "carry_forward_bullets": ["v3 carry A"]
 }
 },
 "inventory_entries": {},
 "schema_version": 3
}"#;
        std::fs::write(&path, schema_version_3_json).unwrap();
        let loaded = AtomicStore::load(&path).unwrap();
        let entry = loaded
            .changelog_entries
            .get("Round 200")
            .expect("Round 200 entry present");
        assert_eq!(
            entry.publishable_decision_summary.as_deref(),
            Some("v3 entry summary"),
            "publishable_decision_summary must clone from audit on v3 load"
        );
        assert_eq!(
            entry.publishable_changes_bullets,
            vec!["v3 change A".to_string(), "v3 change B".to_string()]
        );
        assert_eq!(
            entry.publishable_verification_bullets,
            vec!["v3 verify A".to_string()]
        );
        assert_eq!(entry.publishable_impact_refs, vec!["43".to_string()]);
        assert_eq!(
            entry.publishable_carry_forward_bullets,
            vec!["v3 carry A".to_string()]
        );
        assert!(
            entry.publishable_matches_audit(),
            "post-migration default: publishable matches audit"
        );
        // Save then reload: publishable_* now persisted, schema bumps to CURRENT.
        loaded.save(&path).unwrap();
        let reloaded = AtomicStore::load(&path).unwrap();
        assert_eq!(reloaded.schema_version, CURRENT_SCHEMA_VERSION);
        let reloaded_entry = reloaded.changelog_entries.get("Round 200").unwrap();
        assert!(reloaded_entry.publishable_matches_audit());
    }

    #[test]
    fn schema_version_4_preserves_publishable_divergence_on_load() {
        // Round 294 invariant: a v4 store with publishable_* explicitly
        // diverged from audit_* must NOT be clone-overwritten on load. The
        // split exists precisely so redaction / typo fix can persist a
        // divergent published view (audit half stays as the permanent record).
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let v4_json = r#"{
 "sections": {},
 "changelog_entries": {
 "Round 200": {
 "decision_summary": "audit secret token XYZ123",
 "changes_bullets": ["audit references XYZ123 verbatim"],
 "verification_bullets": ["audit verify"],
 "impact_refs": ["43"],
 "carry_forward_bullets": ["audit carry"],
 "publishable_decision_summary": "redacted summary",
 "publishable_changes_bullets": ["redacted change A"],
 "publishable_verification_bullets": ["audit verify"],
 "publishable_impact_refs": ["43"],
 "publishable_carry_forward_bullets": ["audit carry"]
 }
 },
 "inventory_entries": {},
 "schema_version": 4
}"#;
        std::fs::write(&path, v4_json).unwrap();
        let loaded = AtomicStore::load(&path).unwrap();
        let entry = loaded.changelog_entries.get("Round 200").unwrap();
        assert_eq!(
            entry.decision_summary.as_deref(),
            Some("audit secret token XYZ123"),
            "audit half preserved verbatim"
        );
        assert_eq!(
            entry.publishable_decision_summary.as_deref(),
            Some("redacted summary"),
            "publishable divergence preserved across load"
        );
        assert!(
            !entry.publishable_matches_audit(),
            "intentional divergence retained, not overwritten"
        );
    }

    #[test]
    fn append_changelog_entry_clones_audit_into_publishable() {
        // Round 294 default: append_changelog_entry initializes
        // publishable_* = audit_* clone so newly authored entries render
        // byte-identical to pre-R294 baseline until R295 setters diverge them.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        append_changelog_entry(
            &mut store,
            &path,
            ChangelogEntryDraft {
                entry_id: "Round 999",
                decision_summary: Some("appended summary"),
                changes_bullets: &["appended change".into()],
                verification_bullets: &["appended verify".into()],
                impact_refs: &["43".into()],
                carry_forward_bullets: &["appended carry".into()],
            },
            "Round ",
        )
        .unwrap();
        let entry = store.changelog_entries.get("Round 999").unwrap();
        assert!(entry.publishable_matches_audit());
        assert_eq!(
            entry.publishable_decision_summary.as_deref(),
            Some("appended summary")
        );
    }

    // ============ Round 298 silent-accept gate ============
    //
    // The 6 tests below pin the primitive-boundary guard added in R298:
    // entry-id alone with an all-empty body must be rejected so neither CLI
    // (`mnemosyne-cli append-changelog-entry --entry-id X` with no other
    // flags) nor a future MCP wire can land a record-less ChangelogEntry into
    // the frozen ledger. FrozenLedger still wins over Validation (existing
    // frozen-after-append test calls with all-empty body intentionally).

    fn append_with_empty_body(
        store: &mut AtomicStore,
        path: &Path,
        entry_id: &str,
    ) -> Result<AtomicMutateReceipt, AtomicMutateError> {
        append_changelog_entry(
            store,
            path,
            ChangelogEntryDraft {
                entry_id,
                decision_summary: None,
                changes_bullets: &[],
                verification_bullets: &[],
                impact_refs: &[],
                carry_forward_bullets: &[],
            },
            "Round ",
        )
    }

    #[test]
    fn r298_blank_entry_id_rejected() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        let err = append_with_empty_body(&mut store, &path, "   ").unwrap_err();
        match err {
            AtomicMutateError::Validation(msg) => {
                assert!(msg.contains("entry_id"), "msg={}", msg);
                assert!(msg.contains("Round 298"), "msg={}", msg);
            }
            other => panic!("expected Validation, got {:?}", other),
        }
    }

    #[test]
    fn r298_missing_decision_summary_rejected() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        let err = append_with_empty_body(&mut store, &path, "Round 999").unwrap_err();
        match err {
            AtomicMutateError::Validation(msg) => {
                assert!(msg.contains("decision_summary"), "msg={}", msg);
            }
            other => panic!("expected Validation, got {:?}", other),
        }
    }

    #[test]
    fn r298_empty_changes_bullets_rejected() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        let err = append_changelog_entry(
            &mut store,
            &path,
            ChangelogEntryDraft {
                entry_id: "Round 999",
                decision_summary: Some("decision"),
                changes_bullets: &[],
                verification_bullets: &["verify".into()],
                impact_refs: &[],
                carry_forward_bullets: &[],
            },
            "Round ",
        )
        .unwrap_err();
        match err {
            AtomicMutateError::Validation(msg) => {
                assert!(msg.contains("changes_bullets"), "msg={}", msg);
            }
            other => panic!("expected Validation, got {:?}", other),
        }
    }

    #[test]
    fn r298_empty_verification_bullets_rejected() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        let err = append_changelog_entry(
            &mut store,
            &path,
            ChangelogEntryDraft {
                entry_id: "Round 999",
                decision_summary: Some("decision"),
                changes_bullets: &["change".into()],
                verification_bullets: &[],
                impact_refs: &[],
                carry_forward_bullets: &[],
            },
            "Round ",
        )
        .unwrap_err();
        match err {
            AtomicMutateError::Validation(msg) => {
                assert!(msg.contains("verification_bullets"), "msg={}", msg);
            }
            other => panic!("expected Validation, got {:?}", other),
        }
    }

    #[test]
    fn r298_blank_change_bullet_element_rejected() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        let err = append_changelog_entry(
            &mut store,
            &path,
            ChangelogEntryDraft {
                entry_id: "Round 999",
                decision_summary: Some("decision"),
                changes_bullets: &["valid".into(), "   ".into()],
                verification_bullets: &["verify".into()],
                impact_refs: &[],
                carry_forward_bullets: &[],
            },
            "Round ",
        )
        .unwrap_err();
        match err {
            AtomicMutateError::Validation(msg) => {
                assert!(msg.contains("changes_bullets[1]"), "msg={}", msg);
                assert!(msg.contains("blank"), "msg={}", msg);
            }
            other => panic!("expected Validation, got {:?}", other),
        }
    }

    #[test]
    fn r298_blank_optional_element_rejected() {
        // impact_refs and carry_forward_bullets are optional as a vec (empty
        // OK) but a present blank element is still a hygiene reject.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        let err = append_changelog_entry(
            &mut store,
            &path,
            ChangelogEntryDraft {
                entry_id: "Round 999",
                decision_summary: Some("decision"),
                changes_bullets: &["change".into()],
                verification_bullets: &["verify".into()],
                impact_refs: &["".into()],
                carry_forward_bullets: &[],
            },
            "Round ",
        )
        .unwrap_err();
        match err {
            AtomicMutateError::Validation(msg) => {
                assert!(msg.contains("impact_refs[0]"), "msg={}", msg);
            }
            other => panic!("expected Validation, got {:?}", other),
        }
    }

    // ============ Round 424 entry_id_prefix conformance gate ============
    //
    // The 4 tests below pin the append-time prefix gate: a workspace with a
    // non-empty `schema.entry_id_prefix` rejects entry ids that do not start
    // with it (the canonical miss is an accidental bare `test` append during
    // an error bisection). Empty prefix = gate disabled (generic preset);
    // FrozenLedger still wins so pre-gate non-conforming entries stay frozen
    // history rather than re-classifying as Validation rejects.

    fn append_minimal(
        store: &mut AtomicStore,
        path: &Path,
        entry_id: &str,
        entry_id_prefix: &str,
    ) -> Result<AtomicMutateReceipt, AtomicMutateError> {
        append_changelog_entry(
            store,
            path,
            ChangelogEntryDraft {
                entry_id,
                decision_summary: Some("decision"),
                changes_bullets: &["change".into()],
                verification_bullets: &["verify".into()],
                impact_refs: &[],
                carry_forward_bullets: &[],
            },
            entry_id_prefix,
        )
    }

    #[test]
    fn r424_nonconforming_entry_id_rejected() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        let err = append_minimal(&mut store, &path, "test", "Round ").unwrap_err();
        match err {
            AtomicMutateError::Validation(msg) => {
                assert!(msg.contains("entry_id_prefix"), "msg={}", msg);
                assert!(msg.contains("`test`"), "msg={}", msg);
            }
            other => panic!("expected Validation, got {:?}", other),
        }
        assert!(store.changelog_entries.is_empty(), "reject must not insert");
    }

    #[test]
    fn r424_conforming_entry_id_accepted() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        append_minimal(&mut store, &path, "Round 424", "Round ").unwrap();
        assert!(store.changelog_entries.contains_key("Round 424"));
    }

    #[test]
    fn r424_empty_prefix_disables_gate() {
        // Generic preset: entry_id_prefix = "" means no numeric entry_id
        // convention exists; the gate must not invent one.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        append_minimal(&mut store, &path, "test", "").unwrap();
        assert!(store.changelog_entries.contains_key("test"));
    }

    #[test]
    fn r424_frozen_ledger_wins_over_prefix_gate() {
        // A pre-gate non-conforming entry (seeded with the gate disabled)
        // re-appended under an enforcing prefix must surface FrozenLedger,
        // not Validation — the duplicate is the stronger fact.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        append_minimal(&mut store, &path, "test", "").unwrap();
        let err = append_minimal(&mut store, &path, "test", "Round ").unwrap_err();
        match err {
            AtomicMutateError::FrozenLedger(_) => {}
            other => panic!("expected FrozenLedger, got {:?}", other),
        }
    }

    #[test]
    fn inventory_entry_round_trip() {
        // Direct insertion round-trips through save/load preserving status,
        // section_ref, source, reason. Mutate primitives land in Round 274; this
        // test exercises the schema shape, not authoring ergonomics.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        store.inventory_entries.insert(
            "ARP_07".to_string(),
            InventoryEntry {
                status: InventoryStatus::Active,
                section_ref: Some("4.2.4".to_string()),
                source: Some("tc8_p041-p060.pdf#row=12".to_string()),
                reason: None,
            },
        );
        store.inventory_entries.insert(
            "TCP_RETRANSMISSION_TO_04".to_string(),
            InventoryEntry {
                status: InventoryStatus::Deprecated,
                section_ref: Some("4.8.6.11".to_string()),
                source: None,
                reason: Some("superseded by RETRANSMISSION_TO_05 in TC8 v2.3".to_string()),
            },
        );
        store.save(&path).unwrap();
        let loaded = AtomicStore::load(&path).unwrap();
        let arp = loaded.inventory("ARP_07").expect("ARP_07 not found");
        assert_eq!(arp.status, InventoryStatus::Active);
        assert_eq!(arp.section_ref.as_deref(), Some("4.2.4"));
        let tcp = loaded.inventory("TCP_RETRANSMISSION_TO_04").unwrap();
        assert_eq!(tcp.status, InventoryStatus::Deprecated);
        assert!(tcp.reason.as_ref().unwrap().contains("v2.3"));
        assert!(loaded.inventory("ARP_99").is_none());
    }

    #[test]
    fn add_inventory_entry_basic_round_trip() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        add_inventory_entry(
            &mut store,
            &path,
            "ARP_07",
            InventoryStatus::Active,
            Some("4.2.4"),
            Some("tc8_p041-p060.pdf#row=12"),
            None,
        )
        .unwrap();
        let loaded = AtomicStore::load(&path).unwrap();
        let e = loaded.inventory("ARP_07").unwrap();
        assert_eq!(e.status, InventoryStatus::Active);
        assert_eq!(e.section_ref.as_deref(), Some("4.2.4"));
        assert_eq!(e.source.as_deref(), Some("tc8_p041-p060.pdf#row=12"));
        assert!(e.reason.is_none());
    }

    #[test]
    fn add_inventory_entry_rejects_duplicate() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        add_inventory_entry(
            &mut store,
            &path,
            "ARP_07",
            InventoryStatus::Active,
            None,
            None,
            None,
        )
        .unwrap();
        let err = add_inventory_entry(
            &mut store,
            &path,
            "ARP_07",
            InventoryStatus::Deprecated,
            None,
            None,
            None,
        )
        .unwrap_err();
        match err {
            AtomicMutateError::Validation(msg) => {
                assert!(msg.contains("already registered"), "got: {}", msg)
            }
            other => panic!("expected Validation, got {:?}", other),
        }
    }

    #[test]
    fn add_inventory_entry_rejects_invalid_id() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        // empty
        assert!(matches!(
            add_inventory_entry(
                &mut store,
                &path,
                "",
                InventoryStatus::Active,
                None,
                None,
                None
            ),
            Err(AtomicMutateError::Validation(_))
        ));
        // whitespace edges
        assert!(matches!(
            add_inventory_entry(
                &mut store,
                &path,
                " ARP_07",
                InventoryStatus::Active,
                None,
                None,
                None
            ),
            Err(AtomicMutateError::Validation(_))
        ));
        // internal whitespace
        assert!(matches!(
            add_inventory_entry(
                &mut store,
                &path,
                "ARP 07",
                InventoryStatus::Active,
                None,
                None,
                None
            ),
            Err(AtomicMutateError::Validation(_))
        ));
    }

    #[test]
    fn add_inventory_entry_rejects_section_ref_with_section_sigil() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        // CLI strips `§`; the mutate API must reject pre-stripped form to fail
        // loud on a caller bypassing the CLI layer.
        let err = add_inventory_entry(
            &mut store,
            &path,
            "ARP_07",
            InventoryStatus::Active,
            Some("§4.2.4"),
            None,
            None,
        )
        .unwrap_err();
        match err {
            AtomicMutateError::Validation(msg) => {
                assert!(msg.contains("drop leading `§`"), "got: {}", msg)
            }
            other => panic!("expected Validation, got {:?}", other),
        }
    }

    #[test]
    fn set_inventory_status_active_to_deprecated_with_reason() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        add_inventory_entry(
            &mut store,
            &path,
            "TCP_X",
            InventoryStatus::Active,
            None,
            None,
            None,
        )
        .unwrap();
        set_inventory_status(
            &mut store,
            &path,
            "TCP_X",
            InventoryStatus::Deprecated,
            Some("superseded by TCP_Y in TC8 v2.3"),
        )
        .unwrap();
        let loaded = AtomicStore::load(&path).unwrap();
        let e = loaded.inventory("TCP_X").unwrap();
        assert_eq!(e.status, InventoryStatus::Deprecated);
        assert!(e.reason.as_ref().unwrap().contains("v2.3"));
    }

    #[test]
    fn set_inventory_status_reason_none_preserves_existing() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        add_inventory_entry(
            &mut store,
            &path,
            "TCP_X",
            InventoryStatus::Deprecated,
            None,
            None,
            Some("initial reason"),
        )
        .unwrap();
        set_inventory_status(&mut store, &path, "TCP_X", InventoryStatus::Active, None).unwrap();
        let loaded = AtomicStore::load(&path).unwrap();
        let e = loaded.inventory("TCP_X").unwrap();
        assert_eq!(e.status, InventoryStatus::Active);
        assert_eq!(e.reason.as_deref(), Some("initial reason"));
    }

    #[test]
    fn set_inventory_status_reason_empty_string_clears() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        add_inventory_entry(
            &mut store,
            &path,
            "TCP_X",
            InventoryStatus::Deprecated,
            None,
            None,
            Some("initial reason"),
        )
        .unwrap();
        set_inventory_status(
            &mut store,
            &path,
            "TCP_X",
            InventoryStatus::Reserved,
            Some(""),
        )
        .unwrap();
        let loaded = AtomicStore::load(&path).unwrap();
        let e = loaded.inventory("TCP_X").unwrap();
        assert!(e.reason.is_none());
    }

    #[test]
    fn set_inventory_status_not_found_returns_not_found() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        let err = set_inventory_status(&mut store, &path, "ARP_99", InventoryStatus::Active, None)
            .unwrap_err();
        assert!(matches!(err, AtomicMutateError::NotFound(_)));
    }

    #[test]
    fn set_inventory_section_ref_basic_and_clear() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        add_inventory_entry(
            &mut store,
            &path,
            "ARP_07",
            InventoryStatus::Active,
            None,
            None,
            None,
        )
        .unwrap();
        set_inventory_section_ref(&mut store, &path, "ARP_07", Some("4.2.4")).unwrap();
        assert_eq!(
            AtomicStore::load(&path)
                .unwrap()
                .inventory("ARP_07")
                .unwrap()
                .section_ref
                .as_deref(),
            Some("4.2.4")
        );
        set_inventory_section_ref(&mut store, &path, "ARP_07", None).unwrap();
        assert!(AtomicStore::load(&path)
            .unwrap()
            .inventory("ARP_07")
            .unwrap()
            .section_ref
            .is_none());
    }

    #[test]
    fn set_inventory_section_ref_not_found_returns_not_found() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        let err =
            set_inventory_section_ref(&mut store, &path, "ARP_99", Some("4.2.4")).unwrap_err();
        assert!(matches!(err, AtomicMutateError::NotFound(_)));
    }

    #[test]
    fn remove_inventory_entry_basic() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        add_inventory_entry(
            &mut store,
            &path,
            "ARP_07",
            InventoryStatus::Active,
            None,
            None,
            None,
        )
        .unwrap();
        remove_inventory_entry(
            &mut store,
            &path,
            "ARP_07",
            "deprecated upstream in TC8 v2.4",
        )
        .unwrap();
        let loaded = AtomicStore::load(&path).unwrap();
        assert!(loaded.inventory("ARP_07").is_none());
    }

    #[test]
    fn remove_inventory_entry_rejects_empty_reason() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        add_inventory_entry(
            &mut store,
            &path,
            "ARP_07",
            InventoryStatus::Active,
            None,
            None,
            None,
        )
        .unwrap();
        let err = remove_inventory_entry(&mut store, &path, "ARP_07", "  ").unwrap_err();
        assert!(matches!(err, AtomicMutateError::Validation(_)));
    }

    #[test]
    fn remove_inventory_entry_not_found() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        let err = remove_inventory_entry(&mut store, &path, "ARP_99", "any reason").unwrap_err();
        assert!(matches!(err, AtomicMutateError::NotFound(_)));
    }

    #[test]
    fn atomic_inventory_id_set_returns_inventory_keys() {
        // Parallel to atomic_section_id_set_returns_section_keys: cite-time
        // existence check substrate (Round 275 lookup foundation).
        let mut store = AtomicStore::new();
        store
            .inventory_entries
            .insert("ARP_07".to_string(), InventoryEntry::default());
        store.inventory_entries.insert(
            "TCP_FLAGS_INVALID_02".to_string(),
            InventoryEntry::default(),
        );
        store.inventory_entries.insert(
            "SOMEIP_ETS_BASICS_01".to_string(),
            InventoryEntry::default(),
        );
        let id_set = store.atomic_inventory_id_set();
        assert_eq!(id_set.len(), 3);
        assert!(id_set.contains("ARP_07"));
        assert!(id_set.contains("TCP_FLAGS_INVALID_02"));
        assert!(id_set.contains("SOMEIP_ETS_BASICS_01"));
        assert!(!id_set.contains("ARP_99"));
    }

    #[test]
    fn atomic_section_id_set_returns_section_keys() {
        // MD-DELETION-RATIFY foundation: atomic store in section
        // keys only source as one section_id set carry.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_section(&mut store, "39");
        seed_section(&mut store, "41");
        seed_section(&mut store, "66");
        set_section_intent(&mut store, &path, "39", "graph schema").unwrap();
        set_section_intent(&mut store, &path, "41", "datalog rule").unwrap();
        set_section_intent(&mut store, &path, "66", "self-application").unwrap();
        let id_set = store.atomic_section_id_set();
        assert_eq!(id_set.len(), 3);
        assert!(id_set.contains("39"));
        assert!(id_set.contains("41"));
        assert!(id_set.contains("66"));
    }

    #[test]
    fn add_section_binding_basic_round_trip() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_section(&mut store, "39");
        add_section_binding(
            &mut store,
            &path,
            "39",
            "crates/mnemosyne-atomic/src/lib.rs",
            Some("AtomicSection"),
            BindingKind::Implements,
        )
        .unwrap();
        add_section_binding(
            &mut store,
            &path,
            "39",
            "crates/mnemosyne-cli/src/atomic_cli.rs",
            None,
            BindingKind::Implements,
        )
        .unwrap();
        let loaded = AtomicStore::load(&path).unwrap();
        let impls = &loaded.section("39").unwrap().bindings;
        assert_eq!(impls.len(), 2);
        assert_eq!(impls[0].file, "crates/mnemosyne-atomic/src/lib.rs");
        assert_eq!(impls[0].symbol.as_deref(), Some("AtomicSection"));
        assert_eq!(impls[1].file, "crates/mnemosyne-cli/src/atomic_cli.rs");
        assert!(impls[1].symbol.is_none());
    }

    #[test]
    fn add_section_binding_rejects_duplicate() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_section(&mut store, "39");
        add_section_binding(
            &mut store,
            &path,
            "39",
            "src/foo.rs",
            Some("bar"),
            BindingKind::Implements,
        )
        .unwrap();
        let err = add_section_binding(
            &mut store,
            &path,
            "39",
            "src/foo.rs",
            Some("bar"),
            BindingKind::Implements,
        )
        .unwrap_err();
        match err {
            AtomicMutateError::Validation(msg) => assert!(msg.contains("already present")),
            other => panic!("expected Validation, got {:?}", other),
        }
        // file-only vs symbol-qualified are distinct entries.
        add_section_binding(
            &mut store,
            &path,
            "39",
            "src/foo.rs",
            None,
            BindingKind::Implements,
        )
        .unwrap();
        assert_eq!(store.section("39").unwrap().bindings.len(), 2);
    }

    #[test]
    fn add_section_binding_rejects_malformed_file() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        let cases = [
            "",
            "   ",
            "/abs/path.rs",
            "./rel.rs",
            "a/../b.rs",
            "a\\b.rs",
            "a//b.rs",
            "dir/",
            " leading.rs",
            "trailing.rs ",
        ];
        for bad in cases {
            let err =
                add_section_binding(&mut store, &path, "39", bad, None, BindingKind::Implements)
                    .unwrap_err();
            assert!(
                matches!(err, AtomicMutateError::Validation(_)),
                "expected Validation for `{}`, got {:?}",
                bad,
                err
            );
        }
    }

    #[test]
    fn add_section_binding_rejects_malformed_symbol() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        // empty / whitespace symbol.
        for bad in ["", "   ", " sym", "sym ", "sym\nname"] {
            let err = add_section_binding(
                &mut store,
                &path,
                "39",
                "src/foo.rs",
                Some(bad),
                BindingKind::Implements,
            )
            .unwrap_err();
            assert!(
                matches!(err, AtomicMutateError::Validation(_)),
                "expected Validation for symbol `{:?}`, got {:?}",
                bad,
                err
            );
        }
    }

    #[test]
    fn add_section_binding_accepts_opaque_qualified_symbols() {
        // Symbol is opaque — language-agnostic. No grammar regex; any
        // non-empty trimmed string with no internal newline is accepted.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_section(&mut store, "39");
        for sym in [
            "foo",
            "module::path::foo",
            "Class.method",
            "Foo<T>::bar",
            "pkg/Type#method",
            "ns.sub.fn",
        ] {
            add_section_binding(
                &mut store,
                &path,
                "39",
                "src/foo.rs",
                Some(sym),
                BindingKind::Implements,
            )
            .unwrap();
        }
        assert_eq!(store.section("39").unwrap().bindings.len(), 6);
    }

    // Round 283 — remove_section_binding tests.

    #[test]
    fn remove_section_binding_basic_round_trip() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_section(&mut store, "X");
        add_section_binding(
            &mut store,
            &path,
            "X",
            "src/foo.rs",
            None,
            BindingKind::Implements,
        )
        .unwrap();
        assert_eq!(store.section("X").unwrap().bindings.len(), 1);
        remove_section_binding(&mut store, &path, "X", "src/foo.rs", None, "code moved").unwrap();
        let loaded = AtomicStore::load(&path).unwrap();
        assert_eq!(loaded.section("X").unwrap().bindings.len(), 0);
    }

    #[test]
    fn remove_section_binding_symbol_aware_match() {
        // (file, None) vs (file, Some("sym")) are distinct set elements;
        // removing the file-only row must NOT touch the symbol-narrowed row.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_section(&mut store, "X");
        add_section_binding(
            &mut store,
            &path,
            "X",
            "src/foo.rs",
            None,
            BindingKind::Implements,
        )
        .unwrap();
        add_section_binding(
            &mut store,
            &path,
            "X",
            "src/foo.rs",
            Some("fn_a"),
            BindingKind::Implements,
        )
        .unwrap();
        add_section_binding(
            &mut store,
            &path,
            "X",
            "src/foo.rs",
            Some("fn_b"),
            BindingKind::Implements,
        )
        .unwrap();
        remove_section_binding(&mut store, &path, "X", "src/foo.rs", None, "cleanup").unwrap();
        let loaded = AtomicStore::load(&path).unwrap();
        let impls = &loaded.section("X").unwrap().bindings;
        assert_eq!(impls.len(), 2, "only the file-only row should be removed");
        assert!(impls.iter().any(|i| i.symbol.as_deref() == Some("fn_a")));
        assert!(impls.iter().any(|i| i.symbol.as_deref() == Some("fn_b")));
    }

    #[test]
    fn remove_section_binding_section_not_found() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        let err = remove_section_binding(&mut store, &path, "ghost", "src/foo.rs", None, "x")
            .unwrap_err();
        assert!(matches!(err, AtomicMutateError::NotFound(_)));
    }

    #[test]
    fn remove_section_binding_impl_not_found() {
        // Section exists, but the (file, symbol) tuple does not — fail-loud
        // (no silent no-op).
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_section(&mut store, "X");
        add_section_binding(
            &mut store,
            &path,
            "X",
            "src/foo.rs",
            None,
            BindingKind::Implements,
        )
        .unwrap();
        let err =
            remove_section_binding(&mut store, &path, "X", "src/other.rs", None, "wrong file")
                .unwrap_err();
        assert!(matches!(err, AtomicMutateError::NotFound(_)));
    }

    #[test]
    fn remove_section_binding_rejects_empty_reason() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_section(&mut store, "X");
        add_section_binding(
            &mut store,
            &path,
            "X",
            "src/foo.rs",
            None,
            BindingKind::Implements,
        )
        .unwrap();
        let err =
            remove_section_binding(&mut store, &path, "X", "src/foo.rs", None, "  ").unwrap_err();
        assert!(matches!(err, AtomicMutateError::Validation(_)));
    }

    // ---- BindingKind tests (schema v5) ----

    #[test]
    fn add_section_binding_round_trips_kind() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_section(&mut store, "X");
        add_section_binding(
            &mut store,
            &path,
            "X",
            "src/scheduler.rs",
            Some("dispatch"),
            BindingKind::Implements,
        )
        .unwrap();
        add_section_binding(
            &mut store,
            &path,
            "X",
            "src/snapshot.rs",
            Some("delay_field"),
            BindingKind::References,
        )
        .unwrap();
        let loaded = AtomicStore::load(&path).unwrap();
        let bindings = &loaded.section("X").unwrap().bindings;
        assert_eq!(bindings.len(), 2);
        let implements = bindings
            .iter()
            .find(|b| b.file == "src/scheduler.rs")
            .unwrap();
        assert_eq!(implements.kind, BindingKind::Implements);
        let references = bindings
            .iter()
            .find(|b| b.file == "src/snapshot.rs")
            .unwrap();
        assert_eq!(references.kind, BindingKind::References);
    }

    #[test]
    fn set_section_binding_kind_reclassifies_in_place() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_section(&mut store, "X");
        add_section_binding(
            &mut store,
            &path,
            "X",
            "src/snapshot.rs",
            Some("delay_field"),
            BindingKind::Implements,
        )
        .unwrap();
        // Stage-B reclassification: a DTO field is «trace», not «satisfy».
        set_section_binding_kind(
            &mut store,
            &path,
            "X",
            "src/snapshot.rs",
            Some("delay_field"),
            BindingKind::References,
            "DTO field stores the delay; §6.2.3 satisfied by the scheduler",
        )
        .unwrap();
        let loaded = AtomicStore::load(&path).unwrap();
        let bindings = &loaded.section("X").unwrap().bindings;
        assert_eq!(
            bindings.len(),
            1,
            "reclassify mutates in place, no duplicate"
        );
        assert_eq!(bindings[0].kind, BindingKind::References);
    }

    // ---- CoverageExpectation tests (schema v6, Round 389) ----

    #[test]
    fn coverage_expectation_write_path_parity_setter_vs_import() {
        // The two write paths to AtomicSection.coverage_expectation —
        // set_section_coverage_expectation and import_sections — must accept the
        // same value set and store it identically (CLAUDE.md
        // half-enforced-invariant rule). CoverageExpectation is a closed enum, so
        // both accept every value by construction; this pins they never diverge.
        let tmp = TempDir::new().unwrap();
        // Path A: seed (Normative default) then setter to Informative.
        let path_a = tmp.path().join(".atomic/a.atomic.json");
        let mut a = AtomicStore::new();
        seed_section(&mut a, "X");
        set_section_coverage_expectation(
            &mut a,
            &path_a,
            "X",
            mnemosyne_core::CoverageExpectation::OutOfScopeHere,
            "terminology section, nothing to implement",
        )
        .unwrap();
        // Path B: import with the classification inline.
        let path_b = tmp.path().join(".atomic/b.atomic.json");
        let mut b = AtomicStore::new();
        import_sections(
            &mut b,
            &path_b,
            &[SectionImport {
                section_id: "X".to_string(),
                parent_doc: "docs/GENERATED.md".to_string(),
                title: "X".to_string(),
                parent_section: None,
                normative_excerpt: None,
                coverage_expectation: mnemosyne_core::CoverageExpectation::OutOfScopeHere,
            }],
        )
        .unwrap();
        assert_eq!(
            a.section("X").unwrap().coverage_expectation,
            mnemosyne_core::CoverageExpectation::OutOfScopeHere,
        );
        assert_eq!(
            a.section("X").unwrap().coverage_expectation,
            b.section("X").unwrap().coverage_expectation,
        );
    }

    #[test]
    fn coverage_expectation_defaults_normative_and_round_trips() {
        // A section with no coverage_expectation on disk (the pre-v6 / default
        // shape) loads as Normative — the behavior-preserving migration. An
        // Informative classification persists and round-trips.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/ws.atomic.json");
        let mut store = AtomicStore::new();
        seed_section(&mut store, "Norm");
        seed_section(&mut store, "Info");
        set_section_coverage_expectation(
            &mut store,
            &path,
            "Info",
            mnemosyne_core::CoverageExpectation::OutOfScopeHere,
            "overview prose",
        )
        .unwrap();
        let json = std::fs::read_to_string(&path).unwrap();
        assert!(
            json.contains("\"out_of_scope_here\""),
            "OutOfScopeHere deviation persists under the canonical 3-state tag"
        );
        let reloaded = AtomicStore::load(&path).unwrap();
        assert_eq!(
            reloaded.section("Norm").unwrap().coverage_expectation,
            mnemosyne_core::CoverageExpectation::Normative,
            "unclassified section defaults Normative"
        );
        assert_eq!(
            reloaded.section("Info").unwrap().coverage_expectation,
            mnemosyne_core::CoverageExpectation::OutOfScopeHere,
        );
    }

    #[test]
    fn coverage_expectation_legacy_informative_tag_now_rejects() {
        // R422 — clean break: the `informative` alias was removed, so a
        // pre-3-state store carrying it fails to load LOUDLY (an unknown enum tag
        // errors — NOT a silent drop). A consumer migrates `informative` →
        // `out_of_scope_here` before bumping, rather than relying on a compat shim.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/ws.atomic.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let legacy = r#"{
            "sections": { "Info": { "title": "T", "parent_doc": "d",
                "coverage_expectation": "informative" } },
            "changelog_entries": {},
            "schema_version": 9
        }"#;
        std::fs::write(&path, legacy).unwrap();
        assert!(
            AtomicStore::load(&path).is_err(),
            "legacy `informative` no longer deserializes (R422 clean break)"
        );
    }

    #[test]
    fn set_section_binding_kind_rejects_empty_reason_and_missing_binding() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_section(&mut store, "X");
        // empty reason
        let err = set_section_binding_kind(
            &mut store,
            &path,
            "X",
            "src/foo.rs",
            None,
            BindingKind::References,
            "   ",
        )
        .unwrap_err();
        assert!(matches!(err, AtomicMutateError::Validation(_)));
        // binding absent → NotFound (no silent create)
        let err = set_section_binding_kind(
            &mut store,
            &path,
            "X",
            "src/foo.rs",
            None,
            BindingKind::References,
            "real reason",
        )
        .unwrap_err();
        assert!(matches!(err, AtomicMutateError::NotFound(_)));
    }

    /// CLAUDE.md half-enforced-invariant mandate (R295/R305): `add_section_binding`
    /// and `set_section_binding_kind` are two write paths to `Binding.kind`. Feed
    /// both every closed-enum value and assert both accept it (the kind set never
    /// diverges between the two paths).
    #[test]
    fn binding_kind_write_path_parity() {
        for kind in [
            BindingKind::Implements,
            BindingKind::References,
            BindingKind::Verifies,
        ] {
            let tmp = TempDir::new().unwrap();
            let path = tmp.path().join(".atomic/workspace.atomic.json");
            // Path A: add with this kind.
            let mut store_a = AtomicStore::new();
            seed_section(&mut store_a, "X");
            let add = add_section_binding(&mut store_a, &path, "X", "src/f.rs", None, kind);
            // Path B: add with some other kind, then set to this kind.
            let other = match kind {
                BindingKind::Implements => BindingKind::References,
                BindingKind::References => BindingKind::Verifies,
                BindingKind::Verifies => BindingKind::Implements,
            };
            let path_b = tmp.path().join(".atomic/b.atomic.json");
            let mut store_b = AtomicStore::new();
            seed_section(&mut store_b, "X");
            add_section_binding(&mut store_b, &path_b, "X", "src/f.rs", None, other).unwrap();
            let set =
                set_section_binding_kind(&mut store_b, &path_b, "X", "src/f.rs", None, kind, "r");
            assert_eq!(
                add.is_ok(),
                set.is_ok(),
                "add and set must agree on accepting kind {:?}",
                kind
            );
            assert!(add.is_ok(), "closed enum value must be accepted by both");
            assert_eq!(store_a.section("X").unwrap().bindings[0].kind, kind);
            assert_eq!(store_b.section("X").unwrap().bindings[0].kind, kind);
        }
    }

    #[test]
    fn kind_migration_report_lists_legacy_bindings_then_none_after_save() {
        // A store deserialized from the pre-v5 JSON shape (field `implementations`,
        // no `kind`) migrates every binding to Implements and reports them.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let v4_json = r#"{
            "schema_version": 4,
            "sections": {
                "X": {
                    "skeleton": { "title": "X", "parent_doc": "d", "parent_section": null, "decision_status": "Active" },
                    "implementations": [
                        { "file": "src/a.rs", "symbol": "foo" },
                        { "file": "src/b.rs" }
                    ]
                }
            },
            "changelog_entries": {},
            "inventory_entries": {}
        }"#;
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, v4_json).unwrap();
        let loaded = AtomicStore::load(&path).unwrap();
        // bindings migrated (alias) + defaulted to Implements (serde default).
        let bindings = &loaded.section("X").unwrap().bindings;
        assert_eq!(bindings.len(), 2);
        assert!(bindings.iter().all(|b| b.kind == BindingKind::Implements));
        // report present while schema_version < 5.
        let report = loaded
            .kind_migration_report()
            .expect("pre-v5 store reports");
        assert_eq!(report.from_schema_version, 4);
        assert_eq!(report.rows.len(), 2);
        assert!(report
            .rows
            .iter()
            .all(|r| r.defaulted_kind == BindingKind::Implements));
        // After save (bumps to CURRENT) the report is gone — migration is one-time.
        loaded.save(&path).unwrap();
        let reloaded = AtomicStore::load(&path).unwrap();
        assert_eq!(reloaded.schema_version, CURRENT_SCHEMA_VERSION);
        assert!(reloaded.kind_migration_report().is_none());
    }

    #[test]
    fn remove_section_drops_entry_and_persists() {
        // Round 267 — remove_section deletes the section_id entry from the
        // store and persists the change. Subsequent section() returns None.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_section(&mut store, "doomed");
        set_section_intent(&mut store, &path, "doomed", "to be removed").unwrap();
        assert!(store.section("doomed").is_some());
        remove_section(&mut store, &path, "doomed", "smoke-test cleanup").unwrap();
        assert!(store.section("doomed").is_none());
        let reloaded = AtomicStore::load(&path).unwrap();
        assert!(reloaded.section("doomed").is_none());
    }

    #[test]
    fn remove_section_rejects_empty_reason() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_section(&mut store, "1");
        set_section_intent(&mut store, &path, "1", "x").unwrap();
        let err = remove_section(&mut store, &path, "1", "   ").unwrap_err();
        assert!(matches!(err, AtomicMutateError::Validation(_)));
        // Section unchanged after rejected mutate.
        assert!(store.section("1").is_some());
    }

    #[test]
    fn remove_section_returns_not_found_for_missing_id() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        let err = remove_section(&mut store, &path, "ghost", "no such section").unwrap_err();
        assert!(matches!(err, AtomicMutateError::NotFound(_)));
    }

    // Round 287 — atomic add_section primitive tests. Pairs with remove_section
    // tests above. Outline-lift carry closure (Phase C primitives).

    #[test]
    fn add_section_basic_creates_outline_and_persists() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        let receipt = add_section(
            &mut store,
            &path,
            "39",
            "docs/GENERATED.md",
            "Test Title",
            None,
        )
        .unwrap();
        assert_eq!(receipt.primitive, "add_section");
        assert_eq!(receipt.target_id, "39");
        let s = store.section("39").expect("§39 created");
        assert_eq!(s.skeleton.title, "Test Title");
        assert_eq!(s.skeleton.parent_doc, "docs/GENERATED.md");
        assert_eq!(s.skeleton.parent_section, None);
        assert_eq!(s.skeleton.decision_status, Some(DecisionStatus::Active));
        // Round-trip through sidecar.
        let reloaded = AtomicStore::load(&path).unwrap();
        let s2 = reloaded.section("39").unwrap();
        assert_eq!(s2.skeleton.title, "Test Title");
        assert_eq!(s2.skeleton.parent_doc, "docs/GENERATED.md");
    }

    #[test]
    fn add_section_rejects_empty_section_id() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        let err = add_section(&mut store, &path, "   ", "docs/X.md", "T", None).unwrap_err();
        assert!(matches!(err, AtomicMutateError::Validation(_)));
        assert!(
            store.sections.is_empty(),
            "no section created on rejected mutate"
        );
    }

    #[test]
    fn add_section_rejects_empty_parent_doc() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        let err = add_section(&mut store, &path, "39", "", "T", None).unwrap_err();
        assert!(matches!(err, AtomicMutateError::Validation(_)));
    }

    #[test]
    fn add_section_rejects_empty_title() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        let err = add_section(&mut store, &path, "39", "docs/X.md", "  ", None).unwrap_err();
        assert!(matches!(err, AtomicMutateError::Validation(_)));
    }

    #[test]
    fn add_section_rejects_duplicate_section_id() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        add_section(&mut store, &path, "39", "docs/X.md", "First", None).unwrap();
        let err = add_section(&mut store, &path, "39", "docs/X.md", "Second", None).unwrap_err();
        match err {
            AtomicMutateError::Validation(msg) => assert!(msg.contains("already exists")),
            other => panic!("expected Validation, got {:?}", other),
        }
        // Original section unchanged.
        assert_eq!(store.section("39").unwrap().skeleton.title, "First");
    }

    #[test]
    fn add_section_rejects_missing_parent_section() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        let err =
            add_section(&mut store, &path, "39.1", "docs/X.md", "Child", Some("39")).unwrap_err();
        assert!(matches!(err, AtomicMutateError::NotFound(_)));
    }

    #[test]
    fn add_section_with_existing_parent_succeeds() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        add_section(&mut store, &path, "39", "docs/X.md", "Parent", None).unwrap();
        add_section(&mut store, &path, "39.1", "docs/X.md", "Child", Some("39")).unwrap();
        let child = store.section("39.1").expect("§39.1 created");
        assert_eq!(child.skeleton.parent_section.as_deref(), Some("39"));
        assert_eq!(child.skeleton.title, "Child");
    }

    #[test]
    fn add_section_remove_section_symmetric_round_trip() {
        // add_section + remove_section pair: state returns to the empty baseline.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        add_section(&mut store, &path, "100", "docs/X.md", "ephemeral", None).unwrap();
        assert!(store.section("100").is_some());
        remove_section(&mut store, &path, "100", "test cleanup").unwrap();
        assert!(store.section("100").is_none());
        let reloaded = AtomicStore::load(&path).unwrap();
        assert!(reloaded.section("100").is_none());
    }

    // Round 287 — outline set_* primitive tests (Phase C).

    #[test]
    fn set_section_title_basic_and_round_trip() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        add_section(&mut store, &path, "39", "docs/X.md", "old title", None).unwrap();
        set_section_title(&mut store, &path, "39", "new title").unwrap();
        assert_eq!(store.section("39").unwrap().skeleton.title, "new title");
        let reloaded = AtomicStore::load(&path).unwrap();
        assert_eq!(reloaded.section("39").unwrap().skeleton.title, "new title");
    }

    #[test]
    fn set_section_title_rejects_empty() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        add_section(&mut store, &path, "39", "docs/X.md", "T", None).unwrap();
        let err = set_section_title(&mut store, &path, "39", "   ").unwrap_err();
        assert!(matches!(err, AtomicMutateError::Validation(_)));
        // Original title unchanged.
        assert_eq!(store.section("39").unwrap().skeleton.title, "T");
    }

    #[test]
    fn set_section_title_not_found_on_missing_section() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        let err = set_section_title(&mut store, &path, "ghost", "x").unwrap_err();
        assert!(matches!(err, AtomicMutateError::NotFound(_)));
    }

    #[test]
    fn set_section_parent_doc_basic_and_round_trip() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        add_section(&mut store, &path, "39", "docs/OLD.md", "T", None).unwrap();
        set_section_parent_doc(&mut store, &path, "39", "docs/NEW.md").unwrap();
        assert_eq!(
            store.section("39").unwrap().skeleton.parent_doc,
            "docs/NEW.md"
        );
        let reloaded = AtomicStore::load(&path).unwrap();
        assert_eq!(
            reloaded.section("39").unwrap().skeleton.parent_doc,
            "docs/NEW.md"
        );
    }

    #[test]
    fn set_section_parent_doc_rejects_empty() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        add_section(&mut store, &path, "39", "docs/X.md", "T", None).unwrap();
        let err = set_section_parent_doc(&mut store, &path, "39", "").unwrap_err();
        assert!(matches!(err, AtomicMutateError::Validation(_)));
    }

    #[test]
    fn set_section_parent_doc_not_found_on_missing_section() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        let err = set_section_parent_doc(&mut store, &path, "ghost", "docs/X.md").unwrap_err();
        assert!(matches!(err, AtomicMutateError::NotFound(_)));
    }

    #[test]
    fn set_section_parent_section_some_and_none_round_trip() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        add_section(&mut store, &path, "39", "docs/X.md", "Parent", None).unwrap();
        add_section(&mut store, &path, "39.1", "docs/X.md", "Child", None).unwrap();
        // Re-parent child under parent.
        set_section_parent_section(&mut store, &path, "39.1", Some("39")).unwrap();
        assert_eq!(
            store
                .section("39.1")
                .unwrap()
                .skeleton
                .parent_section
                .as_deref(),
            Some("39")
        );
        // Promote back to top-level (None).
        set_section_parent_section(&mut store, &path, "39.1", None).unwrap();
        assert_eq!(store.section("39.1").unwrap().skeleton.parent_section, None);
    }

    #[test]
    fn set_section_parent_section_not_found_on_missing_parent() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        add_section(&mut store, &path, "39.1", "docs/X.md", "Child", None).unwrap();
        let err = set_section_parent_section(&mut store, &path, "39.1", Some("ghost")).unwrap_err();
        assert!(matches!(err, AtomicMutateError::NotFound(_)));
        // Child unchanged.
        assert_eq!(store.section("39.1").unwrap().skeleton.parent_section, None);
    }

    #[test]
    fn set_section_parent_section_rejects_self_loop() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        add_section(&mut store, &path, "39", "docs/X.md", "T", None).unwrap();
        let err = set_section_parent_section(&mut store, &path, "39", Some("39")).unwrap_err();
        match err {
            AtomicMutateError::Validation(msg) => assert!(msg.contains("self-loop")),
            other => panic!("expected Validation, got {:?}", other),
        }
    }

    #[test]
    fn set_section_parent_section_not_found_on_missing_section() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        let err = set_section_parent_section(&mut store, &path, "ghost", None).unwrap_err();
        assert!(matches!(err, AtomicMutateError::NotFound(_)));
    }

    #[test]
    fn set_section_decision_status_persists_and_round_trips() {
        // Round 265 — atomic decision_status field round-trips through
        // sidecar JSON. Default = None (skip_serializing_if), Some(_) appears
        // as lowercase string in JSON, deserializes back to enum.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_section(&mut store, "39");
        set_section_decision_status(
            &mut store,
            &path,
            "39",
            DecisionStatus::Superseded,
            Some("40"),
            None,
        )
        .unwrap();
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(raw.contains("\"decision_status\": \"superseded\""));
        let reloaded = AtomicStore::load(&path).unwrap();
        assert_eq!(
            reloaded.section("39").unwrap().skeleton.decision_status,
            Some(DecisionStatus::Superseded)
        );
    }

    #[test]
    fn set_section_decision_status_overwrite_is_idempotent() {
        // Re-setting the same status does not error, and overwriting with a
        // different status replaces the previous value (no append-only semantics
        // — this is a single-field setter).
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_section(&mut store, "1");
        set_section_decision_status(&mut store, &path, "1", DecisionStatus::Active, None, None)
            .unwrap();
        set_section_decision_status(&mut store, &path, "1", DecisionStatus::Active, None, None)
            .unwrap();
        set_section_decision_status(
            &mut store,
            &path,
            "1",
            DecisionStatus::Superseded,
            Some("2"),
            None,
        )
        .unwrap();
        assert_eq!(
            store.section("1").unwrap().skeleton.decision_status,
            Some(DecisionStatus::Superseded)
        );
    }

    #[test]
    fn set_section_decision_status_superseded_without_superseding_rejects() {
        // T1 rule 4 (atomic axis) author-time guard: Superseded transition
        // without a superseding section_id is a semantically-incoherent state
        // ("replaced, but no replacement recorded") and must reject at the
        // mutate boundary. Symmetric with the markdown-axis guard.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        let err = set_section_decision_status(
            &mut store,
            &path,
            "39",
            DecisionStatus::Superseded,
            None,
            None,
        )
        .unwrap_err();
        match err {
            AtomicMutateError::Validation(msg) => {
                assert!(
                    msg.contains("T1 rule 4"),
                    "expected T1 rule 4 attribution in error message, got: {}",
                    msg
                );
                assert!(
                    msg.contains("atomic axis"),
                    "expected atomic axis attribution in error message, got: {}",
                    msg
                );
            }
            other => panic!("expected Validation, got {:?}", other),
        }
        // Atomic store must remain unchanged (no partial write).
        assert!(
            store.section("39").is_none()
                || store
                    .section("39")
                    .unwrap()
                    .skeleton
                    .decision_status
                    .is_none()
        );
    }

    #[test]
    fn set_section_decision_status_active_no_superseding_required() {
        // Active and Removed targets do not require a superseding ref — only
        // Superseded does. Removed is tombstone-exempt (asserts finality, not
        // replacement); Active is the default starting state.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_section(&mut store, "1");
        seed_section(&mut store, "2");
        set_section_decision_status(&mut store, &path, "1", DecisionStatus::Active, None, None)
            .unwrap();
        set_section_decision_status(&mut store, &path, "2", DecisionStatus::Removed, None, None)
            .unwrap();
        assert_eq!(
            store.section("1").unwrap().skeleton.decision_status,
            Some(DecisionStatus::Active)
        );
        assert_eq!(
            store.section("2").unwrap().skeleton.decision_status,
            Some(DecisionStatus::Removed)
        );
    }

    #[test]
    fn set_section_decision_status_superseded_with_superseding_writes() {
        // Author-time guard accepts any non-None superseding string; existence
        // checking is rule 1's territory (validate-workspace), not rule 4's.
        // Symmetric with the markdown-axis guard which also defers existence
        // checking.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_section(&mut store, "39");
        set_section_decision_status(
            &mut store,
            &path,
            "39",
            DecisionStatus::Superseded,
            Some("40"),
            None,
        )
        .unwrap();
        assert_eq!(
            store.section("39").unwrap().skeleton.decision_status,
            Some(DecisionStatus::Superseded)
        );
        // R342: the superseding target is stored structurally.
        assert_eq!(
            store.section("39").unwrap().superseded_by.as_deref(),
            Some("40")
        );
    }

    #[test]
    fn set_section_decision_status_clears_superseded_by_on_active() {
        // R342 pairing invariant: → Superseded stores the forward-pointer; a
        // later → Active clears it so no stale superseded_by survives. Single
        // write path enforces decision_status / superseded_by coherence.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_section(&mut store, "39");
        set_section_decision_status(
            &mut store,
            &path,
            "39",
            DecisionStatus::Superseded,
            Some("40"),
            None,
        )
        .unwrap();
        assert_eq!(
            store.section("39").unwrap().superseded_by.as_deref(),
            Some("40")
        );
        set_section_decision_status(&mut store, &path, "39", DecisionStatus::Active, None, None)
            .unwrap();
        assert!(store.section("39").unwrap().superseded_by.is_none());
    }

    #[test]
    fn set_section_decision_status_open_sets_and_clears_resolved_by() {
        // R579 — symmetric to superseded_by: → Open stores the optional
        // resolution forward-pointer; a later → Active clears it. The single
        // write path keeps decision_status / resolved_by coherent.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_section(&mut store, "q");
        seed_section(&mut store, "r");
        set_section_decision_status(
            &mut store,
            &path,
            "q",
            DecisionStatus::Open,
            None,
            Some("r"),
        )
        .unwrap();
        assert_eq!(
            store.section("q").unwrap().skeleton.decision_status,
            Some(DecisionStatus::Open)
        );
        assert_eq!(
            store.section("q").unwrap().resolved_by.as_deref(),
            Some("r")
        );
        // Resolving the question (→ Active) clears the forward-pointer.
        set_section_decision_status(&mut store, &path, "q", DecisionStatus::Active, None, None)
            .unwrap();
        assert!(store.section("q").unwrap().resolved_by.is_none());
    }

    #[test]
    fn atomic_section_decision_status_default_is_none() {
        // Default = None (no atomic override). Mutate primitives that don't
        // touch decision_status leave the field at None — consumers fall back
        // to the parser-derived status.
        //
        // Round 287 — seed via direct insert (not add_section, which sets
        // decision_status to Some(Active) by construction). This test
        // specifically exercises the None-default code path that pre-Round 287
        // sections carry.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_section(&mut store, "1");
        set_section_intent(&mut store, &path, "1", "test intent").unwrap();
        assert!(store
            .section("1")
            .unwrap()
            .skeleton
            .decision_status
            .is_none());
        // serde skip_serializing_if confirms field is absent in JSON.
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(!raw.contains("decision_status"));
    }

    #[test]
    fn atomic_section_id_set_empty_when_only_changelog() {
        // changelog_entries-only stores have an empty section_id set (changelog vs section
        // axis separation carry).
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        append_changelog_entry(
            &mut store,
            &path,
            ChangelogEntryDraft {
                entry_id: "Round 243",
                decision_summary: Some("test"),
                changes_bullets: &["change".into()],
                verification_bullets: &["verify".into()],
                impact_refs: &[],
                carry_forward_bullets: &["carry".into()],
            },
            "Round ",
        )
        .unwrap();
        let id_set = store.atomic_section_id_set();
        assert!(id_set.is_empty());
    }

    // ============ Round 295 publishable setters ============

    fn seed_entry(store: &mut AtomicStore, path: &Path, entry_id: &str) {
        append_changelog_entry(
            store,
            path,
            ChangelogEntryDraft {
                entry_id,
                decision_summary: Some("audit summary"),
                changes_bullets: &["audit change".into()],
                verification_bullets: &["audit verify".into()],
                impact_refs: &["43".into()],
                carry_forward_bullets: &["audit carry".into()],
            },
            "Round ",
        )
        .unwrap();
    }

    #[test]
    fn publishable_setters_modify_publishable_only() {
        // Round 295 invariant: setters change publishable_* and leave audit_*
        // intact. Audit half is the permanent record, immutable post-append.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_entry(&mut store, &path, "Round 999");

        set_changelog_publishable_decision_summary(
            &mut store,
            &path,
            "Round 999",
            "redacted summary",
        )
        .unwrap();
        set_changelog_publishable_changes_bullets(
            &mut store,
            &path,
            "Round 999",
            &["redacted change".into()],
        )
        .unwrap();
        set_changelog_publishable_verification_bullets(
            &mut store,
            &path,
            "Round 999",
            &["redacted verify".into()],
        )
        .unwrap();
        set_changelog_publishable_impact_refs(&mut store, &path, "Round 999", &["61".into()])
            .unwrap();
        set_changelog_publishable_carry_forward_bullets(
            &mut store,
            &path,
            "Round 999",
            &["redacted carry".into()],
        )
        .unwrap();

        let entry = store.changelog_entries.get("Round 999").unwrap();
        // audit half intact
        assert_eq!(entry.decision_summary.as_deref(), Some("audit summary"));
        assert_eq!(entry.changes_bullets, vec!["audit change".to_string()]);
        assert_eq!(entry.verification_bullets, vec!["audit verify".to_string()]);
        assert_eq!(entry.impact_refs, vec!["43".to_string()]);
        assert_eq!(entry.carry_forward_bullets, vec!["audit carry".to_string()]);
        // publishable half diverged
        assert_eq!(
            entry.publishable_decision_summary.as_deref(),
            Some("redacted summary")
        );
        assert_eq!(
            entry.publishable_changes_bullets,
            vec!["redacted change".to_string()]
        );
        assert_eq!(
            entry.publishable_verification_bullets,
            vec!["redacted verify".to_string()]
        );
        assert_eq!(entry.publishable_impact_refs, vec!["61".to_string()]);
        assert_eq!(
            entry.publishable_carry_forward_bullets,
            vec!["redacted carry".to_string()]
        );
        assert!(
            !entry.publishable_matches_audit(),
            "publishable / audit divergence is the whole point of these setters"
        );
    }

    #[test]
    fn publishable_setter_rejects_missing_entry() {
        // entry_mut_strict refuses to author a new entry — the audit half can
        // only come from append_changelog_entry.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        let err =
            set_changelog_publishable_decision_summary(&mut store, &path, "Round 404", "anything")
                .unwrap_err();
        match err {
            AtomicMutateError::NotFound(_) => {}
            other => panic!("expected NotFound, got {:?}", other),
        }
    }

    #[test]
    fn publishable_setter_round_trips_through_save_load() {
        // After setting, save then reload via AtomicStore::load: the divergent
        // publishable_* persists (v4 store, no migration overwrite).
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_entry(&mut store, &path, "Round 999");
        set_changelog_publishable_decision_summary(
            &mut store,
            &path,
            "Round 999",
            "redacted on disk",
        )
        .unwrap();
        let reloaded = AtomicStore::load(&path).unwrap();
        assert_eq!(reloaded.schema_version, CURRENT_SCHEMA_VERSION);
        let entry = reloaded.changelog_entries.get("Round 999").unwrap();
        assert_eq!(entry.decision_summary.as_deref(), Some("audit summary"));
        assert_eq!(
            entry.publishable_decision_summary.as_deref(),
            Some("redacted on disk")
        );
    }

    // ============ R305 field-invariant parity ============
    //
    // For each AtomicChangelogEntry field that has multiple write paths
    // (the audit half via `append_changelog_entry` + the publishable mirror
    // via `set_changelog_publishable_*`), the publishable setter must never
    // be *stricter* than the audit append. Anything append accepts, the
    // matching setter must also accept — otherwise an entry written via the
    // audit clone can become un-editable via its own publishable path.
    //
    // Background: R294 entry was authored with a 906-char
    // publishable_decision_summary (via the audit clone — append has no
    // length cap). R295 then introduced the 5 publishable setters and paste-
    // copied the section-side `check_intent_len` (cap 200) / `check_bullet_len`
    // from the facts-as-one-liner section policy. The R294 entry was authored
    // through the cap-0 audit path but could not be edited through the
    // cap-200 setter path — paste-error. The tests below pin the post-R305
    // parity so any future setter that copies a tighter invariant breaks CI.

    #[test]
    fn field_parity_decision_summary_accepts_uncapped_input() {
        // 2 KiB ≫ legacy 200-char cap; well past any plausible "real" entry.
        let long_summary = "x".repeat(2_000);

        // audit path accepts.
        let tmp_a = TempDir::new().unwrap();
        let path_a = tmp_a.path().join(".atomic/workspace.atomic.json");
        let mut store_a = AtomicStore::new();
        append_changelog_entry(
            &mut store_a,
            &path_a,
            ChangelogEntryDraft {
                entry_id: "Round PA",
                decision_summary: Some(&long_summary),
                changes_bullets: &["c".into()],
                verification_bullets: &["v".into()],
                impact_refs: &["1".into()],
                carry_forward_bullets: &["cf".into()],
            },
            "Round ",
        )
        .expect("append must accept arbitrary-length decision_summary");

        // publishable setter path accepts the same input on a pre-existing entry.
        let tmp_b = TempDir::new().unwrap();
        let path_b = tmp_b.path().join(".atomic/workspace.atomic.json");
        let mut store_b = AtomicStore::new();
        seed_entry(&mut store_b, &path_b, "Round PA");
        set_changelog_publishable_decision_summary(
            &mut store_b,
            &path_b,
            "Round PA",
            &long_summary,
        )
        .expect("publishable setter must mirror append's cap-0 invariant");
    }

    #[test]
    fn field_parity_bullet_fields_accept_uncapped_elements() {
        // 10 KiB per element. Each bullet-family field — changes,
        // verification, impact_refs, carry_forward — must accept what append
        // would have accepted at clone time.
        let long_bullet = "x".repeat(10_000);
        let bullets = vec![long_bullet.clone()];

        // audit path: append accepts long bullets across all four bullet-family fields.
        let tmp_a = TempDir::new().unwrap();
        let path_a = tmp_a.path().join(".atomic/workspace.atomic.json");
        let mut store_a = AtomicStore::new();
        append_changelog_entry(
            &mut store_a,
            &path_a,
            ChangelogEntryDraft {
                entry_id: "Round PB",
                decision_summary: Some("audit summary"),
                changes_bullets: &bullets,
                verification_bullets: &bullets,
                impact_refs: &bullets,
                carry_forward_bullets: &bullets,
            },
            "Round ",
        )
        .expect("append must accept long bullets across all bullet-family fields");

        // publishable setter path: each of the 4 setters accepts the same input.
        let tmp_b = TempDir::new().unwrap();
        let path_b = tmp_b.path().join(".atomic/workspace.atomic.json");
        let mut store_b = AtomicStore::new();
        seed_entry(&mut store_b, &path_b, "Round PB");
        set_changelog_publishable_changes_bullets(&mut store_b, &path_b, "Round PB", &bullets)
            .expect("publishable changes setter must mirror append's cap-0 invariant");
        set_changelog_publishable_verification_bullets(&mut store_b, &path_b, "Round PB", &bullets)
            .expect("publishable verification setter must mirror append's cap-0 invariant");
        set_changelog_publishable_impact_refs(&mut store_b, &path_b, "Round PB", &bullets)
            .expect("publishable impact_refs setter must mirror append's cap-0 invariant");
        set_changelog_publishable_carry_forward_bullets(
            &mut store_b,
            &path_b,
            "Round PB",
            &bullets,
        )
        .expect("publishable carry_forward setter must mirror append's cap-0 invariant");
    }

    // ============ Round 296 publishable hash anchoring ============

    #[test]
    fn publishable_hash_deterministic_and_stable() {
        // Same content → same hash (deterministic). Different content →
        // different hash (forge-resistance basis for the R296 ledger gate).
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_entry(&mut store, &path, "Round 999");

        let entry = store.changelog_entries.get("Round 999").unwrap();
        let hash_a = entry.publishable_hash_hex();
        let hash_b = entry.publishable_hash_hex();
        assert_eq!(hash_a, hash_b, "hash must be deterministic");
        assert_eq!(hash_a.len(), 64, "SHA256 hex = 64 chars");

        // Mutate publishable_* → hash changes.
        set_changelog_publishable_decision_summary(
            &mut store,
            &path,
            "Round 999",
            "different summary",
        )
        .unwrap();
        let new_hash = store
            .changelog_entries
            .get("Round 999")
            .unwrap()
            .publishable_hash_hex();
        assert_ne!(hash_a, new_hash, "mutation must change hash");
    }

    #[test]
    fn publishable_hash_differs_from_audit_hash_when_diverged() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_entry(&mut store, &path, "Round 999");
        // Pre-divergence: publishable_matches_audit, but the two hashes use
        // different field names so the digests are not identical even when
        // contents match. That's intentional — they are different bodies and
        // must produce different anchors.
        set_changelog_publishable_decision_summary(&mut store, &path, "Round 999", "redacted")
            .unwrap();
        let entry = store.changelog_entries.get("Round 999").unwrap();
        assert_ne!(
            entry.publishable_hash_hex(),
            entry.audit_hash_hex(),
            "diverged publishable / audit must hash to different anchors"
        );
    }

    // ============ Round 300 emit_publishable_override_ledger_draft ============

    #[test]
    fn r300_emit_draft_returns_none_when_in_sync() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_entry(&mut store, &path, "Round 999");
        // No publishable divergence yet — primitive must return None.
        let result =
            emit_publishable_override_ledger_draft(&store, "Round 999", "r", "a", "redaction")
                .unwrap();
        assert!(result.is_none(), "in-sync entry must yield no draft");
    }

    #[test]
    fn r300_emit_draft_unknown_entry_id_returns_not_found() {
        let tmp = TempDir::new().unwrap();
        let _path = tmp.path().join(".atomic/workspace.atomic.json");
        let store = AtomicStore::new();
        let err =
            emit_publishable_override_ledger_draft(&store, "Round 999", "r", "a", "redaction")
                .unwrap_err();
        match err {
            AtomicMutateError::NotFound(msg) => {
                assert!(msg.contains("Round 999"), "msg={}", msg);
            }
            other => panic!("expected NotFound, got {:?}", other),
        }
    }

    #[test]
    fn r300_emit_draft_lists_only_divergent_fields() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_entry(&mut store, &path, "Round 999");
        set_changelog_publishable_decision_summary(
            &mut store,
            &path,
            "Round 999",
            "redacted summary",
        )
        .unwrap();
        let draft = emit_publishable_override_ledger_draft(
            &store,
            "Round 999",
            "audit reason",
            "Round T",
            "redaction",
        )
        .unwrap()
        .expect("divergent entry must emit a draft");
        assert!(draft.contains("[[publishable_override_ledger]]"));
        assert!(draft.contains("kind = \"redaction\""));
        assert!(draft.contains("target_id = \"Round 999\""));
        assert!(
            draft.contains("fields = [\"publishable_decision_summary\"]"),
            "only the touched field must appear; draft:\n{}",
            draft
        );
        assert!(draft.contains("reason = \"audit reason\""));
        assert!(draft.contains("applied_in = \"Round T\""));
    }

    #[test]
    fn r300_emit_draft_hash_matches_validate_workspace_expectation() {
        // The hash anchor in the emitted draft must match the post-mutation
        // publishable_hash_hex(): validate-workspace recomputes this on every
        // run; mismatch ⇒ R296 gate rejects. R300 must compute against the
        // current entry state, not a pre-mutation snapshot.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_entry(&mut store, &path, "Round 999");
        set_changelog_publishable_decision_summary(&mut store, &path, "Round 999", "redacted")
            .unwrap();
        let entry = store.changelog_entries.get("Round 999").unwrap();
        let expected_after = entry.publishable_hash_hex();
        let expected_before = entry.audit_hash_hex();
        let draft =
            emit_publishable_override_ledger_draft(&store, "Round 999", "r", "a", "redaction")
                .unwrap()
                .unwrap();
        assert!(
            draft.contains(&format!("content_hash_after = \"{}\"", expected_after)),
            "draft must anchor to current publishable hash; draft:\n{}",
            draft
        );
        assert!(
            draft.contains(&format!("content_hash_before = \"{}\"", expected_before)),
            "draft must record audit hash as before; draft:\n{}",
            draft
        );
    }

    /// Seed a section carrying a normative_excerpt with an empty text_sha256
    /// (the authoring / import_sections-inline path) — the precondition
    /// [`import_epub_excerpts`] refreshes. Replaces the deleted hand-authoring
    /// setter as the test seed.
    fn seed_excerpt(
        store: &mut AtomicStore,
        path: &Path,
        id: &str,
        text: &str,
        url: &str,
        rev: &str,
    ) {
        let mut e = imp(id, "docs/GENERATED.md", "T");
        e.normative_excerpt = Some(NormativeExcerpt {
            text: text.to_string(),
            anchor_url: url.to_string(),
            source_revision: rev.to_string(),
            text_sha256: String::new(),
        });
        import_sections(store, path, &[e]).unwrap();
    }

    #[test]
    fn build_normative_excerpt_accepts_valid_empty_hash() {
        let ne = build_normative_excerpt(
            "The <event> element ...",
            "https://www.w3.org/TR/scxml/#event",
            "2015-09-01",
            "",
        )
        .unwrap();
        assert_eq!(ne.text, "The <event> element ...");
        assert_eq!(ne.anchor_url, "https://www.w3.org/TR/scxml/#event");
        assert_eq!(ne.source_revision, "2015-09-01");
        // Empty hash = unrevalidatable (hand-authored / pre-v8).
        assert_eq!(ne.text_sha256, "");
    }

    #[test]
    fn build_normative_excerpt_trims_trailing_newline() {
        let ne = build_normative_excerpt("spec text\n\n", "https://example.com/spec", "rev1", "")
            .unwrap();
        assert_eq!(
            ne.text, "spec text",
            "trailing newlines trimmed for stable storage"
        );
    }

    #[test]
    fn build_normative_excerpt_rejects_blank_text() {
        let err =
            build_normative_excerpt(" \n ", "https://example.com/spec", "rev1", "").unwrap_err();
        assert!(matches!(err, AtomicMutateError::Validation(_)));
    }

    #[test]
    fn build_normative_excerpt_rejects_blank_source_revision() {
        let err =
            build_normative_excerpt("text", "https://example.com/spec", "   ", "").unwrap_err();
        assert!(matches!(err, AtomicMutateError::Validation(_)));
    }

    #[test]
    fn build_normative_excerpt_rejects_non_url_anchor() {
        let err = build_normative_excerpt("text", "not-a-url", "rev1", "").unwrap_err();
        match err {
            AtomicMutateError::Validation(msg) => {
                assert!(msg.contains("absolute http(s):// URL"), "got: {msg}")
            }
            other => panic!("expected Validation, got {other:?}"),
        }
    }

    #[test]
    fn build_normative_excerpt_rejects_missing_host() {
        let err = build_normative_excerpt("text", "https:///path-only", "rev1", "").unwrap_err();
        assert!(matches!(err, AtomicMutateError::Validation(_)));
    }

    #[test]
    fn build_normative_excerpt_accepts_matching_hash() {
        let text = "spec text";
        let hash = sha256_hex(text.as_bytes());
        let ne = build_normative_excerpt(text, "https://example.com/spec", "rev1", &hash).unwrap();
        assert_eq!(ne.text_sha256, hash);
    }

    #[test]
    fn build_normative_excerpt_hashes_stored_text_not_raw() {
        // The hash anchors the STORED (newline-trimmed) string, not the raw input.
        let stored = "spec text";
        let hash = sha256_hex(stored.as_bytes());
        let ne = build_normative_excerpt("spec text\n", "https://example.com/spec", "rev1", &hash)
            .unwrap();
        assert_eq!(ne.text, stored);
        assert_eq!(ne.text_sha256, hash);
    }

    #[test]
    fn build_normative_excerpt_rejects_mismatched_hash() {
        let err =
            build_normative_excerpt("spec text", "https://example.com/spec", "rev1", "deadbeef")
                .unwrap_err();
        match err {
            AtomicMutateError::Validation(msg) => {
                assert!(msg.contains("text_sha256 mismatch"), "got: {msg}")
            }
            other => panic!("expected Validation, got {other:?}"),
        }
    }

    #[test]
    fn text_sha256_matches_three_states() {
        let mk = |text: &str, hash: &str| NormativeExcerpt {
            text: text.to_string(),
            anchor_url: "https://example.com/s".to_string(),
            source_revision: "rev".to_string(),
            text_sha256: hash.to_string(),
        };
        // empty hash → None (unrevalidatable)
        assert_eq!(mk("spec text", "").text_sha256_matches(), None);
        // correct hash → Some(true)
        let good = sha256_hex(b"spec text");
        assert_eq!(mk("spec text", &good).text_sha256_matches(), Some(true));
        // wrong hash → Some(false)
        assert_eq!(
            mk("spec text", "deadbeef").text_sha256_matches(),
            Some(false)
        );
        // recompute is the value a correct hash holds
        assert_eq!(mk("spec text", "").recompute_text_sha256(), good);
    }

    #[test]
    fn excerpt_hash_backfill_report_lists_only_empty_hash() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        // a: authored excerpt → empty text_sha256 (in the report).
        seed_excerpt(&mut store, &path, "a", "text a", "https://x.org/#a", "rev1");
        // b: authored, then a hash populated (simulates an EPUB import) → excluded.
        seed_excerpt(&mut store, &path, "b", "text b", "https://x.org/#b", "rev2");
        store
            .sections
            .get_mut("b")
            .unwrap()
            .normative_excerpt
            .as_mut()
            .unwrap()
            .text_sha256 = "deadbeef".into();
        // c: no excerpt → excluded.
        seed_section(&mut store, "c");
        let report = store.excerpt_hash_backfill_report();
        assert_eq!(report.rows.len(), 1);
        assert_eq!(report.rows[0].section_id, "a");
        assert_eq!(report.rows[0].source_revision, "rev1");
    }

    #[test]
    fn import_epub_excerpts_refreshes_text_and_preserves_identity() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_excerpt(
            &mut store,
            &path,
            "scxml-3.13",
            "old text",
            "https://www.w3.org/TR/scxml/#event",
            "2015-09-01",
        );
        let new_text = "new EPUB text";
        let hash = sha256_hex(new_text.as_bytes());
        let (_r, unmatched) = import_epub_excerpts(
            &mut store,
            &path,
            &[ExcerptImport {
                section_id: "scxml-3.13".into(),
                text: new_text.into(),
                text_sha256: hash.clone(),
            }],
        )
        .unwrap();
        assert!(unmatched.is_empty());
        let ne = store
            .section("scxml-3.13")
            .unwrap()
            .normative_excerpt
            .as_ref()
            .unwrap();
        assert_eq!(ne.text, new_text);
        assert_eq!(ne.text_sha256, hash);
        // Authored identity preserved (store-side, not EPUB-projected).
        assert_eq!(ne.anchor_url, "https://www.w3.org/TR/scxml/#event");
        assert_eq!(ne.source_revision, "2015-09-01");
    }

    #[test]
    fn import_epub_excerpts_overwrites_existing_hash() {
        // Frozen-ledger gate removed in R403: a second import overwrites.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_excerpt(
            &mut store,
            &path,
            "x",
            "v0",
            "https://example.com/s",
            "rev1",
        );
        let h1 = sha256_hex(b"first");
        import_epub_excerpts(
            &mut store,
            &path,
            &[ExcerptImport {
                section_id: "x".into(),
                text: "first".into(),
                text_sha256: h1,
            }],
        )
        .unwrap();
        let h2 = sha256_hex(b"second");
        import_epub_excerpts(
            &mut store,
            &path,
            &[ExcerptImport {
                section_id: "x".into(),
                text: "second".into(),
                text_sha256: h2.clone(),
            }],
        )
        .unwrap();
        let ne = store
            .section("x")
            .unwrap()
            .normative_excerpt
            .as_ref()
            .unwrap();
        assert_eq!(ne.text, "second");
        assert_eq!(ne.text_sha256, h2);
    }

    #[test]
    fn import_epub_excerpts_reports_unmatched() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_excerpt(
            &mut store,
            &path,
            "has",
            "t",
            "https://example.com/s",
            "rev1",
        );
        seed_section(&mut store, "noexcerpt"); // present but no excerpt → unmatched
        let ht = sha256_hex(b"refresh");
        let (_r, unmatched) = import_epub_excerpts(
            &mut store,
            &path,
            &[
                ExcerptImport {
                    section_id: "has".into(),
                    text: "refresh".into(),
                    text_sha256: ht,
                },
                ExcerptImport {
                    section_id: "noexcerpt".into(),
                    text: "x".into(),
                    text_sha256: String::new(),
                },
                ExcerptImport {
                    section_id: "absent".into(),
                    text: "y".into(),
                    text_sha256: String::new(),
                },
            ],
        )
        .unwrap();
        assert_eq!(
            unmatched,
            vec!["noexcerpt".to_string(), "absent".to_string()]
        );
    }

    #[test]
    fn import_epub_excerpts_rejects_hash_mismatch() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_excerpt(
            &mut store,
            &path,
            "x",
            "orig",
            "https://example.com/s",
            "rev1",
        );
        let err = import_epub_excerpts(
            &mut store,
            &path,
            &[ExcerptImport {
                section_id: "x".into(),
                text: "new".into(),
                text_sha256: "deadbeef".into(),
            }],
        )
        .unwrap_err();
        assert!(matches!(err, AtomicMutateError::Validation(_)));
    }

    #[test]
    fn import_epub_excerpts_errors_when_nothing_matches() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_section(&mut store, "x"); // present but no excerpt
        let err = import_epub_excerpts(
            &mut store,
            &path,
            &[ExcerptImport {
                section_id: "x".into(),
                text: "t".into(),
                text_sha256: String::new(),
            }],
        )
        .unwrap_err();
        assert!(matches!(err, AtomicMutateError::NotFound(_)));
    }

    // ---- A2: import_sections (bulk create) ----

    fn imp(section_id: &str, parent_doc: &str, title: &str) -> SectionImport {
        SectionImport {
            section_id: section_id.to_string(),
            parent_doc: parent_doc.to_string(),
            title: title.to_string(),
            parent_section: None,
            normative_excerpt: None,
            coverage_expectation: mnemosyne_core::CoverageExpectation::Normative,
        }
    }

    #[test]
    fn import_sections_creates_absent() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        let manifest = vec![
            imp("scxml-3.13", "docs/GENERATED.md", "Event Descriptors"),
            imp("scxml-5.10", "docs/GENERATED.md", "Datamodel"),
        ];
        let receipt = import_sections(&mut store, &path, &manifest).unwrap();
        assert_eq!(store.sections.len(), 2);
        assert!(store.sections.contains_key("scxml-3.13"));
        assert!(
            receipt.target_id.contains("2 created"),
            "{}",
            receipt.target_id
        );
        assert!(receipt.written_bytes > 0);
    }

    #[test]
    fn import_sections_idempotent_noop_second_run() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        let manifest = vec![imp("scxml-3.13", "docs/GENERATED.md", "Event Descriptors")];
        import_sections(&mut store, &path, &manifest).unwrap();
        let receipt2 = import_sections(&mut store, &path, &manifest).unwrap();
        assert_eq!(store.sections.len(), 1);
        assert!(
            receipt2.target_id.contains("0 created"),
            "{}",
            receipt2.target_id
        );
        assert_eq!(receipt2.written_bytes, 0, "no-op manifest must not save");
    }

    #[test]
    fn import_sections_rejects_divergent() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        import_sections(
            &mut store,
            &path,
            &[imp("scxml-3.13", "docs/GENERATED.md", "Original")],
        )
        .unwrap();
        let err = import_sections(
            &mut store,
            &path,
            &[imp("scxml-3.13", "docs/GENERATED.md", "Changed Title")],
        )
        .unwrap_err();
        assert!(matches!(err, AtomicMutateError::Validation(_)));
        assert!(format!("{err}").contains("DIVERGENT"), "{err}");
    }

    #[test]
    fn import_sections_inline_excerpt_anchored_at_create() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        let mut e = imp("scxml-3.13", "docs/GENERATED.md", "Event Descriptors");
        e.normative_excerpt = Some(NormativeExcerpt {
            text: "verbatim spec".to_string(),
            anchor_url: "https://www.w3.org/TR/scxml/#event".to_string(),
            source_revision: "2024-rec".to_string(),
            text_sha256: String::new(),
        });
        import_sections(&mut store, &path, &[e]).unwrap();
        let ne = store
            .section("scxml-3.13")
            .unwrap()
            .normative_excerpt
            .as_ref()
            .unwrap();
        assert_eq!(ne.text, "verbatim spec");
        assert_eq!(ne.source_revision, "2024-rec");
    }

    #[test]
    fn import_epub_anchors_sets_locator_reports_unmatched_and_overwrites() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        import_sections(
            &mut store,
            &path,
            &[imp(
                "scxml-3.13",
                "docs/GENERATED.md",
                "Selecting Transitions",
            )],
        )
        .unwrap();
        let loc = EpubLocator {
            spine_href: "OEBPS/spec.xhtml".to_string(),
            fragment: "scxml-3.13".to_string(),
            cfi: Some("epubcfi(/6/4!/4)".to_string()),
        };
        // one matching id + one absent id
        let (_r, unmatched) = import_epub_anchors(
            &mut store,
            &path,
            &[
                ("scxml-3.13".to_string(), loc.clone()),
                ("scxml-absent".to_string(), loc.clone()),
            ],
        )
        .unwrap();
        assert_eq!(unmatched, vec!["scxml-absent".to_string()]);
        assert_eq!(
            store.section("scxml-3.13").unwrap().epub_locator.as_ref(),
            Some(&loc)
        );
        // overwrite is allowed (derived pointer, not frozen)
        let loc2 = EpubLocator {
            spine_href: "OEBPS/ch3.xhtml".to_string(),
            fragment: "scxml-3.13".to_string(),
            cfi: None,
        };
        import_epub_anchors(
            &mut store,
            &path,
            &[("scxml-3.13".to_string(), loc2.clone())],
        )
        .unwrap();
        assert_eq!(
            store.section("scxml-3.13").unwrap().epub_locator,
            Some(loc2)
        );
    }

    #[test]
    fn import_epub_anchors_errors_when_nothing_matches() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        import_sections(
            &mut store,
            &path,
            &[imp("scxml-1", "docs/GENERATED.md", "X")],
        )
        .unwrap();
        let loc = EpubLocator {
            spine_href: "OEBPS/spec.xhtml".to_string(),
            fragment: "scxml-9".to_string(),
            cfi: None,
        };
        assert!(matches!(
            import_epub_anchors(&mut store, &path, &[("scxml-9".to_string(), loc)]),
            Err(AtomicMutateError::NotFound(_))
        ));
    }

    #[test]
    fn import_sections_parent_within_manifest_ordered() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        let mut child = imp("scxml-D-2-func", "docs/GENERATED.md", "func helper");
        child.parent_section = Some("scxml-D".to_string());
        // Parent first, child second → the child's parent_section resolves.
        let manifest = vec![imp("scxml-D", "docs/GENERATED.md", "Appendix D"), child];
        import_sections(&mut store, &path, &manifest).unwrap();
        assert_eq!(
            store
                .section("scxml-D-2-func")
                .unwrap()
                .skeleton
                .parent_section
                .as_deref(),
            Some("scxml-D")
        );
    }

    #[test]
    fn import_sections_rejects_invalid_excerpt() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        let mut e = imp("scxml-3.13", "docs/GENERATED.md", "X");
        e.normative_excerpt = Some(NormativeExcerpt {
            text: "   ".to_string(),
            anchor_url: "https://example.com/s".to_string(),
            source_revision: "rev".to_string(),
            text_sha256: String::new(),
        });
        let err = import_sections(&mut store, &path, &[e]).unwrap_err();
        assert!(
            format!("{err}").contains("normative_excerpt text blank"),
            "{err}"
        );
    }

    #[test]
    fn import_sections_intra_manifest_divergent_rejected_no_save() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        let manifest = vec![
            imp("scxml-3.13", "docs/GENERATED.md", "First"),
            imp("scxml-3.13", "docs/GENERATED.md", "Second"),
        ];
        let err = import_sections(&mut store, &path, &manifest).unwrap_err();
        assert!(format!("{err}").contains("DIVERGENT"), "{err}");
        assert!(
            !path.exists(),
            "a rejected manifest must not persist anything"
        );
    }

    #[test]
    fn import_sections_strips_section_and_parent_sigil() {
        // SCE-found footgun: a citation-form manifest (section_ids with a
        // leading sigil) must store BARE keys (render must not double the
        // sigil), symmetric with add_section.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        let mut child = imp("§scxml-D-2", "docs/GENERATED.md", "child");
        child.parent_section = Some("§scxml-D".to_string());
        let manifest = vec![imp("§scxml-D", "docs/GENERATED.md", "Appendix D"), child];
        import_sections(&mut store, &path, &manifest).unwrap();
        assert!(store.sections.contains_key("scxml-D"));
        assert!(store.sections.contains_key("scxml-D-2"));
        assert!(
            !store.sections.contains_key("§scxml-D"),
            "sigil leaked into key"
        );
        // Parent ref resolved against the bare key.
        assert_eq!(
            store
                .section("scxml-D-2")
                .unwrap()
                .skeleton
                .parent_section
                .as_deref(),
            Some("scxml-D")
        );
    }

    #[test]
    fn add_section_and_import_sections_normalize_sigil_identically() {
        // Both section-create ingestion paths route through build_candidate_section,
        // so a sigil-prefixed id stores the same bare key either way.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut s1 = AtomicStore::new();
        add_section(&mut s1, &path, "§scxml-1", "docs/GENERATED.md", "A", None).unwrap();
        let mut s2 = AtomicStore::new();
        import_sections(&mut s2, &path, &[imp("§scxml-1", "docs/GENERATED.md", "A")]).unwrap();
        assert_eq!(
            s1.sections.keys().collect::<Vec<_>>(),
            s2.sections.keys().collect::<Vec<_>>()
        );
        assert!(s1.sections.contains_key("scxml-1"));
    }

    #[test]
    fn normative_excerpt_write_path_parity() {
        // CLAUDE.md half-enforced-invariant rule: import_sections inline-at-create
        // and import_epub_excerpts are the TWO write paths to normative_excerpt;
        // both route through build_normative_excerpt and must agree on
        // accept/reject. anchor_url + source_revision are authoring-only inputs
        // (the EPUB path PRESERVES them from an already-validated excerpt), so the
        // overlapping controllable inputs are (text, text_sha256) — vary those,
        // hold url/rev valid. This locks a future bypass on either path.
        const URL: &str = "https://example.com/s";
        const REV: &str = "rev";
        let good_hash = sha256_hex(b"spec text");
        // (text, text_sha256)
        let cases: &[(&str, &str)] = &[
            ("spec text", ""),                 // valid, unrevalidatable
            ("   ", ""),                       // blank text → reject
            ("spec text", good_hash.as_str()), // valid + matching hash
            ("spec text", "deadbeef"),         // hash mismatch → reject
        ];
        for (text, hash) in cases {
            let tmp = TempDir::new().unwrap();
            let path = tmp.path().join(".atomic/workspace.atomic.json");

            // import_sections inline-at-create path.
            let mut s1 = AtomicStore::new();
            let mut e = imp("x", "docs/GENERATED.md", "T");
            e.normative_excerpt = Some(NormativeExcerpt {
                text: text.to_string(),
                anchor_url: URL.to_string(),
                source_revision: REV.to_string(),
                text_sha256: hash.to_string(),
            });
            let import_ok = import_sections(&mut s1, &path, &[e]).is_ok();

            // import_epub_excerpts path: pre-seed a valid excerpt carrying the
            // same url/rev, then refresh with (text, hash).
            let mut s2 = AtomicStore::new();
            seed_excerpt(&mut s2, &path, "x", "seed", URL, REV);
            let epub_ok = import_epub_excerpts(
                &mut s2,
                &path,
                &[ExcerptImport {
                    section_id: "x".into(),
                    text: text.to_string(),
                    text_sha256: hash.to_string(),
                }],
            )
            .is_ok();

            assert_eq!(
                import_ok, epub_ok,
                "write-path parity broken for (text={text:?}, hash={hash:?}): import={import_ok} epub={epub_ok}"
            );
        }
    }
    // ========================================================================
    // Narrative fact primitives (Phase 1A, Round 430).
    // ========================================================================

    /// Fixture: a valid FactImport against `seed_section`-created chapters.
    fn sample_fact(fact_id: &str, frame: &str) -> FactImport {
        FactImport {
            entities: vec![],
            fact_id: fact_id.to_string(),
            frame: frame.to_string(),
            branch: None,
            claim: "the count is an eccentric nobleman".to_string(),
            canon_from: "ch-1".to_string(),
            canon_to: None,
            evidence: vec!["ch-1".to_string()],
            conflicts_with: vec![],
            supersedes_in_frame: None,
            payoff_expectation: None,
            pays_off: vec![],
            typed: None,
            quote: None,
        }
    }

    fn seed_chapters(store: &mut AtomicStore) {
        seed_section(store, "ch-1");
        seed_section(store, "ch-2");
        seed_section(store, "ch-3");
    }

    /// Positional wrapper over [`set_disclosure`] for the test below (Round 510
    /// — the primitive now takes a `DisclosureDecision`; this keeps the test
    /// call sites terse).
    #[allow(clippy::too_many_arguments)]
    fn set_disc(
        store: &mut AtomicStore,
        path: &Path,
        telling: &str,
        fact: &str,
        mode: &str,
        first_at: &[(String, String)],
        surface: Option<(&str, Option<&str>)>,
    ) -> Result<AtomicMutateReceipt, AtomicMutateError> {
        set_disclosure(
            store,
            path,
            DisclosureDecision {
                telling_id: telling,
                fact_id: fact,
                mode,
                first_at,
                surface,
            },
        )
    }

    /// Round 506 — disclosure plan registry + set_disclosure: the
    /// gate-enabling typed invariant (withhold/first_at need a typed fact),
    /// the fail-loud refs, per-world-line first_at, idempotent-policy re-add,
    /// divergent-policy reject, and a v22 round-trip.
    #[test]
    fn disclosure_plan_and_set_disclosure_invariants() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("s.json");
        let mut store = AtomicStore::new();
        seed_chapters(&mut store);
        store.frames.insert("gt".to_string(), Frame::default());
        store.entities.insert("pike".to_string(), Entity::default());
        add_branch(
            &mut store,
            &path,
            "route",
            "",
            Some((mnemosyne_core::MAIN_BRANCH, "ch-2")),
            &[],
        )
        .unwrap();
        add_predicate(&mut store, &path, "did", "scalar", "").unwrap();
        add_fact(&mut store, &path, &sample_fact("f-untyped", "gt")).unwrap();
        let typed_fact = FactImport {
            entities: vec!["pike".to_string()],
            typed: Some(TypedClaim {
                subject: "pike".to_string(),
                predicate: "did".to_string(),
                object: TypedObject::Value {
                    value: "climbed".to_string(),
                },
            }),
            ..sample_fact("f-typed", "gt")
        };
        add_fact(&mut store, &path, &typed_fact).unwrap();

        add_disclosure_plan(
            &mut store,
            &path,
            "dark-souls",
            "withhold",
            "fragment telling",
        )
        .unwrap();
        let err = add_disclosure_plan(&mut store, &path, "bad", "loud", "").unwrap_err();
        assert!(err.to_string().contains("unknown default_mode"), "{err}");

        // Gate-enabling invariant: withhold / first_at on an UNTYPED fact rejects.
        let err = set_disc(
            &mut store,
            &path,
            "dark-souls",
            "f-untyped",
            "withhold",
            &[],
            None,
        )
        .unwrap_err();
        assert!(err.to_string().contains("no typed claim"), "{err}");
        let err = set_disc(
            &mut store,
            &path,
            "dark-souls",
            "f-untyped",
            "state",
            &[(mnemosyne_core::MAIN_BRANCH.to_string(), "ch-2".to_string())],
            None,
        )
        .unwrap_err();
        assert!(err.to_string().contains("no typed claim"), "{err}");
        // A plain state with no timing on an untyped fact is craft-only (un-gated) → ok.
        set_disc(
            &mut store,
            &path,
            "dark-souls",
            "f-untyped",
            "state",
            &[],
            None,
        )
        .unwrap();

        // Typed fact: withhold ok; per-world first_at + surface ok.
        set_disc(
            &mut store,
            &path,
            "dark-souls",
            "f-typed",
            "withhold",
            &[],
            None,
        )
        .unwrap();
        set_disc(
            &mut store,
            &path,
            "dark-souls",
            "f-typed",
            "state",
            &[("route".to_string(), "ch-3".to_string())],
            Some(("ch-2", Some("pike"))),
        )
        .unwrap();
        let ov = &store.disclosure_plans["dark-souls"].overrides["f-typed"];
        assert_eq!(ov.mode, DisclosureMode::State);
        assert_eq!(ov.first_at.get("route").map(String::as_str), Some("ch-3"));
        let surface = ov.surface.as_ref().unwrap();
        assert_eq!(surface.scene, "ch-2");
        assert_eq!(surface.object.as_deref(), Some("pike"));

        // Fail-loud refs.
        let err =
            set_disc(&mut store, &path, "missing", "f-typed", "state", &[], None).unwrap_err();
        assert!(
            err.to_string()
                .contains("not present in the disclosure_plans"),
            "{err}"
        );
        let err = set_disc(
            &mut store,
            &path,
            "dark-souls",
            "f-absent",
            "state",
            &[],
            None,
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("not present in narrative_facts"),
            "{err}"
        );
        let err = set_disc(
            &mut store,
            &path,
            "dark-souls",
            "f-typed",
            "state",
            &[("nope".to_string(), "ch-2".to_string())],
            None,
        )
        .unwrap_err();
        assert!(err.to_string().contains("branch `nope`"), "{err}");
        let err = set_disc(
            &mut store,
            &path,
            "dark-souls",
            "f-typed",
            "state",
            &[(
                mnemosyne_core::MAIN_BRANCH.to_string(),
                "ch-404".to_string(),
            )],
            None,
        )
        .unwrap_err();
        assert!(err.to_string().contains("ch-404"), "{err}");
        let err = set_disc(
            &mut store,
            &path,
            "dark-souls",
            "f-typed",
            "state",
            &[],
            Some(("ch-2", Some("ghost"))),
        )
        .unwrap_err();
        assert!(err.to_string().contains("surface object `ghost`"), "{err}");

        // Idempotent re-add (policy unchanged, overrides untouched) = no-op.
        let again = add_disclosure_plan(
            &mut store,
            &path,
            "dark-souls",
            "withhold",
            "fragment telling",
        )
        .unwrap();
        assert_eq!(again.written_bytes, 0);
        // Divergent policy rejects.
        let err = add_disclosure_plan(&mut store, &path, "dark-souls", "state", "fragment telling")
            .unwrap_err();
        assert!(err.to_string().contains("DIVERGENT policy"), "{err}");

        // v22 round-trip.
        let reloaded = AtomicStore::load(&path).unwrap();
        assert_eq!(reloaded.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(reloaded.disclosure_plans["dark-souls"].overrides.len(), 2);
    }

    // ---- typing-proposals import (Round 459, design sec 7.15 Round B) ----

    /// Substrate for the import tests: one frame, one entity, one scalar
    /// predicate, two untyped facts about the entity.
    fn typing_substrate(path: &Path) -> AtomicStore {
        let mut store = AtomicStore::new();
        seed_chapters(&mut store);
        import_facts(
            &mut store,
            path,
            &FactsManifest {
                disclosure_plans: vec![],
                frames: vec![FrameImport {
                    frame_id: "gt".to_string(),
                    description: String::new(),
                }],
                branches: vec![],
                entities: vec![EntityImport {
                    entity_id: "kara".to_string(),
                    kind: String::new(),
                    description: String::new(),
                }],
                predicates: vec![PredicateImport {
                    predicate_id: "alive".to_string(),
                    object_kind: "scalar".to_string(),
                    description: String::new(),
                }],
                facts: vec![
                    FactImport {
                        entities: vec!["kara".to_string()],
                        ..sample_fact("f-1", "gt")
                    },
                    FactImport {
                        entities: vec!["kara".to_string()],
                        claim: "kara is alive".to_string(),
                        ..sample_fact("f-2", "gt")
                    },
                ],
            },
        )
        .unwrap();
        store
    }

    fn proposal(fact: &str, claim: &str, rationale: &str) -> TypingProposal {
        TypingProposal {
            fact: fact.to_string(),
            typed: TypedClaim {
                subject: "kara".to_string(),
                predicate: "alive".to_string(),
                object: TypedObject::Value {
                    value: "alive".to_string(),
                },
            },
            claim_sha256: sha256_hex(claim.as_bytes()),
            rationale: rationale.to_string(),
        }
    }

    fn proposals_file(proposals: Vec<TypingProposal>) -> TypingProposalsFile {
        TypingProposalsFile {
            schema: "typing-proposals/v1".to_string(),
            comment: String::new(),
            proposals,
        }
    }

    /// All-or-nothing: one stale proposal blocks the whole file (store
    /// untouched, both verdicts surfaced); the corrected file applies
    /// atomically; dry-run validates identically and never writes.
    #[test]
    fn typing_proposals_import_is_all_or_nothing_with_staleness_pin() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("s.json");
        let mut store = typing_substrate(&path);
        // f-2's proposal stamps the WRONG claim text — the R439 staleness pin.
        let file = proposals_file(vec![
            proposal("f-1", "the count is an eccentric nobleman", "state claim"),
            proposal("f-2", "an outdated claim text", "state claim"),
        ]);
        let report = import_typing_proposals(&mut store, &path, &file, "sha", false).unwrap();
        assert_eq!((report.accepted, report.rejected), (1, 1));
        assert!(!report.applied);
        assert!(report.verdicts[1].verdict.contains("stale proposal"));
        assert!(
            store.narrative_facts["f-1"].typed.is_none(),
            "all-or-nothing: nothing applies while any proposal rejects"
        );
        // Corrected file: dry-run first (validates, writes nothing) ...
        let file = proposals_file(vec![
            proposal("f-1", "the count is an eccentric nobleman", "state claim"),
            proposal("f-2", "kara is alive", "state claim"),
        ]);
        let dry = import_typing_proposals(&mut store, &path, &file, "sha", true).unwrap();
        assert_eq!((dry.accepted, dry.rejected), (2, 0));
        assert!(dry.dry_run && !dry.applied && dry.written_bytes == 0);
        assert!(store.narrative_facts["f-1"].typed.is_none());
        // ... then the real run applies both in one transaction.
        let real = import_typing_proposals(&mut store, &path, &file, "sha", false).unwrap();
        assert!(real.applied && real.written_bytes > 0);
        let reloaded = AtomicStore::load(&path).unwrap();
        assert!(reloaded.narrative_facts["f-1"].typed.is_some());
        assert!(reloaded.narrative_facts["f-2"].typed.is_some());
    }

    /// Every reject class carries its own named verdict: duplicate
    /// in-file, missing fact, already-typed target (fill-blanks only),
    /// empty rationale, and a builder reject (the one R305/R446 site —
    /// no new invariant logic in the import path).
    #[test]
    fn typing_proposals_reject_classes_are_named() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("s.json");
        let mut store = typing_substrate(&path);
        // Pre-type f-1 so the fill-blanks reject fires.
        let ok = proposals_file(vec![proposal(
            "f-1",
            "the count is an eccentric nobleman",
            "r",
        )]);
        assert!(
            import_typing_proposals(&mut store, &path, &ok, "sha", false)
                .unwrap()
                .applied
        );
        let mut unregistered = proposal("f-2", "kara is alive", "r");
        unregistered.typed.predicate = "deviancy".to_string();
        let mut no_rationale = proposal("f-2", "kara is alive", "");
        no_rationale.fact = "f-2".to_string();
        let file = proposals_file(vec![
            proposal("f-1", "the count is an eccentric nobleman", "r"), // already typed
            proposal("f-ghost", "x", "r"),                              // missing fact
            no_rationale,                                               // empty rationale
            unregistered, // duplicate f-2 AND unregistered predicate — dup wins (first seen)
        ]);
        let report = import_typing_proposals(&mut store, &path, &file, "sha", false).unwrap();
        assert!(!report.applied);
        assert_eq!(report.rejected, 4);
        assert!(report.verdicts[0]
            .verdict
            .contains("already carries a typed leg"));
        assert!(report.verdicts[1]
            .verdict
            .contains("not present in atomic store"));
        assert!(report.verdicts[2].verdict.contains("rationale mandatory"));
        assert!(report.verdicts[3].verdict.contains("duplicate proposal"));
    }

    /// Loader boundary: schema tag mismatch, unknown fields
    /// (deny_unknown_fields — no lenient-parse legacy on a new artifact),
    /// and an empty proposals list all fail loud.
    #[test]
    fn typing_proposals_loader_fails_loud() {
        let tmp = TempDir::new().unwrap();
        let write = |name: &str, body: &str| {
            let p = tmp.path().join(name);
            fs::write(&p, body).unwrap();
            p
        };
        let bad_schema = write(
            "a.json",
            r#"{"schema":"typing-proposals/v2","proposals":[]}"#,
        );
        assert!(load_typing_proposals(&bad_schema)
            .unwrap_err()
            .contains("not `typing-proposals/v1`"));
        let unknown_key = write(
            "b.json",
            r#"{"schema":"typing-proposals/v1","proposals":[],"confidence":0.9}"#,
        );
        assert!(load_typing_proposals(&unknown_key)
            .unwrap_err()
            .contains("does not parse"));
        let empty = write(
            "c.json",
            r#"{"schema":"typing-proposals/v1","proposals":[]}"#,
        );
        assert!(load_typing_proposals(&empty)
            .unwrap_err()
            .contains("empty proposals list"));
    }

    // ---- edge-proposals import (Round 463, design sec 7.16 Round B) ----

    /// Substrate for the edge-import tests: two frames, two untyped gt
    /// facts in chain position, one cross-frame fact.
    fn edge_substrate(path: &Path) -> AtomicStore {
        let mut store = AtomicStore::new();
        seed_chapters(&mut store);
        import_facts(
            &mut store,
            path,
            &FactsManifest {
                disclosure_plans: vec![],
                frames: vec![
                    FrameImport {
                        frame_id: "gt".to_string(),
                        description: String::new(),
                    },
                    FrameImport {
                        frame_id: "kara".to_string(),
                        description: String::new(),
                    },
                ],
                branches: vec![],
                entities: vec![],
                predicates: vec![],
                facts: vec![
                    sample_fact("f-1", "gt"),
                    FactImport {
                        claim: "kara is alive".to_string(),
                        canon_from: "ch-2".to_string(),
                        evidence: vec!["ch-2".to_string()],
                        ..sample_fact("f-2", "gt")
                    },
                    sample_fact("f-3", "kara"),
                ],
            },
        )
        .unwrap();
        store
    }

    fn succession_proposal(
        successor: &str,
        s_claim: &str,
        predecessor: &str,
        p_claim: &str,
    ) -> SuccessionProposal {
        SuccessionProposal {
            successor: successor.to_string(),
            predecessor: predecessor.to_string(),
            successor_claim_sha256: sha256_hex(s_claim.as_bytes()),
            predecessor_claim_sha256: sha256_hex(p_claim.as_bytes()),
            rationale: "r".to_string(),
        }
    }

    fn conflict_proposal(
        fact: &str,
        f_claim: &str,
        target: &str,
        t_claim: &str,
    ) -> ConflictProposal {
        ConflictProposal {
            fact: fact.to_string(),
            target: target.to_string(),
            fact_claim_sha256: sha256_hex(f_claim.as_bytes()),
            target_claim_sha256: sha256_hex(t_claim.as_bytes()),
            rationale: "r".to_string(),
        }
    }

    fn edge_file(
        succession: Vec<SuccessionProposal>,
        conflicts: Vec<ConflictProposal>,
    ) -> EdgeProposalsFile {
        EdgeProposalsFile {
            schema: "edge-proposals/v1".to_string(),
            comment: String::new(),
            succession,
            conflicts,
        }
    }

    /// Round 461 regression, hole 1: amend-fact retargeting can no longer
    /// close an A⇄B succession cycle (verified live exit-0 before the
    /// shared cycle guard).
    #[test]
    fn amend_fact_succession_cycle_rejects() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("s.json");
        let mut store = edge_substrate(&path);
        // f-2 supersedes f-1 (legal), then amend f-1 to supersede f-2.
        amend_fact(
            &mut store,
            &path,
            &FactImport {
                claim: "kara is alive".to_string(),
                canon_from: "ch-2".to_string(),
                evidence: vec!["ch-2".to_string()],
                supersedes_in_frame: Some("f-1".to_string()),
                ..sample_fact("f-2", "gt")
            },
            "chain it",
        )
        .unwrap();
        let err = amend_fact(
            &mut store,
            &path,
            &FactImport {
                supersedes_in_frame: Some("f-2".to_string()),
                ..sample_fact("f-1", "gt")
            },
            "cycle attempt",
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("cycle"),
            "the shared guard rejects: {err}"
        );
        assert!(
            store.narrative_facts["f-1"].supersedes_in_frame.is_none(),
            "store untouched"
        );
    }

    /// Round 461 regression, hole 2: import-facts forward refs can no
    /// longer close a mutual-supersession cycle in one manifest.
    #[test]
    fn import_facts_forward_ref_succession_cycle_rejects() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("s.json");
        let mut store = AtomicStore::new();
        seed_chapters(&mut store);
        let err = import_facts(
            &mut store,
            &path,
            &FactsManifest {
                disclosure_plans: vec![],
                frames: vec![FrameImport {
                    frame_id: "gt".to_string(),
                    description: String::new(),
                }],
                branches: vec![],
                entities: vec![],
                predicates: vec![],
                facts: vec![
                    FactImport {
                        supersedes_in_frame: Some("f-d".to_string()),
                        ..sample_fact("f-c", "gt")
                    },
                    FactImport {
                        supersedes_in_frame: Some("f-c".to_string()),
                        ..sample_fact("f-d", "gt")
                    },
                ],
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("cycle"), "{err}");
        assert!(store.narrative_facts.is_empty(), "nothing staged survives");
    }

    /// Field-invariant parity (the R305 rule applied to succession): the
    /// fact write path and the proposals import accept and reject the
    /// SAME edge cases — cross-frame target, missing target, cycle.
    #[test]
    fn succession_invariant_parity_across_write_paths() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("s.json");
        // Cross-frame: both paths reject with the in-frame rule.
        let mut store = edge_substrate(&path);
        let via_fact = add_fact(
            &mut store,
            &path,
            &FactImport {
                supersedes_in_frame: Some("f-3".to_string()),
                ..sample_fact("f-new", "gt")
            },
        )
        .unwrap_err();
        let file = edge_file(
            vec![succession_proposal(
                "f-1",
                "the count is an eccentric nobleman",
                "f-3",
                "the count is an eccentric nobleman",
            )],
            vec![],
        );
        let report = import_edge_proposals(&mut store, &path, &file, "sha", false).unwrap();
        assert!(via_fact.to_string().contains("in-frame succession only"));
        assert!(report.verdicts[0]
            .verdict
            .contains("in-frame succession only"));
        // Missing target: both paths reject on existence.
        let via_fact = add_fact(
            &mut store,
            &path,
            &FactImport {
                supersedes_in_frame: Some("f-ghost".to_string()),
                ..sample_fact("f-new", "gt")
            },
        )
        .unwrap_err();
        let file = edge_file(
            vec![succession_proposal(
                "f-1",
                "the count is an eccentric nobleman",
                "f-ghost",
                "x",
            )],
            vec![],
        );
        let report = import_edge_proposals(&mut store, &path, &file, "sha", false).unwrap();
        assert!(via_fact.to_string().contains("not present"));
        assert!(report.verdicts[0].verdict.contains("not present"));
        // Legal in-frame edge: both paths accept (the positive half of
        // parity — same inputs, same verdict).
        let mut store_b = edge_substrate(&tmp.path().join("b.json"));
        add_fact(
            &mut store_b,
            &tmp.path().join("b.json"),
            &FactImport {
                supersedes_in_frame: Some("f-1".to_string()),
                claim: "via fact path".to_string(),
                ..sample_fact("f-new", "gt")
            },
        )
        .unwrap();
        let file = edge_file(
            vec![succession_proposal(
                "f-2",
                "kara is alive",
                "f-1",
                "the count is an eccentric nobleman",
            )],
            vec![],
        );
        let report = import_edge_proposals(&mut store, &path, &file, "sha", false).unwrap();
        assert!(report.applied, "{:?}", report.verdicts);
    }

    /// All-or-nothing with the TWO-SIDED staleness pin: one stale endpoint
    /// blocks the whole file; the corrected file dry-runs (no write) then
    /// applies atomically — succession pointer set, conflict edge stamped.
    #[test]
    fn edge_proposals_import_all_or_nothing_two_sided_pins() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("s.json");
        let mut store = edge_substrate(&path);
        let stale = edge_file(
            vec![succession_proposal(
                "f-2",
                "kara is alive",
                "f-1",
                "the count is an eccentric nobleman",
            )],
            vec![conflict_proposal(
                "f-3",
                "an outdated claim text",
                "f-1",
                "the count is an eccentric nobleman",
            )],
        );
        let report = import_edge_proposals(&mut store, &path, &stale, "sha", false).unwrap();
        assert_eq!((report.accepted, report.rejected), (1, 1));
        assert!(!report.applied);
        assert!(report.verdicts[1].verdict.contains("stale proposal"));
        assert!(
            store.narrative_facts["f-2"].supersedes_in_frame.is_none(),
            "all-or-nothing: nothing applies while any proposal rejects"
        );
        let good = edge_file(
            vec![succession_proposal(
                "f-2",
                "kara is alive",
                "f-1",
                "the count is an eccentric nobleman",
            )],
            vec![conflict_proposal(
                "f-3",
                "the count is an eccentric nobleman",
                "f-1",
                "the count is an eccentric nobleman",
            )],
        );
        let dry = import_edge_proposals(&mut store, &path, &good, "sha", true).unwrap();
        assert!(dry.dry_run && !dry.applied && dry.rejected == 0);
        assert_eq!(dry.written_bytes, 0);
        assert!(store.narrative_facts["f-2"].supersedes_in_frame.is_none());
        let real = import_edge_proposals(&mut store, &path, &good, "sha", false).unwrap();
        assert!(real.applied);
        assert!(real.written_bytes > 0);
        assert_eq!(
            store.narrative_facts["f-2"].supersedes_in_frame.as_deref(),
            Some("f-1")
        );
        let edge = &store.narrative_facts["f-3"].conflicts_with[0];
        assert_eq!(edge.target, "f-1");
        assert_eq!(
            edge.target_claim_sha256,
            sha256_hex("the count is an eccentric nobleman".as_bytes()),
            "write-time stamp computed, never copied from the proposal"
        );
    }

    /// Every reject class is named in its verdict; conflict dedup is
    /// parity with add_fact_conflict (either side, either order).
    #[test]
    fn edge_proposals_reject_classes_are_named() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("s.json");
        let mut store = edge_substrate(&path);
        // Record one conflict edge through the manual path.
        add_fact_conflict(&mut store, &path, "f-1", "f-3").unwrap();
        // And chain f-2 onto f-1 so fill-blanks fires.
        amend_fact(
            &mut store,
            &path,
            &FactImport {
                claim: "kara is alive".to_string(),
                canon_from: "ch-2".to_string(),
                evidence: vec!["ch-2".to_string()],
                supersedes_in_frame: Some("f-1".to_string()),
                ..sample_fact("f-2", "gt")
            },
            "chain",
        )
        .unwrap();
        let mut no_rationale = conflict_proposal(
            "f-2",
            "kara is alive",
            "f-3",
            "the count is an eccentric nobleman",
        );
        no_rationale.rationale = "  ".to_string();
        let file = edge_file(
            vec![
                // fill-blanks: f-2 already carries a pointer
                succession_proposal(
                    "f-2",
                    "kara is alive",
                    "f-1",
                    "the count is an eccentric nobleman",
                ),
                // duplicate successor in one file
                succession_proposal(
                    "f-2",
                    "kara is alive",
                    "f-3",
                    "the count is an eccentric nobleman",
                ),
            ],
            vec![
                // already recorded — REVERSED side (symmetric dedup, the
                // add_fact_conflict parity)
                conflict_proposal(
                    "f-3",
                    "the count is an eccentric nobleman",
                    "f-1",
                    "the count is an eccentric nobleman",
                ),
                // self-conflict
                conflict_proposal(
                    "f-1",
                    "the count is an eccentric nobleman",
                    "f-1",
                    "the count is an eccentric nobleman",
                ),
                no_rationale,
            ],
        );
        let report = import_edge_proposals(&mut store, &path, &file, "sha", false).unwrap();
        assert!(!report.applied);
        assert_eq!(report.rejected, 5);
        assert!(report.verdicts[0]
            .verdict
            .contains("already carries a succession pointer"));
        assert!(report.verdicts[1].verdict.contains("duplicate proposal"));
        assert!(report.verdicts[2].verdict.contains("already recorded"));
        assert!(report.verdicts[3].verdict.contains("conflict with itself"));
        assert!(report.verdicts[4].verdict.contains("rationale mandatory"));
        // Parity check: the manual path rejects the same recorded pair.
        assert!(matches!(
            add_fact_conflict(&mut store, &path, "f-3", "f-1"),
            Err(AtomicMutateError::FrozenLedger(_))
        ));
    }

    /// Two proposals that are each fine alone must not jointly close a
    /// cycle — the staged-edge overlay (R461 pin) catches the second.
    #[test]
    fn edge_proposals_joint_cycle_rejects() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("s.json");
        let mut store = edge_substrate(&path);
        let file = edge_file(
            vec![
                succession_proposal(
                    "f-2",
                    "kara is alive",
                    "f-1",
                    "the count is an eccentric nobleman",
                ),
                succession_proposal(
                    "f-1",
                    "the count is an eccentric nobleman",
                    "f-2",
                    "kara is alive",
                ),
            ],
            vec![],
        );
        let report = import_edge_proposals(&mut store, &path, &file, "sha", false).unwrap();
        assert!(!report.applied);
        assert_eq!(report.verdicts[0].verdict, "accepted");
        assert!(report.verdicts[1].verdict.contains("cycle"));
        assert!(store.narrative_facts["f-2"].supersedes_in_frame.is_none());
    }

    /// Loader boundary: schema tag mismatch, unknown fields (a stray
    /// confidence score must fail loud, never silently drop — the
    /// Goodhart guard), and a fully empty file.
    #[test]
    fn edge_proposals_loader_fails_loud() {
        let tmp = TempDir::new().unwrap();
        let write = |name: &str, body: &str| {
            let p = tmp.path().join(name);
            fs::write(&p, body).unwrap();
            p
        };
        let bad_schema = write("a.json", r#"{"schema":"edge-proposals/v0"}"#);
        assert!(load_edge_proposals(&bad_schema)
            .unwrap_err()
            .contains("not `edge-proposals/v1`"));
        let unknown_key = write(
            "b.json",
            r#"{"schema":"edge-proposals/v1","succession":[{"successor":"a","predecessor":"b","successor_claim_sha256":"x","predecessor_claim_sha256":"y","rationale":"r","confidence":0.9}]}"#,
        );
        assert!(load_edge_proposals(&unknown_key)
            .unwrap_err()
            .contains("does not parse"));
        let empty = write("c.json", r#"{"schema":"edge-proposals/v1"}"#);
        assert!(load_edge_proposals(&empty)
            .unwrap_err()
            .contains("no proposals"));
    }

    #[test]
    fn import_facts_round_trips_with_forward_succession() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_chapters(&mut store);
        // f-new supersedes f-old, declared BEFORE f-old in the manifest —
        // forward refs within one manifest are legal (refs checked post-stage).
        let mut f_new = sample_fact("f-new", "jonathan");
        f_new.canon_from = "ch-3".to_string();
        f_new.claim = "the count is something unnatural".to_string();
        f_new.supersedes_in_frame = Some("f-old".to_string());
        f_new.quote = Some("he crawled face-down the castle wall".to_string());
        let mut f_old = sample_fact("f-old", "jonathan");
        f_old.canon_to = Some("ch-2".to_string());
        let manifest = FactsManifest {
            disclosure_plans: vec![],
            entities: vec![],
            branches: vec![],
            frames: vec![
                FrameImport {
                    frame_id: "jonathan".to_string(),
                    description: "Jonathan Harker's epistemic frame".to_string(),
                },
                FrameImport {
                    frame_id: "ground-truth".to_string(),
                    description: String::new(),
                },
            ],
            predicates: vec![],
            facts: vec![f_new, f_old],
        };
        let receipt = import_facts(&mut store, &path, &manifest).unwrap();
        assert_eq!(
            receipt.target_id,
            "2 frames + 0 branches + 0 entities + 0 predicates + 2 facts + 0 disclosure-plans \
             + 0 disclosure-overrides created, 0 no-op"
        );
        let reloaded = AtomicStore::load(&path).unwrap();
        assert_eq!(reloaded.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(reloaded.frames.len(), 2);
        let new = &reloaded.narrative_facts["f-new"];
        assert_eq!(new.supersedes_in_frame.as_deref(), Some("f-old"));
        // quote_sha256 computed by the primitive, never caller-supplied.
        assert_eq!(
            new.quote_sha256.as_deref(),
            Some(sha256_hex("he crawled face-down the castle wall".as_bytes()).as_str())
        );
        assert_eq!(
            reloaded.narrative_facts["f-old"].canon_to.as_deref(),
            Some("ch-2")
        );
        // Idempotent re-import: pure no-op, nothing written.
        let again = import_facts(&mut store, &path, &manifest).unwrap();
        assert_eq!(again.written_bytes, 0);
        assert_eq!(
            again.target_id,
            "0 frames + 0 branches + 0 entities + 0 predicates + 0 facts + 0 disclosure-plans \
             + 0 disclosure-overrides created, 4 no-op"
        );
    }

    #[test]
    fn import_facts_rejects_unknown_frame_and_missing_refs() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_chapters(&mut store);
        // Unknown frame (registry empty).
        let manifest = FactsManifest {
            disclosure_plans: vec![],
            entities: vec![],
            branches: vec![],
            frames: vec![],
            predicates: vec![],
            facts: vec![sample_fact("f1", "nobody")],
        };
        let err = import_facts(&mut store, &path, &manifest).unwrap_err();
        assert!(err.to_string().contains("frames registry"), "{err}");
        // Evidence ref to a non-section.
        let frames = vec![FrameImport {
            frame_id: "gt".to_string(),
            description: String::new(),
        }];
        let mut bad_evidence = sample_fact("f1", "gt");
        bad_evidence.evidence = vec!["ch-99".to_string()];
        let err = import_facts(
            &mut store,
            &path,
            &FactsManifest {
                disclosure_plans: vec![],
                entities: vec![],
                frames: frames.clone(),
                branches: vec![],
                predicates: vec![],
                facts: vec![bad_evidence],
            },
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("not present as a section"),
            "{err}"
        );
        // Empty evidence: a claim without provenance is unauditable.
        let mut no_evidence = sample_fact("f1", "gt");
        no_evidence.evidence = vec![];
        let err = import_facts(
            &mut store,
            &path,
            &FactsManifest {
                disclosure_plans: vec![],
                entities: vec![],
                branches: vec![],
                frames: frames.clone(),
                predicates: vec![],
                facts: vec![no_evidence],
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("evidence mandatory"), "{err}");
        // canon_from must be a section.
        let mut bad_canon = sample_fact("f1", "gt");
        bad_canon.canon_from = "ch-99".to_string();
        let err = import_facts(
            &mut store,
            &path,
            &FactsManifest {
                disclosure_plans: vec![],
                entities: vec![],
                branches: vec![],
                frames,
                predicates: vec![],
                facts: vec![bad_canon],
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("canon_from"), "{err}");
        // Every rejection happened before any save: store file never created.
        assert!(!path.exists());
    }

    #[test]
    fn import_facts_rejects_cross_frame_succession() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_chapters(&mut store);
        let mut successor = sample_fact("f2", "seward");
        successor.supersedes_in_frame = Some("f1".to_string());
        let manifest = FactsManifest {
            disclosure_plans: vec![],
            entities: vec![],
            branches: vec![],
            frames: vec![
                FrameImport {
                    frame_id: "jonathan".to_string(),
                    description: String::new(),
                },
                FrameImport {
                    frame_id: "seward".to_string(),
                    description: String::new(),
                },
            ],
            predicates: vec![],
            facts: vec![sample_fact("f1", "jonathan"), successor],
        };
        let err = import_facts(&mut store, &path, &manifest).unwrap_err();
        assert!(
            err.to_string().contains("in-frame succession only"),
            "{err}"
        );
        assert!(!path.exists());
    }

    #[test]
    fn import_facts_divergent_rejects_whole_manifest() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_chapters(&mut store);
        let frames = vec![FrameImport {
            frame_id: "gt".to_string(),
            description: String::new(),
        }];
        import_facts(
            &mut store,
            &path,
            &FactsManifest {
                disclosure_plans: vec![],
                entities: vec![],
                branches: vec![],
                frames: frames.clone(),
                predicates: vec![],
                facts: vec![sample_fact("f1", "gt")],
            },
        )
        .unwrap();
        let before = std::fs::read_to_string(&path).unwrap();
        let mut divergent = sample_fact("f1", "gt");
        divergent.claim = "a different claim".to_string();
        let err = import_facts(
            &mut store,
            &path,
            &FactsManifest {
                disclosure_plans: vec![],
                entities: vec![],
                branches: vec![],
                frames,
                predicates: vec![],
                facts: vec![divergent],
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("DIVERGENT"), "{err}");
        // Reject happened before the save — on-disk store untouched.
        assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
    }

    /// R305 field-invariant parity: the SAME edge-case inputs through both
    /// write paths must produce the SAME accept/reject verdict.
    #[test]
    fn fact_write_path_parity_add_vs_import() {
        let cases: Vec<(&str, FactImport)> = vec![
            ("valid", sample_fact("p1", "gt")),
            ("unknown frame", sample_fact("p2", "nobody")),
            (
                "missing evidence ref",
                FactImport {
                    entities: vec![],
                    evidence: vec!["ch-99".to_string()],
                    ..sample_fact("p3", "gt")
                },
            ),
            (
                "empty evidence",
                FactImport {
                    entities: vec![],
                    evidence: vec![],
                    ..sample_fact("p4", "gt")
                },
            ),
            (
                "self conflict",
                FactImport {
                    entities: vec![],
                    conflicts_with: vec!["p5".to_string()],
                    ..sample_fact("p5", "gt")
                },
            ),
            (
                "blank canon_to",
                FactImport {
                    entities: vec![],
                    canon_to: Some("  ".to_string()),
                    ..sample_fact("p6", "gt")
                },
            ),
            (
                "blank branch",
                FactImport {
                    entities: vec![],
                    branch: Some("  ".to_string()),
                    ..sample_fact("p7", "gt")
                },
            ),
            (
                "unregistered branch",
                FactImport {
                    branch: Some("sea-rotue".to_string()),
                    ..sample_fact("p8", "gt")
                },
            ),
            (
                "unregistered entity",
                FactImport {
                    entities: vec!["dracual".to_string()],
                    ..sample_fact("p9", "gt")
                },
            ),
            // Round 442 — setup/payoff field invariants, same parity set.
            (
                "self pays_off",
                FactImport {
                    pays_off: vec!["p10".to_string()],
                    ..sample_fact("p10", "gt")
                },
            ),
            (
                "blank pays_off ref",
                FactImport {
                    pays_off: vec!["  ".to_string()],
                    ..sample_fact("p11", "gt")
                },
            ),
            (
                "duplicate pays_off ref",
                FactImport {
                    pays_off: vec!["x".to_string(), "x".to_string()],
                    ..sample_fact("p12", "gt")
                },
            ),
            (
                "missing pays_off target",
                FactImport {
                    pays_off: vec!["never-written".to_string()],
                    ..sample_fact("p13", "gt")
                },
            ),
            (
                "unknown payoff_expectation tag",
                FactImport {
                    payoff_expectation: Some("chekhov".to_string()),
                    ..sample_fact("p14", "gt")
                },
            ),
        ];
        for (label, entry) in cases {
            let tmp = TempDir::new().unwrap();
            let path_a = tmp.path().join("a.json");
            let path_b = tmp.path().join("b.json");
            let mut store_a = AtomicStore::new();
            let mut store_b = AtomicStore::new();
            for s in [&mut store_a, &mut store_b] {
                seed_chapters(s);
                s.frames.insert("gt".to_string(), Frame::default());
            }
            let add_ok = add_fact(&mut store_a, &path_a, &entry).is_ok();
            let import_ok = import_facts(
                &mut store_b,
                &path_b,
                &FactsManifest {
                    disclosure_plans: vec![],
                    entities: vec![],
                    frames: vec![],
                    branches: vec![],
                    predicates: vec![],
                    facts: vec![entry],
                },
            )
            .is_ok();
            assert_eq!(
                add_ok, import_ok,
                "write-path parity broken for case `{label}`: add={add_ok} import={import_ok}"
            );
        }
    }

    /// Round 442 — setup/payoff fields: tag parses through the shared
    /// builder, the edge round-trips, the unmarked default never serializes
    /// (pre-payoff stores stay byte-stable), and an inbound `pays_off` ref
    /// blocks retraction exactly like conflict/succession refs.
    #[test]
    fn fact_payoff_fields_roundtrip_and_block_retract() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("s.json");
        let mut store = AtomicStore::new();
        seed_chapters(&mut store);
        store.frames.insert("gt".to_string(), Frame::default());
        add_fact(
            &mut store,
            &path,
            &FactImport {
                payoff_expectation: Some("expected".to_string()),
                ..sample_fact("setup-1", "gt")
            },
        )
        .unwrap();
        add_fact(
            &mut store,
            &path,
            &FactImport {
                pays_off: vec!["setup-1".to_string()],
                ..sample_fact("payoff-1", "gt")
            },
        )
        .unwrap();
        let reloaded = AtomicStore::load(&path).unwrap();
        assert_eq!(
            reloaded.narrative_facts["setup-1"].payoff_expectation,
            PayoffExpectation::Expected
        );
        assert_eq!(
            reloaded.narrative_facts["payoff-1"].pays_off,
            vec!["setup-1".to_string()]
        );
        // The unmarked default stays off the wire (byte-stability).
        let raw = fs::read_to_string(&path).unwrap();
        assert!(!raw.contains("unmarked"));
        // Inbound pays_off ref blocks retraction (referential fail-loud).
        let err = retract_fact(&mut store, &path, "setup-1", "test").unwrap_err();
        assert!(
            err.to_string().contains("pays_off"),
            "retract must name the payoff referrer: {err}"
        );
        retract_fact(&mut store, &path, "payoff-1", "test").unwrap();
        retract_fact(&mut store, &path, "setup-1", "test").unwrap();
    }

    /// Round 446 — typed-leg invariants, the SAME edge inputs through both
    /// write paths (R305 field-invariant parity; the rule that caught the
    /// R295 paste-error). Registry/shape rejects must agree between
    /// `add_fact` and `import_facts`.
    #[test]
    fn typed_claim_write_path_parity_add_vs_import() {
        let typed = |subject: &str, predicate: &str, object: TypedObject| {
            Some(TypedClaim {
                subject: subject.to_string(),
                predicate: predicate.to_string(),
                object,
            })
        };
        let scalar = |v: &str| TypedObject::Value {
            value: v.to_string(),
        };
        let ent = |id: &str| TypedObject::Entity { id: id.to_string() };
        let with_entities = |fact_id: &str, ents: &[&str], t| FactImport {
            entities: ents.iter().map(|s| s.to_string()).collect(),
            typed: t,
            ..sample_fact(fact_id, "gt")
        };
        let cases: Vec<(&str, FactImport)> = vec![
            (
                "valid scalar typed",
                with_entities(
                    "t1",
                    &["kara"],
                    typed("kara", "alive", scalar("operational")),
                ),
            ),
            (
                "valid entity typed",
                with_entities(
                    "t2",
                    &["kara", "todd-gun"],
                    typed("kara", "holds", ent("todd-gun")),
                ),
            ),
            (
                "unknown predicate",
                with_entities("t3", &["kara"], typed("kara", "at-locaton", scalar("x"))),
            ),
            (
                "subject not in entities list",
                with_entities("t4", &[], typed("kara", "alive", scalar("operational"))),
            ),
            (
                "subject unregistered",
                with_entities("t5", &["kara"], typed("alucard", "alive", scalar("x"))),
            ),
            (
                "object entity not listed",
                with_entities("t6", &["kara"], typed("kara", "holds", ent("todd-gun"))),
            ),
            (
                "entity object on scalar predicate",
                with_entities(
                    "t7",
                    &["kara", "todd-gun"],
                    typed("kara", "alive", ent("todd-gun")),
                ),
            ),
            (
                "value object on entity predicate",
                with_entities("t8", &["kara"], typed("kara", "holds", scalar("todd-gun"))),
            ),
            (
                "blank subject",
                with_entities("t9", &["kara"], typed("  ", "alive", scalar("x"))),
            ),
            (
                "blank scalar value",
                with_entities("t10", &["kara"], typed("kara", "alive", scalar("  "))),
            ),
        ];
        for (label, entry) in cases {
            let tmp = TempDir::new().unwrap();
            let path_a = tmp.path().join("a.json");
            let path_b = tmp.path().join("b.json");
            let mut store_a = AtomicStore::new();
            let mut store_b = AtomicStore::new();
            for s in [&mut store_a, &mut store_b] {
                seed_chapters(s);
                s.frames.insert("gt".to_string(), Frame::default());
                s.entities.insert("kara".to_string(), Entity::default());
                s.entities.insert("todd-gun".to_string(), Entity::default());
                s.predicates.insert(
                    "alive".to_string(),
                    Predicate {
                        object_kind: PredicateObjectKind::Scalar,
                        description: String::new(),
                    },
                );
                s.predicates.insert(
                    "holds".to_string(),
                    Predicate {
                        object_kind: PredicateObjectKind::Entity,
                        description: String::new(),
                    },
                );
            }
            let add_ok = add_fact(&mut store_a, &path_a, &entry).is_ok();
            let import_ok = import_facts(
                &mut store_b,
                &path_b,
                &FactsManifest {
                    disclosure_plans: vec![],
                    entities: vec![],
                    frames: vec![],
                    branches: vec![],
                    predicates: vec![],
                    facts: vec![entry],
                },
            )
            .is_ok();
            assert_eq!(
                add_ok, import_ok,
                "typed write-path parity broken for case `{label}`: add={add_ok} import={import_ok}"
            );
        }
    }

    /// Round 590 — the disclosure write-path parity guard (CLAUDE.md
    /// multi-write-path rule): the all-primitive manifest's disclosure override
    /// path and the standalone `set_disclosure` must accept-or-reject the SAME
    /// edge cases — especially the gate-enabling typed-fact invariant. Both route
    /// through `apply_disclosure_override`, so this pins that they cannot drift.
    #[test]
    fn disclosure_write_path_parity_set_vs_manifest() {
        // (label, fact_id, mode, first_at pairs)
        let cases: Vec<(&str, &str, &str, Vec<[String; 2]>)> = vec![
            ("state on typed fact", "typed-1", "state", vec![]),
            ("state on untyped fact (no pin)", "prose-1", "state", vec![]),
            // Typed-fact invariant: a withhold OR a first_at pin needs a typed leg.
            ("withhold on untyped fact", "prose-1", "withhold", vec![]),
            (
                "first_at on untyped fact",
                "prose-1",
                "state",
                vec![["main".to_string(), "ch-2".to_string()]],
            ),
            (
                "first_at on typed fact",
                "typed-1",
                "state",
                vec![["main".to_string(), "ch-2".to_string()]],
            ),
            ("unknown mode", "typed-1", "bogus", vec![]),
            ("missing fact", "ghost", "state", vec![]),
            (
                "first_at unregistered branch",
                "typed-1",
                "state",
                vec![["ghost-world".to_string(), "ch-2".to_string()]],
            ),
            (
                "first_at unknown coord",
                "typed-1",
                "state",
                vec![["main".to_string(), "ch-404".to_string()]],
            ),
        ];
        for (label, fact_id, mode, first_at) in cases {
            let tmp = TempDir::new().unwrap();
            let path_a = tmp.path().join("a.json");
            let path_b = tmp.path().join("b.json");
            let mut store_a = AtomicStore::new();
            let mut store_b = AtomicStore::new();
            for (s, p) in [(&mut store_a, &path_a), (&mut store_b, &path_b)] {
                seed_chapters(s);
                s.frames.insert("gt".to_string(), Frame::default());
                s.entities.insert("kara".to_string(), Entity::default());
                s.predicates.insert(
                    "alive".to_string(),
                    Predicate {
                        object_kind: PredicateObjectKind::Scalar,
                        description: String::new(),
                    },
                );
                add_fact(
                    s,
                    p,
                    &FactImport {
                        entities: vec!["kara".to_string()],
                        typed: Some(TypedClaim {
                            subject: "kara".to_string(),
                            predicate: "alive".to_string(),
                            object: TypedObject::Value {
                                value: "operational".to_string(),
                            },
                        }),
                        ..sample_fact("typed-1", "gt")
                    },
                )
                .unwrap();
                add_fact(s, p, &sample_fact("prose-1", "gt")).unwrap();
                add_disclosure_plan(s, p, "reader", "withhold", "").unwrap();
            }

            // Standalone path.
            let first_at_pairs: Vec<(String, String)> = first_at
                .iter()
                .map(|[b, c]| (b.clone(), c.clone()))
                .collect();
            let set_ok = set_disclosure(
                &mut store_a,
                &path_a,
                DisclosureDecision {
                    telling_id: "reader",
                    fact_id,
                    mode,
                    first_at: &first_at_pairs,
                    surface: None,
                },
            )
            .is_ok();

            // Manifest path (the plan already exists → policy no-op, override applies).
            let import_ok = import_facts(
                &mut store_b,
                &path_b,
                &FactsManifest {
                    frames: vec![],
                    branches: vec![],
                    entities: vec![],
                    predicates: vec![],
                    facts: vec![],
                    disclosure_plans: vec![DisclosurePlanImport {
                        telling_id: "reader".to_string(),
                        default_mode: Some("withhold".to_string()),
                        description: String::new(),
                        overrides: vec![DisclosureOverrideImport {
                            fact_id: fact_id.to_string(),
                            mode: mode.to_string(),
                            first_at,
                            surface: None,
                        }],
                    }],
                },
            )
            .is_ok();

            assert_eq!(
                set_ok, import_ok,
                "disclosure write-path parity broken for `{label}`: set={set_ok} import={import_ok}"
            );
            // On accept, both paths must produce the IDENTICAL stored override —
            // parity of the result, not just of the accept/reject verdict (R592).
            if set_ok {
                assert_eq!(
                    store_a.disclosure_plans, store_b.disclosure_plans,
                    "disclosure stored-result parity broken for `{label}`"
                );
            }
        }
    }

    /// Round 446 — the predicate registry: tag parse fail-loud, A2 3-way
    /// verdicts via the shared staging path, and the typed leg round-trips
    /// while an untyped fact stays byte-stable (no `typed` key on the
    /// wire).
    #[test]
    fn predicate_registry_and_typed_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("s.json");
        let mut store = AtomicStore::new();
        seed_chapters(&mut store);
        store.frames.insert("gt".to_string(), Frame::default());
        store.entities.insert("kara".to_string(), Entity::default());
        // Unknown object_kind tag rejects (no silent default).
        let err = add_predicate(&mut store, &path, "alive", "boolean", "").unwrap_err();
        assert!(err.to_string().contains("unknown object_kind"), "{err}");
        // Create, then byte-identical no-op, then divergent reject.
        add_predicate(&mut store, &path, "alive", "scalar", "life state").unwrap();
        let receipt = add_predicate(&mut store, &path, "alive", "scalar", "life state").unwrap();
        assert!(receipt.target_id.ends_with("(no-op)"));
        let err = add_predicate(&mut store, &path, "alive", "entity", "life state").unwrap_err();
        assert!(err.to_string().contains("DIVERGENT"), "{err}");
        // Typed fact round-trips; prose-only fact never serializes `typed`.
        add_fact(
            &mut store,
            &path,
            &FactImport {
                entities: vec!["kara".to_string()],
                typed: Some(TypedClaim {
                    subject: "kara".to_string(),
                    predicate: "alive".to_string(),
                    object: TypedObject::Value {
                        value: "operational".to_string(),
                    },
                }),
                ..sample_fact("typed-1", "gt")
            },
        )
        .unwrap();
        add_fact(&mut store, &path, &sample_fact("prose-1", "gt")).unwrap();
        let reloaded = AtomicStore::load(&path).unwrap();
        let t = reloaded.narrative_facts["typed-1"].typed.as_ref().unwrap();
        assert_eq!(t.subject, "kara");
        assert_eq!(t.predicate, "alive");
        assert_eq!(
            t.object,
            TypedObject::Value {
                value: "operational".to_string()
            }
        );
        assert!(reloaded.narrative_facts["prose-1"].typed.is_none());
        let raw = fs::read_to_string(&path).unwrap();
        assert_eq!(
            raw.matches("\"typed\"").count(),
            1,
            "prose-only facts stay off the wire"
        );
        // Reloaded store carries the bumped schema version.
        assert_eq!(reloaded.schema_version, CURRENT_SCHEMA_VERSION);
    }

    /// Round 446 — manifest path: predicates land before facts in ONE
    /// transaction (a fact may use a predicate declared in the same
    /// manifest), and `amend_fact` can attach a typed leg to an existing
    /// prose fact (the dogfood typing path).
    #[test]
    fn import_facts_predicates_first_and_amend_attaches_typed() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("s.json");
        let mut store = AtomicStore::new();
        seed_chapters(&mut store);
        import_facts(
            &mut store,
            &path,
            &FactsManifest {
                disclosure_plans: vec![],
                frames: vec![FrameImport {
                    frame_id: "gt".to_string(),
                    description: String::new(),
                }],
                branches: vec![],
                entities: vec![EntityImport {
                    entity_id: "kara".to_string(),
                    kind: String::new(),
                    description: String::new(),
                }],
                predicates: vec![PredicateImport {
                    predicate_id: "alive".to_string(),
                    object_kind: "scalar".to_string(),
                    description: String::new(),
                }],
                facts: vec![FactImport {
                    entities: vec!["kara".to_string()],
                    typed: Some(TypedClaim {
                        subject: "kara".to_string(),
                        predicate: "alive".to_string(),
                        object: TypedObject::Value {
                            value: "operational".to_string(),
                        },
                    }),
                    ..sample_fact("f-1", "gt")
                }],
            },
        )
        .unwrap();
        assert!(store.predicates.contains_key("alive"));
        // Amend a prose fact to attach its typed leg, id unchanged.
        add_fact(
            &mut store,
            &path,
            &FactImport {
                entities: vec!["kara".to_string()],
                ..sample_fact("f-2", "gt")
            },
        )
        .unwrap();
        amend_fact(
            &mut store,
            &path,
            &FactImport {
                entities: vec!["kara".to_string()],
                typed: Some(TypedClaim {
                    subject: "kara".to_string(),
                    predicate: "alive".to_string(),
                    object: TypedObject::Value {
                        value: "destroyed".to_string(),
                    },
                }),
                ..sample_fact("f-2", "gt")
            },
            "typing pass",
        )
        .unwrap();
        assert!(store.narrative_facts["f-2"].typed.is_some());
    }

    /// Round 443 session review — a `pays_off` forward ref WITHIN one
    /// manifest is legal (the store ∪ manifest visibility set, the
    /// succession symmetry), while `add_fact` against the bare store still
    /// rejects the same shape.
    #[test]
    fn import_facts_allows_forward_payoff_ref_within_manifest() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("s.json");
        let mut store = AtomicStore::new();
        seed_chapters(&mut store);
        store.frames.insert("gt".to_string(), Frame::default());
        import_facts(
            &mut store,
            &path,
            &FactsManifest {
                disclosure_plans: vec![],
                frames: vec![],
                branches: vec![],
                entities: vec![],
                predicates: vec![],
                facts: vec![
                    // The payoff lists FIRST, naming a setup later in the
                    // same manifest.
                    FactImport {
                        pays_off: vec!["su-later".to_string()],
                        ..sample_fact("p-first", "gt")
                    },
                    FactImport {
                        payoff_expectation: Some("expected".to_string()),
                        ..sample_fact("su-later", "gt")
                    },
                ],
            },
        )
        .unwrap();
        assert_eq!(
            store.narrative_facts["p-first"].pays_off,
            vec!["su-later".to_string()]
        );
    }

    /// Round 433 — world-line branch axis: omitted branch = MAIN_BRANCH, the
    /// default never serializes (pre-branch stores stay byte-stable), and a
    /// declared branch round-trips.
    #[test]
    fn fact_branch_defaults_to_main_and_skips_serialization() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("s.json");
        let mut store = AtomicStore::new();
        seed_chapters(&mut store);
        store.frames.insert("gt".to_string(), Frame::default());
        store
            .branches
            .insert("vampire-route".to_string(), Branch::default());
        add_fact(&mut store, &path, &sample_fact("f-main", "gt")).unwrap();
        add_fact(
            &mut store,
            &path,
            &FactImport {
                entities: vec![],
                branch: Some("vampire-route".to_string()),
                ..sample_fact("f-route", "gt")
            },
        )
        .unwrap();
        assert_eq!(
            store.narrative_facts["f-main"].branch,
            mnemosyne_core::MAIN_BRANCH
        );
        assert_eq!(store.narrative_facts["f-route"].branch, "vampire-route");
        let raw = std::fs::read_to_string(&path).unwrap();
        let json: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(
            json["narrative_facts"]["f-main"].get("branch").is_none(),
            "default branch must not serialize"
        );
        assert_eq!(
            json["narrative_facts"]["f-route"]["branch"],
            "vampire-route"
        );
        let reloaded = AtomicStore::load(&path).unwrap();
        assert_eq!(
            reloaded.narrative_facts["f-main"].branch,
            mnemosyne_core::MAIN_BRANCH
        );
    }

    /// Round 433 — succession never crosses a world-line (B-2: scope =
    /// (frame, branch); branch divergence is data, not succession).
    #[test]
    fn succession_across_branches_rejected_on_both_write_paths() {
        let tmp = TempDir::new().unwrap();
        let path_a = tmp.path().join("a.json");
        let path_b = tmp.path().join("b.json");
        let mut store_a = AtomicStore::new();
        let mut store_b = AtomicStore::new();
        for s in [&mut store_a, &mut store_b] {
            seed_chapters(s);
            s.frames.insert("gt".to_string(), Frame::default());
            s.branches
                .insert("vampire-route".to_string(), Branch::default());
        }
        let predecessor = sample_fact("f-old", "gt");
        let successor = FactImport {
            entities: vec![],
            branch: Some("vampire-route".to_string()),
            supersedes_in_frame: Some("f-old".to_string()),
            ..sample_fact("f-new", "gt")
        };
        add_fact(&mut store_a, &path_a, &predecessor).unwrap();
        let err = add_fact(&mut store_a, &path_a, &successor).unwrap_err();
        assert!(err.to_string().contains("world-line"), "{err}");
        let err = import_facts(
            &mut store_b,
            &path_b,
            &FactsManifest {
                disclosure_plans: vec![],
                entities: vec![],
                frames: vec![],
                branches: vec![],
                predicates: vec![],
                facts: vec![predecessor, successor],
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("world-line"), "{err}");
    }

    /// Round 434 — authorial retract: removes an unreferenced fact, demands
    /// a reason, fails loud on inbound refs and on a missing fact.
    #[test]
    fn retract_fact_removes_unreferenced_and_blocks_referenced() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("s.json");
        let mut store = AtomicStore::new();
        seed_chapters(&mut store);
        store.frames.insert("gt".to_string(), Frame::default());
        add_fact(&mut store, &path, &sample_fact("f-typo", "gt")).unwrap();
        add_fact(&mut store, &path, &sample_fact("f-kept", "gt")).unwrap();
        // Reason is mandatory (audit-trail safeguard).
        let err = retract_fact(&mut store, &path, "f-typo", "  ").unwrap_err();
        assert!(err.to_string().contains("reason"), "{err}");
        // Referenced fact cannot be retracted (conflict edge inbound).
        add_fact_conflict(&mut store, &path, "f-kept", "f-typo").unwrap();
        let err = retract_fact(&mut store, &path, "f-typo", "authorial slip").unwrap_err();
        assert!(err.to_string().contains("f-kept"), "{err}");
        // Retract the referrer first, then the target goes cleanly.
        retract_fact(&mut store, &path, "f-kept", "drop the edge holder").unwrap();
        retract_fact(&mut store, &path, "f-typo", "authorial slip").unwrap();
        assert!(store.narrative_facts.is_empty());
        let reloaded = AtomicStore::load(&path).unwrap();
        assert!(reloaded.narrative_facts.is_empty());
        // Missing fact = NotFound.
        let err = retract_fact(&mut store, &path, "f-gone", "x").unwrap_err();
        assert!(matches!(err, AtomicMutateError::NotFound(_)));
    }

    /// Round 434 — authorial amend: in-place revision through the shared
    /// builder (quote hash restamped), no-op on identical content, NotFound
    /// on a missing fact, and same-scope inbound-successor protection.
    #[test]
    fn amend_fact_revises_in_place_with_parity_invariants() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("s.json");
        let mut store = AtomicStore::new();
        seed_chapters(&mut store);
        store.frames.insert("gt".to_string(), Frame::default());
        store.frames.insert("mina".to_string(), Frame::default());
        add_fact(&mut store, &path, &sample_fact("f1", "gt")).unwrap();
        // Amend a missing fact = NotFound (creation is add_fact).
        let err = amend_fact(&mut store, &path, &sample_fact("f-absent", "gt"), "fix").unwrap_err();
        assert!(matches!(err, AtomicMutateError::NotFound(_)));
        // Revision lands; quote_sha256 restamped by the builder.
        let revised = FactImport {
            entities: vec![],
            claim: "the count is a nobleman of the Carpathians".to_string(),
            quote: Some("boyar blood".to_string()),
            ..sample_fact("f1", "gt")
        };
        amend_fact(&mut store, &path, &revised, "typo fix").unwrap();
        let f1 = &store.narrative_facts["f1"];
        assert_eq!(f1.claim, "the count is a nobleman of the Carpathians");
        assert_eq!(
            f1.quote_sha256.as_deref(),
            Some(sha256_hex("boyar blood".as_bytes()).as_str())
        );
        // Byte-identical re-amend = no-op, nothing written.
        let again = amend_fact(&mut store, &path, &revised, "same").unwrap();
        assert_eq!(again.written_bytes, 0);
        // Shared-builder invariants hold on the amend path too (parity).
        let bad = FactImport {
            entities: vec![],
            evidence: vec![],
            ..revised.clone()
        };
        let err = amend_fact(&mut store, &path, &bad, "x").unwrap_err();
        assert!(err.to_string().contains("evidence"), "{err}");
        // A superseded fact cannot be amended out of its scope.
        let successor = FactImport {
            entities: vec![],
            canon_from: "ch-3".to_string(),
            supersedes_in_frame: Some("f1".to_string()),
            ..sample_fact("f2", "gt")
        };
        add_fact(&mut store, &path, &successor).unwrap();
        let moved = FactImport {
            entities: vec![],
            frame: "mina".to_string(),
            ..revised.clone()
        };
        let err = amend_fact(&mut store, &path, &moved, "x").unwrap_err();
        assert!(err.to_string().contains("superseded by `f2`"), "{err}");
        // Divergent add_fact now advises BOTH paths (sec 7.10 finding).
        let divergent = FactImport {
            entities: vec![],
            claim: "something else".to_string(),
            ..sample_fact("f1", "gt")
        };
        let err = add_fact(&mut store, &path, &divergent).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("supersede in-frame") && msg.contains("amend_fact"),
            "{msg}"
        );
    }

    /// Round 436 — branch registry: the default never registers, divergent
    /// descriptions reject, idempotent no-op, and registration is what
    /// unlocks authoring onto the world-line (write-side typo gate).
    #[test]
    fn add_branch_registry_gates_fact_writes() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("s.json");
        let mut store = AtomicStore::new();
        seed_chapters(&mut store);
        store.frames.insert("gt".to_string(), Frame::default());
        // The default world-line is known by construction, never registered.
        let err = add_branch(
            &mut store,
            &path,
            mnemosyne_core::MAIN_BRANCH,
            "",
            None,
            &[],
        )
        .unwrap_err();
        assert!(err.to_string().contains("known by construction"), "{err}");
        // Unregistered branch on a fact rejects at the write path.
        let on_route = FactImport {
            entities: vec![],
            branch: Some("sea-route".to_string()),
            ..sample_fact("f-route", "gt")
        };
        let err = add_fact(&mut store, &path, &on_route).unwrap_err();
        assert!(err.to_string().contains("branch registry"), "{err}");
        // Register, then the same write lands.
        add_branch(
            &mut store,
            &path,
            "sea-route",
            "the Demeter voyage",
            None,
            &[],
        )
        .unwrap();
        add_fact(&mut store, &path, &on_route).unwrap();
        assert_eq!(store.narrative_facts["f-route"].branch, "sea-route");
        // Idempotent re-register = no-op; divergent description rejects.
        let again = add_branch(
            &mut store,
            &path,
            "sea-route",
            "the Demeter voyage",
            None,
            &[],
        )
        .unwrap();
        assert_eq!(again.written_bytes, 0);
        let err =
            add_branch(&mut store, &path, "sea-route", "something else", None, &[]).unwrap_err();
        assert!(err.to_string().contains("DIVERGENT"), "{err}");
        let reloaded = AtomicStore::load(&path).unwrap();
        assert_eq!(
            reloaded.branches["sea-route"].description,
            "the Demeter voyage"
        );
    }

    /// Round 438 — fork registration invariants + lineage succession on the
    /// write path.
    #[test]
    fn add_branch_fork_validation_and_lineage_succession() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("s.json");
        let mut store = AtomicStore::new();
        seed_chapters(&mut store);
        store.frames.insert("gt".to_string(), Frame::default());
        // Round 448 — blank id fails with the precise cause even when a
        // fork declaration is present (not a blank-named fork message).
        let err = add_branch(&mut store, &path, "  ", "", Some(("main", "ch-2")), &[]).unwrap_err();
        assert!(err.to_string().contains("branch_id mandatory"), "{err}");
        // Parent must pre-exist; fork point must be a section; no self-fork.
        let err =
            add_branch(&mut store, &path, "deep", "", Some(("route", "ch-2")), &[]).unwrap_err();
        assert!(err.to_string().contains("fork parent"), "{err}");
        let err =
            add_branch(&mut store, &path, "route", "", Some(("route", "ch-2")), &[]).unwrap_err();
        assert!(err.to_string().contains("itself"), "{err}");
        let err =
            add_branch(&mut store, &path, "route", "", Some(("main", "ch-99")), &[]).unwrap_err();
        assert!(err.to_string().contains("ch-99"), "{err}");
        // Valid fork round-trips; immutable thereafter (divergent reject).
        add_branch(&mut store, &path, "route", "", Some(("main", "ch-2")), &[]).unwrap();
        let reloaded = AtomicStore::load(&path).unwrap();
        let fork = reloaded.branches["route"].forks_from.as_ref().unwrap();
        assert_eq!((fork.branch.as_str(), fork.at.as_str()), ("main", "ch-2"));
        let err =
            add_branch(&mut store, &path, "route", "", Some(("main", "ch-3")), &[]).unwrap_err();
        assert!(err.to_string().contains("DIVERGENT"), "{err}");
        // Lineage succession: a route fact may supersede an inherited main
        // fact (in-world change inside one world-line); an unrelated
        // standalone branch still rejects.
        add_fact(&mut store, &path, &sample_fact("f-old", "gt")).unwrap();
        let revision = FactImport {
            branch: Some("route".to_string()),
            canon_from: "ch-3".to_string(),
            supersedes_in_frame: Some("f-old".to_string()),
            ..sample_fact("f-new", "gt")
        };
        add_fact(&mut store, &path, &revision).unwrap();
        add_branch(&mut store, &path, "standalone", "", None, &[]).unwrap();
        let stray = FactImport {
            branch: Some("standalone".to_string()),
            canon_from: "ch-3".to_string(),
            supersedes_in_frame: Some("f-old".to_string()),
            ..sample_fact("f-stray", "gt")
        };
        let err = add_fact(&mut store, &path, &stray).unwrap_err();
        assert!(err.to_string().contains("does not inherit"), "{err}");
    }

    /// Round 532 — confluence (`converges_from`) registration invariants: ≥ 2
    /// distinct registered parents, each merge point a real section, the
    /// fork-XOR-confluence rule, and the idempotent/divergent round-trip
    /// (the `add_branch_fork_validation` analog for the merge side).
    #[test]
    fn add_branch_confluence_validation() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("s.json");
        let mut store = AtomicStore::new();
        seed_chapters(&mut store);
        store.frames.insert("gt".to_string(), Frame::default());
        // Two parent world-lines to converge.
        add_branch(&mut store, &path, "sluice", "", Some(("main", "ch-2")), &[]).unwrap();
        add_branch(&mut store, &path, "ride", "", Some(("main", "ch-2")), &[]).unwrap();
        // A confluence merges >= 2 parents (a 1-parent merge is just a fork).
        let err =
            add_branch(&mut store, &path, "dawn", "", None, &[("sluice", "ch-3")]).unwrap_err();
        assert!(err.to_string().contains("≥ 2 parent"), "{err}");
        // Parent must pre-exist.
        let err = add_branch(
            &mut store,
            &path,
            "dawn",
            "",
            None,
            &[("sluice", "ch-3"), ("ghost", "ch-3")],
        )
        .unwrap_err();
        assert!(err.to_string().contains("converge parent"), "{err}");
        // Merge point must be a section.
        let err = add_branch(
            &mut store,
            &path,
            "dawn",
            "",
            None,
            &[("sluice", "ch-99"), ("ride", "ch-3")],
        )
        .unwrap_err();
        assert!(err.to_string().contains("ch-99"), "{err}");
        // A duplicate parent rejects.
        let err = add_branch(
            &mut store,
            &path,
            "dawn",
            "",
            None,
            &[("sluice", "ch-3"), ("sluice", "ch-2")],
        )
        .unwrap_err();
        assert!(err.to_string().contains("more than once"), "{err}");
        // Fork XOR confluence — never both.
        let err = add_branch(
            &mut store,
            &path,
            "dawn",
            "",
            Some(("main", "ch-2")),
            &[("sluice", "ch-3"), ("ride", "ch-3")],
        )
        .unwrap_err();
        assert!(err.to_string().contains("never both"), "{err}");
        // Valid confluence round-trips; forks_from stays None.
        add_branch(
            &mut store,
            &path,
            "dawn",
            "",
            None,
            &[("sluice", "ch-3"), ("ride", "ch-3")],
        )
        .unwrap();
        let reloaded = AtomicStore::load(&path).unwrap();
        assert_eq!(reloaded.branches["dawn"].converges_from.len(), 2);
        assert!(reloaded.branches["dawn"].forks_from.is_none());
        // Idempotent re-register = no-op; a divergent merge set rejects.
        let again = add_branch(
            &mut store,
            &path,
            "dawn",
            "",
            None,
            &[("sluice", "ch-3"), ("ride", "ch-3")],
        )
        .unwrap();
        assert_eq!(again.written_bytes, 0);
        let err = add_branch(
            &mut store,
            &path,
            "dawn",
            "",
            None,
            &[("sluice", "ch-2"), ("ride", "ch-3")],
        )
        .unwrap_err();
        assert!(err.to_string().contains("DIVERGENT"), "{err}");
    }

    /// Round 440 — parity fix regression: an idempotent re-import of a
    /// manifest WITH conflict edges must no-op (pins stamp before the
    /// verdict, exactly like add_fact).
    #[test]
    fn import_facts_with_edges_reimports_as_no_op() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("s.json");
        let mut store = AtomicStore::new();
        seed_chapters(&mut store);
        let mut a = sample_fact("f-a", "gt");
        a.conflicts_with = vec!["f-b".to_string()];
        let b = FactImport {
            claim: "a contradicting account".to_string(),
            ..sample_fact("f-b", "gt")
        };
        let manifest = FactsManifest {
            disclosure_plans: vec![],
            frames: vec![FrameImport {
                frame_id: "gt".to_string(),
                description: String::new(),
            }],
            branches: vec![],
            entities: vec![],
            predicates: vec![],
            facts: vec![a, b],
        };
        import_facts(&mut store, &path, &manifest).unwrap();
        let again = import_facts(&mut store, &path, &manifest).unwrap();
        assert_eq!(again.written_bytes, 0, "{}", again.target_id);
        assert!(again.target_id.contains("3 no-op"), "{}", again.target_id);
    }

    /// Round 440 — stale-pin nuance: re-adding content-identical facts
    /// after the conflict TARGET was amended rejects with the
    /// re-affirmation message, not the generic divergent one.
    #[test]
    fn stamp_only_divergence_names_the_reaffirmation_path() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("s.json");
        let mut store = AtomicStore::new();
        seed_chapters(&mut store);
        store.frames.insert("gt".to_string(), Frame::default());
        let target = sample_fact("f-target", "gt");
        let mut owner = FactImport {
            conflicts_with: vec!["f-target".to_string()],
            ..sample_fact("f-owner", "gt")
        };
        owner.claim = "the contradicting account".to_string();
        add_fact(&mut store, &path, &target).unwrap();
        add_fact(&mut store, &path, &owner).unwrap();
        amend_fact(
            &mut store,
            &path,
            &FactImport {
                claim: "a revised target claim".to_string(),
                ..sample_fact("f-target", "gt")
            },
            "revision",
        )
        .unwrap();
        let err = add_fact(&mut store, &path, &owner).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("STALE judgment pins"), "{msg}");
        assert!(msg.contains("amend_fact"), "{msg}");
    }

    /// Round 437 — entity registry: refs fail loud until registered, dup
    /// refs reject, empty `entities` never serializes (byte-stability), and
    /// the registry row round-trips.
    #[test]
    fn add_entity_registry_gates_fact_refs() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("s.json");
        let mut store = AtomicStore::new();
        seed_chapters(&mut store);
        store.frames.insert("gt".to_string(), Frame::default());
        let about_count = FactImport {
            entities: vec!["dracula".to_string()],
            ..sample_fact("f-about", "gt")
        };
        let err = add_fact(&mut store, &path, &about_count).unwrap_err();
        assert!(err.to_string().contains("entity registry"), "{err}");
        add_entity(&mut store, &path, "dracula", "character", "the count").unwrap();
        add_fact(&mut store, &path, &about_count).unwrap();
        assert_eq!(
            store.narrative_facts["f-about"].entities,
            vec!["dracula".to_string()]
        );
        // Duplicate refs reject.
        let dup = FactImport {
            entities: vec!["dracula".to_string(), "dracula".to_string()],
            ..sample_fact("f-dup", "gt")
        };
        let err = add_fact(&mut store, &path, &dup).unwrap_err();
        assert!(err.to_string().contains("duplicate entity"), "{err}");
        // Entity-less fact serializes no `entities` key (byte-stability).
        add_fact(&mut store, &path, &sample_fact("f-plain", "gt")).unwrap();
        let raw = std::fs::read_to_string(&path).unwrap();
        let json: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(json["narrative_facts"]["f-plain"].get("entities").is_none());
        let reloaded = AtomicStore::load(&path).unwrap();
        assert_eq!(reloaded.entities["dracula"].kind, "character");
        // Divergent re-register rejects; identical = no-op.
        let again = add_entity(&mut store, &path, "dracula", "character", "the count").unwrap();
        assert_eq!(again.written_bytes, 0);
        let err = add_entity(&mut store, &path, "dracula", "location", "").unwrap_err();
        assert!(err.to_string().contains("DIVERGENT"), "{err}");
    }

    #[test]
    fn add_fact_conflict_appends_edge_and_rejects_duplicates() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed_chapters(&mut store);
        store.frames.insert("gt".to_string(), Frame::default());
        store.frames.insert("seward".to_string(), Frame::default());
        add_fact(&mut store, &path, &sample_fact("f1", "gt")).unwrap();
        add_fact(&mut store, &path, &sample_fact("f2", "seward")).unwrap();
        // Unknown target fail-loud.
        let err = add_fact_conflict(&mut store, &path, "f1", "f9").unwrap_err();
        assert!(matches!(err, AtomicMutateError::NotFound(_)), "{err}");
        // Self-conflict reject.
        let err = add_fact_conflict(&mut store, &path, "f1", "f1").unwrap_err();
        assert!(err.to_string().contains("itself"), "{err}");
        add_fact_conflict(&mut store, &path, "f1", "f2").unwrap();
        let reloaded = AtomicStore::load(&path).unwrap();
        let edges = &reloaded.narrative_facts["f1"].conflicts_with;
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].target, "f2");
        // Judgment-time content pin, computed by the primitive (R439).
        assert_eq!(
            edges[0].target_claim_sha256,
            sha256_hex(reloaded.narrative_facts["f2"].claim.as_bytes())
        );
        // Already recorded — in EITHER direction.
        let err = add_fact_conflict(&mut store, &path, "f1", "f2").unwrap_err();
        assert!(matches!(err, AtomicMutateError::FrozenLedger(_)), "{err}");
        let err = add_fact_conflict(&mut store, &path, "f2", "f1").unwrap_err();
        assert!(matches!(err, AtomicMutateError::FrozenLedger(_)), "{err}");
    }

    #[test]
    fn add_frame_idempotent_no_op_and_divergent_reject() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        add_frame(&mut store, &path, "gt", "the ground truth axis").unwrap();
        let again = add_frame(&mut store, &path, "gt", "the ground truth axis").unwrap();
        assert_eq!(again.written_bytes, 0);
        let err = add_frame(&mut store, &path, "gt", "another description").unwrap_err();
        assert!(err.to_string().contains("DIVERGENT"), "{err}");
    }

    #[test]
    fn legacy_store_without_narrative_maps_loads_empty() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("v11.json");
        std::fs::write(
            &path,
            r#"{"schema_version": 11, "sections": {}, "changelog_entries": {}}"#,
        )
        .unwrap();
        let store = AtomicStore::load(&path).unwrap();
        assert!(store.frames.is_empty());
        assert!(store.narrative_facts.is_empty());
        assert_eq!(store.schema_version, 11);
    }
}
