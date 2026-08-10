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
use std::path::{Path, PathBuf};

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
    /// The program the recorder ran, whole and as the record spells it.
    ///
    /// WHICH COMPILER DID THE WORK IS PART OF WHAT THE WORK WAS, and this crate
    /// has said so since its first line — the record keeps the compiler's path
    /// because "the MSRV job runs a different `rustc` over the same sources".
    /// The key dropped it, and it is here for the reason the fields above it are
    /// given: so that a cargo release which narrows what goes into `-C metadata`
    /// makes this reader SPLIT units rather than merge them.
    ///
    /// IT SPLITS NOTHING TODAY, AND THAT IS MEASURED RATHER THAN HOPED. Adding
    /// it to the key left the floor of a real eight-job census at 2707 distinct
    /// units, exactly where it was — so no two records that shared a unit
    /// disagreed about their compiler. The MSRV job's compilations were already
    /// separate, and so were the `clippy-driver` passes `cargo clippy` runs over
    /// every workspace member in three of these jobs. The duplication this crate
    /// reports was NOT inflated by either, which is the opposite of what the
    /// round that added this field expected to find.
    ///
    /// THE WHOLE STRING, machine-specific prefix and all, because it is what the
    /// record says and this reader invents nothing. Two censuses taken on
    /// different machines therefore share no unit, which is why [`compare`] is
    /// for two runs of the same CI rather than a run and a local replay.
    pub driver: String,
    /// Whose source the compiler read — see [`Origin`].
    ///
    /// PART OF THE KEY AND NOT BESIDE IT, for the reason the fields above it
    /// are: `metadata` already hashes the package's source, so two units that
    /// agree on everything else cannot honestly disagree here, and if a cargo
    /// release ever makes them the split under-reports duplication rather than
    /// inventing it.
    pub origin: Origin,
}

/// Where the file one compilation read lives.
///
/// WHY A CENSUS THAT CANNOT SAY THIS CANNOT SAY WHAT A CACHE MISSED. Every job
/// in this repository's workflow caches `~/.cargo/registry` and `~/.cargo/git`,
/// and both of those hold SOURCES rather than compiled artifacts: a job that
/// restores them and no `target` still compiles every crate it depends on. So
/// "how many of these compilations were of crates cargo fetched" is the question
/// that separates a cache which is saving nothing from one which is, and until
/// this existed nothing could be asked it — the input path is in every record
/// and [`read`] was dropping it on the floor.
///
/// READ OFF CARGO'S LAYOUT, NOT OFF THIS REPOSITORY'S WORKFLOW. Where cargo
/// unpacks a registry crate is a fact about cargo and is true in a checkout that
/// caches nothing; which of those trees a workflow chooses to carry is a
/// separate reading, and [`compare`] — which holds two censuses that may be of
/// two different commits — has no workflow to consult.
///
/// WHICH WAY IT IS WRONG WHEN IT IS WRONG. Both fetched variants demand the rest
/// of cargo's layout below the pair of directories that names them, so a crate
/// of this repository's own called `registry` with a `src/` in it reads as
/// [`Origin::Tree`] rather than as a dependency. That direction is chosen: this
/// reading exists to find out whether the runner recompiles dependencies, and a
/// rule that guessed generously would be answering its own question.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Origin {
    /// `<cargo home>/registry/src/<index>/<name>-<version>/…` — a crate cargo
    /// downloaded from a registry and unpacked.
    Registry,
    /// `<cargo home>/git/checkouts/<name>-<hash>/<rev>/…` — a git dependency.
    Git,
    /// Anything else: the sources this checkout holds, and a path dependency's.
    Tree,
}

impl Origin {
    /// Did cargo fetch this source rather than find it in the checkout?
    ///
    /// THE TWO TREES THIS IS TRUE OF ARE THE TWO EVERY CACHE IN THIS WORKFLOW
    /// CARRIES, which is what makes it the predicate to ask of a cache: a
    /// fetched crate compiled anyway is one whose sources arrived and whose
    /// compiled form did not.
    pub fn fetched(self) -> bool {
        matches!(self, Origin::Registry | Origin::Git)
    }

    /// How a reader says it.
    pub fn why(self) -> &'static str {
        match self {
            Origin::Registry => "fetched from a registry",
            Origin::Git => "fetched from a git dependency",
            Origin::Tree => "in the checkout",
        }
    }

    /// Where a compiler's input file lived.
    pub fn of(input: &str) -> Origin {
        let parts: Vec<&str> = Path::new(input)
            .components()
            .filter_map(|part| match part {
                std::path::Component::Normal(word) => word.to_str(),
                _ => None,
            })
            .collect();
        for window in parts.windows(4) {
            // FOUR COMPONENTS, NOT TWO. `registry/src` on its own is a directory
            // pair any repository may have; what says cargo made it is the index
            // directory and the `<name>-<version>` under it, and demanding both
            // is what keeps a crate of ours called `registry` out of the answer.
            if window[0] == "registry" && window[1] == "src" && names_a_package(window[3]) {
                return Origin::Registry;
            }
            if window[0] == "git" && window[1] == "checkouts" {
                return Origin::Git;
            }
        }
        Origin::Tree
    }
}

/// Does this component name an unpacked crate — `<name>-<version>`?
///
/// A VERSION BEGINS WITH A DIGIT, which is the whole of the test: cargo's own
/// directory names are the package name, a `-`, and the semantic version, and
/// nothing weaker distinguishes `serde-1.0.219` from an ordinary directory whose
/// name happens to hold a hyphen.
fn names_a_package(component: &str) -> bool {
    component.rsplit_once('-').is_some_and(|(name, version)| {
        !name.is_empty() && version.starts_with(|first: char| first.is_ascii_digit())
    })
}

/// The rustc flags whose value is the NEXT word rather than part of this one.
///
/// WHY THERE IS A LIST HERE AT ALL, in a crate whose every other law is derived.
/// rustc takes exactly one free-standing input, so finding it means knowing
/// which of the words that do not begin with `-` belong to somebody else:
/// `--crate-name ci_plan` is two words and `ci_plan` is not a path. Each entry
/// is a flag rustc defines as taking a separated value; a flag written joined
/// (`--edition=2021`) needs no entry, because its value never becomes a word.
///
/// A LIST THAT IS WRONG IS CAUGHT RATHER THAN BELIEVED, which is what makes it
/// safe to be a list. A missing entry leaves two free-standing words and a
/// spurious one leaves none, and BOTH are [`Invocation::Unplaced`] — a refusal
/// this census counts and a job's worth of which it refuses over. There is no
/// arrangement of this list that produces a wrong origin quietly.
const FLAGS_TAKING_THE_NEXT_WORD: &[&str] = &[
    "--crate-name",
    "--crate-type",
    "--edition",
    "--emit",
    "--print",
    "--out-dir",
    "-o",
    "--explain",
    "--target",
    "--cfg",
    "--check-cfg",
    "--extern",
    "--sysroot",
    "--error-format",
    "--color",
    "--cap-lints",
    "--remap-path-prefix",
    "--json",
    "--diagnostic-width",
    "--env-set",
    "--crate-attr",
    "--codegen",
    "-C",
    "-L",
    "-l",
    "-A",
    "--allow",
    "-W",
    "--warn",
    "-D",
    "--deny",
    "-F",
    "--forbid",
    "--force-warn",
    "-Z",
];

/// The one file the compiler was told to read.
///
/// `None` WHEN THERE IS NOT EXACTLY ONE, rather than the first or the last of
/// several. rustc accepts a single input, so a reader that finds two has
/// mistaken somebody's value for a path and a reader that finds none has eaten
/// the path as somebody's value — and in both cases the honest output is that
/// this invocation was not placed, not a guess at which candidate to believe.
fn input_of(argv: &[String]) -> Result<&str, Vec<String>> {
    let mut free: Vec<&str> = Vec::new();
    let mut expecting_a_value = false;
    for word in argv {
        if expecting_a_value {
            expecting_a_value = false;
            continue;
        }
        if word.starts_with('-') {
            expecting_a_value = FLAGS_TAKING_THE_NEXT_WORD.contains(&word.as_str());
            continue;
        }
        free.push(word.as_str());
    }
    match free.as_slice() {
        [only] => Ok(only),
        many => Err(many.iter().map(|word| (*word).to_string()).collect()),
    }
}

/// One compilation: what was compiled, and where its output was written.
///
/// THE DESTINATION IS NOT PART OF THE UNIT AND MUST NOT BECOME ONE. A job that
/// holds two `target` directories compiles a shared crate into both, and that
/// second compilation is the FINDING — the within-job repetition no merge of
/// jobs can remove. Putting the destination in the key would split those two
/// into two units and erase it, reporting a job that pays twice as one that
/// pays once for two different things.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Compiled {
    /// What was compiled.
    pub unit: Unit,
    /// Where it was written, as [`Destination`] reads it.
    pub into: Destination,
}

/// Where one compilation's output went, and whose source it read.
///
/// THE TWO AXES TRAVEL TOGETHER BECAUSE THE QUESTION NEEDS BOTH. "This job
/// restored its `target` exactly and still compiled 461 crates cargo had
/// fetched" is answerable only by crossing them: the fetched share alone cannot
/// say whether that work went into the tree the cache brought back or into one
/// no cache holds, and those are opposite findings about the same number.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Destination {
    /// `--out-dir`, spelled as the record spells it.
    ///
    /// `None` when the compiler was given none, which rustc allows and cargo
    /// does not do. Kept as a class rather than dropped, for the reason every
    /// other class this reader cannot key is kept: a destination it could not
    /// read must not be counted as a destination it read as covered.
    pub out_dir: Option<String>,
    /// Whose source the compilation read.
    pub origin: Origin,
}

