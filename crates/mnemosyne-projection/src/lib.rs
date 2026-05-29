//! Read-side warm projection service — convergence C/D Step 1 (the validation
//! walking skeleton).
//!
//! This is the application service that finally wires the fine-grained Salsa
//! cascade engine into a live path. It composes the design_doc adapter
//! projections (`mnemosyne-atomic`) with the projection engine
//! (`mnemosyne-cascade`): it folds the git-native authoring log into a
//! `BranchIndex` of per-entity Salsa inputs and serves Layer-0 validation from a
//! *warm* `FineCascadeDb` held across calls.
//!
//! ## Why a warm service (and why it lives one layer up)
//!
//! Salsa memoization is in-process, so the incremental advantage only exists in
//! a long-running host — a one-shot CLI rebuilds cold every invocation. This
//! service is therefore meant to be embedded in a warm host (the MCP server is
//! the first; see ARCHITECTURE.md, Round 337). It depends *inward* on the adapter
//! and the engine, and nothing depends back on it, so the engine never learns
//! about the adapter (the dependency-inversion placement mirrors `mnemosyne-index`).
//!
//! ## RocksDB-free (the CQRS split)
//!
//! The service reads the log via the adapter projections and drives the Salsa
//! engine entirely in memory — it never touches the materialized RocksDB index.
//! That is what lets an authoring host embed it without dragging RocksDB into the
//! write path (Round 328; Round 337 CQRS split). The durable RocksDB index
//! (`mnemosyne-index`) is the *other*, cross-process read model; this is the
//! in-process one.
//!
//! ## Scope (Step 1)
//!
//! Validation only — the cheapest real projection, enough to prove the
//! warm-host + RocksDB-free split end-to-end. The render projection (incremental
//! `GENERATED.md`) and incremental delta-application on mutate (so a re-sync
//! keeps unchanged sub-queries memoized) are the next steps, co-designed as
//! convergence D.

pub mod render;

pub use render::RenderProjectionService;

use mnemosyne_atomic::AtomicStore;
use mnemosyne_cascade::{
    build_branch_index, frozen_list_membership_aggregated, reconcile_branch_index,
    section_decision_status_aggregated, BranchIndex, FineCascadeDb, ValidationResult,
};

/// Combined Layer-0 validation over a projected branch: the two cascade
/// aggregators (Section decision-status supersession and FrozenList membership).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionValidation {
    pub section_decision: ValidationResult,
    pub frozen_membership: ValidationResult,
}

impl ProjectionValidation {
    /// True when both aggregators pass.
    pub fn ok(&self) -> bool {
        self.section_decision.ok && self.frozen_membership.ok
    }

    /// Sum of violations across both aggregators.
    pub fn total_violations(&self) -> u32 {
        self.section_decision
            .violation_count
            .saturating_add(self.frozen_membership.violation_count)
    }
}

/// Warm read-side projection service: a live `FineCascadeDb` plus the
/// `BranchIndex` projected from the authoring log. Hold one across calls; repeated
/// [`validate`](Self::validate) on a stable index is served from the Salsa memo
/// cache (the in-process warmth a one-shot CLI cannot have).
pub struct ProjectionService {
    db: FineCascadeDb,
    index: BranchIndex,
    branch_id: u64,
}

impl ProjectionService {
    /// Project `branch_id` of `atomic` into a warm engine.
    pub fn build(atomic: &AtomicStore, branch_id: u64) -> Self {
        let db = FineCascadeDb::new();
        let index = project_branch(&db, atomic, branch_id);
        Self {
            db,
            index,
            branch_id,
        }
    }

    /// The branch this service projects.
    pub fn branch_id(&self) -> u64 {
        self.branch_id
    }

    /// Validate the warm projection. Repeated calls without an intervening
    /// [`reload`](Self::reload) hit the Salsa memo cache.
    pub fn validate(&self) -> ProjectionValidation {
        ProjectionValidation {
            section_decision: section_decision_status_aggregated(&self.db, self.index),
            frozen_membership: frozen_list_membership_aggregated(&self.db, self.index),
        }
    }

    /// Re-sync the projection from the current log, reusing the warm engine.
    ///
    /// Applies the minimal Salsa-input delta against the live `BranchIndex`
    /// (convergence D): unchanged entities keep their record handles, so their
    /// memoized sub-queries carry across the re-sync and only the entities that
    /// actually changed re-execute on the next [`validate`](Self::validate). The
    /// `BranchIndex` handle itself is preserved, so the aggregators stay memoized
    /// when no sub-query result changed.
    pub fn reload(&mut self, atomic: &AtomicStore) {
        let sections = atomic.project_section_facts(self.branch_id);
        let cross_refs = atomic.project_cross_ref_facts(self.branch_id);
        let changelog = atomic.project_changelog_entry_facts(self.branch_id);
        reconcile_branch_index(
            &mut self.db,
            self.index,
            &sections,
            &cross_refs,
            &[],
            &changelog,
        );
    }
}

