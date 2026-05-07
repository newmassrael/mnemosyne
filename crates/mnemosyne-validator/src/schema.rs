//! design_doc schema — DESIGN §39 *Phase 0 design_doc schema closed-form registered* full shape.
//!
//! Full 4 entity/relation shape (§39 closed-form registered — Round 60 ratify carry):
//! - **Section** (entity, 5 field): `section_id` canonical / `parent_doc` /
//! `parent_section` nullable ref / `title` / `decision_status` enum
//! - **ChangelogEntry** (entity, append-only, 4 field): `entry_id` canonical /
//! `parent_changelog_entry` nullable ref / `sub_bullets` ordered list /
//! `frozen_at_transaction_time` i64
//! - **FrozenList** (entity, version-locked, 4 field): `list_id` canonical /
//! `created_at_changelog_entry` ref. / `members` set:entity_ref / `lock_kind` enum
//! - **CrossRef** (relation, 2 fields + ref_kind enum + created_at): `from_section` /
//! `to_target` / `ref_kind` enum (decision / impl / cross_doc) /
//! `created_at_changelog_entry` ref.

use std::collections::BTreeMap;

/// Section entity — DESIGN §39 closed-form 5 field full shape.
/// `parent_section` nullable ref (file doc-root = None).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
 /// Canonical key — §N format ("39", "61.1") or unnumbered slug ("changelog").
 pub section_id: String,
 /// Owning doc identifier (relative path or doc-id).
 pub parent_doc: String,
 /// nullable ref — the file's first h1 = None; subsequent headings carry parent section_id.
 pub parent_section: Option<String>,
 pub title: String,
 pub decision_status: DecisionStatus,
}

/// Section.decision_status enum — DESIGN §39 closed-form registered carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DecisionStatus {
 Active,
 Superseded,
 Removed,
}

/// ChangelogEntry — DESIGN §39 closed-form 4 field, append-only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangelogEntry {
 /// Canonical key — "Round N" format.
 pub entry_id: String,
 /// nullable ref — chained sub-entry parent (Round 33 → 33.5 same chain).
 pub parent_changelog_entry: Option<String>,
 /// Ordered nested bullet content (raw markdown text preserved).
 ///
 /// Round 244 — *legacy field, carry stable* (sub_bullets cascade A scope
 /// decision). Round 19 frozen ledger consistency — this field Round 1-162 legacy
 /// Markdown entry's prose body — carried until the atomic store is registered as the source.
 /// New entry (Round 163+) — empty. Atomic dimension is the authoritative source.
 /// [`crate::atomic::AtomicChangelogEntry`] (`changes_bullets` /
 /// `verification_bullets` / `impact_refs` / `carry_forward_bullets`).
 /// MD-DELETION-RATIFY (Round 248) carry on parser extraction skip path
 /// entry — for those, this field defaults to empty.
 pub sub_bullets: Vec<String>,
 /// frozen_at_transaction_time -- in the Phase 0 prototype this captures register-order
 /// i64 monotonic stamp. Full Phase 0 implementation scope.
 /// Transaction-time service source.
 pub frozen_at_transaction_time: i64,
}

/// FrozenList — DESIGN §39 closed-form 4 field, version-locked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrozenList {
 pub list_id: String,
 /// ChangelogEntry.entry_id ref — this list 's frozen anchor.
 pub created_at_changelog_entry: String,
 /// Set semantics — Vec<String> in deterministic order (preserves insertion order).
 pub members: Vec<String>,
 pub lock_kind: LockKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LockKind {
 ReleaseLock,
 DecisionFreeze,
}

/// CrossRef relation — Section→Section|Entity, 2 field + ref_kind + created_at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossRef {
 pub from_section: String,
 pub to_target: String,
 pub ref_kind: RefKind,
 pub created_at_changelog_entry: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RefKind {
 Decision,
 Impl,
 CrossDoc,
}

