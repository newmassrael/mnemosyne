//! gRPC reflection service smoke test (Round 96).
//!
//! Brings up the Mnemosyne gRPC server with the standard
//! `grpc.reflection.v1alpha.ServerReflection` service registered. A reflection
//! client requests `ListServices` and asserts the proto-defined Mnemosyne
//! service path appears in the response — confirming the FileDescriptorSet
//! emitted by `build.rs` is wired into the runtime reflection layer.

use mnemosyne_server::grpc::{build_reflection_service, MnemosyneGrpcService};
use mnemosyne_server::handler::ProposalHandler;
use mnemosyne_store::MnemosyneStore;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;

fn fresh_handler() -> (TempDir, Arc<ProposalHandler>) {
 let dir = TempDir::new().unwrap();
 let store = Arc::new(MnemosyneStore::open(dir.path()).unwrap());
 let handler = Arc::new(ProposalHandler::new(store));
 (dir, handler)
}

async fn bring_up_server_with_reflection() -> Result<
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
 let reflection_service = build_reflection_service().expect("reflection service build");

 let server = tokio::spawn(async move {
 Server::builder()
 .add_service(svc.into_server())
 .add_service(reflection_service)
 .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async move {
  shutdown_rx.await.ok();
 })
 .await
 .expect("server run");
 });

 Ok((addr, shutdown_tx, server, dir))
}

/// Drive the reflection service via raw gRPC `ServerReflection` client to
/// verify the Mnemosyne service is enumerable. Uses the v1alpha proto
/// (matching `tonic_reflection::server::Builder::build_v1alpha`).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reflection_lists_mnemosyne_service() {
 use tonic_reflection::pb::v1alpha::server_reflection_client::ServerReflectionClient;
 use tonic_reflection::pb::v1alpha::server_reflection_request::MessageRequest;
 use tonic_reflection::pb::v1alpha::server_reflection_response::MessageResponse;
 use tonic_reflection::pb::v1alpha::ServerReflectionRequest;

 let (addr, shutdown_tx, server, _dir) = bring_up_server_with_reflection()
 .await
 .expect("server bring-up");
 let endpoint = format!("http://{}", addr);
 let channel = tonic::transport::Channel::from_shared(endpoint)
 .expect("endpoint parse")
 .connect()
 .await
 .expect("channel connect");
 let mut client = ServerReflectionClient::new(channel);

 let request_stream = tokio_stream::iter([ServerReflectionRequest {
 host: String::new(),
 message_request: Some(MessageRequest::ListServices(String::new())),
 }]);
 let mut response_stream = client
 .server_reflection_info(request_stream)
 .await
 .expect("reflection info")
 .into_inner();
 let response = response_stream
 .message()
 .await
 .expect("first response")
 .expect("response present");

 let services = match response.message_response {
 Some(MessageResponse::ListServicesResponse(resp)) => resp.service,
 other => panic!("expected ListServicesResponse, got {other:?}"),
 };

 let names: Vec<&str> = services.iter().map(|s| s.name.as_str()).collect();
 assert!(
 names.contains(&"mnemosyne.v1.Mnemosyne"),
 "expected mnemosyne.v1.Mnemosyne in reflection ListServices, got {names:?}"
 );

 shutdown_tx.send(()).ok();
 server.await.ok();
}

/// Verify the reflection service can return the FileDescriptorProto for the
/// Mnemosyne service when queried by symbol — confirms the encoded descriptor
/// set baked in by `build.rs` is non-empty and routable.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reflection_returns_file_descriptor_for_mnemosyne_symbol() {
 use tonic_reflection::pb::v1alpha::server_reflection_client::ServerReflectionClient;
 use tonic_reflection::pb::v1alpha::server_reflection_request::MessageRequest;
 use tonic_reflection::pb::v1alpha::server_reflection_response::MessageResponse;
 use tonic_reflection::pb::v1alpha::ServerReflectionRequest;

 let (addr, shutdown_tx, server, _dir) = bring_up_server_with_reflection()
 .await
 .expect("server bring-up");
 let endpoint = format!("http://{}", addr);
 let channel = tonic::transport::Channel::from_shared(endpoint)
 .expect("endpoint parse")
 .connect()
 .await
 .expect("channel connect");
 let mut client = ServerReflectionClient::new(channel);

 let request_stream = tokio_stream::iter([ServerReflectionRequest {
 host: String::new(),
 message_request: Some(MessageRequest::FileContainingSymbol(
 "mnemosyne.v1.Mnemosyne".to_string(),
 )),
 }]);
 let mut response_stream = client
 .server_reflection_info(request_stream)
 .await
 .expect("reflection info")
 .into_inner();
 let response = response_stream
 .message()
 .await
 .expect("first response")
 .expect("response present");

 match response.message_response {
 Some(MessageResponse::FileDescriptorResponse(resp)) => {
 assert!(
  !resp.file_descriptor_proto.is_empty(),
  "expected at least one FileDescriptorProto for the Mnemosyne symbol"
 );
 }
 other => panic!("expected FileDescriptorResponse, got {other:?}"),
 }

 shutdown_tx.send(()).ok();
 server.await.ok();
}
