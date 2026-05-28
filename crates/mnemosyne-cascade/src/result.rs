//! Cascade validation output value object.
//!
//! Shared return type of the fine-grained engine's aggregator queries
//! ([`crate::fine_grained::section_decision_status_aggregated`] /
//! [`crate::fine_grained::frozen_list_membership_aggregated`]). A tracked
//! query's return must implement `salsa::Update`; the value is also `Hash` /
//! `Eq` so Salsa can backdate (skip downstream recompute when the result is
//! bit-equal across a mutation).

/// Cascade query output — `ok` plus the per-aggregator violation count.
#[derive(Default, Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub struct ValidationResult {
    pub ok: bool,
    pub violation_count: u32,
}

impl ValidationResult {
    pub fn ok() -> Self {
        Self {
            ok: true,
            violation_count: 0,
        }
    }

    pub fn violations(count: u32) -> Self {
        Self {
            ok: false,
            violation_count: count,
        }
    }
}
