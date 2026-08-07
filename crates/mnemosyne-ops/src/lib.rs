//! `mnemosyne-ops` — shared in-process orchestration consumed by the
//! `mnemosyne-cli` bin and the `mnemosyne-mcp` server.
//!
//! R316 eliminated the MCP→CLI subprocess spawn; R319 extracts the
//! orchestration into this dedicated library so neither binary depends on
//! the other. Both link `mnemosyne-ops` and call typed Rust functions:
//! mutate via [`run_atomic_mutate`], reads via [`query`] / [`validate`] /
//! [`style`], cascade render via [`cascade`]. The bins keep only their own
//! I/O concerns (arg parsing + stdout for the CLI; MCP protocol for the
//! server).

pub mod cascade;
pub mod query;
pub mod style;
pub mod validate;

/// Re-exported so a consumer talks to its kernel rather than past it into the
/// config crate (Round 1000): every path override crossing into `ops` is one
/// of these, and constructing one is where a caller says what its path is
/// relative to.
pub use mnemosyne_config::AbsolutePath;

/// Re-exported so a consumer of a verdict can enumerate the sources one can
/// carry without reaching past its kernel into the validator (Round 1009).
pub use mnemosyne_validate::verdict::ViolationSource;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use mnemosyne_atomic::{
    AtomicMutateError, AtomicMutateReceipt, AtomicStore, ContentExcerpt, PopulationCensus,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use cascade::{validate_atomic_store, AtomicValidationSummary};
pub use query::{
    list_changelog, list_inventory, list_sections, query_inventory, query_section, query_term,
    InventoryEntryView, ListSectionsReport, QuerySectionMode, QueryTermInput,
};
pub use style::{style_check, StyleCheckInput, StyleCheckReport};
pub use validate::{validate_workspace, ValidateWorkspaceReport};

/// Errors surfaced from any op. Thin wrapper that preserves the structured
/// `AtomicMutateError` variant so callers (mcp) can map cleanly to MCP
/// error categories without reparsing strings.
#[derive(Debug, Error)]
pub enum OpError {
    #[error("{0}")]
    Mutate(#[from] AtomicMutateError),
    #[error("redact: {0}")]
    Redact(#[from] mnemosyne_atomic::RedactError),
    #[error("{0}")]
    Other(String),
}

impl From<anyhow::Error> for OpError {
    fn from(e: anyhow::Error) -> Self {
        OpError::Other(format!("{:#}", e))
    }
}

impl From<std::io::Error> for OpError {
    fn from(e: std::io::Error) -> Self {
        OpError::Other(format!("io: {}", e))
    }
}

/// Outcome of a successful atomic mutate — the receipt the primitive
/// produced. The atomic store is the only artifact; there is nothing to
/// regenerate.
#[derive(Debug, Clone, Serialize)]
pub struct MutateOutcome {
    pub receipt: AtomicMutateReceipt,
}

/// Input to the convenience-form `redact_term` op.
#[derive(Debug, Clone, Serialize)]
pub struct RedactTermInput {
    pub pattern: String,
    pub replacement: String,
    pub regex: bool,
    pub case_insensitive: bool,
    pub scope: Option<String>,
    pub dry_run: bool,
    pub reason: String,
    pub applied_in: String,
    pub kind: Option<String>,
}

/// Resolve the sidecar path with the same precedence chain the CLI uses:
/// explicit override → `[atomic] sidecar_path` config → built-in
/// `<workspace>/docs/.atomic/workspace.atomic.json`. `anchor` is a discovery
/// start; workspace-relative paths join the config-declared `[workspace]
/// root` (see [`cascade::workspace_root_from`]), so this delegates fully to
/// the anchor-aware cascade resolver rather than joining to `anchor`.
pub fn resolve_sidecar(anchor: &Path, sidecar: Option<&AbsolutePath>) -> anyhow::Result<PathBuf> {
    cascade::resolve_sidecar(anchor, sidecar)
}

/// The FILES a store projection reads, resolved (Round 772) — the discovered
/// config, the atomic sidecar that config names, and the canon-order file it
/// declares. Exactly the inputs [`playable_world_report`] and
/// [`quest_graph_report`] open, by the same resolution they use.
///
/// This exists for a build-time bake, which must declare its inputs to cargo so
/// an edit regenerates the artifact. Declaring them by GUESS is what R770 did
/// and it was wrong for the first real consumer: the built-in default sidecar
/// was named while the workspace declared `[atomic] sidecar_path`, so the store
/// the projection actually read was never watched and a rebuilt store left the
/// bake silently stale. A declared input has to be the file the loader OPENS, so
/// it is derived from the loader's own resolver here rather than restated.
///
/// A workspace with no config yet still gets its would-be `mnemosyne.toml`
/// declared: CREATING one moves the sidecar, and an undeclared creation is the
/// same staleness one step later.
///
/// # Errors
///
/// [`OpError`] if the config cannot be read or the sidecar cannot be resolved —
/// the same failures the projection would hit, raised before it runs.
pub fn projection_inputs(
    workspace_root: &Path,
    sidecar: Option<&AbsolutePath>,
) -> Result<Vec<PathBuf>, OpError> {
    let mut inputs = Vec::new();
    match mnemosyne_config::discover_config(workspace_root)? {
        Some(loaded) => {
            inputs.push(loaded.config_path);
            if let Some(order) = loaded
                .config
                .continuity
                .as_ref()
                .and_then(|c| c.canon_order_path.as_ref())
            {
                let declared = PathBuf::from(order);
                inputs.push(if declared.is_absolute() {
                    declared
                } else {
                    loaded.workspace_root.join(declared)
                });
            }
        }
        None => inputs.push(workspace_root.join("mnemosyne.toml")),
    }
    inputs.push(resolve_sidecar(workspace_root, sidecar)?);
    Ok(inputs)
}

/// Run an atomic mutate primitive in-process: load the store, invoke the
/// supplied closure against it, and return the receipt. The closure
/// persists the store itself (`save_with_receipt`); the atomic store is the
/// only artifact, so there is nothing further to regenerate.
pub fn run_atomic_mutate<F>(
    workspace_root: &Path,
    sidecar: Option<&AbsolutePath>,
    primitive: F,
) -> Result<MutateOutcome, OpError>
where
    F: FnOnce(&mut AtomicStore, &Path) -> Result<AtomicMutateReceipt, AtomicMutateError>,
{
    let sidecar_path = resolve_sidecar(workspace_root, sidecar)?;
    let mut store =
        AtomicStore::load(&sidecar_path).map_err(|e| OpError::Other(format!("{}", e)))?;
    let receipt = primitive(&mut store, &sidecar_path)?;
    Ok(MutateOutcome { receipt })
}

/// Resolve the workspace's `schema.entry_id_prefix` for the Round 424
/// append conformance gate. Single resolution path shared by the CLI and
/// the MCP server so both wires enforce the identical policy: absent
/// `[schema]` falls back to [`SchemaSection::mnemosyne_preset`] (pre-143
/// back-compat, same as the CLI schema cache); a missing mnemosyne.toml or
/// a malformed config fails loud — the gate cannot know its policy.
///
/// [`SchemaSection::mnemosyne_preset`]: mnemosyne_config::SchemaSection::mnemosyne_preset
pub fn workspace_entry_id_prefix(workspace_root: &Path) -> Result<String, OpError> {
    let loaded = mnemosyne_config::discover_config(workspace_root)?.ok_or_else(|| {
        OpError::Other(
            "mnemosyne.toml not found — entry_id_prefix gate policy unresolvable".to_string(),
        )
    })?;
    Ok(loaded
        .config
        .schema
        .map(|s| s.entry_id_prefix)
        .unwrap_or_else(|| mnemosyne_config::SchemaSection::mnemosyne_preset().entry_id_prefix))
}

/// The tracked recorded-population report: a program's output, with the
/// sentence that says so stored beside the numbers (Round 979).
///
/// The wrapper exists so a reader who opens the file learns what it is without
/// having to find the gate that writes it. `axes` is the payload, and it is
/// exactly `Vec<PopulationCensus>` — the same type the store field holds — so
/// the file format is not a second contract that could drift from the field: it
/// IS the field's serialization, and the compiler owns the correspondence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PopulationCensusReport {
    /// Why these bytes may not be hand-edited.
    pub generated: String,
    pub axes: Vec<PopulationCensus>,
}

/// The sentence [`render_population_census`] stamps into every report.
const CENSUS_REPORT_NOTE: &str =
    "These counts are a program's output, not a claim. Do not hand-edit \
     them: regenerate the report in the SAME commit as the change that moved \
     the population, so this file's history is what each axis said when.";

/// Render the tracked report for a recount (Round 979).
///
/// One home for the bytes, shared by the gate that blesses the file and by any
/// reader that compares against it, so "what does a current report look like"
/// has one answer.
pub fn render_population_census(axes: &[PopulationCensus]) -> Result<String, OpError> {
    let report = PopulationCensusReport {
        generated: CENSUS_REPORT_NOTE.to_string(),
        axes: axes.to_vec(),
    };
    let mut out = serde_json::to_string_pretty(&report)
        .map_err(|e| OpError::Other(format!("render population census: {}", e)))?;
    out.push('\n');
    Ok(out)
}

/// Where this workspace keeps its recorded-population report, or `None` when it
/// keeps none (Round 979).
///
/// Resolved through the config so the gate that writes the file and the append
/// path that reads counts out of it cannot end up on different paths.
pub fn workspace_census_report_path(workspace_root: &Path) -> Result<Option<PathBuf>, OpError> {
    let Some(loaded) = mnemosyne_config::discover_config(workspace_root)? else {
        return Ok(None);
    };
    Ok(loaded
        .config
        .census
        .map(|c| loaded.workspace_root.join(c.report)))
}

/// The recorded population this workspace's report states, for an append that
/// asked to record it (Round 979).
///
/// SINGLE RESOLUTION PATH, shared by the CLI and the MCP server. The reason is
/// the one `CLAUDE.md` states as an anti-pattern: two write paths into one
/// field, each enforcing its own idea of what the field may hold, is a field
/// with no invariant at all. Here both wires take a BOOLEAN and land here, so
/// there is no second reading of the report to diverge from this one — and no
/// wire through which a caller could supply a count of their own.
///
/// Every failure is loud. A workspace that declares no report, or declares one
/// that is missing or unparseable, cannot record a census, and saying so beats
/// appending an entry whose census is silently empty — an absent field reads as
/// "this round made no census claim", which would be a lie the store keeps.
pub fn workspace_population_census(
    workspace_root: &Path,
) -> Result<Vec<PopulationCensus>, OpError> {
    let path = workspace_census_report_path(workspace_root)?.ok_or_else(|| {
        OpError::Other(
            "this workspace declares no [census] report, so there is no recount \
             to record — add `[census] report = \"<path>\"` to mnemosyne.toml, or \
             append without recording a census"
                .to_string(),
        )
    })?;
    let raw = std::fs::read_to_string(&path).map_err(|e| {
        OpError::Other(format!(
            "read the census report at {}: {} — the report is written by the \
             workspace's own recount, so a missing file means the recount has \
             never been run here",
            path.display(),
            e
        ))
    })?;
    let report: PopulationCensusReport = serde_json::from_str(&raw).map_err(|e| {
        OpError::Other(format!(
            "parse the census report at {}: {}",
            path.display(),
            e
        ))
    })?;
    if report.axes.is_empty() {
        return Err(OpError::Other(format!(
            "the census report at {} states no axis, so recording it would file \
             an empty population under this entry",
            path.display()
        )));
    }
    Ok(report.axes)
}

/// Load the atomic store at the resolved sidecar path.
///
/// A missing sidecar is NOT an error — `AtomicStore::load` already returns an
/// empty store for a fresh workspace. This propagates only genuine failures
/// (corrupt JSON, IO error, or a newer-than-supported `schema_version`) so a
/// corrupt SSOT fails loud instead of silently reading as empty (the prior
/// `unwrap_or_default` masked corruption as a clean empty store).
pub fn load_atomic_store(
    workspace_root: &Path,
    sidecar: Option<&AbsolutePath>,
) -> Result<AtomicStore, OpError> {
    // A SIDECAR THE CALLER NAMED IS AN ASSERTION THAT IT EXISTS, and this is
    // the READ path, so an absent one is a typo rather than a bootstrap. The
    // distinction is exactly the `Option`: `None` is the workspace's own
    // sidecar, whether built in or declared by `[atomic] sidecar_path`, and
    // that one may legitimately not exist yet. Writes are unaffected — they go
    // through `run_atomic_mutate`, which loads for itself, so
    // `add-entity-kind --sidecar side.json` still CREATES the file.
    if let Some(named) = sidecar {
        return load_named_store(named, "the sidecar store");
    }
    let sidecar_path = resolve_sidecar(workspace_root, sidecar)?;
    AtomicStore::load(&sidecar_path).map_err(|e| OpError::Other(format!("{}", e)))
}

/// Load a store the CALLER NAMED, refusing a path that is not there.
///
/// The counterpart to [`load_atomic_store`] and deliberately the opposite
/// policy on one point. `AtomicStore::load` answers a missing path with an
/// empty store, which is what the workspace's own sidecar needs: a store that
/// has not been written yet is the bootstrap state, not an error. A store the
/// caller named is the other case — the caller is asserting the file exists,
/// and an empty store stands in for it silently.
///
/// That silence is the whole of the difference. Both render-acceptance gates
/// report ABSENCE of findings as their pass signal (`off_path` / `leaks`
/// empty), so an unreadable `against` produced a clean verdict from a gate
/// that had evaluated nothing — the failure mode the contract names in its own
/// words, that a gate which evaluated NOTHING must never read the same as one
/// that PASSED.
///
/// # Errors
///
/// [`OpError`] if the file is absent, or if it is present and will not parse.
pub fn load_named_store(named: &AbsolutePath, what: &str) -> Result<AtomicStore, OpError> {
    let path = named.as_path();
    if !path.exists() {
        return Err(OpError::Other(format!(
            "{what} `{}` does not exist. A store the caller NAMED must be there \
             to be read: an absent one would otherwise load as an EMPTY store, \
             and an answer about nothing reads exactly like an answer that \
             found nothing — 0 findings, exit 0",
            path.display()
        )));
    }
    AtomicStore::load(path).map_err(|e| OpError::Other(format!("{}", e)))
}

/// Every registered entity's declared kind — `entity_id -> kind` over the whole
/// registry, read straight from `AtomicStore.entities`. The bulk companion to
/// [`entity_dossier`] (which answers one entity): a consumer that owns a kind
/// registry validates it against the store in one read instead of N. tide's
/// object / place gates use it — does every store `kind:object` have a screen
/// name, and is every named id a real store object of that kind.
///
/// # Errors
///
/// [`OpError`] if the store (or its sidecar) cannot be read.
pub fn entity_kinds(
    workspace_root: &Path,
    sidecar: Option<&AbsolutePath>,
) -> Result<BTreeMap<String, String>, OpError> {
    let store = load_atomic_store(workspace_root, sidecar)?;
    Ok(store
        .entities
        .iter()
        // A report map of TEXT — the id becomes a string at this output edge.
        .map(|(id, e)| (id.to_string(), e.kind.to_string()))
        .collect())
}

/// One typed claim as the store holds it, with the coordinates that say WHOSE
/// claim it is (Round 940). The subject/object legs are the claim; `frame` and
/// `branch` are what stop a consumer reading a rumour as a fact and a fork's
/// claim as the trunk's.
///
/// The object keeps its [`mnemosyne_core::TypedObject`] shape rather than being
/// stringified at this edge, unlike [`entity_kinds`]: the shapes are what a
/// consumer dispatches on (an `opened_by = f-*` bridge joins against a fact id,
/// a `state = token` does not), and a consumer that had to re-parse a rendered
/// object would be hand-parsing our store again — the thing this read exists to
/// stop.
#[derive(Debug, Clone, Serialize)]
pub struct TypedClaimRow {
    pub fact_id: String,
    pub subject: String,
    pub object: mnemosyne_core::TypedObject,
    /// The frame the claim is held in — `ground-truth`, or a belief frame. A
    /// belief-frame typed claim is a real authored shape, not a hypothetical:
    /// the map corpus's `f-rumour-bell` types the drowned bell as `ringing` in
    /// the `townsfolk` frame, which is what the town says and not what is so.
    pub frame: String,
    /// The branch (world) the claim is declared on. Authored data spreads one
    /// subject's claims across branches — the first consumer's store carries
    /// `pursues` for one character on three.
    pub branch: String,
}

/// Every typed claim in the store, keyed by predicate — `predicate ->
/// [TypedClaimRow]`, read straight from `AtomicStore.narrative_facts`.
///
/// The typed-leg companion to [`entity_kinds`]: a consumer asking "which facts
/// carry predicate P, and whose claims are they" gets one read instead of
/// opening our sidecar itself. Round 939 measured that hand-parse happening in a
/// live consumer's build (`bake_viewpoint` scans `narrative_facts` for its
/// `pred-plays` subject) while the kernel held the same scan privately in its
/// quest axis — the capability existed and only the door was missing.
///
/// UNFILTERED on purpose. A `predicates` parameter would have to answer what an
/// empty list means, and both answers are traps: "empty = all" is a footgun, and
/// "empty = nothing" makes a not-computed result indistinguishable from an
/// absent one (the R924 class). Keyed by predicate, a consumer's filter is a map
/// lookup, and an empty vec can only mean the store holds no such claim.
///
/// Rows arrive fact-id ordered within each predicate, so a bake reading this is
/// byte-stable across runs. That order is INHERITED — `narrative_facts` is keyed
/// by fact id — rather than imposed by a sort here, because a sort over an
/// already-ordered source is a guard no test can discriminate. The order is a
/// promise to the caller either way, so the authored pin asserts it against the
/// output and would redden if this read ever collected from an unordered map.
///
/// # Errors
///
/// [`OpError`] if the store (or its sidecar) cannot be read.
pub fn typed_claims(
    workspace_root: &Path,
    sidecar: Option<&AbsolutePath>,
) -> Result<BTreeMap<String, Vec<TypedClaimRow>>, OpError> {
    let store = load_atomic_store(workspace_root, sidecar)?;
    let mut by_predicate: BTreeMap<String, Vec<TypedClaimRow>> = BTreeMap::new();
    for (fact_id, fact) in &store.narrative_facts {
        let Some(claim) = &fact.typed else { continue };
        by_predicate
            .entry(claim.predicate.to_string())
            .or_default()
            .push(TypedClaimRow {
                fact_id: fact_id.to_string(),
                subject: claim.subject.to_string(),
                object: claim.object.clone(),
                frame: fact.frame.to_string(),
                branch: fact.branch.to_string(),
            });
    }
    Ok(by_predicate)
}

/// Every section's narrative-prose `content_excerpt` (R756 P3a) — `section_id ->
/// ContentExcerpt`, read straight from `AtomicStore.sections`. The bulk read the
/// engine's `store_passages` (R757 P3b) projects into provenance-bound `Passage`s,
/// so a manuscript-less consumer (a generic renderer, pinion) gets the prose FROM
/// THE STORE with no per-consumer anchor file. Sections with no excerpt are
/// omitted; the excerpt's `text_sha256` is the offline drift anchor (R404), not
/// re-checked here (that is `scan_content_drift`'s job).
///
/// # Errors
///
/// [`OpError`] if the store (or its sidecar) cannot be read.
pub fn section_content_excerpts(
    workspace_root: &Path,
    sidecar: Option<&AbsolutePath>,
) -> Result<BTreeMap<String, ContentExcerpt>, OpError> {
    let store = load_atomic_store(workspace_root, sidecar)?;
    Ok(store
        .sections
        .iter()
        .filter_map(|(id, s)| {
            s.content_excerpt
                .as_ref()
                .map(|e| (id.to_string(), e.clone()))
        })
        .collect())
}

/// Read every section's authored ladder (Round 768) — `section_id ->
/// SectionLadder` over the sections that declare one, the interactive sibling of
/// [`section_content_excerpts`]. Sections with no ladder are omitted (a scene the
/// reader only reads is not an empty ladder, it is no ladder).
///
/// # Errors
///
/// [`OpError`] if the store (or its sidecar) cannot be read.
pub fn section_ladders(
    workspace_root: &Path,
    sidecar: Option<&AbsolutePath>,
) -> Result<BTreeMap<String, mnemosyne_atomic::SectionLadder>, OpError> {
    let store = load_atomic_store(workspace_root, sidecar)?;
    Ok(store
        .sections
        .iter()
        .filter_map(|(id, s)| s.ladder.as_ref().map(|l| (id.to_string(), l.clone())))
        .collect())
}

/// The `[continuity]` policy view both read ops resolve from ONE config
/// discovery (Round 435 single-path rule, the `workspace_entry_id_prefix`
/// precedent; folded to a single `discover_config` in Round 436).
struct ContinuityPolicy {
    root: PathBuf,
    continuity: Option<mnemosyne_config::ContinuitySection>,
}

fn continuity_policy(workspace_root: &Path) -> Result<ContinuityPolicy, OpError> {
    let loaded = mnemosyne_config::discover_config(workspace_root)?;
    Ok(match loaded {
        Some(l) => ContinuityPolicy {
            root: l.workspace_root,
            continuity: l.config.continuity,
        },
        None => ContinuityPolicy {
            root: workspace_root.to_path_buf(),
            continuity: None,
        },
    })
}

/// Resolve the declared canon-order FILE from a [`ContinuityPolicy`]:
/// explicit override (bypasses the sha256 pin — the pin claims nothing
/// about a different file, the R428 `--catalog` rule) >
/// `[continuity].canon_order_path` (+ optional pin) > empty declaration.
/// Construction into a `CanonOrder` happens after the store loads — the
/// per-branch composition needs the fork ancestry (Round 438).
fn resolve_canon_order_file(
    policy: &ContinuityPolicy,
    order_override: Option<&AbsolutePath>,
) -> Result<mnemosyne_validate::continuity::CanonOrderFile, OpError> {
    use mnemosyne_validate::continuity::{load_canon_order, CanonOrderFile};
    let cont = policy.continuity.as_ref();
    match (
        order_override,
        cont.and_then(|c| c.canon_order_path.as_ref()),
    ) {
        // Round 1000 — the override arrives already resolved against a base
        // its own wire named (R538's rule now lives in the CLI, where the
        // working directory is the user's own choice). Bypasses the sha256 pin
        // (the pin claims nothing about a different file — the R428
        // `--catalog` rule).
        (Some(p), _) => load_canon_order(p.as_path(), None).map_err(OpError::Other),
        (None, Some(p)) => load_canon_order(
            &policy.root.join(p),
            cont.and_then(|c| c.canon_order_sha256.as_deref()),
        )
        .map_err(OpError::Other),
        (None, None) => Ok(CanonOrderFile::default()),
    }
}

/// Resolve the declared narrative-rules FILE from a [`ContinuityPolicy`]
/// (Round 449, the canon-order resolution mirrored): explicit override
/// (bypasses the sha256 pin — the pin claims nothing about a different
/// file, the R428 `--catalog` rule) > `[continuity].rules_path` (+ optional
/// pin) > empty rule set (no rules authored = the recorded-edge gate
/// alone).
fn resolve_narrative_rules(
    policy: &ContinuityPolicy,
    rules_override: Option<&AbsolutePath>,
) -> Result<mnemosyne_validate::continuity::NarrativeRulesFile, OpError> {
    use mnemosyne_validate::continuity::{load_narrative_rules, NarrativeRulesFile};
    let cont = policy.continuity.as_ref();
    match narrative_rules_path(policy, rules_override) {
        // An override bypasses the sha256 pin: the pin claims nothing about a
        // different file (the R428 `--catalog` rule, as `resolve_canon_order_file`
        // applies it one axis over).
        Some(path) if rules_override.is_some() => {
            load_narrative_rules(&path, None).map_err(OpError::Other)
        }
        Some(path) => load_narrative_rules(&path, cont.and_then(|c| c.rules_sha256.as_deref()))
            .map_err(OpError::Other),
        None => Ok(NarrativeRulesFile::default()),
    }
}

/// WHICH narrative-rules file a read will open, resolved without opening it.
///
/// Split out of [`resolve_narrative_rules`] so the artifact a build DECLARES to
/// cargo and the artifact a read OPENS are one decision rather than two that
/// agree today. R772 closed exactly that divergence on the sidecar: the store a
/// bake read was not the file it watched, so editing it left the artifact
/// silently stale. The map axis is the sharpest case of the same shape — a
/// transition rule is what declares which facts are edges at all, so a baked map
/// built against a stale rules file is not slightly wrong, it is a different map.
fn narrative_rules_path(
    policy: &ContinuityPolicy,
    rules_override: Option<&AbsolutePath>,
) -> Option<PathBuf> {
    // Round 1000 — an explicit override is already absolute and is used as
    // given; only the CONFIG-declared path joins the workspace root. Before
    // this the override was joined to the root here while `--order`'s was
    // joined to the working directory in `resolve_canon_order_file`: two
    // explicit overrides on one command resolving against different bases,
    // which nothing said and nothing could have caught.
    if let Some(p) = rules_override {
        return Some(p.as_path().to_path_buf());
    }
    let declared = policy
        .continuity
        .as_ref()
        .and_then(|c| c.rules_path.clone())?;
    Some(policy.root.join(declared))
}

/// The files a BAKED MAP depends on — [`projection_inputs`] plus the
/// narrative-rules artifact, which no other projection opens and which decides
/// the whole content of this one.
///
/// # Errors
///
/// [`OpError`] if the workspace config or the sidecar cannot be resolved.
pub fn transition_map_inputs(
    workspace_root: &Path,
    sidecar: Option<&AbsolutePath>,
    rules_override: Option<&AbsolutePath>,
) -> Result<Vec<PathBuf>, OpError> {
    let mut inputs = projection_inputs(workspace_root, sidecar)?;
    let policy = continuity_policy(workspace_root)?;
    if let Some(rules) = narrative_rules_path(&policy, rules_override) {
        inputs.push(rules);
    }
    Ok(inputs)
}

/// Compose the declaration with the store's fork ancestry (Round 438) and
/// forward confluence-suffixes (Round 533) into the queryable order — one
/// construction path for both reads, BOTH world-line directions.
fn compose_canon_order(
    decl: &mnemosyne_validate::continuity::CanonOrderFile,
    store: &AtomicStore,
) -> Result<mnemosyne_validate::continuity::CanonOrder, OpError> {
    use mnemosyne_validate::continuity::CanonOrder;
    CanonOrder::from_declaration(decl, &store.branches).map_err(OpError::Other)
}

/// The store + composed order for a WORLD-SCOPED read, with the Round 857
/// refusal applied: a per-world answer over an UNDECLARED canon order is empty
/// in every world and says so nowhere.
///
/// The four report wrappers that serve a per-world question route through here,
/// so the rule has one home and a new one inherits it. Callers whose SUBJECT is
/// the missing order — `authoring_frontier_report`, which exists to name every
/// unordered scene (R596), and the continuity scan, which prints `order_nodes`
/// beside `sections` (R667) — deliberately do NOT: for them an empty order is
/// input, not a defect, and that decision is ratified by their own tests.
///
/// `read` names the projection in the refusal, since the reader's next move is
/// to pass `--order` or declare `[continuity].canon_order_path`.
fn world_scoped_inputs(
    workspace_root: &Path,
    sidecar: Option<&AbsolutePath>,
    order_override: Option<&AbsolutePath>,
    read: &str,
) -> Result<(AtomicStore, mnemosyne_validate::continuity::CanonOrder), OpError> {
    let policy = continuity_policy(workspace_root)?;
    let decl = resolve_canon_order_file(&policy, order_override)?;
    let store = load_atomic_store(workspace_root, sidecar)?;
    let order = compose_canon_order(&decl, &store)?;
    mnemosyne_validate::continuity::check_order_declared(&store, &order, read)
        .map_err(OpError::Other)?;
    Ok((store, order))
}

/// The continuity-scan envelope both wires emit (Round 435): the configured
/// severity (None = `[continuity]` absent = gate disabled, scan still
/// reported) plus the full frame-scoped report. Gating policy (exit code /
/// MCP error) stays with the caller.
#[derive(Debug, Clone, Serialize)]
pub struct ContinuityScanReport {
    pub severity: Option<String>,
    /// Per-class severity for interval (timeline) violations (Round 491,
    /// design sec 7.20 step 3). `None` = OFF: an interval violation is
    /// surfaced (here and in `report-timeline-gaps`) but never gates —
    /// unlike exclusive/transition, a timeline gap can be a legitimate
    /// authored time-bend, so gating is the author's opt-in.
    pub interval_severity: Option<String>,
    pub facts: usize,
    /// The scenes the declared order places, NAMED (Round 1061) — carried from
    /// the scan verbatim; see the field it forwards for why it is not a count.
    pub order_nodes: Vec<String>,
    /// Sections in the registry — `order_nodes`' denominator (Round 667). The
    /// order's nodes are a subset (the store-boundary check rejects a node that
    /// is not a section), so a surplus here means sections on no declared road:
    /// the author's todo, named in a CLI notice, listed by
    /// `report-authoring-frontier` (R596), never gated.
    pub sections: usize,
    pub conflict_pairs_checked: usize,
    pub cross_scope_pairs: usize,
    pub unordered_pairs: usize,
    /// Evidence refs whose section holds prose but which carry no review
    /// affirmation (Round 806) — the claim was never judged against a
    /// fingerprint. Not a violation, always reported.
    pub evidence_unreviewed: usize,
    /// Facts whose quote membership could not be decided because an evidence
    /// section holds no prose (Round 811). Not a violation, always reported.
    pub fact_quotes_uncheckable: usize,
    /// Ladder rungs whose coordinate was resolved against its section's current
    /// prose (Round 817) — the denominator of the stranded verdicts. Not a
    /// violation, always reported.
    pub ladder_rungs_resolved: usize,
    /// Declared narrative rules evaluated (Round 449; 0 = no rules file).
    pub rules: usize,
    /// Of `rules`, how many are INTERVAL-class (Round 491): a nonzero count
    /// with `interval_severity` OFF is a declared-but-ungated timeline rule
    /// the CLI names in a NOTICE (the R491 opt-in nudge).
    pub interval_rules: usize,
    /// Registered branches that declare no road of their own, so their road — and
    /// their ENDING — is their lineage's (Round 614). Not an error: a world-line that
    /// diverges only in FACTS and rides the trunk on is a real shape. But the substrate
    /// cannot tell it from a divergent ending whose road was never declared, and under
    /// THAT reading the terminal gates measure the trunk's ending instead of its own —
    /// so the ambiguity is NAMED (the CLI notice), never guessed.
    pub undeclared_roads: Vec<String>,
    /// Exclusive-rule candidate pairs the declared order cannot compare.
    /// `None` = no exclusive rule was declared (Round 924).
    pub rule_unordered_pairs: Option<usize>,
    /// Same-frame same-subject typed pairs no succession PATH connects —
    /// surfaced, never gated (Round 449; path not edge, Round 452).
    /// `None` = no transition rule was declared (Round 924).
    pub unchained_state_pairs: Option<usize>,
    /// Round 916 — the subset of `unchained_state_pairs` that NO ROUTE joins in
    /// the hierarchy-augmented map: the subject is asserted on both sides of a
    /// gap no journey crosses, so no ellipsis could have covered it. Surfaced,
    /// never gated; EVERY transition rule, directed or not (Round 924 — the
    /// claim needs no genre, and on a directed rule it is conservative because
    /// the component walk symmetrizes). `None` = no transition rule was
    /// declared, so this was never computed: `Some(0)` is a measurement.
    pub unchained_unreachable_pairs: Option<usize>,
    /// Round 921 — every declared step as the REAL classifier judged it, carried
    /// through so a consumer reading this report and the gate cannot disagree
    /// about what a step IS. Always populated (R663: a knob decides whether an
    /// axis fails, never whether it is measured).
    pub step_judgements: Vec<mnemosyne_validate::continuity::StepJudgement>,
    /// Round 934 — transition rules whose `map_invented_place` completeness
    /// class could not be asked: their adjacency predicate declares no leg
    /// kind, so the store cannot say which entities are places. Not a
    /// violation; surfaced so a class that never ran does not read as a class
    /// that found nothing.
    pub completeness_unaskable: Vec<mnemosyne_validate::continuity::UnaskableCompleteness>,
    /// Round 1031 — every declared quest prerequisite judged against every road,
    /// carried through whole for the reason `step_judgements` is: the census a
    /// reader prints must be derived from the SAME walk the two violation arms
    /// were drawn from. Empty = no `requires` claim declared (never askable).
    pub quest_prerequisite_judgements:
        Vec<mnemosyne_validate::continuity::QuestPrerequisiteJudgement>,
    /// Interval-rule resolutions that could not be evaluated (operand absent
    /// on the right/bound leg, non-numeric, or ambiguous) — surfaced, never
    /// gated (Round 489, the R485 `unverifiable` class).
    /// `None` = no interval rule was declared (Round 924).
    pub interval_unverifiable: Option<usize>,
    pub violation_count: usize,
    /// Interval (timeline) violations within `violation_count` (Round 491):
    /// these gate under `interval_severity`, the structural remainder
    /// (`violation_count - interval_violation_count`) under `severity`.
    pub interval_violation_count: usize,
    pub violations: Vec<mnemosyne_validate::continuity::ContinuityViolation>,
}

/// Run the frame-scoped continuity scan (Round 431 gate, read-only half)
/// over the workspace store with the shared order/severity/rules
/// resolution (rules = Round 449).
pub fn continuity_scan(
    workspace_root: &Path,
    sidecar: Option<&AbsolutePath>,
    order_override: Option<&AbsolutePath>,
    rules_override: Option<&AbsolutePath>,
) -> Result<ContinuityScanReport, OpError> {
    let policy = continuity_policy(workspace_root)?;
    let decl = resolve_canon_order_file(&policy, order_override)?;
    let rules = resolve_narrative_rules(&policy, rules_override)?;
    let severity = policy
        .continuity
        .as_ref()
        .map(|c| c.severity.as_str().to_string());
    let interval_severity = policy
        .continuity
        .as_ref()
        .and_then(|c| c.interval_severity)
        .map(|s| s.as_str().to_string());
    let store = load_atomic_store(workspace_root, sidecar)?;
    let order = compose_canon_order(&decl, &store)?;
    let report = mnemosyne_validate::continuity::scan_continuity(&store, &order, &rules.rules)
        .map_err(OpError::Other)?;
    let interval_violation_count = report
        .violations
        .iter()
        .filter(|v| {
            matches!(
                v,
                mnemosyne_validate::continuity::ContinuityViolation::RuleIntervalViolation { .. }
            )
        })
        .count();
    Ok(ContinuityScanReport {
        severity,
        interval_severity,
        facts: report.facts,
        order_nodes: report.order_nodes.clone(),
        sections: report.sections,
        conflict_pairs_checked: report.conflict_pairs_checked,
        cross_scope_pairs: report.cross_scope_pairs,
        unordered_pairs: report.unordered_pairs,
        evidence_unreviewed: report.evidence_unreviewed,
        fact_quotes_uncheckable: report.fact_quotes_uncheckable,
        ladder_rungs_resolved: report.ladder_rungs_resolved,
        rules: report.rules,
        interval_rules: report.interval_rules,
        undeclared_roads: report.undeclared_roads.clone(),
        rule_unordered_pairs: report.rule_unordered_pairs,
        unchained_state_pairs: report.unchained_state_pairs,
        unchained_unreachable_pairs: report.unchained_unreachable_pairs,
        step_judgements: report.step_judgements.clone(),
        completeness_unaskable: report.completeness_unaskable.clone(),
        quest_prerequisite_judgements: report.quest_prerequisite_judgements.clone(),
        interval_unverifiable: report.interval_unverifiable,
        violation_count: report.violations.len(),
        interval_violation_count,
        violations: report.violations,
    })
}

/// The verdict of a `propose-verdict` dry-run transaction (Round 588).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposeVerdict {
    /// The batch applied cleanly and passed every gate — safe to commit (apply
    /// for real via `import-facts`). NOTHING was written: this is a dry run.
    Commit,
    /// The batch was rejected — see `violations`. NOTHING was written.
    Rollback,
}

impl ProposeVerdict {
    /// Stable lowercase label (matches the serde rename).
    pub fn as_str(self) -> &'static str {
        match self {
            ProposeVerdict::Commit => "commit",
            ProposeVerdict::Rollback => "rollback",
        }
    }
}

