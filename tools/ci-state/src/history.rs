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
//! AND A JOB'S ROWS ARE TWO POPULATIONS, SO THERE ARE TWO READERS. R1261 split
//! them — a duration is a fact about every run, a COST is a fact only about one
//! that ran all of its steps — and wrote only the first reader, keeping the second
//! population as a count and a maximum inside it. That is why nothing here could
//! answer WHERE a job's failures land: four minutes in is a defect that fails
//! fast and says nothing about cost, forty minutes in is a job that did a passing
//! run's work and then threw it away, and both printed as `2 more did not
//! complete`. [`crate::history::Landing`] is that reader, and the split it reads is
//! spelled once (`series_in`) rather than once per question.
//!
//! THE TWO LINKS ABOVE ARE CRATE-QUALIFIED AND THE THIRD IS NOT A LINK AT ALL,
//! which is a fact about where a `mod` item's documentation is RESOLVED rather than
//! a style. This block documents `history`, the item is declared in `lib.rs`, and
//! rustdoc resolves its links in the scope that declaration sits in — the crate
//! root. That is why `[Spent::percent]` above resolves (`Spent` is a root item) and
//! why an unqualified `[Landing]` does not, though it is defined in this very file:
//! the gate caught both, and the message `no item named Landing in scope` reads as
//! a typo until that is known. `series_in` is private, so a link to it would resolve
//! only under `--document-private-items` — which is what the gate passes and not
//! what a plain `cargo doc` does, so it is named in prose instead.
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

/// How many recent commits a job may stay quiet over before that is a finding.
///
/// READ OFF THE RECORD, NOT CHOSEN (R1304). At the moment this was written, the
/// 67 records in this tree answered: over the last FIFTEEN commits every job a
/// workflow runs unconditionally had concluded at least once, and over the last
/// TEN, three had not — `every cache declared is one CI keeps`, `every
/// compilation is one job's` and `what this run reads outside this tree`. Ten is
/// therefore the window where the answer stops being empty and starts being
/// three jobs nobody had a verdict from, and fifteen is where it goes quiet
/// again. A number picked before that distribution was known would be a number
/// somebody liked.
pub const QUIET_FOR: usize = 10;

/// The jobs a run should have judged that no recent commit has a verdict from.
///
/// A JOB THAT IS ALWAYS CANCELLED READS EXACTLY LIKE A JOB THAT ALWAYS PASSES,
/// and that is the hole this closes. `cancelled` is not red, so the census says
/// nothing about it and the walk carries nothing from it; a workspace whose only
/// judge is such a job is ungated in practice, which is how a law that FAILED on
/// live code sat on `main` for two rounds.
///
/// `expected` IS WHAT THE WORKFLOWS DECLARE UNCONDITIONALLY, asked of `ci-plan`
/// rather than derived from the record itself. Deriving it from the record would
/// make a job that has never once concluded invisible — it would not be in the
/// population — which is the exact reading this is for.
///
/// THE RECORD HOLDS ONLY WHAT CONCLUDED. `Kept::of` records a job whose cost
/// could be held against a budget, and a cancelled or still-running job has no
/// such cost, so absence from the window IS the absence of a verdict.
#[must_use]
pub fn quiet_jobs(kept: &[Kept], expected: &[String], window: usize) -> Vec<String> {
    let heard: BTreeSet<&str> = kept
        .iter()
        .rev()
        .take(window)
        .flat_map(|one| one.jobs.iter().map(|job| job.check.as_str()))
        .collect();
    expected
        .iter()
        .filter(|job| !heard.contains(job.as_str()))
        .cloned()
        .collect()
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
    /// The commit of the OLDEST COMPLETED run on the expensive side — where the
    /// LEVEL splits.
    ///
    /// NOT NECESSARILY WHERE THE COST ROSE, which is what R1275 measured and what
    /// [`Step::rose`] carries. This field is a fact about the cost curve, and the
    /// cost curve is drawn over completed runs only; the runs a rise lands on are
    /// disproportionately runs that did NOT complete, because a change that makes
    /// a job dearer is often the change that breaks it.
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
    /// The oldest run of ANY conclusion from which the expensive level has held
    /// unbroken up to the split — THE PLACE TO LOOK.
    ///
    /// EQUAL TO THE SPLIT IN THE ORDINARY CASE, and this repository's own record
    /// is not the ordinary case: `validate` splits at `4eccb3d4` and had already
    /// reached the expensive level two runs earlier, on `a300556` — R1229, the
    /// commit whose own round left a comment saying it "changed the work of a
    /// job's longest step". The two runs between are a cancellation and a
    /// failure, so neither is on the cost curve, and a reader sent to the split
    /// is a reader reading a diff that contains no cause. See [`Step::earlier_by`].
    pub rose: Run,
    /// How many recorded runs sit between the rise and the split — `0` when the
    /// split IS the rise, which is what makes the distinction printable without
    /// a second field saying whether it applies.
    pub earlier_by: usize,
    /// The last run below the expensive level: the other end of any comparison.
    ///
    /// `None` ONLY WHEN THE RISE IS THE OLDEST ROW OF THE RECORD, which a step
    /// cannot ordinarily be — [`A_LEVEL`] completed runs sit below the split — but
    /// which a hand-edited record can be, and a reporter that indexed backwards
    /// from a bound it had only argued for would panic on a push.
    pub under: Option<Run>,
}

