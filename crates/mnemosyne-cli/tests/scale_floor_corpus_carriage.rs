//! The blind re-extraction stores of the scale-floor experiment, carried into
//! the tracked population — and what the carriage had to invent to do it
//! (Round 1176).
//!
//! WHY THIS CORPUS. Round 473 ran the scale-floor experiment on two full-length
//! stories, blind: separate extractors read a 100 KB story each and built a
//! store from it, and the verdict (plain beat loop 3-0, dangling 3 vs 5) is the
//! one measurement this track has of whether the substrate holds up at a
//! novel's scale. Its stores then sat OUTSIDE this repository, and the score
//! could not be re-asked: a walk over the tracked corpora could not reach them,
//! and every law written since Round 1036 asks the tracked corpora. The largest
//! authored records this project has were the ones its gates never saw.
//!
//! WHAT STOPPED THEM, measured rather than assumed (Round 1176):
//!
//!   1. The removed `value` / `scalar` object shape (R708) — the same break
//!      thirteen tracked corpora already needed carried, EXCEPT that here the
//!      typed legs live in `typing-proposals.json` rather than in the fact
//!      manifest, so the vocabulary derivation had to widen to the corpus's
//!      reviewed proposals or it read this author as using nothing.
//!   2. A split manifest — the harness wrote its registries to
//!      `registries.json` and a later pass's facts to `supplement.json`, and
//!      `import-facts` takes one manifest.
//!   3. The pre-R697 transition rule — `allowed: [["alive", "dead"]]`, a pair
//!      list in the rules file, which today's parser rejects outright.
//!
//! None of the three is about what its author wrote, which is the whole point:
//! a record that stops loading because the schema moved is not evidence about
//! the record.

use std::collections::{BTreeMap, BTreeSet};

use mnemosyne_atomic::AtomicStore;

use crate::common;
use common::{
    authored_stores, corpus_fact_manifest, corpus_typed_legs, corpus_workspace_try, read_json,
    repo_root, run, upgrade_corpus_rules, workspace_try, Manifests, SIDECAR,
};

/// The two stores, by the name every walk prints.
const ARMS: [&str; 2] = [
    "claudedocs/phase1-scale-floor-experiment/run/store-A",
    "claudedocs/phase1-scale-floor-experiment/run/store-B",
];

/// Both arms are in the population, and each carries what its author BUILT —
/// not merely what its fact manifest happens to hold.
///
/// The counts are pinned per arm because the reviewed second pass is what makes
/// this corpus worth asking: an import that silently skipped the proposals
/// would still produce a loadable store, one with every typed leg and every
/// succession edge missing, and no other law in this tree would notice. Typed
/// legs and edges are the only things the transition and exclusivity rules can
/// see at all.
#[test]
fn both_blind_arms_are_in_the_population_with_their_reviewed_second_pass() {
    let (stores, _) = authored_stores();
    let mut measured: BTreeMap<String, (usize, usize, usize, usize)> = BTreeMap::new();
    for arm in ARMS {
        let store = stores
            .iter()
            .find(|store| store.name.split(" (").next() == Some(arm))
            .unwrap_or_else(|| {
                panic!(
                    "`{arm}` is not in the population this tree can ask; the loadable names are \
                     {:?}",
                    stores.iter().map(|s| &s.name).collect::<Vec<_>>()
                )
            });
        let loaded = AtomicStore::load(&store.ws.path().join(SIDECAR))
            .unwrap_or_else(|e| panic!("`{arm}` built a store that will not load back: {e}"));
        let typed = loaded
            .narrative_facts
            .values()
            .filter(|fact| fact.typed.is_some())
            .count();
        let succession = loaded
            .narrative_facts
            .values()
            .filter(|fact| fact.supersedes_in_frame.is_some())
            .count();
        measured.insert(
            arm.to_string(),
            (
                loaded.sections.len(),
                loaded.narrative_facts.len(),
                typed,
                succession,
            ),
        );
    }

    for (arm, (sections, facts, typed, succession)) in &measured {
        println!("  {arm}: {sections} scenes, {facts} facts, {typed} typed, {succession} chained");
    }

    // Facts = the author's manifest plus the ONE edge fact the R697 carriage
    // adds per arm (`alive -> dead`), which the next test holds to that count.
    assert_eq!(
        measured,
        BTreeMap::from([
            (ARMS[0].to_string(), (60, 83, 19, 9)),
            (ARMS[1].to_string(), (65, 97, 19, 9)),
        ]),
        "what the blind arms carry into the store"
    );
}

