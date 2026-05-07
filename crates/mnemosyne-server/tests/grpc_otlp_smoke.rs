//! gRPC OTLP smoke test (Round 102).
//!
//! Brings up the OTLP tracing pipeline against a dummy local endpoint, emits a
//! span, and verifies the helper composes the `tracing-opentelemetry` layer
//! without panicking. Span emission travels through the batch exporter on a
//! best-effort basis — this smoke test does not stand up a real OTLP collector,
//! it only proves the wiring compiles and the subscriber initializes.
//!
//! Requires `--features otlp` — the entire file is gated behind the feature so
//! default builds carry no opentelemetry compile cost (Round 99 trace_id field
//! stays a plain `String` without OTLP wiring).

#![cfg(feature = "otlp")]

use mnemosyne_server::grpc::init_otlp_tracing_subscriber;
use tokio::net::TcpListener;

/// Verifies `init_otlp_tracing_subscriber` returns Ok and installs the global
/// tracing subscriber. Emits a span with the OTLP-compatible `trace.id`
/// attribute; the assertion is that the span macro path executes without
/// panicking — the actual span flush happens asynchronously inside the
/// batch exporter and is not observable from the test boundary without a
/// real OTLP collector.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn otlp_tracing_subscriber_initializes_and_accepts_spans() {
 // Reserve a port the OS will not reuse immediately — the exporter targets
 // it but we never accept handshakes. The batch processor swallows
 // unreachable-endpoint errors silently; the helper must still return Ok.
 let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
 let addr = listener.local_addr().expect("local_addr");
 drop(listener);
 let endpoint = format!("http://127.0.0.1:{}", addr.port());

 let _guard = init_otlp_tracing_subscriber(&endpoint)
 .expect("OTLP tracing subscriber must initialize");

 // Emit a span carrying both the legacy `trace_id` field and the OTLP-
 // compatible `trace.id` attribute key (Round 99 + Round 102 propagation
 // shape) — the layer should accept both without panic.
 let span = tracing::info_span!(
 "otlp.smoke",
 trace_id = "deadbeefdeadbeefdeadbeefdeadbeef",
 "trace.id" = "deadbeefdeadbeefdeadbeefdeadbeef"
 );
 let _enter = span.enter();
 tracing::info!("otlp smoke event");
}
