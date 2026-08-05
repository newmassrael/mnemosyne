//! A road filter SELECTS; it does not re-derive. (Round 1049.)
//!
//! Round 1048 taught the three narrative projections to say which question they
//! were asked, and left the other half open: an answer that NAMES its filter
//! still has to MEAN it. The pair contract it shipped
//! (`manuscript_projection_agreement.rs`) asks both reads unfiltered and never
//! once under `--world`, so three of that round's four world injections left it
//! green — the `world: null` arm was the only one any corpus ever fired.
//!
//! THE UNIT IS THE READ, NOT THE PAIR. Adding a filtered arm to one pair asks
//! "do these two agree under a filter" once. What a filter MEANS is a property
//! of ONE read and is derivable for the whole shipped surface at once, the same
//! move R1048 made when it stopped declaring pairs and derived provenance. So
//! this walk asks every read that takes a road filter, on every road the corpus
//! registers, and holds it to the only thing a filter can honestly be:
//!
//! - the roads it KEEPS answer exactly what the unfiltered read said about them,
//!   byte for byte, once the read's record of the filter is stripped out;
//! - it never answers a road the unfiltered read did not;
//! - and every road-keyed structure inside the answer either narrows to the
//!   asked road or stays WHOLE, which is a per-read fact this walk prints as a
//!   table and pins, because "the fork tree stays full under a filter" has been
//!   a doc comment since R556 and nothing has ever run it.
//!
//! WHY THAT IS THE HONEST RULE AND NOT "EVERYTHING NARROWS". The playable
//! world's fork tree is navigation context: the topology is inherently
//! cross-world, so a filtered read keeps it whole ON PURPOSE. A rule that
//! demanded narrowing everywhere would be refuted by the shipped design on its
//! first run. A rule that demanded nothing about shape would not notice a filter
//! that quietly stopped filtering. Naming which structures narrow — and counting
//! them — is the form that survives both.
//!
//! THE POPULATION IS DERIVED, NOT LISTED. The verbs come from the shipped help,
//! the flags from each verb's own usage line, and a flag counts as a ROAD FILTER
//! when the value vocabulary the shared resolver gives it IS the corpus's road
//! registry — so a new road-taking flag joins this walk the run it ships, and a
//! verb that grows one is covered without anybody writing it down (the R1046
//! lesson). Asked of every store an author shipped (`authored_stores()`), with
//! the ones that cannot answer counted by name rather than dropped (R1036).

use std::collections::{BTreeMap, BTreeSet};

use mnemosyne_atomic::AtomicStore;
use serde_json::Value;

mod common;
use common::{
    advertised_reads, authored_stores, baseline_argv, flags_of, record_of, run, substance,
    usage_lines, values_for, SIDECAR,
};

/// What one road-keyed structure did under the filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Shape {
    /// The filter narrowed it to the asked road.
    Narrowed,
    /// It stayed whole — a structure whose meaning is cross-world.
    Whole,
}

/// How a structure carries the road it is about — the three encodings the
/// shipped reads use, derived from the value rather than named per verb.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Keyed {
    /// An object whose every key is a registered road (`worlds`, `per_world`).
    Map,
    /// An array of road ids (the quest graph's sorted per-world key set).
    Ids,
    /// An array of records, each ABOUT one road under this field (the quest
    /// graph's `locators`, one per world where a giving fact is disclosed).
    Records(String),
}

/// One filtered-versus-unfiltered comparison in progress.
struct Selection<'a> {
    /// Every road this corpus registers — the ids that can appear as a KEY.
    roads: &'a BTreeSet<String>,
    /// The road this probe asked for.
    road: &'a str,
    /// Road-keyed structures reached, by path, and what the filter did to them.
    /// Only recorded where the unfiltered structure held MORE than one road:
    /// on a one-road corpus "narrowed" and "whole" are the same answer, and
    /// recording it would put a verdict in the table that no corpus supported.
    shapes: BTreeMap<String, Shape>,
    /// Everything the filter changed that selecting cannot explain.
    moved: Vec<String>,
}