/// The rules carriage is not a formality: the step the author LICENSED is
/// licensed, and a step they FORBADE is rejected.
///
/// This is the round's non-emptiness injection, and it is written this way
/// because the obvious version is empty. Asking the authored store whether
/// `dead -> alive` is licensed answers "no" for the wrong reason: neither arm
/// CONTAINS that step, so the absence of a reject proves nothing about the
/// carried rule and everything about what the extractors happened to write.
/// (Measured — the first draft of this test asserted exactly that and passed
/// against a store with an empty refusal set.)
///
/// So the reverse step is AUTHORED IN, through the manifest an author could
/// equally have written, and the gate must then reject it. A carriage that
/// dropped the constraint, symmetrised the edge, or wired an adjacency
/// predicate no rule reads all fail here; one that invented a constraint the
/// author never wrote fails the first half.
#[test]
fn the_carried_life_arc_licenses_the_authored_step_and_rejects_a_reversed_one() {
    for arm in ARMS {
        let dir = repo_root().join(arm);
        let authored = corpus_fact_manifest(&dir);

        let steps = |facts: &serde_json::Value| -> (BTreeSet<(String, String)>, usize) {
            let ws = corpus_workspace_try(&dir, facts)
                .unwrap_or_else(|e| panic!("{arm}: the manifest must import: {e}"));
            // Read the judgement, not the exit code: `validate-continuity`
            // exits non-zero on ANY violation, and a hundred-scene blind
            // extraction having some is not this law's business.
            let out = run(ws.path(), &["validate-continuity", "--json"]);
            let report: serde_json::Value =
                serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
                    panic!(
                        "{arm}: the continuity read emitted no json ({e}): {}",
                        String::from_utf8_lossy(&out.stderr)
                    )
                });
            let licensed = report["step_judgements"]
                .as_array()
                .expect("the continuity read judges steps")
                .iter()
                .filter(|step| step["licensed"].as_bool() == Some(true))
                .map(|step| {
                    (
                        step["from"].as_str().unwrap_or_default().to_string(),
                        step["to"].as_str().unwrap_or_default().to_string(),
                    )
                })
                .collect();
            let rejects = report["violations"]
                .as_array()
                .map(|rows| {
                    rows.iter()
                        .filter(|v| v["kind"] == "rule_transition_invalid")
                        .count()
                })
                .unwrap_or_default();
            (licensed, rejects)
        };

        let (licensed, rejects) = steps(&authored);
        println!("  {arm} as authored: licensed {licensed:?}, transition rejects {rejects}");
        assert!(
            licensed.contains(&("alive".to_string(), "dead".to_string())),
            "{arm}: the author's own `alive -> dead` step is not licensed, so the carried map is \
             not the map they declared — licensed {licensed:?}"
        );
        assert_eq!(
            rejects, 0,
            "{arm}: the corpus as its author left it must not violate their own rule"
        );

        // The injection: a subject who dies and then is alive again, on the
        // arm's own seat, typed with the arm's own predicate and tokens.
        let mut reversed = authored.clone();
        let seat = reversed["facts"][0].clone();
        for (id, token, supersedes) in [
            ("probe-arc-dead", "dead", None),
            ("probe-arc-alive", "alive", Some("probe-arc-dead")),
        ] {
            let mut fact = seat.clone();
            let row = fact.as_object_mut().expect("a fact is an object");
            row.insert("fact_id".to_string(), serde_json::json!(id));
            row.insert(
                "claim".to_string(),
                serde_json::json!(format!("Probe: augustin-roeder is {token}.")),
            );
            row.insert(
                "entities".to_string(),
                serde_json::json!(["augustin-roeder"]),
            );
            row.insert(
                "typed".to_string(),
                serde_json::json!({
                    "subject": "augustin-roeder",
                    "predicate": "life_state",
                    "object": { "kind": "token", "token": token },
                }),
            );
            match supersedes {
                Some(predecessor) => {
                    row.insert(
                        "supersedes_in_frame".to_string(),
                        serde_json::json!(predecessor),
                    );
                }
                None => {
                    row.remove("supersedes_in_frame");
                }
            }
            row.remove("quote");
            row.remove("pays_off");
            row.remove("payoff_expectation");
            reversed["facts"]
                .as_array_mut()
                .expect("facts array")
                .push(fact);
        }

        let (licensed, rejects) = steps(&reversed);
        println!("  {arm} with a reversed arc: licensed {licensed:?}, rejects {rejects}");
        assert!(
            rejects >= 1,
            "{arm}: a `dead -> alive` step passed the carried rule, so the rule forbids nothing — \
             licensed {licensed:?}"
        );
        assert!(
            !licensed.contains(&("dead".to_string(), "alive".to_string())),
            "{arm}: `dead -> alive` came back licensed, so the carriage symmetrised the edge"
        );
    }
}

