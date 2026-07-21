//! The quest-graph projection + the fail-loud completability gate — the
//! JOURNAL-axis sibling of [`PlayableProjection`](crate::PlayableProjection).
//!
//! Reads the store's universal quest graph (`report-quest-graph`, the R559/R568
//! projection) at runtime and exposes it as a presentation-agnostic quest layer:
//! every quest is store-derived (the kernel invents none), so the compile-time
//! snapshot a consumer would otherwise bake in cannot drift from the store.
//!
//! The completability gate generalizes a consumer's investigation-openability
//! check: a quest's completion-PRECONDITION facts (the `opened_by`-class edges,
//! a fact bridge since R707) must be diggable on the world's walk BEFORE the
//! quest completes, or the knowledge that opens the quest can never be reached in
//! time and the quest can never legitimately complete by play. The precondition
//! predicate is consumer-declared (the [`journal_predicates`] contract), never
//! hardcoded — the kernel stays content-agnostic.
//!
//! [`journal_predicates`]: crate::EngineOverrides::journal_predicates

use std::collections::BTreeMap;
use std::path::Path;

use mnemosyne_core::TypedObject;
use mnemosyne_validate::continuity::{QuestGraphReport, QuestState};

use crate::{EngineError, EngineOverrides, PlayableProjection};

/// A quest's completion on one road — the discharging fact, the scene it
/// completes at, and the actor the store names as discharger. Store-derived.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct QuestCompletionView {
    /// The fact discharging the quest on this road (a `narrative_facts` key).
    pub fact: String,
    /// The scene the quest completes at on this road.
    pub scene: String,
    /// The actor the fact's `completed_by` claim names as discharger, when it
    /// carries one for this quest (`None` when untyped or a foreign completion).
    pub actor: Option<String>,
}

/// A quest's state on one world-line — the derived open/done/unknown verdict plus
/// the completion beat(s) on that road (empty when open).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct QuestWorldView {
    /// The derived verdict on this road.
    pub state: QuestState,
    /// The completion beat(s) discharging the quest here; empty when open.
    pub completions: Vec<QuestCompletionView>,
}

/// One quest as the kernel exposes it — read from the store's quest graph, so
/// every field is store-derived (the kernel invents no quest).
/// `#[non_exhaustive]`: a downstream crate READS a quest but cannot fabricate one
/// with a struct literal from another crate.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct QuestView {
    /// The quest entity id.
    pub quest_id: String,
    /// The quest objective — the entity's description.
    pub objective: String,
    /// The actor entities that LEAD the quest (`pursues` subjects), sorted.
    pub actors: Vec<String>,
    /// Prerequisite quest ids that must complete first (`requires` objects),
    /// sorted — the declarative order (the canon proves the timing).
    pub prerequisites: Vec<String>,
    /// Per world-line, the quest's derived state + completion beat(s), keyed by
    /// world.
    pub per_world: BTreeMap<String, QuestWorldView>,
    /// Completion-precondition facts — the facts a consumer's declared
    /// precondition predicate (e.g. `opened_by`) names for this quest: knowledge
    /// that must be diggable before the quest completes. Sorted + deduped; empty
    /// when the consumer declared no precondition predicate or the quest has
    /// none. The completability gate reads these.
    pub preconditions: Vec<String>,
}

/// A fail-loud quest-completability finding — a spot where the quest layer makes
/// a quest impossible to complete by play. Reported, never silently dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum QuestGateViolation {
    /// A quest's completion-precondition fact is never offered on the world's
    /// walk BEFORE the quest completes: the knowledge that opens the quest can
    /// never be dug in time, so the quest can never legitimately complete. The
    /// quest-layer analog of the ladder gate's
    /// [`GateViolation::PreconditionUnreachable`](crate::GateViolation::PreconditionUnreachable),
    /// generalized from a consumer's investigation-openability check.
    PreconditionUnreachable {
        /// The world-line walked.
        world: String,
        /// The quest whose precondition dangles.
        quest: String,
        /// The completion scene the precondition must precede (the deadline).
        completion_scene: String,
        /// The precondition `fact_id` the walk never offers before completion.
        needs: String,
    },
}

