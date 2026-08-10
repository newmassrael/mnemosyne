//! What this repository's CI caches COST in wall-clock, read off the runs it has
//! already paid for.
//!
//! WHY THIS EXISTS. GG1b asked whether the `unrun-tests` cache pays for itself,
//! and the arithmetic the ledger carries is one subtraction: a warm job compiled
//! 332 units in 194.6 s where a cold one compiled 1017 in 409.2 s, so the cache
//! saves about 214 s of compiling and costs 134 s of restoring — net about +81 s.
//! A later run made the same subtraction come out at +27 s, and the whole of the
//! difference was in the restore seconds. From that it was concluded that the
//! question needed REPEATED RUNS of one commit to settle.
//!
//! TWO THINGS ARE WRONG WITH STOPPING THERE, and both are readable today:
//!
//!   - THE REPEATS HAVE ALREADY BEEN PAID FOR. Every push runs this workflow, and
//!     GitHub keeps each run's per-step timings to the second. The noisy term is
//!     sampled dozens of times over in history; commissioning new runs to measure
//!     it is buying a thing already owned.
//!   - THE SUBTRACTION IS MISSING A TERM. `actions/cache` saves whenever the
//!     primary key missed, and the save is a step of the job — `Post Cache …`.
//!     In run 31432163172 that step took 113 s for the build directory, against a
//!     144 s restore. No version of the recorded arithmetic counts it, and this
//!     repository moves every cache key on any push that touches a lockfile or
//!     the workflow, which is most of them. A cost that is paid on most runs and
//!     counted on none can carry the answer's sign.
//!
//! SO THIS PROGRAM PRICES THE COST SIDE, per cache, per run, over history — and
//! it does not pretend to compute the net. What a cache SAVED is compile seconds
//! that did not happen, which no timing of a run that used the cache can contain;
//! the ledger holds those observations, and the derivation belongs there. R1124
//! is the reason that separation is written down rather than assumed: a number
//! filed without its derivation went stale in silence and the same question came
//! back as an owner decision five times.

use std::collections::BTreeMap;

/// One step of one job, as GitHub answers for it.
///
/// TIMESTAMPS ARE PRESENT-BUT-NULLABLE and typed to say so. A step that never ran
/// carries nulls, and serde treats a DERIVED `Option` field as OPTIONAL — so a
/// body that stopped carrying `started_at` would read as a run where nothing ever
/// started, priced at nothing. `deserialize_with` takes the implicit default away;
/// R1136 found that trap one gate over, in the field that decides whether a commit
/// is red.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct Step {
    pub name: String,
    #[serde(deserialize_with = "present_but_nullable")]
    pub started_at: Option<String>,
    #[serde(deserialize_with = "present_but_nullable")]
    pub completed_at: Option<String>,
}

/// One job of one run, with the steps this program prices.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct Job {
    pub name: String,
    #[serde(deserialize_with = "present_but_nullable")]
    pub conclusion: Option<String>,
    #[serde(deserialize_with = "present_but_nullable")]
    pub started_at: Option<String>,
    #[serde(deserialize_with = "present_but_nullable")]
    pub completed_at: Option<String>,
    pub steps: Vec<Step>,
}

/// Read a field GitHub always sends and may send as `null`.
fn present_but_nullable<'de, D, T>(reader: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    serde::Deserialize::deserialize(reader)
}

/// One page of the jobs endpoint.
#[derive(serde::Deserialize)]
struct JobsPage {
    total_count: u64,
    jobs: Vec<Job>,
}

/// One workflow run, as the runs endpoint answers.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct Run {
    pub id: u64,
    pub head_sha: String,
    #[serde(deserialize_with = "present_but_nullable")]
    pub conclusion: Option<String>,
    pub created_at: String,
}

/// One page of the runs endpoint.
///
/// ITS `total_count` MEANS SOMETHING ELSE than the jobs endpoint's, and reusing
/// the neighbouring law here would be wrong: this one counts every run the
/// repository has ever had, not the rows on the page. So the check is that a page
/// asked for `n` came back with at most `n`, and that a SHORT page is explained by
/// the repository holding fewer runs than were asked for.
#[derive(serde::Deserialize)]
struct RunsPage {
    total_count: u64,
    workflow_runs: Vec<Run>,
}

