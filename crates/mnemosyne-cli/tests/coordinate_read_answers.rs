//! A COORDINATE read answers at the LINEAGE of the road it is given. (R1050.)
//!
//! Round 1049 derived, from the shipped surface, which reads take a road and
//! then split them in two: a SELECTOR picks part of an answer that holds one
//! entry per road, and a COORDINATE moves the point the WHOLE answer is
//! evaluated at. It judged the selectors — "the roads you keep say what the
//! unfiltered read already said about them" — and could only NAME the
//! coordinate, because that rule is refuted by the shipped design on a
//! coordinate read (`report-frame-view --branch claim` legitimately answers
//! MORE facts than the unfiltered read does, and a different verdict about
//! each). It left the gap as its own carry: whether the coordinate answers
//! CORRECTLY at the road it is given "is a different contract, and no test in
//! this tree makes it".
//!
//! This is that contract. `report-frame-view` is not in the read-agreement
//! backlog either — the panel that derives that backlog can only ask reads it
//! can supply arguments for, and this one needs a frame AND a canon coordinate,
//! so it is one of the four the panel reports as unaskable. The single read
//! most obviously ABOUT the road axis is judged by nothing at all.
//!
//! WHAT A COORDINATE CAN HONESTLY BE HELD TO. A world-line's view is its own
//! history plus everything it inherited: "facts on ancestor branches up to each
//! fork point are part of this view; a fork's own revisions never leak back
//! into the ancestor's view" has been the R438 promise in the projection's doc
//! comment since it shipped. That is a claim about WHICH ROADS a view may draw
//! from, and it is checkable without recomputing the projection:
//!
//! - LINEAGE — every fact an answer at road `r` names sits on a branch the FORK
//!   TREE puts on `r`'s lineage. The oracle is a different shipped read
//!   (`report-fork-tree`), not a second copy of the membership lattice.
//! - BOUND — and it starts at a scene that road actually PLAYS, per the shipped
//!   manuscript. A fork inherits its parent only up to the fork point, and the
//!   scenes the parent plays afterwards are exactly the ones the fork's
//!   manuscript does not: the cut the fork tree cannot state, stated by a read.
//! - INHERITANCE — a forked road's answer actually NAMES facts from its
//!   ancestor, and the ancestor's answer never names the fork's. Counted, so
//!   "inherited history" is a measurement rather than a doc sentence.
//! - MOVES — asked at the SAME canon coordinate, a road answers something other
//!   than the default road does. Without this a read that ignored its flag
//!   would satisfy every other claim here, because the default road's view is
//!   on every road's lineage.
//! - DEPENDENCE — delete one fact an author wrote and the roads that cannot see
//!   its branch answer BYTE-IDENTICALLY, while every probe that named it moves.
//!   This is the half no static reading gives: an answer that lists the right
//!   facts while computing from the wrong ones passes LINEAGE and fails here.
//! - FAIL-LOUD — a road no registry holds is refused, not answered emptily (the
//!   R466 rule). The sibling walk skips this arm for coordinate cells.
//!
//! WHY TWO SHIPPED READS ARE THE ORACLE AND NOT A LOCAL LINEAGE WALK.
//! Re-deriving `world_membership` here would make the contract circular in the
//! R1041 way — two spellings of one derivation, agreeing because they are the
//! same idea twice. `report-fork-tree` and `report-playthrough-manuscript` are
//! SHIPPED reads with their own consumers, and between them they state both
//! halves of membership: the topology (which roads are related) and the cut
//! (where a road stops following its parent). Neither is asked to recompute the
//! projection; each is asked what it already publishes.
//!
//! THE POPULATION IS DERIVED. The coordinate cells come from the same shared
//! resolvers the selector walk uses, so a second coordinate read joins this
//! contract the run it ships. The other arguments each read requires are swept
//! over the corpus's own vocabulary, except one: a required flag whose
//! vocabulary IS the section registry is a CANON COORDINATE, and it is filled
//! with the points of the road being asked about: its END, where that road has
//! all of its history behind it, and every DIVERGENCE it plays through, where
//! it has exactly the prefix it shares with a neighbour. Both come from the
//! shipped reads; a coordinate off the road would answer `unknown` about nearly
//! everything, which measures B-1 honesty rather than the road axis.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use mnemosyne_atomic::AtomicStore;
use serde_json::Value;

