//! What this reporter says, and what it refuses to leave unsaid.
//!
//! THE READING HAS ITS OWN FILE (`github.rs`, against recorded bodies). This one
//! is about the sentences: which rows get printed, which get counted, and which
//! of the four verdicts a commit lands on. Both halves are needed and neither
//! substitutes — a reporter that parses GitHub perfectly and prints the wrong
//! sentence is the failure R1125 shipped one gate over, where an oracle matching
//! a SUBSTRING of the failure output agreed with the output it existed to refuse.

use ci_state::{annotation_report, one_line, report, verdict, Annotation, Check, Output, Verdict};

const SHA: &str = "2d630331b1279e3b7a28985876b53ef0b07fbe77";

/// A check with a conclusion, spelled the way GitHub spells one.
fn check(id: u64, name: &str, conclusion: Option<&str>, annotations: u64) -> Check {
    Check {
        id,
        name: name.to_string(),
        head_sha: SHA.to_string(),
        status: if conclusion.is_some() {
            "completed".to_string()
        } else {
            "in_progress".to_string()
        },
        conclusion: conclusion.map(str::to_string),
        output: Output {
            annotations_count: annotations,
        },
    }
}

fn note(level: &str, message: &str) -> Annotation {
    Annotation {
        annotation_level: level.to_string(),
        message: message.to_string(),
    }
}

/// The census names every row, and the lines name every row that is not routine.
///
/// BOTH HALVES OR NEITHER IS HONEST. Printing all nine rows on every green push
/// trains a reader to skip the block; printing only the failures says nothing
/// about how much was looked at. The counts are what makes the omission legible,
/// which is the rule the annotation cap below already followed.
#[test]
fn the_census_names_every_row_and_the_lines_name_every_row_that_is_not_success() {
    let checks = [
        check(1, "validate", Some("success"), 0),
        check(
            2,
            "every cache declared is one CI keeps",
            Some("failure"),
            1,
        ),
        check(3, "every compilation is one job's", Some("skipped"), 0),
        check(4, "MSRV", Some("success"), 0),
    ];
    let lines = report(SHA, &checks);
    assert!(
        lines[0].contains("4 check(s)")
            && lines[0].contains("2 success")
            && lines[0].contains("1 failure")
            && lines[0].contains("1 skipped"),
        "the census accounts for every row: {}",
        lines[0]
    );
    let body = lines[1..].join("\n");
    assert!(
        body.contains("every cache declared is one CI keeps") && body.contains("failure"),
        "the failure is named: {body}"
    );
    assert!(
        body.contains("every compilation is one job's"),
        "and so is the skip, which is not routine even though it is not red: {body}"
    );
    assert!(
        !body.contains("validate") && !body.contains("MSRV"),
        "the successes are counted rather than listed: {body}"
    );
}

/// A red commit is SAID to be red, in the words the hook used to print.
#[test]
fn a_red_commit_is_told_it_is_red_and_told_that_nothing_is_blocking() {
    let checks = [check(1, "validate", Some("failure"), 0)];
    let said = report(SHA, &checks).join("\n");
    assert!(said.contains("is RED"), "{said}");
    assert!(
        said.contains("Not blocking"),
        "the semantics R890 argued for from the history, not a softening of it: {said}"
    );
}

/// A clear commit is not told anything about being red.
///
/// THE CONTROL. A reporter that always printed the warning would be as useless as
/// one that never did, and only this direction says which of the two it is.
#[test]
fn a_clear_commit_is_not_warned_about_anything() {
    let checks = [
        check(1, "validate", Some("success"), 0),
        check(2, "MSRV", Some("neutral"), 0),
    ];
    assert_eq!(verdict(&checks), Verdict::Clear);
    let said = report(SHA, &checks).join("\n");
    assert!(!said.contains("RED"), "{said}");
    assert!(
        said.contains("2 check(s)"),
        "and it still says how much it looked at: {said}"
    );
}

/// A commit whose checks have not finished is neither red nor clear.
///
/// THE THIRD ANSWER THE PROJECTION COULD NOT GIVE. `(.conclusion // "-")` wrote a
/// dash for a check still running and then asked whether any line ended in one of
/// four failing words, so "still running" and "green" were one answer.
#[test]
fn a_commit_still_running_is_neither_red_nor_clear() {
    let checks = [
        check(1, "validate", Some("success"), 0),
        check(2, "MSRV", None, 0),
    ];
    assert_eq!(verdict(&checks), Verdict::Pending);
    let said = report(SHA, &checks).join("\n");
    assert!(!said.contains("RED"), "nothing has failed yet: {said}");
    assert!(
        said.contains("still running"),
        "and the reader is told the answer is not in yet: {said}"
    );
}