/// What this program asks GitHub for the last `wanted` runs of one workflow.
///
/// THE WORKFLOW IS AN ARGUMENT, not a constant: this repository has two, and a
/// gate that named one of them in its source would price whichever the author was
/// thinking of. `{owner}` and `{repo}` are `gh`'s own placeholders, resolved from
/// the checkout.
pub fn runs_query(workflow: &str, wanted: usize) -> Vec<String> {
    vec![
        "api".to_string(),
        format!("repos/{{owner}}/{{repo}}/actions/workflows/{workflow}/runs?per_page={wanted}"),
    ]
}

/// What this program asks GitHub about one run's jobs.
pub fn jobs_query(run_id: u64) -> Vec<String> {
    vec![
        "api".to_string(),
        "--paginate".to_string(),
        format!("repos/{{owner}}/{{repo}}/actions/runs/{run_id}/jobs"),
    ]
}

/// The answer a `gh` that failed quietly gives.
const NOTHING_PRINTED: &str = "`gh` printed nothing at all, which is not the answer an empty \
     page gives — that one arrives as a page saying so";

/// Every job of one run, read off GitHub's own answer.
///
/// `total_count` HERE IS THE ROW COUNT, and it is checked, because a read that
/// stops early is a run with fewer jobs — and a job that is not in the answer is
/// a cache whose cost is not in the total.
pub fn jobs_in(run_id: u64, body: &str) -> Result<Vec<Job>, String> {
    let mut jobs = Vec::new();
    let mut counted: Option<u64> = None;
    for (index, page) in serde_json::Deserializer::from_str(body)
        .into_iter::<JobsPage>()
        .enumerate()
    {
        let page = page.map_err(|why| {
            format!(
                "page {} of GitHub's answer about run {run_id}'s jobs is not a shape this program \
                 can read ({why}) — it needs `total_count` and `jobs`, and in each job a `name`, \
                 a `conclusion`, a `started_at`, a `completed_at` and `steps` carrying the same. \
                 An answer missing one of those is a read that failed, not a run that cost nothing",
                index + 1
            )
        })?;
        match counted {
            None => counted = Some(page.total_count),
            Some(first) if first != page.total_count => {
                return Err(format!(
                    "GitHub said run {run_id} has {first} job(s) on the first page of its answer \
                     and {} on page {} — the run moved underneath this read",
                    page.total_count,
                    index + 1
                ));
            }
            Some(_) => {}
        }
        jobs.extend(page.jobs);
    }
    let counted = counted.ok_or_else(|| NOTHING_PRINTED.to_string())?;
    if jobs.len() as u64 != counted {
        return Err(format!(
            "GitHub said run {run_id} has {counted} job(s) and {} arrived — a partial read prices \
             fewer caches than the run paid for, and fewer is the direction that reads as cheap",
            jobs.len()
        ));
    }
    Ok(jobs)
}

/// The last runs of one workflow, read off GitHub's own answer.
pub fn runs_in(wanted: usize, body: &str) -> Result<Vec<Run>, String> {
    let page: RunsPage = serde_json::from_str(body.trim()).map_err(|why| {
        if body.trim().is_empty() {
            NOTHING_PRINTED.to_string()
        } else {
            format!(
                "GitHub's answer about this workflow's runs is not a shape this program can read \
                 ({why}) — it needs `total_count` and `workflow_runs`, and in each row an `id`, a \
                 `head_sha`, a `conclusion` and a `created_at`"
            )
        }
    })?;
    if page.workflow_runs.len() > wanted {
        return Err(format!(
            "{wanted} run(s) were asked for and {} arrived — a page longer than its request is an \
             answer to a question this program did not put",
            page.workflow_runs.len()
        ));
    }
    // A SHORT PAGE IS ONLY HONEST IF THE WORKFLOW REALLY HAS THAT FEW. Anything
    // else is a read that stopped early, and it arrives as a smaller sample —
    // which is the direction in which a distribution looks tighter than it is.
    if (page.workflow_runs.len() as u64) < wanted as u64
        && page.total_count > page.workflow_runs.len() as u64
    {
        return Err(format!(
            "{wanted} run(s) were asked for, {} arrived, and GitHub says this workflow has {} — a \
             page shorter than both is a read that stopped early, and a smaller sample looks like \
             a tighter one",
            page.workflow_runs.len(),
            page.total_count
        ));
    }
    Ok(page.workflow_runs)
}

