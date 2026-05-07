//! Audit append-only enforcement — DESIGN §6 audit CF source of truth.
//!
//! `AuditAppender` writes one `AuditRecord` per accepted proposal (or per
//! rejection that needs durable record). Transaction id is a monotonic u64
//! seeded from `audit_cf.last_key + 1` at server startup (DESIGN §4.378).
//!
//! Append-only enforcement applies at both the API surface (no mutate /
//! delete operations) AND at the storage layer (DESIGN §15 secondary read on
//! audit CF is blocked, but writes are permitted because audit IS the
//! append-only target).

use crate::error::{Result, ServerError};
use crate::gate::{GateOutcome, GateTier};
use crate::proposal::Proposal;
use byteorder::{BigEndian, ByteOrder};
use mnemosyne_store::{CfId, MnemosyneStore};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditRecord {
 pub transaction_id: u64,
 pub proposal_id: String,
 pub actor: String,
 pub accepted: bool,
 pub gate_routing_reason: String,
 pub rejection_reason: Option<String>,
 pub proposal_kind_tag: String,
 /// Round 99 — W3C trace context propagated from the inbound request, or a
 /// server-generated UUID fallback when no `traceparent` was supplied.
 /// `#[serde(default)]` lets pre-Round 99 audit records (which never had
 /// this field on disk) decode cleanly as `None`.
 #[serde(default)]
 pub trace_id: Option<String>,
 /// Round 104 — W3C `tracestate` header passthrough. The vendor-specific
 /// key/value list from the inbound `tracestate` RPC metadata, propagated
 /// verbatim into the audit trail so observability tooling joining logs
 /// across the gRPC boundary preserves vendor context. `None` when the
 /// inbound RPC carried no tracestate or for embedded callers without an
 /// explicit context. `#[serde(default)]` keeps pre-Round 104 records
 /// (without this field) decodable as `None`.
 #[serde(default)]
 pub tracestate: Option<String>,
}

/// Round 104 — W3C trace context propagated alongside a proposal through the
/// pipeline. Composes `trace_id` (Round 99) and `tracestate` (Round 104) into a
/// single value-typed handle so audit-append signatures stay stable as future
/// trace propagation fields (baggage, etc.) extend the struct without further
/// signature churn.
///
/// `Default::default()` yields the embedded-caller shape (no trace context);
/// the gRPC entry point fills both fields from the inbound metadata.
#[derive(Debug, Clone, Default)]
pub struct TraceContext {
 pub trace_id: Option<String>,
 pub tracestate: Option<String>,
}

impl TraceContext {
 /// Convenience constructor for trace-id-only contexts (Round 99 carry path
 /// where no tracestate is available yet, or test fixtures that only
 /// exercise trace_id propagation).
 pub fn with_trace_id(trace_id: String) -> Self {
 Self {
 trace_id: Some(trace_id),
 tracestate: None,
 }
 }
}

/// Default capacity of the audit broadcast channel (Round 103). Sized so brief
/// bursts of commits do not stall slow subscribers; when a subscriber lags
/// past this capacity its `Receiver` returns `RecvError::Lagged` and the
/// gRPC tail loop closes the stream rather than corrupting ordering.
///
/// Round 109 — exposed as a `pub const` so callers (deployment harness, sizing
/// audit benchmarks, integration tests) can compare against the live value
/// instead of duplicating the magic number. The constructor [`AuditAppender::new`]
/// uses this default; callers needing a different size go through
/// [`AuditAppender::with_broadcast_capacity`].
pub const AUDIT_BROADCAST_CAPACITY: usize = 256;

