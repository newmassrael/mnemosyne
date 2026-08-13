//! The frontier and the coverage report answer "what still dangles on this
//! road" — and one of them silently drops the roads where the answer is NONE.
//! (Round 1042.)
//!
//! The second declared cross-read agreement, on the pair the Round 1040 walk
//! ranked next: `report-authoring-frontier <-> report-payoff-coverage`, 13
//! subjects in common. Round 1041 established the method and why it is a
//! DECLARATION rather than a derivation — five derivations over read output
//! have failed to decide agreement, and the fifth was refuted by injection.
//!
//! THE SHARED QUESTION. `report-payoff-coverage` reports, per world, the
//! `dangling` setups — the ones visible on that road with no payoff to credit
//! them. `report-authoring-frontier` reports `dangling_setups`, keyed by world,
//! and its own `total_gaps` counts them. Both are the author's outstanding
//! work on a road, and today the frontier reuses `payoff_coverage` to get them,
//! so the LISTS cannot drift apart without someone re-deriving one of them.
//!
//! THE WORLD SETS CAN, AND DO. `dangling_by_world()` keeps only the worlds with
//! work outstanding, so a road where the author has paid every gun is absent
//! from the frontier's map and present in coverage's with an empty list. A
//! consumer joining the two by world reads the missing key as "no answer" where
//! coverage says "the answer is none" — the R1038 shape, in which comparing
//! only where both answered lets one side lose a road in silence.
//!
//! So the omission is not asserted away, it is PINNED as the contract: the
//! frontier carries exactly the roads coverage calls non-empty, and the
//! disagreement list names any road where that stops holding in either
//! direction. Asked of every corpus an author shipped that still loads (the
//! R1036 population rule), with the totals asserted rather than printed —
//! including how many roads exercise the omission arm, because an arm no
//! corpus reaches is a claim with no evidence behind it (Round 1041 shipped
//! one of those and said so).

use std::collections::BTreeSet;

use crate::common;
use common::{authored_stores, run};

/// The pair of shipped reads this contract judges, named ONCE and run from
/// here. The backlog walk (`surface/read_agreement_population.rs`) reads this
/// declaration out of the source, because it ranks 87 pairs by shared subjects
/// to say which to compare next and could not otherwise tell which of them
/// already have a contract.
const DECLARES: [&str; 2] = ["report-authoring-frontier", "report-payoff-coverage"];

/// A read's `[id, ...]` list of setup ids.
fn ids(list: &serde_json::Value, whose: &str) -> Vec<String> {
    list.as_array()
        .unwrap_or_else(|| panic!("{whose} is a list"))
        .iter()
        .map(|v| {
            v.as_str()
                .unwrap_or_else(|| panic!("{whose} holds setup ids"))
                .to_string()
        })
        .collect()
}

#[test]
fn the_frontier_carries_exactly_the_roads_coverage_says_still_dangle() {
    let mut answered = 0usize;
    let mut roads_with_work = 0usize;
    let mut roads_all_paid = 0usize;
    let mut setups_compared = 0usize;
    let mut disagreements: Vec<String> = Vec::new();

    let (stores, unloadable) = authored_stores();
    let asked = stores.len() + unloadable.len();
    let mut silent: Vec<String> = unloadable.iter().map(ToString::to_string).collect();
    for store in &stores {
        let name = &store.name;
        let ws = &store.ws;
        let read = |verb: &str| {
            let out = run(ws.path(), &[verb, "--json"]);
            out.status
                .success()
                .then(|| {
                    serde_json::from_slice::<serde_json::Value>(&out.stdout)
                        .unwrap_or_else(|e| panic!("{verb} on {name} is not json: {e}"))
                })
                .ok_or_else(|| verb.to_string())
        };
        let (frontier, coverage) = match (read(DECLARES[0]), read(DECLARES[1])) {
            (Ok(f), Ok(c)) => (f, c),
            (f, c) => {
                let refused: Vec<String> = [f, c].into_iter().filter_map(Result::err).collect();
                silent.push(format!("{name} ({} refused)", refused.join(" ")));
                continue;
            }
        };
        answered += 1;

        let empty = serde_json::Map::new();
        let cov_worlds = coverage["worlds"].as_object().unwrap_or(&empty);
        let front_worlds = frontier["dangling_setups"].as_object().unwrap_or(&empty);
        let roads: BTreeSet<&String> = cov_worlds.keys().chain(front_worlds.keys()).collect();
        for road in roads {
            let listed = front_worlds.get(road).map(|v| ids(v, "frontier"));
            let Some(cov) = cov_worlds.get(road) else {
                // A road the frontier knows and coverage does not: the frontier
                // took its roads FROM coverage, so this is a re-derivation.
                disagreements.push(format!(
                    "{name}/{road}: the frontier carries this road and coverage does not",
                ));
                continue;
            };
            let dangling = ids(&cov["dangling"], "coverage.dangling");
            if dangling.is_empty() {
                roads_all_paid += 1;
                if let Some(listed) = listed {
                    disagreements.push(format!(
                        "{name}/{road}: coverage says nothing dangles and the frontier \
                         lists {listed:?}",
                    ));
                }
                continue;
            }
            roads_with_work += 1;
            setups_compared += dangling.len();
            match listed {
                None => disagreements.push(format!(
                    "{name}/{road}: coverage says {dangling:?} still dangle and the \
                     frontier does not carry the road at all",
                )),
                Some(listed) if listed != dangling => disagreements.push(format!(
                    "{name}/{road}: coverage says {dangling:?} and the frontier says \
                     {listed:?}",
                )),
                Some(_) => {}
            }
        }
    }

    // Print BEFORE asserting (the R1026 lesson).
    println!(
        "{asked} authored corpora asked, {answered} answered both reads, {} could not:",
        silent.len(),
    );
    for name in &silent {
        println!("    skip {name}");
    }
    println!(
        "{roads_with_work} roads still owe work ({setups_compared} dangling setups on \
         them), {roads_all_paid} roads have paid every gun"
    );
    for row in &disagreements {
        println!("    DISAGREE {row}");
    }

    // THE EVIDENCE, asserted. `roads_all_paid` is the arm that exercises the
    // OMISSION half of the contract: if it ever reads 0, the claim that the
    // frontier drops empty roads is being carried by no corpus at all.
    assert_eq!(
        (
            asked,
            answered,
            roads_with_work,
            roads_all_paid,
            setups_compared
        ),
        (44, 41, 31, 64, 81),
        "the corpora that answer both reads, and how much they compare"
    );

    assert_eq!(
        disagreements,
        Vec::<String>::new(),
        "the two reads disagree about what still dangles on a road"
    );
}
