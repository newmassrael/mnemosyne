//! gRPC TLS smoke test (Round 97).
//!
//! Brings up the Mnemosyne gRPC server with a tonic `ServerTlsConfig` built
//! from a self-signed cert (rcgen), then drives the server with a TLS-enabled
//! client whose `ClientTlsConfig` trusts the same cert as a CA. Asserts that a
//! `SubmitProposal` round-trip succeeds end-to-end over an encrypted channel.
//!
//! Requires `--features tls` — the entire file is gated behind the feature so
//! default builds carry no rustls compile cost.

#![cfg(feature = "tls")]

use mnemosyne_server::grpc::{
    encode_proposal, install_default_crypto_provider, server_tls_config, tls_identity_from_pem,
    MnemosyneClient, MnemosyneGrpcService,
};
use mnemosyne_server::handler::ProposalHandler;
use mnemosyne_server::proposal::{Proposal, ProposalKind};
use mnemosyne_store::MnemosyneStore;
use rcgen::{generate_simple_self_signed, CertifiedKey};
use std::sync::Arc;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::{Certificate, Channel, ClientTlsConfig, Server};

fn fresh_handler() -> (TempDir, Arc<ProposalHandler>) {
    let dir = TempDir::new().unwrap();
    let store = Arc::new(MnemosyneStore::open(dir.path()).unwrap());
    let handler = Arc::new(ProposalHandler::new(store));
    (dir, handler)
}

struct CertPair {
    cert_pem: String,
    key_pem: String,
}

fn fresh_self_signed_cert() -> CertPair {
    let CertifiedKey { cert, key_pair } =
        generate_simple_self_signed(vec!["localhost".to_string()]).expect("self-signed cert");
    CertPair {
        cert_pem: cert.pem(),
        key_pem: key_pair.serialize_pem(),
    }
}

fn entity_create_proposal(id: &str) -> Proposal {
    Proposal {
        proposal_id: id.into(),
        actor: "tls-tester".into(),
        kind: ProposalKind::EntityCreate {
            entity_type: "Section".into(),
            branch_id: 1,
            entity_id: 7,
            valid_from: 1000,
            payload: b"tls-payload".to_vec(),
        },
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn grpc_tls_handshake_round_trips_submit_proposal() {
    install_default_crypto_provider();

    let cert = fresh_self_signed_cert();
    let (_dir, handler) = fresh_handler();

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    let svc = MnemosyneGrpcService::new(handler);
    let identity = tls_identity_from_pem(cert.cert_pem.as_bytes(), cert.key_pem.as_bytes());
    let tls = server_tls_config(identity);

    let server = tokio::spawn(async move {
        Server::builder()
            .tls_config(tls)
            .expect("server tls config")
            .add_service(svc.into_server())
            .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async move {
                shutdown_rx.await.ok();
            })
            .await
            .expect("server run");
    });

    let ca = Certificate::from_pem(cert.cert_pem.as_bytes());
    let client_tls = ClientTlsConfig::new()
        .ca_certificate(ca)
        .domain_name("localhost");

    let endpoint = format!("https://localhost:{}", addr.port());
    let channel = Channel::from_shared(endpoint)
        .expect("endpoint parse")
        .tls_config(client_tls)
        .expect("client tls config")
        .connect()
        .await
        .expect("tls handshake");
    let mut client = MnemosyneClient::new(channel);

    let proposal = entity_create_proposal("p-tls-001");
    let response = client
        .submit_proposal(encode_proposal(proposal))
        .await
        .expect("submit_proposal over tls")
        .into_inner();

    assert!(
        response.accepted,
        "TLS path must accept the well-formed proposal"
    );
    assert!(
        response.audit_transaction_id_set,
        "audit_transaction_id flag must be set on accept"
    );
    assert_eq!(
        response.audit_transaction_id, 1,
        "first audit txn on a fresh store"
    );
    assert_eq!(response.proposal_id, "p-tls-001");

    shutdown_tx.send(()).ok();
    server.await.ok();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn grpc_tls_client_without_ca_rejects_handshake() {
    install_default_crypto_provider();

    let cert = fresh_self_signed_cert();
    let (_dir, handler) = fresh_handler();

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    let svc = MnemosyneGrpcService::new(handler);
    let identity = tls_identity_from_pem(cert.cert_pem.as_bytes(), cert.key_pem.as_bytes());
    let tls = server_tls_config(identity);

    let server = tokio::spawn(async move {
        Server::builder()
            .tls_config(tls)
            .expect("server tls config")
            .add_service(svc.into_server())
            .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async move {
                shutdown_rx.await.ok();
            })
            .await
            .expect("server run");
    });

    // Client trusts no CA → handshake must fail (the self-signed server cert
    // is not in the platform trust store either, since rustls without an
    // explicit trust anchor refuses all peer certs).
    let client_tls = ClientTlsConfig::new().domain_name("localhost");
    let endpoint = format!("https://localhost:{}", addr.port());
    let connect_result = Channel::from_shared(endpoint)
        .expect("endpoint parse")
        .tls_config(client_tls)
        .expect("client tls config")
        .connect()
        .await;

    assert!(
        connect_result.is_err(),
        "client without CA must reject self-signed server cert"
    );

    shutdown_tx.send(()).ok();
    server.await.ok();
}