/// Round 111 — outbound fanout trait for cross-process audit-record
/// distribution. The default Mnemosyne deployment runs single-process, so
/// the in-memory broadcast (Round 103) covers tail-following on its own.
/// Multi-server fanout (audit observers running on a different host than
/// the writing server) plugs an [`AuditFanout`] impl into the appender;
/// each successful `write` calls [`AuditFanout::publish`] *after* the
/// durable RocksDB commit.
///
/// Trait surface: write-side only. Inbound relay (external records →
/// local broadcast) is a transport-specific concern handled by the
/// concrete impl (the in-memory broker exposes
/// [`InMemoryAuditBroker::subscriber`] for that path; a redis-backed
/// or NATS impl spawns its own listener task).
///
/// Note on §6 framing: the audit append-only invariant lives on each
/// server's RocksDB audit CF and is unaffected by fanout — fanout is an
/// *observation* layer, not a write path. A cross-process subscriber
/// receives a copy of the record but never writes to the local audit CF
/// of the originating server. DESIGN.md §6 spec stays frozen.
pub trait AuditFanout: Send + Sync + std::fmt::Debug {
 fn publish(&self, record: &AuditRecord);
}

/// Default fanout: drops every published record. Used when an
/// [`AuditAppender`] is constructed via [`AuditAppender::new`] (single-
/// process deployment, no external subscribers).
#[derive(Debug, Default)]
pub struct NoopAuditFanout;

impl AuditFanout for NoopAuditFanout {
 fn publish(&self, _record: &AuditRecord) {}
}

/// Round 111 — in-memory implementation of the cross-process broker. Two
/// `AuditAppender` instances, each holding an [`InMemoryAuditPublisher`]
/// pointing at the same broker, observe each other's commits without any
/// network hop. Used by integration tests in lieu of a real
/// redis/NATS/Kafka backend; the transport surface is identical from the
/// appender's perspective, so the same trait method serves the production
/// path once a concrete adapter is wired in.
#[derive(Debug)]
pub struct InMemoryAuditBroker {
 tx: tokio::sync::broadcast::Sender<AuditRecord>,
}

impl Default for InMemoryAuditBroker {
 fn default() -> Self {
 Self::new()
 }
}

impl InMemoryAuditBroker {
 pub fn new() -> Self {
 Self::with_capacity(1024)
 }

 pub fn with_capacity(capacity: usize) -> Self {
 let (tx, _) = tokio::sync::broadcast::channel(capacity);
 Self { tx }
 }

 /// Build a publisher that the writing server hands to its
 /// `AuditAppender`. Multiple publishers cloned from one broker share
 /// the same downstream subscribers.
 pub fn publisher(&self) -> InMemoryAuditPublisher {
 InMemoryAuditPublisher {
 tx: self.tx.clone(),
 }
 }

 /// Open a fresh receiver for an observing server. Records published
 /// on the broker arrive on this receiver in commit order; combine
 /// with [`relay_inbound_to_appender`] to bridge the broker stream
 /// into a remote `AuditAppender`'s local broadcast.
 pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<AuditRecord> {
 self.tx.subscribe()
 }
}

/// Adapter wrapping a broker [`tokio::sync::broadcast::Sender`] in the
/// trait surface. Cheap to clone — every clone is a handle to the same
/// underlying broker.
#[derive(Debug, Clone)]
pub struct InMemoryAuditPublisher {
 tx: tokio::sync::broadcast::Sender<AuditRecord>,
}

impl AuditFanout for InMemoryAuditPublisher {
 fn publish(&self, record: &AuditRecord) {
 let _ = self.tx.send(record.clone());
 }
}

/// Round 111 — relay loop pushing every record received on `inbound` into
/// `target`'s broadcast channel via [`AuditAppender::publish_external`].
/// Spawn as a dedicated tokio task; the loop exits when the broker
/// channel is closed or every relay sink is dropped.
pub async fn relay_inbound_to_appender(
 mut inbound: tokio::sync::broadcast::Receiver<AuditRecord>,
 target: Arc<AuditAppender>,
) {
 loop {
 match inbound.recv().await {
 Ok(record) => target.publish_external(record),
 Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
 // Lagged on the broker means we lost a window of records.
 // Skip and keep relaying — the local broadcast subscribers
 // observe the lag indirectly through their own Lagged
 // handling.
 Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
 }
 }
}

