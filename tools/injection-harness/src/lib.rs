//! What a sweep IS, and the one law every reader of one obeys.
//!
//! THE MANIFEST TYPES AND THE ANCHOR LAW LIVE HERE RATHER THAN IN `main.rs`
//! BECAUSE A DECISION IN `main.rs` HAS NO READER. R1096 measured what that costs
//! in this repository: the thing that lied was an exit code, and the whole of
//! what a suite could ask of it was that one number. The law below is now
//! askable — of a fixture, and of every sweep this repository tracks.
//!
//! AND THAT SECOND READER IS THE POINT. An injection sweep is a proof, and its
//! anchors are the part of it that decays: they are exact text, and the source
//! they name moves. R1103 inserted one line between the two lines an anchor
//! matched on, and from that commit the census gate's `every-job-owes-a-restore-record`
//! injection applied to nothing. Nothing said so, because the only thing that
//! checks an anchor is a sweep being RUN — and a sweep is run when somebody
//! decides to run it. R1107 found it four rounds later, by running one.
//!
//! A proof that has quietly stopped proving reads exactly like one that holds.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// One textual replacement in one file. `from` must occur EXACTLY once.
///
/// Comparable since R1198, because a firing record keeps the edits it was proven
/// against and the gate holds them against the manifest's.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Edit {
    pub file: String,
    pub from: String,
    pub to: String,
}

/// What a manifest CLAIMS the `expect_red` lists below it to be.
///
/// THE CLAIM IS DATA BECAUSE IT DECIDES WHAT A RUN MEANS. A sweep answers two
/// questions with one set of runs — does breaking this read redden the contract
/// at all, and is the contract the ONLY thing that notices — and the second is
/// worth nothing unless the run could have seen everything that might. Round
/// 1138 answered it off a run scoped to one crate while its edits landed in
/// another, so the suite of the crate it broke never ran; Round 1139 widened the
/// scope and found twelve reds that scope could not have shown, six of the seven
/// injections being caught by the edited crate's own tests.
///
/// Declaring the set exhaustive is what makes that self-detecting from then on:
/// run the same manifest narrower and the missing reds are named, run it wider
/// and the new ones are, where a subset claim is silent in both directions.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RedSet {
    /// `expect_red` names tests that MUST go red and says nothing about the
    /// rest. The default, and the only honest reading for a sweep whose whole
    /// red set nobody has measured.
    #[default]
    AtLeast,
    /// `expect_red` names the WHOLE red set, so a red the manifest does not name
    /// fails the sweep instead of being counted beside it.
    Exhaustive,
}

/// One injection: what it breaks, and what the sweep expects to go red.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Injection {
    pub name: String,
    /// What this injection is FOR, in the author's words — carried into the
    /// report so a number is never read without the claim it is evidence for.
    #[serde(default)]
    pub why: String,
    pub edits: Vec<Edit>,
    /// Test names this injection is expected to turn red. Empty means "say what
    /// went red and judge nothing", which is honest for an exploratory sweep;
    /// naming them makes the harness itself fail when the sweep does not reach
    /// what it was aimed at (the "0 means suspect the injection" rule).
    #[serde(default)]
    pub expect_red: Vec<String>,
}

/// What one injection was PROVEN to do, the last time somebody ran it.
///
/// R1198 — AN INJECTION THAT HAS NEVER BEEN RUN IS NOT A PROOF, and until this
/// existed nothing could tell one from an injection that has. The three laws
/// over sweeps ask whether the anchor still applies, whether the suite still
/// exists and whether the named tests still exist — all questions about TEXT,
/// all answerable without running anything, and all of them true of an injection
/// that would redden nothing at all. Writing one is cheap and running it costs a
/// suite, so the gap is exactly where a sweep quietly stops being evidence.
///
/// THE DEFINITION IS KEPT BESIDE THE RESULT rather than hashed, because the two
/// spellings are compared and a reader of a failure has to see WHAT was proven.
/// That is the shape R1116 established for a cache key spelled twice: the
/// duplication is not the defect, an unread duplicate is.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Firing {
    /// The injection AS IT WAS WHEN IT FIRED. A later edit to the edits or to
    /// what they are expected to redden makes this row about a different
    /// injection, and the gate says so rather than carrying the old evidence
    /// forward under the new name.
    pub edits: Vec<Edit>,
    #[serde(default)]
    pub expect_red: Vec<String>,
    /// Every test that went red under it and not under the control.
    pub tests: Vec<String>,
}

/// Evidence a person has decided answers to nothing, and their reason.
///
/// R1258 — THE DECISION IS THE ARTEFACT, NOT THE DELETION. A row for an
/// injection the manifest no longer has is caught by the law over these records,
/// and that is right: a rename or a deletion has left a proof pointing at
/// nothing, and somebody has to say which it was. What was missing is a way to
/// SAY it. The only way out was to delete the whole record and re-prove every
/// injection in it — here that was five, in `tools/twice-compiled` it is
/// sixty-six — and the cheap way out, editing the row out of the file by hand,
/// destroys the very decision the law existed to force. Both of those end with
/// the reason unwritten.
///
/// SO THE ROW IS KEPT, WHOLE, BESIDE THE REASON IT WAS RETIRED. Nothing is
/// destroyed: a reader who later meets an injection whose edits look familiar
/// can see that it was proven once, under another name, and what it reddened
/// then. The record stops CLAIMING it as live evidence, which is all the law
/// ever asked.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Forgotten {
    /// Why the person who ran `forget` says this evidence answers to nothing —
    /// the renamed-to, the re-aiming, the injection that was withdrawn. It is
    /// the whole content of the decision, so it is refused when it is blank.
    pub because: String,
    /// The row as it stood, unaltered. `Firing` and not a summary of one: a
    /// second spelling of "what was proven" would be free to disagree with the
    /// first, and this is a file whose worth is that nobody can type into it.
    pub was: Firing,
}

