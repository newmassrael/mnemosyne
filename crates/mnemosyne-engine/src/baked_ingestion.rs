//! The contract every baked-ingestion door carries — stated ONCE (Round 791).
//!
//! This module holds no code. It exists because the kernel has three public
//! doors that turn plain data into guarded kernel values —
//! [`PlayableProjection::from_parts`](crate::PlayableProjection::from_parts),
//! [`QuestProjection::from_parts`](crate::QuestProjection::from_parts), and
//! [`passages_from_parts`](crate::passages_from_parts) — and one contract that
//! governs all three. Written out at each door, that contract would be three
//! texts free to drift, and the first two already drifted into saying something
//! that is not true. A fourth door must link here rather than restate this.
//!
//! # What a baked door is
//!
//! A build-time bake reads the store where the store is, resolves everything
//! resolvable there, and emits Rust source. At run time the consumer's binary
//! holds data its own compiler type-checked, and opens no store. That is the
//! Round 764 shape, and it is why these doors return no `Result`: the checks
//! were not deleted, they were discharged at build time.
//!
//! # What a baked door costs, exactly
//!
//! **Every bake door is a fabrication door, and this cannot be engineered
//! away.** Generated source is spliced into the CONSUMER's crate, so any
//! constructor the generated code can reach, a hand-written line in that crate
//! can reach too. The kernel cannot grant generated code a privilege the
//! consumer lacks. A sealed token, a checksum, a `#[doc(hidden)]` marker: each
//! narrows discoverability and none narrows capability, and a lock that only
//! looks like a lock is worse than a documented opening.
//!
//! So: `Line`, `Passage` and the rest keep crate-private fields and their
//! `compile_fail` doctests still hold — a struct literal and a mutated clone are
//! still compile errors. **Those doctests do not cover this path.** Through a
//! parts type, whose fields are public, a consumer can hand the kernel narrative
//! the store never held.
//!
//! # What the guard still buys
//!
//! Not nothing, and worth naming so the remaining value is not thrown away by a
//! later round that reads "the door is open" and stops caring:
//!
//! - Invention is unreachable through any READING path. Every accessor hands out
//!   what came in; none of them constructs.
//! - It cannot happen by accident. Fabrication is a parts literal a person
//!   wrote, visible in the consumer's own source and reviewable there.
//! - The sanctioned producers are the kernel's own emitters
//!   (`mnemosyne-engine-build`), which read the store and nothing else.
//!
//! # The correction this module records
//!
//! [`ProjectionParts`](crate::ProjectionParts) said its door needs no `Result`
//! because "there is no untrusted input left to check it against". That is true
//! of parts an emitter produced and false of parts a consumer typed, and the
//! sentence is why the opening went unnoticed from Round 769 until Round 790 —
//! including by a test that demonstrates it while proving something else
//! (`parts_are_constructible_from_a_downstream_crate` builds a `LinePart` with
//! arbitrary text outside the kernel and ingests it).
//!
//! The honest form is above: the input is untrusted, there is nothing left to
//! check it AGAINST at run time, and that is the trade baking makes.
