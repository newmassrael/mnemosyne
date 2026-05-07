//! Phase -1A stage 2C §11 provenance overhead criterion harness.
//!
//! §18 line 1847 + 1904 trigger: ≥ 10 % overhead → split provenance into a
//! separate CF.

use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};
use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use schema_bench::{populate_provenance_fixture, ProvenanceStore};
use tempfile::TempDir;

const N_EXPLICIT: usize = 1_000;
const DERIVED_PER: usize = 10;

struct Fixture {
 _dir: TempDir,
 store: ProvenanceStore,
 n_total: u64,
}

impl Fixture {
 fn build() -> Self {
 let dir = TempDir::new().expect("tempdir");
 let store = ProvenanceStore::open(dir.path(), 64).expect("open store");
 populate_provenance_fixture(&store, N_EXPLICIT, DERIVED_PER).expect("populate");
 let n_total = (N_EXPLICIT * (1 + DERIVED_PER)) as u64;
 Fixture {
 _dir: dir,
 store,
 n_total,
 }
 }
}

fn bench_point_query(c: &mut Criterion) {
 let fixture = Fixture::build();
 let mut rng = ChaCha20Rng::seed_from_u64(0x1111_2222);
 let mut group = c.benchmark_group("provenance_point_query");
 group.sample_size(1000);
 group.measurement_time(Duration::from_secs(8));
 group.bench_function("baseline_get", |b| {
 b.iter(|| {
 let id = rng.gen_range(1..=fixture.n_total);
 let _ = fixture.store.get_baseline(id).unwrap();
 })
 });
 group.bench_function("provenance_get", |b| {
 b.iter(|| {
 let id = rng.gen_range(1..=fixture.n_total);
 let _ = fixture.store.get_provenance(id).unwrap();
 })
 });
 group.finish();
}

fn bench_cascade_scan(c: &mut Criterion) {
 let fixture = Fixture::build();
 let mut rng = ChaCha20Rng::seed_from_u64(0x3333_4444);
 let mut group = c.benchmark_group("provenance_cascade");
 group.sample_size(100);
 group.measurement_time(Duration::from_secs(15));
 group.bench_function("scan_for_derived_from", |b| {
 b.iter(|| {
 // Pick a random source row id (every (1 + DERIVED_PER)-th id is
 // an explicit source by construction).
 let stride = (1 + DERIVED_PER) as u64;
 let max_src = N_EXPLICIT as u64;
 let src_idx = rng.gen_range(0..max_src);
 let src_id = src_idx * stride + 1;
 let _ = fixture.store.cascade_scan(src_id).unwrap();
 })
 });
 group.finish();
}

criterion_group!(
 name = provenance;
 config = Criterion::default()
 .warm_up_time(Duration::from_secs(2))
 .noise_threshold(0.05);
 targets = bench_point_query, bench_cascade_scan,
);
criterion_main!(provenance);
