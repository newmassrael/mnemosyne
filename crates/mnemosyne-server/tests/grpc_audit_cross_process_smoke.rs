//! Cross-process audit broadcast smoke (Round 111).
//!
//! Substantiates the [`mnemosyne_server::audit::AuditFanout`] trait surface
//! by wiring two independent Mnemosyne servers through a shared
//! [`InMemoryAuditBroker`]. Server A holds a publisher; server B's audit
//! appender receives records relayed from the broker via
//! `publish_external`. A direct subscriber on server B's broadcast
//! observes a record committed on server A — the cross-process fanout
//! that Round 103 (d)(iv) names.
//!
//! The in-memory broker stands in for a real redis pub/sub or NATS
//! adapter; the trait surface is identical, so a production deployment
//! swaps the adapter without changing the appender.
//!
//! §6 framing: each server's RocksDB audit CF still holds only its own
//! commits — the relayed record on server B never lands in server B's
//! audit CF. Cross-process fanout is an *observation* layer; the
//! append-only ledger remains the source of truth on each server.
//!
//! Why this test exercises the broadcast layer directly rather than the
//! gRPC `subscribe_audit_trail` surface: the gRPC tail flow is already
//! pinned by `grpc_audit_live_tail_smoke.rs` (Round 103), which proves
//! that any record reaching `broadcast_tx` reaches a tail subscriber.
//! The Round 111 carry only adds a new producer side
//! (`publish_external`) — wiring through the gRPC layer adds runtime
//! complexity without strengthening the assertion.

use mnemosyne_server::audit::{AuditRecord, InMemoryAuditBroker, AUDIT_BROADCAST_CAPACITY};
use mnemosyne_server::handler::ProposalHandler;
use mnemosyne_server::proposal::{Proposal, ProposalKind};
use mnemosyne_store::MnemosyneStore;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;

fn entity_create(id: &str, entity_id: u64) -> Proposal {
    Proposal {
        proposal_id: id.into(),
        actor: "cross-tester".into(),
        kind: ProposalKind::EntityCreate {
            entity_type: "Section".into(),
            branch_id: 1,
            entity_id,
            valid_from: entity_id * 1000,
            payload: b"cross-payload".to_vec(),
        },
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn audit_fanout_relays_record_from_writing_server_to_observer() {
    let broker = Arc::new(InMemoryAuditBroker::new());

    // Server A — writes records and publishes via the broker.
    let dir_a = TempDir::new().unwrap();
    let store_a = Arc::new(MnemosyneStore::open(dir_a.path()).unwrap());
    let handler_a = Arc::new(ProposalHandler::with_audit_fanout(
        store_a,
        AUDIT_BROADCAST_CAPACITY,
        Arc::new(broker.publisher()),
    ));

    // Server B — independent store; receives external records via
    // `publish_external` after the relay forwards them.
    let dir_b = TempDir::new().unwrap();
    let store_b = Arc::new(MnemosyneStore::open(dir_b.path()).unwrap());
    let handler_b = Arc::new(ProposalHandler::new(Arc::clone(&store_b)));

    // Subscribe directly to server B's broadcast — stands in for the
    // gRPC `subscribe_audit_trail` flow which is independently pinned
    // by the Round 103 live-tail smoke.
    let mut observer_rx = handler_b.audit().subscribe();

    // Bridge: relay broker records into server B's local broadcast.
    // The relay task holds an Arc<ProposalHandler> alive so the
    // appender's broadcast_tx stays open.
    let mut broker_rx = broker.subscribe();
    let relay_target = Arc::clone(&handler_b);
    let relay = tokio::spawn(async move {
        loop {
            match broker_rx.recv().await {
                Ok(record) => relay_target.audit().publish_external(record),
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
            }
        }
    });

    // Commit on server A — fanout publishes to broker, relay forwards
    // to server B's broadcast, observer_rx receives.
    handler_a
        .audit()
        .append_accepted(&entity_create("p-cross-1", 1), &[])
        .expect("commit on A");

    let received: AuditRecord = tokio::time::timeout(Duration::from_secs(2), observer_rx.recv())
        .await
        .expect("cross-process delivery timed out")
        .expect("broadcast recv ok");
    assert_eq!(received.proposal_id, "p-cross-1");
    assert!(received.accepted);

    // §6 framing: server B's RocksDB audit CF holds NO records — the
    // cross-process relay path never writes to the local store.
    let rocks_records = handler_b.audit().iter_from(0).expect("iter B");
    assert!(
        rocks_records.is_empty(),
        "cross-process relay must not write to observer's local audit CF; \
  found {} records",
        rocks_records.len()
    );

    drop(broker); // close the broker; relay broker_rx eventually sees Closed
    relay.abort(); // forcefully end the relay (the publisher held by
                   // handler_a still owns a Sender clone, so without
                   // dropping handler_a the broker_rx never signals
                   // Closed organically — abort is the deterministic
                   // teardown for the test).
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn publish_external_does_not_touch_local_audit_cf() {
    // Standalone proof of the §6 framing invariant: `publish_external`
    // pushes onto the local broadcast for tail subscribers but writes
    // nothing to the RocksDB audit CF. A subsequent `iter_from` confirms
    // the audit ledger is empty.
    let dir = TempDir::new().unwrap();
    let store = Arc::new(MnemosyneStore::open(dir.path()).unwrap());
    let handler = Arc::new(ProposalHandler::new(store));

    let mut rx = handler.audit().subscribe();
    handler.audit().publish_external(AuditRecord {
        transaction_id: 999,
        proposal_id: "external-only".into(),
        actor: "remote-host".into(),
        accepted: true,
        gate_routing_reason: "accept".into(),
        rejection_reason: None,
        proposal_kind_tag: "entity_create".into(),
        trace_id: None,
        tracestate: None,
    });

    let received = tokio::time::timeout(Duration::from_millis(500), rx.recv())
        .await
        .expect("subscribe must observe externally-pushed record")
        .expect("recv ok");
    assert_eq!(received.transaction_id, 999);
    assert_eq!(received.proposal_id, "external-only");

    let rocks_records = handler.audit().iter_from(0).expect("iter");
    assert!(
        rocks_records.is_empty(),
        "publish_external must not touch the RocksDB audit CF"
    );
}
