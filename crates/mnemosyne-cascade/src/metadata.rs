//! Cascade dependency-graph metadata — *output* dependency-graph carry.
//! auto visualize (Studio cascade preview affected_asset_count + CLI batch
//! validation source). Read-only consumer path, runtime-independent.
//!
//! paradigm carry — Salsa runtime is itself Rust-only; this metadata
//! 5-language emit scope (Studio Kotlin / CLI Python / C++ runtime SDK
//! cascade preview output source).

/// Cascade dependency graph edges — `(query_name, dep_entity)` pairs.
/// Populated from [`crate::spec::design_doc_cascade_fixture`] queries' read deps.
pub fn cascade_dependency_edges() -> &'static [(&'static str, &'static str)] {
    &[
        ("section_decision_status", "Section"),
        ("section_decision_status", "ChangelogEntry"),
        ("frozen_list_membership", "FrozenList"),
        ("frozen_list_membership", "CrossRef"),
    ]
}

/// Per-query CascadeOrdering axis — `(query_name, ordering)` pairs.
pub fn cascade_orderings() -> &'static [(&'static str, &'static str)] {
    &[
        ("section_decision_status", "global_fifo"),
        ("frozen_list_membership", "global_fifo"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::design_doc_cascade_fixture;

    /// Metadata must agree with the spec fixture — drift detection.
    #[test]
    fn edges_align_with_fixture_reads() {
        let spec = design_doc_cascade_fixture();
        let mut expected: Vec<(&str, &str)> = Vec::new();
        for q in &spec.queries {
            for r in &q.reads {
                expected.push((q.name.as_str(), r.entity.as_str()));
            }
        }
        let actual: Vec<(&str, &str)> = cascade_dependency_edges().to_vec();
        assert_eq!(actual.len(), expected.len());
        for (q, e) in &expected {
            assert!(
                actual.iter().any(|(qq, ee)| qq == q && ee == e),
                "edge ({q}, {e}) missing in metadata"
            );
        }
    }

    #[test]
    fn orderings_align_with_fixture_axes() {
        let spec = design_doc_cascade_fixture();
        let actual = cascade_orderings();
        assert_eq!(actual.len(), spec.queries.len());
        for q in &spec.queries {
            assert!(actual
                .iter()
                .any(|(name, ord)| *name == q.name && *ord == q.ordering));
        }
    }

    #[test]
    fn fixture_has_four_edges_two_queries() {
        assert_eq!(cascade_dependency_edges().len(), 4);
        assert_eq!(cascade_orderings().len(), 2);
    }
}
