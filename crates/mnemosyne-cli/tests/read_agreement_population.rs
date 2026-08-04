//! Which subjects does more than one shipped read answer about? (R1039/R1040)
//!
//! Round 1037 found a defect by comparing two shipped reads that answer the
//! same question — `validate-continuity` judged the authored corpus's main
//! quest discharged on three roads while `report-quest-graph` reported
//! `unknown` on all four, a contradiction that had shipped since R568. That
//! comparison was made BY HAND, for one pair, and it hit on the first try. A
//! hand-picked second pair is the mistake this arc has made four times over, so
//! the population is derived here instead.
//!
//! A SUBJECT is an id the store registers. A read's RECORD about it is the
//! minimal enclosing object mentioning that id — what the read wrote about it.
//!
//! TWO DERIVATIONS OF "ANSWERS ABOUT" WERE MEASURED AND DISCARDED FIRST, and
//! both failures are why this one is shaped the way it is:
//!
//! - MENTION. 220 of the corpus's 220 subjects come out shared, because three
//!   reads render the whole store. Co-mention is not co-answering.
//! - A VERDICT FIELD, recognised first by shape ("a few short lowercase
//!   tokens") and then by CLOSURE ("its value set does not grow as more
//!   authored stores are asked"). The shape guess swept in `telling` and
//!   `parent`, ids that happen to look like tokens. The closure measurement
//!   over all 28 loadable corpora did no better, and why is worth keeping: a
//!   field only ONE corpus exercises has a small union whatever it is —
//!   `quest_id` pools to 4 values across 28 stores because one store declares
//!   quests. Whether a field carries a closed vocabulary is a fact about its
//!   TYPE, and no amount of looking at output recovers it.
//!
//! So this walk asks what the perturbation CAN answer, which is what R1037's
//! finding actually rested on: a read ANSWERS ABOUT a subject when its record
//! for that subject CHANGES under some authorable edit. A read that only
//! renders the id answers about nothing it does not itself carry, and the
//! corruption population is derived from the store's own legs rather than aimed
//! at any read.
//!
//! What this walk does NOT claim: that two reads answering about one subject
//! must agree. It says where to look. The pair R1037 examined by hand is in the
//! backlog, and that is the oracle — a derivation that cannot re-find a known
//! positive is a list, not a backlog.

use std::collections::{BTreeMap, BTreeSet};

use mnemosyne_atomic::AtomicStore;

mod common;
use common::{
    ask_panel, corruptions, dnd_quest_facts, dnd_quest_workspace_from, dnd_quest_workspace_try,
    panel, telling_of, Answer, SIDECAR,
};

/// Every id this store registers — what a read could be talking about. Read
/// from the store rather than from the reads, so a read inventing an id it
/// never registered is invisible here rather than counted as a subject.
fn subjects(store: &AtomicStore) -> BTreeSet<String> {
    let mut out: BTreeSet<String> = BTreeSet::new();
    out.extend(store.narrative_facts.keys().map(ToString::to_string));
    out.extend(store.entities.keys().map(ToString::to_string));
    out.extend(store.sections.keys().map(ToString::to_string));
    out.extend(store.branches.keys().map(ToString::to_string));
    out
}

/// The records one read wrote about each subject: for every mention of an id,
/// the MINIMAL enclosing object, serialized so two runs can be compared.
///
/// Minimal, not any: an ancestor object contains every id under it, so letting
/// ancestors claim would make the whole report one record about everything. A
/// nested object that mentions the id claims it and stops the propagation,
/// which is what makes "the quest node" the record about `q-main` rather than
/// "the quest graph".
fn records_by_subject(
    value: &serde_json::Value,
    subjects: &BTreeSet<String>,
    out: &mut BTreeMap<String, BTreeSet<String>>,
) -> BTreeSet<String> {
    match value {
        serde_json::Value::String(s) if subjects.contains(s) => BTreeSet::from([s.clone()]),
        serde_json::Value::Array(items) => items
            .iter()
            .flat_map(|item| records_by_subject(item, subjects, out))
            .collect(),
        serde_json::Value::Object(map) => {
            let mut here: BTreeSet<String> = BTreeSet::new();
            for (key, child) in map {
                // A map KEY is an id too — `per_world` and `worlds` are keyed
                // by world-line, and that is the read writing about it.
                if subjects.contains(key) {
                    here.insert(key.clone());
                }
                here.extend(records_by_subject(child, subjects, out));
            }
            if !here.is_empty() {
                let record = value.to_string();
                for subject in &here {
                    out.entry(subject.clone())
                        .or_default()
                        .insert(record.clone());
                }
            }
            // Claimed here: an ancestor is not also a record about them.
            BTreeSet::new()
        }
        _ => BTreeSet::new(),
    }
}

