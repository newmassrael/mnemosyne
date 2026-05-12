//! typed facts → markdown emitter — *Markdown variant spec* source binding.
//!
//! OPTION H-2 adoption carry — emit table row 7/8/9 branch logic:
//! - **row 7**: CrossRef (ref_kind ∈ {decision, impl}, intra-doc) → `§{N}` inline literal
//! - **row 8**: CrossRef (ref_kind = cross_doc, to_target = workspace default cross-doc
//! target = DESIGN.md) → `§{N}` inline literal (default-doc target source-markdown
//! notation preserved; the parser reclassifies, then emits §N inline verbatim — round-trip
//! diff = ∅ guaranteed; symmetric to mapping table row 12 lookup priority (2)'s emit side.
//! - **row 9**: CrossRef (ref_kind = cross_doc, to_target ≠ workspace default cross-doc
//! target) → `[{text}]({other.md}#{anchor})` markdown link

use crate::schema::{section_by_id, FrozenList, LockKind, ParsedDoc, RefKind};

/// Convert heading text → GitHub-flavored markdown anchor.
///
/// spec 3 step:
/// 1. lowercase
/// 2. Strip every character that is not alphanumeric / hyphen / underscore / CJK.
/// 3. Substitute remaining whitespace with `-` (adjacent dashes from a double space → `--` preserved).
///
/// e.g. `## 60. Core / client boundary` → `60--core---client-boundary`
pub fn to_github_anchor(heading: &str) -> String {
 let mut chars: Vec<char> = Vec::with_capacity(heading.chars().count());
 for ch in heading.chars() {
 let lc = ch.to_ascii_lowercase();
 if lc.is_ascii_alphanumeric() || lc == '-' || lc == '_' || is_cjk_char(lc) {
 chars.push(lc);
 } else if lc == ' ' || lc == '\t' {
 chars.push(' ');
 } else {
 chars.push(' ');
 }
 }
 let mut out = String::with_capacity(chars.len());
 for ch in &chars {
 if *ch == ' ' {
 out.push('-');
 } else {
 out.push(*ch);
 }
 }
 out.trim_matches('-').to_string()
}

fn is_cjk_char(ch: char) -> bool {
 matches!(ch as u32,
 0x3040..=0x30FF
 | 0x3400..=0x4DBF
 | 0x4E00..=0x9FFF
 | 0xAC00..=0xD7AF
 )
}

/// ParsedDoc → markdown bytes. Deterministic — same ParsedDoc → same markdown bytes.
///
/// Default workspace default-doc binding = `Workspace::MNEMOSYNE_DEFAULT_DOC`
/// (`docs/DESIGN.md`) — emit on `to_target` prefix match in row 8 vs row 9 branch.
/// To emit using a different workspace's default-doc, use [`emit_markdown_with_default`].
pub fn emit_markdown(doc: &ParsedDoc) -> String {
 emit_markdown_with_default(doc, Some(crate::workspace::Workspace::MNEMOSYNE_DEFAULT_DOC))
}

/// `default_doc` = workspace's default cross-doc target prefix. None = row 8 unused.
pub fn emit_markdown_with_default(doc: &ParsedDoc, default_doc: Option<&str>) -> String {
 let mut out = String::new();
 let depth_map = build_section_depth_map(doc);

 for section in &doc.sections {
 let depth = depth_map
 .get(section.section_id.as_str())
 .copied()
 .unwrap_or(1);
 let heading_prefix = "#".repeat(depth);
 let heading_line = if section.parent_section.is_none() {
 format!("{} {}", heading_prefix, section.title)
 } else if let Some(num) = numbered_last_segment(&section.section_id) {
 format!("{} {}. {}", heading_prefix, num, section.title)
 } else {
 format!("{} {}", heading_prefix, section.title)
 };
 out.push_str(&heading_line);
 out.push_str("\n\n");

 // CrossRef inline emit — row 7/8/9 branch logic.
 let mut cross_ref_line = String::new();
 let mut first = true;
 for cr in doc.cross_refs.iter().filter(|c| c.from_section == section.section_id) {
 if !first {
  cross_ref_line.push_str(", ");
 }
 first = false;
 cross_ref_line.push_str(&emit_cross_ref(cr, default_doc));
 }
 if !cross_ref_line.is_empty() {
 out.push_str(&cross_ref_line);
 out.push_str("\n\n");
 }
 }

 for fl in &doc.frozen_lists {
 out.push_str(&emit_frozen_list_table(fl));
 out.push_str("\n");
 }

 if !doc.changelog_entries.is_empty() {
 for entry in &doc.changelog_entries {
 out.push_str(&format!("- {}:\n", entry.entry_id));
 for sub in &entry.sub_bullets {
  out.push_str(&format!("  - {}\n", sub));
 }
 }
 out.push_str("\n");
 }

 out
}

