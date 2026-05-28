//! Full-scale round-trip measurement (Round 66, OPTION E-6).
//!
//! the 7 markdown docs in this mnemosyne repo (DESIGN / ARCHITECTURE / ROADMAP / VISION /
//! CONCEPTS / README / PRIOR_ART) full in markdown_import::parse_markdown +
//! markdown_export::emit_markdown's *symmetric sym* validation (parse → emit → re-parse →
//! diff = ∅) measurement.
//!
//! DESIGN §66 *Phase 0 entry block prerequisite* item 1 + item 2's *self-validating
//! Mechanical-credibility full-scale validation scope — Round 62-65
//! Follow-up to the small-fixture-feasibility validation layer (Phase 0 implementation
//! layer entry-time measurement source).
//!
//! this round = produces measurement data only — Round 67+ follow-up round delivers design ratify + fix
//! scope: minute output work.

use codegen_prototype::markdown_export::{compare_typed_facts, emit_markdown};
use codegen_prototype::markdown_import::{parse_markdown, ParsedDoc, RefKind};
use codegen_prototype::t1_validator::cross_ref_orphan_reject;
use std::path::PathBuf;

/// 7 markdown doc paths — relative to mnemosyne repo root.
const DOC_PATHS: &[&str] = &[
 "docs/DESIGN.md",
 "docs/ARCHITECTURE.md",
 "docs/ROADMAP.md",
 "docs/VISION.md",
 "docs/CONCEPTS.md",
 "README.md",
 "docs/PRIOR_ART.md",
];

fn repo_root() -> PathBuf {
 // CARGO_MANIFEST_DIR = bench/crates/codegen-prototype, repo root = ../../..
 let manifest_dir = env!("CARGO_MANIFEST_DIR");
 PathBuf::from(manifest_dir)
 .join("../../..")
 .canonicalize()
 .expect("repo root recovery failed — bench/crates/codegen-prototype position validation")
}

fn read_doc(rel_path: &str) -> String {
 let abs_path = repo_root().join(rel_path);
 std::fs::read_to_string(&abs_path)
 .unwrap_or_else(|e| panic!("read {} failure: {}", abs_path.display(), e))
}

#[derive(Debug, Clone)]
struct DocStats {
 name: String,
 bytes: usize,
 lines: usize,
 parsed: ParsedDoc,
 round_trip_section_match: bool,
 round_trip_changelog_match: bool,
 round_trip_cross_ref_match: bool,
 mandatory_preserved: bool,
 emitted_bytes: usize,
}

fn measure_doc(rel_path: &str) -> DocStats {
 let content = read_doc(rel_path);
 let bytes = content.len();
 let lines = content.lines().count();
 let parsed = parse_markdown(&content, rel_path);
 let emitted = emit_markdown(&parsed);
 let reparsed = parse_markdown(&emitted, rel_path);
 let diff = compare_typed_facts(&parsed, &reparsed);

 DocStats {
 name: rel_path.to_string(),
 bytes,
 lines,
 parsed,
 round_trip_section_match: diff.section_identity_match,
 round_trip_changelog_match: diff.changelog_sequence_match,
 round_trip_cross_ref_match: diff.cross_ref_set_match,
 mandatory_preserved: diff.mandatory_preserved,
 emitted_bytes: emitted.len(),
 }
}

#[test]
fn all_seven_docs_readable() {
 for path in DOC_PATHS {
 let content = read_doc(path);
 assert!(
 !content.is_empty(),
 "{} - empty — file existence + read validation failure",
 path
 );
 }
}

#[test]
fn all_seven_docs_parse_without_panic() {
 for path in DOC_PATHS {
 let stats = measure_doc(path);
 // parse itself- panic in becomes- if PASS — typed facts's both- doc endmany many-.
 assert!(
 !stats.parsed.sections.is_empty(),
 "{} parse result sections - 0 — h1 / h2 missing ofcore",
 path
 );
 }
}

