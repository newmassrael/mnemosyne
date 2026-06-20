//! Spec-revision drift scan (RFC-001 UC-1 "B2").
//!
//! An external-spec mirror workspace pins, per Section, the upstream
//! revision its `normative_excerpt` was anchored at
//! (`normative_excerpt.source_revision`), while the workspace as a whole
//! tracks a *current* revision (`[workspace.spec_source].revision`). When
//! the upstream standard is revised the workspace-level rev is bumped; any
//! still-`Active` Section whose anchored rev now trails the workspace rev
//! is **drift** — code still cites a Section pinned to a stale spec
//! revision.
//!
//! This is a pure, offline, deterministic *label* diff: it compares the
//! two free-form revision strings for equality and never fetches the
//! upstream. Byte-level drift (upstream rev label unchanged but the
//! fetched content diverges) is the consumer/CI's job via
//! `[workspace.spec_source].fetched_sha256`, not this scan's.
//!
//! Partial migration is a legitimate intermediate state: a rev bump is
//! modeled as the old Section transitioning to `Superseded` (it is then
//! *expected* to hold the old rev) plus a new `Active` Section carrying
//! the bumped excerpt — the same supersession pattern used everywhere
//! else. So `Superseded`/`Removed` Sections are exempt; only live
//! (`Active`, or the unset default) Sections can drift.

use mnemosyne_atomic::AtomicStore;
use mnemosyne_core::DecisionStatus;

/// One spec-revision drift finding: a live Section whose anchored spec
/// revision differs from the workspace's current spec revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecDriftViolation {
    /// Drifted Section id (without the `§` prefix), as stored.
    pub section_id: String,
    /// The Section's `normative_excerpt.source_revision` (the rev it was
    /// anchored at).
    pub section_revision: String,
    /// The workspace's current `[workspace.spec_source].revision`.
    pub workspace_revision: String,
}

impl SpecDriftViolation {
    /// Flat machine-readable JSON for the `validate-spec-drift --json`
    /// surface. `status` is always `"drift"` — the array only ever holds
    /// violations (a green scan emits an empty `violations[]`).
    pub fn to_cli_json(&self) -> serde_json::Value {
        serde_json::json!({
            "section_id": self.section_id,
            "section_revision": self.section_revision,
            "workspace_revision": self.workspace_revision,
            "status": "drift",
        })
    }
}