/// Seconds between two of GitHub's stamps.
///
/// THE DOMAIN IS STATED RATHER THAN AVOIDED: both stamps are fixed-width UTC to
/// the second (`2026-08-10T21:05:31Z`) and belong to one step of one job, and
/// GitHub kills a job at six hours. So the difference is taken within a day, and
/// a day boundary crossed between them is the one carry this needs. That is why
/// no calendar is pulled in for a subtraction — and why this refuses, rather than
/// guessing, anything it cannot read as that shape.
pub fn seconds_between(from: &str, to: &str) -> Result<u64, String> {
    let start = second_of_day(from)?;
    let end = second_of_day(to)?;
    let seconds = if end >= start {
        end - start
    } else {
        // One midnight, which is the only carry a step shorter than a day can
        // have.
        end + 86_400 - start
    };
    // AND A CEILING, BECAUSE THE RECORDINGS SHOWED WHY. A job GitHub skipped
    // reports stamps that run BACKWARDS — `…T14:01:37Z` to `…T14:01:36Z`, one
    // second the wrong way — and the carry above reads that as 86 399 seconds,
    // which is a number that would quietly dominate every total on this page.
    // GitHub kills a job at six hours, so anything past that is not a duration
    // this program can have measured, whichever way it arose.
    if seconds > 6 * 3600 {
        return Err(format!(
            "`{from}` to `{to}` reads as {seconds} s, and GitHub stops a job at six hours — a \
             stamp pair this far apart is either running backwards (which a skipped job's really \
             does) or is not the pair this program thinks it is"
        ));
    }
    Ok(seconds)
}

/// The second of the day one of GitHub's stamps names.
fn second_of_day(stamp: &str) -> Result<u64, String> {
    let time = stamp
        .split_once('T')
        .map(|(_, time)| time.trim_end_matches('Z'))
        .ok_or_else(|| unreadable_stamp(stamp))?;
    let mut fields = time.split(':');
    let mut next = || -> Result<u64, String> {
        fields
            .next()
            .and_then(|field| field.parse::<u64>().ok())
            .ok_or_else(|| unreadable_stamp(stamp))
    };
    let (hours, minutes, seconds) = (next()?, next()?, next()?);
    if hours > 23 || minutes > 59 || seconds > 60 {
        return Err(unreadable_stamp(stamp));
    }
    Ok(hours * 3600 + minutes * 60 + seconds)
}

fn unreadable_stamp(stamp: &str) -> String {
    format!(
        "`{stamp}` is not the fixed-width UTC stamp this program reads \
         (`2026-08-10T21:05:31Z`) — a duration guessed from an unreadable clock is a number \
         nobody can defend"
    )
}

/// What one cache cost one job, in seconds.
///
/// `None` FOR A STEP THAT DID NOT RUN, which is not the same as a step that cost
/// nothing: a skipped or cancelled job carries the step with no stamps at all, and
/// pricing that at zero would pull every summary below towards a cache that is
/// cheaper than it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Price {
    pub job: String,
    pub cache: String,
    pub restore: Option<u64>,
    pub save: Option<u64>,
}

impl Price {
    /// What this cache cost the job in total, when both halves ran.
    pub fn total(&self) -> Option<u64> {
        Some(self.restore? + self.save?)
    }
}

/// The prefix GitHub gives the step that RESTORES a cache, and the one it gives
/// the step that SAVES it.
const RESTORE: &str = "Cache ";
const SAVE: &str = "Post Cache ";

