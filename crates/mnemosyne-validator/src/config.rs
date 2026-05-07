//! Workspace config — `mnemosyne.toml` schema + load + discovery (Round 142
//! WORKSPACE-CONFIG-ABSTRACTION, Phase 0e generic library extraction).
//!
//! Phase 0e framing reset (Round 141): Mnemosyne is *LLM-driven MD management
//! infrastructure for any codebase*, not a project-specific tool. The
//! workspace path list / default cross-doc target / repo root that used to be
//! hardcoded in `WORKSPACE_DOC_PATHS` / `MNEMOSYNE_DEFAULT_DOC` are pulled out
//! into a TOML file an external user authors.
//!
//! ## Schema
//!
//! ```toml
//! [workspace]
//! docs = ["docs/DESIGN.md", "docs/ARCHITECTURE.md", "README.md"]
//! default_doc = "docs/DESIGN.md" # optional
//! root = "."  # optional, default = file's dir
//! ```
//!
//! ## Discovery
//!
//! `discover_config(start)` walks from `start` upward looking for
//! `mnemosyne.toml` (or `.mnemosyne/config.toml`) — same pattern as git. Returns the
//! parsed config + the directory it was found in (= workspace root).

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Top-level workspace config schema, mapping 1:1 to TOML tables.
///
/// `[workspace]` is required. `[schema]`, `[style]`, `[terminology]` are
/// optional — when omitted, callers fall back to preset defaults
/// (`mnemosyne_preset` for this codebase, `generic_default` for external
/// generic-markdown users).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceConfig {
 pub workspace: WorkspaceSection,
 #[serde(default)]
 pub schema: Option<SchemaSection>,
 #[serde(default)]
 pub style: Option<StyleSection>,
 #[serde(default)]
 pub terminology: Option<TerminologySection>,
}

/// `[style]` table — locale + threshold overrides for T3/T4 style rules
/// (Round 145 STYLE-RULE-I18N).
///
/// `locale` selects the sentence-boundary handler (Korean / Japanese /
/// Chinese / English). `thresholds` lets external users override per-rule
/// char count caps without forking the validator.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct StyleSection {
 /// Locale tag for sentence boundary recognition.
 /// Recognized values: `"ko"` (default), `"ja"`, `"zh"`, `"en"`.
 /// Unknown values fall back to `"en"`.
 #[serde(default = "default_locale")]
 pub locale: String,

 /// Per-rule char count overrides. Keys must match StyleRule rule_id
 /// (`"max_sentence_length"`, `"max_paragraph_length"`,
 /// `"max_section_body_length"`). Missing keys fall back to compile-time
 /// defaults.
 #[serde(default)]
 pub thresholds: std::collections::BTreeMap<String, u32>,
}

/// `[terminology]` table — workspace-wide glossary of canonical terms +
/// non-canonical variants the parser should warn about (Round 145).
///
/// Schema: each `[terminology.glossary]` row maps a canonical form to a
/// list of non-canonical variants. The Mnemosyne preset registers
/// `Salsa`/`salsa` and `bi-temporal`/`bitemporal`; external users add
/// project-specific terms.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TerminologySection {
 /// canonical → list of variants. e.g.
 /// `{ "Salsa": ["salsa"], "bi-temporal": ["bitemporal"] }`.
 #[serde(default)]
 pub glossary: std::collections::BTreeMap<String, Vec<String>>,
}

fn default_locale() -> String {
 "ko".to_string()
}