#[test]
fn full_scale_parse_summary_dump() {
 println!();
 println!("=== Round 66 OPTION E-6 full-scale parse + round-trip measure data ===");
 println!();
 println!(
 "{:<28} {:>8} {:>6} {:>9} {:>10} {:>10} {:>9} {:>10} {:>10}",
 "doc", "bytes", "lines", "sections", "changelog", "cross_ref", "warning", "emitted_b", "RT-PASS"
 );
 println!("{}", "─".repeat(110));

 let mut total_bytes = 0usize;
 let mut total_lines = 0usize;
 let mut total_sections = 0usize;
 let mut total_changelog = 0usize;
 let mut total_cross_ref = 0usize;
 let mut total_warnings = 0usize;
 let mut total_emitted = 0usize;
 let mut full_pass_count = 0usize;
 let mut sec_pass = 0usize;
 let mut cl_pass = 0usize;
 let mut cr_pass = 0usize;

 for path in DOC_PATHS {
 let s = measure_doc(path);
 println!(
 "{:<28} {:>8} {:>6} {:>9} {:>10} {:>10} {:>9} {:>10} {:>10}",
 s.name,
 s.bytes,
 s.lines,
 s.parsed.sections.len(),
 s.parsed.changelog_entries.len(),
 s.parsed.cross_refs.len(),
 s.parsed.warnings.len(),
 s.emitted_bytes,
 if s.mandatory_preserved {
  "PASS"
 } else {
  "DIFF"
 }
 );
 total_bytes += s.bytes;
 total_lines += s.lines;
 total_sections += s.parsed.sections.len();
 total_changelog += s.parsed.changelog_entries.len();
 total_cross_ref += s.parsed.cross_refs.len();
 total_warnings += s.parsed.warnings.len();
 total_emitted += s.emitted_bytes;
 if s.mandatory_preserved {
 full_pass_count += 1;
 }
 if s.round_trip_section_match {
 sec_pass += 1;
 }
 if s.round_trip_changelog_match {
 cl_pass += 1;
 }
 if s.round_trip_cross_ref_match {
 cr_pass += 1;
 }
 }
 println!("{}", "─".repeat(110));
 println!(
 "{:<28} {:>8} {:>6} {:>9} {:>10} {:>10} {:>9} {:>10} {:>10}",
 "TOTAL (7 doc)",
 total_bytes,
 total_lines,
 total_sections,
 total_changelog,
 total_cross_ref,
 total_warnings,
 total_emitted,
 format!("{}/{}", full_pass_count, DOC_PATHS.len()),
 );
 println!();
 println!(
 "Round-trip integrity dimensionper PASS rate (DESIGN §61 *preserved mandatory dimension* 3 dimension):"
 );
 println!(
 " - section identity match: {}/{}",
 sec_pass,
 DOC_PATHS.len()
 );
 println!(
 " - changelog sequence match: {}/{}",
 cl_pass,
 DOC_PATHS.len()
 );
 println!(
 " - cross_ref set match: {}/{}",
 cr_pass,
 DOC_PATHS.len()
 );
 println!(
 " - mandatory preserved mandatory dimension: {}/{}",
 full_pass_count,
 DOC_PATHS.len()
 );
 println!();
 println!("=== Round 66 measure data produce complete ===");
 println!("source-of-truth: bench/crates/codegen-prototype/tests/markdown_full_scale.rs");
 println!("design round ratify: Round 67+ (follow-up round, OPTION E-7+ — edge case minutesection + fix scope)");
}

#[test]
fn design_md_parse_recognizes_round_entries() {
 let stats = measure_doc("docs/DESIGN.md");
 // Rounds 1-65 are registered — this round's entry brings the total to ~65+ (see above).
 // this round measure source = ChangelogEntry recognize feasibility validation (full count formal-property- separate layer).
 assert!(
 stats.parsed.changelog_entries.len() >= 30,
 "DESIGN.md changelog_entries - {} — Round N entry recognize not- ofcore (>=30 expected)",
 stats.parsed.changelog_entries.len()
 );
}