mod common;
use common::{
    advertised_reads, answer_is_keyed_by_road, authored_stores, baseline_argv,
    corpus_workspace_try, flags_of, record_of, road_filters, road_lines, run, substance,
    usage_lines, values_for, Flag, SIDECAR,
};

/// Ask a read and hand back its JSON, or `None` when it refuses.
fn ask(ws: &Path, argv: &[String]) -> Option<Value> {
    let argv: Vec<&str> = argv.iter().map(String::as_str).collect();
    let out = run(ws, &argv);
    out.status
        .success()
        .then(|| serde_json::from_slice(&out.stdout).ok())
        .flatten()
}

/// Every road-line the shipped FORK TREE relates to each road: the road itself,
/// its fork ancestors, the parents of any confluence it is, and every
/// confluence it flows into — closed transitively.
///
/// The two directions are both real. Backwards is inheritance (a fork carries
/// its parent's history to the fork point); forwards is the merge (a branch
/// that rejoins shares the confluence's continuation, so facts authored ON the
/// confluence are part of that branch's world-line too).
///
/// Also returns any disagreement between the tree's two encodings of a merge —
/// `converges` on the confluence and `rejoins` on the branch, the second
/// derived from the first by inversion (R836). This walk reads the merge edge,
/// so an inversion that ever stopped matching would quietly change what
/// "lineage" means here.
struct Topology {
    /// Road -> every road-line the tree relates to it.
    lineage: BTreeMap<String, BTreeSet<String>>,
    /// Every coordinate at which some road leaves or rejoins another — the
    /// canon points where a world-line's history is decided. A road's own line
    /// carries the ones it plays through, and those are the second place this
    /// walk asks each read: at the END a road has all its history behind it, at
    /// a DIVERGENCE it has exactly the shared prefix, and the two are different
    /// questions about the same lineage.
    divergences: BTreeSet<String>,
    /// Where the tree's two encodings of one merge edge do not match.
    disagreements: Vec<String>,
}

fn fork_tree_topology(ws: &Path, roads: &[String]) -> Topology {
    let tree = ask(ws, &["report-fork-tree".into(), "--json".into()])
        .expect("the fork tree is a read every corpus can answer");
    let branches = tree["branches"].as_array().cloned().unwrap_or_default();

    // branch -> the roads it inherits from directly.
    let mut parents: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    // road -> the confluences that converge FROM it.
    let mut forward: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    // The merge edges as the confluence states them, and as the branch does.
    let mut stated: BTreeSet<(String, String, String)> = BTreeSet::new();
    let mut inverted: BTreeSet<(String, String, String)> = BTreeSet::new();
    let mut divergences: BTreeSet<String> = BTreeSet::new();
    for branch in &branches {
        let id = branch["branch_id"].as_str().unwrap_or_default().to_string();
        if let Some(fork) = branch.get("fork").and_then(Value::as_object) {
            let parent = fork["parent"].as_str().unwrap_or_default().to_string();
            parents.entry(id.clone()).or_default().insert(parent);
            if let Some(at) = fork["at"].as_str() {
                divergences.insert(at.to_string());
            }
        }
        for edge in branch
            .get("converges")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let parent = edge["parent"].as_str().unwrap_or_default().to_string();
            let at = edge["at"].as_str().unwrap_or_default().to_string();
            parents
                .entry(id.clone())
                .or_default()
                .insert(parent.clone());
            forward
                .entry(parent.clone())
                .or_default()
                .insert(id.clone());
            divergences.insert(at.clone());
            stated.insert((parent, id.clone(), at));
        }
        for edge in branch
            .get("rejoins")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            inverted.insert((
                id.clone(),
                edge["into"].as_str().unwrap_or_default().to_string(),
                edge["at"].as_str().unwrap_or_default().to_string(),
            ));
        }
    }

    let mut lineage: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for road in roads {
        let mut seen: BTreeSet<String> = BTreeSet::new();
        let mut frontier = vec![road.clone()];
        while let Some(cur) = frontier.pop() {
            if !seen.insert(cur.clone()) {
                continue;
            }
            for next in parents
                .get(&cur)
                .into_iter()
                .flatten()
                .chain(forward.get(&cur).into_iter().flatten())
            {
                frontier.push(next.clone());
            }
        }
        lineage.insert(road.clone(), seen);
    }

    let mut disagreements = Vec::new();
    for edge in stated.symmetric_difference(&inverted) {
        disagreements.push(format!("{edge:?}"));
    }
    Topology {
        lineage,
        divergences,
        disagreements,
    }
}

