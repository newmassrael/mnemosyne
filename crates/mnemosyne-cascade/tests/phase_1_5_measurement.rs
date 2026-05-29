//! Cascade measurement — fine-grained engine invalidation gates.
//!
//! Migrated from the coarse-engine Phase 1.5 measurement when the fine-grained
//! engine became C's production engine and the coarse `runtime`/`snapshot` path
//! was retired (R338). The coarse engine re-executed at *branch* granularity, so
//! the only thing its "invalidation" gate could measure was payload size; the
//! fine engine's defining property is **size-independent invalidation** — a
//! single fact mutation re-executes a bounded number of sub-queries regardless
//! of branch size. These gates assert that directly: the same mutation is run at
//! two fixture scales and the per-DB execution counter is compared.
//!
//! Synthetic per-branch fixture (the scale dimension the inline unit tests in
//! `fine_grained.rs` do not cover): 10 branches × (50 Section + 50 ChangelogEntry
//! + 25 FrozenList + 50 CrossRef) = 1,750 facts. Deterministic, < 1s under
//! `cargo test`. Full release-scale measurement remains a separate bench concern.

use mnemosyne_cascade::{
    build_branch_index, frozen_list_membership_aggregated, reconcile_branch_index,
    section_decision_status_aggregated, BranchIndex, FineCascadeDb, ValidationResult,
};
use mnemosyne_core::{
    ChangelogEntryFact, CrossRefFact, DecisionStatus, FactKey, FrozenListFact, SectionFact,
    SectionSkeleton,
};
use salsa::Setter;

const SECTIONS_PER_BRANCH: usize = 50;
const CHANGELOG_PER_BRANCH: usize = 50;
const FROZEN_LISTS_PER_BRANCH: usize = 25;
const CROSS_REFS_PER_BRANCH: usize = 50;
const BRANCH_COUNT: usize = 10;
const TOTAL_FACTS: usize = BRANCH_COUNT
    * (SECTIONS_PER_BRANCH
        + CHANGELOG_PER_BRANCH
        + FROZEN_LISTS_PER_BRANCH
        + CROSS_REFS_PER_BRANCH);

fn section_fact(branch_id: u64, entity_id: u64, status: DecisionStatus) -> SectionFact {
    SectionFact {
        key: FactKey {
            branch_id,
            entity_id,
            valid_from: 100,
        },
        section_id: format!("{branch_id}.{entity_id}"),
        skeleton: SectionSkeleton {
            title: format!("section {entity_id} of branch {branch_id}"),
            parent_doc: format!("docs/synthetic-{branch_id}.md"),
            parent_section: None,
            decision_status: Some(status),
        },
    }
}

/// Full synthetic branch — all four fact kinds, sized for the 1,750-fact
/// aggregate. Owners and frozen rounds resolve, every section is Active, so both
/// aggregators pass.
struct SyntheticBranch {
    sections: Vec<SectionFact>,
    changelog_entries: Vec<ChangelogEntryFact>,
    frozen_lists: Vec<FrozenListFact>,
    cross_refs: Vec<CrossRefFact>,
}

fn synthetic_branch(branch_id: u64) -> SyntheticBranch {
    let base = branch_id * 1_000_000;
    let sections = (0..SECTIONS_PER_BRANCH)
        .map(|i| section_fact(branch_id, base + i as u64, DecisionStatus::Active))
        .collect();
    let changelog_entries = (0..CHANGELOG_PER_BRANCH)
        .map(|i| ChangelogEntryFact {
            key: FactKey {
                branch_id,
                entity_id: base + 1000 + i as u64,
                valid_from: 100 + i as u64,
            },
            round_number: i as u64,
            summary: format!("synthetic round {i} branch {branch_id}"),
        })
        .collect();
    let frozen_lists = (0..FROZEN_LISTS_PER_BRANCH)
        .map(|i| FrozenListFact {
            key: FactKey {
                branch_id,
                entity_id: base + 2000 + i as u64,
                valid_from: 100,
            },
            owner_section: base + (i % SECTIONS_PER_BRANCH) as u64,
            frozen_round: i as u64,
            kind: "release_lock".into(),
        })
        .collect();
    let cross_refs = (0..CROSS_REFS_PER_BRANCH)
        .map(|i| CrossRefFact {
            branch_id,
            from_section: base + (i % SECTIONS_PER_BRANCH) as u64,
            to_section: base + ((i + 1) % SECTIONS_PER_BRANCH) as u64,
            ref_kind: "decision".into(),
        })
        .collect();
    SyntheticBranch {
        sections,
        changelog_entries,
        frozen_lists,
        cross_refs,
    }
}