/// Single CrossRef emit — row 7/8/9 branch logic.
fn emit_cross_ref(cr: &crate::schema::CrossRef, default_doc: Option<&str>) -> String {
 match cr.ref_kind {
 // Row 7: intra-doc decision/impl → `§{N}` (or impl link).
 RefKind::Decision => format!("§{}", cr.to_target),
 RefKind::Impl => format!("[link]({})", cr.to_target),
 // Row 8 vs Row 9 branch: cross_doc to_target prefix match.
 RefKind::CrossDoc => emit_cross_doc(cr, default_doc),
 }
}

/// Cross-doc emit — row 8 (default-doc) vs row 9 (non-default).
fn emit_cross_doc(cr: &crate::schema::CrossRef, default_doc: Option<&str>) -> String {
 if let Some(default) = default_doc {
 // Row 8: to_target = `{default_doc}#§{N}` canonical form
 // → `§{N}` inline literal (source markdown notation preserved).
 let canonical_prefix = format!("{}#§", default);
 if let Some(rest) = cr.to_target.strip_prefix(&canonical_prefix) {
 return format!("§{}", rest);
 }
 }
 // Row 9: cross-doc to non-default → markdown link form preserved.
 format!("[link]({})", cr.to_target)
}

fn build_section_depth_map<'a>(
 doc: &'a ParsedDoc,
) -> std::collections::BTreeMap<&'a str, usize> {
 use std::collections::BTreeSet;
 let mut depth: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
 let by_id = section_by_id(doc);
 for section in &doc.sections {
 let mut d = 1usize;
 let mut cur = section;
 let mut visited: BTreeSet<&str> = BTreeSet::new();
 visited.insert(cur.section_id.as_str());
 while let Some(parent_id) = &cur.parent_section {
 d += 1;
 cur = match by_id.get(parent_id.as_str()) {
  Some(s) => s,
  None => break,
 };
 if !visited.insert(cur.section_id.as_str()) {
  break;
 }
 if d > 16 {
  break;
 }
 }
 depth.insert(section.section_id.as_str(), d);
 }
 depth
}

/// If the last segment is numeric (`N` / `N.M`), return that segment.
/// fix carry — emit on `60/1`, `roadmap/.../5` and other prefixed numbered nested forms.
/// nested form also emits the last segment's number prefix (re-parse recognizes the number +
/// parent prefix reapplied).
fn numbered_last_segment(id: &str) -> Option<&str> {
 let last = id.rsplit('/').next()?;
 if !last.is_empty() && is_numbered_section_id(last) {
 Some(last)
 } else {
 None
 }
}

fn is_numbered_section_id(id: &str) -> bool {
 let mut saw_digit = false;
 for ch in id.chars() {
 if ch.is_ascii_digit() {
 saw_digit = true;
 } else if ch == '.' {
 // ok between digits
 } else {
 return false;
 }
 }
 saw_digit
}

fn emit_frozen_list_table(fl: &FrozenList) -> String {
 let mut out = String::new();
 let lock_label = match fl.lock_kind {
 LockKind::ReleaseLock => "release_lock",
 LockKind::DecisionFreeze => "decision_freeze",
 };
 out.push_str(&format!(
 "**FrozenList `{}` (created at {}, lock_kind: {})**\n\n",
 fl.list_id, fl.created_at_changelog_entry, lock_label
 ));
 out.push_str("| member |\n|---|\n");
 for m in &fl.members {
 out.push_str(&format!("| {} |\n", m));
 }
 out
}

