//! Spec mutate API surface — *Spec mutate API surface* binding source
//!. was the first mutate primitive
//! production lift.
//!
//! *legacy v1 markdown-mutate path* (sub_bullets cascade C marker).
//! This module's [`append_changelog_entry`] = -162 legacy markdown
//! surgical insert path. paradigm shift carry: production primitive =
//! [`crate::atomic::append_changelog_entry_v2`] (atomic store standalone mutate,
//! production wire). v1 path's sub_bullets dependency carries stable
//! for frozen ledger compatibility — post MD-DELETION-RATIFY the v1
//! entry point is effectively unused.
//!
//! ## append_changelog_entry primitive
//!
//! Append a new ChangelogEntry as a direct child of the `## Changelog` heading
//! (entry_id monotonic enforced + frozen_at_transaction_time monotonic enforced
//! + T1 cross_ref orphan reject (new orphan blocked) + T2 frozen_ledger_jaccard
//! (existing entry sub_bullets jaccard ≥ 1.0) + atomic round-trip enforced +
//! rollback on failure).
//!
//! ### Atomic round-trip flow
//!
//! ```text
//! mutate primitive invoke
//! ↓
//! 1. read original disk content (byte snapshot)
//! ↓
//! 2. validate inputs (entry_id format / monotonic / frozen_at_transaction_time monotonic)
//! ↓
//! 3. Find the insert position (`## Changelog` direct child, last line + 1).
//! ↓
//! 4. Format the new entry (`- Round N (TITLE):\n - bullet 1\n - bullet 2\n...`).
//! ↓
//! 5. Surgical insert into the byte snapshot (preserves all bytes outside the affected region).
//! ↓
//! 6. Atomic write (temp file + rename).
//! ↓
//! 7. Re-parse from disk → typed-facts state.
//! ↓
//! 8. T2 frozen_ledger_jaccard (prev → reparsed) — existing entry mutation 0
//! ↓
//! 9. T1 cross_ref_orphan_reject (after workspace step (1)+(2) reclassify) — 0 new orphans
//! ↓
//! 10. Structural verification (reparsed has +1 changelog entry; entry_id + sub_bullets match).
//! ↓
//! 11. On failure: rollback (write back original bytes) + return MutateError.
//! ```
//!
//! ### Surgical insert vs full re-emit
//!
//! This first primitive adopts *surgical insert* (byte preservation
//! outside the affected region, 0 mutation elsewhere) — emit-based full-doc
//! recreation through the current parser/emitter pair drops ChangelogEntry
//! title-in-parens information (the parser captures only entry_id), so a
//! re-emit would be destructive (rich title-in-parens info on disk would be
//! lost). First round of Phase 0c = surgical carry + round-trip
//! diff = ∅ validation (typed-facts unit). Lossy full re-emit is deferred to
//! Phase 1A+ post-schema-extension carry.

use crate::emitter::{compare_typed_facts, emit_markdown_with_default};
use crate::parser::parse_markdown;
use crate::schema::ParsedDoc;
use crate::t2::frozen_ledger_jaccard;
use crate::validator::{cross_ref_orphan_reject_with_workspace, ValidationError};
use crate::workspace::Workspace;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Mutate operation receipt — *MutateReceipt envelope shape* source binding.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MutateReceipt {
 pub primitive: String,
 pub affected_docs: Vec<String>,
 pub affected_sections: Vec<String>,
 pub written_bytes_per_doc: BTreeMap<String, usize>,
 pub round_trip_diff_count: usize,
 pub validator_path_invocations: Vec<String>,
 pub applied_at_transaction_time: i64,
}

/// Mutate operation error — *MutateError* enum source binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutateError {
 pub primitive: String,
 pub kind: MutateErrorKind,
 pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MutateErrorKind {
 ValidatorReject,
 RoundTripDriftError,
 FrozenLedgerViolation,
 OrphanRejection,
 MonotonicViolation,
 AppendOnlyViolation,
 IoError,
 NotFound,
 StructuralVerificationFailed,
}

impl std::fmt::Display for MutateError {
 fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
 write!(
 f,
 "{} {:?}: {}",
 self.primitive, self.kind, self.detail
 )
 }
}

impl std::error::Error for MutateError {}

