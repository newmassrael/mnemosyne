//! mnemosyne-engine — the default playable-projection kernel (P1).
//!
//! The Narrative Studio's read-side engine (design:
//! `claudedocs/mnemosyne-engine-design.md`). It consumes `report-playable-world`
//! (a pure read projection of the frozen fact store) and produces a per-world,
//! per-section stream of DISCLOSED narrative — every line provenance-bound to
//! the store fact it projects. The store is never mutated; this crate depends
//! only inward (`ops` -> `validate` -> `core`) and nothing depends back on it,
//! so it is presentation- and execution-agnostic (a renderer / a statechart
//! executor consume its output; it consumes neither).
//!
//! # The provenance contract (why invention is unrepresentable)
//!
//! [`Line`] is the ONLY carrier of narrative content, and it is
//! `#[non_exhaustive]` with no public constructor: a downstream presentation
//! crate can READ a line's `fact_id`/`text` but can never FABRICATE one from a
//! free string. So a renderer cannot put a sentence on the narrative surface
//! that no store fact backs — the class of bug that shipped through a consumer's
//! chrome template (the tide field report: an engine that "invented narrative"
//! with zero store backing) is a compile error here, not a test to remember.
//! This is the R643 detectable->unrepresentable doctrine moved to the engine's
//! type boundary.
//!
//! A withheld fact emits no locator, so it never becomes a [`Line`]: the store
//! filters disclosure additively, and this kernel never re-implements a
//! subtractive withhold filter.

mod gate;
mod overrides;
mod projection;
mod prose;
mod quest;
#[cfg(test)]
mod test_support;
mod types;

pub use gate::GateViolation;
pub use mnemosyne_core::{DisclosureMode, MAIN_BRANCH};
pub use mnemosyne_validate::continuity::QuestState;
pub use overrides::{DefaultOverrides, EngineOverrides, OverrideLoadError, StaticOverrides};
pub use projection::PlayableProjection;
pub use prose::{ContentAnchor, ContentSource, Locator, Passage, PrefixSlices, ProseError};
pub use quest::{
    QuestCompletionView, QuestGateViolation, QuestProjection, QuestView, QuestWorldView,
};
pub use types::{Door, Fork, Interactivity, Line, Rung, SceneView};

use std::fmt;

/// A fault projecting the playable world. Fail-loud: the kernel never silently
/// drops a locator or invents a fallback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    /// A locator named a `fact_id` absent from its world's `begins` events —
    /// the playable-world read is stale relative to the store (regenerate it).
    /// A dangling pointer is a hard error, never a silent drop.
    LocatorFactMissing {
        /// The world-line whose locator dangled.
        world: String,
        /// The `fact_id` the locator named but no `begins` event carried.
        fact_id: String,
    },
    /// The underlying `report-playable-world` projection failed (an unregistered
    /// world, a typo'd telling, an unreadable store).
    Projection(String),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineError::LocatorFactMissing { world, fact_id } => write!(
                f,
                "locator points at `{fact_id}` but world `{world}` has no such begins event \
                 — playable-world is stale relative to the store"
            ),
            EngineError::Projection(msg) => {
                write!(f, "playable-world projection failed: {msg}")
            }
        }
    }
}

impl std::error::Error for EngineError {}
