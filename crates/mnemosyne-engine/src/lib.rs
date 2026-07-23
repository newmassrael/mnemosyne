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
pub use mnemosyne_core::{DisclosureMode, Modality, MAIN_BRANCH};
pub use mnemosyne_validate::continuity::QuestState;
pub use overrides::{DefaultOverrides, EngineOverrides, OverrideLoadError, StaticOverrides};
pub use projection::PlayableProjection;
pub use prose::{ContentAnchor, ContentSource, Locator, Passage, PrefixSlices, ProseError};
pub use quest::{
    QuestCompletionView, QuestGateViolation, QuestProjection, QuestView, QuestWorldView,
};
pub use types::{CastMember, ChoiceEntityRef, Door, Fork, Interactivity, Line, Rung, SceneView};

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

/// Read every registered entity's declared kind from the store — `entity_id ->
/// kind` over the whole registry.
///
/// The engine never classifies entities by kind for its OWN logic — a consumer
/// splits its registries by kind ([`Interactivity::objects`] is FED, not derived
/// here). This is the raw store read a consumer needs to VALIDATE its kind
/// registry against the store: tide's object/place gates ask "does every store
/// `kind:object` have a screen name, and is every named id a real store object
/// of that kind." A read-through of [`mnemosyne_ops::entity_kinds`] so the
/// consumer talks only to its kernel, not past it into `ops`.
///
/// # Errors
///
/// [`EngineError::Projection`] if the store (or its sidecar) cannot be read.
pub fn store_entity_kinds(
    workspace_root: &std::path::Path,
) -> Result<std::collections::BTreeMap<String, String>, EngineError> {
    mnemosyne_ops::entity_kinds(workspace_root, None)
        .map_err(|e| EngineError::Projection(e.to_string()))
}

/// Provenance-bound narrative prose FROM THE STORE (R757 P3b) — `section_id ->
/// Passage`. Reads each section's `content_excerpt` (R756 P3a) via
/// [`mnemosyne_ops::section_content_excerpts`] and projects it with
/// `Passage::from_excerpt` (the store-cache model), so a manuscript-less consumer
/// (a generic renderer, pinion) gets prose bound to its manuscript anchor WITHOUT
/// the engine holding the manuscript. The excerpt was sha-pinned at ingestion and
/// is trusted like a [`Line`]; drift is `scan_content_drift`'s separate offline
/// guard. Sections with no excerpt are omitted.
///
/// This is the store-owned generalization of a per-consumer anchor file (R756 P3):
/// the narrative prose anchor + projected text live in the store's Section, so the
/// engine hands any consumer the same provenance-bound `Passage` with no manuscript
/// and no anchor file of its own.
///
/// # Errors
///
/// [`EngineError::Projection`] if the store (or its sidecar) cannot be read.
pub fn store_passages(
    workspace_root: &std::path::Path,
) -> Result<std::collections::HashMap<String, Passage>, EngineError> {
    let excerpts = mnemosyne_ops::section_content_excerpts(workspace_root, None)
        .map_err(|e| EngineError::Projection(e.to_string()))?;
    Ok(excerpts
        .into_iter()
        .map(|(id, ex)| (id, Passage::from_excerpt(&ex)))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::{store_entity_kinds, store_passages};
    use tempfile::TempDir;

    /// The kernel's read-through of the store's entity registry — `id -> kind`
    /// for the whole registry, what a consumer validates its own kind registry
    /// against. (The read lives in `mnemosyne_ops::entity_kinds`; this proves the
    /// kernel re-export tide's object/place gates call.)
    #[test]
    fn store_entity_kinds_reads_the_registry_through_the_kernel() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::write(
            root.join("mnemosyne.toml"),
            "[workspace]\nroot = \".\"\n\n[atomic]\nsidecar_path = \"store.json\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join("store.json"),
            r#"{"schema_version":39,"sections":{},"frames":{},"narrative_facts":{},
               "entities":{"ent-post":{"kind":"object"},"ent-weir":{"kind":"place"}}}"#,
        )
        .unwrap();
        let kinds = store_entity_kinds(root).expect("kernel reads the store kinds");
        assert_eq!(kinds.get("ent-post").map(String::as_str), Some("object"));
        assert_eq!(kinds.get("ent-weir").map(String::as_str), Some("place"));
        assert_eq!(kinds.len(), 2);
    }

    /// R757 P3b — the kernel projects the store's `content_excerpt`s into
    /// provenance-bound `Passage`s, so a manuscript-less consumer gets narrative
    /// prose FROM THE STORE. A section with no excerpt yields no passage (no
    /// invented prose), the fail-loud omission.
    #[test]
    fn store_passages_projects_content_excerpts_from_the_store() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::write(
            root.join("mnemosyne.toml"),
            "[workspace]\nroot = \".\"\n\n[atomic]\nsidecar_path = \"store.json\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join("store.json"),
            r#"{"schema_version":41,"frames":{},"narrative_facts":{},"entities":{},
               "sections":{
                 "d01-nat":{"content_excerpt":{
                   "anchor":{"source":"MANUSCRIPT.md","locator":{"Prefix":"지운은"}},
                   "text":"지운은 둑에 발을 올렸다.","text_sha256":""}},
                 "d02-nat":{}
               }}"#,
        )
        .unwrap();
        let passages = store_passages(root).expect("kernel reads store excerpts");
        // The section WITH an excerpt projects a provenance-bound passage (text +
        // its manuscript anchor, both from the store — no manuscript needed here).
        let p = passages.get("d01-nat").expect("d01-nat has a passage");
        assert_eq!(p.text(), "지운은 둑에 발을 올렸다.");
        assert_eq!(p.anchor().source, "MANUSCRIPT.md");
        // The section WITHOUT an excerpt is omitted — the kernel invents nothing.
        assert!(!passages.contains_key("d02-nat"));
        assert_eq!(passages.len(), 1);
    }
}
