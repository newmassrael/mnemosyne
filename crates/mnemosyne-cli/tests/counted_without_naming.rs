//! Which shipped reads answer a change with a NUMBER and nothing else?
//! (Round 1054.)
//!
//! Round 1053 changed a shipped wire because a contract could not be written
//! over it. The authoring frontier's scene census carried a COUNT, so two facts
//! trading the coordinates they are anchored at left the entire report
//! byte-identical, and no test over that read's output could have judged
//! identity. The finding was pinned in the PAIR file that happened to meet it —
//! and what it judges is a property of ONE read. The same shape had been
//! recorded once already, on `report-frame-view`'s `not_holding`, and written
//! down as a limit. A shape met twice by hand is a population nobody has asked
//! the program for; asking the whole read surface a hand-met question is what
//! Round 1049 did, and it came back with shipping defects.
//!
//! THE LAW. Take an edit an author could commit, and one read's answer before
//! and after. If the answer moved and EVERY moving thing in it is a number, that
//! number is the read's whole account of the change: a consumer learns that
//! something moved and cannot learn what. That is the defect. A count that rides
//! beside the list it counts is NOT in the class and this walk must not flag it —
//! `holding_count` moves only when `holding` does — and buying that
//! discrimination from a measurement rather than from a rule about field names is
//! the whole point. Round 1039 already measured what naming rules are worth here:
//! a field's vocabulary "is a fact about its TYPE, and no amount of looking at
//! output recovers it".
//!
//! THE POPULATIONS ARE BOTH DERIVED, AND NEITHER IS THIS FILE'S.
//!
//! - The reads: [`common::panel`], the shipped `--help` asked for every required
//!   argument the corpus can supply — 30 reads over 68 questions (R1051). A
//!   verdict here is about the READ, so the questions one verb answers are
//!   unioned: a number is named if SOME question names it.
//! - The edits: [`common::corruptions`], the store's own legs (R1033). This walk
//!   adds no edit of its own, which is what keeps it from finding the defect it
//!   went looking for.
//!
//! THE FIRST SWEEP CAME BACK EMPTY, AND THE POPULATION WAS WHY. Not one number
//! on the whole surface moved alone, and the reason was not that the surface is
//! clean: `corruptions` says it takes "the legs a fact ACTUALLY carries" and
//! carried only the ones saying what a fact CLAIMS — its typed object, its
//! predicate, its payoff edges, its evidence. Every count a shipped read emits
//! summarizes WHERE a fact is, WHOSE view holds it, or WHICH world-line authored
//! it, and no edit in that population touched any of the three. So the legs that
//! place a fact and attribute it were added where the population lives (41
//! corruptions became 92, none refused by the write path), and the walk was run
//! again. An empty answer from a probe that cannot reach the thing it asks about
//! is the failure this arc has now met three times.
//!
//! WHAT THE LAW IS CONSERVATIVE ABOUT, stated because the walk counts it: a
//! number can be lossy and still escape, when the same edit moves something named
//! elsewhere in the same answer for an unrelated reason. So every flag raised is
//! sound and the coverage is a number this walk prints (`accompanied`) rather
//! than a claim. Narrowing that would mean scoping the comparison to the record
//! (R1039) — a real next step, and not one to take before a sweep says whether it
//! is needed.
//!
//! WHAT THE SECOND SWEEP FOUND is in [`COUNTED_WITHOUT_NAMING`], and this round
//! did not stop at the census: both wires it named now name what they count. The
//! third member of the class is one this walk CANNOT reach, and
//! [`the_frame_view_names_the_facts_it_calls_not_holding`] both proves it and
//! says why the sweep is blind to it.

use std::collections::{BTreeMap, BTreeSet};

use mnemosyne_atomic::AtomicStore;

mod common;
use common::{
    ask_panel, corruptions, dnd_quest_facts, dnd_quest_workspace_from, dnd_quest_workspace_try,
    panel, registered_ids, telling_of, Answer, SIDECAR,
};

/// Every number in one answer, keyed by the path it sits at, with array indices
/// and id-valued map keys collapsed so the key names a FIELD of the read.
///
/// The values are kept as a list in document order rather than summed: a field
/// whose rows trade values has moved, and a total would say it had not — which
/// is the very confusion this walk exists to find.
fn numbers(
    value: &serde_json::Value,
    path: &str,
    ids: &BTreeSet<String>,
) -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    collect_numbers(value, path, ids, &mut out);
    out
}

