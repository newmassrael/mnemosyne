//! The scale-floor experiment's graded floor, re-asked by today's gates
//! (Round 1180).
//!
//! WHY. Round 473 designed, and June 2026 executed, the one measurement this
//! track has of whether the substrate holds up at a novel's scale: two blind
//! extractors each read a ~100 KB story and built a store from it, and the
//! deterministic floor came out `A = 3, B = 5` on a defect endpoint the
//! protocol manifest defined BEFORE any prose existed. Round 1176 carried
//! those two stores into the tracked population, so the walks can now open
//! them. This round asks the next question, which is the one the score is for:
//! **do today's gates still say what the graded floor said?**
//!
//! That question has to be a law rather than a session's measurement, because
//! the score is the thing that decays silently. The gates have moved through
//! five hundred rounds since June; a verb that quietly stopped counting an axis
//! would leave every store greener than it is, and the only evidence that would
//! notice is a corpus with a published, independently-graded answer. There is
//! exactly one such corpus, and this is it.
//!
//! WHAT IS PINNED, and where each number comes from:
//!
//!   * the AXES from `scale-floor-manifest.json`, whose sha256 was pinned in
//!     the R473 ledger entry before execution — so the mapping from a D-class
//!     to today's vocabulary is checked against the protocol, not asserted
//!     against itself (`the_graded_axes_are_the_ones_this_law_asks_for`);
//!   * the SCORE from `evidence/defect-tables.md`, the blind grader's own
//!     table (`D1 0/0 · D2 0/0 · D3 3/5 · D4 0/0`, primary endpoint 3 vs 5).
//!
//! The score is written HERE rather than parsed out of that document on
//! purpose: the document is not sealed by the kit record (`replay.json`
//! declares the two stores, and `declare-run-tree` walks only the run tree), so
//! a law that read its expectations out of it would move whenever the document
//! moved. Frozen in code, the two disagree loudly if either side is edited.
//!
//! TWO THINGS ARE NOT THE SAME, and both are said rather than hidden.
//!
//! First, today's `validate-continuity` finds one thing on arm B that the
//! graded floor's four classes do not name: an `evidence_unreachable` fact.
//! That check is Round 522, built five months after this extraction was graded,
//! so the floor could not have counted it. The endpoint is class-scoped, so the
//! score is untouched — but a gate that says MORE than the grader did is
//! exactly what this law exists to notice, and it is pinned as its own number.
//! It is also the guard against the subtler failure: the violation map here is
//! EXHAUSTIVE rather than filtered, so a graded kind that the engine renamed
//! would surface as an ungraded one instead of silently counting zero.
//!
//! Second, D4's zero does not come from the continuity gate at all. Round 433
//! enforces the fork-boundary invariant AT THE WRITE PATH and re-checks it in
//! the scan; measured here, all four primitives that can write
//! `supersedes_in_frame` refuse the fault outright, so the scan is never
//! handed one. `no_write_path_will_store_a_fault_the_d4_axis_counts` is
//! therefore the honest form of that axis's law.

use std::collections::{BTreeMap, BTreeSet};

use crate::common;
use common::{corpus_fact_manifest, corpus_workspace_try, json_report, repo_root, run};

/// The two arms, by the name every walk prints.
const ARMS: [&str; 2] = [
    "claudedocs/phase1-scale-floor-experiment/run/store-A",
    "claudedocs/phase1-scale-floor-experiment/run/store-B",
];

/// D1 — "gated rule violations", in the violation vocabulary today emits.
const D1_KINDS: [&str; 2] = ["rule_transition_invalid", "rule_exclusive_overlap"];

/// D4 — "fork-boundary faults". The manifest names two findings, and both are
/// asked for by name here even though neither can be reached through a write
/// path today (see `no_write_path_will_store_a_fault_the_d4_axis_counts`): a
/// kind this list gets WRONG cannot hide, because any violation kind outside
/// these lists lands in `ungraded_violations`, which is asserted exhaustively.
const D4_KINDS: [&str; 2] = ["succession_cross_branch", "succession_cycle"];

