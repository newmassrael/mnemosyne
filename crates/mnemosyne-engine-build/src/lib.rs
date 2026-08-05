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
//! The generated item hands back a BORROW that lives as long as the process —
//! `playable_projection() -> &'static PlayableProjection` (Round 786) — so a
//! consumer holds the handle rather than the projection, and every `&str` an
//! accessor returns is a `&'static str` without one signature changing. Storing
//! the returned value in a struct and reading out of it was self-referential
//! while the entry point handed over ownership, which is why the first consumer
//! copied the whole projection into types of its own instead.
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

use mnemosyne_engine::{
    EngineError, EngineOverrides, MapProjection, PlayableProjection, QuestProjection,
};

mod render;
pub use render::{render, render_map, render_passages, render_quest};

/// Project the workspace at build time and return Rust source defining
/// `pub fn playable_projection() -> &'static PlayableProjection`.
///
/// Also prints the `cargo:rerun-if-changed` lines for the inputs the projection
/// read, so a store edit regenerates the artifact. Printing them is why this runs
/// from a build script rather than a binary: those lines are a build-script
/// protocol, and emitting them here is what keeps the consumer's own script free
/// of logic.
///
/// # The lifetime the entry point hands out (Round 786)
///
/// The item returns a borrow of a projection built once per process, not a value.
/// Nothing in the kernel moved: lifetime elision already reads
/// `pub fn text(&self) -> &str` as `fn text<'s>(&'s self) -> &'s str`, so given a
/// `&'static` receiver those same accessors yield `&'static str`, and
/// `PlayableProjection::walk`, `::spine` and `Line::entities` yield
/// `&'static [String]`. What blocked the first consumer was never the accessor
/// signature it named — it was that this entry point handed back something it had
/// to OWN, so reading out of the copy its own struct held would have been
/// self-referential.
///
/// A consumer that stored the projection and rebuilt it into types of its own can
/// stop: hold the `&'static` and let the borrows flow. Note that this CHANGES the
/// generated signature, so a consumer bumping past Round 786 adapts one line.
///
/// The property is gated by
/// `the_baked_artifacts_hand_out_process_lifetime_borrows` in this crate's tests,
/// which feeds real accessor output into parameters that accept only `&'static`.
/// What follows is the pair proving that feed DISCRIMINATES rather than accepting
/// anything: the two blocks differ in exactly one visible line, whether the
/// projection is held for the process, and the second must not compile.
///
/// ```
/// # fn parts() -> mnemosyne_engine::ProjectionParts {
/// #     mnemosyne_engine::ProjectionParts {
/// #         telling: "reader".into(), by_world: Vec::new(), walks: Vec::new(),
/// #         titles: Vec::new(), cast: Vec::new(), forks: Vec::new(),
/// #         divergent_endings: Vec::new(), interactivity: Default::default(),
/// #         choice_entity_refs: Vec::new(), ask_doors: Vec::new(),
/// #         journal_offers: Vec::new(),
/// #     }
/// # }
/// fn keep_forever(_: &'static str) {}
/// let projection = mnemosyne_engine::PlayableProjection::from_parts(parts());
/// let held: &'static mnemosyne_engine::PlayableProjection = Box::leak(Box::new(projection));
/// keep_forever(held.telling());
/// ```
///
/// ```compile_fail
/// # fn parts() -> mnemosyne_engine::ProjectionParts {
/// #     mnemosyne_engine::ProjectionParts {
/// #         telling: "reader".into(), by_world: Vec::new(), walks: Vec::new(),
/// #         titles: Vec::new(), cast: Vec::new(), forks: Vec::new(),
/// #         divergent_endings: Vec::new(), interactivity: Default::default(),
/// #         choice_entity_refs: Vec::new(), ask_doors: Vec::new(),
/// #         journal_offers: Vec::new(),
/// #     }
/// # }
/// fn keep_forever(_: &'static str) {}
/// let held = mnemosyne_engine::PlayableProjection::from_parts(parts());
/// keep_forever(held.telling());
/// ```
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
/// defining `pub fn quest_projection() -> &'static QuestProjection` (Round 774,
/// borrowing since Round 786) — the JOURNAL-axis sibling of
/// [`emit_playable_projection`], and the same trade: a consumer that baked both
/// opens the store zero times at startup.
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

