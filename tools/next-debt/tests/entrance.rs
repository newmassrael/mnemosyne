//! The program run as a process, because the three exit codes ARE the contract.
//!
//! ASKED OF THE PROCESS AND NOT ONLY OF THE LIBRARY, which is the lesson the
//! crate next door paid for twice: a refusal path can be plumbed, carried into
//! every report and never once taken, and a library test handed the answer
//! proves the plumbing rather than the asking. What a caller of this program
//! has is `0`, `1`, `2` and the lines above them; nothing else is the interface,
//! and `main` is where all three are decided.
//!
//! THE FIXTURES ARE TWO TEXT FILES, which is all this program reads: a ledger,
//! and the process document inside the tree `--repo` names. Nothing here writes
//! a file it then executes — `tools/written-executable` refuses that shape, and
//! the reason is `ETXTBSY` under a run that has ten other crates forking.

use std::path::Path;
use std::process::{Command, Output};

use next_debt::{REAIMING_CAP, UNRANKED_RATCHET};

/// A ledger with one critical row, one ordinary one and one nobody ranked.
fn a_ledger(pin: usize) -> String {
    format!(
        "# a fixture ledger

- **{UNRANKED_RATCHET} = {pin}** \u{2014} what the pin says.

- **N300**(\u{2460}, **critical**, `@from: none`) \u{2014} take this first.
- **N301**(\u{2460}, ordinary, `@from: N300`) \u{2014} what repaying it made.
- **N302**(\u{2460}) \u{2014} nobody ranked this one.
"
    )
}

/// A process document that declares the cap.
fn a_rulebook(cap: usize) -> String {
    format!(
        "# a fixture process document\n\n- **{REAIMING_CAP} = {cap}** \u{2014} deeper defers.\n"
    )
}

/// A tree with both documents in the places the program looks for them.
fn a_case(ledger: &str, rulebook: &str) -> tempfile::TempDir {
    let tree = tempfile::tempdir().expect("a directory to stand in");
    std::fs::write(tree.path().join("ledger.md"), ledger).expect("the fixture ledger is written");
    std::fs::write(tree.path().join("RULEBOOK.md"), rulebook)
        .expect("the fixture process document is written");
    tree
}

fn ask(tree: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_next-debt"))
        .args(["--ledger"])
        .arg(tree.join("ledger.md"))
        .args(["--repo"])
        .arg(tree)
        .output()
        .expect("the program can be run")
}

/// What the program says, both streams together, because a refusal is on one and
/// a report is on the other and a case should not have to know which.
fn spoken(answer: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&answer.stdout),
        String::from_utf8_lossy(&answer.stderr)
    )
}

#[test]
fn a_sound_ledger_whose_pin_matches_is_zero_and_prints_the_critical_line() {
    let tree = a_case(&a_ledger(1), &a_rulebook(1));
    let answer = ask(tree.path());
    let said = spoken(&answer);
    assert_eq!(answer.status.code(), Some(0), "{said}");
    assert!(said.contains("critical 1"), "{said}");
    assert!(said.contains("N300 (line 5)"), "{said}");
    assert!(said.contains("unranked 1, pinned at 1"), "{said}");
    assert!(said.contains("deferred 0 at depth > 1"), "{said}");
}

/// THE RATCHET IS THE ONE THING THIS ENFORCES, and it is enforced in the
/// direction that matters: a row opened without a rank raises the count, and the
/// repair named is to rank the row rather than to move the pin.
#[test]
fn more_unranked_rows_than_the_pin_allows_is_one() {
    let tree = a_case(&a_ledger(0), &a_rulebook(1));
    let answer = ask(tree.path());
    let said = spoken(&answer);
    assert_eq!(answer.status.code(), Some(1), "{said}");
    assert!(said.contains("the ratchet is BROKEN"), "{said}");
}

/// AND A PIN NOBODY TIGHTENED IS ALSO ONE, which is what makes it a ratchet
/// rather than a ceiling: a pin left above the ledger passes every measurement
/// under it, which is the shape of a gate that has stopped being read.
#[test]
fn a_pin_the_ledger_has_fallen_below_is_one() {
    let tree = a_case(&a_ledger(4), &a_rulebook(1));
    let answer = ask(tree.path());
    let said = spoken(&answer);
    assert_eq!(answer.status.code(), Some(1), "{said}");
    assert!(said.contains("the pin is LOOSE"), "{said}");
    assert!(
        said.contains(&format!("{UNRANKED_RATCHET} = 1")),
        "the replacement line is spelled out: {said}"
    );
}

/// A CAP NO DOCUMENT DECLARES IS NOT A CAP OF ZERO. Being unable to read the
/// number is `2`, and the program carries no value of its own to fall back on.
#[test]
fn a_process_document_declaring_no_cap_is_two() {
    let tree = a_case(
        &a_ledger(1),
        "# a fixture process document\n\nno cap here.\n",
    );
    let answer = ask(tree.path());
    let said = spoken(&answer);
    assert_eq!(answer.status.code(), Some(2), "{said}");
    assert!(said.contains("NO VERDICT"), "{said}");
    assert!(said.contains(REAIMING_CAP), "{said}");
}

/// AND A LEDGER WITH NOTHING OPEN IN IT IS `2` RATHER THAN A CELEBRATION, the
/// rule this borrows whole from the census: a walk that read nothing and a
/// finished arc print the same silence, and only one of them is done.
#[test]
fn a_ledger_with_no_open_row_is_not_judged_here() {
    let tree = a_case(
        &format!("- **{UNRANKED_RATCHET} = 0** \u{2014} nothing is open.\n"),
        &a_rulebook(1),
    );
    let answer = ask(tree.path());
    let said = spoken(&answer);
    assert_eq!(answer.status.code(), Some(2), "{said}");
    assert!(
        said.contains("open-debts"),
        "it names who does judge it: {said}"
    );
}

/// A ROW THIS READER CANNOT CLASSIFY STOPS THE ANSWER, and names itself.
#[test]
fn a_row_that_cannot_be_steered_by_is_two() {
    let tree = a_case(
        &format!(
            "- **{UNRANKED_RATCHET} = 0**\n\n- **N400**(\u{2460}, ordinary, `@from: N999`) \u{2014} from what.\n"
        ),
        &a_rulebook(1),
    );
    let answer = ask(tree.path());
    let said = spoken(&answer);
    assert_eq!(answer.status.code(), Some(2), "{said}");
    assert!(said.contains("N400"), "{said}");
    assert!(said.contains("N999"), "{said}");
}
