//! mnemosyne-engine-build — bake a store projection at BUILD time (Round 770).
//!
//! A consumer used to project the store on every run. That is correct and it is
//! also work done once per launch for an answer that only changes when the store
//! does, and the store is a committed file. This crate moves the projection to
//! the build: it reads the store, runs the whole fail-loud projection, and writes
//! the result as Rust source. The consumer compiles that source and starts with
//! no store read, no parse, and no `Result`
//! ([`PlayableProjection::from_parts`](mnemosyne_engine::PlayableProjection::from_parts)
//! is infallible, R769).
//!
//! BOTH axes bake (Round 774): [`emit_playable_projection`] for the narrative
//! one, [`emit_quest_projection`] for the journal one. A consumer that calls both
//! opens the store zero times at startup, which is the whole of "everything at
//! compile time" rather than the half of it one emitter buys.
//!
//! # Why the checks did not go away
//!
//! They moved to the one place that HAS the store. A dangling locator, an
//! unresolvable ladder anchor, an unreadable sidecar — every one of those is now
//! a BUILD failure instead of a runtime one. What the compiler adds on top is the
//! shape contract: change a kernel type and the generated source stops compiling,
//! which is the check a serialized snapshot can never carry (an unknown field is
//! ignored, a missing option defaults, and the stale artifact loads quietly).
//!
//! # Why the artifact cannot go stale
//!
//! [`emit_playable_projection`] prints `cargo:rerun-if-changed` for every input
//! it read — RESOLVED through the workspace's own config rather than assumed,
//! which is the correction Round 772 made after R770 assumed them and named a
//! store the first real consumer does not use. Cargo regenerates whenever the
//! store moves, and the output belongs in `OUT_DIR` where it is never committed.
//! A declared input that is not the file the loader opens is worse than no claim
//! at all: it reads as watched. The anti-drift property a runtime
//! read was chosen for comes from DERIVING FROM THE STORE, not from the timing of
//! the derivation. The honest residual: a pre-built binary is pinned to the store
//! revision it was built from, the same trade a consumer already makes pinning
//! this workspace by git rev.
//!
//! # Using it
//!
//! The consumer's `build.rs` carries no logic of its own — that is the point. A
//! build script that DERIVED anything would put the derivation back in the
//! consumer's repository, which is the thing being removed:
//!
//! This is the whole body of the consumer's `build.rs` `main` (shown unwrapped so
//! rustdoc compiles it as part of this crate's test suite):
//!
//! ```no_run
//! let overrides = mnemosyne_engine::StaticOverrides {
//!     interactivity: mnemosyne_engine::store_interactivity(std::path::Path::new("."))
//!         .expect("the store's interactive layer"),
//!     ..Default::default()
//! };
//! let source = mnemosyne_engine_build::emit_playable_projection(
//!     std::path::Path::new("."),
//!     "reader",
//!     &overrides,
//! )
//! .expect("bake the playable projection");
//! std::fs::write(
//!     std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("playable.rs"),
//!     source,
//! )
//! .unwrap();
//! ```
//!
//! and the crate does `include!(concat!(env!("OUT_DIR"), "/playable.rs"));`.
//!
//! The quest axis is the same shape, with the consumer's own precondition
//! predicate the one thing it must declare (the kernel does not know a
//! predicate's name):
//!
//! ```no_run
//! let overrides = mnemosyne_engine::StaticOverrides {
//!     quest_precondition_predicates: vec!["opened_by".to_string()],
//!     ..Default::default()
//! };
//! let source = mnemosyne_engine_build::emit_quest_projection(
//!     std::path::Path::new("."),
//!     "reader",
//!     &overrides,
//! )
//! .expect("bake the quest projection");
//! std::fs::write(
//!     std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("quests.rs"),
//!     source,
//! )
//! .unwrap();
//! ```
//!
//! Both `include!`s may sit in the SAME module (Round 781). An artifact walls its
//! generated internals behind a module named for the public item it defines, so
//! two artifacts share no private name; before that they both named their chunk
//! functions `__mn_0` onward and splicing the pair together was `E0428` once per
//! chunk. A module apiece is needed only to bake the same axis twice, where the
//! PUBLIC item is what collides.

