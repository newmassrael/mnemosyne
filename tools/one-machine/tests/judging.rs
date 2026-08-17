//! The decision, asked of every state it has.
//!
//! THE THIRD CODE IS WHAT THESE ARE ABOUT. A two-code gate reports zero findings
//! in every one of the states below — nothing dispatched, the answer taken on
//! this very machine, an answer about bytes that have changed, a run still going
//! — and zero findings is what a clean tree looks like. Each case names the one
//! sentence that state must produce, because a gate whose refusals are all one
//! message sends every reader to the same wrong repair.

use one_machine::{judge, read_log, read_placement, started_ago, Claim, Header, Seen, Verdict};

const HERE: &str = "1111111111111111111111111111111111111111";
const SENT: &str = "2222222222222222222222222222222222222222";

fn claim(fingerprint: &str) -> Claim {
    Claim {
        host: "elsewhere-alias".to_owned(),
        log: "$HOME/.remote-build/one-machine/mnemosyne.log".to_owned(),
        fingerprint: fingerprint.to_owned(),
        launched: 1_000,
    }
}

/// A log the way the far side writes one: the header, then the two results, then
/// the launcher's sentinel.
fn log(
    host: &str,
    fingerprint: &str,
    suite: Option<i64>,
    census: Option<i64>,
    ended: bool,
) -> String {
    let mut text = Header {
        host: host.to_owned(),
        fingerprint: fingerprint.to_owned(),
        started: 1_000,
    }
    .render();
    text.push('\n');
    if let Some(code) = suite {
        text.push_str(&format!("one-machine suite exit={code}\n"));
    }
    if let Some(code) = census {
        text.push_str(&format!("one-machine census exit={code}\n"));
    }
    if ended {
        text.push_str("REMOTE_BUILD_EXIT=0\n");
    }
    text
}

fn seen(claim: Option<Result<Claim, String>>, log: Result<String, String>) -> Seen {
    Seen {
        claim,
        log,
        here: HERE.to_owned(),
        this_host: "the-machine-that-wrote-the-gate".to_owned(),
    }
}

fn not_judged(verdict: &Verdict) -> &str {
    match verdict {
        Verdict::NotJudged(message) => message,
        other => panic!("expected NO VERDICT, got {other:?}"),
    }
}

fn finding(verdict: &Verdict) -> &str {
    match verdict {
        Verdict::Finding(message) => message,
        other => panic!("expected a finding, got {other:?}"),
    }
}

#[test]
fn nothing_dispatched_is_not_a_clean_check() {
    let verdict = judge(&seen(None, Err("nothing was dispatched".to_owned())));
    assert_eq!(verdict.code(), 2, "{verdict:?}");
    let message = not_judged(&verdict);
    assert!(
        message.contains("no machine has been asked"),
        "the state a two-code gate calls clean has to name itself: {message}"
    );
}

#[test]
fn a_claim_that_cannot_be_read_is_not_an_absent_one() {
    let verdict = judge(&seen(
        Some(Err("no `one-machine claim` line in it".to_owned())),
        Err("nothing was dispatched".to_owned()),
    ));
    assert_eq!(verdict.code(), 2, "{verdict:?}");
    let message = not_judged(&verdict);
    assert!(
        message.contains("cannot be read") && message.contains("not the same as nothing"),
        "something WAS sent and this cannot say where — a different repair: {message}"
    );
}

#[test]
fn a_log_that_cannot_be_fetched_is_not_a_clean_check() {
    let verdict = judge(&seen(
        Some(Ok(claim(HERE))),
        Err("ssh exited 255: Connection refused".to_owned()),
    ));
    assert_eq!(verdict.code(), 2, "{verdict:?}");
    assert!(
        not_judged(&verdict).contains("Connection refused"),
        "the transport's own words, not a summary of them"
    );
}

