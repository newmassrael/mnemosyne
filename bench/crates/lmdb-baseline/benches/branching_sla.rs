//! Phase -1A stage 2D LMDB criterion harness.

use std::time::Duration;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use lmdb_baseline::{populate_full, synthesize_linear_chain, BranchMeta, BranchStore};
use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use tempfile::TempDir;
use workload_gen::{generate, Workload};

struct Fixture {
 _dir: TempDir,
 store: BranchStore,
 workload: Workload,
 chain_depths: Vec<(u32, u64)>,
 next_id: u64,
}

impl Fixture {
 fn build() -> Self {
 let workload = generate(&workload_gen::default_config());
 let dir = TempDir::new().expect("tempdir");
 let store = BranchStore::open(dir.path()).expect("open store");
 populate_full(&store, &workload, 0.005).expect("populate");
 let mut next_id = 1_000_000u64;
 let mut chain_depths = Vec::new();
 for d in [1u32, 5, 10, 20, 50] {
 let leaf = synthesize_linear_chain(&store, next_id, d, "chain").expect("chain");
 chain_depths.push((d, leaf));
 next_id += d as u64 + 1;
 }
 Fixture {
 _dir: dir,
 store,
 workload,
 chain_depths,
 next_id,
 }
 }

 fn entity_ids(&self) -> Vec<u64> {
 self.workload.entities.iter().map(|e| e.id).collect()
 }

 fn non_root_branches(&self) -> Vec<u64> {
 self.workload
 .branches
 .iter()
 .filter(|b| b.id != 0)
 .map(|b| b.id)
 .collect()
 }
}

fn bench_branch_creation(c: &mut Criterion) {
 let fixture = Fixture::build();
 let mut next_id = 5_000_000u64;
 let mut group = c.benchmark_group("branch_creation");
 group.sample_size(1000);
 group.measurement_time(Duration::from_secs(8));
 group.throughput(Throughput::Elements(1));
 group.bench_function("put_branch_meta", |b| {
 b.iter(|| {
 let id = next_id;
 next_id += 1;
 fixture
  .store
  .put_branch(
  id,
  &BranchMeta {
  parent: Some(0),
  depth: 1,
  label: "create".to_string(),
  },
  )
  .unwrap();
 })
 });
 group.finish();
 drop(fixture);
}

fn bench_point_query_chain_depths(c: &mut Criterion) {
 let fixture = Fixture::build();
 let entities = fixture.entity_ids();
 let mut group = c.benchmark_group("point_query_chain_depth");
 group.sample_size(1000);
 group.measurement_time(Duration::from_secs(10));
 for (depth, leaf) in fixture.chain_depths.iter().copied() {
 let mut rng = ChaCha20Rng::seed_from_u64(0xCAFE_BABE ^ depth as u64);
 group.bench_with_input(BenchmarkId::from_parameter(depth), &leaf, |b, &leaf| {
 b.iter(|| {
  let eid = entities[rng.gen_range(0..entities.len())];
  let _ = fixture.store.point_query(leaf, eid, 1_000_000).unwrap();
 })
 });
 }
 group.finish();
 drop(fixture);
}

fn bench_cross_branch_diff(c: &mut Criterion) {
 let fixture = Fixture::build();
 let branches = fixture.non_root_branches();
 let entities = fixture.entity_ids();
 let sample: Vec<u64> = entities.into_iter().take(500).collect();
 let mut rng = ChaCha20Rng::seed_from_u64(0x9001_9001);
 let mut group = c.benchmark_group("cross_branch_diff");
 group.sample_size(100);
 group.measurement_time(Duration::from_secs(15));
 group.bench_function("heads_500_entities", |b| {
 b.iter(|| {
 let a = branches[rng.gen_range(0..branches.len())];
 let bb = branches[rng.gen_range(0..branches.len())];
 let _ = fixture
  .store
  .cross_branch_diff(a, bb, &sample, 1_000_000)
  .unwrap();
 })
 });
 group.finish();
 drop(fixture);
}

fn bench_save_tree_n100(c: &mut Criterion) {
 let fixture = Fixture::build();
 let entities = fixture.entity_ids();
 let sample: Vec<u64> = entities.into_iter().take(500).collect();
 let base = 6_000_000u64;
 fixture
 .store
 .put_branch(
 base,
 &BranchMeta {
  parent: Some(0),
  depth: 1,
  label: "save_base".to_string(),
 },
 )
 .unwrap();
 let mut children = Vec::with_capacity(100);
 for i in 0..100 {
 let id = base + 1 + i as u64;
 fixture
 .store
 .put_branch(
  id,
  &BranchMeta {
  parent: Some(base),
  depth: 2,
  label: format!("save_{}", i),
  },
 )
 .unwrap();
 children.push(id);
 }
 let mut rng = ChaCha20Rng::seed_from_u64(0x5A_5A_5A_5A);
 let mut group = c.benchmark_group("save_tree_n100");
 group.sample_size(100);
 group.measurement_time(Duration::from_secs(15));
 group.bench_function("cross_branch_diff", |b| {
 b.iter(|| {
 let a = children[rng.gen_range(0..children.len())];
 let bb = children[rng.gen_range(0..children.len())];
 let _ = fixture
  .store
  .cross_branch_diff(a, bb, &sample, 1_000_000)
  .unwrap();
 })
 });
 group.finish();
 drop(fixture);
}

fn bench_flatten_recovery(c: &mut Criterion) {
 let fixture = Fixture::build();
 let entities = fixture.entity_ids();
 let sample: Vec<u64> = entities.into_iter().take(500).collect();
 let depth_20 = fixture
 .chain_depths
 .iter()
 .find(|(d, _)| *d == 20)
 .copied()
 .expect("depth-20 chain");
 let new_branch = 7_000_000u64;
 fixture
 .store
 .flatten_branch(depth_20.1, &sample, 1_000_000, new_branch, "flat".into())
 .unwrap();

 let mut rng = ChaCha20Rng::seed_from_u64(0xF1A7_F1A7);
 let mut group = c.benchmark_group("flatten_post_recovery");
 group.sample_size(1000);
 group.measurement_time(Duration::from_secs(8));
 group.bench_function("point_query_after_flatten", |b| {
 b.iter(|| {
 let eid = sample[rng.gen_range(0..sample.len())];
 let _ = fixture
  .store
  .point_query(new_branch, eid, 1_000_000)
  .unwrap();
 })
 });
 group.finish();
 let _ = fixture.next_id;
 drop(fixture);
}

criterion_group!(
 name = sla;
 config = Criterion::default()
 .warm_up_time(Duration::from_secs(2))
 .noise_threshold(0.05);
 targets =
 bench_branch_creation,
 bench_point_query_chain_depths,
 bench_cross_branch_diff,
 bench_save_tree_n100,
 bench_flatten_recovery,
);
criterion_main!(sla);
