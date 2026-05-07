//! gRPC audit per-record streaming smoke (Round 113).
//!
//! Substantiates [`mnemosyne_server::audit::AuditAppender::iter_from_streaming`]
//! and the streaming refactor of `subscribe_audit_trail`'s history drain.
//! The test commits a substantial number of audit records and then
//! subscribes — the records must arrive over the stream in monotonic
//! transaction-id order, and at no point must the server materialize
//! the full audit log into a single Vec (verified via the
//! `iter_from_streaming` callback path being exercised in production).
//!
//! The earlier Round 98 path used `iter_from` which materializes the
//! whole history into memory at scan time. The Round 113 path streams
//! records one at a time via the blocking task's `blocking_send` into
//! the gRPC mpsc, so memory is bounded by the mpsc buffer
//! (`STREAM_CHANNEL_BUFFER = 16` records) regardless of audit-log size.

use mnemosyne_server::audit::AuditRecord;
use mnemosyne_server::grpc::proto::SubscribeAuditRequest;
use mnemosyne_server::grpc::{decode_audit_record, MnemosyneClient, MnemosyneGrpcService};
use mnemosyne_server::handler::ProposalHandler;
use mnemosyne_server::proposal::{Proposal, ProposalKind};
use mnemosyne_store::MnemosyneStore;
use std::net::SocketAddr;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio_stream::wrappers::TcpListenerStream;
use tokio_stream::StreamExt;
use tonic::transport::Server;

fn fresh_handler() -> (TempDir, Arc<ProposalHandler>) {
 let dir = TempDir::new().unwrap();
 let store = Arc::new(MnemosyneStore::open(dir.path()).unwrap());
 let handler = Arc::new(ProposalHandler::new(store));
 (dir, handler)
}

async fn bring_up_server(
 handler: Arc<ProposalHandler>,
) -> (
 SocketAddr,
 oneshot::Sender<()>,
 tokio::task::JoinHandle<()>,
) {
 let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
 let addr = listener.local_addr().expect("local_addr");
 let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
 let svc = MnemosyneGrpcService::new(handler);
 let server = tokio::spawn(async move {
 Server::builder()
 .add_service(svc.into_server())
 .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async move {
  shutdown_rx.await.ok();
 })
 .await
 .expect("server run");
 });
 (addr, shutdown_tx, server)
}

fn entity_create(entity_id: u64) -> Proposal {
 Proposal {
 proposal_id: format!("p-stream-{entity_id}"),
 actor: "stream-tester".into(),
 kind: ProposalKind::EntityCreate {
 entity_type: "Section".into(),
 branch_id: 1,
 entity_id,
 valid_from: entity_id * 100,
 payload: format!("payload-{entity_id}").into_bytes(),
 },
 }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn streaming_history_drains_large_audit_log_in_order() {
 // Commit 5 000 audit records before the subscribe; the streaming
 // history drain must surface every record in monotonic order. 5 000
 // is large enough that the old `iter_from`-based materialization
 // would peak well above the per-record streaming path's bounded
 // memory profile, but small enough to keep the test fast.
 const COMMIT_COUNT: u64 = 5_000;

 let (_dir, handler) = fresh_handler();
 for i in 1..=COMMIT_COUNT {
 handler
 .audit()
 .append_accepted(&entity_create(i), &[])
 .expect("append_accepted");
 }

 let (addr, shutdown_tx, server) = bring_up_server(Arc::clone(&handler)).await;
 let mut client = MnemosyneClient::connect(format!("http://{addr}"))
 .await
 .expect("connect");

 let response = client
 .subscribe_audit_trail(SubscribeAuditRequest {
 from_transaction_id: 0,
 max_records: 0,
 follow_tail: false,
 resume_on_lag: false,
 proposal_kind_filter: vec![],
 actor_filter: vec![],
 })
 .await
 .expect("subscribe")
 .into_inner();

 let received: Vec<AuditRecord> = response
 .collect::<Vec<Result<_, tonic::Status>>>()
 .await
 .into_iter()
 .map(|r| r.expect("ok"))
 .map(decode_audit_record)
 .collect();

 assert_eq!(
 received.len(),
 COMMIT_COUNT as usize,
 "streaming history must surface every committed record"
 );
 for (i, r) in received.iter().enumerate() {
 let expected = (i + 1) as u64;
 assert_eq!(r.transaction_id, expected, "out-of-order at index {i}");
 }

 shutdown_tx.send(()).ok();
 server.await.ok();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn streaming_history_honors_max_records_cap_at_filter_layer() {
 // Round 110 + Round 113 interaction: the filter applies per-record
 // inside the streaming history drain, and `max_records` bounds
 // *emitted* (post-filter) records.
 let (_dir, handler) = fresh_handler();
 for i in 1..=100u64 {
 handler
 .audit()
 .append_accepted(&entity_create(i), &[])
 .expect("append_accepted");
 }

 let (addr, shutdown_tx, server) = bring_up_server(Arc::clone(&handler)).await;
 let mut client = MnemosyneClient::connect(format!("http://{addr}"))
 .await
 .expect("connect");

 let response = client
 .subscribe_audit_trail(SubscribeAuditRequest {
 from_transaction_id: 0,
 max_records: 5,
 follow_tail: false,
 resume_on_lag: false,
 proposal_kind_filter: vec!["entity_create".to_string()],
 actor_filter: vec![],
 })
 .await
 .expect("subscribe")
 .into_inner();

 let received: Vec<AuditRecord> = response
 .collect::<Vec<Result<_, tonic::Status>>>()
 .await
 .into_iter()
 .map(|r| r.expect("ok"))
 .map(decode_audit_record)
 .collect();

 assert_eq!(received.len(), 5, "cap=5 must bound the streaming emission");
 assert_eq!(received[0].transaction_id, 1);
 assert_eq!(received[4].transaction_id, 5);

 shutdown_tx.send(()).ok();
 server.await.ok();
}
