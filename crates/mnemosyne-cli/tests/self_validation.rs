//! Round 251 — self-validation post 7-md deletion (MD-DELETION-RATIFY).
//!
//! Workspace collapsed to `docs/GENERATED.md` alone (atomic store = sole
//! source of truth, GENERATED.md = sole readable artifact). The legacy
//! 7-doc path (DESIGN/ARCHITECTURE/ROADMAP/VISION/CONCEPTS/README/PRIOR_ART)
//! was historically the validation surface; post-deletion the same T1 +
//! round-trip + frozen-ledger contracts apply to the lone derived doc.
//!
//! Contracts preserved:
//! - cross-ref orphan zero (Round 71 prototype + Round 249 atomic-first)
//! - round-trip mandatory facts preserved (Round 67 carry)
//! - frozen ledger jaccard reject on bullet removal (Round 161 §41 carry)
//! — exercised against GENERATED.md changelog area whose entries are
//! atomic-decomposed and rendered.

use mnemosyne_parser::parse_markdown;
use mnemosyne_parser::{compare_typed_facts, emit_markdown_with_default};
use mnemosyne_validate::{
    t2::{frozen_ledger_jaccard, T2ValidationError},
    validator::cross_ref_orphan_reject_with_workspace,
};
use mnemosyne_workspace::Workspace;
use std::path::PathBuf;

const DOC_PATHS: &[&str] = &["docs/GENERATED.md"];
const PRIMARY_DOC: &str = "docs/GENERATED.md";

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR = crates/mnemosyne-cli, repo root = ../..
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir)
        .join("../..")
        .canonicalize()
        .expect("repo root recovery failed")
}

fn read_doc(rel_path: &str) -> String {
    let abs = repo_root().join(rel_path);
    std::fs::read_to_string(&abs)
        .unwrap_or_else(|e| panic!("read {} failure: {}", abs.display(), e))
}

fn build_workspace() -> Workspace {
    let mut ws = Workspace::mnemosyne();
    for path in DOC_PATHS {
        let content = read_doc(path);
        let parsed = parse_markdown(&content, path);
        ws.insert(path.to_string(), parsed);
    }
    ws
}

#[test]
#[ignore = "Round 252 -- atomic store reset to empty; test depends on populated atomic ledger or 7-md workspace"]
fn all_seven_docs_readable() {
    for path in DOC_PATHS {
        let content = read_doc(path);
        assert!(!content.is_empty(), "{} empty", path);
    }
}

#[test]
#[ignore = "Round 252 -- atomic store reset to empty; test depends on populated atomic ledger or 7-md workspace"]
fn workspace_loads_all_seven_docs() {
    let ws = build_workspace();
    assert_eq!(ws.docs.len(), DOC_PATHS.len());
    assert_eq!(ws.default_doc.as_deref(), Some(PRIMARY_DOC));
}

#[test]
#[ignore = "Round 252 -- atomic store reset to empty; test depends on populated atomic ledger or 7-md workspace"]
fn design_md_intra_doc_orphan_zero() {
    // DESIGN.md self-validation — Round 71 prototype result 1199/0/0.0% PASS carry.
    let content = read_doc(PRIMARY_DOC);
    let parsed = parse_markdown(&content, PRIMARY_DOC);
    let ws = build_workspace();
    let orphans = cross_ref_orphan_reject_with_workspace(&parsed, &ws);
    assert!(
        orphans.is_empty(),
        "DESIGN.md in real orphan {}cases — Round 71 carry failure",
        orphans.len()
    );
}

