//! THE SWEEP, AND THE TARGET THAT OWNS IT (Round 1071, owned here in R1074).
//!
//! Three laws ask the SAME panel about the SAME derived population, and each
//! built it for itself: measured at 220.53s + 191.08s + 139.94s in one suite
//! run — 9m12s, and the dominant term in a CI job sitting at 26m45s of a
//! 30-minute budget. The population is ONE substrate and the three laws are
//! its consumers, which is why they live in one test binary: separate binaries
//! are separate processes and cannot share anything but the disk.
//!
//! IT HOLDS EACH TRIAL'S ANSWERS AS THE BYTES THE READ PRINTED, and hands a
//! consumer a parsed copy for the length of its own iteration. 312 parsed
//! panels do not fit in memory — one panel is most of a megabyte of JSON and
//! the parsed form is several times that — while the bytes do, and parsing
//! them three times is nothing beside the ~64,000 process launches this
//! removes. The held size is REPORTED rather than assumed, so a population
//! that outgrows the choice says so instead of being killed for it.
//!
//! WHY THIS IS NOT IN `tests/common/` ANY MORE. Round 1071 left the sweep in
//! the module every test binary shares, and what stood between a second copy
//! of that cost and the tree was a sentence in a doc comment. The saving is
//! per-process, so a second binary calling `sweep()` does not share anything —
//! it pays the whole 212s again, and nothing says so: the suite simply takes
//! twice as long, which is exactly the shape of the defect Round 1071 was
//! built to remove. So the sweep names the target it belongs to and the
//! COMPILER enforces it ([`owned_by`]); a law that wants the population joins
//! this binary rather than starting a second one.
//!
//! THE DERIVATION IS NOT THIS, and the distinction is measured rather than
//! assumed. [`crate::common::corruptions`] only describes the edits, and
//! Round 1074 measured what describing them costs: `authorable_population` —
//! the population's own oracle, and rightly its own binary since it judges the
//! DEFINITION rather than the surface — calls it five times and the whole
//! binary runs in 4.12s. What is expensive is APPLYING 312 edits and asking
//! every advertised read about each, and that is what lives here. So the gate
//! below is over the sweep, not over the derivation, which is not where
//! Round 1071's carry put it.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use mnemosyne_atomic::AtomicStore;

use crate::common;
use common::{Corruption, Read, SIDECAR};

// ==========================================================================
// THE OWNER (Round 1074).
// ==========================================================================

/// The test target entitled to build the population.
///
/// Cargo compiles each `tests/*.rs` into its own binary and sets
/// `CARGO_CRATE_NAME` to that target's name, so the crate this module is being
/// compiled INTO is a compile-time fact — which is what makes the ownership
/// check a build error rather than a convention.
pub const OWNER: &str = "authoring_surface";

/// Whether `crate_name` is the target this module belongs to.
///
/// `const fn` because the answer is needed at compile time; a plain `==` on
/// `&str` is not const, so the comparison is spelled over the bytes.
pub const fn owned_by(crate_name: &str) -> bool {
    let (name, owner) = (crate_name.as_bytes(), OWNER.as_bytes());
    if name.len() != owner.len() {
        return false;
    }
    let mut at = 0;
    while at < name.len() {
        if name[at] != owner[at] {
            return false;
        }
        at += 1;
    }
    true
}

const _: () = assert!(
    owned_by(env!("CARGO_CRATE_NAME")),
    "the corruption sweep belongs to the `authoring_surface` test target. \
     Including it from another test file does not share the sweep — separate \
     integration-test files are separate BINARIES and the memo is a OnceLock \
     per PROCESS, so the second one builds all 312 stores and asks every \
     advertised read about each of them again, for the whole of the sweep's \
     cost a second time (212s when Round 1071 measured it). Put the new law \
     in tests/surface/ and `mod` it from tests/authoring_surface.rs, where it \
     reads the sweep this binary already paid for."
);

