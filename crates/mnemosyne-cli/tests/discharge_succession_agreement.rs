//! A discharge and a succession are the same typed step, read with two
//! different scopes — and only one of the two reads says so. (Round 1043.)
//!
//! The third declared cross-read agreement, on the pair the Round 1040 walk
//! ranked next: `report-edge-candidates <-> report-payoff-coverage` (12
//! subjects in common; the substantiation half of that coverage is where the
//! shared question actually lives). Rounds 1041 and 1042 established why this
//! is a DECLARATION rather than a derivation — five derivations over read
//! output failed to decide agreement, the fifth refuted by injection.
//!
//! THE SHARED QUESTION. Both reads look at the same thing: two typed claims on
//! one `(subject, predicate)` whose objects differ, i.e. a state that moved.
//! `report-payoff-substantiation` calls the later one a DISCHARGE of the
//! earlier and reports the setup substantiated. `report-edge-candidates` calls
//! the pair a SUCCESSION GAP when no `supersedes_in_frame` chain connects them,
//! which is the store telling the author "these two look like one arc, wire
//! them up".
//!
//! THEY DO NOT USE THE SAME SCOPE, AND THE STRUCTURE CANNOT. The pair walk
//! behind the gaps skips two facts in different FRAMES, which is not an
//! oversight: succession is `supersedes_in_frame`, a within-perspective
//! relation by construction. The discharge rule has no frame test at all —
//! payoff edges cross frames freely (R442) — so a ground-truth fact discharges
//! a setup written in a character's frame, and NO succession edge could ever
//! record that step. On the one blind-authored corpus, 3 of its 5 discharging
//! pairs are cross-frame — the MAJORITY of that store's positive verdicts rest
//! on steps the other read is structurally unable to see. Over every store this
//! tree can ask it is 3 of 26, and the other 23 hold: 21 are already chained by
//! an author who wired the succession, 2 are proposed as gaps.
//!
//! So this walk declares the relation WITH its exception, and counts both arms:
//! a same-frame discharge must appear to the other read (as a gap, or already
//! chained), and a cross-frame one is named as the class the succession model
//! cannot express rather than quietly skipped. Whether a cross-frame state
//! change SHOULD substantiate a setup is a design question with an owner; that
//! it is invisible to the succession surface is a measurement.
//!
//! Asked of every store an author shipped that this tree can ask
//! (`authored_stores()`, the R1042 resolver), reading only what the two shipped
//! reads emit — no store access, so this is what a consumer joining the two
//! actually holds.

use std::collections::{BTreeMap, BTreeSet};

use crate::common;
use common::{authored_stores, run};

/// The pair of shipped reads this contract judges, named ONCE and run from
/// here. The backlog walk (`surface/read_agreement_population.rs`) reads this
/// declaration out of the source, because it ranks 87 pairs by shared subjects
/// to say which to compare next and could not otherwise tell which of them
/// already have a contract.
const DECLARES: [&str; 2] = ["report-edge-candidates", "report-payoff-substantiation"];

/// The `supersedes_in_frame` chain, from the read's own rows: fact -> the fact
/// it supersedes. One backward pointer per fact, so the walk up is linear.
fn chained(rows: &[serde_json::Value]) -> BTreeSet<(String, String)> {
    let back: BTreeMap<&str, &str> = rows
        .iter()
        .filter_map(|row| {
            Some((
                row["fact_id"].as_str()?,
                row["supersedes_in_frame"].as_str()?,
            ))
        })
        .collect();
    let mut out = BTreeSet::new();
    for start in back.keys() {
        let mut here = *start;
        // Cycle-guarded: the write path rejects succession cycles, but a read
        // is re-read from a store that may have been edited out of band.
        let mut seen: BTreeSet<&str> = BTreeSet::from([here]);
        while let Some(next) = back.get(here) {
            if !seen.insert(next) {
                break;
            }
            out.insert(pair(start, next));
            here = next;
        }
    }
    out
}

/// One unordered pair of fact ids — neither read promises an order.
fn pair(a: &str, b: &str) -> (String, String) {
    if a < b {
        (a.to_string(), b.to_string())
    } else {
        (b.to_string(), a.to_string())
    }
}

