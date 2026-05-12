//! Spec binding: §code-citation-defense, §code-citation-defense/bidirectional-binding.
//!
//! code citation verification (Stage 2 of the 3-stage
//! code-citation defense — introduced the agent-time CLAUDE.md
//! rule, this module backs the validator-time `validate-code-refs`
//! subcommand, + 258 wire pre-commit / cascade triggers).
//!
//! extends the scanner with the spec ↔ code bidirectional
//! binding check (Path B substrate from 's
//! `AtomicSection.implementations`). The scanner now also extracts
//! `§<id>` citations and applies set-equality against each section's
//! `implementations` set (OPTION D pattern lifted from the
//! cross-ref orphan ledger).
//!
//! ## Pattern derivation
//!
//! `Round NNN`-shaped citations use the configured `entry_id_prefix`
//!:
//!
//! ```text
//! \b<prefix><digits>(\.<digits>)?\b
//! ```
//!
//! `§<id>`-shaped citations use a fixed `§` sigil + opaque token shape
//! `[A-Za-z0-9._/-]+` (covers numeric ids ``, fractional ``,
//! kebab + slash slugs `§atomic-store/changelog-atomic-ledger`):
//!
//! ```text
//! §[A-Za-z0-9._/-]+ (trailing `.` not consumed)
//! ```
//!
//! Word-boundary discipline excludes identifier-like incidental hits.
//!
//! ## Violation taxonomy
//!
//! `Round NNN` axis (existing — /258):
//! - `Missing` — entry_id not in `changelog_entries`
//! - `Decay` — `--filter-id` cascade scan match
//!
//! `§<id>` axis:
//! - `SectionMissing` — §<id> not in `atomic_section_id_set`
//! - `CitationUnbound` — §<id> exists but citing file F not in
//! §<id>.`implementations` (code-side; spec doesn't agree)
//! - `ImplementationUnbacked` — (file F, sym?) in
//! §<id>.`implementations` but F has no §<id> citation (spec-side;
//! code doesn't agree)
//!
//! The first two binding directions are *asymmetric in shape*: code-side
//! violations have a concrete (file, line, entry_id); spec-side
//! violations have no line and carry the impl-entry symbol. This is
//! modeled as a 2-variant `CodeRefViolation` enum rather than collapsing
//! both directions into one struct with a `line: 0` sentinel — the
//! shape difference is a domain fact, not an encoding accident.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::atomic::AtomicStore;
use crate::config::{OrphanKind, OrphanLedgerEntry};

/// One `Round NNN` / `§<id>` citation candidate extracted from a source
/// file. `entry_id` retains the cite shape verbatim (`""` or
/// `""` — `§` prefix kept so the kind axis is readable from the id
/// alone).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Citation {
 pub file: PathBuf,
 pub line: usize,
 pub entry_id: String,
}

/// One verification failure surfaced to the caller.
///
/// Two variants — code-side citations (`Citation`) and spec-side claims
/// (`ImplementationUnbacked`) have structurally different evidence
/// (a concrete file:line vs an impl-entry without a code witness), so
/// the enum splits at that natural boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodeRefViolation {
 /// Citation-side violation — there is a concrete cite at file:line,
 /// and the cite is wrong in some way (`kind` distinguishes how).
 Citation {
 citation: Citation,
 kind: ViolationKind,
 },
 /// Spec-side violation — the atomic store records
 /// `§section_id.implementations` containing (file, symbol?), but the
 /// file has no `§section_id` citation. The spec claims an
 /// implementation that the code does not witness.
 ImplementationUnbacked {
 section_id: String,
 file: PathBuf,
 symbol: Option<String>,
 },
}

impl CodeRefViolation {
 /// Stable kind tag for JSON output / CLI rendering. Citation
 /// violations carry their `ViolationKind` tag; `ImplementationUnbacked`
 /// is its own top-level kind.
 pub fn kind_tag(&self) -> &'static str {
 match self {
 CodeRefViolation::Citation { kind, .. } => match kind {
 ViolationKind::Missing => "missing",
 ViolationKind::Decay => "decay",
 ViolationKind::SectionMissing => "section_missing",
 ViolationKind::CitationUnbound => "citation_unbound",
 },
 CodeRefViolation::ImplementationUnbacked { .. } => "impl_unbacked",
 }
 }

 /// Defect class — drives `--severity-missing` vs
 /// `--severity-binding` bucketing. Hallucination-class = cited
 /// identifier doesn't exist (Missing, SectionMissing). Binding-class
 /// = set-equality violation (CitationUnbound, ImplementationUnbacked).
 /// Decay is its own informational class — never reject-bucketed.
 pub fn defect_class(&self) -> DefectClass {
 match self {
 CodeRefViolation::Citation { kind, .. } => match kind {
 ViolationKind::Missing | ViolationKind::SectionMissing => {
 DefectClass::Hallucination
 }
 ViolationKind::CitationUnbound => DefectClass::Binding,
 ViolationKind::Decay => DefectClass::Decay,
 },
 CodeRefViolation::ImplementationUnbacked { .. } => DefectClass::Binding,
 }
 }
}