/// Every injection of one sweep that has been shown to fire.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Firings {
    /// Written for a person who opens the file, so that "machine-written" is
    /// said in the file rather than only in the code that writes it.
    #[serde(rename = "_", default)]
    pub prose: Vec<String>,
    /// Whether a run that covered the WHOLE manifest wrote this.
    ///
    /// THE CLAIM AND THE EVIDENCE ARE KEPT APART SO THAT THE GATE CAN COMPARE
    /// THEM, which is where this law's teeth are: a record that says it is whole
    /// and does not cover some injection is a manifest that gained one since it
    /// was proven, and that is exactly the case R1179's `--only` made cheap to
    /// create. A record built up out of `--only` runs never claims it, so
    /// proving one new injection stays a one-injection job instead of putting
    /// the other sixteen under an obligation — which is what the first draft of
    /// this did, and the reason the field exists.
    #[serde(default)]
    pub complete: bool,
    /// By injection name.
    pub fired: BTreeMap<String, Firing>,
    /// Rows retired by name, by the person who decided they answer to nothing.
    ///
    /// WRITTEN EVEN WHEN EMPTY, and that is the same distinction `read_firings`
    /// refuses to blur one line up: a key that disappears when there is nothing
    /// in it makes "nobody has forgotten anything here" and "this record was
    /// written before the decision could be recorded at all" the same file. One
    /// of those is an answer and the other is an absence.
    #[serde(default)]
    pub forgotten: BTreeMap<String, Forgotten>,
}

/// What this repository suffixes a sweep's firing record with.
pub const FIRINGS_SUFFIX: &str = ".firings.json";

/// The record that belongs to one manifest, whether or not it exists yet.
///
/// NAMED FROM THE MANIFEST AND NOT FROM ITS DIRECTORY, which is a repair this
/// round made against its own first draft: `tools/ci-plan/` holds TWO sweeps,
/// and one record per directory made the second one's every injection read as
/// unproven while the first one's evidence read as answering to nothing. It is
/// the same correction R1117 made for a restore record that was a job's when a
/// job may declare two caches — a record belongs to the thing it is about.
#[must_use]
pub fn firings_path(manifest: &Path) -> PathBuf {
    let stem = manifest
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix(".json"))
        .unwrap_or("sweep");
    manifest
        .parent()
        .unwrap_or(Path::new("."))
        .join(format!("{stem}{FIRINGS_SUFFIX}"))
}

/// Read one, `Ok(None)` when a sweep has never had one written.
///
/// A file that exists and does not read as a record is an ERROR rather than an
/// absence, for the reason every reader here refuses rather than defaults: a
/// record nobody can parse and a sweep nobody has run look the same to a gate
/// that treats both as "no evidence", and only one of them is honest.
pub fn read_firings(path: &Path) -> Result<Option<Firings>, String> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("{} unreadable: {e}", path.display())),
    };
    serde_json::from_str(&raw)
        .map(Some)
        .map_err(|e| format!("{} is not a firing record: {e}", path.display()))
}

/// Write one, pretty and newline-terminated.
///
/// HERE RATHER THAN AT EACH CALLER because there are now two — the run that
/// records what fired, and the pass that voids evidence about an injection that
/// has been edited since — and a record serialised two ways is a file whose
/// shape depends on which path last touched it.
pub fn write_firings(path: &Path, record: &Firings) -> Result<(), String> {
    let mut serialized =
        serde_json::to_string_pretty(record).map_err(|e| format!("{}: {e}", path.display()))?;
    serialized.push('\n');
    fs::write(path, serialized).map_err(|e| format!("{} unwritable: {e}", path.display()))
}

/// Does this row record the injection as it is written today?
#[must_use]
pub fn records_the_same_injection(row: &Firing, injection: &Injection) -> bool {
    row.edits == injection.edits && row.expect_red == injection.expect_red
}

/// Drop the rows that are evidence about a definition this manifest no longer
/// has, for the injections a run is ABOUT TO re-prove. Returns their names.
///
/// WHY THIS HAPPENS BEFORE THE CONTROL AND NOT AFTER THE RUN. R1198's law says a
/// row proven against a different definition is evidence about another
/// injection, and it says so by turning red — which is right, and which for ONE
/// sweep in this repository is a deadlock. The harness's own `self-check.sweep.json`
/// runs the harness's own suite as its control, and that suite HOLDS that law:
/// edit a proven injection there and the stale row reddens the control, the
/// harness refuses to start on a red control, and the `--only` run that would
/// clear the row can never happen. R1199 hit it and escaped only because the row
/// was still uncommitted, so `git checkout` could remove it; for a committed one
/// the only way out was to delete the whole record and re-prove every injection
/// in it, which is the cost `--only` exists to avoid.
///
/// VOIDING IS NOT LAUNDERING, and the difference is that this removes evidence
/// rather than inventing it. A row that still matches its injection is left
/// alone — nothing about it is stale, and a pass that dropped it would be
/// throwing away a proof. What is dropped is exactly what the gate would have
/// called void, by the SAME predicate the gate uses, and the run that drops it
/// is on its way to proving that injection again.
///
/// AND THE COMPLETENESS CLAIM CANNOT SURVIVE IT. `complete` says a run over the
/// whole manifest established every row; with one of those rows gone the claim
/// has nothing under it, and leaving it set would be the one shape R1198 built
/// this record to catch — a file that claims to be whole while some injection is
/// unaccounted for.
pub fn void_stale_evidence(manifest: &Path, about: &[Injection]) -> Result<Vec<String>, String> {
    let path = firings_path(manifest);
    let Some(mut record) = read_firings(&path)? else {
        return Ok(Vec::new());
    };
    let voided = stale_evidence(&record, about);
    if !voided.is_empty() {
        for name in &voided {
            record.fired.remove(name);
        }
        record.complete = false;
        write_firings(&path, &record)?;
    }
    Ok(voided)
}

/// Which of these injections a record holds evidence about that no longer
/// describes them.
///
/// PURE, AND BOTH READERS DRIVE IT. One voids what it is about to re-prove; the
/// other explains a red control by naming the rows the run did NOT select — and
/// two spellings of "is this row stale" would let those two disagree about the
/// same file, in a place where the disagreement reads as a sweep that simply
/// cannot be run.
#[must_use]
pub fn stale_evidence(record: &Firings, about: &[Injection]) -> Vec<String> {
    about
        .iter()
        .filter(|injection| {
            record
                .fired
                .get(&injection.name)
                .is_some_and(|row| !records_the_same_injection(row, injection))
        })
        .map(|injection| injection.name.clone())
        .collect()
}

