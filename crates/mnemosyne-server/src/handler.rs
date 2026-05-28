//! Proposal handler — orchestrates the Phase 0 server pipeline:
//!
//! 1. Tier 1 gate (semantic) — accept / reject.
//! 2. Tier 2 gate (structural) — accept / reject (Phase 0 stub).
//! 3. Tier 3 gate (convention) — collect warnings.
//! 4. On accept: audit-append + storage commit.
//! 5. On reject: audit-append (rejection record) + return rejection result.

use crate::audit::{append_outcome_with_trace_context, AuditAppender, AuditFanout, TraceContext};
use crate::error::Result;
use crate::gate::{
    DefaultTier1, DefaultTier2, DefaultTier3, GateOutcome, GateTier, Tier1Gate, Tier2Gate,
    Tier3Gate,
};
use crate::proposal::{Proposal, ProposalKind, ProposalResult};
use mnemosyne_store::{CfId, MnemosyneStore};
use std::sync::Arc;

pub struct ProposalHandler {
    store: Arc<MnemosyneStore>,
    audit: AuditAppender,
    tier1: Box<dyn Tier1Gate + Send + Sync>,
    tier2: Box<dyn Tier2Gate + Send + Sync>,
    tier3: Box<dyn Tier3Gate + Send + Sync>,
}

impl ProposalHandler {
    pub fn new(store: Arc<MnemosyneStore>) -> Self {
        let audit = AuditAppender::new(Arc::clone(&store));
        Self {
            store,
            audit,
            tier1: Box::new(DefaultTier1),
            tier2: Box::new(DefaultTier2),
            tier3: Box::new(DefaultTier3),
        }
    }

    /// construct with an explicit audit broadcast channel capacity.
    /// Wires through to [`AuditAppender::with_broadcast_capacity`]. Used by
    /// integration tests / sizing benchmarks that need a deterministic
    /// Lagged threshold.
    pub fn with_audit_broadcast_capacity(store: Arc<MnemosyneStore>, capacity: usize) -> Self {
        let audit = AuditAppender::with_broadcast_capacity(Arc::clone(&store), capacity);
        Self {
            store,
            audit,
            tier1: Box::new(DefaultTier1),
            tier2: Box::new(DefaultTier2),
            tier3: Box::new(DefaultTier3),
        }
    }

    /// construct with a custom [`AuditFanout`] for cross-process
    /// audit observation. The fanout's `publish` runs after every
    /// successful audit write, so observer servers attached to the same
    /// fanout backends see commits from this server in real time.
    pub fn with_audit_fanout(
        store: Arc<MnemosyneStore>,
        capacity: usize,
        fanout: Arc<dyn AuditFanout>,
    ) -> Self {
        let audit =
            AuditAppender::with_broadcast_capacity_and_fanout(Arc::clone(&store), capacity, fanout);
        Self {
            store,
            audit,
            tier1: Box::new(DefaultTier1),
            tier2: Box::new(DefaultTier2),
            tier3: Box::new(DefaultTier3),
        }
    }

    /// Replace the Tier 1 gate (used by tests + Phase 0+ richer rules).
    pub fn with_tier1(mut self, gate: Box<dyn Tier1Gate + Send + Sync>) -> Self {
        self.tier1 = gate;
        self
    }

    pub fn handle(&self, proposal: &Proposal) -> Result<ProposalResult> {
        self.handle_with_trace_context(proposal, &TraceContext::default())
    }