/// One recorded run of one job: a commit to go and read, when it went, and what
/// the job took there.
///
/// A TYPE BECAUSE A PLACE TO LOOK IS THREE FACTS AND NOT ONE. R1270 printed a
/// commit, which sends a reader to a diff; the duration beside it is what tells
/// them whether the diff they are reading accounts for the whole move or a tenth
/// of it, and the stamp is what lets them find the run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    /// The commit this run went on.
    pub commit: String,
    /// When it went.
    pub when: String,
    /// What the job took, in seconds.
    pub took: u64,
}

impl Run {
    /// One recorded row, as a place to look.
    fn of((commit, when, spent): Row<'_>) -> Self {
        Self {
            commit: commit.to_string(),
            when: when.to_string(),
            took: spent.took,
        }
    }
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

/// Where the expensive level of a step actually BEGINS, read over every run
/// rather than over the completed ones.
///
/// THE DEFECT THIS CLOSES IS R1270'S OWN OUTPUT (N198). That round built the
/// detector, named `validate`'s split at `4eccb3d4`, and printed "what to ask is
/// what changed at that commit". Reading `4eccb3d4` answers nothing: it is a
/// change to one gate's path handling. The job's `cargo test --workspace` step
/// went from 1276 s to 1772 s two runs EARLIER, on `a300556`, and the two runs
/// between are a cancellation and a failure — which is exactly why the split
/// could not see them. The completed population is the right population for a
/// LEVEL and the wrong one for a BEGINNING, and the two questions had one answer.
///
/// AND THE WALK IS CONTIGUOUS RATHER THAN A SEARCH FOR THE EARLIEST EXPENSIVE
/// RUN, which is the whole of what keeps it from lying. A job that hung once in
/// its cheap era has one old row at 100% of its budget, and "the earliest run at
/// or above the expensive level" would name that row and send a reader to a
/// commit three weeks before anything changed. What is asserted here is narrower
/// and is what a reader acts on: the level has held from HERE, without a single
/// run below it since.
///
/// Answers the pair `(the rise, the last run under it)`. The second is `None`
/// only when the rise is the oldest row this record holds.
fn rise_of(all: &[Row<'_>], against: u64, split: &str, above: u64) -> (Option<Run>, Option<Run>) {
    let Some(split_at) = all.iter().position(|(commit, _, _)| *commit == split) else {
        // A SPLIT THAT IS NOT IN THE POPULATION IT WAS DRAWN FROM cannot arise
        // from `movements`, which takes both from one `Series`. It is total
        // because the alternative is an index this function argued for.
        return (None, None);
    };
    let mut first = split_at;
    while first > 0 && share_of(all[first - 1].2.took, against) >= above {
        first -= 1;
    }
    let under = first.checked_sub(1).map(|at| Run::of(all[at]));
    (Some(Run::of(all[first])), under)
}

/// The largest move between adjacent entries, or zero when there is no pair.
fn largest_adjacent_move(shares: &[u64]) -> u64 {
    shares
        .windows(2)
        .map(|pair| pair[1].abs_diff(pair[0]))
        .max()
        .unwrap_or(0)
}

/// One run of one job, as the record holds it: the commit it ran on, when that
/// run began, and what the job took.
type Row<'a> = (&'a str, &'a str, &'a Spent);

/// One job's rows, split into the two populations the record holds.
///
/// THE SPLIT IS SPELLED ONCE. R1261 established it — a duration is a fact about
/// every run that happened, a COST is a fact only about one that ran all of its
/// steps — and then wrote the comparison `conclusion == RAN_TO_COMPLETION` in each
/// place that needed it. Three readers now ask about these two populations
/// ([`movements`], [`Landing`], [`never_completed`]) and a fourth is a matter of
/// time, so the partition is here and each of them takes a side of it.
struct Series<'a> {
    /// The check name, which is what a job's history is keyed by.
    check: &'a str,
    /// Every row of this job, oldest run first — the population the two below
    /// are a partition of.
    ///
    /// KEPT BESIDE THE PARTITION RATHER THAN REBUILT FROM IT (R1275). A DURATION
    /// is a fact about every run, and the question "when did this job start
    /// costing this much" is a duration question even though the LEVEL either
    /// side of it is a cost. Merging `completed` and `stopped` back together at
    /// the point of asking would be a second spelling of the order this function
    /// already established, and the two would disagree the first time two runs
    /// shared a stamp.
    all: Vec<Row<'a>>,
    /// The rows that ran all of their steps, oldest run first.
    completed: Vec<Row<'a>>,
    /// The rows that did not, oldest run first.
    stopped: Vec<Row<'a>>,
    /// The budget every share of this job is held against: the NEWEST row's,
    /// completed or not, because it is what the job DECLARES now — which a run
    /// that failed declares just as well as one that passed.
    against: u64,
}

