//! What CI said about one commit — READ out of GitHub's answer, never projected
//! out of it by `gh -q`.
//!
//! WHY THIS PROGRAM EXISTS. `.githooks/pre-push` reports the CI state of the
//! commit a push builds on, because nothing in this repository could see a red
//! CI and four consecutive red runs went unnoticed for eleven hours (R890), and
//! because a green conclusion is not the whole of what a run says (R893). It did
//! that with three `gh -q '<expression>'` calls, and those expressions were the
//! one part of the hook nothing could test: `crates/mnemosyne-cli/tests/
//! git_hooks_smoke.rs` drives the real hook with a STUB `gh` that returns
//! already-filtered lines, and its own header said what that costs — "a wrong
//! `--json` field or jq expression in the hook would still pass here".
//!
//! IT WAS MEASURED BEFORE IT WAS MOVED, twice, each time by breaking one
//! expression and running the suite that is supposed to hold the hook up:
//!
//!   - `.conclusion` renamed to `.verdict` in the run filter: 14 passed, 0
//!     failed. With a real `gh` every conclusion then prints as `-`, the string
//!     the hook substitutes for a run that has not concluded, so the red-detection
//!     that follows can never match and a RED commit reports clean.
//!   - `annotations_count` renamed to `annotation_count` in the check-run filter:
//!     14 passed, 0 failed. With a real `gh` no annotated check is ever selected,
//!     so every commit reports "no CI annotations" — which is exactly the
//!     blindness R893 added the call to end.
//!
//! Both are silent in the direction that reads as good news, which is the
//! property R1130 named when it moved `cache-budget` off the same kind of
//! expression: a read that comes back short is a well-formed answer describing a
//! healthier repository than the one that exists.
//!
//! ONE ENDPOINT, AND IT IS THE PER-COMMIT ONE. The hook asked `gh run list
//! --limit 20` and filtered it for the commit's sha, which is a WINDOW: a commit
//! that has fallen off the end of the twenty most recent runs is indistinguishable
//! from a commit nothing ever ran on, and the hook printed the second sentence for
//! both. `repos/{owner}/{repo}/commits/<sha>/check-runs` is asked ABOUT the commit,
//! carries GitHub's own `total_count` beside its rows so a short read is loud
//! rather than silent, and names the JOB rather than the workflow — which is the
//! grain the last two red runs in this repository actually differed at.

use std::collections::BTreeSet;

/// What a push learned about what CI cost, kept, so the next one can read a
/// trend rather than a level (R1260).
pub mod history;

/// What GitHub calls a conclusion that means the commit did not pass.
///
/// EXACTLY THE SET R890 CHOSE, kept whole rather than narrowed while the reading
/// around it changed: `failure`, `cancelled`, `timed_out` and `startup_failure`
/// are what the hook's own `grep -E` matched. `skipped` and `neutral` are NOT
/// here and that is deliberate — this repository's workflow skips a job whose
/// inputs did not change on every green push, and a reporter that called that red
/// would be ignored within a day.
pub const FAILING: [&str; 4] = ["failure", "cancelled", "timed_out", "startup_failure"];

/// What GitHub calls a job that never ran.
///
/// NOT RED, AND NOT A DURATION EITHER (R1261). The comment above says why this
/// word is absent from `FAILING`; what R1260's record then found is that it is
/// absent from the other question too. This repository's workflow skips a job
/// whose inputs did not change, GitHub stamps such a job's start and completion
/// at the same instant, and the block that reads cost therefore recorded it as
/// ZERO SECONDS — which is not a cost of nothing, it is the absence of a
/// measurement. Measured on the first full record: 9 of the 23 rows for `every
/// compilation is one job's` and 8 of 23 for `every cache declared is one CI
/// keeps` were skips.
pub const DID_NOT_RUN: &str = "skipped";

/// What GitHub calls a job that ran all of its steps.
///
/// THE POPULATION OF A TREND, and only of a trend (R1261). A job that FAILED ran
/// and stopped where it broke, so its duration is a time-to-failure — `validate`
/// took 331 s on `d412b06e` against some 2400 s on the commits around it, and
/// nothing about that is the work getting cheaper. A job its own budget ended is
/// a CENSORED measurement: `validate` on `cabcd5cf` is 90m02s of the 90 minutes
/// it declares, which is the most important number in the whole record and still
/// not a point on a cost curve. Both stay in the record and in the level line,
/// where they are true; neither is a point in a movement, and the sentence that
/// prints a movement says how many it set aside and how far the longest of them
/// got.
pub const RAN_TO_COMPLETION: &str = "success";

/// One check run on a commit, as GitHub answers for it.
///
/// NAMED FIELDS AND NOT A PROJECTION, which is what `Deserialize` here buys: a
/// field GitHub renames stops this program rather than emptying it, and
/// `tests/github.rs` holds that rename against a RECORDED REAL body so the day it
/// happens is a red test. Every other field on the row — `app`, `check_suite`,
/// `html_url`, `pull_requests` — is ignored on purpose: a reader that refused
/// unknown fields would go red the day GitHub adds one, which is a gate failing
/// for somebody else's work.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct Check {
    pub id: u64,
    pub name: String,
    /// Where GitHub sends a person who clicks this check, and — for a check an
    /// Actions job is the face of — the only thing in this answer that names the
    /// JOB behind it. [`job_of`] reads the id out of it.
    ///
    /// IT WAS IGNORED ON PURPOSE UNTIL R1236, and what that cost is written in
    /// that round: this endpoint's rows carry a conclusion and no steps, so a job
    /// reported as `cancelled` after 45 minutes is indistinguishable from one this
    /// repository's own change hung. It was `apt-get` stalling before anything of
    /// this repository ran, and finding that out took a second tool.
    pub details_url: String,
    /// The commit this row is ABOUT, which is not the same fact as the commit it
    /// was asked for. A well-formed answer to the wrong question is the failure
    /// R1122 paid for in the neighbouring gate — perfectly shaped, and carrying
    /// the wrong subject — so the two are compared rather than assumed equal.
    pub head_sha: String,
    /// `queued`, `in_progress` or `completed`, verbatim.
    pub status: String,
    /// `None` while the check has not concluded.
    ///
    /// A THIRD ANSWER, and the projection this replaces had only two: it wrote
    /// `(.conclusion // "-")`, so a check still running and a check whose
    /// conclusion GitHub stopped sending both arrived as the same dash, and
    /// neither matched the red-detection that followed.
    ///
    /// PRESENT-BUT-NULLABLE, WHICH IS NOT WHAT `Option` MEANS TO SERDE. A derived
    /// `Option` field is OPTIONAL: a body that stopped carrying `conclusion`
    /// deserializes to `None` without complaint, and this program would then read
    /// every check on a red commit as one that has not finished — the same
    /// collapse the `// "-"` projection made, rebuilt in a type. `deserialize_with`
    /// is what takes the implicit default away, so a missing key is a named
    /// refusal and only an explicit `null` is a check still running.
    #[serde(deserialize_with = "present_but_nullable")]
    pub conclusion: Option<String>,
    /// When the job began, as GitHub stamps it, or `None` when it has not.
    ///
    /// READ SINCE R1244's SUCCESSOR, and ignored before it for the same reason
    /// `details_url` was: a conclusion says whether a job passed and says nothing
    /// about what it COST. R1229 changed the work of a job's longest step, left
    /// its `timeout-minutes` alone, and learned about it from a cancellation —
    /// with these two fields and the budget the workflow declares, the run before
    /// that one could have said the job was already at ninety per cent.
    #[serde(deserialize_with = "present_but_nullable")]
    pub started_at: Option<String>,
    /// When it ended. `None` while it is still running, which is a fact and not
    /// a missing field — hence the same `deserialize_with` as `conclusion`.
    #[serde(deserialize_with = "present_but_nullable")]
    pub completed_at: Option<String>,
    pub output: Output,
}

