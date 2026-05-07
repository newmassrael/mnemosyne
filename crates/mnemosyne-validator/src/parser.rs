//! markdown → typed facts parser — DESIGN §61 *Markdown variant spec* source binding.
//!
//! Single-pass mechanical parse. Round 71 OPTION H-3 carry — markdown link
//! Excludes the `[text](#anchor)` anchor href from CrossRef emit (intra-doc anchor and
//! Prevents silent orphan rejection caused by §N section_id slug-form mismatches.
//!
//! This module's scope is *single-doc* — multi-doc lookup priority step (2)
//! [`crate::workspace::Workspace::reclassify_cross_refs`] subsequent pass.
//! parser §N inline literal all default `RefKind::Decision` as emit, workspace
//! Reclassify step (2) performs the cross-doc auto-reclassify (Round 70 OPTION H-2 adoption carry).
//!
//! Round 244 — *sub_bullets extraction = legacy fallback path*. The parser's
//! `## Changelog` block in sub-bullet → [`ChangelogEntry::sub_bullets`]
//! population scope = only the legacy Round 1-162 markdown entries carry as a source; extension
//! 163+ entry atomic store ([`crate::atomic::AtomicChangelogEntry`]) in
//! once the markdown decomposition into sub_bullets is gone. MD-DELETION-RATIFY (Round 248)
//! carry = 7 source MDs deleted + parser changelog-block path unused → this extraction
//! Skip-entry path. Cascade B/C (Round 245-246) = surface scope (emitter / query
//! / mutate / validator) — once they rebase to atomic-first, this path's effectiveness drops to 0.

use crate::config::SchemaSection;
use crate::schema::{
 ChangelogEntry, CrossRef, DecisionStatus, ParsedDoc, RefKind, Section,
};
use std::collections::BTreeMap;

/// markdown bytes → typed facts state. Deterministic — same input → same ParsedDoc.
///
/// This parser is *single-doc single-pass* (DESIGN §61 *Iterative bootstrap protocol*
/// Markdown-specialization carry — Phase A/B no-op, Phase C mechanical, Phase D
/// fallback). Workspace-level cross-doc reclassify [`crate::workspace::Workspace`]
/// subsequent pass.
///
/// Round 143 SCHEMA-AS-INPUT carry: forwards to
/// [`parse_markdown_with_schema`] with the Mnemosyne preset
/// ([`SchemaSection::mnemosyne_preset`]). External users wanting a
/// different changelog title set call the schema-aware entry directly.
pub fn parse_markdown(input: &str, parent_doc: &str) -> ParsedDoc {
 let schema = SchemaSection::mnemosyne_preset();
 parse_markdown_with_schema(input, parent_doc, &schema)
}

/// Round 143 — schema-aware variant of [`parse_markdown`]. The `schema`
/// argument decides which heading titles open a `## Changelog` section
/// (and, in subsequent rounds, anchor / changelog-entry / locale rules).
///
/// External users supply their own schema via `mnemosyne.toml::[schema]`;
/// the Mnemosyne self-application uses the `design_doc` preset, which
/// preserves the existing parser semantics byte-for-byte.
pub fn parse_markdown_with_schema(
 input: &str,
 parent_doc: &str,
 schema: &SchemaSection,
) -> ParsedDoc {
 let mut out = ParsedDoc::default();
 let mut state = ParseState::new(parent_doc, schema);

 for (line_idx, raw_line) in input.lines().enumerate() {
 let line = raw_line;
 state.current_line = line_idx + 1;

 // Code fence boundary — CommonMark §98 allows 0–3 leading spaces.
 // Lines inside the fence are verbatim and not interpreted by the parser.
 let leading_ws = line.len() - line.trim_start().len();
 if leading_ws <= 3 && line.trim_start().starts_with("```") {
 state.in_code_fence = !state.in_code_fence;
 state.append_body_line(line);
 continue;
 }
 if state.in_code_fence {
 state.append_body_line(line);
 continue;
 }

 // Horizontal rule — Section boundary marker (no fact create).
 if line.trim() == "---" {
 state.flush_pending_changelog(&mut out);
 continue;
 }

 // Heading recognition.
 if let Some(heading) = parse_heading(line) {
 state.flush_pending_changelog(&mut out);
 apply_heading(&mut state, &mut out, heading);
 continue;
 }

 // Inside `## Changelog` — bullet → ChangelogEntry.
 if state.in_changelog_section() {
 if let Some(entry) = parse_changelog_top_bullet(line, &state.entry_id_prefix) {
  state.flush_pending_changelog(&mut out);
  state.pending_changelog = Some(PendingChangelog {
  entry_id: entry.entry_id,
  sub_bullets: Vec::new(),
  frozen_at_transaction_time: state.next_transaction_time(),
  });
  continue;
 }
 if let Some(sub_bullet) = parse_changelog_sub_bullet(line) {
  if let Some(pending) = &mut state.pending_changelog {
  pending.sub_bullets.push(sub_bullet);
  continue;
  }
  out.warnings.push(format!(
  "line {}: Changelog in sub-bullet parent ChangelogEntry missing — Section.body fallback",
  state.current_line
  ));
 }
 state.flush_pending_changelog(&mut out);
 }

 // CrossRef extraction — §N inline literal + [text](url) markdown link.
 // Round 249 — skip cross_ref extraction inside the changelog section.
 // Legacy DESIGN.md had bullets (consumed via `parse_changelog_top_bullet`
 // continue); GENERATED.md emits per-entry `###` sub-sections whose
 // prose body would otherwise capture textual `§X.Y` references from
 // decision_summary as authoritative cross_refs (false-positive
 // orphans). Authoritative impact is tracked via atomic store
 // `impact_refs` instead.
 if !state.in_changelog_section() {
 for cross_ref in extract_cross_refs(line, &state) {
  out.cross_refs.push(cross_ref);
 }
 }

 // Line ref legacy detection — DESIGN §61 mapping table row 13 NOT capture, warn only.
 if let Some(line_ref_warn) = detect_legacy_line_ref(line, state.current_line) {
 out.warnings.push(line_ref_warn);
 }

 // Section.body fallback (typed entity not promoted, Phase 0 carry).
 state.append_body_line(line);
 }

 state.flush_pending_changelog(&mut out);

 // Round 118 — section_body buffer → ParsedDoc.bodies (raw lines joined).
 // bench/codegen-prototype/src/markdown_import.rs in Round 116 equivalent path —
 // §15 spec query API 's SectionView.body source. derived dimension (round-trip
 // compare other) -therefore sections vec ordering and separate.
 for (section_id, lines) in state.section_body.into_iter() {
 out.bodies.insert(section_id, lines.join("\n"));
 }

 out
}

