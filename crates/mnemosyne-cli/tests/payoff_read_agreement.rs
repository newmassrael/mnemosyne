//! Two shipped reads answer about the same payoffs. Do they agree? (Round 1041)
//!
//! Round 1037 found a shipped contradiction by comparing two reads that answer
//! one question, BY HAND. Rounds 1039 and 1040 replaced the hand-picking with a
//! derived population and left the backlog of 28 read pairs, and the carry they
//! left is the honest one: the population says WHO answers about a subject,
//! never WHETHER the answers agree.
//!
//! FOUR DERIVATIONS OF "DO THEY AGREE" HAVE NOW BEEN MEASURED AND REFUTED, and
//! the fourth was refuted by injection rather than by argument (Round 1041):
//!
//! - a VERDICT FIELD by shape, and by closure over 28 authored stores (R1039,
//!   R1040) — a field only one corpus exercises has a small value set whatever
//!   it is, and whether a field carries a closed vocabulary is a fact about its
//!   TYPE that output cannot recover;
//! - RESPONSIVENESS ASYMMETRY, "one read moves under an edit the other ignores"
//!   — vacuous: 489 of 581 shared cells are asymmetric, because two reads
//!   legitimately judge one subject along different axes;
//! - DISJOINT EVIDENCE, "no single authorable edit moves both records" — 65 of
//!   581 cells, selective enough to look like a finding list, and BLIND: with
//!   R1037's shared derivation reverted in the tree, the walk's numbers did not
//!   move at all while `quest_discharge_agreement_smoke` named the three roads.
//!   A contradiction is a statement about VALUES; sensitivity is not.
//!
//! So agreement between two reads has to be DECLARED once and then enforced
//! everywhere, which is what this file is for. The pair is not hand-picked: the
//! R1040 walk ranked `report-payoff-coverage <-> report-payoff-substantiation`
//! into its backlog with 12 subjects in common, and reading the two records
//! side by side is what turned up the contract below.
//!
//! THE CONTRACT. Both reads partition the same population — the (world, setup)
//! pairs whose setup is PAID on that road. `report-payoff-coverage` calls that
//! set `paid`; `report-payoff-substantiation` splits the same set into
//! `substantiated` / `unsubstantiated` / `unverifiable` by whether the payoff
//! can be shown to land after its setup. A consumer handed "this setup is paid
//! here" by one read and no row at all by the other is in exactly the position
//! R1037's runtime was.
//!
//! AND THE PAIR WAS ALREADY CARRYING ONE. Writing the check found that the two
//! reads emitted the same row — `{setup, payoffs}` — and meant different things
//! by it: coverage lists every payoff that CREDITS the setup, substantiation
//! lists, for a substantiated one, only those that DISCHARGE its typed state.
//! A consumer joining them by setup reads a narrowing as a contradiction, in a
//! store where nothing is wrong. No authored corpus shows it (all 27 credit
//! their substantiated setups with exactly the discharging payoffs), so the
//! TREE showed it — `a_substantiated_setup_lists_fewer_payoffs_than_credit_it`
//! in `continuity.rs` is that store. The narrowed row now has its own type and
//! its own field name, and this walk asserts the relation each bucket has to
//! the crediting list rather than assuming one.
//!
//! Asked of every store an author shipped that this tree can still ask (the
//! R1036 population rule) — the tracked corpora AND the migrated dnd-quest
//! record, which a sweep of tracked manifests alone excludes because its own
//! tracked manifest is the pre-migration file that no longer loads. The totals
//! are ASSERTED rather than printed: a store that stops answering silently
//! shrinks the evidence, which is the defect Round 1036 found by aiming an
//! injection at exactly that path.

use std::collections::{BTreeMap, BTreeSet};

mod common;
use common::{authored_stores, run};

/// The pair of shipped reads this contract judges, named ONCE and run from
/// here. The backlog walk (`surface/read_agreement_population.rs`) reads this
/// declaration out of the source, because it ranks 87 pairs by shared subjects
/// to say which to compare next and could not otherwise tell which of them
/// already have a contract.
const DECLARES: [&str; 2] = ["report-payoff-coverage", "report-payoff-substantiation"];

/// A read's `[{setup, payoffs}]` list as a map, with the duplicate-setup case
/// made loud: two rows for one setup would make "the same population" ambiguous
/// and silently drop one of them.
fn by_setup(rows: &serde_json::Value, whose: &str) -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for row in rows
        .as_array()
        .unwrap_or_else(|| panic!("{whose} is a list"))
    {
        let setup = row["setup"]
            .as_str()
            .unwrap_or_else(|| panic!("{whose} row has a setup"))
            .to_string();
        let payoffs: Vec<String> = row["payoffs"]
            .as_array()
            .unwrap_or_else(|| panic!("{whose} row has payoffs"))
            .iter()
            .map(|p| p.as_str().expect("a payoff id").to_string())
            .collect();
        assert!(
            out.insert(setup.clone(), payoffs).is_none(),
            "{whose} lists `{setup}` twice, so which row is the answer is undefined",
        );
    }
    out
}