/// The result of the `propose-verdict` transaction (Round 588, R585 debt item
/// 2) — the generate-gate-repair loop's atomic unit. Apply a candidate batch to
/// a THROWAWAY in-memory clone of the store, run every applicable gate, and
/// return commit-or-rollback plus actionable violations. A pure DRY RUN: the
/// real store is never written (the scratch-sidecar contract, done in memory).
#[derive(Debug, Clone, Serialize)]
pub struct ProposeVerdictReport {
    /// The authoritative go/no-go: commit = the store's configured gate ACCEPTS
    /// this batch (safe to apply); rollback = it would reject. Mirrors
    /// `validate-continuity`'s `[continuity]` severity policy exactly (R592).
    pub verdict: ProposeVerdict,
    /// What the batch WOULD create if committed (the import summary) — present
    /// even on rollback so the agent sees the intended scope.
    pub applied_summary: String,
    pub violation_count: usize,
    /// How many of `violations` are at REJECT severity (the ones that cause the
    /// rollback). On a `commit` verdict this is 0 and any listed violations are
    /// below-reject advisories (a `warn`/`info` class, or an interval time-bend
    /// with `interval_severity` OFF) — the loop keys off `verdict`, not on
    /// `violations` being empty.
    pub gating_violation_count: usize,
    /// ALL actionable violations found (shape + continuity), regardless of
    /// severity — so the loop sees warn/info advisories even on a commit.
    pub violations: Vec<mnemosyne_validate::verdict::ActionableViolation>,
    /// Per-world dangling setups the batch WOULD leave (Round 599,
    /// unattended-loop-experiment/v2 gap A) — Expected setups with no visible
    /// payoff on a world-line, computed on the throwaway clone (R442). ADVISORY:
    /// dangling NEVER flips the verdict (the dangling-is-a-todo discipline), so a
    /// populated map can ride a `commit` OR a `rollback` caused by other findings.
    /// Surfaced HERE, in the dry run, so a loop sees a structural dangling BEFORE
    /// it commits — the frontier's `dangling_setups` was post-import only, so a
    /// bare-prefix dangle used to require a full store reset to fix. Only worlds
    /// with ≥ 1 dangling. Empty on a shape rejection.
    pub dangling_setups: BTreeMap<String, Vec<String>>,
}

