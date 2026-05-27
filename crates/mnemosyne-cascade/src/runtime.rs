//! Salsa 0.26 runtime — actual `#[salsa::input]` / `#[salsa::tracked]` /
//! `#[salsa::db]` binding for the design_doc cascade queries.
//!
//! Mirror of the prototype golden `bench/codegen-prototype/tests/fixtures/
//! salsa_wire_emit.rs` — that file exists as a regen-able snapshot of the
//! emitter output, here we ingest the same runtime semantics directly into a
//! production-grade compile path.
//!
//! `MnemosyneCascadeDb` owns the `salsa::Storage<Self>` and implements both
//! `salsa::Database` and the local `CascadeDb` trait. Tracked functions read
//! the per-branch `BranchSnapshotData` (decoded from `CascadeBranch.snapshot_payload`)
//! and validate the T1 invariants surfaced as cascade queries:
//!
//! - `section_decision_status` — for each Section with `decision_status =
//! "Superseded"`, there must exist an outbound CrossRef of kind
//! `decision`/`impl` (the supersession pointer to the superseding section).
//! - `frozen_list_membership` — for each FrozenList, the `owner_section` must
//! exist as a Section in the snapshot, and at least one ChangelogEntry must
//! accompany the snapshot (membership delta requires changelog attachment
//! per T1 rule 3).

use crate::snapshot::BranchSnapshotData;

/// Cascade query output type — Phase 0 carries `ok` + per-violation count.
/// Phase 1.5 expands this to carry T1/T2/T3 gate results + audit trail; the
/// deterministic-stub `Default` (`ok: false`) preserved as the empty-snapshot
/// vacuous-failure path so callers explicitly opt-in to vacuous truth via
/// non-empty snapshot payloads.
#[derive(Default, Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub struct ValidationResult {
 pub ok: bool,
 pub violation_count: u32,
}

impl ValidationResult {
 pub fn ok() -> Self {
 Self {
 ok: true,
 violation_count: 0,
 }
 }

 pub fn violations(count: u32) -> Self {
 Self {
 ok: false,
 violation_count: count,
 }
 }
}

/// Cascade root input — branch identity wrap (Salsa primitive types are not
/// `SalsaStructInDb` directly, hence the wrap).
///
/// `revision` bumps when any fact in the branch changes — Salsa memo cache
/// key. `snapshot_payload` is `BranchSnapshotData` serde_json-encoded for
/// deterministic byte equality.
#[salsa::input]
pub struct CascadeBranch {
 pub branch_id: u64,
 pub revision: u64,
 pub snapshot_payload: Vec<u8>,
}

#[salsa::input]
pub struct SectionInput {
 pub branch_id: u64,
 pub entity_id: u64,
 pub valid_from: u64,
 pub payload: Vec<u8>,
}

#[salsa::input]
pub struct ChangelogEntryInput {
 pub branch_id: u64,
 pub entity_id: u64,
 pub valid_from: u64,
 pub payload: Vec<u8>,
}

#[salsa::input]
pub struct FrozenListInput {
 pub branch_id: u64,
 pub entity_id: u64,
 pub valid_from: u64,
 pub payload: Vec<u8>,
}

#[salsa::db]
pub trait CascadeDb: salsa::Database {
 fn section_decision_status(&self, branch: CascadeBranch) -> ValidationResult;
 fn frozen_list_membership(&self, branch: CascadeBranch) -> ValidationResult;
}

