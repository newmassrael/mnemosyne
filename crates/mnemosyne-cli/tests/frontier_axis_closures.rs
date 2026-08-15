//! The frontier says WHAT TO CALL, and the roster that says it cannot drift
//! from the report it describes.
//!
//! Round 1218. `report-authoring-frontier` is the read an unattended loop pulls
//! its next work from, and until this round it published what is outstanding
//! and never what to call about it. Rounds 1214, 1216 and 1217 each closed one
//! axis by hand, and each wrote the same carry — the loop reads an axis NAME and
//! learns the verb from a person. A sentence three rounds keep writing is undone
//! work.
//!
//! The failure this guards against is not "the roster is incomplete". It is the
//! shape a table beside an API always takes: the struct grows, the prose does
//! not, and nothing is red. The MCP description of this very report had drifted
//! that way for four fields across many rounds (R1217 found it). So the roster
//! is held against the keys the tool ACTUALLY EMITS, asked of the tool at the
//! process boundary rather than read off a type — two calls, one with a telling
//! and one without, because the conditional axes are absent from the first.
//!
//! What the roster may CLAIM is bounded too. `closes` means this repository has
//! run that call over its authored corpora, and the three laws in
//! `frontier_closure` take their verb from here so the declaration is the thing
//! under test. Everything else is `believed`, `no_verb` or `not_work`, and the
//! count of `closes` rows is pinned: a fourth appears only when a law arrives to
//! prove it.

use std::collections::BTreeSet;
use std::path::Path;

use crate::common;
use common::{authored_stores, run, run_ok};

/// The roster, as the CLI publishes it. Static — the workspace is irrelevant,
/// which is itself part of the contract (a loop reads it once per session).
pub fn axes(at: &Path) -> Vec<serde_json::Value> {
    let out = run_ok(at, &["describe-frontier-axes", "--json"]);
    serde_json::from_str::<serde_json::Value>(&out)
        .expect("the roster is json")
        .get("axes")
        .and_then(|a| a.as_array())
        .cloned()
        .expect("the roster carries an `axes` list")
}

/// The verb the roster says closes `field`, or `None` when it declares no
/// proven one. THE ONE READER for the closure laws, so a renamed verb moves the
/// laws with it.
pub fn closing_verb(at: &Path, field: &str) -> Option<String> {
    axes(at)
        .into_iter()
        .find(|a| a["field"] == field)
        .and_then(|a| match a["closure"]["kind"].as_str() {
            Some("closes") => a["closure"]["verb"].as_str().map(ToString::to_string),
            _ => None,
        })
}

/// Every key the frontier tool emits, asked of the tool: the union over a call
/// with no telling and a call with one, since the telling-scoped axes are
/// skipped entirely in the first.
fn emitted_fields(workspace: &Path, telling: &str) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    for args in [
        vec!["report-authoring-frontier", "--json"],
        vec!["report-authoring-frontier", "--json", "--telling", telling],
    ] {
        let out = run(workspace, &args);
        assert!(
            out.status.success(),
            "{args:?} must answer: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let report: serde_json::Value =
            serde_json::from_slice(&out.stdout).expect("the frontier emits json");
        keys.extend(
            report
                .as_object()
                .expect("the report is an object")
                .keys()
                .cloned(),
        );
    }
    keys
}

/// A store carrying every axis at once is not needed — only one that answers
/// both calls, so the union covers the conditional fields.
fn a_store_with_a_telling() -> tempfile::TempDir {
    let (stores, _) = authored_stores();
    let store = stores
        .into_iter()
        .find(|s| {
            let report = run(s.ws.path(), &["report-authoring-frontier", "--json"]);
            report.status.success()
                && serde_json::from_slice::<serde_json::Value>(&report.stdout)
                    .ok()
                    .and_then(|r| r["tellings_declared"].as_array().map(|t| !t.is_empty()))
                    .unwrap_or(false)
        })
        .expect("the authored population has a store that declares a telling");
    store.ws
}

#[test]
fn the_roster_names_exactly_the_fields_the_frontier_emits() {
    let ws = a_store_with_a_telling();
    let declared: BTreeSet<String> = axes(ws.path())
        .iter()
        .map(|a| {
            a["field"]
                .as_str()
                .expect("every row names a field")
                .to_string()
        })
        .collect();
    // The telling id comes from the store rather than being invented: a typo'd
    // telling is fail-loud, and this call has to succeed to contribute keys.
    let report: serde_json::Value =
        serde_json::from_str(&run_ok(ws.path(), &["report-authoring-frontier", "--json"]))
            .expect("frontier json");
    let telling = report["tellings_declared"][0]
        .as_str()
        .expect("the store declares one")
        .to_string();
    let emitted = emitted_fields(ws.path(), &telling);

    let missing: Vec<&String> = emitted.difference(&declared).collect();
    let invented: Vec<&String> = declared.difference(&emitted).collect();
    assert!(
        missing.is_empty(),
        "the frontier emits {missing:?} and the roster says nothing about it — a loop reading \
         that field learns nothing about what to call"
    );
    assert!(
        invented.is_empty(),
        "the roster names {invented:?} and the frontier emits no such field — a contract \
         describing a field that does not exist is worse than silence"
    );
    println!("  {} field(s), each with a closure row", emitted.len());
}

