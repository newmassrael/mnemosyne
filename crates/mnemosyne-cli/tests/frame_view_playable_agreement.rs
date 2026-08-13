//! The playable world hands a runtime a WALK; the frame view answers, at one
//! point of it, WHOSE VIEW HOLDS WHAT. (Round 1138.)
//!
//! The ninth declared cross-read agreement, taken from the top of the Round 1088
//! backlog: `report-frame-view <-> report-playable-world`, 135 subjects in
//! common — and 135 is also every subject the frame view answers about at all,
//! so this pair is the whole of one read's population inside the other's. Rounds
//! 1041-1045 established why agreement is DECLARED rather than derived (five
//! derivations over read output failed to decide it, the fifth refuted by
//! injection), and Round 1050 established what makes a declaration worth
//! writing: the oracle must be ANOTHER SHIPPED READ, not this file re-deriving
//! the answer.
//!
//! THE SHARED QUESTION. Both reads stand on `WorldCtx::holds_at` — the one
//! predicate the continuity gate uses — and they publish opposite halves of it:
//!
//! - the playable world is keyed by ROAD and SCENE. Per scene it names the facts
//!   that BEGIN there and the ones that END there (with the kind that says when),
//!   and reduces the state itself to a NUMBER, `holding_count`. It knows nothing
//!   about frames except the one it stamps on each event.
//! - the frame view is keyed by FRAME, ROAD and POINT. It NAMES the facts of one
//!   frame holding at one coordinate, the ones that definitively do not, and the
//!   ones the declared order cannot decide — and it has no notion of a telling
//!   at all.
//!
//! So no field here equals a field there, and every law below is a JOIN. The
//! frame view is the only read that can say WHOSE view holds a fact, and the
//! playable world is the only read that says what the runtime is HANDED at that
//! scene; the two meet exactly where a play break lives.
//!
//! THE LAWS, each with its own evidence:
//!
//! - HOLDING — at every scene of every road, the facts the frame views NAME as
//!   holding, unioned over every registered frame, are exactly the facts the
//!   road's own events replay ([`common::holding_replay`], the resolver Round
//!   1056 proved the count is a function of). Two readers of one predicate, one
//!   arriving through a walk of events and the other through the partial order
//!   and the successor index. The COUNT is asserted beside the set, and the set
//!   is the part a count cannot state (the R1053 shape).
//! - ONE FRAME — no fact is named holding under two frames. The view's whole
//!   claim is that it never mixes them.
//! - POPULATION — for a road, the frame views' populations (holding plus
//!   not-holding plus undecidable, unioned over frames) are exactly the facts the
//!   playable world accounts for on that road: begun at some scene, or named
//!   unplaced, or called undecidable. And each frame's population does not move
//!   with the POINT — which is what makes it a property of (frame, road) and lets
//!   this be asked once per pair rather than once per scene.
//! - BEGUN — a fact holding at a scene began at or before it on that road, per
//!   the walk the other read publishes.
//! - ORDER — the other half of HOLDING, and the one the confluence fragment
//!   forced. A replay of the events is a SEQUENCE reading and `holds_at` is an
//!   ORDER reading; the walk is ONE linearization of a partial order, and the
//!   manuscript names every adjacent pair it could not compare
//!   (`undeclared_adjacencies`). Across such a gap the replay says a fact has
//!   begun and the declaration cannot decide it — so the two READS agree with
//!   each other (the count sides with the order), the difference is the replay's,
//!   and every member of it is explained by a gap the same answer names. Round
//!   1056 proved the count is a function of the events over the authored corpora,
//!   where no walk has such a gap; this is the qualification that claim needed
//!   and could not have found there.
//! - EXPIRED / SUPERSEDED — the two end kinds stop at different times, and the
//!   wire says so in prose: `Expired` = "the fact still holds AT it, through it —
//!   this is its last scene"; `Superseded` = "the replaced fact no longer holds
//!   FROM it". Judged from the frame view for the first time: expired at a scene
//!   holds there and is gone from the next, superseded at a scene is already gone
//!   there. A fact that BEGINS and is superseded at one scene is the arm where
//!   the two orders of that sentence come apart.
//! - UNDECIDABLE — a fact the road cannot place is not merely absent from the
//!   holding set, it is `unknown`: B-1 honesty, and the two reads reach it by
//!   different predicates. The manuscript asks whether a coordinate is a NODE of
//!   this world's composed order; the frame view asks whether it is COMPARABLE to
//!   the point. A fact unplaced by `canon_from` and a fact of undecidable world
//!   visibility are unknown at every scene; one unplaced by `canon_to` is unknown
//!   from the scene it begins at, and definitively not holding before it.
//! - FRAGMENT — a confluence is a prefix-less FRAGMENT, and three surfaces say
//!   so from the one shared `is_confluence`: the fork tree's `converges`, the
//!   manuscript's `confluence_fragment`, and the frame view's. A road the
//!   unfiltered dump omits is asked with `--world` (the R1049 rule: a filtered
//!   read is the one a runtime makes).
//! - SEAT — the law no other pair can state. The playable world hands the
//!   runtime a LOCATOR: a pointer at a fact, seated at a scene, resolved under
//!   the telling's disclosure plan. The frame view says whether that fact is true
//!   there. Three arms, all measured: a seat at or after the scene the fact
//!   begins HOLDS it; a seat before that scene does not, which is R949's
//!   disclosure-before-truth class judged from the epistemic side; and a seat off
//!   this road's walk is a scene where the story cannot decide the fact at all —
//!   the runtime pointed at a coordinate that answers `unknown`.
//!
//! WHY THIS PAIR IS NOT THE CONTRACTS BESIDE IT COMPOSED, which the R1088
//! discipline requires answering first, and which the tests below MEASURE rather
//! than argue (`neither_read_can_be_asked_the_other_read_s_question`):
//!
//! - `coordinate_read_answers` (R1050) already judges `report-frame-view
//!   --branch` against the fork tree and the manuscript. What it judges is
//!   MEMBERSHIP — which roads a view may draw from, and where a road stops
//!   following its parent. It never asks what the view says about a fact's TIME,
//!   which is the whole of `holds_at` and the whole of this pair. That file's
//!   header also said this read "is not in the read-agreement backlog either",
//!   because the panel could not supply its two required arguments; Round 1051
//!   gave the panel the corpus's own vocabulary and the read entered the
//!   population, which is how the backlog came to rank this pair first.
//! - the playable half EMBEDS the manuscript verbatim (pinned by R1048), so the
//!   walk laws could be stated over either read — and LAW SEAT could not. The
//!   locators exist only in this read: the manuscript answer carries none, and
//!   asking it for one is not a thing its usage line offers.
//! - neither read can be asked the other's question. `report-frame-view` has no
//!   `--telling` and `report-playable-world` has no `--frame`; both refuse the
//!   flag rather than ignoring it. The pair therefore crosses the one boundary
//!   the reads do not: a telling-scoped pointer meeting a telling-free verdict.
//!
//! TWO POPULATIONS, ONE IMPLEMENTATION. The authored corpora
//! (`authored_stores()`, the R1042 resolver) under every telling they declare
//! (the R1045 rule) carry the walk laws in bulk — 400 scenes over 18 roads, and
//! both end kinds, which is more than the pair beside this one found there. What
//! they leave at ZERO is every B-1 arm: no corpus holds a fact anchored outside
//! its order, one whose end coordinate is off it, a successor seated off it, a
//! confluence declaring a telling, a walk with an adjacency the order cannot
//! compare, a seat before its truth, or a seat off the road. Laws asserted over
//! an empty arm are claims with no evidence, so the tree constructs the store the
//! authors did not — through the same import recipe, so it is a store an author
//! could have shipped — and the same `judge` runs over both. The two populations
//! are asserted SEPARATELY: that the corpora exercise none of those is itself a
//! measurement, and averaging it into a total would hide it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use mnemosyne_atomic::AtomicStore;

