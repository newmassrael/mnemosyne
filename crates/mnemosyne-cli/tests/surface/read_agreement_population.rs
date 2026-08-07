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
//! A SUBJECT is an id the store registers. A read's RECORD about it is what
//! that read WROTE ABOUT IT, and where a read writes an id decides what that
//! is:
//!
//! - as a field's VALUE, the minimal enclosing object — the row is the
//!   sentence, and `{"quest": "q-main", "state": "unknown"}` says `unknown`
//!   about `q-main`;
//! - as a KEY of a map, the value under that key — a report holding a subtree
//!   per road said the one under `r-1` about `r-1`, not the map that holds
//!   every other road beside it;
//! - as a bare element of a LIST, its MEMBERSHIP of that list, addressed down
//!   through the ancestors that key it: `scene_coverage[scene=sec-3].facts[]`.
//!
//! The third is Round 1055 and the first two are the same rule finally stated.
//! Until here EVERY occurrence claimed the minimal enclosing object, so a fact
//! listed in a scene's census carried the whole census row as its record and
//! moved whenever any of its NEIGHBOURS did. Round 1054 made three reads name
//! the sets they had been counting, each list holding most of the store, and
//! the number of subjects more than one read answers about went 79 -> 210 out
//! of 214 in one round: a backlog ranked by co-answering had become a backlog
//! ranked by co-ENUMERATING, and the backlog is what says which pair to build
//! the next contract over. The obvious repair — a list member's record is its
//! own membership — is wrong as stated, and the census is the refutation: it
//! lists facts PER SCENE, so a record that forgets the scene cannot see a fact
//! move to another one. Hence the address, and the key of an array row is
//! DERIVED (the fields holding a subject id in every row, distinct across
//! them) rather than named per read. The rule's own test is not here: it judges
//! [`common::wrote_about`], which two walks stand on, so it lives beside the
//! definition in `answer_addressing.rs` (Round 1057).
//!
//! What a membership record cannot see is ORDER, and this walk asserts that
//! rather than assuming it: the sweep watches every listed set for a
//! permutation that leaves its membership alone, and finds none. The day one
//! appears, position belongs in the address and the walk says so by going red.
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

use crate::common;
use crate::sweep;
use common::{permutations, registered_ids, wrote_about, Wrote};
use sweep::Answer;

/// Every read's records for one store. The derivation itself is
/// [`common::wrote_about`] — since Round 1056 the census of lossy numbers needs
/// the same one, and a second copy is how the two would come to disagree about
/// what a read said about which id.
fn panel_records(
    answers: &BTreeMap<String, Answer>,
    subjects: &BTreeSet<String>,
) -> BTreeMap<String, Wrote> {
    let mut out = BTreeMap::new();
    for (verb, answer) in answers {
        if let Answer::Json(json) = answer {
            out.insert(verb.clone(), wrote_about(json, subjects));
        }
    }
    out
}