/// Run the `propose-verdict` dry-run transaction (Round 588; R592 severity
/// fidelity). Loads the base store (default or `sidecar`) into a throwaway
/// clone, applies the candidate `manifest` in memory (shape invariants), then
/// runs the continuity gate over the mutated clone, mapping every finding to an
/// actionable violation. A shape rejection is fail-fast (one violation, hard
/// rollback, no gate run). The continuity verdict mirrors the store's configured
/// `[continuity]` severity EXACTLY via the shared `evaluate_continuity_gate` — a
/// dry run never rejects content the real gate accepts. Deterministic, AI out of
/// the gate, the real store never touched — the loop calls this until `commit`,
/// THEN applies for real via `import-facts`.
pub fn propose_verdict(
    workspace_root: &Path,
    sidecar: Option<&AbsolutePath>,
    order_override: Option<&AbsolutePath>,
    rules_override: Option<&AbsolutePath>,
    manifest: &mnemosyne_atomic::FactsManifest,
) -> Result<ProposeVerdictReport, OpError> {
    let policy = continuity_policy(workspace_root)?;
    let decl = resolve_canon_order_file(&policy, order_override)?;
    let rules = resolve_narrative_rules(&policy, rules_override)?;
    let mut store = load_atomic_store(workspace_root, sidecar)?;

    // 1. Apply the batch (shape invariants). A Validation breach is a shape
    //    violation → rollback (the apply is fail-fast; the partial clone is
    //    discarded, the real store untouched). Any other error is a real
    //    failure, propagated — not an authoring violation.
    let outcome = match mnemosyne_atomic::apply_facts_manifest(&mut store, manifest) {
        Ok(o) => o,
        Err(AtomicMutateError::Validation(msg)) => {
            let violations = vec![mnemosyne_validate::verdict::ActionableViolation::shape(msg)];
            return Ok(ProposeVerdictReport {
                verdict: ProposeVerdict::Rollback,
                applied_summary: "no facts applied (shape rejection)".to_string(),
                violation_count: violations.len(),
                // A shape rejection is a hard, un-appliable failure — it always
                // gates, independent of the continuity severity policy.
                gating_violation_count: violations.len(),
                violations,
                // No valid clone to analyse dangling on.
                dangling_setups: BTreeMap::new(),
            });
        }
        Err(e) => return Err(OpError::Mutate(e)),
    };

    // 2. Run the continuity gate over the MUTATED clone; map each finding to an
    //    actionable violation. The verdict mirrors the store's configured
    //    [continuity] severity EXACTLY (R592, the shared evaluate_continuity_gate
    //    that validate-continuity also uses): a class rolls back only at `reject`.
    //    ALL violations are still surfaced so the loop sees warn/info advisories.
    let order = compose_canon_order(&decl, &store)?;
    // Advisory dangling coverage on the clone (Round 599, v2 gap A): the same
    // per-world payoff analysis the frontier runs, but HERE in the dry run so a
    // loop sees a structural dangling before it commits — never gating (dangling
    // is a todo, not an error, R442).
    let dangling_setups = mnemosyne_validate::continuity::payoff_coverage(&store, &order)
        .map_err(OpError::Other)?
        .dangling_by_world();
    let report = mnemosyne_validate::continuity::scan_continuity(&store, &order, &rules.rules)
        .map_err(OpError::Other)?;
    let severity = policy.continuity.as_ref().map(|c| c.severity);
    let interval_severity = policy.continuity.as_ref().and_then(|c| c.interval_severity);
    let gate = mnemosyne_validate::continuity::evaluate_continuity_gate(
        severity,
        interval_severity,
        &report.violations,
    );
    let violations: Vec<mnemosyne_validate::verdict::ActionableViolation> = report
        .violations
        .iter()
        .map(mnemosyne_validate::verdict::continuity_actionable)
        .collect();
    let structural_gating = if matches!(severity, Some(s) if s.is_reject()) {
        gate.structural_count
    } else {
        0
    };
    let interval_gating = if matches!(interval_severity, Some(s) if s.is_reject()) {
        gate.interval_count
    } else {
        0
    };
    let gating_violation_count = structural_gating + interval_gating;
    let verdict = if gate.gates {
        ProposeVerdict::Rollback
    } else {
        ProposeVerdict::Commit
    };
    Ok(ProposeVerdictReport {
        verdict,
        applied_summary: outcome.summary,
        violation_count: violations.len(),
        gating_violation_count,
        violations,
        dangling_setups,
    })
}

/// One scene's fact coverage (Round 589) — WHICH facts are anchored (via their
/// `canon_from`) at this section, sorted. `structural` (Round 618,
/// MNEMO-GAP-005 part 3a) is the DERIVED subset of `facts` that is quest
/// plumbing (`structural_fact_ids`): a coverage read subtracts it so bookkeeping
/// does not inflate "how much narrative a scene carries". Canon-vs-invented is
/// NOT split here — it is per-branch adaptation-fidelity kept consumer-side
/// (decision C); a consumer that wants it combines this with the facts' `branch`.
///
/// BOTH WERE COUNTS UNTIL ROUND 1053, and a count cannot say WHICH fact it
/// counted. Round 1052 declared this read's census against the playable world —
/// every fact counted at a scene is named there — and could only ever check it
/// as a per-scene `≤` plus an equal total, which is an equality of NUMBERS: two
/// facts trading the coordinates they are anchored at left every number where it
/// was, and (measured, not argued) left the WHOLE report byte-identical while
/// the playable world plainly walked a reader through the difference. The same
/// shape had just been met on `not_holding` (R1050), so it is a class. No
/// contract over a count could have closed it; the wire had to name what it
/// counts. The count is `facts.len()` — kept nowhere, so it cannot drift from
/// the list it summarizes.
#[derive(Debug, Clone, Serialize)]
pub struct SceneCoverage {
    pub scene: String,
    pub facts: Vec<String>,
    pub structural: Vec<String>,
}

/// Per-world-line ownership density (Round 617, denominator corrected Round 619)
/// — of every scene a world-line TRAVELS, how many facts did it author itself.
///
/// A divergent world inherits its trunk prefix, so the frontier's zero-fact /
/// per-scene view shows it FULL by inheritance. Dividing its OWN facts
/// (`branch == B`) by its FULL traversed road (`road_scenes`, R614) surfaces a
/// world that rides a long inherited road while owning little: a low density is
/// the "looks full, owns little" dilution the gap wanted flagged. `owned_facts` =
/// facts authored on this world-line; `road_scenes` = the count of coordinates it
/// travels; `density` = `owned_facts / road_scenes`, **None** only when the world
/// travels no road at all (a store with no declared order). `main` is the trunk
/// baseline.
///
/// The denominator is the FULL traversed road, NOT the world's own DECLARED
/// segment. The Round 617 own-segment denominator was wrong twice over: it
/// suppressed the dilution signal (a divergent world reads dense on the handful
/// of scenes it declared, hiding the long inherited span that IS the dilution),
/// and it miscounted a declared-into attach coordinate as own (a silent 2× error
/// on a legal store). The full traversed road is both the honest signal and
/// bug-free — a road is never empty (bar a store with no order), so there is no
/// divide-by-zero and no confusing "rides the trunk" inversion. It is NOT claimed
/// to match any external divisor.
#[derive(Debug, Clone, Serialize)]
pub struct BranchDensity {
    /// The facts this world-line authored, NAMED (Round 1054). It shipped as
    /// `owned_facts`, a count, and a walk over every shipped read measured what
    /// that cost: moving one fact to another world-line moved this number, the
    /// `density` derived from it, and NOTHING else in the whole report. A
    /// consumer acting on a density could not open its own numerator. The count
    /// is `owned.len()` and is kept nowhere — the Round 1053 wire discipline.
    pub owned: Vec<String>,
    /// The scenes this world-line travels, NAMED (Round 1061) — the denominator
    /// of `density`, opened for the same reason the numerator was.
    ///
    /// It shipped as `road_scenes`, a count, and Round 1054 named `owned` right
    /// beside it without being able to reach this one: the corruption
    /// population stopped at the fact manifest, and a road's length is a
    /// property of the canon ORDER. Rounds 1052, 1054 and 1055 each recorded
    /// that as a limit of the walk. Round 1061 widened the population to the
    /// manifests an author actually writes, put this number to a second value
    /// for the first time, and it moved while nothing this answer names moved
    /// — a consumer holding a density could open its numerator and not its
    /// denominator. The count is `road.len()` and is kept nowhere (the R1053
    /// wire discipline).
    pub road: Vec<String>,
    pub density: Option<f64>,
}

/// The consolidated authoring FRONTIER (Round 589, R585 debt item 3) — every
/// coverage gap an unattended generate-gate-repair loop pulls its next work
/// from, JOINed from the scattered projections (payoff R442, disclosure R507,
/// quest R568, plus the store's own scene/fact structure) into one read. Pure
/// read, never gated (the dangling-is-a-todo discipline). The telling-scoped
/// gaps (quests / disclosures) are present only when a telling is given.
#[derive(Debug, Clone, Serialize)]
pub struct AuthoringFrontierReport {
    /// The telling the quest + disclosure gaps were computed for (None = the
    /// telling-scoped sections were omitted).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub telling: Option<String>,
    /// Sections with NO fact anchored (no fact's `canon_from` names them) — the
    /// empty scenes to author into, sorted. Carries NO placement axis: a placed
    /// empty and an unplaced empty land here alike (see `unplaced_scenes`).
    pub zero_fact_scenes: Vec<String>,
    /// EVERY section the declared canon order does not position (Round 667) —
    /// `section ∈ registry ∧ ∉ order.nodes()`, sorted, content-independent.
    ///
    /// The PLACEMENT axis, which had no owner until this field: R596's
    /// `unordered_scenes` filters to FACT-BEARING sections (its question is
    /// renderability, so an empty unplaced scene is deliberately out of it), and
    /// `zero_fact_scenes` filters on content with no placement predicate at all
    /// — so an EMPTY unplaced section was computed NOWHERE, and sat in
    /// `zero_fact_scenes` indistinguishable from a placed empty. Round 663
    /// injected exactly that (a bare registered section) and read the silence as
    /// proof the substrate could not make the comparison at all.
    ///
    /// NOT named for the ROAD, deliberately: this reads `order.nodes()`, the
    /// PRECEDENCE union, and reading a road off that node set is the R611 defect
    /// (`continuity.rs`, "Reading the ROAD off the PRECEDENCE node set"). `road`
    /// is reserved for the bounded per-world axis (`names` / `linearize`). The
    /// two coincide for the global union today; the name must not be what pins
    /// that. `positioned` is the word the canon-coordinate check uses for this
    /// same predicate.
    ///
    /// `unordered_scenes` is now derived from this set, so placement has ONE
    /// resolver. Deliberately NOT in `total_gaps`: every member is already
    /// counted there exactly once, via `zero_fact_scenes` (empty) or
    /// `unordered_scenes` (fact-bearing) — two disjoint sets, partitioned on
    /// `fact_count`, whose union covers this one. Never gated — an unplaced
    /// section may simply be unplaced YET, the mode `FactCanonOffBranch`
    /// already tolerates over the SAME predicate (a coordinate no order
    /// positions is the orderless/forward-declared mode, tolerated not flagged).
    pub unplaced_scenes: Vec<String>,
    /// Fact-bearing sections NOT placed in the resolved canon order (Round 596,
    /// unattended-loop-experiment/v1 Finding 4) — a scene carries facts but no
    /// declared order edge reaches it, so `report-playthrough-manuscript` /
    /// `report-fork-tree` (and any render / pinion consumer) cannot place it.
    /// When NO canon order is declared, EVERY fact-bearing scene is unordered:
    /// the frontier's signal that the order artifact — required for a renderable
    /// store, but not part of the fact manifest — is missing. Sorted.
    pub unordered_scenes: Vec<String>,
    /// Fact count anchored per section (every section, including zero) — the
    /// per-node coverage map, section-id order.
    pub scene_coverage: Vec<SceneCoverage>,
    /// Per-world-line ownership density (Round 617) — `main` + every registered
    /// branch, so a divergent world that looks full by inheritance but owns
    /// little is visible. Pure read, never gated. See [`BranchDensity`].
    pub branch_owned_density: BTreeMap<String, BranchDensity>,
    /// The derived STRUCTURAL (quest-plumbing) fact ids (Round 619,
    /// `structural_fact_ids`), sorted — the union of `scene_coverage.structural`
    /// over every scene, exposed flat so a consumer can JOIN it to each fact's
    /// `branch` (retiring an external id-prefix heuristic) rather than walking
    /// the census for it. Canon-vs-invented is NOT here (consumer-side,
    /// decision C).
    ///
    /// It was the flat set BESIDE a per-scene aggregate until Round 1053, when
    /// the census began naming what it counts; the two are now the same ids
    /// keyed two ways, and a structural fact anchored at a section this store
    /// does not register is the one thing here the census cannot hold.
    pub structural_facts: Vec<String>,
    /// Dangling setups per world-line (Expected facts with no visible payoff,
    /// R442) — the Chekhov guns still to fire. Only worlds with ≥ 1 dangling.
    pub dangling_setups: BTreeMap<String, Vec<String>>,
    /// Quests whose giving setup could not be bound (R568) — no `completed_by`
    /// fact, or one that pays off no `Expected` setup, told apart by the R1037
    /// reason. Present only when a telling is given.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unresolved_quests: Option<Vec<mnemosyne_validate::continuity::UnresolvedQuest>>,
    /// Facts never given an explicit disclosure decision under the telling
    /// (withheld by default, R507). Present only when a telling is given.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub never_planned_disclosures: Option<Vec<String>>,
    /// Disclosures SEATED BEFORE THE FACT IS TRUE (Round 949) — an authored
    /// `surface.scene` that sits earlier on the fact's own road than its
    /// `canon_from`. Present only when a telling is given.
    ///
    /// Seating a disclosure LATE is the whole point of a telling: the reader
    /// learns at scene twenty what has been so since scene nine. Seating it
    /// EARLY has no such reading — it tells the reader something the store
    /// itself says is not yet so. Round 949 constructed one and measured what
    /// happened: the store imported it, the renderer printed the fact ten scenes
    /// before it was true, the same scene then placed one character in two rooms
    /// at once, and a place withheld until a later scene was named through it —
    /// while `validate-continuity` read `violations: 0`, coverage said nothing,
    /// and this report said `0 gap(s)`. The gate judges fact EXTENTS, which were
    /// never wrong; nothing was reading the SEAT.
    ///
    /// A belief held early is not this: that is a frame, and it has its own
    /// field. Nothing here is inferred — both coordinates are authored, and a
    /// pair this cannot ORDER (either coordinate off the fact's road) is
    /// skipped rather than guessed at.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disclosures_seated_before_truth: Option<Vec<SeatBeforeTruth>>,
    /// The MAP axis's gaps (Round 891) — registered places no declared map
    /// connects, plus costs/guards keyed to a non-edge. Order-free and
    /// telling-free, like the map itself, so it is present unconditionally.
    ///
    /// The axis was built (R697 edges, R710 costs, R722 guards) and read (R875)
    /// entirely after this JOIN was written (R589), and no round reached back —
    /// so the loop's work source could not pull map work, and a store whose
    /// scenes have no way between them reported its OTHER gaps and read as
    /// healthy on this one. `transition_rules: 0` is the third state, not zero
    /// work: see [`mnemosyne_validate::continuity::MapFrontierReport`].
    pub map_frontier: mnemosyne_validate::continuity::MapFrontierReport,
    /// Total distinct gap items across every category — the loop's "work
    /// remaining" gauge (a dangling setup counted once across worlds).
    pub total_gaps: usize,
}

/// A disclosure whose authored seat sits earlier than the fact it discloses
/// becomes true (Round 949).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
pub struct SeatBeforeTruth {
    pub fact_id: String,
    /// The world whose road ordered the two coordinates — the fact's own branch.
    pub world: String,
    /// The authored `surface.scene`.
    pub seated_at: String,
    /// The fact's `canon_from` — where the store says it becomes so.
    pub true_from: String,
}

