//! Which subjects does more than one shipped read answer about? (R1039/R1040)
//!
//! Round 1037 found a defect by comparing two shipped reads that answer the
//! same question — `validate-continuity` judged the authored corpus's main
//! quest discharged on three roads while `report-quest-graph` reported
//! `unknown` on all four, a contradiction that had shipped since R568. That
//! comparison was made BY HAND, for one pair, and it hit on the first try. A
//! hand-picked second pair is the mistake this arc has made four times over, so
//! the population is derived here instead.
//!
//! A SUBJECT is an id the store registers. A read's RECORD about it is what
//! that read WROTE ABOUT IT, and where a read writes an id decides what that
//! is:
//!
//! - as a field's VALUE, the minimal enclosing object — the row is the
//!   sentence, and `{"quest": "q-main", "state": "unknown"}` says `unknown`
//!   about `q-main`;
//! - as a KEY of a map, the value under that key — a report holding a subtree
//!   per road said the one under `r-1` about `r-1`, not the map that holds
//!   every other road beside it;
//! - as a bare element of a LIST, its MEMBERSHIP of that list, addressed down
//!   through the ancestors that key it: `scene_coverage[scene=sec-3].facts[]`.
//!
//! The third is Round 1055 and the first two are the same rule finally stated.
//! Until here EVERY occurrence claimed the minimal enclosing object, so a fact
//! listed in a scene's census carried the whole census row as its record and
//! moved whenever any of its NEIGHBOURS did. Round 1054 made three reads name
//! the sets they had been counting, each list holding most of the store, and
//! the number of subjects more than one read answers about went 79 -> 210 out
//! of 214 in one round: a backlog ranked by co-answering had become a backlog
//! ranked by co-ENUMERATING, and the backlog is what says which pair to build
//! the next contract over. The obvious repair — a list member's record is its
//! own membership — is wrong as stated, and the census is the refutation: it
//! lists facts PER SCENE, so a record that forgets the scene cannot see a fact
//! move to another one. Hence the address, and the key of an array row is
//! DERIVED (the fields holding a subject id in every row, distinct across
//! them) rather than named per read.
//!
//! What a membership record cannot see is ORDER, and this walk asserts that
//! rather than assuming it: the sweep watches every listed set for a
//! permutation that leaves its membership alone, and finds none. The day one
//! appears, position belongs in the address and the walk says so by going red.
//!
//! TWO DERIVATIONS OF "ANSWERS ABOUT" WERE MEASURED AND DISCARDED FIRST, and
//! both failures are why this one is shaped the way it is:
//!
//! - MENTION. 220 of the corpus's 220 subjects come out shared, because three
//!   reads render the whole store. Co-mention is not co-answering.
//! - A VERDICT FIELD, recognised first by shape ("a few short lowercase
//!   tokens") and then by CLOSURE ("its value set does not grow as more
//!   authored stores are asked"). The shape guess swept in `telling` and
//!   `parent`, ids that happen to look like tokens. The closure measurement
//!   over all 28 loadable corpora did no better, and why is worth keeping: a
//!   field only ONE corpus exercises has a small union whatever it is —
//!   `quest_id` pools to 4 values across 28 stores because one store declares
//!   quests. Whether a field carries a closed vocabulary is a fact about its
//!   TYPE, and no amount of looking at output recovers it.
//!
//! So this walk asks what the perturbation CAN answer, which is what R1037's
//! finding actually rested on: a read ANSWERS ABOUT a subject when its record
//! for that subject CHANGES under some authorable edit. A read that only
//! renders the id answers about nothing it does not itself carry, and the
//! corruption population is derived from the store's own legs rather than aimed
//! at any read.
//!
//! What this walk does NOT claim: that two reads answering about one subject
//! must agree. It says where to look. The pair R1037 examined by hand is in the
//! backlog, and that is the oracle — a derivation that cannot re-find a known
//! positive is a list, not a backlog.

use std::collections::{BTreeMap, BTreeSet};

use mnemosyne_atomic::AtomicStore;

mod common;
use common::{
    ask_panel, corruptions, dnd_quest_facts, dnd_quest_workspace_from, dnd_quest_workspace_try,
    panel, registered_ids, telling_of, Answer, SIDECAR,
};