/// Read a field that GitHub always sends and may send as `null`.
///
/// The whole of it is [`Check::conclusion`]'s doc comment: naming a
/// `deserialize_with` is how a derived `Option` field stops being optional.
fn present_but_nullable<'de, D>(reader: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    serde::Deserialize::deserialize(reader)
}

/// The part of a check run's output this program reads.
///
/// `annotations_count` IS THE DECLARED NUMBER and it is read from here rather
/// than from the annotations themselves, because the annotations endpoint answers
/// one page and GitHub's own interface stops at fifty: the count is the only
/// thing that says whether what came back was all of it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct Output {
    pub annotations_count: u64,
}

/// One page of GitHub's answer about a commit's check runs.
///
/// `total_count` IS THE ONLY THING IN THE ANSWER THAT SAYS WHETHER THE ROWS ARE
/// ALL OF THEM. Every other failure of this read is loud — a row missing a name
/// does not parse, a body that never arrived is empty — but a read that stops
/// early is a valid answer describing a commit with fewer checks, and FEWER is
/// the direction in which a red job goes unmentioned.
#[derive(serde::Deserialize)]
struct CheckPage {
    total_count: u64,
    check_runs: Vec<Check>,
}

/// One annotation on one check run.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct Annotation {
    pub annotation_level: String,
    pub message: String,
}

/// What this program asks GitHub about a commit, as the words `gh` is handed.
///
/// IN THE LIBRARY SO THE QUESTION HAS A READER (R1096). `--paginate` is
/// load-bearing and invisible: the endpoint answers thirty rows a page, this
/// repository's workflow already declares nine jobs, and a `gh` without it would
/// answer with a body that is valid, well-shaped and short the day a tenth
/// workflow lands. `{owner}` and `{repo}` are `gh`'s own placeholders, resolved
/// from the checkout, so this program never names the repository it reports on.
pub fn checks_query(sha: &str) -> Vec<String> {
    vec![
        "api".to_string(),
        "--paginate".to_string(),
        format!("repos/{{owner}}/{{repo}}/commits/{sha}/check-runs"),
    ]
}

/// What this program asks GitHub about one check run's annotations.
///
/// ONE PAGE, DELIBERATELY, and the declared count travels beside it from
/// [`Output::annotations_count`]: a run that emitted hundreds of identical
/// annotations is not worth paging through at push time, and the shortfall is
/// PRINTED rather than hidden. `--paginate` here would trade a bounded read for
/// an unbounded one to answer a question nobody asked.
pub fn annotations_query(check_id: u64) -> Vec<String> {
    vec![
        "api".to_string(),
        format!("repos/{{owner}}/{{repo}}/check-runs/{check_id}/annotations"),
    ]
}

/// The Actions job a check run is the face of, read out of its `details_url`.
///
/// THE ONLY LINK THE PER-COMMIT ENDPOINT CARRIES. A check run's `id` is the CHECK's
/// and the steps live on the JOB, and nothing in this answer says the two are the
/// same thing — the URL a person would click is where GitHub writes the job's id
/// down: `…/actions/runs/<run>/job/<job>`.
///
/// `None` IS AN ANSWER AND NOT A FAILURE. A check that is not an Actions job at all
/// — anything another app posts against a commit — has a `details_url` pointing
/// somewhere else entirely, and there is no job to ask about. The caller prints
/// that rather than treating it as an unreadable answer.
pub fn job_of(details_url: &str) -> Option<u64> {
    let (_, tail) = details_url.rsplit_once("/job/")?;
    tail.parse().ok()
}

/// The Actions RUN a check run belongs to, out of the same URL.
///
/// A DIFFERENT NUMBER FROM THE JOB'S AND THE ANSWER TO A DIFFERENT QUESTION.
/// Cancellation by a newer push is a property of a RUN — GitHub stops the whole
/// workflow run and every job in it — so attributing it needs the run and not the
/// job. The URL carries both, in that order: `…/actions/runs/<run>/job/<job>`.
///
/// READ FROM THE FRONT, deliberately. `job_of` splits from the RIGHT because the
/// job is the last segment; taking the run means the segment after `/runs/` and
/// before whatever follows, and a reader that split from the right here would find
/// the job's number under the run's name whenever the two happen to be adjacent.
pub fn run_of(details_url: &str) -> Option<u64> {
    let (_, tail) = details_url.split_once("/runs/")?;
    let number = tail.split('/').next()?;
    number.parse().ok()
}

/// GitHub's own words when it cancels a run because a newer push took its place.
///
/// THE ONLY THING THAT SAYS WHY A RUN WAS CANCELLED. A cancelled check carries the
/// word `cancelled` and nothing else: a job somebody stopped, a job a timeout ate
/// and a job GitHub retired in favour of a later push are one word, and the first
/// two are about THIS commit while the third is not about it at all.
///
/// A SUBSTRING OF SOMEBODY ELSE'S SENTENCE, and the failure mode of that is chosen
/// rather than accepted: if GitHub rewords it, nothing here matches, and a
/// superseded run goes back to being reported as RED. That is the LOUD direction —
/// a reader is told to look at something that turns out to be fine, rather than
/// told nothing about something that is not.
pub const SUPERSEDED_BY_A_LATER_PUSH: &str = "Canceling since a higher priority waiting request";

/// The checks whose run a LATER PUSH cancelled, by name.
///
/// PER RUN AND NOT PER CHECK, which is the whole reason this takes both arguments.
/// Measured on `74035d7` (2026-08-19): three checks cancelled, and only ONE of them
/// carried the sentence — the other two never started, so GitHub had nothing to
/// annotate them with. A reader that asked each check for its own reason would
/// call one of the three superseded and leave the other two looking like this
/// commit's failures, which is the same half-answer as a conclusion without the
/// step that ended it.
pub fn superseded_checks(checks: &[Check], read: &[Said]) -> std::collections::BTreeSet<String> {
    let mut runs = std::collections::BTreeSet::new();
    for said in read {
        if !said.annotation.message.contains(SUPERSEDED_BY_A_LATER_PUSH) {
            continue;
        }
        if let Some(run) = checks
            .iter()
            .find(|check| check.name == said.check)
            .and_then(|check| run_of(&check.details_url))
        {
            runs.insert(run);
        }
    }
    checks
        .iter()
        .filter(|check| is_failing(check))
        .filter(|check| run_of(&check.details_url).is_some_and(|run| runs.contains(&run)))
        .map(|check| check.name.clone())
        .collect()
}

/// What this program asks GitHub about one job's steps.
///
/// ASKED ONLY ABOUT A CHECK THAT DID NOT PASS, which is what keeps this free on
/// the ordinary push: a green commit makes no such call at all, the same rule the
/// annotations call already follows through `annotations_count`.
pub fn steps_query(job_id: u64) -> Vec<String> {
    vec![
        "api".to_string(),
        format!("repos/{{owner}}/{{repo}}/actions/jobs/{job_id}"),
    ]
}

/// One step of one job, as GitHub answers for it.
///
/// THE TIMES ARE A PLAIN `Option` AND THE CONCLUSION IS NOT, and the difference is
/// which fact this program depends on. A step's name and conclusion are what say
/// where a job stopped; the timestamps are what turn "cancelled" into "stalled for
/// forty-five minutes", and a step that has not started carries neither. So a
/// missing `conclusion` key is a refusal ([`present_but_nullable`]) and a missing
/// time is a line with less on it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct Step {
    pub name: String,
    pub number: u64,
    pub status: String,
    #[serde(deserialize_with = "present_but_nullable")]
    pub conclusion: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

/// The part of a job's answer this program reads.
#[derive(serde::Deserialize)]
struct Job {
    steps: Vec<Step>,
}

