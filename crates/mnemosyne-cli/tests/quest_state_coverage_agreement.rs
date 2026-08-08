//! One read says the quest is finished on this road; the other says the setup
//! that quest opens is still an outstanding obligation there. (Round 1045.)
//!
//! The fourth declared cross-read agreement, on the next pair from the Round
//! 1040 backlog: `report-payoff-coverage <-> report-quest-graph` (10 subjects in
//! common). Rounds 1041-1043 established why agreement is DECLARED rather than
//! derived — five derivations over read output failed to decide it, the fifth
//! refuted by injection.
//!
//! THE SHARED QUESTION. Both reads answer "is this quest's giving obligation
//! outstanding on this road". Coverage answers about the SETUP: a giving fact is
//! `dangling` in a world when nothing paid it off there. The quest graph answers
//! about the QUEST: `open` means an obligation is outstanding here, `done` means
//! it is discharged.
//!
//! WRITING THE CHECK FOUND THE READ THROWING THE ANSWER AWAY. The quest graph's
//! state derivation intersects the quest's givings with this road's dangling
//! list and kept ONE BIT of the result. That is not a cosmetic loss, because
//! `giving_facts` is a STORE-WIDE union over every `completed_by` fact of the
//! quest while the state is per-road: a consumer holding both could not tell
//! WHICH giving is outstanding here without re-deriving the R442 coverage and
//! the R559 giving binding for itself, which is the recomputation the projection
//! seam exists to prevent. And the bit is dropped entirely under `done`, which
//! since R1037 is derived from a visible `completed_by` and consults coverage
//! not at all — so a quest completable two ways reads `done` on the road it took
//! while the other way's giving still dangles there, and the read said nothing.
//! `QuestWorldState::outstanding_givings` now carries it, so the arms below are
//! EQUALITIES rather than one-way implications.
//!
//! No authored corpus this tree can ask exercises that last class (every quest
//! in the one corpus that declares quests is completed exactly one way), so the
//! TREE shows it instead — `a_quest_is_done_where_its_other_giving_still_dangles`
//! in `continuity.rs` is that store. The count is pinned at 0 here for the same
//! reason R1043 pinned its cross-frame count: if it moves, either the corpus
//! grew the class or the derivation changed, and both are worth looking at.
//!
//! Asked of every store an author shipped that this tree can ask
//! (`authored_stores()`, the R1042 resolver) under EVERY telling it declares,
//! reading only what the two shipped reads emit — the store is opened for one
//! thing, the `--telling` argument the quest graph requires and the corpus
//! itself declares. WHY a store answers nothing is counted by name: a sweep that
//! silently asks 7 of 44 is the R1036 hole, and the reasons are the measurement.

use std::collections::{BTreeMap, BTreeSet};

use mnemosyne_atomic::AtomicStore;

mod common;
use common::{authored_stores, declared_tellings, run, SIDECAR};

/// The pair of shipped reads this contract judges, named ONCE and run from
/// here. The backlog walk (`surface/read_agreement_population.rs`) reads this
/// declaration out of the source, because it ranks 87 pairs by shared subjects
/// to say which to compare next and could not otherwise tell which of them
/// already have a contract.
const DECLARES: [&str; 2] = ["report-payoff-coverage", "report-quest-graph"];

/// The setups coverage reports as DANGLING in one world — the obligations it
/// says are outstanding there.
fn dangling_setups(world: &serde_json::Value) -> BTreeSet<&str> {
    world["dangling"]
        .as_array()
        .expect("the dangling list")
        .iter()
        .map(|d| d.as_str().expect("a dangling entry is a setup id"))
        .collect()
}

/// The ids in a read's string list field.
fn ids(value: &serde_json::Value, whose: &str) -> BTreeSet<String> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("{whose} is a list"))
        .iter()
        .map(|v| {
            v.as_str()
                .unwrap_or_else(|| panic!("{whose} holds ids"))
                .to_string()
        })
        .collect()
}

/// What the two reads said, tallied. Every arm runs and the failures come out as
/// a LIST — stopping at the first reports one line of a walk over 44 stores
/// (the R1026 lesson).
#[derive(Default)]
struct Census {
    answered: usize,
    cells: usize,
    open: usize,
    unknown: usize,
    done: usize,
    /// The class: `done` on a road that still owes one of the quest's givings.
    done_outstanding: usize,
    roads_only_in_coverage: usize,
    disagreements: Vec<String>,
}