// ============================================================================
// Parse state — single-pass parser internal.
// ============================================================================

struct ParseState {
 parent_doc: String,
 /// Stack of (section_id, depth-as-`#`-count). Top = nearest ancestor.
 section_stack: Vec<(String, usize)>,
 current_line: usize,
 in_code_fence: bool,
 pending_changelog: Option<PendingChangelog>,
 transaction_time_counter: i64,
 section_body: BTreeMap<String, Vec<String>>,
 /// True iff the current section is the changelog section itself
 /// (heading title matches `changelog_titles`). Reset on every heading.
 in_changelog: bool,
 /// Round 249 — depth at which the changelog section opened. Subsequent
 /// headings at strictly greater depth stay inside the changelog scope
 /// (so their body lines are skipped from cross_ref extraction). Cleared
 /// when a heading at depth ≤ this value is encountered.
 changelog_open_depth: Option<usize>,
 /// Round 143 — owned snapshot of the schema's changelog title set.
 /// Owned (not a borrow) so `ParseState` carries no lifetime.
 changelog_titles: Vec<String>,
 /// Round 144 — string prefix that opens a ChangelogEntry top bullet
 /// (`"Round "` for Mnemosyne preset, `"ADR-"` for ADR-style, `""` to
 /// disable numeric entry_id capture).
 entry_id_prefix: String,
}

struct PendingChangelog {
 entry_id: String,
 sub_bullets: Vec<String>,
 frozen_at_transaction_time: i64,
}

impl ParseState {
 fn new(parent_doc: &str, schema: &SchemaSection) -> Self {
 Self {
 parent_doc: parent_doc.to_string(),
 section_stack: Vec::new(),
 current_line: 0,
 in_code_fence: false,
 pending_changelog: None,
 transaction_time_counter: 0,
 section_body: BTreeMap::new(),
 in_changelog: false,
 changelog_open_depth: None,
 changelog_titles: schema.changelog_titles.clone(),
 entry_id_prefix: schema.entry_id_prefix.clone(),
 }
 }

 /// Round 143 — case-sensitive match against the schema's configured
 /// changelog title set, with case-insensitive `changelog` carry.
 ///
 /// Round 249 — prefix match (split on first whitespace) so that GENERATED.md
 /// emit form `Changelog (atomic ledger)` matches schema title `Changelog`.
 /// Legacy DESIGN.md form `Changelog` matches via exact equality (prefix
 /// is the whole string). The check is intentionally non-greedy: if the
 /// title starts with a configured changelog title and the next char is
 /// either end-of-string or whitespace, it counts.
 fn is_changelog_title(&self, title: &str) -> bool {
 let starts_with_title = self.changelog_titles.iter().any(|c| {
 title == c
  || (title.starts_with(c.as_str())
  && title[c.len()..]
  .chars()
  .next()
  .map(|ch| ch.is_whitespace())
  .unwrap_or(false))
 });
 starts_with_title || title.eq_ignore_ascii_case("changelog")
 }

 fn next_transaction_time(&mut self) -> i64 {
 self.transaction_time_counter += 1;
 self.transaction_time_counter
 }

 fn current_section_id(&self) -> Option<&str> {
 self.section_stack.last().map(|(id, _)| id.as_str())
 }

