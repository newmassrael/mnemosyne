//! The census put to a ledger that carries every way it has been wrong.
//!
//! EACH CASE IS A DEFECT THAT ACTUALLY SHIPPED, not a shape somebody imagined.
//! The one-liner this crate replaces counted its own documentation, counted a
//! mention as a row, and missed the notation the ledger's own tables use to
//! retire one — and the third made the number wrong, which is the number the
//! whole debt arc terminates on.

use open_debts::{open_autonomous, registrations, retired, Shape};

/// A bullet registration, the shape most rows use.
const BULLET: &str = "\
- **N100**(①) — a thing that can be done here.
 A second line of the same row.
- **N101**(②) — a limit, recorded rather than worked.
";

#[test]
fn a_bullet_row_is_registered_and_its_branch_is_its_parenthetical() {
    let rows = registrations(BULLET);
    assert_eq!(rows.len(), 2, "both bullets register: {rows:?}");
    assert_eq!(rows[0].id, "N100");
    assert_eq!(rows[0].shape, Shape::Bullet);
    assert!(
        rows[0].body.contains("A second line"),
        "the row is its body"
    );
    let open = open_autonomous(BULLET);
    assert_eq!(open.len(), 1, "only the ① one is this branch's: {open:?}");
    assert_eq!(open[0].id, "N100");
}

/// AND A ROW'S BODY ENDS AT THE NEXT BULLET. Read to the next registration
/// anywhere and the `CLOSED` belonging to the row BENEATH is swallowed, which
/// retired three live rows the first time this was written.
#[test]
fn a_closed_row_does_not_retire_the_row_above_it() {
    let ledger = "\
- **N200**(①) — still open, and the next line is somebody else's retirement.
- **N201**(①) — done. CLOSED (R1)
";
    let open = open_autonomous(ledger);
    assert_eq!(
        open.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
        vec!["N200"],
        "N200 is open and N201 is not: {open:?}"
    );
}

/// A row named in a round summary is not a row.
///
/// `신규 = **N123**(①자율)` is somebody saying a debt exists. Counting it opens
/// something with nothing in it to do — a row that can never be closed, which is
/// how a termination condition becomes unreachable.
#[test]
fn a_mention_inside_a_summary_is_not_a_registration() {
    let ledger = "\
**R1231 did a thing. New = **N300**(①자율).**
- **N301**(①) — and this one has a body.
";
    let rows = registrations(ledger);
    assert_eq!(rows.len(), 2, "both are SEEN: {rows:?}");
    assert!(rows[0].is_a_mention(), "the summary one is a mention");
    assert!(!rows[1].is_a_mention(), "the bullet one is not");
    let open = open_autonomous(ledger);
    assert_eq!(
        open.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
        vec!["N301"]
    );
}

/// An inline registration that DOES carry a body is a row.
///
/// The other half of the rule above, and the half that keeps it from being a
/// filter on inline registrations as such: the ledger registers real debts
/// inline, inside `신규 =` lists, and those have their reason in the parenthetical.
#[test]
fn an_inline_registration_with_a_body_is_a_row() {
    let ledger =
        "New = **N400**(the guard lives on the caller rather than the chain, ①) · **N401**(②).\n";
    let open = open_autonomous(ledger);
    assert_eq!(
        open.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
        vec!["N400"],
        "the one with a reason is a row; the bare ② is neither ① nor a row: {open:?}"
    );
    assert_eq!(open[0].shape, Shape::Inline);
}

/// THE ONE THAT MADE THE NUMBER WRONG. The ledger's tables retire a row by
/// striking the id through, and the prose reader looked for `CLOSED` within a
/// few characters of the id — which that table row puts a round number and two
/// emoji away. Thirty ids use this notation; one of them was counted open.
#[test]
fn a_struck_through_id_is_retired() {
    let ledger = "\
- **N500**(①) — registered here, and retired in the table below.
| ~~N500~~ | a round closed it, and the word is far from the id |
- **N501**(①) — not struck through anywhere.
";
    assert!(retired(ledger).contains("N500"), "{:?}", retired(ledger));
    let open = open_autonomous(ledger);
    assert_eq!(
        open.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
        vec!["N501"]
    );
}

/// A run of ids retired by one round is retired WHOLE.
///
/// `N220·N221·N222 CLOSED` — a reader that takes the nearest id retires the last
/// of three and leaves two rows open that a round already paid for.
#[test]
fn a_run_of_ids_before_one_closed_is_retired_whole() {
    let ledger = "\
- **N600**(①) — one.
- **N601**(①) — two.
- **N602**(①) — three.
**R1283 (N600·N601·N602 CLOSED).**
";
    let closed = retired(ledger);
    for id in ["N600", "N601", "N602"] {
        assert!(closed.contains(id), "{id} is in the run: {closed:?}");
    }
    assert!(open_autonomous(ledger).is_empty());
}

/// AND THE RUN STOPS WHERE THE SENTENCE DOES. A line that names debts and then
/// closes something else must not retire the ones it named.
#[test]
fn ids_a_sentence_merely_names_are_not_retired_by_it() {
    let ledger = "\
- **N700**(①) — open.
The next round is N700 or N701, and separately R9 CLOSED the arc.
- **N701**(①) — open.
";
    let closed = retired(ledger);
    assert!(
        !closed.contains("N700") && !closed.contains("N701"),
        "a sentence naming them is not a retirement: {closed:?}"
    );
    assert_eq!(open_autonomous(ledger).len(), 2);
}

/// AN INLINE PARENTHETICAL WRAPS, and the marker is often on the second line.
///
/// FOUND BY DISAGREEING WITH THE THING IT REPLACES. This crate's first census of
/// the real ledger came back FIVE lower than the pasted one-liner's; one of the
/// five was a row the one-liner had wrongly counted, and the other four were
/// wrapped registrations this walk could not see because it read a line at a
/// time. A smaller number is not a better one, and the difference had to be
/// explained before either could be believed.
#[test]
fn an_inline_registration_whose_parenthetical_wraps_is_still_a_row() {
    let ledger = "\
New = **N900**(the guard lives on the caller rather than the chain — and the
marker sits on the NEXT line, ①) · **N901**(②).
";
    let open = open_autonomous(ledger);
    assert_eq!(
        open.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
        vec!["N900"],
        "a wrapped parenthetical is one parenthetical: {open:?}"
    );
    assert!(
        open[0].body.contains("NEXT line"),
        "and its body is the whole of it, both lines: {:?}",
        open[0].body
    );
}

/// A row that carries two branches is counted once, and its ambiguity survives.
#[test]
fn a_row_marked_for_two_branches_is_counted_once() {
    let ledger = "- **N800**(①/③) — measurable here, and possibly a cost call.\n";
    let open = open_autonomous(ledger);
    assert_eq!(open.len(), 1);
    assert!(
        open[0].body.contains("①/③"),
        "the reader keeps what the ledger wrote rather than resolving it"
    );
}

/// THE POPULATION IS NOT EMPTY BY ACCIDENT. A ledger this cannot parse yields no
/// rows, and no rows is what a finished arc looks like — so the binary refuses
/// rather than reporting success, and this is the library half of that.
#[test]
fn a_ledger_with_no_registrations_yields_nothing_to_mistake_for_finished() {
    let ledger = "A file with prose about debts and no registration notation at all.\n";
    assert!(registrations(ledger).is_empty());
    assert!(open_autonomous(ledger).is_empty());
}
