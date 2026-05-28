//! §61 markdown import adapter prototype (Round 62, OPTION E-2).
//!
//! Carries the Round 60 §39 *Phase 0 design_doc schema closed-form registered*
//! decision: a markdown → typed-facts transform prototype that covers the full
//! 4 entity/relation shape (Section: 5 fields / ChangelogEntry: 4 fields /
//! FrozenList: 4 fields / CrossRef: 2 fields). Round 61 §61/§56 body framing
//! update carry — first prototype validation source for *Phase 0 implementation
//! entry prerequisite framing*.
//!
//! Inputs:
//! - DESIGN.md §61 *Markdown variant spec* (line 5271+) — 13 mapping rules
//! - DESIGN.md §39 *Phase 0 design_doc schema closed-form registered* — 4 entities/relations
//! - small fixture: a markdown string covering DESIGN §39's first 1-2 sections
//!
//! Outputs (DESIGN §61 *Markdown variant spec* carry):
//! - `parse_markdown(input: &str) -> ParsedDoc`
//! - `ParsedDoc { sections, changelog_entries, frozen_lists, cross_refs, warnings }`
//! - small-fixture parse-consistency validation (in-memory typed-facts state)
//!
//! Prototype role (Round 61 framing update carry):
//! - 5th module of the bench/codegen-prototype crate (entity_indexer /
//! cf_wrapper / salsa_wire / closure_runtime / markdown_import).
//! - Emits schema-side *attribute typed signatures* only (parser-output
//! typed-facts schema-instance consistency is not this layer's burden — the
//! T1 validator body in §41 + §66 is the source of truth, per Round 61 framing).
//! - Validation source for small-fixture parse-feasibility (a full-scale
//! 7-markdown-file round-trip is a follow-up round, OPTION E-6+ scope).
//! - Line refs / line counts / TOC are not preserved (Round 18/24 *count-sync
//! update burden* auto-resolution contract — parser surfaces these via the
//! warning layer).

use std::collections::BTreeMap;

// ============================================================================
// Typed facts — DESIGN §39 *Phase 0 design_doc schema closed-form registered* full shape.
// Round 60 §39 closed-form registered carry.
// ============================================================================

/// Section entity — DESIGN §39 closed-form 5 field full shape.
/// `parent_section` nullable ref (file doc-root = None).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
 /// Canonical key — §N format ("39", "61.1") or unnumbered slug ("changelog").
 pub section_id: String,
 /// Parent doc identifier (relative path or doc-id).
 pub parent_doc: String,
 /// nullable ref — file's first h1 = None, follow-up heading = parent section_id.
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
 pub sub_bullets: Vec<String>,
 /// frozen_at_transaction_time — in the Phase 0 prototype this captures
 /// register-order as a monotonic i64 stamp (Round 60 closed-form
 /// registered carry; the full Phase 0 implementation will source this
 /// from the transaction-time service).
 pub frozen_at_transaction_time: i64,
}

/// FrozenList — DESIGN §39 closed-form 4 field, version-locked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrozenList {
 pub list_id: String,
 /// ChangelogEntry.entry_id ref — this list's frozen anchor.
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

/// CrossRef relation — Section→Section|Entity, 2 field.
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

// ============================================================================
// Parsed document — output of parse_markdown.
// ============================================================================

/// Parser output — the 4 typed-fact kinds from DESIGN §61 *Markdown variant spec*.
///
/// Round 116 carry — `bodies` + `line_anchors` are the source for the
/// spec query API's `SectionView`. The `parsed_doc_canonical` shape skips
/// these two derived fields when emitting (they sit outside the round-trip
/// diff = ∅ contract; they are derived dimensions, not source-of-truth).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedDoc {
 pub sections: Vec<Section>,
 pub changelog_entries: Vec<ChangelogEntry>,
 pub frozen_lists: Vec<FrozenList>,
 pub cross_refs: Vec<CrossRef>,
 /// Parser warnings — legacy line refs, unmapped constructs, partial
 /// FrozenList lock_kind, etc. (DESIGN §61 *parser warn exposure* carry).
 pub warnings: Vec<String>,
 /// Round 116 — section_id → raw body lines (everything from the heading's
 /// next line up to the next heading, with code-fence interiors preserved
 /// verbatim). Source for the spec query API's `SectionView.body`. Derived
 /// dimension (excluded from round-trip diff comparisons).
 pub bodies: BTreeMap<String, String>,
 /// Round 116 — section_id → heading line number (1-indexed). Source for
 /// the spec query API's `SectionView.line_anchor`. Derived dimension.
 pub line_anchors: BTreeMap<String, usize>,
}

