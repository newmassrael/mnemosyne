//! Phase -1A stage 2C schema-shape micro-bench (DESIGN.md §18 line 1840-1849).
//!
//! Delivered: asset_refs blob vs normalized (§18 line 1844, hot-fact write
//! cost + read amplification covering the §4 schema decision), and
//! fixed-width prefix scan correctness (§18 line 1845, composite-key encoding
//! of 8-B big-endian branch ids does not let a prefix scan for one branch hit
//! rows belonging to another even when ids differ by orders of magnitude).
//!
//! Deferred: §44 row-per-encounter cascade granularity, §11 provenance
//! marker query / retraction overhead.
//!
//! Cross-language emit equivalence (5-language conformance) is checked
//! statically against reference byte vectors hand-written to mirror what each
//! target language's standard library produces. Real Phase -1B codegen output
//! supersedes this when it lands.

use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use byteorder::{BigEndian, ByteOrder};
use rocksdb::{
 BlockBasedOptions, Cache, ColumnFamilyDescriptor, DBCompressionType, Direction, IteratorMode,
 Options, WriteBatch, DB,
};

pub mod provenance;
pub use provenance::{
 populate_fixture as populate_provenance_fixture, BaselineRow, ProvenanceKind, ProvenanceRow,
 ProvenanceStore,
};

pub mod encounter;
pub use encounter::{generate_encounters, EncounterStore};

// ─── Codec primitives ────────────────────────────────────────────────────────

/// 8 B big-endian fixed-width encoding for `u64` ids. Used by both
/// `branch_id` and `fact_id` slots in composite keys (§40 codec).
pub fn fixed_be_u64(id: u64) -> [u8; 8] {
 let mut buf = [0u8; 8];
 BigEndian::write_u64(&mut buf, id);
 buf
}

/// LEB128 varint encoding for `u64`. Returned vector is 1-10 B depending on
/// magnitude. Included here only so the correctness micro-bench can show *why*
/// fixed-width is the chosen path: varint produces variable-length prefixes,
/// which makes lexicographic prefix scans coupled to the integer comparator.
pub fn leb128_u64(mut id: u64) -> Vec<u8> {
 let mut out = Vec::with_capacity(10);
 loop {
 let byte = (id & 0x7F) as u8;
 id >>= 7;
 if id == 0 {
 out.push(byte);
 return out;
 }
 out.push(byte | 0x80);
 }
}

// ─── Asset-refs layout: blob vs normalized ───────────────────────────────────

pub const CF_BLOB: &str = "fact_to_assets_blob";
pub const CF_NORM: &str = "asset_refs_normalized";

pub struct AssetRefsStore {
 pub db: Arc<DB>,
 _cache: Cache,
}

impl AssetRefsStore {
 pub fn open(path: &Path, block_cache_mib: usize) -> Result<Self> {
 let cache = Cache::new_lru_cache(block_cache_mib * 1024 * 1024);

 let mut block_opts = BlockBasedOptions::default();
 block_opts.set_block_cache(&cache);
 block_opts.set_bloom_filter(10.0, false);
 block_opts.set_block_size(16 * 1024);
 block_opts.set_cache_index_and_filter_blocks(true);
 block_opts.set_pin_l0_filter_and_index_blocks_in_cache(true);

 let mut cf_opts = Options::default();
 cf_opts.set_block_based_table_factory(&block_opts);
 cf_opts.set_compression_type(DBCompressionType::Lz4);

 let cfs = vec![
 ColumnFamilyDescriptor::new(CF_BLOB, cf_opts.clone()),
 ColumnFamilyDescriptor::new(CF_NORM, cf_opts),
 ];
 let mut db_opts = Options::default();
 db_opts.create_if_missing(true);
 db_opts.create_missing_column_families(true);
 db_opts.set_max_background_jobs(4);

 let db = DB::open_cf_descriptors(&db_opts, path, cfs)
 .with_context(|| format!("open rocksdb at {}", path.display()))?;
 Ok(Self {
 db: Arc::new(db),
 _cache: cache,
 })
 }