/// The pairs of shipped reads that both ANSWER ABOUT a subject — both of their
/// records for it move under some authorable edit. Most-shared first; this is
/// the backlog, and R1037's hand-picked pair sits inside it.
const BACKLOG: [&str; 87] = [
    "200 report-authoring-frontier <-> report-playable-world",
    "179 report-playable-world <-> report-playthrough-manuscript",
    "158 report-authoring-frontier <-> report-playthrough-manuscript",
    "135 report-frame-view <-> report-playable-world",
    "135 report-frame-view <-> report-playthrough-manuscript",
    "114 report-authoring-frontier <-> report-frame-view",
    "98 report-authoring-frontier <-> report-payoff-coverage",
    "98 report-payoff-coverage <-> report-playable-world",
    "92 report-payoff-coverage <-> report-playthrough-manuscript",
    "83 report-frame-view <-> report-payoff-coverage",
    "81 report-playable-world <-> report-quest-graph",
    "81 report-playthrough-manuscript <-> report-quest-graph",
    "74 report-authoring-frontier <-> report-quest-graph",
    "62 report-authoring-frontier <-> validate-continuity",
    "62 report-playable-world <-> validate-continuity",
    "62 report-playthrough-manuscript <-> validate-continuity",
    "62 report-quest-graph <-> validate-continuity",
    "59 report-entity <-> report-playable-world",
    "59 report-entity <-> report-playthrough-manuscript",
    "58 report-edge-candidates <-> report-playable-world",
    "56 report-authoring-frontier <-> report-coverage",
    "56 report-authoring-frontier <-> report-spec-map",
    "56 report-coverage <-> report-playable-world",
    "56 report-coverage <-> report-playthrough-manuscript",
    "56 report-coverage <-> report-quest-graph",
    "56 report-coverage <-> report-spec-map",
    "56 report-coverage <-> validate-continuity",
    "56 report-edge-candidates <-> report-playthrough-manuscript",
    "56 report-playable-world <-> report-spec-map",
    "56 report-playthrough-manuscript <-> report-spec-map",
    "56 report-quest-graph <-> report-spec-map",
    "56 report-spec-map <-> validate-continuity",
    "51 report-authoring-frontier <-> report-edge-candidates",
    "46 report-entity <-> report-frame-view",
    "45 report-edge-candidates <-> report-entity",
    "45 report-frame-view <-> report-quest-graph",
    "44 report-authoring-frontier <-> report-entity",
    "41 report-entity <-> report-quest-graph",
    "39 report-edge-candidates <-> report-frame-view",
    "36 report-edge-candidates <-> report-quest-graph",
    "31 report-edge-candidates <-> report-payoff-coverage",
    "30 report-authoring-frontier <-> report-payoff-substantiation",
    "30 report-frame-view <-> validate-continuity",
    "30 report-payoff-coverage <-> report-payoff-substantiation",
    "30 report-payoff-substantiation <-> report-playable-world",
    "26 report-coverage <-> report-frame-view",
    "26 report-frame-view <-> report-spec-map",
    "24 report-entity <-> report-payoff-coverage",
    "24 report-payoff-substantiation <-> report-playthrough-manuscript",
    "22 report-entity <-> validate-continuity",
    "20 report-edge-candidates <-> report-payoff-substantiation",
    "20 report-edge-candidates <-> validate-continuity",
    "17 report-frame-view <-> report-payoff-substantiation",
    "16 report-coverage <-> report-entity",
    "16 report-entity <-> report-spec-map",
    "14 report-authoring-frontier <-> validate-render-fidelity",
    "14 report-coverage <-> report-edge-candidates",
    "14 report-edge-candidates <-> report-spec-map",
    "14 report-edge-candidates <-> report-typing-candidates",
    "14 report-entity <-> report-payoff-substantiation",
    "14 report-entity <-> report-typing-candidates",
    "14 report-payoff-coverage <-> report-quest-graph",
    "14 report-playable-world <-> report-typing-candidates",
    "14 report-playable-world <-> validate-render-fidelity",
    "14 report-playthrough-manuscript <-> report-typing-candidates",
    "14 report-playthrough-manuscript <-> validate-render-fidelity",
    "13 report-authoring-frontier <-> report-typing-candidates",
    "13 report-frame-view <-> report-typing-candidates",
    "12 report-quest-graph <-> report-typing-candidates",
    "11 report-payoff-substantiation <-> report-quest-graph",
    "8 report-quest-graph <-> validate-render-fidelity",
    "8 report-typing-candidates <-> validate-continuity",
    "8 validate-continuity <-> validate-render-fidelity",
    "7 report-payoff-coverage <-> report-typing-candidates",
    "7 report-payoff-coverage <-> validate-render-fidelity",
    "7 report-payoff-substantiation <-> report-typing-candidates",
    "6 report-coverage <-> report-typing-candidates",
    "6 report-frame-view <-> validate-render-fidelity",
    "6 report-spec-map <-> report-typing-candidates",
    "5 report-edge-candidates <-> validate-render-fidelity",
    "5 report-entity <-> validate-render-fidelity",
    "4 report-coverage <-> validate-render-fidelity",
    "4 report-payoff-coverage <-> validate-continuity",
    "4 report-payoff-substantiation <-> validate-continuity",
    "4 report-payoff-substantiation <-> validate-render-fidelity",
    "4 report-spec-map <-> validate-render-fidelity",
    "3 report-typing-candidates <-> validate-render-fidelity",
];