/// The carriage, asked directly, adds EXACTLY the rows the author's pair list
/// implies — and says so.
///
/// The upgrade is the one place in the corpus recipe that INVENTS rather than
/// derives (a typed subject must be a registered entity, so a state token
/// becomes one), and an invention nobody counts is how a walk comes to read a
/// recipe's row as an author's. Both arms declare the same one-step arc, so the
/// carriage's whole output is pinned here.
#[test]
fn the_transition_carriage_names_every_row_it_adds() {
    for arm in ARMS {
        let dir = repo_root().join(arm);
        let mut facts = corpus_fact_manifest(&dir);
        common::upgrade_corpus_manifest(&mut facts, &corpus_typed_legs(&dir));
        let before = facts["facts"].as_array().map(Vec::len).unwrap_or_default();
        let mut rules = read_json(&dir.join("narrative-rules.json"));
        let carriage = upgrade_corpus_rules(&mut rules, &mut facts)
            .unwrap_or_else(|| panic!("`{arm}` declares a pre-R697 transition rule to carry"));

        assert_eq!(carriage.rules, ["alive-arc"], "{arm}");
        assert_eq!(carriage.adjacency, ["life_state_adjacent"], "{arm}");
        assert_eq!(
            carriage.steps,
            [("alive".to_string(), "dead".to_string())],
            "{arm}"
        );
        assert_eq!(carriage.state_entities, ["alive"], "{arm}");
        assert_eq!(
            facts["facts"].as_array().map(Vec::len).unwrap_or_default(),
            before + 1,
            "{arm}: one edge fact per authored step, and nothing else"
        );
        // The rewritten rule is what the parser will read: the pair list is
        // gone and the adjacency predicate is named in its place.
        let rule = &rules["rules"][0];
        assert!(rule.get("allowed").is_none(), "{arm}: {rule}");
        assert_eq!(rule["adjacency"], "life_state_adjacent", "{arm}");
        // The author's OTHER rule is untouched — the carriage is scoped to the
        // form that stopped parsing.
        assert_eq!(rules["rules"][1]["class"], "exclusive", "{arm}");
        assert!(rules["rules"][1].get("adjacency").is_none(), "{arm}");
    }
}

// ==========================================================================
// R1213 — the arcs no corpus in this tree declares, and whether each invented
// row is NEEDED.
//
// R1176 shipped the carriage with two witnesses that declare the SAME one-step
// arc, and wrote down what that leaves unasked: "a multi-step arc, a cycle, or
// a rule whose FROM token is already a registered entity would each take a path
// this round did not walk". Those three sentences describe the work rather than
// a limit of the world, so they are walked here — against corpora built for the
// purpose, because no author in this tree writes one.
//
// The same round said the invention is "measurement, not proof": the write path
// taught the shape by rejecting twice, and nothing in the tree re-asks it. The
// second half below re-asks it by REMOVAL, which is the only form of minimality
// a measurement can carry — each invented row is dropped in turn and the import
// must refuse, and the untouched carriage must import.
// ==========================================================================

/// A rules file declaring one pre-R697 transition rule over `steps`.
fn rules_declaring(steps: &[(&str, &str)]) -> serde_json::Value {
    serde_json::json!({
        "rules": [{
            "id": "probe-arc",
            "class": "transition",
            "predicate": "probe_state",
            "allowed": steps
                .iter()
                .map(|(from, to)| serde_json::json!([from, to]))
                .collect::<Vec<_>>(),
        }]
    })
}

