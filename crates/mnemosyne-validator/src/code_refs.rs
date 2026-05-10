//! Round 256 — code citation verification (Stage 2 of the 3-stage
//! code-citation defense — Round 255 introduced the agent-time CLAUDE.md
//! rule, this module backs the validator-time `validate-code-refs`
//! subcommand, Round 257 + 258 will wire pre-commit / cascade triggers).
//!
//! The scanner walks configured code paths, extracts citation candidates
//! matching the `entry_id_prefix` from `[schema]`, and reports each
//! citation whose target entry_id is missing from the atomic store.
//!
//! ## Pattern derivation
//!
//! No separate regex configuration. The scanner uses the same
//! `entry_id_prefix` the parser uses (Round 144 carry — Mnemosyne preset
//! = `"Round "`, ADR preset = `"ADR-"`). Match shape:
//!
//! ```text
//! \b<prefix><digits>(\.<digits>)?\b
//! ```
//!
//! Word-boundary on both sides excludes identifier-like incidental hits
//! (`TestRound254Helper`, `round_254_helper`) — citations must stand
//! alone in the surrounding text.
//!
//! ## Out of scope (Round 256 MVP)
//!
//! - Section `§<id>` citations + decision_status (Active vs Superseded)
//!  — `AtomicChangelogEntry` has no `decision_status` field; entries are
//!  frozen-ledger by definition. Section status check is a clean
//!  follow-up scope (future round) once Section primitives populate the
//!  atomic store.
//! - Tree-sitter language-aware extraction. v1 is a uniform text scan
//!  (citations live in author-written text — comments, strings, doc
//!  blocks — and grep semantics are good enough). False positives are
//!  absorbed by the existing `[[orphan_ledger]]` pattern (Round 253-254).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// One citation candidate extracted from a source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Citation {
 /// Workspace-relative file path the citation was found in.
 pub file: PathBuf,
 /// 1-indexed line number.
 pub line: usize,
 /// Reconstructed entry_id, e.g. `"Round 254"` or `"Round 33.5"`.
 pub entry_id: String,
}

/// One verification failure surfaced to the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeRefViolation {
 pub citation: Citation,
 pub kind: ViolationKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViolationKind {
 /// `entry_id` not in the atomic store `changelog_entries` map
 /// (hallucinated or refers to a removed entry).
 Missing,
 /// Round 258 — citation matches an explicit decay filter (e.g. an
 /// entry_id the cascade caller knows just transitioned to Superseded).
 /// Surfaced regardless of whether the id is still in the valid set —
 /// the entry exists, but author should review whether the code is
 /// still accurate against the new decision.
 Decay,
}

/// Walk configured paths under `root`, collecting all readable files.
///
/// Skips hidden directories (`.git/`, `.mnemosyne/`), `target/`, and
/// `node_modules/` — these never carry author-written citations.
/// Non-existent configured paths are silently skipped (warned by the
/// caller); the design gives external users a way to declare intent for
/// a path that may exist in some checkouts but not others.
pub fn walk_paths(root: &Path, paths: &[String]) -> std::io::Result<Vec<PathBuf>> {
 let mut out = Vec::new();
 for p in paths {
 let abs = root.join(p);
 if !abs.exists() {
 continue;
 }
 collect_files(&abs, &mut out, true)?;
 }
 out.sort();
 Ok(out)
}

fn collect_files(p: &Path, out: &mut Vec<PathBuf>, is_root: bool) -> std::io::Result<()> {
 if p.is_file() {
 out.push(p.to_path_buf());
 return Ok(());
 }
 if !p.is_dir() {
 return Ok(());
 }
 if !is_root {
 let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
 if name.starts_with('.') || name == "target" || name == "node_modules" {
 return Ok(());
 }
 }
 for entry in std::fs::read_dir(p)? {
 let entry = entry?;
 collect_files(&entry.path(), out, false)?;
 }
 Ok(())
}