 fn in_changelog_section(&self) -> bool {
 // Round 249 — true if the current section heading is itself a
 // changelog title OR if any ancestor still inside the open
 // changelog scope (depth-aware: GENERATED.md emits per-entry `###`
 // sub-sections whose body would otherwise leak textual `§X.Y`
 // references as authoritative cross_refs).
 self.in_changelog || self.changelog_open_depth.is_some()
 }

 fn append_body_line(&mut self, line: &str) {
 if let Some(id) = self.current_section_id() {
 self.section_body
  .entry(id.to_string())
  .or_default()
  .push(line.to_string());
 }
 }

 fn flush_pending_changelog(&mut self, out: &mut ParsedDoc) {
 if let Some(pending) = self.pending_changelog.take() {
 out.changelog_entries.push(ChangelogEntry {
  entry_id: pending.entry_id,
  parent_changelog_entry: None,
  sub_bullets: pending.sub_bullets,
  frozen_at_transaction_time: pending.frozen_at_transaction_time,
 });
 }
 }
}

// ============================================================================
// Heading parser — DESIGN §61 mapping table row 1-4.
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedHeading {
 depth: usize,
 title: String,
 section_number: Option<String>,
}

fn parse_heading(line: &str) -> Option<ParsedHeading> {
 // ATX heading per CommonMark §62: 0–3 leading spaces, then 1–6 `#`,
 // then a space/tab or end-of-line. `#1` (hash directly followed by a
 // non-space) is *not* a heading; treating it as one creates spurious
 // sections from prose like "Phase 0 #1 and softened..." (round-trip
 // break — false-positive H1 from inline-`#N` references).
 let leading_ws = line.len() - line.trim_start().len();
 if leading_ws > 3 {
 return None;
 }
 let trimmed = line.trim_start();
 if !trimmed.starts_with('#') {
 return None;
 }
 let depth = trimmed.bytes().take_while(|&b| b == b'#').count();
 if depth == 0 || depth > 6 {
 return None;
 }
 let after_hashes_raw = &trimmed[depth..];
 if !after_hashes_raw.is_empty()
 && !after_hashes_raw.starts_with(' ')
 && !after_hashes_raw.starts_with('\t')
 {
 return None;
 }
 let after_hashes = after_hashes_raw.trim_start();
 if after_hashes.is_empty() {
 return None;
 }
 let (section_number, title) = split_section_number(after_hashes);
 Some(ParsedHeading {
 depth,
 title: title.trim().to_string(),
 section_number,
 })
}

/// `39. Graph schema codegen` → (Some("39"), "Graph schema codegen").
/// Also accepts a leading `§` prefix: `§1 Framing overview` → (Some("1"), "Framing overview").
fn split_section_number(s: &str) -> (Option<String>, String) {
 let scan_from = s.strip_prefix('§').unwrap_or(s);
 let bytes = scan_from.as_bytes();
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
 let mut number_str = &scan_from[..num_end];
 if number_str.ends_with('.') {
 num_end -= 1;
 number_str = &scan_from[..num_end];
 }
 let rest = &scan_from[idx..];
 let rest = rest.strip_prefix('.').unwrap_or(rest);
 let rest = rest.trim_start();
 if number_str.is_empty() {
 return (None, s.to_string());
 }
 (Some(number_str.to_string()), rest.to_string())
}

fn apply_heading(state: &mut ParseState, out: &mut ParsedDoc, heading: ParsedHeading) {
 while let Some(&(_, top_depth)) = state.section_stack.last() {
 if top_depth >= heading.depth {
 state.section_stack.pop();
 } else {
 break;
 }
 }
 let parent_section = state.section_stack.last().map(|(id, _)| id.clone());

 // section_id decision (Round 67 fix carry):
 // - depth 2 numbered (top-level decision section §N): bare §N
 // - depth >= 3 numbered (nested ### N. ...): `{parent}/{N}` prefix
 // - h1 doc-root unnumbered (parent = None): bare slug
 // - depth >= 2 unnumbered nested: `{parent}/{slug}` prefix
 let section_id = match (&heading.section_number, heading.depth, &parent_section) {
 (Some(n), 2, _) => n.clone(),
 (Some(n), _, Some(pid)) => format!("{}/{}", pid, n),
 (Some(n), _, None) => n.clone(),
 (None, _, None) => slug_for_unnumbered(&heading.title),
 (None, _, Some(pid)) => format!("{}/{}", pid, slug_for_unnumbered(&heading.title)),
 };

 state
 .section_stack
 .push((section_id.clone(), heading.depth));

 state.in_changelog = state.is_changelog_title(&heading.title);
 // Round 249 — open / close the changelog scope so descendants stay
 // inside it (cross_ref extraction skipped) until a heading at depth
 // ≤ open_depth is encountered.
 if state.in_changelog {
 state.changelog_open_depth = Some(heading.depth);
 } else if let Some(open_depth) = state.changelog_open_depth {
 if heading.depth <= open_depth {
 state.changelog_open_depth = None;
 }
 }

 // Round 118 — heading line anchor (1-indexed) capture. same section_id -
 // If a section_id appears twice, first-write-wins (cycle warn; line_anchors keeps the first).
 out.line_anchors
 .entry(section_id.clone())
 .or_insert(state.current_line);

 out.sections.push(Section {
 section_id,
 parent_doc: state.parent_doc.clone(),
 parent_section,
 title: heading.title,
 decision_status: DecisionStatus::Active,
 });
}

