//! The frontier counts the author's work per SCENE; the manuscript walks the
//! same scenes per ROAD — and both answer BEFORE any telling exists. (Round
//! 1088.)
//!
//! The eighth declared cross-read agreement, taken from the top of the Round
//! 1087 backlog: `report-authoring-frontier <-> report-playthrough-manuscript`,
//! 158 subjects in common, the highest-ranked pair no contract judged. Rounds
//! 1041-1045 established why agreement is DECLARED rather than derived (five
//! derivations over read output failed to decide it, the fifth refuted by
//! injection), and Round 1050 established what makes a declaration worth
//! writing: the oracle must be ANOTHER SHIPPED READ.
//!
//! WHY THIS PAIR IS NOT THE TWO CONTRACTS BESIDE IT COMPOSED, which is the
//! first thing to answer here and the reason the file is shaped the way it is.
//! Round 1048 pins that the playable world EMBEDS this manuscript verbatim, and
//! Round 1052 pins the frontier against that embedded copy — so under a telling,
//! this pair's scene laws follow from those two and restating them would be two
//! copies of one thought (the R1041 shape). Both of those contracts stand on
//! `report-playable-world`, and that read REFUSES without `--telling`. So:
//!
//! - the whole composition is unavailable to a store that declares no telling,
//!   and 18 of the 44 authored corpora declare none — they reach neither
//!   existing contract, and the two reads here are the pair that can still be
//!   asked about them;
//! - `--reading-walk` is an argument only the manuscript has. The playable world
//!   cannot be asked it at all, so nothing about the reading copy is reachable
//!   through the composition from any store, telling or not.
//!
//! Both halves of that are MEASURED rather than argued — the refusal in
//! [`the_playable_world_cannot_be_asked_what_this_pair_answers`], the population
//! in the counts below — because "the other contract cannot reach this" is
//! exactly the kind of claim that rots silently when the other contract grows.
//!
//! THE SHARED QUESTION. Both reads decompose one composed canon order and hand
//! back different halves of it:
//!
//! - the frontier is keyed by SCENE (`scene_coverage`, a row per registered
//!   section, plus the placement sets naming the ones no order reaches) and by
//!   ROAD only in aggregate (`branch_owned_density`, a length, a count and the
//!   road itself);
//! - the manuscript is keyed by ROAD (a scene walk per world-line) and names the
//!   sections each road does not travel.
//!
//! So no field here equals a field there, and every law below is a JOIN.
//!
//! THE LAWS, each with its own evidence:
//!
//! - ROADS — the frontier reports density for every registered road; the
//!   manuscript's unfiltered dump walks all but the confluences, and each road
//!   the dump omits comes back from a `--world` ask marked
//!   `confluence_fragment`. The exception is proved by the other read's OWN
//!   marker rather than by this file consulting the store. Round 1052 judged the
//!   same difference against the playable world's fork tree; this is the other
//!   read's other way of saying it, and a re-derivation drifts between them.
//! - ROAD — the frontier's `road` IS that road's walk, scene for scene. Two call
//!   sites of one linearizer.
//! - PARTITION — the scenes a road travels and the sections it calls off-road
//!   partition the frontier's scene census, disjointly and exhaustively.
//! - UNPLACED — a section the frontier says no order positions is off EVERY
//!   road. The frontier owns the one placement resolver (R667).
//! - UNORDERED — a fact anchored at a section the order does not position is
//!   never BEGUN by any road, and some road names it unplaced AT that
//!   coordinate. This is the frontier's renderability signal (R596:
//!   `unordered_scenes` is the fact-bearing half of the placement set) judged
//!   against the consumer it is a signal about — that the manuscript cannot
//!   place these facts is the frontier's claim, and this is the manuscript
//!   saying so itself instead of going quiet. The roads that call the fact
//!   `undecidable` rather than naming its coordinate are COUNTED: a road that
//!   does not carry the fact has nothing to place.
//! - CENSUS — the facts the frontier holds at a scene and the facts the
//!   manuscript names there are the same SET: each begins there on some road, or
//!   some road calls that coordinate unplaced. The leftovers are the facts no
//!   road can decide (`undecidable` everywhere), named rather than absorbed.
//! - EMPTY — a scene the frontier calls zero-fact begins nothing on any road.
//!
//! And the law no other pair can state, in its own test below:
//!
//! - READING COPY — a scene the frontier calls zero-fact is DROPPED from the
//!   reading walk of every road that travels it, and the prune moves nothing
//!   else in the answer. The converse is COUNTED, not asserted: a scene the
//!   reading walk drops may be one the census holds facts at, because those
//!   facts belong to another road, and one such scene is expected work rather
//!   than a defect. (`k-join` in the constructed store is exactly that: the left
//!   traveller sees something there and the right traveller does not.)
//!
//! TWO POPULATIONS, ONE IMPLEMENTATION. The authored corpora
//! (`authored_stores()`, the R1042 resolver) asked BARE — no telling, which is
//! the ask this pair exists for — and they leave four arms at ZERO: no corpus
//! holds a section outside its order, a scene with nothing in it, a confluence,
//! or a fact no order can place. Four laws asserted over an empty arm are four
//! claims with no evidence, so the tree constructs the store the authors did not
//! — through the same import recipe, so it is a store an author could have
//! shipped — and the same `judge` runs over both. The two populations are
//! asserted SEPARATELY: that the authored corpora exercise none of those four is
//! itself a measurement, and averaging it into a total would hide it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use mnemosyne_atomic::AtomicStore;

