//! The frontier counts the author's work per SCENE; the playable world walks
//! the same scenes per ROAD. (Round 1052.)
//!
//! The sixth declared cross-read agreement, taken from the top of the Round
//! 1051 backlog: `report-authoring-frontier <-> report-playable-world`, 57
//! subjects in common when it was declared, 59 once Round 1053 made the scene
//! census name its facts and 80 once Round 1054 made the density numerator name
//! its own — the largest overlap no contract judged at the time. Rounds
//! 1041-1045 established why agreement is DECLARED rather than derived (five
//! derivations over read output failed to decide it, the fifth refuted by
//! injection), and Round 1050 established what makes a declaration worth
//! writing: the oracle must be ANOTHER SHIPPED READ, not this file re-deriving
//! the answer, or the pair becomes two copies of one thought (the R1041 shape).
//!
//! THE SHARED QUESTION. Both reads decompose one store along the SAME two axes
//! and hand back different halves of it:
//!
//! - the frontier is keyed by SCENE (`scene_coverage`, a row per registered
//!   section, plus the placement sets naming the ones no order reaches) and by
//!   ROAD only in aggregate (`branch_owned_density`, a length and a count);
//! - the playable world is keyed by ROAD (a manuscript per world-line, its
//!   scenes in walk order) and names the sections each road does not travel.
//!
//! So no field here equals a field there. Every law below is a JOIN, which is
//! what makes this pair different from the four before it, where one read
//! embedded or reused the other's projection and the contract pinned that reuse.
//!
//! THE LAWS, each with its own evidence:
//!
//! - ROADS — the frontier knows every road the playable world walks, and the
//!   roads it knows that the dump does not walk are EXACTLY the confluences the
//!   playable world's own fork tree names. The exception is proved by the other
//!   read, not by this file consulting the store.
//! - ROAD — the frontier's `road` IS that road's walk, scene for scene. Two
//!   call sites of one linearizer; a re-derivation drifts here first. It was a
//!   LENGTH law until Round 1061 named the frontier's denominator, and a length
//!   cannot tell two roads of one length through different scenes apart.
//! - PARTITION — the scenes a road travels and the sections it calls off-road
//!   partition the frontier's scene census, disjointly and exhaustively.
//! - UNPLACED — a section the frontier says no order positions is off EVERY
//!   road. The frontier owns the one placement resolver (R667); this is the
//!   only statement tying it to what a reader is walked through.
//! - EMPTY — a scene the frontier calls zero-fact begins nothing on any road.
//! - CENSUS — the facts the frontier holds at a scene and the facts the playable
//!   world names there are the same SET: each begins there on some road, or some
//!   road calls that coordinate unplaced. The leftovers are the facts no road can
//!   decide (`undecidable` everywhere), named rather than absorbed. This was an
//!   equality of NUMBERS until Round 1053 made the census name what it counts.
//! - STRUCTURAL — a fact the playable world hands the runtime with a quest typed
//!   leg is one the frontier's census has already subtracted as plumbing, at the
//!   scene it begins. The converse is counted, not asserted: a quest GIVING setup
//!   is structural because a completion pays it off, and carries no typed leg for
//!   the other read to show. Round 1052 left this field touching no law; Round
//!   1053 emptied it and watched the workspace stay green, which is why it now
//!   does.
//! - DISCLOSURE — a fact the frontier calls never-planned carries NO locator,
//!   and a disclosure it calls seated-before-truth is a locator sitting earlier
//!   in the walk than the scene its fact begins at. The R643 arm ("a withheld
//!   fact is not on the map") and the R949 arm ("a seat before the truth")
//!   judged from the other side for the first time.
//!
//! TWO POPULATIONS, ONE IMPLEMENTATION. The authored corpora
//! (`authored_stores()`, the R1042 resolver) under every telling they declare
//! (the R1045 rule) — and they leave four arms at ZERO: no corpus holds a
//! section outside its order, a scene with nothing in it, a seat before its
//! truth, or a confluence that also declares a telling (the one corpus with a
//! merge declares no telling, so this pair's per-telling half cannot ask it).
//! Four laws asserted over an empty arm are four claims with no evidence, so
//! the tree constructs the store the authors did not — through the same import
//! recipe, so it is a store an author could have shipped — and the same
//! `judge` runs over both. The two populations are asserted SEPARATELY: that
//! the authored corpora exercise none of those four is itself a measurement,
//! and averaging it into a total would hide it.
//!
//! THE CONFLUENCE ROAD IS ASKED WITH `--world` (the R1049 rule: a filtered read
//! is the one a runtime makes) — and what that ask buys was MEASURED rather
//! than assumed, because the first version of this sentence said the census
//! could not close without it and the shape line refuted that in one run. Both
//! forks walk THROUGH the merge into its continuation (`left=7sc`, `right=7sc`
//! over a 4-scene trunk), so the fact the confluence owns is named on the forks
//! anyway. What the ask does buy is the confluence's own road: its LENGTH and
//! its PARTITION are judged nowhere else, and its manuscript is the one that
//! carries a single locator over eight scenes because a fragment leaves the
//! pre-merge trunk undecidable.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use mnemosyne_atomic::AtomicStore;

use mnemosyne_validate::continuity::{
    QUEST_PRED_COMPLETED_BY, QUEST_PRED_PURSUES, QUEST_PRED_REQUIRES,
};

use crate::common;
use common::{authored_stores, constructed_corpus, declared_tellings, run, SIDECAR};

/// The pair of shipped reads this contract judges, named ONCE and run from
/// here. The backlog walk (`surface/read_agreement_population.rs`) reads this
/// declaration out of the source, because it ranks 87 pairs by shared subjects
/// to say which to compare next and could not otherwise tell which of them
/// already have a contract.
const DECLARES: [&str; 2] = ["report-authoring-frontier", "report-playable-world"];

/// The ids in a `[{fact_id: ...}, ...]` list.
fn fact_ids(list: &serde_json::Value) -> Vec<String> {
    list.as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item["fact_id"].as_str().map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default()
}

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

