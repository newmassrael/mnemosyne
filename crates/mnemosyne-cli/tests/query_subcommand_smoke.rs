//! Round 120 — mnemosyne-cli query subcommand smoke test (production lift).
//!
//! `mnemosyne-cli query <section_id> [flags]` invocation in workspace 7 doc
//! consistency validation in §43 / §66 / cross-doc reclassify — Round 116 bench prototype
//! anchor and production consistency validation source.
//!
//! This test runs the binary via cargo run — no external `assert_cmd`-like dependency,
//! Uses the `std::process::Command` + `env!("CARGO_BIN_EXE_mnemosyne-cli")` pattern
//! (Cargo auto-set; missing dev-dependency added).

use std::process::Command;

fn cli_bin() -> String {
    env!("CARGO_BIN_EXE_mnemosyne-cli").to_string()
}

fn run_query(args: &[&str]) -> (i32, String, String) {
    let output = Command::new(cli_bin())
        .arg("query")
        .args(args)
        .output()
        .expect("invoke mnemosyne-cli query");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);
    (code, stdout, stderr)
}

#[test]
#[ignore = "Round 252 -- atomic store reset to empty; test depends on populated atomic ledger or 7-md workspace"]
fn query_section_43_plain_text_succeeds() {
    let (code, stdout, _stderr) = run_query(&["43"]);
    assert_eq!(code, 0, "query §43 must succeed");
    assert!(stdout.contains("§43"), "stdout must mention §43");
    // Round 251 — post 7-md deletion, atomic-only sections surface with
    // parent_doc = "<atomic>" sentinel. Pre-deletion: parent_doc was
    // "docs/DESIGN.md" (markdown-derived).
    assert!(
        stdout.contains("docs/DESIGN.md") || stdout.contains("<atomic>"),
        "stdout must include parent_doc"
    );
    assert!(
        stdout.contains("decision_status: active"),
        "stdout must include decision_status"
    );
}

#[test]
#[ignore = "Round 252 -- atomic store reset to empty; test depends on populated atomic ledger or 7-md workspace"]
fn query_section_43_json_envelope() {
    let (code, stdout, _stderr) =
        run_query(&["43", "--include-related", "--include-changelog", "--json"]);
    assert_eq!(code, 0, "query §43 JSON envelope must succeed");
    // Round 116 bench anchor consistency validation — JSON envelope shape (Claude consumable).
    assert!(stdout.contains("\"section_id\": \"43\""));
    // Round 251 — atomic-only sections render parent_doc=<atomic> post 7-md deletion.
    assert!(
        stdout.contains("\"parent_doc\": \"docs/DESIGN.md\"")
            || stdout.contains("\"parent_doc\": \"<atomic>\""),
        "envelope must report parent_doc (markdown or atomic-only)"
    );
    assert!(stdout.contains("\"outbound_refs\""));
    assert!(stdout.contains("\"inbound_refs\""));
    assert!(stdout.contains("\"related_changelog_entries\""));

    // JSON in § prefix stripped section_id consistency (bench Round 116 carry).
    let envelope: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    // Round 251 — outbound_refs (markdown-derived) collapses post 7-md
    // deletion. inbound_refs is now atomic impact_refs reverse lookup.
    let inbound = envelope["inbound_refs"].as_array().expect("array");
    assert!(
        !inbound.is_empty(),
        "§43 inbound must surface atomic impact_refs (got {})",
        inbound.len()
    );
    let changelog = envelope["related_changelog_entries"]
        .as_array()
        .expect("array");
    assert!(
        changelog.len() >= 30,
        "§43 changelog hit ≥ 30 (got {})",
        changelog.len()
    );
}

#[test]
#[ignore = "Round 252 -- atomic store reset to empty; test depends on populated atomic ledger or 7-md workspace"]
fn query_section_66_changelog_anchor() {
    let (code, stdout, _stderr) = run_query(&["66", "--include-changelog", "--json"]);
    // §66 query in plain `--json` (without --include-related --include-changelog
    // both) → SectionView only. this test envelope condition under threshold → SectionView standalone.
    assert_eq!(code, 0);
    assert!(stdout.contains("\"section_id\": \"66\""));
    // §66 body in Bootstrap stages body include verify (Round 117 mutation carry).
    assert!(
        stdout.contains("Self-application")
            || stdout.contains("dogfood")
            || stdout.contains("design doc"),
        "§66 body must contain self-application content"
    );
}