impl Census {
    /// Hold one store's two answers to each other, under one telling.
    fn compare(&mut self, name: &str, coverage: &serde_json::Value, graph: &serde_json::Value) {
        let empty = serde_json::Map::new();
        let cover_worlds = coverage["worlds"].as_object().unwrap_or(&empty);
        let graph_worlds: BTreeSet<&str> = graph["worlds"]
            .as_array()
            .expect("the quest graph's world list")
            .iter()
            .map(|w| w.as_str().expect("a world id"))
            .collect();

        // THE ROAD SETS FIRST. Coverage classifies every query world and the
        // graph reports the worlds playable-world hands it, so coverage is the
        // superset — the asymmetry is counted. The other direction is a road the
        // graph judges quests on and coverage never classified, and nothing
        // could hold that join together (the R1038 lesson: compare only where
        // both answered and a lost road shrinks the comparison in silence).
        for world in &graph_worlds {
            if !cover_worlds.contains_key(*world) {
                self.disagreements.push(format!(
                    "{name}: the quest graph reports road `{world}` and payoff \
                     coverage does not classify it at all",
                ));
            }
        }
        self.roads_only_in_coverage += cover_worlds
            .keys()
            .filter(|world| !graph_worlds.contains(world.as_str()))
            .count();

        for quest in graph["quests"].as_array().expect("the quest nodes") {
            let quest_id = quest["quest_id"].as_str().expect("a quest id");
            let giving = ids(&quest["giving_facts"], "giving_facts");
            for (world, state) in quest["per_world"].as_object().unwrap_or(&empty) {
                let Some(cover) = cover_worlds.get(world) else {
                    continue; // reported above as a road coverage never classified
                };
                self.cells += 1;
                let outstanding = ids(&state["outstanding_givings"], "outstanding_givings");
                let dangling = dangling_setups(cover);
                let should: BTreeSet<String> = giving
                    .iter()
                    .filter(|g| dangling.contains(g.as_str()))
                    .cloned()
                    .collect();
                let completed = !state["completions"]
                    .as_array()
                    .expect("the completions list")
                    .is_empty();

                // ARM 1 — the outstanding list IS the intersection, both ways.
                if outstanding != should {
                    self.disagreements.push(format!(
                        "{name}/{world}: quest `{quest_id}` calls {outstanding:?} outstanding \
                         and coverage dangles {should:?} of its givings",
                    ));
                }

                // ARM 2 — each verdict's relation to that list, and to whether
                // the quest was discharged here at all.
                match state["state"].as_str().expect("a quest state") {
                    // OPEN is the dangling list read back: an obligation is
                    // outstanding here and nothing discharged the quest.
                    "open" => {
                        self.open += 1;
                        if outstanding.is_empty() || completed {
                            self.disagreements.push(format!(
                                "{name}/{world}: quest `{quest_id}` is open with \
                                 {outstanding:?} outstanding and completed={completed}",
                            ));
                        }
                    }
                    // UNKNOWN is its complement: nothing outstanding, nothing
                    // discharged — the quest is not on this road at all.
                    "unknown" => {
                        self.unknown += 1;
                        if !outstanding.is_empty() || completed {
                            self.disagreements.push(format!(
                                "{name}/{world}: quest `{quest_id}` is unknown with \
                                 {outstanding:?} outstanding and completed={completed}",
                            ));
                        }
                    }
                    // DONE means a `completed_by` fact is visible here, and says
                    // NOTHING about coverage — so the outstanding list is the
                    // only thing that can tell a consumer the road still owes a
                    // setup. Counted, never rejected: which read is right about
                    // a quest completed one of two ways is a question for the
                    // owner, not for a walk holding two outputs.
                    "done" => {
                        self.done += 1;
                        if !completed {
                            self.disagreements.push(format!(
                                "{name}/{world}: quest `{quest_id}` is done and names no \
                                 completion fact",
                            ));
                        }
                        if !outstanding.is_empty() {
                            self.done_outstanding += 1;
                        }
                    }
                    other => self.disagreements.push(format!(
                        "{name}/{world}: quest `{quest_id}` reads state `{other}`, which is \
                         not a verdict this walk knows how to hold coverage to",
                    )),
                }
            }
        }
    }
}