/// The protocol manifest, whose sha256 the R473 entry pinned before execution.
fn protocol_manifest() -> serde_json::Value {
    common::read_json(
        &repo_root().join("claudedocs/phase1-scale-floor-experiment/scale-floor-manifest.json"),
    )
}

/// What the four graded classes, and the honesty surfaces beside them, come to
/// on one arm — every number read from a gate rather than from a note.
#[derive(Debug, PartialEq, Eq)]
struct Floor {
    d1: usize,
    d2: usize,
    d3: usize,
    d4: usize,
    /// Continuity violations whose kind is in NEITHER graded class — what
    /// today's gate says that the June grader's classes do not name.
    ungraded_violations: BTreeMap<String, usize>,
    /// `recorded_not_counted` per the manifest: reported beside the endpoint,
    /// never summed into it.
    recorded: BTreeMap<&'static str, usize>,
    scenes: usize,
    facts: usize,
    /// The world-lines a story ENDS on, derived from the branch registry.
    endings: Vec<String>,
}

impl Floor {
    fn primary_endpoint(&self) -> usize {
        self.d1 + self.d2 + self.d3 + self.d4
    }
}

/// The branches no other branch forks from — the "ending world-lines" D3 counts
/// on, derived from what the author declared rather than from a list of names.
///
/// This matters more than it looks: the trunk and the mid-story junction dangle
/// nearly every required setup BY CONSTRUCTION (their payoffs lie downstream),
/// so a D3 that summed every world instead of the endings would read 24 and 29
/// where the grader read 3 and 5. The exclusion is structural, and so is the
/// derivation.
fn ending_world_lines(manifest: &serde_json::Value) -> Vec<String> {
    let branches = manifest["branches"].as_array().cloned().unwrap_or_default();
    let forked_from: BTreeSet<String> = branches
        .iter()
        .filter_map(|b| b["forks_from"].as_str().map(str::to_string))
        .collect();
    let endings: Vec<String> = branches
        .iter()
        .filter_map(|b| b["branch_id"].as_str())
        .filter(|id| !forked_from.contains(*id))
        .map(str::to_string)
        .collect();
    assert!(
        !endings.is_empty(),
        "a branch registry in which every branch is forked from is a cycle, not a story: \
         {branches:?}"
    );
    endings
}

