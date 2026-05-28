//! gRPC trace_id propagation smoke test (Round 99).
//!
//! Exercises the W3C `traceparent` propagation path introduced in Round 99:
//! - When a client supplies a valid `traceparent` header, the audit record on
//! disk MUST carry the same 32-hex trace-id.
//! - When no `traceparent` is present, the server MUST mint a UUID-shaped
//! fallback trace_id (non-empty, distinct between two requests).
//! - Pre-Round 99 audit records (which have no `trace_id` field on disk) MUST
//! decode cleanly as `trace_id = None` — backwards-compat guarantee.

use mnemosyne_server::audit::{AuditAppender, AuditRecord};
use mnemosyne_server::grpc::{encode_proposal, MnemosyneClient, MnemosyneGrpcService};
use mnemosyne_server::handler::ProposalHandler;
use mnemosyne_server::proposal::{Proposal, ProposalKind};
use mnemosyne_store::{CfId, MnemosyneStore};
use std::net::SocketAddr;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::metadata::MetadataValue;
use tonic::transport::Server;

const SAMPLE_TRACEPARENT: &str = "00-0123456789abcdef0123456789abcdef-0123456789abcdef-01";
const SAMPLE_TRACE_ID: &str = "0123456789abcdef0123456789abcdef";

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

fn entity_create_proposal(id: &str) -> Proposal {
    Proposal {
        proposal_id: id.into(),
        actor: "trace-tester".into(),
        kind: ProposalKind::EntityCreate {
            entity_type: "Section".into(),
            branch_id: 1,
            entity_id: 11,
            valid_from: 1000,
            payload: b"trace-payload".to_vec(),
        },
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn traceparent_header_propagates_into_audit_record_trace_id() {
    let (_dir, handler) = fresh_handler();
    let handler_for_read = Arc::clone(&handler);
    let (addr, shutdown_tx, server) = bring_up_server(handler).await;
    let endpoint = format!("http://{}", addr);
    let mut client = MnemosyneClient::connect(endpoint).await.expect("connect");

    let mut req = tonic::Request::new(encode_proposal(entity_create_proposal("p-trace-001")));
    req.metadata_mut().insert(
        "traceparent",
        MetadataValue::try_from(SAMPLE_TRACEPARENT).expect("metadata value"),
    );
    let response = client
        .submit_proposal(req)
        .await
        .expect("submit")
        .into_inner();
    assert!(response.accepted);
    let txn_id = response.audit_transaction_id;
    assert_eq!(txn_id, 1);

    let record = handler_for_read
        .audit()
        .read(txn_id)
        .expect("audit read")
        .expect("audit record present");
    assert_eq!(
        record.trace_id.as_deref(),
        Some(SAMPLE_TRACE_ID),
        "audit trace_id must mirror the inbound traceparent trace-id"
    );

    shutdown_tx.send(()).ok();
    server.await.ok();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn missing_traceparent_falls_back_to_server_minted_uuid() {
    let (_dir, handler) = fresh_handler();
    let handler_for_read = Arc::clone(&handler);
    let (addr, shutdown_tx, server) = bring_up_server(handler).await;
    let endpoint = format!("http://{}", addr);
    let mut client = MnemosyneClient::connect(endpoint).await.expect("connect");

    let resp1 = client
        .submit_proposal(encode_proposal(entity_create_proposal(
            "p-trace-fallback-1",
        )))
        .await
        .expect("submit 1")
        .into_inner();
    let resp2 = client
        .submit_proposal(encode_proposal(entity_create_proposal(
            "p-trace-fallback-2",
        )))
        .await
        .expect("submit 2")
        .into_inner();

    let audit = handler_for_read.audit();
    let r1 = audit
        .read(resp1.audit_transaction_id)
        .expect("audit read")
        .expect("audit record present");
    let r2 = audit
        .read(resp2.audit_transaction_id)
        .expect("audit read")
        .expect("audit record present");

    let t1 = r1.trace_id.expect("fallback trace_id minted");
    let t2 = r2.trace_id.expect("fallback trace_id minted");
    assert_ne!(
        t1, t2,
        "two requests without traceparent must mint distinct ids"
    );
    // UUID v4 string is 36 chars (32 hex + 4 dashes); also not empty.
    assert_eq!(t1.len(), 36);
    assert_eq!(t2.len(), 36);

    shutdown_tx.send(()).ok();
    server.await.ok();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pre_round_99_audit_record_decodes_with_trace_id_none() {
    // Backwards-compat guarantee: an audit record serialized BEFORE Round 99
    // (no `trace_id` field in the JSON payload) must decode cleanly with
    // `trace_id = None`. Simulate by writing a legacy JSON payload directly
    // to the audit CF and reading it back through the appender.
    let dir = TempDir::new().unwrap();
    let store = Arc::new(MnemosyneStore::open(dir.path()).unwrap());
    let appender = AuditAppender::new(Arc::clone(&store));

    let legacy_payload = serde_json::json!({
    "transaction_id": 42u64,
    "proposal_id": "legacy-p",
    "actor": "legacy-actor",
    "accepted": true,
    "gate_routing_reason": "accept",
    "rejection_reason": null,
    "proposal_kind_tag": "entity_create"
    });
    let bytes = serde_json::to_vec(&legacy_payload).expect("legacy serialize");
    store
        .put(CfId::Audit, 0, 0, 42, &bytes)
        .expect("legacy put");

    let recovered: AuditRecord = appender.read(42).expect("read").expect("present");
    assert_eq!(recovered.transaction_id, 42);
    assert_eq!(recovered.proposal_id, "legacy-p");
    assert!(recovered.accepted);
    assert!(
        recovered.trace_id.is_none(),
        "legacy audit record must decode trace_id = None"
    );
}