/// Extract every `<prefix><digits>(.<digits>)?` citation candidate from
/// `content`, with 1-indexed line numbers. The `prefix` argument is the
/// `[schema].entry_id_prefix` value (default `"Round "`).
pub fn extract_citations(prefix: &str, content: &str) -> Vec<(usize, String)> {
 let mut out = Vec::new();
 if prefix.is_empty() {
 return out;
 }
 for (line_idx, line) in content.lines().enumerate() {
 let mut start = 0;
 while start <= line.len() {
 let rel = match line[start..].find(prefix) {
 Some(r) => r,
 None => break,
 };
 let i = start + rel;
 let prev_ok = i == 0
 || !line[..i]
 .chars()
 .last()
 .map(|c| c.is_alphanumeric() || c == '_')
 .unwrap_or(false);
 if !prev_ok {
 start = i + 1;
 continue;
 }
 let after = &line[i + prefix.len()..];
 match scan_round_number(after) {
 Some(num) => {
 let next_idx = i + prefix.len() + num.len();
 let next_ok = next_idx >= line.len()
 || !line[next_idx..]
  .chars()
  .next()
  .map(|c| c.is_alphanumeric() || c == '_')
  .unwrap_or(false);
 if next_ok {
 out.push((line_idx + 1, format!("{}{}", prefix, num)));
 }
 start = next_idx;
 }
 None => {
 start = i + prefix.len();
 }
 }
 }
 }
 out
}

/// Read `<digits>(.<digits>)?` from the start of `s`. Returns the
/// matched substring, or `None` if `s` does not start with a digit.
/// Trailing `.` without fractional digits is not consumed.
fn scan_round_number(s: &str) -> Option<String> {
 let mut chars = s.chars().peekable();
 let mut buf = String::new();
 while let Some(&c) = chars.peek() {
 if c.is_ascii_digit() {
 buf.push(c);
 chars.next();
 } else {
 break;
 }
 }
 if buf.is_empty() {
 return None;
 }
 if chars.peek() == Some(&'.') {
 let mut probe = chars.clone();
 probe.next();
 let mut frac = String::new();
 while let Some(&c) = probe.peek() {
 if c.is_ascii_digit() {
 frac.push(c);
 probe.next();
 } else {
 break;
 }
 }
 if !frac.is_empty() {
 buf.push('.');
 buf.push_str(&frac);
 }
 }
 Some(buf)
}

/// Scan all configured paths for citations and return violations against
/// `valid_entry_ids`. Files that cannot be read as UTF-8 are silently
/// skipped (binary blobs, lock files, etc.).
///
/// The returned list is sorted by `(file, line)` for deterministic
/// reporting.
pub fn scan_paths(
 workspace_root: &Path,
 paths: &[String],
 prefix: &str,
 valid_entry_ids: &BTreeSet<String>,
) -> std::io::Result<Vec<CodeRefViolation>> {
 scan_paths_filtered(workspace_root, paths, prefix, valid_entry_ids, None)
}

/// Round 258 — same as [`scan_paths`] but the result is restricted to
/// citations whose `entry_id` equals `filter_id` (when `Some`). This is
/// the read side of the *Stage 3 supersede cascade* — when a decision
/// transitions Active → Superseded, the cascade caller invokes this with
/// the superseded entry_id to enumerate the code locations that now
/// reference a stale decision.
///
/// `filter_id = None` ⇒ no filtering (identical to [`scan_paths`]).
///
/// Note: the auto-cascade trigger (post-mutate hook on
/// `set-section-decision-status`) is *not* wired here. That requires
/// `AtomicSection.decision_status` as a first-class atomic field
/// (schema extension carry, future round). Once present, the cascade
/// caller is a one-line invocation of this function.
pub fn scan_paths_filtered(
 workspace_root: &Path,
 paths: &[String],
 prefix: &str,
 valid_entry_ids: &BTreeSet<String>,
 filter_id: Option<&str>,
) -> std::io::Result<Vec<CodeRefViolation>> {
 let files = walk_paths(workspace_root, paths)?;
 let mut violations = Vec::new();
 for abs in files {
 let content = match std::fs::read_to_string(&abs) {
 Ok(c) => c,
 Err(_) => continue,
 };
 let rel = abs
 .strip_prefix(workspace_root)
 .map(|p| p.to_path_buf())
 .unwrap_or(abs.clone());
 for (line, entry_id) in extract_citations(prefix, &content) {
 let matches_filter = filter_id.map(|f| entry_id == f).unwrap_or(false);
 let is_missing = !valid_entry_ids.contains(&entry_id);
 let kind = if matches_filter {
 ViolationKind::Decay
 } else if filter_id.is_none() && is_missing {
 ViolationKind::Missing
 } else {
 continue;
 };
 violations.push(CodeRefViolation {
 citation: Citation {
 file: rel.clone(),
 line,
 entry_id,
 },
 kind,
 });
 }
 }
 violations.sort_by(|a, b| {
 a.citation
 .file
 .cmp(&b.citation.file)
 .then(a.citation.line.cmp(&b.citation.line))
 });
 Ok(violations)
}

#[cfg(test)]
mod tests {
 use super::*;

