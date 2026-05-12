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
//! - ChangelogEntry atomic: `append_changelog_entry_v2`
//!
//! `Section.implementations` lands as the substrate for Path B
//! of the code-citation defense (Spec ↔ Code bidirectional binding). The
//! atomic store records "this section is implemented at file:symbol";
//! cross-checks code citations against the spec's authoritative
//! binding (set-equality, the OPTION D pattern lifted from cross-
//! ref orphan reject). Schema + mutate primitive only — validator
//! extension and section seeding are deferred to later rounds.

use crate::schema::DecisionStatus;
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
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtomicSection {
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
/// Default = all empty. The legacy `sub_bullets` field carries stable — atomic
/// fields = additive only. T2 frozen_ledger_jaccard rule extends to atomic
/// fields: once committed, atomic fields are frozen
/// (deletion = T2 violation, addition = OK).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtomicChangelogEntry {
 /// 1 sentence headline.
 #[serde(default, skip_serializing_if = "Option::is_none")]
 pub decision_summary: Option<String>,
 #[serde(default, skip_serializing_if = "Vec::is_empty")]
 pub changes_bullets: Vec<String>,
 #[serde(default, skip_serializing_if = "Vec::is_empty")]
 pub verification_bullets: Vec<String>,
 /// cross-ref list (target section_id without `§` prefix).
 #[serde(default, skip_serializing_if = "Vec::is_empty")]
 pub impact_refs: Vec<String>,
 #[serde(default, skip_serializing_if = "Vec::is_empty")]
 pub carry_forward_bullets: Vec<String>,
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
/// `docs/.atomic/workspace.atomic.json` (path configurable via
/// `[atomic] sidecar_path` in mnemosyne.toml — extend 162 carry, default
/// if unset).
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
// Load is back-compat: version-1 stores deserialize with inventory_entries default
// (empty BTreeMap via #[serde(default)]); the next save rewrites schema_version to 2.
const CURRENT_SCHEMA_VERSION: u32 = 2;
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
 pub fn load(path: &Path) -> Result<Self, AtomicStoreError> {
 if !path.exists() {
 return Ok(Self::new());
 }
 let bytes = fs::read(path)?;
 let store: AtomicStore = serde_json::from_slice(&bytes)?;
 if store.schema_version > CURRENT_SCHEMA_VERSION {
 return Err(AtomicStoreError::SchemaVersionMismatch {
  store: store.schema_version,
  expected: CURRENT_SCHEMA_VERSION,
 });
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

 /// Get / create-default section atomic entry.
 pub fn section_mut(&mut self, section_id: &str) -> &mut AtomicSection {
 self.sections.entry(section_id.to_string()).or_default()
 }

 /// Get / create-default changelog atomic entry.
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
/// Used by `style.rs` body rules (run-on / sentence-length scan) and
/// `query.rs::build_section_view` (atomic-first body source). Bullet blocks
/// render with `- ` prefixes (so `is_only_code_or_table` filters them out
/// of paragraph-length checks); examples render as fenced code blocks
/// (skipped by detectors).
pub fn synthesize_section_body(atomic: &AtomicSection) -> String {
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
 if !atomic.implementations.is_empty() {
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

pub fn set_section_intent(
 store: &mut AtomicStore,
 sidecar_path: &Path,
 section_id: &str,
 intent: &str,
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
 check_intent_len(intent)?;
 store.section_mut(section_id).intent = Some(intent.to_string());
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
 store.section_mut(section_id).rationale_bullets = bullets.to_vec();
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
 store.section_mut(section_id).inputs_bullets = bullets.to_vec();
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
 store.section_mut(section_id).outputs_bullets = bullets.to_vec();
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
 store
 .section_mut(section_id)
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
 store.section_mut(section_id).alternatives_rejected = alternatives.to_vec();
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
 store.section_mut(section_id).impact_scope = refs.to_vec();
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
 store.section_mut(section_id).examples.push(example);
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
/// Symmetric with the markdown-axis guard at
/// `mutate::set_section_decision_status`. `Removed` is tombstone-exempt
/// (asserts finality, not replacement).
///
/// This guard does not validate that the named superseding section_id
/// exists in the atomic store — cross-ref orphan checking is T1 rule 1's
/// territory, picked up by `validate-workspace`. This primitive only
/// enforces presence of the *intent to forward*, mirroring the markdown
/// axis which also defers existence checking to the validator pass.
pub fn set_section_decision_status_atomic(
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
 store.section_mut(section_id).decision_status = Some(new_status);
 save_with_receipt(
 store,
 sidecar_path,
 "set_section_decision_status_atomic",
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
 let section = store.section_mut(section_id);
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

/// `append_changelog_entry_v2` primitive — atomic-aware changelog append.
///
/// Frozen ledger semantics: once committed,
/// existing fields cannot be modified or removed (T2 jaccard); subsequent
/// mutations to the same `entry_id` are rejected via FrozenLedger error.
pub fn append_changelog_entry_v2(
 store: &mut AtomicStore,
 sidecar_path: &Path,
 entry_id: &str,
 decision_summary: Option<&str>,
 changes_bullets: &[String],
 verification_bullets: &[String],
 impact_refs: &[String],
 carry_forward_bullets: &[String],
) -> Result<AtomicMutateReceipt, AtomicMutateError> {
 if store.changelog_entries.contains_key(entry_id) {
 return Err(AtomicMutateError::FrozenLedger(format!(
 "entry_id `{}` already exists in atomic store; mutations to existing \
  entries are forbidden (Round 161 §41 frozen ledger)",
 entry_id
 )));
 }
 let entry = AtomicChangelogEntry {
 decision_summary: decision_summary.map(str::to_string),
 changes_bullets: changes_bullets.to_vec(),
 verification_bullets: verification_bullets.to_vec(),
 impact_refs: impact_refs.to_vec(),
 carry_forward_bullets: carry_forward_bullets.to_vec(),
 };
 store
 .changelog_entries
 .insert(entry_id.to_string(), entry);
 save_with_receipt(
 store,
 sidecar_path,
 "append_changelog_entry_v2",
 "changelog_entry",
 entry_id,
 )
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
/// atomic primitive's. Mirrors the `set_section_decision_status_atomic`
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

 #[test]
 fn save_load_round_trip() {
 let tmp = TempDir::new().unwrap();
 let path = tmp.path().join(".atomic/workspace.atomic.json");
 let mut store = AtomicStore::new();
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
 append_changelog_entry_v2(
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
 let err = append_changelog_entry_v2(
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
 assert_eq!(reloaded.schema_version, 2);
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

 #[test]
 fn remove_section_drops_entry_and_persists() {
 // Round 267 — remove_section deletes the section_id entry from the
 // store and persists the change. Subsequent section() returns None.
 let tmp = TempDir::new().unwrap();
 let path = tmp.path().join(".atomic/workspace.atomic.json");
 let mut store = AtomicStore::new();
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

 #[test]
 fn set_section_decision_status_atomic_persists_and_round_trips() {
 // Round 265 — atomic decision_status field round-trips through
 // sidecar JSON. Default = None (skip_serializing_if), Some(_) appears
 // as lowercase string in JSON, deserializes back to enum.
 let tmp = TempDir::new().unwrap();
 let path = tmp.path().join(".atomic/workspace.atomic.json");
 let mut store = AtomicStore::new();
 set_section_decision_status_atomic(
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
 fn set_section_decision_status_atomic_overwrite_is_idempotent() {
 // Re-setting the same status does not error, and overwriting with a
 // different status replaces the previous value (no append-only semantics
 // — this is a single-field setter).
 let tmp = TempDir::new().unwrap();
 let path = tmp.path().join(".atomic/workspace.atomic.json");
 let mut store = AtomicStore::new();
 set_section_decision_status_atomic(&mut store, &path, "1", DecisionStatus::Active, None)
 .unwrap();
 set_section_decision_status_atomic(&mut store, &path, "1", DecisionStatus::Active, None)
 .unwrap();
 set_section_decision_status_atomic(
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
 fn set_section_decision_status_atomic_superseded_without_superseding_rejects() {
 // T1 rule 4 (atomic axis) author-time guard: Superseded transition
 // without a superseding section_id is a semantically-incoherent state
 // ("replaced, but no replacement recorded") and must reject at the
 // mutate boundary. Symmetric with the markdown-axis guard.
 let tmp = TempDir::new().unwrap();
 let path = tmp.path().join(".atomic/workspace.atomic.json");
 let mut store = AtomicStore::new();
 let err = set_section_decision_status_atomic(
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
 fn set_section_decision_status_atomic_active_no_superseding_required() {
 // Active and Removed targets do not require a superseding ref — only
 // Superseded does. Removed is tombstone-exempt (asserts finality, not
 // replacement); Active is the default starting state.
 let tmp = TempDir::new().unwrap();
 let path = tmp.path().join(".atomic/workspace.atomic.json");
 let mut store = AtomicStore::new();
 set_section_decision_status_atomic(&mut store, &path, "1", DecisionStatus::Active, None)
 .unwrap();
 set_section_decision_status_atomic(&mut store, &path, "2", DecisionStatus::Removed, None)
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
 fn set_section_decision_status_atomic_superseded_with_superseding_writes() {
 // Author-time guard accepts any non-None superseding string; existence
 // checking is rule 1's territory (validate-workspace), not rule 4's.
 // Symmetric with the markdown-axis guard which also defers existence
 // checking.
 let tmp = TempDir::new().unwrap();
 let path = tmp.path().join(".atomic/workspace.atomic.json");
 let mut store = AtomicStore::new();
 set_section_decision_status_atomic(
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
 let tmp = TempDir::new().unwrap();
 let path = tmp.path().join(".atomic/workspace.atomic.json");
 let mut store = AtomicStore::new();
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
 append_changelog_entry_v2(
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
}
