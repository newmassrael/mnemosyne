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
pub use fact::{
    ChangelogEntryFact, CrossRefFact, FactKey, FrozenListFact, SectionFact, SectionSkeleton,
};

mod narrative;
pub use narrative::{
    forward_confluences, is_confluence, is_known_world, succession_branch_inherits,
    world_membership, world_membership_memoized, Branch, BranchFork, ConflictAssertion,
    DisclosureMode, DisclosureOverride, DisclosurePlan, DisclosureSurface, EdgeCost,
    EffectiveDisclosure, Entity, EntityKind, Frame, IntervalOp, NarrativeFact, Parameter,
    ParameterGate, PayoffExpectation, Predicate, PredicateObjectKind, TypedClaim, TypedObject,
    Unit, WorldMembership, MAIN_BRANCH,
};

mod section_ref;
pub use section_ref::{numeric_section_refs, strip_section_marker};

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
    pub bindings: Vec<BindingRef>,
    pub decision_status: Option<DecisionStatus>,
    pub coverage_expectation: CoverageExpectation,
    pub verification_expectation: VerificationExpectation,
}

/// Trace-link claim strength on a Path B binding (code → spec section).
/// Canonical substrate enum (lives here in L0 core, mirroring
/// [`DecisionStatus`], so atomic / validate / plugins share one type with
/// no adapter). `Implements` = SysML «satisfy» (fulfills the requirement;
/// the only kind that counts as implements-coverage); `References` = SysML
/// «trace» (related to, no fulfillment claim); `Verifies` = SysML «verify»
/// (a test/evidence artifact establishes the requirement). `Verifies` counts
/// as neither implements-coverage NOR a code↔spec citation edge: its `file`
/// is a test/report artifact whose link to the section is sourced externally
/// (e.g. a conformance manifest), not from a `§<id>` citation, so it is
/// excluded from the bidirectional citation set-equality. `refines` remains
/// in the closed taxonomy but deferred (load-time migration makes a new
/// variant a single-step change).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BindingKind {
    Implements,
    References,
    Verifies,
}

impl BindingKind {
    pub fn as_str(self) -> &'static str {
        match self {
            BindingKind::Implements => "implements",
            BindingKind::References => "references",
            BindingKind::Verifies => "verifies",
        }
    }

    /// Parse the canonical lowercase tag ([`Self::as_str`]) back to a kind.
    /// `None` for any other string. Used to round-trip the kind through the
    /// projection layer's primitive salsa inputs (core is L0 zero-dep and
    /// cannot derive `salsa::Update`, so the enum is lowered to its tag).
    pub fn from_tag(s: &str) -> Option<Self> {
        match s {
            "implements" => Some(BindingKind::Implements),
            "references" => Some(BindingKind::References),
            "verifies" => Some(BindingKind::Verifies),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindingRef {
    pub file: String,
    pub symbol: Option<String>,
    pub kind: BindingKind,
}

/// Whether a section's coverage axiom applies. A `Normative` section is a
/// requirement that expects an `implements` binding — a non-`Removed`
/// `Normative` section with zero `implements` bindings is the coverage gap
/// (the Round 269 axiom). An `Informative` section is prose-only (terminology
/// / overview / references) with nothing to implement here, and is exempt from
/// the axiom. Canonical substrate enum (L0 core, mirroring [`BindingKind`] /
/// [`DecisionStatus`]) so atomic / validate / render share one type with no
/// adapter. Not lifted into [`SectionSkeleton`]: coverage applicability is not
/// medium-neutral (meaningless for a non-code medium), so it lives with the
/// adapter-local binding capability, exactly as [`BindingKind`] does.
/// `Normative` is the default — it preserves the pre-classification behavior
/// (every section expects coverage), so a store with no classification gates
/// identically to before.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CoverageExpectation {
    #[default]
    Normative,
    /// Part of the standard but not implemented by THIS consumer; revisitable if
    /// scope expands (design sec 6). Serialized as `out_of_scope_here`. The
    /// pre-3-state `informative` tag is NOT aliased (R422 clean break): a store
    /// still carrying it fails to load LOUDLY, so a consumer migrates it
    /// (`informative` → `out_of_scope_here`) deliberately rather than relying on
    /// a silent compat shim. No silent-drop risk (unknown enum tags error).
    OutOfScopeHere,
    /// Inherently non-implementable prose / context (terminology / overview).
    Informational,
}

impl CoverageExpectation {
    /// Canonical lowercase label (matches the serde representation).
    pub fn as_str(self) -> &'static str {
        match self {
            CoverageExpectation::Normative => "normative",
            CoverageExpectation::OutOfScopeHere => "out_of_scope_here",
            CoverageExpectation::Informational => "informational",
        }
    }

    /// Parse the canonical lowercase tag ([`Self::as_str`]) back to a value.
    /// `None` for any other string. Used to round-trip the classification
    /// through the projection layer's primitive salsa inputs (core is L0
    /// zero-dep and cannot derive `salsa::Update`, so the enum is lowered to
    /// its tag), mirroring [`BindingKind::from_tag`].
    pub fn from_tag(s: &str) -> Option<Self> {
        match s {
            "normative" => Some(CoverageExpectation::Normative),
            "out_of_scope_here" => Some(CoverageExpectation::OutOfScopeHere),
            "informational" => Some(CoverageExpectation::Informational),
            _ => None,
        }
    }
}

