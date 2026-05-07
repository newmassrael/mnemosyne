//! Fine-grained Salsa dependency tracking layer (Round 92).
//!
//! Round 89 measurement surfaced *Gate (ii) limitation* — the default
//! `section_decision_status` / `frozen_list_membership` queries re-execute at
//! *branch granularity*. This module is the follow-up layer: it carries a
//! per-Section / per-CrossRef / per-FrozenList Salsa input pattern with
//! *individual tracked sub-queries* + per-branch aggregators.
//!
//! ## Design
//!
//! - Each Section / CrossRef / FrozenList / ChangelogEntry has its own
//! `#[salsa::input]` handle (mutable fields carry individual identity).
//! - `BranchIndex` Salsa input carries the per-branch list of record handles.
//! - Per-record tracked sub-queries (`section_decision_violation` /
//! `frozen_list_owner_resolution`) read the record handle + branch index.
//! - Aggregator tracked queries (`section_decision_status_aggregated` /
//! `frozen_list_membership_aggregated`) sum the sub-query results.
//!
//! ## Invalidation semantics (Salsa 0.26 field-level dep tracking)
//!
//! Each per-record sub-query forms a dep on that record's field-level reads —
//! mutating one record's field only invalidates the relevant sub-query,
//! while sibling records' sub-queries in the same branch carry. The
//! Aggregator depends on the sub-query result list — when one sub-query's
//! return value changes the aggregator re-executes, but if sub-query results
//! are identical the aggregator short-circuits ("Active" mutations cascade
//! down to 0 invalidations).
//!
//! ## Per-DB invocation counter (Salsa WillExecute event hook)
//!
//! `FineCascadeDb::new()` forwards a `Some(event_handler)` to `Storage::new`.
//! Salsa fires `EventKind::WillExecute` immediately before each tracked
//! function body runs (cache miss); the handler increments a per-DB
//! `Arc<AtomicUsize>` counter. The proxy is not a *true polynomial size
//! band* measurement — after a single fact mutation, reading the counter
//! gives a direct *count of sub-queries that received an invalidation*.
//! No global static is used, so parallel tests do not race.
//!
//! ## Boundary with existing API
//!
//! The existing `runtime` module's `section_decision_status(db, branch:
//! CascadeBranch)` is the opaque-`Vec<u8>`-snapshot, branch-level full
//! re-execution path — the Phase 0 production stack's stable carry. This
//! fine-grained API is a separate parallel path: bench measurement +
//! Phase 1.5+ production substitution candidate. §43 *cascade_query Forge
//! kind* body framing is unmutated; this layer's introduction is ratified
//! through Changelog entries only.

use crate::ValidationResult;
use mnemosyne_core::{ChangelogEntryFact, CrossRefFact, FrozenListFact, SectionFact};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

// --- per-DB invocation counter --------------------------------------------

/// Per-DB tracked function body execution counter. Wired into Salsa's
/// `EventKind::WillExecute` event hook — fires once per cache miss (body
/// runs). Cloned across the DB struct + the event handler closure so both
/// sides share the same counter.
#[derive(Default, Clone)]
pub struct ExecCounter {
 inner: Arc<AtomicUsize>,
}

impl ExecCounter {
 pub fn get(&self) -> usize {
 self.inner.load(Ordering::Relaxed)
 }
 pub fn reset(&self) {
 self.inner.store(0, Ordering::Relaxed);
 }
 fn bump(&self) {
 self.inner.fetch_add(1, Ordering::Relaxed);
 }
}

// --- per-record Salsa inputs -----------------------------------------------

#[salsa::input]
pub struct SectionRecord {
 pub branch_id: u64,
 pub entity_id: u64,
 pub valid_from: u64,
 pub doc_path: String,
 pub section_id: String,
 pub title: String,
 pub decision_status: String,
}

#[salsa::input]
pub struct CrossRefRecord {
 pub branch_id: u64,
 pub from_section: u64,
 pub to_section: u64,
 pub ref_kind: String,
}

#[salsa::input]
pub struct FrozenListRecord {
 pub branch_id: u64,
 pub entity_id: u64,
 pub valid_from: u64,
 pub owner_section: u64,
 pub frozen_round: u64,
 pub kind: String,
}

#[salsa::input]
pub struct ChangelogRecord {
 pub branch_id: u64,
 pub entity_id: u64,
 pub valid_from: u64,
 pub round_number: u64,
 pub summary: String,
 pub appended_at: u64,
}

#[salsa::input]
pub struct BranchIndex {
 pub branch_id: u64,
 pub sections: Vec<SectionRecord>,
 pub cross_refs: Vec<CrossRefRecord>,
 pub frozen_lists: Vec<FrozenListRecord>,
 pub changelog_entries: Vec<ChangelogRecord>,
 /// Round 114 — entity_id-keyed pre-indexed map for `sections`. Kept in
 /// sync with the `sections` Vec at build time (`build_branch_index`
 /// fills both atomically). Lookup-only consumers
 /// (`section_by_entity_id`) hit this field for O(log n) `get`; snapshot
 /// / iterator consumers fall back to the existing `sections` Vec.
 pub sections_map: std::collections::BTreeMap<u64, SectionRecord>,
 /// Round 114 — round_number-keyed pre-indexed map for
 /// `changelog_entries`. Kept in sync with the `changelog_entries` Vec
 /// at build time. Lookup-only consumers (`changelog_by_round_number`)
 /// hit this field for O(log n) `get`; iteration consumers fall back to
 /// the existing `changelog_entries` Vec.
 /// On `round_number` collision (two entries sharing the same round
 /// number) the semantics are *last-write-wins* — the entry inserted
 /// last during `build_branch_index` survives. In practice round_number
 /// is unique, so the collision branch never fires in measurements.
 pub changelog_entries_map: std::collections::BTreeMap<u64, ChangelogRecord>,
}

// --- per-record tracked sub-queries ----------------------------------------