 #[test]
 fn scan_round_number_plain() {
 assert_eq!(scan_round_number("254 rest"), Some("254".to_string()));
 }

 #[test]
 fn scan_round_number_with_fraction() {
 assert_eq!(scan_round_number("33.5)"), Some("33.5".to_string()));
 }

 #[test]
 fn scan_round_number_trailing_dot_not_consumed() {
 assert_eq!(scan_round_number("254. End"), Some("254".to_string()));
 }

 #[test]
 fn scan_round_number_rejects_non_digit_start() {
 assert_eq!(scan_round_number("foo"), None);
 assert_eq!(scan_round_number(""), None);
 }

 #[test]
 fn extract_citations_basic() {
 let src = "// Round 254 carry\n// see Round 33.5 for sub-round\n";
 let out = extract_citations("Round ", src);
 assert_eq!(
 out,
 vec![
 (1, "Round 254".to_string()),
 (2, "Round 33.5".to_string())
 ]
 );
 }

 #[test]
 fn extract_citations_skips_identifier_like() {
 // Identifier `TestRound254Helper` and `round_254_helper` should not
 // match — word boundary excludes them. Note: `TestRound 254` (with
 // space) WOULD match `Round 254` after `Test`; the boundary is on
 // the prefix start, not at `Round`.
 let src = "TestRound254Helper\nlet round_254_helper = 1;\n";
 let out = extract_citations("Round ", src);
 assert_eq!(out, vec![]);
 }

 #[test]
 fn extract_citations_post_boundary_excludes_alphanumeric_tail() {
 // `Round 254a` is not a citation — the trailing `a` makes it look
 // like an identifier (or typo), not a clean entry_id.
 let src = "see Round 254a here\n";
 let out = extract_citations("Round ", src);
 assert_eq!(out, vec![]);
 }

 #[test]
 fn extract_citations_brackets_and_parens_ok() {
 let src = "(Round 254) [Round 100] {Round 1}\n";
 let out = extract_citations("Round ", src);
 assert_eq!(
 out,
 vec![
 (1, "Round 254".to_string()),
 (1, "Round 100".to_string()),
 (1, "Round 1".to_string())
 ]
 );
 }

 #[test]
 fn extract_citations_external_prefix() {
 let src = "ADR-0042 implements ADR-7\n";
 let out = extract_citations("ADR-", src);
 assert_eq!(
 out,
 vec![
 (1, "ADR-0042".to_string()),
 (1, "ADR-7".to_string())
 ]
 );
 }

 #[test]
 fn extract_citations_empty_prefix_yields_empty() {
 assert!(extract_citations("", "Round 254\n").is_empty());
 }

 #[test]
 fn scan_paths_filtered_decay_surfaces_filter_id_match() {
 // Round 258 — when filter_id is set, citations matching it are
 // reported as Decay regardless of whether the id is in the valid
 // set. Other citations are not reported.
 let tmp = tempfile::tempdir().unwrap();
 let src = tmp.path().join("src");
 std::fs::create_dir_all(&src).unwrap();
 std::fs::write(
 src.join("a.rs"),
 "// Round 1 here\n// Round 5 here\n// Round 1 again\n",
 )
 .unwrap();

 let mut valid = BTreeSet::new();
 valid.insert("Round 1".to_string());
 valid.insert("Round 5".to_string());

 let v = scan_paths_filtered(
 tmp.path(),
 &["src/".to_string()],
 "Round ",
 &valid,
 Some("Round 1"),
 )
 .unwrap();

 // 2 citations of Round 1, none of Round 5 (filter excludes).
 assert_eq!(v.len(), 2);
 assert!(v.iter().all(|x| x.kind == ViolationKind::Decay));
 assert!(v.iter().all(|x| x.citation.entry_id == "Round 1"));
 }

 #[test]
 fn scan_paths_filter_none_reports_only_missing() {
 // When filter_id is None, only citations missing from the valid
 // set are reported (Missing kind).
 let tmp = tempfile::tempdir().unwrap();
 let src = tmp.path().join("src");
 std::fs::create_dir_all(&src).unwrap();
 std::fs::write(src.join("a.rs"), "// Round 1\n// Round 999\n").unwrap();

 let mut valid = BTreeSet::new();
 valid.insert("Round 1".to_string());

 let v = scan_paths_filtered(
 tmp.path(),
 &["src/".to_string()],
 "Round ",
 &valid,
 None,
 )
 .unwrap();
 assert_eq!(v.len(), 1);
 assert_eq!(v[0].citation.entry_id, "Round 999");
 assert_eq!(v[0].kind, ViolationKind::Missing);
 }
}