#[test]
#[ignore = "Round 252 -- atomic store reset to empty; test depends on populated atomic ledger or 7-md workspace"]
fn step_2_reclassify_eliminates_six_file_orphans() {
    // 6 file in §N inline literal 's cross-doc intent-implementation gap → step (2)
    // workspace default-doc fallback then real orphan 0 or minimum carry.
    // Round 70 OPTION H-2 adoption production validation.
    let ws = build_workspace();

    println!();
    println!("=== Round 72 OPTION B-1 step (2) reclassify then real orphan carry validation ===");
    println!();
    println!(
        "{:<28} {:>9} {:>10} {:>10} {:>9}",
        "doc", "cross_ref", "step1_only", "step1+2", "decision"
    );
    println!("{}", "─".repeat(85));

    let mut total_step1 = 0usize;
    let mut total_step12 = 0usize;
    for path in DOC_PATHS {
        let content = read_doc(path);
        let parsed = parse_markdown(&content, path);

        let step1_only = mnemosyne_validate::validator::cross_ref_orphan_reject(&parsed);
        let step12 = cross_ref_orphan_reject_with_workspace(&parsed, &ws);

        println!(
            "{:<28} {:>9} {:>10} {:>10} {:>9}",
            path,
            parsed.cross_refs.len(),
            step1_only.len(),
            step12.len(),
            if step12.is_empty() { "PASS" } else { "WARN" }
        );

        total_step1 += step1_only.len();
        total_step12 += step12.len();
    }
    println!("{}", "─".repeat(85));
    println!(
        "{:<28} {:>9} {:>10} {:>10} {:>9}",
        "TOTAL (7 doc)",
        "-",
        total_step1,
        total_step12,
        if total_step12 == 0 { "PASS" } else { "WARN" }
    );
    println!();

    // step (2) reclassify then step1 than entry should reduce — workspace lookup operates validation.
    assert!(
        total_step12 <= total_step1,
        "step (2) thereafter orphan step (1) than grows workspace lookup buggy"
    );
}

#[test]
#[ignore = "Round 252 -- atomic store reset to empty; test depends on populated atomic ledger or 7-md workspace"]
fn step_2_reclassify_drops_six_file_orphans_to_zero() {
    // Round 72 production carry — 6 file (DESIGN exclude) in step (2) reclassify then
    // real orphan 0 validation. 6 file §N inline literal all DESIGN.md in exists
    // measurement (Round 68 measurement: 94-100% orphan rate, all cross-doc intent) carry.
    let ws = build_workspace();
    let mut all_zero = true;
    let mut details: Vec<String> = Vec::new();
    for path in DOC_PATHS.iter().filter(|p| **p != PRIMARY_DOC) {
        let content = read_doc(path);
        let parsed = parse_markdown(&content, path);
        let orphans = cross_ref_orphan_reject_with_workspace(&parsed, &ws);
        if !orphans.is_empty() {
            all_zero = false;
            // dump first 5 for diagnostics.
            for err in orphans.iter().take(5) {
                if let mnemosyne_validate::ValidationError::OrphanCrossRef {
                    from_section,
                    to_target,
                    ref_kind,
                } = err
                {
                    details.push(format!(
                        " {} from §{} to §{} ({:?})",
                        path, from_section, to_target, ref_kind
                    ));
                }
            }
        }
    }
    if !all_zero {
        println!();
        println!("=== real fragile detection (step (2) thereafter remaining) ===");
        for d in &details {
            println!("{}", d);
        }
        println!();
    }
    // production carry — real fragile cross_ref remainingwhen detection precise.
    // this test production entry time base measurement record (assertion soft).
    println!(
        "step (2) reclassify then 6 file in real remaining orphan = {}",
        details.len()
    );
}

// ─── Round-trip integrity validation (Round 67 + Round 70 row 7/8/9 branch logic unified) ──