/// A failure outweighs an unfinished check.
///
/// THE ORDER MATTERS AND IT IS ASSERTED: a commit with one failed job and one job
/// still going is RED now, and a reporter that answered `Pending` would ask
/// somebody to wait for news that has already arrived.
#[test]
fn a_failure_beside_an_unfinished_check_is_red_now() {
    let checks = [
        check(1, "validate", Some("failure"), 0),
        check(2, "MSRV", None, 0),
    ];
    assert_eq!(verdict(&checks), Verdict::Red);
}

/// A commit nothing ran on says exactly that.
#[test]
fn a_commit_with_no_checks_says_nothing_has_run_on_it() {
    let said = report(SHA, &[]).join("\n");
    assert!(
        said.contains("no CI checks") && said.contains("2d630331"),
        "and it names the commit: {said}"
    );
    assert!(
        !said.contains("RED"),
        "a commit nothing ran on is not a commit that failed: {said}"
    );
}

/// Every line names the commit by its first eight characters, and no more.
#[test]
fn the_commit_is_printed_short_and_it_is_the_commit_asked_about() {
    let said = report(SHA, &[check(1, "validate", Some("success"), 0)]).join("\n");
    assert!(said.contains("2d630331"), "{said}");
    assert!(
        !said.contains(SHA),
        "the whole sha is forty characters of noise in a hook's output: {said}"
    );
}

/// The same annotation from several jobs is one thing a reader wants to see once.
///
/// AND BOTH NUMBERS ARE PRINTED. GitHub emits one annotation per job, so eight
/// copies of "Node.js 20 actions are deprecated" is one fact — but a line saying
/// only "1 annotation" would understate what the commit reported.
#[test]
fn annotations_are_deduplicated_and_both_numbers_are_printed() {
    let read = vec![
        note("warning", "Node.js 20 actions are deprecated"),
        note("warning", "Node.js 20 actions are deprecated"),
        note("failure", "Process completed with exit code 1."),
    ];
    let said = annotation_report(SHA, 3, &read).join("\n");
    assert!(said.contains("2 distinct of 3 reported"), "{said}");
    assert_eq!(
        said.matches("Node.js 20").count(),
        1,
        "the repeated one is printed once: {said}"
    );
    assert!(said.contains("exit code 1."), "{said}");
}

/// A cap that does not say what it dropped reads as "that was all of them".
#[test]
fn more_annotations_than_are_shown_are_counted_rather_than_dropped() {
    let read: Vec<Annotation> = (0..14)
        .map(|n| note("warning", &format!("finding number {n}")))
        .collect();
    let said = annotation_report(SHA, 14, &read).join("\n");
    assert!(said.contains("14 distinct of 14 reported"), "{said}");
    assert_eq!(
        said.lines().filter(|line| line.contains("finding")).count(),
        10,
        "ten are shown: {said}"
    );
    assert!(
        said.contains("(+4 distinct not shown)"),
        "and the other four are counted: {said}"
    );
}

/// A commit that reported annotations none of which could be read says so.
///
/// NOT "no annotations", which is the other answer entirely: one is a quiet
/// commit and the other is a reporter that failed to fetch what the commit said.
#[test]
fn annotations_declared_but_unread_are_not_reported_as_none() {
    let said = annotation_report(SHA, 3, &[]).join("\n");
    assert!(said.contains("3 annotation(s), none readable"), "{said}");
    let quiet = annotation_report(SHA, 0, &[]).join("\n");
    assert!(
        quiet.contains("no CI annotations") && !quiet.contains("none readable"),
        "and a commit with nothing to say is not that: {quiet}"
    );
}

/// An annotation is printed as its level and the first line of its message.
#[test]
fn an_annotation_prints_as_its_level_and_its_first_line() {
    let long = note(
        "failure",
        "error[E0308]: mismatched types\n  --> src/lib.rs:12:5\n   |\n12 | ok\n",
    );
    let line = one_line(&long);
    assert_eq!(line, "failure error[E0308]: mismatched types");
    assert!(
        !line.contains("src/lib.rs"),
        "a whole diagnostic is not a line: {line}"
    );
}

/// A long line is cut by CHARACTERS, and a message that is not ASCII does not
/// take the reporter down with it.
///
/// THE BYTE SLICE THIS REPLACES WOULD PANIC. A reporter that dies on somebody
/// else's error text is worse than one that prints nothing, and a compiler
/// diagnostic quoting source is exactly where a non-ASCII character arrives.
#[test]
fn a_long_message_is_cut_by_characters_and_survives_a_non_ascii_one() {
    let wide = note("warning", &"가".repeat(400));
    let line = one_line(&wide);
    assert_eq!(
        line.chars().count(),
        "warning ".chars().count() + 160,
        "one hundred and sixty characters of message: {line}"
    );
}
