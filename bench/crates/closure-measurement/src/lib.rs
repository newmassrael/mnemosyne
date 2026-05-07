//! Lock #1 closure runtime 5-language emit-divergence measurement spike (Round 93).
//!
//! DESIGN.md §11 *Knowledge closure rule* + §41 *closure runtime fallback path* +
//! Tier 5 measurement-pending Lock #1 (registered Round 38) measurement-backed
//! substantiation source. Round 76 floor (data shape 5-lang feasible) / ceiling
//! 35-fixture extension layer for the (computation Rust-only) substantive-validation carry.
//!
//! ## Workload
//!
//! 35 synthetic closure rule fixtures (varying rule count / depth / premise
//! count / predicate naming) × 5 backends (rust / kotlin / python / cpp / go)
//! = **175 fixture run** (DESIGN.md §11 *target metric* canonical alignment,
//! Round 95).
//!
//! Round 93 baseline — `protobuf` carries the 5-backend slot (§11
//! `go` was added in Round 95 via `emit_closure_metadata_go`,
//! the `BACKENDS` constant and `backends_of` function replaces `go` — §11 canonical
//! Alignment carry. The `protobuf` field on `EmittedClosureMetadata` is preserved for backwards-
//! Compat reference is preserved only for this measurement set's workload.
//!
//! ## Measurement dimensions
//!
//! - **Floor (data shape 5-language emit feasible)**: per-(fixture, backend)
//! canonical_set Jaccard inclusion = 1.0. PASS = 175/175.
//! - **Ceiling (computation is Rust-only; 5-backend byte-identical NOT achieved)**:
//! per-fixture pairwise byte equality of 5 backend emits — `n×n=25`, off-diag
//! `5C2 = 10` pairs per fixture × 35 fixtures = **350 pairs**. Pass path
//! (RFC pass) = 350/350 byte-equal (vacuously satisfied for empty violation
//! sets). The not-passing path (current reality) = 0/350 byte-equal (closure scope
//! 5-language emit contract's *partial-break inherent essence*.
//! - **Pairwise Jaccard distance matrix (5×5)**: per-backend per-fixture
//! identifier set extraction → cross-backend Jaccard distance aggregation.
//! diag = 1.0, off-diag < 1.0 (byte-different syntax → distinct identifier
//! sets, despite shared canonical_set inclusion).
//!
//! ## Lock #1 decision source
//!
//! - **floor pass** (175/175 PASS) → data shape 5-lang feasible, derivation
//! kind RFC fallback path's *5-language emit contract's closure-scope partial
//! break* audit trail's *measurement-backed substantiation*.
//! - **ceiling does not pass** (350/350 byte-different) → forward-chaining computation
//! substantive measurement of the Rust-only inherence.
//!
//! Round 76 substantive-decision source's 35-fixture extension layer — Lock #1's *full
//! measurement-pending lock* carry ratified; the RFC-pass moment on §35-36 paradigm
//! Decision can be ratified later; RFC does not pass on *the 5-language emit contract's closure
//! scope (partial break of the formal audit trail) — measurement-backed substantiation carry.

use codegen_prototype::closure_runtime::{
 closure_metadata_canonical_set, emit_closure_metadata_all_languages, ClosureRule,
 EmittedClosureMetadata, RuleConclusion, RulePremise,
};
use codegen_prototype::entity_indexer::{jaccard_inclusion, sha256_hex};
use serde::Serialize;
use std::collections::BTreeSet;

/// Total fixture count target (DESIGN.md §11 *target metric* consistency).
pub const FIXTURE_COUNT: usize = 35;

/// Backend names in measurement order (5-backend Lock #1 measurement carry,
/// existing `EmittedClosureMetadata` field order).
/// Round 95 — §11 canonical 5-backend list (rust/kotlin/python/cpp/go).
/// Replaces the prior `protobuf` placeholder with `go` for canonical alignment.
/// `protobuf` carries on `EmittedClosureMetadata` as a backwards-compat
/// reference field — not part of the §11 measurement set.
pub const BACKENDS: [&str; 5] = ["rust", "kotlin", "python", "cpp", "go"];

// --- 35-fixture generator --------------------------------------------------