/// Cascade query: ChangelogEntry append → Section.decision_status update.
///
/// Invariant — each `Section` whose `decision_status` is `"Superseded"` must
/// have at least one outbound `CrossRef` (`from_section == section.section_id`)
/// of `ref_kind ∈ {"decision", "impl"}` — the supersession pointer to the
/// superseding section. Mirrors T1 rule 4 (`section_decision_status_transition`)
/// surfaced as a cascade query (per *cascade_query Forge kind*).
#[salsa::tracked]
pub fn section_decision_status<'db>(
 db: &'db dyn CascadeDb,
 branch: CascadeBranch,
) -> ValidationResult {
 let _branch_id = branch.branch_id(db);
 let _revision = branch.revision(db);
 let payload = branch.snapshot_payload(db);
 let snap = match BranchSnapshotData::decode(&payload) {
 Ok(s) => s,
 Err(_) => return ValidationResult::violations(0),
 };
 let mut violation_count: u32 = 0;
 for section in &snap.sections {
 if !section
 .decision_status
 .eq_ignore_ascii_case("superseded")
 {
 continue;
 }
 let has_supersedes_ref = snap.cross_refs.iter().any(|cr| {
 cr.from_section == section.entity_id
  && (cr.ref_kind.eq_ignore_ascii_case("decision")
  || cr.ref_kind.eq_ignore_ascii_case("impl"))
 });
 if !has_supersedes_ref {
 violation_count = violation_count.saturating_add(1);
 }
 }
 if violation_count == 0 {
 ValidationResult::ok()
 } else {
 ValidationResult::violations(violation_count)
 }
}

/// Cascade query: FrozenList membership check (CrossRef cascade).
///
/// Invariants:
/// 1. Each `FrozenList.owner_section` must reference an `entity_id` present
/// in the snapshot's Section set (referential integrity).
/// 2. When the snapshot contains at least one `FrozenList`, at least one
/// `ChangelogEntry` must accompany — membership delta requires changelog
/// attachment per T1 rule 3 (`frozen_list_membership_delta`).
#[salsa::tracked]
pub fn frozen_list_membership<'db>(
 db: &'db dyn CascadeDb,
 branch: CascadeBranch,
) -> ValidationResult {
 let _branch_id = branch.branch_id(db);
 let _revision = branch.revision(db);
 let payload = branch.snapshot_payload(db);
 let snap = match BranchSnapshotData::decode(&payload) {
 Ok(s) => s,
 Err(_) => return ValidationResult::violations(0),
 };
 let mut violation_count: u32 = 0;
 let section_ids: std::collections::BTreeSet<u64> =
 snap.sections.iter().map(|s| s.entity_id).collect();
 for frozen_list in &snap.frozen_lists {
 if !section_ids.contains(&frozen_list.owner_section) {
 violation_count = violation_count.saturating_add(1);
 }
 }
 if !snap.frozen_lists.is_empty() && snap.changelog_entries.is_empty() {
 violation_count = violation_count.saturating_add(snap.frozen_lists.len() as u32);
 }
 if violation_count == 0 {
 ValidationResult::ok()
 } else {
 ValidationResult::violations(violation_count)
 }
}

/// Concrete cascade DB — owns Salsa storage + dispatches `CascadeDb` trait
/// methods to the tracked functions above.
#[salsa::db]
#[derive(Default, Clone)]
pub struct MnemosyneCascadeDb {
 storage: salsa::Storage<Self>,
}

#[salsa::db]
impl salsa::Database for MnemosyneCascadeDb {}

#[salsa::db]
impl CascadeDb for MnemosyneCascadeDb {
 fn section_decision_status(&self, branch: CascadeBranch) -> ValidationResult {
 section_decision_status(self, branch)
 }
 fn frozen_list_membership(&self, branch: CascadeBranch) -> ValidationResult {
 frozen_list_membership(self, branch)
 }
}

#[cfg(test)]
mod tests {
 use super::*;
 use crate::snapshot::BranchSnapshotData;
 use mnemosyne_facts::{ChangelogEntryFact, CrossRefFact, FrozenListFact, SectionFact};

 fn make_branch(db: &MnemosyneCascadeDb, branch_id: u64, snap: &BranchSnapshotData) -> CascadeBranch {
 let payload = snap.encode().expect("encode");
 CascadeBranch::new(db, branch_id, 1, payload)
 }

