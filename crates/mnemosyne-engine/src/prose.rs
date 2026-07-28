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
//! # The provenance contract (invention is unreachable by reading, for prose)
//!
//! `Passage` has crate-private fields, no public constructor, and no
//! `Deserialize`. No READING path constructs one, and a downstream crate cannot
//! write a struct literal or overwrite a clone — both are compile errors, proven
//! below. The `text` is what the content-SSOT holds at the anchor, not an
//! invented sentence (the R643 `Line` forgery guard, applied to prose).
//!
//! Until Round 791 this section was headed "invention is unrepresentable, for
//! prose" and said [`Passage::resolve`] was the ONLY path to one. That stopped
//! being true when this round added [`passages_from_parts`] so the kernel could
//! bake a consumer's passages — the same opening the line axis has carried since
//! Round 769 without recording it. Baking cannot be done without it: generated
//! source lands in the consumer's crate, so it can hold no capability the
//! consumer lacks. **The full contract is stated once in
//! [`crate::baked_ingestion`]**; what survives here is that invention is
//! unreachable by reading, impossible by accident, and visible in the consumer's
//! own source when it happens.
//!
//! Phase 1 is the type + the fail-loud resolution + the manuscript
//! (`Locator::Prefix`) resolver; the consumer SUPPLIES the content-SSOT. Later
//! phases move the anchors + source into the store (R755 Phase 3) and swap the
//! locator to an EPUB CFI (Phase 4) — a substrate swap the abstraction absorbs.

use std::borrow::Cow;
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
/// `Deserialize`: no reading path builds one, so a downstream crate READS a
/// passage rather than writing its content. Two `compile_fail` doctests prove it.
///
/// **What those doctests do NOT cover** (Round 791): the baked-ingestion door
/// [`passages_from_parts`], whose parts type has public fields. They remain worth
/// having — they close the paths a consumer reaches for by accident — but they
/// are not the whole guard, and [`crate::baked_ingestion`] states what is.
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
    ///
    /// `Cow<'static, str>` since Round 793, and it is an IMPLEMENTATION DETAIL —
    /// [`Passage::text`] still returns `&str`, so no caller sees it. What it buys
    /// is that a baked passage POINTS at the literal in the binary instead of
    /// copying it: measured at 98.2% of the bytes a fully borrowed passage saves
    /// (Round 792), while [`Passage::resolve`] and [`Passage::from_excerpt`] keep
    /// owning because they build their text at run time.
    ///
    /// One type with two ownership modes rather than two types, which is the
    /// Round 785 decision on the projection axis, reached again here for the same
    /// reason: the split would be permanent API surface for every consumer.
    pub(crate) text: Cow<'static, str>,
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
        Ok(Self {
            anchor,
            text: Cow::Owned(text),
        })
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
            text: Cow::Owned(excerpt.text.clone()),
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

    /// Emit this passage as plain data (Round 791) — the bake half of the
    /// build-time seam, the sibling of [`Line::to_part`](crate::Line::to_part).
    #[must_use]
    pub fn to_part(&self) -> PassagePart {
        PassagePart {
            anchor: self.anchor.clone(),
            text: self.text.clone(),
        }
    }
}

/// A [`Passage`] as plain data (Round 791) — the emit/ingest mirror, for the same
/// reason [`LinePart`](crate::LinePart) exists: `Passage` has crate-private
/// fields, so generated code in another crate cannot construct one, and a baked
/// artifact carries this instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PassagePart {
    /// The content-SSOT anchor — the passage's provenance.
    pub anchor: ContentAnchor,
    /// The authored text at that anchor.
    ///
    /// `Cow<'static, str>` since Round 793, so a BAKED part can carry
    /// `Cow::Borrowed` of the literal the emitter wrote and the passage built
    /// from it points at the binary rather than copying it. `Passage::to_part`
    /// on a live passage yields `Cow::Owned`, and the two compare equal by
    /// CONTENT, which is what keeps the live-to-baked round trip an equality
    /// rather than a shape check.
    pub text: Cow<'static, str>,
}

/// A whole passage set as plain data (Round 791) — what a build-time bake writes
/// out and reads back, keyed by section id.
///
/// Deterministic: [`passages_to_parts`] emits in sorted key order, so the same
/// store produces byte-identical generated source and a rebuild that changed
/// nothing changes nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PassagesParts {
    /// `section id -> the passage anchored there`, in sorted key order.
    pub passages: Vec<(String, PassagePart)>,
}

