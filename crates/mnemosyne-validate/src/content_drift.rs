//! Content-integrity drift scan (R404 — EPUB-as-content-SSOT revalidation).
//!
//! `normative_excerpt.text` is a *derived cache* of the committed EPUB
//! (R403); `normative_excerpt.text_sha256` is the offline revalidation
//! anchor the EPUB extractor (`medium-forge` `epub-anchor-map/v2`) emitted
//! and the mutate API verifies at write time. This scan re-hashes the
//! stored `text` and compares it to the declared `text_sha256` — entirely
//! offline, no EPUB and no re-extraction (the Rust core never re-extracts;
//! that is the Python tool's job). A non-empty hash that no longer matches
//! `sha256(text)` means the cache was edited *out of band* — a direct
//! sidecar-JSON edit that bypassed the mutate API. That is corruption, not
//! a legitimate intermediate state, so `[content_drift].severity` defaults
//! to `reject` (unlike `[spec_drift]`'s `warn`).
//!
//! **Out of scope (single-sourced elsewhere):**
//! - The `normative_excerpt` BACKFILL WORKLIST — which spec excerpts need a
//!   hash extracted — is `report-excerpt-hash-backfill` (R402). Unrevalidatable
//!   excerpts are not drift (an empty hash certifies nothing, so nothing can
//!   drift from it) and this scan still only reports genuine hash *mismatches*,
//!   but it now COUNTS the ones it skipped, over all three kinds, in its own
//!   [`ExcerptCensus`] (Round 895). The CLI used to borrow that number from the
//!   R402 report, which sees `normative_excerpt` only — a context number
//!   narrower than the scan it stood beside.
//! - *Spec-revision* drift (anchored rev vs workspace rev) is
//!   [`crate::spec_drift`].
//! - EPUB-*file* identity (committed EPUB vs a pinned `epub_sha256`) is a
//!   separate provenance axis (R405), not this content-integrity scan.
//!
//! Status-agnostic: a corrupted cache is corrupt whether the Section is
//! `Active`, `Superseded`, or `Removed` — integrity does not depend on
//! lifecycle (contrast `spec_drift`, where `Superseded` Sections are
//! *expected* to trail the rev). Iterates `store.sections` in `BTreeMap`
//! key order → stably ordered by `section_id`. Pure + offline + deterministic.

use mnemosyne_atomic::AtomicStore;

/// Which provenance-excerpt cache drifted (R756 generalized the scan to cover
/// both; R757 added scene presence). `Normative` = the spec/EPUB-external mirror
/// (`normative_excerpt`); `Content` = the narrative-prose anchor
/// (`content_excerpt`); `ScenePresence` = a `scene_cast` presence's manuscript
/// quote (Round 757, B0). Same offline sha model; the kind tells a consumer which
/// cache to re-ingest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExcerptKind {
    /// `normative_excerpt` — the spec/EPUB-external-mirror excerpt.
    Normative,
    /// `content_excerpt` — the narrative-prose content anchor (R756).
    Content,
    /// `scene_cast[].excerpt` — a scene presence's manuscript quote (Round 757).
    ScenePresence,
}

impl ExcerptKind {
    fn as_str(self) -> &'static str {
        match self {
            ExcerptKind::Normative => "normative",
            ExcerptKind::Content => "content",
            ExcerptKind::ScenePresence => "scene_presence",
        }
    }
}

/// One content-integrity finding: a Section whose cached excerpt `text` no longer
/// hashes to its declared `text_sha256`. `excerpt` names which cache drifted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentDriftViolation {
    /// Drifted Section id (without the `§` prefix), as stored.
    pub section_id: String,
    /// Which excerpt cache drifted (`normative_excerpt` vs `content_excerpt`).
    pub excerpt: ExcerptKind,
    /// The `text_sha256` recorded on the excerpt (the extracted/ingested anchor).
    pub declared_sha256: String,
    /// `sha256(text)` recomputed from the stored cache — diverges from
    /// `declared_sha256`, which is the drift.
    pub computed_sha256: String,
}

impl ContentDriftViolation {
    /// Flat machine-readable JSON for the `validate-content-drift --json`
    /// surface. `status` is always `"drift"` — the array only ever holds
    /// violations (a green scan emits an empty `violations[]`).
    pub fn to_cli_json(&self) -> serde_json::Value {
        serde_json::json!({
            "section_id": self.section_id,
            "excerpt": self.excerpt.as_str(),
            "declared_sha256": self.declared_sha256,
            "computed_sha256": self.computed_sha256,
            "status": "drift",
        })
    }
}

