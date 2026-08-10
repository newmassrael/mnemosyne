//! Report what CI compiled, and refuse if a job that compiles left no record.
//!
//! Three ways in:
//!
//! - `twice-compiled <log directory>` reads logs the jobs of this run already
//!   wrote. This is what runs on a push: the measurement costs nothing beyond
//!   the builds that were happening anyway.
//! - `twice-compiled --replay <scratch>` produces those logs on this machine, by
//!   running each job's `run:` steps in a git worktree of its own. That is the
//!   expensive way, and the one that answers the question without a push.
//! - `twice-compiled compare <earlier> <later>` holds one census against
//!   another, each a `gh run download` directory. It reads no workflow and no
//!   repository — the two may be of different commits — and it REFUSES to total
//!   a pair whose jobs did not begin in the same state, which is the number
//!   Round 1099 quoted when it deleted a cache that was saving ten minutes.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

use ci_plan::RunStep;
use twice_compiled::{
    judge, load, unresolvable, Census, Cost, Declared, Entrance, Origin, JOB_VARIABLE,
    WRAPPER_VARIABLE,
};

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let entrance = twice_compiled::read_arguments(&arguments).unwrap_or_else(|why| {
        eprintln!("twice-compiled: {why}");
        std::process::exit(2);
    });
    match entrance {
        // THE ENTRANCE THAT NEEDS NOTHING THIS CHECKOUT HOLDS. Two censuses may
        // be of two different commits, so there is no workflow to read and no
        // repository to be in — what both runs uploaded is enough, because R1101
        // made every census say what it was taken under.
        Entrance::Compare { earlier, later } => hold_against(&earlier, &later),
        judged => judge_a_run(judged),
    }
}

/// Read two censuses and hold them against each other.
///
/// EXIT 1 WHEN THE TOTALS ARE NOT QUOTABLE, which is the verdict rather than an
/// aside: a delta between two runs that began in different states is the number
/// Round 1099 quoted, and it deleted a cache that was saving ten minutes.
fn hold_against(earlier: &str, later: &str) -> ! {
    let read = |directory: &str| {
        twice_compiled::load_collected(Path::new(directory)).unwrap_or_else(|error| {
            eprintln!("twice-compiled: cannot read {directory}: {error}");
            std::process::exit(2);
        })
    };
    let held = twice_compiled::compare(&read(earlier), &read(later));
    print!(
        "{}",
        twice_compiled::render_comparison(&held, earlier, later)
    );
    std::process::exit(if held.totals().is_some() { 0 } else { 1 })
}

/// The gate proper: judge one run's records, or a replay of them.
fn judge_a_run(entrance: Entrance) -> ! {
    let root = std::env::current_dir().expect("a working directory");
    assert!(
        root.join(".github/workflows").is_dir(),
        "run this from the repository root: {} has no .github/workflows",
        root.display()
    );

    let workflow = workflow_path(&root, &entrance);
    let document = ci_plan::load_workflow(&root, &workflow);
    let steps = ci_plan::run_steps(&document);
    // BOTH POPULATIONS, FROM ONE FILE AND ONE READER. The jobs that compile are
    // what the census is of; the jobs that restore a cache are what owes it a
    // record of the state it was taken in, and they are not the same jobs —
    // `tools/cache-budget` asks the second question of this same reader.
    let declared = Declared::of(
        &steps,
        &ci_plan::cache_steps(&document, &workflow),
        &ci_plan::artifact_uploads(&document, &workflow),
    );
    assert!(
        !declared.jobs.is_empty(),
        "{workflow} declares no job with a `run:` step at all — a census over \
         zero jobs is the empty answer that looks like a clean one"
    );

    let mut absent = BTreeSet::new();
    let logs = match &entrance {
        Entrance::Replay { scratch, .. } => replay(&root, &steps, Path::new(scratch), &mut absent),
        Entrance::Logs { directory, .. } => {
            // THE JOB THIS IS RUNNING IN, asked of the runner. Its own build is
            // still in flight while it judges, so it can have no log yet.
            if let Ok(mine) = std::env::var("GITHUB_JOB") {
                absent.insert(mine);
            }
            PathBuf::from(directory)
        }
        Entrance::Compare { .. } => unreachable!("`main` sends a comparison elsewhere"),
    };

    let census = load(&logs, &absent)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", logs.display()));
    report(&census, &declared, &absent, &root);

    let refusals = judge(&census, &declared, &absent);
    if refusals.is_empty() {
        println!(
            "\nevery one of the {} job(s) {workflow} declares recorded what it \
             compiled",
            declared.jobs.len() - absent.len()
        );
        std::process::exit(0);
    }
    println!();
    for refusal in &refusals {
        println!("REFUSED {refusal}");
    }
    std::process::exit(1);
}