/// The quest-graph projection for one telling — the quests the store declares
/// (each store-derived) plus the consumer's completion-precondition edges. The
/// JOURNAL-axis sibling of [`PlayableProjection`]; the completability gate reads
/// a [`PlayableProjection`] for the walk + disclosed facts it checks against.
#[derive(Debug, Clone)]
pub struct QuestProjection {
    telling: String,
    quests: Vec<QuestView>,
}

impl QuestProjection {
    /// Project the workspace store's quest graph under `telling`, reading it in
    /// process (no JSON round-trip). The overrides'
    /// [`quest_precondition_predicates`](EngineOverrides::quest_precondition_predicates)
    /// name the typed predicates whose claims are quest completion-preconditions
    /// (e.g. `opened_by`); their object facts are attached per quest for the
    /// completability gate.
    ///
    /// # Errors
    ///
    /// [`EngineError::Projection`] if the quest-graph read fails (unregistered
    /// world, typo'd telling, a malformed quest predicate, or an unreadable
    /// store).
    pub fn from_workspace(
        workspace_root: &Path,
        telling: &str,
        order_override: Option<&str>,
        overrides: &impl EngineOverrides,
    ) -> Result<Self, EngineError> {
        let report =
            mnemosyne_ops::quest_graph_report(workspace_root, None, None, order_override, telling)
                .map_err(|e| EngineError::Projection(e.to_string()))?;
        let preconditions =
            read_preconditions(workspace_root, overrides.quest_precondition_predicates())
                .map_err(EngineError::Projection)?;
        Ok(Self::from_report(report, &preconditions))
    }

    /// Index an already-projected quest graph + a precondition map (quest id ->
    /// its completion-precondition fact ids) — the testable core
    /// ([`Self::from_workspace`] is the store-reading wrapper).
    #[must_use]
    pub fn from_report(
        report: QuestGraphReport,
        preconditions: &BTreeMap<String, Vec<String>>,
    ) -> Self {
        let QuestGraphReport {
            telling, quests, ..
        } = report;
        let quests = quests
            .into_iter()
            .map(|q| {
                let mut pre = preconditions.get(&q.quest_id).cloned().unwrap_or_default();
                pre.sort();
                pre.dedup();
                QuestView {
                    quest_id: q.quest_id,
                    objective: q.objective,
                    actors: q.actors,
                    prerequisites: q.prerequisites,
                    per_world: q
                        .per_world
                        .into_iter()
                        .map(|(world, ws)| {
                            let completions = ws
                                .completions
                                .into_iter()
                                .map(|c| QuestCompletionView {
                                    fact: c.fact,
                                    scene: c.scene,
                                    actor: c.actor,
                                })
                                .collect();
                            (
                                world,
                                QuestWorldView {
                                    state: ws.state,
                                    completions,
                                },
                            )
                        })
                        .collect(),
                    preconditions: pre,
                }
            })
            .collect();
        Self { telling, quests }
    }

    /// The telling this projection was cut for.
    #[must_use]
    pub fn telling(&self) -> &str {
        &self.telling
    }

    /// Every quest the store declares, sorted by id.
    #[must_use]
    pub fn quests(&self) -> &[QuestView] {
        &self.quests
    }

    /// One quest by id, when present.
    #[must_use]
    pub fn quest(&self, quest_id: &str) -> Option<&QuestView> {
        self.quests.iter().find(|q| q.quest_id == quest_id)
    }