/// What the scan actually looked at (Round 895), derived from the SAME walk that
/// produces the violations rather than recounted beside it.
///
/// An empty violation list answers two different questions identically — "every
/// cache still hashes to its pin" and "there was no cache to hash" — and the
/// second is the state a fact-first store with no authored prose is in. R894
/// measured a 56-section store where NOT ONE section carried an excerpt, and the
/// CLI reported `sections=56 ... drift: total=0`, every number true and the whole
/// line reading as coverage. These counts are what tell the two apart.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ExcerptCensus {
    /// Excerpt caches whose populated `text_sha256` was compared against
    /// `sha256(text)` — the scan's actual reach. Drifted caches are counted here
    /// too: they were examined, and that is how the drift was found.
    pub examined: usize,
    /// Excerpt caches skipped for an empty `text_sha256` (unrevalidatable — an
    /// empty hash certifies nothing, so nothing can drift from it). Counted over
    /// ALL THREE kinds, unlike `report-excerpt-hash-backfill` (R402), which is a
    /// `normative_excerpt` backfill worklist and answers a narrower question.
    pub unrevalidatable: usize,
    /// Sections carrying at least one excerpt cache of any kind. Compared against
    /// the store's section count, this is the difference between "nothing wrong"
    /// and "nothing here".
    pub sections_with_excerpt: usize,
}

/// The scan's whole answer: what drifted, and what was looked at to find out.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContentDriftScan {
    /// One entry per drifted cache, `BTreeMap`-key ordered by section.
    pub violations: Vec<ContentDriftViolation>,
    /// The reach behind that list — see [`ExcerptCensus`].
    pub census: ExcerptCensus,
}