/// Compose the authoring-frontier report (Round 589). ONE store load + order
/// compose, then every sub-projection runs over it (no redundant reloads): the
/// scene/fact structure gives zero-fact scenes + per-node coverage, R442 payoff
/// gives per-world dangling setups, and — when a telling is given — R568 quests
/// give the unresolved set and R507 disclosure gives the never-planned facts. A
/// pure read JOIN, never gated.
pub fn authoring_frontier_report(
    workspace_root: &Path,
    sidecar: Option<&AbsolutePath>,
    order_override: Option<&AbsolutePath>,
    telling: Option<&str>,
    rules_override: Option<&AbsolutePath>,
) -> Result<AuthoringFrontierReport, OpError> {
    let policy = continuity_policy(workspace_root)?;
    let decl = resolve_canon_order_file(&policy, order_override)?;
    // The map axis (Round 891). Resolved through the SAME artifact resolver the
    // gate and `transition_map_report` use — the rule is what DECLARES the
    // adjacency predicate, which core must not know (invariant 4).
    let rules = resolve_narrative_rules(&policy, rules_override)?;
    let store = load_atomic_store(workspace_root, sidecar)?;
    let order = compose_canon_order(&decl, &store)?;
    let map_frontier = mnemosyne_validate::continuity::map_frontier(&store, &rules.rules)
        .map_err(OpError::Other)?;

    // Scene coverage: every section starts at zero, each fact credits its
    // canon_from (the anchor). A canon_from is always an existing section (the
    // shape gate), so nothing lands outside the map. The structural subset
    // (Round 618, MNEMO-GAP-005) is derived once — quest plumbing that a
    // coverage read subtracts (no stored marker: canon/invented stays
    // consumer-side, decision C).
    let structural_ids =
        mnemosyne_validate::continuity::structural_fact_ids(&store).map_err(OpError::Other)?;
    let empty_scene = || -> BTreeMap<String, Vec<String>> {
        store
            .sections
            .keys()
            .map(|s| (s.to_string(), Vec::new()))
            .collect()
    };
    let mut anchored = empty_scene();
    let mut structural_at = empty_scene();
    for (fid, fact) in &store.narrative_facts {
        if let Some(at) = anchored.get_mut(fact.canon_from.as_str()) {
            at.push(fid.to_string());
        }
        if structural_ids.contains(fid.as_str()) {
            if let Some(at) = structural_at.get_mut(fact.canon_from.as_str()) {
                at.push(fid.to_string());
            }
        }
    }
    let zero_fact_scenes: Vec<String> = anchored
        .iter()
        .filter(|(_, facts)| facts.is_empty())
        .map(|(s, _)| s.clone())
        .collect();
    // Placement (Round 667), the ONE resolver: every section the order does not
    // position, content-independent. The projection below is its consumer, so a
    // section's placement is decided in exactly one place.
    let ordered: BTreeSet<&mnemosyne_core::SectionId> = order.nodes().collect();
    let unplaced_scenes: Vec<String> = anchored
        .keys()
        .filter(|scene| !ordered.contains(&mnemosyne_core::SectionId::from(scene.as_str())))
        .cloned()
        .collect();
    // Unordered fact-bearing scenes (Finding 4): a scene carries facts but is
    // not a node of the composed canon order, so no consumer can place it. With
    // no order declared, `nodes()` is empty and every fact-bearing scene lands
    // here — the frontier surfacing the missing order artifact. Now DERIVED from
    // the placement set above rather than recomputing the predicate: this is the
    // renderability projection (facts that can never be placed), which is why it
    // excludes the empty ones — they have nothing to render yet.
    let unordered_scenes: Vec<String> = unplaced_scenes
        .iter()
        .filter(|scene| anchored.get(scene.as_str()).is_some_and(|f| !f.is_empty()))
        .cloned()
        .collect();
    let scene_coverage: Vec<SceneCoverage> = anchored
        .into_iter()
        .map(|(scene, facts)| {
            let structural = structural_at.remove(&scene).unwrap_or_default();
            SceneCoverage {
                scene,
                facts,
                structural,
            }
        })
        .collect();

    // Per-world dangling setups (R442) — keep only worlds with work outstanding.
    let payoff =
        mnemosyne_validate::continuity::payoff_coverage(&store, &order).map_err(OpError::Other)?;
    let dangling_setups = payoff.dangling_by_world();
    let distinct_dangling: BTreeSet<&String> = payoff
        .worlds
        .values()
        .flat_map(|w| w.dangling.iter())
        .collect();

    // Telling-scoped gaps (R568 quests + R507 disclosure) only when asked.
    let (unresolved_quests, never_planned_disclosures, disclosures_seated_before_truth) =
        match telling {
            Some(t) => {
                let quests = mnemosyne_validate::continuity::quest_graph(&store, &order, None, t)
                    .map_err(OpError::Other)?;
                let disclosure = mnemosyne_validate::disclosure::disclosure_coverage(&store, t)
                    .map_err(OpError::Other)?;
                // Round 949 — the seat, ordered against the fact's own road. Both
                // coordinates are authored; an unorderable pair is skipped, never
                // guessed.
                let mut seated_early: Vec<SeatBeforeTruth> = Vec::new();
                if let Some(plan) = store.disclosure_plans.get(t) {
                    for (fact_id, ov) in &plan.overrides {
                        let (Some(surface), Some(fact)) =
                            (ov.surface.as_ref(), store.narrative_facts.get(fact_id))
                        else {
                            continue;
                        };
                        let road = order.linearize(&fact.branch);
                        let at = |s: &str| road.iter().position(|n| n.as_str() == s);
                        if let (Some(seat), Some(truth)) =
                            (at(surface.scene.as_str()), at(fact.canon_from.as_str()))
                        {
                            if seat < truth {
                                seated_early.push(SeatBeforeTruth {
                                    fact_id: fact_id.to_string(),
                                    world: fact.branch.to_string(),
                                    seated_at: surface.scene.to_string(),
                                    true_from: fact.canon_from.to_string(),
                                });
                            }
                        }
                    }
                }
                seated_early.sort();
                (
                    Some(quests.unresolved_quests),
                    Some(disclosure.never_planned),
                    Some(seated_early),
                )
            }
            None => (None, None, None),
        };

    // Per-world-line ownership density (Round 617, denominator corrected Round
    // 619): main + every registered branch, owned facts over the FULL road the
    // world travels. Pure read — never gated, so it does NOT feed total_gaps.
    let mut branch_owned_density: BTreeMap<String, BranchDensity> = BTreeMap::new();
    for world in std::iter::once(mnemosyne_core::BranchId::from(mnemosyne_core::MAIN_BRANCH))
        .chain(store.branches.keys().cloned())
    {
        let road: Vec<String> = order
            .linearize(&world)
            .iter()
            .map(ToString::to_string)
            .collect();
        let owned: Vec<String> = store
            .narrative_facts
            .iter()
            .filter(|(_, f)| f.branch == world)
            .map(|(id, _)| id.to_string())
            .collect();
        let density = (!road.is_empty()).then(|| owned.len() as f64 / road.len() as f64);
        branch_owned_density.insert(
            world.to_string(),
            BranchDensity {
                owned,
                road,
                density,
            },
        );
    }

    let total_gaps = zero_fact_scenes.len()
        + unordered_scenes.len()
        + distinct_dangling.len()
        + unresolved_quests.as_ref().map_or(0, Vec::len)
        + never_planned_disclosures.as_ref().map_or(0, Vec::len)
        + disclosures_seated_before_truth.as_ref().map_or(0, Vec::len)
        + map_frontier.total_gaps;

    Ok(AuthoringFrontierReport {
        telling: telling.map(str::to_string),
        zero_fact_scenes,
        unplaced_scenes,
        unordered_scenes,
        scene_coverage,
        branch_owned_density,
        structural_facts: structural_ids.into_iter().collect(),
        dangling_setups,
        unresolved_quests,
        never_planned_disclosures,
        disclosures_seated_before_truth,
        map_frontier,
        total_gaps,
    })
}

/// The frame-view envelope both wires emit (Round 435). `holding_count`
/// rides beside the full entries so a scanning consumer never counts.
#[derive(Debug, Clone, Serialize)]
pub struct FrameViewReport {
    pub frame: String,
    pub branch: String,
    pub at: String,
    pub entity: Option<String>,
    pub holding: Vec<mnemosyne_validate::continuity::FrameViewEntry>,
    pub holding_count: usize,
    /// NAMED since Round 1054 — see [`mnemosyne_validate::continuity::FrameView`].
    pub not_holding: Vec<String>,
    pub unknown: Vec<String>,
    /// The world-line is a confluence FRAGMENT (Round 746) — carried through from
    /// the projection so the CLI/MCP render can name it, the same signal the
    /// manuscript / playable-world / quest-graph surfaces already carry.
    pub confluence_fragment: bool,
}

/// Run the frame-at-T projection (Round 432) over the workspace store with
/// the shared order resolution. `branch` omitted = the default world-line.
pub fn continuity_frame_view(
    workspace_root: &Path,
    sidecar: Option<&AbsolutePath>,
    frame: &str,
    branch: Option<&str>,
    entity: Option<&str>,
    at: &str,
    order_override: Option<&AbsolutePath>,
) -> Result<FrameViewReport, OpError> {
    let policy = continuity_policy(workspace_root)?;
    let decl = resolve_canon_order_file(&policy, order_override)?;
    let store = load_atomic_store(workspace_root, sidecar)?;
    let order = compose_canon_order(&decl, &store)?;
    let branch = branch.unwrap_or(mnemosyne_core::MAIN_BRANCH);
    // Entry into the store vocabulary: `frame` and `branch` arrive as raw
    // CLI/MCP arguments and become registry ids here, once, for both wires.
    let frame = mnemosyne_core::FrameId::from(frame);
    let branch = mnemosyne_core::BranchId::from(branch);
    let view = mnemosyne_validate::continuity::frame_view(
        &store,
        &order,
        &frame,
        &branch,
        entity,
        &at.into(),
    )
    .map_err(OpError::Other)?;
    Ok(FrameViewReport {
        frame: view.frame,
        branch: view.branch,
        at: view.at,
        entity: view.entity,
        holding_count: view.holding.len(),
        holding: view.holding,
        not_holding: view.not_holding,
        unknown: view.unknown,
        confluence_fragment: view.confluence_fragment,
    })
}

/// Run the setup/payoff coverage classification (Round 442) over the
/// workspace store with the shared order resolution — pure read projection,
/// per query world (main + every registered branch). Dangling setups are a
/// report finding (the author's todo list), deliberately never gated.
pub fn payoff_coverage_report(
    workspace_root: &Path,
    sidecar: Option<&AbsolutePath>,
    order_override: Option<&AbsolutePath>,
) -> Result<mnemosyne_validate::continuity::PayoffCoverageReport, OpError> {
    let (store, order) =
        world_scoped_inputs(workspace_root, sidecar, order_override, "payoff coverage")?;
    mnemosyne_validate::continuity::payoff_coverage(&store, &order).map_err(OpError::Other)
}

/// The typing-discovery input package (Round 458, design sec 7.15 Round
/// A): every untyped fact + the registered vocabulary in one call. Pure
/// read projection; order-independent (typing is a property of the fact,
/// not of any canon declaration), so no order resolution runs.
pub fn typing_candidates_report(
    workspace_root: &Path,
    sidecar: Option<&AbsolutePath>,
) -> Result<mnemosyne_validate::continuity::TypingCandidatesReport, OpError> {
    let store = load_atomic_store(workspace_root, sidecar)?;
    mnemosyne_validate::continuity::typing_candidates(&store).map_err(OpError::Other)
}

/// Import typed legs from a reviewed `typing-proposals/v1` artifact
/// (Round 459, design sec 7.15 Round B) — load + shape-check the file,
/// then run the all-or-nothing import (or its dry-run twin) against the
/// resolved store. Returns the full verdict report; gating policy (exit
/// code / MCP error) stays with the caller. Not routed through
/// [`run_atomic_mutate`] because the outcome is a verdict report, not a
/// bare receipt — the MCP wire still serializes it under the server
/// mutate lock.
pub fn import_typing_proposals_report(
    workspace_root: &Path,
    sidecar: Option<&AbsolutePath>,
    proposals_path: &Path,
    dry_run: bool,
) -> Result<mnemosyne_atomic::TypingImportReport, OpError> {
    let (file, file_sha256) =
        mnemosyne_atomic::load_typing_proposals(proposals_path).map_err(OpError::Other)?;
    let sidecar_path = resolve_sidecar(workspace_root, sidecar)?;
    let mut store =
        AtomicStore::load(&sidecar_path).map_err(|e| OpError::Other(format!("{}", e)))?;
    Ok(mnemosyne_atomic::import_typing_proposals(
        &mut store,
        &sidecar_path,
        &file,
        &file_sha256,
        dry_run,
    )?)
}

/// Deterministic payoff substantiation (Round 485) — classify every credited
/// setup as substantiated / unsubstantiated / unverifiable by the typed
/// state-change rule, per world. Pure read projection, no LLM (the R484
/// all-deterministic redesign that replaced the R481 drift-verdict surface).
pub fn payoff_substantiation_report(
    workspace_root: &Path,
    sidecar: Option<&AbsolutePath>,
    order_override: Option<&AbsolutePath>,
) -> Result<mnemosyne_validate::continuity::PayoffSubstantiationReport, OpError> {
    let policy = continuity_policy(workspace_root)?;
    let decl = resolve_canon_order_file(&policy, order_override)?;
    let store = load_atomic_store(workspace_root, sidecar)?;
    let order = compose_canon_order(&decl, &store)?;
    mnemosyne_validate::continuity::payoff_substantiation(&store, &order).map_err(OpError::Other)
}

/// Timeline-gap projection (Round 490, design sec 7.20 step 2) — the
/// deterministic interval evaluator surfaced as a READ report, per world,
/// never gated. Resolves the same `narrative-rules` artifact as the gate
/// (`continuity_scan`); only `interval` rules contribute.
///
/// `world` scopes to one road (Round 1049) — carried into the projection, not
/// applied to its output by a caller: the CLI used to filter in its PROSE loop
/// alone, so the `--json` wire answered every road under `--world <one>`.
pub fn timeline_gaps_report(
    workspace_root: &Path,
    sidecar: Option<&AbsolutePath>,
    order_override: Option<&AbsolutePath>,
    rules_override: Option<&AbsolutePath>,
    world: Option<&str>,
) -> Result<mnemosyne_validate::continuity::TimelineGapsReport, OpError> {
    let policy = continuity_policy(workspace_root)?;
    let decl = resolve_canon_order_file(&policy, order_override)?;
    let rules = resolve_narrative_rules(&policy, rules_override)?;
    let store = load_atomic_store(workspace_root, sidecar)?;
    let order = compose_canon_order(&decl, &store)?;
    // Entry into the store vocabulary, once, for both wires.
    let world = world.map(mnemosyne_core::BranchId::from);
    mnemosyne_validate::continuity::timeline_gaps(&store, &order, &rules.rules, world.as_ref())
        .map_err(OpError::Other)
}

/// The declared-map read (Round 875) — every transition rule's map with the
/// edge side-table values (`edge_costs` R710, `edge_guards` R722/R723) the
/// store already holds. Resolves the same `narrative-rules` artifact as the
/// gate, for the same reason `timeline_gaps` does: the rule is what DECLARES
/// which predicate names an edge, and core must not know (invariant 4).
///
/// Takes NO canon order — the map has never been canon-ordered (the gate
/// evaluates it flat, R696 review finding #6), so requiring one would be a
/// scoping this read does not perform.
pub fn transition_map_report(
    workspace_root: &Path,
    sidecar: Option<&AbsolutePath>,
    rules_override: Option<&AbsolutePath>,
) -> Result<mnemosyne_validate::continuity::TransitionMapReport, OpError> {
    let policy = continuity_policy(workspace_root)?;
    let rules = resolve_narrative_rules(&policy, rules_override)?;
    let store = load_atomic_store(workspace_root, sidecar)?;
    mnemosyne_validate::continuity::transition_map(&store, &rules.rules).map_err(OpError::Other)
}

/// Import succession + conflict edges from a reviewed `edge-proposals/v1`
/// artifact (Round 463, design sec 7.16 Round B) — load + shape-check the
/// file, then run the all-or-nothing import (or its dry-run twin). Returns
/// the full verdict report; gating policy stays with the caller (the
/// import_typing_proposals_report shape).
pub fn import_edge_proposals_report(
    workspace_root: &Path,
    sidecar: Option<&AbsolutePath>,
    proposals_path: &Path,
    dry_run: bool,
) -> Result<mnemosyne_atomic::EdgeImportReport, OpError> {
    let (file, file_sha256) =
        mnemosyne_atomic::load_edge_proposals(proposals_path).map_err(OpError::Other)?;
    let sidecar_path = resolve_sidecar(workspace_root, sidecar)?;
    let mut store =
        AtomicStore::load(&sidecar_path).map_err(|e| OpError::Other(format!("{}", e)))?;
    Ok(mnemosyne_atomic::import_edge_proposals(
        &mut store,
        &sidecar_path,
        &file,
        &file_sha256,
        dry_run,
    )?)
}

/// The edge-discovery input package (Round 462, design sec 7.16 Round A):
/// every fact row (claim + sha256 pin + all recorded edges) plus the
/// deterministic succession-gap hints, with the shared order resolution
/// (the hints need world visibility; the facts table never degrades).
pub fn edge_candidates_report(
    workspace_root: &Path,
    sidecar: Option<&AbsolutePath>,
    order_override: Option<&AbsolutePath>,
) -> Result<mnemosyne_validate::continuity::EdgeCandidatesReport, OpError> {
    let policy = continuity_policy(workspace_root)?;
    let decl = resolve_canon_order_file(&policy, order_override)?;
    let store = load_atomic_store(workspace_root, sidecar)?;
    let order = compose_canon_order(&decl, &store)?;
    mnemosyne_validate::continuity::edge_candidates(&store, &order).map_err(OpError::Other)
}

/// Run the dramatic-irony intervals derivation (Round 455, design sec
/// 7.14) over the workspace store with the shared order resolution —
/// pure read projection over recorded cross-frame conflict edges, per
/// query world. Craft signal, deliberately never gated.
pub fn irony_intervals_report(
    workspace_root: &Path,
    sidecar: Option<&AbsolutePath>,
    order_override: Option<&AbsolutePath>,
) -> Result<mnemosyne_validate::continuity::IronyIntervalsReport, OpError> {
    let policy = continuity_policy(workspace_root)?;
    let decl = resolve_canon_order_file(&policy, order_override)?;
    let store = load_atomic_store(workspace_root, sidecar)?;
    let order = compose_canon_order(&decl, &store)?;
    mnemosyne_validate::continuity::irony_intervals(&store, &order).map_err(OpError::Other)
}

