//! gRPC metadata-driven authentication smoke (Round 108).
//!
//! Substantiates [`mnemosyne_server::grpc::require_authorization_metadata`]:
//! a server-side interceptor that rejects any inbound RPC whose metadata
//! lacks an `authorization` header. Verifies (1) RPC without the header
//! returns `Status::unauthenticated`, (2) RPC with the header passes
//! through the interceptor and reaches the handler.
//!
//! The interceptor does not validate the *value* of the header — token /
//! signature verification is layered on top in Phase 0+ (Round 108 (d) carry).

use mnemosyne_server::grpc::{
    encode_proposal, require_authorization_metadata, MnemosyneClient, MnemosyneGrpcService,
};
use mnemosyne_server::handler::ProposalHandler;
use mnemosyne_server::proposal::{Proposal, ProposalKind};
use mnemosyne_store::MnemosyneStore;
use std::net::SocketAddr;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::metadata::MetadataValue;
use tonic::transport::Server;

fn fresh_handler() -> (TempDir, Arc<ProposalHandler>) {
    let dir = TempDir::new().unwrap();
    let store = Arc::new(MnemosyneStore::open(dir.path()).unwrap());
    let handler = Arc::new(ProposalHandler::new(store));
    (dir, handler)
}

async fn bring_up_server_with_auth(
    handler: Arc<ProposalHandler>,
) -> (SocketAddr, oneshot::Sender<()>, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let svc = MnemosyneGrpcService::new(handler);
    let intercepted = tonic::service::interceptor::InterceptedService::new(
        svc.into_server(),
        require_authorization_metadata,
    );
    let server = tokio::spawn(async move {
        Server::builder()
            .add_service(intercepted)
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
        actor: "auth-tester".into(),
        kind: ProposalKind::EntityCreate {
            entity_type: "Section".into(),
            branch_id: 1,
            entity_id: 8,
            valid_from: 6000,
            payload: b"auth-payload".to_vec(),
        },
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rpc_without_authorization_header_is_unauthenticated() {
    let (_dir, handler) = fresh_handler();
    let (addr, shutdown_tx, server) = bring_up_server_with_auth(handler).await;
    let mut client = MnemosyneClient::connect(format!("http://{addr}"))
        .await
        .expect("connect");

    let result = client
        .submit_proposal(encode_proposal(entity_create_proposal("p-auth-missing")))
        .await;

    let status = result.expect_err("must fail without authorization metadata");
    assert_eq!(
        status.code(),
        tonic::Code::Unauthenticated,
        "missing `authorization` must produce Unauthenticated, got {:?}: {}",
        status.code(),
        status.message()
    );

    shutdown_tx.send(()).ok();
    server.await.ok();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rpc_with_authorization_header_passes_interceptor() {
    let (_dir, handler) = fresh_handler();
    let (addr, shutdown_tx, server) = bring_up_server_with_auth(handler).await;
    let mut client = MnemosyneClient::connect(format!("http://{addr}"))
        .await
        .expect("connect");

    let mut req = tonic::Request::new(encode_proposal(entity_create_proposal("p-auth-ok")));
    req.metadata_mut().insert(
        "authorization",
        MetadataValue::try_from("Bearer test-token").expect("metadata value"),
    );
    let response = client
        .submit_proposal(req)
        .await
        .expect("submit must pass with authorization metadata")
        .into_inner();
    assert!(response.accepted);

    shutdown_tx.send(()).ok();
    server.await.ok();
}