/// What one `rustc` invocation was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Invocation {
    /// A compilation cargo drives.
    Compilation(Box<Compiled>),
    /// `rustc -vV`, `--print=cfg`, the crate-type probe — cargo asking the
    /// compiler about itself. Real processes, no compilation.
    Probe,
    /// A compilation with no `-C metadata`, which is what a build script running
    /// `rustc` itself looks like (`autocfg` and friends). Counted and printed
    /// rather than dropped, because a class this reader cannot key is a class it
    /// must not silently call absent.
    Unkeyed,
    /// A keyed compilation whose arguments do not hold exactly one free-standing
    /// input file, so there is no path to say the source came from.
    ///
    /// KEPT AS ITS OWN CLASS FOR THE REASON `Unkeyed` IS, and with more at
    /// stake: a unit whose origin were guessed would be counted under a heading
    /// — "compiled although cargo had fetched it" — that decides whether this
    /// repository builds a compile cache at all. So an invocation this reader
    /// cannot place leaves the units entirely and is counted here, where
    /// [`Refusal::JobRanCompilationsThisReaderCannotPlace`] can refuse over it.
    Unplaced(Unplaceable),
}

/// A keyed compilation this reader could not place, and how it failed to.
///
/// NAMED RATHER THAN COUNTED, because the two things a person fixing this needs
/// are which compilations were lost and WHICH DIRECTION the reading was wrong
/// in, and a refusal that reports a bare number sends them to read a megabyte of
/// records to recover both.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Unplaceable {
    /// `--crate-name`.
    pub crate_name: String,
    /// The free-standing words the arguments held — never exactly one.
    ///
    /// THE WORDS AND NOT THEIR COUNT. Empty says a flag in
    /// [`FLAGS_TAKING_THE_NEXT_WORD`] ate the input path; two or more says a
    /// flag taking a separated value is missing from that list, and then the
    /// words themselves are the whole of the repair — one of them is the path
    /// and the others name the flag that should have consumed them. A count
    /// sends its reader back to the records to recover what this already had.
    pub candidates: Vec<String>,
}

impl Unplaceable {
    /// How a reader says what went wrong, with the words that say it.
    pub fn why(&self) -> String {
        match self.candidates.as_slice() {
            [] => "no free-standing word at all — a flag in this reader's list \
                   ate the input path"
                .to_string(),
            many => format!(
                "{} free-standing words where one path should be, so a flag \
                 taking a separated value is missing from this reader's list: {}",
                many.len(),
                many.join(" ")
            ),
        }
    }
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

    /// Take in what another row of this table cost.
    ///
    /// Both halves, because they are the pair this type exists to keep together:
    /// a fold that summed the compilations and left the seconds behind would
    /// give every breakdown a count no price could be read against, which is the
    /// mistake `rustc-log`'s own header names.
    pub fn absorb(&mut self, other: Cost) {
        self.times += other.times;
        self.micros = self.micros.saturating_add(other.micros);
    }
}

/// The words the compiler itself was given, with the chain that reached it gone.
///
/// CARGO HANDS A WRAPPER THE NEXT PROGRAM IN THE CHAIN as that wrapper's first
/// argument. `RUSTC_WRAPPER` is run as `<wrapper> <rustc> <arguments…>`, and
/// when a `RUSTC_WORKSPACE_WRAPPER` is set as well — which is exactly what
/// `cargo clippy` does, for workspace members only — as `<wrapper> <workspace
/// wrapper> <rustc> <arguments…>`. The recorder writes down the program it ran
/// and the words it passed on, so a record of a clippy pass carries TWO program
/// paths ahead of the first flag.
///
/// THE RULE IS THAT CONTRACT AND NOT A LIST OF PROGRAM NAMES: the chain is every
/// word before the first one beginning with `-`, and the compiler's own
/// arguments start there. It holds for a chain of any depth and for a driver
/// this repository has never heard of, which a list of names does not.
fn arguments_of(argv: &[String]) -> &[String] {
    let first_flag = argv
        .iter()
        .position(|word| word.starts_with('-'))
        .unwrap_or(argv.len());
    &argv[first_flag..]
}

/// Read one record. The first word is the program that ran; the rest are what it
/// was passed, which for a wrapper chain begins with the next program along.
pub fn read(record: &[String]) -> Invocation {
    let driver = record[0].clone();
    let argv = arguments_of(&record[1..]);
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
    let input = match input_of(argv) {
        Ok(input) => input,
        Err(candidates) => {
            return Invocation::Unplaced(Unplaceable {
                crate_name: crate_name.to_string(),
                candidates,
            })
        }
    };
    let origin = Origin::of(input);
    Invocation::Compilation(Box::new(Compiled {
        unit: Unit {
            crate_name: crate_name.to_string(),
            metadata: metadata.to_string(),
            emit: flag_value(argv, "--emit").unwrap_or_default().to_string(),
            crate_types: crate_types(argv),
            test: argv.iter().any(|word| word == "--test"),
            driver,
            origin,
        },
        into: Destination {
            out_dir: flag_value(argv, "--out-dir").map(str::to_string),
            origin,
        },
    }))
}

// --- one job ----------------------------------------------------------------

/// What one job compiled, grouped by which of its cache's paths would hold the
/// output.
///
/// THE TWO HALVES ARE ONE ANSWER AND ARE BUILT TOGETHER. "The cache spared this"
/// and "no cache could have" are the same walk over the same destinations, and a
/// caller assembling them from two calls could be given a pair that does not sum
/// to the job.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Coverage {
    /// Per declared cache path that resolves against the checkout, what was
    /// written under it — split by whose source it was.
    pub held: BTreeMap<String, BTreeMap<Origin, Cost>>,
    /// What was written where no declared path of this job's cache reaches.
    ///
    /// WORK NO RESTORE OF THIS CACHE COULD EVER SPARE, which is a different
    /// finding from a cache that missed: this repository's jobs build side
    /// workspaces whose `target` directories are in nobody's `path:` list.
    ///
    /// KEYED BY THE TREE AND NOT SUMMED INTO ONE NUMBER, because the repair is
    /// a `path:` line and a reader owed one needs to know WHICH directory to
    /// write. One total says a cache is not reaching; the rows say where to
    /// point it.
    pub outside: BTreeMap<String, BTreeMap<Origin, Cost>>,
}

impl Destination {
    /// The build directory this compilation's output sits under.
    ///
    /// CARGO'S LAYOUT IS `<target directory>/<profile>/…`, so the output of one
    /// workspace arrives under dozens of `--out-dir` values — one per build
    /// script, one per profile — and a report listing them is a report nobody
    /// reads. What a reader needs is the tree, because the repair is a `path:`
    /// line naming one.
    ///
    /// THE RULE IS THE LAST COMPONENT CALLED `target`, WHICH IS AN ASSUMPTION
    /// AND IS STATED. It is cargo's default and this repository sets no
    /// `CARGO_TARGET_DIR`; a destination with no such component is returned
    /// WHOLE rather than cut at a guess, so a checkout that renamed it prints
    /// long rows instead of wrong ones.
    pub fn tree(&self) -> Option<&str> {
        let dir = self.out_dir.as_deref()?;
        match dir.rfind("/target/") {
            Some(at) => Some(&dir[..at + "/target".len()]),
            None => Some(dir),
        }
    }
}

