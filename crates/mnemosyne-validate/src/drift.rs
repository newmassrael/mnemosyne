//! Claim-vs-evidence drift detection (Round 481) — the depth ladder's first
//! rung (R476). Scale-floor (R475) exposed it: a store fact can CLAIM "setup X
//! pays off" and pass every deterministic gate while the PROSE never delivers —
//! the gate validates the store's internal consistency, never whether a claim is
//! borne out by the evidence it cites. That is a SEMANTIC check; it needs an
//! LLM, recorded through the R428 confirmation machinery (a `FactEvidence`
//! claim, model confirmer, `semantic_review`, self-confirm reject, staleness).
//!
//! This module is the READ half: the drift predicate (`fact_claim_drifted`) and
//! the candidate/surface report (`drift_candidates`). The WRITE half is
//! `mnemosyne_atomic::import_drift_verdicts`. v1 scope = payoff facts (a fact
//! with a non-empty `pays_off` — precisely the loop's "12/12 paid" case).
//! Venue-neutral by construction: the claim-vs-evidence shape is identical for
//! spec `normative_excerpt` sections, a later extension.

use std::collections::BTreeMap;

use mnemosyne_atomic::{
    confirmation_report_with, AtomicStore, ClaimTarget, ConfirmationClaim, ConfirmationEvent,
};
use serde::Serialize;

/// R420 drift, narrative-fact flavor: has the fact's claim text changed since
/// the reviewer judged it? Store-only (no files — facts are store-resident).
/// `true` when the stamped `spec_sha256` is present, non-empty, and no longer
/// matches `sha256(live fact.claim)`. An empty hash is unrevalidatable, not
/// drift (the R404 rule). Returns `false` for any non-`FactEvidence` event, so
/// the drift projection leaves the verifies events untouched.
pub fn fact_claim_drifted(event: &ConfirmationEvent, store: &AtomicStore) -> bool {
    let ClaimTarget::Fact(fact_id) = event.claim.target() else {
        return false;
    };
    let Some(stamped) = event.artifact_hashes.spec_sha256.as_deref() else {
        return false;
    };
    if stamped.is_empty() {
        return false;
    }
    match store.narrative_facts.get(fact_id) {
        Some(fact) => mnemosyne_core::sha256_hex(fact.claim.as_bytes()) != stamped,
        // The fact was retracted after review — the verdict is about nothing
        // live; treat as drifted so it never counts as a current confirmation.
        None => true,
    }
}

/// Per-fact drift status, projected from its `FactEvidence` confirmation events
/// (over the events still valid under `fact_claim_drifted`). A `FactEvidence`
/// claim never reaches `Confirmed` via the verifies required-evidence-set (no
/// deterministic tool can judge prose), so the surface reads the semantic-review
/// leg directly rather than `ConfirmationStatus::Confirmed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftStatus {
    /// At least one independent semantic-review Confirm, no open Refute — the
    /// claim is borne out by its evidence (not drifting).
    Reviewed,
    /// An open Refute — a reviewer judged the claim NOT borne out (drift).
    Refuted,
    /// No verdict recorded yet (the work queue).
    Unreviewed,
    /// Had a Confirm that drifted out (the claim text changed after review) —
    /// demands re-review, distinct from never-reviewed.
    Stale,
}

impl DriftStatus {
    /// Everything but `Reviewed` is the drift surface (the design's "Refuted or
    /// never independently Confirmed").
    pub fn is_drift(self) -> bool {
        self != DriftStatus::Reviewed
    }
}

/// One payoff fact's row — the LLM input contract (claim + quote + sha to stamp)
/// AND the current drift status (the surface), in one read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DriftCandidate {
    pub fact_id: String,
    pub frame: String,
    pub branch: String,
    pub claim: String,
    /// sha256 of `claim` — the reviewer stamps this into each verdict (R439 pin).
    pub claim_sha256: String,
    pub canon_from: String,
    /// The verbatim evidence quote, when the fact carries one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote: Option<String>,
    /// Structure-section ids evidencing the claim (≥ 1 by construction).
    pub evidence: Vec<String>,
    /// The setup fact ids this fact claims to pay off (non-empty for a v1
    /// candidate).
    pub pays_off: Vec<String>,
    pub status: DriftStatus,
    pub confirm_count: usize,
    pub refute_count: usize,
}