    /// + pipeline entry carrying an explicit
    /// [`TraceContext`] (trace_id + tracestate). Both fields flow into every
    /// audit record this proposal produces (accept *or* reject), so
    /// observability tooling can join logs across the gRPC layer and the
    /// audit trail by trace_id and preserves vendor-specific tracestate.
    ///
    /// instruments each gate evaluation and the audit append as
    /// nested `tracing` spans (`gate.evaluate` with `tier` attribute,
    /// `audit.append`) parented to the active span at call time. When the
    /// caller is the gRPC service the parent span is the entry-level RPC
    /// span, giving an OTLP exporter a full hierarchy
    /// without further code changes.
    pub fn handle_with_trace_context(
        &self,
        proposal: &Proposal,
        ctx: &TraceContext,
    ) -> Result<ProposalResult> {
        // Tier 1.
        let t1 = tracing::info_span!("gate.evaluate", tier = 1u8)
            .in_scope(|| self.tier1.evaluate(proposal));
        if let GateOutcome::Reject { tier, reason } = &t1 {
            let txn = tracing::info_span!("audit.append", outcome = "reject", tier = ?tier)
                .in_scope(|| {
                    self.audit
                        .append_rejected_with_trace_context(proposal, *tier, reason, ctx)
                })?;
            return Ok(ProposalResult {
                proposal_id: proposal.proposal_id.clone(),
                accepted: false,
                audit_transaction_id: Some(txn),
                rejection_reason: Some(reason.clone()),
            });
        }

        // Tier 2.
        let t2 = tracing::info_span!("gate.evaluate", tier = 2u8)
            .in_scope(|| self.tier2.evaluate(proposal));
        if let GateOutcome::Reject { tier, reason } = &t2 {
            let txn = tracing::info_span!("audit.append", outcome = "reject", tier = ?tier)
                .in_scope(|| {
                    self.audit
                        .append_rejected_with_trace_context(proposal, *tier, reason, ctx)
                })?;
            return Ok(ProposalResult {
                proposal_id: proposal.proposal_id.clone(),
                accepted: false,
                audit_transaction_id: Some(txn),
                rejection_reason: Some(reason.clone()),
            });
        }

        // Tier 3 — warnings only.
        let mut warnings: Vec<String> = tracing::info_span!("gate.evaluate", tier = 3u8)
            .in_scope(|| self.tier3.evaluate(proposal));
        if let GateOutcome::Accept { warnings: w1 } = t1 {
            warnings.extend(w1);
        }
        if let GateOutcome::Accept { warnings: w2 } = t2 {
            warnings.extend(w2);
        }

        // Commit storage write + audit append-only.
        self.commit_storage(proposal)?;
        let outcome = GateOutcome::Accept { warnings };
        let txn = tracing::info_span!("audit.append", outcome = "accept")
            .in_scope(|| append_outcome_with_trace_context(&self.audit, proposal, &outcome, ctx))?;
        Ok(ProposalResult {
            proposal_id: proposal.proposal_id.clone(),
            accepted: true,
            audit_transaction_id: Some(txn),
            rejection_reason: None,
        })
    }

    /// atomic batch handler. Evaluates gates on every proposal
    /// in order; if any proposal rejects, EVERY proposal in the batch is
    /// rejected (per-proposal rejection result + per-proposal audit
    /// record carrying the original rejection reason from the failing
    /// proposal). When every proposal accepts, all storage writes commit
    /// in a single `write_batch_multi_cf` call — RocksDB's all-or-nothing
    /// transactional batch.
    ///
    /// Audit records are still emitted per proposal (per the audit
    /// append-only invariant); the atomicity guarantee covers the
    /// *entity / relations* CFs, not the audit ledger. A rejected batch
    /// audits each proposal as rejected with reason
    /// `"atomic batch rejected: {first reject's reason}"`.
    pub fn handle_batch_atomic(
        &self,
        proposals: &[Proposal],
        ctx: &TraceContext,
    ) -> Result<Vec<ProposalResult>> {
        if proposals.is_empty() {
            return Ok(Vec::new());
        }

        // Phase 1 — evaluate gates on every proposal up front.
        let mut outcomes: Vec<(GateOutcome, Vec<String>)> = Vec::with_capacity(proposals.len());
        let mut first_reject: Option<String> = None;
        for p in proposals {
            let t1 = tracing::info_span!("gate.evaluate", tier = 1u8)
                .in_scope(|| self.tier1.evaluate(p));
            if let GateOutcome::Reject { reason, .. } = &t1 {
                if first_reject.is_none() {
                    first_reject = Some(reason.clone());
                }
            }
            let t2 = tracing::info_span!("gate.evaluate", tier = 2u8)
                .in_scope(|| self.tier2.evaluate(p));
            if let GateOutcome::Reject { reason, .. } = &t2 {
                if first_reject.is_none() {
                    first_reject = Some(reason.clone());
                }
            }
            let mut warnings: Vec<String> = tracing::info_span!("gate.evaluate", tier = 3u8)
                .in_scope(|| self.tier3.evaluate(p));
            if let GateOutcome::Accept { warnings: w1 } = &t1 {
                warnings.extend(w1.clone());
            }
            if let GateOutcome::Accept { warnings: w2 } = &t2 {
                warnings.extend(w2.clone());
            }
            // Encode outcome as the COMBINED gate outcome — accept iff
            // both t1 and t2 accept; reject otherwise (with the
            // rejecting tier's reason).
            let outcome = match (t1, t2) {
                (GateOutcome::Reject { tier, reason }, _)
                | (GateOutcome::Accept { .. }, GateOutcome::Reject { tier, reason }) => {
                    GateOutcome::Reject { tier, reason }
                }
                (GateOutcome::Accept { .. }, GateOutcome::Accept { .. }) => GateOutcome::Accept {
                    warnings: warnings.clone(),
                },
            };
            outcomes.push((outcome, warnings));
        }

        // Phase 2 — branch on whether any reject happened.
        if let Some(batch_reject_reason) = first_reject {
            // Atomic reject — every proposal gets a rejection result.
            let composite_reason = format!("atomic batch rejected: {batch_reject_reason}");
            let mut results = Vec::with_capacity(proposals.len());
            for (p, (outcome, _w)) in proposals.iter().zip(outcomes.into_iter()) {
                let (tier, _orig_reason) = match outcome {
                    GateOutcome::Reject { tier, reason } => (tier, reason),
                    // Even accepted proposals get rejected under atomic.
                    GateOutcome::Accept { .. } => (GateTier::Tier1, String::new()),
                };
                let txn = tracing::info_span!("audit.append", outcome = "atomic_reject").in_scope(
                    || {
                        self.audit.append_rejected_with_trace_context(
                            p,
                            tier,
                            &composite_reason,
                            ctx,
                        )
                    },
                )?;
                results.push(ProposalResult {
                    proposal_id: p.proposal_id.clone(),
                    accepted: false,
                    audit_transaction_id: Some(txn),
                    rejection_reason: Some(composite_reason.clone()),
                });
            }
            return Ok(results);
        }

        // Phase 3 — every proposal accepted; commit storage in one batch.
        let mut storage_entries: Vec<(CfId, u64, u64, u64, Vec<u8>)> = Vec::new();
        for p in proposals {
            if let Some(tuple) = proposal_storage_tuple(p) {
                storage_entries.push(tuple);
            }
        }
        if !storage_entries.is_empty() {
            self.store.write_batch_multi_cf(&storage_entries)?;
        }

        let mut results = Vec::with_capacity(proposals.len());
        for (p, (outcome, _warnings)) in proposals.iter().zip(outcomes.into_iter()) {
            let txn = tracing::info_span!("audit.append", outcome = "atomic_accept")
                .in_scope(|| append_outcome_with_trace_context(&self.audit, p, &outcome, ctx))?;
            results.push(ProposalResult {
                proposal_id: p.proposal_id.clone(),
                accepted: true,
                audit_transaction_id: Some(txn),
                rejection_reason: None,
            });
        }
        Ok(results)
    }

