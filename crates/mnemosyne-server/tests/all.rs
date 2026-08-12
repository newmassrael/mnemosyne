//! Every integration test of this crate, in ONE binary.
//!
//! WHY. Each file under `tests/` is its own crate and its own LINK, and this
//! crate's graph is the expensive one in the workspace: RocksDB, tonic, prost,
//! rustls, opentelemetry. Twenty-two files therefore produced twenty-two
//! executables that each statically linked all of it. Measured before Round
//! 1147's profile change they were 230.4 MB apiece; after it, 34 MB apiece —
//! and there were still twenty-two of them, which is 0.66 GB of one dependency
//! graph written down twenty-two times.
//!
//! WHAT IT COSTS TO DO IT THIS WAY, AND WHAT IT COST TWICE. The tests share a
//! process, so anything process-global becomes shared state, and this file has
//! now been wrong about that in two different ways.
//!
//! FIRST: the check before the move looked for `set_current_dir`,
//! `env::set_var` and `env::remove_var` and found none — and missed that
//! `tracing`'s callsite interest cache is process-global even though
//! `with_default` is thread-local. `handler_span_hierarchy_smoke` failed 2 of
//! 30 runs under CPU contention inside this binary, 0 of 30 with its own
//! process, 0 of 100 after it got one back.
//!
//! SECOND, and it reached CI: the four `grpc_otlp_*` tests call
//! `init_otlp_tracing_subscriber*`, which ends in `.try_init()` — the
//! PROCESS-WIDE install. Only one of four can win it; the rest die with "a
//! global default trace dispatcher has already been set". Two things hid it.
//! The gate scanning for process-globals reads the TEST file, and these files
//! call a helper whose body does the install. And every one of them is
//! `#![cfg(feature = "otlp")]`, so `cargo test --workspace` WITHOUT
//! `--all-features` compiles them to nothing — the 1872-green this round
//! reported never built them. CI runs `--all-features` and said so.
//!
//! So two rules for adding a file here, and the second is the one that was
//! learned the hard way:
//!
//! 1. If it touches what the PROCESS owns — working directory, environment,
//!    tracing dispatcher, signal handlers, global allocator — it needs its own
//!    target. Directly OR through a helper.
//! 2. Verify with the FEATURES CI USES. A feature-gated test that does not
//!    compile cannot fail, and a suite that never built it reports green.
//!
//! WHAT DID NOT CHANGE. Test NAMES gain their module as a prefix
//! (`grpc_smoke::the_thing`), which is what `cargo test <name>` already matched
//! on and what the injection harness's red sets already resolve at module
//! boundaries. The feature gates each file carries as `#![cfg(feature = "…")]`
//! are inner attributes and keep applying to the file, now as a module — an
//! `otlp` test still compiles to nothing without the feature.
//!
//! `autotests = false` in `Cargo.toml` is what makes this the only target;
//! without it cargo would build these files twice, once here and once each on
//! its own.

// The shared harness, declared ONCE for the whole binary. The files below say
// `use crate::common` rather than `mod common`, because a `mod` inside a
// `#[path]`-loaded module resolves against that module's own directory.
#[path = "common/mod.rs"]
mod common;

#[path = "grpc_audit_cross_process_smoke.rs"]
mod grpc_audit_cross_process_smoke;

#[path = "grpc_audit_live_tail_smoke.rs"]
mod grpc_audit_live_tail_smoke;

#[path = "grpc_audit_resume_protocol_smoke.rs"]
mod grpc_audit_resume_protocol_smoke;

#[path = "grpc_audit_streaming_smoke.rs"]
mod grpc_audit_streaming_smoke;

#[path = "grpc_audit_subscriber_filter_smoke.rs"]
mod grpc_audit_subscriber_filter_smoke;

#[path = "grpc_batch_atomic_smoke.rs"]
mod grpc_batch_atomic_smoke;

#[path = "grpc_client_lb_smoke.rs"]
mod grpc_client_lb_smoke;

#[path = "grpc_health_smoke.rs"]
mod grpc_health_smoke;

#[path = "grpc_metadata_auth_smoke.rs"]
mod grpc_metadata_auth_smoke;

#[path = "grpc_mtls_smoke.rs"]
mod grpc_mtls_smoke;

// The four `grpc_otlp_*` tests are deliberately absent — see the header. Each
// calls `init_otlp_tracing_subscriber*`, which ends in `.try_init()`: the
// process-wide install. Only one of four can win it.

#[path = "grpc_reflection_smoke.rs"]
mod grpc_reflection_smoke;

#[path = "grpc_smoke.rs"]
mod grpc_smoke;

#[path = "grpc_streaming_smoke.rs"]
mod grpc_streaming_smoke;

#[path = "grpc_tls_smoke.rs"]
mod grpc_tls_smoke;

#[path = "grpc_trace_id_smoke.rs"]
mod grpc_trace_id_smoke;

#[path = "grpc_tracestate_smoke.rs"]
mod grpc_tracestate_smoke;

// `handler_span_hierarchy_smoke` is deliberately absent — see the header. Its
// subject is the process-global tracing dispatcher, so it owns a process.

#[path = "proposal_pipeline.rs"]
mod proposal_pipeline;
