//! What a push learned about what CI cost, KEPT — so the next push can say
//! whether it is going UP.
//!
//! WHY THIS EXISTS. R1245 made this reporter print, on every push, the job
//! closest to its budget: `the closest to its budget was `validate` — 40m24s of
//! 90m (44%)`. That sentence is a LEVEL. Nothing kept it, so the only way to ask
//! whether forty-four per cent is where that job has always sat or where it
//! arrived this month was for a person to hold two screens side by side — and
//! nobody does that, which is how R1229 changed the work of a job's longest step,
//! left the budget alone, and found out from a cancellation. A level answers "is
//! this job close to dying"; it cannot answer "is this job costing more than it
//! used to", and the second question is the one a warning band is too late for.
//!
//! A LEVEL AND A TREND ARE TWO LAWS AND THIS FILE DOES NOT MERGE THEM. The
//! obvious next move is to derive [`crate::CREEPING`] from this record — "warn at
//! the ninetieth percentile of what this repository actually does" — and it is
//! the one move that cannot work. A threshold read off the record's own history
//! RISES WITH THE CREEP it is supposed to catch: every push that costs a little
//! more raises the baseline the threshold is measured from, so a job that doubles
//! over two months is inside the band on every single push. The band stays what
//! it is — a share of the budget, which is the thing that actually ends the job —
//! and what this file adds is MOVEMENT, printed with no threshold at all. A
//! number a reader watches is a different instrument from an alarm, which is the
//! argument R1245 made for printing the worst job even when everything is fine.
//!
//! WHAT IS KEPT IS RAW AND THE SHARE IS COMPUTED. A record holds what the job
//! TOOK and what it was ALLOWED, never the percentage between them, so there is
//! one spelling of that division ([`Spent::percent`]) and a record cannot come to
//! disagree with the reporter that wrote it. It also makes the trap below
//! visible: raising a job's `timeout-minutes` lowers its share without making the
//! job one second cheaper, and a record that had kept only the percentage would
//! print that as an improvement.
//!
//! LOSING THIS DIRECTORY LOSES NOTHING THAT CANNOT BE ASKED FOR AGAIN. It lives
//! under `target/`, which `cargo clean` empties and which no commit carries — and
//! that is affordable precisely because every row here is a PROJECTION of an
//! answer GitHub still holds. Reporting on a commit records it, so a history
//! wiped by a clean is refilled by reporting on the commits again; there is no
//! separate importer to keep working, and no state here that is the only copy of
//! anything.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::{clock, epoch_seconds, share_of, short, Check, Spent, RAN_TO_COMPLETION};

/// Where this reporter keeps what it measured, relative to the tree it reports
/// on.
///
/// UNDER `target/` BECAUSE THAT IS WHERE THIS REPOSITORY IS ENTITLED TO WRITE
/// RECORDS. `scratch-budget`'s `resolve` refuses to collect a directory outside a
/// build directory — a collector deletes files — so a record directory anywhere
/// else is one that can never gain a collector, which is the state R1199 found
/// six directories in.
///
/// DECLARED IN `tools/scratch-budget/scratch.json` AND HELD THERE BY A LAW in
/// this crate's own suite. The law lives with the WRITER rather than with the
/// collector, and that is a decision: `scratch-budget`'s own law asks the two
/// kinds of program it can ask — sweep manifests, which declare `logs`, and
/// `scripts/verify.sh`, which is asked — and this reporter is neither. Asking it
/// from there would mean compiling this crate inside that crate's test; asserting
/// it from here costs a lookup in a tracked file.
pub const RECORDS: &str = "target/ci-budgets";

/// Where the records of one tree live.
#[must_use]
pub fn records_in(tree: &Path) -> PathBuf {
    tree.join(RECORDS)
}

