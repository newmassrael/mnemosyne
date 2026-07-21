//! Shared builders for the crate's unit tests — construct a terse
//! `PlayableWorldReport` (and interactive fixtures) without repeating the wide
//! upstream struct literals in every test module.

use std::collections::BTreeMap;

use mnemosyne_core::{DisclosureMode, TypedClaim, TypedObject};
use mnemosyne_validate::continuity::{
    ForkTreeBranch, ForkTreeEdge, ForkTreeReport, ManuscriptFactEvent, ManuscriptScene, MapLocator,
    PlayableWorld, PlayableWorldReport, WorldManuscript,
};

use crate::Rung;

pub(crate) fn begin(
    fact_id: &str,
    claim: &str,
    frame: &str,
    entities: &[&str],
) -> ManuscriptFactEvent {
    ManuscriptFactEvent {
        fact_id: fact_id.into(),
        frame: frame.into(),
        claim: claim.into(),
        entities: entities.iter().map(|e| (*e).to_string()).collect(),
        canon_from: "sc-01".into(),
        canon_to: None,
        evidence: Vec::new(),
        typed: None,
        quote: None,
        count: None,
        disclosure: None,
    }
}

/// A fact carrying a typed predicate (a quest leg) — for journal-routing tests.
pub(crate) fn journal_begin(fact_id: &str, claim: &str, predicate: &str) -> ManuscriptFactEvent {
    ManuscriptFactEvent {
        typed: Some(TypedClaim {
            subject: "subj".into(),
            predicate: predicate.into(),
            object: TypedObject::Token {
                token: "tok".into(),
            },
        }),
        ..begin(fact_id, claim, "ground-truth", &[])
    }
}

pub(crate) fn locator(fact_id: &str, scene: &str, mode: DisclosureMode) -> MapLocator {
    MapLocator {
        world_line: "main".into(),
        fact_id: fact_id.into(),
        scene: scene.into(),
        scene_ordinal: None,
        object: None,
        mode,
        first_at: None,
    }
}

pub(crate) fn scene(
    section: &str,
    title: &str,
    begins: Vec<ManuscriptFactEvent>,
) -> ManuscriptScene {
    ManuscriptScene {
        section: section.into(),
        title: title.into(),
        epub_locator: None,
        begins,
        ends: Vec::new(),
        holding_count: 0,
    }
}

/// A fork/converge branch fixture. `converges_from` names the parents this
/// branch merges from (empty = a pure fork / divergent ending).
pub(crate) fn branch(
    branch_id: &str,
    description: &str,
    fork_parent: &str,
    fork_at: &str,
    converges_from: &[&str],
) -> ForkTreeBranch {
    ForkTreeBranch {
        branch_id: branch_id.into(),
        description: description.into(),
        fork: Some(ForkTreeEdge {
            parent: fork_parent.into(),
            at: fork_at.into(),
            at_placed: true,
        }),
        converges: converges_from
            .iter()
            .map(|p| ForkTreeEdge {
                parent: (*p).to_string(),
                at: fork_at.into(),
                at_placed: true,
            })
            .collect(),
    }
}

pub(crate) fn rung(question: &str, reveals: &str, needs: &[&str]) -> Rung {
    Rung {
        question: question.into(),
        reveals: reveals.into(),
        needs: needs.iter().map(|n| (*n).to_string()).collect(),
    }
}

/// A one-world report (`main`) from scenes + locators + an optional fork tree.
pub(crate) fn report(
    world: &str,
    scenes: Vec<ManuscriptScene>,
    locators: Vec<MapLocator>,
    fork_tree: ForkTreeReport,
) -> PlayableWorldReport {
    let mut worlds = BTreeMap::new();
    worlds.insert(
        world.to_string(),
        PlayableWorld {
            manuscript: WorldManuscript {
                scenes,
                ..Default::default()
            },
            locators,
        },
    );
    PlayableWorldReport {
        telling: "reader".into(),
        fork_tree,
        worlds,
    }
}