/// Parser/emitter typed-facts container — in-memory state for the 4 entity/relation kinds.
///
/// Round 118 carry — `bodies` + `line_anchors` in §15 *Spec query API surface*
/// SectionView carry source (Round 117 §15 body-registered carry). The bench prototype
/// (Round 116) production lift — `parsed_doc_canonical` is out of scope (carry)
/// (round-trip diff = ∅ validation cardinality identical maintain, derived dimension).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedDoc {
 pub sections: Vec<Section>,
 pub changelog_entries: Vec<ChangelogEntry>,
 pub frozen_lists: Vec<FrozenList>,
 pub cross_refs: Vec<CrossRef>,
 /// Parser warnings — line ref legacy / unmapped construct / FrozenList
 /// lock_kind such as absence (DESIGN §61 *parser warn exposed* carry).
 pub warnings: Vec<String>,
 /// Round 118 — section_id → raw body lines (everything from after the heading up to before the next heading,
 /// code-fence interiors preserved verbatim. Source for §15 spec query API's SectionView.body
 /// source — derived dimension (excluded from round-trip diff comparisons).
 ///
 /// Round 247 — *legacy field, atomic-first fallback only*. atomic store
 /// for the section, this returns [`crate::atomic::synthesize_section_body`]'s result.
 /// SectionView.body authoritative source, this field legacy markdown
 /// (Round 1-162 prose body) carry stable scope. MD-DELETION-RATIFY (extend
 /// 248) carry on parser extraction-skip + this field defaults to empty only.
 pub bodies: BTreeMap<String, String>,
 /// Round 118 — section_id → heading line number (1-indexed). §15 spec query
 /// Source for the API's SectionView.line_anchor — derived dimension.
 pub line_anchors: BTreeMap<String, usize>,
}

/// Render ParsedDoc as canonical text (deterministic ordering for hash check).
/// Round 52 sha256 canon pattern equivalent.
pub fn parsed_doc_canonical(doc: &ParsedDoc) -> String {
 let mut out = String::new();
 out.push_str("=== sections ===\n");
 for s in &doc.sections {
 out.push_str(&format!(
 "section_id={} parent_doc={} parent_section={:?} title={} status={:?}\n",
 s.section_id, s.parent_doc, s.parent_section, s.title, s.decision_status
 ));
 }
 out.push_str("=== changelog_entries ===\n");
 for e in &doc.changelog_entries {
 out.push_str(&format!(
 "entry_id={} parent={:?} txn_time={} sub_bullets[{}]\n",
 e.entry_id,
 e.parent_changelog_entry,
 e.frozen_at_transaction_time,
 e.sub_bullets.len()
 ));
 for b in &e.sub_bullets {
 out.push_str(&format!(" | {}\n", b));
 }
 }
 out.push_str("=== frozen_lists ===\n");
 for f in &doc.frozen_lists {
 out.push_str(&format!(
 "list_id={} created_at={} lock_kind={:?} members={:?}\n",
 f.list_id, f.created_at_changelog_entry, f.lock_kind, f.members
 ));
 }
 out.push_str("=== cross_refs ===\n");
 for c in &doc.cross_refs {
 out.push_str(&format!(
 "from={} to={} kind={:?} created_at={:?}\n",
 c.from_section, c.to_target, c.ref_kind, c.created_at_changelog_entry
 ));
 }
 out.push_str("=== warnings ===\n");
 for w in &doc.warnings {
 out.push_str(&format!("WARN: {}\n", w));
 }
 out
}

/// Sha256 hex digest of arbitrary string (deterministic content hash).
pub fn sha256_hex(s: &str) -> String {
 use sha2::Digest;
 let mut hasher = sha2::Sha256::new();
 hasher.update(s.as_bytes());
 let digest = hasher.finalize();
 let mut out = String::with_capacity(64);
 for b in digest.iter() {
 out.push_str(&format!("{:02x}", b));
 }
 out
}

/// Section by id helper — used by depth map + reference resolution.
pub(crate) fn section_by_id(doc: &ParsedDoc) -> BTreeMap<&str, &Section> {
 doc.sections
 .iter()
 .map(|s| (s.section_id.as_str(), s))
 .collect()
}

#[cfg(test)]
mod tests {
 use super::*;

 #[test]
 fn parsed_doc_canonical_deterministic() {
 let doc = ParsedDoc {
 sections: vec![Section {
  section_id: "39".to_string(),
  parent_doc: "DESIGN.md".to_string(),
  parent_section: None,
  title: "Graph schema codegen".to_string(),
  decision_status: DecisionStatus::Active,
 }],
 ..Default::default()
 };
 let a = parsed_doc_canonical(&doc);
 let b = parsed_doc_canonical(&doc);
 assert_eq!(a, b);
 assert!(a.contains("section_id=39"));
 }

 #[test]
 fn sha256_hex_stable_64_chars() {
 let h = sha256_hex("test");
 assert_eq!(h.len(), 64);
 assert_eq!(h, sha256_hex("test"));
 assert_ne!(h, sha256_hex("other"));
 }
}