/// A path with its array indices blanked — `/quests[0]/locators` reads
/// `/quests[]/locators`. The shape table is a claim about the READ, not about
/// how many quests a corpus happens to hold, and keying it by index would make
/// an author adding a quest look like a structural change.
fn shape_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    let mut digits = false;
    for ch in path.chars() {
        match ch {
            '[' => {
                digits = true;
                out.push('[');
            }
            ']' => {
                digits = false;
                out.push(']');
            }
            _ if digits => {}
            _ => out.push(ch),
        }
    }
    out
}

/// A value cut down for a failure message — a whole world's manuscript printed
/// raw is megabytes of scene text and says nothing about where to look.
fn brief(value: &Value) -> String {
    let mut text = value.to_string();
    if text.chars().count() > 80 {
        text = text.chars().take(77).collect::<String>() + "...";
    }
    text
}

impl Selection<'_> {
    /// How this value carries roads, if it carries them at all. Read from the
    /// UNFILTERED side: it is the answer that says what shape the read has, and
    /// the filtered side is what has to match it.
    fn keying(&self, value: &Value) -> Option<Keyed> {
        match value {
            Value::Object(map) if !map.is_empty() => map
                .keys()
                .all(|key| self.roads.contains(key))
                .then_some(Keyed::Map),
            Value::Array(items) if !items.is_empty() => {
                if items
                    .iter()
                    .all(|item| item.as_str().is_some_and(|id| self.roads.contains(id)))
                {
                    return Some(Keyed::Ids);
                }
                // A record is about one road when ONE of its fields holds a
                // road id, in every element. Two such fields would leave the
                // walk choosing which one the filter means, which is a judgement
                // it must not make silently — it says so instead.
                let carriers: Vec<&String> = items[0].as_object()?.keys().collect();
                let carriers: Vec<String> = carriers
                    .into_iter()
                    .filter(|key| {
                        items.iter().all(|item| {
                            item.get(key)
                                .and_then(Value::as_str)
                                .is_some_and(|id| self.roads.contains(id))
                        })
                    })
                    .cloned()
                    .collect();
                match carriers.len() {
                    1 => Some(Keyed::Records(carriers[0].clone())),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// The roads a value holds under a known keying — read WITHOUT re-testing
    /// the shape, because a filter that narrows a structure to nothing leaves
    /// an empty one, and empty must not read as "no longer that structure".
    fn roads_in(&self, value: &Value, keyed: &Keyed) -> Option<Vec<String>> {
        match (keyed, value) {
            (Keyed::Map, Value::Object(map)) => Some(map.keys().cloned().collect()),
            (Keyed::Ids, Value::Array(items)) => items
                .iter()
                .map(|item| item.as_str().map(ToString::to_string))
                .collect(),
            (Keyed::Records(field), Value::Array(items)) => items
                .iter()
                .map(|item| {
                    item.get(field)
                        .and_then(Value::as_str)
                        .map(ToString::to_string)
                })
                .collect(),
            _ => None,
        }
    }

    /// Whether this answer is ABOUT roads at all — whether a road id appears
    /// anywhere in it as a key rather than only as a value.
    ///
    /// This is what separates a SELECTOR from a COORDINATE, and it is derived
    /// rather than declared. `report-playthrough-manuscript --world` picks a
    /// road out of an answer that holds one entry per road; `report-frame-view
    /// --branch` takes the same vocabulary and means something else entirely —
    /// it moves the coordinate the whole answer is evaluated AT, so its
    /// `not_holding` count legitimately RISES on a road where fewer facts hold.
    /// Holding a coordinate to "the roads you keep say what they said" would be
    /// refuted by the shipped design on the first run, so this walk names the
    /// coordinate reads and counts them instead of judging them.
    fn is_about_roads(&self, value: &Value) -> bool {
        if self.keying(value).is_some() {
            return true;
        }
        match value {
            Value::Object(map) => map.values().any(|value| self.is_about_roads(value)),
            Value::Array(items) => items.iter().any(|value| self.is_about_roads(value)),
            _ => false,
        }
    }

    fn compare(&mut self, path: &str, all: &Value, one: &Value) {
        if let Some(keyed) = self.keying(all) {
            let (Some(before), Some(after)) =
                (self.roads_in(all, &keyed), self.roads_in(one, &keyed))
            else {
                self.moved.push(format!(
                    "{path}: the filtered read answers {}, which is not the road-keyed shape \
                     the unfiltered read used",
                    brief(one)
                ));
                return;
            };
            let seen: BTreeSet<&String> = before.iter().collect();
            let invented: Vec<&String> = after.iter().filter(|r| !seen.contains(r)).collect();
            if !invented.is_empty() {
                self.moved.push(format!(
                    "{path}: the filtered read answers roads the unfiltered one did not: \
                     {invented:?}"
                ));
            }
            // Only a structure holding MORE than one road can tell "narrowed"
            // from "whole" — a one-road structure answers both at once, and
            // recording it would put a verdict in the table no corpus supported.
            if seen.len() > 1 {
                let narrowed: Vec<&String> =
                    before.iter().filter(|road| *road == self.road).collect();
                if after == before {
                    self.shapes.insert(shape_path(path), Shape::Whole);
                } else if after.iter().collect::<Vec<_>>() == narrowed {
                    self.shapes.insert(shape_path(path), Shape::Narrowed);
                } else {
                    self.moved.push(format!(
                        "{path}: the filter kept {after:?}, which is neither the asked road nor \
                         the {} roads the unfiltered read answered",
                        seen.len()
                    ));
                    return;
                }
            }
            // The roads the filter KEPT must say what they said unfiltered.
            match (&keyed, all, one) {
                (Keyed::Map, Value::Object(all), Value::Object(one)) => {
                    for road in after.iter().filter(|road| seen.contains(road)) {
                        self.compare(&format!("{path}/{road}"), &all[road], &one[road]);
                    }
                }
                (Keyed::Records(field), Value::Array(all), Value::Array(one)) => {
                    let kept: Vec<&Value> = all
                        .iter()
                        .filter(|item| {
                            after
                                .iter()
                                .any(|road| item.get(field).and_then(Value::as_str) == Some(road))
                        })
                        .collect();
                    if kept.len() == one.len() {
                        for (index, (all, one)) in kept.into_iter().zip(one).enumerate() {
                            self.compare(&format!("{path}[{index}]"), all, one);
                        }
                    } else {
                        self.moved.push(format!(
                            "{path}: the filter kept {} of the {} records about the roads it \
                             answers",
                            one.len(),
                            kept.len()
                        ));
                    }
                }
                // `Ids` carries nothing but the road names, already compared.
                _ => {}
            }
            return;
        }
        match (all, one) {
            (Value::Object(all), Value::Object(one)) => {
                for key in all.keys().chain(one.keys()).collect::<BTreeSet<_>>() {
                    match (all.get(key), one.get(key)) {
                        (Some(all), Some(one)) => self.compare(&format!("{path}/{key}"), all, one),
                        (Some(_), None) => self
                            .moved
                            .push(format!("{path}/{key}: the filter dropped it")),
                        (None, _) => self
                            .moved
                            .push(format!("{path}/{key}: the filter added it")),
                    }
                }
            }
            (Value::Array(all), Value::Array(one)) if all.len() == one.len() => {
                for (index, (all, one)) in all.iter().zip(one).enumerate() {
                    self.compare(&format!("{path}[{index}]"), all, one);
                }
            }
            (Value::Array(all), Value::Array(one)) => self.moved.push(format!(
                "{path}: {} entries unfiltered, {} filtered",
                all.len(),
                one.len()
            )),
            (all, one) if all == one => {}
            (all, one) => self.moved.push(format!(
                "{path}: `{}` unfiltered, `{}` filtered",
                brief(all),
                brief(one)
            )),
        }
    }
}

#[test]
fn a_road_filter_answers_what_the_unfiltered_read_already_said() {
    let (stores, unloadable) = authored_stores();
    let asked = stores.len() + unloadable.len();

    // (verb, path) -> what the filter did to that structure, across corpora.
    let mut shapes: BTreeMap<(String, String), BTreeSet<Shape>> = BTreeMap::new();
    // The verbs a road filter demonstrably scopes SOMETHING on, and the ones a
    // multi-road corpus put in front of the walk without anything narrowing.
    let mut scoped: BTreeSet<String> = BTreeSet::new();
    let mut multi_road: BTreeSet<String> = BTreeSet::new();
    let mut moved: Vec<String> = Vec::new();
    let mut unprobed: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut probes = 0usize;
    let mut cells: BTreeSet<(String, String)> = BTreeSet::new();
    // The cells whose flag takes road ids but whose ANSWER is not about roads:
    // a coordinate, not a selector. An EARNED exclusion — named and counted, so
    // the census says what it could not decide (the R1029 rule).
    let mut coordinate: BTreeSet<(String, String)> = BTreeSet::new();
    // The fail-loud arm: how many (read, corpus) pairs were asked for a road no
    // registry holds, and which of them answered anyway.
    let mut loud = 0usize;
    let mut silent_typo: Vec<String> = Vec::new();

    for store in &stores {
        let ws = store.ws.path();
        let name = &store.name;
        let Ok(atomic) = AtomicStore::load(&ws.join(SIDECAR)) else {
            continue;
        };
        let roads = values_for("--world", &atomic);
        let road_set: BTreeSet<String> = roads.iter().cloned().collect();
        let usage_of = usage_lines(ws);
        for verb in advertised_reads(ws) {
            let Some(usage) = usage_of.get(&verb) else {
                *unprobed.entry("advertised with no usage line").or_default() += 1;
                continue;
            };
            let flags = flags_of(usage);
            // A ROAD FILTER is a flag whose value vocabulary IS the corpus's
            // road registry — derived from the shared resolver rather than
            // spelled here, so `--world` and `--branch` are found the same way
            // and a third road-taking flag joins this walk the run it ships.
            let filters: Vec<&common::Flag> = flags
                .iter()
                .filter(|flag| flag.takes_value && values_for(&flag.name, &atomic) == roads)
                .collect();
            if filters.is_empty() {
                continue;
            }
            let Some(base) = baseline_argv(&flags, &atomic) else {
                *unprobed
                    .entry("a required flag has no value this corpus declares")
                    .or_default() += 1;
                continue;
            };
            let ask = |extra: &[String]| -> Option<Value> {
                let mut argv: Vec<&str> = vec![verb.as_str()];
                argv.extend(base.iter().map(String::as_str));
                argv.extend(extra.iter().map(String::as_str));
                argv.push("--json");
                let out = run(ws, &argv);
                out.status
                    .success()
                    .then(|| serde_json::from_slice(&out.stdout).ok())
                    .flatten()
            };
            for filter in filters {
                if filter.required {
                    // The baseline already carries it, so there is no
                    // unfiltered answer to select FROM.
                    *unprobed
                        .entry("the road filter is required, so nothing is unfiltered")
                        .or_default() += 1;
                    continue;
                }
                cells.insert((verb.clone(), filter.name.clone()));
                let Some(unfiltered) = ask(&[]) else {
                    *unprobed
                        .entry("refuses or answers `--json` in prose")
                        .or_default() += 1;
                    continue;
                };
                let scan = Selection {
                    roads: &road_set,
                    road: "",
                    shapes: BTreeMap::new(),
                    moved: Vec::new(),
                };
                if !scan.is_about_roads(&unfiltered) {
                    coordinate.insert((verb.clone(), filter.name.clone()));
                    continue;
                }
                if roads.len() > 1 {
                    multi_road.insert(verb.clone());
                }
                // A road the registry does not hold. This is the ONE argument
                // this walk invents, and it is earned: the question is not what
                // the read answers but WHETHER it answers, and a filter that
                // takes an unregistered id and returns an empty answer is the
                // failure mode the sibling reads have guarded since R466 ("a
                // typo'd world must not read as an empty manuscript"). Round
                // 1049 added that guard to `report-timeline-gaps`, and a guard
                // no test reaches is a guard nobody knows is there.
                let unregistered = "no-such-road-in-any-registry";
                assert!(
                    !road_set.contains(unregistered),
                    "{name} registers the id this walk uses as its unregistered one"
                );
                if ask(&[filter.name.clone(), unregistered.to_string()]).is_some() {
                    silent_typo.push(format!("{name} `{verb} {} {unregistered}`", filter.name));
                }
                loud += 1;
                for road in &roads {
                    let Some(filtered) = ask(&[filter.name.clone(), road.clone()]) else {
                        *unprobed.entry("the read refuses that road").or_default() += 1;
                        continue;
                    };
                    probes += 1;
                    // The read's RECORD of the filter is not an answer about a
                    // road; comparing with it in would make every filtered read
                    // differ by construction (the R1048 circularity).
                    let record = record_of(&filtered, &unfiltered, Some(road));
                    let mut walk = Selection {
                        roads: &road_set,
                        road,
                        shapes: BTreeMap::new(),
                        moved: Vec::new(),
                    };
                    walk.compare(
                        "",
                        &substance(&unfiltered, &record),
                        &substance(&filtered, &record),
                    );
                    for (path, shape) in walk.shapes {
                        if shape == Shape::Narrowed {
                            scoped.insert(verb.clone());
                        }
                        shapes
                            .entry((verb.clone(), path))
                            .or_default()
                            .insert(shape);
                    }
                    for row in walk.moved {
                        moved.push(format!("{name} `{verb} {} {road}` {row}", filter.name));
                    }
                }
            }
        }
    }

    // Print BEFORE asserting (the R1026 lesson) — the table IS the finding.
    println!(
        "{asked} authored stores asked, {probes} (verb, road, corpus) probes over {} (verb, \
         flag) cells, {loud} of them also asked for a road no registry holds",
        cells.len()
    );
    for (why, count) in &unprobed {
        println!("  {count:5} probes not run: {why}");
    }
    for (verb, flag) in &coordinate {
        println!("  COORDINATE {verb} {flag} — the answer is not keyed by road");
    }
    let table: Vec<String> = shapes
        .iter()
        .map(|((verb, path), seen)| {
            format!(
                "{verb} {} {:?}",
                if path.is_empty() { "/" } else { path.as_str() },
                seen.iter().collect::<Vec<_>>()
            )
        })
        .collect();
    for row in &table {
        println!("  SCOPE {row}");
    }
    for verb in multi_road.difference(&scoped) {
        println!("  SCOPES NOTHING {verb}");
    }
    for row in &moved {
        println!("    MOVED {row}");
    }
    for row in &silent_typo {
        println!("    SILENT {row} answered instead of refusing");
    }

    let mut broken: Vec<String> = Vec::new();
    let mut check = |ok: bool, claim: &str| {
        if !ok {
            broken.push(claim.to_string());
        }
    };

    check(
        (asked, unloadable.len()) == (44, 16),
        "POPULATION (stores): every corpus an author shipped is asked, and the \
         ones the R857 rot closed are counted rather than dropped",
    );
    check(
        (cells.len(), coordinate.len(), probes) == (5, 1, 112),
        "POPULATION (cells): the reads the shipped usage lines say take a road \
         filter, the ones whose answer is a COORDINATE rather than a selection, \
         and the (read, road, corpus) probes the corpora could answer",
    );
    check(
        table
            == [
                // The fork tree is navigation context, and its topology is
                // inherently cross-world — the R556 doc comment has said so
                // since the read shipped, and until this table nothing ran it.
                "report-playable-world /fork_tree/branches [Whole]",
                "report-playable-world /worlds [Narrowed]",
                "report-playthrough-manuscript /worlds [Narrowed]",
                "report-quest-graph /fork_tree/branches [Whole]",
                // Reached through the quest graph's three road encodings: a
                // per-quest map, a per-quest list of records each about one
                // road, and the sorted key set spelled as a list of ids.
                "report-quest-graph /quests[]/locators [Narrowed]",
                "report-quest-graph /quests[]/per_world [Narrowed]",
                "report-quest-graph /worlds [Narrowed]",
                // Round 1049 — before this round `--world` scoped the PROSE
                // loop and the `--json` wire answered every road, so this row
                // read `[Whole]` and the read scoped nothing.
                "report-timeline-gaps /worlds [Narrowed]",
            ],
        "SHAPE: which structure of each answer the road filter scopes, and \
         which stays whole because its meaning is cross-world",
    );
    check(
        multi_road.difference(&scoped).count() == 0,
        "SCOPING: every read that advertises a road filter scopes something \
         with it on a corpus that has more than one road",
    );
    check(
        moved.is_empty(),
        "CONTRACT: a filtered read answers, for the roads it keeps, exactly \
         what the unfiltered read already said about them",
    );
    check(
        (loud, silent_typo.len()) == (76, 0),
        "FAIL-LOUD: every road filter was asked for a road no registry holds, \
         and refused — a typo'd road must not read as an answer with nothing in \
         it (the R466 rule, which `report-timeline-gaps` did not follow until \
         Round 1049 moved its filter into the projection)",
    );

    assert_eq!(
        broken,
        Vec::<String>::new(),
        "a road filter is doing something other than selecting"
    );
}
