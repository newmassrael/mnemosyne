//! 10 CF layout — / source of truth.
//!
//! `entities / relations / temporal_index / temporal_index_open / branch_meta /
//! assets / asset_refs / audit / epistemic / secrets`.
//!
//! Each CF carries:
//! - `name`: RocksDB ColumnFamily handle name (snake_case literal).
//! - `iter_pattern`: `prefix_scan` (entity-shaped) / `range_scan` (open-interval) /
//! `append_only_seq` (audit-shaped, monotonic key).
//! - `secondary_readable`: secondary read-access (audit / secrets are blocked).
//! - `schema_version`: per-CF migration marker (start at 1, bump on schema change).

use rocksdb::{ColumnFamilyDescriptor, Options};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IterPattern {
 /// `branch_id || entity_id` 16 B prefix scan, time-ordered within prefix.
 PrefixScan,
 /// Open-interval / range key (temporal_index_open variant).
 RangeScan,
 /// Append-only monotonic — audit / log shape.
 AppendOnlySeq,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CfId {
 Entities,
 Relations,
 TemporalIndex,
 TemporalIndexOpen,
 BranchMeta,
 Assets,
 AssetRefs,
 Audit,
 Epistemic,
 Secrets,
 /// Internal CF for schema-version metadata persistence (migration source).
 /// Not in enumeration — wrapper-internal, not user-facing.
 MigrationMeta,
}

impl CfId {
 pub fn name(self) -> &'static str {
 match self {
 CfId::Entities => "entities",
 CfId::Relations => "relations",
 CfId::TemporalIndex => "temporal_index",
 CfId::TemporalIndexOpen => "temporal_index_open",
 CfId::BranchMeta => "branch_meta",
 CfId::Assets => "assets",
 CfId::AssetRefs => "asset_refs",
 CfId::Audit => "audit",
 CfId::Epistemic => "epistemic",
 CfId::Secrets => "secrets",
 CfId::MigrationMeta => "migration_meta",
 }
 }
}

#[derive(Debug, Clone, Copy)]
pub struct CfMeta {
 pub id: CfId,
 pub iter_pattern: IterPattern,
 pub secondary_readable: bool,
 pub schema_version: u32,
}

impl CfMeta {
 pub fn name(&self) -> &'static str {
 self.id.name()
 }
}

/// ten CF + internal `migration_meta` (eleven total descriptors).
/// secondary_readable / iter_pattern policy directly mirrors / /.
pub const ALL_CFS: &[CfMeta] = &[
 CfMeta {
 id: CfId::Entities,
 iter_pattern: IterPattern::PrefixScan,
 secondary_readable: true,
 schema_version: 1,
 },
 CfMeta {
 id: CfId::Relations,
 iter_pattern: IterPattern::PrefixScan,
 secondary_readable: true,
 schema_version: 1,
 },
 CfMeta {
 id: CfId::TemporalIndex,
 iter_pattern: IterPattern::PrefixScan,
 secondary_readable: true,
 schema_version: 1,
 },
 CfMeta {
 id: CfId::TemporalIndexOpen,
 iter_pattern: IterPattern::RangeScan,
 secondary_readable: true,
 schema_version: 1,
 },
 CfMeta {
 id: CfId::BranchMeta,
 iter_pattern: IterPattern::PrefixScan,
 secondary_readable: true,
 schema_version: 1,
 },
 CfMeta {
 id: CfId::Assets,
 iter_pattern: IterPattern::PrefixScan,
 secondary_readable: true,
 schema_version: 1,
 },
 CfMeta {
 id: CfId::AssetRefs,
 iter_pattern: IterPattern::PrefixScan,
 secondary_readable: true,
 schema_version: 1,
 },
 CfMeta {
 id: CfId::Audit,
 iter_pattern: IterPattern::AppendOnlySeq,
 // — audit is intentionally blocked from secondary read.
 secondary_readable: false,
 schema_version: 1,
 },
 CfMeta {
 id: CfId::Epistemic,
 iter_pattern: IterPattern::PrefixScan,
 secondary_readable: true,
 schema_version: 1,
 },
 CfMeta {
 id: CfId::Secrets,
 iter_pattern: IterPattern::AppendOnlySeq,
 // secrets — blocked from secondary read (encrypted at rest).
 secondary_readable: false,
 schema_version: 1,
 },
 CfMeta {
 id: CfId::MigrationMeta,
 iter_pattern: IterPattern::PrefixScan,
 secondary_readable: false,
 schema_version: 1,
 },
];