/// Every job in the record, split, one entry per name.
///
/// A JOB WITH NO ROW AT ALL CANNOT ARISE — a name is in this map because a row
/// carried it — so the only empty side possible is one of the two populations, and
/// both of those are real states this reporter has sentences for.
fn series_in(kept: &[Kept]) -> Vec<Series<'_>> {
    // THE COMMIT TRAVELS WITH THE POINT since R1270, because a step is a PLACE:
    // "the level rose here" is only worth printing if the reader is told where
    // here is, and a timestamp is not something anybody can go and read. R1274
    // needs it for the same reason on the other population — the run that got
    // furthest before stopping is a commit somebody may want to open.
    let mut rows: BTreeMap<&str, Vec<Row<'_>>> = BTreeMap::new();
    for one in kept {
        for job in &one.jobs {
            rows.entry(job.check.as_str()).or_default().push((
                one.commit.as_str(),
                one.ran_at.as_str(),
                job,
            ));
        }
    }
    rows.into_iter()
        .filter_map(|(check, rows)| {
            let (_, _, newest) = *rows.last()?;
            let (completed, stopped) = rows
                .iter()
                .copied()
                .partition(|(_, _, one)| one.conclusion == RAN_TO_COMPLETION);
            Some(Series {
                check,
                all: rows,
                completed,
                stopped,
                against: newest.budget_minutes,
            })
        })
        .collect()
}

/// Where the runs of one job that did NOT complete STOPPED, as shares of the
/// budget it declares now.
///
/// THE QUESTION THIS ANSWERS IS NOT WHAT THE JOB COSTS (R1274, closing what R1261
/// left open). A run that stopped has no place on a cost curve and it did happen:
/// the machine spent those minutes, and how many of them it spent before stopping
/// is the difference between two findings a reader acts on differently. `validate`
/// failing 331 s into a ninety-minute budget is a job that fell over before doing
/// any of its work — nothing about the budget is implicated. The same job failing
/// 2253 s in has done as much work as a run that PASSES sometimes does, and thrown
/// it away; and at 5415 s its own budget ended it. All three read as `did not
/// complete`, and until this type they printed as one count and one maximum.
///
/// IT IS ALSO THE SEPARATOR R1261 SAID WAS MISSING. A run a later push retired
/// never reaches this record — [`crate::spent_against_budgets`] sets those aside as
/// [`crate::Unmeasured::Retired`] — so a `cancelled` row HERE is a cancellation
/// that was not a supersession, and the only thing that tells a job its timeout
/// killed from one somebody stopped by hand is how near the budget it got. That is
/// this reader's whole output.
///
/// THE SHARES ARE THE POPULATION AND EVERYTHING ELSE IS DERIVED from them, so
/// there is no stored minimum to come to disagree with the list it was taken from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Landing {
    /// The share each of those runs reached, in the order the runs happened.
    pub shares: Vec<u64>,
    /// The commit of the furthest of them — a place to go and read.
    ///
    /// THE NEWEST WHERE SEVERAL REACHED THE SAME SHARE. A reader sent to look at
    /// why a job dies at its budget wants the most recent time it did, and the
    /// older ones are in the record beside it either way.
    pub furthest_at: String,
}

