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
//! THE LAW IS ASKED OF EVERY PAIR OF ANSWERS, not of each answer against the
//! unedited store (Round 1056). What makes a number lossy is that it is NOT A
//! FUNCTION of what the answer names: two answers that name identical things and
//! disagree about it prove the number carries information nothing named carries.
//! Comparing against the baseline asks that of 92 pairs and leaves the escape
//! Round 1054 named — an edit that moves a name for an unrelated reason excuses
//! the number — so answers are bucketed by WHAT THEY NAME and every pair in a
//! bucket is compared. The first sweep under that law found one field the
//! baseline comparison could not:
//! [`payoff_substantiation_names_every_setup_it_counts`] is what it found, why
//! the pair mattered, and the wire this round changed to close it.
//!
//! ROUND 1054 PROPOSED A DIFFERENT NARROWING — scope the comparison to the
//! RECORD, the read's account of one subject — and this round implemented it,
//! swept it, and REFUTED it. Its first flag was the manuscript's per-scene
//! `holding_count` under 34 edits on two reads, and that count is derivable from
//! the events the same answer names in earlier rows:
//! [`the_manuscript_count_is_derivable_from_the_events_it_names`] proves it over
//! 823 scenes of 41 roads. A read's account of a subject is not bounded by one
//! record when the records form a chain, so the record is the wrong scope for
//! this question — while being exactly the right one for "which reads answer
//! about this id" (`common::wrote_about`, whose addressing this walk does use to
//! say where a number sits).
//!
//! The coverage measure stays and is still printed rather than claimed: for each
//! single pair, `accompanied` is the numbers that moved while something named
//! moved too.
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
    panel, registered_ids, telling_of, wrote_about, Answer, SIDECAR,
};

/// Every number in one answer, keyed by the FIELD it sits at.
///
/// The addressing is [`common::wrote_about`], the one derivation this arc has
/// for where a place in an answer IS — ids collapsed to `*` so a per-road map is
/// one field rather than one finding per road, and array rows collapsed to the
/// fields that key them. The values are kept as a list in the order the records
/// come in rather than summed: rows that trade values have moved, and a total
/// would say they had not, which is the very confusion this walk exists to find.
fn numbers_of(answer: &serde_json::Value, ids: &BTreeSet<String>) -> Numbers {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for per_record in wrote_about(answer, ids).numbers.values() {
        for (field, values) in per_record {
            out.entry(field.clone())
                .or_default()
                .extend(values.iter().cloned());
        }
    }
    out
}

/// What one answer NAMES, keyed so a bucket costs two words instead of a report.
type Fingerprint = (u64, u64);

/// The numbers one answer carried, by the field they sit at.
type Numbers = BTreeMap<String, Vec<String>>;

/// Per read-and-question: what an answer named -> the numbers one such answer
/// carried, and the edit that produced it. Two answers in one bucket name
/// identical things, so a number that differs between them is not a function of
/// the names.
type ByNames = BTreeMap<String, BTreeMap<Fingerprint, (Numbers, String)>>;

