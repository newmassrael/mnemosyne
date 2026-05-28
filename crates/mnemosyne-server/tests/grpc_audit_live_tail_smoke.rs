//! gRPC audit live tail-following smoke test (Round 103).
//!
//! Exercises the broadcast-channel tail-follow path on `SubscribeAuditTrail`:
//!
//! 1. **History-then-tail** — pre-commit two records, subscribe with
//! `follow_tail=true` and `max_records=0` (unbounded). The stream first
//! drains the historical 2 records, then receives a third record committed
//! after subscription as a live push.
//!
//! 2. **max_records cap** — pre-commit two records, subscribe with
//! `follow_tail=true` and `max_records=2`. Only the historical 2 records
//! appear; the cap fires before any tail push, the stream closes cleanly.

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
        actor: "tail-tester".into(),
        kind: ProposalKind::EntityCreate {
            entity_type: "Section".into(),
            branch_id: 1,
            entity_id,
            valid_from: entity_id * 1000,
            payload: format!("payload-{entity_id}").into_bytes(),
        },
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn subscribe_audit_trail_pushes_post_subscription_records() {
    let (_dir, handler) = fresh_handler();
    // Pre-commit two records before subscribing.
    for entity_id in 1..=2u64 {
        let proposal = entity_create_proposal(&format!("p-pre-{entity_id}"), entity_id);
        handler.handle(&proposal).expect("pre-commit");
    }

    let (addr, shutdown_tx, server) = bring_up_server(Arc::clone(&handler)).await;
    let endpoint = format!("http://{}", addr);
    let mut client = MnemosyneClient::connect(endpoint).await.expect("connect");

    let mut response = client
        .subscribe_audit_trail(SubscribeAuditRequest {
            from_transaction_id: 0,
            max_records: 0,
            follow_tail: true,
            resume_on_lag: false,
            proposal_kind_filter: vec![],
            actor_filter: vec![],
        })
        .await
        .expect("subscribe_audit_trail")
        .into_inner();

    // Drain the 2 historical records.
    let first = response
        .next()
        .await
        .expect("history record 1")
        .expect("ok");
    let second = response
        .next()
        .await
        .expect("history record 2")
        .expect("ok");
    assert_eq!(decode_audit_record(first).transaction_id, 1);
    assert_eq!(decode_audit_record(second).transaction_id, 2);

    // Commit a third record after subscription — should arrive over the tail.
    let proposal = entity_create_proposal("p-tail-3", 3);
    handler.handle(&proposal).expect("post-subscription commit");

    let third = tokio::time::timeout(std::time::Duration::from_millis(500), response.next())
        .await
        .expect("tail push must arrive within timeout")
        .expect("tail record present")
        .expect("ok");
    let decoded = decode_audit_record(third);
    assert_eq!(decoded.transaction_id, 3);
    assert_eq!(decoded.proposal_id, "p-tail-3");
    assert!(decoded.accepted);

    // Drop the gRPC stream BEFORE awaiting the server so the in-flight
    // audit subscription task can detect tx.closed() and exit. Without this,
    // the spawn task stays parked on tail.recv() forever and the server's
    // graceful-shutdown wait deadlocks the test.
    drop(response);
    shutdown_tx.send(()).ok();
    server.await.ok();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn subscribe_audit_trail_max_records_cap_terminates_tail_follow() {
    let (_dir, handler) = fresh_handler();
    // Pre-commit two records.
    for entity_id in 1..=2u64 {
        let proposal = entity_create_proposal(&format!("p-pre-{entity_id}"), entity_id);
        handler.handle(&proposal).expect("pre-commit");
    }

    let (addr, shutdown_tx, server) = bring_up_server(Arc::clone(&handler)).await;
    let endpoint = format!("http://{}", addr);
    let mut client = MnemosyneClient::connect(endpoint).await.expect("connect");

    // 2 historical + tail-follow + cap=2 → only the 2 history records emit;
    // tail-follow phase sees the cap is already hit and exits cleanly.
    let response = client
        .subscribe_audit_trail(SubscribeAuditRequest {
            from_transaction_id: 0,
            max_records: 2,
            follow_tail: true,
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

    assert_eq!(
        collected.len(),
        2,
        "cap=2 must terminate stream after history"
    );
    assert_eq!(collected[0].transaction_id, 1);
    assert_eq!(collected[1].transaction_id, 2);

    // `collect()` already drained `response` to completion; the cap-terminated
    // server-side spawn task exits on its own. Server graceful shutdown clean.
    shutdown_tx.send(()).ok();
    server.await.ok();
}
