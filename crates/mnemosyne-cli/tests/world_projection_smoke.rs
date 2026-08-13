//! The two gates the authored corpus could not ask, and what makes them
//! askable (Round 1070).
//!
//! `validate-render-fidelity` and `validate-disclosure-leak` have sat outside
//! every sweep this arc runs. The play-break census names them by hand as "the
//! reads this corpus cannot ask without inventing an argument", and Round 1051
//! wrote down why they survived the round that halved that list: what they need
//! is a FILE, and no vocabulary can supply a file.
//!
//! Round 1068 designed the fixture as THE AUTHORED STORE ITSELF, reasoning that
//! its fact count is not zero and that the population's road edits would move
//! the composed order under it. This file is where that design was RUN, and the
//! first half of it is false: handed the whole store, the gate refuses at every
//! world the corpus registers. The gate is single-world BY CONTRACT — it
//! classifies every fact it is handed against ONE world's composed order — and
//! the authored corpus spans four world-lines, so its siblings read as drift in
//! bulk. That verdict is about the caller; nothing in the corpus disagrees with
//! itself.
//!
//! What the gate needs is the SINGLE-WORLD PROJECTION of the store, and that
//! operation existed only inside `tools/experiment-harness`, a workspace that
//! ships to nobody: the product shipped a gate whose required input shape the
//! product could not produce. `project-world` is that operation shipped
//! (Round 1070), and these tests are the measurement — that the whole store
//! refuses, that the projection answers about something, and that a road edit
//! an author could commit MOVES the answer.
//!
//! The last one is the load-bearing test. Making an unaskable read askable is
//! only progress if the read's answer can move: a read wired to a fixture it
//! cannot disagree with turns a hole the census PRINTS into a green cell nobody
//! questions.

use std::path::Path;

use crate::common;
use common::{audit_dir, dnd_quest_manifests, run, workspace_try, Manifests};

/// Every world this corpus can be asked about — the spine plus its registered
/// forks, read from the store rather than named here.
const WORLDS: [&str; 4] = ["claim", "main", "parley", "shatter"];

/// Where a projection is written inside a workspace.
fn projection_path(ws: &Path, world: &str) -> String {
    ws.join(format!("projected-{world}.json"))
        .display()
        .to_string()
}

