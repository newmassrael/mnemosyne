//! Integration test — proposal handler round-trip + 3-tier gate semantic +
//! audit append-only enforcement + service trait smoke.

use mnemosyne_server::{AuditAppender, MnemosyneServer, MnemosyneService, Proposal, ProposalKind};
use mnemosyne_store::{CfId, MnemosyneStore};
use std::sync::Arc;
use tempfile::TempDir;

fn fresh_server() -> (TempDir, MnemosyneServer) {
    let dir = TempDir::new().unwrap();
    let store = Arc::new(MnemosyneStore::open(dir.path()).unwrap());
    (dir, MnemosyneServer::new(store))
}

#[test]
fn entity_create_full_pipeline_succeeds() {
    let (_dir, server) = fresh_server();
    let p = Proposal {
        proposal_id: "ip-001".to_string(),
        actor: "alice".to_string(),
        kind: ProposalKind::EntityCreate {
            entity_type: "Section".to_string(),
            branch_id: 1,
            entity_id: 1,
            valid_from: 100,
            payload: b"section-payload".to_vec(),
        },
    };
    let r = server.submit(&p).unwrap();
    assert!(r.accepted);
    let audit = server
        .handler()
        .audit()
        .read(r.audit_transaction_id.unwrap())
        .unwrap()
        .unwrap();
    assert!(audit.accepted);
    assert_eq!(audit.proposal_kind_tag, "entity_create");
}

#[test]
fn cross_ref_with_zero_target_rejected_with_audit_record() {
    let (_dir, server) = fresh_server();
    let p = Proposal {
        proposal_id: "ip-002".to_string(),
        actor: "alice".to_string(),
        kind: ProposalKind::CrossRefCreate {
            branch_id: 1,
            from_section: 66,
            to_section: 0,
            ref_kind: "decision".to_string(),
        },
    };
    let r = server.submit(&p).unwrap();
    assert!(!r.accepted);
    assert!(r
        .rejection_reason
        .unwrap()
        .contains("to_section unresolved"));
    let audit = server
        .handler()
        .audit()
        .read(r.audit_transaction_id.unwrap())
        .unwrap()
        .unwrap();
    assert!(!audit.accepted);
    assert_eq!(audit.gate_routing_reason, "t1_reject");
}

#[test]
fn changelog_with_zero_entity_id_rejected_at_tier1() {
    let (_dir, server) = fresh_server();
    let p = Proposal {
        proposal_id: "ip-003".to_string(),
        actor: "alice".to_string(),
        kind: ProposalKind::ChangelogAppend {
            branch_id: 1,
            entity_id: 0,
            valid_from: 100,
            payload: vec![],
        },
    };
    let r = server.submit(&p).unwrap();
    assert!(!r.accepted);
    let audit = server
        .handler()
        .audit()
        .read(r.audit_transaction_id.unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(audit.gate_routing_reason, "t1_reject");
}

#[test]
fn frozen_list_without_changelog_attachment_rejected_at_tier1() {
    let (_dir, server) = fresh_server();
    let p = Proposal {
        proposal_id: "ip-004".to_string(),
        actor: "alice".to_string(),
        kind: ProposalKind::FrozenListMembershipChange {
            branch_id: 1,
            list_id: 200,
            valid_from: 100,
            attached_changelog_entry_id: 0,
            payload: vec![],
        },
    };
    let r = server.submit(&p).unwrap();
    assert!(!r.accepted);
    let audit = server
        .handler()
        .audit()
        .read(r.audit_transaction_id.unwrap())
        .unwrap()
        .unwrap();
    assert!(audit.rejection_reason.unwrap().contains("missing attached"));
}

#[test]
fn frozen_list_with_changelog_attachment_accepted_and_committed() {
    let (_dir, server) = fresh_server();
    let p = Proposal {
        proposal_id: "ip-005".to_string(),
        actor: "alice".to_string(),
        kind: ProposalKind::FrozenListMembershipChange {
            branch_id: 1,
            list_id: 200,
            valid_from: 100,
            attached_changelog_entry_id: 73,
            payload: b"frozen-list-state".to_vec(),
        },
    };
    let r = server.submit(&p).unwrap();
    assert!(r.accepted);
    let stored = server
        .handler()
        .store()
        .get(CfId::Entities, 1, 200, 100)
        .unwrap();
    assert_eq!(stored.as_deref(), Some(b"frozen-list-state".as_ref()));
}

#[test]
fn audit_append_only_rejects_overwrite_attempt() {
    let dir = TempDir::new().unwrap();
    let store = Arc::new(MnemosyneStore::open(dir.path()).unwrap());
    let appender = AuditAppender::new(Arc::clone(&store));
    let p = Proposal {
        proposal_id: "ip-006".to_string(),
        actor: "alice".to_string(),
        kind: ProposalKind::EntityCreate {
            entity_type: "Section".to_string(),
            branch_id: 1,
            entity_id: 1,
            valid_from: 100,
            payload: vec![],
        },
    };
    let txn = appender.append_accepted(&p, &[]).unwrap();
    let err = appender
        .check_no_overwrite(txn)
        .expect_err("must reject overwrite");
    assert!(format!("{err}").contains("append-only"));
}

#[test]
fn service_trait_smoke_via_arc_handler() {
    let (_dir, server) = fresh_server();
    let handler = server.handler().clone();
    let svc: Arc<dyn MnemosyneService + Send + Sync> = handler;
    let p = Proposal {
        proposal_id: "ip-007".to_string(),
        actor: "alice".to_string(),
        kind: ProposalKind::EntityCreate {
            entity_type: "Section".to_string(),
            branch_id: 1,
            entity_id: 99,
            valid_from: 100,
            payload: b"trait-dispatched".to_vec(),
        },
    };
    let r = svc.submit_proposal(&p).unwrap();
    assert!(r.accepted);
}

#[test]
fn monotonic_audit_transaction_ids_within_pipeline() {
    let (_dir, server) = fresh_server();
    let p1 = Proposal {
        proposal_id: "ip-008a".to_string(),
        actor: "alice".to_string(),
        kind: ProposalKind::EntityCreate {
            entity_type: "Section".to_string(),
            branch_id: 1,
            entity_id: 1,
            valid_from: 100,
            payload: vec![],
        },
    };
    let p2 = Proposal {
        proposal_id: "ip-008b".to_string(),
        actor: "alice".to_string(),
        kind: ProposalKind::EntityCreate {
            entity_type: "Section".to_string(),
            branch_id: 1,
            entity_id: 2,
            valid_from: 100,
            payload: vec![],
        },
    };
    let r1 = server.submit(&p1).unwrap();
    let r2 = server.submit(&p2).unwrap();
    assert_eq!(
        r2.audit_transaction_id.unwrap(),
        r1.audit_transaction_id.unwrap() + 1
    );
}