/// semantic axis that drives CLI severity flag bucketing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefectClass {
 /// Cited identifier doesn't exist (Missing, SectionMissing).
 Hallucination,
 /// Set-equality violation (CitationUnbound, ImplementationUnbacked).
 Binding,
 /// Cascade scan informational surface (Decay).
 Decay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViolationKind {
 /// `entry_id` not in the atomic store `changelog_entries` map
 /// (hallucinated or refers to a removed entry).
 Missing,
 /// citation matches an explicit decay filter (e.g. an
 /// entry_id the cascade caller knows just transitioned to Superseded).
 /// Surfaced regardless of whether the id is still in the valid set —
 /// the entry exists, but author should review whether the code is
 /// still accurate against the new decision.
 Decay,
 /// `§<id>` citation where `<id>` is not in the atomic
 /// store's section_id set (analog of `Missing` on the section axis).
 SectionMissing,
 /// `§<id>` citation where `<id>` exists in the atomic
 /// store but the citing file is not registered in
 /// `§<id>.implementations`. The code-side half of the bidirectional
 /// set-equality violation (spec disagrees with code).
 CitationUnbound,
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

/// extract every `§<id>` citation candidate from `content`.
///
/// Token shape: `§` followed by 1+ chars from `[A-Za-z0-9._/-]`. Tail
/// trailing `.` is not consumed (mirrors `scan_round_number` so `.` at
/// end of sentence yields `39`, not `39.`). Returned entries use the bare
/// id (no `§` prefix) so callers can directly index `AtomicSection` keys.
/// Line numbers are 1-indexed.
///
/// `§` is itself a non-ASCII / non-identifier character, so prefix-side
/// word-boundary is implicit. Tail-side boundary: id terminates on any
/// char outside the token shape.
pub fn extract_section_citations(content: &str) -> Vec<(usize, String)> {
 let mut out = Vec::new();
 for (line_idx, line) in content.lines().enumerate() {
 // — single-line backtick state. `` inside a code-span
 // is documentation example, not a citation. Toggled on each backtick
 // and reset at line end (multi-line fenced code spans are not
 // recognized in v1; the comment-only stripper already gates this for
 // most source files, and inline backtick spans cover the doc-comment
 // example case that survives stripping).
 let mut in_backtick = false;
 let mut chars = line.char_indices().peekable();
 while let Some((i, c)) = chars.next() {
 if c == '`' {
 in_backtick = !in_backtick;
 continue;
 }
 if in_backtick {
 continue;
 }
 if c != '§' {
 continue;
 }
 // Tail: read [A-Za-z0-9._/-]+ starting at the byte after `§`.
 // `.` is constrained to digit-digit boundaries so
 // `.implementations` parses as `39` (the prose-style field
 // reference suffix is not part of the section_id) while
 // `` (fractional id) remains intact.
 let tail_start = i + c.len_utf8();
 let tail = &line[tail_start..];
 let tail_chars: Vec<(usize, char)> = tail.char_indices().collect();
 let mut last_byte = 0usize;
 for (idx, &(j, t)) in tail_chars.iter().enumerate() {
 if t == '.' {
 let prev_is_digit = idx > 0 && tail_chars[idx - 1].1.is_ascii_digit();
 let next_is_digit = tail_chars
 .get(idx + 1)
 .map(|(_, c)| c.is_ascii_digit())
 .unwrap_or(false);
 if !(prev_is_digit && next_is_digit) {
  break;
 }
 last_byte = j + t.len_utf8();
 continue;
 }
 if !is_section_id_char(t) {
 break;
 }
 last_byte = j + t.len_utf8();
 }
 if last_byte == 0 {
 continue;
 }
 let mut end = last_byte;
 if tail[..end].ends_with('.') {
 end -= 1;
 }
 if end == 0 {
 continue;
 }
 let id = tail[..end].to_string();
 // skip metavariable placeholders like `§N`, `§X`,
 // `§Y` used in doc-comments to mean "any section id". A real
 // section_id is either multi-char or starts with lowercase /
 // digit; a single uppercase letter is metasyntax.
 let is_metavar = id.chars().count() == 1
 && id.chars().next().map(|c| c.is_ascii_uppercase()).unwrap_or(false);
 if !is_metavar {
 out.push((line_idx + 1, id));
 }
 // Advance the outer iterator past what we consumed.
 // (peekable / char_indices doesn't have skip-to-byte, so we
 // re-seek by consuming until we pass `tail_start + end`.)
 let consumed_until = tail_start + end;
 while let Some(&(k, _)) = chars.peek() {
 if k < consumed_until {
 chars.next();
 } else {
 break;
 }
 }
 }
 }
 out
}

fn is_section_id_char(c: char) -> bool {
 c.is_ascii_alphanumeric() || c == '.' || c == '/' || c == '-' || c == '_'
}

// ============================================================================
// Comment-only filtering.
//
// The scanner pattern-matches the entire file body, which surfaces
// string-literal fixtures (e.g. test markdown that contains "" as
// data) as false-positive citations. The comment-only layer strips
// non-comment chars to a single space so that line numbers are preserved
// 1:1 while only language-comment text reaches the citation extractor.
//
// This is a *heuristic*, not a full parser: ~95% accuracy with ~100 LOC,
// which keeps the 5-min setup promise (no AST dependency). Limitations:
// - Rust raw strings (`r"..."`, `r#"..."#`) treated as normal strings;
// - Python triple-quoted strings not recognized;
// - shell heredocs not recognized;
// - escape rules simplified (`\X` skips one char inside strings).
// These miss cases are deliberately deferred — when they bite, opt-out via
// `[code_refs] comment_only = false` restores the whole-text scan.
// ============================================================================

/// Per-language comment recognition mode. The dispatcher in
/// [`comment_syntax_for`] maps file extensions onto these variants;
/// `Unknown` extensions fall through to whole-text scan (back-compat).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentSyntax {
 /// C-family: `// line` + `/* block */` (Rust, C/C++, Go, JS/TS, Java, Kotlin, Swift, Scala).
 Slash,
 /// Hash-family: `# line` only, no block syntax (Python, shell, Ruby, TOML, YAML).
 Hash,
 /// No filtering — whole text is scanned (back-compat for unknown extensions).
 Unknown,
}