/// What one commit's jobs cost, as this reporter keeps it.
///
/// `deny_unknown_fields` FOR THE SAME REASON THE COLLECTOR'S DECLARATION HAS IT:
/// a key nobody meant, sitting beside one that matters, is a record that reads as
/// complete while carrying a field this program has never looked at. There is one
/// machine writing these and one program reading them, so a refusal here is loud
/// and costs nobody else anything.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Kept {
    /// The commit these jobs ran on.
    pub commit: String,
    /// The earliest start GitHub stamped on any of this commit's checks.
    ///
    /// THE ORDERING KEY, AND IT IS THE RUN'S OWN FACT RATHER THAN THIS FILE'S
    /// MTIME. A push reports on the commit it is building ON, so the ordinary
    /// order of writing here is the order of commits — but reporting on an older
    /// commit is exactly how a wiped history is refilled, and under an mtime key
    /// that would insert a two-week-old run as the newest point of every trend.
    pub ran_at: String,
    /// Every job of this commit whose cost could be held against a budget.
    pub jobs: Vec<Spent>,
}

impl Kept {
    /// What is worth keeping about one commit, or nothing.
    ///
    /// A COMMIT WITH NO MEASURED JOB IS NOT RECORDED AT ALL. An empty record is
    /// a row in every count and a point in no series: it would make the history
    /// look longer than the evidence in it, which is the direction that reads as
    /// "this trend is well founded".
    #[must_use]
    pub fn of(commit: &str, checks: &[Check], spent: &[Spent]) -> Option<Self> {
        if spent.is_empty() {
            return None;
        }
        let ran_at = checks
            .iter()
            .filter_map(|check| check.started_at.as_deref())
            .filter_map(|stamp| epoch_seconds(stamp).map(|at| (at, stamp)))
            .min_by_key(|(at, _)| *at)
            .map(|(_, stamp)| stamp.to_string())?;
        Some(Self {
            commit: commit.to_string(),
            ran_at,
            jobs: spent.to_vec(),
        })
    }
}

/// The file one commit's record is written to, or why that commit cannot name a
/// file.
///
/// THE COMMIT IS DATA AND A DIRECTORY WALKED FROM DATA HAS TO BE BOUNDED. This
/// name arrives on `argv`, and `<tree>/target/ci-budgets/<whatever>.json` with no
/// guard writes wherever a `..` in it points. R1259 paid for the general form of
/// this one round earlier — a manifest field naming a directory, resolved against
/// whoever ran it, walked a whole home directory — and the lesson there was that
/// the defect belongs to the unbounded walk rather than to the input that
/// exposed it. A git object name is lowercase hex and there is no reason to
/// accept anything else.
fn record_name(commit: &str) -> Result<String, String> {
    let hex = |byte: &u8| matches!(byte, b'0'..=b'9' | b'a'..=b'f');
    if (7..=64).contains(&commit.len()) && commit.as_bytes().iter().all(hex) {
        return Ok(format!("{commit}.json"));
    }
    Err(format!(
        "`{commit}` is not a commit this reporter will name a file after — a \
         record file is named from `argv` and a name is data, so it is lowercase \
         hex of 7 to 64 characters or it is refused"
    ))
}

/// Write one commit's record, replacing whatever was there for that commit.
///
/// REPLACED RATHER THAN APPENDED, and that loses nothing: a later reading of one
/// commit is strictly better informed than an earlier one, because the jobs that
/// had not finished have, and a job that has finished never changes what it took.
/// Two rows for one commit would be two points in every series for a commit that
/// ran once.
///
/// WRITTEN BESIDE AND RENAMED INTO PLACE. A record half-written by a process that
/// died is a file the reader below has to name as unreadable, on every push, until
/// somebody deletes it; a rename is atomic on the one filesystem this ever writes
/// to. The temporary name carries this process's id — the rule `unowned-scratch`
/// enforces on anything this repository leaves in a directory it shares — and it
/// does not end in `.json`, so a reader never sees a partial record even in the
/// window before the rename.
pub fn keep(tree: &Path, kept: &Kept) -> Result<PathBuf, String> {
    let name = record_name(&kept.commit)?;
    let directory = records_in(tree);
    fs::create_dir_all(&directory)
        .map_err(|why| format!("{} could not be created: {why}", directory.display()))?;
    let raw = serde_json::to_string_pretty(kept)
        .map_err(|why| format!("a record of {} would not serialise: {why}", kept.commit))?;
    let beside = directory.join(format!("{name}.{}", std::process::id()));
    fs::write(&beside, raw).map_err(|why| format!("{} unwritable: {why}", beside.display()))?;
    let at = directory.join(&name);
    fs::rename(&beside, &at)
        .map_err(|why| format!("{} could not be put in place: {why}", at.display()))?;
    Ok(at)
}