    fn commit_storage(&self, proposal: &Proposal) -> Result<()> {
        match &proposal.kind {
            ProposalKind::EntityCreate {
                branch_id,
                entity_id,
                valid_from,
                payload,
                ..
            }
            | ProposalKind::EntityUpdate {
                branch_id,
                entity_id,
                valid_from,
                payload,
                ..
            }
            | ProposalKind::ChangelogAppend {
                branch_id,
                entity_id,
                valid_from,
                payload,
            } => {
                self.store
                    .put(CfId::Entities, *branch_id, *entity_id, *valid_from, payload)?;
            }
            ProposalKind::CrossRefCreate {
                branch_id,
                from_section,
                to_section,
                ref_kind,
            } => {
                let value = ref_kind.as_bytes();
                self.store.put(
                    CfId::Relations,
                    *branch_id,
                    *from_section,
                    *to_section,
                    value,
                )?;
            }
            ProposalKind::FrozenListMembershipChange {
                branch_id,
                list_id,
                valid_from,
                payload,
                ..
            } => {
                self.store
                    .put(CfId::Entities, *branch_id, *list_id, *valid_from, payload)?;
            }
        }
        Ok(())
    }

    pub fn audit(&self) -> &AuditAppender {
        &self.audit
    }
}

/// extract the storage write tuple from a proposal for use in
/// `write_batch_multi_cf`. Returns `None` when the proposal has no
/// storage side effect (currently every kind writes; this stays a
/// nullable shape so future read-only proposal kinds plug in cleanly).
fn proposal_storage_tuple(proposal: &Proposal) -> Option<(CfId, u64, u64, u64, Vec<u8>)> {
    match &proposal.kind {
        ProposalKind::EntityCreate {
            branch_id,
            entity_id,
            valid_from,
            payload,
            ..
        }
        | ProposalKind::EntityUpdate {
            branch_id,
            entity_id,
            valid_from,
            payload,
            ..
        }
        | ProposalKind::ChangelogAppend {
            branch_id,
            entity_id,
            valid_from,
            payload,
        } => Some((
            CfId::Entities,
            *branch_id,
            *entity_id,
            *valid_from,
            payload.clone(),
        )),
        ProposalKind::CrossRefCreate {
            branch_id,
            from_section,
            to_section,
            ref_kind,
        } => Some((
            CfId::Relations,
            *branch_id,
            *from_section,
            *to_section,
            ref_kind.as_bytes().to_vec(),
        )),
        ProposalKind::FrozenListMembershipChange {
            branch_id,
            list_id,
            valid_from,
            payload,
            ..
        } => Some((
            CfId::Entities,
            *branch_id,
            *list_id,
            *valid_from,
            payload.clone(),
        )),
    }
}

