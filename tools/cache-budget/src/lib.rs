//! Every cache this repository's CI declares is one it gets to KEEP.
//!
//! A cache that is declared, saved, and deleted before it can be restored is
//! worse than no cache at all: the job pays minutes to tar and upload it and then
//! rebuilds from nothing anyway. It is also invisible. CI is green, the job is
//! merely slow, and slow has no annotation. `unrun-tests` ran 27, 27 and 28
//! minutes on three consecutive pushes while a sibling job beside it ran 14m,
//! 14m and 1m36s, and nothing in this repository could say why.
//!
//! WHY THE OBVIOUS GATE IS VACUOUS, which is the whole reason this file is shaped
//! the way it is. "Sum the caches and compare against the limit" cannot fail:
//! GitHub deletes caches, least recently accessed first, precisely until the
//! total is under the limit. A repository asking for three times its budget
//! therefore measures the SAME total as one asking for half of it, and the gate
//! would print a clean number forever while every cache in it was being deleted
//! before its next use.
//!
//! So the law is about ABSENCE, which eviction cannot hide:
//!
//! - the DEMAND fits the budget, where demand prices each absent cache from the
//!   largest present one holding a SUBSET of its paths. A cache holding a whole
//!   `target` costs at least what a cache holding that `target` costs, and six of
//!   them are six of those — the arithmetic no round did while adding the fourth,
//!   fifth and sixth;
//! - the archives NOTHING DECLARES are inside that same total. A key outliving
//!   the job that wrote it keeps eating the budget, and it can only be found by
//!   asking both sides — but the harm it does is BYTES, so it is counted rather
//!   than categorically refused. R1123 measured what the categorical form cost:
//!   every rename orphans its own archive for the seven days it takes to age
//!   out, so the gate refused a repository for making a repair, and R1122 had to
//!   pin a real defect because renaming was the only way to close it;
//! - no cache's `restore-keys` reach an archive holding paths it never declared.
//!   An archive unpacks AS IT WAS STORED, so `path:` says what a cache SAVES and
//!   cannot stop somebody else's build directory landing in a job that asked for
//!   a registry;
//! - two jobs writing ONE key agree on what that key holds, or the cache's
//!   contents depend on which job saved first;
//! - and a gate that could price nothing REFUSES rather than passing, because a
//!   gate that looked at nothing and a gate that found nothing wrong print the
//!   same silence.
//!
//! Both sides are asked of a machine. DECLARED comes from `ci-plan`, this
//! repository's one reader of what its CI says, so this gate cannot drift from
//! the gates asking the same files what CI RUNS. HELD comes from the GitHub API,
//! which is the only thing that knows what a cache actually costs.

use std::collections::{BTreeMap, BTreeSet};

use ci_plan::CacheDeclaration;

/// The cache storage GitHub gives one repository, in bytes.
///
/// A NAMED CONSTANT AND AN ARGUMENT BOTH: the number is GitHub's, documented at
/// 10 GB per repository, and a gate that hardcoded it into its verdict could not
/// be tested at the boundary without a repository sitting on it. Every decision
/// below takes the limit as a parameter; this is only the default the binary
/// passes.
pub const DEFAULT_LIMIT_BYTES: u64 = 10 * 1000 * 1000 * 1000;

/// A job that declares a key, and the workflow it declares it in.
///
/// THE TWO HALVES KEPT APART rather than formatted into one string, because one
/// of them is a JOIN KEY: what a job's cache actually restored is measured by
/// `tools/restored` and filed under the job id, and a reader that had to take
/// that id back out of `mnemosyne-validate.yml \`validate\`` would be parsing a
/// rendering. The rendering is `Display`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Owner {
    /// The workflow file it is written in.
    pub source: String,
    /// The job id — the same spelling `needs:` uses, and the name a restore
    /// record is filed under.
    pub job: String,
}

impl std::fmt::Display for Owner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} `{}`", self.source, self.job)
    }
}

/// Every owner, as one phrase.
fn named(owners: &[Owner]) -> Vec<String> {
    owners.iter().map(Owner::to_string).collect()
}

/// One cache GitHub actually holds.
///
/// THESE THREE NAMES ARE GITHUB'S WIRE NAMES and not this gate's choice of them,
/// which is what `Deserialize` here means: the answer is read into the type
/// directly, so a field renamed for tidiness renames the thing this gate asks
/// the API for. `tests/github.rs` reads a RECORDED REAL body, so that rename is
/// a red test rather than a gate that quietly reads nothing. Every other field
/// GitHub sends — `id`, `ref`, `version`, `last_accessed_at` — is ignored on
/// purpose: a reader that refused unknown fields would go red the day GitHub
/// adds one, which is a gate failing for somebody else's work.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct Held {
    pub key: String,
    pub size_in_bytes: u64,
    /// When GitHub created it, verbatim — `2026-08-08T17:13:25.229538000Z`.
    ///
    /// Compared as a STRING, which is sound only because the API returns one
    /// fixed-width UTC spelling for every entry, and is the reason this is not
    /// parsed into a date: a dependency on a date library to order two strings
    /// GitHub has already zero-padded would be the more fragile of the two.
    pub created_at: String,
}

/// One page of GitHub's answer to the cache endpoint.
///
/// `total_count` IS THE ONLY THING IN THE ANSWER THAT SAYS WHETHER THE ROWS ARE
/// ALL OF THEM, and it is why this gate reads pages rather than rows. Every
/// other failure of this read is loud — a row missing a size does not parse, a
/// body that never arrived is empty — but a read that stops early is silent and
/// arrives as a smaller repository. Under-counting the storage is a PASS on
/// exactly the repository this gate exists to refuse, so the count travels with
/// the rows and is checked against them.
#[derive(serde::Deserialize)]
struct Page {
    total_count: u64,
    actions_caches: Vec<Held>,
}

/// What this gate asks GitHub, as the words `gh` is handed.
///
/// IN THE LIBRARY SO THE QUESTION HAS A READER. `--paginate` is load-bearing and
/// invisible: this repository holds more caches than one page carries, and a
/// `gh` without it answers with a body that is valid, well-shaped and short —
/// the exact case `tests/github.rs` builds out of the first page of a recorded
/// real answer. `{owner}` and `{repo}` are `gh`'s own placeholders, resolved
/// from the checkout, so this gate never names the repository it is judging.
pub fn caches_query() -> Vec<String> {
    ["api", "--paginate", "repos/{owner}/{repo}/actions/caches"]
        .map(str::to_string)
        .to_vec()
}

/// What this gate asks GitHub about the run it is inside.
///
/// THE RUN ID IS PART OF THE QUESTION, which is the whole of what makes the
/// answer this run's: `runs/{id}` is a different resource from `runs`, and an
/// answer about the wrong run is a start time that excuses every cache built
/// after it.
pub fn run_query(run_id: &str) -> Vec<String> {
    vec![
        "api".to_string(),
        format!("repos/{{owner}}/{{repo}}/actions/runs/{run_id}"),
    ]
}

/// One run, as GitHub answers for it — the half of the body this gate reads.
///
/// ONE FIELD OF A HUNDRED, and named rather than projected for the reason
/// [`caches_in`] gives: a `--jq` expression naming a field the API no longer
/// sends prints an empty line, and an empty start time excuses nothing or
/// everything depending on which way the comparison falls.
#[derive(serde::Deserialize)]
struct RunPage {
    run_started_at: String,
}

/// Why GitHub's answer about a run could not be read, in the reader's words and
/// this gate's — the shape [`unreadable_page`] takes for the other endpoint.
fn unreadable_run(run_id: &str, why: &serde_json::Error) -> String {
    format!(
        "GitHub's answer about run {run_id} is not a shape this gate can read ({why}) — it \
         needs `run_started_at`, and an answer without one is a read that failed, not a run \
         that began at no particular time"
    )
}

/// When the run this gate is inside began, read off GitHub's own answer.
///
/// NOT THE SAME SPELLING AS A CACHE'S `created_at` — this endpoint answers to
/// the second (`2026-08-10T09:54:23Z`) and the cache endpoint to the fraction
/// (`2026-08-10T10:14:11.221132000Z`). Both recordings are in `tests/`, and the
/// comparison between them is made to the second for exactly that reason.
///
/// AN EMPTY STAMP IS A REFUSAL AND NOT A ZERO TIME. Every cache in the
/// repository was created after the beginning of time, so a blank read here
/// would report every one of them as an archive this run built from nothing.
pub fn run_started_in(run_id: &str, body: &str) -> Result<String, String> {
    let page: RunPage = serde_json::from_str(body).map_err(|why| unreadable_run(run_id, &why))?;
    let started_at = page.run_started_at.trim();
    if started_at.is_empty() {
        return Err(format!(
            "run {run_id} reports no start time, so which caches predate it is unknown \
             rather than none — every cache in the repository is newer than nothing"
        ));
    }
    Ok(started_at.to_string())
}

/// How many of a workflow's runs this gate asks GitHub for in one page.
///
/// A HUNDRED, WHICH IS THE ENDPOINT'S MAXIMUM, AND THE DEPTH IS NO LONGER A
/// GUESS. This was FIVE until R1312, on the reasoning that the rows above the
/// run being looked for are a sibling run of the same push, a re-run, or a short
/// red streak. That is a claim about a repository's CADENCE, and this repository
/// falsifies it: its concurrency group cancels the run in flight when the next
/// push arrives, a cancelled run usually does not reach its post steps, and on
/// 2026-09-02 FOURTEEN consecutive runs wrote no archive at all — the caches API
/// dates nothing between `03:02:19Z` and `09:08:38Z`. The run that last wrote
/// `Linux-cargo-validate-` was fifteen runs and fifteen commits back, and GitHub
/// calls THAT one `cancelled` too: cancelled is not the same as having written
/// nothing, which is why the floor below is read off the archives and never off
/// a conclusion. The gate looked at five, found none, narrowed to the push
/// range, and reported eight archives as unexplained rebuilds — while the
/// workflow file that every one of those keys names in its own `hashFiles` had
/// moved three commits earlier. A false red, and the reason it was false is
/// this number.
///
/// WHAT STOPS THE WALK IS THE EVIDENCE AND NOT THIS NUMBER. A candidate's jobs
/// are asked about only until the walk has gone past the moment this key's own
/// archive was written — [`archive_floor`], read off the caches API — so a page
/// this wide costs one more row in one answer, not a hundred calls.
pub const RUNS_PER_PAGE: usize = 100;

/// What this gate asks GitHub about the runs of ONE workflow.
///
/// THE WORKFLOW IS PART OF THE QUESTION, and it is the whole of what makes the
/// answer the right one: a key is asked for only when the workflow DECLARING it
/// runs, so the interval over which a change to its inputs could have gone
/// unobserved is that workflow's own, and no other's. Addressed by file name
/// because that is the identity the endpoint takes, derived here from the path
/// `ci-plan` reads so that no caller spells it a second time.
///
/// BRANCH-SCOPED WHEN THERE IS A BRANCH TO NAME, because GitHub's cache storage
/// is: an archive saved on one ref is not offered to another, so runs of another
/// branch are not observations of this key. On a `pull_request` the runner names
/// no branch this endpoint accepts — `GITHUB_REF_NAME` is `123/merge` — which is
/// why the caller passes the BASE branch there and why this takes an `Option`
/// rather than assuming one exists.
pub fn workflow_runs_query(workflow: &str, branch: Option<&str>) -> Vec<String> {
    let file = workflow.rsplit('/').next().unwrap_or(workflow);
    let mut path =
        format!("repos/{{owner}}/{{repo}}/actions/workflows/{file}/runs?per_page={RUNS_PER_PAGE}");
    if let Some(branch) = branch.map(str::trim).filter(|branch| !branch.is_empty()) {
        path.push_str("&branch=");
        path.push_str(branch);
    }
    vec!["api".to_string(), path]
}

