//! mnemosyne-engine-build — bake a playable projection at BUILD time (Round 770).
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

use std::path::Path;

use mnemosyne_engine::{EngineError, EngineOverrides, PlayableProjection};

mod render;
pub use render::render;

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
    };

    /// The generator's own output, compiled INTO this crate by `build.rs`. That
    /// this module exists at all is the gate: if `render` emitted Rust that does
    /// not compile, or named a kernel type that changed shape, the build fails
    /// here — the same failure a consumer would get, caught in this workspace.
    mod baked {
        include!(concat!(env!("OUT_DIR"), "/fixture_playable.rs"));
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
        let doors = proj.scene("main", "sc-01").doors;
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
}