/// `[schema]` table — markdown-to-entity mapping config (Round 143).
///
/// The 4 entity types (Section / CrossRef / ChangelogEntry / FrozenList)
/// are fixed primitives; this section configures *which markdown patterns*
/// the parser maps onto them. External users override via
/// `mnemosyne.toml::[schema]`; the Mnemosyne self-application registers
/// its `design_doc` preset here as the first dogfood consumer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SchemaSection {
 /// Heading titles that mark a `ChangelogEntry` container section.
 /// Default = `["Changelog", "Changelog", "changelog"]` (Mnemosyne preset).
 /// Generic markdown users typically set `["Changelog"]`.
 #[serde(default = "default_changelog_titles")]
 pub changelog_titles: Vec<String>,

 /// Round 144 — string prefix that opens a ChangelogEntry top bullet.
 /// Mnemosyne preset = `"Round "`; ADR preset = `"ADR-"`; Round preset =
 /// `"Round "`; Decision preset = `"Decision "`. The parser extracts
 /// digits (with `.` separator) immediately after this prefix as the
 /// numeric portion of `entry_id`; the full entry_id includes the prefix
 /// (e.g., `"Round 33.5"`, `"ADR-0042"`).
 #[serde(default = "default_entry_id_prefix")]
 pub entry_id_prefix: String,

 /// Round 144 — anchor convention placeholder. The Mnemosyne preset is
 /// `"section_number"` (legacy `§N` literal). External users can label
 /// their convention here for diagnostics; deeper anchor-pattern wiring
 /// (heading anchor / ADR-NNNN / custom regex parser) is a Round 145+
 /// concern and the parser still derives section_id by the legacy rules.
 #[serde(default = "default_anchor_convention")]
 pub anchor_convention: String,

 /// Diagnostic label for this schema (e.g. `"design_doc"`, `"generic"`,
 /// `"adr"`). Carried through MutateReceipt + tracing spans for
 /// Cross-medium debugging. No semantic effect on parsing.
 #[serde(default = "default_medium_name")]
 pub medium_name: String,
}

fn default_changelog_titles() -> Vec<String> {
 vec![
 "Changelog".to_string(),
 "Changelog".to_string(),
 "changelog".to_string(),
 ]
}

fn default_entry_id_prefix() -> String {
 "Round ".to_string()
}

fn default_anchor_convention() -> String {
 "section_number".to_string()
}

fn default_medium_name() -> String {
 "design_doc".to_string()
}

impl SchemaSection {
 /// Mnemosyne self-application preset — design_doc medium with the
 /// existing Changelog / Changelog title set.
 pub fn mnemosyne_preset() -> Self {
 Self {
 changelog_titles: default_changelog_titles(),
 entry_id_prefix: default_entry_id_prefix(),
 anchor_convention: default_anchor_convention(),
 medium_name: "design_doc".to_string(),
 }
 }

 /// Generic markdown preset — only "Changelog" (case-insensitive)
 /// recognized; medium_name = `"generic"`. Use this for an external
 /// project that does not author its own `[schema]` block.
 pub fn generic_default() -> Self {
 Self {
 changelog_titles: vec!["Changelog".to_string(), "changelog".to_string()],
 // Generic markdown rarely numbers changelog entries; an empty
 // prefix means the parser disables numeric entry_id capture.
 entry_id_prefix: String::new(),
 anchor_convention: "heading_slug".to_string(),
 medium_name: "generic".to_string(),
 }
 }

 /// Round 144 — ADR-style preset (anchor = `ADR-NNNN`, entries = `ADR-`).
 /// Useful as a sample for external users authoring an `mnemosyne.toml`
 /// against an Architectural Decision Records project.
 pub fn adr_preset() -> Self {
 Self {
 changelog_titles: vec!["Decisions".to_string()],
 entry_id_prefix: "ADR-".to_string(),
 anchor_convention: "adr_id".to_string(),
 medium_name: "adr".to_string(),
 }
 }

 /// Case-sensitive title match against the configured changelog title
 /// set. Matches the parser's existing `is_changelog_title` semantics
 /// for the Mnemosyne preset.
 pub fn is_changelog_title(&self, title: &str) -> bool {
 self.changelog_titles.iter().any(|c| c == title)
 || title.eq_ignore_ascii_case("changelog")
 }
}

impl Default for SchemaSection {
 fn default() -> Self {
 Self::mnemosyne_preset()
 }
}

/// `[workspace]` table — doc paths + default cross-doc target + optional
/// root override (relative paths resolve against the config file's dir
/// unless `root` is set).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceSection {
 /// Ordered list of doc paths (relative to workspace root). Set must be
 /// non-empty — empty list rejected at load time.
 pub docs: Vec<String>,

 /// Optional default cross-doc target — when a §N reference fails the
 /// intra-doc lookup and the target is registered here, the parser
 /// reclassifies as `cross_doc` (DESIGN §61 mapping table row 12 step 2).
 /// Must be a member of `docs` if set.
 #[serde(default)]
 pub default_doc: Option<String>,

 /// Workspace root override — relative paths resolve against this when
 /// set, otherwise against the config file's parent dir.
 #[serde(default)]
 pub root: Option<String>,
}

