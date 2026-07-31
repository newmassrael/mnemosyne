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

/// What the scan actually looked at, returned beside what it found.
///
/// Round 901 — the Round 895 repair one verb over. An empty violation list
/// means "every mirrored Section is on the current revision" ONLY when there
/// were mirrored Sections; on a store carrying no `normative_excerpt` it means
/// the scan compared nothing, and both rendered as `drift: total=0` beside the
/// store's whole section count. The reach has to travel with the finding.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpecMirrorCensus {
    /// Sections whose anchored `source_revision` was compared against the
    /// workspace revision — the scan's real reach. Drifted Sections count here
    /// too: they were examined, which is how the drift was found.
    pub examined: usize,
    /// Sections carrying a `normative_excerpt` but exempt by decision status
    /// (`Superseded` / `Removed` / `Open` are expected to trail the workspace
    /// rev, so they are skipped rather than examined).
    pub exempt_by_status: usize,
    /// Sections carrying a `normative_excerpt` at all — `examined` plus
    /// `exempt_by_status`. Against the store's section count, this is the
    /// difference between "nothing drifted" and "nothing here mirrors a spec".
    pub sections_with_excerpt: usize,
}

/// The scan's whole answer: what drifted, and what was looked at to find out.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpecDriftScan {
    /// One entry per drifted Section, `BTreeMap`-key ordered by `section_id`.
    pub violations: Vec<SpecDriftViolation>,
    /// The reach behind that list — see [`SpecMirrorCensus`].
    pub census: SpecMirrorCensus,
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
/// The census is counted in this same walk, so the reported reach cannot
/// diverge from the reported findings — a second pass over the store would be
/// free to answer a different question than the one that found the drift.
///
/// Iterates `store.sections` in `BTreeMap` key order, so the result is
/// stably ordered by `section_id`. Pure + offline + deterministic.
pub fn scan_spec_drift(store: &AtomicStore, workspace_revision: &str) -> SpecDriftScan {
    let mut scan = SpecDriftScan::default();
    for (section_id, section) in &store.sections {
        let Some(excerpt) = section.normative_excerpt.as_ref() else {
            continue;
        };
        scan.census.sections_with_excerpt += 1;
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
            scan.census.exempt_by_status += 1;
            continue;
        }
        scan.census.examined += 1;
        if excerpt.source_revision == workspace_revision {
            continue;
        }
        scan.violations.push(SpecDriftViolation {
            section_id: section_id.to_string(),
            section_revision: excerpt.source_revision.clone(),
            workspace_revision: workspace_revision.to_string(),
        });
    }
    scan
}

#[cfg(test)]
mod tests {
    use super::*;
    use mnemosyne_atomic::{AtomicSection, ContentExcerpt, NormativeExcerpt};
    use mnemosyne_core::{ContentAnchor, Locator, SectionSkeleton};

