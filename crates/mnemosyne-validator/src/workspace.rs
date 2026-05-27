//! Workspace-level config — mapping table row 12 lookup priority step (2)
//! source. OPTION H-2 adoption (workspace default cross-doc target binding,
//! (mnemosyne workspace = DESIGN.md) carry.
//!
//! ops-tuning param-spec / decision-surface separation pattern equivalent — for the workspace
//! the default cross-doc target itself is not part of the spec-decision surface (it's workspace-level
//! config); other design_doc workspaces are free to choose their own self-default — mnemosyne
//! Only this production crate binds the workspace default to DESIGN.md.

use crate::config::LoadedConfig;
use crate::schema::ParsedDoc;
use std::collections::{BTreeMap, BTreeSet};

/// Workspace state — source of truth for multi-doc lookup.
///
/// `default_doc` = workspace's default cross-doc target (mnemosyne workspace
/// (default = DESIGN.md). Parser lookup priority step (2) consults this default_doc
/// Performs the cross-doc auto-reclassify check using the section_id_set fallback.
///
/// `atomic_id_set` is the atomic-store-derived section_id set
/// populated by `cmd_validate_workspace`. When markdown re-parse cannot
/// resolve a `to_target` (workspace.docs=[GENERATED.md] mode or 7-md
/// deletion path), step (2.5) checks atomic store as the canonical source.
#[derive(Debug, Clone, Default)]
pub struct Workspace {
 /// Workspace's docs (path -> ParsedDoc) mapping.
 pub docs: BTreeMap<String, ParsedDoc>,
 /// Workspace default cross-doc target (e.g. "docs/GENERATED.md").
 /// None = default unspecified (lookup priority step (2) skip).
 pub default_doc: Option<String>,
 /// Atomic-store-derived section_id set — cross_ref atomic-first.
 /// Empty = atomic source unspecified (step (2.5) skip, existing markdown-only behavior).
 pub atomic_id_set: BTreeSet<String>,
}

impl Workspace {
 /// Mnemosyne workspace default — `docs/GENERATED.md` post 
 /// MD-DELETION-RATIFY (atomic store = sole source of truth, GENERATED.md
 /// = sole readable artifact). Pre-deletion this constant pointed to
 /// `docs/DESIGN.md`.
 ///
 /// framing-reset (Phase 0e generic library extraction): this
 /// constant is retained as a *fallback only* for callers that cannot
 /// load `mnemosyne.toml`. Production paths route through
 /// [`Workspace::from_config`] + [`crate::config::discover_config`]; this
 /// constant must not appear in public-facing call sites of external
 /// users.
 pub const MNEMOSYNE_DEFAULT_DOC: &'static str = "docs/GENERATED.md";

 /// Empty workspace with default_doc unset.
 pub fn new() -> Self {
 Self::default()
 }

 /// Workspace with mnemosyne default (`docs/DESIGN.md`). Fallback factory
 /// retained for tests and callers without config access — production
 /// callers should prefer [`Workspace::from_config`].
 pub fn mnemosyne() -> Self {
 Self {
 docs: BTreeMap::new(),
 default_doc: Some(Self::MNEMOSYNE_DEFAULT_DOC.to_string()),
 atomic_id_set: BTreeSet::new(),
 }
 }

 /// build an empty workspace whose `default_doc` comes from a
 /// loaded config (replaces the hardcoded `MNEMOSYNE_DEFAULT_DOC`
 /// fallback). Docs are still inserted by the caller via [`Self::insert`]
 /// once parsed; this factory only sets the cross-doc reclassify target.
 pub fn from_config(loaded: &LoadedConfig) -> Self {
 Self {
 docs: BTreeMap::new(),
 default_doc: loaded.config.workspace.default_doc.clone(),
 atomic_id_set: BTreeSet::new(),
 }
 }

 /// Insert or replace a parsed doc.
 pub fn insert(&mut self, path: impl Into<String>, doc: ParsedDoc) {
 self.docs.insert(path.into(), doc);
 }