    /// The fail-loud completability gate: for every quest that COMPLETES in a
    /// world, each of its completion-precondition facts must be OFFERED on that
    /// world's walk STRICTLY BEFORE the quest's earliest completion scene. A
    /// precondition offered only at-or-after completion (or never) means the
    /// knowledge that opens the quest can never be dug in time — the quest can
    /// never legitimately complete by play. A quest that is OPEN in a world (no
    /// completion beat there) has no deadline, so it is not gated. Pure read;
    /// never mutates. Returns violations in quest-then-world order.
    #[must_use]
    pub fn completability(&self, playable: &PlayableProjection) -> Vec<QuestGateViolation> {
        let mut violations = Vec::new();
        for quest in &self.quests {
            if quest.preconditions.is_empty() {
                continue;
            }
            for (world, wv) in &quest.per_world {
                let walk = playable.walk(world);
                // The deadline = the earliest completion scene index on this
                // world's walk. No completion on this walk (open here, or a
                // completion scene off the walk) = no deadline = not gated.
                let Some(deadline) = wv
                    .completions
                    .iter()
                    .filter_map(|c| walk.iter().position(|s| *s == c.scene))
                    .min()
                else {
                    continue;
                };
                for need in &quest.preconditions {
                    // Offered STRICTLY BEFORE the completion scene — the
                    // knowledge must be dug before the quest discharges. A
                    // withheld fact emits no line, so "offered" is exactly
                    // "diggable under this telling".
                    let in_time = walk.iter().take(deadline).any(|section| {
                        playable
                            .lines(world, section)
                            .iter()
                            .any(|l| l.fact_id() == need)
                    });
                    if !in_time {
                        violations.push(QuestGateViolation::PreconditionUnreachable {
                            world: world.clone(),
                            quest: quest.quest_id.clone(),
                            completion_scene: walk[deadline].clone(),
                            needs: need.clone(),
                        });
                    }
                }
            }
        }
        violations
    }
}

