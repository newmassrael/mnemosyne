//! gRPC transport adapter (Round 91, hardened in Round 96).
//!
//! Wraps [`crate::handler::ProposalHandler`] in the tonic-generated `Mnemosyne`
//! service trait. The proto schema in `proto/mnemosyne.proto` is the wire
//! format; the embedded `Proposal` / `ProposalResult` types stay authoritative.
//!
//! Transport independence guarantee: a `Proposal` submitted via gRPC routes
//! through the same `ProposalHandler::handle` call as the embedded
//! [`crate::handler::MnemosyneServer::submit`] entry point. The `tests/grpc_smoke.rs`
//! integration test exercises both transports against fresh stores and asserts
//! the resulting `ProposalResult` values are equal — same gates, same audit
//! transaction id, same rejection reasoning.
//!
//! `ProposalHandler::handle` runs synchronous RocksDB writes, so the service
//! impl drives it under `tokio::task::spawn_blocking` to keep the tonic
//! async runtime non-blocking.
//!
//! ## Round 96 — server-side hardening
//!
//! - **Health-check** (`grpc.health.v1.Health`): standard
//! `tonic_health::server::health_reporter` — clients query `Check` and
//! receive `SERVING` once the Mnemosyne service is registered.
//! - **Reflection** (`grpc.reflection.v1alpha.ServerReflection`):
//! `tonic_reflection::server::Builder` registers the proto's
//! `FileDescriptorSet` (emitted by `build.rs`) so clients can list services
//! and descriptors at runtime without a static .proto.
//! - **Tracing interceptor**: a thin `tonic::service::Interceptor` records
//! the incoming method path on the current `tracing` span — wires gRPC
//! audit trail into the existing tracing fabric.
//! - **TLS toggle**: opt-in via the `tls` cargo feature (Round 97). When the
//! feature is on, `tls_identity_from_pem` and `server_tls_config` build a
//! `tonic::transport::ServerTlsConfig` from PEM-encoded cert/key bytes that
//! the caller passes to `Server::builder().tls_config(...)`. Default builds
//! stay TLS-free — no rustls compile, no runtime cost. Certificate generation for
//! tests is provided via `rcgen` as a dev-dependency; see
//! `tests/grpc_tls_smoke.rs` for the end-to-end pattern.

use crate::audit::{AuditRecord, TraceContext};
use crate::handler::ProposalHandler;
use crate::proposal::{Proposal, ProposalKind, ProposalResult};
use std::sync::Arc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::metadata::MetadataMap;
use tonic::{Request, Response, Status, Streaming};

pub mod proto {
 tonic::include_proto!("mnemosyne.v1");
}

pub use proto::mnemosyne_server::{Mnemosyne, MnemosyneServer};
pub use proto::mnemosyne_client::MnemosyneClient;

/// FileDescriptorSet for the `mnemosyne.v1` proto schema, emitted by
/// `build.rs` via `tonic_build::configure().file_descriptor_set_path(...)`.
/// Powers the gRPC reflection service.
pub const MNEMOSYNE_FILE_DESCRIPTOR_SET: &[u8] =
 include_bytes!(concat!(env!("OUT_DIR"), "/mnemosyne_descriptor.bin"));

/// gRPC adapter binding the embedded `ProposalHandler` to the `Mnemosyne`
/// proto service.
#[derive(Clone)]
pub struct MnemosyneGrpcService {
 handler: Arc<ProposalHandler>,
}

impl MnemosyneGrpcService {
 pub fn new(handler: Arc<ProposalHandler>) -> Self {
 Self { handler }
 }

 pub fn into_server(self) -> MnemosyneServer<Self> {
 MnemosyneServer::new(self)
 }
}

/// Channel buffer for streaming RPC outbound queues. 16 keeps backpressure
/// reactive while absorbing brief bursts; client disconnect closes the
/// receiver half and the server task exits the next send attempt.
const STREAM_CHANNEL_BUFFER: usize = 16;

/// Round 99 — extract the W3C trace-id portion of a `traceparent` header.
///
/// Format: `00-{32-hex trace-id}-{16-hex parent-id}-{2-hex flags}`. Returns
/// `None` if the header is missing, malformed, or has a trace-id that does
/// not satisfy the 32-hex-digit shape. Strict parsing — invalid headers fall
/// back to server-generated trace_ids rather than corrupting the audit trail.
fn extract_traceparent_trace_id(metadata: &MetadataMap) -> Option<String> {
 let raw = metadata.get("traceparent")?.to_str().ok()?;
 let mut parts = raw.split('-');
 let _version = parts.next()?;
 let trace_id = parts.next()?;
 if trace_id.len() == 32 && trace_id.chars().all(|c| c.is_ascii_hexdigit()) {
 Some(trace_id.to_string())
 } else {
 None
 }
}

/// Round 99 — resolve a trace_id from incoming metadata. Honors a valid W3C
/// `traceparent` header if present; otherwise mints a fresh UUID v4 so every
/// audited proposal carries a non-`None` trace_id when crossing the gRPC
/// boundary. Embedded callers without metadata stay on the `handle` path
/// (trace_id `None`).
fn resolve_trace_id(metadata: &MetadataMap) -> String {
 extract_traceparent_trace_id(metadata).unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
}

/// Round 104 — extract the W3C `tracestate` header verbatim. Format
/// (RFC W3C Trace Context §3.3): a comma-separated list of `key=value`
/// vendor entries, each up to 256 chars. We do *not* parse or normalize the
/// list — observability tooling on the consuming side relies on exact
/// passthrough so vendor-specific information survives the gRPC boundary
/// unchanged. Returns `None` when the header is absent or non-ASCII.
fn extract_tracestate(metadata: &MetadataMap) -> Option<String> {
 metadata.get("tracestate")?.to_str().ok().map(str::to_string)
}

/// Round 104 — resolve the full [`TraceContext`] (trace_id + tracestate) from
/// incoming metadata. trace_id follows the Round 99 traceparent + UUID-fallback
/// path; tracestate is a strict passthrough that stays `None` when absent
/// (no fallback — there is no server-minted equivalent of vendor state).
fn resolve_trace_context(metadata: &MetadataMap) -> TraceContext {
 TraceContext {
 trace_id: Some(resolve_trace_id(metadata)),
 tracestate: extract_tracestate(metadata),
 }
}

