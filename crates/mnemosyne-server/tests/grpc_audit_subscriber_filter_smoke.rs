//! gRPC audit per-subscriber filter smoke (Round 110).
//!
//! Substantiates the `SubscribeAuditRequest.proposal_kind_filter` and
//! `actor_filter` fields. 5 historical commits cover 3 distinct
//! `proposal_kind_tag`s (entity_create / changelog_append / cross_ref_create
//! reject) across 2 actors (alice / bob); the test subscribes with a
//! kind+actor filter pair and asserts only matching records are emitted.
//! Empty filter lists must preserve the wholesale forwarding semantics of
//! Round 103.

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

fn entity_create(id: &str, actor: &str, entity_id: u64) -> Proposal {
    Proposal {
        proposal_id: id.into(),
        actor: actor.into(),
        kind: ProposalKind::EntityCreate {
            entity_type: "Section".into(),
            branch_id: 1,
            entity_id,
            valid_from: entity_id * 1000,
            payload: b"payload".to_vec(),
        },
    }
}

fn changelog_append(id: &str, actor: &str, entity_id: u64) -> Proposal {
    Proposal {
        proposal_id: id.into(),
        actor: actor.into(),
        kind: ProposalKind::ChangelogAppend {
            branch_id: 1,
            entity_id,
            valid_from: entity_id * 1000 + 1,
            payload: b"changelog".to_vec(),
        },
    }
}

fn cross_ref_orphan(id: &str, actor: &str) -> Proposal {
    Proposal {
        proposal_id: id.into(),
        actor: actor.into(),
        kind: ProposalKind::CrossRefCreate {
            branch_id: 1,
            from_section: 0,
            to_section: 39,
            ref_kind: "decision".into(),
        },
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn proposal_kind_and_actor_filters_intersect_via_and() {
    let (_dir, handler) = fresh_handler();
    // Pre-commit 5 records covering 3 kinds × 2 actors:
    // txn 1: entity_create alice → matches kind, actor
    // txn 2: changelog_append alice → matches actor only
    // txn 3: entity_create bob → matches kind only
    // txn 4: cross_ref_create alice (orphan rejection — t1_reject) → matches actor only
    // txn 5: changelog_append bob → matches neither
    handler
        .handle(&entity_create("p1", "alice", 1))
        .expect("commit 1");
    handler
        .handle(&entity_create("p-prep-2", "alice", 2))
        .expect("prep for changelog");
    handler
        .handle(&changelog_append("p2", "alice", 2))
        .expect("commit 2");
    handler
        .handle(&entity_create("p3", "bob", 3))
        .expect("commit 3");
    handler
        .handle(&cross_ref_orphan("p4", "alice"))
        .expect("commit 4 (audited rejection)");
    handler
        .handle(&entity_create("p-prep-5", "bob", 5))
        .expect("prep");
    handler
        .handle(&changelog_append("p5", "bob", 5))
        .expect("commit 5");

    let (addr, shutdown_tx, server) = bring_up_server(Arc::clone(&handler)).await;
    let mut client = MnemosyneClient::connect(format!("http://{addr}"))
        .await
        .expect("connect");

    // Filter: only entity_create kind AND only alice. Of the 7 audit
    // records committed above (5 substantive + 2 prep entity_creates),
    // entity_create+alice records are: txn1 (p1, alice) and the prep
    // entity_create for p2 ("p-prep-2", alice). bob's entity_creates are
    // dropped, alice's changelog/cross_ref are dropped, bob's anything
    // is dropped.
    let response = client
        .subscribe_audit_trail(SubscribeAuditRequest {
            from_transaction_id: 0,
            max_records: 0,
            follow_tail: false,
            resume_on_lag: false,
            proposal_kind_filter: vec!["entity_create".to_string()],
            actor_filter: vec!["alice".to_string()],
        })
        .await
        .expect("subscribe filtered")
        .into_inner();

    let received: Vec<_> = response
        .collect::<Vec<Result<_, tonic::Status>>>()
        .await
        .into_iter()
        .map(|r| r.expect("ok"))
        .map(decode_audit_record)
        .collect();

    assert!(
        !received.is_empty(),
        "filter must allow at least one matching record through"
    );
    for r in &received {
        assert_eq!(
            r.proposal_kind_tag, "entity_create",
            "non-matching kind escaped the filter: {}",
            r.proposal_kind_tag
        );
        assert_eq!(
            r.actor, "alice",
            "non-matching actor escaped the filter: {}",
            r.actor
        );
    }
    // The two qualifying records are the entity_create commits for "p1"
    // and "p-prep-2"; both are alice.
    let proposal_ids: std::collections::HashSet<&str> =
        received.iter().map(|r| r.proposal_id.as_str()).collect();
    assert!(
        proposal_ids.contains("p1") && proposal_ids.contains("p-prep-2"),
        "expected both alice/entity_create proposals, got {:?}",
        proposal_ids
    );

    shutdown_tx.send(()).ok();
    server.await.ok();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn empty_filters_preserve_wholesale_forwarding() {
    let (_dir, handler) = fresh_handler();
    handler
        .handle(&entity_create("p1", "alice", 1))
        .expect("commit 1");
    handler
        .handle(&entity_create("p2", "bob", 2))
        .expect("commit 2");

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
        .expect("subscribe wholesale")
        .into_inner();

    let received: Vec<_> = response
        .collect::<Vec<Result<_, tonic::Status>>>()
        .await
        .into_iter()
        .map(|r| r.expect("ok"))
        .map(decode_audit_record)
        .collect();
    assert_eq!(
        received.len(),
        2,
        "empty filters must forward every record (Round 103 carry stable)"
    );

    shutdown_tx.send(()).ok();
    server.await.ok();
}