/// Scan `store` for content-integrity drift: every Section carrying a
/// `normative_excerpt`, a `content_excerpt` (R756), OR a `scene_cast` presence
/// quote (R757) with a **non-empty** `text_sha256` that no longer equals
/// `sha256(text)` contributes a [`ContentDriftViolation`] (one per drifted cache;
/// a Section could carry several).
///
/// Excerpts with an empty `text_sha256` are skipped (unrevalidatable — owned
/// by `report-excerpt-hash-backfill`, not drift). Pure + offline + deterministic,
/// `BTreeMap`-key ordered; within a Section: normative, then content, then each
/// `scene_cast` presence in stored order.
///
/// Returns the [`ExcerptCensus`] alongside, counted by this same walk — a second
/// pass to answer "how much did you check" would be free to disagree with the
/// pass that answered "what is wrong".
pub fn scan_content_drift(store: &AtomicStore) -> ContentDriftScan {
    let mut violations: Vec<ContentDriftViolation> = Vec::new();
    let mut census = ExcerptCensus::default();
    for (section_id, section) in &store.sections {
        // (kind, matches, declared, computed) for each excerpt the Section carries.
        let normative = section.normative_excerpt.as_ref().map(|e| {
            (
                ExcerptKind::Normative,
                e.excerpt.text_sha256_matches(),
                e.excerpt.text_sha256.clone(),
                e.excerpt.recompute_text_sha256(),
            )
        });
        let content = section.content_excerpt.as_ref().map(|e| {
            (
                ExcerptKind::Content,
                e.text_sha256_matches(),
                e.text_sha256.clone(),
                e.recompute_text_sha256(),
            )
        });
        // Each scene presence carries the same ContentExcerpt drift surface.
        let presences = section.scene_cast.iter().map(|p| {
            (
                ExcerptKind::ScenePresence,
                p.excerpt.text_sha256_matches(),
                p.excerpt.text_sha256.clone(),
                p.excerpt.recompute_text_sha256(),
            )
        });
        let mut carries_a_cache = false;
        for (kind, matches, declared, computed) in
            normative.into_iter().chain(content).chain(presences)
        {
            carries_a_cache = true;
            // None = empty hash (unrevalidatable, not drift); Some(true) = clean.
            // Only Some(false) — a populated hash that no longer matches — drifts.
            match matches {
                None => census.unrevalidatable += 1,
                Some(true) => census.examined += 1,
                Some(false) => {
                    census.examined += 1;
                    violations.push(ContentDriftViolation {
                        section_id: section_id.to_string(),
                        excerpt: kind,
                        declared_sha256: declared,
                        computed_sha256: computed,
                    });
                }
            }
        }
        if carries_a_cache {
            census.sections_with_excerpt += 1;
        }
    }
    ContentDriftScan { violations, census }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mnemosyne_atomic::{AtomicSection, ContentExcerpt, NormativeExcerpt};
    use mnemosyne_core::{ContentAnchor, DecisionStatus, Locator, SectionSkeleton};
    /// The violation list alone — what these tests assert on. The scan also
    /// returns its census (Round 895); the census has tests of its own below.
    fn drift(store: &AtomicStore) -> Vec<ContentDriftViolation> {
        scan_content_drift(store).violations
    }

    fn sha256_hex(s: &str) -> String {
        mnemosyne_core::sha256_hex(s.as_bytes())
    }

    /// A Section carrying only a `content_excerpt` (R756 narrative-prose anchor).
    fn content_section(text: &str, hash: &str) -> AtomicSection {
        AtomicSection {
            content_excerpt: Some(ContentExcerpt {
                anchor: ContentAnchor {
                    source: "MANUSCRIPT.md".to_string(),
                    locator: Locator::Prefix(text.chars().take(8).collect()),
                },
                text: text.to_string(),
                text_sha256: hash.to_string(),
            }),
            ..Default::default()
        }
    }

    fn section(text: &str, hash: &str, status: Option<DecisionStatus>) -> AtomicSection {
        AtomicSection {
            skeleton: SectionSkeleton {
                title: "t".to_string(),
                parent_doc: "docs/spec.epub".to_string(),
                parent_section: None,
                decision_status: status,
            },
            normative_excerpt: Some(NormativeExcerpt {
                excerpt: ContentExcerpt {
                    anchor: ContentAnchor {
                        source: "https://www.w3.org/TR/scxml/#x".to_string(),
                        locator: Locator::Prefix(text.chars().take(8).collect()),
                    },
                    text: text.to_string(),
                    text_sha256: hash.to_string(),
                },
                anchor_url: "https://www.w3.org/TR/scxml/#x".to_string(),
                source_revision: "rev".to_string(),
            }),
            ..Default::default()
        }
    }

    fn store_with(sections: &[(&str, AtomicSection)]) -> AtomicStore {
        let mut store = AtomicStore::default();
        for (id, sec) in sections {
            store.sections.insert((*id).into(), sec.clone());
        }
        store
    }

    #[test]
    fn matching_hash_is_clean() {
        let store = store_with(&[(
            "scxml-3.13",
            section("spec text", &sha256_hex("spec text"), None),
        )]);
        assert!(drift(&store).is_empty());
    }

    #[test]
    fn mismatched_hash_is_drift() {
        let store = store_with(&[("scxml-3.13", section("spec text", "deadbeef", None))]);
        let v = drift(&store);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].section_id, "scxml-3.13");
        assert_eq!(v[0].declared_sha256, "deadbeef");
        assert_eq!(v[0].computed_sha256, sha256_hex("spec text"));
    }

    #[test]
    fn empty_hash_is_not_drift() {
        // Unrevalidatable (owned by report-excerpt-hash-backfill), not drift.
        let store = store_with(&[("scxml-3.13", section("spec text", "", None))]);
        assert!(drift(&store).is_empty());
    }

    #[test]
    fn drift_is_status_agnostic() {
        // A corrupt cache drifts whether Active, Superseded, or Removed.
        let store = store_with(&[
            ("a", section("ta", "bad", Some(DecisionStatus::Active))),
            ("b", section("tb", "bad", Some(DecisionStatus::Superseded))),
            ("c", section("tc", "bad", Some(DecisionStatus::Removed))),
        ]);
        let ids: Vec<_> = drift(&store).into_iter().map(|v| v.section_id).collect();
        assert_eq!(ids, vec!["a", "b", "c"]); // BTreeMap-ordered
    }

    #[test]
    fn section_without_excerpt_is_skipped() {
        let store = store_with(&[("design-only", AtomicSection::default())]);
        assert!(drift(&store).is_empty());
    }

    // ── R756: content_excerpt drift, the same offline sha check generalized ──

    #[test]
    fn content_excerpt_matching_hash_is_clean() {
        let store = store_with(&[("d01-nat", content_section("prose", &sha256_hex("prose")))]);
        assert!(drift(&store).is_empty());
    }

    #[test]
    fn content_excerpt_mismatched_hash_is_drift() {
        // Injection: a content_excerpt whose stored text no longer hashes to its
        // declared sha (an out-of-band edit) — non-vacuity of the generalized scan.
        let store = store_with(&[("d01-nat", content_section("edited prose", "deadbeef"))]);
        let v = drift(&store);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].section_id, "d01-nat");
        assert_eq!(v[0].excerpt, ExcerptKind::Content);
        assert_eq!(v[0].declared_sha256, "deadbeef");
        assert_eq!(v[0].computed_sha256, sha256_hex("edited prose"));
    }

    #[test]
    fn content_excerpt_empty_hash_is_not_drift() {
        // Unrevalidatable (never ingested with a hash), not drift — same as normative.
        let store = store_with(&[("d01-nat", content_section("prose", ""))]);
        assert!(drift(&store).is_empty());
    }

    #[test]
    fn both_excerpts_on_one_section_each_drift_normative_first() {
        // A Section could carry both; each drifted cache is its own violation,
        // normative before content (stable order).
        let mut sec = section("spec", "badspec", None);
        sec.content_excerpt = Some(ContentExcerpt {
            anchor: ContentAnchor {
                source: "MANUSCRIPT.md".to_string(),
                locator: Locator::Prefix("prose".to_string()),
            },
            text: "prose".to_string(),
            text_sha256: "badprose".to_string(),
        });
        let v = drift(&store_with(&[("s", sec)]));
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].excerpt, ExcerptKind::Normative);
        assert_eq!(v[1].excerpt, ExcerptKind::Content);
    }

    // ── R757 (B0): scene_cast presence-quote drift, the same offline sha check ──

    /// A Section carrying a `scene_cast` presence whose quote hashes to `hash`.
    fn scene_section(entity: &str, quote: &str, hash: &str) -> AtomicSection {
        AtomicSection {
            scene_cast: vec![mnemosyne_atomic::ScenePresence {
                entity: entity.into(),
                modality: mnemosyne_core::Modality::Observed,
                can_answer: true,
                excerpt: ContentExcerpt {
                    anchor: ContentAnchor {
                        source: "MANUSCRIPT.md".to_string(),
                        locator: Locator::Prefix(quote.chars().take(8).collect()),
                    },
                    text: quote.to_string(),
                    text_sha256: hash.to_string(),
                },
            }],
            ..Default::default()
        }
    }

    #[test]
    fn scene_presence_matching_hash_is_clean() {
        let store = store_with(&[(
            "d08-bam",
            scene_section(
                "ent-jongdeuk",
                "종득은 문간에",
                &sha256_hex("종득은 문간에"),
            ),
        )]);
        assert!(drift(&store).is_empty());
    }

    #[test]
    fn scene_presence_mismatched_hash_is_drift() {
        // Injection: a scene_cast presence quote whose stored text no longer
        // hashes to its declared sha — non-vacuity of the R757 generalized scan.
        let store = store_with(&[(
            "d08-bam",
            scene_section("ent-jongdeuk", "고쳐 쓴 인용", "deadbeef"),
        )]);
        let v = drift(&store);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].section_id, "d08-bam");
        assert_eq!(v[0].excerpt, ExcerptKind::ScenePresence);
        assert_eq!(v[0].declared_sha256, "deadbeef");
        assert_eq!(v[0].computed_sha256, sha256_hex("고쳐 쓴 인용"));
    }

    #[test]
    fn scene_presence_empty_hash_is_not_drift() {
        // Unrevalidatable (never ingested with a hash), not drift — same as the others.
        let store = store_with(&[(
            "d08-bam",
            scene_section("ent-jongdeuk", "종득은 문간에", ""),
        )]);
        assert!(drift(&store).is_empty());
    }

    #[test]
    fn multiple_scene_presences_drift_in_stored_order_after_content() {
        // A Section can carry content + several presences; each drifted cache is
        // its own violation, content before scene presences, presences in order.
        let mut sec = content_section("edited", "badcontent");
        sec.scene_cast = vec![
            mnemosyne_atomic::ScenePresence {
                entity: "ent-a".into(),
                modality: mnemosyne_core::Modality::Observed,
                can_answer: true,
                excerpt: ContentExcerpt {
                    anchor: ContentAnchor {
                        source: "MANUSCRIPT.md".to_string(),
                        locator: Locator::Prefix("a".to_string()),
                    },
                    text: "a-quote".to_string(),
                    text_sha256: "bada".to_string(),
                },
            },
            mnemosyne_atomic::ScenePresence {
                entity: "ent-b".into(),
                modality: mnemosyne_core::Modality::Told,
                can_answer: false,
                excerpt: ContentExcerpt {
                    anchor: ContentAnchor {
                        source: "MANUSCRIPT.md".to_string(),
                        locator: Locator::Prefix("b".to_string()),
                    },
                    text: "b-quote".to_string(),
                    text_sha256: "badb".to_string(),
                },
            },
        ];
        let v = drift(&store_with(&[("s", sec)]));
        assert_eq!(v.len(), 3);
        assert_eq!(v[0].excerpt, ExcerptKind::Content);
        assert_eq!(v[1].excerpt, ExcerptKind::ScenePresence);
        assert_eq!(v[1].computed_sha256, sha256_hex("a-quote"));
        assert_eq!(v[2].excerpt, ExcerptKind::ScenePresence);
        assert_eq!(v[2].computed_sha256, sha256_hex("b-quote"));
    }

    #[test]
    fn a_clean_scan_and_a_scan_of_nothing_differ_only_in_the_census() {
        // THE discriminating pair (Round 895). Both stores yield an empty
        // violation list, and before the census that was the entire answer —
        // R894 measured a real 56-section store reading as clean while carrying
        // no prose at all. The assertion is not "the census is populated"; it is
        // that these two states, identical in the finding list, are told apart.
        let carries = store_with(&[
            ("a", content_section("clean a", &sha256_hex("clean a"))),
            ("b", content_section("clean b", &sha256_hex("clean b"))),
        ]);
        let bare = store_with(&[
            ("a", AtomicSection::default()),
            ("b", AtomicSection::default()),
        ]);

        let clean = scan_content_drift(&carries);
        let empty = scan_content_drift(&bare);
        assert!(clean.violations.is_empty());
        assert!(empty.violations.is_empty());
        assert_ne!(
            clean.census, empty.census,
            "two stores a reader must not confuse produced the same census"
        );

        assert_eq!(clean.census.examined, 2);
        assert_eq!(clean.census.sections_with_excerpt, 2);
        // Nothing was compared, and both counts say so — a section count would
        // have said 2 here as well, which is the number that misled.
        assert_eq!(empty.census.examined, 0);
        assert_eq!(empty.census.sections_with_excerpt, 0);
        assert_eq!(bare.sections.len(), 2);
    }

    #[test]
    fn the_census_counts_every_excerpt_kind_and_separates_the_hashless() {
        // `unrevalidatable` is the scan's own count over all three kinds, not the
        // R402 backfill worklist's `normative_excerpt`-only one. A section with a
        // hashed normative excerpt, a hashless content excerpt and a hashless
        // scene quote splits 1 examined / 2 unrevalidatable — a normative-only
        // count would say 0 and read as full coverage.
        let mut sec = section("normative", &sha256_hex("normative"), None);
        sec.content_excerpt = Some(ContentExcerpt {
            anchor: ContentAnchor {
                source: "MANUSCRIPT.md".to_string(),
                locator: Locator::Prefix("c".to_string()),
            },
            text: "hand authored".to_string(),
            text_sha256: String::new(),
        });
        sec.scene_cast = vec![mnemosyne_atomic::ScenePresence {
            entity: "ent-a".into(),
            modality: mnemosyne_core::Modality::Observed,
            can_answer: true,
            excerpt: ContentExcerpt {
                anchor: ContentAnchor {
                    source: "MANUSCRIPT.md".to_string(),
                    locator: Locator::Prefix("q".to_string()),
                },
                text: "a-quote".to_string(),
                text_sha256: String::new(),
            },
        }];
        let scan = scan_content_drift(&store_with(&[("s", sec)]));
        assert!(scan.violations.is_empty());
        assert_eq!(scan.census.examined, 1);
        assert_eq!(scan.census.unrevalidatable, 2);
        assert_eq!(scan.census.sections_with_excerpt, 1);
    }

    #[test]
    fn a_drifted_cache_still_counts_as_examined() {
        // Otherwise "examined" would drop exactly when the scan did the most work,
        // and a store whose every cache drifted would report 0 examined — the
        // reading this census exists to prevent, inverted.
        let scan = scan_content_drift(&store_with(&[("s", content_section("edited", "deadbeef"))]));
        assert_eq!(scan.violations.len(), 1);
        assert_eq!(scan.census.examined, 1);
        assert_eq!(scan.census.unrevalidatable, 0);
    }
}
