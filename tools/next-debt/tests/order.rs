//! The steering rules put to a ledger written the way this ledger is written.
//!
//! EVERY CASE IS A SHAPE THE REAL LEDGER HOLDS OR A WAY IT COULD GO WRONG, and
//! the fixtures copy its notation rather than a tidier one: the rank is written
//! bold, the provenance mark sits inside a code span, and the parenthetical
//! carries prose after both. A reader tested against a cleaner ledger than the
//! one it runs on is a reader that passes and then answers about nothing.

use next_debt::{
    declared, steering, DeclarationFault, Fault, Origin, Rank, REAIMING_CAP, UNRANKED_RATCHET,
};

/// The shape the ledger actually writes a ranked, rooted row in.
const AS_WRITTEN: &str = "\
- **N100**(\u{2460}, **critical**, `@from: none` \u{2014} met while repaying, not made) \u{2014}
 the row a session should take first.
- **N101**(\u{2460}, ordinary, `@from: none`) \u{2014} ranked, and ranked below it.
- **N102**(\u{2460}) \u{2014} nobody has ranked this one, which is not the same as ordinary.
";

#[test]
fn a_rank_is_read_from_the_parenthetical_bold_or_bare() {
    let order = steering(AS_WRITTEN).expect("this ledger can be steered");
    let ranks: Vec<(&str, Rank)> = order
        .open
        .iter()
        .map(|row| (row.id.as_str(), row.rank))
        .collect();
    assert_eq!(
        ranks,
        vec![
            ("N100", Rank::Critical),
            ("N101", Rank::Ordinary),
            ("N102", Rank::Unranked),
        ],
        "the emphasis around a rank is punctuation, not part of the word"
    );
    assert_eq!(
        order
            .critical()
            .iter()
            .map(|row| row.id.as_str())
            .collect::<Vec<_>>(),
        vec!["N100"],
        "the critical line is the one a round takes from"
    );
    assert_eq!(
        order.unranked(),
        1,
        "and the undecided one is counted apart"
    );
}

/// A RANK SPELLED INSIDE A LONGER WORD IS NOT A RANK. The parenthetical carries
/// prose, and prose contains words; a reader matching on a substring would rank
/// a row by a sentence about it.
#[test]
fn a_rank_inside_a_longer_word_is_prose() {
    let ledger = "- **N110**(\u{2460}, criticality was never the question) \u{2014} a row.\n";
    let order = steering(ledger).expect("this ledger can be steered");
    assert_eq!(order.open[0].rank, Rank::Unranked, "{:?}", order.open);
}

/// AND A ROW THAT SAYS BOTH IS A REFUSAL. Choosing one of them would be this
/// reader deciding what the ledger left ambiguous, quietly.
#[test]
fn a_row_written_both_ranks_is_not_judged() {
    let ledger = "- **N120**(\u{2460}, critical, and also ordinary) \u{2014} which is it.\n";
    let faults = steering(ledger).expect_err("a row written both ways is refused");
    assert_eq!(
        faults,
        vec![Fault::RankIsBoth {
            id: "N120".to_string(),
            line: 1
        }]
    );
}

/// The three roots and the one hop, which is the whole of the depth rule.
#[test]
fn provenance_depth_counts_hops_to_the_work_that_was_being_done() {
    let ledger = "\
- **N200**(\u{2460}) \u{2014} registered before the rule existed, so it records nothing.
- **N201**(\u{2460}, ordinary, `@from: none`) \u{2014} met rather than made.
- **N202**(\u{2460}, ordinary, `@from: R1312`) \u{2014} a round made it, and a round is not a row.
- **N203**(\u{2460}, ordinary, `@from: N201`) \u{2014} the row being repaid made it.
";
    let order = steering(ledger).expect("this ledger can be steered");
    let depth = |id: &str| *order.depths.get(id).expect("every open row has a depth");
    assert_eq!(depth("N200"), 0, "no mark at all is a root");
    assert_eq!(depth("N201"), 0, "and so is `none`");
    assert_eq!(depth("N202"), 1, "a round is one hop from the work");
    assert_eq!(depth("N203"), 1, "and so is the row being repaid");
    assert_eq!(
        order.unrooted(),
        1,
        "only the row that records nothing is unrooted: {:?}",
        order.open
    );
    assert_eq!(order.open[2].origin, Origin::Round("R1312".to_string()));
    assert_eq!(order.open[3].origin, Origin::Row("N201".to_string()));
}

/// A PARENT MAY BE RETIRED AND THE CHAIN STILL HAS A DEPTH. This is the shape
/// the real ledger's first chain has: the row a repayment created outlives the
/// row that was being repaid, and a walk over open rows only would call the
/// parent missing and refuse a ledger that is correct.
#[test]
fn a_chain_walks_through_a_row_that_has_since_been_retired() {
    let ledger = "\
- **N210**(\u{2460}) \u{2014} the row that was being repaid. CLOSED (R1313)
- **N211**(\u{2460}, ordinary, `@from: N210`) \u{2014} what repaying it made.
";
    let order = steering(ledger).expect("a retired parent is still a parent");
    assert_eq!(
        order
            .open
            .iter()
            .map(|row| row.id.as_str())
            .collect::<Vec<_>>(),
        vec!["N211"],
        "the retired row is not open: {:?}",
        order.open
    );
    assert_eq!(order.depths.get("N211"), Some(&1));
}