fn collect_numbers(
    value: &serde_json::Value,
    path: &str,
    ids: &BTreeSet<String>,
    out: &mut BTreeMap<String, Vec<String>>,
) {
    match value {
        serde_json::Value::Number(n) => {
            out.entry(path.to_string()).or_default().push(n.to_string())
        }
        serde_json::Value::Array(items) => {
            let path = format!("{path}[]");
            for item in items {
                collect_numbers(item, &path, ids, out);
            }
        }
        serde_json::Value::Object(map) => {
            for (key, child) in map {
                let key = if ids.contains(key) { "*" } else { key.as_str() };
                let child_path = if path.is_empty() {
                    key.to_string()
                } else {
                    format!("{path}.{key}")
                };
                collect_numbers(child, &child_path, ids, out);
            }
        }
        _ => {}
    }
}

/// The answer with every number blanked — everything it NAMES, and the shape it
/// named it in. Two answers equal here differ in numbers alone.
fn named(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Number(_) => serde_json::Value::Null,
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.iter().map(named).collect())
        }
        serde_json::Value::Object(map) => {
            serde_json::Value::Object(map.iter().map(|(k, v)| (k.clone(), named(v))).collect())
        }
        other => other.clone(),
    }
}

/// The fields this walk reports as counting what their read does not name, as
/// `verb path`. A list, not a total: an exclusion nobody can read is an
/// exclusion nobody removes (the R1029 rule).
///
/// EMPTY IS THE SHIPPED STATE, and it is a gate rather than a green tick: the
/// walk asks every advertised read, so a read that grows a lossy count is caught
/// the run it ships. What the first sweep found, and this round fixed:
///
/// - `report-authoring-frontier branch_owned_density.*.owned_facts` and the
///   `density` derived from it — moving one fact to another world-line moved
///   both and nothing else in the whole report. Rounds 1052 and 1053 recorded
///   this from the other side as a PAIR's limit ("`owned_facts` is what this
///   pair cannot see"); it was never a limit of a pair.
/// - `report-payoff-coverage worlds.*.exempt` — the one class of a four-way
///   classification whose members no field carried. Its own doc comment said
///   so out loud, in the words "counted, not listed".
const COUNTED_WITHOUT_NAMING: [&str; 0] = [];

