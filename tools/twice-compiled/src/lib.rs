//! Every compilation this repository's CI pays for is one job's.
//!
//! Seven jobs in `mnemosyne-validate.yml` build this workspace, and the reason
//! given for keeping them apart has always been that they resolve different
//! features — written as prose, in four separate comments, never once measured.
//! The same prose is why one cache key holds 7.83 GB of a 10 GB budget while the
//! other seven hold 0.65 GB between them: a build tree per job is what a job per
//! build tree costs.
//!
//! THE MEASUREMENT IS THE POINT. "Different feature resolve" is a claim with a
//! number behind it, and this asks the machine for that number rather than
//! arguing it:
//!
//! - **What CI runs** — [`ci_plan`] reads every cargo invocation out of the
//!   tracked workflows, so the job list is the workflows' and not a copy of it.
//! - **What each job compiles** — `tools/rustc-log` sits in `RUSTC_WRAPPER` and
//!   records every `rustc` the job runs, including the ones a gate spawns by
//!   shelling out to cargo, which is the majority of the work in three of these
//!   jobs and is invisible to `--message-format=json`.
//!
//! A COMPILATION IS NOT A COMPILATION, which is why every number here comes in
//! two: how many, and what they cost. The first census this took found the head
//! of the duplication to be `build_script_build` at 409 surplus compilations —
//! and a build script is among the cheapest units cargo drives, while one
//! `mnemosyne-store` test binary is among the dearest. Ranked by rows, the
//! repair goes to the dependencies; ranked by seconds it may not, and nothing
//! could tell the two rankings apart until the recorder carried a clock.
//!
//! THE SECONDS ANSWER A QUESTION THE ROWS CANNOT. Merging two jobs removes the
//! compilations they share, and it also puts one job's minutes after the other's
//! instead of beside them — the pair that saves the most work can still be the
//! pair that lengthens the run. `twice_compiled::Merge` states that trade in the
//! measured units of both sides: the work merging removes, and the window the
//! merged job would run in, bounded above and below by what was observed rather
//! than modelled.
//!
//! A UNIT IS CARGO'S OWN, not this crate's idea of one. `-C metadata=<hash>` is
//! the fingerprint cargo computes for a compilation from the package, its
//! resolved features, the profile and the compiler's own version, and it is on
//! every `rustc` cargo runs and on none of the probes it runs first. Two jobs
//! that emit the same hash are compiling the same thing; two that do not are
//! genuinely resolving differently, and the MSRV job is the control that shows
//! this reader can tell — it runs the same words as `validate` over the same
//! sources on a different toolchain, so if its units did not come back disjoint,
//! the key would be wrong.
//!
//! A CENSUS IS ALSO A STATE, and until Round 1101 this one could not say which.
//! The counts here are of a COLD build by construction — cargo runs no compiler
//! for a unit that is already fresh, so a job whose cache came back compiles
//! less and this reader sees less. That makes the cache state a control variable
//! of every number below, and two censuses taken under different ones are not
//! each other's control.
//!
//! Round 1099 compared two of them by hand. It read a run whose cache keys had
//! all moved as a run that had built from nothing, found its census identical to
//! the next run's, concluded that a 7.5 GB cache was skipping no compilations,
//! and deleted it. The cache was saving 426 compilations and ten minutes; Round
//! 1100 put it back. The run called cold had restored a whole previous
//! generation through `restore-keys`, and NEITHER of this repository's cache
//! instruments could have said so — `actions/cache` reports `cache-hit: false`
//! for a prefix match, and a job warmed by one still saves a new entry, which is
//! what `tools/cache-budget` reads as a cache built from nothing.
//!
//! So every job now writes what its restore put on disk (`tools/restored`), and
//! that record is joined to the log beside it here. The census REPORTS the state
//! rather than judging it, and REFUSES when a job that declares a cache did not
//! say — a number quoted without the state it was taken in is the number that
//! got deleted.
//!
//! WHAT IT ASSERTS TODAY is that it reached every job, that every job it reached
//! left a clock behind, and that every job with a cache said what that cache
//! brought. A workflow job that issues a cargo command and leaves no record is
//! the empty answer that reads like a clean one, which is the failure this
//! repository keeps meeting; a job whose records carry counts and no seconds is
//! the quieter version of it, where every total adds up and the work reads as
//! free. The duplication itself is REPORTED rather than refused, because the
//! number is what licenses the repair and the repair is what makes a limit
//! assertable.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub use ci_plan::RunStep;

// --- one compilation --------------------------------------------------------

/// One compilation, named the way cargo names it.
///
/// `metadata` alone is very nearly the whole key; the rest is there so that a
/// cargo release which narrows what goes into that hash makes this reader SPLIT
/// units rather than merge them. Splitting under-reports duplication, which is
/// the direction a gate should fail in.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Unit {
    /// `--crate-name`.
    pub crate_name: String,
    /// `-C metadata=…` — cargo's fingerprint for this unit.
    pub metadata: String,
    /// `--emit=…`, which is what tells a `cargo check` unit from a `cargo build`
    /// one when everything else about them agrees.
    pub emit: String,
    /// `--crate-type` values, in the order written.
    pub crate_types: Vec<String>,
    /// `--test`: a test harness build of a target is not the target.
    pub test: bool,
}

/// What one `rustc` invocation was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Invocation {
    /// A compilation cargo drives.
    Compilation(Box<Unit>),
    /// `rustc -vV`, `--print=cfg`, the crate-type probe — cargo asking the
    /// compiler about itself. Real processes, no compilation.
    Probe,
    /// A compilation with no `-C metadata`, which is what a build script running
    /// `rustc` itself looks like (`autocfg` and friends). Counted and printed
    /// rather than dropped, because a class this reader cannot key is a class it
    /// must not silently call absent.
    Unkeyed,
}

/// The value of `--flag value` or `--flag=value`, for a flag written either way.
fn flag_value<'a>(argv: &'a [String], name: &str) -> Option<&'a str> {
    let joined = format!("{name}=");
    argv.iter().enumerate().find_map(|(index, word)| {
        if let Some(value) = word.strip_prefix(&joined) {
            return Some(value);
        }
        (word == name)
            .then(|| argv.get(index + 1).map(String::as_str))
            .flatten()
    })
}

/// The value of a `-C key=value`, written either as two words or as `-Ckey=…`.
fn codegen_value<'a>(argv: &'a [String], key: &str) -> Option<&'a str> {
    let prefix = format!("{key}=");
    argv.iter().enumerate().find_map(|(index, word)| {
        if let Some(value) = word.strip_prefix("-C").and_then(|rest| {
            let rest = rest.strip_prefix(' ').unwrap_or(rest);
            rest.strip_prefix(&prefix)
        }) {
            return Some(value);
        }
        if word != "-C" {
            return None;
        }
        argv.get(index + 1)?.strip_prefix(&prefix)
    })
}