// ============================================================================
// Round-trip integrity validation helper.
// ============================================================================

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RoundTripDiff {
 pub section_identity_match: bool,
 pub section_count_a: usize,
 pub section_count_b: usize,
 pub changelog_sequence_match: bool,
 pub changelog_entry_count_a: usize,
 pub changelog_entry_count_b: usize,
 pub cross_ref_set_match: bool,
 pub cross_ref_count_a: usize,
 pub cross_ref_count_b: usize,
 pub mandatory_preserved: bool,
}

/// Compare two ParsedDoc on preserved mandatory dimension only.
///
/// `sub_bullets` compare *legacy carry stable* (-162 entry
/// prose-body round-trip validation source). atomic-store entries:
/// An empty markdown sub_bullets list makes the comparison vacuously equal. The atomic-first
/// citation surface [`crate::query::changelog_entries_for_section`] scope,
/// this round-trip diff and distinct dimension (cascade B consistency).
pub fn compare_typed_facts(a: &ParsedDoc, b: &ParsedDoc) -> RoundTripDiff {
 use std::collections::BTreeSet;

 let a_section_keys: BTreeSet<(String, Option<String>, String)> = a
 .sections
 .iter()
 .map(|s| (s.section_id.clone(), s.parent_section.clone(), s.title.clone()))
 .collect();
 let b_section_keys: BTreeSet<(String, Option<String>, String)> = b
 .sections
 .iter()
 .map(|s| (s.section_id.clone(), s.parent_section.clone(), s.title.clone()))
 .collect();
 let section_identity_match = a_section_keys == b_section_keys;

 let a_changelog_seq: Vec<(String, Vec<String>)> = a
 .changelog_entries
 .iter()
 .map(|e| (e.entry_id.clone(), e.sub_bullets.clone()))
 .collect();
 let b_changelog_seq: Vec<(String, Vec<String>)> = b
 .changelog_entries
 .iter()
 .map(|e| (e.entry_id.clone(), e.sub_bullets.clone()))
 .collect();
 let changelog_sequence_match = a_changelog_seq == b_changelog_seq;

 let a_cross: BTreeSet<(String, String, RefKind)> = a
 .cross_refs
 .iter()
 .map(|c| (c.from_section.clone(), c.to_target.clone(), c.ref_kind))
 .collect();
 let b_cross: BTreeSet<(String, String, RefKind)> = b
 .cross_refs
 .iter()
 .map(|c| (c.from_section.clone(), c.to_target.clone(), c.ref_kind))
 .collect();
 let cross_ref_set_match = a_cross == b_cross;

 let mandatory_preserved =
 section_identity_match && changelog_sequence_match && cross_ref_set_match;

 RoundTripDiff {
 section_identity_match,
 section_count_a: a.sections.len(),
 section_count_b: b.sections.len(),
 changelog_sequence_match,
 changelog_entry_count_a: a.changelog_entries.len(),
 changelog_entry_count_b: b.changelog_entries.len(),
 cross_ref_set_match,
 cross_ref_count_a: a.cross_refs.len(),
 cross_ref_count_b: b.cross_refs.len(),
 mandatory_preserved,
 }
}

#[cfg(test)]
mod tests {
 use super::*;
 use crate::parser::{design_doc_small_fixture, parse_markdown};
 use crate::schema::{sha256_hex, CrossRef};

 #[test]
 fn anchor_simple_lowercase() {
 assert_eq!(to_github_anchor("Phase 0"), "phase-0");
 }

 #[test]
 fn anchor_strips_punctuation() {
 assert_eq!(
 to_github_anchor("60. Core / client boundary"),
 "60--core---client-boundary"
 );
 }

 #[test]
 fn anchor_lowercases_and_hyphens_spaces() {
 let s = to_github_anchor("Changelog History");
 assert_eq!(s, "changelog-history");
 }