/// The gate's refusing arm, which the compile-time assertion above can never
/// demonstrate: a `const` assertion that holds proves only its true side, and
/// a predicate that answered `true` for every name would satisfy it just as
/// well. So the false side is asked here, and the true side is asked of the
/// REAL crate name rather than of a literal — if this file is ever moved into
/// a differently-named target, that is the assertion that says so instead of
/// the gate quietly guarding nothing.
#[test]
fn the_sweep_names_the_target_that_may_build_it() {
    assert!(
        owned_by(env!("CARGO_CRATE_NAME")),
        "the sweep is compiled into `{}`, not into `{OWNER}`",
        env!("CARGO_CRATE_NAME")
    );
    for foreign in [
        "authorable_population",
        "coordinate_read_answers",
        "",
        // A prefix and a suffix of the owner's name: a check that compared
        // only the start would let the first through, and one that compared
        // only the length would let neither be told apart from the owner.
        "authoring_surfac",
        "authoring_surface_two",
    ] {
        assert!(
            !owned_by(foreign),
            "`{foreign}` is not the owner, and a gate that admits it admits \
             every second sweep"
        );
    }
}

// ==========================================================================
// WHAT A READ SAID
// ==========================================================================

/// One read's answer about one store. A verb that takes `--json` and answers in
/// prose anyway can still be compared for difference, but it holds no list, so
/// it can never be asked whether it STARTED SAYING SOMETHING — a limit of the
/// panel that is named and printed rather than absorbed.
pub enum Answer {
    Json(serde_json::Value),
    Prose(String),
}

impl Answer {
    pub fn read(stdout: Vec<u8>) -> Self {
        let text = String::from_utf8(stdout).expect("cli output is utf-8");
        match serde_json::from_str(&text) {
            Ok(json) => Answer::Json(json),
            Err(_) => Answer::Prose(text),
        }
    }
}

/// What every advertised read said about one store, keyed by [`Read::label`] —
/// the read AND the question, since one verb can be asked several.
pub struct Panelled {
    pub failed: Vec<String>,
    pub answers: BTreeMap<String, Answer>,
}

pub fn ask_panel(ws: &Path, panel: &[Read]) -> Panelled {
    let mut failed = Vec::new();
    let mut answers = BTreeMap::new();
    for read in panel {
        let out = common::run(ws, &read.argv());
        if out.status.success() {
            answers.insert(read.label(), Answer::read(out.stdout));
        } else {
            failed.push(read.label());
        }
    }
    Panelled { failed, answers }
}

// ==========================================================================
// THE SWEEP ITSELF
// ==========================================================================

/// What every advertised read printed about one corrupted store, unparsed.
pub struct Seen {
    /// Reads that exited non-zero — a gate rejecting this edit.
    pub failed: Vec<String>,
    answers: BTreeMap<String, Vec<u8>>,
    sidecar: Vec<u8>,
}

impl Seen {
    /// The panel's answers, parsed. Built per consumer per iteration and
    /// dropped with it: this is the allocation the sweep refuses to keep.
    pub fn parsed(&self) -> Panelled {
        Panelled {
            failed: self.failed.clone(),
            answers: self
                .answers
                .iter()
                .map(|(label, bytes)| (label.clone(), Answer::read(bytes.clone())))
                .collect(),
        }
    }

    /// The store the corrupted manifests imported to — the corruption's own
    /// echo, before any read had an opinion about it.
    pub fn sidecar(&self) -> serde_json::Value {
        serde_json::from_slice(&self.sidecar).expect("the sidecar the import wrote is JSON")
    }

    fn bytes(&self) -> usize {
        self.sidecar.len() + self.answers.values().map(Vec::len).sum::<usize>()
    }
}

/// One corruption, applied and asked.
pub struct Trial {
    pub corruption: Corruption,
    /// `Err` = the write path REFUSED the edit, with its reason. Not a failure
    /// of the sweep: "the class cannot be authored" is a verdict about the
    /// corruption, and each law counts it in its own terms.
    pub seen: Result<Seen, String>,
}

