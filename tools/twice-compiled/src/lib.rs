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
//! WHAT IT ASSERTS TODAY is that it reached every job: a workflow job that
//! issues a cargo command and leaves no record is the empty answer that reads
//! like a clean one, which is the failure this repository keeps meeting. The
//! duplication itself is REPORTED rather than refused, because the number is
//! what licenses the repair and the repair is what makes a limit assertable.

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
    /// Each unit this job compiled, and HOW MANY TIMES it compiled it.
    ///
    /// A count rather than a set, because a job can compile one unit twice: a
    /// job holding two `target` directories — the root one and a tool
    /// workspace's — pays for a shared dependency in both. Keeping only the set
    /// makes those repeats vanish from every breakdown while staying in the
    /// total, and two numbers in one report that do not add up is a report
    /// nobody can act on.
    pub units: BTreeMap<Unit, usize>,
}

impl JobLog {
    /// Keyed compilations, counted with their repeats.
    pub fn compilations(&self) -> usize {
        self.units.values().sum()
    }

    /// Compilations this job paid for twice by itself.
    pub fn repeats(&self) -> usize {
        self.compilations() - self.units.len()
    }
}

/// Read one job's log.
pub fn read_log(text: &str) -> JobLog {
    let mut log = JobLog::default();
    for record in rustc_log::decode_all(text) {
        log.invocations += 1;
        match read(&record) {
            Invocation::Probe => log.probes += 1,
            Invocation::Unkeyed => log.unkeyed += 1,
            Invocation::Compilation(unit) => *log.units.entry(*unit).or_default() += 1,
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
    pub fn surplus_by_crate(&self) -> BTreeMap<&str, usize> {
        let mut paid: BTreeMap<&str, usize> = BTreeMap::new();
        for log in self.jobs.values() {
            for (unit, times) in &log.units {
                *paid.entry(unit.crate_name.as_str()).or_default() += times;
            }
        }
        for unit in self.distinct() {
            if let Some(count) = paid.get_mut(unit.crate_name.as_str()) {
                *count -= 1;
            }
        }
        paid.retain(|_, surplus| *surplus > 0);
        paid
    }

    /// How many units each pair of jobs both compile.
    pub fn pairwise(&self) -> BTreeMap<(&str, &str), usize> {
        let mut out = BTreeMap::new();
        let jobs: Vec<(&str, &JobLog)> = self
            .jobs
            .iter()
            .map(|(name, log)| (name.as_str(), log))
            .collect();
        for (index, (left, left_log)) in jobs.iter().enumerate() {
            for (right, right_log) in &jobs[index + 1..] {
                let shared = left_log
                    .units
                    .keys()
                    .filter(|unit| right_log.units.contains_key(*unit))
                    .count();
                if shared > 0 {
                    out.insert((*left, *right), shared);
                }
            }
        }
        out
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
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refusal::JobLeftNoRecord { job } => write!(
                f,
                "job `{job}` recorded no compilation — a job whose recorder is \
                 unwired reads exactly like a job with nothing to build"
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
        }
    }
}

/// The environment variable naming the recorder, as cargo spells it.
pub const WRAPPER_VARIABLE: &str = "RUSTC_WRAPPER";

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

/// The recorder's own manifest.
///
/// THE ONE STEP THAT CANNOT BE RECORDED is the step that builds the recorder,
/// because the binary does not exist while it runs — and cargo reads an empty
/// `RUSTC_WRAPPER` as none, which is how that step switches recording off for
/// itself. The exemption is DERIVED from what the step does rather than kept as
/// a job name in a list, so it stays exactly one step wide: a second step that
/// switched recording off would have to build the recorder to be excused.
///
/// What it costs is the recorder's own compilation, which is one dependency-free
/// crate and is why it has no dependencies.
pub const RECORDER_MANIFEST: &str = "tools/rustc-log/Cargo.toml";

/// Every job one workflow declares, with the `run:` steps it is made of.
///
/// Read from the workflow rather than kept beside it, so a job added tomorrow is
/// in the population the moment it exists — the property the four rounds before
/// R1090 each broke by hand-maintaining a list.
pub fn declared_jobs(steps: &[ci_plan::RunStep]) -> BTreeMap<String, Vec<ci_plan::RunStep>> {
    let mut out: BTreeMap<String, Vec<ci_plan::RunStep>> = BTreeMap::new();
    for step in steps {
        out.entry(step.job.clone()).or_default().push(step.clone());
    }
    out
}

/// Judge a census against the jobs one workflow declares.
///
/// `absent` is the jobs this census cannot hold, and it is a parameter rather
/// than a list here because the two callers derive it differently and both from
/// the machine: on a runner it is the one job the gate is itself running in,
/// whose build is still happening while it judges (`GITHUB_JOB`), and in a local
/// replay it is the jobs the replay refused to run because GitHub, not this
/// machine, resolves their environment.
pub fn judge(
    census: &Census,
    declared: &BTreeMap<String, Vec<ci_plan::RunStep>>,
    absent: &BTreeSet<String>,
) -> Vec<Refusal> {
    let mut refusals = Vec::new();
    for (job, steps) in declared {
        if absent.contains(job) {
            continue;
        }
        for step in steps {
            let builds_the_recorder = step.script.contains(RECORDER_MANIFEST);
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
        if census.jobs.get(job).is_none_or(|log| log.units.is_empty()) {
            refusals.push(Refusal::JobLeftNoRecord { job: job.clone() });
        }
    }
    for job in census.jobs.keys() {
        if !declared.contains_key(job) {
            refusals.push(Refusal::RecordFromNoJob { job: job.clone() });
        }
    }
    refusals.sort();
    refusals.dedup();
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
        if path.extension().and_then(|extension| extension.to_str()) != Some("log") {
            continue;
        }
        let Some(job) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        if absent.contains(job) {
            continue;
        }
        let text = std::fs::read_to_string(&path)?;
        census.jobs.insert(job.to_string(), read_log(&text));
    }
    Ok(census)
}