/// Every `--crate-type` value, in the order written.
fn crate_types(argv: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for (index, word) in argv.iter().enumerate() {
        if let Some(value) = word.strip_prefix("--crate-type=") {
            out.push(value.to_string());
        } else if word == "--crate-type" {
            if let Some(value) = argv.get(index + 1) {
                out.push(value.clone());
            }
        }
    }
    out
}

/// What one job paid for one unit: how many compilations, and how long they ran.
///
/// The two travel together because every question here needs both and neither
/// can be recovered from the other — a job that compiles a unit twice pays twice
/// the seconds, and two jobs' single compilations of the same unit are two rows
/// at two prices.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cost {
    /// Compilations.
    pub times: usize,
    /// Microseconds they took, summed.
    pub micros: u64,
}

impl Cost {
    /// What ONE compilation of this unit cost, in this job.
    ///
    /// Integer division, and the remainder is dropped on purpose: every total
    /// below is built from sums of `micros` and only the per-compilation figures
    /// use this, so a rounded microsecond cannot make two lines of the report
    /// disagree.
    pub fn each(&self) -> u64 {
        if self.times == 0 {
            return 0;
        }
        self.micros / self.times as u64
    }

    /// Add one compilation.
    pub fn add(&mut self, micros: u64) {
        self.times += 1;
        self.micros = self.micros.saturating_add(micros);
    }
}

/// Read one record. The first word is the compiler; the rest are its arguments.
pub fn read(record: &[String]) -> Invocation {
    let argv = &record[1..];
    // A PROBE FIRST, because cargo's crate-type probe carries `--crate-name ___`
    // and would otherwise look like a compilation of a crate called `___`.
    if argv.iter().any(|word| word.starts_with("--print")) {
        return Invocation::Probe;
    }
    let Some(crate_name) = flag_value(argv, "--crate-name") else {
        return Invocation::Probe;
    };
    let Some(metadata) = codegen_value(argv, "metadata") else {
        return Invocation::Unkeyed;
    };
    Invocation::Compilation(Box::new(Unit {
        crate_name: crate_name.to_string(),
        metadata: metadata.to_string(),
        emit: flag_value(argv, "--emit").unwrap_or_default().to_string(),
        crate_types: crate_types(argv),
        test: argv.iter().any(|word| word == "--test"),
    }))
}

// --- one job ----------------------------------------------------------------

/// What one job compiled.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JobLog {
    /// Every `rustc` the job ran, probes included.
    pub invocations: usize,
    /// Invocations that were cargo asking the compiler about itself.
    pub probes: usize,
    /// Compilations with no `-C metadata` — see [`Invocation::Unkeyed`].
    pub unkeyed: usize,
    /// Each unit this job compiled, HOW MANY TIMES it compiled it, and what
    /// that cost.
    ///
    /// A count rather than a set, because a job can compile one unit twice: a
    /// job holding two `target` directories — the root one and a tool
    /// workspace's — pays for a shared dependency in both. Keeping only the set
    /// makes those repeats vanish from every breakdown while staying in the
    /// total, and two numbers in one report that do not add up is a report
    /// nobody can act on.
    pub units: BTreeMap<Unit, Cost>,
    /// Every invocation's duration summed, probes and unkeyed compilations
    /// included — the whole of what the job's `rustc` processes cost.
    pub micros: u64,
    /// When each of this job's compilers ran: start and exit, one pair per
    /// invocation, in the order the records arrived.
    ///
    /// KEPT RATHER THAN FOLDED INTO A WINDOW, because a window cannot answer the
    /// question the window is asked for. Merging two jobs is worth wall-clock
    /// only where a compiler was actually alive; the minutes a job spends
    /// running the tests it just built are inside its window and are not made
    /// shorter by compiling less. Separating the two takes the intervals, so
    /// they are what is stored and everything else here is derived from them.
    pub intervals: Vec<(u64, u64)>,
}

impl JobLog {
    /// Keyed compilations, counted with their repeats.
    pub fn compilations(&self) -> usize {
        self.units.values().map(|cost| cost.times).sum()
    }

    /// Compilations this job paid for twice by itself.
    pub fn repeats(&self) -> usize {
        self.compilations() - self.units.len()
    }

    /// What the keyed compilations cost — the job's WORK.
    ///
    /// Not its wall-clock: cargo runs as many compilers as the machine has
    /// cores, so this is the area under all of them and is larger than the time
    /// the job took. `JobLog::span_micros` is the other half of that pair.
    pub fn compiled_micros(&self) -> u64 {
        self.units.values().map(|cost| cost.micros).sum()
    }

    /// The first compiler's start and the last one's exit.
    ///
    /// TAKEN FROM THE CLOCKS AND NOT FROM THE ORDER OF THE RECORDS. A record is
    /// appended when a compiler EXITS, and cargo runs as many at once as the
    /// machine has cores, so the long compilation that began the job arrives
    /// after the short ones that began later. A reader taking the first and last
    /// LINES would read a window shorter than the job's, every time.
    pub fn window(&self) -> Option<(u64, u64)> {
        let first = self.intervals.iter().map(|(start, _)| *start).min()?;
        let last = self.intervals.iter().map(|(_, end)| *end).max()?;
        Some((first, last))
    }

    /// From the first compiler's start to the last one's exit — the job's
    /// COMPILING WINDOW.
    ///
    /// WHAT IT IS NOT is the job's duration. It excludes the checkout, the cache
    /// restore and the artifact upload that bracket it, and it INCLUDES every
    /// gap inside it. Both differences are named because the number is used to
    /// reason about merging jobs, and a reader who took it for the job's
    /// duration would be over-counting what a repair can reach.
    pub fn span_micros(&self) -> u64 {
        self.window()
            .map(|(first, last)| last.saturating_sub(first))
            .unwrap_or(0)
    }

    /// How much of the window had AT LEAST ONE compiler alive.
    ///
    /// The union of the intervals, not their sum: a job running eight compilers
    /// at once is busy for as long as the eight of them overlap, and summing
    /// would report a job busier than the clock allows. This is the part of a
    /// job's window that compiling less can shorten.
    pub fn busy_micros(&self) -> u64 {
        let mut intervals = self.intervals.clone();
        intervals.sort_unstable();
        let mut busy: u64 = 0;
        let mut open: Option<(u64, u64)> = None;
        for (start, end) in intervals {
            match open {
                Some((from, until)) if start <= until => open = Some((from, until.max(end))),
                Some((from, until)) => {
                    busy = busy.saturating_add(until.saturating_sub(from));
                    open = Some((start, end));
                }
                None => open = Some((start, end)),
            }
        }
        if let Some((from, until)) = open {
            busy = busy.saturating_add(until.saturating_sub(from));
        }
        busy
    }