/// Per-section outbound CrossRef pre-indexed cache (Round 94 (b)). Returns the
/// CrossRef handles whose `from_section` matches the requested section_id.
///
/// ## Why per-section indexing
///
/// Without this layer, [`section_decision_violation`] for a Superseded section
/// reads every `CrossRef.from_section` field directly (full Vec scan to filter
/// outbound refs). A single `ref_kind` mutation on any CrossRef X invalidates
/// every Superseded sub-query whose iteration reached X — worst-case linear
/// fan-out.
///
/// With this layer, `section_decision_violation` only reads the per-section
/// pre-indexed result Vec. Salsa "backdating" then prevents cascading
/// invalidation when the result Vec is bit-equal across mutation:
///
/// - **CrossRef.ref_kind mutation**: this body does NOT read `ref_kind` →
/// no dep formed → no invalidation here. Only the consumer sub-query for
/// `S = X.from_section` re-runs (it reads `cr.ref_kind` on the indexed
/// result). Other Superseded sections' sub-queries stay cached.
/// - **CrossRef.from_section mutation**: this body re-executes for every
/// section S (full scan to re-filter), but the result Vec only changes
/// for S ∈ {old.from_section, new.from_section}. Salsa skips downstream
/// bodies whose input return is bit-equal — so consumer sub-queries
/// re-run only for those ≤ 2 affected sections.
/// - **CrossRef Vec push/remove on BranchIndex**: invalidates all S's
/// indexed cache; downstream invalidation again gated by Vec equality.
#[salsa::tracked]
pub fn outbound_crossrefs_by_section<'db>(
 db: &'db dyn CascadeDb,
 branch_index: BranchIndex,
 section_id: u64,
) -> Vec<CrossRefRecord> {
 let cross_refs = branch_index.cross_refs(db);
 cross_refs
 .iter()
 .filter(|cr| cr.from_section(db) == section_id)
 .copied()
 .collect()
}

/// Per-section decision_status check. Returns 0 on pass, 1 on violation
/// (Superseded section without outbound decision/impl ref). Active sections
/// short-circuit without reading cross_refs — no dep on the cross-ref list,
/// so mutating any CrossRef does NOT invalidate this sub-query for active
/// sections.
///
/// Round 94 (b) — for Superseded sections, delegates to
/// [`outbound_crossrefs_by_section`] for per-section CrossRef list isolation.
/// `ref_kind`/`from_section` field reads occur only on the pre-indexed result,
/// not on the entire BranchIndex CrossRef Vec.
#[salsa::tracked]
pub fn section_decision_violation<'db>(
 db: &'db dyn CascadeDb,
 section: SectionRecord,
 branch_index: BranchIndex,
) -> u32 {
 let status = section.decision_status(db);
 if !status.eq_ignore_ascii_case("superseded") {
 return 0;
 }
 let entity_id = section.entity_id(db);
 let outbound = outbound_crossrefs_by_section(db, branch_index, entity_id);
 let has_supersedes_ref = outbound.iter().any(|cr| {
 cr.ref_kind(db).eq_ignore_ascii_case("decision")
 || cr.ref_kind(db).eq_ignore_ascii_case("impl")
 });
 if has_supersedes_ref {
 0
 } else {
 1
 }
}

/// Per-entity-id Section pre-indexed cache (Round 100). Returns the
/// `SectionRecord` whose `entity_id` matches the requested `entity_id`, or
/// `None` if no section is found.
///
/// ## Why per-entity_id indexing
///
/// Without this layer, [`frozen_list_owner_resolution`] for a FrozenList whose
/// `owner_section` references some entity scans the entire `BranchIndex.sections`
/// Vec on every call, reading every section's `entity_id` field. A FrozenList
/// owner_section mutation forces a full re-scan even though the owner→section
/// mapping is constant for almost every other FrozenList in the branch.
///
/// With this layer, `frozen_list_owner_resolution` only reads the per-entity_id
/// resolution result (a single `SectionRecord` handle or `None`). Salsa
/// "backdating" then prevents cascading invalidation when the resolved handle
/// is unchanged across mutation:
///
/// - **Section.title / Section.decision_status mutation**: this body does
/// NOT read those fields → no dep formed → no invalidation here. Downstream
/// sub-queries also stay cached (they consume only this layer's handle
/// return, not the title field).
/// - **BranchIndex.sections Vec push/remove**: invalidates every entity_id's
/// indexed cache; downstream invalidation gated by per-entity result
/// equality (existing matches still resolve to the same handle).
/// - **Section.entity_id mutation**: shifts which section resolves at which
/// key — every entity_id's indexed cache invalidates, and downstream
/// `frozen_list_owner_resolution` re-runs only for FrozenLists whose
/// owner_section now resolves to a different handle.
///
/// Round 94 (b) `outbound_crossrefs_by_section` pattern equivalent.
#[salsa::tracked]
pub fn section_by_entity_id<'db>(
 db: &'db dyn CascadeDb,
 branch_index: BranchIndex,
 entity_id: u64,
) -> Option<SectionRecord> {
 // Round 114 — BTreeMap-backed O(log n) lookup. The body reads
 // `sections_map` rather than the `sections` Vec, so a Vec-only
 // mutation that does NOT update the map (must not happen — they
 // are kept in sync at build time) would not invalidate this
 // sub-query. The pre-indexed Salsa cache + map source carry per-
 // entity_id cache slot semantics inherited from Round 100.
 branch_index.sections_map(db).get(&entity_id).copied()
}

/// Per-frozen-list owner resolution check. Returns 0 on pass, 1 on violation
/// (owner_section not present in the branch's section list).
///
/// Round 100 — delegates to [`section_by_entity_id`] for per-entity_id cache
/// isolation. This body now only reads the resolved handle's presence —
/// downstream sub-queries no longer pay for the full sections-Vec scan.
#[salsa::tracked]
pub fn frozen_list_owner_resolution<'db>(
 db: &'db dyn CascadeDb,
 frozen_list: FrozenListRecord,
 branch_index: BranchIndex,
) -> u32 {
 let owner_id = frozen_list.owner_section(db);
 if section_by_entity_id(db, branch_index, owner_id).is_some() {
 0
 } else {
 1
 }
}

