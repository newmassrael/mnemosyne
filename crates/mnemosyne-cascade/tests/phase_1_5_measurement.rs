//! Phase 1.5 cascade gate — measurement source for this layer.
//!
//! DESIGN.md §43 *Phase 1.5 cascade gate measurement source carry* (Round 22
//! Tier 5 #1 Salsa-lock handle — result-stable carry; Round 36 paradigm-decision source.
//! explicit) — 50K asset / 5K fact / 100 branch synthetic over:
//!
//! (i) memory footprint < 1GB
//! (ii) invalidation polynomial (size band 0 / 1-9 / 10-99 / 100+ distribution)
//! (iii) cascade preview ±0 accuracy (eager resolution enforced over lazy-semantics comparison)
//!
//! When the 3 gates do not pass, the Salsa → Differential Dataflow / Adapton / direct-implementation
//! library re-selection — Phase 2 commit deferred.
//!
//! Round 81 = registers only the *baseline scaffold* of this measurement source — full
//! 50K-scale measurement runs in the release-mode bench harness (Phase 1.5 measurement
//! spike-time carry). This file's small-scale fixture:
//!
//! - 10 branches × (50 Section + 50 ChangelogEntry + 25 FrozenList + 50 CrossRef)
//! - = 1,750 fact entries / 100 branches × 500 facts = 1/29 of the 50K full-scale workload
//!
//! cargo test is deterministic and completes in < 1s (CI scope); actual 50K measurement
//! separate round (Phase 1.5 entry time measurement-pending lock #1 carry).

use mnemosyne_cascade::{
 frozen_list_membership, section_decision_status, BranchSnapshotData, CascadeBranch,
 MnemosyneCascadeDb, ValidationResult,
};
use mnemosyne_core::{ChangelogEntryFact, CrossRefFact, FrozenListFact, SectionFact};

const SECTIONS_PER_BRANCH: usize = 50;
const CHANGELOG_PER_BRANCH: usize = 50;
const FROZEN_LISTS_PER_BRANCH: usize = 25;
const CROSS_REFS_PER_BRANCH: usize = 50;
const BRANCH_COUNT: usize = 10;

/// Synthetic per-branch fixture — deterministic generator. branch_id seeds the
/// entity_id allocation so cross-branch facts stay disjoint.
fn synthetic_branch_snapshot(branch_id: u64) -> BranchSnapshotData {
 let mut sections = Vec::with_capacity(SECTIONS_PER_BRANCH);
 let mut changelog_entries = Vec::with_capacity(CHANGELOG_PER_BRANCH);
 let mut frozen_lists = Vec::with_capacity(FROZEN_LISTS_PER_BRANCH);
 let mut cross_refs = Vec::with_capacity(CROSS_REFS_PER_BRANCH);

 let base = branch_id * 1_000_000;
 for i in 0..SECTIONS_PER_BRANCH {
 let entity_id = base + i as u64;
 sections.push(SectionFact {
 branch_id,
 entity_id,
 valid_from: 100,
 doc_path: format!("docs/synthetic-{}.md", branch_id),
 section_id: format!("{}.{}", branch_id, i),
 title: format!("Section {} of branch {}", i, branch_id),
 decision_status: "Active".into(),
 });
 }
 for i in 0..CHANGELOG_PER_BRANCH {
 let entity_id = base + 1000 + i as u64;
 changelog_entries.push(ChangelogEntryFact {
 branch_id,
 entity_id,
 valid_from: 100 + i as u64,
 round_number: i as u64,
 summary: format!("synthetic round {} branch {}", i, branch_id),
 appended_at: 2026_05_03 + i as u64,
 });
 }
 for i in 0..FROZEN_LISTS_PER_BRANCH {
 let entity_id = base + 2000 + i as u64;
 let owner_idx = i % SECTIONS_PER_BRANCH;
 let owner_section = base + owner_idx as u64;
 frozen_lists.push(FrozenListFact {
 branch_id,
 entity_id,
 valid_from: 100,
 owner_section,
 frozen_round: i as u64,
 kind: "release_lock".into(),
 });
 }
 for i in 0..CROSS_REFS_PER_BRANCH {
 let from_idx = i % SECTIONS_PER_BRANCH;
 let to_idx = (i + 1) % SECTIONS_PER_BRANCH;
 cross_refs.push(CrossRefFact {
 branch_id,
 from_section: base + from_idx as u64,
 to_section: base + to_idx as u64,
 ref_kind: "decision".into(),
 });
 }

 BranchSnapshotData {
 sections,
 changelog_entries,
 frozen_lists,
 cross_refs,
 }
}

fn classify_size_band(n: usize) -> &'static str {
 match n {
 0 => "0",
 1..=9 => "1-9",
 10..=99 => "10-99",
 _ => "100+",
 }
}

/// Gate (i) — memory footprint per snapshot encoding.
///
/// Phase 1.5 target: < 1GB at 50K asset. Baseline (small fixture) ratio
/// projected — assert per-branch payload < 100KB so 100-branch full scale
/// stays well under 10MB encoding overhead (Salsa Storage allocations are
/// dominant in real measurement, not measured here).
#[test]
fn gate_i_memory_footprint_per_branch_under_band() {
 for branch_id in 0..BRANCH_COUNT as u64 {
 let snap = synthetic_branch_snapshot(branch_id);
 let payload = snap.encode().expect("encode");
 // Rough band — full 50K target: < 1GB. Per-branch baseline < 100KB.
 assert!(
 payload.len() < 100_000,
 "branch {} payload exceeded baseline band: {} bytes",
 branch_id,
 payload.len()
 );
 }
}