/// Every step of one job, read off GitHub's own answer.
pub fn steps_in(job_id: u64, body: &str) -> Result<Vec<Step>, String> {
    if body.trim().is_empty() {
        return Err(format!(
            "`gh` printed nothing at all about job {job_id}, which is not the answer a job with \
             no steps gives — that one arrives as a body carrying an empty list"
        ));
    }
    serde_json::from_str::<Job>(body)
        .map(|job| job.steps)
        .map_err(|why| {
            format!(
                "GitHub's answer about job {job_id}'s steps is not a shape this reporter can read \
                 ({why}) — it needs a `steps` list, and on every row a `name`, a `number`, a \
                 `status` and a `conclusion`. An answer without them is a read that failed, not a \
                 job that stopped nowhere"
            )
        })
}

/// Where a job stopped, and what that left unrun.
///
/// THE FIRST FAILING STEP AND NOT THE LAST, because the later ones are its
/// consequences: today's recording carries a `failure` on the artifact upload
/// eight steps after the one that actually stalled, and a reporter naming that one
/// would send a reader to the wrong repair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stoppage<'a> {
    /// The step whose conclusion ended the job.
    pub step: &'a Step,
    /// How many steps came after it, whatever they did.
    pub after: usize,
    /// How many of those never ran at all.
    pub never_ran: usize,
}

/// What GitHub calls a step that was never reached.
///
/// AND IT IS THE ONLY THING THAT SAYS SO, which R1275 measured the hard way. A
/// skipped step does not arrive with null stamps — GitHub gives it the START AND
/// END OF THE STOPPAGE, equal to each other — so a duration read off its stamps
/// is `0 s`, a well-formed measurement of work that never happened. On the
/// cancelled run either side of this repository's own rise, five steps read that
/// way, and the attribution block reported the job's `validate-workspace` step as
/// having got ninety-six seconds CHEAPER on the run where it did not run at all.
/// Every reader of this vocabulary must ask the conclusion; the stamps do not
/// carry the fact.
pub(crate) const NEVER_RAN: &str = "skipped";

/// Where this job stopped, if any step says so.
///
/// `None` IS A FACT WORTH PRINTING. A job can end in a way no step carries — a
/// runner that never started, a job cancelled before its first step — and a
/// reporter that simply said nothing there would be silent in exactly the case
/// where a reader has the least to go on.
pub fn stopped_at(steps: &[Step]) -> Option<Stoppage<'_>> {
    let at = steps
        .iter()
        .position(|step| ended_it(step.conclusion.as_deref()))?;
    let rest = &steps[at + 1..];
    Some(Stoppage {
        step: &steps[at],
        after: rest.len(),
        never_ran: rest
            .iter()
            .filter(|step| step.conclusion.as_deref() == Some(NEVER_RAN))
            .count(),
    })
}

/// What a stoppage reads as, on the one line printed under a failing check.
///
/// THE NAME, THE CLOCK, AND WHAT NEVER RAN — the three facts that separate "this
/// repository's change broke it" from "it never got as far as this repository's
/// change". R1236's own red is the worked example: a job cancelled at 45 minutes,
/// stopped at `Install protoc`, with every later step skipped, which is a stall in
/// the Ubuntu archive and not a line anybody here wrote.
pub fn stoppage_line(stoppage: &Stoppage) -> String {
    let when = match (
        stoppage.step.started_at.as_deref(),
        stoppage.step.completed_at.as_deref(),
    ) {
        (Some(from), Some(to)) => format!(", {from} → {to}"),
        _ => String::new(),
    };
    let rest = if stoppage.after == 0 {
        "and it was the last step".to_string()
    } else if stoppage.never_ran == 0 {
        format!(
            "and every one of the {} step(s) after it still ran",
            stoppage.after
        )
    } else {
        format!(
            "and {} of the {} step(s) after it never ran",
            stoppage.never_ran, stoppage.after
        )
    };
    format!(
        "stopped at step {} `{}` ({}{when}) {rest}",
        stoppage.step.number,
        stoppage.step.name,
        stoppage
            .step
            .conclusion
            .as_deref()
            .unwrap_or("no conclusion"),
    )
}

/// What this reporter says when a job's steps name no stopping point.
pub const STOPPED_NOWHERE: &str = "no step of that job carries the failure — GitHub reported it \
     on the job itself, which is what a runner that never started looks like";

/// The answer a `gh` that failed quietly gives, and a commit with no checks never
/// does.
///
/// SAID ONCE so the difference between the two silences is a line a reader can
/// find and an injection can take away.
const NOTHING_PRINTED: &str = "`gh` printed nothing at all, which is not the answer a commit \
     with no checks gives — that one arrives as a page saying so";

/// Why a page of the answer could not be read, in the reader's words and this
/// program's.
///
/// BOTH, AND THE READER'S VERBATIM: `missing field \`annotations_count\`` is what
/// says WHICH field GitHub stopped sending, and a message that summarised it would
/// leave the one fact a repair needs on the floor.
fn unreadable_page(page: usize, why: &serde_json::Error) -> String {
    format!(
        "page {page} of `gh`'s answer about this commit is not a shape this reporter can read \
         ({why}) — it needs GitHub's own `total_count` and `check_runs`, and in each row an \
         `id`, a `name`, a `head_sha`, a `details_url`, a `status`, a `conclusion` and an \
         `output` carrying `annotations_count`. An answer missing one of those is a read that \
         failed, not a commit nothing ran on"
    )
}

/// Every check run GitHub holds for one commit, read off its own answer.
///
/// `gh --paginate` prints one JSON object PER PAGE, concatenated, so the answer is
/// a stream rather than a document — the same shape `cache-budget` measured
/// against a recorded four-page body.
///
/// EMPTY IS TWO ANSWERS AND THEY ARE TOLD APART HERE. A commit nothing ran on says
/// so in a page; a read that failed prints nothing at all. The projection this
/// replaces gave both of them the same empty stdout, and the hook printed "no CI
/// runs recorded" over each.
pub fn checks_in(sha: &str, body: &str) -> Result<Vec<Check>, String> {
    let mut checks = Vec::new();
    let mut counted: Option<u64> = None;
    for (index, page) in serde_json::Deserializer::from_str(body)
        .into_iter::<CheckPage>()
        .enumerate()
    {
        let page = page.map_err(|why| unreadable_page(index + 1, &why))?;
        match counted {
            None => counted = Some(page.total_count),
            Some(first) if first != page.total_count => {
                return Err(format!(
                    "GitHub said this commit carries {first} check(s) on the first page of its \
                     answer and {} on page {} — the checks moved underneath this read, and rows \
                     taken from pages that disagree are a report nobody can defend",
                    page.total_count,
                    index + 1
                ));
            }
            Some(_) => {}
        }
        checks.extend(page.check_runs);
    }
    let counted = counted.ok_or_else(|| NOTHING_PRINTED.to_string())?;
    if checks.len() as u64 != counted {
        return Err(format!(
            "GitHub said this commit carries {counted} check(s) and {} arrived — a partial read \
             drops jobs, and a dropped job is one whose failure this reporter would not mention",
            checks.len()
        ));
    }
    // THE ANSWER MUST BE ABOUT THE COMMIT IT WAS ASKED FOR. Nothing else here
    // could tell: a body describing another commit parses perfectly, carries a
    // consistent count, and reports that commit's health as this one's.
    if let Some(other) = checks.iter().find(|check| check.head_sha != sha) {
        return Err(format!(
            "this reporter asked about {sha} and GitHub answered about {} (check `{}`) — a \
             well-formed answer to another question reports somebody else's commit as this one's",
            other.head_sha, other.name
        ));
    }
    Ok(checks)
}

/// Every annotation on one check run, read off GitHub's own answer.
///
/// A BARE ARRAY, not a counted page: this endpoint sends no `total_count`, which
/// is exactly why [`Output::annotations_count`] is carried here from the other
/// answer instead of being recomputed from these rows.
pub fn annotations_in(check_id: u64, body: &str) -> Result<Vec<Annotation>, String> {
    if body.trim().is_empty() {
        return Err(format!(
            "`gh` printed nothing at all about check {check_id}, which is not the answer a check \
             with no annotations gives — that one arrives as an empty list"
        ));
    }
    serde_json::from_str(body).map_err(|why| {
        format!(
            "GitHub's answer about check {check_id}'s annotations is not a shape this reporter \
             can read ({why}) — it needs an `annotation_level` and a `message` on every row, and \
             an answer without them is a read that failed, not a check with nothing to say"
        )
    })
}