/// What an answer NAMES, as a fixed-width key — two independent 64-bit hashes of
/// the number-blanked answer, so answers that name identical things land in one
/// bucket without the walk holding 30 reads x 93 stores of report text in
/// memory (a measurement that runs the machine out of memory is not a
/// measurement).
fn fingerprint_of(named: &serde_json::Value) -> Fingerprint {
    use std::hash::{Hash, Hasher};
    let text = named.to_string();
    let hash = |salt: &str| {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        salt.hash(&mut hasher);
        text.hash(&mut hasher);
        hasher.finish()
    };
    (hash("names"), hash("shape"))
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
///
/// And what the all-pairs law found in Round 1056, also fixed here:
///
/// - `report-payoff-substantiation setups_total` — every fact marked `expected`
///   store-wide, against a body that classifies only the CREDITED ones. The
///   setups nobody paid were counted and never named. No pair with the unedited
///   store shows it; two edited stores that name the same things do.
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
    // label -> what an answer NAMES -> the numbers one such answer carried, and
    // the edit that produced it. Two answers in one bucket name identical things,
    // so a number that differs between them is not a function of the names.
    let mut by_names: ByNames = BTreeMap::new();

    // The unedited store is one of the answers the law compares, not a
    // privileged one: it goes into the same buckets, so "the baseline and this
    // edit name the same things and disagree about a number" is the same finding
    // as any other pair.
    for (label, answer) in &baseline.answers {
        if let Answer::Json(json) = answer {
            by_names.entry(label.clone()).or_default().insert(
                fingerprint_of(&named(json)),
                (numbers_of(json, &ids), "(unedited)".to_string()),
            );
        }
    }

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
            let (was, now) = (numbers_of(before, &ids), numbers_of(after, &ids));
            let moved: BTreeSet<String> = was
                .keys()
                .chain(now.keys())
                .filter(|path| was.get(*path) != now.get(*path))
                .cloned()
                .collect();
            // OVER ALL PAIRS, NOT ONLY AGAINST THE BASELINE (Round 1056). A
            // number is a read's whole account of something exactly when it is
            // NOT A FUNCTION of what the answer names: two answers that name
            // identical things and disagree about the number prove the number
            // carries information nothing named carries. Comparing each answer
            // to the baseline asks that of 92 pairs; keying by what the answer
            // NAMES asks it of every pair the sweep produces, which is the
            // narrowing Round 1054 filed and reaches the case it named — an edit
            // that moves a name for an unrelated reason no longer excuses the
            // number, because another edit lands on the same names.
            let fingerprint = fingerprint_of(&named(after));
            match by_names
                .entry(label.clone())
                .or_default()
                .entry(fingerprint)
            {
                std::collections::btree_map::Entry::Vacant(slot) => {
                    slot.insert((now.clone(), edit.clone()));
                }
                std::collections::btree_map::Entry::Occupied(slot) => {
                    let (theirs, other) = slot.get();
                    for path in theirs.keys().chain(now.keys()) {
                        if theirs.get(path) != now.get(path) {
                            alone
                                .entry((verb.clone(), path.clone()))
                                .or_default()
                                .insert(format!("{edit} against {other}, same names"));
                        }
                    }
                }
            }
            if moved.is_empty() {
                if named(before) == named(after) {
                    still += 1;
                }
                continue;
            }
            // THE COVERAGE MEASURE: the numbers one edit moved while moving
            // something named too. Not evidence of loss — evidence of how much
            // of the surface a single pair cannot speak about, which is why the
            // fingerprint above asks all of them.
            if named(before) != named(after) {
                for path in moved {
                    accompanied
                        .entry((verb.clone(), path))
                        .or_default()
                        .insert(edit.clone());
                }
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
        (still, prose_comparisons, accompanied.len()) == (5194, 216, 11),
        "REACH, ASSERTED RATHER THAN IMPLIED: of the 6256 (question, edit) pairs \
         this walk makes, 5194 move the read not at all and 216 are against the \
         one verb that takes `--json` and answers in prose, which holds no fields \
         to key on. The law can speak only about the rest, and it finds numbers \
         moving in 11 fields. An empty census is what a clean surface looks like \
         AND what a walk that stopped reaching anything looks like; these three \
         numbers are what tell them apart. It was 5195 and 10 until Round 1056: \
         `report-payoff-substantiation` now names the setups nobody paid, so one \
         more edit moves that read at all, and its total joins the fields whose \
         moves are accounted for by something named",
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

/// The manuscript's per-scene count IS derivable from the events the same answer
/// names — which is why Round 1056 does not scope the law above to the RECORD.
///
/// That scoping is what Round 1054 filed as this law's narrowing: a number
/// escapes the whole-answer test when the same edit moves a name somewhere else
/// for an unrelated reason, so ask whether anything named moved inside the
/// number's own record instead. It was implemented, swept, and the first thing
/// it flagged was this count — `worlds.*.scenes[section=*].holding_count`, under
/// every `branch` and `canon_from` retarget, 34 edits on two reads.
///
/// The flag is a FALSE POSITIVE, and the reason is the axis rather than the
/// field: a scene row does not name the facts holding at it, but the road's
/// EARLIER rows do. The count at scene N is the facts that began at or before N
/// and have not ended, and both halves are named events in the same answer. A
/// consumer reading one row learns only a number; a consumer reading the road
/// learns which facts, and a read's account of a subject is not bounded by one
/// record when the records form a chain.
///
/// So this test measures the derivation rather than asserting it, over every
/// authored corpus and every road. It also executes a claim that shipped with
/// the wire in Round 466 and had never been run: "the delta story and the holds
/// semantics cross-check each other — a delta reconstruction that disagrees with
/// the count has hit an unplaced coordinate, never a second semantics".
#[test]
fn the_manuscript_count_is_derivable_from_the_events_it_names() {
    let (stores, unloadable) = common::authored_stores();
    let mut roads = 0usize;
    let mut scenes = 0usize;
    let mut mismatches: Vec<String> = Vec::new();
    let mut touched_by_unplaced = 0usize;
    for store in &stores {
        let out = common::run(
            store.ws.path(),
            &["report-playthrough-manuscript", "--json"],
        );
        if !out.status.success() {
            continue;
        }
        let manuscript: serde_json::Value =
            serde_json::from_slice(&out.stdout).expect("manuscript json");
        for (world, road) in manuscript["worlds"].as_object().into_iter().flatten() {
            roads += 1;
            // The facts this road says it CANNOT place — the escape hatch the
            // wire's own claim names. A fact whose end coordinate is outside the
            // order has no `ends` event to replay.
            let unplaced: BTreeSet<&str> = ["unplaced_facts", "undecidable"]
                .iter()
                .flat_map(|field| road[*field].as_array().into_iter().flatten())
                .filter_map(|row| row["fact_id"].as_str())
                .collect();
            let mut holding: BTreeSet<&str> = BTreeSet::new();
            for scene in road["scenes"].as_array().into_iter().flatten() {
                scenes += 1;
                for event in scene["begins"].as_array().into_iter().flatten() {
                    if let Some(id) = event["fact_id"].as_str() {
                        holding.insert(id);
                    }
                }
                // THE TWO END KINDS STOP AT DIFFERENT TIMES, and the replay has
                // to know it — which is a thing the answer says, in each event's
                // `kind`. A fact still holds AT the scene its `canon_to` names
                // (the interval is closed: `holds_at` asks `p <= canon_to`), and
                // it has already stopped at the scene where a SUCCESSOR begins
                // (that one asks `successor.canon_from <= p`). Replaying both as
                // "gone here" undercounts 26 of 823 scenes, which is how this
                // test found the distinction rather than assuming it.
                for event in scene["ends"].as_array().into_iter().flatten() {
                    let (Some(id), Some(kind)) =
                        (event["fact_id"].as_str(), event["kind"].as_str())
                    else {
                        continue;
                    };
                    if kind == "superseded" {
                        holding.remove(id);
                    }
                }
                let replayed: BTreeSet<&str> = holding
                    .iter()
                    .copied()
                    .filter(|id| !unplaced.contains(id))
                    .collect();
                if holding.len() != replayed.len() {
                    touched_by_unplaced += 1;
                }
                let counted = scene["holding_count"].as_u64().unwrap_or_default() as usize;
                if replayed.len() != counted {
                    mismatches.push(format!(
                        "{} {world} {}: replayed {} of the events it names, counted {counted}",
                        store.name,
                        scene["section"].as_str().unwrap_or("?"),
                        replayed.len(),
                    ));
                }
                // Now the closed end: a fact whose `canon_to` is THIS scene held
                // here and is gone from the next one.
                for event in scene["ends"].as_array().into_iter().flatten() {
                    if let Some(id) = event["fact_id"].as_str() {
                        holding.remove(id);
                    }
                }
            }
        }
    }
    // Print before asserting (the R1026 rule): the reach is the finding as much
    // as the verdict, and a first-violation stop would report one line of it.
    println!(
        "{} authored stores ({} unloadable), {roads} roads, {scenes} scenes replayed; \
         {touched_by_unplaced} scene(s) where an unplaced coordinate is in flight; \
         {} mismatch(es)",
        stores.len(),
        unloadable.len(),
        mismatches.len(),
    );
    for line in &mismatches {
        println!("  MISMATCH {line}");
    }
    assert!(
        scenes > 100,
        "the replay reached {scenes} scenes, which is a walk that stopped \
         working rather than a repository that emptied"
    );
    assert_eq!(
        mismatches,
        Vec::<String>::new(),
        "the manuscript's holding count is NOT a function of the events the same \
         answer names, which would make the count lossy after all and the \
         record-scoped flag a true positive"
    );
}

/// `report-payoff-substantiation` must NAME every setup it counts, and until
/// Round 1056 it did not. (Round 1056.)
///
/// The census above found this one by asking its question of every PAIR of
/// answers rather than of each answer against the unedited store: retargeting
/// `f-041`'s world-line and dropping `f-041`'s payoff expectation produce two
/// answers that NAME identical things and disagree about `setups_total`. A number
/// that two answers naming the same things disagree about is carrying information
/// nothing named carries — which is this law's whole subject, and the pair that
/// proves it is a pair neither answer forms with the baseline.
///
/// The cause is a class this read is silent about. `setups_total` is every fact
/// marked `expected`, store-wide; `worlds` classifies only the setups some payoff
/// CREDITED, three ways. A setup no payoff credits — the author's todo, which the
/// coverage sibling names as `dangling` — appears in this report only as a
/// contribution to the total. So a consumer reads "12 setups" against a body
/// naming nine and cannot learn which three are missing, or that they are the
/// ones substantiation cannot even be asked about.
///
/// This test states the property rather than the fix: every setup the total
/// counts is named somewhere in the same answer.
///
/// IT ASKS A CONSTRUCTED STORE AS WELL, and that is not decoration. All 28
/// authored corpora satisfy the property as shipped — an author who marks a setup
/// goes on to pay it — so a sweep of what authors wrote reports a clean surface
/// and the census's finding reads as noise. The shape that breaks it is a setup
/// NO payoff credits, which the census reached by dropping one expectation from
/// an authored store. What the corpora cannot make, the tree makes, through the
/// same import recipe an author would use (the R1052 rule).
#[test]
fn payoff_substantiation_names_every_setup_it_counts() {
    let section = |id: &str, title: &str| serde_json::json!({"section_id": id, "parent_doc": "constructed", "title": title});
    let sections = serde_json::json!([
        section("n-01", "the gun on the wall"),
        section("n-02", "the gun goes off"),
    ]);
    let order = serde_json::json!({"edges": [["n-01", "n-02"]]});
    let fact = |id: &str, at: &str, claim: &str| {
        serde_json::json!({
            "fact_id": id,
            "frame": "f-teller",
            "claim": claim,
            "canon_from": at,
            "evidence": [at],
        })
    };
    let mut paid_setup = fact("fx-paid-setup", "n-01", "a rifle hangs over the hearth");
    paid_setup["payoff_expectation"] = serde_json::json!("expected");
    let mut payoff = fact("fx-payoff", "n-02", "the rifle is fired");
    payoff["pays_off"] = serde_json::json!(["fx-paid-setup"]);
    // THE SHAPE NO AUTHOR SHIPPED: marked as a setup, credited by nothing.
    let mut dangling_setup = fact("fx-dangling-setup", "n-01", "a locked case sits beneath it");
    dangling_setup["payoff_expectation"] = serde_json::json!("expected");
    let facts = serde_json::json!({
        "frames": [{"frame_id": "f-teller"}],
        "facts": [paid_setup, payoff, dangling_setup],
    });
    let constructed = common::constructed_corpus(&sections, &order, &facts)
        .unwrap_or_else(|e| panic!("the constructed manifests must import: {e}"));

    let (stores, unloadable) = common::authored_stores();
    let mut targets: Vec<(String, &std::path::Path)> = stores
        .iter()
        .map(|store| (store.name.clone(), store.ws.path()))
        .collect();
    targets.push((
        "a constructed store with one uncredited setup".to_string(),
        constructed.path(),
    ));
    let mut asked = 0usize;
    let mut shortfalls: Vec<String> = Vec::new();
    for (name, path) in &targets {
        let out = common::run(path, &["report-payoff-substantiation", "--json"]);
        if !out.status.success() {
            continue;
        }
        asked += 1;
        let report: serde_json::Value =
            serde_json::from_slice(&out.stdout).expect("substantiation json");
        let mut named: BTreeSet<&str> = BTreeSet::new();
        for road in report["worlds"]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(_, w)| w)
        {
            for class in ["substantiated", "unsubstantiated", "unverifiable"] {
                for row in road[class].as_array().into_iter().flatten() {
                    if let Some(setup) = row["setup"].as_str() {
                        named.insert(setup);
                    }
                }
            }
            for class in ["dangling", "unknown"] {
                for row in road[class].as_array().into_iter().flatten() {
                    if let Some(setup) = row.as_str() {
                        named.insert(setup);
                    }
                }
            }
        }
        let counted = report["setups_total"].as_u64().unwrap_or_default() as usize;
        if named.len() != counted {
            shortfalls.push(format!(
                "{name}: counts {counted} setups, names {}",
                named.len(),
            ));
        }
    }
    // Print before asserting (the R1026 rule).
    println!(
        "{asked} stores answered ({} unloadable); {} name fewer setups than they count",
        unloadable.len(),
        shortfalls.len(),
    );
    for line in &shortfalls {
        println!("  SHORTFALL {line}");
    }
    assert!(asked > 20, "the sweep asked {asked} stores");
    assert_eq!(
        shortfalls,
        Vec::<String>::new(),
        "a setup counted and never named is a number a consumer cannot account for"
    );
}