 #[test]
 fn empty_snapshot_section_decision_status_is_vacuously_ok() {
 let db = MnemosyneCascadeDb::default();
 let branch = make_branch(&db, 0, &BranchSnapshotData::default());
 assert_eq!(section_decision_status(&db, branch), ValidationResult::ok());
 }

 #[test]
 fn empty_snapshot_frozen_list_membership_is_vacuously_ok() {
 let db = MnemosyneCascadeDb::default();
 let branch = make_branch(&db, 0, &BranchSnapshotData::default());
 assert_eq!(frozen_list_membership(&db, branch), ValidationResult::ok());
 }

 #[test]
 fn active_section_passes_decision_status_check() {
 let db = MnemosyneCascadeDb::default();
 let snap = BranchSnapshotData {
 sections: vec![SectionFact {
  branch_id: 1,
  entity_id: 39,
  valid_from: 100,
  doc_path: "docs/DESIGN.md".into(),
  section_id: "39".into(),
  title: "x".into(),
  decision_status: "Active".into(),
 }],
 ..Default::default()
 };
 let branch = make_branch(&db, 1, &snap);
 assert_eq!(section_decision_status(&db, branch), ValidationResult::ok());
 }

 #[test]
 fn superseded_section_with_outbound_decision_ref_passes() {
 let db = MnemosyneCascadeDb::default();
 let snap = BranchSnapshotData {
 sections: vec![SectionFact {
  branch_id: 1,
  entity_id: 15,
  valid_from: 100,
  doc_path: "docs/DESIGN.md".into(),
  section_id: "15".into(),
  title: "old SDK".into(),
  decision_status: "Superseded".into(),
 }],
 cross_refs: vec![CrossRefFact {
  branch_id: 1,
  from_section: 15,
  to_section: 56,
  ref_kind: "decision".into(),
 }],
 ..Default::default()
 };
 let branch = make_branch(&db, 1, &snap);
 assert_eq!(section_decision_status(&db, branch), ValidationResult::ok());
 }

 #[test]
 fn superseded_section_without_outbound_ref_fails() {
 let db = MnemosyneCascadeDb::default();
 let snap = BranchSnapshotData {
 sections: vec![SectionFact {
  branch_id: 1,
  entity_id: 15,
  valid_from: 100,
  doc_path: "docs/DESIGN.md".into(),
  section_id: "15".into(),
  title: "old SDK".into(),
  decision_status: "Superseded".into(),
 }],
 ..Default::default()
 };
 let branch = make_branch(&db, 1, &snap);
 assert_eq!(
 section_decision_status(&db, branch),
 ValidationResult::violations(1)
 );
 }

 #[test]
 fn superseded_section_with_unrelated_crossref_still_fails() {
 let db = MnemosyneCascadeDb::default();
 let snap = BranchSnapshotData {
 sections: vec![SectionFact {
  branch_id: 1,
  entity_id: 15,
  valid_from: 100,
  doc_path: "docs/DESIGN.md".into(),
  section_id: "15".into(),
  title: "old SDK".into(),
  decision_status: "Superseded".into(),
 }],
 cross_refs: vec![CrossRefFact {
  branch_id: 1,
  from_section: 99,
  to_section: 15,
  ref_kind: "decision".into(),
 }],
 ..Default::default()
 };
 let branch = make_branch(&db, 1, &snap);
 assert_eq!(
 section_decision_status(&db, branch),
 ValidationResult::violations(1)
 );
 }