fn slug_for_unnumbered(title: &str) -> String {
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
 } else if is_cjk(ch) {
 buf.push(ch);
 }
 }
 if buf.is_empty() {
 return "section".to_string();
 }
 buf
}

fn is_cjk(ch: char) -> bool {
 matches!(ch as u32,
 0x3040..=0x30FF
 | 0x3400..=0x4DBF
 | 0x4E00..=0x9FFF
 | 0xAC00..=0xD7AF
 )
}

// Round 249 — legacy free-function `is_changelog_title` removed; production
// + tests now route through `ParseState::is_changelog_title` (schema-aware
// prefix match) and `SchemaSection::is_changelog_title` (config-aware).

// ============================================================================
// ChangelogEntry parser — DESIGN §61 mapping table row 5-6.
// ============================================================================

struct TopBullet {
 entry_id: String,
}

fn parse_changelog_top_bullet(line: &str, prefix: &str) -> Option<TopBullet> {
 if !line.starts_with("- ") {
 return None;
 }
 let body = &line[2..];
 let entry_id = extract_entry_id_with_prefix(body, prefix)?;
 Some(TopBullet { entry_id })
}

/// Round 144 — schema-driven entry_id extraction. The schema's
/// `entry_id_prefix` (e.g., `"Round "`, `"ADR-"`, `"Round "`) opens an entry
/// bullet; the digits + dot-separator chain that follow produce the
/// numeric portion. Returns the full id including the prefix
/// (`"Round 33.5"`, `"ADR-0042"`).
///
/// Empty `prefix` disables capture (returns `None` for any input) — the
/// generic_default preset uses this when the user has not declared an
/// entry pattern.
fn extract_entry_id_with_prefix(s: &str, prefix: &str) -> Option<String> {
 if prefix.is_empty() {
 return None;
 }
 let trimmed = s.trim_start();
 if !trimmed.starts_with(prefix) {
 return None;
 }
 let after = &trimmed[prefix.len()..];
 let mut idx = 0usize;
 let bytes = after.as_bytes();
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
 return None;
 }
 Some(format!("{}{}", prefix, &after[..idx]))
}

/// Legacy variant — retained for tests and call sites that have not yet
/// adopted the schema-driven path. Production runtime path uses
/// `extract_entry_id_with_prefix(.., schema.entry_id_prefix)`.
#[cfg(test)]
fn extract_entry_id(s: &str) -> Option<String> {
 extract_entry_id_with_prefix(s, "Round ")
}

fn parse_changelog_sub_bullet(line: &str) -> Option<String> {
 if !line.starts_with("  - ") {
 return None;
 }
 Some(line[4..].to_string())
}

// ============================================================================
// CrossRef extraction — DESIGN §61 mapping table row 12 (lookup priority step (1) default).
// ============================================================================

/// Extract CrossRef from a single line within enclosing section.
///
/// This stage = lookup priority step (1) only — emits `RefKind::Decision` by default.
/// step (2) workspace default-doc auto-reclassify [`crate::workspace::Workspace`]
/// Subsequent pass (Round 70 OPTION H-2 adoption carry).
fn extract_cross_refs(line: &str, state: &ParseState) -> Vec<CrossRef> {
 let mut out = Vec::new();
 let from_section = match state.current_section_id() {
 Some(id) => id.to_string(),
 None => return out,
 };

 // §N / §N.M inline literal scan.
 let bytes = line.as_bytes();
 let mut i = 0usize;
 while i < bytes.len() {
 if let Some(rest) = line.get(i..) {
 if let Some(stripped) = rest.strip_prefix('§') {
  let consumed_section_marker = '§'.len_utf8();
  let mut j = 0usize;
  let stripped_bytes = stripped.as_bytes();
  let mut saw_digit = false;
  while j < stripped_bytes.len() {
  let b = stripped_bytes[j];
  if b.is_ascii_digit() || (b == b'.' && saw_digit) {
  if b.is_ascii_digit() {
   saw_digit = true;
  }
  j += 1;
  } else {
  break;
  }
  }
  if saw_digit {
  let mut num_end = j;
  let raw = &stripped[..num_end];
  if raw.ends_with('.') {
  num_end -= 1;
  }
  let target = stripped[..num_end].to_string();
  out.push(CrossRef {
  from_section: from_section.clone(),
  to_target: target,
  ref_kind: RefKind::Decision,
  created_at_changelog_entry: None,
  });
  i += consumed_section_marker + j;
  continue;
  }
 }
 }
 let step = match line[i..].chars().next() {
 Some(c) => c.len_utf8(),
 None => 1,
 };
 i += step;
 }

 // Markdown link `[text](url)` scan.
 let mut idx = 0usize;
 while let Some(open) = line[idx..].find('[') {
 let abs_open = idx + open;
 let close_text = match line[abs_open..].find(']') {
 Some(c) => abs_open + c,
 None => break,
 };
 let after_close = close_text + 1;
 if line[after_close..].starts_with('(') {
 if let Some(close_paren) = line[after_close..].find(')') {
  let url = &line[after_close + 1..after_close + close_paren];
  // Intra-doc anchor href `#anchor` CrossRef emit out of scope — slug
  // form mismatch silent orphan reject prevent (Round 71 OPTION H-3 carry).
  if url.starts_with('#') {
  idx = after_close + close_paren + 1;
  continue;
  }
  // Directory ref `[text](dir/)` — DESIGN §61 row directory_ref policy
  // (Round 78 ratify, Round 80 production migration). Filesystem path
  // notice marker, not a design fact — dropped silently from cross_refs.
  if is_directory_ref(url) {
  idx = after_close + close_paren + 1;
  continue;
  }
  let kind = if url.contains(".md") {
  RefKind::CrossDoc
  } else {
  RefKind::Impl
  };
  out.push(CrossRef {
  from_section: from_section.clone(),
  to_target: url.to_string(),
  ref_kind: kind,
  created_at_changelog_entry: None,
  });
  idx = after_close + close_paren + 1;
  continue;
 }
 }
 idx = after_close;
 }

 out
}

