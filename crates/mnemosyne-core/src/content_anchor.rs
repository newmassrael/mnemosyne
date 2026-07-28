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

/// Where a [`Locator::Prefix`] lands in a document (Round 815).
///
/// THE resolution rule for a prefix coordinate, shared by everything that holds
/// one. It was born inside the engine's slicer, where Round 766 established the
/// three-way verdict: a prefix naming NO place and a prefix naming TWO are both
/// failures, and only a unique hit is a coordinate. It lives here because the
/// store holds prefix coordinates too — the section ladder's rungs — and had no
/// way to ask this question without depending on the engine, so it asked
/// nothing and accepted a rung pointing at prose that does not exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefixResolution {
    /// Exactly one occurrence, at this byte offset.
    Unique(usize),
    /// No occurrence — the coordinate names prose the document does not hold.
    NotFound,
    /// More than one occurrence, so the coordinate does not say WHICH. The count
    /// is truthful (the walk finishes) rather than a bare "several".
    Ambiguous(usize),
}

/// Resolve `prefix` against `text` — see [`PrefixResolution`].
#[must_use]
pub fn resolve_prefix(text: &str, prefix: &str) -> PrefixResolution {
    let mut hits = text.match_indices(prefix).map(|(at, _)| at);
    let Some(offset) = hits.next() else {
        return PrefixResolution::NotFound;
    };
    if hits.next().is_some() {
        // The first two are already counted; finish the walk only to report a
        // truthful number.
        return PrefixResolution::Ambiguous(2 + hits.count());
    }
    PrefixResolution::Unique(offset)
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