pub struct AuditAppender {
 store: Arc<MnemosyneStore>,
 next_transaction_id: AtomicU64,
 /// Round 103 — broadcast channel for live tail-following subscribers. Each
 /// successful RocksDB put pushes a clone of the record onto the channel;
 /// failure to send (no active subscribers) is silently ignored.
 broadcast_tx: tokio::sync::broadcast::Sender<AuditRecord>,
 /// Round 111 — outbound fanout for cross-process audit observers.
 /// Defaults to [`NoopAuditFanout`] for single-process deployments;
 /// [`AuditAppender::with_fanout`] swaps in a real implementation
 /// (in-memory broker, redis pub/sub, NATS, etc.).
 fanout: Arc<dyn AuditFanout>,
}

impl AuditAppender {
 /// Construct an audit appender, seeding `next_transaction_id` from the
 /// existing audit CF state. Phase 0: branch_id=0, entity_id=0 namespace
 /// for audit-internal counter — production allocator (server startup
 /// scan of `audit_cf.last_key + 1`) is a Phase 0+ wiring concern.
 pub fn new(store: Arc<MnemosyneStore>) -> Self {
 Self::with_broadcast_capacity(store, AUDIT_BROADCAST_CAPACITY)
 }

 /// Round 109 — construct with an explicit broadcast channel capacity.
 /// `capacity` is the maximum number of in-flight records the broadcast
 /// retains before the oldest unread record is overwritten and a slow
 /// subscriber sees `RecvError::Lagged` on its next `recv`. Production
 /// callers use [`Self::new`] which defaults to `AUDIT_BROADCAST_CAPACITY`;
 /// integration tests and sizing benchmarks pass an explicit capacity to
 /// exercise the lag path on a deterministic flood size.
 pub fn with_broadcast_capacity(store: Arc<MnemosyneStore>, capacity: usize) -> Self {
 Self::with_broadcast_capacity_and_fanout(store, capacity, Arc::new(NoopAuditFanout))
 }

 /// Round 111 — construct with a custom [`AuditFanout`]. Used by
 /// deployments that fan out audit observation to other servers
 /// (multi-host clusters) via an in-memory broker, redis pub/sub, or
 /// any other AuditFanout-compatible transport. Pass [`Arc::new(NoopAuditFanout)`]
 /// for single-process operation.
 pub fn with_broadcast_capacity_and_fanout(
 store: Arc<MnemosyneStore>,
 capacity: usize,
 fanout: Arc<dyn AuditFanout>,
 ) -> Self {
 let (broadcast_tx, _initial_rx) = tokio::sync::broadcast::channel(capacity);
 Self {
 store,
 next_transaction_id: AtomicU64::new(1),
 broadcast_tx,
 fanout,
 }
 }

 /// Round 103 — subscribe to newly-committed audit records. Returns a
 /// `Receiver` that observes every record written via [`Self::write`]
 /// after the call. Subscribers attached before any commits see all
 /// subsequent commits; lagging subscribers receive `RecvError::Lagged`
 /// when the broadcast channel overruns.
 pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<AuditRecord> {
 self.broadcast_tx.subscribe()
 }

 pub fn next_id(&self) -> u64 {
 self.next_transaction_id.fetch_add(1, Ordering::SeqCst)
 }

 pub fn append_accepted(
 &self,
 proposal: &Proposal,
 warnings: &[String],
 ) -> Result<u64> {
 self.append_accepted_with_trace_context(proposal, warnings, &TraceContext::default())
 }

 /// Round 99 + Round 104 — accepted-record append carrying an explicit
 /// trace context (trace_id + tracestate). Embedded callers without a
 /// context use [`Self::append_accepted`] which substitutes
 /// [`TraceContext::default`].
 pub fn append_accepted_with_trace_context(
 &self,
 proposal: &Proposal,
 warnings: &[String],
 ctx: &TraceContext,
 ) -> Result<u64> {
 let txn_id = self.next_id();
 let record = AuditRecord {
 transaction_id: txn_id,
 proposal_id: proposal.proposal_id.clone(),
 actor: proposal.actor.clone(),
 accepted: true,
 gate_routing_reason: if warnings.is_empty() {
  "accept".to_string()
 } else {
  "accept_with_warnings".to_string()
 },
 rejection_reason: None,
 proposal_kind_tag: kind_tag(&proposal.kind),
 trace_id: ctx.trace_id.clone(),
 tracestate: ctx.tracestate.clone(),
 };
 self.write(&record)?;
 Ok(txn_id)
 }