/// Run the playthrough-manuscript linearization (Round 466, design sec
/// 7.17) over the workspace store with the shared order resolution —
/// pure read projection: per query world (or the single `world` filter),
/// the composed order's deterministic topological walk with declared
/// fact events placed on it. Reading surface, deliberately never gated.
pub fn playthrough_manuscript_report(
    workspace_root: &Path,
    sidecar: Option<&AbsolutePath>,
    world: Option<&str>,
    order_override: Option<&AbsolutePath>,
    telling: Option<&str>,
    reading_walk: bool,
) -> Result<mnemosyne_validate::continuity::PlaythroughManuscriptReport, OpError> {
    let (store, order) = world_scoped_inputs(
        workspace_root,
        sidecar,
        order_override,
        "the playthrough manuscript",
    )?;
    // Entry into the store vocabulary, once, for both wires.
    let world = world.map(mnemosyne_core::BranchId::from);
    let mut report = mnemosyne_validate::continuity::playthrough_manuscript(
        &store,
        &order,
        world.as_ref(),
        telling,
    )
    .map_err(OpError::Other)?;
    // Round 509 — the reading-walk projection: prune each world to its
    // content scenes (those where a world-visible fact begins). The structural
    // manuscript (the verb default) keeps every order node; a READING copy
    // wants only the scenes that introduce content (the R500 begins>0
    // convention). A deterministic, in-code prune replaces the orchestrator's
    // hand-made `.filtered` files (the harness debt R505 flagged), so the next
    // blind run produces per-world reading copies without manual surgery.
    // Round 1048 — recorded where it is APPLIED. The projection never prunes,
    // so `false` is the truth there; this is the one place the answer changes,
    // and a consumer holding two manuscripts of one store has no other way to
    // learn that one of them is missing its contentless scenes.
    report.reading_walk = reading_walk;
    if reading_walk {
        for world in report.worlds.values_mut() {
            world.scenes.retain(|scene| !scene.begins.is_empty());
        }
    }
    Ok(report)
}

/// Project the fork tree (Round 497, design sec 7.21) over the workspace
/// store with the shared order resolution — the cross-world choice graph
/// the CYOA renderer assumes: every registered world-line with its
/// divergence coordinate (parent + fork point + the choice-label
/// description), the fork point resolved against the parent's composed
/// order. Pure read projection, deliberately never gated.
pub fn fork_tree_report(
    workspace_root: &Path,
    sidecar: Option<&AbsolutePath>,
    order_override: Option<&AbsolutePath>,
) -> Result<mnemosyne_validate::continuity::ForkTreeReport, OpError> {
    let policy = continuity_policy(workspace_root)?;
    let decl = resolve_canon_order_file(&policy, order_override)?;
    let store = load_atomic_store(workspace_root, sidecar)?;
    let order = compose_canon_order(&decl, &store)?;
    mnemosyne_validate::continuity::fork_tree(&store, &order).map_err(OpError::Other)
}

/// Project the playable world (Round 556/557, design sec 7.37) over the
/// workspace store with the shared order resolution — the `map_locator` seam a
/// pinion narrative runtime consumes: per telling, the cross-world fork
/// topology + each world-line's scene walk + the per-scene disclosure
/// [`mnemosyne_validate::continuity::MapLocator`]s. A pure JOIN over the
/// existing manuscript (R466) and fork-tree (R497) projections; pure read,
/// never gated. `world` filters the per-world map (the fork tree stays full).
pub fn playable_world_report(
    workspace_root: &Path,
    sidecar: Option<&AbsolutePath>,
    world: Option<&str>,
    order_override: Option<&AbsolutePath>,
    telling: &str,
) -> Result<mnemosyne_validate::continuity::PlayableWorldReport, OpError> {
    let (store, order) = world_scoped_inputs(
        workspace_root,
        sidecar,
        order_override,
        "the playable world",
    )?;
    // Entry into the store vocabulary, once, for both wires.
    let world = world.map(mnemosyne_core::BranchId::from);
    mnemosyne_validate::continuity::playable_world(&store, &order, world.as_ref(), telling)
        .map_err(OpError::Other)
}

/// Project the quest graph (Round 559 design sec 7.38, Round 568 build) over the
/// workspace store with the shared order resolution — the fact→quest leg a
/// pinion narrative runtime (or an authoring consumer) consumes: per telling,
/// each derived quest (a pursues object / requires endpoint / completed_by subject) projected to a `QuestNode` (objective, actor,
/// per-world derived open/done state, prerequisites, completion fact, giver
/// surface locator). A pure JOIN over the existing payoff-coverage (R442) and
/// playable-world (R557) projections; pure read, never gated. `world` filters
/// the per-world map (the fork tree stays full).
pub fn quest_graph_report(
    workspace_root: &Path,
    sidecar: Option<&AbsolutePath>,
    world: Option<&str>,
    order_override: Option<&AbsolutePath>,
    telling: &str,
) -> Result<mnemosyne_validate::continuity::QuestGraphReport, OpError> {
    let (store, order) =
        world_scoped_inputs(workspace_root, sidecar, order_override, "the quest graph")?;
    // Entry into the store vocabulary, once, for both wires.
    let world = world.map(mnemosyne_core::BranchId::from);
    mnemosyne_validate::continuity::quest_graph(&store, &order, world.as_ref(), telling)
        .map_err(OpError::Other)
}

/// The medium-neutral authoring contract (Round 587, R585 debt item 1) — the
/// `describe-schema` surface an external generate-gate-repair agent reads to
/// self-serve the registries / fact shape / fixed vocabularies / rule classes /
/// quest encoding / write-time invariants instead of reading source. A PURE
/// static projection: store-independent (the contract is fixed; store CONTENTS
/// are `query`/`list-*`), no I/O, cannot fail.
pub fn describe_schema() -> mnemosyne_validate::schema::SchemaContract {
    mnemosyne_validate::schema::describe_schema()
}

/// Disclosure coverage (Round 507, design sec 7.24 step 4) — the per-telling
/// classification surface (disclosed / hidden-by-design / never-planned). Pure
/// read projection, order-independent, never gated.
pub fn disclosure_coverage_report(
    workspace_root: &Path,
    sidecar: Option<&AbsolutePath>,
    telling: &str,
) -> Result<mnemosyne_validate::disclosure::DisclosureCoverageReport, OpError> {
    let store = load_atomic_store(workspace_root, sidecar)?;
    mnemosyne_validate::disclosure::disclosure_coverage(&store, telling).map_err(OpError::Other)
}

/// Premature-leak gate (Round 507, design sec 7.24 step 5, R502) — the authored
/// plan vs a BLIND RE-EXTRACTED prose store (`against`), matched by typed tuple
/// in `truth_frame` for `world`. Guards `world` against the branch registry and
/// `truth_frame` against the frame registry before running.
pub fn disclosure_leak_report(
    workspace_root: &Path,
    sidecar: Option<&AbsolutePath>,
    against: &AbsolutePath,
    order_override: Option<&AbsolutePath>,
    telling: &str,
    world: &str,
    truth_frame: &str,
) -> Result<mnemosyne_validate::disclosure::DisclosureLeakReport, OpError> {
    let policy = continuity_policy(workspace_root)?;
    let decl = resolve_canon_order_file(&policy, order_override)?;
    let authored = load_atomic_store(workspace_root, sidecar)?;
    let order = compose_canon_order(&decl, &authored)?;
    // Entry into the store vocabulary, once, for both wires.
    let world = &mnemosyne_core::BranchId::from(world);
    if !mnemosyne_core::is_known_world(&authored.branches, world) {
        return Err(OpError::Other(format!(
            "world `{world}` not present in the branch registry (fail-loud)"
        )));
    }
    // Entry into the store vocabulary: `truth_frame` arrives as a raw CLI/MCP
    // argument and becomes a registry id here, once, for both wires — before
    // the registry check, so the check reads the same typed id the gate does.
    let truth_frame = mnemosyne_core::FrameId::from(truth_frame);
    if !authored.frames.contains_key(&truth_frame) {
        return Err(OpError::Other(format!(
            "truth_frame `{truth_frame}` not present in the frame registry (fail-loud)"
        )));
    }
    let reextracted = load_named_store(against, "the re-extracted prose store `against`")?;
    mnemosyne_validate::disclosure::disclosure_leak(
        &authored,
        &reextracted,
        &order,
        telling,
        world,
        &truth_frame,
    )
    .map_err(OpError::Other)
}

/// Render↔world-line fidelity gate (Round 507, design sec 7.24 step 6, R505) —
/// the BLIND RE-EXTRACTED prose store (`against`) checked against `world`'s
/// composed order (the prose analog of R488). Guards `world` against the branch
/// registry before running.
pub fn render_fidelity_report(
    workspace_root: &Path,
    sidecar: Option<&AbsolutePath>,
    against: &AbsolutePath,
    order_override: Option<&AbsolutePath>,
    world: &str,
) -> Result<mnemosyne_validate::disclosure::RenderFidelityReport, OpError> {
    let policy = continuity_policy(workspace_root)?;
    let decl = resolve_canon_order_file(&policy, order_override)?;
    let authored = load_atomic_store(workspace_root, sidecar)?;
    let order = compose_canon_order(&decl, &authored)?;
    // Entry into the store vocabulary, once, for both wires.
    let world = &mnemosyne_core::BranchId::from(world);
    if !mnemosyne_core::is_known_world(&authored.branches, world) {
        return Err(OpError::Other(format!(
            "world `{world}` not present in the branch registry (fail-loud)"
        )));
    }
    let reextracted = load_named_store(against, "the re-extracted prose store `against`")?;
    Ok(mnemosyne_validate::disclosure::render_fidelity(
        &reextracted,
        &order,
        world,
    ))
}

/// What one single-world projection did (Round 1070) — the counts a caller
/// needs to see that the file it just wrote is about something.
#[derive(Debug, Clone, Serialize)]
pub struct WorldProjectionReport {
    pub world: String,
    /// Where the projected store was written.
    pub out: String,
    /// Narrative facts the projection KEPT — the count
    /// `validate-render-fidelity` will report as `reextracted_facts`.
    pub kept: usize,
    /// Narrative facts dropped as belonging to another world-line.
    pub dropped: usize,
}

/// Emit the single-world projection of a store — the input shape
/// `validate-render-fidelity` requires (Round 1070).
///
/// `subject` is the store being projected, defaulting to the workspace's own
/// sidecar; the BRANCH REGISTRY always comes from the authored workspace store,
/// because a re-extracted prose store is prose and need not carry the world
/// lattice at all. `world` is guarded against that registry exactly as the
/// fidelity gate guards it, so a typo is refused here rather than yielding an
/// empty projection that reads as a clean render downstream.
///
/// # Errors
///
/// [`OpError`] if a store cannot be read, the world is not registered, the
/// branch lineage is cyclic, or the output cannot be written.
pub fn project_world_store(
    workspace_root: &Path,
    sidecar: Option<&AbsolutePath>,
    subject: Option<&AbsolutePath>,
    world: &str,
    out: &AbsolutePath,
) -> Result<WorldProjectionReport, OpError> {
    let authored = load_atomic_store(workspace_root, sidecar)?;
    let world = &mnemosyne_core::BranchId::from(world);
    if !mnemosyne_core::is_known_world(&authored.branches, world) {
        return Err(OpError::Other(format!(
            "world `{world}` not present in the branch registry (fail-loud)"
        )));
    }
    let subject = match subject {
        Some(named) => load_named_store(named, "the store to project")?,
        None => authored.clone(),
    };
    let before = subject.narrative_facts.len();
    let projected =
        mnemosyne_validate::disclosure::project_world(&subject, &authored.branches, world)
            .map_err(OpError::Other)?;
    let kept = projected.narrative_facts.len();
    projected
        .save(out.as_path())
        .map_err(|e| OpError::Other(format!("{e}")))?;
    Ok(WorldProjectionReport {
        world: world.to_string(),
        out: out.as_path().display().to_string(),
        kept,
        dropped: before - kept,
    })
}

/// One fact row in an entity dossier (Round 437) — raw authoring-time view
/// (no holds evaluation; the frame-at-T projection is `continuity_frame_view`
/// with the entity filter).
#[derive(Debug, Clone, Serialize)]
pub struct EntityFactRow {
    pub fact_id: String,
    pub frame: String,
    pub branch: String,
    pub claim: String,
    pub canon_from: String,
    pub canon_to: Option<String>,
    pub evidence: Vec<String>,
    /// Typed leg (Round 446), surfaced verbatim when authored.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub typed: Option<mnemosyne_core::TypedClaim>,
    /// Verbatim source quote (R736 content parity with FrameViewEntry /
    /// ManuscriptFactEvent) — the dossier is the raw fact list, so it echoes
    /// the same stored content, not a reduced-fidelity subset.
    pub quote: Option<String>,
    /// Multiset count (R731 `fact_counts`) riding this fact — asserted content,
    /// echoed verbatim when authored, never summed (the R712 layering line).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<i64>,
}

/// R679 — one unregistered entity kind and the entities that name it, the unit
/// of the migration worklist a pre-registry (v23-) or out-of-band store needs.
#[derive(Debug, Clone, Serialize)]
pub struct EntityKindMigrationRow {
    pub kind: String,
    pub entities: Vec<String>,
}

/// R679 — the entity-kind migration worklist: the distinct unregistered KINDS a
/// store uses, each with the entities using it, so an adopter knows the exact
/// `add-entity-kind` calls to make. The complete list of the KIND facet, which
/// the validate-workspace failure only samples (R681: the gate covers more than
/// kinds — frame/branch/entity/canon/evidence/typed refs — so this report is the
/// kind worklist, not the whole gate's). Reuses the shared
/// [`mnemosyne_atomic::unregistered_entity_kinds`] detector, so the report and
/// the gate's kind facet cannot disagree.
#[derive(Debug, Clone, Serialize)]
pub struct EntityKindMigration {
    pub unregistered_kinds: Vec<EntityKindMigrationRow>,
    /// Entities naming an UNREGISTERED kind — the size of the worklist, not of
    /// the store. Round 896 renamed it from `total_entities`, a name this
    /// project's own CLI misread: the empty-worklist branch printed it as a
    /// denominator, where it is 0 by construction.
    pub entities_naming_an_unregistered_kind: usize,
    /// Every entity the store registers — the denominator (Round 896). Without
    /// it, "0 unregistered kinds" reads the same on a store whose kinds are all
    /// declared and on a store holding no entities at all.
    pub entities_examined: usize,
}

pub fn entity_kind_migration(
    workspace_root: &Path,
    sidecar: Option<&AbsolutePath>,
) -> Result<EntityKindMigration, OpError> {
    let store = load_atomic_store(workspace_root, sidecar)?;
    let mut by_kind: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (id, kind) in mnemosyne_atomic::unregistered_entity_kinds(&store) {
        by_kind.entry(kind).or_default().push(id);
    }
    let entities_naming_an_unregistered_kind = by_kind.values().map(Vec::len).sum();
    let entities_examined = store.entities.len();
    let unregistered_kinds = by_kind
        .into_iter()
        .map(|(kind, entities)| EntityKindMigrationRow { kind, entities })
        .collect();
    Ok(EntityKindMigration {
        unregistered_kinds,
        entities_naming_an_unregistered_kind,
        entities_examined,
    })
}

/// Round 730 (DEBT-K) — one numeric-threshold gate on a choice, in the neutral
/// economy read. `op` is the operator SYMBOL (`>=` etc.), the interval rule's
/// reporting symbol (shared since the R730 `IntervalOp` lift).
#[derive(Debug, Clone, Serialize)]
pub struct ParameterEconomyGateRow {
    /// The choice fact the gate rides.
    pub fact: String,
    /// The comparison symbol (`>=`, `<=`, `==`, `>`, `<`).
    pub op: String,
    /// The required accumulated value.
    pub threshold: i64,
}

/// Round 940 — one authored delta on one meter: the beat that moves it and by
/// how much. Verbatim authored data, the DELTA-axis peer of
/// [`ParameterEconomyGateRow`].
///
/// This row is what Round 941 added and Round 730 did not have. The read carried
/// the gates per-fact from the start and the deltas only as a count and a Σ, so
/// the datum a playing runtime must actually apply — which fact moves which
/// meter by how much — was summed away before it reached anyone, and the first
/// consumer's build re-derived it by parsing our sidecar. Two stores with
/// different beats and the same totals produced BYTE-IDENTICAL reports, which is
/// the property the discriminating-pair test now forbids.
#[derive(Debug, Clone, Serialize)]
pub struct ParameterEconomyDeltaRow {
    /// The beat (fact) carrying the delta.
    pub fact: String,
    /// The authored change, signed. Never accumulated here — the running sum is
    /// the consumer's playthrough job (the R712 layering line).
    pub delta: i64,
}

/// Round 730 (DEBT-K) — one meter's economy row: its declared description, the
/// beats that move it and by how much, the apply-once Σ of positive and of
/// negative deltas, and the gates that reference it. A NEUTRAL aggregate: `sum_positive` /
/// `sum_negative` are DESCRIPTIVE Σ over the authored deltas the consumer
/// interprets with its OWN accumulation model (grinding, one-shot, clamped) —
/// NOT a reachability verdict (the R728 review killed `ParameterGateUnreachable`:
/// Σ is an upper bound only under an apply-once/unclamped model, which is the
/// consumer's dynamic playthrough evaluation, not Mnemosyne's — the R712 layering
/// line).
#[derive(Debug, Clone, Serialize)]
pub struct ParameterEconomyRow {
    pub parameter: String,
    pub description: String,
    /// Every authored delta on this meter, fact-id ordered (Round 941). THE
    /// row-level datum; the three aggregates below are projections of it kept in
    /// the same struct for an author reading at a glance, the way
    /// [`EntityDossier::fact_count`] sits beside its own rows.
    pub deltas: Vec<ParameterEconomyDeltaRow>,
    /// How many beats carry a delta on this meter (= `deltas.len()`).
    pub delta_count: usize,
    /// Σ of the POSITIVE deltas (an apply-once max reach — descriptive only).
    pub sum_positive: i64,
    /// Σ of the NEGATIVE deltas (an apply-once min reach — descriptive only).
    pub sum_negative: i64,
    /// The gates that threshold this meter.
    pub gates: Vec<ParameterEconomyGateRow>,
}

/// Round 730 (DEBT-K) — the VISIBLE accumulation read (gap 3): per REGISTERED
/// meter, the delta inventory and the gates. A pure read projection over the
/// `parameters` / `parameter_deltas` / `parameter_gates` side-tables — NO order,
/// NO world, NO verdict (a NEUTRAL Σ the consumer interprets with its own model).
/// Deltas / gates naming an UNregistered parameter are out-of-band and belong to
/// the validate detectors (`parameter_delta_violations` /
/// `parameter_gate_violations`), not this read — the report is registered-scoped.
#[derive(Debug, Clone, Serialize)]
pub struct ParameterEconomyReport {
    pub meters: Vec<ParameterEconomyRow>,
}