// ============================================================================
// Small fixture — a markdown string covering DESIGN §39's first 1-2 sections.
// Phase 0 prototype validation source (Round 61 framing carry — small-fixture
// feasibility validation; full-scale round-trip is OPTION E-6+ scope).
// ============================================================================

/// Small markdown-string fixture covering the subset of the 13 DESIGN §61
/// mapping rules required by the prototype.
///
/// Covered items:
/// - `#` h1 (file doc-root)
/// - `## N. Title` numbered top-level (Section section_id = §N)
/// - `## Title` unnumbered top-level (Changelog)
/// - `### Title` nested heading
/// - `§N` inline literal CrossRef (decision)
/// - `[text](other.md#anchor)` cross-doc CrossRef
/// - `- Round N (...):` ChangelogEntry (under Changelog)
/// - `  - sub-bullet` ChangelogEntry.sub_bullets (2-space nested)
/// - `\`\`\`rust` code fence (Section.body verbatim)
/// - `(line N)` legacy line ref (parser warn, NOT captured)
/// - `---` horizontal rule (Section boundary marker)
pub fn design_doc_small_fixture() -> &'static str {
 r#"# Mnemosyne Design Decisions

This doc is §66 self-application's first dogfood medium.

## 39. Graph schema codegen

§39 is the source for the graph_schema kind. §41 datalog_rule and [ARCHITECTURE](architecture.md#l1-narrative-data-model) sit downstream of §39.

### Phase 0 design_doc schema closed-form registered

The design_doc schema's full 4-entity/relation shape — Section / ChangelogEntry / FrozenList / CrossRef. This registration is the source for §66 prerequisite #3 (§39 closed-form registered carry). (line 2929)

```rust
pub struct Section {
 pub section_id: String,
 pub title: String,
}
```

---

## 61. Import adapter framework

The §56 export adapter's *symmetric* counterpart. §61 body is the source of truth for the markdown ↔ typed-facts transform rules.

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
// Parser — mechanical parse of DESIGN §61's 13 mapping rules.
// ============================================================================

/// markdown bytes → typed-facts state. Deterministic — same input yields same `ParsedDoc`.
///
/// This parser is *single-pass* (DESIGN §61 *Iterative bootstrap protocol*'s
/// markdown specialization — Phase A/B no-op, Phase C mechanical, Phase D fallback).
pub fn parse_markdown(input: &str, parent_doc: &str) -> ParsedDoc {
 let mut out = ParsedDoc::default();
 let mut state = ParseState::new(parent_doc);

 for (line_idx, raw_line) in input.lines().enumerate() {
 let line = raw_line;
 state.current_line = line_idx + 1;

 // Code fence boundary — lines inside the fence are verbatim and not interpreted by the parser.
 if line.starts_with("```") {
 state.in_code_fence = !state.in_code_fence;
 state.append_body_line(line);
 continue;
 }
 if state.in_code_fence {
 state.append_body_line(line);
 // §N inline literals inside a code fence are NOT captured — preserved verbatim
 // (DESIGN §61 mapping table row "code fence ` ```lang ` fenced code block" carry).
 continue;
 }

 // Horizontal rule — Section boundary marker (no fact emitted; DESIGN §61
 // mapping table row "--- horizontal rule" carry).
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
 if let Some(entry) = parse_changelog_top_bullet(line) {
  // Flush pending entry before starting new one.
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
  // Sub-bullet without parent — warn and treat as section body.
  out.warnings.push(format!(
  "line {}: sub-bullet inside Changelog has no parent ChangelogEntry — falling back to Section.body",
  state.current_line
  ));
 }
 // Other content inside changelog — flush pending and continue as body.
 state.flush_pending_changelog(&mut out);
 }

 // CrossRef extraction — §N inline literal + [text](url) markdown link.
 for cross_ref in extract_cross_refs(line, &state) {
 out.cross_refs.push(cross_ref);
 }

 // Line ref legacy detection — `(line N)` is NOT captured (DESIGN §61 mapping table row).
 if let Some(line_ref_warn) = detect_legacy_line_ref(line, state.current_line) {
 out.warnings.push(line_ref_warn);
 }

 // Section.body fallback (Phase 0 — not promoted to a typed entity;
 // DESIGN §61 mapping table "non-fact bullet" row carry).
 state.append_body_line(line);
 }

 state.flush_pending_changelog(&mut out);

 // Round 116 — section_body buffer → ParsedDoc.bodies (raw lines joined).
 // BTreeMap iteration is deterministic. Section.body is a derived dimension
 // (excluded from round-trip diff comparisons), so its ordering is independent
 // of the sections Vec.
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
 /// Body buffer per section_id (Section.body unparsed fallback).
 section_body: BTreeMap<String, Vec<String>>,
 in_changelog: bool,
}