/// One page of GitHub's answer about a workflow's runs.
///
/// `total_count` IS NOT READ HERE, and that is a difference from [`Page`] rather
/// than an oversight: this question asks for the NEWEST page on purpose, so the
/// rows are a deliberate sample and the count is the workflow's whole history —
/// five hundred of them. Checking one against the other would refuse every
/// repository that has run its CI more than five times.
#[derive(serde::Deserialize)]
struct RunsPage {
    workflow_runs: Vec<RunRow>,
}

/// One run in that page — the three fields this gate reads of the sixty GitHub
/// sends.
///
/// `conclusion` IS NOT AMONG THEM SINCE R1207, and its absence is the whole of
/// this round. A run's conclusion is not evidence about an archive: what writes
/// one is the cache step's own `Post …`, which lives in a job and has a
/// conclusion of its own. A run can fail with that step long since finished —
/// measured on this repository's own history at 122 saves inside 19 red runs of
/// the newest hundred — and a run can conclude success with the declaring job
/// skipped. The id is here for the same reason: it is how the jobs endpoint,
/// which does carry that evidence, is addressed.
#[derive(serde::Deserialize)]
struct RunRow {
    id: u64,
    head_sha: String,
    run_started_at: String,
}

/// A run that came before this one — a candidate for the last moment one of its
/// workflow's archives was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PriorRun {
    /// GitHub's id for it, which is what the jobs endpoint is addressed by.
    pub id: u64,
    /// The commit it ran at, in full.
    pub sha: String,
    /// When it started, in the runs endpoint's own spelling.
    pub started_at: String,
}

/// Every run of one workflow that could have left the archive this run failed to
/// hit, newest first — read off GitHub's own answer.
///
/// TWO CONDITIONS, AND BOTH ARE CASES THIS REPOSITORY REALLY PRODUCED:
///
///   - started strictly before this run. Two workflows triggered by one push
///     start in the same second — measured: `mnemosyne-validate` and
///     `evidence-replay` both report `2026-08-13T11:24:58Z` for the push that
///     made this reader necessary.
///   - at a commit other than the one being judged. That one is load-bearing
///     rather than defensive: the run of THIS commit is the run whose miss is
///     being judged, and an interval of `HEAD..HEAD` answers "nothing moved" for
///     every key in the repository — the narrow-range failure R1095 already paid
///     for once.
///
/// THE THIRD CONDITION USED TO BE `conclusion == "success"` AND IT WAS THE WRONG
/// QUESTION. A run's conclusion is a fact about every job in it; what leaves an
/// archive is one step of one job. R1207 measured the disagreement over this
/// repository's newest hundred runs and nine declarations: 134 of the 900 bounds
/// were OLDER than the truth, and in 16 more this gate had no bound at all where
/// the step had one. Older means WIDER, and a wider interval excuses a miss whose
/// inputs moved before the archive was actually last written — leniency, in a
/// gate whose whole job is to notice.
///
/// So the conclusion is gone from here and the evidence is asked of
/// [`saved_the_archive`], one run at a time, by the caller that can pay for the
/// call. Returning CANDIDATES rather than one answer is what lets that caller
/// stop at the first run that really saved.
///
/// NEWEST FIRST BY THE STAMP AND NOT BY POSITION. GitHub does send them newest
/// first, but that is an ordering this gate would be trusting silently; the
/// stamps are one fixed-width UTC spelling from a single endpoint, so sorting
/// them is an answer this program can defend.
///
/// An EMPTY LIST is a reading and not a failure: a workflow whose newest page
/// holds no qualifying run is one this gate cannot bound an interval with, and
/// the caller narrows to the push range and prints why. An unreadable answer is
/// the other thing entirely, and it is an `Err`.
pub fn candidate_runs(
    workflow: &str,
    body: &str,
    started_before: &str,
    not_at: &str,
) -> Result<Vec<PriorRun>, String> {
    if body.trim().is_empty() {
        return Err(format!(
            "`gh` printed nothing at all about the runs of `{workflow}`, which is not the \
             answer a workflow that has never run gives — that one arrives as a page \
             carrying no rows"
        ));
    }
    let page: RunsPage = serde_json::from_str(body).map_err(|why| {
        format!(
            "GitHub's answer about the runs of `{workflow}` is not a shape this gate can read \
             ({why}) — it needs `workflow_runs`, and in each row a `head_sha`, a \
             `run_started_at` and a `conclusion`. An answer missing one of those is a read \
             that failed, not a workflow nothing has ever run"
        )
    })?;
    let mut candidates: Vec<PriorRun> = Vec::new();
    for row in page.workflow_runs {
        let started_at = row.run_started_at.trim();
        let sha = row.head_sha.trim();
        if sha.is_empty() || started_at.is_empty() {
            return Err(format!(
                "a run of `{workflow}` arrived with no commit or no start time, so which \
                 interval it bounds is unknown rather than empty — an interval starting at \
                 nothing excuses every cache in the repository"
            ));
        }
        if to_the_second(started_at) >= to_the_second(started_before) || sha == not_at {
            continue;
        }
        candidates.push(PriorRun {
            id: row.id,
            sha: sha.to_string(),
            started_at: started_at.to_string(),
        });
    }
    candidates.sort_by(|left, right| {
        to_the_second(&right.started_at)
            .cmp(to_the_second(&left.started_at))
            .then_with(|| right.id.cmp(&left.id))
    });
    Ok(candidates)
}

/// What this gate asks GitHub about the jobs of ONE run.
///
/// A PAGE SIZE THAT CANNOT TRUNCATE THIS REPOSITORY'S ANSWER: `mnemosyne-validate`
/// has nine jobs and GitHub's maximum here is a hundred. A truncated page would
/// answer "that step did not save" about a job it never sent, which is the
/// silent-lenient direction this whole round exists to remove — so the reader
/// below refuses a page that says there are more jobs than it was handed.
pub fn run_jobs_query(run_id: u64) -> Vec<String> {
    vec![
        "api".to_string(),
        format!("repos/{{owner}}/{{repo}}/actions/runs/{run_id}/jobs?per_page=100"),
    ]
}

/// One page of GitHub's answer about a run's jobs.
#[derive(serde::Deserialize)]
struct JobsPage {
    total_count: usize,
    jobs: Vec<JobRow>,
}

/// One job in that page — its steps, which is where the archive's fate is.
///
/// `steps` IS OPTIONAL IN THE ANSWER AND REQUIRED IN A COMPLETED JOB, and those
/// are two different facts that one `#[serde(default)]` would fold together. A
/// job GitHub has not started sends no step list and legitimately says nothing
/// about any archive; a COMPLETED job that sends none is an answer this gate
/// cannot use, and defaulting it to empty would read as "that step did not save"
/// — the silent-lenient direction, arriving through a renamed field.
#[derive(serde::Deserialize)]
struct JobRow {
    name: String,
    status: String,
    steps: Option<Vec<StepRow>>,
}

/// One step of a job.
///
/// `conclusion` IS AN `Option` BECAUSE GITHUB SENDS `null` FOR A STEP THAT HAS
/// NOT FINISHED, and that is exactly the step this gate must not read as a save:
/// a run still in flight has an archive it may or may not end up writing.
#[derive(serde::Deserialize)]
struct StepRow {
    name: String,
    conclusion: Option<String>,
}

/// The prefix GitHub gives the step that WRITES an `actions/cache` archive.
///
/// The action declares a post-step, and GitHub names it after the step that
/// declared it. That naming is the join, which is why `ci-plan` refuses a cache
/// step with no name of its own.
pub const SAVE_STEP_PREFIX: &str = "Post ";

/// Whether one run wrote the archive of ONE declared cache, read off GitHub's
/// answer about that run's jobs.
///
/// THE EVIDENCE IS THE STEP'S OWN CONCLUSION and nothing above it. The job may
/// have failed after saving, the run may have failed because a different job
/// did, and neither changes what is in the cache — R1207 measured 122 such saves
/// inside the 19 red runs of the newest hundred.
///
/// A TRUNCATED PAGE IS A REFUSAL. "I was not sent that job" and "that job did not
/// save" are different answers, and folding the first into the second is the
/// shape this repository has paid for repeatedly: a check that could not look
/// reporting the answer of one that looked and found nothing.
pub fn saved_the_archive(run: &PriorRun, step: &str, body: &str) -> Result<bool, String> {
    let id = run.id;
    if body.trim().is_empty() {
        return Err(format!(
            "`gh` printed nothing at all about the jobs of run {id}, which is not the answer \
             a run with no jobs gives — that one arrives as a page carrying no rows"
        ));
    }
    let page: JobsPage = serde_json::from_str(body).map_err(|why| {
        format!(
            "GitHub's answer about the jobs of run {id} is not a shape this gate can read \
             ({why}) — it needs `total_count` and `jobs`, and in each job a `steps` list \
             whose entries carry a `name` and a `conclusion`"
        )
    })?;
    if page.total_count > page.jobs.len() {
        return Err(format!(
            "GitHub says run {id} has {} job(s) and sent {} — an answer this gate cannot \
             complete says nothing about whether `{step}` saved, and reading it as `no` \
             would widen every interval it bounds",
            page.total_count,
            page.jobs.len()
        ));
    }
    let wanted = format!("{SAVE_STEP_PREFIX}{step}");
    let mut saved = false;
    for job in &page.jobs {
        let Some(steps) = job.steps.as_ref() else {
            if job.status == "completed" {
                return Err(format!(
                    "GitHub sent no step list for job `{}` of run {id}, which it says is \
                     completed — so whether `{step}` saved is unknown rather than no, and \
                     reading it as no would widen the interval this run bounds",
                    job.name
                ));
            }
            continue;
        };
        saved |= steps.iter().any(|step| {
            step.name.trim() == wanted && step.conclusion.as_deref() == Some("success")
        });
    }
    Ok(saved)
}

/// Why a page of the answer could not be read, in the reader's words and this
/// gate's.
///
/// BOTH, and the reader's verbatim: `missing field \`size_in_bytes\`` is what
/// says WHICH field GitHub stopped sending, and a message that summarised it
/// would leave the one fact a repair needs on the floor.
fn unreadable_page(page: usize, why: &serde_json::Error) -> String {
    format!(
        "page {page} of `gh`'s answer is not a shape this gate can read ({why}) — it needs \
         GitHub's own `total_count` and `actions_caches`, and in each row a `key`, a \
         `size_in_bytes` and a `created_at`. An answer missing one of those is a read that \
         failed, not a repository holding nothing"
    )
}

/// The answer a `gh` that failed quietly gives, and a repository holding no
/// caches never does.
///
/// SAID ONCE so that the difference between the two silences is a line a reader
/// can find, and an injection can take away.
const NOTHING_PRINTED: &str = "`gh` printed nothing at all, which is not the answer a \
     repository holding no caches gives — that one arrives as a page saying so";