/// Append a new ChangelogEntry to `## Changelog` section of the target doc.
///
/// Atomic round-trip flow + rollback-on-failure (mutate API surface
/// binding source). The production lift uses surgical insert —
/// byte preservation outside the affected region, round-trip diff = ∅ enforced.
///
/// *legacy v1 path* (sub_bullets cascade C marker). Production
/// primitive = [`crate::atomic::append_changelog_entry_v2`]. This function's
/// `sub_bullets` dependency stays stable for existing markdown — post 
/// MD-DELETION-RATIFY this entry point is effectively unused; new entries should
/// use v2.
///
/// # Arguments
/// - `workspace`: typed facts workspace (the target doc must already be loaded).
/// - `doc_path`: workspace-relative path of the doc to mutate (e.g. `"docs/DESIGN.md"`).
/// - `entry_id`: new entry_id (e.g. `""`). Must equal the last entry's
/// next Round N (monotonic enforcement).
/// - `title`: optional title-in-parens (e.g. `"APPEND-CHANGELOG-ENTRY-MUTATE-PRIMITIVE"`).
/// `None` emits `- {entry_id}:` only.
/// - `sub_bullets`: ordered list of sub_bullet contents (each string is a
/// single-line bullet body).
/// - `frozen_at_transaction_time`: monotonic timestamp (must be strictly greater
/// than the previous entry).
/// - `docs_root`: physical disk root for the atomic write (parent of the
/// workspace doc_path — typically the repo root).
///
/// # Returns
/// `MutateReceipt` on success, `MutateError` on failure (rollback performed automatically).
pub fn append_changelog_entry(
 workspace: &Workspace,
 doc_path: &str,
 entry_id: &str,
 title: Option<&str>,
 sub_bullets: &[String],
 frozen_at_transaction_time: i64,
 docs_root: &Path,
) -> Result<MutateReceipt, MutateError> {
 const PRIM: &str = "append_changelog_entry";

 // ---- 1. Get prev typed facts from workspace ----
 let prev = workspace.docs.get(doc_path).ok_or_else(|| MutateError {
 primitive: PRIM.to_string(),
 kind: MutateErrorKind::NotFound,
 detail: format!("doc `{}` not loaded in workspace", doc_path),
 })?;

 // ---- 2. Validate inputs ----
 if !entry_id.starts_with("Round ") {
 return Err(MutateError {
 primitive: PRIM.to_string(),
 kind: MutateErrorKind::AppendOnlyViolation,
 detail: format!("entry_id must start with `Round ` -- got `{}`", entry_id),
 });
 }
 let entry_n = parse_entry_n(entry_id).ok_or_else(|| MutateError {
 primitive: PRIM.to_string(),
 kind: MutateErrorKind::AppendOnlyViolation,
 detail: format!("entry_id `{}` does not parse to numeric N", entry_id),
 })?;

 if let Some(last) = prev.changelog_entries.last() {
 let last_n = parse_entry_n(&last.entry_id).ok_or_else(|| MutateError {
 primitive: PRIM.to_string(),
 kind: MutateErrorKind::StructuralVerificationFailed,
 detail: format!(
  "prev last entry `{}` does not parse to numeric N",
  last.entry_id
 ),
 })?;
 if entry_n != last_n + 1 {
 return Err(MutateError {
  primitive: PRIM.to_string(),
  kind: MutateErrorKind::MonotonicViolation,
  detail: format!(
  "entry_id `{}` not monotonic -- last was `{}`, expected `Round {}`",
  entry_id,
  last.entry_id,
  last_n + 1
  ),
 });
 }
 if frozen_at_transaction_time <= last.frozen_at_transaction_time {
 return Err(MutateError {
  primitive: PRIM.to_string(),
  kind: MutateErrorKind::MonotonicViolation,
  detail: format!(
  "frozen_at_transaction_time {} not strictly monotonic — last was {}",
  frozen_at_transaction_time, last.frozen_at_transaction_time
  ),
 });
 }
 }

 if sub_bullets.is_empty() {
 return Err(MutateError {
 primitive: PRIM.to_string(),
 kind: MutateErrorKind::AppendOnlyViolation,
 detail: "sub_bullets must be non-empty — entry must have at least one bullet"
  .to_string(),
 });
 }

 // ---- 3. Read original disk content ----
 let abs_path = docs_root.join(doc_path);
 let original = fs::read_to_string(&abs_path).map_err(|e| MutateError {
 primitive: PRIM.to_string(),
 kind: MutateErrorKind::IoError,
 detail: format!("read {}: {}", abs_path.display(), e),
 })?;

 // ---- 4. Find insert position (after last existing changelog entry's content) ----
 let insert_pos = find_changelog_insert_position(&original).ok_or_else(|| MutateError {
 primitive: PRIM.to_string(),
 kind: MutateErrorKind::StructuralVerificationFailed,
 detail: format!(
 "doc `{}` has no `## Changelog` section — cannot append entry",
 doc_path
 ),
 })?;

 // ---- 5. Format new entry ----
 let new_entry_text = format_new_entry(entry_id, title, sub_bullets);

 // ---- 6. Surgical insert ----
 let mut new_content = String::with_capacity(original.len() + new_entry_text.len());
 new_content.push_str(&original[..insert_pos]);
 new_content.push_str(&new_entry_text);
 new_content.push_str(&original[insert_pos..]);

 // ---- 7. Atomic write ----
 atomic_write(&abs_path, &new_content).map_err(|e| MutateError {
 primitive: PRIM.to_string(),
 kind: MutateErrorKind::IoError,
 detail: format!("atomic write {}: {}", abs_path.display(), e),
 })?;

 // From here on, any failure must rollback (write back original).
 let validation_result = validate_after_write(
 workspace,
 doc_path,
 prev,
 entry_id,
 sub_bullets,
 &abs_path,
 );

 match validation_result {
 Ok(reparsed_summary) => Ok(MutateReceipt {
 primitive: PRIM.to_string(),
 affected_docs: vec![doc_path.to_string()],
 affected_sections: reparsed_summary.affected_sections,
 written_bytes_per_doc: {
  let mut m = BTreeMap::new();
  m.insert(doc_path.to_string(), new_content.len());
  m
 },
 round_trip_diff_count: 0,
 validator_path_invocations: vec![
  "t2::frozen_ledger_jaccard".to_string(),
  "t1::cross_ref_orphan_reject_with_workspace".to_string(),
  "structural_verification::changelog_entry_appended".to_string(),
 ],
 applied_at_transaction_time: frozen_at_transaction_time,
 }),
 Err(err) => {
 // Rollback — best-effort write back original content.
 let _ = atomic_write(&abs_path, &original);
 Err(err)
 }
 }
}

struct ReparsedSummary {
 affected_sections: Vec<String>,
}

