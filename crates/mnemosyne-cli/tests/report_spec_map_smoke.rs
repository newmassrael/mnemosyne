//! Round 392 — `report-spec-map` subcommand smoke tests.
//!
//! `report-spec-map` is a read-only per-section projection unifying, for each
//! section: coverage class (single-sourced with `report-coverage` /
//! `validate-code-refs` via `classify_coverage`), external-spec provenance
//! (`normative_excerpt` anchor_url + source_revision), Path B bindings, the
//! spec-revision drift flag (`scan_spec_drift`), and the reverse citation count
//! (`SetEqualityValidator::citation_index`). The R392 backfill recorded the
//! verb; this locks its CLI output contract — the shape the spec-map
//! visualization consumer reads.
//!
//! Test scope:
//! (i) mixed store, no spec_source / no plugin → JSON summary + per-section
//!  rows; coverage classes single-sourced; `spec` null / `drift` false /
//!  `citation_count` 0 when there is no spec mirror and no validator plugin
//! (ii) `[workspace.spec_source]` + a section trailing the workspace revision →
//!  that row's `drift: true`, `spec` provenance surfaced, summary
//!  `drifted` / `with_excerpt` / top-level `spec_source`
//! (iii) `[plugins.set_equality_validator]` + a code site citing a section →
//!  `citation_count` / `cited_from` populated + summary `total_citations`
//! (iv) TTY (no `--json`) → the human summary block
//! (v) read-only: the store is byte-identical after a run
//! (vi) EPUB-SSOT locator (R393) surfaces per-section + `with_epub_locator`
//!  summary count, so the viewer resolves the rendered position from this one
//!  projection rather than a 2nd store read

use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn cli_binary() -> &'static str {
    env!("CARGO_BIN_EXE_mnemosyne-cli")
}

/// Build a workspace. `spec_source` writes `[workspace.spec_source]`;
/// `code_refs` writes `[plugins.set_equality_validator]` scanning `src/`.
/// `sections` is the atomic store's `sections` map (schema v7).
fn write_workspace(
    workspace: &Path,
    spec_source: Option<(&str, &str)>,
    code_refs: bool,
    sections: serde_json::Value,
) {
    fs::create_dir_all(workspace.join("docs/.atomic")).unwrap();
    fs::create_dir_all(workspace.join("src")).unwrap();
    let mut cfg = String::from(
        "[workspace]\ndocs = [\"docs/GENERATED.md\"]\ndefault_doc = \"docs/GENERATED.md\"\n\
         [schema]\nentry_id_prefix = \"Round \"\n",
    );
    if let Some((url, revision)) = spec_source {
        cfg.push_str(&format!(
            "[workspace.spec_source]\nurl = \"{url}\"\nrevision = \"{revision}\"\n"
        ));
    }
    if code_refs {
        cfg.push_str("[plugins.set_equality_validator]\npaths = [\"src/\"]\n");
    }
    fs::write(workspace.join("mnemosyne.toml"), cfg).unwrap();

    let atomic = serde_json::json!({
        "schema_version": 7,
        "sections": sections,
        "changelog_entries": {}
    });
    fs::write(
        workspace.join("docs/.atomic/workspace.atomic.json"),
        serde_json::to_string_pretty(&atomic).unwrap(),
    )
    .unwrap();
    fs::write(workspace.join("docs/GENERATED.md"), "# Stub\n").unwrap();
}

fn run_cli(workspace: &Path, args: &[&str]) -> std::process::Output {
    Command::new(cli_binary())
        .args(args)
        .current_dir(workspace)
        .output()
        .expect("cli exec")
}

fn parse_json(out: &std::process::Output) -> serde_json::Value {
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_str(&String::from_utf8_lossy(&out.stdout)).expect("valid JSON")
}

/// Locate the per-section row for `id` in the projection's `sections` array.
fn section_row<'a>(parsed: &'a serde_json::Value, id: &str) -> &'a serde_json::Value {
    parsed["sections"]
        .as_array()
        .expect("sections array")
        .iter()
        .find(|s| s["section_id"] == id)
        .unwrap_or_else(|| panic!("section row `{id}` missing"))
}

