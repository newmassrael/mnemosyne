//! Content-anchored story prose — the provenance-bound carrier for the narration
//! a player reads that is NOT a store fact (R755 design, Phase 1).
//!
//! A [`Passage`] is the prose sibling of [`Line`](crate::Line): where a `Line`
//! projects a store `fact_id`, a `Passage` projects a resolvable
//! [`ContentAnchor`] into the authored content-SSOT (the manuscript today, an
//! EPUB later). So authored prose is ANCHORED — never fabricated at render —
//! while staying OUT of the fact store (a ladder question or an epigraph asserts
//! no world-fact). This closes the kernel's "no invented narrative" guarantee
//! over the story prose, not only the fact-clue overlay.
//!
//! # The provenance contract (invention is unrepresentable, for prose)
//!
//! `Passage` has crate-private fields, no public constructor, and no
//! `Deserialize`. The ONLY path to one is [`Passage::resolve`], which joins an
//! anchor to its verbatim text through a [`ContentSource`] — so a downstream
//! crate can READ a passage but can never build one from a free string. The
//! `text` is what the content-SSOT holds at the anchor, not an invented sentence
//! (the R643 `Line` forgery guard, applied to prose).
//!
//! Phase 1 is the type + the fail-loud resolution + the manuscript
//! (`Locator::Prefix`) resolver; the consumer SUPPLIES the content-SSOT. Later
//! phases move the anchors + source into the store (R755 Phase 3) and swap the
//! locator to an EPUB CFI (Phase 4) — a substrate swap the abstraction absorbs.

use std::collections::HashMap;
use std::fmt;

// `ContentAnchor` + `Locator` are Layer-0 pointers (R756): they live in
// `mnemosyne-core` so the store (`mnemosyne-atomic`'s `content_excerpt`) and the
// engine share ONE anchor type with no atomic↔engine dependency. Re-exported
// below so `mnemosyne_engine::{ContentAnchor, Locator}` stays the public path;
// the resolution machinery (`Passage`, `ContentSource`, `PrefixSlices`) is here.
pub use mnemosyne_core::{ContentAnchor, Locator};

/// A provenance-bound unit of authored narration — the prose sibling of
/// [`Line`](crate::Line). Crate-private fields, no public constructor, no
/// `Deserialize`: the sole path to one is [`Passage::resolve`], so a downstream
/// crate READS a passage but can never fabricate one from a free string. The
/// forgery guard is proven by two `compile_fail` doctests.
///
/// Struct-literal construction does not compile from another crate:
///
/// ```compile_fail
/// use mnemosyne_engine::{ContentAnchor, Locator, Passage};
/// let _ = Passage {
///     anchor: ContentAnchor { source: "m".into(), locator: Locator::Prefix("p".into()) },
///     text: "the engine made this up".to_string(),
/// };
/// ```
///
/// Nor does clone-and-overwrite — a real passage is freely readable, but its
/// content cannot be mutated:
///
/// ```compile_fail
/// fn forge(seed: &mnemosyne_engine::Passage) -> String {
///     let mut forged = seed.clone();
///     forged.text = "the engine made this up".to_string();
///     forged.text
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Passage {
    /// The content-SSOT anchor this passage projects — its provenance.
    pub(crate) anchor: ContentAnchor,
    /// The authored text at the anchor: a projection of the content-SSOT, never
    /// a free string.
    pub(crate) text: String,
}

impl Passage {
    /// Resolve an anchor against a content source into a provenance-bound
    /// passage — the ONLY constructor. The text is whatever the source holds at
    /// the anchor; a passage cannot be built from a bare string.
    ///
    /// # Errors
    ///
    /// [`ProseError`] if the source does not resolve the anchor (a dangling
    /// anchor — the prose analog of a stale fact locator, fail-loud).
    pub fn resolve(anchor: ContentAnchor, source: &impl ContentSource) -> Result<Self, ProseError> {
        let text = source.resolve(&anchor)?;
        Ok(Self { anchor, text })
    }

    /// Build a passage from a store `content_excerpt` (R757 P3b) — the STORE-CACHE
    /// model. The excerpt's (anchor, text) was manuscript-resolved and sha-pinned at
    /// ingestion (R756 P3a `import-content-excerpts`), so the passage is trusted the
    /// way a [`crate::Line`] is (a store projection) WITHOUT the engine holding the
    /// manuscript — this is what lets a manuscript-less consumer get provenance-bound
    /// prose. Crate-private: a consumer obtains a `Passage` only via
    /// [`crate::store_passages`] (which reads the real store), never by handing in a
    /// fabricated excerpt, so the forgery guard holds.
    pub(crate) fn from_excerpt(excerpt: &mnemosyne_atomic::ContentExcerpt) -> Self {
        Self {
            anchor: excerpt.anchor.clone(),
            text: excerpt.text.clone(),
        }
    }

    /// The authored text at the anchor.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The content-SSOT anchor this passage projects — its provenance.
    #[must_use]
    pub fn anchor(&self) -> &ContentAnchor {
        &self.anchor
    }
}

