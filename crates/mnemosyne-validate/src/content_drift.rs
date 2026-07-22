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
//! - *Unrevalidatable* excerpts (empty `text_sha256` — hand-authored or
//!   pre-v8, never imported from an EPUB) are owned by
//!   `report-excerpt-hash-backfill` (R402); they are NOT drift (an empty
//!   hash certifies nothing, so nothing can drift from it). The
//!   `validate-content-drift` CLI surfaces that count separately for
//!   context, but this scan only reports genuine hash *mismatches*.
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
/// both). `Normative` = the spec/EPUB-external mirror (`normative_excerpt`);
/// `Content` = the narrative-prose anchor (`content_excerpt`). Same offline sha
/// model; the kind tells a consumer which cache to re-ingest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExcerptKind {
    /// `normative_excerpt` — the spec/EPUB-external-mirror excerpt.
    Normative,
    /// `content_excerpt` — the narrative-prose content anchor (R756).
    Content,
}

impl ExcerptKind {
    fn as_str(self) -> &'static str {
        match self {
            ExcerptKind::Normative => "normative",
            ExcerptKind::Content => "content",
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

/// Scan `store` for content-integrity drift: every Section carrying a
/// `normative_excerpt` OR a `content_excerpt` (R756) with a **non-empty**
/// `text_sha256` that no longer equals `sha256(text)` contributes a
/// [`ContentDriftViolation`] (one per drifted cache; a Section could carry both).
///
/// Excerpts with an empty `text_sha256` are skipped (unrevalidatable — owned
/// by `report-excerpt-hash-backfill`, not drift). Pure + offline + deterministic,
/// `BTreeMap`-key ordered, normative before content within a Section.
pub fn scan_content_drift(store: &AtomicStore) -> Vec<ContentDriftViolation> {
    store
        .sections
        .iter()
        .flat_map(|(section_id, section)| {
            // (kind, matches, declared, computed) for each excerpt the Section carries.
            let normative = section.normative_excerpt.as_ref().map(|e| {
                (
                    ExcerptKind::Normative,
                    e.text_sha256_matches(),
                    e.text_sha256.clone(),
                    e.recompute_text_sha256(),
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
            // None = empty hash (unrevalidatable, not drift); Some(true) = clean.
            // Only Some(false) — a populated hash that no longer matches — drifts.
            [normative, content]
                .into_iter()
                .flatten()
                .filter_map(|(kind, matches, declared, computed)| {
                    (matches == Some(false)).then(|| ContentDriftViolation {
                        section_id: section_id.clone(),
                        excerpt: kind,
                        declared_sha256: declared,
                        computed_sha256: computed,
                    })
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mnemosyne_atomic::{AtomicSection, ContentExcerpt, NormativeExcerpt};
    use mnemosyne_core::{ContentAnchor, DecisionStatus, Locator, SectionSkeleton};
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
                text: text.to_string(),
                anchor_url: "https://www.w3.org/TR/scxml/#x".to_string(),
                source_revision: "rev".to_string(),
                text_sha256: hash.to_string(),
            }),
            ..Default::default()
        }
    }

    fn store_with(sections: &[(&str, AtomicSection)]) -> AtomicStore {
        let mut store = AtomicStore::default();
        for (id, sec) in sections {
            store.sections.insert((*id).to_string(), sec.clone());
        }
        store
    }

    #[test]
    fn matching_hash_is_clean() {
        let store = store_with(&[(
            "scxml-3.13",
            section("spec text", &sha256_hex("spec text"), None),
        )]);
        assert!(scan_content_drift(&store).is_empty());
    }

    #[test]
    fn mismatched_hash_is_drift() {
        let store = store_with(&[("scxml-3.13", section("spec text", "deadbeef", None))]);
        let v = scan_content_drift(&store);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].section_id, "scxml-3.13");
        assert_eq!(v[0].declared_sha256, "deadbeef");
        assert_eq!(v[0].computed_sha256, sha256_hex("spec text"));
    }

    #[test]
    fn empty_hash_is_not_drift() {
        // Unrevalidatable (owned by report-excerpt-hash-backfill), not drift.
        let store = store_with(&[("scxml-3.13", section("spec text", "", None))]);
        assert!(scan_content_drift(&store).is_empty());
    }

    #[test]
    fn drift_is_status_agnostic() {
        // A corrupt cache drifts whether Active, Superseded, or Removed.
        let store = store_with(&[
            ("a", section("ta", "bad", Some(DecisionStatus::Active))),
            ("b", section("tb", "bad", Some(DecisionStatus::Superseded))),
            ("c", section("tc", "bad", Some(DecisionStatus::Removed))),
        ]);
        let ids: Vec<_> = scan_content_drift(&store)
            .into_iter()
            .map(|v| v.section_id)
            .collect();
        assert_eq!(ids, vec!["a", "b", "c"]); // BTreeMap-ordered
    }

    #[test]
    fn section_without_excerpt_is_skipped() {
        let store = store_with(&[("design-only", AtomicSection::default())]);
        assert!(scan_content_drift(&store).is_empty());
    }

    // ── R756: content_excerpt drift, the same offline sha check generalized ──

    #[test]
    fn content_excerpt_matching_hash_is_clean() {
        let store = store_with(&[("d01-nat", content_section("prose", &sha256_hex("prose")))]);
        assert!(scan_content_drift(&store).is_empty());
    }

    #[test]
    fn content_excerpt_mismatched_hash_is_drift() {
        // Injection: a content_excerpt whose stored text no longer hashes to its
        // declared sha (an out-of-band edit) — non-vacuity of the generalized scan.
        let store = store_with(&[("d01-nat", content_section("edited prose", "deadbeef"))]);
        let v = scan_content_drift(&store);
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
        assert!(scan_content_drift(&store).is_empty());
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
        let v = scan_content_drift(&store_with(&[("s", sec)]));
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].excerpt, ExcerptKind::Normative);
        assert_eq!(v[1].excerpt, ExcerptKind::Content);
    }
}
