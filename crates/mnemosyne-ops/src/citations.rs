//! THE CITATION GATE, RUN FROM ONE PLACE.
//!
//! Until Round 1333 the whole construction of a citation-gate run — resolve the
//! scope, read `[plugins.set_equality_validator]`, apply severity precedence,
//! load the store, build the validator, derive the attribution, walk, read the
//! verdicts, count by axis — lived inline in `mnemosyne-cli`'s
//! `cmd_validate_code_refs`, between its flag parsing and its two report
//! writers. Any second surface wanting the same answer had to build that
//! sequence again, and a sequence built twice is two answers waiting to
//! disagree about which severities were in force, which store was read, or what
//! the scope selected. Rounds 1322 to 1324 are what that costs when it is a
//! grammar; this is the same shape one level up.
//!
//! WHAT THIS OWNS: everything between a loaded config and the numbers a report
//! is written from. WHAT IT DOES NOT: how a caller spells its flags, how it
//! prints, and what it does about the answer. The gate's verdict reaches a
//! consumer as data; whether that data refuses a commit is the consumer's, which
//! is the line Rounds 481 to 488 drew and this module does not cross.
//!
//! THE SYMBOL RESOLVERS ARE HANDED IN, not built here. The table of backends a
//! build ships lives with the binary that ships them, because it is a statement
//! about that build rather than about this operation. A caller that ships none
//! gets a run whose symbol axis reports `no resolver` by name — which is the
//! answer, not a silence, and is why this signature can afford to take them.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use mnemosyne_atomic::AtomicStore;
use mnemosyne_config::{LoadedConfig, Severity};
use mnemosyne_validate::code_refs::{
    numbering_origin_coverage, vcs_ignored_among, AuditAxis, AxisVerdicts, CitationAttribution,
    CitationReaderCoverage, CitationSite, CodeRefViolation, NumberingOriginAxis,
    NumberingOriginReport, PathScope, PathScopeCoverage, ProposedImplementation,
    SetEqualityValidator, SymbolAxisCoverage, VcsIgnoreAxis,
};

use crate::OpError;

/// A `--severity-*` override per axis, already parsed. `None` leaves the
/// configured value in force.
///
/// Parsing belongs to the caller because the spelling does: a CLI flag and an
/// agent-facing argument are different vocabularies for the same axis, and the
/// one thing they must share is what the value MEANS once parsed.
#[derive(Debug, Clone, Copy, Default)]
pub struct CitationSeverityOverrides {
    /// `missing` — a citation naming an entry the store does not hold.
    pub missing: Option<Severity>,
    /// `binding` — a section citation with no backing binding.
    pub binding: Option<Severity>,
    /// `coverage` — implementation/verification coverage of a section.
    pub coverage: Option<Severity>,
    /// `verification` — the opt-in verify axis; an override ENABLES it.
    pub verification: Option<Severity>,
    /// `classification` — the opt-in misclassified-coverage axis.
    pub classification: Option<Severity>,
    /// `blanket` — the opt-in blanket-verifies axis.
    pub blanket: Option<Severity>,
    /// `inventory` — the inventory citation lifecycle axes.
    pub inventory: Option<Severity>,
}

/// The severities a run actually enforced, after `override > config` and the
/// coverage axis's inheritance from `binding`.
#[derive(Debug, Clone, Copy)]
pub struct ResolvedSeverities {
    /// See [`CitationSeverityOverrides::missing`].
    pub missing: Severity,
    /// See [`CitationSeverityOverrides::binding`].
    pub binding: Severity,
    /// Inherits `binding` when neither a flag nor the config names it.
    pub coverage: Severity,
    /// `None` = the axis is off for this run.
    pub verification: Option<Severity>,
    /// `None` = the axis is off for this run.
    pub classification: Option<Severity>,
    /// `None` = the axis is off for this run.
    pub blanket: Option<Severity>,
    /// See [`CitationSeverityOverrides::inventory`].
    pub inventory: Severity,
    /// Config-only; no override spelling exists yet.
    pub prose_fact_assertion: Option<Severity>,
}