/// Emit a passage set as plain data (Round 791) — the bake half.
#[must_use]
pub fn passages_to_parts(passages: &HashMap<String, Passage>) -> PassagesParts {
    let mut out: Vec<(String, PassagePart)> = passages
        .iter()
        .map(|(section, passage)| (section.clone(), passage.to_part()))
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    PassagesParts { passages: out }
}

/// Ingest baked passage parts (Round 791) — the read half.
///
/// **This is a baked-ingestion door. The contract every such door carries is
/// stated once, in [`crate::baked_ingestion`], and this one adds nothing to
/// it.** Read that before calling this, and before adding a fourth door.
#[must_use]
pub fn passages_from_parts(parts: PassagesParts) -> HashMap<String, Passage> {
    parts
        .passages
        .into_iter()
        .map(|(section, part)| {
            (
                section,
                Passage {
                    anchor: part.anchor,
                    text: part.text,
                },
            )
        })
        .collect()
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
    /// A [`Locator::Prefix`] prefix occurs MORE THAN ONCE in the source document
    /// (Round 766): the coordinate is ambiguous, and picking an occurrence would
    /// be picking a slice on the reader's behalf. A prefix that must be extended
    /// to name one place is an authoring fix, not something to guess at.
    PrefixAmbiguous {
        /// The source document.
        source: String,
        /// The verbatim prefix that occurs more than once.
        prefix: String,
        /// How many times it occurs.
        occurrences: usize,
    },
    /// Two anchors declare the SAME coordinate (Round 766) — one slice would
    /// silently overwrite the other in the resolved map, so the duplicate is
    /// refused instead of swallowed.
    DuplicateAnchor {
        /// The source document.
        source: String,
        /// The verbatim prefix declared twice.
        prefix: String,
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
            ProseError::PrefixAmbiguous {
                source,
                prefix,
                occurrences,
            } => write!(
                f,
                "prefix `{prefix}` occurs {occurrences} times in document `{source}` — \
                 the coordinate names no single place; extend it"
            ),
            ProseError::DuplicateAnchor { source, prefix } => write!(
                f,
                "prefix `{prefix}` is declared twice for document `{source}` — \
                 one slice would swallow the other"
            ),
        }
    }
}

impl std::error::Error for ProseError {}

