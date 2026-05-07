//! Phase -1A stage 2C asset_refs blob vs normalized criterion harness.
//!
//! §18 line 1844: hot fact write contention + read amplification under the
//! sample 1000+ / jitter < 30 % gate.

use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};
use schema_bench::{generate_skewed_refs, hot_facts, AssetRefsStore};
use tempfile::TempDir;

struct Fixture {
 _dir: TempDir,
 store: AssetRefsStore,
 hot_fact_id: u64,
 next_asset: u64,
}

impl Fixture {
 fn build() -> Self {
 let n_facts = 5_000usize;
 let n_refs = 25_000usize;
 let refs = generate_skewed_refs(n_facts, n_refs, 7);
 let dir = TempDir::new().expect("tempdir");
 let store = AssetRefsStore::open(dir.path(), 64).expect("open store");
 for (idx, list) in refs.iter().enumerate() {
 if list.is_empty() {
  continue;
 }
 let fact_id = (idx as u64) + 1;
 store.blob_set(fact_id, list).expect("blob_set");
 store.norm_bulk(fact_id, list).expect("norm_bulk");
 }
 store.flush().expect("flush");
 let hot = hot_facts(&refs, 0.01);
 let hot_fact_idx = hot[0];
 let hot_fact_id = (hot_fact_idx as u64) + 1;
 Fixture {
 _dir: dir,
 store,
 hot_fact_id,
 next_asset: 10_000_000,
 }
 }
}

fn bench_blob_append(c: &mut Criterion) {
 let mut fixture = Fixture::build();
 let mut group = c.benchmark_group("asset_refs_write");
 group.sample_size(1000);
 group.measurement_time(Duration::from_secs(10));
 group.bench_function("blob_append_hot_fact", |b| {
 b.iter(|| {
 let aid = fixture.next_asset;
 fixture.next_asset += 1;
 fixture.store.blob_append(fixture.hot_fact_id, aid).unwrap();
 })
 });
 group.bench_function("norm_insert_hot_fact", |b| {
 b.iter(|| {
 let aid = fixture.next_asset;
 fixture.next_asset += 1;
 fixture
  .store
  .norm_insert(fixture.hot_fact_id, aid)
  .unwrap();
 })
 });
 group.finish();
}

fn bench_read_amp(c: &mut Criterion) {
 let fixture = Fixture::build();
 let mut group = c.benchmark_group("asset_refs_read");
 group.sample_size(1000);
 group.measurement_time(Duration::from_secs(8));
 group.bench_function("blob_read_hot_fact", |b| {
 b.iter(|| {
 let _ = fixture.store.blob_read(fixture.hot_fact_id).unwrap();
 })
 });
 group.bench_function("norm_read_hot_fact", |b| {
 b.iter(|| {
 let _ = fixture.store.norm_read(fixture.hot_fact_id).unwrap();
 })
 });
 group.finish();
}

criterion_group!(
 name = asset_refs;
 config = Criterion::default()
 .warm_up_time(Duration::from_secs(2))
 .noise_threshold(0.05);
 targets = bench_blob_append, bench_read_amp,
);
criterion_main!(asset_refs);