#[test]
fn every_verb_the_roster_names_is_a_verb_this_binary_has() {
    let ws = a_store_with_a_telling();
    let help = run_ok(ws.path(), &["--help"]);
    let mut named = 0usize;
    for axis in axes(ws.path()) {
        let Some(verb) = axis["closure"]["verb"].as_str() else {
            continue;
        };
        named += 1;
        assert!(
            help.contains(&format!(" {verb} ")) || help.contains(&format!(" {verb}\n")),
            "the roster tells a loop to call `{verb}` for `{}`, and this binary's help does \
             not list it — the loop would be handed a command that does not exist",
            axis["field"]
        );
    }
    assert!(
        named >= 3,
        "a roster that names no verb proves nothing about verbs: {named} named"
    );
}

#[test]
fn only_a_call_this_repository_has_run_is_published_as_proven() {
    let ws = a_store_with_a_telling();
    let mut proven: Vec<String> = Vec::new();
    let mut believed = 0usize;
    let mut no_verb: Vec<String> = Vec::new();
    let mut not_work = 0usize;
    for axis in axes(ws.path()) {
        let field = axis["field"].as_str().unwrap_or_default().to_string();
        match axis["closure"]["kind"].as_str() {
            Some("closes") => proven.push(field),
            Some("believed") => believed += 1,
            Some("no_verb") => no_verb.push(field),
            Some("not_work") => not_work += 1,
            other => panic!("`{field}` carries an unknown closure kind {other:?}"),
        }
    }
    proven.sort();
    println!("  proven {proven:?}, believed {believed}, no verb {no_verb:?}, not work {not_work}");
    // PINNED, because `closes` is a claim about what this suite RUNS. These are
    // the axes Rounds 1214, 1216, 1217 and 1219 closed over the authored
    // corpora, and each of those laws reads its verb from the roster. A sixth
    // may only appear beside a law that proves it.
    assert_eq!(
        proven,
        vec![
            "dangling_setups".to_string(),
            "never_planned_disclosures".to_string(),
            "telling_needed".to_string(),
            "unresolved_quests".to_string(),
            "zero_fact_scenes".to_string()
        ],
        "the proven set changed: either a law arrived (say so here) or a call was published \
         as run when nothing runs it"
    );
    assert!(
        believed > 0,
        "the believed state has to occur or the distinction between run and believed is untested"
    );
    assert_eq!(
        no_verb,
        vec!["unordered_scenes".to_string()],
        "the axis a loop cannot close by calling anything is the one thing here it must be \
         told about by name"
    );
}

#[test]
fn the_axis_with_no_verb_is_one_the_work_gauge_still_counts() {
    // WHY THE `no_verb` STATE EXISTS AT ALL. An axis nobody can close would be
    // harmless if it were also uncounted — a loop reading `total_gaps` would
    // finish. This one is counted, so a loop that treats every gap as closable
    // spins on it forever, and from outside that is indistinguishable from work.
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let ws = tmp.path();
    std::fs::create_dir_all(ws.join("docs/.atomic")).expect("mkdir");
    std::fs::write(ws.join("mnemosyne.toml"), "[workspace]\n").expect("config");
    std::fs::write(
        ws.join("docs/.atomic/workspace.atomic.json"),
        r#"{"schema_version":23,"sections":{},"changelog_entries":{}}"#,
    )
    .expect("seed");
    std::fs::write(
        ws.join("sections.json"),
        r#"[{"section_id":"sc-01","parent_doc":"d","title":"The well",
             "coverage_expectation":"informational"}]"#,
    )
    .expect("sections");
    std::fs::write(
        ws.join("facts.json"),
        r#"{"frames":[{"frame_id":"gt"}],
            "facts":[{"fact_id":"f0","frame":"gt","claim":"The well is deep",
                      "canon_from":"sc-01","evidence":["sc-01"]}]}"#,
    )
    .expect("facts");
    run_ok(ws, &["import-sections", "--manifest", "sections.json"]);
    run_ok(ws, &["import-facts", "--manifest", "facts.json"]);

    let report: serde_json::Value =
        serde_json::from_str(&run_ok(ws, &["report-authoring-frontier", "--json"]))
            .expect("frontier json");
    assert_eq!(
        report["unordered_scenes"],
        serde_json::json!(["sc-01"]),
        "with no canon order declared, a fact-bearing scene is unordered"
    );
    assert_eq!(
        report["total_gaps"].as_u64(),
        Some(1),
        "and it is counted as work: that is what makes `no_verb` a warning rather than a note"
    );
}