/// What one answer wrote about the store's subjects.
#[derive(Default)]
struct Records {
    /// subject -> every record this answer holds about it.
    by_subject: BTreeMap<String, BTreeSet<String>>,
    /// The bare subject ids listed at an address, in document order. Kept so
    /// the sweep can ask the one question a MEMBERSHIP record cannot answer:
    /// whether an authorable edit ever permutes a list without changing what
    /// is in it.
    listed: BTreeMap<String, Vec<String>>,
    /// Rows of an array of objects that NO field addresses, so the walk fell
    /// back to the position each happens to sit at. Named, not counted (the
    /// R1029 rule) — a positional address is the one part of this derivation
    /// that a reordering read would break, so the next round needs to see
    /// which rows are on it.
    unaddressed: BTreeSet<String>,
    /// Two places in one answer that produced the same address. Structurally
    /// impossible while row keys are distinct, and measured rather than
    /// argued: an address that is not unique silently merges two lists.
    collisions: usize,
}

impl Records {
    fn wrote(&mut self, subject: &str, record: String) {
        self.by_subject
            .entry(subject.to_string())
            .or_default()
            .insert(record);
    }
}

fn under(path: &str, key: &str) -> String {
    if path.is_empty() {
        key.to_string()
    } else {
        format!("{path}.{key}")
    }
}

/// The fields that ADDRESS a row of this array: the SMALLEST set of fields
/// holding a subject id in every row whose values, taken together, are
/// distinct across the rows. Derived from the values rather than named per
/// read — the same discrimination `common::road_keying` makes for roads,
/// widened to every id the store registers and to composite keys.
///
/// Distinctness is what makes it a key: a field every row carries but two rows
/// share does not say which row you are at. And a single field is not enough
/// on the shipped surface — the disclosure coverage's `inert_reveal_pins` is
/// one row per (fact, world) and NEITHER column is distinct alone, which is
/// how 9 rows came to be addressed by the position they sat at.
///
/// Smallest because an address should carry no more than what identifies the
/// row: a field that varies for its own reasons would make the address move
/// for a reason that is not about which row this is.
fn key_fields(items: &[serde_json::Value], subjects: &BTreeSet<String>) -> Vec<String> {
    let Some(first) = items.first().and_then(serde_json::Value::as_object) else {
        return Vec::new();
    };
    let candidates: Vec<String> = first
        .keys()
        .filter(|key| {
            items.iter().all(|item| {
                item.get(key)
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|id| subjects.contains(id))
            })
        })
        .cloned()
        .collect();
    let keys = |fields: &[String]| {
        let mut seen: BTreeSet<Vec<&str>> = BTreeSet::new();
        items.iter().all(|item| {
            let tuple: Vec<&str> = fields
                .iter()
                .filter_map(|field| item.get(field).and_then(serde_json::Value::as_str))
                .collect();
            seen.insert(tuple)
        })
    };
    for size in 1..=candidates.len() {
        if let Some(found) = combinations(&candidates, size)
            .into_iter()
            .find(|fields| keys(fields))
        {
            return found;
        }
    }
    Vec::new()
}

/// Every `size`-element subset of `of`, in the order the fields come in —
/// so the address a row gets is a fact about the answer and not about which
/// subset happened to be tried first.
fn combinations(of: &[String], size: usize) -> Vec<Vec<String>> {
    if size == 0 {
        return vec![Vec::new()];
    }
    let mut out = Vec::new();
    for (index, field) in of.iter().enumerate() {
        for rest in combinations(&of[index + 1..], size - 1) {
            let mut one = vec![field.clone()];
            one.extend(rest);
            out.push(one);
        }
    }
    out
}

fn row_address(path: &str, item: &serde_json::Value, index: usize, keys: &[String]) -> String {
    let addressed: Vec<String> = keys
        .iter()
        .filter_map(|field| {
            item.get(field)
                .and_then(serde_json::Value::as_str)
                .map(|id| format!("{field}={id}"))
        })
        .collect();
    if addressed.is_empty() {
        format!("{path}[{index}]")
    } else {
        format!("{path}[{}]", addressed.join(","))
    }
}