#[test]
fn a_log_with_no_header_is_not_a_census_of_ours() {
    let verdict = judge(&seen(
        Some(Ok(claim(HERE))),
        Ok("bash: scripts/census-elsewhere.sh: No such file or directory\n".to_owned()),
    ));
    assert_eq!(verdict.code(), 2, "{verdict:?}");
    assert!(
        not_judged(&verdict).contains("no `one-machine header` line"),
        "a log that is not a census is not evidence about anything"
    );
}

/// The premise of the whole crate.
#[test]
fn a_census_taken_on_this_machine_is_not_a_second_opinion() {
    let verdict = judge(&seen(
        Some(Ok(claim(HERE))),
        Ok(log(
            "the-machine-that-wrote-the-gate",
            HERE,
            Some(0),
            Some(0),
            true,
        )),
    ));
    assert_eq!(
        verdict.code(),
        2,
        "a clean census in every other respect, taken HERE: {verdict:?}"
    );
    assert!(
        not_judged(&verdict).contains("THIS machine"),
        "this is the one state the gate exists to refuse"
    );
}

/// And the name is compared without regard to case, because two machines do not
/// stop being two because one of them shouts.
#[test]
fn the_machine_is_the_same_machine_whatever_its_case() {
    let verdict = judge(&seen(
        Some(Ok(claim(HERE))),
        Ok(log(
            "The-Machine-That-Wrote-The-Gate",
            HERE,
            Some(0),
            Some(0),
            true,
        )),
    ));
    assert_eq!(verdict.code(), 2, "{verdict:?}");
}

#[test]
fn what_arrived_is_not_what_left() {
    let verdict = judge(&seen(
        Some(Ok(claim(SENT))),
        Ok(log("a-build-machine", HERE, Some(0), Some(0), true)),
    ));
    assert_eq!(verdict.code(), 2, "{verdict:?}");
    let message = not_judged(&verdict);
    assert!(
        message.contains("did not send it"),
        "a transport disagreement is the transport's defect, not a stale round: {message}"
    );
}

#[test]
fn a_census_about_other_bytes_is_not_about_these() {
    let verdict = judge(&seen(
        Some(Ok(claim(SENT))),
        Ok(log("a-build-machine", SENT, Some(0), Some(0), true)),
    ));
    assert_eq!(verdict.code(), 2, "{verdict:?}");
    let message = not_judged(&verdict);
    assert!(
        message.contains("another tree") && message.contains(HERE),
        "it must print both fingerprints so the reader can tell which way it drifted: {message}"
    );
}

#[test]
fn a_run_that_has_not_ended_has_not_answered() {
    let verdict = judge(&seen(
        Some(Ok(claim(HERE))),
        Ok(log("a-build-machine", HERE, None, None, false)),
    ));
    assert_eq!(verdict.code(), 2, "{verdict:?}");
    assert!(
        not_judged(&verdict).contains("has not ended"),
        "the sentinel is the only thing that says a detached run is over"
    );
}

/// A census reads the run it rides on, so a suite that died early is a shorter
/// run — and a clean census over it means less than it looks like.
#[test]
fn a_suite_that_failed_elsewhere_is_a_finding_of_its_own() {
    let verdict = judge(&seen(
        Some(Ok(claim(HERE))),
        Ok(log("a-build-machine", HERE, Some(101), Some(0), true)),
    ));
    assert_eq!(verdict.code(), 1, "{verdict:?}");
    let message = finding(&verdict);
    assert!(
        message.contains("101") && message.contains("shorter run"),
        "it has to say WHY a clean census over a red suite is not a clean answer: {message}"
    );
}

#[test]
fn a_suite_whose_status_was_never_recorded_is_not_passed_over() {
    let verdict = judge(&seen(
        Some(Ok(claim(HERE))),
        Ok(log("a-build-machine", HERE, None, Some(0), true)),
    ));
    assert_eq!(
        verdict.code(),
        1,
        "an unrecorded suite status is not a green one: {verdict:?}"
    );
}

