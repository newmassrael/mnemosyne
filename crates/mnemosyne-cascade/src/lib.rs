//! Mnemosyne cascade — the Salsa incremental-projection engine (Layer-0 → read
//! model) for the design_doc cascade queries.
//!
//! The engine is built on per-entity `#[salsa::input]` records (Section /
//! CrossRef / FrozenList / ChangelogEntry) collected under a `BranchIndex`
//! input, plus per-record tracked sub-queries and per-branch aggregators. Salsa
//! 0.26 field-level dependency tracking gives bounded, size-independent
//! invalidation: mutating one record's field re-executes only the sub-queries
//! that read it, and the aggregator backdates when sub-query results are
//! unchanged.
//!
//! The engine is pure and in-memory: it consumes canonical facts
//! (`mnemosyne-facts`) directly and knows nothing of the authoring adapter or
//! the RocksDB index. The read-side service that builds a `BranchIndex` from the
//! live log lives one layer up (so this crate never depends on the adapter).
//!
//! ## Modules
//!
//! - [`fine_grained`]: per-entity Salsa inputs + tracked sub-queries + per-branch
//!   aggregators + the concrete `FineCascadeDb` runtime + `build_branch_index`.
//! - [`result`]: the `ValidationResult` query output value object.
//! - [`metadata`]: cascade dependency graph + ordering axis (read-only consumers
//!   visualize the query topology).
//! - [`spec`]: cascade query spec types (CascadeQuerySpec / ReadDep / TriggerSpec)
//!   + `design_doc_cascade_fixture`.

pub mod fine_grained;
pub mod metadata;
pub mod result;
pub mod spec;

pub use fine_grained::{
    build_branch_index, changelog_by_round_number, frozen_list_membership_aggregated,
    outbound_crossrefs_by_section, section_by_entity_id, section_decision_status_aggregated,
    BranchIndex, CascadeDb, ChangelogRecord, CrossRefRecord, FineCascadeDb, FrozenListRecord,
    SectionRecord,
};
pub use metadata::{cascade_dependency_edges, cascade_orderings};
pub use result::ValidationResult;
pub use spec::{
    design_doc_cascade_fixture, CascadeQuerySpec, CascadeWireSpec, ReadDep, TriggerSpec,
};