#[tonic::async_trait]
impl Mnemosyne for MnemosyneGrpcService {
 async fn submit_proposal(
 &self,
 request: Request<proto::Proposal>,
 ) -> Result<Response<proto::ProposalResult>, Status> {
 // Round 105 — create an explicit entry-level RPC span. Tonic does NOT
 // auto-instrument requests with a tracing span, so prior calls to
 // `Span::current().record(...)` (Round 96/99/102) were silent no-ops
 // when no caller-supplied parent span existed. Wrapping the body in
 // an explicit `submit_proposal` span makes the recorded fields
 // (grpc.method, trace_id, trace.id, tracestate) actually surface in
 // the OTLP exporter and in any tracing subscriber. Field declarations
 // use `Empty` so the recorded values populate slots reserved at span
 // creation rather than being dropped (`record` only mutates declared
 // fields).
 let span = tracing::info_span!(
 "submit_proposal",
 grpc.method = tracing::field::Empty,
 trace_id = tracing::field::Empty,
 "trace.id" = tracing::field::Empty,
 tracestate = tracing::field::Empty,
 );
 let _enter = span.enter();

 // Round 96 — record the gRPC method on the entry span.
 tracing::Span::current().record("grpc.method", "submit_proposal");

 // Round 99 — propagate W3C traceparent into the audit trail, or mint a
 // server-side UUID when no header was supplied. Round 102 — also record
 // the trace_id under OTLP-compatible span attribute keys (`trace.id`)
 // so the OpenTelemetry exporter (when the `otlp` feature is enabled)
 // surfaces it in the standard semantic-convention shape.
 // Round 104 — also propagate the W3C `tracestate` header verbatim into
 // the audit trail (vendor-specific key/value passthrough).
 let trace_ctx = resolve_trace_context(request.metadata());
 if let Some(id) = trace_ctx.trace_id.as_deref() {
 tracing::Span::current().record("trace_id", id);
 tracing::Span::current().record("trace.id", id);
 }
 if let Some(ts) = trace_ctx.tracestate.as_deref() {
 tracing::Span::current().record("tracestate", ts);
 }

 let wire = request.into_inner();
 let proposal = decode_proposal(wire)
 .map_err(|reason| Status::invalid_argument(reason))?;
 let handler = Arc::clone(&self.handler);
 // Round 104 — capture the entry-level RPC span and re-enter it inside
 // the blocking task so the gate.evaluate / audit.append child spans
 // emitted by the handler are parented correctly. spawn_blocking does
 // not propagate tracing's thread-local context on its own.
 let parent_span = tracing::Span::current();
 let ctx_for_handler = trace_ctx.clone();
 let result = tokio::task::spawn_blocking(move || {
 let _enter = parent_span.enter();
 handler.handle_with_trace_context(&proposal, &ctx_for_handler)
 })
 .await
 .map_err(|join_err| Status::internal(format!("handler join error: {join_err}")))?
 .map_err(|server_err| Status::internal(format!("handler error: {server_err}")))?;
 Ok(Response::new(encode_result(result)))
 }

 type SubmitProposalBatchStream = ReceiverStream<Result<proto::ProposalResult, Status>>;

 /// Round 98 — client-streaming → server-streaming batch ingest. Each inbound
 /// `Proposal` flows through the same `ProposalHandler::handle` path as
 /// the unary RPC; results stream back in arrival order. Per-proposal
 /// errors decode into per-result `Status` items so a malformed proposal
 /// in the middle of a batch does not tear down the whole stream.
 async fn submit_proposal_batch(
 &self,
 request: Request<Streaming<proto::Proposal>>,
 ) -> Result<Response<Self::SubmitProposalBatchStream>, Status> {
 // Round 105 — explicit entry-level RPC span (see submit_proposal for
 // rationale). Empty-field declaration so subsequent record() calls
 // mutate declared slots rather than being dropped.
 let span = tracing::info_span!(
 "submit_proposal_batch",
 grpc.method = tracing::field::Empty,
 trace_id = tracing::field::Empty,
 "trace.id" = tracing::field::Empty,
 tracestate = tracing::field::Empty,
 );
 let _enter = span.enter();

 tracing::Span::current().record("grpc.method", "submit_proposal_batch");

 // Round 99 — single trace context pinned to the whole batch RPC.
 // Every proposal in the inbound stream lands in the audit trail with
 // the same trace_id (and tracestate, Round 104), so observability
 // tools can group batch submissions and preserve vendor context.
 // Round 102 — also record under the OTLP-compatible `trace.id` key.
 let batch_ctx = resolve_trace_context(request.metadata());
 if let Some(id) = batch_ctx.trace_id.as_deref() {
 tracing::Span::current().record("trace_id", id);
 tracing::Span::current().record("trace.id", id);
 }
 if let Some(ts) = batch_ctx.tracestate.as_deref() {
 tracing::Span::current().record("tracestate", ts);
 }

 // Round 112 — opt-in atomic batch mode. Metadata key
 // `x-mnemosyne-batch-atomic = "true"` switches the batch from
 // per-proposal incremental commits (Round 98 default) to a single
 // all-or-nothing transactional commit. Any value other than
 // exactly `"true"` (case-sensitive) selects the default mode.
 let atomic_batch = request
 .metadata()
 .get("x-mnemosyne-batch-atomic")
 .and_then(|v| v.to_str().ok())
 .map(|s| s == "true")
 .unwrap_or(false);

 let mut inbound = request.into_inner();
 let handler = Arc::clone(&self.handler);
 let (tx, rx) = tokio::sync::mpsc::channel(STREAM_CHANNEL_BUFFER);
 // Round 104 — capture the parent batch span for propagation into each
 // per-proposal blocking task; child spans (gate.evaluate /
 // audit.append) thus parent correctly to the batch entry span.
 let parent_span = tracing::Span::current();

 if atomic_batch {
 let ctx_for_atomic = batch_ctx.clone();
 tokio::spawn(async move {
  let mut proposals: Vec<Proposal> = Vec::new();
  loop {
  let next = inbound.message().await;
  let wire = match next {
  Ok(Some(w)) => w,
  Ok(None) => break,
  Err(s) => {
   let _ = tx.send(Err(s)).await;
   return;
  }
  };
  match decode_proposal(wire) {
  Ok(p) => proposals.push(p),
  Err(reason) => {
   // Even one decode failure poisons the whole atomic batch:
   // emit a single invalid_argument and stop. The batch is
   // not committed, no per-proposal results emitted.
   let _ = tx
   .send(Err(Status::invalid_argument(format!(
   "atomic batch decode failure: {reason}"
   ))))
   .await;
   return;
  }
  }
  }
  let parent_span_clone = parent_span.clone();
  let h = Arc::clone(&handler);
  let join = tokio::task::spawn_blocking(move || {
  let _enter = parent_span_clone.enter();
  h.handle_batch_atomic(&proposals, &ctx_for_atomic)
  })
  .await;
  match join {
  Err(je) => {
  let _ = tx
   .send(Err(Status::internal(format!("atomic join error: {je}"))))
   .await;
  }
  Ok(Err(se)) => {
  let _ = tx
   .send(Err(Status::internal(format!("atomic handler error: {se}"))))
   .await;
  }
  Ok(Ok(results)) => {
  for r in results {
   if tx.send(Ok(encode_result(r))).await.is_err() {
   return;
   }
  }
  }
  }
 });
 return Ok(Response::new(ReceiverStream::new(rx)));
 }

 tokio::spawn(async move {
 loop {
  let next = inbound.message().await;
  let wire = match next {
  Ok(Some(w)) => w,
  Ok(None) => break,
  Err(s) => {
  let _ = tx.send(Err(s)).await;
  return;
  }
  };
  let proposal = match decode_proposal(wire) {
  Ok(p) => p,
  Err(reason) => {
  if tx.send(Err(Status::invalid_argument(reason))).await.is_err() {
   return;
  }
  continue;
  }
  };
  let h = Arc::clone(&handler);
  let ctx_for_handler = batch_ctx.clone();
  let parent_span_clone = parent_span.clone();
  let result = tokio::task::spawn_blocking(move || {
  let _enter = parent_span_clone.enter();
  h.handle_with_trace_context(&proposal, &ctx_for_handler)
  })
  .await;
  let item = match result {
  Err(join_err) => {
  Err(Status::internal(format!("handler join error: {join_err}")))
  }
  Ok(Err(server_err)) => {
  Err(Status::internal(format!("handler error: {server_err}")))
  }
  Ok(Ok(r)) => Ok(encode_result(r)),
  };
  if tx.send(item).await.is_err() {
  return;
  }
 }
 });

 Ok(Response::new(ReceiverStream::new(rx)))
 }