/// Markdown link target directory path recognized check — DESIGN §61 Round 78 ratify carry.
///
/// rule: a trailing `/` marks a directory (`bench/`, `crates/`, `path/to/dir/`).
/// all relevant. `.md` file path directory not (separate cross-doc kind).
fn is_directory_ref(url: &str) -> bool {
 !url.is_empty() && url.ends_with('/')
}

fn detect_legacy_line_ref(line: &str, line_no: usize) -> Option<String> {
 let needle = "(line ";
 if let Some(start) = line.find(needle) {
 let after = &line[start + needle.len()..];
 if after.chars().next().map_or(false, |c| c.is_ascii_digit()) {
 return Some(format!(
  "line {}: legacy `(line N)` ref detected — NOT captured (DESIGN §61 carry, line ref derived on export pass)",
  line_no
 ));
 }
 }
 None
}

// ============================================================================
// Small fixture — bench prototype carry, production reference.
// ============================================================================

/// Small fixture markdown string covering DESIGN §61 mapping rule row 13's subset.
pub fn design_doc_small_fixture() -> &'static str {
 r#"# Mnemosyne Design Decisions

this doc is §66 self-application's first dogfood medium.

## 39. Graph schema codegen

§39 graph_schema kind 's source. §41 datalog_rule and [ARCHITECTURE](architecture.md#l1-narrative-data-model) this §39 's downstream.

### Phase 0 design_doc schema closed-form registered

design_doc schema 's 4 entity/relation full shape — Section / ChangelogEntry / FrozenList / CrossRef. this registered §66 prerequisite #3 's §39 closed-form registered carry source. (line 2929)

```rust
pub struct Section {
 pub section_id: String,
 pub title: String,
}
```

---

## 61. Import adapter framework

§56 export adapter 's *symmetric*. §61 body = markdown ↔ typed facts transform rule source of truth.

## Changelog

- Round 60 (OPTION G — §39 closed-form registered round):
  - §39 *Phase 0 design_doc schema closed-form registered* subsection created
  - §66 prerequisite #3 line update
- Round 61 (OPTION E-1 — §61/§56 body framing update round):
  - §61 / §56 intro paragraph schema source of truth 3 layer separation
  - §61 / §56 *Phase 0 implementation entry prerequisite framing* paragraph created
"#
}

// ============================================================================
// Tests — small fixture parse consistency validation (bench prototype 18 unit test carry).
// ============================================================================

#[cfg(test)]
mod tests {
 use super::*;
 use crate::schema::{parsed_doc_canonical, sha256_hex};

 #[test]
 fn fixture_is_non_empty() {
 let fixture = design_doc_small_fixture();
 assert!(!fixture.is_empty());
 assert!(fixture.contains("# Mnemosyne Design Decisions"));
 assert!(fixture.contains("## 39. Graph schema codegen"));
 assert!(fixture.contains("## Changelog"));
 assert!(fixture.contains("- Round 60"));
 }

 #[test]
 fn parse_h1_doc_root() {
 let doc = parse_markdown(design_doc_small_fixture(), "DESIGN.md");
 let h1 = doc
 .sections
 .iter()
 .find(|s| s.title == "Mnemosyne Design Decisions");
 assert!(h1.is_some());
 let h1 = h1.unwrap();
 assert_eq!(h1.parent_section, None);
 assert_eq!(h1.decision_status, DecisionStatus::Active);
 }