/// `--workflow <path>`, or the one the runner says this job belongs to.
///
/// THE READING IS `ci-plan`'S, and R1107 is why it is not this function's any
/// more. `tools/cache-budget` came to need the same answer — which workflow this
/// run is of decides whose restore records could have been collected in it —
/// and a second cut of `owner/repo/<path>@<ref>` is a second answer free to
/// disagree with this one, which is the shape that crate exists to remove. It
/// also gained the check this had never made: the name is held against the files
/// this repository tracks, so a reference nothing recognises stops the gate
/// instead of pointing it at a path that is gone.
fn workflow_path(root: &Path, entrance: &Entrance) -> String {
    let named = match entrance {
        Entrance::Logs { workflow, .. } | Entrance::Replay { workflow, .. } => workflow.clone(),
        Entrance::Compare { .. } => unreachable!("a comparison reads no workflow"),
    };
    if let Some(path) = named {
        return path;
    }
    ci_plan::workflow_of_reference(
        std::env::var(ci_plan::WORKFLOW_VARIABLE).ok().as_deref(),
        &ci_plan::workflow_files(root),
    )
    .unwrap_or_else(|why| {
        panic!("no --workflow, and {why} — this gate judges the jobs of ONE workflow")
    })
}

/// Seconds, from the microseconds every number here is measured in.
fn seconds(micros: u64) -> f64 {
    micros as f64 / 1_000_000.0
}

/// A coverage row's split, spelled for the end of a line.
///
/// EMPTY WHEN THERE IS NOTHING IN IT, so a row that is honestly zero does not
/// carry a breakdown of zeros that reads like a measurement.
fn naming(split: &BTreeMap<Origin, Cost>) -> String {
    if split.is_empty() {
        return String::new();
    }
    let parts: Vec<String> = split
        .iter()
        .map(|(origin, cost)| format!("{} {}", cost.times, origin.why()))
        .collect();
    format!(" ({})", parts.join(", "))
}