    /// How much of the window had NO compiler alive at all.
    ///
    /// THE PART OF A JOB THAT COMPILING LESS CANNOT REACH: a suite running the
    /// binaries it just built, a gate reading a store, a download. It is
    /// measured rather than assumed away because a merge estimate that scaled it
    /// with the compiling would promise minutes that are not there.
    pub fn idle_micros(&self) -> u64 {
        self.span_micros().saturating_sub(self.busy_micros())
    }
}

/// Read one job's log.
pub fn read_log(text: &str) -> JobLog {
    let mut log = JobLog::default();
    for record in rustc_log::decode_all(text) {
        log.invocations += 1;
        log.micros = log.micros.saturating_add(record.micros);
        log.intervals.push((record.started_at, record.ended_at()));
        match read(&record.argv) {
            Invocation::Probe => log.probes += 1,
            Invocation::Unkeyed => log.unkeyed += 1,
            Invocation::Compilation(unit) => log.units.entry(*unit).or_default().add(record.micros),
        }
    }
    log
}

// --- every job --------------------------------------------------------------

/// What CI compiled, job by job.
#[derive(Debug, Clone, Default)]
pub struct Census {
    /// Keyed by job id, which is unique WITHIN one workflow because GitHub makes
    /// it a mapping key there. Across workflows it need not be, and that is one
    /// of the reasons a census covers a single workflow: two files could each
    /// declare a `validate`, and their logs would be one another's.
    pub jobs: BTreeMap<String, JobLog>,
    /// What each job STARTED FROM: the record `tools/restored` leaves either
    /// side of the job's cache restore, keyed by the file it arrived in.
    ///
    /// THE STATE THE COUNTS WERE TAKEN IN, kept beside them rather than derived
    /// from them, because it cannot be derived from them: a job that compiled
    /// 773 units because its tree was empty and a job that compiled 773 units
    /// because its tree was stale are the same census and different runs. The
    /// error is kept rather than dropped so that a record which did not decode
    /// travels as a refusal and not as a job with no cache at all.
    pub restored: BTreeMap<String, Result<restored::Restored, restored::Malformed>>,
}

impl Census {
    /// Every distinct unit any job compiled.
    pub fn distinct(&self) -> BTreeSet<&Unit> {
        self.jobs
            .values()
            .flat_map(|log| log.units.keys())
            .collect()
    }

    /// Compilations summed over jobs — what CI actually pays for.
    pub fn paid(&self) -> usize {
        self.jobs.values().map(JobLog::compilations).sum()
    }

    /// Compilations a single machine sharing one `target` would pay for — the
    /// floor, and the number the sum above is worth comparing to.
    pub fn floor(&self) -> usize {
        self.distinct().len()
    }

    /// What CI's compilations cost, summed over jobs.
    ///
    /// WORK AND NOT WALL-CLOCK, for the reason `JobLog::compiled_micros` gives:
    /// these are areas under processes that ran in parallel.
    pub fn paid_micros(&self) -> u64 {
        self.jobs.values().map(JobLog::compiled_micros).sum()
    }

    /// For each distinct unit, what the ONE compilation of it that would survive
    /// costs.
    ///
    /// THE DEAREST OBSERVED, and that choice is the direction this errs in. The
    /// same unit takes different times in different jobs — a runner under a
    /// different load, a cache in a different state — and the surviving
    /// compilation has to be priced at one of them. Taking the dearest makes the
    /// floor as high as the evidence allows, so every saving reported below is
    /// the LEAST that the repair could win. A gate that talks somebody into a
    /// repair should quote the smaller number.
    pub fn retained_micros(&self) -> BTreeMap<&Unit, u64> {
        let mut out: BTreeMap<&Unit, u64> = BTreeMap::new();
        for log in self.jobs.values() {
            for (unit, cost) in &log.units {
                let entry = out.entry(unit).or_default();
                *entry = (*entry).max(cost.each());
            }
        }
        out
    }

    /// What one machine sharing one `target` would pay in seconds — the floor
    /// the sum above is worth comparing to.
    pub fn floor_micros(&self) -> u64 {
        self.retained_micros().values().sum()
    }

    /// The seconds a job spends compiling something it already compiled.
    ///
    /// The time half of `Census::repeated_within_jobs`, and merging jobs cannot
    /// reach it for the same reason.
    pub fn repeated_within_jobs_micros(&self) -> u64 {
        self.jobs
            .values()
            .flat_map(|log| log.units.values())
            .map(|cost| cost.micros.saturating_sub(cost.each()))
            .sum()
    }

    /// The seconds that exist because the work is split across jobs.
    ///
    /// Defined as what is left rather than summed on its own, exactly as
    /// `Census::shared_between_jobs` is, so that the two halves and the whole
    /// cannot drift apart by a rounded microsecond.
    pub fn shared_between_jobs_micros(&self) -> u64 {
        self.paid_micros()
            .saturating_sub(self.floor_micros())
            .saturating_sub(self.repeated_within_jobs_micros())
    }

    /// Surplus compilations a job pays for inside itself.
    ///
    /// THIS HALF IS THE ONE MERGING JOBS CANNOT REACH, which is why it is
    /// counted apart from the rest. A job that builds several SEPARATE
    /// workspaces holds a `target` directory per workspace, and a dependency
    /// they share is compiled once in each — one job, one runner, one cache
    /// key, and still two compilations. Making that job and another one job
    /// changes nothing about it.
    pub fn repeated_within_jobs(&self) -> usize {
        self.jobs.values().map(JobLog::repeats).sum()
    }

    /// Surplus compilations that exist because the work is split across jobs.
    ///
    /// THIS HALF IS WHAT MERGING WOULD REMOVE: two jobs emitting the same
    /// `-C metadata` are resolving identically, so one job with one `target`
    /// would compile it once. It is the number the "they resolve different
    /// features" sentence was standing in for.
    pub fn shared_between_jobs(&self) -> usize {
        self.paid() - self.floor() - self.repeated_within_jobs()
    }

    /// Every unit more than one job compiled, with the jobs that did.
    pub fn shared(&self) -> BTreeMap<&Unit, Vec<&str>> {
        let mut out: BTreeMap<&Unit, Vec<&str>> = BTreeMap::new();
        for (job, log) in &self.jobs {
            for unit in log.units.keys() {
                out.entry(unit).or_default().push(job.as_str());
            }
        }
        out.retain(|_, jobs| jobs.len() > 1);
        out
    }