/// What one population put in front of the laws, and what came back. Every
/// field is asserted: an arm that quietly reads zero is a law nothing tested,
/// which is the failure this file's second population exists to prevent.
#[derive(Default)]
struct Evidence {
    answered: usize,
    roads: usize,
    filtered_roads: usize,
    scenes_walked: usize,
    section_rows: usize,
    unplaced_checks: usize,
    empty_scene_checks: usize,
    facts_named: usize,
    undecidable_everywhere: usize,
    quest_typed_events: usize,
    structural_untyped: usize,
    locators: usize,
    overridden_seats: usize,
    never_planned_checks: usize,
    seated_early: usize,
    early_on_unnamed_road: usize,
    withheld_early_seats: usize,
    /// One line per (store, telling): the roads and their lengths. The totals
    /// above are sums, and a sum is where a road that stopped answering hides
    /// (R1036); this is the distribution they came from.
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
            "\n{whose}: {} (store, telling) pairs answered both reads\n  \
             {} roads compared ({} asked with `--world` because the playable dump excludes \
             confluences), {} scenes walked, {} section-rows partitioned, {} unplaced-section \
             checks, {} zero-fact scene checks\n  \
             {} facts named at a scene the frontier counts, {} undecidable on every road\n  \
             {} begins-events carrying a quest typed leg, {} structural facts no road shows \
             one for (the quest GIVING setups, which carry no typed leg of their own)\n  \
             {} locators ({} seated away from the scene their fact begins at), {} never-planned \
             facts checked against them, {} early seats both reads name, {} early on a road the \
             frontier does not name, {} early seats withheld so no locator exists",
            self.answered,
            self.roads,
            self.filtered_roads,
            self.scenes_walked,
            self.section_rows,
            self.unplaced_checks,
            self.empty_scene_checks,
            self.facts_named,
            self.undecidable_everywhere,
            self.quest_typed_events,
            self.structural_untyped,
            self.locators,
            self.overridden_seats,
            self.never_planned_checks,
            self.seated_early,
            self.early_on_unnamed_road,
            self.withheld_early_seats,
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

/// Ask both reads of one store under one telling and apply every law. The whole
/// contract lives here so the constructed store cannot be judged by a second,
/// gentler copy of it.
fn judge(ws: &Path, name: &str, telling: &str, ev: &mut Evidence) {
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
    let frontier = match read(&[DECLARES[0], "--telling", telling, "--json"]) {
        Ok(f) => f,
        Err(why) => {
            ev.note("the frontier refuses", format!("{name} [{telling}]: {why}"));
            return;
        }
    };
    let playable = match read(&[DECLARES[1], "--telling", telling, "--json"]) {
        Ok(p) => p,
        Err(why) => {
            ev.note(
                "the playable world refuses",
                format!("{name} [{telling}]: {why}"),
            );
            return;
        }
    };
    ev.answered += 1;

    // PROVENANCE — both reports say which question they answered, checked
    // before anything is compared. Comparing two answers to unstated questions
    // is what the first sweep of the R1048 pair did, and it read as eighteen
    // disagreements that were not there.
    if frontier["telling"].as_str() != Some(telling) {
        ev.disagreements.push(format!(
            "{name} [{telling}]: the frontier was asked for that telling and its report says `{}`",
            frontier["telling"]
        ));
    }
    if playable["telling"].as_str() != Some(telling) {
        ev.disagreements.push(format!(
            "{name} [{telling}]: the playable world was asked for that telling and its report \
             says `{}`",
            playable["telling"]
        ));
    }
    if !playable["world"].is_null() {
        ev.disagreements.push(format!(
            "{name} [{telling}]: the playable world was given no road filter and its report says \
             `{}`",
            playable["world"]
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

    // LAW ROADS. The playable world's OWN fork tree says which registered
    // world-lines are confluences — a merge node is not a playthrough, so the
    // unfiltered dump excludes it (R533). The frontier has no such filter: it
    // reports density for every registered branch. The two road sets therefore
    // differ BY CONSTRUCTION, and the contract is that they differ by exactly
    // that set and nothing else.
    let walked: BTreeSet<&str> = playable["worlds"]
        .as_object()
        .unwrap_or(&empty_map)
        .keys()
        .map(String::as_str)
        .collect();
    let confluences: BTreeSet<&str> = playable["fork_tree"]["branches"]
        .as_array()
        .unwrap_or(&empty_list)
        .iter()
        .filter(|branch| {
            !branch["converges"]
                .as_array()
                .unwrap_or(&empty_list)
                .is_empty()
        })
        .filter_map(|branch| branch["branch_id"].as_str())
        .collect();
    let frontier_roads: BTreeSet<&str> = known_roads.keys().copied().collect();
    for road in walked.difference(&frontier_roads) {
        ev.disagreements.push(format!(
            "{name} [{telling}]: the playable world walks `{road}` and the frontier reports no \
             density for it",
        ));
    }
    let unwalked: BTreeSet<&str> = frontier_roads.difference(&walked).copied().collect();
    if unwalked != confluences {
        ev.disagreements.push(format!(
            "{name} [{telling}]: the frontier knows {unwalked:?} that the playable dump does not \
             walk, and the playable world's own fork tree calls {confluences:?} confluences",
        ));
    }

    // Every road the frontier names is asked. The ones the dump does not carry
    // are asked with `--world` — a fact they own would otherwise be named by
    // nobody, which the census below turns from a worry into a number.
    let mut manuscripts: BTreeMap<&str, serde_json::Value> = BTreeMap::new();
    for road in &frontier_roads {
        if let Some(world) = playable["worlds"].get(road) {
            manuscripts.insert(road, world.clone());
            continue;
        }
        match read(&[DECLARES[1], "--telling", telling, "--world", road, "--json"]) {
            Ok(filtered) => {
                ev.filtered_roads += 1;
                if filtered["world"].as_str() != Some(road) {
                    ev.disagreements.push(format!(
                        "{name} [{telling}]/{road}: the playable world was asked for that road and \
                         its report says `{}`",
                        filtered["world"]
                    ));
                }
                match filtered["worlds"].get(road) {
                    Some(world) => {
                        manuscripts.insert(road, world.clone());
                    }
                    None => ev.disagreements.push(format!(
                        "{name} [{telling}]/{road}: the playable world answered the road filter \
                         and its map does not carry that road",
                    )),
                }
            }
            Err(why) => ev.note(
                "a road the frontier names that the playable world refuses",
                format!("{name} [{telling}]/{road}: {why}"),
            ),
        }
    }

    // `named_at` accumulates, ACROSS ROADS, the facts the playable world
    // attributes to each scene — beginning there, or naming it as an unplaced
    // coordinate. It is the census's left-hand side.
    let mut named_at: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    // Per scene, the facts the playable world shows carrying a quest typed leg —
    // the left-hand side of LAW STRUCTURAL.
    let mut quest_typed_at: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    let mut decided_somewhere: BTreeSet<String> = BTreeSet::new();
    let mut undecidable_somewhere: BTreeSet<String> = BTreeSet::new();
    let frontier_unplaced: BTreeSet<String> =
        strings(&frontier["unplaced_scenes"]).into_iter().collect();
    let zero_fact: BTreeSet<String> = strings(&frontier["zero_fact_scenes"]).into_iter().collect();
    // Per road: scenes walked / locators emitted / locators seated before the
    // scene their fact begins at. Printed as the shape line, because every
    // total below is a sum over these and a sum is where a road that stopped
    // answering hides (R1036).
    let mut per_road: BTreeMap<&str, [usize; 3]> = BTreeMap::new();
    for (road, world) in &manuscripts {
        ev.roads += 1;
        let manuscript = &world["manuscript"];
        let walk: Vec<&str> = manuscript["scenes"]
            .as_array()
            .unwrap_or(&empty_list)
            .iter()
            .filter_map(|scene| scene["section"].as_str())
            .collect();

        // LAW ROAD. Round 1061 named the frontier's road, so this compares the
        // two SEQUENCES rather than their lengths — two roads of one length
        // through different scenes were indistinguishable here until now, and
        // the whole point of the pair is that one linearizer has two call sites.
        let empty_road: Vec<&str> = Vec::new();
        let declared = known_roads.get(road).unwrap_or(&empty_road);
        if declared != &walk {
            ev.disagreements.push(format!(
                "{name} [{telling}]/{road}: the frontier says the road is {declared:?} and the \
                 playable world walks {walk:?}",
            ));
        }
        ev.scenes_walked += walk.len();
        per_road.entry(road).or_default()[0] = walk.len();

        // LAW PARTITION — travelled + off-road = the registry, with neither an
        // overlap nor a section unaccounted for.
        let travelled: BTreeSet<&str> = walk.iter().copied().collect();
        let off_road: BTreeSet<String> = strings(&manuscript["sections_off_road"])
            .into_iter()
            .collect();
        let registry: BTreeSet<&str> = coverage.keys().copied().collect();
        let claimed: BTreeSet<&str> = travelled
            .iter()
            .copied()
            .chain(off_road.iter().map(String::as_str))
            .collect();
        for section in travelled.iter().filter(|s| off_road.contains(**s)) {
            ev.disagreements.push(format!(
                "{name} [{telling}]/{road}: `{section}` is both walked and called off-road",
            ));
        }
        if claimed != registry {
            let missed: Vec<&&str> = registry.difference(&claimed).collect();
            let invented: Vec<&&str> = claimed.difference(&registry).collect();
            ev.disagreements.push(format!(
                "{name} [{telling}]/{road}: the frontier's scene census and this road's \
                 walk+off-road do not partition each other (uncovered {missed:?}, unknown to the \
                 frontier {invented:?})",
            ));
        }
        ev.section_rows += registry.len();

        // LAW UNPLACED — a section no order positions is travelled by no road.
        for section in &frontier_unplaced {
            ev.unplaced_checks += 1;
            if travelled.contains(section.as_str()) {
                ev.disagreements.push(format!(
                    "{name} [{telling}]/{road}: the frontier says no order positions `{section}` \
                     and this road walks it",
                ));
            }
        }

        // LAW EMPTY + the census's per-scene left-hand side.
        for scene in manuscript["scenes"].as_array().unwrap_or(&empty_list) {
            let Some(section) = scene["section"].as_str() else {
                continue;
            };
            let begins = fact_ids(&scene["begins"]);
            if zero_fact.contains(section) {
                ev.empty_scene_checks += 1;
                if !begins.is_empty() {
                    ev.disagreements.push(format!(
                        "{name} [{telling}]/{road}: the frontier calls `{section}` a zero-fact \
                         scene and the playable world begins {begins:?} there",
                    ));
                }
            }
            let Some((section, _)) = coverage.get_key_value(section) else {
                continue;
            };
            // The typed legs the playable world hands a runtime — the other
            // read's answer to "which of these facts is quest plumbing". Read
            // BEFORE `begins` is flattened to ids, because the predicate is the
            // whole signal (R676: the three predicates are the sole marker).
            for event in scene["begins"].as_array().unwrap_or(&empty_list) {
                let (Some(fact), Some(predicate)) = (
                    event["fact_id"].as_str(),
                    event["typed"]["predicate"].as_str(),
                ) else {
                    continue;
                };
                if !matches!(
                    predicate,
                    QUEST_PRED_PURSUES | QUEST_PRED_REQUIRES | QUEST_PRED_COMPLETED_BY
                ) {
                    continue;
                }
                ev.quest_typed_events += 1;
                quest_typed_at
                    .entry(section)
                    .or_default()
                    .insert(fact.to_string());
            }
            for fact in begins {
                decided_somewhere.insert(fact.clone());
                named_at.entry(section).or_default().insert(fact);
            }
        }
        for unplaced in manuscript["unplaced_facts"]
            .as_array()
            .unwrap_or(&empty_list)
        {
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
            if let Some((section, _)) = coverage.get_key_value(coordinate) {
                named_at
                    .entry(section)
                    .or_default()
                    .insert(fact.to_string());
            }
        }
        for fact in strings(&manuscript["undecidable"]) {
            undecidable_somewhere.insert(fact);
        }
    }

    // LAW CENSUS. Scene by scene, the facts the frontier holds and the facts the
    // playable world names there are the SAME SET, but for the ones no road
    // could decide — and those are named too, not absorbed into a total.
    //
    // Round 1052 could only state this as a per-scene `≤` plus an equal total,
    // because the census shipped a COUNT: two facts trading the coordinates they
    // are anchored at moved neither number and left this law green over an edit
    // the playable world plainly walks a reader through. That is measured, in
    // `the_frontier_census_names_the_facts_it_counts` below, and Round 1053
    // changed the wire so it can be said at all.
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
                "{name} [{telling}]: the playable world names {invented:?} at `{section}` and the \
                 frontier's census does not hold them there",
            ));
        }
        undecided_at.extend(held.difference(&named).cloned());
    }
    ev.facts_named += named_total;
    // The leftovers are exactly the facts no road could place. A fact the
    // frontier holds at a scene that no road names THERE, but that some road
    // decided elsewhere, is a disagreement about where it begins — the class the
    // old total could not separate from a fact nobody could decide at all.
    if undecided_at != undecidable_only {
        let unexplained: Vec<&String> = undecided_at.difference(&undecidable_only).collect();
        let missing: Vec<&String> = undecidable_only.difference(&undecided_at).collect();
        ev.disagreements.push(format!(
            "{name} [{telling}]: the frontier holds {unexplained:?} at scenes no road names them \
             at while some road did decide them, and calls {missing:?} undecidable everywhere \
             while its own census does not leave them over",
        ));
    }