impl Landing {
    /// What the runs that stopped landed at, or nothing when none did.
    ///
    /// `None` RATHER THAN AN EMPTY ONE, because a job every run of which completed
    /// has no second population and a reader must not print a clause about it.
    fn of(stopped: &[Row<'_>], against: u64) -> Option<Self> {
        let (furthest_at, _, _) = stopped
            .iter()
            .max_by_key(|(_, _, one)| share_of(one.took, against))?;
        Some(Self {
            shares: stopped
                .iter()
                .map(|(_, _, one)| share_of(one.took, against))
                .collect(),
            furthest_at: (*furthest_at).to_string(),
        })
    }

    /// How many runs of this job did not complete.
    #[must_use]
    pub fn runs(&self) -> usize {
        self.shares.len()
    }

    /// The share the SOONEST of them stopped at.
    #[must_use]
    pub fn soonest(&self) -> u64 {
        self.shares.iter().copied().min().unwrap_or(0)
    }

    /// The share the FURTHEST of them reached.
    ///
    /// THE CEILING SURVIVES THE EXCLUSION. `validate` on `cabcd5cf` is 5415 s of
    /// its ninety minutes — its own budget ending it — and that is the single most
    /// important row in this repository's record while being no part of any cost
    /// curve. Dropping it silently would take the one number a reader most needs
    /// out of the only sentence that looks back.
    #[must_use]
    pub fn furthest(&self) -> u64 {
        self.shares.iter().copied().max().unwrap_or(0)
    }

    /// How many of them got at least as far as `floor`.
    ///
    /// THE FLOOR A CALLER PASSES IS THE CHEAPEST RUN THAT COMPLETED, which is the
    /// most conservative reading available: a run that stopped BELOW the cheapest
    /// pass cannot be called anything but an early failure, and one at or above it
    /// had already spent what a passing run sometimes spends. Holding it against
    /// the DEAREST pass instead would call four of `validate`'s six stoppages
    /// early, when they sit inside the band its passing runs occupy.
    #[must_use]
    pub fn past(&self, floor: u64) -> usize {
        self.shares.iter().filter(|share| **share >= floor).count()
    }
}

/// Where one job's runs that did not complete stopped, whether or not that job has
/// a cost curve.
///
/// THE JOB WITH NO CURVE IS THE ONE THIS MATTERS MOST FOR. [`movements`] produces
/// nothing for a job that has never completed — there is no cost to draw — so
/// before this the ONLY thing said about such a job was that there was nothing to
/// say. Where its runs stop is the whole of what is known about it.
fn landing_of(kept: &[Kept], check: &str) -> Option<Landing> {
    series_in(kept)
        .into_iter()
        .find(|series| series.check == check)
        .and_then(|series| Landing::of(&series.stopped, series.against))
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
    /// The recorded runs of this job that did NOT run all their steps, and are
    /// therefore no part of the movement above (R1261) — and WHERE they stopped
    /// (R1274). `None` when every recorded run of this job completed.
    ///
    /// CARRIED RATHER THAN COUNTED, and the sentence prints it. A job that fails
    /// on half the commits it runs on has a movement drawn over the other half,
    /// and a reader who is not told that is reading a curve over a population
    /// they did not choose. A reader told only how MANY is one who cannot tell a
    /// job that falls over in four minutes from a job that burns forty and then
    /// dies — see [`Landing`].
    pub landing: Option<Landing>,
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

/// One [`Movement`] per job in the record that has a cost curve, by name.
///
/// THE INPUT IS ALREADY IN ORDER — [`kept_in`] sorts by the run's own stamp — and
/// `series_in` walks it once, so "first" and "last" here mean oldest-run and
/// newest-run rather than whatever order a directory listing came back in.
///
/// AND THE POPULATION IS THE RUNS THAT COMPLETED (R1261). A duration is a fact
/// about every job that ran; a COST is a fact only about one that ran all of its
/// steps, and a curve drawn through both is a curve through two different
/// quantities. The runs that did not complete are CARRIED rather than dropped, so
/// the exclusion is a sentence rather than a shorter list, and where they stopped
/// is part of it — see [`Movement::landing`] and [`Landing`].
#[must_use]
pub fn movements(kept: &[Kept]) -> Vec<Movement> {
    series_in(kept)
        .into_iter()
        .filter_map(|series| {
            let Series {
                check,
                all,
                completed,
                stopped,
                against,
            } = series;
            let (_, since, first) = *completed.first()?;
            let (_, _, last) = *completed.last()?;
            let shares: Vec<u64> = completed
                .iter()
                .map(|(_, _, one)| share_of(one.took, against))
                .collect();
            // THE STEP IS DRAWN OVER THE SAME POPULATION AS THE MOVEMENT — the
            // runs that completed. A duration is a fact about every run; a COST
            // is a fact only about one that ran all its steps, and a level built
            // from both would be a level over two different quantities.
            //
            // AND WHERE IT BEGAN IS DRAWN OVER EVERY RUN (R1275). The level is a
            // cost and the beginning is a duration, and `rise_of` is the one
            // place that difference is spelled — see its own comment for the
            // record that made the distinction cost something.
            let step = step_in(&shares).map(|at| {
                let split = completed[at].0;
                let above = shares[at..].iter().copied().min().unwrap_or(0);
                let (rose, under) = rise_of(&all, against, split, above);
                let rose = rose.unwrap_or_else(|| Run::of(completed[at]));
                let earlier_by = all
                    .iter()
                    .position(|(commit, _, _)| *commit == split)
                    .zip(all.iter().position(|(commit, _, _)| *commit == rose.commit))
                    .map_or(0, |(split_at, rose_at)| split_at.saturating_sub(rose_at));
                Step {
                    at: split.to_string(),
                    when: completed[at].1.to_string(),
                    before: at,
                    below: shares[..at].iter().copied().max().unwrap_or(0),
                    after: shares.len() - at,
                    above,
                    rose,
                    earlier_by,
                    under,
                }
            });
            Some(Movement {
                check: check.to_string(),
                first: first.clone(),
                last: last.clone(),
                low: shares.iter().copied().min()?,
                high: shares.iter().copied().max()?,
                commits: completed.len(),
                since: since.to_string(),
                budgets: completed
                    .iter()
                    .map(|(_, _, one)| one.budget_minutes)
                    .collect(),
                jitter: largest_adjacent_move(&shares),
                step,
                landing: Landing::of(&stopped, against),
            })
        })
        .collect()
}

/// Where a job's runs that did not complete stopped, as the phrase both readers of
/// [`Landing`] share.
///
/// ONE RUN GETS `at` RATHER THAN A RANGE OF ONE, because `landing between 6% and
/// 6%` is a number printed twice wearing the clothes of a spread.
fn where_they_stopped(landing: &Landing) -> String {
    if landing.soonest() == landing.furthest() {
        return format!("landing at {}% of the budget", landing.furthest());
    }
    format!(
        "landing between {}% and {}% of the budget",
        landing.soonest(),
        landing.furthest()
    )
}

/// The share at which a run has used ALL of its budget, so what it wanted is not
/// what it took.
///
/// A HUNDRED IS EXACT HERE RATHER THAN A ROUND NUMBER. [`share_of`] floors, so a
/// run that used 99.6% of its budget answers 99 and only a run that used the whole
/// of it answers 100 — which makes this the boundary between a measurement and a
/// CENSORED one rather than a threshold anybody chose.
const AT_THE_BUDGET: u64 = 100;

/// What the furthest run wants, when the budget is what it ran out of.
///
/// A CENSORED MEASUREMENT IS A LOWER BOUND AND R1261 PRINTED IT AS A REACH. That
/// round said so in its own carry: `the longest reaching 100%` is true and it is
/// weaker than what happened — the job wanted MORE than its budget and nothing
/// knows how much more — and it left the shape unbuilt because inventing a form for
/// a fact this repository had met once would be a form for nothing.
///
/// IT HAS NOW BEEN MET TWICE, which is what took the argument away: `validate`
/// 5415 s into a 5400 s budget on `cabcd5cf`, and `the server targets the workspace
/// suite compiles to nothing` 2714 s into 2700 s. Both are jobs whose cost this
/// record cannot state at all, and a reader who takes `100%` for a measurement will
/// read a raised `timeout-minutes` as the fix and have no way to know whether it
/// was enough.
///
/// EMPTY FOR EVERY OTHER RUN, including one that stopped at 95% — where the share
/// IS the measurement and there is nothing censored about it.
fn censored_clause(landing: &Landing) -> &'static str {
    if landing.furthest() < AT_THE_BUDGET {
        return "";
    }
    ", which is the whole budget — so what that run wanted is at least all of it and \
     nothing here knows how much more"
}

/// What a movement leaves out AND WHERE THOSE RUNS STOPPED, as the clause that
/// follows it.
///
/// EMPTY WHEN NOTHING WAS LEFT OUT, and a sentence the moment anything was. A
/// curve drawn over eleven of a job's twenty recorded runs is a curve over a
/// population the reader did not choose, and the number that says so has to sit
/// beside the number it qualifies rather than in a block below it.
///
/// AND THE READING IS PRINTED, NOT LEFT AS ARITHMETIC (R1274). `5 of them at or
/// past 24%` is a fact and `not an early failure` is what a reader does with it;
/// the two halves of this repository's own record read in opposite directions —
/// `MSRV` stops at 17% of thirty minutes, below anything that passes, while four
/// of `validate`'s six stoppages sit inside the band its passing runs occupy — and
/// a sentence that gave only the count would leave the reader to do that
/// comparison against a floor that is not on the page.
fn landing_clause(movement: &Movement) -> String {
    let Some(landing) = &movement.landing else {
        return String::new();
    };
    let floor = movement.low;
    let past = landing.past(floor);
    let reading = if past == 0 {
        format!(
            "none of them got as far as the {floor}% the cheapest run that completed took, so \
             those are times to failure and say nothing about what the work costs"
        )
    } else {
        format!(
            "{past} of them at or past the {floor}% the cheapest run that completed took, so \
             what ended those is no early failure but a run that did a passing run's work and \
             then threw it away"
        )
    };
    format!(
        "; {} more did not complete, {} — {reading}, the furthest on {}{}",
        landing.runs(),
        where_they_stopped(landing),
        short(&landing.furthest_at),
        censored_clause(landing)
    )
}

/// Where the steps of one job on one commit come from.
///
/// A TRAIT AND NOT A `gh` CALL, for the reason this crate's `main.rs` header
/// gives: what is decided here has to be reachable by a test, and a process is
/// the one thing a test cannot reach. The implementation that talks to GitHub is
/// ten lines in `main.rs` and holds no decision; every case about WHICH step a
/// rise belongs to runs against a fixture.
///
/// WHAT IT COSTS, STATED RATHER THAN HIDDEN. Answering this means two `gh` calls
/// — the commit's check rows, then that job's steps — and the block below asks it
/// twice, for the two runs either side of a rise. Measured on this machine at
/// ~0.6 s a call, so ~2.4 s on a push that already spends minutes in clippy and
/// the workspace validate, and ONLY on a push whose record holds a step at all:
/// of the ten jobs in this repository's record, nine have no step and make no
/// call. A cache keyed by the pair was considered and left unbuilt — it would buy
/// two seconds and add a second record shape to keep working.
pub trait StepsOf {
    /// Every step of `check`'s job on `commit`, or why it could not be read.
    ///
    /// # Errors
    ///
    /// Whatever stopped the answer arriving: no such check on that commit, a run
    /// GitHub has aged out, a `gh` that cannot reach it. The caller prints it as
    /// a line — a step nobody could attribute is a fact, and falling silent there
    /// would read as a step with no cause inside the job.
    fn steps_of(&self, commit: &str, check: &str) -> Result<Vec<crate::Step>, String>;
}

/// What one step of a job did on ONE side of a rise.
///
/// THREE STATES BECAUSE TWO OF THEM ARE ROUTINELY CONFUSED and read in opposite
/// directions. A step ABSENT from a run's answer is a step the workflow did not
/// have yet — R1229 added one to `validate` and it is absent from every earlier
/// run — and a step that is present and NEVER RAN is one the job did not reach,
/// which is what every step after a stoppage is. Collapsing either into `0 s`
/// would let this block report a job's steps as having become free on the run
/// where the job died.
///
/// AND THE SECOND ONE IS NOT VISIBLE IN THE STAMPS, which R1275 found by running
/// this against the record rather than by reading GitHub's documentation. A
/// skipped step carries the start and end of the STOPPAGE, equal to each other,
/// so it reads as a step that took no time. Five steps of the cancelled run
/// either side of this repository's own rise read that way, and the first output
/// of this block reported `mnemosyne-cli validate-workspace` as ninety-six
/// seconds cheaper on the run where it did not run. [`crate::NEVER_RAN`] is the
/// only thing that says so.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// No step of that name is in that run's answer at all.
    Absent,
    /// A step of that name is there and did not run — GitHub's own `skipped`, or
    /// a row with no readable pair of stamps.
    NeverRan,
    /// It took this long.
    Took(u64),
}

