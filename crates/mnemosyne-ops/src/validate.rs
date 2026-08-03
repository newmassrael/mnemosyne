//! `validate-workspace` library op. Encapsulates the multi-step T1/T2/
//! round-trip/atomic-ledger pipeline as a single function that returns a
//! structured report. The CLI bin pretty-prints, the MCP server
//! serializes to JSON.

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::Context;
use mnemosyne_config::OrphanKind;
use mnemosyne_style::{
    check_style_atomic, default_ruleset_with_config, StyleSeverity, StyleViolation,
};
use mnemosyne_validate::{validator::scan_store_prose_cross_ref_orphans, ValidationError};
use serde::Serialize;

use crate::cascade::validate_atomic_store;
use crate::{query::load_workspace, OpError, PopulationCensusReport};

#[derive(Debug, Clone, Serialize)]
pub struct ValidateWorkspaceReport {
    pub orphan_actual: Vec<OrphanRef>,
    pub orphan_ledger: Vec<OrphanRef>,
    pub orphan_new: Vec<OrphanRef>,
    pub orphan_resolved: Vec<OrphanRef>,
    pub atomic_entries: usize,
    pub atomic_sections: usize,
    pub atomic_orphan_entry_refs: usize,
    pub atomic_orphan_section_refs: usize,
    pub atomic_new_entries: Vec<(String, String)>,
    pub atomic_resolved_entries: Vec<(String, String)>,
    pub atomic_new_sections: Vec<(String, String)>,
    pub atomic_resolved_sections: Vec<(String, String)>,
    pub style_t3_reject: usize,
    pub style_t3_warn: usize,
    pub style_t4_info: usize,
    pub style_t3_reject_messages: Vec<String>,
    pub supersede_violations: Vec<String>,
    pub publishable_divergence: usize,
    pub publishable_ledger_rows: usize,
    pub publishable_unmatched: Vec<String>,
    pub store_registry_violations: usize,
    pub scan_considered: usize,
    pub scan_scanned: usize,
    pub scan_unscanned: Vec<String>,
    pub scan_stale_exclusions: Vec<String>,
    /// Round 860 — files a configured path covers that a declared exclusion also
    /// names. ADVISORY: the config contradicts itself and the exclusion is the
    /// half that cannot win, but no existing consumer's gate weakens because of
    /// it, so naming it beats failing a workspace that has been correct for
    /// rounds.
    pub scan_excluded_but_scanned: Vec<String>,
    /// Round 854 — the files the Round 840 axis actually read, any language: the
    /// ones the gate covers and the ones an exclusion removed. Reported every
    /// run, because "0 swallowed" out of 0 excluded files read is the same clean
    /// a genuinely clean tree prints (the Round 783 non-vacuity rule), and the
    /// axis was silently Rust-only for a whole consumer's C++ tree.
    pub scan_gate_files: usize,
    pub scan_excluded_files: usize,
    /// Round 866 — what the tree's own VCS calls build output INSIDE the
    /// excluded set. The swallowed answer above is read out of that set, so a
    /// set that differs between a developer and CI makes the answer differ too.
    /// Round 864 asked this of the read set and left the excluded one open; a
    /// consumer ledger measures 9591 of 13685 here. Advisory, and reported in
    /// all three states for the Round 856 reason.
    pub scan_excluded_vcs: mnemosyne_validate::code_refs::VcsIgnoreAxis,
    /// Round 840 — citations an exclusion removed from the gate entirely.
    pub scan_swallowed_citations: Vec<String>,
    /// Round 867 — which subtrees the tree's own VCS says belong to ANOTHER
    /// repository, and how many citations of this store that removed from both
    /// sides of the swallowed answer. `None` = no citation-gate config, so there
    /// is nothing to attribute. Advisory, and LOUD BY OBLIGATION: every sibling
    /// axis tightens and this one loosens, so a wrong verdict here un-gates real
    /// citations, and the count is what keeps that from being silent.
    pub scan_numbering_origin: Option<mnemosyne_validate::code_refs::NumberingOriginReport>,
    /// Round 979 — census rows recorded by entries this commit is ADDING that
    /// do not match the workspace's report. Empty on any tree where the answer
    /// cannot be known (see [`census_contemporaneity`]).
    pub census_stale: Vec<String>,
    /// Round 980 — what the contemporaneity check was able to look at. Reported
    /// on every run, because an empty `census_stale` is the same clean a
    /// workspace gets when the check never ran.
    pub census_reach: CensusReach,
    /// Round 983 — whether this workspace's entry ids are required to carry the
    /// number that dates every count in an entry's prose.
    pub entry_id_dating: EntryIdDating,
    pub failed: bool,
    pub failure_reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OrphanRef {
    pub doc: String,
    pub from_section: String,
    pub to_target: String,
}

/// Run the full validate-workspace pipeline as a pure function. Does not
/// print; returns the structured report. `failed = true` when at least
/// one bail condition is hit (round-trip break, new orphan, resolved
/// ledger entry, T3 reject).
pub fn validate_workspace(workspace_root: &Path) -> Result<ValidateWorkspaceReport, OpError> {
    let (loaded, atomic_store) = load_workspace(workspace_root).map_err(OpError::from)?;

    // Store-direct cross-ref orphan scan: free-prose §N references resolved
    // against the store (the SSOT). Orphan-ledger keys carry a stable
    // "atomic-store" doc label.
    let orphan_doc_label = "atomic-store".to_string();
    let mut actual_orphan_keys: BTreeSet<(String, String, String)> = BTreeSet::new();
    for (from_section, to_target) in scan_store_prose_cross_ref_orphans(&atomic_store) {
        actual_orphan_keys.insert((orphan_doc_label.clone(), from_section, to_target));
    }

    let mut known_orphan_keys: BTreeSet<(String, String, String)> = BTreeSet::new();
    for entry in &loaded.config.orphan_ledger {
        if entry.kind != OrphanKind::MarkdownRef {
            continue;
        }
        known_orphan_keys.insert((entry.doc.clone(), entry.from.clone(), entry.to.clone()));
    }
    let orphan_new: Vec<OrphanRef> = actual_orphan_keys
        .difference(&known_orphan_keys)
        .map(|(d, f, t)| OrphanRef {
            doc: d.clone(),
            from_section: f.clone(),
            to_target: t.clone(),
        })
        .collect();
    let orphan_resolved: Vec<OrphanRef> = known_orphan_keys
        .difference(&actual_orphan_keys)
        .map(|(d, f, t)| OrphanRef {
            doc: d.clone(),
            from_section: f.clone(),
            to_target: t.clone(),
        })
        .collect();
    let orphan_actual: Vec<OrphanRef> = actual_orphan_keys
        .iter()
        .map(|(d, f, t)| OrphanRef {
            doc: d.clone(),
            from_section: f.clone(),
            to_target: t.clone(),
        })
        .collect();
    let orphan_ledger_view: Vec<OrphanRef> = known_orphan_keys
        .iter()
        .map(|(d, f, t)| OrphanRef {
            doc: d.clone(),
            from_section: f.clone(),
            to_target: t.clone(),
        })
        .collect();

    // Style violations.
    let ruleset = default_ruleset_with_config(
        loaded.config.style.as_ref(),
        loaded.config.terminology.as_ref(),
    );
    // Store-direct style: iterate the atomic store (the SSOT). Violations
    // carry a stable "atomic-store" doc label.
    let style_violations: Vec<StyleViolation> =
        check_style_atomic("atomic-store", &atomic_store, &ruleset);
    let terminology_violations: Vec<&StyleViolation> = style_violations
        .iter()
        .filter(|v| v.rule_id == "terminology_consistency")
        .collect();
    let t3_total = style_violations
        .iter()
        .filter(|v| v.severity == StyleSeverity::Warn)
        .count();
    let t4_count = style_violations
        .iter()
        .filter(|v| v.severity == StyleSeverity::Info)
        .count();
    let t3_reject_count = terminology_violations.len();
    let t3_warn_count = t3_total - t3_reject_count;
    let t3_reject_messages: Vec<String> = terminology_violations
        .iter()
        .map(|v| format!("{}: §{} — {}", v.doc_path, v.section_id, v.message))
        .collect();

    // Atomic store ledger.
    let id_set = atomic_store.atomic_section_id_set();
    let atomic = validate_atomic_store(workspace_root, &id_set)
        .with_context(|| "validate_atomic_store")
        .map_err(|e| OpError::Other(format!("{:#}", e)))?;
    let atomic_entry_actual: BTreeSet<(String, String)> =
        atomic.orphan_entry_refs.iter().cloned().collect();
    let atomic_section_actual: BTreeSet<(String, String)> =
        atomic.orphan_section_refs.iter().cloned().collect();
    let mut atomic_entry_ledger: BTreeSet<(String, String)> = BTreeSet::new();
    let mut atomic_section_ledger: BTreeSet<(String, String)> = BTreeSet::new();
    for entry in &loaded.config.orphan_ledger {
        match entry.kind {
            OrphanKind::AtomicEntryRef => {
                atomic_entry_ledger.insert((entry.from.clone(), entry.to.clone()));
            }
            OrphanKind::AtomicSectionRef => {
                atomic_section_ledger.insert((entry.from.clone(), entry.to.clone()));
            }
            _ => {}
        }
    }
    let atomic_new_entries: Vec<(String, String)> = atomic_entry_actual
        .difference(&atomic_entry_ledger)
        .cloned()
        .collect();
    let atomic_resolved_entries: Vec<(String, String)> = atomic_entry_ledger
        .difference(&atomic_entry_actual)
        .cloned()
        .collect();
    let atomic_new_sections: Vec<(String, String)> = atomic_section_actual
        .difference(&atomic_section_ledger)
        .cloned()
        .collect();
    let atomic_resolved_sections: Vec<(String, String)> = atomic_section_ledger
        .difference(&atomic_section_actual)
        .cloned()
        .collect();

    // T1 rule 4 (atomic axis) — Superseded sections must carry the
    // structural superseded_by forward-pointer (R342). State-based
    // post-condition gate reading the atomic store as SSOT; the CLI's
    // validate-workspace runs the same check, so the MCP wire must too
    // (R318 closed the gap where ops omitted it).
    let supersede_violations: Vec<String> =
        mnemosyne_validate::atomic_section_supersede_state_reject(&atomic_store)
            .into_iter()
            .map(|e| {
                let ValidationError::SupersedeMissingRef { section_id, .. } = e;
                format!(
                    "§{} decision_status=Superseded but superseded_by is unset",
                    section_id
                )
            })
            .collect();

    // R296 publishable / audit divergence ledger gate. Each entry whose
    // publishable half diverges from the audit half must have a matching
    // [[publishable_override_ledger]] row (target_id + content_hash_after).
    let ledger = &loaded.config.publishable_override_ledger;
    let divergent: Vec<(&String, &mnemosyne_atomic::AtomicChangelogEntry)> = atomic_store
        .changelog_entries
        .iter()
        .filter(|(_, e)| !e.publishable_matches_audit())
        .collect();
    let publishable_divergence = divergent.len();
    let publishable_ledger_rows = ledger.len();
    let mut publishable_unmatched: Vec<String> = Vec::new();
    for (entry_id, entry) in &divergent {
        let current_hash = entry.publishable_hash_hex();
        let matched = ledger
            .iter()
            .any(|row| row.target_id == **entry_id && row.content_hash_after == current_hash);
        if !matched {
            publishable_unmatched.push(format!(
                "diverged `{}` — publishable_hash={} (no matching ledger row)",
                entry_id, current_hash
            ));
        }
    }

    // Store-registry integrity (R677, extending R675 from the kind facet to the
    // whole registry): every entity kind, and every fact's frame / branch /
    // entities / canon coordinates / evidence, must resolve. Enforced HERE in
    // the baseline gate — not only in validate-continuity's boundary — so a
    // half-migrated or out-of-band-edited store fails the gate a map adopter
    // actually runs. The SAME detector as the boundary, so the two enforce one
    // set (the R675 half-enforced-invariant rule, now over the whole registry).
    let registry_violations = mnemosyne_atomic::store_registry_violations(&atomic_store);

    // Citation-gate COVERAGE (Round 783): every Rust source is either scanned or
    // declared out. Round 777 derived the scan set inside `crates/`, which left
    // the claim about WHICH trees hold citations still a hand list — and it had
    // drifted, silently, by four build scripts and all of `tools/`.
    //
    // Round 835 — the CODE root, not the anchor. `workspace_root` here is the
    // directory a command was invoked from; scan paths are declared relative to
    // `[workspace] root`, which is what `loaded.workspace_root` holds and what
    // every other path in the tool resolves against (the rule is stated on
    // `cascade::workspace_root_from`, whose own doc names `root = "../../.."`
    // as the case). Passing the anchor made this gate reject every external
    // workspace whose ledger lives in a subdirectory — reported from the field,
    // where five enrolled workspaces all failed on paths the SAME BINARY
    // resolved correctly in `validate-code-refs` one command over.
    let code_root = &loaded.workspace_root;
    let scan = {
        let cfg = loaded
            .config
            .plugins
            .as_ref()
            .and_then(|p| p.set_equality_validator.as_ref());
        let paths = cfg.map(|c| c.paths.clone()).unwrap_or_default();
        let exclusions = cfg.map(|c| c.scan_exclusions.clone()).unwrap_or_default();
        mnemosyne_validate::code_refs::scan_coverage(code_root, &paths, &exclusions).map_err(
            |e| {
                // Say WHICH root was used and WHERE IT CAME FROM. Without this
                // the message reads as an accusation against the consumer's
                // `paths`, and the obvious repair is to start editing correct
                // entries — the field report's diagnosis took three commands
                // because nothing in the failure named a root at all.
                OpError::from(anyhow::anyhow!(
                    "scan coverage: {e}\n  scan paths are resolved against {}",
                    loaded.root_provenance()
                ))
            },
        )?
    };
    let rel = |p: &std::path::Path| p.strip_prefix(code_root).unwrap_or(p).display().to_string();
    let scan_unscanned: Vec<String> = scan.unscanned.iter().map(|p| rel(p)).collect();

    // Round 840 — an exclusion is a CLAIM ("no citation this ledger gates lives
    // here") and nothing checked it. Reported from the field: the external spec
    // consumer followed our own four-line exclusion advice, checked the trees by
    // hand first, and found 55 hand-authored citations that would have been
    // silently un-gated. Only citations found NOWHERE the gate reads count — a
    // copy inside an excluded tree loses nothing, which is what keeps their
    // honest codegen-output exclusions silent.
    //
    // Round 854 — "nowhere the gate reads" now means every language the gate
    // reads. The same consumer reported the first version answering that
    // whole-tree question from a Rust-only file set, which named seven sections
    // cited and bound in scanned C++ as excluded-only.
    //
    // Round 867 — the attribution is derived ONCE here and shared by the axis and
    // its coverage line below, so no two answers to "whose numbering is this" can
    // exist in one run.
    let code_refs_cfg = loaded
        .config
        .plugins
        .as_ref()
        .and_then(|p| p.set_equality_validator.as_ref());
    let attribution = code_refs_cfg.map(|c| {
        mnemosyne_validate::code_refs::CitationAttribution::new(
            code_root,
            c,
            mnemosyne_validate::code_refs::NumberingOriginAxis::derive(code_root),
        )
    });
    let swallowed: Vec<String> = {
        match attribution.as_ref() {
            None => Vec::new(),
            Some(attr) => mnemosyne_validate::code_refs::swallowed_citations(&scan, &id_set, attr)
                .into_iter()
                .map(|s| {
                    format!(
                        "`{}` is cited only inside an excluded tree ({}:{}, {} file(s)) — \
                     the exclusion removed it from the gate entirely",
                        s.section_id,
                        rel(&s.file),
                        s.line,
                        s.occurrences
                    )
                })
                .collect(),
        }
    };

    // Round 866 — the swallowed answer above is read OUT OF the excluded set, so
    // whether it is stable depends on whether that set is the same on a
    // developer's disk and in CI. Round 864 asked the tree's own VCS that
    // question about the read set and left this one open; the consumer priced
    // it: 9591 of one ledger's 13685 excluded files exist only where someone
    // has built. It answers 0 swallowed on every ledger today, and that is a
    // measured result rather than a property (the Round 862 rule).
    let excluded_vcs =
        mnemosyne_validate::code_refs::vcs_ignored_among(code_root, &scan.excluded_files);

    // Round 867 — this axis LOOSENS where its siblings tighten, so it counts out
    // loud. The set is BOTH sides the swallowed answer is read out of: the read
    // set decides `still_seen` and the excluded set decides what is found, so a
    // subtree derived foreign on either side moves the answer.
    let numbering_origin = attribution.as_ref().map(|attr| {
        let both: std::collections::BTreeSet<std::path::PathBuf> = scan
            .scanned_files
            .iter()
            .chain(scan.excluded_files.iter())
            .cloned()
            .collect();
        mnemosyne_validate::code_refs::numbering_origin_coverage(attr, &both)
    });

    // Failure aggregation.
    let mut failure_reasons: Vec<String> = Vec::new();
    if !scan_unscanned.is_empty() {
        failure_reasons.push(format!(
            "{} Rust source(s) neither scanned by [plugins.set_equality_validator].paths \
             nor declared in scan_exclusions — add a path or declare the exclusion",
            scan_unscanned.len()
        ));
    }
    if !scan.stale_exclusions.is_empty() {
        failure_reasons.push(format!(
            "{} scan_exclusions entry(ies) match no file — delete them",
            scan.stale_exclusions.len()
        ));
    }
    // A REJECT, like its Round 783 sibling: an exclusion that removes the only
    // copy of a citation is a gate reporting clean over coverage it gave away.
    if !swallowed.is_empty() {
        failure_reasons.push(format!(
            "{} citation(s) exist only inside an excluded tree — scan the tree or \
             bind the citation; an exclusion asserts the gate loses nothing",
            swallowed.len()
        ));
    }
    if !registry_violations.is_empty() {
        let sample = registry_violations
            .iter()
            .take(3)
            .cloned()
            .collect::<Vec<_>>()
            .join("; ");
        failure_reasons.push(format!(
            "store registry integrity: {} out-of-band violation(s) — {}",
            registry_violations.len(),
            sample
        ));
    }
    if !orphan_new.is_empty() {
        failure_reasons.push(format!(
            "new orphan {} cases — register in [[orphan_ledger]] or fix",
            orphan_new.len()
        ));
    }
    if !orphan_resolved.is_empty() {
        failure_reasons.push(format!(
            "{} ledger entry(ies) resolved — delete from [[orphan_ledger]]",
            orphan_resolved.len()
        ));
    }
    if t3_reject_count > 0 {
        failure_reasons.push(format!(
            "T3 deterministic violation {} cases — terminology_consistency",
            t3_reject_count
        ));
    }
    if !atomic_new_entries.is_empty() || !atomic_new_sections.is_empty() {
        failure_reasons.push(format!(
            "atomic orphan new (entries={}, sections={})",
            atomic_new_entries.len(),
            atomic_new_sections.len()
        ));
    }
    if !atomic_resolved_entries.is_empty() || !atomic_resolved_sections.is_empty() {
        failure_reasons.push(format!(
            "atomic orphan resolved (entries={}, sections={})",
            atomic_resolved_entries.len(),
            atomic_resolved_sections.len()
        ));
    }
    if !supersede_violations.is_empty() {
        failure_reasons.push(format!(
            "T1 rule 4 (atomic axis): {} Superseded section(s) without superseding cross-ref",
            supersede_violations.len()
        ));
    }
    if !publishable_unmatched.is_empty() {
        failure_reasons.push(format!(
            "publishable/audit divergence on {} entry(ies) without matching [[publishable_override_ledger]] row",
            publishable_unmatched.len()
        ));
    }
    // Round 983 — whether an entry id is even required to date the counts in
    // its own prose. Resolved through the same shared path the append gate
    // uses, and counted with production's own resolver, so this cannot say one
    // thing while the gate does another.
    let entry_id_dating = if crate::workspace_entry_id_prefix(workspace_root)?.is_empty() {
        EntryIdDating::NotDemanded
    } else {
        EntryIdDating::Demanded {
            dated: atomic_store
                .changelog_entries
                .keys()
                .filter(|id| mnemosyne_atomic::project::parse_round_number(id).is_some())
                .count(),
            total: atomic_store.changelog_entries.len(),
        }
    };

    // Round 979 — a census an UNCOMMITTED entry records must be what the
    // workspace's report says right now.
    let (census_reach, census_stale) = census_contemporaneity(workspace_root, &atomic_store)?;
    if !census_stale.is_empty() {
        failure_reasons.push(format!(
            "{} uncommitted entry census row(s) disagree with the workspace's [census] report",
            census_stale.len()
        ));
    }
    let failed = !failure_reasons.is_empty();

    Ok(ValidateWorkspaceReport {
        census_stale,
        census_reach,
        entry_id_dating,
        orphan_actual,
        orphan_ledger: orphan_ledger_view,
        orphan_new,
        orphan_resolved,
        atomic_entries: atomic.entries,
        atomic_sections: atomic.sections,
        atomic_orphan_entry_refs: atomic.orphan_entry_refs.len(),
        atomic_orphan_section_refs: atomic.orphan_section_refs.len(),
        atomic_new_entries,
        atomic_resolved_entries,
        atomic_new_sections,
        atomic_resolved_sections,
        style_t3_reject: t3_reject_count,
        style_t3_warn: t3_warn_count,
        style_t4_info: t4_count,
        style_t3_reject_messages: t3_reject_messages,
        supersede_violations,
        publishable_divergence,
        publishable_ledger_rows,
        publishable_unmatched,
        store_registry_violations: registry_violations.len(),
        scan_considered: scan.considered,
        scan_scanned: scan.scanned,
        scan_unscanned,
        scan_stale_exclusions: scan.stale_exclusions,
        scan_excluded_but_scanned: scan.excluded_but_scanned.iter().map(|p| rel(p)).collect(),
        scan_gate_files: scan.scanned_files.len(),
        scan_excluded_files: scan.excluded_files.len(),
        scan_excluded_vcs: excluded_vcs,
        scan_swallowed_citations: swallowed,
        scan_numbering_origin: numbering_origin,
        failed,
        failure_reasons,
    })
}

/// WHAT THE CONTEMPORANEITY CHECK COULD SEE (Round 980).
///
/// A verdict of "no violations" is worth what the reach behind it is worth, and
/// this check has four ways of finding nothing — three of which are not the
/// same as "nothing is wrong". Round 979 shipped it reporting only violations,
/// so a workspace that had never installed the pre-commit hook, or kept no
/// census at all, read exactly like one the gate had just cleared. That silence
/// was named in Round 979's own carry as "nothing here tells them so" and is
/// this type: every way of not knowing gets a name in the output, which is the
/// Round 854 rule (a zero out of a population of zero is not a clean bill).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CensusReach {
    /// The workspace declares no `[census] report`, so no entry here can record
    /// a census and there is nothing this check could ever say.
    NotDeclared,
    /// A report is declared and no entry records a census yet.
    NothingRecorded,
    /// Which entries this commit ADDS is unknowable — outside a repository, on a
    /// branch with no `HEAD`, or with a committed store that will not parse — so
    /// no entry can be checked and none is reported as wrong.
    Undecidable { reason: String },
    /// The population the check read.
    Measured {
        /// Entries in the store that record a census at all.
        recorded: usize,
        /// Of those, the ones this commit is adding, checked against the report
        /// in the working tree.
        uncommitted: usize,
        /// Of those, the ones the TIP commit added, checked against the report as
        /// that commit left it — the arm that needs no hook and runs in CI.
        landed_at_head: usize,
        /// Older entries: what the report said when they landed is behind more
        /// history than a bounded check will walk.
        out_of_reach: usize,
        /// Census rows compared against a report.
        rows_checked: usize,
    },
}

