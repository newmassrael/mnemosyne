//! Mnemosyne validator — Phase 0 production crate (DESIGN.md / / /).
//!
//! This crate is the mnemosyne workspace's *first production crate* (-71 prototype
//! end-of-sequence carry signal — *Phase 0 implementation-layer first carry*.
//! design_doc medium's 4 typed-fact entities/relations (Section / ChangelogEntry /
//! FrozenList / CrossRef closed-form registered source carry) bridging markdown ↔ typed
//! facts transform + T1 validator 4 rule standalone behavior source of truth.
//!
//! ## Module separation (5 modules)
//!
//! - [`schema`]: closed-form full shape — Section / ChangelogEntry / FrozenList /
//! CrossRef + DecisionStatus / LockKind / RefKind enum (4 entity/relation typed facts).
//! - [`parser`]: markdown variant spec — markdown bytes → typed facts (lookup
//! Lookup-priority step 3.
//! - [`emitter`]: markdown variant spec — typed facts → markdown bytes (row 7/8/9
//! branch logic OPTION H-2 adoption carry).
//! - [`validator`]: *Phase 0 Validator (T1 standalone behavior, 4 rule)* + ValidationError
//! 4-variant typed enum lookup carry).
//! - [`workspace`]: workspace-level config (default-doc binding
//! ops-tuning param spec decision surface separation pattern equivalent — mnemosyne workspace
//! default_doc = DESIGN.md).
//! - [`query`]: *Spec query API surface* 4 primitive (ratify +
//! production lift) — section_by_id / related_sections /
//! changelog_entries_for_section / workspace_section_id_set + JSON envelope
//! shape (Claude-consumable). prerequisite #5 *AI agent dogfood proof*
//! production gate.

pub mod validator;
pub mod workspace;
pub mod query;
pub mod t2;
pub mod style;
pub mod render;
pub mod code_refs;
pub mod commit_ledger;

pub use validator::{
 atomic_section_supersede_state_reject, changelog_entry_append_only, cross_ref_orphan_reject,
 frozen_list_membership_delta, section_decision_status_transition, ValidationError,
};
pub use workspace::Workspace;
pub use query::{
 build_envelope, changelog_entries_for_section, query_term, related_sections,
 related_sections_with_atomic, section_by_id, workspace_section_id_set,
 ChangelogEntryView, CrossRefView, QueryEnvelope, QueryTermError, RelatedSections,
 SectionView, TermHit, TermMode, TermQuery, TermScope, TermTargetKind,
};
pub use t2::{frozen_ledger_atomic, frozen_ledger_jaccard, T2ValidationError};
pub use style::{
 check_style, default_ruleset, default_ruleset_with_config, glossary_from_config, StyleRule,
 StyleScope, StyleSeverity, StyleThreshold, StyleTier,
 StyleViolation,
};
pub use render::{render_changelog_entry, render_section, RenderError};
pub use commit_ledger::{diff as commit_ledger_diff, CommitLedgerDriftReport};