/// One named step of a job, and what it took on either side of a rise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Moved {
    /// The step's name, which is what a step is identified BY.
    ///
    /// NOT ITS NUMBER, and that is the whole of why this compares by name.
    /// Inserting a step renumbers every step after it: R1229 added one to
    /// `validate`, so the run before the rise numbers `cargo test --workspace` 9
    /// and the run after numbers it 9 as well while four later steps shifted —
    /// and on any insert nearer the top, a comparison by number would hold each
    /// step against its neighbour and report every one of them as having moved.
    pub name: String,
    /// What it took on the last run below the level.
    pub under: Side,
    /// And on the run the level rose at.
    pub risen: Side,
}

impl Moved {
    /// How much this step moved, when both sides are a measurement.
    ///
    /// `None` WHENEVER EITHER SIDE IS NOT ONE, which is what keeps a step that
    /// never ran out of the arithmetic instead of in it as a large negative.
    #[must_use]
    pub fn seconds(&self) -> Option<i64> {
        match (self.under, self.risen) {
            (Side::Took(under), Side::Took(risen)) => {
                Some(i64::try_from(i128::from(risen) - i128::from(under)).unwrap_or(i64::MAX))
            }
            _ => None,
        }
    }
}

/// What one named step took across one run, summed.
///
/// A NAME THAT APPEARS TWICE IS SUMMED because both rows are that name's cost in
/// that run, and a workflow is free to run the same-named step more than once. A
/// row among them that did not run takes the whole name down to [`Side::NeverRan`]
/// rather than contributing nothing: a partial sum is a number smaller than the
/// truth wearing a measurement's clothes, and this block would print it as the
/// step having got cheaper.
fn side_of(steps: &[crate::Step], name: &str) -> Side {
    let mut total: u64 = 0;
    let mut seen = false;
    for step in steps.iter().filter(|step| step.name == name) {
        seen = true;
        // THE CONCLUSION IS ASKED BEFORE THE STAMPS, because a skipped step has
        // stamps and they are the stoppage's — see [`Side`].
        if step.conclusion.as_deref() == Some(crate::NEVER_RAN) {
            return Side::NeverRan;
        }
        let Some(seconds) = crate::step_seconds(step) else {
            return Side::NeverRan;
        };
        total = total.saturating_add(seconds);
    }
    if seen {
        Side::Took(total)
    } else {
        Side::Absent
    }
}