#[test]
fn the_reads_that_count_what_they_do_not_name() {
    let facts_json = dnd_quest_facts();
    let ws = dnd_quest_workspace_from(&facts_json);
    let store = AtomicStore::load(&ws.path().join(SIDECAR)).expect("the imported store loads");
    let ids = registered_ids(&store);
    let telling = telling_of(&store);
    let (panel, unaskable) = panel(ws.path(), &telling);
    let baseline = ask_panel(ws.path(), &panel);
    assert!(
        baseline.failed.is_empty(),
        "the panel is exactly the reads that answered at baseline: {:?}",
        baseline.failed
    );
    let verb_of: BTreeMap<String, String> = panel
        .iter()
        .map(|read| (read.label(), read.verb.clone()))
        .collect();

    // (verb, field) -> the edits under which that number moved ALONE. Keyed by
    // the READ, so a verb asked several questions is judged by all of them: a
    // number is named if SOME question the corpus can ask names it.
    let mut alone: BTreeMap<(String, String), BTreeSet<String>> = BTreeMap::new();
    // The same, for the numbers that moved WITH something named — the measure of
    // how much of the surface the law could have flagged and did not.
    let mut accompanied: BTreeMap<(String, String), BTreeSet<String>> = BTreeMap::new();
    // (verb, edit) pairs where the answer did not move at all. A read blind to an
    // edit is out of this walk's reach, and the number says how far out.
    let mut still = 0usize;
    let mut prose_comparisons = 0usize;

    let population = corruptions(&store, &facts_json);
    let mut applied = 0usize;
    let mut refused = 0usize;
    for corruption in &population {
        let mut mutated = facts_json.clone();
        let mut hits = 0usize;
        for entry in mutated["facts"].as_array_mut().expect("facts array") {
            if entry["fact_id"] == corruption.fact.as_str() {
                (corruption.apply)(entry);
                hits += 1;
            }
        }
        assert_eq!(
            hits, 1,
            "{}/{} applied {hits} times",
            corruption.fact, corruption.leg
        );
        let Ok(mutated_ws) = dnd_quest_workspace_try(&mutated) else {
            // The write path refuses it: not a move an author could make.
            refused += 1;
            continue;
        };
        applied += 1;
        let edit = format!("{}/{}", corruption.fact, corruption.leg);
        let seen = ask_panel(mutated_ws.path(), &panel);
        for (label, before) in &baseline.answers {
            let verb = verb_of[label].clone();
            let (Answer::Json(before), Some(Answer::Json(after))) =
                (before, seen.answers.get(label))
            else {
                // A read that answers `--json` in prose holds no fields to key
                // on. Counted rather than curated away (the R1029 rule).
                prose_comparisons += 1;
                continue;
            };
            let (was, now) = (numbers(before, "", &ids), numbers(after, "", &ids));
            let moved: BTreeSet<String> = was
                .keys()
                .chain(now.keys())
                .filter(|path| was.get(*path) != now.get(*path))
                .cloned()
                .collect();
            if moved.is_empty() {
                if named(before) == named(after) {
                    still += 1;
                }
                continue;
            }
            let into = if named(before) == named(after) {
                &mut alone
            } else {
                &mut accompanied
            };
            for path in moved {
                into.entry((verb.clone(), path))
                    .or_default()
                    .insert(edit.clone());
            }
        }
    }

    // Print BEFORE asserting (the R1026 lesson): the distribution is the finding,
    // and a first-violation stop would report one line of it.
    println!(
        "{} reads over {} questions, {} UNASKABLE; {} corruptions applied, {} refused by the \
         write path; {prose_comparisons} comparisons against a prose answer; {still} (read, edit) \
         pairs the read did not move for at all\n",
        verb_of.values().collect::<BTreeSet<_>>().len(),
        panel.len(),
        unaskable.len(),
        applied,
        refused,
    );
    for (verb, reason) in &unaskable {
        println!("  UNASKABLE {verb}: {reason}");
    }
    println!("\nCOUNTED WITHOUT NAMING — the number moved and nothing named did:");
    for ((verb, path), edits) in &alone {
        println!("  {verb} {path}");
        for edit in edits {
            println!("        {edit}");
        }
    }
    println!(
        "\nnumbers that moved WITH something named ({} fields) — the law's negative answers:",
        accompanied.len(),
    );
    for ((verb, path), edits) in &accompanied {
        println!("  {:4} edits  {verb} {path}", edits.len());
    }

    let mut broken: Vec<String> = Vec::new();
    let mut check = |ok: bool, claim: &str| {
        if !ok {
            broken.push(claim.to_string());
        }
    };

    check(
        (
            verb_of.values().collect::<BTreeSet<_>>().len(),
            panel.len(),
            applied,
            refused,
        ) == (30, 68, 92, 0),
        "INPUTS: both populations come from `common`, so this walk cannot narrow \
         either to suit itself — 30 reads over 68 questions, and 92 authorable \
         corruptions with none refused. The corruption count was 41 until this \
         round: that derivation says it takes the legs a fact ACTUALLY carries \
         and carried only the ones saying what a fact CLAIMS, so the first sweep \
         over this law came back empty and could not have come back otherwise",
    );
    check(
        alone
            .keys()
            .map(|(verb, path)| format!("{verb} {path}"))
            .collect::<Vec<_>>()
            == COUNTED_WITHOUT_NAMING,
        "THE CENSUS: every field of every shipped read that an authorable edit \
         moves ON ITS OWN, named. Round 1053 met this shape on the frontier's \
         scene census and Round 1050 on `report-frame-view`'s `not_holding`; \
         both were written down where they were met, and neither asked the \
         surface",
    );
    check(
        accompanied
            .keys()
            .any(|(verb, path)| verb == "report-frame-view" && path == "holding_count"),
        "NON-VACUITY: the law says NO as well as yes, and it says no to the \
         canonical shape it must not flag — `holding_count` rides beside the \
         entries it counts (R435) and moves only when they do, which no rule \
         about a field's name could tell apart from the `not_holding` this round \
         had to change",
    );
    check(
        (still, prose_comparisons, accompanied.len()) == (5195, 216, 10),
        "REACH, ASSERTED RATHER THAN IMPLIED: of the 6256 (question, edit) pairs \
         this walk makes, 5195 move the read not at all and 216 are against the \
         one verb that takes `--json` and answers in prose, which holds no fields \
         to key on. The law can speak only about the rest, and it finds numbers \
         moving in 10 fields. An empty census is what a clean surface looks like \
         AND what a walk that stopped reaching anything looks like; these three \
         numbers are what tell them apart",
    );
    check(
        accompanied.keys().any(|(verb, path)| {
            verb == "report-authoring-frontier" && path == "branch_owned_density.*.density"
        }),
        "NON-VACUITY, THE FIX: the density is still a number and edits still move \
         it — it left the census because the `owned` list beside it now moves \
         too. Without this line an empty census would also be what a read that \
         stopped answering looks like",
    );

    assert_eq!(
        broken,
        Vec::<String>::new(),
        "the census of numbers that name nothing no longer holds"
    );
}