use crate::common;
use common::{
    authored_stores, constructed_corpus, declared_tellings, holding_replay, run, SIDECAR,
};

/// The pair of shipped reads this contract judges, named ONCE and run from here.
/// The backlog walk (`surface/read_agreement_population.rs`) reads this
/// declaration out of the source, because it ranks 87 pairs by shared subjects to
/// say which to compare next and could not otherwise tell which already have a
/// contract.
const DECLARES: [&str; 2] = ["report-frame-view", "report-playable-world"];

/// The strings in a `[..]` list, empty when the key is absent.
fn strings(list: &serde_json::Value) -> BTreeSet<String> {
    list.as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// Every frame view one store has been asked for.
///
/// Memoized because the frame view has NO `--telling` — it cannot answer
/// differently under one — while the playable half must be asked per telling. A
/// store declaring two tellings would otherwise pay twice for identical answers.
/// `asks` counts what actually reached the binary, so the printed reach is the
/// work done rather than the work requested.
struct Views<'a> {
    ws: &'a Path,
    asked: BTreeMap<(String, String, String), Option<serde_json::Value>>,
    asks: usize,
    refusals: Vec<String>,
}

impl<'a> Views<'a> {
    fn of(ws: &'a Path) -> Self {
        Self {
            ws,
            asked: BTreeMap::new(),
            asks: 0,
            refusals: Vec::new(),
        }
    }

    /// The view of `frame` on `road` at `section`, or `None` when the read
    /// refuses it — recorded by name, never a silent skip.
    fn at(&mut self, frame: &str, road: &str, section: &str) -> Option<&serde_json::Value> {
        let key = (frame.to_string(), road.to_string(), section.to_string());
        if !self.asked.contains_key(&key) {
            self.asks += 1;
            let out = run(
                self.ws,
                &[
                    DECLARES[0],
                    "--frame",
                    frame,
                    "--branch",
                    road,
                    "--at",
                    section,
                    "--json",
                ],
            );
            let parsed = match out.status.success() {
                true => serde_json::from_slice(&out.stdout).ok(),
                false => {
                    self.refusals.push(format!(
                        "{frame}/{road}@{section}: {}",
                        String::from_utf8_lossy(&out.stderr)
                            .lines()
                            .next()
                            .unwrap_or("(no stderr)")
                    ));
                    None
                }
            };
            self.asked.insert(key.clone(), parsed);
        }
        self.asked.get(&key).and_then(Option::as_ref)
    }
}