/// The required setups: the facts whose author marked a payoff expectation.
fn required_setups(manifest: &serde_json::Value) -> BTreeSet<String> {
    manifest["facts"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter(|fact| !fact["payoff_expectation"].is_null())
        .filter_map(|fact| fact["fact_id"].as_str().map(str::to_string))
        .collect()
}

/// The arm's store, built from a fact manifest through the corpus recipe.
fn arm_workspace(arm: &str, manifest: &serde_json::Value) -> tempfile::TempDir {
    corpus_workspace_try(&repo_root().join(arm), manifest)
        .unwrap_or_else(|e| panic!("{arm}: the manifest must import: {e}"))
}

/// The same build, with the write path's REFUSAL handed back as the answer.
fn refusal_of(arm: &str, manifest: &serde_json::Value) -> String {
    match corpus_workspace_try(&repo_root().join(arm), manifest) {
        Err(reason) => reason,
        Ok(_) => panic!("{arm}: the write path accepted a manifest it must refuse"),
    }
}

/// Build the arm's store from a fact manifest and score it by the four classes.
fn floor_of(arm: &str, manifest: &serde_json::Value) -> Floor {
    let ws = arm_workspace(arm, manifest);

    // Read the JUDGEMENT, not the exit code: `validate-continuity` exits 1 on
    // any violation, and whether this store has one is the thing being asked.
    let out = run(ws.path(), &["validate-continuity", "--json"]);
    let continuity: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "{arm}: the continuity read emitted no json ({e}): {}",
            String::from_utf8_lossy(&out.stderr)
        )
    });
    let mut by_kind: BTreeMap<String, usize> = BTreeMap::new();
    for violation in continuity["violations"]
        .as_array()
        .cloned()
        .unwrap_or_default()
    {
        *by_kind
            .entry(
                violation["kind"]
                    .as_str()
                    .unwrap_or("(a violation with no kind)")
                    .to_string(),
            )
            .or_default() += 1;
    }
    let count_of = |kinds: &[&str]| -> usize {
        kinds
            .iter()
            .map(|kind| by_kind.get(*kind).copied().unwrap_or_default())
            .sum()
    };
    let graded: BTreeSet<&str> = D1_KINDS.iter().chain(D4_KINDS.iter()).copied().collect();
    let ungraded_violations = by_kind
        .iter()
        .filter(|(kind, _)| !graded.contains(kind.as_str()))
        .map(|(kind, n)| (kind.clone(), *n))
        .collect();

    let payoff = json_report(ws.path(), &["report-payoff-coverage", "--json"]);
    let endings = ending_world_lines(manifest);
    let required = required_setups(manifest);
    let worlds = payoff["worlds"].as_object().cloned().unwrap_or_default();
    let mut d3 = 0;
    let mut payoff_before_setup = 0;
    let mut payoffs_to_unmarked = 0;
    let mut unknown = 0;
    for ending in &endings {
        let world = worlds.get(ending).unwrap_or_else(|| {
            panic!(
                "{arm}: the payoff report has no world for the ending world-line `{ending}`; it \
                 answers about {:?}",
                worlds.keys().collect::<Vec<_>>()
            )
        });
        let dangling: BTreeSet<String> = world["dangling"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|id| id.as_str().map(str::to_string))
            .collect();
        // D3 counts dangling among the REQUIRED setups; anything else the
        // report calls dangling is craft signal the protocol records and does
        // not count, so a widening of the report's own notion cannot inflate
        // the score silently.
        let unrequired: Vec<&String> = dangling.difference(&required).collect();
        assert!(
            unrequired.is_empty(),
            "{arm}/{ending}: the payoff report calls {unrequired:?} dangling, and they are not \
             among the required setups this manifest declares"
        );
        d3 += dangling.len();
        payoff_before_setup += world["payoff_before_setup"]
            .as_array()
            .map(Vec::len)
            .unwrap_or_default();
        payoffs_to_unmarked += world["payoffs_to_unmarked"]
            .as_array()
            .map(Vec::len)
            .unwrap_or_default();
        unknown += world["unknown"]
            .as_array()
            .map(Vec::len)
            .unwrap_or_default();
    }

    let usize_at = |value: &serde_json::Value, key: &str| -> usize {
        value[key]
            .as_u64()
            .unwrap_or_else(|| panic!("{arm}: `{key}` is not a count: {}", value[key]))
            as usize
    };
    let recorded = BTreeMap::from([
        ("payoff_before_setup", payoff_before_setup),
        ("payoffs_to_unmarked", payoffs_to_unmarked),
        ("per-world unknown", unknown),
        (
            "cross_scope_pairs",
            usize_at(&continuity, "cross_scope_pairs"),
        ),
        ("unordered_pairs", usize_at(&continuity, "unordered_pairs")),
        (
            "rule_unordered_pairs",
            usize_at(&continuity, "rule_unordered_pairs"),
        ),
        (
            "undecidable_edges",
            payoff["undecidable_edges"]
                .as_array()
                .map(Vec::len)
                .unwrap_or_default(),
        ),
    ]);

    Floor {
        d1: count_of(&D1_KINDS),
        d2: usize_at(&continuity, "unchained_state_pairs"),
        d3,
        d4: count_of(&D4_KINDS),
        ungraded_violations,
        recorded,
        scenes: usize_at(&continuity, "sections"),
        facts: usize_at(&continuity, "facts"),
        endings,
    }
}