/// What one annotation reads as on one line.
///
/// THE FIRST LINE AND AT MOST 160 CHARACTERS, because an annotation carries a
/// whole compiler diagnostic and this is printed at push time. Counted in
/// CHARACTERS and not bytes: a message that happens to hold a non-ASCII character
/// would panic a byte slice, and a reporter that dies on somebody's error text is
/// worse than one that prints nothing.
pub fn one_line(annotation: &Annotation) -> String {
    let head: String = annotation
        .message
        .lines()
        .next()
        .unwrap_or_default()
        .chars()
        .take(160)
        .collect();
    format!("{} {}", annotation.annotation_level, head)
}

/// One annotation, and the check that reported it.
///
/// THE PAIR AND NOT THE ANNOTATION ALONE (R1238). GitHub answers about
/// annotations one CHECK at a time, so the caller holds both — and this reporter
/// used to take only the second, merge every check's into one list, and print
/// the distinct set with nothing saying who said what. Measured on `cabcd5c`:
/// two checks failed, five distinct annotations came back flat, and finding out
/// which job carried `Process completed with exit code 127` took three `gh api`
/// calls made by hand. It was `validate` and only `validate`; the other two
/// lines were consequences both jobs emitted.
///
/// THAT IS THE SAME DEFECT R1236 REPAIRED ONE FIELD OVER. A conclusion without
/// the step that ended it, an annotation without the check that said it: both
/// are well-formed answers that cannot be attributed, and attribution is the
/// whole of what a person reads a red commit for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Said {
    /// The check that reported it, by the name the run shows.
    pub check: String,
    pub annotation: Annotation,
}

/// Seconds since the epoch for GitHub's own timestamp, or `None`.
///
/// NOT A GENERAL RFC 3339 READER, AND SAYING SO IS THE POINT. What arrives here
/// is one shape — `YYYY-MM-DDTHH:MM:SSZ`, always UTC, always twenty characters —
/// and a parser that accepted offsets, fractional seconds and a lower-case `t`
/// would be claiming to read a format this program has never been handed. The
/// narrower reader REFUSES anything else, which is the loud direction: a stamp it
/// cannot read stops a comparison rather than producing a plausible one.
///
/// THE CALENDAR IS THE CLOSED-FORM `days_from_civil`, exact for every date in
/// this range, and it is here rather than in a dependency because the whole of
/// what this needs is a DIFFERENCE between two stamps of one fixed shape. The
/// cases hold it against known epochs, a leap day, and both boundaries it could
/// plausibly get wrong.
#[must_use]
pub fn epoch_seconds(stamp: &str) -> Option<i64> {
    let bytes = stamp.as_bytes();
    if bytes.len() != 20 || bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    if bytes[10] != b'T' || bytes[13] != b':' || bytes[16] != b':' || bytes[19] != b'Z' {
        return None;
    }
    let number = |from: usize, to: usize| stamp.get(from..to)?.parse::<i64>().ok();
    let (year, month, day) = (number(0, 4)?, number(5, 7)?, number(8, 10)?);
    let (hour, minute, second) = (number(11, 13)?, number(14, 16)?, number(17, 19)?);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    if hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    let shifted = if month <= 2 { year - 1 } else { year };
    let era = shifted.div_euclid(400);
    let year_of_era = shifted - era * 400;
    let day_of_year = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = era * 146_097 + day_of_era - 719_468;
    Some(days * 86_400 + hour * 3_600 + minute * 60 + second)
}

/// How long a job took, or `None` when either end cannot be read.
///
/// A NEGATIVE DIFFERENCE IS A REFUSAL RATHER THAN A ZERO. GitHub stamps a skipped
/// job's completion BEFORE its start — the recording this crate keeps has one,
/// `13:40:41` beginning and `13:40:36` ending — and a duration clamped to zero
/// there is a number nobody wrote, sitting where a measurement is expected.
#[must_use]
pub fn seconds_between(started: &str, completed: &str) -> Option<u64> {
    let (from, to) = (epoch_seconds(started)?, epoch_seconds(completed)?);
    u64::try_from(to - from).ok()
}

/// How long one STEP of a job took, or `None` when it carries no readable pair of
/// stamps.
///
/// `None` IS THE ANSWER FOR A STEP THAT NEVER RAN, and it is not the same answer
/// as zero. A job that stopped leaves every later step in its answer with both
/// stamps null, and a duration of nought seconds sitting where a step was skipped
/// would let a comparison across two runs report that step as having got faster —
/// see [`history::Side`], which is where that distinction is kept rather than
/// flattened.
#[must_use]
pub fn step_seconds(step: &Step) -> Option<u64> {
    seconds_between(step.started_at.as_deref()?, step.completed_at.as_deref()?)
}

/// What one job took, against what it was allowed to take.
///
/// SERIALISED SINCE R1260, because this is exactly what a record of a push keeps:
/// the two raw numbers and the name they belong to, never the percentage between
/// them. One spelling of that division means a record cannot come to disagree
/// with the reporter that wrote it, and keeping the budget beside the duration is
/// what makes a raised `timeout-minutes` visible as a moved denominator rather
/// than as a job that got cheaper.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spent {
    /// The check row's name, which is the job's `name:` or its id.
    pub check: String,
    /// Seconds between the job's two stamps.
    pub took: u64,
    /// The declared budget, in minutes.
    pub budget_minutes: u64,
    /// GitHub's own word for how the job ended, verbatim.
    ///
    /// KEPT BECAUSE A DURATION IS NOT THE SAME FACT AS A COST (R1261). What a job
    /// took is true of every job that ran; what a job COSTS is true only of one
    /// that ran all of its steps, and the difference is invisible in a number.
    /// The level line wants every row — the job its budget ended is the one it
    /// most needs to name — and a trend wants only the comparable ones, so the
    /// record carries the word that tells them apart rather than a flag one of
    /// the two readers decided on.
    pub conclusion: String,
}

impl Spent {
    /// How much of the budget the job used, rounded down, as a whole percent.
    ///
    /// TOTAL ARITHMETIC, WHICH IT HAD NO NEED OF UNTIL R1260. While both numbers
    /// came from one `gh` answer in this process, `took * 100` was a duration
    /// GitHub had just stamped. They now also arrive off DISK, out of a record a
    /// person can edit — and `took * 100` overflows a `u64` for a duration nobody
    /// worked but somebody can type, which is a panic in a reporter that is not
    /// allowed to block a push.
    #[must_use]
    pub fn percent(&self) -> u64 {
        share_of(self.took, self.budget_minutes)
    }
}

/// What a duration is, as a share of a budget in minutes.
///
/// A FREE FUNCTION BECAUSE THE TREND HOLDS ONE JOB'S DURATIONS AGAINST A BUDGET
/// THAT IS NOT EACH ROW'S OWN (R1260). Comparing what a job cost in June against
/// what it costs now means holding both against ONE denominator — otherwise a
/// raised `timeout-minutes` reads as the job getting cheaper. That comparison and
/// [`Spent::percent`] must be the same arithmetic, so there is one of it.
///
/// TOTAL, over numbers that arrive off disk: see [`Spent::percent`]. Both
/// products are taken in `u128`, where neither can overflow, so nothing here
/// SATURATES either — a budget clamped to the largest representable number would
/// answer `100%` for a job that used none of it, which is a wrong number in the
/// direction that reads as alarming.
#[must_use]
pub fn share_of(took: u64, budget_minutes: u64) -> u64 {
    let allowed = u128::from(budget_minutes) * 60;
    if allowed == 0 {
        return 0;
    }
    u64::try_from(u128::from(took) * 100 / allowed).unwrap_or(u64::MAX)
}