/// Config discovery + load result.
#[derive(Debug, Clone)]
pub struct LoadedConfig {
 pub config: WorkspaceConfig,
 /// Absolute path to the directory all `docs[].path` resolve against.
 pub workspace_root: PathBuf,
 /// Absolute path to the config file itself (for diagnostics).
 pub config_path: PathBuf,
}

impl LoadedConfig {
 /// Resolve a doc path entry to an absolute path under `workspace_root`.
 pub fn doc_abs_path(&self, rel: &str) -> PathBuf {
 self.workspace_root.join(rel)
 }

 /// Iterate doc paths (each as `&str`).
 pub fn doc_paths(&self) -> impl Iterator<Item = &str> {
 self.config.workspace.docs.iter().map(String::as_str)
 }
}

/// Parse a TOML byte slice into a config struct + validate.
pub fn parse_config(content: &str) -> Result<WorkspaceConfig> {
 let cfg: WorkspaceConfig = toml::from_str(content).context("mnemosyne.toml parse failed")?;
 validate(&cfg)?;
 Ok(cfg)
}

fn validate(cfg: &WorkspaceConfig) -> Result<()> {
 if cfg.workspace.docs.is_empty() {
 bail!("mnemosyne.toml: `workspace.docs` must contain at least one path");
 }
 if let Some(default) = &cfg.workspace.default_doc {
 if !cfg.workspace.docs.iter().any(|d| d == default) {
 bail!(
  "mnemosyne.toml: `workspace.default_doc = {:?}` is not a member of `workspace.docs`",
  default
 );
 }
 }
 Ok(())
}

/// Load a config from a known TOML file path. Resolves `workspace_root` from
/// the explicit `[workspace] root` field if set, else from the config file's
/// parent dir.
pub fn load_config(config_path: &Path) -> Result<LoadedConfig> {
 let content = std::fs::read_to_string(config_path)
 .with_context(|| format!("read {}", config_path.display()))?;
 let config = parse_config(&content)?;

 let config_dir = config_path
 .parent()
 .ok_or_else(|| anyhow!("config path {} has no parent", config_path.display()))?
 .to_path_buf();

 let workspace_root = match &config.workspace.root {
 Some(r) => {
 let candidate = config_dir.join(r);
 candidate
  .canonicalize()
  .unwrap_or_else(|_| candidate.clone())
 }
 None => config_dir,
 };

 Ok(LoadedConfig {
 config,
 workspace_root,
 config_path: config_path.to_path_buf(),
 })
}

const PRIMARY_FILENAME: &str = "mnemosyne.toml";
const FALLBACK_FILENAME: &str = ".mnemosyne/config.toml";

/// Walk upward from `start` looking for `mnemosyne.toml` then
/// `.mnemosyne/config.toml`. Returns the first match (load + validate) or
/// `None` if the entire ancestor chain has no config file.
pub fn discover_config(start: &Path) -> Result<Option<LoadedConfig>> {
 let mut cursor = if start.is_absolute() {
 start.to_path_buf()
 } else {
 std::env::current_dir()
 .context("CWD lookup")?
 .join(start)
 };

 loop {
 for candidate_name in [PRIMARY_FILENAME, FALLBACK_FILENAME] {
 let candidate = cursor.join(candidate_name);
 if candidate.is_file() {
  return Ok(Some(load_config(&candidate)?));
 }
 }
 match cursor.parent() {
 Some(parent) => cursor = parent.to_path_buf(),
 None => return Ok(None),
 }
 }
}

#[cfg(test)]
mod tests {
 use super::*;
 use std::fs;
 use tempfile::TempDir;

 #[test]
 fn parse_minimal_config() {
 let content = r#"
[workspace]
docs = ["a.md", "b.md"]
"#;
 let cfg = parse_config(content).unwrap();
 assert_eq!(cfg.workspace.docs, vec!["a.md", "b.md"]);
 assert!(cfg.workspace.default_doc.is_none());
 assert!(cfg.workspace.root.is_none());
 }