/// Every cache GitHub holds for this repository, read off its own answer.
///
/// A READING AND NOT A PROJECTION. This used to be a `--jq` expression flattening
/// the answer into tab-separated rows before this program saw it, which put the
/// one seam between this gate and GitHub in a language no test here can execute:
/// `jq` is not on this machine and `gh` needs a network and a credential, so
/// nothing could ask whether the expression still named fields the API still
/// sends. It also threw away `total_count` — see [`Page`].
///
/// `gh --paginate` prints one JSON object PER PAGE, concatenated, so the answer
/// is a stream rather than a document; that shape is measured rather than
/// assumed, and `tests/actions-caches.paginated.json` is what four pages of it
/// actually look like.
///
/// EMPTY IS TWO ANSWERS AND THEY ARE TOLD APART HERE. A repository holding no
/// caches says so in a page; a read that failed prints nothing at all. The
/// projection this replaces gave both of them the same empty stdout.
pub fn caches_in(body: &str) -> Result<Vec<Held>, String> {
    let mut held = Vec::new();
    let mut counted: Option<u64> = None;
    for (index, page) in serde_json::Deserializer::from_str(body)
        .into_iter::<Page>()
        .enumerate()
    {
        let page = page.map_err(|why| unreadable_page(index + 1, &why))?;
        match counted {
            None => counted = Some(page.total_count),
            Some(first) if first != page.total_count => {
                return Err(format!(
                    "GitHub said this repository holds {first} caches on the first page of \
                     its answer and {} on page {} — the storage moved underneath this read, \
                     and rows summed across pages that disagree are a total nobody can defend",
                    page.total_count,
                    index + 1
                ));
            }
            Some(_) => {}
        }
        held.extend(page.actions_caches);
    }
    let counted = counted.ok_or_else(|| NOTHING_PRINTED.to_string())?;
    if held.len() as u64 != counted {
        return Err(format!(
            "GitHub said this repository holds {counted} caches and {} arrived — a partial \
             read prices the budget short, and short is the direction that PASSES the \
             repository this gate exists to refuse",
            held.len()
        ));
    }
    Ok(held)
}

/// One cache KEY, and what became of it.
///
/// The row is a key rather than a declaration because THE KEY IS THE CACHE: two
/// jobs naming one key share one entry in GitHub's storage and cost the budget
/// once. Counting per declaration would price a shared cache twice and report a
/// repository as over budget for having read a cache from two places, which is
/// the thing sharing a key is FOR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// The prefix a restore matches on — `Linux-cargo-unrun-`.
    pub prefix: String,
    /// Every job that declares this key.
    pub owners: Vec<Owner>,
    /// What this key holds. The union when its declarations disagree, which is
    /// the loud direction and is refused separately.
    pub paths: BTreeSet<String>,
    /// The globs this key hashes, from the first declaration of it — what would
    /// legitimately have forced it to be rebuilt.
    pub hashed: Vec<String>,
    /// The NEWEST cache held under this prefix, if any.
    pub held: Option<Held>,
    /// The generations under this prefix that the newest one replaced, oldest
    /// last. They are real bytes and GitHub really is holding them, so they are
    /// PRINTED — but they are not what this gate judges, because no edit to a
    /// workflow can prevent them: a dependency bump changes every key in the
    /// repository at once and the previous generation stays until it ages out.
    /// Counting them would refuse a tree for having had its lockfile touched.
    pub superseded: Vec<Held>,
    /// What an absent cache is reckoned to cost, and where the number came from.
    /// `None` when nothing comparable has ever been seen.
    pub estimate: Option<Estimate>,
    /// Whether this key declares `restore-keys` — whether an archive under it
    /// from an earlier run can reach a job whose primary key missed.
    ///
    /// READ FROM THE DECLARATION, and it is the half Round 1160 did not carry.
    /// That round took the fallback off the build-directory cache, because a
    /// fallback onto a build tree has no bound and that archive was restoring 37
    /// GB, and it split the law REQUIRING a fallback into two halves by
    /// `build.target-dir`. The judgement below was not split with it: it read an
    /// archive predating a run as one `restore-keys` had to serve, which is true
    /// only where a fallback is written. For a cache without one, a missed key
    /// and an empty tree are the SAME EVENT — and reporting that as a lost
    /// archive is a gate telling a repository its own decision is a defect.
    pub falls_back: bool,
}

impl Row {
    /// What this key costs the budget: what it holds, or what it would.
    pub fn bytes(&self) -> Option<u64> {
        match &self.held {
            Some(held) => Some(held.size_in_bytes),
            None => self.estimate.as_ref().map(|estimate| estimate.bytes),
        }
    }

    /// Which restore one of this key's owners performs — the join key the
    /// records are filed under.
    ///
    /// ASKED OF THE ROW rather than assembled by every caller, because the pair
    /// is what a record is filed by and a caller holding a `Row` and an `Owner`
    /// already has both halves. Six lookups used to take only the job, which was
    /// the whole identity while a job had one cache.
    pub fn restore_by(&self, owner: &Owner) -> restored::Restore {
        restored::Restore {
            job: owner.job.clone(),
            cache: self.prefix.clone(),
        }
    }

    /// The newest archive under this prefix that already existed when the run
    /// began — what `restore-keys` had to fall back to.
    ///
    /// EVERY GENERATION, NOT JUST THE NEWEST, because the newest is routinely the
    /// one THIS run saved: a job that missed its key writes a new archive in its
    /// post step, and by the time this gate asks the API that archive is the
    /// head of the prefix. The one that was available to the job is whichever
    /// predates the run's start, which is the same comparison `Recreated` makes
    /// and the same reason it is made to the second.
    /// THROUGH [`newest_predating`] SINCE R1312, which is the same rule
    /// [`archive_floor`] bounds the run walk with. They are one question asked
    /// at two moments — before there are rows, and after — and two spellings of
    /// it could disagree about which archive a run could have restored.
    pub fn restorable_when(&self, started_at: &str) -> Option<&Held> {
        newest_predating(self.held.iter().chain(self.superseded.iter()), started_at)
    }
}

/// What an absent cache would take, and the cache that number was read off.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Estimate {
    pub bytes: u64,
    /// The key of the present cache this was read from — never a number with no
    /// origin, because one nobody can trace is one nobody can argue with.
    pub from: String,
    /// The absent cache's paths that its source does NOT hold, so this is a
    /// lower bound and not a reading.
    ///
    /// It is the difference between the two verdicts this gate can reach. A
    /// lower bound is enough to REFUSE — the real demand is at least this and
    /// this is already over — but it is never enough to PASS: the registry-only
    /// caches in this repository are measured at 0.10 GB and the ones holding a
    /// build tree at 3 GB and up, so a missing `target` cache priced off a
    /// registry cache reads as a thirtieth of what it costs, which is a green
    /// verdict on precisely the failure this gate was built for.
    pub unpriced_paths: BTreeSet<String>,
}

/// Why a repository's caching cannot work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The caches this repository declares, plus the archives nothing declares,
    /// cannot all exist at once.
    OverBudget {
        demand: u64,
        /// What the archives no declaration matches weigh — see
        /// [`Report::held_by_nothing`].
        orphaned: u64,
        limit: u64,
        absent: Vec<String>,
    },
    /// One key, two jobs, two different answers about what it holds.
    Divergent { prefix: String, owners: Vec<Owner> },
    /// One cache's `restore-keys` fall back onto another cache's archives, and
    /// that archive holds paths the first never asked for.
    ///
    /// AN ARCHIVE UNPACKS AS IT WAS STORED. `path:` is what a cache SAVES and has
    /// no say in what a restore of somebody else's archive puts on disk, and
    /// `restore-keys` is a prefix match over the WHOLE repository's storage
    /// rather than over one job's. So a job whose primary key misses can land a
    /// tree nothing measured and nothing asked for — in this repository, an 8.90
    /// GB build directory arriving in a job that declares two cargo-home trees.
    ///
    /// SUBSET AND NOT EQUALITY, because the harmless case is the common one and
    /// is what the mechanism is FOR: five of this repository's keys nest under
    /// another whose paths they hold exactly, and falling back onto one of their
    /// generations gets the outer job the paths it asked for.
    FallbackReachesAnotherCache {
        prefix: String,
        other: String,
        /// What the other cache holds that this one never asked for.
        holds: Vec<String>,
    },
    /// Saved BY THIS RUN, which means the primary key did not hit exactly, with
    /// nothing this key hashes having moved to explain it.
    ///
    /// WHAT THIS DOES NOT SAY, AND SAID WRONGLY UNTIL R1101 MEASURED IT: that the
    /// job paid for a cold build. `actions/cache` saves whenever the PRIMARY key
    /// missed, and `restore-keys` is a separate mechanism that can have served an
    /// earlier generation in the same job — so this signal alone cannot tell a
    /// cold build from a warm-but-stale one. Round 1099 read a warm run as cold
    /// and deleted a cache that was saving ten minutes; the sentence this refusal
    /// printed would have agreed with it. What each owner actually started from
    /// is measured now, and travels here so the message can stop guessing.
    ///
    /// AND A GENERATION HAD TO BE THERE TO RESTORE — the same condition
    /// [`Refusal::RestoredNothingWithAGenerationHeld`] states outright, missing
    /// here until R1135 turned main red with it. The harm this names is a cache
    /// that was AVAILABLE AND WASTED; where nothing was available, a cold build
    /// was unavoidable and this refusal has nothing to offer, because the range
    /// it diffs for an excuse is the current push and the key may have been
    /// invalidated in an earlier one.
    Recreated {
        prefix: String,
        /// Whether this key declares `restore-keys`, which decides what an
        /// UNMEASURED miss can be said about: with a fallback the job may still
        /// have been warm, and without one the miss is itself the cold build.
        falls_back: bool,
        owners: Vec<Owner>,
        hashed: Vec<String>,
        /// What each owner's disk said, where a record for it was read.
        started: Vec<(String, restored::Warmth)>,
    },
    /// A job restored NOTHING, and a generation it could have restored was
    /// already there when the run started.
    ///
    /// THE COST THE BUDGET EXISTS TO BUY OFF, and until R1101 nothing in this
    /// repository could see it. `cache-hit` is false for a prefix match, the
    /// cache API cannot say what a job's disk received, and a legitimately
    /// invalidated key was EXCUSED here — so a job that fell all the way through
    /// to an empty tree while a restorable archive sat in storage was reported as
    /// the honest price of a dependency bump.
    ///
    /// It is also what a wrong wiring looks like: the two measuring steps must
    /// bracket the `actions/cache` step, and one that does not brackets nothing
    /// and reports every job as having restored nothing. Both readings are worth
    /// stopping a run for.
    RestoredNothingWithAGenerationHeld {
        job: String,
        prefix: String,
        /// The newest archive under that prefix that predates this run.
        generation: Held,
    },
    /// The gate could not reach, or could not price, enough to have a verdict.
    /// Distinct from a pass.
    Unreached(String),
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refusal::OverBudget {
                demand,
                orphaned,
                limit,
                absent,
            } => write!(
                f,
                "the caches this repository declares come to {}{} against a {} \
                 budget, so GitHub deletes them, least recently used first, until \
                 they fit — {} of them are absent right now ({}). Every job \
                 restoring one of those rebuilds from nothing on every run, and \
                 the only symptom is a green job that takes half an hour",
                gigabytes(*demand),
                if *orphaned == 0 {
                    String::new()
                } else {
                    format!(
                        ", plus {} held under keys no workflow declares",
                        gigabytes(*orphaned)
                    )
                },
                gigabytes(*limit),
                absent.len(),
                absent.join(", ")
            ),
            Refusal::Divergent { prefix, owners } => write!(
                f,
                "`{prefix}` is declared with different paths by {} — one key is \
                 one cache, so what it holds would depend on which job saved it \
                 first, and every job restoring the other spelling gets a tree it \
                 did not ask for",
                named(owners).join(" and ")
            ),
            Refusal::FallbackReachesAnotherCache {
                prefix,
                other,
                holds,
            } => write!(
                f,
                "`{prefix}` falls back onto every archive saved under `{other}`, \
                 and those hold {} — which `{prefix}` never asked for. An archive \
                 unpacks as it was STORED, so `path:` cannot stop it landing: a \
                 job whose primary key misses gets a tree nothing measured, \
                 nothing declared and nothing can price. Give one of the two a \
                 key the other's prefix does not reach",
                holds.join(", ")
            ),
            Refusal::Recreated {
                prefix,
                falls_back,
                owners,
                hashed,
                started,
            } => write!(
                f,
                "`{prefix}` was SAVED BY THIS RUN, so the primary key {} asked \
                 for did not hit, and {}. What that cost is a separate question \
                 and this is the answer to it: {}",
                named(owners).join(" and "),
                if hashed.is_empty() {
                    "this key hashes nothing, so nothing could have invalidated it".to_string()
                } else {
                    format!(
                        "nothing matching {} changed in this commit",
                        hashed.join(", ")
                    )
                },
                if started.is_empty() && !falls_back {
                    // NO FALLBACK MAKES THE MISS THE ANSWER. The sentence below
                    // is about a cache that can be served by an earlier
                    // generation; where none is declared, the primary key is the
                    // only thing GitHub tries, so a miss IS the cold and there is
                    // nothing left unmeasured about it.
                    "this key declares no `restore-keys`, so the primary key is \
                     the only thing GitHub tries and this miss IS the cold build"
                        .to_string()
                } else if started.is_empty() {
                    "NOT MEASURED on this run — a missed key is not a cold job, \
                     because `restore-keys` can serve an earlier generation, and \
                     no restore record was read here to say which happened"
                        .to_string()
                } else {
                    started
                        .iter()
                        .map(|(job, warmth)| format!("`{job}` {}", warmth.why()))
                        .collect::<Vec<_>>()
                        .join("; ")
                }
            ),
            Refusal::RestoredNothingWithAGenerationHeld {
                job,
                prefix,
                generation,
            } => write!(
                f,
                "job `{job}` began with an EMPTY tree — not one byte arrived under \
                 the paths `{prefix}` holds — while `{}` ({}, created {}) was \
                 already in storage for `restore-keys` to fall back to. Either \
                 that archive was never offered, or the two measuring steps do \
                 not bracket the cache step and this run measured nothing at all",
                generation.key,
                gigabytes(generation.size_in_bytes),
                generation.created_at
            ),
            Refusal::Unreached(why) => write!(f, "this gate reached nothing it could judge: {why}"),
        }
    }
}

