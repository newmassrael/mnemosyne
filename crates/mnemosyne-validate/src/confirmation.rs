//! R419 — confirmation gate (max-rigor v1). For every `Normative` + `Dedicated`
//! section, each `Verifies` binding must map to a `Confirmed` claim in the
//! event log; otherwise it is an unconfirmed gap. Pure over the binding-graph
//! snapshot + the store's confirmation events. Opt-in via `severity_confirmation`
//! — `None` (the default) means the gate is disabled and costs nothing.
//!
//! Layers one rung above the R413 verify axis: `is_verification_gap` checks a
//! `verifies` test EXISTS; this gate checks it was independently re-verified
//! (design sec 12.5). Drift/staleness (the code/test artifact-hash substrate) is
//! out of scope here — that lands in R420 and adds a `Stale` status; this scan
//! treats every stored event as current.

use std::collections::HashMap;

use mnemosyne_atomic::{confirmation_report, AtomicStore, ConfirmationClaim, ConfirmationStatus};
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
    /// The claim's current status: `Proposed` (no events yet, or the
    /// required-evidence-set is unmet) or `Refuted` (an open refutation).
    pub status: ConfirmationStatus,
}

/// Scan the confirmation gate (R419). A `Verifies` binding is unconfirmed iff
/// its `VerifiesBinding` claim is anything but `Confirmed` (including the
/// "no events yet" case, which the projection reports as `Proposed`). The claim
/// key is `(section_id, file, symbol)` — it must match the binding exactly, so a
/// confirmation recorded against a different file/symbol does not count.
pub fn scan_confirmation_gate(
    snapshot: &AtomicSnapshot,
    store: &AtomicStore,
) -> Vec<UnconfirmedBinding> {
    let status_of: HashMap<ConfirmationClaim, ConfirmationStatus> = confirmation_report(store)
        .claims
        .into_iter()
        .map(|c| (c.claim, c.status))
        .collect();
    let mut out = Vec::new();
    for (section_id, section) in &snapshot.sections {
        let removed =
            section.decision_status.unwrap_or(DecisionStatus::Active) == DecisionStatus::Removed;
        if removed
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