 pub fn append_rejected(
 &self,
 proposal: &Proposal,
 tier: GateTier,
 reason: &str,
 ) -> Result<u64> {
 self.append_rejected_with_trace_context(proposal, tier, reason, &TraceContext::default())
 }

 /// Round 99 + Round 104 — rejected-record append carrying an explicit
 /// trace context (trace_id + tracestate).
 pub fn append_rejected_with_trace_context(
 &self,
 proposal: &Proposal,
 tier: GateTier,
 reason: &str,
 ctx: &TraceContext,
 ) -> Result<u64> {
 let txn_id = self.next_id();
 let record = AuditRecord {
 transaction_id: txn_id,
 proposal_id: proposal.proposal_id.clone(),
 actor: proposal.actor.clone(),
 accepted: false,
 gate_routing_reason: match tier {
  GateTier::Tier1 => "t1_reject".to_string(),
  GateTier::Tier2 => "t2_reject".to_string(),
  GateTier::Tier3 => "t3_warn".to_string(),
 },
 rejection_reason: Some(reason.to_string()),
 proposal_kind_tag: kind_tag(&proposal.kind),
 trace_id: ctx.trace_id.clone(),
 tracestate: ctx.tracestate.clone(),
 };
 self.write(&record)?;
 Ok(txn_id)
 }

 fn write(&self, record: &AuditRecord) -> Result<()> {
 // Audit CF key: branch_id=0 || entity_id=0 || valid_from=transaction_id.
 // The transaction_id slot mirrors §4.378 monotonic key encoding.
 let payload = serde_json::to_vec(record).map_err(|e| {
 ServerError::AuditViolation(format!("audit serde: {e}"))
 })?;
 self.store
 .put(CfId::Audit, 0, 0, record.transaction_id, &payload)?;
 // Round 103 — push to the live tail-following broadcast channel after
 // the durable write. `send` returns `Err` only when no subscribers
 // are attached; that is the steady-state case for embedded callers
 // and is intentionally ignored.
 let _ = self.broadcast_tx.send(record.clone());
 // Round 111 — outbound fanout to cross-process subscribers (no-op
 // by default). Best-effort: a fanout backend failure must not
 // poison the local audit commit.
 self.fanout.publish(record);
 Ok(())
 }

 /// Round 111 — push a record received from an external fanout source
 /// (redis subscriber, in-memory broker relay, etc.) onto the local
 /// broadcast channel. Does NOT touch the local RocksDB audit CF —
 /// the originating server's append-only ledger remains the source of
 /// truth (DESIGN §6 spec stays frozen). Tail subscribers attached to
 /// this appender observes external records alongside locally-
 /// committed records.
 pub fn publish_external(&self, record: AuditRecord) {
 let _ = self.broadcast_tx.send(record);
 }

 /// Read an audit record by transaction id. Append-only — there is no
 /// `mutate` / `delete` operation by design.
 pub fn read(&self, transaction_id: u64) -> Result<Option<AuditRecord>> {
 let bytes = self.store.get(CfId::Audit, 0, 0, transaction_id)?;
 match bytes {
 None => Ok(None),
 Some(b) => {
  let r: AuditRecord = serde_json::from_slice(&b).map_err(|e| {
  ServerError::AuditViolation(format!("audit decode: {e}"))
  })?;
  Ok(Some(r))
 }
 }
 }