/// Per-round_number ChangelogEntry pre-indexed cache (Round 101). Returns the
/// `ChangelogRecord` whose `round_number` matches the requested round, or
/// `None` if no entry is found.
///
/// ## Why per-round_number indexing
///
/// Without this layer, [`frozen_list_changelog_attachment`] resolved attachment
/// at branch-wide granularity — `frozen_lists.nonempty && changelog.is_empty()`
/// triggered a single bulk violation count. After the per-FrozenList round
/// resolution refactor, every FrozenList's `frozen_round` must independently
/// resolve to a ChangelogEntry; without this layer the refactored body would
/// scan the entire `changelog_entries` Vec for each FrozenList.
///
/// With this layer, `frozen_list_changelog_attachment` only reads the
/// per-round resolution result handle. Salsa "backdating" then prevents
/// cascading invalidation when the resolved handle is unchanged across
/// mutation:
///
/// - **ChangelogRecord.summary / appended_at mutation**: this body does NOT
/// read those fields → no dep formed → no invalidation here. Downstream
/// sub-queries also stay cached.
/// - **ChangelogRecord.round_number mutation**: every per-round cache slot
/// re-runs (the iteration reads each entry's `round_number`); only those
/// whose result changed propagate. Bounded by the number of unique
/// `frozen_round` keys queried.
/// - **BranchIndex.changelog_entries Vec push/remove**: invalidates every
/// round's indexed cache; downstream invalidation gated by per-round
/// result equality.
///
/// Round 100's `section_by_entity_id` pattern equivalent.
#[salsa::tracked]
pub fn changelog_by_round_number<'db>(
 db: &'db dyn CascadeDb,
 branch_index: BranchIndex,
 round_number: u64,
) -> Option<ChangelogRecord> {
 // Round 114 — BTreeMap-backed O(log n) lookup. Mirrors `section_by_entity_id`.
 branch_index
 .changelog_entries_map(db)
 .get(&round_number)
 .copied()
}

/// Per-FrozenList changelog round resolution check. Returns aggregate violation
/// count — one violation per FrozenList whose `frozen_round` does not resolve
/// to any ChangelogEntry on the branch.
///
/// Round 101 — refactored from branch-wide check to per-FrozenList round
/// resolution via [`changelog_by_round_number`]. Each FrozenList's
/// `frozen_round` is independently resolved against the changelog index;
/// missing rounds count toward the violation total. Salsa per-field tracking
/// + the per-round pre-indexed layer cooperate to isolate ChangelogEntry
/// field mutations that the index does not read (e.g. `summary`).
#[salsa::tracked]
pub fn frozen_list_changelog_attachment<'db>(
 db: &'db dyn CascadeDb,
 branch_index: BranchIndex,
) -> u32 {
 let frozen_lists = branch_index.frozen_lists(db);
 let mut total: u32 = 0;
 for fl in frozen_lists {
 let round = fl.frozen_round(db);
 if changelog_by_round_number(db, branch_index, round).is_none() {
 total = total.saturating_add(1);
 }
 }
 total
}

// --- aggregator tracked queries --------------------------------------------

#[salsa::tracked]
pub fn section_decision_status_aggregated<'db>(
 db: &'db dyn CascadeDb,
 branch_index: BranchIndex,
) -> ValidationResult {
 let sections = branch_index.sections(db);
 let mut total: u32 = 0;
 for s in sections {
 total = total.saturating_add(section_decision_violation(db, s, branch_index));
 }
 if total == 0 {
 ValidationResult::ok()
 } else {
 ValidationResult::violations(total)
 }
}

#[salsa::tracked]
pub fn frozen_list_membership_aggregated<'db>(
 db: &'db dyn CascadeDb,
 branch_index: BranchIndex,
) -> ValidationResult {
 let frozen_lists = branch_index.frozen_lists(db);
 let mut total: u32 = 0;
 for fl in frozen_lists {
 total = total.saturating_add(frozen_list_owner_resolution(db, fl, branch_index));
 }
 total = total.saturating_add(frozen_list_changelog_attachment(db, branch_index));
 if total == 0 {
 ValidationResult::ok()
 } else {
 ValidationResult::violations(total)
 }
}

// --- DB trait + concrete runtime --------------------------------------------

#[salsa::db]
pub trait CascadeDb: salsa::Database {
 fn fine_section_decision_status(&self, branch_index: BranchIndex) -> ValidationResult;
 fn fine_frozen_list_membership(&self, branch_index: BranchIndex) -> ValidationResult;
}

#[salsa::db]
#[derive(Clone)]
pub struct FineCascadeDb {
 storage: salsa::Storage<Self>,
 exec_counter: ExecCounter,
}

impl Default for FineCascadeDb {
 fn default() -> Self {
 Self::new()
 }
}

impl FineCascadeDb {
 pub fn new() -> Self {
 let exec_counter = ExecCounter::default();
 let counter_for_event = exec_counter.clone();
 let storage = salsa::Storage::new(Some(Box::new(move |event| {
 if matches!(event.kind, salsa::EventKind::WillExecute { .. }) {
  counter_for_event.bump();
 }
 })));
 Self {
 storage,
 exec_counter,
 }
 }

 /// Per-DB tracked function body execution counter — total bodies run since
 /// last `reset_exec_counter`. Cache hits do NOT increment.
 pub fn exec_counter(&self) -> usize {
 self.exec_counter.get()
 }

 pub fn reset_exec_counter(&self) {
 self.exec_counter.reset();
 }
}

#[salsa::db]
impl salsa::Database for FineCascadeDb {}

#[salsa::db]
impl CascadeDb for FineCascadeDb {
 fn fine_section_decision_status(&self, branch_index: BranchIndex) -> ValidationResult {
 section_decision_status_aggregated(self, branch_index)
 }
 fn fine_frozen_list_membership(&self, branch_index: BranchIndex) -> ValidationResult {
 frozen_list_membership_aggregated(self, branch_index)
 }
}

// --- builder helpers -------------------------------------------------------