/// Project one branch of the log into a fine-grained `BranchIndex`. FrozenList
/// has no design_doc-adapter representation (Round 327), so that engine input is
/// empty; Section / CrossRef / ChangelogEntry come from the adapter projections.
fn project_branch(db: &FineCascadeDb, atomic: &AtomicStore, branch_id: u64) -> BranchIndex {
    let sections = atomic.project_section_facts(branch_id);
    let cross_refs = atomic.project_cross_ref_facts(branch_id);
    let changelog = atomic.project_changelog_entry_facts(branch_id);
    build_branch_index(db, branch_id, &sections, &cross_refs, &[], &changelog)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mnemosyne_atomic::{AtomicSection, MAIN_BRANCH_ID};
    use mnemosyne_core::{DecisionStatus, SectionSkeleton};

    fn section(title: &str, status: Option<DecisionStatus>, impact: &[&str]) -> AtomicSection {
        AtomicSection {
            skeleton: SectionSkeleton {
                title: title.to_string(),
                parent_doc: "docs/DESIGN.md".to_string(),
                parent_section: None,
                decision_status: status,
            },
            impact_scope: impact.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }

    fn store_with(sections: Vec<(&str, AtomicSection)>) -> AtomicStore {
        let mut store = AtomicStore::new();
        for (id, s) in sections {
            store.sections.insert(id.to_string(), s);
        }
        store
    }

    #[test]
    fn empty_store_validates_ok() {
        let svc = ProjectionService::build(&AtomicStore::new(), MAIN_BRANCH_ID);
        let v = svc.validate();
        assert!(v.ok());
        assert_eq!(v.total_violations(), 0);
    }

    #[test]
    fn active_sections_validate_ok() {
        let store = store_with(vec![
            ("alpha", section("Alpha", Some(DecisionStatus::Active), &[])),
            (
                "beta",
                section("Beta", Some(DecisionStatus::Active), &["alpha"]),
            ),
        ]);
        let svc = ProjectionService::build(&store, MAIN_BRANCH_ID);
        assert!(svc.validate().ok());
    }

    #[test]
    fn superseded_without_outbound_ref_is_flagged() {
        // A Superseded section with no outbound decision/impl cross-ref violates
        // the supersession invariant — the projection must surface it.
        let store = store_with(vec![(
            "old",
            section("Old", Some(DecisionStatus::Superseded), &[]),
        )]);
        let svc = ProjectionService::build(&store, MAIN_BRANCH_ID);
        let v = svc.validate();
        assert!(!v.ok());
        assert_eq!(v.section_decision, ValidationResult::violations(1));
    }

    #[test]
    fn superseded_with_outbound_impact_ref_passes() {
        // impact_scope projects to a CrossRef of kind "impact_scope", which is
        // not a supersession pointer (decision/impl), so a bare impact edge does
        // not satisfy the invariant.
        let store = store_with(vec![
            (
                "old",
                section("Old", Some(DecisionStatus::Superseded), &["new"]),
            ),
            ("new", section("New", Some(DecisionStatus::Active), &[])),
        ]);
        let svc = ProjectionService::build(&store, MAIN_BRANCH_ID);
        // impact_scope alone is not a decision/impl ref → still one violation.
        assert_eq!(
            svc.validate().section_decision,
            ValidationResult::violations(1)
        );
    }

    #[test]
    fn superseded_with_stored_superseded_by_passes() {
        // R342: the structural superseded_by forward-pointer projects to a
        // `decision`-kind CrossRefFact, which satisfies the invariant — so a
        // Superseded section that recorded its replacement validates clean.
        // This is the over-flagging bug the warm projection had before the
        // pointer was stored structurally (it could only see impact_scope).
        let mut old = section("Old", Some(DecisionStatus::Superseded), &[]);
        old.superseded_by = Some("new".to_string());
        let store = store_with(vec![
            ("old", old),
            ("new", section("New", Some(DecisionStatus::Active), &[])),
        ]);
        let svc = ProjectionService::build(&store, MAIN_BRANCH_ID);
        assert!(svc.validate().ok());
        assert_eq!(svc.validate().section_decision, ValidationResult::ok());
    }

    #[test]
    fn reload_re_syncs_after_a_log_change() {
        let mut store = store_with(vec![(
            "alpha",
            section("Alpha", Some(DecisionStatus::Active), &[]),
        )]);
        let mut svc = ProjectionService::build(&store, MAIN_BRANCH_ID);
        assert!(svc.validate().ok());

        // Mutate the log: flip alpha to Superseded with no supersession ref.
        store
            .sections
            .get_mut("alpha")
            .unwrap()
            .skeleton
            .decision_status = Some(DecisionStatus::Superseded);

        // The warm projection still reflects the pre-change state until reload.
        assert!(svc.validate().ok());
        svc.reload(&store);
        assert_eq!(
            svc.validate().section_decision,
            ValidationResult::violations(1)
        );
    }

    #[test]
    fn reload_handles_section_add_and_remove() {
        // Exercises the membership-change branch of the incremental reconcile
        // (add allocates a record + resets the list; remove drops it).
        let mut store = store_with(vec![(
            "alpha",
            section("Alpha", Some(DecisionStatus::Active), &[]),
        )]);
        let mut svc = ProjectionService::build(&store, MAIN_BRANCH_ID);
        assert!(svc.validate().ok());

        // Add a Superseded section with no supersession ref → one violation.
        store.sections.insert(
            "beta".to_string(),
            section("Beta", Some(DecisionStatus::Superseded), &[]),
        );
        svc.reload(&store);
        assert_eq!(
            svc.validate().section_decision,
            ValidationResult::violations(1)
        );

        // Remove the offending section → clean again.
        store.sections.remove("beta");
        svc.reload(&store);
        assert!(svc.validate().ok());
    }
}
