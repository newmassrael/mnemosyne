//! gRPC mTLS + cert rotation smoke (Round 107).
//!
//! Substantiates three facets of the Round 107 mTLS + rotation surface:
//! 1. **Matched client cert.** Server requires `client_ca_root`; client
//! presenting a cert chained to that CA completes the handshake and
//! a `submit_proposal` round-trips.
//! 2. **Missing client cert.** Same server config; client without a cert
//! identity fails the TLS handshake with a `Connect`-class error.
//! 3. **Cert rotation.** A `TlsIdentityRotator` swaps the server's
//! `ServerConfig` to a freshly issued identity at runtime; the new
//! handshake succeeds against the rotated cert.
//!
//! Self-signed CAs / leaf identities are produced with `rcgen`; everything
//! lives inside the `tls` feature so default builds do not pull rustls.

#![cfg(feature = "tls")]

use mnemosyne_server::grpc::{
 build_rustls_server_config, encode_proposal, install_default_crypto_provider,
 server_tls_config_mtls, spawn_rotating_tls_acceptor, tls_identity_from_pem,
 MnemosyneClient, MnemosyneGrpcService, TlsIdentityRotator,
};
use mnemosyne_server::handler::ProposalHandler;
use mnemosyne_server::proposal::{Proposal, ProposalKind};
use mnemosyne_store::MnemosyneStore;
use rcgen::{
 BasicConstraints, Certificate as RcgenCert, CertificateParams, IsCa, KeyPair, KeyUsagePurpose,
};
use std::sync::Arc;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::{Certificate, Channel, ClientTlsConfig, Identity, Server};

fn fresh_handler() -> (TempDir, Arc<ProposalHandler>) {
 let dir = TempDir::new().unwrap();
 let store = Arc::new(MnemosyneStore::open(dir.path()).unwrap());
 let handler = Arc::new(ProposalHandler::new(store));
 (dir, handler)
}

fn entity_create_proposal(id: &str, entity_id: u64) -> Proposal {
 Proposal {
 proposal_id: id.into(),
 actor: "mtls-tester".into(),
 kind: ProposalKind::EntityCreate {
 entity_type: "Section".into(),
 branch_id: 1,
 entity_id,
 valid_from: 4000,
 payload: b"mtls-payload".to_vec(),
 },
 }
}

/// PEM-encoded self-signed CA + a leaf identity signed by it. Used as both
/// the server identity (chained to `ca_pem`) and as a client identity in the
/// mTLS happy-path test.
struct SignedIdentity {
 ca_pem: String,
 leaf_cert_pem: String,
 leaf_key_pem: String,
}

