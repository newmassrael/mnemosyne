//! Plugin substrate for Mnemosyne.
//!
//! RFC-003 FR-1 (transport abstraction) + FR-2 (validator + binding plugin
//! categories) land as a first-class crate so future plugin authors import
//! one symbol surface and the trust boundary between core and plugin is
//! enforced by Cargo edges, not naming convention.
//!
//! Two trait categories cover every foreseen extension surface:
//! - `Validator` reads the atomic store + plugin-specific input and emits
//!   zero or more *typed* findings via the associated `type Finding`
//!   (e.g. set-equality citation audit emits a `CodeRefViolation` enum;
//!   behavioral spec checkers emit their own typed payload). The
//!   companion `ErasedValidator` trait (blanket-implemented for every
//!   `Validator`) provides object-safe dispatch through `PluginRegistry`
//!   with findings serialized to `serde_json::Value` at the trait edge.
//! - `SymbolResolver` is a binding-class capability that answers
//!   `(file, line) -> Option<symbol_name>` so the validator can enforce
//!   `Implementation.symbol` at file+symbol granularity instead of file-
//!   only set-equality.
//!
//! Three transport variants are exposed in the public type surface even
//! though only `InProcess` is wired in the substrate's first round; `Mcp`
//! / `Cli` callers surface `ResolverError::NotImplemented` until a sample
//! backend lands. The variant set is stable so future transport land does
//! not change call sites.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

mod fact;
pub use fact::{FactKey, SectionSkeleton};

pub trait SymbolResolver: Send + Sync {
    fn version_surface(&self) -> VersionSurface;

    fn resolve_symbol_at(&self, file: &Path, line: u32) -> Result<Option<String>, ResolverError>;
}

/// Validator plugin contract — typed-finding form.
///
/// Each plugin declares its own `Finding` type with the rich shape that
/// best fits its domain (citation defense → `CodeRefViolation` enum,
/// behavioral checker → its own typed payload, etc.). The associated-
/// type form gives plugin authors and concrete callers full static
/// guarantees on payload shape. Use `Validator` when the caller knows
/// the concrete plugin type; use the object-safe [`ErasedValidator`]
/// companion (blanket-implemented for every `Validator`) when dispatch
/// must go through `PluginRegistry`.
pub trait Validator: Send + Sync {
    /// Plugin-specific typed finding payload. Must be `Serialize` so the
    /// erased dispatch path can carry the value across the object-safe
    /// trait boundary; `Debug` for diagnostics; `Send` for cross-thread
    /// dispatch.
    type Finding: Serialize + Send + std::fmt::Debug;

    fn version_surface(&self) -> VersionSurface;

    fn validate(
        &self,
        context: &ValidationContext<'_>,
    ) -> Result<Vec<Self::Finding>, ValidatorError>;
}

/// Object-safe companion to [`Validator`]. Blanket-implemented for every
/// `V: Validator`, so registering a typed validator into
/// [`PluginRegistry`] is the same code path: `Box::new(my_validator)`
/// coerces to `Box<dyn ErasedValidator>` automatically.
///
/// The erased path serializes each typed finding to `serde_json::Value`
/// at the trait edge — losing static type info in exchange for object-
/// safety. Callers that need the typed shape back can hold the concrete
/// `V` directly and invoke [`Validator::validate`] instead.
pub trait ErasedValidator: Send + Sync {
    fn version_surface(&self) -> VersionSurface;

    fn validate_erased(
        &self,
        context: &ValidationContext<'_>,
    ) -> Result<Vec<serde_json::Value>, ValidatorError>;
}

impl<V> ErasedValidator for V
where
    V: Validator,
{
    fn version_surface(&self) -> VersionSurface {
        <V as Validator>::version_surface(self)
    }

    fn validate_erased(
        &self,
        context: &ValidationContext<'_>,
    ) -> Result<Vec<serde_json::Value>, ValidatorError> {
        let findings = <V as Validator>::validate(self, context)?;
        findings
            .into_iter()
            .map(|f| {
                serde_json::to_value(f).map_err(|e| {
                    ValidatorError::Internal(format!(
                        "Finding serialization failed at erased dispatch edge: {}",
                        e
                    ))
                })
            })
            .collect()
    }
}

/// Read-only view of the atomic store as seen by `Validator` plugins.
///
/// The trait lives in `mnemosyne-core` (not in any downstream crate) so
/// the trust boundary is the Cargo edge: external Validator authors
/// import only `mnemosyne-core` and consume the store via this trait —
/// no reverse edge back into the producer crate (`mnemosyne-atomic`) is
/// required.
///
/// `snapshot()` is the single read primitive: producers materialize every
/// field the current plugin contract needs upfront, callers index into
/// the returned `AtomicSnapshot`. Eager-snapshot shape (vs lazy
/// iterators) keeps the type object-safe, makes the surface
/// JSON-serializable end-to-end (R308 MCP-transport prerequisite), and
/// gives external plugin authors a single shape to reason about.
pub trait AtomicStoreView: Send + Sync {
    fn snapshot(&self) -> AtomicSnapshot;
}