 #[test]
 fn frozen_list_with_owner_in_snapshot_and_changelog_passes() {
 let db = MnemosyneCascadeDb::default();
 let snap = BranchSnapshotData {
 sections: vec![SectionFact {
  branch_id: 1,
  entity_id: 39,
  valid_from: 100,
  doc_path: "docs/DESIGN.md".into(),
  section_id: "39".into(),
  title: "x".into(),
  decision_status: "Active".into(),
 }],
 changelog_entries: vec![ChangelogEntryFact {
  branch_id: 1,
  entity_id: 60,
  valid_from: 100,
  round_number: 60,
  summary: "round 60 ratify".into(),
  appended_at: 2026_05_03,
 }],
 frozen_lists: vec![FrozenListFact {
  branch_id: 1,
  entity_id: 1000,
  valid_from: 100,
  owner_section: 39,
  frozen_round: 60,
  kind: "release_lock".into(),
 }],
 ..Default::default()
 };
 let branch = make_branch(&db, 1, &snap);
 assert_eq!(frozen_list_membership(&db, branch), ValidationResult::ok());
 }

 #[test]
 fn frozen_list_with_dangling_owner_fails() {
 let db = MnemosyneCascadeDb::default();
 let snap = BranchSnapshotData {
 changelog_entries: vec![ChangelogEntryFact {
  branch_id: 1,
  entity_id: 60,
  valid_from: 100,
  round_number: 60,
  summary: "x".into(),
  appended_at: 100,
 }],
 frozen_lists: vec![FrozenListFact {
  branch_id: 1,
  entity_id: 1000,
  valid_from: 100,
  owner_section: 99,
  frozen_round: 60,
  kind: "release_lock".into(),
 }],
 ..Default::default()
 };
 let branch = make_branch(&db, 1, &snap);
 assert_eq!(
 frozen_list_membership(&db, branch),
 ValidationResult::violations(1)
 );
 }

 #[test]
 fn frozen_list_without_changelog_attachment_fails() {
 let db = MnemosyneCascadeDb::default();
 let snap = BranchSnapshotData {
 sections: vec![SectionFact {
  branch_id: 1,
  entity_id: 39,
  valid_from: 100,
  doc_path: "docs/DESIGN.md".into(),
  section_id: "39".into(),
  title: "x".into(),
  decision_status: "Active".into(),
 }],
 frozen_lists: vec![FrozenListFact {
  branch_id: 1,
  entity_id: 1000,
  valid_from: 100,
  owner_section: 39,
  frozen_round: 60,
  kind: "release_lock".into(),
 }],
 ..Default::default()
 };
 let branch = make_branch(&db, 1, &snap);
 assert_eq!(
 frozen_list_membership(&db, branch),
 ValidationResult::violations(1)
 );
 }

 #[test]
 fn tracked_function_memoize_stability() {
 let db = MnemosyneCascadeDb::default();
 let snap = BranchSnapshotData {
 sections: vec![SectionFact {
  branch_id: 1,
  entity_id: 39,
  valid_from: 100,
  doc_path: "x".into(),
  section_id: "39".into(),
  title: "x".into(),
  decision_status: "Active".into(),
 }],
 ..Default::default()
 };
 let branch = make_branch(&db, 1, &snap);
 let a = section_decision_status(&db, branch);
 let b = section_decision_status(&db, branch);
 assert_eq!(a, b);
 let c = frozen_list_membership(&db, branch);
 let d = frozen_list_membership(&db, branch);
 assert_eq!(c, d);
 }

 #[test]
 fn cascade_db_trait_dispatch() {
 let db = MnemosyneCascadeDb::default();
 let snap = BranchSnapshotData::default();
 let branch = make_branch(&db, 7, &snap);
 let r1 = db.section_decision_status(branch);
 let r2 = db.frozen_list_membership(branch);
 assert_eq!(r1, ValidationResult::ok());
 assert_eq!(r2, ValidationResult::ok());
 }

 #[test]
 fn malformed_payload_returns_default_violations() {
 let db = MnemosyneCascadeDb::default();
 let branch = CascadeBranch::new(&db, 1, 1, vec![0xFF, 0xFE, 0xFD]);
 let r = section_decision_status(&db, branch);
 assert!(!r.ok);
 }
}