/// Every read's records, indexed by subject, for one store.
fn panel_records(
    answers: &BTreeMap<String, Answer>,
    subjects: &BTreeSet<String>,
) -> BTreeMap<String, BTreeMap<String, BTreeSet<String>>> {
    let mut out = BTreeMap::new();
    for (verb, answer) in answers {
        if let Answer::Json(json) = answer {
            let mut per_read = BTreeMap::new();
            records_by_subject(json, subjects, &mut per_read);
            out.insert(verb.clone(), per_read);
        }
    }
    out
}

/// The pairs of shipped reads that both ANSWER ABOUT a subject — both of their
/// records for it move under some authorable edit. Most-shared first; this is
/// the backlog, and R1037's hand-picked pair sits inside it.
const BACKLOG: [&str; 28] = [
    "72 report-playable-world <-> report-playthrough-manuscript",
    "53 report-authoring-frontier <-> report-playable-world",
    "53 report-authoring-frontier <-> report-playthrough-manuscript",
    "47 report-edge-candidates <-> report-playable-world",
    "47 report-edge-candidates <-> report-playthrough-manuscript",
    "32 report-edge-candidates <-> report-quest-graph",
    "31 report-authoring-frontier <-> report-edge-candidates",
    "29 report-playable-world <-> report-quest-graph",
    "29 report-playthrough-manuscript <-> report-quest-graph",
    "24 report-authoring-frontier <-> report-quest-graph",
    "13 report-authoring-frontier <-> report-payoff-coverage",
    "12 report-edge-candidates <-> report-payoff-coverage",
    "12 report-edge-candidates <-> report-payoff-substantiation",
    "12 report-payoff-coverage <-> report-payoff-substantiation",
    "10 report-authoring-frontier <-> report-payoff-substantiation",
    "10 report-payoff-coverage <-> report-quest-graph",
    "10 report-payoff-substantiation <-> report-quest-graph",
    "9 report-edge-candidates <-> validate-continuity",
    "9 report-payoff-coverage <-> report-playable-world",
    "9 report-payoff-coverage <-> report-playthrough-manuscript",
    "9 report-payoff-substantiation <-> report-playable-world",
    "9 report-payoff-substantiation <-> report-playthrough-manuscript",
    "9 report-playable-world <-> validate-continuity",
    "9 report-playthrough-manuscript <-> validate-continuity",
    "9 report-quest-graph <-> validate-continuity",
    "7 report-authoring-frontier <-> validate-continuity",
    "3 report-payoff-coverage <-> validate-continuity",
    "3 report-payoff-substantiation <-> validate-continuity",
];

