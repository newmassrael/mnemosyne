//! query-cli — bench-prototype demo binary for the spec query API (Round 116).
//!
//! `query-cli §43 [--include-related] [--include-changelog] [--json]` —
//! 4 query primitives across the 7-doc workspace (section_by_id / related_sections /
//! changelog_entries_for_section / workspace_section_id_set) demo path.
//!
//! Phase 0b entry #1 — production lift in Round 120 (mnemosyne-cli query
//! subcommand) thereafter. This binary is a prototype demo + Round 116 measurement
//! source-limited.
//!
//! Usage:
//! query-cli §43 --include-related --include-changelog --json
//! query-cli 39 --json
//! query-cli --list-sections
//! query-cli §66 --include-changelog
//!
//! Workspace root carry — `.git` + `docs/DESIGN.md` heuristic (production
//! `mnemosyne-cli` and equivalent, Round 120 production lift on shared).

use anyhow::{anyhow, bail, Context, Result};
use codegen_prototype::markdown_import::parse_markdown;
use codegen_prototype::query_api::{
 build_envelope, changelog_entries_for_section, related_sections, section_by_id,
 workspace_section_id_set, Workspace,
};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const WORKSPACE_DOCS: &[&str] = &[
 "docs/DESIGN.md",
 "docs/ARCHITECTURE.md",
 "docs/ROADMAP.md",
 "docs/VISION.md",
 "docs/CONCEPTS.md",
 "README.md",
 "docs/PRIOR_ART.md",
];

#[derive(Debug, Default)]
struct CliArgs {
 section_id: Option<String>,
 include_related: bool,
 include_changelog: bool,
 json: bool,
 list_sections: bool,
}

fn main() -> Result<()> {
 let args = parse_args(env::args().skip(1).collect())?;

 let root = locate_repo_root()
 .context("repo root recovery failed — .git + docs/DESIGN.md not found")?;
 let workspace = load_workspace(&root)?;

 if args.list_sections {
 let set = workspace_section_id_set(&workspace);
 for id in &set {
 println!("{}", id);
 }
 eprintln!("# total {} section(s)", set.len());
 return Ok(());
 }

 let section_id = args
 .section_id
 .ok_or_else(|| anyhow!("section_id arg required — e.g. query-cli §43"))?;

 if args.json && args.include_related && args.include_changelog {
 let envelope = build_envelope(&workspace, &section_id).ok_or_else(|| {
 anyhow!("section_id `{}` workspace in not found", section_id)
 })?;
 let json = serde_json::to_string_pretty(&envelope)?;
 println!("{}", json);
 return Ok(());
 }

 if args.json {
 let view = section_by_id(&workspace, &section_id).ok_or_else(|| {
 anyhow!("section_id `{}` workspace in not found", section_id)
 })?;
 let json = serde_json::to_string_pretty(&view)?;
 println!("{}", json);
 return Ok(());
 }

 // Plain text mode (default).
 let view = section_by_id(&workspace, &section_id).ok_or_else(|| {
 anyhow!("section_id `{}` workspace in not found", section_id)
 })?;
 println!(
 "§{} ({}#L{}) {}",
 view.section_id, view.parent_doc, view.line_anchor, view.title
 );
 println!("decision_status: {}", view.decision_status);
 if let Some(parent) = &view.parent_section {
 println!("parent_section: §{}", parent);
 }
 if !view.body.is_empty() {
 println!();
 println!("--- body ---");
 println!("{}", view.body);
 println!("--- end body ---");
 }

 if args.include_related {
 let related = related_sections(&workspace, &section_id);
 println!();
 println!("outbound_refs ({}):", related.outbound_refs.len());
 for r in &related.outbound_refs {
 println!(" {} → {} [{}]", r.from_section, r.to_target, r.ref_kind);
 }
 println!();
 println!("inbound_refs ({}):", related.inbound_refs.len());
 for r in &related.inbound_refs {
 println!(" {}#§{} → {} [{}]", r.from_doc, r.from_section, r.to_target, r.ref_kind);
 }
 }

 if args.include_changelog {
 let entries = changelog_entries_for_section(&workspace, &section_id);
 println!();
 println!("related_changelog_entries ({}):", entries.len());
 for e in &entries {
 println!(
  " [{}] {} (txn={}, citations={}, sub_bullets={})",
  e.parent_doc,
  e.entry_id,
  e.frozen_at_transaction_time,
  e.citation_count,
  e.sub_bullets.len()
 );
 }
 }

 Ok(())
}

fn parse_args(args: Vec<String>) -> Result<CliArgs> {
 let mut out = CliArgs::default();
 for arg in args {
 match arg.as_str() {
 "--include-related" => out.include_related = true,
 "--include-changelog" => out.include_changelog = true,
 "--json" => out.json = true,
 "--list-sections" => out.list_sections = true,
 "-h" | "--help" => {
  print_help();
  std::process::exit(0);
 }
 other if other.starts_with("--") => {
  bail!("unknown flag `{}`", other);
 }
 other => {
  if out.section_id.is_some() {
  bail!("section_id argument duplicate (already `{}`)", out.section_id.unwrap());
  }
  // Strip leading § if present (§43 ↔ 43 identical identifier..
  let stripped = other.strip_prefix('§').unwrap_or(other).to_string();
  out.section_id = Some(stripped);
 }
 }
 }
 Ok(out)
}

fn print_help() {
 println!("query-cli — bench spec query API demo (Round 116)");
 println!();
 println!("USAGE:");
 println!(" query-cli §<section_id> [--include-related] [--include-changelog] [--json]");
 println!(" query-cli --list-sections");
 println!();
 println!("FLAGS:");
 println!(" --include-related outbound + inbound 1-hop CrossRef traversal");
 println!(" --include-changelog §N citation ChangelogEntry carry");
 println!(" --json  JSON envelope output (Claude consumable)");
 println!(" --list-sections workspace full section_id set print");
 println!();
 println!("EXAMPLES:");
 println!(" query-cli §43 --include-related --include-changelog --json");
 println!(" query-cli 66 --include-changelog");
}

fn locate_repo_root() -> Result<PathBuf> {
 let mut current = env::current_dir()?;
 loop {
 if current.join(".git").exists() && current.join("docs/DESIGN.md").exists() {
 return Ok(current);
 }
 match current.parent() {
 Some(parent) => current = parent.to_path_buf(),
 None => bail!("repo root recovery failed — .git + docs/DESIGN.md not found"),
 }
 }
}

fn load_workspace(root: &Path) -> Result<Workspace> {
 let mut ws = Workspace::mnemosyne();
 for rel in WORKSPACE_DOCS {
 let path = root.join(rel);
 let content = fs::read_to_string(&path)
 .with_context(|| format!("doc filework read failure: {}", path.display()))?;
 let parsed = parse_markdown(&content, rel);
 ws.insert(*rel, parsed);
 }
 Ok(ws)
}