/// The fidelity gate's answer, or the refusal it printed instead.
fn fidelity(ws: &Path, against: &str, world: &str) -> Result<serde_json::Value, String> {
    let out = run(
        ws,
        &[
            "validate-render-fidelity",
            "--against",
            against,
            "--world",
            world,
            "--json",
        ],
    );
    if out.status.success() {
        Ok(serde_json::from_slice(&out.stdout).expect("the gate answers JSON"))
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

#[test]
fn the_whole_authored_store_is_refused_by_the_gate_at_every_world() {
    let ws = workspace_try(&dnd_quest_manifests(), Some(&audit_dir()))
        .expect("the authored corpus must load");
    let mut drawn = Vec::new();
    for world in WORLDS {
        let refusal = fidelity(ws.path(), common::SIDECAR, world)
            .expect_err("a four-world store cannot pass a single-world gate");
        assert!(
            refusal.contains("render fidelity gate FAILED"),
            "world `{world}` refused for another reason: {refusal}"
        );
        drawn.push(format!("{world}: {refusal}"));
    }
    for row in &drawn {
        println!("  {row}");
    }
    assert_eq!(
        drawn.len(),
        WORLDS.len(),
        "THE MEASUREMENT ROUND 1068 DID NOT RUN: the authored store is not the \
         fixture. Its facts span four world-lines and the gate classifies \
         against one, so every world draws its siblings off-path — a verdict \
         about the caller, since no fact in this corpus disagrees with its own \
         coordinate."
    );
}

#[test]
fn the_projection_is_what_the_gate_can_actually_be_asked_with() {
    let ws = workspace_try(&dnd_quest_manifests(), Some(&audit_dir()))
        .expect("the authored corpus must load");
    for world in WORLDS {
        let out = projection_path(ws.path(), world);
        let emitted = run(
            ws.path(),
            &["project-world", "--world", world, "--out", &out, "--json"],
        );
        assert!(
            emitted.status.success(),
            "project-world {world}: {}",
            String::from_utf8_lossy(&emitted.stderr)
        );
        let emitted: serde_json::Value =
            serde_json::from_slice(&emitted.stdout).expect("the projection answers JSON");

        let report = fidelity(ws.path(), &out, world)
            .unwrap_or_else(|e| panic!("the projected store must pass at `{world}`: {e}"));
        // NOT ZERO, and not asserted as a constant: the count the gate reports
        // is the count the projection kept, which is what makes "askable" mean
        // "askable about something". A fixture with no facts is a clean report
        // about nothing — askable and inert, which is worse than unaskable.
        assert_eq!(
            report["reextracted_facts"], emitted["kept"],
            "the gate reports the projection's own fact count"
        );
        assert!(
            report["reextracted_facts"].as_u64().expect("a count") > 0,
            "the projection for `{world}` is empty, so the gate would answer \
             clean about nothing"
        );
        assert!(
            emitted["dropped"].as_u64().expect("a count") > 0,
            "nothing was dropped for `{world}`, so this store never spanned \
             more than one world-line and the refusal above had another cause"
        );
        assert_eq!(report["off_path"].as_array().expect("a list").len(), 0);
        assert_eq!(report["unplaced"].as_array().expect("a list").len(), 0);
        println!(
            "  {world}: kept {} dropped {} -> clean, terminal={}",
            emitted["kept"], emitted["dropped"], report["reached_terminal"]
        );
    }
}

/// The baseline workspace and the frozen single-world projection of `world`,
/// written inside it. FROZEN is the point: the prose a world was rendered from
/// does not change because its road did, which is what leaves the gate anything
/// to compare.
fn baseline_with_projection(world: &str) -> (tempfile::TempDir, String) {
    let ws = workspace_try(&dnd_quest_manifests(), Some(&audit_dir()))
        .expect("the authored corpus must load");
    let out = projection_path(ws.path(), world);
    let emitted = run(
        ws.path(),
        &["project-world", "--world", world, "--out", &out, "--json"],
    );
    assert!(
        emitted.status.success(),
        "the baseline projection is written: {}",
        String::from_utf8_lossy(&emitted.stderr)
    );
    let clean = fidelity(ws.path(), &out, world).expect("the baseline projection passes");
    assert_eq!(clean["off_path"].as_array().expect("a list").len(), 0);
    assert_eq!(clean["unplaced"].as_array().expect("a list").len(), 0);
    (ws, out)
}

/// The gate's report in `ws`, read whether it exited 0 or not — the report is
/// printed before the exit code is decided, and both arms below need it.
fn fidelity_report(ws: &Path, against: &str, world: &str) -> (serde_json::Value, bool) {
    let out = run(
        ws,
        &[
            "validate-render-fidelity",
            "--against",
            against,
            "--world",
            world,
            "--json",
        ],
    );
    let rejected = !out.status.success();
    (
        serde_json::from_slice(&out.stdout).expect("the gate answers JSON before it exits"),
        rejected,
    )
}

#[test]
fn a_road_edit_that_orphans_a_scene_is_reported_and_not_rejected() {
    let (_baseline, against) = baseline_with_projection("shatter");

    // THE EDIT: shatter's road steps straight from sc-23s to sc-25s, so sc-24s
    // is named by no world's declaration at all. It goes in through the same
    // import an author runs, so an edit the write path rejects would come back
    // as a rejection rather than be smuggled in.
    let mut manifests = dnd_quest_manifests();
    let replaced = drop_scene_from_road(&mut manifests, "shatter", "sc-24s");
    assert_eq!(
        replaced, 2,
        "the two edges that walked through sc-24s became one"
    );
    let edited = workspace_try(&manifests, Some(&audit_dir()))
        .expect("skipping a scene is an edit an author could commit");

    let (report, rejected) = fidelity_report(edited.path(), &against, "shatter");
    println!("  orphaned: {report}");
    assert_eq!(
        coords(&report, "unplaced"),
        ["sc-24s", "sc-24s"],
        "the frozen prose still stands at a scene no road walks any more, and \
         the gate names it"
    );
    assert!(coords(&report, "off_path").is_empty());
    // THE HONEST HALF. `unplaced` is a surface, not a verdict — only `off_path`
    // exits non-zero. So this edit MOVES the answer without REJECTING it, which
    // is exactly the distinction a census bucket rests on: a read that resizes
    // a list has somewhere to stand, and whether to gate is then policy with a
    // named owner rather than a missing capability.
    assert!(
        !rejected,
        "an unplaceable coordinate is the gate's honesty surface, never its \
         FAIL signal — a test that expected a rejection here would be pinning \
         a behaviour the gate deliberately does not have"
    );
}

#[test]
fn a_road_edit_that_hands_a_scene_to_a_sibling_road_is_rejected() {
    let (_baseline, against) = baseline_with_projection("shatter");

    // THE SAME SCENE, moved rather than orphaned: `claim` walks it now. It is a
    // declaration node of ANOTHER world and not of this one, which is the
    // drift the gate exists to reject.
    let mut manifests = dnd_quest_manifests();
    drop_scene_from_road(&mut manifests, "shatter", "sc-24s");
    append_scene_to_road(&mut manifests, "claim", "sc-24s");
    let edited = workspace_try(&manifests, Some(&audit_dir()))
        .expect("moving a scene between roads is an edit an author could commit");

    let (report, rejected) = fidelity_report(edited.path(), &against, "shatter");
    println!("  handed over: {report}");
    assert_eq!(
        coords(&report, "off_path"),
        ["sc-24s", "sc-24s"],
        "the prose drifted onto the world-line that now owns the scene"
    );
    assert!(coords(&report, "unplaced").is_empty());
    assert!(
        rejected,
        "off-path IS the FAIL signal — this is the arm that makes the gate a \
         gate rather than a report"
    );
}

/// The coordinates one list of the fidelity report names.
fn coords(report: &serde_json::Value, list: &str) -> Vec<String> {
    report[list]
        .as_array()
        .expect("a list")
        .iter()
        .map(|f| f["coord"].as_str().expect("a coord").to_string())
        .collect()
}

/// Rewrite `road` so it steps straight past `scene`, and return how many edges
/// were replaced. The scene keeps its registry entry — an author who cut a beat
/// out of one world's walk leaves the scene declared.
fn drop_scene_from_road(manifests: &mut Manifests, road: &str, scene: &str) -> usize {
    let edges = manifests.order["branches"][road]
        .as_array_mut()
        .expect("the road declares edges");
    let into: Vec<String> = edges
        .iter()
        .filter(|e| e[1] == scene)
        .map(|e| e[0].as_str().expect("a node").to_string())
        .collect();
    let out_of: Vec<String> = edges
        .iter()
        .filter(|e| e[0] == scene)
        .map(|e| e[1].as_str().expect("a node").to_string())
        .collect();
    assert_eq!(
        (into.len(), out_of.len()),
        (1, 1),
        "`{scene}` is a single step on `{road}`, so skipping it is one edge"
    );
    let before = edges.len();
    edges.retain(|e| e[0] != scene && e[1] != scene);
    edges.push(serde_json::json!([into[0], out_of[0]]));
    before - (edges.len() - 1)
}

/// Extend `road` with `scene` at its far end — the road's own terminal, derived
/// as the node its edges reach and never leave.
fn append_scene_to_road(manifests: &mut Manifests, road: &str, scene: &str) {
    let edges = manifests.order["branches"][road]
        .as_array_mut()
        .expect("the road declares edges");
    let sources: Vec<&str> = edges
        .iter()
        .map(|e| e[0].as_str().expect("a node"))
        .collect();
    let mut ends: Vec<String> = edges
        .iter()
        .map(|e| e[1].as_str().expect("a node"))
        .filter(|node| !sources.contains(node))
        .map(ToString::to_string)
        .collect();
    ends.sort();
    ends.dedup();
    assert_eq!(
        ends.len(),
        1,
        "`{road}` has {} ends, so `its terminal` is not derivable",
        ends.len()
    );
    edges.push(serde_json::json!([ends[0], scene]));
}