 #[test]
 fn parse_full_config() {
 let content = r#"
[workspace]
docs = ["docs/DESIGN.md", "README.md"]
default_doc = "docs/DESIGN.md"
root = "."
"#;
 let cfg = parse_config(content).unwrap();
 assert_eq!(cfg.workspace.docs.len(), 2);
 assert_eq!(cfg.workspace.default_doc.as_deref(), Some("docs/DESIGN.md"));
 assert_eq!(cfg.workspace.root.as_deref(), Some("."));
 }

 #[test]
 fn empty_docs_rejected() {
 let content = "[workspace]\ndocs = []\n";
 let err = parse_config(content).unwrap_err();
 assert!(err.to_string().contains("workspace.docs"));
 }

 #[test]
 fn default_doc_must_be_in_docs() {
 let content = r#"
[workspace]
docs = ["a.md", "b.md"]
default_doc = "missing.md"
"#;
 let err = parse_config(content).unwrap_err();
 assert!(err.to_string().contains("default_doc"));
 }

 #[test]
 fn discover_walks_upward() {
 let tmp = TempDir::new().unwrap();
 let root = tmp.path();
 let nested = root.join("a/b/c");
 fs::create_dir_all(&nested).unwrap();
 fs::write(
 root.join("mnemosyne.toml"),
 "[workspace]\ndocs = [\"x.md\"]\n",
 )
 .unwrap();

 let loaded = discover_config(&nested).unwrap().expect("config found");
 assert_eq!(loaded.config.workspace.docs, vec!["x.md"]);
 // Workspace root resolves to the config file's dir.
 assert_eq!(
 loaded.workspace_root.canonicalize().unwrap(),
 root.canonicalize().unwrap()
 );
 }

 #[test]
 fn discover_missing_returns_none() {
 let tmp = TempDir::new().unwrap();
 let result = discover_config(tmp.path()).unwrap();
 assert!(result.is_none());
 }

 #[test]
 fn discover_prefers_primary_over_fallback() {
 let tmp = TempDir::new().unwrap();
 fs::create_dir_all(tmp.path().join(".mnemosyne")).unwrap();
 fs::write(
 tmp.path().join(".mnemosyne/config.toml"),
 "[workspace]\ndocs = [\"fallback.md\"]\n",
 )
 .unwrap();
 fs::write(
 tmp.path().join("mnemosyne.toml"),
 "[workspace]\ndocs = [\"primary.md\"]\n",
 )
 .unwrap();

 let loaded = discover_config(tmp.path()).unwrap().unwrap();
 assert_eq!(loaded.config.workspace.docs, vec!["primary.md"]);
 }

 #[test]
 fn schema_section_parses_when_present() {
 let content = r#"
[workspace]
docs = ["a.md"]

[schema]
changelog_titles = ["Changelog", "Changelog"]
medium_name = "design_doc"
"#;
 let cfg = parse_config(content).unwrap();
 let schema = cfg.schema.expect("schema present");
 assert_eq!(schema.changelog_titles, vec!["Changelog", "Changelog"]);
 assert_eq!(schema.medium_name, "design_doc");
 }

 #[test]
 fn schema_section_omitted_yields_none() {
 let content = "[workspace]\ndocs = [\"a.md\"]\n";
 let cfg = parse_config(content).unwrap();
 assert!(cfg.schema.is_none(), "schema must default to None");
 }

 #[test]
 fn schema_presets_carry_expected_titles() {
 let mnemo = SchemaSection::mnemosyne_preset();
 assert!(mnemo.is_changelog_title("Changelog"));
 assert!(mnemo.is_changelog_title("changelog"));

 let generic = SchemaSection::generic_default();
 assert!(generic.is_changelog_title("Changelog"));
 assert!(generic.is_changelog_title("CHANGELOG"));
 }

 #[test]
 fn root_override_resolves_relative() {
 let tmp = TempDir::new().unwrap();
 let nested = tmp.path().join("subdir");
 fs::create_dir_all(&nested).unwrap();
 fs::write(
 nested.join("mnemosyne.toml"),
 "[workspace]\ndocs = [\"a.md\"]\nroot = \"..\"\n",
 )
 .unwrap();

 let loaded = load_config(&nested.join("mnemosyne.toml")).unwrap();
 assert_eq!(
 loaded.workspace_root.canonicalize().unwrap(),
 tmp.path().canonicalize().unwrap()
 );
 }
}