 #[test]
 fn parse_numbered_top_level_section() {
 let doc = parse_markdown(design_doc_small_fixture(), "DESIGN.md");
 let s39 = doc.sections.iter().find(|s| s.section_id == "39");
 assert!(s39.is_some());
 let s39 = s39.unwrap();
 assert_eq!(s39.title, "Graph schema codegen");
 assert!(s39.parent_section.is_some());
 }

 #[test]
 fn parse_unnumbered_section_changelog() {
 let doc = parse_markdown(design_doc_small_fixture(), "DESIGN.md");
 let cl = doc.sections.iter().find(|s| s.title == "Changelog");
 assert!(cl.is_some());
 let cl = cl.unwrap();
 assert_eq!(cl.section_id, "mnemosyne-design-decisions/changelog");
 assert_eq!(
 cl.parent_section.as_deref(),
 Some("mnemosyne-design-decisions")
 );
 }

 #[test]
 fn parse_nested_heading_parent_chain() {
 let doc = parse_markdown(design_doc_small_fixture(), "DESIGN.md");
 let nested = doc
 .sections
 .iter()
 .find(|s| s.title == "Phase 0 design_doc schema closed-form registered");
 assert!(nested.is_some());
 assert_eq!(nested.unwrap().parent_section.as_deref(), Some("39"));
 }

 #[test]
 fn parse_changelog_entries() {
 let doc = parse_markdown(design_doc_small_fixture(), "DESIGN.md");
 assert_eq!(doc.changelog_entries.len(), 2);
 let e60 = &doc.changelog_entries[0];
 let e61 = &doc.changelog_entries[1];
 assert_eq!(e60.entry_id, "Round 60");
 assert_eq!(e61.entry_id, "Round 61");
 assert!(!e60.sub_bullets.is_empty());
 assert!(!e61.sub_bullets.is_empty());
 assert!(e60.frozen_at_transaction_time < e61.frozen_at_transaction_time);
 }

 #[test]
 fn parse_changelog_sub_bullets_ordered() {
 let doc = parse_markdown(design_doc_small_fixture(), "DESIGN.md");
 let e60 = &doc.changelog_entries[0];
 assert_eq!(e60.sub_bullets.len(), 2);
 assert!(e60.sub_bullets[0].contains("§39"));
 assert!(e60.sub_bullets[1].contains("§66 prerequisite #3"));
 }

 #[test]
 fn parse_inline_section_cross_ref() {
 let doc = parse_markdown(design_doc_small_fixture(), "DESIGN.md");
 let s39_refs: Vec<&CrossRef> = doc
 .cross_refs
 .iter()
 .filter(|c| c.from_section == "39" && c.ref_kind == RefKind::Decision)
 .collect();
 assert!(s39_refs.iter().any(|c| c.to_target == "39"));
 assert!(s39_refs.iter().any(|c| c.to_target == "41"));
 }

 #[test]
 fn parse_cross_doc_link() {
 let doc = parse_markdown(design_doc_small_fixture(), "DESIGN.md");
 let cross_doc: Vec<&CrossRef> = doc
 .cross_refs
 .iter()
 .filter(|c| c.ref_kind == RefKind::CrossDoc)
 .collect();
 assert!(!cross_doc.is_empty());
 assert!(cross_doc
 .iter()
 .any(|c| c.to_target.contains("architecture.md")));
 }

 #[test]
 fn parse_legacy_line_ref_warned_not_captured() {
 let doc = parse_markdown(design_doc_small_fixture(), "DESIGN.md");
 let warned = doc
 .warnings
 .iter()
 .any(|w| w.contains("legacy") && w.contains("line"));
 assert!(warned);
 }

 #[test]
 fn parse_code_fence_does_not_emit_cross_ref() {
 let input = "## 39. Test\n\n```rust\n§7 inside code\n```\n";
 let doc = parse_markdown(input, "DESIGN.md");
 let has_section_7 = doc.cross_refs.iter().any(|c| c.to_target == "7");
 assert!(!has_section_7);
 }

 #[test]
 fn parse_horizontal_rule_does_not_emit_section() {
 let input = "## 39. Test\n\n---\n\n## 61. After\n";
 let doc = parse_markdown(input, "DESIGN.md");
 assert_eq!(doc.sections.len(), 2);
 assert_eq!(doc.sections[0].section_id, "39");
 assert_eq!(doc.sections[1].section_id, "61");
 }

 #[test]
 fn parse_determinism() {
 let a = parse_markdown(design_doc_small_fixture(), "DESIGN.md");
 let b = parse_markdown(design_doc_small_fixture(), "DESIGN.md");
 assert_eq!(a, b);
 assert_eq!(parsed_doc_canonical(&a), parsed_doc_canonical(&b));
 }