    // LAW STRUCTURAL. The census marks a subset of each scene's facts as quest
    // plumbing, so a coverage read can subtract it and not read bookkeeping as
    // narrative (R619). The playable world is what SAYS which facts are: it hands
    // the runtime each begins-event's typed leg verbatim, and R676 made the three
    // quest predicates the sole marker of quest-ness. So a fact the runtime is
    // shown pursuing, requiring or completing a quest must be one the frontier
    // has already subtracted, at the very scene it begins.
    //
    // Round 1052 declared this pair and left `structural_facts` touching no law
    // at all; Round 1053 measured what that cost by emptying the subset and
    // watching the whole workspace stay green. The converse is NOT a law and is
    // counted instead: a quest GIVING setup is structural because a completion
    // pays it off, not because it carries a typed leg, so the playable world has
    // nothing to show for it and a residual here is expected work, not a defect.
    let structural_at: BTreeMap<&str, BTreeSet<String>> = frontier["scene_coverage"]
        .as_array()
        .unwrap_or(&empty_list)
        .iter()
        .filter_map(|row| {
            row["scene"]
                .as_str()
                .map(|scene| (scene, strings(&row["structural"]).into_iter().collect()))
        })
        .collect();
    let structural_flat: BTreeSet<String> =
        strings(&frontier["structural_facts"]).into_iter().collect();
    let mut structural_shown: BTreeSet<String> = BTreeSet::new();
    for (section, shown) in &quest_typed_at {
        structural_shown.extend(shown.iter().cloned());
        let marked = structural_at.get(section).cloned().unwrap_or_default();
        let unsubtracted: Vec<&String> = shown.difference(&marked).collect();
        if !unsubtracted.is_empty() {
            ev.disagreements.push(format!(
                "{name} [{telling}]: the playable world hands the runtime {unsubtracted:?} at \
                 `{section}` typed as quest plumbing and the frontier's census does not mark them \
                 structural there",
            ));
        }
    }
    let unlisted: Vec<&String> = structural_shown.difference(&structural_flat).collect();
    if !unlisted.is_empty() {
        ev.disagreements.push(format!(
            "{name} [{telling}]: {unlisted:?} carry a quest typed leg on some road and the \
             frontier's flat structural set does not name them",
        ));
    }
    // The two keyings of one set: every scene's marked subset is drawn from the
    // facts that scene holds, and the flat list is their union plus the
    // structural facts anchored at no registered section.
    for (section, marked) in &structural_at {
        let held = coverage.get(section).cloned().unwrap_or_default();
        let stray: Vec<&String> = marked.difference(&held).collect();
        let unlisted: Vec<&String> = marked.difference(&structural_flat).collect();
        if !stray.is_empty() || !unlisted.is_empty() {
            ev.disagreements.push(format!(
                "{name} [{telling}]: the census marks {stray:?} structural at `{section}` without \
                 holding them there, and {unlisted:?} without the flat structural set naming them",
            ));
        }
    }
    ev.structural_untyped += structural_flat.difference(&structural_shown).count();