/// Project the workspace's declared MAPS at build time and return Rust source
/// defining `pub fn map_projection() -> &'static MapProjection` — the PLACE-axis
/// sibling of [`emit_playable_projection`], and the emitter the first consumer
/// of this axis has been waiting on.
///
/// # What this replaces
///
/// A consumer opening our store sidecar by hand. The map read landed on the CLI
/// and MCP surfaces and nowhere a linked kernel could reach, so the consumer
/// that had just authored 28 edges' costs and guards parsed the sidecar itself
/// to get them back — 1.2 MB per start, then moved to a build-time bake of its
/// own. Its comment names the cost (our store's shape bitten in two places on
/// its side) and the single swap point that was waiting on us. This is it.
///
/// # Why it declares one more input than its siblings
///
/// [`declare_inputs`] watches the config, the canon order and the sidecar. A map
/// read opens one more file — the narrative-rules artifact — and it is not a
/// detail: a transition rule is what declares which facts are edges at all, so a
/// bake built against a rules file cargo is not watching does not go slightly
/// stale, it silently bakes a DIFFERENT map. `mnemosyne_ops::transition_map_inputs`
/// resolves it through the same path resolution the read uses, so the file
/// declared and the file opened cannot drift (the R772 rule).
///
/// # Why no telling and no order
///
/// The map is neither disclosure-scoped nor canon-ordered — the gate evaluates
/// it flat, and R875's read takes neither for that reason. Accepting one here
/// would name a narrowing this artifact does not perform.
///
/// # Errors
///
/// [`EngineError::Projection`] if the map read fails — an unreadable store, an
/// unresolvable rules artifact, a rule naming an unregistered predicate, or a
/// side-table entry the store-registry boundary rejects. Each is a BUILD
/// failure; the consumer's runtime never sees them.
pub fn emit_map_projection(
    workspace_root: &Path,
    rules_override: Option<&mnemosyne_engine::AbsolutePath>,
) -> Result<String, EngineError> {
    for input in mnemosyne_engine::transition_map_inputs(workspace_root, rules_override)? {
        println!("cargo:rerun-if-changed={}", input.display());
    }
    let projection = MapProjection::from_workspace(workspace_root, rules_override)?;
    Ok(render_map(&projection.to_parts()))
}

