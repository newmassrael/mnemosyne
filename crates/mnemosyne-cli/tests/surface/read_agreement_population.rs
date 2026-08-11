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

/// Which pair one contract declares it judges, or the reason this walk could
/// not tell.
///
/// A real parser rather than a search for the verb's name: every one of these
/// files NAMES other reads in its header prose — the arc's history is written
/// there — so a text match would report a pair as judged because a comment
/// mentioned it. `syn` sees the `const DECLARES` item and nothing a comment
/// says. The pair comes back in the order the backlog sorts, so a contract may
/// name its two reads whichever way round reads best.
fn declared_in(source: &str) -> Result<Option<(String, String)>, String> {
    let file = syn::parse_file(source).map_err(|e| e.to_string())?;
    for item in &file.items {
        let syn::Item::Const(declaration) = item else {
            continue;
        };
        if declaration.ident != "DECLARES" {
            continue;
        }
        let syn::Expr::Array(array) = &*declaration.expr else {
            return Err("DECLARES is not an array of two read verbs".to_string());
        };
        let verbs: Vec<String> = array
            .elems
            .iter()
            .map(|element| match element {
                syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(verb),
                    ..
                }) => Ok(verb.value()),
                other => Err(format!("{other:?} is not a read verb spelled out")),
            })
            .collect::<Result<_, _>>()?;
        let [a, b] = verbs.as_slice() else {
            return Err(format!("DECLARES names {} verb(s), not two", verbs.len()));
        };
        return Ok(Some(if a <= b {
            (a.clone(), b.clone())
        } else {
            (b.clone(), a.clone())
        }));
    }
    Ok(None)
}

/// Every pair some contract in this crate's test suite declares it judges, and
/// the files a declaration was expected from and not found in.
///
/// THE DECLARATION LIVES IN THE CONTRACT. A list of judged pairs kept beside
/// this walk would be a second thing to update, and this whole round exists
/// because the FIRST one went stale: the top two rows of [`BACKLOG`] were
/// declared in Rounds 1048 and 1052 and nothing here knew, so "the pair to
/// compare next" named work already shipped. A contract states its pair in one
/// `const DECLARES` that the contract itself runs on, and this reads that
/// statement out of the source — one place to write it, and it cannot say a
/// pair the test does not ask.
///
/// The second return is the safety net in the direction that matters: a file
/// this repository named `*agreement*` and that declares nothing is a contract
/// this walk cannot see, which is how the list would go stale again.
///
/// The directory is an argument because every contract in this tree DOES
/// declare, so pointed at the real one the net's finding arm never runs and no
/// test could reach it — a calculation with nothing aimed at it, which is the
/// defect Round 1086 closed one crate over and this is the same repair. The
/// caller passes `tests/`; the tests below pass trees built to make each arm
/// fire.
fn declared_pairs(
    tests: &std::path::Path,
) -> (BTreeSet<(String, String)>, Vec<String>, Vec<String>) {
    let mut declared = BTreeSet::new();
    let mut silent = Vec::new();
    let mut unreadable = Vec::new();
    let mut entries: Vec<std::path::PathBuf> = std::fs::read_dir(tests)
        .unwrap_or_else(|e| panic!("{} is readable: {e}", tests.display()))
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|kind| kind == "rs"))
        .collect();
    entries.sort();
    for path in entries {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();
        let source = match std::fs::read_to_string(&path) {
            Ok(source) => source,
            Err(e) => {
                unreadable.push(format!("{name}: {e}"));
                continue;
            }
        };
        match declared_in(&source) {
            Ok(Some(pair)) => {
                declared.insert(pair);
            }
            Ok(None) => {
                if name.contains("agreement") {
                    silent.push(name);
                }
            }
            Err(why) => unreadable.push(format!("{name}: {why}")),
        }
    }
    (declared, silent, unreadable)
}

#[test]
fn a_read_a_contract_only_mentions_is_not_a_read_it_judges() {
    // THE CONTROL FOR THE INSTRUMENT, and the reason it is a parser. Every
    // contract in this suite names other reads in its header — the arc's own
    // history is written there — so a search for the verb's text answers
    // "judged" off a comment. The declaration is an ITEM; a comment is not.
    let source = "//! Round 1052 declared `report-playable-world <-> \
                  report-authoring-frontier`, and this one is about something \
                  else. See `validate-continuity`.\n\
                  const DECLARES: [&str; 2] = [\"report-quest-graph\", \
                  \"report-payoff-coverage\"];\n\
                  fn main() {}\n";
    assert_eq!(
        declared_in(source).expect("the source parses"),
        Some((
            "report-payoff-coverage".to_string(),
            "report-quest-graph".to_string(),
        )),
        "the pair is the one the const states — sorted the way the backlog \
         sorts, so a contract may name its two reads whichever way round reads \
         best — and never one a comment mentions"
    );
}