/// Every registered fact id this answer names, anywhere in it.
///
/// Derived rather than read per verb: a coordinate read's subjects are whatever
/// facts it mentions, and a walk that knew `holding[].fact_id` by name would be
/// blind to the next read's spelling (the R1046 lesson).
fn subjects(value: &Value, facts: &BTreeMap<String, String>, out: &mut BTreeSet<String>) {
    match value {
        Value::String(text) => {
            if facts.contains_key(text) {
                out.insert(text.clone());
            }
        }
        Value::Object(map) => {
            for value in map.values() {
                subjects(value, facts, out);
            }
        }
        Value::Array(items) => {
            for value in items {
                subjects(value, facts, out);
            }
        }
        _ => {}
    }
}

/// One fact per branch that the manifest can lose: the first id on that branch
/// that nothing else in the manifest refers to.
///
/// Derived from the manifest, because that is what a deletion edits. A fact
/// another fact pays off, or a disclosure plan discloses, cannot simply go —
/// the import would refuse it, and a refused edit is not a move an author could
/// have made (the R1033 rule). Branches with no such fact are returned by their
/// absence and counted by the caller.
fn deletable_per_branch(
    facts_json: &Value,
    branch_of: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let entries = facts_json["facts"].as_array().cloned().unwrap_or_default();
    let ids: BTreeSet<String> = entries
        .iter()
        .filter_map(|f| f["fact_id"].as_str().map(ToString::to_string))
        .collect();

    // Every id mentioned by something OTHER than the fact's own entry — the
    // manifest's side tables included, since a plan naming a fact is a
    // reference the write path checks.
    let mut referenced: BTreeSet<String> = BTreeSet::new();
    let mut mentions = |value: &Value, own: Option<&str>| {
        let mut seen = BTreeSet::new();
        subjects_of_ids(value, &ids, &mut seen);
        for id in seen {
            if Some(id.as_str()) != own {
                referenced.insert(id);
            }
        }
    };
    for entry in &entries {
        let own = entry["fact_id"].as_str();
        mentions(entry, own);
    }
    if let Some(map) = facts_json.as_object() {
        for (key, value) in map {
            if key != "facts" {
                mentions(value, None);
            }
        }
    }

    let mut out: BTreeMap<String, String> = BTreeMap::new();
    for entry in &entries {
        let Some(id) = entry["fact_id"].as_str() else {
            continue;
        };
        if referenced.contains(id) {
            continue;
        }
        let Some(branch) = branch_of.get(id) else {
            continue;
        };
        out.entry(branch.clone()).or_insert_with(|| id.to_string());
    }
    out
}

/// [`subjects`] over a bare id set — the manifest half, where the ids come from
/// the manifest itself rather than from a loaded store.
fn subjects_of_ids(value: &Value, ids: &BTreeSet<String>, out: &mut BTreeSet<String>) {
    match value {
        Value::String(text) => {
            if ids.contains(text) {
                out.insert(text.clone());
            }
        }
        Value::Object(map) => {
            for value in map.values() {
                subjects_of_ids(value, ids, out);
            }
        }
        Value::Array(items) => {
            for value in items {
                subjects_of_ids(value, ids, out);
            }
        }
        _ => {}
    }
}

/// One probe of a coordinate read: the argv apart from the road, and how it
/// reads in a message.
#[derive(Clone)]
struct Combo {
    /// The required arguments other than the canon coordinate.
    args: Vec<String>,
    /// The canon-coordinate flags, filled in per probe with the point on the
    /// road being asked about.
    coordinates: Vec<String>,
    label: String,
}