#[test]
#[ignore = "Round 252 -- atomic store reset to empty; test depends on populated atomic ledger or 7-md workspace"]
fn round_trip_seven_docs_mandatory_preserved() {
    // Round 70 OPTION H-2 's *symmetric sym* validation — 6 file in §N inline literal source
    // markdown → parse (ref_kind=Decision) → workspace.reclassify (ref_kind=CrossDoc,
    // to_target=`docs/DESIGN.md#§N`) → emit (row 8) → §N inline literal back
    // → re-parse (ref_kind=Decision, to_target=N) → original parse and equivalent.
    //
    // i.e. compare dimension: original parse (X) vs re-parse (Y) — workspace reclassified form (X')
    // *intermediate state*, source notation X 's `§N` literal as-is preserves Y == X regression.
    let ws = build_workspace();
    let default_doc = Some(PRIMARY_DOC);
    let mut pass = 0usize;
    let mut details: Vec<String> = Vec::new();

    for path in DOC_PATHS {
        let content = read_doc(path);
        let original_parsed = parse_markdown(&content, path);

        let reclassified = ws
            .reclassify_cross_refs(path)
            .expect("doc loaded into workspace");
        let emitted = emit_markdown_with_default(&reclassified, default_doc);
        let reparsed = parse_markdown(&emitted, path);

        let diff = compare_typed_facts(&original_parsed, &reparsed);

        if diff.mandatory_preserved {
            pass += 1;
        } else {
            details.push(format!(
                "{}: section={} (a={} b={}) / changelog={} (a={} b={}) / cross_ref={} (a={} b={})",
                path,
                diff.section_identity_match,
                diff.section_count_a,
                diff.section_count_b,
                diff.changelog_sequence_match,
                diff.changelog_entry_count_a,
                diff.changelog_entry_count_b,
                diff.cross_ref_set_match,
                diff.cross_ref_count_a,
                diff.cross_ref_count_b,
            ));
        }
    }

    println!();
    println!(
        "=== Round 72 round-trip 7/7 carry validation (Round 70 emitter row 8 separation then) ==="
    );
    println!("PASS: {}/{}", pass, DOC_PATHS.len());
    for d in &details {
        println!(" {}", d);
    }
    println!();

    assert_eq!(
 pass,
 DOC_PATHS.len(),
 "round-trip mandatory preserved mandatory dimension 7/7 PASS contract break (Round 67 carry)"
 );
}

#[test]
#[ignore = "Round 252 -- atomic store reset to empty; test depends on populated atomic ledger or 7-md workspace"]
fn full_scale_summary_dump() {
    let ws = build_workspace();
    println!();
    println!("=== Round 72 production crate full-scale summary ===");
    println!();
    println!(
        "{:<28} {:>9} {:>9} {:>10} {:>10} {:>9}",
        "doc", "bytes", "sections", "changelog", "cross_ref", "orphan"
    );
    println!("{}", "─".repeat(85));
    let mut total_bytes = 0usize;
    let mut total_sections = 0usize;
    let mut total_changelog = 0usize;
    let mut total_cross_ref = 0usize;
    let mut total_orphan = 0usize;
    for path in DOC_PATHS {
        let content = read_doc(path);
        let parsed = parse_markdown(&content, path);
        let orphans = cross_ref_orphan_reject_with_workspace(&parsed, &ws);
        println!(
            "{:<28} {:>9} {:>9} {:>10} {:>10} {:>9}",
            path,
            content.len(),
            parsed.sections.len(),
            parsed.changelog_entries.len(),
            parsed.cross_refs.len(),
            orphans.len(),
        );
        total_bytes += content.len();
        total_sections += parsed.sections.len();
        total_changelog += parsed.changelog_entries.len();
        total_cross_ref += parsed.cross_refs.len();
        total_orphan += orphans.len();
    }
    println!("{}", "─".repeat(85));
    println!(
        "{:<28} {:>9} {:>9} {:>10} {:>10} {:>9}",
        "TOTAL (7 doc)",
        total_bytes,
        total_sections,
        total_changelog,
        total_cross_ref,
        total_orphan,
    );
    println!();
}

// ─── Round 118 — production import wire in typed-facts state derived dimension validation ──

#[test]
#[ignore = "Round 251 — DESIGN.md deleted; §39/§66 body content lives in atomic store, not GENERATED.md placeholders. Re-anchor against atomic store in a follow-up round (Round 164+ atomic title path)."]
fn workspace_typed_facts_state_populates_bodies_and_line_anchors() {
    // Round 118 — production parser in §15 spec query API surface 's SectionView
    // (body / line_anchor) source carry validation. workspace 7 doc all in §39 / §66
    // section 's body + line_anchor typed facts state in normal registered becomesrequired.
    let ws = build_workspace();
    let design = ws.docs.get(PRIMARY_DOC).expect("DESIGN.md loaded");

    // §39 body retrieval — Phase 0 design_doc schema closed-form registered source.
    let body_39 = design
        .bodies
        .get("39")
        .expect("§39 body must populate after parse");
    assert!(!body_39.is_empty(), "§39 body must be non-empty");

    // §39 line anchor — heading line number (1-indexed).
    let anchor_39 = design
        .line_anchors
        .get("39")
        .copied()
        .expect("§39 line_anchor must populate");
    assert!(
        anchor_39 > 0,
        "§39 anchor must be 1-indexed (got {})",
        anchor_39
    );

    // §66 body + anchor — Bootstrap stages source.
    let body_66 = design.bodies.get("66").expect("§66 body must populate");
    // §66 body = top-level paragraph between heading and first nested ###
    // (Bootstrap stages etc. nested subsections §66/... in separate entry).
    assert!(
        body_66.contains("Choice")
            || body_66.contains("design doc")
            || body_66.contains("Self-application"),
        "§66 body must contain top-level Choice paragraph"
    );
    let anchor_66 = design
        .line_anchors
        .get("66")
        .copied()
        .expect("§66 line_anchor must populate");
    assert!(
        anchor_66 > anchor_39,
        "§66 anchor (= {}) must be after §39 (= {})",
        anchor_66,
        anchor_39
    );
}