struct PendingChangelog {
 entry_id: String,
 sub_bullets: Vec<String>,
 frozen_at_transaction_time: i64,
}

impl ParseState {
 fn new(parent_doc: &str) -> Self {
 Self {
 parent_doc: parent_doc.to_string(),
 section_stack: Vec::new(),
 current_line: 0,
 in_code_fence: false,
 pending_changelog: None,
 transaction_time_counter: 0,
 section_body: BTreeMap::new(),
 in_changelog: false,
 }
 }

 fn next_transaction_time(&mut self) -> i64 {
 self.transaction_time_counter += 1;
 self.transaction_time_counter
 }

 fn current_section_id(&self) -> Option<&str> {
 self.section_stack.last().map(|(id, _)| id.as_str())
 }

 fn in_changelog_section(&self) -> bool {
 self.in_changelog
 }

 fn append_body_line(&mut self, line: &str) {
 if let Some(id) = self.current_section_id() {
 self.section_body
  .entry(id.to_string())
  .or_default()
  .push(line.to_string());
 }
 // Else: pre-section preamble lines are discarded (Phase 0 fallback).
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
 /// Numbered top-level: `## 39. Graph schema codegen` → Some("39").
 /// Sub-numbered: `### 39.1 Phase 0 ...` → Some("39.1").
 /// Unnumbered: None.
 section_number: Option<String>,
}

fn parse_heading(line: &str) -> Option<ParsedHeading> {
 let trimmed = line.trim_start();
 if !trimmed.starts_with('#') {
 return None;
 }
 let depth = trimmed.bytes().take_while(|&b| b == b'#').count();
 if depth == 0 || depth > 6 {
 return None;
 }
 let after_hashes = trimmed[depth..].trim_start();
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
/// `39.1 Phase 0 ...` → (Some("39.1"), "Phase 0 ...").
/// `Changelog`  → (None, "Changelog").
fn split_section_number(s: &str) -> (Option<String>, String) {
 // Match leading [0-9]+(\.[0-9]+)* followed by '.' or ' '.
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
 // Strip trailing '.' from number prefix if present.
 let mut num_end = idx;
 let mut number_str = &s[..num_end];
 if number_str.ends_with('.') {
 num_end -= 1;
 number_str = &s[..num_end];
 }
 // Skip the separator (must be '.' followed by space, or just space).
 let rest = &s[idx..];
 let rest = rest.strip_prefix('.').unwrap_or(rest);
 let rest = rest.trim_start();
 if number_str.is_empty() {
 return (None, s.to_string());
 }
 (Some(number_str.to_string()), rest.to_string())
}

fn apply_heading(state: &mut ParseState, out: &mut ParsedDoc, heading: ParsedHeading) {
 // Pop stack until top depth < heading.depth (parent_section this decision then section_id create).
 while let Some(&(_, top_depth)) = state.section_stack.last() {
 if top_depth >= heading.depth {
 state.section_stack.pop();
 } else {
 break;
 }
 }
 let parent_section = state.section_stack.last().map(|(id, _)| id.clone());

 // section_id decision (Round 67 fix — unnumbered nested section title duplicate + numbered
 // nested (## 60a in ### 1. Metadata schema same case) duplicate all prevent):
 // - depth 2 numbered (top-level decision section §N, ## under H1): bare §N (canonical key)
 // - depth >= 3 numbered (nested ### 1. Metadata schema etc.): `{parent}/{N}` prefix
 // - h1 doc-root unnumbered (parent = None): bare slug
 // - depth >= 2 unnumbered nested: `{parent}/{slug}` prefix
 let section_id = match (&heading.section_number, heading.depth, &parent_section) {
 (Some(n), 2, _) => n.clone(),
 (Some(n), _, Some(pid)) => format!("{}/{}", pid, n),
 (Some(n), _, None) => n.clone(),
 (None, _, None) => slug_for_unnumbered(&heading.title),
 (None, _, Some(pid)) => {
 format!("{}/{}", pid, slug_for_unnumbered(&heading.title))
 }
 };

 state
 .section_stack
 .push((section_id.clone(), heading.depth));

 state.in_changelog = is_changelog_title(&heading.title);

 // Round 116 — heading line anchor (1-indexed) capture.
 // same section_id - two time etc.pagewhen first-write-wins (parse_markdown in
 // duplicate section_id - - cycle warn scope, line_anchors - first carry).
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
 // ASCII / digit / hyphen / underscore / CJK preserved, others stripped.
 // Whitespace → '-'.
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
 0x3040..=0x30FF // Hiragana + Katakana
 | 0x3400..=0x4DBF // CJK Unified Ideographs Extension A
 | 0x4E00..=0x9FFF // CJK Unified Ideographs
 | 0xAC00..=0xD7AF // Hangul Syllables
 )
}

fn is_changelog_title(title: &str) -> bool {
 title == "Changelog" || title.eq_ignore_ascii_case("changelog") || title == "Changelog"
}

// ============================================================================
// ChangelogEntry parser — DESIGN §61 mapping table row 5-6.
// ============================================================================

struct TopBullet {
 entry_id: String,
}

fn parse_changelog_top_bullet(line: &str) -> Option<TopBullet> {
 // Top-level bullet (zero-indent) under `## Changelog`.
 if !line.starts_with("- ") {
 return None;
 }
 let body = &line[2..];
 // Recognize `Round N` prefix (with optional trailing `(...)`/`:`).
 let entry_id = extract_entry_id(body)?;
 Some(TopBullet { entry_id })
}

fn extract_entry_id(s: &str) -> Option<String> {
 let trimmed = s.trim_start();
 let prefix = "Round ";
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
 Some(format!("Round {}", &after[..idx]))
}

fn parse_changelog_sub_bullet(line: &str) -> Option<String> {
 // 2-space indent + `- `.
 if !line.starts_with(" - ") {
 return None;
 }
 Some(line[4..].to_string())
}

// ============================================================================
// CrossRef extraction — DESIGN §61 mapping table row 10-11.
// ============================================================================

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
 // Step over single char (UTF-8 safe).
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
  // Intra-doc anchor href `#anchor` - CrossRef emit out of scope —
  // §N inline literal scan this self doc citation source (DESIGN §61
  // mapping table row 12 lookup priority (1) intra-doc), markdown link
  // `[text](#anchor)`'s anchor slug - §N section_id and separate
  // (slug form mismatch — silent orphan reject prevent).
  if url.starts_with('#') {
  idx = after_close + close_paren + 1;
  continue;
  }
  // Directory ref `[text](dir/)` — DESIGN §61 row directory_ref
  // policy (Round 78 ratify, Round 80 production migration twin sync).
  // Filesystem path as notice marker -branch design fact not.
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

// ============================================================================
// Line ref legacy detection — DESIGN §61 mapping table row 12 (NOT capture, warn only).
// ============================================================================

/// Detect whether a markdown link target is a directory path (DESIGN §61 Round 78 ratify carry).
///
/// Rule: a trailing `/` marks a directory (`bench/`, `crates/`, `path/to/dir/`).
/// A `.md` file path is NOT a directory — it is a separate cross-doc kind.
fn is_directory_ref(url: &str) -> bool {
 !url.is_empty() && url.ends_with('/')
}

fn detect_legacy_line_ref(line: &str, line_no: usize) -> Option<String> {
 let needle = "(line ";
 if let Some(start) = line.find(needle) {
 // Validate that we have digits after.
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
// Determinism + sha256 helpers (Round 52 pattern carry).
// ============================================================================

/// Render ParsedDoc as canonical text (deterministic ordering for hash check).
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

// ============================================================================
// Tests — small fixture parse consistency validation (Round 62 measure data source).
// ============================================================================

#[cfg(test)]
mod tests {
 use super::*;

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
 assert!(h1.is_some(), "h1 doc-root section missing");
 let h1 = h1.unwrap();
 assert_eq!(h1.parent_section, None);
 assert_eq!(h1.decision_status, DecisionStatus::Active);
 }

 #[test]
 fn parse_numbered_top_level_section() {
 let doc = parse_markdown(design_doc_small_fixture(), "DESIGN.md");
 let s39 = doc.sections.iter().find(|s| s.section_id == "39");
 assert!(s39.is_some(), "## 39. ... section missing");
 let s39 = s39.unwrap();
 assert_eq!(s39.title, "Graph schema codegen");
 // h1 is parent (depth 1).
 assert!(s39.parent_section.is_some());
 }

 #[test]
 fn parse_unnumbered_section_changelog() {
 let doc = parse_markdown(design_doc_small_fixture(), "DESIGN.md");
 let cl = doc
 .sections
 .iter()
 .find(|s| s.title == "Changelog");
 assert!(cl.is_some());
 // Round 67 fix carry — unnumbered nested section's section_id -
 // `{parent_section_id}/{slug}` format (h1 doc-root parent in nested).
 let cl = cl.unwrap();
 assert_eq!(cl.section_id, "mnemosyne-design-decisions/change-history");
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
 // ### depth 3, parent = ## 39 depth 2.
 assert_eq!(nested.unwrap().parent_section.as_deref(), Some("39"));
 }

 #[test]
 fn parse_changelog_entries() {
 let doc = parse_markdown(design_doc_small_fixture(), "DESIGN.md");
 assert_eq!(doc.changelog_entries.len(), 2, "expected 2 entries");
 let e60 = &doc.changelog_entries[0];
 let e61 = &doc.changelog_entries[1];
 assert_eq!(e60.entry_id, "Round 60");
 assert_eq!(e61.entry_id, "Round 61");
 assert!(!e60.sub_bullets.is_empty());
 assert!(!e61.sub_bullets.is_empty());
 // frozen_at_transaction_time monotonic increasing.
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
 // Body of `## 39` mentions `§39` and `§41`.
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
 assert!(!cross_doc.is_empty(), "expected at least one cross-doc ref");
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
 assert!(warned, "expected legacy `(line N)` warning");
 }

 #[test]
 fn parse_code_fence_does_not_emit_cross_ref() {
 let input = "## 39. Test\n\n```rust\n§7 inside code\n```\n";
 let doc = parse_markdown(input, "DESIGN.md");
 // §7 inside code fence must NOT become a CrossRef.
 let has_section_7 = doc.cross_refs.iter().any(|c| c.to_target == "7");
 assert!(!has_section_7, "§N inside code fence must NOT capture");
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
 assert_eq!(a, b, "parse must be deterministic");
 assert_eq!(parsed_doc_canonical(&a), parsed_doc_canonical(&b));
 }

 #[test]
 fn parse_canonical_render_sha256_stable() {
 use crate::entity_indexer::sha256_hex;
 let doc = parse_markdown(design_doc_small_fixture(), "DESIGN.md");
 let canonical = parsed_doc_canonical(&doc);
 let h1 = sha256_hex(&canonical);
 let doc2 = parse_markdown(design_doc_small_fixture(), "DESIGN.md");
 let h2 = sha256_hex(&parsed_doc_canonical(&doc2));
 assert_eq!(h1, h2);
 assert_eq!(h1.len(), 64);
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
 fn parse_directory_ref_excluded_from_cross_ref() {
 // Round 80 — bench prototype twin sync (production
 // crates/mnemosyne-validate/src/parser.rs::parse_directory_ref_excluded_from_cross_ref).
 // markdown link target's directory path (`[text](dir/)` form) - dropped silently from cross_refs.
 let input = "## status\n\n[bench](bench/) [crates](crates/) [file](crates/foo.rs)\n";
 let doc = parse_markdown(input, "README.md");
 assert!(!doc.cross_refs.iter().any(|c| c.to_target == "bench/"));
 assert!(!doc.cross_refs.iter().any(|c| c.to_target == "crates/"));
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
}
