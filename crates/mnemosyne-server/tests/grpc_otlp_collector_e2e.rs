//! gRPC OTLP collector end-to-end smoke (Round 105).
//!
//! Brings up an in-process mock OTLP collector that implements the
//! `opentelemetry.proto.collector.trace.v1.TraceService` gRPC service,
//! initializes the server's `init_otlp_tracing_subscriber` against that
//! collector's address, drives a `submit_proposal` RPC through a fresh
//! Mnemosyne server, then asserts the collector observed at least one
//! `ResourceSpans` carrying span names emitted by the handler
//! (`gate.evaluate` and `audit.append` from Round 104).
//!
//! Phase 0+ deployment harness pattern — no external dependency on a real
//! `opentelemetry-collector-contrib` container; the mock is a thin tonic
//! service that records every inbound `ExportTraceServiceRequest`.
//!
//! Requires `--features otlp`. The whole test file is gated so default
//! builds carry no opentelemetry compile cost (Round 102 carry stable).

#![cfg(feature = "otlp")]

use mnemosyne_server::grpc::{
    encode_proposal, init_otlp_tracing_subscriber, MnemosyneClient, MnemosyneGrpcService,
};
use mnemosyne_server::handler::ProposalHandler;
use mnemosyne_server::proposal::{Proposal, ProposalKind};
use mnemosyne_store::MnemosyneStore;
use opentelemetry_proto::tonic::collector::trace::v1::trace_service_server::{
    TraceService, TraceServiceServer,
};
use opentelemetry_proto::tonic::collector::trace::v1::{
    ExportTraceServiceRequest, ExportTraceServiceResponse,
};
use opentelemetry_proto::tonic::trace::v1::ResourceSpans;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::metadata::MetadataValue;
use tonic::transport::Server;

/// In-process mock collector that captures every `ResourceSpans` payload
/// pushed by the OTLP exporter. The captured state is `Arc<Mutex<...>>` so
/// the test thread can read it after triggering a flush.
struct MockTraceCollector {
    received: Arc<Mutex<Vec<ResourceSpans>>>,
}

#[tonic::async_trait]
impl TraceService for MockTraceCollector {
    async fn export(
        &self,
        request: tonic::Request<ExportTraceServiceRequest>,
    ) -> Result<tonic::Response<ExportTraceServiceResponse>, tonic::Status> {
        let mut guard = self.received.lock().expect("collector mutex");
        for rs in request.into_inner().resource_spans {
            guard.push(rs);
        }
        Ok(tonic::Response::new(ExportTraceServiceResponse {
            partial_success: None,
        }))
    }
}

fn fresh_handler() -> (TempDir, Arc<ProposalHandler>) {
    let dir = TempDir::new().unwrap();
    let store = Arc::new(MnemosyneStore::open(dir.path()).unwrap());
    let handler = Arc::new(ProposalHandler::new(store));
    (dir, handler)
}

async fn bring_up_mnemosyne_server(
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
            .expect("mnemosyne server run");
    });
    (addr, shutdown_tx, server)
}

async fn bring_up_mock_collector() -> (
    SocketAddr,
    Arc<Mutex<Vec<ResourceSpans>>>,
    oneshot::Sender<()>,
    tokio::task::JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("collector bind");
    let addr = listener.local_addr().expect("collector local_addr");
    let received = Arc::new(Mutex::new(Vec::new()));
    let collector = MockTraceCollector {
        received: Arc::clone(&received),
    };
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let server = tokio::spawn(async move {
        Server::builder()
            .add_service(TraceServiceServer::new(collector))
            .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async move {
                shutdown_rx.await.ok();
            })
            .await
            .expect("mock collector run");
    });
    (addr, received, shutdown_tx, server)
}

fn entity_create_proposal(id: &str) -> Proposal {
    Proposal {
        proposal_id: id.into(),
        actor: "otlp-e2e-tester".into(),
        kind: ProposalKind::EntityCreate {
            entity_type: "Section".into(),
            branch_id: 1,
            entity_id: 7,
            valid_from: 1500,
            payload: b"otlp-e2e-payload".to_vec(),
        },
    }
}