 #[test]
 fn parse_canonical_render_sha256_stable() {
 let doc = parse_markdown(design_doc_small_fixture(), "DESIGN.md");
 let canonical = parsed_doc_canonical(&doc);
 let h1 = sha256_hex(&canonical);
 let doc2 = parse_markdown(design_doc_small_fixture(), "DESIGN.md");
 let h2 = sha256_hex(&parsed_doc_canonical(&doc2));
 assert_eq!(h1, h2);
 assert_eq!(h1.len(), 64);
 }

 #[test]
 fn parse_populates_bodies_for_known_section() {
 // Round 118 — production import wire in §15 spec query API's
 // SectionView.body source validation.
 let doc = parse_markdown(design_doc_small_fixture(), "DESIGN.md");
 let body_39 = doc
 .bodies
 .get("39")
 .expect("§39 body must be populated");
 assert!(
 body_39.contains("graph_schema") || body_39.contains("§39"),
 "§39 body must contain inline text from §39 section"
 );
 }

 #[test]
 fn parse_populates_line_anchors_one_indexed() {
 // Round 118 — production import wire in §15 spec query API's
 // SectionView.line_anchor source validation (1-indexed).
 let doc = parse_markdown(design_doc_small_fixture(), "DESIGN.md");
 let anchor_39 = doc
 .line_anchors
 .get("39")
 .copied()
 .expect("§39 line_anchor must be populated");
 assert!(
 anchor_39 > 0,
 "line_anchor must be 1-indexed (got {})",
 anchor_39
 );
 // ## 39 heading sits at line ≥ 5 (after the h1 + intro paragraph).
 assert!(anchor_39 >= 5, "§39 heading at line >= 5 (got {})", anchor_39);
 }

 #[test]
 fn parse_bodies_excluded_from_canonical_hash() {
 // Round 118 — bodies / line_anchors two derived field -
 // parsed_doc_canonical output out of scope (round-trip diff = ∅ validation
 // cardinality identical maintain). bench (Round 116) equivalent path.
 let doc = parse_markdown(design_doc_small_fixture(), "DESIGN.md");
 let canonical = parsed_doc_canonical(&doc);
 // canonical in §39 body content unregistered (derived dimension carry).
 assert!(
 !canonical.contains("=== bodies ==="),
 "canonical must not include bodies section"
 );
 assert!(
 !canonical.contains("=== line_anchors ==="),
 "canonical must not include line_anchors section"
 );
 }

 #[test]
 fn parse_section_id_slug_strips_punctuation() {
 let s = slug_for_unnumbered("60. Core / client boundary");
 assert!(!s.contains('/'));
 assert!(!s.contains('.'));
 assert!(s.starts_with("60"));
 }

 #[test]
 fn parse_section_number_handles_subnumber() {
 let (num, title) = split_section_number("39.1 Phase 0 design_doc schema");
 assert_eq!(num.as_deref(), Some("39.1"));
 assert!(title.starts_with("Phase 0"));
 }

 #[test]
 fn extract_entry_id_recognizes_round_format() {
 assert_eq!(
 extract_entry_id("Round 60 (OPTION G — ...):"),
 Some("Round 60".to_string())
 );
 assert_eq!(
 extract_entry_id("Round 33.5 (sub-round):"),
 Some("Round 33.5".to_string())
 );
 assert_eq!(extract_entry_id("not a Round entry"), None);
 }

 #[test]
 fn parse_intra_doc_anchor_link_excluded_from_cross_ref() {
 // Round 71 OPTION H-3 fix carry — `[text](#anchor)` 's anchor href -
 // CrossRef emit from exclude (silent orphan prevent).
 let input = "## 39. Test\n\nSee [link to here](#anchor-here) and [other](other.md#anchor).\n";
 let doc = parse_markdown(input, "DESIGN.md");
 // intra-doc anchor href capture not done.
 assert!(!doc.cross_refs.iter().any(|c| c.to_target.starts_with('#')));
 // cross-doc link captured.
 assert!(doc
 .cross_refs
 .iter()
 .any(|c| c.to_target == "other.md#anchor"));
 }

 #[test]
 fn parse_directory_ref_excluded_from_cross_ref() {
 // Round 78 ratify, Round 80 production migration — markdown link target's
 // directory path (`[text](dir/)` form) dropped silently from cross_refs.
 let input = "## status\n\n[bench](bench/) [crates](crates/) [file](crates/foo.rs)\n";
 let doc = parse_markdown(input, "README.md");
 assert!(!doc.cross_refs.iter().any(|c| c.to_target == "bench/"));
 assert!(!doc.cross_refs.iter().any(|c| c.to_target == "crates/"));
 // trailing `/` no file path as-is capture (Impl).
 assert!(doc.cross_refs.iter().any(|c| c.to_target == "crates/foo.rs"));
 }

 #[test]
 fn is_directory_ref_recognizes_trailing_slash() {
 assert!(is_directory_ref("bench/"));
 assert!(is_directory_ref("path/to/dir/"));
 assert!(!is_directory_ref(""));
 assert!(!is_directory_ref("file.md"));
 assert!(!is_directory_ref("dir/file.rs"));
 assert!(!is_directory_ref("docs/DESIGN.md#§39"));
 }

