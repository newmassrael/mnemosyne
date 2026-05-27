//! Plugin substrate for Mnemosyne.
//!
//! RFC-003 FR-1 (transport abstraction) + FR-2 (validator + binding plugin
//! categories) land as a first-class crate so future plugin authors import
//! one symbol surface and the trust boundary between core and plugin is
//! enforced by Cargo edges, not naming convention.
//!
//! Two trait categories cover every foreseen extension surface:
//! - `Validator` reads the atomic store + plugin-specific input and emits
//!   zero or more `ValidationFinding` records (e.g. set-equality citation
//!   audit, behavioral spec checker, narrative continuity validator).
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
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub trait SymbolResolver: Send + Sync {
    fn version_surface(&self) -> VersionSurface;

    fn resolve_symbol_at(
        &self,
        file: &Path,
        line: u32,
    ) -> Result<Option<String>, ResolverError>;
}

pub trait Validator: Send + Sync {
    fn version_surface(&self) -> VersionSurface;

    fn validate(
        &self,
        context: &ValidationContext<'_>,
    ) -> Result<Vec<ValidationFinding>, ValidatorError>;
}

/// Read-only view of the atomic store as seen by `Validator` plugins.
///
/// The trait lives in `mnemosyne-plugin` (not `mnemosyne-validator`) so the
/// trust boundary is the Cargo edge: external Validator authors import
/// only `mnemosyne-plugin` and consume the store via this trait — no
/// reverse edge back into the validator crate is required.
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
/// canonical impl in `mnemosyne-validator::atomic::AtomicStore`) fill
/// every field; consumers (SetEqualityValidator and future plugins) read
/// the indices they need.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AtomicSnapshot {
    pub changelog_entry_ids: BTreeSet<String>,
    /// Section-id set including implied parent prefixes derived from
    /// `/` path components (mirror of `AtomicStore::atomic_section_id_set`).
    pub section_ids_with_implied_parents: BTreeSet<String>,
    pub sections: BTreeMap<String, SectionView>,
    pub inventory: BTreeMap<String, InventoryStatusView>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SectionView {
    pub implementations: Vec<ImplementationRef>,
    pub decision_status: Option<DecisionStatusView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImplementationRef {
    pub file: String,
    pub symbol: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DecisionStatusView {
    Active,
    Superseded,
    Removed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InventoryStatusView {
    Active,
    Reserved,
    Deprecated,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationFinding {
    pub severity: Severity,
    /// Validator-defined sub-kind tag (e.g., `"missing"`,
    /// `"section_missing"`, `"impl_unbacked"`). External consumers route
    /// per-class severity / counting off this string. None = the
    /// validator does not subdivide its findings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    pub section_id: Option<String>,
    pub file: Option<PathBuf>,
    pub line: Option<u32>,
    pub message: String,
    /// Validator-specific structured payload — preserves rich detail
    /// that doesn't fit the universal axes (e.g. `entry_id`, `symbol`,
    /// `decision_status`). JSON-compatible by construction so dispatch
    /// over MCP / CLI transports is lossless.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extras: BTreeMap<String, serde_json::Value>,
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
    validators: HashMap<String, Box<dyn Validator>>,
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

    pub fn register_validator(
        &mut self,
        key: impl Into<String>,
        validator: Box<dyn Validator>,
    ) {
        self.validators.insert(key.into(), validator);
    }

    pub fn symbol_resolver(&self, key: &str) -> Option<&dyn SymbolResolver> {
        self.symbol_resolvers.get(key).map(|b| b.as_ref())
    }

    pub fn validator(&self, key: &str) -> Option<&dyn Validator> {
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
            plugin_name: "mnemosyne-plugin::McpResolver".into(),
            plugin_version: env!("CARGO_PKG_VERSION").into(),
            schema_min: 4,
            schema_max: 4,
        }
    }

    fn resolve_symbol_at(
        &self,
        _file: &Path,
        _line: u32,
    ) -> Result<Option<String>, ResolverError> {
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
            plugin_name: "mnemosyne-plugin::CliResolver".into(),
            plugin_version: env!("CARGO_PKG_VERSION").into(),
            schema_min: 4,
            schema_max: 4,
        }
    }

    fn resolve_symbol_at(
        &self,
        _file: &Path,
        _line: u32,
    ) -> Result<Option<String>, ResolverError> {
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
        let out = r
            .resolve_symbol_at(Path::new("/dev/null"), 1)
            .expect("ok");
        assert!(out.is_none());
        assert!(reg.symbol_resolver("unregistered").is_none());
    }

    #[test]
    fn transport_variants_parse() {
        let toml_in_process = r#"transport = "in-process"
backend = "tree-sitter-rust""#;
        let parsed: Transport = toml::from_str(toml_in_process).unwrap();
        assert!(matches!(parsed, Transport::InProcess { ref backend } if backend == "tree-sitter-rust"));

        let toml_mcp = r#"transport = "mcp"
command = ["python", "-m", "resolver"]"#;
        let parsed: Transport = toml::from_str(toml_mcp).unwrap();
        assert!(matches!(parsed, Transport::Mcp { ref command } if command == &vec!["python".to_string(), "-m".to_string(), "resolver".to_string()]));

        let toml_cli = r#"transport = "cli"
command = ["gopls"]
output_parser = "gopls_v0_15""#;
        let parsed: Transport = toml::from_str(toml_cli).unwrap();
        assert!(matches!(parsed, Transport::Cli { .. }));
    }
}