/// Retire one row by name, with the reason it answers to nothing. Returns what
/// was retired.
///
/// R1258 — THE VERB THE VOIDING PASS ABOVE IS NOT. Those two look alike and are
/// opposite in the thing that matters: voiding is about an injection the
/// manifest STILL HAS, it is decided by a predicate rather than by a person, and
/// the run that does it is on its way to proving that injection again in the
/// next minute. Forgetting is about an injection the manifest DOES NOT HAVE, it
/// is a judgement nothing can derive — a rename, a re-aiming, a withdrawal —
/// and no run is coming to replace what it retires. A pass that decided this one
/// by itself would be a program deciding that a proof is no longer owed, which
/// is the one thing this record exists to stop.
///
/// WHAT IT REFUSES, AND WHY EACH REFUSAL HAS NO FLAG TO TURN IT OFF:
///
///   - THE MANIFEST STILL NAMES IT. This is the laundering path, whole: an
///     injection that is still in the sweep still owes evidence, and dropping
///     its row would leave the manifest looking proven-enough while one of its
///     claims has quietly stopped being measured. `--only <name>` re-proves it
///     for the price of one injection; that is the answer, and it is cheap
///     precisely so this refusal can be absolute.
///   - THE RECORD HOLDS NO SUCH ROW. A name nobody can find is a caller whose
///     belief about this file is already wrong — most often a typo, sometimes a
///     record other than the one they meant — and a silent success would let
///     them believe a decision was recorded when nothing was.
///   - IT IS ALREADY FORGOTTEN. Answering "done" would overwrite the first
///     person's reason with the second's, and the first reason is the older and
///     better-attested of the two.
///   - THE REASON IS BLANK. The reason IS the artefact. A row retired with an
///     empty one has thrown the decision away just as surely as deleting it by
///     hand, only with a file left behind that looks like a record of it.
///
/// THE COMPLETENESS CLAIM SURVIVES THIS, where the voiding pass above must
/// withdraw it. `complete` says a run over the whole manifest established a row
/// for every injection in it; retiring a row for an injection that is NO LONGER
/// in the manifest cannot uncover one, so the claim is still true of the
/// manifest as it stands — and the law re-checks that coverage on every run, so
/// the claim is not being taken on trust here. Withdrawing it would do real
/// damage in the other direction: a record demoted to partial demands nothing of
/// the rows beside it, which is exactly the tooth this record was built to keep.
pub fn forget(manifest: &Path, name: &str, because: &str) -> Result<Forgotten, String> {
    if because.trim().is_empty() {
        return Err(format!(
            "forgetting `{name}` records a DECISION, and its reason is the whole \
             of it — say what happened to that injection"
        ));
    }
    let sweep: Manifest = read_manifest(manifest)?;
    if sweep.injections.iter().any(|i| i.name == name) {
        return Err(format!(
            "{} still names the injection `{name}`, so its evidence is still \
             owed. Re-prove it with `--only {name}`; forgetting a row an \
             injection still needs would leave that claim unmeasured and the \
             record looking whole",
            manifest.display()
        ));
    }
    let path = firings_path(manifest);
    let Some(mut record) = read_firings(&path)? else {
        return Err(format!(
            "{} does not exist, so there is no evidence about `{name}` to forget",
            path.display()
        ));
    };
    if let Some(already) = record.forgotten.get(name) {
        return Err(format!(
            "{} already records `{name}` as forgotten, because: {}",
            path.display(),
            already.because
        ));
    }
    let Some(was) = record.fired.remove(name) else {
        let held: Vec<&String> = record.fired.keys().collect();
        return Err(format!(
            "{} records no firing for `{name}` — it holds {held:?}",
            path.display()
        ));
    };
    let retired = Forgotten {
        because: because.trim().to_string(),
        was,
    };
    record.forgotten.insert(name.to_string(), retired.clone());
    write_firings(&path, &record)?;
    Ok(retired)
}

/// What this tool answers to as its first word.
///
/// R1258 — THE VOCABULARY IS A LIBRARY FACT BECAUSE IT HAS TWO READERS. One is
/// the dispatch in `main.rs`. The other is the law over what every tracked sweep
/// manifest documents about running itself: thirty-one of them carry the command
/// line in their own prose, and when this round gave the tool verbs all
/// thirty-one went stale at once, silently, because nothing had ever read one.
/// A vocabulary spelled in `main.rs` alone is one the law would have to spell
/// again, and two spellings of what a tool answers to is how the documentation
/// comes to describe a tool that is not there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verb {
    /// Run a sweep: the control, then each injection, then the report.
    Sweep,
    /// Retire one row of a firing record, with the reason it answers to
    /// nothing.
    Forget,
    /// NOT FOR A PERSON TO TYPE: this binary re-exec'd as the owner of one suite
    /// run. `supervise::supervised_command` is the only thing that spells it,
    /// and it is in this enum rather than beside it because the first word is
    /// read in ONE place — a word handled somewhere else is the second reader
    /// this type exists to prevent.
    Supervise,
}

impl Verb {
    /// How a person is told what this tool answers to.
    pub const USAGE: &'static str = "sweep <manifest.json> [--control-only] \
                                     [--only <name>]... | forget <manifest.json> \
                                     --injection <name> --because <why>";

    /// The verb this word names, if it names one.
    #[must_use]
    pub fn of(word: &str) -> Option<Verb> {
        match word {
            "sweep" => Some(Verb::Sweep),
            "forget" => Some(Verb::Forget),
            "supervise" => Some(Verb::Supervise),
            _ => None,
        }
    }
}

/// A command a sweep manifest documents for running itself, in two halves.
///
/// THE TWO HALVES ARE DIFFERENT CLAIMS AND THE SEPARATOR IS WHAT DIVIDES THEM:
/// what comes before `--` decides WHICH PROGRAM runs, and what comes after it is
/// that program's own argv. A header can be wrong in either — pointed at another
/// crate's manifest, or at another sweep's file — and the two mistakes read
/// nothing alike to whoever pastes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentedCommand {
    /// Up to the separator: the cargo invocation that chooses the program.
    pub cargo: Vec<String>,
    /// After it: the words this tool is handed.
    pub argv: Vec<String>,
}

/// What is wrong with a documented command, if anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Misdirected {
    /// It does not say which crate's binary to run, so it runs whatever the
    /// working directory's workspace happens to build.
    NoTool,
    /// It runs a different crate's binary: the manifest path it names.
    AnotherTool(String),
    /// Its first word after the separator is not a verb this tool answers to.
    NotAVerb(String),
    /// It names a verb and no manifest for it.
    NoManifest,
    /// It names a different sweep: the path it points at.
    AnotherManifest(String),
    /// It carries words past the manifest. A header is `sweep <this file>` and
    /// nothing else — every tracked one is — so what is here is a flag or an
    /// argument nobody checked, and checking it any other way would mean this
    /// crate parsing its own command line a second time.
    Extra(Vec<String>),
}