    /// How many compilations of each crate CI pays for beyond the first.
    ///
    /// WHAT IS SHARED DECIDES WHICH REPAIR IS THE RIGHT ONE, and the totals
    /// cannot say. Duplication concentrated in third-party dependencies is
    /// answered by sharing a compilation cache between jobs; duplication in this
    /// repository's own crates is answered by the jobs being one job. A number
    /// with no breakdown behind it licenses whichever repair was already
    /// preferred.
    /// How many compilations of each crate CI pays for beyond the first, and
    /// what they cost.
    ///
    /// THE TWO RANKINGS ARE NOT THE SAME RANKING, which is the whole reason the
    /// seconds are here: a build script is a surplus row and very nearly no
    /// money, and one test binary of this repository's own is the other way
    /// round. Ranked by rows alone this reads as an argument for a shared
    /// compilation cache; the seconds are what say whether it is.
    pub fn surplus_by_crate(&self) -> BTreeMap<&str, Cost> {
        let mut paid: BTreeMap<&str, Cost> = BTreeMap::new();
        for log in self.jobs.values() {
            for (unit, cost) in &log.units {
                let entry = paid.entry(unit.crate_name.as_str()).or_default();
                entry.times += cost.times;
                entry.micros = entry.micros.saturating_add(cost.micros);
            }
        }
        for (unit, retained) in self.retained_micros() {
            if let Some(surplus) = paid.get_mut(unit.crate_name.as_str()) {
                surplus.times -= 1;
                surplus.micros = surplus.micros.saturating_sub(retained);
            }
        }
        paid.retain(|_, surplus| surplus.times > 0);
        paid
    }

    /// The state each job started in — the control variable every count above
    /// is taken under.
    ///
    /// ONLY THE RECORDS THAT DECODED, because the alternative is a job appearing
    /// here with a state invented for it; the ones that did not are refusals and
    /// travel as those.
    pub fn started(&self) -> BTreeMap<&str, restored::Warmth> {
        self.restored
            .iter()
            .filter_map(|(job, record)| Some((job.as_str(), record.as_ref().ok()?.warmth())))
            .collect()
    }

    /// What merging each pair of jobs would remove, and what it would cost.
    pub fn pairwise(&self) -> BTreeMap<(&str, &str), Merge> {
        let mut out = BTreeMap::new();
        let jobs: Vec<(&str, &JobLog)> = self
            .jobs
            .iter()
            .map(|(name, log)| (name.as_str(), log))
            .collect();
        for (index, (left, left_log)) in jobs.iter().enumerate() {
            for (right, right_log) in &jobs[index + 1..] {
                let merge = Merge::of(left_log, right_log);
                if merge.units > 0 {
                    out.insert((*left, *right), merge);
                }
            }
        }
        out
    }
}

/// What making two jobs one job would remove, and what it would cost.
///
/// BOTH SIDES OF THE TRADE, because this repository has already written down one
/// side of it four times. Merging removes the compilations two jobs share — that
/// is `Merge::units` and `Merge::saved_micros`, both measured. It also puts one
/// job's minutes AFTER the other's rather than beside them, and a pair that
/// saves the most work can still be the pair that lengthens the run: the jobs
/// that share the most here are the two longest.
///
/// The window is given as a range and not a number. Its ends are arithmetic on
/// observed spans and nothing else. The estimate between them scales only the
/// part of each window that had a compiler ALIVE in it, and carries the part
/// that did not across untouched — the minutes a job spends running the tests it
/// just built do not get shorter because it compiled less. What is left assumed
/// is that the merged job compiles at the rate the two of them did, which a
/// runner with a fixed core count makes very nearly true.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Merge {
    /// Compilations merging removes: the units both jobs compile.
    pub units: usize,
    /// What those compilations cost, priced at the CHEAPER of the two jobs'
    /// figures for each — the least the merge can win.
    pub saved_micros: u64,
    /// The merged job's compiling window cannot be shorter than the longer of
    /// the two it replaces — which is also what the pair costs the run TODAY,
    /// running beside one another. Merging can spend wall-clock; it can never
    /// win any.
    pub floor_micros: u64,
    /// Nor longer than the two of them end to end, which is what merging and
    /// sharing nothing would cost.
    pub ceiling_micros: u64,
    /// Between the two: the compiling part of the pair's windows scaled by the
    /// work that remains, plus the idle part carried across whole.
    pub estimate_micros: u64,
    /// How much of the pair's two windows had no compiler alive — the part of
    /// the estimate that no amount of removed compiling can shorten.
    pub idle_micros: u64,
}

impl Merge {
    /// Measure the trade for one pair.
    fn of(left: &JobLog, right: &JobLog) -> Merge {
        let mut units = 0;
        let mut saved_micros: u64 = 0;
        for (unit, cost) in &left.units {
            if let Some(other) = right.units.get(unit) {
                units += 1;
                saved_micros = saved_micros.saturating_add(cost.each().min(other.each()));
            }
        }
        let work = left
            .compiled_micros()
            .saturating_add(right.compiled_micros());
        let floor_micros = left.span_micros().max(right.span_micros());
        let ceiling_micros = left.span_micros().saturating_add(right.span_micros());
        let idle_micros = left.idle_micros().saturating_add(right.idle_micros());
        let busy_micros = left.busy_micros().saturating_add(right.busy_micros());
        let compiling = if work == 0 {
            busy_micros
        } else {
            let remaining = u128::from(work.saturating_sub(saved_micros));
            let scaled = u128::from(busy_micros) * remaining / u128::from(work);
            u64::try_from(scaled).unwrap_or(u64::MAX)
        };
        Merge {
            units,
            saved_micros,
            floor_micros,
            ceiling_micros,
            estimate_micros: compiling.saturating_add(idle_micros).max(floor_micros),
            idle_micros,
        }
    }
}

// --- the law ----------------------------------------------------------------