/// What KIND of verification evidence a `Normative` section is expected to
/// carry — the axis orthogonal to [`CoverageExpectation`]. `Dedicated` = a
/// behavioral requirement whose evidence is a concrete test/report artifact,
/// so a `verifies` binding is expected (a `Dedicated` section with zero
/// `verifies` bindings is the `VerificationMissing` gap). `ByConstruction` =
/// a requirement with no independently-assertable per-unit oracle (e.g.
/// transcribed algorithm pseudocode exercised holistically by behavioral
/// tests), exempt from the dedicated-verify gate. This is SEPARATE from
/// `CoverageExpectation` because a `ByConstruction` section is still
/// `Normative` for the implements axiom (it has implementing code) — folding
/// the two into one field would force an `Informative` mislabel that silently
/// drops it from implements-coverage. Consulted only when
/// `coverage_expectation == Normative` (an `Informative` section is exempt
/// from both axes). `Dedicated` is the default: a new normative section, and
/// any section with at least one independently-assertable clause, expects
/// dedicated evidence until classified otherwise. Mirrors [`BindingKind`] /
/// [`CoverageExpectation`] (L0 core, adapter-local, no medium-neutral lift).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationExpectation {
    #[default]
    Dedicated,
    ByConstruction,
}

impl VerificationExpectation {
    /// Canonical lowercase label (matches the serde representation).
    pub fn as_str(self) -> &'static str {
        match self {
            VerificationExpectation::Dedicated => "dedicated",
            VerificationExpectation::ByConstruction => "by_construction",
        }
    }

    /// Parse the canonical lowercase tag ([`Self::as_str`]) back to a value.
    /// `None` for any other string. Mirrors [`CoverageExpectation::from_tag`].
    pub fn from_tag(s: &str) -> Option<Self> {
        match s {
            "dedicated" => Some(VerificationExpectation::Dedicated),
            "by_construction" => Some(VerificationExpectation::ByConstruction),
            _ => None,
        }
    }
}

/// Section.decision_status lifecycle vocabulary — substrate-canonical
/// enum. Lives in `mnemosyne-core` (not in any downstream leaf crate)
/// so every plugin author works against one type, and
/// the snapshot returned from `AtomicStoreView::snapshot` round-trips
/// without an adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DecisionStatus {
    Active,
    Superseded,
    Removed,
    /// The section poses a not-yet-decided question — it sits on the decision
    /// lifecycle but no decision is in force yet (`Open` → `Active` when
    /// resolved, or `Removed` if withdrawn; never `Superseded`, which replaces
    /// one *decision* with another). Like `Removed`, an `Open` section is EXEMPT
    /// from the coverage / verification axioms: there is no decision to back
    /// with code or tests yet. This is the structured-fact SSOT home for the
    /// open-question state — prose POINTS at an open question (`§<id>` in `Open`
    /// state) instead of RESTATING "this is still open"
    /// (claudedocs/structured-fact-ssot-design.md sec 12a). Appended last to
    /// preserve the existing `Ord` over Active/Superseded/Removed.
    Open,
}

impl DecisionStatus {
    /// Canonical lowercase label (matches the serde representation). Used by
    /// adapters that still carry the status as a string at a layer boundary.
    pub fn as_str(self) -> &'static str {
        match self {
            DecisionStatus::Active => "active",
            DecisionStatus::Superseded => "superseded",
            DecisionStatus::Removed => "removed",
            DecisionStatus::Open => "open",
        }
    }

    /// Parse the canonical lowercase tag back to a value — the ONE resolver both
    /// the CLI (`set-section-decision-status`) and the MCP tool share (R678), so
    /// the two surfaces cannot accept different vocabularies. `None` for any
    /// other string (fail-loud at the caller; no silent default).
    pub fn from_tag(s: &str) -> Option<Self> {
        match s {
            "active" => Some(DecisionStatus::Active),
            "superseded" => Some(DecisionStatus::Superseded),
            "removed" => Some(DecisionStatus::Removed),
            "open" => Some(DecisionStatus::Open),
            _ => None,
        }
    }

    /// Lifecycle states that carry no in-force, backable decision — `Removed`
    /// (tombstone) and `Open` (question not yet decided). Such sections are
    /// EXEMPT from the decision-backing axioms (coverage / verification /
    /// confirmation): there is nothing to back with code or tests. `Active` and
    /// `Superseded` are NOT exempt — a superseded section's historical bindings
    /// are still audited. Single source for the exemption set so the axes cannot
    /// drift apart (CLAUDE.md half-enforced-invariant guard).
    pub fn is_axiom_exempt(self) -> bool {
        matches!(self, DecisionStatus::Removed | DecisionStatus::Open)
    }
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

