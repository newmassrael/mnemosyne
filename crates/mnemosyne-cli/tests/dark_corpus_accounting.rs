//! A corpus goes dark because its AUTHOR's submission was refused, never
//! because the schema moved past it (Round 1174).
//!
//! Sixteen of the forty-four tracked corpora did not load, and every walk over
//! the population printed that count and pinned it. The size was never silent.
//! What was silent is the only thing that decides whether the number matters:
//! WHY. The recipe held each refusal message at the moment it judged and kept
//! the name alone, so a corpus dark because its author shipped something the
//! write path rejects read exactly like a corpus dark because a vocabulary it
//! predates became a closed set. Thirteen were the second kind:
//!
//!   - twelve to R708, which removed the free-text `value` / `scalar` object
//!     shape;
//!   - all thirteen to R732's entity-kind registry, which arrived under them;
//!   - one to R752, which turned a disclosure `first_at` PAIR list into
//!     per-branch trigger sets.
//!
//! `common::upgrade_corpus_manifest` carries a manifest across all three by
//! closing over the vocabulary the corpus itself used, and the three that
//! remain dark are first submissions their own experiment recorded as
//! rejected. THAT is the state this file pins, in the two halves that make it
//! a claim rather than a count:
//!
//!   1. every dark corpus is a `-first-submission` record — the experiment's
//!      own name for "the author's rejected attempt";
//!   2. the ACCEPTED sibling of each one loads. Without this half the first is
//!      a claim about a directory name; with it, the refusal is demonstrably
//!      about the submission and not about the schema, because the same
//!      author's next submission of the same corpus goes through.
//!
//! And the upgrade's own ledger is pinned per corpus, so it cannot quietly
//! start changing something else. An upgrade nobody can read is a second
//! author writing into the evidence.

use std::collections::BTreeMap;

use crate::common;
use common::{authored_corpora, authored_stores, read_json, repo_root, upgrade_corpus_manifest};

/// The suffix an experiment gives the submission its own gate turned back.
const FIRST_SUBMISSION: &str = "-first-submission";

#[test]
fn a_corpus_is_dark_only_because_its_author_s_submission_was_refused() {
    let (stores, unloadable) = authored_stores();

    println!(
        "{} corpora load, {} do not:",
        stores.len(),
        unloadable.len()
    );
    for corpus in &unloadable {
        println!("    DARK {corpus}");
    }

    let mut broken: Vec<String> = Vec::new();
    for corpus in &unloadable {
        let Some(accepted) = corpus.name.strip_suffix(FIRST_SUBMISSION) else {
            broken.push(format!(
                "{} is dark and is not a first submission, so nothing says its \
                 author was refused rather than the schema moving under it",
                corpus.name
            ));
            continue;
        };
        // The other half: the same corpus, resubmitted and accepted. A dark
        // record whose accepted sibling ALSO fails to load is a corpus this
        // tree lost, not a rejection it recorded. The sibling's name may carry
        // the recipe's upgrade marker, which is about HOW it was built and not
        // about which corpus it is.
        if !stores
            .iter()
            .any(|store| store.name.split(" (").next() == Some(accepted))
        {
            broken.push(format!(
                "{} is dark and its accepted sibling `{accepted}` does not load \
                 either, so the refusal is not demonstrably about the submission",
                corpus.name
            ));
        }
    }

    assert_eq!(
        (stores.len(), unloadable.len()),
        (41, 3),
        "the corpora this tree can ask and the ones it cannot — 40 tracked \
         plus the migrated dnd-quest record against the 3 first submissions"
    );
    assert_eq!(
        broken,
        Vec::<String>::new(),
        "a corpus is dark for a reason other than its author being refused"
    );
}