/// WHY THE STORE CHANGED (R1024) — the ledger's read side.
///
/// A reason recorded and unreadable would be the gap this ledger was built to
/// close, moved rather than closed: the argument for keeping it in the store at
/// all was that an agent resuming through `mnemosyne-cli query` cannot read git
/// history, so the store has to answer "why is this what it is now". `target`
/// filters to one record, INCLUDING one that no longer exists — five of the ten
/// primitives that write a row are removals, and the removed thing is exactly
/// what a reader has no other way to ask about.
pub fn mutation_reason_report(
    workspace_root: &Path,
    sidecar: Option<&AbsolutePath>,
    target: Option<&str>,
) -> Result<MutationReasonReport, OpError> {
    let store = load_atomic_store(workspace_root, sidecar)?;
    let wanted = target.map(str::trim).filter(|t| !t.is_empty());
    let rows: Vec<mnemosyne_atomic::MutationReason> = store
        .mutation_reasons
        .iter()
        .filter(|r| wanted.is_none_or(|t| r.target_id == t))
        .cloned()
        .collect();
    Ok(MutationReasonReport {
        target: wanted.map(str::to_string),
        total: store.mutation_reasons.len(),
        rows,
    })
}

/// The reasoned-mutation ledger as read. `total` is the WHOLE ledger even when
/// `rows` is filtered, so a caller can never read a narrow answer as the whole
/// of what the store holds (the Round 854 rule these gates keep re-learning).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MutationReasonReport {
    pub target: Option<String>,
    pub total: usize,
    pub rows: Vec<mnemosyne_atomic::MutationReason>,
}

pub fn parameter_economy_report(
    workspace_root: &Path,
    sidecar: Option<&AbsolutePath>,
) -> Result<ParameterEconomyReport, OpError> {
    let store = load_atomic_store(workspace_root, sidecar)?;
    let meters = store
        .parameters
        .iter()
        .map(|(param, decl)| {
            let mut deltas = Vec::new();
            let mut sum_positive = 0i64;
            let mut sum_negative = 0i64;
            // `parameter_deltas` is keyed by fact id, so the rows come out
            // fact-ordered without a sort here.
            for (fact, per_meter) in &store.parameter_deltas {
                if let Some(d) = per_meter.get(param) {
                    deltas.push(ParameterEconomyDeltaRow {
                        fact: fact.to_string(),
                        delta: *d,
                    });
                    if *d > 0 {
                        sum_positive += *d;
                    } else {
                        sum_negative += *d;
                    }
                }
            }
            let delta_count = deltas.len();
            let gates = store
                .parameter_gates
                .iter()
                .filter(|(_, g)| &g.parameter == param)
                .map(|(fact, g)| ParameterEconomyGateRow {
                    fact: fact.to_string(),
                    op: g.op.symbol().to_string(),
                    threshold: g.threshold,
                })
                .collect();
            ParameterEconomyRow {
                // Report rows stay `String` — an id becomes text at the OUTPUT
                // boundary, the mirror of the input conversion.
                parameter: param.to_string(),
                description: decl.description.clone(),
                deltas,
                delta_count,
                sum_positive,
                sum_negative,
                gates,
            }
        })
        .collect();
    Ok(ParameterEconomyReport { meters })
}

/// One binding that inherited `kind = implements` from a pre-v5 store, pending
/// Stage-B reclassification (implements vs references). `defaulted_kind` is the
/// typed `BindingKind` (Round 694 — DEBT-MIGRATION-PROJECTION; it was needlessly
/// downgraded to `String`). It serialises `rename_all = "lowercase"`, so the CLI
/// json is byte-identical to the prior hand-stringified form.
#[derive(Debug, Clone, Serialize)]
pub struct BindingKindMigrationRow {
    pub section_id: String,
    pub file: String,
    pub symbol: Option<String>,
    pub defaulted_kind: mnemosyne_core::BindingKind,
}

/// The v4→v5 binding-kind migration worklist — the shared shape the CLI
/// (`report-binding-migration`) and the MCP tool both render, so the two
/// surfaces cannot drift on what the report contains (the R679 pattern applied
/// to the sibling report DEBT-BINDING-MIGRATION-MCP named). `from_schema_version`
/// is `None` when the store is already at the current schema — no migration
/// pending, `rows` empty.
#[derive(Debug, Clone, Serialize)]
pub struct BindingKindMigration {
    pub from_schema_version: Option<u32>,
    pub rows: Vec<BindingKindMigrationRow>,
}

/// The v4→v5 binding-kind migration worklist (Round 686 — the shared path
/// behind CLI `report-binding-migration` and the MCP tool of the same name).
/// Loads the store and normalises [`mnemosyne_atomic::AtomicStore::kind_migration_report`]
/// — whose `KindMigrationReport` is not `Serialize` and whose `None` (already
/// current schema) both surfaces must render identically — into the one
/// serializable [`BindingKindMigration`].
pub fn binding_kind_migration(
    workspace_root: &Path,
    sidecar: Option<&AbsolutePath>,
) -> Result<BindingKindMigration, OpError> {
    let store = load_atomic_store(workspace_root, sidecar)?;
    Ok(match store.kind_migration_report() {
        None => BindingKindMigration {
            from_schema_version: None,
            rows: Vec::new(),
        },
        Some(report) => BindingKindMigration {
            from_schema_version: Some(report.from_schema_version),
            rows: report
                .rows
                .into_iter()
                .map(|r| BindingKindMigrationRow {
                    section_id: r.section_id,
                    file: r.file,
                    symbol: r.symbol,
                    defaulted_kind: r.defaulted_kind,
                })
                .collect(),
        },
    })
}

/// "All facts about X" (Round 437, design sec 7.10 gap 4) — every fact
/// referencing the entity, across all frames and branches, with the
/// registry row. Fail-loud on an unregistered entity.
#[derive(Debug, Clone, Serialize)]
pub struct EntityDossier {
    pub entity_id: String,
    pub kind: String,
    pub description: String,
    pub fact_count: usize,
    pub facts: Vec<EntityFactRow>,
}

pub fn entity_dossier(
    workspace_root: &Path,
    sidecar: Option<&AbsolutePath>,
    entity_id: &str,
) -> Result<EntityDossier, OpError> {
    let store = load_atomic_store(workspace_root, sidecar)?;
    // Entry into the store vocabulary: a raw CLI/MCP argument becomes a registry
    // id here, once, for both wires.
    let id = mnemosyne_core::EntityId::from(entity_id.trim());
    let Some(entity) = store.entities.get(&id) else {
        return Err(OpError::Other(format!(
            "entity `{id}` not present in the entity registry (fail-loud — a typo'd \
             entity must not read as an empty dossier)"
        )));
    };
    let facts: Vec<EntityFactRow> = store
        .narrative_facts
        .iter()
        .filter(|(_, f)| f.entities.contains(&id))
        .map(|(fid, f)| EntityFactRow {
            fact_id: fid.to_string(),
            frame: f.frame.to_string(),
            branch: f.branch.to_string(),
            claim: f.claim.clone(),
            canon_from: f.canon_from.to_string(),
            canon_to: f.canon_to.as_ref().map(ToString::to_string),
            evidence: f.evidence.iter().map(|e| e.section.to_string()).collect(),
            typed: f.typed.clone(),
            quote: f.quote.clone(),
            count: store.fact_counts.get(fid).copied(),
        })
        .collect();
    Ok(EntityDossier {
        entity_id: id.to_string(),
        kind: entity.kind.to_string(),
        description: entity.description.clone(),
        fact_count: facts.len(),
        facts,
    })
}

/// Run the convenience-form redact_term primitive (R297). Mirrors
/// `mnemosyne-cli redact-term` semantics but returns the structured
/// report instead of printing it.
pub fn redact_term(
    workspace_root: &Path,
    sidecar: Option<&AbsolutePath>,
    regenerate: bool,
    input: &RedactTermInput,
) -> Result<(mnemosyne_atomic::RedactionReport, bool), OpError> {
    use mnemosyne_atomic::{RedactMode, RedactRequest, RedactScope};
    let mode = if input.regex {
        RedactMode::Regex
    } else {
        RedactMode::Literal
    };
    let scope = match input.scope.as_deref().unwrap_or("all") {
        "all" => RedactScope::All,
        "decision_summary" | "publishable_decision_summary" => RedactScope::DecisionSummary,
        "changes_bullets" | "publishable_changes_bullets" => RedactScope::ChangesBullets,
        "verification_bullets" | "publishable_verification_bullets" => {
            RedactScope::VerificationBullets
        }
        "impact_refs" | "publishable_impact_refs" => RedactScope::ImpactRefs,
        "carry_forward_bullets" | "publishable_carry_forward_bullets" => {
            RedactScope::CarryForwardBullets
        }
        other => {
            return Err(OpError::Other(format!(
                "unknown scope `{}` — expected: all | decision_summary | changes_bullets \
                 | verification_bullets | impact_refs | carry_forward_bullets",
                other
            )));
        }
    };
    let req = RedactRequest {
        pattern: input.pattern.clone(),
        replacement: input.replacement.clone(),
        mode,
        case_insensitive: input.case_insensitive,
        scope,
        dry_run: input.dry_run,
        reason: input.reason.clone(),
        applied_in: input.applied_in.clone(),
        kind: input
            .kind
            .clone()
            .unwrap_or_else(|| "redaction".to_string()),
    };
    let sidecar_path = resolve_sidecar(workspace_root, sidecar)?;
    let mut store =
        AtomicStore::load(&sidecar_path).map_err(|e| OpError::Other(format!("{}", e)))?;
    let report = mnemosyne_atomic::redact_term(&mut store, &sidecar_path, &req)?;
    // Inert (no GENERATED.md to regenerate); flag removed in the cleanup round.
    let _ = regenerate;
    Ok((report, false))
}

/// Scan code citations for now-stale references to `inventory_id` —
/// mirrors the CLI's `print_inventory_decay_trigger` cascade (R276) but
/// returns the hits instead of printing to stderr. Empty when the
/// workspace has no `[plugins.set_equality_validator]` inventory config.
pub fn inventory_decay_scan(
    workspace_root: &Path,
    inventory_id: &str,
) -> anyhow::Result<Vec<mnemosyne_validate::code_refs::Citation>> {
    // A malformed mnemosyne.toml fails loud (matches the R362 resolver
    // fail-fast); Ok(None) = no config file = nothing to scan.
    let Some(loaded) = mnemosyne_config::discover_config(workspace_root)? else {
        return Ok(Vec::new());
    };
    let Some(cfg) = loaded
        .config
        .plugins
        .as_ref()
        .and_then(|p| p.set_equality_validator.as_ref())
    else {
        return Ok(Vec::new());
    };
    if cfg.paths.is_empty()
        || (cfg.inventory_prefixes.is_empty() && cfg.inventory_path_prefixes.is_empty())
    {
        return Ok(Vec::new());
    }
    // An unreadable scan path fails loud rather than reporting "no decay" —
    // the `scan_section_decay` sibling the R360 fail-loud sweep missed.
    let hits = mnemosyne_validate::code_refs::scan_inventory_decay(
        workspace_root,
        &cfg.paths,
        inventory_id,
        &cfg.inventory_prefixes,
        &cfg.inventory_path_prefixes,
        cfg.comment_only,
    )?;
    Ok(hits)
}