fn validate_after_write(
 workspace: &Workspace,
 doc_path: &str,
 prev: &ParsedDoc,
 expected_entry_id: &str,
 expected_sub_bullets: &[String],
 abs_path: &Path,
) -> Result<ReparsedSummary, MutateError> {
 const PRIM: &str = "append_changelog_entry";

 // Re-read from disk + parse.
 let written = fs::read_to_string(abs_path).map_err(|e| MutateError {
 primitive: PRIM.to_string(),
 kind: MutateErrorKind::IoError,
 detail: format!("re-read {}: {}", abs_path.display(), e),
 })?;
 let reparsed = parse_markdown(&written, doc_path);

 // Structural verification: reparsed should have exactly one more changelog entry,
 // and that entry should match expected_entry_id + expected_sub_bullets.
 if reparsed.changelog_entries.len() != prev.changelog_entries.len() + 1 {
 return Err(MutateError {
 primitive: PRIM.to_string(),
 kind: MutateErrorKind::StructuralVerificationFailed,
 detail: format!(
  "reparsed changelog count = {} (expected {} = prev {} + 1)",
  reparsed.changelog_entries.len(),
  prev.changelog_entries.len() + 1,
  prev.changelog_entries.len()
 ),
 });
 }
 let last = reparsed
 .changelog_entries
 .last()
 .expect("len checked above");
 if last.entry_id != expected_entry_id {
 return Err(MutateError {
 primitive: PRIM.to_string(),
 kind: MutateErrorKind::StructuralVerificationFailed,
 detail: format!(
  "reparsed last entry_id = `{}` (expected `{}`)",
  last.entry_id, expected_entry_id
 ),
 });
 }
 if last.sub_bullets != expected_sub_bullets {
 return Err(MutateError {
 primitive: PRIM.to_string(),
 kind: MutateErrorKind::StructuralVerificationFailed,
 detail: format!(
  "reparsed sub_bullets ({} items) do not match expected ({} items)",
  last.sub_bullets.len(),
  expected_sub_bullets.len()
 ),
 });
 }

 // T2 frozen_ledger_jaccard: prev → reparsed must be append-only on existing entries.
 let t2_violations = frozen_ledger_jaccard(prev, &reparsed);
 if !t2_violations.is_empty() {
 return Err(MutateError {
 primitive: PRIM.to_string(),
 kind: MutateErrorKind::FrozenLedgerViolation,
 detail: format!(
  "T2 frozen_ledger_jaccard {} violation(s): {:?}",
  t2_violations.len(),
  t2_violations
 ),
 });
 }

 // T1 cross_ref_orphan_reject: no NEW orphans introduced by mutation.
 let mut new_ws = workspace.clone();
 new_ws.insert(doc_path.to_string(), reparsed.clone());

 let prev_orphan_keys: BTreeSet<(String, String)> =
 cross_ref_orphan_reject_with_workspace(prev, workspace)
 .into_iter()
 .filter_map(orphan_key)
 .collect();
 let new_orphan_keys: BTreeSet<(String, String)> =
 cross_ref_orphan_reject_with_workspace(&reparsed, &new_ws)
 .into_iter()
 .filter_map(orphan_key)
 .collect();
 let introduced: Vec<&(String, String)> =
 new_orphan_keys.difference(&prev_orphan_keys).collect();
 if !introduced.is_empty() {
 return Err(MutateError {
 primitive: PRIM.to_string(),
 kind: MutateErrorKind::OrphanRejection,
 detail: format!(
  "{} new orphan(s) introduced by mutation: {:?}",
  introduced.len(),
  introduced
  .iter()
  .map(|(f, t)| format!("§{} → §{}", f, t))
  .collect::<Vec<_>>()
 ),
 });
 }

 // Round-trip diff = ∅ — emit reparsed → re-parse → compare.
 let emitted = emit_markdown_with_default(&reparsed, new_ws.default_doc.as_deref());
 let reemit_parsed = parse_markdown(&emitted, doc_path);
 let diff = compare_typed_facts(&reparsed, &reemit_parsed);
 if !diff.mandatory_preserved {
 return Err(MutateError {
 primitive: PRIM.to_string(),
 kind: MutateErrorKind::RoundTripDriftError,
 detail: format!(
  "round-trip drift: section {}->{}, changelog {}->{}, cross_ref {}->{}",
  diff.section_count_a,
  diff.section_count_b,
  diff.changelog_entry_count_a,
  diff.changelog_entry_count_b,
  diff.cross_ref_count_a,
  diff.cross_ref_count_b
 ),
 });
 }

 Ok(ReparsedSummary {
 affected_sections: vec!["Changelog".to_string()],
 })
}

fn orphan_key(err: ValidationError) -> Option<(String, String)> {
 match err {
 ValidationError::OrphanCrossRef {
 from_section,
 to_target,
 ..
 } => Some((from_section, to_target)),
 _ => None,
 }
}

/// Parse `Round N` to numeric N (e.g. "" -> 124).
fn parse_entry_n(entry_id: &str) -> Option<u32> {
 let after = entry_id.strip_prefix("Round ")?;
 after.parse::<u32>().ok()
}

/// Find byte position to insert new changelog entry text.
///
/// Surgical insert position = end of the last direct child of `## Changelog`
/// (immediately after the last line's trailing newline, before the next
/// sibling section heading). If no subsequent sibling section exists, EOF.
///
/// Marker detection *line-start* enforced — `## Changelog` literal body
/// content can appear inside (e.g. a Round entry containing `... ` Changelog `...`); therefore
/// `## Changelog` only counts when at line start (file start or preceded by `\n`)
/// real heading as recognized.
fn find_changelog_insert_position(content: &str) -> Option<usize> {
 let changelog_marker = "## Changelog";
 let header_pos = find_line_start_marker(content, changelog_marker)?;

 // After the header, scan forward to find either:
 // - Next `## ` heading at byte-of-line (sibling section boundary)
 // - Next `# ` heading at byte-of-line (parent boundary, very unlikely after changelog)
 // - EOF
 let after_header = &content[header_pos..];
 let mut cursor = 0usize;
 let mut last_known_end = after_header.len();

 while cursor < after_header.len() {
 let rest = &after_header[cursor..];
 let line_end = rest.find('\n').map(|n| n + 1).unwrap_or(rest.len());
 let line = &rest[..line_end];
 // Skip the changelog header line itself (cursor==0).
 if cursor != 0 && (line.starts_with("## ") || line.starts_with("# ")) {
 last_known_end = cursor;
 break;
 }
 cursor += line_end;
 }

 Some(header_pos + last_known_end)
}

/// Find first byte position of `marker` such that it begins at the start of a line
/// (file start or preceded by `\n`). Returns None if no line-start match.
fn find_line_start_marker(content: &str, marker: &str) -> Option<usize> {
 let mut search_from = 0usize;
 while search_from < content.len() {
 let rest = &content[search_from..];
 let rel = rest.find(marker)?;
 let abs = search_from + rel;
 // Check if abs is at start of file or immediately after a newline.
 if abs == 0 || content.as_bytes()[abs - 1] == b'\n' {
 return Some(abs);
 }
 // Otherwise advance past this match and keep searching.
 search_from = abs + marker.len();
 }
 None
}

/// Format new entry text — `- Round N (TITLE):\n - bullet 1\n - bullet 2\n`.
fn format_new_entry(entry_id: &str, title: Option<&str>, sub_bullets: &[String]) -> String {
 let mut out = String::new();
 // Leading blank line for separation from previous entry's last bullet.
 out.push('\n');
 if let Some(t) = title {
 out.push_str(&format!("- {} ({}):\n", entry_id, t));
 } else {
 out.push_str(&format!("- {}:\n", entry_id));
 }
 for sub in sub_bullets {
 out.push_str("  - ");
 out.push_str(sub);
 out.push('\n');
 }
 out
}

/// Atomic write — temp file + rename. Blocks partial writes.
fn atomic_write(path: &Path, content: &str) -> io::Result<()> {
 let parent = path.parent().unwrap_or_else(|| Path::new("."));
 let file_name = path
 .file_name()
 .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no file name"))?
 .to_string_lossy()
 .into_owned();
 let temp_name = format!(".{}.mnemosyne-mutate.tmp", file_name);
 let temp_path: PathBuf = parent.join(temp_name);
 fs::write(&temp_path, content)?;
 fs::rename(&temp_path, path)?;
 Ok(())
}