/// The probe grid for one read on one corpus: every combination of the required
/// arguments the corpus can supply, with any section-valued flag left to be
/// filled in per road.
fn grid(flags: &[Flag], filter: &Flag, store: &AtomicStore) -> Option<Vec<Combo>> {
    let sections = values_for("--at", store);
    let mut combos = vec![Combo {
        args: Vec::new(),
        coordinates: Vec::new(),
        label: String::new(),
    }];
    for flag in flags.iter().filter(|f| f.required && f.name != filter.name) {
        if !flag.takes_value {
            for combo in &mut combos {
                combo.args.push(flag.name.clone());
            }
            continue;
        }
        let values = values_for(&flag.name, store);
        if values.is_empty() {
            return None;
        }
        // A required flag whose vocabulary IS the section registry is a CANON
        // COORDINATE: the point in the story the read is asked about. It is not
        // swept over every section — the caller fills it with a point on the
        // road being asked about, which is a different question per road.
        if values == sections {
            for combo in &mut combos {
                combo.coordinates.push(flag.name.clone());
            }
            continue;
        }
        combos = combos
            .iter()
            .flat_map(|combo| {
                values.iter().map(|value| {
                    let mut next = combo.clone();
                    next.args.push(flag.name.clone());
                    next.args.push(value.clone());
                    next.label = format!("{} {} {value}", next.label, flag.name);
                    next
                })
            })
            .collect();
    }
    Some(combos)
}

impl Combo {
    /// This probe's argv on one road at one canon point.
    fn argv(&self, verb: &str, filter: &str, road: &str, end: &str) -> Vec<String> {
        let mut argv = vec![verb.to_string()];
        argv.extend(self.args.iter().cloned());
        for flag in &self.coordinates {
            argv.push(flag.clone());
            argv.push(end.to_string());
        }
        argv.push(filter.to_string());
        argv.push(road.to_string());
        argv.push("--json".to_string());
        argv
    }
}