use std::path::Path;

use mnemosyne_engine::{EngineError, EngineOverrides, PlayableProjection, QuestProjection};

mod render;
pub use render::{render, render_quest};

/// Project the workspace at build time and return Rust source defining
/// `pub fn playable_projection() -> PlayableProjection`.
///
/// Also prints the `cargo:rerun-if-changed` lines for the inputs the projection
/// read, so a store edit regenerates the artifact. Printing them is why this runs
/// from a build script rather than a binary: those lines are a build-script
/// protocol, and emitting them here is what keeps the consumer's own script free
/// of logic.
///
/// # Errors
///
/// Whatever the projection fails with — [`EngineError::Projection`] for an
/// unreadable store or a typo'd telling, [`EngineError::LocatorFactMissing`] for
/// a dangling locator, [`EngineError::RungQuestionUnresolvable`] for a ladder
/// anchor that does not land. Each is a BUILD failure, which is the round's whole
/// point: the consumer's runtime never sees them.
pub fn emit_playable_projection(
    workspace_root: &Path,
    telling: &str,
    overrides: &impl EngineOverrides,
) -> Result<String, EngineError> {
    declare_inputs(workspace_root)?;
    let projection = PlayableProjection::from_workspace(workspace_root, telling, overrides)?;
    Ok(render(&projection.to_parts()))
}

/// Project the workspace's QUEST graph at build time and return Rust source
/// defining `pub fn quest_projection() -> QuestProjection` (Round 774) — the
/// JOURNAL-axis sibling of [`emit_playable_projection`], and the same trade: a
/// consumer that baked both opens the store zero times at startup.
///
/// # What moves and what does not
///
/// The DERIVATION of the quest layer moves to the build; the EVALUATION of quest
/// rules does not. R764 said quest preconditions are game rules over player state
/// and stay at runtime, and that is untouched here — a baked
/// [`QuestProjection`](mnemosyne_engine::QuestProjection) still answers
/// `completability` against a playable projection whenever it is asked, because
/// the gate is a question, not a field.
///
/// # Why there is no order override
///
/// [`QuestProjection::from_workspace`](mnemosyne_engine::QuestProjection::from_workspace)
/// takes one; this does not, and passes `None`. An `--order` override is
/// CWD-relative CLI semantics with no meaning in a build script, and worse, it
/// would read a canon order this crate did not DECLARE to cargo — which is
/// exactly the divergence between the file declared and the file opened that
/// R772 closed. Declaration and read stay one thing by leaving no way to split
/// them.
///
/// # Errors
///
/// [`EngineError::Projection`] if the quest-graph read fails — an unregistered
/// world, a typo'd telling, a malformed quest predicate, an unreadable store.
/// Each is a BUILD failure; the consumer's runtime never sees them.
pub fn emit_quest_projection(
    workspace_root: &Path,
    telling: &str,
    overrides: &impl EngineOverrides,
) -> Result<String, EngineError> {
    declare_inputs(workspace_root)?;
    let projection = QuestProjection::from_workspace(workspace_root, telling, None, overrides)?;
    Ok(render_quest(&projection.to_parts()))
}

