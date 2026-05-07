//! Phase -1B mnemosyne-self-scope codegen prototype (DESIGN.md §39/§42/§43).
//!
//! This crate is the codegen-file recognition prototype's residence within the mnemosyne repo.
//! Phase -1A measurement spike's isolated cargo workspace pattern (§18 line 1954) equivalent —
//! Maintains the *prototype scope* boundary while staying in sync with the mnemosyne-* production crates.
//!
//! Round 42 entry decision (§39 entity-relation graph indexer codegen priority work):
//! §39 → §42 → §43 dependency — §39 is the upstream-most (the other two consume its input
//! source). Phase -1A stage 2C schema-shape decision 4 item (24 B fixed-width BE
//! composite key / normalized asset_refs / row-per-encounter §44 / epistemic CF
//! separation) input + §66 prerequisite #3 (design_doc schema = Section / CrossRef /
//! ChangelogEntry / FrozenList — 4 entity/relation registered in §39 (Phase 0
//! Entry-block prerequisite source.
//!
//! This crate's scope separation (Round 42):
//! - `entity_indexer`: §39 entity-relation graph indexer codegen (Round 42 first work scope)
//! - `cf_wrapper`: §42 RocksDB CF runtime wrapper (Round 42 follow-up, consumes §39 input)
//! - `salsa_wire`: §43 Salsa wire codegen (Round 42 subsequent, Phase 1.5 cascade gate measurement source)

pub mod entity_indexer;
pub mod cf_wrapper;
pub mod salsa_wire;
pub mod closure_runtime;
pub mod markdown_import;
pub mod markdown_export;
pub mod t1_validator;
pub mod query_api;