/// A reason this gate will not report a number.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Refusal {
    /// A job left no compilation behind. Either the wrapper is not wired into
    /// it, or its log never arrived — and both look exactly like a job with
    /// nothing to build.
    JobLeftNoRecord { job: String },
    /// The census covers fewer than two jobs.
    ///
    /// THE SUBJECT OF THIS GATE IS WHAT TWO JOBS BOTH COMPILE, so a census of
    /// one job has no cross-job duplication BY CONSTRUCTION and a census of none
    /// has nothing at all — and both print every total as a clean zero and exit
    /// as though the file were in good order. A local replay did exactly that:
    /// it refused all nine jobs, printed `0 compilations across 0 job(s)`, and
    /// signed off with "every one of the 0 job(s) recorded what it compiled".
    /// A gate that cannot say how far it reached is a gate whose silence and
    /// whose success are the same output.
    CensusCoversTooFewJobs { covered: usize },
    /// A job's compilations took no time at all.
    ///
    /// THE SECOND WAY AN INSTRUMENT GOES SILENT, and it is quieter than the
    /// first: the counts are all there, every total adds up, and only the
    /// seconds are zero — which reads as work that is free, the exact finding
    /// that would argue for merging every job in the file. It happens when a job
    /// runs an older recorder than the one this reads, which is a real state on
    /// a runner restoring a cached binary, and no count can notice it.
    JobRecordedNoTime { job: String },
    /// A log names a job this workflow does not declare. The measurement would
    /// be of a CI that no longer exists.
    RecordFromNoJob { job: String },
    /// A step runs without the recorder in its environment, so whatever it
    /// compiles is invisible here. Named per variable, because the two do
    /// different jobs and either one missing is the same silence.
    StepIsNotRecorded { job: String, missing: String },
    /// A job records to a file that is not named after it. Two jobs writing one
    /// path make a census of one job with every unit in it, which reports NO
    /// duplication at all — and the way that happens is a job copied from
    /// another with the name not changed, which is the paste error this
    /// repository already has a rule about.
    LogIsNotNamedForItsJob { job: String, path: String },
    /// A job with a cache left no record of what that cache brought.
    ///
    /// THE STATE IS A CONTROL VARIABLE OF THE COUNTS, and a census missing it
    /// reads exactly like one taken in whatever state the reader assumed. Round
    /// 1099 assumed cold, and the assumption cost a cache that was saving ten
    /// minutes a run.
    JobDidNotSayWhatItRestored { job: String },
    /// A record arrived and could not be read.
    RestoreRecordIsMalformed { job: String, why: String },
    /// A record measured paths that are not the ones the job's cache holds.
    ///
    /// THE LIST IS WRITTEN TWICE — once as the cache's `path:` and once as the
    /// argument to the step that measures it — and this is what makes that safe.
    /// Both sides are read off the machine and compared here, so a path added to
    /// a cache and not to the measurement is a refusal rather than a silently
    /// smaller difference.
    RestoreRecordMeasuredOtherPaths {
        job: String,
        measured: Vec<String>,
        declared: Vec<String>,
    },
    /// A record says the primary key matched exactly and that nothing arrived on
    /// disk. The two instruments contradict each other, and which of them is
    /// wrong decides whether this census is of a warm run or a cold one.
    RestoredNothingAfterAnExactHit { job: String },
    /// A record's file is named for one job and its contents for another — the
    /// same paste error `LogIsNotNamedForItsJob` covers, caught on the other
    /// side, because the record carries the job the runner said it was in.
    RestoreRecordNamesAnotherJob { file: String, said: String },
    /// A record from a job whose cache this workflow does not declare, or from
    /// no job of this workflow at all.
    RestoreRecordFromAJobWithNoCache { job: String },
    /// A job with a cache does not say WHERE to write what it restored, or says
    /// a path that is another job's — the mirror of `LogIsNotNamedForItsJob`,
    /// read off the workflow rather than off the record, so it fires on a job
    /// that has not run yet.
    RestoreIsNotRecorded { job: String, path: String },
    /// A job with a cache does not measure one side of its restore exactly once.
    ///
    /// WHAT A JOB STARTED FROM IS A DIFFERENCE, so it takes two measurements and
    /// exactly two. A side measured twice is a record whose second reading
    /// overwrites the first; a side not measured at all is a job that writes no
    /// record, which the census then reports as a job that did not say — the
    /// right verdict arrived at an hour late, from a run, rather than from the
    /// file that is already wrong.
    RestoreSideIsNotMeasuredOnce {
        job: String,
        side: restored::Side,
        times: usize,
    },
    /// A job measures one side of its restore on the WRONG side of the cache
    /// step.
    ///
    /// THE FAILURE THAT LOOKS LIKE A FINDING. Both measurements on one side of
    /// the restore give a difference of zero, and zero is exactly the shape of a
    /// job that compiled from an empty tree — the state R1099 misread, at the
    /// cost of a cache that was saving ten minutes a run. R1102 made the runtime
    /// verdict loud for it; this is the same defect caught in the file, where it
    /// is a wiring mistake anyone can see rather than a census nobody can trust.
    RestoreIsMeasuredOnTheWrongSide {
        job: String,
        side: restored::Side,
        measured_at: usize,
        cache_at: usize,
    },
    /// A step measures a cache restore in a job that declares no cache — the
    /// mirror of `RestoreRecordFromAJobWithNoCache`, read off the workflow, so
    /// it fires before any run leaves a record behind.
    RestoreIsMeasuredWithNoCache { job: String },
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refusal::JobLeftNoRecord { job } => write!(
                f,
                "job `{job}` recorded no compilation — a job whose recorder is \
                 unwired reads exactly like a job with nothing to build"
            ),
            Refusal::CensusCoversTooFewJobs { covered } => write!(
                f,
                "this census covers {covered} job(s), and its whole subject is \
                 what TWO of them both compile — fewer than two has no finding \
                 available to it and prints a clean zero for every total"
            ),
            Refusal::JobRecordedNoTime { job } => write!(
                f,
                "job `{job}` compiled, and every compilation took no time at \
                 all — work that is free is the finding a reader of this report \
                 is hunting for, and a recorder older than this reader prints it"
            ),
            Refusal::RecordFromNoJob { job } => write!(
                f,
                "a log names job `{job}`, which this workflow does not declare \
                 — that would be a measurement of a CI that is gone"
            ),
            Refusal::StepIsNotRecorded { job, missing } => write!(
                f,
                "a step of job `{job}` runs without ${missing}, so what it \
                 compiles cannot be counted and its absence would read as zero"
            ),
            Refusal::LogIsNotNamedForItsJob { job, path } => write!(
                f,
                "job `{job}` records to `{path}`, which is not `{job}.log` — the \
                 census reads the job's name off the file, so two jobs sharing a \
                 path become one blob wearing the shape of a census"
            ),
            Refusal::JobDidNotSayWhatItRestored { job } => write!(
                f,
                "job `{job}` declares a cache and left no record of what it \
                 brought — this census counts a COLD build by construction, so \
                 the cache state is a control variable of every number in it, \
                 and a census missing it reads as one taken in whatever state \
                 the reader assumed"
            ),
            Refusal::RestoreRecordIsMalformed { job, why } => write!(
                f,
                "job `{job}` left a record of what it restored that does not \
                 read: {why}"
            ),
            Refusal::RestoreRecordMeasuredOtherPaths {
                job,
                measured,
                declared,
            } => write!(
                f,
                "job `{job}` measured {} across its cache restore and its cache \
                 holds {} — the two lists are written in two places and this is \
                 the only thing holding them together, so a path in one and not \
                 the other is a restore measured smaller than it was",
                measured.join(", "),
                declared.join(", ")
            ),
            Refusal::RestoredNothingAfterAnExactHit { job } => write!(
                f,
                "job `{job}` was told by `actions/cache` that its primary key \
                 matched exactly, and not one byte arrived under the paths that \
                 cache holds — one of the two instruments is wrong, and which \
                 one decides whether this census is of a warm run or a cold one"
            ),
            Refusal::RestoreRecordNamesAnotherJob { file, said } => write!(
                f,
                "`{file}.restored` says it was written by job `{said}` — a \
                 record carries the job the runner named, so this is two jobs \
                 writing one path and one of them has no record at all"
            ),
            Refusal::RestoreRecordFromAJobWithNoCache { job } => write!(
                f,
                "a record of what job `{job}` restored arrived, and this \
                 workflow declares no cache for it — either the census is of a \
                 CI that is gone, or a job is measuring a restore that never \
                 happens"
            ),
            Refusal::RestoreIsNotRecorded { job, path } if path.is_empty() => write!(
                f,
                "a step of job `{job}` runs without ${}, and that job declares a \
                 cache — so nothing says what its build tree held when it \
                 started, and this census reads as one taken in whatever state \
                 its reader assumed",
                restored::VARIABLE
            ),
            Refusal::RestoreIsNotRecorded { job, path } => write!(
                f,
                "job `{job}` declares a cache and writes what it restored to \
                 `{path}`, which is not `{job}.restored` — the census reads the \
                 job's name off the file, so two jobs sharing a path leave one \
                 of them with no state at all"
            ),
            Refusal::RestoreSideIsNotMeasuredOnce { job, side, times } => write!(
                f,
                "job `{job}` declares a cache and runs `restored {}` {times} \
                 time(s) — what a job started from is the DIFFERENCE between two \
                 measurements, so it takes one of each and no more",
                side.word()
            ),
            Refusal::RestoreIsMeasuredOnTheWrongSide {
                job,
                side,
                measured_at,
                cache_at,
            } => write!(
                f,
                "job `{job}` runs `restored {}` as step {measured_at} and its \
                 cache is step {cache_at} — with both measurements on one side \
                 of the restore the difference is zero, which is exactly what a \
                 job that compiled from an empty tree reports",
                side.word()
            ),
            Refusal::RestoreIsMeasuredWithNoCache { job } => write!(
                f,
                "job `{job}` measures a cache restore and declares no cache — \
                 there is nothing between its two measurements, so it would \
                 report an empty tree for a job that never had one to fill"
            ),
        }
    }
}

