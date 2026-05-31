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

use mnemosyne_config::{OrphanKind, OrphanLedgerEntry, SetEqualityValidatorConfig};
use mnemosyne_core::DecisionStatus;

/// One `Round NNN` / `§<id>` citation candidate extracted from a source
/// file. `entry_id` retains the cite shape verbatim (`""` or
/// `""` — `§` prefix kept so the kind axis is readable from the id
/// alone).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
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
                ViolationKind::SymbolMismatch => "symbol_mismatch",
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
                ViolationKind::CitationUnbound | ViolationKind::SymbolMismatch => {
                    DefectClass::Binding
                }
                ViolationKind::Decay => DefectClass::Decay,
                ViolationKind::InventoryMissing | ViolationKind::InventoryDeprecated => {
                    DefectClass::Inventory
                }
            },
            CodeRefViolation::ImplementationUnbacked { .. } => DefectClass::Binding,
            CodeRefViolation::ImplementationMissing { .. } => DefectClass::Binding,
        }
    }

    /// Render the violation as a flat JSON object — the shape
    /// `mnemosyne-cli validate-code-refs --json` emits per violation:
    /// `{"kind": <tag>, "file": <path>, "line": <n>, "section_id": <id>,
    /// "entry_id": <id>, "symbol": <name>, "decision_status": <status>}`,
    /// with optional fields omitted when absent. The default Serialize
    /// derive on `CodeRefViolation` produces a nested
    /// variant-tagged form intended for the `ErasedValidator` dispatch
    /// boundary; this method is the CLI-stable flat shape.
    pub fn to_cli_json(&self) -> serde_json::Value {
        use serde_json::{Map, Value};
        let mut obj = Map::new();
        let kind_tag = self.kind_tag();
        obj.insert("kind".into(), Value::String(kind_tag.into()));
        match self {
            CodeRefViolation::Citation { citation, .. } => {
                obj.insert(
                    "file".into(),
                    Value::String(citation.file.to_string_lossy().into_owned()),
                );
                obj.insert("line".into(), Value::Number(citation.line.into()));
                obj.insert("entry_id".into(), Value::String(citation.entry_id.clone()));
            }
            CodeRefViolation::ImplementationUnbacked {
                section_id,
                file,
                symbol,
            } => {
                obj.insert("section_id".into(), Value::String(section_id.clone()));
                obj.insert(
                    "file".into(),
                    Value::String(file.to_string_lossy().into_owned()),
                );
                if let Some(s) = symbol {
                    obj.insert("symbol".into(), Value::String(s.clone()));
                }
            }
            CodeRefViolation::ImplementationMissing {
                section_id,
                decision_status,
            } => {
                obj.insert("section_id".into(), Value::String(section_id.clone()));
                let status_str = match decision_status {
                    Some(s) => format!("{:?}", s).to_lowercase(),
                    None => "none(default-active)".into(),
                };
                obj.insert("decision_status".into(), Value::String(status_str));
            }
        }
        Value::Object(obj)
    }
}