/// Tell cargo which files the bake depends on, so a store edit regenerates it.
///
/// The paths are RESOLVED through the same config the loader reads
/// ([`mnemosyne_engine::projection_inputs`]), never assumed. R770 assumed them,
/// naming the built-in default sidecar; the first real consumer declares
/// `[atomic] sidecar_path`, so the store the bake actually read was never
/// watched and rebuilding it left the artifact silently stale — the one failure
/// this crate's "cannot go stale" claim is about.
///
/// Declared BEFORE the projection runs: the dependency is registered even when
/// the projection then fails, so a fixed store is picked up as an edit rather
/// than relying on cargo re-running a script that errored.
fn declare_inputs(workspace_root: &Path) -> Result<(), EngineError> {
    for input in mnemosyne_engine::projection_inputs(workspace_root)? {
        println!("cargo:rerun-if-changed={}", input.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use mnemosyne_engine::{
        DisclosureMode, DoorPart, Interactivity, LinePart, PlayableProjection, ProjectionParts,
        QuestCompletionPart, QuestPart, QuestProjection, QuestProjectionParts, QuestState,
        QuestWorldPart,
    };

    /// The generator's own output, compiled INTO this crate by `build.rs`. That
    /// this module exists at all is the gate: if `render` emitted Rust that does
    /// not compile, or named a kernel type that changed shape, the build fails
    /// here — the same failure a consumer would get, caught in this workspace.
    ///
    /// BOTH artifacts land in ONE module (Round 781), which is the arrangement
    /// this crate's own docs describe and the only one that exercises the axis
    /// added in R774: an emitter names its chunk functions from a counter that
    /// restarts at zero, so two artifacts define the same private names and
    /// splicing them together was `E0428` once per chunk. Until this round every
    /// `include!` in this workspace — here and the six in the stack probe — sat in
    /// a module of its own, so nothing here could see it and the first consumer
    /// found it instead. A pair kept apart proves only that they compile.
    mod baked {
        include!(concat!(env!("OUT_DIR"), "/fixture_playable.rs"));
        include!(concat!(env!("OUT_DIR"), "/fixture_quest.rs"));
    }

    #[test]
    fn the_generated_source_rebuilds_the_projection_it_was_baked_from() {
        // Compiling was the hard part; this asserts the values SURVIVED, so a
        // generator that emitted syntactically valid but wrong code still fails.
        let proj = baked::playable_projection();
        assert_eq!(proj.telling(), "reader");
        assert_eq!(proj.walk("main"), ["sc-01".to_string()]);
        assert!(proj.is_divergent_ending("dark"));

        // The nasty string round-tripped through a Rust literal: an embedded
        // quote, a newline, a backslash and non-ASCII. `render` claims `{:?}` is
        // exactly Rust's escaping; this is rustc agreeing.
        let nasty = "그는 \"셈\"이라 했다.\n뒤에 \\ 하나.";
        let lines = proj.lines("main", "sc-01");
        assert_eq!(lines[0].text(), nasty);
        assert_eq!(proj.title("sc-01"), Some(nasty));

        // Option in both states, and a count.
        assert_eq!(lines[0].carrier(), Some("ent-ledger"));
        assert_eq!(lines[0].mode(), DisclosureMode::Hint);
        assert_eq!(lines[1].carrier(), None);

        // Cast, forks, and all three door kinds survive.
        assert_eq!(proj.cast_at("sc-01")[0].entity(), "ent-jongdeuk");
        assert_eq!(proj.forks_at("sc-01", "main")[0].world, "dark");
        let doors = proj
            .scene("main", "sc-01", &std::collections::HashSet::new())
            .doors;
        assert!(doors.iter().any(|d| matches!(
            d,
            mnemosyne_engine::Door::Ask { question, .. } if question == nasty
        )));
        assert!(doors
            .iter()
            .any(|d| matches!(d, mnemosyne_engine::Door::Examine { object, .. } if object == "ent-ledger")));
    }

    #[test]
    fn parts_are_constructible_from_a_downstream_crate() {
        // The check an IN-CRATE test structurally cannot make. `Line`, `Door` and
        // `Fork` are `#[non_exhaustive]` with crate-private fields, so a literal
        // for any of them fails to compile OUTSIDE mnemosyne-engine — and R769's
        // equivalence test lives inside it, which is why it could not see that
        // `ProjectionParts` still carried two such types (fixed in R770 by
        // `ForkPart` / `DoorPart`). This crate is downstream, so if the parts type
        // ever readmits a non-constructible member, THIS stops compiling.
        let parts = ProjectionParts {
            telling: "reader".to_string(),
            by_world: vec![(
                "main".to_string(),
                vec![(
                    "sc-01".to_string(),
                    vec![LinePart {
                        fact_id: "f-a".to_string(),
                        text: "a line".to_string(),
                        mode: DisclosureMode::State,
                        frame: String::new(),
                        entities: Vec::new(),
                        carrier: None,
                        typed_predicate: None,
                        quote: None,
                        count: None,
                    }],
                )],
            )],
            walks: vec![("main".to_string(), vec!["sc-01".to_string()])],
            titles: Vec::new(),
            cast: Vec::new(),
            forks: Vec::new(),
            divergent_endings: Vec::new(),
            interactivity: Interactivity::default(),
            choice_entity_refs: Vec::new(),
            ask_doors: vec![(
                "sc-01".to_string(),
                vec![DoorPart::Ask {
                    question: "물었다".to_string(),
                    reveals: "f-a".to_string(),
                }],
            )],
        };
        let proj = PlayableProjection::from_parts(parts);
        assert_eq!(proj.lines("main", "sc-01").len(), 1);
    }

    #[test]
    fn the_generated_quest_source_rebuilds_the_projection_it_was_baked_from() {
        // Compiling was the hard part; this asserts the VALUES survived, so a
        // generator that emitted syntactically valid but wrong code still fails.
        let proj = baked::quest_projection();
        assert_eq!(proj.telling(), "reader");
        assert_eq!(proj.quests().len(), 2);

        let knot = proj.quest("q-knot-1").expect("the quest is baked");
        // The nasty string round-tripped through a Rust literal: a quest
        // objective is authored prose and quotes as freely as a line does.
        assert_eq!(knot.objective, "그는 \"셈\"이라 했다.\n뒤에 \\ 하나.");
        assert_eq!(knot.actors, ["ent-jiun".to_string()]);
        assert_eq!(knot.prerequisites, ["q-salt".to_string()]);
        assert_eq!(knot.preconditions, ["f-clue".to_string()]);

        // Two roads on one quest, and all three states across the fixture.
        assert_eq!(knot.per_world["main"].state, QuestState::Done);
        assert_eq!(knot.per_world["dark"].state, QuestState::Unknown);
        assert_eq!(
            proj.quest("q-salt").expect("the second quest").per_world["main"].state,
            QuestState::Open
        );

        // Option in both states, on the completions that carry the actor.
        let done = &knot.per_world["main"].completions;
        assert_eq!(done[0].actor.as_deref(), Some("ent-eldest"));
        assert_eq!(done[0].scene, "sc-gut");
        assert_eq!(done[1].actor, None);
    }

    /// Parts carrying `n` lines in one section — a fixture that GROWS, for the
    /// scaling assertion below.
    fn parts_with_lines(n: usize) -> ProjectionParts {
        ProjectionParts {
            telling: "reader".to_string(),
            by_world: vec![(
                "main".to_string(),
                vec![(
                    "sc-01".to_string(),
                    (0..n)
                        .map(|i| LinePart {
                            fact_id: format!("f-{i:06}"),
                            text: "그는 \"셈\"이라 했다.".to_string(),
                            mode: DisclosureMode::State,
                            frame: "ground-truth".to_string(),
                            entities: vec!["ent-a".to_string()],
                            carrier: None,
                            typed_predicate: None,
                            quote: None,
                            count: None,
                        })
                        .collect(),
                )],
            )],
            walks: vec![("main".to_string(), vec!["sc-01".to_string()])],
            titles: Vec::new(),
            cast: Vec::new(),
            forks: Vec::new(),
            divergent_endings: Vec::new(),
            interactivity: Interactivity::default(),
            choice_entity_refs: Vec::new(),
            ask_doors: Vec::new(),
        }
    }

    #[test]
    fn a_bigger_store_makes_more_functions_not_a_bigger_one() {
        // Round 775 — the emitter's contract, and the reason it exists: rustc's
        // per-body work is superlinear, so a generator that puts a whole store in
        // one function body pays worse than proportionally as the store grows.
        // Measured on a controlled pair (same literals, same types, same bytes):
        // 29.1s / 1.21GB in one body against 5.5s / 0.57GB in a hundred, and
        // halving the input took the one-body case to 9.0s — 2x the input for
        // 3.2x the time. On the first consumer's real store the emitted file went
        // from 34.5s / 1.13GB to 7.5s / 0.53GB.
        //
        // The assertion is the SCALING, not a byte threshold: quadruple the store
        // and the file must grow while the largest single body must not. Before
        // this round the largest line WAS the file — 98% of it.
        let longest = |src: &str| src.lines().map(str::len).max().unwrap_or(0);
        let small = crate::render(&parts_with_lines(200));
        let big = crate::render(&parts_with_lines(800));
        assert!(
            big.len() > 3 * small.len(),
            "the fixture must actually grow: {} vs {}",
            small.len(),
            big.len()
        );
        assert!(
            longest(&big) < 2 * longest(&small),
            "a function body grew with the store: {} -> {}",
            longest(&small),
            longest(&big)
        );
        // And the growth went where it was supposed to go.
        assert!(big.matches("fn __mn_").count() > small.matches("fn __mn_").count());
    }

    /// One quest — the journal-axis sibling of [`parts_with_lines`], sized only to
    /// make the emitter produce a chunk function.
    fn parts_with_quests() -> QuestProjectionParts {
        QuestProjectionParts {
            telling: "reader".to_string(),
            quests: vec![QuestPart {
                quest_id: "q-1".to_string(),
                objective: "a quest".to_string(),
                actors: Vec::new(),
                prerequisites: Vec::new(),
                per_world: vec![(
                    "main".to_string(),
                    QuestWorldPart {
                        state: QuestState::Open,
                        completions: Vec::new(),
                    },
                )],
                preconditions: Vec::new(),
            }],
        }
    }

    #[test]
    fn the_two_artifacts_define_the_same_private_names_and_compose_anyway() {
        // Round 781. `mod baked` above splices both artifacts into one module, and
        // that is a COMPILE-time claim — which is also what two empty files would
        // satisfy. This is the arm that makes it non-vacuous, and it is here rather
        // than in a file of its own for the Round 780 reason: split apart, the
        // composition could report green while the hazard it absorbs had quietly
        // stopped being present.
        let playable = crate::render(&parts_with_lines(1));
        let quest = crate::render_quest(&parts_with_quests());

        // The hazard: the counter restarts per render, so both really do define
        // `__mn_0`. If either stopped, `mod baked` would prove nothing.
        for (what, src) in [("playable", &playable), ("quest", &quest)] {
            assert!(
                src.contains("fn __mn_0("),
                "the {what} artifact defines no `__mn_0`, so the name collision the \
                 module wall absorbs is not present and `mod baked` compiling says \
                 nothing about it:\n{src}"
            );
        }

        // The wall: derived from the public item each artifact defines, so it is
        // distinct exactly when the two are composable at all. Asserted as a
        // DIFFERENCE rather than against the two spellings — a test that named them
        // would restate the emitter instead of checking it.
        let wall = |src: &str| {
            src.lines()
                .find(|l| l.starts_with("mod __mn_"))
                .expect("the artifact walls its chunk functions in a module")
                .to_string()
        };
        assert_ne!(
            wall(&playable),
            wall(&quest),
            "both artifacts wall their identical private names behind the SAME \
             module name, so the wall does not separate them"
        );
    }

    #[test]
    fn quest_parts_are_constructible_from_a_downstream_crate() {
        // The check an IN-CRATE test structurally cannot make, and the reason it
        // is repeated on this axis rather than assumed from the playable one:
        // every QuestView type is `#[non_exhaustive]`, so a literal for any of
        // them fails to compile OUTSIDE mnemosyne-engine — while R773's
        // round-trip test lives INSIDE it and would not notice. This crate is
        // downstream, so if QuestProjectionParts ever readmits a member no
        // foreign crate can build, THIS stops compiling. That is exactly how the
        // R769 defect survived until R770 caught it from here.
        let parts = QuestProjectionParts {
            telling: "reader".to_string(),
            quests: vec![QuestPart {
                quest_id: "q-1".to_string(),
                objective: "a quest".to_string(),
                actors: Vec::new(),
                prerequisites: Vec::new(),
                per_world: vec![(
                    "main".to_string(),
                    QuestWorldPart {
                        state: QuestState::Done,
                        completions: vec![QuestCompletionPart {
                            fact: "f-done".to_string(),
                            scene: "sc-01".to_string(),
                            actor: None,
                        }],
                    },
                )],
                preconditions: Vec::new(),
            }],
        };
        let proj = QuestProjection::from_parts(parts);
        assert_eq!(proj.quests().len(), 1);
    }
}
