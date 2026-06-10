//! Round 297 — `redact_term` convenience primitive (RFC P1 final piece).
//!
//! Wraps the R295 publishable setters into a single pattern-and-replacement
//! scan over the publishable half of `AtomicChangelogEntry`. Routes only to
//! `publishable_*` (audit_* is system-immutable post-append); pairs with the
//! R296 `[[publishable_override_ledger]]` gate by emitting a ready-to-paste
//! ledger draft in the report so callers do not hand-author SHA256 anchors.
//!
//! Closes RFC G1 (no atomic primitive for cross-store term replacement) when
//! taken together with R294 schema split + R295 setters + R296 ledger gate.

use crate::{
    set_changelog_publishable_carry_forward_bullets, set_changelog_publishable_changes_bullets,
    set_changelog_publishable_decision_summary, set_changelog_publishable_impact_refs,
    set_changelog_publishable_verification_bullets, AtomicMutateError, AtomicStore,
};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RedactError {
    #[error("invalid regex `{pattern}`: {source}")]
    InvalidRegex {
        pattern: String,
        #[source]
        source: regex::Error,
    },
    #[error("empty pattern not allowed")]
    EmptyPattern,
    #[error("missing reason — redaction must be auditable")]
    MissingReason,
    #[error("missing applied_in — redaction must reference its originating round / commit")]
    MissingAppliedIn,
    #[error("mutate failure on `{entry_id}`: {source}")]
    Mutate {
        entry_id: String,
        #[source]
        source: AtomicMutateError,
    },
}

/// Match mode for the redaction pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedactMode {
    Literal,
    Regex,
}

/// Field scope filter. Each variant restricts the scan to one publishable
/// field; `All` scans every publishable_* field across every entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedactScope {
    All,
    DecisionSummary,
    ChangesBullets,
    VerificationBullets,
    ImpactRefs,
    CarryForwardBullets,
}