#[test]
fn the_population_of_subjects_more_than_one_shipped_read_answers_about() {
    // ONE SWEEP, THREE LAWS (Round 1071): the corpus, the panel and every
    // trial are built once for this binary and read here.
    let sweep = sweep::sweep();
    let subjects = registered_ids(&sweep.store);
    let panel = &sweep.panel;
    let unaskable = &sweep.unaskable;
    let baseline = &sweep.baseline;
    let baseline_records = panel_records(&baseline.answers, &subjects);
    // The panel is keyed by read-and-question since Round 1051 — one verb can
    // be asked several. Every verdict below is about the READ, so the label
    // maps back to it and the answers a verb gave to its several questions are
    // unioned: a read ANSWERS ABOUT a subject when SOME question it can be
    // asked moves its record.
    let verb_of: BTreeMap<String, String> = panel
        .iter()
        .map(|read| (read.label(), read.verb.clone()))
        .collect();
    let prose_only: BTreeSet<&str> = baseline
        .answers
        .iter()
        .filter(|(_, a)| matches!(a, Answer::Prose(_)))
        .map(|(label, _)| verb_of[label].as_str())
        .collect();
    let prose_only: Vec<&str> = prose_only.into_iter().collect();
    // read -> every subject any of its questions MENTIONED at baseline.
    let mut mentioned: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (label, per_read) in &baseline_records {
        mentioned
            .entry(verb_of[label].clone())
            .or_default()
            .extend(per_read.by_subject.keys().cloned());
    }
    // Every address this derivation could not key by a field, and every
    // address it produced twice — the two ways the addressing could be
    // unsound, measured over the whole panel rather than reasoned about.
    let unaddressed: BTreeSet<String> = baseline_records
        .iter()
        .flat_map(|(label, per_read)| {
            per_read
                .unaddressed
                .iter()
                .map(move |row| format!("{label}: {row}"))
        })
        .collect();
    let mut collisions: usize = baseline_records
        .values()
        .map(|per_read| per_read.collisions)
        .sum();

    // read -> the subjects whose record it MOVED for, over the whole derived
    // corruption population. Moving is what separates answering from rendering.
    let mut responsive: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    // Lists that came back holding the same ids in a different order.
    let mut permuted: BTreeSet<String> = BTreeSet::new();
    let mut applied = 0usize;
    let mut refused = 0usize;
    // Reads that stopped answering under some edit. `order.json` is read at read
    // time rather than imported, so a road an author could not have meant is
    // refused HERE and nowhere else; a walk that did not look would record the
    // subjects that read went quiet about as subjects it does not answer about.
    let mut read_failures: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for trial in &sweep.trials {
        let corruption = &trial.corruption;
        let Ok(seen) = &trial.seen else {
            // The write path refuses it: not a move an author could make, so it
            // is evidence about nothing here.
            refused += 1;
            continue;
        };
        applied += 1;
        // Parsed for this iteration and dropped with it — the sweep holds the
        // bytes, never 312 parsed panels.
        let seen = seen.parsed();
        for label in &seen.failed {
            read_failures
                .entry(verb_of[label].clone())
                .or_default()
                .insert(corruption.label());
        }
        let seen_records = panel_records(&seen.answers, &subjects);
        for (label, before) in &baseline_records {
            let after = seen_records.get(label);
            for subject in &subjects {
                let was = before.by_subject.get(subject);
                let now = after.and_then(|per_read| per_read.by_subject.get(subject));
                if was != now {
                    responsive
                        .entry(verb_of[label].clone())
                        .or_default()
                        .insert(subject.clone());
                }
            }
            // THE ONE THING A MEMBERSHIP RECORD CANNOT SEE — asked on every
            // corruption, over every list, rather than assumed.
            let Some(after) = after else { continue };
            permuted.extend(
                permutations(before, after)
                    .into_iter()
                    .map(|address| format!("{label}: {address}")),
            );
            collisions += after.collisions;
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
         path; {} reads asked over {} questions, {} answering `--json` in prose \
         ({}), {} UNASKABLE:\n",
        subjects.len(),
        applied,
        refused,
        mentioned.len(),
        baseline_records.len(),
        prose_only.len(),
        prose_only.join(" "),
        unaskable.len(),
    );
    // NAMED, not counted. Round 1051: this line printed a bare count, and the
    // four reads behind it were outside every population this walk derives —
    // including the backlog that says which pair to compare next. An exclusion
    // nobody can read is an exclusion nobody removes (the R1029 rule).
    for (verb, reason) in unaskable {
        println!("  UNASKABLE {verb}: {reason}");
    }
    // The addressing's own two failure modes, printed every run: a row no
    // field keys is addressed by where it sits, and an address produced twice
    // would merge two lists into one record.
    println!(
        "  ADDRESSING: {} rows addressed by position, {collisions} address collisions",
        unaddressed.len(),
    );
    // A read that stopped answering did not stop ANSWERING ABOUT — it refused
    // the store. Printed so a subject that leaves a read's population is read as
    // the refusal it is rather than as silence.
    for (verb, edits) in &read_failures {
        println!(
            "  REFUSED THE STORE: {verb} under {} edit(s), e.g. {}",
            edits.len(),
            edits.iter().next().expect("a named edit"),
        );
    }
    for row in &unaddressed {
        println!("    BY POSITION {row}");
    }
    for list in &permuted {
        println!("    PERMUTED {list}");
    }
    println!("per read: subjects MENTIONED at baseline / ANSWERED ABOUT (record moved):");
    for (verb, subjects_seen) in &mentioned {
        println!(
            "  {:38} {:4} / {:4}",
            verb,
            subjects_seen.len(),
            responsive.get(verb).map_or(0, BTreeSet::len),
        );
    }
    println!(
        "\n{} of {} subjects are answered about by MORE THAN ONE read",
        shared.len(),
        readers_of.len(),
    );
    // NAMED, not counted (the R1029 rule): until here the count said some
    // subjects were unreached without saying which. Read it as a fact about
    // this POPULATION and not about the surface — the corruptions are the quest
    // layer's legs, so an id no quest fact touches cannot be moved by any of
    // them, and these five are entities of that kind.
    let unanswered: Vec<&str> = subjects
        .iter()
        .map(String::as_str)
        .filter(|subject| !readers_of.contains_key(subject))
        .collect();
    println!(
        "{} subjects no read answers about UNDER THIS POPULATION: {}",
        unanswered.len(),
        unanswered.join(" "),
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
        (
            subjects.len(),
            mentioned.len(),
            baseline_records.len(),
            prose_only.len(),
        ) == (221, 31, 77, 1),
        "INPUTS: 221 registered ids, 31 reads holding records over 77 \
         QUESTIONS, 1 answering `--json` in prose. Round 1051: the panel used \
         to guess a read's arguments in two shapes — nothing, or a telling — so \
         two reads were counted as an unaskable NUMBER and never entered any \
         population here, and every swept read was asked at ONE point of its \
         argument space. The 221st id is `main`, and it arrived because this \
         walk and the R1054 census had each derived `the ids the store \
         registers` by hand and DISAGREED about exactly that one — a world \
         every store has whether or not it registers a branch. One resolver \
         now (`common::registered_ids`), and what the disagreement cost is \
         below in ADDRESSING",
    );
    check(
        (applied, refused) == (256, 56),
        "POPULATION: 256 authorable corruptions applied and 56 the write path \
         refuses. It was 41 until Round 1054, which added the legs that PLACE a \
         fact and ATTRIBUTE it, and 92 until Round 1061, which stopped taking \
         the FACT manifest for the corpus: an author writes three, and the \
         other two hold the scene registry and the canon order. The 56 refusals \
         are one leg — cutting a scene out of the registry, which the fact \
         import rejects because facts stand at it",
    );
    // MENTION IS VACUOUS AND THE WALK SAYS SO EVERY RUN rather than in prose:
    // every registered id is mentioned by more than one read at baseline,
    // because three reads render the whole store. If that stops holding, the
    // reason this walk perturbs instead of reading has changed.
    check(
        subjects.iter().all(|subject| {
            baseline_records
                .values()
                .filter(|per_read| per_read.by_subject.contains_key(subject))
                .count()
                > 1
        }),
        "VACUITY: keyed on mention alone EVERY subject is shared, which is why \
         answering is measured by perturbation",
    );
    check(
        (readers_of.len(), shared.len()) == (221, 221),
        "ANSWERED: subjects some read answers about, and those more than one \
         does. The second number was 79, then 210 of 214 when Round 1054 made \
         three reads NAME the sets they had been counting, then 105 of 216 when \
         Round 1055 gave a bare id in a list the ADDRESS of that list as its \
         record, so a co-listed neighbour moving stopped being this subject's \
         record moving.\n\n\
         BOTH ARE 221 NOW — every registered id, answered about by more than \
         one read — and the reason is the population rather than the reads. \
         Round 1061 widened the authorable edit set past the fact manifest to \
         the corpus an author writes, and the 56 SECTIONS are registered ids \
         that nothing could move until an edit could reach the scene registry \
         and the canon order. A subject no edit can move is a subject no read \
         can be shown to answer about; those 116 were never evidence that the \
         reads were silent about scenes, only that the walk could not ask",
    );
    check(
        permuted.is_empty(),
        "ORDER: no authorable edit permutes a listed set while leaving its \
         membership alone — the one thing a membership record cannot see, \
         measured over every corruption and every list rather than assumed. A \
         read that ORDERS the ids it lists would need position in the address, \
         and this is the assertion that would say so",
    );
    check(
        unaddressed.is_empty() && collisions == 0,
        "ADDRESSING: every row of an array of objects is keyed by a field \
         holding a subject id that is distinct across the rows, so no record is \
         addressed by the position it happens to sit at, and no two places in \
         one answer collide on one address",
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