 /// Lookup whether default_doc has the given section_id.
 /// Returns `false` if default_doc is unset OR not loaded into workspace.
 ///
 /// Match precedence: full section_id, then trailing-segment alias for
 /// nested numbered sections (so `` resolves to `2/2.1`).
 pub fn default_doc_has_section(&self, section_id: &str) -> bool {
 let Some(default) = self.default_doc.as_ref() else {
 return false;
 };
 let Some(default_doc) = self.docs.get(default) else {
 return false;
 };
 default_doc.sections.iter().any(|s| {
 s.section_id == section_id
 || s.section_id
  .rsplit_once('/')
  .is_some_and(|(_, last)| last == section_id)
 })
 }

 /// atomic-store section_id existence check (cross_ref atomic-first).
 /// If `atomic_id_set` is empty, step (2.5) is skipped and returns `false`.
 pub fn atomic_has_section(&self, section_id: &str) -> bool {
 self.atomic_id_set.contains(section_id)
 }

 /// inject the atomic-store-derived section_id set (cmd_validate_workspace
 /// invokes `AtomicStore::atomic_section_id_set` for the source.
 pub fn set_atomic_id_set(&mut self, set: BTreeSet<String>) {
 self.atomic_id_set = set;
 }

 /// Reclassify CrossRef ref_kind via lookup priority 3 step (OPTION
 /// H-2 adoption carry — mapping table row 12 source binding).
 ///
 /// Step:
 /// (1) intra-doc — if §N exists in self doc's section_id_set, ref_kind = `decision`
 /// (no change; parser default carry).
 /// (2) self doc missing + workspace default_doc in §N exists → to_target =
 /// `{default_doc}#§N` canonical form, ref_kind = `cross_doc`
 /// (cross-doc auto-reclassify).
 /// (3) both all missing → orphan reject (validator rule 1, ref_kind change
 /// missing — this function only reclassifies; rejects fall under the validator scope).
 pub fn reclassify_cross_refs(&self, doc_path: &str) -> Option<ParsedDoc> {
 let mut doc = self.docs.get(doc_path)?.clone();
 let intra_set: std::collections::BTreeSet<String> = doc
 .sections
 .iter()
 .map(|s| s.section_id.clone())
 .collect();

 for cr in doc.cross_refs.iter_mut() {
 // Skip if already cross-doc explicitly emitted as link form.
 if cr.ref_kind == crate::schema::RefKind::CrossDoc {
  continue;
 }
 // Step (1): intra-doc — change missing.
 if intra_set.contains(&cr.to_target) {
  continue;
 }
 // Step (2): default_doc fallback — auto-reclassify.
 if self.default_doc_has_section(&cr.to_target) {
  let default = self.default_doc.as_ref().expect("checked above");
  cr.to_target = format!("{}#§{}", default, cr.to_target);
  cr.ref_kind = crate::schema::RefKind::CrossDoc;
  continue;
 }
 // Step (3): both all missing — this function change missing (validator
 // rule 1 subsequent reject).
 }
 Some(doc)
 }
}

#[cfg(test)]
mod tests {
 use super::*;
 use crate::schema::{DecisionStatus, RefKind, Section};

 fn make_section(id: &str, title: &str, parent_doc: &str) -> Section {
 Section {
 section_id: id.to_string(),
 parent_doc: parent_doc.to_string(),
 parent_section: None,
 title: title.to_string(),
 decision_status: DecisionStatus::Active,
 atomic_section_id: Some(id.to_string()),
 }
 }

 #[test]
 fn mnemosyne_default_doc_constant() {
 assert_eq!(Workspace::MNEMOSYNE_DEFAULT_DOC, "docs/GENERATED.md");
 let ws = Workspace::mnemosyne();
 assert_eq!(ws.default_doc.as_deref(), Some("docs/GENERATED.md"));
 }

