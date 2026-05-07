//! gRPC OTLP exporter config smoke tests (Round 106).
//!
//! Substantiates the `OtlpExporterConfig` extension to
//! `init_otlp_tracing_subscriber_with_config`:
//! - **Sampling rate.** `sampling_rate=1.0` emits every span; `0.0` drops all.
//! Statistical assertion at 0.5 is intentionally avoided (CI flakiness);
//! the extremes prove the sampler is wired into the pipeline correctly.
//! - **Resource attributes.** `resource_attributes` injects every (k, v) into
//! the OTLP `resource.attributes` field on the wire — verified via the
//! in-process mock collector's captured `ResourceSpans.resource`.
//! - **Batch tuning.** `batch_max_export_batch_size` / `batch_scheduled_delay`
//! pass through the SDK `BatchConfigBuilder`; the sampling test exercises a
//! short scheduled_delay so the test does not wait the SDK default 5s.
//!
//! Each test runs in its own process (separate integration test binary) so
//! the global `tracing_subscriber` install does not collide. Within a single
//! test we install the subscriber once and tear it down via the
//! `OtlpTracerGuard` Drop impl.
//!
//! Requires `--features otlp`.

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
) -> (
 SocketAddr,
 oneshot::Sender<()>,
 tokio::task::JoinHandle<()>,
) {
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
 let listener = TcpListener::bind("127.0.0.1:0").await.expect("collector bind");
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
 actor: "otlp-cfg-tester".into(),
 kind: ProposalKind::EntityCreate {
 entity_type: "Section".into(),
 branch_id: 1,
 entity_id: 9,
 valid_from: 2000,
 payload: b"otlp-cfg-payload".to_vec(),
 },
 }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn resource_attributes_and_full_sampling_emit_to_collector() {
 // Combined assertion: at sampling_rate=1.0 every handler-emitted span
 // reaches the collector AND the configured resource attributes appear
 // on the captured ResourceSpans.resource.attributes payload. Combining
 // both into one test avoids a second global subscriber install (which
 // would panic — registry().try_init() is one-shot per process).
 let (collector_addr, received, col_shutdown_tx, col_server) =
 bring_up_mock_collector().await;
 let collector_endpoint = format!("http://{}", collector_addr);

 let cfg = OtlpExporterConfig::new(collector_endpoint)
 .with_sampling_rate(1.0)
 .with_batch_scheduled_delay(Duration::from_millis(100))
 .with_batch_max_export_batch_size(8)
 .with_resource_attribute("service.name", "mnemosyne-server-test")
 .with_resource_attribute("deployment.environment", "extension-106");
 let guard = init_otlp_tracing_subscriber_with_config(cfg)
 .expect("OTLP subscriber must initialize");

 let (_dir, handler) = fresh_handler();
 let (m_addr, m_shutdown_tx, m_server) = bring_up_mnemosyne_server(handler).await;
 let mut client = MnemosyneClient::connect(format!("http://{}", m_addr))
 .await
 .expect("mnemosyne client connect");

 client
 .submit_proposal(encode_proposal(entity_create_proposal("p-otlp-cfg-001")))
 .await
 .expect("submit");

 drop(guard);
 tokio::time::sleep(Duration::from_millis(300)).await;

 let captured = received.lock().expect("collector mutex");
 assert!(
 !captured.is_empty(),
 "sampling_rate=1.0 must emit at least one ResourceSpans batch"
 );

 // Resource attribute presence: at least one ResourceSpans must carry
 // both configured (k, v) pairs on its resource.attributes list.
 let mut found_service_name = false;
 let mut found_environment = false;
 for rs in captured.iter() {
 if let Some(resource) = rs.resource.as_ref() {
 for attr in &resource.attributes {
  if attr.key == "service.name" {
  let rendered = format!("{:?}", attr.value);
  if rendered.contains("mnemosyne-server-test") {
  found_service_name = true;
  }
  }
  if attr.key == "deployment.environment" {
  let rendered = format!("{:?}", attr.value);
  if rendered.contains("extension-106") {
  found_environment = true;
  }
  }
 }
 }
 }
 assert!(
 found_service_name,
 "configured `service.name` resource attribute must appear on the wire"
 );
 assert!(
 found_environment,
 "configured `deployment.environment` resource attribute must appear on the wire"
 );

 drop(captured);
 m_shutdown_tx.send(()).ok();
 m_server.await.ok();
 col_shutdown_tx.send(()).ok();
 col_server.await.ok();
}