/// What one job compiled.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JobLog {
    /// Every `rustc` the job ran, probes included.
    pub invocations: usize,
    /// Invocations that were cargo asking the compiler about itself.
    pub probes: usize,
    /// Compilations with no `-C metadata` — see [`Invocation::Unkeyed`].
    pub unkeyed: usize,
    /// Where this job's compilations were written, crossed with whose source
    /// they read — see [`Destination`].
    ///
    /// A SECOND AXIS AND NOT A FINER KEY, for the reason [`Compiled`] gives.
    /// Its rows sum to the same compilations and the same seconds as
    /// [`JobLog::units`] does, which is asserted rather than assumed: two
    /// breakdowns of one population that disagree are one report nobody can act
    /// on, and nothing about the way each is folded makes them agree by
    /// construction.
    pub written: BTreeMap<Destination, Cost>,
    /// Keyed compilations whose input file could not be placed, by what they
    /// were and how the reading failed — see [`Invocation::Unplaced`].
    ///
    /// THE COST IS KEPT TOO, because these seconds are inside the job's total
    /// and outside [`JobLog::compiled_micros`], and a reader owed the size of
    /// what is missing cannot be handed only its count.
    pub unplaced: BTreeMap<Unplaceable, Cost>,
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

    /// What this job compiled, split by whose source it was.
    ///
    /// DERIVED AND NOT STORED, because [`Unit`] already carries the origin and a
    /// second copy of the same fact is a second thing to keep true. An origin no
    /// unit had is absent rather than zero — the caller that wants "and none at
    /// all from a registry" asks for it and is told nothing is there, which is
    /// the honest shape when the alternative is a row invented by the reader.
    pub fn by_origin(&self) -> BTreeMap<Origin, Cost> {
        let mut out: BTreeMap<Origin, Cost> = BTreeMap::new();
        for (unit, cost) in &self.units {
            out.entry(unit.origin).or_default().absorb(*cost);
        }
        out
    }

    /// What this job compiled into trees NONE of its cache's paths hold,
    /// split by whose source it was.
    ///
    /// THE OTHER HALF OF "DID THE CACHE SPARE THIS". A restore can only spare
    /// work whose output it brought back, so a compilation written somewhere the
    /// cache does not reach is work no hit of that cache could ever have
    /// avoided — and this repository's jobs build side workspaces whose `target`
    /// directories are not in any `path:` list. Until this existed, a job that
    /// restored 27 GB exactly and compiled 461 fetched crates read as a cache
    /// that had failed, when it may be a cache that was never asked.
    ///
    /// `root` IS THE CHECKOUT, because a workflow's `path:` is relative to it
    /// and an `--out-dir` is absolute. A declared path that does not resolve
    /// against the checkout — `~/.cargo/registry`, which only a shell expands —
    /// covers nothing here, which is right for a cargo home and is stated
    /// because it is a silence: those paths hold no compiler output at all.
    ///
    /// A COMPILATION WITH NO DESTINATION IS IN NEITHER HALF. It is counted by
    /// [`JobLog::wrote_nowhere_named`] and refused over, because "written
    /// outside the cache" and "written where this reader could not look" are
    /// the same zero to everything downstream.
    ///
    /// `None` WHEN THE CENSUS WAS TAKEN ON ANOTHER MACHINE, which is the whole
    /// reason this returns an option. An `--out-dir` written on a runner begins
    /// `/home/runner/work/…` and no path under a different checkout will ever
    /// prefix it, so a reader that answered anyway would report every
    /// compilation as written outside the cache — a catastrophic-looking number
    /// produced by looking in the wrong place. Not one destination under `root`
    /// is that, and it is said rather than answered.
    pub fn coverage(&self, root: &Path, cached: &[String]) -> Option<Coverage> {
        if !self.written.iter().any(|(into, _)| {
            into.out_dir
                .as_ref()
                .is_some_and(|dir| Path::new(dir).starts_with(root))
        }) {
            return None;
        }
        // LONGEST DECLARED PATH WINS, and the paths are made absolute first so
        // there is nothing to be ambiguous about: `<root>/bench/target` does not
        // begin with `<root>/target`, however much the two spellings share.
        let mut reachable: Vec<(&String, PathBuf)> = cached
            .iter()
            .filter(|path| !path.starts_with('~'))
            .map(|path| (path, root.join(path)))
            .collect();
        reachable.sort_by_key(|(_, absolute)| std::cmp::Reverse(absolute.components().count()));
        let mut found = Coverage::default();
        for (into, cost) in &self.written {
            let Some(dir) = &into.out_dir else { continue };
            let dir = Path::new(dir);
            match reachable
                .iter()
                .find(|(_, absolute)| dir.starts_with(absolute))
            {
                Some((declared, _)) => found
                    .held
                    .entry((*declared).clone())
                    .or_default()
                    .entry(into.origin)
                    .or_default()
                    .absorb(*cost),
                None => found
                    .outside
                    .entry(
                        into.tree()
                            .unwrap_or(dir.to_str().unwrap_or(""))
                            .to_string(),
                    )
                    .or_default()
                    .entry(into.origin)
                    .or_default()
                    .absorb(*cost),
            }
        }
        Some(found)
    }

    /// Compilations whose arguments named no `--out-dir` at all.
    pub fn wrote_nowhere_named(&self) -> usize {
        self.written
            .iter()
            .filter(|(into, _)| into.out_dir.is_none())
            .map(|(_, cost)| cost.times)
            .sum()
    }

    /// Compilations this reader could not place, counted with their repeats.
    pub fn unplaced_compilations(&self) -> usize {
        self.unplaced.values().map(|cost| cost.times).sum()
    }

    /// What this job spent compiling crates cargo had fetched.
    ///
    /// THE NUMBER A CACHE IS JUDGED BY: every cache in this workflow carries
    /// `~/.cargo/registry` and `~/.cargo/git`, so a job whose restore hit and
    /// whose fetched cost is still large is a job whose cache brought sources
    /// and no compiled form of them.
    pub fn fetched(&self) -> Cost {
        let mut out = Cost::default();
        for (unit, cost) in &self.units {
            if unit.origin.fetched() {
                out.absorb(*cost);
            }
        }
        out
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
            Invocation::Unplaced(what) => log.unplaced.entry(what).or_default().add(record.micros),
            Invocation::Compilation(compiled) => {
                log.units
                    .entry(compiled.unit)
                    .or_default()
                    .add(record.micros);
                log.written
                    .entry(compiled.into)
                    .or_default()
                    .add(record.micros);
            }
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
    /// What each cache restore BROUGHT: the record `tools/restored` leaves
    /// either side of one `actions/cache` step.
    ///
    /// THE STATE THE COUNTS WERE TAKEN IN, kept beside them rather than derived
    /// from them, because it cannot be derived from them: a job that compiled
    /// 773 units because its tree was empty and a job that compiled 773 units
    /// because its tree was stale are the same census and different runs. The
    /// error is kept rather than dropped so that a record which did not decode
    /// travels as a refusal and not as a job with no cache at all.
    ///
    /// KEYED BY THE FILE NAME AND NOT BY THE JOB, which is not the same choice
    /// [`Census::jobs`] makes and is a consequence of R1117. A compilation log
    /// IS a job's; a restore record is one CACHE's, so a job declaring two
    /// leaves two files and a map keyed by the job would hold whichever was read
    /// last — the cheap registry's price standing in for the build directory's.
    /// The name is the only handle a record that did NOT decode has, which is
    /// why it is the key rather than [`restored::Restore`]: a malformed record
    /// has no readable job and no readable cache, and dropping it would report
    /// the loudest defect available here as a job that simply said nothing.
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

    /// What CI pays for, split by whose source each compilation read.
    ///
    /// SUMMED OVER JOBS AND NOT OVER DISTINCT UNITS, because the question this
    /// answers is what the runners spent: one dependency compiled in eight jobs
    /// is eight compilations of a crate eight caches had already fetched.
    pub fn by_origin(&self) -> BTreeMap<Origin, Cost> {
        let mut out: BTreeMap<Origin, Cost> = BTreeMap::new();
        for log in self.jobs.values() {
            for (origin, cost) in log.by_origin() {
                out.entry(origin).or_default().absorb(cost);
            }
        }
        out
    }

    /// Compilations no job could place — see [`Invocation::Unplaced`].
    pub fn unplaced(&self) -> usize {
        self.jobs.values().map(JobLog::unplaced_compilations).sum()
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

    /// The state each RESTORE began in — the control variable every count above
    /// is taken under.
    ///
    /// ONLY THE RECORDS THAT DECODED, because the alternative is a restore
    /// appearing here with a state invented for it; the ones that did not are
    /// refusals and travel as those.
    ///
    /// KEYED BY THE RECORD'S OWN ACCOUNT OF ITSELF and not by the file it
    /// arrived in: the file name is checked to be its job's and says nothing
    /// about which cache, so joining on it would put two of one job's restores
    /// under one key and lose one of them.
    pub fn started(&self) -> BTreeMap<restored::Restore, restored::Warmth> {
        self.restored
            .values()
            .filter_map(|record| {
                let record = record.as_ref().ok()?;
                Some((record.restore(), record.warmth()))
            })
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
    /// A job ran compilations whose input file this reader could not place, so
    /// the census cannot say whose sources that job compiled.
    ///
    /// THE THIRD WAY AN INSTRUMENT GOES SILENT, and the one that is invisible in
    /// every total: the counts still add up, the seconds are still there, and
    /// only the SPLIT is short — a job with four hundred unplaced compilations
    /// reports a small fetched cost and a small tree cost and looks like a job
    /// with little to build. The split is what says whether a cache that brought
    /// sources brought anything worth having, so it must not be read off a
    /// population this reader silently lost part of.
    JobRanCompilationsThisReaderCannotPlace {
        job: String,
        compilations: usize,
        /// What they were, so the refusal sends its reader to a crate rather
        /// than to a megabyte of records.
        what: Vec<Unplaceable>,
    },
    /// A job ran compilations that named no `--out-dir`, so nothing can say
    /// whether the cache holds where their output went.
    ///
    /// THE SAME ZERO AS WORK THE CACHE DOES HOLD. A destination this reader
    /// never saw is absent from both halves of [`Coverage`], and both halves
    /// still sum and still print — the job simply reads as having compiled less
    /// than it did, in whichever direction happens to matter.
    JobWroteWhereNothingSaid { job: String, compilations: usize },
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
    /// One of a job's caches left no record of what it brought.
    ///
    /// THE STATE IS A CONTROL VARIABLE OF THE COUNTS, and a census missing it
    /// reads exactly like one taken in whatever state the reader assumed. Round
    /// 1099 assumed cold, and the assumption cost a cache that was saving ten
    /// minutes a run.
    JobDidNotSayWhatItRestored { job: String, cache: String },
    /// A record arrived and could not be read. Named by its FILE, because a
    /// record that did not decode has no readable job and no readable cache —
    /// the two halves every other refusal here names it by.
    RestoreRecordIsMalformed { file: String, why: String },
    /// A record measured paths that are not the ones ITS cache holds.
    ///
    /// THE LIST IS WRITTEN TWICE — once as the cache's `path:` and once as the
    /// argument to the step that measures it — and this is what makes that safe.
    /// Both sides are read off the machine and compared here, so a path added to
    /// a cache and not to the measurement is a refusal rather than a silently
    /// smaller difference.
    RestoreRecordMeasuredOtherPaths {
        job: String,
        cache: String,
        measured: Vec<String>,
        declared: Vec<String>,
    },
    /// A record says the primary key matched exactly and that nothing arrived on
    /// disk. The two instruments contradict each other, and which of them is
    /// wrong decides whether this census is of a warm run or a cold one.
    RestoredNothingAfterAnExactHit { job: String, cache: String },
    /// Two records claim to be the price of one job's one cache.
    ///
    /// AN OVERWRITE IS THE ALTERNATIVE AND IT IS SILENT. The join is a map, so a
    /// second record for one restore would simply replace the first and the
    /// census would print one price with no sign that another had been offered —
    /// and the two are routinely different numbers, since the way this arises is
    /// two measuring pairs writing one file or two runs unpacked into one
    /// directory.
    OneRestoreIsRecordedTwice {
        restore: String,
        /// BOTH FILES, because either one alone sends the reader to a file that
        /// is not obviously wrong. Which of the two a map happened to keep is
        /// the order `read_dir` returned them in, so naming only that one is a
        /// refusal whose subject is an accident.
        files: Vec<String>,
        measured: Vec<String>,
    },
    /// One job's two instruments were built from different commits.
    InstrumentsOfOneJobDisagreeAboutTheirBuild {
        job: String,
        measured: String,
        recorded: String,
    },
    /// Two jobs of one run were measured by instruments built from different
    /// commits, so their censuses are not in the same units.
    JobsOfOneRunWereMeasuredByDifferentBuilds { jobs: Vec<(String, String)> },
    /// A record says it is the price of a cache its job does not declare.
    RestoreRecordNamesACacheTheJobDoesNotDeclare {
        job: String,
        named: String,
        declared: Vec<String>,
    },
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
        /// The record the pair writes, which is what says WHICH pair this is —
        /// a job may have one per cache since R1121.
        record: String,
        side: restored::Side,
        times: usize,
    },
    /// A measuring pair does not sit either side of exactly one cache step.
    AMeasuringPairBracketsOtherThanOneCache {
        job: String,
        record: String,
        caches: usize,
    },
    /// A measuring pair brackets one cache and says it is pricing another.
    ///
    /// THE ONE THING NO RECORD CAN CONTRADICT. `restored before --cache <prefix>`
    /// is what the record's `cache` line is written from, and the check on that
    /// line asks only whether the prefix is one the JOB declares — which every
    /// one of a job's own prefixes is. So a `before` argument copied from the
    /// step above it produces a well-formed record naming a real cache of the
    /// right job, and the interval it carries belongs to the other one. That is
    /// the arithmetic R1099 got wrong, arrived at by a route the round that
    /// repaired it left open.
    AMeasuringPairNamesAnotherCache {
        job: String,
        record: String,
        named: Vec<String>,
        brackets: String,
    },
    /// A job measures a number of restores that is not the number of caches it
    /// declares.
    AJobMeasuresOtherThanOneRestorePerCache {
        job: String,
        pairs: usize,
        caches: usize,
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
    /// A program a job runs after a cache restore lives under a path that cache
    /// holds, so the restore replaces it with the previous generation's copy.
    ProgramRunAfterARestoreLivesUnderIt {
        job: String,
        program: String,
        held: String,
    },
    /// A step measures a cache restore in a job that declares no cache — the
    /// mirror of `RestoreRecordFromAJobWithNoCache`, read off the workflow, so
    /// it fires before any run leaves a record behind.
    RestoreIsMeasuredWithNoCache { job: String },
    /// A job measures a cache restore in a workflow that uploads no artifact at
    /// all, so the record it writes is destroyed with its runner.
    ///
    /// THE OTHER HALF OF THE DERIVATION that exempts such a workflow from owing
    /// a record. Both directions come out of one reading of the file, which is
    /// what stops the exemption from being a list somebody has to remember.
    RestoreIsMeasuredWhereNothingCollectsIt { job: String },
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
            Refusal::JobRanCompilationsThisReaderCannotPlace {
                job,
                compilations,
                what,
            } => write!(
                f,
                "job `{job}` ran {compilations} compilation(s) whose input file \
                 this reader could not find exactly one of — the counts are \
                 whole and the split by whose source they were is not, and the \
                 split is what says whether a restored cache was worth having: {}",
                what.iter()
                    .map(|one| format!("`{}` — {}", one.crate_name, one.why()))
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
            Refusal::JobWroteWhereNothingSaid { job, compilations } => write!(
                f,
                "job `{job}` ran {compilations} compilation(s) that named no \
                 `--out-dir`, so nothing can say whether its cache holds where \
                 that output went — and a destination this reader never saw is \
                 absent from both halves of the coverage, which still sum"
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
            Refusal::JobDidNotSayWhatItRestored { job, cache } => write!(
                f,
                "job `{job}` declares cache `{cache}` and left no record of what \
                 it brought — this census counts a COLD build by construction, \
                 so the cache state is a control variable of every number in it, \
                 and a census missing it reads as one taken in whatever state \
                 the reader assumed"
            ),
            Refusal::RestoreRecordIsMalformed { file, why } => write!(
                f,
                "`{file}` is a record of what a cache restored that does not \
                 read: {why}"
            ),
            Refusal::RestoreRecordMeasuredOtherPaths {
                job,
                cache,
                measured,
                declared,
            } => write!(
                f,
                "job `{job}` measured {} across the restore of `{cache}` and \
                 that cache holds {} — the two lists are written in two places \
                 and this is the only thing holding them together, so a path in \
                 one and not the other is a restore measured smaller than it was",
                measured.join(", "),
                declared.join(", ")
            ),
            Refusal::RestoredNothingAfterAnExactHit { job, cache } => write!(
                f,
                "job `{job}` was told by `actions/cache` that the primary key of \
                 `{cache}` matched exactly, and not one byte arrived under the \
                 paths that cache holds — one of the two instruments is wrong, \
                 and which one decides whether this census is of a warm run or a \
                 cold one"
            ),
            Refusal::OneRestoreIsRecordedTwice {
                restore,
                files,
                measured,
            } => write!(
                f,
                "{} both claim to be the price of {restore}, measuring {} — one \
                 restore has one price, so the join would keep whichever was \
                 read last and print it with no sign that another was offered",
                files
                    .iter()
                    .map(|file| format!("`{file}`"))
                    .collect::<Vec<_>>()
                    .join(" and "),
                measured
                    .iter()
                    .map(|paths| format!("[{paths}]"))
                    .collect::<Vec<_>>()
                    .join(" and ")
            ),
            Refusal::InstrumentsOfOneJobDisagreeAboutTheirBuild {
                job,
                measured,
                recorded,
            } => write!(
                f,
                "job `{job}` measured its restore with an instrument built from \
                 `{measured}` and its compilations with a recorder built from \
                 `{recorded}` — the two are built by one step from one commit, so \
                 a difference is one of them having arrived from somewhere else"
            ),
            Refusal::JobsOfOneRunWereMeasuredByDifferentBuilds { jobs } => write!(
                f,
                "the jobs of this run were measured by recorders built from \
                 different commits ({jobs:?}) — their censuses are not in the \
                 same units, and R1118 is what that costs: `unrun-tests` restored \
                 its own build directory over the binaries it had just made, and \
                 its seconds moved by a factor of four when the fresh recorder \
                 finally ran"
            ),
            Refusal::RestoreRecordNamesACacheTheJobDoesNotDeclare {
                job,
                named,
                declared,
            } => write!(
                f,
                "job `{job}` recorded a restore it says is the price of cache \
                 `{named}`, and the caches it declares are {declared:?} — a \
                 record whose cache is not one of its job's is an interval \
                 charged to nothing, and the next reader to ask what a cache \
                 costs would be told the wrong number rather than none"
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
            Refusal::RestoreSideIsNotMeasuredOnce {
                job,
                record,
                side,
                times,
            } => write!(
                f,
                "job `{job}` writes `{record}` and runs `restored {}` {times} \
                 time(s) for it — what a cache brought is the DIFFERENCE between \
                 two measurements, so each record takes one of each and no more",
                side.word()
            ),
            Refusal::AMeasuringPairBracketsOtherThanOneCache {
                job,
                record,
                caches,
            } => write!(
                f,
                "job `{job}`'s pair writing `{record}` sits either side of \
                 {caches} cache step(s) and owes exactly one — zero prices a \
                 restore that does not happen, and two charges one interval to \
                 both, which is how a 32 GB build directory's price is read off \
                 a 756 MB registry's restore"
            ),
            Refusal::AMeasuringPairNamesAnotherCache {
                job,
                record,
                named,
                brackets,
            } => write!(
                f,
                "job `{job}`'s pair writing `{record}` sits either side of \
                 `{brackets}` and says it prices {} — the record's `cache` line \
                 is written from that argument and is only ever checked against \
                 the caches the JOB declares, every one of which this passes, so \
                 nothing downstream can tell one of a job's restores from \
                 another once the two are swapped",
                if named.is_empty() {
                    "no cache at all".to_string()
                } else {
                    named
                        .iter()
                        .map(|one| format!("`{one}`"))
                        .collect::<Vec<_>>()
                        .join(" and ")
                }
            ),
            Refusal::AJobMeasuresOtherThanOneRestorePerCache { job, pairs, caches } => write!(
                f,
                "job `{job}` declares {caches} cache(s) and measures {pairs} \
                 restore(s) — a cache nobody measured is one whose price is \
                 unknown while the census reads as complete"
            ),
            Refusal::ProgramRunAfterARestoreLivesUnderIt { job, program, held } => write!(
                f,
                "job `{job}` runs `{program}` after a restore of a cache that \
                 holds `{held}` — the restore replaces that binary with the one \
                 the previous generation stored, so the program the job runs is \
                 not the program it built. R1117's record format changed and \
                 this killed `unrun-tests` in one step; every run before it, the \
                 same substitution was silent and the recorder measuring that \
                 job's whole census came out of the cache"
            ),
            Refusal::RestoreIsMeasuredWithNoCache { job } => write!(
                f,
                "job `{job}` measures a cache restore and declares no cache — \
                 there is nothing between its two measurements, so it would \
                 report an empty tree for a job that never had one to fill"
            ),
            Refusal::RestoreIsMeasuredWhereNothingCollectsIt { job } => write!(
                f,
                "job `{job}` measures a cache restore in a workflow that uploads \
                 no artifact at all — the record goes onto a runner that is \
                 destroyed when the job ends, so nothing can ever read it"
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
/// One `actions/cache` step of one job, as the workflow declares it.
///
/// THREE FIELDS THAT WERE THREE MAPS. Each answers a different question about
/// the same step — what it holds, where it sits, and which key it is — and they
/// are one value because a job's caches are a list of steps, not three lists
/// that happen to be the same length.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheOfAJob {
    /// Where in the job's `steps:` list it sits, counting every step, so it is
    /// directly comparable with [`ci_plan::RunStep::index`].
    pub index: usize,
    /// The resolved key prefix — the identity every gate here joins on, and the
    /// one a restore record names.
    pub prefix: String,
    /// The `path:` entries, in the order written.
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Declared {
    /// Every job with a `run:` step, and the steps.
    pub jobs: BTreeMap<String, Vec<ci_plan::RunStep>>,
    /// Every cache each job declares, in the order it declares them.
    ///
    /// R1120 — ONE HOME, AFTER THREE. This was three maps keyed by job: merged
    /// paths, indices, and prefixes. Three parallel lists of one thing can
    /// disagree about how many of it there are, and for a job declaring two
    /// caches they DID: two indices, two prefixes, and one merged path list with
    /// no way back to which cache held which path. That merge was deliberate
    /// while a record was a job's — the restore record bracketed a REGION and
    /// what arrived in it was what arrived under every path any of those caches
    /// held — and R1117 made a record a CACHE's, which is when the merge stopped
    /// being a simplification and became a loss.
    ///
    /// The readers that still want an old answer ask for it
    /// ([`Declared::cached_paths`], [`Declared::prefixes`]), so the region law
    /// and the per-cache laws are derived from one datum rather than kept beside
    /// each other. R1122 removed the third of those, `caches_at`: once the
    /// wiring law asked WHICH cache a pair brackets rather than merely how many,
    /// a list of positions with the prefixes taken off it had no caller left,
    /// and a helper kept alive by its own test is the carry this repository
    /// deletes rather than preserves.
    pub caches: BTreeMap<String, Vec<CacheOfAJob>>,
    /// Does this workflow collect anything a later job could download?
    ///
    /// A BOOLEAN BECAUSE THE QUESTION IS ONE: a workflow that uploads no
    /// artifact at all leaves nothing behind when its runners are destroyed, so
    /// a record written in it is unreadable however carefully it is written. It
    /// is read off the file rather than kept as a list of exempt workflows —
    /// see [`Declared::of`].
    pub collects_records: bool,
}

impl Declared {
    /// Read the populations out of one workflow's already-parsed parts.
    ///
    /// `uploads` IS THE THIRD ONE AND IT DECIDES WHO OWES A RECORD AT ALL. Every
    /// record these gates join is written to a file on a runner that is
    /// destroyed when the job ends; what survives is what an upload step
    /// collected. So a workflow that uploads nothing produces no record anything
    /// can download, and asking its jobs to measure a restore would be asking
    /// them to write a file nobody can read.
    ///
    /// DERIVED AND NOT LISTED. `evidence-replay.yml` is exactly that workflow
    /// today — it declares a cache, uploads nothing, and until this was read the
    /// wiring law simply never looked at it. A list of exempt workflows beside
    /// the law is the shape four rounds have each closed one level at a time.
    pub fn of(
        steps: &[ci_plan::RunStep],
        caches: &[ci_plan::CacheDeclaration],
        uploads: &[ci_plan::ArtifactUpload],
    ) -> Declared {
        let mut jobs: BTreeMap<String, Vec<ci_plan::RunStep>> = BTreeMap::new();
        for step in steps {
            jobs.entry(step.job.clone()).or_default().push(step.clone());
        }
        let mut cached: BTreeMap<String, Vec<CacheOfAJob>> = BTreeMap::new();
        for cache in caches {
            cached
                .entry(cache.owner.clone())
                .or_default()
                .push(CacheOfAJob {
                    index: cache.index,
                    prefix: cache.prefix.clone(),
                    paths: cache.paths.clone(),
                });
        }
        Declared {
            jobs,
            caches: cached,
            collects_records: ci_plan::collects_artifacts(uploads),
        }
    }

    /// Every path any of a job's caches holds, first mention first.
    ///
    /// THE REGION, and it is asked for rather than stored: while a restore record
    /// is a job's, what arrived in the region between its two measurements is
    /// what arrived under every one of these. R1117 made a record a cache's, so
    /// this is the answer the REGION laws want and not the one the per-cache laws
    /// want — which is exactly why it is a function of the list rather than a
    /// second copy beside it.
    pub fn cached_paths(&self, job: &str) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for cache in self.caches.get(job).into_iter().flatten() {
            for path in &cache.paths {
                if !out.contains(path) {
                    out.push(path.clone());
                }
            }
        }
        out
    }

    /// The resolved key prefix of each cache a job declares.
    pub fn prefixes(&self, job: &str) -> Vec<String> {
        self.caches
            .get(job)
            .into_iter()
            .flatten()
            .map(|cache| cache.prefix.clone())
            .collect()
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
            // NOT AN ARM OF THE MATCH ABOVE, because it is not the same silence
            // wearing a third name: those two are a job that recorded nothing
            // this reader could use at all, and this is a job that recorded
            // plenty and lost part of it. A job can be refused for this while
            // every count it reports is real.
            Some(log) if !log.unplaced.is_empty() => {
                refusals.push(Refusal::JobRanCompilationsThisReaderCannotPlace {
                    job: job.clone(),
                    compilations: log.unplaced_compilations(),
                    what: log.unplaced.keys().cloned().collect(),
                })
            }
            Some(log) if log.wrote_nowhere_named() > 0 => {
                refusals.push(Refusal::JobWroteWhereNothingSaid {
                    job: job.clone(),
                    compilations: log.wrote_nowhere_named(),
                })
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
/// Every program one `run:` script executes from a path inside the checkout.
///
/// THE WORDS THAT LOOK LIKE A PATH TO A FILE THIS TREE HOLDS — a leading `./`,
/// or a bare relative path with a directory in it. `cargo` and `git` are names
/// resolved on `PATH` and are not this repository's files, so they are not
/// candidates for being overwritten by a restore; `./instruments/release/restored`
/// is. Read from the script rather than listed, so a step added tomorrow is
/// under the law the day it is written.
pub fn programs_run(script: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in script.lines() {
        // THE FIRST WORD AND ONLY THE FIRST. The rest are arguments, and an
        // argument that looks like a path (`--manifest-path tools/x/Cargo.toml`)
        // is read by the program rather than being one.
        let Some(word) = line.split_whitespace().next() else {
            continue;
        };
        let candidate = word.trim_matches(['"', '\'']);
        let relative = candidate.strip_prefix("./").unwrap_or(candidate);
        // An absolute path is somebody else's tree, and a word with no `/` is a
        // name resolved on `PATH`. Neither is a file this checkout holds at a
        // path some cache of this workflow could restore over.
        if candidate.starts_with('/') || !relative.contains('/') {
            continue;
        }
        out.push(relative.to_string());
    }
    out
}

pub fn judge_wiring(declared: &Declared) -> Vec<Refusal> {
    let mut refusals = Vec::new();
    // A WORKFLOW THAT COLLECTS NOTHING OWES NOTHING, and the inverse is a
    // refusal: a job measuring a restore where no artifact is uploaded writes a
    // file that is destroyed with its runner. Both directions are derived from
    // the same reading, so neither is a list.
    if !declared.collects_records {
        for (job, steps) in &declared.jobs {
            if steps
                .iter()
                .any(|step| !restored::sides_measured(&step.script).is_empty())
            {
                refusals
                    .push(Refusal::RestoreIsMeasuredWhereNothingCollectsIt { job: job.clone() });
            }
        }
        return refusals;
    }
    // WHAT A RESTORE OVERWRITES, which is a different axis from where the
    // measurements sit. R1118: `unrun-tests` builds its instruments into
    // `target`, caches `target`, and restores it AFTER the build — so the 8.90
    // GB that arrives replaces the binaries the job just made, and the program
    // it runs is the one the previous generation stored. The wiring law above
    // reads the ORDER of steps in the file and cannot see this: both steps are
    // in the right places and the file is still wrong.
    //
    // ASKED OF EVERY JOB AND NOT ONLY THE ONE THAT BROKE. A cache added tomorrow
    // to a job whose programs live under `target/` is the same defect, and a
    // check scoped to today's caches would not be there for it.
    for (job, steps) in &declared.jobs {
        // EVERY CACHED PATH AND NOT ONLY THE ONES IN THE CHECKOUT. A `~`-rooted
        // path cannot be a prefix of a checkout-relative program, so filtering
        // it out changed no answer — the injection sweep proved the filter dead
        // by removing it and watching nothing go red. It was also WRONG in the
        // one case it would have decided: a program under a restored
        // `~/.cargo/...` is overwritten exactly as one under `target/` is.
        let held = declared.cached_paths(job);
        if held.is_empty() {
            continue;
        }
        for step in steps {
            for program in programs_run(&step.script) {
                for path in &held {
                    // A PREFIX ON PATH COMPONENTS. `targeted/release/x` is not
                    // under `target`, and a plain `starts_with` would say it is
                    // — a refusal naming a file no cache touches is how a gate
                    // teaches people to ignore it.
                    if program == *path || program.starts_with(&format!("{path}/")) {
                        refusals.push(Refusal::ProgramRunAfterARestoreLivesUnderIt {
                            job: job.clone(),
                            program: program.clone(),
                            held: path.clone(),
                        });
                    }
                }
            }
        }
    }
    for job in declared.caches.keys() {
        let held = declared.caches.get(job).map(Vec::as_slice).unwrap_or(&[]);
        // WHERE THE RECORD WILL BE WRITTEN — the same law the compilation log
        // has, on the other record.
        //
        // ASKED OF THE STEPS THAT MEASURE AND NOT OF EVERY STEP, which R1121's
        // shape made wrong rather than merely wide. The variable used to sit in
        // the job's `env:`, so every step of a cached job carried it and asking
        // all of them cost nothing; a job with TWO caches writes TWO records, so
        // the value cannot be the job's any more and the steps that do not
        // measure have no record to name. A law still demanding it of them would
        // refuse exactly the workflow this round exists to make possible.
        for step in declared.jobs.get(job).into_iter().flatten() {
            if restored::sides_measured(&step.script).is_empty() {
                continue;
            }
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
        //
        // R1121 — GROUPED BY THE RECORD THEY WRITE, which is what turns this
        // from a law about a REGION into a law about each cache. Until R1117 a
        // record was a job's, so the pair had to sit outside every cache the job
        // declared and what arrived between them was the sum; now a record is
        // one cache's, and the two measurements that write one file are the two
        // that price one restore. The record path is what says which pair a
        // measurement belongs to — nothing else in the file does — so it is the
        // key here rather than a position anybody has to count.
        let mut pairs: BTreeMap<String, Vec<(usize, restored::Side)>> = BTreeMap::new();
        // AND WHICH CACHE THE PAIR SAYS IT PRICES, taken off the same steps. The
        // `--cache` argument is what the record's `cache` line is written from,
        // so a pair that names the wrong one produces a record naming a cache
        // its job really does declare — passing every check that reads the
        // record, and charging one restore's interval to the other cache.
        let mut named: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for step in declared.jobs.get(job).into_iter().flatten() {
            let record = step
                .env
                .get(restored::VARIABLE)
                .cloned()
                .unwrap_or_default();
            for side in restored::sides_measured(&step.script) {
                pairs
                    .entry(record.clone())
                    .or_default()
                    .push((step.index, side));
            }
            for cache in restored::caches_named(&step.script) {
                named.entry(record.clone()).or_default().push(cache);
            }
        }
        for (record, measured) in &pairs {
            let mut sides = Vec::new();
            for side in restored::Side::BOTH {
                let ours: Vec<usize> = measured
                    .iter()
                    .filter(|(_, measured)| *measured == side)
                    .map(|(index, _)| *index)
                    .collect();
                match ours.as_slice() {
                    [only] => sides.push(*only),
                    _ => refusals.push(Refusal::RestoreSideIsNotMeasuredOnce {
                        job: job.clone(),
                        record: record.clone(),
                        side,
                        times: ours.len(),
                    }),
                }
            }
            let [before_at, after_at] = sides.as_slice() else {
                continue;
            };
            // EXACTLY ONE CACHE BETWEEN THEM. Zero is a pair pricing a restore
            // that does not happen; two is one interval charged to both, which
            // is the arithmetic R1099 got wrong for a job and this is what stops
            // it returning per cache. A pair in the wrong ORDER brackets nothing
            // and is caught by the same count, which is why the two questions do
            // not need two refusals.
            let between: Vec<&CacheOfAJob> = held
                .iter()
                .filter(|cache| *before_at < cache.index && cache.index < *after_at)
                .collect();
            let [bracketed] = between.as_slice() else {
                refusals.push(Refusal::AMeasuringPairBracketsOtherThanOneCache {
                    job: job.clone(),
                    record: record.clone(),
                    caches: between.len(),
                });
                continue;
            };
            // AND IT NAMES THE ONE IT BRACKETS. Checked only once the pair is
            // known to bracket exactly one cache, because "which cache is this
            // the price of" has no answer at all until then — two names for one
            // wiring mistake would send two readers to two different repairs.
            match named.get(record).map(Vec::as_slice) {
                Some([said]) if *said == bracketed.prefix => {}
                other => refusals.push(Refusal::AMeasuringPairNamesAnotherCache {
                    job: job.clone(),
                    record: record.clone(),
                    named: other.unwrap_or_default().to_vec(),
                    brackets: bracketed.prefix.clone(),
                }),
            }
        }
        // AND ONE PAIR PER CACHE. A job that declares two and measures one has a
        // restore nobody priced, and the record it does write reads as the whole
        // job's.
        if pairs.len() != held.len() {
            refusals.push(Refusal::AJobMeasuresOtherThanOneRestorePerCache {
                job: job.clone(),
                pairs: pairs.len(),
                caches: held.len(),
            });
        }
    }
    // THE OTHER DIRECTION, which no record can report until one arrives: a job
    // measuring a restore that never happens.
    for (job, steps) in &declared.jobs {
        if declared.caches.contains_key(job) {
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
    // WHAT DECODED, FILED UNDER WHAT IT SAYS IT IS. R1121 left this join on the
    // JOB, which was the whole identity of a record while a job had one cache;
    // the point of splitting a cache step is that it no longer is. A second
    // record for one restore is a refusal rather than an overwrite, because the
    // two would price one interval twice and whichever the walk reached last
    // would silently become the answer.
    let mut said: BTreeMap<restored::Restore, (&str, &restored::Restored)> = BTreeMap::new();
    for (file, record) in &census.restored {
        let Ok(record) = record else { continue };
        if absent.contains(&record.job) {
            continue;
        }
        // A RECORD IN THE WRONG FILE IS NOT A SECOND PRICE FOR THE RESTORE IT
        // NAMES. Two jobs writing one path leaves one record carrying another
        // job's name, and it would land here as a duplicate of that job's real
        // one — a second name for the paste error, sending a reader to a repair
        // that is not the one. The name is checked below and this waits for it.
        if !restored::names_its_job(file, &record.job) {
            continue;
        }
        if let Some((was, earlier)) = said.insert(record.restore(), (file, record)) {
            refusals.push(Refusal::OneRestoreIsRecordedTwice {
                restore: record.restore().to_string(),
                files: vec![was.to_string(), file.clone()],
                measured: vec![earlier.measured().join(", "), record.measured().join(", ")],
            });
        }
    }
    for job in declared.caches.keys() {
        if absent.contains(job) || !declared.jobs.contains_key(job) {
            continue;
        }
        // ONE VERDICT PER CACHE, AND THE PATHS ARE THAT CACHE'S. Until this
        // round the comparison was against the REGION — every path any of the
        // job's caches held — which is the same list only while the job has one
        // cache. For a job with two it is wrong in both directions at once: each
        // record measures its own cache's paths and is refused for it, and a
        // record that measured the whole region would be accepted while pricing
        // a restore that never touched half of it.
        for cache in declared.caches.get(job).into_iter().flatten() {
            let wanted = restored::Restore {
                job: job.clone(),
                cache: cache.prefix.clone(),
            };
            let Some((_, record)) = said.get(&wanted) else {
                refusals.push(Refusal::JobDidNotSayWhatItRestored {
                    job: job.clone(),
                    cache: cache.prefix.clone(),
                });
                continue;
            };
            if record.measured() != cache.paths.iter().map(String::as_str).collect::<Vec<_>>() {
                refusals.push(Refusal::RestoreRecordMeasuredOtherPaths {
                    job: job.clone(),
                    cache: cache.prefix.clone(),
                    measured: record
                        .measured()
                        .iter()
                        .map(|one| one.to_string())
                        .collect(),
                    declared: cache.paths.clone(),
                });
            }
            // THE CONTRADICTION IS A REFUSAL AND NOT A THIRD READING. The
            // other three states of `restored::Warmth` are states of the
            // world; this one is the two instruments disagreeing, and a
            // census cannot be quoted while it is unresolved.
            if record.warmth() == restored::Warmth::HitThatBroughtNothing {
                refusals.push(Refusal::RestoredNothingAfterAnExactHit {
                    job: job.clone(),
                    cache: cache.prefix.clone(),
                });
            }
        }
    }
    // WHICH BUILD OF THE INSTRUMENTS MEASURED THIS RUN. R1119. Two questions,
    // both answered from the census alone so neither needs an environment:
    // whether one job's two instruments agree with each other, and whether the
    // jobs agree with one another. The second is what would have caught R1118 —
    // six jobs measured by the commit's recorder and one by whatever its cache
    // held.
    //
    // WHAT IT CANNOT SEE, stated: a run in which EVERY instrument came from a
    // cache of the same older commit is internally consistent, and the guard for
    // that is `ProgramRunAfterARestoreLivesUnderIt` above — the programs live
    // where no cache reaches, so the case cannot arise while that law holds.
    let mut recorders: Vec<(String, String)> = Vec::new();
    for record in census.restored.values().flatten() {
        if absent.contains(&record.job) {
            continue;
        }
        let (measured, recorded) = &record.built_from;
        // `none` is the recorder deliberately absent — the instruments' own
        // build sets `RUSTC_WRAPPER: ""`, and a step running under that has
        // no recorder to disagree with.
        if recorded != "none" && measured != recorded {
            refusals.push(Refusal::InstrumentsOfOneJobDisagreeAboutTheirBuild {
                job: record.job.clone(),
                measured: measured.clone(),
                recorded: recorded.clone(),
            });
        }
        if recorded != "none" {
            recorders.push((record.job.clone(), recorded.clone()));
        }
    }
    if recorders
        .iter()
        .any(|(_, commit)| Some(commit) != recorders.first().map(|(_, first)| first))
    {
        let mut jobs = recorders.clone();
        jobs.sort();
        refusals.push(Refusal::JobsOfOneRunWereMeasuredByDifferentBuilds { jobs });
    }
    for (file, record) in &census.restored {
        let record = match record {
            Ok(record) => record,
            // A RECORD THAT DID NOT DECODE HAS ONLY ITS FILE NAME, which is why
            // it is refused here rather than where a cache goes looking for one:
            // nothing in it can be joined to a declaration, and reading its
            // absence from that join as "the job said nothing" would report the
            // loudest defect available here under the quietest name.
            Err(why) => {
                refusals.push(Refusal::RestoreRecordIsMalformed {
                    file: file.clone(),
                    why: why.to_string(),
                });
                continue;
            }
        };
        if absent.contains(&record.job) {
            continue;
        }
        // THE RECORD CARRIES THE JOB THE RUNNER NAMED, and the file carries the
        // job the workflow spelled by hand. Two jobs writing one path leaves
        // exactly this disagreement behind, and it is the one direction the
        // checks below cannot see: the surviving record is well-formed and
        // measures real paths.
        //
        // ASKED FIRST, AND THE OTHERS ONLY IF IT HOLDS. A pasted record names
        // the wrong job AND, through it, a cache that job does not declare —
        // one defect, and two names for it send two readers to two repairs.
        if !restored::names_its_job(file, &record.job) {
            refusals.push(Refusal::RestoreRecordNamesAnotherJob {
                file: file.clone(),
                said: record.job.clone(),
            });
            continue;
        }
        let Some(prefixes) = declared
            .caches
            .contains_key(&record.job)
            .then(|| declared.prefixes(&record.job))
        else {
            refusals.push(Refusal::RestoreRecordFromAJobWithNoCache {
                job: record.job.clone(),
            });
            continue;
        };
        // WHICH CACHE THIS IS THE PRICE OF, checked against what the workflow
        // declares. R1117 gave the record the field; without this it would be
        // one more identity spelled twice and compared never, which is exactly
        // the defect R1116 found in `restore-keys` — a property nothing could
        // contradict.
        if !prefixes.contains(&record.cache) {
            refusals.push(Refusal::RestoreRecordNamesACacheTheJobDoesNotDeclare {
                job: record.job.clone(),
                named: record.cache.clone(),
                declared: prefixes,
            });
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
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        // TWO KINDS OF RECORD, IN ONE DIRECTORY, READ BY ONE LOADER: what a job
        // compiled and what each of its cache restores brought. They are
        // uploaded together because they are one measurement — the counts are of
        // a cold build by construction, so the state is the units they are in.
        //
        // AND THEY ARE NOT COUNTED THE SAME WAY. A log is a job's, so its stem
        // IS the job; a restore record is one cache's since R1117, so a job may
        // leave several and the stem is `<job>` or `<job>.<nickname>`. Which
        // makes the SKIP different too: a caller naming an absent job means
        // every record that job wrote, and `absent.contains(stem)` would let a
        // nicknamed one through — a record from the job the gate is itself
        // running in, read as a finding about a build still happening.
        match extension {
            Some("log") => {
                if absent.contains(stem) {
                    continue;
                }
                let text = std::fs::read_to_string(&path)?;
                census.jobs.insert(stem.to_string(), read_log(&text));
            }
            Some("restored") => {
                if absent.iter().any(|job| restored::names_its_job(name, job)) {
                    continue;
                }
                let text = std::fs::read_to_string(&path)?;
                census
                    .restored
                    .insert(name.to_string(), restored::decode(&text));
            }
            _ => continue,
        }
    }
    Ok(census)
}

// --- holding one census against another -------------------------------------

/// One of the two censuses in a comparison.
///
/// NOT `restored::Side`, which names the two sides of a cache restore inside one
/// job. These are two whole measurements, and the words are `Earlier` and
/// `Later` so that no use of them can be read as the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Side {
    Earlier,
    Later,
}

impl std::fmt::Display for Side {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Side::Earlier => write!(f, "the earlier census"),
            Side::Later => write!(f, "the later census"),
        }
    }
}

/// Why one job's two sides are not a difference.
///
/// THE WHOLE SUBJECT OF THIS TYPE IS ROUND 1099. Two censuses were read side by
/// side, one of them was called cold because its keys had moved, the counts came
/// out equal, and a 7.5 GB cache that was saving 426 compilations and ten
/// minutes was deleted. Both runs had in fact been warmed through
/// `restore-keys`. Every count in a census is of a COLD build by construction —
/// cargo runs no compiler for a unit that is already fresh — so what a job began
/// with is the unit its numbers are in, and a subtraction across two different
/// units is not a measurement.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Incomparable {
    /// One of the job's restores brought a different state in the two censuses,
    /// so any change in what it compiled is confounded by what it began with.
    ///
    /// NAMED PER RESTORE AND NOT PER JOB. A job with two caches has two starting
    /// states, and they move independently — the run that split this
    /// repository's build directory off its cargo home had one of them exact and
    /// the other empty. A verdict that collapsed them would say "the job started
    /// warm" about a job half of whose tree was not there.
    StartedInDifferentStates {
        restore: restored::Restore,
        earlier: restored::Warmth,
        later: restored::Warmth,
    },
    /// One side said what a restore brought and the other did not. The same
    /// silence on both sides is not a difference; a silence on one is.
    OnlyOneSideSaidWhatItStartedFrom {
        restore: restored::Restore,
        silent: Side,
    },
    /// The job left a compilation record in one census and not the other — a job
    /// added, removed or renamed between the two runs. Its numbers have nothing
    /// to be subtracted from.
    OnlyInOneCensus { job: String, missing: Side },
}

impl std::fmt::Display for Incomparable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Incomparable::StartedInDifferentStates {
                restore,
                earlier,
                later,
            } => write!(
                f,
                "{restore} began in different states — earlier: {}; later: {} \
                 — and every count in a census is of a cold build, so the state \
                 is the unit those counts are in and the difference is not one",
                earlier.why(),
                later.why()
            ),
            Incomparable::OnlyOneSideSaidWhatItStartedFrom { restore, silent } => write!(
                f,
                "{restore} said what it brought in one census and not in \
                 {silent} — the same silence on both sides is no difference, and \
                 a silence on one side is a state nobody can supply"
            ),
            Incomparable::OnlyInOneCensus { job, missing } => write!(
                f,
                "job `{job}` left no record in {missing}, so its numbers have \
                 nothing to be subtracted from"
            ),
        }
    }
}

/// What one census says, whole.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Totals {
    /// Compilations summed over jobs — what CI paid for.
    pub paid: usize,
    /// What one machine sharing a `target` would have paid.
    pub floor: usize,
    /// What the compilations cost, summed over jobs.
    pub micros: u64,
}

impl Totals {
    /// What one census says about itself.
    pub fn of(census: &Census) -> Totals {
        Totals {
            paid: census.paid(),
            floor: census.floor(),
            micros: census.paid_micros(),
        }
    }
}

/// What one job's two censuses say, when they are a difference at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delta {
    /// What each of the job's restores brought — the SAME state on both sides
    /// for every one of them, which is the only case there is a difference to
    /// read, and BOTH values because the sizes differ and the predicate that let
    /// this pair through does not look at them.
    ///
    /// ONE ROW PER CACHE, keyed by its prefix. Empty when neither census said
    /// anything: a job with no cache always begins with an empty tree, so the
    /// same silence twice is not a difference.
    pub started: BTreeMap<String, (restored::Warmth, restored::Warmth)>,
    /// Compilations, earlier then later.
    pub compilations: (usize, usize),
    /// What they cost, earlier then later.
    pub micros: (u64, u64),
    /// The same pair, split by whose source each compilation read.
    ///
    /// WITHOUT THIS A DELTA NAMES NO CAUSE. Four hundred compilations appearing
    /// between two runs is one finding if they are this repository's own crates
    /// — somebody added code, or a fingerprint moved — and an entirely different
    /// one if they are crates cargo had already fetched, because the second says
    /// a cache brought sources whose compiled form it did not bring. The counts
    /// above cannot tell those apart, and the pair that can is the one below.
    ///
    /// EVERY ORIGIN EITHER SIDE HAD, with the missing side's cost zero: an
    /// origin present in one census and absent from the other is exactly the
    /// difference a reader is here for, and dropping the row would hide the
    /// largest kind of change this can find.
    pub origins: BTreeMap<Origin, (Cost, Cost)>,
    /// What each side ran that this reader could not place, earlier then later
    /// — see [`Invocation::Unplaced`].
    ///
    /// BESIDE THE SPLIT AND NOT INSTEAD OF IT. These are missing from every
    /// number above, including the compilations, so a row of them is the reader
    /// saying how much of the job it is not describing. `compare` has no
    /// workflow to refuse against, so what it owes is this line.
    pub unplaced: BTreeMap<Unplaceable, (Cost, Cost)>,
}

impl Delta {
    /// Compilations the later census pays that the earlier did not, negative
    /// when it pays fewer.
    pub fn compilations_changed(&self) -> i64 {
        self.compilations.1 as i64 - self.compilations.0 as i64
    }

    /// The same for what they cost.
    pub fn micros_changed(&self) -> i64 {
        self.micros.1 as i64 - self.micros.0 as i64
    }
}

/// Hold two breakdowns of the same kind side by side.
///
/// A KEY ONLY ONE SIDE HAD IS A ROW WHOSE OTHER SIDE IS ZERO, and never a row
/// that is dropped: an origin present in one census and absent from the other,
/// or a crate one run could not place and the other could, is exactly the
/// difference a reader came for.
///
/// ONE FUNCTION FOR BOTH BREAKDOWNS, because "how are two of these held
/// together" must not have a second answer that fills the gaps differently.
fn side_by_side<K: Ord + Clone>(
    earlier: &BTreeMap<K, Cost>,
    later: &BTreeMap<K, Cost>,
) -> BTreeMap<K, (Cost, Cost)> {
    earlier
        .keys()
        .chain(later.keys())
        .map(|key| {
            let pair = (
                earlier.get(key).copied().unwrap_or_default(),
                later.get(key).copied().unwrap_or_default(),
            );
            (key.clone(), pair)
        })
        .collect()
}

/// Two censuses held against each other.
///
/// THE TOTALS ARE BEHIND A FUNCTION AND THE FIELDS ARE PRIVATE, which is the
/// whole design. A caller cannot reach a sum over a set in which one job changed
/// state, because that sum is the one Round 1099 quoted — and a law that a
/// caller has to remember to consult is one it will read past.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comparison {
    /// The jobs whose two sides are a difference, by job id.
    pub jobs: BTreeMap<String, Delta>,
    /// Why the others are not, sorted.
    pub incomparable: Vec<Incomparable>,
    earlier: Totals,
    later: Totals,
}

impl Comparison {
    /// The two censuses' totals — ONLY when every job in either of them is
    /// comparable.
    ///
    /// A TOTAL IS A SUM OVER A POPULATION, so one job in a different state
    /// spoils the whole of it and not merely its own row. That is not
    /// conservatism: the counts of a job that was cold in one run and warm in
    /// the other differ by hundreds, which is larger than any real change the
    /// comparison is being made to find.
    /// AND NOT OVER NOTHING. Two empty directories compare equal, and equal is
    /// exactly what two runs that agreed look like — the shape every gate in
    /// this repository has already been caught by once. A comparison that
    /// reached no comparable job has no difference available to it and must not
    /// sign off as one that found none.
    pub fn totals(&self) -> Option<(Totals, Totals)> {
        (self.incomparable.is_empty() && !self.jobs.is_empty())
            .then_some((self.earlier, self.later))
    }
}

/// Hold one census against another.
///
/// NO WORKFLOW IS READ. The two censuses may be of two different commits, whose
/// workflows declare different jobs — which is a thing this can say (a job in
/// one census and not the other) rather than a thing it needs told. What it has
/// is what both runs uploaded, and that is enough because R1101 made every
/// census say what it was taken under.
pub fn compare(earlier: &Census, later: &Census) -> Comparison {
    let earlier_started = earlier.started();
    let later_started = later.started();
    let mut jobs = BTreeMap::new();
    let mut incomparable = Vec::new();
    let names: BTreeSet<&str> = earlier
        .jobs
        .keys()
        .chain(later.jobs.keys())
        .map(String::as_str)
        .collect();
    for job in names {
        let (Some(before), Some(after)) = (earlier.jobs.get(job), later.jobs.get(job)) else {
            incomparable.push(Incomparable::OnlyInOneCensus {
                job: job.to_string(),
                missing: if earlier.jobs.contains_key(job) {
                    Side::Later
                } else {
                    Side::Earlier
                },
            });
            continue;
        };
        // EVERY RESTORE EITHER CENSUS RECORDED FOR THIS JOB, and not the ones
        // one of them happened to have: a cache split in two between the runs
        // shows up as a restore only the later census knows about, which is a
        // silence on one side and exactly the thing this refuses to subtract
        // across.
        let restores: BTreeSet<&restored::Restore> = earlier_started
            .keys()
            .chain(later_started.keys())
            .filter(|restore| restore.job == job)
            .collect();
        let mut started = BTreeMap::new();
        let mut confounded = false;
        for restore in restores {
            match (earlier_started.get(restore), later_started.get(restore)) {
                (Some(before), Some(after)) if before.same_state(after) => {
                    started.insert(restore.cache.clone(), (*before, *after));
                }
                (Some(before), Some(after)) => {
                    incomparable.push(Incomparable::StartedInDifferentStates {
                        restore: restore.clone(),
                        earlier: *before,
                        later: *after,
                    });
                    confounded = true;
                }
                (present, _) => {
                    incomparable.push(Incomparable::OnlyOneSideSaidWhatItStartedFrom {
                        restore: restore.clone(),
                        silent: if present.is_some() {
                            Side::Later
                        } else {
                            Side::Earlier
                        },
                    });
                    confounded = true;
                }
            }
        }
        // ONE CONFOUNDED RESTORE CONFOUNDS THE JOB. Its compilations are one
        // number over every tree it began with, so a cache whose state moved
        // makes the whole of that number unsubtractable — there is no per-cache
        // count to keep.
        if confounded {
            continue;
        }
        jobs.insert(
            job.to_string(),
            Delta {
                started,
                compilations: (before.compilations(), after.compilations()),
                micros: (before.compiled_micros(), after.compiled_micros()),
                origins: side_by_side(&before.by_origin(), &after.by_origin()),
                unplaced: side_by_side(&before.unplaced, &after.unplaced),
            },
        );
    }
    incomparable.sort();
    incomparable.dedup();
    Comparison {
        jobs,
        incomparable,
        earlier: Totals::of(earlier),
        later: Totals::of(later),
    }
}

/// Load a census from a directory of records, or from the per-artifact
/// subdirectories `gh run download` leaves behind.
///
/// BOTH SHAPES, BECAUSE BOTH ARE REAL. On a runner the gate is handed one
/// directory with every job's records merged into it; a person comparing two
/// runs types `gh run download <id>`, which writes one subdirectory per
/// artifact. A loader that knew only the first reads the second as a run in
/// which no job recorded anything — an empty census, which prints every total as
/// a clean zero.
///
/// A JOB FOUND TWICE IS AN ERROR AND NOT AN OVERWRITE: two records for one job
/// means two runs' artifacts were unpacked into one directory, and whichever the
/// walk reached last would silently become the answer.
pub fn load_collected(directory: &Path) -> std::io::Result<Census> {
    let mut census = load(directory, &BTreeSet::new())?;
    let mut entries: Vec<PathBuf> = std::fs::read_dir(directory)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<_>>()?;
    entries.sort();
    for path in entries {
        if !path.is_dir() {
            continue;
        }
        let inner = load(&path, &BTreeSet::new())?;
        for (job, log) in inner.jobs {
            if census.jobs.insert(job.clone(), log).is_some() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "job `{job}` left a compilation record in two places under \
                         {} — that is two runs unpacked into one directory, and \
                         whichever this walk reached last would become the answer",
                        directory.display()
                    ),
                ));
            }
        }
        for (file, record) in inner.restored {
            if census.restored.insert(file.clone(), record).is_some() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "`{file}` is a restore record in two places under {}",
                        directory.display()
                    ),
                ));
            }
        }
    }
    Ok(census)
}