/// Why one check's cost could not be held against a budget.
///
/// A TYPE RATHER THAN A SENTENCE, because one of these is NOT a shortfall: a job
/// that has not finished is a state of the world, and the ordinary commit a push
/// builds on has several. Printing one line each turns the ordinary case into six
/// lines of alarm, and printing nothing turns a block that shrank into a block
/// that was clean — so the kinds are told apart and the common one is COUNTED.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unmeasured {
    /// A check no job of this repository's workflows declares — another app's
    /// status, or a job that has been renamed out from under its history.
    NoSuchJob { check: String },
    /// A name more than one workflow declares, so a budget read by it would be
    /// somebody else's.
    Ambiguous { check: String, files: Vec<String> },
    /// A budget written as an expression: a bound, and not a number.
    BudgetIsAnExpression { check: String, written: String },
    /// The job has not finished. A state, not a defect.
    NotFinished { check: String },
    /// A later push ended the run this job was in, so the wall clock between its
    /// two stamps is not what the job cost. A state, not a defect.
    Retired { check: String },
    /// The job never ran, so its two identical stamps are not a duration at all.
    /// A state, not a defect — this repository skips a job whose inputs did not
    /// change on every green push.
    Skipped { check: String },
    /// Two stamps this reader cannot turn into a duration.
    Unreadable {
        check: String,
        started: String,
        completed: String,
    },
    /// A workflow file that could not be read at all.
    Workflow { why: String },
}

impl std::fmt::Display for Unmeasured {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoSuchJob { check } => write!(
                formatter,
                "`{check}` is a check no job of this repository's workflows declares, so \
                 there is no budget to hold it to"
            ),
            Self::Ambiguous { check, files } => write!(
                formatter,
                "`{check}` is declared by {} jobs across workflows ({}), so a budget read \
                 by name would be somebody else's",
                files.len(),
                files.join(", ")
            ),
            Self::BudgetIsAnExpression { check, written } => write!(
                formatter,
                "`{check}` declares `{written}`, which is a bound this reader cannot \
                 evaluate — it is a budget and it is not a number"
            ),
            Self::NotFinished { check } => {
                write!(formatter, "`{check}` has not finished")
            }
            Self::Retired { check } => write!(
                formatter,
                "`{check}` was ended by a LATER PUSH, so the time between its two \
                 stamps is how long it waited to be cancelled and not what it cost"
            ),
            Self::Skipped { check } => write!(
                formatter,
                "`{check}` was skipped, so its two identical stamps are the absence \
                 of a measurement and not a job that cost nothing"
            ),
            Self::Unreadable {
                check,
                started,
                completed,
            } => write!(
                formatter,
                "`{check}` is stamped {started} to {completed}, which this reader cannot \
                 read as a duration"
            ),
            Self::Workflow { why } => write!(formatter, "{why}"),
        }
    }
}

/// The share of its budget a job may use before this reporter says so.
///
/// A WARNING BAND AND NOT A LIMIT — the limit is the timeout, and it ends the job.
/// What this number is for is NOTICE: R1229 changed the work of a job's longest
/// step, left the budget alone, and the first run answered with a cancellation, so
/// what was missing was a round's warning rather than a stricter rule. Measured on
/// the last fully green run before this was written, the worst job in this
/// repository sat at 44% (`validate`, 40m24s of 90m), so seventy-five leaves room
/// for an ordinary bad day and still speaks a round before the job dies.
///
/// AND IT IS NOT DERIVED FROM THE RECORD [`history`] KEEPS, WHICH IS A DECISION
/// (R1260). A band read off this repository's own history — "warn above the
/// ninetieth percentile of what jobs here actually do" — rises with the creep it
/// exists to catch: every push that costs a little more raises the baseline the
/// band is measured from, so a job that doubles over two months is inside it on
/// every single push. This number is a share of the thing that ENDS the job, and
/// the record answers the other question — whether the cost is moving — with no
/// threshold at all.
pub const CREEPING: u64 = 75;

/// The job that used the largest share of its budget, of those that were measured.
///
/// ONE SPELLING, BECAUSE TWO READERS ASK IT. The report below names this job in
/// its first line, and [`history::kept_report`] follows THAT job through the
/// record — and a trend drawn about a different job than the level line named is
/// worse than no trend, because both sentences read as being about one thing.
#[must_use]
pub fn closest_to_budget(spent: &[Spent]) -> Option<&Spent> {
    spent.iter().max_by_key(|one| one.percent())
}

/// What each job took against what it declared, and what could not be compared.
///
/// AMBIGUITY IS A REFUSAL. `ci-plan`'s law makes a check name unique WITHIN a
/// workflow; ACROSS workflows two jobs may share one, and this reader is handed a
/// commit's checks with no workflow written on them. A name two workflows both
/// declare is named and skipped rather than joined to whichever came first —
/// which is the difference between "I cannot say" and a number about the wrong
/// job.
///
/// EVERY OTHER WAY A ROW CANNOT BE COMPARED IS ALSO NAMED, because a block that
/// silently shrinks is how a reader comes to believe every job was measured. A
/// check no job declares, a job still running, a budget written as an expression
/// and a stamp this cannot read are four different sentences.
///
/// AND A RUN A LATER PUSH RETIRED IS NOT A COST (R1260, paying for R1245). R1242
/// taught the census that a cancelled run says nothing about the commit it is on;
/// the block that reads what a job COST was written after it and never learned
/// the same fact. Measured on the record this repository kept in the round that
/// found it: commit `1ddeff31` reports NINE checks, every one of them `cancelled`
/// by the next push, every one of them stamped 11:39:12 to 12:37:16 — one wall
/// clock, shared by nine jobs that were mostly sitting in a queue. Against their
/// budgets that reads as `MSRV` at 193% of thirty minutes, a job that would have
/// been killed at thirty if it had ever been running. The number is not large
/// because anything is wrong; it is a duration of a different thing, and keeping
/// it would have written that into every trend for as long as the record lives.
#[must_use]
pub fn spent_against_budgets(
    checks: &[Check],
    budgets: &[(String, ci_plan::JobBudget)],
    retired: &BTreeSet<String>,
) -> (Vec<Spent>, Vec<Unmeasured>) {
    let mut spent = Vec::new();
    let mut unread = Vec::new();
    for check in checks {
        if retired.contains(&check.name) {
            unread.push(Unmeasured::Retired {
                check: check.name.clone(),
            });
            continue;
        }
        // BEFORE THE BUDGET IS EVEN LOOKED UP (R1261), because the question this
        // answers comes first: a job that did not run has nothing to hold against
        // anything, whichever workflow declares it and whatever it declares.
        if check.conclusion.as_deref() == Some(DID_NOT_RUN) {
            unread.push(Unmeasured::Skipped {
                check: check.name.clone(),
            });
            continue;
        }
        let declaring: Vec<&(String, ci_plan::JobBudget)> = budgets
            .iter()
            .filter(|(_, job)| job.check_name() == check.name)
            .collect();
        let job = match declaring.as_slice() {
            [] => {
                unread.push(Unmeasured::NoSuchJob {
                    check: check.name.clone(),
                });
                continue;
            }
            [one] => &one.1,
            many => {
                unread.push(Unmeasured::Ambiguous {
                    check: check.name.clone(),
                    files: many.iter().map(|(file, _)| file.clone()).collect(),
                });
                continue;
            }
        };
        let Some(budget_minutes) = job
            .timeout
            .as_deref()
            .and_then(|written| written.parse().ok())
        else {
            unread.push(Unmeasured::BudgetIsAnExpression {
                check: check.name.clone(),
                written: job
                    .timeout
                    .clone()
                    .unwrap_or_else(|| "no timeout-minutes".to_string()),
            });
            continue;
        };
        let (Some(started), Some(completed)) =
            (check.started_at.as_deref(), check.completed_at.as_deref())
        else {
            unread.push(Unmeasured::NotFinished {
                check: check.name.clone(),
            });
            continue;
        };
        let Some(took) = seconds_between(started, completed) else {
            unread.push(Unmeasured::Unreadable {
                check: check.name.clone(),
                started: started.to_string(),
                completed: completed.to_string(),
            });
            continue;
        };
        spent.push(Spent {
            check: check.name.clone(),
            took,
            budget_minutes,
            conclusion: outcome(check).to_string(),
        });
    }
    (spent, unread)
}

