//! The census put to a ledger that carries every way it has been wrong.
//!
//! EACH CASE IS A DEFECT THAT ACTUALLY SHIPPED, not a shape somebody imagined.
//! The one-liner this crate replaces counted its own documentation, counted a
//! mention as a row, and missed the notation the ledger's own tables use to
//! retire one — and the third made the number wrong, which is the number the
//! whole debt arc terminates on.

use std::collections::BTreeSet;

use open_debts::{registrations, Registration, Shape};

/// The census over a ledger whose retirements name no commit.
///
/// EVERY FIXTURE THAT PREDATES THE COMMIT CHECK (R1298) reaches the library
/// through here, and the argument for it is a property of those fixtures rather
/// than a convenience: none of them names a commit, so there is nothing that
/// could have failed to resolve. The cases that DO name one build their own set
/// and call the library directly, which is what keeps this helper from becoming
/// a way of never exercising the parameter.
fn open_autonomous(ledger: &str) -> Vec<Registration> {
    open_debts::open_autonomous(ledger, &BTreeSet::new())
}

/// The retirement set over the same kind of ledger, for the same reason.
fn retired(ledger: &str) -> BTreeSet<String> {
    open_debts::retired(ledger, &BTreeSet::new())
}

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

// ── A RETIREMENT IS A CLAIM, AND A CLAIM CAN BE FALSE (R1298) ────────────────

/// A row ABOUT retirement is not a retired row.
///
/// THIS IS THE DEFECT AS IT HAPPENED, not a shape imagined for a test. N270 was
/// registered to say that this ledger's retirements cite commits nobody checks;
/// its headline named the word; the census retired it and reported one fewer
/// open row with nothing at all to show that it had.
#[test]
fn a_row_that_names_the_retirement_word_is_not_retired_by_saying_it() {
    let ledger = "\
- **N900**(①) — the ledger's `CLOSED` marker cites a commit and nobody checks it.
 The rule is `row.body.contains(\"CLOSED\")`, which retires a row for saying it.
";
    let open = open_autonomous(ledger);
    assert_eq!(
        open.len(),
        1,
        "a row whose SUBJECT is the word stays open: {open:?}"
    );
    assert_eq!(open[0].id, "N900");
}

/// A retirement QUOTED in corner brackets is a quotation, not a retirement.
///
/// THE ROW RETIRED ITSELF TWICE. The code-span rule was written first and was
/// not enough: N270's body quoted a whole live retirement — the `Z19` example
/// that widened the attribution rule — in the corner brackets this ledger uses
/// for quoting prose, and the census dropped the row again. A reader that
/// honours only one of two quotation notations makes safety depend on which
/// mark the author reached for.
#[test]
fn a_retirement_quoted_in_corner_brackets_retires_nothing() {
    let ledger = "\
- **N909**(①) — the reader missed this shape.
 그중 `Z19` 는 「(CLOSED 2026-08-14)」로 날짜가 귀속이었다.
";
    let open = open_autonomous(ledger);
    assert_eq!(
        open.len(),
        1,
        "quoting a retirement is not performing one: {open:?}"
    );
    assert_eq!(open[0].id, "N909");
}

/// And prose that uses the word while naming nobody retires nothing either.
///
/// THE TWO REFUSALS ARE DIFFERENT and both were needed: the case above hides
/// the word in a code span, this one writes it plainly with nothing attributing
/// it. A ledger where either retires a row is a ledger where a count can drop
/// because somebody wrote a sentence.
#[test]
fn a_retirement_that_names_nothing_retires_nothing() {
    let ledger = "- **N901**(①) — this row is not CLOSED and nothing here says who closed it.\n";
    let open = open_autonomous(ledger);
    assert_eq!(open.len(), 1, "unattributed prose is prose: {open:?}");
}

/// Named by a round, it is a retirement — the control for the two above.
#[test]
fn a_retirement_that_names_a_round_still_retires() {
    let ledger = "- **N902**(①) — done.\n 🟢CLOSED (R1244).\n";
    assert!(
        open_autonomous(ledger).is_empty(),
        "naming the round that did it is what a retirement has always looked like"
    );
}

/// A day is a name too, and this rule learned that from the ledger.
///
/// THE FIRST FORM OF THE ATTRIBUTION RULE KNEW THREE NAMES and refused two live
/// retirements on the real file. One of them — `Z19 상세 (CLOSED 2026-08-14)` —
/// is attributed by a date and nothing else, so the notation was evidence and
/// the reader was what was wrong. The other names nothing at all and is still
/// refused, which is how the two are told apart.
#[test]
fn a_retirement_attributed_by_a_day_is_a_retirement() {
    let dated = "- **N907**(①) — done.\n **Z19 상세 (CLOSED 2026-08-14) — the detail**\n";
    assert!(
        retired(dated).contains("Z19"),
        "the ledger writes this shape and this reader must not refuse it"
    );
    let nameless = "- **N908**(①) — done.\n 🟢**N908 CLOSED — 단 다른 답으로.**\n";
    assert!(
        !retired(nameless).contains("N908"),
        "and a retirement that names nothing is still refused"
    );
}

