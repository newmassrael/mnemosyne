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
use crate::{query::load_workspace, OpError};

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
    /// Round 840 — citations an exclusion removed from the gate entirely.
    pub scan_swallowed_citations: Vec<String>,
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
    let swallowed: Vec<String> = {
        let cfg = loaded
            .config
            .plugins
            .as_ref()
            .and_then(|p| p.set_equality_validator.as_ref());
        match cfg {
            None => Vec::new(),
            Some(c) => mnemosyne_validate::code_refs::swallowed_citations(
                &scan,
                &id_set,
                &c.external_section_prefixes,
                &c.external_section_prefixes_bare,
                c.comment_only,
            )
            .map_err(|e| OpError::from(anyhow::anyhow!("exclusion integrity: {e}")))?
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
            "{} citation(s) exist only inside an excluded tree — scan the tree or              bind the citation; an exclusion asserts the gate loses nothing",
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
    let failed = !failure_reasons.is_empty();

    Ok(ValidateWorkspaceReport {
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
        scan_swallowed_citations: swallowed,
        failed,
        failure_reasons,
    })
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
        let _ = writeln!(
            out,
            "exclusion integrity: {} citation(s) swallowed by an exclusion (Round 840)",
            self.scan_swallowed_citations.len()
        );
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
        assert_eq!(
            report.scan_scanned,
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