#[test]
fn design_md_parse_recognizes_section_id_39_56_61_66() {
 let stats = measure_doc("docs/DESIGN.md");
 let section_ids: std::collections::BTreeSet<&str> = stats
 .parsed
 .sections
 .iter()
 .map(|s| s.section_id.as_str())
 .collect();
 // -core §39 / §56 / §61 / §66 body registered — markdown_import + markdown_export sequence's source.
 assert!(section_ids.contains("39"), "§39 section_id recognize failure");
 assert!(section_ids.contains("56"), "§56 section_id recognize failure");
 assert!(section_ids.contains("61"), "§61 section_id recognize failure");
 assert!(section_ids.contains("66"), "§66 section_id recognize failure");
}

#[test]
fn dump_round_trip_diff_per_file() {
 use std::collections::BTreeSet;
 println!();
 println!("=== Round-trip diff detail per DIFF file ===");
 for path in DOC_PATHS {
 let content = read_doc(path);
 let parsed = parse_markdown(&content, path);
 let emitted = emit_markdown(&parsed);
 let reparsed = parse_markdown(&emitted, path);
 let diff = compare_typed_facts(&parsed, &reparsed);
 if diff.mandatory_preserved {
 continue;
 }
 println!();
 println!("─── {} ──────────────────────────────────────", path);
 println!(
 " sections: {} → {} (match={})",
 diff.section_count_a, diff.section_count_b, diff.section_identity_match
 );
 println!(
 " changelog: {} → {} (match={})",
 diff.changelog_entry_count_a, diff.changelog_entry_count_b, diff.changelog_sequence_match
 );
 println!(
 " cross_ref: {} → {} (match={})",
 diff.cross_ref_count_a, diff.cross_ref_count_b, diff.cross_ref_set_match
 );
 if !diff.section_identity_match {
 let a_keys: BTreeSet<(String, Option<String>, String)> = parsed
  .sections
  .iter()
  .map(|s| (s.section_id.clone(), s.parent_section.clone(), s.title.clone()))
  .collect();
 let b_keys: BTreeSet<(String, Option<String>, String)> = reparsed
  .sections
  .iter()
  .map(|s| (s.section_id.clone(), s.parent_section.clone(), s.title.clone()))
  .collect();
 let only_a: Vec<_> = a_keys.difference(&b_keys).take(8).collect();
 let only_b: Vec<_> = b_keys.difference(&a_keys).take(8).collect();
 println!(" only-in-original ({} total, first 8):", a_keys.difference(&b_keys).count());
 for k in &only_a {
  println!(" {:?}", k);
 }
 println!(" only-in-reparsed ({} total, first 8):", b_keys.difference(&a_keys).count());
 for k in &only_b {
  println!(" {:?}", k);
 }
 }
 if !diff.cross_ref_set_match {
 let a_set: BTreeSet<(String, String, RefKind)> = parsed
  .cross_refs
  .iter()
  .map(|c| (c.from_section.clone(), c.to_target.clone(), c.ref_kind))
  .collect();
 let b_set: BTreeSet<(String, String, RefKind)> = reparsed
  .cross_refs
  .iter()
  .map(|c| (c.from_section.clone(), c.to_target.clone(), c.ref_kind))
  .collect();
 let only_a: Vec<_> = a_set.difference(&b_set).take(8).collect();
 let only_b: Vec<_> = b_set.difference(&a_set).take(8).collect();
 println!(" cross_ref only-in-original ({} total, first 8):", a_set.difference(&b_set).count());
 for k in &only_a {
  println!(" {:?}", k);
 }
 println!(" cross_ref only-in-reparsed ({} total, first 8):", b_set.difference(&a_set).count());
 for k in &only_b {
  println!(" {:?}", k);
 }
 }
 }
}