// ============================================================================
// remaining 4 mutate primitive (Phase 0c entry #3).
// ============================================================================

use crate::schema::RefKind;


/// `add_cross_ref` primitive — append `§{to_target}` reference text into
/// from_section's body.
pub fn add_cross_ref(
 workspace: &Workspace,
 doc_path: &str,
 from_section: &str,
 to_target: &str,
 ref_kind: RefKind,
 docs_root: &Path,
) -> Result<MutateReceipt, MutateError> {
 const PRIM: &str = "add_cross_ref";

 let prev = workspace.docs.get(doc_path).ok_or_else(|| MutateError {
 primitive: PRIM.to_string(),
 kind: MutateErrorKind::NotFound,
 detail: format!("doc `{}` not loaded in workspace", doc_path),
 })?;

 if !prev.sections.iter().any(|s| s.section_id == from_section) {
 return Err(MutateError {
 primitive: PRIM.to_string(),
 kind: MutateErrorKind::NotFound,
 detail: format!("from_section `{}` not in doc", from_section),
 });
 }

 // For Decision/Impl ref_kind: to_target must exist as section_id in workspace
 // (intra-doc or default-doc cross-ref). For CrossDoc: to_target accepted as-is.
 if ref_kind != RefKind::CrossDoc {
 let intra = prev.sections.iter().any(|s| s.section_id == to_target);
 let cross = workspace.default_doc_has_section(to_target);
 if !intra && !cross {
 return Err(MutateError {
  primitive: PRIM.to_string(),
  kind: MutateErrorKind::OrphanRejection,
  detail: format!(
  "to_target `{}` not found in intra-doc or default-doc — would be orphan",
  to_target
  ),
 });
 }
 }

 let abs_path = docs_root.join(doc_path);
 let original = fs::read_to_string(&abs_path).map_err(|e| MutateError {
 primitive: PRIM.to_string(),
 kind: MutateErrorKind::IoError,
 detail: format!("read: {}", e),
 })?;

 let insert_pos = find_section_end_position(&original, from_section).ok_or_else(|| {
 MutateError {
 primitive: PRIM.to_string(),
 kind: MutateErrorKind::StructuralVerificationFailed,
 detail: format!("could not find body end for section `{}`", from_section),
 }
 })?;

 let inline_text = match ref_kind {
 RefKind::Decision => format!("§{}", to_target),
 RefKind::Impl => format!("[link]({})", to_target),
 RefKind::CrossDoc => format!("[link]({})", to_target),
 };
 let new_paragraph = format!("\nReferences {} (Round 125 add_cross_ref).\n\n", inline_text);

 let mut new_content = String::with_capacity(original.len() + new_paragraph.len());
 new_content.push_str(&original[..insert_pos]);
 new_content.push_str(&new_paragraph);
 new_content.push_str(&original[insert_pos..]);

 atomic_write(&abs_path, &new_content).map_err(|e| MutateError {
 primitive: PRIM.to_string(),
 kind: MutateErrorKind::IoError,
 detail: format!("atomic write: {}", e),
 })?;

 let from_section_owned = from_section.to_string();
 let to_target_owned = to_target.to_string();
 let result = validate_general_after_write(
 workspace,
 doc_path,
 prev,
 &abs_path,
 PRIM,
 |reparsed| {
 // Verify: reparsed has at least one new cross_ref matching (from, to, kind).
 let prev_set: BTreeSet<(String, String, RefKind)> = prev
  .cross_refs
  .iter()
  .map(|c| (c.from_section.clone(), c.to_target.clone(), c.ref_kind))
  .collect();
 let new_match = reparsed.cross_refs.iter().any(|c| {
  c.from_section == from_section_owned
  && (c.to_target == to_target_owned
  || c.to_target.ends_with(&format!("#§{}", to_target_owned)))
  && !prev_set.contains(&(
  c.from_section.clone(),
  c.to_target.clone(),
  c.ref_kind,
  ))
 });
 if !new_match {
  return Err(MutateError {
  primitive: PRIM.to_string(),
  kind: MutateErrorKind::StructuralVerificationFailed,
  detail: format!(
  "no new cross_ref from §{} to {} found in reparsed",
  from_section_owned, to_target_owned
  ),
  });
 }
 Ok(vec![from_section_owned.clone()])
 },
 );

 finalize_mutate(
 result,
 PRIM,
 doc_path,
 &abs_path,
 &original,
 &new_content,
 0,
 )
}

/// `set_section_decision_status` primitive — mutate API surface
///.
///
/// **Phase 1+ schema extension carry** — current parser hardcodes
/// `decision_status: DecisionStatus::Active` for all parsed sections (no
/// markdown convention captures the field). This primitive's semantic meaning is mutate
/// ChangelogEntry schema path's `decision_status` field is decoupled from the markdown body.
/// convention (e.g. section body in `**Decision status: superseded by §X**`
/// marker) — the parser/emitter pair can carry this once a post-design lands. 's
/// out-of-scope (Phase 1A+ schema-extension carry).
///
/// This stub registers the spec surface (mutate API surface — all 5 primitives
/// production wire) — placeholder; invoked on `MutateErrorKind::ValidatorReject`
/// + Phase 1 schema extension explicit detail.
pub fn set_section_decision_status(
 workspace: &Workspace,
 doc_path: &str,
 section_id: &str,
 new_status: crate::schema::DecisionStatus,
 superseding: Option<&str>,
 _docs_root: &Path,
) -> Result<MutateReceipt, MutateError> {
 const PRIM: &str = "set_section_decision_status";

 let prev = workspace.docs.get(doc_path).ok_or_else(|| MutateError {
 primitive: PRIM.to_string(),
 kind: MutateErrorKind::NotFound,
 detail: format!("doc `{}` not loaded in workspace", doc_path),
 })?;

 if !prev.sections.iter().any(|s| s.section_id == section_id) {
 return Err(MutateError {
 primitive: PRIM.to_string(),
 kind: MutateErrorKind::NotFound,
 detail: format!("section_id `{}` not in doc", section_id),
 });
 }

 // T1 rule 4 surface validation — active → superseded transition requires
 // superseding cross-ref. this stub validation only carry, write path -
 // Phase 1+ carry.
 if new_status == crate::schema::DecisionStatus::Superseded && superseding.is_none() {
 return Err(MutateError {
 primitive: PRIM.to_string(),
 kind: MutateErrorKind::ValidatorReject,
 detail: "superseding section_id mandatory for active → superseded transition (T1 rule 4)".to_string(),
 });
 }

 Err(MutateError {
 primitive: PRIM.to_string(),
 kind: MutateErrorKind::ValidatorReject,
 detail: format!(
 "Phase 1+ schema extension carry — parser hardcodes DecisionStatus::Active for all sections (Round 125 stub). \
  requested change of `{}` to `{:?}` (superseding={:?}) deferred until parser/emitter \
  pair adds DecisionStatus marker convention ( e.g. section body `**Decision status: \
  superseded by §X**` markdown form). this primitive §15 mutate API surface registration only.",
 section_id, new_status, superseding
 ),
 })
}

