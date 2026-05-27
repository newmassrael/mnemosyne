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
//! `add_section_example` / `add_section_implementation`
//! - ChangelogEntry atomic: `append_changelog_entry`
//!
//! `Section.implementations` lands as the substrate for Path B
//! of the code-citation defense (Spec ↔ Code bidirectional binding). The
//! atomic store records "this section is implemented at file:symbol";
//! cross-checks code citations against the spec's authoritative
//! binding (set-equality, the OPTION D pattern lifted from cross-
//! ref orphan reject). Schema + mutate primitive only — validator
//! extension and section seeding are deferred to later rounds.

use crate::schema::{DecisionStatus, Section};
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
 /// Heading title. Mirrors schema.rs::Section.title. Default = "" during
 /// Round 287 transitional state (pre-Phase-I backfill).
 #[serde(default, skip_serializing_if = "String::is_empty")]
 pub title: String,
 /// Owning doc identifier (workspace-relative path or doc-id). Mirrors
 /// schema.rs::Section.parent_doc. Default = "" during Round 287
 /// transitional state. Replaces the legacy `ATOMIC_ONLY_PARENT_DOC`
 /// sentinel surfaced via query.rs synthetic_section construction.
 #[serde(default, skip_serializing_if = "String::is_empty")]
 pub parent_doc: String,
 /// Nullable parent section_id. `None` = top-level section in its doc.
 /// Mirrors schema.rs::Section.parent_section.
 #[serde(default, skip_serializing_if = "Option::is_none")]
 pub parent_section: Option<String>,
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
 /// cross-ref list (target section_id without `§` prefix).
 #[serde(default, skip_serializing_if = "Vec::is_empty")]
 pub impact_scope: Vec<String>,
 /// code/config block list. T3 style threshold: code block itself exempt.
 #[serde(default, skip_serializing_if = "Vec::is_empty")]
 pub examples: Vec<ExampleBlock>,
 /// Path B (Spec ↔ Code bidirectional binding) substrate.
 /// Set of `(file, symbol?)` bindings that authoritatively own "this
 /// section is implemented here". cross-checks code citations
 /// against this set. Append-only (no replace/remove primitive); duplicate
 /// `(file, symbol)` rejected at write time (set semantics).
 #[serde(default, skip_serializing_if = "Vec::is_empty")]
 pub implementations: Vec<Implementation>,
 /// Round 265 — atomic decision_status override.
 ///
 /// `None` = no atomic override; consumers fall back to the parser-derived
 /// status (currently hard-coded to `Active` workspace-wide). `Some(_)` =
 /// the atomic store authoritatively declares the section's status,
 /// overriding the parser default. Wired through `query::build_section_view`
 /// so SectionView reports the atomic value when present.
 ///
 /// Unblocks Stage B freshness — once a section transitions to
 /// `Superseded` here, downstream tooling (auto-cascade trigger, decay scan)
 /// can react. Trigger wiring itself is a later round.
 #[serde(default, skip_serializing_if = "Option::is_none")]
 pub decision_status: Option<DecisionStatus>,

 /// External-spec mirror — vendored normative quote anchored to this
 /// Section (RFC-002 FR-1). When `Some`, the Section represents a
 /// section of an external standard (W3C / IETF RFC / IEEE / AUTOSAR /
 /// …) mirrored into this workspace; the embedded text + anchor URL +
 /// source revision pin let reviewers verify code citations against
 /// the exact spec text the workspace was built against.
 ///
 /// **Frozen-ledger zone**: once anchored (None → Some), the value is
 /// immutable. The mutate primitive
 /// [`set_section_normative_excerpt`] rejects Some → Some transitions.
 /// Spec revision drift is modeled as `Section.decision_status =
 /// Superseded` + a new Section carrying the updated excerpt — same
 /// pattern as R294 audit-half immutability and R265 decision_status
 /// supersession.
 ///
 /// `None` (default) on every Section that does not mirror an external
 /// standard. The `[workspace.spec_source]` config (FR-2) names the
 /// upstream the entire workspace tracks; per-Section `source_revision`
 /// is the rev that was current when this Section's excerpt was
 /// anchored, so partially-migrated workspaces can carry old + new
 /// rev side-by-side via `Active` + `Superseded` Sections.
 #[serde(default, skip_serializing_if = "Option::is_none")]
 pub normative_excerpt: Option<NormativeExcerpt>,
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RejectedAlternative {
 pub alternative: String,
 pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExampleBlock {
 /// Language tag for fenced code block (`rust` / `toml` / `markdown` / etc).
 pub language: String,
 pub code: String,
}

/// Path B binding entry (Spec → Code).
///
/// `file` = workspace-relative POSIX path (no leading `/`, no `..` segment,
/// no backslash; validated at write time by [`add_section_implementation`]).
/// `symbol` = optional opaque language-agnostic identifier (function /
/// type / qualified path); when present, narrows the binding from "this
/// file" to "this symbol within this file". Stored opaquely — the spec
/// layer does not encode language grammar; 's bidirectional
/// cross-check operates on the strings as-is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Implementation {
 pub file: String,
 #[serde(default, skip_serializing_if = "Option::is_none")]
 pub symbol: Option<String>,
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
///   `generate_docs` renders publishable_*. CQRS / read-write split pattern.
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
 use sha2::{Digest, Sha256};
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
 let mut hasher = Sha256::new();
 hasher.update(serde_json::to_vec(&payload).unwrap_or_default());
 let bytes = hasher.finalize();
 let mut s = String::with_capacity(bytes.len() * 2);
 for b in bytes {
 use std::fmt::Write;
 let _ = write!(&mut s, "{:02x}", b);
 }
 s
 }

 /// Round 296 — SHA256 of the audit half, hex-encoded. Optional
 /// content_hash_before anchor in the ledger; informational since the
 /// audit half is immutable post-append.
 pub fn audit_hash_hex(&self) -> String {
 use sha2::{Digest, Sha256};
 let payload = serde_json::json!({
 "decision_summary": self.decision_summary,
 "changes_bullets": self.changes_bullets,
 "verification_bullets": self.verification_bullets,
 "impact_refs": self.impact_refs,
 "carry_forward_bullets": self.carry_forward_bullets,
 });
 let mut hasher = Sha256::new();
 hasher.update(serde_json::to_vec(&payload).unwrap_or_default());
 let bytes = hasher.finalize();
 let mut s = String::with_capacity(bytes.len() * 2);
 for b in bytes {
 use std::fmt::Write;
 let _ = write!(&mut s, "{:02x}", b);
 }
 s
 }
}