#[test]
fn t1_validator_orphan_measurement_per_file() {
 println!();
 println!("=== Round 68 OPTION A — T1 cross_ref_orphan_reject 7 file measurement ===");
 println!();
 println!(
 "{:<28} {:>9} {:>10} {:>10} {:>9}",
 "doc", "cross_ref", "orphan", "orphan%", "decision"
 );
 println!("{}", "─".repeat(80));
 let mut total_cross_ref = 0usize;
 let mut total_orphan = 0usize;
 for path in DOC_PATHS {
 let content = read_doc(path);
 let parsed = parse_markdown(&content, path);
 let total = parsed.cross_refs.len();
 let intra = parsed
 .cross_refs
 .iter()
 .filter(|c| c.ref_kind != RefKind::CrossDoc)
 .count();
 let orphans = cross_ref_orphan_reject(&parsed);
 let orphan_count = orphans.len();
 let pct = if intra > 0 {
 100.0 * orphan_count as f64 / intra as f64
 } else {
 0.0
 };
 println!(
 "{:<28} {:>9} {:>10} {:>9.1}% {:>9}",
 path,
 total,
 orphan_count,
 pct,
 if orphan_count == 0 { "PASS" } else { "WARN" }
 );
 total_cross_ref += total;
 total_orphan += orphan_count;
 }
 println!("{}", "─".repeat(80));
 println!(
 "{:<28} {:>9} {:>10} {:>9.1}% {:>9}",
 "TOTAL (7 doc)",
 total_cross_ref,
 total_orphan,
 if total_cross_ref > 0 {
 100.0 * total_orphan as f64 / total_cross_ref as f64
 } else {
 0.0
 },
 if total_orphan == 0 { "PASS" } else { "WARN" }
 );
 println!();
 println!("=== Round 68 measure data produce complete ===");
 println!("source-of-truth: bench/crates/codegen-prototype/src/t1_validator.rs");
 println!("design round ratify: Round 69+ (next round, prerequisite #4 measurement ratify)");
}

#[test]
fn dump_design_md_orphan_detail() {
 // Round 69 OPTION H-1 round — DESIGN.md 2 orphan's from_section + to_target +
 // ref_kind + raw source line context dump. minutekind input source (real fragile /
 // parser bug / cross-doc intent) — design ratify scope identify.
 let path = "docs/DESIGN.md";
 let content = read_doc(path);
 let parsed = parse_markdown(&content, path);
 let orphans = cross_ref_orphan_reject(&parsed);
 println!();
 println!("=== Round 69 OPTION H-1 — DESIGN.md orphan detail dump ===");
 println!();
 println!("total cross_ref : {}", parsed.cross_refs.len());
 println!("intra-doc cross_ref : {}", parsed.cross_refs.iter().filter(|c| c.ref_kind != RefKind::CrossDoc).count());
 println!("orphan (rule 1) : {}", orphans.len());
 println!();

 let lines: Vec<&str> = content.lines().collect();
 for (i, err) in orphans.iter().enumerate() {
 if let codegen_prototype::t1_validator::ValidationError::OrphanCrossRef {
 from_section,
 to_target,
 ref_kind,
 } = err
 {
 println!("─── orphan #{} ──────────────────────────────────────", i + 1);
 println!(" from_section : §{}", from_section);
 println!(" to_target : §{}", to_target);
 println!(" ref_kind : {:?}", ref_kind);
 // grep raw source for §{to_target} occurrences (with word boundary check
 // — exclude longer numbers like §11 matching §1).
 let needle_dot = format!("§{}.", to_target);
 let needle_bare = format!("§{}", to_target);
 let mut hit_count = 0usize;
 for (lno, line) in lines.iter().enumerate() {
  // Find candidate occurrences and verify next char is non-digit (boundary).
  let mut search_from = 0usize;
  while let Some(pos) = line[search_from..].find(&needle_bare) {
  let abs = search_from + pos;
  let after = &line[abs + needle_bare.len()..];
  let next_char = after.chars().next();
  let is_boundary = match next_char {
  Some(c) if c.is_ascii_digit() => false,
  _ => true,
  };
  if is_boundary || line[abs..].starts_with(&needle_dot) {
  if hit_count < 5 {
   let preview = if line.len() > 200 {
   &line[..200]
   } else {
   line
   };
   println!(" L{:>5}: {}", lno + 1, preview);
  }
  hit_count += 1;
  break; // only one print per line
  }
  search_from = abs + needle_bare.len();
  }
 }
 println!(" total raw occurrences of §{} (boundary-checked): {}", to_target, hit_count);
 // Also count direct `to_target` in known section_id list — Section.section_id
 // as recognize done doc in §N enumerate.
 let known_ids: std::collections::BTreeSet<&str> = parsed
  .sections
  .iter()
  .map(|s| s.section_id.as_str())
  .collect();
 println!(" to_target ∈ section_id_set: {}", known_ids.contains(to_target.as_str()));
 // dump from_section title + parent_section for navigation context.
 if let Some(sec) = parsed.sections.iter().find(|s| s.section_id == *from_section) {
  println!(
  " from_section title : {:?} (parent_section: {:?})",
  sec.title, sec.parent_section
  );
 } else {
  println!(" from_section title : <NOT FOUND in section_id_set — pseudo-section?>");
 }
 println!();
 }
 }
 println!("=== orphan detail dump complete ===");
 println!("source-of-truth: bench/crates/codegen-prototype/tests/markdown_full_scale.rs");
 println!("classification (real fragile / parser bug / cross-doc intent) → Round 69 design ratify");
}