/// What one population put in front of the laws, and what came back. Every field
/// is asserted: an arm that quietly reads zero is a law nothing tested, which is
/// what the second population exists to prevent.
#[derive(Default)]
struct Evidence {
    /// (store, telling) pairs that answered the playable half.
    answered: usize,
    roads: usize,
    /// Roads the unfiltered dump does not carry, asked with `--world`.
    filtered_roads: usize,
    fragments: usize,
    /// Scenes whose holding SET was compared.
    scenes: usize,
    /// Frame views that reached the binary.
    views: usize,
    /// (frame, road) populations compared against the road's own accounting.
    populations: usize,
    /// Facts named holding, summed over every scene and frame.
    named: usize,
    begun_checks: usize,
    expired_at: usize,
    superseded_at: usize,
    /// Facts a scene both begins and supersedes — where the two end kinds'
    /// sentences come apart.
    begun_and_superseded: usize,
    unplaced_from_checks: usize,
    unplaced_to_checks: usize,
    undecidable_checks: usize,
    /// Facts listed unplaced only for a SUCCESSOR's seat: still holding, which
    /// is the reading `common::holding_replay` separated in Round 1138.
    unplaced_by_successor: usize,
    /// Verdicts the event replay and the two reads legitimately differ on,
    /// because the walk between the fact's events and the scene crosses an
    /// adjacency the declared order cannot compare (LAW ORDER).
    order_undecided: usize,
    /// Facts in that difference for a PLACEMENT reason instead — asserted zero,
    /// because the replay subtracts what the road cannot place and the views
    /// call it unknown, so neither side should be naming it.
    unplaceable_in_difference: usize,
    locators: usize,
    seats_at_or_after_truth: usize,
    seats_before_truth: usize,
    seats_off_road: usize,
    /// One line per (store, telling): the roads, their lengths and their seats.
    /// Every total above is a sum, and a sum is where a road that stopped
    /// answering hides (R1036); this is the distribution they came from.
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
            "\n{whose}: {} (store, telling) pairs answered the playable world\n  \
             {} roads ({} asked with `--world` because the dump excludes confluences, {} \
             confluence fragments), {} scenes compared, {} frame views asked, {} (frame, road) \
             populations\n  \
             {} facts named holding; {} begun-before checks, {} expired scenes, {} superseded \
             scenes ({} of them also beginning the fact)\n  \
             {} unplaced-`canon_from` checks, {} unplaced-`canon_to` checks, {} undecidable \
             checks, {} facts unplaced only for a successor's seat, {} verdicts an undeclared \
             adjacency leaves the replay and the reads apart on ({} apart for a placement reason \
             instead)\n  \
             {} locators: {} seated at or after the truth, {} before it, {} off this road's walk",
            self.answered,
            self.roads,
            self.filtered_roads,
            self.fragments,
            self.scenes,
            self.views,
            self.populations,
            self.named,
            self.begun_checks,
            self.expired_at,
            self.superseded_at,
            self.begun_and_superseded,
            self.unplaced_from_checks,
            self.unplaced_to_checks,
            self.undecidable_checks,
            self.unplaced_by_successor,
            self.order_undecided,
            self.unplaceable_in_difference,
            self.locators,
            self.seats_at_or_after_truth,
            self.seats_before_truth,
            self.seats_off_road,
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
fn judge(views: &mut Views, name: &str, telling: &str, frames: &[String], ev: &mut Evidence) {
    let ws = views.ws;
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

    // PROVENANCE — the report says which question it answered, checked before
    // anything is compared. Comparing two answers to unstated questions is what
    // the first sweep of the R1048 pair did, and it read as eighteen
    // disagreements that were not there.
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

    let empty_list: Vec<serde_json::Value> = Vec::new();

    // EVERY REGISTERED ROAD, and the fork tree's own word on which of them are
    // confluences. `MAIN_BRANCH` is a world every store has whether or not it
    // registers a branch, and the tree never lists it.
    let branches = playable["fork_tree"]["branches"]
        .as_array()
        .unwrap_or(&empty_list);
    let confluences: BTreeSet<&str> = branches
        .iter()
        .filter(|branch| {
            !branch["converges"]
                .as_array()
                .unwrap_or(&empty_list)
                .is_empty()
        })
        .filter_map(|branch| branch["branch_id"].as_str())
        .collect();
    let mut roads: Vec<String> = branches
        .iter()
        .filter_map(|branch| branch["branch_id"].as_str().map(ToString::to_string))
        .chain(std::iter::once(mnemosyne_core::MAIN_BRANCH.to_string()))
        .collect();
    roads.sort();
    roads.dedup();

    let mut shape: Vec<String> = Vec::new();
    for road in &roads {
        // A road the unfiltered dump does not carry is asked with `--world` —
        // the ask a runtime makes, and the only one that reaches a confluence.
        let world = match playable["worlds"].get(road) {
            Some(world) => world.clone(),
            None => match read(&[DECLARES[1], "--telling", telling, "--world", road, "--json"]) {
                Ok(filtered) => {
                    ev.filtered_roads += 1;
                    if filtered["world"].as_str() != Some(road.as_str()) {
                        ev.disagreements.push(format!(
                            "{name} [{telling}]/{road}: the playable world was asked for that \
                             road and its report says `{}`",
                            filtered["world"]
                        ));
                    }
                    match filtered["worlds"].get(road) {
                        Some(world) => world.clone(),
                        None => {
                            ev.disagreements.push(format!(
                                "{name} [{telling}]/{road}: the playable world answered the road \
                                 filter and its map does not carry that road",
                            ));
                            continue;
                        }
                    }
                }
                Err(why) => {
                    ev.note(
                        "a registered road the playable world refuses",
                        format!("{name} [{telling}]/{road}: {why}"),
                    );
                    continue;
                }
            },
        };
        ev.roads += 1;
        let manuscript = &world["manuscript"];
        let scenes = manuscript["scenes"].as_array().unwrap_or(&empty_list);

        // LAW FRAGMENT, half one: the manuscript's marker against the fork
        // tree's `converges`. Both come from the one `is_confluence`, and the
        // frame view's own copy is checked per view below.
        let fragment = confluences.contains(road.as_str());
        if fragment {
            ev.fragments += 1;
        }
        if manuscript["confluence_fragment"].as_bool() != Some(fragment) {
            ev.disagreements.push(format!(
                "{name} [{telling}]/{road}: the fork tree calls it a confluence={fragment} and \
                 its manuscript says `{}`",
                manuscript["confluence_fragment"],
            ));
        }

        // THE ROAD'S OWN ACCOUNT OF ITS FACTS, read off the wire: where each is
        // begun (with the frame the event stamps), which coordinate it could not
        // place, and which it cannot decide the visibility of.
        let mut begins_at: BTreeMap<&str, usize> = BTreeMap::new();
        let mut frame_of: BTreeMap<&str, &str> = BTreeMap::new();
        let mut superseded: Vec<(usize, &str)> = Vec::new();
        let mut expired: Vec<(usize, &str)> = Vec::new();
        // THE WALK IS ONE READING OF A PARTIAL ORDER, and the read says where:
        // `undeclared_adjacencies` names every adjacent pair the composed order
        // cannot compare. A replay of the events is a SEQUENCE reading and
        // `holds_at` is an ORDER reading, so the two are the same statement only
        // across a stretch of walk with no undeclared gap in it.
        let walk: Vec<&str> = scenes
            .iter()
            .filter_map(|scene| scene["section"].as_str())
            .collect();
        let undeclared: BTreeSet<[&str; 2]> = manuscript["undeclared_adjacencies"]
            .as_array()
            .unwrap_or(&empty_list)
            .iter()
            .filter_map(|pair| {
                let pair = pair.as_array()?;
                Some([pair.first()?.as_str()?, pair.get(1)?.as_str()?])
            })
            .collect();
        // `gaps[j]` = how many undeclared adjacencies lie in the first `j` steps,
        // so any stretch is one subtraction.
        let mut gaps: Vec<usize> = Vec::with_capacity(walk.len());
        gaps.push(0);
        for step in walk.windows(2) {
            let last = *gaps.last().expect("the prefix starts at zero");
            gaps.push(last + usize::from(undeclared.contains(&[step[0], step[1]])));
        }
        let orderable = |a: usize, b: usize| -> bool {
            let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
            gaps.get(hi)
                .zip(gaps.get(lo))
                .is_some_and(|(hi, lo)| hi == lo)
        };
        for (index, scene) in scenes.iter().enumerate() {
            for event in scene["begins"].as_array().unwrap_or(&empty_list) {
                let (Some(id), Some(frame)) = (event["fact_id"].as_str(), event["frame"].as_str())
                else {
                    continue;
                };
                begins_at.insert(id, index);
                frame_of.insert(id, frame);
            }
            for event in scene["ends"].as_array().unwrap_or(&empty_list) {
                let (Some(id), Some(kind)) = (event["fact_id"].as_str(), event["kind"].as_str())
                else {
                    continue;
                };
                match kind {
                    "superseded" => superseded.push((index, id)),
                    "expired" => expired.push((index, id)),
                    _ => ev.disagreements.push(format!(
                        "{name} [{telling}]/{road}: an end event of kind `{kind}`, which is \
                         neither of the two the wire declares",
                    )),
                }
            }
        }
        let mut unplaced: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        for row in manuscript["unplaced_facts"]
            .as_array()
            .unwrap_or(&empty_list)
        {
            let (Some(id), Some(field)) = (row["fact_id"].as_str(), row["field"].as_str()) else {
                continue;
            };
            unplaced.entry(field).or_default().insert(id);
        }
        let undecidable = strings(&manuscript["undecidable"]);
        let accounted: BTreeSet<String> = begins_at
            .keys()
            .map(|id| (*id).to_string())
            .chain(unplaced.values().flatten().map(|id| (*id).to_string()))
            .chain(undecidable.iter().cloned())
            .collect();

        let replay = holding_replay(manuscript);
        ev.unplaced_by_successor += replay.unplaceable_by_successor.len();

        // Per frame: the population the views report, and the point it was first
        // read at — so "the population does not move with the point" is a
        // comparison rather than an assumption.
        let mut population: BTreeMap<&str, (BTreeSet<String>, String)> = BTreeMap::new();
        // Per scene: which facts the views name holding there, so the end-kind
        // laws below read the verdict rather than asking again.
        let mut named_at: Vec<BTreeSet<String>> = Vec::new();
        let mut refused_a_view = false;

        for (index, (section, replayed)) in replay.at.iter().enumerate() {
            let mut named: BTreeSet<String> = BTreeSet::new();
            let mut unsure: BTreeSet<String> = BTreeSet::new();
            let mut namer: BTreeMap<String, &str> = BTreeMap::new();
            for frame in frames {
                let Some(view) = views.at(frame, road, section) else {
                    refused_a_view = true;
                    continue;
                };
                let view = view.clone();
                ev.views += 1;
                // PROVENANCE, this read's half: three arguments, echoed.
                if (
                    view["frame"].as_str(),
                    view["branch"].as_str(),
                    view["at"].as_str(),
                ) != (Some(frame), Some(road.as_str()), Some(section.as_str()))
                {
                    ev.disagreements.push(format!(
                        "{name} [{telling}]/{road}@{section}: the view was asked for frame \
                         `{frame}` and its report says frame `{}` branch `{}` at `{}`",
                        view["frame"], view["branch"], view["at"],
                    ));
                }
                // LAW FRAGMENT, half two — the third surface of one predicate.
                if view["confluence_fragment"].as_bool() != Some(fragment) {
                    ev.disagreements.push(format!(
                        "{name} [{telling}]/{road}@{section}: the fork tree calls it a \
                         confluence={fragment} and the frame view says `{}`",
                        view["confluence_fragment"],
                    ));
                }
                let holding: BTreeSet<String> = view["holding"]
                    .as_array()
                    .unwrap_or(&empty_list)
                    .iter()
                    .filter_map(|entry| entry["fact_id"].as_str().map(ToString::to_string))
                    .collect();
                let not_holding = strings(&view["not_holding"]);
                let unknown = strings(&view["unknown"]);
                if view["holding_count"].as_u64().map(|n| n as usize) != Some(holding.len()) {
                    ev.disagreements.push(format!(
                        "{name} [{telling}]/{road}@{section} [{frame}]: the envelope counts {} \
                         and names {} holding",
                        view["holding_count"],
                        holding.len(),
                    ));
                }

                // LAW ONE FRAME — a view never mixes them, so no fact is named
                // holding under two.
                for id in &holding {
                    if let Some(other) = namer.insert(id.clone(), frame) {
                        ev.disagreements.push(format!(
                            "{name} [{telling}]/{road}@{section}: `{id}` is named holding under \
                             both `{other}` and `{frame}`",
                        ));
                    }
                }
                named.extend(holding.iter().cloned());
                unsure.extend(unknown.iter().cloned());

                // LAW POPULATION, half one: it does not move with the point.
                let pop: BTreeSet<String> = holding
                    .iter()
                    .chain(not_holding.iter())
                    .chain(unknown.iter())
                    .cloned()
                    .collect();
                match population.get(frame.as_str()) {
                    None => {
                        ev.populations += 1;
                        population.insert(frame, (pop.clone(), section.clone()));
                    }
                    Some((first, first_at)) if *first != pop => ev.disagreements.push(format!(
                        "{name} [{telling}]/{road} [{frame}]: the population at `{first_at}` \
                         holds {} facts and at `{section}` holds {} — a view's population is the \
                         frame's visible facts and cannot move with the point",
                        first.len(),
                        pop.len(),
                    )),
                    Some(_) => {}
                }

                // LAW UNDECIDABLE — B-1 honesty, reached by two different
                // predicates. A coordinate the walk cannot NAME versus one the
                // order cannot COMPARE.
                for id in unplaced.get("canon_from").into_iter().flatten() {
                    if !pop.contains(*id) {
                        continue;
                    }
                    ev.unplaced_from_checks += 1;
                    if !unknown.contains(*id) {
                        ev.disagreements.push(format!(
                            "{name} [{telling}]/{road}@{section} [{frame}]: the road cannot place \
                             `{id}`'s own coordinate and the view does not call it unknown",
                        ));
                    }
                }
                for id in unplaced.get("canon_to").into_iter().flatten() {
                    if !pop.contains(*id) {
                        continue;
                    }
                    // Unknown from the scene it begins at: before that, the order
                    // decides it does not hold yet without ever consulting the
                    // end it cannot place — and only where the order can read
                    // the walk between the two as a sequence at all.
                    let begun = begins_at
                        .get(*id)
                        .is_some_and(|begin| orderable(*begin, index) && *begin <= index);
                    let decided = begins_at
                        .get(*id)
                        .is_some_and(|begin| orderable(*begin, index));
                    ev.unplaced_to_checks += 1;
                    let called = match begun || !decided {
                        true => unknown.contains(*id),
                        false => !unknown.contains(*id) && not_holding.contains(*id),
                    };
                    if !called {
                        ev.disagreements.push(format!(
                            "{name} [{telling}]/{road}@{section} [{frame}]: `{id}`'s end \
                             coordinate is off this road and the view calls it \
                             holding={} unknown={} not-holding={} at a scene it has{} begun at",
                            holding.contains(*id),
                            unknown.contains(*id),
                            not_holding.contains(*id),
                            if begun { "" } else { " not" },
                        ));
                    }
                }
                for id in &undecidable {
                    if !pop.contains(id) {
                        continue;
                    }
                    ev.undecidable_checks += 1;
                    if !unknown.contains(id) {
                        ev.disagreements.push(format!(
                            "{name} [{telling}]/{road}@{section} [{frame}]: the road calls `{id}` \
                             undecidable and the view does not call it unknown",
                        ));
                    }
                }
            }

            // LAW HOLDING — the SET, not the number. The union over frames is
            // what the road's own events replay, wherever the walk between the
            // fact's events and this scene is one the declared order can read as
            // a sequence.
            //
            // LAW ORDER is the other half, and the confluence fragment is what
            // made it necessary: where the walk crosses an adjacency the order
            // cannot compare, a replay says a fact has begun and `holds_at` says
            // the declaration cannot decide it. The two READS agree with each
            // other there — the count sides with the order — so the difference
            // is the REPLAY's, and every member of it must be explained by an
            // undeclared adjacency the same answer names.
            for id in named.symmetric_difference(replayed) {
                let events = [begins_at.get(id.as_str()).copied()]
                    .into_iter()
                    .flatten()
                    .chain(
                        superseded
                            .iter()
                            .chain(expired.iter())
                            .filter(|(_, ended)| ended == id)
                            .map(|(at, _)| *at),
                    );
                // A PLACEMENT REASON WOULD BE A DIFFERENT EXPLANATION, and it is
                // counted rather than accepted: the replay already subtracts what
                // this road cannot place and the views already call it unknown,
                // so such a fact should not be in this difference at all. Both
                // populations read ZERO, and that is asserted — an arm nothing
                // reaches is a branch with no evidence behind it, which is the
                // shape this whole round is about.
                if unplaced.values().any(|ids| ids.contains(id.as_str()))
                    || undecidable.contains(id)
                {
                    ev.unplaceable_in_difference += 1;
                    continue;
                }
                if events.clone().any(|at| !orderable(at, index)) {
                    ev.order_undecided += 1;
                    if replayed.contains(id) && !unsure.contains(id) {
                        ev.disagreements.push(format!(
                            "{name} [{telling}]/{road}@{section}: the events replay `{id}` as \
                             holding, no view names it, and no view calls it unknown either — an \
                             undeclared adjacency makes a verdict undecidable, not negative",
                        ));
                    }
                    continue;
                }
                ev.disagreements.push(format!(
                    "{name} [{telling}]/{road}@{section}: `{id}` is {} and {} by the frame views, \
                     across a stretch of walk the declared order reads as a sequence",
                    match replayed.contains(id) {
                        true => "replayed as holding by this road's own events",
                        false => "not replayed as holding by this road's own events",
                    },
                    match named.contains(id) {
                        true => "named",
                        false => "not named",
                    },
                ));
            }
            // AND THE COUNT BESIDE IT. Round 1056 proved the count is a function
            // of the events; this is the other read arriving at the same number
            // through the order rather than through the walk.
            let counted = scenes
                .get(index)
                .and_then(|scene| scene["holding_count"].as_u64())
                .map(|n| n as usize);
            if counted != Some(named.len()) {
                ev.disagreements.push(format!(
                    "{name} [{telling}]/{road}@{section}: the scene counts {counted:?} holding and \
                     the frame views name {}",
                    named.len(),
                ));
            }
            ev.scenes += 1;
            ev.named += named.len();

            // LAW BEGUN — a fact holding here began at or before here, per the
            // walk the other read publishes.
            for id in &named {
                ev.begun_checks += 1;
                match begins_at.get(id.as_str()) {
                    Some(begin) if *begin <= index => {}
                    other => ev.disagreements.push(format!(
                        "{name} [{telling}]/{road}@{section}: a view holds `{id}` at scene {index} \
                         and this road begins it at {other:?}",
                    )),
                }
            }
            named_at.push(named);
        }

        // LAW EXPIRED / SUPERSEDED — the two kinds' sentences, judged from the
        // views collected above. A view this read refused leaves the verdict
        // unread, and the refusal is already recorded; asserting over it would
        // read an absent answer as a negative one.
        if !refused_a_view {
            for (index, id) in &superseded {
                ev.superseded_at += 1;
                if named_at[*index].contains(*id) {
                    ev.disagreements.push(format!(
                        "{name} [{telling}]/{road}: a successor begins at scene {index} and a \
                         view still holds the superseded `{id}` there",
                    ));
                }
                if begins_at.get(*id) == Some(index) {
                    ev.begun_and_superseded += 1;
                }
            }
            for (index, id) in &expired {
                ev.expired_at += 1;
                // Its last scene: it holds HERE, and it is gone from the next —
                // unless a successor also cuts it here, which is the other kind's
                // sentence and takes precedence.
                let cut_here = superseded
                    .iter()
                    .any(|(at, other)| at == index && other == id);
                // And unless the order cannot read the walk between the scene it
                // begins at and this one, where "it still holds here" is not a
                // verdict the declaration can reach (LAW ORDER).
                let reachable = begins_at
                    .get(*id)
                    .is_some_and(|begin| orderable(*begin, *index));
                if !cut_here && reachable && !named_at[*index].contains(*id) {
                    ev.disagreements.push(format!(
                        "{name} [{telling}]/{road}: `{id}` expires at scene {index} and no view \
                         holds it there, though the interval is closed at its end",
                    ));
                }
                if let Some(next) = named_at.get(index + 1) {
                    if next.contains(*id) {
                        ev.disagreements.push(format!(
                            "{name} [{telling}]/{road}: `{id}` expires at scene {index} and a \
                             view still holds it at the next scene",
                        ));
                    }
                }
            }
        }

        // LAW POPULATION, half two: the frames' populations together are the
        // facts this road accounts for. Every fact the road begins, cannot place
        // or cannot decide belongs to exactly one frame's view, and nothing else
        // does.
        if !refused_a_view && !frames.is_empty() {
            let reported: BTreeSet<String> = population
                .values()
                .flat_map(|(pop, _)| pop.iter().cloned())
                .collect();
            if reported != accounted {
                let invented: Vec<&String> = reported.difference(&accounted).collect();
                let missing: Vec<&String> = accounted.difference(&reported).collect();
                ev.disagreements.push(format!(
                    "{name} [{telling}]/{road}: the views hold {invented:?} in their populations \
                     that this road does not account for, and the road accounts for {missing:?} \
                     that no view's population holds",
                ));
            }
        }

        // LAW SEAT — the locators, the half no manuscript ask can reach.
        let mut seats = [0usize; 3];
        for locator in world["locators"].as_array().unwrap_or(&empty_list) {
            ev.locators += 1;
            let (Some(id), Some(seat)) = (locator["fact_id"].as_str(), locator["scene"].as_str())
            else {
                continue;
            };
            if locator["world_line"].as_str() != Some(road.as_str()) {
                ev.disagreements.push(format!(
                    "{name} [{telling}]/{road}: a locator for `{id}` says its world-line is `{}`",
                    locator["world_line"],
                ));
            }
            // Locators are emitted from begins-events, so a seat for a fact this
            // road does not begin is a pointer into nothing.
            let (Some(&truth), Some(frame)) = (begins_at.get(id), frame_of.get(id)) else {
                ev.disagreements.push(format!(
                    "{name} [{telling}]/{road}: a locator seats `{id}`, which this road never \
                     begins",
                ));
                continue;
            };
            let ordinal = locator["scene_ordinal"].as_u64().map(|n| n as usize);
            let Some(view) = views.at(frame, road, seat) else {
                continue;
            };
            let held = view["holding"]
                .as_array()
                .unwrap_or(&empty_list)
                .iter()
                .any(|entry| entry["fact_id"].as_str() == Some(id));
            let unknown = strings(&view["unknown"]).contains(id);
            match ordinal {
                // The audience meets it at or after the scene it becomes true.
                Some(at) if at >= truth => {
                    ev.seats_at_or_after_truth += 1;
                    seats[0] += 1;
                    if !held {
                        ev.disagreements.push(format!(
                            "{name} [{telling}]/{road}: `{id}` is seated at `{seat}` (scene {at}), \
                             at or after the scene {truth} it begins at, and the `{frame}` view \
                             does not hold it there",
                        ));
                    }
                }
                // R949's class from the epistemic side: the runtime is pointed at
                // a fact that is not true yet.
                Some(at) => {
                    ev.seats_before_truth += 1;
                    seats[1] += 1;
                    if held {
                        ev.disagreements.push(format!(
                            "{name} [{telling}]/{road}: `{id}` is seated at `{seat}` (scene {at}), \
                             before the scene {truth} it begins at, and the `{frame}` view holds \
                             it there anyway",
                        ));
                    }
                }
                // A seat off this road's walk: the story cannot decide the fact
                // at that coordinate at all.
                None => {
                    ev.seats_off_road += 1;
                    seats[2] += 1;
                    if !unknown {
                        ev.disagreements.push(format!(
                            "{name} [{telling}]/{road}: `{id}` is seated at `{seat}`, which is not \
                             a scene of this road's walk, and the `{frame}` view does not call it \
                             unknown there",
                        ));
                    }
                }
            }
        }

        shape.push(format!(
            "{road}={}sc/{}fr/{}loc({}after/{}before/{}off)",
            replay.at.len(),
            population.len(),
            seats.iter().sum::<usize>(),
            seats[0],
            seats[1],
            seats[2],
        ));
    }
    ev.shape
        .push(format!("{name} [{telling}]: {}", shape.join(" ")));
}

/// The frames a store registers — every frame the view can be asked about, so
/// the union over them is the whole of what the count aggregates.
fn frames_of(ws: &Path) -> Vec<String> {
    AtomicStore::load(&ws.join(SIDECAR))
        .map(|store| store.frames.keys().map(ToString::to_string).collect())
        .unwrap_or_default()
}

/// The sections, order and facts of the store the authors did not write.
///
/// Two frames, so the union over frames is a union of two non-empty answers. A
/// confluence that DECLARES a telling, which no corpus has. A succession, so a
/// scene supersedes; a closed interval, so a scene expires; a fact that begins
/// and is superseded at ONE scene. A fact anchored where no order positions it,
/// one whose END is there, and one whose SUCCESSOR is seated there — the three
/// `unplaced_facts` fields, which are three different reasons and not one. And
/// three seats: at the truth, before it, and off the road entirely.
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
        section("k-02", "the errand is given"),
        section("k-03", "the road divides"),
        section("k-left", "the left road"),
        section("k-right", "the right road"),
        section("k-join", "where both roads arrive"),
        section("k-after", "the shared continuation"),
        section("k-loose", "a scene no order positions"),
    ]);
    let order = serde_json::json!({
        "edges": [["k-01", "k-02"], ["k-02", "k-03"]],
        "branches": {
            "left": [["k-03", "k-left"], ["k-left", "k-join"]],
            "right": [["k-03", "k-right"], ["k-right", "k-join"]],
            "merge": [["k-join", "k-after"]],
        },
    });
    let fact = |id: &str, frame: &str, branch: Option<&str>, at: &str, claim: &str| {
        let mut row = serde_json::json!({
            "fact_id": id,
            "frame": frame,
            "claim": claim,
            "canon_from": at,
            "evidence": [at],
        });
        if let Some(branch) = branch {
            row["branch"] = serde_json::json!(branch);
        }
        row
    };
    // A closed interval: still true AT `k-02`, gone from `k-03`.
    let mut expires = fact(
        "kx-rumour",
        "k-teller",
        None,
        "k-01",
        "a rumour rides ahead",
    );
    expires["canon_to"] = serde_json::json!("k-02");
    // The successor of `kx-open`, seated at the scene the roads divide: that
    // scene SUPERSEDES the first belief and BEGINS this one.
    let mut supersedes = fact(
        "kx-open-again",
        "k-teller",
        None,
        "k-03",
        "the gate was never open",
    );
    supersedes["supersedes_in_frame"] = serde_json::json!("kx-open");
    // BEGUN AND SUPERSEDED AT ONE SCENE — where the two end kinds' sentences
    // come apart. `kx-brief` begins at `k-02` and its successor begins there
    // too, so the scene both starts it and says it no longer holds FROM here.
    let mut corrects = fact(
        "kx-brief-again",
        "k-child",
        None,
        "k-02",
        "the errand was misheard",
    );
    corrects["supersedes_in_frame"] = serde_json::json!("kx-brief");
    // Its end coordinate is a scene no order positions, so no road can decide
    // when it stops.
    let mut end_off_road = fact(
        "kx-endless",
        "k-child",
        None,
        "k-02",
        "the errand has no end",
    );
    end_off_road["canon_to"] = serde_json::json!("k-loose");
    // A successor seated off the order: `holds_at` cuts a fact at a successor it
    // can ORDER, and this one it cannot, so `kx-watched` goes on holding.
    let mut successor_off_road = fact(
        "kx-watched-no-more",
        "k-child",
        None,
        "k-loose",
        "no one is watching the road after all",
    );
    successor_off_road["supersedes_in_frame"] = serde_json::json!("kx-watched");
    let facts = serde_json::json!({
        "frames": [{"frame_id": "k-teller"}, {"frame_id": "k-child"}],
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
            fact("kx-open", "k-teller", None, "k-01", "the gate stands open"),
            expires,
            supersedes,
            fact("kx-watched", "k-child", None, "k-02", "someone is watching the road"),
            successor_off_road,
            fact("kx-brief", "k-child", None, "k-02", "the errand is plainly stated"),
            corrects,
            end_off_road,
            fact("kx-left", "k-teller", Some("left"), "k-left", "the left road is taken"),
            fact("kx-right", "k-teller", Some("right"), "k-right", "the right road is taken"),
            fact("kx-merge", "k-child", Some("merge"), "k-after",
                 "both roads end at the same door"),
            fact("kx-loose", "k-child", None, "k-loose",
                 "a fact at a coordinate no order positions"),
        ],
        "disclosure_plans": [{
            "telling_id": "k-one",
            "default_mode": "withhold",
            "overrides": [
                // A derived seat: the audience meets it where it becomes true.
                {"fact_id": "kx-open", "mode": "state"},
                // Seated BEFORE its truth (R949): stated at the first scene,
                // true only from the scene the roads divide at.
                {"fact_id": "kx-open-again", "mode": "state", "surface": {"scene": "k-01"}},
                // Seated OFF the road: `k-left` is not a scene the right road
                // walks, so the right road's locator has no ordinal.
                {"fact_id": "kx-watched", "mode": "state", "surface": {"scene": "k-left"}},
                {"fact_id": "kx-left", "mode": "state"},
                {"fact_id": "kx-right", "mode": "state"},
                {"fact_id": "kx-merge", "mode": "state"},
            ],
        }],
    });
    (sections, order, facts)
}

