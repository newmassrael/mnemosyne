//! Handler nested-span hierarchy smoke test (Round 153 carry — split from
//! `grpc_tracestate_smoke.rs` for process-level isolation).
//!
//! Verifies that `ProposalHandler::handle` emits nested `gate.evaluate` and
//! `audit.append` child spans whose parent is the active span at call time.
//! A custom `tracing_subscriber::Layer` captures `(span_id → name, parent_id)`
//! tuples to assert the hierarchy.
//!
//! Round 153 — this binary is intentionally separate from
//! `grpc_tracestate_smoke.rs`. The two test bodies installed *different*
//! tracing dispatchers (this one via `tracing::subscriber::with_default`,
//! the other three via the default `NoSubscriber` while exercising the same
//! `info_span!` callsites in `handler.rs`). Running them in the same test
//! binary occasionally interleaved the callsite Interest cache and the
//! sharded-slab span Pool, producing a flaky failure where the
//! `gate.evaluate` / `audit.append` children went to the global dispatcher
//! instead of the test-local one. Splitting into a dedicated binary places a
//! process boundary between the two state spaces and removes the race.

use mnemosyne_server::handler::ProposalHandler;
use mnemosyne_server::proposal::{Proposal, ProposalKind};
use mnemosyne_store::MnemosyneStore;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;
use tracing::span::{Attributes, Id};
use tracing::Subscriber;
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

fn fresh_handler() -> (TempDir, Arc<ProposalHandler>) {
 let dir = TempDir::new().unwrap();
 let store = Arc::new(MnemosyneStore::open(dir.path()).unwrap());
 let handler = Arc::new(ProposalHandler::new(store));
 (dir, handler)
}

fn entity_create_proposal(id: &str) -> Proposal {
 Proposal {
 proposal_id: id.into(),
 actor: "span-hierarchy-tester".into(),
 kind: ProposalKind::EntityCreate {
 entity_type: "Section".into(),
 branch_id: 1,
 entity_id: 11,
 valid_from: 1000,
 payload: b"span-hierarchy-payload".to_vec(),
 },
 }
}

/// Records (span_id → name) and (span_id → parent_id) for every span
/// created during a tracing-instrumented region. Used to assert that the
/// handler emits `gate.evaluate` and `audit.append` child spans parented to
/// the current span at call time.
#[derive(Default)]
struct SpanCapture {
 by_id: HashMap<u64, (String, Option<u64>)>,
}

impl SpanCapture {
 fn names(&self) -> Vec<&str> {
 self.by_id.values().map(|(n, _)| n.as_str()).collect()
 }

 fn parent_name_of(&self, name: &str) -> Option<&str> {
 let (_, parent_id) = self.by_id.values().find(|(n, _)| n == name)?;
 let pid = (*parent_id)?;
 self.by_id.get(&pid).map(|(n, _)| n.as_str())
 }
}

struct CaptureLayer {
 capture: Arc<Mutex<SpanCapture>>,
}

impl<S> Layer<S> for CaptureLayer
where
 S: Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
 fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
 let parent_id = ctx.lookup_current().map(|s| s.id().into_u64());
 let mut g = self.capture.lock().unwrap();
 g.by_id
 .insert(id.into_u64(), (attrs.metadata().name().to_string(), parent_id));
 }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn handler_emits_nested_gate_evaluate_and_audit_append_spans() {
 use tracing_subscriber::layer::SubscriberExt as _;

 let capture = Arc::new(Mutex::new(SpanCapture::default()));
 let layer = CaptureLayer {
 capture: Arc::clone(&capture),
 };
 let subscriber = tracing_subscriber::registry().with(layer);

 let (_dir, handler) = fresh_handler();
 let p = entity_create_proposal("p-span-hierarchy");

 // Drive the embedded handler under a known parent span ("test.parent").
 // The handler is exercised directly (synchronous embedded path) so the
 // child spans land in the same thread and the lookup_current() chain
 // resolves to our parent. The gRPC propagation (parent_span.enter() in
 // spawn_blocking) is structurally identical and exercised by the
 // tracestate smoke tests in `grpc_tracestate_smoke.rs`.
 tracing::subscriber::with_default(subscriber, || {
 let parent = tracing::info_span!("test.parent");
 let _enter = parent.enter();
 let result = handler.handle(&p).expect("handle");
 assert!(result.accepted);
 });

 let g = capture.lock().unwrap();
 let names = g.names();
 assert!(
 names.contains(&"test.parent"),
 "parent test span missing — capture layer wiring broken"
 );
 assert!(
 names.contains(&"gate.evaluate"),
 "expected `gate.evaluate` child span, captured: {:?}",
 names
 );
 assert!(
 names.contains(&"audit.append"),
 "expected `audit.append` child span, captured: {:?}",
 names
 );
 assert_eq!(
 g.parent_name_of("gate.evaluate"),
 Some("test.parent"),
 "`gate.evaluate` must be parented to the active span"
 );
 assert_eq!(
 g.parent_name_of("audit.append"),
 Some("test.parent"),
 "`audit.append` must be parented to the active span"
 );
}
