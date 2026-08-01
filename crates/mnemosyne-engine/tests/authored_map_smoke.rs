//! The place axis on authored data — the kernel door over a blind author's map.
//!
//! Round 875 built the map READ and closed with an honest confession: "Not
//! dogfoodable here: our own store declares no transition rule, so every number
//! in this round is from a fixture or the consumer's tree." That was true when
//! it was written. It stopped being true when the map-corpus arm-D run landed a
//! blind-authored town with a declared map, and this file is what makes the
//! change count — a corpus nothing loads is a corpus that rots, which is the
//! whole lesson of Round 873.
//!
//! The store is REBUILT from the tracked manifests where they already live,
//! rather than copied into a fixtures directory beside this file (the rebuild
//! is `corpus::rebuild`, shared with the claim-axis pin since Round 940).
//! Reading the evidence in place means this test reddens if the frozen record is
//! ever mutated.
//!
//! WHAT THIS CANNOT PIN, stated here rather than left to be inferred from a
//! green run: five blind-authored maps are on record and every one of them is
//! `directed`, and not one uses `edge_costs` or `edge_guards`. So the undirected
//! branch and both side tables have NO witness in authored data, and their
//! oracles are the in-crate unit tests over built fixtures. What this file pins
//! is the half real authors actually wrote.

mod corpus;

use corpus::rebuild;
use mnemosyne_engine::MapProjection;

/// The numbers are the ones the run recorded (`transition-map.log`, arm D stage
/// B), so a change here is a change to a published measurement.
#[test]
fn the_kernel_reads_a_blind_authors_declared_map() {
    let tmp = rebuild();
    let projection = MapProjection::from_workspace(tmp.path(), None).expect("the map projects");

    assert_eq!(
        projection.transition_rules(),
        1,
        "the author declared one transition rule"
    );
    let map = projection
        .map("cairnwell-map")
        .expect("the rule the author named");
    assert_eq!(map.adjacency, "adjacent");
    // The rule's OWN predicate — what says where a subject is, as opposed to
    // which places are joined. Without it a consumer holding this report can see
    // the roads and cannot ask who stands on one, and the scene-placing join is
    // impossible. The author declared it; the read must carry it.
    assert_eq!(
        map.predicate, "at",
        "the location predicate the transition rule declares"
    );
    assert_ne!(
        map.predicate, map.adjacency,
        "the two declared names are different questions and must not collapse"
    );
    assert_eq!(map.nodes.len(), 12, "12 places");
    assert_eq!(map.edges.len(), 20, "20 roads");
    assert!(
        !map.undirected,
        "the author declared a DIRECTED map and wrote both ways where both ways exist"
    );

    // Provenance: every road names the fact that declares it. This is the
    // property that makes an invented road unrepresentable here, so it is
    // asserted over the whole set rather than sampled.
    assert!(
        map.edges.iter().all(|e| !e.fact_id.is_empty()),
        "every edge carries its declaring fact"
    );
    assert!(
        map.edges
            .iter()
            .any(|e| e.fact_id == "f-adj-stair-landing" && e.from == "loc-cairn-stair"),
        "the recorded stair-to-landing road is present with its recorded fact"
    );

    // The half no author exercised, asserted as ABSENT rather than left
    // unmentioned — a reader of a green run should not conclude these were
    // proven present.
    assert!(
        map.edges.iter().all(|e| e.cost.is_none()),
        "no blind author reached for edge_costs"
    );
    assert!(
        map.edges.iter().all(|e| e.guard.is_none()),
        "nor for edge_guards"
    );
    assert!(projection.unattached_costs().is_empty());
    assert!(projection.unattached_guards().is_empty());
}

/// The navigational question the place axis exists to answer, asked of authored
/// data. The discriminating half is the descent the brief asked for — one way
/// that can be taken down and not climbed back up — which on a directed map is
/// a road that appears from one endpoint and not the other.
#[test]
fn steps_from_answers_where_a_traveller_can_go_next_in_the_authored_town() {
    let tmp = rebuild();
    let projection = MapProjection::from_workspace(tmp.path(), None).expect("the map projects");
    let map = projection.map("cairnwell-map").expect("the map");

    let mut from_market: Vec<&str> = map
        .steps_from("loc-market-square")
        .into_iter()
        .map(|(_, to)| to)
        .collect();
    from_market.sort_unstable();
    assert_eq!(
        from_market,
        [
            "loc-cairn-stair",
            "loc-highwater-quarter",
            "loc-shrine-ford"
        ],
        "the market's three roads, as the store declares them"
    );

    // The one-way descent: reachable forward, and NOT walkable back. On this
    // directed map `steps_from` must not invent the return leg, and the recorded
    // edge list has no fact declaring one.
    let down: Vec<&str> = map
        .steps_from("loc-boat-landing")
        .into_iter()
        .map(|(_, to)| to)
        .collect();
    assert_eq!(down, ["loc-drowned-quarter"]);
    assert!(
        map.steps_from("loc-eel-quay")
            .iter()
            .all(|(_, to)| *to != "loc-drowned-quarter"),
        "no road is invented into the drowned quarter from the quay"
    );

    // A place every road avoids still answers, and answers empty rather than
    // panicking or guessing.
    assert!(
        map.steps_from("loc-nowhere-at-all").is_empty(),
        "an unknown place has no roads, and asking is not an error"
    );
}

/// A baked map must be regenerated when the rules file changes, so the rules
/// file has to be among the inputs the build declares to cargo.
///
/// This is the sharpest instance of the divergence Round 772 closed: the other
/// projections do not open a rules artifact, so `projection_inputs` never listed
/// one, and a map bake that watched only what they watch would keep serving the
/// map a DELETED transition rule used to produce. Not "slightly stale" — a rule
/// is what declares which facts are edges at all, so the answer changes from a
/// town to no map whatsoever while cargo sees nothing to redo.
///
/// The corpus supplies the discriminating config: arm D's `mnemosyne.toml` pins
/// `rules_path = "rules.json"`, so the file is named by policy rather than by an
/// override this test could have invented.
#[test]
fn a_map_bake_declares_the_rules_file_it_reads() {
    let tmp = rebuild();
    let inputs = mnemosyne_engine::transition_map_inputs(tmp.path(), None)
        .expect("the declared inputs resolve");

    let rules = tmp.path().join("rules.json");
    assert!(
        inputs.contains(&rules),
        "the rules artifact the read opens is not among the declared inputs: {inputs:?}"
    );
    // And the inputs its siblings already declared are still there, so the
    // addition is an addition rather than a replacement.
    assert!(inputs.contains(&tmp.path().join("mnemosyne.toml")));
    assert!(inputs.contains(&tmp.path().join("order.json")));
    assert!(
        inputs.iter().any(|p| p.ends_with("workspace.atomic.json")),
        "the sidecar is still declared: {inputs:?}"
    );

    // An explicit override is what gets READ when one is passed, so it is what
    // must get DECLARED — the two decisions come from one resolver.
    let overridden = mnemosyne_engine::transition_map_inputs(tmp.path(), Some("other-rules.json"))
        .expect("the declared inputs resolve under an override");
    assert!(overridden.contains(&tmp.path().join("other-rules.json")));
    assert!(
        !overridden.contains(&rules),
        "the policy's rules file is not what an overridden read opens: {overridden:?}"
    );
}