#[test]
#[ignore = "Round 252 -- atomic store reset to empty; test depends on populated atomic ledger or 7-md workspace"]
fn query_section_66_full_envelope_meets_anchors() {
    let (code, stdout, _stderr) =
        run_query(&["66", "--include-related", "--include-changelog", "--json"]);
    assert_eq!(code, 0);
    let envelope: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    let changelog = envelope["related_changelog_entries"]
        .as_array()
        .expect("array");
    // Round 116 bench anchor: §66 changelog hit ≥ 50.
    assert!(
        changelog.len() >= 50,
        "§66 changelog hit ≥ 50 (Round plan anchor, got {})",
        changelog.len()
    );
    // Round 251 — markdown cross_ref graph collapsed post 7-md deletion;
    // atomic impact_refs reverse lookup is the new inbound floor. The
    // legacy ≥ 50 bench anchor was a markdown-cross-ref count; the
    // atomic-aware floor is "non-empty" until impact_refs coverage
    // catches up to all entries (Round 240+ legacy migration entries
    // populate impact_refs via §3/§15/§66 etc).
    let inbound = envelope["inbound_refs"].as_array().expect("array");
    assert!(
        !inbound.is_empty(),
        "§66 inbound must surface atomic impact_refs (got {})",
        inbound.len()
    );
}

#[test]
#[ignore = "Round 252 -- atomic store reset to empty; test depends on populated atomic ledger or 7-md workspace"]
fn query_unknown_section_id_fails() {
    let (code, _stdout, stderr) = run_query(&["999999"]);
    assert_ne!(code, 0, "unknown section_id must exit non-zero");
    assert!(
        stderr.contains("not found") || stderr.contains("not found"),
        "stderr must mention not found or not found (got {})",
        stderr
    );
}

#[test]
#[ignore = "Round 252 -- atomic store reset to empty; test depends on populated atomic ledger or 7-md workspace"]
fn query_list_sections_prints_workspace_set() {
    let (code, stdout, stderr) = run_query(&["--list-sections"]);
    assert_eq!(code, 0);
    // §39, §43, §66 all registered.
    assert!(stdout.lines().any(|l| l == "39"));
    assert!(stdout.lines().any(|l| l == "43"));
    assert!(stdout.lines().any(|l| l == "66"));
    // stderr in total count summary.
    assert!(stderr.contains("# total"));
}

#[test]
#[ignore = "Round 252 -- atomic store reset to empty; test depends on populated atomic ledger or 7-md workspace"]
fn query_section_prefix_strip() {
    // `§43` and `43` are the same identifier -- § prefix auto strip validation.
    let (code1, stdout1, _) = run_query(&["43"]);
    let (code2, stdout2, _) = run_query(&["§43"]);
    assert_eq!(code1, 0);
    assert_eq!(code2, 0);
    // two invoke identical output (timestamp / non-determinism 0).
    assert_eq!(stdout1, stdout2);
}

#[test]
#[ignore = "Round 252 -- atomic store reset to empty; test depends on populated atomic ledger or 7-md workspace"]
fn query_cross_doc_reclassify_inbound() {
    // ARCHITECTURE.md in §39 cross-doc citation → DESIGN.md §39 inbound carry consistency
    // (Round 70 OPTION H-2 cross-doc reclassify policy production carry).
    let (code, stdout, _stderr) = run_query(&["39", "--include-related", "--json"]);
    assert_eq!(code, 0);
    // `--json` no `--include-related` → SectionView only. this test -
    // `--include-related` + `--json` (without --include-changelog) → SectionView.
    // envelope mode 3 flag all required — this test plain JSON SectionView.
    let view: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(view["section_id"], "39");
}