/// Print the census. Everything, including the classes this reader cannot key —
/// a gate that prints only its findings cannot be told from one that never ran.
fn report(census: &Census, declared: &Declared, absent: &BTreeSet<String>, root: &Path) {
    println!("twice-compiled — every compilation CI pays for is one job's\n");
    for job in declared.jobs.keys() {
        if absent.contains(job) {
            println!("  {job:<22} not in this census");
            continue;
        }
        let Some(log) = census.jobs.get(job) else {
            println!("  {job:<22} NO LOG");
            continue;
        };
        println!(
            "  {job:<22} {:>6} compiled  {:>6} distinct  {:>5} repeated  \
             {:>5} probes  {:>4} unkeyed",
            log.compilations(),
            log.units.len(),
            log.repeats(),
            log.probes,
            log.unkeyed,
        );
        // THE SECOND LINE IS THE ONE THAT PRICES THE FIRST. `compiling` is the
        // area under every compiler the job ran and is larger than the job; the
        // window is from the first compiler's start to the last one's exit, so
        // their ratio is how many compilers were alive on average. The idle
        // figure is the part of that window with NO compiler in it at all —
        // a suite running what it just built — and it is what says how much of
        // this job removing work can actually reach.
        let window = log.span_micros();
        let busy = log.busy_micros();
        let at_once = if busy == 0 {
            0.0
        } else {
            log.micros as f64 / busy as f64
        };
        println!(
            "  {:<22} {:>8.1} s compiling within a {:>8.1} s window \
             ({at_once:.1} at once while busy, {:.1} s of it idle)",
            "",
            seconds(log.compiled_micros()),
            seconds(window),
            seconds(log.idle_micros()),
        );
        // THE THIRD LINE IS WHOSE CODE THE FIRST TWO WERE ABOUT, and it is the
        // one the line below is answerable against: every cache in this workflow
        // carries `~/.cargo/registry` and `~/.cargo/git`, which hold SOURCES.
        // A job that restored them exactly and still compiled hundreds of
        // fetched crates has a cache that brought the sources of work it then
        // did anyway, and no reading of the state alone can say that.
        let fetched = log.fetched();
        println!(
            "  {:<22} {:>6} of them fetched by cargo ({:>7.1} s), {:>5} from the \
             checkout ({:.1} s)",
            "",
            fetched.times,
            seconds(fetched.micros),
            log.compilations() - fetched.times,
            seconds(log.compiled_micros().saturating_sub(fetched.micros)),
        );
        // THE FOURTH LINE IS WHETHER THIS JOB'S CACHE REACHES WHERE THAT WORK
        // WENT. A restore can only spare what it brings back, so a compilation
        // written outside every path the cache declares is work no hit of it
        // could ever have avoided — a different finding from a cache that
        // missed, and until this was read the two were one number.
        match log.coverage(root, &declared.cached_paths(job)) {
            Some(found) => {
                for (path, split) in &found.held {
                    println!(
                        "  {:<22} {:>6} into `{path}`, which its cache holds{}",
                        "",
                        split.values().map(|cost| cost.times).sum::<usize>(),
                        naming(split),
                    );
                }
                for (tree, split) in &found.outside {
                    println!(
                        "  {:<22} {:>6} into `{tree}`, WHERE NO PATH OF ITS CACHE \
                         REACHES — no restore could have spared this{}",
                        "",
                        split.values().map(|cost| cost.times).sum::<usize>(),
                        naming(split),
                    );
                }
            }
            // WHERE IT WAS TAKEN IS PART OF WHAT IT SAYS. Answering anyway would
            // print every compilation as written outside the cache, which is
            // what looking in the wrong place produces rather than a finding.
            None => {
                println!(
                    "  {:<22} none of its destinations are under {} — this census \
                     was taken on another machine, so what its cache reaches \
                     cannot be read here; the destinations themselves can:",
                    "",
                    root.display(),
                );
                // THE DATA IT DOES HOLD, UNINTERPRETED. A reader told only that
                // the joining is unavailable has been handed nothing, and these
                // rows are what the join would have been made of.
                let mut written: BTreeMap<&str, BTreeMap<Origin, Cost>> = BTreeMap::new();
                for (into, cost) in &log.written {
                    written
                        .entry(into.tree().unwrap_or("<none named>"))
                        .or_default()
                        .entry(into.origin)
                        .or_default()
                        .absorb(*cost);
                }
                for (dir, split) in written {
                    println!(
                        "  {:<22} {:>6} into {dir}{}",
                        "",
                        split.values().map(|cost| cost.times).sum::<usize>(),
                        naming(&split),
                    );
                }
            }
        }
        if !log.unplaced.is_empty() {
            for (what, cost) in &log.unplaced {
                println!(
                    "  {:<22} {:>6} NOT PLACED, missing from the line above: \
                     `{}` — {}",
                    "",
                    cost.times,
                    what.crate_name,
                    what.why(),
                );
            }
        }
        // THE FOURTH LINE IS THE UNITS THE FIRST TWO ARE IN. cargo runs no
        // compiler for a unit that is already fresh, so this whole census is of
        // whatever was NOT restored, and the same job in two cache states is two
        // different numbers that are each correct. Round 1099 read two of them
        // as a controlled comparison and deleted the cache.
        //
        // A JOB WITH NO CACHE IS NOT A JOB WITH NO STATE — it is a job that
        // started from nothing every time, which is what having no cache means.
        // Saying so is the difference between the two silences.
        match (declared.caches.contains_key(job), census.restored.get(job)) {
            (_, Some(Ok(record))) => {
                // AND WHAT THAT COST, beside the seconds the job spent
                // compiling. The two numbers are the whole of "was this cache
                // worth having": a restore that takes longer than the compiling
                // it spared is a cache that made the job slower, and until the
                // record carried a clock neither this gate nor anything else
                // could put them next to each other.
                println!(
                    "  {:<22} started from: {} — the restore took {:.1} s, \
                     against {:.1} s this job spent compiling",
                    "",
                    record.warmth().why(),
                    seconds(record.restore_micros()),
                    seconds(log.busy_micros()),
                )
            }
            (_, Some(Err(why))) => println!("  {:<22} started from: UNREADABLE — {why}", ""),
            (true, None) => println!("  {:<22} started from: NOT SAID", ""),
            (false, None) => println!(
                "  {:<22} started from: an empty tree — this job declares no cache",
                ""
            ),
        }
    }

    let paid = census.paid();
    let floor = census.floor();
    println!(
        "\n  CI pays for   {paid:>6} compilations across {} job(s)  \
         {:>9.1} s of compiling",
        census.jobs.len(),
        seconds(census.paid_micros()),
    );
    println!(
        "  the floor is  {floor:>6} distinct units{:<26}{:>9.1} s",
        "",
        seconds(census.floor_micros()),
    );
    // WHOSE CODE CI IS PAYING TO COMPILE, under the total it is a share of. A
    // repair aimed at this repository's own crates cannot reach the fetched
    // rows, and a compile cache is a repair aimed at nothing else — so which of
    // these two lines is the large one decides what is worth building.
    for (origin, cost) in census.by_origin() {
        let share = if paid == 0 {
            0.0
        } else {
            100.0 * cost.times as f64 / paid as f64
        };
        println!(
            "  {:<13} {:>6} {:<20} ({share:.1}%){:<11}{:>9.1} s",
            "",
            cost.times,
            origin.why(),
            "",
            seconds(cost.micros),
        );
    }
    let unplaced = census.unplaced();
    if unplaced > 0 {
        println!(
            "  {:<13} {unplaced:>6} NOT PLACED — the two lines above are of a \
             population this reader lost part of",
            "",
        );
    }
    let duplicated = paid.saturating_sub(floor);
    let share = if paid == 0 {
        0.0
    } else {
        100.0 * duplicated as f64 / paid as f64
    };
    let surplus_micros = census.paid_micros().saturating_sub(census.floor_micros());
    let share_micros = if census.paid_micros() == 0 {
        0.0
    } else {
        100.0 * surplus_micros as f64 / census.paid_micros() as f64
    };
    println!(
        "  duplicated    {duplicated:>6} ({share:.1}% of what CI compiles){:<10}\
         {:>9.1} s ({share_micros:.1}%)",
        "",
        seconds(surplus_micros),
    );
    // THE SPLIT IS THE DECISION. One half exists because the work is spread over
    // jobs and would go away if those jobs were one job; the other half is a job
    // compiling the same unit twice inside itself, which no amount of merging
    // reaches — a job that builds several SEPARATE workspaces holds a `target`
    // per workspace. A single percentage licenses whichever repair was already
    // preferred; these two lines say which one the number is actually about.
    println!(
        "    of which  {:>6} because the work is split across jobs (merging \
         removes){:<1}{:>9.1} s",
        census.shared_between_jobs(),
        "",
        seconds(census.shared_between_jobs_micros()),
    );
    println!(
        "              {:>6} inside one job, over its own separate workspaces \
         (merging does not){:>9.1} s",
        census.repeated_within_jobs(),
        seconds(census.repeated_within_jobs_micros()),
    );
    // AND WHAT THE WHOLE FAMILY OF REPAIRS IS WORTH, which is the line every
    // number above needs beside it. R1098 measured this by hand — 75.4% of these
    // windows idle, so all compile-side repair together was worth at most 394.5 s
    // on the critical path — and an arc decision was taken on it. Nothing printed
    // it, so nothing could say when it stopped being true, and the question it
    // settled came back five times under five names.
    //
    // THE TWO QUANTITIES ARE NAMED APART because one is a share of TIME and the
    // one above it is a share of WORK: 38% of the compiler-seconds being surplus
    // and 75% of the wall-clock having no compiler in it at all are both true,
    // and only the second says what a repair can reach.
    let window = census.window_micros();
    let idle = if window == 0 {
        0.0
    } else {
        100.0 * census.idle_micros() as f64 / window as f64
    };
    println!(
        "\n  the ceiling on ALL of it is {:.1} s of critical path — these jobs run \
         beside each other, so a repair that removed EVERY compilation shortens \
         the run by at most the busiest job's own compiling ({:.1} s of window \
         across the census, {idle:.1}% of it with no compiler alive at all)",
        seconds(census.ceiling_micros()),
        seconds(window),
    );
    // THE STATE THE TOTALS ARE IN, printed with them rather than left in the
    // per-job lines above, because the number that gets quoted is this one and
    // the number that got quoted is what deleted a cache that was working.
    started_in(census, declared, absent);

    let pairwise = census.pairwise();
    if pairwise.is_empty() {
        println!("\n  no two jobs compile the same unit");
        return;
    }
    // RANKED BY SECONDS AND NOT BY ROWS. The two orders are different orders,
    // and the round that built this reader had already written the repair list
    // in the row order before it could see the other one.
    println!("\n  what merging a pair would remove, and what it would cost:");
    let mut rows: Vec<((&str, &str), twice_compiled::Merge)> = pairwise.into_iter().collect();
    rows.sort_by_key(|(pair, merge)| (std::cmp::Reverse(merge.saved_micros), *pair));
    for ((left, right), merge) in rows {
        println!("    {left} + {right}");
        println!(
            "      {:>6} compilations  {:>8.1} s of compiling removed",
            merge.units,
            seconds(merge.saved_micros),
        );
        // THE OTHER SIDE OF THE TRADE, printed beside the saving rather than
        // left to whoever acts on it: the two jobs run at the same time today,
        // so the merged one starts from the longer of them and can only grow.
        println!(
            "      window  {:>8.1} s today  ->  {:>8.1} s estimated \
             (never below {:.1} s, never above {:.1} s; {:.1} s of it is idle \
             and scales with nothing)",
            seconds(merge.floor_micros),
            seconds(merge.estimate_micros),
            seconds(merge.floor_micros),
            seconds(merge.ceiling_micros),
            seconds(merge.idle_micros),
        );
    }

    // WHICH REPAIR the number licenses depends on what is in it: duplication in
    // third-party dependencies is answered by a shared compilation cache, and
    // duplication in this repository's own crates is answered by the jobs being
    // one job. Twenty rows because the tail is long and the head is the decision.
    let surplus = census.surplus_by_crate();
    let total: usize = surplus.values().map(|cost| cost.times).sum();
    println!(
        "\n  the {} crate(s) compiled more than once, {total} surplus \
         compilation(s), dearest first:",
        surplus.len()
    );
    // DEAREST AND NOT MOST NUMEROUS. A build script is a row and almost no
    // money; a test binary of this repository's own is the other way round, and
    // which of them heads this list is which repair the number licenses.
    let mut crates: Vec<(&str, twice_compiled::Cost)> = surplus.into_iter().collect();
    crates.sort_by_key(|(name, cost)| (std::cmp::Reverse(cost.micros), *name));
    for (name, cost) in crates.iter().take(20) {
        println!(
            "    {:>6}  {:>8.1} s  {name}",
            cost.times,
            seconds(cost.micros)
        );
    }
    if crates.len() > 20 {
        let times: usize = crates[20..].iter().map(|(_, cost)| cost.times).sum();
        let micros: u64 = crates[20..].iter().map(|(_, cost)| cost.micros).sum();
        println!(
            "    {times:>6}  {:>8.1} s  … and {} more crate(s)",
            seconds(micros),
            crates.len() - 20
        );
    }
}

