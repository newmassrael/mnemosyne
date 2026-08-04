//! Which subjects does more than one shipped read answer about? (R1039)
//!
//! Round 1037 found a defect by comparing two shipped reads that answer the
//! same question — `validate-continuity` judged the authored corpus's main
//! quest discharged on three roads while `report-quest-graph` reported
//! `unknown` on all four, a contradiction that had shipped since R568. That
//! comparison was made BY HAND, for one pair, and it hit on the first try.
//!
//! A hand-picked second pair would be the same mistake this arc has made four
//! times (R1026 / R1027 / R1029 / R1030: "the remaining axis is X" was a
//! hypothesis every time, and wrong every time). So the population is derived
//! instead, and this walk is what derives it.
//!
//! A SUBJECT is an id the store registers — a fact, an entity, a section, a
//! world-line. A read's RECORD about a subject is the MINIMAL enclosing object
//! mentioning that id, because that is what the read wrote about it.
//!
//! MENTIONING IS NOT ANSWERING, and this walk measured that before it relied on
//! it: keyed on mention alone, 220 of the corpus's 220 subjects are "shared",
//! because `report-playable-world`, `report-playthrough-manuscript` and
//! `report-edge-candidates` each render the whole store. A backlog built on
//! that is noise, and it is the R1034 lesson one level up — a rendering read
//! must not be allowed to masquerade as one that has an opinion.
//!
//! So a read ANSWERS ABOUT a subject when its record carries a VERDICT: a field
//! whose values across that read's whole output form a small closed set of
//! short tokens, none of which is a registered id. That is what
//! `report-quest-graph` does with `state: done|open|unknown` and
//! `validate-continuity` with `verdict: satisfied|inapplicable|…` — the two
//! fields whose disagreement R1037 found. The verdict fields are DERIVED from
//! each read's own output and printed, so the walk can be checked for having
//! invented them rather than found them.
//!
//! What this walk does NOT claim: that every shared subject is a disagreement
//! waiting to happen. Two reads can judge one fact along axes with nothing to
//! say to each other. The population is the place to look, not the finding —
//! and the read PAIR table is what says where to look first.

use std::collections::{BTreeMap, BTreeSet};

use mnemosyne_atomic::AtomicStore;