#[test]
fn dump_remaining_orphans_detail() {
 // Round 78 — production validation result remaining orphan minutekind source ledger.
 // 7 markdown doc full in cross_ref orphan's fine-grained classification:
 // (a) intra-doc parser-bug: to_target is a real heading but parser missed it
 // (heading-detection edge case, e.g. "## §6.6" scope anchor slug transform).
 // (b) cross-doc intent: workspace default-doc lookup priority step (2)
 // reclassify candidate (Round 70 OPTION H-2 adoption carry then remaining scope).
 // (c) directory-ref: to_target ends with "/" (markdown link `[text](dir/)` form)
 // — parser in dir-ref recognize / Impl-kind emit exclude / separate fact scope separation formal- source.
 // (d) genuine-fragile: real missing reference (consistency -, resolved intent mutation source).
 println!();
 println!("=== Round 78 — remaining orphan classification dump (7 docs) ===");
 println!();

 #[derive(Debug, Clone, Copy, PartialEq, Eq)]
 enum OrphanCategory {
 IntraDocParserBug,
 CrossDocIntent,
 DirectoryRef,
 GenuineFragile,
 }

 fn classify(
 from_section: &str,
 to_target: &str,
 ref_kind: &RefKind,
 all_doc_section_ids: &std::collections::BTreeSet<String>,
 ) -> (OrphanCategory, &'static str) {
 // (c) directory-ref: target ends with "/" or contains a "/" path segment
 // typical of `[text](bench/)` style markdown link.
 if to_target.ends_with('/') || (to_target.contains('/') && !to_target.contains('#')) {
 return (
  OrphanCategory::DirectoryRef,
  "markdown link points to a directory path (e.g. `(bench/)`); \
  §61 mapping table needs a `directory_ref` row — emit policy: \
  either drop from cross_refs or carry as Impl-kind with dir tag.",
 );
 }
 // (b) cross-doc intent: target appears as section_id in any other doc
 // — workspace lookup step (2) candidate.
 if all_doc_section_ids.contains(to_target) {
 return (
  OrphanCategory::CrossDocIntent,
  "to_target exists as section_id in another doc — workspace \
  default-doc lookup (Round 70 OPTION H-2) handles this in production.",
 );
 }
 // (a) intra-doc parser-bug: ref_kind is Decision (intra-doc), and the
 // target has §-form numeric pattern but isn't in any section_id_set.
 // Likely a heading-slug variant the parser missed.
 let _ = from_section;
 let looks_numeric_section =
 to_target.chars().all(|c| c.is_ascii_digit() || c == '.');
 if matches!(ref_kind, RefKind::Decision) && looks_numeric_section {
 return (
  OrphanCategory::IntraDocParserBug,
  "intra-doc Decision ref pointing to numeric section_id but \
  parser did not emit a matching Section — heading detection \
  edge case.",
 );
 }
 (
 OrphanCategory::GenuineFragile,
 "no resolution path matched — genuine fragile cross-ref, requires \
  content-level mutation.",
 )
 }

 // Build cross-doc section_id set for category (b).
 let mut all_doc_section_ids: std::collections::BTreeSet<String> =
 std::collections::BTreeSet::new();
 for path in DOC_PATHS {
 let content = read_doc(path);
 let parsed = parse_markdown(&content, path);
 for s in &parsed.sections {
 all_doc_section_ids.insert(s.section_id.clone());
 }
 }

 let mut totals = std::collections::BTreeMap::<String, usize>::new();
 let mut grand_total = 0usize;

 for path in DOC_PATHS {
 let content = read_doc(path);
 let parsed = parse_markdown(&content, path);
 let orphans = cross_ref_orphan_reject(&parsed);
 if orphans.is_empty() {
 continue;
 }
 println!("─── {path} ─────────────────────────────────────────");
 println!(" total orphans : {}", orphans.len());
 let local_ids: std::collections::BTreeSet<String> = parsed
 .sections
 .iter()
 .map(|s| s.section_id.clone())
 .collect();
 for (i, err) in orphans.iter().enumerate() {
 if let codegen_prototype::t1_validator::ValidationError::OrphanCrossRef {
  from_section,
  to_target,
  ref_kind,
 } = err
 {
  let cross_doc_set: std::collections::BTreeSet<String> = all_doc_section_ids
  .difference(&local_ids)
  .cloned()
  .collect();
  let (cat, note) = classify(from_section, to_target, ref_kind, &cross_doc_set);
  let cat_label = match cat {
  OrphanCategory::IntraDocParserBug => "INTRA_DOC_PARSER_BUG",
  OrphanCategory::CrossDocIntent => "CROSS_DOC_INTENT",
  OrphanCategory::DirectoryRef => "DIRECTORY_REF",
  OrphanCategory::GenuineFragile => "GENUINE_FRAGILE",
  };
  *totals.entry(cat_label.to_string()).or_insert(0) += 1;
  grand_total += 1;
  println!(
  " [{:02}] from=§{} to={} ref_kind={:?} → {}\n  policy: {}",
  i + 1,
  from_section,
  to_target,
  ref_kind,
  cat_label,
  note
  );
 }
 }
 println!();
 }

 println!("=== category totals (grand_total={}) ===", grand_total);
 for (k, v) in &totals {
 println!(" {} : {}", k, v);
 }
 println!();
 println!("source-of-truth: bench/crates/codegen-prototype/tests/markdown_full_scale.rs");
 println!("§61 mapping-table row: DIRECTORY_REF policy etc. — DESIGN.md Round 78 ratify carry");

 // The categorization itself is a measurement output; the test passes as long
 // as the run succeeds. Production assertions live in mnemosyne-validate
 // self_validation tests (where workspace reclassification eliminates the
 // CROSS_DOC_INTENT bucket).
}