/// The sections, order and facts of a store where two facts of two frames can
/// trade whose belief they are: one frame holds a standing fact and a lapsed
/// one, the other holds a lapsed one, and at the end of the road each frame's
/// view therefore reports exactly one fact as definitively not holding.
fn frame_trade_manifests() -> (serde_json::Value, serde_json::Value, serde_json::Value) {
    let section = |id: &str, title: &str| serde_json::json!({"section_id": id, "parent_doc": "constructed", "title": title});
    let sections = serde_json::json!([
        section("n-01", "the watch begins"),
        section("n-02", "the watch is relieved"),
        section("n-03", "the road goes on"),
    ]);
    let order = serde_json::json!({"edges": [["n-01", "n-02"], ["n-02", "n-03"]]});
    let fact = |id: &str, frame: &str, to: Option<&str>, claim: &str| {
        let mut row = serde_json::json!({
            "fact_id": id,
            "frame": frame,
            "claim": claim,
            "canon_from": "n-01",
            "evidence": ["n-01"],
        });
        if let Some(to) = to {
            row["canon_to"] = serde_json::json!(to);
        }
        row
    };
    let facts = serde_json::json!({
        "frames": [{"frame_id": "f-watcher"}, {"frame_id": "f-rival"}],
        "facts": [
            fact("fx-standing", "f-watcher", None, "the gate is watched"),
            fact("fx-lapsed", "f-watcher", Some("n-02"), "the watcher is at the gate"),
            fact("fx-spent", "f-rival", Some("n-02"), "the rival waits in the lane"),
        ],
    });
    (sections, order, facts)
}