impl InventoryStatus {
    /// Canonical snake_case label (matches the serde representation). Used by
    /// adapters that carry the status as a string at a layer boundary
    /// (CLI/MCP render). Mirrors [`DecisionStatus::as_str`].
    pub fn as_str(self) -> &'static str {
        match self {
            InventoryStatus::Active => "active",
            InventoryStatus::Deprecated => "deprecated",
            InventoryStatus::Reserved => "reserved",
        }
    }
}

/// Error returned when a string is not a valid [`InventoryStatus`] label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseInventoryStatusError {
    pub got: String,
}

impl std::fmt::Display for ParseInventoryStatusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "`{}` invalid (expected active|deprecated|reserved)",
            self.got
        )
    }
}

impl std::error::Error for ParseInventoryStatusError {}

impl std::str::FromStr for InventoryStatus {
    type Err = ParseInventoryStatusError;

    /// Parse the canonical label (case-insensitive). The sole vocabulary
    /// source for the CLI/MCP `--status` parsers (Round 357 DRY).
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "active" => Ok(InventoryStatus::Active),
            "deprecated" => Ok(InventoryStatus::Deprecated),
            "reserved" => Ok(InventoryStatus::Reserved),
            _ => Err(ParseInventoryStatusError { got: s.to_string() }),
        }
    }
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

/// Lowercase-hex sha256 digest — THE one content-hash encoding (Round
/// 460 consolidation: six hand-rolled copies across four crates included
/// the cross-crate claim-pin invariant, stamped by the typing-candidates
/// report and re-checked by the proposals import — two implementations
/// of one invariant is the half-enforced-invariant class, R305). Every
/// sha256 pin in the system (claim pins, authority-artifact pins,
/// content-drift hashes, quote pins) routes here.
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for b in digest {
        use std::fmt::Write;
        let _ = write!(out, "{b:02x}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The encoding is load-bearing for every stored pin: 64 lowercase
    /// hex chars, deterministic.
    #[test]
    fn sha256_hex_stable_64_lowercase_chars() {
        let h = sha256_hex(b"test");
        assert_eq!(h.len(), 64);
        assert_eq!(h, sha256_hex(b"test"));
        assert_ne!(h, sha256_hex(b"other"));
        assert!(h
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn decision_status_open_label_and_axiom_exemption() {
        // R578 — Open completes the decision lifecycle and joins Removed as an
        // axiom-exempt state (no in-force decision to back with code/tests).
        assert_eq!(DecisionStatus::Open.as_str(), "open");
        assert!(DecisionStatus::Open.is_axiom_exempt());
        assert!(DecisionStatus::Removed.is_axiom_exempt());
        assert!(!DecisionStatus::Active.is_axiom_exempt());
        assert!(!DecisionStatus::Superseded.is_axiom_exempt());
    }

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

    #[test]
    fn inventory_status_as_str_from_str_round_trip() {
        use std::str::FromStr;
        for s in [
            InventoryStatus::Active,
            InventoryStatus::Deprecated,
            InventoryStatus::Reserved,
        ] {
            assert_eq!(InventoryStatus::from_str(s.as_str()), Ok(s));
        }
        // Case-insensitive, matching the prior to_ascii_lowercase parsers.
        assert_eq!(
            InventoryStatus::from_str("DEPRECATED"),
            Ok(InventoryStatus::Deprecated)
        );
        // as_str matches the serde snake_case representation.
        assert_eq!(InventoryStatus::Active.as_str(), "active");
    }

    #[test]
    fn binding_kind_as_str_from_tag_round_trip() {
        // Pins the as_str <-> from_tag round-trip the projection layer relies
        // on (it lowers kind to a tag for Salsa, then reconstructs via
        // from_tag().expect(...)). A typo in either arm would break this test
        // instead of silently mis-rendering a binding's kind.
        for k in [BindingKind::Implements, BindingKind::References] {
            assert_eq!(BindingKind::from_tag(k.as_str()), Some(k));
        }
        // Tags equal the serde lowercase representation.
        assert_eq!(BindingKind::Implements.as_str(), "implements");
        assert_eq!(BindingKind::References.as_str(), "references");
        // Unknown / wrong-case tags do not parse (no silent default).
        assert_eq!(BindingKind::from_tag("IMPLEMENTS"), None);
        assert_eq!(BindingKind::from_tag("satisfies"), None);
        assert_eq!(BindingKind::from_tag(""), None);
    }

    #[test]
    fn inventory_status_from_str_rejects_unknown() {
        use std::str::FromStr;
        let err = InventoryStatus::from_str("retired").unwrap_err();
        assert_eq!(err.got, "retired");
        assert_eq!(
            err.to_string(),
            "`retired` invalid (expected active|deprecated|reserved)"
        );
    }
}