 type SubscribeAuditTrailStream = ReceiverStream<Result<proto::AuditRecord, Status>>;

 /// Round 98 — server-streaming audit subscription. The audit appender's
 /// `iter_from` materializes the records visible at scan time; the stream
 /// drains them honoring `max_records` (0 = unbounded).
 ///
 /// Round 103 — when `follow_tail` is true, after the historical drain the
 /// server attaches to the audit broadcast channel and forwards
 /// newly-committed records in real time. `max_records` (when non-zero)
 /// caps the *combined* history + tail emission count; 0 means unbounded
 /// across both phases. Tail-follow stops on client disconnect (the mpsc
 /// `tx.send` returns Err) or on broadcast lag (Lagged error closes the
 /// stream rather than corrupting ordering).
 async fn subscribe_audit_trail(
 &self,
 request: Request<proto::SubscribeAuditRequest>,
 ) -> Result<Response<Self::SubscribeAuditTrailStream>, Status> {
 // Round 105 — explicit entry-level RPC span (see submit_proposal).
 let span = tracing::info_span!(
 "subscribe_audit_trail",
 grpc.method = tracing::field::Empty,
 );
 let _enter = span.enter();
 tracing::Span::current().record("grpc.method", "subscribe_audit_trail");

 let req = request.into_inner();
 let handler = Arc::clone(&self.handler);
 let (tx, rx) = tokio::sync::mpsc::channel(STREAM_CHANNEL_BUFFER);

 // Round 103 — attach the tail subscriber BEFORE the historical scan so
 // any record committed during the scan window is captured by the
 // broadcast receiver instead of lost between the snapshot cursor and
 // the tail handoff.
 let tail_rx = if req.follow_tail {
 Some(handler.audit().subscribe())
 } else {
 None
 };

 let resume_on_lag = req.resume_on_lag;
 // Round 110 — capture per-subscriber filters once. Empty lists (the
 // default for both fields) short-circuit predicate evaluation in
 // the inner loop so the filtered code path costs ~0 when no
 // filtering is requested.
 let kind_filter: std::collections::HashSet<String> =
 req.proposal_kind_filter.iter().cloned().collect();
 let actor_filter: std::collections::HashSet<String> =
 req.actor_filter.iter().cloned().collect();
 let matches_filters = std::sync::Arc::new(
 move |record: &AuditRecord| -> bool {
  if !kind_filter.is_empty() && !kind_filter.contains(&record.proposal_kind_tag) {
  return false;
  }
  if !actor_filter.is_empty() && !actor_filter.contains(&record.actor) {
  return false;
  }
  true
 },
 );
 tokio::spawn(async move {
 let from_txn = req.from_transaction_id;
 let max_records = req.max_records;
 let history_cap = if max_records == 0 {
  usize::MAX
 } else {
  max_records as usize
 };

 // Round 113 — per-record streaming history drain. The blocking
 // task pumps records into `tx` via `blocking_send`, so the
 // RocksDB iterator never materializes the full audit log
 // into memory. Returns the count actually emitted plus the
 // last transaction_id so the tail-follow phase can pick up
 // the cursor.
 let scan_handler = Arc::clone(&handler);
 let tx_for_history = tx.clone();
 let filter_for_history = Arc::clone(&matches_filters);
 let history = tokio::task::spawn_blocking(move || -> std::result::Result<(usize, u64), String> {
  let mut emitted_local: usize = 0;
  let mut last_emitted_local: u64 = 0;
  let mut send_failed = false;
  let scan_result = scan_handler.audit().iter_from_streaming(from_txn, |record| {
  if emitted_local >= history_cap {
  return false;
  }
  if !filter_for_history(&record) {
  return true;
  }
  let txn = record.transaction_id;
  let wire = encode_audit_record(&record);
  if tx_for_history.blocking_send(Ok(wire)).is_err() {
  send_failed = true;
  return false;
  }
  emitted_local += 1;
  last_emitted_local = txn;
  true
  });
  if let Err(e) = scan_result {
  return Err(format!("audit iter error: {e}"));
  }
  let _ = send_failed; // exit-early signal; nothing else to do
  Ok((emitted_local, last_emitted_local))
 })
 .await;

 let (mut emitted, mut last_emitted_txn) = match history {
  Err(join_err) => {
  let _ = tx
  .send(Err(Status::internal(format!("audit join error: {join_err}"))))
  .await;
  return;
  }
  Ok(Err(msg)) => {
  let _ = tx.send(Err(Status::internal(msg))).await;
  return;
  }
  Ok(Ok(pair)) => pair,
 };

 // Round 103 — tail-follow phase. `emitted` already counts history;
 // continue forwarding until the cap fires, the client closes, or
 // the broadcast lags. The `select!` between `tail.recv()` and
 // `tx.closed()` is essential — without the second branch the
 // loop would block on `tail.recv()` even after the client
 // disconnects, hanging the spawned task indefinitely.
 if let Some(mut tail) = tail_rx {
  let max_total = max_records as usize;
  loop {
  if max_records != 0 && emitted >= max_total {
  return;
  }
  let next = tokio::select! {
  biased;
  _ = tx.closed() => return,
  r = tail.recv() => r,
  };
  match next {
  Ok(record) => {
   // Skip records the historical scan already
   // emitted. Records committed during the scan
   // window may surface in both phases — bias the
   // wire output toward strict monotonicity.
   if record.transaction_id < from_txn {
   continue;
   }
   // Round 110 — apply the per-subscriber filter on
   // the tail phase too, so filtered subscribers
   // never see non-matching records anywhere.
   if !matches_filters(&record) {
   continue;
   }
   let txn = record.transaction_id;
   if tx
   .send(Ok(encode_audit_record(&record)))
   .await
   .is_err()
   {
   return;
   }
   emitted += 1;
   last_emitted_txn = txn;
  }
  Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
  Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
   // Round 109 — branch on resume_on_lag.
   // true → resource_exhausted with `lagged-at-txn`
   // metadata for client-side cursor retry.
   // false → Round 103 graceful close (data_loss).
   let status = if resume_on_lag {
   let mut s = Status::resource_exhausted(format!(
   "audit broadcast lagged — last_emitted_txn={last_emitted_txn}, retry with from_transaction_id={}",
   last_emitted_txn.saturating_add(1)
   ));
   if let Ok(value) = last_emitted_txn.to_string().parse() {
   s.metadata_mut().insert("lagged-at-txn", value);
   }
   s
   } else {
   Status::data_loss(
   "audit broadcast lagged — subscriber must resubscribe",
   )
   };
   let _ = tx.send(Err(status)).await;
   return;
  }
  }
  }
 }
 });