    // LAW DISCLOSURE, half one — a never-planned fact carries no locator. The
    // frontier's list is the author's todo; a locator is the runtime's pointer
    // AT the fact. R643: a fact the plan never decided about is withheld by
    // default, and a withheld fact is not on the map — the arm whose absence
    // once put a story's core truth on a consumer's screen.
    let mut located: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for (road, world) in &manuscripts {
        for locator in world["locators"].as_array().unwrap_or(&empty_list) {
            ev.locators += 1;
            per_road.entry(road).or_default()[1] += 1;
            if let Some(fact) = locator["fact_id"].as_str() {
                located.entry(road).or_default().insert(fact);
            }
        }
    }
    for fact in strings(&frontier["never_planned_disclosures"]) {
        ev.never_planned_checks += 1;
        for (road, on_road) in &located {
            if on_road.contains(fact.as_str()) {
                ev.disagreements.push(format!(
                    "{name} [{telling}]/{road}: the frontier says `{fact}` was never given a \
                     disclosure decision and the playable world hands the runtime a locator for it",
                ));
            }
        }
    }

    // LAW DISCLOSURE, half two — a seat before the truth, judged from the other
    // side. The frontier orders the authored `surface.scene` against the fact's
    // `canon_from` on the fact's OWN road; the playable world states the same
    // thing as an ordinal into the walk it hands the runtime.
    let seats: BTreeMap<(&str, &str), &serde_json::Value> = frontier
        ["disclosures_seated_before_truth"]
        .as_array()
        .unwrap_or(&empty_list)
        .iter()
        .filter_map(|row| Some(((row["world"].as_str()?, row["fact_id"].as_str()?), row)))
        .collect();
    let mut agreed_seats: BTreeSet<(&str, &str)> = BTreeSet::new();
    // The frontier scopes each row to the fact's OWN branch, so a row for the
    // road being walked may legitimately be absent. The FACT may not be: a fork
    // inherits its parent's prefix whole, so a seat that is early on the fork is
    // early on the parent's road too, where the frontier does name it.
    let seated_facts: BTreeSet<&str> = seats.keys().map(|(_, fact)| *fact).collect();
    for (road, world) in &manuscripts {
        let scenes = world["manuscript"]["scenes"]
            .as_array()
            .unwrap_or(&empty_list);
        let ordinal: BTreeMap<&str, usize> = scenes
            .iter()
            .enumerate()
            .filter_map(|(index, scene)| Some((scene["section"].as_str()?, index)))
            .collect();
        let begins_at: BTreeMap<&str, usize> = scenes
            .iter()
            .enumerate()
            .flat_map(|(index, scene)| {
                scene["begins"]
                    .as_array()
                    .unwrap_or(&empty_list)
                    .iter()
                    .filter_map(move |event| Some((event["fact_id"].as_str()?, index)))
                    .collect::<Vec<_>>()
            })
            .collect();
        for locator in world["locators"].as_array().unwrap_or(&empty_list) {
            let (Some(fact), Some(seat)) = (locator["fact_id"].as_str(), locator["scene"].as_str())
            else {
                continue;
            };
            let seat_ordinal = ordinal.get(seat).copied();
            if locator["scene_ordinal"].as_u64().map(|n| n as usize) != seat_ordinal {
                ev.disagreements.push(format!(
                    "{name} [{telling}]/{road}: the locator for `{fact}` is seated at `{seat}` and \
                     its ordinal {} is not that scene's index {seat_ordinal:?}",
                    locator["scene_ordinal"],
                ));
            }
            let Some(&truth) = begins_at.get(fact) else {
                // A locator for a fact this road does not begin: the
                // manuscript's own placement surfaces say why, and the census
                // above is what judges those.
                continue;
            };
            let truth_section = scenes[truth]["section"].as_str().unwrap_or_default();
            if seat != truth_section {
                ev.overridden_seats += 1;
            }
            match (
                seat_ordinal.is_some_and(|seat| seat < truth),
                seats.get(&(road as &str, fact)),
            ) {
                (true, Some(row)) => {
                    ev.seated_early += 1;
                    per_road.entry(road).or_default()[2] += 1;
                    agreed_seats.insert((road, fact));
                    if row["seated_at"].as_str() != Some(seat) {
                        ev.disagreements.push(format!(
                            "{name} [{telling}]/{road}: the frontier says `{fact}` is seated at \
                             `{}` and the locator says `{seat}`",
                            row["seated_at"],
                        ));
                    }
                    if row["true_from"].as_str() != Some(truth_section) {
                        ev.disagreements.push(format!(
                            "{name} [{telling}]/{road}: the frontier says `{fact}` becomes true at \
                             `{}` and the playable world begins it at `{truth_section}`",
                            row["true_from"],
                        ));
                    }
                }
                (true, None) => {
                    per_road.entry(road).or_default()[2] += 1;
                    if !seated_facts.contains(fact) {
                        ev.disagreements.push(format!(
                            "{name} [{telling}]/{road}: the playable world seats `{fact}` before \
                             the scene it begins at and the frontier does not call it seated \
                             before its truth on any road",
                        ));
                    }
                    // Early on a road the frontier does not name. It scopes each
                    // row to the fact's OWN branch, so a fact inherited by a
                    // fork is early there too and listed once. Counted, not
                    // called a disagreement: one authored coordinate is one
                    // piece of work for the loop the frontier feeds, and the
                    // number is what a consumer joining by (fact, road) would
                    // be missing.
                    ev.early_on_unnamed_road += 1;
                }
                (false, Some(_)) => ev.disagreements.push(format!(
                    "{name} [{telling}]/{road}: the frontier says `{fact}` is disclosed before it \
                     is true and the playable world seats its locator at or after the scene it \
                     begins",
                )),
                (false, None) => {}
            }
        }
    }
    // The rows the walk never reached: a seat can be early and WITHHELD, and
    // then no locator exists to carry it (R643). The frontier is right to list
    // it — the author still seated it early — and this is the shape of the
    // disagreement that would otherwise read as one.
    for (road, fact) in seats.keys() {
        if !agreed_seats.contains(&(road, fact)) {
            if manuscripts.contains_key(road)
                && !located.get(road).is_some_and(|f| f.contains(fact))
            {
                ev.withheld_early_seats += 1;
            } else if manuscripts.contains_key(road) {
                ev.disagreements.push(format!(
                    "{name} [{telling}]/{road}: the frontier says `{fact}` is seated before it is \
                     true and the playable world's locator for it is not early",
                ));
            }
        }
    }