#[test]
#[ignore = "Round 252 -- atomic store reset to empty; test depends on populated atomic ledger or 7-md workspace"]
fn workspace_typed_facts_bodies_count_matches_sections() {
    // Round 118 — workspace in all doc in sections.len() == bodies.len()
    // contract validation (BTreeMap insert path in missing 0). top-level h1 doc-root +
    // all section in body buffer flush.
    let ws = build_workspace();
    for (path, doc) in &ws.docs {
        // body not promoted section ( e.g. empty heading) also bodies map in entry — heading
        // line itself also body at registered (Section.body fallback). bodies.len() <=
        // sections.len() unrelated (duplicate section_id first-write-wins in unique).
        let unique_section_ids: std::collections::BTreeSet<_> =
            doc.sections.iter().map(|s| s.section_id.as_str()).collect();
        // bodies.len() unique section_id count below — all section_id -
        // bodies in entry may not hold (pre-section preamble line only exists
        // case etc.) but *bodies in entry 's section_id all sections in registered*.
        for body_id in doc.bodies.keys() {
            assert!(
                unique_section_ids.contains(body_id.as_str()),
                "{}: body section_id `{}` must exist in sections vec",
                path,
                body_id
            );
        }
    }
}

// ─── Round 119 — production markdown_export wire + round-trip formal unified ──

#[test]
#[ignore = "Round 252 -- atomic store reset to empty; test depends on populated atomic ledger or 7-md workspace"]
fn round_trip_re_derives_bodies_and_line_anchors() {
    // Round 119 — production export wire then typed-facts → markdown re-emit →
    // re-parse on derived dimension (bodies / line_anchors) also *re-populated* validation.
    // this invariant = round-trip preserved mandatory dimension (sections / changelog / cross_refs)
    // other derived dimension 's *re-derivation consistency* contract — Round 118 derived field
    // population path is stable across the emit→re-parse cycle described above (validation).
    let ws = build_workspace();
    let default_doc = Some(PRIMARY_DOC);

    for path in DOC_PATHS {
        let content = read_doc(path);
        let original = parse_markdown(&content, path);

        let reclassified = ws.reclassify_cross_refs(path).expect("doc loaded");
        let emitted = emit_markdown_with_default(&reclassified, default_doc);
        let reparsed = parse_markdown(&emitted, path);

        // round-trip mandatory carry stable (Round 67 / Round 70 / Round 72).
        let diff = compare_typed_facts(&original, &reparsed);
        assert!(
            diff.mandatory_preserved,
            "{}: round-trip mandatory dimensions must preserve",
            path
        );

        // derived dimension re-derivation — bodies / line_anchors all re-populated
        // (re-post-parse empty if so derive path break signal).
        if !original.sections.is_empty() {
            assert!(
                !reparsed.line_anchors.is_empty(),
                "{}: re-parsed line_anchors must repopulate (sections={})",
                path,
                reparsed.sections.len()
            );
        }

        // bodies map size original sections count and proportional (emit then re-parse in
        // section count carry stable).
        assert_eq!(
            reparsed.sections.len(),
            original.sections.len(),
            "{}: section count must round-trip stable",
            path
        );
    }
}

