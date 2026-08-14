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

/// Does this row record the injection as it is written today?
#[must_use]
pub fn records_the_same_injection(row: &Firing, injection: &Injection) -> bool {
    row.edits == injection.edits && row.expect_red == injection.expect_red
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
pub fn snapshot_and_dry_run(
    repo: &Path,
    injections: &[Injection],
) -> Result<BTreeMap<PathBuf, Vec<u8>>, String> {
    let mut snapshot: BTreeMap<PathBuf, Vec<u8>> = BTreeMap::new();
    for injection in injections {
        let mut staged: BTreeMap<PathBuf, String> = BTreeMap::new();
        for edit in &injection.edits {
            let path = repo.join(&edit.file);
            if !snapshot.contains_key(&path) {
                let bytes =
                    fs::read(&path).map_err(|e| format!("{} unreadable: {e}", path.display()))?;
                snapshot.insert(path.clone(), bytes);
            }
            let text = match staged.remove(&path) {
                Some(edited) => edited,
                None => String::from_utf8(snapshot[&path].clone()).map_err(|_| {
                    format!(
                        "{} is not text, so no replacement in it can be described",
                        path.display()
                    )
                })?,
            };
            let edited = replace_once(&text, edit)
                .map_err(|problem| format!("{}: {problem}", injection.name))?;
            staged.insert(path, edited);
        }
    }
    Ok(snapshot)
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
