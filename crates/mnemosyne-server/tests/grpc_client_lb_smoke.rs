//! gRPC client-side load balancing smoke (Round 108).
//!
//! Brings up two independent Mnemosyne servers on separate ephemeral ports
//! (each with its own `ProposalHandler` and store), constructs a single
//! tonic `Channel` via [`mnemosyne_server::grpc::balanced_channel`] that
//! round-robins between them, and submits N proposals. Asserts both stores
//! observe at least one proposal — i.e. RPC traffic genuinely fans out
//! across the LB targets.
//!
//! Default-feature integration test (no `--features` gate); load balancing
//! works on the plain HTTP/2 channel that Round 91 already ships.

use mnemosyne_server::grpc::{
    balanced_channel, encode_proposal, MnemosyneClient, MnemosyneGrpcService,
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

fn entity_create_proposal(id: &str, entity_id: u64) -> Proposal {
    Proposal {
        proposal_id: id.into(),
        actor: "lb-tester".into(),
        kind: ProposalKind::EntityCreate {
            entity_type: "Section".into(),
            branch_id: 1,
            entity_id,
            valid_from: 5000 + entity_id,
            payload: b"lb-payload".to_vec(),
        },
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn balanced_channel_distributes_rpcs_across_two_servers() {
    // 1. Bring up two independent servers, each with its own store.
    let (_dir_a, handler_a) = fresh_handler();
    let (_dir_b, handler_b) = fresh_handler();
    let handler_a_for_audit = Arc::clone(&handler_a);
    let handler_b_for_audit = Arc::clone(&handler_b);
    let (addr_a, shutdown_a, server_a) = bring_up_server(handler_a).await;
    let (addr_b, shutdown_b, server_b) = bring_up_server(handler_b).await;

    // 2. Client channel that load-balances across both endpoints.
    let endpoints = vec![format!("http://{addr_a}"), format!("http://{addr_b}")];
    let channel = balanced_channel(endpoints).expect("balanced channel build");
    let mut client = MnemosyneClient::new(channel);

    // 3. Submit enough proposals that round-robin must touch both servers.
    // 20 RPCs is well above the 2-server threshold and tolerates an
    // initial warm-up imbalance.
    for i in 0..20u64 {
        let r = client
            .submit_proposal(encode_proposal(entity_create_proposal(
                &format!("p-lb-{i}"),
                i + 1,
            )))
            .await
            .expect("submit_proposal over balanced channel")
            .into_inner();
        assert!(r.accepted);
    }

    // 4. Both stores must have at least one accepted proposal — i.e. the
    // LB genuinely fanned traffic out, did not pin to a single server.
    let audit_a = handler_a_for_audit.audit().iter_from(0).expect("audit a");
    let audit_b = handler_b_for_audit.audit().iter_from(0).expect("audit b");
    assert!(
        !audit_a.is_empty(),
        "server A must observe at least one RPC under round-robin LB"
    );
    assert!(
        !audit_b.is_empty(),
        "server B must observe at least one RPC under round-robin LB"
    );
    assert_eq!(
        audit_a.len() + audit_b.len(),
        20,
        "every submitted proposal must land on exactly one server"
    );

    shutdown_a.send(()).ok();
    shutdown_b.send(()).ok();
    server_a.await.ok();
    server_b.await.ok();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn balanced_channel_rejects_all_invalid_endpoints() {
    let result = balanced_channel(vec![
        "not://a valid url".to_string(),
        "::malformed::".to_string(),
    ]);
    assert!(
        result.is_err(),
        "balanced_channel must Err when no endpoint parses"
    );
}