 /// Append `asset_id` to fact `fact_id`'s blob list. Re-reads, decodes,
 /// inserts (preserving sorted-set semantics), encodes, writes back.
 pub fn blob_append(&self, fact_id: u64, asset_id: u64) -> Result<()> {
 let cf = self
 .db
 .cf_handle(CF_BLOB)
 .ok_or_else(|| anyhow!("missing CF {}", CF_BLOB))?;
 let key = fixed_be_u64(fact_id);
 let mut current: Vec<u64> = match self.db.get_cf(cf, key)? {
 Some(buf) => bincode::deserialize(&buf)?,
 None => Vec::new(),
 };
 if current.binary_search(&asset_id).is_err() {
 let pos = current.partition_point(|x| *x < asset_id);
 current.insert(pos, asset_id);
 }
 let buf = bincode::serialize(&current)?;
 self.db.put_cf(cf, key, buf)?;
 Ok(())
 }

 /// Bulk-load blob for `fact_id` (overwrites). Used during fixture setup.
 pub fn blob_set(&self, fact_id: u64, asset_ids: &[u64]) -> Result<()> {
 let cf = self
 .db
 .cf_handle(CF_BLOB)
 .ok_or_else(|| anyhow!("missing CF {}", CF_BLOB))?;
 let key = fixed_be_u64(fact_id);
 let buf = bincode::serialize(asset_ids)?;
 self.db.put_cf(cf, key, buf)?;
 Ok(())
 }

 /// Read all asset ids referencing `fact_id` (blob layout).
 pub fn blob_read(&self, fact_id: u64) -> Result<Vec<u64>> {
 let cf = self
 .db
 .cf_handle(CF_BLOB)
 .ok_or_else(|| anyhow!("missing CF {}", CF_BLOB))?;
 let key = fixed_be_u64(fact_id);
 match self.db.get_cf(cf, key)? {
 Some(buf) => Ok(bincode::deserialize(&buf)?),
 None => Ok(Vec::new()),
 }
 }

 /// Insert one row in the normalized layout.
 pub fn norm_insert(&self, fact_id: u64, asset_id: u64) -> Result<()> {
 let cf = self
 .db
 .cf_handle(CF_NORM)
 .ok_or_else(|| anyhow!("missing CF {}", CF_NORM))?;
 let mut key = [0u8; 16];
 BigEndian::write_u64(&mut key[0..8], fact_id);
 BigEndian::write_u64(&mut key[8..16], asset_id);
 self.db.put_cf(cf, key, [])?;
 Ok(())
 }

 /// Bulk insert normalized rows.
 pub fn norm_bulk(&self, fact_id: u64, asset_ids: &[u64]) -> Result<()> {
 let cf = self
 .db
 .cf_handle(CF_NORM)
 .ok_or_else(|| anyhow!("missing CF {}", CF_NORM))?;
 let mut batch = WriteBatch::default();
 for &aid in asset_ids {
 let mut key = [0u8; 16];
 BigEndian::write_u64(&mut key[0..8], fact_id);
 BigEndian::write_u64(&mut key[8..16], aid);
 batch.put_cf(cf, key, []);
 }
 self.db.write(batch)?;
 Ok(())
 }

 /// Prefix-scan all asset ids for `fact_id` (normalized layout).
 pub fn norm_read(&self, fact_id: u64) -> Result<Vec<u64>> {
 let cf = self
 .db
 .cf_handle(CF_NORM)
 .ok_or_else(|| anyhow!("missing CF {}", CF_NORM))?;
 let mut prefix = [0u8; 8];
 BigEndian::write_u64(&mut prefix, fact_id);
 let mut out = Vec::new();
 let iter = self
 .db
 .iterator_cf(cf, IteratorMode::From(&prefix, Direction::Forward));
 for item in iter {
 let (k, _) = item?;
 if k.len() != 16 || k[..8] != prefix {
  break;
 }
 out.push(BigEndian::read_u64(&k[8..16]));
 }
 Ok(out)
 }

 pub fn flush(&self) -> Result<()> {
 self.db.flush()?;
 Ok(())
 }
}

// ─── Zipf-skewed asset-ref fixture ───────────────────────────────────────────