/// The authored content-SSOT a consumer supplies — a manuscript / EPUB the kernel
/// resolves anchors against. The consumer implements it over whatever it loads
/// (a Markdown manuscript, an EPUB); the kernel resolves anchors THROUGH it so a
/// [`Passage`]'s text is always what the source holds at the anchor.
/// [`PrefixSlices`] is the engine's resolver for the manuscript
/// ([`Locator::Prefix`]) case.
pub trait ContentSource {
    /// Resolve an anchor to its verbatim text in this source.
    ///
    /// # Errors
    ///
    /// [`ProseError`] if the anchor does not resolve here (unknown source,
    /// unsupported locator, or a prefix the document does not contain).
    fn resolve(&self, anchor: &ContentAnchor) -> Result<String, ProseError>;
}

/// A failure resolving a [`Passage`] — fail-loud, never a silent empty passage.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProseError {
    /// The anchor names a different content-SSOT document than this source holds.
    SourceMismatch {
        /// The document id the anchor named.
        anchor_source: String,
        /// The document id this source actually is.
        source: String,
    },
    /// A [`Locator::Prefix`] prefix does not occur in the source document — the
    /// anchor dangles (a typo, or the manuscript changed under it).
    PrefixNotFound {
        /// The source document.
        source: String,
        /// The verbatim prefix the document does not contain.
        prefix: String,
    },
    /// This source cannot resolve the anchor's locator kind (e.g. an EPUB
    /// [`Locator::Cfi`] handed to the manuscript-prefix resolver — R755 Phase 4).
    UnsupportedLocator {
        /// The source document.
        source: String,
    },
}

impl fmt::Display for ProseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProseError::SourceMismatch {
                anchor_source,
                source,
            } => write!(
                f,
                "anchor names document `{anchor_source}` but this source is `{source}`"
            ),
            ProseError::PrefixNotFound { source, prefix } => write!(
                f,
                "prefix `{prefix}` does not occur in document `{source}` — the anchor dangles"
            ),
            ProseError::UnsupportedLocator { source } => write!(
                f,
                "document `{source}` cannot resolve this locator kind (no CFI resolver yet)"
            ),
        }
    }
}

impl std::error::Error for ProseError {}

/// The engine's [`ContentSource`] for the manuscript-anchor model: one
/// content-SSOT document (its text) sliced by its [`Locator::Prefix`] anchors.
/// Each anchor's passage runs from the first occurrence of its verbatim prefix to
/// the next anchor's prefix (or the document end). This is the derive-prose
/// slicing discipline (tide's `derive-prose.py`) brought into the kernel: the
/// slice text is verbatim from the supplied document, so a passage cannot carry a
/// sentence the manuscript does not.
///
/// Slicing + verification happen at construction (order-independent input — the
/// anchors are sorted by where their prefix occurs); [`resolve`](ContentSource::resolve)
/// is then a lookup. A prefix the document lacks is a fail-loud construction
/// error, never a silent drop.
#[derive(Debug, Clone)]
pub struct PrefixSlices {
    source: String,
    slices: HashMap<ContentAnchor, String>,
}

impl PrefixSlices {
    /// Slice `text` (the document identified by `source`) by its `Prefix`
    /// anchors. Every anchor must name `source` and be a [`Locator::Prefix`]
    /// whose prefix occurs in `text`.
    ///
    /// # Errors
    ///
    /// [`ProseError::SourceMismatch`] if an anchor names another document;
    /// [`ProseError::UnsupportedLocator`] for a non-prefix locator;
    /// [`ProseError::PrefixNotFound`] if a prefix does not occur in `text`.
    pub fn new(source: &str, text: &str, anchors: &[ContentAnchor]) -> Result<Self, ProseError> {
        // Resolve each anchor to the byte offset where its prefix begins, failing
        // loud on a mismatch or a missing prefix.
        let mut placed: Vec<(usize, &ContentAnchor, &str)> = Vec::with_capacity(anchors.len());
        for anchor in anchors {
            if anchor.source != source {
                return Err(ProseError::SourceMismatch {
                    anchor_source: anchor.source.clone(),
                    source: source.to_string(),
                });
            }
            let Locator::Prefix(prefix) = &anchor.locator else {
                return Err(ProseError::UnsupportedLocator {
                    source: source.to_string(),
                });
            };
            let offset = text
                .find(prefix.as_str())
                .ok_or_else(|| ProseError::PrefixNotFound {
                    source: source.to_string(),
                    prefix: prefix.clone(),
                })?;
            placed.push((offset, anchor, prefix.as_str()));
        }
        // Order by where each prefix occurs — the segment of anchor i runs to the
        // start of anchor i+1 (or the document end). Sort is by offset only; a
        // stable tie-break is irrelevant because a zero-length slice is harmless.
        placed.sort_by_key(|(offset, _, _)| *offset);

        let mut slices = HashMap::with_capacity(placed.len());
        for (i, (offset, anchor, _)) in placed.iter().enumerate() {
            let end = placed.get(i + 1).map_or(text.len(), |(next, _, _)| *next);
            slices.insert((*anchor).clone(), text[*offset..end].to_string());
        }
        Ok(Self {
            source: source.to_string(),
            slices,
        })
    }