#[test]
fn the_population_of_subjects_more_than_one_shipped_read_answers_about() {
    let facts_json = dnd_quest_facts();
    let ws = dnd_quest_workspace_from(&facts_json);
    let store = AtomicStore::load(&ws.path().join(SIDECAR)).expect("the imported store loads");
    let subjects = subjects(&store);
    let telling = telling_of(&store);
    let (panel, unaskable) = panel(ws.path(), &telling);
    let baseline = ask_panel(ws.path(), &panel);
    assert!(
        baseline.failed.is_empty(),
        "the panel is exactly the reads that answered at baseline: {:?}",
        baseline.failed
    );
    let baseline_records = panel_records(&baseline.answers, &subjects);
    let prose_only: Vec<&str> = baseline
        .answers
        .iter()
        .filter(|(_, a)| matches!(a, Answer::Prose(_)))
        .map(|(verb, _)| verb.as_str())
        .collect();

    // read -> the subjects whose record it MOVED for, over the whole derived
    // corruption population. Moving is what separates answering from rendering.
    let mut responsive: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let population = corruptions(&store, &facts_json);
    let mut applied = 0usize;
    let mut refused = 0usize;
    for corruption in &population {
        let mut mutated = facts_json.clone();
        let mut hits = 0usize;
        for entry in mutated["facts"].as_array_mut().expect("facts array") {
            if entry["fact_id"] == corruption.fact.as_str() {
                (corruption.apply)(entry);
                hits += 1;
            }
        }
        assert_eq!(
            hits, 1,
            "{}/{} applied {hits} times",
            corruption.fact, corruption.leg
        );
        let Ok(mutated_ws) = dnd_quest_workspace_try(&mutated) else {
            // The write path refuses it: not a move an author could make, so it
            // is evidence about nothing here.
            refused += 1;
            continue;
        };
        applied += 1;
        let seen = ask_panel(mutated_ws.path(), &panel);
        let seen_records = panel_records(&seen.answers, &subjects);
        for (verb, before) in &baseline_records {
            let after = seen_records.get(verb);
            for subject in &subjects {
                let was = before.get(subject);
                let now = after.and_then(|per_read| per_read.get(subject));
                if was != now {
                    responsive
                        .entry(verb.clone())
                        .or_default()
                        .insert(subject.clone());
                }
            }
        }
    }

    // subject -> the reads that answer about it.
    let mut readers_of: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for (verb, moved) in &responsive {
        for subject in moved {
            readers_of
                .entry(subject.as_str())
                .or_default()
                .insert(verb.as_str());
        }
    }
    let shared: BTreeMap<&str, &BTreeSet<&str>> = readers_of
        .iter()
        .filter(|(_, reads)| reads.len() > 1)
        .map(|(subject, reads)| (*subject, reads))
        .collect();

    // THE BACKLOG: which PAIR of reads answers about the most subjects in
    // common. R1037 compared one pair by hand; this says which pair to compare
    // next, and how much of the store rides on each answer agreeing.
    let mut overlap: BTreeMap<(&str, &str), usize> = BTreeMap::new();
    for reads in shared.values() {
        let listed: Vec<&str> = reads.iter().copied().collect();
        for (i, a) in listed.iter().enumerate() {
            for b in &listed[i + 1..] {
                *overlap.entry((a, b)).or_default() += 1;
            }
        }
    }
    let mut ranked: Vec<((&str, &str), usize)> = overlap.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

    // Print BEFORE asserting — the distribution is the point, and a
    // first-violation stop would report one line of it (the R1026 lesson).
    println!(
        "{} registered subjects; {} corruptions applied, {} refused by the write \
         path; {} reads asked, {} answering `--json` in prose ({}), {} unaskable\n",
        subjects.len(),
        applied,
        refused,
        baseline_records.len(),
        prose_only.len(),
        prose_only.join(" "),
        unaskable.len(),
    );
    println!("per read: subjects MENTIONED at baseline / ANSWERED ABOUT (record moved):");
    for (verb, per_read) in &baseline_records {
        println!(
            "  {:38} {:4} / {:4}",
            verb,
            per_read.len(),
            responsive.get(verb).map_or(0, BTreeSet::len),
        );
    }
    println!(
        "\n{} of {} subjects are answered about by MORE THAN ONE read",
        shared.len(),
        readers_of.len(),
    );
    println!("\nread pairs by shared subjects — the backlog, most-shared first:");
    for ((a, b), n) in &ranked {
        println!("  {n:4}  {a} <-> {b}");
    }

    // EVERY CHECK RUNS AND THE FAILURES COME OUT AS A LIST. Stopping at the
    // first makes an injection report one line about a walk with six
    // independent claims and hides which are alive — seven injections into the
    // R1039 form landed on two assertions between them. The R1026 rule, applied
    // to this test's OWN claims rather than to its findings.
    let mut broken: Vec<String> = Vec::new();
    let mut check = |ok: bool, claim: &str| {
        if !ok {
            broken.push(claim.to_string());
        }
    };

    check(
        ranked
            .iter()
            .any(|((a, b), _)| (*a, *b) == ("report-quest-graph", "validate-continuity")),
        "ORACLE: the derivation reproduces the one pair a hand comparison already \
         proved was worth making",
    );
    check(
        shared.get("q-main").is_some_and(|reads| {
            reads.contains("report-quest-graph") && reads.contains("validate-continuity")
        }),
        "ORACLE: it names `q-main`, the subject R1037's defect was about, rather \
         than finding the right pair for the wrong reason",
    );
    check(
        (subjects.len(), baseline_records.len(), prose_only.len()) == (220, 27, 1),
        "INPUTS: 220 registered ids, 27 reads holding records, 1 answering \
         `--json` in prose",
    );
    check(
        (applied, refused) == (41, 0),
        "POPULATION: 41 authorable corruptions, none refused by the write path",
    );
    // MENTION IS VACUOUS AND THE WALK SAYS SO EVERY RUN rather than in prose:
    // every registered id is mentioned by more than one read at baseline,
    // because three reads render the whole store. If that stops holding, the
    // reason this walk perturbs instead of reading has changed.
    check(
        subjects.iter().all(|subject| {
            baseline_records
                .values()
                .filter(|per_read| per_read.contains_key(subject))
                .count()
                > 1
        }),
        "VACUITY: keyed on mention alone EVERY subject is shared, which is why \
         answering is measured by perturbation",
    );
    check(
        (readers_of.len(), shared.len()) == (98, 79),
        "ANSWERED: subjects some read answers about, and those more than one does",
    );
    check(
        ranked
            .iter()
            .map(|((a, b), n)| format!("{n} {a} <-> {b}"))
            .collect::<Vec<_>>()
            == BACKLOG,
        "BACKLOG: every read pair that answers about a subject in common, \
         most-shared first",
    );

    assert_eq!(
        broken,
        Vec::<String>::new(),
        "the derived read-agreement population no longer holds"
    );
}
