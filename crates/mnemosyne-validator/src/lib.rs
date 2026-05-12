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

pub mod schema;
pub mod parser;
pub mod emitter;
pub mod validator;
pub mod workspace;
pub mod config;
pub mod query;
pub mod t2;
pub mod mutate;
pub mod style;
pub mod atomic;
pub mod render;
pub mod code_refs;

pub use schema::{
 ChangelogEntry, CrossRef, DecisionStatus, FrozenList, LockKind, ParsedDoc, RefKind, Section,
};
pub use parser::{parse_markdown, parse_markdown_with_schema};
pub use emitter::{compare_typed_facts, emit_markdown, to_github_anchor, RoundTripDiff};
pub use validator::{
 changelog_entry_append_only, cross_ref_orphan_reject, frozen_list_membership_delta,
 section_decision_status_transition, ValidationError,
};
pub use workspace::Workspace;
pub use config::{
 discover_config, load_config, parse_config, CodeRefsSection, LoadedConfig, OrphanKind,
 OrphanLedgerEntry, SchemaSection, StyleSection, TerminologySection, WorkspaceConfig,
 WorkspaceSection,
};
pub use query::{
 build_envelope, changelog_entries_for_section, related_sections,
 related_sections_with_atomic, section_by_id, workspace_section_id_set,
 ChangelogEntryView, CrossRefView, QueryEnvelope, RelatedSections, SectionView,
};
pub use t2::{frozen_ledger_atomic, frozen_ledger_jaccard, T2ValidationError};
pub use mutate::{
 add_cross_ref, add_section, append_changelog_entry, set_section_body,
 set_section_decision_status, MutateError, MutateErrorKind, MutateReceipt,
};
pub use style::{
 check_style, default_ruleset, default_ruleset_with_config, glossary_from_config, StyleRule,
 StyleScope, StyleSeverity, StyleThreshold, StyleTier,
 StyleViolation,
};
pub use atomic::{
 add_section_caveat, add_section_example, add_section_implementation,
 append_changelog_entry_v2, set_section_alternatives,
 set_section_decision_status_atomic, set_section_impact_scope,
 set_section_inputs, set_section_intent, set_section_outputs,
 set_section_rationale, AtomicChangelogEntry, AtomicMutateError,
 AtomicMutateReceipt, AtomicSection, AtomicStore, AtomicStoreError,
 ExampleBlock, Implementation, RejectedAlternative,
};
pub use render::{render_changelog_entry, render_section, RenderError};
