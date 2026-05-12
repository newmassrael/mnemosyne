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
//! - `ImplementationMissing` — §<id> exists with non-`Removed`
//! `decision_status` but `implementations` is empty (spec-side
//! coverage axiom: "Active = backed by code"). Third edge of the
//! Path B set-equality, complementing the two file-grained binding
//! directions above.
//!
//! The binding directions are *asymmetric in shape*: code-side
//! violations have a concrete (file, line, entry_id); the
//! `ImplementationUnbacked` spec-side variant has no line and carries
//! the impl-entry symbol; the `ImplementationMissing` spec-side variant
//! has neither file nor symbol (it is a section-level absence). This is
//! modeled as a 3-variant `CodeRefViolation` enum rather than collapsing
//! the directions into one struct with sentinel fields — the shape
//! differences are domain facts, not encoding accidents.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::atomic::AtomicStore;
use crate::config::{OrphanKind, OrphanLedgerEntry};
use crate::schema::DecisionStatus;

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
/// Three variants — code-side citations (`Citation`), file-grained
/// spec-side claims (`ImplementationUnbacked`), and section-level
/// spec-side absences (`ImplementationMissing`) have structurally
/// different evidence (a concrete file:line vs an impl-entry without a
/// code witness vs a section with no impl entries at all), so the enum
/// splits at those natural boundaries.
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
 /// Spec-side coverage axiom — `§section_id` exists in the atomic
 /// store with a non-`Removed` `decision_status` but its
 /// `implementations` list is empty: the section asserts a decision
 /// without naming any code that realizes it.
 ///
 /// `decision_status` is kept as the raw `Option<DecisionStatus>`
 /// (not pre-resolved to `Active`) so the audit-trail consumer can
 /// distinguish "no atomic override, parser default applies" from
 /// "atomic override = Active"; the None → Active fallback is a
 /// consumer-side convention (Round 265) and resolving it at
 /// emission time would discard authoring intent.
 ImplementationMissing {
 section_id: String,
 decision_status: Option<DecisionStatus>,
 },
}

impl CodeRefViolation {
 /// Stable kind tag for JSON output / CLI rendering. Citation
 /// violations carry their `ViolationKind` tag; the spec-side
 /// variants each have their own top-level kind.
 pub fn kind_tag(&self) -> &'static str {
 match self {
 CodeRefViolation::Citation { kind, .. } => match kind {
 ViolationKind::Missing => "missing",
 ViolationKind::Decay => "decay",
 ViolationKind::SectionMissing => "section_missing",
 ViolationKind::CitationUnbound => "citation_unbound",
 ViolationKind::InventoryMissing => "inventory_missing",
 ViolationKind::InventoryDeprecated => "inventory_deprecated",
 },
 CodeRefViolation::ImplementationUnbacked { .. } => "impl_unbacked",
 CodeRefViolation::ImplementationMissing { .. } => "impl_missing",
 }
 }

 /// Defect class — drives `--severity-missing` vs
 /// `--severity-binding` bucketing. Hallucination-class = cited
 /// identifier doesn't exist (Missing, SectionMissing). Binding-class
 /// = set-equality violation (CitationUnbound, ImplementationUnbacked,
 /// ImplementationMissing — all three edges of the Path B
 /// bidirectional binding). Decay is its own informational class —
 /// never reject-bucketed.
 pub fn defect_class(&self) -> DefectClass {
 match self {
 CodeRefViolation::Citation { kind, .. } => match kind {
 ViolationKind::Missing | ViolationKind::SectionMissing => {
 DefectClass::Hallucination
 }
 ViolationKind::CitationUnbound => DefectClass::Binding,
 ViolationKind::Decay => DefectClass::Decay,
 ViolationKind::InventoryMissing | ViolationKind::InventoryDeprecated => {
 DefectClass::Inventory
 }
 },
 CodeRefViolation::ImplementationUnbacked { .. } => DefectClass::Binding,
 CodeRefViolation::ImplementationMissing { .. } => DefectClass::Binding,
 }
 }
}

/// semantic axis that drives CLI severity flag bucketing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefectClass {
 /// Cited identifier doesn't exist (Missing, SectionMissing).
 Hallucination,
 /// Set-equality violation (CitationUnbound, ImplementationUnbacked,
 /// ImplementationMissing — all three edges of the Path B
 /// bidirectional binding).
 Binding,
 /// Cascade scan informational surface (Decay).
 Decay,
 /// Round 275 — Inventory axis violations (InventoryMissing,
 /// InventoryDeprecated). Distinct from Hallucination because the
 /// inventory genre has a different lifecycle vocabulary (Active /
 /// Deprecated / Reserved) and a separate severity knob
 /// (`severity_inventory`) for per-project tuning.
 Inventory,
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
 /// Round 275 — Inventory ID citation where the cited id is not in
 /// `AtomicStore.inventory_entries`. Hallucination-class on the
 /// inventory axis (Phase 1A 5th entity).
 InventoryMissing,
 /// Round 275 — Inventory ID citation where the cited id exists but
 /// `InventoryEntry.status == Deprecated`. Author should update or
 /// remove the cite; the inventory entry is no longer in active use.
 /// `Reserved` status does not trigger this — Reserved is "set aside,
 /// cite permitted" by R275 design.
 InventoryDeprecated,
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
///
/// Pre-Round-277 caller convenience — delegates to
/// [`extract_section_citations_v3`] with empty external-prefix slices
/// (external-skip disabled, identical to the v1 behavior).
pub fn extract_section_citations(content: &str) -> Vec<(usize, String)> {
 extract_section_citations_v3(content, &[], &[])
}

/// Round 277 — `§<id>` extractor with external-standard numeric-mode
/// skip. Delegates to [`extract_section_citations_v3`] with empty
/// `prefixes_bare` slice (numeric-mode only, identical to the R277/R281
/// behavior).
pub fn extract_section_citations_v2(
 content: &str,
 external_prefixes: &[String],
) -> Vec<(usize, String)> {
 extract_section_citations_v3(content, external_prefixes, &[])
}