/// Input to `redact_term`. Constructed by the CLI handler from flags.
#[derive(Debug, Clone)]
pub struct RedactRequest {
    pub pattern: String,
    pub replacement: String,
    pub mode: RedactMode,
    pub case_insensitive: bool,
    pub scope: RedactScope,
    pub dry_run: bool,
    /// Required: written verbatim to the [[publishable_override_ledger]]
    /// draft so the audit trail explains *why*. RedactError::MissingReason
    /// if empty after trim.
    pub reason: String,
    /// Required: round id (or commit hash) that authorizes the redaction.
    /// Written to the ledger draft's `applied_in`.
    pub applied_in: String,
    /// Free-form classification (e.g. `"redaction"`, `"typo"`). Defaults to
    /// `"redaction"` at the CLI layer when unspecified.
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedactionHit {
    pub entry_id: String,
    pub field: &'static str,
    /// 0-based bullet index for vec fields; `None` for the scalar
    /// decision_summary.
    pub index: Option<usize>,
    /// Original substring(s) of the field that matched the pattern.
    /// Always present, even on dry_run.
    pub original: String,
    /// What the field would become / does become after replacement.
    pub redacted: String,
}

#[derive(Debug, Clone)]
pub struct RedactionReport {
    pub dry_run: bool,
    pub hits: Vec<RedactionHit>,
    /// One ledger draft TOML block per touched entry (regardless of
    /// dry_run). Caller pastes into `mnemosyne.toml` then re-runs
    /// validate-workspace; the R296 gate verifies the content_hash_after
    /// matches what we wrote.
    pub ledger_drafts: Vec<String>,
}

impl RedactionReport {
    pub fn touched_entries(&self) -> Vec<&str> {
        let mut out: Vec<&str> = self.hits.iter().map(|h| h.entry_id.as_str()).collect();
        out.sort();
        out.dedup();
        out
    }
}

/// Apply `redact_term` against the atomic store.
///
/// `dry_run = true`: returns the report without mutating or saving. The
/// ledger draft is computed against the *would-be* publishable hash so
/// callers can preview the exact ledger row before applying.
///
/// `dry_run = false`: applies the replacement field-by-field through the
/// R295 setters (so the same length validation + audit invariant apply),
/// then computes ledger drafts against the actual post-mutation state.
pub fn redact_term(
    store: &mut AtomicStore,
    sidecar_path: &Path,
    req: &RedactRequest,
) -> Result<RedactionReport, RedactError> {
    if req.pattern.trim().is_empty() {
        return Err(RedactError::EmptyPattern);
    }
    if req.reason.trim().is_empty() {
        return Err(RedactError::MissingReason);
    }
    if req.applied_in.trim().is_empty() {
        return Err(RedactError::MissingAppliedIn);
    }

    let matcher = build_matcher(req)?;
    let mut hits: Vec<RedactionHit> = Vec::new();
    // (entry_id, field, new_value_for_string, new_vec_for_bullets)
    // We accumulate proposed mutations first so dry_run can return a full
    // report without touching the store; non-dry_run then applies them.
    let mut planned_summary: Vec<(String, String)> = Vec::new();
    let mut planned_changes: Vec<(String, Vec<String>)> = Vec::new();
    let mut planned_verification: Vec<(String, Vec<String>)> = Vec::new();
    let mut planned_impact_refs: Vec<(String, Vec<String>)> = Vec::new();
    let mut planned_carry: Vec<(String, Vec<String>)> = Vec::new();

    for (entry_id, entry) in &store.changelog_entries {
        if scope_includes(req.scope, RedactScope::DecisionSummary) {
            if let Some(summary) = entry.publishable_decision_summary.as_deref() {
                let new_summary = matcher.replace_all(summary, &req.replacement);
                if new_summary != summary {
                    hits.push(RedactionHit {
                        entry_id: entry_id.clone(),
                        field: "publishable_decision_summary",
                        index: None,
                        original: summary.to_string(),
                        redacted: new_summary.clone(),
                    });
                    planned_summary.push((entry_id.clone(), new_summary));
                }
            }
        }
        if scope_includes(req.scope, RedactScope::ChangesBullets) {
            if let Some(new_vec) = redact_vec(
                &matcher,
                &req.replacement,
                &entry.publishable_changes_bullets,
                entry_id,
                "publishable_changes_bullets",
                &mut hits,
            ) {
                planned_changes.push((entry_id.clone(), new_vec));
            }
        }
        if scope_includes(req.scope, RedactScope::VerificationBullets) {
            if let Some(new_vec) = redact_vec(
                &matcher,
                &req.replacement,
                &entry.publishable_verification_bullets,
                entry_id,
                "publishable_verification_bullets",
                &mut hits,
            ) {
                planned_verification.push((entry_id.clone(), new_vec));
            }
        }
        if scope_includes(req.scope, RedactScope::ImpactRefs) {
            if let Some(new_vec) = redact_vec(
                &matcher,
                &req.replacement,
                &entry.publishable_impact_refs,
                entry_id,
                "publishable_impact_refs",
                &mut hits,
            ) {
                planned_impact_refs.push((entry_id.clone(), new_vec));
            }
        }
        if scope_includes(req.scope, RedactScope::CarryForwardBullets) {
            if let Some(new_vec) = redact_vec(
                &matcher,
                &req.replacement,
                &entry.publishable_carry_forward_bullets,
                entry_id,
                "publishable_carry_forward_bullets",
                &mut hits,
            ) {
                planned_carry.push((entry_id.clone(), new_vec));
            }
        }
    }

    if !req.dry_run {
        // Apply field by field through the R295 setters so validation +
        // save_with_receipt semantics are preserved. Each setter saves
        // independently — that matches the existing primitive boundary;
        // an in-flight failure leaves earlier mutations on disk, which is
        // the same behavior as a sequence of manual setter calls.
        for (entry_id, value) in planned_summary {
            set_changelog_publishable_decision_summary(store, sidecar_path, &entry_id, &value)
                .map_err(|e| RedactError::Mutate {
                    entry_id,
                    source: e,
                })?;
        }
        for (entry_id, vec) in planned_changes {
            set_changelog_publishable_changes_bullets(store, sidecar_path, &entry_id, &vec)
                .map_err(|e| RedactError::Mutate {
                    entry_id,
                    source: e,
                })?;
        }
        for (entry_id, vec) in planned_verification {
            set_changelog_publishable_verification_bullets(store, sidecar_path, &entry_id, &vec)
                .map_err(|e| RedactError::Mutate {
                    entry_id,
                    source: e,
                })?;
        }
        for (entry_id, vec) in planned_impact_refs {
            set_changelog_publishable_impact_refs(store, sidecar_path, &entry_id, &vec).map_err(
                |e| RedactError::Mutate {
                    entry_id,
                    source: e,
                },
            )?;
        }
        for (entry_id, vec) in planned_carry {
            set_changelog_publishable_carry_forward_bullets(store, sidecar_path, &entry_id, &vec)
                .map_err(|e| RedactError::Mutate {
                entry_id,
                source: e,
            })?;
        }
    }

    let touched: Vec<String> = {
        let mut s: Vec<String> = hits.iter().map(|h| h.entry_id.clone()).collect();
        s.sort();
        s.dedup();
        s
    };

    let ledger_drafts = touched
        .iter()
        .filter_map(|entry_id| {
            // For dry_run, simulate the post-mutation state inline so the
            // hash matches what the user will see after applying. For real
            // application, we already mutated above so the live entry is
            // post-mutation.
            let entry = if req.dry_run {
                let mut sim = store.changelog_entries.get(entry_id)?.clone();
                apply_planned_to_entry(&mut sim, entry_id, &hits, &req.replacement, &matcher);
                Some(sim)
            } else {
                store.changelog_entries.get(entry_id).cloned()
            }?;
            let touched_fields: Vec<String> = hits
                .iter()
                .filter(|h| &h.entry_id == entry_id)
                .map(|h| h.field.to_string())
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect();
            let content_hash_after = entry.publishable_hash_hex();
            let content_hash_before = entry.audit_hash_hex();
            Some(format_ledger_row(
                &req.kind,
                entry_id,
                &touched_fields,
                &req.reason,
                &req.applied_in,
                &content_hash_before,
                &content_hash_after,
            ))
        })
        .collect();

    Ok(RedactionReport {
        dry_run: req.dry_run,
        hits,
        ledger_drafts,
    })
}

// ============================================================================
// helpers — kept private; the public surface is RedactRequest + redact_term.
// ============================================================================

enum Matcher {
    Literal {
        needle: String,
        case_insensitive: bool,
    },
    Regex(regex::Regex),
}

impl Matcher {
    fn replace_all(&self, input: &str, replacement: &str) -> String {
        match self {
            Matcher::Literal {
                needle,
                case_insensitive,
            } => {
                if *case_insensitive {
                    // Case-insensitive literal replace via regex with
                    // escaped pattern — keeps literal-mode semantics
                    // (no special chars) while honoring the flag.
                    match regex::RegexBuilder::new(&regex::escape(needle))
                        .case_insensitive(true)
                        .build()
                    {
                        Ok(re) => re.replace_all(input, replacement).to_string(),
                        Err(_) => input.to_string(),
                    }
                } else {
                    input.replace(needle, replacement)
                }
            }
            Matcher::Regex(re) => re.replace_all(input, replacement).to_string(),
        }
    }
}

fn build_matcher(req: &RedactRequest) -> Result<Matcher, RedactError> {
    match req.mode {
        RedactMode::Literal => Ok(Matcher::Literal {
            needle: req.pattern.clone(),
            case_insensitive: req.case_insensitive,
        }),
        RedactMode::Regex => regex::RegexBuilder::new(&req.pattern)
            .case_insensitive(req.case_insensitive)
            .build()
            .map(Matcher::Regex)
            .map_err(|e| RedactError::InvalidRegex {
                pattern: req.pattern.clone(),
                source: e,
            }),
    }
}

fn scope_includes(active: RedactScope, target: RedactScope) -> bool {
    matches!(active, RedactScope::All) || active == target
}

fn redact_vec(
    matcher: &Matcher,
    replacement: &str,
    src: &[String],
    entry_id: &str,
    field: &'static str,
    hits: &mut Vec<RedactionHit>,
) -> Option<Vec<String>> {
    let mut new_vec = src.to_vec();
    let mut changed = false;
    for (i, bullet) in src.iter().enumerate() {
        let new_bullet = matcher.replace_all(bullet, replacement);
        if new_bullet != *bullet {
            hits.push(RedactionHit {
                entry_id: entry_id.to_string(),
                field,
                index: Some(i),
                original: bullet.clone(),
                redacted: new_bullet.clone(),
            });
            new_vec[i] = new_bullet;
            changed = true;
        }
    }
    if changed {
        Some(new_vec)
    } else {
        None
    }
}

fn apply_planned_to_entry(
    entry: &mut crate::AtomicChangelogEntry,
    entry_id: &str,
    hits: &[RedactionHit],
    replacement: &str,
    matcher: &Matcher,
) {
    for hit in hits.iter().filter(|h| h.entry_id == entry_id) {
        match hit.field {
            "publishable_decision_summary" => {
                if let Some(s) = entry.publishable_decision_summary.as_deref() {
                    entry.publishable_decision_summary = Some(matcher.replace_all(s, replacement));
                }
            }
            "publishable_changes_bullets" => {
                if let Some(i) = hit.index {
                    if let Some(b) = entry.publishable_changes_bullets.get_mut(i) {
                        *b = matcher.replace_all(b, replacement);
                    }
                }
            }
            "publishable_verification_bullets" => {
                if let Some(i) = hit.index {
                    if let Some(b) = entry.publishable_verification_bullets.get_mut(i) {
                        *b = matcher.replace_all(b, replacement);
                    }
                }
            }
            "publishable_impact_refs" => {
                if let Some(i) = hit.index {
                    if let Some(b) = entry.publishable_impact_refs.get_mut(i) {
                        *b = matcher.replace_all(b, replacement);
                    }
                }
            }
            "publishable_carry_forward_bullets" => {
                if let Some(i) = hit.index {
                    if let Some(b) = entry.publishable_carry_forward_bullets.get_mut(i) {
                        *b = matcher.replace_all(b, replacement);
                    }
                }
            }
            _ => {}
        }
    }
}

pub(crate) fn format_ledger_row(
    kind: &str,
    target_id: &str,
    fields: &[String],
    reason: &str,
    applied_in: &str,
    content_hash_before: &str,
    content_hash_after: &str,
) -> String {
    let fields_arr = fields
        .iter()
        .map(|f| format!("\"{}\"", f))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "[[publishable_override_ledger]]\n\
         kind = \"{}\"\n\
         target_id = \"{}\"\n\
         fields = [{}]\n\
         reason = \"{}\"\n\
         applied_in = \"{}\"\n\
         content_hash_before = \"{}\"\n\
         content_hash_after = \"{}\"\n",
        kind, target_id, fields_arr, reason, applied_in, content_hash_before, content_hash_after
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{append_changelog_entry, AtomicStore, ChangelogEntryDraft};
    use tempfile::TempDir;

    fn req_literal(pattern: &str, replacement: &str) -> RedactRequest {
        RedactRequest {
            pattern: pattern.to_string(),
            replacement: replacement.to_string(),
            mode: RedactMode::Literal,
            case_insensitive: false,
            scope: RedactScope::All,
            dry_run: false,
            reason: "test".to_string(),
            applied_in: "Round T".to_string(),
            kind: "redaction".to_string(),
        }
    }

    fn seed(store: &mut AtomicStore, path: &Path, entry_id: &str) {
        append_changelog_entry(
            store,
            path,
            ChangelogEntryDraft {
                entry_id,
                decision_summary: Some("XYZ123 leaked summary"),
                changes_bullets: &["XYZ123 in changes".into(), "clean change".into()],
                verification_bullets: &["XYZ123 in verify".into()],
                impact_refs: &["43".into()],
                carry_forward_bullets: &["XYZ123 in carry".into()],
            },
            "Round ",
        )
        .unwrap();
    }

    #[test]
    fn redact_term_dry_run_does_not_mutate() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed(&mut store, &path, "Round 999");

        let mut req = req_literal("XYZ123", "[REDACTED]");
        req.dry_run = true;
        let report = redact_term(&mut store, &path, &req).unwrap();

        assert!(report.dry_run);
        assert_eq!(report.hits.len(), 4); // summary + 1 of 2 changes + verify + carry
                                          // store untouched
        let entry = store.changelog_entries.get("Round 999").unwrap();
        assert_eq!(
            entry.publishable_decision_summary.as_deref(),
            Some("XYZ123 leaked summary"),
            "dry_run must not mutate"
        );
        // ledger draft computed against would-be hash
        assert_eq!(report.ledger_drafts.len(), 1);
        let draft = &report.ledger_drafts[0];
        assert!(draft.contains("[[publishable_override_ledger]]"));
        assert!(draft.contains("target_id = \"Round 999\""));
        assert!(draft.contains("kind = \"redaction\""));
    }

