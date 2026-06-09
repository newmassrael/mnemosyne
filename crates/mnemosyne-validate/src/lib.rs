//! Mnemosyne validate — T1/T2/T3 validation rules, code-citation defense,
//! and commit-ledger drift detection.
//!
//! - [`validator`] T1 store-direct rules: prose cross-ref orphan scan +
//!   atomic-section supersede state gate.
//! - [`t2`] T2 atomic frozen-ledger append-only checks.
//! - [`code_refs`] R256+ code-citation defense: SetEqualityValidator,
//!   section/inventory decay scan, symbol-mismatch axis.
//! - [`commit_ledger`] commit↔ledger drift report (last-N-commits scan
//!   vs atomic ledger entry IDs).
//! - [`spec_drift`] RFC-001 UC-1 "B2": spec-revision label-drift scan
//!   (per-Section `source_revision` vs workspace `spec_source.revision`).
//! - [`content_drift`] R404: offline content-integrity scan — re-hash each
//!   excerpt's stored `text` vs its declared `text_sha256`.

pub mod code_refs;
pub mod commit_ledger;
pub mod confirmation;
pub mod content_drift;
pub mod spec_drift;
pub mod t2;
pub mod validator;

pub use commit_ledger::{diff as commit_ledger_diff, CommitLedgerDriftReport};
pub use content_drift::{scan_content_drift, ContentDriftViolation};
pub use spec_drift::{scan_spec_drift, SpecDriftViolation};
pub use t2::{frozen_ledger_atomic, T2ValidationError};
pub use validator::{atomic_section_supersede_state_reject, ValidationError};