/// What a caller asks the gate for.
pub struct CitationScanRequest<'a> {
    /// The workspace whose `mnemosyne.toml` was loaded — the op never discovers
    /// one, so a caller holding a workspace cannot be handed another's answer.
    pub loaded: &'a LoadedConfig,
    /// The `Round NNN` prefix citations are read under.
    pub entry_id_prefix: String,
    /// Files or directories to narrow the run to. `None` is an unscoped run;
    /// `Some(&[])` is a caller that asked for a scope and named nothing, which
    /// is REFUSED rather than widened — the two are different requests and the
    /// second is malformed. Collapsing them would let a mis-spelled invocation
    /// come back as a clean whole-tree report, which is the reading this
    /// distinction exists to prevent.
    ///
    /// A scoped run reports every tree-wide axis by NAME rather than as zero,
    /// which is [`AxisVerdicts`]'s doing and not this module's.
    pub scope_paths: Option<&'a [String]>,
    /// Restrict the run to one entry id's decay axis.
    pub filter_id: Option<String>,
    /// Per-axis severity overrides.
    pub severities: CitationSeverityOverrides,
    /// The symbol resolvers this build ships, by language.
    pub symbol_resolvers: BTreeMap<String, Box<dyn mnemosyne_core::SymbolResolver>>,
}

/// The answer, including the case where there is nothing to answer with.
pub enum CitationScan {
    /// `[plugins.set_equality_validator]` is absent, so this workspace has not
    /// asked for the gate. A separate variant rather than an empty report,
    /// because "nobody configured this" and "configured and clean" are the two
    /// readings a consumer must never confuse.
    NotConfigured,
    /// The run happened; here is everything a report is written from.
    Ran(Box<CitationScanReport>),
}

/// Everything one gate run produced.
///
/// Every field is what a single walk observed. Two of them are deliberately
/// paired: `counts` is what each axis EMITTED and `measured` is what each axis
/// was ASKED — `None` there means the axis did not judge, and a consumer that
/// prints `0` for it has told someone their tree is clean about a question
/// nobody asked (Round 1141).
/// How much the store this run judged against held.
///
/// Read from the ONE store load the run made, so a report's "valid entries"
/// line and its verdicts cannot be about two different reads of the sidecar.
#[derive(Debug, Clone, Copy)]
pub struct StoreCounts {
    /// Changelog entries.
    pub changelog_entries: usize,
    /// Spec sections.
    pub sections: usize,
    /// Inventory entries.
    pub inventory_entries: usize,
}

pub struct CitationScanReport {
    /// The gate section this run was configured by, as the workspace declared
    /// it — without the opt-in severities an override injects, which the
    /// validator got and a report writer must not read back as configuration.
    pub config: mnemosyne_config::SetEqualityValidatorConfig,
    /// The workspace root the walk was rooted at.
    pub root: PathBuf,
    /// Every violation, in walk order.
    pub violations: Vec<CodeRefViolation>,
    /// What each declared citation reader reached.
    pub reader_axis: Vec<CitationReaderCoverage>,
    /// Which axes judged, and the named reason for each that did not.
    pub verdicts: AxisVerdicts,
    /// Violations per axis. Absent key = none emitted.
    pub counts: BTreeMap<AuditAxis, usize>,
    /// Per axis: `Some(n)` measured, `None` not judged.
    pub measured: BTreeMap<AuditAxis, Option<usize>>,
    /// What the symbol axis could not reach.
    pub symbol_axis: SymbolAxisCoverage,
    /// The files this run read.
    pub read_files: Vec<PathBuf>,
    /// What a `--paths` scope selected and what it did not reach; `None` for an
    /// unscoped run.
    pub path_scope: Option<PathScopeCoverage>,
    /// What the tree's own VCS calls build output inside the read set.
    pub vcs_axis: VcsIgnoreAxis,
    /// Which read files speak another document's numbering.
    pub numbering_origin: NumberingOriginReport,
    /// `verifies` bindings whose claims are not yet Confirmed. Standing and
    /// informational: independent of every severity knob, because an
    /// existence-green gate that hides a semantic gap breeds complacency.
    pub unconfirmed_verifies: usize,
    /// How much the judged store held.
    pub store_counts: StoreCounts,
    /// The advisory fact/entity citation axis. Produced every run, including
    /// when it covers nothing: a store with no facts and a store whose every
    /// citation lands produce the same silence otherwise.
    pub id_citations: mnemosyne_validate::code_refs::IdCiteReport,
    /// The severities this run enforced.
    pub resolved: ResolvedSeverities,
}