/// `40m24s`, as this reporter prints a duration.
pub(crate) fn clock(seconds: u64) -> String {
    format!("{}m{:02}s", seconds / 60, seconds % 60)
}

/// What this reporter says about what a commit's jobs cost.
///
/// THE WORST ONE IS PRINTED EVEN WHEN EVERYTHING IS FINE, which is the whole
/// difference between a number a reader watches and a number that appears once it
/// is too late. R1241 measured that shape one gate over: a count published only on
/// failure is a count nobody can see fall.
#[must_use]
pub fn budget_report(spent: &[Spent], unread: &[Unmeasured]) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(worst) = closest_to_budget(spent) {
        lines.push(format!(
            "of {} job(s) measured, the closest to its budget was `{}` — {} of {}m ({}%)",
            spent.len(),
            worst.check,
            clock(worst.took),
            worst.budget_minutes,
            worst.percent()
        ));
    }
    for one in spent.iter().filter(|one| one.percent() >= CREEPING) {
        lines.push(format!(
            "  `{}` took {} of the {}m it declares ({}%) — the budget is what ends the \
             job, and it is close",
            one.check,
            clock(one.took),
            one.budget_minutes,
            one.percent()
        ));
    }
    // THE UNFINISHED ARE COUNTED AND THE REST ARE NAMED. A commit a push builds on
    // routinely has jobs still going, and one line each for those turns the
    // ordinary state of the world into six lines of alarm — while dropping them
    // would leave a reader believing every job was measured.
    let unfinished = unread
        .iter()
        .filter(|why| matches!(why, Unmeasured::NotFinished { .. }))
        .count();
    if unfinished > 0 {
        lines.push(format!(
            "  {unfinished} job(s) have not finished, so what they took is not a fact \
             about this commit yet"
        ));
    }
    // COUNTED FOR THE SAME REASON, and the case is even more ordinary: GitHub's
    // concurrency group cancels a whole run at once, so a session that pushes
    // twice in an hour retires NINE checks in one go, and nine lines saying so is
    // a screen of alarm about the entirely normal.
    let retired = unread
        .iter()
        .filter(|why| matches!(why, Unmeasured::Retired { .. }))
        .count();
    if retired > 0 {
        lines.push(format!(
            "  {retired} job(s) were ended by a LATER PUSH, so the clock between their \
             stamps is how long they waited to be cancelled and not what they cost"
        ));
    }
    // AND COUNTED FOR THE THIRD TIME, for the most ordinary case of all (R1261):
    // this repository skips a job whose inputs did not change, so a green push
    // routinely has two of these and nothing at all is wrong.
    let skipped = unread
        .iter()
        .filter(|why| matches!(why, Unmeasured::Skipped { .. }))
        .count();
    if skipped > 0 {
        lines.push(format!(
            "  {skipped} job(s) were skipped, so their two identical stamps are the \
             absence of a measurement rather than a job that cost nothing"
        ));
    }
    // THE ORDINARY REASONS ARE NAMED, AND SO ARE THE REST (R1283). This was a
    // negated `matches!`, whose catch-all cannot be written out — so a NEW
    // reason a job goes unmeasured would fall into the half this loop PRINTS,
    // which is the safe direction, and a reader would still never have been
    // asked which half it belongs in. The direction is why this was not a defect
    // in the field; naming both halves is why the next reason is a decision.
    for why in unread.iter().filter(|why| match why {
        Unmeasured::NotFinished { .. }
        | Unmeasured::Retired { .. }
        | Unmeasured::Skipped { .. } => false,
        Unmeasured::NoSuchJob { .. }
        | Unmeasured::Ambiguous { .. }
        | Unmeasured::BudgetIsAnExpression { .. }
        | Unmeasured::Unreadable { .. }
        | Unmeasured::Workflow { .. } => true,
    }) {
        lines.push(format!("  NOT MEASURED {why}"));
    }
    lines
}

/// What a commit's checks add up to.
///
/// THREE ANSWERS AND NOT TWO. A commit whose checks have not finished is neither
/// red nor clear, and the projection this replaces could not say so: it wrote a
/// dash for a missing conclusion and then asked whether any line ENDED in one of
/// four words, so "still running" and "green" were one answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Nothing has run on this commit. From the per-commit endpoint this is a
    /// fact, where the run-list window it replaces could only ever mean "not in
    /// the last twenty runs".
    Nothing,
    /// At least one check concluded in a way [`FAILING`] names.
    Red,
    /// Nothing failed, and at least one check has not concluded.
    Pending,
    /// Every check concluded, and none of them failed.
    Clear,
}

/// What this commit's checks add up to.
pub fn verdict(checks: &[Check]) -> Verdict {
    if checks.is_empty() {
        return Verdict::Nothing;
    }
    if checks.iter().any(is_failing) {
        return Verdict::Red;
    }
    if checks.iter().any(|check| check.conclusion.is_none()) {
        return Verdict::Pending;
    }
    Verdict::Clear
}

/// Whether a conclusion is one of the words that means "did not pass".
///
/// ONE SPELLING FOR A CHECK AND FOR A STEP (R1236). The same four words decide
/// both questions, and the moment they were written twice the two could disagree —
/// a reporter that called a job red and then found no step in it that ended it,
/// which reads as GitHub's answer being inconsistent rather than as this file
/// being. The injection over this line reddens both laws at once, which is the
/// truth about them.
fn ended_it(conclusion: Option<&str>) -> bool {
    conclusion.is_some_and(|conclusion| FAILING.contains(&conclusion))
}

/// Whether one check concluded in a way that means the commit did not pass.
pub fn is_failing(check: &Check) -> bool {
    ended_it(check.conclusion.as_deref())
}

/// How a check reads in the census and in a row — its conclusion, or the fact
/// that it has none.
fn outcome(check: &Check) -> &str {
    check.conclusion.as_deref().unwrap_or("still running")
}

/// The first eight characters of a sha, as everything here prints it.
///
/// PUBLIC SINCE R1300, because the walk names commits the library never sees:
/// a second spelling in `main.rs` is how two parts of one report come to print
/// the same commit differently.
pub fn short(sha: &str) -> String {
    sha.chars().take(8).collect()
}