 Ok(Response::new(ReceiverStream::new(rx)))
 }
}

/// Round 108 — client-side helper that builds a tonic [`Channel`] which
/// load-balances inbound RPCs across a fixed list of server endpoints. Each
/// endpoint string must be a fully-qualified URL (e.g. `http://host:port`).
/// The returned channel uses tonic's built-in round-robin discovery — every
/// RPC picks the next healthy endpoint in rotation.
///
/// `balance_list` is a static-list shape (no DNS-resolved discovery, no
/// runtime endpoint addition). For dynamic discovery the
/// `Channel::balance_channel(capacity)` builder paired with
/// `Endpoint`-emitting events is the proper path; this helper covers the
/// common case where endpoints come from configuration at startup.
///
/// Returns `Err` only when *all* supplied endpoint strings fail to parse.
/// A subset of malformed entries is logged and dropped — surviving
/// endpoints still load-balance correctly.
pub fn balanced_channel(
 endpoints: impl IntoIterator<Item = String>,
) -> Result<tonic::transport::Channel, String> {
 let mut parsed: Vec<tonic::transport::Endpoint> = Vec::new();
 for url in endpoints {
 match tonic::transport::Endpoint::from_shared(url.clone()) {
 Ok(ep) => parsed.push(ep),
 Err(e) => tracing::warn!(endpoint = %url, error = %e, "skipping malformed lb endpoint"),
 }
 }
 if parsed.is_empty() {
 return Err("balanced_channel: no valid endpoints supplied".to_string());
 }
 Ok(tonic::transport::Channel::balance_list(parsed.into_iter()))
}

/// Round 108 — server-side authentication interceptor. Rejects any inbound
/// RPC whose metadata lacks an `authorization` header with `Status::
/// unauthenticated`. Compose via
/// `Server::builder().add_service(InterceptedService::new(svc, require_authorization_metadata))`
/// or the lower-level `tonic::service::interceptor`. The interceptor does
/// not validate the *value* of the header — Phase 0+ token-verification
/// (JWT signature, mTLS subject DN binding) layers on top.
pub fn require_authorization_metadata(req: Request<()>) -> Result<Request<()>, Status> {
 if req.metadata().get("authorization").is_none() {
 return Err(Status::unauthenticated(
 "missing required `authorization` metadata",
 ));
 }
 Ok(req)
}

/// Tracing interceptor — records the gRPC method path on the current span.
/// Returns the request unchanged; failure cases at the gRPC layer (e.g.
/// missing metadata) do not gate the request.
///
/// Compose via `tonic::service::interceptor(svc, with_tracing_span)` when
/// wiring a server. The Mnemosyne `submit_proposal` body also annotates the
/// span explicitly — this interceptor is the generic catch-all path for
/// future RPC additions.
pub fn with_tracing_span(req: Request<()>) -> Result<Request<()>, Status> {
 if let Some(path) = req.metadata().get("grpc-method") {
 if let Ok(value) = path.to_str() {
 tracing::Span::current().record("grpc.method", value);
 }
 }
 Ok(req)
}

/// Build a tonic-health reporter pre-registered as `SERVING` for the
/// Mnemosyne service. Returns the `(reporter, health_service)` pair so
/// callers can update health status dynamically (e.g. mark `NOT_SERVING`
/// during graceful shutdown) and add the health service to the server.
///
/// Round 96 — standard `grpc.health.v1.Health` protocol. The health-service
/// type is opaque because tonic-health 0.12 returns
/// `HealthServer<impl Health>` and the inner type is not part of the public
/// API.
pub async fn build_health_service() -> (
 tonic_health::server::HealthReporter,
 tonic_health::pb::health_server::HealthServer<impl tonic_health::pb::health_server::Health>,
) {
 let (mut reporter, health_service) = tonic_health::server::health_reporter();
 reporter
 .set_serving::<MnemosyneServer<MnemosyneGrpcService>>()
 .await;
 (reporter, health_service)
}

/// Build a gRPC reflection service that serves the `mnemosyne.v1` proto
/// schema at runtime. Clients can use grpcurl-style tooling to enumerate
/// services and methods without a local .proto.
///
/// Round 96 — `grpc.reflection.v1alpha.ServerReflection` protocol.
pub fn build_reflection_service(
) -> Result<tonic_reflection::server::v1alpha::ServerReflectionServer<impl tonic_reflection::server::v1alpha::ServerReflection>, tonic_reflection::server::Error> {
 tonic_reflection::server::Builder::configure()
 .register_encoded_file_descriptor_set(MNEMOSYNE_FILE_DESCRIPTOR_SET)
 .build_v1alpha()
}

/// Build a `tonic::transport::Identity` from PEM-encoded cert and key bytes.
/// Thin wrapper that surfaces tonic's TLS construction without forcing
/// callers to depend on tonic directly.
///
/// Round 97 — gated behind the `tls` feature; default builds carry no rustls.
#[cfg(feature = "tls")]
pub fn tls_identity_from_pem(
 cert_pem: impl AsRef<[u8]>,
 key_pem: impl AsRef<[u8]>,
) -> tonic::transport::Identity {
 tonic::transport::Identity::from_pem(cert_pem, key_pem)
}

