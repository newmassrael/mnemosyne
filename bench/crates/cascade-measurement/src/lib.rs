//! Phase 1.5 cascade gate full-scale measurement (DESIGN.md §43, Round 22 Tier 5
//! #1 Salsa-lock measurement: pending-lock carry source.
//!
//! 100 branches × (500 Section + 50 ChangelogEntry + 100 FrozenList + 500
//! CrossRef) = 50K asset / 5K fact / 10K FrozenList / 50K CrossRef = ~115K
//! total facts. Phase -1A `bench/crates/direct-impl/` measurement protocol
//! equivalent (ChaCha20Rng deterministic seed, hdrhistogram percentile, sample 1000+).
//!
//! ## 3 gates
//!
//! (i) -all- footprint < 1GB — encoded snapshot bytes + Salsa storage
//! (process RSS proxy)
//! (ii) invalidation polynomial size band — single-fact mutation in derived
//! fact-count distribution (band 0 / 1-9 / 10-99 / 100+)
//! (iii) cascade preview ±0 (also formal) — 1000-iter deterministic re-run + cross-DB
//! byte-equality
//!
//! When the 3 gates do not pass, switch from Salsa to Differential Dataflow / Adapton / direct implementation
//! Re-formalization deferred to Phase 2 commit.
//!
//! ## Limitations (carry)
//!
//! Gate (ii)'s "invalidation polynomial" — the current Salsa runtime API at *single
//! tracked query per branch* (section_decision_status / frozen_list_membership)
//! only exposed — single-fact mutation defaults to *branch-wide full re-execution*,
//! real polynomial measurement — Phase 1.5+'s fine-grained dependency-tracking layer
//! (per-Section / per-CrossRef tracked sub-queries) — thereafter narrowed in scope. This measure
//! in size band - *violation_count distribution after mutation*'s proxy.

use mnemosyne_cascade::{
 build_branch_index, frozen_list_membership, section_decision_status,
 section_decision_status_aggregated, BranchSnapshotData, CascadeBranch, CrossRefRecord,
 FineCascadeDb, MnemosyneCascadeDb, SectionRecord,
};
use mnemosyne_core::{ChangelogEntryFact, CrossRefFact, FrozenListFact, SectionFact};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use salsa::Setter;
use serde::Serialize;

/// Full-scale measurement parameters (DESIGN.md §43 measurement source carry).
pub const SECTIONS_PER_BRANCH: usize = 500;
pub const CHANGELOG_PER_BRANCH: usize = 50;
pub const FROZEN_LISTS_PER_BRANCH: usize = 100;
pub const CROSS_REFS_PER_BRANCH: usize = 500;
pub const BRANCH_COUNT: usize = 100;

/// Phase -1A bench/ pattern equivalent deterministic seed (Round 30 / Round 81 baseline
/// pattern carry — within-run reproducibility).
pub const FIXTURE_SEED: u64 = 0xC0FFEE_C0FFEE;

/// Synthetic per-branch fixture — deterministic ChaCha20Rng generator. Each
/// branch gets a derived sub-seed from the master seed for cross-branch
/// independence + within-run reproducibility.
pub fn synthetic_branch_snapshot(branch_id: u64) -> BranchSnapshotData {
 let mut rng = ChaCha20Rng::seed_from_u64(FIXTURE_SEED.wrapping_add(branch_id));
 let base = branch_id * 1_000_000;

 let mut sections = Vec::with_capacity(SECTIONS_PER_BRANCH);
 for i in 0..SECTIONS_PER_BRANCH {
 // ~5% Superseded; ~95% Active. Superseded sections need outbound
 // CrossRef of decision/impl kind to pass cascade.
 let superseded = rng.gen_bool(0.05);
 sections.push(SectionFact {
 branch_id,
 entity_id: base + i as u64,
 valid_from: 100,
 doc_path: format!("docs/synthetic-{}.md", branch_id),
 section_id: format!("{}.{}", branch_id, i),
 title: format!("Section {} of branch {}", i, branch_id),
 decision_status: if superseded { "Superseded" } else { "Active" }.into(),
 });
 }

 let mut changelog_entries = Vec::with_capacity(CHANGELOG_PER_BRANCH);
 for i in 0..CHANGELOG_PER_BRANCH {
 changelog_entries.push(ChangelogEntryFact {
 branch_id,
 entity_id: base + 1_000_000 + i as u64,
 valid_from: 100 + i as u64,
 round_number: i as u64,
 summary: format!("synthetic round {} branch {}", i, branch_id),
 appended_at: 2026_05_03 + i as u64,
 });
 }

 let mut frozen_lists = Vec::with_capacity(FROZEN_LISTS_PER_BRANCH);
 for i in 0..FROZEN_LISTS_PER_BRANCH {
 // owner_section refers to a real Section (referential integrity).
 let owner_idx = rng.gen_range(0..SECTIONS_PER_BRANCH);
 frozen_lists.push(FrozenListFact {
 branch_id,
 entity_id: base + 2_000_000 + i as u64,
 valid_from: 100,
 owner_section: base + owner_idx as u64,
 frozen_round: i as u64,
 kind: "release_lock".into(),
 });
 }

 let mut cross_refs = Vec::with_capacity(CROSS_REFS_PER_BRANCH);
 // First, ensure every Superseded section gets at least one outbound
 // decision/impl CrossRef (cascade rule satisfaction). Then fill remaining
 // with random outbound refs.
 let superseded_idxs: Vec<usize> = sections
 .iter()
 .enumerate()
 .filter(|(_, s)| s.decision_status == "Superseded")
 .map(|(i, _)| i)
 .collect();
 for &from_idx in &superseded_idxs {
 let to_idx = (from_idx + 1) % SECTIONS_PER_BRANCH;
 cross_refs.push(CrossRefFact {
 branch_id,
 from_section: base + from_idx as u64,
 to_section: base + to_idx as u64,
 ref_kind: "decision".into(),
 });
 }
 while cross_refs.len() < CROSS_REFS_PER_BRANCH {
 let from_idx = rng.gen_range(0..SECTIONS_PER_BRANCH);
 let to_idx = rng.gen_range(0..SECTIONS_PER_BRANCH);
 cross_refs.push(CrossRefFact {
 branch_id,
 from_section: base + from_idx as u64,
 to_section: base + to_idx as u64,
 ref_kind: "decision".into(),
 });
 }
 cross_refs.truncate(CROSS_REFS_PER_BRANCH);

 BranchSnapshotData {
 sections,
 changelog_entries,
 frozen_lists,
 cross_refs,
 }
}

