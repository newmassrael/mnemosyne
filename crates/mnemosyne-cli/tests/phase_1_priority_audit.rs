//! Round 172 — PHASE-1-PRIORITY-DECISION-AUDIT.
//!
//! Phase 1+ narrative entry decision round audit ledger (Round 22 operational
//! ordering / Round 47-49 prototype-first / Round 139 audit pattern equivalent).
//! Audits 6 carry areas registered in
//! `project_phase_1_narrative_carry.md` on 4 deterministic dimensions:
//!
//! - value  : creator-facing dogfood weight (1..=4, larger better)
//! - risk  : implementation difficulty (1..=4, smaller better)
//! - measurability : ratifiability of deliverable (1..=3, larger better)
//! - unmet_deps : upstream carry areas not yet RESOLVED (smaller better)
//!
//! Score = (value × measurability) ÷ (risk × (1 + unmet_deps)).
//!
//! Decision (closing inline): **fictional medium adapter first** —
//! candidate A adopted — dominates runner-up by ≥ 2× (creator-client target
//! direct path; the schema-as-input Phase 0e infrastructure is heavily reused. Branch
//! second, bi-temporal / cascade-narrative tied for third, studio-ui /
//! saga downstream.
//!
//! permission boundary: audit only — atomic store / workspace docs scope mutation 0,
//! 0 runtime change in the production crate (test-ledger only).

#[derive(Debug, Clone)]
struct AreaAudit {
 name: &'static str,
 value: u32,
 risk: u32,
 measurability: u32,
 unmet_deps: u32,
}

impl AreaAudit {
 fn score(&self) -> f64 {
 (self.value * self.measurability) as f64
 / (self.risk * (1 + self.unmet_deps)) as f64
 }
}

/// 6 carry areas as registered in `project_phase_1_narrative_carry.md`.
/// Score parameters are derived from the audit ledger in Round 172 entry.
fn carry_areas() -> Vec<AreaAudit> {
 vec![
 // ① branch — alternative design timeline / what-if exploration
 // (DESIGN.md §1 / §66 Stage 2).
 // value=2 (maintainer + writer dogfood), risk=3 (commit graph +
 // branch overlay), measurability=3 (fork/merge/diff round-trip),
 // unmet_deps=0.
 AreaAudit {
 name: "branch",
 value: 2,
 risk: 3,
 measurability: 3,
 unmet_deps: 0,
 },
 // ② bi-temporal — canon_time × transaction_time (DESIGN.md §4 / §11).
 // value=3 (fundamental query axis), risk=4 (schema rewrite +
 // query API rewrite), measurability=2 (time-travel query),
 // unmet_deps=0.
 AreaAudit {
 name: "bi-temporal",
 value: 3,
 risk: 4,
 measurability: 2,
 unmet_deps: 0,
 },
 // ③ cascade-narrative — narrative-specific cascade auto-update
 // (Round 168 infra reuse).
 // value=2 (writer cascade fan-out), risk=2 (infra reuse),
 // measurability=3 (fan-out band), unmet_deps=1 (fictional adapter).
 AreaAudit {
 name: "cascade-narrative",
 value: 2,
 risk: 2,
 measurability: 3,
 unmet_deps: 1,
 },
 // ④ saga — compensating retract under frozen ledger (DESIGN.md §12).
 // value=1 (low-frequency operation), risk=4 (frozen ledger conflict),
 // measurability=2 (compensating txn), unmet_deps=1 (bi-temporal).
 AreaAudit {
 name: "saga",
 value: 1,
 risk: 4,
 measurability: 2,
 unmet_deps: 1,
 },
 // ⑤ fictional-adapter — Novel / TRPG / Wiki / Game medium overlay
 // (DESIGN.md §39 / §56 / §61 / §66, Round 143 schema-as-input carry).
 // value=4 (creator-client target, first writer dogfood),
 // risk=2 (schema reuse Phase 0e infra), measurability=3 (4 medium
 // round-trip), unmet_deps=0 (schema-as-input DONE in Phase 0e).
 AreaAudit {
 name: "fictional-adapter",
 value: 4,
 risk: 2,
 measurability: 3,
 unmet_deps: 0,
 },
 // ⑥ studio-ui — creator-facing client (DESIGN.md §60 / §66 Stage 1+).
 // value=3 (writer typed-fact mutate UX), risk=4 (Kotlin/Tauri/web
 // stack pick), measurability=2 (mutate/query/cascade), unmet_deps=1
 // (fictional adapter; gRPC already DONE in Phase 0c).
 AreaAudit {
 name: "studio-ui",
 value: 3,
 risk: 4,
 measurability: 2,
 unmet_deps: 1,
 },
 ]
}