/// The engine's [`ContentSource`] for the manuscript-anchor model: one
/// content-SSOT document (its text) sliced by its [`Locator::Prefix`] anchors.
/// Each anchor's passage runs from the sole occurrence of its verbatim prefix to
/// the next anchor's prefix (or the document end). This is the derive-prose
/// slicing discipline (tide's `derive-prose.py`) brought into the kernel: the
/// slice text is verbatim from the supplied document, so a passage cannot carry a
/// sentence the manuscript does not.
///
/// Slicing + verification happen at construction (order-independent input — the
/// anchors are sorted by where their prefix occurs); [`resolve`](ContentSource::resolve)
/// is then a lookup. THREE construction faults are loud rather than silent: a
/// prefix the document lacks ([`ProseError::PrefixNotFound`]), a prefix naming
/// more than one place ([`ProseError::PrefixAmbiguous`], Round 766), and the same
/// anchor declared twice ([`ProseError::DuplicateAnchor`], Round 766).
///
/// Whether a caller's DECLARED order must agree with the prose order is not asked
/// here — an anchor set may legitimately arrive unordered (from a map), and only
/// a caller with ordered semantics can say. Such a caller reads
/// [`in_prose_order`](Self::in_prose_order) and judges for itself.
#[derive(Debug, Clone)]
pub struct PrefixSlices {
    source: String,
    slices: HashMap<ContentAnchor, String>,
    /// The anchors in the order their prefixes occur in the document — computed
    /// during the one locate pass so an ordered caller never repeats the search.
    in_prose_order: Vec<ContentAnchor>,
    /// The offsets of the anchors as `new` received them — see
    /// [`declared_offsets`](Self::declared_offsets).
    declared_offsets: Vec<usize>,
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
        // loud on a mismatch, a missing prefix, or an AMBIGUOUS one.
        //
        // Round 766 — this used to take `text.find(prefix)`, i.e. the FIRST
        // occurrence, and to `sort_by_key` the anchors into prose order. Both were
        // silent: a prefix naming two places resolved to whichever came first, and
        // a declaration order disagreeing with the document was quietly rewritten.
        // The first consumer's own slicer rejected all three cases (ambiguous
        // prefix, out-of-order declaration, duplicate anchor), so promoting its
        // rule into this primitive without these checks would have moved a rule
        // into a weaker home — a regression wearing a promotion's clothes. The
        // three invariants live HERE now, once, for every consumer.
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
            // Round 815 — the three-way verdict moved to `mnemosyne_core` so the
            // STORE can ask it too (its ladder rungs are prefix coordinates and
            // it had no way to check them). Same rule, one home; this arm only
            // dresses the verdict in this crate's error type.
            let offset = match mnemosyne_core::resolve_prefix(text, prefix.as_str()) {
                mnemosyne_core::PrefixResolution::Unique(at) => at,
                mnemosyne_core::PrefixResolution::NotFound => {
                    return Err(ProseError::PrefixNotFound {
                        source: source.to_string(),
                        prefix: prefix.clone(),
                    });
                }
                mnemosyne_core::PrefixResolution::Ambiguous(occurrences) => {
                    return Err(ProseError::PrefixAmbiguous {
                        source: source.to_string(),
                        prefix: prefix.clone(),
                        occurrences,
                    });
                }
            };
            if placed.iter().any(|(_, seen, _)| *seen == anchor) {
                return Err(ProseError::DuplicateAnchor {
                    source: source.to_string(),
                    prefix: prefix.clone(),
                });
            }
            placed.push((offset, anchor, prefix.as_str()));
        }
        // Order by where each prefix occurs — the segment of anchor i runs to the
        // start of anchor i+1 (or the document end). The INPUT order is
        // deliberately free: an anchor set may arrive from a map, and a generic
        // slicer has no business demanding the caller's iteration order match a
        // document. Whether a DECLARED order must agree with the prose order is a
        // question only a caller with ordered semantics (a ladder's rungs) can
        // ask, so it asks it with `in_prose_order` rather than being answered
        // here for every caller.
        // Captured BEFORE the sort: the offsets as the caller declared them, which
        // is what an ordered caller judges with (Round 821). Recording the input
        // order is a fact about the input, not a rule about it — the rule stays
        // out of this generic slicer exactly as Round 766 decided.
        let declared_offsets: Vec<usize> = placed.iter().map(|(offset, _, _)| *offset).collect();
        placed.sort_by_key(|(offset, _, _)| *offset);

        let mut slices = HashMap::with_capacity(placed.len());
        for (i, (offset, anchor, _)) in placed.iter().enumerate() {
            let end = placed.get(i + 1).map_or(text.len(), |(next, _, _)| *next);
            slices.insert((*anchor).clone(), text[*offset..end].to_string());
        }
        Ok(Self {
            source: source.to_string(),
            slices,
            declared_offsets,
            in_prose_order: placed
                .into_iter()
                .map(|(_, anchor, _)| anchor.clone())
                .collect(),
        })
    }

    /// Where each anchor GIVEN TO [`new`](Self::new) begins, in the order it was
    /// given (Round 821). The companion to [`in_prose_order`](Self::in_prose_order):
    /// that one answers "what order does the document put these in", this one
    /// answers "where did the ones I declared land", and an ordered caller needs
    /// both halves to judge its declaration.
    ///
    /// It exists so the judgement can be made by
    /// [`mnemosyne_core::declared_order_break`] — the one home for the ladder-order
    /// rule, shared with the scan that re-reads the stored ladder — rather than by
    /// each caller comparing sequences for itself.
    #[must_use]
    pub fn declared_offsets(&self) -> &[usize] {
        &self.declared_offsets
    }

    /// The anchors this source holds, in the order their prefixes occur in the
    /// document (Round 766). The locating is done once, here, so a caller with
    /// ORDERED semantics — a ladder, whose nth rung must be the nth passage —
    /// judges its declared order against the prose without re-implementing the
    /// search and drifting from it.
    #[must_use]
    pub fn in_prose_order(&self) -> &[ContentAnchor] {
        &self.in_prose_order
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
    use std::borrow::Cow;

    use super::{
        passages_from_parts, ContentAnchor, ContentSource, Locator, Passage, PassagePart,
        PassagesParts, PrefixSlices, ProseError,
    };

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

    /// Round 793 — the ingestion door must PRESERVE a borrow, not merely accept
    /// one.
    ///
    /// This is the half that can regress in silence. `Cow` compares by CONTENT,
    /// so every other test in this workspace — the round trip, the live-to-baked
    /// equality, the emitter's own fixture — passes identically whether the baked
    /// passage points at the literal or copied it. A door written
    /// `part.text.into_owned().into()` would look correct and undo the round, and
    /// only the discriminant says otherwise.
    ///
    /// The owning direction is asserted too: a part that OWNS must stay owned, or
    /// the door is quietly leaking one representation into the other rather than
    /// carrying what it was given.
    #[test]
    fn the_ingestion_door_carries_the_borrow_it_was_given() {
        let anchor = ContentAnchor {
            source: "M.md".to_string(),
            locator: Locator::Prefix("이름을".to_string()),
        };
        let borrowed = passages_from_parts(PassagesParts {
            passages: vec![(
                "sc-01".to_string(),
                PassagePart {
                    anchor: anchor.clone(),
                    text: Cow::Borrowed("물때가 셈한다."),
                },
            )],
        });
        assert!(
            matches!(borrowed["sc-01"].text, Cow::Borrowed(_)),
            "a baked passage copied the literal instead of pointing at it, which \
             is the whole of Round 793 and no content assertion can see it"
        );

        let owned = passages_from_parts(PassagesParts {
            passages: vec![(
                "sc-01".to_string(),
                PassagePart {
                    anchor,
                    text: Cow::Owned("물때가 셈한다.".to_string()),
                },
            )],
        });
        assert!(matches!(owned["sc-01"].text, Cow::Owned(_)));

        // And the two are equal, which is exactly why the discriminant needed its
        // own assertion.
        assert_eq!(borrowed, owned);
    }

    /// Round 793 — the run-time constructors still OWN, because they build their
    /// text rather than pointing at a literal. Asserted so the `Cow` cannot drift
    /// into a claim that everything borrows.
    #[test]
    fn the_runtime_constructors_still_own_their_text() {
        let excerpt = mnemosyne_atomic::ContentExcerpt {
            anchor: ContentAnchor {
                source: "M.md".to_string(),
                locator: Locator::Prefix("이름을".to_string()),
            },
            text: "물때가 셈한다.".to_string(),
            text_sha256: String::new(),
        };
        assert!(matches!(
            Passage::from_excerpt(&excerpt).text,
            Cow::Owned(_)
        ));
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
    fn a_prefix_naming_two_places_is_rejected_not_silently_first() {
        // Round 766 — the slicer used to take `text.find`, i.e. the FIRST
        // occurrence, so an ambiguous coordinate quietly got a slice. Picking one
        // is picking on the author's behalf; the fix is to extend the prefix.
        const TWICE: &str = "The tide pulls out. Bunok counts. The tide returns.";
        let err = PrefixSlices::new(DOC, TWICE, &[prefix("The tide")]).unwrap_err();
        assert_eq!(
            err,
            ProseError::PrefixAmbiguous {
                source: DOC.to_string(),
                prefix: "The tide".to_string(),
                occurrences: 2,
            }
        );
        // Extending it to name one place is accepted — the ambiguity was real,
        // not an artefact of rejecting repeats of a common word.
        assert!(PrefixSlices::new(DOC, TWICE, &[prefix("The tide returns")]).is_ok());
    }

    #[test]
    fn the_same_anchor_declared_twice_is_rejected_not_overwritten() {
        // Round 766 — the slice map is keyed by anchor, so a duplicate used to
        // overwrite its twin silently: two declared holds, one surviving slice.
        let err =
            PrefixSlices::new(DOC, TEXT, &[prefix("The tide"), prefix("The tide")]).unwrap_err();
        assert_eq!(
            err,
            ProseError::DuplicateAnchor {
                source: DOC.to_string(),
                prefix: "The tide".to_string(),
            }
        );
    }

    #[test]
    fn in_prose_order_reports_the_document_order_whatever_the_input_order() {
        // Round 766 — the ordering an ordered caller (a ladder) judges against.
        // The locating happens once here, so that caller never re-implements the
        // search and cannot drift from it. Input order stays free.
        let source = PrefixSlices::new(
            DOC,
            TEXT,
            &[
                prefix("Night falls"),
                prefix("The tide"),
                prefix("Bunok counts"),
            ],
        )
        .unwrap();
        let order: Vec<&Locator> = source.in_prose_order().iter().map(|a| &a.locator).collect();
        assert_eq!(
            order,
            vec![
                &Locator::Prefix("The tide".to_string()),
                &Locator::Prefix("Bunok counts".to_string()),
                &Locator::Prefix("Night falls".to_string()),
            ]
        );
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