    #[test]
    fn redact_term_apply_mutates_publishable_only() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed(&mut store, &path, "Round 999");

        let req = req_literal("XYZ123", "[REDACTED]");
        let report = redact_term(&mut store, &path, &req).unwrap();
        assert!(!report.dry_run);
        assert_eq!(report.hits.len(), 4);

        let entry = store.changelog_entries.get("Round 999").unwrap();
        // publishable mutated
        assert_eq!(
            entry.publishable_decision_summary.as_deref(),
            Some("[REDACTED] leaked summary")
        );
        assert_eq!(
            entry.publishable_changes_bullets[0],
            "[REDACTED] in changes"
        );
        assert_eq!(entry.publishable_changes_bullets[1], "clean change");
        // audit untouched
        assert_eq!(
            entry.decision_summary.as_deref(),
            Some("XYZ123 leaked summary")
        );
        assert_eq!(entry.changes_bullets[0], "XYZ123 in changes");
        assert!(!entry.publishable_matches_audit());
    }

    #[test]
    fn redact_term_scope_filter_restricts_fields() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed(&mut store, &path, "Round 999");

        let mut req = req_literal("XYZ123", "[REDACTED]");
        req.scope = RedactScope::DecisionSummary;
        let report = redact_term(&mut store, &path, &req).unwrap();
        assert_eq!(report.hits.len(), 1);
        assert_eq!(report.hits[0].field, "publishable_decision_summary");

        let entry = store.changelog_entries.get("Round 999").unwrap();
        assert_eq!(
            entry.publishable_decision_summary.as_deref(),
            Some("[REDACTED] leaked summary")
        );
        // bullets untouched
        assert_eq!(entry.publishable_changes_bullets[0], "XYZ123 in changes");
    }

    #[test]
    fn redact_term_regex_mode_with_case_insensitive() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        append_changelog_entry(
            &mut store,
            &path,
            ChangelogEntryDraft {
                entry_id: "Round 999",
                decision_summary: Some("EMAIL: foo@example.com leaked"),
                changes_bullets: &["another foo@example.com cite".into()],
                verification_bullets: &["v".into()],
                impact_refs: &[],
                carry_forward_bullets: &["c".into()],
            },
            "Round ",
        )
        .unwrap();
        let mut req = req_literal(r"\b\w+@\w+\.\w+\b", "[EMAIL]");
        req.mode = RedactMode::Regex;
        req.case_insensitive = true;
        let report = redact_term(&mut store, &path, &req).unwrap();
        assert_eq!(report.hits.len(), 2);
        let entry = store.changelog_entries.get("Round 999").unwrap();
        assert_eq!(
            entry.publishable_decision_summary.as_deref(),
            Some("EMAIL: [EMAIL] leaked")
        );
    }

    #[test]
    fn redact_term_idempotent_after_apply() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed(&mut store, &path, "Round 999");
        let req = req_literal("XYZ123", "[REDACTED]");
        let _ = redact_term(&mut store, &path, &req).unwrap();
        // second run: pattern no longer present in publishable_*, so no hits
        let report2 = redact_term(&mut store, &path, &req).unwrap();
        assert!(
            report2.hits.is_empty(),
            "idempotent: re-running redact with same pattern is a no-op"
        );
    }

    #[test]
    fn redact_term_rejects_empty_pattern_reason_applied_in() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed(&mut store, &path, "Round 999");

        let mut req = req_literal("", "x");
        assert!(matches!(
            redact_term(&mut store, &path, &req).unwrap_err(),
            RedactError::EmptyPattern
        ));

        req.pattern = "XYZ123".into();
        req.reason = "".into();
        assert!(matches!(
            redact_term(&mut store, &path, &req).unwrap_err(),
            RedactError::MissingReason
        ));

        req.reason = "ok".into();
        req.applied_in = "  ".into();
        assert!(matches!(
            redact_term(&mut store, &path, &req).unwrap_err(),
            RedactError::MissingAppliedIn
        ));
    }

    #[test]
    fn redact_term_invalid_regex_surfaces() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed(&mut store, &path, "Round 999");
        let mut req = req_literal("[unclosed", "x");
        req.mode = RedactMode::Regex;
        assert!(matches!(
            redact_term(&mut store, &path, &req).unwrap_err(),
            RedactError::InvalidRegex { .. }
        ));
    }

    #[test]
    fn redact_term_ledger_draft_hash_matches_post_apply_hash() {
        // After applying, the printed content_hash_after must equal the
        // entry's publishable_hash_hex() — that's the contract that lets
        // the R296 gate verify the ledger row.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".atomic/workspace.atomic.json");
        let mut store = AtomicStore::new();
        seed(&mut store, &path, "Round 999");
        let req = req_literal("XYZ123", "[REDACTED]");
        let report = redact_term(&mut store, &path, &req).unwrap();
        assert_eq!(report.ledger_drafts.len(), 1);
        let post_hash = store
            .changelog_entries
            .get("Round 999")
            .unwrap()
            .publishable_hash_hex();
        assert!(
            report.ledger_drafts[0].contains(&format!("content_hash_after = \"{}\"", post_hash)),
            "ledger draft must anchor to post-apply publishable hash; draft:\n{}",
            report.ledger_drafts[0]
        );
    }
}
