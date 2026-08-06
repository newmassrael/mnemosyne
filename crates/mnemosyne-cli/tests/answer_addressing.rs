//! WHAT A READ WROTE ABOUT AN ID, AND WHERE — the definition's own test.
//! (Round 1055, moved here in Round 1057.)
//!
//! The derivation is [`common::wrote_about`], and it has two consumers: the
//! read-agreement population asks whether a subject's record MOVED, and the
//! census of lossy numbers asks where a number SITS and which names could
//! account for it. It lived in the first of those and judged what both of them
//! stand on, which is the shape Round 1054 met from the other side — a pin in
//! the file that happened to meet it, judging something of a different rank.
//!
//! A SUBJECT is an id the store registers, and where a read writes it decides
//! what the read said about it:
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
//! AND AN ADDRESS IS NOT A FIELD. The address identifies a ROW and carries the
//! key that picks it out; the field is a PLACE IN THE SHAPE of the read and
//! carries `[]`. Round 1055 spelled the key into both, and the key is DERIVED
//! from the values (the fields holding a subject id in every row, distinct
//! across them), so one place came out under different names in two answers of
//! one read — which is invisible to a law that compares an answer to itself and
//! fatal to one that asks what a number counts. Round 1057 measured it: the
//! census of lossy numbers had been reporting the quest graph's locator ordinal
//! as two fields since Round 1054.
//!
//! A number cannot say which rule shipped — both of the rules Round 1055
//! rejected produce perfectly good numbers — so the rule is asserted here, on
//! answers this file writes, and the claims that decided its shape are the ones
//! that killed the alternatives.

use std::collections::{BTreeMap, BTreeSet};

mod common;
use common::{permutations, wrote_about, Wrote};