pub fn classify_size_band(n: usize) -> &'static str {
 match n {
 0 => "0",
 1..=9 => "1-9",
 10..=99 => "10-99",
 _ => "100+",
 }
}

#[derive(Debug, Clone, Serialize)]
pub struct GateIResult {
 pub branch_count: usize,
 pub total_encoded_bytes: usize,
 pub min_branch_bytes: usize,
 pub max_branch_bytes: usize,
 pub mean_branch_bytes: usize,
 pub aggregate_under_1gb: bool,
 pub salsa_db_load_ok: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct GateIIResult {
 pub branch_count: usize,
 pub band_0: usize,
 pub band_1_9: usize,
 pub band_10_99: usize,
 pub band_100_plus: usize,
 /// After single-fact mutation (flip one Section to Superseded without
 /// outbound ref), violation_count delta should be exactly +1 per branch.
 pub mutation_delta_one_count: usize,
 pub mutation_delta_other_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct GateIIIResult {
 pub branch_count: usize,
 pub iter_per_branch: usize,
 pub deterministic_pass: usize,
 pub cross_db_pass: usize,
 pub eager_lazy_byte_equal_pass: usize,
}

/// Fine-grained Gate (ii) — *real* invalidation polynomial size band, measured
/// against the per-record Salsa input layer ([`FineCascadeDb`]). Unlike
/// `GateIIResult` which tracks `violation_count` as a proxy, this counts
/// actual tracked function body executions after a single fact mutation
/// (per-DB Salsa `WillExecute` event hook). Round 92 limitation carry.
///
/// Measurement protocol per branch (single Active→Superseded mutation, same as
/// `measure_gate_ii`):
///
/// 1. Build per-record `BranchIndex` Salsa inputs.
/// 2. Run aggregator → cache populated. Reset `exec_counter`.
/// 3. Mutate one Section's `decision_status` from "Active" to "Superseded".
/// 4. Re-run aggregator. Read `exec_counter` → number of tracked function
/// body executions triggered by the single mutation.
/// 5. Bucket the count into size band (0 / 1-9 / 10-99 / 100+).
#[derive(Debug, Clone, Serialize)]
pub struct GateIIFineGrainedResult {
 pub branch_count: usize,
 /// Per-branch `exec_counter` distribution after single-fact mutation.
 pub band_0: usize,
 pub band_1_9: usize,
 pub band_10_99: usize,
 pub band_100_plus: usize,
 /// Min/max/mean exec_counter across all branches.
 pub min_invocations: usize,
 pub max_invocations: usize,
 pub mean_invocations: usize,
 /// Aggregator + sub-query bodies in the cache after a clean run (baseline
 /// for understanding mutation deltas).
 pub baseline_body_count_per_branch: usize,
 /// Linearity check — sub-queries per Section + 1 aggregator. With 500
 /// sections per branch the baseline is 501. Mutation invalidates ≤ this.
 pub mutation_within_polynomial_bound: bool,
 /// Round 100 — `frozen_list_membership_aggregated` baseline body count after
 /// the `section_by_entity_id` layer was inserted. With 100 FrozenLists per
 /// branch + the new layer the baseline grows by FROZEN_LISTS_PER_BRANCH
 /// (one `section_by_entity_id` sub-query per unique owner_section).
 /// Round 101 carry — post-refactor this baseline also includes the
 /// `changelog_by_round_number` per-FrozenList sub-queries (one per unique
 /// `frozen_round` queried by `frozen_list_changelog_attachment`).
 pub frozen_list_baseline_body_count_per_branch: usize,
 /// Round 100 — exec_counter after a single `FrozenListRecord.owner_section`
 /// mutation that flips the resolution from a present section to an unknown
 /// id. Bounded by the per-FrozenList sub-query + 1 layer body + 1
 /// aggregator = 3 invocations regardless of branch size.
 pub frozen_list_owner_mutation_min_invocations: usize,
 pub frozen_list_owner_mutation_max_invocations: usize,
 pub frozen_list_owner_mutation_mean_invocations: usize,
 /// Round 101 — baseline body count for the changelog pipeline. After the
 /// `frozen_list_changelog_attachment` per-FrozenList round-resolution
 /// After the refactor, the changelog layer is integrated into
 /// `frozen_list_membership_aggregated`. This field captures the same
 /// post-refactor baseline as `frozen_list_baseline_body_count_per_branch`
 /// (they coincide because the two pipelines are now coupled), surfaced
 /// separately for changelog-layer audit-trail clarity.
 pub changelog_baseline_body_count_per_branch: usize,
 /// Round 101 — exec_counter after a single
 /// `ChangelogRecord.round_number` mutation that breaks one resolution
 /// (target round → `u64::MAX`). Bounded by the number of unique
 /// `frozen_round` keys queried (every `changelog_by_round_number` cache
 /// slot reads every entry's `round_number` via iteration → all unique
 /// round caches re-run; Salsa backdating then gates downstream
 /// propagation). Polynomial bound: `≤ unique_frozen_rounds + small constant`.
 pub changelog_round_mutation_min_invocations: usize,
 pub changelog_round_mutation_max_invocations: usize,
 pub changelog_round_mutation_mean_invocations: usize,
}

/// Incremental BranchIndex mutation impact (Round 94 (c)). Captures the
/// invalidation cost of `BranchIndex` Vec field replacement (sections /
/// cross_refs) — the price of the per-section pre-indexed layer (Round 94 (b))
/// when the underlying Vec itself changes rather than an inner record's field.
///
/// ## Why measure this separately from `GateIIFineGrainedResult`
///
/// `measure_gate_ii_fine_grained` mutates a single `SectionRecord.decision_status`
/// — the BranchIndex Vec is unchanged, so per-section pre-indexed caches stay
/// valid except for the target. By contrast, this measurement uses
/// `BranchIndex.set_sections` / `set_cross_refs`, which replace the entire
/// Vec — every per-section `outbound_crossrefs_by_section` cache (and every
/// section sub-query that reads `branch_index.sections`) is invalidated.
/// Salsa "backdating" still gates downstream consumer re-runs, so the
/// effective fan-out depends on how many per-section result Vecs actually
/// change byte-equal-wise.
#[derive(Debug, Clone, Serialize)]
pub struct GateIIIncrementalResult {
 pub branch_count: usize,
 /// Single-section-add (push to BranchIndex.sections). The new section is
 /// Active, so `section_decision_violation` for it short-circuits without
 /// reading `outbound_crossrefs_by_section`.
 pub section_add_min: usize,
 pub section_add_max: usize,
 pub section_add_mean: usize,
 /// Single-CrossRef-remove (drop one CrossRef whose `from_section` is a
 /// Superseded section's outbound decision/impl). All
 /// `outbound_crossrefs_by_section` caches invalidate, but only the
 /// affected from_section's consumer sub-query and the aggregator re-run
 /// (per-section result Vecs unchanged elsewhere → Salsa backdating).
 pub crossref_remove_min: usize,
 pub crossref_remove_max: usize,
 pub crossref_remove_mean: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct MeasurementReport {
 pub fixture_seed: u64,
 pub scale: ScaleParams,
 pub gate_i: GateIResult,
 pub gate_ii: GateIIResult,
 pub gate_ii_fine_grained: GateIIFineGrainedResult,
 pub gate_ii_incremental: GateIIIncrementalResult,
 pub gate_iii: GateIIIResult,
 pub all_gates_pass: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScaleParams {
 pub branches: usize,
 pub sections_per_branch: usize,
 pub changelog_per_branch: usize,
 pub frozen_lists_per_branch: usize,
 pub cross_refs_per_branch: usize,
 pub total_facts: usize,
}

impl ScaleParams {
 pub fn full() -> Self {
 Self {
 branches: BRANCH_COUNT,
 sections_per_branch: SECTIONS_PER_BRANCH,
 changelog_per_branch: CHANGELOG_PER_BRANCH,
 frozen_lists_per_branch: FROZEN_LISTS_PER_BRANCH,
 cross_refs_per_branch: CROSS_REFS_PER_BRANCH,
 total_facts: BRANCH_COUNT
  * (SECTIONS_PER_BRANCH
  + CHANGELOG_PER_BRANCH
  + FROZEN_LISTS_PER_BRANCH
  + CROSS_REFS_PER_BRANCH),
 }
 }
}

/// Gate (i) — memory footprint. Measure per-branch encoded snapshot bytes +
/// aggregate. Load all branches into a single MnemosyneCascadeDb and assert no
/// load error. Process RSS measurement is omitted at this layer (call site
/// captures via `getrusage` or `sysinfo` if needed).
pub fn measure_gate_i(branches: usize) -> anyhow::Result<GateIResult> {
 let mut sizes = Vec::with_capacity(branches);
 let mut total: usize = 0;
 let db = MnemosyneCascadeDb::default();
 let mut load_ok = true;
 for branch_id in 0..branches as u64 {
 let snap = synthetic_branch_snapshot(branch_id);
 let payload = snap.encode()?;
 sizes.push(payload.len());
 total += payload.len();
 let _branch = CascadeBranch::new(&db, branch_id, 1, payload);
 }
 // Trigger query execution to materialize Salsa storage allocations.
 for branch_id in 0..branches as u64 {
 let snap = synthetic_branch_snapshot(branch_id);
 let payload = snap.encode()?;
 let branch = CascadeBranch::new(&db, branch_id, 1, payload);
 let r1 = section_decision_status(&db, branch);
 let r2 = frozen_list_membership(&db, branch);
 if !(r1.ok || r1.violation_count > 0) || !(r2.ok || r2.violation_count > 0) {
 load_ok = false;
 }
 }
 let min_bytes = *sizes.iter().min().unwrap_or(&0);
 let max_bytes = *sizes.iter().max().unwrap_or(&0);
 let mean_bytes = if sizes.is_empty() { 0 } else { total / sizes.len() };
 Ok(GateIResult {
 branch_count: branches,
 total_encoded_bytes: total,
 min_branch_bytes: min_bytes,
 max_branch_bytes: max_bytes,
 mean_branch_bytes: mean_bytes,
 aggregate_under_1gb: total < 1_000_000_000,
 salsa_db_load_ok: load_ok,
 })
}

/// Gate (ii) — invalidation size band. For each branch:
/// 1. Baseline query → record violation_count_0.
/// 2. Mutate snapshot — flip one Active section to Superseded, drop outbound
/// decision-kind CrossRefs from that section (synthetic violation
/// injection).
/// 3. Re-query (fresh CascadeBranch with bumped revision) → record
/// violation_count_1.
/// 4. Bucket the count into size band. Verify delta == +1 (single mutation
/// should yield exactly one new violation under cascade rule).
pub fn measure_gate_ii(branches: usize) -> anyhow::Result<GateIIResult> {
 let mut band_0 = 0;
 let mut band_1_9 = 0;
 let mut band_10_99 = 0;
 let mut band_100_plus = 0;
 let mut delta_one = 0;
 let mut delta_other = 0;

 for branch_id in 0..branches as u64 {
 let db = MnemosyneCascadeDb::default();
 let mut snap = synthetic_branch_snapshot(branch_id);
 let baseline_payload = snap.encode()?;
 let baseline_branch = CascadeBranch::new(&db, branch_id, 1, baseline_payload);
 let r0 = section_decision_status(&db, baseline_branch);

 // Mutation: pick the first Active section, flip to Superseded, remove
 // outbound decision/impl CrossRefs from that section.
 let target_idx = snap
 .sections
 .iter()
 .position(|s| s.decision_status == "Active")
 .unwrap_or(0);
 let target_id = snap.sections[target_idx].entity_id;
 snap.sections[target_idx].decision_status = "Superseded".into();
 snap.cross_refs.retain(|cr| {
 !(cr.from_section == target_id
  && (cr.ref_kind.eq_ignore_ascii_case("decision")
  || cr.ref_kind.eq_ignore_ascii_case("impl")))
 });

 let mutated_payload = snap.encode()?;
 let mutated_branch = CascadeBranch::new(&db, branch_id, 2, mutated_payload);
 let r1 = section_decision_status(&db, mutated_branch);

 let band_count = r1.violation_count as usize;
 match classify_size_band(band_count) {
 "0" => band_0 += 1,
 "1-9" => band_1_9 += 1,
 "10-99" => band_10_99 += 1,
 _ => band_100_plus += 1,
 }
 if r1.violation_count == r0.violation_count + 1 {
 delta_one += 1;
 } else {
 delta_other += 1;
 }
 }

 Ok(GateIIResult {
 branch_count: branches,
 band_0,
 band_1_9,
 band_10_99,
 band_100_plus,
 mutation_delta_one_count: delta_one,
 mutation_delta_other_count: delta_other,
 })
}

/// Gate (iii) — cascade preview ±0 formal-also. Three sub-checks:
/// (a) deterministic — same db, same branch, run query 1000 times, all
/// results byte-equal.
/// (b) cross-db — same fixture, two distinct MnemosyneCascadeDb instances,
/// same result.
/// (c) eager-lazy byte-equal — Salsa default is lazy memoization; manually
/// force eager run by re-creating the branch + bumping revision and
/// comparing — equivalent semantics carry.
pub fn measure_gate_iii(branches: usize, iter_per_branch: usize) -> anyhow::Result<GateIIIResult> {
 let mut det_pass = 0;
 let mut cross_pass = 0;
 let mut eager_lazy_pass = 0;
 for branch_id in 0..branches as u64 {
 let snap = synthetic_branch_snapshot(branch_id);
 let payload = snap.encode()?;

 // (a) deterministic
 {
 let db = MnemosyneCascadeDb::default();
 let branch = CascadeBranch::new(&db, branch_id, 1, payload.clone());
 let baseline = section_decision_status(&db, branch);
 let mut all_match = true;
 for _ in 0..iter_per_branch {
  let r = section_decision_status(&db, branch);
  if r != baseline {
  all_match = false;
  break;
  }
 }
 if all_match {
  det_pass += 1;
 }
 }

 // (b) cross-db
 {
 let db1 = MnemosyneCascadeDb::default();
 let b1 = CascadeBranch::new(&db1, branch_id, 1, payload.clone());
 let r1 = section_decision_status(&db1, b1);

 let db2 = MnemosyneCascadeDb::default();
 let b2 = CascadeBranch::new(&db2, branch_id, 1, payload.clone());
 let r2 = section_decision_status(&db2, b2);

 if r1 == r2 {
  cross_pass += 1;
 }
 }

 // (c) eager-lazy byte-equal
 {
 let db = MnemosyneCascadeDb::default();
 let lazy_branch = CascadeBranch::new(&db, branch_id, 1, payload.clone());
 let lazy_result = section_decision_status(&db, lazy_branch);
 let eager_branch = CascadeBranch::new(&db, branch_id, 2, payload.clone());
 let eager_result = section_decision_status(&db, eager_branch);
 if lazy_result == eager_result {
  eager_lazy_pass += 1;
 }
 }
 }
 Ok(GateIIIResult {
 branch_count: branches,
 iter_per_branch,
 deterministic_pass: det_pass,
 cross_db_pass: cross_pass,
 eager_lazy_byte_equal_pass: eager_lazy_pass,
 })
}

/// Fine-grained Gate (ii) measurement — counts actual Salsa tracked function
/// body executions (cache misses) after a single Active→Superseded mutation,
/// using the per-record `FineCascadeDb` + `WillExecute` event hook. Round 92
/// limitation carry.
pub fn measure_gate_ii_fine_grained(branches: usize) -> anyhow::Result<GateIIFineGrainedResult> {
 use mnemosyne_cascade::frozen_list_membership_aggregated;

 let mut band_0 = 0;
 let mut band_1_9 = 0;
 let mut band_10_99 = 0;
 let mut band_100_plus = 0;
 let mut all_invocations: Vec<usize> = Vec::with_capacity(branches);
 let mut baseline_body_count_sample: usize = 0;
 let mut within_bound = true;
 // Round 100 — frozen-list pipeline measurement, captured alongside the
 // section pipeline so the new `section_by_entity_id` layer's impact on
 // baseline + mutation cost surfaces in the standard report.
 let mut frozen_list_baseline_sample: usize = 0;
 let mut frozen_list_mutation_invocations: Vec<usize> = Vec::with_capacity(branches);
 // Round 101 — changelog pipeline measurement. The baseline coincides with
 // the frozen-list baseline post-refactor (the two pipelines are coupled
 // through `frozen_list_changelog_attachment`); the mutation captures the
 // single ChangelogEntry.round_number invalidation cost.
 let mut changelog_baseline_sample: usize = 0;
 let mut changelog_round_mutation_invocations: Vec<usize> = Vec::with_capacity(branches);

 for branch_id in 0..branches as u64 {
 let mut db = FineCascadeDb::new();
 let snap = synthetic_branch_snapshot(branch_id);
 let idx = build_branch_index(
 &db,
 branch_id,
 &snap.sections,
 &snap.cross_refs,
 &snap.frozen_lists,
 &snap.changelog_entries,
 );

 // Baseline run — populates cache. Counter captures total body
 // executions (sub-queries + aggregator) for the initial computation.
 db.reset_exec_counter();
 let _r0 = section_decision_status_aggregated(&db, idx);
 let baseline = db.exec_counter();
 if branch_id == 0 {
 baseline_body_count_sample = baseline;
 }

 // Round 100 — frozen-list pipeline baseline. Run on the same DB so the
 // section pipeline's cache state is irrelevant; the counter is reset
 // at each step. Round 101 — post-refactor this baseline also includes
 // the `changelog_by_round_number` sub-queries (one per unique
 // frozen_round queried by `frozen_list_changelog_attachment`).
 db.reset_exec_counter();
 let _fl0 = frozen_list_membership_aggregated(&db, idx);
 let frozen_list_baseline = db.exec_counter();
 if branch_id == 0 {
 frozen_list_baseline_sample = frozen_list_baseline;
 changelog_baseline_sample = frozen_list_baseline;
 }

 // Mutation: pick the first Active section, flip to Superseded. The
 // fine-grained sub-query for that section will re-execute (returning
 // 1 instead of 0), then the aggregator re-executes (returning
 // violations(N+1) instead of violations(N) or ok). Other sections'
 // sub-queries are NOT re-executed — they still see Active in their
 // own SectionRecord input, which has not changed.
 let target_section = idx
 .sections(&db)
 .iter()
 .find(|s| s.decision_status(&db) == "Active")
 .copied()
 .expect("branch must contain at least one Active section");
 target_section
 .set_decision_status(&mut db)
 .to("Superseded".into());

 // Reset counter and measure mutation impact.
 db.reset_exec_counter();
 let _r1 = section_decision_status_aggregated(&db, idx);
 let exec = db.exec_counter();
 all_invocations.push(exec);

 match classify_size_band(exec) {
 "0" => band_0 += 1,
 "1-9" => band_1_9 += 1,
 "10-99" => band_10_99 += 1,
 _ => band_100_plus += 1,
 }
 // Polynomial bound check — invalidation count must be ≤ baseline
 // (sub-queries + aggregator). At 500 sections per branch, baseline is
 // ~501 (500 sub-queries + 1 aggregator). A single mutation should
 // invalidate ≤ a small constant, well within the polynomial bound.
 if exec > baseline {
 within_bound = false;
 }

 // Round 100 — single FrozenList.owner_section mutation that breaks
 // resolution. Expected exec ≤ 3 (the FrozenList's sub-query +
 // section_by_entity_id for the new owner_id + aggregator).
 let target_frozen_list = idx
 .frozen_lists(&db)
 .iter()
 .copied()
 .next()
 .expect("branch must contain at least one FrozenList");
 target_frozen_list
 .set_owner_section(&mut db)
 .to(u64::MAX);
 db.reset_exec_counter();
 let _fl1 = frozen_list_membership_aggregated(&db, idx);
 frozen_list_mutation_invocations.push(db.exec_counter());

 // Round 101 — single ChangelogEntry.round_number mutation that breaks
 // one round resolution (target → u64::MAX). Every
 // `changelog_by_round_number` cache slot's body iterates the
 // changelog Vec and reads each entry's `round_number`, so the
 // mutation invalidates every unique-round cache. Salsa backdating
 // gates `frozen_list_changelog_attachment` re-execution to the
 // case where one of its read sub-query results actually changed.
 // Polynomial bound: ≤ unique_frozen_rounds + small constant.
 let target_changelog = idx
 .changelog_entries(&db)
 .iter()
 .copied()
 .next()
 .expect("branch must contain at least one ChangelogEntry");
 target_changelog
 .set_round_number(&mut db)
 .to(u64::MAX);
 db.reset_exec_counter();
 let _cl1 = frozen_list_membership_aggregated(&db, idx);
 changelog_round_mutation_invocations.push(db.exec_counter());
 }

 let min = *all_invocations.iter().min().unwrap_or(&0);
 let max = *all_invocations.iter().max().unwrap_or(&0);
 let mean = if all_invocations.is_empty() {
 0
 } else {
 all_invocations.iter().sum::<usize>() / all_invocations.len()
 };

 let fl_min = *frozen_list_mutation_invocations.iter().min().unwrap_or(&0);
 let fl_max = *frozen_list_mutation_invocations.iter().max().unwrap_or(&0);
 let fl_mean = if frozen_list_mutation_invocations.is_empty() {
 0
 } else {
 frozen_list_mutation_invocations.iter().sum::<usize>()
 / frozen_list_mutation_invocations.len()
 };

 let cl_min = *changelog_round_mutation_invocations.iter().min().unwrap_or(&0);
 let cl_max = *changelog_round_mutation_invocations.iter().max().unwrap_or(&0);
 let cl_mean = if changelog_round_mutation_invocations.is_empty() {
 0
 } else {
 changelog_round_mutation_invocations.iter().sum::<usize>()
 / changelog_round_mutation_invocations.len()
 };

 Ok(GateIIFineGrainedResult {
 branch_count: branches,
 band_0,
 band_1_9,
 band_10_99,
 band_100_plus,
 min_invocations: min,
 max_invocations: max,
 mean_invocations: mean,
 baseline_body_count_per_branch: baseline_body_count_sample,
 mutation_within_polynomial_bound: within_bound,
 frozen_list_baseline_body_count_per_branch: frozen_list_baseline_sample,
 frozen_list_owner_mutation_min_invocations: fl_min,
 frozen_list_owner_mutation_max_invocations: fl_max,
 frozen_list_owner_mutation_mean_invocations: fl_mean,
 changelog_baseline_body_count_per_branch: changelog_baseline_sample,
 changelog_round_mutation_min_invocations: cl_min,
 changelog_round_mutation_max_invocations: cl_max,
 changelog_round_mutation_mean_invocations: cl_mean,
 })
}

/// Incremental BranchIndex mutation measurement (Round 94 (c)). For each
/// branch, measure two distinct mutation patterns:
///
/// 1. **Single-section-add** — push a new Active SectionRecord onto
/// `BranchIndex.sections`. The new section's sub-query is fresh; other
/// sections' sub-queries stay cached (their SectionRecord handles are
/// unchanged). Aggregator re-runs because `branch_index.sections` Vec
/// changed.
///
/// 2. **Single-CrossRef-remove** — drop one CrossRef whose `from_section`
/// is a Superseded section's outbound decision/impl ref. The
/// `BranchIndex.cross_refs` Vec is replaced, invalidating every
/// `outbound_crossrefs_by_section(idx, S)` cache. Salsa backdating
/// gates downstream propagation: only the affected from_section's
/// sub-query re-runs (its outbound list shrank), all other sections'
/// sub-queries stay cached (their outbound lists unchanged).
pub fn measure_gate_ii_incremental(branches: usize) -> anyhow::Result<GateIIIncrementalResult> {
 use salsa::Setter;
 let mut section_add_invocations: Vec<usize> = Vec::with_capacity(branches);
 let mut crossref_remove_invocations: Vec<usize> = Vec::with_capacity(branches);

 for branch_id in 0..branches as u64 {
 let snap = synthetic_branch_snapshot(branch_id);

 // ----- single-section-add measurement -----
 {
 let mut db = FineCascadeDb::new();
 let idx = build_branch_index(
  &db,
  branch_id,
  &snap.sections,
  &snap.cross_refs,
  &snap.frozen_lists,
  &snap.changelog_entries,
 );
 // Baseline run.
 let _r0 = section_decision_status_aggregated(&db, idx);
 db.reset_exec_counter();

 // Allocate a brand-new Active SectionRecord and push onto the
 // BranchIndex sections Vec.
 let new_entity_id = branch_id * 1_000_000 + (SECTIONS_PER_BRANCH as u64 + 1);
 let new_section = SectionRecord::new(
  &db,
  branch_id,
  new_entity_id,
  100,
  format!("docs/synthetic-{}.md", branch_id),
  format!("{}.{}", branch_id, SECTIONS_PER_BRANCH + 1),
  format!("Section new of branch {}", branch_id),
  "Active".into(),
 );
 let mut new_sections = idx.sections(&db);
 new_sections.push(new_section);
 idx.set_sections(&mut db).to(new_sections);

 let _r1 = section_decision_status_aggregated(&db, idx);
 section_add_invocations.push(db.exec_counter());
 }

 // ----- single-CrossRef-remove measurement -----
 {
 let mut db = FineCascadeDb::new();
 let idx = build_branch_index(
  &db,
  branch_id,
  &snap.sections,
  &snap.cross_refs,
  &snap.frozen_lists,
  &snap.changelog_entries,
 );
 let _r0 = section_decision_status_aggregated(&db, idx);
 db.reset_exec_counter();

 // Find a CrossRef whose from_section is a Superseded section
 // (decision/impl outbound). Drop it. This makes the source
 // section transition pass → fail in the cascade.
 let cross_refs: Vec<CrossRefRecord> = idx.cross_refs(&db);
 let target_idx_opt = cross_refs.iter().enumerate().find_map(|(i, cr)| {
  let kind = cr.ref_kind(&db);
  if kind.eq_ignore_ascii_case("decision") || kind.eq_ignore_ascii_case("impl") {
  let from = cr.from_section(&db);
  let from_section_is_superseded = idx.sections(&db).iter().any(|s| {
  s.entity_id(&db) == from
   && s.decision_status(&db).eq_ignore_ascii_case("superseded")
  });
  if from_section_is_superseded {
  Some(i)
  } else {
  None
  }
  } else {
  None
  }
 });
 let target_idx = match target_idx_opt {
  Some(i) => i,
  None => {
  // No qualifying CrossRef in this branch — record a
  // zero-impact mutation (skip but track sample).
  crossref_remove_invocations.push(0);
  continue;
  }
 };
 let mut new_cross_refs = cross_refs.clone();
 new_cross_refs.remove(target_idx);
 idx.set_cross_refs(&mut db).to(new_cross_refs);

 let _r1 = section_decision_status_aggregated(&db, idx);
 crossref_remove_invocations.push(db.exec_counter());
 }
 }

 let stats = |samples: &[usize]| -> (usize, usize, usize) {
 if samples.is_empty() {
 return (0, 0, 0);
 }
 let min = *samples.iter().min().unwrap();
 let max = *samples.iter().max().unwrap();
 let mean = samples.iter().sum::<usize>() / samples.len();
 (min, max, mean)
 };
 let (sa_min, sa_max, sa_mean) = stats(&section_add_invocations);
 let (cr_min, cr_max, cr_mean) = stats(&crossref_remove_invocations);

 Ok(GateIIIncrementalResult {
 branch_count: branches,
 section_add_min: sa_min,
 section_add_max: sa_max,
 section_add_mean: sa_mean,
 crossref_remove_min: cr_min,
 crossref_remove_max: cr_max,
 crossref_remove_mean: cr_mean,
 })
}

pub fn run_full_measurement(branches: usize, iter_per_branch: usize) -> anyhow::Result<MeasurementReport> {
 let scale = ScaleParams {
 branches,
 sections_per_branch: SECTIONS_PER_BRANCH,
 changelog_per_branch: CHANGELOG_PER_BRANCH,
 frozen_lists_per_branch: FROZEN_LISTS_PER_BRANCH,
 cross_refs_per_branch: CROSS_REFS_PER_BRANCH,
 total_facts: branches
 * (SECTIONS_PER_BRANCH
  + CHANGELOG_PER_BRANCH
  + FROZEN_LISTS_PER_BRANCH
  + CROSS_REFS_PER_BRANCH),
 };
 let gate_i = measure_gate_i(branches)?;
 let gate_ii = measure_gate_ii(branches)?;
 let gate_ii_fine_grained = measure_gate_ii_fine_grained(branches)?;
 let gate_ii_incremental = measure_gate_ii_incremental(branches)?;
 let gate_iii = measure_gate_iii(branches, iter_per_branch)?;
 let all_pass = gate_i.aggregate_under_1gb
 && gate_i.salsa_db_load_ok
 && gate_ii.mutation_delta_other_count == 0
 && gate_ii_fine_grained.mutation_within_polynomial_bound
 && gate_iii.deterministic_pass == branches
 && gate_iii.cross_db_pass == branches
 && gate_iii.eager_lazy_byte_equal_pass == branches;
 Ok(MeasurementReport {
 fixture_seed: FIXTURE_SEED,
 scale,
 gate_i,
 gate_ii,
 gate_ii_fine_grained,
 gate_ii_incremental,
 gate_iii,
 all_gates_pass: all_pass,
 })
}

#[cfg(test)]
mod tests {
 use super::*;

 #[test]
 fn fixture_total_fact_count_matches_full_scale() {
 let snap = synthetic_branch_snapshot(0);
 assert_eq!(snap.sections.len(), SECTIONS_PER_BRANCH);
 assert_eq!(snap.changelog_entries.len(), CHANGELOG_PER_BRANCH);
 assert_eq!(snap.frozen_lists.len(), FROZEN_LISTS_PER_BRANCH);
 assert_eq!(snap.cross_refs.len(), CROSS_REFS_PER_BRANCH);
 }

 #[test]
 fn fixture_is_deterministic_across_runs() {
 let a = synthetic_branch_snapshot(7);
 let b = synthetic_branch_snapshot(7);
 assert_eq!(a, b);
 }

 #[test]
 fn small_scale_gate_i_passes() {
 let r = measure_gate_i(3).expect("gate i");
 assert_eq!(r.branch_count, 3);
 assert!(r.aggregate_under_1gb);
 assert!(r.salsa_db_load_ok);
 assert!(r.total_encoded_bytes > 0);
 }

 #[test]
 fn small_scale_gate_iii_full_pass() {
 let r = measure_gate_iii(3, 50).expect("gate iii");
 assert_eq!(r.deterministic_pass, 3);
 assert_eq!(r.cross_db_pass, 3);
 assert_eq!(r.eager_lazy_byte_equal_pass, 3);
 }

 #[test]
 fn small_scale_gate_ii_violation_delta_one() {
 let r = measure_gate_ii(3).expect("gate ii");
 assert_eq!(r.branch_count, 3);
 // Each branch should produce delta == +1 after single mutation
 // (cascade rule: Superseded → outbound decision/impl required).
 assert_eq!(r.mutation_delta_one_count, 3);
 assert_eq!(r.mutation_delta_other_count, 0);
 }

 #[test]
 fn small_scale_gate_ii_fine_grained_band_distribution() {
 // 3 branches, single Active→Superseded mutation per branch.
 //
 // Per Round 92 + Round 94 (b) per-section CrossRef pre-indexed layer:
 // - Target sub-query body always re-executes (mutation invalidated
 // its `decision_status` field) → +1 invocation.
 // - `outbound_crossrefs_by_section(idx, target_id)` first invocation
 // when target was previously Active (sub-query short-circuited
 // before reaching the outbound layer) → +1 invocation on the
 // transition to Superseded.
 // - Aggregator body re-executes only if the target sub-query's
 // return value changed (Salsa backdating). Active→Superseded
 // with outbound decision/impl present → sub-query still returns
 // 0 → aggregator skipped. Without outbound → sub-query returns
 // 1 → aggregator re-runs.
 //
 // Therefore exec_counter ∈ {2, 3} per branch — well within band_1_9.
 let r = measure_gate_ii_fine_grained(3).expect("fine-grained gate ii");
 assert_eq!(r.branch_count, 3);
 assert!(r.mutation_within_polynomial_bound);
 assert_eq!(r.band_0, 0);
 assert_eq!(r.band_1_9, 3);
 assert_eq!(r.band_10_99, 0);
 assert_eq!(r.band_100_plus, 0);
 assert!(r.min_invocations >= 2, "min must be ≥ 2 (target sub + outbound first miss)");
 assert!(
 r.max_invocations <= 3,
 "max must be ≤ 3 (sub + outbound + aggregator); got {}",
 r.max_invocations
 );
 // Baseline body count for branch 0: 500 section sub-queries
 // + 1 aggregator + outbound_crossrefs_by_section call per Superseded
 // section (~24 Superseded under FIXTURE_SEED branch 0). Total = 525.
 assert_eq!(r.baseline_body_count_per_branch, 525);
 }

 #[test]
 fn small_scale_gate_ii_incremental_section_add_is_constant() {
 // Round 94 (c) — single Active SectionRecord push onto BranchIndex.sections.
 // Expected exec_counter per branch = 2:
 // 1. New section's sub-query first cache miss (Active → return 0,
 // short-circuits before outbound layer).
 // 2. Aggregator re-runs (BranchIndex.sections Vec changed).
 // Existing sections' sub-queries stay cached (their SectionRecord
 // handles unchanged). All branches deterministic at exec=2.
 let r = measure_gate_ii_incremental(3).expect("incremental gate ii");
 assert_eq!(r.branch_count, 3);
 assert_eq!(r.section_add_min, 2, "section add lower bound");
 assert_eq!(r.section_add_max, 2, "section add upper bound");
 assert_eq!(r.section_add_mean, 2, "section add mean");
 }

 #[test]
 fn small_scale_gate_ii_incremental_crossref_remove_bounded_by_superseded_count() {
 // Round 94 (c) — single CrossRef remove invalidates BranchIndex.cross_refs.
 // Every per-section `outbound_crossrefs_by_section(idx, S)` cache that
 // was populated during baseline (only Superseded sections call it)
 // re-executes; Salsa backdating then gates downstream propagation.
 //
 // Expected: 1 ≤ exec ≤ ~Superseded count + a small constant.
 // SECTIONS_PER_BRANCH = 500 with ~5% Superseded ⇒ ~25 outbound
 // re-runs per branch + 1 affected sub-query + 1 aggregator ≈ 27.
 let r = measure_gate_ii_incremental(3).expect("incremental gate ii");
 assert!(
 r.crossref_remove_min >= 1,
 "at least 1 invocation per branch on cross_refs Vec replace; got {}",
 r.crossref_remove_min
 );
 assert!(
 r.crossref_remove_max <= 200,
 "max bounded by ~Superseded count; got {}",
 r.crossref_remove_max
 );
 assert!(
 r.crossref_remove_mean >= 1,
 "mean must reflect non-trivial cost; got {}",
 r.crossref_remove_mean
 );
 }
}