/// Every step of one job across two runs, biggest mover first.
///
/// THE UNION OF THE TWO ANSWERS, never one side's list walked against the other.
/// A step present only in the later run is the shape a workflow change makes and
/// is exactly what a reader is looking for here; iterating the earlier run's
/// steps would drop it in silence.
#[must_use]
pub fn moved_steps(under: &[crate::Step], risen: &[crate::Step]) -> Vec<Moved> {
    let mut names: Vec<&str> = under
        .iter()
        .chain(risen)
        .map(|step| step.name.as_str())
        .collect();
    names.sort_unstable();
    names.dedup();
    let mut moved: Vec<Moved> = names
        .into_iter()
        .map(|name| Moved {
            name: name.to_string(),
            under: side_of(under, name),
            risen: side_of(risen, name),
        })
        .collect();
    // BIGGEST MOVE FIRST, IN EITHER DIRECTION, and the steps that are not a
    // comparison last — `None` sorts below every `Some`, which is the order this
    // block prints in and the reason it needs no second partition.
    moved.sort_by(|left, right| {
        right
            .seconds()
            .map(i64::abs)
            .cmp(&left.seconds().map(i64::abs))
            .then_with(|| left.name.cmp(&right.name))
    });
    moved
}

/// How many one-sided steps are named before the rest are counted.
///
/// THE SAME RULE AS [`NAMED_TROUBLE`] AND FOR THE SAME REASON: a cancelled run
/// leaves every step after the stoppage unstamped, so this list is one item long
/// on an ordinary comparison and a dozen on the interesting one — and a dozen
/// lines of the same shape is a block a reader learns to skip past.
const NAMED_STEPS: usize = 3;