fn build_synthetic(db: &FineCascadeDb, branch_id: u64) -> BranchIndex {
    let b = synthetic_branch(branch_id);
    build_branch_index(
        db,
        branch_id,
        &b.sections,
        &b.cross_refs,
        &b.frozen_lists,
        &b.changelog_entries,
    )
}

/// A branch of `n` Active sections and no other facts — the clean substrate for
/// the invalidation gate: flipping one section to Superseded yields exactly one
/// violation (no outbound ref), and the re-execution count is independent of `n`.
fn active_only_branch(db: &FineCascadeDb, branch_id: u64, n: usize) -> BranchIndex {
    let base = branch_id * 1_000_000;
    let sections: Vec<SectionFact> = (0..n)
        .map(|i| section_fact(branch_id, base + i as u64, DecisionStatus::Active))
        .collect();
    build_branch_index(db, branch_id, &sections, &[], &[], &[])
}

#[test]
fn full_fixture_passes_both_aggregators_at_scale() {
    let db = FineCascadeDb::new();
    for branch_id in 0..BRANCH_COUNT as u64 {
        let idx = build_synthetic(&db, branch_id);
        assert_eq!(
            section_decision_status_aggregated(&db, idx),
            ValidationResult::ok(),
            "branch {branch_id} decision status"
        );
        assert_eq!(
            frozen_list_membership_aggregated(&db, idx),
            ValidationResult::ok(),
            "branch {branch_id} frozen-list membership"
        );
    }
}

#[test]
fn fixture_total_fact_count_matches_projection() {
    let total: usize = (0..BRANCH_COUNT as u64)
        .map(|b| {
            let s = synthetic_branch(b);
            s.sections.len() + s.changelog_entries.len() + s.frozen_lists.len() + s.cross_refs.len()
        })
        .sum();
    assert_eq!(total, TOTAL_FACTS);
    assert_eq!(total, 1_750);
}

/// Gate (iii) — deterministic re-run: same input → same output, both within one
/// DB (memoize stability) and across fresh DB instances (no storage-state leak).
#[test]
fn aggregator_results_are_deterministic_within_and_across_dbs() {
    for branch_id in 0..BRANCH_COUNT as u64 {
        let db1 = FineCascadeDb::new();
        let idx1 = build_synthetic(&db1, branch_id);
        let a1 = section_decision_status_aggregated(&db1, idx1);
        let a2 = section_decision_status_aggregated(&db1, idx1);
        assert_eq!(a1, a2, "branch {branch_id} re-run drift");

        let db2 = FineCascadeDb::new();
        let idx2 = build_synthetic(&db2, branch_id);
        let b1 = section_decision_status_aggregated(&db2, idx2);
        assert_eq!(a1, b1, "branch {branch_id} cross-db drift");
    }
}

/// Violation injection — one Superseded section without an outbound decision/impl
/// ref is detected as exactly one violation, even embedded in the full fixture.
#[test]
fn violation_injection_detected_in_full_fixture() {
    let db = FineCascadeDb::new();
    let mut b = synthetic_branch(0);
    b.sections[0].skeleton.decision_status = Some(DecisionStatus::Superseded);
    let target = b.sections[0].key.entity_id;
    b.cross_refs.retain(|cr| cr.from_section != target);
    let idx = build_branch_index(
        &db,
        0,
        &b.sections,
        &b.cross_refs,
        &b.frozen_lists,
        &b.changelog_entries,
    );
    assert_eq!(
        section_decision_status_aggregated(&db, idx),
        ValidationResult::violations(1)
    );
}