/// Generate a skewed reference list: total `n_refs` (fact, asset) pairs over
/// `n_facts` facts, with Zipf-1.0-ish distribution so the top 1 % of facts
/// hold a disproportionate share. Returns one `Vec<u64>` per fact.
pub fn generate_skewed_refs(n_facts: usize, n_refs: usize, seed: u64) -> Vec<Vec<u64>> {
 use rand::Rng;
 use rand::SeedableRng;
 use rand_chacha::ChaCha20Rng;

 let mut rng = ChaCha20Rng::seed_from_u64(seed);
 // Weight i (1-indexed) ∝ 1 / i^s with s = 1.0 (Zipf), then sample.
 let weights: Vec<f64> = (1..=n_facts as u64).map(|i| 1.0 / i as f64).collect();
 let total: f64 = weights.iter().sum();
 // Cumulative.
 let mut cum = Vec::with_capacity(n_facts);
 let mut acc = 0.0;
 for w in &weights {
 acc += *w;
 cum.push(acc / total);
 }
 let mut refs: Vec<Vec<u64>> = vec![Vec::new(); n_facts];
 let mut next_asset = 1u64;
 for _ in 0..n_refs {
 let u: f64 = rng.gen();
 let idx = cum.partition_point(|c| *c < u);
 let aid = next_asset;
 next_asset += 1;
 refs[idx.min(n_facts - 1)].push(aid);
 }
 for v in refs.iter_mut() {
 v.sort_unstable();
 v.dedup();
 }
 refs
}

/// Identify the top-`pct` (e.g. 0.01 = 1 %) hottest facts by reference count.
pub fn hot_facts(refs: &[Vec<u64>], pct: f64) -> Vec<usize> {
 let n_keep = ((refs.len() as f64 * pct).ceil() as usize).max(1);
 let mut by_size: Vec<(usize, usize)> = refs.iter().enumerate().map(|(i, v)| (i, v.len())).collect();
 by_size.sort_by(|a, b| b.1.cmp(&a.1));
 by_size.into_iter().take(n_keep).map(|(i, _)| i).collect()
}

// ─── Tests: codec correctness + cross-language byte equality + prefix scan ───

#[cfg(test)]
mod codec_correctness {
 use super::*;

 #[test]
 fn fixed_width_byte_equality_with_reference_emit() {
 // Hand-written reference bytes for what each language stdlib emits for
 // `branch_id.to_be_bytes()` (Rust), `id.to_bytes(8, "big")` (Python),
 // `ByteBuffer.allocate(8).order(BIG_ENDIAN).putLong(id)` (Kotlin/Java),
 // `htobe64(id)` then write 8 bytes (C++), `Fixed64` BE wire (protobuf
 // when used with manual BE container — protobuf wire is LE, so the
 // contract here is that the *application-layer* key bytes are BE
 // regardless of language).
 let cases: &[(u64, [u8; 8])] = &[
 (0, [0, 0, 0, 0, 0, 0, 0, 0]),
 (1, [0, 0, 0, 0, 0, 0, 0, 1]),
 (10, [0, 0, 0, 0, 0, 0, 0, 0x0A]),
 (100, [0, 0, 0, 0, 0, 0, 0, 0x64]),
 (255, [0, 0, 0, 0, 0, 0, 0, 0xFF]),
 (256, [0, 0, 0, 0, 0, 0, 0x01, 0x00]),
 (65_535, [0, 0, 0, 0, 0, 0, 0xFF, 0xFF]),
 (1_000_000, [0, 0, 0, 0, 0, 0x0F, 0x42, 0x40]),
 (u64::MAX, [0xFF; 8]),
 ];
 for (id, expected) in cases {
 assert_eq!(fixed_be_u64(*id), *expected, "id = {}", id);
 }
 }

 #[test]
 fn fixed_width_lex_ordering_matches_numeric_ordering() {
 // Fundamental property: fixed BE encoding lex order == numeric order.
 let mut prev = fixed_be_u64(0);
 for i in [1u64, 10, 100, 1_000, 1_000_000, u64::MAX / 2, u64::MAX] {
 let cur = fixed_be_u64(i);
 assert!(prev < cur, "{:?} should precede {:?} ({} < {})", prev, cur, 0, i);
 prev = cur;
 }
 }