/// The cache state the totals above were measured in, in one sentence.
///
/// A CENSUS IS NOT COMPARABLE TO ANOTHER CENSUS TAKEN IN A DIFFERENT ONE, and
/// the whole cost of learning that was a 7.5 GB cache deleted for saving nothing
/// while it was saving 426 compilations. The sentence is printed even when every
/// job started the same way, because the reader who needs it is the one holding
/// two reports, and a line that appears only sometimes is a line nobody looks
/// for.
fn started_in(census: &Census, declared: &Declared, absent: &BTreeSet<String>) {
    let started = census.started();
    // A JOB WITH NO CACHE STARTS FROM NOTHING EVERY RUN, which is a state that
    // cannot vary and therefore cannot be the difference between two censuses.
    // It is counted apart from the jobs that restored nothing DESPITE a cache,
    // because those two are the same disk and different findings.
    let cacheless: Vec<&str> = census
        .jobs
        .keys()
        .filter(|job| !declared.caches.contains_key(*job) && !absent.contains(*job))
        .map(String::as_str)
        .collect();
    if started.is_empty() {
        println!(
            "\n  no job with a cache in this census said what it started from, \
             so these totals are in no units at all — they are of whatever was \
             not already on the disk, and nothing here says what that was \
             ({} job(s) declare no cache and always start from nothing)",
            cacheless.len()
        );
        return;
    }
    // COUNTED PER RESTORE, WHICH IS NO LONGER PER JOB. A job that declares two
    // caches can be warm in one and cold in the other, and a tally that put it
    // in one bucket would be answering about whichever cache the reader guessed
    // — the very substitution R1117 split the record to prevent.
    let mut exact = Vec::new();
    let mut prefix = Vec::new();
    let mut nothing = Vec::new();
    let mut contradictory = Vec::new();
    for (restore, warmth) in &started {
        let named = restore.to_string();
        match warmth {
            restored::Warmth::ExactHit { .. } => exact.push(named),
            restored::Warmth::PrefixHit { .. } => prefix.push(named),
            restored::Warmth::Nothing => nothing.push(named),
            restored::Warmth::HitThatBroughtNothing => contradictory.push(named),
        }
    }
    let cacheless: Vec<String> = cacheless.iter().map(|job| format!("`{job}`")).collect();
    println!(
        "\n  taken with {} restore(s) warm from an exact hit, {} warm from an \
         earlier generation, {} from nothing{}",
        exact.len(),
        prefix.len(),
        nothing.len(),
        if contradictory.is_empty() {
            String::new()
        } else {
            format!(", {} contradicting itself", contradictory.len())
        }
    );
    for (label, named) in [
        ("exact hit", &exact),
        ("earlier generation", &prefix),
        ("nothing", &nothing),
        ("contradiction", &contradictory),
        ("no cache at all", &cacheless),
    ] {
        if !named.is_empty() {
            println!("    {label:<20} {}", named.join(", "));
        }
    }
    println!(
        "  a census taken in another state is not this one's control: cargo \
         runs no compiler for a unit that is already fresh, so these counts are \
         of what was NOT restored"
    );
}