#[test]
fn gate_i_aggregate_memory_footprint_under_baseline() {
 let total: usize = (0..BRANCH_COUNT as u64)
 .map(|b| {
 synthetic_branch_snapshot(b)
  .encode()
  .expect("encode")
  .len()
 })
 .sum();
 // Baseline scaffold — full 50K scale would be ~30x; gate assertion lives
 // in Phase 1.5 release-mode bench, not here.
 assert!(
 total < 1_000_000,
 "aggregate payload exceeded 1MB baseline: {} bytes",
 total
 );
}

/// Gate (ii) — invalidation polynomial size band per single-fact mutation.
///
/// Salsa memoization size-band distribution (0 / 1-9 / 10-99 / 100+). Baseline measurement.
/// — each cascade-query invocation against this fixture costs 1 unit (memoize hit/miss). Phase 1.5
/// Release-mode actual band-distribution output (count of derived facts impacted by the mutation,
/// counter source).
#[test]
fn gate_ii_invalidation_size_band_baseline() {
 let db = MnemosyneCascadeDb::default();
 for branch_id in 0..BRANCH_COUNT as u64 {
 let snap = synthetic_branch_snapshot(branch_id);
 let payload = snap.encode().expect("encode");
 let branch = CascadeBranch::new(&db, branch_id, 1, payload);
 let r = section_decision_status(&db, branch);
 let band = classify_size_band(r.violation_count as usize);
 assert!(
 ["0", "1-9", "10-99", "100+"].contains(&band),
 "unexpected band: {}",
 band
 );
 // Active-only synthetic — 0 violations expected.
 assert_eq!(band, "0", "branch {} expected band 0", branch_id);
 }
}

/// Gate (iii) — cascade preview ±0 accuracy (deterministic re-run).
///
/// same input → same output. Salsa memoize-stability baseline validation —
/// the essence of *eager-resolution-over-lazy semantics comparison* (deterministic
/// floor-measurement of the preview output.
#[test]
fn gate_iii_cascade_preview_deterministic_byte_equal() {
 let db = MnemosyneCascadeDb::default();
 for branch_id in 0..BRANCH_COUNT as u64 {
 let snap = synthetic_branch_snapshot(branch_id);
 let payload = snap.encode().expect("encode");
 let branch = CascadeBranch::new(&db, branch_id, 1, payload);
 let a1 = section_decision_status(&db, branch);
 let a2 = section_decision_status(&db, branch);
 assert_eq!(a1, a2, "section_decision_status drifted on re-run");
 let b1 = frozen_list_membership(&db, branch);
 let b2 = frozen_list_membership(&db, branch);
 assert_eq!(b1, b2, "frozen_list_membership drifted on re-run");
 }
}

/// Gate (iii.b) — cross-DB determinism. Same fixture, different DB instances
/// On reconstruction yields the same result. Validates that Salsa storage state has no impact on the result.
#[test]
fn gate_iii_cross_db_deterministic() {
 for branch_id in 0..BRANCH_COUNT as u64 {
 let snap = synthetic_branch_snapshot(branch_id);
 let payload = snap.encode().expect("encode");

 let db1 = MnemosyneCascadeDb::default();
 let branch1 = CascadeBranch::new(&db1, branch_id, 1, payload.clone());
 let r1 = section_decision_status(&db1, branch1);

 let db2 = MnemosyneCascadeDb::default();
 let branch2 = CascadeBranch::new(&db2, branch_id, 1, payload);
 let r2 = section_decision_status(&db2, branch2);

 assert_eq!(r1, r2, "branch {} cross-db drift", branch_id);
 }
}

/// Synthetic violation injection — flips one Section to Superseded without
/// outbound CrossRef. The cascade query detects exactly 1 violation.
#[test]
fn gate_iii_violation_injection_round_trip_accurate() {
 let db = MnemosyneCascadeDb::default();
 let mut snap = synthetic_branch_snapshot(0);
 snap.sections[0].decision_status = "Superseded".into();
 let target = snap.sections[0].entity_id;
 snap.cross_refs.retain(|cr| cr.from_section != target);

 let payload = snap.encode().expect("encode");
 let branch = CascadeBranch::new(&db, 0, 1, payload);
 let r = section_decision_status(&db, branch);
 assert_eq!(r, ValidationResult::violations(1));
}

/// Aggregate fact count — full 50K scale projection sanity check.
#[test]
fn fixture_total_fact_count_matches_projection() {
 let total: usize = (0..BRANCH_COUNT as u64)
 .map(|b| synthetic_branch_snapshot(b).fact_count())
 .sum();
 let expected = BRANCH_COUNT
 * (SECTIONS_PER_BRANCH
 + CHANGELOG_PER_BRANCH
 + FROZEN_LISTS_PER_BRANCH
 + CROSS_REFS_PER_BRANCH);
 assert_eq!(total, expected);
 assert_eq!(total, 1_750);
}