/// Snapshot of every atomic-store surface a `Validator` plugin reads.
///
/// Closed-form by construction — extending the surface requires growing
/// this struct, which the substrate then ratifies. Producers (the
/// canonical impl in `mnemosyne-atomic::AtomicStore`) fill every field;
/// consumers (`SetEqualityValidator` and future plugins) read the
/// indices they need.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AtomicSnapshot {
    pub changelog_entry_ids: BTreeSet<String>,
    /// Section-id set including implied parent prefixes derived from
    /// `/` path components (mirror of `AtomicStore::atomic_section_id_set`).
    pub section_ids_with_implied_parents: BTreeSet<String>,
    pub sections: BTreeMap<String, SectionView>,
    pub inventory: BTreeMap<String, InventoryStatus>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SectionView {
    pub implementations: Vec<ImplementationRef>,
    pub decision_status: Option<DecisionStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImplementationRef {
    pub file: String,
    pub symbol: Option<String>,
}

/// Section.decision_status lifecycle vocabulary — substrate-canonical
/// enum. Lives in `mnemosyne-core` (not in `mnemosyne-schema` or any
/// downstream crate) so every plugin author works against one type, and
/// the snapshot returned from `AtomicStoreView::snapshot` round-trips
/// without an adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DecisionStatus {
    Active,
    Superseded,
    Removed,
}

/// Inventory entry lifecycle vocabulary — substrate-canonical enum.
/// Genre distinct from `DecisionStatus`: stable external IDs (test
/// cases, requirement IDs, regulation IDs) whose lifecycle is
/// `Active` / `Deprecated` / `Reserved`. Lives in `mnemosyne-core`
/// alongside `DecisionStatus` so every plugin reads one canonical
/// status surface.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InventoryStatus {
    #[default]
    Active,
    Deprecated,
    Reserved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionSurface {
    pub plugin_name: String,
    pub plugin_version: String,
    pub schema_min: u32,
    pub schema_max: u32,
}

pub struct ValidationContext<'a> {
    pub workspace_root: &'a Path,
    pub atomic_sidecar: &'a Path,
    pub store: &'a dyn AtomicStoreView,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Reject,
    Warn,
    Info,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginCategory {
    Validator,
    Binding,
}

/// Transport variant. Surfaces every plan-of-record backend mode from
/// the RFC-003 transport-abstraction section; only `InProcess` returns
/// concrete answers in the substrate's first round. The others reach
/// the active call site but return `ResolverError::NotImplemented`
/// until sample backends land.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "transport", rename_all = "kebab-case")]
pub enum Transport {
    InProcess {
        backend: String,
    },
    Mcp {
        command: Vec<String>,
    },
    Cli {
        command: Vec<String>,
        output_parser: Option<String>,
    },
}

#[derive(Debug, Error)]
pub enum ResolverError {
    #[error("transport not implemented yet (scaffolding only — sample backend deferred)")]
    NotImplemented,
    #[error("plugin not registered: {0}")]
    Unregistered(String),
    #[error("resolver internal failure: {0}")]
    Internal(String),
}

#[derive(Debug, Error)]
pub enum ValidatorError {
    #[error("validator internal failure: {0}")]
    Internal(String),
}

/// Explicit-init registry. Backend crates expose a `register(&mut
/// PluginRegistry)` entry point; the top-level binary (mnemosyne-cli /
/// mnemosyne-mcp) opts in by depending on the backend crate and calling
/// `register`. No global state, no inventory crate, no dlopen — the trust
/// boundary is the Cargo edge.
#[derive(Default)]
pub struct PluginRegistry {
    symbol_resolvers: HashMap<String, Box<dyn SymbolResolver>>,
    validators: HashMap<String, Box<dyn ErasedValidator>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_symbol_resolver(
        &mut self,
        key: impl Into<String>,
        resolver: Box<dyn SymbolResolver>,
    ) {
        self.symbol_resolvers.insert(key.into(), resolver);
    }

    /// Register a `Validator` plugin. The boxed value is held as a
    /// `Box<dyn ErasedValidator>`; coercion from `Box<V>` where
    /// `V: Validator` is automatic via the blanket
    /// `impl<V: Validator> ErasedValidator for V`.
    pub fn register_validator(
        &mut self,
        key: impl Into<String>,
        validator: Box<dyn ErasedValidator>,
    ) {
        self.validators.insert(key.into(), validator);
    }