/// Synthetic closure rule fixture #idx (0-based, range 0..FIXTURE_COUNT).
///
/// Varies along three axes for fixture diversity:
/// - rule_count: 1..=4 (cycles every 4 indices)
/// - depth: 1..=4
/// - premise_count: 1..=3
///
/// Predicate names are derived from `idx` for cross-fixture distinctness while
/// remaining canonical-set deterministic per-fixture (re-emit byte-stable).
pub fn synthetic_closure_fixture(idx: usize) -> Vec<ClosureRule> {
 let rule_count = (idx % 4) + 1;
 let depth = ((idx / 4) % 4 + 1) as u32;
 let premise_count = (idx / 16) % 3 + 1;
 let mut rules = Vec::with_capacity(rule_count);
 for r in 0..rule_count {
 let rule_id = format!("synth_rule_{idx:02}_{r}");
 let mut premises = Vec::with_capacity(premise_count);
 for p in 0..premise_count {
 premises.push(RulePremise {
  predicate: format!("predicate_{idx}_{r}_{p}"),
  roles: vec![
  ("subject".into(), format!("V{p}")),
  ("object".into(), format!("V{}", p + 1)),
  ],
 });
 }
 let conclusion = RuleConclusion {
 predicate: format!("derived_{idx}_{r}"),
 roles: vec![
  ("subject".into(), "V0".into()),
  ("object".into(), format!("V{premise_count}")),
 ],
 };
 rules.push(ClosureRule {
 id: rule_id,
 depth,
 premises,
 conclusion,
 });
 }
 rules
}

// --- floor / ceiling / Jaccard distance result types ----------------------

