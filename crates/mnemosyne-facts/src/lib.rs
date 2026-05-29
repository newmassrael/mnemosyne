//! Mnemosyne facts — derived-index persistence binding for the canonical typed
//! facts (Section / ChangelogEntry / FrozenList / CrossRef).
//!
//! The canonical fact *structs* live in `mnemosyne-core` (Layer 0 — the one
//! canonical fact model). This crate owns only the *index* side of the
//! substrate: the byte codec that serializes those structs into the RocksDB
//! materialized index, and the typed put/get binding over `mnemosyne-store`.
//!
//! ## Module separation
//!
//! - [`facts`]: the [`IndexCodec`] byte codec — deterministic length-prefixed
//!   value encoding for the canonical fact structs (identical fact → identical
//!   bytes, required for content-addressable hashing and audit comparison).
//! - [`persist`]: [`TypedFactStore`] binds `mnemosyne-store::MnemosyneStore`
//!   to typed put/get for the 4 entity/relation kinds.

pub mod facts;
pub mod persist;

// The canonical fact structs are owned by `mnemosyne-core` (Layer 0); consumers
// import them from there directly. This crate exports only the *index* side of
// the substrate — the byte codec and the typed put/get persistence binding.
pub use facts::{FactCodecError, IndexCodec};
pub use persist::{PersistError, TypedFactStore};
