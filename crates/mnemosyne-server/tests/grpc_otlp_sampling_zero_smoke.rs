//! gRPC OTLP zero-rate sampling smoke (Round 106).
//!
//! Substantiates that `OtlpExporterConfig::with_sampling_rate(0.0)` drops
//! every span — the trace-id-ratio sampler at rate 0 must produce zero
//! `ResourceSpans` deliveries to the collector for any number of submitted
//! RPCs. Lives in its own integration test binary so the global
//! `tracing_subscriber` install does not conflict with the rate=1.0 smoke
//! test (`grpc_otlp_config_smoke.rs`).

#![cfg(feature = "otlp")]

use mnemosyne_server::grpc::{
    encode_proposal, init_otlp_tracing_subscriber_with_config, MnemosyneClient,
    MnemosyneGrpcService, OtlpExporterConfig,
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
use tonic::transport::Server;

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

fn entity_create_proposal(id: &str, entity_id: u64) -> Proposal {
    Proposal {
        proposal_id: id.into(),
        actor: "otlp-zero-tester".into(),
        kind: ProposalKind::EntityCreate {
            entity_type: "Section".into(),
            branch_id: 1,
            entity_id,
            valid_from: 3000,
            payload: b"otlp-zero-payload".to_vec(),
        },
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn zero_rate_sampling_drops_all_spans() {
    let (collector_addr, received, col_shutdown_tx, col_server) = bring_up_mock_collector().await;
    let collector_endpoint = format!("http://{}", collector_addr);

    let cfg = OtlpExporterConfig::new(collector_endpoint)
        .with_sampling_rate(0.0)
        .with_batch_scheduled_delay(Duration::from_millis(100))
        .with_batch_max_export_batch_size(8);
    let guard =
        init_otlp_tracing_subscriber_with_config(cfg).expect("OTLP subscriber must initialize");

    let (_dir, handler) = fresh_handler();
    let (m_addr, m_shutdown_tx, m_server) = bring_up_mnemosyne_server(handler).await;
    let mut client = MnemosyneClient::connect(format!("http://{}", m_addr))
        .await
        .expect("mnemosyne client connect");

    // Submit a handful of proposals. With sampling_rate=0.0 the SDK drops
    // every emitted span at the sampler boundary — none reach the OTLP
    // exporter, none reach the collector.
    for i in 0..5u64 {
        client
            .submit_proposal(encode_proposal(entity_create_proposal(
                &format!("p-otlp-zero-{i}"),
                i + 1,
            )))
            .await
            .expect("submit");
    }

    drop(guard);
    tokio::time::sleep(Duration::from_millis(300)).await;

    let captured = received.lock().expect("collector mutex");
    let total_spans: usize = captured
        .iter()
        .flat_map(|rs| rs.scope_spans.iter().map(|ss| ss.spans.len()))
        .sum();
    assert_eq!(
 total_spans, 0,
 "sampling_rate=0.0 must drop every span; collector observed {total_spans} spans across {} ResourceSpans batches",
 captured.len()
 );

    drop(captured);
    m_shutdown_tx.send(()).ok();
    m_server.await.ok();
    col_shutdown_tx.send(()).ok();
    col_server.await.ok();
}