#[derive(Debug, Clone, Serialize)]
pub struct FloorResult {
 pub fixture_count: usize,
 pub backend_count: usize,
 pub total_runs: usize,
 pub jaccard_inclusion_pass_count: usize,
 /// Per-backend pass count across all fixtures. PASS = inclusion = 1.0.
 pub per_backend_pass: Vec<(String, usize)>,
 pub all_pass: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CeilingResult {
 pub fixture_count: usize,
 pub backend_count: usize,
 /// Total off-diagonal pairs measured: 5C2 × 35 = 350.
 pub total_pairs: usize,
 /// How many pairs have identical bytes — RFC pass path = total, RFC
 /// non-pass path = 0 (current reality).
 pub byte_identical_pairs: usize,
 /// `partial_keepership_substantiation` — true if byte_identical_pairs == 0
 /// (Computation-Rust-only-inherent measurement-backed substantiation.)
 pub partial_keepership_substantiation: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct JaccardMatrixResult {
 pub fixture_count: usize,
 /// 5×5 pairwise Jaccard distance matrix between backends, computed over
 /// the union of identifier sets across all fixtures. diag = 1.0, off-diag
 /// = `|A∩B| / |A∪B|` of canonical-set identifiers extracted from each
 /// backend's emit (whitespace-tokenized, alphanumeric+underscore only).
 pub matrix: Vec<Vec<f64>>,
 /// Mean off-diagonal distance — single-number summary of cross-backend
 /// identifier set similarity.
 pub mean_off_diagonal: f64,
 /// Min off-diagonal distance.
 pub min_off_diagonal: f64,
 /// Max off-diagonal distance.
 pub max_off_diagonal: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClosureMeasurementReport {
 pub fixture_count: usize,
 pub backend_count: usize,
 pub backends: Vec<String>,
 pub floor: FloorResult,
 pub ceiling: CeilingResult,
 pub jaccard: JaccardMatrixResult,
 pub lock_1_floor_pass: bool,
 pub lock_1_ceiling_partial_keepership_substantiated: bool,
}

// --- measurement bodies ----------------------------------------------------

fn backends_of(meta: &EmittedClosureMetadata) -> [&str; 5] {
 // Round 95 — §11 canonical alignment (rust/kotlin/python/cpp/go).
 [
 meta.rust.as_str(),
 meta.kotlin.as_str(),
 meta.python.as_str(),
 meta.cpp.as_str(),
 meta.go.as_str(),
 ]
}

/// Floor measurement — per-(fixture, backend) canonical_set Jaccard inclusion.
pub fn measure_floor(fixtures: usize) -> FloorResult {
 let mut per_backend_pass: Vec<usize> = vec![0; 5];
 let mut total_pass: usize = 0;
 for idx in 0..fixtures {
 let rules = synthetic_closure_fixture(idx);
 let canonical = closure_metadata_canonical_set(&rules);
 let meta = emit_closure_metadata_all_languages(&rules);
 for (b_idx, text) in backends_of(&meta).iter().enumerate() {
 let j = jaccard_inclusion(text, &canonical);
 if (j - 1.0).abs() < f64::EPSILON {
  per_backend_pass[b_idx] += 1;
  total_pass += 1;
 }
 }
 }
 let total = fixtures * 5;
 FloorResult {
 fixture_count: fixtures,
 backend_count: 5,
 total_runs: total,
 jaccard_inclusion_pass_count: total_pass,
 per_backend_pass: BACKENDS
 .iter()
 .zip(per_backend_pass.iter())
 .map(|(name, count)| (name.to_string(), *count))
 .collect(),
 all_pass: total_pass == total,
 }
}

/// Ceiling measurement — per-fixture pairwise byte equality of 5 backend
/// emits. Computes `5C2 = 10` pairs per fixture × N fixtures.
pub fn measure_ceiling(fixtures: usize) -> CeilingResult {
 let mut total_pairs = 0;
 let mut identical_pairs = 0;
 for idx in 0..fixtures {
 let rules = synthetic_closure_fixture(idx);
 let meta = emit_closure_metadata_all_languages(&rules);
 let backends = backends_of(&meta);
 for i in 0..5 {
 for j in (i + 1)..5 {
  total_pairs += 1;
  if backends[i] == backends[j] {
  identical_pairs += 1;
  }
 }
 }
 }
 CeilingResult {
 fixture_count: fixtures,
 backend_count: 5,
 total_pairs,
 byte_identical_pairs: identical_pairs,
 partial_keepership_substantiation: identical_pairs == 0,
 }
}

/// Extract identifier-like tokens from a string — alphanumeric + underscore
/// runs of length ≥ 2. Deterministic given input bytes; used for Jaccard
/// distance measurement between backend emits.
fn identifier_tokens(s: &str) -> BTreeSet<String> {
 let mut tokens = BTreeSet::new();
 let mut cur = String::new();
 for c in s.chars() {
 if c.is_alphanumeric() || c == '_' {
 cur.push(c);
 } else {
 if cur.len() >= 2 {
  tokens.insert(cur.clone());
 }
 cur.clear();
 }
 }
 if cur.len() >= 2 {
 tokens.insert(cur);
 }
 tokens
}

fn jaccard_distance(a: &BTreeSet<String>, b: &BTreeSet<String>) -> f64 {
 if a.is_empty() && b.is_empty() {
 return 1.0;
 }
 let intersect = a.intersection(b).count() as f64;
 let union = a.union(b).count() as f64;
 if union == 0.0 {
 1.0
 } else {
 intersect / union
 }
}

/// Pairwise Jaccard distance between backend identifier sets, aggregated over
/// all fixtures (union of identifier sets per backend).
pub fn measure_pairwise_jaccard(fixtures: usize) -> JaccardMatrixResult {
 let mut per_backend: Vec<BTreeSet<String>> = vec![BTreeSet::new(); 5];
 for idx in 0..fixtures {
 let rules = synthetic_closure_fixture(idx);
 let meta = emit_closure_metadata_all_languages(&rules);
 for (b_idx, text) in backends_of(&meta).iter().enumerate() {
 per_backend[b_idx].extend(identifier_tokens(text));
 }
 }
 let mut matrix: Vec<Vec<f64>> = vec![vec![0.0; 5]; 5];
 let mut off_diag: Vec<f64> = Vec::with_capacity(20);
 for i in 0..5 {
 for j in 0..5 {
 let d = jaccard_distance(&per_backend[i], &per_backend[j]);
 matrix[i][j] = d;
 if i != j {
  off_diag.push(d);
 }
 }
 }
 let sum: f64 = off_diag.iter().sum();
 let mean = if off_diag.is_empty() {
 0.0
 } else {
 sum / off_diag.len() as f64
 };
 let min = off_diag.iter().cloned().fold(f64::INFINITY, f64::min);
 let max = off_diag.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
 JaccardMatrixResult {
 fixture_count: fixtures,
 matrix,
 mean_off_diagonal: mean,
 min_off_diagonal: if min.is_finite() { min } else { 0.0 },
 max_off_diagonal: if max.is_finite() { max } else { 0.0 },
 }
}

pub fn run_full_measurement(fixtures: usize) -> ClosureMeasurementReport {
 let floor = measure_floor(fixtures);
 let ceiling = measure_ceiling(fixtures);
 let jaccard = measure_pairwise_jaccard(fixtures);
 let floor_pass = floor.all_pass;
 let ceiling_substantiated = ceiling.partial_keepership_substantiation;
 ClosureMeasurementReport {
 fixture_count: fixtures,
 backend_count: 5,
 backends: BACKENDS.iter().map(|s| s.to_string()).collect(),
 floor,
 ceiling,
 jaccard,
 lock_1_floor_pass: floor_pass,
 lock_1_ceiling_partial_keepership_substantiated: ceiling_substantiated,
 }
}

/// SHA-256 fingerprint of a single fixture's 5-backend emit aggregate — used
/// for determinism cross-check + debug output.
pub fn fixture_aggregate_sha256(idx: usize) -> String {
 let rules = synthetic_closure_fixture(idx);
 let meta = emit_closure_metadata_all_languages(&rules);
 let aggregate = format!(
 "{}\n{}\n{}\n{}\n{}",
 meta.rust, meta.kotlin, meta.python, meta.cpp, meta.go
 );
 sha256_hex(&aggregate)
}

#[cfg(test)]
mod tests {
 use super::*;

 #[test]
 fn fixture_count_constant_matches_lock1_target() {
 assert_eq!(FIXTURE_COUNT, 35, "DESIGN.md §11 target = 35 fixtures");
 }

 #[test]
 fn synthetic_fixture_is_deterministic() {
 let a = synthetic_closure_fixture(7);
 let b = synthetic_closure_fixture(7);
 assert_eq!(a, b);
 }

 #[test]
 fn synthetic_fixtures_produce_distinct_predicates() {
 let f0 = synthetic_closure_fixture(0);
 let f5 = synthetic_closure_fixture(5);
 assert_ne!(
 f0[0].id, f5[0].id,
 "different fixture indices must produce distinct rule ids"
 );
 }

 #[test]
 fn floor_measurement_passes_for_full_35_fixtures() {
 let r = measure_floor(FIXTURE_COUNT);
 assert_eq!(r.fixture_count, 35);
 assert_eq!(r.backend_count, 5);
 assert_eq!(r.total_runs, 175);
 assert_eq!(
 r.jaccard_inclusion_pass_count, 175,
 "floor: all 175 (fixture × backend) runs must satisfy canonical_set inclusion = 1.0"
 );
 assert!(r.all_pass);
 for (backend, count) in &r.per_backend_pass {
 assert_eq!(
  *count, 35,
  "backend {} must pass all 35 fixtures",
  backend
 );
 }
 }

 #[test]
 fn ceiling_measurement_substantiates_partial_keepership() {
 let r = measure_ceiling(FIXTURE_COUNT);
 assert_eq!(r.total_pairs, 350);
 assert_eq!(
 r.byte_identical_pairs, 0,
 "ceiling: 0 byte-identical pairs across 350 — RFC not.ass path measurement-backed"
 );
 assert!(r.partial_keepership_substantiation);
 }

 #[test]
 fn pairwise_jaccard_matrix_is_5x5_with_diag_one() {
 let r = measure_pairwise_jaccard(FIXTURE_COUNT);
 assert_eq!(r.matrix.len(), 5);
 for row in &r.matrix {
 assert_eq!(row.len(), 5);
 }
 for i in 0..5 {
 assert!(
  (r.matrix[i][i] - 1.0).abs() < f64::EPSILON,
  "diagonal must be 1.0 (self-similarity)"
 );
 }
 // Off-diagonal must be < 1.0 (distinct languages have distinct tokens).
 assert!(r.max_off_diagonal < 1.0);
 // But the canonical set is shared, so off-diagonal must be > 0.
 assert!(r.min_off_diagonal > 0.0);
 }

 #[test]
 fn jaccard_matrix_is_symmetric() {
 let r = measure_pairwise_jaccard(FIXTURE_COUNT);
 for i in 0..5 {
 for j in 0..5 {
  let diff = (r.matrix[i][j] - r.matrix[j][i]).abs();
  assert!(diff < f64::EPSILON, "matrix must be symmetric at ({i},{j})");
 }
 }
 }

 #[test]
 fn fixture_aggregate_sha256_is_64_char_hex() {
 let h = fixture_aggregate_sha256(0);
 assert_eq!(h.len(), 64);
 for c in h.chars() {
 assert!(c.is_ascii_hexdigit());
 }
 }

 #[test]
 fn fixture_aggregate_sha256_is_deterministic() {
 assert_eq!(fixture_aggregate_sha256(7), fixture_aggregate_sha256(7));
 }

 #[test]
 fn fixture_aggregates_are_distinct_across_indices() {
 let h0 = fixture_aggregate_sha256(0);
 let h1 = fixture_aggregate_sha256(1);
 assert_ne!(h0, h1, "different fixtures must yield distinct sha256");
 }

 #[test]
 fn run_full_measurement_lock1_floor_pass_ceiling_substantiated() {
 let r = run_full_measurement(FIXTURE_COUNT);
 assert_eq!(r.fixture_count, 35);
 assert_eq!(r.backend_count, 5);
 assert!(r.lock_1_floor_pass);
 assert!(r.lock_1_ceiling_partial_keepership_substantiated);
 }
}
