//! Mnemosyne store — Phase 0 production crate (DESIGN.md /).
//!
//! this crate 10 CF schema (entities / relations / temporal_index /
//! temporal_index_open / branch_meta / assets / asset_refs / audit /
//! epistemic / secrets) in actual `rocksdb::DB` binding + ColumnFamilyDescriptor
//! registered + 24 B BE composite key encode/decode + WriteBatch + iterator
//! source of truth.
//!
//! OPTION B-2 production carry — bench/codegen-prototype/src/cf_wrapper.rs
//! Typed CRUD wrapper emit pattern (`{Entity}CF::put / get / iter_branch /
//! Production runtime binding for `write_batch` — prototype-scope codegen emit
//! source string only output, this production crate emit pattern's actual rocksdb
//! Invocation source.
//!
//! ## Module separation
//!
//! - [`cf_layout`]: 10 CF metadata (CfId enum + CfMeta struct + IterPattern
//! enum + secondary_readable flag + schema_version).
//! - [`key_codec`]: 24 B BE composite key encode/decode (Phase -1A stage 2C
//! decision carry — branch_id (8 B BE) || entity_id (8 B BE) || valid_from (8 B BE)).
//! - [`store`]: `MnemosyneStore` actual `rocksdb::DB` wrapper + per-CF
//! put/get/iter/write_batch + WriteBatch atomic group.
//! - [`migration`]: `MigrationMeta` schema_version tracking + per-CF version
//! bump + scaffold_new_cf source.
//! - [`error`]: `StoreError` typed enum (rocksdb error / missing CF / key
//! length violation etc.).

pub mod cf_layout;
pub mod error;
pub mod key_codec;
pub mod migration;
pub mod store;

pub use cf_layout::{cf_descriptors, CfId, CfMeta, IterPattern, ALL_CFS};
pub use error::StoreError;
pub use key_codec::{decode_composite_key, encode_composite_key, KEY_LEN};
pub use migration::{MigrationMeta, MIGRATION_META_KEY_PREFIX};
pub use store::MnemosyneStore;