/// Build a `BranchIndex` (with per-record Salsa inputs allocated) from the
/// existing typed-fact bundle. Used by bench measurement + tests — not on the
/// Phase 0 production hot path.
pub fn build_branch_index(
 db: &FineCascadeDb,
 branch_id: u64,
 sections: &[SectionFact],
 cross_refs: &[CrossRefFact],
 frozen_lists: &[FrozenListFact],
 changelog_entries: &[ChangelogEntryFact],
) -> BranchIndex {
 let section_records: Vec<SectionRecord> = sections
 .iter()
 .map(|s| {
 SectionRecord::new(
  db,
  s.branch_id,
  s.entity_id,
  s.valid_from,
  s.doc_path.clone(),
  s.section_id.clone(),
  s.title.clone(),
  s.decision_status.clone(),
 )
 })
 .collect();
 let cross_ref_records: Vec<CrossRefRecord> = cross_refs
 .iter()
 .map(|cr| {
 CrossRefRecord::new(
  db,
  cr.branch_id,
  cr.from_section,
  cr.to_section,
  cr.ref_kind.clone(),
 )
 })
 .collect();
 let frozen_list_records: Vec<FrozenListRecord> = frozen_lists
 .iter()
 .map(|fl| {
 FrozenListRecord::new(
  db,
  fl.branch_id,
  fl.entity_id,
  fl.valid_from,
  fl.owner_section,
  fl.frozen_round,
  fl.kind.clone(),
 )
 })
 .collect();
 let changelog_records: Vec<ChangelogRecord> = changelog_entries
 .iter()
 .map(|c| {
 ChangelogRecord::new(
  db,
  c.branch_id,
  c.entity_id,
  c.valid_from,
  c.round_number,
  c.summary.clone(),
  c.appended_at,
 )
 })
 .collect();
 // Round 114 — populate the BTreeMap pre-indexed layers alongside the
 // canonical Vecs. Both stay in lockstep through `build_branch_index`;
 // mutating tests that call BranchIndex setters directly must update
 // both the Vec and the Map fields together.
 let mut sections_map: std::collections::BTreeMap<u64, SectionRecord> =
 std::collections::BTreeMap::new();
 for s in &section_records {
 sections_map.insert(s.entity_id(db), *s);
 }
 let mut changelog_entries_map: std::collections::BTreeMap<u64, ChangelogRecord> =
 std::collections::BTreeMap::new();
 for c in &changelog_records {
 changelog_entries_map.insert(c.round_number(db), *c);
 }
 BranchIndex::new(
 db,
 branch_id,
 section_records,
 cross_ref_records,
 frozen_list_records,
 changelog_records,
 sections_map,
 changelog_entries_map,
 )
}

#[cfg(test)]
mod tests {
 use super::*;
 use mnemosyne_core::SectionFact;

 fn make_section(branch: u64, entity: u64, status: &str) -> SectionFact {
 SectionFact {
 branch_id: branch,
 entity_id: entity,
 valid_from: 100,
 doc_path: "docs/DESIGN.md".into(),
 section_id: format!("{branch}.{entity}"),
 title: format!("section {entity}"),
 decision_status: status.into(),
 }
 }

 fn make_cross_ref(branch: u64, from: u64, to: u64, kind: &str) -> CrossRefFact {
 CrossRefFact {
 branch_id: branch,
 from_section: from,
 to_section: to,
 ref_kind: kind.into(),
 }
 }

 fn make_frozen_list(branch: u64, entity: u64, owner: u64) -> FrozenListFact {
 FrozenListFact {
 branch_id: branch,
 entity_id: entity,
 valid_from: 100,
 owner_section: owner,
 frozen_round: 60,
 kind: "release_lock".into(),
 }
 }

 fn make_changelog(branch: u64, entity: u64) -> ChangelogEntryFact {
 ChangelogEntryFact {
 branch_id: branch,
 entity_id: entity,
 valid_from: 100,
 round_number: 60,
 summary: "x".into(),
 appended_at: 2026_05_03,
 }
 }

 fn make_frozen_list_with_round(
 branch: u64,
 entity: u64,
 owner: u64,
 round: u64,
 ) -> FrozenListFact {
 FrozenListFact {
 branch_id: branch,
 entity_id: entity,
 valid_from: 100,
 owner_section: owner,
 frozen_round: round,
 kind: "release_lock".into(),
 }
 }

 fn make_changelog_with_round(branch: u64, entity: u64, round: u64) -> ChangelogEntryFact {
 ChangelogEntryFact {
 branch_id: branch,
 entity_id: entity,
 valid_from: 100,
 round_number: round,
 summary: "x".into(),
 appended_at: 2026_05_03,
 }
 }

 #[test]
 fn empty_branch_aggregators_are_vacuously_ok() {
 let db = FineCascadeDb::new();
 let idx = build_branch_index(&db, 1, &[], &[], &[], &[]);
 assert_eq!(
 section_decision_status_aggregated(&db, idx),
 ValidationResult::ok()
 );
 assert_eq!(
 frozen_list_membership_aggregated(&db, idx),
 ValidationResult::ok()
 );
 }

 #[test]
 fn active_sections_pass_aggregated() {
 let db = FineCascadeDb::new();
 let idx = build_branch_index(
 &db,
 1,
 &[make_section(1, 39, "Active"), make_section(1, 40, "Active")],
 &[],
 &[],
 &[],
 );
 assert_eq!(
 section_decision_status_aggregated(&db, idx),
 ValidationResult::ok()
 );
 }

 #[test]
 fn superseded_with_outbound_passes_aggregated() {
 let db = FineCascadeDb::new();
 let idx = build_branch_index(
 &db,
 1,
 &[make_section(1, 15, "Superseded")],
 &[make_cross_ref(1, 15, 56, "decision")],
 &[],
 &[],
 );
 assert_eq!(
 section_decision_status_aggregated(&db, idx),
 ValidationResult::ok()
 );
 }

 #[test]
 fn superseded_without_outbound_fails_aggregated() {
 let db = FineCascadeDb::new();
 let idx = build_branch_index(
 &db,
 1,
 &[make_section(1, 15, "Superseded")],
 &[],
 &[],
 &[],
 );
 assert_eq!(
 section_decision_status_aggregated(&db, idx),
 ValidationResult::violations(1)
 );
 }