/// `set_section_body` primitive — replace body content between section heading
/// and next sibling heading.
pub fn set_section_body(
 workspace: &Workspace,
 doc_path: &str,
 section_id: &str,
 new_body: &str,
 docs_root: &Path,
) -> Result<MutateReceipt, MutateError> {
 const PRIM: &str = "set_section_body";

 let prev = workspace.docs.get(doc_path).ok_or_else(|| MutateError {
 primitive: PRIM.to_string(),
 kind: MutateErrorKind::NotFound,
 detail: format!("doc `{}` not loaded in workspace", doc_path),
 })?;

 if !prev.sections.iter().any(|s| s.section_id == section_id) {
 return Err(MutateError {
 primitive: PRIM.to_string(),
 kind: MutateErrorKind::NotFound,
 detail: format!("section_id `{}` not in doc", section_id),
 });
 }

 let abs_path = docs_root.join(doc_path);
 let original = fs::read_to_string(&abs_path).map_err(|e| MutateError {
 primitive: PRIM.to_string(),
 kind: MutateErrorKind::IoError,
 detail: format!("read: {}", e),
 })?;

 // Find section's heading line + end of body (next sibling heading or EOF).
 let (body_start, body_end) =
 find_section_body_range(&original, section_id).ok_or_else(|| MutateError {
 primitive: PRIM.to_string(),
 kind: MutateErrorKind::StructuralVerificationFailed,
 detail: format!(
  "could not locate body range for section `{}` (heading detection failed)",
  section_id
 ),
 })?;

 let mut new_content = String::with_capacity(original.len() + new_body.len());
 new_content.push_str(&original[..body_start]);
 new_content.push_str(new_body);
 if !new_body.ends_with('\n') {
 new_content.push('\n');
 }
 if !new_content.ends_with("\n\n") {
 new_content.push('\n');
 }
 new_content.push_str(&original[body_end..]);

 atomic_write(&abs_path, &new_content).map_err(|e| MutateError {
 primitive: PRIM.to_string(),
 kind: MutateErrorKind::IoError,
 detail: format!("atomic write: {}", e),
 })?;

 let section_id_owned = section_id.to_string();
 let result = validate_general_after_write(
 workspace,
 doc_path,
 prev,
 &abs_path,
 PRIM,
 |reparsed| {
 // Verify: section still exists with same id.
 if !reparsed
  .sections
  .iter()
  .any(|s| s.section_id == section_id_owned)
 {
  return Err(MutateError {
  primitive: PRIM.to_string(),
  kind: MutateErrorKind::StructuralVerificationFailed,
  detail: format!(
  "section `{}` not found in reparsed after body replace",
  section_id_owned
  ),
  });
 }
 Ok(vec![section_id_owned.clone()])
 },
 );

 finalize_mutate(
 result,
 PRIM,
 doc_path,
 &abs_path,
 &original,
 &new_content,
 0,
 )
}

// ============================================================================
// Helpers for primitives.
// ============================================================================

fn slug_for_unnumbered_external(title: &str) -> String {
 let mut buf = String::new();
 let mut prev_space = false;
 for ch in title.chars() {
 if ch.is_whitespace() {
 if !prev_space && !buf.is_empty() {
  buf.push('-');
 }
 prev_space = true;
 continue;
 }
 prev_space = false;
 if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
 buf.push(ch.to_ascii_lowercase());
 } else if (ch as u32) >= 0x3040 && (ch as u32) <= 0x9FFF {
 buf.push(ch);
 } else if (ch as u32) >= 0xAC00 && (ch as u32) <= 0xD7AF {
 buf.push(ch);
 }
 }
 if buf.is_empty() {
 return "section".to_string();
 }
 buf
}



/// Find byte position at end of section's body (just before next heading at
/// same or shallower depth, or EOF).
///
/// Heuristic: scan from section's heading line forward. Look for next line
/// that starts with `# ` or `## ` or `### ` etc. with **≤** the heading depth.
/// If none, return content.len().
fn find_section_end_position(content: &str, section_id: &str) -> Option<usize> {
 let heading_pos = find_section_heading(content, section_id)?;
 // Determine current heading's depth (count `#` chars at start).
 let after = &content[heading_pos..];
 let line_end = after.find('\n').map(|n| n + 1).unwrap_or(after.len());
 let heading_line = &after[..line_end];
 let cur_depth = heading_line
 .chars()
 .take_while(|c| *c == '#')
 .count();
 if cur_depth == 0 {
 return None;
 }
 // Scan forward from after the heading line.
 let mut cursor = line_end;
 while cursor < after.len() {
 let rest = &after[cursor..];
 let line_end_inner = rest.find('\n').map(|n| n + 1).unwrap_or(rest.len());
 let line = &rest[..line_end_inner];
 // Check if line starts with `#` chars + space.
 let depth = line.chars().take_while(|c| *c == '#').count();
 if depth > 0 && depth <= cur_depth {
 // Verify it's actually a heading (depth #s followed by space).
 if line.as_bytes().get(depth) == Some(&b' ') {
  return Some(heading_pos + cursor);
 }
 }
 cursor += line_end_inner;
 }
 Some(content.len())
}