/// Why one step is not a comparison, in the words its two sides make true.
fn one_sided(moved: &Moved) -> &'static str {
    match (moved.under, moved.risen) {
        (Side::Absent, _) => "only in the later run",
        (_, Side::Absent) => "only in the earlier run",
        (Side::NeverRan, Side::NeverRan) => "never ran in either",
        (Side::NeverRan, _) => "never ran in the earlier run",
        (_, Side::NeverRan) => "never ran in the later run",
        (Side::Took(_), Side::Took(_)) => "a comparison after all",
    }
}

/// What moved INSIDE a job, between the two runs either side of where its cost
/// rose.
///
/// THE QUESTION R1270 LEFT ITS READER HOLDING (N198). "The level rose at this
/// commit" sends somebody to a diff; what they need next is which of the job's
/// seventeen steps got dearer, because that is the difference between "this
/// repository's suite grew" and "the runner's toolchain install is slower this
/// week" — a defect and the weather, printed identically until this block.
/// Measured by hand for `validate` before it was written: `cargo test
/// --workspace` 1276 s → 1772 s, and every other step of that job inside eight
/// seconds of where it was.
///
/// THE TWO SUMS ARE PRINTED SEPARATELY AND ARE ALLOWED TO DISAGREE. The job moved
/// +390 s across that rise while its dearest step moved +496 s, and the gap is the
/// steps the later run never reached because it was cancelled. A block that
/// reconciled them would have to invent a duration for a step that did not run.
fn attribution_lines(
    step: &Step,
    under: &Run,
    cheap: &[crate::Step],
    dear: &[crate::Step],
) -> Vec<String> {
    let moved = moved_steps(cheap, dear);
    let job =
        i64::try_from(i128::from(step.rose.took) - i128::from(under.took)).unwrap_or(i64::MAX);
    let movers: Vec<&Moved> = moved.iter().filter(|one| one.seconds().is_some()).collect();
    let mut lines = Vec::new();
    let inside = match movers.split_first() {
        None => "no step of it ran in both runs, so nothing inside it is a comparison".to_string(),
        Some((worst, rest)) => {
            // A BOUND IS A MAGNITUDE AND CARRIES NO SIGN (R1275 measured what
            // pretending otherwise says). This is the largest move among the
            // rest IN EITHER DIRECTION, and the first output of this block ran it
            // through `clock_signed` — printing `+1m36s` for a step that had got
            // ninety-six seconds CHEAPER, in the sentence whose whole job is to
            // say that nothing else here rose.
            let others = match rest
                .iter()
                .filter_map(|one| one.seconds().map(i64::abs))
                .max()
            {
                None => "and it is the only step that ran in both".to_string(),
                Some(most) => format!(
                    "while the other {} step(s) that ran in both moved by at most {} either way",
                    rest.len(),
                    clock(most.unsigned_abs())
                ),
            };
            format!(
                "`{}` moved {} ({} → {}) {others}",
                worst.name,
                clock_signed(worst.seconds().unwrap_or(0)),
                side_line(worst.under),
                side_line(worst.risen)
            )
        }
    };
    lines.push(format!(
        "      inside it, the job took {} on {} and {} on {} ({}), and {inside}",
        clock(under.took),
        short(&under.commit),
        clock(step.rose.took),
        short(&step.rose.commit),
        clock_signed(job),
    ));
    let odd: Vec<&Moved> = moved.iter().filter(|one| one.seconds().is_none()).collect();
    if !odd.is_empty() {
        let named: Vec<String> = odd
            .iter()
            .take(NAMED_STEPS)
            .map(|one| format!("`{}` ({})", one.name, one_sided(one)))
            .collect();
        let rest = odd.len().saturating_sub(named.len());
        let more = if rest > 0 {
            format!(", +{rest} more")
        } else {
            String::new()
        };
        lines.push(format!(
            "      and {} step(s) there are no comparison: {}{more} — a run that stopped \
             reaches fewer steps, so a step missing on one side is not always a step that \
             was added or taken away",
            odd.len(),
            named.join(", ")
        ));
    }
    lines
}

/// One side of a comparison, as it prints.
fn side_line(side: Side) -> String {
    match side {
        Side::Absent => "not in that run".to_string(),
        Side::NeverRan => "never ran".to_string(),
        Side::Took(seconds) => clock(seconds),
    }
}

/// A DIFFERENCE, which carries its sign — `+8m16s`, `-45s`.
///
/// SEPARATE FROM [`clock`] BECAUSE A DURATION HAS NO SIGN and every other number
/// this file prints is one. `clock` on a negative would need an `i64` it never
/// takes, and a difference printed without its sign is the same string for a job
/// that got dearer and one that got cheaper.
fn clock_signed(seconds: i64) -> String {
    let sign = if seconds < 0 { "-" } else { "+" };
    format!("{sign}{}", clock(seconds.unsigned_abs()))
}