 /// Forward iterator over audit records with `transaction_id >= start`.
 /// Records are returned in monotonic transaction-id order, matching the
 /// audit CF key encoding (`branch_id=0, entity_id=0, valid_from=txn_id`).
 ///
 /// Round 98 — source for `SubscribeAuditTrail` streaming RPC. Phase 0
 /// implementation materializes the result eagerly; per-record streaming
 /// over the RocksDB iterator is exposed in Round 113 via
 /// [`Self::iter_from_streaming`].
 pub fn iter_from(&self, start: u64) -> Result<Vec<AuditRecord>> {
 let pairs = self.store.iter_branch_entity(CfId::Audit, 0, 0)?;
 let mut out = Vec::new();
 for (txn_id, bytes) in pairs {
 if txn_id < start {
  continue;
 }
 let r: AuditRecord = serde_json::from_slice(&bytes).map_err(|e| {
  ServerError::AuditViolation(format!("audit decode at txn {txn_id}: {e}"))
 })?;
 out.push(r);
 }
 Ok(out)
 }

 /// Round 113 — per-record streaming over the audit log. Hands every
 /// record with `transaction_id >= start` to `callback` as it is
 /// decoded from RocksDB; the callback returns `true` to continue or
 /// `false` to stop early. Memory profile: ~constant (one record at
 /// a time) regardless of audit-log size. Used by the gRPC
 /// `subscribe_audit_trail` history phase to drain large audit logs
 /// without materializing every record into a single Vec.
 ///
 /// Decode failures stop iteration with `Err(AuditViolation)` —
 /// caller-visible failure rather than silent skip, since a corrupted
 /// audit record on disk is a §6 invariant violation.
 pub fn iter_from_streaming<F>(&self, start: u64, mut callback: F) -> Result<()>
 where
 F: FnMut(AuditRecord) -> bool,
 {
 let mut decode_err: Option<ServerError> = None;
 self.store.iter_branch_entity_streaming(
 CfId::Audit,
 0,
 0,
 |txn_id, bytes| {
  if txn_id < start {
  return true;
  }
  match serde_json::from_slice::<AuditRecord>(&bytes) {
  Ok(r) => callback(r),
  Err(e) => {
  decode_err = Some(ServerError::AuditViolation(format!(
   "audit decode at txn {txn_id}: {e}"
  )));
  false
  }
  }
 },
 )?;
 if let Some(e) = decode_err {
 return Err(e);
 }
 Ok(())
 }

 /// Reject any attempt to overwrite an existing audit record. Audit
 /// append-only enforcement at the API level (storage layer alone cannot
 /// distinguish overwrite-with-same-key from new write — server enforces).
 pub fn check_no_overwrite(&self, transaction_id: u64) -> Result<()> {
 if self.store.get(CfId::Audit, 0, 0, transaction_id)?.is_some() {
 return Err(ServerError::AuditViolation(format!(
  "audit append-only: transaction_id {transaction_id} already recorded"
 )));
 }
 Ok(())
 }

 /// Encode transaction_id as the last 8 bytes of the composite key for
 /// inspection / debug dumps.
 pub fn encode_audit_key_suffix(transaction_id: u64) -> [u8; 8] {
 let mut buf = [0u8; 8];
 BigEndian::write_u64(&mut buf, transaction_id);
 buf
 }
}

fn kind_tag(kind: &crate::proposal::ProposalKind) -> String {
 match kind {
 crate::proposal::ProposalKind::EntityCreate { .. } => "entity_create".to_string(),
 crate::proposal::ProposalKind::EntityUpdate { .. } => "entity_update".to_string(),
 crate::proposal::ProposalKind::ChangelogAppend { .. } => "changelog_append".to_string(),
 crate::proposal::ProposalKind::CrossRefCreate { .. } => "cross_ref_create".to_string(),
 crate::proposal::ProposalKind::FrozenListMembershipChange { .. } => {
 "frozen_list_membership_change".to_string()
 }
 }
}

/// Adapter for `GateOutcome` → audit-append routing.
pub fn append_outcome(
 appender: &AuditAppender,
 proposal: &Proposal,
 outcome: &GateOutcome,
) -> Result<u64> {
 append_outcome_with_trace_context(appender, proposal, outcome, &TraceContext::default())
}

