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
#[derive(Debug, Clone, PartialEq, Eq)]
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
    pub fn restorable_when(&self, started_at: &str) -> Option<&Held> {
        self.held
            .iter()
            .chain(self.superseded.iter())
            .filter(|generation| to_the_second(&generation.created_at) <= to_the_second(started_at))
            .max_by(|left, right| left.created_at.cmp(&right.created_at))
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
    Recreated {
        prefix: String,
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
                if started.is_empty() {
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
    /// The key prefixes whose hashed inputs moved in the range this run covers,
    /// so that a cache this run had to build is excused rather than refused.
    /// Computed by asking git with the globs the key itself names, because a
    /// second implementation of GitHub's glob matching is a second answer.
    pub inputs_changed: BTreeSet<String>,
    /// Where that question was asked FROM, and why — printed with the verdict,
    /// because two different ranges answer two different questions and a reader
    /// cannot tell which one a number came from.
    pub range: RangeStart,
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
                for owner in &row.owners {
                    if self.started.get(&row.restore_by(owner)) != Some(&restored::Warmth::Nothing)
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
                let Some(held) = &row.held else { continue };
                if to_the_second(&held.created_at) <= to_the_second(&run.started_at) {
                    continue;
                }
                if run.inputs_changed.contains(&row.prefix) {
                    continue;
                }
                out.push(Refusal::Recreated {
                    prefix: row.prefix.clone(),
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
        Some(run) => out.push_str(&format!(
            "run of {} started {}, so a cache created after that is a job that \
             rebuilt; {} key(s) had their hashed inputs moved {}{}\n",
            run.workflow,
            run.started_at,
            run.inputs_changed.len(),
            run.range.why(),
            if run.inputs_changed.is_empty() {
                String::new()
            } else {
                format!(
                    " ({})",
                    run.inputs_changed
                        .iter()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        )),
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
