//! gRPC tracestate smoke test (Round 104).
//!
//! Exercises the W3C `tracestate` header propagation path introduced as an extension
//! 104:
//! - When a client supplies a `tracestate` header, the audit record on disk
//! MUST carry that header verbatim under `AuditRecord.tracestate`.
//! - When no `tracestate` is present, the audit record MUST carry
//! `tracestate = None` (no server-minted fallback — there is no equivalent
//! of vendor state to invent on the server side).
//! - Pre-Round 104 audit records (without the `tracestate` field on disk) MUST
//! decode cleanly as `tracestate = None` — backwards-compat guarantee.
//!
//! The nested-span hierarchy assertion (handler emits `gate.evaluate` and
//! `audit.append` child spans) lives in the dedicated binary
//! `handler_span_hierarchy_smoke.rs` (Round 153 carry — split for
//! process-level tracing-dispatcher isolation).

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

const SAMPLE_TRACESTATE: &str = "vendor1=value1,vendor2=value2";

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
        actor: "tracestate-tester".into(),
        kind: ProposalKind::EntityCreate {
            entity_type: "Section".into(),
            branch_id: 1,
            entity_id: 11,
            valid_from: 1000,
            payload: b"tracestate-payload".to_vec(),
        },
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tracestate_header_propagates_into_audit_record_verbatim() {
    let (_dir, handler) = fresh_handler();
    let handler_for_read = Arc::clone(&handler);
    let (addr, shutdown_tx, server) = bring_up_server(handler).await;
    let endpoint = format!("http://{}", addr);
    let mut client = MnemosyneClient::connect(endpoint).await.expect("connect");

    let mut req = tonic::Request::new(encode_proposal(entity_create_proposal("p-tracestate-001")));
    req.metadata_mut().insert(
        "tracestate",
        MetadataValue::try_from(SAMPLE_TRACESTATE).expect("metadata value"),
    );
    let response = client
        .submit_proposal(req)
        .await
        .expect("submit")
        .into_inner();
    assert!(response.accepted);
    let txn_id = response.audit_transaction_id;

    let record = handler_for_read
        .audit()
        .read(txn_id)
        .expect("audit read")
        .expect("audit record present");
    assert_eq!(
        record.tracestate.as_deref(),
        Some(SAMPLE_TRACESTATE),
        "audit tracestate must mirror inbound tracestate verbatim"
    );

    shutdown_tx.send(()).ok();
    server.await.ok();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn missing_tracestate_records_none_no_server_fallback() {
    // Round 104 — unlike trace_id (which falls back to a UUID when no
    // `traceparent` is supplied), tracestate has no server-minted fallback.
    // An RPC without the header MUST land on disk with tracestate = None.
    let (_dir, handler) = fresh_handler();
    let handler_for_read = Arc::clone(&handler);
    let (addr, shutdown_tx, server) = bring_up_server(handler).await;
    let endpoint = format!("http://{}", addr);
    let mut client = MnemosyneClient::connect(endpoint).await.expect("connect");

    let response = client
        .submit_proposal(encode_proposal(entity_create_proposal("p-tracestate-none")))
        .await
        .expect("submit")
        .into_inner();
    let record = handler_for_read
        .audit()
        .read(response.audit_transaction_id)
        .expect("audit read")
        .expect("audit record present");
    assert!(
        record.tracestate.is_none(),
        "absent inbound tracestate must record None on disk (no server fallback)"
    );
    // trace_id still gets a UUID fallback — verify the two propagation paths
    // are independent (Round 99 carry stable beside Round 104).
    assert!(
        record.trace_id.is_some(),
        "trace_id UUID fallback must remain independent of tracestate path"
    );

    shutdown_tx.send(()).ok();
    server.await.ok();
}

#[test]
fn pre_round_104_audit_record_decodes_with_tracestate_none() {
    // Backwards-compat guarantee: an audit record serialized BEFORE Round 104
    // (no `tracestate` field in the JSON payload) must decode cleanly with
    // `tracestate = None`. Simulate by writing a Round 99-shape JSON payload
    // (with trace_id, without tracestate) directly to the audit CF and
    // reading it back through the appender.
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
    "proposal_kind_tag": "entity_create",
    "trace_id": "deadbeefdeadbeefdeadbeefdeadbeef"
    });
    let bytes = serde_json::to_vec(&legacy_payload).expect("legacy serialize");
    store
        .put(CfId::Audit, 0, 0, 42, &bytes)
        .expect("legacy put");

    let recovered: AuditRecord = appender.read(42).expect("read").expect("present");
    assert_eq!(recovered.transaction_id, 42);
    assert_eq!(
        recovered.trace_id.as_deref(),
        Some("deadbeefdeadbeefdeadbeefdeadbeef"),
        "Round 99 trace_id decode carry stable"
    );
    assert!(
        recovered.tracestate.is_none(),
        "Round 104 tracestate must decode None for pre-Round 104 records"
    );
}