    pub fn symbol_resolver(&self, key: &str) -> Option<&dyn SymbolResolver> {
        self.symbol_resolvers.get(key).map(|b| b.as_ref())
    }

    pub fn validator(&self, key: &str) -> Option<&dyn ErasedValidator> {
        self.validators.get(key).map(|b| b.as_ref())
    }

    pub fn symbol_resolver_keys(&self) -> impl Iterator<Item = &str> {
        self.symbol_resolvers.keys().map(|s| s.as_str())
    }

    pub fn validator_keys(&self) -> impl Iterator<Item = &str> {
        self.validators.keys().map(|s| s.as_str())
    }
}

/// MCP-transport `SymbolResolver` placeholder. R306 surfaces the variant
/// in the type / config / registry path so `[plugins.symbol_resolver.<lang>]
/// transport = "mcp"` configs parse and reach the call site; the actual
/// MCP client wire (handshake, `resolve_symbol_at` tool call, JSON-RPC
/// streaming) is deferred to R307+ once a sample MCP backend is
/// confirmed (candidate: Python LSP wrapper, or mnemosyne-mcp itself
/// exposing a SymbolResolver tool for self-referential dogfood).
pub struct McpResolver {
    pub command: Vec<String>,
}

impl SymbolResolver for McpResolver {
    fn version_surface(&self) -> VersionSurface {
        VersionSurface {
            plugin_name: "mnemosyne-core::McpResolver".into(),
            plugin_version: env!("CARGO_PKG_VERSION").into(),
            schema_min: 4,
            schema_max: 4,
        }
    }

    fn resolve_symbol_at(&self, _file: &Path, _line: u32) -> Result<Option<String>, ResolverError> {
        Err(ResolverError::NotImplemented)
    }
}

/// CLI-transport `SymbolResolver` placeholder. R306 surfaces the variant
/// so `transport = "cli"` configs parse and reach the call site; actual
/// shell-out (gopls / clangd / pyright stdio with structured output
/// parser) deferred to R307+ once a sample CLI backend is confirmed
/// (candidate: gopls, pending system installation).
pub struct CliResolver {
    pub command: Vec<String>,
    pub output_parser: Option<String>,
}

impl SymbolResolver for CliResolver {
    fn version_surface(&self) -> VersionSurface {
        VersionSurface {
            plugin_name: "mnemosyne-core::CliResolver".into(),
            plugin_version: env!("CARGO_PKG_VERSION").into(),
            schema_min: 4,
            schema_max: 4,
        }
    }

    fn resolve_symbol_at(&self, _file: &Path, _line: u32) -> Result<Option<String>, ResolverError> {
        Err(ResolverError::NotImplemented)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct AlwaysNoneResolver;

    impl SymbolResolver for AlwaysNoneResolver {
        fn version_surface(&self) -> VersionSurface {
            VersionSurface {
                plugin_name: "always-none".into(),
                plugin_version: "0.0.0".into(),
                schema_min: 4,
                schema_max: 4,
            }
        }
        fn resolve_symbol_at(
            &self,
            _file: &Path,
            _line: u32,
        ) -> Result<Option<String>, ResolverError> {
            Ok(None)
        }
    }

    #[test]
    fn registry_round_trip() {
        let mut reg = PluginRegistry::new();
        reg.register_symbol_resolver("rust", Box::new(AlwaysNoneResolver));
        let r = reg.symbol_resolver("rust").expect("registered");
        let out = r.resolve_symbol_at(Path::new("/dev/null"), 1).expect("ok");
        assert!(out.is_none());
        assert!(reg.symbol_resolver("unregistered").is_none());
    }

    #[test]
    fn transport_variants_parse() {
        let toml_in_process = r#"transport = "in-process"
backend = "tree-sitter-rust""#;
        let parsed: Transport = toml::from_str(toml_in_process).unwrap();
        assert!(
            matches!(parsed, Transport::InProcess { ref backend } if backend == "tree-sitter-rust")
        );

        let toml_mcp = r#"transport = "mcp"
command = ["python", "-m", "resolver"]"#;
        let parsed: Transport = toml::from_str(toml_mcp).unwrap();
        assert!(
            matches!(parsed, Transport::Mcp { ref command } if command == &vec!["python".to_string(), "-m".to_string(), "resolver".to_string()])
        );

        let toml_cli = r#"transport = "cli"
command = ["gopls"]
output_parser = "gopls_v0_15""#;
        let parsed: Transport = toml::from_str(toml_cli).unwrap();
        assert!(matches!(parsed, Transport::Cli { .. }));
    }
}
