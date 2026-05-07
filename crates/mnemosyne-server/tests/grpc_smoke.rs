//! gRPC ↔ embedded transport equivalence smoke test (Round 91).
//!
//! Both transports drive the same `ProposalHandler` against equivalent fresh
//! stores. The test asserts that for the same `Proposal` input the resulting
//! `ProposalResult` value matches across transports — same accept/reject
//! decision, same audit transaction id (both stores start clean so the first
//! proposal is always txn 1), same rejection reason.
//!
//! This is the *transport-agnostic ProposalHandler core preserve* invariant
//! from the Round 91 carry: gRPC is a thin adapter, not a parallel pipeline.

use mnemosyne_server::grpc::{
 decode_result, encode_proposal, MnemosyneClient, MnemosyneGrpcService,
};
use mnemosyne_server::handler::ProposalHandler;
use mnemosyne_server::proposal::{Proposal, ProposalKind, ProposalResult};
use mnemosyne_server::MnemosyneServer;
use mnemosyne_store::MnemosyneStore;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;

fn fresh_handler() -> (TempDir, Arc<ProposalHandler>) {
 let dir = TempDir::new().unwrap();
 let store = Arc::new(MnemosyneStore::open(dir.path()).unwrap());
 let handler = Arc::new(ProposalHandler::new(store));
 (dir, handler)
}

fn fresh_embedded_server() -> (TempDir, MnemosyneServer) {
 let dir = TempDir::new().unwrap();
 let store = Arc::new(MnemosyneStore::open(dir.path()).unwrap());
 (dir, MnemosyneServer::new(store))
}

async fn submit_via_grpc(
 handler: Arc<ProposalHandler>,
 proposal: Proposal,
) -> Result<ProposalResult, Box<dyn std::error::Error>> {
 let listener = TcpListener::bind("127.0.0.1:0").await?;
 let addr = listener.local_addr()?;
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
 let endpoint = format!("http://{}", addr);
 let mut client = MnemosyneClient::connect(endpoint).await?;
 let wire = encode_proposal(proposal);
 let response = client.submit_proposal(wire).await?.into_inner();
 shutdown_tx.send(()).ok();
 server.await.ok();
 Ok(decode_result(response))
}

fn entity_create_proposal(id: &str) -> Proposal {
 Proposal {
 proposal_id: id.into(),
 actor: "tester".into(),
 kind: ProposalKind::EntityCreate {
 entity_type: "Section".into(),
 branch_id: 1,
 entity_id: 42,
 valid_from: 1000,
 payload: b"section-payload".to_vec(),
 },
 }
}

fn cross_ref_orphan_proposal(id: &str) -> Proposal {
 Proposal {
 proposal_id: id.into(),
 actor: "tester".into(),
 kind: ProposalKind::CrossRefCreate {
 branch_id: 1,
 from_section: 0, // orphan: from unresolved → Tier 1 reject
 to_section: 39,
 ref_kind: "decision".into(),
 },
 }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn grpc_and_embedded_agree_on_entity_create_accept() {
 let proposal = entity_create_proposal("p-equiv-001");

 let (_embedded_dir, embedded_server) = fresh_embedded_server();
 let embedded_result = embedded_server.submit(&proposal).expect("embedded submit");

 let (_grpc_dir, grpc_handler) = fresh_handler();
 let grpc_result = submit_via_grpc(grpc_handler, proposal)
 .await
 .expect("grpc submit");

 assert_eq!(embedded_result, grpc_result);
 assert!(embedded_result.accepted);
 assert_eq!(embedded_result.audit_transaction_id, Some(1));
 assert_eq!(embedded_result.proposal_id, "p-equiv-001");
 assert!(embedded_result.rejection_reason.is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn grpc_and_embedded_agree_on_cross_ref_orphan_reject() {
 let proposal = cross_ref_orphan_proposal("p-equiv-002");

 let (_embedded_dir, embedded_server) = fresh_embedded_server();
 let embedded_result = embedded_server.submit(&proposal).expect("embedded submit");

 let (_grpc_dir, grpc_handler) = fresh_handler();
 let grpc_result = submit_via_grpc(grpc_handler, proposal)
 .await
 .expect("grpc submit");

 assert_eq!(embedded_result, grpc_result);
 assert!(!embedded_result.accepted);
 assert_eq!(embedded_result.audit_transaction_id, Some(1));
 assert!(embedded_result
 .rejection_reason
 .as_deref()
 .map(|r| !r.is_empty())
 .unwrap_or(false));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn grpc_storage_commit_is_durable_across_transport() {
 let proposal = entity_create_proposal("p-durable-001");
 let (_grpc_dir, handler) = fresh_handler();
 let store = Arc::clone(handler.store());

 let result = submit_via_grpc(Arc::clone(&handler), proposal)
 .await
 .expect("grpc submit");
 assert!(result.accepted);

 let stored = store
 .get(mnemosyne_store::CfId::Entities, 1, 42, 1000)
 .expect("get");
 assert_eq!(stored.as_deref(), Some(b"section-payload".as_ref()));
}