 #[test]
 fn varint_emit_self_consistent() {
 // Round-trip: encode then decode reproduces the input. Used as the
 // "varint exists and is well-defined" reference, but varint is not the
 // chosen scheme — see prefix_scan_correctness below.
 for &id in &[0u64, 1, 127, 128, 255, 16_383, 16_384, 1_000_000, u64::MAX] {
 let buf = leb128_u64(id);
 let decoded = decode_leb128(&buf).unwrap();
 assert_eq!(decoded, id, "round-trip failed for {}", id);
 }
 }

 fn decode_leb128(buf: &[u8]) -> Result<u64, &'static str> {
 let mut result: u64 = 0;
 let mut shift = 0;
 for (i, b) in buf.iter().enumerate() {
 if i >= 10 {
  return Err("overflow");
 }
 result |= ((b & 0x7F) as u64) << shift;
 if b & 0x80 == 0 {
  return Ok(result);
 }
 shift += 7;
 }
 Err("truncated")
 }

 #[test]
 fn fixed_width_prefix_never_overlaps_distinct_branches() {
 // Build composite keys (branch_id || entity_id) for branches that span
 // orders of magnitude and verify prefix scan for one branch never
 // returns rows from another, which is the §18 line 1845 correctness
 // gate.
 let dir = tempfile::TempDir::new().unwrap();
 let store = AssetRefsStore::open(dir.path(), 16).unwrap();
 let cf = store.db.cf_handle(CF_NORM).unwrap();
 let branches: &[u64] = &[1, 10, 100, 1_000, 10_000, 1_000_000, u64::MAX / 2];
 let entities: &[u64] = &[1, 2, 3, 4, 5];
 for &b in branches {
 for &e in entities {
  let mut key = [0u8; 16];
  BigEndian::write_u64(&mut key[0..8], b);
  BigEndian::write_u64(&mut key[8..16], e);
  store.db.put_cf(cf, key, []).unwrap();
 }
 }
 for &b in branches {
 let got = store.norm_read(b).unwrap();
 assert_eq!(got.len(), entities.len(), "branch {} row count", b);
 assert_eq!(got, entities.to_vec(), "branch {} row contents", b);
 }
 }
}

#[cfg(test)]
mod asset_refs_correctness {
 use super::*;

 #[test]
 fn blob_and_normalized_resolve_equivalently() {
 let dir = tempfile::TempDir::new().unwrap();
 let store = AssetRefsStore::open(dir.path(), 16).unwrap();
 let refs = generate_skewed_refs(50, 200, 7);
 for (idx, list) in refs.iter().enumerate() {
 let fact_id = (idx as u64) + 1;
 store.blob_set(fact_id, list).unwrap();
 store.norm_bulk(fact_id, list).unwrap();
 }
 for (idx, expected) in refs.iter().enumerate() {
 let fact_id = (idx as u64) + 1;
 let blob = store.blob_read(fact_id).unwrap();
 let norm = store.norm_read(fact_id).unwrap();
 assert_eq!(&blob, expected, "blob mismatch fact {}", fact_id);
 assert_eq!(&norm, expected, "norm mismatch fact {}", fact_id);
 }
 }

 #[test]
 fn blob_append_preserves_sorted_set() {
 let dir = tempfile::TempDir::new().unwrap();
 let store = AssetRefsStore::open(dir.path(), 16).unwrap();
 store.blob_set(42, &[1, 5, 9]).unwrap();
 store.blob_append(42, 3).unwrap();
 store.blob_append(42, 9).unwrap(); // duplicate
 store.blob_append(42, 11).unwrap();
 let got = store.blob_read(42).unwrap();
 assert_eq!(got, vec![1, 3, 5, 9, 11]);
 }

 #[test]
 fn skewed_refs_concentrate_at_head() {
 let refs = generate_skewed_refs(1000, 10_000, 1);
 let hot = hot_facts(&refs, 0.01);
 let hot_count: usize = hot.iter().map(|&i| refs[i].len()).sum();
 let total: usize = refs.iter().map(|v| v.len()).sum();
 let ratio = hot_count as f64 / total as f64;
 // Zipf-1.0 over 1000 buckets: top 1% (10 buckets) should hold > 30%.
 assert!(
 ratio > 0.30,
 "top-1% only holds {:.1}%, expected > 30%",
 ratio * 100.0
 );
 }
}