 #[test]
 fn unread_field_mutation_triggers_zero_invocations() {
 // Salsa 0.26 tracks field-level reads. The `section_decision_violation`
 // body only reads `decision_status` and `entity_id` (and cross_refs
 // only if Superseded). It NEVER reads `title`. Mutating `title` on any
 // Section should therefore trigger ZERO body executions across all
 // sub-queries + aggregators — perfect cache reuse, the strongest
 // possible fine-grained tracking property.
 use salsa::Setter;
 let mut db = FineCascadeDb::new();
 let idx = build_branch_index(
 &db,
 1,
 &[
  make_section(1, 1, "Active"),
  make_section(1, 2, "Active"),
  make_section(1, 3, "Active"),
  make_section(1, 4, "Active"),
  make_section(1, 5, "Active"),
 ],
 &[],
 &[],
 &[],
 );

 let r0 = section_decision_status_aggregated(&db, idx);
 assert!(r0.ok);

 db.reset_exec_counter();
 let target_section = idx.sections(&db)[2];
 target_section.set_title(&mut db).to("changed".into());

 let r1 = section_decision_status_aggregated(&db, idx);
 assert!(r1.ok);
 let exec = db.exec_counter();
 assert_eq!(
 exec, 0,
 "mutating an unread field (Section.title) must invalidate zero sub-queries; got {exec}"
 );
 }

 #[test]
 fn single_active_to_superseded_mutation_invalidates_only_target_section() {
 use salsa::Setter;
 let mut db = FineCascadeDb::new();
 let idx = build_branch_index(
 &db,
 1,
 &[
  make_section(1, 1, "Active"),
  make_section(1, 2, "Active"),
  make_section(1, 3, "Active"),
 ],
 &[],
 &[],
 &[],
 );

 let r0 = section_decision_status_aggregated(&db, idx);
 assert!(r0.ok);

 db.reset_exec_counter();
 let target = idx.sections(&db)[1];
 target.set_decision_status(&mut db).to("Superseded".into());

 let r1 = section_decision_status_aggregated(&db, idx);
 assert_eq!(r1.violation_count, 1);
 let exec = db.exec_counter();
 // Round 94 (b) per-section CrossRef pre-indexed layer breakdown:
 // 1. Target sub-query re-executes (decision_status changed)
 // 2. outbound_crossrefs_by_section(idx, target.entity_id) first
 // invocation — newly-Superseded sections delegate to this layer,
 // and this section_id had not been queried during the baseline
 // Active run.
 // 3. Aggregator re-executes (target sub now returns 1 instead of 0)
 // Other Active sections' sub-queries stay cached.
 assert_eq!(
 exec, 3,
 "expected 3 body executions (target sub + outbound_crossrefs_by_section + aggregator); got {exec}"
 );
 }

 #[test]
 fn frozen_list_with_owner_and_changelog_passes() {
 let db = FineCascadeDb::new();
 let idx = build_branch_index(
 &db,
 1,
 &[make_section(1, 39, "Active")],
 &[],
 &[make_frozen_list(1, 1000, 39)],
 &[make_changelog(1, 60)],
 );
 assert_eq!(
 frozen_list_membership_aggregated(&db, idx),
 ValidationResult::ok()
 );
 }

 #[test]
 fn frozen_list_with_dangling_owner_fails() {
 let db = FineCascadeDb::new();
 let idx = build_branch_index(
 &db,
 1,
 &[],
 &[],
 &[make_frozen_list(1, 1000, 99)],
 &[make_changelog(1, 60)],
 );
 assert_eq!(
 frozen_list_membership_aggregated(&db, idx),
 ValidationResult::violations(1)
 );
 }

 #[test]
 fn frozen_list_without_changelog_attachment_fails() {
 let db = FineCascadeDb::new();
 let idx = build_branch_index(
 &db,
 1,
 &[make_section(1, 39, "Active")],
 &[],
 &[make_frozen_list(1, 1000, 39)],
 &[],
 );
 assert_eq!(
 frozen_list_membership_aggregated(&db, idx),
 ValidationResult::violations(1)
 );
 }

 #[test]
 fn cascade_db_trait_dispatch_works() {
 let db = FineCascadeDb::new();
 let idx = build_branch_index(&db, 1, &[], &[], &[], &[]);
 let r1 = db.fine_section_decision_status(idx);
 let r2 = db.fine_frozen_list_membership(idx);
 assert_eq!(r1, ValidationResult::ok());
 assert_eq!(r2, ValidationResult::ok());
 }

 #[test]
 fn aggregator_idempotent_under_repeated_call() {
 let db = FineCascadeDb::new();
 let idx = build_branch_index(
 &db,
 1,
 &[make_section(1, 1, "Active")],
 &[],
 &[],
 &[],
 );
 let _ = section_decision_status_aggregated(&db, idx);
 let baseline = db.exec_counter();

 // Subsequent calls without mutation should hit cache — counter
 // should not increment.
 for _ in 0..10 {
 let _ = section_decision_status_aggregated(&db, idx);
 }
 assert_eq!(db.exec_counter(), baseline);
 }

 #[test]
 fn distinct_db_instances_have_independent_counters() {
 let db1 = FineCascadeDb::new();
 let db2 = FineCascadeDb::new();
 let idx1 = build_branch_index(&db1, 1, &[make_section(1, 1, "Active")], &[], &[], &[]);
 let _ = section_decision_status_aggregated(&db1, idx1);
 let count1 = db1.exec_counter();
 assert!(count1 > 0);
 assert_eq!(db2.exec_counter(), 0, "db2 counter must stay zero");
 }

 #[test]
 fn outbound_crossrefs_by_section_returns_only_matching_handles() {
 // Round 94 (b) — pre-indexed layer correctness check.
 let db = FineCascadeDb::new();
 let idx = build_branch_index(
 &db,
 1,
 &[
  make_section(1, 10, "Superseded"),
  make_section(1, 20, "Superseded"),
 ],
 &[
  make_cross_ref(1, 10, 50, "decision"),
  make_cross_ref(1, 10, 51, "impl"),
  make_cross_ref(1, 20, 52, "decision"),
  make_cross_ref(1, 30, 53, "decision"),
 ],
 &[],
 &[],
 );
 let outbound_10 = outbound_crossrefs_by_section(&db, idx, 10);
 assert_eq!(outbound_10.len(), 2);
 let outbound_20 = outbound_crossrefs_by_section(&db, idx, 20);
 assert_eq!(outbound_20.len(), 1);
 let outbound_99 = outbound_crossrefs_by_section(&db, idx, 99);
 assert_eq!(outbound_99.len(), 0);
 }

