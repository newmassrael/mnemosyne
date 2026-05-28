//! Mnemosyne cascade — Phase 0 production crate (DESIGN.md).
//!
//! This crate is *cascade_query kind*'s actual Salsa 0.26 runtime binding —
//! `#[salsa::input]` CascadeBranch + entity input structs + `#[salsa::tracked]`
//! cascade query functions + `#[salsa::db]` `CascadeDb` trait + concrete
//! `MnemosyneCascadeDb` runtime + cascade dependency graph metadata.
//!
//! prototype `bench/codegen-prototype/src/salsa_wire.rs` (codegen emit source)
//! Production binding — emits one source string compiled via the actual Rust pipeline
//! direct dogfood of the codegen result.
//!
//! paradigm carry — the Salsa runtime itself emits Rust-only (cascade_query
//! scope — *partial break of the 5-language emit contract* (audit trail). Studio Kotlin / CLI
//! Python's dependency-graph visualization `metadata` module — the 5-language emit
//! in read-only consumer path.
//!
//! ## Module separation
//!
//! - [`runtime`]: Salsa input structs (`CascadeBranch`, `SectionInput`,
//! `ChangelogEntryInput`, `FrozenListInput`) + tracked query functions +
//! `CascadeDb` trait + `MnemosyneCascadeDb` concrete runtime + `ValidationResult`.
//! - [`metadata`]: cascade dependency graph + ordering axis (Studio/CLI visualize
//! read-only consumer path, 5-language metadata emit).
//! - [`spec`]: cascade query spec types (CascadeQuerySpec / ReadDep / TriggerSpec)
//! + `design_doc_cascade_fixture`.
//! - [`snapshot`]: per-branch typed-fact bundle + serde encoding + store load
//! helper.

pub mod fine_grained;
pub mod metadata;
pub mod runtime;
pub mod snapshot;
pub mod spec;

pub use fine_grained::{
    build_branch_index, changelog_by_round_number, frozen_list_membership_aggregated,
    outbound_crossrefs_by_section, section_by_entity_id, section_decision_status_aggregated,
    BranchIndex, ChangelogRecord, CrossRefRecord, FineCascadeDb, FrozenListRecord, SectionRecord,
};
pub use metadata::{cascade_dependency_edges, cascade_orderings};
pub use runtime::{
    frozen_list_membership, section_decision_status, CascadeBranch, CascadeDb, ChangelogEntryInput,
    FrozenListInput, MnemosyneCascadeDb, SectionInput, ValidationResult,
};
pub use snapshot::{BranchEntityPartition, BranchSnapshotData, SnapshotError};
pub use spec::{
    design_doc_cascade_fixture, CascadeQuerySpec, CascadeWireSpec, ReadDep, TriggerSpec,
};