impl std::fmt::Display for CodeRefViolation {
    /// Render the human-readable CLI line for one violation. Format
    /// mirrors the legacy `violation_to_finding` message output:
    /// `[<kind>] <file>:<line> <entry_id>` for citations,
    /// `[<kind>] <file>:<no-cite> §<section_id> (<symbol>)` for
    /// implementation-unbacked, and `[<kind>] §<section_id>
    /// (status=<status>)` for implementation-missing.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind_tag = self.kind_tag();
        match self {
            CodeRefViolation::Citation { citation, .. } => write!(
                f,
                "[{}] {}:{} {}",
                kind_tag,
                citation.file.to_string_lossy(),
                citation.line,
                citation.entry_id
            ),
            CodeRefViolation::ImplementationUnbacked {
                section_id,
                file,
                symbol,
            } => write!(
                f,
                "[{}] {}:<no-cite> §{}{}",
                kind_tag,
                file.to_string_lossy(),
                section_id,
                symbol
                    .as_deref()
                    .map(|s| format!(" ({})", s))
                    .unwrap_or_default()
            ),
            CodeRefViolation::ImplementationMissing {
                section_id,
                decision_status,
            } => {
                let status_str = match decision_status {
                    Some(s) => format!("{:?}", s).to_lowercase(),
                    None => "none(default-active)".into(),
                };
                write!(f, "[{}] §{} (status={})", kind_tag, section_id, status_str)
            }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
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
    /// Round 306 — RFC-002 FR-3 symbol-level enforcement.
    ///
    /// At a `§<id>` citation site (`file`:`line` carrying the cite), the
    /// `SymbolResolver` plugin's `resolve_symbol_at(file, line)` returns a
    /// name that is NOT a member of the set of `Implementation.symbol`
    /// values the cited section records for the citing file. A section may
    /// be implemented by several symbols in one file, so the registered
    /// symbols form a set and the cite is bound iff its enclosing symbol is
    /// one of them. The binding exists at file granularity (R260) but no
    /// registered symbol covers this line — code drifted under the spec's
    /// claim, or the symbol set is stale.
    SymbolMismatch,
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
                // Advance past the matched char by its full UTF-8 width, never
                // a hardcoded +1: a non-ASCII `entry_id_prefix` puts `i` at a
                // multibyte boundary, and `i + 1` would land mid-codepoint so
                // the next `line[start..]` slice panics (same class as the
                // Round 279 Bug #1 fix in extract_inventory_citations_with_tail).
                let advance = line[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
                start = i + advance;
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
/// `§<id>` extractor with two external-standard skip axes:
/// *numeric* (RFC / IEEE / ISO/IEC, `<PREFIX> <NUMERIC> §<id>`) via
/// `external_prefixes_numeric` and *bare* (AUTOSAR family,
/// `<PREFIX> §<id>` without numeric) via `external_prefixes_bare`.
///
/// The two axes are independent — same prefix may appear in both if the
/// standard supports both forms; matching tries the axis that applies
/// based on the shape of the token preceding `§`.
///
/// Empty slices = the corresponding axis disabled. Both empty = no
/// external skip, every `§<id>` is treated as internal to this
/// workspace's atomic store.
///
/// Round 380 — external context also propagates across two citation
/// scopes wider than a single immediately-preceding prefix: (c) same-line
/// chains (`<prefix> §A / §B / §C` — cites after the first inherit when
/// separated only by `/` or whitespace; a comma or word breaks the chain)
/// and (d) comment-block wraps (a sigil that is the first content on its
/// line inherits when the previous comment line *ends with* the prefix,
/// e.g. `/// WAI-ARIA 1.2` then `/// §6.6.6`). Both still require a
/// registered prefix verbatim, so a citation never skips without one.
pub fn extract_section_citations(
    content: &str,
    external_prefixes_numeric: &[String],
    external_prefixes_bare: &[String],
) -> Vec<(usize, String)> {
    let external_enabled =
        !external_prefixes_numeric.is_empty() || !external_prefixes_bare.is_empty();
    let mut out = Vec::new();
    // R380 — previous physical line, for the comment-block-wrap carry (d).
    // In comment-only mode `strip_to_comments` preserves line numbers (code
    // lines become spaces), so this is the previous *comment* line whenever
    // the carry could legitimately fire.
    let mut prev_line = "";
    for (line_idx, line) in content.lines().enumerate() {
        // — single-line backtick state. `` inside a code-span
        // is documentation example, not a citation. Toggled on each backtick
        // and reset at line end (multi-line fenced code spans are not
        // recognized; the comment-only stripper already gates this for
        // most source files, and inline backtick spans cover the doc-comment
        // example case that survives stripping).
        let mut in_backtick = false;
        // R380 — line-local chain state for `<prefix> §A / §B / §C` (c).
        let mut chain_external = false;
        let mut last_cite_end = 0usize;
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
            // `` (fractional id) remains intact. Parsed first (before the
            // external verdict) so the chain bookkeeping (c) always has the
            // cite's byte extent.
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
            let cite_end = tail_start + end;

            // External-standard verdict — three context paths (R277/284 +
            // R380), all still gated on a verbatim-registered prefix:
            //  - direct: `<prefix>` immediately precedes the sigil (R277/284)
            //  - chained (c): the previous same-line cite was external and
            //    only chain separators (`/`, whitespace) sit between
            //  - carried (d): the sigil is the first content on its line and
            //    the previous comment line ends with the prefix (wrapped)
            let is_external = external_enabled
                && (is_external_section_cite(
                    &line[..i],
                    external_prefixes_numeric,
                    external_prefixes_bare,
                ) || (chain_external && gap_is_chain_only(&line[last_cite_end..i]))
                    || (line_prose_is_marker_only(&line[..i])
                        && prev_line_ends_with_prefix(
                            prev_line,
                            external_prefixes_numeric,
                            external_prefixes_bare,
                        )));

            // skip metavariable placeholders like `§N`, `§X`,
            // `§Y` used in doc-comments to mean "any section id". A real
            // section_id is either multi-char or starts with lowercase /
            // digit; a single uppercase letter is metasyntax.
            let is_metavar = id.chars().count() == 1
                && id
                    .chars()
                    .next()
                    .map(|c| c.is_ascii_uppercase())
                    .unwrap_or(false);
            if !is_metavar && !is_external {
                out.push((line_idx + 1, id));
            }
            // A metavar carries no external context forward; an internal
            // cite breaks the chain; an external cite continues it.
            chain_external = is_external && !is_metavar;
            last_cite_end = cite_end;

            // Advance the outer iterator past what we consumed.
            // (peekable / char_indices doesn't have skip-to-byte, so we
            // re-seek by consuming until we pass `cite_end`.)
            while let Some(&(k, _)) = chars.peek() {
                if k < cite_end {
                    chars.next();
                } else {
                    break;
                }
            }
        }
        prev_line = line;
    }
    out
}

/// Round 380 — is `gap` (the text between two same-line `§` cites) made of
/// only chain separators? `/` and whitespace chain (`§A / §B`, `§A/§B`);
/// a comma, word, or any other char breaks the chain so a distinct cite
/// after `, ` / ` and ` is still validated as internal.
fn gap_is_chain_only(gap: &str) -> bool {
    !gap.is_empty() && gap.chars().all(|c| c.is_whitespace() || c == '/')
}

/// Round 380 — is the text before a `§` only a comment marker (leading
/// whitespace + a run of `/` or `#`) with no prose? Such a sigil is the
/// first content on its line and may be a wrapped-citation continuation.
fn line_prose_is_marker_only(before: &str) -> bool {
    before
        .trim_start()
        .trim_start_matches(['/', '#'])
        .trim()
        .is_empty()
}

/// Round 380 — does the previous comment line *end with* an external
/// prefix (so a wrapped `/// <prefix>\n/// §id` continuation inherits it)?
/// Reuses [`is_external_section_cite`] by appending a space, so the same
/// numeric/bare/multi-word matching applies; only fires when the prefix is
/// the literal trailing content of the line.
fn prev_line_ends_with_prefix(
    prev_line: &str,
    prefixes_numeric: &[String],
    prefixes_bare: &[String],
) -> bool {
    if prev_line.trim().is_empty() {
        return false;
    }
    let mut ctx = String::with_capacity(prev_line.len() + 1);
    ctx.push_str(prev_line);
    ctx.push(' ');
    is_external_section_cite(&ctx, prefixes_numeric, prefixes_bare)
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
/// Round 379 — prefixes may be multi-word (`"CSS Color"`, `"Unicode
/// Standard"`): the prose before the document-number (numeric mode) or
/// before the sigil (bare mode) is matched against each registered
/// prefix as a token-boundary *suffix*. Document-number tokens may carry
/// a leading `#` (`UAX #9`) or trailing letters (`802.11ax`).
/// Byte offset of the start of the last whitespace-delimited token in `s`.
/// Splits on the last Unicode-whitespace char and advances past its full
/// UTF-8 width (not a hardcoded +1): `char::is_whitespace` matches multibyte
/// whitespace (U+00A0, U+2028, …), so `rfind(..).map(|i| i + 1)` could land
/// mid-codepoint and panic the following slice. Returns 0 when `s` has no
/// whitespace.
fn last_whitespace_token_start(s: &str) -> usize {
    s.char_indices()
        .rev()
        .find(|(_, c)| c.is_whitespace())
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0)
}

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
    let last_token_start = last_whitespace_token_start(trimmed);
    let last_token = &trimmed[last_token_start..];
    if last_token.is_empty() {
        return false;
    }
    if is_document_number_token(last_token) {
        // Numeric mode (R277, widened R379). The document-number token may
        // carry a leading `#` (`UAX #9`) or trailing letters (`802.11ax`);
        // the prose *before* it must end with a registered numeric prefix
        // (which may itself be multi-word, e.g. `CSS Color`).
        if prefixes_numeric.is_empty() {
            return false;
        }
        let before_num = trimmed[..last_token_start].trim_end();
        if before_num.is_empty() {
            return false;
        }
        prose_ends_with_prefix(before_num, prefixes_numeric)
    } else {
        // Bare mode (R284, widened R379). The prose must end with a
        // registered bare prefix (which may be multi-word, e.g.
        // `Unicode Standard`).
        if prefixes_bare.is_empty() {
            return false;
        }
        prose_ends_with_prefix(trimmed, prefixes_bare)
    }
}

/// Round 379 — does `tok` look like a standard's *document-number* token?
///
/// Accepts an optional leading `#` (Unicode Annex form `UAX #9`), then
/// requires a leading ASCII digit and an all-alphanumeric-or-dot body
/// (`791`, `802.3`, `1.2`, `9`, `802.11ax`). Rejects names (`Color`,
/// `Standard`) and hyphenated tokens (`WAI-ARIA`), which select bare mode.
fn is_document_number_token(tok: &str) -> bool {
    let body = tok.strip_prefix('#').unwrap_or(tok);
    let mut chars = body.chars();
    if !matches!(chars.next(), Some(c) if c.is_ascii_digit()) {
        return false;
    }
    body.chars().all(|c| c.is_ascii_alphanumeric() || c == '.')
}

/// Round 379 — does `prose` end with one of `prefixes` on a token
/// boundary? A prefix may be multi-word (`"CSS Color"`, `"Unicode
/// Standard"`); the char before the match must be a non-alphanumeric
/// boundary, so `(RFC` / `[RFC` / a leading whitespace all match but
/// `FOORFC` does not. Verbatim suffix match — no domain knowledge, the
/// engine never learns what any prefix means.
fn prose_ends_with_prefix(prose: &str, prefixes: &[String]) -> bool {
    let prose = prose.trim();
    for p in prefixes {
        if p.is_empty() || !prose.ends_with(p.as_str()) {
            continue;
        }
        let idx = prose.len() - p.len();
        if !prose.is_char_boundary(idx) {
            continue;
        }
        let boundary_ok = idx == 0
            || !prose[..idx]
                .chars()
                .next_back()
                .map(|c| c.is_alphanumeric())
                .unwrap_or(false);
        if boundary_ok {
            return true;
        }
    }
    false
}

/// The namespace segment of a `§<id>` citation: the part before the first
/// `-`, or the whole id when it has no `-`. Pure, offline, no domain
/// knowledge — the engine never learns what any particular namespace means.
///
/// `scxml-6.4` → `scxml` · `mesh-16.7` → `mesh` · `D` → `D` ·
/// `scxml-D-interpret` → `scxml`. Used by the workspace `section_namespace`
/// scope to decide whether a citation falls under this ledger's jurisdiction.
fn citation_namespace(section_id: &str) -> &str {
    // `split_once` makes the no-hyphen case explicit (the whole id is its own
    // namespace) instead of an unreachable `split('-').next()` fallback.
    section_id
        .split_once('-')
        .map_or(section_id, |(namespace, _)| namespace)
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
pub fn extract_inventory_citations(prefixes: &[String], content: &str) -> Vec<(usize, String)> {
    extract_inventory_citations_with_tail(prefixes, content, InventoryTailMode::IdToken)
}

/// Extract *section-path-shaped* inventory citations.
///
/// Companion axis to [`extract_inventory_citations`] for external-spec
/// mirror adopters (W3C SCXML, IETF RFC, IEEE, AUTOSAR, …) whose
/// citation tail uses section-path characters (`A-Za-z0-9./-_`) instead
/// of the opaque-ID shape (`[A-Z0-9_]+ ending in digit`). Token form:
/// `<prefix><tail>` where `<tail>` matches `[A-Za-z0-9./-_]+` with no
/// digit-terminus requirement — `3.13`, `test144`, `D.2.selectTransitions`
/// all match.
///
/// Word-boundary, backtick-skip, longest-prefix-first ordering, and
/// dedup semantics are identical to [`extract_inventory_citations`].
/// Returns empty when `prefixes.is_empty()` (axis disabled).
///
/// Use case: an adopter mirroring W3C SCXML registers
/// `inventory_path_prefixes = ["W3C SCXML "]` and a W3C SCXML section
/// like `3.13` gets registered as `InventoryEntry { id = "W3C SCXML
/// 3.13", … }` in the atomic store. Citations of the form
/// `// W3C SCXML 3.13` in code resolve against the inventory axis
/// without forcing a mass cite migration to the sigil-prefixed form.
pub fn extract_inventory_path_citations(
    prefixes: &[String],
    content: &str,
) -> Vec<(usize, String)> {
    extract_inventory_citations_with_tail(prefixes, content, InventoryTailMode::SectionPath)
}

/// Inventory citation tail shape — distinguishes opaque-ID citations
/// from section-path identifiers. Internal to the extractor; callers
/// pick the public function (`extract_inventory_citations` vs
/// `extract_inventory_path_citations`) and the corresponding mode is
/// applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InventoryTailMode {
    /// `[A-Z0-9_]+` with tail ending in a digit. Targets opaque
    /// inventory IDs (`ARP_07`, `TCP_RETRANSMISSION_TO_04`).
    IdToken,
    /// `[A-Za-z0-9./-_]+` with no digit-terminus requirement. Targets
    /// section paths (`3.13`, `test144`, `D.2.selectTransitions`).
    SectionPath,
}

fn extract_inventory_citations_with_tail(
    prefixes: &[String],
    content: &str,
    tail_mode: InventoryTailMode,
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
                // tail char class differs per mode:
                //   IdToken    → [A-Z0-9_]+ (uppercase, digits, underscore)
                //   SectionPath → [A-Za-z0-9./-_]+ (alnum + . / - _; mirrors
                //                 `is_section_id_char` used by the section-citation axis)
                let tail_bytes = &bytes[tail_start..];
                let mut t = 0usize;
                while t < tail_bytes.len() {
                    let c = tail_bytes[t];
                    let is_tail = match tail_mode {
                        InventoryTailMode::IdToken => {
                            c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_'
                        }
                        InventoryTailMode::SectionPath => {
                            c.is_ascii_alphanumeric()
                                || c == b'.'
                                || c == b'/'
                                || c == b'-'
                                || c == b'_'
                        }
                    };
                    if is_tail {
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
                // IdToken mode: tail must end in a digit (TC8 / ISO test-spec
                // convention; suppresses identifier-shaped false positives).
                // SectionPath mode: no digit-terminus — section paths can end
                // in a letter (`D.2.selectTransitions`) or a digit (`3.13`).
                if tail_mode == InventoryTailMode::IdToken && !tail_bytes[t - 1].is_ascii_digit() {
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
                // an ASCII byte (tails in both modes are ASCII by design).
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
// `[plugins.set_equality_validator] comment_only = false` restores the whole-text scan.
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
        "rs" | "c" | "h" | "cc" | "cpp" | "cxx" | "hpp" | "hxx" | "hh" | "go" | "js" | "ts"
        | "jsx" | "tsx" | "mjs" | "cjs" | "java" | "scala" | "kt" | "kts" | "swift" => {
            CommentSyntax::Slash
        }
        "py" | "sh" | "bash" | "zsh" | "rb" | "toml" | "yaml" | "yml" => CommentSyntax::Hash,
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
                for (_, rest) in chars.by_ref() {
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
                for (_, rest) in chars.by_ref() {
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
/// Scanner with all four cite axes wired in:
///
/// 1. `Round NNN` axis — `<entry_id_prefix><number>` (decay-aware via
///    `filter_id`).
/// 2. `§<id>` axis with two external-standard skip modes —
///    *numeric* (`<PREFIX> <NUMERIC> §<id>`) via
///    `external_section_prefixes_numeric` and *bare*
///    (`<PREFIX> §<id>` doc-name only) via `external_section_prefixes_bare`.
/// 3. Inventory axis with two tail shapes — *opaque-ID*
///    (`<prefix><[A-Z0-9_]+ ending in digit>`) via `inventory_prefixes`
///    and *section-path* (`<prefix><[A-Za-z0-9./-_]+>`) via
///    `inventory_path_prefixes`. Both feed the same `InventoryEntry`
///    store and share `severity_inventory`.
/// 4. Bidirectional set-equality (Path B) — `§X.implementations` files
///    vs cited-by sets — surfaces `CitationUnbound`,
///    `ImplementationUnbacked`, and `ImplementationMissing` (R269
///    coverage axiom).
///
/// `orphan_ledger` rows with `kind = CodeCitation` suppress
/// section-citation-axis violations and rows with `kind =
/// InventoryCitation` (R285) suppress inventory-axis violations.
///
/// Pass an empty slice on any axis to disable it. `filter_id` is the
/// decay-scan toggle (Steps 3-4 stay silent under decay mode for
/// surface-narrowing).
/// Map a file path to the language ID used as the
/// `[plugins.symbol_resolver.<lang>]` key. Round 306 — wires
/// `SymbolResolver` plugins per file extension. Unknown extensions
/// return `None`; the symbol axis is silently skipped for that file.
fn lang_for_file(path: &Path) -> Option<&'static str> {
    match path.extension().and_then(|e| e.to_str()) {
        Some("rs") => Some("rust"),
        Some("py") => Some("python"),
        Some("go") => Some("go"),
        Some("h" | "hh" | "hpp" | "hxx" | "cpp" | "cc" | "cxx") => Some("cpp"),
        _ => None,
    }
}

/// First-class Validator plugin embodying the set-equality citation
/// audit. Routes through `PluginRegistry` so the validator-class trait
/// surface is reached from production code (`cmd_validate_code_refs`
/// constructs, registers, and dispatches), closing R306 carry item #1.
///
/// Field rationale:
/// - `config` — paths / severity / comment_only / inventory + external
///   prefix axes (in-place from `SetEqualityValidatorConfig`).
/// - `entry_id_prefix` — schema-driven (`<entry_id_prefix><number>`
///   cite shape). Cached at construction so `Validator::validate` does
///   not re-discover from `ValidationContext`.
/// - `orphan_ledger` — workspace-config-driven `[[orphan_ledger]]` rows.
/// - `symbol_resolvers` — BindingClass plugin map keyed by language ID
///   (`rust`/`python`/`go`). Owned (not registry-borrowed) so
///   `Validator::validate` is self-contained — no registry parameter on
///   `ValidationContext`. Empty map = symbol axis disabled.
/// - `filter_id` — decay-cascade caller's per-instance toggle. `None`
///   for normal runs; `Some(<entry_id>)` for cascade-mode callers
///   narrowing to one entry's decay scan.
pub struct SetEqualityValidator {
    pub config: SetEqualityValidatorConfig,
    pub entry_id_prefix: String,
    pub orphan_ledger: Vec<OrphanLedgerEntry>,
    pub symbol_resolvers: BTreeMap<String, Box<dyn mnemosyne_core::SymbolResolver>>,
    pub filter_id: Option<String>,
}

impl SetEqualityValidator {
    /// Rich scan returning `CodeRefViolation`. The plugin trait method
    /// `validate(ctx)` calls into this and maps each variant to a
    /// `ValidationFinding` for cross-plugin dispatch; direct callers
    /// (the decay-cascade trigger after a Superseded transition) keep
    /// the structured shape.
    ///
    /// Algorithm: Round NNN axis + §<id> axis with two external-skip
    /// modes + Inventory axis with two tail shapes + bidirectional
    /// set-equality (Path B) + spec-side coverage axiom. See
    /// [`CodeRefViolation`] doc for per-variant evidence.
    pub fn scan(
        &self,
        workspace_root: &Path,
        snapshot: &mnemosyne_core::AtomicSnapshot,
    ) -> std::io::Result<Vec<CodeRefViolation>> {
        let prefix = self.entry_id_prefix.as_str();
        let filter_id = self.filter_id.as_deref();
        let comment_only = self.config.comment_only;
        let inventory_prefixes = self.config.inventory_prefixes.as_slice();
        let external_section_prefixes_numeric = self.config.external_section_prefixes.as_slice();
        let external_section_prefixes_bare = self.config.external_section_prefixes_bare.as_slice();
        let inventory_path_prefixes = self.config.inventory_path_prefixes.as_slice();
        let section_namespace = self.config.section_namespace.as_deref();
        // Empty resolver map = symbol axis silently skipped; identical
        // semantic to the pre-R307 `Option<&BTreeMap>` shape where None
        // bypassed lookup entirely.
        let symbol_resolvers_opt = if self.symbol_resolvers.is_empty() {
            None
        } else {
            Some(&self.symbol_resolvers)
        };
        let paths = self.config.paths.as_slice();
        let orphan_ledger = self.orphan_ledger.as_slice();

        // valid_entry_ids must match the shape produced by `extract_citations`,
        // which returns `<prefix><number>` (e.g. "Round 293"). Atomic ledger
        // keys are either short-form ("Round 292") or long-form
        // ("Round 293 — <title>"); both get normalized to `<prefix><number>`
        // by stripping prefix + re-running `scan_round_number`. Keys without
        // the prefix cannot collide with the cited shape and are skipped.
        let valid_entry_ids: BTreeSet<String> = snapshot
            .changelog_entry_ids
            .iter()
            .filter_map(|k| {
                let rest = k.strip_prefix(prefix)?;
                let num = scan_round_number(rest)?;
                Some(format!("{}{}", prefix, num))
            })
            .collect();
        let section_id_set = &snapshot.section_ids_with_implied_parents;

        // Pre-index §X.implementations by section_id for O(log n) per-cite
        // membership check + step 3 universe enumeration.
        let impl_files_by_section: BTreeMap<&str, BTreeSet<&str>> = snapshot
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

        // RFC-002 FR-3 symbol-level enforcement index — section_id → file →
        // {symbols} (every Implementation.symbol that is Some). A section is
        // legitimately realized by more than one symbol in a file (e.g. a
        // typed-throw contract spread across parse entry points), so the
        // index is set-valued: a cite is bound at symbol granularity iff its
        // resolved enclosing symbol is a MEMBER of the registered set. Drives
        // SymbolMismatch where the file IS bound (R260) but no registered
        // symbol covers the cited line.
        let impl_symbols_by_section_file: BTreeMap<&str, BTreeMap<&str, BTreeSet<&str>>> = snapshot
            .sections
            .iter()
            .map(|(sid, sec)| {
                let mut m: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
                for i in &sec.implementations {
                    if let Some(s) = i.symbol.as_deref() {
                        m.entry(i.file.as_str()).or_default().insert(s);
                    }
                }
                (sid.as_str(), m)
            })
            .collect();

        // Orphan ledger lookup: (file, id) pairs explicitly registered as
        // known-stale code citations on the `§`-axis vs the inventory axis.
        // Independent indices so `CodeCitation` rows don't suppress inventory
        // violations and `InventoryCitation` rows don't suppress `§`-axis.
        let ledger_index: BTreeSet<(&str, &str)> = orphan_ledger
            .iter()
            .filter(|e| e.kind == OrphanKind::CodeCitation)
            .map(|e| (e.from.as_str(), e.to.as_str()))
            .collect();
        let inventory_ledger_index: BTreeSet<(&str, &str)> = orphan_ledger
            .iter()
            .filter(|e| e.kind == OrphanKind::InventoryCitation)
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
            for (line, section_id) in extract_section_citations(
                &content,
                external_section_prefixes_numeric,
                external_section_prefixes_bare,
            ) {
                // Namespace scope — when this workspace declares a
                // `section_namespace`, a citation whose namespace segment
                // (the part before the first `-`) is not exactly that value
                // belongs to a different ledger. Skip it entirely: no
                // SectionMissing, no `cited_by` binding record (step 3 must
                // not treat a foreign cite as this workspace's binding).
                if let Some(ns) = section_namespace {
                    if citation_namespace(&section_id) != ns {
                        continue;
                    }
                }
                // Ledger suppression — if (file, id) is explicitly registered as a
                // known-stale code citation, treat as if the binding were correct
                // (record in `cited_by` so step 3 doesn't double-fire).
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
                // §<id>.implementations files). Matching is by `file` string only;
                // symbol is opaque metadata not in the bidirectional set-equality.
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
                } else if let Some(resolvers) = symbol_resolvers_opt {
                    // RFC-002 FR-3 symbol-level enforcement. File-level binding
                    // passed; if the cited section records any `symbol` for this
                    // file, the resolver for the file's language is consulted and
                    // the resolved enclosing symbol must be a member of that set.
                    // A non-member surfaces as SymbolMismatch (Binding-class).
                    // Resolver returning None/Err is silent.
                    if let Some(expected_syms) = impl_symbols_by_section_file
                        .get(section_id.as_str())
                        .and_then(|m| m.get(rel_str.as_str()))
                    {
                        if let Some(lang) = lang_for_file(&rel) {
                            if let Some(resolver) = resolvers.get(lang) {
                                let abs_for_resolve = workspace_root.join(&rel);
                                if let Ok(Some(resolved)) =
                                    resolver.resolve_symbol_at(&abs_for_resolve, line as u32)
                                {
                                    if !expected_syms.contains(resolved.as_str()) {
                                        violations.push(CodeRefViolation::Citation {
                                            citation: Citation {
                                                file: rel.clone(),
                                                line,
                                                entry_id: format!("§{}", section_id),
                                            },
                                            kind: ViolationKind::SymbolMismatch,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ---- Inventory ID axis (Phase 1A) ----
            // Active / Reserved → silent; Deprecated → InventoryDeprecated;
            // missing IDs → InventoryMissing. `[[orphan_ledger]] kind =
            // InventoryCitation` suppresses both. Chain section-path
            // inventory axis (`inventory_path_prefixes`); dedup on (line, id)
            // so a prefix registered in both axes surfaces once.
            let mut inventory_cites = extract_inventory_citations(inventory_prefixes, &content);
            inventory_cites.extend(extract_inventory_path_citations(
                inventory_path_prefixes,
                &content,
            ));
            inventory_cites.sort();
            inventory_cites.dedup();
            for (line, inventory_id) in inventory_cites {
                let kind = match snapshot.inventory.get(&inventory_id).copied() {
                    None => Some(ViolationKind::InventoryMissing),
                    Some(mnemosyne_core::InventoryStatus::Deprecated) => {
                        Some(ViolationKind::InventoryDeprecated)
                    }
                    // Active / Reserved — cite-permitted.
                    Some(_) => None,
                };
                if let Some(k) = kind {
                    if inventory_ledger_index.contains(&(rel_str.as_str(), inventory_id.as_str())) {
                        continue;
                    }
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
            for (section_id, section) in &snapshot.sections {
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

        // ---- Step 4: spec-side coverage axiom ----
        // Workspace-wide: a section with non-Removed decision_status and
        // zero implementations is the "Active = backed by code" axiom
        // violation. Removed is tombstone-exempt. None → Active fallback
        // used for the trigger only; the raw Option is preserved on the
        // emitted variant (carried as schema DecisionStatus for back-compat
        // with `CodeRefViolation::ImplementationMissing`'s shape).
        if filter_id.is_none() {
            for (section_id, section) in &snapshot.sections {
                if !section.implementations.is_empty() {
                    continue;
                }
                // R309 textbook unification: SectionView.decision_status now IS
                // DecisionStatus (canonical, lifted to mnemosyne-core). Step 4
                // axiom + emitted ImplementationMissing variant share the same enum
                // — no adapter layer.
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
}

impl mnemosyne_core::Validator for SetEqualityValidator {
    type Finding = CodeRefViolation;

    fn version_surface(&self) -> mnemosyne_core::VersionSurface {
        mnemosyne_core::VersionSurface {
            plugin_name: "mnemosyne-validate::SetEqualityValidator".into(),
            plugin_version: env!("CARGO_PKG_VERSION").into(),
            schema_min: 4,
            schema_max: 4,
        }
    }

    fn validate(
        &self,
        ctx: &mnemosyne_core::ValidationContext<'_>,
    ) -> Result<Vec<CodeRefViolation>, mnemosyne_core::ValidatorError> {
        let snapshot = ctx.store.snapshot();
        self.scan(ctx.workspace_root, &snapshot)
            .map_err(|e| mnemosyne_core::ValidatorError::Internal(e.to_string()))
    }
}

/// Round 266 — auto-cascade trigger primitive (Stage B freshness).
///
/// Targeted decay scan for §<section_id> citations of *one* section,
/// returned as a flat list of [`Citation`]. Used by the mutate-time hook
/// in `set-section-decision-status` CLI: when a section transitions
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
        for (line, sid) in extract_section_citations(&content, &[], &[]) {
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
///
/// Decay scan covers both inventory axes: opaque-ID via
/// `inventory_prefixes` and section-path via `inventory_path_prefixes`.
/// Cascade trigger calls this after an `InventoryEntry` transitions to
/// a status that needs cite-side notification, so a path-shape ID
/// rename / deprecation surfaces its cite-sites too. An empty slice
/// disables the corresponding axis.
pub fn scan_inventory_decay(
    workspace_root: &Path,
    paths: &[String],
    inventory_id: &str,
    inventory_prefixes: &[String],
    inventory_path_prefixes: &[String],
    comment_only: bool,
) -> std::io::Result<Vec<Citation>> {
    if inventory_prefixes.is_empty() && inventory_path_prefixes.is_empty() {
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
        // Chain opaque-ID + section-path axes; dedup on (line, id) so a
        // prefix registered in both axes surfaces once.
        let mut cites = extract_inventory_citations(inventory_prefixes, &content);
        cites.extend(extract_inventory_path_citations(
            inventory_path_prefixes,
            &content,
        ));
        cites.sort();
        cites.dedup();
        for (line, id) in cites {
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
fn sort_violations(violations: &mut [CodeRefViolation]) {
    violations.sort_by(|a, b| {
        use std::cmp::Ordering;
        use CodeRefViolation::*;
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
    use mnemosyne_atomic::{add_section_implementation, AtomicStore};
    use tempfile::TempDir;

    /// Test-only wrapper that drives `SetEqualityValidator::scan` with no
    /// SymbolResolver registry — i.e., pre-R306 set-equality-only mode.
    /// Tests that specifically exercise R306 symbol-axis enforcement
    /// construct a `SetEqualityValidator` directly with a populated
    /// `symbol_resolvers` map.
    #[allow(clippy::too_many_arguments)]
    fn scan_paths_no_resolvers(
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
        inventory_path_prefixes: &[String],
    ) -> std::io::Result<Vec<CodeRefViolation>> {
        // The common case carries no namespace scope; `_ns` is the single
        // implementation, so the existing call sites stay untouched.
        scan_paths_no_resolvers_ns(
            workspace_root,
            paths,
            prefix,
            store,
            orphan_ledger,
            filter_id,
            comment_only,
            inventory_prefixes,
            external_section_prefixes_numeric,
            external_section_prefixes_bare,
            inventory_path_prefixes,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn scan_paths_no_resolvers_ns(
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
        inventory_path_prefixes: &[String],
        section_namespace: Option<&str>,
    ) -> std::io::Result<Vec<CodeRefViolation>> {
        use mnemosyne_core::AtomicStoreView;
        let validator = SetEqualityValidator {
            config: SetEqualityValidatorConfig {
                paths: paths.to_vec(),
                severity_missing: "reject".into(),
                severity_binding: "reject".into(),
                severity_inventory: "reject".into(),
                comment_only,
                inventory_prefixes: inventory_prefixes.to_vec(),
                external_section_prefixes: external_section_prefixes_numeric.to_vec(),
                external_section_prefixes_bare: external_section_prefixes_bare.to_vec(),
                inventory_path_prefixes: inventory_path_prefixes.to_vec(),
                section_namespace: section_namespace.map(String::from),
            },
            entry_id_prefix: prefix.to_string(),
            orphan_ledger: orphan_ledger.to_vec(),
            symbol_resolvers: BTreeMap::new(),
            filter_id: filter_id.map(String::from),
        };
        let snapshot = store.snapshot();
        validator.scan(workspace_root, &snapshot)
    }

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
            vec![(1, "Round 254".to_string()), (2, "Round 33.5".to_string())]
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
            vec![(1, "ADR-0042".to_string()), (1, "ADR-7".to_string())]
        );
    }

    #[test]
    fn extract_citations_empty_prefix_yields_empty() {
        assert!(extract_citations("", "Round 254\n").is_empty());
    }

    #[test]
    fn extract_citations_non_ascii_prefix_no_panic() {
        // A non-ASCII `entry_id_prefix` (no config rule forbids one) puts the
        // match offset `i` on a multibyte boundary. When the prefix is
        // preceded by an alphanumeric (word-boundary reject), the old
        // `start = i + 1` advance landed mid-codepoint and the next slice
        // panicked. The first occurrence is a clean citation; the second is
        // glued to `x` and must be skipped — without panicking.
        let src = "라운드 254 and x라운드 7\n";
        let out = extract_citations("라운드 ", src);
        assert_eq!(out, vec![(1, "라운드 254".to_string())]);
    }

    #[test]
    fn is_external_section_cite_numeric_multibyte_whitespace_no_panic() {
        // U+2028 LINE SEPARATOR is Unicode whitespace (3 bytes). The old
        // `rfind(char::is_whitespace).map(|i| i + 1)` landed mid-codepoint and
        // panicked. The token after it ("791") is numeric and "RFC" precedes
        // it, so the numeric axis must still match across the multibyte gap.
        let prefixes = vec!["RFC".to_string()];
        assert!(is_external_section_cite("RFC\u{2028}791 ", &prefixes, &[]));
    }

    #[test]
    fn is_external_section_cite_bare_multibyte_whitespace_no_panic() {
        // U+00A0 NO-BREAK SPACE is Unicode whitespace (2 bytes). The bare axis
        // splits on the last whitespace to isolate the trailing token; the
        // advance must clear the full multibyte width, not +1.
        let bare = vec!["TR_SOMEIP".to_string()];
        assert!(is_external_section_cite("x\u{00A0}TR_SOMEIP ", &[], &bare));
    }

    // ============ §<id> extractor unit tests ============

    #[test]
    fn extract_section_citations_basic_numeric() {
        let src = "// §39 carry\n// also §61 for context\n";
        let out = extract_section_citations(src, &[], &[]);
        assert_eq!(out, vec![(1, "39".to_string()), (2, "61".to_string())]);
    }

    #[test]
    fn extract_section_citations_fractional_id() {
        let src = "// see §61.1 for sub-section\n";
        let out = extract_section_citations(src, &[], &[]);
        assert_eq!(out, vec![(1, "61.1".to_string())]);
    }

    #[test]
    fn extract_section_citations_slash_slug() {
        let src = "// §atomic-store/changelog-atomic-ledger anchor\n";
        let out = extract_section_citations(src, &[], &[]);
        assert_eq!(
            out,
            vec![(1, "atomic-store/changelog-atomic-ledger".to_string())]
        );
    }

    #[test]
    fn extract_section_citations_trailing_dot_not_consumed() {
        let src = "End of sentence §39. Next line\n";
        let out = extract_section_citations(src, &[], &[]);
        assert_eq!(out, vec![(1, "39".to_string())]);
    }

    #[test]
    fn extract_section_citations_brackets_and_parens() {
        let src = "(§39) [§61.1] {§atomic-store}\n";
        let out = extract_section_citations(src, &[], &[]);
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
        let out = extract_section_citations(src, &[], &[]);
        assert!(out.is_empty());
    }

    #[test]
    fn extract_section_citations_underscore_allowed() {
        let src = "// §atomic_store snake case slug\n";
        let out = extract_section_citations(src, &[], &[]);
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
        // Round 287 fail-loud: seed Section before add_section_implementation
        // (test fixture path — direct insert bypasses audit-receipt overhead).
        store.sections.insert(
            section_id.to_string(),
            mnemosyne_atomic::AtomicSection::default(),
        );
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
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &[],
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
        std::fs::write(tmp.path().join("src/foo.rs"), "// see §999 hallucinated\n").unwrap();
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &[],
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
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &[],
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
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &[],
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
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &ledger,
            None,
            true,
            &[],
            &[],
            &[],
            &[],
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
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &ledger,
            None,
            true,
            &[],
            &[],
            &[],
            &[],
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
            mnemosyne_atomic::AtomicChangelogEntry::default(),
        );
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &s2,
            &[],
            Some("Round 1"),
            true,
            &[],
            &[],
            &[],
            &[],
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
        let hits = scan_section_decay(tmp.path(), &["src/".to_string()], "39", true).unwrap();
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
        let hits = scan_section_decay(tmp.path(), &["src/".to_string()], "39", true).unwrap();
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
        assert_eq!(
            comment_hits.len(),
            1,
            "comment_only excludes string literal"
        );
        assert_eq!(comment_hits[0].line, 2);
        let raw_hits = scan_section_decay(tmp.path(), &["src/".to_string()], "39", false).unwrap();
        assert_eq!(raw_hits.len(), 2, "comment_only=false picks up both");
    }

    // ============ comment-only filtering tests ============

    #[test]
    fn comment_syntax_dispatch_by_extension() {
        use std::path::PathBuf;
        // Slash family.
        for ext in [
            "rs", "c", "h", "cc", "cpp", "hpp", "go", "js", "ts", "jsx", "tsx", "java", "kt",
            "swift",
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
        assert_eq!(
            comment_syntax_for(&PathBuf::from("a")),
            CommentSyntax::Unknown
        );
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
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &[],
        )
        .unwrap();
        // Only one Missing (the line 2 comment); line 1 string literal suppressed.
        let missing: Vec<_> = v
            .iter()
            .filter(|x| {
                matches!(
                    x,
                    CodeRefViolation::Citation {
                        kind: ViolationKind::Missing,
                        ..
                    }
                )
            })
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
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            false,
            &[],
            &[],
            &[],
            &[],
        )
        .unwrap();
        // Whole-text scan picks up BOTH occurrences (line 1 and line 2).
        let missing: Vec<_> = v
            .iter()
            .filter(|x| {
                matches!(
                    x,
                    CodeRefViolation::Citation {
                        kind: ViolationKind::Missing,
                        ..
                    }
                )
            })
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
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &[],
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
        // Round 287 fail-loud: explicit Section creation via direct insert
        // (test fixture path — no audit-receipt needed).
        store.sections.insert(
            section_id.to_string(),
            mnemosyne_atomic::AtomicSection {
                skeleton: mnemosyne_core::SectionSkeleton {
                    decision_status,
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        // implementations stays at Vec::default() = []
        store
    }

    #[test]
    fn coverage_axiom_active_empty_impls_triggers() {
        let tmp = TempDir::new().unwrap();
        let store = build_store_with_empty_section("39", Some(DecisionStatus::Active));
        // No source files written — workspace is otherwise silent.
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &[],
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
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &[],
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
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &[],
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
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &[],
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
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert!(
            v.iter()
                .all(|x| !matches!(x, CodeRefViolation::ImplementationMissing { .. })),
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
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            Some("Round 99"),
            true,
            &[],
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert!(
            v.is_empty(),
            "filter_id should silence coverage axiom, got: {:?}",
            v
        );
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
    fn scan_survives_non_ascii_comment_chars() {
        // Round 279 Bug #1 regression — full scan path (including
        // strip_to_comments) must not panic when a workspace source file
        // contains the original em-dash trigger from the tc8-harness
        // bug report.
        use mnemosyne_atomic::AtomicStore;
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        let content = format!(
            "// SERVICE-ID-2 (0xF4E8) target {} DUT offers FOO_01\n",
            '\u{2014}'
        );
        std::fs::write(tmp.path().join("src/x.rs"), content).unwrap();
        let store = AtomicStore::new();
        let prefixes = vec!["FOO_".to_string()];
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &prefixes,
            &[],
            &[],
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
        let out =
            extract_inventory_citations(&prefixes, "// ARP_07 and TCP_RETRANSMISSION_TO_04\n");
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
        assert!(
            out.is_empty(),
            "backtick span should suppress, got: {:?}",
            out
        );
    }

    // ============================================================================
    // Section-path inventory axis tests (RFC-002 FR-4 narrow ext).
    // ============================================================================

    #[test]
    fn extract_inventory_path_citations_w3c_scxml_dotted_numeric() {
        // The motivating case — W3C SCXML 3.13 (dotted-numeric tail) must
        // match an inventory_path_prefix of "W3C SCXML ".
        let prefixes = vec!["W3C SCXML ".to_string()];
        let out =
            extract_inventory_path_citations(&prefixes, "// see W3C SCXML 3.13 for <event>\n");
        assert_eq!(out, vec![(1, "W3C SCXML 3.13".to_string())]);
    }

    #[test]
    fn extract_inventory_path_citations_lowercase_tail() {
        // IRP test144 — lowercase alpha + digits, no underscore. R275
        // axis rejects this (uppercase-only); section-path axis accepts.
        let prefixes = vec!["IRP ".to_string()];
        let out = extract_inventory_path_citations(&prefixes, "// IRP test144 catalog\n");
        assert_eq!(out, vec![(1, "IRP test144".to_string())]);
    }

    #[test]
    fn extract_inventory_path_citations_alpha_terminus() {
        // Section paths can end in a letter (`D.2.selectTransitions` in
        // SCXML Appendix D) — no digit-terminus requirement under section-path mode.
        let prefixes = vec!["SCXML-".to_string()];
        let out = extract_inventory_path_citations(
            &prefixes,
            "// SCXML-D.2.selectTransitions algorithm\n",
        );
        assert_eq!(out, vec![(1, "SCXML-D.2.selectTransitions".to_string())]);
    }

    #[test]
    fn extract_inventory_path_citations_multi_prefix() {
        let prefixes = vec!["W3C SCXML ".to_string(), "IRP ".to_string()];
        let out = extract_inventory_path_citations(
            &prefixes,
            "// W3C SCXML 3.13 vs IRP test144 cross-ref\n",
        );
        assert_eq!(
            out,
            vec![
                (1, "IRP test144".to_string()),
                (1, "W3C SCXML 3.13".to_string()),
            ]
        );
    }

    #[test]
    fn extract_inventory_path_citations_word_boundary_rejects_alphanumeric_prev() {
        // `xW3C SCXML 3.13` should NOT match — prefix is not on a word
        // boundary (the preceding 'x' is alphanumeric).
        let prefixes = vec!["W3C SCXML ".to_string()];
        let out = extract_inventory_path_citations(&prefixes, "// xW3C SCXML 3.13 internal name\n");
        assert!(out.is_empty(), "expected no match, got: {:?}", out);
    }

    #[test]
    fn extract_inventory_path_citations_skips_backtick_codespan() {
        let prefixes = vec!["W3C SCXML ".to_string()];
        let out =
            extract_inventory_path_citations(&prefixes, "// example: `W3C SCXML 3.13` literal\n");
        assert!(
            out.is_empty(),
            "backtick span should suppress, got: {:?}",
            out
        );
    }

    #[test]
    fn extract_inventory_path_citations_longest_prefix_wins() {
        // Both `W3C ` and `W3C SCXML ` registered — the longer specific
        // prefix wins for "W3C SCXML 3.13".
        let prefixes = vec!["W3C ".to_string(), "W3C SCXML ".to_string()];
        let out = extract_inventory_path_citations(&prefixes, "// W3C SCXML 3.13\n");
        assert_eq!(
            out,
            vec![(1, "W3C SCXML 3.13".to_string())],
            "longer prefix must win"
        );
    }

    #[test]
    fn extract_inventory_path_citations_empty_prefixes_disables_axis() {
        let out = extract_inventory_path_citations(&[], "// W3C SCXML 3.13\n");
        assert!(out.is_empty());
    }

    #[test]
    fn extract_inventory_path_citations_no_id_token_axis_interference() {
        // The section-path axis axis must NOT swallow R275 opaque IDs — distinct tail
        // grammar even if the function were misused. Lowercase tail like
        // `arp_07` would not match R275 (uppercase-only) but would match
        // section-path axis if prefix is registered there. This test pins that section-path axis
        // does not auto-skip uppercase tails — `ARP_07` is still valid
        // under section-path mode because [A-Za-z0-9./-_] is a superset.
        let prefixes = vec!["ARP_".to_string()];
        let out = extract_inventory_path_citations(&prefixes, "// ARP_07 cite\n");
        assert_eq!(out, vec![(1, "ARP_07".to_string())]);
    }

    #[test]
    fn scan_section_path_inventory_missing() {
        // Full-scanner path: a path-shape cite (`W3C SCXML 3.13`) with
        // no matching atomic store entry must surface as InventoryMissing
        // via the section-path axis axis, not silently pass.
        use mnemosyne_atomic::AtomicStore;
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(
            tmp.path().join("src/foo.rs"),
            "// W3C SCXML 3.13 cited but not registered\n",
        )
        .unwrap();
        let store = AtomicStore::new();
        let path_prefixes = vec!["W3C SCXML ".to_string()];
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &path_prefixes,
        )
        .unwrap();
        assert_eq!(v.len(), 1, "got: {:?}", v);
        match &v[0] {
            CodeRefViolation::Citation { kind, citation } => {
                assert!(matches!(kind, ViolationKind::InventoryMissing));
                assert_eq!(citation.entry_id, "W3C SCXML 3.13");
            }
            other => panic!("expected Citation, got {:?}", other),
        }
    }

    #[test]
    fn scan_section_path_inventory_active_silent() {
        // Registered InventoryEntry with Active status — cite passes
        // silently on the section-path axis axis, same policy as R275.
        use mnemosyne_atomic::{AtomicStore, InventoryEntry};
        use mnemosyne_core::InventoryStatus;
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("src/foo.rs"), "// W3C SCXML 3.13 cite\n").unwrap();
        let mut store = AtomicStore::new();
        store.inventory_entries.insert(
            "W3C SCXML 3.13".to_string(),
            InventoryEntry {
                status: InventoryStatus::Active,
                section_ref: None,
                source: None,
                reason: None,
            },
        );
        let path_prefixes = vec!["W3C SCXML ".to_string()];
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &path_prefixes,
        )
        .unwrap();
        assert!(
            v.is_empty(),
            "Active section-path axis cite must pass silently, got: {:?}",
            v
        );
    }

    #[test]
    fn scan_both_inventory_axes_dedup() {
        // A prefix registered in BOTH axes (e.g., legacy `ARP_` carried
        // into section-path axis for migration reasons) must surface a matching cite
        // once, not twice. Dedup on (line, id).
        use mnemosyne_atomic::AtomicStore;
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("src/foo.rs"), "// ARP_07 cite\n").unwrap();
        let store = AtomicStore::new();
        let opaque = vec!["ARP_".to_string()];
        let path = vec!["ARP_".to_string()];
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &opaque,
            &[],
            &[],
            &path,
        )
        .unwrap();
        assert_eq!(
            v.len(),
            1,
            "ARP_07 in both axes must dedup to 1 InventoryMissing, got: {:?}",
            v
        );
    }

    #[test]
    fn scan_inventory_missing_reject() {
        use mnemosyne_atomic::AtomicStore;
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("src/foo.rs"), "// ARP_07 not in store\n").unwrap();
        let store = AtomicStore::new();
        let prefixes = vec!["ARP_".to_string()];
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &prefixes,
            &[],
            &[],
            &[],
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
    fn scan_inventory_deprecated_reject() {
        use mnemosyne_atomic::{AtomicStore, InventoryEntry};
        use mnemosyne_core::InventoryStatus;
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
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &prefixes,
            &[],
            &[],
            &[],
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
    fn scan_inventory_active_and_reserved_silent() {
        use mnemosyne_atomic::{AtomicStore, InventoryEntry};
        use mnemosyne_core::InventoryStatus;
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
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &prefixes,
            &[],
            &[],
            &[],
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
    fn extract_skips_rfc_external_cite() {
        let prefixes = vec!["RFC".to_string()];
        let out = extract_section_citations("// RFC 2131 §3.5 is external\n", &prefixes, &[]);
        assert!(
            out.is_empty(),
            "RFC <num> §<id> must be skipped, got: {:?}",
            out
        );
    }

    #[test]
    fn extract_skips_ieee_external_cite() {
        let prefixes = vec!["IEEE".to_string()];
        let out = extract_section_citations("// IEEE 802.3 §2.4 frame format\n", &prefixes, &[]);
        assert!(out.is_empty(), "IEEE skip failed, got: {:?}", out);
    }

    #[test]
    fn extract_skips_iso_iec_external_cite() {
        // ISO/IEC contains `/` and is itself a single non-whitespace token
        // — the single-token rule handles it natively.
        let prefixes = vec!["ISO/IEC".to_string()];
        let out = extract_section_citations("// ISO/IEC 14882 §1.5\n", &prefixes, &[]);
        assert!(out.is_empty(), "ISO/IEC skip failed, got: {:?}", out);
    }

    #[test]
    fn extract_keeps_internal_when_no_external_context() {
        let prefixes = vec!["RFC".to_string(), "IEEE".to_string()];
        let out = extract_section_citations("// §4.2.4 internal cite\n", &prefixes, &[]);
        assert_eq!(out, vec![(1, "4.2.4".to_string())]);
    }

    #[test]
    fn extract_section_citations_empty_external_prefixes_treats_all_as_internal() {
        // With both external-skip axes empty, every §<id> is treated as
        // internal — `RFC 2131 §3.5` does NOT skip; both 3.5 and 4.2.4
        // surface as internal citations.
        let out = extract_section_citations("// RFC 2131 §3.5 and §4.2.4 mixed\n", &[], &[]);
        assert!(out.iter().any(|(_, id)| id == "3.5"));
        assert!(out.iter().any(|(_, id)| id == "4.2.4"));
    }

    #[test]
    fn extract_requires_whitespace_between_numeric_and_sigil() {
        // `RFC2131§3` (no whitespace) is NOT the recognized form — falls
        // through to the regular extractor. Source uses `\u{00a7}` so the
        // fixture string itself doesn't show up as a `§3` citation when
        // the self-application scan walks `code_refs.rs`.
        let prefixes = vec!["RFC".to_string()];
        let out = extract_section_citations("// RFC2131\u{00a7}3 inline form\n", &prefixes, &[]);
        assert_eq!(out, vec![(1, "3".to_string())]);
    }

    // Round 281 Bug #5A — surrounding punctuation must not block the
    // external-prefix verbatim match. Comment prose commonly wraps the
    // standard reference in parens / brackets / quotes.

    #[test]
    fn extract_skips_paren_prefixed_rfc() {
        let prefixes = vec!["RFC".to_string()];
        let out = extract_section_citations(
            "// fragmentation fields (RFC 791 \u{00a7}3.1) per spec\n",
            &prefixes,
            &[],
        );
        assert!(
            out.is_empty(),
            "(RFC 791) form must be skipped; got: {:?}",
            out
        );
    }

    #[test]
    fn extract_skips_bracket_prefixed_rfc() {
        let prefixes = vec!["RFC".to_string()];
        let out = extract_section_citations(
            "// see [RFC 793 \u{00a7}3.9] for retransmit semantics\n",
            &prefixes,
            &[],
        );
        assert!(
            out.is_empty(),
            "[RFC 793] form must be skipped; got: {:?}",
            out
        );
    }

    #[test]
    fn extract_skips_quote_prefixed_rfc() {
        let prefixes = vec!["RFC".to_string()];
        let out = extract_section_citations(
            "// per \"RFC 2131 \u{00a7}3.4\" the client retransmits\n",
            &prefixes,
            &[],
        );
        assert!(
            out.is_empty(),
            "\"RFC 2131\" form must be skipped; got: {:?}",
            out
        );
    }

    #[test]
    fn extract_bare_rfc_form_still_skipped() {
        // Regression for the original Round 277 form — punctuation strip must
        // not regress the bare-token case.
        let prefixes = vec!["RFC".to_string()];
        let out =
            extract_section_citations("// RFC 2131 \u{00a7}3.5 client behavior\n", &prefixes, &[]);
        assert!(
            out.is_empty(),
            "bare RFC form must stay skipped; got: {:?}",
            out
        );
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
    fn extract_skips_bare_tr_someip() {
        let bare = vec!["TR_SOMEIP".to_string()];
        let out = extract_section_citations(
            "// drives a Nack with TTL=0 (TR_SOMEIP \u{00a7}6.7.4.2.4).\n",
            &[],
            &bare,
        );
        assert!(
            out.is_empty(),
            "TR_SOMEIP bare form must skip; got: {:?}",
            out
        );
    }

    #[test]
    fn extract_skips_bare_someipsd() {
        let bare = vec!["SOMEIPSD".to_string()];
        let out = extract_section_citations(
            "// multicast reply per SOMEIPSD \u{00a7}6.7.5.2 path\n",
            &[],
            &bare,
        );
        assert!(
            out.is_empty(),
            "SOMEIPSD bare form must skip; got: {:?}",
            out
        );
    }

    #[test]
    fn extract_skips_paren_wrapped_bare_prefix() {
        // R281 leading-punct strip applies in bare mode too.
        let bare = vec!["AUTOSAR".to_string()];
        let out = extract_section_citations(
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
    fn extract_bare_mode_negative_unregistered_prefix() {
        // Internal §X.Y must surface when the preceding word is not in
        // the bare-prefix registry.
        let bare = vec!["TR_SOMEIP".to_string()];
        let out = extract_section_citations("// see FOO \u{00a7}4.2.4 internal cite\n", &[], &bare);
        assert_eq!(out, vec![(1, "4.2.4".to_string())]);
    }

    #[test]
    fn extract_numeric_and_bare_axes_independent() {
        // `RFC 791 §3.1` (numeric) + `TR_SOMEIP §6.7.4.2.4` (bare) on the
        // same line, both registered in their respective axes → both skip.
        let numeric = vec!["RFC".to_string()];
        let bare = vec!["TR_SOMEIP".to_string()];
        let out = extract_section_citations(
            "// RFC 791 \u{00a7}3.1 and TR_SOMEIP \u{00a7}6.7.4.2.4 both\n",
            &numeric,
            &bare,
        );
        assert!(out.is_empty(), "both forms must skip; got: {:?}", out);
    }

    #[test]
    fn extract_numeric_mode_unaffected_by_bare_registration() {
        // R277 / R281 regression: numeric path keeps working when only the
        // numeric axis is registered; an empty bare slice must not change
        // semantics for the numeric path.
        let numeric = vec!["RFC".to_string()];
        let out = extract_section_citations("// RFC 2131 \u{00a7}3.5 client\n", &numeric, &[]);
        assert!(
            out.is_empty(),
            "numeric RFC path must keep working; got: {:?}",
            out
        );
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
    fn is_external_section_cite_hash_document_number_r379() {
        // R379 (a): a hash-prefixed document number (UAX #9, UAX #15)
        // selects numeric mode and reads UAX as the prefix.
        let numeric = vec!["UAX".to_string()];
        assert!(is_external_section_cite("// UAX #9 ", &numeric, &[]));
        assert!(is_external_section_cite("per UAX #15 ", &numeric, &[]));
        // Letter-suffixed document number (802.11ax) also classifies.
        let ieee = vec!["IEEE".to_string()];
        assert!(is_external_section_cite("IEEE 802.11ax ", &ieee, &[]));
        // Negative: a hash number with no registered prefix must not skip.
        assert!(!is_external_section_cite("// see #9 ", &numeric, &[]));
    }

    #[test]
    fn is_external_section_cite_multi_word_prefix_r379() {
        // R379 (b): multi-word prefixes match as a token-boundary suffix.
        let numeric = vec!["CSS Color".to_string()];
        assert!(is_external_section_cite("// CSS Color 4 ", &numeric, &[]));
        // Bare multi-word: Unicode Standard.
        let bare = vec!["Unicode Standard".to_string()];
        assert!(is_external_section_cite("// Unicode Standard ", &[], &bare));
        // Negative: a different leading word must not skip (no over-reach).
        assert!(!is_external_section_cite(
            "// random Color 4 ",
            &numeric,
            &[]
        ));
        // Negative: suffix must match on a token boundary (SCSS is not CSS).
        let css = vec!["CSS".to_string()];
        assert!(!is_external_section_cite("// SCSS 3 ", &css, &[]));
    }

    #[test]
    fn extract_skips_w3c_shapes_r379() {
        // End-to-end: UAX #9 and CSS Color 4 citations no longer surface
        // as internal once the prefix is registered; a bare internal cite
        // (5.16) still does.
        let numeric = vec!["UAX".to_string(), "CSS Color".to_string()];
        let content = "// UAX #9 \u{00a7}3.3.1 reorder\n// CSS Color 4 \u{00a7}8.1 oklch\n// \u{00a7}5.16 internal\n";
        let out = extract_section_citations(content, &numeric, &[]);
        assert_eq!(
            out,
            vec![(3usize, "5.16".to_string())],
            "only the internal cite should remain; got: {:?}",
            out
        );
    }

    #[test]
    fn extract_chains_multi_cite_same_line_r380() {
        // R380 (c): `UAX #9 §6.6.8 / §6.6.9 / §6.6.10` — only the first cite
        // carries the prefix; the rest inherit across `/` separators.
        let numeric = vec!["UAX".to_string()];
        let content = "// UAX #9 \u{00a7}6.6.8 / \u{00a7}6.6.9 / \u{00a7}6.6.10\n";
        let out = extract_section_citations(content, &numeric, &[]);
        assert!(
            out.is_empty(),
            "chained external cites must skip; got: {:?}",
            out
        );
    }

    #[test]
    fn extract_chain_breaks_on_comma_r380() {
        // R380 (c) over-skip guard: a comma is NOT a chain separator, so a
        // distinct internal cite after `, ` is still validated.
        let numeric = vec!["UAX".to_string()];
        let content = "// UAX #9 \u{00a7}3.3, \u{00a7}5.16 internal\n";
        let out = extract_section_citations(content, &numeric, &[]);
        assert_eq!(
            out,
            vec![(1usize, "5.16".to_string())],
            "comma must break the chain; got: {:?}",
            out
        );
    }

    #[test]
    fn extract_carries_wrapped_prefix_across_comment_lines_r380() {
        // R380 (d): `/// WAI-ARIA 1.2` then `/// §6.6.6` — the sigil is the
        // first content on its line and inherits the prior line's prefix.
        let numeric = vec!["WAI-ARIA".to_string()];
        let content = "/// WAI-ARIA 1.2\n/// \u{00a7}6.6.6\n";
        let out = extract_section_citations(content, &numeric, &[]);
        assert!(out.is_empty(), "wrapped prefix must carry; got: {:?}", out);
        // Composes with the chain: continuation line may itself chain.
        let chained = "/// WAI-ARIA 1.2\n/// \u{00a7}6.6.6 / \u{00a7}6.6.7\n";
        assert!(extract_section_citations(chained, &numeric, &[]).is_empty());
    }

    #[test]
    fn extract_wrap_carry_requires_prefix_at_line_tail_r380() {
        // R380 (d) over-skip guard #1: the previous line must *end with* the
        // prefix. Trailing prose after it ⇒ no carry, cite stays internal.
        let numeric = vec!["WAI-ARIA".to_string()];
        let content = "/// implements WAI-ARIA 1.2 fully\n/// \u{00a7}6.6.6\n";
        let out = extract_section_citations(content, &numeric, &[]);
        assert_eq!(out, vec![(2usize, "6.6.6".to_string())], "got: {:?}", out);
    }

    #[test]
    fn extract_wrap_carry_only_immediate_previous_line_r380() {
        // R380 (d) over-skip guard #2: only the immediately previous line
        // carries; an intervening prose line breaks it.
        let numeric = vec!["WAI-ARIA".to_string()];
        let content = "/// WAI-ARIA 1.2\n/// unrelated note\n/// \u{00a7}6.6.6\n";
        let out = extract_section_citations(content, &numeric, &[]);
        assert_eq!(out, vec![(3usize, "6.6.6".to_string())], "got: {:?}", out);
    }

    #[test]
    fn scan_bare_external_skips_section_missing() {
        use mnemosyne_atomic::AtomicStore;
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(
            tmp.path().join("src/foo.rs"),
            "// drives Nack (TR_SOMEIP \u{00a7}6.7.4.2.4) per spec\n",
        )
        .unwrap();
        let store = AtomicStore::new();
        let bare = vec!["TR_SOMEIP".to_string()];
        let v = scan_paths_no_resolvers(
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
            &[],
        )
        .unwrap();
        assert!(
            v.is_empty(),
            "bare-mode TR_SOMEIP cite must be skipped; got: {:?}",
            v
        );
    }

    #[test]
    fn extract_mixed_internal_and_external_on_same_line() {
        let prefixes = vec!["RFC".to_string()];
        let out =
            extract_section_citations("// see RFC 2131 §3.5 and §4.2.4 here\n", &prefixes, &[]);
        assert_eq!(out, vec![(1, "4.2.4".to_string())]);
    }

    #[test]
    fn scan_external_rfc_cite_does_not_trigger_section_missing() {
        use mnemosyne_atomic::AtomicStore;
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(
            tmp.path().join("src/foo.rs"),
            "// RFC 2131 §3.5 external — should NOT fire SectionMissing\n",
        )
        .unwrap();
        let store = AtomicStore::new();
        let externals = vec!["RFC".to_string()];
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &externals,
            &[],
            &[],
        )
        .unwrap();
        assert!(
            v.is_empty(),
            "RFC external cite must be skipped, got: {:?}",
            v
        );
    }

    #[test]
    fn scan_internal_cite_still_fires_after_external_skip() {
        use mnemosyne_atomic::AtomicStore;
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
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &externals,
            &[],
            &[],
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
            &[],
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
        let hits =
            scan_inventory_decay(tmp.path(), &["src/".to_string()], "ARP_07", &[], &[], true)
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
            &[],
            true,
        )
        .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].line, 2);
    }

    #[test]
    fn scan_empty_inventory_prefixes_disables_inventory_axis() {
        // An empty inventory_prefixes slice disables the inventory axis:
        // even when the store has Deprecated entries, no violation surfaces.
        use mnemosyne_atomic::{AtomicStore, InventoryEntry};
        use mnemosyne_core::InventoryStatus;
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
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert!(
            v.is_empty(),
            "empty inventory_prefixes must not scan inventory, got: {:?}",
            v
        );
    }

    // ============================================================================
    // Round 285 — inventory-axis orphan_ledger suppression tests.
    // ============================================================================

    #[test]
    fn inventory_orphan_ledger_suppresses_inventory_deprecated() {
        use mnemosyne_atomic::{AtomicStore, InventoryEntry};
        use mnemosyne_core::InventoryStatus;
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("src/foo.rs"), "// IPv4_OPTIONS_01 hist\n").unwrap();
        let mut store = AtomicStore::new();
        store.inventory_entries.insert(
            "IPv4_OPTIONS_01".to_string(),
            InventoryEntry {
                status: InventoryStatus::Deprecated,
                ..Default::default()
            },
        );
        let ledger = vec![OrphanLedgerEntry {
            kind: OrphanKind::InventoryCitation,
            doc: "<inventory-citation>".to_string(),
            from: "src/foo.rs".to_string(),
            to: "IPv4_OPTIONS_01".to_string(),
            reason: "Historical: V2->V3 deleted, dissector skips IP options".to_string(),
            since: "Round 285".to_string(),
        }];
        let prefixes = vec!["IPv4_".to_string()];
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &ledger,
            None,
            true,
            &prefixes,
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert!(
            v.is_empty(),
            "ledger must suppress Deprecated cite; got: {:?}",
            v
        );
    }

    #[test]
    fn inventory_orphan_ledger_suppresses_inventory_missing() {
        // Deleted-from-store case: id not registered at all, ledger still
        // suppresses (author's intentional historical reference).
        use mnemosyne_atomic::AtomicStore;
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("src/foo.rs"), "// IPv4_OPTIONS_01 hist\n").unwrap();
        let store = AtomicStore::new();
        let ledger = vec![OrphanLedgerEntry {
            kind: OrphanKind::InventoryCitation,
            doc: "<inventory-citation>".to_string(),
            from: "src/foo.rs".to_string(),
            to: "IPv4_OPTIONS_01".to_string(),
            reason: "Historical: id removed from inventory, comment retained".to_string(),
            since: "Round 285".to_string(),
        }];
        let prefixes = vec!["IPv4_".to_string()];
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &ledger,
            None,
            true,
            &prefixes,
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert!(
            v.is_empty(),
            "ledger must suppress Missing cite; got: {:?}",
            v
        );
    }

    #[test]
    fn inventory_orphan_ledger_unregistered_fires() {
        // (file, id) not in ledger → violation fires normally.
        use mnemosyne_atomic::{AtomicStore, InventoryEntry};
        use mnemosyne_core::InventoryStatus;
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("src/foo.rs"), "// IPv4_OPTIONS_02 cite\n").unwrap();
        let mut store = AtomicStore::new();
        store.inventory_entries.insert(
            "IPv4_OPTIONS_02".to_string(),
            InventoryEntry {
                status: InventoryStatus::Deprecated,
                ..Default::default()
            },
        );
        // Ledger only covers _01, not _02.
        let ledger = vec![OrphanLedgerEntry {
            kind: OrphanKind::InventoryCitation,
            doc: "<inventory-citation>".to_string(),
            from: "src/foo.rs".to_string(),
            to: "IPv4_OPTIONS_01".to_string(),
            reason: "Historical _01 only".to_string(),
            since: "Round 285".to_string(),
        }];
        let prefixes = vec!["IPv4_".to_string()];
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &ledger,
            None,
            true,
            &prefixes,
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert_eq!(v.len(), 1, "_02 must fire (not in ledger); got: {:?}", v);
        match &v[0] {
            CodeRefViolation::Citation { kind, .. } => {
                assert!(matches!(kind, ViolationKind::InventoryDeprecated));
            }
            other => panic!("expected Citation, got {:?}", other),
        }
    }

    #[test]
    fn inventory_orphan_ledger_axis_filter_isolates_kinds() {
        // CodeCitation ledger rows must NOT suppress inventory violations,
        // and vice-versa. Axes are independent.
        use mnemosyne_atomic::{AtomicStore, InventoryEntry};
        use mnemosyne_core::InventoryStatus;
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("src/foo.rs"), "// IPv4_OPTIONS_01 cite\n").unwrap();
        let mut store = AtomicStore::new();
        store.inventory_entries.insert(
            "IPv4_OPTIONS_01".to_string(),
            InventoryEntry {
                status: InventoryStatus::Deprecated,
                ..Default::default()
            },
        );
        // CodeCitation kind — should NOT suppress inventory cite.
        let ledger = vec![OrphanLedgerEntry {
            kind: OrphanKind::CodeCitation,
            doc: "<code-citation>".to_string(),
            from: "src/foo.rs".to_string(),
            to: "IPv4_OPTIONS_01".to_string(),
            reason: "wrong-axis row".to_string(),
            since: "Round 285".to_string(),
        }];
        let prefixes = vec!["IPv4_".to_string()];
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &ledger,
            None,
            true,
            &prefixes,
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert_eq!(
            v.len(),
            1,
            "CodeCitation row must not suppress inventory cite; got: {:?}",
            v
        );
    }

    // ============ Round 293 entry-id prefix-normalize ============

    #[test]
    fn long_form_entry_id_matches_short_form_citation() {
        // R293 trigger: entry-id stored as "Round 293 — <title>" must match
        // a code citation of the form "Round 293". Without the normalize step
        // the citation would be flagged Missing even though the round exists.
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("a.rs"), "// Round 293 carry\n").unwrap();
        let mut store = AtomicStore::new();
        store.changelog_entries.insert(
            "Round 293 — R291 backfill entry append + commit↔ledger drift gate".to_string(),
            mnemosyne_atomic::AtomicChangelogEntry::default(),
        );
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert!(
            v.is_empty(),
            "long-form entry-id must match Round 293 cite; got: {:?}",
            v
        );
    }

    #[test]
    fn short_form_entry_id_still_matches_after_normalize() {
        // Regression guard: most ledger entries are short-form ("Round 292").
        // The normalize step must not break direct equality matches.
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("a.rs"), "// Round 292 cite\n").unwrap();
        let mut store = AtomicStore::new();
        store.changelog_entries.insert(
            "Round 292".to_string(),
            mnemosyne_atomic::AtomicChangelogEntry::default(),
        );
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert!(
            v.is_empty(),
            "short-form entry-id must continue to match; got: {:?}",
            v
        );
    }

    #[test]
    fn unknown_round_still_flags_missing_after_normalize() {
        // Regression guard: normalize must not silence genuinely missing
        // citations. Cite a hallucinated round → Missing. The fixture content
        // is built via format!() rather than a string literal so the
        // production validate-code-refs scan over this very source file does
        // not pick up the synthetic round number as a real citation.
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        let cite = format!("// {} 9{} hallucinated\n", "Round", "99");
        std::fs::write(src.join("a.rs"), cite).unwrap();
        let mut store = AtomicStore::new();
        store.changelog_entries.insert(
            "Round 292".to_string(),
            mnemosyne_atomic::AtomicChangelogEntry::default(),
        );
        let v = scan_paths_no_resolvers(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &[],
        )
        .unwrap();
        assert_eq!(
            v.len(),
            1,
            "hallucinated round must still flag Missing; got: {:?}",
            v
        );
        match &v[0] {
            CodeRefViolation::Citation { citation, kind } => {
                assert_eq!(*kind, ViolationKind::Missing);
                assert_eq!(citation.entry_id, "Round 999");
            }
            other => panic!("unexpected variant: {:?}", other),
        }
    }

    // ============ section_namespace scope tests ============

    #[test]
    fn citation_namespace_segments() {
        assert_eq!(citation_namespace("scxml-6.4"), "scxml");
        assert_eq!(citation_namespace("mesh-16.7"), "mesh");
        assert_eq!(citation_namespace("scxml-D-interpret"), "scxml");
        // no hyphen → whole id is its own namespace segment
        assert_eq!(citation_namespace("D"), "D");
        assert_eq!(citation_namespace("39"), "39");
    }

    #[test]
    fn namespace_scopes_out_foreign_cite() {
        // A `mesh-16.7` cite under section_namespace="scxml" belongs to a
        // different ledger — skip it, no SectionMissing despite empty store.
        use mnemosyne_atomic::AtomicStore;
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(
            tmp.path().join("src/foo.rs"),
            "// dedup per \u{00a7}mesh-16.7 elsewhere\n",
        )
        .unwrap();
        let store = AtomicStore::new();
        let v = scan_paths_no_resolvers_ns(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &[],
            Some("scxml"),
        )
        .unwrap();
        assert!(
            v.is_empty(),
            "foreign-namespace cite must be skipped: {:?}",
            v
        );
    }

    #[test]
    fn namespace_keeps_matching_cite_in_scope() {
        // A `scxml-9.99` cite under section_namespace="scxml" is in scope, so
        // its absence from the (empty) store fires SectionMissing.
        use mnemosyne_atomic::AtomicStore;
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(
            tmp.path().join("src/foo.rs"),
            "// see \u{00a7}scxml-9.99 hallucinated\n",
        )
        .unwrap();
        let store = AtomicStore::new();
        let v = scan_paths_no_resolvers_ns(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &[],
            Some("scxml"),
        )
        .unwrap();
        assert_eq!(v.len(), 1, "in-namespace unknown id must fire: {:?}", v);
        match &v[0] {
            CodeRefViolation::Citation { kind, citation } => {
                assert_eq!(*kind, ViolationKind::SectionMissing);
                assert!(citation.entry_id.contains("scxml-9.99"));
            }
            other => panic!("unexpected variant: {:?}", other),
        }
    }

    #[test]
    fn namespace_unset_checks_every_cite() {
        // Back-compat: with no section_namespace, a `mesh-16.7` cite is
        // treated as internal and fires SectionMissing against the store.
        use mnemosyne_atomic::AtomicStore;
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(
            tmp.path().join("src/foo.rs"),
            "// dedup per \u{00a7}mesh-16.7 elsewhere\n",
        )
        .unwrap();
        let store = AtomicStore::new();
        let v = scan_paths_no_resolvers_ns(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &[],
            None,
        )
        .unwrap();
        assert_eq!(v.len(), 1, "unset namespace must check all cites: {:?}", v);
    }

    #[test]
    fn namespace_exact_segment_not_prefix() {
        // `scxmlfoo` is a different segment than `scxml`, so `scxmlfoo-1` is
        // foreign and skipped; `scxml-D-interpret` is in scope and fires.
        use mnemosyne_atomic::AtomicStore;
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(
            tmp.path().join("src/foo.rs"),
            "// \u{00a7}scxmlfoo-1 foreign; \u{00a7}scxml-D-interpret in-scope\n",
        )
        .unwrap();
        let store = AtomicStore::new();
        let v = scan_paths_no_resolvers_ns(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &[],
            Some("scxml"),
        )
        .unwrap();
        assert_eq!(
            v.len(),
            1,
            "only the exact-segment cite is in scope: {:?}",
            v
        );
        match &v[0] {
            CodeRefViolation::Citation { citation, .. } => {
                assert!(citation.entry_id.contains("scxml-D-interpret"));
            }
            other => panic!("unexpected variant: {:?}", other),
        }
    }

    #[test]
    fn namespace_no_hyphen_id_is_foreign() {
        // A bare `D` cite (no hyphen) has namespace segment "D" ≠ "scxml" → skipped.
        use mnemosyne_atomic::AtomicStore;
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(
            tmp.path().join("src/foo.rs"),
            "// appendix \u{00a7}D root reference\n",
        )
        .unwrap();
        let store = AtomicStore::new();
        let v = scan_paths_no_resolvers_ns(
            tmp.path(),
            &["src/".to_string()],
            "Round ",
            &store,
            &[],
            None,
            true,
            &[],
            &[],
            &[],
            &[],
            Some("scxml"),
        )
        .unwrap();
        assert!(
            v.is_empty(),
            "no-hyphen foreign id must be skipped: {:?}",
            v
        );
    }
}