/// Whether a documented command would run THIS manifest, with THIS tool.
///
/// PURE, AND BOTH READERS DRIVE IT — the shape [`stale_evidence`] and
/// [`unsound_decisions`] are in, and the reason is the same: the law over the
/// tracked manifests is a walk over files that are all correct today, so a
/// fixture is the only thing that can ask this judgement a question it should
/// answer NO to.
#[must_use]
pub fn misdirected(command: &DocumentedCommand, manifest: &str) -> Vec<Misdirected> {
    let mut faults = Vec::new();
    match command
        .cargo
        .iter()
        .position(|word| word == "--manifest-path")
        .and_then(|at| command.cargo.get(at + 1))
    {
        Some(named) if named.ends_with(HARNESS_MANIFEST) => {}
        Some(named) => faults.push(Misdirected::AnotherTool(named.clone())),
        None => faults.push(Misdirected::NoTool),
    }
    match command.argv.first() {
        Some(word) if Verb::of(word).is_some() => {}
        Some(word) => faults.push(Misdirected::NotAVerb(word.clone())),
        None => faults.push(Misdirected::NoManifest),
    }
    match command.argv.get(1) {
        Some(named) if named == manifest => {}
        Some(named) => faults.push(Misdirected::AnotherManifest(named.clone())),
        None => faults.push(Misdirected::NoManifest),
    }
    if command.argv.len() > 2 {
        faults.push(Misdirected::Extra(command.argv[2..].to_vec()));
    }
    faults
}

/// Where this tool's own crate manifest sits, as a documented command has to
/// name it. Spelled once, because the law that checks the headers and the
/// headers themselves are the two things that must not drift apart.
pub const HARNESS_MANIFEST: &str = "tools/injection-harness/Cargo.toml";

/// The command a sweep manifest's own prose documents, if it documents one.
///
/// WHAT IS BEING PARSED IS A COMMAND SOMEBODY IS MEANT TO PASTE. Every tracked
/// manifest opens with one, wrapped across two prose lines with a trailing
/// backslash the way a shell wraps it, and until this round no program had ever
/// looked at one. They are the same words as the tool's real interface and they
/// decay against it in silence: the round that added verbs invalidated all
/// thirty-one, and the only reason none survived is that a person went through
/// them by hand.
///
/// THE SEPARATOR IS THE ANCHOR, not the position. Both wrappings in this
/// repository put `--` in a different place — before the line break in most,
/// after it in `tools/ci-plan/locked-resolution-sweep.json` — and what comes
/// after it is the tool's own argv wherever the break happened to fall.
///
/// AND THE SEPARATOR IS ALSO WHAT MAKES THIS A COMMAND RATHER THAN A SENTENCE:
/// prose talks about cargo, and `tools/stale-artifacts/injection-sweep.json`
/// says "two cargo runs" a few lines under its own header. A line that names
/// this tool and passes it nothing is somebody writing about it.
#[must_use]
pub fn documented_command(prose: &[String]) -> Option<DocumentedCommand> {
    // FOLDED FIRST, because a shell continuation is one command and the break is
    // a fact about the width of the file rather than about the invocation.
    let folded = prose.join("\n").replace("\\\n", " ");
    folded.lines().find_map(|line| {
        if !line.contains("cargo run") || !line.contains("injection-harness") {
            return None;
        }
        let words: Vec<&str> = line.split_whitespace().collect();
        let separator = words.iter().position(|word| *word == "--")?;
        Some(DocumentedCommand {
            cargo: words[..separator]
                .iter()
                .map(|w| (*w).to_string())
                .collect(),
            argv: words[separator + 1..]
                .iter()
                .map(|w| (*w).to_string())
                .collect(),
        })
    })
}

/// Why a recorded decision does not stand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unsound {
    /// The manifest names this injection AGAIN. The record is then in two
    /// states at once — retired as answering to nothing, and owing evidence —
    /// and yesterday's withdrawal must not stand in for today's proof.
    CameBack,
    /// The decision was recorded with nothing in it. The reason is the whole of
    /// a decision; without one the row was deleted, and a file was left behind
    /// that reads as though the question had been answered.
    NoReason,
}

/// Every decision this record carries that does not stand, by name.
///
/// PURE, AND BOTH READERS DRIVE IT — the same shape as [`stale_evidence`] above,
/// and here it is what keeps the law from being vacuous. [`forget`] refuses to
/// CREATE either of these, so over a repository where every decision was made
/// through the verb the law's walk finds nothing; a check that can only be
/// exercised by a corpus that does not exist yet is a check nobody has ever seen
/// answer. A fixture can ask this directly, and an injection into it reddens
/// that fixture whatever the tracked records happen to hold.
#[must_use]
pub fn unsound_decisions(record: &Firings, about: &Manifest) -> Vec<(String, Unsound)> {
    let mut faults = Vec::new();
    for (name, decision) in &record.forgotten {
        if about.injections.iter().any(|i| &i.name == name) {
            faults.push((name.clone(), Unsound::CameBack));
        }
        if decision.because.trim().is_empty() {
            faults.push((name.clone(), Unsound::NoReason));
        }
    }
    faults
}

/// Which of a manifest's injections a sweep is to run.
///
/// R1179 — PROVING ONE INJECTION USED TO COST THE WHOLE MANIFEST. There was no
/// way to name one, so the price of asking "does the injection I just wrote
/// actually reach the test I aimed it at" was every injection in the file: 52 of
/// them, twenty minutes, on a manifest whose author had just added four. R1178
/// paid that price once and still shipped a MISAIMED one — its
/// `every_declared_key_is_judged_over_its_own_workflow_history` injects its own
/// resolver, so the reader that injection broke is not one that test ever calls.
/// A verification that expensive is a verification that gets written down as an
/// intention.
///
/// AND THE SCOPE TRAVELS INTO THE REPORT, for the reason that report's own first
/// field records: a red set is a fact about a population, and a run that measured
/// four injections of fifty-two must not print the shape of one that measured all
/// of them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Scope {
    /// Every injection the manifest carries.
    EveryInjection { count: usize },
    /// Only these, named on the command line, out of that many in the manifest.
    Only { names: Vec<String>, of: usize },
}