/// Round 284 — `§<id>` extractor with two external-skip axes:
/// *numeric* (RFC / IEEE / ISO/IEC, `<PREFIX> <NUMERIC> §<id>`) and
/// *bare* (AUTOSAR family, `<PREFIX> §<id>` without numeric).
///
/// The two axes are independent — same prefix may appear in both if the
/// standard supports both forms; matching tries the axis that applies
/// based on the shape of the token preceding `§`.
///
/// Empty slices = the corresponding axis disabled. Both empty = no
/// external skip, equivalent to [`extract_section_citations`].
pub fn extract_section_citations_v3(
 content: &str,
 external_prefixes_numeric: &[String],
 external_prefixes_bare: &[String],
) -> Vec<(usize, String)> {
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
 // Round 277/284 — external-standard context check. Skip if
 // the § is preceded (same line) by either numeric-mode
 // `<prefix> <numeric>` or bare-mode `<prefix>` (with leading
 // punctuation strip from R281).
 if (!external_prefixes_numeric.is_empty() || !external_prefixes_bare.is_empty())
 && is_external_section_cite(
 &line[..i],
 external_prefixes_numeric,
 external_prefixes_bare,
 )
 {
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

/// Round 277 + 284 — detect external-standard context preceding a `§`
/// sigil.
///
/// Two recognized forms, mutually exclusive on the shape of the token
/// immediately before the `§`:
///
/// - **Numeric mode** (R277): `<prefix> <numeric> §<id>` where
/// `<numeric>` is digits + dots (`2131`, `802.3`, `14882`). Prefix
/// matched verbatim against `prefixes_numeric` after punctuation
/// strip (R281). Used by RFC / IEEE / ISO/IEC.
/// - **Bare mode** (R284): `<prefix> §<id>` — no numeric between
/// prefix and sigil. Prefix matched verbatim against
/// `prefixes_bare` after punctuation strip. Used by AUTOSAR family
/// (TR_SOMEIP, SOMEIPSD, SWS_SD) and other doc-name-only standards.
///
/// Mode selection is by *last token shape*: if the last token (closest
/// to the sigil) is numeric, the numeric path runs; otherwise the bare
/// path runs. The two axes are independent — same prefix may be
/// registered in both if the standard supports both forms; matching
/// tries the relevant axis.
///
/// Multi-token prefixes (e.g., `"ETSI TS"`) are not v1 — only the last
/// non-whitespace token before the trigger is consulted. Workaround:
/// register the trailing token (`"TS"`) as a slightly looser match.
fn is_external_section_cite(
 line_before_sigil: &str,
 prefixes_numeric: &[String],
 prefixes_bare: &[String],
) -> bool {
 // Both forms require whitespace between the trigger and the sigil;
 // otherwise this is an inline reference (`RFC2131§3`) which is not
 // the recognized form.
 let trimmed = line_before_sigil.trim_end();
 if trimmed.len() == line_before_sigil.len() {
 return false;
 }
 let last_token_start = trimmed
 .rfind(char::is_whitespace)
 .map(|i| i + 1)
 .unwrap_or(0);
 let last_token = &trimmed[last_token_start..];
 if last_token.is_empty() {
 return false;
 }
 let last_is_numeric = last_token
 .chars()
 .all(|c| c.is_ascii_digit() || c == '.')
 && last_token.chars().any(|c| c.is_ascii_digit());

 if last_is_numeric {
 // Numeric mode (R277). Prev token must match prefixes_numeric.
 if prefixes_numeric.is_empty() {
 return false;
 }
 let before_last = trimmed[..last_token_start].trim_end();
 if before_last.is_empty() {
 return false;
 }
 let prev_token_start = before_last
 .rfind(char::is_whitespace)
 .map(|i| i + 1)
 .unwrap_or(0);
 let prev_token = &before_last[prev_token_start..];
 let prev_clean = prev_token.trim_start_matches(|c: char| !c.is_alphanumeric());
 prefixes_numeric.iter().any(|p| p == prev_clean)
 } else {
 // Bare mode (R284). Last token itself must match prefixes_bare.
 if prefixes_bare.is_empty() {
 return false;
 }
 let last_clean = last_token.trim_start_matches(|c: char| !c.is_alphanumeric());
 prefixes_bare.iter().any(|p| p == last_clean)
 }
}

fn is_section_id_char(c: char) -> bool {
 c.is_ascii_alphanumeric() || c == '.' || c == '/' || c == '-' || c == '_'
}

/// Round 275 — Extract inventory ID citations from `content` (Phase 1A).
///
/// For each `prefix` in `prefixes`, scans `<prefix><tail>` tokens where
/// `<tail>` matches `[A-Z0-9_]+` *and ends in a digit*. The digit-terminus
/// rule distinguishes inventory IDs (e.g., `ARP_07`,
/// `TCP_RETRANSMISSION_TO_04`) from coding-convention identifiers
/// (`TCP_BUFFER_SIZE`, `ARP_PROTO_TYPE`) — the dominant false-positive
/// surface when scanning C/Rust/Java codebases.
///
/// Word-boundary rules mirror `extract_citations`: the char before
/// `<prefix>` must be non-alphanumeric/non-underscore, and the char after
/// `<tail>` must be the same. Backtick code-span skipping mirrors
/// `extract_section_citations` (the comment-only filter handles the
/// dominant string-literal surface; this is the inline doc-example
/// guard).
///
/// Output: `(line_idx_1_based, full_inventory_id)` pairs, deduped on
/// `(line, id)` so that a single token matched by multiple registered
/// prefixes (e.g., `SOMEIP_` and `SOMEIP_ETS_` both registered, token =
/// `SOMEIP_ETS_BASICS_01`) surfaces once with the longest-prefix match
/// recorded. Returns empty when `prefixes.is_empty()` (axis disabled).
pub fn extract_inventory_citations(
 prefixes: &[String],
 content: &str,
) -> Vec<(usize, String)> {
 if prefixes.is_empty() {
 return Vec::new();
 }
 // Longest-prefix-first ordering so that overlapping registrations
 // (`SOMEIP_` and `SOMEIP_ETS_`) yield the longer match — the more
 // specific ID is what the author intended.
 let mut ordered: Vec<&String> = prefixes.iter().collect();
 ordered.sort_by_key(|p| std::cmp::Reverse(p.len()));

 let mut seen: BTreeSet<(usize, String)> = BTreeSet::new();
 for (line_idx, line) in content.lines().enumerate() {
 let mut in_backtick = false;
 let bytes = line.as_bytes();
 // Round 279 Bug #1 fix — drive the outer loop with `char_indices`
 // instead of raw byte indexing. A non-ASCII char in the comment
 // (em-dash `—`, Korean, CJK, …) previously left `i` mid-multibyte,
 // and the next `line[i..].starts_with(prefix)` call panicked at
 // a UTF-8 char-boundary check. `char_indices` yields only valid
 // boundaries, so `line[i..]` is always safe; advancement after a
 // match is done via `peek/next` until past the matched byte span.
 let mut chars = line.char_indices().peekable();
 while let Some((i, c)) = chars.next() {
 if c == '`' {
 in_backtick = !in_backtick;
 continue;
 }
 if in_backtick {
 continue;
 }
 let mut matched_len: Option<usize> = None;
 let mut matched_id: Option<String> = None;
 for prefix in &ordered {
 if !line[i..].starts_with(prefix.as_str()) {
  continue;
 }
 // word boundary before the prefix
 let prev_ok = i == 0
  || !line[..i]
  .chars()
  .last()
  .map(|c| c.is_alphanumeric() || c == '_')
  .unwrap_or(false);
 if !prev_ok {
  continue;
 }
 let tail_start = i + prefix.len();
 // tail = [A-Z0-9_]+ — uppercase-or-digit-or-underscore.
 let tail_bytes = &bytes[tail_start..];
 let mut t = 0usize;
 while t < tail_bytes.len() {
  let c = tail_bytes[t];
  if c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_' {
  t += 1;
  } else {
  break;
  }
 }
 if t == 0 {
  continue;
 }
 let tail_end = tail_start + t;
 // word boundary after the tail
 let next_ok = tail_end >= line.len()
  || !line[tail_end..]
  .chars()
  .next()
  .map(|c| c.is_alphanumeric() || c == '_')
  .unwrap_or(false);
 if !next_ok {
  continue;
 }
 // tail must end in a digit — TC8 / ISO test-spec convention,
 // suppresses identifier-shaped false positives.
 if !tail_bytes[t - 1].is_ascii_digit() {
  continue;
 }
 let id = format!("{}{}", prefix, &line[tail_start..tail_end]);
 matched_len = Some(prefix.len() + t);
 matched_id = Some(id);
 break; // longest-first ordering — first match wins
 }
 if let (Some(consumed), Some(id)) = (matched_len, matched_id) {
 seen.insert((line_idx + 1, id));
 // Advance past the consumed bytes — `peek/next` until we pass
 // `i + consumed`. char_indices keeps the iterator on valid
 // char boundaries even when prefix-length advance lands on
 // an ASCII byte (TC ID tails are uppercase ASCII by design).
 let target_byte = i + consumed;
 while let Some(&(k, _)) = chars.peek() {
 if k < target_byte {
  chars.next();
 } else {
  break;
 }
 }
 }
 }
 }
 seen.into_iter().collect()
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
/// 4. Same walk: for each §X with `decision_status != Removed` and
/// empty `implementations` → `ImplementationMissing` (spec-side
/// coverage axiom — Round 269).
///
/// `filter_id` is the decay-scan toggle. When `Some`, only
/// Round NNN citations matching the filter are surfaced (as `Decay`);
/// all other Round NNN citations are suppressed, and the §<id> axis
/// stays silent for symmetry (a Superseded-decision cascade caller is
/// asking "where is this entry_id mentioned?", not "audit the whole
/// store" — keep the surface narrow). Steps 3 and 4 are also skipped
/// under decay-filter mode for the same surface-narrowing reason.
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
/// Pre-Round-275 caller convenience — delegates to
/// [`scan_paths_bidirectional_v2`] with an empty `inventory_prefixes`
/// slice (inventory axis disabled). Existing tests + cascade callers
/// keep their 7-arg shape; the Phase 1A wire-up calls v2 directly.
pub fn scan_paths_bidirectional(
 workspace_root: &Path,
 paths: &[String],
 prefix: &str,
 store: &AtomicStore,
 orphan_ledger: &[OrphanLedgerEntry],
 filter_id: Option<&str>,
 comment_only: bool,
) -> std::io::Result<Vec<CodeRefViolation>> {
 scan_paths_bidirectional_v2(
 workspace_root,
 paths,
 prefix,
 store,
 orphan_ledger,
 filter_id,
 comment_only,
 &[],
 )
}

/// Round 275 — Phase 1A scanner with inventory axis. Pre-Round-277
/// caller convenience — delegates to [`scan_paths_bidirectional_v3`]
/// with an empty `external_section_prefixes` slice (external skip
/// disabled, identical to the v2 behavior).
pub fn scan_paths_bidirectional_v2(
 workspace_root: &Path,
 paths: &[String],
 prefix: &str,
 store: &AtomicStore,
 orphan_ledger: &[OrphanLedgerEntry],
 filter_id: Option<&str>,
 comment_only: bool,
 inventory_prefixes: &[String],
) -> std::io::Result<Vec<CodeRefViolation>> {
 scan_paths_bidirectional_v3(
 workspace_root,
 paths,
 prefix,
 store,
 orphan_ledger,
 filter_id,
 comment_only,
 inventory_prefixes,
 &[],
 )
}

/// Round 277 — Phase 1A scanner with inventory axis + external-standard
/// numeric-mode `§<id>` skip. Delegates to
/// [`scan_paths_bidirectional_v4`] with an empty
/// `external_section_prefixes_bare` slice — back-compat shim for
/// callers from R277 / R281.
pub fn scan_paths_bidirectional_v3(
 workspace_root: &Path,
 paths: &[String],
 prefix: &str,
 store: &AtomicStore,
 orphan_ledger: &[OrphanLedgerEntry],
 filter_id: Option<&str>,
 comment_only: bool,
 inventory_prefixes: &[String],
 external_section_prefixes: &[String],
) -> std::io::Result<Vec<CodeRefViolation>> {
 scan_paths_bidirectional_v4(
 workspace_root,
 paths,
 prefix,
 store,
 orphan_ledger,
 filter_id,
 comment_only,
 inventory_prefixes,
 external_section_prefixes,
 &[],
 )
}

/// Round 284 — scanner with two external-standard `§<id>` axes:
/// *numeric* (R277 form, `<PREFIX> <NUMERIC> §<id>`) and *bare*
/// (R284 form, `<PREFIX> §<id>` doc-name only). The two axes are
/// independent; an empty slice disables the corresponding axis.
pub fn scan_paths_bidirectional_v4(
 workspace_root: &Path,
 paths: &[String],
 prefix: &str,
 store: &AtomicStore,
 orphan_ledger: &[OrphanLedgerEntry],
 filter_id: Option<&str>,
 comment_only: bool,
 inventory_prefixes: &[String],
 external_section_prefixes_numeric: &[String],
 external_section_prefixes_bare: &[String],
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
 for (line, section_id) in extract_section_citations_v3(
 &content,
 external_section_prefixes_numeric,
 external_section_prefixes_bare,
 ) {
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

 // ---- Round 275 — Inventory ID axis (Phase 1A) ----
 // `inventory_prefixes` empty = axis disabled (5-min setup carry).
 // Active / Reserved status pass silently; Deprecated triggers
 // `InventoryDeprecated`; missing IDs trigger `InventoryMissing`.
 // Per-file decoration happens inside this loop; ledger suppression
 // is a future carry (see Round 275 changelog).
 for (line, inventory_id) in extract_inventory_citations(inventory_prefixes, &content) {
 let kind = match store.inventory(&inventory_id).map(|e| e.status) {
 None => Some(ViolationKind::InventoryMissing),
 Some(crate::atomic::InventoryStatus::Deprecated) => {
 Some(ViolationKind::InventoryDeprecated)
 }
 // Active / Reserved — cite-permitted.
 Some(_) => None,
 };
 if let Some(k) = kind {
 violations.push(CodeRefViolation::Citation {
 citation: Citation {
  file: rel.clone(),
  line,
  entry_id: inventory_id,
 },
 kind: k,
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

 // ---- Step 4: spec-side coverage axiom (Round 269) ----
 // Workspace-wide enumeration: a section with non-Removed decision_status
 // and zero implementations is the "Active = backed by code" axiom
 // violation. Removed is tombstone-exempt (legitimately carries no impls).
 // None → Active fallback (Round 265 consumer-side convention) used only
 // for the trigger comparison; the raw Option is preserved in the emitted
 // variant so the audit-trail consumer keeps full information.
 // Skip under decay-filter mode for surface-narrowing symmetry with
 // Steps 2-3 (a Superseded-cascade caller's question is targeted).
 if filter_id.is_none() {
 for (section_id, section) in &store.sections {
 if !section.implementations.is_empty() {
 continue;
 }
 let resolved = section.decision_status.unwrap_or(DecisionStatus::Active);
 if resolved == DecisionStatus::Removed {
 continue;
 }
 violations.push(CodeRefViolation::ImplementationMissing {
 section_id: section_id.clone(),
 decision_status: section.decision_status,
 });
 }
 }

 sort_violations(&mut violations);
 Ok(violations)
}

/// Round 266 — auto-cascade trigger primitive (Stage B freshness).
///
/// Targeted decay scan for §<section_id> citations of *one* section,
/// returned as a flat list of [`Citation`]. Used by the mutate-time hook
/// in `set-section-decision-status-atomic` CLI: when a section transitions
/// to Superseded/Removed, this surfaces the source-side citations that
/// will need authoring follow-up (no rejection — informational only).
///
/// Skips file-read failures silently (consistent with the bidirectional
/// scanner's behavior). Honors `comment_only` via `strip_to_comments` so
/// fixture string literals don't generate noise.
///
/// `paths` is workspace-relative; symbol-side bindings are not consulted
/// (decay is about cite locations, not implementation universe).
pub fn scan_section_decay(
 workspace_root: &Path,
 paths: &[String],
 section_id: &str,
 comment_only: bool,
) -> std::io::Result<Vec<Citation>> {
 let files = walk_paths(workspace_root, paths)?;
 let mut hits = Vec::new();
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
 for (line, sid) in extract_section_citations(&content) {
 if sid == section_id {
 hits.push(Citation {
  file: rel.clone(),
  line,
  entry_id: format!("§{}", sid),
 });
 }
 }
 }
 hits.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
 Ok(hits)
}

/// Round 276 — Inventory axis cascade trigger primitive (Phase 1A).
///
/// Targeted decay scan for a single inventory ID's citations across
/// `paths`. Mirrors [`scan_section_decay`] on the §<id> axis. Used by
/// the mutate-time hook in the `add-inventory-entry` (registered
/// Deprecated), `set-inventory-status` (transition to Deprecated), and
/// `remove-inventory-entry` CLI surfaces — the cascade surfaces author-
/// follow-up sites without rejecting the mutate.
///
/// `inventory_prefixes` are required for the extractor lookup; an empty
/// slice yields no hits regardless of input. `comment_only` toggles the
/// shared filter so fixture string literals don't generate noise.
///
/// Skips file-read failures silently (consistent with the bidirectional
/// scanner). Returns hits sorted by `(file, line)`.
pub fn scan_inventory_decay(
 workspace_root: &Path,
 paths: &[String],
 inventory_id: &str,
 inventory_prefixes: &[String],
 comment_only: bool,
) -> std::io::Result<Vec<Citation>> {
 if inventory_prefixes.is_empty() {
 return Ok(Vec::new());
 }
 let files = walk_paths(workspace_root, paths)?;
 let mut hits = Vec::new();
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
 for (line, id) in extract_inventory_citations(inventory_prefixes, &content) {
 if id == inventory_id {
 hits.push(Citation {
  file: rel.clone(),
  line,
  entry_id: id,
 });
 }
 }
 }
 hits.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
 Ok(hits)
}

/// Deterministic ordering — Citation variants sort by (file, line, entry_id);
/// ImplementationUnbacked variants sort by (file, section_id, symbol);
/// ImplementationMissing variants sort by section_id. The variant order is
/// Citation < ImplementationUnbacked < ImplementationMissing so existing
/// reports keep their relative diff stability when the third edge surfaces.
fn sort_violations(violations: &mut Vec<CodeRefViolation>) {
 violations.sort_by(|a, b| {
 use CodeRefViolation::*;
 use std::cmp::Ordering;
 fn rank(v: &CodeRefViolation) -> u8 {
 match v {
 Citation { .. } => 0,
 ImplementationUnbacked { .. } => 1,
 ImplementationMissing { .. } => 2,
 }
 }
 let r = rank(a).cmp(&rank(b));
 if r != Ordering::Equal {
 return r;
 }
 match (a, b) {
 (Citation { citation: c1, .. }, Citation { citation: c2, .. }) => c1
 .file
 .cmp(&c2.file)
 .then(c1.line.cmp(&c2.line))
 .then(c1.entry_id.cmp(&c2.entry_id)),
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
 (
 ImplementationMissing { section_id: s1, .. },
 ImplementationMissing { section_id: s2, .. },
 ) => s1.cmp(s2),
 // rank() already separated cross-variant pairs above.
 _ => unreachable!("cross-variant ordering handled by rank()"),
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

 // ============ Round 266 scan_section_decay tests ============

 #[test]
 fn scan_section_decay_surfaces_only_target_section() {
 // Round 266 — targeted §<id> decay scan returns only citations of
 // the requested section_id; other sections in the same file ignored.
 let tmp = TempDir::new().unwrap();
 let src = tmp.path().join("src");
 std::fs::create_dir_all(&src).unwrap();
 std::fs::write(
 src.join("a.rs"),
 "// §39 here\n// §61 here\n// §39 again\n// §99 elsewhere\n",
 )
 .unwrap();
 let hits =
 scan_section_decay(tmp.path(), &["src/".to_string()], "39", true).unwrap();
 assert_eq!(hits.len(), 2);
 assert_eq!(hits[0].entry_id, "§39");
 assert_eq!(hits[0].line, 1);
 assert_eq!(hits[1].line, 3);
 }

 #[test]
 fn scan_section_decay_empty_when_no_citations() {
 let tmp = TempDir::new().unwrap();
 let src = tmp.path().join("src");
 std::fs::create_dir_all(&src).unwrap();
 std::fs::write(src.join("clean.rs"), "fn main() {}\n").unwrap();
 let hits =
 scan_section_decay(tmp.path(), &["src/".to_string()], "39", true).unwrap();
 assert!(hits.is_empty());
 }

 #[test]
 fn scan_section_decay_respects_comment_only_flag() {
 // String-literal §X tokens must be excluded under comment_only=true
 // (consistent with the bidirectional scanner's behavior). When false,
 // the whole-text scan picks them up.
 let tmp = TempDir::new().unwrap();
 let src = tmp.path().join("src");
 std::fs::create_dir_all(&src).unwrap();
 std::fs::write(
 src.join("fixture.rs"),
 "let s = \"§39 in string\";\n// §39 in comment\n",
 )
 .unwrap();
 let comment_hits =
 scan_section_decay(tmp.path(), &["src/".to_string()], "39", true).unwrap();
 assert_eq!(comment_hits.len(), 1, "comment_only excludes string literal");
 assert_eq!(comment_hits[0].line, 2);
 let raw_hits =
 scan_section_decay(tmp.path(), &["src/".to_string()], "39", false).unwrap();
 assert_eq!(raw_hits.len(), 2, "comment_only=false picks up both");
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

 // ============ Round 269: ImplementationMissing (spec-side coverage axiom) ============

 /// Builds an empty workspace dir + a store whose `section_id` exists
 /// but has no implementations. `decision_status` lets the test pin
 /// the atomic override; pass `None` to exercise the parser-default
 /// fallback path.
 fn build_store_with_empty_section(
 section_id: &str,
 decision_status: Option<DecisionStatus>,
 ) -> AtomicStore {
 let mut store = AtomicStore::new();
 let section = store.section_mut(section_id);
 section.decision_status = decision_status;
 // implementations stays at Vec::default() = []
 store
 }

 #[test]
 fn coverage_axiom_active_empty_impls_triggers() {
 let tmp = TempDir::new().unwrap();
 let store = build_store_with_empty_section("39", Some(DecisionStatus::Active));
 // No source files written — workspace is otherwise silent.
 std::fs::create_dir_all(tmp.path().join("src")).unwrap();
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
 CodeRefViolation::ImplementationMissing {
 section_id,
 decision_status,
 } => {
 assert_eq!(section_id, "39");
 assert_eq!(*decision_status, Some(DecisionStatus::Active));
 }
 other => panic!("unexpected variant: {:?}", other),
 }
 }

 #[test]
 fn coverage_axiom_none_status_falls_back_to_active_triggers() {
 // Parser-default fallback (Round 265 convention) — None resolves
 // to Active for the trigger check, but the emitted variant
 // preserves the raw None so the audit-trail consumer can tell.
 let tmp = TempDir::new().unwrap();
 let store = build_store_with_empty_section("39", None);
 std::fs::create_dir_all(tmp.path().join("src")).unwrap();
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
 CodeRefViolation::ImplementationMissing {
 section_id,
 decision_status,
 } => {
 assert_eq!(section_id, "39");
 assert_eq!(*decision_status, None, "raw Option preserved, not resolved");
 }
 other => panic!("unexpected variant: {:?}", other),
 }
 }

 #[test]
 fn coverage_axiom_superseded_empty_impls_also_triggers() {
 // Superseded with empty impls = "marked dead but never recorded
 // where it lived" — audit gap, surfaced.
 let tmp = TempDir::new().unwrap();
 let store = build_store_with_empty_section("39", Some(DecisionStatus::Superseded));
 std::fs::create_dir_all(tmp.path().join("src")).unwrap();
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
 CodeRefViolation::ImplementationMissing {
 section_id,
 decision_status,
 } => {
 assert_eq!(section_id, "39");
 assert_eq!(*decision_status, Some(DecisionStatus::Superseded));
 }
 other => panic!("unexpected variant: {:?}", other),
 }
 }

 #[test]
 fn coverage_axiom_removed_empty_impls_does_not_trigger() {
 // Removed = tombstone genre, legitimately carries no impls.
 let tmp = TempDir::new().unwrap();
 let store = build_store_with_empty_section("39", Some(DecisionStatus::Removed));
 std::fs::create_dir_all(tmp.path().join("src")).unwrap();
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
 assert!(v.is_empty(), "Removed must not trigger, got: {:?}", v);
 }

 #[test]
 fn coverage_axiom_non_empty_impls_does_not_trigger() {
 // Section with at least one implementation is exempt from the
 // coverage axiom regardless of citation match status (which is
 // the ImplementationUnbacked axis's job).
 let tmp = TempDir::new().unwrap();
 let store_path = tmp.path().join(".atomic/workspace.atomic.json");
 let store = build_store_with_impl(&store_path, "39", "src/foo.rs", None);
 std::fs::create_dir_all(tmp.path().join("src")).unwrap();
 std::fs::write(tmp.path().join("src/foo.rs"), "// §39 cite\n").unwrap();
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
 assert!(
 v.iter().all(|x| !matches!(x, CodeRefViolation::ImplementationMissing { .. })),
 "no ImplementationMissing expected, got: {:?}",
 v
 );
 }

 #[test]
 fn coverage_axiom_decay_filter_silences_surface() {
 // Symmetry with Steps 2-3: a Superseded-cascade caller asks
 // "where is THIS entry_id cited?", not "audit the whole store".
 // Coverage axiom stays silent under filter_id.
 let tmp = TempDir::new().unwrap();
 let store = build_store_with_empty_section("39", Some(DecisionStatus::Active));
 std::fs::create_dir_all(tmp.path().join("src")).unwrap();
 let v = scan_paths_bidirectional(
 tmp.path(),
 &["src/".to_string()],
 "Round ",
 &store,
 &[],
 Some("Round 99"),
 true,
 )
 .unwrap();
 assert!(v.is_empty(), "filter_id should silence coverage axiom, got: {:?}", v);
 }

 // ============================================================================
 // Round 275 — Inventory axis tests (Phase 1A).
 // ============================================================================

 #[test]
 fn extract_inventory_citations_survives_non_ascii_comment_chars() {
 // Round 279 Bug #1 regression — the byte-index loop used to panic
 // at the first `line[i..].starts_with(prefix)` call when a multi-
 // byte char (em-dash `\u{2014}`, Korean, CJK) sat between earlier
 // ASCII and the prefix. The fixture replays the original tc8-
 // harness panic frame and exercises Korean + CJK as well.
 let prefixes = vec!["FOO_".to_string()];
 // Source uses \u{2014} so the test file itself stays ASCII-clean
 // (the self-application scan must not see an em-dash literal).
 let fixture = format!(
 "// SERVICE-ID-2 (0xF4E8) is the natural target {} FOO_01 cite\n\
  // \u{D55C}\u{AE00} \u{C8FC}\u{C11D} \u{C548} FOO_02\n\
  // \u{4E2D}\u{6587}\u{6CE8}\u{91CA} FOO_03\n",
 '\u{2014}'
 );
 let out = extract_inventory_citations(&prefixes, &fixture);
 assert_eq!(
 out,
 vec![
 (1, "FOO_01".to_string()),
 (2, "FOO_02".to_string()),
 (3, "FOO_03".to_string()),
 ],
 "all three cites must surface; no panic on multi-byte chars"
 );
 }

 #[test]
 fn scan_v3_survives_non_ascii_comment_chars() {
 // Round 279 Bug #1 regression — full scan path (including
 // strip_to_comments) must not panic when a workspace source file
 // contains the original em-dash trigger from the tc8-harness
 // bug report.
 use crate::atomic::AtomicStore;
 let tmp = TempDir::new().unwrap();
 std::fs::create_dir_all(tmp.path().join("src")).unwrap();
 let content = format!(
 "// SERVICE-ID-2 (0xF4E8) target {} DUT offers FOO_01\n",
 '\u{2014}'
 );
 std::fs::write(tmp.path().join("src/x.rs"), content).unwrap();
 let store = AtomicStore::new();
 let prefixes = vec!["FOO_".to_string()];
 let v = scan_paths_bidirectional_v3(
 tmp.path(),
 &["src/".to_string()],
 "Round ",
 &store,
 &[],
 None,
 true,
 &prefixes,
 &[],
 )
 .expect("scan must not panic on multi-byte comment chars");
 // FOO_01 is the only cite and it's not registered, so it surfaces
 // as InventoryMissing. The point of the test is "no panic" plus
 // correct extraction past the em-dash.
 assert_eq!(v.len(), 1, "expected exactly the FOO_01 cite, got: {:?}", v);
 }

 #[test]
 fn extract_inventory_citations_basic() {
 let prefixes = vec!["ARP_".to_string()];
 let out = extract_inventory_citations(&prefixes, "// ARP_07 cite\nfn x() {}\n");
 assert_eq!(out, vec![(1, "ARP_07".to_string())]);
 }

 #[test]
 fn extract_inventory_citations_multi_prefix() {
 let prefixes = vec!["ARP_".to_string(), "TCP_".to_string()];
 let out = extract_inventory_citations(
 &prefixes,
 "// ARP_07 and TCP_RETRANSMISSION_TO_04\n",
 );
 assert_eq!(
 out,
 vec![
  (1, "ARP_07".to_string()),
  (1, "TCP_RETRANSMISSION_TO_04".to_string()),
 ]
 );
 }

 #[test]
 fn extract_inventory_citations_tail_must_end_in_digit() {
 // Coding-convention identifiers (TCP_BUFFER_SIZE) are NOT inventory IDs.
 // Only tokens ending in a digit are treated as cites.
 let prefixes = vec!["TCP_".to_string()];
 let out = extract_inventory_citations(
 &prefixes,
 "// TCP_BUFFER_SIZE constant ; TCP_BUFFER_03 cite\n",
 );
 assert_eq!(out, vec![(1, "TCP_BUFFER_03".to_string())]);
 }

 #[test]
 fn extract_inventory_citations_longest_prefix_wins() {
 // When SOMEIP_ and SOMEIP_ETS_ are both registered, SOMEIP_ETS_BASICS_01
 // is reported once under the longer (more specific) prefix.
 let prefixes = vec!["SOMEIP_".to_string(), "SOMEIP_ETS_".to_string()];
 let out = extract_inventory_citations(&prefixes, "// SOMEIP_ETS_BASICS_01\n");
 assert_eq!(out, vec![(1, "SOMEIP_ETS_BASICS_01".to_string())]);
 }

 #[test]
 fn extract_inventory_citations_word_boundary_rejects_alphanumeric_prev() {
 // `MY_ARP_07` should NOT match ARP_ prefix — the prefix is not on a
 // word boundary.
 let prefixes = vec!["ARP_".to_string()];
 let out = extract_inventory_citations(&prefixes, "// MY_ARP_07 internal\n");
 assert!(out.is_empty(), "expected no match, got: {:?}", out);
 }

 #[test]
 fn extract_inventory_citations_empty_prefixes_disables_axis() {
 let out = extract_inventory_citations(&[], "// ARP_07 cite\n");
 assert!(out.is_empty());
 }

 #[test]
 fn extract_inventory_citations_skips_backtick_codespan() {
 let prefixes = vec!["ARP_".to_string()];
 let out = extract_inventory_citations(&prefixes, "// example: `ARP_07` literal\n");
 assert!(out.is_empty(), "backtick span should suppress, got: {:?}", out);
 }

 #[test]
 fn scan_v2_inventory_missing_reject() {
 use crate::atomic::AtomicStore;
 let tmp = TempDir::new().unwrap();
 std::fs::create_dir_all(tmp.path().join("src")).unwrap();
 std::fs::write(tmp.path().join("src/foo.rs"), "// ARP_07 not in store\n").unwrap();
 let store = AtomicStore::new();
 let prefixes = vec!["ARP_".to_string()];
 let v = scan_paths_bidirectional_v2(
 tmp.path(),
 &["src/".to_string()],
 "Round ",
 &store,
 &[],
 None,
 true,
 &prefixes,
 )
 .unwrap();
 assert_eq!(v.len(), 1, "got: {:?}", v);
 match &v[0] {
 CodeRefViolation::Citation { kind, citation } => {
  assert!(matches!(kind, ViolationKind::InventoryMissing));
  assert_eq!(citation.entry_id, "ARP_07");
 }
 other => panic!("expected Citation, got {:?}", other),
 }
 }

 #[test]
 fn scan_v2_inventory_deprecated_reject() {
 use crate::atomic::{AtomicStore, InventoryEntry, InventoryStatus};
 let tmp = TempDir::new().unwrap();
 std::fs::create_dir_all(tmp.path().join("src")).unwrap();
 std::fs::write(tmp.path().join("src/foo.rs"), "// ARP_07 cite\n").unwrap();
 let mut store = AtomicStore::new();
 store.inventory_entries.insert(
 "ARP_07".to_string(),
 InventoryEntry {
  status: InventoryStatus::Deprecated,
  section_ref: None,
  source: None,
  reason: Some("superseded".to_string()),
 },
 );
 let prefixes = vec!["ARP_".to_string()];
 let v = scan_paths_bidirectional_v2(
 tmp.path(),
 &["src/".to_string()],
 "Round ",
 &store,
 &[],
 None,
 true,
 &prefixes,
 )
 .unwrap();
 assert_eq!(v.len(), 1, "got: {:?}", v);
 match &v[0] {
 CodeRefViolation::Citation { kind, .. } => {
  assert!(matches!(kind, ViolationKind::InventoryDeprecated));
 }
 other => panic!("expected Citation, got {:?}", other),
 }
 }

 #[test]
 fn scan_v2_inventory_active_and_reserved_silent() {
 use crate::atomic::{AtomicStore, InventoryEntry, InventoryStatus};
 let tmp = TempDir::new().unwrap();
 std::fs::create_dir_all(tmp.path().join("src")).unwrap();
 std::fs::write(
 tmp.path().join("src/foo.rs"),
 "// ARP_07 active\n// ARP_08 reserved\n",
 )
 .unwrap();
 let mut store = AtomicStore::new();
 store.inventory_entries.insert(
 "ARP_07".to_string(),
 InventoryEntry {
  status: InventoryStatus::Active,
  ..Default::default()
 },
 );
 store.inventory_entries.insert(
 "ARP_08".to_string(),
 InventoryEntry {
  status: InventoryStatus::Reserved,
  ..Default::default()
 },
 );
 let prefixes = vec!["ARP_".to_string()];
 let v = scan_paths_bidirectional_v2(
 tmp.path(),
 &["src/".to_string()],
 "Round ",
 &store,
 &[],
 None,
 true,
 &prefixes,
 )
 .unwrap();
 assert!(
 v.is_empty(),
 "Active and Reserved must be cite-permitted, got: {:?}",
 v
 );
 }

 // ============================================================================
 // Round 277 — External-standard §<id> skip tests (Phase 1A P1).
 // ============================================================================

 #[test]
 fn extract_v2_skips_rfc_external_cite() {
 let prefixes = vec!["RFC".to_string()];
 let out =
 extract_section_citations_v2("// RFC 2131 §3.5 is external\n", &prefixes);
 assert!(
 out.is_empty(),
 "RFC <num> §<id> must be skipped, got: {:?}",
 out
 );
 }

 #[test]
 fn extract_v2_skips_ieee_external_cite() {
 let prefixes = vec!["IEEE".to_string()];
 let out =
 extract_section_citations_v2("// IEEE 802.3 §2.4 frame format\n", &prefixes);
 assert!(out.is_empty(), "IEEE skip failed, got: {:?}", out);
 }

 #[test]
 fn extract_v2_skips_iso_iec_external_cite() {
 // ISO/IEC contains `/` and is itself a single non-whitespace token
 // — the v1 single-token rule handles it natively.
 let prefixes = vec!["ISO/IEC".to_string()];
 let out =
 extract_section_citations_v2("// ISO/IEC 14882 §1.5\n", &prefixes);
 assert!(out.is_empty(), "ISO/IEC skip failed, got: {:?}", out);
 }

 #[test]
 fn extract_v2_keeps_internal_when_no_external_context() {
 let prefixes = vec!["RFC".to_string(), "IEEE".to_string()];
 let out = extract_section_citations_v2("// §4.2.4 internal cite\n", &prefixes);
 assert_eq!(out, vec![(1, "4.2.4".to_string())]);
 }

 #[test]
 fn extract_v2_empty_prefixes_matches_v1_behavior() {
 // v1 wrapper delegates with empty prefixes; the two paths must
 // yield identical output for the same input.
 let v1 = extract_section_citations("// RFC 2131 §3.5 and §4.2.4 mixed\n");
 let v2 = extract_section_citations_v2("// RFC 2131 §3.5 and §4.2.4 mixed\n", &[]);
 assert_eq!(v1, v2);
 // Both retain the (incorrect-from-v2-perspective) RFC §.
 assert!(v1.iter().any(|(_, id)| id == "3.5"));
 assert!(v1.iter().any(|(_, id)| id == "4.2.4"));
 }

 #[test]
 fn extract_v2_requires_whitespace_between_numeric_and_sigil() {
 // `RFC2131§3` (no whitespace) is NOT the recognized form — falls
 // through to the regular extractor. Source uses `\u{00a7}` so the
 // fixture string itself doesn't show up as a `§3` citation when
 // the self-application scan walks `code_refs.rs`.
 let prefixes = vec!["RFC".to_string()];
 let out = extract_section_citations_v2("// RFC2131\u{00a7}3 inline form\n", &prefixes);
 assert_eq!(out, vec![(1, "3".to_string())]);
 }

 // Round 281 Bug #5A — surrounding punctuation must not block the
 // external-prefix verbatim match. Comment prose commonly wraps the
 // standard reference in parens / brackets / quotes.

 #[test]
 fn extract_v2_skips_paren_prefixed_rfc() {
 let prefixes = vec!["RFC".to_string()];
 let out = extract_section_citations_v2(
 "// fragmentation fields (RFC 791 \u{00a7}3.1) per spec\n",
 &prefixes,
 );
 assert!(
 out.is_empty(),
 "(RFC 791) form must be skipped; got: {:?}",
 out
 );
 }

 #[test]
 fn extract_v2_skips_bracket_prefixed_rfc() {
 let prefixes = vec!["RFC".to_string()];
 let out = extract_section_citations_v2(
 "// see [RFC 793 \u{00a7}3.9] for retransmit semantics\n",
 &prefixes,
 );
 assert!(out.is_empty(), "[RFC 793] form must be skipped; got: {:?}", out);
 }

 #[test]
 fn extract_v2_skips_quote_prefixed_rfc() {
 let prefixes = vec!["RFC".to_string()];
 let out = extract_section_citations_v2(
 "// per \"RFC 2131 \u{00a7}3.4\" the client retransmits\n",
 &prefixes,
 );
 assert!(out.is_empty(), "\"RFC 2131\" form must be skipped; got: {:?}", out);
 }

 #[test]
 fn extract_v2_bare_rfc_form_still_skipped() {
 // Regression for the original Round 277 form — punctuation strip must
 // not regress the bare-token case.
 let prefixes = vec!["RFC".to_string()];
 let out = extract_section_citations_v2(
 "// RFC 2131 \u{00a7}3.5 client behavior\n",
 &prefixes,
 );
 assert!(out.is_empty(), "bare RFC form must stay skipped; got: {:?}", out);
 }

 #[test]
 fn is_external_section_cite_strips_leading_punctuation() {
 let prefixes = vec!["RFC".to_string()];
 // Unit-level coverage of the prev_token cleanse (numeric mode).
 assert!(is_external_section_cite("(RFC 791 ", &prefixes, &[]));
 assert!(is_external_section_cite("[RFC 793 ", &prefixes, &[]));
 assert!(is_external_section_cite("\"RFC 2131 ", &prefixes, &[]));
 assert!(is_external_section_cite("«RFC 826 ", &prefixes, &[]));
 assert!(is_external_section_cite("RFC 3927 ", &prefixes, &[]));
 // Negative: random suffix on the prefix word should still miss.
 assert!(!is_external_section_cite("RFCs 791 ", &prefixes, &[]));
 }

 // Round 284 — bare-prefix (doc-name) mode tests. AUTOSAR family
 // (TR_SOMEIP / SOMEIPSD / SWS_SD) lacks a numeric document number,
 // so the prefix sits directly before the sigil: `<PREFIX> §<id>`.

 #[test]
 fn extract_v3_skips_bare_tr_someip() {
 let bare = vec!["TR_SOMEIP".to_string()];
 let out = extract_section_citations_v3(
 "// drives a Nack with TTL=0 (TR_SOMEIP \u{00a7}6.7.4.2.4).\n",
 &[],
 &bare,
 );
 assert!(out.is_empty(), "TR_SOMEIP bare form must skip; got: {:?}", out);
 }

 #[test]
 fn extract_v3_skips_bare_someipsd() {
 let bare = vec!["SOMEIPSD".to_string()];
 let out = extract_section_citations_v3(
 "// multicast reply per SOMEIPSD \u{00a7}6.7.5.2 path\n",
 &[],
 &bare,
 );
 assert!(out.is_empty(), "SOMEIPSD bare form must skip; got: {:?}", out);
 }

 #[test]
 fn extract_v3_skips_paren_wrapped_bare_prefix() {
 // R281 leading-punct strip applies in bare mode too.
 let bare = vec!["AUTOSAR".to_string()];
 let out = extract_section_citations_v3(
 "// wire format (AUTOSAR \u{00a7}7.3) over UDP\n",
 &[],
 &bare,
 );
 assert!(
 out.is_empty(),
 "(AUTOSAR §X) form must skip in bare mode; got: {:?}",
 out
 );
 }

 #[test]
 fn extract_v3_bare_mode_negative_unregistered_prefix() {
 // Internal §X.Y must surface when the preceding word is not in
 // the bare-prefix registry.
 let bare = vec!["TR_SOMEIP".to_string()];
 let out = extract_section_citations_v3(
 "// see FOO \u{00a7}4.2.4 internal cite\n",
 &[],
 &bare,
 );
 assert_eq!(out, vec![(1, "4.2.4".to_string())]);
 }

 #[test]
 fn extract_v3_numeric_and_bare_axes_independent() {
 // `RFC 791 §3.1` (numeric) + `TR_SOMEIP §6.7.4.2.4` (bare) on the
 // same line, both registered in their respective axes → both skip.
 let numeric = vec!["RFC".to_string()];
 let bare = vec!["TR_SOMEIP".to_string()];
 let out = extract_section_citations_v3(
 "// RFC 791 \u{00a7}3.1 and TR_SOMEIP \u{00a7}6.7.4.2.4 both\n",
 &numeric,
 &bare,
 );
 assert!(out.is_empty(), "both forms must skip; got: {:?}", out);
 }

 #[test]
 fn extract_v3_numeric_mode_unaffected_by_bare_registration() {
 // R277 / R281 regression: numeric path keeps working when only the
 // numeric axis is registered; an empty bare slice must not change
 // semantics for the numeric path.
 let numeric = vec!["RFC".to_string()];
 let out =
 extract_section_citations_v3("// RFC 2131 \u{00a7}3.5 client\n", &numeric, &[]);
 assert!(out.is_empty(), "numeric RFC path must keep working; got: {:?}", out);
 }

 #[test]
 fn is_external_section_cite_bare_mode_strips_leading_punctuation() {
 let bare = vec!["TR_SOMEIP".to_string()];
 // Unit-level coverage of the bare-mode strip + verbatim match.
 assert!(is_external_section_cite("// (TR_SOMEIP ", &[], &bare));
 assert!(is_external_section_cite("// [TR_SOMEIP ", &[], &bare));
 assert!(is_external_section_cite("per TR_SOMEIP ", &[], &bare));
 // Negative: unregistered word.
 assert!(!is_external_section_cite("// FOO ", &[], &bare));
 // Negative: numeric mode trigger with empty numeric axis.
 assert!(!is_external_section_cite("RFC 791 ", &[], &bare));
 }

 #[test]
 fn scan_v4_bare_external_skips_section_missing() {
 use crate::atomic::AtomicStore;
 let tmp = TempDir::new().unwrap();
 std::fs::create_dir_all(tmp.path().join("src")).unwrap();
 std::fs::write(
 tmp.path().join("src/foo.rs"),
 "// drives Nack (TR_SOMEIP \u{00a7}6.7.4.2.4) per spec\n",
 )
 .unwrap();
 let store = AtomicStore::new();
 let bare = vec!["TR_SOMEIP".to_string()];
 let v = scan_paths_bidirectional_v4(
 tmp.path(),
 &["src/".to_string()],
 "Round ",
 &store,
 &[],
 None,
 true,
 &[],
 &[],
 &bare,
 )
 .unwrap();
 assert!(
 v.is_empty(),
 "bare-mode TR_SOMEIP cite must be skipped; got: {:?}",
 v
 );
 }

 #[test]
 fn extract_v2_mixed_internal_and_external_on_same_line() {
 let prefixes = vec!["RFC".to_string()];
 let out = extract_section_citations_v2(
 "// see RFC 2131 §3.5 and §4.2.4 here\n",
 &prefixes,
 );
 assert_eq!(out, vec![(1, "4.2.4".to_string())]);
 }

 #[test]
 fn scan_v3_external_rfc_cite_does_not_trigger_section_missing() {
 use crate::atomic::AtomicStore;
 let tmp = TempDir::new().unwrap();
 std::fs::create_dir_all(tmp.path().join("src")).unwrap();
 std::fs::write(
 tmp.path().join("src/foo.rs"),
 "// RFC 2131 §3.5 external — should NOT fire SectionMissing\n",
 )
 .unwrap();
 let store = AtomicStore::new();
 let externals = vec!["RFC".to_string()];
 let v = scan_paths_bidirectional_v3(
 tmp.path(),
 &["src/".to_string()],
 "Round ",
 &store,
 &[],
 None,
 true,
 &[],
 &externals,
 )
 .unwrap();
 assert!(
 v.is_empty(),
 "RFC external cite must be skipped, got: {:?}",
 v
 );
 }

 #[test]
 fn scan_v3_internal_cite_still_fires_after_external_skip() {
 use crate::atomic::AtomicStore;
 let tmp = TempDir::new().unwrap();
 std::fs::create_dir_all(tmp.path().join("src")).unwrap();
 // `\u{00a7}` avoids the literal sigil in this source file (self-
 // scan would otherwise see the fixture as an unrelated cite).
 std::fs::write(
 tmp.path().join("src/foo.rs"),
 "// RFC 2131 \u{00a7}3.5 ok; \u{00a7}99 missing\n",
 )
 .unwrap();
 let store = AtomicStore::new();
 let externals = vec!["RFC".to_string()];
 let v = scan_paths_bidirectional_v3(
 tmp.path(),
 &["src/".to_string()],
 "Round ",
 &store,
 &[],
 None,
 true,
 &[],
 &externals,
 )
 .unwrap();
 // Only the internal `\u{00a7}99` should surface.
 assert_eq!(v.len(), 1, "got: {:?}", v);
 match &v[0] {
 CodeRefViolation::Citation { kind, citation } => {
  assert!(matches!(kind, ViolationKind::SectionMissing));
  assert!(citation.entry_id.contains("99"));
 }
 other => panic!("expected Citation, got {:?}", other),
 }
 }

 #[test]
 fn scan_inventory_decay_surfaces_only_target_id() {
 let tmp = TempDir::new().unwrap();
 std::fs::create_dir_all(tmp.path().join("src")).unwrap();
 std::fs::write(
 tmp.path().join("src/a.rs"),
 "// ARP_07 target\n// ARP_08 other\n",
 )
 .unwrap();
 let prefixes = vec!["ARP_".to_string()];
 let hits = scan_inventory_decay(
 tmp.path(),
 &["src/".to_string()],
 "ARP_07",
 &prefixes,
 true,
 )
 .unwrap();
 assert_eq!(hits.len(), 1);
 assert_eq!(hits[0].entry_id, "ARP_07");
 assert_eq!(hits[0].line, 1);
 }

 #[test]
 fn scan_inventory_decay_empty_prefixes_yields_no_hits() {
 // Axis-disabled (empty prefixes) is a no-op regardless of file content.
 let tmp = TempDir::new().unwrap();
 std::fs::create_dir_all(tmp.path().join("src")).unwrap();
 std::fs::write(tmp.path().join("src/a.rs"), "// ARP_07 cite\n").unwrap();
 let hits = scan_inventory_decay(
 tmp.path(),
 &["src/".to_string()],
 "ARP_07",
 &[],
 true,
 )
 .unwrap();
 assert!(hits.is_empty());
 }

 #[test]
 fn scan_inventory_decay_respects_comment_only_flag() {
 // String literal cite must be suppressed under comment_only=true.
 let tmp = TempDir::new().unwrap();
 std::fs::create_dir_all(tmp.path().join("src")).unwrap();
 std::fs::write(
 tmp.path().join("src/a.rs"),
 "let s = \"ARP_07 inside string\";\n// ARP_07 in comment\n",
 )
 .unwrap();
 let prefixes = vec!["ARP_".to_string()];
 let hits = scan_inventory_decay(
 tmp.path(),
 &["src/".to_string()],
 "ARP_07",
 &prefixes,
 true,
 )
 .unwrap();
 assert_eq!(hits.len(), 1);
 assert_eq!(hits[0].line, 2);
 }

 #[test]
 fn scan_v1_wrapper_disables_inventory_axis() {
 // The pre-Round-275 7-arg shape calls into v2 with an empty
 // inventory_prefixes slice. Even when the store has Deprecated
 // entries, no violation surfaces — back-compat guarantee.
 use crate::atomic::{AtomicStore, InventoryEntry, InventoryStatus};
 let tmp = TempDir::new().unwrap();
 std::fs::create_dir_all(tmp.path().join("src")).unwrap();
 std::fs::write(tmp.path().join("src/foo.rs"), "// ARP_07 cite\n").unwrap();
 let mut store = AtomicStore::new();
 store.inventory_entries.insert(
 "ARP_07".to_string(),
 InventoryEntry {
  status: InventoryStatus::Deprecated,
  ..Default::default()
 },
 );
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
 assert!(v.is_empty(), "v1 wrapper must not scan inventory, got: {:?}", v);
 }
}