/// Steps a replay on this machine must not run, and why.
///
/// NAMED PREFIXES rather than a general judgement about what a step does: these
/// two install what a hosted runner lacks and this machine already has, and
/// every other step runs verbatim. A step that needed something else would fail
/// loudly rather than be silently skipped.
const NOT_OURS: [(&str, &str); 2] = [
    ("sudo", "installs a runner package this machine already has"),
    ("rustup", "installs a toolchain this machine already has"),
];

/// Build the recorder, then run every job's steps in a worktree of its own.
///
/// A WORKTREE PER JOB rather than one checkout with the target directory moved
/// aside, because a job's cost is spread over several `target` directories — the
/// root one, and one per tool workspace a gate shells into. Redirecting them all
/// into one would make cargo share units between them that a runner compiles
/// twice, which is the very number being measured.
///
/// EVERY WORKTREE FROM ONE REVISION, RESOLVED ONCE AND PRINTED. A CI run measures
/// a single commit; a replay takes hours, and `HEAD` is a moving name — a commit
/// landing in the repository while it works silently splits the census across two
/// source trees. That is not hypothetical: a replay of this workflow took its
/// first four jobs from one commit and its fifth from another that arrived
/// mid-run, and nothing in the report said so. The revision is therefore resolved
/// before the first worktree and passed to all of them, and the report says which
/// one it was — a census that cannot name what it measured is one nobody can
/// compare against anything.
fn replay(
    root: &Path,
    steps: &[RunStep],
    scratch: &Path,
    absent: &mut BTreeSet<String>,
) -> PathBuf {
    let revision = resolve_head(root);
    println!("[replay] every job from {revision}, resolved once");
    let wrapper = build_wrapper(root);
    let logs = scratch.join("logs");
    std::fs::create_dir_all(&logs).expect("a log directory");

    let mut by_job: BTreeMap<&str, Vec<&RunStep>> = BTreeMap::new();
    for step in steps {
        by_job.entry(step.job.as_str()).or_default().push(step);
    }

    for (job, steps) in by_job {
        // A REFUSAL RATHER THAN A GUESS. `RUSTUP_TOOLCHAIN: ${{ steps.msrv…}}`
        // is resolved by GitHub from a step this replay does not run, and a job
        // replayed on the wrong toolchain would report its units as shared with
        // every other job — the loudest possible wrong answer.
        if let Some(reason) = unresolvable(&steps) {
            println!("[replay] SKIP {job} — {reason}");
            absent.insert(job.to_string());
            continue;
        }

        let tree = scratch.join("trees").join(job);
        make_worktree(root, &tree, &revision);
        let log = logs.join(format!("{job}.log"));
        let _ = std::fs::remove_file(&log);
        let restore = logs.join(format!("{job}.restored"));
        let _ = std::fs::remove_file(&restore);

        for step in steps {
            let head = step.script.split_whitespace().next().unwrap_or_default();
            if let Some((_, why)) = NOT_OURS.iter().find(|(word, _)| *word == head) {
                println!("[replay] {job}: skipping `{head}` — {why}");
                continue;
            }
            println!("[replay] {job}: {}", step.script.replace('\n', " "));
            // THE TWO VARIABLES THIS REPLAY OWNS, applied last so that whatever
            // the workflow spells for them is replaced. `twice_compiled::
            // unresolvable` skips exactly these names for that reason, and both
            // sides read the one list so they cannot come apart.
            // THE VARIABLES THIS REPLAY OWNS. The last three are what make a
            // replayed census say what it is: a replay runs a job's `run:`
            // steps and never its `uses:` cache step, so nothing is restored,
            // `cache-hit` is honestly `false`, and the two measurements around
            // the absent restore record a job that started from an empty tree —
            // which is exactly what it did.
            let mine = [
                (WRAPPER_VARIABLE, wrapper.as_os_str()),
                (rustc_log::LOG_VARIABLE, log.as_os_str()),
                (restored::VARIABLE, restore.as_os_str()),
                (restored::EXACT_VARIABLE, OsStr::new("false")),
                (JOB_VARIABLE, OsStr::new(job)),
            ];
            debug_assert!(
                mine.iter()
                    .all(|(name, _)| twice_compiled::REPLAY_SETS.contains(name)),
                "a variable set here and not named in REPLAY_SETS is one \
                 `unresolvable` would refuse a job over"
            );
            let status = Command::new("bash")
                .arg("-c")
                .arg(&step.script)
                .envs(&step.env)
                .envs(mine)
                .current_dir(&tree)
                .status()
                .expect("the step runs");
            if !status.success() {
                // NOT FATAL, and named rather than swallowed: the compilations
                // this step paid for already happened and are already in the
                // log, so the census is still the truth about what was built.
                println!("[replay] {job}: exited {status} — its compilations still count");
            }
        }

        drop_worktree(root, &tree);
    }
    logs
}