/// Build a `tonic::transport::ServerTlsConfig` from a server identity.
/// Pass the result to `Server::builder().tls_config(config)?`.
///
/// Round 97 — gated behind the `tls` feature; pairs with `tls_identity_from_pem`.
#[cfg(feature = "tls")]
pub fn server_tls_config(identity: tonic::transport::Identity) -> tonic::transport::ServerTlsConfig {
 tonic::transport::ServerTlsConfig::new().identity(identity)
}

/// Round 107 — mTLS variant: server identity *plus* a client CA root for
/// peer verification. Tonic's `ServerTlsConfig::client_ca_root` switches the
/// underlying rustls config from `with_no_client_auth()` to
/// `with_client_cert_verifier(...)`, so any inbound connection without a
/// client cert chained to `client_ca_pem` fails the TLS handshake.
///
/// `client_ca_pem` is the PEM-encoded CA used to validate inbound client
/// certs — typically the same self-signed CA whose private key signed each
/// permitted client identity.
#[cfg(feature = "tls")]
pub fn server_tls_config_mtls(
 identity: tonic::transport::Identity,
 client_ca_pem: impl AsRef<[u8]>,
) -> tonic::transport::ServerTlsConfig {
 let ca = tonic::transport::Certificate::from_pem(client_ca_pem);
 tonic::transport::ServerTlsConfig::new()
 .identity(identity)
 .client_ca_root(ca)
}

/// Install the ring-backed default `CryptoProvider` for rustls 0.23. Idempotent:
/// repeated calls are no-ops once a provider is already set (any caller path —
/// server or client — installs once for the whole process). Must be called
/// before the first `tls_config(...)` build, otherwise rustls panics with
/// "no process-level CryptoProvider available".
///
/// Round 97 — gated behind the `tls` feature. The helper is idempotent so both
/// server bring-up and client bring-up paths can call it independently without
/// risk of double-install.
#[cfg(feature = "tls")]
pub fn install_default_crypto_provider() {
 let _ = rustls::crypto::ring::default_provider().install_default();
}

// --- Round 107 — dynamic TLS cert rotation -----------------------------------

/// Round 107 — build a `rustls::ServerConfig` from PEM cert/key bytes, with
/// optional mTLS client cert verification. Returns the rustls config rather
/// than tonic's `ServerTlsConfig` because the rotation path bypasses tonic's
/// `tls_config(...)` (which is static after Server::builder build) and feeds
/// rustls connections directly into `serve_with_incoming` via
/// [`spawn_rotating_tls_acceptor`].
///
/// `client_ca_pem` enables mTLS when supplied; `None` produces a one-way
/// server-auth config.
#[cfg(feature = "tls")]
pub fn build_rustls_server_config(
 cert_pem: &[u8],
 key_pem: &[u8],
 client_ca_pem: Option<&[u8]>,
) -> Result<std::sync::Arc<rustls::ServerConfig>, String> {
 use std::io::BufReader;
 use std::sync::Arc;

 let certs: Vec<rustls::pki_types::CertificateDer<'static>> =
 rustls_pemfile::certs(&mut BufReader::new(cert_pem))
 .collect::<Result<Vec<_>, _>>()
 .map_err(|e| format!("cert pem parse: {e}"))?;
 if certs.is_empty() {
 return Err("cert pem yielded zero certificates".to_string());
 }
 let key = rustls_pemfile::private_key(&mut BufReader::new(key_pem))
 .map_err(|e| format!("key pem parse: {e}"))?
 .ok_or_else(|| "key pem yielded zero keys".to_string())?;

 let builder = rustls::ServerConfig::builder();
 let mut cfg = if let Some(ca) = client_ca_pem {
 let ca_certs: Vec<rustls::pki_types::CertificateDer<'static>> =
 rustls_pemfile::certs(&mut BufReader::new(ca))
  .collect::<Result<Vec<_>, _>>()
  .map_err(|e| format!("client ca pem parse: {e}"))?;
 let mut roots = rustls::RootCertStore::empty();
 for c in ca_certs {
 roots
  .add(c)
  .map_err(|e| format!("client ca add: {e}"))?;
 }
 let verifier =
 rustls::server::WebPkiClientVerifier::builder(Arc::new(roots))
  .build()
  .map_err(|e| format!("client cert verifier build: {e}"))?;
 builder
 .with_client_cert_verifier(verifier)
 .with_single_cert(certs, key)
 .map_err(|e| format!("rustls config build: {e}"))?
 } else {
 builder
 .with_no_client_auth()
 .with_single_cert(certs, key)
 .map_err(|e| format!("rustls config build: {e}"))?
 };
 // gRPC requires HTTP/2; advertise it via ALPN so the client side
 // negotiates h2 instead of falling back to the default rustls ordering
 // (which omits h2 entirely and produces `H2NotNegotiated` against tonic).
 cfg.alpn_protocols = vec![b"h2".to_vec()];
 Ok(Arc::new(cfg))
}

/// Round 107 — handle to a watch channel carrying the *current* rustls
/// `ServerConfig`. New TLS handshakes performed by
/// [`spawn_rotating_tls_acceptor`] read this watch every accept, so a
/// successful [`Self::rotate`] takes effect on the *next* connection without
/// tearing down the listener or in-flight RPCs.
///
/// `Send + Sync + Clone`: a single rotator can be cloned across tasks (admin
/// reload endpoint, file watcher, signal handler).
#[cfg(feature = "tls")]
#[derive(Clone)]
pub struct TlsIdentityRotator {
 sender: tokio::sync::watch::Sender<std::sync::Arc<rustls::ServerConfig>>,
}

#[cfg(feature = "tls")]
impl TlsIdentityRotator {
 /// Build a new rotator paired with a [`TlsIdentityHandle`]. The handle
 /// is consumed by the acceptor stream; the rotator stays in caller
 /// hands for `rotate()` calls.
 pub fn new(
 initial: std::sync::Arc<rustls::ServerConfig>,
 ) -> (Self, TlsIdentityHandle) {
 let (sender, receiver) = tokio::sync::watch::channel(initial);
 (Self { sender }, TlsIdentityHandle { receiver })
 }

 /// Swap in a new `ServerConfig`. Subsequent inbound connections accepted
 /// by [`spawn_rotating_tls_acceptor`] negotiate TLS using `new_config`.
 /// Returns `Err` only when the receiver half has been dropped (no
 /// acceptor task is consuming the watch).
 pub fn rotate(
 &self,
 new_config: std::sync::Arc<rustls::ServerConfig>,
 ) -> Result<(), String> {
 self.sender
 .send(new_config)
 .map_err(|e| format!("rotation channel closed: {e}"))
 }
}