#[test]
fn a_census_that_found_something_is_a_finding() {
    let verdict = judge(&seen(
        Some(Ok(claim(HERE))),
        Ok(log("a-build-machine", HERE, Some(0), Some(1), true)),
    ));
    assert_eq!(verdict.code(), 1, "{verdict:?}");
    assert!(finding(&verdict).contains("nothing declares"));
}

#[test]
fn a_census_that_refused_is_not_a_census_that_passed() {
    let verdict = judge(&seen(
        Some(Ok(claim(HERE))),
        Ok(log("a-build-machine", HERE, Some(0), Some(2), true)),
    ));
    assert_eq!(verdict.code(), 2, "{verdict:?}");
    assert!(not_judged(&verdict).contains("could not judge"));
}

#[test]
fn a_run_that_ended_writing_no_verdict_is_not_judged() {
    let verdict = judge(&seen(
        Some(Ok(claim(HERE))),
        Ok(log("a-build-machine", HERE, Some(0), None, true)),
    ));
    assert_eq!(verdict.code(), 2, "{verdict:?}");
    assert!(not_judged(&verdict).contains("records no census verdict"));
}

#[test]
fn a_clean_census_from_another_machine_is_the_answer_this_gate_is_for() {
    let verdict = judge(&seen(
        Some(Ok(claim(HERE))),
        Ok(log("a-build-machine", HERE, Some(0), Some(0), true)),
    ));
    assert_eq!(verdict.code(), 0, "{verdict:?}");
    match verdict {
        Verdict::Elsewhere { host, .. } => assert_eq!(host, "a-build-machine"),
        other => panic!("expected the machine to be named, got {other:?}"),
    }
}

/// The order of the checks is the order of the repairs, and this is the case
/// that proves the order rather than the set: a log that is BOTH this machine's
/// AND about older bytes must send the reader to the machine, because
/// re-dispatching does not fix a gate that is its own second opinion.
#[test]
fn identity_is_answered_before_freshness() {
    let verdict = judge(&seen(
        Some(Ok(claim(SENT))),
        Ok(log(
            "the-machine-that-wrote-the-gate",
            SENT,
            Some(0),
            Some(0),
            true,
        )),
    ));
    assert_eq!(verdict.code(), 2, "{verdict:?}");
    let message = not_judged(&verdict);
    assert!(
        message.contains("THIS machine"),
        "both are wrong; the machine is the one that re-dispatching would not fix: {message}"
    );
}

/// The transport can leave the cursor where the wrapped command left it, so the
/// launcher's append lands on the tail of another line. The remote-build
/// protocol records that exact shape; a reader anchored to the start of a line
/// answers "still running" about a run that ended.
#[test]
fn the_sentinel_is_found_when_the_transport_leaves_it_mid_line() {
    let mut text = log("a-build-machine", HERE, Some(0), Some(0), false);
    text.push_str("test result: ok. 2047 passed; 0 failedREMOTE_BUILD_EXIT=0\n");
    let verdict = judge(&seen(Some(Ok(claim(HERE))), Ok(text)));
    assert_eq!(verdict.code(), 0, "{verdict:?}");
}

#[test]
fn a_header_and_a_claim_survive_being_written_and_read() {
    let header = Header {
        host: "a-build-machine".to_owned(),
        fingerprint: HERE.to_owned(),
        started: 1_234,
    };
    let answer = read_log(&header.render());
    assert_eq!(answer.header.as_ref(), Some(&header));

    let original = claim(SENT);
    let round_tripped = Claim::read(&original.render()).expect("a claim this crate wrote");
    assert_eq!(round_tripped, original);
}

#[test]
fn a_claim_missing_a_field_is_unreadable_rather_than_defaulted() {
    let text = "one-machine claim host=pc2 log=/x fingerprint=abc\n";
    let error = Claim::read(text).expect_err("a claim with no `launched`");
    assert!(error.contains("launched"), "{error}");

    let error = Claim::read("").expect_err("no claim at all");
    assert!(error.contains("no `one-machine claim` line"), "{error}");
}