#[test]
fn the_frame_view_names_at_each_point_what_the_playable_world_counts_there() {
    // POPULATION ONE — every store an author shipped that this tree can ask.
    let (stores, unloadable) = authored_stores();
    let asked = stores.len() + unloadable.len();
    let mut authored = Evidence::default();
    let mut authored_views = 0usize;
    for corpus in &unloadable {
        authored.note("the store does not load", corpus.named_reason());
    }
    for store in &stores {
        let ws = store.ws.path();
        let frames = frames_of(ws);
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
        if frames.is_empty() {
            authored.note("registers no frame", store.name.clone());
            continue;
        }
        // ONE VIEW CACHE PER STORE — the frame view has no `--telling`, so a
        // store declaring two pays for its views once.
        let mut views = Views::of(ws);
        for telling in &tellings {
            judge(&mut views, &store.name, telling, &frames, &mut authored);
        }
        authored_views += views.asks;
        for refusal in &views.refusals {
            authored.note(
                "the frame view refuses",
                format!("{}: {refusal}", store.name),
            );
        }
    }

    // POPULATION TWO — the store no author wrote, through the same recipe.
    let (sections, order, facts) = constructed_manifests();
    let built = constructed_corpus(&sections, &order, &facts)
        .unwrap_or_else(|e| panic!("the constructed manifests must import: {e}"));
    let mut constructed = Evidence::default();
    let mut views = Views::of(built.path());
    let constructed_frames = frames_of(built.path());
    judge(
        &mut views,
        "the constructed store",
        "k-one",
        &constructed_frames,
        &mut constructed,
    );
    let constructed_views = views.asks;
    for refusal in &views.refusals {
        constructed.note("the frame view refuses", refusal.clone());
    }

    // Print BEFORE asserting (the R1026 lesson).
    println!(
        "{asked} authored stores asked; {authored_views} frame-view asks reached the binary for \
         them and {constructed_views} for the constructed store",
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
            authored.count("declares no telling, so the playable world cannot be asked"),
            authored.count("registers no frame"),
        ) == (44, 3, 30, 0),
        "POPULATION (stores): of the corpora asked, these never reach the \
         comparison — 3 whose author's submission the write path rejected, 30 \
         declaring no telling for a pair whose playable half is per-telling, and \
         none of the rest without a frame. The corpora that declare a confluence \
         are inside that 30, which is why the constructed store still exists",
    );
    check(
        (
            authored.answered,
            authored.roads,
            authored.filtered_roads,
            authored.fragments,
            authored.scenes,
        ) == (14, 21, 0, 0, 444),
        "AUTHORED REACH: the (store, telling) pairs that answered, the roads \
         compared, the roads the dump could not answer for, the confluence \
         fragments, and the scenes whose holding SET was compared",
    );
    check(
        (
            authored.views,
            authored_views,
            authored.populations,
            authored.named,
        ) == (2750, 2567, 114, 20061),
        "AUTHORED EVIDENCE: the frame views read, how many of those reached the \
         binary (the rest are the second telling of a store, which cannot move \
         a read that has no `--telling`), the (frame, road) populations compared \
         against the road's own accounting, and the facts they named holding \
         across every scene",
    );
    check(
        (
            authored.begun_checks,
            authored.expired_at,
            authored.superseded_at,
            authored.begun_and_superseded,
        ) == (20061, 18, 369, 0),
        "AUTHORED END KINDS: the authored corpora DO close intervals and DO \
         supersede beliefs, so both end-kind laws carry authored evidence — 18 \
         scenes where a fact holds through its last coordinate and 369 where a \
         successor has already taken over. None of them does both at one scene, \
         which is the arm the constructed store carries. Every fact named \
         holding was checked against the scene the road begins it at",
    );
    check(
        (
            authored.unplaced_from_checks,
            authored.unplaced_to_checks,
            authored.undecidable_checks,
            authored.unplaced_by_successor,
            authored.order_undecided,
            authored.unplaceable_in_difference,
        ) == (0, 0, 0, 0, 0, 0),
        "AUTHORED B-1 SILENCE, ASSERTED: no corpus anchors a fact where no order \
         positions it, ends one there, seats a successor there, or leaves a road \
         unable to decide a fact's visibility — the arms of LAW UNDECIDABLE. Nor \
         does any authored walk cross an adjacency the order cannot compare, \
         which is why the event replay and the two reads agree here everywhere \
         and why the qualification LAW ORDER states is invisible in this \
         population",
    );
    check(
        (
            authored.locators,
            authored.seats_at_or_after_truth,
            authored.seats_before_truth,
            authored.seats_off_road,
        ) == (1554, 1554, 0, 0),
        "AUTHORED SEATS: every locator an author's telling seats seats it at or \
         after the scene the fact becomes true. The two other arms are zero, so \
         R949's class and the off-road seat are the constructed store's alone",
    );
    check(
        (
            constructed.answered,
            constructed.roads,
            constructed.filtered_roads,
            constructed.fragments,
            constructed.scenes,
        ) == (1, 4, 1, 1, 22),
        "CONSTRUCTED REACH: one store, four roads — a 3-scene trunk, two forks \
         that carry on through the merge, and the confluence's own 7-scene \
         fragment, which only a `--world` ask reaches and which all three \
         surfaces mark",
    );
    check(
        (
            constructed.views,
            constructed.populations,
            constructed.named,
        ) == (44, 8, 54),
        "CONSTRUCTED EVIDENCE: two frames asked at every scene of every road, one \
         population per (frame, road), and the facts they named holding",
    );
    check(
        (
            constructed.expired_at,
            constructed.superseded_at,
            constructed.begun_and_superseded,
        ) == (3, 6, 3),
        "CONSTRUCTED END KINDS: the closed interval expires on the trunk and both \
         forks, and two successions supersede on each — one of them at the very \
         scene it BEGINS the fact it replaces, which is where the two kinds' \
         sentences come apart and the arm no authored corpus reaches",
    );
    check(
        (
            constructed.unplaced_from_checks,
            constructed.unplaced_to_checks,
            constructed.undecidable_checks,
            constructed.unplaced_by_successor,
            constructed.order_undecided,
            constructed.unplaceable_in_difference,
        ) == (6, 15, 87, 1, 2, 0),
        "CONSTRUCTED B-1: the fact anchored off the order, the one whose end is \
         off it, and the facts the confluence fragment cannot decide — three \
         reasons a coordinate is undecidable, checked separately. Then the two \
         readings Round 1138 had to separate: a fact unplaced only for a \
         SUCCESSOR's seat goes on holding (1), and a walk that crosses an \
         adjacency the order cannot compare leaves the event replay and the two \
         reads apart (2), which is LAW ORDER and which only the fragment reaches. \
         The last is zero in BOTH populations: nothing is in that difference for \
         a placement reason, because the replay subtracts what the road cannot \
         place and the views call it unknown",
    );
    check(
        (
            constructed.locators,
            constructed.seats_at_or_after_truth,
            constructed.seats_before_truth,
            constructed.seats_off_road,
        ) == (14, 9, 3, 2),
        "CONSTRUCTED SEATS: all three arms fire — the derived seats at the truth, \
         the R949 seat before it on the trunk and both forks, and the two seats \
         at a scene their own road never walks",
    );
    check(
        authored.disagreements.is_empty() && constructed.disagreements.is_empty(),
        "CONTRACT: at every scene of every road, the facts the frame views name \
         holding are the facts the road's own events replay; the seats the \
         telling hands a runtime are true where they are met; and what neither \
         read can decide, both call undecidable",
    );

    assert_eq!(
        broken,
        Vec::<String>::new(),
        "the frame-view/playable-world correspondence no longer holds"
    );
}