/// Walk one answer and record what it wrote about each subject it names.
///
/// An ancestor never claims: an object holds every id under it, so letting it
/// claim would make the whole report one record about everything. The three
/// occurrence kinds in this file's header are the three arms below, and each
/// stops the propagation where the read's own statement about that id ends.
fn collect(value: &serde_json::Value, path: &str, subjects: &BTreeSet<String>, out: &mut Records) {
    match value {
        serde_json::Value::Object(map) => {
            let mut named_here: Vec<&String> = Vec::new();
            for (key, child) in map {
                let child_path = under(path, key);
                // A map KEY is an id too — `per_world` and `worlds` are keyed
                // by world-line. What the read wrote about that road is the
                // subtree under it, not its siblings' subtrees as well.
                if subjects.contains(key) {
                    out.wrote(key, format!("{child_path} = {child}"));
                }
                match child {
                    serde_json::Value::String(id) if subjects.contains(id) => named_here.push(id),
                    _ => collect(child, &child_path, subjects, out),
                }
            }
            if !named_here.is_empty() {
                // The row is the sentence: a field naming an id makes THIS
                // object what the read said about it, lists and all.
                let record = format!("{path} = {value}");
                for id in named_here {
                    out.wrote(id, record.clone());
                }
            }
        }
        serde_json::Value::Array(items) => {
            let keys = key_fields(items, subjects);
            let listed_at = format!("{path}[]");
            let mut listed: Vec<String> = Vec::new();
            for (index, item) in items.iter().enumerate() {
                match item {
                    serde_json::Value::String(id) if subjects.contains(id) => {
                        out.wrote(id, listed_at.clone());
                        listed.push(id.clone());
                    }
                    _ => {
                        let row = row_address(path, item, index, &keys);
                        if item.is_object() && keys.is_empty() {
                            out.unaddressed.insert(row.clone());
                        }
                        collect(item, &row, subjects, out);
                    }
                }
            }
            if !listed.is_empty() && out.listed.insert(listed_at, listed).is_some() {
                out.collisions += 1;
            }
        }
        _ => {}
    }
}

/// The lists that came back holding EXACTLY what they held, in another order —
/// the one thing a membership record cannot see, so the walk asks for it on
/// every edit instead of assuming it never happens.
///
/// A list whose membership changed is not one of these: that move is what the
/// record is for, and it is already counted as an answer.
fn permutations(before: &Records, after: &Records) -> BTreeSet<String> {
    before
        .listed
        .iter()
        .filter(|(address, order)| {
            after.listed.get(*address).is_some_and(|now| {
                now != *order
                    && now.iter().collect::<BTreeSet<_>>() == order.iter().collect::<BTreeSet<_>>()
            })
        })
        .map(|(address, _)| address.clone())
        .collect()
}

/// Every read's records for one store.
fn panel_records(
    answers: &BTreeMap<String, Answer>,
    subjects: &BTreeSet<String>,
) -> BTreeMap<String, Records> {
    let mut out = BTreeMap::new();
    for (verb, answer) in answers {
        if let Answer::Json(json) = answer {
            let mut records = Records::default();
            collect(json, "", subjects, &mut records);
            out.insert(verb.clone(), records);
        }
    }
    out
}

/// The pairs of shipped reads that both ANSWER ABOUT a subject — both of their
/// records for it move under some authorable edit. Most-shared first; this is
/// the backlog, and R1037's hand-picked pair sits inside it.
const BACKLOG: [&str; 55] = [
    "102 report-playable-world <-> report-playthrough-manuscript",
    "86 report-authoring-frontier <-> report-playable-world",
    "86 report-authoring-frontier <-> report-playthrough-manuscript",
    "56 report-entity <-> report-playable-world",
    "56 report-entity <-> report-playthrough-manuscript",
    "44 report-authoring-frontier <-> report-entity",
    "43 report-edge-candidates <-> report-entity",
    "43 report-edge-candidates <-> report-playable-world",
    "43 report-edge-candidates <-> report-playthrough-manuscript",
    "39 report-authoring-frontier <-> report-edge-candidates",
    "39 report-frame-view <-> report-playable-world",
    "39 report-frame-view <-> report-playthrough-manuscript",
    "38 report-entity <-> report-frame-view",
    "37 report-playable-world <-> report-quest-graph",
    "37 report-playthrough-manuscript <-> report-quest-graph",
    "36 report-entity <-> report-quest-graph",
    "33 report-edge-candidates <-> report-quest-graph",
    "30 report-authoring-frontier <-> report-quest-graph",
    "29 report-edge-candidates <-> report-frame-view",
    "27 report-authoring-frontier <-> report-frame-view",
    "24 report-frame-view <-> report-quest-graph",
    "21 report-authoring-frontier <-> report-payoff-coverage",
    "21 report-edge-candidates <-> report-payoff-coverage",
    "21 report-entity <-> report-payoff-coverage",
    "21 report-payoff-coverage <-> report-playable-world",
    "21 report-payoff-coverage <-> report-playthrough-manuscript",
    "15 report-frame-view <-> report-payoff-coverage",
    "14 report-edge-candidates <-> report-typing-candidates",
    "14 report-entity <-> report-typing-candidates",
    "14 report-payoff-coverage <-> report-quest-graph",
    "14 report-playable-world <-> report-typing-candidates",
    "14 report-playthrough-manuscript <-> report-typing-candidates",
    "13 report-authoring-frontier <-> report-payoff-substantiation",
    "13 report-authoring-frontier <-> report-typing-candidates",
    "13 report-edge-candidates <-> report-payoff-substantiation",
    "13 report-entity <-> report-payoff-substantiation",
    "13 report-frame-view <-> report-typing-candidates",
    "13 report-payoff-coverage <-> report-payoff-substantiation",
    "13 report-payoff-substantiation <-> report-playable-world",
    "13 report-payoff-substantiation <-> report-playthrough-manuscript",
    "11 report-authoring-frontier <-> validate-continuity",
    "11 report-edge-candidates <-> validate-continuity",
    "11 report-entity <-> validate-continuity",
    "11 report-payoff-substantiation <-> report-quest-graph",
    "11 report-playable-world <-> validate-continuity",
    "11 report-playthrough-manuscript <-> validate-continuity",
    "11 report-quest-graph <-> validate-continuity",
    "10 report-quest-graph <-> report-typing-candidates",
    "9 report-frame-view <-> report-payoff-substantiation",
    "7 report-payoff-coverage <-> report-typing-candidates",
    "7 report-payoff-substantiation <-> report-typing-candidates",
    "6 report-frame-view <-> validate-continuity",
    "4 report-payoff-coverage <-> validate-continuity",
    "4 report-payoff-substantiation <-> validate-continuity",
    "3 report-typing-candidates <-> validate-continuity",
];