#[test]
fn the_placement_program_is_read_rather_than_second_guessed() {
    let remote = read_placement(
        "bx: WHERE=remote host=pc2\n\
         where=remote host=pc2 budget=30 fleet=pc2=30c\n\
         why=local is 9GB into swap\n",
    )
    .expect("the seam's own output");
    assert!(remote.remote);
    assert_eq!(remote.host.as_deref(), Some("pc2"));
    assert_eq!(remote.why, "local is 9GB into swap");

    let here = read_placement("where=local host= budget=- fleet=-\nwhy=no host answered\n")
        .expect("a local placement is an answer, not an error");
    assert!(!here.remote);
    assert_eq!(here.host, None);
    assert_eq!(here.why, "no host answered");

    let error = read_placement("bx: unknown flag --explain-choice\n")
        .expect_err("a program without the seam");
    assert!(error.contains("--explain-choice"), "{error}");
}

#[test]
fn a_stamp_from_a_clock_ahead_of_this_one_does_not_go_backwards() {
    assert_eq!(started_ago(u64::MAX), 0);
}

/// A log the size of the ones this really produces, with one line of each kind.
fn a_real_sized_log() -> String {
    let mut text = log("a-build-machine", HERE, Some(101), Some(1), true);
    for number in 0..2_000 {
        text.push_str(&format!(
            "test case_number_{number} ... ok\n     Running tests/case_{number}.rs\n"
        ));
    }
    text.push_str("test result: FAILED. 378 passed; 1 failed\n");
    text.push_str("thread 'a_case' panicked at crates/x/tests/common/mod.rs:73:22:\n");
    text.push_str("[outside-reach] 121023 process(es), 7927800 line(s)\n");
    text
}

/// MEASURED ON THE REAL ONES: 2,766 lines of which 17 decide, and 2,815 of which
/// 23 do when the suite failed. Both callers here are git hooks, so a gate that
/// prints the whole log does not carry the finding to the person pushing — it
/// buries it under the ordinary output of a passing suite.
#[test]
fn what_the_reader_is_shown_is_what_decides_and_not_the_whole_log() {
    let text = a_real_sized_log();
    let shown = one_machine::carrying_lines(&text);
    assert!(
        text.lines().count() > 4_000,
        "the case has to be about a log the size of a real one"
    );
    assert!(
        shown.len() < 20,
        "a hook cannot print {} lines and be read: {shown:?}",
        shown.len()
    );
    assert!(
        !shown.iter().any(|line| line.contains("case_number_1000")),
        "the ordinary output of a passing suite is not what decides"
    );
    // AND EVERY KIND THAT DOES DECIDE SURVIVES, unedited and in order.
    for kept in [
        "one-machine header",
        "one-machine suite exit=101",
        "one-machine census exit=1",
        "REMOTE_BUILD_EXIT=0",
        "[outside-reach] 121023 process(es)",
        "test result: FAILED",
        "panicked at",
    ] {
        assert!(
            shown.iter().any(|line| line.contains(kept)),
            "`{kept}` is part of the answer and was dropped: {shown:?}"
        );
    }
    assert!(
        shown.iter().all(|line| text.contains(*line)),
        "every line shown is the far side's own, unedited"
    );
}

/// And a clean verdict carries the size of what was looked at, because "nothing
/// undeclared" is also what a census over an empty trace says.
#[test]
fn a_clean_answer_carries_the_far_sides_own_count_of_what_it_read() {
    let text = a_real_sized_log();
    let scale = one_machine::scale_of(&text).expect("the census printed its own count");
    assert!(scale.contains("121023 process(es)"), "{scale}");
    assert_eq!(
        one_machine::scale_of("one-machine header host=x fingerprint=y started=1"),
        None,
        "a log that never counted anything has no count to print"
    );
}