/// Find byte position of section heading line by section_id.
///
/// Tracks a section stack while scanning so nested sub-sections under numbered
/// or unnumbered parents resolve to the correct `{parent}/{slug}` form (parser
/// rule, parser.rs:257-263). fix — pre-128 logic was predicted from the title
/// alone (no parent prefix), causing mutate API sub-section ops to fail with
/// StructuralVerificationFailed for any depth ≥ 3 section.
fn find_section_heading(content: &str, section_id: &str) -> Option<usize> {
 let mut cursor = 0usize;
 let mut stack: Vec<(String, usize)> = Vec::new();
 while cursor < content.len() {
 let rest = &content[cursor..];
 let line_end = rest.find('\n').map(|n| n + 1).unwrap_or(rest.len());
 let line = &rest[..line_end];
 // Check for line-start match (cursor==0 OR previous char was \n).
 let line_start_ok = cursor == 0 || content.as_bytes()[cursor - 1] == b'\n';
 if line_start_ok {
 let depth = line.chars().take_while(|c| *c == '#').count();
 if depth > 0 && line.as_bytes().get(depth) == Some(&b' ') {
  let title_part = line[depth + 1..].trim_end_matches('\n').trim_end();
  while let Some(&(_, top_depth)) = stack.last() {
  if top_depth >= depth {
  stack.pop();
  } else {
  break;
  }
  }
  let parent_id = stack.last().map(|(id, _)| id.as_str());
  let predicted = predict_section_id_for_heading(title_part, depth, parent_id);
  if predicted == section_id {
  return Some(cursor);
  }
  stack.push((predicted, depth));
 }
 }
 cursor += line_end;
 }
 None
}

/// Build section_id matching the parser's rule (parser.rs:257-263) given the
/// heading's title, markdown depth, and parent section_id (if any).
fn predict_section_id_for_heading(
 title_part: &str,
 depth: usize,
 parent_id: Option<&str>,
) -> String {
 let trimmed = title_part.trim();
 let (number_opt, _rest) = parse_leading_section_number(trimmed);
 match (number_opt, depth, parent_id) {
 (Some(n), 2, _) => n,
 (Some(n), _, Some(pid)) => format!("{}/{}", pid, n),
 (Some(n), _, None) => n,
 (None, _, None) => slug_for_unnumbered_external(trimmed),
 (None, _, Some(pid)) => format!("{}/{}", pid, slug_for_unnumbered_external(trimmed)),
 }
}

/// Extract a leading section-number prefix from a heading title, mirroring
/// `parser::split_section_number` (parser.rs:209). fix — pre-132
/// logic required a `.` after the digit run, so headings like `### 6axis
/// enforce` (parsed by parser as section_number="6") were missed by the
/// mutate-API lookup. The parser accepts any digit-or-dot run as the prefix
/// and trims a trailing `.` plus whitespace from the remainder.
fn parse_leading_section_number(s: &str) -> (Option<String>, String) {
 let bytes = s.as_bytes();
 let mut idx = 0usize;
 let mut saw_digit = false;
 while idx < bytes.len() {
 let b = bytes[idx];
 if b.is_ascii_digit() || (b == b'.' && saw_digit) {
 if b.is_ascii_digit() {
  saw_digit = true;
 }
 idx += 1;
 } else {
 break;
 }
 }
 if !saw_digit {
 return (None, s.to_string());
 }
 let mut num_end = idx;
 let mut number_str: &str = &s[..num_end];
 if number_str.ends_with('.') {
 num_end -= 1;
 number_str = &s[..num_end];
 }
 let rest = &s[idx..];
 let rest = rest.strip_prefix('.').unwrap_or(rest);
 let rest = rest.trim_start().to_string();
 if number_str.is_empty() {
 return (None, s.to_string());
 }
 (Some(number_str.to_string()), rest)
}

/// Find section's body byte range — from line after heading to next sibling
/// heading (or EOF). Returns (body_start, body_end).
fn find_section_body_range(content: &str, section_id: &str) -> Option<(usize, usize)> {
 let heading_pos = find_section_heading(content, section_id)?;
 let after = &content[heading_pos..];
 let line_end = after.find('\n').map(|n| n + 1).unwrap_or(after.len());
 let body_start = heading_pos + line_end;
 // fix: body ends at the FIRST heading after this section's heading
 // (any depth ≥ 1) — not at the next sibling. The pre-129 logic used
 // find_section_end_position which scans for `depth ≤ cur_depth`, so
 // set_section_body would overwrite all nested sub-sections inside the
 // section's range. Caught when 's first attempt at deleted
 // 13 sub-sections (~290 lines) silently while passing round-trip.
 let body_end = find_first_heading_after(content, body_start).unwrap_or(content.len());
 Some((body_start, body_end))
}

/// Scan from `start` for the next line beginning with `#` characters followed
/// by a space (any depth). Returns the byte offset of that line, or None if
/// no further heading is found. `start` must be a line boundary.
fn find_first_heading_after(content: &str, start: usize) -> Option<usize> {
 let mut cursor = start;
 while cursor < content.len() {
 let rest = &content[cursor..];
 let line_end = rest.find('\n').map(|n| n + 1).unwrap_or(rest.len());
 let line = &rest[..line_end];
 let depth = line.chars().take_while(|c| *c == '#').count();
 if depth > 0 && line.as_bytes().get(depth) == Some(&b' ') {
 return Some(cursor);
 }
 cursor += line_end;
 }
 None
}