/// Gate (ii.a) — an unread-field mutation (Section.title) invalidates nothing,
/// independent of branch size. Run at two scales; both must be zero.
#[test]
fn title_mutation_invalidates_zero_sub_queries_at_any_scale() {
    for &n in &[5usize, 50] {
        let mut db = FineCascadeDb::new();
        let idx = active_only_branch(&db, 1, n);
        let r0 = section_decision_status_aggregated(&db, idx);
        assert!(r0.ok);

        db.reset_exec_counter();
        let target = idx.sections(&db)[n / 2];
        target.set_title(&mut db).to("changed".into());
        let r1 = section_decision_status_aggregated(&db, idx);
        assert!(r1.ok);
        assert_eq!(
            db.exec_counter(),
            0,
            "Section.title mutation must invalidate zero sub-queries at n={n}"
        );
    }
}

/// Gate (ii.b) — the invalidation polynomial is **size-independent**. Flipping
/// one Active section to Superseded re-executes the same bounded set of bodies
/// (target sub-query + its per-section outbound cache + the aggregator) whether
/// the branch holds 5 or 50 sections.
#[test]
fn decision_status_flip_invalidation_is_independent_of_branch_size() {
    fn flip_one_to_superseded(n: usize) -> usize {
        let mut db = FineCascadeDb::new();
        let idx = active_only_branch(&db, 1, n);
        let r0 = section_decision_status_aggregated(&db, idx);
        assert!(r0.ok, "baseline must be clean at n={n}");

        db.reset_exec_counter();
        let target = idx.sections(&db)[n / 2];
        target
            .set_decision_status(&mut db)
            .to(Some(DecisionStatus::Superseded));
        let r1 = section_decision_status_aggregated(&db, idx);
        assert_eq!(r1.violation_count, 1, "exactly one violation at n={n}");
        db.exec_counter()
    }

    let small = flip_one_to_superseded(5);
    let large = flip_one_to_superseded(50);
    assert_eq!(
        small, large,
        "invalidation count must not grow with branch size (5 vs 50 sections): {small} != {large}"
    );
    // target sub-query + outbound_crossrefs_by_section(target) + aggregator.
    assert_eq!(
        small, 3,
        "expected the bounded 3-body re-execution; got {small}"
    );
}

/// Convergence D — re-syncing through `reconcile_branch_index` (the read-side
/// service's incremental reload) applies the *same* minimal delta a direct field
/// setter would: a new fact snapshot that flips one section to Superseded
/// re-executes the identical bounded body set (target sub-query + its outbound
/// cache + the aggregator) at 5 and 50 sections. A wholesale rebuild would
/// instead re-execute every section's sub-query, growing with branch size — so
/// the size-independence here is the proof that reload reuses unchanged handles.
#[test]
fn reconcile_single_section_flip_invalidation_is_independent_of_branch_size() {
    fn flip_via_reconcile(n: usize) -> usize {
        let mut db = FineCascadeDb::new();
        let base = 1_000_000u64;
        let mut sections: Vec<SectionFact> = (0..n)
            .map(|i| section_fact(1, base + i as u64, DecisionStatus::Active))
            .collect();
        let idx = build_branch_index(&db, 1, &sections, &[], &[], &[]);
        assert!(
            section_decision_status_aggregated(&db, idx).ok,
            "baseline must be clean at n={n}"
        );

        // The new snapshot differs only in the middle section's decision_status.
        sections[n / 2].skeleton.decision_status = Some(DecisionStatus::Superseded);

        db.reset_exec_counter();
        reconcile_branch_index(&mut db, idx, &sections, &[], &[], &[]);
        let r = section_decision_status_aggregated(&db, idx);
        assert_eq!(r.violation_count, 1, "exactly one violation at n={n}");
        db.exec_counter()
    }

    let small = flip_via_reconcile(5);
    let large = flip_via_reconcile(50);
    assert_eq!(
        small, large,
        "reconcile re-sync invalidation must not grow with branch size (5 vs 50): {small} != {large}"
    );
    assert_eq!(
        small, 3,
        "reconcile must apply the same bounded 3-body delta a direct setter does; got {small}"
    );
}