/// Run the citation gate over one workspace.
///
/// # Errors
///
/// A scope naming a path outside the workspace, a sidecar that cannot be
/// resolved or loaded, a walk that fails, or an internal disagreement between
/// what an axis said it judged and what it emitted.
pub fn scan_citations(request: CitationScanRequest) -> Result<CitationScan, OpError> {
    let CitationScanRequest {
        loaded,
        entry_id_prefix,
        scope_paths,
        filter_id,
        severities,
        symbol_resolvers,
    } = request;

    // Before anything else is decided: an unusable scope (empty entry, or a
    // path outside the workspace) is a malformed request, and a malformed
    // request must not be able to come back as this run's "not configured" or
    // as a clean report.
    let path_scope = match scope_paths {
        None => None,
        Some(requested) => Some(PathScope::new(&loaded.workspace_root, requested)?),
    };

    let Some(cfg) = loaded
        .config
        .plugins
        .as_ref()
        .and_then(|p| p.set_equality_validator.as_ref())
    else {
        return Ok(CitationScan::NotConfigured);
    };

    // Severity precedence: an override wins over the config. The coverage axis
    // inherits the RESOLVED binding severity when neither names it, so a
    // workspace that only sets `severity_binding` keeps gating coverage with it.
    let missing = severities.missing.unwrap_or(cfg.severity_missing);
    let binding = severities.binding.unwrap_or(cfg.severity_binding);
    let coverage = severities
        .coverage
        .or(cfg.severity_coverage)
        .unwrap_or(binding);
    // The three opt-in axes: an override ENABLES one for this run, which is why
    // it is injected into the validator's config below — the scan only emits
    // their violations when the config holds a severity for them.
    let verification = severities.verification.or(cfg.severity_verification);
    let classification = severities.classification.or(cfg.severity_classification);
    let blanket = severities.blanket.or(cfg.severity_blanket);
    let inventory = severities.inventory.unwrap_or(cfg.severity_inventory);
    let resolved = ResolvedSeverities {
        missing,
        binding,
        coverage,
        verification,
        classification,
        blanket,
        inventory,
        prose_fact_assertion: cfg.severity_prose_fact_assertion,
    };

    let root = loaded.workspace_root.clone();
    // Sidecar resolution discovers config from the anchor (the toml's dir), not
    // the resolved root, so a subdir-rooted ledger finds its `[atomic]`.
    let anchor = loaded
        .config_path
        .parent()
        .map_or_else(|| root.clone(), Path::to_path_buf);
    let atomic_path = crate::cascade::resolve_sidecar(&anchor, None)?;
    let store = AtomicStore::load(&atomic_path).map_err(|e| {
        OpError::Other(format!("atomic store load: {}: {e}", atomic_path.display()))
    })?;

    let mut validator_cfg = cfg.clone();
    validator_cfg.severity_verification = verification;
    validator_cfg.severity_classification = classification;
    validator_cfg.severity_blanket = blanket;
    let validator = SetEqualityValidator {
        config: validator_cfg,
        entry_id_prefix,
        orphan_ledger: loaded.config.orphan_ledger.clone(),
        symbol_resolvers,
        filter_id,
        path_scope,
    };

    // The numbering origin derived ONCE and handed to the scan and to the
    // advisory axes alike, so no two of them can disagree about whose numbering
    // a file speaks.
    let attribution = CitationAttribution::new(&root, cfg, NumberingOriginAxis::derive(&root));
    let snapshot = mnemosyne_core::AtomicStoreView::snapshot(&store);
    // One walk, both answers: the judgments and what the citation readers
    // reached while making them, so a report cannot be about a different
    // reading of the tree than the verdicts are.
    let (violations, reader_axis) = validator.scan_and_reach(&attribution, &snapshot)?;
    let verdicts = validator.axis_verdicts();
    let symbol_axis = validator.symbol_axis_coverage(&attribution, &snapshot)?;
    let read_files = validator.read_set(&root)?;
    // Measured against the UNSCOPED walk, so `out_of_read_set` can distinguish
    // "this gate never reads that file" from "that file is clean".
    let path_scope_coverage = validator.scope_coverage(&root)?;
    let read_set: BTreeSet<PathBuf> = read_files.iter().cloned().collect();
    let vcs_axis = vcs_ignored_among(&root, &read_set);
    let numbering_origin = numbering_origin_coverage(&attribution, &read_set);
    // Read from the SAME store this run judged against. Loading it a second
    // time would open the window Round 1327 closed one axis over: two reads of
    // one file can disagree, and then the report is about a tree the verdicts
    // are not.
    let catalog = match loaded.config.verifies_catalog.as_ref() {
        None => None,
        Some(c) => Some(
            mnemosyne_validate::verifies_linkage::load_catalog(
                &root.join(&c.path),
                c.sha256.as_deref(),
            )
            .map_err(|e| OpError::Other(e.to_string()))?,
        ),
    };
    let unconfirmed_verifies = mnemosyne_validate::confirmation::scan_confirmation_gate(
        &snapshot,
        &store,
        &root,
        catalog.as_ref(),
    )
    .len();
    let store_counts = StoreCounts {
        changelog_entries: store.changelog_entries.len(),
        sections: store.sections.len(),
        inventory_entries: store.inventory_entries.len(),
    };
    let id_citations = mnemosyne_validate::code_refs::scan_id_citations(
        &root,
        &read_files,
        cfg.comment_only,
        &store
            .narrative_facts
            .keys()
            .map(ToString::to_string)
            .collect(),
        &store.entities.keys().map(ToString::to_string).collect(),
    );

    let mut counts = BTreeMap::<AuditAxis, usize>::new();
    for v in &violations {
        *counts.entry(v.axis()).or_insert(0) += 1;
    }
    // A count exists only where a measurement happened. An axis this run did not
    // judge reports its NAME and its reason, never `0` — zero is the answer a
    // clean tree gives, and a mode that borrows it has told the consumer their
    // tree is clean.
    let measured: BTreeMap<AuditAxis, Option<usize>> = verdicts
        .iter()
        .map(|(axis, reason)| {
            (
                axis,
                match reason {
                    None => Some(counts.get(&axis).copied().unwrap_or(0)),
                    Some(_) => None,
                },
            )
        })
        .collect();
    // The map claims a skip; the violations are what actually happened. If an
    // axis reported "not judged" emitted anything, one of the two is lying and
    // the run must say so rather than return a report that reads consistent.
    for (axis, count) in &counts {
        if measured.get(axis).copied().flatten().is_none() {
            return Err(OpError::Other(format!(
                "internal: axis `{}` was reported as not judged but emitted {count} violation(s)",
                axis.kind_tag()
            )));
        }
    }

    Ok(CitationScan::Ran(Box::new(CitationScanReport {
        config: cfg.clone(),
        root,
        violations,
        reader_axis,
        verdicts,
        counts,
        measured,
        symbol_axis,
        read_files,
        path_scope: path_scope_coverage,
        vcs_axis,
        numbering_origin,
        unconfirmed_verifies,
        store_counts,
        id_citations,
        resolved,
    })))
}