/// Every record this tree holds, oldest run first, and everything that would not
/// read as one.
///
/// A RECORD THAT WILL NOT READ IS NAMED AND THE REST ARE STILL READ. One bad file
/// must not take a repository's whole history down with it, and it must not be
/// skipped in silence either — a directory that quietly drops half its rows is a
/// trend computed over a window nobody chose.
///
/// THE FILENAME AND THE RECORD HAVE TO AGREE. A copied file is a well-formed
/// answer about another commit, which is the shape R1122 paid for in the
/// neighbouring gate and the reason the census here compares the two.
#[must_use]
pub fn kept_in(tree: &Path) -> (Vec<Kept>, Vec<String>) {
    let directory = records_in(tree);
    let mut kept: Vec<Kept> = Vec::new();
    let mut trouble: Vec<String> = Vec::new();
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        // NOT AN ERROR: a tree nothing has reported on yet has no such directory,
        // and every clone is in that state on its first push.
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => return (kept, trouble),
        Err(why) => {
            trouble.push(format!(
                "{} could not be listed, so nothing is known about what earlier \
                 pushes cost: {why}",
                directory.display()
            ));
            return (kept, trouble);
        }
    };
    for entry in entries {
        let path = match entry {
            Ok(entry) => entry.path(),
            Err(why) => {
                trouble.push(format!("a record in {} would not read: {why}", RECORDS));
                continue;
            }
        };
        // Anything that is not a record is passed over, which is what makes the
        // write above safe: the file it renames from ends in a process id.
        if path.extension().is_none_or(|end| end != "json") {
            continue;
        }
        match read_record(&path) {
            Ok(one) => kept.push(one),
            Err(why) => trouble.push(why),
        }
    }
    kept.sort_by(|left, right| {
        epoch_seconds(&left.ran_at)
            .cmp(&epoch_seconds(&right.ran_at))
            .then_with(|| left.commit.cmp(&right.commit))
    });
    (kept, trouble)
}

/// One record file, judged.
fn read_record(path: &Path) -> Result<Kept, String> {
    let raw =
        fs::read_to_string(path).map_err(|why| format!("{} unreadable: {why}", path.display()))?;
    let kept: Kept = serde_json::from_str(&raw).map_err(|why| {
        format!(
            "{} is not a record of what a commit cost: {why}",
            path.display()
        )
    })?;
    let named = path.file_stem().unwrap_or_default().to_string_lossy();
    if named != kept.commit {
        return Err(format!(
            "{} holds a record of {} — a record named after one commit and \
             describing another is a point this reporter cannot place",
            path.display(),
            short(&kept.commit)
        ));
    }
    if epoch_seconds(&kept.ran_at).is_none() {
        return Err(format!(
            "{} is stamped `{}`, which this reader cannot order against anything",
            path.display(),
            kept.ran_at
        ));
    }
    Ok(kept)
}

/// A place in one job's completed series where every run before was cheaper than
/// every run after — a STEP, and not a slope.
///
/// WHY THE DISTINCTION IS THE WHOLE POINT. A movement between the ENDS of a
/// window reads as a climb, and a reader who reads it that way extrapolates: so
/// many points per commit, so many commits until the budget. If the window is two
/// LEVELS with a jump between them, that arithmetic is about a line nothing is
/// on. The question stops being "when does it reach the budget" and becomes "what
/// changed at that commit", which is a different round.
///
/// R1270 MEASURED WHAT NOT SAYING WHICH COSTS. `validate` reads 24% → 46% across
/// 24 completed runs, and R1268's ledger wrote that down as a climb of eleven
/// points in one commit. It is neither: the first four runs are 24–27%, the
/// twenty after them are 33–48%, and the level has not moved in three days. The
/// eleven points were the gap between two adjacent runs of a job whose adjacent
/// runs differ by that much routinely.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// The commit of the OLDEST run on the expensive side — the place to look.
    pub at: String,
    /// When that run went.
    pub when: String,
    /// How many completed runs sit below the split.
    pub before: usize,
    /// The HIGHEST share among them.
    pub below: u64,
    /// How many sit above it.
    pub after: usize,
    /// The LOWEST share among those.
    pub above: u64,
}