/// The environment variable naming the recorder, as cargo spells it.
pub const WRAPPER_VARIABLE: &str = "RUSTC_WRAPPER";

/// The variables a local replay sets for itself, whatever the workflow spells
/// for them.
///
/// ONE LIST, TWO USES, and the second one is why it is here rather than beside
/// the replay: these names are applied to every step the replay runs, AND they
/// are the names `twice_compiled::unresolvable` must not read. The workflow
/// spells both as `${{ github.workspace }}/…`, which is an expression only
/// GitHub resolves — so a replay that looked for expressions without skipping
/// the ones it replaces refuses every job in the file for a value nobody reads.
/// That is not hypothetical: it is what happened the day the recorder was wired
/// into every job, and it stayed that way because a replay is expensive and
/// nobody ran it again.
///
/// FIVE OF THEM, AND THE LAST THREE ARE WHY A REPLAY IS COMPARABLE TO ITSELF AND
/// NOT TO CI. A replay restores no cache — it runs a job's `run:` steps and nothing
/// else — so it sets `cache-hit` to `false` and lets the two measurements around
/// the (absent) restore say what they truly say: nothing arrived. Its censuses
/// are therefore recorded as taken from an empty tree, which is what they are,
/// and the comparison a reader might make against a warm CI run is one the
/// records now refuse to make quietly. `GITHUB_JOB` is the job id the recorder
/// asks the runner for; a replay is running that job, so it says so.
pub const REPLAY_SETS: [&str; 5] = [
    WRAPPER_VARIABLE,
    rustc_log::LOG_VARIABLE,
    restored::VARIABLE,
    restored::EXACT_VARIABLE,
    JOB_VARIABLE,
];

/// The variable the runner names the job id in, and the one `tools/restored`
/// takes the name in its record from.
pub const JOB_VARIABLE: &str = "GITHUB_JOB";

/// This crate's own manifest.
///
/// THE GATE DOES NOT REPLAY ITSELF. Its job downloads what the others wrote and
/// joins it, so replaying it means running the census over a directory the
/// replay is in the middle of filling — and counting the instrument's own build
/// as a cost CI pays. Derived from the manifest the step names, like the
/// recorder's exemption, so it stays exactly as wide as the thing it excuses.
pub const GATE_MANIFEST: &str = "tools/twice-compiled/Cargo.toml";

/// The first thing in a job that this machine cannot reproduce, if there is one.
///
/// TWO KINDS, and both are refusals rather than guesses. An expression only
/// GitHub resolves — `RUSTUP_TOOLCHAIN: ${{ steps.msrv.outputs.version }}`, a
/// token from `secrets` — would otherwise be replayed as the literal text or as
/// nothing, and a job replayed on the wrong toolchain reports its units as
/// shared with every other job, which is the loudest wrong answer available
/// here. And the gate's own job, which is not a build this CI pays for but the
/// reading of one.
pub fn unresolvable(steps: &[&RunStep]) -> Option<String> {
    for step in steps {
        if step.script.contains(GATE_MANIFEST) {
            return Some(format!(
                "it runs {GATE_MANIFEST}, which is this gate — replaying it \
                 would read a log directory this replay is still filling and \
                 count the instrument's own build as a cost CI pays"
            ));
        }
        if let Some((name, value)) = step
            .env
            .iter()
            .filter(|(name, _)| !REPLAY_SETS.contains(&name.as_str()))
            .find(|(_, value)| value.contains("${{"))
        {
            return Some(format!("{name}={value} is resolved by GitHub, not here"));
        }
        if step.script.contains("${{") {
            return Some(format!(
                "a step's own script holds a GitHub expression: {}",
                step.script.replace('\n', " ")
            ));
        }
    }
    None
}

/// Does this log path belong to this job, and to no other?
///
/// SPELLED OUT PER JOB rather than derived in the workflow from
/// `${{ github.job }}`, because that expression is set by the runner and is only
/// defined inside a step — in a job's `env:` it evaluates to nothing, and every
/// job would then append to the same file. So the name is written by hand, and
/// what makes a hand-written name safe is that this checks it: a job copied from
/// another with the path not changed is the paste error, and it is silent
/// otherwise because the census would still print a number.
pub fn names_its_job(path: &str, job: &str) -> bool {
    path.rsplit('/').next() == Some(&format!("{job}.log"))
}

/// The manifests of the programs that MEASURE this census.
///
/// THE ONE STEP THAT CANNOT BE RECORDED is the step that builds them. The
/// recorder cannot record its own build, because the binary does not exist while
/// it runs, and cargo reads an empty `RUSTC_WRAPPER` as none — which is how that
/// step switches recording off for itself. `tools/restored` runs in the same
/// step for the reason the gate's own job is left out of its own census: an
/// instrument's build is not a cost this repository's CI pays for the
/// repository's sake, and counting it would put the instrument inside its own
/// reading.
///
/// The exemption is DERIVED from what the step does rather than kept as a job
/// name in a list, so it stays exactly one step wide: a second step that
/// switched recording off would have to build one of these to be excused.
///
/// What it costs is two dependency-free crates, which is why neither has
/// dependencies.
pub const INSTRUMENT_MANIFESTS: [&str; 2] = [RECORDER_MANIFEST, "tools/restored/Cargo.toml"];