/// WHETHER THE LEDGER'S COUNTS ARE DATED AT ALL (Round 983).
///
/// The ledger needs no ban on undated counts, and the reason is structural: an
/// entry is filed under its own round, so a count inside it is pinned to the
/// moment it was taken. That property is worth exactly what the entry id
/// guarantees — and it is guaranteed only where a workspace configures
/// `schema.entry_id_prefix`, because the Round 976 gate demands a number only
/// after a prefix it was given. A consumer with `entry_id_prefix = ""` has the
/// gate stand down and gets no dating at all.
///
/// Round 976 stated that in its own carry and ended with "nothing here tells
/// them so". Nothing did. This is the same defect Round 980 closed one axis
/// over, found by a program rather than by re-reading: every state is named in
/// the output, so a workspace that has the protection and one that never could
/// no longer print the same clean.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EntryIdDating {
    /// No `schema.entry_id_prefix`, so the Round 976 gate stands down: nothing
    /// demands a number in an entry id, and every count in this ledger's prose
    /// is therefore undated.
    NotDemanded,
    /// The gate is in force. `dated` counts entries whose id yields a round
    /// number through the projection's own resolver; anything short of `total`
    /// is frozen history from before the gate.
    Demanded { dated: usize, total: usize },
}

/// THE CENSUS AN ENTRY IS ABOUT TO FREEZE MUST BE THE ONE THE TREE HOLDS NOW.
///
/// `--record-census` reads the workspace's report, so the number in an entry is
/// derived and never typed. What derivation alone cannot close is ORDER: a round
/// that appends its entry, then moves the population and re-blesses the report,
/// freezes a count that was true one step earlier. Nothing downstream can tell
/// that apart from a count that simply aged — an entry is history, and history
/// is supposed to disagree with the present.
///
/// It IS decidable at exactly one moment: while the entry is still uncommitted.
/// So the question this asks is narrow on purpose — not "does every recorded
/// census match today" (which would fail on every honest older entry) but "does
/// the census this commit is ADDING match what this tree says", which is the
/// only version of the question with a right answer.
///
/// REACH, stated rather than implied: in CI nothing is uncommitted, so the
/// entry set is empty and this passes VACUOUSLY there. It bites in the tree that
/// wrote the entry, on the `validate-workspace` the pre-commit hook already
/// runs. That is the same shape as the runbook-tracking gate, and for the same
/// reason: a defect that can only exist before a commit has to be caught before
/// the commit.
///
/// Every way of NOT KNOWING yields an empty answer rather than a violation: no
/// `[census]` table, no git, no `HEAD` (the first commit of a repository), an
/// unreadable committed store. A gate that guessed here would reject a
/// legitimate workspace for the shape of its history.
fn census_contemporaneity(
    workspace_root: &Path,
    store: &mnemosyne_atomic::AtomicStore,
) -> Result<(CensusReach, Vec<String>), OpError> {
    if crate::workspace_census_report_path(workspace_root)?.is_none() {
        return Ok((CensusReach::NotDeclared, Vec::new()));
    }
    let recording: Vec<(&String, &Vec<mnemosyne_atomic::PopulationCensus>)> = store
        .changelog_entries
        .iter()
        .filter(|(_, e)| !e.population_census.is_empty())
        .map(|(id, e)| (id, &e.population_census))
        .collect();
    if recording.is_empty() {
        return Ok((CensusReach::NothingRecorded, Vec::new()));
    }
    let undecidable = |why: &str| {
        Ok((
            CensusReach::Undecidable {
                reason: why.to_string(),
            },
            Vec::new(),
        ))
    };
    let sidecar = crate::resolve_sidecar(workspace_root, None)?;
    let (Some(dir), Some(name)) = (sidecar.parent(), sidecar.file_name()) else {
        return undecidable("the atomic sidecar has no parent directory to ask git from");
    };
    let out = std::process::Command::new("git")
        .args(["show", &format!("HEAD:./{}", name.to_string_lossy())])
        .current_dir(dir)
        .output();
    let Ok(out) = out else {
        return undecidable("git is not runnable here");
    };
    if !out.status.success() {
        return undecidable(
            "git has no committed store at HEAD (outside a repository, or before the first commit)",
        );
    }
    let Ok(committed) = serde_json::from_slice::<serde_json::Value>(&out.stdout) else {
        return undecidable("the store committed at HEAD does not parse as JSON");
    };
    let already: BTreeSet<String> = committed
        .get("changelog_entries")
        .and_then(|v| v.as_object())
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default();

    // Round 982 — the entries the TIP COMMIT added, and the report as that
    // commit left it. This is the arm that needs no pre-commit hook: a consumer
    // who never installed one still gets every round checked by CI, on the push
    // that lands it. Bounded to one commit back on purpose — asking "what did
    // the report say when THIS entry landed" for an arbitrary entry is a pickaxe
    // walk over every revision of a multi-megabyte store, and a gate that costs
    // that gets turned off.
    let previous: Option<BTreeSet<String>> = git_store_entries(dir, name, "HEAD~1");
    let head_report: Option<Vec<mnemosyne_atomic::PopulationCensus>> =
        git_census_report(workspace_root, "HEAD");

    let recorded = recording.len();
    let mut uncommitted = 0usize;
    let mut landed_at_head = 0usize;
    let mut out_of_reach = 0usize;
    let mut rows_checked = 0usize;
    let mut stale = Vec::new();
    let mut report: Option<Vec<mnemosyne_atomic::PopulationCensus>> = None;
    for (id, rows) in recording {
        let at_head = already.contains(id);
        let landed_here = at_head && previous.as_ref().is_some_and(|p| !p.contains(id));
        if at_head && !landed_here {
            out_of_reach += 1;
            continue;
        }
        // Resolved once, and only for a tree that has something to check — a
        // workspace with no `[census]` table cannot have recorded one either,
        // so reaching here with an unreadable report is a real defect and says
        // so rather than passing quietly.
        let axes = if landed_here {
            landed_at_head += 1;
            match &head_report {
                Some(a) => a,
                None => {
                    out_of_reach += 1;
                    landed_at_head -= 1;
                    continue;
                }
            }
        } else {
            uncommitted += 1;
            match &report {
                Some(a) => a,
                None => {
                    report = Some(crate::workspace_population_census(workspace_root)?);
                    report.as_ref().expect("just resolved")
                }
            }
        };
        for row in rows {
            rows_checked += 1;
            if !axes.contains(row) {
                stale.push(format!(
                    "{id}: `{}` recorded as {}={} {}={}, which is not what the \
                     census report it was filed against says — re-bless the \
                     report and append the entry after it, in that order",
                    row.axis, row.left_label, row.left, row.right_label, row.right
                ));
            }
        }
    }
    Ok((
        CensusReach::Measured {
            recorded,
            uncommitted,
            landed_at_head,
            out_of_reach,
            rows_checked,
        },
        stale,
    ))
}