 // CommonMark §62 ATX-heading conformance — `#` must be followed by a
 // space, tab, or end of line. Inline `#N` references (e.g. "Phase 0
 // #1 and softened ...") must not be lifted to a numbered H1, or the
 // emitter produces `# 1. and softened ...` and round-trip breaks.
 #[test]
 fn atx_heading_requires_space_after_hashes() {
 // Hash directly followed by digit — not a heading.
 assert!(parse_heading("#1 and softened §5.J.3").is_none());
 // Hash directly followed by a letter — not a heading.
 assert!(parse_heading("#endif").is_none());
 // Hash + space + content — valid heading.
 let h = parse_heading("# 1. Title text").unwrap();
 assert_eq!(h.depth, 1);
 assert_eq!(h.section_number.as_deref(), Some("1"));
 assert_eq!(h.title, "Title text");
 // Hash + tab + content — valid heading per spec.
 let h = parse_heading("#\tafter tab").unwrap();
 assert_eq!(h.depth, 1);
 assert_eq!(h.title, "after tab");
 // Bare hashes only — not a heading (no content after trim).
 assert!(parse_heading("###").is_none());
 }

 // CommonMark §62 — ATX heading allows 0–3 leading spaces. Four or more
 // spaces puts the line into indented-code-block territory.
 #[test]
 fn atx_heading_rejects_four_plus_leading_spaces() {
 assert!(parse_heading("    # title").is_none());
 assert!(parse_heading("     #5 footnote").is_none());
 // 0–3 leading spaces are accepted.
 assert!(parse_heading("# title").is_some());
 assert!(parse_heading(" # title").is_some());
 assert!(parse_heading("  # title").is_some());
 assert!(parse_heading("   # title").is_some());
 }

 // CommonMark §98 — fenced code blocks may be indented up to three
 // spaces. Lines inside the fence are verbatim, so a `#endif` line
 // inside an indented C example must not surface as an H1 section.
 #[test]
 fn parse_markdown_indented_fence_is_recognized() {
 let input = concat!(
 "# Doc\n",
 "\n",
 "Prose paragraph.\n",
 "\n",
 "  ```c\n",
 "  #if Z_FEATURE == 1\n",
 "      do_thing();\n",
 "  #endif\n",
 "  ```\n",
 "\n",
 "More prose.\n",
 );
 let doc = parse_markdown(input, "test.md");
 // Only the H1 'Doc' should be a section. `#if` / `#endif` inside the
 // 2-space-indented fence must not be promoted to headings.
 let titles: Vec<&str> = doc.sections.iter().map(|s| s.title.as_str()).collect();
 assert_eq!(titles, vec!["Doc"], "got titles: {:?}", titles);
 }

 #[test]
 fn parse_markdown_round_trips_indented_fence_with_hash_lines() {
 // End-to-end round-trip: parse → emit → re-parse must yield the
 // same typed facts. The pre-fix parser created spurious `endif` /
 // `if z_feature_fragmentation--1` H1 sections from the fence body,
 // breaking section_identity even though no real heading existed.
 let input = concat!(
 "# RFC\n",
 "\n",
 "## OQ-W7\n",
 "\n",
 "- bullet:\n",
 "  ```c\n",
 "  #if Z_FEATURE_FRAGMENTATION == 1\n",
 "      ret = ok;\n",
 "  #endif\n",
 "  ```\n",
 "  After-fence prose with §2 reference.\n",
 "\n",
 "## OQ-W8\n",
 "\n",
 "Trailing section.\n",
 );
 let parsed = parse_markdown(input, "rfc.md");
 let emitted = crate::emitter::emit_markdown_with_default(&parsed, None);
 let reparsed = parse_markdown(&emitted, "rfc.md");
 let diff = crate::emitter::compare_typed_facts(&parsed, &reparsed);
 assert!(
 diff.section_identity_match,
 "section_identity broke: a={} b={}",
 diff.section_count_a,
 diff.section_count_b,
 );
 assert!(
 diff.cross_ref_set_match,
 "cross_ref_set broke: a={} b={}",
 diff.cross_ref_count_a,
 diff.cross_ref_count_b,
 );
 }

 // Inline `#N` style references in prose — common in review-comment
 // text like "(retracted Phase 0 #1)" — must not be parsed as headings.
 #[test]
 fn parse_markdown_inline_hash_number_in_prose_is_not_heading() {
 let input = concat!(
 "# Doc\n",
 "\n",
 "Some bullet with continuation:\n",
 "\n",
 "- item one with reviewer phrasing (retracted Phase 0\n",
 "  #1 and softened §5 variant criticism).\n",
 );
 let doc = parse_markdown(input, "test.md");
 let titles: Vec<&str> = doc.sections.iter().map(|s| s.title.as_str()).collect();
 assert_eq!(titles, vec!["Doc"], "got titles: {:?}", titles);
 }
}
