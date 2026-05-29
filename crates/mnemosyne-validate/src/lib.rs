//! Mnemosyne validate — T1/T2/T3 validation rules, code-citation defense,
//! and commit-ledger drift detection.
//!
//! - [`validator`] T1 rules: cross_ref orphan / changelog append-only /
//!   frozen-list membership delta / decision-status transition /
//!   atomic-section supersede state.
//! - [`t2`] T2 frozen-ledger jaccard and atomic-section frozen checks.
//! - [`code_refs`] R256+ code-citation defense: SetEqualityValidator,
//!   section/inventory decay scan, symbol-mismatch axis.
//! - [`commit_ledger`] commit↔ledger drift report (last-N-commits scan
//!   vs atomic ledger entry IDs).
//! - [`spec_drift`] RFC-001 UC-1 "B2": spec-revision label-drift scan
//!   (per-Section `source_revision` vs workspace `spec_source.revision`).

pub mod code_refs;
pub mod commit_ledger;
pub mod spec_drift;
pub mod t2;
pub mod validator;

pub use commit_ledger::{diff as commit_ledger_diff, CommitLedgerDriftReport};
pub use spec_drift::{scan_spec_drift, SpecDriftViolation};
pub use t2::{frozen_ledger_atomic, frozen_ledger_jaccard, T2ValidationError};
pub use validator::{
    atomic_section_supersede_state_reject, changelog_entry_append_only, cross_ref_orphan_reject,
    frozen_list_membership_delta, section_decision_status_transition, ValidationError,
};