    /// The document id this source slices.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }
}

impl ContentSource for PrefixSlices {
    fn resolve(&self, anchor: &ContentAnchor) -> Result<String, ProseError> {
        if anchor.source != self.source {
            return Err(ProseError::SourceMismatch {
                anchor_source: anchor.source.clone(),
                source: self.source.clone(),
            });
        }
        // A prefix anchor absent from the slice map never occurred in the
        // document — the same dangling-prefix failure as construction.
        match &anchor.locator {
            Locator::Prefix(prefix) => {
                self.slices
                    .get(anchor)
                    .cloned()
                    .ok_or_else(|| ProseError::PrefixNotFound {
                        source: self.source.clone(),
                        prefix: prefix.clone(),
                    })
            }
            Locator::Cfi(_) => Err(ProseError::UnsupportedLocator {
                source: self.source.clone(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ContentAnchor, ContentSource, Locator, Passage, PrefixSlices, ProseError};

    const DOC: &str = "manuscript-part1";
    // Three ordered passages in one document; the anchors are given OUT of order
    // to prove the slicer orders by occurrence, not input order.
    const TEXT: &str = "The tide pulls out at dawn. Bunok counts the bells. \
                        Night falls on the flat.";

    fn prefix(text: &str) -> ContentAnchor {
        ContentAnchor {
            source: DOC.to_string(),
            locator: Locator::Prefix(text.to_string()),
        }
    }

    #[test]
    fn from_excerpt_projects_a_store_excerpt_verbatim() {
        // R757 P3b — the store-cache ctor: a passage built from a store excerpt
        // carries its text + anchor verbatim (trusted like a Line; the sha-pin at
        // ingestion + `scan_content_drift` are the store's guard, not re-checked
        // here), so a manuscript-less consumer gets provenance-bound prose.
        let excerpt = mnemosyne_atomic::ContentExcerpt {
            anchor: prefix("The tide"),
            text: "The tide pulls out at dawn.".to_string(),
            text_sha256: String::new(),
        };
        let p = Passage::from_excerpt(&excerpt);
        assert_eq!(p.text(), "The tide pulls out at dawn.");
        assert_eq!(p.anchor().locator, Locator::Prefix("The tide".to_string()));
    }

    #[test]
    fn a_passage_projects_the_manuscript_slice_at_its_anchor() {
        let a_dawn = prefix("The tide");
        let a_bells = prefix("Bunok counts");
        let a_night = prefix("Night falls");
        // Deliberately unsorted input.
        let source = PrefixSlices::new(
            DOC,
            TEXT,
            &[a_night.clone(), a_dawn.clone(), a_bells.clone()],
        )
        .unwrap();

        let dawn = Passage::resolve(a_dawn, &source).unwrap();
        assert_eq!(dawn.text(), "The tide pulls out at dawn. ");
        assert!(matches!(dawn.anchor().locator, Locator::Prefix(_)));

        let bells = Passage::resolve(a_bells, &source).unwrap();
        assert_eq!(bells.text(), "Bunok counts the bells. ");

        // The last anchor's slice runs to the document end.
        let night = Passage::resolve(a_night, &source).unwrap();
        assert_eq!(night.text(), "Night falls on the flat.");
    }

    #[test]
    fn a_prefix_the_manuscript_lacks_is_a_fail_loud_error() {
        // Construction fails loud when a prefix does not occur — no silent drop.
        let err = PrefixSlices::new(DOC, TEXT, &[prefix("A stake was found")]).unwrap_err();
        assert_eq!(
            err,
            ProseError::PrefixNotFound {
                source: DOC.to_string(),
                prefix: "A stake was found".to_string(),
            }
        );
    }

    #[test]
    fn resolving_a_dangling_anchor_fails_loud_not_empty() {
        let source = PrefixSlices::new(DOC, TEXT, &[prefix("The tide")]).unwrap();
        // An anchor never sliced into this source resolves to an error, not "".
        let dangling = prefix("Night falls");
        assert!(matches!(
            Passage::resolve(dangling, &source),
            Err(ProseError::PrefixNotFound { .. })
        ));
    }

    #[test]
    fn an_anchor_for_another_document_is_rejected() {
        let source = PrefixSlices::new(DOC, TEXT, &[prefix("The tide")]).unwrap();
        let foreign = ContentAnchor {
            source: "manuscript-part2".to_string(),
            locator: Locator::Prefix("The tide".to_string()),
        };
        assert!(matches!(
            source.resolve(&foreign),
            Err(ProseError::SourceMismatch { .. })
        ));
    }

    #[test]
    fn the_prefix_resolver_does_not_pretend_to_resolve_a_cfi() {
        let cfi = ContentAnchor {
            source: DOC.to_string(),
            locator: Locator::Cfi("epubcfi(/6/4!/4/2)".to_string()),
        };
        // A CFI in the anchor list is rejected at construction (Phase 4 territory).
        assert!(matches!(
            PrefixSlices::new(DOC, TEXT, std::slice::from_ref(&cfi)),
            Err(ProseError::UnsupportedLocator { .. })
        ));
    }
}