/// The recorder's own manifest.
pub const RECORDER_MANIFEST: &str = "tools/rustc-log/Cargo.toml";

/// What one workflow declares: every job with the `run:` steps it is made of,
/// and the cached paths each job restores.
///
/// ONE VALUE BECAUSE THEY COME FROM ONE FILE, and every law below reads both.
/// The two populations are NOT the same population — this repository's two gate
/// jobs compile plenty and cache nothing — and a signature taking them as two
/// arguments is one a caller can fill from two different workflows. That is not
/// a hypothetical failure mode here: the census is per-workflow precisely
/// because two files may each declare a `validate`.
///
/// Both are read from the workflow rather than kept beside it, so a job added
/// tomorrow is in the population the moment it exists — the property the four
/// rounds before R1090 each broke by hand-maintaining a list.
#[derive(Debug, Clone, Default)]
pub struct Declared {
    /// Every job with a `run:` step, and the steps.
    pub jobs: BTreeMap<String, Vec<ci_plan::RunStep>>,
    /// The cached paths each job declares, in the order it declares them.
    ///
    /// Merged when one job declares more than one cache: the restore record
    /// brackets a REGION of the job, and what arrived in that region is what
    /// arrived under every path any of those caches holds. A duplicate spelling
    /// is dropped and the order of first mention kept, because that is the order
    /// the measuring step is written in.
    pub caches: BTreeMap<String, Vec<String>>,
    /// WHERE in each job's `steps:` list its caches sit.
    ///
    /// Kept beside the paths rather than folded into them because it answers a
    /// different question, and one the merge above destroys: the paths of two
    /// caches are one region, but that region has an outer edge on each side and
    /// a measurement has to be outside both of them. Every index counts the
    /// whole `steps:` list, so it is directly comparable with
    /// [`ci_plan::RunStep::index`].
    pub caches_at: BTreeMap<String, Vec<usize>>,
}

impl Declared {
    /// Read both populations out of one workflow's already-parsed halves.
    pub fn of(steps: &[ci_plan::RunStep], caches: &[ci_plan::CacheDeclaration]) -> Declared {
        let mut jobs: BTreeMap<String, Vec<ci_plan::RunStep>> = BTreeMap::new();
        for step in steps {
            jobs.entry(step.job.clone()).or_default().push(step.clone());
        }
        let mut cached: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut at: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for cache in caches {
            let paths = cached.entry(cache.owner.clone()).or_default();
            for path in &cache.paths {
                if !paths.contains(path) {
                    paths.push(path.clone());
                }
            }
            at.entry(cache.owner.clone()).or_default().push(cache.index);
        }
        Declared {
            jobs,
            caches: cached,
            caches_at: at,
        }
    }
}

/// Judge a census against the jobs one workflow declares.
///
/// `absent` is the jobs this census cannot hold, and it is a parameter rather
/// than a list here because the two callers derive it differently and both from
/// the machine: on a runner it is the one job the gate is itself running in,
/// whose build is still happening while it judges (`GITHUB_JOB`), and in a local
/// replay it is the jobs the replay refused to run because GitHub, not this
/// machine, resolves their environment.
pub fn judge(census: &Census, declared: &Declared, absent: &BTreeSet<String>) -> Vec<Refusal> {
    let mut refusals = Vec::new();
    refusals.extend(judge_restores(census, declared, absent));
    // HOW FAR THIS CENSUS REACHED, ASSERTED BEFORE ANYTHING IS READ FROM IT. The
    // jobs a caller declares absent are legitimate — the gate's own job on a
    // runner, the jobs a local replay cannot reproduce — but a census in which
    // that list has swallowed everything is not a clean file, it is a
    // measurement that did not happen.
    let covered = declared
        .jobs
        .keys()
        .filter(|job| !absent.contains(*job))
        .count();
    if covered < 2 {
        refusals.push(Refusal::CensusCoversTooFewJobs { covered });
    }
    for (job, steps) in &declared.jobs {
        if absent.contains(job) {
            continue;
        }
        for step in steps {
            let builds_the_recorder = INSTRUMENT_MANIFESTS
                .iter()
                .any(|manifest| step.script.contains(manifest));
            for variable in [WRAPPER_VARIABLE, rustc_log::LOG_VARIABLE] {
                // AN EMPTY VALUE IS UNSET to cargo, and that is exactly how the
                // recorder's own build step turns itself off — so it has to
                // count as missing here too, and be excused only there.
                if step.env.get(variable).is_none_or(|value| value.is_empty())
                    && !builds_the_recorder
                {
                    refusals.push(Refusal::StepIsNotRecorded {
                        job: job.clone(),
                        missing: variable.to_string(),
                    });
                }
            }
            if let Some(path) = step.env.get(rustc_log::LOG_VARIABLE) {
                if !path.is_empty() && !names_its_job(path, job) {
                    refusals.push(Refusal::LogIsNotNamedForItsJob {
                        job: job.clone(),
                        path: path.clone(),
                    });
                }
            }
        }
        match census.jobs.get(job) {
            None => refusals.push(Refusal::JobLeftNoRecord { job: job.clone() }),
            Some(log) if log.units.is_empty() => {
                refusals.push(Refusal::JobLeftNoRecord { job: job.clone() })
            }
            // ONE REFUSAL PER JOB, NOT TWO: a job with no compilations has no
            // seconds either, and saying both would report the same silence
            // twice under two names.
            Some(log) if log.compiled_micros() == 0 => {
                refusals.push(Refusal::JobRecordedNoTime { job: job.clone() })
            }
            Some(_) => {}
        }
    }
    for job in census.jobs.keys() {
        if !declared.jobs.contains_key(job) {
            refusals.push(Refusal::RecordFromNoJob { job: job.clone() });
        }
    }
    refusals.sort();
    refusals.dedup();
    refusals
}

