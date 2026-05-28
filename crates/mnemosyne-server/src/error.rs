//! Typed server errors.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ServerError {
    #[error(transparent)]
    Store(#[from] mnemosyne_store::StoreError),

    #[error(transparent)]
    Core(#[from] mnemosyne_facts::PersistError),

    #[error("validator rejection: {0}")]
    ValidatorReject(String),

    #[error("3-tier gate rejection at {tier:?}: {reason}")]
    GateReject { tier: GateTierLabel, reason: String },

    #[error("audit append-only violation: {0}")]
    AuditViolation(String),

    #[error("malformed proposal: {0}")]
    MalformedProposal(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateTierLabel {
    Tier1,
    Tier2,
    Tier3,
}

pub type Result<T> = std::result::Result<T, ServerError>;