/// The drift-detection report: every payoff fact with its review status.
#[derive(Debug, Clone, Default, Serialize)]
pub struct DriftReport {
    /// Payoff facts, id-sorted — the LLM input contract and the drift surface.
    pub candidates: Vec<DriftCandidate>,
    /// Total payoff facts (candidates).
    pub payoff_facts: usize,
    /// Reviewed (not drifting).
    pub reviewed: usize,
    /// Drifting (refuted, unreviewed, or stale) — the work surface.
    pub drifting: usize,
}

/// Collect drift candidates (Round 481). Order-independent — claim-vs-evidence
/// is a property of the fact, not of any canon declaration — so no order
/// resolution runs. Pure read projection; the substrate contains no LLM client.
pub fn drift_candidates(store: &AtomicStore) -> Result<DriftReport, String> {
    // Confirmation projection over the events still valid under fact drift.
    let report = confirmation_report_with(store, |e| !fact_claim_drifted(e, store));
    let mut by_fact: BTreeMap<&str, DriftStatus> = BTreeMap::new();
    let mut counts: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for c in &report.claims {
        let ConfirmationClaim::FactEvidence { fact_id } = &c.claim else {
            continue;
        };
        let status = if c.refute_count > 0 {
            DriftStatus::Refuted
        } else if c.independent_semantic >= 1 {
            DriftStatus::Reviewed
        } else if c.stale_count > 0 {
            DriftStatus::Stale
        } else {
            DriftStatus::Unreviewed
        };
        by_fact.insert(fact_id.as_str(), status);
        counts.insert(fact_id.as_str(), (c.confirm_count, c.refute_count));
    }

    let mut candidates: Vec<DriftCandidate> = Vec::new();
    for (fact_id, f) in &store.narrative_facts {
        if f.pays_off.is_empty() {
            continue;
        }
        let status = by_fact
            .get(fact_id.as_str())
            .copied()
            .unwrap_or(DriftStatus::Unreviewed);
        let (confirm_count, refute_count) = counts.get(fact_id.as_str()).copied().unwrap_or((0, 0));
        candidates.push(DriftCandidate {
            fact_id: fact_id.clone(),
            frame: f.frame.clone(),
            branch: f.branch.clone(),
            claim_sha256: mnemosyne_core::sha256_hex(f.claim.as_bytes()),
            claim: f.claim.clone(),
            canon_from: f.canon_from.clone(),
            quote: f.quote.clone(),
            evidence: f.evidence.clone(),
            pays_off: f.pays_off.clone(),
            status,
            confirm_count,
            refute_count,
        });
    }
    // narrative_facts is a BTreeMap, so candidates are already id-sorted.
    let reviewed = candidates.iter().filter(|c| !c.status.is_drift()).count();
    Ok(DriftReport {
        payoff_facts: candidates.len(),
        reviewed,
        drifting: candidates.len() - reviewed,
        candidates,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mnemosyne_atomic::{
        ArtifactHashes, ConfirmMethod, ConfirmationEvent, Confirmer, ConfirmerKind, Verdict,
    };
    use mnemosyne_core::{NarrativeFact, PayoffExpectation};

    fn payoff_fact(claim: &str) -> NarrativeFact {
        NarrativeFact {
            frame: "gt".to_string(),
            branch: "main".to_string(),
            entities: vec![],
            claim: claim.to_string(),
            canon_from: "sc-10".to_string(),
            canon_to: None,
            evidence: vec!["sc-10".to_string()],
            conflicts_with: vec![],
            supersedes_in_frame: None,
            payoff_expectation: PayoffExpectation::Unmarked,
            typed: None,
            pays_off: vec!["f-setup".to_string()],
            quote: Some("the page named Brandt".to_string()),
            quote_sha256: None,
        }
    }

    fn fact_event(
        fact_id: &str,
        verdict: Verdict,
        confirming: &str,
        claim_sha: &str,
    ) -> ConfirmationEvent {
        ConfirmationEvent {
            claim: ConfirmationClaim::FactEvidence {
                fact_id: fact_id.to_string(),
            },
            confirmer: Confirmer {
                kind: ConfirmerKind::Model,
                id: "claude-opus-4-8".to_string(),
                version: "2026-06".to_string(),
            },
            method: ConfirmMethod::SemanticReview,
            artifact_hashes: ArtifactHashes {
                spec_sha256: Some(claim_sha.to_string()),
                code_sha256: vec![],
                test_sha256: vec![],
            },
            authoring_run: "author".to_string(),
            confirming_run: confirming.to_string(),
            verdict,
            rationale: "reviewed".to_string(),
            timestamp: "2026-06-12T00:00:00Z".to_string(),
        }
    }

    fn store_with(claim: &str) -> (AtomicStore, String) {
        let mut store = AtomicStore::new();
        store
            .narrative_facts
            .insert("f-pay".to_string(), payoff_fact(claim));
        let sha = mnemosyne_core::sha256_hex(claim.as_bytes());
        (store, sha)
    }

    #[test]
    fn fact_claim_drifted_matches_live_claim() {
        let (store, sha) = store_with("the diary names the killer");
        let fresh = fact_event("f-pay", Verdict::Confirm, "rev", &sha);
        assert!(
            !fact_claim_drifted(&fresh, &store),
            "matching sha = not drifted"
        );
        let stale = fact_event("f-pay", Verdict::Confirm, "rev", "deadbeef");
        assert!(
            fact_claim_drifted(&stale, &store),
            "mismatched sha = drifted"
        );
        let mut empty = fresh.clone();
        empty.artifact_hashes.spec_sha256 = Some(String::new());
        assert!(
            !fact_claim_drifted(&empty, &store),
            "empty hash = unrevalidatable, not drift"
        );
    }

    #[test]
    fn unreviewed_payoff_fact_is_the_default_drift() {
        let (store, _) = store_with("the diary names the killer");
        let report = drift_candidates(&store).unwrap();
        assert_eq!(report.payoff_facts, 1);
        assert_eq!(report.drifting, 1);
        assert_eq!(report.candidates[0].status, DriftStatus::Unreviewed);
    }

    #[test]
    fn independent_confirm_clears_drift() {
        let (mut store, sha) = store_with("the diary names the killer");
        store.confirmation_events.insert(
            "evt-1".to_string(),
            fact_event("f-pay", Verdict::Confirm, "rev", &sha),
        );
        let report = drift_candidates(&store).unwrap();
        assert_eq!(report.candidates[0].status, DriftStatus::Reviewed);
        assert_eq!(report.drifting, 0);
        assert_eq!(report.candidates[0].confirm_count, 1);
    }

    #[test]
    fn a_refute_is_drift_even_with_a_confirm() {
        let (mut store, sha) = store_with("the diary names the killer");
        store.confirmation_events.insert(
            "evt-1".to_string(),
            fact_event("f-pay", Verdict::Confirm, "rev", &sha),
        );
        store.confirmation_events.insert(
            "evt-2".to_string(),
            fact_event("f-pay", Verdict::Refute, "rev2", &sha),
        );
        let report = drift_candidates(&store).unwrap();
        assert_eq!(report.candidates[0].status, DriftStatus::Refuted);
        assert_eq!(report.drifting, 1);
    }

    #[test]
    fn a_drifted_confirm_goes_stale() {
        let (mut store, _) = store_with("the diary names the killer");
        // A confirm stamped with a sha that no longer matches the live claim.
        store.confirmation_events.insert(
            "evt-1".to_string(),
            fact_event("f-pay", Verdict::Confirm, "rev", "outdatedsha"),
        );
        let report = drift_candidates(&store).unwrap();
        assert_eq!(report.candidates[0].status, DriftStatus::Stale);
        assert_eq!(report.drifting, 1);
    }

    #[test]
    fn non_payoff_fact_is_not_a_candidate() {
        let mut store = AtomicStore::new();
        let mut f = payoff_fact("a plain fact");
        f.pays_off = vec![]; // no payoff claim -> not a v1 candidate
        store.narrative_facts.insert("f-plain".to_string(), f);
        let report = drift_candidates(&store).unwrap();
        assert_eq!(report.payoff_facts, 0);
    }
}
