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

/// What GitHub calls a conclusion that means the commit did not pass.
///
/// EXACTLY THE SET R890 CHOSE, kept whole rather than narrowed while the reading
/// around it changed: `failure`, `cancelled`, `timed_out` and `startup_failure`
/// are what the hook's own `grep -E` matched. `skipped` and `neutral` are NOT
/// here and that is deliberate — this repository's workflow skips a job whose
/// inputs did not change on every green push, and a reporter that called that red
/// would be ignored within a day.
pub const FAILING: [&str; 4] = ["failure", "cancelled", "timed_out", "startup_failure"];

/// One check run on a commit, as GitHub answers for it.
///
/// NAMED FIELDS AND NOT A PROJECTION, which is what `Deserialize` here buys: a
/// field GitHub renames stops this program rather than emptying it, and
/// `tests/github.rs` holds that rename against a RECORDED REAL body so the day it
/// happens is a red test. Every other field on the row — `app`, `check_suite`,
/// `html_url`, `started_at`, `completed_at`, `pull_requests` — is ignored on
/// purpose: a reader that refused unknown fields would go red the day GitHub adds
/// one, which is a gate failing for somebody else's work.
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
const NEVER_RAN: &str = "skipped";

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
fn short(sha: &str) -> String {
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
                "^^ the commit you are building on is RED. Not blocking (fixing it is \
                 itself a push), but do not push past it blind.{also}"
            ));
        }
    }
    lines
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
