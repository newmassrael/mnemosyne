//! The claim axis on authored data — the kernel door over a blind author's
//! typed claims.
//!
//! Round 939 counted the kernel's read surface against the store's write surface
//! and found two runtime-facing paths where a live consumer hand-parses our
//! sidecar because the kernel hands nothing back. This file pins the closure of
//! one of them: the typed-claim query the consumer's `bake_viewpoint` reimplements
//! in `serde_json`, and which the kernel already held privately in its quest axis.
//!
//! The corpus is the same blind-authored town the place axis reads
//! (`corpus::rebuild`), for the reason that made that pin worth having: the
//! oracle is data a real author wrote against `describe-schema` alone, not a
//! fixture written by the same hand as the code.
//!
//! WHAT THIS CANNOT PIN, stated rather than left to be inferred from a green run:
//!
//! - The BRANCH coordinate has no witness here. Every fact in this corpus is on
//!   the trunk, so the row's `branch` is `main` throughout. The authored witness
//!   for branch spread is in the first consumer's own store, measured read-only
//!   in Round 939: one character's `pursues` claims sit on three different
//!   branches, which is exactly the shape a branch-blind read would flatten.
//! - The `Fact` object shape (`opened_by = f-*`, the quest-precondition bridge)
//!   is not in this corpus either — no tracked corpus authors it. Its oracle is
//!   the built-store test beside this one.

mod corpus;

use std::collections::BTreeSet;

use corpus::rebuild;
use mnemosyne_engine::{store_typed_claims, TypedObject};

/// The counts are the corpus's own, so a change here is a change to the frozen
/// record rather than to this file's taste.
#[test]
fn the_kernel_reads_a_blind_authors_typed_claims() {
    let tmp = rebuild();
    let claims = store_typed_claims(tmp.path()).expect("the kernel reads the claims");

    // Six predicates carry claims; the author declared them and nothing else.
    let predicates: Vec<&str> = claims.keys().map(String::as_str).collect();
    assert_eq!(
        predicates,
        [
            "adjacent",
            "at",
            "contains",
            "flood_depth",
            "holds",
            "state"
        ],
        "the predicates the author actually typed, in one deterministic order"
    );
    assert_eq!(claims["adjacent"].len(), 20, "20 roads");
    assert_eq!(claims["at"].len(), 23, "23 standings");
    assert_eq!(claims["contains"].len(), 6);
    assert_eq!(claims["flood_depth"].len(), 2);
    assert_eq!(claims["holds"].len(), 3);
    assert_eq!(claims["state"].len(), 9);
    assert_eq!(
        claims.values().map(Vec::len).sum::<usize>(),
        63,
        "63 typed claims — the corpus has 75 facts, and an untyped fact is \
         absent here rather than present with an empty leg"
    );

    // Provenance: every row names the fact that declares it, so a claim with no
    // fact behind it is unrepresentable.
    assert!(claims
        .values()
        .flatten()
        .all(|row| row.fact_id.starts_with("f-")));

    // Rows are ordered by fact id, which is what makes a bake reading this
    // byte-stable across runs.
    for rows in claims.values() {
        let ids: Vec<&str> = rows.iter().map(|r| r.fact_id.as_str()).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted, "rows are fact-id ordered");
    }
}

/// THE SHARP HALF — a belief-frame claim must not read as ground truth.
///
/// This corpus holds exactly one typed claim outside `ground-truth`: the town
/// says the drowned bellhouse rings itself at slack water, and the author typed
/// that rumour as a `state` claim in the `townsfolk` frame. A read that dropped
/// the frame would hand a consumer "the bell IS ringing" with the same authority
/// as "the market square is adjacent to the stair", which is the defect Round 922
/// closed on the map axis arriving on the claim axis.
#[test]
fn a_rumour_arrives_marked_as_a_rumour() {
    let tmp = rebuild();
    let claims = store_typed_claims(tmp.path()).expect("the kernel reads the claims");

    let held_in: BTreeSet<&str> = claims
        .values()
        .flatten()
        .map(|row| row.frame.as_str())
        .collect();
    assert_eq!(
        held_in,
        BTreeSet::from(["ground-truth", "townsfolk"]),
        "the frames the authored claims are held in"
    );

    let rumours: Vec<&str> = claims
        .values()
        .flatten()
        .filter(|row| row.frame != "ground-truth")
        .map(|row| row.fact_id.as_str())
        .collect();
    assert_eq!(
        rumours,
        ["f-rumour-bell"],
        "the one claim the town holds and the world does not"
    );

    // And it is NOT hidden from the read — it arrives, carrying the coordinate
    // that says whose claim it is. Withholding it would be the opposite defect:
    // the kernel deciding which frames a consumer may see.
    let bell = claims["state"]
        .iter()
        .find(|row| row.fact_id == "f-rumour-bell")
        .expect("the rumour is carried, not filtered away");
    assert_eq!(bell.subject, "loc-drowned-bell");
    assert_eq!(bell.frame, "townsfolk");
    assert_eq!(
        bell.object,
        TypedObject::Token {
            token: "ringing".to_string()
        },
        "the object keeps its shape — a token, not a rendered string"
    );

    // The same predicate carries ground-truth claims too, so the frame is what
    // separates them and not the predicate. Without this the test above would
    // pass on a store where `state` meant "rumour".
    assert!(
        claims["state"]
            .iter()
            .filter(|row| row.frame == "ground-truth")
            .count()
            >= 1,
        "`state` is not a rumour-only predicate"
    );
}

/// The consumer's own question, asked of authored data: the subjects of the
/// facts carrying one predicate. This is the shape `bake_viewpoint` hand-parses
/// the sidecar for (`pred-plays` -> the one player), reproduced through the door.
///
/// The corpus has no `pred-plays` — a viewpoint is that consumer's story rule —
/// so the question is asked of `at`, which the same corpus authored 23 times.
#[test]
fn the_subjects_of_one_predicate_are_a_map_lookup_not_a_scan() {
    let tmp = rebuild();
    let claims = store_typed_claims(tmp.path()).expect("the kernel reads the claims");

    let standing: BTreeSet<&str> = claims["at"]
        .iter()
        .map(|row| row.subject.as_str())
        .collect();
    assert_eq!(
        standing,
        BTreeSet::from(["ch-crake", "ch-hask", "ch-mirren", "ch-ordel", "ch-veil"]),
        "the five townsfolk the author ever placed somewhere"
    );

    // Every `at` object is an entity — the place stood in. A consumer joining
    // this against the map's node set needs the shape, not a string it has to
    // re-parse.
    assert!(claims["at"]
        .iter()
        .all(|row| matches!(&row.object, TypedObject::Entity { id } if id.to_string().starts_with("loc-"))));

    // A predicate nobody authored is ABSENT, and asking is not an error — the
    // caller sees the store was read and holds no such claim, which is the
    // distinction a filtered read cannot make.
    assert!(!claims.contains_key("pred-plays"));
}