#[test]
fn a_quest_is_open_exactly_where_its_own_giving_still_dangles() {
    let mut census = Census::default();
    let (stores, unloadable) = authored_stores();
    let asked = stores.len() + unloadable.len();

    // WHY a store answered nothing, by name.
    let mut silent: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    let mut note = |why: &'static str, what: String| silent.entry(why).or_default().push(what);
    for name in &unloadable {
        note("the store does not load", name.clone());
    }
    for store in &stores {
        let name = &store.name;
        let ws = &store.ws;
        let read = |argv: &[&str]| {
            let out = run(ws.path(), argv);
            if out.status.success() {
                return Ok(serde_json::from_slice::<serde_json::Value>(&out.stdout)
                    .unwrap_or_else(|e| panic!("{argv:?} on {name} is not json: {e}")));
            }
            Err(String::from_utf8_lossy(&out.stderr)
                .lines()
                .next()
                .unwrap_or("(no stderr)")
                .to_string())
        };
        // EVERY telling the corpus declares, not one of them: the quest graph is
        // a per-telling read, so the contract has to hold under each, and a
        // sole-telling rule would drop a corpus that declares two for no reason
        // beyond the walk finding it awkward to choose (the R1042 lesson —
        // widen the population with a resolver, then check it bears load).
        let tellings = AtomicStore::load(&ws.path().join(SIDECAR))
            .map(|store| declared_tellings(&store))
            .unwrap_or_default();
        if tellings.is_empty() {
            note(
                "declares no telling, so the quest graph cannot be asked",
                name.clone(),
            );
            continue;
        }
        let coverage = match read(&[DECLARES[0], "--json"]) {
            Ok(c) => c,
            Err(why) => {
                note("payoff coverage refuses", format!("{name}: {why}"));
                continue;
            }
        };
        for telling in &tellings {
            let graph = match read(&[DECLARES[1], "--telling", telling, "--json"]) {
                Ok(g) => g,
                Err(why) => {
                    note(
                        "the quest graph refuses",
                        format!("{name} [{telling}]: {why}"),
                    );
                    continue;
                }
            };
            census.answered += 1;
            if graph["quests"].as_array().is_some_and(Vec::is_empty) {
                note(
                    "answered both, declares no quest",
                    format!("{name} [{telling}]"),
                );
            }
            census.compare(&format!("{name} [{telling}]"), &coverage, &graph);
        }
    }

    // Print BEFORE asserting (the R1026 lesson).
    let Census {
        answered,
        cells,
        open,
        unknown,
        done,
        done_outstanding,
        roads_only_in_coverage,
        disagreements,
    } = &census;
    println!(
        "{asked} authored stores asked, {answered} (store, telling) pairs answered both reads\n\
         {cells} quest-road cells: {open} open, {unknown} unknown, {done} done \
         ({done_outstanding} done on a road that still owes one of the quest's givings)\n\
         {roads_only_in_coverage} roads coverage classifies that the quest graph does not report"
    );
    for (why, names) in &silent {
        println!("  {:3} {why}", names.len());
        for row in names {
            println!("        {row}");
        }
    }
    for row in disagreements {
        println!("    DISAGREE {row}");
    }

    let mut broken: Vec<String> = Vec::new();
    let mut check = |ok: bool, claim: &str| {
        if !ok {
            broken.push(claim.to_string());
        }
    };

    // THE EVIDENCE, asserted rather than printed. A corpus that stops answering
    // shrinks a contract's reach in silence — the defect R1036 found by aiming
    // an injection at exactly that path.
    // THE TWO UNITS ARE NEVER ADDED. A store is asked once; a (store, telling)
    // pair is what answers, and a corpus declaring two tellings contributes two.
    let count = |why: &str| silent.get(why).map_or(0, Vec::len);
    check(
        (
            asked,
            count("the store does not load"),
            count("declares no telling, so the quest graph cannot be asked"),
        ) == (44, 16, 18),
        "POPULATION (stores): of the corpora asked, these never reach the \
         comparison at all — 16 to the R857 rot, 18 declaring no telling for a \
         per-telling read",
    );
    check(
        (*answered, count("answered both, declares no quest"), *cells) == (13, 12, 16),
        "EVIDENCE (store-telling pairs): the pairs that answered both reads, \
         those declaring no quest, and the quest-road cells the rest put in \
         front of each other — ONE pair carries every cell below, so this \
         contract is as narrow as the corpus that declares quests",
    );
    check(
        (*open, *unknown, *done) == (5, 1, 10),
        "CENSUS: every verdict class is exercised, so no arm below holds \
         vacuously",
    );
    check(
        *done_outstanding == 0,
        "CLASS: no authored corpus completes a quest one of two ways, so the \
         one place `done` and coverage can diverge is shown by the tree \
         (`a_quest_is_done_where_its_other_giving_still_dangles`) and not here",
    );
    check(
        *roads_only_in_coverage == 0,
        "ROADS: the two reads cover the same roads on every corpus asked, so \
         the superset arm is currently a guard rather than a live difference",
    );
    check(
        disagreements.is_empty(),
        "CONTRACT: the quest graph and payoff coverage agree about which \
         obligation is outstanding on which road",
    );

    assert_eq!(
        broken,
        Vec::<String>::new(),
        "the quest-state / payoff-coverage correspondence no longer holds"
    );
}

