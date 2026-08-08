//! The playable world's per-road manuscript IS the playthrough manuscript for
//! that road — under the SAME telling. (Round 1048.)
//!
//! The fifth declared cross-read agreement, and the first one taken from the
//! TOP of the Round 1040 backlog rather than from a hand-picked shortlist:
//! `report-playable-world <-> report-playthrough-manuscript`, 72 subjects in
//! common, the largest overlap on the shipped surface. Rounds 1041-1045
//! established why agreement is DECLARED rather than derived — five derivations
//! over read output failed to decide it, the fifth refuted by injection.
//!
//! THE SHARED QUESTION. Both reads linearize a world-line into scenes. The
//! playable world does not re-derive that walk; it CALLS the manuscript
//! projection and embeds the result verbatim (the R558 discipline). So this
//! contract is not a hope that two derivations agree — it is a pin on the ONE
//! derivation staying one, and it fails the day somebody re-derives.
//!
//! THE ARGUMENT IS PART OF THE QUESTION, AND THAT IS WHAT WRITING THIS FOUND.
//! The first sweep asked the playable world with the corpus's telling and the
//! manuscript without one, because the manuscript read makes it optional and
//! answers happily bare. Every road cell differed — and the difference was the
//! `disclosure` column on every begins-event, present on one side and absent on
//! the other. The reads had not disagreed; the WALK had asked them different
//! questions, and neither answer said which question it was. One store, one
//! verb, three different manuscripts (bare, and one per declared telling) with
//! nothing in the answer to tell them apart. That is the R1042 shape at the
//! argument surface, and `read_argument_provenance.rs` is the population it
//! belongs to — four such arguments on the whole shipped read surface, all four
//! on the reads a runtime consumes.
//!
//! So the contract below has TWO halves: the manuscripts are equal, and both
//! reports say what they were asked. The second is not decoration — it is the
//! only thing that makes the first checkable by anyone but this file.
//!
//! Asked of every store an author shipped that this tree can ask
//! (`authored_stores()`, the R1042 resolver) under EVERY telling it declares
//! (the R1045 rule — a sole-telling shortcut drops a corpus that declares two
//! for no reason but the walk finding it awkward to choose). WHY a store answers
//! nothing is counted by name: a sweep that silently asks 7 of 44 is the R1036
//! hole, and the reasons are the measurement.

use std::collections::{BTreeMap, BTreeSet};

use mnemosyne_atomic::AtomicStore;

mod common;
use common::{authored_stores, declared_tellings, run, SIDECAR};

/// The pair of shipped reads this contract judges, named ONCE and run from
/// here. The backlog walk (`surface/read_agreement_population.rs`) reads this
/// declaration out of the source, because it ranks 87 pairs by shared subjects
/// to say which to compare next and could not otherwise tell which of them
/// already have a contract.
const DECLARES: [&str; 2] = ["report-playable-world", "report-playthrough-manuscript"];