/// What accounts for a job's step, asked of the two runs either side of where it
/// rose.
///
/// EVERY WAY THIS CAN COME BACK EMPTY IS A LINE, the rule `main.rs`'s own
/// `steps_of` states: a reader who is told nothing concludes there was nothing to
/// see, and the whole value of this block is that a reader stops guessing.
#[must_use]
pub fn attribution(steps: &dyn StepsOf, check: &str, step: &Step) -> Vec<String> {
    let Some(under) = &step.under else {
        return vec![format!(
            "      no run of `{check}` below that level is recorded here, so there is nothing \
             to hold the expensive side against"
        )];
    };
    let mut trouble = Vec::new();
    let mut ask = |run: &Run| match steps.steps_of(&run.commit, check) {
        Ok(steps) => Some(steps),
        Err(why) => {
            trouble.push(format!(
                "      NOTE what `{check}` did on {} cannot be read, so what moved inside it \
                 is unattributed — {why}",
                short(&run.commit)
            ));
            None
        }
    };
    let (cheap, dear) = (ask(under), ask(&step.rose));
    let mut lines = match (cheap, dear) {
        (Some(cheap), Some(dear)) => attribution_lines(step, under, &cheap, &dear),
        _ => Vec::new(),
    };
    lines.extend(trouble);
    lines
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
         and the series separates nowhere else, so what to ask is what changed{}",
        step.before,
        short(&step.at),
        step.when,
        step.below,
        step.after,
        step.above,
        rise_clause(step)
    )
}

/// WHERE to ask it, which is not always the split — the clause that ends
/// [`step_clause`].
///
/// THE ORDINARY ANSWER IS THE SPLIT and this repository's own record is not the
/// ordinary case. R1270 shipped the split with the words "what changed at that
/// commit"; the commit was `4eccb3d4`, whose diff is a path-normalisation in one
/// gate, and the job had been expensive since `a300556` two runs before. Both
/// runs between failed to complete, so neither could be on a curve drawn over
/// completed runs — see [`rise_of`].
fn rise_clause(step: &Step) -> String {
    if step.earlier_by == 0 {
        return " at that commit rather than when the budget is reached".to_string();
    }
    format!(
        " at {} ({}) rather than at the split — the expensive level was already there {} \
         run(s) earlier and has not been below it since, and every run in between is one \
         that did not complete, so no cost of theirs could be on the series above while \
         what they TOOK is a fact all the same",
        short(&step.rose.commit),
        step.rose.when,
        step.earlier_by
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
        landing_clause(movement)
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
    series_in(kept)
        .into_iter()
        .filter(|series| series.completed.is_empty())
        .map(|series| series.check.to_string())
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
/// One movement's line, and what accounts for its step when it has one.
///
/// THE ATTRIBUTION FOLLOWS THE LINE IT EXPLAINS rather than being a block of its
/// own at the bottom. Both movement lines this report can print may carry a step,
/// and two attributions gathered under one heading would leave a reader matching
/// them back to their jobs by name.
fn movement_block(movement: &Movement, steps: &dyn StepsOf, preamble: &str) -> Vec<String> {
    let mut lines = vec![format!("  {preamble}{}", movement_line(movement))];
    if let Some(step) = &movement.step {
        lines.extend(attribution(steps, &movement.check, step));
    }
    lines
}

#[must_use]
pub fn trend_report(kept: &[Kept], closest: Option<&str>, steps: &dyn StepsOf) -> Vec<String> {
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
        Some(one) if one.commits > 1 => lines.extend(movement_block(one, steps, "")),
        Some(one) => lines.push(format!(
            "  `{}` has completed on 1 commit only, so the job closest to its budget has no \
             trend yet",
            one.check
        )),
        // NOT SILENCE WHEN THE JOB IS ONE THAT HAS NEVER COMPLETED (R1261): the
        // level line has just named it, and a block that then says nothing about
        // it reads as a job with an unremarkable history rather than one with no
        // history at all.
        //
        // AND WHERE ITS RUNS STOPPED IS THE WHOLE OF WHAT IS KNOWN ABOUT IT
        // (R1274). "Nothing to compare it against" is true of the cost and it is
        // not the end of the answer: a job that stops at 5% of its budget every
        // time is failing before it does any work, and one that stops at 95% is
        // one whose budget is about to be the thing that ends it. Both were this
        // sentence, and it named neither.
        None => {
            if let Some(name) = closest.filter(|name| unfinished.contains(*name)) {
                let stopped = match landing_of(kept, name) {
                    Some(landing) => format!(
                        " — its {} run(s) here stopped {}, the furthest on {}{}",
                        landing.runs(),
                        where_they_stopped(&landing),
                        short(&landing.furthest_at),
                        censored_clause(&landing)
                    ),
                    None => String::new(),
                };
                lines.push(format!(
                    "  `{name}` has no completed run in this record, so there is nothing to \
                     compare what it cost against{stopped}"
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
            lines.extend(movement_block(
                one,
                steps,
                "and the steepest rise is not that job — ",
            ));
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
pub fn kept_report(
    tree: &Path,
    commit: &str,
    checks: &[Check],
    spent: &[Spent],
    steps: &dyn StepsOf,
) -> Vec<String> {
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
        steps,
    ));
    lines
}
