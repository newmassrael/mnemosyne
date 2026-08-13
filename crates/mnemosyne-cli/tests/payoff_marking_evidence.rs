//! Does an author ever pay off a fact they never marked as a setup? (R1037)
//!
//! Round 1036 left `payoffs_to_unmarked is always empty` as the ONE surviving
//! gate candidate of this arc, on evidence of 41 authored worlds every one of
//! them at zero. That evidence is narrower than it sounds, in two ways this
//! walk closes:
//!
//! - It is WORLD-SCOPED. `report-payoff-coverage` classifies an edge only in a
//!   world where BOTH endpoints are visible; an edge whose endpoints never
//!   co-occur is inert there and is counted by no world's list. The rule a gate
//!   would enforce is about the STORE — `pays_off` and `payoff_expectation` are
//!   two halves of one relation whether or not any world-line sees them
//!   together — so the store-wide count is the one that refutes it.
//! - It was drawn from the 27 corpora that still LOAD. Sixteen tracked corpora
//!   are pre-migration manifests that no longer import, and a corpus that stops
//!   loading silently shrinks the evidence every rule is judged against
//!   (Round 1036 counted them; it could not ask them).
//!
//! Both are answered by reading the AUTHOR'S OWN manifest, which every corpus
//! has whether or not the import still accepts it. A manifest reader is only
//! worth as much as its agreement with the write path, so this walk runs BOTH
//! readers on every corpus that loads and asserts they agree, and only then
//! reads the sixteen that no store can be built from. The spelling of the
//! marking comes from the product's own `PayoffExpectation` via serde, never
//! from a literal written here — a reader that quietly failed to recognise
//! `expected` would report every corpus clean, which is the direction that
//! INVENTS gate candidates (Round 1036).

use mnemosyne_atomic::AtomicStore;
use mnemosyne_core::PayoffExpectation;
use std::collections::BTreeMap;

use crate::common;
use common::{
    authored_corpora, corpus_workspace_try, dnd_quest_facts, dnd_quest_workspace_try, read_json,
    repo_root,
};

/// The sidecar the import writes, relative to the workspace root.
const SIDECAR: &str = "docs/.atomic/workspace.atomic.json";

/// What one store SAYS about the setup/payoff relation, counted identically on
/// both sides of the write path so the two counts can be compared.
#[derive(Debug, Default, PartialEq, Eq)]
struct Declared {
    facts: usize,
    /// `pays_off` edges, whatever they name.
    edges: usize,
    /// Edges naming a fact this store does not hold. Zero by construction on
    /// the store side — the write path refuses them — so a non-zero here is a
    /// manifest the import would reject, and the row says so rather than
    /// silently counting it as marked.
    unresolved: usize,
    /// The edges this walk exists to count: a payoff naming a setup its author
    /// left `Unmarked`. Rendered `payoff->setup`, because a count cannot be
    /// repaired and a named edge can.
    to_unmarked: Vec<String>,
}

/// Read the author's own manifest. Returns the reason rather than panicking
/// when the shape is not one this reader understands: a corpus it cannot parse
/// must be VISIBLE as unparsed, never counted as clean.
fn declared_in_manifest(manifest: &serde_json::Value) -> Result<Declared, String> {
    let rows = manifest
        .get("facts")
        .and_then(serde_json::Value::as_array)
        .ok_or("no `facts` array")?;
    let mut expectation: BTreeMap<&str, PayoffExpectation> = BTreeMap::new();
    for row in rows {
        let id = row
            .get("fact_id")
            .and_then(serde_json::Value::as_str)
            .ok_or("a fact row carries no `fact_id`")?;
        // The default is the product's, and the spelling is the product's:
        // serde is what the import itself would use to read this field.
        let marked = match row.get("payoff_expectation") {
            None => PayoffExpectation::default(),
            Some(v) => serde_json::from_value(v.clone())
                .map_err(|e| format!("`{id}`: payoff_expectation {v}: {e}"))?,
        };
        expectation.insert(id, marked);
    }
    let mut out = Declared {
        facts: expectation.len(),
        ..Default::default()
    };
    for row in rows {
        let id = row["fact_id"].as_str().expect("read above");
        let Some(targets) = row.get("pays_off") else {
            continue;
        };
        let targets = targets
            .as_array()
            .ok_or_else(|| format!("`{id}`: pays_off is not an array"))?;
        for target in targets {
            let target = target
                .as_str()
                .ok_or_else(|| format!("`{id}`: a pays_off ref is not a string"))?;
            out.edges += 1;
            match expectation.get(target) {
                None => out.unresolved += 1,
                Some(PayoffExpectation::Expected) => {}
                Some(PayoffExpectation::Unmarked) => {
                    out.to_unmarked.push(format!("{id}->{target}"))
                }
            }
        }
    }
    out.to_unmarked.sort();
    Ok(out)
}

/// The same question asked of the store the write path actually built.
fn declared_in_store(store: &AtomicStore) -> Declared {
    let facts = &store.narrative_facts;
    let mut out = Declared {
        facts: facts.len(),
        ..Default::default()
    };
    for (id, fact) in facts {
        for target in &fact.pays_off {
            out.edges += 1;
            match facts.get(target).map(|f| f.payoff_expectation) {
                None => out.unresolved += 1,
                Some(PayoffExpectation::Expected) => {}
                Some(PayoffExpectation::Unmarked) => {
                    out.to_unmarked.push(format!("{id}->{target}"))
                }
            }
        }
    }
    out.to_unmarked.sort();
    out
}