 #[test]
 fn anchor_underscore_hyphen_preserved() {
 assert_eq!(to_github_anchor("foo_bar-baz"), "foo_bar-baz");
 }

 #[test]
 fn anchor_trims_outer_hyphens() {
 assert_eq!(to_github_anchor(" hello "), "hello");
 assert_eq!(to_github_anchor("...title..."), "title");
 }

 #[test]
 fn emit_h1_doc_root() {
 let parsed = parse_markdown(design_doc_small_fixture(), "DESIGN.md");
 let md = emit_markdown(&parsed);
 assert!(md.contains("# Mnemosyne Design Decisions"));
 }

 #[test]
 fn emit_numbered_top_level_section() {
 let parsed = parse_markdown(design_doc_small_fixture(), "DESIGN.md");
 let md = emit_markdown(&parsed);
 assert!(md.contains("## 39. Graph schema codegen"));
 assert!(md.contains("## 61. Import adapter framework"));
 }

 #[test]
 fn emit_unnumbered_section_changelog() {
 let parsed = parse_markdown(design_doc_small_fixture(), "DESIGN.md");
 let md = emit_markdown(&parsed);
 assert!(md.contains("## Changelog"));
 }

 #[test]
 fn emit_nested_heading_depth() {
 let parsed = parse_markdown(design_doc_small_fixture(), "DESIGN.md");
 let md = emit_markdown(&parsed);
 assert!(md.contains("### Phase 0 design_doc schema closed-form registered"));
 }

 #[test]
 fn emit_changelog_entries_with_sub_bullets() {
 let parsed = parse_markdown(design_doc_small_fixture(), "DESIGN.md");
 let md = emit_markdown(&parsed);
 assert!(md.contains("- Round 60:"));
 assert!(md.contains("- Round 61:"));
 assert!(md.contains(" - "));
 }

 #[test]
 fn emit_cross_refs_inline() {
 let parsed = parse_markdown(design_doc_small_fixture(), "DESIGN.md");
 let md = emit_markdown(&parsed);
 assert!(md.contains("§39"));
 assert!(md.contains("§41"));
 assert!(md.contains("§56"));
 }

 #[test]
 fn emit_determinism() {
 let parsed = parse_markdown(design_doc_small_fixture(), "DESIGN.md");
 let a = emit_markdown(&parsed);
 let b = emit_markdown(&parsed);
 assert_eq!(a, b);
 }

 #[test]
 fn emit_canonical_render_sha256_stable() {
 let parsed = parse_markdown(design_doc_small_fixture(), "DESIGN.md");
 let md = emit_markdown(&parsed);
 let h1 = sha256_hex(&md);
 let parsed2 = parse_markdown(design_doc_small_fixture(), "DESIGN.md");
 let md2 = emit_markdown(&parsed2);
 let h2 = sha256_hex(&md2);
 assert_eq!(h1, h2);
 assert_eq!(h1.len(), 64);
 }

 #[test]
 fn round_trip_section_identity_preserved() {
 let parsed = parse_markdown(design_doc_small_fixture(), "DESIGN.md");
 let emitted = emit_markdown(&parsed);
 let reparsed = parse_markdown(&emitted, "DESIGN.md");
 let diff = compare_typed_facts(&parsed, &reparsed);
 assert!(
 diff.section_identity_match,
 "Section identity must round-trip ({} → {})",
 diff.section_count_a, diff.section_count_b
 );
 }

 #[test]
 fn round_trip_changelog_sequence_preserved() {
 let parsed = parse_markdown(design_doc_small_fixture(), "DESIGN.md");
 let emitted = emit_markdown(&parsed);
 let reparsed = parse_markdown(&emitted, "DESIGN.md");
 let diff = compare_typed_facts(&parsed, &reparsed);
 assert!(diff.changelog_sequence_match);
 }