/// CF descriptor set for `DB::open_cf_descriptors`. Each CF gets a fresh `Options`
/// — additional per-CF tuning (compaction style, prefix extractor) is a Phase 0+
/// implementation concern; the prototype baseline stays default-options.
pub fn cf_descriptors() -> Vec<ColumnFamilyDescriptor> {
 ALL_CFS
 .iter()
 .map(|m| ColumnFamilyDescriptor::new(m.name(), Options::default()))
 .collect()
}

/// Lookup a CF metadata entry by id. Compile-time enumeration, runtime O(N=11).
pub fn meta(id: CfId) -> &'static CfMeta {
 ALL_CFS
 .iter()
 .find(|m| m.id == id)
 .expect("ALL_CFS covers every CfId variant")
}

/// `secondary_readable=true` subset secondary subset filter.
pub fn secondary_readable_cfs() -> Vec<&'static CfMeta> {
 ALL_CFS.iter().filter(|m| m.secondary_readable).collect()
}

#[cfg(test)]
mod tests {
 use super::*;

 /// enumerates exactly ten user-facing CFs (audit + secrets blocked
 /// from secondary, all others readable). The wrapper adds one internal
 /// `migration_meta` CF, so descriptor list = 11.
 #[test]
 fn ten_user_facing_plus_one_internal() {
 assert_eq!(ALL_CFS.len(), 11);
 let user_facing = ALL_CFS
 .iter()
 .filter(|m| m.id != CfId::MigrationMeta)
 .count();
 assert_eq!(user_facing, 10);
 }

 #[test]
 fn names_match_design_md() {
 let expected = [
 "entities",
 "relations",
 "temporal_index",
 "temporal_index_open",
 "branch_meta",
 "assets",
 "asset_refs",
 "audit",
 "epistemic",
 "secrets",
 "migration_meta",
 ];
 let actual: Vec<&str> = ALL_CFS.iter().map(|m| m.name()).collect();
 assert_eq!(actual, expected);
 }

 /// — audit + secrets are blocked from secondary read.
 #[test]
 fn audit_and_secrets_blocked_from_secondary() {
 assert!(!meta(CfId::Audit).secondary_readable);
 assert!(!meta(CfId::Secrets).secondary_readable);
 assert!(meta(CfId::Entities).secondary_readable);
 assert!(meta(CfId::Relations).secondary_readable);
 }

 #[test]
 fn audit_is_append_only_pattern() {
 assert_eq!(meta(CfId::Audit).iter_pattern, IterPattern::AppendOnlySeq);
 assert_eq!(meta(CfId::Entities).iter_pattern, IterPattern::PrefixScan);
 assert_eq!(
 meta(CfId::TemporalIndexOpen).iter_pattern,
 IterPattern::RangeScan
 );
 }

 #[test]
 fn cf_descriptors_match_metadata() {
 let descriptors = cf_descriptors();
 assert_eq!(descriptors.len(), ALL_CFS.len());
 }

 #[test]
 fn secondary_readable_subset_excludes_audit_secrets() {
 let names: Vec<&str> = secondary_readable_cfs().iter().map(|m| m.name()).collect();
 assert!(!names.contains(&"audit"));
 assert!(!names.contains(&"secrets"));
 assert!(!names.contains(&"migration_meta"));
 assert!(names.contains(&"entities"));
 assert!(names.contains(&"relations"));
 }

 #[test]
 fn schema_version_starts_at_one() {
 for m in ALL_CFS {
 assert_eq!(m.schema_version, 1, "{} schema_version != 1", m.name());
 }
 }
}