/// The per-scene count cannot say WHOSE view holds what — which is why this pair
/// is asked per FRAME. (Round 1138.)
///
/// Round 1056 proved the manuscript's `holding_count` is a function of the events
/// the same answer names, so the count is not lossy about WHICH facts hold. It is
/// silent about something else: it aggregates over frames. Two facts of DIFFERENT
/// frames trading the coordinates they are anchored at leaves every count exactly
/// where it was, and moves what each frame knows at both scenes.
///
/// So this measures what the number cannot see and what the other read does: the
/// counts are equal, the begins-events move (the edit is real and the playable
/// world describes it), and the two frame views at those scenes swap. Without the
/// frame axis, LAW HOLDING would hold for a store where the wrong frame knows
/// everything.
#[test]
fn the_scene_count_cannot_say_whose_view_holds_what() {
    let (sections, order, facts) = constructed_manifests();
    // `kx-open` is `k-teller`'s at `k-01`; `kx-watched` is `k-child`'s at
    // `k-02`. Neither carries an authored seat that would move with them, and
    // neither is anyone's successor.
    let traded = {
        let mut traded = facts.clone();
        let mut moved = 0usize;
        for fact in traded["facts"].as_array_mut().expect("facts array") {
            let to = match fact["fact_id"].as_str() {
                Some("kx-open") => "k-02",
                Some("kx-watched") => "k-01",
                _ => continue,
            };
            fact["canon_from"] = serde_json::json!(to);
            fact["evidence"] = serde_json::json!([to]);
            moved += 1;
        }
        assert_eq!(moved, 2, "the trade moved {moved} facts, not the two named");
        traded
    };
    let build = |manifest: &serde_json::Value| {
        constructed_corpus(&sections, &order, manifest)
            .unwrap_or_else(|e| panic!("the constructed manifests must import: {e}"))
    };
    let (before, after) = (build(&facts), build(&traded));

    let ask = |ws: &Path, argv: &[&str]| -> serde_json::Value {
        let out = run(ws, argv);
        assert!(
            out.status.success(),
            "{argv:?} on the constructed store: {}",
            String::from_utf8_lossy(&out.stderr),
        );
        serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("{argv:?} is not json: {e}"))
    };
    let playable = |ws: &Path| ask(ws, &[DECLARES[1], "--telling", "k-one", "--json"]);
    let view = |ws: &Path, frame: &str, at: &str| {
        ask(
            ws,
            &[
                DECLARES[0],
                "--frame",
                frame,
                "--branch",
                "main",
                "--at",
                at,
                "--json",
            ],
        )
    };

    let empty: Vec<serde_json::Value> = Vec::new();
    // The road as the playable world walks it: per scene, the count and the
    // facts it begins.
    let walk = |playable: &serde_json::Value| -> Vec<(String, u64, BTreeSet<String>)> {
        playable["worlds"]["main"]["manuscript"]["scenes"]
            .as_array()
            .unwrap_or(&empty)
            .iter()
            .map(|scene| {
                (
                    scene["section"].as_str().unwrap_or_default().to_string(),
                    scene["holding_count"].as_u64().unwrap_or_default(),
                    scene["begins"]
                        .as_array()
                        .unwrap_or(&empty)
                        .iter()
                        .filter_map(|event| event["fact_id"].as_str().map(ToString::to_string))
                        .collect(),
                )
            })
            .collect()
    };
    let held = |view: &serde_json::Value| -> BTreeSet<String> {
        view["holding"]
            .as_array()
            .unwrap_or(&empty)
            .iter()
            .filter_map(|entry| entry["fact_id"].as_str().map(ToString::to_string))
            .collect()
    };

    let (walked_before, walked_after) = (
        walk(&playable(before.path())),
        walk(&playable(after.path())),
    );
    let counts = |walked: &[(String, u64, BTreeSet<String>)]| -> Vec<(String, u64)> {
        walked
            .iter()
            .map(|(section, count, _)| (section.clone(), *count))
            .collect()
    };
    let teller = (
        held(&view(before.path(), "k-teller", "k-01")),
        held(&view(after.path(), "k-teller", "k-01")),
    );
    let child = (
        held(&view(before.path(), "k-child", "k-01")),
        held(&view(after.path(), "k-child", "k-01")),
    );

    println!(
        "counts before {:?}\ncounts after  {:?}\nk-teller at k-01: {:?} -> {:?}\nk-child  at \
         k-01: {:?} -> {:?}",
        counts(&walked_before),
        counts(&walked_after),
        teller.0,
        teller.1,
        child.0,
        child.1,
    );

    let mut broken: Vec<String> = Vec::new();
    let mut check = |ok: bool, claim: &str| {
        if !ok {
            broken.push(claim.to_string());
        }
    };

    check(
        walked_before != walked_after,
        "THE EDIT IS REAL: the playable world begins the two facts at each \
         other's scenes after the trade, so this is a store difference a reader \
         walks through and not a difference of spelling",
    );
    check(
        counts(&walked_before) == counts(&walked_after),
        "INVISIBLE TO THE COUNT: every scene holds as many facts as it did \
         before — the trade is exactly the edit a per-scene number cannot report",
    );
    let moved =
        |before: &BTreeSet<String>, after: &BTreeSet<String>| -> (Vec<String>, Vec<String>) {
            (
                before.difference(after).cloned().collect(),
                after.difference(before).cloned().collect(),
            )
        };
    check(
        moved(&teller.0, &teller.1) == (vec!["kx-open".to_string()], vec![])
            && moved(&child.0, &child.1) == (vec![], vec!["kx-watched".to_string()]),
        "THE LAW: the two views SWAP. The teller stops knowing at the first scene \
         exactly what the child starts knowing there, and nothing else in either \
         view moves — the difference the count aggregates away, and the reason \
         this pair is asked per frame",
    );

    assert_eq!(
        broken,
        Vec::<String>::new(),
        "the frame axis is not what separates these two reads"
    );
}