/// What this reporter says about a commit's checks, line by line.
///
/// A CENSUS AND THEN THE ROWS THAT ARE NOT `success`. Printing all nine rows on
/// every green push trains a reader to skip the block, and printing only the
/// failures says nothing about how much was looked at — so the counts name every
/// row and the lines name every row that is not routine. Nothing is dropped
/// silently, which is the rule the hook's annotation cap already followed.
pub fn report(
    sha: &str,
    checks: &[Check],
    superseded: &std::collections::BTreeSet<String>,
) -> Vec<String> {
    let short = short(sha);
    if checks.is_empty() {
        return vec![format!(
            "no CI checks on {short} — nothing has run on the commit this push builds on"
        )];
    }
    let mut census: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for check in checks {
        *census.entry(outcome(check)).or_default() += 1;
    }
    let tally: Vec<String> = census
        .iter()
        .map(|(what, count)| format!("{count} {what}"))
        .collect();
    let mut lines = vec![format!(
        "CI on {short} — {} check(s): {}",
        checks.len(),
        tally.join(", ")
    )];
    for check in checks {
        if check.conclusion.as_deref() != Some("success") {
            let why = if superseded.contains(&check.name) {
                " (a later push superseded this run)"
            } else {
                ""
            };
            lines.push(format!("  {} — {}{}", outcome(check), check.name, why));
        }
    }
    // A RUN A LATER PUSH RETIRED IS NOT A RED COMMIT (R1242), and the difference is
    // the whole of what a reader does next. GitHub's concurrency group stops the
    // run in flight the moment a newer one queues on the same ref, so a session
    // that pushes three rounds in ninety minutes cancels its own two earlier runs
    // — and every one of them reported `cancelled` and read as this commit's
    // failure. Measured on `74035d7`: three cancelled checks, one of them
    // twenty-seven minutes into `cargo test --workspace`, and the reason was the
    // NEXT push. What that commit needed was no verdict at all, and the later
    // commit's run — nine checks, all success — is where the answer was.
    let failing: Vec<&Check> = checks.iter().filter(|check| is_failing(check)).collect();
    let retired = failing
        .iter()
        .filter(|check| superseded.contains(&check.name))
        .count();
    if verdict(checks) == Verdict::Red {
        if retired == failing.len() {
            lines.push(format!(
                "^^ NO VERDICT on {short} — every check that did not pass was cancelled by a \
                 LATER PUSH on this ref, not by anything on this commit. Read the later \
                 commit's run; this one was never finished."
            ));
        } else {
            let also = if retired == 0 {
                String::new()
            } else {
                format!(
                    " ({retired} of the {} that did not pass were merely superseded by a later \
                     push)",
                    failing.len()
                )
            };
            lines.push(format!(
                "^^ the commit you are building on is RED. Fixing it is itself a push, so \
                 this does not stop one that says which red it is building on — see the \
                 refusal below.{also}"
            ));
        }
    }
    // AND "NOT FINISHED" IS PRINTED AS A REFUSAL RATHER THAN A ROW (R1297). The
    // tally above puts `3 still running` in the same sentence as `4 success` and
    // `1 failure`, so the one state that is NOT an answer reads as one of the
    // answers — and a reader who has seen the line believes the commit was
    // judged. It was the census this repository actually read: R1295 looked at a
    // run with six jobs undecided, took "not a verdict" for "nothing to act on",
    // and never asked again; the failure had already been sitting there for
    // eleven minutes when it pushed.
    if verdict(checks) == Verdict::Pending {
        let undecided = checks
            .iter()
            .filter(|check| check.conclusion.is_none())
            .count();
        lines.push(format!(
            "^^ NO VERDICT YET on {short} — {undecided} of {} check(s) have not concluded. \
             This commit is NOT judged, and a tally is not a verdict: whatever these say \
             is said after you have stopped looking. Read it again at the end of this \
             round.",
            checks.len()
        ));
    }
    lines
}

/// The variable a push uses to say which red it knows it is publishing over.
///
/// AN ACKNOWLEDGEMENT IS NOT A CONFIRMATION, and the difference is the whole of
/// why this holds a value rather than a yes. A retry, a `--force`, a second
/// invocation — every "are you sure" gate this repository could have built is
/// discharged by doing the same thing twice, which is precisely what somebody
/// who has not read the report does. Naming the checks cannot be done without
/// the report in hand.
pub const ACKNOWLEDGEMENT: &str = "MNEMOSYNE_PUSH_OVER_RED";

/// How that variable spells more than one check.
pub const ACKNOWLEDGEMENT_SEPARATOR: char = ',';

/// How it joins a job to the commit that job is red on.
///
/// THE COMMIT IS PART OF THE NAME SINCE R1301, and R1300 is why. The walk made
/// the reds a push must name span SEVERAL commits, and the acknowledgement was
/// a set of job NAMES — so one job red on two commits, for two different
/// reasons, was one name, and saying it once discharged both. That is right for
/// the fix-forward case and it is a narrowing everywhere else: there was no way
/// to say "I have read the one on this commit and not the one on that".
pub const ACKNOWLEDGEMENT_AT: char = '@';

/// How a push spells one outstanding red: the job, then where it is red.
///
/// THE SHORT SHA, because it is what every line of this reporter prints and an
/// acknowledgement a reader cannot copy off the refusal is one they will guess
/// at.
#[must_use]
pub fn spell(sha: &str, job: &str) -> String {
    format!("{job}{ACKNOWLEDGEMENT_AT}{}", short(sha))
}

/// One commit a push walked past, with what CI said about it.
///
/// THE WALK EXISTS BECAUSE ONE COMMIT WAS NOT ENOUGH (R1300). Gate 6 asked about
/// `origin/main` and nothing else, so a commit that was still PENDING when the
/// next push went over it was never asked about again: no later push's base is
/// that commit, its run goes red afterwards, and the red has no reader. R1297
/// closed the case where the base is ALREADY red and named this one as the hole
/// it left.
#[derive(Debug, Clone)]
pub struct Walked {
    pub sha: String,
    pub checks: Vec<Check>,
    /// The names R1242's subtraction already removed for this commit.
    pub superseded: BTreeSet<String>,
}

/// Whether this commit's verdict EXISTS — every check concluded, either way.
///
/// THE WALK'S STOPPING POINT, and it is deliberately not "Clear". The debt row
/// asked for a walk back until a commit whose verdict is Clear, and measuring
/// that refuted it: from `609101f` the nearest all-success commit is `cea5584`,
/// TEN commits back, because this repository pushes every twenty minutes and a
/// run takes thirty to sixty, so nearly every run is cancelled by the next push.
/// A rule with no reachable stopping point is not a bound.
///
/// AND JUDGED IS THE RIGHT LINE ANYWAY, because the hole is about verdicts that
/// did not exist yet. A commit whose checks have all concluded HAS a verdict —
/// whatever it says, somebody could read it — and the pending tail in front of
/// it is exactly the set nobody could. Measured at the same moment: depth 2.
#[must_use]
pub fn judged(checks: &[Check]) -> bool {
    matches!(verdict(checks), Verdict::Red | Verdict::Clear)
}

/// The reds a walk leaves outstanding, given newest-first.
///
/// A NEWER GREEN SIGHTING RETIRES AN OLDER RED, and it needs no memory of what
/// anybody acknowledged: if job `J` failed on an older commit and has since run
/// green, the tree moved past it. Without this the gate is a WALL rather than a
/// gate — measured on the real history, `separate in-repo workspaces` failed on
/// `c7540f1` and `0d1c333`, was fixed, and ran green on `1eab0c0`; a walk that
/// demanded every red it ever passed would demand those two for ever, and a
/// refusal nobody can discharge is one people learn to bypass.
///
/// THE GREENS OF A COMMIT DO NOT RETIRE ITS OWN REDS. They are recorded after
/// that commit's reds are read, because a job cannot be both on one commit and
/// the sighting that matters is a LATER one.
#[must_use]
pub fn outstanding_reds(walk: &[Walked]) -> Vec<(String, String)> {
    let mut green_since: BTreeSet<&str> = BTreeSet::new();
    let mut outstanding = Vec::new();
    for step in walk {
        for check in &step.checks {
            if is_failing(check)
                && !step.superseded.contains(&check.name)
                && !green_since.contains(check.name.as_str())
            {
                outstanding.push((step.sha.clone(), check.name.clone()));
            }
        }
        for check in &step.checks {
            if check.conclusion.as_deref() == Some(RAN_TO_COMPLETION) {
                green_since.insert(check.name.as_str());
            }
        }
    }
    outstanding
}

// `reds_to_name` STOOD HERE AND R1301 REMOVED IT. R1300 replaced it with
// `outstanding_reds`, which answers the same question over a WALK rather than
// one commit — for a single commit the two agree, because a green sighting
// needs something newer to be sighted at — and left the old one alive with a
// test as its only reader. A superseded path kept because something still calls
// it is the legacy carry this project's own `CLAUDE.md` forbids, and the test
// went with the function: audit history lives in the changelog, code lives in
// code. The subtraction it did (R1242: a run a later push cancelled is not this
// commit's failure) is inside `outstanding_reds`, where its caller reads it.

