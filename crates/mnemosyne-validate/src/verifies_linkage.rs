//! R426 — authoritative test-catalog linkage check (SCE field-report P2 + the
//! P5 granularity lint).
//!
//! A consumer-generated CATALOG declares, per test artifact, the section(s)
//! its authoritative metadata targets (e.g. the W3C `metadata.txt` `specnum`
//! field). This scan validates every `verifies` binding against it
//! deterministically: the bound section must be among the artifact's declared
//! targets. This is the validity rung the existence-only verify axis (R413)
//! lacks — in the SCE episode it would have rejected all 25 cross-family
//! mismatches at commit time, with no model and no per-verdict error.
//!
//! Boundary (design sec 2.6): Mnemosyne takes only this neutral JSON contract;
//! parsing domain formats (`metadata.txt`, …) into the catalog is the
//! consumer's tooling (precedent: medium-forge). Uncataloged artifacts are a
//! COUNT, not a violation — a partial catalog is legitimate (it validates what
//! it knows).

use std::collections::BTreeMap;
use std::path::Path;

use mnemosyne_core::{AtomicSnapshot, BindingKind, DecisionStatus};
use serde::Deserialize;

/// One catalog row: a test artifact and the section(s) its authoritative
/// metadata declares it targets. Extra JSON fields are ignored (lenient,
/// the epub-anchor-map precedent).
#[derive(Debug, Clone, Deserialize)]
pub struct CatalogEntry {
    pub file: String,
    #[serde(default)]
    pub symbol: Option<String>,
    pub section_ids: Vec<String>,
}

/// The `verifies-catalog/v1` contract — consumer-generated.
#[derive(Debug, Clone, Deserialize)]
pub struct VerifiesCatalog {
    pub entries: Vec<CatalogEntry>,
}

/// How a binding disagrees with the catalog (the P5 lint distinction).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MismatchKind {
    /// Bound to a CHILD of a declared section (e.g. bound `6.4.1`, declared
    /// `6.4`) — claiming finer granularity than the authoritative source
    /// supports. The structural root of blanket-binding (SCE P5).
    FinerThanDeclared,
    /// Bound to a section outside the declared family entirely.
    Cross,
}

impl MismatchKind {
    pub fn as_str(self) -> &'static str {
        match self {
            MismatchKind::FinerThanDeclared => "finer_than_declared",
            MismatchKind::Cross => "cross",
        }
    }
}

/// One `verifies` binding that contradicts the catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkageMismatch {
    pub section_id: String,
    pub file: String,
    pub symbol: Option<String>,
    /// What the catalog declares for this artifact (sorted).
    pub declared: Vec<String>,
    pub kind: MismatchKind,
}

/// Scan result: mismatches gate (at the configured severity); uncataloged
/// artifacts are surfaced as a count only.
#[derive(Debug, Clone, Default)]
pub struct LinkageReport {
    pub mismatches: Vec<LinkageMismatch>,
    /// `verifies`-bound artifacts with no catalog entry — not gating (a
    /// partial catalog is legitimate); surfaced so a total-catalog consumer
    /// can watch for 0.
    pub uncataloged: usize,
    /// Total `verifies` bindings examined (non-`Removed` sections).
    pub examined: usize,
}

/// Load a catalog from disk (JSON, lenient on unknown fields).
pub fn load_catalog(path: &Path) -> Result<VerifiesCatalog, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| format!("verifies-catalog read {}: {}", path.display(), e))?;
    serde_json::from_str(&raw)
        .map_err(|e| format!("verifies-catalog parse {}: {}", path.display(), e))
}

/// Whether the catalog declares `section_id` as a target of `(file, symbol)` —
/// EXACT match only (the R427 confirmed-rule catalog-live branch keys off
/// this; a finer-than-declared binding does NOT confirm).
pub fn catalog_declares(
    catalog: &VerifiesCatalog,
    file: &str,
    symbol: Option<&str>,
    section_id: &str,
) -> bool {
    catalog.entries.iter().any(|e| {
        e.file == file
            && e.symbol.as_deref() == symbol
            && e.section_ids.iter().any(|s| s == section_id)
    })
}