/// Generic post-write validation pass — T2 + T1 + round-trip + caller verification.
fn validate_general_after_write<V>(
 workspace: &Workspace,
 doc_path: &str,
 prev: &ParsedDoc,
 abs_path: &Path,
 primitive: &str,
 structural_verify: V,
) -> Result<Vec<String>, MutateError>
where
 V: FnOnce(&ParsedDoc) -> Result<Vec<String>, MutateError>,
{
 let written = fs::read_to_string(abs_path).map_err(|e| MutateError {
 primitive: primitive.to_string(),
 kind: MutateErrorKind::IoError,
 detail: format!("re-read {}: {}", abs_path.display(), e),
 })?;
 let reparsed = parse_markdown(&written, doc_path);

 // Caller-supplied structural verification.
 let affected_sections = structural_verify(&reparsed)?;

 // T2 frozen_ledger_jaccard.
 let t2 = frozen_ledger_jaccard(prev, &reparsed);
 if !t2.is_empty() {
 return Err(MutateError {
 primitive: primitive.to_string(),
 kind: MutateErrorKind::FrozenLedgerViolation,
 detail: format!("{} T2 violation(s)", t2.len()),
 });
 }

 // T1 new orphans.
 let mut new_ws = workspace.clone();
 new_ws.insert(doc_path.to_string(), reparsed.clone());
 let prev_orphans: BTreeSet<(String, String)> =
 cross_ref_orphan_reject_with_workspace(prev, workspace)
 .into_iter()
 .filter_map(orphan_key)
 .collect();
 let new_orphans: BTreeSet<(String, String)> =
 cross_ref_orphan_reject_with_workspace(&reparsed, &new_ws)
 .into_iter()
 .filter_map(orphan_key)
 .collect();
 let introduced: Vec<&(String, String)> = new_orphans.difference(&prev_orphans).collect();
 if !introduced.is_empty() {
 return Err(MutateError {
 primitive: primitive.to_string(),
 kind: MutateErrorKind::OrphanRejection,
 detail: format!(
  "{} new orphan(s): {:?}",
  introduced.len(),
  introduced
  .iter()
  .map(|(f, t)| format!("§{}→§{}", f, t))
  .collect::<Vec<_>>()
 ),
 });
 }

 // Round-trip diff.
 let emitted = emit_markdown_with_default(&reparsed, new_ws.default_doc.as_deref());
 let reemit_parsed = parse_markdown(&emitted, doc_path);
 let diff = compare_typed_facts(&reparsed, &reemit_parsed);
 if !diff.mandatory_preserved {
 return Err(MutateError {
 primitive: primitive.to_string(),
 kind: MutateErrorKind::RoundTripDriftError,
 detail: format!(
  "round-trip drift: section {}->{}, changelog {}->{}, cross_ref {}->{}",
  diff.section_count_a,
  diff.section_count_b,
  diff.changelog_entry_count_a,
  diff.changelog_entry_count_b,
  diff.cross_ref_count_a,
  diff.cross_ref_count_b
 ),
 });
 }

 Ok(affected_sections)
}

/// Build receipt + handle rollback on failure.
fn finalize_mutate(
 validation: Result<Vec<String>, MutateError>,
 primitive: &str,
 doc_path: &str,
 abs_path: &Path,
 original: &str,
 new_content: &str,
 applied_at_transaction_time: i64,
) -> Result<MutateReceipt, MutateError> {
 match validation {
 Ok(affected_sections) => Ok(MutateReceipt {
 primitive: primitive.to_string(),
 affected_docs: vec![doc_path.to_string()],
 affected_sections,
 written_bytes_per_doc: {
  let mut m = BTreeMap::new();
  m.insert(doc_path.to_string(), new_content.len());
  m
 },
 round_trip_diff_count: 0,
 validator_path_invocations: vec![
  "t2::frozen_ledger_jaccard".to_string(),
  "t1::cross_ref_orphan_reject_with_workspace".to_string(),
  "structural_verification".to_string(),
 ],
 applied_at_transaction_time,
 }),
 Err(err) => {
 let _ = atomic_write(abs_path, original);
 Err(err)
 }
 }
}

#[cfg(test)]
mod tests {
 use super::*;
 use crate::schema::{ChangelogEntry, DecisionStatus, Section};
 use std::io::Write;

 fn write_temp_doc(content: &str) -> tempfile::TempDir {
 let dir = tempfile::tempdir().unwrap();
 let docs = dir.path().join("docs");
 fs::create_dir_all(&docs).unwrap();
 let path = docs.join("TEST.md");
 let mut f = fs::File::create(&path).unwrap();
 f.write_all(content.as_bytes()).unwrap();
 dir
 }

 fn minimal_doc_with_changelog(prev_entries: usize) -> String {
 let mut s = String::new();
 s.push_str("# Test Doc\n\n## §1 First\n\nbody1\n\n## §2 Second\n\nbody2\n\n## Changelog\n\n");
 for i in 1..=prev_entries {
 s.push_str(&format!("- Round {}:\n  - bullet text {}\n", i, i));
 }
 s
 }

 fn parse_workspace(_dir: &Path, content: &str) -> Workspace {
 let parsed = parse_markdown(content, "docs/TEST.md");
 let mut ws = Workspace::new();
 ws.insert("docs/TEST.md".to_string(), parsed);
 ws
 }

 #[test]
 fn parse_entry_n_parses_round_n() {
 assert_eq!(parse_entry_n("Round 124"), Some(124));
 assert_eq!(parse_entry_n("Round 1"), Some(1));
 assert_eq!(parse_entry_n("Round abc"), None);
 assert_eq!(parse_entry_n("not extension"), None);
 }

 #[test]
 fn find_insert_position_after_last_entry() {
 let content = "# A\n\n## Changelog\n\n- Round 1:\n  - x\n- Round 2:\n  - y\n";
 let pos = find_changelog_insert_position(content).unwrap();
 // Insert position should be EOF since no following ## section.
 assert_eq!(pos, content.len());
 }

 #[test]
 fn find_insert_position_before_next_section() {
 let content = "# A\n\n## Changelog\n\n- Round 1:\n  - x\n\n## Next\n\nbody\n";
 let pos = find_changelog_insert_position(content).unwrap();
 let inserted = &content[..pos];
 assert!(inserted.ends_with("- x\n\n"), "inserted={:?}", inserted);
 let after = &content[pos..];
 assert!(after.starts_with("## Next"), "after={:?}", after);
 }

 #[test]
 fn find_insert_position_no_changelog_returns_none() {
 let content = "# A\n\n## §1 Body\n";
 assert!(find_changelog_insert_position(content).is_none());
 }

 #[test]
 fn format_new_entry_with_title() {
 let out = format_new_entry(
 "Round 124",
 Some("APPEND-CHANGELOG-ENTRY"),
 &["bullet a".to_string(), "bullet b".to_string()],
 );
 assert!(out.contains("- Round 124 (APPEND-CHANGELOG-ENTRY):\n"));
 assert!(out.contains(" - bullet a\n"));
 assert!(out.contains(" - bullet b\n"));
 }

 #[test]
 fn format_new_entry_without_title() {
 let out = format_new_entry("Round 5", None, &["x".to_string()]);
 assert!(out.contains("- Round 5:\n"));
 assert!(!out.contains("("));
 }

