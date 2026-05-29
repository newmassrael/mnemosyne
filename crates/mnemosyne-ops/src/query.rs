//! Read-only query ops. Each function loads the workspace (markdown +
//! atomic store) and returns structured data without printing — the CLI
//! bin formats for stdout, the MCP server serializes to JSON.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use mnemosyne_atomic::AtomicStore;
use mnemosyne_config::{discover_config, LoadedConfig, SchemaSection};
use mnemosyne_parser::parse_markdown_with_schema;
use mnemosyne_query::{
    build_envelope, changelog_entries_for_section, query_term as query_term_inner,
    related_sections_with_atomic, section_by_id, workspace_section_id_set, ChangelogEntryView,
    QueryEnvelope, RelatedSections, SectionView, TermHit, TermMode, TermQuery, TermScope,
};
use mnemosyne_schema::ParsedDoc;
use mnemosyne_workspace::Workspace;
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

/// Load the markdown workspace + the atomic store. Mirrors
/// `cli::load_workspace` but lives in the lib so MCP can call it
/// without spawning the bin.
pub fn load_workspace(workspace_root: &Path) -> Result<(Workspace, LoadedConfig, AtomicStore)> {
    let loaded = discover_config(workspace_root)
        .map_err(|e| anyhow!("mnemosyne.toml load failed: {}", e))?
        .ok_or_else(|| anyhow!("mnemosyne.toml not found in {}", workspace_root.display()))?;
    let schema = loaded
        .config
        .schema
        .clone()
        .unwrap_or_else(SchemaSection::mnemosyne_preset);
    let atomic_store = load_atomic_store(workspace_root, None)?;
    let mut ws = Workspace::from_config(&loaded);
    ws.set_atomic_id_set(atomic_store.atomic_section_id_set());
    let docs: Vec<String> = loaded.doc_paths().map(|s| s.to_string()).collect();
    for path in &docs {
        let abs = workspace_root.join(path);
        let content =
            std::fs::read_to_string(&abs).with_context(|| format!("read {}", abs.display()))?;
        let parsed = parse_markdown_with_schema(&content, path, &schema);
        ws.insert(path.clone(), parsed);
    }
    Ok((ws, loaded, atomic_store))
}

/// Workspace + atomic-store union of section ids (the canonical visible
/// set, post-7-md-deletion). Returned in BTreeSet order.
pub fn list_sections(workspace_root: &Path) -> Result<ListSectionsReport, OpError> {
    let (ws, _, atomic_store) = load_workspace(workspace_root).map_err(OpError::from)?;
    let mut set = workspace_section_id_set(&ws);
    set.extend(atomic_store.atomic_section_id_set());
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
    let (ws, _, atomic_store) = load_workspace(workspace_root).map_err(OpError::from)?;
    let id = mnemosyne_core::strip_section_marker(section_id);
    match mode {
        QuerySectionMode::Brief => {
            let view = section_by_id(&ws, &atomic_store, id).ok_or_else(|| {
                OpError::Other(format!("section_id `{}` not found in workspace", id))
            })?;
            Ok(QuerySectionPayload::Brief(view))
        }
        QuerySectionMode::WithRelated => {
            let view = section_by_id(&ws, &atomic_store, id).ok_or_else(|| {
                OpError::Other(format!("section_id `{}` not found in workspace", id))
            })?;
            let related = related_sections_with_atomic(&ws, &atomic_store, id);
            let changelog = changelog_entries_for_section(&ws, &atomic_store, id);
            Ok(QuerySectionPayload::WithRelated {
                section: view,
                related,
                changelog,
            })
        }
        QuerySectionMode::Envelope => {
            let envelope = build_envelope(&ws, &atomic_store, id).ok_or_else(|| {
                OpError::Other(format!("section_id `{}` not found in workspace", id))
            })?;
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

/// Parse + load all configured docs and return the per-doc ParsedDoc map,
/// reusing the workspace loader so MCP/CLI share the same code path.
pub fn parsed_docs(workspace_root: &Path) -> Result<BTreeMap<String, ParsedDoc>, OpError> {
    let (ws, _, _) = load_workspace(workspace_root).map_err(OpError::from)?;
    Ok(ws.docs.clone())
}
