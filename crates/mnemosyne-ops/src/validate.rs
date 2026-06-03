//! `validate-workspace` library op. Encapsulates the multi-step T1/T2/
//! round-trip/atomic-ledger pipeline as a single function that returns a
//! structured report. The CLI bin pretty-prints, the MCP server
//! serializes to JSON.

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::Context;
use mnemosyne_atomic::AtomicStore;
use mnemosyne_config::OrphanKind;
use mnemosyne_query::workspace_section_id_set;
use mnemosyne_style::{
    check_style_atomic, default_ruleset_with_config, StyleSeverity, StyleViolation,
};
use mnemosyne_validate::{validator::scan_store_prose_cross_ref_orphans, ValidationError};
use serde::Serialize;

use crate::cascade::validate_atomic_store;
use crate::{query::load_workspace, resolve_sidecar, OpError};

#[derive(Debug, Clone, Serialize)]
pub struct ValidateWorkspaceReport {
    pub docs_loaded: usize,
    pub docs_configured: usize,
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
    let (ws, loaded, _) = load_workspace(workspace_root).map_err(OpError::from)?;
    let parsed_docs: Vec<(String, mnemosyne_schema::ParsedDoc)> = ws
        .docs
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    // Load the atomic store once (the SSOT) — used for the store-direct orphan
    // scan and the store-direct style check below.
    let sidecar_path = resolve_sidecar(workspace_root, None)?;
    let atomic_store =
        AtomicStore::load(&sidecar_path).map_err(|e| OpError::Other(format!("{}", e)))?;

    // Store-direct cross-ref orphan scan: free-prose §N references resolved
    // against the store. Labelled with the configured doc path so orphan-ledger
    // keys are unchanged from the parsed-markdown era.
    let orphan_doc_label = parsed_docs
        .first()
        .map(|(p, _)| p.clone())
        .unwrap_or_else(|| "atomic-store".to_string());
    let mut actual_orphan_keys: BTreeSet<(String, String, String)> = BTreeSet::new();
    for (from_section, to_target) in scan_store_prose_cross_ref_orphans(&atomic_store, &ws) {
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
    // Store-direct style: iterate the atomic store (the SSOT) rather than the
    // parsed markdown. Label violations with the configured doc path so output
    // is unchanged from the parsed-markdown era.
    let style_doc_label = parsed_docs
        .first()
        .map(|(p, _)| p.as_str())
        .unwrap_or("atomic-store");
    let style_violations: Vec<StyleViolation> =
        check_style_atomic(style_doc_label, &atomic_store, &ruleset);
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
    let mut id_set = workspace_section_id_set(&ws);
    id_set.extend(ws.atomic_id_set.iter().cloned());
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
            .filter_map(|e| match e {
                ValidationError::SupersedeMissingRef { section_id, .. } => Some(format!(
                    "§{} decision_status=Superseded but superseded_by is unset",
                    section_id
                )),
                _ => None,
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

    // Failure aggregation.
    let mut failure_reasons: Vec<String> = Vec::new();
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
        docs_loaded: parsed_docs.len(),
        docs_configured: loaded.config.workspace.docs.len(),
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
        let _ = writeln!(out, "docs={}/{}", self.docs_loaded, self.docs_configured);
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