/// Emit a `[[publishable_override_ledger]]` draft for an entry whose
/// publishable half currently diverges from the audit half (R300).
pub fn emit_publishable_override_ledger_draft(
    workspace_root: &Path,
    sidecar: Option<&AbsolutePath>,
    entry_id: &str,
    reason: &str,
    applied_in: &str,
    kind: Option<&str>,
) -> Result<Option<String>, OpError> {
    // The SHARED read loader, so a named sidecar that is not there is refused
    // here too. This verb was the fourth site of that defect and was found by
    // counting every consumer of the loader rather than by reading the one the
    // repair started from: it is read-only (`&store`), but it resolved and
    // loaded for itself, which is how it kept the permissive rule after the
    // read path had stopped having one.
    let store = load_atomic_store(workspace_root, sidecar)?;
    let draft = mnemosyne_atomic::emit_publishable_override_ledger_draft(
        &store,
        entry_id,
        reason,
        applied_in,
        kind.unwrap_or("redaction"),
    )?;
    Ok(draft)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// A fresh workspace with no sidecar file loads as an empty store — a
    /// missing sidecar is a legitimate state, not an error.
    #[test]
    fn load_atomic_store_missing_sidecar_is_empty_not_error() {
        let tmp = TempDir::new().unwrap();
        let store =
            load_atomic_store(tmp.path(), None).expect("missing sidecar must load as empty");
        assert!(store.atomic_section_id_set().is_empty());
    }

    /// A NAMED SIDECAR THAT IS NOT THERE IS A TYPO, NOT AN EMPTY WORLD.
    ///
    /// The sibling of Round 1014's `against`, and the same mechanism: a store
    /// that cannot be found loads as an empty one, so a read verb pointed at
    /// `sidde.json` answers about a world with nothing in it and exits 0. It
    /// was measured on the real binary before it was repaired —
    /// `report-entity-kind-migration --sidecar side.json` says "0 unregistered
    /// kinds over 1 entity", and the same command one character wrong says "0
    /// unregistered kinds over 0 entities, the store registers no entities, so
    /// nothing was checked". Both are exit 0 and the second reads like a clean
    /// bill.
    ///
    /// THE REPAIR SPLITS BY DIRECTION RATHER THAN BEING BLANKET, because the
    /// other half was measured too: `add-entity-kind --sidecar side.json`
    /// CREATES the file, which is a real workflow and must keep working. Writes
    /// go through `run_atomic_mutate`, which loads for itself; this is the read
    /// path, and only when the caller NAMED the file. The default sidecar keeps
    /// the permissive rule directly below, because there absence is bootstrap.
    #[test]
    fn a_named_sidecar_that_is_absent_is_refused_rather_than_read_as_empty() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::write(root.join("mnemosyne.toml"), "[workspace]\n").unwrap();

        // NON-VACUITY: the same call, on a named sidecar that IS there, reads
        // what it holds — so the refusal below is about the file's absence and
        // not about naming one at all.
        let present = root.join("side.json");
        std::fs::write(
            &present,
            r#"{"schema_version":45,"sections":{},"changelog_entries":{},
                "entity_kinds":{"place":{}},"entities":{"p-a":{"kind":"place"}}}"#,
        )
        .unwrap();
        let named = AbsolutePath::new(present).unwrap();
        let store = load_atomic_store(root, Some(&named)).expect("a named sidecar that is there");
        assert_eq!(
            store.entities.len(),
            1,
            "the named sidecar loaded, but not the world it holds"
        );

        let absent = AbsolutePath::new(root.join("sidde.json")).unwrap();
        assert!(
            load_atomic_store(root, Some(&absent)).is_err(),
            "a named sidecar that is not there was read as an EMPTY STORE, so a \
             one-character typo turns a populated world into a clean bill and \
             every report over it answers 0 with exit 0"
        );

        // The default sidecar keeps the opposite rule, asserted here as well as
        // below: the two policies live one line apart and must not drift.
        load_atomic_store(root, None).expect("an absent DEFAULT sidecar is the bootstrap state");
    }

    /// THE READ VERB THAT RESOLVED FOR ITSELF GETS THE RULE TOO.
    ///
    /// `emit_publishable_override_ledger_draft` is read-only but did not go
    /// through `load_atomic_store`: it called `resolve_sidecar` and the loader
    /// itself, which is how it kept the permissive rule after the read path had
    /// stopped having one. It was found by counting every consumer of the
    /// loader, not by reading the verb the repair started from.
    ///
    /// THIS ARM EXISTS BECAUSE THE REPAIR SURVIVED WITHOUT IT. Reverting that
    /// one line left the whole workspace suite green at 1609 passing, so the
    /// fix was real and unguarded — the shape where a later edit undoes a
    /// repair and nothing says so.
    ///
    /// The two arms are separated by WHICH failure they get rather than by
    /// pass/fail: with the store present the verb gets as far as looking for
    /// the entry, and with it absent it never reads a store at all. Asserting
    /// only "the absent one errors" would pass on a verb that always errors.
    #[test]
    fn a_read_verb_that_resolves_for_itself_also_refuses_an_absent_named_sidecar() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::write(root.join("mnemosyne.toml"), "[workspace]\n").unwrap();
        let present = root.join("side.json");
        std::fs::write(
            &present,
            r#"{"schema_version":45,"sections":{},"changelog_entries":{}}"#,
        )
        .unwrap();

        let named = AbsolutePath::new(present).unwrap();
        let with_store = emit_publishable_override_ledger_draft(
            root,
            Some(&named),
            "Round 1",
            "why",
            "Round 2",
            None,
        );
        let with_store = format!("{with_store:?}");
        assert!(
            !with_store.contains("does not exist"),
            "the present-sidecar arm reported a missing file, so the two arms \
             below are not separated by the file at all: {with_store}"
        );

        let absent = AbsolutePath::new(root.join("sidde.json")).unwrap();
        let err = emit_publishable_override_ledger_draft(
            root,
            Some(&absent),
            "Round 1",
            "why",
            "Round 2",
            None,
        )
        .expect_err("a named sidecar that is not there must be refused");
        assert!(
            format!("{err}").contains("does not exist"),
            "the verb answered about a store it never opened: {err}"
        );
    }

    /// A corrupt sidecar must propagate the error, not silently read as an
    /// empty store. Regression for the `unwrap_or_default` that previously
    /// masked corruption (R356).
    #[test]
    fn load_atomic_store_corrupt_sidecar_propagates_error() {
        let tmp = TempDir::new().unwrap();
        let sidecar = AtomicStore::default_sidecar_path(tmp.path());
        std::fs::create_dir_all(sidecar.parent().unwrap()).unwrap();
        std::fs::write(&sidecar, b"{ this is not valid json").unwrap();
        assert!(
            load_atomic_store(tmp.path(), None).is_err(),
            "corrupt sidecar must fail loud, not silently empty"
        );
    }

    /// Round 941 — THE DISCRIMINATING PAIR. Two stores that move the same meter
    /// by the same totals through DIFFERENT beats must not read the same.
    ///
    /// This is the defect stated as a demonstration rather than as an opinion.
    /// Before the per-beat row existed, these two stores produced byte-identical
    /// reports: `delta_count` 2, Σ+ 3, Σ- 0 for both. A consumer reading the
    /// report could not tell which beat moves the meter and by how much, which is
    /// precisely what a playing runtime must apply — so the first consumer's
    /// build parsed our sidecar instead, and Round 939 found it doing so.
    ///
    /// The assertion is on the DIFFERENCE, not on either store alone: an
    /// implementation that carried a per-beat row but filled it from the wrong
    /// side would still pass a single-store check.
    #[test]
    fn two_stores_with_the_same_totals_and_different_beats_do_not_read_alike() {
        fn economy(deltas: &str) -> ParameterEconomyReport {
            let tmp = TempDir::new().unwrap();
            let root = tmp.path();
            std::fs::write(
                root.join("mnemosyne.toml"),
                "[workspace]\nroot = \".\"\n\n[atomic]\nsidecar_path = \"store.json\"\n",
            )
            .unwrap();
            std::fs::write(
                root.join("store.json"),
                format!(
                    r#"{{"schema_version":43,"sections":{{}},"frames":{{}},"entities":{{}},
                        "narrative_facts":{{}},
                        "parameters":{{"affection":{{"description":"how warmly she reads him"}}}},
                        "parameter_deltas":{deltas}}}"#
                ),
            )
            .unwrap();
            parameter_economy_report(root, None).expect("the economy reads")
        }

        let gift_then_letter = economy(
            r#"{"f-gift":{"affection":2},"f-letter":{"affection":1},"f-slight":{"affection":-1}}"#,
        );
        let letter_then_gift = economy(
            r#"{"f-gift":{"affection":1},"f-letter":{"affection":2},"f-slight":{"affection":-1}}"#,
        );

        // The totals are identical, which is what made the old read blind.
        for report in [&gift_then_letter, &letter_then_gift] {
            let m = &report.meters[0];
            assert_eq!(m.delta_count, 3);
            assert_eq!(m.sum_positive, 3);
            assert_eq!(m.sum_negative, -1);
        }

        // The rows are not.
        let rows = |r: &ParameterEconomyReport| -> Vec<(String, i64)> {
            r.meters[0]
                .deltas
                .iter()
                .map(|d| (d.fact.clone(), d.delta))
                .collect()
        };
        assert_eq!(
            rows(&gift_then_letter),
            vec![
                ("f-gift".to_string(), 2),
                ("f-letter".to_string(), 1),
                // The negative beat is here so the SIGN is pinned at this level
                // too: an injection that returned `d.abs()` was caught only by
                // the end-to-end wire test until this row existed.
                ("f-slight".to_string(), -1)
            ],
            "the beat that carries the delta, and its sign, survive the read"
        );
        assert_ne!(
            rows(&gift_then_letter),
            rows(&letter_then_gift),
            "two different worlds must not project to one report"
        );

        // And the aggregates stay honest projections of the rows they now sit
        // beside, rather than a second source for the same datum.
        let m = &gift_then_letter.meters[0];
        assert_eq!(m.delta_count, m.deltas.len());
        assert_eq!(
            m.sum_positive,
            m.deltas
                .iter()
                .map(|d| d.delta)
                .filter(|d| *d > 0)
                .sum::<i64>(),
        );
    }

    /// R772 — a bake declares the files it read, and the declaration must be the
    /// path the loader OPENS. The oracle is that resolver itself, never a second
    /// hand-written list: a hand-written list is a copy of exactly the class of
    /// bug being killed here (R770 hand-wrote the built-in default and the first
    /// real consumer declares `[atomic] sidecar_path`, so the store it actually
    /// read went unwatched).
    #[test]
    fn projection_inputs_name_the_files_the_loader_resolves() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::write(
            root.join("mnemosyne.toml"),
            "[workspace]\nroot = \".\"\n\n[atomic]\nsidecar_path = \".atomic/tide.atomic.json\"\n\
             \n[continuity]\ncanon_order_path = \"canon-order.json\"\n",
        )
        .unwrap();

        let inputs = projection_inputs(root, None).expect("resolve the declared inputs");

        // Derived: whatever the loader would open must be declared.
        let opened = resolve_sidecar(root, None).expect("the loader's own resolution");
        assert!(
            inputs.contains(&opened),
            "the sidecar the loader opens ({}) is not declared: {inputs:?}",
            opened.display()
        );
        assert!(inputs.contains(&root.join("mnemosyne.toml")));
        assert!(inputs.contains(&root.join("canon-order.json")));

        // Non-vacuity: the built-in default is NOT this workspace's store, so a
        // declaration that still named it would pass every assertion above while
        // watching a file that does not exist.
        assert!(
            !inputs.contains(&AtomicStore::default_sidecar_path(root)),
            "the built-in default is declared though the config moved the store"
        );
    }

    /// A workspace with no config yet declares its would-be config anyway:
    /// creating one moves the sidecar, and an undeclared creation is the same
    /// staleness one step later.
    #[test]
    fn projection_inputs_declare_the_config_that_does_not_exist_yet() {
        let tmp = TempDir::new().unwrap();
        let inputs = projection_inputs(tmp.path(), None).expect("no config is not an error");
        assert!(inputs.contains(&tmp.path().join("mnemosyne.toml")));
        assert!(inputs.contains(&AtomicStore::default_sidecar_path(tmp.path())));
    }

    /// No config file = nothing to scan = an empty hit set, not an error.
    #[test]
    fn inventory_decay_scan_missing_config_is_empty_not_error() {
        let tmp = TempDir::new().unwrap();
        let hits = inventory_decay_scan(tmp.path(), "X").expect("missing config = empty");
        assert!(hits.is_empty());
    }

    /// `entity_kinds` returns the WHOLE registry as `id -> kind`: every entity,
    /// its declared kind (a kind-less entity reads as `""`, present not absent).
    /// The bulk read tide's object/place gates validate their kind registries
    /// against.
    #[test]
    fn entity_kinds_maps_each_entity_to_its_declared_kind() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::write(
            root.join("mnemosyne.toml"),
            "[workspace]\nroot = \".\"\n\n[atomic]\nsidecar_path = \"store.json\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join("store.json"),
            r#"{"schema_version":39,"sections":{},"frames":{},"narrative_facts":{},
               "entities":{"ent-post":{"kind":"object"},"ent-weir":{"kind":"place"},
                           "ent-bell":{"kind":"object"},"ent-nameless":{}}}"#,
        )
        .unwrap();
        let kinds = entity_kinds(root, None).expect("entity_kinds reads the store");
        assert_eq!(kinds.get("ent-post").map(String::as_str), Some("object"));
        assert_eq!(kinds.get("ent-weir").map(String::as_str), Some("place"));
        assert_eq!(kinds.get("ent-bell").map(String::as_str), Some("object"));
        assert_eq!(kinds.get("ent-nameless").map(String::as_str), Some(""));
        assert_eq!(kinds.len(), 4);
    }

    /// R757 P3b — the bulk read the engine's `store_passages` projects into
    /// provenance-bound passages: each section's `content_excerpt` (R756 P3a) read
    /// from the store; a section without one is omitted (no invented prose).
    #[test]
    fn section_content_excerpts_reads_each_sections_prose_anchor() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::write(
            root.join("mnemosyne.toml"),
            "[workspace]\nroot = \".\"\n\n[atomic]\nsidecar_path = \"store.json\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join("store.json"),
            r#"{"schema_version":41,"frames":{},"narrative_facts":{},"entities":{},
               "sections":{
                 "d01-nat":{"content_excerpt":{
                   "anchor":{"source":"MANUSCRIPT.md","locator":{"Prefix":"지운은"}},
                   "text":"지운은 둑에 발을 올렸다.","text_sha256":""}},
                 "d02-nat":{}
               }}"#,
        )
        .unwrap();
        let ex = section_content_excerpts(root, None).expect("reads the store");
        let d01 = ex.get("d01-nat").expect("d01-nat has an excerpt");
        assert_eq!(d01.text, "지운은 둑에 발을 올렸다.");
        assert_eq!(d01.anchor.source, "MANUSCRIPT.md");
        assert!(!ex.contains_key("d02-nat"));
        assert_eq!(ex.len(), 1);
    }

    /// A malformed mnemosyne.toml fails loud instead of silently reporting
    /// "no decay" — regression for the R360/R362 sibling swallows the R364
    /// sweep closed (`let Ok(Some) = discover_config` + `unwrap_or_default`).
    #[test]
    fn inventory_decay_scan_malformed_config_fails_loud() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("mnemosyne.toml"), "[plugins\nbad = ").unwrap();
        assert!(
            inventory_decay_scan(tmp.path(), "X").is_err(),
            "malformed config must fail loud, not silently empty"
        );
    }

    /// A minimal narrative workspace: sections sc-1/sc-2 (a canon chain), a
    /// `gt` frame, and one fact anchored at sc-1. `[continuity].severity`
    /// configurable so a test can exercise the gate policy.
    fn narrative_ws(severity: &str) -> TempDir {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::write(
            root.join("mnemosyne.toml"),
            format!(
                "[workspace]\nroot = \".\"\n\n[atomic]\nsidecar_path = \"store.json\"\n\n\
                 [continuity]\ncanon_order_path = \"canon.json\"\nseverity = \"{severity}\"\n"
            ),
        )
        .unwrap();
        std::fs::write(
            root.join("canon.json"),
            r#"{"edges":[["sc-1","sc-2"]],"branches":{}}"#,
        )
        .unwrap();
        std::fs::write(
            root.join("store.json"),
            r#"{"schema_version":23,"sections":{"sc-1":{},"sc-2":{}},"frames":{"gt":{}},
               "narrative_facts":{"f-1":{"frame":"gt","claim":"c","canon_from":"sc-1","evidence":["sc-1"]}}}"#,
        )
        .unwrap();
        tmp
    }

    fn fact_at(fact_id: &str, section: &str, frame: &str) -> mnemosyne_atomic::FactImport {
        mnemosyne_atomic::FactImport {
            fact_id: fact_id.to_string(),
            frame: frame.to_string(),
            branch: None,
            entities: vec![],
            claim: "a candidate claim".to_string(),
            canon_from: section.to_string(),
            canon_to: None,
            evidence: vec![section.to_string()],
            conflicts_with: vec![],
            supersedes_in_frame: None,
            payoff_expectation: None,
            pays_off: vec![],
            typed: None,
            quote: None,
        }
    }

    fn manifest(facts: Vec<mnemosyne_atomic::FactImport>) -> mnemosyne_atomic::FactsManifest {
        mnemosyne_atomic::FactsManifest {
            edge_costs: Vec::new(),
            edge_guards: Vec::new(),
            frames: vec![],
            branches: vec![],
            entity_kinds: vec![],
            units: vec![],
            entities: vec![],
            predicates: vec![],
            facts,
            disclosure_plans: vec![],
        }
    }

    /// A clean candidate commits; a bad-frame candidate rolls back with a shape
    /// violation and leaves the store untouched (Round 588/592).
    #[test]
    fn propose_verdict_commit_and_shape_rollback() {
        let ws = narrative_ws("reject");
        let root = ws.path();

        let clean = propose_verdict(
            root,
            None,
            None,
            None,
            &manifest(vec![fact_at("f-2", "sc-2", "gt")]),
        )
        .unwrap();
        assert_eq!(clean.verdict, ProposeVerdict::Commit);
        assert_eq!(clean.gating_violation_count, 0);
        assert!(clean.violations.is_empty());

        let bad = propose_verdict(
            root,
            None,
            None,
            None,
            &manifest(vec![fact_at("f-3", "sc-1", "ghost-frame")]),
        )
        .unwrap();
        assert_eq!(bad.verdict, ProposeVerdict::Rollback);
        assert_eq!(bad.gating_violation_count, 1);
        assert_eq!(
            bad.violations[0].source,
            mnemosyne_validate::verdict::ViolationSource::Shape
        );

        // Dry run: the store still holds exactly the seeded fact.
        let store = load_atomic_store(root, None).unwrap();
        assert_eq!(store.narrative_facts.len(), 1);
    }

    /// Round 592 (finding 1): a structural violation gates under the default
    /// `reject` severity but NOT under `warn` — propose-verdict mirrors the
    /// store's configured policy instead of rolling back on everything.
    #[test]
    fn propose_verdict_mirrors_configured_severity() {
        // A fact defaulting to `main` while the canon chain positions sc-1/sc-2 on
        // main is fine; force an off-branch by pointing canon_from at an unordered
        // section is not possible here, so use a warn-severity store and a
        // conflicting pair to produce a structural violation.
        let bad_pair = vec![
            {
                let mut f = fact_at("f-a", "sc-1", "gt");
                f.claim = "the bell rang".into();
                f
            },
            {
                let mut f = fact_at("f-b", "sc-1", "gt");
                f.claim = "the bell was silent".into();
                f.conflicts_with = vec!["f-a".into()];
                f
            },
        ];
        // reject severity → the conflict gates → rollback.
        let ws_reject = narrative_ws("reject");
        let r = propose_verdict(
            ws_reject.path(),
            None,
            None,
            None,
            &manifest(bad_pair.clone()),
        )
        .unwrap();
        assert_eq!(r.verdict, ProposeVerdict::Rollback);
        assert!(r.gating_violation_count >= 1);
        // warn severity → the SAME conflict is surfaced but does NOT gate → commit.
        let ws_warn = narrative_ws("warn");
        let w = propose_verdict(ws_warn.path(), None, None, None, &manifest(bad_pair)).unwrap();
        assert_eq!(w.verdict, ProposeVerdict::Commit);
        assert_eq!(w.gating_violation_count, 0);
        assert!(
            !w.violations.is_empty(),
            "a warn-level violation must still be surfaced on a commit"
        );
    }

    /// Round 599 (unattended-loop-experiment/v2 gap A): propose-verdict surfaces
    /// a would-be dangling setup as an ADVISORY on the dry run — the verdict
    /// stays `commit` (dangling never gates), but the loop sees the dangling
    /// BEFORE it imports, so a bare-prefix dangle no longer requires a
    /// post-import store reset to discover.
    #[test]
    fn propose_verdict_surfaces_dangling_advisory_without_gating() {
        let ws = narrative_ws("reject");
        // An Expected setup with no payoff dangles on `main`.
        let mut setup = fact_at("f-setup", "sc-1", "gt");
        setup.payoff_expectation = Some("expected".to_string());
        let r = propose_verdict(ws.path(), None, None, None, &manifest(vec![setup])).unwrap();
        // Non-gating: the setup is a valid write, so the batch commits.
        assert_eq!(r.verdict, ProposeVerdict::Commit);
        assert_eq!(r.gating_violation_count, 0);
        // The dangling IS surfaced in the dry run, per world-line.
        assert!(
            r.dangling_setups
                .get("main")
                .is_some_and(|d| d.contains(&"f-setup".to_string())),
            "dangling advisory must name f-setup on main: {:?}",
            r.dangling_setups
        );
    }

    /// The authoring frontier reports zero-fact scenes, per-scene coverage, and
    /// gates the telling-scoped sections behind `--telling` (Round 589).
    #[test]
    fn authoring_frontier_reports_gaps_and_gates_telling() {
        let ws = narrative_ws("reject");
        let r = authoring_frontier_report(ws.path(), None, None, None, None).unwrap();
        assert_eq!(r.zero_fact_scenes, vec!["sc-2".to_string()]);
        let census: std::collections::BTreeMap<_, _> = r
            .scene_coverage
            .iter()
            .map(|s| (s.scene.as_str(), s.facts.clone()))
            .collect();
        // NAMED, not counted (Round 1053): `1` here would hold for a census that
        // credited the fact to the wrong scene, which is what shipped until that
        // round and what nothing could have caught.
        assert_eq!(census["sc-1"], vec!["f-1".to_string()]);
        assert!(census["sc-2"].is_empty());
        // The canon order (canon.json edges sc-1 -> sc-2) covers the fact-bearing
        // sc-1, so nothing is unordered (Round 596).
        assert!(r.unordered_scenes.is_empty());
        assert_eq!(r.total_gaps, 1); // just the one zero-fact scene
                                     // Telling-scoped sections are omitted without a telling.
        assert!(r.telling.is_none());
        assert!(r.unresolved_quests.is_none());
        assert!(r.never_planned_disclosures.is_none());
        assert!(r.disclosures_seated_before_truth.is_none());
    }

    /// A DISCLOSURE SEATED BEFORE ITS FACT IS TRUE IS A GAP (Round 949).
    ///
    /// Seating a disclosure LATE is what a telling is for. Seating it EARLY
    /// states to the reader something the store itself says is not yet so, and
    /// until this round nothing read the seat at all: Round 949 built one and
    /// watched the store import it, the renderer print the fact ten scenes
    /// before it was true, one character stand in two rooms in one scene, and
    /// `validate-continuity` answer `violations: 0`.
    ///
    /// THE THREE ARMS ARE THE GUARD, in one plan. A seat LATER than `canon_from`
    /// is the legitimate reveal and must not be flagged — without that arm this
    /// would pass while flagging every authored surface. A seat EQUAL to it is
    /// the ordinary case. Only the earlier one is a gap.
    #[test]
    fn a_disclosure_seated_before_its_fact_is_true_is_a_gap() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::write(
            root.join("mnemosyne.toml"),
            "[workspace]\nroot = \".\"\n\n[atomic]\nsidecar_path = \"store.json\"\n\n             [continuity]\ncanon_order_path = \"canon.json\"\nseverity = \"reject\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join("canon.json"),
            r#"{"edges":[["sc-1","sc-2"],["sc-2","sc-3"]],"branches":{}}"#,
        )
        .unwrap();
        // FOUR arms, and every one of them is load-bearing. An earlier draft had
        // three that were really early/equal/EQUAL, no off-road pair, and a
        // `total_gaps >= 1` that the zero-fact scenes satisfied on their own —
        // all three injections against it stayed green.
        let fact = |id: &str, truth: &str| {
            format!(
                r#""{id}":{{"frame":"gt","claim":"a thing is so","canon_from":"{truth}","branch":"main","entities":[],"evidence":["{truth}"]}}"#
            )
        };
        let ovr = |id: &str, seat: &str| {
            format!(r#""{id}":{{"mode":"state","first_at":{{}},"surface":{{"scene":"{seat}"}}}}"#)
        };
        // `sc-4` is a registered section the canon order does NOT place, so a
        // seat there cannot be ordered against anything — the skip arm.
        std::fs::write(
            root.join("store.json"),
            format!(
                r#"{{"schema_version":23,"sections":{{"sc-1":{{}},"sc-2":{{}},"sc-3":{{}},"sc-4":{{}}}},"frames":{{"gt":{{}}}},"narrative_facts":{{{},{},{},{}}},"disclosure_plans":{{"t":{{"default_mode":"state","overrides":{{{},{},{},{}}}}}}}}}"#,
                fact("f-early", "sc-3"),
                fact("f-at", "sc-3"),
                fact("f-late", "sc-1"),
                fact("f-offroad", "sc-3"),
                ovr("f-early", "sc-1"),
                ovr("f-at", "sc-3"),
                ovr("f-late", "sc-3"),
                ovr("f-offroad", "sc-4"),
            ),
        )
        .unwrap();

        let r = authoring_frontier_report(root, None, None, Some("t"), None).unwrap();
        let rows = r
            .disclosures_seated_before_truth
            .as_ref()
            .expect("a telling was given");
        assert_eq!(
            rows,
            &vec![SeatBeforeTruth {
                fact_id: "f-early".to_string(),
                world: "main".to_string(),
                seated_at: "sc-1".to_string(),
                true_from: "sc-3".to_string(),
            }],
            "only the seat EARLIER than its fact's truth is a gap"
        );
        // EXACT, not `>= 1`: the store's own zero-fact scenes satisfy a floor on
        // their own, so a floor assertion cannot tell whether the seat was
        // counted at all. Removing it from the tally must move this number.
        let without_telling = authoring_frontier_report(root, None, None, None, None)
            .unwrap()
            .total_gaps;
        assert_eq!(
            r.total_gaps,
            without_telling + 1,
            "the seat counts as work remaining, not a footnote"
        );
    }

    /// Round 596 (unattended-loop-experiment/v1 Finding 4): a fact-bearing scene
    /// the canon order does not place is surfaced as an `unordered` gap — the
    /// frontier's signal that a renderable store still needs its order artifact.
    /// With an empty order, EVERY fact-bearing scene lands here (the exact gap
    /// the loop's "done" — frontier 0/0/0 — used to hide).
    #[test]
    fn authoring_frontier_flags_unordered_scenes_when_order_absent() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::write(
            root.join("mnemosyne.toml"),
            "[workspace]\nroot = \".\"\n\n[atomic]\nsidecar_path = \"store.json\"\n\n\
             [continuity]\ncanon_order_path = \"canon.json\"\nseverity = \"reject\"\n",
        )
        .unwrap();
        // An empty order declares no edges: nothing is placed.
        std::fs::write(root.join("canon.json"), r#"{"edges":[],"branches":{}}"#).unwrap();
        std::fs::write(
            root.join("store.json"),
            r#"{"schema_version":23,"sections":{"sc-1":{},"sc-2":{}},"frames":{"gt":{}},
               "narrative_facts":{"f-1":{"frame":"gt","claim":"c","canon_from":"sc-1","evidence":["sc-1"]}}}"#,
        )
        .unwrap();
        let r = authoring_frontier_report(root, None, None, None, None).unwrap();
        // sc-1 carries a fact but the order places nothing -> unordered.
        assert_eq!(r.unordered_scenes, vec!["sc-1".to_string()]);
        // sc-2 is zero-fact (a distinct gap) but not fact-bearing, so not unordered.
        assert_eq!(r.zero_fact_scenes, vec!["sc-2".to_string()]);
        assert_eq!(r.total_gaps, 2); // one zero-fact + one unordered
    }

    /// Round 857 — a WORLD-SCOPED read refuses an undeclared canon order, while
    /// the frontier — whose subject IS the absence — still answers.
    ///
    /// Found by consuming `report-quest-graph` the way a projection runtime
    /// would. On the first playable consumer's real store, run without `--order`,
    /// it printed 22 quests over 7 worlds with per-world states and no
    /// complaint — from a manuscript walk that had visited ZERO scenes. Against
    /// the declared order the same command reports 117 done where the silent run
    /// reported 65, and 12 giver locators where it reported none.
    ///
    /// Both sides are asserted on ONE fixture because the two readings of an
    /// empty order are both ratified and must not collapse into each other: the
    /// per-world question is unanswerable, and "which scenes are not yet
    /// ordered" is exactly answerable. A guard in the shared inner projection
    /// would have broken the second — it did, in this round's first cut, and
    /// these two frontier tests are what caught it.
    #[test]
    fn a_world_scoped_read_refuses_an_undeclared_order_but_the_frontier_answers() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::write(
            root.join("mnemosyne.toml"),
            "[workspace]\nroot = \".\"\n\n[atomic]\nsidecar_path = \"store.json\"\n\n\
             [continuity]\ncanon_order_path = \"canon.json\"\nseverity = \"reject\"\n",
        )
        .unwrap();
        std::fs::write(root.join("canon.json"), r#"{"edges":[],"branches":{}}"#).unwrap();
        std::fs::write(
            root.join("store.json"),
            r#"{"schema_version":23,"sections":{"sc-1":{},"sc-2":{}},"frames":{"gt":{}},
               "disclosure_plans":{"player":{"description":"d","overrides":{}}},
               "narrative_facts":{"f-1":{"frame":"gt","claim":"c","canon_from":"sc-1","evidence":["sc-1"]}}}"#,
        )
        .unwrap();

        for (what, err) in [
            (
                "quest graph",
                quest_graph_report(root, None, None, None, "player").err(),
            ),
            (
                "payoff coverage",
                payoff_coverage_report(root, None, None).err(),
            ),
            (
                "playable world",
                playable_world_report(root, None, None, None, "player").err(),
            ),
            (
                "playthrough manuscript",
                playthrough_manuscript_report(root, None, None, None, Some("player"), false).err(),
            ),
        ] {
            let msg = format!("{:#}", err.unwrap_or_else(|| panic!("{what} must refuse")));
            assert!(
                msg.contains("no canon order is declared") && msg.contains("--order"),
                "{what}: the refusal must name the state and the repair: {msg}"
            );
            assert!(
                msg.contains("2 section(s)") && msg.contains("1 fact(s)"),
                "{what}: and what would have been silently skipped: {msg}"
            );
        }

        // The ratified exception, unchanged.
        let frontier = authoring_frontier_report(root, None, None, None, None).unwrap();
        assert_eq!(frontier.unordered_scenes, vec!["sc-1".to_string()]);

        // CONTROL: declare one edge and the world-scoped read answers, so the
        // refusal is about the empty declaration and not about the fixture.
        std::fs::write(
            root.join("canon.json"),
            r#"{"edges":[["sc-1","sc-2"]],"branches":{}}"#,
        )
        .unwrap();
        let graph =
            quest_graph_report(root, None, None, None, "player").expect("a declared order answers");
        assert_eq!(graph.worlds, vec!["main".to_string()]);
    }

    /// Round 1048 — THE READING PRUNE CHANGES THE ANSWER, AND THE ANSWER SAYS
    /// SO. `--reading-walk` drops every scene that introduces no content, so
    /// two manuscripts of one store differ by which scenes are in them; the
    /// report records the flag, because nothing else in the output distinguishes
    /// "this store has no contentless scenes" from "they were pruned away".
    ///
    /// NO AUTHORED CORPUS CAN SHOW THIS. All 823 scenes across the 28 corpora
    /// this tree can ask begin at least one fact, so the derived provenance gate
    /// (`read_argument_provenance.rs`) reads the flag as INERT and demands
    /// nothing — it is quiet for want of a corpus, not for want of a hole. The
    /// tree shows the class instead, which is the R1045 discipline.
    #[test]
    fn the_reading_walk_prunes_contentless_scenes_and_the_report_says_it_did() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::write(
            root.join("mnemosyne.toml"),
            "[workspace]\nroot = \".\"\n\n[atomic]\nsidecar_path = \"store.json\"\n\n\
             [continuity]\ncanon_order_path = \"canon.json\"\nseverity = \"reject\"\n",
        )
        .unwrap();
        // `sc-2` is on the road and introduces nothing — the scene the reading
        // copy drops and the structural manuscript keeps.
        std::fs::write(
            root.join("canon.json"),
            r#"{"edges":[["sc-1","sc-2"]],"branches":{}}"#,
        )
        .unwrap();
        std::fs::write(
            root.join("store.json"),
            r#"{"schema_version":23,"sections":{"sc-1":{},"sc-2":{}},"frames":{"gt":{}},
               "disclosure_plans":{"player":{"description":"d","overrides":{}}},
               "narrative_facts":{"f-1":{"frame":"gt","claim":"c","canon_from":"sc-1","evidence":["sc-1"]}}}"#,
        )
        .unwrap();

        let read = |reading_walk: bool| {
            playthrough_manuscript_report(root, None, None, None, Some("player"), reading_walk)
                .expect("a declared order answers")
        };
        let structural = read(false);
        let reading = read(true);

        let scenes = |report: &mnemosyne_validate::continuity::PlaythroughManuscriptReport| {
            report.worlds["main"]
                .scenes
                .iter()
                .map(|scene| scene.section.clone())
                .collect::<Vec<_>>()
        };
        // NON-VACUITY FIRST: the prune must actually remove something here, or
        // the provenance claim below rides on two identical answers.
        assert_eq!(scenes(&structural), vec!["sc-1", "sc-2"]);
        assert_eq!(scenes(&reading), vec!["sc-1"]);
        assert_eq!(
            (structural.reading_walk, reading.reading_walk),
            (false, true),
            "the two answers differ, so each has to say which walk produced it"
        );
        // And the other two provenance fields are unchanged by the prune —
        // whichever walk ran, the telling and the road filter are what was asked.
        for report in [&structural, &reading] {
            assert_eq!(report.telling.as_deref(), Some("player"));
            assert_eq!(report.world, None);
        }
    }

    /// Round 667 — placement is its own axis, and the EMPTY unplaced section is
    /// the case that had no computation anywhere: `unordered_scenes` filters to
    /// fact-bearing (R596, renderability), `zero_fact_scenes` filters on content
    /// with no placement predicate, so an empty unplaced section sat in
    /// `zero_fact_scenes` indistinguishable from a placed empty. R663 injected
    /// exactly that and read the silence as "the substrate cannot compare".
    ///
    /// The fixture is built around that CONFOUND: `s2` (empty, PLACED) beside
    /// `s4` (empty, UNPLACED). A store whose empties are all placed — which is
    /// what the first cut of this round measured — cannot tell the two apart,
    /// and every claim about the split looks true by accident.
    #[test]
    fn authoring_frontier_unplaced_scenes_are_content_independent() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::write(
            root.join("mnemosyne.toml"),
            "[workspace]\nroot = \".\"\n\n[atomic]\nsidecar_path = \"store.json\"\n\n\
             [continuity]\ncanon_order_path = \"canon.json\"\nseverity = \"reject\"\n",
        )
        .unwrap();
        // The order positions s1 and s2 only; s3 and s4 are unplaced.
        std::fs::write(
            root.join("canon.json"),
            r#"{"edges":[["s1","s2"]],"branches":{}}"#,
        )
        .unwrap();
        // s1 fact/placed · s2 empty/PLACED · s3 fact/unplaced · s4 empty/unplaced.
        std::fs::write(
            root.join("store.json"),
            r#"{"schema_version":23,"sections":{"s1":{},"s2":{},"s3":{},"s4":{}},
               "frames":{"gt":{}},
               "narrative_facts":{
                 "f-1":{"frame":"gt","claim":"c","canon_from":"s1","evidence":["s1"]},
                 "f-3":{"frame":"gt","claim":"c","canon_from":"s3","evidence":["s3"]}}}"#,
        )
        .unwrap();
        let r = authoring_frontier_report(root, None, None, None, None).unwrap();

        // The placement axis, regardless of content — s4 is the half that used
        // to be computed nowhere.
        assert_eq!(r.unplaced_scenes, vec!["s3".to_string(), "s4".to_string()]);
        // The PLACED empty is not unplaced: the confound, pinned.
        assert!(!r.unplaced_scenes.contains(&"s2".to_string()));
        // Content axis, blind to placement: both empties, placed or not.
        assert_eq!(r.zero_fact_scenes, vec!["s2".to_string(), "s4".to_string()]);
        // Renderability = the fact-bearing projection of the placement set (R596).
        assert_eq!(r.unordered_scenes, vec!["s3".to_string()]);
        assert!(
            r.unordered_scenes
                .iter()
                .all(|s| r.unplaced_scenes.contains(s)),
            "unordered must stay a subset of unplaced: {:?} vs {:?}",
            r.unordered_scenes,
            r.unplaced_scenes
        );
        // No double count: zero-fact {s2,s4} and unordered {s3} are disjoint and
        // cover unplaced, so unplaced_scenes must NOT add to the total.
        assert_eq!(r.total_gaps, 3);

        // THE IDENTITY THE CLI NOTICE RESTS ON, pinned in the one crate that can
        // see both sides: the notice prints `sections - order_nodes` and sends
        // the reader to `unplaced scenes`, so those must be the SAME number or
        // the pointer lies — which is exactly how this round's first cut shipped
        // (it counted 3 at a list of 1).
        let scan = continuity_scan(root, None, None, None).unwrap();
        assert_eq!(scan.sections, 4);
        // Round 1061 — the placed side is NAMED, so the identity below is a
        // statement about two lists rather than about two numbers that agree.
        assert_eq!(scan.order_nodes, ["s1", "s2"]);
        assert_eq!(
            scan.sections - scan.order_nodes.len(),
            r.unplaced_scenes.len(),
            "the notice's count must equal the list it points at"
        );
    }

    /// Round 667 — the notice is GUARDED on a declared order, because an order
    /// with no nodes is not an incomplete order: it is a store that never
    /// declared one. A SPEC store is that shape (sections, zero facts, no
    /// `[continuity]`), and Mnemosyne's own reads `0/5` — unguarded, the notice
    /// told it five spec sections were unrenderable scenes. The guard lives in
    /// the CLI, so what is pinned here is the STATE it keys off: `order_nodes ==
    /// 0` while sections stand, with the missing-order signal still carried by
    /// R596's `unordered_scenes` (every fact-bearing scene) so nothing is lost.
    #[test]
    fn no_declared_order_is_not_an_incomplete_order() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::write(
            root.join("mnemosyne.toml"),
            "[workspace]\nroot = \".\"\n\n[atomic]\nsidecar_path = \"store.json\"\n\n\
             [continuity]\ncanon_order_path = \"canon.json\"\nseverity = \"reject\"\n",
        )
        .unwrap();
        std::fs::write(root.join("canon.json"), r#"{"edges":[],"branches":{}}"#).unwrap();
        std::fs::write(
            root.join("store.json"),
            r#"{"schema_version":23,"sections":{"s1":{},"s2":{}},"frames":{"gt":{}},
               "narrative_facts":{
                 "f-1":{"frame":"gt","claim":"c","canon_from":"s1","evidence":["s1"]}}}"#,
        )
        .unwrap();
        let scan = continuity_scan(root, None, None, None).unwrap();
        // The state the CLI guard reads: no order declared at all.
        assert!(scan.order_nodes.is_empty());
        assert_eq!(scan.sections, 2);

        // Nothing is lost by staying quiet: R596 already reports every
        // fact-bearing scene when no order is declared.
        let r = authoring_frontier_report(root, None, None, None, None).unwrap();
        assert_eq!(r.unordered_scenes, vec!["s1".to_string()]);
        assert_eq!(r.unplaced_scenes, vec!["s1".to_string(), "s2".to_string()]);
    }

    /// Round 617 (density) corrected Round 619: branch-owned density = a
    /// world-line's own facts over the FULL road it TRAVELS, so a world that
    /// rides a long inherited road while owning little reads LOW. Locks the R619
    /// fixes: every world (incl. a CONFLUENCE and a facts-only/undeclared
    /// divergence) gets a `Some` density — never a divide-by-zero, never the
    /// confusing "rides the trunk" `None` the own-segment version produced — and
    /// `density > 1.0` (facts-per-scene) is a legitimate value.
    #[test]
    fn authoring_frontier_branch_owned_density() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::write(
            root.join("mnemosyne.toml"),
            "[workspace]\nroot = \".\"\n\n[atomic]\nsidecar_path = \"store.json\"\n\n\
             [continuity]\ncanon_order_path = \"canon.json\"\nseverity = \"reject\"\n",
        )
        .unwrap();
        // main s1 -> s2 (base); `braid` forks main@s1 declaring NO road (facts-only
        // divergence); `weave` is a CONFLUENCE of {main@s2, braid@s2} declaring the
        // continuation s2 -> s3. Every world travels {s1,s2,s3} (3 scenes).
        std::fs::write(
            root.join("canon.json"),
            r#"{"edges":[["s1","s2"]],"branches":{"weave":[["s2","s3"]]}}"#,
        )
        .unwrap();
        std::fs::write(
            root.join("store.json"),
            r#"{"schema_version":23,
               "sections":{"s1":{},"s2":{},"s3":{}},
               "frames":{"gt":{}},
               "branches":{"braid":{"forks_from":{"branch":"main","at":"s1"}},
                           "weave":{"converges_from":[{"branch":"main","at":"s2"},
                                                       {"branch":"braid","at":"s2"}]}},
               "narrative_facts":{
                 "f-m1":{"frame":"gt","claim":"c","canon_from":"s1","evidence":["s1"]},
                 "f-m2":{"frame":"gt","claim":"c","canon_from":"s1","evidence":["s1"]},
                 "f-m3":{"frame":"gt","claim":"c","canon_from":"s1","evidence":["s1"]},
                 "f-m4":{"frame":"gt","claim":"c","canon_from":"s1","evidence":["s1"]},
                 "f-w1":{"frame":"gt","branch":"weave","claim":"c","canon_from":"s3","evidence":["s3"]},
                 "f-b1":{"frame":"gt","branch":"braid","claim":"c","canon_from":"s2","evidence":["s2"]}}}"#,
        )
        .unwrap();
        let r = authoring_frontier_report(root, None, None, None, None).unwrap();
        let d = &r.branch_owned_density;

        let owned = |d: &BranchDensity| -> Vec<String> { d.owned.clone() };
        let road = |d: &BranchDensity| -> Vec<String> { d.road.clone() };
        let scenes = ["s1", "s2", "s3"].map(str::to_string).to_vec();

        // main owns 4 facts over its 3 traversed scenes -> density > 1.0.
        let m = &d["main"];
        assert_eq!(
            (owned(m), road(m)),
            (
                ["f-m1", "f-m2", "f-m3", "f-m4"]
                    .map(str::to_string)
                    .to_vec(),
                scenes.clone()
            ),
            "Round 1054 named the numerator and Round 1061 the denominator. This \
             line read `(m.owned_facts, m.road_scenes) == (4, 3)`, and each half \
             was named the round a corruption population first reached the \
             manifest it lives in — facts, then the canon order"
        );
        assert_eq!(m.density, Some(4.0 / 3.0));

        // the CONFLUENCE gets a real density over its full traversal — no
        // divide-by-zero (the own-segment version's fatal case), no `None`.
        let w = &d["weave"];
        assert_eq!(
            (owned(w), road(w)),
            (["f-w1"].map(str::to_string).to_vec(), scenes.clone())
        );
        assert_eq!(w.density, Some(1.0 / 3.0));

        // the facts-only / undeclared-road divergence gets a real density too —
        // it rides a 3-scene road owning 1 fact, NOT a confusing "n/a rides trunk".
        let b = &d["braid"];
        assert_eq!(
            (owned(b), road(b)),
            (["f-b1"].map(str::to_string).to_vec(), scenes)
        );
        assert_eq!(b.density, Some(1.0 / 3.0));
        assert!(b.density.is_some(), "a facts-only divergence is never None");

        // density is a pure read: it does NOT feed the gap gauge.
        assert_eq!(r.total_gaps, 0);
    }
}
