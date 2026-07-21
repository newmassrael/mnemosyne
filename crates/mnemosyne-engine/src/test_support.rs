//! Shared builders for the crate's unit tests — construct a terse
//! `PlayableWorldReport` (and interactive fixtures) without repeating the wide
//! upstream struct literals in every test module.

use mnemosyne_core::{DisclosureMode, TypedClaim, TypedObject};
use mnemosyne_validate::continuity::{
    ForkTreeBranch, ForkTreeEdge, ForkTreeReport, ManuscriptFactEvent, ManuscriptScene, MapLocator,
    PlayableWorld, PlayableWorldReport, QuestCompletion, QuestGraphReport, QuestNode, QuestState,
    QuestWorldState, WorldManuscript,
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

/// A quest completion beat (the discharging fact, its scene, the actor).
pub(crate) fn completion(fact: &str, scene: &str, actor: Option<&str>) -> QuestCompletion {
    QuestCompletion {
        fact: fact.into(),
        scene: scene.into(),
        actor: actor.map(str::to_string),
    }
}

/// One quest node: id + objective + prerequisites + per-world (state,
/// completions). Actors/giving/locators are left default (unused by the quest
/// tests, which exercise the completability gate).
pub(crate) fn quest_node(
    quest_id: &str,
    objective: &str,
    prerequisites: &[&str],
    per_world: Vec<(&str, QuestState, Vec<QuestCompletion>)>,
) -> QuestNode {
    QuestNode {
        quest_id: quest_id.into(),
        objective: objective.into(),
        prerequisites: prerequisites.iter().map(|p| (*p).to_string()).collect(),
        per_world: per_world
            .into_iter()
            .map(|(world, state, completions)| {
                (world.to_string(), QuestWorldState { state, completions })
            })
            .collect(),
        ..Default::default()
    }
}

/// A quest-graph report (`reader` telling) from quest nodes.
pub(crate) fn quest_report(quests: Vec<QuestNode>) -> QuestGraphReport {
    QuestGraphReport {
        telling: "reader".into(),
        quests,
        ..Default::default()
    }
}

/// A one-world report (`main`) from scenes + locators + an optional fork tree.
pub(crate) fn report(
    world: &str,
    scenes: Vec<ManuscriptScene>,
    locators: Vec<MapLocator>,
    fork_tree: ForkTreeReport,
) -> PlayableWorldReport {
    report_worlds(vec![(world, scenes, locators)], fork_tree)
}

/// A multi-world report: each `(world, scenes, locators)` its own walk, sharing
/// one fork tree — for per-world gate discrimination (a fact diggable on one
/// road but not a sibling).
pub(crate) fn report_worlds(
    worlds: Vec<(&str, Vec<ManuscriptScene>, Vec<MapLocator>)>,
    fork_tree: ForkTreeReport,
) -> PlayableWorldReport {
    let worlds = worlds
        .into_iter()
        .map(|(world, scenes, locators)| {
            (
                world.to_string(),
                PlayableWorld {
                    manuscript: WorldManuscript {
                        scenes,
                        ..Default::default()
                    },
                    locators,
                },
            )
        })
        .collect();
    PlayableWorldReport {
        telling: "reader".into(),
        fork_tree,
        worlds,
    }
}