    ev.shape.push(format!(
        "{name} [{telling}]: {} sections | {}",
        coverage.len(),
        per_road
            .iter()
            .map(|(road, [scenes, locators, early])| format!(
                "{road}={scenes}sc/{locators}loc/{early}early"
            ))
            .collect::<Vec<_>>()
            .join(" "),
    ));
}

/// Two facts trade the coordinate they are anchored at — `canon_from` and the
/// evidence that names it, so each fact moves whole rather than being left
/// pointing at the scene it came from.
fn trade_coordinates(facts: &serde_json::Value, one: &str, other: &str) -> serde_json::Value {
    let coordinate = |id: &str| {
        facts["facts"]
            .as_array()
            .expect("facts array")
            .iter()
            .find(|fact| fact["fact_id"] == id)
            .unwrap_or_else(|| panic!("the constructed store declares `{id}`"))["canon_from"]
            .as_str()
            .unwrap_or_else(|| panic!("`{id}` is anchored somewhere"))
            .to_string()
    };
    let (here, there) = (coordinate(one), coordinate(other));
    assert_ne!(
        here, there,
        "a trade between two facts at ONE coordinate would be no edit at all"
    );
    let mut traded = facts.clone();
    let mut moved = 0usize;
    for fact in traded["facts"].as_array_mut().expect("facts array") {
        let to = match fact["fact_id"].as_str() {
            Some(id) if id == one => &there,
            Some(id) if id == other => &here,
            _ => continue,
        };
        fact["canon_from"] = serde_json::json!(to);
        fact["evidence"] = serde_json::json!([to]);
        moved += 1;
    }
    assert_eq!(moved, 2, "the trade moved {moved} facts, not the two named");
    traded
}