mod common;
use common::{
    ask_panel, dnd_quest_facts, dnd_quest_workspace_from, panel, telling_of, Answer, SIDECAR,
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

/// Every field in one read's output whose values look like a VERDICT: a small
/// closed set of short lowercase tokens, none of them a registered id.
///
/// Derived from the read's own output rather than declared here. Rust cannot
/// enumerate the types that implement the store's `closed_vocabulary!` (R1027),
/// and a hand table of verdict field names is exactly the keyed-by-name gate
/// population this repository has been removing — so the tokens are recognised
/// by their SHAPE in the data, and printed for the reader to check.
fn verdict_fields(value: &serde_json::Value, subjects: &BTreeSet<String>) -> BTreeSet<String> {
    let mut values_of: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    fn walk(
        value: &serde_json::Value,
        values_of: &mut BTreeMap<String, BTreeSet<String>>,
        key: Option<&str>,
    ) {
        match value {
            serde_json::Value::String(s) => {
                if let Some(k) = key {
                    values_of
                        .entry(k.to_string())
                        .or_default()
                        .insert(s.clone());
                }
            }
            serde_json::Value::Array(items) => {
                for item in items {
                    walk(item, values_of, key);
                }
            }
            serde_json::Value::Object(map) => {
                for (k, child) in map {
                    walk(child, values_of, Some(k));
                }
            }
            _ => {}
        }
    }
    walk(value, &mut values_of, None);
    values_of
        .into_iter()
        .filter(|(_, seen)| {
            !seen.is_empty()
                && seen.len() <= 8
                && seen.iter().all(|v| {
                    !v.is_empty()
                        && v.len() <= 24
                        && !subjects.contains(v)
                        && v.chars()
                            .all(|c| c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit())
                })
        })
        .map(|(field, _)| field)
        .collect()
}

/// The records one read wrote about each subject: for every mention of an id,
/// the MINIMAL enclosing object, serialized so two runs can be compared.
///
/// Minimal, not any: an ancestor object contains every id under it, so letting
/// ancestors claim would make the whole report one answer about everything. A
/// nested object that mentions the id claims it and stops the propagation,
/// which is what makes "the quest node" the answer about `q-main` rather than
/// "the quest graph".
fn answers_about(
    value: &serde_json::Value,
    subjects: &BTreeSet<String>,
    out: &mut BTreeMap<String, BTreeSet<String>>,
) -> BTreeSet<String> {
    match value {
        serde_json::Value::String(s) if subjects.contains(s) => BTreeSet::from([s.clone()]),
        serde_json::Value::Array(items) => items
            .iter()
            .flat_map(|item| answers_about(item, subjects, out))
            .collect(),
        serde_json::Value::Object(map) => {
            let mut here: BTreeSet<String> = BTreeSet::new();
            for (key, child) in map {
                // A map KEY is an id too — `per_world` and `worlds` are keyed
                // by world-line, and that is the read answering about it.
                if subjects.contains(key) {
                    here.insert(key.clone());
                }
                here.extend(answers_about(child, subjects, out));
            }
            if !here.is_empty() {
                let record = value.to_string();
                for subject in &here {
                    out.entry(subject.clone())
                        .or_default()
                        .insert(record.clone());
                }
            }
            // Claimed here: an ancestor is not also an answer about them.
            BTreeSet::new()
        }
        _ => BTreeSet::new(),
    }
}

/// The pairs of shipped reads that judge a subject in common, most-shared
/// first — the backlog R1037's hand-picked pair sits inside.
const BACKLOG: [&str; 16] = [
    "56 report-playable-world <-> report-spec-map",
    "17 report-playable-world <-> report-quest-graph",
    "13 report-playable-world <-> report-playthrough-manuscript",
    "10 report-edge-candidates <-> report-playable-world",
    "10 report-edge-candidates <-> report-playthrough-manuscript",
    "9 report-playable-world <-> validate-continuity",
    "4 report-quest-graph <-> report-spec-map",
    "4 report-quest-graph <-> validate-continuity",
    "4 report-spec-map <-> validate-continuity",
    "2 report-edge-candidates <-> validate-continuity",
    "2 report-playthrough-manuscript <-> validate-continuity",
    "1 report-edge-candidates <-> report-quest-graph",
    "1 report-fork-tree <-> report-playable-world",
    "1 report-fork-tree <-> report-quest-graph",
    "1 report-fork-tree <-> report-spec-map",
    "1 report-playthrough-manuscript <-> report-quest-graph",
];

#[test]
fn the_population_of_subjects_more_than_one_shipped_read_answers_about() {
    let ws = dnd_quest_workspace_from(&dnd_quest_facts());
    let store = AtomicStore::load(&ws.path().join(SIDECAR)).expect("the imported store loads");
    let subjects = subjects(&store);
    assert!(
        subjects.len() > 100,
        "the corpus registers {} ids, which is a store that stopped loading \
         rather than one with nothing in it",
        subjects.len()
    );

    let telling = telling_of(&store);
    let (panel, unaskable) = panel(ws.path(), &telling);
    let asked = ask_panel(ws.path(), &panel);
    assert!(
        asked.failed.is_empty(),
        "the panel is exactly the reads that answered at baseline: {:?}",
        asked.failed
    );

    // read -> subject -> the records that read wrote about it, and the verdict
    // fields that read uses at all.
    let mut by_read: BTreeMap<&str, BTreeMap<String, BTreeSet<String>>> = BTreeMap::new();
    let mut verdicts_of: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    let mut prose_only: Vec<&str> = Vec::new();
    for (verb, answer) in &asked.answers {
        match answer {
            Answer::Prose(_) => prose_only.push(verb.as_str()),
            Answer::Json(json) => {
                let mut per_read = BTreeMap::new();
                answers_about(json, &subjects, &mut per_read);
                by_read.insert(verb.as_str(), per_read);
                verdicts_of.insert(verb.as_str(), verdict_fields(json, &subjects));
            }
        }
    }

    // subject -> the reads that ANSWER about it: mention alone is rendering,
    // so a record only counts when it carries one of that read's verdict
    // fields. Measured first with mention alone, which made every subject
    // shared — see the module note.
    let mut readers_of: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    let mut mentioners_of: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for (verb, per_read) in &by_read {
        let verdicts = &verdicts_of[verb];
        for (subject, records) in per_read {
            mentioners_of.entry(subject).or_default().insert(verb);
            let judged = records.iter().any(|record| {
                let parsed: serde_json::Value =
                    serde_json::from_str(record).expect("a record round-trips");
                parsed
                    .as_object()
                    .is_some_and(|map| map.keys().any(|k| verdicts.contains(k)))
            });
            if judged {
                readers_of.entry(subject).or_default().insert(verb);
            }
        }
    }
    let shared: BTreeMap<&str, &BTreeSet<&str>> = readers_of
        .iter()
        .filter(|(_, reads)| reads.len() > 1)
        .map(|(subject, reads)| (*subject, reads))
        .collect();

    // The BACKLOG, and the whole reason the population is worth deriving: which
    // PAIR of reads answers about the most subjects in common. R1037 compared
    // one pair by hand; this says which pair to compare next, and how much of
    // the store rides on each answer agreeing.
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

    // Print BEFORE asserting — the distribution is the whole point, and a
    // first-violation stop would report one line of it (the R1026 lesson).
    println!(
        "{} registered subjects, {} json reads asked ({} answered in prose and \
         hold no record: {}), {} unaskable by this corpus\n",
        subjects.len(),
        by_read.len(),
        prose_only.len(),
        prose_only.join(" "),
        unaskable.len(),
    );
    println!("per read: subjects MENTIONED / ANSWERED ABOUT, and its verdict fields:");
    for (verb, per_read) in &by_read {
        println!(
            "  {:38} {:4} / {:4}  {:?}",
            verb,
            per_read.len(),
            readers_of
                .values()
                .filter(|reads| reads.contains(verb))
                .count(),
            verdicts_of[verb],
        );
    }
    println!(
        "\n{} subjects are MENTIONED by more than one read (the vacuous reading); \
         {} of {} are ANSWERED ABOUT by more than one",
        mentioners_of.values().filter(|r| r.len() > 1).count(),
        shared.len(),
        readers_of.len(),
    );
    println!("\nread pairs by shared subjects — the backlog, most-shared first:");
    for ((a, b), n) in &ranked {
        println!("  {n:4}  {a} <-> {b}");
    }

    // EVERY CHECK RUNS, AND THE FAILURES COME OUT AS A LIST. Stopping at the
    // first one makes an injection report a single line about a walk with six
    // independent claims, and it hides which of them are alive: seven
    // injections into this walk landed on two assertions between them, and the
    // rest had no evidence of being exercised at all. That is the R1026 lesson
    // applied to this test's OWN assertions rather than to its findings.
    let mut broken: Vec<String> = Vec::new();
    let mut check = |ok: bool, claim: &str| {
        if !ok {
            broken.push(claim.to_string());
        }
    };

    // THE ORACLE. R1037 found a real, shipped contradiction between
    // `report-quest-graph` and `validate-continuity` by picking that pair BY
    // HAND. A derivation that cannot reproduce a known positive is not a
    // backlog, it is a list — so the pair whose defect is already known must be
    // in it, and the subjects it names must include the quest that carried it.
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
        (subjects.len(), by_read.len(), prose_only.len()) == (220, 27, 1),
        "INPUTS: 220 registered ids, 27 reads holding records, 1 answering \
         `--json` in prose",
    );
    // MENTION IS VACUOUS AND THE WALK SAYS SO EVERY RUN, rather than in prose:
    // every registered id is mentioned by more than one read, because three
    // reads render the whole store. If this stops holding, the vacuity argument
    // has changed and the verdict filter needs re-justifying.
    check(
        mentioners_of
            .values()
            .filter(|reads| reads.len() > 1)
            .count()
            == subjects.len(),
        "VACUITY: keyed on mention alone EVERY subject is shared, which is why \
         the verdict filter exists",
    );
    check(
        (readers_of.len(), shared.len()) == (206, 81),
        "JUDGED: 206 subjects some read judges, 81 that more than one judges",
    );
    // THE BACKLOG, pinned by name. A new read, a read that starts or stops
    // judging a subject, or a corpus that grows one all move this — which is the
    // point: the next pair to compare is re-derived every run, not remembered.
    check(
        ranked
            .iter()
            .map(|((a, b), n)| format!("{n} {a} <-> {b}"))
            .collect::<Vec<_>>()
            == BACKLOG,
        "BACKLOG: every read pair that judges a subject in common, most-shared \
         first",
    );

    assert_eq!(
        broken,
        Vec::<String>::new(),
        "the derived read-agreement population no longer holds"
    );
}