#[test]
fn dump_design_md_emit_first_50_lines() {
 let content = read_doc("docs/DESIGN.md");
 let parsed = parse_markdown(&content, "docs/DESIGN.md");
 let emitted = emit_markdown(&parsed);
 println!();
 println!("=== Emit output first 50 lines (DESIGN.md) ===");
 for (i, line) in emitted.lines().take(50).enumerate() {
 println!(" {:>3}: {}", i + 1, line);
 }
 println!();
 println!("=== Original sections — first 10 ===");
 for s in parsed.sections.iter().take(10) {
 println!(
 " section_id={:?} parent={:?} title={:?}",
 s.section_id, s.parent_section, s.title
 );
 }
 println!();
 println!("=== Reparsed sections — first 10 ===");
 let reparsed = parse_markdown(&emitted, "docs/DESIGN.md");
 for s in reparsed.sections.iter().take(10) {
 println!(
 " section_id={:?} parent={:?} title={:?}",
 s.section_id, s.parent_section, s.title
 );
 }
}

#[test]
fn design_md_round_trip_section_count_stability() {
 let stats = measure_doc("docs/DESIGN.md");
 // section count itself- round-trip on *complete preserved mandatory* dimension.
 // this test - section_count_a == section_count_b validation (mandatory_preserved's subset).
 let _content = read_doc("docs/DESIGN.md");
 let parsed_a = stats.parsed.clone();
 let emitted = emit_markdown(&parsed_a);
 let parsed_b = parse_markdown(&emitted, "docs/DESIGN.md");
 let diff = compare_typed_facts(&parsed_a, &parsed_b);
 assert_eq!(
 diff.section_count_a, diff.section_count_b,
 "DESIGN.md section count round-trip nostable — {} → {} (drift detected)",
 diff.section_count_a, diff.section_count_b
 );
}