/// The sections, order and facts of the store the authors did not write: a
/// confluence under a declared telling, a section outside the order (carrying a
/// fact, so it is `unordered` as well as unplaced), a scene with nothing in it,
/// a disclosure seated before its truth, and a second one seated early but
/// WITHHELD so the runtime never receives it.
fn constructed_manifests() -> (serde_json::Value, serde_json::Value, serde_json::Value) {
    let section = |id: &str, title: &str| {
        serde_json::json!({
            "section_id": id,
            "parent_doc": "constructed",
            "title": title,
        })
    };
    let sections = serde_json::json!([
        section("c-01", "the road opens"),
        section("c-02", "the errand is given"),
        section("c-empty", "a scene with nothing in it yet"),
        section("c-03", "the road divides"),
        section("c-left", "the left road"),
        section("c-right", "the right road"),
        section("c-join", "where both roads arrive"),
        section("c-after", "the shared continuation"),
        section("c-loose", "a scene no order positions"),
    ]);
    let order = serde_json::json!({
        "edges": [["c-01", "c-02"], ["c-02", "c-empty"], ["c-empty", "c-03"]],
        "branches": {
            "left": [["c-03", "c-left"], ["c-left", "c-join"]],
            "right": [["c-03", "c-right"], ["c-right", "c-join"]],
            "merge": [["c-join", "c-after"]],
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
    // The withheld fact carries a typed leg because the write path requires
    // one: a withhold decision is un-gateable without it (the premature-leak
    // gate matches by typed tuple), so an author could not ship this store
    // without typing the fact either.
    let mut hidden = fact("fx-hidden", None, "c-02", "the errand is a lie");
    hidden["entities"] = serde_json::json!(["e-errand"]);
    hidden["typed"] = serde_json::json!({
        "subject": "e-errand",
        "predicate": "p-truth",
        "object": {"kind": "token", "token": "false"},
    });
    let facts = serde_json::json!({
        "frames": [{"frame_id": "f-narrator"}],
        "entity_kinds": [{"kind_id": "thing"}],
        "entities": [{"entity_id": "e-errand", "kind": "thing",
                      "description": "the errand the road is walked for"}],
        "predicates": [{"predicate_id": "p-truth", "object_kind": "token",
                        "object_tokens": ["true", "false"],
                        "description": "what is so about the errand"}],
        "branches": [
            {"branch_id": "left", "description": "the left road",
             "forks_from": "main", "forks_at": "c-03"},
            {"branch_id": "right", "description": "the right road",
             "forks_from": "main", "forks_at": "c-03"},
            {"branch_id": "merge", "description": "where both roads arrive",
             "converges_from": [{"branch": "left", "at": "c-left"},
                                {"branch": "right", "at": "c-right"}]},
        ],
        "facts": [
            fact("fx-open", None, "c-01", "the gate stands open"),
            hidden,
            fact("fx-mid", None, "c-03", "the two roads are known to part here"),
            fact("fx-left", Some("left"), "c-left", "the left road is taken"),
            fact("fx-right", Some("right"), "c-right", "the right road is taken"),
            fact("fx-merge", Some("merge"), "c-after", "both roads end at the same door"),
            // Never given a disclosure decision AND placed on the road — the
            // R643 arm needs a never-planned fact a locator could exist for.
            fact("fx-quiet", None, "c-02", "the gate-keeper is never named"),
            fact("fx-loose", None, "c-loose", "a fact at a coordinate no order positions"),
        ],
        "disclosure_plans": [{
            "telling_id": "t-one",
            "default_mode": "withhold",
            "overrides": [
                {"fact_id": "fx-open", "mode": "state"},
                // Seated before its truth: stated at the first scene, true only
                // from the scene the roads divide at.
                {"fact_id": "fx-mid", "mode": "state", "surface": {"scene": "c-01"}},
                // Early AND withheld: the frontier lists the seat, the runtime
                // gets no locator (R643).
                {"fact_id": "fx-hidden", "mode": "withhold", "surface": {"scene": "c-01"}},
                {"fact_id": "fx-left", "mode": "state"},
                {"fact_id": "fx-right", "mode": "state"},
                {"fact_id": "fx-merge", "mode": "state"},
            ],
        }],
    });
    (sections, order, facts)
}

#[test]
fn the_frontier_counts_per_scene_what_the_playable_world_walks_per_road() {
    // POPULATION ONE — every store an author shipped that this tree can ask.
    let (stores, unloadable) = authored_stores();
    let asked = stores.len() + unloadable.len();
    let mut authored = Evidence::default();
    for corpus in &unloadable {
        authored.note("the store does not load", corpus.named_reason());
    }
    for store in &stores {
        let ws = store.ws.path();
        let tellings = AtomicStore::load(&ws.join(SIDECAR))
            .map(|store| declared_tellings(&store))
            .unwrap_or_default();
        if tellings.is_empty() {
            authored.note(
                "declares no telling, so the playable world cannot be asked",
                store.name.clone(),
            );
            continue;
        }
        for telling in &tellings {
            judge(ws, &store.name, telling, &mut authored);
        }
    }

    // POPULATION TWO — the store no author wrote, through the same recipe.
    let (sections, order, facts) = constructed_manifests();
    let built = constructed_corpus(&sections, &order, &facts)
        .unwrap_or_else(|e| panic!("the constructed manifests must import: {e}"));
    let mut constructed = Evidence::default();
    judge(
        built.path(),
        "the constructed store",
        "t-one",
        &mut constructed,
    );

    // Print BEFORE asserting (the R1026 lesson).
    println!("{asked} authored stores asked");
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
            authored.count("declares no telling, so the playable world cannot be asked"),
        ) == (44, 3, 30),
        "POPULATION (stores): of the corpora asked, these never reach the \
         comparison at all — 3 whose author's submission the write path \
         rejected, 30 declaring no telling for a pair whose playable half is \
         per-telling. The corpora that declare a confluence are inside that 30, \
         which is why the constructed store still exists",
    );
    check(
        (
            authored.answered,
            authored.roads,
            authored.filtered_roads,
            authored.scenes_walked,
            authored.section_rows,
        ) == (14, 21, 0, 444, 572),
        "AUTHORED EVIDENCE: the (store, telling) pairs that answered both reads, \
         the roads compared, the roads the dump could not answer for, the scenes \
         walked, and the section-rows the partition law compared",
    );
    check(
        (
            authored.unplaced_checks,
            authored.empty_scene_checks,
            authored.seated_early,
            authored.early_on_unnamed_road,
            authored.withheld_early_seats,
        ) == (0, 0, 0, 0, 0),
        "AUTHORED SILENCE, ASSERTED: no corpus holds a section outside its \
         order, a scene with nothing in it, or a seat before its truth — so \
         four of the seven laws are carried entirely by the constructed store, \
         and this line is what makes that visible rather than averaged away",
    );
    check(
        (authored.facts_named, authored.undecidable_everywhere) == (1151, 0),
        "AUTHORED CENSUS: the facts the playable world names at a scene the \
         frontier counts, and the residual it can place on no road — with the \
         residual at zero the per-scene `≤` and the equal total ARE per-scene \
         equality",
    );
    check(
        (authored.quest_typed_events, authored.structural_untyped) == (52, 7),
        "AUTHORED STRUCTURAL: the begins-events the playable world hands a \
         runtime with a quest typed leg — every one of them subtracted by the \
         census at the scene it begins — and the structural facts no road shows \
         one for, which are the quest GIVING setups. Round 1174 lit the \
         dnd-quest corpus's own tracked manifest, so these no longer ride on the \
         single hand-migrated record",
    );
    check(
        (
            constructed.quest_typed_events,
            constructed.structural_untyped,
        ) == (0, 0),
        "CONSTRUCTED STRUCTURAL, ASSERTED ZERO: the store the tree builds \
         declares no quest, so it carries this law's evidence not at all — \
         stated rather than averaged in with the authored 52",
    );
    check(
        (
            authored.locators,
            authored.overridden_seats,
            authored.never_planned_checks,
        ) == (1554, 8, 20),
        "AUTHORED DISCLOSURE: the locators judged, how many sit away from their \
         fact's own scene (the only ones that CAN be early), and the \
         never-planned facts checked against them",
    );
    check(
        (
            constructed.answered,
            constructed.roads,
            constructed.filtered_roads,
            constructed.scenes_walked,
            constructed.section_rows,
        ) == (1, 4, 1, 26, 36),
        "CONSTRUCTED EVIDENCE: one store, four roads — a 4-scene trunk, two \
         forks that each walk 7 because they carry on through the merge, and \
         the confluence's own 8-scene fragment, which only a `--world` ask \
         reaches",
    );
    check(
        (
            constructed.unplaced_checks,
            constructed.empty_scene_checks,
            constructed.facts_named,
            constructed.undecidable_everywhere,
        ) == (4, 7, 8, 0),
        "CONSTRUCTED PLACEMENT: the unplaced section checked on all four roads, \
         the two zero-fact scenes checked on every road that walks them, and \
         the census naming all eight facts with nothing left undecidable \
         everywhere — so the per-scene `≤` and the equal total ARE equality",
    );
    check(
        (
            constructed.locators,
            constructed.overridden_seats,
            constructed.never_planned_checks,
            constructed.seated_early,
            constructed.early_on_unnamed_road,
            constructed.withheld_early_seats,
        ) == (11, 3, 2, 1, 2, 1),
        "CONSTRUCTED DISCLOSURE: the R949 arm fired on the fact's own road, on \
         the two forks that inherit it (which the frontier does not name, \
         scoping each row to the fact's own branch), and on the withheld seat \
         the runtime never receives. The confluence fragment shows why the \
         count is two and not three: it carries ONE locator over eight scenes, \
         because a fragment leaves the pre-merge trunk undecidable",
    );
    check(
        authored.disagreements.is_empty() && constructed.disagreements.is_empty(),
        "CONTRACT: the frontier's per-scene census and per-road lengths are the \
         playable world's walks, and its telling-scoped todo is the locators the \
         runtime reads",
    );

    assert_eq!(
        broken,
        Vec::<String>::new(),
        "the frontier/playable-world correspondence no longer holds"
    );
}

/// A store where two facts trade scenes is a store the frontier must be able to
/// tell apart — and until Round 1053 it could not. (Round 1053.)
///
/// Round 1052 declared the CENSUS law above: every fact the frontier counts at a
/// scene is named there by the playable world. Its evidence was a per-scene `≤`
/// plus an equal total, and those two statements together are an equality of
/// NUMBERS. Two facts trading the coordinates they are anchored at leaves every
/// number exactly where it was, so the contract stayed green over an edit the
/// other read plainly sees — the shape Round 1050 had already met on
/// `not_holding`, where a count could not say WHICH fact it had counted. Twice
/// in three rounds is a class, not an accident.
///
/// The hole was not in the contract. It was in the WIRE: `scene_coverage`
/// shipped `fact_count`, so no test over this read's output could have judged
/// identity, and no contract that could be written would have closed it. This
/// test is what says so — it asserts what does NOT move. Every count is where it
/// was, and with the census set aside the two reports are byte-equal, so the
/// census is the only field of this read where the trade can ever appear. On the
/// wire this round replaced, that made the two stores indistinguishable.
#[test]
fn the_frontier_census_names_the_facts_it_counts() {
    let (sections, order, facts) = constructed_manifests();
    // Both facts sit on `main`, neither carries an authored seat, and their
    // scenes hold one and two facts — so the trade moves no count, no placement
    // set, no density and no disclosure row. It moves only WHICH fact is where.
    let traded = trade_coordinates(&facts, "fx-open", "fx-quiet");
    let build = |manifest: &serde_json::Value| {
        constructed_corpus(&sections, &order, manifest)
            .unwrap_or_else(|e| panic!("the constructed manifests must import: {e}"))
    };
    let (before, after) = (build(&facts), build(&traded));

    let ask = |ws: &Path, verb: &str| -> serde_json::Value {
        let out = run(ws, &[verb, "--telling", "t-one", "--json"]);
        assert!(
            out.status.success(),
            "{verb} on the constructed store: {}",
            String::from_utf8_lossy(&out.stderr),
        );
        serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("{verb} is not json: {e}"))
    };
    let frontier = (
        ask(before.path(), DECLARES[0]),
        ask(after.path(), DECLARES[0]),
    );
    let playable = (
        ask(before.path(), DECLARES[1]),
        ask(after.path(), DECLARES[1]),
    );

    let empty: Vec<serde_json::Value> = Vec::new();
    // Where the playable world begins a fact on a road — the other read's answer
    // to the same question, which is what makes the trade an edit and not a
    // rewording.
    let begins_at = |playable: &serde_json::Value, road: &str, fact: &str| -> Option<String> {
        playable["worlds"][road]["manuscript"]["scenes"]
            .as_array()?
            .iter()
            .find(|scene| {
                scene["begins"]
                    .as_array()
                    .is_some_and(|events| events.iter().any(|event| event["fact_id"] == fact))
            })?["section"]
            .as_str()
            .map(ToString::to_string)
    };
    // The census as (scene -> how many, how many structural). NOT the names:
    // this is the number the wire used to carry, and the claim below is that it
    // is the same number on both sides of the trade.
    let counted = |frontier: &serde_json::Value| -> BTreeMap<String, (usize, usize)> {
        frontier["scene_coverage"]
            .as_array()
            .unwrap_or(&empty)
            .iter()
            .map(|row| {
                (
                    row["scene"].as_str().unwrap_or_default().to_string(),
                    (
                        strings(&row["facts"]).len(),
                        strings(&row["structural"]).len(),
                    ),
                )
            })
            .collect()
    };
    let named_total: usize = counted(&frontier.0).values().map(|(all, _)| all).sum();
    let without_census = |frontier: &serde_json::Value| {
        let mut rest = frontier.clone();
        rest.as_object_mut()
            .expect("the frontier report is an object")
            .remove("scene_coverage");
        rest
    };

    println!(
        "the trade moves `fx-open` {:?} -> {:?} and `fx-quiet` {:?} -> {:?} on main; the \
         frontier's census names {named_total} facts across {} scenes",
        begins_at(&playable.0, "main", "fx-open"),
        begins_at(&playable.1, "main", "fx-open"),
        begins_at(&playable.0, "main", "fx-quiet"),
        begins_at(&playable.1, "main", "fx-quiet"),
        counted(&frontier.0).len(),
    );

    let mut broken: Vec<String> = Vec::new();
    let mut check = |ok: bool, claim: &str| {
        if !ok {
            broken.push(claim.to_string());
        }
    };

    check(
        [
            begins_at(&playable.0, "main", "fx-open").as_deref(),
            begins_at(&playable.1, "main", "fx-open").as_deref(),
            begins_at(&playable.0, "main", "fx-quiet").as_deref(),
            begins_at(&playable.1, "main", "fx-quiet").as_deref(),
        ] == [Some("c-01"), Some("c-02"), Some("c-02"), Some("c-01")],
        "THE EDIT IS REAL: the playable world begins the two facts at each \
         other's scenes after the trade, so this is a store difference a reader \
         walks through and not a difference of spelling",
    );
    check(
        counted(&frontier.0) == counted(&frontier.1),
        "INVISIBLE TO COUNTS: every scene holds as many facts, and as many \
         structural ones, as it did before — the trade is exactly the edit a \
         census of numbers cannot report",
    );
    check(
        without_census(&frontier.0) == without_census(&frontier.1),
        "AND TO EVERY OTHER FIELD: with the census set aside the two reports are \
         equal, so `scene_coverage` is the ONLY place in this read where the \
         trade could ever appear",
    );
    check(
        named_total == 8 && counted(&frontier.0).len() == 9,
        "NON-VACUITY: the census names all eight facts of the constructed store \
         over its nine registered scenes. Without this the claim above holds for \
         a census that names nothing at all",
    );
    check(
        frontier.0["scene_coverage"] != frontier.1["scene_coverage"],
        "THE LAW: the census MOVED. It names the facts it counts, so a store \
         where two of them trade scenes is a store this read describes \
         differently — which is what makes the Round 1052 CENSUS law an equality \
         of facts rather than of numbers",
    );

    assert_eq!(
        broken,
        Vec::<String>::new(),
        "the frontier's scene census does not name what it counts"
    );
}