/// An ISO-8601 UTC stamp cut to whole seconds — the widest prefix two GitHub
/// endpoints spell the same way.
///
/// `2026-08-08T22:17:13Z` from the runs endpoint and
/// `2026-08-08T17:13:25.229538000Z` from the caches endpoint agree on the first
/// nineteen characters and disagree immediately after, so that prefix is the only
/// part of them that can be compared as text. Cutting rather than parsing keeps
/// this program without a notion of time: the two stamps are GitHub's, and the
/// only thing being asked of them is which came first.
///
/// A tie — a cache created within the same second the run started — reads as
/// created BEFORE it, which is the lenient direction and the correct one: no job
/// finishes and saves a cache in its run's opening second.
fn to_the_second(stamp: &str) -> &str {
    const SECONDS: usize = "2026-08-08T22:17:13".len();
    stamp.get(..SECONDS).unwrap_or(stamp)
}

/// Bytes as the unit a budget is argued in.
fn gigabytes(bytes: u64) -> String {
    format!("{:.2} GB", bytes as f64 / 1e9)
}

/// The run being judged, when there is one.
///
/// WITHOUT IT THE GATE STILL ANSWERS THE BUDGET QUESTION AND SAYS SO. Run on a
/// developer's machine there is no run to be inside, and a gate that invented one
/// would judge every cache in the repository as freshly built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    /// The workflow file this run is of — `.github/workflows/mnemosyne-validate.yml`.
    ///
    /// WHICH RECORDS COULD POSSIBLY BE HERE. A restore record is an artifact and
    /// an artifact belongs to A RUN: the download step in this gate's job
    /// collects the artifacts of the run it is itself inside, and a run's jobs
    /// are exactly one workflow's jobs. So a cache declared in another workflow
    /// has an owner whose record this gate can never be handed, and until R1107
    /// the report printed that as `did not say what it started from` — a
    /// sentence that reads as the job's shortcoming and is in fact this gate's
    /// horizon. It is carried on [`Run`] because it is a property of the run
    /// being judged and is unknowable without one.
    pub workflow: String,
    /// When it started, verbatim — `2026-08-08T22:17:13Z`.
    ///
    /// NOT THE SAME SPELLING AS A CACHE'S `created_at`, which is why the two are
    /// compared through [`to_the_second`] rather than directly. The runs endpoint
    /// gives whole seconds and the caches endpoint gives nanoseconds, and a plain
    /// string comparison of `…13Z` against `…13.229538000Z` orders them by the
    /// byte after the seconds — `.` is below `Z`, so it silently decides ties in
    /// one direction for a reason that has nothing to do with time. Both come
    /// from GitHub's clock; only their precision differs.
    pub started_at: String,
    /// One entry per key: the interval its hashed inputs were asked about, and
    /// what the answer was — so that a cache this run had to build is excused
    /// rather than refused. Computed by asking git with the globs the key itself
    /// names, because a second implementation of GitHub's glob matching is a
    /// second answer.
    ///
    /// THE QUESTION AND THE ANSWER IN ONE DATUM, which is the shape R1178 paid
    /// for. This was two fields — a set of prefixes that had moved, and ONE range
    /// they were all asked over — and the second of those was a single answer to
    /// a question that is per key: a key is asked for only when the workflow
    /// DECLARING it runs, and this repository has a path-filtered workflow that
    /// does not run on every push. Its cache key's lockfile moved in two commits
    /// that workflow never saw, so the miss was legitimate and the gate, asking
    /// only about the commits THIS push carried, reported it as a defect and
    /// turned main red on run 31695396997. A range shared by every key cannot
    /// hold that difference, and a reader cannot tell which question a number
    /// answered unless the two travel together.
    pub asked: Vec<Asked>,
}

impl Run {
    /// Did this key's hashed inputs move over the interval it was asked about —
    /// and was there an interval to ask over at all?
    ///
    /// A KEY NOBODY ASKED ABOUT DID NOT MOVE, which is the same reading the set
    /// this replaced gave: a key hashing nothing has nothing that could have
    /// invalidated it, and the report says as much in its own words.
    ///
    /// `None` IS THE ANSWER R1312 ADDED, AND IT IS NOT `Some(false)`. "Nothing
    /// matching these globs moved over the interval this key's archive could
    /// have seen" and "that interval was never bounded" are different facts, and
    /// this returned the first for both — which is how a substituted, narrower
    /// interval turned `main` red on a rebuild that a commit just outside it
    /// fully explained.
    pub fn movement(&self, prefix: &str) -> Option<bool> {
        let mine = || self.asked.iter().filter(|asked| asked.prefix == prefix);
        // AN ANSWER THAT SAYS `MOVED` SETTLES IT, which is what the `any` this
        // replaced meant: an excuse found is an excuse, whatever a second
        // reading of the same prefix could not bound.
        if mine().any(|asked| asked.moved == Some(true)) {
            return Some(true);
        }
        if mine().any(|asked| asked.moved.is_none()) {
            return None;
        }
        Some(false)
    }

    /// The interval this key could not be judged over, when there was one.
    ///
    /// SO THE REFUSAL CAN SAY WHAT WAS NOT REACHED. A gate that cannot judge and
    /// a gate that judged and found nothing print the same silence otherwise,
    /// and the whole of R1312 is that those two were being printed as one.
    pub fn unbounded(&self, prefix: &str) -> Option<&Window> {
        self.asked
            .iter()
            .find(|asked| asked.prefix == prefix && asked.moved.is_none())
            .map(|asked| &asked.over)
    }
}

/// One key's question: the interval its hashed inputs were asked about, and the
/// answer git gave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asked {
    /// The key prefix — `Linux-cargo-replay-`.
    pub prefix: String,
    /// The interval, and where it came from.
    pub over: Window,
    /// Whether anything matching the key's own globs moved over it — `None` when
    /// there was no interval to put to git, which is not the same answer as
    /// nothing having moved.
    pub moved: Option<bool>,
}

/// The interval a key's "did its hashed inputs move" question is asked over.
///
/// NOT THE PUSH, IN THE CASE THAT MATTERS. A push range is the right interval
/// only for a key whose workflow runs on every push; for one declared in a
/// path-filtered workflow the key is asked for on some pushes and not others, and
/// between two of its runs the lockfiles it hashes can move any number of times
/// with nothing to absorb the change. The interval that means something is
/// therefore the workflow's OWN: since it last ran, which is the last moment
/// anything asked for that key at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Window {
    /// Since the run that last WROTE this key's archive, at another commit — the
    /// last moment its inputs were observed.
    ///
    /// R1207 MOVED THIS FROM THE RUN TO THE STEP. It used to mean "since the
    /// declaring workflow last CONCLUDED success", which is a fact about every
    /// job in that run and not about this archive; the two disagreed on 134 of
    /// 900 measured bounds, always in the direction that starts the interval too
    /// early and excuses too much.
    SinceThatArchiveWasSaved {
        /// The workflow file, as `ci-plan` spells it.
        workflow: String,
        /// The cache step whose `Post …` wrote it, as the workflow names it.
        step: String,
        /// The commit that run was of.
        sha: String,
        /// When it started, in the runs endpoint's spelling.
        at: String,
    },
    /// The push range, with the reason the declaring workflow's own last run
    /// could not bound the interval.
    ///
    /// A FALLBACK THAT SAYS SO. It is the honest interval for a workflow this
    /// gate cannot see a previous run of — a new one, a checkout too shallow to
    /// hold the commit, a `pull_request` whose base gave no answer — and it is
    /// narrower than the truth, so the reason travels with it rather than being
    /// left as a silently different question.
    ThisPush {
        /// Where the push started, and why, from [`range_start`].
        start: RangeStart,
        /// Why the workflow's own last run was not used.
        why: String,
    },
    /// NO INTERVAL AT ALL: this key HAS an archive an earlier run wrote, and the
    /// run that wrote it was not reached.
    ///
    /// THE VARIANT R1312 ADDED, AND ITS ABSENCE IS THE WHOLE OF THAT ROUND. The
    /// two above are intervals; a question this gate could not bound is not a
    /// third interval and above all not a SHORTER one. Substituting the push
    /// range here answers "nothing matching those globs moved" for a key whose
    /// globs moved outside it — and that answer is a `Recreated` refusal, so the
    /// gate refuses a repository doing exactly what it was asked to do.
    ///
    /// IT CARRIES THE EVIDENCE THAT THE INTERVAL EXISTS. The archive named here
    /// is the generation this key's `restore-keys` fell back to: something wrote
    /// it, at a moment this walk did not reach, and that moment is the start of
    /// the interval nobody bounded. Where NO archive predates the run there is
    /// no such moment and no verdict to reach either, and that case stays
    /// [`Window::ThisPush`] rather than becoming a refusal.
    Unbounded {
        /// The archive whose writing bounds the interval — `Linux-cargo-validate-4495440…`.
        generation: String,
        /// When GitHub created it, verbatim.
        created_at: String,
        /// How far the walk got, and why it stopped short.
        why: String,
    },
}

