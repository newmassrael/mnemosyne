//! Commit↔ledger consistency surface (Round 293).
//!
//! Pure set-difference between two `BTreeSet<u32>` inputs:
//! - `cited_rounds`: round numbers extracted from recent git commit subjects
//!   (e.g. `R291` / `(R293)` mentions; collected by the CLI thin wrapper).
//! - `ledger_rounds`: round numbers parsed from `AtomicStore.changelog_entries`
//!   keys (`Round NNN — ...`).
//!
//! `missing` = cited but absent from ledger → audit-trail hole (R291 hole that
//! triggered this round). `extra` = in ledger but not cited within the scan
//! window → expected for older rounds whose commits fell out of the bound;
//! reported informationally only.
//!
//! Kept dep-free (no git, no IO) so it can be unit-tested cheaply and reused
//! by future axes (CI, pre-commit, alternative VCS frontends).

use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitLedgerDriftReport {
    pub cited_count: usize,
    pub ledger_count: usize,
    pub missing: Vec<u32>,
    pub extra: Vec<u32>,
}

impl CommitLedgerDriftReport {
    pub fn is_clean(&self) -> bool {
        self.missing.is_empty()
    }
}

pub fn diff(
    cited_rounds: &BTreeSet<u32>,
    ledger_rounds: &BTreeSet<u32>,
) -> CommitLedgerDriftReport {
    let missing: Vec<u32> = cited_rounds.difference(ledger_rounds).copied().collect();
    let extra: Vec<u32> = ledger_rounds.difference(cited_rounds).copied().collect();
    CommitLedgerDriftReport {
        cited_count: cited_rounds.len(),
        ledger_count: ledger_rounds.len(),
        missing,
        extra,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(items: &[u32]) -> BTreeSet<u32> {
        items.iter().copied().collect()
    }

    #[test]
    fn clean_when_cited_subset_of_ledger() {
        let report = diff(&s(&[290, 292]), &s(&[290, 291, 292]));
        assert!(report.is_clean());
        assert!(report.missing.is_empty());
        assert_eq!(report.extra, vec![291]);
    }

    #[test]
    fn missing_round_surfaces_when_cited_but_absent() {
        // R291 hole that triggered this round.
        let report = diff(&s(&[290, 291, 292]), &s(&[290, 292]));
        assert!(!report.is_clean());
        assert_eq!(report.missing, vec![291]);
        assert!(report.extra.is_empty());
    }

    #[test]
    fn empty_inputs_are_clean() {
        let report = diff(&BTreeSet::new(), &BTreeSet::new());
        assert!(report.is_clean());
        assert_eq!(report.cited_count, 0);
        assert_eq!(report.ledger_count, 0);
    }

    #[test]
    fn cited_count_and_ledger_count_reflect_inputs() {
        let report = diff(&s(&[1, 2, 3]), &s(&[2, 3, 4, 5]));
        assert_eq!(report.cited_count, 3);
        assert_eq!(report.ledger_count, 4);
        assert_eq!(report.missing, vec![1]);
        assert_eq!(report.extra, vec![4, 5]);
    }

    #[test]
    fn missing_and_extra_sorted_ascending() {
        let report = diff(&s(&[100, 50, 200]), &s(&[50, 75, 150]));
        assert_eq!(report.missing, vec![100, 200]);
        assert_eq!(report.extra, vec![75, 150]);
    }
}