/// Validate every `verifies` binding against the catalog. A binding matches
/// iff its section is EXACTLY one of the artifact's declared sections; a child
/// of a declared section is `FinerThanDeclared` (the P5 granularity lint),
/// anything else is `Cross`. `Removed` sections are excluded (tombstones).
pub fn scan_verifies_linkage(
    snapshot: &AtomicSnapshot,
    catalog: &VerifiesCatalog,
) -> LinkageReport {
    let mut declared_of: BTreeMap<(&str, Option<&str>), Vec<&str>> = BTreeMap::new();
    for e in &catalog.entries {
        declared_of
            .entry((e.file.as_str(), e.symbol.as_deref()))
            .or_default()
            .extend(e.section_ids.iter().map(String::as_str));
    }
    let mut report = LinkageReport::default();
    for (section_id, section) in &snapshot.sections {
        let removed =
            section.decision_status.unwrap_or(DecisionStatus::Active) == DecisionStatus::Removed;
        if removed {
            continue;
        }
        for b in &section.bindings {
            if !matches!(b.kind, BindingKind::Verifies) {
                continue;
            }
            report.examined += 1;
            let Some(declared) = declared_of.get(&(b.file.as_str(), b.symbol.as_deref())) else {
                report.uncataloged += 1;
                continue;
            };
            if declared.contains(&section_id.as_str()) {
                continue; // exact match — valid
            }
            let finer = declared
                .iter()
                .any(|d| section_id.starts_with(&format!("{}.", d)));
            let mut declared_sorted: Vec<String> = declared.iter().map(|s| s.to_string()).collect();
            declared_sorted.sort_unstable();
            declared_sorted.dedup();
            report.mismatches.push(LinkageMismatch {
                section_id: section_id.clone(),
                file: b.file.clone(),
                symbol: b.symbol.clone(),
                declared: declared_sorted,
                kind: if finer {
                    MismatchKind::FinerThanDeclared
                } else {
                    MismatchKind::Cross
                },
            });
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use mnemosyne_core::{BindingRef, SectionView};

    fn snap(bindings: &[(&str, &str)]) -> AtomicSnapshot {
        // (section_id, file) pairs, all symbol-less verifies bindings.
        let mut s = AtomicSnapshot::default();
        for &(sec, file) in bindings {
            s.sections
                .entry(sec.to_string())
                .or_insert_with(|| SectionView {
                    bindings: vec![],
                    decision_status: None,
                    coverage_expectation: Default::default(),
                    verification_expectation: Default::default(),
                })
                .bindings
                .push(BindingRef {
                    file: file.to_string(),
                    symbol: None,
                    kind: BindingKind::Verifies,
                });
        }
        s
    }

    fn catalog(entries: &[(&str, &[&str])]) -> VerifiesCatalog {
        VerifiesCatalog {
            entries: entries
                .iter()
                .map(|&(file, secs)| CatalogEntry {
                    file: file.to_string(),
                    symbol: None,
                    section_ids: secs.iter().map(|s| s.to_string()).collect(),
                })
                .collect(),
        }
    }

    #[test]
    fn linkage_classifies_match_finer_cross_and_uncataloged() {
        // Test215 declared for 6.4; bound to 6.4 (ok), 6.4.1 (finer), 3.13
        // (cross). Test999 has no catalog entry (uncataloged, not gating).
        let s = snap(&[
            ("6.4", "t/Test215.h"),
            ("6.4.1", "t/Test215.h"),
            ("3.13", "t/Test215.h"),
            ("5.1", "t/Test999.h"),
        ]);
        let c = catalog(&[("t/Test215.h", &["6.4"])]);
        let r = scan_verifies_linkage(&s, &c);
        assert_eq!(r.examined, 4);
        assert_eq!(r.uncataloged, 1, "Test999 counted, not a mismatch");
        assert_eq!(r.mismatches.len(), 2);
        let kind_of = |sec: &str| {
            r.mismatches
                .iter()
                .find(|m| m.section_id == sec)
                .map(|m| m.kind)
        };
        assert_eq!(kind_of("6.4.1"), Some(MismatchKind::FinerThanDeclared));
        assert_eq!(kind_of("3.13"), Some(MismatchKind::Cross));
        assert_eq!(kind_of("6.4"), None, "exact match is valid");
    }
}