impl Window {
    /// The revision to diff from — `None` when there is no interval to diff.
    ///
    /// AN `Option` SINCE R1312. It returned a revision unconditionally, which
    /// meant every caller received an interval whether or not one had been
    /// found, and the one that had not was silently the narrowest.
    pub fn rev(&self) -> Option<&str> {
        match self {
            Window::SinceThatArchiveWasSaved { sha, .. } => Some(sha),
            Window::ThisPush { start, .. } => Some(start.rev()),
            Window::Unbounded { .. } => None,
        }
    }

    /// One line naming the interval and where it came from.
    pub fn why(&self) -> String {
        match self {
            Window::SinceThatArchiveWasSaved {
                workflow,
                step,
                sha,
                at,
            } => format!(
                "over {}..HEAD, since `{step}` in {workflow} last wrote its archive at another \
                 commit ({at})",
                &sha[..7.min(sha.len())]
            ),
            Window::ThisPush { start, why } => format!("{} — {why}", start.why()),
            Window::Unbounded {
                generation,
                created_at,
                why,
            } => format!(
                "over NO INTERVAL — `{generation}` was written {created_at} and the run \
                 that wrote it was not reached, so what moved since then is unknown \
                 rather than nothing: {why}"
            ),
        }
    }
}

/// Whether a workflow's own last run could be found, or why not.
///
/// TWO ANSWERS AND NOT AN `Option`, because the second one is a sentence the
/// report prints: "the interval was narrowed" and "the interval was narrowed
/// BECAUSE" are what tell a reader whether a green verdict was earned over the
/// interval that means something or over the one that was left.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowSource {
    /// The run that bounds the interval.
    Ran(PriorRun),
    /// Why no run of that workflow could bound it.
    Unavailable(String),
}

/// Ask, for every key that hashes anything, whether its inputs moved over the
/// interval that key was last asked over.
///
/// PURE, AND BOTH SIDES ARE CLOSURES: which run bounds a workflow's interval is
/// GitHub's answer and whether a glob moved is git's, so neither is this
/// function's to invent — and with both injected, the DECISION they feed can be
/// driven by a suite with no network and no repository. That is the half R1178
/// found unreadable: this reasoning lived in `main.rs`, where nothing could ask
/// it anything, and it was wrong in a way that only a red `main` reported.
///
/// ONE QUESTION PER DECLARATION SINCE R1207, not per workflow. Two keys declared
/// in one file used to share an interval, because the bound was the workflow's
/// last successful RUN; the bound is now the last run that wrote THAT key's
/// archive, and two keys in one file are routinely written at different commits —
/// a job can be skipped, or fail before its post step, while its sibling saves.
/// The answers are still held as they arrive, keyed by the pair, so a repeated
/// question costs nothing.
///
/// FIRST DECLARATION OF A PREFIX WINS, which is the same choice [`Row`] makes for
/// `hashed`: one key is one cache, and a second declaration of it naming
/// different globs is a divergence refused by name elsewhere rather than a second
/// interval judged here.
pub fn windows_asked(
    declared: &[CacheDeclaration],
    push: &RangeStart,
    floor_of: impl Fn(&str) -> Option<Held>,
    mut last_save_of: impl FnMut(&str, &str, Option<&Held>) -> Result<WindowSource, String>,
    mut moved_since: impl FnMut(&str, &[String]) -> Result<bool, String>,
) -> Result<Vec<Asked>, String> {
    let mut out: Vec<Asked> = Vec::new();
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut source_of: BTreeMap<(&str, &str), WindowSource> = BTreeMap::new();
    for declaration in declared {
        if declaration.hashed.is_empty() || !seen.insert(declaration.prefix.as_str()) {
            continue;
        }
        // WHAT THE INTERVAL WOULD START AT, ASKED BEFORE THE WALK RATHER THAN
        // AFTER IT. It decides two things at once: how deep the walk has to go
        // before it can honestly stop, and — when the walk comes back
        // empty-handed — whether there was an interval to miss at all.
        let floor = floor_of(&declaration.prefix);
        let at = (declaration.source.as_str(), declaration.step.as_str());
        let source = match source_of.get(&at) {
            Some(source) => source,
            None => {
                let source = last_save_of(&declaration.source, &declaration.step, floor.as_ref())?;
                source_of.entry(at).or_insert(source)
            }
        };
        let over = match source {
            WindowSource::Ran(prior) => Window::SinceThatArchiveWasSaved {
                workflow: declaration.source.clone(),
                step: declaration.step.clone(),
                sha: prior.sha.clone(),
                at: prior.started_at.clone(),
            },
            // AND HERE IS WHERE R1312 SPLIT ONE ANSWER INTO TWO. A key with an
            // earlier generation has a moment its archive was written; failing
            // to reach that moment leaves the question UNBOUNDED, and the push
            // range is not a smaller version of it. A key with no earlier
            // generation has no such moment, nothing to fall back on, and no
            // `Recreated` verdict to reach — for that one the push range is
            // still the honest narrow answer it always was.
            WindowSource::Unavailable(why) => match &floor {
                Some(generation) => Window::Unbounded {
                    generation: generation.key.clone(),
                    created_at: generation.created_at.clone(),
                    why: why.clone(),
                },
                None => Window::ThisPush {
                    start: push.clone(),
                    why: why.clone(),
                },
            },
        };
        let moved = match over.rev() {
            Some(rev) => Some(moved_since(rev, &declaration.hashed)?),
            None => None,
        };
        out.push(Asked {
            prefix: declaration.prefix.clone(),
            over,
            moved,
        });
    }
    Ok(out)
}

/// The archive this key's `restore-keys` could have fallen back to — the moment
/// its archive was last written before this run.
///
/// THE EVIDENCE THAT BOUNDS THE WALK, and it is already in this gate's hands: the
/// caches API says when every generation under a prefix was created, so how far
/// back a run history has to be read is a MEASUREMENT rather than a constant
/// somebody picked. R1312 exists because it was a constant.
///
/// THE SAME POPULATION AND THE SAME COMPARISON AS [`Row::restorable_when`],
/// through the same resolver, because they are the same question asked at two
/// moments: this one before there are rows to ask, that one after. Two spellings
/// of it could disagree, and the disagreement would be a gate that bounds an
/// interval against one archive and judges the rebuild against another.
pub fn archive_floor<'a>(
    prefix: &str,
    declared: &[CacheDeclaration],
    held: &'a [Held],
    started_at: &str,
) -> Option<&'a Held> {
    let prefixes: Vec<String> = declared
        .iter()
        .map(|declaration| declaration.prefix.clone())
        .collect();
    newest_predating(
        held.iter()
            .filter(|cache| owner_of(&prefixes, &cache.key).is_some_and(|owner| owner == prefix)),
        started_at,
    )
}

/// The newest of these archives that already existed at `started_at`.
///
/// ONE RESOLVER FOR "WHAT COULD THIS RUN HAVE RESTORED", called with two
/// populations. The tie goes to the archive, for the reason [`to_the_second`]
/// gives: no job finishes and saves in its run's opening second.
fn newest_predating<'a>(
    generations: impl Iterator<Item = &'a Held>,
    started_at: &str,
) -> Option<&'a Held> {
    generations
        .filter(|generation| to_the_second(&generation.created_at) <= to_the_second(started_at))
        .max_by(|left, right| left.created_at.cmp(&right.created_at))
}

/// Has the walk gone back past the moment this key's archive was written?
///
/// THE RUN THAT WROTE AN ARCHIVE STARTED BEFORE IT, because the save is a POST
/// step. So a candidate that started at or before that moment is the last one
/// worth buying an answer about: nothing older can be the run being looked for.
/// This is what ends the walk — not a page size, and not a guess about how many
/// runs in a row a repository can cancel.
pub fn walked_past(candidate: &PriorRun, floor: &Held) -> bool {
    to_the_second(&candidate.started_at) <= to_the_second(&floor.created_at)
}

/// The commit the "did the hashed inputs move" question is asked from.
///
/// A PUSH CARRIES A RANGE AND THIS ONCE ASKED ABOUT ONE COMMIT. That is not a
/// hypothetical: two commits went up together, the workflow moved in the FIRST
/// of them, and `git diff HEAD~1 HEAD` saw a tip commit that had touched no
/// hashed input — so eight jobs that had legitimately rebuilt from nothing were
/// reported as a defect and turned main red. The gate refused for a reason
/// outside its own law, which is the same failure as a gate that does not fire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RangeStart {
    /// The commit this push started from, named by the runner
    /// (`github.event.before`) and present in this checkout.
    Push(String),
    /// The parent of `HEAD`, with the reason the push range was not used.
    ///
    /// CORRECT BY CONSTRUCTION FOR A PULL REQUEST, which is the case that
    /// reaches it in normal operation: the runner checks out a merge commit
    /// whose first parent is the base branch, so `HEAD~1..HEAD` is exactly the
    /// change the pull request proposes. For a multi-commit PUSH it is the
    /// narrow answer that caused the failure above, which is why the workflow
    /// passes the range and why this says out loud when it could not use it.
    ParentOfHead(&'static str),
}

impl RangeStart {
    /// The revision to diff from.
    pub fn rev(&self) -> &str {
        match self {
            RangeStart::Push(sha) => sha,
            RangeStart::ParentOfHead(_) => "HEAD~1",
        }
    }

    /// One line naming the range and, when it is the narrow one, why.
    pub fn why(&self) -> String {
        match self {
            RangeStart::Push(sha) => {
                format!(
                    "over {}..HEAD, the commits this push carried",
                    &sha[..7.min(sha.len())]
                )
            }
            RangeStart::ParentOfHead(reason) => {
                format!("over HEAD~1..HEAD ({reason})")
            }
        }
    }
}

/// The variable the workflow names the push's starting commit in.
pub const RANGE_VARIABLE: &str = "MNEMOSYNE_RANGE_FROM";