/// Map a file path's extension to the appropriate [`CommentSyntax`].
/// Case-insensitive on the extension. Files with no extension fall to
/// [`CommentSyntax::Unknown`].
pub fn comment_syntax_for(path: &Path) -> CommentSyntax {
 let ext = match path.extension().and_then(|s| s.to_str()) {
 Some(e) => e.to_ascii_lowercase(),
 None => return CommentSyntax::Unknown,
 };
 match ext.as_str() {
 "rs" | "c" | "h" | "cc" | "cpp" | "cxx" | "hpp" | "hxx" | "hh" | "go"
 | "js" | "ts" | "jsx" | "tsx" | "mjs" | "cjs" | "java" | "scala"
 | "kt" | "kts" | "swift" => CommentSyntax::Slash,
 "py" | "sh" | "bash" | "zsh" | "rb" | "toml" | "yaml" | "yml" => {
 CommentSyntax::Hash
 }
 _ => CommentSyntax::Unknown,
 }
}

/// Replace non-comment characters with spaces so citation extractors see
/// only comment text. Line breaks are preserved 1:1 so line numbers stay
/// accurate. Unknown syntax returns the input unchanged.
pub fn strip_to_comments(content: &str, syntax: CommentSyntax) -> String {
 match syntax {
 CommentSyntax::Unknown => content.to_string(),
 CommentSyntax::Slash => strip_slash(content),
 CommentSyntax::Hash => strip_hash(content),
 }
}

fn strip_slash(content: &str) -> String {
 let mut out = String::with_capacity(content.len());
 let mut in_block = false;
 for (line_idx, line) in content.lines().enumerate() {
 if line_idx > 0 {
 out.push('\n');
 }
 let mut in_string = false;
 let mut chars = line.char_indices().peekable();
 while let Some((_, c)) = chars.next() {
 if in_block {
 if c == '*' && chars.peek().map(|(_, n)| *n) == Some('/') {
 out.push('*');
 chars.next();
 out.push('/');
 in_block = false;
 } else {
 out.push(c);
 }
 continue;
 }
 if in_string {
 if c == '\\' {
 out.push(' ');
 if chars.next().is_some() {
 out.push(' ');
 }
 continue;
 }
 if c == '"' {
 in_string = false;
 }
 out.push(' ');
 continue;
 }
 // Code state — look for comment openers.
 if c == '/' && chars.peek().map(|(_, n)| *n) == Some('/') {
 out.push('/');
 chars.next();
 out.push('/');
 while let Some((_, rest)) = chars.next() {
 out.push(rest);
 }
 break;
 }
 if c == '/' && chars.peek().map(|(_, n)| *n) == Some('*') {
 out.push('/');
 chars.next();
 out.push('*');
 in_block = true;
 continue;
 }
 if c == '"' {
 in_string = true;
 out.push(' ');
 continue;
 }
 out.push(' ');
 }
 // EOL — single-line strings auto-close (we don't carry in_string
 // across lines; multi-line raw strings are an accepted miss case).
 }
 out
}