/// The PROSE wire must say what the type carries — R1037's defect, which was a
/// shipped line reading "unresolved (no completed_by anchor)" over quests that
/// had one.
///
/// Written because the injection that removes the clause from the human line
/// left the whole workspace GREEN. Everything above reads `--json`, so the read
/// a person actually looks at could go back to printing "done" over a road that
/// still owes a setup and nothing would say so. A substring check over the whole
/// report would not do it either: the same ids are printed on the quest's
/// `giving:` line, so the clause can vanish while every id is still on screen.
/// The check is per WORLD LINE.
#[test]
fn the_prose_line_says_what_the_road_still_owes() {
    let mut cells = 0usize;
    let mut owing = 0usize;
    let mut wrong: Vec<String> = Vec::new();

    let (stores, _) = authored_stores();
    for store in &stores {
        let ws = &store.ws;
        let name = &store.name;
        let Ok(atomic) = AtomicStore::load(&ws.path().join(SIDECAR)) else {
            continue;
        };
        for telling in declared_tellings(&atomic) {
            let argv = [DECLARES[1], "--telling", telling.as_str()];
            let prose = run(ws.path(), &argv);
            let mut json_argv = argv.to_vec();
            json_argv.push("--json");
            let json = run(ws.path(), &json_argv);
            if !prose.status.success() || !json.status.success() {
                continue;
            }
            let report: serde_json::Value =
                serde_json::from_slice(&json.stdout).expect("the quest graph is json");
            let prose = String::from_utf8(prose.stdout).expect("the report is utf-8");

            // The prose, indexed the way it is printed: a `quest `<id>`:` line
            // opens a block, and the world lines under it are indented and are
            // not the `giver[...]` locators.
            let mut seen: BTreeMap<(String, String), String> = BTreeMap::new();
            let mut quest = String::new();
            for line in prose.lines() {
                if let Some(rest) = line.strip_prefix("quest `") {
                    quest = rest.split('`').next().unwrap_or_default().to_string();
                } else if let Some(rest) = line.strip_prefix("    ") {
                    if rest.starts_with("giver[") {
                        continue;
                    }
                    if let Some((world, said)) = rest.split_once(": ") {
                        seen.insert((quest.clone(), world.to_string()), said.to_string());
                    }
                }
            }

            for q in report["quests"].as_array().expect("the quest nodes") {
                let quest_id = q["quest_id"].as_str().expect("a quest id");
                let empty = serde_json::Map::new();
                for (world, state) in q["per_world"].as_object().unwrap_or(&empty) {
                    cells += 1;
                    let outstanding = ids(&state["outstanding_givings"], "outstanding_givings");
                    let key = (quest_id.to_string(), world.clone());
                    let Some(said) = seen.get(&key) else {
                        wrong.push(format!(
                            "{name} [{telling}]: the prose prints no line for quest \
                             `{quest_id}` on road `{world}`",
                        ));
                        continue;
                    };
                    if outstanding.is_empty() {
                        continue;
                    }
                    owing += 1;
                    let missing: Vec<&String> =
                        outstanding.iter().filter(|g| !said.contains(*g)).collect();
                    if !missing.is_empty() {
                        wrong.push(format!(
                            "{name} [{telling}]: quest `{quest_id}` owes {missing:?} on road \
                             `{world}` and its line says `{said}`",
                        ));
                    }
                }
            }
        }
    }

    println!("{cells} quest-road lines read, {owing} of them owing a giving");
    for row in &wrong {
        println!("    SILENT {row}");
    }

    // Non-vacuity, asserted: with no owing road the walk would pass over a wire
    // that prints nothing at all.
    assert_eq!(
        (cells, owing),
        (16, 5),
        "the quest-road lines this corpus prints, and how many carry an \
         outstanding giving for the prose to name"
    );
    assert_eq!(
        wrong,
        Vec::<String>::new(),
        "the human-facing quest line says less than the report holds"
    );
}
