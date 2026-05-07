//! Phase -1A stage 2C §44 row-per-encounter criterion harness.
//!
//! §18 line 1846: write throughput at the encounter scale + cascade
//! invalidation scope. §18 line 1903 trigger fires if normalized throughput
//! drops below the blob baseline.

use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use schema_bench::{generate_encounters, EncounterStore};
use tempfile::TempDir;

const N_RUNS: u32 = 5;
const N_AGENTS: u32 = 100;
const N_FACTS: u32 = 100;

struct Fixture {
 _dir: TempDir,
 store: EncounterStore,
 encounters: Vec<(u32, u32, u32, u32)>,
}

impl Fixture {
 fn build() -> Self {
 let dir = TempDir::new().expect("tempdir");
 let store = EncounterStore::open(dir.path(), 64).expect("open");
 let encounters = generate_encounters(N_RUNS, N_FACTS, N_AGENTS, 7);
 Fixture {
 _dir: dir,
 store,
 encounters,
 }
 }
}

fn bench_blob_append(c: &mut Criterion) {
 let fixture = Fixture::build();
 let mut rng = ChaCha20Rng::seed_from_u64(0xAA_BB_CC_DD);
 let mut group = c.benchmark_group("encounter_write");
 group.sample_size(1000);
 group.measurement_time(Duration::from_secs(10));
 group.throughput(Throughput::Elements(1));
 group.bench_function("blob_append", |b| {
 b.iter(|| {
 let &(run, agent, fact, count) =
  &fixture.encounters[rng.gen_range(0..fixture.encounters.len())];
 fixture.store.blob_append(run, agent, fact, count).unwrap();
 })
 });
 group.bench_function("norm_insert", |b| {
 b.iter(|| {
 let &(run, agent, fact, count) =
  &fixture.encounters[rng.gen_range(0..fixture.encounters.len())];
 fixture.store.norm_insert(run, agent, fact, count).unwrap();
 })
 });
 group.finish();
}

fn bench_invalidate_scope(c: &mut Criterion) {
 // Pre-populate both layouts for the cascade-scope query.
 let fixture = Fixture::build();
 for &(run, agent, fact, count) in &fixture.encounters {
 fixture.store.blob_append(run, agent, fact, count).unwrap();
 }
 fixture.store.norm_bulk(&fixture.encounters).unwrap();
 fixture.store.flush().unwrap();

 let mut rng = ChaCha20Rng::seed_from_u64(0xBC_DE_FF_01);
 let mut group = c.benchmark_group("encounter_invalidate");
 group.sample_size(1000);
 group.measurement_time(Duration::from_secs(10));
 group.bench_function("blob_scope", |b| {
 b.iter(|| {
 let run = rng.gen_range(0..N_RUNS);
 let agent = rng.gen_range(0..N_AGENTS);
 let _ = fixture.store.blob_invalidate_scope(run, agent).unwrap();
 })
 });
 group.bench_function("norm_scope", |b| {
 b.iter(|| {
 let run = rng.gen_range(0..N_RUNS);
 let agent = rng.gen_range(0..N_AGENTS);
 let _ = fixture.store.norm_invalidate_scope(run, agent).unwrap();
 })
 });
 group.finish();
}

criterion_group!(
 name = encounter;
 config = Criterion::default()
 .warm_up_time(Duration::from_secs(2))
 .noise_threshold(0.05);
 targets = bench_blob_append, bench_invalidate_scope,
);
criterion_main!(encounter);