/// The changelog entry ids a committed store holds at `rev`, or `None` when
/// that revision has no readable store (Round 982).
fn git_store_entries(dir: &Path, name: &std::ffi::OsStr, rev: &str) -> Option<BTreeSet<String>> {
    let out = std::process::Command::new("git")
        .args(["show", &format!("{rev}:./{}", name.to_string_lossy())])
        .current_dir(dir)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let store: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    Some(
        store
            .get("changelog_entries")
            .and_then(|v| v.as_object())
            .map(|o| o.keys().cloned().collect())
            .unwrap_or_default(),
    )
}

/// The census report as `rev` left it, or `None` when that revision has none
/// (Round 982) — a workspace that declared `[census]` later, a shallow clone, a
/// report tracked under a different path back then.
fn git_census_report(
    workspace_root: &Path,
    rev: &str,
) -> Option<Vec<mnemosyne_atomic::PopulationCensus>> {
    let path = crate::workspace_census_report_path(workspace_root).ok()??;
    let (dir, name) = (path.parent()?, path.file_name()?);
    let out = std::process::Command::new("git")
        .args(["show", &format!("{rev}:./{}", name.to_string_lossy())])
        .current_dir(dir)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let report: PopulationCensusReport = serde_json::from_slice(&out.stdout).ok()?;
    Some(report.axes)
}