/// A retirement naming a commit this repository has retires its row.
#[test]
fn a_retirement_naming_a_commit_that_exists_retires_its_row() {
    let ledger = "- **N903**(①) — done. CLOSED (R1297, 커밋 `adacf08`)\n";
    let named = open_debts::commits_named_by_retirements(ledger);
    assert_eq!(
        named.keys().collect::<Vec<_>>(),
        vec!["adacf08"],
        "the sha is read off the word that introduces it: {named:?}"
    );
    assert!(
        open_debts::open_autonomous(ledger, &BTreeSet::new()).is_empty(),
        "with nothing unresolved, this is an ordinary retirement"
    );
}

/// AND NAMING A COMMIT THAT IS NOT THERE RETIRES NOTHING — the mutation that
/// says this gate is not vacuous.
///
/// Every commit the real ledger cites resolves today, so the only way to see
/// this hold is to break it on purpose. It is the exact shape that shipped:
/// R1297's row claimed `커밋 4a4d0e0` before that commit was made.
#[test]
fn a_retirement_naming_a_commit_that_is_not_there_retires_nothing() {
    let ledger = "- **N904**(①) — done. CLOSED (R1297, 커밋 `4a4d0e0`)\n";
    let unresolved: BTreeSet<String> = ["4a4d0e0".to_string()].into_iter().collect();
    assert!(
        open_debts::open_autonomous(ledger, &BTreeSet::new()).is_empty(),
        "the control: with the commit present the row is retired"
    );
    let open = open_debts::open_autonomous(ledger, &unresolved);
    assert_eq!(
        open.len(),
        1,
        "a closure against a commit that does not exist is a false claim, \
         and a false name is worse than no name: {open:?}"
    );
    assert!(
        open_debts::retired(ledger, &unresolved).is_empty(),
        "and the line reader must refuse it too — one resolver, or the pair drifts"
    );
}

/// A backticked hexadecimal that no word introduces is not a commit.
///
/// `31387185994` IS A RUN ID and it reads as hexadecimal. A rule that took any
/// backticked hex would report GitHub run ids as dangling commits — a gate that
/// reddens on things that are not its subject is one people turn off.
#[test]
fn a_run_id_beside_a_retirement_is_not_read_as_a_commit() {
    let ledger = "- **N905**(①) — done. CLOSED (R1134, run `31387185994` was the red)\n";
    assert!(
        open_debts::commits_named_by_retirements(ledger).is_empty(),
        "only `커밋`/`commit` introduces a sha"
    );
}

/// An empty branch is not a finished arc while a closure cannot be supported.
///
/// THE PREDICATE THE WHOLE ARC ENDS ON, asked of the library so a law can ask it
/// too. Three shapes, and each was a way to reach `0` without the work being
/// done: a row still open, a closure against a commit that is not there, and a
/// closure naming nobody at all. The last one is the reversal R1299 made — it
/// printed and passed for one round, on an argument written before anyone had
/// counted how many rows it excused.
#[test]
fn the_arc_is_not_finished_while_a_closure_cannot_be_supported() {
    let done = "- **N910**(②) — a limit, recorded rather than worked.\n";
    assert!(
        open_debts::finished(done, &BTreeSet::new()),
        "nothing open and nothing claimed falsely IS the terminating shape"
    );

    let still_open = "- **N911**(①) — there is work here.\n";
    assert!(
        !open_debts::finished(still_open, &BTreeSet::new()),
        "an open row keeps the arc going, which is the condition's whole point"
    );

    let nameless = "- **N912**(②) — done.\n 🟢**N912 CLOSED — 단 다른 답으로.**\n";
    assert!(
        open_debts::open_autonomous(nameless, &BTreeSet::new()).is_empty(),
        "the control: this row is not in the autonomous branch at all, so the \
         walk alone would call the arc finished"
    );
    assert!(
        !open_debts::finished(nameless, &BTreeSet::new()),
        "but a closure naming nobody is a claim the ledger cannot support, and \
         it blocks from whatever branch it sits in"
    );

    let dangling = "- **N913**(②) — done. CLOSED (R1, 커밋 `4a4d0e0`)\n";
    let unresolved: BTreeSet<String> = ["4a4d0e0".to_string()].into_iter().collect();
    assert!(
        open_debts::finished(dangling, &BTreeSet::new()),
        "the control: with that commit present, this ledger is finished"
    );
    assert!(
        !open_debts::finished(dangling, &unresolved),
        "and with it absent the arc must not be called finished"
    );
}

/// The two write paths answer the same question the same way.
///
/// THE DEFECT WAS THAT THEY DID NOT. `retired` demanded a run of ids reaching
/// the word and `open_autonomous` accepted it anywhere in the row, so the same
/// six letters meant two things one function apart — and it was the looser one
/// that decided the count.
#[test]
fn the_row_reader_and_the_line_reader_agree_about_what_a_retirement_is() {
    for line in [
        "- **N906**(①) — the `CLOSED` marker is this row's subject.",
        "- **N906**(①) — not CLOSED by anything named here.",
    ] {
        let ledger = format!("{line}\n");
        assert!(
            !retired(&ledger).contains("N906"),
            "the line reader must not retire it: {line}"
        );
        assert_eq!(
            open_autonomous(&ledger).len(),
            1,
            "and neither must the row reader: {line}"
        );
    }
}