 #[test]
 fn unrelated_crossref_ref_kind_mutation_isolates_other_superseded_sections() {
 // Round 94 (b) isolation property — single CrossRef.ref_kind mutation
 // affects ONLY the from_section's sub-query. Other Superseded sections
 // (whose outbound list does not contain the mutated CrossRef) stay
 // cached.
 //
 // Setup: 3 Superseded sections, each with its own outbound decision
 // CrossRef. Mutate CrossRef X's ref_kind on section_1's outbound
 // (decision → unknown_kind). Expected:
 // - outbound_crossrefs_by_section(idx, 1) does NOT re-run (it does
 // not read ref_kind, only from_section).
 // - section_decision_violation(section_1, idx) re-runs (it reads
 // ref_kind on its outbound list, which now contains X with new
 // ref_kind → still detects no decision/impl → violations += 1).
 // - section_decision_violation for sections 2, 3 stay cached
 // (ref_kind on X is NOT in their outbound).
 // - Aggregator re-runs (section_1's sub-query return changed).
 //
 // Total expected exec_counter ≤ 2 (section_1 sub-query + aggregator).
 // Without the pre-indexed layer, all 3 Superseded sub-queries would
 // re-run because they all iterate the global cross_refs list and read
 // each CrossRef's ref_kind during iteration.
 use salsa::Setter;
 let mut db = FineCascadeDb::new();
 let idx = build_branch_index(
 &db,
 1,
 &[
  make_section(1, 1, "Superseded"),
  make_section(1, 2, "Superseded"),
  make_section(1, 3, "Superseded"),
 ],
 &[
  make_cross_ref(1, 1, 100, "decision"),
  make_cross_ref(1, 2, 101, "decision"),
  make_cross_ref(1, 3, 102, "decision"),
 ],
 &[],
 &[],
 );
 // Baseline: all 3 sections pass (each has outbound decision ref).
 let r0 = section_decision_status_aggregated(&db, idx);
 assert!(r0.ok);

 db.reset_exec_counter();
 // Mutate X's ref_kind to a non-{decision,impl} value. X is section_1's
 // outbound CrossRef. After mutation section_1 no longer has a valid
 // outbound decision/impl ref → violations += 1.
 let target_cross_ref = idx
 .cross_refs(&db)
 .iter()
 .find(|cr| cr.from_section(&db) == 1)
 .copied()
 .expect("section_1's outbound CrossRef must exist");
 target_cross_ref
 .set_ref_kind(&mut db)
 .to("see_also".into());

 let r1 = section_decision_status_aggregated(&db, idx);
 assert_eq!(r1.violation_count, 1);
 let exec = db.exec_counter();
 // Expected:
 // - section_decision_violation(section_1, idx) re-runs (1 invocation)
 // - aggregator re-runs (section_1's sub-query return changed) (1 invocation)
 // - outbound_crossrefs_by_section does NOT re-run (it does not read
 // ref_kind — only from_section, which is unchanged).
 // - section_decision_violation for sections 2, 3 stay cached (their
 // outbound lists do not include the mutated CrossRef).
 assert_eq!(
 exec, 2,
 "ref_kind mutation should invalidate only the from_section's sub-query + aggregator; got {exec}"
 );
 }

 #[test]
 fn outbound_crossrefs_by_section_caches_per_section_id() {
 // Each (idx, section_id) pair produces an independent cache slot.
 // Repeated calls with the same arguments must NOT increment counter.
 let db = FineCascadeDb::new();
 let idx = build_branch_index(
 &db,
 1,
 &[
  make_section(1, 1, "Superseded"),
  make_section(1, 2, "Superseded"),
 ],
 &[
  make_cross_ref(1, 1, 100, "decision"),
  make_cross_ref(1, 2, 101, "decision"),
 ],
 &[],
 &[],
 );
 let _ = outbound_crossrefs_by_section(&db, idx, 1);
 let _ = outbound_crossrefs_by_section(&db, idx, 2);
 let baseline = db.exec_counter();
 for _ in 0..5 {
 let _ = outbound_crossrefs_by_section(&db, idx, 1);
 let _ = outbound_crossrefs_by_section(&db, idx, 2);
 }
 assert_eq!(db.exec_counter(), baseline);
 }

 #[test]
 fn section_by_entity_id_returns_matching_handle() {
 // Round 100 — pre-indexed layer correctness check. 5 sections covering
 // the matched-id, missing-id, and zero-id boundary cases.
 let db = FineCascadeDb::new();
 let idx = build_branch_index(
 &db,
 1,
 &[
  make_section(1, 10, "Active"),
  make_section(1, 20, "Active"),
  make_section(1, 30, "Active"),
  make_section(1, 40, "Active"),
  make_section(1, 50, "Active"),
 ],
 &[],
 &[],
 &[],
 );

 for entity_id in [10u64, 20, 30, 40, 50] {
 let resolved = section_by_entity_id(&db, idx, entity_id);
 assert!(
  resolved.is_some(),
  "expected Section with entity_id={entity_id} to resolve"
 );
 assert_eq!(resolved.unwrap().entity_id(&db), entity_id);
 }

 // Unknown entity_ids resolve to None.
 assert!(section_by_entity_id(&db, idx, 999).is_none());
 assert!(section_by_entity_id(&db, idx, 0).is_none());
 }