/// Neither read can be asked the other's question, and the manuscript this pair's
/// playable half embeds carries no seats. (Round 1138, the R1088 discipline.)
///
/// A new contract's first duty is to say why it is not the contracts beside it
/// composed, and to MEASURE it rather than argue it — "the other contract cannot
/// reach this" is exactly the claim that rots silently when the other contract
/// grows. Three refusals, through the real binary.
#[test]
fn neither_read_can_be_asked_the_other_read_s_question() {
    let (sections, order, facts) = constructed_manifests();
    let built = constructed_corpus(&sections, &order, &facts)
        .unwrap_or_else(|e| panic!("the constructed manifests must import: {e}"));
    let ws = built.path();

    let telling_asked_of_the_view = run(
        ws,
        &[
            DECLARES[0],
            "--frame",
            "k-teller",
            "--branch",
            "main",
            "--at",
            "k-01",
            "--telling",
            "k-one",
        ],
    );
    let frame_asked_of_the_world = run(
        ws,
        &[DECLARES[1], "--telling", "k-one", "--frame", "k-teller"],
    );
    let manuscript = run(ws, &["report-playthrough-manuscript", "--json"]);
    assert!(
        manuscript.status.success(),
        "the manuscript answers: {}",
        String::from_utf8_lossy(&manuscript.stderr)
    );
    let manuscript_text = String::from_utf8_lossy(&manuscript.stdout).to_string();
    let playable = run(ws, &[DECLARES[1], "--telling", "k-one", "--json"]);
    assert!(playable.status.success());
    let playable_text = String::from_utf8_lossy(&playable.stdout).to_string();

    println!(
        "the view asked a telling: {:?}\nthe world asked a frame: {:?}\nthe manuscript names \
         locators: {}\nthe playable world names locators: {}",
        String::from_utf8_lossy(&telling_asked_of_the_view.stderr)
            .lines()
            .next(),
        String::from_utf8_lossy(&frame_asked_of_the_world.stderr)
            .lines()
            .next(),
        manuscript_text.contains("locators"),
        playable_text.contains("locators"),
    );

    let mut broken: Vec<String> = Vec::new();
    let mut check = |ok: bool, claim: &str| {
        if !ok {
            broken.push(claim.to_string());
        }
    };

    check(
        !telling_asked_of_the_view.status.success(),
        "THE VIEW HAS NO TELLING: it refuses the flag rather than ignoring it, so \
         every verdict this pair joins is one the disclosure plan cannot have \
         moved",
    );
    check(
        !frame_asked_of_the_world.status.success(),
        "THE WORLD HAS NO FRAME: it refuses the flag, so the epistemic half of \
         every law here is reachable through no other read",
    );
    check(
        !manuscript_text.contains("locators") && playable_text.contains("locators"),
        "AND LAW SEAT IS THIS READ'S ALONE: the playable world embeds the \
         manuscript verbatim (R1048) and adds the locators, so the walk laws \
         above could be stated over either read and the seats could be stated \
         over neither the manuscript nor the frontier — which is what makes this \
         pair more than the two contracts beside it composed",
    );

    assert_eq!(
        broken,
        Vec::<String>::new(),
        "this pair is reachable from the contracts beside it after all"
    );
}
