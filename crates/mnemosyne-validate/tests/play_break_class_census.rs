//! What stands between an authoring slip and the runtime (Round 1033).
//!
//! Round 1031 built a gate for ONE play-breaking class and chose that class by
//! reading a doc comment. That is the "the only one left is X" habit this repo
//! has caught itself in four rounds running: a hand-list read as a measurement.
//! This file replaces the hand-list with a walk.
//!
//! The population is DERIVED, not enumerated: the quest layer of the one
//! blind-authored branching corpus (`structural_fact_ids`, the shipping
//! definition of quest plumbing, plus the facts that pay it off), and per fact
//! the legs it ACTUALLY has. Each leg yields one corruption an author could
//! plausibly commit — a retargeted claim, a dropped payoff edge, a scene one
//! step late — applied to the MANIFEST and pushed through the real write path.
//!
//! Every corruption is then run past three stations and lands in exactly one
//! bucket:
//!
//! - `REFUSED` — the write path rejects it. The class cannot be authored at all.
//! - `CAUGHT` — the continuity gate reports it, or a projection fails loud.
//! - `SILENT` — gates green, and a projection a runtime reads CHANGES. Nothing
//!   but the author's attention stands here. This is the arc's backlog, and it
//!   is produced by this walk rather than by anyone's memory.
//! - `INERT` — gates green and both projections byte-identical. The store holds
//!   a datum no projection carries, so no runtime can be broken by it — and
//!   equally, no consumer can see it.
//!
//! The census is pinned. A new leg kind, a new quest fact, or a gate that starts
//! or stops seeing a class all move it, which is the point: the number is the
//! arc's remaining surface, and it is re-derived on every run.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use mnemosyne_atomic::{AtomicStore, FactsManifest, SectionImport};
use mnemosyne_validate::continuity::{
    load_canon_order, playable_world, quest_graph, scan_continuity, structural_fact_ids, CanonOrder,
};

/// The telling the dnd-quest author wrote its disclosure plan under.
const TELLING: &str = "delve";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repo root is two levels above the crate manifest")
        .to_path_buf()
}

fn read_json(path: PathBuf) -> serde_json::Value {
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("fixture {} unreadable: {e}", path.display()));
    serde_json::from_str(&text).expect("fixture json")
}

fn sections_manifest() -> Vec<SectionImport> {
    serde_json::from_value(read_json(
        repo_root().join("claudedocs/phase1-dnd-quest-experiment/v3/run/author/sections.json"),
    ))
    .expect("sections manifest")
}

fn facts_manifest_json() -> serde_json::Value {
    read_json(repo_root().join("crates/mnemosyne-cli/tests/fixtures/dnd-quest/facts.json"))
}

fn canon_order(store: &AtomicStore) -> CanonOrder {
    let decl = load_canon_order(
        &repo_root().join("claudedocs/phase1-dnd-quest-experiment/v3/run/author/order.json"),
        None,
    )
    .expect("canon order");
    CanonOrder::from_declaration(&decl, &store.branches).expect("compose order")
}

/// Build a store through the REAL write path. `Err` = the manifest was refused,
/// which is a verdict, not a test failure.
fn build(facts: &serde_json::Value, sidecar: &Path) -> Result<AtomicStore, String> {
    let manifest: FactsManifest =
        serde_json::from_value(facts.clone()).map_err(|e| format!("manifest shape: {e}"))?;
    let mut store = AtomicStore::new();
    mnemosyne_atomic::import_sections(&mut store, sidecar, &sections_manifest())
        .map_err(|e| format!("import_sections: {e}"))?;
    mnemosyne_atomic::import_facts(&mut store, sidecar, &manifest)
        .map_err(|e| format!("import_facts: {e}"))?;
    Ok(store)
}