/// The vocabulary this law asks today's gates for is the vocabulary the PINNED
/// protocol defined — not a mapping this file invented and then agreed with.
///
/// Written first because it is the failure the rest of the file cannot catch: a
/// scoreboard that counts the wrong kind reproduces `0` beautifully. The
/// manifest's sha256 went into the R473 ledger entry before any prose existed,
/// so the definitions it carries cannot have been fitted to the result.
#[test]
fn the_graded_axes_are_the_ones_this_law_asks_for() {
    let manifest = protocol_manifest();
    let metrics = &manifest["grading"]["deterministic"]["defect_metrics"];
    let definition = |class: &str| -> String {
        metrics[class]
            .as_str()
            .unwrap_or_else(|| panic!("the protocol defines `{class}`: {metrics}"))
            .to_string()
    };

    let d1 = definition("D1");
    for kind in D1_KINDS {
        assert!(
            d1.contains(kind),
            "D1 is defined as `{d1}`, which does not name `{kind}`"
        );
    }
    let d2 = definition("D2");
    assert!(
        d2.contains("unchained_state_pairs"),
        "D2 is defined as `{d2}`"
    );
    let d3 = definition("D3");
    for word in ["dangling", "REQUIRED", "world-line"] {
        assert!(d3.contains(word), "D3 is defined as `{d3}`");
    }
    let d4 = definition("D4");
    assert!(
        d4.contains("succession_cross_branch") && d4.to_lowercase().contains("successioncycle"),
        "D4 is defined as `{d4}`"
    );
    assert_eq!(
        definition("primary_endpoint"),
        "D1+D2+D3+D4 summed per story",
        "the endpoint this law sums"
    );
}

/// The graded floor, re-asked: today's gates reproduce the blind grader's
/// table, arm for arm.
///
/// `evidence/defect-tables.md`, the June 2026 report:
///
/// | metric | story-A | story-B |
/// |---|---|---|
/// | D1 gated rule violations | 0 | 0 |
/// | D2 `unchained_state_pairs` | 0 | 0 |
/// | D3 dangling required setups (ending world-lines) | 3 | 5 |
/// | D4 fork-boundary faults | 0 | 0 |
/// | primary endpoint | 3 | 5 |
/// | facts / scenes / world-lines | 82 / 60 / 3 | 96 / 65 / 3 |
///
/// Facts read 83 and 97 here, one more per arm, and that is not drift: the
/// R697 carriage adds exactly one edge fact per authored transition step, which
/// `scale_floor_corpus_carriage` pins to that count and no other.
#[test]
fn todays_gates_reproduce_the_graded_floor_score() {
    let mut scored = BTreeMap::new();
    for arm in ARMS {
        let manifest = corpus_fact_manifest(&repo_root().join(arm));
        let floor = floor_of(arm, &manifest);
        println!(
            "  {arm}: D1 {} D2 {} D3 {} D4 {} => endpoint {} · {} scenes {} facts · endings {:?}",
            floor.d1,
            floor.d2,
            floor.d3,
            floor.d4,
            floor.primary_endpoint(),
            floor.scenes,
            floor.facts,
            floor.endings
        );
        println!("    ungraded violations: {:?}", floor.ungraded_violations);
        println!("    recorded, not counted: {:?}", floor.recorded);
        scored.insert(arm, floor);
    }

    let a = &scored[ARMS[0]];
    let b = &scored[ARMS[1]];

    assert_eq!(
        (a.d1, a.d2, a.d3, a.d4, a.primary_endpoint()),
        (0, 0, 3, 0, 3),
        "story-A's graded floor"
    );
    assert_eq!(
        (b.d1, b.d2, b.d3, b.d4, b.primary_endpoint()),
        (0, 0, 5, 0, 5),
        "story-B's graded floor"
    );
    assert_eq!(
        (a.scenes, a.endings.len(), b.scenes, b.endings.len()),
        (60, 3, 65, 3),
        "scenes and world-lines per arm"
    );
    // 82 and 96 as the grader counted them, plus the one carriage edge fact.
    assert_eq!((a.facts, b.facts), (82 + 1, 96 + 1), "facts per arm");

    // Every honesty surface the protocol says to report beside the endpoint was
    // 0 in the graded table, and the report is only honest while they are read.
    for (arm, floor) in &scored {
        for (surface, count) in &floor.recorded {
            assert_eq!(
                *count, 0,
                "{arm}: `{surface}` was 0 in the graded table and is {count} today"
            );
        }
    }

    // THE ONE DIFFERENCE, pinned rather than hidden. Arm A is silent; arm B's
    // `b42-papers-cendre` cites evidence at `sc-60`, a scene its own branch
    // cannot reach from `sc-42`. The grader's four classes do not name this
    // kind, so the endpoint is untouched — but today's gate sees a defect in a
    // blind extraction that the June floor did not, and that belongs in the
    // record next to the score rather than in a session's notes.
    assert!(
        a.ungraded_violations.is_empty(),
        "story-A: today's gate says something the graded floor did not: {:?}",
        a.ungraded_violations
    );
    assert_eq!(
        b.ungraded_violations,
        BTreeMap::from([("evidence_unreachable".to_string(), 1)]),
        "story-B: what today's gate says beyond the four graded classes"
    );
}