/// What this gate was asked to do.
///
/// A TYPE RATHER THAN A LOOK AT `arguments.first()`, because the three entrances
/// need different things and one of them needs NOTHING this repository holds:
/// a comparison of two censuses reads no workflow, no repository and no run, so
/// the checks the other two must make would refuse it for lacking what it does
/// not use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entrance {
    /// Judge the records a run already wrote.
    Logs {
        directory: String,
        workflow: Option<String>,
    },
    /// Produce those records on this machine.
    Replay {
        scratch: String,
        workflow: Option<String>,
    },
    /// Hold one census against another.
    Compare { earlier: String, later: String },
}

/// Read the words this gate was given.
///
/// IN THE LIBRARY, WITH A READER, for the reason `cache-budget`'s is: R1096
/// measured what a decision in `main.rs` costs, and the flag bug R1102 caught
/// before it shipped was a value read as a positional argument. So a flag
/// consumes its own value here, an unknown flag is a refusal rather than a
/// directory, and every one of those is a question a test can ask.
///
/// AN ERROR RATHER THAN A PANIC, because every one of these is somebody typing
/// a command and the answer belongs on stderr.
pub fn read_arguments(arguments: &[String]) -> Result<Entrance, String> {
    let mut words = arguments.iter();
    let mut positional: Vec<&str> = Vec::new();
    let mut workflow: Option<String> = None;
    let mut scratch: Option<String> = None;
    let mut comparing = false;
    while let Some(word) = words.next() {
        match word.as_str() {
            "--workflow" => workflow = Some(words.next().ok_or("--workflow needs a path")?.clone()),
            "--replay" => {
                scratch = Some(
                    words
                        .next()
                        .ok_or("--replay needs a scratch directory")?
                        .clone(),
                )
            }
            "compare" => comparing = true,
            other if other.starts_with("--") => {
                return Err(format!(
                    "`{other}` is not a flag this gate defines — refusing rather \
                     than reading it as a directory"
                ))
            }
            other => positional.push(other),
        }
    }
    if comparing {
        // THE ONE ENTRANCE THAT READS NO WORKFLOW, and being handed one is a
        // misunderstanding worth saying out loud rather than ignoring: the two
        // censuses may be of two different commits, whose workflows declare
        // different jobs, and that is something this ANSWERS rather than needs.
        if workflow.is_some() || scratch.is_some() {
            return Err(
                "`compare` reads no workflow and runs nothing — the two censuses \
                 may be of two different commits, and a job in one and not the \
                 other is part of its answer"
                    .to_string(),
            );
        }
        let [earlier, later] = positional.as_slice() else {
            return Err(format!(
                "usage: twice-compiled compare <earlier records> <later records> \
                 — {} directory(ies) given",
                positional.len()
            ));
        };
        return Ok(Entrance::Compare {
            earlier: (*earlier).to_string(),
            later: (*later).to_string(),
        });
    }
    if let Some(scratch) = scratch {
        return Ok(Entrance::Replay { scratch, workflow });
    }
    match positional.as_slice() {
        [directory] => Ok(Entrance::Logs {
            directory: (*directory).to_string(),
            workflow,
        }),
        [] => Err(
            "usage: twice-compiled <log directory> | --replay <scratch> | \
                   compare <earlier records> <later records>"
                .to_string(),
        ),
        many => Err(format!(
            "one log directory, and {} were given: {many:?}",
            many.len()
        )),
    }
}