 #[test]
 fn empty_workspace_lookup_returns_false() {
 let ws = Workspace::new();
 assert!(!ws.default_doc_has_section("39"));
 }

 #[test]
 fn default_doc_has_section_lookup() {
 let mut ws = Workspace::mnemosyne();
 let doc = ParsedDoc {
 sections: vec![
  make_section("39", "Graph schema codegen", "docs/GENERATED.md"),
  make_section("66", "Self-application", "docs/GENERATED.md"),
 ],
 ..Default::default()
 };
 ws.insert("docs/GENERATED.md", doc);
 assert!(ws.default_doc_has_section("39"));
 assert!(ws.default_doc_has_section("66"));
 assert!(!ws.default_doc_has_section("99"));
 }

 #[test]
 fn reclassify_step_2_promotes_to_cross_doc() {
 // other doc in citation — intra-doc missing, default-doc in exists → cross_doc reclassify.
 let mut ws = Workspace::mnemosyne();

 let design = ParsedDoc {
 sections: vec![make_section("39", "Graph schema", "docs/GENERATED.md")],
 ..Default::default()
 };
 ws.insert("docs/GENERATED.md", design);

 let other = ParsedDoc {
 sections: vec![make_section("l1", "Layer 1", "docs/OTHER.md")],
 cross_refs: vec![crate::schema::CrossRef {
  from_section: "l1".to_string(),
  to_target: "39".to_string(),
  ref_kind: RefKind::Decision,
  created_at_changelog_entry: None,
 }],
 ..Default::default()
 };
 ws.insert("docs/OTHER.md", other);

 let reclassified = ws.reclassify_cross_refs("docs/OTHER.md").unwrap();
 let cr = &reclassified.cross_refs[0];
 assert_eq!(cr.ref_kind, RefKind::CrossDoc);
 assert_eq!(cr.to_target, "docs/GENERATED.md#§39");
 }

 #[test]
 fn reclassify_step_1_intra_doc_unchanged() {
 // self doc in exists → step (1), change missing.
 let mut ws = Workspace::mnemosyne();
 let doc = ParsedDoc {
 sections: vec![
  make_section("39", "Graph schema", "docs/GENERATED.md"),
  make_section("66", "Self-application", "docs/GENERATED.md"),
 ],
 cross_refs: vec![crate::schema::CrossRef {
  from_section: "66".to_string(),
  to_target: "39".to_string(),
  ref_kind: RefKind::Decision,
  created_at_changelog_entry: None,
 }],
 ..Default::default()
 };
 ws.insert("docs/GENERATED.md", doc);

 let reclassified = ws.reclassify_cross_refs("docs/GENERATED.md").unwrap();
 let cr = &reclassified.cross_refs[0];
 assert_eq!(cr.ref_kind, RefKind::Decision);
 assert_eq!(cr.to_target, "39");
 }

 #[test]
 fn reclassify_step_3_orphan_unchanged() {
 // both all missing → this function change missing (validator rule 1 subsequent reject).
 let mut ws = Workspace::mnemosyne();
 let design = ParsedDoc {
 sections: vec![make_section("39", "Graph schema", "docs/GENERATED.md")],
 ..Default::default()
 };
 ws.insert("docs/GENERATED.md", design);

 let other = ParsedDoc {
 sections: vec![make_section("l1", "Layer 1", "docs/OTHER.md")],
 cross_refs: vec![crate::schema::CrossRef {
  from_section: "l1".to_string(),
  to_target: "999".to_string(),
  ref_kind: RefKind::Decision,
  created_at_changelog_entry: None,
 }],
 ..Default::default()
 };
 ws.insert("docs/OTHER.md", other);

 let reclassified = ws.reclassify_cross_refs("docs/OTHER.md").unwrap();
 let cr = &reclassified.cross_refs[0];
 assert_eq!(cr.ref_kind, RefKind::Decision);
 assert_eq!(cr.to_target, "999");
 }
}