#[test]
fn every_same_frame_discharge_is_a_step_the_edge_report_can_see() {
    let mut answered = 0usize;
    let mut discharges = 0usize;
    let mut same_frame = 0usize;
    let mut cross_frame = 0usize;
    let mut already_chained = 0usize;
    let mut disagreements: Vec<String> = Vec::new();

    let (stores, unloadable) = authored_stores();
    let asked = stores.len() + unloadable.len();
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
        let (edges, substantiation) = match (read(DECLARES[0]), read(DECLARES[1])) {
            (Ok(e), Ok(s)) => (e, s),
            _ => continue,
        };
        answered += 1;

        let rows = edges["facts"].as_array().expect("the edge report's rows");
        let frame_of: BTreeMap<&str, &str> = rows
            .iter()
            .filter_map(|row| Some((row["fact_id"].as_str()?, row["frame"].as_str()?)))
            .collect();
        let gaps: BTreeSet<(String, String)> = edges["succession_gaps"]
            .as_array()
            .expect("the gap list")
            .iter()
            .map(|g| {
                pair(
                    g["fact_a"].as_str().expect("a gap names a fact"),
                    g["fact_b"].as_str().expect("a gap names a fact"),
                )
            })
            .collect();
        let chain = chained(rows);

        let empty = serde_json::Map::new();
        let mut seen: BTreeSet<(String, String)> = BTreeSet::new();
        for (road, world) in substantiation["worlds"].as_object().unwrap_or(&empty) {
            for row in world["substantiated"]
                .as_array()
                .expect("the substantiated rows")
            {
                let setup = row["setup"].as_str().expect("a setup id");
                for payoff in row["discharging_payoffs"]
                    .as_array()
                    .expect("R1041's discharging list")
                {
                    let payoff = payoff.as_str().expect("a payoff id");
                    if !seen.insert(pair(setup, payoff)) {
                        continue; // the same step, seen on an earlier road
                    }
                    discharges += 1;
                    let (Some(fa), Some(fb)) =
                        (frame_of.get(setup).copied(), frame_of.get(payoff).copied())
                    else {
                        disagreements.push(format!(
                            "{name}/{road}: substantiation discharges `{setup}` with \
                             `{payoff}` and the edge report does not carry both facts",
                        ));
                        continue;
                    };
                    if fa != fb {
                        // The succession relation is within-frame by
                        // construction, so no edge could ever record this step.
                        cross_frame += 1;
                        if gaps.contains(&pair(setup, payoff)) {
                            disagreements.push(format!(
                                "{name}/{road}: `{setup}`({fa}) and `{payoff}`({fb}) are in \
                                 different frames and the edge report proposes a \
                                 succession anyway",
                            ));
                        }
                        continue;
                    }
                    same_frame += 1;
                    if chain.contains(&pair(setup, payoff)) {
                        already_chained += 1;
                    } else if !gaps.contains(&pair(setup, payoff)) {
                        disagreements.push(format!(
                            "{name}/{road}: substantiation discharges `{setup}` with \
                             `{payoff}` in frame `{fa}`, and the edge report neither \
                             chains them nor proposes the succession",
                        ));
                    }
                }
            }
        }
    }

    // Print BEFORE asserting (the R1026 lesson).
    println!(
        "{asked} authored stores asked, {answered} answered both reads\n\
         {discharges} discharging steps: {same_frame} same-frame \
         ({already_chained} already chained), {cross_frame} cross-frame — the class \
         `supersedes_in_frame` cannot express"
    );
    for row in &disagreements {
        println!("    DISAGREE {row}");
    }

    // THE EVIDENCE. `cross_frame` is pinned because it is the finding, not a
    // leftover: if it ever reads 0, either the corpus stopped exercising the
    // class or the discharge rule grew a frame test, and both are worth
    // looking at. `same_frame` is pinned for the same reason in reverse.
    assert_eq!(
        (asked, answered, discharges, same_frame, cross_frame),
        (44, 28, 26, 23, 3),
        "the discharging steps this tree can ask about, split by frame"
    );

    assert_eq!(
        disagreements,
        Vec::<String>::new(),
        "a discharge and the succession surface disagree about one typed step"
    );
}