/// A fact manifest with the one seat fact the carriage clones, declaring
/// `entities` as already registered.
fn manifest_declaring(entities: &[&str]) -> serde_json::Value {
    serde_json::json!({
        "entities": entities
            .iter()
            .map(|id| serde_json::json!({ "entity_id": id, "kind": "person" }))
            .collect::<Vec<_>>(),
        "facts": [{
            "fact_id": "seat",
            "claim": "The fact this corpus opens on.",
            "scene": "one",
        }],
    })
}

/// Rows of one manifest key, by their id field.
fn ids(manifest: &serde_json::Value, key: &str, id_field: &str) -> Vec<String> {
    manifest
        .get(key)
        .and_then(|rows| rows.as_array())
        .map(|rows| {
            rows.iter()
                .filter_map(|row| row[id_field].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn a_multi_step_arc_registers_every_state_it_leaves_and_none_it_only_enters() {
    let mut facts = manifest_declaring(&[]);
    let mut rules = rules_declaring(&[("a", "b"), ("b", "c")]);
    let carriage =
        upgrade_corpus_rules(&mut rules, &mut facts).expect("the rule is a pre-R697 pair list");

    assert_eq!(carriage.steps.len(), 2, "{carriage:?}");
    // A TYPED SUBJECT MUST BE AN ENTITY AND A TOKEN OBJECT MUST NOT, so what a
    // step LEAVES becomes an entity and what it only ENTERS stays a token. `c`
    // is never a subject here, and registering it would be an invention the
    // pair list does not imply.
    assert_eq!(carriage.state_entities, ["a", "b"], "{carriage:?}");
    assert_eq!(ids(&facts, "entities", "entity_id"), ["a", "b"]);
    // One predicate for the rule, one entity kind for the states it invented,
    // one edge fact per step, and the seat untouched.
    assert_eq!(
        ids(&facts, "predicates", "predicate_id"),
        ["probe_state_adjacent"]
    );
    assert_eq!(
        ids(&facts, "entity_kinds", "kind_id"),
        ["probe_state-state"]
    );
    assert_eq!(
        ids(&facts, "facts", "fact_id"),
        [
            "seat",
            "probe_state_adjacent-a-to-b",
            "probe_state_adjacent-b-to-c"
        ]
    );
    // And the predicate's token list holds every state either leg names, or the
    // edge fact's object would be a token the predicate does not admit.
    assert_eq!(
        facts["predicates"][0]["object_tokens"],
        serde_json::json!(["a", "b", "c"])
    );
}

#[test]
fn a_cycle_carries_both_directions_as_two_edges_and_registers_each_state_once() {
    let mut facts = manifest_declaring(&[]);
    let mut rules = rules_declaring(&[("a", "b"), ("b", "a")]);
    let carriage =
        upgrade_corpus_rules(&mut rules, &mut facts).expect("the rule is a pre-R697 pair list");

    assert_eq!(
        carriage.steps,
        [
            ("a".to_string(), "b".to_string()),
            ("b".to_string(), "a".to_string())
        ],
        "a cycle is two steps and the carriage must not fold them"
    );
    assert_eq!(
        carriage.state_entities,
        ["a", "b"],
        "each state is registered once however many steps leave it: {carriage:?}"
    );
    assert_eq!(
        ids(&facts, "facts", "fact_id"),
        [
            "seat",
            "probe_state_adjacent-a-to-b",
            "probe_state_adjacent-b-to-a"
        ],
        "the two directions are two facts, and their ids differ"
    );
}

#[test]
fn a_state_the_corpus_already_registers_is_not_invented_a_second_time() {
    let mut facts = manifest_declaring(&["a"]);
    let mut rules = rules_declaring(&[("a", "b"), ("b", "c")]);
    let carriage =
        upgrade_corpus_rules(&mut rules, &mut facts).expect("the rule is a pre-R697 pair list");

    assert_eq!(
        carriage.state_entities,
        ["b"],
        "`a` is the author's own entity, so carrying it again would be the recipe \
         claiming a row the author wrote: {carriage:?}"
    );
    assert_eq!(
        ids(&facts, "entities", "entity_id"),
        ["a", "b"],
        "and the author's row is left as it stands"
    );
    // THE KIND IS DECLARED ONLY WHERE A STATE IS INVENTED. A corpus whose every
    // FROM token it already registers gets no `-state` kind at all, which is the
    // arm below.
    assert_eq!(
        ids(&facts, "entity_kinds", "kind_id"),
        ["probe_state-state"]
    );

    let mut facts = manifest_declaring(&["a", "b"]);
    let mut rules = rules_declaring(&[("a", "b"), ("b", "a")]);
    let carriage = upgrade_corpus_rules(&mut rules, &mut facts).expect("a pre-R697 pair list");
    assert!(
        carriage.state_entities.is_empty(),
        "nothing was invented here: {carriage:?}"
    );
    assert!(
        ids(&facts, "entity_kinds", "kind_id").is_empty(),
        "so no kind is declared either — a kind with no member is a row the \
         recipe added for nobody"
    );
}

/// Every row the carriage invents is one the import REFUSES to do without, and
/// the three of them together are enough.
///
/// This is what "minimal" can mean for a measurement: each invented row is
/// removed in turn from a manifest that imports, and the import must refuse; the
/// untouched one must import. It cannot prove no OTHER shape would also work —
/// that is not a question a run can answer — but it does answer the two halves
/// that were being taken on trust, and R1176's own words for the state of this
/// knowledge were "the write path taught the constraint by rejecting twice".
#[test]
fn each_row_the_carriage_invents_is_one_the_import_refuses_to_do_without() {
    let arm = ARMS[0];
    let dir = repo_root().join(arm);
    let sections = read_json(&dir.join("sections.json"));
    let order = read_json(&dir.join("order.json"));
    let carried = || {
        let mut facts = corpus_fact_manifest(&dir);
        common::upgrade_corpus_manifest(&mut facts, &corpus_typed_legs(&dir));
        let mut rules = read_json(&dir.join("narrative-rules.json"));
        let carriage =
            upgrade_corpus_rules(&mut rules, &mut facts).expect("this arm carries a rule");
        (facts, carriage)
    };
    let imports = |facts: serde_json::Value| -> Result<(), String> {
        workspace_try(
            &Manifests {
                sections: sections.clone(),
                order: order.clone(),
                facts,
            },
            None,
        )
        .map(|_| ())
    };

    // THE CONTROL, and it is half the law: with the three inventions and nothing
    // else, the manifest imports. A fourth missing row would fail here.
    let (whole, carriage) = carried();
    imports(whole.clone()).unwrap_or_else(|e| {
        panic!("{arm}: the carried manifest must import as it stands, and did not: {e}")
    });

    let adjacency = carriage.adjacency[0].clone();
    let state = carriage.state_entities[0].clone();
    // THE KIND IS READ OFF THE ROW THE CARRIAGE WROTE, not spelled again here: a
    // second spelling of `{predicate}-state` would agree with the recipe until
    // the day it did not.
    let kind = whole["entities"]
        .as_array()
        .expect("the manifest holds entities")
        .iter()
        .find(|row| row["entity_id"] == serde_json::json!(state))
        .and_then(|row| row["kind"].as_str())
        .unwrap_or_else(|| panic!("{arm}: the invented state `{state}` names a kind"))
        .to_string();
    for (what, key, id_field, id) in [
        (
            "the adjacency predicate",
            "predicates",
            "predicate_id",
            adjacency.as_str(),
        ),
        ("the state entity", "entities", "entity_id", state.as_str()),
        (
            "the entity kind that state belongs to",
            "entity_kinds",
            "kind_id",
            kind.as_str(),
        ),
    ] {
        let mut without = whole.clone();
        // `drop_row` refuses a removal that removed nothing, so a case about a
        // row the carriage did not add fails here rather than passing quietly.
        drop_row(&mut without, key, id_field, id);
        let verdict = imports(without);
        assert!(
            verdict.is_err(),
            "{arm}: the import accepted a manifest without {what}, so the carriage adds a row \
             nothing requires"
        );
        println!("  {arm} without {what}: refused");
    }
}

/// Remove the row of `key` whose `id_field` is `id`, and say so if there was
/// none — a removal that removed nothing is a case about nothing.
fn drop_row(manifest: &mut serde_json::Value, key: &str, id_field: &str, id: &str) {
    let rows = manifest[key]
        .as_array_mut()
        .unwrap_or_else(|| panic!("`{key}` is an array of rows"));
    let before = rows.len();
    rows.retain(|row| row[id_field] != serde_json::json!(id));
    assert_eq!(
        rows.len() + 1,
        before,
        "`{key}` held no row whose {id_field} is `{id}`"
    );
}