/// Scan `store` for spec-revision drift against `workspace_revision`
/// (the workspace-level `[workspace.spec_source].revision`).
///
/// A Section contributes a [`SpecDriftViolation`] iff all hold:
/// - it carries a `normative_excerpt` (it mirrors an external spec —
///   Sections without one are ordinary design entries, never drift);
/// - its `decision_status` is `Active` or unset (the live default);
///   `Superseded`/`Removed` Sections are *expected* to hold an older rev
///   (the partial-migration pattern) and are exempt;
/// - its anchored `source_revision` differs from `workspace_revision`.
///
/// Iterates `store.sections` in `BTreeMap` key order, so the result is
/// stably ordered by `section_id`. Pure + offline + deterministic.
pub fn scan_spec_drift(store: &AtomicStore, workspace_revision: &str) -> Vec<SpecDriftViolation> {
    store
        .sections
        .iter()
        .filter_map(|(section_id, section)| {
            let excerpt = section.normative_excerpt.as_ref()?;
            // Superseded/Removed Sections are expected to trail the workspace
            // rev (partial-migration); Open (not-yet-decided) is not a ratified
            // live spec mirror either. Only live Active Sections drift. Unset
            // decision_status == Active (the live default).
            if matches!(
                section.skeleton.decision_status,
                Some(DecisionStatus::Superseded)
                    | Some(DecisionStatus::Removed)
                    | Some(DecisionStatus::Open)
            ) {
                return None;
            }
            if excerpt.source_revision == workspace_revision {
                return None;
            }
            Some(SpecDriftViolation {
                section_id: section_id.clone(),
                section_revision: excerpt.source_revision.clone(),
                workspace_revision: workspace_revision.to_string(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mnemosyne_atomic::{AtomicSection, NormativeExcerpt};
    use mnemosyne_core::SectionSkeleton;

    fn section_with_rev(rev: Option<&str>, status: Option<DecisionStatus>) -> AtomicSection {
        AtomicSection {
            skeleton: SectionSkeleton {
                title: "t".to_string(),
                parent_doc: "GENERATED.md".to_string(),
                parent_section: None,
                decision_status: status,
            },
            normative_excerpt: rev.map(|r| NormativeExcerpt {
                text: "the normative text".to_string(),
                anchor_url: "https://www.w3.org/TR/scxml/#x".to_string(),
                source_revision: r.to_string(),
                text_sha256: String::new(),
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
    fn matching_rev_is_not_drift() {
        let store = store_with(&[("scxml-3.13", section_with_rev(Some("2024-rec"), None))]);
        assert!(scan_spec_drift(&store, "2024-rec").is_empty());
    }

    #[test]
    fn active_stale_rev_is_drift() {
        let store = store_with(&[(
            "scxml-3.13",
            section_with_rev(Some("2020-rec"), Some(DecisionStatus::Active)),
        )]);
        let v = scan_spec_drift(&store, "2024-rec");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].section_id, "scxml-3.13");
        assert_eq!(v[0].section_revision, "2020-rec");
        assert_eq!(v[0].workspace_revision, "2024-rec");
    }

    #[test]
    fn unset_status_stale_rev_is_drift() {
        // Unset decision_status == Active (the live default) → drifts.
        let store = store_with(&[("scxml-3.13", section_with_rev(Some("2020-rec"), None))]);
        assert_eq!(scan_spec_drift(&store, "2024-rec").len(), 1);
    }

    #[test]
    fn superseded_stale_rev_is_exempt() {
        let store = store_with(&[(
            "scxml-3.13",
            section_with_rev(Some("2020-rec"), Some(DecisionStatus::Superseded)),
        )]);
        assert!(scan_spec_drift(&store, "2024-rec").is_empty());
    }

    #[test]
    fn removed_stale_rev_is_exempt() {
        let store = store_with(&[(
            "scxml-3.13",
            section_with_rev(Some("2020-rec"), Some(DecisionStatus::Removed)),
        )]);
        assert!(scan_spec_drift(&store, "2024-rec").is_empty());
    }

    #[test]
    fn section_without_excerpt_never_drifts() {
        // Ordinary design Section (no spec mirror) → never drift.
        let store = store_with(&[("ordinary-decision", section_with_rev(None, None))]);
        assert!(scan_spec_drift(&store, "2024-rec").is_empty());
    }

    #[test]
    fn result_is_ordered_by_section_id() {
        let store = store_with(&[
            ("scxml-5.10", section_with_rev(Some("old"), None)),
            ("scxml-3.13", section_with_rev(Some("old"), None)),
            ("scxml-4.1", section_with_rev(Some("old"), None)),
        ]);
        let v = scan_spec_drift(&store, "new");
        let ids: Vec<&str> = v.iter().map(|d| d.section_id.as_str()).collect();
        assert_eq!(ids, ["scxml-3.13", "scxml-4.1", "scxml-5.10"]);
    }

    #[test]
    fn to_cli_json_shape() {
        let v = SpecDriftViolation {
            section_id: "scxml-3.13".to_string(),
            section_revision: "2020-rec".to_string(),
            workspace_revision: "2024-rec".to_string(),
        };
        assert_eq!(
            v.to_cli_json(),
            serde_json::json!({
                "section_id": "scxml-3.13",
                "section_revision": "2020-rec",
                "workspace_revision": "2024-rec",
                "status": "drift",
            })
        );
    }
}