const SAMPLE_TRACEPARENT: &str = "00-cafef00dcafef00dcafef00dcafef00d-cafef00dcafef00d-01";
const SAMPLE_TRACE_ID_HEX: &str = "cafef00dcafef00dcafef00dcafef00d";

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn end_to_end_otlp_collector_receives_handler_emitted_spans() {
    // 1. Mock OTLP collector listening on an ephemeral port.
    let (collector_addr, received, col_shutdown_tx, col_server) = bring_up_mock_collector().await;
    let collector_endpoint = format!("http://{}", collector_addr);

    // 2. Initialize the server's OTLP tracing subscriber pointing at the
    // mock collector. The guard owns the SDK provider — drop on shutdown
    // flushes pending batches synchronously.
    let guard =
        init_otlp_tracing_subscriber(&collector_endpoint).expect("OTLP subscriber must initialize");

    // 3. Bring up Mnemosyne server + client.
    let (_dir, handler) = fresh_handler();
    let (m_addr, m_shutdown_tx, m_server) = bring_up_mnemosyne_server(handler).await;
    let mut client = MnemosyneClient::connect(format!("http://{}", m_addr))
        .await
        .expect("mnemosyne client connect");

    // 4. Drive a single submit_proposal with a known traceparent so the
    // propagated trace.id surfaces as a span attribute we can grep for.
    let mut req = tonic::Request::new(encode_proposal(entity_create_proposal("p-otlp-e2e-001")));
    req.metadata_mut().insert(
        "traceparent",
        MetadataValue::try_from(SAMPLE_TRACEPARENT).expect("traceparent metadata"),
    );
    let resp = client
        .submit_proposal(req)
        .await
        .expect("submit")
        .into_inner();
    assert!(resp.accepted, "submit_proposal must succeed");

    // 5. Drop the OTLP guard to force a flush. The SDK's batch processor
    // drains pending spans through the exporter on shutdown; the mock
    // collector receives the export RPC before this thread continues.
    drop(guard);

    // 6. Allow the in-process tonic server one more tick to settle the
    // final export response. The drop-shutdown is synchronous on the
    // SDK side, but the collector's response handler runs on the tokio
    // runtime — give it a chance to commit the captured payload.
    tokio::time::sleep(Duration::from_millis(200)).await;

    // 7. Assert: the collector received at least one ResourceSpans whose
    // scope_spans carries a span named "submit_proposal" *or*
    // "gate.evaluate" / "audit.append" (Round 104 nested hierarchy).
    // The exact span names depend on tonic's auto-generated entry span
    // + the handler's instrumentation; the assertion accepts any of the
    // handler-emitted names so the test is robust to tonic span-name
    // changes across versions.
    let captured = received.lock().expect("collector mutex");
    assert!(
        !captured.is_empty(),
        "OTLP collector must receive at least one ResourceSpans batch"
    );
    let mut all_span_names: Vec<String> = Vec::new();
    let mut found_trace_id_attr = false;
    for rs in captured.iter() {
        for ss in &rs.scope_spans {
            for span in &ss.spans {
                all_span_names.push(span.name.clone());
                for attr in &span.attributes {
                    if attr.key == "trace.id" {
                        if let Some(v) = attr.value.as_ref().and_then(|av| av.value.as_ref()) {
                            // Match StringValue via debug-rendered shape so
                            // the test stays decoupled from `AnyValue`'s
                            // generated enum variant names (which differ
                            // slightly across opentelemetry-proto versions).
                            let rendered = format!("{:?}", v);
                            if rendered.contains(SAMPLE_TRACE_ID_HEX) {
                                found_trace_id_attr = true;
                            }
                        }
                    }
                }
            }
        }
    }
    let handler_emitted = ["gate.evaluate", "audit.append"];
    let has_handler_span = all_span_names
        .iter()
        .any(|n| handler_emitted.contains(&n.as_str()));
    assert!(
 has_handler_span,
 "collector must observe at least one handler-emitted child span (`gate.evaluate` or `audit.append`); names captured: {:?}",
 all_span_names
 );
    assert!(
 found_trace_id_attr,
 "at least one captured span must carry the W3C-propagated `trace.id` attribute mirroring the inbound traceparent (expected hex `{SAMPLE_TRACE_ID_HEX}`); span names: {:?}",
 all_span_names
 );

    // 8. Cleanup.
    drop(captured);
    m_shutdown_tx.send(()).ok();
    m_server.await.ok();
    col_shutdown_tx.send(()).ok();
    col_server.await.ok();
}