/// Round 107 — handle consumed by the rotating TLS acceptor. Wraps the
/// receiver half of the rotation watch.
#[cfg(feature = "tls")]
pub struct TlsIdentityHandle {
 receiver: tokio::sync::watch::Receiver<std::sync::Arc<rustls::ServerConfig>>,
}

#[cfg(feature = "tls")]
impl TlsIdentityHandle {
 /// Snapshot the current `ServerConfig`. Used by the acceptor loop on
 /// every inbound connection.
 pub fn current(&self) -> std::sync::Arc<rustls::ServerConfig> {
 self.receiver.borrow().clone()
 }
}

/// Round 107 — spawn a TCP accept loop that wraps each accepted stream with a
/// fresh `tokio_rustls::TlsAcceptor` built from the *current* `ServerConfig`
/// in the rotation watch. Yields `Result<TlsStream<TcpStream>, std::io::Error>`
/// items into a `tokio_stream::wrappers::UnboundedReceiverStream` shaped for
/// `Server::builder().serve_with_incoming(...)`.
///
/// Why an explicit accept loop: tonic's `Server::tls_config(...)` consumes a
/// static `ServerTlsConfig` baked into the listener's accept loop — there is
/// no ergonomic hot-swap. By feeding tonic pre-handshaked TLS streams we keep
/// one-shot identity rotation as a first-class capability.
#[cfg(feature = "tls")]
pub fn spawn_rotating_tls_acceptor(
 listener: tokio::net::TcpListener,
 handle: TlsIdentityHandle,
) -> tokio_stream::wrappers::UnboundedReceiverStream<
 Result<
 tokio_rustls::server::TlsStream<tokio::net::TcpStream>,
 std::io::Error,
 >,
> {
 let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
 tokio::spawn(async move {
 loop {
 let (tcp, _addr) = match listener.accept().await {
  Ok(v) => v,
  Err(e) => {
  let _ = tx.send(Err(e));
  break;
  }
 };
 let cfg = handle.current();
 let acceptor = tokio_rustls::TlsAcceptor::from(cfg);
 let tx2 = tx.clone();
 // Each handshake runs in its own task so a slow / failing
 // handshake does not stall subsequent inbound TCP accepts.
 tokio::spawn(async move {
  match acceptor.accept(tcp).await {
  Ok(stream) => {
  let _ = tx2.send(Ok(stream));
  }
  Err(e) => {
  // Surface handshake failures to the consuming
  // server only as logged events — tonic treats
  // a single Err item as fatal for the whole
  // listener stream, so we drop the failure.
  tracing::debug!(error = %e, "rotating tls handshake failed");
  }
  }
 });
 }
 });
 tokio_stream::wrappers::UnboundedReceiverStream::new(rx)
}

/// Round 106 — value-typed configuration for [`init_otlp_tracing_subscriber_with_config`].
/// Carries the OTLP endpoint plus tunables for the batch span processor
/// (`max_export_batch_size` / `scheduled_delay`), the sampling decision
/// (`sampling_rate`, mapped to `Sampler::TraceIdRatioBased`), and resource
/// attributes injected onto every emitted span (`service.name`,
/// `deployment.environment`, etc.). Builder methods give a fluent surface
/// without breaking the existing `init_otlp_tracing_subscriber(endpoint)`
/// entry point — that helper now constructs `OtlpExporterConfig::new(endpoint)`
/// internally.
#[cfg(feature = "otlp")]
#[derive(Debug, Clone)]
pub struct OtlpExporterConfig {
 /// OTLP gRPC collector endpoint (`http://host:port`).
 pub endpoint: String,
 /// Maximum spans per batch handed to the exporter. SDK default 512.
 pub batch_max_export_batch_size: usize,
 /// Delay between scheduled batch flushes. SDK default 5s.
 pub batch_scheduled_delay: std::time::Duration,
 /// Sampling rate in [0.0, 1.0]. 1.0 emits every span; 0.0 drops all.
 /// Mapped to `opentelemetry_sdk::trace::Sampler::TraceIdRatioBased`.
 pub sampling_rate: f64,
 /// Resource attributes injected on every emitted span (e.g.
 /// `("service.name", "mnemosyne-server")`,
 /// `("deployment.environment", "prod")`). Surfaced under the OTLP
 /// `resource.attributes` field on the wire.
 pub resource_attributes: Vec<(String, String)>,
}

#[cfg(feature = "otlp")]
impl OtlpExporterConfig {
 /// Defaults: SDK batch defaults, sampling rate 1.0, no resource attrs.
 pub fn new(endpoint: impl Into<String>) -> Self {
 Self {
 endpoint: endpoint.into(),
 batch_max_export_batch_size: 512,
 batch_scheduled_delay: std::time::Duration::from_secs(5),
 sampling_rate: 1.0,
 resource_attributes: Vec::new(),
 }
 }

 pub fn with_batch_max_export_batch_size(mut self, n: usize) -> Self {
 self.batch_max_export_batch_size = n;
 self
 }

 pub fn with_batch_scheduled_delay(mut self, d: std::time::Duration) -> Self {
 self.batch_scheduled_delay = d;
 self
 }

 pub fn with_sampling_rate(mut self, rate: f64) -> Self {
 self.sampling_rate = rate;
 self
 }

 pub fn with_resource_attribute(
 mut self,
 key: impl Into<String>,
 value: impl Into<String>,
 ) -> Self {
 self.resource_attributes.push((key.into(), value.into()));
 self
 }
}

/// Round 102 — initialize a tracing subscriber that forwards spans to an OTLP
/// gRPC collector. Composes `opentelemetry-otlp` (tonic transport) with
/// `tracing-opentelemetry`, so existing `tracing::Span` instrumentation
/// (Round 96 grpc.method, Round 99 trace_id) propagates through the OTLP wire
/// format without further code changes.
///
/// `endpoint` is an `http://host:port` URL; the helper routes via the
/// `grpc-tonic` exporter path. The returned [`OtlpTracerGuard`] keeps the
/// `SdkTracerProvider` alive for the lifetime of the process — drop it during
/// graceful shutdown to flush in-flight spans.
///
/// Round 106 — thin wrapper around [`init_otlp_tracing_subscriber_with_config`]
/// using SDK defaults. Use the `_with_config` entry point when batch tuning,
/// sampling, or resource attribute injection is needed.
///
/// Idempotency note: this installs a global default subscriber via
/// `tracing_subscriber::registry().init()`. Calling it twice in the same
/// process panics — the helper is intended for the server's bring-up path
/// (or in tests, behind a one-shot `Once` guard).
///
/// Round 102 — gated behind the `otlp` feature. Default builds carry no
/// opentelemetry compile cost (Round 99 trace_id field stays a plain `String`
/// without OTLP wiring).
#[cfg(feature = "otlp")]
pub fn init_otlp_tracing_subscriber(endpoint: &str) -> Result<OtlpTracerGuard, String> {
 init_otlp_tracing_subscriber_with_config(OtlpExporterConfig::new(endpoint))
}