/// Inventory entry lifecycle status (Round 273, Phase 1A).
///
/// Distinguished from `DecisionStatus` (audit-trail genre — `Active` /
/// `Superseded` / `Removed` over Section / ChangelogEntry decisions) because
/// inventory entries belong to a different genre: stable external IDs (test
/// cases, requirement IDs, regulation IDs) whose lifecycle vocabulary is
/// "in use" / "no longer cited" / "set aside but reserved". Cite-time
/// reject semantics in later rounds key off `Deprecated`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InventoryStatus {
 Active,
 Deprecated,
 Reserved,
}

impl Default for InventoryStatus {
 fn default() -> Self {
 InventoryStatus::Active
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
 /// Schema version — bump on breaking shape change.
 #[serde(default = "default_schema_version")]
 pub schema_version: u32,
}

fn default_schema_version() -> u32 {
 1
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
// Load is back-compat across all versions ≤ CURRENT: version-N stores
// deserialize with newer fields defaulted; the next save rewrites
// schema_version to CURRENT.
const CURRENT_SCHEMA_VERSION: u32 = 4;
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

 /// Round 287 — fail-loud Section lookup.
 ///
 /// Returns `None` if `section_id` is absent. Replaces the pre-287
 /// silent-create variant (`entry(...).or_default()`) which let any
 /// caller materialize an outline-less Section by typo. Creation now
 /// routes exclusively through [`add_section`].
 pub fn section_mut(&mut self, section_id: &str) -> Option<&mut AtomicSection> {
 self.sections.get_mut(section_id)
 }

 /// Get / create-default changelog atomic entry.
 ///
 /// ChangelogEntry creation path differs from Section: `append_changelog_entry`
 /// is the explicit primitive, and this getter remains create-on-miss until
 /// a parallel fail-loud refactor (out of scope for Round 287's Section axis).
 pub fn entry_mut(&mut self, entry_id: &str) -> &mut AtomicChangelogEntry {
 self.changelog_entries
 .entry(entry_id.to_string())
 .or_default()
 }

 pub fn section(&self, section_id: &str) -> Option<&AtomicSection> {
 self.sections.get(section_id)
 }

 pub fn entry(&self, entry_id: &str) -> Option<&AtomicChangelogEntry> {
 self.changelog_entries.get(entry_id)
 }

 /// Resolve a markdown-derived [`Section`] to its atomic counterpart.
 ///
 /// Why this is not a straight `section(&section.section_id)` call: the
 /// parser's `section_id` is a markdown-tree disambiguator and may carry
 /// a parent prefix (`{doc-slug}/sections/{atomic-id}`) for nested
 /// headings, while the atomic store is keyed by the bare atomic id the
 /// renderer wrote. The `Section.atomic_section_id` field carries that
 /// bare id verbatim from the heading's `§<token>` slot — try it first.
 ///
 /// Fallback path (`section_id` directly) preserves compatibility with
 /// pre-decompose markdown and any future external doc whose headings
 /// already happen to be bare atomic ids.
 pub fn resolve(&self, section: &Section) -> Option<&AtomicSection> {
 if let Some(aid) = section.atomic_section_id.as_deref() {
 if let Some(found) = self.sections.get(aid) {
 return Some(found);
 }
 }
 self.sections.get(&section.section_id)
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
 if include_implementations && !atomic.implementations.is_empty() {
 let block: Vec<String> = atomic
 .implementations
 .iter()
 .map(|i| match &i.symbol {
 Some(s) => format!("- {}:{}", i.file, s),
 None => format!("- {}", i.file),
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
fn check_changelog_entry_v2_required(
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
 let written = fs::metadata(sidecar_path).map(|m| m.len() as usize).unwrap_or(0);
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
/// Returns `NotFound` when `section_id` is absent. Closes the `section_mut()`
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
 save_with_receipt(store, sidecar_path, "set_section_intent", "section", section_id)
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
 section_mut_strict(store, section_id)?.examples.push(example);
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
 save_with_receipt(
 store,
 sidecar_path,
 "remove_section",
 "section",
 section_id,
 )
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
///   silent overwrite; the `section_mut().or_default()` silent-create
///   footgun is closed in a follow-on phase)
/// - `parent_section`, when `Some(_)`, must be non-empty and exist in store
///   (referential integrity at write time)
///
/// `decision_status` is initialized to `Some(Active)` — newly created sections
/// are *explicitly* Active, distinct from Round 269's `None = parser default`
/// case which only applies to pre-251 carry sections. Subsequent transitions
/// route through `set_section_decision_status`.
pub fn add_section(
 store: &mut AtomicStore,
 sidecar_path: &Path,
 section_id: &str,
 parent_doc: &str,
 title: &str,
 parent_section: Option<&str>,
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
 let section_id_t = section_id.trim();
 let parent_doc_t = parent_doc.trim();
 let title_t = title.trim();

 if section_id_t.is_empty() {
 return Err(AtomicMutateError::Validation(
 "add_section: section_id mandatory (non-empty after trim)".to_string(),
 ));
 }
 if parent_doc_t.is_empty() {
 return Err(AtomicMutateError::Validation(
 "add_section: parent_doc mandatory (non-empty after trim)".to_string(),
 ));
 }
 if title_t.is_empty() {
 return Err(AtomicMutateError::Validation(
 "add_section: title mandatory (non-empty after trim)".to_string(),
 ));
 }
 if store.sections.contains_key(section_id_t) {
 return Err(AtomicMutateError::Validation(format!(
 "add_section: section_id `{}` already exists in atomic store (use set_section_* primitives to mutate)",
 section_id_t
 )));
 }
 let parent_section_norm = if let Some(parent) = parent_section {
 let parent_t = parent.trim();
 if parent_t.is_empty() {
 return Err(AtomicMutateError::Validation(
  "add_section: parent_section must be None or non-empty".to_string(),
 ));
 }
 if !store.sections.contains_key(parent_t) {
 return Err(AtomicMutateError::NotFound(format!(
  "add_section: parent_section `{}` not present in atomic store",
  parent_t
 )));
 }
 Some(parent_t.to_string())
 } else {
 None
 };

 let section = AtomicSection {
 title: title_t.to_string(),
 parent_doc: parent_doc_t.to_string(),
 parent_section: parent_section_norm,
 decision_status: Some(DecisionStatus::Active),
 ..Default::default()
 };
 store.sections.insert(section_id_t.to_string(), section);

 save_with_receipt(
 store,
 sidecar_path,
 "add_section",
 "section",
 section_id_t,
 )
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
 section_mut_strict(store, section_id)?.title = title_t.to_string();
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
 section_mut_strict(store, section_id)?.parent_doc = pd_t.to_string();
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
  "set_section_parent_section: parent_section must be None or non-empty".to_string(),
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
 section_mut_strict(store, section_id)?.parent_section = parent_norm;
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
/// This guard does not validate that the named superseding section_id
/// exists in the atomic store — cross-ref orphan checking is T1 rule 1's
/// territory, picked up by `validate-workspace`. This primitive only
/// enforces presence of the *intent to forward*; existence is checked
/// by the validator pass.
pub fn set_section_decision_status(
 store: &mut AtomicStore,
 sidecar_path: &Path,
 section_id: &str,
 new_status: DecisionStatus,
 superseding: Option<&str>,
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
 if new_status == DecisionStatus::Superseded && superseding.is_none() {
 return Err(AtomicMutateError::Validation(
 "(T1 rule 4, atomic axis): superseding section_id mandatory for active → superseded transition".to_string(),
 ));
 }
 section_mut_strict(store, section_id)?.decision_status = Some(new_status);
 save_with_receipt(
 store,
 sidecar_path,
 "set_section_decision_status",
 "section",
 section_id,
 )
}

/// External-spec mirror — anchor a vendored normative excerpt onto a
/// Section (RFC-002 FR-1). **Append-only / frozen-ledger semantic**:
/// the primitive accepts `None → Some` but refuses `Some → Some`. To
/// model spec revision drift, the caller transitions the existing
/// Section to `decision_status = Superseded` and creates a new
/// Section carrying the updated excerpt — same pattern as audit-half
/// immutability in `append_changelog_entry`.
///
/// Validates:
/// - `text` non-empty (trimmed).
/// - `anchor_url` parses as absolute URL (scheme `http`/`https` + host).
/// - `source_revision` non-empty (trimmed).
/// - Target Section exists.
/// - Target Section's `normative_excerpt` is currently `None` (frozen
///   reject otherwise).
pub fn set_section_normative_excerpt(
 store: &mut AtomicStore,
 sidecar_path: &Path,
 section_id: &str,
 text: &str,
 anchor_url: &str,
 source_revision: &str,
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
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
 let host_end = rest.find(|c: char| c == '/' || c == '?' || c == '#')
 .unwrap_or(rest.len());
 !rest[..host_end].is_empty()
 })
 .unwrap_or(false);
 if !is_url {
 return Err(AtomicMutateError::Validation(format!(
 "normative_excerpt anchor_url `{}` must be an absolute http(s):// URL with a host",
 anchor_url
 )));
 }
 let section = section_mut_strict(store, section_id)?;
 if section.normative_excerpt.is_some() {
 return Err(AtomicMutateError::FrozenLedger(format!(
 "normative_excerpt already anchored on §{} — once set, the field is immutable; \
  model spec rev drift by superseding this Section and creating a new one with the updated excerpt",
 section_id
 )));
 }
 section.normative_excerpt = Some(NormativeExcerpt {
 text: text.trim_end_matches('\n').to_string(),
 anchor_url: anchor_url.to_string(),
 source_revision: source_revision.to_string(),
 });
 save_with_receipt(
 store,
 sidecar_path,
 "set_section_normative_excerpt",
 "section",
 section_id,
 )
}

pub fn add_section_implementation(
 store: &mut AtomicStore,
 sidecar_path: &Path,
 section_id: &str,
 file: &str,
 symbol: Option<&str>,
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
 let file_clean = validate_implementation_file(file)?;
 let symbol_clean = match symbol {
 Some(s) => Some(validate_implementation_symbol(s)?),
 None => None,
 };
 let candidate = Implementation {
 file: file_clean,
 symbol: symbol_clean,
 };
 let section = section_mut_strict(store, section_id)?;
 if section.implementations.contains(&candidate) {
 return Err(AtomicMutateError::Validation(format!(
 "implementation `{}{}` already present on §{} (set semantics — duplicates rejected at write time)",
 candidate.file,
 candidate
 .symbol
 .as_deref()
 .map(|s| format!(":{}", s))
 .unwrap_or_default(),
 section_id,
 )));
 }
 section.implementations.push(candidate);
 save_with_receipt(
 store,
 sidecar_path,
 "add_section_implementation",
 "section",
 section_id,
 )
}

/// Round 283 — remove one (file, symbol?) implementation binding from a
/// Section.
///
/// Section.implementations carries current-truth semantics (R259
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
/// input validation (mirrors `add_section_implementation`).
/// - `NotFound`: `section_id` absent, or the `(file, symbol)` tuple
/// is not registered on the section (no silent no-op — caller asked
/// to remove a specific binding).
///
/// Symmetric with [`add_section_implementation`]; `--reason` mandatory
/// mirrors [`remove_section`] (Round 267) and `remove_inventory_entry`
/// (Round 274).
pub fn remove_section_implementation(
 store: &mut AtomicStore,
 sidecar_path: &Path,
 section_id: &str,
 file: &str,
 symbol: Option<&str>,
 reason: &str,
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
 if reason.trim().is_empty() {
 return Err(AtomicMutateError::Validation(
 "remove_section_implementation: --reason mandatory (audit-trail safeguard)".to_string(),
 ));
 }
 let file_clean = validate_implementation_file(file)?;
 let symbol_clean = match symbol {
 Some(s) => Some(validate_implementation_symbol(s)?),
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
 let target = Implementation {
 file: file_clean,
 symbol: symbol_clean,
 };
 let pos = match section.implementations.iter().position(|i| i == &target) {
 Some(p) => p,
 None => {
 return Err(AtomicMutateError::NotFound(format!(
 "implementation `{}{}` not registered on §{}",
 target.file,
 target
 .symbol
 .as_deref()
 .map(|s| format!(":{}", s))
 .unwrap_or_default(),
 section_id
 )));
 }
 };
 section.implementations.remove(pos);
 save_with_receipt(
 store,
 sidecar_path,
 "remove_section_implementation",
 "section",
 section_id,
 )
}

fn validate_implementation_file(raw: &str) -> Result<String, AtomicMutateError> {
 let trimmed = raw.trim();
 if trimmed.is_empty() {
 return Err(AtomicMutateError::Validation(
 "implementation file: must be non-empty".to_string(),
 ));
 }
 if trimmed != raw {
 return Err(AtomicMutateError::Validation(format!(
 "implementation file: leading or trailing whitespace not allowed (`{}`)",
 raw
 )));
 }
 if trimmed.starts_with('/') {
 return Err(AtomicMutateError::Validation(format!(
 "implementation file: must be workspace-relative (no leading `/`): `{}`",
 trimmed
 )));
 }
 if trimmed.starts_with("./") {
 return Err(AtomicMutateError::Validation(format!(
 "implementation file: drop leading `./` for canonical form (`{}`)",
 trimmed
 )));
 }
 if trimmed.contains('\\') {
 return Err(AtomicMutateError::Validation(format!(
 "implementation file: backslash not allowed (workspace paths are POSIX): `{}`",
 trimmed
 )));
 }
 if trimmed.contains("//") {
 return Err(AtomicMutateError::Validation(format!(
 "implementation file: collapse internal `//` (`{}`)",
 trimmed
 )));
 }
 if trimmed.ends_with('/') {
 return Err(AtomicMutateError::Validation(format!(
 "implementation file: trailing `/` not allowed (must point at a file, not a dir): `{}`",
 trimmed
 )));
 }
 for seg in trimmed.split('/') {
 if seg == ".." {
 return Err(AtomicMutateError::Validation(format!(
 "implementation file: `..` segment not allowed (no traversal in normalized paths): `{}`",
 trimmed
 )));
 }
 }
 Ok(trimmed.to_string())
}

fn validate_implementation_symbol(raw: &str) -> Result<String, AtomicMutateError> {
 let trimmed = raw.trim();
 if trimmed.is_empty() {
 return Err(AtomicMutateError::Validation(
 "implementation symbol: must be non-empty when supplied (omit the field for file-level binding)".to_string(),
 ));
 }
 if trimmed != raw {
 return Err(AtomicMutateError::Validation(format!(
 "implementation symbol: leading or trailing whitespace not allowed (`{}`)",
 raw
 )));
 }
 if trimmed.contains('\n') || trimmed.contains('\r') {
 return Err(AtomicMutateError::Validation(format!(
 "implementation symbol: newline not allowed (`{:?}`)",
 raw
 )));
 }
 Ok(trimmed.to_string())
}

// ============================================================================
// ChangelogEntry atomic mutate primitive.
// ============================================================================

/// `append_changelog_entry` primitive — atomic-aware changelog append.
///
/// Frozen ledger semantics: once committed,
/// existing fields cannot be modified or removed (T2 jaccard); subsequent
/// mutations to the same `entry_id` are rejected via FrozenLedger error.
pub fn append_changelog_entry(
 store: &mut AtomicStore,
 sidecar_path: &Path,
 entry_id: &str,
 decision_summary: Option<&str>,
 changes_bullets: &[String],
 verification_bullets: &[String],
 impact_refs: &[String],
 carry_forward_bullets: &[String],
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
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
 // Round 298 — required-field gate at the primitive boundary so CLI / MCC
 // / future wires share the same enforcement surface. Frozen-ledger reject
 // wins over field validation (existing FrozenLedger test passes empty
 // body intentionally).
 check_changelog_entry_v2_required(
 decision_summary,
 changes_bullets,
 verification_bullets,
 impact_refs,
 carry_forward_bullets,
 )?;
 // Round 294 — initialize publishable_* = audit_* clone. The two halves
 // diverge later via R295 publishable setters (paired with the R296
 // [[publishable_override_ledger]] gate). Default-equal at append time so
 // generate_docs render shape is byte-identical to pre-R294.
 let mut entry = AtomicChangelogEntry {
 decision_summary: decision_summary.map(str::to_string),
 changes_bullets: changes_bullets.to_vec(),
 verification_bullets: verification_bullets.to_vec(),
 impact_refs: impact_refs.to_vec(),
 carry_forward_bullets: carry_forward_bullets.to_vec(),
 ..Default::default()
 };
 entry.clone_audit_into_publishable();
 store
 .changelog_entries
 .insert(entry_id.to_string(), entry);
 save_with_receipt(
 store,
 sidecar_path,
 "append_changelog_entry",
 "changelog_entry",
 entry_id,
 )
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
// `entry_mut_strict` is the strict variant of `AtomicStore::entry_mut`
// (which create-on-miss for back-compat with the v1 path); the publishable
// setters require the entry to exist first because they cannot author the
// audit half.
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
 entry_mut_strict(store, entry_id)?.publishable_decision_summary =
 Some(summary.to_string());
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
 kind,
 entry_id,
 &fields,
 reason,
 applied_in,
 &before,
 &after,
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

#[cfg(test)]
mod tests {
 use super::*;
 use tempfile::TempDir;

 /// Round 287 — test fixture helper. Direct sections.insert (bypasses
 /// audit-receipt path) to seed a Section so content-axis primitives can
 /// be exercised. Production code routes Section creation through
 /// `add_section`; tests use this helper to keep setup boilerplate down.
 fn seed_section(store: &mut AtomicStore, section_id: &str) {
 store
 .sections
 .insert(section_id.to_string(), AtomicSection::default());
 }

 #[test]
 fn save_load_round_trip() {
 let tmp = TempDir::new().unwrap();
 let path = tmp.path().join(".atomic/workspace.atomic.json");
 let mut store = AtomicStore::new();
 seed_section(&mut store, "43");
 set_section_intent(&mut store, &path, "43", "test intent").unwrap();
 let loaded = AtomicStore::load(&path).unwrap();
 assert_eq!(loaded.section("43").unwrap().intent.as_deref(), Some("test intent"));
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
 fn changelog_entry_v2_frozen_after_append() {
 let tmp = TempDir::new().unwrap();
 let path = tmp.path().join(".atomic/workspace.atomic.json");
 let mut store = AtomicStore::new();
 append_changelog_entry(
 &mut store,
 &path,
 "Round 162",
 Some("test summary"),
 &["change 1".into()],
 &["verify 1".into()],
 &["43".into()],
 &["carry 1".into()],
 )
 .unwrap();
 let err = append_changelog_entry(
 &mut store,
 &path,
 "Round 162",
 Some("attempted overwrite"),
 &[],
 &[],
 &[],
 &[],
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
 assert_eq!(reloaded.schema_version, 4);
 }

 #[test]
 fn schema_version_2_store_loads_with_empty_outline_fields() {
 // Round 287 back-compat: a v2 store (pre-outline-lift) deserializes
 // with empty AtomicSection.title / .parent_doc + parent_section = None.
 // Next save rewrites to v3.
 let tmp = TempDir::new().unwrap();
 let path = tmp.path().join(".atomic/workspace.atomic.json");
 std::fs::create_dir_all(path.parent().unwrap()).unwrap();
 let legacy_v2_json = r#"{
 "sections": {
 "39": {
 "intent": "old-shape section without outline"
 }
 },
 "changelog_entries": {},
 "inventory_entries": {},
 "schema_version": 2
 }"#;
 std::fs::write(&path, legacy_v2_json).unwrap();
 let loaded = AtomicStore::load(&path).unwrap();
 let s = loaded.sections.get("39").expect("§39 present");
 assert_eq!(s.title, "", "title defaults to empty pre-backfill");
 assert_eq!(s.parent_doc, "", "parent_doc defaults to empty pre-backfill");
 assert_eq!(s.parent_section, None, "parent_section defaults to None");
 assert_eq!(s.intent.as_deref(), Some("old-shape section without outline"));
 loaded.save(&path).unwrap();
 let reloaded = AtomicStore::load(&path).unwrap();
 assert_eq!(reloaded.schema_version, 4);
 }

 #[test]
 fn schema_version_3_clones_audit_into_publishable_on_load() {
 // Round 294 v3→v4 migration: a v3 store has audit_* fields populated
 // but no publishable_* fields. Loading must clone audit_* into
 // publishable_* per entry so the render shape stays byte-identical.
 let tmp = TempDir::new().unwrap();
 let path = tmp.path().join(".atomic/workspace.atomic.json");
 std::fs::create_dir_all(path.parent().unwrap()).unwrap();
 let legacy_v3_json = r#"{
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
 std::fs::write(&path, legacy_v3_json).unwrap();
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
 // Save then reload: publishable_* now persisted, schema bumps to 4.
 loaded.save(&path).unwrap();
 let reloaded = AtomicStore::load(&path).unwrap();
 assert_eq!(reloaded.schema_version, 4);
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
 "Round 999",
 Some("appended summary"),
 &["appended change".into()],
 &["appended verify".into()],
 &["43".into()],
 &["appended carry".into()],
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
 append_changelog_entry(store, path, entry_id, None, &[], &[], &[], &[])
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
 "Round 999",
 Some("decision"),
 &[],
 &["verify".into()],
 &[],
 &[],
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
 "Round 999",
 Some("decision"),
 &["change".into()],
 &[],
 &[],
 &[],
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
 "Round 999",
 Some("decision"),
 &["valid".into(), "   ".into()],
 &["verify".into()],
 &[],
 &[],
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
 "Round 999",
 Some("decision"),
 &["change".into()],
 &["verify".into()],
 &["".into()],
 &[],
 )
 .unwrap_err();
 match err {
 AtomicMutateError::Validation(msg) => {
  assert!(msg.contains("impact_refs[0]"), "msg={}", msg);
 }
 other => panic!("expected Validation, got {:?}", other),
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
 add_inventory_entry(&mut store, &path, "ARP_07", InventoryStatus::Active, None, None, None).unwrap();
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
 add_inventory_entry(&mut store, &path, "", InventoryStatus::Active, None, None, None),
 Err(AtomicMutateError::Validation(_))
 ));
 // whitespace edges
 assert!(matches!(
 add_inventory_entry(&mut store, &path, " ARP_07", InventoryStatus::Active, None, None, None),
 Err(AtomicMutateError::Validation(_))
 ));
 // internal whitespace
 assert!(matches!(
 add_inventory_entry(&mut store, &path, "ARP 07", InventoryStatus::Active, None, None, None),
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
 AtomicMutateError::Validation(msg) => assert!(msg.contains("drop leading `§`"), "got: {}", msg),
 other => panic!("expected Validation, got {:?}", other),
 }
 }

 #[test]
 fn set_inventory_status_active_to_deprecated_with_reason() {
 let tmp = TempDir::new().unwrap();
 let path = tmp.path().join(".atomic/workspace.atomic.json");
 let mut store = AtomicStore::new();
 add_inventory_entry(&mut store, &path, "TCP_X", InventoryStatus::Active, None, None, None).unwrap();
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
 set_inventory_status(&mut store, &path, "TCP_X", InventoryStatus::Reserved, Some("")).unwrap();
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
 add_inventory_entry(&mut store, &path, "ARP_07", InventoryStatus::Active, None, None, None).unwrap();
 set_inventory_section_ref(&mut store, &path, "ARP_07", Some("4.2.4")).unwrap();
 assert_eq!(
 AtomicStore::load(&path).unwrap().inventory("ARP_07").unwrap().section_ref.as_deref(),
 Some("4.2.4")
 );
 set_inventory_section_ref(&mut store, &path, "ARP_07", None).unwrap();
 assert!(AtomicStore::load(&path).unwrap().inventory("ARP_07").unwrap().section_ref.is_none());
 }

 #[test]
 fn set_inventory_section_ref_not_found_returns_not_found() {
 let tmp = TempDir::new().unwrap();
 let path = tmp.path().join(".atomic/workspace.atomic.json");
 let mut store = AtomicStore::new();
 let err = set_inventory_section_ref(&mut store, &path, "ARP_99", Some("4.2.4")).unwrap_err();
 assert!(matches!(err, AtomicMutateError::NotFound(_)));
 }

 #[test]
 fn remove_inventory_entry_basic() {
 let tmp = TempDir::new().unwrap();
 let path = tmp.path().join(".atomic/workspace.atomic.json");
 let mut store = AtomicStore::new();
 add_inventory_entry(&mut store, &path, "ARP_07", InventoryStatus::Active, None, None, None).unwrap();
 remove_inventory_entry(&mut store, &path, "ARP_07", "deprecated upstream in TC8 v2.4").unwrap();
 let loaded = AtomicStore::load(&path).unwrap();
 assert!(loaded.inventory("ARP_07").is_none());
 }

 #[test]
 fn remove_inventory_entry_rejects_empty_reason() {
 let tmp = TempDir::new().unwrap();
 let path = tmp.path().join(".atomic/workspace.atomic.json");
 let mut store = AtomicStore::new();
 add_inventory_entry(&mut store, &path, "ARP_07", InventoryStatus::Active, None, None, None).unwrap();
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
 store
 .inventory_entries
 .insert("TCP_FLAGS_INVALID_02".to_string(), InventoryEntry::default());
 store
 .inventory_entries
 .insert("SOMEIP_ETS_BASICS_01".to_string(), InventoryEntry::default());
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
 fn add_section_implementation_basic_round_trip() {
 let tmp = TempDir::new().unwrap();
 let path = tmp.path().join(".atomic/workspace.atomic.json");
 let mut store = AtomicStore::new();
 seed_section(&mut store, "39");
 add_section_implementation(
 &mut store,
 &path,
 "39",
 "crates/mnemosyne-validator/src/atomic.rs",
 Some("AtomicSection"),
 )
 .unwrap();
 add_section_implementation(
 &mut store,
 &path,
 "39",
 "crates/mnemosyne-cli/src/atomic_cli.rs",
 None,
 )
 .unwrap();
 let loaded = AtomicStore::load(&path).unwrap();
 let impls = &loaded.section("39").unwrap().implementations;
 assert_eq!(impls.len(), 2);
 assert_eq!(impls[0].file, "crates/mnemosyne-validator/src/atomic.rs");
 assert_eq!(impls[0].symbol.as_deref(), Some("AtomicSection"));
 assert_eq!(impls[1].file, "crates/mnemosyne-cli/src/atomic_cli.rs");
 assert!(impls[1].symbol.is_none());
 }

 #[test]
 fn add_section_implementation_rejects_duplicate() {
 let tmp = TempDir::new().unwrap();
 let path = tmp.path().join(".atomic/workspace.atomic.json");
 let mut store = AtomicStore::new();
 seed_section(&mut store, "39");
 add_section_implementation(&mut store, &path, "39", "src/foo.rs", Some("bar")).unwrap();
 let err = add_section_implementation(&mut store, &path, "39", "src/foo.rs", Some("bar"))
 .unwrap_err();
 match err {
 AtomicMutateError::Validation(msg) => assert!(msg.contains("already present")),
 other => panic!("expected Validation, got {:?}", other),
 }
 // file-only vs symbol-qualified are distinct entries.
 add_section_implementation(&mut store, &path, "39", "src/foo.rs", None).unwrap();
 assert_eq!(store.section("39").unwrap().implementations.len(), 2);
 }

 #[test]
 fn add_section_implementation_rejects_malformed_file() {
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
 add_section_implementation(&mut store, &path, "39", bad, None).unwrap_err();
 assert!(
 matches!(err, AtomicMutateError::Validation(_)),
 "expected Validation for `{}`, got {:?}",
 bad,
 err
 );
 }
 }

 #[test]
 fn add_section_implementation_rejects_malformed_symbol() {
 let tmp = TempDir::new().unwrap();
 let path = tmp.path().join(".atomic/workspace.atomic.json");
 let mut store = AtomicStore::new();
 // empty / whitespace symbol.
 for bad in ["", "   ", " sym", "sym ", "sym\nname"] {
 let err =
 add_section_implementation(&mut store, &path, "39", "src/foo.rs", Some(bad))
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
 fn add_section_implementation_accepts_opaque_qualified_symbols() {
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
 add_section_implementation(&mut store, &path, "39", "src/foo.rs", Some(sym)).unwrap();
 }
 assert_eq!(store.section("39").unwrap().implementations.len(), 6);
 }

 // Round 283 — remove_section_implementation tests.

 #[test]
 fn remove_section_implementation_basic_round_trip() {
 let tmp = TempDir::new().unwrap();
 let path = tmp.path().join(".atomic/workspace.atomic.json");
 let mut store = AtomicStore::new();
 seed_section(&mut store, "X");
 add_section_implementation(&mut store, &path, "X", "src/foo.rs", None).unwrap();
 assert_eq!(store.section("X").unwrap().implementations.len(), 1);
 remove_section_implementation(&mut store, &path, "X", "src/foo.rs", None, "code moved")
 .unwrap();
 let loaded = AtomicStore::load(&path).unwrap();
 assert_eq!(loaded.section("X").unwrap().implementations.len(), 0);
 }

 #[test]
 fn remove_section_implementation_symbol_aware_match() {
 // (file, None) vs (file, Some("sym")) are distinct set elements;
 // removing the file-only row must NOT touch the symbol-narrowed row.
 let tmp = TempDir::new().unwrap();
 let path = tmp.path().join(".atomic/workspace.atomic.json");
 let mut store = AtomicStore::new();
 seed_section(&mut store, "X");
 add_section_implementation(&mut store, &path, "X", "src/foo.rs", None).unwrap();
 add_section_implementation(&mut store, &path, "X", "src/foo.rs", Some("fn_a")).unwrap();
 add_section_implementation(&mut store, &path, "X", "src/foo.rs", Some("fn_b")).unwrap();
 remove_section_implementation(&mut store, &path, "X", "src/foo.rs", None, "cleanup")
 .unwrap();
 let loaded = AtomicStore::load(&path).unwrap();
 let impls = &loaded.section("X").unwrap().implementations;
 assert_eq!(impls.len(), 2, "only the file-only row should be removed");
 assert!(impls.iter().any(|i| i.symbol.as_deref() == Some("fn_a")));
 assert!(impls.iter().any(|i| i.symbol.as_deref() == Some("fn_b")));
 }

 #[test]
 fn remove_section_implementation_section_not_found() {
 let tmp = TempDir::new().unwrap();
 let path = tmp.path().join(".atomic/workspace.atomic.json");
 let mut store = AtomicStore::new();
 let err = remove_section_implementation(&mut store, &path, "ghost", "src/foo.rs", None, "x")
 .unwrap_err();
 assert!(matches!(err, AtomicMutateError::NotFound(_)));
 }

 #[test]
 fn remove_section_implementation_impl_not_found() {
 // Section exists, but the (file, symbol) tuple does not — fail-loud
 // (no silent no-op).
 let tmp = TempDir::new().unwrap();
 let path = tmp.path().join(".atomic/workspace.atomic.json");
 let mut store = AtomicStore::new();
 seed_section(&mut store, "X");
 add_section_implementation(&mut store, &path, "X", "src/foo.rs", None).unwrap();
 let err = remove_section_implementation(
 &mut store,
 &path,
 "X",
 "src/other.rs",
 None,
 "wrong file",
 )
 .unwrap_err();
 assert!(matches!(err, AtomicMutateError::NotFound(_)));
 }

 #[test]
 fn remove_section_implementation_rejects_empty_reason() {
 let tmp = TempDir::new().unwrap();
 let path = tmp.path().join(".atomic/workspace.atomic.json");
 let mut store = AtomicStore::new();
 seed_section(&mut store, "X");
 add_section_implementation(&mut store, &path, "X", "src/foo.rs", None).unwrap();
 let err = remove_section_implementation(&mut store, &path, "X", "src/foo.rs", None, "  ")
 .unwrap_err();
 assert!(matches!(err, AtomicMutateError::Validation(_)));
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
 let err =
 remove_section(&mut store, &path, "ghost", "no such section").unwrap_err();
 assert!(matches!(err, AtomicMutateError::NotFound(_)));
 }

 // Round 287 — atomic add_section primitive tests. Pairs with remove_section
 // tests above. Outline-lift carry closure (Phase C primitives).

 #[test]
 fn add_section_basic_creates_outline_and_persists() {
 let tmp = TempDir::new().unwrap();
 let path = tmp.path().join(".atomic/workspace.atomic.json");
 let mut store = AtomicStore::new();
 let receipt =
 add_section(&mut store, &path, "39", "docs/GENERATED.md", "Test Title", None)
 .unwrap();
 assert_eq!(receipt.primitive, "add_section");
 assert_eq!(receipt.target_id, "39");
 let s = store.section("39").expect("§39 created");
 assert_eq!(s.title, "Test Title");
 assert_eq!(s.parent_doc, "docs/GENERATED.md");
 assert_eq!(s.parent_section, None);
 assert_eq!(s.decision_status, Some(DecisionStatus::Active));
 // Round-trip through sidecar.
 let reloaded = AtomicStore::load(&path).unwrap();
 let s2 = reloaded.section("39").unwrap();
 assert_eq!(s2.title, "Test Title");
 assert_eq!(s2.parent_doc, "docs/GENERATED.md");
 }

 #[test]
 fn add_section_rejects_empty_section_id() {
 let tmp = TempDir::new().unwrap();
 let path = tmp.path().join(".atomic/workspace.atomic.json");
 let mut store = AtomicStore::new();
 let err = add_section(&mut store, &path, "   ", "docs/X.md", "T", None).unwrap_err();
 assert!(matches!(err, AtomicMutateError::Validation(_)));
 assert!(store.sections.is_empty(), "no section created on rejected mutate");
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
 let err =
 add_section(&mut store, &path, "39", "docs/X.md", "Second", None).unwrap_err();
 match err {
 AtomicMutateError::Validation(msg) => assert!(msg.contains("already exists")),
 other => panic!("expected Validation, got {:?}", other),
 }
 // Original section unchanged.
 assert_eq!(store.section("39").unwrap().title, "First");
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
 assert_eq!(child.parent_section.as_deref(), Some("39"));
 assert_eq!(child.title, "Child");
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
 assert_eq!(store.section("39").unwrap().title, "new title");
 let reloaded = AtomicStore::load(&path).unwrap();
 assert_eq!(reloaded.section("39").unwrap().title, "new title");
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
 assert_eq!(store.section("39").unwrap().title, "T");
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
 assert_eq!(store.section("39").unwrap().parent_doc, "docs/NEW.md");
 let reloaded = AtomicStore::load(&path).unwrap();
 assert_eq!(reloaded.section("39").unwrap().parent_doc, "docs/NEW.md");
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
 store.section("39.1").unwrap().parent_section.as_deref(),
 Some("39")
 );
 // Promote back to top-level (None).
 set_section_parent_section(&mut store, &path, "39.1", None).unwrap();
 assert_eq!(store.section("39.1").unwrap().parent_section, None);
 }

 #[test]
 fn set_section_parent_section_not_found_on_missing_parent() {
 let tmp = TempDir::new().unwrap();
 let path = tmp.path().join(".atomic/workspace.atomic.json");
 let mut store = AtomicStore::new();
 add_section(&mut store, &path, "39.1", "docs/X.md", "Child", None).unwrap();
 let err = set_section_parent_section(&mut store, &path, "39.1", Some("ghost"))
 .unwrap_err();
 assert!(matches!(err, AtomicMutateError::NotFound(_)));
 // Child unchanged.
 assert_eq!(store.section("39.1").unwrap().parent_section, None);
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
 )
 .unwrap();
 let raw = std::fs::read_to_string(&path).unwrap();
 assert!(raw.contains("\"decision_status\": \"superseded\""));
 let reloaded = AtomicStore::load(&path).unwrap();
 assert_eq!(
 reloaded.section("39").unwrap().decision_status,
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
 set_section_decision_status(&mut store, &path, "1", DecisionStatus::Active, None)
 .unwrap();
 set_section_decision_status(&mut store, &path, "1", DecisionStatus::Active, None)
 .unwrap();
 set_section_decision_status(
 &mut store,
 &path,
 "1",
 DecisionStatus::Superseded,
 Some("2"),
 )
 .unwrap();
 assert_eq!(
 store.section("1").unwrap().decision_status,
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
 assert!(store.section("39").is_none() || store.section("39").unwrap().decision_status.is_none());
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
 set_section_decision_status(&mut store, &path, "1", DecisionStatus::Active, None)
 .unwrap();
 set_section_decision_status(&mut store, &path, "2", DecisionStatus::Removed, None)
 .unwrap();
 assert_eq!(
 store.section("1").unwrap().decision_status,
 Some(DecisionStatus::Active)
 );
 assert_eq!(
 store.section("2").unwrap().decision_status,
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
 )
 .unwrap();
 assert_eq!(
 store.section("39").unwrap().decision_status,
 Some(DecisionStatus::Superseded)
 );
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
 assert!(store.section("1").unwrap().decision_status.is_none());
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
 "Round 243",
 Some("test"),
 &["change".into()],
 &["verify".into()],
 &[],
 &["carry".into()],
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
 entry_id,
 Some("audit summary"),
 &["audit change".into()],
 &["audit verify".into()],
 &["43".into()],
 &["audit carry".into()],
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
 set_changelog_publishable_impact_refs(
 &mut store,
 &path,
 "Round 999",
 &["61".into()],
 )
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
 let err = set_changelog_publishable_decision_summary(
 &mut store,
 &path,
 "Round 404",
 "anything",
 )
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
 assert_eq!(reloaded.schema_version, 4);
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
 "Round PA",
 Some(&long_summary),
 &["c".into()],
 &["v".into()],
 &["1".into()],
 &["cf".into()],
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
 "Round PB",
 Some("audit summary"),
 &bullets,
 &bullets,
 &bullets,
 &bullets,
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
 set_changelog_publishable_carry_forward_bullets(&mut store_b, &path_b, "Round PB", &bullets)
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
 set_changelog_publishable_decision_summary(
 &mut store,
 &path,
 "Round 999",
 "redacted",
 )
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
 let err = emit_publishable_override_ledger_draft(
 &store, "Round 999", "r", "a", "redaction",
 )
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
 &store, "Round 999", "audit reason", "Round T", "redaction",
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
 set_changelog_publishable_decision_summary(
 &mut store,
 &path,
 "Round 999",
 "redacted",
 )
 .unwrap();
 let entry = store.changelog_entries.get("Round 999").unwrap();
 let expected_after = entry.publishable_hash_hex();
 let expected_before = entry.audit_hash_hex();
 let draft = emit_publishable_override_ledger_draft(
 &store, "Round 999", "r", "a", "redaction",
 )
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

 #[test]
 fn normative_excerpt_sets_when_none() {
 let tmp = TempDir::new().unwrap();
 let path = tmp.path().join(".atomic/workspace.atomic.json");
 let mut store = AtomicStore::new();
 seed_section(&mut store, "scxml-3.13");
 set_section_normative_excerpt(
 &mut store,
 &path,
 "scxml-3.13",
 "The <event> element ...",
 "https://www.w3.org/TR/scxml/#event",
 "2015-09-01",
 )
 .unwrap();
 let excerpt = store
 .section("scxml-3.13")
 .unwrap()
 .normative_excerpt
 .as_ref()
 .unwrap();
 assert_eq!(excerpt.text, "The <event> element ...");
 assert_eq!(excerpt.anchor_url, "https://www.w3.org/TR/scxml/#event");
 assert_eq!(excerpt.source_revision, "2015-09-01");
 }

 #[test]
 fn normative_excerpt_rejects_overwrite() {
 let tmp = TempDir::new().unwrap();
 let path = tmp.path().join(".atomic/workspace.atomic.json");
 let mut store = AtomicStore::new();
 seed_section(&mut store, "scxml-3.13");
 set_section_normative_excerpt(
 &mut store,
 &path,
 "scxml-3.13",
 "first",
 "https://example.com/spec",
 "rev1",
 )
 .unwrap();
 let err = set_section_normative_excerpt(
 &mut store,
 &path,
 "scxml-3.13",
 "second",
 "https://example.com/spec",
 "rev2",
 )
 .unwrap_err();
 match err {
 AtomicMutateError::FrozenLedger(msg) => {
 assert!(
  msg.contains("already anchored"),
  "expected frozen-ledger reject msg; got: {}",
  msg
 );
 }
 other => panic!("expected FrozenLedger, got {:?}", other),
 }
 // Original value preserved
 let excerpt = store
 .section("scxml-3.13")
 .unwrap()
 .normative_excerpt
 .as_ref()
 .unwrap();
 assert_eq!(excerpt.text, "first");
 }

 #[test]
 fn normative_excerpt_rejects_blank_text() {
 let tmp = TempDir::new().unwrap();
 let path = tmp.path().join(".atomic/workspace.atomic.json");
 let mut store = AtomicStore::new();
 seed_section(&mut store, "scxml-3.13");
 let err = set_section_normative_excerpt(
 &mut store,
 &path,
 "scxml-3.13",
 " \n ",
 "https://example.com/spec",
 "rev1",
 )
 .unwrap_err();
 matches!(err, AtomicMutateError::Validation(_));
 }

 #[test]
 fn normative_excerpt_rejects_non_url_anchor() {
 let tmp = TempDir::new().unwrap();
 let path = tmp.path().join(".atomic/workspace.atomic.json");
 let mut store = AtomicStore::new();
 seed_section(&mut store, "scxml-3.13");
 let err = set_section_normative_excerpt(
 &mut store,
 &path,
 "scxml-3.13",
 "text",
 "not-a-url",
 "rev1",
 )
 .unwrap_err();
 match err {
 AtomicMutateError::Validation(msg) => {
 assert!(
  msg.contains("absolute http(s):// URL"),
  "expected URL-validation msg; got: {}",
  msg
 );
 }
 other => panic!("expected Validation, got {:?}", other),
 }
 }

 #[test]
 fn normative_excerpt_rejects_missing_host() {
 let tmp = TempDir::new().unwrap();
 let path = tmp.path().join(".atomic/workspace.atomic.json");
 let mut store = AtomicStore::new();
 seed_section(&mut store, "scxml-3.13");
 let err = set_section_normative_excerpt(
 &mut store,
 &path,
 "scxml-3.13",
 "text",
 "https:///path-only",
 "rev1",
 )
 .unwrap_err();
 matches!(err, AtomicMutateError::Validation(_));
 }

 #[test]
 fn normative_excerpt_trims_trailing_newline() {
 let tmp = TempDir::new().unwrap();
 let path = tmp.path().join(".atomic/workspace.atomic.json");
 let mut store = AtomicStore::new();
 seed_section(&mut store, "scxml-3.13");
 set_section_normative_excerpt(
 &mut store,
 &path,
 "scxml-3.13",
 "spec text\n\n",
 "https://example.com/spec",
 "rev1",
 )
 .unwrap();
 let excerpt = store
 .section("scxml-3.13")
 .unwrap()
 .normative_excerpt
 .as_ref()
 .unwrap();
 assert_eq!(
 excerpt.text, "spec text",
 "trailing newlines should be trimmed for stable round-trip render"
 );
 }
}