#[test]
fn mixed_store_projects_summary_and_per_section_rows() {
    let tmp = TempDir::new().unwrap();
    write_workspace(
        tmp.path(),
        None,  // no spec mirror
        false, // no validator plugin
        serde_json::json!({
            // Normative + implements binding → implemented (fact present).
            "bound": {
                "title": "Bound", "parent_doc": "docs/GENERATED.md",
                "bindings": [{"file": "src/foo.rs", "symbol": "Foo", "kind": "implements"}]
            },
            // Normative, zero bindings → normative gap.
            "gap": { "title": "Gap", "parent_doc": "docs/GENERATED.md" },
            // Informative → exempt.
            "info": {
                "title": "Terminology", "parent_doc": "docs/GENERATED.md",
                "coverage_expectation": "informative"
            },
            // Removed → excluded from the applicable denominator.
            "dead": {
                "title": "Dead", "parent_doc": "docs/GENERATED.md",
                "decision_status": "removed"
            }
        }),
    );
    let parsed = parse_json(&run_cli(tmp.path(), &["report-spec-map", "--json"]));

    // Top-level contract.
    assert!(
        parsed["spec_source"].is_null(),
        "no [workspace.spec_source]"
    );
    let summary = &parsed["summary"];
    assert_eq!(summary["total_sections"], 4);
    assert_eq!(summary["with_excerpt"], 0);
    assert_eq!(summary["coverage_ratio"], 0.5); // 1 implemented / 2 applicable
    assert_eq!(summary["by_class"]["implemented"], 1);
    assert_eq!(summary["by_class"]["normative_gap"], 1);
    assert_eq!(summary["by_class"]["informative_exempt"], 1);
    assert_eq!(summary["by_class"]["removed_excluded"], 1);
    assert_eq!(summary["drifted"], 0);
    assert_eq!(summary["total_citations"], 0);

    // Coverage class is single-sourced through `classify_coverage`.
    assert_eq!(
        section_row(&parsed, "bound")["coverage_class"],
        "implemented"
    );
    assert_eq!(
        section_row(&parsed, "gap")["coverage_class"],
        "normative_gap"
    );
    assert_eq!(
        section_row(&parsed, "info")["coverage_class"],
        "informative_exempt"
    );
    assert_eq!(
        section_row(&parsed, "dead")["coverage_class"],
        "removed_excluded"
    );

    // Per-section row field set (the visualization-consumer contract).
    let bound = section_row(&parsed, "bound");
    assert_eq!(bound["title"], "Bound");
    assert_eq!(bound["parent_doc"], "docs/GENERATED.md");
    assert_eq!(bound["decision_status"], "active");
    assert_eq!(bound["drift"], false);
    assert!(bound["spec"].is_null(), "no normative_excerpt → spec null");
    assert_eq!(bound["citation_count"], 0);
    let binding = &bound["bindings"][0];
    assert_eq!(binding["file"], "src/foo.rs");
    assert_eq!(binding["symbol"], "Foo");
    assert_eq!(binding["kind"], "implements");
}

#[test]
fn drift_flag_and_spec_provenance_surface() {
    let tmp = TempDir::new().unwrap();
    write_workspace(
        tmp.path(),
        Some(("https://www.w3.org/TR/scxml/", "2024-rec")),
        false,
        serde_json::json!({
            // Active section anchored at an older revision → drift.
            "scxml-3.13": {
                "title": "Spec section", "parent_doc": "docs/GENERATED.md",
                "normative_excerpt": {
                    "text": "the normative text",
                    "anchor_url": "https://www.w3.org/TR/scxml/#sec-x",
                    "source_revision": "2020-rec"
                }
            }
        }),
    );
    let parsed = parse_json(&run_cli(tmp.path(), &["report-spec-map", "--json"]));

    assert_eq!(parsed["spec_source"]["revision"], "2024-rec");
    assert_eq!(parsed["summary"]["with_excerpt"], 1);
    assert_eq!(parsed["summary"]["drifted"], 1);

    let row = section_row(&parsed, "scxml-3.13");
    assert_eq!(row["drift"], true);
    assert_eq!(
        row["spec"]["anchor_url"],
        "https://www.w3.org/TR/scxml/#sec-x"
    );
    assert_eq!(row["spec"]["source_revision"], "2020-rec");
}

