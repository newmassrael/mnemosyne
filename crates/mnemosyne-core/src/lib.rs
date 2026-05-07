//! Mnemosyne core — Phase 0 production crate (DESIGN.md §39).
//!
//! This crate registers the 4 entity/relation defined in §39 *Phase 0 design_doc schema closed-form registered*.
//! (Section / ChangelogEntry / FrozenList / CrossRef) in production typed facts
//! source of truth. The bench prototype `entity_indexer.rs` codegen emit (template
//! Production binding for the source — `mnemosyne-store`'s 24 B BE composite
//! persist as key + 5-language emit (Rust is authoritative; Kotlin / Python / C++
//! / Protobuf reference) + canonical_identifier_set + cross-language Jaccard
//! inclusion validation pass.
//!
//! ## Module separation
//!
//! - [`schema`]: meta-level GraphSpec / EntityDef / RelationDef / FieldType /
//! Persistence — language-agnostic schema description.
//! - [`fixture`]: §39 closed-form 4 entity/relation `design_doc_schema_fixture`.
//! - [`emit`]: 5-language code emit (rust / kotlin / python / cpp / protobuf).
//! Deterministic — identical input → byte-identical output.
//! - [`canonical`]: canonical_identifier_set + jaccard_inclusion + sha256_hex
//! for cross-language identifier presence verification.
//! - [`facts`]: typed-fact serialization (Section / ChangelogEntry / FrozenList /
//! CrossRef instance encoding to bytes for storage).
//! - [`persist`]: `TypedFactStore` binding `mnemosyne-store::MnemosyneStore` →
//! typed put/get/iter for the 4 entity/relation kinds.

pub mod canonical;
pub mod emit;
pub mod facts;
pub mod fixture;
pub mod persist;
pub mod schema;

pub use canonical::{canonical_identifier_set, jaccard_inclusion, sha256_hex};
pub use emit::{emit_all_languages, emit_cpp, emit_kotlin, emit_protobuf, emit_python, emit_rust, EmittedMultiLang};
pub use facts::{ChangelogEntryFact, CrossRefFact, FactCodecError, FrozenListFact, SectionFact};
pub use fixture::design_doc_schema_fixture;
pub use persist::{PersistError, TypedFactStore};
pub use schema::{
 CompositeKey, EntityDef, FieldDef, FieldType, GraphSpec, Persistence, RelationDef,
};
