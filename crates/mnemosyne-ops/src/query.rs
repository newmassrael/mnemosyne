//! Read-only query ops. Each function loads the atomic store (the SSOT) and
//! returns structured data without printing — the CLI bin formats for stdout,
//! the MCP server serializes to JSON.

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{anyhow, Result};
use mnemosyne_atomic::AtomicStore;
use mnemosyne_config::{discover_config, LoadedConfig};
use mnemosyne_query::{
    build_envelope, changelog_entries_for_section, query_term as query_term_inner,
    related_sections_with_atomic, section_by_id, ChangelogEntryView, QueryEnvelope,
    RelatedSections, SectionView, TermHit, TermMode, TermQuery, TermScope,
};
use serde::Serialize;

use crate::{load_atomic_store, OpError};

#[derive(Debug, Clone, Copy)]
pub enum QuerySectionMode {
    Brief,
    WithRelated,
    Envelope,
}

#[derive(Debug, Clone, Serialize)]
pub struct ListSectionsReport {
    pub section_ids: Vec<String>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct InventoryEntryView {
    pub id: String,
    pub status: &'static str,
    pub section_ref: Option<String>,
    pub source: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum QuerySectionPayload {
    Brief(SectionView),
    WithRelated {
        section: SectionView,
        related: RelatedSections,
        changelog: Vec<ChangelogEntryView>,
    },
    Envelope(QueryEnvelope),
}

#[derive(Debug, Clone)]
pub struct QueryTermInput {
    pub pattern: String,
    pub regex: bool,
    pub case_insensitive: bool,
    pub scope: Option<String>,
    pub fields: Vec<String>,
}

/// Load the config + the atomic store (the SSOT). Lives in the lib so MCP
/// can call it without spawning the bin.
pub fn load_workspace(anchor: &Path) -> Result<(LoadedConfig, AtomicStore)> {
    let loaded = discover_config(anchor)
        .map_err(|e| anyhow!("mnemosyne.toml load failed: {}", e))?
        .ok_or_else(|| anyhow!("mnemosyne.toml not found in {}", anchor.display()))?;
    let atomic_store = load_atomic_store(anchor, None)?;
    Ok((loaded, atomic_store))
}

/// Atomic-store section ids (the canonical visible set). Returned in
/// BTreeSet order.
pub fn list_sections(workspace_root: &Path) -> Result<ListSectionsReport, OpError> {
    let (_, atomic_store) = load_workspace(workspace_root).map_err(OpError::from)?;
    let set = atomic_store.atomic_section_id_set();
    let total = set.len();
    Ok(ListSectionsReport {
        section_ids: set.into_iter().collect(),
        total,
    })
}

/// Look up a single section. `mode` controls how much neighborhood data
/// to include; the returned payload is JSON-ready.
pub fn query_section(
    workspace_root: &Path,
    section_id: &str,
    mode: QuerySectionMode,
) -> Result<QuerySectionPayload, OpError> {
    let (_, atomic_store) = load_workspace(workspace_root).map_err(OpError::from)?;
    let id = mnemosyne_core::strip_section_marker(section_id);
    match mode {
        QuerySectionMode::Brief => {
            let view = section_by_id(&atomic_store, id)
                .ok_or_else(|| OpError::Other(format!("section_id `{}` not found in store", id)))?;
            Ok(QuerySectionPayload::Brief(view))
        }
        QuerySectionMode::WithRelated => {
            let view = section_by_id(&atomic_store, id)
                .ok_or_else(|| OpError::Other(format!("section_id `{}` not found in store", id)))?;
            let related = related_sections_with_atomic(&atomic_store, id);
            let changelog = changelog_entries_for_section(&atomic_store, id);
            Ok(QuerySectionPayload::WithRelated {
                section: view,
                related,
                changelog,
            })
        }
        QuerySectionMode::Envelope => {
            let envelope = build_envelope(&atomic_store, id)
                .ok_or_else(|| OpError::Other(format!("section_id `{}` not found in store", id)))?;
            Ok(QuerySectionPayload::Envelope(envelope))
        }
    }
}

/// Literal/regex search across atomic Section + ChangelogEntry +
/// Inventory text fields (R292).
pub fn query_term(workspace_root: &Path, input: &QueryTermInput) -> Result<Vec<TermHit>, OpError> {
    let atomic_store = load_atomic_store(workspace_root, None)?;
    let scope = match input.scope.as_deref().unwrap_or("all") {
        "all" => TermScope::All,
        "sections" => TermScope::Sections,
        "changelog" | "changelog-entries" => TermScope::ChangelogEntries,
        "inventory" => TermScope::Inventory,
        other => {
            return Err(OpError::Other(format!(
                "scope must be one of all|sections|changelog|inventory (got `{}`)",
                other
            )))
        }
    };
    let field_filter = if input.fields.is_empty() {
        None
    } else {
        Some(input.fields.iter().cloned().collect::<BTreeSet<_>>())
    };
    let q = TermQuery {
        pattern: input.pattern.clone(),
        mode: if input.regex {
            TermMode::Regex
        } else {
            TermMode::Literal
        },
        case_insensitive: input.case_insensitive,
        scope,
        field_filter,
    };
    query_term_inner(&atomic_store, &q).map_err(|e| OpError::Other(format!("{}", e)))
}

/// All inventory entries from the atomic store (R273).
pub fn list_inventory(workspace_root: &Path) -> Result<Vec<InventoryEntryView>, OpError> {
    let store = load_atomic_store(workspace_root, None)?;
    Ok(store
        .inventory_entries
        .iter()
        .map(|(id, e)| InventoryEntryView {
            id: id.clone(),
            status: e.status.as_str(),
            section_ref: e.section_ref.clone(),
            source: e.source.clone(),
            reason: e.reason.clone(),
        })
        .collect())
}

/// Single inventory entry lookup.
pub fn query_inventory(
    workspace_root: &Path,
    inventory_id: &str,
) -> Result<InventoryEntryView, OpError> {
    let store = load_atomic_store(workspace_root, None)?;
    let entry = store.inventory(inventory_id).ok_or_else(|| {
        OpError::Other(format!(
            "inventory_id `{}` not present in atomic store",
            inventory_id
        ))
    })?;
    Ok(InventoryEntryView {
        id: inventory_id.to_string(),
        status: entry.status.as_str(),
        section_ref: entry.section_ref.clone(),
        source: entry.source.clone(),
        reason: entry.reason.clone(),
    })
}