#[test]
fn the_upgrade_says_what_it_changed_in_every_corpus_it_touched() {
    let mut ledger: BTreeMap<String, String> = BTreeMap::new();
    for dir in authored_corpora() {
        let name = dir
            .strip_prefix(repo_root())
            .unwrap_or(&dir)
            .display()
            .to_string();
        let mut facts = read_json(&dir.join("facts.json"));
        if let Some(upgrade) = upgrade_corpus_manifest(&mut facts) {
            ledger.insert(name, upgrade.to_string());
        }
    }

    for (name, what) in &ledger {
        println!("  UPGRADED {name}: {what}");
    }

    // The ledger ITSELF, pinned. A corpus that stops needing the upgrade, or
    // one that starts needing more of it, is a change to the evidence every
    // other walk reads — and the whole reason the upgrade lives in the recipe
    // rather than in thirteen rewritten manifests is that the record on disk
    // does not move. Something has to hold it to that.
    let rows: Vec<String> = ledger
        .iter()
        .map(|(name, what)| format!("{name} :: {what}"))
        .collect();
    assert_eq!(
        rows,
        [
            "claudedocs/phase1-2d-projection-experiment/v1/run/author :: 0 value object(s) -> \
             token, re-declared [], 3 first_at pair list(s) -> trigger sets",
            "claudedocs/phase1-ai-authoring-experiment/v1/run/author :: 10 value object(s) -> \
             token, re-declared [\"early_bell_meaning\", \"kittiwake_cause\", \"senses\"], \
             declared kinds [\"object\", \"person\", \"place\"]",
            "claudedocs/phase1-ai-authoring-experiment/v2/run/author :: 44 value object(s) -> \
             token, re-declared [\"belief\", \"disposition\", \"fate\", \"location\", \
             \"record-standing\"], declared kinds [\"object\", \"person\"]",
            "claudedocs/phase1-ai-breadth-depth-experiment/v1/run/author :: 18 value object(s) \
             -> token, re-declared [\"cellar-state\", \"culpability\", \"final-disposition\", \
             \"illness\", \"ledger-state\", \"manner-of-death\"], declared kinds [\"event\", \
             \"item\", \"person\", \"place\"]",
            "claudedocs/phase1-ai-npc-breadth-experiment/v1/run/author :: 3 value object(s) -> \
             token, re-declared [\"fate\"], DROPPED unused [\"bond_state\", \"box_state\"], \
             declared kinds [\"item\", \"person\"]",
            "claudedocs/phase1-concurrency-probe/v1/run/author :: 4 value object(s) -> token, \
             re-declared [\"line_state\"], declared kinds [\"group\", \"object\", \"person\", \
             \"place\"]",
            "claudedocs/phase1-convergence-probe/v1/run/author :: 15 value object(s) -> token, \
             re-declared [\"dam_state\", \"gate_state\", \"keeper_recognition\", \"mill_state\", \
             \"standing\"], declared kinds [\"group\", \"object\", \"person\"]",
            "claudedocs/phase1-convergence-probe/v2/run/author :: 14 value object(s) -> token, \
             re-declared [\"fate\", \"gate-state\", \"sluice-state\", \"verdict\"], declared \
             kinds [\"object\", \"person\", \"place\"]",
            "claudedocs/phase1-dnd-quest-experiment/v1/run/author :: 18 value object(s) -> token, \
             re-declared [\"betrayal\", \"cause_of_rising\", \"reach_rule\", \"rising_state\", \
             \"warden_disposition\"], declared kinds [\"item\", \"person\", \"place\", \"quest\"]",
            "claudedocs/phase1-dnd-quest-experiment/v2/run/author :: 18 value object(s) -> token, \
             re-declared [\"betrayal\", \"cause_of_rising\", \"reach_rule\", \"rising_state\", \
             \"warden_disposition\"], declared kinds [\"item\", \"person\", \"place\", \"quest\"]",
            "claudedocs/phase1-dnd-quest-experiment/v3/run/author :: 18 value object(s) -> token, \
             re-declared [\"betrayal\", \"cause_of_rising\", \"reach_rule\", \"rising_state\", \
             \"warden_disposition\"], declared kinds [\"item\", \"person\", \"place\", \"quest\"]",
            "claudedocs/phase1-npc-dialogue-experiment/v1/run/author :: 6 value object(s) -> \
             token, re-declared [\"cause\", \"lit\", \"whereabouts\"], declared kinds [\"item\", \
             \"person\", \"place\"]",
            "claudedocs/phase1-time-travel-experiment/v1/run/author :: 10 value object(s) -> \
             token, re-declared [\"condition\"], declared kinds [\"person\", \"place\", \"prop\", \
             \"state-object\"]",
        ],
        "the upgrade changed something other than the three vocabulary breaks \
         it is declared to carry"
    );
}