impl ValidateWorkspaceReport {
    /// Render the report as the same plain-text summary the CLI bin
    /// previously emitted (line-for-line compat). Used by both `mnemosyne-
    /// cli validate-workspace` and the MCP server's `validate_workspace`
    /// tool so the human-readable output stays stable.
    pub fn render_plain(&self) -> String {
        use std::fmt::Write;
        let mut out = String::new();
        let _ = writeln!(out, "=== mnemosyne-cli validate-workspace ===");
        let _ = writeln!(
            out,
            "T1 orphan total={} (ledger={}, new=+{}, resolved=-{})",
            self.orphan_actual.len(),
            self.orphan_ledger.len(),
            self.orphan_new.len(),
            self.orphan_resolved.len(),
        );
        for o in &self.orphan_actual {
            let _ = writeln!(
                out,
                "  orphan {}: §{} -> §{}",
                o.doc, o.from_section, o.to_target
            );
        }
        if !self.orphan_new.is_empty() {
            let _ = writeln!(out, "new orphans (ledger registration or fix enforced):");
            for o in &self.orphan_new {
                let _ = writeln!(
                    out,
                    "  + {}: §{} -> §{}",
                    o.doc, o.from_section, o.to_target
                );
            }
        }
        if !self.orphan_resolved.is_empty() {
            let _ = writeln!(out, "resolved ledger entries (delete from ledger):");
            for o in &self.orphan_resolved {
                let _ = writeln!(
                    out,
                    "  - {}: §{} -> §{}",
                    o.doc, o.from_section, o.to_target
                );
            }
        }
        let _ = writeln!(
            out,
            "style violations: T3 reject={} / T3 warn={} / T4 info={} (Round 138 tier mobility ratify)",
            self.style_t3_reject, self.style_t3_warn, self.style_t4_info
        );
        for m in &self.style_t3_reject_messages {
            let _ = writeln!(out, "  - {}", m);
        }
        let _ = writeln!(
            out,
            "atomic ledger: entries={} / sections={} / orphan_refs={}+{}",
            self.atomic_entries,
            self.atomic_sections,
            self.atomic_orphan_entry_refs,
            self.atomic_orphan_section_refs,
        );
        for v in &self.supersede_violations {
            let _ = writeln!(out, "  T1 rule 4 (atomic axis): {}", v);
        }
        let _ = writeln!(
            out,
            "store registry integrity: {} out-of-band violation(s) (Round 677)",
            self.store_registry_violations
        );
        // The considered count is the non-vacuity figure: "0 unscanned" out of 0
        // sources is what a broken walk reports, so both numbers are printed.
        let _ = writeln!(
            out,
            "citation-gate coverage: {} rust source(s), {} scanned, {} unscanned, {} stale exclusion(s) (Round 783)",
            self.scan_considered,
            self.scan_scanned,
            self.scan_unscanned.len(),
            self.scan_stale_exclusions.len(),
        );
        for p in &self.scan_unscanned {
            let _ = writeln!(out, "  unscanned: {}", p);
        }
        for e in &self.scan_stale_exclusions {
            let _ = writeln!(out, "  stale exclusion: {}", e);
        }
        // Round 860 — say it once with the count and the repair, then name a few.
        // A consumer reaching for `scan_exclusions` to stop the gate reading a
        // subtree gets config that matches files and changes nothing; the field
        // report that found this had to diff the counts to notice.
        if !self.scan_excluded_but_scanned.is_empty() {
            let _ = writeln!(
                out,
                "  advisory: {} file(s) are BOTH covered by [plugins.set_equality_validator].paths \
                 and named by scan_exclusions — an exclusion declares coverage intent and does \
                 not narrow what the gate reads; narrow `paths` instead (Round 860)",
                self.scan_excluded_but_scanned.len()
            );
            for p in self.scan_excluded_but_scanned.iter().take(3) {
                let _ = writeln!(out, "    both scanned and excluded: {}", p);
            }
        }
        // Same non-vacuity discipline as the line above, one axis over: the
        // file counts say whether the axis read anything at all, and in ANY
        // language. Round 854 — it read only Rust, and a consumer whose
        // citations live in C++ got a whole-tree verdict from an empty set.
        let _ = writeln!(
            out,
            "exclusion integrity: {} citation(s) swallowed by an exclusion (Round 840); \
             read {} gate file(s), {} excluded file(s), any language (Round 854)",
            self.scan_swallowed_citations.len(),
            self.scan_gate_files,
            self.scan_excluded_files,
        );
        // Round 866 — printed in all three states directly under the count it
        // qualifies, because the reader deciding whether to trust "0 swallowed"
        // is reading that line right now.
        match &self.scan_excluded_vcs {
            mnemosyne_validate::code_refs::VcsIgnoreAxis::Measured {
                considered,
                ignored,
                ignored_extensions,
            } => {
                let _ = writeln!(
                    out,
                    "  vcs axis (advisory, Round 866): {} of {} excluded file(s) are build \
                     output by this tree's own VCS {}{}",
                    ignored.len(),
                    considered,
                    mnemosyne_validate::code_refs::summarize_extensions(ignored_extensions, 5),
                    if ignored.is_empty() {
                        ""
                    } else {
                        " — the swallowed count above is developer-shaped"
                    }
                );
            }
            mnemosyne_validate::code_refs::VcsIgnoreAxis::NotDetermined { reason } => {
                let _ = writeln!(
                    out,
                    "  vcs axis (advisory, Round 866): not determined — {reason}"
                );
            }
        }
        // Round 867 — printed beside the count for the same reason, and printed
        // even at zero: this is the one axis in the family that makes citations
        // DISAPPEAR, so a run that skipped some must say how many and from where.
        if let Some(origin) = &self.scan_numbering_origin {
            match &origin.axis {
                mnemosyne_validate::code_refs::NumberingOriginAxis::Measured {
                    foreign_subtrees,
                } => {
                    let _ = writeln!(
                        out,
                        "  numbering origin (advisory, Round 867): {} foreign subtree(s) by this \
                         tree's own VCS — {} §-token(s) in {} of {} file(s) are NOT read as this \
                         store's {}",
                        foreign_subtrees.len(),
                        origin.citations_skipped,
                        origin.files_foreign,
                        origin.files_considered,
                        mnemosyne_validate::code_refs::summarize_extensions(
                            &origin.skipped_per_subtree,
                            5
                        ),
                    );
                }
                mnemosyne_validate::code_refs::NumberingOriginAxis::NotDetermined { reason } => {
                    let _ = writeln!(
                        out,
                        "  numbering origin (advisory, Round 867): not determined — {reason}; \
                         every citation stays this store's"
                    );
                }
            }
        }
        for e in &self.scan_swallowed_citations {
            let _ = writeln!(out, "  swallowed: {}", e);
        }
        let _ = writeln!(
            out,
            "publishable / audit divergence: entries={} ledger_rows={}",
            self.publishable_divergence, self.publishable_ledger_rows
        );
        for u in &self.publishable_unmatched {
            let _ = writeln!(out, "  {}", u);
        }
        let _ = writeln!(
            out,
            "census contemporaneity: {} (Round 982; reaches what this commit is \
             adding and what the tip commit added, and says so when it reaches \
             neither)",
            match &self.census_reach {
                CensusReach::NotDeclared =>
                    "off — this workspace declares no [census] report".to_string(),
                CensusReach::NothingRecorded => "no entry records a census yet".to_string(),
                CensusReach::Undecidable { reason } =>
                    format!("UNDECIDABLE — {reason}, so no entry was checked"),
                CensusReach::Measured {
                    recorded,
                    uncommitted,
                    landed_at_head,
                    out_of_reach,
                    rows_checked,
                } => format!(
                    "{recorded} entry(ies) record one, {uncommitted} uncommitted, \
                     {landed_at_head} landed at HEAD, {out_of_reach} older than \
                     this check walks, {rows_checked} row(s) checked against a report"
                ),
            }
        );
        for s in &self.census_stale {
            let _ = writeln!(out, "  {}", s);
        }
        let _ = writeln!(
            out,
            "ledger dating: {} (Round 983; a count inside an entry is pinned by \
             the round it is filed under, and only a configured \
             schema.entry_id_prefix demands that number)",
            match &self.entry_id_dating {
                EntryIdDating::NotDemanded =>
                    "OFF — this workspace configures no schema.entry_id_prefix, so no \
                     entry id has to carry a round number and every count in the \
                     ledger's prose is undated"
                        .to_string(),
                EntryIdDating::Demanded { dated, total } =>
                    format!("{dated} of {total} entry(ies) are dated by their own key"),
            }
        );
        if self.failed {
            let _ = writeln!(out, "FAILED:");
            for r in &self.failure_reasons {
                let _ = writeln!(out, "  - {}", r);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// A workspace whose ledger lives BELOW the code it documents — the shape
    /// every external adopter has and this repo does not.
    ///
    /// Returns `(tempdir, toml_dir)`. The tree is:
    /// ```text
    /// <repo>/crates/alpha/src/lib.rs      <- the code the scan must find
    /// <repo>/docs/spec/mnemosyne.toml     <- [workspace] root = "../.."
    /// ```
    fn nested_ledger_workspace() -> (tempfile::TempDir, std::path::PathBuf) {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let repo = tmp.path();
        let src = repo.join("crates/alpha/src");
        fs::create_dir_all(&src).expect("src");
        fs::write(src.join("lib.rs"), "// Round 835\npub fn f() {}\n").expect("lib.rs");
        // MIXED LANGUAGE by construction (Round 854). Every scan path in this
        // repo holds Rust only, which is why a Rust-only file universe was
        // invisible here and had to be reported from a C++ consumer's field.
        // Cites nothing: its job is to be a file the gate reads and a
        // Rust-shaped view of the workspace does not.
        fs::write(
            src.join("impl.cpp"),
            "// no citation here\nint g() { return 0; }\n",
        )
        .expect("impl.cpp");
        let toml_dir = repo.join("docs/spec");
        fs::create_dir_all(toml_dir.join(".atomic")).expect("toml dir");
        fs::write(
            toml_dir.join("mnemosyne.toml"),
            "[workspace]\nroot = \"../..\"\n\n\
             [atomic]\nsidecar_path = \"docs/spec/.atomic/store.json\"\n\n\
             [plugins.set_equality_validator]\npaths = [\"crates/*/src/\"]\n",
        )
        .expect("write config");
        fs::write(
            toml_dir.join(".atomic/store.json"),
            format!(
                "{{\"schema_version\":{},\"sections\":{{}}}}",
                mnemosyne_atomic::CURRENT_SCHEMA_VERSION
            ),
        )
        .expect("write store");
        (tmp, toml_dir)
    }

    /// Round 835 — the baseline gate resolves scan paths against `[workspace]
    /// root`, like every other path in the tool.
    ///
    /// Reported from the field, not found here: five enrolled workspaces of an
    /// external consumer were all rejected by `validate-workspace` on scan
    /// paths that `validate-code-refs` — the SAME binary, one command over —
    /// resolved correctly. This repo never saw it because its own config
    /// declares no `[workspace] root`, so the anchor and the code root are the
    /// same directory and the bug is invisible by construction. That is why the
    /// fixture below is a NESTED ledger: the defect only exists where the two
    /// differ, so a fixture where they agree would pass either way.
    #[test]
    fn scan_coverage_resolves_against_the_declared_root_not_the_anchor() {
        let (_tmp, toml_dir) = nested_ledger_workspace();
        let report = validate_workspace(&toml_dir).expect(
            "validate-workspace must resolve scan paths against [workspace] root; \
             a scan-coverage error here is the field-reported defect",
        );
        // NON-VACUITY: the axis actually saw the file. Without this the test
        // would also pass against a gate that scanned nothing and said nothing,
        // which is the failure mode the Round 783 check exists to prevent.
        assert_eq!(
            report.scan_considered, 1,
            "the scan found no Rust source under the declared root — the axis is \
             empty, so its silence proves nothing"
        );
        assert_eq!(report.scan_scanned, 1, "the file was found but not scanned");
        assert!(
            report.scan_unscanned.is_empty(),
            "unscanned: {:?}",
            report.scan_unscanned
        );
    }

    /// A workspace that HAS the dating guarantee and one that never could must
    /// not print the same clean (Round 983).
    ///
    /// The counts are read from a fixture whose store deliberately holds one
    /// entry the gate would reject today — frozen history from before it — so
    /// `dated` is not the same number as `total` and a rendering that printed
    /// the entry count twice would pass.
    #[test]
    fn ledger_dating_is_reported_in_both_states() {
        fn workspace(prefix: &str) -> tempfile::TempDir {
            let tmp = tempfile::TempDir::new().expect("tempdir");
            let ws = tmp.path();
            fs::create_dir_all(ws.join("docs/.atomic")).expect("atomic dir");
            fs::write(
                ws.join("mnemosyne.toml"),
                format!("[workspace]\n[schema]\nentry_id_prefix = \"{prefix}\"\n"),
            )
            .expect("config");
            fs::write(
                ws.join("docs/.atomic/workspace.atomic.json"),
                serde_json::json!({
                    "schema_version": mnemosyne_atomic::CURRENT_SCHEMA_VERSION,
                    "sections": {},
                    "changelog_entries": {
                        "Round 900": {"decision_summary": "dated"},
                        "Round misc": {"decision_summary": "frozen history, undated"},
                    },
                })
                .to_string(),
            )
            .expect("store");
            tmp
        }

        let demanded = workspace("Round ");
        let report = validate_workspace(demanded.path()).expect("validate");
        assert_eq!(
            report.entry_id_dating,
            EntryIdDating::Demanded { dated: 1, total: 2 },
            "the count must come from the projection's resolver, not from the \
             entry count: an id with no number is not dated"
        );
        let demanded_line = report
            .render_plain()
            .lines()
            .find(|l| l.starts_with("ledger dating:"))
            .expect("the report says nothing about dating")
            .to_string();

        let off = workspace("");
        let report = validate_workspace(off.path()).expect("validate");
        assert_eq!(report.entry_id_dating, EntryIdDating::NotDemanded);
        let off_line = report
            .render_plain()
            .lines()
            .find(|l| l.starts_with("ledger dating:"))
            .expect("the report says nothing about dating")
            .to_string();

        assert_ne!(
            demanded_line, off_line,
            "a workspace with the dating guarantee and one that never could say \
             the same thing, which is the silence Round 976 left"
        );
        assert!(
            off_line.contains("OFF"),
            "the off state does not name itself: {off_line}"
        );
    }

    /// Round 866 — `validate-workspace` asks the VCS about the EXCLUDED set.
    ///
    /// The wiring is the thing under test, not the axis: passing
    /// `scanned_files` here instead of `excluded_files` compiles, prints a
    /// plausible line, and answers the wrong question. This workspace cannot
    /// catch that on its own — our excluded set holds zero build output, so both
    /// wirings print zero — which is why the fixture makes the two sets differ in
    /// SIZE as well as in content.
    #[test]
    fn the_excluded_set_axis_reports_the_excluded_set_not_the_read_set() {
        let (tmp, toml_dir) = nested_ledger_workspace();
        let repo = tmp.path();
        let out = std::process::Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(repo)
            .output()
            .expect("git must be runnable to test the VCS axis");
        assert!(out.status.success());
        fs::write(repo.join(".gitignore"), "*.gen\n").expect("gitignore");
        let tests = repo.join("crates/alpha/tests");
        fs::create_dir_all(&tests).expect("tests dir");
        // Three files, so the excluded set (3) can never be mistaken for the
        // read set (2) by its count alone.
        fs::write(tests.join("a.rs"), "// Round 835\n").expect("a.rs");
        fs::write(tests.join("b.rs"), "// Round 835\n").expect("b.rs");
        fs::write(tests.join("out.gen"), "// Round 835\n").expect("out.gen");
        fs::write(
            toml_dir.join("mnemosyne.toml"),
            "[workspace]\nroot = \"../..\"\n\n\
             [atomic]\nsidecar_path = \"docs/spec/.atomic/store.json\"\n\n\
             [plugins.set_equality_validator]\npaths = [\"crates/*/src/\"]\n\
             scan_exclusions = [\"crates/*/tests/\"]\n",
        )
        .expect("write config");

        let report = validate_workspace(&toml_dir).expect("validate-workspace");
        assert_eq!(
            (report.scan_gate_files, report.scan_excluded_files),
            (2, 3),
            "the fixture must make the two sets differ in size, or the wiring is \
             untested"
        );
        let mnemosyne_validate::code_refs::VcsIgnoreAxis::Measured {
            considered,
            ignored,
            ..
        } = &report.scan_excluded_vcs
        else {
            panic!(
                "a git workspace must be measurable: {:?}",
                report.scan_excluded_vcs
            );
        };
        assert_eq!(
            (*considered, ignored.as_slice()),
            (3, [repo.join("crates/alpha/tests/out.gen")].as_slice()),
            "the axis must report the EXCLUDED set — the set the swallowed \
             verdict above it is read out of"
        );
    }

    /// Round 835 — the baseline gate and the citation gate agree on the root.
    ///
    /// This is the comparison the consumer had to make BY HAND to diagnose the
    /// defect: run the two subcommands and notice they disagree about the same
    /// configured paths. Encoding it means any future divergence in how the two
    /// resolve a root fails here, rather than in someone else's CI.
    #[test]
    fn the_baseline_gate_and_the_citation_gate_scan_the_same_files() {
        let (_tmp, toml_dir) = nested_ledger_workspace();
        let loaded = mnemosyne_config::discover_config(&toml_dir)
            .expect("discover")
            .expect("config present");
        let paths = loaded
            .config
            .plugins
            .as_ref()
            .and_then(|p| p.set_equality_validator.as_ref())
            .map(|c| c.paths.clone())
            .expect("the fixture configures scan paths");

        // What the CITATION gate walks, through its own resolved root.
        let citation_files =
            mnemosyne_validate::code_refs::walk_paths(&loaded.workspace_root, &paths)
                .expect("citation-gate walk");
        // What the BASELINE gate counts.
        let report = validate_workspace(&toml_dir).expect("baseline gate");

        assert!(
            !citation_files.is_empty(),
            "the citation gate walked nothing — the comparison below would be \
             between two zeroes"
        );
        // Round 854 — the comparison is against the count of files the baseline
        // gate READS, not its Rust-source count. Those two were the same number
        // for as long as the fixture was all-Rust, so this assertion held
        // vacuously while the coverage struct filtered the citation gate's own
        // universe down to `.rs`. The line below is what makes it load-bearing.
        assert!(
            citation_files
                .iter()
                .any(|p| p.extension().is_some_and(|e| e != "rs")),
            "the walk found no non-Rust file — the equality below cannot tell a \
             language-agnostic universe from a Rust-only one: {citation_files:?}"
        );
        assert_eq!(
            report.scan_gate_files,
            citation_files.len(),
            "the two gates disagree about which files the SAME configured paths \
             cover — the exact symptom the field report diagnosed by running \
             both commands"
        );
    }

    /// Round 840 — an exclusion that removes the ONLY copy of a citation is a
    /// gate giving away coverage while reporting clean.
    ///
    /// Reported from the field. The external spec consumer followed our own
    /// four-line exclusion advice, checked the trees by hand BEFORE declaring
    /// them, and found 55 hand-authored citations the exclusions would have
    /// silently un-gated. The advice produced exit 0 and would have cost real
    /// coverage; nothing in the tool could tell the difference.
    ///
    /// Both directions are asserted in one test on ONE fixture, because the
    /// distinction is the whole design: a citation ALSO present in a scanned
    /// file loses nothing when a copy is excluded (their honest codegen-output
    /// case, which must stay silent), and a citation present only in an excluded
    /// tree is gone (their wire case, which must be loud). A detector that fired
    /// on either alone would be useless in the opposite direction.
    #[test]
    fn an_exclusion_that_swallows_the_only_copy_of_a_citation_fails() {
        let (tmp, toml_dir) = nested_ledger_workspace();
        let repo = tmp.path();
        // The store must actually HOLD the cited sections: the detector is
        // scoped to this ledger's own section space, so a citation of some other
        // ledger's namespace is not this one's coverage to lose. Found by
        // running the unscoped first version against a real consumer, where it
        // named fifteen foreign citations on a correctly configured workspace.
        fs::write(
            toml_dir.join(".atomic/store.json"),
            format!(
                "{{\"schema_version\":{},\"sections\":{{\"1.1\":{{}},\"9.9\":{{}}}}}}",
                mnemosyne_atomic::CURRENT_SCHEMA_VERSION
            ),
        )
        .expect("store with the cited sections");
        let excluded = repo.join("crates/generated/src");
        fs::create_dir_all(&excluded).expect("excluded dir");
        // Cited in a SCANNED file too — excluding this copy loses nothing.
        fs::write(
            repo.join("crates/alpha/src/lib.rs"),
            "// Round 835\n// see §1.1\npub fn f() {}\n",
        )
        .expect("scanned source");
        // The `4.4` citation in the fixture below is cited ONLY here and this
        // store does not hold it — a foreign
        // ledger's citation, which this workspace never gated and cannot lose.
        // It stays in the fixture for every phase below, so the scoping filter
        // is load-bearing rather than incidentally satisfied.
        fs::write(
            excluded.join("out.rs"),
            "// generated — see §1.1 and §4.4\npub fn g() {}\n",
        )
        .expect("excluded copy");
        let config = |exclusions: &str| {
            format!(
                "[workspace]\nroot = \"../..\"\n\n\
                 [atomic]\nsidecar_path = \"docs/spec/.atomic/store.json\"\n\n\
                 [plugins.set_equality_validator]\npaths = [\"crates/alpha/src/\"]\n\
                 scan_exclusions = [{exclusions}]\n"
            )
        };
        fs::write(
            toml_dir.join("mnemosyne.toml"),
            config("\"crates/generated/\""),
        )
        .expect("config");

        // A DUPLICATED citation is silent — this is the honest-exclusion case,
        // and asserting it first means the failure below cannot be satisfied by
        // a detector that simply flags every excluded tree.
        let report = validate_workspace(&toml_dir).expect("a duplicated citation is no loss");
        assert!(
            report.scan_swallowed_citations.is_empty(),
            "an excluded copy of a citation the gate still reads is not a loss: {:?}",
            report.scan_swallowed_citations
        );

        // A citation-shaped token in CODE is not a citation. The detector
        // inherits `comment_only` from the gate, and one that saw more than the
        // gate reads would report losses that were never coverage — the Round
        // 820 finding, where counting full text turned one real case into ten.
        fs::write(
            excluded.join("out.rs"),
            "// generated — see §1.1 and §4.4\npub fn g() -> &'static str { \"§7.7\" }\n",
        )
        .expect("excluded source with a code-position token");
        let report = validate_workspace(&toml_dir).expect("report still builds");
        assert!(
            report.scan_swallowed_citations.is_empty(),
            "a citation-shaped token in CODE is not a citation the gate reads: {:?}",
            report.scan_swallowed_citations
        );

        // Now the SAME excluded file is the only place `9.9` is cited.
        fs::write(
            excluded.join("out.rs"),
            "// generated — see §1.1 and §4.4 and §9.9\npub fn g() {}\n",
        )
        .expect("excluded source with a unique citation");
        let report = validate_workspace(&toml_dir).expect("report still builds");
        assert_eq!(
            report.scan_swallowed_citations.len(),
            1,
            "expected exactly the unique citation, got {:?}",
            report.scan_swallowed_citations
        );
        let msg = &report.scan_swallowed_citations[0];
        assert!(
            msg.contains("9.9"),
            "the message must name the citation: {msg}"
        );
        assert!(
            msg.contains("out.rs"),
            "the message must name a file to start from: {msg}"
        );
        assert!(
            report.failed,
            "a swallowed citation must FAIL the gate, like its Round 783 sibling"
        );
    }

    /// Round 854 — the reachability axis reads every language the gate reads.
    ///
    /// Reported from the field, and it is the Round 840 detector answering a
    /// question about the whole tree from a Rust-only universe. `scan_coverage`
    /// filtered its scanned AND excluded sets to `.rs`, so for a consumer whose
    /// citations live in C++ the "still seen" side was empty: seven sections
    /// cited and bound in scanned C++ were reported as reachable only from an
    /// excluded tree. The cost was real — the consumer dropped the citation from
    /// its codegen templates to get the gate green.
    ///
    /// Both error directions come from that ONE filter, and both are asserted
    /// here because a fix verified in one direction is half a fix:
    ///
    /// - FALSE POSITIVE — cited in scanned C++ and in an excluded Rust copy:
    ///   the gate still reads it, so nothing was lost and the axis must be
    ///   silent. Before the fix this was the loud case.
    /// - FALSE NEGATIVE — cited ONLY in an excluded C++ file: this is the exact
    ///   coverage loss Round 840 exists to catch, and before the fix the axis
    ///   could not see the file at all. This one is worse: a swallowed citation
    ///   reads as clean.
    ///
    /// This repo's own fixtures could not have found it — every scan path here
    /// holds Rust only, so the filter is invisible by construction. Hence a
    /// MIXED-LANGUAGE fixture: the `.cpp` files are the test.
    #[test]
    fn the_reachability_axis_reads_every_language_the_gate_reads() {
        let (tmp, toml_dir) = nested_ledger_workspace();
        let repo = tmp.path();
        // The detector is scoped to this ledger's own section space, so the
        // store must hold every id the fixture cites.
        fs::write(
            toml_dir.join(".atomic/store.json"),
            format!(
                "{{\"schema_version\":{},\"sections\":\
                 {{\"1.1\":{{}},\"2.2\":{{}},\"8.8\":{{}}}}}}",
                mnemosyne_atomic::CURRENT_SCHEMA_VERSION
            ),
        )
        .expect("store with the cited sections");
        let excluded = repo.join("crates/generated/src");
        fs::create_dir_all(&excluded).expect("excluded dir");
        // SCANNED Rust — the control: `1.1` is cited here and in the excluded
        // tree, and stayed silent before this round too. Its silence separates
        // "the fix works" from "the detector went quiet".
        fs::write(
            repo.join("crates/alpha/src/lib.rs"),
            "// Round 835\n// see §1.1\npub fn f() {}\n",
        )
        .expect("scanned rust source");
        // SCANNED C++ — the false positive. The gate reads this file; the
        // reachability axis did not.
        fs::write(
            repo.join("crates/alpha/src/impl.cpp"),
            "// see §2.2\nint g() { return 0; }\n",
        )
        .expect("scanned cpp source");
        // EXCLUDED Rust — an honest copy of two citations the gate still reads.
        fs::write(
            excluded.join("out.rs"),
            "// generated — see §1.1 and §2.2\npub fn g() {}\n",
        )
        .expect("excluded rust copy");
        // EXCLUDED C++ — the false negative. `8.8` is cited nowhere else.
        fs::write(
            excluded.join("gen.cpp"),
            "// generated — see §8.8\nint h() { return 0; }\n",
        )
        .expect("excluded cpp source");
        fs::write(
            toml_dir.join("mnemosyne.toml"),
            "[workspace]\nroot = \"../..\"\n\n\
             [atomic]\nsidecar_path = \"docs/spec/.atomic/store.json\"\n\n\
             [plugins.set_equality_validator]\npaths = [\"crates/alpha/src/\"]\n\
             scan_exclusions = [\"crates/generated/\"]\n",
        )
        .expect("config");

        let report = validate_workspace(&toml_dir).expect("report builds");
        // NON-VACUITY on the axis itself: it read the C++ files. Without these
        // two counts the assertions below would also pass against an axis that
        // read one Rust file on each side and said the right thing by luck.
        assert_eq!(
            report.scan_gate_files, 2,
            "the axis must read both scanned files (lib.rs + impl.cpp), not the \
             Rust one only"
        );
        assert_eq!(
            report.scan_excluded_files, 2,
            "the axis must read both excluded files (out.rs + gen.cpp)"
        );
        let swallowed = &report.scan_swallowed_citations;
        assert_eq!(
            swallowed.len(),
            1,
            "expected exactly the citation that is cited nowhere the gate reads, \
             got {swallowed:?}"
        );
        assert!(
            swallowed[0].contains("8.8"),
            "the only swallowed citation is the one cited solely in an excluded \
             C++ file: {swallowed:?}"
        );
        assert!(
            !swallowed.iter().any(|s| s.contains("2.2")),
            "`2.2` is cited in a SCANNED C++ file — the gate reads it, so the \
             excluded copy costs nothing: {swallowed:?}"
        );
    }

    /// Round 854 — an exclusion that removes only non-Rust files is not stale.
    ///
    /// The same `.rs` filter, third symptom: `stale_exclusions` reported an
    /// exclusion as matching nothing whenever it matched nothing in RUST, and a
    /// stale exclusion is a REJECT. So a C++ consumer excluding a C++ tree was
    /// told to delete the declaration that was doing the work.
    #[test]
    fn an_exclusion_matching_only_non_rust_files_is_not_stale() {
        let (tmp, toml_dir) = nested_ledger_workspace();
        let repo = tmp.path();
        let cppgen = repo.join("crates/cppgen");
        fs::create_dir_all(&cppgen).expect("cppgen dir");
        fs::write(
            cppgen.join("x.cpp"),
            "// generated\nint x() { return 0; }\n",
        )
        .expect("cpp file");
        fs::write(
            toml_dir.join("mnemosyne.toml"),
            "[workspace]\nroot = \"../..\"\n\n\
             [atomic]\nsidecar_path = \"docs/spec/.atomic/store.json\"\n\n\
             [plugins.set_equality_validator]\npaths = [\"crates/alpha/src/\"]\n\
             scan_exclusions = [\"crates/cppgen/\"]\n",
        )
        .expect("config");

        let report = validate_workspace(&toml_dir).expect("report builds");
        assert!(
            report.scan_stale_exclusions.is_empty(),
            "the exclusion matches a real tree — it is stale only under a \
             Rust-only view of the workspace: {:?}",
            report.scan_stale_exclusions
        );
        assert!(
            !report.failed,
            "a live exclusion must not fail the gate: {:?}",
            report.failure_reasons
        );
    }

    /// Round 835 — the failure names the root AND where it came from.
    ///
    /// Read cold, `configured scan paths resolve to nothing under <dir>` accuses
    /// the reader's `paths`, and the obvious repair is to edit correct entries.
    /// The clause under test is what makes the message self-diagnosing.
    #[test]
    fn a_scan_coverage_failure_names_the_root_and_its_provenance() {
        let (_tmp, toml_dir) = nested_ledger_workspace();
        // A path that exists under the ANCHOR but not under the declared root,
        // so the failure is reached with the fix in place.
        fs::write(
            toml_dir.join("mnemosyne.toml"),
            "[workspace]\nroot = \"../..\"\n\n\
             [atomic]\nsidecar_path = \"docs/spec/.atomic/store.json\"\n\n\
             [plugins.set_equality_validator]\npaths = [\"no/such/tree/\"]\n",
        )
        .expect("rewrite config");
        let err = validate_workspace(&toml_dir).expect_err("an empty scan set must fail");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("[workspace] root = \"../..\""),
            "the failure must name the declaration that chose the root: {msg}"
        );
        assert!(
            msg.contains("mnemosyne.toml"),
            "the failure must name the file that declared it: {msg}"
        );
    }
}
