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
//! - nothing is HELD that nothing declares. A key outliving the job that wrote it
//!   keeps eating the budget, and it can only be found by asking both sides;
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
    /// Every job that declares this key, as `<workflow> \`<job>\``.
    pub owners: Vec<String>,
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
    /// The declared caches cannot all exist at once.
    OverBudget {
        demand: u64,
        limit: u64,
        absent: Vec<String>,
    },
    /// Held, and declared by nothing — budget spent on a job that no longer
    /// exists, which shows up only when both sides are asked.
    Orphan { key: String, size_in_bytes: u64 },
    /// One key, two jobs, two different answers about what it holds.
    Divergent { prefix: String, owners: Vec<String> },
    /// Held, and built from nothing BY THIS RUN — the job missed and paid for a
    /// cold build, which is the cost the whole budget exists to buy off.
    Recreated {
        prefix: String,
        owners: Vec<String>,
        hashed: Vec<String>,
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
                limit,
                absent,
            } => write!(
                f,
                "the caches this repository declares come to {} against a {} \
                 budget, so GitHub deletes them, least recently used first, until \
                 they fit — {} of them are absent right now ({}). Every job \
                 restoring one of those rebuilds from nothing on every run, and \
                 the only symptom is a green job that takes half an hour",
                gigabytes(*demand),
                gigabytes(*limit),
                absent.len(),
                absent.join(", ")
            ),
            Refusal::Orphan { key, size_in_bytes } => write!(
                f,
                "`{key}` holds {} and no workflow declares it — a key outlives the \
                 job that wrote it and keeps its share of the budget",
                gigabytes(*size_in_bytes)
            ),
            Refusal::Divergent { prefix, owners } => write!(
                f,
                "`{prefix}` is declared with different paths by {} — one key is \
                 one cache, so what it holds would depend on which job saved it \
                 first, and every job restoring the other spelling gets a tree it \
                 did not ask for",
                owners.join(" and ")
            ),
            Refusal::Recreated {
                prefix,
                owners,
                hashed,
            } => write!(
                f,
                "`{prefix}` was BUILT FROM NOTHING by this run, so {} paid for a \
                 cold build — `actions/cache` saves only when it did not find an \
                 exact hit, and {}. This is the cost the budget exists to buy \
                 off, and it is invisible: the job is green and merely slow",
                owners.join(" and "),
                if hashed.is_empty() {
                    "this key hashes nothing, so nothing could have invalidated it".to_string()
                } else {
                    format!(
                        "nothing matching {} changed in this commit",
                        hashed.join(", ")
                    )
                }
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

/// What one repository's caching looks like.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub limit: u64,
    pub rows: Vec<Row>,
    pub orphans: Vec<Held>,
    pub divergent: Vec<Refusal>,
    /// The run this report is about, if it is about one.
    pub run: Option<Run>,
}

impl Report {
    /// The total the declared caches want, when it can be reckoned at all.
    pub fn demand(&self) -> Option<u64> {
        self.rows.iter().map(Row::bytes).sum()
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
            Some(demand) if demand > self.limit => out.push(Refusal::OverBudget {
                demand,
                limit: self.limit,
                absent: self.absent(),
            }),
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
                });
            }
        }
        for orphan in &self.orphans {
            out.push(Refusal::Orphan {
                key: orphan.key.clone(),
                size_in_bytes: orphan.size_in_bytes,
            });
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
) -> Report {
    let mut rows: Vec<Row> = Vec::new();
    let mut divergent = Vec::new();
    let mut at: BTreeMap<&str, usize> = BTreeMap::new();
    for declaration in declared {
        let owner = format!("{} `{}`", declaration.source, declaration.owner);
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
    }
}