/// What a READING of the citation graph needs from a workspace.
///
/// The gate has two questions and only one of them is a verdict. `scan_citations`
/// answers "is anything wrong here"; these answer "WHERE are the citations" —
/// the same distinction the port arc turned on (Rounds 1328-1332), one level up.
/// A reading takes no severities, no scope and no filter, because none of those
/// change where a citation IS.
pub struct CitationReadRequest<'a> {
    /// The workspace whose `mnemosyne.toml` was loaded.
    pub loaded: &'a LoadedConfig,
    /// The `Round NNN` prefix citations are read under.
    pub entry_id_prefix: String,
    /// The symbol resolvers this build ships, by language.
    pub symbol_resolvers: BTreeMap<String, Box<dyn mnemosyne_core::SymbolResolver>>,
}

/// Build the validator a reading needs and hand it, its attribution and the
/// store's snapshot to `read`.
///
/// THE ONE ASSEMBLY FOR A READING, for the reason `scan_citations` exists for a
/// verdict: two call sites writing this sequence are two answers waiting to
/// disagree about which store was read and under which prefix. It is private
/// because a caller wanting a different reading should add a verb here rather
/// than take the parts away and combine them elsewhere.
///
/// `Ok(None)` means the workspace never configured the gate — the same variant
/// `scan_citations` returns, and for the same reason: "nobody asked for this"
/// and "asked for it and it is clean" must not read alike.
fn with_reading<T>(
    request: CitationReadRequest,
    read: impl FnOnce(
        &SetEqualityValidator,
        &CitationAttribution,
        &mnemosyne_core::AtomicSnapshot,
    ) -> std::io::Result<T>,
) -> Result<Option<T>, OpError> {
    let CitationReadRequest {
        loaded,
        entry_id_prefix,
        symbol_resolvers,
    } = request;
    let Some(cfg) = loaded
        .config
        .plugins
        .as_ref()
        .and_then(|p| p.set_equality_validator.as_ref())
    else {
        return Ok(None);
    };
    let root = loaded.workspace_root.clone();
    let anchor = loaded
        .config_path
        .parent()
        .map_or_else(|| root.clone(), Path::to_path_buf);
    let atomic_path = crate::cascade::resolve_sidecar(&anchor, None)?;
    let store = AtomicStore::load(&atomic_path).map_err(|e| {
        OpError::Other(format!("atomic store load: {}: {e}", atomic_path.display()))
    })?;
    let validator = SetEqualityValidator {
        config: cfg.clone(),
        entry_id_prefix,
        orphan_ledger: loaded.config.orphan_ledger.clone(),
        symbol_resolvers,
        filter_id: None,
        path_scope: None,
    };
    let attribution = CitationAttribution::new(&root, cfg, NumberingOriginAxis::derive(&root));
    let snapshot = mnemosyne_core::AtomicStoreView::snapshot(&store);
    Ok(Some(read(&validator, &attribution, &snapshot)?))
}

/// Which files cite each section, and where.
///
/// # Errors
///
/// A sidecar that cannot be resolved or loaded, or a walk that fails.
pub fn citation_index(
    request: CitationReadRequest,
) -> Result<Option<BTreeMap<String, Vec<CitationSite>>>, OpError> {
    with_reading(request, |validator, attribution, snapshot| {
        validator.citation_index(attribution, snapshot)
    })
}

/// Which sections a file already cites strongly enough to propose an
/// implementation binding for.
///
/// # Errors
///
/// A sidecar that cannot be resolved or loaded, or a walk that fails.
pub fn propose_implementations(
    request: CitationReadRequest,
) -> Result<Option<Vec<ProposedImplementation>>, OpError> {
    with_reading(request, |validator, attribution, snapshot| {
        validator.propose_implementations(attribution, snapshot)
    })
}
