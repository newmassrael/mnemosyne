//! Canonical Layer-0 fact model — the domain-agnostic skeleton every
//! medium adapter and persistence index shares.
//!
//! This module owns the medium-neutral shape of a versioned fact: the
//! bitemporal key envelope ([`FactKey`]) and the per-entity skeleton
//! ([`SectionSkeleton`]) carrying only attributes that exist regardless of
//! medium. Rich, medium-shaped content (a design_doc's rationale, a
//! fiction's scene, an ADR's decision) lives in the Layer-1 adapter, never
//! here — keeping Layer 0 ignorant of any medium (ARCHITECTURE.md North Star
//! + the convergence-debt section).

use serde::{Deserialize, Serialize};

use crate::DecisionStatus;

/// Bitemporal + branch identity coordinate shared by every versioned typed
/// fact. The triple `(branch_id, entity_id, valid_from)` is the composite key
/// the persistence index (`mnemosyne-store`) encodes as a 24-byte big-endian
/// key. Hoisted into the domain core (Round 323 — Convergence A) so the
/// bitemporal envelope is defined once instead of copy-pasted across every
/// fact struct in `mnemosyne-facts`.
///
/// Relations (e.g. CrossRef) use a distinct key shape (source/target entity
/// ids) and intentionally do not carry a `FactKey`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FactKey {
    pub branch_id: u64,
    pub entity_id: u64,
    /// Valid-time axis lower bound — when the fact became true in the modeled
    /// world. Transaction-time is tracked by the store, not carried here.
    pub valid_from: u64,
}

/// Canonical Layer-0 Section skeleton — the medium-neutral attributes every
/// Section fact carries no matter which medium authored it (Round 325 —
/// Convergence A3).
///
/// A `design_doc` section, a fiction scene, and an ADR all have a title, an
/// owning doc, an optional parent, a decision lifecycle status, and a set of
/// outbound cross-refs. Those five attributes are the skeleton. Everything
/// medium-shaped (a design_doc's `intent`/`rationale`/`normative_excerpt`,
/// etc.) belongs to the Layer-1 adapter payload, not here.
///
/// The on-disk authoring representation (the JSON atomic store, the
/// design_doc adapter at `mnemosyne-atomic`) embeds this struct via
/// `#[serde(flatten)]`, so the skeleton fields serialize inline with the
/// adapter's content fields — the JSON shape is unchanged by the split. The
/// bitemporal [`FactKey`] is the *index/log key*, assigned at projection
/// time, and is deliberately **not** part of the authoring skeleton.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SectionSkeleton {
    /// Heading title. Default = "" during the pre-backfill transitional
    /// state (Round 287 outline lift).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
    /// Owning doc identifier (workspace-relative path or doc-id). Default =
    /// "" during the transitional state.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub parent_doc: String,
    /// Nullable parent section_id. `None` = top-level section in its doc.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_section: Option<String>,
    /// Outbound cross-ref list (target section_id without the `§` prefix).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub impact_scope: Vec<String>,
    /// Atomic decision_status override (Round 265). `None` = fall back to the
    /// parser-derived status; `Some(_)` = the store authoritatively declares
    /// the section's lifecycle state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision_status: Option<DecisionStatus>,
}