    fn section_with_rev(rev: Option<&str>, status: Option<DecisionStatus>) -> AtomicSection {
        AtomicSection {
            skeleton: SectionSkeleton {
                title: "t".to_string(),
                parent_doc: "GENERATED.md".to_string(),
                parent_section: None,
                decision_status: status,
            },
            normative_excerpt: rev.map(|r| NormativeExcerpt {
                excerpt: ContentExcerpt {
                    anchor: ContentAnchor {
                        source: "https://www.w3.org/TR/scxml/#x".to_string(),
                        locator: Locator::Prefix("the normative text".to_string()),
                    },
                    text: "the normative text".to_string(),
                    text_sha256: String::new(),
                },
                anchor_url: "https://www.w3.org/TR/scxml/#x".to_string(),
                source_revision: r.to_string(),
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
    fn matching_rev_is_not_drift() {
        let store = store_with(&[("scxml-3.13", section_with_rev(Some("2024-rec"), None))]);
        assert!(scan_spec_drift(&store, "2024-rec").violations.is_empty());
    }

    #[test]
    fn active_stale_rev_is_drift() {
        let store = store_with(&[(
            "scxml-3.13",
            section_with_rev(Some("2020-rec"), Some(DecisionStatus::Active)),
        )]);
        let v = scan_spec_drift(&store, "2024-rec").violations;
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].section_id, "scxml-3.13");
        assert_eq!(v[0].section_revision, "2020-rec");
        assert_eq!(v[0].workspace_revision, "2024-rec");
    }

    #[test]
    fn unset_status_stale_rev_is_drift() {
        // Unset decision_status == Active (the live default) → drifts.
        let store = store_with(&[("scxml-3.13", section_with_rev(Some("2020-rec"), None))]);
        assert_eq!(scan_spec_drift(&store, "2024-rec").violations.len(), 1);
    }

    #[test]
    fn superseded_stale_rev_is_exempt() {
        let store = store_with(&[(
            "scxml-3.13",
            section_with_rev(Some("2020-rec"), Some(DecisionStatus::Superseded)),
        )]);
        assert!(scan_spec_drift(&store, "2024-rec").violations.is_empty());
    }

    #[test]
    fn removed_stale_rev_is_exempt() {
        let store = store_with(&[(
            "scxml-3.13",
            section_with_rev(Some("2020-rec"), Some(DecisionStatus::Removed)),
        )]);
        assert!(scan_spec_drift(&store, "2024-rec").violations.is_empty());
    }

    #[test]
    fn section_without_excerpt_never_drifts() {
        // Ordinary design Section (no spec mirror) → never drift.
        let store = store_with(&[("ordinary-decision", section_with_rev(None, None))]);
        assert!(scan_spec_drift(&store, "2024-rec").violations.is_empty());
    }

    #[test]
    fn result_is_ordered_by_section_id() {
        let store = store_with(&[
            ("scxml-5.10", section_with_rev(Some("old"), None)),
            ("scxml-3.13", section_with_rev(Some("old"), None)),
            ("scxml-4.1", section_with_rev(Some("old"), None)),
        ]);
        let v = scan_spec_drift(&store, "new").violations;
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

    /// Round 901 — the discriminating PAIR, not "the census is populated".
    /// Both stores report zero drift; the difference between them is whether
    /// anything was compared at all, and that difference has to be visible.
    #[test]
    fn two_stores_with_no_drift_are_told_apart_by_the_census() {
        let mirroring = store_with(&[
            ("scxml-3.13", section_with_rev(Some("2024-rec"), None)),
            ("scxml-4.1", section_with_rev(Some("2024-rec"), None)),
        ]);
        let no_mirror = store_with(&[
            ("design-a", section_with_rev(None, None)),
            ("design-b", section_with_rev(None, None)),
        ]);

        let checked = scan_spec_drift(&mirroring, "2024-rec");
        let nothing_checked = scan_spec_drift(&no_mirror, "2024-rec");

        // The premise: without this, the assertion below is vacuous.
        assert!(checked.violations.is_empty());
        assert!(nothing_checked.violations.is_empty());
        assert_ne!(
            checked.census, nothing_checked.census,
            "a store mirroring two sections and a store mirroring none must not \
             report the same reach behind the same empty finding list"
        );
        assert_eq!(checked.census.examined, 2);
        assert_eq!(nothing_checked.census.examined, 0);
        assert_eq!(nothing_checked.census.sections_with_excerpt, 0);
    }

    #[test]
    fn a_drifted_section_counts_as_examined() {
        // Examined is the scan's reach, not its pass count: finding the drift
        // IS having looked. A census that dropped drifted sections would make
        // `examined == 0` reachable on a store that reported drift.
        let store = store_with(&[("scxml-3.13", section_with_rev(Some("2020-rec"), None))]);
        let scan = scan_spec_drift(&store, "2024-rec");
        assert_eq!(scan.violations.len(), 1);
        assert_eq!(scan.census.examined, 1);
        assert_eq!(scan.census.exempt_by_status, 0);
    }

    #[test]
    fn a_status_exempt_mirror_is_counted_but_not_examined() {
        // Superseded sections mirror a spec (so they are not "nothing here")
        // yet are deliberately skipped (so they are not reach either). The two
        // counters keep those distinct; one number could not.
        let store = store_with(&[
            (
                "scxml-3.13",
                section_with_rev(Some("2020-rec"), Some(DecisionStatus::Superseded)),
            ),
            ("scxml-4.1", section_with_rev(Some("2024-rec"), None)),
        ]);
        let scan = scan_spec_drift(&store, "2024-rec");
        assert!(scan.violations.is_empty());
        assert_eq!(scan.census.sections_with_excerpt, 2);
        assert_eq!(scan.census.exempt_by_status, 1);
        assert_eq!(scan.census.examined, 1);
        // The invariant that makes the three numbers readable together.
        assert_eq!(
            scan.census.examined + scan.census.exempt_by_status,
            scan.census.sections_with_excerpt
        );
    }
}