#[test]
fn the_playable_world_embeds_the_manuscript_it_was_asked_for() {
    let (stores, unloadable) = authored_stores();
    let asked = stores.len() + unloadable.len();

    let mut silent: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();
    let mut note = |why: &'static str, what: String| silent.entry(why).or_default().push(what);
    for name in &unloadable {
        note("the store does not load", name.clone());
    }

    let mut answered = 0usize;
    let mut roads = 0usize;
    let mut filtered_roads = 0usize;
    let mut scenes = 0usize;
    let mut disclosed_events = 0usize;
    let mut disagreements: Vec<String> = Vec::new();

    for store in &stores {
        let ws = store.ws.path();
        let name = &store.name;
        let read = |argv: &[&str]| -> Result<serde_json::Value, String> {
            let out = run(ws, argv);
            if out.status.success() {
                return Ok(serde_json::from_slice(&out.stdout)
                    .unwrap_or_else(|e| panic!("{argv:?} on {name} is not json: {e}")));
            }
            Err(String::from_utf8_lossy(&out.stderr)
                .lines()
                .next()
                .unwrap_or("(no stderr)")
                .to_string())
        };
        let tellings = AtomicStore::load(&ws.join(SIDECAR))
            .map(|store| declared_tellings(&store))
            .unwrap_or_default();
        if tellings.is_empty() {
            note(
                "declares no telling, so the playable world cannot be asked",
                name.clone(),
            );
            continue;
        }
        for telling in &tellings {
            let manuscript = match read(&[DECLARES[1], "--telling", telling, "--json"]) {
                Ok(m) => m,
                Err(why) => {
                    note(
                        "the manuscript refuses",
                        format!("{name} [{telling}]: {why}"),
                    );
                    continue;
                }
            };
            let playable = match read(&[DECLARES[0], "--telling", telling, "--json"]) {
                Ok(p) => p,
                Err(why) => {
                    note(
                        "the playable world refuses",
                        format!("{name} [{telling}]: {why}"),
                    );
                    continue;
                }
            };
            answered += 1;

            // HALF ONE — both reports say what they were asked. Checked before
            // the manuscripts are compared, because a comparison between two
            // answers to unstated questions is what the first sweep of this pair
            // did, and it read as eighteen disagreements that were not there.
            for (whose, report) in [
                ("the manuscript", &manuscript),
                ("the playable world", &playable),
            ] {
                if report["telling"].as_str() != Some(telling.as_str()) {
                    disagreements.push(format!(
                        "{name} [{telling}]: {whose} was asked for telling `{telling}` and its \
                         report says `{}`",
                        report["telling"]
                    ));
                }
                if !report["world"].is_null() {
                    disagreements.push(format!(
                        "{name} [{telling}]: {whose} was given no road filter and its report \
                         says `{}`",
                        report["world"]
                    ));
                }
            }

            // HALF TWO — the road sets, then every road's manuscript, whole.
            let empty = serde_json::Map::new();
            let mine: BTreeSet<&String> = manuscript["worlds"]
                .as_object()
                .unwrap_or(&empty)
                .keys()
                .collect();
            let theirs: BTreeSet<&String> = playable["worlds"]
                .as_object()
                .unwrap_or(&empty)
                .keys()
                .collect();
            if mine != theirs {
                disagreements.push(format!(
                    "{name} [{telling}]: the manuscript walks {mine:?} and the playable world \
                     walks {theirs:?}",
                ));
            }
            for road in mine.intersection(&theirs) {
                roads += 1;

                // HALF THREE — the same pair, asked for ONE road. Until Round
                // 1049 this contract only ever asked both reads unfiltered, so
                // the `world` half of HALF ONE fired only its `null` arm and
                // three of Round 1048's four world injections left this file
                // green while the provenance gate went red. A filtered read is
                // the one a runtime actually makes.
                let filtered =
                    |verb: &str| read(&[verb, "--telling", telling, "--world", road, "--json"]);
                match (filtered(DECLARES[1]), filtered(DECLARES[0])) {
                    (Ok(manuscript), Ok(playable)) => {
                        filtered_roads += 1;
                        for (whose, report) in [
                            ("the manuscript", &manuscript),
                            ("the playable world", &playable),
                        ] {
                            if report["world"].as_str() != Some(road.as_str()) {
                                disagreements.push(format!(
                                    "{name} [{telling}]/{road}: {whose} was asked for that road \
                                     and its report says `{}`",
                                    report["world"]
                                ));
                            }
                            if report["telling"].as_str() != Some(telling.as_str()) {
                                disagreements.push(format!(
                                    "{name} [{telling}]/{road}: {whose} under a road filter says \
                                     telling `{}`",
                                    report["telling"]
                                ));
                            }
                        }
                        if manuscript["worlds"][road.as_str()]
                            != playable["worlds"][road.as_str()]["manuscript"]
                        {
                            disagreements.push(format!(
                                "{name} [{telling}]/{road}: under a road filter the playable \
                                 world's embedded manuscript is not the manuscript read's answer"
                            ));
                        }
                    }
                    (manuscript, playable) => {
                        for (whose, answer) in [
                            ("the manuscript", manuscript),
                            ("the playable world", playable),
                        ] {
                            if let Err(why) = answer {
                                note(
                                    "a read refuses the road filter",
                                    format!("{name} [{telling}]/{road}: {whose}: {why}"),
                                );
                            }
                        }
                    }
                }

                let mine = &manuscript["worlds"][road.as_str()];
                let theirs = &playable["worlds"][road.as_str()]["manuscript"];
                if mine != theirs {
                    // Name the FIELDS that moved, not just "they differ": a
                    // whole-object mismatch printed raw is two megabytes of
                    // scene text and says nothing about where to look.
                    let fields: Vec<&String> = mine
                        .as_object()
                        .unwrap_or(&empty)
                        .keys()
                        .chain(theirs.as_object().unwrap_or(&empty).keys())
                        .filter(|key| mine[key.as_str()] != theirs[key.as_str()])
                        .collect();
                    disagreements.push(format!(
                        "{name} [{telling}]/{road}: the playable world's embedded manuscript \
                         differs from the manuscript read in {fields:?}",
                    ));
                }
                for scene in mine["scenes"].as_array().unwrap_or(&Vec::new()) {
                    scenes += 1;
                    for event in scene["begins"].as_array().unwrap_or(&Vec::new()) {
                        // NON-VACUITY, counted rather than trusted: the column
                        // whose absence made the first sweep read as eighteen
                        // disagreements. If it ever reads 0 the equality above
                        // is comparing two manuscripts with nothing in them that
                        // the telling decides, and this contract has stopped
                        // being about the telling at all.
                        if !event["disclosure"].is_null() {
                            disclosed_events += 1;
                        }
                    }
                }
            }
        }
    }

    // Print BEFORE asserting (the R1026 lesson).
    println!(
        "{asked} authored stores asked, {answered} (store, telling) pairs answered both reads\n\
         {roads} roads compared whole ({filtered_roads} of them asked again under `--world`), \
         {scenes} scenes, {disclosed_events} begins-events carrying a disclosure the telling \
         decided"
    );
    for (why, names) in &silent {
        println!("  {:3} {why}", names.len());
        for row in names {
            println!("        {row}");
        }
    }
    for row in &disagreements {
        println!("    DISAGREE {row}");
    }

    let mut broken: Vec<String> = Vec::new();
    let mut check = |ok: bool, claim: &str| {
        if !ok {
            broken.push(claim.to_string());
        }
    };

    let count = |why: &str| silent.get(why).map_or(0, Vec::len);
    check(
        (
            asked,
            count("the store does not load"),
            count("declares no telling, so the playable world cannot be asked"),
        ) == (44, 16, 18),
        "POPULATION (stores): of the corpora asked, these never reach the \
         comparison at all — 16 to the R857 rot, 18 declaring no telling for a \
         pair of per-telling reads",
    );
    check(
        (answered, roads, scenes, filtered_roads) == (13, 18, 400, 18),
        "EVIDENCE: the (store, telling) pairs that answered both reads, how \
         much of each store the two put in front of each other, and how many of \
         those roads were asked a SECOND time with the filter a runtime uses",
    );
    check(
        disclosed_events == 1483,
        "NON-VACUITY: the begins-events whose disclosure column the telling \
         decides — the column whose absence on one side made the first sweep of \
         this pair read as eighteen disagreements that were not there",
    );
    check(
        disagreements.is_empty(),
        "CONTRACT: the playable world embeds the manuscript read's answer for \
         each road, under the telling both reports name",
    );

    assert_eq!(
        broken,
        Vec::<String>::new(),
        "the manuscript/playable-world correspondence no longer holds"
    );
}
