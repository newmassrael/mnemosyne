//! Mnemosyne server — Phase 0 production crate (DESIGN.md / /).
//!
//! Phase 0 server-side stack entry point for this crate — proposal handler
//! ([`handler`]) + 3-tier gate (T1/T2/T3, [`gate`]) + audit append-only
//! enforcement ([`audit`]) + service interface ([`service`]).
//!
//! OPTION B-5 production carry — full Phase 0 stack binding (store +
//! core + cascade + validator).
//!
//! ## Transport binding
//!
//! This crate's embedded service interface — a plain Rust trait
//! ([`service::MnemosyneService`]) — host application is the in-process server
//! direct invoke path. carry — gRPC transport (tonic + prost) -
//! [`grpc`] module's [`grpc::MnemosyneGrpcService`] wraps the same [`handler::ProposalHandler`]
//! Direct wrap. Embedded ↔ gRPC `ProposalResult` value-equal (transport
//! independence validation = `tests/grpc_smoke.rs`).
//!
//! ## Module separation
//!
//! - [`proposal`]: `Proposal` request type + `ProposalResult` response type +
//! `ProposalKind` enum (entity_create / entity_update / changelog_append /
//! cross_ref_create / frozen_list_membership_change).
//! - [`gate`]: 3-tier gate (T1 cross-ref orphan reject + append-only +
//! membership-delta + supersede; T2 structural; T3 convention) —
//! Tier mapping carry.
//! - [`audit`]: `AuditAppender` — append-only audit CF in transaction-record
//! write.
//! - [`handler`]: `ProposalHandler` — proposal pipeline orchestrator (parse →
//! gate → audit → commit).
//! - [`service`]: `MnemosyneService` async trait — embedded API surface.
//! - [`grpc`]: tonic-generated `Mnemosyne` service binding.
//! - [`error`]: `ServerError` typed enum.

pub mod audit;
pub mod error;
pub mod gate;
pub mod grpc;
pub mod handler;
pub mod proposal;
pub mod service;

pub use audit::{AuditAppender, AuditRecord};
pub use error::ServerError;
pub use gate::{GateOutcome, GateTier, Tier1Gate, Tier2Gate, Tier3Gate};
pub use grpc::{
    build_health_service, build_reflection_service, with_tracing_span, MnemosyneGrpcService,
    MnemosyneServer as GrpcServer, MNEMOSYNE_FILE_DESCRIPTOR_SET,
};
#[cfg(feature = "otlp")]
pub use grpc::{init_otlp_tracing_subscriber, OtlpTracerGuard};
#[cfg(feature = "tls")]
pub use grpc::{install_default_crypto_provider, server_tls_config, tls_identity_from_pem};
pub use handler::{MnemosyneServer, ProposalHandler};
pub use proposal::{Proposal, ProposalKind, ProposalResult};
pub use service::MnemosyneService;