/// A comparison as a person reads it.
///
/// IN THE LIBRARY FOR THE REASON THE READING RULES ARE: what a gate SAYS is a
/// decision, and a decision in `main.rs` can only be asked its exit code —
/// R1096, and R1104 for the sibling gate.
///
/// BOTH START STATES ARE PRINTED EVEN THOUGH THEY MATCHED, because the predicate
/// that let the pair through looks at the variant and not the size: two prefix
/// hits of 246 MB and 27 GB are one state to it. A reader is owed the numbers
/// the check does not look at.
pub fn render_comparison(held: &Comparison, earlier: &str, later: &str) -> String {
    let mut out = format!("twice-compiled — {earlier} held against {later}\n\n");
    if held.jobs.is_empty() && held.incomparable.is_empty() {
        out.push_str(
            "  NEITHER CENSUS HOLDS A JOB — two empty directories compare equal, \
             and that reads exactly like two runs that agreed\n",
        );
        return out;
    }
    for (job, delta) in &held.jobs {
        out.push_str(&format!(
            "  {job:<22} {:>6} -> {:<6} compilations ({:+})   {:>8.1} -> {:<8.1} s ({:+.1})\n",
            delta.compilations.0,
            delta.compilations.1,
            delta.compilations_changed(),
            delta.micros.0 as f64 / 1e6,
            delta.micros.1 as f64 / 1e6,
            delta.micros_changed() as f64 / 1e6,
        ));
        // WHOSE CODE THE DIFFERENCE IS IN, under the difference itself. A row
        // here is what makes the count above actionable: the same delta means a
        // repair to this repository's crates or a cache that carried sources
        // without their compiled form, and those are not the same finding.
        for (origin, (before, after)) in &delta.origins {
            out.push_str(&format!(
                "  {:<22} {:>6} -> {:<6} {:<28} {:>8.1} -> {:<8.1} s\n",
                "",
                before.times,
                after.times,
                origin.why(),
                before.micros as f64 / 1e6,
                after.micros as f64 / 1e6,
            ));
        }
        for (what, (before, after)) in &delta.unplaced {
            out.push_str(&format!(
                "  {:<22} {:>6} -> {:<6} NOT PLACED: `{}` — missing from every number above; {}\n",
                "",
                before.times,
                after.times,
                what.crate_name,
                what.why(),
            ));
        }
        if delta.started.is_empty() {
            out.push_str(&format!(
                "  {:<22} neither census said what it started from, which is the \
                 same silence rather than a difference\n",
                ""
            ));
        }
        // ONE LINE PER CACHE, because a job may restore more than one and their
        // states move independently — a single sentence about "the job" would be
        // a sentence about whichever the reader guessed.
        for (cache, (before, after)) in &delta.started {
            out.push_str(&format!(
                "  {:<22} `{cache}` began the same way — earlier: {}; later: {}\n",
                "",
                before.why(),
                after.why()
            ));
        }
    }
    match held.totals() {
        Some((before, after)) => out.push_str(&format!(
            "\n  CI paid          {:>6} -> {:<6} compilations ({:+})   {:>8.1} -> {:<8.1} s\n  \
             the floor was    {:>6} -> {:<6} distinct units\n",
            before.paid,
            after.paid,
            after.paid as i64 - before.paid as i64,
            before.micros as f64 / 1e6,
            after.micros as f64 / 1e6,
            before.floor,
            after.floor,
        )),
        None => out.push_str(
            "\n  NO TOTALS. A total is a sum over a population, so one job in a \
             different state spoils the whole of it — and the counts of a job \
             that was cold in one run and warm in the other differ by hundreds, \
             which is larger than any change a comparison is made to find\n",
        ),
    }
    for why in &held.incomparable {
        out.push_str(&format!("\n  REFUSED {why}"));
    }
    if !held.incomparable.is_empty() {
        out.push('\n');
    }
    out
}