#[test]
fn a_file_declaring_nothing_and_a_file_this_walk_cannot_read_are_different_answers() {
    // An empty answer and an unreadable one look identical downstream: both
    // leave the pair unmarked, and an unmarked pair sends the next round to do
    // work that is already shipped. So a declaration this walk cannot make
    // sense of is an ERROR that the surrounding checks report, not a silent
    // None.
    assert_eq!(
        declared_in("fn main() {}\n").expect("the source parses"),
        None,
        "a test file that is not a contract declares no pair, and that is not a \
         failure"
    );
    assert!(
        declared_in("fn ( this is not rust\n").is_err(),
        "a file that does not parse holds an UNKNOWN declaration; answering \
         None would report its pair as nobody's"
    );
    assert!(
        declared_in("const DECLARES: [&str; 2] = [FIRST, \"report-entity\"];\nfn main() {}\n")
            .is_err(),
        "a declaration that names something other than the verb itself is one \
         this walk cannot resolve, and resolving it half way is worse than \
         saying so"
    );
    assert!(
        declared_in("const DECLARES: [&str; 1] = [\"report-entity\"];\nfn main() {}\n").is_err(),
        "an agreement is between TWO reads"
    );
}

#[test]
fn a_contract_that_declares_nothing_is_named_and_one_that_declares_is_not() {
    // THE NET'S FINDING ARM, which the real tree cannot reach: every contract
    // in it declares, so pointed there this arm never runs and the walk would
    // ship a rule nothing has ever seen fire. The tree is built to make it
    // fire, and the control is the SAME tree with the declaration added.
    let at = tempfile::tempdir().expect("a scratch directory");
    let contract = at.path().join("newest_agreement.rs");
    let declaration = "const DECLARES: [&str; 2] = [\"report-entity\", \"report-frame-view\"];\n";
    std::fs::write(&contract, "fn main() {}\n").expect("write the contract");
    std::fs::write(at.path().join("something_else.rs"), "fn main() {}\n").expect("write a test");

    let (declared, silent, unreadable) = declared_pairs(at.path());
    assert_eq!(
        silent,
        vec!["newest_agreement.rs".to_string()],
        "a file this repository named a contract and that states no pair is a \
         contract this walk cannot see — and an unseen contract is how the \
         DECLARED marks go stale again, silently"
    );
    assert!(declared.is_empty() && unreadable.is_empty(), "{declared:?}");

    // THE CONTROL: the same file, declaring. If the net fired on the name
    // alone, this would still be listed.
    std::fs::write(&contract, format!("{declaration}fn main() {{}}\n")).expect("declare");
    let (declared, silent, unreadable) = declared_pairs(at.path());
    assert!(
        silent.is_empty() && unreadable.is_empty(),
        "the net is about the DECLARATION, not the file name: {silent:?}"
    );
    assert_eq!(
        declared,
        [("report-entity".to_string(), "report-frame-view".to_string())]
            .into_iter()
            .collect::<BTreeSet<_>>(),
        "and the pair it states is the one that comes back"
    );
}