/// Which commit to ask from, given what the runner said and what the checkout
/// actually holds.
///
/// `present` IS A PARAMETER because the answer depends on the checkout depth,
/// which is a property of the machine and not of this decision. A commit named
/// but not fetched is the shallow-clone case, and diffing from it would make git
/// fail — the gate would then refuse to judge a repository that is fine.
pub fn range_start(named: Option<&str>, present: impl Fn(&str) -> bool) -> RangeStart {
    let Some(sha) = named.map(str::trim).filter(|sha| !sha.is_empty()) else {
        return RangeStart::ParentOfHead("no push range in the environment: not a push event");
    };
    // ALL ZEROS IS GITHUB'S "there was no previous tip" — a branch created by
    // this push. Everything in it is new, so there is no earlier commit to ask
    // about and the narrow range is the whole of what there is to see.
    if sha.chars().all(|digit| digit == '0') {
        return RangeStart::ParentOfHead(
            "this push created the branch, so it started from nothing",
        );
    }
    if !present(sha) {
        return RangeStart::ParentOfHead(
            "the commit this push started from is not in this checkout — it is \
             too shallow to see the whole push",
        );
    }
    RangeStart::Push(sha.to_string())
}

/// Why no record was read for one owner — the job's silence, or this gate's
/// horizon.
///
/// THE DISTINCTION THIS GATE PRINTED BACKWARDS. `evidence-replay.yml` declares a
/// cache, so its `replay` job is an owner in this report; its records are written
/// on a runner in a different run and no download step in a `mnemosyne-validate`
/// run can ever be handed them. The report said `replay` "did not say what it
/// started from", which names a job as deficient for a limit belonging to the
/// reader — and R1106 had just established that the repair such a sentence asks
/// for (wire the measurement into that workflow too) is the WRONG repair, because
/// that workflow uploads nothing and the record would die with its runner.
///
/// Every reason below is DERIVED — from the workflow files for what collects, and
/// from the runner for which run this is — so none of them is a list of exempt
/// jobs kept beside the law.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unheard {
    /// Its records were collectable here and it left none. The job's own silence,
    /// and the only one of these that is a gap in the repository.
    ItsOwnSilence,
    /// Its workflow uploads no artifact at all, so anything it writes is
    /// destroyed with its runner — unreadable from anywhere, not merely here.
    NothingCollectsIt { workflow: String },
    /// Its workflow is not this run's, and artifacts belong to a run.
    AnotherWorkflow { workflow: String },
    /// This gate is not inside a run, so no run's artifacts were collected.
    NoRun,
}

impl std::fmt::Display for Unheard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Unheard::ItsOwnSilence => write!(f, "did not say what it started from"),
            Unheard::NothingCollectsIt { workflow } => write!(
                f,
                "cannot be heard from anywhere: nothing in {workflow} uploads an \
                 artifact, so a record written there is destroyed with its runner"
            ),
            Unheard::AnotherWorkflow { workflow } => write!(
                f,
                "cannot be heard from here: it is declared in {workflow}, and a \
                 record is an artifact of the run that wrote it"
            ),
            Unheard::NoRun => write!(
                f,
                "cannot be heard from here: this is not a run, so no run's \
                 artifacts were collected"
            ),
        }
    }
}

/// What one repository's caching looks like.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub limit: u64,
    pub rows: Vec<Row>,
    pub orphans: Vec<Held>,
    pub divergent: Vec<Refusal>,
    /// The run this report is about, if it is about one.
    pub run: Option<Run>,
    /// What a job's disk held after one of its restores, by which restore it was.
    ///
    /// THE OTHER INSTRUMENT, JOINED HERE. This gate reads what STORAGE holds and
    /// `tools/restored` reads what a job's DISK received, and until R1101 nothing
    /// put the two together — a reader had to, and the one who did got it
    /// backwards. Empty where no record was read, which is a state this report
    /// says out loud rather than treating as "nothing was restored".
    ///
    /// PER RESTORE AND NOT PER JOB, which is exactly the join this report is:
    /// every lookup below already holds a [`Row`] (a key) and an [`Owner`] (a
    /// job), so the pair was always available and the map was throwing half of
    /// it away. A job with two caches under a job-keyed map reports one of them
    /// twice and the other never.
    pub started: BTreeMap<restored::Restore, restored::Warmth>,
    /// How many `actions/cache` steps the workflows declare. NOT `rows.len()`:
    /// several steps may share one key, and the gap between the two counts is
    /// the first thing a reader of this report checks.
    pub declared_steps: usize,
    /// How many caches GitHub holds, superseded generations and undeclared ones
    /// included — the population this report was reckoned against.
    pub held_caches: usize,
    /// Which tracked workflows collect an artifact at all, by path.
    ///
    /// HALF OF THIS GATE'S HORIZON, read off the files through the one reader
    /// that owns the question (`ci_plan::collects_artifacts`) so that this gate
    /// and the census gate cannot come to different answers about who can be
    /// heard. The other half is [`Run::workflow`].
    pub collecting: BTreeSet<String>,
}

impl Report {
    /// Why nothing was read for this owner — asked only when nothing was.
    ///
    /// ORDERED BY HOW FAR THE STATEMENT REACHES, most far-reaching first. "No
    /// artifact collects it" holds for every reader there will ever be and is the
    /// one that says a cross-run download would not help; "another workflow's"
    /// holds only from here and leaves that door open. A reader given the weaker
    /// reason when the stronger one is true would go and build the thing R1106
    /// established must not be built.
    pub fn unheard(&self, owner: &Owner) -> Unheard {
        if !self.collecting.contains(&owner.source) {
            return Unheard::NothingCollectsIt {
                workflow: owner.source.clone(),
            };
        }
        match &self.run {
            None => Unheard::NoRun,
            Some(run) if run.workflow != owner.source => Unheard::AnotherWorkflow {
                workflow: owner.source.clone(),
            },
            Some(_) => Unheard::ItsOwnSilence,
        }
    }

    /// Every owner whose record was READ although this gate reckons its workflow
    /// collects nothing — a derivation contradicted by what it was handed.
    ///
    /// THE ONE ANCHOR THAT DOES NOT COME FROM THE SAME READING IT CHECKS. Every
    /// sentence about who can be heard rests on `collecting`, and a reader that
    /// returned an empty set would explain all eight owners' silence with
    /// "nothing collects it" — a report that has become entirely self-consistent
    /// and entirely wrong, which is the class of defect this whole repair is
    /// about. A record in hand is the observation that reading cannot argue with.
    fn heard_where_nothing_collects(&self) -> Vec<Owner> {
        let mut out: Vec<Owner> = self
            .rows
            .iter()
            .flat_map(|row| row.owners.iter().map(move |owner| (row, owner)))
            .filter(|(row, owner)| {
                self.started.contains_key(&row.restore_by(owner))
                    && !self.collecting.contains(&owner.source)
            })
            .map(|(_, owner)| owner.clone())
            .collect();
        out.sort();
        out.dedup();
        out
    }

    /// The total the declared caches want, when it can be reckoned at all.
    pub fn demand(&self) -> Option<u64> {
        self.rows.iter().map(Row::bytes).sum()
    }

    /// What the archives no declaration matches weigh.
    ///
    /// R1123 — A COST AND NOT A CATEGORY. These used to be a refusal of their
    /// own, one per key, on the reasoning that "a key outlives the job that
    /// wrote it and keeps its share of the budget". The harm named there is the
    /// BUDGET, and the budget is arithmetic — so the honest place for these
    /// bytes is inside it, where 0.15 GB left behind by a rename passes and
    /// 8.90 GB left behind by a forgotten job does not.
    ///
    /// WHAT THE CATEGORICAL REFUSAL COST, measured on this repository: every key
    /// this file renames orphans its own archive for the seven days it takes to
    /// age out, so the gate refused a tree for making a REPAIR — and R1122 had
    /// to pin a real defect it could not close (`Linux-cargo-` reaching the
    /// build directory's archive) because the only repair for it is a rename.
    /// A gate that refuses for a reason outside its own law is the shape R1110
    /// wrote down and R1115 paid for again.
    ///
    /// THEY ARE STILL PRINTED, key by key, by [`render`]. Nothing about them is
    /// hidden; what changed is that the verdict is the sum rather than the
    /// presence.
    pub fn held_by_nothing(&self) -> u64 {
        self.orphans.iter().map(|orphan| orphan.size_in_bytes).sum()
    }

    /// The declared caches that are not there.
    pub fn absent(&self) -> Vec<String> {
        self.rows
            .iter()
            .filter(|row| row.held.is_none())
            .map(|row| row.prefix.clone())
            .collect()
    }

    /// Absent, and with nothing anywhere to price them from.
    pub fn unpriced(&self) -> Vec<String> {
        self.rows
            .iter()
            .filter(|row| row.bytes().is_none())
            .map(|row| row.prefix.clone())
            .collect()
    }

