//! Markdown surgical-insert mutate primitives — `add_cross_ref`,
//! `set_section_decision_status`, `set_section_body`. Each primitive
//! performs an atomic byte-preserving edit on the target doc with
//! rollback-on-failure: read snapshot → surgical insert → atomic write
//! → re-parse → T1/T2/structural validation → rollback if any check
//! fails.
//!
//! The atomic-store mutate API (`crate::atomic::*`) supersedes these
//! for new authoring; the surgical-insert primitives here remain for
//! the narrow set of markdown-fact edits that have no atomic-store
//! analogue yet.

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
 let mut in_code_fence = false;
 while cursor < after.len() {
 let rest = &after[cursor..];
 let line_end_inner = rest.find('\n').map(|n| n + 1).unwrap_or(rest.len());
 let line = &rest[..line_end_inner];
 // Code-fence guard — symmetric with `find_section_heading` and
 // `find_first_heading_after`. Without it, an inline `# define` or
 // `# Generated by ...` line inside a fenced block ends the
 // section prematurely (e.g. add_cross_ref insert point lands
 // mid-fence).
 if is_code_fence_line(line) {
 in_code_fence = !in_code_fence;
 cursor += line_end_inner;
 continue;
 }
 if !in_code_fence {
 let depth = line.chars().take_while(|c| *c == '#').count();
 if depth > 0 && depth <= cur_depth {
  // Verify it's actually a heading (depth #s followed by space).
  if line.as_bytes().get(depth) == Some(&b' ') {
  return Some(heading_pos + cursor);
  }
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
 let mut in_code_fence = false;
 while cursor < content.len() {
 let rest = &content[cursor..];
 let line_end = rest.find('\n').map(|n| n + 1).unwrap_or(rest.len());
 let line = &rest[..line_end];
 // Check for line-start match (cursor==0 OR previous char was \n).
 let line_start_ok = cursor == 0 || content.as_bytes()[cursor - 1] == b'\n';
 if line_start_ok {
 // CommonMark code-fence boundary — 0-3 leading spaces + ```.
 // Lines inside a fence are verbatim, never heading. Mirrors
 // parser.rs::parse_markdown_with_schema; without this guard the
 // mutate API treats inline `#define` / `# comment` lines as
 // section headings and corrupts the parent-prefix stack.
 if is_code_fence_line(line) {
  in_code_fence = !in_code_fence;
  cursor += line_end;
  continue;
 }
 if !in_code_fence {
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
 }
 cursor += line_end;
 }
 None
}

/// CommonMark code-fence boundary. True when `line` opens or closes a
/// fenced code block (0-3 leading spaces followed by triple-backtick).
/// Mirrors the detection used by `parser::parse_markdown_with_schema` so
/// mutate-side scans stay consistent with parser-side section extraction.
fn is_code_fence_line(line: &str) -> bool {
 let no_eol = line.trim_end_matches('\n');
 let leading_ws = no_eol.len() - no_eol.trim_start().len();
 leading_ws <= 3 && no_eol.trim_start().starts_with("```")
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
 let mut in_code_fence = false;
 while cursor < content.len() {
 let rest = &content[cursor..];
 let line_end = rest.find('\n').map(|n| n + 1).unwrap_or(rest.len());
 let line = &rest[..line_end];
 // Code-fence guard mirrors `find_section_heading` — inline `#`
 // lines inside fenced blocks must not be promoted to section
 // boundaries, otherwise `set_section_body` truncates the target
 // body at the first inlined `#define` / `# comment` line.
 if is_code_fence_line(line) {
 in_code_fence = !in_code_fence;
 cursor += line_end;
 continue;
 }
 if !in_code_fence {
 let depth = line.chars().take_while(|c| *c == '#').count();
 if depth > 0 && line.as_bytes().get(depth) == Some(&b' ') {
  return Some(cursor);
 }
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


 #[test]
 fn find_section_heading_ignores_inline_hash_inside_code_fence() {
 // RFC-style doc with a fenced block carrying `#define` lines — the
 // mutate API must not treat them as H1 headings (would corrupt the
 // parent-prefix stack and break depth-3 lookups).
 let content = "\
# Top Doc

## Appendix C — End State

### What will not deliver:

- Bullet A
- Bullet B

```c
#define FOO 1
# Generated by codegen
#endif
```

### What zero-copy means:

- Bullet C
";
 // Heading lookup for the depth-3 sub-section under Appendix C must
 // resolve despite the fenced `#define` / `# Generated` lines that
 // appear between two ### headings.
 let pos = find_section_heading(
 content,
 "top-doc/appendix-c--end-state/what-will-not-deliver",
 )
 .expect("depth-3 heading should resolve through the fenced block");
 assert!(content[pos..].starts_with("### What will not deliver:"));
 }

 #[test]
 fn find_first_heading_after_skips_fenced_hash_lines() {
 // body_end detection must skip a fenced `# comment` between two
 // headings; otherwise `set_section_body` truncates the section's
 // body at the inline hash and silently drops downstream prose.
 let content = "\
## §1 First

prose before fence

```text
# fenced hash that is NOT a heading
```

prose after fence

## §2 Second

body two
";
 // Start scan from just after the `## §1 First` heading line.
 let after_first = content.find("\n").unwrap() + 1;
 let next = find_first_heading_after(content, after_first)
 .expect("next heading must be ## §2, not the fenced `# fenced...`");
 assert!(content[next..].starts_with("## §2 Second"));
 }

 #[test]
 fn find_section_end_position_skips_fenced_hash_lines() {
 // Same hazard as `find_first_heading_after`: `add_cross_ref`'s
 // insert-after-section point must land after the real next sibling,
 // not on a fenced `## inline` line that lives inside the body.
 //
 // Uses unnumbered headings to keep this test orthogonal to the
 // § / numbered-prefix parser/mutate asymmetry — this test exercises
 // code-fence skipping only.
 let content = "\
# Top Doc

## First Section

prose A

```text
## fenced second-level NOT a heading
```

prose B

## Second Section

body two
";
 let end = find_section_end_position(content, "top-doc/first-section")
 .expect("end-of-section must clear the fenced `## fenced...`");
 assert!(content[end..].starts_with("## Second Section"));
 }

 #[test]
 fn is_code_fence_line_detects_indented_opener() {
 // CommonMark allows 0-3 leading spaces on a fence boundary.
 assert!(is_code_fence_line("```\n"));
 assert!(is_code_fence_line(" ```rust\n"));
 assert!(is_code_fence_line(" ```\n"));
 assert!(is_code_fence_line(" ```text\n"));
 // 4+ spaces is an indented code block, not a fence.
 assert!(!is_code_fence_line("    ```\n"));
 // Bare prose with backticks elsewhere.
 assert!(!is_code_fence_line("inline `code` not a fence\n"));
 assert!(!is_code_fence_line("\n"));
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
 atomic_section_id: None,
 };
 let _ = ChangelogEntry {
 entry_id: String::new(),
 parent_changelog_entry: None,
 sub_bullets: Vec::new(),
 frozen_at_transaction_time: 0,
 };
 }
}