/// Round 106 — initialize an OTLP tracing subscriber from a fully-specified
/// [`OtlpExporterConfig`]. Wires the batch span processor (queue size +
/// scheduled delay), the trace-id-ratio sampler, and resource attributes
/// onto the SDK pipeline.
#[cfg(feature = "otlp")]
pub fn init_otlp_tracing_subscriber_with_config(
 cfg: OtlpExporterConfig,
) -> Result<OtlpTracerGuard, String> {
 use opentelemetry::trace::TracerProvider as _;
 use opentelemetry::KeyValue;
 use opentelemetry_otlp::WithExportConfig as _;
 use opentelemetry_sdk::trace::{BatchConfigBuilder, Config as SdkTraceConfig, Sampler};
 use opentelemetry_sdk::Resource;
 use tracing_subscriber::layer::SubscriberExt as _;
 use tracing_subscriber::util::SubscriberInitExt as _;

 let exporter = opentelemetry_otlp::new_exporter()
 .tonic()
 .with_endpoint(cfg.endpoint.clone());

 let batch_config = BatchConfigBuilder::default()
 .with_max_export_batch_size(cfg.batch_max_export_batch_size)
 .with_scheduled_delay(cfg.batch_scheduled_delay)
 .build();

 let resource_kvs: Vec<KeyValue> = cfg
 .resource_attributes
 .iter()
 .map(|(k, v)| KeyValue::new(k.clone(), v.clone()))
 .collect();
 let resource = if resource_kvs.is_empty() {
 Resource::empty()
 } else {
 Resource::new(resource_kvs)
 };

 let trace_config = SdkTraceConfig::default()
 .with_sampler(Sampler::TraceIdRatioBased(cfg.sampling_rate))
 .with_resource(resource);

 let provider = opentelemetry_otlp::new_pipeline()
 .tracing()
 .with_exporter(exporter)
 .with_batch_config(batch_config)
 .with_trace_config(trace_config)
 .install_batch(opentelemetry_sdk::runtime::Tokio)
 .map_err(|e| format!("OTLP pipeline install failed: {e}"))?;

 let tracer = provider.tracer("mnemosyne-server");
 let layer = tracing_opentelemetry::layer().with_tracer(tracer);

 tracing_subscriber::registry()
 .with(layer)
 .try_init()
 .map_err(|e| format!("tracing subscriber init failed: {e}"))?;

 Ok(OtlpTracerGuard { provider })
}

/// Round 102 — RAII guard that owns the OTLP tracer provider; dropping flushes
/// pending spans (best-effort — relies on the SDK's batch exporter shutdown).
#[cfg(feature = "otlp")]
pub struct OtlpTracerGuard {
 provider: opentelemetry_sdk::trace::TracerProvider,
}

#[cfg(feature = "otlp")]
impl Drop for OtlpTracerGuard {
 fn drop(&mut self) {
 let _ = self.provider.shutdown();
 }
}

// --- conversions: proto wire <-> embedded Rust types ------------------------

fn decode_proposal(wire: proto::Proposal) -> Result<Proposal, String> {
 let kind_wrapper = wire
 .kind
 .ok_or_else(|| "Proposal.kind missing".to_string())?;
 let inner = kind_wrapper
 .kind
 .ok_or_else(|| "ProposalKind.kind oneof empty".to_string())?;
 let kind = match inner {
 proto::proposal_kind::Kind::EntityCreate(p) => ProposalKind::EntityCreate {
 entity_type: p.entity_type,
 branch_id: p.branch_id,
 entity_id: p.entity_id,
 valid_from: p.valid_from,
 payload: p.payload,
 },
 proto::proposal_kind::Kind::EntityUpdate(p) => ProposalKind::EntityUpdate {
 entity_type: p.entity_type,
 branch_id: p.branch_id,
 entity_id: p.entity_id,
 valid_from: p.valid_from,
 payload: p.payload,
 },
 proto::proposal_kind::Kind::ChangelogAppend(p) => ProposalKind::ChangelogAppend {
 branch_id: p.branch_id,
 entity_id: p.entity_id,
 valid_from: p.valid_from,
 payload: p.payload,
 },
 proto::proposal_kind::Kind::CrossRefCreate(p) => ProposalKind::CrossRefCreate {
 branch_id: p.branch_id,
 from_section: p.from_section,
 to_section: p.to_section,
 ref_kind: p.ref_kind,
 },
 proto::proposal_kind::Kind::FrozenListMembershipChange(p) => {
 ProposalKind::FrozenListMembershipChange {
  branch_id: p.branch_id,
  list_id: p.list_id,
  valid_from: p.valid_from,
  attached_changelog_entry_id: p.attached_changelog_entry_id,
  payload: p.payload,
 }
 }
 };
 Ok(Proposal {
 proposal_id: wire.proposal_id,
 actor: wire.actor,
 kind,
 })
}

fn encode_result(result: ProposalResult) -> proto::ProposalResult {
 let (audit_id, audit_set) = match result.audit_transaction_id {
 Some(id) => (id, true),
 None => (0, false),
 };
 let (reason, reason_set) = match result.rejection_reason {
 Some(r) => (r, true),
 None => (String::new(), false),
 };
 proto::ProposalResult {
 proposal_id: result.proposal_id,
 accepted: result.accepted,
 audit_transaction_id: audit_id,
 audit_transaction_id_set: audit_set,
 rejection_reason: reason,
 rejection_reason_set: reason_set,
 }
}

/// Inverse of `encode_result` — used by client integration tests to compare
/// the wire response back to the embedded `ProposalResult` value shape.
pub fn decode_result(wire: proto::ProposalResult) -> ProposalResult {
 ProposalResult {
 proposal_id: wire.proposal_id,
 accepted: wire.accepted,
 audit_transaction_id: wire.audit_transaction_id_set.then_some(wire.audit_transaction_id),
 rejection_reason: wire.rejection_reason_set.then_some(wire.rejection_reason),
 }
}