    /// Everything wrong, most consequential first.
    pub fn refusals(&self) -> Vec<Refusal> {
        let mut out = self.divergent.clone();
        let contradicted = self.heard_where_nothing_collects();
        if !contradicted.is_empty() {
            out.push(Refusal::Unreached(format!(
                "a record was read for {} whose workflow this gate reckons \
                 uploads no artifact at all — the record is in hand, so that \
                 reading is wrong, and every sentence this report makes about \
                 whose silence is whose rests on it",
                named(&contradicted).join(" and ")
            )));
        }
        if self.rows.is_empty() {
            out.push(Refusal::Unreached(
                "no workflow in this repository declares a cache, which cannot be \
                 true of a repository whose CI compiles Rust — an empty answer \
                 here is the reader failing, not the repository being tidy"
                    .to_string(),
            ));
            return out;
        }
        match self.demand() {
            None => out.push(Refusal::Unreached(format!(
                "{} of the {} declared caches are absent and nothing holding a \
                 subset of their paths has ever been observed, so what they cost \
                 is UNKNOWN rather than acceptable ({})",
                self.unpriced().len(),
                self.rows.len(),
                self.unpriced().join(", ")
            ))),
            // A LOWER BOUND IS ENOUGH TO REFUSE. The real demand is at least this,
            // and this is already over.
            //
            // R1123 — AND THE ARCHIVES NOTHING DECLARES ARE PART OF IT. They are
            // bytes GitHub is holding against the same 10 GB, so leaving them out
            // of the comparison and refusing them separately answered the harm
            // twice in one direction and never in the other: a 0.15 GB archive
            // left by a rename was refused, and a repository whose declarations
            // fit only because an 8.90 GB orphan was excluded from the sum was
            // not.
            Some(demand) if demand + self.held_by_nothing() > self.limit => {
                out.push(Refusal::OverBudget {
                    demand,
                    orphaned: self.held_by_nothing(),
                    limit: self.limit,
                    absent: self.absent(),
                })
            }
            // AND IT IS NEVER ENOUGH TO PASS. An absent cache priced off one that
            // holds only some of its paths is read at the cost of the parts
            // somebody has seen, and in this repository the unseen part is the
            // build tree — thirty times the registry beside it.
            Some(_) => {
                let mut blind: Vec<String> = self
                    .rows
                    .iter()
                    .filter_map(|row| {
                        let estimate = row.estimate.as_ref()?;
                        (!estimate.unpriced_paths.is_empty()).then(|| {
                            format!(
                                "{} (nothing observed has held {})",
                                row.prefix,
                                estimate
                                    .unpriced_paths
                                    .iter()
                                    .cloned()
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            )
                        })
                    })
                    .collect();
                if !blind.is_empty() {
                    blind.sort();
                    out.push(Refusal::Unreached(format!(
                        "the demand adds up to {} of the {} budget, but {} of the \
                         absent caches could only be priced from one holding PART \
                         of what they hold, so that total is a lower bound and not \
                         a reading: {}",
                        gigabytes(self.demand().unwrap_or_default()),
                        gigabytes(self.limit),
                        blind.len(),
                        blind.join("; ")
                    )));
                }
            }
        }
        // A CACHE THAT EXISTS IS NOT THE SAME AS A JOB THAT WAS WARM, and the
        // difference is the whole cost. `actions/cache` saves only when it did
        // NOT find an exact hit, so a cache whose `created_at` falls inside this
        // run is a job that restored nothing and rebuilt — green, unannotated,
        // and half an hour long. Round 1089 reached that verdict by hand, reading
        // three runs of job durations; this is the same judgement, made by the
        // program, from the one field that says it.
        //
        // Excused when the key's own hashed inputs moved in this commit: one cold
        // run is the honest price of a dependency change, and the key names the
        // globs that decide it.
        if let Some(run) = &self.run {
            for row in &self.rows {
                // WHAT A JOB'S DISK RECEIVED, WHICH IS NOT WHAT STORAGE HOLDS,
                // and the join this repository did not have. A generation that
                // predates the run is one `restore-keys` could have served; a job
                // that started from nothing anyway is the failure the budget
                // exists to prevent, and it is invisible on either instrument
                // alone. Checked whether or not the key was legitimately
                // invalidated: a dependency bump excuses a MISSED KEY, never an
                // empty disk.
                // A CACHE WITH NO `restore-keys` HAS NOTHING TO FALL BACK ON, so
                // a missed primary key and an empty tree are one event rather
                // than two, and there is no finding in it. Round 1160 made that
                // state deliberate for the build-directory cache and this
                // judgement kept reading an older archive as reachable, which
                // put `main` red on run 31646189780 for doing what it was asked.
                if row.falls_back {
                    for owner in &row.owners {
                        if self.started.get(&row.restore_by(owner))
                            != Some(&restored::Warmth::Nothing)
                        {
                            continue;
                        }
                        let Some(generation) = row.restorable_when(&run.started_at) else {
                            continue;
                        };
                        out.push(Refusal::RestoredNothingWithAGenerationHeld {
                            job: owner.job.clone(),
                            prefix: row.prefix.clone(),
                            generation: generation.clone(),
                        });
                    }
                }
                let Some(held) = &row.held else { continue };
                if to_the_second(&held.created_at) <= to_the_second(&run.started_at) {
                    continue;
                }
                // AND SOMETHING HAD TO BE THERE TO RESTORE, which is the
                // condition the sibling refusal above states outright and this
                // one was missing. R1135 measured what that cost on run
                // 31394095606: `Linux-cargo-side-` had its key moved by a
                // lockfile bump TWO PUSHES back, the run that would have saved
                // the new archive FAILED — `actions/cache` does not save from a
                // failed job — and no older generation survived. So this run
                // compiled from an empty tree, and the refusal told main it was
                // an unexplained rebuild while the true explanation was that
                // there was nothing to hit.
                //
                // THE HARM THIS NAMES IS A CACHE THAT WAS AVAILABLE AND WASTED.
                // With no restorable generation, a cold build was unavoidable
                // whatever the workflow says, and a refusal that fires anyway is
                // one nobody can act on: the range it diffs is THIS push, so a
                // key legitimately invalidated in an earlier one has no excuse
                // to offer. The cold state is still printed by the report — what
                // changes is only whether it stops a run.
                if row.restorable_when(&run.started_at).is_none() {
                    continue;
                }
                // THE EXCUSE, AND THE THIRD ANSWER R1312 SEPARATED FROM IT. A
                // key whose globs moved over the interval its archive could have
                // seen is legitimately cold. A key whose globs did not move is
                // the finding below. A key whose interval was never bounded is
                // NEITHER, and printing it as the second is how this gate
                // reported eight honest rebuilds as a defect and turned main red
                // — the interval it substituted was three commits short of the
                // one that explained them.
                match run.movement(&row.prefix) {
                    Some(true) => continue,
                    None => {
                        out.push(Refusal::Unreached(format!(
                            "`{}` was rebuilt by this run and whether that was legitimate \
                             is UNKNOWN rather than wrong — {}",
                            row.prefix,
                            run.unbounded(&row.prefix)
                                .map(Window::why)
                                .unwrap_or_else(|| "no interval was recorded for it".to_string())
                        )));
                        continue;
                    }
                    Some(false) => {}
                }
                out.push(Refusal::Recreated {
                    prefix: row.prefix.clone(),
                    falls_back: row.falls_back,
                    owners: row.owners.clone(),
                    hashed: row.hashed.clone(),
                    started: row
                        .owners
                        .iter()
                        .filter_map(|owner| {
                            Some((
                                owner.job.clone(),
                                *self.started.get(&row.restore_by(owner))?,
                            ))
                        })
                        .collect(),
                });
            }
        }
        out
    }
}

/// Which row a held cache belongs to: the one with the LONGEST prefix it starts
/// with, or none.
///
/// Most specific wins, and this repository is why. Its oldest key is
/// `${{ runner.os }}-cargo-`, which is a prefix of `Linux-cargo-unrun-` and of
/// every other key in the file. Giving a cache to every row it starts with would
/// credit the small general job with the huge specific job's cache, leave the
/// specific one looking satisfied, and report a repository where nothing is
/// absent and the demand is a third of the truth — a green verdict built entirely
/// out of double counting.
fn owner_of<'a>(prefixes: &'a [String], key: &str) -> Option<&'a String> {
    prefixes
        .iter()
        .filter(|prefix| key.starts_with(prefix.as_str()))
        .max_by_key(|prefix| prefix.len())
}

/// Hold the two sides against each other. PURE, and every input is an argument:
/// the limit so the boundary can be tested, both populations so the verdict can
/// be driven without a repository and without a network, and the run so that
/// "this cache was built just now" can be asked of a clock this function does not
/// read.
pub fn conclude(
    limit: u64,
    declared: &[CacheDeclaration],
    held: &[Held],
    run: Option<&Run>,
    started: &BTreeMap<restored::Restore, restored::Warmth>,
    collecting: &BTreeSet<String>,
) -> Report {
    let mut rows: Vec<Row> = Vec::new();
    // WHOSE ARCHIVE EACH CACHE'S FALLBACK CAN REACH — a property of the
    // DECLARATIONS alone, so it holds for a repository that has never run, and
    // it travels with the divergence refusals because both are the same kind of
    // statement: two declarations that cannot both be what they say they are.
    let mut divergent: Vec<Refusal> = ci_plan::fallback_reaches(declared)
        .into_iter()
        .map(|(cache, other)| {
            let asked: BTreeSet<&str> = cache.paths.iter().map(String::as_str).collect();
            Refusal::FallbackReachesAnotherCache {
                prefix: cache.prefix.clone(),
                other: other.prefix.clone(),
                holds: other
                    .paths
                    .iter()
                    .filter(|path| !asked.contains(path.as_str()))
                    .cloned()
                    .collect(),
            }
        })
        .collect();
    let mut at: BTreeMap<&str, usize> = BTreeMap::new();
    for declaration in declared {
        let owner = Owner {
            source: declaration.source.clone(),
            job: declaration.owner.clone(),
        };
        let paths: BTreeSet<String> = declaration.paths.iter().cloned().collect();
        match at.get(declaration.prefix.as_str()) {
            Some(&index) => {
                let row: &mut Row = &mut rows[index];
                if row.paths != paths {
                    let mut owners = row.owners.clone();
                    owners.push(owner.clone());
                    divergent.push(Refusal::Divergent {
                        prefix: row.prefix.clone(),
                        owners,
                    });
                    // The UNION, which is the loud direction: a key whose
                    // declarations disagree costs at least the most anybody
                    // claims for it.
                    row.paths.extend(paths);
                }
                // ONE OWNER WRITING A FALLBACK IS ENOUGH FOR THE ARCHIVE TO BE
                // REACHABLE: `restore-keys` is a property of the RESTORE, so a
                // key some job falls back on is one an archive can serve. Two
                // owners disagreeing about it is a separate finding — the
                // divergence above is what says so — and this stays the loud
                // direction, which is the one that keeps judging.
                row.falls_back |= !declaration.restore_keys.is_empty();
                row.owners.push(owner);
            }
            None => {
                at.insert(declaration.prefix.as_str(), rows.len());
                rows.push(Row {
                    prefix: declaration.prefix.clone(),
                    owners: vec![owner],
                    paths,
                    hashed: declaration.hashed.clone(),
                    held: None,
                    superseded: Vec::new(),
                    estimate: None,
                    falls_back: !declaration.restore_keys.is_empty(),
                });
            }
        }
    }

    let prefixes: Vec<String> = rows.iter().map(|row| row.prefix.clone()).collect();
    let mut claimed = vec![false; held.len()];
    for (index, cache) in held.iter().enumerate() {
        let Some(prefix) = owner_of(&prefixes, &cache.key) else {
            continue;
        };
        claimed[index] = true;
        let row = &mut rows[at[prefix.as_str()]];
        // THE NEWEST of the generations alive under one prefix, not the largest.
        // The question this gate answers is what the keys the workflows declare
        // cost, and that is one generation of each; the previous generation is
        // GitHub keeping something nobody asked for any more, and it goes away on
        // its own. Reading the largest instead would leave a repository red for a
        // week after it made its caches smaller — the exact repair this gate
        // shipped with, reported as the failure.
        match row.held.take() {
            Some(previous) if previous.created_at > cache.created_at => {
                row.superseded.push(cache.clone());
                row.held = Some(previous);
            }
            Some(previous) => {
                row.superseded.push(previous);
                row.held = Some(cache.clone());
            }
            None => row.held = Some(cache.clone()),
        }
    }
    for row in &mut rows {
        row.superseded
            .sort_by(|left, right| right.created_at.cmp(&left.created_at));
    }

    // Pricing is a SECOND pass, because a cache absent now may be priceable from
    // one declared later in the file, and a single pass would price it from
    // whatever happened to come first.
    let observed: Vec<(BTreeSet<String>, Held)> = rows
        .iter()
        .filter_map(|row| row.held.clone().map(|held| (row.paths.clone(), held)))
        .collect();
    for row in &mut rows {
        if row.held.is_some() {
            continue;
        }
        // A LOWER BOUND WITH A PROOF, not a guess: this cache holds everything
        // that one holds and more, so it costs at least that much. Pricing from
        // any cache that merely OVERLAPS would let a registry-only cache be
        // priced as a whole `target` and invent demand that is not there; pricing
        // only from an identical path list would leave every cache in this
        // repository unpriceable, because no two of them hold quite the same set.
        let best = observed
            .iter()
            .filter(|(paths, _)| paths.is_subset(&row.paths))
            .max_by_key(|(_, held)| held.size_in_bytes);
        row.estimate = best.map(|(paths, held)| Estimate {
            bytes: held.size_in_bytes,
            from: held.key.clone(),
            unpriced_paths: row.paths.difference(paths).cloned().collect(),
        });
    }

    let orphans: Vec<Held> = held
        .iter()
        .zip(&claimed)
        .filter(|(_, taken)| !**taken)
        .map(|(cache, _)| cache.clone())
        .collect();

    Report {
        limit,
        rows,
        orphans,
        divergent,
        run: run.cloned(),
        started: started.clone(),
        declared_steps: declared.len(),
        held_caches: held.len(),
        collecting: collecting.clone(),
    }
}

