//! R419/R420 — confirmation gate (max-rigor v1). For every `Normative` +
//! `Dedicated` section, each `Verifies` binding must map to a `Confirmed` claim
//! in the event log; otherwise it is an unconfirmed gap. Opt-in via
//! `severity_confirmation` — `None` (the default) means the gate is disabled and
//! costs nothing.
//!
//! Layers one rung above the R413 verify axis: `is_verification_gap` checks a
//! `verifies` test EXISTS; this gate checks it was independently re-verified
//! (design sec 12.5).
//!
//! R420 — DRIFT. A confirmation is only valid while the artifacts it examined are
//! unchanged. This module re-hashes the bound artifacts (the file-reading half
//! the core deliberately omits, design sec 4.6) and feeds a validity predicate to
//! `confirmation_report_with`; an event whose `artifact_hashes` diverged drops out
//! of the valid set, so a previously-confirmed claim becomes `Stale` and the gate
//! flags it again. Empty hashes are *unrevalidatable*, not drift (R404 rule).

use std::collections::HashMap;
use std::path::Path;

use mnemosyne_atomic::{
    confirmation_report_with, AtomicStore, ConfirmationClaim, ConfirmationEvent, ConfirmationStatus,
};
use mnemosyne_core::{
    AtomicSnapshot, BindingKind, CoverageExpectation, DecisionStatus, VerificationExpectation,
};

/// One `Normative` + `Dedicated` `Verifies` binding whose claim is not yet
/// `Confirmed`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnconfirmedBinding {
    pub section_id: String,
    pub file: String,
    pub symbol: Option<String>,
    /// The claim's current status: `Proposed` (no/insufficient evidence),
    /// `Refuted` (an open refutation), or `Stale` (a confirm drifted out, R420).
    pub status: ConfirmationStatus,
}

fn hash_file(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    Some(mnemosyne_core::sha256_hex(&bytes))
}

/// R420 drift check: has any artifact the event examined changed since? Returns
/// `true` (drifted) if a recorded, non-empty hash no longer matches the live
/// artifact. Empty hash lists are unrevalidatable (not drift). The mapping from
/// hashes to files is the binding graph: `spec` = the claim section's
/// `normative_excerpt`; `test` = the verifies binding's own file; `code` = the
/// section's `implements` binding files.
fn artifact_drifted(event: &ConfirmationEvent, store: &AtomicStore, workspace_root: &Path) -> bool {
    let section_id = event.claim.section_id();
    let section = store.sections.get(section_id);

    // spec — store-only (R404 text_sha256 reuse).
    if let Some(spec) = event.artifact_hashes.spec_sha256.as_deref() {
        if !spec.is_empty() {
            if let Some(exc) = section.and_then(|s| s.normative_excerpt.as_ref()) {
                if !exc.excerpt.text_sha256.is_empty() && exc.excerpt.text_sha256 != spec {
                    return true;
                }
            }
        }
    }
    // test — the verifies binding's own file.
    if !event.artifact_hashes.test_sha256.is_empty() {
        if let ConfirmationClaim::VerifiesBinding { file, .. } = &event.claim {
            match hash_file(&workspace_root.join(file)) {
                Some(actual) if event.artifact_hashes.test_sha256.contains(&actual) => {}
                _ => return true, // changed, missing, or hash not among those recorded
            }
        }
    }
    // code — the section's `implements` binding files.
    if !event.artifact_hashes.code_sha256.is_empty() {
        if let Some(section) = section {
            for b in &section.bindings {
                if matches!(b.kind, BindingKind::Implements) {
                    match hash_file(&workspace_root.join(&b.file)) {
                        Some(actual) if event.artifact_hashes.code_sha256.contains(&actual) => {}
                        _ => return true,
                    }
                }
            }
        }
    }
    false
}

/// Scan the confirmation gate (R419 + R420 drift). A `Verifies` binding is
/// unconfirmed iff its `VerifiesBinding` claim is anything but `Confirmed` once
/// drifted events are dropped (`Proposed` / `Refuted` / `Stale`). The claim key
/// is `(section_id, file, symbol)` — it must match the binding exactly.
pub fn scan_confirmation_gate(
    snapshot: &AtomicSnapshot,
    store: &AtomicStore,
    workspace_root: &Path,
    catalog: Option<&crate::verifies_linkage::VerifiesCatalog>,
) -> Vec<UnconfirmedBinding> {
    let status_of: HashMap<ConfirmationClaim, ConfirmationStatus> =
        confirmation_report_with(store, |e| !artifact_drifted(e, store, workspace_root))
            .claims
            .into_iter()
            .map(|c| (c.claim, c.status))
            .collect();
    let mut out = Vec::new();
    for (section_id, section) in &snapshot.sections {
        let exempt = section
            .decision_status
            .unwrap_or(DecisionStatus::Active)
            .is_axiom_exempt();
        if exempt
            || !matches!(section.coverage_expectation, CoverageExpectation::Normative)
            || !matches!(
                section.verification_expectation,
                VerificationExpectation::Dedicated
            )
        {
            continue;
        }
        for b in &section.bindings {
            if !matches!(b.kind, BindingKind::Verifies) {
                continue;
            }
            let claim = ConfirmationClaim::VerifiesBinding {
                section_id: section_id.clone(),
                file: b.file.clone(),
                symbol: b.symbol.clone(),
            };
            let status = status_of
                .get(&claim)
                .copied()
                .unwrap_or(ConfirmationStatus::Proposed);
            // R427 (SCE P3) — catalog-live branch: an authoritative
            // deterministic exact match in the configured catalog confirms the
            // claim, re-verified on EVERY run (stronger than any stored event:
            // a snapshot can go stale, the live check cannot). An open
            // refutation still blocks (a refute outweighs, design sec 8).
            if status != ConfirmationStatus::Refuted {
                if let Some(cat) = catalog {
                    if crate::verifies_linkage::catalog_declares(
                        cat,
                        &b.file,
                        b.symbol.as_deref(),
                        section_id,
                    ) {
                        continue;
                    }
                }
            }
            if status != ConfirmationStatus::Confirmed {
                out.push(UnconfirmedBinding {
                    section_id: section_id.clone(),
                    file: b.file.clone(),
                    symbol: b.symbol.clone(),
                    status,
                });
            }
        }
    }
    out
}
