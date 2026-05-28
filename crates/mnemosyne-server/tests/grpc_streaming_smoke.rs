//! gRPC streaming RPC smoke test (Round 98).
//!
//! Exercises the two streaming RPCs introduced in Round 98 against a real tonic
//! server bound on a fresh store:
//! - `SubmitProposalBatch` — client-streaming → server-streaming. Sends 3
//! proposals (2 valid + 1 orphan-rejected) over the inbound stream and
//! verifies the server emits 3 `ProposalResult` items in arrival order
//! with audit transaction ids 1..=3.
//! - `SubscribeAuditTrail` — server-streaming. After committing 3 proposals
//! via the unary path, subscribes from `from_transaction_id=2` and asserts
//! the stream replays exactly the 2nd and 3rd records in monotonic order.

use mnemosyne_server::grpc::proto::SubscribeAuditRequest;
use mnemosyne_server::grpc::{
    decode_audit_record, encode_proposal, MnemosyneClient, MnemosyneGrpcService,
};
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
) -> (SocketAddr, oneshot::Sender<()>, tokio::task::JoinHandle<()>) {
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

fn entity_create_proposal(id: &str, entity_id: u64) -> Proposal {
    Proposal {
        proposal_id: id.into(),
        actor: "stream-tester".into(),
        kind: ProposalKind::EntityCreate {
            entity_type: "Section".into(),
            branch_id: 1,
            entity_id,
            valid_from: entity_id * 1000,
            payload: format!("payload-{entity_id}").into_bytes(),
        },
    }
}

fn cross_ref_orphan_proposal(id: &str) -> Proposal {
    Proposal {
        proposal_id: id.into(),
        actor: "stream-tester".into(),
        kind: ProposalKind::CrossRefCreate {
            branch_id: 1,
            from_section: 0, // orphan: from unresolved → Tier 1 reject
            to_section: 39,
            ref_kind: "decision".into(),
        },
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn submit_proposal_batch_emits_one_result_per_inbound_proposal() {
    let (_dir, handler) = fresh_handler();
    let (addr, shutdown_tx, server) = bring_up_server(handler).await;
    let endpoint = format!("http://{}", addr);
    let mut client = MnemosyneClient::connect(endpoint).await.expect("connect");

    let proposals = vec![
        encode_proposal(entity_create_proposal("p-batch-001", 1)),
        encode_proposal(cross_ref_orphan_proposal("p-batch-002")),
        encode_proposal(entity_create_proposal("p-batch-003", 3)),
    ];
    let inbound = tokio_stream::iter(proposals);

    let response = client
        .submit_proposal_batch(inbound)
        .await
        .expect("submit_proposal_batch")
        .into_inner();

    let collected: Vec<_> = response
        .collect::<Vec<Result<_, tonic::Status>>>()
        .await
        .into_iter()
        .map(|r| r.expect("server-side stream ok"))
        .collect();

    assert_eq!(collected.len(), 3, "one result per inbound proposal");
    assert_eq!(collected[0].proposal_id, "p-batch-001");
    assert!(collected[0].accepted);
    assert_eq!(collected[0].audit_transaction_id, 1);
    assert_eq!(collected[1].proposal_id, "p-batch-002");
    assert!(!collected[1].accepted, "orphan cross-ref must reject");
    assert_eq!(collected[1].audit_transaction_id, 2);
    assert!(!collected[1].rejection_reason.is_empty());
    assert_eq!(collected[2].proposal_id, "p-batch-003");
    assert!(collected[2].accepted);
    assert_eq!(collected[2].audit_transaction_id, 3);

    shutdown_tx.send(()).ok();
    server.await.ok();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn subscribe_audit_trail_replays_records_from_cursor() {
    let (_dir, handler) = fresh_handler();
    // Commit three proposals through the embedded handler so audit txns 1..=3
    // exist on disk before subscription.
    for entity_id in 1..=3u64 {
        let proposal = entity_create_proposal(&format!("p-pre-{entity_id}"), entity_id);
        handler.handle(&proposal).expect("pre-commit");
    }

    let (addr, shutdown_tx, server) = bring_up_server(handler).await;
    let endpoint = format!("http://{}", addr);
    let mut client = MnemosyneClient::connect(endpoint).await.expect("connect");

    let response = client
        .subscribe_audit_trail(SubscribeAuditRequest {
            from_transaction_id: 2,
            max_records: 0,
            follow_tail: false,
            resume_on_lag: false,
            proposal_kind_filter: vec![],
            actor_filter: vec![],
        })
        .await
        .expect("subscribe_audit_trail")
        .into_inner();

    let collected: Vec<_> = response
        .collect::<Vec<Result<_, tonic::Status>>>()
        .await
        .into_iter()
        .map(|r| r.expect("server-side stream ok"))
        .map(decode_audit_record)
        .collect();

    assert_eq!(collected.len(), 2, "cursor=2 must replay txns 2 and 3");
    assert_eq!(collected[0].transaction_id, 2);
    assert_eq!(collected[0].proposal_id, "p-pre-2");
    assert!(collected[0].accepted);
    assert_eq!(collected[1].transaction_id, 3);
    assert_eq!(collected[1].proposal_id, "p-pre-3");
    assert!(collected[1].accepted);

    shutdown_tx.send(()).ok();
    server.await.ok();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn subscribe_audit_trail_honors_max_records_cap() {
    let (_dir, handler) = fresh_handler();
    for entity_id in 1..=5u64 {
        let proposal = entity_create_proposal(&format!("p-pre-{entity_id}"), entity_id);
        handler.handle(&proposal).expect("pre-commit");
    }

    let (addr, shutdown_tx, server) = bring_up_server(handler).await;
    let endpoint = format!("http://{}", addr);
    let mut client = MnemosyneClient::connect(endpoint).await.expect("connect");

    // 5 records on disk; cursor=0, cap=2 → first 2 records only.
    let response = client
        .subscribe_audit_trail(SubscribeAuditRequest {
            from_transaction_id: 0,
            max_records: 2,
            follow_tail: false,
            resume_on_lag: false,
            proposal_kind_filter: vec![],
            actor_filter: vec![],
        })
        .await
        .expect("subscribe_audit_trail")
        .into_inner();

    let collected: Vec<_> = response
        .collect::<Vec<Result<_, tonic::Status>>>()
        .await
        .into_iter()
        .map(|r| r.expect("server-side stream ok"))
        .collect();

    assert_eq!(collected.len(), 2, "max_records=2 caps the outbound stream");
    assert_eq!(collected[0].transaction_id, 1);
    assert_eq!(collected[1].transaction_id, 2);

    shutdown_tx.send(()).ok();
    server.await.ok();
}