/// Every cache one run's jobs paid for, priced from the steps either side of it.
///
/// THE PAIR IS THE UNIT, and an unpaired half is a refusal rather than a row.
/// `actions/cache` contributes exactly two steps per cache — the restore it
/// declares and the post-step that saves — so one without the other means this
/// program is reading a shape it was not written for, and a row built from half a
/// pair prices a cache at half its cost.
pub fn prices_in(jobs: &[Job]) -> Result<Vec<Price>, String> {
    let mut prices = Vec::new();
    for job in jobs {
        let mut restores: BTreeMap<&str, &Step> = BTreeMap::new();
        let mut saves: BTreeMap<&str, &Step> = BTreeMap::new();
        for step in &job.steps {
            // SAVE IS TESTED FIRST because its prefix CONTAINS the restore's:
            // `Post Cache x` starts with `Post ` and holds `Cache ` right after
            // it, so a reader that asked about the restore prefix with
            // `contains` — or that tested them the other way round after a
            // refactor — would file every save as a second restore.
            if let Some(name) = step.name.strip_prefix(SAVE) {
                if saves.insert(name, step).is_some() {
                    return Err(twice(&job.name, SAVE, name));
                }
            } else if let Some(name) = step.name.strip_prefix(RESTORE) {
                if restores.insert(name, step).is_some() {
                    return Err(twice(&job.name, RESTORE, name));
                }
            }
        }
        for (name, restore) in &restores {
            let save = saves.remove(name).ok_or_else(|| {
                format!(
                    "job `{}` restores the cache `{name}` and never saves it — `actions/cache` \
                     contributes both steps, so half a pair is a shape this program was not \
                     written for, and pricing it would report half of what the cache cost",
                    job.name
                )
            })?;
            prices.push(Price {
                job: job.name.clone(),
                cache: (*name).to_string(),
                restore: ran_for(restore)?,
                save: ran_for(save)?,
            });
        }
        if let Some((orphan, _)) = saves.iter().next() {
            return Err(format!(
                "job `{}` saves the cache `{orphan}` and never restores it — the same half-pair, \
                 from the other side",
                job.name
            ));
        }
    }
    Ok(prices)
}

fn twice(job: &str, prefix: &str, name: &str) -> String {
    format!(
        "job `{job}` carries two `{prefix}{name}` steps — two prices for one cache is a total \
         nobody can defend, and taking either one is a choice this program will not make"
    )
}

/// How long a step ran, or `None` when it did not run at all.
fn ran_for(step: &Step) -> Result<Option<u64>, String> {
    match (&step.started_at, &step.completed_at) {
        (Some(from), Some(to)) => seconds_between(from, to).map(Some),
        (None, None) => Ok(None),
        // ONE STAMP AND NOT THE OTHER is neither a step that ran nor one that did
        // not, and both readings are wrong: a start with no end is a step still
        // running, which a completed run does not have.
        _ => Err(format!(
            "step `{}` has one of its two stamps and not the other ({:?} .. {:?}) — that is \
             neither a step that ran nor one that did not",
            step.name, step.started_at, step.completed_at
        )),
    }
}

/// What a set of measurements looks like.
///
/// MIN, MEDIAN AND MAX AND THE COUNT, rather than a mean: the question this
/// program was built for is how far the noisy term SPREADS, and a mean is the one
/// summary that hides exactly that. The median is the lower of the two middles
/// when the count is even, so every number printed is one that was measured.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spread {
    pub count: usize,
    pub min: u64,
    pub median: u64,
    pub max: u64,
    pub total: u64,
}

/// The spread of a set of measurements, or `None` when there are none.
///
/// NOT ZERO FOR AN EMPTY SET. A summary of nothing that prints as zeroes is the
/// empty answer that reads like a clean one, and this whole round exists because a
/// missing term read as a zero.
pub fn spread(values: &[u64]) -> Option<Spread> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    Some(Spread {
        count: sorted.len(),
        min: sorted[0],
        median: sorted[(sorted.len() - 1) / 2],
        max: sorted[sorted.len() - 1],
        total: sorted.iter().sum(),
    })
}

/// Every cache seen across a set of runs, keyed by the job and cache it names.
///
/// KEYED BY BOTH, because a cache is a job's: two jobs restoring the same archive
/// pay for it separately, and merging them would report one price for two costs.
pub fn by_cache(prices: &[Price]) -> BTreeMap<(String, String), Vec<&Price>> {
    let mut out: BTreeMap<(String, String), Vec<&Price>> = BTreeMap::new();
    for price in prices {
        out.entry((price.job.clone(), price.cache.clone()))
            .or_default()
            .push(price);
    }
    out
}