/// The corpus, the panel, and every trial — built once per test binary.
///
/// NOTHING HERE IS CARRIED FOR A CALLER THAT MIGHT WANT IT. Moving this struct
/// out of `tests/common/` took it out from under that module's blanket
/// `#![allow(dead_code)]` — which is right where it is, since each test binary
/// uses a different part of the shared module — and the first compile said
/// `manifests` and `held_bytes` were never read. They were the same dead data
/// Round 1062 found in the injection harness's `exit_code`: recorded, carried
/// for the process, consulted by nobody. The manifests are used to BUILD the
/// trials and the held size is printed by [`build_sweep`] itself, so neither
/// needed a field; a law that comes to want one adds it back in the round that
/// reads it.
pub struct Sweep {
    pub store: AtomicStore,
    pub telling: String,
    pub panel: Vec<Read>,
    /// The advertised reads this corpus cannot ask, with their refusals.
    pub unaskable: Vec<(String, String)>,
    pub baseline: Panelled,
    pub baseline_sidecar: serde_json::Value,
    pub trials: Vec<Trial>,
}

/// THE sweep, memoized for the process. The `#[test]`s in this binary share
/// it; the first to ask pays for it and the others read it.
pub fn sweep() -> &'static Sweep {
    static SWEEP: std::sync::OnceLock<Sweep> = std::sync::OnceLock::new();
    SWEEP.get_or_init(build_sweep)
}

fn build_sweep() -> Sweep {
    let manifests = common::dnd_quest_manifests();
    let ws = common::workspace_try(&manifests, Some(&common::audit_dir()))
        .expect("the authored corpus must load");
    let store = AtomicStore::load(&ws.path().join(SIDECAR)).expect("the imported store loads");
    let telling = common::telling_of(&store);
    let (panel, unaskable) = common::panel(ws.path(), &telling);
    let baseline = ask_panel(ws.path(), &panel);
    assert!(
        baseline.failed.is_empty(),
        "the panel is exactly the reads that answered at baseline: {:?}",
        baseline.failed
    );
    let baseline_sidecar = common::read_sidecar(ws.path());

    let population = common::corruptions(&store, &manifests);
    assert!(
        population.len() >= 30,
        "the derived population collapsed to {} — a walk that finds almost \
         nothing reads exactly like a store with almost no surface",
        population.len()
    );
    let mut trials = Vec::with_capacity(population.len());
    let mut held_bytes = 0usize;
    for corruption in population {
        let seen = match common::workspace_try(
            &corruption.applied(&manifests),
            Some(&common::audit_dir()),
        ) {
            Err(refusal) => Err(refusal),
            Ok(mutated) => {
                // The gates compare the BASELINE's prose against this store's
                // roads, so the fixture rides in unchanged (Round 1072).
                common::carry_projections(ws.path(), mutated.path(), &store);
                let mut answers = BTreeMap::new();
                let mut failed = Vec::new();
                for read in &panel {
                    let out = common::run(mutated.path(), &read.argv());
                    if out.status.success() {
                        answers.insert(read.label(), out.stdout);
                    } else {
                        failed.push(read.label());
                    }
                }
                let seen = Seen {
                    failed,
                    answers,
                    sidecar: fs::read(mutated.path().join(SIDECAR))
                        .expect("the import wrote a sidecar"),
                };
                held_bytes += seen.bytes();
                Ok(seen)
            }
        };
        trials.push(Trial { corruption, seen });
    }

    // THE COST OF THE CHOICE, PRINTED. Holding the answers is what lets the
    // laws share one sweep, and a population that outgrows the machine should
    // say so here rather than be discovered as an OOM.
    println!(
        "sweep: {} trial(s), {} refused by the write path, {:.1} MiB of answer \
         bytes held for the process",
        trials.len(),
        trials.iter().filter(|t| t.seen.is_err()).count(),
        held_bytes as f64 / (1024.0 * 1024.0)
    );

    Sweep {
        store,
        telling,
        panel,
        unaskable,
        baseline,
        baseline_sidecar,
        trials,
    }
}
