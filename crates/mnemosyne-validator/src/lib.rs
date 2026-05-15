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
pub mod commit_ledger;
pub mod redact;

pub use schema::{
 ChangelogEntry, CrossRef, DecisionStatus, FrozenList, LockKind, ParsedDoc, RefKind, Section,
};
pub use parser::{parse_markdown, parse_markdown_with_schema};
pub use emitter::{compare_typed_facts, emit_markdown, to_github_anchor, RoundTripDiff};
pub use validator::{
 atomic_section_supersede_state_reject, changelog_entry_append_only, cross_ref_orphan_reject,
 frozen_list_membership_delta, section_decision_status_transition, ValidationError,
};
pub use workspace::Workspace;
pub use config::{
 discover_config, load_config, parse_config, AtomicConfigSection, CodeRefsSection,
 LoadedConfig, OrphanKind, OrphanLedgerEntry, PublishableOverrideLedgerEntry,
 SchemaSection, StyleSection, TerminologySection, WorkspaceConfig,
 WorkspaceSection,
};
pub use query::{
 build_envelope, changelog_entries_for_section, query_term, related_sections,
 related_sections_with_atomic, section_by_id, workspace_section_id_set,
 ChangelogEntryView, CrossRefView, QueryEnvelope, QueryTermError, RelatedSections,
 SectionView, TermHit, TermMode, TermQuery, TermScope, TermTargetKind,
};
pub use t2::{frozen_ledger_atomic, frozen_ledger_jaccard, T2ValidationError};
pub use mutate::{
 add_cross_ref, append_changelog_entry, set_section_body,
 set_section_decision_status, MutateError, MutateErrorKind, MutateReceipt,
};
pub use style::{
 check_style, default_ruleset, default_ruleset_with_config, glossary_from_config, StyleRule,
 StyleScope, StyleSeverity, StyleThreshold, StyleTier,
 StyleViolation,
};
pub use atomic::{
 add_inventory_entry, add_section_caveat, add_section_example,
 add_section_implementation, append_changelog_entry_v2,
 emit_publishable_override_ledger_draft, remove_inventory_entry,
 remove_section, remove_section_implementation,
 set_changelog_publishable_carry_forward_bullets,
 set_changelog_publishable_changes_bullets,
 set_changelog_publishable_decision_summary,
 set_changelog_publishable_impact_refs,
 set_changelog_publishable_verification_bullets, set_inventory_section_ref,
 set_inventory_status, set_section_alternatives,
 set_section_decision_status_atomic, set_section_impact_scope,
 set_section_inputs, set_section_intent, set_section_outputs,
 set_section_parent_doc, set_section_parent_section, set_section_rationale,
 set_section_title, AtomicChangelogEntry, AtomicMutateError,
 AtomicMutateReceipt, AtomicSection, AtomicStore, AtomicStoreError,
 ExampleBlock, Implementation, InventoryEntry, InventoryStatus,
 RejectedAlternative,
};
// Round 287 — atomic `add_section` lives at `atomic::add_section` to avoid
// the legacy `mutate::add_section` name collision (Phase H will remove the
// legacy variant; until then callers use the module-qualified path).
pub use render::{render_changelog_entry, render_section, RenderError};
pub use commit_ledger::{diff as commit_ledger_diff, CommitLedgerDriftReport};
pub use redact::{
 redact_term, RedactError, RedactMode, RedactRequest, RedactScope, RedactionHit,
 RedactionReport,
};