fn ranked_by_score() -> Vec<(String, f64)> {
 let mut scored: Vec<(String, f64)> = carry_areas()
 .iter()
 .map(|a| (a.name.to_string(), a.score()))
 .collect();
 scored.sort_by(|a, b| {
 b.1.partial_cmp(&a.1)
 .unwrap()
 .then_with(|| a.0.cmp(&b.0))
 });
 scored
}

// ============================================================================
// Dim α — score ranking determinism
// ============================================================================

#[test]
fn audit_priority_score_ranking_deterministic() {
 let scored = ranked_by_score();
 let names: Vec<&str> = scored.iter().map(|(n, _)| n.as_str()).collect();

 assert_eq!(
 names[0], "fictional-adapter",
 "highest priority must be fictional adapter (creator-client target, schema-as-input Phase 0e reuse)"
 );
 assert_eq!(
 names[1], "branch",
 "second priority must be branch (independent of bi-temporal, maintainer dogfood)"
 );
 assert_eq!(
 names[5], "saga",
 "lowest priority must be saga (bi-temporal dependency + frozen ledger conflict)"
 );
}

// ============================================================================
// Dim β — fictional adapter domination margin
// ============================================================================

#[test]
fn audit_fictional_adapter_dominates_by_margin() {
 let areas = carry_areas();
 let fictional = areas.iter().find(|a| a.name == "fictional-adapter").unwrap();
 let runner_up = areas
 .iter()
 .filter(|a| a.name != "fictional-adapter")
 .map(|a| a.score())
 .fold(0.0f64, f64::max);
 let margin = fictional.score() / runner_up;
 assert!(
 margin >= 2.0,
 "fictional adapter must dominate runner-up by >= 2.0x (currently {:.2}x, runner-up {:.2}, fictional {:.2})",
 margin,
 runner_up,
 fictional.score()
 );
}

// ============================================================================
// Dim γ — dependency graph integrity
// ============================================================================

#[test]
fn audit_dependency_graph_acyclic_three_edges() {
 // Encoded edges (fictional → cascade-narrative, fictional → studio-ui,
 // bi-temporal → saga). Acyclic by construction — branch / bi-temporal
 // / fictional-adapter form the 3 root nodes (unmet_deps = 0), the rest
 // descend.
 let areas = carry_areas();
 let total_deps: u32 = areas.iter().map(|a| a.unmet_deps).sum();
 assert_eq!(
 total_deps, 3,
 "exactly 3 dependency edges across 6 areas (fictional -> cascade-narrative, fictional -> studio-ui, bi-temporal -> saga)"
 );
 let roots: Vec<&str> = areas
 .iter()
 .filter(|a| a.unmet_deps == 0)
 .map(|a| a.name)
 .collect();
 assert_eq!(
 roots.len(),
 3,
 "exactly 3 root nodes (branch / bi-temporal / fictional-adapter)"
 );
 assert!(roots.contains(&"branch"));
 assert!(roots.contains(&"bi-temporal"));
 assert!(roots.contains(&"fictional-adapter"));
}

// ============================================================================
// Dim δ — candidate A ratify (closing inline decision)
// ============================================================================

#[test]
fn audit_candidate_a_ratify() {
 // candidate A = fictional adapter first. Ratified by:
 // (1) score ranking #1 (audit_priority_score_ranking_deterministic)
 // (2) ≥ 2x domination margin (audit_fictional_adapter_dominates_by_margin)
 // (3) zero unmet upstream deps (schema-as-input DONE in Phase 0e)
 // (4) creator-client target alignment (project_mnemosyne.md identity)
 let areas = carry_areas();
 let fictional = areas.iter().find(|a| a.name == "fictional-adapter").unwrap();
 assert_eq!(
 fictional.unmet_deps, 0,
 "candidate A pre-condition: fictional adapter must have zero unmet upstream deps (schema-as-input Phase 0e carry)"
 );
 assert_eq!(
 fictional.value, 4,
 "candidate A weight: fictional adapter must have value=4 (highest, creator-client target dogfood)"
 );
}

// ============================================================================
// Dim ε — full ranking matrix snapshot (drift detector)
// ============================================================================

#[test]
fn audit_ranking_matrix_snapshot() {
 let scored = ranked_by_score();
 let actual: Vec<&str> = scored.iter().map(|(n, _)| n.as_str()).collect();
 let expected = vec![
 "fictional-adapter",
 "branch",
 "bi-temporal",
 "cascade-narrative",
 "studio-ui",
 "saga",
 ];
 assert_eq!(
 actual, expected,
 "priority ranking drift — re-audit 6 area parameters in Round 172 entry if intentional"
 );
}
