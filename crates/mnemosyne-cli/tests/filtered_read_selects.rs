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
    advertised_reads, answer_is_keyed_by_road, authored_stores, baseline_argv, flags_of, record_of,
    road_filters, road_keying, roads_in, run, substance, usage_lines, values_for, Keyed, SIDECAR,
};

/// What one road-keyed structure did under the filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Shape {
    /// The filter narrowed it to the asked road.
    Narrowed,
    /// It stayed whole — a structure whose meaning is cross-world.
    Whole,
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
    fn compare(&mut self, path: &str, all: &Value, one: &Value) {
        if let Some(keyed) = road_keying(all, self.roads) {
            let (Some(before), Some(after)) = (roads_in(all, &keyed), roads_in(one, &keyed)) else {
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
            let filters = road_filters(&flags, &atomic);
            if filters.is_empty() {
                continue;
            }
            let Some(base) = baseline_argv(&flags, &atomic, ws) else {
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
                // A COORDINATE, not a selection — judged by the sibling walk
                // (`coordinate_read_answers.rs`), which holds it to the lineage
                // of the road it was given rather than to "the roads you keep
                // say what they said".
                if !answer_is_keyed_by_road(&unfiltered, &road_set) {
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
                // Round 1061 — the walk each locator's `scene_ordinal` indexes,
                // added so the number is openable from the answer that prints
                // it. A fourth road encoding, and this table judged it without
                // being told: it scopes like the other three, and the CONTRACT
                // check below says the roads it keeps say what they said.
                "report-quest-graph /roads [Narrowed]",
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