/// How many completed runs a side needs before it is a LEVEL rather than a
/// couple of points.
///
/// TWO WOULD BE ENOUGH TO SATISFY THE ARITHMETIC AND IS NOT ENOUGH TO MEAN
/// ANYTHING. "Every run before is cheaper than every run after" is trivially true
/// of a series whose first measurement happened to be its lowest, and this
/// record's own jobs move by eight to thirteen points between adjacent commits —
/// so a two-run side is a claim about noise. Three is the smallest side that
/// cannot be one accident, and it is stated here rather than buried in a
/// comparison because it is the whole of what this detector is willing to assert.
const A_LEVEL: usize = 3;

/// The one place a job's completed series splits into two levels, if there is
/// exactly one.
///
/// A CLEAN SPLIT is an index where every share before it is strictly below every
/// share after it, with at least [`A_LEVEL`] runs on each side.
///
/// EXACTLY ONE, AND THAT IS THE WHOLE DISCRIMINATOR AGAINST A SLOPE. A series
/// that climbs steadily is clean at EVERY index — 10, 20, 30, 40, 50, 60, 70
/// splits anywhere you cut it — so "there is a clean split" on its own would
/// report the steadiest climb in the repository as a step and send its reader to
/// one arbitrary commit to look for a cause spread across all of them. A step is
/// a series that separates in ONE place and nowhere else.
///
/// Measured against this repository's own record, which is what says the rule is
/// not vacuous in either direction: of the ten jobs it holds, nine have no clean
/// split at all and `validate` has exactly one.
///
/// `None` is the ordinary answer and means the movement between the ends is what
/// it looks like — the reader has the range and the jitter beside it either way.
#[must_use]
pub fn step_in(shares: &[u64]) -> Option<usize> {
    let mut only = None;
    for at in A_LEVEL..=shares.len().saturating_sub(A_LEVEL) {
        let (before, after) = shares.split_at(at);
        let (Some(below), Some(above)) = (before.iter().max(), after.iter().min()) else {
            continue;
        };
        if below >= above {
            continue;
        }
        if only.is_some() {
            // A SECOND CLEAN SPLIT AND THE ANSWER IS NO. Whether this series is a
            // staircase or a slope, it does not separate in one place, and this
            // reporter says nothing rather than picking one of them.
            return None;
        }
        only = Some(at);
    }
    only
}

/// The largest move between adjacent entries, or zero when there is no pair.
fn largest_adjacent_move(shares: &[u64]) -> u64 {
    shares
        .windows(2)
        .map(|pair| pair[1].abs_diff(pair[0]))
        .max()
        .unwrap_or(0)
}

