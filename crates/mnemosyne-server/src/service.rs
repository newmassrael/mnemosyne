//! `MnemosyneService` — embedded async API surface.
//!
//! Plain Rust async trait. The gRPC transport binding (tonic + prost) is
//! deferred until protoc toolchain is available. When that lands, the
//! transport layer wraps this same trait via a tonic-generated server impl.

use crate::error::Result;
use crate::handler::ProposalHandler;
use crate::proposal::{Proposal, ProposalResult};
use std::sync::Arc;

pub trait MnemosyneService {
    /// Submit a proposal through the 3-tier gate pipeline. Returns the
    /// proposal result with audit transaction id (whether accepted or
    /// rejected).
    fn submit_proposal(&self, proposal: &Proposal) -> Result<ProposalResult>;
}

impl MnemosyneService for ProposalHandler {
    fn submit_proposal(&self, proposal: &Proposal) -> Result<ProposalResult> {
        self.handle(proposal)
    }
}

impl MnemosyneService for Arc<ProposalHandler> {
    fn submit_proposal(&self, proposal: &Proposal) -> Result<ProposalResult> {
        self.handle(proposal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::MnemosyneServer;
    use crate::proposal::ProposalKind;
    use mnemosyne_store::MnemosyneStore;
    use tempfile::TempDir;

    #[test]
    fn service_trait_dispatch_via_handler() {
        let dir = TempDir::new().unwrap();
        let store = Arc::new(MnemosyneStore::open(dir.path()).unwrap());
        let server = MnemosyneServer::new(store);
        let handler = server.handler().clone();
        let svc: &dyn MnemosyneService = &*handler;
        let p = Proposal {
            proposal_id: "p-svc".to_string(),
            actor: "test".to_string(),
            kind: ProposalKind::EntityCreate {
                entity_type: "Section".to_string(),
                branch_id: 1,
                entity_id: 1,
                valid_from: 100,
                payload: b"data".to_vec(),
            },
        };
        let r = svc.submit_proposal(&p).unwrap();
        assert!(r.accepted);
    }
}