/// One corpus's answer, and whether a store could be built to check it against.
struct Row {
    name: String,
    manifest: Result<Declared, String>,
    store: Option<Declared>,
}

#[test]
fn every_authored_corpus_answers_whether_a_payoff_names_an_unmarked_setup() {
    let mut rows: Vec<Row> = Vec::new();

    // The migrated record FIRST — it is the store every corruption walk in this
    // arc runs against, and Round 1036's finding was that leaving the corpus
    // under measurement out of its own refuter turns refutations into
    // candidates.
    let migrated = dnd_quest_facts();
    rows.push(Row {
        name: "crates/mnemosyne-cli/tests/fixtures/dnd-quest (the migrated record)".to_string(),
        manifest: declared_in_manifest(&migrated),
        store: dnd_quest_workspace_try(&migrated)
            .ok()
            .and_then(|ws| AtomicStore::load(&ws.path().join(SIDECAR)).ok())
            .map(|store| declared_in_store(&store)),
    });

    for dir in authored_corpora() {
        let name = dir
            .strip_prefix(repo_root())
            .unwrap_or(&dir)
            .display()
            .to_string();
        let manifest = read_json(&dir.join("facts.json"));
        rows.push(Row {
            name,
            manifest: declared_in_manifest(&manifest),
            store: corpus_workspace_try(&dir, &manifest)
                .ok()
                .and_then(|ws| AtomicStore::load(&ws.path().join(SIDECAR)).ok())
                .map(|store| declared_in_store(&store)),
        });
    }

    // Print BEFORE asserting: a first-violation stop reports one line where the
    // walk's whole value is the distribution (the R1026 lesson).
    println!("setup/payoff marking, as every authored corpus declares it:\n");
    for Row {
        name,
        manifest,
        store,
    } in &rows
    {
        match manifest {
            Err(why) => println!("  UNPARSED {name} :: {why}"),
            Ok(d) => println!(
                "  {} {name} :: facts={} pays_off={} unresolved={} to_unmarked={:?}",
                if store.is_some() { "loads " } else { "dead  " },
                d.facts,
                d.edges,
                d.unresolved,
                d.to_unmarked,
            ),
        }
    }

    // THE CROSS-CHECK THAT LICENSES THE REST. On every corpus a store can be
    // built from, the author's manifest and the imported store must give the
    // same three numbers and the same named edges. Without this the sixteen
    // dead corpora would be read by a reader nothing ever validated, and a
    // reader that silently sees nothing reports every one of them clean.
    let mut checked = 0usize;
    for Row {
        name,
        manifest,
        store,
    } in &rows
    {
        let (Ok(manifest), Some(store)) = (manifest, store) else {
            continue;
        };
        assert_eq!(
            manifest, store,
            "the manifest reader and the write path disagree about {name}, so \
             nothing this walk says about the corpora that no longer load is \
             worth anything"
        );
        checked += 1;
    }

    let loaded = rows.iter().filter(|r| r.store.is_some()).count();
    let unparsed = rows.iter().filter(|r| r.manifest.is_err()).count();
    let parsed: Vec<&Declared> = rows
        .iter()
        .filter_map(|r| r.manifest.as_ref().ok())
        .collect();
    let facts: usize = parsed.iter().map(|d| d.facts).sum();
    let edges: usize = parsed.iter().map(|d| d.edges).sum();
    let bearing = parsed.iter().filter(|d| d.edges > 0).count();
    let unresolved: usize = parsed.iter().map(|d| d.unresolved).sum();
    let verdict: Vec<String> = rows
        .iter()
        .filter_map(|r| r.manifest.as_ref().ok().map(|d| (&r.name, d)))
        .flat_map(|(name, d)| {
            d.to_unmarked
                .iter()
                .map(move |edge| format!("{name} :: {edge}"))
        })
        .collect();
    println!(
        "\n{} corpora asked, {loaded} load, {unparsed} unparsed, {checked} \
         cross-checked against the write path",
        rows.len(),
    );
    println!(
        "{facts} facts declared, {edges} pays_off edges across {bearing} corpora, \
         {unresolved} naming a fact their own manifest does not declare",
    );
    println!("payoff edges naming an UNMARKED setup: {verdict:?}");

    assert_eq!(
        (rows.len(), loaded, unparsed, checked),
        (44, 28, 0, 28),
        "the population: corpora asked, those a store can be built from, those \
         whose manifest this reader cannot read, and those where both readers \
         answered and were compared"
    );

    // THE EVIDENCE, not just the verdict. A rule resting on 41 worlds could
    // still be resting on four edges; how many edges and how many separate
    // authors produced them is the whole of what is known.
    assert_eq!(
        (facts, edges, bearing, unresolved),
        (3800, 296, 44, 0),
        "facts declared, pays_off edges, corpora declaring at least one, and \
         edges whose target their own manifest never declares"
    );

    // THE VERDICT. Empty = the rule this arc's last candidate proposes is
    // un-refuted by every store an author has shipped here, across both the
    // world-scoped reading and the wider store-wide one. A corpus can only
    // refute (Round 1032), so this is un-refuted, never confirmed.
    assert_eq!(
        verdict,
        Vec::<String>::new(),
        "the authored corpora that pay off a setup they never marked"
    );
}
