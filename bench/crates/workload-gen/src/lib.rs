//! Phase -1A stage 1 synthetic workload generator (DESIGN.md §18 line 1796-1817).
//!
//! Generates the canonical workload used by stage 2A (XTDB baseline), stage 2B
//! (direct-impl branching prototype), and stage 2C (schema-shape micro-bench).
//! All consumers operate on the same `Workload` value to keep cross-stage
//! comparisons symmetric.
//!
//! Determinism contract: same `WorkloadConfig` (including `seed`) produces a
//! byte-equal `Workload`. Verified by [`verify_determinism`].
//!
//! Scale contract (§18 line 1912): a generated `Workload` must clear 90% of
//! every spec target, otherwise downstream measurement is *unfit* under the
//! incompleteness gate.

use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use rand_distr::{Distribution, Poisson};
use serde::{Deserialize, Serialize};

/// Workload schema version. Bump on any layout change that breaks byte-equality
/// across builds — downstream stages refuse to load mismatched versions.
pub const PROTOCOL_VERSION: u32 = 1;

/// §18 line 1796-1817 spec scale.
pub fn default_config() -> WorkloadConfig {
 WorkloadConfig {
 protocol_version: PROTOCOL_VERSION,
 assets: 200_000,
 facts: 50_000,
 branches: 1_000,
 agents: 1_000,
 entity_dist: EntityDistribution {
 person: 1_000,
 place: 100,
 faction: 30,
 event: 5_000,
 item: 10_000,
 concept: 500,
 },
 branch_tree_depth_avg: 5,
 branch_tree_depth_max: 30,
 facts_per_asset_avg: 5,
 seed: 0xC0FFEE_C0FFEE,
 }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkloadConfig {
 pub protocol_version: u32,
 pub assets: usize,
 pub facts: usize,
 pub branches: usize,
 pub agents: usize,
 pub entity_dist: EntityDistribution,
 pub branch_tree_depth_avg: u32,
 pub branch_tree_depth_max: u32,
 pub facts_per_asset_avg: u32,
 pub seed: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EntityDistribution {
 pub person: usize,
 pub place: usize,
 pub faction: usize,
 pub event: usize,
 pub item: usize,
 pub concept: usize,
}

impl EntityDistribution {
 pub fn total(&self) -> usize {
 self.person + self.place + self.faction + self.event + self.item + self.concept
 }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EntityKind {
 Person,
 Place,
 Faction,
 Event,
 Item,
 Concept,
}

impl EntityKind {
 pub fn as_str(self) -> &'static str {
 match self {
 EntityKind::Person => "Person",
 EntityKind::Place => "Place",
 EntityKind::Faction => "Faction",
 EntityKind::Event => "Event",
 EntityKind::Item => "Item",
 EntityKind::Concept => "Concept",
 }
 }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Entity {
 pub id: u64,
 pub kind: EntityKind,
 pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Fact {
 pub id: u64,
 pub predicate: String,
 pub subject: u64,
 pub object: u64,
 pub valid_from: u64,
 pub valid_to: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Branch {
 pub id: u64,
 pub parent: Option<u64>,
 pub depth: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Agent {
 pub id: u64,
 pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Asset {
 pub id: u64,
 pub content_hash: [u8; 32],
 pub facts_referenced: Vec<u64>,
 pub branch_id: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Workload {
 pub config: WorkloadConfig,
 pub branches: Vec<Branch>,
 pub entities: Vec<Entity>,
 pub facts: Vec<Fact>,
 pub agents: Vec<Agent>,
 pub assets: Vec<Asset>,
}

#[derive(Clone, Debug)]
pub struct ScaleReport {
 pub assets: ScaleEntry,
 pub facts: ScaleEntry,
 pub branches: ScaleEntry,
 pub agents: ScaleEntry,
 pub entities: ScaleEntry,
}

#[derive(Clone, Debug)]
pub struct ScaleEntry {
 pub target: usize,
 pub actual: usize,
}

impl ScaleEntry {
 pub fn ratio(&self) -> f64 {
 if self.target == 0 {
 return 1.0;
 }
 self.actual as f64 / self.target as f64
 }
 pub fn ok(&self) -> bool {
 self.ratio() >= 0.9
 }
}

impl ScaleReport {
 pub fn all_ok(&self) -> bool {
 self.assets.ok()
 && self.facts.ok()
 && self.branches.ok()
 && self.agents.ok()
 && self.entities.ok()
 }
}

impl Workload {
 /// §18 line 1912 incompleteness gate: workload must reach ≥ 90% of every
 /// spec target. Any miss locks downstream measurement.
 pub fn scale_report(&self) -> ScaleReport {
 let cfg = &self.config;
 ScaleReport {
 assets: ScaleEntry {
  target: cfg.assets,
  actual: self.assets.len(),
 },
 facts: ScaleEntry {
  target: cfg.facts,
  actual: self.facts.len(),
 },
 branches: ScaleEntry {
  target: cfg.branches,
  actual: self.branches.len(),
 },
 agents: ScaleEntry {
  target: cfg.agents,
  actual: self.agents.len(),
 },
 entities: ScaleEntry {
  target: cfg.entity_dist.total(),
  actual: self.entities.len(),
 },
 }
 }
}

/// Generate a workload from `config`. Output is deterministic in `config`.
pub fn generate(config: &WorkloadConfig) -> Workload {
 assert_eq!(
 config.protocol_version, PROTOCOL_VERSION,
 "workload protocol version mismatch"
 );
 let mut rng = ChaCha20Rng::seed_from_u64(config.seed);
 let branches = gen_branches(&mut rng, config);
 let entities = gen_entities(&config.entity_dist);
 let facts = gen_facts(&mut rng, &entities, config.facts);
 let agents = gen_agents(config.agents);
 let assets = gen_assets(
 &mut rng,
 &facts,
 &branches,
 config.assets,
 config.facts_per_asset_avg,
 );
 Workload {
 config: config.clone(),
 branches,
 entities,
 facts,
 agents,
 assets,
 }
}

fn gen_branches(rng: &mut ChaCha20Rng, config: &WorkloadConfig) -> Vec<Branch> {
 let max_depth = config.branch_tree_depth_max;
 let avg_depth = config.branch_tree_depth_avg.max(1);
 // Geometric distribution P(X = k) with mean = avg_depth: p = 1/(1+mean).
 let p = 1.0 / (1.0 + avg_depth as f64);

 let mut branches: Vec<Branch> = Vec::with_capacity(config.branches);
 let mut by_depth: Vec<Vec<u64>> = vec![vec![0]];
 branches.push(Branch {
 id: 0,
 parent: None,
 depth: 0,
 });

 for i in 1..config.branches as u64 {
 let max_target_depth = (by_depth.len() as u32 - 1).min(max_depth - 1);
 let target_depth = sample_geometric(rng, p).min(max_target_depth as u64) as u32;
 let candidates = &by_depth[target_depth as usize];
 let parent_id = candidates[rng.gen_range(0..candidates.len())];

 let new_depth = target_depth + 1;
 let branch = Branch {
 id: i,
 parent: Some(parent_id),
 depth: new_depth,
 };

 while by_depth.len() <= new_depth as usize {
 by_depth.push(Vec::new());
 }
 by_depth[new_depth as usize].push(i);
 branches.push(branch);
 }

 branches
}

fn sample_geometric(rng: &mut ChaCha20Rng, p: f64) -> u64 {
 // Inverse-CDF: floor(ln(1 - U) / ln(1 - p)) for U ~ Uniform[0, 1).
 let u: f64 = rng.gen_range(0.0..1.0);
 ((1.0 - u).ln() / (1.0 - p).ln()).floor() as u64
}

fn gen_entities(dist: &EntityDistribution) -> Vec<Entity> {
 let mut entities = Vec::with_capacity(dist.total());
 let mut next_id: u64 = 1;
 let kinds: [(EntityKind, usize); 6] = [
 (EntityKind::Person, dist.person),
 (EntityKind::Place, dist.place),
 (EntityKind::Faction, dist.faction),
 (EntityKind::Event, dist.event),
 (EntityKind::Item, dist.item),
 (EntityKind::Concept, dist.concept),
 ];
 for (kind, count) in kinds {
 for idx in 0..count {
 entities.push(Entity {
  id: next_id,
  kind,
  name: format!("{}_{:06}", kind.as_str(), idx),
 });
 next_id += 1;
 }
 }
 entities
}

const PREDICATES: &[&str] = &[
 "loves",
 "knows",
 "born_in",
 "died_in",
 "lives_in",
 "located_in",
 "happened_at",
 "owns",
 "member_of",
 "related_to",
 "founded",
 "destroyed",
 "succeeded",
 "preceded",
 "contains",
 "part_of",
 "discovered",
 "wrote",
 "read",
 "spoke_to",
 "killed",
 "saved",
 "betrayed",
 "trusted",
 "feared",
 "respected",
 "served",
 "ruled",
 "rebelled_against",
 "allied_with",
];

fn gen_facts(rng: &mut ChaCha20Rng, entities: &[Entity], count: usize) -> Vec<Fact> {
 let n = entities.len();
 let mut facts = Vec::with_capacity(count);
 for i in 0..count {
 let pred_idx = rng.gen_range(0..PREDICATES.len());
 let subject = entities[rng.gen_range(0..n)].id;
 let object = entities[rng.gen_range(0..n)].id;
 let valid_from = rng.gen_range(0..1_000_000u64);
 let valid_to = if rng.gen_bool(0.3) {
 None
 } else {
 Some(valid_from + rng.gen_range(1..100_000))
 };
 facts.push(Fact {
 id: (i as u64) + 1,
 predicate: PREDICATES[pred_idx].to_string(),
 subject,
 object,
 valid_from,
 valid_to,
 });
 }
 facts
}

fn gen_agents(count: usize) -> Vec<Agent> {
 (0..count as u64)
 .map(|i| Agent {
 id: i + 1,
 name: format!("agent_{:06}", i),
 })
 .collect()
}

fn gen_assets(
 rng: &mut ChaCha20Rng,
 facts: &[Fact],
 branches: &[Branch],
 count: usize,
 facts_per_asset_avg: u32,
) -> Vec<Asset> {
 let n_facts = facts.len();
 let n_branches = branches.len();
 let poisson = Poisson::new(facts_per_asset_avg as f64).expect("lambda > 0");
 let mut assets = Vec::with_capacity(count);
 for i in 0..count {
 let asset_id = (i as u64) + 1;
 let content_hash = synth_content_hash(asset_id);
 let n_refs = (poisson.sample(rng) as usize).max(1);
 let mut refs: Vec<u64> = Vec::with_capacity(n_refs);
 for _ in 0..n_refs {
 refs.push(facts[rng.gen_range(0..n_facts)].id);
 }
 refs.sort_unstable();
 refs.dedup();
 let branch_id = branches[rng.gen_range(0..n_branches)].id;
 assets.push(Asset {
 id: asset_id,
 content_hash,
 facts_referenced: refs,
 branch_id,
 });
 }
 assets
}

fn synth_content_hash(asset_id: u64) -> [u8; 32] {
 // Deterministic non-cryptographic placeholder. First 8 bytes = id (BE),
 // remaining bytes derived from a SplitMix64 walk so collisions across
 // distinct ids are improbable for the 200K spec scale.
 let mut h = [0u8; 32];
 let mut state = asset_id;
 for chunk in h.chunks_mut(8) {
 state = splitmix64(state);
 chunk.copy_from_slice(&state.to_be_bytes());
 }
 h
}

fn splitmix64(mut x: u64) -> u64 {
 x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
 let mut z = x;
 z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
 z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
 z ^ (z >> 31)
}

// ─── Query trace (round-robin per Priority 0 (e) decision) ───────────────────

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Query {
 PointLookup {
 entity_id: u64,
 branch_id: u64,
 },
 ThreeHopGraph {
 start: u64,
 branch_id: u64,
 },
 TemporalRange {
 entity_id: u64,
 from: u64,
 to: u64,
 },
 CrossBranchDiff {
 branch_a: u64,
 branch_b: u64,
 },
 EpistemicQuery {
 agent_id: u64,
 fact_id: u64,
 },
}

/// Per §18 line 1811-1816 mix.
const QUERY_MIX_BLOCK: [(&str, u32); 5] = [
 ("point", 40),
 ("three_hop", 30),
 ("temporal", 15),
 ("cross_branch", 10),
 ("epistemic", 5),
];

/// Generate a deterministic query trace of length `count`. Round-robin in
/// 100-query blocks (Priority 0 (e) decision); within a block kinds are
/// emitted in fixed order so stage 2A and stage 2B replay the same trace.
pub fn generate_query_trace(workload: &Workload, count: usize, seed: u64) -> Vec<Query> {
 let mut rng = ChaCha20Rng::seed_from_u64(seed);
 let mut trace = Vec::with_capacity(count);
 let n_entities = workload.entities.len();
 let n_branches = workload.branches.len();
 let n_facts = workload.facts.len();
 let n_agents = workload.agents.len();

 let mut emitted = 0usize;
 while emitted < count {
 for (kind, share) in QUERY_MIX_BLOCK {
 for _ in 0..share {
  if emitted >= count {
  return trace;
  }
  let q = match kind {
  "point" => Query::PointLookup {
  entity_id: workload.entities[rng.gen_range(0..n_entities)].id,
  branch_id: workload.branches[rng.gen_range(0..n_branches)].id,
  },
  "three_hop" => Query::ThreeHopGraph {
  start: workload.entities[rng.gen_range(0..n_entities)].id,
  branch_id: workload.branches[rng.gen_range(0..n_branches)].id,
  },
  "temporal" => {
  let from: u64 = rng.gen_range(0..1_000_000);
  let span: u64 = rng.gen_range(1..100_000);
  Query::TemporalRange {
   entity_id: workload.entities[rng.gen_range(0..n_entities)].id,
   from,
   to: from + span,
  }
  }
  "cross_branch" => {
  let a = workload.branches[rng.gen_range(0..n_branches)].id;
  let b = workload.branches[rng.gen_range(0..n_branches)].id;
  Query::CrossBranchDiff {
   branch_a: a,
   branch_b: b,
  }
  }
  "epistemic" => Query::EpistemicQuery {
  agent_id: workload.agents[rng.gen_range(0..n_agents)].id,
  fact_id: workload.facts[rng.gen_range(0..n_facts)].id,
  },
  _ => unreachable!(),
  };
  trace.push(q);
  emitted += 1;
 }
 }
 }
 trace
}

// ─── Determinism verification (§18 incompleteness gate prerequisite) ─────────

/// Re-generate from the same config and assert byte-equality. Returns the
/// regenerated workload on success.
pub fn verify_determinism(config: &WorkloadConfig, original: &Workload) -> Result<Workload, String> {
 let regen = generate(config);
 if regen != *original {
 return Err("regeneration produced different output — determinism broken".to_string());
 }
 Ok(regen)
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
 use super::*;

 fn small_config(seed: u64) -> WorkloadConfig {
 WorkloadConfig {
 protocol_version: PROTOCOL_VERSION,
 assets: 100,
 facts: 50,
 branches: 20,
 agents: 10,
 entity_dist: EntityDistribution {
  person: 5,
  place: 3,
  faction: 2,
  event: 4,
  item: 4,
  concept: 2,
 },
 branch_tree_depth_avg: 3,
 branch_tree_depth_max: 10,
 facts_per_asset_avg: 3,
 seed,
 }
 }

 #[test]
 fn determinism_byte_equal() {
 let cfg = small_config(42);
 let a = generate(&cfg);
 let b = generate(&cfg);
 assert_eq!(a, b);
 }

 #[test]
 fn different_seed_diverges() {
 let a = generate(&small_config(42));
 let b = generate(&small_config(43));
 assert_ne!(a.assets, b.assets);
 }

 #[test]
 fn scale_targets_met() {
 let cfg = small_config(7);
 let w = generate(&cfg);
 assert!(w.scale_report().all_ok());
 assert_eq!(w.assets.len(), cfg.assets);
 assert_eq!(w.facts.len(), cfg.facts);
 assert_eq!(w.branches.len(), cfg.branches);
 assert_eq!(w.agents.len(), cfg.agents);
 assert_eq!(w.entities.len(), cfg.entity_dist.total());
 }

 #[test]
 fn branch_tree_invariants() {
 let cfg = small_config(11);
 let w = generate(&cfg);
 assert_eq!(w.branches[0].parent, None);
 assert_eq!(w.branches[0].depth, 0);
 for b in &w.branches[1..] {
 assert!(b.parent.is_some());
 let parent_id = b.parent.unwrap();
 let parent = w.branches.iter().find(|p| p.id == parent_id).unwrap();
 assert_eq!(b.depth, parent.depth + 1);
 assert!(b.depth <= cfg.branch_tree_depth_max);
 }
 }

 #[test]
 fn query_trace_mix_shape() {
 let cfg = small_config(1);
 let w = generate(&cfg);
 let trace = generate_query_trace(&w, 1000, 999);
 assert_eq!(trace.len(), 1000);
 let mut counts = [0u32; 5];
 for q in &trace {
 match q {
  Query::PointLookup { .. } => counts[0] += 1,
  Query::ThreeHopGraph { .. } => counts[1] += 1,
  Query::TemporalRange { .. } => counts[2] += 1,
  Query::CrossBranchDiff { .. } => counts[3] += 1,
  Query::EpistemicQuery { .. } => counts[4] += 1,
 }
 }
 // Round-robin in 100-blocks → exact ratios per 100 emissions.
 assert_eq!(counts[0], 400);
 assert_eq!(counts[1], 300);
 assert_eq!(counts[2], 150);
 assert_eq!(counts[3], 100);
 assert_eq!(counts[4], 50);
 }

 #[test]
 fn query_trace_determinism() {
 let w = generate(&small_config(1));
 let a = generate_query_trace(&w, 500, 17);
 let b = generate_query_trace(&w, 500, 17);
 assert_eq!(a, b);
 }

 #[test]
 fn fact_refs_resolve() {
 let cfg = small_config(13);
 let w = generate(&cfg);
 let entity_ids: std::collections::HashSet<u64> =
 w.entities.iter().map(|e| e.id).collect();
 for f in &w.facts {
 assert!(entity_ids.contains(&f.subject));
 assert!(entity_ids.contains(&f.object));
 }
 let fact_ids: std::collections::HashSet<u64> = w.facts.iter().map(|f| f.id).collect();
 let branch_ids: std::collections::HashSet<u64> =
 w.branches.iter().map(|b| b.id).collect();
 for a in &w.assets {
 assert!(branch_ids.contains(&a.branch_id));
 for fid in &a.facts_referenced {
  assert!(fact_ids.contains(fid));
 }
 }
 }
}