fn issue_signed_identity(common_name: &str) -> SignedIdentity {
 // Build a self-signed CA.
 let mut ca_params = CertificateParams::new(vec!["mnemosyne-test-ca".to_string()])
 .expect("ca params");
 ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
 ca_params.key_usages = vec![
 KeyUsagePurpose::KeyCertSign,
 KeyUsagePurpose::CrlSign,
 KeyUsagePurpose::DigitalSignature,
 ];
 let ca_key = KeyPair::generate().expect("ca key");
 let ca_cert: RcgenCert = ca_params.self_signed(&ca_key).expect("ca self-sign");

 // Leaf identity signed by the CA.
 let leaf_params = CertificateParams::new(vec![common_name.to_string()]).expect("leaf params");
 let leaf_key = KeyPair::generate().expect("leaf key");
 let leaf_cert = leaf_params
 .signed_by(&leaf_key, &ca_cert, &ca_key)
 .expect("leaf sign");

 SignedIdentity {
 ca_pem: ca_cert.pem(),
 leaf_cert_pem: leaf_cert.pem(),
 leaf_key_pem: leaf_key.serialize_pem(),
 }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mtls_with_matching_client_cert_succeeds() {
 install_default_crypto_provider();

 let server_id = issue_signed_identity("localhost");
 let client_id = issue_signed_identity("mnemosyne-test-client");
 // Server trusts the CA that signed the client identity; we re-use the
 // *server's* CA as the client trust root for simplicity (a single CA
 // signs both sides — typical for in-cluster mTLS).
 let server_ca_pem = server_id.ca_pem.clone();
 let trusted_client_ca_pem = client_id.ca_pem.clone();

 let (_dir, handler) = fresh_handler();

 let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
 let addr = listener.local_addr().expect("local_addr");
 let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

 let svc = MnemosyneGrpcService::new(handler);
 let identity = tls_identity_from_pem(
 server_id.leaf_cert_pem.as_bytes(),
 server_id.leaf_key_pem.as_bytes(),
 );
 let tls = server_tls_config_mtls(identity, trusted_client_ca_pem.as_bytes());

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

 let server_ca = Certificate::from_pem(server_ca_pem.as_bytes());
 let client_identity = Identity::from_pem(
 client_id.leaf_cert_pem.as_bytes(),
 client_id.leaf_key_pem.as_bytes(),
 );
 let client_tls = ClientTlsConfig::new()
 .ca_certificate(server_ca)
 .identity(client_identity)
 .domain_name("localhost");
 let endpoint = format!("https://localhost:{}", addr.port());
 let channel = Channel::from_shared(endpoint)
 .expect("endpoint parse")
 .tls_config(client_tls)
 .expect("client tls config")
 .connect()
 .await
 .expect("mtls handshake");
 let mut client = MnemosyneClient::new(channel);

 let response = client
 .submit_proposal(encode_proposal(entity_create_proposal("p-mtls-001", 1)))
 .await
 .expect("submit_proposal over mtls")
 .into_inner();
 assert!(response.accepted);

 shutdown_tx.send(()).ok();
 server.await.ok();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mtls_without_client_cert_rejects_handshake() {
 install_default_crypto_provider();

 let server_id = issue_signed_identity("localhost");
 let separate_ca = issue_signed_identity("expected-client-ca");

 let (_dir, handler) = fresh_handler();

 let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
 let addr = listener.local_addr().expect("local_addr");
 let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

 let svc = MnemosyneGrpcService::new(handler);
 let identity = tls_identity_from_pem(
 server_id.leaf_cert_pem.as_bytes(),
 server_id.leaf_key_pem.as_bytes(),
 );
 let tls = server_tls_config_mtls(identity, separate_ca.ca_pem.as_bytes());

 let server = tokio::spawn(async move {
 Server::builder()
 .tls_config(tls)
 .expect("server tls config")
 .add_service(svc.into_server())
 .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async move {
  shutdown_rx.await.ok();
 })
 .await
 .ok();
 });

 // Client trusts the server's CA but presents NO client identity.
 let server_ca = Certificate::from_pem(server_id.ca_pem.as_bytes());
 let client_tls = ClientTlsConfig::new()
 .ca_certificate(server_ca)
 .domain_name("localhost");
 let endpoint = format!("https://localhost:{}", addr.port());
 let channel_result = Channel::from_shared(endpoint)
 .expect("endpoint parse")
 .tls_config(client_tls)
 .expect("client tls config")
 .connect()
 .await;

 // The handshake reaches the application layer (channel connect may
 // succeed because tonic / hyper defer the auth check), then the first
 // RPC fails. Either failure mode satisfies the assertion: the server
 // never accepts an unauthenticated client.
 let rpc_failed = match channel_result {
 Err(_) => true,
 Ok(channel) => {
 let mut client = MnemosyneClient::new(channel);
 client
  .submit_proposal(encode_proposal(entity_create_proposal("p-mtls-rej", 2)))
  .await
  .is_err()
 }
 };
 assert!(
 rpc_failed,
 "server requiring mTLS must refuse a connection without client cert"
 );

 shutdown_tx.send(()).ok();
 let _ = server.await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cert_rotation_swaps_server_identity_at_runtime() {
 install_default_crypto_provider();

 // Two separate self-signed identities; we rotate the server from #1 to
 // #2 mid-flight, then verify a fresh client trusting the rotated CA can
 // complete a handshake. (No mTLS in this test — rotation is independent
 // of client-auth and the simpler shape keeps the assertion focused.)
 let id_initial = issue_signed_identity("localhost");
 let id_rotated = issue_signed_identity("localhost");

 let initial_cfg = build_rustls_server_config(
 id_initial.leaf_cert_pem.as_bytes(),
 id_initial.leaf_key_pem.as_bytes(),
 None,
 )
 .expect("initial rustls config");
 let rotated_cfg = build_rustls_server_config(
 id_rotated.leaf_cert_pem.as_bytes(),
 id_rotated.leaf_key_pem.as_bytes(),
 None,
 )
 .expect("rotated rustls config");

 let (rotator, handle) = TlsIdentityRotator::new(initial_cfg);

 let (_dir, handler) = fresh_handler();
 let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
 let addr = listener.local_addr().expect("local_addr");
 let svc = MnemosyneGrpcService::new(handler);
 let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

 let acceptor_stream = spawn_rotating_tls_acceptor(listener, handle);
 let server = tokio::spawn(async move {
 Server::builder()
 .add_service(svc.into_server())
 .serve_with_incoming_shutdown(acceptor_stream, async move {
  shutdown_rx.await.ok();
 })
 .await
 .ok();
 });

 // Client trusting the *initial* CA submits one RPC successfully.
 let initial_ca = Certificate::from_pem(id_initial.ca_pem.as_bytes());
 let client_tls = ClientTlsConfig::new()
 .ca_certificate(initial_ca)
 .domain_name("localhost");
 let endpoint = format!("https://localhost:{}", addr.port());
 let channel = Channel::from_shared(endpoint.clone())
 .expect("endpoint parse")
 .tls_config(client_tls)
 .expect("client tls config")
 .connect()
 .await
 .expect("initial handshake");
 let mut initial_client = MnemosyneClient::new(channel);
 let resp = initial_client
 .submit_proposal(encode_proposal(entity_create_proposal("p-rot-pre", 1)))
 .await
 .expect("submit on initial cert")
 .into_inner();
 assert!(resp.accepted);

 // Rotate the server identity. Subsequent handshakes pick up the new cert.
 rotator
 .rotate(rotated_cfg)
 .expect("rotation must succeed");

 // Tiny grace: ensure the watch publish settles before the new handshake.
 tokio::time::sleep(std::time::Duration::from_millis(50)).await;

 // Client trusting the *rotated* CA opens a fresh channel. The
 // pre-rotation channel (initial_client) is still alive on the old TCP
 // connection — we only assert the *new* handshake succeeds.
 let rotated_ca = Certificate::from_pem(id_rotated.ca_pem.as_bytes());
 let rotated_tls = ClientTlsConfig::new()
 .ca_certificate(rotated_ca)
 .domain_name("localhost");
 let new_channel = Channel::from_shared(endpoint)
 .expect("endpoint parse")
 .tls_config(rotated_tls)
 .expect("rotated client tls config")
 .connect()
 .await
 .expect("rotated handshake");
 let mut rotated_client = MnemosyneClient::new(new_channel);
 let resp_rotated = rotated_client
 .submit_proposal(encode_proposal(entity_create_proposal("p-rot-post", 2)))
 .await
 .expect("submit on rotated cert")
 .into_inner();
 assert!(resp_rotated.accepted);

 shutdown_tx.send(()).ok();
 let _ = server.await;
}