fn strip_hash(content: &str) -> String {
 let mut out = String::with_capacity(content.len());
 for (line_idx, line) in content.lines().enumerate() {
 if line_idx > 0 {
 out.push('\n');
 }
 let mut in_single = false;
 let mut in_double = false;
 let mut chars = line.char_indices().peekable();
 while let Some((_, c)) = chars.next() {
 if in_single || in_double {
 if c == '\\' {
 out.push(' ');
 if chars.next().is_some() {
 out.push(' ');
 }
 continue;
 }
 if in_single && c == '\'' {
 in_single = false;
 } else if in_double && c == '"' {
 in_double = false;
 }
 out.push(' ');
 continue;
 }
 if c == '#' {
 out.push('#');
 while let Some((_, rest)) = chars.next() {
 out.push(rest);
 }
 break;
 }
 if c == '"' {
 in_double = true;
 out.push(' ');
 continue;
 }
 if c == '\'' {
 in_single = true;
 out.push(' ');
 continue;
 }
 out.push(' ');
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

/// entry_id-only scan (legacy thin wrapper, retained for
/// backward compatibility with callers that don't carry the full atomic
/// store). New callers should use [`scan_paths_bidirectional`].
///
/// defaults `comment_only=false` to preserve the Round
/// 256/258 whole-text scan semantics for any external caller still bound
/// to this entrypoint.
pub fn scan_paths(
 workspace_root: &Path,
 paths: &[String],
 prefix: &str,
 valid_entry_ids: &BTreeSet<String>,
) -> std::io::Result<Vec<CodeRefViolation>> {
 scan_paths_filtered(workspace_root, paths, prefix, valid_entry_ids, None, false)
}

/// entry_id-only scan with optional decay filter (legacy
/// thin wrapper). New callers should use [`scan_paths_bidirectional`]
/// which also covers the §<id> axis and the Path B bidirectional check.
///
/// `comment_only` toggles the comment-only filtering layer.
/// When `true`, each file's content is passed through [`strip_to_comments`]
/// (with [`comment_syntax_for`] picking the per-extension mode) before
/// citation extraction; when `false`, the whole file is scanned (Round
/// 256/258 semantics).
pub fn scan_paths_filtered(
 workspace_root: &Path,
 paths: &[String],
 prefix: &str,
 valid_entry_ids: &BTreeSet<String>,
 filter_id: Option<&str>,
 comment_only: bool,
) -> std::io::Result<Vec<CodeRefViolation>> {
 let files = walk_paths(workspace_root, paths)?;
 let mut violations = Vec::new();
 for abs in files {
 let raw = match std::fs::read_to_string(&abs) {
 Ok(c) => c,
 Err(_) => continue,
 };
 let content = if comment_only {
 strip_to_comments(&raw, comment_syntax_for(&abs))
 } else {
 raw
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
 violations.push(CodeRefViolation::Citation {
 citation: Citation {
  file: rel.clone(),
  line,
  entry_id,
 },
 kind,
 });
 }
 }
 sort_violations(&mut violations);
 Ok(violations)
}

/// full Path B scan: Round NNN axis + §<id> axis +
/// bidirectional set-equality check + orphan ledger suppression for
/// `OrphanKind::CodeCitation` rows.
///
/// Algorithm (per scanned file F):
/// 1. Extract `<prefix>NNN` citations → `Missing` (or `Decay` under
/// `filter_id`) using existing /258 path.
/// 2. Extract `§<id>` citations:
/// - `<id>` not in `store.atomic_section_id_set()` → `SectionMissing`
/// - `<id>` exists but F not in `§<id>.implementations` files →
/// `CitationUnbound`
/// - else OK (record F in `cited_by[<id>]` for step 3)
/// 3. After all files scanned, walk `store.sections`. For each §X, for
/// each `Implementation { file, symbol }` in `§X.implementations`:
/// if `file` ∉ `cited_by[X]` → `ImplementationUnbacked`.
///
/// `filter_id` is the decay-scan toggle. When `Some`, only
/// Round NNN citations matching the filter are surfaced (as `Decay`);
/// all other Round NNN citations are suppressed, and the §<id> axis
/// stays silent for symmetry (a Superseded-decision cascade caller is
/// asking "where is this entry_id mentioned?", not "audit the whole
/// store" — keep the surface narrow).
///
/// `orphan_ledger` rows with `kind = CodeCitation` suppress any §<id>
/// violation matching `(from = file, to = id)`. Other kinds are
/// ignored by this scanner (they belong to the atomic-internal /
/// markdown axes).
///
/// `comment_only` toggles the comment-only filtering layer.
/// When `true`, each file's content is passed through [`strip_to_comments`]
/// (per-extension dispatch via [`comment_syntax_for`]) so the citation
/// extractor only sees comment text. Unknown extensions fall through to
/// whole-text scan regardless of the flag.
pub fn scan_paths_bidirectional(
 workspace_root: &Path,
 paths: &[String],
 prefix: &str,
 store: &AtomicStore,
 orphan_ledger: &[OrphanLedgerEntry],
 filter_id: Option<&str>,
 comment_only: bool,
) -> std::io::Result<Vec<CodeRefViolation>> {
 let valid_entry_ids: BTreeSet<String> = store.changelog_entries.keys().cloned().collect();
 let section_id_set = store.atomic_section_id_set();

 // Pre-index §X.implementations by section_id (so we can membership-check
 // (file in §X.implementations files) in O(log n) per cite, and so we
 // know the full impls universe for step 3).
 let impl_files_by_section: BTreeMap<&str, BTreeSet<&str>> = store
 .sections
 .iter()
 .map(|(sid, sec)| {
 let files: BTreeSet<&str> = sec
 .implementations
 .iter()
 .map(|i| i.file.as_str())
 .collect();
 (sid.as_str(), files)
 })
 .collect();

 // Orphan ledger lookup: (file, id) pairs explicitly registered as
 // known-stale code citations.
 let ledger_index: BTreeSet<(&str, &str)> = orphan_ledger
 .iter()
 .filter(|e| e.kind == OrphanKind::CodeCitation)
 .map(|e| (e.from.as_str(), e.to.as_str()))
 .collect();

 let files = walk_paths(workspace_root, paths)?;
 let mut violations: Vec<CodeRefViolation> = Vec::new();

 // file_path → BTreeSet<section_id> citations actually observed.
 // Drives step 3's bidirectional check.
 let mut cited_by: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

 for abs in files {
 let raw = match std::fs::read_to_string(&abs) {
 Ok(c) => c,
 Err(_) => continue,
 };
 let content = if comment_only {
 strip_to_comments(&raw, comment_syntax_for(&abs))
 } else {
 raw
 };
 let rel = abs
 .strip_prefix(workspace_root)
 .map(|p| p.to_path_buf())
 .unwrap_or(abs.clone());
 let rel_str = rel.to_string_lossy().to_string();

 // ---- Round NNN axis ----
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
 violations.push(CodeRefViolation::Citation {
 citation: Citation {
  file: rel.clone(),
  line,
  entry_id,
 },
 kind,
 });
 }

 // ---- §<id> axis ----
 // Decay-filter mode narrows the surface to Round NNN only — Path B
 // cross-check stays silent (cascade caller's question is targeted).
 if filter_id.is_some() {
 continue;
 }
 for (line, section_id) in extract_section_citations(&content) {
 // Ledger suppression — if (file, id) is explicitly registered
 // as a known-stale code citation, treat as if the binding were
 // correct (record in `cited_by` so step 3 doesn't double-fire).
 let suppressed = ledger_index.contains(&(rel_str.as_str(), section_id.as_str()));
 cited_by
 .entry(rel_str.clone())
 .or_default()
 .insert(section_id.clone());
 if suppressed {
 continue;
 }
 if !section_id_set.contains(&section_id) {
 violations.push(CodeRefViolation::Citation {
 citation: Citation {
  file: rel.clone(),
  line,
  entry_id: format!("§{}", section_id),
 },
 kind: ViolationKind::SectionMissing,
 });
 continue;
 }
 // Section exists — check spec-side membership of (file in
 // §<id>.implementations files). Note: matching is by `file`
 // string only (symbol is opaque metadata, not part of the
 // bidirectional set-equality in v1 — same fact treated
 // consistently from both directions).
 let bound = impl_files_by_section
 .get(section_id.as_str())
 .map(|files| files.contains(rel_str.as_str()))
 .unwrap_or(false);
 if !bound {
 violations.push(CodeRefViolation::Citation {
 citation: Citation {
  file: rel.clone(),
  line,
  entry_id: format!("§{}", section_id),
 },
 kind: ViolationKind::CitationUnbound,
 });
 }
 }
 }

 // ---- Step 3: spec-side bidirectional half ----
 // Skip under decay-filter mode.
 if filter_id.is_none() {
 for (section_id, section) in &store.sections {
 for impl_entry in &section.implementations {
 let suppressed =
 ledger_index.contains(&(impl_entry.file.as_str(), section_id.as_str()));
 if suppressed {
 continue;
 }
 let cited = cited_by
 .get(&impl_entry.file)
 .map(|set| set.contains(section_id))
 .unwrap_or(false);
 if !cited {
 violations.push(CodeRefViolation::ImplementationUnbacked {
 section_id: section_id.clone(),
 file: PathBuf::from(&impl_entry.file),
 symbol: impl_entry.symbol.clone(),
 });
 }
 }
 }
 }

 sort_violations(&mut violations);
 Ok(violations)
}

/// Deterministic ordering — Citation variants sort by (file, line, entry_id);
/// ImplementationUnbacked variants sort by (file, section_id, symbol) and
/// come after Citation variants for predictable reporting.
fn sort_violations(violations: &mut Vec<CodeRefViolation>) {
 violations.sort_by(|a, b| {
 use CodeRefViolation::*;
 match (a, b) {
 (Citation { citation: c1, .. }, Citation { citation: c2, .. }) => c1
 .file
 .cmp(&c2.file)
 .then(c1.line.cmp(&c2.line))
 .then(c1.entry_id.cmp(&c2.entry_id)),
 (Citation { .. }, ImplementationUnbacked { .. }) => std::cmp::Ordering::Less,
 (ImplementationUnbacked { .. }, Citation { .. }) => std::cmp::Ordering::Greater,
 (
 ImplementationUnbacked {
 file: f1,
 section_id: s1,
 symbol: y1,
 },
 ImplementationUnbacked {
 file: f2,
 section_id: s2,
 symbol: y2,
 },
 ) => f1.cmp(f2).then(s1.cmp(s2)).then(y1.cmp(y2)),
 }
 });
}

#[cfg(test)]
mod tests {
 use super::*;
 use crate::atomic::{add_section_implementation, AtomicStore};
 use tempfile::TempDir;

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
 let src = "TestRound254Helper\nlet round_254_helper = 1;\n";
 let out = extract_citations("Round ", src);
 assert_eq!(out, vec![]);
 }

 #[test]
 fn extract_citations_post_boundary_excludes_alphanumeric_tail() {
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

 // ============ §<id> extractor unit tests ============

 #[test]
 fn extract_section_citations_basic_numeric() {
 let src = "// §39 carry\n// also §61 for context\n";
 let out = extract_section_citations(src);
 assert_eq!(
 out,
 vec![(1, "39".to_string()), (2, "61".to_string())]
 );
 }

 #[test]
 fn extract_section_citations_fractional_id() {
 let src = "// see §61.1 for sub-section\n";
 let out = extract_section_citations(src);
 assert_eq!(out, vec![(1, "61.1".to_string())]);
 }

 #[test]
 fn extract_section_citations_slash_slug() {
 let src = "// §atomic-store/changelog-atomic-ledger anchor\n";
 let out = extract_section_citations(src);
 assert_eq!(
 out,
 vec![(1, "atomic-store/changelog-atomic-ledger".to_string())]
 );
 }

 #[test]
 fn extract_section_citations_trailing_dot_not_consumed() {
 let src = "End of sentence §39. Next line\n";
 let out = extract_section_citations(src);
 assert_eq!(out, vec![(1, "39".to_string())]);
 }

 #[test]
 fn extract_section_citations_brackets_and_parens() {
 let src = "(§39) [§61.1] {§atomic-store}\n";
 let out = extract_section_citations(src);
 assert_eq!(
 out,
 vec![
 (1, "39".to_string()),
 (1, "61.1".to_string()),
 (1, "atomic-store".to_string())
 ]
 );
 }

 #[test]
 fn extract_section_citations_solitary_sigil_no_id_skipped() {
 let src = "Just a § sigil with no id following\n";
 let out = extract_section_citations(src);
 assert!(out.is_empty());
 }

 #[test]
 fn extract_section_citations_underscore_allowed() {
 let src = "// §atomic_store snake case slug\n";
 let out = extract_section_citations(src);
 assert_eq!(out, vec![(1, "atomic_store".to_string())]);
 }

 // ============ bidirectional scan integration tests ============

 fn build_store_with_impl(
 path: &Path,
 section_id: &str,
 impl_file: &str,
 symbol: Option<&str>,
 ) -> AtomicStore {
 let mut store = AtomicStore::new();
 add_section_implementation(&mut store, path, section_id, impl_file, symbol).unwrap();
 store
 }

 #[test]
 fn bidirectional_clean_codebase_no_violations() {
 // cite in src/foo.rs +.implementations contains src/foo.rs.
 let tmp = TempDir::new().unwrap();
 let store_path = tmp.path().join(".atomic/workspace.atomic.json");
 let store = build_store_with_impl(&store_path, "39", "src/foo.rs", Some("Foo"));
 std::fs::create_dir_all(tmp.path().join("src")).unwrap();
 std::fs::write(
 tmp.path().join("src/foo.rs"),
 "// §39 — Foo binds here\nfn main() {}\n",
 )
 .unwrap();
 let v = scan_paths_bidirectional(
 tmp.path(),
 &["src/".to_string()],
 "Round ",
 &store,
 &[],
 None,
 true,
 )
 .unwrap();
 assert!(v.is_empty(), "unexpected violations: {:?}", v);
 }

 #[test]
 fn bidirectional_section_missing_when_id_not_in_store() {
 // cite but no in the store.
 let tmp = TempDir::new().unwrap();
 let store = AtomicStore::new();
 std::fs::create_dir_all(tmp.path().join("src")).unwrap();
 std::fs::write(
 tmp.path().join("src/foo.rs"),
 "// see §999 hallucinated\n",
 )
 .unwrap();
 let v = scan_paths_bidirectional(
 tmp.path(),
 &["src/".to_string()],
 "Round ",
 &store,
 &[],
 None,
 true,
 )
 .unwrap();
 assert_eq!(v.len(), 1);
 match &v[0] {
 CodeRefViolation::Citation { citation, kind } => {
 assert_eq!(*kind, ViolationKind::SectionMissing);
 assert_eq!(citation.entry_id, "§999");
 }
 other => panic!("unexpected variant: {:?}", other),
 }
 }

 #[test]
 fn bidirectional_citation_unbound_when_file_not_in_impls() {
 // exists with impl src/bar.rs, but src/foo.rs cites.
 let tmp = TempDir::new().unwrap();
 let store_path = tmp.path().join(".atomic/workspace.atomic.json");
 let store = build_store_with_impl(&store_path, "39", "src/bar.rs", None);
 std::fs::create_dir_all(tmp.path().join("src")).unwrap();
 std::fs::write(
 tmp.path().join("src/foo.rs"),
 "// §39 — unauthorized cite\n",
 )
 .unwrap();
 std::fs::write(tmp.path().join("src/bar.rs"), "// §39 — authoritative\n").unwrap();
 let v = scan_paths_bidirectional(
 tmp.path(),
 &["src/".to_string()],
 "Round ",
 &store,
 &[],
 None,
 true,
 )
 .unwrap();
 assert_eq!(v.len(), 1, "got: {:?}", v);
 match &v[0] {
 CodeRefViolation::Citation { citation, kind } => {
 assert_eq!(*kind, ViolationKind::CitationUnbound);
 assert_eq!(citation.entry_id, "§39");
 assert_eq!(citation.file.to_string_lossy(), "src/foo.rs");
 }
 other => panic!("unexpected variant: {:?}", other),
 }
 }

 #[test]
 fn bidirectional_implementation_unbacked_when_impl_file_lacks_cite() {
 //.implementations contains src/foo.rs:Foo, but src/foo.rs has
 // no citation.
 let tmp = TempDir::new().unwrap();
 let store_path = tmp.path().join(".atomic/workspace.atomic.json");
 let store = build_store_with_impl(&store_path, "39", "src/foo.rs", Some("Foo"));
 std::fs::create_dir_all(tmp.path().join("src")).unwrap();
 std::fs::write(
 tmp.path().join("src/foo.rs"),
 "// no spec citation at all\nfn foo() {}\n",
 )
 .unwrap();
 let v = scan_paths_bidirectional(
 tmp.path(),
 &["src/".to_string()],
 "Round ",
 &store,
 &[],
 None,
 true,
 )
 .unwrap();
 assert_eq!(v.len(), 1, "got: {:?}", v);
 match &v[0] {
 CodeRefViolation::ImplementationUnbacked {
 section_id,
 file,
 symbol,
 } => {
 assert_eq!(section_id, "39");
 assert_eq!(file.to_string_lossy(), "src/foo.rs");
 assert_eq!(symbol.as_deref(), Some("Foo"));
 }
 other => panic!("unexpected variant: {:?}", other),
 }
 }

 #[test]
 fn bidirectional_orphan_ledger_suppresses_citation_unbound() {
 //.implementations names src/bar.rs only; src/foo.rs cites
 // but is registered in the orphan ledger as a known-stale code
 // citation. Suppressed.
 let tmp = TempDir::new().unwrap();
 let store_path = tmp.path().join(".atomic/workspace.atomic.json");
 let store = build_store_with_impl(&store_path, "39", "src/bar.rs", None);
 std::fs::create_dir_all(tmp.path().join("src")).unwrap();
 std::fs::write(tmp.path().join("src/foo.rs"), "// §39 cite\n").unwrap();
 std::fs::write(tmp.path().join("src/bar.rs"), "// §39 cite\n").unwrap();
 let ledger = vec![OrphanLedgerEntry {
 kind: OrphanKind::CodeCitation,
 doc: "<code-citation>".to_string(),
 from: "src/foo.rs".to_string(),
 to: "39".to_string(),
 reason: "legacy carry".to_string(),
 since: "Round 260".to_string(),
 }];
 let v = scan_paths_bidirectional(
 tmp.path(),
 &["src/".to_string()],
 "Round ",
 &store,
 &ledger,
 None,
 true,
 )
 .unwrap();
 assert!(v.is_empty(), "expected suppression, got: {:?}", v);
 }

 #[test]
 fn bidirectional_orphan_ledger_suppresses_implementation_unbacked() {
 //.implementations names src/foo.rs, src/foo.rs has no cite,
 // but ledger registers (src/foo.rs, 39) as known-stale. Suppressed.
 let tmp = TempDir::new().unwrap();
 let store_path = tmp.path().join(".atomic/workspace.atomic.json");
 let store = build_store_with_impl(&store_path, "39", "src/foo.rs", None);
 std::fs::create_dir_all(tmp.path().join("src")).unwrap();
 std::fs::write(tmp.path().join("src/foo.rs"), "// no cite here\n").unwrap();
 let ledger = vec![OrphanLedgerEntry {
 kind: OrphanKind::CodeCitation,
 doc: "<code-citation>".to_string(),
 from: "src/foo.rs".to_string(),
 to: "39".to_string(),
 reason: "legacy carry".to_string(),
 since: "Round 260".to_string(),
 }];
 let v = scan_paths_bidirectional(
 tmp.path(),
 &["src/".to_string()],
 "Round ",
 &store,
 &ledger,
 None,
 true,
 )
 .unwrap();
 assert!(v.is_empty(), "expected suppression, got: {:?}", v);
 }

 #[test]
 fn bidirectional_filter_id_silences_section_axis() {
 // Decay-filter narrows surface to Round NNN only; §<id> binding
 // violations should not surface even if present.
 let tmp = TempDir::new().unwrap();
 let store = AtomicStore::new();
 std::fs::create_dir_all(tmp.path().join("src")).unwrap();
 std::fs::write(
 tmp.path().join("src/foo.rs"),
 "// §999 hallucinated\n// Round 1 cite\n",
 )
 .unwrap();
 // is in the store; is not. With filter_id=,
 // we expect to surface as Decay and to stay silent.
 let mut s2 = store.clone();
 s2.changelog_entries.insert(
 "Round 1".to_string(),
 crate::atomic::AtomicChangelogEntry::default(),
 );
 let v = scan_paths_bidirectional(
 tmp.path(),
 &["src/".to_string()],
 "Round ",
 &s2,
 &[],
 Some("Round 1"),
 true,
 )
 .unwrap();
 assert_eq!(v.len(), 1);
 match &v[0] {
 CodeRefViolation::Citation { citation, kind } => {
 assert_eq!(*kind, ViolationKind::Decay);
 assert_eq!(citation.entry_id, "Round 1");
 }
 other => panic!("unexpected variant: {:?}", other),
 }
 }

 // ============ Legacy /258 thin-wrapper tests ============

 #[test]
 fn scan_paths_filtered_decay_surfaces_filter_id_match() {
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
 true,
 )
 .unwrap();
 assert_eq!(v.len(), 2);
 for x in &v {
 match x {
 CodeRefViolation::Citation { kind, citation } => {
 assert_eq!(*kind, ViolationKind::Decay);
 assert_eq!(citation.entry_id, "Round 1");
 }
 other => panic!("unexpected variant: {:?}", other),
 }
 }
 }

 #[test]
 fn scan_paths_filter_none_reports_only_missing() {
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
 true,
 )
 .unwrap();
 assert_eq!(v.len(), 1);
 match &v[0] {
 CodeRefViolation::Citation { kind, citation } => {
 assert_eq!(*kind, ViolationKind::Missing);
 assert_eq!(citation.entry_id, "Round 999");
 }
 other => panic!("unexpected variant: {:?}", other),
 }
 }

 // ============ comment-only filtering tests ============

 #[test]
 fn comment_syntax_dispatch_by_extension() {
 use std::path::PathBuf;
 // Slash family.
 for ext in [
 "rs", "c", "h", "cc", "cpp", "hpp", "go", "js", "ts", "jsx", "tsx",
 "java", "kt", "swift",
 ] {
 let p = PathBuf::from(format!("a.{}", ext));
 assert_eq!(
 comment_syntax_for(&p),
 CommentSyntax::Slash,
 "expected Slash for .{}",
 ext
 );
 }
 // Hash family.
 for ext in ["py", "sh", "bash", "rb", "toml", "yaml", "yml"] {
 let p = PathBuf::from(format!("a.{}", ext));
 assert_eq!(
 comment_syntax_for(&p),
 CommentSyntax::Hash,
 "expected Hash for .{}",
 ext
 );
 }
 // Unknown / extensionless.
 assert_eq!(
 comment_syntax_for(&PathBuf::from("a.unknown")),
 CommentSyntax::Unknown
 );
 assert_eq!(comment_syntax_for(&PathBuf::from("a")), CommentSyntax::Unknown);
 // Case-insensitive.
 assert_eq!(
 comment_syntax_for(&PathBuf::from("a.RS")),
 CommentSyntax::Slash
 );
 }

 #[test]
 fn strip_slash_preserves_line_comment_content() {
 let src = "let x = 1; // Round 254 carry\nlet y = 2;\n";
 let out = strip_to_comments(src, CommentSyntax::Slash);
 // Comment text retained, code chars stripped to spaces.
 assert!(out.contains("// Round 254 carry"));
 assert!(!out.contains("let x = 1;"));
 assert!(!out.contains("let y = 2;"));
 // Line count preserved.
 assert_eq!(out.lines().count(), src.lines().count());
 }

 #[test]
 fn strip_slash_removes_round_inside_string_literal() {
 // `` inside string literal must NOT survive comment-only mode.
 let src = "let s = \"Round 254\";\n";
 let out = strip_to_comments(src, CommentSyntax::Slash);
 assert!(!out.contains("Round 254"));
 assert!(!out.contains("Round"));
 }

 #[test]
 fn strip_slash_block_comment_multiline() {
 let src = "let x = 1; /* Round 254\n carry */ let y = 2;\n";
 let out = strip_to_comments(src, CommentSyntax::Slash);
 assert!(out.contains("Round 254"));
 assert!(out.contains("carry"));
 // Code outside block stripped.
 assert!(!out.contains("let x = 1;"));
 assert!(!out.contains("let y = 2;"));
 }

 #[test]
 fn strip_slash_string_with_double_slash_not_treated_as_comment() {
 // The `//` inside a string is NOT a comment opener.
 let src = "let s = \"// not a comment\"; // real comment\n";
 let out = strip_to_comments(src, CommentSyntax::Slash);
 // The real comment survives.
 assert!(out.contains("// real comment"));
 // The fake one (inside string) does not.
 assert!(!out.contains("not a comment"));
 }

 #[test]
 fn strip_hash_preserves_line_comment_content() {
 let src = "x = 1 # Round 254 carry\ny = 2\n";
 let out = strip_to_comments(src, CommentSyntax::Hash);
 assert!(out.contains("# Round 254 carry"));
 assert!(!out.contains("x = 1"));
 assert_eq!(out.lines().count(), src.lines().count());
 }

 #[test]
 fn strip_hash_removes_hash_inside_string_literal() {
 // `#` inside a quoted string must NOT be treated as a comment opener.
 let src = "url = \"http://example.com/#anchor\" # real comment\n";
 let out = strip_to_comments(src, CommentSyntax::Hash);
 assert!(out.contains("# real comment"));
 // The url content stripped — `#anchor` should not survive as a hash-comment.
 assert!(!out.contains("anchor\""));
 }

 #[test]
 fn strip_unknown_is_passthrough() {
 let src = "raw text with Round 254 anywhere\n";
 let out = strip_to_comments(src, CommentSyntax::Unknown);
 assert_eq!(out, src);
 }

 #[test]
 fn bidirectional_comment_only_filters_string_literal_noise() {
 //.rs file: only the comment cite should fire; string-literal Round NNN
 // must NOT produce a Missing violation under comment_only=true.
 let tmp = TempDir::new().unwrap();
 let store = AtomicStore::new();
 std::fs::create_dir_all(tmp.path().join("src")).unwrap();
 std::fs::write(
 tmp.path().join("src/foo.rs"),
 "let fixture = \"Round 999 is fixture data\";\n// Round 999 real cite\n",
 )
 .unwrap();
 let v = scan_paths_bidirectional(
 tmp.path(),
 &["src/".to_string()],
 "Round ",
 &store,
 &[],
 None,
 true,
 )
 .unwrap();
 // Only one Missing (the line 2 comment); line 1 string literal suppressed.
 let missing: Vec<_> = v
 .iter()
 .filter(|x| matches!(
 x,
 CodeRefViolation::Citation { kind: ViolationKind::Missing, .. }
 ))
 .collect();
 assert_eq!(missing.len(), 1, "got: {:?}", v);
 if let CodeRefViolation::Citation { citation, .. } = missing[0] {
 assert_eq!(citation.line, 2, "comment is on line 2, not line 1");
 }
 }

 #[test]
 fn bidirectional_comment_only_false_legacy_back_compat() {
 // With comment_only=false, both string-literal and comment cites fire
 //.
 let tmp = TempDir::new().unwrap();
 let store = AtomicStore::new();
 std::fs::create_dir_all(tmp.path().join("src")).unwrap();
 std::fs::write(
 tmp.path().join("src/foo.rs"),
 "let fixture = \"Round 999 fixture\";\n// Round 999 cite\n",
 )
 .unwrap();
 let v = scan_paths_bidirectional(
 tmp.path(),
 &["src/".to_string()],
 "Round ",
 &store,
 &[],
 None,
 false,
 )
 .unwrap();
 // Whole-text scan picks up BOTH occurrences (line 1 and line 2).
 let missing: Vec<_> = v
 .iter()
 .filter(|x| matches!(
 x,
 CodeRefViolation::Citation { kind: ViolationKind::Missing, .. }
 ))
 .collect();
 assert_eq!(missing.len(), 2, "got: {:?}", v);
 }

 #[test]
 fn bidirectional_comment_only_unknown_extension_passthrough() {
 //.unknown extension → CommentSyntax::Unknown → whole-text scan even
 // under comment_only=true.
 let tmp = TempDir::new().unwrap();
 let store = AtomicStore::new();
 std::fs::create_dir_all(tmp.path().join("src")).unwrap();
 std::fs::write(
 tmp.path().join("src/notes.unknown"),
 "raw text Round 999 anywhere\n",
 )
 .unwrap();
 let v = scan_paths_bidirectional(
 tmp.path(),
 &["src/".to_string()],
 "Round ",
 &store,
 &[],
 None,
 true,
 )
 .unwrap();
 // Unknown extension preserves /258 whole-text behavior.
 assert_eq!(v.len(), 1, "got: {:?}", v);
 }
}
