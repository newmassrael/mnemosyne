//! `style-check` library op. Returns structured T3/T4 style violations
//! across all configured docs (or a single doc when filtered).

use std::path::Path;

use mnemosyne_atomic::AtomicStore;
use mnemosyne_style::{
    check_style, default_ruleset_with_config, StyleSeverity, StyleViolation,
};
use serde::Serialize;

use super::{query::load_workspace, resolve_sidecar, OpError};

#[derive(Debug, Clone)]
pub struct StyleCheckInput {
    /// Optional path relative to workspace root. None = check every doc.
    pub doc: Option<String>,
    /// Severity filter: "t3" / "t4" / "all" (default).
    pub severity: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StyleCheckReport {
    pub doc_filter: Option<String>,
    pub severity_filter: String,
    pub violations: Vec<StyleViolationView>,
    pub t3_reject: usize,
    pub t3_warn: usize,
    pub t4_info: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct StyleViolationView {
    pub doc_path: String,
    pub section_id: String,
    pub rule_id: String,
    pub severity: &'static str,
    pub message: String,
    pub line: usize,
}

pub fn style_check(
    workspace_root: &Path,
    input: &StyleCheckInput,
) -> Result<StyleCheckReport, OpError> {
    let (ws, loaded, _) = load_workspace(workspace_root).map_err(OpError::from)?;
    let ruleset = default_ruleset_with_config(
        loaded.config.style.as_ref(),
        loaded.config.terminology.as_ref(),
    );
    let sidecar_path = resolve_sidecar(workspace_root, None);
    let atomic = AtomicStore::load(&sidecar_path).unwrap_or_default();

    let mut all: Vec<StyleViolation> = Vec::new();
    for (path, parsed) in &ws.docs {
        if let Some(ref filter) = input.doc {
            if filter != path {
                continue;
            }
        }
        let mut v = check_style(path, parsed, &atomic, &ruleset);
        all.append(&mut v);
    }

    let severity_filter = input.severity.clone().unwrap_or_else(|| "all".to_string());
    let mut t3_reject = 0usize;
    let mut t3_warn = 0usize;
    let mut t4_info = 0usize;
    let mut filtered: Vec<StyleViolationView> = Vec::new();
    for v in all.iter() {
        let is_reject = v.rule_id == "terminology_consistency";
        let sev_label: &'static str = match (v.severity, is_reject) {
            (StyleSeverity::Warn, true) => "reject",
            (StyleSeverity::Warn, false) => "warn",
            (StyleSeverity::Info, _) => "info",
        };
        match sev_label {
            "reject" => t3_reject += 1,
            "warn" => t3_warn += 1,
            "info" => t4_info += 1,
            _ => {}
        }
        let include = match severity_filter.as_str() {
            "t3" => sev_label == "reject" || sev_label == "warn",
            "t4" => sev_label == "info",
            _ => true,
        };
        if include {
            filtered.push(StyleViolationView {
                doc_path: v.doc_path.clone(),
                section_id: v.section_id.clone(),
                rule_id: v.rule_id.clone(),
                severity: sev_label,
                message: v.message.clone(),
                line: v.line_anchor.unwrap_or(0),
            });
        }
    }

    Ok(StyleCheckReport {
        doc_filter: input.doc.clone(),
        severity_filter,
        violations: filtered,
        t3_reject,
        t3_warn,
        t4_info,
    })
}
