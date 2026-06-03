//! `generate-docs` library op (render the atomic store to GENERATED.md).

use std::path::Path;

use crate::cascade::{render_atomic_store_to_md, resolve_output, write_generated_md};
use serde::Serialize;

use crate::{resolve_sidecar, OpError};

#[derive(Debug, Clone, Serialize)]
pub struct GenerateDocsReport {
    pub sidecar_path: String,
    pub output_path: String,
    pub sections_rendered: usize,
    pub entries_rendered: usize,
    pub written_bytes: usize,
}

/// Render atomic store → markdown bytes → write to output path. Returns
/// a structured report (no printing). Used by both the CLI bin's
/// `generate-docs` subcommand and the MCP server's `generate_docs` tool.
pub fn generate_docs(
    workspace_root: &Path,
    sidecar: Option<&Path>,
    output: Option<&Path>,
) -> Result<GenerateDocsReport, OpError> {
    let sidecar_path = resolve_sidecar(workspace_root, sidecar)?;
    let output_path = match output {
        Some(p) if p.is_absolute() => p.to_path_buf(),
        Some(p) => workspace_root.join(p),
        None => resolve_output(workspace_root, None)?,
    };
    let (content, store) = render_atomic_store_to_md(workspace_root, &sidecar_path)
        .map_err(|e| OpError::Other(format!("{:#}", e)))?;
    write_generated_md(&output_path, &content).map_err(|e| OpError::Other(format!("{:#}", e)))?;
    Ok(GenerateDocsReport {
        sidecar_path: sidecar_path.display().to_string(),
        output_path: output_path.display().to_string(),
        sections_rendered: store.sections.len(),
        entries_rendered: store.changelog_entries.len(),
        written_bytes: content.len(),
    })
}