/// A frame's view must be able to say WHICH of its facts stopped holding — and
/// until this round it could not. (Round 1054.)
///
/// `report-frame-view` classifies a frame's facts three ways at a canon point:
/// `holding`, `unknown`, and `not_holding`. The first two shipped as lists and
/// the third as a count, which was recorded once as a limit and left. The
/// residual is not recoverable from the rest of the answer either — it is the
/// frame's population minus the other two, and the population appears nowhere in
/// the report.
///
/// So this is what that cost, measured the way Round 1053 measured the frontier's
/// census: two facts of two frames trade whose belief they are. Each frame still
/// holds one standing fact and one lapsed one, so every count is where it was;
/// with `not_holding` set aside the two reports are byte-equal, so that field is
/// the ONLY place in this read where the trade can ever appear. On the wire this
/// round replaced, the two stores were indistinguishable.
///
/// The census above cannot reach this, and the reason is worth keeping: the panel
/// asks each read at the END of the road (the R1050 rule — the point where the
/// store has all of its history behind it), and there every fact of a frame has
/// already become true, so no single-leg edit moves the residual alone. A walk
/// over the shipped surface is bounded by the questions the corpus can ask it.
#[test]
fn the_frame_view_names_the_facts_it_calls_not_holding() {
    let (sections, order, facts) = frame_trade_manifests();
    let mut traded = facts.clone();
    let mut moved = 0usize;
    for fact in traded["facts"].as_array_mut().expect("facts array") {
        let frame = match fact["fact_id"].as_str() {
            Some("fx-lapsed") => "f-rival",
            Some("fx-spent") => "f-watcher",
            _ => continue,
        };
        fact["frame"] = serde_json::json!(frame);
        moved += 1;
    }
    assert_eq!(moved, 2, "the trade moved {moved} facts, not the two named");

    let build = |manifest: &serde_json::Value| {
        common::constructed_corpus(&sections, &order, manifest)
            .unwrap_or_else(|e| panic!("the constructed manifests must import: {e}"))
    };
    let (before, after) = (build(&facts), build(&traded));
    let ask = |ws: &std::path::Path, argv: &[&str]| -> serde_json::Value {
        let out = common::run(ws, argv);
        assert!(
            out.status.success(),
            "{argv:?} on the constructed store: {}",
            String::from_utf8_lossy(&out.stderr),
        );
        serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("{argv:?} is not json: {e}"))
    };
    let watcher = |ws: &std::path::Path| {
        ask(
            ws,
            &[
                "report-frame-view",
                "--frame",
                "f-watcher",
                "--at",
                "n-03",
                "--json",
            ],
        )
    };
    // The oracle is ANOTHER SHIPPED READ (the R1050 rule): the manuscript hands
    // a runtime each fact's frame verbatim, so it is what says the trade is an
    // edit a reader meets rather than a difference of spelling.
    let frames_of = |ws: &std::path::Path| -> BTreeMap<String, String> {
        let manuscript = ask(ws, &["report-playthrough-manuscript", "--json"]);
        let mut out = BTreeMap::new();
        for world in manuscript["worlds"].as_object().into_iter().flatten() {
            for scene in world.1["scenes"].as_array().into_iter().flatten() {
                for event in scene["begins"].as_array().into_iter().flatten() {
                    if let (Some(fact), Some(frame)) =
                        (event["fact_id"].as_str(), event["frame"].as_str())
                    {
                        out.insert(fact.to_string(), frame.to_string());
                    }
                }
            }
        }
        out
    };

    let (was, now) = (watcher(before.path()), watcher(after.path()));
    let residual = |view: &serde_json::Value| -> Vec<String> {
        view["not_holding"]
            .as_array()
            .expect("the residual is a list")
            .iter()
            .filter_map(|id| id.as_str().map(ToString::to_string))
            .collect()
    };
    let counts = |view: &serde_json::Value| -> (usize, usize, usize) {
        (
            view["holding"].as_array().map_or(0, Vec::len),
            residual(view).len(),
            view["unknown"].as_array().map_or(0, Vec::len),
        )
    };
    let without_residual = |view: &serde_json::Value| {
        let mut rest = view.clone();
        rest.as_object_mut()
            .expect("the frame view is an object")
            .remove("not_holding");
        rest
    };

    println!(
        "the trade moves `fx-lapsed` {:?} -> {:?} and `fx-spent` {:?} -> {:?}; f-watcher at \
         `n-03` reports holding/not_holding/unknown {:?} then {:?}, naming {:?} then {:?}",
        frames_of(before.path()).get("fx-lapsed"),
        frames_of(after.path()).get("fx-lapsed"),
        frames_of(before.path()).get("fx-spent"),
        frames_of(after.path()).get("fx-spent"),
        counts(&was),
        counts(&now),
        residual(&was),
        residual(&now),
    );

    let mut broken: Vec<String> = Vec::new();
    let mut check = |ok: bool, claim: &str| {
        if !ok {
            broken.push(claim.to_string());
        }
    };

    let (seen_before, seen_after) = (frames_of(before.path()), frames_of(after.path()));
    check(
        [
            seen_before.get("fx-lapsed").map(String::as_str),
            seen_after.get("fx-lapsed").map(String::as_str),
            seen_before.get("fx-spent").map(String::as_str),
            seen_after.get("fx-spent").map(String::as_str),
        ] == [
            Some("f-watcher"),
            Some("f-rival"),
            Some("f-rival"),
            Some("f-watcher"),
        ],
        "THE EDIT IS REAL: the manuscript hands the runtime each fact under the \
         other's frame after the trade, so this is a store difference a reader \
         meets and not a difference of spelling",
    );
    check(
        counts(&was) == counts(&now),
        "INVISIBLE TO COUNTS: the frame still holds one fact, still calls one \
         definitively not-holding and still leaves none undecided — the trade is \
         exactly the edit a residual of numbers cannot report",
    );
    check(
        without_residual(&was) == without_residual(&now),
        "AND TO EVERY OTHER FIELD: with the residual set aside the two views are \
         equal, so `not_holding` is the ONLY place in this read where the trade \
         could ever appear",
    );
    check(
        counts(&was) == (1, 1, 0),
        "NON-VACUITY: the frame really does hold one and refuse one at this \
         point. Without this the claims above hold for a view that classifies \
         nothing at all",
    );
    check(
        residual(&was) == ["fx-lapsed"] && residual(&now) == ["fx-spent"],
        "THE LAW: the residual MOVED, and it names the fact it is about — which \
         is what makes `not_holding` an answer a consumer can act on rather than \
         a number that something, somewhere, is no longer true",
    );

    assert_eq!(
        broken,
        Vec::<String>::new(),
        "the frame view does not name the facts it calls not-holding"
    );
}