/// The injections a sweep is to run, given what was named on the command line.
///
/// ORDER IS THE MANIFEST'S, NOT THE ARGUMENTS'. A sweep edits one tree in
/// sequence and its report is read beside the file; two runs naming the same
/// injections in different orders would be two orderings of one measurement.
///
/// A NAME THAT MATCHES NOTHING IS A REFUSAL. The alternative is a sweep that runs
/// zero injections, prints a clean report and exits 0 — which is the "0 means
/// suspect the injection" rule failing in the one place nobody would look, the
/// spelling of the thing being asked for.
pub fn select<'a>(
    injections: &'a [Injection],
    only: &[String],
) -> Result<(Vec<&'a Injection>, Scope), String> {
    if only.is_empty() {
        return Ok((
            injections.iter().collect(),
            Scope::EveryInjection {
                count: injections.len(),
            },
        ));
    }
    let mut unknown: Vec<&str> = Vec::new();
    for name in only {
        if !injections.iter().any(|it| &it.name == name) {
            unknown.push(name.as_str());
        }
    }
    if !unknown.is_empty() {
        return Err(format!(
            "no injection in this manifest is named {unknown:?} — it carries {} \
             ({}), and a sweep that ran none of them would print a clean report \
             about nothing",
            injections.len(),
            injections
                .iter()
                .map(|it| it.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    let chosen: Vec<&Injection> = injections
        .iter()
        .filter(|it| only.iter().any(|name| name == &it.name))
        .collect();
    let scope = Scope::Only {
        names: chosen.iter().map(|it| it.name.clone()).collect(),
        of: injections.len(),
    };
    Ok((chosen, scope))
}

/// AN UNKNOWN KEY IS A REFUSAL RATHER THAN A DEFAULT, which is why `_` is a
/// field and why `deny_unknown_fields` is on all three of these types. Every
/// optional field here is one a manifest may simply not carry, so a MISSPELLED
/// key would otherwise read as absent and the manifest would run under the
/// default while its author read their own spelling and believed it. Round 1136
/// paid for exactly that shape in another gate: a derived `Option` that serde
/// fills silently rebuilt the collapse the type had replaced.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// The manifest's own header — what this sweep is and how to run it, in the
    /// author's words. Read by people, and declared here so that a key nobody
    /// meant cannot hide beside it.
    #[serde(rename = "_", default)]
    pub prose: Vec<String>,
    /// The tree to edit and run in.
    pub repo: PathBuf,
    /// The suite, argv-style. Kept in the manifest rather than assumed, because
    /// a harness that hardcodes `cargo test` cannot be tested without running
    /// one.
    pub test_command: Vec<String>,
    /// Where the full logs go. One file per run, never truncated.
    pub logs: PathBuf,
    /// Refuse to start a run when less than this much memory is available.
    ///
    /// The standing rule on this machine is to re-check occupancy BEFORE EVERY
    /// BUILD, because other checkouts share the RAM and a measurement that runs
    /// the machine out of memory is not a measurement. Eight rounds running, the
    /// re-check happened at the start of a session and before the big sweeps and
    /// not before every build — which is what a person does and a program need
    /// not.
    #[serde(default)]
    pub min_free_mb: Option<u64>,
    /// What the `expect_red` lists claim to be. Absent means the weaker claim,
    /// which is what every sweep meant before the field existed.
    #[serde(default)]
    pub red_set: RedSet,
    /// Tests that read THIS MANIFEST'S ANCHORS against the tree, and are
    /// therefore red while an injection has replaced one of them.
    ///
    /// THE MECHANISM REDDENS THEM, NOT THE READ ANY INJECTION REMOVED. An
    /// anchor is exact text that must occur once; applying an injection
    /// replaces it, so from that moment this manifest no longer describes the
    /// tree it names, and anything that checks that says so. It is the law
    /// working rather than a guard nobody wrote — and it is the reason the
    /// harness's own manifest could not declare its red sets exhaustive: under
    /// that claim the artefact would fail the sweep it is an artefact of.
    ///
    /// NAMING THEM HERE RATHER THAN IN `expect_red` IS THE WHOLE POINT. An
    /// injection that named one would be claiming the read it removed is what
    /// reddened it, and that claim CANNOT FAIL — the mechanism satisfies it
    /// whatever the code does. [`unfalsifiable`] refuses exactly that, so the
    /// escape from an unnameable red is not a licence to launder a real
    /// expectation through it.
    ///
    /// AND THE EXCUSE IS CHECKED IN BOTH DIRECTIONS. Where an injection takes
    /// this manifest's anchors with it ([`unanchored`]), every name here MUST
    /// go red — a blanket excuse that has quietly stopped applying is the
    /// silent kind of decay this file exists to refuse. Where an injection
    /// leaves them standing, nothing here is excused and a red among these
    /// names is as unexpected as any other.
    #[serde(default)]
    pub tests_that_read_the_anchors: Vec<String>,
    pub injections: Vec<Injection>,
}

/// Read one manifest, with its two paths resolved AGAINST THE MANIFEST'S OWN
/// DIRECTORY.
///
/// THE BASE IS THE ONE A FILE ALWAYS KNOWS. `repo` and `logs` used to be
/// resolved against the process's working directory, which meant a sweep's
/// meaning depended on where somebody stood when they ran it — and where that
/// was lived in the manifest's `_` prose, in two different conventions, readable
/// by nobody but a person. R1108 needed a second reader (a law over every sweep
/// this repository tracks) and there was no answer in the files for it to use.
///
/// Resolving from the manifest's directory makes a sweep runnable from anywhere
/// and its tree derivable from the file, which is the same move `ci-plan` makes
/// for everything it reads. Lexical rather than `canonicalize`, which requires
/// the path to exist: the log directory is created afterwards, and a reader that
/// refused a manifest for naming a directory it is about to make would be a
/// reader about nothing.
pub fn read_manifest(path: &Path) -> Result<Manifest, String> {
    let raw =
        fs::read_to_string(path).map_err(|e| format!("{} unreadable: {e}", path.display()))?;
    let mut manifest: Manifest = serde_json::from_str(&raw)
        .map_err(|e| format!("{} is not a manifest: {e}", path.display()))?;
    let beside = path.parent().unwrap_or(Path::new("."));
    manifest.repo = absolute(&beside.join(&manifest.repo))?;
    manifest.logs = absolute(&beside.join(&manifest.logs))?;
    // AND AN EXHAUSTIVE CLAIM OVER AN EMPTY LIST IS A CONTRADICTION, refused
    // here rather than after the suites. Under the weaker claim an empty
    // `expect_red` is honest — "say what went red and judge nothing" is what an
    // exploratory sweep does — but under this one it says the injection reddens
    // NOTHING, which is the single outcome a sweep exists to DETECT rather than
    // to declare. Refusing at read time is the same trade the dry-run pre-flight
    // makes: this repository's whole-workspace sweep is 85 minutes of suites,
    // and a claim that cannot be true should not cost them.
    if manifest.red_set == RedSet::Exhaustive {
        let silent: Vec<&str> = manifest
            .injections
            .iter()
            .filter(|injection| injection.expect_red.is_empty())
            .map(|injection| injection.name.as_str())
            .collect();
        if !silent.is_empty() {
            return Err(format!(
                "{} claims its red sets are exhaustive, and {} injection(s) name \
                 no red at all: {silent:?}. Under that claim an empty list is the \
                 assertion that breaking the read changes nothing",
                path.display(),
                silent.len()
            ));
        }
    }
    Ok(manifest)
}

/// A path made absolute lexically, against the process's working directory.
fn absolute(path: &Path) -> Result<PathBuf, String> {
    std::path::absolute(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// Every tracked `.json` in a repository, from `git ls-files`.
///
/// FROM GIT AND NOT FROM A WALK, for the reason `ci-plan` reads workflows that
/// way: a manifest that is not tracked is one nobody else can run, and one that
/// is tracked is one a reader will believe. A list of sweeps kept beside a law
/// would go stale the first time a crate gained one, in the direction that
/// reads as a pass.
///
/// HERE RATHER THAN IN A TEST because it is now asked by more than one crate —
/// `tests/sweeps.rs` asks it of every law it holds, and `tools/scratch-budget`
/// asks it to find out which log directories this repository writes into. Two
/// spellings of "the sweeps this repository tracks" is two populations that can
/// disagree, and they would disagree silently.
pub fn tracked_json(root: &Path) -> Result<Vec<String>, String> {
    let out = std::process::Command::new("git")
        .args(["ls-files", "*.json"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("git ls-files in {}: {e}", root.display()))?;
    if !out.status.success() {
        return Err(format!(
            "git ls-files in {} failed: {}",
            root.display(),
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let mut files: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_string)
        .collect();
    files.sort();
    Ok(files)
}

/// Whether a manifest is an INPUT TO A TEST rather than a sweep somebody runs.
///
/// Declared ONCE for every law that has a population, because two laws
/// disagreeing about which files they are about is the same defect as a law
/// over no files at all. What the rule cost to arrive at is written where the
/// laws use it (`tests/sweeps.rs`): the classifier is where the file sits,
/// because the obvious alternative — does the tree it names hold the files it
/// edits — is the very question those laws ask.
pub fn a_test_input(path: &str) -> bool {
    path.split('/').any(|part| part == "tests")
}

/// The sweeps a repository TRACKS AND RUNS: every tracked `.json` that reads as
/// a manifest and is not an input to a test, with its path as `git` spells it.
///
/// The one answer to "which sweeps are this repository's", so that a law about
/// their anchors, a law about their commands and a law about the directories
/// they write into cannot each be about a different set.
pub fn tracked_sweeps(root: &Path) -> Result<Vec<(String, Manifest)>, String> {
    let mut sweeps = Vec::new();
    for path in tracked_json(root)? {
        if a_test_input(&path) {
            continue;
        }
        if let Ok(manifest) = read_manifest(&root.join(&path)) {
            sweeps.push((path, manifest));
        }
    }
    Ok(sweeps)
}

/// One replacement, as the law the dry run and the write BOTH obey: the anchor
/// occurs EXACTLY once, and what comes back is the text with that one occurrence
/// replaced.
///
/// A replacement that matched nothing produces a run whose silence reads as "the
/// injection did not fire", and one that matched twice produces a change nobody
/// described. Two places decide that — the pre-flight, which refuses before any
/// run, and the write, which is what actually edits the tree — and a law only
/// one of them enforced is the shape where the dry run accepts an edit the write
/// refuses, or worse, the reverse.
pub fn replace_once(text: &str, edit: &Edit) -> Result<String, String> {
    let hits = text.matches(&edit.from).count();
    if hits != 1 {
        return Err(format!(
            "{} : the text to replace occurs {hits} times, not once",
            edit.file
        ));
    }
    Ok(text.replacen(&edit.from, &edit.to, 1))
}

/// Every file any injection touches, as bytes — and, in the same pass, a DRY RUN
/// of every injection.
///
/// ONE FUNCTION AND NOT TWO, so that the bytes checked are the bytes `apply`
/// will edit by construction rather than by argument. The pre-flight exists
/// because the only other place an anchor is checked is `apply`, which runs
/// after the control and after every injection before it: a typo in the ninth of
/// nine costs the control plus eight whole-suite runs, on a machine where one of
/// those is tens of minutes, and the sweep then ends having measured nothing and
/// having edited the tree eight times.
///
/// It is a DRY RUN and not a count against the pristine bytes: an injection's
/// second edit may legitimately rewrite what its first one wrote, and a gate
/// that refused that would be refusing for a reason outside its own law.
///
/// TAKING NO SUITE AND RUNNING NOTHING is what lets a test call it over every
/// sweep this repository tracks — the anchors are text, and whether they still
/// apply is a question about text.
///
/// IT REPORTS EVERY INJECTION THAT WILL NOT APPLY, not the first, and that is a
/// measured correction rather than a preference. R1205 restructured the workspace
/// lister and left SIX dead anchors across two manifests; this function returned
/// at the first one, so the law above it printed "2 of the sweeps this repository
/// tracks no longer apply" — one per manifest — and a reader who fixed those two
/// would have re-run to find two more, twice. Finishing costs nothing here: no
/// suite runs, no tree is edited, and the work is `str::matches` over files
/// already in memory. A count that is smaller than the truth is the shape this
/// repository has paid for before, and `--no-fail-fast` is on its suites for it.
///
/// An injection that fails is abandoned at its failing edit rather than carried
/// on with: its later edits may legitimately depend on what the failed one would
/// have written, so judging them against a tree that never got it would report
/// problems the sweep does not have.
///
/// The problems are joined with newlines, one per line, each naming its
/// injection — so a caller that wants them counted can split them, and a caller
/// that only prints gets them all.
pub fn snapshot_and_dry_run(
    repo: &Path,
    injections: &[Injection],
) -> Result<BTreeMap<PathBuf, Vec<u8>>, String> {
    let mut snapshot: BTreeMap<PathBuf, Vec<u8>> = BTreeMap::new();
    let mut problems: Vec<String> = Vec::new();
    for injection in injections {
        let mut from_disk = |path: &Path| -> Result<String, String> {
            if !snapshot.contains_key(path) {
                let bytes =
                    fs::read(path).map_err(|e| format!("{} unreadable: {e}", path.display()))?;
                snapshot.insert(path.to_path_buf(), bytes);
            }
            String::from_utf8(snapshot[path].clone()).map_err(|_| {
                format!(
                    "{} is not text, so no replacement in it can be described",
                    path.display()
                )
            })
        };
        if let Err(problem) = stage(repo, injection, &mut from_disk) {
            problems.push(format!("{}: {problem}", injection.name));
        }
    }
    if problems.is_empty() {
        Ok(snapshot)
    } else {
        Err(problems.join("\n"))
    }
}

/// One injection applied to text rather than to disk — the files it rewrites,
/// with its edits chained in the order the sweep's own `apply` does them.
///
/// THE READER IS AN ARGUMENT BECAUSE THE TREE IS THE QUESTION. Handed the files
/// as they sit, this is the dry run above: does this injection still apply.
/// Handed a tree that ALREADY HAS another injection in it, the same code answers
/// the question [`unanchored`] asks — does this manifest still describe its tree
/// while that one is applied. Two spellings of "apply an injection to text"
/// would be two answers free to disagree, and the second is the one the sweep
/// judges an unnameable red by.
fn stage(
    repo: &Path,
    injection: &Injection,
    read: &mut impl FnMut(&Path) -> Result<String, String>,
) -> Result<BTreeMap<PathBuf, String>, String> {
    let mut staged: BTreeMap<PathBuf, String> = BTreeMap::new();
    for edit in &injection.edits {
        let path = repo.join(&edit.file);
        let text = match staged.remove(&path) {
            Some(edited) => edited,
            None => read(&path)?,
        };
        staged.insert(path, replace_once(&text, edit)?);
    }
    Ok(staged)
}

/// The injections under which THIS MANIFEST NO LONGER DESCRIBES ITS TREE.
///
/// An anchor is exact text that must occur exactly once, and applying an
/// injection replaces it: from that moment the manifest names text its tree does
/// not hold, and every reader of those anchors — this crate's own law over every
/// tracked sweep, most of all — is red for that reason and not for the read the
/// injection removed.
///
/// IT IS NOT EVERY INJECTION, WHICH IS WHY THIS IS COMPUTED RATHER THAN
/// DECLARED. An injection whose `to` CONTAINS its `from` leaves the anchor
/// standing, and this repository has two of them: one that prepends a line, and
/// one that appends a comment to a line held inside another sweep's manifest —
/// where the anchor survives and the OTHER sweep's comes loose, which is that
/// injection's whole subject. Measured before this function existed, in the
/// firing record: 19 of the 20 rows carried the anchor law's red and the
/// twentieth was the injection that prepends.
///
/// AGAINST THE TREE AS IT STANDS, so a manifest with an anchor ALREADY loose
/// says nothing here: that failure is in the baseline, the law is red in the
/// control too, and a red the control also had is not this injection's. The same
/// subtraction the sweep makes over its suites, made over text.
/// AND IT IS A `Result` BECAUSE THE EMPTY SET IS AN ANSWER. A tree whose files
/// this cannot read excuses nothing, and returning "no injection unanchors
/// anything" for it would hand every reader the quiet half of the verdict: the
/// sweep would judge an unnameable red as unexpected, and the law below would
/// find no unfalsifiable expectation in a manifest it never opened.
pub fn unanchored(repo: &Path, injections: &[Injection]) -> Result<BTreeSet<String>, String> {
    let snapshot = read_touched(repo, injections)?;
    let baseline = which_apply(repo, injections, &snapshot);
    let mut lost = BTreeSet::new();
    for applied in injections {
        if !baseline.contains(&applied.name) {
            continue;
        }
        let mut read = |path: &Path| -> Result<String, String> {
            snapshot.get(path).cloned().ok_or_else(|| {
                format!(
                    "{} is not one of the files this sweep edits",
                    path.display()
                )
            })
        };
        let Ok(staged) = stage(repo, applied, &mut read) else {
            continue;
        };
        let mut under = snapshot.clone();
        under.extend(staged);
        if which_apply(repo, injections, &under) != baseline {
            lost.insert(applied.name.clone());
        }
    }
    Ok(lost)
}

/// Every file any injection touches, as text — and nothing said about whether
/// the injections apply, which is [`which_apply`]'s question.
fn read_touched(
    repo: &Path,
    injections: &[Injection],
) -> Result<BTreeMap<PathBuf, String>, String> {
    let mut tree = BTreeMap::new();
    for injection in injections {
        for edit in &injection.edits {
            let path = repo.join(&edit.file);
            if tree.contains_key(&path) {
                continue;
            }
            let bytes =
                fs::read(&path).map_err(|e| format!("{} unreadable: {e}", path.display()))?;
            let text =
                String::from_utf8(bytes).map_err(|_| format!("{} is not text", path.display()))?;
            tree.insert(path, text);
        }
    }
    Ok(tree)
}

/// Which of these injections still apply to a tree given as text.
fn which_apply(
    repo: &Path,
    injections: &[Injection],
    tree: &BTreeMap<PathBuf, String>,
) -> BTreeSet<String> {
    injections
        .iter()
        .filter(|injection| {
            let mut read = |path: &Path| -> Result<String, String> {
                tree.get(path)
                    .cloned()
                    .ok_or_else(|| format!("{} was not read", path.display()))
            };
            stage(repo, injection, &mut read).is_ok()
        })
        .map(|injection| injection.name.clone())
        .collect()
}

/// Expectations a manifest names that the MECHANISM ALREADY GUARANTEES, one
/// line each.
///
/// A test that reads this manifest's anchors is red whenever an injection takes
/// them with it ([`unanchored`]), whatever the code under it does. An
/// `expect_red` naming that test from such an injection therefore cannot fail:
/// delete the property it is aimed at and the sweep still scores it reached. It
/// is the vacuous expectation this whole device exists to detect, arrived at
/// from the other side — and the escape hatch
/// [`Manifest::tests_that_read_the_anchors`] would be the way to build one by
/// accident, which is why the hatch and this refusal landed together.
///
/// HERE AND NOT IN THE SWEEP, so the same rule the sweep refuses to start on is
/// the rule this crate's suite holds over every tracked manifest. A sweep is
/// tens of minutes; the suite is where a reader finds out.
pub fn unfalsifiable(repo: &Path, manifest: &Manifest) -> Result<Vec<String>, String> {
    if manifest.tests_that_read_the_anchors.is_empty() {
        return Ok(Vec::new());
    }
    let lost = unanchored(repo, &manifest.injections)?;
    let mut found = Vec::new();
    for injection in &manifest.injections {
        if !lost.contains(&injection.name) {
            continue;
        }
        for expected in &injection.expect_red {
            if manifest
                .tests_that_read_the_anchors
                .iter()
                .any(|reader| answers_to(expected, reader) || answers_to(reader, expected))
            {
                found.push(format!(
                    "{}: `{expected}` reads this manifest's anchors, and this \
                     injection takes them with it — so the red is the \
                     mechanism's and this expectation cannot fail",
                    injection.name
                ));
            }
        }
    }
    Ok(found)
}

// --- what a plan's name answers to -------------------------------------------

/// Whether one expected name is among the tests that went red.
///
/// A harness prints a test by the path its target reaches it through —
/// `read_agreement_population::the_walk` for a `#[test]` inside a module — and
/// the person writing a plan writes down the name they gave the function. Held
/// against each other as plain strings, a sweep in which every injection landed
/// exactly where it was aimed comes back "aimed at X and did not reach it" for
/// all of it. That happened, on six injections and forty minutes of suite runs,
/// and the message it prints is the strongest one this tool has: a misaimed
/// injection. A refusal a gate makes for a reason outside its own law is the
/// same defect as a gate that does not fire.
///
/// So a name matches its own suffix at a MODULE BOUNDARY, and nowhere else:
/// `a::b::name` answers to `name` and to `b::name`, and `other_name` does not
/// answer to `name`. Not a substring test — that would let `judges` match
/// `the_walk_judges_nothing` and quietly credit an injection with a red it did
/// not cause.
///
/// HERE RATHER THAN IN `main.rs`, for the reason at the top of this file. R1183
/// gave the rule a SECOND READER — a law over every sweep this repository
/// tracks, asking whether each `expect_red` still names a test that exists — and
/// a second reader of a rule that lives inside a binary has no choice but to
/// spell the rule again. Two spellings of "what a plan's name answers to" is the
/// one way this device can call an injection misaimed for a reason of its own.
pub fn reached(fired: &BTreeSet<String>, expected: &str) -> bool {
    fired.iter().any(|red| answers_to(red, expected))
}

/// The same rule for ONE name, which is what the other directions need: a red
/// that answers to no expectation is one the manifest never described, and an
/// expectation no test answers to is one aimed at nothing.
pub fn answers_to(red: &str, expected: &str) -> bool {
    red == expected
        || red
            .strip_suffix(expected)
            .is_some_and(|prefix| prefix.ends_with("::"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn injection(from: &str, expect_red: &[&str]) -> Injection {
        Injection {
            name: "an-injection".to_string(),
            why: String::new(),
            edits: vec![Edit {
                file: "src/lib.rs".to_string(),
                from: from.to_string(),
                to: "broken".to_string(),
            }],
            expect_red: expect_red.iter().map(|n| (*n).to_string()).collect(),
        }
    }

    /// R1198 — the record is about a DEFINITION, so editing the injection must
    /// strand the evidence rather than carry it forward.
    #[test]
    fn an_edited_injection_is_not_covered_by_the_old_evidence() {
        let proven = injection("the original text", &["a_law"]);
        let row = Firing {
            edits: proven.edits.clone(),
            expect_red: proven.expect_red.clone(),
            tests: vec!["a_law".to_string()],
        };
        assert!(
            records_the_same_injection(&row, &proven),
            "the row proves the injection it was measured against"
        );
        assert!(
            !records_the_same_injection(&row, &injection("some other text", &["a_law"])),
            "an injection whose EDITS have changed breaks something else now, and \
             yesterday's firing is evidence about what it used to break"
        );
        assert!(
            !records_the_same_injection(&row, &injection("the original text", &["a_law", "more"])),
            "and one that now CLAIMS MORE has evidence for the smaller claim only"
        );
    }

    /// R1198 — a corrupted record and a sweep nobody has run are different
    /// answers, and only one of them is honest.
    #[test]
    fn a_record_that_will_not_parse_is_not_an_absent_one() {
        assert!(
            read_firings(Path::new("no-such-record.firings.json"))
                .expect("an absent record is not an error")
                .is_none(),
            "a sweep nobody has run yet simply has no record"
        );
        // A REAL FILE THAT IS NOT ONE, and the crate's own manifest is the
        // nearest thing this test can be sure exists and is sure is not a
        // firing record.
        let why = read_firings(Path::new("Cargo.toml"))
            .expect_err("a file that exists and is not a record is not an absence");
        assert!(
            why.contains("not a firing record"),
            "and it says which of the two it is: {why}"
        );
    }

    /// R1198 — the refutation this repository's own tree supplied on the day the
    /// record was designed.
    #[test]
    fn a_record_belongs_to_one_sweep_and_not_to_a_directory() {
        let one = firings_path(Path::new("tools/ci-plan/injection-sweep.json"));
        let other = firings_path(Path::new("tools/ci-plan/locked-resolution-sweep.json"));
        assert_eq!(
            one.parent(),
            other.parent(),
            "both sweeps live in the same directory, which is the case that \
             matters"
        );
        assert_ne!(
            one, other,
            "and their records are different files — one per directory made the \
             second sweep's every injection read as unproven while the first \
             sweep's evidence read as answering to nothing"
        );
        assert!(
            one.to_string_lossy().ends_with(FIRINGS_SUFFIX),
            "the record is named from the manifest it is about: {one:?}"
        );
    }

    #[test]
    fn a_test_answers_to_its_own_name_and_not_to_a_name_it_merely_contains() {
        // THE REFUSAL THIS TOOL MADE FOR A REASON OUTSIDE ITS OWN LAW. A plan
        // names the `#[test]` function; a harness prints the path its target
        // reaches it through. Compared as plain strings, six injections that
        // each landed exactly where they were aimed came back as six misaimed
        // injections — the loudest verdict this tool has, spent on nothing.
        let fired = BTreeSet::from([
            "read_agreement_population::the_walk".to_string(),
            "plain".to_string(),
        ]);
        assert!(
            reached(&fired, "the_walk"),
            "the name a plan is written with is the name the function has"
        );
        assert!(
            reached(&fired, "read_agreement_population::the_walk"),
            "and the path the harness prints is still itself"
        );
        assert!(reached(&fired, "plain"), "an unqualified red is unchanged");

        // AND THE OTHER DIRECTION, which is why this is a suffix at a module
        // boundary rather than a substring: crediting an injection with a red
        // it did not cause is how a sweep says a contract is alive when the
        // thing that went red was its neighbour.
        assert!(
            !reached(&fired, "walk"),
            "`the_walk` is not the test called `walk` — a substring match would \
             credit this injection with somebody else's failure"
        );
        assert!(
            !reached(&fired, "population::the_walk"),
            "half a module segment is not a module path"
        );
        assert!(!reached(&fired, "the_walk_that_is_not_this_one"));
    }
}
