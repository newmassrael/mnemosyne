//! Integration test — fine-grained Salsa engine tracked-function invocation,
//! memoize stability, CascadeDb trait dispatch, and the cascade dependency-graph
//! metadata / fixture topology.

use mnemosyne_cascade::{
    build_branch_index, cascade_dependency_edges, cascade_orderings, design_doc_cascade_fixture,
    frozen_list_membership_aggregated, section_decision_status_aggregated, CascadeDb,
    FineCascadeDb, ValidationResult,
};

#[test]
fn empty_branch_section_decision_status_is_vacuously_ok() {
    let db = FineCascadeDb::new();
    let idx = build_branch_index(&db, 0, &[], &[], &[], &[]);
    assert_eq!(
        section_decision_status_aggregated(&db, idx),
        ValidationResult::ok()
    );
}

#[test]
fn empty_branch_frozen_list_membership_is_vacuously_ok() {
    let db = FineCascadeDb::new();
    let idx = build_branch_index(&db, 7, &[], &[], &[], &[]);
    assert_eq!(
        frozen_list_membership_aggregated(&db, idx),
        ValidationResult::ok()
    );
}

#[test]
fn memoize_returns_same_result_on_re_invocation() {
    let db = FineCascadeDb::new();
    let idx = build_branch_index(&db, 42, &[], &[], &[], &[]);
    let a = section_decision_status_aggregated(&db, idx);
    let b = section_decision_status_aggregated(&db, idx);
    assert_eq!(a, b);
    let c = frozen_list_membership_aggregated(&db, idx);
    let d = frozen_list_membership_aggregated(&db, idx);
    assert_eq!(c, d);
}

#[test]
fn cascade_db_trait_method_dispatch() {
    let db = FineCascadeDb::new();
    let idx = build_branch_index(&db, 0, &[], &[], &[], &[]);
    let r1 = db.fine_section_decision_status(idx);
    let r2 = db.fine_frozen_list_membership(idx);
    assert_eq!(r1, ValidationResult::ok());
    assert_eq!(r2, ValidationResult::ok());
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
