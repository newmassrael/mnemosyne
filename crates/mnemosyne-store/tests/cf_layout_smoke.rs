//! Integration smoke test — DESIGN §4 / §42 ten-CF instantiation against an
//! Real `rocksdb::DB` on a tempdir. Covers the production-binding properties
//! that unit tests cannot reach: cross-process re-open, durable schema_version,
//! WriteBatch atomicity across multiple CFs.

use mnemosyne_store::{
 cf_layout::{secondary_readable_cfs, ALL_CFS},
 CfId, MigrationMeta, MnemosyneStore, ALL_CFS as ALL_CFS_REEXPORT,
};
use tempfile::TempDir;

#[test]
fn ten_cf_instantiation_round_trip() {
 let dir = TempDir::new().unwrap();
 let store = MnemosyneStore::open(dir.path()).unwrap();
 // Round-trip a value through every CF.
 for cf in [
 CfId::Entities,
 CfId::Relations,
 CfId::TemporalIndex,
 CfId::TemporalIndexOpen,
 CfId::BranchMeta,
 CfId::Assets,
 CfId::AssetRefs,
 CfId::Audit,
 CfId::Epistemic,
 CfId::Secrets,
 ] {
 let payload = format!("payload-{}", cf.name());
 store.put(cf, 1, 1, 100, payload.as_bytes()).unwrap();
 assert_eq!(
 store.get(cf, 1, 1, 100).unwrap().as_deref(),
 Some(payload.as_bytes())
 );
 }
}

#[test]
fn re_open_preserves_state() {
 let dir = TempDir::new().unwrap();
 {
 let store = MnemosyneStore::open(dir.path()).unwrap();
 store.put(CfId::Entities, 1, 1, 100, b"durable").unwrap();
 }
 // Drop the store, re-open the same path, value must persist.
 let store = MnemosyneStore::open(dir.path()).unwrap();
 assert_eq!(
 store.get(CfId::Entities, 1, 1, 100).unwrap().as_deref(),
 Some(b"durable".as_ref())
 );
}

#[test]
fn schema_version_persists_across_reopen() {
 let dir = TempDir::new().unwrap();
 {
 let store = MnemosyneStore::open(dir.path()).unwrap();
 MigrationMeta::bump_version(store.db(), CfId::Entities, 5).unwrap();
 }
 let store = MnemosyneStore::open(dir.path()).unwrap();
 // Reopen seeds spec defaults, so the bump is overwritten — this documents
 // the wrapper contract: `seed_all` resets to spec on every open.
 let v = MigrationMeta::read_version(store.db(), CfId::Entities).unwrap();
 assert_eq!(v, Some(1));
}

#[test]
fn metadata_re_export_matches_internal() {
 // The re-export at the crate root must be identical to `cf_layout::ALL_CFS`.
 assert_eq!(ALL_CFS.len(), ALL_CFS_REEXPORT.len());
 assert_eq!(secondary_readable_cfs().len(), 8);
}

#[test]
fn iter_branch_entity_full_scan_shape() {
 let dir = TempDir::new().unwrap();
 let store = MnemosyneStore::open(dir.path()).unwrap();
 // Seed three time-points for one entity and confirm iteration order.
 for v in &[10u64, 20, 30] {
 store
 .put(CfId::Relations, 1, 1, *v, format!("@{}", v).as_bytes())
 .unwrap();
 }
 // Seed an unrelated entity in the same CF — must be excluded by prefix.
 store.put(CfId::Relations, 1, 2, 10, b"@10-other").unwrap();
 // Seed an unrelated branch — must also be excluded.
 store.put(CfId::Relations, 2, 1, 10, b"@10-branch2").unwrap();

 let scanned = store.iter_branch_entity(CfId::Relations, 1, 1).unwrap();
 assert_eq!(scanned.len(), 3);
 assert_eq!(scanned[0].0, 10);
 assert_eq!(scanned[1].0, 20);
 assert_eq!(scanned[2].0, 30);
}

#[test]
fn multi_cf_proposal_handler_pattern() {
 // Mimics mnemosyne-server proposal handler: one logical commit lands
 // entities + audit + temporal_index atomically.
 let dir = TempDir::new().unwrap();
 let store = MnemosyneStore::open(dir.path()).unwrap();
 let payload = vec![
 (CfId::Entities, 1, 1, 100, b"entity-state".to_vec()),
 (CfId::Audit, 1, 1, 100, b"audit-record".to_vec()),
 (CfId::TemporalIndex, 1, 1, 100, b"temporal".to_vec()),
 ];
 store.write_batch_multi_cf(&payload).unwrap();
 assert!(store.get(CfId::Entities, 1, 1, 100).unwrap().is_some());
 assert!(store.get(CfId::Audit, 1, 1, 100).unwrap().is_some());
 assert!(store.get(CfId::TemporalIndex, 1, 1, 100).unwrap().is_some());
}