/// The two projections a runtime reads, serialized — the thing a corruption
/// either changes or does not.
fn projections(store: &AtomicStore, order: &CanonOrder) -> Result<String, String> {
    let world = playable_world(store, order, None, TELLING)?;
    let quests = quest_graph(store, order, None, TELLING)?;
    Ok(format!(
        "{}\n{}",
        serde_json::to_string(&world).expect("world json"),
        serde_json::to_string(&quests).expect("quest json"),
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Bucket {
    Refused,
    Caught,
    Silent,
    Inert,
}

impl Bucket {
    fn as_str(self) -> &'static str {
        match self {
            Bucket::Refused => "REFUSED",
            Bucket::Caught => "CAUGHT",
            Bucket::Silent => "SILENT",
            Bucket::Inert => "INERT",
        }
    }
}

/// One authorable corruption: which fact, which leg, and the edit itself.
struct Corruption {
    fact: String,
    leg: &'static str,
    apply: Box<dyn Fn(&mut serde_json::Value)>,
}

/// Swap an entity out of a fact's `entities` list and the replacement in — the
/// R446 invariant the write path enforces, so a claim retarget carries it.
fn swap_entity(fact: &mut serde_json::Value, from: &str, to: &str) {
    let list = fact["entities"].as_array_mut().expect("entities array");
    list.retain(|e| e != from);
    let to_value = serde_json::Value::from(to);
    if !list.contains(&to_value) {
        list.push(to_value);
    }
}

/// DERIVE the corruption population: the quest layer's facts, and per fact the
/// legs it actually carries. Nothing here names a defect class — the classes are
/// whatever the legs turn out to be.
fn corruptions(store: &AtomicStore, facts_json: &serde_json::Value) -> Vec<Corruption> {
    let structural = structural_fact_ids(store).expect("quest plumbing derives");
    // The quest layer = the plumbing plus whatever pays it off (a completion
    // fact credits a giving setup, and both are the runtime's business).
    let mut population: BTreeSet<String> = structural.clone();
    for (id, fact) in &store.narrative_facts {
        if fact
            .pays_off
            .iter()
            .any(|t| structural.contains(t.as_str()))
        {
            population.insert(id.to_string());
        }
    }

    // The alternatives a retarget can choose, derived from what the store
    // already uses in that role — so the corruption stays type-plausible.
    let mut objects_of: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut predicates: BTreeSet<String> = BTreeSet::new();
    for fact in store.narrative_facts.values() {
        let Some(claim) = &fact.typed else { continue };
        if let mnemosyne_core::TypedObject::Entity { id } = &claim.object {
            predicates.insert(claim.predicate.to_string());
            objects_of
                .entry(claim.predicate.to_string())
                .or_default()
                .insert(id.to_string());
        }
    }

    let mut out = Vec::new();
    for entry in facts_json["facts"].as_array().expect("facts array") {
        let id = entry["fact_id"].as_str().expect("fact_id").to_string();
        if !population.contains(&id) {
            continue;
        }
        let fact = &store.narrative_facts[&id.as_str().into()];

        // The legs, each guarded by whether this fact HAS it.
        if let Some(claim) = &fact.typed {
            if let mnemosyne_core::TypedObject::Entity { id: object } = &claim.object {
                let object = object.to_string();
                let predicate = claim.predicate.to_string();
                // Leg 1 — the claim points at a different entity of the same role.
                if let Some(alt) = objects_of[&predicate]
                    .iter()
                    .find(|c| **c != object)
                    .cloned()
                {
                    let from = object.clone();
                    out.push(Corruption {
                        fact: id.clone(),
                        leg: "typed.object",
                        apply: Box::new(move |f| {
                            f["typed"]["object"]["id"] = alt.as_str().into();
                            swap_entity(f, &from, &alt);
                        }),
                    });
                }
                // Leg 2 — the claim carries a different predicate.
                if let Some(alt) = predicates.iter().find(|p| **p != predicate).cloned() {
                    out.push(Corruption {
                        fact: id.clone(),
                        leg: "typed.predicate",
                        apply: Box::new(move |f| f["typed"]["predicate"] = alt.as_str().into()),
                    });
                }
            }
        }
        // Leg 3 — one payoff edge goes missing.
        if !fact.pays_off.is_empty() {
            out.push(Corruption {
                fact: id.clone(),
                leg: "pays_off",
                apply: Box::new(|f| {
                    f["pays_off"].as_array_mut().expect("pays_off").remove(0);
                }),
            });
        }
        // Leg 4 — a setup stops expecting its payoff.
        if fact.payoff_expectation == mnemosyne_core::PayoffExpectation::Expected {
            out.push(Corruption {
                fact: id.clone(),
                leg: "payoff_expectation",
                apply: Box::new(|f| {
                    f.as_object_mut()
                        .expect("fact object")
                        .remove("payoff_expectation");
                }),
            });
        }
        // Leg 5 — a backreference goes missing.
        if fact.evidence.len() > 1 {
            out.push(Corruption {
                fact: id.clone(),
                leg: "evidence",
                apply: Box::new(|f| {
                    f["evidence"].as_array_mut().expect("evidence").remove(0);
                }),
            });
        }
    }
    out
}

#[test]
fn the_walk_says_what_stands_between_an_authoring_slip_and_the_runtime() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let sidecar = tmp.path().join("store.json");
    let facts_json = facts_manifest_json();

    let baseline = build(&facts_json, &sidecar).expect("the authored store must load");
    let order = canon_order(&baseline);
    assert!(
        scan_continuity(&baseline, &order, &[])
            .expect("baseline scan")
            .violations
            .is_empty(),
        "the authored store is the baseline and must be clean"
    );
    let baseline_projection = projections(&baseline, &order).expect("baseline projections");

    let population = corruptions(&baseline, &facts_json);
    assert!(
        population.len() >= 30,
        "the derived population collapsed to {} — a walk that finds almost nothing \
         reads exactly like a store with almost no surface",
        population.len()
    );

    let mut census: BTreeMap<Bucket, usize> = BTreeMap::new();
    let mut silent: Vec<String> = Vec::new();
    let mut inert: Vec<String> = Vec::new();
    for (index, corruption) in population.iter().enumerate() {
        let mut mutated = facts_json.clone();
        let mut applied = 0usize;
        for entry in mutated["facts"].as_array_mut().expect("facts array") {
            if entry["fact_id"] == corruption.fact.as_str() {
                (corruption.apply)(entry);
                applied += 1;
            }
        }
        assert_eq!(
            applied, 1,
            "corruption {index} ({}/{}) applied {applied} times",
            corruption.fact, corruption.leg
        );

        let sidecar = tmp.path().join(format!("s{index}.json"));
        let bucket = match build(&mutated, &sidecar) {
            Err(_) => Bucket::Refused,
            Ok(store) => match CanonOrder::from_declaration(
                &load_canon_order(
                    &repo_root()
                        .join("claudedocs/phase1-dnd-quest-experiment/v3/run/author/order.json"),
                    None,
                )
                .expect("canon order"),
                &store.branches,
            ) {
                Err(_) => Bucket::Caught,
                Ok(order) => match scan_continuity(&store, &order, &[]) {
                    Err(_) => Bucket::Caught,
                    Ok(report) if !report.violations.is_empty() => Bucket::Caught,
                    Ok(_) => match projections(&store, &order) {
                        Err(_) => Bucket::Caught,
                        Ok(p) if p != baseline_projection => Bucket::Silent,
                        Ok(_) => Bucket::Inert,
                    },
                },
            },
        };
        *census.entry(bucket).or_default() += 1;
        let row = format!("{}/{}", corruption.fact, corruption.leg);
        match bucket {
            Bucket::Silent => silent.push(row),
            Bucket::Inert => inert.push(row),
            _ => {}
        }
    }

    // Print BEFORE asserting: a first-violation stop would make the whole walk
    // report one line (the R1026 lesson).
    println!(
        "play-break class census over {} derived corruptions:",
        population.len()
    );
    for (bucket, n) in &census {
        println!("  {:8} {n}", bucket.as_str());
    }
    println!(
        "SILENT (gates green, the runtime's world changes) — {}:",
        silent.len()
    );
    for row in &silent {
        println!("    {row}");
    }
    println!(
        "INERT (gates green, no projection carries it) — {}:",
        inert.len()
    );
    for row in &inert {
        println!("    {row}");
    }

    assert_eq!(
        census.values().sum::<usize>(),
        population.len(),
        "every corruption lands in exactly one bucket"
    );
    // THE NUMBER THIS ROUND EXISTS TO PRODUCE. Pinned, so that a gate which
    // starts seeing a class moves it, and so that a new leg kind or a new quest
    // fact has to be looked at rather than absorbed.
    //
    // REFUSED is 0 and that is a measurement, not an omission: every corruption
    // here carries its own entities list (`swap_entity`), which is exactly what
    // Round 1031 learned the write path demands. An author who keeps the store's
    // own invariants can commit all 41.
    assert_eq!(
        census,
        BTreeMap::from([
            (Bucket::Caught, 6),
            (Bucket::Silent, 33),
            (Bucket::Inert, 2)
        ]),
        "the play-break census over the quest layer"
    );
    // The INERT pair is a finding in its own right and is named rather than
    // counted: a `pays_off` edge that neither projection a runtime reads carries
    // at all. Scoped honestly — `report-payoff-coverage` is a THIRD read and is
    // not in this walk, so "inert" means inert to the runtime's two, not unread
    // by everything.
    assert_eq!(
        inert,
        vec!["f-180/pays_off".to_string(), "f-411/pays_off".to_string()],
        "the payoff edges no runtime projection carries"
    );
}