impl ProposalHandler {
    pub fn store(&self) -> &Arc<MnemosyneStore> {
        &self.store
    }
}
// top-level closure of the inner impl block above; the helper
// `proposal_storage_tuple` lives between two impl blocks for legibility.

/// `MnemosyneServer` — embedded API facade. Wraps `ProposalHandler` for direct
/// in-process invocation. The gRPC transport layer (deferred) wraps this same
/// handler.
pub struct MnemosyneServer {
    handler: Arc<ProposalHandler>,
}

impl MnemosyneServer {
    pub fn new(store: Arc<MnemosyneStore>) -> Self {
        Self {
            handler: Arc::new(ProposalHandler::new(store)),
        }
    }

    pub fn handler(&self) -> &Arc<ProposalHandler> {
        &self.handler
    }

    /// Embedded API entry point — synchronous wrapper for callers in the same
    /// process. Async callers go through [`crate::service::MnemosyneService`].
    pub fn submit(&self, proposal: &Proposal) -> Result<ProposalResult> {
        self.handler.handle(proposal)
    }
}

/// Custom Tier 1 gate that always rejects (test helper for proposal_pipeline).
pub struct AlwaysRejectTier1 {
    pub reason: String,
}

impl Tier1Gate for AlwaysRejectTier1 {
    fn evaluate(&self, _proposal: &Proposal) -> GateOutcome {
        GateOutcome::Reject {
            tier: GateTier::Tier1,
            reason: self.reason.clone(),
        }
    }
}

/// Re-export ServerError for downstream `?`-propagation.
pub use crate::error::ServerError as HandlerError;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn fresh_server() -> (TempDir, MnemosyneServer) {
        let dir = TempDir::new().unwrap();
        let store = Arc::new(MnemosyneStore::open(dir.path()).unwrap());
        (dir, MnemosyneServer::new(store))
    }

    #[test]
    fn entity_create_round_trip_through_handler() {
        let (_dir, server) = fresh_server();
        let p = Proposal {
            proposal_id: "p-001".to_string(),
            actor: "alice".to_string(),
            kind: ProposalKind::EntityCreate {
                entity_type: "Section".to_string(),
                branch_id: 1,
                entity_id: 1,
                valid_from: 100,
                payload: b"section-payload".to_vec(),
            },
        };
        let result = server.submit(&p).unwrap();
        assert!(result.accepted);
        assert!(result.audit_transaction_id.is_some());
        assert!(result.rejection_reason.is_none());
        // Storage was committed.
        let stored = server
            .handler()
            .store()
            .get(CfId::Entities, 1, 1, 100)
            .unwrap();
        assert_eq!(stored.as_deref(), Some(b"section-payload".as_ref()));
    }

    #[test]
    fn cross_ref_orphan_rejected_at_tier1_with_audit_record() {
        let (_dir, server) = fresh_server();
        let p = Proposal {
            proposal_id: "p-002".to_string(),
            actor: "alice".to_string(),
            kind: ProposalKind::CrossRefCreate {
                branch_id: 1,
                from_section: 0, // orphan: from unresolved
                to_section: 39,
                ref_kind: "decision".to_string(),
            },
        };
        let result = server.submit(&p).unwrap();
        assert!(!result.accepted);
        assert!(result.rejection_reason.is_some());
        assert!(result.audit_transaction_id.is_some());
        // Storage was NOT committed.
        assert!(server
            .handler()
            .store()
            .get(CfId::Relations, 1, 0, 39)
            .unwrap()
            .is_none());
        // Audit record is `t1_reject`.
        let audit = server
            .handler()
            .audit()
            .read(result.audit_transaction_id.unwrap())
            .unwrap()
            .expect("audit");
        assert!(!audit.accepted);
        assert_eq!(audit.gate_routing_reason, "t1_reject");
    }

    #[test]
    fn always_reject_tier1_short_circuits_pipeline() {
        let _dir = TempDir::new().unwrap();
        let store = Arc::new(MnemosyneStore::open(_dir.path()).unwrap());
        let handler =
            ProposalHandler::new(Arc::clone(&store)).with_tier1(Box::new(AlwaysRejectTier1 {
                reason: "test override".to_string(),
            }));
        let p = Proposal {
            proposal_id: "p".to_string(),
            actor: "alice".to_string(),
            kind: ProposalKind::EntityCreate {
                entity_type: "Section".to_string(),
                branch_id: 1,
                entity_id: 1,
                valid_from: 100,
                payload: vec![],
            },
        };
        let r = handler.handle(&p).unwrap();
        assert!(!r.accepted);
        assert_eq!(r.rejection_reason.as_deref(), Some("test override"));
        // Storage NOT committed.
        assert!(store.get(CfId::Entities, 1, 1, 100).unwrap().is_none());
    }
}
