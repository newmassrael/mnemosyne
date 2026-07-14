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

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use mnemosyne_atomic::{AtomicMutateError, AtomicMutateReceipt, AtomicStore};
use serde::Serialize;
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
pub fn resolve_sidecar(anchor: &Path, sidecar: Option<&Path>) -> anyhow::Result<PathBuf> {
    let s = sidecar.map(|p| p.to_string_lossy().into_owned());
    cascade::resolve_sidecar(anchor, s.as_deref())
}

/// Run an atomic mutate primitive in-process: load the store, invoke the
/// supplied closure against it, and return the receipt. The closure
/// persists the store itself (`save_with_receipt`); the atomic store is the
/// only artifact, so there is nothing further to regenerate.
pub fn run_atomic_mutate<F>(
    workspace_root: &Path,
    sidecar: Option<&Path>,
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

/// Load the atomic store at the resolved sidecar path.
///
/// A missing sidecar is NOT an error — `AtomicStore::load` already returns an
/// empty store for a fresh workspace. This propagates only genuine failures
/// (corrupt JSON, IO error, or a newer-than-supported `schema_version`) so a
/// corrupt SSOT fails loud instead of silently reading as empty (the prior
/// `unwrap_or_default` masked corruption as a clean empty store).
pub fn load_atomic_store(
    workspace_root: &Path,
    sidecar: Option<&Path>,
) -> Result<AtomicStore, OpError> {
    let sidecar_path = resolve_sidecar(workspace_root, sidecar)?;
    AtomicStore::load(&sidecar_path).map_err(|e| OpError::Other(format!("{}", e)))
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
    order_override: Option<&str>,
) -> Result<mnemosyne_validate::continuity::CanonOrderFile, OpError> {
    use mnemosyne_validate::continuity::{load_canon_order, CanonOrderFile};
    let cont = policy.continuity.as_ref();
    match (
        order_override,
        cont.and_then(|c| c.canon_order_path.as_ref()),
    ) {
        // R538 — an explicit `--order` CLI override is CWD-relative (the same
        // rule as `--sidecar` / `--manifest`; the config-declared path below
        // stays workspace-rooted). Bypasses the sha256 pin (the pin claims
        // nothing about a different file — the R428 `--catalog` rule).
        (Some(p), _) => {
            let cwd = std::env::current_dir()
                .map_err(|e| OpError::Other(format!("CWD lookup for --order resolution: {e}")))?;
            load_canon_order(&cascade::resolve_explicit_cli_path(&cwd, p), None)
                .map_err(OpError::Other)
        }
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
    rules_override: Option<&str>,
) -> Result<mnemosyne_validate::continuity::NarrativeRulesFile, OpError> {
    use mnemosyne_validate::continuity::{load_narrative_rules, NarrativeRulesFile};
    let cont = policy.continuity.as_ref();
    match (rules_override, cont.and_then(|c| c.rules_path.as_ref())) {
        (Some(p), _) => load_narrative_rules(&policy.root.join(p), None).map_err(OpError::Other),
        (None, Some(p)) => load_narrative_rules(
            &policy.root.join(p),
            cont.and_then(|c| c.rules_sha256.as_deref()),
        )
        .map_err(OpError::Other),
        (None, None) => Ok(NarrativeRulesFile::default()),
    }
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
    pub order_nodes: usize,
    pub conflict_pairs_checked: usize,
    pub cross_scope_pairs: usize,
    pub unordered_pairs: usize,
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
    pub rule_unordered_pairs: usize,
    /// Same-frame same-subject typed pairs no succession PATH connects —
    /// surfaced, never gated (Round 449; path not edge, Round 452).
    pub unchained_state_pairs: usize,
    /// Interval-rule resolutions that could not be evaluated (operand absent
    /// on the right/bound leg, non-numeric, or ambiguous) — surfaced, never
    /// gated (Round 489, the R485 `unverifiable` class).
    pub interval_unverifiable: usize,
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
    sidecar: Option<&Path>,
    order_override: Option<&str>,
    rules_override: Option<&str>,
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
        order_nodes: report.order_nodes,
        conflict_pairs_checked: report.conflict_pairs_checked,
        cross_scope_pairs: report.cross_scope_pairs,
        unordered_pairs: report.unordered_pairs,
        rules: report.rules,
        interval_rules: report.interval_rules,
        undeclared_roads: report.undeclared_roads.clone(),
        rule_unordered_pairs: report.rule_unordered_pairs,
        unchained_state_pairs: report.unchained_state_pairs,
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
    sidecar: Option<&Path>,
    order_override: Option<&str>,
    rules_override: Option<&str>,
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

/// One scene's fact coverage (Round 589) — how many facts are anchored (via
/// their `canon_from`) at this section. `structural` (Round 618, MNEMO-GAP-005
/// part 3a) is the DERIVED subset of `fact_count` that is quest plumbing
/// (`structural_fact_ids`): a coverage read subtracts it so bookkeeping does not
/// inflate "how much narrative a scene carries". Canon-vs-invented is NOT split
/// here — it is per-branch adaptation-fidelity kept consumer-side (decision C);
/// a consumer that wants it combines this with the facts' `branch`.
#[derive(Debug, Clone, Serialize)]
pub struct SceneCoverage {
    pub scene: String,
    pub fact_count: usize,
    pub structural: usize,
}

/// Per-world-line ownership density (Round 617, MNEMO-GAP-005) — how much a
/// world-line OWNS on the road it DECLARED itself, versus the prefix it inherits.
///
/// A divergent world inherits its trunk prefix, so the frontier's zero-fact /
/// per-scene view shows it FULL even when its own segment carries almost nothing;
/// this separates the two. `owned_facts` = every fact authored on the world-line
/// (`branch == B`) anywhere. `own_scene_facts` restricts to facts whose canon
/// coordinate lies on the world's OWN declared segment — road-consistent with the
/// denominator, so an inherited-prefix or shared-fork-coordinate fact never
/// inflates it (the density-inflation trap the R613 fact/road duality warns of).
/// `own_road_scenes` = the size of that own segment. `density` =
/// `own_scene_facts / own_road_scenes`, or **None** when the own segment is EMPTY
/// (a facts-only / undeclared-road divergence riding the trunk — a signal, never
/// `0` "owns nothing at its own scenes" and never infinity). `own_road_declared`
/// is false for such a world (it is in `undeclared_roads`), so a `None` reads
/// "rides the trunk," not a defect. Because a merge relocates trunk ownership
/// onto the confluence (Rounds 612/614), in a merged store the trunk is owned
/// across `main` and the confluences it flows into, each with its own piece.
#[derive(Debug, Clone, Serialize)]
pub struct BranchDensity {
    pub owned_facts: usize,
    pub own_scene_facts: usize,
    pub own_road_scenes: usize,
    pub density: Option<f64>,
    pub own_road_declared: bool,
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
    /// empty scenes to author into, sorted.
    pub zero_fact_scenes: Vec<String>,
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
    /// Dangling setups per world-line (Expected facts with no visible payoff,
    /// R442) — the Chekhov guns still to fire. Only worlds with ≥ 1 dangling.
    pub dangling_setups: BTreeMap<String, Vec<String>>,
    /// Quests whose giving setup could not be bound (no completed_by anchor,
    /// R568). Present only when a telling is given.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unresolved_quests: Option<Vec<String>>,
    /// Facts never given an explicit disclosure decision under the telling
    /// (withheld by default, R507). Present only when a telling is given.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub never_planned_disclosures: Option<Vec<String>>,
    /// Total distinct gap items across every category — the loop's "work
    /// remaining" gauge (a dangling setup counted once across worlds).
    pub total_gaps: usize,
}

/// Compose the authoring-frontier report (Round 589). ONE store load + order
/// compose, then every sub-projection runs over it (no redundant reloads): the
/// scene/fact structure gives zero-fact scenes + per-node coverage, R442 payoff
/// gives per-world dangling setups, and — when a telling is given — R568 quests
/// give the unresolved set and R507 disclosure gives the never-planned facts. A
/// pure read JOIN, never gated.
pub fn authoring_frontier_report(
    workspace_root: &Path,
    sidecar: Option<&Path>,
    order_override: Option<&str>,
    telling: Option<&str>,
) -> Result<AuthoringFrontierReport, OpError> {
    let policy = continuity_policy(workspace_root)?;
    let decl = resolve_canon_order_file(&policy, order_override)?;
    let store = load_atomic_store(workspace_root, sidecar)?;
    let order = compose_canon_order(&decl, &store)?;

    // Scene coverage: every section starts at zero, each fact credits its
    // canon_from (the anchor). A canon_from is always an existing section (the
    // shape gate), so nothing lands outside the map. The structural subset
    // (Round 618, MNEMO-GAP-005) is derived once — quest plumbing that a
    // coverage read subtracts (no stored marker: canon/invented stays
    // consumer-side, decision C).
    let structural_ids = mnemosyne_validate::continuity::structural_fact_ids(&store);
    let mut counts: BTreeMap<String, usize> =
        store.sections.keys().map(|s| (s.clone(), 0usize)).collect();
    let mut structural_counts: BTreeMap<String, usize> =
        store.sections.keys().map(|s| (s.clone(), 0usize)).collect();
    for (fid, fact) in &store.narrative_facts {
        if let Some(c) = counts.get_mut(&fact.canon_from) {
            *c += 1;
        }
        if structural_ids.contains(fid) {
            if let Some(c) = structural_counts.get_mut(&fact.canon_from) {
                *c += 1;
            }
        }
    }
    let zero_fact_scenes: Vec<String> = counts
        .iter()
        .filter(|(_, n)| **n == 0)
        .map(|(s, _)| s.clone())
        .collect();
    // Unordered fact-bearing scenes (Finding 4): a scene carries facts but is
    // not a node of the composed canon order, so no consumer can place it. With
    // no order declared, `nodes()` is empty and every fact-bearing scene lands
    // here — the frontier surfacing the missing order artifact.
    let ordered: BTreeSet<&str> = order.nodes().collect();
    let unordered_scenes: Vec<String> = counts
        .iter()
        .filter(|(scene, n)| **n > 0 && !ordered.contains(scene.as_str()))
        .map(|(s, _)| s.clone())
        .collect();
    let scene_coverage: Vec<SceneCoverage> = counts
        .into_iter()
        .map(|(scene, fact_count)| {
            let structural = structural_counts.get(&scene).copied().unwrap_or(0);
            SceneCoverage {
                scene,
                fact_count,
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
    let (unresolved_quests, never_planned_disclosures) = match telling {
        Some(t) => {
            let quests = mnemosyne_validate::continuity::quest_graph(&store, &order, None, t)
                .map_err(OpError::Other)?;
            let disclosure = mnemosyne_validate::disclosure::disclosure_coverage(&store, t)
                .map_err(OpError::Other)?;
            (
                Some(quests.unresolved_quests),
                Some(disclosure.never_planned),
            )
        }
        None => (None, None),
    };

    // Per-world-line ownership density (Round 617): main + every registered
    // branch, owned facts over the world's OWN declared road segment. Pure read —
    // a None density is a signal (rides the trunk), never a gate, so it does NOT
    // feed total_gaps.
    let undeclared: BTreeSet<&str> = order.undeclared_roads().collect();
    let mut branch_owned_density: BTreeMap<String, BranchDensity> = BTreeMap::new();
    for world in std::iter::once(mnemosyne_core::MAIN_BRANCH)
        .chain(store.branches.keys().map(String::as_str))
    {
        let own = order.own_road_segment(world);
        let own_road_scenes = own.len();
        let mut owned_facts = 0usize;
        let mut own_scene_facts = 0usize;
        for fact in store.narrative_facts.values() {
            if fact.branch == world {
                owned_facts += 1;
                if own.contains(fact.canon_from.as_str()) {
                    own_scene_facts += 1;
                }
            }
        }
        let density =
            (own_road_scenes > 0).then(|| own_scene_facts as f64 / own_road_scenes as f64);
        branch_owned_density.insert(
            world.to_string(),
            BranchDensity {
                owned_facts,
                own_scene_facts,
                own_road_scenes,
                density,
                own_road_declared: !undeclared.contains(world),
            },
        );
    }

    let total_gaps = zero_fact_scenes.len()
        + unordered_scenes.len()
        + distinct_dangling.len()
        + unresolved_quests.as_ref().map_or(0, Vec::len)
        + never_planned_disclosures.as_ref().map_or(0, Vec::len);

    Ok(AuthoringFrontierReport {
        telling: telling.map(str::to_string),
        zero_fact_scenes,
        unordered_scenes,
        scene_coverage,
        branch_owned_density,
        dangling_setups,
        unresolved_quests,
        never_planned_disclosures,
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
    pub not_holding: usize,
    pub unknown: Vec<String>,
}

/// Run the frame-at-T projection (Round 432) over the workspace store with
/// the shared order resolution. `branch` omitted = the default world-line.
pub fn continuity_frame_view(
    workspace_root: &Path,
    sidecar: Option<&Path>,
    frame: &str,
    branch: Option<&str>,
    entity: Option<&str>,
    at: &str,
    order_override: Option<&str>,
) -> Result<FrameViewReport, OpError> {
    let policy = continuity_policy(workspace_root)?;
    let decl = resolve_canon_order_file(&policy, order_override)?;
    let store = load_atomic_store(workspace_root, sidecar)?;
    let order = compose_canon_order(&decl, &store)?;
    let branch = branch.unwrap_or(mnemosyne_core::MAIN_BRANCH);
    let view =
        mnemosyne_validate::continuity::frame_view(&store, &order, frame, branch, entity, at)
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
    })
}

/// Run the setup/payoff coverage classification (Round 442) over the
/// workspace store with the shared order resolution — pure read projection,
/// per query world (main + every registered branch). Dangling setups are a
/// report finding (the author's todo list), deliberately never gated.
pub fn payoff_coverage_report(
    workspace_root: &Path,
    sidecar: Option<&Path>,
    order_override: Option<&str>,
) -> Result<mnemosyne_validate::continuity::PayoffCoverageReport, OpError> {
    let policy = continuity_policy(workspace_root)?;
    let decl = resolve_canon_order_file(&policy, order_override)?;
    let store = load_atomic_store(workspace_root, sidecar)?;
    let order = compose_canon_order(&decl, &store)?;
    mnemosyne_validate::continuity::payoff_coverage(&store, &order).map_err(OpError::Other)
}

/// The typing-discovery input package (Round 458, design sec 7.15 Round
/// A): every untyped fact + the registered vocabulary in one call. Pure
/// read projection; order-independent (typing is a property of the fact,
/// not of any canon declaration), so no order resolution runs.
pub fn typing_candidates_report(
    workspace_root: &Path,
    sidecar: Option<&Path>,
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
    sidecar: Option<&Path>,
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
    sidecar: Option<&Path>,
    order_override: Option<&str>,
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
pub fn timeline_gaps_report(
    workspace_root: &Path,
    sidecar: Option<&Path>,
    order_override: Option<&str>,
    rules_override: Option<&str>,
) -> Result<mnemosyne_validate::continuity::TimelineGapsReport, OpError> {
    let policy = continuity_policy(workspace_root)?;
    let decl = resolve_canon_order_file(&policy, order_override)?;
    let rules = resolve_narrative_rules(&policy, rules_override)?;
    let store = load_atomic_store(workspace_root, sidecar)?;
    let order = compose_canon_order(&decl, &store)?;
    mnemosyne_validate::continuity::timeline_gaps(&store, &order, &rules.rules)
        .map_err(OpError::Other)
}

/// Import succession + conflict edges from a reviewed `edge-proposals/v1`
/// artifact (Round 463, design sec 7.16 Round B) — load + shape-check the
/// file, then run the all-or-nothing import (or its dry-run twin). Returns
/// the full verdict report; gating policy stays with the caller (the
/// import_typing_proposals_report shape).
pub fn import_edge_proposals_report(
    workspace_root: &Path,
    sidecar: Option<&Path>,
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
    sidecar: Option<&Path>,
    order_override: Option<&str>,
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
    sidecar: Option<&Path>,
    order_override: Option<&str>,
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
    sidecar: Option<&Path>,
    world: Option<&str>,
    order_override: Option<&str>,
    telling: Option<&str>,
    reading_walk: bool,
) -> Result<mnemosyne_validate::continuity::PlaythroughManuscriptReport, OpError> {
    let policy = continuity_policy(workspace_root)?;
    let decl = resolve_canon_order_file(&policy, order_override)?;
    let store = load_atomic_store(workspace_root, sidecar)?;
    let order = compose_canon_order(&decl, &store)?;
    let mut report =
        mnemosyne_validate::continuity::playthrough_manuscript(&store, &order, world, telling)
            .map_err(OpError::Other)?;
    // Round 509 — the reading-walk projection: prune each world to its
    // content scenes (those where a world-visible fact begins). The structural
    // manuscript (the verb default) keeps every order node; a READING copy
    // wants only the scenes that introduce content (the R500 begins>0
    // convention). A deterministic, in-code prune replaces the orchestrator's
    // hand-made `.filtered` files (the harness debt R505 flagged), so the next
    // blind run produces per-world reading copies without manual surgery.
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
    sidecar: Option<&Path>,
    order_override: Option<&str>,
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
    sidecar: Option<&Path>,
    world: Option<&str>,
    order_override: Option<&str>,
    telling: &str,
) -> Result<mnemosyne_validate::continuity::PlayableWorldReport, OpError> {
    let policy = continuity_policy(workspace_root)?;
    let decl = resolve_canon_order_file(&policy, order_override)?;
    let store = load_atomic_store(workspace_root, sidecar)?;
    let order = compose_canon_order(&decl, &store)?;
    mnemosyne_validate::continuity::playable_world(&store, &order, world, telling)
        .map_err(OpError::Other)
}

/// Project the quest graph (Round 559 design sec 7.38, Round 568 build) over the
/// workspace store with the shared order resolution — the fact→quest leg a
/// pinion narrative runtime (or an authoring consumer) consumes: per telling,
/// each `Entity{kind:"quest"}` projected to a `QuestNode` (objective, actor,
/// per-world derived open/done state, prerequisites, completion fact, giver
/// surface locator). A pure JOIN over the existing payoff-coverage (R442) and
/// playable-world (R557) projections; pure read, never gated. `world` filters
/// the per-world map (the fork tree stays full).
pub fn quest_graph_report(
    workspace_root: &Path,
    sidecar: Option<&Path>,
    world: Option<&str>,
    order_override: Option<&str>,
    telling: &str,
) -> Result<mnemosyne_validate::continuity::QuestGraphReport, OpError> {
    let policy = continuity_policy(workspace_root)?;
    let decl = resolve_canon_order_file(&policy, order_override)?;
    let store = load_atomic_store(workspace_root, sidecar)?;
    let order = compose_canon_order(&decl, &store)?;
    mnemosyne_validate::continuity::quest_graph(&store, &order, world, telling)
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
    sidecar: Option<&Path>,
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
    sidecar: Option<&Path>,
    against: &Path,
    order_override: Option<&str>,
    telling: &str,
    world: &str,
    truth_frame: &str,
) -> Result<mnemosyne_validate::disclosure::DisclosureLeakReport, OpError> {
    let policy = continuity_policy(workspace_root)?;
    let decl = resolve_canon_order_file(&policy, order_override)?;
    let authored = load_atomic_store(workspace_root, sidecar)?;
    let order = compose_canon_order(&decl, &authored)?;
    if !mnemosyne_core::is_known_world(&authored.branches, world) {
        return Err(OpError::Other(format!(
            "world `{world}` not present in the branch registry (fail-loud)"
        )));
    }
    if !authored.frames.contains_key(truth_frame) {
        return Err(OpError::Other(format!(
            "truth_frame `{truth_frame}` not present in the frame registry (fail-loud)"
        )));
    }
    let reextracted = AtomicStore::load(against).map_err(|e| OpError::Other(format!("{}", e)))?;
    mnemosyne_validate::disclosure::disclosure_leak(
        &authored,
        &reextracted,
        &order,
        telling,
        world,
        truth_frame,
    )
    .map_err(OpError::Other)
}

/// Render↔world-line fidelity gate (Round 507, design sec 7.24 step 6, R505) —
/// the BLIND RE-EXTRACTED prose store (`against`) checked against `world`'s
/// composed order (the prose analog of R488). Guards `world` against the branch
/// registry before running.
pub fn render_fidelity_report(
    workspace_root: &Path,
    sidecar: Option<&Path>,
    against: &Path,
    order_override: Option<&str>,
    world: &str,
) -> Result<mnemosyne_validate::disclosure::RenderFidelityReport, OpError> {
    let policy = continuity_policy(workspace_root)?;
    let decl = resolve_canon_order_file(&policy, order_override)?;
    let authored = load_atomic_store(workspace_root, sidecar)?;
    let order = compose_canon_order(&decl, &authored)?;
    if !mnemosyne_core::is_known_world(&authored.branches, world) {
        return Err(OpError::Other(format!(
            "world `{world}` not present in the branch registry (fail-loud)"
        )));
    }
    let reextracted = AtomicStore::load(against).map_err(|e| OpError::Other(format!("{}", e)))?;
    Ok(mnemosyne_validate::disclosure::render_fidelity(
        &reextracted,
        &order,
        world,
    ))
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
    sidecar: Option<&Path>,
    entity_id: &str,
) -> Result<EntityDossier, OpError> {
    let store = load_atomic_store(workspace_root, sidecar)?;
    let id = entity_id.trim();
    let Some(entity) = store.entities.get(id) else {
        return Err(OpError::Other(format!(
            "entity `{id}` not present in the entity registry (fail-loud — a typo'd \
             entity must not read as an empty dossier)"
        )));
    };
    let facts: Vec<EntityFactRow> = store
        .narrative_facts
        .iter()
        .filter(|(_, f)| f.entities.iter().any(|e| e == id))
        .map(|(fid, f)| EntityFactRow {
            fact_id: fid.clone(),
            frame: f.frame.clone(),
            branch: f.branch.clone(),
            claim: f.claim.clone(),
            canon_from: f.canon_from.clone(),
            canon_to: f.canon_to.clone(),
            evidence: f.evidence.clone(),
            typed: f.typed.clone(),
        })
        .collect();
    Ok(EntityDossier {
        entity_id: id.to_string(),
        kind: entity.kind.clone(),
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
    sidecar: Option<&Path>,
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
    sidecar: Option<&Path>,
    entry_id: &str,
    reason: &str,
    applied_in: &str,
    kind: Option<&str>,
) -> Result<Option<String>, OpError> {
    let sidecar_path = resolve_sidecar(workspace_root, sidecar)?;
    let store = AtomicStore::load(&sidecar_path).map_err(|e| OpError::Other(format!("{}", e)))?;
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

    /// No config file = nothing to scan = an empty hit set, not an error.
    #[test]
    fn inventory_decay_scan_missing_config_is_empty_not_error() {
        let tmp = TempDir::new().unwrap();
        let hits = inventory_decay_scan(tmp.path(), "X").expect("missing config = empty");
        assert!(hits.is_empty());
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
            frames: vec![],
            branches: vec![],
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
        assert_eq!(bad.violations[0].source, "shape");

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
        let r = authoring_frontier_report(ws.path(), None, None, None).unwrap();
        assert_eq!(r.zero_fact_scenes, vec!["sc-2".to_string()]);
        let counts: std::collections::BTreeMap<_, _> = r
            .scene_coverage
            .iter()
            .map(|s| (s.scene.as_str(), s.fact_count))
            .collect();
        assert_eq!(counts["sc-1"], 1);
        assert_eq!(counts["sc-2"], 0);
        // The canon order (canon.json edges sc-1 -> sc-2) covers the fact-bearing
        // sc-1, so nothing is unordered (Round 596).
        assert!(r.unordered_scenes.is_empty());
        assert_eq!(r.total_gaps, 1); // just the one zero-fact scene
                                     // Telling-scoped sections are omitted without a telling.
        assert!(r.telling.is_none());
        assert!(r.unresolved_quests.is_none());
        assert!(r.never_planned_disclosures.is_none());
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
        let r = authoring_frontier_report(root, None, None, None).unwrap();
        // sc-1 carries a fact but the order places nothing -> unordered.
        assert_eq!(r.unordered_scenes, vec!["sc-1".to_string()]);
        // sc-2 is zero-fact (a distinct gap) but not fact-bearing, so not unordered.
        assert_eq!(r.zero_fact_scenes, vec!["sc-2".to_string()]);
        assert_eq!(r.total_gaps, 2); // one zero-fact + one unordered
    }

    /// Round 617 (MNEMO-GAP-005 part 3b): branch-owned density counts a world-line's
    /// facts over the road it DECLARED, so a divergent world that looks full by
    /// inheritance but owns little is visible. Locks the two traps the gap's
    /// design tripped: a shared-fork-coordinate fact inflates `owned_facts` but
    /// NOT `own_scene_facts` (road-consistent numerator, no inflation), and an
    /// undeclared-road world reports `density: None` (rides the trunk), never a
    /// divide-by-zero.
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
        // main trunk sc-1 -> sc-2 -> sc-3; `end` forks at sc-2 with its OWN scene
        // sc-end; `ghost` forks at sc-1 declaring NO road (undeclared, rides trunk).
        std::fs::write(
            root.join("canon.json"),
            r#"{"edges":[["sc-1","sc-2"],["sc-2","sc-3"]],
               "branches":{"end":[["sc-2","sc-end"]]}}"#,
        )
        .unwrap();
        std::fs::write(
            root.join("store.json"),
            r#"{"schema_version":23,
               "sections":{"sc-1":{},"sc-2":{},"sc-3":{},"sc-end":{}},
               "frames":{"gt":{}},
               "branches":{"end":{"forks_from":{"branch":"main","at":"sc-2"}},
                           "ghost":{"forks_from":{"branch":"main","at":"sc-1"}}},
               "narrative_facts":{
                 "f-m1":{"frame":"gt","claim":"c","canon_from":"sc-1","evidence":["sc-1"]},
                 "f-m2":{"frame":"gt","claim":"c","canon_from":"sc-2","evidence":["sc-2"]},
                 "f-m3":{"frame":"gt","claim":"c","canon_from":"sc-3","evidence":["sc-3"]},
                 "f-e1":{"frame":"gt","branch":"end","claim":"c","canon_from":"sc-end","evidence":["sc-end"]},
                 "f-e2":{"frame":"gt","branch":"end","claim":"c","canon_from":"sc-2","evidence":["sc-2"]},
                 "f-g1":{"frame":"gt","branch":"ghost","claim":"c","canon_from":"sc-1","evidence":["sc-1"]}}}"#,
        )
        .unwrap();
        let r = authoring_frontier_report(root, None, None, None).unwrap();
        let d = &r.branch_owned_density;

        // main (root) owns its whole trunk {sc-1,sc-2,sc-3}: 3 facts / 3 scenes.
        let m = &d["main"];
        assert_eq!(
            (m.owned_facts, m.own_scene_facts, m.own_road_scenes),
            (3, 3, 3)
        );
        assert_eq!(m.density, Some(1.0));
        assert!(m.own_road_declared);

        // `end` authored 2 facts but only ONE is on its own scene (sc-end); the
        // fact at the shared fork coordinate sc-2 is inherited ground, excluded
        // from the numerator — no inflation (the R613 dual-axis trap).
        let end = &d["end"];
        assert_eq!(
            (end.owned_facts, end.own_scene_facts, end.own_road_scenes),
            (2, 1, 1)
        );
        assert_eq!(end.density, Some(1.0));
        assert!(end.own_road_declared);

        // `ghost` declares no road: honest None, never a divide-by-zero.
        let g = &d["ghost"];
        assert_eq!(
            (g.owned_facts, g.own_scene_facts, g.own_road_scenes),
            (1, 0, 0)
        );
        assert_eq!(g.density, None);
        assert!(!g.own_road_declared);

        // density is a pure read: it does NOT feed the gap gauge.
        assert_eq!(r.total_gaps, 0);
    }
}