/// Where one job's share of its budget has been across the record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Movement {
    /// The check name, which is what a job's history is keyed by. A RENAMED JOB
    /// STARTS A NEW SERIES, and that is honest rather than a gap: nothing in
    /// GitHub's answer says the new name is the old job, and a reporter that
    /// guessed would draw a trend across two different pieces of work.
    pub check: String,
    /// The oldest measurement of this job in the record.
    pub first: Spent,
    /// The newest.
    pub last: Spent,
    /// The lowest and highest share seen across the window, so a movement between
    /// the ends can be read against the noise around it. A runner having a slow
    /// day moves this number and nothing about the job did.
    ///
    /// HELD AGAINST THE BUDGET THE JOB DECLARES NOW, like every other share in
    /// this type — see [`Movement::points`].
    pub low: u64,
    pub high: u64,
    /// How many commits ran this job — ITS OWN window, not the record's. A job
    /// added last week has a short history inside a long record, and a movement
    /// measured from the record's oldest commit would be measured from a commit
    /// this job never ran on.
    pub commits: usize,
    /// When the oldest of them ran.
    pub since: String,
    /// Every distinct budget this job has declared across the window.
    pub budgets: BTreeSet<u64>,
    /// How many recorded runs of this job did NOT run all their steps, and are
    /// therefore no part of the movement above (R1261).
    ///
    /// COUNTED RATHER THAN DROPPED, and the sentence prints it. A job that fails
    /// on half the commits it runs on has a movement drawn over the other half,
    /// and a reader who is not told that is reading a curve over a population
    /// they did not choose.
    pub set_aside: usize,
    /// The largest move between ADJACENT completed runs — how much this job
    /// swings on its own, from one commit to the next.
    ///
    /// THE NUMBER THAT SAYS WHICH MOVEMENTS ARE NEWS. R1268 read `validate`
    /// gaining eleven points between two pushes as a cost that was running away;
    /// this job moves by as much as thirteen between adjacent commits while
    /// changing nothing, and `every test compiled is one CI runs` swings twenty
    /// while beginning and ending the window at four per cent. A movement
    /// smaller than this number is the job's noise and not its trend, and the
    /// only way a reader could know that was to go and read thirty JSON files.
    pub jitter: u64,
    /// Where the completed series splits into two levels, when it does — see
    /// [`Step`]. `None` means the movement between the ends is what it looks
    /// like, which is the ordinary answer.
    pub step: Option<Step>,
    /// The furthest any of those set-aside runs got, as a share of the budget the
    /// job declares now.
    ///
    /// THE CEILING SURVIVES ITS EXCLUSION. `validate` on `cabcd5cf` is 90m02s of
    /// its 90 minutes — its own budget ending it — and that is the single most
    /// important row in this repository's record while being no part of any cost
    /// curve. Dropping it silently would take the one number a reader most needs
    /// out of the only sentence that looks back.
    pub reached: u64,
}

impl Movement {
    /// The budget every share in this movement is held against — the one the job
    /// declares now.
    #[must_use]
    pub fn against(&self) -> u64 {
        self.last.budget_minutes
    }

    /// What the oldest measurement of this job would be as a share of the budget
    /// it declares today.
    #[must_use]
    pub fn first_share(&self) -> u64 {
        share_of(self.first.took, self.against())
    }

    /// And the newest, which is the share the level line printed.
    #[must_use]
    pub fn last_share(&self) -> u64 {
        share_of(self.last.took, self.against())
    }

    /// The change in this job's cost, in points of the budget it declares now.
    ///
    /// ONE DENOMINATOR FOR BOTH ENDS, WHICH IS THE WHOLE OF WHY THIS IS NOT
    /// `last.percent() - first.percent()`. Raising a job's `timeout-minutes` from
    /// 90 to 120 drops its raw share from 44% to 33% without making the job one
    /// second cheaper, and a trend built from raw shares would print that as the
    /// steepest FALL in the repository. Holding both durations against the budget
    /// in force today takes the denominator out of the comparison and leaves the
    /// question this file exists for: is the job costing more than it used to.
    ///
    /// TOTAL ARITHMETIC over numbers that come off DISK. A share is computed from
    /// a duration a record carries, and a record is a file somebody can edit.
    #[must_use]
    pub fn points(&self) -> i64 {
        let (first, last) = (
            i128::from(self.first_share()),
            i128::from(self.last_share()),
        );
        i64::try_from(last - first).unwrap_or(i64::MAX)
    }

    /// Whether every measurement in this window was held against the same budget.
    ///
    /// WHEN THIS IS FALSE THE SENTENCE SAYS SO, because the numbers a reader will
    /// have seen on those pushes are the raw shares and they do not match the
    /// comparison above — the reporter printed 44% in June and 34% today, and the
    /// job got more expensive between them.
    #[must_use]
    pub fn budget_steady(&self) -> bool {
        self.budgets.len() == 1
    }
}