#[test]
fn citation_density_projects_count_and_sites() {
    let tmp = TempDir::new().unwrap();
    write_workspace(
        tmp.path(),
        None,
        true, // validator plugin scanning src/
        serde_json::json!({
            "doc-spec": {
                "title": "Documented spec", "parent_doc": "docs/GENERATED.md",
                "bindings": [{"file": "src/lib.rs", "symbol": "run", "kind": "implements"}]
            }
        }),
    );
    // A code site citing the section (comment form — survives comment_only).
    fs::write(
        tmp.path().join("src/lib.rs"),
        "// implements §doc-spec\npub fn run() {}\n",
    )
    .unwrap();

    let parsed = parse_json(&run_cli(tmp.path(), &["report-spec-map", "--json"]));
    assert_eq!(parsed["summary"]["total_citations"], 1);

    let row = section_row(&parsed, "doc-spec");
    assert_eq!(row["citation_count"], 1);
    let site = &row["cited_from"][0];
    assert_eq!(site["file"], "src/lib.rs");
    assert_eq!(site["line"], 1);
}

#[test]
fn tty_output_prints_human_summary_block() {
    let tmp = TempDir::new().unwrap();
    write_workspace(
        tmp.path(),
        None,
        false,
        serde_json::json!({ "gap": { "title": "Gap", "parent_doc": "docs/GENERATED.md" } }),
    );
    let out = run_cli(tmp.path(), &["report-spec-map"]);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    for needle in [
        "=== spec map ===",
        "by class:",
        "coverage:",
        "spec-revision drift:",
        "citations:",
    ] {
        assert!(stdout.contains(needle), "missing `{needle}` in:\n{stdout}");
    }
}

#[test]
fn epub_locator_surfaces_per_section_and_in_summary() {
    let tmp = TempDir::new().unwrap();
    write_workspace(
        tmp.path(),
        None,
        false,
        serde_json::json!({
            // Full locator: spine_href + fragment + cfi.
            "located": {
                "title": "Located", "parent_doc": "docs/GENERATED.md",
                "epub_locator": {
                    "spine_href": "OEBPS/spec.xhtml",
                    "fragment": "located",
                    "cfi": "epubcfi(/6/4!/4)"
                }
            },
            // Locator without the optional cfi → cfi key omitted.
            "no-cfi": {
                "title": "No CFI", "parent_doc": "docs/GENERATED.md",
                "epub_locator": {
                    "spine_href": "OEBPS/ch2.xhtml",
                    "fragment": "no-cfi"
                }
            },
            // No EPUB mirrored → epub_locator null.
            "bare": { "title": "Bare", "parent_doc": "docs/GENERATED.md" }
        }),
    );
    let parsed = parse_json(&run_cli(tmp.path(), &["report-spec-map", "--json"]));

    assert_eq!(parsed["summary"]["with_epub_locator"], 2);

    let located = section_row(&parsed, "located");
    assert_eq!(located["epub_locator"]["spine_href"], "OEBPS/spec.xhtml");
    assert_eq!(located["epub_locator"]["fragment"], "located");
    assert_eq!(located["epub_locator"]["cfi"], "epubcfi(/6/4!/4)");

    // Absent cfi is omitted (EpubLocator's own skip_serializing_if), not null-keyed.
    let no_cfi = section_row(&parsed, "no-cfi");
    assert_eq!(no_cfi["epub_locator"]["spine_href"], "OEBPS/ch2.xhtml");
    assert!(no_cfi["epub_locator"]["cfi"].is_null());

    // No locator → the whole field is null.
    assert!(section_row(&parsed, "bare")["epub_locator"].is_null());
}

#[test]
fn report_spec_map_is_read_only() {
    let tmp = TempDir::new().unwrap();
    write_workspace(
        tmp.path(),
        None,
        false,
        serde_json::json!({ "gap": { "title": "Gap", "parent_doc": "docs/GENERATED.md" } }),
    );
    let store_path = tmp.path().join("docs/.atomic/workspace.atomic.json");
    let before = fs::read(&store_path).unwrap();
    let out = run_cli(tmp.path(), &["report-spec-map"]);
    assert!(out.status.success());
    let after = fs::read(&store_path).unwrap();
    assert_eq!(before, after, "report-spec-map must not mutate the store");
}
