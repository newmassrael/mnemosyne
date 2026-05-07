//! Integration test — Salsa runtime tracked-function invocation, memoize
//! stability, cascade dependency graph metadata, CascadeDb trait dispatch.
//!
//! Round 81 carry — `CascadeBranch::new(db, branch_id, revision, snapshot_payload)`
//! signature in empty snapshot as vacuous-ok smoke validation.

use mnemosyne_cascade::{
 cascade_dependency_edges, cascade_orderings, design_doc_cascade_fixture,
 frozen_list_membership, section_decision_status, BranchSnapshotData, CascadeBranch,
 MnemosyneCascadeDb, ValidationResult,
};

fn empty_branch(db: &MnemosyneCascadeDb, branch_id: u64) -> CascadeBranch {
 let payload = BranchSnapshotData::default().encode().expect("encode");
 CascadeBranch::new(db, branch_id, 1, payload)
}

#[test]
fn tracked_section_decision_status_invokes() {
 let db = MnemosyneCascadeDb::default();
 let branch = empty_branch(&db, 0);
 assert_eq!(
 section_decision_status(&db, branch),
 ValidationResult::ok()
 );
}

#[test]
fn tracked_frozen_list_membership_invokes() {
 let db = MnemosyneCascadeDb::default();
 let branch = empty_branch(&db, 7);
 assert_eq!(
 frozen_list_membership(&db, branch),
 ValidationResult::ok()
 );
}

#[test]
fn memoize_returns_same_result_on_re_invocation() {
 let db = MnemosyneCascadeDb::default();
 let branch = empty_branch(&db, 42);
 let a = section_decision_status(&db, branch);
 let b = section_decision_status(&db, branch);
 assert_eq!(a, b);
 let c = frozen_list_membership(&db, branch);
 let d = frozen_list_membership(&db, branch);
 assert_eq!(c, d);
}

#[test]
fn dependency_graph_metadata_callable() {
 let edges = cascade_dependency_edges();
 assert_eq!(edges.len(), 4);
 assert!(edges.contains(&("section_decision_status", "Section")));
 assert!(edges.contains(&("section_decision_status", "ChangelogEntry")));
 assert!(edges.contains(&("frozen_list_membership", "FrozenList")));
 assert!(edges.contains(&("frozen_list_membership", "CrossRef")));
}

#[test]
fn ordering_axis_metadata_callable() {
 let orderings = cascade_orderings();
 assert_eq!(orderings.len(), 2);
 for (_, ord) in orderings {
 assert_eq!(*ord, "global_fifo");
 }
}

#[test]
fn cascade_db_trait_method_dispatch() {
 use mnemosyne_cascade::CascadeDb;
 let db = MnemosyneCascadeDb::default();
 let branch = empty_branch(&db, 0);
 let r1 = db.section_decision_status(branch);
 let r2 = db.frozen_list_membership(branch);
 assert_eq!(r1, ValidationResult::ok());
 assert_eq!(r2, ValidationResult::ok());
}

#[test]
fn fixture_topology_matches_metadata() {
 let spec = design_doc_cascade_fixture();
 let edges = cascade_dependency_edges();
 let mut count = 0;
 for q in &spec.queries {
 for r in &q.reads {
 assert!(edges
  .iter()
  .any(|(qq, ee)| *qq == q.name && *ee == r.entity));
 count += 1;
 }
 }
 assert_eq!(count, edges.len());
}
