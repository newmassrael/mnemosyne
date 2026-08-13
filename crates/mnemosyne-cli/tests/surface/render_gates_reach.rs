//! The two render-acceptance gates, asked (Round 1072 — MM2 closed).
//!
//! `validate-render-fidelity` and `validate-disclosure-leak` sat outside every
//! population this arc derives for twenty rounds, named in the census's own
//! assertion as "the reads this corpus cannot ask without inventing an
//! argument". What they need is a FILE, and no vocabulary can supply a file.
//!
//! Round 1068 designed the fixture as the authored store itself; Round 1070 RAN
//! that and refuted it — the gates are single-world by contract and the corpus
//! spans four world-lines, so the whole store draws its siblings off-path at
//! every world — then shipped `project-world`, the operation that produces what
//! they expect. What was left was the seam: the value the panel must supply is
//! not a path the corpus DECLARES but a path to an artifact the corpus must be
//! ASKED to make. `common::project_worlds` asks; `common::values_for` returns
//! the files it finds; the panel's existing machinery does the rest.
//!
//! THIS FILE IS THE PART THAT KEEPS THE CLOSURE FROM BEING EMPTY. Making an
//! unaskable read askable is only progress if the read's answer can MOVE. A
//! read wired to a fixture it cannot disagree with turns a hole the census
//! PRINTS into a green cell nobody questions — which is worse, because an
//! unasked read is visible and an inert one is not. So the two obligations
//! Round 1068 named are discharged here, against the shared sweep:
//!
//! - the fixture is not empty (the gate reports facts, not a clean nothing);
//! - some corruption in the population MOVES what the gate says.

use std::collections::BTreeSet;

use super::sweep;
use crate::common;
use sweep::Answer;

/// Every panel question put to one verb.
fn asked_of(verb: &str) -> Vec<&'static common::Read> {
    sweep::sweep()
        .panel
        .iter()
        .filter(|read| read.verb == verb)
        .collect()
}

#[test]
fn the_render_acceptance_gates_are_on_the_panel_at_all() {
    let sweep = sweep::sweep();
    let unaskable: Vec<&str> = sweep
        .unaskable
        .iter()
        .map(|(verb, _)| verb.as_str())
        .collect();
    for (verb, reason) in &sweep.unaskable {
        println!("    skip {verb} :: {reason}");
    }
    assert!(
        !unaskable.contains(&"validate-render-fidelity"),
        "the fidelity gate is still unaskable: {unaskable:?}"
    );
    assert!(
        !unaskable.contains(&"validate-disclosure-leak"),
        "the leak gate is still unaskable: {unaskable:?}"
    );

    // WHAT EACH WAS ASKED, printed: the leak gate answers at ONE (world,
    // truth-frame) pair of the many its vocabulary offers, and that is a fact
    // about this corpus rather than a limit of the panel — the rest refuse as
    // VACUOUS (no shared vocabulary in that frame) or as leaks.
    for verb in ["validate-render-fidelity", "validate-disclosure-leak"] {
        let questions = asked_of(verb);
        println!("  {verb}: {} question(s) answered", questions.len());
        for read in &questions {
            println!("      {}", read.argv().join(" "));
        }
        assert!(
            !questions.is_empty(),
            "`{verb}` is not in the unaskable list and answered no question \
             either, which is neither state the panel has"
        );
    }
}

#[test]
fn the_fixture_the_gates_were_handed_is_not_empty() {
    let sweep = sweep::sweep();
    let mut counts = Vec::new();
    for read in asked_of("validate-render-fidelity") {
        let Answer::Json(answer) = &sweep.baseline.answers[&read.label()] else {
            panic!("the fidelity gate answers `--json` in JSON");
        };
        let facts = answer["reextracted_facts"]
            .as_u64()
            .expect("the gate reports how many facts it read");
        println!(
            "  {} -> {facts} re-extracted fact(s), off_path {}, unplaced {}",
            read.argv().join(" "),
            answer["off_path"].as_array().expect("a list").len(),
            answer["unplaced"].as_array().expect("a list").len()
        );
        counts.push(facts);
    }
    assert!(!counts.is_empty(), "no fidelity question was answered");
    // THE TRAP ROUND 1068 NAMED, asserted. A store with no narrative facts is
    // a CLEAN fidelity report: no off-path, no unplaced, exit 0. The read
    // would join the panel and never disagree with anything again.
    assert!(
        counts.iter().all(|n| *n > 0),
        "a fidelity question was answered about ZERO facts, which is a clean \
         report about nothing — askable and inert, the one outcome worse than \
         unaskable: {counts:?}"
    );
}

#[test]
fn some_authorable_edit_moves_what_the_gates_say() {
    let sweep = sweep::sweep();
    let gates: Vec<&common::Read> = asked_of("validate-render-fidelity")
        .into_iter()
        .chain(asked_of("validate-disclosure-leak"))
        .collect();
    // Which edits moved each gate — named, not counted, so a gate that moves
    // for one leg and nothing else says which leg.
    let mut moved: BTreeSet<String> = BTreeSet::new();
    let mut rejected: BTreeSet<String> = BTreeSet::new();
    for trial in &sweep.trials {
        let Ok(raw) = &trial.seen else { continue };
        let seen = raw.parsed();
        for read in &gates {
            let label = read.label();
            if seen.failed.contains(&label) {
                // Answered at baseline, refuses here: the loudest move a gate
                // has, and the one that makes it a gate rather than a report.
                rejected.insert(format!("{} <- {}", read.verb, trial.corruption.label()));
                continue;
            }
            let (Some(before), Some(after)) =
                (sweep.baseline.answers.get(&label), seen.answers.get(&label))
            else {
                continue;
            };
            let differs = match (before, after) {
                (Answer::Json(before), Answer::Json(after)) => before != after,
                (Answer::Prose(before), Answer::Prose(after)) => before != after,
                _ => true,
            };
            if differs {
                moved.insert(format!("{} <- {}", read.verb, trial.corruption.label()));
            }
        }
    }
    println!(
        "  {} edit(s) move a gate's answer, {} make one REJECT",
        moved.len(),
        rejected.len()
    );
    for row in moved.iter().chain(&rejected) {
        println!("      {row}");
    }
    // THE SECOND OBLIGATION. An askable read whose answer cannot move is a
    // green cell where a printed hole used to be.
    assert!(
        !moved.is_empty() || !rejected.is_empty(),
        "no edit in the population moves either gate — the fixture cannot \
         disagree with the store it is judged against, so making these reads \
         askable measured nothing"
    );
}