 #[test]
 fn round_trip_cross_ref_set_preserved() {
 let parsed = parse_markdown(design_doc_small_fixture(), "DESIGN.md");
 let emitted = emit_markdown(&parsed);
 let reparsed = parse_markdown(&emitted, "DESIGN.md");
 let a_decision = parsed
 .cross_refs
 .iter()
 .filter(|c| c.ref_kind == RefKind::Decision)
 .count();
 let b_decision = reparsed
 .cross_refs
 .iter()
 .filter(|c| c.ref_kind == RefKind::Decision)
 .count();
 assert_eq!(a_decision, b_decision);
 }

 #[test]
 fn round_trip_typed_facts_overall() {
 let parsed = parse_markdown(design_doc_small_fixture(), "DESIGN.md");
 let emitted = emit_markdown(&parsed);
 let reparsed = parse_markdown(&emitted, "DESIGN.md");
 let diff = compare_typed_facts(&parsed, &reparsed);
 assert!(diff.mandatory_preserved);
 }

 #[test]
 fn frozen_list_table_emit() {
 let fl = FrozenList {
 list_id: "ten_cf_list".to_string(),
 created_at_changelog_entry: "Round 8".to_string(),
 members: vec!["entities".to_string(), "relations".to_string(), "audit".to_string()],
 lock_kind: LockKind::DecisionFreeze,
 };
 let table = emit_frozen_list_table(&fl);
 assert!(table.contains("ten_cf_list"));
 assert!(table.contains("decision_freeze"));
 assert!(table.contains("| entities |"));
 }

 // ── OPTION H-2 row 8/9 branch logic new test ────────────────────

 #[test]
 fn row_8_cross_doc_to_default_doc_emits_section_literal() {
 // Row 8: cross_doc to default-doc → `§{N}` inline literal.
 let cr = CrossRef {
 from_section: "61".to_string(),
 to_target: "docs/DESIGN.md#§39".to_string(),
 ref_kind: RefKind::CrossDoc,
 created_at_changelog_entry: None,
 };
 let s = emit_cross_ref(cr_ref(&cr), Some("docs/DESIGN.md"));
 assert_eq!(s, "§39");
 }

 #[test]
 fn row_9_cross_doc_to_non_default_emits_markdown_link() {
 // Row 9: cross_doc to non-default → markdown link form preserved.
 let cr = CrossRef {
 from_section: "61".to_string(),
 to_target: "docs/ARCHITECTURE.md#l1".to_string(),
 ref_kind: RefKind::CrossDoc,
 created_at_changelog_entry: None,
 };
 let s = emit_cross_ref(cr_ref(&cr), Some("docs/DESIGN.md"));
 assert_eq!(s, "[link](docs/ARCHITECTURE.md#l1)");
 }

 #[test]
 fn row_7_intra_doc_emits_section_literal() {
 let cr = CrossRef {
 from_section: "39".to_string(),
 to_target: "41".to_string(),
 ref_kind: RefKind::Decision,
 created_at_changelog_entry: None,
 };
 let s = emit_cross_ref(cr_ref(&cr), Some("docs/DESIGN.md"));
 assert_eq!(s, "§41");
 }

 #[test]
 fn row_8_round_trip_cross_doc_default_doc_preserved() {
 // carry — default-doc target source notation preserved round-trip equivalent.
 let original_md = "## 61. Test\n\nreference §39 (DESIGN.md cross-doc auto-reclassify then emit).\n";
 let parsed = parse_markdown(original_md, "docs/ARCHITECTURE.md");
 // workspace reclassify simulation: parser default = Decision, this test -
 // emitter side row 7/8/9 branch logic only validation — workspace test reclassify validation.
 let mut doc = parsed.clone();
 for cr in doc.cross_refs.iter_mut() {
 if cr.to_target == "39" {
  cr.ref_kind = RefKind::CrossDoc;
  cr.to_target = "docs/DESIGN.md#§39".to_string();
 }
 }
 let emitted = emit_markdown_with_default(&doc, Some("docs/DESIGN.md"));
 assert!(emitted.contains("§39"));
 assert!(!emitted.contains("[link](docs/DESIGN.md#§39)"));
 }

 fn cr_ref(c: &CrossRef) -> &CrossRef {
 c
 }
}