 #[test]
 fn append_succeeds_on_minimal_doc() {
 let content = minimal_doc_with_changelog(2);
 let dir = write_temp_doc(&content);
 let ws = parse_workspace(dir.path(), &content);

 let receipt = append_changelog_entry(
 &ws,
 "docs/TEST.md",
 "Round 3",
 Some("TEST-TITLE"),
 &["new bullet 1".to_string(), "new bullet 2".to_string()],
 999,
 dir.path(),
 )
 .expect("append should succeed");

 assert_eq!(receipt.primitive, "append_changelog_entry");
 assert_eq!(receipt.affected_docs, vec!["docs/TEST.md"]);
 assert_eq!(receipt.round_trip_diff_count, 0);
 assert_eq!(receipt.applied_at_transaction_time, 999);

 // Verify on-disk content has new entry.
 let after = fs::read_to_string(dir.path().join("docs/TEST.md")).unwrap();
 assert!(after.contains("- Round 3 (TEST-TITLE):"));
 assert!(after.contains(" - new bullet 1"));
 assert!(after.contains(" - new bullet 2"));
 // Pre-existing entries preserved.
 assert!(after.contains("- Round 1:"));
 assert!(after.contains("- Round 2:"));
 }

 #[test]
 fn append_rejects_non_monotonic_entry_id() {
 let content = minimal_doc_with_changelog(2);
 let dir = write_temp_doc(&content);
 let ws = parse_workspace(dir.path(), &content);

 let err = append_changelog_entry(
 &ws,
 "docs/TEST.md",
 "Round 5", // skipped 3 and 4
 None,
 &["x".to_string()],
 999,
 dir.path(),
 )
 .expect_err("non-monotonic entry_id should reject");

 assert_eq!(err.kind, MutateErrorKind::MonotonicViolation);
 // Verify file was rolled back (no change).
 let after = fs::read_to_string(dir.path().join("docs/TEST.md")).unwrap();
 assert_eq!(after, content);
 }

 #[test]
 fn append_rejects_non_monotonic_transaction_time() {
 // Build doc with explicit txn_time so the prev frozen_at_transaction_time is known.
 let content = "# A\n\n## §1 A\n\n## Changelog\n\n- Round 1:\n  - first\n";
 let dir = write_temp_doc(content);
 let mut ws = parse_workspace(dir.path(), content);
 // Override the parser's monotonic stamp so the new entry's txn_time is older.
 let parsed = ws.docs.get_mut("docs/TEST.md").unwrap();
 parsed.changelog_entries[0].frozen_at_transaction_time = 1000;

 let err = append_changelog_entry(
 &ws,
 "docs/TEST.md",
 "Round 2",
 None,
 &["new".to_string()],
 500, // less than 1000
 dir.path(),
 )
 .expect_err("non-monotonic txn_time should reject");
 assert_eq!(err.kind, MutateErrorKind::MonotonicViolation);
 }

 #[test]
 fn append_rejects_empty_sub_bullets() {
 let content = minimal_doc_with_changelog(1);
 let dir = write_temp_doc(&content);
 let ws = parse_workspace(dir.path(), &content);

 let err = append_changelog_entry(
 &ws,
 "docs/TEST.md",
 "Round 2",
 None,
 &[],
 999,
 dir.path(),
 )
 .expect_err("empty sub_bullets should reject");
 assert_eq!(err.kind, MutateErrorKind::AppendOnlyViolation);
 }

 #[test]
 fn append_rejects_invalid_entry_id_prefix() {
 let content = minimal_doc_with_changelog(1);
 let dir = write_temp_doc(&content);
 let ws = parse_workspace(dir.path(), &content);

 let err = append_changelog_entry(
 &ws,
 "docs/TEST.md",
 "ADR-2", // wrong prefix; must start with `Round `
 None,
 &["x".to_string()],
 999,
 dir.path(),
 )
 .expect_err("invalid entry_id prefix should reject");
 assert_eq!(err.kind, MutateErrorKind::AppendOnlyViolation);
 }

 #[test]
 fn append_rejects_doc_without_changelog_section() {
 let content = "# A\n\n## §1 Body\n\nbody text\n";
 let dir = write_temp_doc(content);
 let ws = parse_workspace(dir.path(), content);

 let err = append_changelog_entry(
 &ws,
 "docs/TEST.md",
 "Round 1",
 None,
 &["x".to_string()],
 999,
 dir.path(),
 )
 .expect_err("doc without Changelog should reject");
 assert_eq!(err.kind, MutateErrorKind::StructuralVerificationFailed);
 }

 #[test]
 fn append_first_entry_to_empty_changelog() {
 let content = "# A\n\n## §1 Body\n\nbody\n\n## Changelog\n";
 let dir = write_temp_doc(content);
 let ws = parse_workspace(dir.path(), content);

 let receipt = append_changelog_entry(
 &ws,
 "docs/TEST.md",
 "Round 1",
 Some("FIRST"),
 &["initial bullet".to_string()],
 42,
 dir.path(),
 )
 .expect("first entry should succeed");
 assert_eq!(receipt.applied_at_transaction_time, 42);

 let after = fs::read_to_string(dir.path().join("docs/TEST.md")).unwrap();
 assert!(after.contains("- Round 1 (FIRST):"));
 assert!(after.contains(" - initial bullet"));
 }

 #[test]
 fn append_preserves_byte_content_outside_changelog() {
 let content = "# Test Doc\n\n## §1 First\n\nbody1 with **bold** text\n\n## §2 Second\n\nbody2 with `code`\n\n## Changelog\n\n- Round 1:\n  - x\n";
 let dir = write_temp_doc(content);
 let ws = parse_workspace(dir.path(), content);

 let _ = append_changelog_entry(
 &ws,
 "docs/TEST.md",
 "Round 2",
 None,
 &["new".to_string()],
 999,
 dir.path(),
 )
 .expect("append should succeed");

 let after = fs::read_to_string(dir.path().join("docs/TEST.md")).unwrap();
 // Byte-level preservation of pre-changelog content.
 assert!(after.contains("body1 with **bold** text"));
 assert!(after.contains("body2 with `code`"));
 assert!(after.contains("- Round 1:\n  - x"));
 assert!(after.contains("- Round 2:\n  - new"));
 }

 // Suppress unused imports warning when tests omitted.
 #[allow(dead_code)]
 fn _unused_imports_silencer() {
 let _ = Section {
 section_id: String::new(),
 parent_doc: String::new(),
 parent_section: None,
 title: String::new(),
 decision_status: DecisionStatus::Active,
 };
 let _ = ChangelogEntry {
 entry_id: String::new(),
 parent_changelog_entry: None,
 sub_bullets: Vec::new(),
 frozen_at_transaction_time: 0,
 };
 }
}
