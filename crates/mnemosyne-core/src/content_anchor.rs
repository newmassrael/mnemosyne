//! Content-SSOT anchors — the medium-neutral pointer a provenance-bound passage
//! projects (R755 engine `Passage`, R756 store `content_excerpt`).
//!
//! A [`ContentAnchor`] names WHERE authored prose lives (a manuscript file id or
//! an EPUB spine href) and WHERE within it ([`Locator`]). It is a Layer-0 pointer
//! — a source id + a position — so BOTH the store (`mnemosyne-atomic`'s
//! `ContentExcerpt` on a Section) and the engine (`mnemosyne-engine`'s `Passage`)
//! carry the SAME anchor type, with no `atomic`↔`engine` dependency. The engine
//! re-exports these (`mnemosyne_engine::{ContentAnchor, Locator}`), so its public
//! API is unchanged; the resolution machinery (`Passage`, `ContentSource`,
//! `PrefixSlices`) stays in the engine.

/// Where an authored passage lives in a content-SSOT. Abstract over the
/// substrate: a verbatim text prefix into a manuscript (today), or an EPUB CFI
/// (R755 Phase 4) — the swap is a new [`Locator`] variant, not a redesign. A
/// CONSUMER INPUT (authored data), so it is plainly constructible / serializable;
/// the provenance guarantee is that a resolver rejects an anchor the source does
/// not resolve, not that the anchor is unconstructible.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct ContentAnchor {
    /// The content-SSOT document this anchor points into (a manuscript file id,
    /// or an EPUB spine href).
    pub source: String,
    /// The position within that document.
    pub locator: Locator,
}

/// The position of a passage within its content-SSOT document.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub enum Locator {
    /// A verbatim text prefix into the source (the manuscript-anchor model): the
    /// passage begins at the first occurrence of this exact prefix and runs to
    /// the next anchor (or the document end). Resolved by the engine's
    /// `PrefixSlices`.
    Prefix(String),
    /// An EPUB Canonical Fragment Identifier (R755 Phase 4 — no resolver yet).
    Cfi(String),
}