/// Read a consumer's completion-precondition claims from the store: for each
/// declared precondition predicate, collect its typed claims keyed by subject
/// (the quest) -> the object's fact id, the typed `opened_by = f-*` fact bridge
/// (R707/R708). Only a `TypedObject::Fact` object is a checkable precondition (a
/// fact the walk can offer); other object shapes are not. Empty predicate list =
/// no store read. Fails through the store load with a stringified error.
fn read_preconditions(
    workspace_root: &Path,
    predicates: &[String],
) -> Result<BTreeMap<String, Vec<String>>, String> {
    let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    if predicates.is_empty() {
        return Ok(map);
    }
    let store =
        mnemosyne_ops::load_atomic_store(workspace_root, None).map_err(|e| e.to_string())?;
    for fact in store.narrative_facts.values() {
        let Some(claim) = &fact.typed else { continue };
        if !predicates.contains(&claim.predicate) {
            continue;
        }
        // A completion-precondition object is a typed FACT bridge (R707/R708
        // closed the object-shape: `opened_by = f-*`), so the gate can check it
        // against the facts the walk offers — only a `Fact` id joins against a
        // line's `fact_id`. A validated store carries no other object shape under
        // such a predicate (the R708 write-path gate), and an entity/token id
        // could never be an offered fact, so `Fact` is the sole checkable shape.
        if let TypedObject::Fact { id } = &claim.object {
            map.entry(claim.subject.clone())
                .or_default()
                .push(id.clone());
        }
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use mnemosyne_core::DisclosureMode;
    use mnemosyne_validate::continuity::{ForkTreeReport, QuestState};

    use crate::test_support::{
        begin, completion, locator, quest_node, quest_report, report, report_worlds, scene,
    };
    use crate::{PlayableProjection, QuestGateViolation, QuestProjection, StaticOverrides};

    fn preconditions(pairs: &[(&str, &[&str])]) -> BTreeMap<String, Vec<String>> {
        pairs
            .iter()
            .map(|(q, facts)| {
                (
                    (*q).to_string(),
                    facts.iter().map(|f| (*f).to_string()).collect(),
                )
            })
            .collect()
    }

    #[test]
    fn projects_quests_and_attaches_preconditions() {
        let r = quest_report(vec![quest_node(
            "q-knot-1",
            "the first sin",
            &["q-salt"],
            vec![(
                "main",
                QuestState::Done,
                vec![completion("f-confess", "sc-gut", Some("ent-eldest"))],
            )],
        )]);
        let proj = QuestProjection::from_report(
            r,
            &preconditions(&[("q-knot-1", &["f-b-clue", "f-a-clue", "f-a-clue"])]),
        );
        let q = proj.quest("q-knot-1").expect("the quest is projected");
        assert_eq!(q.objective, "the first sin");
        assert_eq!(q.prerequisites, vec!["q-salt".to_string()]);
        // preconditions sorted + deduped from the map.
        assert_eq!(
            q.preconditions,
            vec!["f-a-clue".to_string(), "f-b-clue".to_string()]
        );
        let wv = &q.per_world["main"];
        assert_eq!(wv.state, QuestState::Done);
        assert_eq!(wv.completions[0].scene, "sc-gut");
        assert_eq!(wv.completions[0].actor.as_deref(), Some("ent-eldest"));
        assert_eq!(proj.telling(), "reader");
        assert_eq!(proj.quests().len(), 1);
    }

    /// A quest whose precondition is offered before the completion scene is
    /// completable — no violation.
    #[test]
    fn a_completable_quest_has_no_violation() {
        // The walk offers f-clue at sc-01; the quest completes at sc-03.
        let playable = PlayableProjection::from_report(
            report(
                "main",
                vec![
                    scene(
                        "sc-01",
                        "Dawn",
                        vec![begin("f-clue", "the clue", "ground-truth", &[])],
                    ),
                    scene("sc-02", "Noon", Vec::new()),
                    scene("sc-03", "Gut", Vec::new()),
                ],
                vec![locator("f-clue", "sc-01", DisclosureMode::State)],
                ForkTreeReport::default(),
            ),
            &StaticOverrides::default(),
        )
        .unwrap();
        let quests = QuestProjection::from_report(
            quest_report(vec![quest_node(
                "q-1",
                "a quest",
                &[],
                vec![(
                    "main",
                    QuestState::Done,
                    vec![completion("f-done", "sc-03", None)],
                )],
            )]),
            &preconditions(&[("q-1", &["f-clue"])]),
        );
        assert!(quests.completability(&playable).is_empty());
    }

    /// A precondition offered only AT the completion scene (not before) is
    /// unreachable — the knowledge arrives too late to open the quest.
    #[test]
    fn a_precondition_offered_only_at_completion_is_flagged() {
        let build_playable = |clue_at: &str| {
            PlayableProjection::from_report(
                report(
                    "main",
                    vec![
                        scene("sc-01", "Dawn", Vec::new()),
                        scene(
                            "sc-03",
                            "Gut",
                            vec![begin("f-clue", "the clue", "ground-truth", &[])],
                        ),
                    ],
                    vec![locator("f-clue", clue_at, DisclosureMode::State)],
                    ForkTreeReport::default(),
                ),
                &StaticOverrides::default(),
            )
            .unwrap()
        };
        let quests = QuestProjection::from_report(
            quest_report(vec![quest_node(
                "q-1",
                "a quest",
                &[],
                vec![(
                    "main",
                    QuestState::Done,
                    vec![completion("f-done", "sc-03", None)],
                )],
            )]),
            &preconditions(&[("q-1", &["f-clue"])]),
        );
        // Offered at sc-03 == the completion scene -> too late.
        let late = build_playable("sc-03");
        assert_eq!(
            quests.completability(&late),
            vec![QuestGateViolation::PreconditionUnreachable {
                world: "main".into(),
                quest: "q-1".into(),
                completion_scene: "sc-03".into(),
                needs: "f-clue".into(),
            }]
        );
        // Non-vacuity: move the SAME clue to sc-01 (before) and the flag clears.
        let early = PlayableProjection::from_report(
            report(
                "main",
                vec![
                    scene(
                        "sc-01",
                        "Dawn",
                        vec![begin("f-clue", "the clue", "ground-truth", &[])],
                    ),
                    scene("sc-03", "Gut", Vec::new()),
                ],
                vec![locator("f-clue", "sc-01", DisclosureMode::State)],
                ForkTreeReport::default(),
            ),
            &StaticOverrides::default(),
        )
        .unwrap();
        assert!(quests.completability(&early).is_empty());
    }

    /// The per-road property (the shape the real-store measurement surfaced on
    /// tide's `braid-ledger`): the SAME quest completes at the shared gut scene
    /// on two roads, but its precondition is diggable only on one. The road that
    /// never offers it before completion is flagged; the road that does is clean.
    /// tide's old main-only openability check could not see the sibling road.
    #[test]
    fn a_precondition_diggable_only_on_a_sibling_road_is_flagged_here() {
        // `main` offers f-clue at sc-01 (before the gut sc-03); `fork` never
        // offers it, though its walk still reaches the shared gut sc-03.
        let playable = PlayableProjection::from_report(
            report_worlds(
                vec![
                    (
                        "main",
                        vec![
                            scene(
                                "sc-01",
                                "Dawn",
                                vec![begin("f-clue", "the clue", "ground-truth", &[])],
                            ),
                            scene("sc-03", "Gut", Vec::new()),
                        ],
                        vec![locator("f-clue", "sc-01", DisclosureMode::State)],
                    ),
                    (
                        "fork",
                        vec![
                            scene("sc-01", "Dawn", Vec::new()),
                            scene("sc-03", "Gut", Vec::new()),
                        ],
                        Vec::new(),
                    ),
                ],
                ForkTreeReport::default(),
            ),
            &StaticOverrides::default(),
        )
        .unwrap();
        let quests = QuestProjection::from_report(
            quest_report(vec![quest_node(
                "q-1",
                "a quest",
                &[],
                vec![
                    (
                        "main",
                        QuestState::Done,
                        vec![completion("f-done", "sc-03", None)],
                    ),
                    (
                        "fork",
                        QuestState::Done,
                        vec![completion("f-done", "sc-03", None)],
                    ),
                ],
            )]),
            &preconditions(&[("q-1", &["f-clue"])]),
        );
        // Flagged on `fork` only — main dug the clue in time.
        assert_eq!(
            quests.completability(&playable),
            vec![QuestGateViolation::PreconditionUnreachable {
                world: "fork".into(),
                quest: "q-1".into(),
                completion_scene: "sc-03".into(),
                needs: "f-clue".into(),
            }]
        );
    }

    /// A quest that never completes in a world has no deadline, so its
    /// preconditions are not gated there (nothing to be "in time" for).
    #[test]
    fn an_open_quest_is_not_gated() {
        let playable = PlayableProjection::from_report(
            report(
                "main",
                vec![scene("sc-01", "Dawn", Vec::new())],
                Vec::new(),
                ForkTreeReport::default(),
            ),
            &StaticOverrides::default(),
        )
        .unwrap();
        // Open in `main`, no completion beat -> not gated even though the
        // precondition is never offered anywhere.
        let quests = QuestProjection::from_report(
            quest_report(vec![quest_node(
                "q-1",
                "an open quest",
                &[],
                vec![("main", QuestState::Open, Vec::new())],
            )]),
            &preconditions(&[("q-1", &["f-never"])]),
        );
        assert!(quests.completability(&playable).is_empty());
    }

    /// No declared precondition predicate = no preconditions = no completability
    /// gate (the default-overrides zero-config path).
    #[test]
    fn a_quest_with_no_preconditions_is_not_gated() {
        let playable = PlayableProjection::from_report(
            report(
                "main",
                vec![scene("sc-01", "Dawn", Vec::new())],
                Vec::new(),
                ForkTreeReport::default(),
            ),
            &StaticOverrides::default(),
        )
        .unwrap();
        let quests = QuestProjection::from_report(
            quest_report(vec![quest_node(
                "q-1",
                "a quest",
                &[],
                vec![(
                    "main",
                    QuestState::Done,
                    vec![completion("f-done", "sc-01", None)],
                )],
            )]),
            &BTreeMap::new(),
        );
        assert!(quests.completability(&playable).is_empty());
    }
}