/// Round 98 — encode the embedded `AuditRecord` into its proto wire form for
/// `SubscribeAuditTrail` streaming. Round 99 — also carries `trace_id`.
/// Round 104 — also carries `tracestate`.
pub fn encode_audit_record(record: &AuditRecord) -> proto::AuditRecord {
 let (reason, reason_set) = match &record.rejection_reason {
 Some(r) => (r.clone(), true),
 None => (String::new(), false),
 };
 let (trace, trace_set) = match &record.trace_id {
 Some(t) => (t.clone(), true),
 None => (String::new(), false),
 };
 let (tracestate, tracestate_set) = match &record.tracestate {
 Some(t) => (t.clone(), true),
 None => (String::new(), false),
 };
 proto::AuditRecord {
 transaction_id: record.transaction_id,
 proposal_id: record.proposal_id.clone(),
 actor: record.actor.clone(),
 accepted: record.accepted,
 gate_routing_reason: record.gate_routing_reason.clone(),
 rejection_reason: reason,
 rejection_reason_set: reason_set,
 proposal_kind_tag: record.proposal_kind_tag.clone(),
 trace_id: trace,
 trace_id_set: trace_set,
 tracestate,
 tracestate_set,
 }
}

/// Inverse of `encode_audit_record` — used by client integration tests to
/// recover the embedded record shape from the wire message. Round 99 — also
/// recovers `trace_id`. Round 104 — also recovers `tracestate`.
pub fn decode_audit_record(wire: proto::AuditRecord) -> AuditRecord {
 AuditRecord {
 transaction_id: wire.transaction_id,
 proposal_id: wire.proposal_id,
 actor: wire.actor,
 accepted: wire.accepted,
 gate_routing_reason: wire.gate_routing_reason,
 rejection_reason: wire.rejection_reason_set.then_some(wire.rejection_reason),
 proposal_kind_tag: wire.proposal_kind_tag,
 trace_id: wire.trace_id_set.then_some(wire.trace_id),
 tracestate: wire.tracestate_set.then_some(wire.tracestate),
 }
}

/// Encode the embedded `Proposal` into its proto wire form (used by clients).
pub fn encode_proposal(p: Proposal) -> proto::Proposal {
 let kind = match p.kind {
 ProposalKind::EntityCreate {
 entity_type,
 branch_id,
 entity_id,
 valid_from,
 payload,
 } => proto::proposal_kind::Kind::EntityCreate(proto::EntityCreate {
 entity_type,
 branch_id,
 entity_id,
 valid_from,
 payload,
 }),
 ProposalKind::EntityUpdate {
 entity_type,
 branch_id,
 entity_id,
 valid_from,
 payload,
 } => proto::proposal_kind::Kind::EntityUpdate(proto::EntityUpdate {
 entity_type,
 branch_id,
 entity_id,
 valid_from,
 payload,
 }),
 ProposalKind::ChangelogAppend {
 branch_id,
 entity_id,
 valid_from,
 payload,
 } => proto::proposal_kind::Kind::ChangelogAppend(proto::ChangelogAppend {
 branch_id,
 entity_id,
 valid_from,
 payload,
 }),
 ProposalKind::CrossRefCreate {
 branch_id,
 from_section,
 to_section,
 ref_kind,
 } => proto::proposal_kind::Kind::CrossRefCreate(proto::CrossRefCreate {
 branch_id,
 from_section,
 to_section,
 ref_kind,
 }),
 ProposalKind::FrozenListMembershipChange {
 branch_id,
 list_id,
 valid_from,
 attached_changelog_entry_id,
 payload,
 } => proto::proposal_kind::Kind::FrozenListMembershipChange(
 proto::FrozenListMembershipChange {
  branch_id,
  list_id,
  valid_from,
  attached_changelog_entry_id,
  payload,
 },
 ),
 };
 proto::Proposal {
 proposal_id: p.proposal_id,
 actor: p.actor,
 kind: Some(proto::ProposalKind { kind: Some(kind) }),
 }
}

#[cfg(test)]
mod tests {
 use super::*;

 fn embedded_round_trip(p: Proposal) -> Proposal {
 let wire = encode_proposal(p.clone());
 decode_proposal(wire).expect("decode")
 }

 #[test]
 fn entity_create_proto_round_trip() {
 let p = Proposal {
 proposal_id: "p-001".into(),
 actor: "alice".into(),
 kind: ProposalKind::EntityCreate {
  entity_type: "Section".into(),
  branch_id: 1,
  entity_id: 42,
  valid_from: 1000,
  payload: b"abc".to_vec(),
 },
 };
 assert_eq!(p, embedded_round_trip(p.clone()));
 }

 #[test]
 fn cross_ref_proto_round_trip() {
 let p = Proposal {
 proposal_id: "p-002".into(),
 actor: "alice".into(),
 kind: ProposalKind::CrossRefCreate {
  branch_id: 1,
  from_section: 66,
  to_section: 39,
  ref_kind: "decision".into(),
 },
 };
 assert_eq!(p, embedded_round_trip(p.clone()));
 }

 #[test]
 fn frozen_list_membership_proto_round_trip() {
 let p = Proposal {
 proposal_id: "p-003".into(),
 actor: "alice".into(),
 kind: ProposalKind::FrozenListMembershipChange {
  branch_id: 1,
  list_id: 1000,
  valid_from: 100,
  attached_changelog_entry_id: 60,
  payload: b"snap".to_vec(),
 },
 };
 assert_eq!(p, embedded_round_trip(p.clone()));
 }

 #[test]
 fn proposal_result_some_audit_round_trip() {
 let r = ProposalResult {
 proposal_id: "p-001".into(),
 accepted: true,
 audit_transaction_id: Some(42),
 rejection_reason: None,
 };
 let wire = encode_result(r.clone());
 assert_eq!(decode_result(wire), r);
 }

 #[test]
 fn proposal_result_rejection_round_trip() {
 let r = ProposalResult {
 proposal_id: "p-002".into(),
 accepted: false,
 audit_transaction_id: Some(7),
 rejection_reason: Some("orphan cross-ref".into()),
 };
 let wire = encode_result(r.clone());
 assert_eq!(decode_result(wire), r);
 }

 #[test]
 fn proposal_result_no_audit_no_reason_round_trip() {
 let r = ProposalResult {
 proposal_id: "p-003".into(),
 accepted: false,
 audit_transaction_id: None,
 rejection_reason: None,
 };
 let wire = encode_result(r.clone());
 assert_eq!(decode_result(wire), r);
 }

 #[test]
 fn decode_proposal_rejects_missing_kind() {
 let wire = proto::Proposal {
 proposal_id: "p".into(),
 actor: "a".into(),
 kind: None,
 };
 assert!(decode_proposal(wire).is_err());
 }

 #[test]
 fn decode_proposal_rejects_empty_kind_oneof() {
 let wire = proto::Proposal {
 proposal_id: "p".into(),
 actor: "a".into(),
 kind: Some(proto::ProposalKind { kind: None }),
 };
 assert!(decode_proposal(wire).is_err());
 }
}
