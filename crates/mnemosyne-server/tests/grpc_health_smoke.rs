//! gRPC health-check service smoke test (Round 96).
//!
//! Brings up the Mnemosyne gRPC server with the standard `grpc.health.v1.Health`
//! service registered + the Mnemosyne service marked `SERVING`. A health
//! client probes both the empty-service overall status and the Mnemosyne
//! service-specific status; both must report `SERVING`.

use mnemosyne_server::grpc::{build_health_service, MnemosyneGrpcService};
use mnemosyne_server::handler::ProposalHandler;
use mnemosyne_store::MnemosyneStore;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;
use tonic_health::pb::health_check_response::ServingStatus;
use tonic_health::pb::health_client::HealthClient;
use tonic_health::pb::HealthCheckRequest;

fn fresh_handler() -> (TempDir, Arc<ProposalHandler>) {
 let dir = TempDir::new().unwrap();
 let store = Arc::new(MnemosyneStore::open(dir.path()).unwrap());
 let handler = Arc::new(ProposalHandler::new(store));
 (dir, handler)
}

async fn bring_up_server_with_health() -> Result<
 (
 std::net::SocketAddr,
 oneshot::Sender<()>,
 tokio::task::JoinHandle<()>,
 TempDir,
 ),
 Box<dyn std::error::Error>,
> {
 let listener = TcpListener::bind("127.0.0.1:0").await?;
 let addr = listener.local_addr()?;
 let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

 let (dir, handler) = fresh_handler();
 let svc = MnemosyneGrpcService::new(handler);
 let (_reporter, health_service) = build_health_service().await;

 let server = tokio::spawn(async move {
 Server::builder()
 .add_service(svc.into_server())
 .add_service(health_service)
 .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async move {
  shutdown_rx.await.ok();
 })
 .await
 .expect("server run");
 });

 Ok((addr, shutdown_tx, server, dir))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn health_check_overall_returns_serving() {
 let (addr, shutdown_tx, server, _dir) =
 bring_up_server_with_health().await.expect("server bring-up");
 let endpoint = format!("http://{}", addr);
 let channel = tonic::transport::Channel::from_shared(endpoint)
 .expect("endpoint parse")
 .connect()
 .await
 .expect("channel connect");
 let mut client = HealthClient::new(channel);

 // Empty service name = overall server status. tonic-health reports
 // SERVING by default once any service is registered.
 let resp = client
 .check(HealthCheckRequest {
 service: String::new(),
 })
 .await
 .expect("health check");
 assert_eq!(resp.into_inner().status(), ServingStatus::Serving);

 shutdown_tx.send(()).ok();
 server.await.ok();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn health_check_mnemosyne_service_returns_serving() {
 let (addr, shutdown_tx, server, _dir) =
 bring_up_server_with_health().await.expect("server bring-up");
 let endpoint = format!("http://{}", addr);
 let channel = tonic::transport::Channel::from_shared(endpoint)
 .expect("endpoint parse")
 .connect()
 .await
 .expect("channel connect");
 let mut client = HealthClient::new(channel);

 // Service-specific check — tonic-health uses the proto service path
 // (package.Service) as the registered name when called with the
 // typed `set_serving::<MnemosyneServer<...>>()` API.
 let resp = client
 .check(HealthCheckRequest {
 service: "mnemosyne.v1.Mnemosyne".to_string(),
 })
 .await
 .expect("health check");
 assert_eq!(resp.into_inner().status(), ServingStatus::Serving);

 shutdown_tx.send(()).ok();
 server.await.ok();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn health_check_unknown_service_returns_not_found() {
 let (addr, shutdown_tx, server, _dir) =
 bring_up_server_with_health().await.expect("server bring-up");
 let endpoint = format!("http://{}", addr);
 let channel = tonic::transport::Channel::from_shared(endpoint)
 .expect("endpoint parse")
 .connect()
 .await
 .expect("channel connect");
 let mut client = HealthClient::new(channel);

 // Unknown service path → tonic-health returns NotFound status code.
 let err = client
 .check(HealthCheckRequest {
 service: "nonexistent.Service".to_string(),
 })
 .await
 .expect_err("unknown service must error");
 assert_eq!(err.code(), tonic::Code::NotFound);

 shutdown_tx.send(()).ok();
 server.await.ok();
}