/// The cap is what turns a depth into an instruction.
#[test]
fn the_cap_defers_what_is_deeper_than_it_and_nothing_else() {
    let ledger = "\
- **N220**(\u{2460}, ordinary, `@from: none`) \u{2014} a root.
- **N221**(\u{2460}, ordinary, `@from: N220`) \u{2014} one hop.
- **N222**(\u{2460}, ordinary, `@from: N221`) \u{2014} two hops.
";
    let order = steering(ledger).expect("this ledger can be steered");
    assert_eq!(order.by_depth().get(&2), Some(&1), "{:?}", order.by_depth());
    assert_eq!(
        order
            .deferred(1)
            .iter()
            .map(|row| row.id.as_str())
            .collect::<Vec<_>>(),
        vec!["N222"],
        "at a cap of one, the two-hop row is registered and not repaid"
    );
    assert!(
        order.deferred(2).is_empty(),
        "and at a cap of two it is this round's work"
    );
}

/// A PROVENANCE MARK NAMING NOTHING REGISTERED IS A REFUSAL, for the reason the
/// census refuses a retirement naming a commit that is not there: being unable
/// to measure is not a measurement of zero.
#[test]
fn a_mark_naming_an_unregistered_row_is_not_judged() {
    let ledger = "- **N230**(\u{2460}, ordinary, `@from: N999`) \u{2014} from what.\n";
    let faults = steering(ledger).expect_err("a dangling mark is refused");
    assert_eq!(
        faults,
        vec![Fault::OriginNamesNothingRegistered {
            id: "N230".to_string(),
            line: 1,
            named: "N999".to_string()
        }]
    );
}

/// AND A MARK NAMING NOTHING AT ALL IS ITS OWN REFUSAL, kept apart from "this
/// row predates the rule": one author tried to follow the rule and mistyped, and
/// folding that into the unrooted count would hide it inside a number that is
/// supposed to be shrinking.
#[test]
fn a_mark_with_no_name_after_it_is_not_unrooted() {
    let ledger = "- **N240**(\u{2460}, ordinary, `@from:`) \u{2014} the mark and nothing else.\n";
    let faults = steering(ledger).expect_err("an empty mark is refused");
    assert_eq!(
        faults,
        vec![Fault::OriginUnreadable {
            id: "N240".to_string(),
            line: 1
        }]
    );
}

/// A RING IS ONE FINDING, however many rows stand on it.
#[test]
fn a_chain_that_returns_to_itself_is_reported_once() {
    let ledger = "\
- **N250**(\u{2460}, ordinary, `@from: N251`) \u{2014} a.
- **N251**(\u{2460}, ordinary, `@from: N250`) \u{2014} b.
";
    let faults = steering(ledger).expect_err("a cycle has no depth");
    assert_eq!(faults.len(), 1, "one ring, one finding: {faults:?}");
    assert!(
        matches!(&faults[0], Fault::OriginCycles { ids } if ids.len() == 3),
        "{faults:?}"
    );
}

/// The pinned numbers, read off a line of the document's own.
#[test]
fn a_declared_number_is_read_through_the_decoration_of_its_line() {
    let ledger = format!("- **{UNRANKED_RATCHET} = 23** \u{2014} measured, and only ever falls.\n");
    assert_eq!(declared(&ledger, UNRANKED_RATCHET), Ok(23));
    let rulebook = format!("- **{REAIMING_CAP} = 1** \u{2014} deeper than this is registered.\n");
    assert_eq!(declared(&rulebook, REAIMING_CAP), Ok(1));
}

/// PROSE ABOUT THE MARKER IS NOT THE MARKER. The section pinning this notation
/// has to spell it, and a reader taking the first match anywhere would take the
/// sentence explaining the rule instead of the rule.
#[test]
fn the_marker_is_only_a_declaration_at_the_head_of_a_line() {
    let ledger = format!("the line reading {UNRANKED_RATCHET} = 23 is what pins it.\n");
    assert_eq!(
        declared(&ledger, UNRANKED_RATCHET),
        Err(DeclarationFault::Absent)
    );
}

/// TWO DECLARATIONS ARE NO DECLARATION, whether or not they agree: a datum with
/// two homes is the shape this repository forbids by name.
#[test]
fn two_lines_declaring_it_leave_no_single_value() {
    let ledger = format!("- **{UNRANKED_RATCHET} = 23**\n\nand later\n\n{UNRANKED_RATCHET} = 23\n");
    assert_eq!(
        declared(&ledger, UNRANKED_RATCHET),
        Err(DeclarationFault::Repeated { lines: vec![1, 5] })
    );
}

/// A DECLARATION THAT IS NOT A NUMBER IS A REFUSAL, not a zero.
#[test]
fn a_declaration_that_is_not_a_number_is_refused() {
    let ledger = format!("{UNRANKED_RATCHET} = several\n");
    assert!(matches!(
        declared(&ledger, UNRANKED_RATCHET),
        Err(DeclarationFault::Unreadable { line: 1, .. })
    ));
    let missing_equals = format!("{UNRANKED_RATCHET} is 3\n");
    assert!(matches!(
        declared(&missing_equals, UNRANKED_RATCHET),
        Err(DeclarationFault::Unreadable { line: 1, .. })
    ));
}