/// One [`Movement`] per job in the record, by name.
///
/// THE INPUT IS ALREADY IN ORDER — [`kept_in`] sorts by the run's own stamp — and
/// this walks it once, so "first" and "last" here mean oldest-run and newest-run
/// rather than whatever order a directory listing came back in.
///
/// AND THE POPULATION IS THE RUNS THAT COMPLETED (R1261). A duration is a fact
/// about every job that ran; a COST is a fact only about one that ran all of its
/// steps, and a curve drawn through both is a curve through two different
/// quantities. What is set aside is counted and its furthest reach is carried, so
/// the exclusion is a sentence rather than a shorter list — see
/// [`Movement::set_aside`] and [`Movement::reached`].
#[must_use]
pub fn movements(kept: &[Kept]) -> Vec<Movement> {
    // THE COMMIT TRAVELS WITH THE POINT since R1270, because a step is a PLACE:
    // "the level rose here" is only worth printing if the reader is told where
    // here is, and a timestamp is not something anybody can go and read.
    let mut series: BTreeMap<&str, Vec<(&str, &str, &Spent)>> = BTreeMap::new();
    for one in kept {
        for job in &one.jobs {
            series.entry(job.check.as_str()).or_default().push((
                one.commit.as_str(),
                one.ran_at.as_str(),
                job,
            ));
        }
    }
    series
        .into_iter()
        .filter_map(|(check, rows)| {
            // THE BUDGET COMES OFF THE NEWEST ROW OF ALL, completed or not: it is
            // what the job DECLARES now, which a run that failed declares just as
            // well as one that passed. Reading it off the newest completed run
            // instead would hold today's durations against a budget the workflow
            // may have changed since.
            let (_, _, newest) = *rows.last()?;
            let against = newest.budget_minutes;
            let (ran, stopped): (Vec<_>, Vec<_>) = rows
                .iter()
                .partition(|(_, _, one)| one.conclusion == RAN_TO_COMPLETION);
            let (_, since, first) = *ran.first()?;
            let (_, _, last) = *ran.last()?;
            let shares: Vec<u64> = ran
                .iter()
                .map(|(_, _, one)| share_of(one.took, against))
                .collect();
            // THE STEP IS DRAWN OVER THE SAME POPULATION AS THE MOVEMENT — the
            // runs that completed. A duration is a fact about every run; a COST
            // is a fact only about one that ran all its steps, and a level built
            // from both would be a level over two different quantities.
            let step = step_in(&shares).map(|at| Step {
                at: ran[at].0.to_string(),
                when: ran[at].1.to_string(),
                before: at,
                below: shares[..at].iter().copied().max().unwrap_or(0),
                after: shares.len() - at,
                above: shares[at..].iter().copied().min().unwrap_or(0),
            });
            Some(Movement {
                check: check.to_string(),
                first: first.clone(),
                last: last.clone(),
                low: shares.iter().copied().min()?,
                high: shares.iter().copied().max()?,
                commits: ran.len(),
                since: since.to_string(),
                budgets: ran.iter().map(|(_, _, one)| one.budget_minutes).collect(),
                jitter: largest_adjacent_move(&shares),
                step,
                set_aside: stopped.len(),
                reached: stopped
                    .iter()
                    .map(|(_, _, one)| share_of(one.took, against))
                    .max()
                    .unwrap_or(0),
            })
        })
        .collect()
}

/// What a movement leaves out, as the clause that follows it.
///
/// EMPTY WHEN NOTHING WAS LEFT OUT, and a sentence the moment anything was. A
/// curve drawn over eleven of a job's twenty recorded runs is a curve over a
/// population the reader did not choose, and the number that says so has to sit
/// beside the number it qualifies rather than in a block below it.
fn set_aside_clause(movement: &Movement) -> String {
    if movement.set_aside == 0 {
        return String::new();
    }
    format!(
        "; {} more did not complete, the longest reaching {}%",
        movement.set_aside, movement.reached
    )
}

/// How much the job swings on its own, as the clause that follows the movement.
///
/// EMPTY UNTIL THERE ARE THREE COMPLETED RUNS, and that is not a threshold for
/// tidiness. With two runs there is ONE adjacent pair, so the largest move
/// between adjacent commits IS the movement between the ends — the same number
/// printed twice, the second time wearing the word "own", which would tell a
/// reader that the only measurement in the window is noise.
fn jitter_clause(movement: &Movement) -> String {
    if movement.commits < 3 {
        return String::new();
    }
    format!(
        "; adjacent commits move it by as much as {} point(s) on their own",
        movement.jitter
    )
}