 #[test]
 fn frozen_list_owner_unrelated_section_field_mutation_isolates_other_frozen_lists() {
 // Round 100 isolation property — Section.title (a field NOT read by
 // section_by_entity_id, which only reads entity_id) mutation must
 // produce zero re-executions in the frozen_list_owner_resolution
 // pipeline. Demonstrates per-field dep tracking carries through the
 // pre-indexed layer.
 //
 // Setup: 3 FrozenLists each owning one of 3 Sections (1↔100, 2↔200,
 // 3↔300). 1 ChangelogEntry so the changelog_attachment side-condition
 // passes (frozen_list_membership_aggregated needs both gates clean).
 // Baseline: aggregator runs, 3 frozen_list_owner_resolution sub-queries
 // execute, 3 section_by_entity_id sub-queries execute.
 // Then mutate Section_2.title. Expected exec_counter = 0 — no body
 // reads `title`, so per-field dep tracking gates every layer.
 use salsa::Setter;
 let mut db = FineCascadeDb::new();
 let idx = build_branch_index(
 &db,
 1,
 &[
  make_section(1, 100, "Active"),
  make_section(1, 200, "Active"),
  make_section(1, 300, "Active"),
 ],
 &[],
 &[
  make_frozen_list(1, 1, 100),
  make_frozen_list(1, 2, 200),
  make_frozen_list(1, 3, 300),
 ],
 &[make_changelog(1, 99)],
 );
 let r0 = frozen_list_membership_aggregated(&db, idx);
 assert!(r0.ok, "all owner resolutions must pass at baseline");

 db.reset_exec_counter();

 let target_section = idx
 .sections(&db)
 .iter()
 .find(|s| s.entity_id(&db) == 200)
 .copied()
 .expect("Section_200 must exist");
 target_section
 .set_title(&mut db)
 .to("renamed-title".into());

 let r1 = frozen_list_membership_aggregated(&db, idx);
 assert!(r1.ok, "title mutation must not affect ownership resolution");
 let exec = db.exec_counter();
 assert_eq!(
 exec, 0,
 "Section.title mutation should invalidate nothing in the frozen-list ownership chain; got {exec}"
 );
 }

 #[test]
 fn section_by_entity_id_caches_per_entity_id() {
 // Round 100 — each (idx, entity_id) pair produces an independent cache
 // slot. Repeated calls with the same arguments must NOT increment the
 // exec counter.
 let db = FineCascadeDb::new();
 let idx = build_branch_index(
 &db,
 1,
 &[
  make_section(1, 1, "Active"),
  make_section(1, 2, "Active"),
  make_section(1, 3, "Active"),
 ],
 &[],
 &[],
 &[],
 );
 let _ = section_by_entity_id(&db, idx, 1);
 let _ = section_by_entity_id(&db, idx, 2);
 let _ = section_by_entity_id(&db, idx, 3);
 let baseline = db.exec_counter();
 for _ in 0..5 {
 let _ = section_by_entity_id(&db, idx, 1);
 let _ = section_by_entity_id(&db, idx, 2);
 let _ = section_by_entity_id(&db, idx, 3);
 }
 assert_eq!(db.exec_counter(), baseline);
 }

 #[test]
 fn changelog_by_round_number_returns_matching_handle() {
 // Round 101 — pre-indexed layer correctness check. 5 changelog entries
 // covering matched-round, missing-round, and zero-id boundary cases.
 let db = FineCascadeDb::new();
 let idx = build_branch_index(
 &db,
 1,
 &[],
 &[],
 &[],
 &[
  make_changelog_with_round(1, 10, 100),
  make_changelog_with_round(1, 20, 200),
  make_changelog_with_round(1, 30, 300),
  make_changelog_with_round(1, 40, 400),
  make_changelog_with_round(1, 50, 500),
 ],
 );

 for round in [100u64, 200, 300, 400, 500] {
 let resolved = changelog_by_round_number(&db, idx, round);
 assert!(
  resolved.is_some(),
  "expected ChangelogEntry with round_number={round} to resolve"
 );
 assert_eq!(resolved.unwrap().round_number(&db), round);
 }

 // Unknown rounds resolve to None.
 assert!(changelog_by_round_number(&db, idx, 999).is_none());
 assert!(changelog_by_round_number(&db, idx, 0).is_none());
 }

 #[test]
 fn frozen_list_changelog_unrelated_changelog_field_mutation_isolates_other_frozen_lists() {
 // Round 101 isolation property — ChangelogEntry.summary (a field NOT
 // read by changelog_by_round_number, which only reads round_number)
 // mutation must produce zero re-executions in the
 // frozen_list_changelog_attachment pipeline. Demonstrates per-field
 // dep tracking carries through the per-round pre-indexed layer.
 //
 // Setup: 3 FrozenLists with frozen_round ∈ {10, 20, 30}, each owning
 // a Section so owner resolution passes. 3 ChangelogEntries with
 // round_number ∈ {10, 20, 30} so every FrozenList resolves.
 // Baseline: aggregator runs, all sub-queries execute, all attachment
 // resolutions succeed → ok.
 // Then mutate ChangelogEntry[round=20].summary. Expected exec_counter
 // = 0 — no body reads `summary`, so per-field dep tracking gates
 // every layer.
 use salsa::Setter;
 let mut db = FineCascadeDb::new();
 let idx = build_branch_index(
 &db,
 1,
 &[
  make_section(1, 100, "Active"),
  make_section(1, 200, "Active"),
  make_section(1, 300, "Active"),
 ],
 &[],
 &[
  make_frozen_list_with_round(1, 1, 100, 10),
  make_frozen_list_with_round(1, 2, 200, 20),
  make_frozen_list_with_round(1, 3, 300, 30),
 ],
 &[
  make_changelog_with_round(1, 1001, 10),
  make_changelog_with_round(1, 1002, 20),
  make_changelog_with_round(1, 1003, 30),
 ],
 );
 let r0 = frozen_list_membership_aggregated(&db, idx);
 assert!(r0.ok, "all changelog resolutions must pass at baseline");

 db.reset_exec_counter();

 let target_changelog = idx
 .changelog_entries(&db)
 .iter()
 .find(|c| c.round_number(&db) == 20)
 .copied()
 .expect("ChangelogEntry round=20 must exist");
 target_changelog
 .set_summary(&mut db)
 .to("renamed-summary".into());

 let r1 = frozen_list_membership_aggregated(&db, idx);
 assert!(
 r1.ok,
 "summary mutation must not affect changelog round resolution"
 );
 let exec = db.exec_counter();
 assert_eq!(
 exec, 0,
 "ChangelogEntry.summary mutation should invalidate nothing in the frozen-list changelog chain; got {exec}"
 );
 }