/// The pairs of shipped reads that both ANSWER ABOUT a subject — both of their
/// records for it move under some authorable edit. Most-shared first; this is
/// the backlog, and R1037's hand-picked pair sits inside it.
///
/// `DECLARED` marks a pair some contract in this crate's test suite already
/// judges, read out of that contract's own source by [`declared_pairs`] rather
/// than kept in a list beside it. Without the mark this list said which pair to
/// compare next and could not see which were done: its top two rows had
/// contracts from Rounds 1052 and 1048, and every reader — including the memory
/// a session starts from — was told the top row was the next job for six
/// sessions after it stopped being one.
const BACKLOG: [&str; 87] = [
    "200 DECLARED report-authoring-frontier <-> report-playable-world",
    "179 DECLARED report-playable-world <-> report-playthrough-manuscript",
    "158 DECLARED report-authoring-frontier <-> report-playthrough-manuscript",
    "135 DECLARED report-frame-view <-> report-playable-world",
    "135 report-frame-view <-> report-playthrough-manuscript",
    "114 report-authoring-frontier <-> report-frame-view",
    "98 DECLARED report-authoring-frontier <-> report-payoff-coverage",
    "98 report-payoff-coverage <-> report-playable-world",
    "92 report-payoff-coverage <-> report-playthrough-manuscript",
    "83 report-frame-view <-> report-payoff-coverage",
    "81 report-playable-world <-> report-quest-graph",
    "81 report-playthrough-manuscript <-> report-quest-graph",
    "74 report-authoring-frontier <-> report-quest-graph",
    "62 report-authoring-frontier <-> validate-continuity",
    "62 report-playable-world <-> validate-continuity",
    "62 report-playthrough-manuscript <-> validate-continuity",
    "62 DECLARED report-quest-graph <-> validate-continuity",
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
    "30 DECLARED report-payoff-coverage <-> report-payoff-substantiation",
    "30 report-payoff-substantiation <-> report-playable-world",
    "26 report-coverage <-> report-frame-view",
    "26 report-frame-view <-> report-spec-map",
    "24 report-entity <-> report-payoff-coverage",
    "24 report-payoff-substantiation <-> report-playthrough-manuscript",
    "22 report-entity <-> validate-continuity",
    "20 DECLARED report-edge-candidates <-> report-payoff-substantiation",
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
    "14 DECLARED report-payoff-coverage <-> report-quest-graph",
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
    // WHICH OF THEM ARE ALREADY JUDGED, asked of the contracts rather than
    // remembered. Every row this marks is a pair some test in this crate runs
    // both halves of and asserts about; the rest are the backlog proper.
    let (declared, undeclared_contracts, unreadable) =
        declared_pairs(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests"));
    let judged = |a: &str, b: &str| declared.contains(&(a.to_string(), b.to_string()));
    let row = |a: &str, b: &str, n: usize| match judged(a, b) {
        true => format!("{n} DECLARED {a} <-> {b}"),
        false => format!("{n} {a} <-> {b}"),
    };
    println!("\nread pairs by shared subjects — the backlog, most-shared first:");
    for ((a, b), n) in &ranked {
        println!("  {n:4}  {}", row(a, b, *n));
    }
    println!(
        "\n{} of {} pairs are judged by a contract in this suite",
        declared.len(),
        ranked.len(),
    );
    // THE LINE THIS WALK EXISTS TO PRODUCE. Printed rather than left to be
    // derived by whoever reads the list next, which is what went wrong: the
    // first two rows had contracts and the reader could not tell.
    let next = ranked
        .iter()
        .find(|((a, b), _)| !judged(a, b))
        .map(|((a, b), n)| format!("{n} {a} <-> {b}"));
    println!(
        "NEXT — the highest-ranked pair no contract judges: {}",
        next.clone().unwrap_or_else(|| "none left".to_string()),
    );
    for name in &undeclared_contracts {
        println!("  UNDECLARED CONTRACT {name}");
    }
    for why in &unreadable {
        println!("  UNREADABLE {why}");
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
            .map(|((a, b), n)| row(a, b, *n))
            .collect::<Vec<_>>()
            == BACKLOG,
        "BACKLOG: every read pair that answers about a subject in common, \
         most-shared first, each marked DECLARED when a contract in this suite \
         judges it",
    );
    check(
        unreadable.is_empty(),
        "CONTRACTS READ: a test file this walk could not parse is a contract it \
         cannot see, and a contract it cannot see reads here as a pair nobody \
         judges — the next round would then do work that is already shipped",
    );
    check(
        undeclared_contracts.is_empty(),
        "CONTRACTS DECLARE: every file this repository named `*agreement*` \
         states its pair in a `const DECLARES` the test itself runs on. This is \
         the net in the direction that matters: a new contract whose pair this \
         walk cannot see is exactly how the mark above goes stale, and it goes \
         stale silently",
    );
    check(
        declared
            .iter()
            .all(|(a, b)| ranked.iter().any(|(pair, _)| *pair == (&**a, &**b))),
        "CONTRACTS ARE IN THE POPULATION: a pair some contract judges answers \
         about at least one subject in common. A declaration outside this \
         ranking is a contract over two reads that no longer share anything — \
         either the contract is stale or this walk lost the pair, and both are \
         findings",
    );
    check(
        next.as_deref() == Some("135 report-frame-view <-> report-playthrough-manuscript"),
        "NEXT: the highest-ranked pair no contract judges. It moves when a \
         round declares one, which is what makes it a pin rather than a note — \
         and until Round 1087 it was taken from a session's memory, where it \
         named the top row for six sessions after Round 1052 declared it. Round \
         1088 was the first round to READ this line off a green run rather than \
         out of a memory file, declare the pair it named, and watch the pin move \
         here on its own; Round 1138 is the second, and it moved the pin to the \
         row beside the one it declared — the same frame view against the \
         manuscript the playable world embeds, which that contract's own \
         measurements say is a different reach: the manuscript answers for the \
         18 corpora that declare no telling, and carries no seats",
    );

    assert_eq!(
        broken,
        Vec::<String>::new(),
        "the derived read-agreement population no longer holds"
    );
}