/// What a movement is SHAPED like, as the clause that follows it.
///
/// EMPTY WHEN THE SERIES DOES NOT SEPARATE, which is the ordinary case and the
/// one where the movement between the ends is what it looks like. When it does
/// separate, this is the difference between a reader extrapolating a line and a
/// reader going to look at a commit — see [`Step`].
fn step_clause(movement: &Movement) -> String {
    let Some(step) = &movement.step else {
        return String::new();
    };
    format!(
        "; and that movement is a STEP rather than a climb — the {} completed run(s) before \
         {} ({}) are all at or below {}% and the {} from there on are all at or above {}%, \
         and the series separates nowhere else, so what to ask is what changed at that \
         commit rather than when the budget is reached",
        step.before,
        short(&step.at),
        step.when,
        step.below,
        step.after,
        step.above
    )
}

/// How one job's history reads as a sentence.
#[must_use]
pub fn movement_line(movement: &Movement) -> String {
    let commits = movement.commits;
    let aside = format!(
        "{}{}{}",
        jitter_clause(movement),
        step_clause(movement),
        set_aside_clause(movement)
    );
    if movement.budget_steady() {
        return format!(
            "`{}` {}% → {}% ({:+} points over {commits} commit(s) it completed; {}–{}% \
             across them{aside})",
            movement.check,
            movement.first_share(),
            movement.last_share(),
            movement.points(),
            movement.low,
            movement.high
        );
    }
    format!(
        "`{}` {} → {} over {commits} commit(s) it completed, against {} distinct budget(s) \
         ({}m → {}m) — held against the {}m it declares now that is {}% → {}% ({:+} points), \
         where the shares printed at the time were {}% and {}%{aside}",
        movement.check,
        clock(movement.first.took),
        clock(movement.last.took),
        movement.budgets.len(),
        movement.first.budget_minutes,
        movement.last.budget_minutes,
        movement.against(),
        movement.first_share(),
        movement.last_share(),
        movement.points(),
        movement.first.percent(),
        movement.last.percent()
    )
}

/// Every job in the record with no completed run at all, by name.
///
/// A JOB THAT HAS ONLY EVER FAILED HAS NO COST CURVE, and [`movements`] therefore
/// produces nothing for it — which would make it vanish from this block without a
/// word. It is the one case where "no trend" is itself the news.
#[must_use]
pub fn never_completed(kept: &[Kept]) -> BTreeSet<String> {
    let mut all: BTreeSet<&str> = BTreeSet::new();
    let mut completed: BTreeSet<&str> = BTreeSet::new();
    for one in kept {
        for job in &one.jobs {
            all.insert(job.check.as_str());
            if job.conclusion == RAN_TO_COMPLETION {
                completed.insert(job.check.as_str());
            }
        }
    }
    all.difference(&completed)
        .map(|check| (*check).to_string())
        .collect()
}