#[test]
fn the_two_payoff_reads_partition_the_same_paid_setups() {
    let mut answered = 0usize;
    let mut worlds_compared = 0usize;
    let mut setups_compared = 0usize;
    let mut disagreements: Vec<String> = Vec::new();

    let (stores, unloadable) = authored_stores();
    let asked = stores.len() + unloadable.len();
    let mut silent: Vec<String> = unloadable
        .iter()
        .map(|name| format!("{name} (does not load)"))
        .collect();
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
        let (coverage, substantiation) = match (read(DECLARES[0]), read(DECLARES[1])) {
            (Ok(c), Ok(s)) => (c, s),
            (c, s) => {
                let refused: Vec<String> = [c, s].into_iter().filter_map(Result::err).collect();
                silent.push(format!("{name} ({} refused)", refused.join(" ")));
                continue;
            }
        };
        answered += 1;

        // The two reads must agree about how many setups the store even holds,
        // before any question about which of them are paid where.
        if coverage["setups_total"] != substantiation["setups_total"] {
            disagreements.push(format!(
                "{name}: setups_total {} vs {}",
                coverage["setups_total"], substantiation["setups_total"],
            ));
        }

        let empty = serde_json::Map::new();
        let cov_worlds = coverage["worlds"].as_object().unwrap_or(&empty);
        let sub_worlds = substantiation["worlds"].as_object().unwrap_or(&empty);
        let roads: BTreeSet<&String> = cov_worlds.keys().chain(sub_worlds.keys()).collect();
        for road in roads {
            // A road one read does not carry at all is a disagreement about the
            // population, not a pair to skip — the R1038 lesson: comparing only
            // where both answered lets a projection lose a road in silence.
            let (Some(cov), Some(sub)) = (cov_worlds.get(road), sub_worlds.get(road)) else {
                disagreements.push(format!(
                    "{name}/{road}: only {} carries this road",
                    if cov_worlds.contains_key(road) {
                        DECLARES[0]
                    } else {
                        DECLARES[1]
                    },
                ));
                continue;
            };
            worlds_compared += 1;

            let paid = by_setup(&cov["paid"], "coverage.paid");
            // Each bucket carries its own relation to the crediting list, and
            // that is the whole finding of this round: `substantiated` NARROWS
            // the payoffs to the ones that discharge the setup's typed state,
            // while the other two clone the crediting list. Asserting equality
            // everywhere would be asserting something the tree deliberately
            // does not do — the corpus cannot show it (no authored corpus
            // credits a substantiated setup with a payoff that fails to
            // discharge it), so the unit test in `continuity.rs` does.
            let mut judged: BTreeMap<String, (&str, Vec<String>)> = BTreeMap::new();
            for (bucket, field) in [
                ("substantiated", "discharging_payoffs"),
                ("unsubstantiated", "payoffs"),
                ("unverifiable", "payoffs"),
            ] {
                for row in sub[bucket].as_array().expect("a substantiation bucket") {
                    let setup = row["setup"].as_str().expect("a setup id").to_string();
                    let payoffs: Vec<String> = row[field]
                        .as_array()
                        .unwrap_or_else(|| panic!("{bucket} rows carry `{field}`"))
                        .iter()
                        .map(|p| p.as_str().expect("a payoff id").to_string())
                        .collect();
                    if let Some((first, _)) = judged.insert(setup.clone(), (bucket, payoffs)) {
                        disagreements.push(format!(
                            "{name}/{road}: `{setup}` is in two substantiation buckets \
                             ({first} and {bucket})",
                        ));
                    }
                }
            }
            setups_compared += paid.len();

            for setup in paid.keys().chain(judged.keys()).collect::<BTreeSet<_>>() {
                match (paid.get(setup), judged.get(setup)) {
                    (Some(credited), Some((bucket, listed))) => {
                        let ok = if *bucket == "substantiated" {
                            !listed.is_empty() && listed.iter().all(|p| credited.contains(p))
                        } else {
                            listed == credited
                        };
                        if !ok {
                            disagreements.push(format!(
                                "{name}/{road}: `{setup}` is credited by {credited:?} and \
                                 {bucket} lists {listed:?}",
                            ));
                        }
                    }
                    (Some(_), None) => disagreements.push(format!(
                        "{name}/{road}: coverage calls `{setup}` paid and \
                         substantiation does not judge it at all",
                    )),
                    (None, Some((bucket, _))) => disagreements.push(format!(
                        "{name}/{road}: substantiation calls `{setup}` {bucket} and \
                         coverage does not call it paid",
                    )),
                    (None, None) => unreachable!("the setup came from one of the two"),
                }
            }
        }
    }

    // Print BEFORE asserting: a first-violation stop reports one line of a
    // measurement over 28 stores (the R1026 lesson).
    println!(
        "{asked} authored corpora asked, {answered} answered both reads, \
         {} could not:",
        silent.len(),
    );
    for name in &silent {
        println!("    skip {name}");
    }
    println!("{worlds_compared} roads compared, {setups_compared} paid setups on them");
    for row in &disagreements {
        println!("    DISAGREE {row}");
    }

    // THE EVIDENCE, asserted rather than printed. A corpus that stops answering
    // shrinks this silently, and then the contract below holds over nothing —
    // the shape of defect Round 1036 found by injecting into exactly that path.
    assert_eq!(
        (asked, answered, worlds_compared, setups_compared),
        (44, 28, 41, 136),
        "the corpora that answer both payoff reads, and how much they compare"
    );

    // THE CONTRACT. Both reads derive their own population from the store and
    // neither reads the other, so this is the only thing standing between a
    // consumer and two shipped answers about one setup.
    assert_eq!(
        disagreements,
        Vec::<String>::new(),
        "two shipped reads disagree about which setups are paid where"
    );
}