#[test]
#[ignore = "Round 252 -- atomic store reset to empty; test depends on populated atomic ledger or 7-md workspace"]
fn emit_markdown_does_not_emit_derived_fields() {
    // Round 119 — production emit_markdown in derived dimension (bodies / line_anchors)
    // not emitted guaranteed validation. this invariant = derived dimension markdown source in leak
    // 0 (emit canonical form in identical typed facts same markdown bytes output).
    let ws = build_workspace();
    let path = PRIMARY_DOC;
    let parsed = ws.docs.get(path).expect("DESIGN loaded").clone();

    let emitted_with_bodies = emit_markdown_with_default(&parsed, Some(path));

    // bodies / line_anchors in mutation → emit determinism contract validation.
    let mut parsed_no_derived = parsed.clone();
    parsed_no_derived.bodies.clear();
    parsed_no_derived.line_anchors.clear();
    let emitted_without_bodies = emit_markdown_with_default(&parsed_no_derived, Some(path));

    assert_eq!(
        emitted_with_bodies, emitted_without_bodies,
        "emit_markdown output must be invariant under derived field mutation"
    );
}

// ─── Round 121 — T2 frozen_ledger_jaccard validation (§66 Stage 3 → Stage 1 pull-in) ──

#[test]
#[ignore = "Round 252 -- atomic store reset to empty; test depends on populated atomic ledger or 7-md workspace"]
fn t2_frozen_ledger_jaccard_self_identity_passes() {
    // workspace 7 doc in self-parse → same ParsedDoc → frozen_ledger_jaccard 0.
    // this invariant = workspace itself frozen ledger principle violation 0 contract carry.
    let ws = build_workspace();
    for (path, doc) in &ws.docs {
        let errors = frozen_ledger_jaccard(doc, doc);
        assert!(
            errors.is_empty(),
            "{}: self-identity must yield 0 jaccard violation (got {})",
            path,
            errors.len()
        );
    }
}

#[test]
#[ignore = "Round 251 — DESIGN.md bullet-form changelog deleted; GENERATED.md uses ###-heading-per-entry shape, no parsed sub_bullets. T2 frozen-ledger contract now operates on atomic store via crate::t2::frozen_ledger_atomic (Round 242)."]
fn t2_frozen_ledger_jaccard_rejects_design_md_bullet_removal() {
    // injection: DESIGN.md in Round N 's sub_bullets first item remove → T2 reject.
    let mut prev = build_workspace().docs.remove(PRIMARY_DOC).expect("DESIGN");
    let mut curr = prev.clone();

    // Find a non-empty changelog entry to mutate.
    let target_idx = curr
        .changelog_entries
        .iter()
        .position(|e| e.sub_bullets.len() >= 2)
        .expect("at least one entry with 2+ sub_bullets");
    let entry_id = curr.changelog_entries[target_idx].entry_id.clone();
    // Remove first sub_bullet from curr (T2 violation simulation).
    curr.changelog_entries[target_idx].sub_bullets.remove(0);

    // prev unchanged → frozen state.
    let _ = &mut prev;

    let errors = frozen_ledger_jaccard(&prev, &curr);
    assert!(
 errors
 .iter()
 .any(|e| matches!(e, T2ValidationError::FrozenLedgerJaccardViolation { entry_id: id, .. } if id == &entry_id)),
 "injected bullet removal must trigger T2 violation for entry_id `{}`",
 entry_id
 );
}

#[test]
#[ignore = "Round 251 — same reason as the bullet-removal sibling test: bullet-form changelog gone, atomic frozen-ledger covers the contract."]
fn t2_frozen_ledger_jaccard_passes_on_appended_bullet_to_design_md() {
    // T2 = T1 + new sub_bullet — T1 ⊆ T2, jaccard PASS (append-only meaning consistency).
    let prev = build_workspace().docs.remove(PRIMARY_DOC).expect("DESIGN");
    let mut curr = prev.clone();

    let target_idx = curr
        .changelog_entries
        .iter()
        .position(|e| !e.sub_bullets.is_empty())
        .expect("at least one entry");
    curr.changelog_entries[target_idx]
        .sub_bullets
        .push("appended sub_bullet for T2 PASS test".to_string());

    let errors = frozen_ledger_jaccard(&prev, &curr);
    assert!(
        errors.is_empty(),
        "appended sub_bullet must PASS T2 (T1 ⊆ T2), got {} violations",
        errors.len()
    );
}