/// What an acknowledgement is worth against the reds it claims to name.
///
/// FIVE ANSWERS AND NOT TWO. "Named" and "not named" would fold three different
/// mistakes into one sentence — an absent variable, a variable naming the wrong
/// job, and a job whose name cannot be spelled in this variable at all — and the
/// reader's next move is different for each.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Acknowledgement {
    /// This commit is not red. Nothing to name, and nothing to refuse.
    NothingToName,
    /// Every red check is named and no other.
    Named,
    /// The commit is red and no acknowledgement was given.
    Absent { reds: Vec<String> },
    /// An acknowledgement was given and it is not this commit's red.
    Mismatched {
        missing: Vec<String>,
        invented: Vec<String>,
    },
    /// A red check's own name contains the separator, so no acknowledgement can
    /// spell it.
    ///
    /// A DEAD END THAT IS SAID OUT LOUD, never a pass. It cannot happen while
    /// this repository's tracked workflows name their jobs — `law.rs` holds that
    /// against the real files — and a gate that quietly let an unspellable red
    /// through would be exactly the exemption-shaped hole this one exists to
    /// close.
    Unspellable { reds: Vec<String> },
}

/// Whether a push has named the red it is publishing over.
///
/// EXACT SET EQUALITY, in both directions. A missing name is somebody who read
/// half the report; an invented one is somebody who read a different commit's,
/// and both are a reader who does not know what is broken.
/// `reds` is what the walk found: the commit each red is on, and the job.
pub fn acknowledgement(reds: &[(String, String)], given: Option<&str>) -> Acknowledgement {
    if reds.is_empty() {
        return Acknowledgement::NothingToName;
    }
    // BOTH SEPARATORS, because both are how this variable is read (R1301). A job
    // whose name holds either cannot be spelled unambiguously, and a parser that
    // can be made not to know is the exemption-shaped hole this gate closes.
    let unspellable: Vec<String> = reds
        .iter()
        .filter(|(_, job)| {
            job.contains(ACKNOWLEDGEMENT_SEPARATOR) || job.contains(ACKNOWLEDGEMENT_AT)
        })
        .map(|(_, job)| job.clone())
        .collect();
    if !unspellable.is_empty() {
        return Acknowledgement::Unspellable { reds: unspellable };
    }
    let spelled: Vec<String> = reds.iter().map(|(sha, job)| spell(sha, job)).collect();
    let Some(given) = given.map(str::trim).filter(|given| !given.is_empty()) else {
        return Acknowledgement::Absent { reds: spelled };
    };
    let named: BTreeSet<&str> = given
        .split(ACKNOWLEDGEMENT_SEPARATOR)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .collect();
    let wanted: BTreeSet<&str> = spelled.iter().map(String::as_str).collect();
    if named == wanted {
        return Acknowledgement::Named;
    }
    Acknowledgement::Mismatched {
        missing: wanted
            .difference(&named)
            .map(|name| (*name).to_string())
            .collect(),
        invented: named
            .difference(&wanted)
            .map(|name| (*name).to_string())
            .collect(),
    }
}

/// What the reporter says when a push may not go over this red unread.
///
/// EVERY LINE OR NONE. The refusal names the commit, the checks, and the exact
/// spelling that discharges it — because a gate whose discharge is a guess is a
/// gate people learn to route around, and routing around this one means the
/// `--no-verify` that no exemption in this repository covers.
pub fn refusal(sha: &str, verdict: &Acknowledgement) -> Vec<String> {
    let short = short(sha);
    match verdict {
        Acknowledgement::NothingToName | Acknowledgement::Named => Vec::new(),
        Acknowledgement::Absent { reds } => {
            let mut lines = vec![format!(
                "REFUSING this push — {short} is RED and nothing here says you have read \
                 it. This is not a block on fixing a red: name what you are building on \
                 and the push goes through."
            )];
            lines.push(format!(
                "  {ACKNOWLEDGEMENT}='{}' git push",
                reds.join(&ACKNOWLEDGEMENT_SEPARATOR.to_string())
            ));
            lines
        }
        Acknowledgement::Mismatched { missing, invented } => {
            let mut lines = vec![format!(
                "REFUSING this push — the acknowledgement does not name what is red on \
                 {short}."
            )];
            if !missing.is_empty() {
                lines.push(format!("  not named, and red: {}", missing.join(", ")));
            }
            if !invented.is_empty() {
                lines.push(format!(
                    "  named, and not red on this commit: {}",
                    invented.join(", ")
                ));
            }
            lines
        }
        Acknowledgement::Unspellable { reds } => vec![
            format!(
                "REFUSING this push — {short} is RED and a check's own name contains \
                 `{ACKNOWLEDGEMENT_SEPARATOR}`, which is how this acknowledgement \
                 separates them, so no spelling of it names this red."
            ),
            format!("  the red check(s): {}", reds.join(" / ")),
        ],
    }
}

/// What this reporter says about a commit's annotations, line by line.
///
/// BOTH NUMBERS, ALWAYS. The same annotation is emitted once per job, so the
/// distinct set is what a reader wants — and a cap that does not say what it
/// dropped reads as "that was all of them". `declared` is GitHub's own count from
/// the check rows; `read` is what actually came back.
///
/// AND EVERY DISTINCT LINE NAMES THE CHECKS THAT SAID IT (R1238). Deduplicating
/// across jobs is what makes this readable and it is also what threw the one fact
/// a reader needs on the floor: `Process completed with exit code 127` beside
/// `The operation was canceled` says nothing about whether one job carried both
/// or two jobs carried one each. Attribution is not a nicety here — it is the
/// difference between "our change did this" and "this happened to us".
pub fn annotation_report(sha: &str, declared: u64, read: &[Said]) -> Vec<String> {
    let short = short(sha);
    if declared == 0 {
        return vec![format!("no CI annotations on {short}")];
    }
    // ONE ENTRY PER DISTINCT LINE, holding the checks that reported it. A
    // `BTreeMap` keyed on the rendered line is the dedup this already did; the
    // value is what it used to discard.
    let mut distinct: std::collections::BTreeMap<String, BTreeSet<String>> =
        std::collections::BTreeMap::new();
    for said in read {
        distinct
            .entry(one_line(&said.annotation))
            .or_default()
            .insert(said.check.clone());
    }
    if distinct.is_empty() {
        return vec![format!(
            "NOTE {short} reports {declared} annotation(s), none readable"
        )];
    }
    let mut lines = vec![format!(
        "CI annotations on {short} — {} distinct of {declared} reported:",
        distinct.len()
    )];
    for (note, checks) in distinct.iter().take(SHOWN) {
        lines.push(format!("  {note}"));
        lines.push(format!("      from {}", said_by(checks)));
    }
    if distinct.len() > SHOWN {
        lines.push(format!(
            "   (+{} distinct not shown)",
            distinct.len() - SHOWN
        ));
    }
    lines
}

/// The checks that reported one line, as the line under it reads.
///
/// NAMED AND NOT COUNTED, up to a cap that says what it left out — the same rule
/// the distinct cap above follows. A job name in this repository is a sentence,
/// so a commit where every job says the same thing would otherwise be one
/// unreadable line; a commit where TWO jobs do is the case this exists for, and
/// there the names are the whole answer.
fn said_by(checks: &BTreeSet<String>) -> String {
    let named: Vec<&str> = checks.iter().take(NAMED).map(String::as_str).collect();
    let rest = checks.len().saturating_sub(named.len());
    let tail = if rest == 0 {
        String::new()
    } else {
        format!(" (+{rest} more)")
    };
    format!("{} check(s): {}{tail}", checks.len(), named.join(", "))
}

/// How many distinct annotations are printed before the rest are counted instead.
const SHOWN: usize = 10;

/// How many checks are named under one annotation before the rest are counted.
const NAMED: usize = 4;