fn build_wrapper(root: &Path) -> PathBuf {
    let manifest = root.join("tools/rustc-log/Cargo.toml");
    // WHERE IT GOES IS TOLD AND NOT DISCOVERED. This checkout's
    // `.cargo/config.toml` sets `build.target-dir`, which cargo resolves against
    // the directory holding that file — but a replay builds this in a REPOSITORY
    // OF ITS OWN, and a fixture that carries no such file puts the binary under
    // the tool's own workspace instead. A path assumed either way is a guess
    // about somebody else's tree; saying it makes the answer the same in both.
    let build = root.join("target");
    let status = Command::new("cargo")
        .args(["build", "--release", "-q", "--manifest-path"])
        .arg(&manifest)
        .env("CARGO_TARGET_DIR", &build)
        // The recorder cannot record its own build: it does not exist yet.
        .env(WRAPPER_VARIABLE, "")
        .status()
        .expect("cargo builds the recorder");
    assert!(status.success(), "cannot build the recorder");
    let wrapper = build.join("release/rustc-log");
    assert!(wrapper.is_file(), "no recorder at {}", wrapper.display());
    wrapper
}

/// The commit `HEAD` names right now, as a hash that will not move.
///
/// ASKED ONCE AND OF GIT. `HEAD` is a name for whatever the repository is on, and
/// a replay outlives that: resolving it per job is how a census comes to be of two
/// trees at once.
fn resolve_head(root: &Path) -> String {
    let out = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .expect("git rev-parse runs");
    assert!(
        out.status.success(),
        "cannot resolve HEAD in {}: {}",
        root.display(),
        String::from_utf8_lossy(&out.stderr).trim()
    );
    let revision = String::from_utf8_lossy(&out.stdout).trim().to_string();
    assert!(
        !revision.is_empty(),
        "git resolved HEAD to nothing in {}",
        root.display()
    );
    revision
}

fn make_worktree(root: &Path, tree: &Path, revision: &str) {
    let _ = std::fs::remove_dir_all(tree);
    std::fs::create_dir_all(tree.parent().expect("a parent")).expect("a worktree parent");
    let status = Command::new("git")
        .args(["worktree", "add", "--detach", "--force"])
        .arg(tree)
        .arg(revision)
        .current_dir(root)
        .status()
        .expect("git worktree add runs");
    assert!(
        status.success(),
        "cannot add a worktree at {}",
        tree.display()
    );
}

fn drop_worktree(root: &Path, tree: &Path) {
    let status = Command::new("git")
        .args(["worktree", "remove", "--force"])
        .arg(tree)
        .current_dir(root)
        .status()
        .expect("git worktree remove runs");
    if !status.success() {
        println!(
            "[replay] could not remove {} — remove it by hand",
            tree.display()
        );
    }
}
