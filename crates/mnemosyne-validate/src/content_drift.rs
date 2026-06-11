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

/// One content-integrity finding: a Section whose cached `normative_excerpt.text`
/// no longer hashes to its declared `text_sha256`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentDriftViolation {
    /// Drifted Section id (without the `§` prefix), as stored.
    pub section_id: String,
    /// The `text_sha256` recorded on the excerpt (the EPUB-extracted anchor).
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
            "declared_sha256": self.declared_sha256,
            "computed_sha256": self.computed_sha256,
            "status": "drift",
        })
    }
}

/// Scan `store` for content-integrity drift: every Section carrying a
/// `normative_excerpt` with a **non-empty** `text_sha256` that no longer
/// equals `sha256(text)` contributes a [`ContentDriftViolation`].
///
/// Excerpts with an empty `text_sha256` are skipped (unrevalidatable — owned
/// by `report-excerpt-hash-backfill`, not drift). Pure + offline + deterministic,
/// `BTreeMap`-key ordered.
pub fn scan_content_drift(store: &AtomicStore) -> Vec<ContentDriftViolation> {
    store
        .sections
        .iter()
        .filter_map(|(section_id, section)| {
            let excerpt = section.normative_excerpt.as_ref()?;
            // None = empty hash (unrevalidatable, not drift); Some(true) = clean.
            // Only Some(false) — a populated hash that no longer matches — drifts.
            match excerpt.text_sha256_matches() {
                Some(false) => Some(ContentDriftViolation {
                    section_id: section_id.clone(),
                    declared_sha256: excerpt.text_sha256.clone(),
                    computed_sha256: excerpt.recompute_text_sha256(),
                }),
                _ => None,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mnemosyne_atomic::{AtomicSection, NormativeExcerpt};
    use mnemosyne_core::{DecisionStatus, SectionSkeleton};
    fn sha256_hex(s: &str) -> String {
        mnemosyne_core::sha256_hex(s.as_bytes())
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
}
