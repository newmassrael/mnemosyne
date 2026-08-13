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
    repo_root, run, upgrade_corpus_rules, SIDECAR,
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