/// THE DEFINITION'S OWN TEST — what a record IS, asked of answers this test
/// writes rather than of the shipped surface.
///
/// The walk below measures the surface WITH this rule and reports numbers, and
/// a number cannot say whether the rule is the one written down: both of the
/// rules Round 1055 rejected produce a perfectly good number. So each claim in
/// this file's header is an assertion here, and the two that decided the shape
/// are the two that killed the alternatives — a NEIGHBOUR moving must not move
/// this subject's record (which is what the rule that shipped until R1055
/// does), and a fact changing SCENE must move it (which is what the obvious
/// repair, membership with no address, cannot see).
#[test]
fn a_record_is_what_the_read_wrote_about_that_id() {
    let subjects: BTreeSet<String> = ["f-1", "f-2", "f-3", "sec-1", "sec-2", "r-1", "r-2", "q-1"]
        .into_iter()
        .map(ToString::to_string)
        .collect();
    let records = |answer: &serde_json::Value| {
        let mut out = Records::default();
        collect(answer, "", &subjects, &mut out);
        assert_eq!(out.collisions, 0, "an address is produced once");
        out
    };
    let one = |record: String| BTreeSet::from([record]);

    // A CENSUS: facts listed per scene, which is the shape that decided the
    // rule. `scene` keys the rows because every row holds one and no two rows
    // hold the same.
    let census = |first: &[&str], second: &[&str]| {
        serde_json::json!({"scene_coverage": [
            {"scene": "sec-1", "facts": first},
            {"scene": "sec-2", "facts": second},
        ]})
    };
    let before = records(&census(&["f-1", "f-2"], &["f-3"]));
    assert_eq!(
        before.by_subject["f-1"],
        one("scene_coverage[scene=sec-1].facts[]".to_string()),
        "a bare id in a list is addressed down through the ancestors that key \
         it, and that address is the whole of the record — the ancestors above \
         it hold every other id too and claim none of them"
    );
    let row = serde_json::json!({"scene": "sec-1", "facts": ["f-1", "f-2"]});
    assert_eq!(
        before.by_subject["sec-1"],
        one(format!("scene_coverage[scene=sec-1] = {row}")),
        "the id that KEYS the row has the row itself as its record, census and \
         all: the read wrote that list about that scene"
    );

    // A NEIGHBOUR MOVES. This is what the minimal-enclosing-object rule got
    // wrong, and it got it wrong in the direction that inflates: f-1's record
    // was the whole row, so f-2 leaving moved it.
    let neighbour_left = records(&census(&["f-1"], &["f-2", "f-3"]));
    assert_eq!(
        neighbour_left.by_subject["f-1"], before.by_subject["f-1"],
        "a co-listed neighbour moving is not this subject's record moving"
    );
    assert_ne!(
        neighbour_left.by_subject["sec-1"], before.by_subject["sec-1"],
        "and the same edit DOES move the scene's record, because what the read \
         wrote about the scene is exactly the list that changed"
    );

    // THE SUBJECT ITSELF MOVES. This is what membership-without-an-address
    // gets wrong: both rows list under `facts`, so a record of "which list"
    // alone is identical here and the census could not see a fact change the
    // scene it is anchored at — the very thing R1053 changed the wire for.
    let moved = records(&census(&["f-2"], &["f-1", "f-3"]));
    assert_eq!(
        moved.by_subject["f-1"],
        one("scene_coverage[scene=sec-2].facts[]".to_string()),
        "a fact anchored at another scene is listed at another address"
    );
    assert_ne!(moved.by_subject["f-1"], before.by_subject["f-1"]);

    // A MAP KEYED BY ID. What the read wrote about `r-1` is the subtree under
    // it — not the map, which holds every other road beside it.
    let worlds = |mine: &[&str], theirs: &[&str]| {
        serde_json::json!({"worlds": {
            "r-1": {"owned": mine},
            "r-2": {"owned": theirs},
        }})
    };
    let roads = records(&worlds(&["f-1"], &["f-3"]));
    assert_eq!(
        roads.by_subject["r-1"],
        one(format!(
            "worlds.r-1 = {}",
            serde_json::json!({"owned": ["f-1"]})
        )),
    );
    assert_eq!(
        roads.by_subject["f-1"],
        one("worlds.r-1.owned[]".to_string()),
    );
    let other_road = records(&worlds(&["f-1"], &["f-2", "f-3"]));
    assert_eq!(
        other_road.by_subject["r-1"], roads.by_subject["r-1"],
        "a change under a SIBLING key is not a change under this one — the map \
         that holds both is nobody's record"
    );
    assert_ne!(other_road.by_subject["r-2"], roads.by_subject["r-2"]);

    // A ROW THAT NAMES AN ID IN A FIELD is the sentence about it, so anything
    // else in that row is part of what the read said.
    let quest = |state: &str| serde_json::json!({"quests": [{"quest": "q-1", "state": state}]});
    let unknown = records(&quest("unknown"));
    assert_eq!(
        unknown.by_subject["q-1"],
        one(format!(
            "quests[quest=q-1] = {}",
            serde_json::json!({"quest": "q-1", "state": "unknown"})
        )),
    );
    assert_ne!(
        records(&quest("discharged")).by_subject["q-1"],
        unknown.by_subject["q-1"],
        "the verdict beside the id is what makes this read ANSWER about it"
    );

    // ONE SUBJECT, SEVERAL RECORDS. `world` keys these rows and `at` does not,
    // because two rows hold the same `at` — a field every row carries but two
    // rows share does not say which row you are at.
    let locators = serde_json::json!({"locators": [
        {"world": "r-1", "at": "sec-1"},
        {"world": "r-2", "at": "sec-1"},
    ]});
    let locators = records(&locators);
    assert_eq!(
        locators.by_subject["sec-1"],
        BTreeSet::from([
            format!(
                "locators[world=r-1] = {}",
                serde_json::json!({"world": "r-1", "at": "sec-1"})
            ),
            format!(
                "locators[world=r-2] = {}",
                serde_json::json!({"world": "r-2", "at": "sec-1"})
            ),
        ]),
    );
    assert!(locators.unaddressed.is_empty());

    // A ROW NO SINGLE FIELD KEYS. One row per (fact, world): `f-1` appears in
    // two of them and `r-1` in two of them, so neither column addresses a row
    // and the pair does. This is the shipped `inert_reveal_pins` shape.
    let pins = records(&serde_json::json!({"pins": [
        {"fact": "f-1", "world": "r-1"},
        {"fact": "f-1", "world": "r-2"},
        {"fact": "f-2", "world": "r-1"},
    ]}));
    assert!(pins.unaddressed.is_empty());
    assert_eq!(
        pins.by_subject["f-1"],
        BTreeSet::from([
            format!(
                "pins[fact=f-1,world=r-1] = {}",
                serde_json::json!({"fact": "f-1", "world": "r-1"})
            ),
            format!(
                "pins[fact=f-1,world=r-2] = {}",
                serde_json::json!({"fact": "f-1", "world": "r-2"})
            ),
        ]),
    );

    // A ROW NO FIELD KEYS falls back to the position it sits at, and says so
    // by name. The walk below asserts this set is EMPTY on the shipped
    // surface; here is the shape that would put something in it.
    let classes = records(&serde_json::json!({"classes": [
        {"kind": "exempt", "facts": ["f-1"]},
        {"kind": "dangling", "facts": ["f-2"]},
    ]}));
    assert_eq!(
        classes.unaddressed,
        BTreeSet::from(["classes[0]".to_string(), "classes[1]".to_string()]),
    );
    assert_eq!(
        classes.by_subject["f-1"],
        one("classes[0].facts[]".to_string()),
    );

    // THE LISTED SETS, kept in document order, are what the ORDER assertion in
    // the walk below reads.
    assert_eq!(
        before.listed,
        BTreeMap::from([
            (
                "scene_coverage[scene=sec-1].facts[]".to_string(),
                vec!["f-1".to_string(), "f-2".to_string()]
            ),
            (
                "scene_coverage[scene=sec-2].facts[]".to_string(),
                vec!["f-3".to_string()]
            ),
        ]),
    );

    // AND THE DETECTOR THAT READS THEM. The walk below asserts it finds
    // nothing on the shipped surface, and an assertion that holds because the
    // detector cannot fire is worth nothing — so here is the shape that fires
    // it, and the shape that must NOT (a list whose membership changed has
    // moved for a reason the record already carries).
    let reordered = records(&census(&["f-2", "f-1"], &["f-3"]));
    assert_eq!(
        permutations(&before, &reordered),
        BTreeSet::from(["scene_coverage[scene=sec-1].facts[]".to_string()]),
        "the same ids in another order is exactly what a membership record \
         cannot see, and the detector says so"
    );
    assert!(
        permutations(&before, &neighbour_left).is_empty(),
        "a list that lost a member did not permute"
    );
}