/// The report as a person reads it: what was reached, then every key, then the
/// totals, then whether the second half was evaluated at all.
///
/// A STRING AND NOT A `println!`, for the reason [`read_arguments`] is here: a
/// decision that lives in `main.rs` has no reader. R1096 measured what that
/// costs — the thing that lied was an exit code, and nothing in the suite could
/// ask it a question.
///
/// TWO INSTRUMENTS AND TWO UNITS, WHICH IS WHY THE LAST LINE OF THE ROWS EXISTS.
/// The GB against a key is what GitHub STORES, a compressed archive taken from
/// the caches API. The MB beside one of its jobs is what arrived on that job's
/// DISK, measured by `tools/restored` either side of the restore. On run
/// 31307111606 those two read 7.83 GB and 27258 MB for the same key — a factor
/// of three and a half — and 0.15 GB against 246 MB for the next one along.
/// Both numbers are right; neither says which quantity it is; and they are
/// printed one line apart, which is where a reader divides one by the other.
pub fn render(report: &Report) -> String {
    let mut out = String::new();
    // WHAT WAS REACHED, first and unconditionally. A gate that never opened
    // anything and a gate that found nothing wrong print the same silence.
    out.push_str(&format!(
        "{} cache step(s) across this repository's workflows under {} key(s), {} \
         held by GitHub, budget {:.2} GB\n",
        report.declared_steps,
        report.rows.len(),
        report.held_caches,
        report.limit as f64 / 1e9
    ));
    for row in &report.rows {
        let size = match (&row.held, &row.estimate) {
            (Some(held), _) => format!("{:>8.2} GB held", held.size_in_bytes as f64 / 1e9),
            (None, Some(estimate)) => format!(
                "{:>8.2} GB ABSENT, priced from {}",
                estimate.bytes as f64 / 1e9,
                estimate.from
            ),
            (None, None) => "       ? ABSENT, never observed".to_string(),
        };
        out.push_str(&format!(
            "  {size}  {}  [{}]  {}\n",
            row.prefix,
            row.paths.iter().cloned().collect::<Vec<_>>().join(" "),
            row.owners
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ));
        // WHAT ITS OWNERS ACTUALLY GOT, printed beside what storage holds. These
        // are the two instruments, and holding them apart is what let a warm run
        // be read as a cold one.
        //
        // AND WHOSE THE SILENCE IS WHEN THERE IS NO READING. A job that could
        // have been heard and was not is a gap in this repository; a job whose
        // records this gate can never be handed is a gap in the gate, and
        // printing one sentence for both puts the reader's attention on the
        // wrong side of the horizon. `replay` was reported as the first for two
        // rounds while being the second.
        for owner in &row.owners {
            match report.started.get(&row.restore_by(owner)) {
                Some(warmth) => {
                    out.push_str(&format!("            `{}` {}\n", owner.job, warmth.why()))
                }
                None => out.push_str(&format!(
                    "            `{}` {}\n",
                    owner.job,
                    report.unheard(owner)
                )),
            }
        }
        // PRINTED THOUGH NOT COUNTED. These are real bytes GitHub is holding, and
        // a gate that judged one generation while silently dropping the others
        // from its output would be reporting a smaller world than it looked at.
        for old in &row.superseded {
            out.push_str(&format!(
                "  {:>8.2} GB held under the same key, superseded on {} and aging \
                 out — not counted, because no workflow can stop a lockfile bump \
                 leaving one behind\n",
                old.size_in_bytes as f64 / 1e9,
                row.held
                    .as_ref()
                    .map_or("—", |newest| newest.created_at.as_str())
            ));
        }
    }
    // DECLARED BY NOTHING AND COUNTED ANYWAY. R1123: these are bytes GitHub is
    // holding against the same budget, so they are in the total below rather
    // than being a refusal of their own — which is what let a rename, the only
    // repair for a key whose fallback reaches another cache's archive, turn this
    // gate red for the seven days the old archive takes to age out.
    for orphan in &report.orphans {
        out.push_str(&format!(
            "  {:>8.2} GB held, declared by nothing and counted against the \
             budget until it ages out: {}\n",
            orphan.size_in_bytes as f64 / 1e9,
            orphan.key
        ));
    }
    // SAID ONLY WHERE BOTH KINDS OF NUMBER WERE ACTUALLY PRINTED. On a run that
    // read no record there is no second quantity to be confused with the first,
    // and a sentence about a comparison nobody can make is noise that teaches a
    // reader to skip the line.
    if report.rows.iter().any(|row| {
        row.held.is_some()
            && row
                .owners
                .iter()
                .any(|owner| report.started.contains_key(&row.restore_by(owner)))
    }) {
        out.push_str(
            "  the GB against a key is the archive GitHub stores; the MB beside a \
             job is what arrived on its disk, and one is not the other\n",
        );
    }
    // ONE LINE, AND IT IS THE LINE THE VERDICT IS MADE ON. The two quantities
    // are named apart because they answer different questions — what this
    // repository ASKS for, and what GitHub is holding for nobody — and summed
    // because the 10 GB does not care which is which.
    match report.demand() {
        Some(demand) if report.held_by_nothing() == 0 => {
            out.push_str(&format!("demand {:.2} GB\n", demand as f64 / 1e9))
        }
        Some(demand) => out.push_str(&format!(
            "demand {:.2} GB declared + {:.2} GB declared by nothing = {:.2} GB\n",
            demand as f64 / 1e9,
            report.held_by_nothing() as f64 / 1e9,
            (demand + report.held_by_nothing()) as f64 / 1e9
        )),
        None => out.push_str("demand UNKNOWN — nothing comparable has been observed\n"),
    }
    // WHETHER THE SECOND HALF WAS EVALUATED AT ALL, said out loud. A gate that
    // silently skipped a law and a gate that found nothing wrong under it print
    // the same clean line otherwise.
    match &report.run {
        // NAMED ONCE, BECAUSE THE PER-OWNER LINES POINT AT IT. Which workflow
        // this run is of decides whose records could have been collected here,
        // and a reader seeing "cannot be heard from here" needs to know what
        // "here" is without going to the runner for it.
        Some(run) => {
            // AND HOW MANY WERE NOT ASKED AT ALL, in the same sentence as how
            // many moved. A key whose interval was never bounded is not a key
            // whose inputs held still, and a count that folds the two prints a
            // gate that could not look as one that looked and found nothing.
            let unbounded = run
                .asked
                .iter()
                .filter(|asked| asked.moved.is_none())
                .count();
            out.push_str(&format!(
                "run of {} started {}, so a cache created after that is a job that \
                 rebuilt; {} of {} key(s) had their hashed inputs moved{}\n",
                run.workflow,
                run.started_at,
                run.asked
                    .iter()
                    .filter(|asked| asked.moved == Some(true))
                    .count(),
                run.asked.len(),
                match unbounded {
                    0 => String::new(),
                    count => format!(", and {count} of them could not be asked"),
                }
            ));
            // AND OVER WHICH INTERVAL EACH ANSWERED, grouped by the interval
            // rather than listed per key. The intervals are per WORKFLOW, so a
            // repository declaring nine keys in two files prints two lines — and
            // a reader can tell a key excused over its own workflow's history
            // from one excused over a push, which is the distinction that was
            // invisible while every key shared one range.
            let mut over: BTreeMap<String, Vec<String>> = BTreeMap::new();
            for asked in &run.asked {
                over.entry(asked.over.why()).or_default().push(format!(
                    "{}{}",
                    asked.prefix,
                    match asked.moved {
                        Some(true) => " MOVED",
                        Some(false) => "",
                        None => " NOT ASKED",
                    }
                ));
            }
            for (interval, keys) in over {
                out.push_str(&format!("      asked {interval}: {}\n", keys.join(", ")));
            }
        }
        None => out.push_str(
            "NOT INSIDE A RUN (`GITHUB_RUN_ID` unset), so whether these caches were \
             restored or rebuilt was NOT evaluated — only the budget was\n",
        ),
    }
    out
}

/// The tree to judge, and where the restore records were collected.
///
/// A FLAG'S VALUE IS NOT A POSITIONAL ARGUMENT, and reading it as one is a whole
/// class of silent wrong answer: `--restored rustc-log` under a reader that took
/// the first word not beginning with `--` would judge the caches of a repository
/// rooted at `rustc-log`, find no workflow there, and refuse for a reason with
/// nothing to do with any cache. So the words are walked once, in order, and a
/// flag consumes its own value.
///
/// IN THE LIBRARY BECAUSE IT IS THE ONE PART OF THE ENTRANCE THAT CAN BE ASKED A
/// QUESTION. The rest of `main` needs a network and a repository; this is pure,
/// and R1096 measured what living in `main.rs` costs a decision — nothing had a
/// reader, because nothing ran the binary.
pub fn read_arguments(arguments: &[String]) -> (std::path::PathBuf, Option<String>) {
    let mut root = None;
    let mut restored = None;
    let mut words = arguments.iter();
    while let Some(word) = words.next() {
        match word.as_str() {
            "--restored" => {
                restored = Some(
                    words
                        .next()
                        .unwrap_or_else(|| panic!("--restored needs a directory"))
                        .clone(),
                );
            }
            other if other.starts_with("--") => panic!("unknown flag {other}"),
            other => {
                assert!(
                    root.is_none(),
                    "one tree at a time: already judging {root:?}, and now given {other:?}"
                );
                root = Some(std::path::PathBuf::from(other));
            }
        }
    }
    (
        root.unwrap_or_else(|| std::path::PathBuf::from(".")),
        restored,
    )
}

/// Every measured start, read out of the records the jobs uploaded.
///
/// A RECORD THAT DOES NOT DECODE IS DROPPED HERE AND REFUSED THERE. The census
/// gate reads the same directory and turns an unreadable record into a refusal
/// naming the file; this gate would be reporting the same defect a second time
/// under a different name, and a second reader of one datum is where two answers
/// come from. What it must not do is read the absence as "restored nothing",
/// which is why the map is keyed only by what was actually said.
///
/// KEYED BY WHAT THE RECORD SAYS IT IS, and not by the file it arrived in. A
/// record is one CACHE's since R1117, so a job declaring two writes two files
/// and a map keyed by the job would hold whichever `read_dir` reached last — the
/// registry's 5-second restore standing in for the build directory's, which is
/// the substitution the whole split exists to prevent. The file name says only
/// that it is its job's; which cache is written inside it.
pub fn started_from(
    directory: &std::path::Path,
) -> std::io::Result<BTreeMap<restored::Restore, restored::Warmth>> {
    let mut out = BTreeMap::new();
    for entry in std::fs::read_dir(directory)? {
        let path = entry?.path();
        if path.extension().and_then(|end| end.to_str()) != Some("restored") {
            continue;
        }
        if let Ok(record) = restored::decode(&std::fs::read_to_string(&path)?) {
            out.insert(record.restore(), record.warmth());
        }
    }
    Ok(out)
}