/// Project the workspace's authored PASSAGES at build time and return Rust source
/// defining `pub fn passages() -> &'static HashMap<String, Passage>` (Round 791)
/// — the third emitter, and the one that takes the last store read out of a
/// consumer's startup.
///
/// # What this replaces
///
/// [`store_passages`](mnemosyne_engine::store_passages), which parses the whole
/// sidecar on every launch. The first consumer measured that call at 38,913
/// allocations and 7.85 MB — 48% of everything left in its startup after it had
/// moved three other axes itself — and could not bake it: `Passage`'s only
/// constructors are crate-private, on purpose. So the baking has to happen here,
/// which is what they asked for.
///
/// # The door this opens, and why it is opened deliberately
///
/// The emitted source calls
/// [`passages_from_parts`](mnemosyne_engine::passages_from_parts), a public door
/// over a parts type with public fields — so a consumer CAN hand the kernel prose
/// the store never held. That is inherent to baking rather than a concession
/// made here: generated source is spliced into the consumer's own crate and can
/// hold no capability the consumer lacks. The playable axis has carried the same
/// opening since Round 769. **The contract is stated once**, in
/// [`mnemosyne_engine::baked_ingestion`]; Round 790 chose to say it plainly
/// rather than to build a token that would only look like a lock.
///
/// # Why no telling, and no overrides
///
/// Passages are anchored authored prose, not a disclosure of it: a
/// [`Passage`](mnemosyne_engine::Passage) is the same text under every telling,
/// because what a telling varies is which FACTS surface, not what the manuscript
/// says. Taking a telling here would emit an artifact whose name implies a
/// narrowing it does not perform — the Round 774 reason `emit_quest_projection`
/// takes no order override, on a different axis.
///
/// # Errors
///
/// [`EngineError::Projection`] if the content-excerpt read fails — an unreadable
/// store, or a sidecar the config does not resolve. A BUILD failure; the
/// consumer's runtime never sees it.
pub fn emit_passages(workspace_root: &Path) -> Result<String, EngineError> {
    declare_inputs(workspace_root)?;
    let passages = mnemosyne_engine::store_passages(workspace_root)?;
    Ok(render_passages(&mnemosyne_engine::passages_to_parts(
        &passages,
    )))
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

    use crate::render::fixture_sizes;

    /// Round 788 — the fixture sizes moved from `const`s to a resolved pair so the
    /// first consumer can run these gates at its own scale, and the resolution is
    /// tested HERE because `build.rs` `include!`s this module's source: the code
    /// the build script runs is the code this test compiles, with no second copy.
    ///
    /// The PAIR is the property, not either number. A consumer able to set the two
    /// ends apart could produce a fixture pair whose "same cost at four times the
    /// store" assertions compare sizes that are not four times apart — green while
    /// asserting nothing — so `small` is derived, and a big that cannot yield an
    /// exact quarter is refused rather than truncated.
    #[test]
    fn the_fixture_pair_stays_four_to_one_or_is_refused() {
        // Unset measures exactly what Round 780 and Round 782 measured.
        assert_eq!(fixture_sizes(None), Ok((200, 800)));

        // The consumer's own scale: 4,612 baked lines is the store that asked.
        assert_eq!(fixture_sizes(Some("4612")), Ok((1153, 4612)));
        assert_eq!(fixture_sizes(Some(" 4612 ")), Ok((1153, 4612)));

        // Every accepted pair IS four to one — the invariant, not a sample.
        for big in [4usize, 800, 4612, 40_000] {
            let (small, resolved) = fixture_sizes(Some(&big.to_string())).expect("a legal size");
            assert_eq!(resolved, big);
            assert_eq!(small * 4, resolved, "the pair drifted off four to one");
        }

        // Refused, each for its own reason, and the ratio one names BOTH
        // neighbours rather than leaving the caller to guess which way to move.
        assert!(fixture_sizes(Some("0")).is_err());
        assert!(fixture_sizes(Some("many")).is_err());
        let err = fixture_sizes(Some("4610")).expect_err("not a quarter of anything");
        assert!(err.contains("4608") && err.contains("4612"), "{err}");
    }

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
        // The THIRD artifact, and the one Round 781 said its own gate could not
        // stand in for: two composing does not prove N compose, and the only way
        // to learn was for a third emitter to exist. It does now, and it went in
        // here rather than beside here.
        include!(concat!(env!("OUT_DIR"), "/fixture_passages.rs"));
        // The FOURTH, the place axis, in the same module for the same reason.
        include!(concat!(env!("OUT_DIR"), "/fixture_map.rs"));
    }

    #[test]
    fn the_generated_source_rebuilds_the_projection_it_was_baked_from() {
        // Compiling was the hard part; this asserts the values SURVIVED, so a
        // generator that emitted syntactically valid but wrong code still fails.
        let proj = baked::playable_projection();
        assert_eq!(proj.telling(), "reader");
        assert_eq!(proj.walk("main").collect::<Vec<_>>(), ["sc-01"]);
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

        // Round 851 — the SECOND world-line through the same scene, which is
        // what the pool made possible to get wrong. `dark` carries the same two
        // disclosures in the opposite order, and its `f-a` differs from `main`'s
        // in `mode` alone. A generator that pooled per section and dropped the
        // per-world sequence hands back main's order; one that keyed the pool by
        // `fact_id` hands back main's mode. Both compile.
        let dark = proj.lines("dark", "sc-01");
        assert_eq!(
            dark.iter()
                .map(mnemosyne_engine::Line::fact_id)
                .collect::<Vec<_>>(),
            ["f-b", "f-a"],
            "the pooled world-line lost its own order"
        );
        assert_eq!(dark[0].text(), "plain");
        assert_eq!(dark[1].text(), nasty);
        assert_eq!(
            dark[1].mode(),
            DisclosureMode::State,
            "the pool collapsed two payloads of one fact into one"
        );
        assert_eq!(lines[0].mode(), DisclosureMode::Hint, "and the other way");
        // The shared entry really is shared: `f-b` is one pool position reached
        // from both worlds, so the two must agree on every field.
        assert_eq!(lines[1].to_part(), dark[0].to_part());

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

        // Round 787 — the knowledge axis survives the bake. Asserted HERE and not
        // only inside the kernel, for the reason Round 774 named: the engine's own
        // round trip never goes through this emitter, so a field the generator
        // silently drops looks identical to a field that is empty. `f-leg` is
        // offered and is NOT a line, so a bake that emitted the prose stream alone
        // could not fake it.
        assert!(
            proj.offers("main", "sc-01", "f-leg"),
            "the generator dropped the journal-offer axis"
        );
        assert!(!lines.iter().any(|l| l.fact_id() == "f-leg"));
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
            telling: "reader".into(),
            by_world: vec![(
                "main".into(),
                vec![(
                    "sc-01".into(),
                    vec![LinePart {
                        fact_id: "f-a".into(),
                        text: "a line".into(),
                        mode: DisclosureMode::State,
                        frame: String::new().into(),
                        entities: Vec::new().into(),
                        carrier: None,
                        typed_predicate: None,
                        typed_quantity: None,
                        quote: None,
                        count: None,
                    }],
                )],
            )],
            walks: vec![("main".into(), vec!["sc-01".into()])],
            titles: Vec::new(),
            cast: Vec::new(),
            forks: Vec::new(),
            divergent_endings: Vec::new(),
            interactivity: Interactivity::default(),
            choice_entity_refs: Vec::new(),
            ask_doors: vec![(
                "sc-01".into(),
                vec![DoorPart::Ask {
                    question: "물었다".to_string(),
                    reveals: "f-a".to_string(),
                }],
            )],
            journal_offers: vec![("main".into(), vec![("sc-01".into(), vec!["f-leg".into()])])],
        };
        let proj = PlayableProjection::from_parts(parts);
        assert_eq!(proj.lines("main", "sc-01").len(), 1);
        // Round 787 — the knowledge axis is constructible from outside the kernel
        // too: plain `String`s, so this arm stays a compile-time statement about
        // the parts type rather than a value check that happens to pass.
        assert!(proj.offers("main", "sc-01", "f-leg"));
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

    #[test]
    fn the_generated_map_source_rebuilds_the_projection_it_was_baked_from() {
        // Compiling was the hard part; this asserts the VALUES survived. The
        // fields that matter most here are the two side-table ones, because
        // dropping them is not a hypothetical failure mode — it is the exact
        // shape of the gap that sent a live consumer to parse our sidecar.
        let proj = baked::map_projection();
        assert_eq!(proj.transition_rules(), 2);
        assert_eq!(proj.maps().len(), 2);
        assert_eq!(proj.unattached_costs(), ["f-stray-cost".to_string()]);
        assert_eq!(proj.unattached_guards(), ["f-stray-guard".to_string()]);

        let town = proj.map("town").expect("the map is baked");
        assert!(!town.undirected);
        assert_eq!(town.containment.as_deref(), Some("inside"));
        assert_eq!(town.self_loops[0].fact_id, "f-adj-a-a");

        // The nasty string round-tripped through a Rust literal, in a place id.
        let nasty = "그는 \"셈\"이라 했다.\n뒤에 \\ 하나.";
        assert!(town.nodes.contains(&nasty.to_string()));
        assert_eq!(town.edges[1].to, nasty);

        // Both side tables, and `Option` in both states on each.
        let cost = town.edges[0].cost.as_ref().expect("the baked cost");
        assert_eq!((cost.n, cost.unit.as_str()), (10, "unit-minute"));
        assert!(town.edges[1].cost.is_none());
        let k_of_n = town.edges[0]
            .guard
            .as_ref()
            .expect("the baked K-of-N guard");
        assert_eq!(k_of_n.threshold, Some(1));
        assert_eq!(k_of_n.conditions.len(), 2);
        let and_guard = town.edges[1].guard.as_ref().expect("the baked AND guard");
        assert_eq!(and_guard.threshold, None, "K-of-N must not flatten");

        // The declared flag survived and still decides: this map is directed, so
        // the back leg is not walkable, and the second map's `undirected: true`
        // is carried rather than defaulted.
        assert_eq!(town.steps_from("loc-a").len(), 1);
        assert!(town.steps_from("loc-b").iter().all(|(_, to)| *to == nasty));
        assert!(
            proj.map("tunnels").expect("the empty map").undirected,
            "the second map's declaration was baked, not defaulted"
        );
    }

    /// Feeds that accept ONLY a borrow living as long as the process. Free
    /// functions rather than `let _: &'static str = ..` bindings, because a
    /// parameter's lifetime is a requirement on the caller while a binding's can
    /// be satisfied by inference reading the annotation as the goal.
    fn keep_forever(_: &'static str) {}
    fn keep_slice_forever(_: &'static [String]) {}
    /// The same requirement for an accessor that hands out an ITERATOR rather
    /// than a slice (Round 795, `Line::entities`). Binding the item type to
    /// `&'static str` is the identical constraint on the caller: the sequence may
    /// be lazy, but what it yields still has to outlive the process.
    fn keep_items_forever(_: impl Iterator<Item = &'static str>) {}

    #[test]
    fn the_baked_artifacts_hand_out_process_lifetime_borrows() {
        // Round 786 — the round's whole property, and it is a claim about TYPES,
        // so the gate is that this compiles: revert the emitter to handing back a
        // value and `proj` is a local, its borrows die with the function, and
        // every line below is an error. There is no vacuous reading of a
        // lifetime constraint the way an empty `Vec<Violation>` reads as clean.
        //
        // The other half — that these feeds discriminate rather than accepting
        // anything — is the doctest pair on `emit_playable_projection`, where the
        // same accessor on an owned projection is required NOT to compile.
        let proj = baked::playable_projection();
        keep_forever(proj.telling());
        keep_forever(proj.title("sc-01").expect("the fixture titles sc-01"));
        keep_items_forever(proj.walk("main"));
        keep_items_forever(proj.spine());

        let line = &proj.lines("main", "sc-01")[0];
        keep_forever(line.text());
        keep_forever(line.fact_id());
        keep_forever(
            line.carrier()
                .expect("the fixture's first line has a carrier"),
        );
        keep_items_forever(line.entities());

        // The quest axis, repeated rather than inferred from the playable one —
        // the R774 discipline. Both emitters end in the same `artifact`, and that
        // is exactly the assumption worth not making: `render_quest` is free to
        // stop calling it while `render` still does.
        let quests = baked::quest_projection();
        keep_forever(quests.telling());
        let knot = quests.quest("q-knot-1").expect("the quest is baked");
        keep_forever(&knot.objective);
        keep_slice_forever(&knot.actors);
    }

    /// Parts carrying `n` lines in one section — a fixture that GROWS, for the
    /// scaling assertion below.
    fn parts_with_lines(n: usize) -> ProjectionParts {
        ProjectionParts {
            telling: "reader".into(),
            by_world: vec![(
                "main".into(),
                vec![(
                    "sc-01".into(),
                    (0..n)
                        .map(|i| LinePart {
                            fact_id: format!("f-{i:06}").into(),
                            text: "그는 \"셈\"이라 했다.".into(),
                            mode: DisclosureMode::State,
                            frame: "ground-truth".into(),
                            entities: vec!["ent-a".into()].into(),
                            carrier: None,
                            typed_predicate: None,
                            typed_quantity: None,
                            quote: None,
                            count: None,
                        })
                        .collect(),
                )],
            )],
            walks: vec![("main".into(), vec!["sc-01".into()])],
            titles: Vec::new(),
            cast: Vec::new(),
            forks: Vec::new(),
            divergent_endings: Vec::new(),
            interactivity: Interactivity::default(),
            choice_entity_refs: Vec::new(),
            ask_doors: Vec::new(),
            // This fixture varies LINE count; the journal axis is not what it
            // scales, so it carries none (Round 787).
            journal_offers: Vec::new(),
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
        //
        // Round 1046 — over EVERY baked artifact rather than the playable one.
        // This gate asserted the bound on one of the four emitters that call the
        // chunker, and inferred the rest, which is the assumption Round 774 wrote
        // down and Round 769 was the cost of: an emitter is free to stop calling
        // `chunked` while its siblings keep calling it. The population is
        // `Baked::ALL`, so the inference is now a measurement.
        let longest = |src: &str| src.lines().map(str::len).max().unwrap_or(0);
        let (small_n, big_n) = fixture_sizes(None).expect("the default pair");
        let mut wrong: Vec<String> = Vec::new();
        for baked in crate::render::Baked::ALL {
            let what = baked.tag();
            let small = baked.fixture(small_n);
            let big = baked.fixture(big_n);
            // A fixture that does not grow makes every claim below vacuous, so it
            // is checked per artifact rather than assumed from the playable one.
            if big.len() <= 3 * small.len() {
                wrong.push(format!(
                    "the {what} fixture must actually grow: {} bytes at {small_n} \
                     against {} at {big_n}",
                    small.len(),
                    big.len()
                ));
            }
            if longest(&big) >= 2 * longest(&small) {
                wrong.push(format!(
                    "a {what} function body grew with the store: {} -> {}",
                    longest(&small),
                    longest(&big)
                ));
            }
            // And the growth went where it was supposed to go.
            let more = |src: &str| src.matches("fn __mn_").count();
            if more(&big) <= more(&small) {
                wrong.push(format!(
                    "the {what} artifact grew without emitting more functions: {} \
                     -> {}",
                    more(&small),
                    more(&big)
                ));
            }
        }
        assert!(
            wrong.is_empty(),
            "{} claim(s) failed across {} baked artifact(s):\n{}",
            wrong.len(),
            crate::render::Baked::ALL.len(),
            wrong.join("\n")
        );
    }

    /// The same section, walked by `worlds` world-lines carrying `lines`
    /// disclosures each — the CROSS PRODUCT shape, as a fixture that can vary one
    /// factor at a time.
    fn parts_walked_by(worlds: usize, lines: usize) -> ProjectionParts {
        let section = |_| {
            (
                "sc-01".into(),
                (0..lines)
                    .map(|i| LinePart {
                        fact_id: format!("f-{i:06}").into(),
                        text: "그는 \"셈\"이라 했다.".into(),
                        mode: DisclosureMode::State,
                        frame: "ground-truth".into(),
                        entities: vec!["ent-a".into()].into(),
                        carrier: None,
                        typed_predicate: None,
                        typed_quantity: None,
                        quote: None,
                        count: None,
                    })
                    .collect(),
            )
        };
        ProjectionParts {
            by_world: (0..worlds)
                .map(|w| (format!("w-{w:03}").into(), vec![section(w)]))
                .collect(),
            ..parts_with_lines(0)
        }
    }

    #[test]
    fn a_disclosure_many_worlds_walk_is_emitted_once() {
        // Round 851 — the round's claim, and the one the round-trip test above
        // cannot make: that test proves the lines come back RIGHT, which a
        // generator emitting every copy also does. This proves they are not
        // copied.
        //
        // The payload is what does not repeat; the positions do. So the arms vary
        // the world count with the store fixed, and the assertion is on what the
        // emitted file HOLDS rather than on its size.
        let ctors = |src: &str| src.matches("::mnemosyne_engine::LinePart {").count();
        let one = crate::render(&parts_walked_by(1, 100));
        let many = crate::render(&parts_walked_by(20, 100));

        // Non-vacuity first: the fixture really does walk the scene twenty times,
        // so 100 constructors is a collapse and not an empty world-line list.
        let walked: usize = parts_walked_by(20, 100)
            .by_world
            .iter()
            .map(|(_, sections)| sections.iter().map(|(_, l)| l.len()).sum::<usize>())
            .sum();
        assert_eq!(walked, 2_000, "the fixture stopped repeating");

        assert_eq!(ctors(&one), 100);
        assert_eq!(
            ctors(&many),
            100,
            "twenty world-lines emitted {} copies of 100 disclosures",
            ctors(&many)
        );
        assert!(
            many.contains("__mn_lines("),
            "the world-lines carry payloads rather than pool positions"
        );

        // And what a twentyfold walk costs is the positions alone. Before this
        // round `many` was twenty times `one`; the surviving growth is one
        // integer per walked line.
        assert!(
            many.len() < 2 * one.len(),
            "the file grew with the cross product: {} -> {}",
            one.len(),
            many.len()
        );
    }

    #[test]
    fn the_pool_comes_out_in_the_order_the_worlds_are_walked() {
        // Round 852 — Round 851 put this generator's first hash map into it, and
        // the crate's oldest promise is that an unchanged store emits a
        // byte-identical file. `HashMap` iteration is seeded PER PROCESS, so a
        // pool emitted in map order is stable within one run and different
        // between two: every other gate here would stay green while a consumer
        // rebuilt on every build. That consumer had just stopped rebuilding on
        // unchanged input by hashing exactly this file's bytes, so the failure
        // would land on the fix it had shipped the same day.
        //
        // The separation that makes it safe is internal — `rendered` is a `Vec`
        // and owns the order, the map is only a lookup — and this asserts it
        // from OUTSIDE, where a later round that reaches for the map cannot
        // quietly be right.
        let at = |id: &str| LinePart {
            fact_id: id.to_string().into(),
            text: "그는 \"셈\"이라 했다.".into(),
            mode: DisclosureMode::State,
            frame: "ground-truth".into(),
            entities: Vec::new().into(),
            carrier: None,
            typed_predicate: None,
            typed_quantity: None,
            quote: None,
            count: None,
        };
        let world = |name: &str, ids: &[&str]| {
            (
                name.to_string().into(),
                vec![("sc-01".into(), ids.iter().map(|id| at(id)).collect())],
            )
        };
        // Walked in sorted world order, and the ids are chosen so first-seen
        // order is neither sorted nor reverse-sorted: a `BTreeMap` reached for
        // instead of the `Vec` would fail this too, not only a `HashMap`.
        let parts = ProjectionParts {
            by_world: vec![
                world("w-a", &["f-07", "f-02", "f-11", "f-02"]),
                world("w-b", &["f-11", "f-05", "f-00"]),
                world("w-c", &["f-02", "f-09", "f-05", "f-13"]),
            ],
            ..parts_with_lines(0)
        };
        let expected = ["f-07", "f-02", "f-11", "f-05", "f-00", "f-09", "f-13"];

        // Only a POOL entry constructs a `LinePart` — a world-line carries
        // positions — so the fact ids in the file, in file order, are the pool.
        let src = crate::render(&parts);
        let emitted: Vec<&str> = src
            .match_indices("::mnemosyne_engine::LinePart { fact_id: ")
            .map(|(i, m)| {
                let rest = &src[i + m.len()..];
                let open = rest.find('"').expect("a quoted fact id");
                let close = rest[open + 1..].find('"').expect("a closed fact id");
                &rest[open + 1..open + 1 + close]
            })
            .collect();
        assert_eq!(
            emitted, expected,
            "the pool came out in some order other than the walk's"
        );
    }

    /// Round 881 — the sibling of the pool gate above, for the OTHER hash-ordered
    /// container this generator serializes.
    ///
    /// `Interactivity` is the only `Serialize` type in the workspace holding a
    /// `HashMap`/`HashSet`; `render::interactivity` sorts both and its doc comment
    /// claims byte-identical source follows. That claim had NO DISCRIMINATING
    /// FIXTURE: the one non-empty `Interactivity` the renderer ever saw carried a
    /// single-entry map, where sorting is a no-op, so deleting BOTH sorts left the
    /// entire workspace green — measured, not assumed. This is the outside
    /// assertion R852 gave the pool, given to the leg it did not reach.
    #[test]
    fn the_interactive_layer_comes_out_in_sorted_order() {
        use std::collections::{HashMap, HashSet};
        let rung = |q: &str| {
            vec![mnemosyne_engine::Rung {
                question: q.to_string(),
                question_anchor: None,
                reveals: "f-x".to_string(),
                needs: Vec::new(),
            }]
        };
        // FIVE entries per leg, inserted in an order that is neither sorted nor
        // reversed. A dropped sort then escapes only if hash order happens to be
        // the sorted one — 1 chance in 120 per leg, where three entries would
        // have left 1 in 6. The gate is a probability, and this is the dial.
        let ids = ["e", "a", "d", "b", "c"];
        let parts = ProjectionParts {
            interactivity: Interactivity {
                ladders: HashMap::from_iter(
                    ids.iter()
                        .map(|i| (format!("{i}-ladder"), rung(&format!("q-{i}")))),
                ),
                objects: HashSet::from_iter(ids.iter().map(|i| format!("{i}-object"))),
                free_investigate: false,
            },
            ..parts_with_lines(0)
        };
        let src = crate::render(&parts);
        // The emitted order, read off the generated source by position, compared
        // against sorted — the pool gate's shape, not a chain of pairwise checks.
        let emitted = |suffix: &str| -> Vec<String> {
            let mut seen: Vec<(usize, String)> = ids
                .iter()
                .map(|i| {
                    let id = format!("{i}-{suffix}");
                    let at = src
                        .find(&id)
                        .unwrap_or_else(|| panic!("`{id}` never reached the generated source"));
                    (at, id)
                })
                .collect();
            seen.sort_by_key(|(at, _)| *at);
            seen.into_iter().map(|(_, id)| id).collect()
        };
        let sorted = |suffix: &str| -> Vec<String> {
            let mut v: Vec<String> = ids.iter().map(|i| format!("{i}-{suffix}")).collect();
            v.sort();
            v
        };
        for suffix in ["ladder", "object"] {
            assert_eq!(
                emitted(suffix),
                sorted(suffix),
                "the interactive layer's {suffix}s came out in some order other \
                 than sorted, so an unchanged store no longer emits a \
                 byte-identical file"
            );
        }
    }

    #[test]
    fn the_artifacts_define_the_same_private_names_and_compose_anyway() {
        // Round 781. `mod baked` above splices both artifacts into one module, and
        // that is a COMPILE-time claim — which is also what two empty files would
        // satisfy. This is the arm that makes it non-vacuous, and it is here rather
        // than in a file of its own for the Round 780 reason: split apart, the
        // composition could report green while the hazard it absorbs had quietly
        // stopped being present.
        // Round 791 — THREE now, which is the thing Round 781's carry said this
        // gate could not stand in for: two artifacts composing does not prove N
        // do, and the only way to learn was for a third emitter to exist. It
        // does, it went into the same `mod baked`, and the wall held with no
        // change to it — the derivation was right rather than lucky.
        //
        // Round 1046 — FOUR, and not by adding a fourth name here. Three was a
        // hand list, the map emitter existed for rounds without appearing in it,
        // and a list cannot report the member nobody wrote into it. The
        // population is `Baked::ALL`, the same one the stack gate weighs, so this
        // grows with the emitters rather than with whoever remembers.
        let sources: Vec<(&str, String)> = crate::render::Baked::ALL
            .iter()
            .map(|baked| (baked.tag(), baked.fixture(1)))
            .collect();

        // The hazard: the counter restarts per render, so all of them really do
        // define `__mn_0`. If any stopped, `mod baked` would prove less than it
        // looks like it proves.
        for (what, src) in &sources {
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
        // Pairwise across the whole population, not just the first two: a wall
        // that separated playable from quest and collided with a third would be
        // the same defect one artifact later, and checking only the original pair
        // is how a gate stops growing with what it guards.
        let walls: Vec<(&str, String)> = sources
            .iter()
            .map(|(what, src)| (*what, wall(src)))
            .collect();
        for (i, (a_name, a)) in walls.iter().enumerate() {
            for (b_name, b) in walls.iter().skip(i + 1) {
                assert_ne!(
                    a, b,
                    "the {a_name} and {b_name} artifacts wall their identical \
                     private names behind the SAME module name, so the wall does \
                     not separate them"
                );
            }
        }
    }

    /// The passage fixture, as a downstream crate must be able to build it.
    fn parts_with_passages() -> mnemosyne_engine::PassagesParts {
        mnemosyne_engine::PassagesParts {
            passages: vec![(
                "sc-01".to_string(),
                mnemosyne_engine::PassagePart {
                    anchor: mnemosyne_engine::ContentAnchor {
                        source: "M.md".to_string(),
                        locator: mnemosyne_engine::Locator::Prefix("이름을".to_string()),
                    },
                    text: "그는 \"셈\"이라 했다.".into(),
                },
            )],
        }
    }

    /// Round 791 — the passage axis's round trip, asserted HERE rather than
    /// inferred from the other two.
    ///
    /// The three emitters share one `artifact` assembler, and that shared ending
    /// is exactly the assumption not worth making: Round 774 repeated this test
    /// on the quest axis for the same reason, and Round 787 found a field the
    /// generator could silently drop while the kernel's own round trip stayed
    /// green. The engine's tests never go through this emitter.
    #[test]
    fn the_generated_passage_source_rebuilds_the_set_it_was_baked_from() {
        let passages = baked::passages();
        assert_eq!(passages.len(), 2);

        // The nasty string survived a Rust literal — quote, newline, backslash,
        // non-ASCII — and so did BOTH locator kinds, which is what a passage
        // carries that a line does not.
        let nasty = "그는 \"셈\"이라 했다.\n뒤에 \\ 하나.";
        let one = passages.get("sc-01").expect("the prefix-anchored passage");
        assert_eq!(one.text(), nasty);
        assert_eq!(one.anchor().source, "M.md");
        assert_eq!(
            one.anchor().locator,
            mnemosyne_engine::Locator::Prefix("이름을".to_string())
        );

        let two = passages.get("sc-02").expect("the cfi-anchored passage");
        assert_eq!(two.text(), "plain");
        assert_eq!(
            two.anchor().locator,
            mnemosyne_engine::Locator::Cfi("/6/4[c]!/4/2".to_string())
        );

        // The Round 786 property on this axis: the entry point hands back a
        // borrow that outlives the call, so a consumer holds it rather than
        // copying out of it. A value-returning emitter would not compile here.
        fn keep_forever(_: &'static mnemosyne_engine::Passage) {}
        keep_forever(one);
    }

    /// Round 791 — the passage parts must be constructible from OUTSIDE the
    /// kernel, repeated on this axis for the reason the other two repeat it: the
    /// engine's own tests are inside the crate where every private field is
    /// reachable, so only a downstream crate can find a parts type that readmits
    /// one. This is also the test that demonstrates the ingestion door described
    /// in `mnemosyne_engine::baked_ingestion` — the text below is invented, and
    /// the kernel takes it.
    #[test]
    fn passage_parts_are_constructible_from_a_downstream_crate() {
        let built = mnemosyne_engine::passages_from_parts(parts_with_passages());
        assert_eq!(built["sc-01"].text(), "그는 \"셈\"이라 했다.");

        // And the round trip is closed the other way: what the kernel emits from
        // a passage set is what the kernel ingests, so `to_part` and the door
        // cannot drift into two shapes of the same datum.
        assert_eq!(
            mnemosyne_engine::passages_to_parts(&built),
            parts_with_passages()
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
                        outstanding_givings: Vec::new(),
                    },
                )],
                preconditions: Vec::new(),
            }],
        };
        let proj = QuestProjection::from_parts(parts);
        assert_eq!(proj.quests().len(), 1);
    }
}
