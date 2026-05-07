//! Typed store errors — DESIGN §42 *RocksDB CF runtime wrapper* failure modes.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
 #[error("rocksdb backend error: {0}")]
 RocksDb(#[from] rocksdb::Error),

 #[error("column family `{0}` not registered with the open DB")]
 MissingCf(&'static str),

 #[error("composite key length mismatch: expected {expected} bytes, got {got}")]
 KeyLength { expected: usize, got: usize },

 #[error("schema version mismatch on cf `{cf}`: stored={stored}, expected={expected}")]
 SchemaVersionMismatch {
 cf: &'static str,
 stored: u32,
 expected: u32,
 },

 #[error("io error: {0}")]
 Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, StoreError>;