use crate::common;
use common::{authored_stores, constructed_corpus, declared_tellings, run, SIDECAR};

/// The pair of shipped reads this contract judges, named ONCE and run from
/// here. The backlog walk (`surface/read_agreement_population.rs`) reads this
/// declaration out of the source, because it ranks 87 pairs by shared subjects
/// to say which to compare next and could not otherwise tell which of them
/// already have a contract.
const DECLARES: [&str; 2] = ["report-authoring-frontier", "report-playthrough-manuscript"];

/// The read both contracts beside this one stand on, and the one this pair
/// exists because a store need not be able to answer.
const NEEDS_A_TELLING: &str = "report-playable-world";

/// The strings in a `[..]` list, empty when the key is absent.
fn strings(list: &serde_json::Value) -> Vec<String> {
    list.as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// Ask one read of one store and hand back its refusal rather than panicking:
/// "this store cannot be asked" is a verdict about the store, and a walk that
/// dies on it reports nothing about the 27 others.
fn read(ws: &Path, name: &str, argv: &[&str]) -> Result<serde_json::Value, String> {
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
}

/// What one population put in front of the laws, and what came back. Every
/// field is asserted: an arm that quietly reads zero is a law nothing tested,
/// which is the failure this file's second population exists to prevent.
#[derive(Default)]
struct Evidence {
    answered: usize,
    roads: usize,
    filtered_roads: usize,
    confluence_marks: usize,
    scenes_walked: usize,
    section_rows: usize,
    unplaced_checks: usize,
    empty_scene_checks: usize,
    facts_named: usize,
    undecidable_everywhere: usize,
    unordered_facts: usize,
    unordered_rows: usize,
    unordered_undecidable: usize,
    /// One line per store: the sections registered and every road with its
    /// length. The totals above are sums, and a sum is where a road that
    /// stopped answering hides (R1036); this is the distribution they came from.
    shape: Vec<String>,
    silent: BTreeMap<&'static str, Vec<String>>,
    disagreements: Vec<String>,
}

impl Evidence {
    fn note(&mut self, why: &'static str, what: String) {
        self.silent.entry(why).or_default().push(what);
    }

    fn count(&self, why: &str) -> usize {
        self.silent.get(why).map_or(0, Vec::len)
    }

    fn report(&self, whose: &str) {
        println!(
            "\n{whose}: {} stores answered both reads with no telling named\n  \
             {} roads compared ({} asked with `--world` because the unfiltered dump omits \
             confluences, {} of them marked a confluence fragment), {} scenes walked, \
             {} section-rows partitioned, {} unplaced-section checks, {} zero-fact scene checks\n  \
             {} facts named at a scene the frontier counts, {} undecidable on every road\n  \
             {} facts at a section no order positions ({} named unplaced at their own \
             coordinate by some road, {} road-sightings that call them undecidable instead)",
            self.answered,
            self.roads,
            self.filtered_roads,
            self.confluence_marks,
            self.scenes_walked,
            self.section_rows,
            self.unplaced_checks,
            self.empty_scene_checks,
            self.facts_named,
            self.undecidable_everywhere,
            self.unordered_facts,
            self.unordered_rows,
            self.unordered_undecidable,
        );
        for row in &self.shape {
            println!("    {row}");
        }
        for (why, names) in &self.silent {
            println!("  {:3} {why}", names.len());
            for row in names {
                println!("        {row}");
            }
        }
        for row in &self.disagreements {
            println!("    DISAGREE {row}");
        }
    }
}

/// Ask both reads of one store with NO telling named and apply every law. The
/// whole contract lives here so the constructed store cannot be judged by a
/// second, gentler copy of it.
fn judge(ws: &Path, name: &str, ev: &mut Evidence) {
    let frontier = match read(ws, name, &[DECLARES[0], "--json"]) {
        Ok(f) => f,
        Err(why) => {
            ev.note("the frontier refuses", format!("{name}: {why}"));
            return;
        }
    };
    let manuscript = match read(ws, name, &[DECLARES[1], "--json"]) {
        Ok(m) => m,
        Err(why) => {
            ev.note("the manuscript refuses", format!("{name}: {why}"));
            return;
        }
    };
    ev.answered += 1;

    // PROVENANCE — the manuscript says which question it answered, checked
    // before anything is compared. Comparing two answers to unstated questions
    // is what the first sweep of the R1048 pair did, and it read as eighteen
    // disagreements that were not there. Here the whole point is that NO telling
    // was named, so `null` is the answer this walk requires; the frontier omits
    // its telling field entirely when it has none, and that absence is its way
    // of saying the same thing (the telling-scoped sections are not there).
    if !manuscript["telling"].is_null() {
        ev.disagreements.push(format!(
            "{name}: the manuscript was named no telling and its report says `{}`",
            manuscript["telling"]
        ));
    }
    if !manuscript["world"].is_null() {
        ev.disagreements.push(format!(
            "{name}: the manuscript was given no road filter and its report says `{}`",
            manuscript["world"]
        ));
    }
    if manuscript["reading_walk"] != serde_json::json!(false) {
        ev.disagreements.push(format!(
            "{name}: the manuscript was not asked for the reading walk and its report says `{}`",
            manuscript["reading_walk"]
        ));
    }
    if !frontier["telling"].is_null() {
        ev.disagreements.push(format!(
            "{name}: the frontier was named no telling and its report says `{}`",
            frontier["telling"]
        ));
    }

    let empty_map = serde_json::Map::new();
    let empty_list: Vec<serde_json::Value> = Vec::new();

    // The frontier's two keyings: per scene, and per road.
    let coverage: BTreeMap<&str, BTreeSet<String>> = frontier["scene_coverage"]
        .as_array()
        .unwrap_or(&empty_list)
        .iter()
        .filter_map(|row| {
            row["scene"]
                .as_str()
                .map(|scene| (scene, strings(&row["facts"]).into_iter().collect()))
        })
        .collect();
    let known_roads: BTreeMap<&str, Vec<&str>> = frontier["branch_owned_density"]
        .as_object()
        .unwrap_or(&empty_map)
        .iter()
        .map(|(road, row)| {
            (
                road.as_str(),
                row["road"]
                    .as_array()
                    .map(|scenes| {
                        scenes
                            .iter()
                            .filter_map(serde_json::Value::as_str)
                            .collect()
                    })
                    .unwrap_or_default(),
            )
        })
        .collect();

    // LAW ROADS. The unfiltered dump is a set of PLAYTHROUGHS, and a merge node
    // is not one (R533) — so the two road sets differ by construction, and the
    // contract is that they differ by exactly the roads the manuscript itself
    // calls confluence fragments when asked for them one at a time.
    let walked: BTreeSet<&str> = manuscript["worlds"]
        .as_object()
        .unwrap_or(&empty_map)
        .keys()
        .map(String::as_str)
        .collect();
    let frontier_roads: BTreeSet<&str> = known_roads.keys().copied().collect();
    for road in walked.difference(&frontier_roads) {
        ev.disagreements.push(format!(
            "{name}: the manuscript walks `{road}` and the frontier reports no density for it",
        ));
    }

    // Every road the frontier names is asked. The ones the dump does not carry
    // are asked with `--world`, and each must say it is a fragment: without that
    // the difference between the two road sets would be this file's assumption
    // about merges rather than the other read's statement.
    let mut walks: BTreeMap<&str, serde_json::Value> = BTreeMap::new();
    for road in &frontier_roads {
        if let Some(world) = manuscript["worlds"].get(road) {
            walks.insert(road, world.clone());
            continue;
        }
        match read(ws, name, &[DECLARES[1], "--world", road, "--json"]) {
            Ok(filtered) => {
                ev.filtered_roads += 1;
                if filtered["world"].as_str() != Some(road) {
                    ev.disagreements.push(format!(
                        "{name}/{road}: the manuscript was asked for that road and its report \
                         says `{}`",
                        filtered["world"]
                    ));
                }
                match filtered["worlds"].get(road) {
                    Some(world) => {
                        if world["confluence_fragment"] == serde_json::json!(true) {
                            ev.confluence_marks += 1;
                        } else {
                            ev.disagreements.push(format!(
                                "{name}/{road}: the frontier names this road and the unfiltered \
                                 manuscript omits it, and asked for it directly the manuscript \
                                 does not call it a confluence fragment",
                            ));
                        }
                        walks.insert(road, world.clone());
                    }
                    None => ev.disagreements.push(format!(
                        "{name}/{road}: the manuscript answered the road filter and its map does \
                         not carry that road",
                    )),
                }
            }
            Err(why) => ev.note(
                "a road the frontier names that the manuscript refuses",
                format!("{name}/{road}: {why}"),
            ),
        }
    }

    // `named_at` accumulates, ACROSS ROADS, the facts the manuscript attributes
    // to each scene — beginning there, or naming it as an unplaced coordinate.
    // It is the census's left-hand side.
    let mut named_at: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    let mut decided_somewhere: BTreeSet<String> = BTreeSet::new();
    let mut undecidable_somewhere: BTreeSet<String> = BTreeSet::new();
    // Per (road, fact): the fact was named unplaced at its own coordinate here.
    let mut unplaced_here: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    let mut begun: BTreeSet<String> = BTreeSet::new();
    let frontier_unplaced: BTreeSet<String> =
        strings(&frontier["unplaced_scenes"]).into_iter().collect();
    let zero_fact: BTreeSet<String> = strings(&frontier["zero_fact_scenes"]).into_iter().collect();
    let mut per_road: BTreeMap<&str, usize> = BTreeMap::new();
    for (road, world) in &walks {
        ev.roads += 1;
        let walk: Vec<&str> = world["scenes"]
            .as_array()
            .unwrap_or(&empty_list)
            .iter()
            .filter_map(|scene| scene["section"].as_str())
            .collect();

        // LAW ROAD — the two SEQUENCES, not their lengths: two roads of one
        // length through different scenes are the drift this pair is for.
        let empty_road: Vec<&str> = Vec::new();
        let declared = known_roads.get(road).unwrap_or(&empty_road);
        if declared != &walk {
            ev.disagreements.push(format!(
                "{name}/{road}: the frontier says the road is {declared:?} and the manuscript \
                 walks {walk:?}",
            ));
        }
        ev.scenes_walked += walk.len();
        per_road.insert(road, walk.len());

        // LAW PARTITION — travelled + off-road = the registry, with neither an
        // overlap nor a section unaccounted for.
        let travelled: BTreeSet<&str> = walk.iter().copied().collect();
        let off_road: BTreeSet<String> = strings(&world["sections_off_road"]).into_iter().collect();
        let registry: BTreeSet<&str> = coverage.keys().copied().collect();
        let claimed: BTreeSet<&str> = travelled
            .iter()
            .copied()
            .chain(off_road.iter().map(String::as_str))
            .collect();
        for section in travelled.iter().filter(|s| off_road.contains(**s)) {
            ev.disagreements.push(format!(
                "{name}/{road}: `{section}` is both walked and called off-road",
            ));
        }
        if claimed != registry {
            let missed: Vec<&&str> = registry.difference(&claimed).collect();
            let invented: Vec<&&str> = claimed.difference(&registry).collect();
            ev.disagreements.push(format!(
                "{name}/{road}: the frontier's scene census and this road's walk+off-road do not \
                 partition each other (uncovered {missed:?}, unknown to the frontier {invented:?})",
            ));
        }
        ev.section_rows += registry.len();

        // LAW UNPLACED — a section no order positions is travelled by no road.
        for section in &frontier_unplaced {
            ev.unplaced_checks += 1;
            if travelled.contains(section.as_str()) {
                ev.disagreements.push(format!(
                    "{name}/{road}: the frontier says no order positions `{section}` and this \
                     road walks it",
                ));
            }
        }

        // LAW EMPTY + the census's per-scene left-hand side.
        for scene in world["scenes"].as_array().unwrap_or(&empty_list) {
            let Some(section) = scene["section"].as_str() else {
                continue;
            };
            let begins: Vec<String> = scene["begins"]
                .as_array()
                .unwrap_or(&empty_list)
                .iter()
                .filter_map(|event| event["fact_id"].as_str().map(ToString::to_string))
                .collect();
            if zero_fact.contains(section) {
                ev.empty_scene_checks += 1;
                if !begins.is_empty() {
                    ev.disagreements.push(format!(
                        "{name}/{road}: the frontier calls `{section}` a zero-fact scene and the \
                         manuscript begins {begins:?} there",
                    ));
                }
            }
            let Some((section, _)) = coverage.get_key_value(section) else {
                continue;
            };
            for fact in begins {
                begun.insert(fact.clone());
                decided_somewhere.insert(fact.clone());
                named_at.entry(section).or_default().insert(fact);
            }
        }
        for unplaced in world["unplaced_facts"].as_array().unwrap_or(&empty_list) {
            let (Some(fact), Some(field), Some(coordinate)) = (
                unplaced["fact_id"].as_str(),
                unplaced["field"].as_str(),
                unplaced["coordinate"].as_str(),
            ) else {
                continue;
            };
            decided_somewhere.insert(fact.to_string());
            // Only `canon_from` attributes the FACT to the scene; the other two
            // fields point at a coordinate belonging to some other fact's
            // placement (`canon_to`, a successor's seat).
            if field != "canon_from" {
                continue;
            }
            unplaced_here
                .entry(road)
                .or_default()
                .insert(fact.to_string());
            if let Some((section, _)) = coverage.get_key_value(coordinate) {
                named_at
                    .entry(section)
                    .or_default()
                    .insert(fact.to_string());
            }
        }
        for fact in strings(&world["undecidable"]) {
            undecidable_somewhere.insert(fact);
        }
    }

    // LAW UNORDERED. `unordered_scenes` is the frontier's renderability signal —
    // the fact-bearing half of the placement set, the facts it says no consumer
    // can place. The manuscript is that consumer, so the claim is judged from
    // its side: no road BEGINS such a fact, and some road names it unplaced at
    // its own coordinate rather than going quiet about it.
    for section in strings(&frontier["unordered_scenes"]) {
        let held = coverage.get(section.as_str()).cloned().unwrap_or_default();
        for fact in &held {
            ev.unordered_facts += 1;
            if begun.contains(fact) {
                ev.disagreements.push(format!(
                    "{name}: the frontier says no order positions `{section}` and some road \
                     begins `{fact}` there anyway",
                ));
            }
            let naming: Vec<&&str> = unplaced_here
                .iter()
                .filter(|(_, facts)| facts.contains(fact))
                .map(|(road, _)| road)
                .collect();
            if naming.is_empty() {
                ev.disagreements.push(format!(
                    "{name}: the frontier says `{fact}` sits at `{section}`, which no order \
                     positions, and no road names it unplaced at that coordinate",
                ));
            } else {
                ev.unordered_rows += 1;
            }
            // A road that does not carry the fact has nothing to place, and says
            // so with `undecidable`. Counted rather than required: which roads
            // see a fact is the other read's business.
            if undecidable_somewhere.contains(fact) {
                ev.unordered_undecidable += 1;
            }
        }
    }

    // LAW CENSUS. Scene by scene, the facts the frontier holds and the facts the
    // manuscript names there are the SAME SET, but for the ones no road could
    // decide — and those are named too, not absorbed into a total.
    let undecidable_only: BTreeSet<String> = undecidable_somewhere
        .difference(&decided_somewhere)
        .cloned()
        .collect();
    ev.undecidable_everywhere += undecidable_only.len();
    let mut named_total = 0usize;
    let mut undecided_at: BTreeSet<String> = BTreeSet::new();
    for (section, held) in &coverage {
        let named = named_at.get(section).cloned().unwrap_or_default();
        named_total += named.len();
        let invented: Vec<&String> = named.difference(held).collect();
        if !invented.is_empty() {
            ev.disagreements.push(format!(
                "{name}: the manuscript names {invented:?} at `{section}` and the frontier's \
                 census does not hold them there",
            ));
        }
        undecided_at.extend(held.difference(&named).cloned());
    }
    ev.facts_named += named_total;
    if undecided_at != undecidable_only {
        let unexplained: Vec<&String> = undecided_at.difference(&undecidable_only).collect();
        let missing: Vec<&String> = undecidable_only.difference(&undecided_at).collect();
        ev.disagreements.push(format!(
            "{name}: the frontier holds {unexplained:?} at scenes no road names them at while \
             some road did decide them, and calls {missing:?} undecidable everywhere while its \
             own census does not leave them over",
        ));
    }

    ev.shape.push(format!(
        "{name}: {} sections | {}",
        coverage.len(),
        per_road
            .iter()
            .map(|(road, scenes)| format!("{road}={scenes}sc"))
            .collect::<Vec<_>>()
            .join(" "),
    ));
}

/// The sections, order and facts of the store the authors did not write: a
/// confluence, a scene with nothing in it that every road travels, a scene one
/// road sees content at and another does not, a section outside the order
/// carrying a fact, and a section outside the order with nothing in it.
///
/// It declares NO telling, deliberately — the ask this pair exists for is the
/// bare one, and a constructed store that declared one would be a store the
/// composition beside this file could have judged.
fn constructed_manifests() -> (serde_json::Value, serde_json::Value, serde_json::Value) {
    let section = |id: &str, title: &str| {
        serde_json::json!({
            "section_id": id,
            "parent_doc": "constructed",
            "title": title,
        })
    };
    let sections = serde_json::json!([
        section("k-01", "the road opens"),
        section("k-02", "an empty stretch"),
        section("k-03", "the road divides"),
        section("k-left", "the left road"),
        section("k-right", "the right road"),
        section("k-join", "where both roads arrive"),
        section("k-after", "the shared continuation"),
        section("k-loose", "a scene no order positions"),
        section("k-bare", "a scene no order positions and nothing in it"),
    ]);
    let order = serde_json::json!({
        "edges": [["k-01", "k-02"], ["k-02", "k-03"]],
        "branches": {
            "left": [["k-03", "k-left"], ["k-left", "k-join"]],
            "right": [["k-03", "k-right"], ["k-right", "k-join"]],
            "merge": [["k-join", "k-after"]],
        },
    });
    let fact = |id: &str, branch: Option<&str>, at: &str, claim: &str| {
        let mut row = serde_json::json!({
            "fact_id": id,
            "frame": "f-narrator",
            "claim": claim,
            "canon_from": at,
            "evidence": [at],
        });
        if let Some(branch) = branch {
            row["branch"] = serde_json::json!(branch);
        }
        row
    };
    let facts = serde_json::json!({
        "frames": [{"frame_id": "f-narrator"}],
        "branches": [
            {"branch_id": "left", "description": "the left road",
             "forks_from": "main", "forks_at": "k-03"},
            {"branch_id": "right", "description": "the right road",
             "forks_from": "main", "forks_at": "k-03"},
            {"branch_id": "merge", "description": "where both roads arrive",
             "converges_from": [{"branch": "left", "at": "k-left"},
                                {"branch": "right", "at": "k-right"}]},
        ],
        "facts": [
            fact("fx-open", None, "k-01", "the gate stands open"),
            fact("fx-mid", None, "k-03", "the two roads are known to part here"),
            fact("fx-left", Some("left"), "k-left", "the left road is taken"),
            fact("fx-right", Some("right"), "k-right", "the right road is taken"),
            // The scene one traveller sees content at and the other does not —
            // the reading copy's counted converse, authored rather than argued.
            fact("fx-join-left", Some("left"), "k-join",
                 "the left traveller alone notices the door"),
            fact("fx-merge", Some("merge"), "k-after", "both roads end at the same door"),
            fact("fx-loose", None, "k-loose", "a fact at a coordinate no order positions"),
        ],
    });
    (sections, order, facts)
}

#[test]
fn the_frontier_counts_per_scene_what_the_manuscript_walks_per_road() {
    // POPULATION ONE — every store an author shipped that this tree can ask,
    // asked with no telling named. No corpus is dropped for declaring none,
    // which is the whole point: 18 of them do.
    let (stores, unloadable) = authored_stores();
    let asked = stores.len() + unloadable.len();
    let mut authored = Evidence::default();
    for corpus in &unloadable {
        authored.note("the store does not load", corpus.named_reason());
    }
    // The number this whole file rests on, MEASURED here rather than cited from
    // the two contracts that skip these stores: a corpus that declares no
    // telling cannot be asked `report-playable-world` at all, so neither Round
    // 1048's contract nor Round 1052's has anything to say about it.
    let mut declare_no_telling = 0usize;
    for store in &stores {
        let ws = store.ws.path();
        let tellings = AtomicStore::load(&ws.join(SIDECAR))
            .map(|store| declared_tellings(&store))
            .unwrap_or_default();
        if tellings.is_empty() {
            declare_no_telling += 1;
        }
        judge(ws, &store.name, &mut authored);
    }

    // POPULATION TWO — the store no author wrote, through the same recipe.
    let (sections, order, facts) = constructed_manifests();
    let built = constructed_corpus(&sections, &order, &facts)
        .unwrap_or_else(|e| panic!("the constructed manifests must import: {e}"));
    let mut constructed = Evidence::default();
    judge(built.path(), "the constructed store", &mut constructed);

    // Print BEFORE asserting (the R1026 lesson).
    println!(
        "{asked} authored stores asked, {declare_no_telling} of the loadable ones declare no \
         telling and so can be asked neither of the two contracts that relate these reads"
    );
    authored.report("AUTHORED");
    constructed.report("CONSTRUCTED");

    let mut broken: Vec<String> = Vec::new();
    let mut check = |ok: bool, claim: &str| {
        if !ok {
            broken.push(claim.to_string());
        }
    };

    check(
        (
            asked,
            authored.count("the store does not load"),
            authored.answered,
            declare_no_telling,
        ) == (46, 3, 43, 32),
        "POPULATION (stores): of the corpora asked, only the 3 whose author's \
         submission the write path rejected never arrive — every other store \
         answers, INCLUDING the 32 that declare no telling and therefore reach \
         neither of the two contracts that relate these reads through \
         `report-playable-world`. That last number is why this pair needs a \
         contract of its own, so it is MEASURED here and not quoted: 43 stores \
         against the 14 (store, telling) pairs those two contracts see. Round \
         1176 carried in the two scale-floor stores, and NEITHER names a telling \
         — the blind re-extraction harness wrote facts and roads, so both landed \
         in the arm this pair is the only contract for",
    );
    check(
        (
            authored.roads,
            authored.filtered_roads,
            authored.confluence_marks,
            authored.scenes_walked,
            authored.section_rows,
        ) == (106, 1, 1, 2535, 4177),
        "AUTHORED EVIDENCE: the roads compared, the roads the unfiltered dump \
         omitted, the confluence fragments among them, the scenes walked, and \
         the section-rows the partition law compared. Round 1174 lit the corpus \
         that declares a CONFLUENCE, so the two middle numbers left zero for the \
         first time and an author's store now carries the fragment arm the \
         constructed store used to carry alone. Round 1176 raised the first, \
         fourth and fifth, and the arithmetic is the attribution: the two \
         scale-floor stores walk five roads each (+10), 170 and 175 scenes \
         (+345), and their partitions are 5x60 and 5x65 rows (+625)",
    );
    check(
        (
            authored.unplaced_checks,
            authored.empty_scene_checks,
            authored.unordered_facts,
            authored.undecidable_everywhere,
        ) == (0, 91, 0, 0),
        "AUTHORED SILENCE, ASSERTED: no corpus holds a section outside its \
         order or a fact no order can place, so those laws are still carried \
         entirely by the constructed store — but fourteen EMPTY SCENES arrived \
         with Round 1174's lit corpora, and that arm is no longer the \
         constructed store's alone. Round 1176 turned fourteen into 91, and the \
         two scale-floor stores are the whole of the difference: 12 of store-A's \
         60 scenes and 17 of store-B's 65 carry no fact at all, each counted once \
         per road that walks it. Those stores ARE a blind re-extraction, so a \
         scene nothing was extracted from is the R473 experiment's own subject — \
         the arm with the most evidence behind it in this walk is one no corpus \
         could reach two rounds ago. This line is what makes the difference \
         visible rather than averaged away",
    );
    check(
        authored.facts_named == 3694,
        "AUTHORED CENSUS: the facts the manuscript names at a scene the frontier \
         counts. With the residual at zero above, the per-scene containment and \
         this total ARE per-scene equality",
    );
    check(
        (
            constructed.answered,
            constructed.roads,
            constructed.filtered_roads,
            constructed.confluence_marks,
            constructed.scenes_walked,
            constructed.section_rows,
        ) == (1, 4, 1, 1, 22, 36),
        "CONSTRUCTED EVIDENCE: one store, four roads — a 3-scene trunk, two \
         forks that each walk 6 because they carry on through the merge, and the \
         confluence's own 7-scene fragment, which only a `--world` ask reaches \
         and which says it is a fragment when asked",
    );
    check(
        (
            constructed.unplaced_checks,
            constructed.empty_scene_checks,
            constructed.facts_named,
            constructed.undecidable_everywhere,
        ) == (8, 4, 7, 0),
        "CONSTRUCTED PLACEMENT: the two unplaced sections checked on all four \
         roads, the zero-fact scene checked on every road that walks it, and the \
         census naming all seven facts with nothing left undecidable everywhere \
         — so the per-scene containment and the equal total ARE equality",
    );
    check(
        (
            constructed.unordered_facts,
            constructed.unordered_rows,
            constructed.unordered_undecidable,
        ) == (1, 1, 1),
        "CONSTRUCTED UNORDERED: the one fact sitting at a coordinate no order \
         positions is begun by no road and IS named unplaced at that coordinate \
         by a road that carries it — and it is also called undecidable by roads \
         that do not, which is counted rather than required. Without this arm \
         the frontier's renderability signal is judged by nobody",
    );
    check(
        authored.disagreements.is_empty() && constructed.disagreements.is_empty(),
        "CONTRACT: the frontier's per-scene census, per-road walks and placement \
         sets are the manuscript's roads — asked of a store that names no telling",
    );

    assert_eq!(
        broken,
        Vec::<String>::new(),
        "the frontier/manuscript correspondence no longer holds"
    );
}

/// The manuscript's report with everything the reading prune is ALLOWED to
/// change taken out: the flag that records which walk ran, and each road's scene
/// list. What is left must be equal on both sides, or the prune moved something
/// it does not own.
fn without_the_walk(report: &serde_json::Value) -> serde_json::Value {
    let mut rest = report.clone();
    let Some(object) = rest.as_object_mut() else {
        return rest;
    };
    object.remove("reading_walk");
    if let Some(worlds) = object.get_mut("worlds").and_then(|w| w.as_object_mut()) {
        for world in worlds.values_mut() {
            if let Some(world) = world.as_object_mut() {
                world.remove("scenes");
            }
        }
    }
    rest
}

/// A scene the frontier calls empty is a scene the reader is never walked
/// through. (Round 1088.)
///
/// `--reading-walk` is the one argument on this pair that the composition
/// through `report-playable-world` cannot reach: that read has no such flag, so
/// the reading copy is judged against nothing at all until here. The prune's own
/// mechanics are pinned one crate down (`mnemosyne-ops`, R1048) — this is the
/// JOIN, and it is the frontier that supplies the oracle: `zero_fact_scenes` is
/// the author's list of scenes with nothing in them, and a reader must not be
/// walked through one.
///
/// THE CONVERSE IS COUNTED, NOT ASSERTED. A scene the prune drops may be one the
/// census holds facts at, because those facts belong to another road — the store
/// below authors exactly that (`k-join`: the left traveller notices the door and
/// the right traveller does not), so the residual is a measured number and not a
/// worry.
#[test]
fn the_reading_copy_drops_the_scenes_the_frontier_calls_empty() {
    let mut authored_dropped = 0usize;
    let mut authored_stores_asked = 0usize;
    let mut disagreements: Vec<String> = Vec::new();

    // POPULATION ONE, and it is expected to be SILENT: every scene of every
    // authored corpus begins at least one fact. That is a fact about what those
    // authors wrote, and asserting it is what keeps "the law held" from meaning
    // "nothing was asked".
    let (stores, _unloadable) = authored_stores();
    for store in &stores {
        let ws = store.ws.path();
        let (Ok(structural), Ok(reading)) = (
            read(ws, &store.name, &[DECLARES[1], "--json"]),
            read(ws, &store.name, &[DECLARES[1], "--reading-walk", "--json"]),
        ) else {
            continue;
        };
        authored_stores_asked += 1;
        let empty = serde_json::Map::new();
        for (road, world) in structural["worlds"].as_object().unwrap_or(&empty) {
            let before = world["scenes"].as_array().map_or(0, Vec::len);
            let after = reading["worlds"][road]["scenes"]
                .as_array()
                .map_or(0, Vec::len);
            authored_dropped += before.saturating_sub(after);
        }
        if without_the_walk(&structural) != without_the_walk(&reading) {
            disagreements.push(format!(
                "{}: the reading prune moved something other than the scene lists",
                store.name
            ));
        }
    }

    // POPULATION TWO — the store the authors did not write.
    let (sections, order, facts) = constructed_manifests();
    let built = constructed_corpus(&sections, &order, &facts)
        .unwrap_or_else(|e| panic!("the constructed manifests must import: {e}"));
    let ws = built.path();
    let name = "the constructed store";
    let ask = |argv: &[&str]| {
        read(ws, name, argv).unwrap_or_else(|why| panic!("{argv:?} on {name}: {why}"))
    };
    let frontier = ask(&[DECLARES[0], "--json"]);
    let structural = ask(&[DECLARES[1], "--json"]);
    let reading = ask(&[DECLARES[1], "--reading-walk", "--json"]);

    let zero_fact: BTreeSet<String> = strings(&frontier["zero_fact_scenes"]).into_iter().collect();
    let held: BTreeMap<String, BTreeSet<String>> = frontier["scene_coverage"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .filter_map(|row| {
                    row["scene"].as_str().map(|scene| {
                        (
                            scene.to_string(),
                            strings(&row["facts"]).into_iter().collect(),
                        )
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let scenes_of = |report: &serde_json::Value, road: &str| -> Vec<String> {
        report["worlds"][road]["scenes"]
            .as_array()
            .map(|scenes| {
                scenes
                    .iter()
                    .filter_map(|scene| scene["section"].as_str().map(ToString::to_string))
                    .collect()
            })
            .unwrap_or_default()
    };

    let empty = serde_json::Map::new();
    let roads: Vec<String> = structural["worlds"]
        .as_object()
        .unwrap_or(&empty)
        .keys()
        .cloned()
        .collect();
    // Per road: the scenes the prune dropped, split into the ones the frontier
    // calls empty (the law) and the ones it holds facts at (the counted
    // converse).
    let mut dropped_empty: BTreeSet<(String, String)> = BTreeSet::new();
    let mut dropped_with_facts: BTreeSet<(String, String)> = BTreeSet::new();
    let mut empty_walked = 0usize;
    for road in &roads {
        let before = scenes_of(&structural, road);
        let after: BTreeSet<String> = scenes_of(&reading, road).into_iter().collect();
        for scene in &before {
            if zero_fact.contains(scene) {
                empty_walked += 1;
                if after.contains(scene) {
                    disagreements.push(format!(
                        "{name}/{road}: the frontier calls `{scene}` a zero-fact scene and the \
                         reading copy still walks a reader through it",
                    ));
                }
            }
            if after.contains(scene) {
                continue;
            }
            match held.get(scene).is_some_and(|facts| !facts.is_empty()) {
                true => dropped_with_facts.insert((road.clone(), scene.clone())),
                false => dropped_empty.insert((road.clone(), scene.clone())),
            };
        }
        // The prune only ever removes: a reading copy that ORDERS its scenes
        // differently, or invents one, is not a prune.
        let kept: Vec<String> = before
            .iter()
            .filter(|s| after.contains(*s))
            .cloned()
            .collect();
        if kept != scenes_of(&reading, road) {
            disagreements.push(format!(
                "{name}/{road}: the reading copy is not the structural walk with scenes removed \
                 — it walks {:?} where the kept subsequence is {kept:?}",
                scenes_of(&reading, road),
            ));
        }
    }

    println!(
        "{authored_stores_asked} authored stores asked both walks, {authored_dropped} scenes \
         dropped between them\nthe constructed store: {} roads, {empty_walked} zero-fact \
         scene-sightings on a road, {} scene-drops the frontier calls empty, {} it holds facts \
         at\n  dropped-empty {dropped_empty:?}\n  dropped-with-facts {dropped_with_facts:?}",
        roads.len(),
        dropped_empty.len(),
        dropped_with_facts.len(),
    );
    for row in &disagreements {
        println!("    DISAGREE {row}");
    }

    let mut broken: Vec<String> = Vec::new();
    let mut check = |ok: bool, claim: &str| {
        if !ok {
            broken.push(claim.to_string());
        }
    };

    check(
        (authored_stores_asked, authored_dropped) == (43, 92),
        "AUTHORED EVIDENCE: the day a corpus does hold an empty scene this \
         number moves and the law below starts being carried by an author's \
         store as well — Round 1174 was that day, at fifteen drops, where this \
         arm read 0 for as long as a third of the corpora were dark. Round 1176 \
         made it 92 by carrying in two blind re-extractions, where a scene an \
         extraction pass took nothing from is a scene with no fact: 29 of their \
         125 scenes are that, and every road that walks one drops it",
    );
    check(
        (empty_walked, dropped_empty.len()) == (3, 3),
        "THE LAW: the constructed store's one empty scene sits on three of the \
         four roads, and every one of those sightings is a scene the reading copy \
         drops. Non-vacuity is the first number: without it `dropped_empty` being \
         all of `empty_walked` says nothing",
    );
    check(
        dropped_with_facts
            == [("right".to_string(), "k-join".to_string())]
                .into_iter()
                .collect::<BTreeSet<_>>(),
        "THE COUNTED CONVERSE: `k-join` is dropped from the right road although \
         the frontier's census holds a fact there — the fact belongs to the left \
         road, and the traveller who never sees it is not walked through the \
         scene. This is why the law runs one way only, and naming the pair is \
         what keeps it from being a residual nobody can read",
    );
    check(
        (
            structural["reading_walk"] == serde_json::json!(false),
            reading["reading_walk"] == serde_json::json!(true),
        ) == (true, true),
        "PROVENANCE: two answers about one store differ, so each has to say which \
         walk produced it. Without the flag a consumer holding both cannot tell \
         the pruned one from a store that has no empty scenes",
    );
    check(
        without_the_walk(&structural) == without_the_walk(&reading),
        "THE PRUNE OWNS THE SCENE LISTS AND NOTHING ELSE: with the flag and the \
         scene lists set aside the two answers are equal, so the reading copy is \
         not a second derivation of the walk that happens to agree about the rest",
    );
    check(
        disagreements.is_empty(),
        "CONTRACT: a scene the frontier calls empty is one no reading copy walks \
         a reader through, and the reading copy is the structural walk with \
         scenes removed",
    );

    assert_eq!(
        broken,
        Vec::<String>::new(),
        "the frontier/reading-copy correspondence no longer holds"
    );
}

/// The reason this pair needs a contract of its own, stated as a measurement.
/// (Round 1088.)
///
/// Rounds 1048 and 1052 relate these two reads through `report-playable-world`,
/// and both of those contracts are unavailable to a store that declares no
/// telling — not because they chose to skip it but because the read they stand
/// on cannot be asked at all. 18 of the 44 authored corpora are in that
/// position. If that ever stops being true this test goes red, and the file
/// above becomes the composition it currently is not.
#[test]
fn the_playable_world_cannot_be_asked_what_this_pair_answers() {
    let (stores, _unloadable) = authored_stores();
    let store = stores
        .first()
        .expect("the corpus sweep finds at least one loadable store");
    let ws = store.ws.path();

    let refusal = read(ws, &store.name, &[NEEDS_A_TELLING, "--json"]);
    let frontier = read(ws, &store.name, &[DECLARES[0], "--json"]);
    let manuscript = read(ws, &store.name, &[DECLARES[1], "--json"]);
    println!(
        "on {}: `{NEEDS_A_TELLING}` bare -> {refusal:?}\n  `{}` bare -> {}\n  `{}` bare -> {}",
        store.name,
        DECLARES[0],
        frontier
            .as_ref()
            .map_or_else(|e| e.clone(), |_| "an answer".to_string()),
        DECLARES[1],
        manuscript
            .as_ref()
            .map_or_else(|e| e.clone(), |_| "an answer".to_string()),
    );

    let mut broken: Vec<String> = Vec::new();
    let mut check = |ok: bool, claim: &str| {
        if !ok {
            broken.push(claim.to_string());
        }
    };

    check(
        refusal
            .as_ref()
            .err()
            .is_some_and(|why| why.contains("--telling")),
        "THE READ BOTH NEIGHBOURING CONTRACTS STAND ON REFUSES: asked without a \
         telling `report-playable-world` names the missing argument and exits \
         non-zero, so neither Round 1048's contract nor Round 1052's can say \
         anything about a store that declares none",
    );
    check(
        frontier.is_ok() && manuscript.is_ok(),
        "AND THIS PAIR ANSWERS: both reads of this contract hand back a report \
         with no telling named, which is the question the composition cannot be \
         asked",
    );

    assert_eq!(
        broken,
        Vec::<String>::new(),
        "the reason this contract exists no longer holds"
    );
}