/// What the record says, beside the level [`crate::budget_report`] just printed.
///
/// THE JOB THE LEVEL NAMED IS THE JOB THE TREND FOLLOWS. "The worst job" is a
/// RANK and it is not a subject: `validate` today and `evidence-replay` yesterday
/// are two jobs, and a series built from whichever was worst on each commit
/// compares neither of them to itself. `closest` is a name, and the history it is
/// looked up in is that name's.
///
/// AND THE STEEPEST RISE IS PRINTED EVEN WHEN IT IS SOMEWHERE ELSE. A job flat at
/// forty-four per cent is not news; a job that went from twelve to thirty-one is,
/// and it is nowhere near the budget that would have made the level line name it.
/// That is the whole difference between the two questions this file separates.
#[must_use]
pub fn trend_report(kept: &[Kept], closest: Option<&str>) -> Vec<String> {
    let Some(oldest) = kept.first() else {
        // NO LINE, AND THIS IS THE ONE PLACE SILENCE IS RIGHT: an empty record
        // means no job of any commit was ever measured here, which the block
        // above has just said in its own words. A second sentence saying it again
        // is noise on every push in a tree that has no workflow.
        return Vec::new();
    };
    if kept.len() == 1 {
        return vec![format!(
            "1 commit is recorded here ({}), so what these jobs cost is a level and not yet \
             a trend",
            short(&oldest.commit)
        )];
    }
    let movements = movements(kept);
    let mut lines = vec![format!(
        "against {} commit(s) recorded here, oldest {}:",
        kept.len(),
        oldest.ran_at
    )];
    let unfinished = never_completed(kept);
    match closest.and_then(|name| movements.iter().find(|one| one.check == name)) {
        Some(one) if one.commits > 1 => lines.push(format!("  {}", movement_line(one))),
        Some(one) => lines.push(format!(
            "  `{}` has completed on 1 commit only, so the job closest to its budget has no \
             trend yet",
            one.check
        )),
        // NOT SILENCE WHEN THE JOB IS ONE THAT HAS NEVER COMPLETED (R1261): the
        // level line has just named it, and a block that then says nothing about
        // it reads as a job with an unremarkable history rather than one with no
        // history at all.
        None => {
            if let Some(name) = closest.filter(|name| unfinished.contains(*name)) {
                lines.push(format!(
                    "  `{name}` has no completed run in this record, so there is nothing to \
                     compare what it cost against"
                ));
            }
        }
    }
    let steepest = movements
        .iter()
        .filter(|one| one.commits > 1)
        .max_by_key(|one| one.points());
    match steepest {
        Some(one) if one.points() > 0 && Some(one.check.as_str()) != closest => {
            lines.push(format!(
                "  and the steepest rise is not that job — {}",
                movement_line(one)
            ))
        }
        // The steepest rise IS the job the line above already followed; saying so
        // twice would be the same sentence with a different preamble.
        Some(one) if one.points() > 0 => {}
        _ => lines.push(
            "  no job's share of its budget is above where it was first recorded".to_string(),
        ),
    }
    // AND THE JOBS WITH NO CURVE AT ALL ARE COUNTED (R1261). One of them may
    // already have been named above, because the level line pointed at it; the
    // rest would otherwise be absent from a block that reads as covering every
    // job in the record.
    let uncurved = unfinished
        .iter()
        .filter(|name| Some(name.as_str()) != closest)
        .count();
    if uncurved > 0 {
        lines.push(format!(
            "  {uncurved} further job(s) in this record have no completed run, so no \
             movement is drawn for them"
        ));
    }
    lines
}

/// Keep what this commit cost, and say what the record makes of it.
///
/// THE DECISION LIVES HERE AND NOT IN `main.rs` (R1096, and R1129's three gates):
/// write, then read, then report — in that order, so the commit being pushed onto
/// is a point in its own trend rather than a number beside one. A `main.rs` that
/// held this would be a decision nothing runs.
///
/// A RECORD THAT COULD NOT BE WRITTEN IS A LINE. Every other refusal in this
/// reporter is, and a push that quietly failed to keep its number would be
/// indistinguishable from one that kept it — until the trend it was building
/// turned out to be one point long.
///
/// AND THE UNREADABLE ONES ARE NAMED UP TO A CAP THAT SAYS WHAT IT LEFT OUT
/// (R1261), which is the rule the annotation block one file over already follows.
/// The shape of a record is a type, so the day it changes EVERY record in the
/// directory becomes unreadable at once — thirty-three of them here — and a
/// hundred lines of the same sentence is a block a reader learns to skip. One
/// unreadable record is a fact worth a line; thirty are one fact.
const NAMED_TROUBLE: usize = 3;

#[must_use]
pub fn kept_report(tree: &Path, commit: &str, checks: &[Check], spent: &[Spent]) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(kept) = Kept::of(commit, checks, spent) {
        if let Err(why) = keep(tree, &kept) {
            lines.push(format!(
                "NOTE what {} cost was measured and not kept — {why}",
                short(commit)
            ));
        }
    }
    let (kept, trouble) = kept_in(tree);
    lines.extend(
        trouble
            .iter()
            .take(NAMED_TROUBLE)
            .map(|why| format!("NOTE {why}")),
    );
    if trouble.len() > NAMED_TROUBLE {
        lines.push(format!(
            "NOTE (+{} more record(s) in {RECORDS} would not read — a whole directory that \
             stopped reading at once is a record shape that changed, and reporting on those \
             commits again rewrites them)",
            trouble.len() - NAMED_TROUBLE
        ));
    }
    lines.extend(trend_report(
        &kept,
        crate::closest_to_budget(spent).map(|one| one.check.as_str()),
    ));
    lines
}