/// What the WORKFLOW says about the restore measurements, judged with no census
/// at all.
///
/// SEPARATE FROM THE RECORD LAWS BECAUSE IT NEEDS NOTHING THAT RAN. Every other
/// restore law joins a record to a declaration, so it can only speak about jobs
/// that left one and is rightly silent about the jobs a caller declares absent.
/// These are properties of the file: they hold for a job that has never run,
/// for the job the gate is itself running in, and for a job a local replay
/// refused — and a wiring mistake caught here is caught before it costs a run.
///
/// WHY IT CAN BE ASKED AT ALL is `ci_plan::RunStep::index`. The `run:` steps and
/// the cache steps are two populations read out of one ordered list, and until
/// they carried a shared coordinate the question "does the measurement bracket
/// the restore?" had no answer in the file — R1102 closed it by OBSERVATION
/// instead (a job whose measurements both sit on one side reports an empty tree,
/// and an empty tree next to a restorable generation is a refusal), which
/// detects the mistake from a run rather than firing on the file that is wrong.
pub fn judge_wiring(declared: &Declared) -> Vec<Refusal> {
    let mut refusals = Vec::new();
    for (job, caches_at) in &declared.caches_at {
        // WHERE THE RECORD WILL BE WRITTEN — the same law the compilation log
        // has, on the other record.
        for step in declared.jobs.get(job).into_iter().flatten() {
            match step.env.get(restored::VARIABLE) {
                Some(path) if !path.is_empty() && restored::names_its_job(path, job) => {}
                Some(path) => refusals.push(Refusal::RestoreIsNotRecorded {
                    job: job.clone(),
                    path: path.clone(),
                }),
                None => refusals.push(Refusal::RestoreIsNotRecorded {
                    job: job.clone(),
                    path: String::new(),
                }),
            }
        }
        // EVERY INVOCATION AND NOT EVERY STEP: one step whose script runs the
        // measurement twice writes the second reading over the first, and
        // counting steps would call that one measurement.
        let measured: Vec<(usize, restored::Side)> = declared
            .jobs
            .get(job)
            .into_iter()
            .flatten()
            .flat_map(|step| {
                restored::sides_measured(&step.script)
                    .into_iter()
                    .map(|side| (step.index, side))
            })
            .collect();
        for side in restored::Side::BOTH {
            let ours: Vec<usize> = measured
                .iter()
                .filter(|(_, measured)| *measured == side)
                .map(|(index, _)| *index)
                .collect();
            let [measured_at] = ours.as_slice() else {
                refusals.push(Refusal::RestoreSideIsNotMeasuredOnce {
                    job: job.clone(),
                    side,
                    times: ours.len(),
                });
                continue;
            };
            // OUTSIDE EVERY CACHE THE JOB DECLARES, not merely outside one of
            // them: the record brackets a REGION, and a measurement that has
            // slipped between two cache steps reports the second one's arrival
            // as nothing.
            for cache_at in caches_at {
                let wrong = match side {
                    restored::Side::Before => measured_at >= cache_at,
                    restored::Side::After => measured_at <= cache_at,
                };
                if wrong {
                    refusals.push(Refusal::RestoreIsMeasuredOnTheWrongSide {
                        job: job.clone(),
                        side,
                        measured_at: *measured_at,
                        cache_at: *cache_at,
                    });
                }
            }
        }
    }
    // THE OTHER DIRECTION, which no record can report until one arrives: a job
    // measuring a restore that never happens.
    for (job, steps) in &declared.jobs {
        if declared.caches_at.contains_key(job) {
            continue;
        }
        if steps
            .iter()
            .any(|step| !restored::sides_measured(&step.script).is_empty())
        {
            refusals.push(Refusal::RestoreIsMeasuredWithNoCache { job: job.clone() });
        }
    }
    refusals
}

/// What every job says it STARTED FROM, held against what its cache declares.
///
/// SEPARATE FROM THE COMPILATION LAWS because it is a different population: the
/// jobs with a cache, which is not the jobs that compile. This repository's two
/// gate jobs compile plenty and cache nothing, and requiring a restore record of
/// them would refuse a workflow that is right.
fn judge_restores(census: &Census, declared: &Declared, absent: &BTreeSet<String>) -> Vec<Refusal> {
    let mut refusals = judge_wiring(declared);
    for (job, paths) in &declared.caches {
        if absent.contains(job) || !declared.jobs.contains_key(job) {
            continue;
        }
        match census.restored.get(job) {
            None => refusals.push(Refusal::JobDidNotSayWhatItRestored { job: job.clone() }),
            Some(Err(why)) => refusals.push(Refusal::RestoreRecordIsMalformed {
                job: job.clone(),
                why: why.to_string(),
            }),
            Some(Ok(record)) => {
                if record.measured() != paths.iter().map(String::as_str).collect::<Vec<_>>() {
                    refusals.push(Refusal::RestoreRecordMeasuredOtherPaths {
                        job: job.clone(),
                        measured: record
                            .measured()
                            .iter()
                            .map(|one| one.to_string())
                            .collect(),
                        declared: paths.clone(),
                    });
                }
                // THE CONTRADICTION IS A REFUSAL AND NOT A THIRD READING. The
                // other three states of `restored::Warmth` are states of the
                // world; this one is the two instruments disagreeing, and a
                // census cannot be quoted while it is unresolved.
                if record.warmth() == restored::Warmth::HitThatBroughtNothing {
                    refusals.push(Refusal::RestoredNothingAfterAnExactHit { job: job.clone() });
                }
            }
        }
    }
    for (file, record) in &census.restored {
        if absent.contains(file) {
            continue;
        }
        if !declared.caches.contains_key(file) {
            refusals.push(Refusal::RestoreRecordFromAJobWithNoCache { job: file.clone() });
            continue;
        }
        // THE RECORD CARRIES THE JOB THE RUNNER NAMED, and the file carries the
        // job the workflow spelled by hand. Two jobs writing one path leaves
        // exactly this disagreement behind, and it is the one direction the
        // check above cannot see: the surviving record is well-formed and
        // measures real paths.
        if let Ok(record) = record {
            if &record.job != file {
                refusals.push(Refusal::RestoreRecordNamesAnotherJob {
                    file: file.clone(),
                    said: record.job.clone(),
                });
            }
        }
    }
    refusals
}

/// Load every `<job>.log` in a directory into a census, except the jobs this
/// census cannot be about.
///
/// The file name IS the job id, which is what lets the wiring in the workflow be
/// one line per job with nothing to keep in step.
///
/// `absent` IS DROPPED HERE AND NOT ONLY IN [`judge`], and the reason is the
/// gate's own job. It runs `cargo run` on this crate with the recorder active,
/// so by the time it reads the directory its OWN log is sitting in it beside the
/// downloaded ones — a partial log, of a build still in progress, from the one
/// job whose compilations are an artefact of measuring rather than a cost CI
/// pays. Skipping it in the verdict but counting it in the totals would put the
/// instrument into its own reading.
pub fn load(directory: &Path, absent: &BTreeSet<String>) -> std::io::Result<Census> {
    let mut census = Census::default();
    for entry in std::fs::read_dir(directory)? {
        let path = entry?.path();
        let extension = path.extension().and_then(|extension| extension.to_str());
        let Some(job) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        if absent.contains(job) {
            continue;
        }
        // TWO RECORDS PER JOB, IN ONE DIRECTORY, READ BY ONE LOADER: what the
        // job compiled and what it started from. They are uploaded together
        // because they are one measurement — the counts are of a cold build by
        // construction, so the state is the units they are in.
        match extension {
            Some("log") => {
                let text = std::fs::read_to_string(&path)?;
                census.jobs.insert(job.to_string(), read_log(&text));
            }
            Some("restored") => {
                let text = std::fs::read_to_string(&path)?;
                census
                    .restored
                    .insert(job.to_string(), restored::decode(&text));
            }
            _ => continue,
        }
    }
    Ok(census)
}