/// Round 99 + Round 104 — `append_outcome` variant carrying an explicit trace
/// context (trace_id + tracestate). Routes Accept/Reject outcomes to the
/// matching context-aware audit append.
pub fn append_outcome_with_trace_context(
 appender: &AuditAppender,
 proposal: &Proposal,
 outcome: &GateOutcome,
 ctx: &TraceContext,
) -> Result<u64> {
 match outcome {
 GateOutcome::Accept { warnings } => {
 appender.append_accepted_with_trace_context(proposal, warnings, ctx)
 }
 GateOutcome::Reject { tier, reason } => {
 appender.append_rejected_with_trace_context(proposal, *tier, reason, ctx)
 }
 }
}

#[cfg(test)]
mod tests {
 use super::*;
 use tempfile::TempDir;

 fn fresh() -> (TempDir, AuditAppender) {
 let dir = TempDir::new().unwrap();
 let store = Arc::new(MnemosyneStore::open(dir.path()).unwrap());
 let appender = AuditAppender::new(store);
 (dir, appender)
 }

 #[test]
 fn append_accepted_writes_record_with_monotonic_id() {
 let (_dir, appender) = fresh();
 let p = Proposal {
 proposal_id: "p1".to_string(),
 actor: "alice".to_string(),
 kind: crate::proposal::ProposalKind::EntityCreate {
  entity_type: "Section".to_string(),
  branch_id: 1,
  entity_id: 1,
  valid_from: 100,
  payload: vec![],
 },
 };
 let id1 = appender.append_accepted(&p, &[]).unwrap();
 let id2 = appender.append_accepted(&p, &[]).unwrap();
 assert_eq!(id2, id1 + 1);
 let recovered = appender.read(id1).unwrap().expect("audit record");
 assert!(recovered.accepted);
 assert_eq!(recovered.proposal_kind_tag, "entity_create");
 }

 #[test]
 fn append_rejected_writes_record_with_reason() {
 let (_dir, appender) = fresh();
 let p = Proposal {
 proposal_id: "p1".to_string(),
 actor: "alice".to_string(),
 kind: crate::proposal::ProposalKind::CrossRefCreate {
  branch_id: 1,
  from_section: 0,
  to_section: 39,
  ref_kind: "decision".to_string(),
 },
 };
 let id = appender
 .append_rejected(&p, GateTier::Tier1, "from_section unresolved")
 .unwrap();
 let recovered = appender.read(id).unwrap().expect("audit record");
 assert!(!recovered.accepted);
 assert_eq!(recovered.gate_routing_reason, "t1_reject");
 assert_eq!(
 recovered.rejection_reason.as_deref(),
 Some("from_section unresolved")
 );
 }

 #[test]
 fn iter_from_returns_records_in_monotonic_order_starting_at_cursor() {
 let (_dir, appender) = fresh();
 let mk = |id: u64| Proposal {
 proposal_id: format!("p{id}"),
 actor: "alice".to_string(),
 kind: crate::proposal::ProposalKind::EntityCreate {
  entity_type: "Section".to_string(),
  branch_id: 1,
  entity_id: id,
  valid_from: id * 100,
  payload: vec![],
 },
 };
 let txn1 = appender.append_accepted(&mk(1), &[]).unwrap();
 let txn2 = appender.append_accepted(&mk(2), &[]).unwrap();
 let txn3 = appender.append_accepted(&mk(3), &[]).unwrap();

 let all = appender.iter_from(0).unwrap();
 assert_eq!(all.len(), 3);
 assert_eq!(all[0].transaction_id, txn1);
 assert_eq!(all[1].transaction_id, txn2);
 assert_eq!(all[2].transaction_id, txn3);

 let from_two = appender.iter_from(txn2).unwrap();
 assert_eq!(from_two.len(), 2);
 assert_eq!(from_two[0].transaction_id, txn2);
 assert_eq!(from_two[1].transaction_id, txn3);

 let beyond = appender.iter_from(txn3 + 1).unwrap();
 assert!(beyond.is_empty());
 }