#[test]
fn the_population_of_subjects_more_than_one_shipped_read_answers_about() {
    let facts_json = dnd_quest_facts();
    let ws = dnd_quest_workspace_from(&facts_json);
    let store = AtomicStore::load(&ws.path().join(SIDECAR)).expect("the imported store loads");
    let subjects = registered_ids(&store);
    let telling = telling_of(&store);
    let (panel, unaskable) = panel(ws.path(), &telling);
    let baseline = ask_panel(ws.path(), &panel);
    assert!(
        baseline.failed.is_empty(),
        "the panel is exactly the reads that answered at baseline: {:?}",
        baseline.failed
    );
    let baseline_records = panel_records(&baseline.answers, &subjects);
    // The panel is keyed by read-and-question since Round 1051 — one verb can
    // be asked several. Every verdict below is about the READ, so the label
    // maps back to it and the answers a verb gave to its several questions are
    // unioned: a read ANSWERS ABOUT a subject when SOME question it can be
    // asked moves its record.
    let verb_of: BTreeMap<String, String> = panel
        .iter()
        .map(|read| (read.label(), read.verb.clone()))
        .collect();
    let prose_only: BTreeSet<&str> = baseline
        .answers
        .iter()
        .filter(|(_, a)| matches!(a, Answer::Prose(_)))
        .map(|(label, _)| verb_of[label].as_str())
        .collect();
    let prose_only: Vec<&str> = prose_only.into_iter().collect();
    // read -> every subject any of its questions MENTIONED at baseline.
    let mut mentioned: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (label, per_read) in &baseline_records {
        mentioned
            .entry(verb_of[label].clone())
            .or_default()
            .extend(per_read.by_subject.keys().cloned());
    }
    // Every address this derivation could not key by a field, and every
    // address it produced twice — the two ways the addressing could be
    // unsound, measured over the whole panel rather than reasoned about.
    let unaddressed: BTreeSet<String> = baseline_records
        .iter()
        .flat_map(|(label, per_read)| {
            per_read
                .unaddressed
                .iter()
                .map(move |row| format!("{label}: {row}"))
        })
        .collect();
    let mut collisions: usize = baseline_records
        .values()
        .map(|per_read| per_read.collisions)
        .sum();

    // read -> the subjects whose record it MOVED for, over the whole derived
    // corruption population. Moving is what separates answering from rendering.
    let mut responsive: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    // Lists that came back holding the same ids in a different order.
    let mut permuted: BTreeSet<String> = BTreeSet::new();
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
            // The write path refuses it: not a move an author could make, so it
            // is evidence about nothing here.
            refused += 1;
            continue;
        };
        applied += 1;
        let seen = ask_panel(mutated_ws.path(), &panel);
        let seen_records = panel_records(&seen.answers, &subjects);
        for (label, before) in &baseline_records {
            let after = seen_records.get(label);
            for subject in &subjects {
                let was = before.by_subject.get(subject);
                let now = after.and_then(|per_read| per_read.by_subject.get(subject));
                if was != now {
                    responsive
                        .entry(verb_of[label].clone())
                        .or_default()
                        .insert(subject.clone());
                }
            }
            // THE ONE THING A MEMBERSHIP RECORD CANNOT SEE — asked on every
            // corruption, over every list, rather than assumed.
            let Some(after) = after else { continue };
            permuted.extend(
                permutations(before, after)
                    .into_iter()
                    .map(|address| format!("{label}: {address}")),
            );
            collisions += after.collisions;
        }
    }

    // subject -> the reads that answer about it.
    let mut readers_of: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for (verb, moved) in &responsive {
        for subject in moved {
            readers_of
                .entry(subject.as_str())
                .or_default()
                .insert(verb.as_str());
        }
    }
    let shared: BTreeMap<&str, &BTreeSet<&str>> = readers_of
        .iter()
        .filter(|(_, reads)| reads.len() > 1)
        .map(|(subject, reads)| (*subject, reads))
        .collect();

    // THE BACKLOG: which PAIR of reads answers about the most subjects in
    // common. R1037 compared one pair by hand; this says which pair to compare
    // next, and how much of the store rides on each answer agreeing.
    let mut overlap: BTreeMap<(&str, &str), usize> = BTreeMap::new();
    for reads in shared.values() {
        let listed: Vec<&str> = reads.iter().copied().collect();
        for (i, a) in listed.iter().enumerate() {
            for b in &listed[i + 1..] {
                *overlap.entry((a, b)).or_default() += 1;
            }
        }
    }
    let mut ranked: Vec<((&str, &str), usize)> = overlap.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

    // Print BEFORE asserting — the distribution is the point, and a
    // first-violation stop would report one line of it (the R1026 lesson).
    println!(
        "{} registered subjects; {} corruptions applied, {} refused by the write \
         path; {} reads asked over {} questions, {} answering `--json` in prose \
         ({}), {} UNASKABLE:\n",
        subjects.len(),
        applied,
        refused,
        mentioned.len(),
        baseline_records.len(),
        prose_only.len(),
        prose_only.join(" "),
        unaskable.len(),
    );
    // NAMED, not counted. Round 1051: this line printed a bare count, and the
    // four reads behind it were outside every population this walk derives —
    // including the backlog that says which pair to compare next. An exclusion
    // nobody can read is an exclusion nobody removes (the R1029 rule).
    for (verb, reason) in &unaskable {
        println!("  UNASKABLE {verb}: {reason}");
    }
    // The addressing's own two failure modes, printed every run: a row no
    // field keys is addressed by where it sits, and an address produced twice
    // would merge two lists into one record.
    println!(
        "  ADDRESSING: {} rows addressed by position, {collisions} address collisions",
        unaddressed.len(),
    );
    for row in &unaddressed {
        println!("    BY POSITION {row}");
    }
    for list in &permuted {
        println!("    PERMUTED {list}");
    }
    println!("per read: subjects MENTIONED at baseline / ANSWERED ABOUT (record moved):");
    for (verb, subjects_seen) in &mentioned {
        println!(
            "  {:38} {:4} / {:4}",
            verb,
            subjects_seen.len(),
            responsive.get(verb).map_or(0, BTreeSet::len),
        );
    }
    println!(
        "\n{} of {} subjects are answered about by MORE THAN ONE read",
        shared.len(),
        readers_of.len(),
    );
    // NAMED, not counted (the R1029 rule): until here the count said some
    // subjects were unreached without saying which. Read it as a fact about
    // this POPULATION and not about the surface — the corruptions are the quest
    // layer's legs, so an id no quest fact touches cannot be moved by any of
    // them, and these five are entities of that kind.
    let unanswered: Vec<&str> = subjects
        .iter()
        .map(String::as_str)
        .filter(|subject| !readers_of.contains_key(subject))
        .collect();
    println!(
        "{} subjects no read answers about UNDER THIS POPULATION: {}",
        unanswered.len(),
        unanswered.join(" "),
    );
    println!("\nread pairs by shared subjects — the backlog, most-shared first:");
    for ((a, b), n) in &ranked {
        println!("  {n:4}  {a} <-> {b}");
    }

    // EVERY CHECK RUNS AND THE FAILURES COME OUT AS A LIST. Stopping at the
    // first makes an injection report one line about a walk with six
    // independent claims and hides which are alive — seven injections into the
    // R1039 form landed on two assertions between them. The R1026 rule, applied
    // to this test's OWN claims rather than to its findings.
    let mut broken: Vec<String> = Vec::new();
    let mut check = |ok: bool, claim: &str| {
        if !ok {
            broken.push(claim.to_string());
        }
    };

    check(
        ranked
            .iter()
            .any(|((a, b), _)| (*a, *b) == ("report-quest-graph", "validate-continuity")),
        "ORACLE: the derivation reproduces the one pair a hand comparison already \
         proved was worth making",
    );
    check(
        shared.get("q-main").is_some_and(|reads| {
            reads.contains("report-quest-graph") && reads.contains("validate-continuity")
        }),
        "ORACLE: it names `q-main`, the subject R1037's defect was about, rather \
         than finding the right pair for the wrong reason",
    );
    check(
        (
            subjects.len(),
            mentioned.len(),
            baseline_records.len(),
            prose_only.len(),
        ) == (221, 29, 66, 1),
        "INPUTS: 221 registered ids, 29 reads holding records over 66 \
         QUESTIONS, 1 answering `--json` in prose. Round 1051: the panel used \
         to guess a read's arguments in two shapes — nothing, or a telling — so \
         two reads were counted as an unaskable NUMBER and never entered any \
         population here, and every swept read was asked at ONE point of its \
         argument space. The 221st id is `main`, and it arrived because this \
         walk and the R1054 census had each derived `the ids the store \
         registers` by hand and DISAGREED about exactly that one — a world \
         every store has whether or not it registers a branch. One resolver \
         now (`common::registered_ids`), and what the disagreement cost is \
         below in ADDRESSING",
    );
    check(
        (applied, refused) == (92, 0),
        "POPULATION: 92 authorable corruptions, none refused by the write path. \
         It was 41 until Round 1054, and the 51 that arrived are the legs that \
         PLACE a fact and ATTRIBUTE it — where it becomes true, where it stops, \
         whose view holds it, which world-line authored it. This derivation had \
         always said it took the legs a fact actually carries and had carried \
         only the ones saying what a fact CLAIMS, so no edit here ever moved a \
         coordinate, a frame or a branch",
    );
    // MENTION IS VACUOUS AND THE WALK SAYS SO EVERY RUN rather than in prose:
    // every registered id is mentioned by more than one read at baseline,
    // because three reads render the whole store. If that stops holding, the
    // reason this walk perturbs instead of reading has changed.
    check(
        subjects.iter().all(|subject| {
            baseline_records
                .values()
                .filter(|per_read| per_read.by_subject.contains_key(subject))
                .count()
                > 1
        }),
        "VACUITY: keyed on mention alone EVERY subject is shared, which is why \
         answering is measured by perturbation",
    );
    check(
        (readers_of.len(), shared.len()) == (216, 105),
        "ANSWERED: subjects some read answers about, and those more than one \
         does. The second number was 79, then 210 of 214 when Round 1054 made \
         three reads NAME the sets they had been counting: under the \
         minimal-enclosing-object rule a fact listed in a scene's census \
         carried that whole row, so it moved whenever a NEIGHBOUR did, and \
         three lists holding most of the store made nearly every pair of reads \
         share nearly every subject. Round 1055 gives a bare id in a list the \
         ADDRESS of that list as its record, so a co-listed neighbour moving is \
         no longer this subject's record moving — and the census still sees a \
         fact change SCENE, because the scene is in the address.\n\n\
         THE FIRST NUMBER WENT UP while the second fell, and that is the same \
         change rather than a second one: an address distinguishes two places \
         that used to collapse. The manuscript writes the same scene row under \
         every road that plays it, so a fact leaving one road's copy left the \
         other copies to serialize identically and the old record set did not \
         move at all — 216 of 221 subjects have a read that answers about \
         them, against 214 when a record was a value with no address",
    );
    check(
        permuted.is_empty(),
        "ORDER: no authorable edit permutes a listed set while leaving its \
         membership alone — the one thing a membership record cannot see, \
         measured over every corruption and every list rather than assumed. A \
         read that ORDERS the ids it lists would need position in the address, \
         and this is the assertion that would say so",
    );
    check(
        unaddressed.is_empty() && collisions == 0,
        "ADDRESSING: every row of an array of objects is keyed by a field \
         holding a subject id that is distinct across the rows, so no record is \
         addressed by the position it happens to sit at, and no two places in \
         one answer collide on one address",
    );
    check(
        ranked
            .iter()
            .map(|((a, b), n)| format!("{n} {a} <-> {b}"))
            .collect::<Vec<_>>()
            == BACKLOG,
        "BACKLOG: every read pair that answers about a subject in common, \
         most-shared first",
    );

    assert_eq!(
        broken,
        Vec::<String>::new(),
        "the derived read-agreement population no longer holds"
    );
}