#[test]
fn a_coordinate_read_answers_at_the_lineage_of_the_road_it_is_given() {
    let (stores, unloadable) = authored_stores();
    let asked = stores.len() + unloadable.len();

    let mut cells: BTreeSet<(String, String)> = BTreeSet::new();
    let mut probes = 0usize;
    let mut unprobed: BTreeMap<&'static str, usize> = BTreeMap::new();
    // The (verb, corpus) cells this contract hands to the sibling walk because
    // their answer IS keyed by road — counted here so the split between the two
    // contracts is a measurement, not an unexplained absence.
    let mut selectors: BTreeMap<String, usize> = BTreeMap::new();
    // LINEAGE — a named fact whose branch is nowhere on the asked road's
    // lineage, and the inheritance the same walk measures.
    let mut off_lineage: Vec<String> = Vec::new();
    let mut inherited: BTreeSet<(String, String)> = BTreeSet::new();
    let mut inheriting_probes = 0usize;
    // BOUND — named facts that start on a scene the road plays, and the ones
    // that start somewhere it does not.
    let mut on_road = 0usize;
    let mut off_road: Vec<String> = Vec::new();
    let mut bound_excludes = 0usize;
    // Divergence coordinates that are not on a given road's line — counted so
    // the grid says which points it did not ask each road at.
    let mut off_line_points: BTreeMap<String, usize> = BTreeMap::new();
    // Every (road, canon point) the grid asked at.
    let mut points_asked: BTreeSet<(String, String, String)> = BTreeSet::new();
    // MOVES — the (corpus, road) pairs where asking AT the road answers
    // something other than asking at the default road for the same canon
    // coordinate, and every pair that was compared at all. A road must move at
    // SOME point, not at every one: at the coordinate where it diverges it has
    // exactly the prefix it shares with its parent, so answering the same there
    // is the contract working rather than a filter doing nothing.
    let mut moves: BTreeSet<(String, String)> = BTreeSet::new();
    let mut compared: BTreeSet<(String, String)> = BTreeSet::new();
    // FAIL-LOUD.
    let mut loud = 0usize;
    let mut silent_typo: Vec<String> = Vec::new();
    // DEPENDENCE.
    let mut deletions = 0usize;
    let mut refused_deletions: Vec<String> = Vec::new();
    let mut branches_without_a_free_fact: Vec<String> = Vec::new();
    let mut must_not_move = 0usize;
    let mut must_move = 0usize;
    let mut either = 0usize;
    let mut dependence: Vec<String> = Vec::new();
    // The fork tree's two spellings of a merge.
    let mut merge_disagreements: Vec<String> = Vec::new();

    for store in &stores {
        let ws = store.ws.path();
        let name = &store.name;
        let Ok(atomic) = AtomicStore::load(&ws.join(SIDECAR)) else {
            *unprobed.entry("the built store does not load").or_default() += 1;
            continue;
        };
        let roads = values_for("--world", &atomic);
        let road_set: BTreeSet<String> = roads.iter().cloned().collect();
        let branch_of: BTreeMap<String, String> = atomic
            .narrative_facts
            .iter()
            .map(|(id, fact)| (id.to_string(), fact.branch.to_string()))
            .collect();
        // Where each fact STARTS — the coordinate the departure bound is about.
        let starts_at: BTreeMap<String, String> = atomic
            .narrative_facts
            .iter()
            .map(|(id, fact)| (id.to_string(), fact.canon_from.to_string()))
            .collect();
        let usage_of = usage_lines(ws);

        for verb in advertised_reads(ws) {
            let Some(usage) = usage_of.get(&verb) else {
                continue;
            };
            let flags = flags_of(usage);
            for filter in road_filters(&flags, &atomic) {
                if filter.required {
                    continue;
                }
                let Some(base) = baseline_argv(&flags, &atomic, ws) else {
                    *unprobed
                        .entry("a required argument has no value this corpus declares")
                        .or_default() += 1;
                    continue;
                };
                let mut bare = vec![verb.clone()];
                bare.extend(base.iter().cloned());
                bare.push("--json".to_string());
                let Some(unfiltered) = ask(ws, &bare) else {
                    *unprobed
                        .entry("the read refuses or answers `--json` in prose")
                        .or_default() += 1;
                    continue;
                };
                // A SELECTOR is the sibling walk's business; this contract is
                // for the reads whose answer is not keyed by road at all.
                if answer_is_keyed_by_road(&unfiltered, &road_set) {
                    *selectors.entry(verb.clone()).or_default() += 1;
                    continue;
                }
                let Some(combos) = grid(&flags, filter, &atomic) else {
                    *unprobed
                        .entry("a required argument has no value this corpus declares")
                        .or_default() += 1;
                    continue;
                };
                cells.insert((verb.clone(), filter.name.clone()));

                let topology = fork_tree_topology(ws, &roads);
                let lineage = &topology.lineage;
                for row in &topology.disagreements {
                    merge_disagreements.push(format!("{name} {row}"));
                }
                let lines = road_lines(ws);

                // A road no registry holds must be REFUSED, not answered with
                // an empty view (the R466 rule). The sibling walk skips this
                // arm for a coordinate cell, so this is the only place the
                // shipped surface is asked it.
                let unregistered = "no-such-road-in-any-registry";
                assert!(
                    !road_set.contains(unregistered),
                    "{name} registers the id this walk uses as its unregistered one"
                );
                if let (Some(combo), Some(line)) = (combos.first(), lines.values().next()) {
                    loud += 1;
                    if ask(
                        ws,
                        &combo.argv(&verb, &filter.name, unregistered, &line.end),
                    )
                    .is_some()
                    {
                        silent_typo.push(format!("{name} `{verb} {} {unregistered}`", filter.name));
                    }
                }

                // The baseline grid: every road, at its END and at every
                // DIVERGENCE it plays through.
                let mut answers: BTreeMap<(String, String, String), Value> = BTreeMap::new();
                let mut named: BTreeMap<(String, String, String), BTreeSet<String>> =
                    BTreeMap::new();
                for road in &roads {
                    let Some(line) = lines.get(road) else {
                        *unprobed
                            .entry("the corpus's manuscript gives that road no scene")
                            .or_default() += 1;
                        continue;
                    };
                    // Where a road stops following its parent is the OTHER
                    // interesting point on it: at the end a view has all of the
                    // road's history behind it, at a divergence it has exactly
                    // the prefix the two roads share. A coordinate read that was
                    // right about the end and wrong about the shared prefix
                    // would pass a grid of ends alone.
                    let mut points: BTreeSet<&String> = BTreeSet::new();
                    points.insert(&line.end);
                    for at in &topology.divergences {
                        if line.scenes.contains(at) {
                            points.insert(at);
                        } else {
                            *off_line_points.entry(road.clone()).or_default() += 1;
                        }
                    }
                    // What the BOUND rule has to EXCLUDE on this road: a fact
                    // on a branch this road inherits from, which starts at a
                    // scene the road does not play — the parent's history past
                    // the fork point. Every frame is swept below, so a read
                    // that leaked one would name it in that frame's probe and
                    // the off-road row would catch it. Counted so "no fact
                    // starts off its road" is known to be a cut the corpora
                    // actually make, rather than a rule nothing tests.
                    let mine = lineage.get(road).cloned().unwrap_or_default();
                    bound_excludes += branch_of
                        .iter()
                        .filter(|(fact, branch)| {
                            mine.contains(*branch) && !line.scenes.contains(&starts_at[*fact])
                        })
                        .count();
                    for (point, combo) in points
                        .iter()
                        .flat_map(|point| combos.iter().map(move |combo| (*point, combo)))
                    {
                        let argv = combo.argv(&verb, &filter.name, road, point);
                        let Some(answer) = ask(ws, &argv) else {
                            *unprobed
                                .entry("the read refuses that coordinate")
                                .or_default() += 1;
                            continue;
                        };
                        probes += 1;
                        let mut said: BTreeSet<String> = BTreeSet::new();
                        subjects(&answer, &branch_of, &mut said);
                        let mut inherits = false;
                        for fact in &said {
                            let branch = &branch_of[fact];
                            if !mine.contains(branch) {
                                off_lineage.push(format!(
                                    "{name} `{verb}{} {} {road} at {point}` names {fact}, \
                                     authored on `{branch}`, which the fork tree does not put on \
                                     this road's lineage {mine:?}",
                                    combo.label, filter.name
                                ));
                                continue;
                            }
                            if branch != road {
                                inherited.insert((road.clone(), branch.clone()));
                                inherits = true;
                            }
                            // BOUND — a fork inherits its parent's history only
                            // up to the fork point, and the scenes the parent
                            // plays afterwards are exactly the ones this road's
                            // manuscript does not. So a fact that STARTS off
                            // this road is a fact past a departure bound, which
                            // the fork tree alone cannot see (it states the
                            // topology, not the cut).
                            if line.scenes.contains(&starts_at[fact]) {
                                on_road += 1;
                            } else {
                                off_road.push(format!(
                                    "{name} `{verb}{} {} {road} at {point}` names {fact}, which \
                                     starts at `{}` — a scene this road's manuscript does not play",
                                    combo.label, filter.name, starts_at[fact]
                                ));
                            }
                        }
                        inheriting_probes += usize::from(inherits);
                        // MOVES — the road axis on its own. Asking the SAME
                        // canon coordinate on the default road isolates the
                        // flag from the point in the story: a coordinate read
                        // that quietly ignored its road would answer both
                        // identically, and every count in this walk would still
                        // add up, because "the whole store's default view"
                        // satisfies the lineage rule on every road (main is on
                        // everyone's lineage). The differential is the only
                        // thing that says the flag is load-bearing.
                        if road != mnemosyne_core::MAIN_BRANCH {
                            let default =
                                combo.argv(&verb, &filter.name, mnemosyne_core::MAIN_BRANCH, point);
                            if let Some(default) = ask(ws, &default) {
                                probes += 1;
                                compared.insert((name.clone(), road.clone()));
                                let record = record_of(&answer, &default, Some(road));
                                if substance(&answer, &record) != substance(&default, &record) {
                                    moves.insert((name.clone(), road.clone()));
                                }
                            }
                        }
                        points_asked.insert((name.clone(), road.clone(), point.clone()));
                        let key = (road.clone(), point.clone(), combo.label.clone());
                        answers.insert(key.clone(), answer);
                        named.insert(key, said);
                    }
                }

                // DEPENDENCE: delete one fact per branch and watch which roads
                // move. A road that cannot see the branch must answer byte for
                // byte what it answered before; a probe that NAMED the deleted
                // fact must not.
                let free = deletable_per_branch(&store.facts, &branch_of);
                let carrying: BTreeSet<&String> = branch_of.values().collect();
                for branch in carrying {
                    let Some(fact) = free.get(branch) else {
                        branches_without_a_free_fact.push(format!(
                            "{name} `{branch}` (every fact on it is referenced)"
                        ));
                        continue;
                    };
                    let mut mutated = store.facts.clone();
                    let entries = mutated["facts"].as_array_mut().expect("facts array");
                    entries.retain(|entry| entry["fact_id"].as_str() != Some(fact.as_str()));
                    let tmp = match corpus_workspace_try(&store.dir, &mutated) {
                        Ok(tmp) => tmp,
                        Err(reason) => {
                            refused_deletions.push(format!("{name} {fact}: {reason}"));
                            continue;
                        }
                    };
                    deletions += 1;
                    for ((road, point, label), before) in &answers {
                        let combo = combos
                            .iter()
                            .find(|c| c.label == *label)
                            .expect("the grid holds the combo it probed with");
                        let argv = combo.argv(&verb, &filter.name, road, point);
                        let after = ask(tmp.path(), &argv);
                        let moved = after.as_ref() != Some(before);
                        let sees = lineage.get(road).is_some_and(|l| l.contains(branch));
                        let was_named =
                            named[&(road.clone(), point.clone(), label.clone())].contains(fact);
                        if !sees {
                            must_not_move += 1;
                            if moved {
                                dependence.push(format!(
                                    "{name} `{verb}{label} {} {road} at {point}` moved when \
                                     {fact} was deleted from `{branch}`, a branch the fork tree \
                                     does not put on this road's lineage",
                                    filter.name
                                ));
                            }
                        } else if was_named {
                            must_move += 1;
                            if !moved {
                                dependence.push(format!(
                                    "{name} `{verb}{label} {} {road} at {point}` named {fact} and \
                                     did not move when it was deleted",
                                    filter.name
                                ));
                            }
                        } else {
                            either += 1;
                        }
                    }
                }
            }
        }
    }

    // Print BEFORE asserting (the R1026 lesson) — the table IS the finding.
    println!(
        "{asked} authored stores asked, {probes} (read, road, argument) probes over {} coordinate \
         (verb, flag) cell(s)",
        cells.len()
    );
    for (verb, flag) in &cells {
        println!("  COORDINATE {verb} {flag}");
    }
    for (why, count) in &unprobed {
        println!("  {count:5} (read, corpus) cell(s) not probed: {why}");
    }
    for (verb, count) in &selectors {
        println!("  {count:5} SELECTOR cell(s) left to the sibling walk: {verb}");
    }
    println!(
        "  INHERITANCE {inheriting_probes} probe(s) name a fact from another road; {} (road, \
         ancestor) pair(s):",
        inherited.len()
    );
    for (road, branch) in &inherited {
        println!("    {road} <- {branch}");
    }
    println!(
        "  DEPENDENCE {deletions} deletion(s), {} refused by the write path, {} branch(es) with \
         no free fact: {must_not_move} probe(s) must not move, {must_move} must, {either} may",
        refused_deletions.len(),
        branches_without_a_free_fact.len()
    );
    for row in &refused_deletions {
        println!("    REFUSED {row}");
    }
    for row in &branches_without_a_free_fact {
        println!("    NO FREE FACT {row}");
    }
    let still: Vec<&(String, String)> = compared.difference(&moves).collect();
    println!(
        "  MOVES {} of {} compared (corpus, road) pair(s) answer something other than the default \
         road at some canon coordinate they share:",
        moves.len(),
        compared.len()
    );
    for (name, road) in &still {
        println!("    STILL {name} `{road}` answers the default road's answer at every point");
    }
    println!("  FAIL-LOUD {loud} unregistered-road probe(s)");
    println!(
        "  BOUND {on_road} named fact(s) start on a scene their road plays, {} start off it; the \
         cut has {bound_excludes} (road, fact) pair(s) to exclude",
        off_road.len()
    );
    println!(
        "  POINTS {} (corpus, road, coordinate) asked — each road at its end and at every \
         divergence it plays; {} divergence(s) skipped as off some road's line",
        points_asked.len(),
        off_line_points.values().sum::<usize>()
    );
    for row in &off_lineage {
        println!("    OFF-LINEAGE {row}");
    }
    for row in off_road.iter().take(20) {
        println!("    OFF-ROAD {row}");
    }
    for row in &dependence {
        println!("    DEPENDENCE {row}");
    }
    for row in &silent_typo {
        println!("    SILENT {row} answered instead of refusing");
    }
    for row in &merge_disagreements {
        println!("    MERGE {row}");
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
        (
            cells.len(),
            probes,
            unprobed.values().sum::<usize>(),
            selectors.values().sum::<usize>(),
        ) == (1, 393, 36, 76),
        "POPULATION (cells): the reads whose road flag moves the coordinate the \
         whole answer is evaluated at, the (read, road, argument) probes the \
         corpora could answer, and the road-taking cells this contract does not \
         judge — 36 whose read needs an argument the corpus does not declare, 76 \
         SELECTORS the sibling walk owns",
    );
    check(
        off_lineage.is_empty(),
        "LINEAGE: a coordinate read names only facts authored on roads the \
         shipped fork tree puts on the lineage of the road it was given — a \
         sibling's facts never leak in, and a fork's revisions never leak back \
         into its ancestor's view (the R438 promise)",
    );
    check(
        (inherited.len(), inheriting_probes) == (9, 128),
        "INHERITANCE: forked roads actually DRAW on their ancestors, so the \
         lineage claim is measured rather than vacuously true of reads that \
         name nothing. Round 1054 raised the probe count: the frame view's \
         residual arm began naming the facts it had only counted, so four more \
         probes name a fact authored on an ancestor road",
    );
    check(
        (
            deletions,
            refused_deletions.len(),
            branches_without_a_free_fact.len(),
        ) == (39, 0, 2),
        "DEPENDENCE (perturbations): one authorable deletion per branch that \
         carries a fact, with the branches whose every fact is referenced \
         counted by name rather than skipped",
    );
    check(
        (must_not_move, must_move, either) == (240, 88, 299),
        "DEPENDENCE (population): every deletion is classified by what the fork \
         tree and the baseline answer already said — the road cannot see that \
         branch, the probe named that fact, or neither decides. Round 1054 moved \
         thirteen probes out of `may` and into `must`: the frame view's residual \
         arm names its facts now, so a probe that had merely COUNTED a deleted \
         fact is one this contract can hold to moving",
    );
    check(
        dependence.is_empty(),
        "DEPENDENCE: deleting a fact leaves every road that cannot see its \
         branch byte-identical, and moves every probe that named it — an answer \
         computed from a lineage other than the one it reports fails here even \
         when it lists the right facts",
    );
    check(
        (points_asked.len(), off_line_points.values().sum::<usize>()) == (64, 0),
        "POINTS: every road is asked at its END and at every DIVERGENCE it \
         plays through, so a read that was right about a road's whole history \
         and wrong about the prefix it shares with its parent is still judged",
    );
    check(
        (on_road, off_road.len(), bound_excludes) == (5520, 0, 136),
        "BOUND: every fact a coordinate read names starts at a scene the road \
         it was asked about actually plays — the departure cut, which the fork \
         tree cannot state (it gives the topology, not where a road stops \
         following its parent). The corpora put 136 (road, fact) pairs on the \
         far side of that cut, so this is a rule they exercise. A non-zero \
         off-road count is not automatically a defect: a start the declared \
         order cannot COMPARE to the bound is honestly `unknown` rather than \
         absent (B-1), and this row would then have to split by the read's \
         undecidable arm. Today no corpus produces one. Round 1054 took this \
         from 3988 to 5520 namings without moving the off-road count: the frame \
         view's residual arm began naming the facts it counted, and every one of \
         them lands inside the same cut",
    );
    check(
        (compared.len(), still.len()) == (13, 0),
        "MOVES: every registered road answers something other than the default \
         road does at SOME canon coordinate they share — the flag is \
         load-bearing. A read that ignored it would satisfy every other claim \
         here, because the default road's view is on every road's lineage. At a \
         road's DIVERGENCE the two legitimately agree, which is why this is a \
         claim about the road and not about each point",
    );
    check(
        (loud, silent_typo.len()) == (28, 0),
        "FAIL-LOUD: a coordinate read asked for a road no registry holds \
         refuses — the R466 rule, which the sibling selector walk cannot ask on \
         a coordinate cell",
    );
    check(
        merge_disagreements.is_empty(),
        "MERGE: the fork tree's two spellings of one confluence edge agree, so \
         the lineage this contract closes over is the same either way it is read",
    );

    assert_eq!(
        broken,
        Vec::<String>::new(),
        "a coordinate read is answering at something other than the lineage of \
         the road it was given"
    );
}