/// Every graded axis can still MOVE on these arms — so each `0` above is a
/// judgement rather than an inability.
///
/// This is the round's non-emptiness proof, and it is written against THESE
/// stores rather than against a fixture on purpose. Whether the continuity
/// gate can emit `rule_exclusive_overlap` somewhere in this tree is a different
/// question from whether it emits it HERE, where the rule that would fire is
/// one a blind extractor derived in June and a carriage has been rewriting ever
/// since. Three of the four axes read `0` on both arms; without this test that
/// is indistinguishable from four axes nobody is computing.
///
/// Each perturbation is a manifest an author could equally have written, run
/// through the same import path as the corpus itself (the R1033 rule) — never
/// a hand-edited sidecar.
#[test]
fn every_graded_axis_still_moves_on_these_arms() {
    for arm in ARMS {
        let dir = repo_root().join(arm);
        let base = corpus_fact_manifest(&dir);
        let baseline = floor_of(arm, &base);
        let endings = ending_world_lines(&base);
        let seat = base["facts"][0].clone();
        let subject = seat["entities"][0]
            .as_str()
            .expect("the seat fact names an entity")
            .to_string();

        // A fact shaped like the arm's own, on the arm's own seat.
        let probe = |id: &str, branch: &str, typed: serde_json::Value| -> serde_json::Value {
            let mut fact = seat.clone();
            let row = fact.as_object_mut().expect("a fact is an object");
            row.insert("fact_id".to_string(), serde_json::json!(id));
            row.insert("branch".to_string(), serde_json::json!(branch));
            row.insert(
                "claim".to_string(),
                serde_json::json!(format!("Probe fact {id}.")),
            );
            row.insert("typed".to_string(), typed);
            for absent in [
                "quote",
                "pays_off",
                "payoff_expectation",
                "supersedes_in_frame",
            ] {
                row.remove(absent);
            }
            fact
        };
        let with = |rows: Vec<serde_json::Value>| -> serde_json::Value {
            let mut manifest = base.clone();
            let facts = manifest["facts"].as_array_mut().expect("facts array");
            facts.extend(rows);
            manifest
        };
        let life = |token: &str| {
            serde_json::json!({
                "subject": subject,
                "predicate": "life_state",
                "object": { "kind": "token", "token": token },
            })
        };

        // --- D1: a step the author's own transition rule forbids. ---
        let mut dead = probe("probe-d1-dead", &endings[0], life("dead"));
        let mut alive = probe("probe-d1-alive", &endings[0], life("alive"));
        alive["supersedes_in_frame"] = serde_json::json!("probe-d1-dead");
        let d1_moved = floor_of(arm, &with(vec![dead.clone(), alive]));

        // --- D2: two states of one subject that no succession edge chains. ---
        dead["fact_id"] = serde_json::json!("probe-d2-dead");
        let loose = probe("probe-d2-alive", &endings[0], life("alive"));
        let d2_moved = floor_of(arm, &with(vec![dead.clone(), loose]));

        // --- D3: a required setup whose payoff is withdrawn on one ending. ---
        //
        // Aimed off the report rather than off the manifest, and that is the
        // whole difference between a perturbation and a wish. The first
        // `pays_off` fact in the manifest is the obvious target and it moves
        // nothing on arm B, because that arm pays its morphine setup off on
        // all three endings and dropping one credit leaves two — measured, on
        // the first draft of this test, which passed on A and read `5` on B.
        // So the target is the setup the report says is paid on THIS ending,
        // and every fact crediting it there is what gets withdrawn.
        let ending = &endings[0];
        let paid = json_report(
            arm_workspace(arm, &base).path(),
            &["report-payoff-coverage", "--json"],
        )["worlds"][ending]["paid"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let credit = paid
            .first()
            .unwrap_or_else(|| panic!("{arm}/{ending}: no required setup is paid off at all"))
            .clone();
        let withdrawn = credit["setup"]
            .as_str()
            .expect("a credit names its setup")
            .to_string();
        let mut unpaid = base.clone();
        for fact in unpaid["facts"].as_array_mut().expect("facts array") {
            let Some(credits) = fact["pays_off"].as_array().cloned() else {
                continue;
            };
            let kept: Vec<serde_json::Value> = credits
                .into_iter()
                .filter(|setup| setup.as_str() != Some(withdrawn.as_str()))
                .collect();
            let row = fact.as_object_mut().expect("a fact is an object");
            match kept.is_empty() {
                true => {
                    row.remove("pays_off");
                }
                false => {
                    row.insert("pays_off".to_string(), serde_json::json!(kept));
                }
            }
        }
        let d3_moved = floor_of(arm, &unpaid);
        let dangling_now: BTreeSet<String> = json_report(
            arm_workspace(arm, &unpaid).path(),
            &["report-payoff-coverage", "--json"],
        )["worlds"][ending]["dangling"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|id| id.as_str().map(str::to_string))
            .collect();

        println!("  {arm} baseline: {baseline:?}");
        for (axis, moved) in [("D1", &d1_moved), ("D2", &d2_moved), ("D3", &d3_moved)] {
            println!(
                "  {arm} {axis} injection => D1 {} D2 {} D3 {} D4 {} endpoint {} ungraded {:?}",
                moved.d1,
                moved.d2,
                moved.d3,
                moved.d4,
                moved.primary_endpoint(),
                moved.ungraded_violations
            );
        }
        println!(
            "  {arm} D3 injection withdrew `{withdrawn}` (credited on {ending} by {}); {ending} \
             now dangles {dangling_now:?}",
            credit["payoffs"]
        );

        assert!(
            d1_moved.d1 > baseline.d1,
            "{arm}: a step the carried rule forbids left D1 at {}",
            d1_moved.d1
        );
        assert!(
            d2_moved.d2 > baseline.d2,
            "{arm}: two unchained states of one subject left D2 at {}",
            d2_moved.d2
        );
        assert!(
            d3_moved.d3 > baseline.d3,
            "{arm}: withdrawing a payoff left D3 at {}",
            d3_moved.d3
        );
        assert!(
            dangling_now.contains(&withdrawn),
            "{arm}/{ending}: `{withdrawn}` lost its only credit here and is still not dangling — \
             D3 rose somewhere else, so this injection is not aimed at what it claims"
        );
    }
}

/// D4's zero is not the continuity gate counting to nothing — it is EVERY write
/// path refusing to store the defect at all. That is a stronger guarantee than
/// the graded floor recorded, and it only holds while it holds on all of them.
///
/// Found by trying to move the axis, which is why this test exists and the
/// fourth arm of the injection test does not: the D4 perturbation this round
/// first wrote — a succession edge from one ending world-line to its sibling —
/// never reached `validate-continuity` at all, because `import-facts` rejected
/// the manifest (Round 433, the invariant; Round 488, the reachability it
/// decides with). So that axis cannot be perturbed the way the other three can,
/// and the honest law is about the refusal rather than about the count.
///
/// It is asked of EVERY primitive that can write `supersedes_in_frame`, which
/// is the field-invariant parity discipline this project already paid for: a
/// field with two write paths and one enforced invariant is a field with no
/// invariant, because the loose path decides what the store may contain. Four
/// primitives can write this one — the bulk manifest, the single-fact
/// primitive, the authorial revision, and the reviewed edge import, which is
/// how these very stores got their succession edges in June — and the grader's
/// `0` is worth exactly as much as the weakest of them. Both shapes D4 names
/// are asked: the sibling-branch edge and the succession cycle.
#[test]
fn no_write_path_will_store_a_fault_the_d4_axis_counts() {
    for arm in ARMS {
        let dir = repo_root().join(arm);
        let base = corpus_fact_manifest(&dir);
        let endings = ending_world_lines(&base);
        // Two ENDING world-lines are siblings by construction: neither forks
        // from the other, so neither inherits the other's belief.
        let (here, sibling) = (&endings[0], &endings[1]);
        let subject = base["facts"][0]["entities"][0]
            .as_str()
            .expect("the seat fact names an entity")
            .to_string();
        let scene_on = |branch: &str| -> String {
            base["facts"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .iter()
                .find(|fact| fact["branch"].as_str() == Some(branch))
                .and_then(|fact| fact["canon_from"].as_str().map(str::to_string))
                .unwrap_or_else(|| panic!("{arm}: no authored fact sits on `{branch}`"))
        };
        let (scene_here, scene_there) = (scene_on(here), scene_on(sibling));

        // --- path 1: the bulk manifest. ---
        let mut manifest = base.clone();
        let seat = manifest["facts"][0].clone();
        let probe = |id: &str, branch: &str, scene: &str, token: &str| -> serde_json::Value {
            let mut fact = seat.clone();
            let row = fact.as_object_mut().expect("a fact is an object");
            row.insert("fact_id".to_string(), serde_json::json!(id));
            row.insert("branch".to_string(), serde_json::json!(branch));
            row.insert("canon_from".to_string(), serde_json::json!(scene));
            row.insert("evidence".to_string(), serde_json::json!([scene]));
            row.insert(
                "claim".to_string(),
                serde_json::json!(format!("Probe fact {id}.")),
            );
            row.insert(
                "typed".to_string(),
                serde_json::json!({
                    "subject": subject,
                    "predicate": "life_state",
                    "object": { "kind": "token", "token": token },
                }),
            );
            for absent in [
                "quote",
                "pays_off",
                "payoff_expectation",
                "supersedes_in_frame",
            ] {
                row.remove(absent);
            }
            fact
        };
        let predecessor = probe("probe-fork-predecessor", here, &scene_here, "alive");
        let mut successor = probe("probe-fork-successor", sibling, &scene_there, "dead");
        successor["supersedes_in_frame"] = serde_json::json!("probe-fork-predecessor");
        manifest["facts"]
            .as_array_mut()
            .expect("facts array")
            .extend([predecessor.clone(), successor.clone()]);
        let refusals = vec![("import-facts".to_string(), refusal_of(arm, &manifest))];

        // The same two facts, UNCHAINED, are a manifest every write path takes
        // — so what the paths below refuse is the edge and not the probe.
        let mut chainless = base.clone();
        chainless["facts"]
            .as_array_mut()
            .expect("facts array")
            .extend([predecessor, {
                let mut row = successor.clone();
                row.as_object_mut()
                    .expect("a fact")
                    .remove("supersedes_in_frame");
                row
            }]);
        let ws = arm_workspace(arm, &chainless);

        let mut refusals = refusals;
        // --- path 2: the single-fact primitive. ---
        let add = run(
            ws.path(),
            &[
                "add-fact",
                "--fact",
                "probe-fork-added",
                "--frame",
                successor["frame"].as_str().expect("the seat names a frame"),
                "--branch",
                sibling,
                "--claim",
                "Probe: a succession edge reaching across the fork.",
                "--canon-from",
                &scene_there,
                "--evidence",
                &scene_there,
                "--entities",
                &subject,
                "--supersedes",
                "probe-fork-predecessor",
                "--typed-subject",
                &subject,
                "--typed-predicate",
                "life_state",
                "--typed-object-token",
                "dead",
            ],
        );
        refusals.push(("add-fact".to_string(), verdict_of(&add)));

        // --- path 3: the authorial in-place revision. ---
        let amend = run(
            ws.path(),
            &[
                "amend-fact",
                "--fact",
                "probe-fork-successor",
                "--reason",
                "Probe: reach the edge across the fork by revision instead.",
                "--frame",
                successor["frame"].as_str().expect("the seat names a frame"),
                "--branch",
                sibling,
                "--claim",
                "Probe fact probe-fork-successor.",
                "--canon-from",
                &scene_there,
                "--evidence",
                &scene_there,
                "--entities",
                &subject,
                "--supersedes",
                "probe-fork-predecessor",
                "--typed-subject",
                &subject,
                "--typed-predicate",
                "life_state",
                "--typed-object-token",
                "dead",
            ],
        );
        refusals.push(("amend-fact".to_string(), verdict_of(&amend)));

        // --- path 4: the reviewed edge import, which is how these very stores
        // got their succession edges in June. ---
        let candidates = json_report(ws.path(), &["report-edge-candidates", "--json"]);
        let sha_of = |fact: &str| -> String {
            candidates["facts"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .iter()
                .find(|row| row["fact_id"].as_str() == Some(fact))
                .and_then(|row| row["claim_sha256"].as_str().map(str::to_string))
                .unwrap_or_else(|| {
                    panic!("{arm}: the edge-candidate report does not carry `{fact}`")
                })
        };
        let proposals = serde_json::json!({
            "schema": "edge-proposals/v1",
            "succession": [{
                "predecessor": "probe-fork-predecessor",
                "successor": "probe-fork-successor",
                "predecessor_claim_sha256": sha_of("probe-fork-predecessor"),
                "successor_claim_sha256": sha_of("probe-fork-successor"),
                "rationale": "Probe: the fork-boundary edge, proposed and reviewed.",
            }],
            "conflicts": [],
        });
        std::fs::write(
            ws.path().join("probe-edges.json"),
            serde_json::to_string(&proposals).expect("proposals serialize"),
        )
        .expect("write probe proposals");
        let edges = run(
            ws.path(),
            &[
                "import-edge-proposals",
                "--proposals",
                "probe-edges.json",
                "--json",
            ],
        );
        refusals.push(("import-edge-proposals".to_string(), verdict_of(&edges)));

        // --- the second shape D4 counts: a succession CYCLE, on one branch. ---
        let mut cyclic = base.clone();
        let mut first = probe("probe-cycle-first", here, &scene_here, "alive");
        let mut second = probe("probe-cycle-second", here, &scene_here, "dead");
        first["supersedes_in_frame"] = serde_json::json!("probe-cycle-second");
        second["supersedes_in_frame"] = serde_json::json!("probe-cycle-first");
        cyclic["facts"]
            .as_array_mut()
            .expect("facts array")
            .extend([first, second]);
        refusals.push((
            "import-facts (a succession cycle)".to_string(),
            refusal_of(arm, &cyclic),
        ));

        for (path, refusal) in &refusals {
            println!("  {arm} {path} refused with: {refusal}");
            assert!(
                !refusal.is_empty(),
                "{arm}: `{path}` STORED a fault D4 counts — that axis's 0 is enforced by whichever \
                 path refuses, so one that accepts is the whole invariant"
            );
        }
        // And the store the accepted writes left behind still carries no edge
        // across the boundary: a path that "refused" while writing anyway would
        // otherwise read as held.
        let after = floor_of(arm, &chainless);
        assert_eq!(after.d4, 0, "{arm}: a fork-boundary fault survived");
    }
}

/// A mutate verb's refusal, from whichever stream it speaks on — empty when it
/// did not refuse at all.
fn verdict_of(out: &std::process::Output) -> String {
    if out.status.success() {
        return String::new();
    }
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    if !stderr.is_empty() {
        return stderr;
    }
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}