 #[tokio::test]
 async fn slow_subscriber_with_full_broadcast_returns_lagged() {
 // Round 109 — substantiate the broadcast Lagged path that the gRPC
 // resume_on_lag protocol surfaces: a Receiver that has not polled
 // while the Sender writes more than `capacity` records sees
 // RecvError::Lagged on its next recv(). The integration-level
 // assertion (gRPC stream emits Status::resource_exhausted with the
 // `lagged-at-txn` metadata cursor) is impractical to drive
 // organically through a real h2 + TCP transport because tonic's
 // send-side buffering hides backpressure; the unit test pins the
 // semantics our gRPC layer relies on.
 let dir = TempDir::new().unwrap();
 let store = Arc::new(MnemosyneStore::open(dir.path()).unwrap());
 let appender = AuditAppender::with_broadcast_capacity(store, 4);
 let mut rx = appender.subscribe();

 // Append 16 records — well past the capacity of 4. The broadcast
 // ring buffer retains only the latest 4; the receiver position is
 // still at 0, so the first recv must surface Lagged.
 for entity_id in 1..=16u64 {
 let p = Proposal {
  proposal_id: format!("p-lag-{entity_id}"),
  actor: "lag-tester".into(),
  kind: crate::proposal::ProposalKind::EntityCreate {
  entity_type: "Section".into(),
  branch_id: 1,
  entity_id,
  valid_from: entity_id * 100,
  payload: vec![],
  },
 };
 appender.append_accepted(&p, &[]).expect("append");
 }

 let first = rx.try_recv();
 match first {
 Err(tokio::sync::broadcast::error::TryRecvError::Lagged(missed)) => {
  assert!(
  missed >= 12,
  "Lagged report must reflect overwritten count; got {missed}"
  );
 }
 other => panic!(
  "first recv after slow-subscriber overflow must be Lagged, got {other:?}"
 ),
 }

 // After Lagged the receiver's position resets to the oldest
 // available record; the next recv yields the head of the surviving
 // window (txn 13..16 with capacity 4).
 let next = rx.try_recv().expect("recv after Lagged must surface a record");
 assert!(
 next.transaction_id >= 13 && next.transaction_id <= 16,
 "post-lagged recv must yield head of surviving window; got txn {}",
 next.transaction_id
 );
 }

 #[tokio::test]
 async fn subscribe_receives_newly_committed_records() {
 // Round 103 — broadcast channel correctness. A subscriber attached
 // before any commits observes every subsequent record; appends
 // before subscription are not replayed (snapshot semantics covered
 // separately by `iter_from`).
 let (_dir, appender) = fresh();
 let mut rx = appender.subscribe();

 let p = Proposal {
 proposal_id: "p-live".into(),
 actor: "alice".into(),
 kind: crate::proposal::ProposalKind::EntityCreate {
  entity_type: "Section".into(),
  branch_id: 1,
  entity_id: 42,
  valid_from: 1000,
  payload: vec![],
 },
 };
 let txn_id = appender.append_accepted(&p, &[]).unwrap();

 let received = tokio::time::timeout(
 std::time::Duration::from_millis(200),
 rx.recv(),
 )
 .await
 .expect("broadcast must deliver within timeout")
 .expect("broadcast send must succeed");

 assert_eq!(received.transaction_id, txn_id);
 assert_eq!(received.proposal_id, "p-live");
 assert!(received.accepted);
 }

 #[test]
 fn check_no_overwrite_rejects_duplicate_transaction_id() {
 let (_dir, appender) = fresh();
 let p = Proposal {
 proposal_id: "p1".to_string(),
 actor: "alice".to_string(),
 kind: crate::proposal::ProposalKind::EntityCreate {
  entity_type: "Section".to_string(),
  branch_id: 1,
  entity_id: 1,
  valid_from: 100,
  payload: vec![],
 },
 };
 let id = appender.append_accepted(&p, &[]).unwrap();
 let err = appender.check_no_overwrite(id).expect_err("duplicate");
 assert!(matches!(err, ServerError::AuditViolation(_)));
 }
}