 #[test]
 fn section_by_entity_id_uses_btreemap_layer_not_vec_scan() {
 // Round 114 — `section_by_entity_id` reads `sections_map` rather than
 // the `sections` Vec. Demonstrate by mutating ONLY the map (via
 // setter), leaving the Vec stale, and verifying the lookup follows
 // the map. In production, `build_branch_index` keeps both in sync;
 // this test artificially diverges them to prove the BTreeMap field
 // is the source of truth for the per-entity_id lookup path.
 use salsa::Setter;
 let mut db = FineCascadeDb::new();
 let idx = build_branch_index(
 &db,
 1,
 &[
  make_section(1, 100, "Active"),
  make_section(1, 200, "Active"),
 ],
 &[],
 &[],
 &[],
 );

 let initial = section_by_entity_id(&db, idx, 100);
 assert!(initial.is_some(), "entity_id=100 must resolve at baseline");

 // Replace the map with one that drops entity_id=100 entirely. The
 // Vec stays unchanged.
 let surviving = idx
 .sections(&db)
 .iter()
 .find(|s| s.entity_id(&db) == 200)
 .copied()
 .expect("section 200 in vec");
 let mut new_map: std::collections::BTreeMap<u64, SectionRecord> =
 std::collections::BTreeMap::new();
 new_map.insert(200, surviving);
 idx.set_sections_map(&mut db).to(new_map);

 let after = section_by_entity_id(&db, idx, 100);
 assert!(
 after.is_none(),
 "BTreeMap-backed lookup must follow `sections_map` mutation, not the stale `sections` Vec"
 );
 let still_present = section_by_entity_id(&db, idx, 200);
 assert!(still_present.is_some(), "entity_id=200 must remain after map mutation");
 }

 #[test]
 fn changelog_by_round_number_uses_btreemap_layer_not_vec_scan() {
 // Round 114 — mirror of the section test for the changelog layer.
 use salsa::Setter;
 let mut db = FineCascadeDb::new();
 let idx = build_branch_index(
 &db,
 1,
 &[],
 &[],
 &[],
 &[
  make_changelog_with_round(1, 1, 10),
  make_changelog_with_round(1, 2, 20),
 ],
 );
 assert!(changelog_by_round_number(&db, idx, 10).is_some());
 let surviving = idx
 .changelog_entries(&db)
 .iter()
 .find(|c| c.round_number(&db) == 20)
 .copied()
 .expect("changelog round=20 in vec");
 let mut new_map: std::collections::BTreeMap<u64, ChangelogRecord> =
 std::collections::BTreeMap::new();
 new_map.insert(20, surviving);
 idx.set_changelog_entries_map(&mut db).to(new_map);
 assert!(changelog_by_round_number(&db, idx, 10).is_none());
 assert!(changelog_by_round_number(&db, idx, 20).is_some());
 }

 #[test]
 fn changelog_by_round_number_caches_per_round_number() {
 // Round 101 — each (idx, round_number) pair produces an independent
 // cache slot. Repeated calls with the same arguments must NOT
 // increment the exec counter.
 let db = FineCascadeDb::new();
 let idx = build_branch_index(
 &db,
 1,
 &[],
 &[],
 &[],
 &[
  make_changelog_with_round(1, 1, 10),
  make_changelog_with_round(1, 2, 20),
  make_changelog_with_round(1, 3, 30),
 ],
 );
 let _ = changelog_by_round_number(&db, idx, 10);
 let _ = changelog_by_round_number(&db, idx, 20);
 let _ = changelog_by_round_number(&db, idx, 30);
 let baseline = db.exec_counter();
 for _ in 0..5 {
 let _ = changelog_by_round_number(&db, idx, 10);
 let _ = changelog_by_round_number(&db, idx, 20);
 let _ = changelog_by_round_number(&db, idx, 30);
 }
 assert_eq!(db.exec_counter(), baseline);
 }

 #[test]
 fn frozen_list_lifecycle_field_mutation_isolates_other_frozen_lists() {
 // Round 115 — `frozen_list_owner_resolution` reads only
 // `frozen_list.owner_section(db)`; `frozen_list_changelog_attachment`
 // iterates the frozen-lists Vec and reads only `fl.frozen_round(db)`.
 // Neither body reads `valid_from` or `kind`. Salsa's per-field
 // dependency tracking must therefore produce zero re-executions
 // when those lifecycle fields mutate.
 //
 // Setup: 3 FrozenLists with frozen_round ∈ {10, 20, 30}, owners 100/200/300,
 // 3 Sections matching the owners, 3 ChangelogEntries matching the rounds.
 // Baseline aggregator runs, all sub-queries cache.
 // Then mutate FrozenList[round=20].valid_from and .kind in turn. Each
 // mutation must produce exec_counter == 0.
 use salsa::Setter;
 let mut db = FineCascadeDb::new();
 let idx = build_branch_index(
 &db,
 1,
 &[
  make_section(1, 100, "Active"),
  make_section(1, 200, "Active"),
  make_section(1, 300, "Active"),
 ],
 &[],
 &[
  make_frozen_list_with_round(1, 1, 100, 10),
  make_frozen_list_with_round(1, 2, 200, 20),
  make_frozen_list_with_round(1, 3, 300, 30),
 ],
 &[
  make_changelog_with_round(1, 1001, 10),
  make_changelog_with_round(1, 1002, 20),
  make_changelog_with_round(1, 1003, 30),
 ],
 );
 let r0 = frozen_list_membership_aggregated(&db, idx);
 assert!(r0.ok, "all changelog + owner resolutions must pass at baseline");

 let target_fl = idx
 .frozen_lists(&db)
 .iter()
 .find(|fl| fl.frozen_round(&db) == 20)
 .copied()
 .expect("frozen_list with round=20");

 // (a) valid_from mutation — no body reads `valid_from`, so per-field
 // tracking gates every layer: exec_counter must stay zero.
 db.reset_exec_counter();
 target_fl.set_valid_from(&mut db).to(99_999);
 let r1 = frozen_list_membership_aggregated(&db, idx);
 assert!(r1.ok);
 assert_eq!(
 db.exec_counter(),
 0,
 "FrozenList.valid_from mutation must not invalidate any sub-query (read by no body); got {}",
 db.exec_counter()
 );

 // (b) kind mutation — same isolation property.
 db.reset_exec_counter();
 target_fl.set_kind(&mut db).to("milestone".into());
 let r2 = frozen_list_membership_aggregated(&db, idx);
 assert!(r2.ok);
 assert_eq!(
 db.exec_counter(),
 0,
 "FrozenList.kind mutation must not invalidate any sub-query (read by no body); got {}",
 db.exec_counter()
 );
 }
}
