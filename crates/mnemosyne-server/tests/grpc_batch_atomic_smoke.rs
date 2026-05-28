//! gRPC submit_proposal_batch atomic mode smoke (Round 112).
//!
//! Substantiates the Round 112 atomic batch path: when the client sets
//! the `x-mnemosyne-batch-atomic = "true"` request metadata header, the
//! server collects all inbound proposals, evaluates gates on each, and
//! either commits every storage write in a single
//! `write_batch_multi_cf` (when all gates accept) or rejects every
//! proposal in the batch (when any gate rejects). The default mode
//! (no metadata or any other value) preserves the Round 98 per-proposal
//! incremental commit behavior.

use mnemosyne_server::grpc::{
    decode_result, encode_proposal, MnemosyneClient, MnemosyneGrpcService,
};
use mnemosyne_server::handler::ProposalHandler;
use mnemosyne_server::proposal::{Proposal, ProposalKind};
use mnemosyne_store::{CfId, MnemosyneStore};
use std::net::SocketAddr;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio_stream::wrappers::TcpListenerStream;
use tokio_stream::StreamExt;
use tonic::metadata::MetadataValue;
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
        actor: "atomic-tester".into(),
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
        actor: "atomic-tester".into(),
        kind: ProposalKind::CrossRefCreate {
            branch_id: 1,
            from_section: 0, // Tier 1 reject — orphan
            to_section: 39,
            ref_kind: "decision".into(),
        },
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn atomic_batch_with_one_reject_blocks_all_storage_writes() {
    let (_dir, handler) = fresh_handler();
    let store_for_check = Arc::clone(handler.store());
    let (addr, shutdown_tx, server) = bring_up_server(Arc::clone(&handler)).await;
    let mut client = MnemosyneClient::connect(format!("http://{addr}"))
        .await
        .expect("connect");

    // 3 proposals: 2 valid entity_create + 1 orphan cross_ref (Tier 1 reject).
    let proposals = vec![
        encode_proposal(entity_create_proposal("p-atomic-1", 1)),
        encode_proposal(cross_ref_orphan_proposal("p-atomic-orphan")),
        encode_proposal(entity_create_proposal("p-atomic-3", 3)),
    ];
    let mut req = tonic::Request::new(tokio_stream::iter(proposals));
    req.metadata_mut().insert(
        "x-mnemosyne-batch-atomic",
        MetadataValue::try_from("true").expect("metadata"),
    );

    let response = client
        .submit_proposal_batch(req)
        .await
        .expect("submit_proposal_batch atomic")
        .into_inner();
    let collected: Vec<_> = response
        .collect::<Vec<Result<_, tonic::Status>>>()
        .await
        .into_iter()
        .map(|r| r.expect("server-side stream ok"))
        .map(decode_result)
        .collect();

    assert_eq!(
        collected.len(),
        3,
        "atomic batch must emit one result per proposal"
    );
    for r in &collected {
        assert!(
 !r.accepted,
 "atomic mode: every proposal in the batch must reject when any one rejects (proposal {} accepted)",
 r.proposal_id
 );
        let reason = r.rejection_reason.as_deref().unwrap_or("");
        assert!(
            reason.starts_with("atomic batch rejected:"),
            "rejection reason must carry the `atomic batch rejected` prefix; got: {reason}"
        );
    }

    // Storage atomicity: NEITHER of the would-be-accepted proposals
    // landed on disk. The orphan cross_ref was never going to commit
    // anyway, but #1 and #3 SHOULD have committed in non-atomic mode.
    assert!(
        store_for_check
            .get(CfId::Entities, 1, 1, 1000)
            .expect("get 1")
            .is_none(),
        "atomic reject must not write entity_id=1"
    );
    assert!(
        store_for_check
            .get(CfId::Entities, 1, 3, 3000)
            .expect("get 3")
            .is_none(),
        "atomic reject must not write entity_id=3"
    );

    shutdown_tx.send(()).ok();
    server.await.ok();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn atomic_batch_with_all_accepts_commits_all_in_one_writebatch() {
    let (_dir, handler) = fresh_handler();
    let store_for_check = Arc::clone(handler.store());
    let (addr, shutdown_tx, server) = bring_up_server(Arc::clone(&handler)).await;
    let mut client = MnemosyneClient::connect(format!("http://{addr}"))
        .await
        .expect("connect");

    let proposals = vec![
        encode_proposal(entity_create_proposal("p-atomic-A", 10)),
        encode_proposal(entity_create_proposal("p-atomic-B", 11)),
        encode_proposal(entity_create_proposal("p-atomic-C", 12)),
    ];
    let mut req = tonic::Request::new(tokio_stream::iter(proposals));
    req.metadata_mut().insert(
        "x-mnemosyne-batch-atomic",
        MetadataValue::try_from("true").expect("metadata"),
    );

    let response = client
        .submit_proposal_batch(req)
        .await
        .expect("submit_proposal_batch atomic")
        .into_inner();
    let collected: Vec<_> = response
        .collect::<Vec<Result<_, tonic::Status>>>()
        .await
        .into_iter()
        .map(|r| r.expect("ok"))
        .map(decode_result)
        .collect();

    assert_eq!(collected.len(), 3);
    for r in &collected {
        assert!(
            r.accepted,
            "all-accept atomic batch must accept every proposal; rejected {}",
            r.proposal_id
        );
        assert!(r.audit_transaction_id.is_some());
    }

    // All three storage writes landed.
    for entity_id in [10, 11, 12] {
        let row = store_for_check
            .get(CfId::Entities, 1, entity_id, entity_id * 1000)
            .expect("get")
            .expect("row present");
        assert_eq!(row, format!("payload-{entity_id}").into_bytes());
    }

    shutdown_tx.send(()).ok();
    server.await.ok();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn non_atomic_batch_preserves_per_proposal_98_semantics() {
    // Round 98 baseline regression: without the atomic metadata flag, a
    // batch with one rejected proposal still commits the surrounding
    // accepted ones. This is the existing per-proposal incremental
    // commit behavior.
    let (_dir, handler) = fresh_handler();
    let store_for_check = Arc::clone(handler.store());
    let (addr, shutdown_tx, server) = bring_up_server(Arc::clone(&handler)).await;
    let mut client = MnemosyneClient::connect(format!("http://{addr}"))
        .await
        .expect("connect");

    let proposals = vec![
        encode_proposal(entity_create_proposal("p-nonatomic-1", 100)),
        encode_proposal(cross_ref_orphan_proposal("p-nonatomic-orphan")),
        encode_proposal(entity_create_proposal("p-nonatomic-3", 102)),
    ];
    // No atomic metadata.
    let req = tonic::Request::new(tokio_stream::iter(proposals));

    let response = client
        .submit_proposal_batch(req)
        .await
        .expect("submit_proposal_batch")
        .into_inner();
    let collected: Vec<_> = response
        .collect::<Vec<Result<_, tonic::Status>>>()
        .await
        .into_iter()
        .map(|r| r.expect("ok"))
        .map(decode_result)
        .collect();

    assert_eq!(collected.len(), 3);
    assert!(
        collected[0].accepted,
        "non-atomic: valid proposal 1 must accept"
    );
    assert!(
        !collected[1].accepted,
        "non-atomic: orphan proposal must reject"
    );
    assert!(
        collected[2].accepted,
        "non-atomic: valid proposal 3 must accept (surrounding orphan)"
    );

    // Storage: 100 and 102 committed, 101 not (orphan didn't write).
    assert!(store_for_check
        .get(CfId::Entities, 1, 100, 100000)
        .expect("get 100")
        .is_some());
    assert!(store_for_check
        .get(CfId::Entities, 1, 102, 102000)
        .expect("get 102")
        .is_some());

    shutdown_tx.send(()).ok();
    server.await.ok();
}