/// THE DEFINITION'S OWN TEST — what a record IS, asked of answers this test
/// writes rather than of the shipped surface.
///
/// The two walks that use it measure the surface WITH this rule and report
/// numbers, and a number cannot say whether the rule is the one written down.
/// The two claims that decided its shape are the two that killed the
/// alternatives — a NEIGHBOUR moving must not move this subject's record (which
/// is what the rule that shipped until R1055 does), and a fact changing SCENE
/// must move it (which is what the obvious repair, membership with no address,
/// cannot see).
#[test]
fn a_record_is_what_the_read_wrote_about_that_id() {
    let subjects: BTreeSet<String> = ["f-1", "f-2", "f-3", "sec-1", "sec-2", "r-1", "r-2", "q-1"]
        .into_iter()
        .map(ToString::to_string)
        .collect();
    let wrote = |answer: &serde_json::Value| {
        let out = wrote_about(answer, &subjects);
        assert_eq!(out.collisions, 0, "an address is produced once");
        out
    };
    let one =
        |address: &str, value: serde_json::Value| BTreeMap::from([(address.to_string(), value)]);
    let member = serde_json::Value::Null;

    // A CENSUS: facts listed per scene, which is the shape that decided the
    // rule. `scene` keys the rows because every row holds one and no two rows
    // hold the same.
    let census = |first: &[&str], second: &[&str]| {
        serde_json::json!({"scene_coverage": [
            {"scene": "sec-1", "facts": first},
            {"scene": "sec-2", "facts": second},
        ]})
    };
    let before = wrote(&census(&["f-1", "f-2"], &["f-3"]));
    assert_eq!(
        before.by_subject["f-1"],
        one("scene_coverage[scene=sec-1].facts[]", member.clone()),
        "a bare id in a list is addressed down through the ancestors that key \
         it, and that address is the whole of the record — the ancestors above \
         it hold every other id too and claim none of them"
    );
    assert_eq!(
        before.by_subject["sec-1"],
        one(
            "scene_coverage[scene=sec-1]",
            serde_json::json!({"scene": "sec-1", "facts": ["f-1", "f-2"]})
        ),
        "the id that KEYS the row has the row itself as its record, census and \
         all: the read wrote that list about that scene"
    );

    // A NEIGHBOUR MOVES. This is what the minimal-enclosing-object rule got
    // wrong, and it got it wrong in the direction that inflates: f-1's record
    // was the whole row, so f-2 leaving moved it.
    let neighbour_left = wrote(&census(&["f-1"], &["f-2", "f-3"]));
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
    let moved = wrote(&census(&["f-2"], &["f-1", "f-3"]));
    assert_eq!(
        moved.by_subject["f-1"],
        one("scene_coverage[scene=sec-2].facts[]", member.clone()),
        "a fact anchored at another scene is listed at another address"
    );
    assert_ne!(moved.by_subject["f-1"], before.by_subject["f-1"]);

    // A MAP KEYED BY ID. What the read wrote about `r-1` is the subtree under
    // it — not the map, which holds every other road beside it. And a number
    // inside that subtree belongs to THAT record: `owned_facts` is part of the
    // read's account of `r-1`, which is what lets the census of lossy numbers
    // ask whether anything NAMED moved alongside it.
    let worlds = |mine: &[&str], theirs: &[&str]| {
        serde_json::json!({"worlds": {
            "r-1": {"owned": mine, "owned_facts": mine.len()},
            "r-2": {"owned": theirs, "owned_facts": theirs.len()},
        }})
    };
    let roads = wrote(&worlds(&["f-1"], &["f-3"]));
    assert_eq!(
        roads.by_subject["r-1"],
        one(
            "worlds.r-1",
            serde_json::json!({"owned": ["f-1"], "owned_facts": 1})
        ),
    );
    assert_eq!(
        roads.by_subject["f-1"],
        one("worlds.r-1.owned[]", member.clone()),
    );
    assert_eq!(
        roads.numbers["worlds.r-1"],
        BTreeMap::from([("worlds.*.owned_facts".to_string(), vec!["1".to_string()])]),
        "a number is filed under the innermost record that holds it, with the \
         id-keyed step collapsed so a per-road map is ONE field rather than one \
         finding per road"
    );
    let other_road = wrote(&worlds(&["f-1"], &["f-2", "f-3"]));
    assert_eq!(
        other_road.by_subject["r-1"], roads.by_subject["r-1"],
        "a change under a SIBLING key is not a change under this one — the map \
         that holds both is nobody's record"
    );
    assert_ne!(other_road.by_subject["r-2"], roads.by_subject["r-2"]);

    // A ROW THAT NAMES AN ID IN A FIELD is the sentence about it, so anything
    // else in that row is part of what the read said.
    let quest = |state: &str| serde_json::json!({"quests": [{"quest": "q-1", "state": state}]});
    let unknown = wrote(&quest("unknown"));
    assert_eq!(
        unknown.by_subject["q-1"],
        one(
            "quests[quest=q-1]",
            serde_json::json!({"quest": "q-1", "state": "unknown"})
        ),
    );
    assert_ne!(
        wrote(&quest("discharged")).by_subject["q-1"],
        unknown.by_subject["q-1"],
        "the verdict beside the id is what makes this read ANSWER about it"
    );

    // A NUMBER NO RECORD HOLDS belongs to the answer's root, which is a record
    // at address "" whether or not it names anything. That is what keeps the
    // census of lossy numbers reaching a top-level total: judged against the
    // whole answer, exactly as it was before records scoped it.
    let totals = wrote(&serde_json::json!({"facts": 3, "scenes": 1}));
    assert_eq!(
        totals.numbers[""],
        BTreeMap::from([
            ("facts".to_string(), vec!["3".to_string()]),
            ("scenes".to_string(), vec!["1".to_string()]),
        ]),
    );

    // ONE SUBJECT, SEVERAL RECORDS. `world` keys these rows and `at` does not,
    // because two rows hold the same `at` — a field every row carries but two
    // rows share does not say which row you are at.
    let locators = wrote(&serde_json::json!({"locators": [
        {"world": "r-1", "at": "sec-1"},
        {"world": "r-2", "at": "sec-1"},
    ]}));
    assert_eq!(
        locators.by_subject["sec-1"],
        BTreeMap::from([
            (
                "locators[world=r-1]".to_string(),
                serde_json::json!({"world": "r-1", "at": "sec-1"})
            ),
            (
                "locators[world=r-2]".to_string(),
                serde_json::json!({"world": "r-2", "at": "sec-1"})
            ),
        ]),
    );
    assert!(locators.unaddressed.is_empty());

    // A ROW NO SINGLE FIELD KEYS. One row per (fact, world): `f-1` appears in
    // two of them and `r-1` in two of them, so neither column addresses a row
    // and the pair does. This is the shipped `inert_reveal_pins` shape.
    let pins = wrote(&serde_json::json!({"pins": [
        {"fact": "f-1", "world": "r-1"},
        {"fact": "f-1", "world": "r-2"},
        {"fact": "f-2", "world": "r-1"},
    ]}));
    assert!(pins.unaddressed.is_empty());
    assert_eq!(
        pins.by_subject["f-1"],
        BTreeMap::from([
            (
                "pins[fact=f-1,world=r-1]".to_string(),
                serde_json::json!({"fact": "f-1", "world": "r-1"})
            ),
            (
                "pins[fact=f-1,world=r-2]".to_string(),
                serde_json::json!({"fact": "f-1", "world": "r-2"})
            ),
        ]),
    );

    // A ROW NO FIELD KEYS falls back to the position it sits at, and says so
    // by name. The walk below asserts this set is EMPTY on the shipped
    // surface; here is the shape that would put something in it.
    let classes = wrote(&serde_json::json!({"classes": [
        {"kind": "exempt", "facts": ["f-1"]},
        {"kind": "dangling", "facts": ["f-2"]},
    ]}));
    assert_eq!(
        classes.unaddressed,
        BTreeSet::from(["classes[0]".to_string(), "classes[1]".to_string()]),
    );
    assert_eq!(
        classes.by_subject["f-1"],
        one("classes[0].facts[]", member.clone()),
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
    let reordered = wrote(&census(&["f-2", "f-1"], &["f-3"]));
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

    // THE FIELD IS THE PLACE, THE ADDRESS IS THE ROW (Round 1057). Two answers
    // of ONE read, differing only in whether `world` happens to be distinct
    // across the rows: `key_fields` therefore keys the first pair by `world`
    // and the second by `fact`, so the two rows have different ADDRESSES —
    // which is what an address is for. The FIELD a number sits at must not
    // move with them, or one place in one read carries two names and nothing
    // can be asked across the two answers.
    let seats = |rows: serde_json::Value| serde_json::json!({"seats": rows});
    let by_world = wrote(&seats(serde_json::json!([
        {"fact": "f-1", "world": "r-1", "at": 3},
        {"fact": "f-1", "world": "r-2", "at": 4},
    ])));
    let by_fact = wrote(&seats(serde_json::json!([
        {"fact": "f-1", "world": "r-1", "at": 3},
        {"fact": "f-2", "world": "r-1", "at": 4},
    ])));
    assert_eq!(
        (
            by_world.by_subject["f-1"]
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            by_fact.by_subject["f-1"]
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
        ),
        (
            vec!["seats[world=r-1]", "seats[world=r-2]"],
            vec!["seats[fact=f-1]"],
        ),
        "the ADDRESS carries the key that picks the row out, and which columns \
         do that is derived from the values"
    );
    let field_of = |wrote: &Wrote| {
        wrote
            .numbers
            .values()
            .flat_map(BTreeMap::keys)
            .cloned()
            .collect::<BTreeSet<String>>()
    };
    assert_eq!(
        (field_of(&by_world), field_of(&by_fact)),
        (
            BTreeSet::from(["seats[].at".to_string()]),
            BTreeSet::from(["seats[].at".to_string()])
        ),
        "and the FIELD does not: it is where in the SHAPE of this read the \
         number sits, and the shape is the same answer either way. Spelling the \
         row key into it made `report-entity` file its fact rows under `branch` \
         for an entity whose facts are one per world-line and under `fact_id` \
         for the rest, so a walk asking what `fact_count` counts found the \
         place it counts missing from half the answers"
    );
}
