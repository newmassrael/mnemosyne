//! The quest-graph projection + the fail-loud completability gate — the
//! JOURNAL-axis sibling of [`PlayableProjection`](crate::PlayableProjection).
//!
//! Reads the store's universal quest graph (`report-quest-graph`, the R559/R568
//! projection) and exposes it as a presentation-agnostic quest layer: every quest
//! is store-derived (the kernel invents none), so a consumer's quest list cannot
//! drift from the store.
//!
//! This said the read happens "at runtime" and justified that by the staleness of
//! "the compile-time snapshot a consumer would otherwise bake in". Round 771
//! corrects it: the anti-drift property comes from DERIVING FROM THE STORE, not
//! from the timing of the derivation. A projection baked at build time under
//! `cargo:rerun-if-changed` and never committed cannot go stale either, and it
//! carries a check a runtime read cannot — change a kernel type and the generated
//! source stops compiling (see `mnemosyne-engine-build`, R770). The residual
//! difference is honest and small: a pre-built binary is pinned to the store
//! revision it was built from, the same trade a consumer already makes pinning
//! this workspace by git rev. The quest axis has no baked path yet; when it wants
//! one, nothing here argues against it.
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

use mnemosyne_ops::AbsolutePath;
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
    /// A quest names a world-line the paired [`PlayableProjection`] does not
    /// carry (Round 778), so this quest's preconditions went UNCHECKED on that
    /// road. Until now the road was skipped in silence, which spent the gate's
    /// clean verdict on a check it never performed.
    ///
    /// The two projections are built by separate calls and joined only here, so
    /// nothing but this makes a divergence visible: a quest layer read from one
    /// store or telling against a walk from another, or a baked pair whose two
    /// emitter invocations disagreed. The first consumer's store has the two sets
    /// identical at seven worlds, which is the point — a check that fires only on
    /// a misconfiguration is the one worth having.
    WorldNotWalked {
        /// The world-line the quest declares.
        world: String,
        /// The quest whose preconditions were not checked there.
        quest: String,
    },
    /// The quest projection and the playable projection it was handed were cut
    /// for DIFFERENT tellings (Round 778) — a quest layer judged against a walk
    /// that was never its own.
    ///
    /// Both projections carry the telling they were resolved under and nothing
    /// compared them, while every constructor takes its telling as a separate
    /// argument (twice over for a build-time bake, which invokes the playable and
    /// quest emitters independently). Reported alone, because a pair this far
    /// apart makes every finding below it a statement about the wrong world, and
    /// a list of those would read as work done.
    TellingMismatch {
        /// The telling the quest projection was resolved under.
        quest_telling: String,
        /// The telling the playable projection was resolved under.
        playable_telling: String,
    },
}

/// The quest-graph projection for one telling — the quests the store declares
/// (each store-derived) plus the consumer's completion-precondition edges. The
/// JOURNAL-axis sibling of [`PlayableProjection`]; the completability gate reads
/// a [`PlayableProjection`] for the walk + disclosed facts it checks against.
/// Not `Clone`, for the reason
/// [`PlayableProjection`](crate::PlayableProjection) is not (Round 788): the
/// quest emitter hands back `&'static Self` too, so a derived `Clone` would make
/// the compiler's suggested repair a silent deep copy on this axis as well. The
/// two artifacts share one assembler and must share this.
#[derive(Debug)]
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
        order_override: Option<&AbsolutePath>,
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
    ///
    /// CRATE-PRIVATE since Round 773, for the reason R771 closed the playable
    /// axis's twin: a [`QuestGraphReport`] is a pub-field struct, so a public
    /// constructor over one let a downstream crate hand the kernel quests the
    /// store never declared — while this module's opening line promises the
    /// opposite ("every quest is store-derived, the kernel invents none"). The
    /// promise now holds by construction. A consumer supplies a whole projection
    /// through [`QuestProjectionParts`] instead, which a build fills from the
    /// store.
    #[must_use]
    pub(crate) fn from_report(
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
    ///
    /// This is the ONE place the two projections meet, and Round 778 made the
    /// meeting checked. They are built by separate calls, each taking its own
    /// `telling`, and a build-time bake invokes the two emitters independently —
    /// so a mismatched pair was not only possible but unremarkable to produce.
    /// A pair cut for different tellings is [`QuestGateViolation::TellingMismatch`]
    /// and nothing else; a quest world the playable projection does not carry is
    /// [`QuestGateViolation::WorldNotWalked`] rather than a silently skipped road.
    ///
    /// What "OFFERED" means here is resolved by
    /// [`PlayableProjection::offers`](crate::PlayableProjection::offers) and
    /// nowhere else (Round 787): the telling offers a fact where the audience
    /// MEETS it, whichever stream the consumer routes it to. Withholding still
    /// narrows it — a withheld fact emits no locator and is offered nowhere,
    /// which is the intended narrowing.
    ///
    /// Round 778 read "offered" as disclosed-as-prose and recorded the
    /// consequence honestly: a consumer whose
    /// [`journal_predicates`](EngineOverrides::journal_predicates) policy routes a
    /// precondition fact into its journal got a FALSE
    /// [`QuestGateViolation::PreconditionUnreachable`] for knowledge the store
    /// offers in time. A gate's verdict is about the store's facts, never about a
    /// consumer's rendering policy, so the reading moved rather than the doc.
    #[must_use]
    pub fn completability(&self, playable: &PlayableProjection) -> Vec<QuestGateViolation> {
        if self.telling != playable.telling() {
            return vec![QuestGateViolation::TellingMismatch {
                quest_telling: self.telling.clone(),
                playable_telling: playable.telling().to_string(),
            }];
        }
        let mut violations = Vec::new();
        for quest in &self.quests {
            if quest.preconditions.is_empty() {
                continue;
            }
            for (world, wv) in &quest.per_world {
                // A road the playable projection never heard of: the walk would
                // come back empty, every position lookup below would miss, and
                // the quest would leave this gate looking checked.
                if !playable.knows_world(world) {
                    violations.push(QuestGateViolation::WorldNotWalked {
                        world: world.clone(),
                        quest: quest.quest_id.clone(),
                    });
                    continue;
                }
                let walk = playable.walk_raw(world);
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
                    // knowledge must be dug before the quest discharges.
                    // `offers` is the knowledge axis (Round 787), not the prose
                    // stream: a precondition the consumer routes into its journal
                    // is still met on this walk, and reading `lines` here made
                    // this gate report it unreachable when it was not.
                    let in_time = walk
                        .iter()
                        .take(deadline)
                        .any(|section| playable.offers(world, section, need));
                    if !in_time {
                        violations.push(QuestGateViolation::PreconditionUnreachable {
                            world: world.clone(),
                            quest: quest.quest_id.clone(),
                            completion_scene: walk[deadline].to_string(),
                            needs: need.clone(),
                        });
                    }
                }
            }
        }
        violations
    }
}

/// A [`QuestCompletionView`] as plain data (Round 773) — the emit/ingest mirror,
/// for the same reason as [`LinePart`](crate::LinePart): the view is
/// `#[non_exhaustive]`, so generated code in another crate cannot construct one,
/// and a baked projection carries this instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestCompletionPart {
    /// The fact discharging the quest on this road.
    pub fact: String,
    /// The scene the quest completes at on this road.
    pub scene: String,
    /// The discharging actor the store names, when it names one.
    pub actor: Option<String>,
}

/// A [`QuestWorldView`] as plain data (Round 773).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestWorldPart {
    /// The derived verdict on this road.
    pub state: QuestState,
    /// The completion beat(s) discharging the quest here; empty when open.
    pub completions: Vec<QuestCompletionPart>,
}

/// A [`QuestView`] as plain data (Round 773).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestPart {
    /// The quest entity id.
    pub quest_id: String,
    /// The quest objective.
    pub objective: String,
    /// The actor entities that lead the quest, sorted.
    pub actors: Vec<String>,
    /// Prerequisite quest ids, sorted.
    pub prerequisites: Vec<String>,
    /// `world -> the quest's state + completion beat(s) there`, in world order.
    pub per_world: Vec<(String, QuestWorldPart)>,
    /// Completion-precondition facts, sorted + deduped.
    pub preconditions: Vec<String>,
}

/// A whole [`QuestProjection`] as plain data (Round 773) — the JOURNAL-axis
/// sibling of [`ProjectionParts`](crate::ProjectionParts), and the same shape for
/// the same reason.
///
/// Every field is already resolved: the quest-graph read and the precondition
/// join both happened when this was produced. So [`QuestProjection::from_parts`]
/// takes no `Result` — there is no untrusted input left to check it against. The
/// completability gate is NOT baked in and deliberately so: it is a question
/// asked of a [`PlayableProjection`] at call time, and R764 drew that line
/// already — a quest precondition evaluated against player state is a game rule
/// that stays at runtime. What moves to the build is the DERIVATION of the quest
/// layer, never the evaluation of its rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestProjectionParts {
    /// The telling this projection was resolved under.
    pub telling: String,
    /// Every quest the store declares, in quest-id order.
    pub quests: Vec<QuestPart>,
}

impl QuestProjection {
    /// Emit this projection as plain data (Round 773) — the bake half of the
    /// build-time seam.
    ///
    /// Deterministic without sorting anything here, which is worth stating rather
    /// than leaving to be rediscovered: `quests` arrives in quest-id order because
    /// the graph derives it from a `BTreeSet`, `per_world` is a `BTreeMap`, and
    /// `preconditions` were sorted and deduped at ingestion. The playable seam
    /// sorts because it holds `HashMap`s; this one would only be re-sorting sorted
    /// input.
    #[must_use]
    pub fn to_parts(&self) -> QuestProjectionParts {
        QuestProjectionParts {
            telling: self.telling.clone(),
            quests: self
                .quests
                .iter()
                .map(|q| QuestPart {
                    quest_id: q.quest_id.clone(),
                    objective: q.objective.clone(),
                    actors: q.actors.clone(),
                    prerequisites: q.prerequisites.clone(),
                    per_world: q
                        .per_world
                        .iter()
                        .map(|(world, wv)| {
                            (
                                world.clone(),
                                QuestWorldPart {
                                    state: wv.state,
                                    completions: wv
                                        .completions
                                        .iter()
                                        .map(|c| QuestCompletionPart {
                                            fact: c.fact.clone(),
                                            scene: c.scene.clone(),
                                            actor: c.actor.clone(),
                                        })
                                        .collect(),
                                },
                            )
                        })
                        .collect(),
                    preconditions: q.preconditions.clone(),
                })
                .collect(),
        }
    }

    /// Ingest baked parts (Round 773) — the read half, with no `Result` for the
    /// same reason [`PlayableProjection::from_parts`] has none: the store read
    /// and the precondition join already ran at bake time.
    ///
    /// This doc said what arrives "is the RESULT of the checks rather than their
    /// input", which is true of emitted parts and false of typed ones — the same
    /// sentence Round 791 corrected on the playable axis, in the same words, one
    /// module over. **This is a baked-ingestion door**; its contract is stated
    /// once in [`crate::baked_ingestion`] and this one adds nothing to it.
    #[must_use]
    pub fn from_parts(parts: QuestProjectionParts) -> Self {
        Self {
            telling: parts.telling,
            quests: parts
                .quests
                .into_iter()
                .map(|q| QuestView {
                    quest_id: q.quest_id,
                    objective: q.objective,
                    actors: q.actors,
                    prerequisites: q.prerequisites,
                    per_world: q
                        .per_world
                        .into_iter()
                        .map(|(world, wp)| {
                            (
                                world,
                                QuestWorldView {
                                    state: wp.state,
                                    completions: wp
                                        .completions
                                        .into_iter()
                                        .map(|c| QuestCompletionView {
                                            fact: c.fact,
                                            scene: c.scene,
                                            actor: c.actor,
                                        })
                                        .collect(),
                                },
                            )
                        })
                        .collect(),
                    preconditions: q.preconditions,
                })
                .collect(),
        }
    }
}

/// Read a consumer's completion-precondition claims from the store: for each
/// declared precondition predicate, collect its typed claims keyed by subject
/// (the quest) -> the object's fact id, the typed `opened_by = f-*` fact bridge
/// (R707/R708). Only a `TypedObject::Fact` object is a checkable precondition (a
/// fact the walk can offer); other object shapes are not. Empty predicate list =
/// no store read. Fails through the store read with a stringified error.
///
/// The scan itself is [`crate::store_typed_claims`] since Round 940, which was
/// this function's private loop before it became a public door. What stays here
/// is the quest axis's own rule — which object shape counts as a precondition —
/// because that is not a fact about the store.
///
/// FRAME AND BRANCH ARE DELIBERATELY NOT FILTERED, and this is the sentence that
/// says so rather than leaving a green run to imply it. The rows now arrive
/// carrying both coordinates, so filtering became possible in Round 940 and was
/// not done: no authored store discriminates. Every recorded precondition claim
/// is `ground-truth`, and each quest's claims sit on exactly one branch, so a
/// filter would be a rule written against no data (the R924 class — answering a
/// question nobody asked). The shape that WOULD discriminate is a precondition
/// declared in a belief frame, which would make what a character believes into a
/// gate the walk must satisfy; write one and this decision is due again.
fn read_preconditions(
    workspace_root: &Path,
    predicates: &[String],
) -> Result<BTreeMap<String, Vec<String>>, String> {
    let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    if predicates.is_empty() {
        return Ok(map);
    }
    let claims = crate::store_typed_claims(workspace_root).map_err(|e| e.to_string())?;
    for predicate in predicates {
        let Some(rows) = claims.get(predicate.as_str()) else {
            continue;
        };
        for row in rows {
            // A completion-precondition object is a typed FACT bridge (R707/R708
            // closed the object-shape: `opened_by = f-*`), so the gate can check
            // it against the facts the walk offers — only a `Fact` id joins
            // against a line's `fact_id`. A validated store carries no other
            // object shape under such a predicate (the R708 write-path gate), and
            // an entity/token id could never be an offered fact, so `Fact` is the
            // sole checkable shape.
            if let TypedObject::Fact { id } = &row.object {
                map.entry(row.subject.clone())
                    .or_default()
                    .push(id.to_string());
            }
        }
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use mnemosyne_core::DisclosureMode;
    use mnemosyne_validate::continuity::{ForkTreeReport, QuestState};
    use tempfile::TempDir;

    use crate::test_support::{
        begin, completion, journal_begin, locator, quest_node, quest_report, report, report_worlds,
        scene,
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

    /// Round 778 — a quest road the playable projection does not carry used to be
    /// skipped in silence, so the gate returned its clean verdict about a check it
    /// never ran. Both directions on ONE quest layer: paired with a projection
    /// that lacks the road it reports WorldNotWalked, paired with one that walks
    /// it it reports the real precondition finding. Either answer is fine; the
    /// empty vec was not.
    #[test]
    fn a_quest_road_the_playable_projection_lacks_is_reported_not_skipped() {
        let quests = QuestProjection::from_report(
            quest_report(vec![quest_node(
                "q-x",
                "the sin",
                &[],
                vec![(
                    "braid",
                    QuestState::Done,
                    vec![completion("f-done", "sc-02", None)],
                )],
            )]),
            &preconditions(&[("q-x", &["f-clue"])]),
        );
        let scenes = || {
            vec![
                scene(
                    "sc-01",
                    "Dawn",
                    vec![begin("f-a", "x", "ground-truth", &[])],
                ),
                scene("sc-02", "Gut", Vec::new()),
            ]
        };
        let locs = || vec![locator("f-a", "sc-01", DisclosureMode::State)];

        let only_main = PlayableProjection::from_report(
            report("main", scenes(), locs(), ForkTreeReport::default()),
            &StaticOverrides::default(),
        )
        .unwrap();
        assert_eq!(
            quests.completability(&only_main),
            vec![QuestGateViolation::WorldNotWalked {
                world: "braid".into(),
                quest: "q-x".into(),
            }],
            "a road the pair does not share must not pass as checked"
        );

        // The same quest layer against a projection that DOES walk braid: the
        // real finding comes back, so the new arm is not standing in for it.
        let with_braid = PlayableProjection::from_report(
            report_worlds(vec![("braid", scenes(), locs())], ForkTreeReport::default()),
            &StaticOverrides::default(),
        )
        .unwrap();
        assert_eq!(
            quests.completability(&with_braid),
            vec![QuestGateViolation::PreconditionUnreachable {
                world: "braid".into(),
                quest: "q-x".into(),
                completion_scene: "sc-02".into(),
                needs: "f-clue".into(),
            }],
        );
    }

    /// Round 787 — "offered" is what the store OFFERS, not what the prose stream
    /// shows. A controlled trio over one quest layer: only the journal policy and
    /// the precondition's locator scene vary.
    ///
    /// The middle arm is the defect Round 778 recorded and left open, and the
    /// third is why the fix is not simply the gate going quiet — a precondition
    /// that really is out of reach still fires under the same journal policy.
    #[test]
    fn a_journal_routed_precondition_is_offered_and_a_late_one_still_is_not() {
        let quests = QuestProjection::from_report(
            quest_report(vec![quest_node(
                "q-x",
                "the sin",
                &[],
                vec![(
                    "main",
                    QuestState::Done,
                    vec![completion("f-done", "sc-02", None)],
                )],
            )]),
            &preconditions(&[("q-x", &["f-clue"])]),
        );
        // `f-clue` is itself a typed journal leg, and `met_at` is the scene whose
        // locator discloses it — the only two things that vary below.
        let build = |met_at: &str| {
            report(
                "main",
                vec![
                    scene(
                        "sc-01",
                        "Dawn",
                        vec![journal_begin("f-clue", "pursues the vault key", "pursues")],
                    ),
                    scene("sc-02", "Gut", Vec::new()),
                ],
                vec![locator("f-clue", met_at, DisclosureMode::State)],
                ForkTreeReport::default(),
            )
        };
        let routing = || StaticOverrides {
            journal_predicates: vec!["pursues".to_string()],
            ..StaticOverrides::default()
        };

        // CONTROL — no journal policy: the fact is a prose line at sc-01 and the
        // gate is clean. Same store, same quest layer, same walk. Without this
        // arm the middle one would also pass on a gate that had stopped checking.
        let shown =
            PlayableProjection::from_report(build("sc-01"), &StaticOverrides::default()).unwrap();
        assert_eq!(
            quests.completability(&shown),
            vec![],
            "prose-disclosed in time: clean"
        );

        // THE DEFECT — the consumer routes `pursues` into its journal. The store
        // still offers f-clue at sc-01, strictly before the sc-02 completion;
        // only the stream carrying it changed. Reading `lines` here reported
        // PreconditionUnreachable for knowledge that is reachable.
        let routed = PlayableProjection::from_report(build("sc-01"), &routing()).unwrap();
        assert_eq!(
            quests.completability(&routed),
            vec![],
            "a journal-routed precondition is still MET on this walk"
        );

        // NON-VACUITY — same journal policy, precondition met only AT the
        // completion scene, so it is not in time. The gate must still fire, or
        // the arm above is just silence.
        let late = PlayableProjection::from_report(build("sc-02"), &routing()).unwrap();
        assert_eq!(
            quests.completability(&late),
            vec![QuestGateViolation::PreconditionUnreachable {
                world: "main".into(),
                quest: "q-x".into(),
                completion_scene: "sc-02".into(),
                needs: "f-clue".into(),
            }],
            "not-in-time must still fire under the same journal policy"
        );
    }

    /// Round 778 — the two projections each carry the telling they were resolved
    /// under and nothing compared them, though every constructor takes it as its
    /// own argument. A mismatched pair reports the mismatch ALONE: the findings it
    /// would otherwise produce are statements about the wrong world, and a list of
    /// those reads as work done.
    #[test]
    fn two_projections_cut_for_different_tellings_report_the_mismatch_alone() {
        let quests = QuestProjection::from_report(
            quest_report(vec![quest_node(
                "q-x",
                "the sin",
                &[],
                vec![(
                    "main",
                    QuestState::Done,
                    vec![completion("f-done", "sc-02", None)],
                )],
            )]),
            &preconditions(&[("q-x", &["f-clue"])]),
        );
        let mut other = report(
            "main",
            vec![
                scene(
                    "sc-01",
                    "Dawn",
                    vec![begin("f-a", "x", "ground-truth", &[])],
                ),
                scene("sc-02", "Gut", Vec::new()),
            ],
            vec![locator("f-a", "sc-01", DisclosureMode::State)],
            ForkTreeReport::default(),
        );
        // Same store, same walk, same quest — only the telling differs, which is
        // exactly the pair a bake produces from two emitter calls.
        other.telling = "player".to_string();
        let playable = PlayableProjection::from_report(other, &StaticOverrides::default()).unwrap();

        assert_eq!(
            quests.completability(&playable),
            vec![QuestGateViolation::TellingMismatch {
                quest_telling: "reader".into(),
                playable_telling: "player".into(),
            }],
            "the mismatch is the only thing worth reporting about this pair"
        );
        // Non-vacuity: matched tellings and the SAME walk still produce the real
        // finding, so the guard is not swallowing the gate.
        let matched = PlayableProjection::from_report(
            report(
                "main",
                vec![
                    scene(
                        "sc-01",
                        "Dawn",
                        vec![begin("f-a", "x", "ground-truth", &[])],
                    ),
                    scene("sc-02", "Gut", Vec::new()),
                ],
                vec![locator("f-a", "sc-01", DisclosureMode::State)],
                ForkTreeReport::default(),
            ),
            &StaticOverrides::default(),
        )
        .unwrap();
        assert_eq!(quests.completability(&matched).len(), 1);
        assert!(matches!(
            quests.completability(&matched)[0],
            QuestGateViolation::PreconditionUnreachable { .. }
        ));
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

    /// Round 940 — the STORE-READING wrapper, which had no in-repo test at all
    /// until the round that rewired its scan onto the public typed-claim door.
    /// Only the first consumer's build exercised it, so a change here was
    /// answerable only by a tree we do not run.
    ///
    /// Three arms over one built workspace, because the rule this function owns
    /// is which typed claims are preconditions and the arms are what distinguish
    /// it from "all of them":
    ///
    /// - `opened_by = f-*` (a FACT object) under a declared predicate IS a
    ///   precondition — a fact the walk can offer, joinable against a line.
    /// - `requires = q-*` (an ENTITY object) under an EQUALLY DECLARED predicate
    ///   is NOT, and it is declared on the same quest, so a dropped shape filter
    ///   would show up as `q-other` sitting in this quest's preconditions rather
    ///   than as an entry nothing looks up.
    /// - `debunked_by = f-*`, a FACT object on the same quest under a predicate
    ///   the consumer did NOT declare, is not read at all. This arm exists
    ///   because the first version of this test did not have it and the policy
    ///   went untested: every undeclared claim in the fixture was entity-shaped,
    ///   so the shape filter caught them and dropping the predicate policy
    ///   entirely changed no answer. An injection found that, not a reading of it.
    #[test]
    fn from_workspace_reads_preconditions_through_the_typed_claim_door() {
        let tmp = TempDir::new().expect("scratch workspace");
        let root = tmp.path();
        std::fs::create_dir_all(root.join("docs/.atomic")).expect("sidecar dir");
        std::fs::write(
            root.join("mnemosyne.toml"),
            "[workspace]\n[continuity]\ncanon_order_path = \"order.json\"\n",
        )
        .expect("config");
        std::fs::write(
            root.join("order.json"),
            r#"{"edges": [["sc-01", "sc-02"]]}"#,
        )
        .expect("canon order");

        let sidecar = mnemosyne_atomic::AtomicStore::default_sidecar_path(root);
        let mut store = mnemosyne_atomic::AtomicStore::default();
        let sections: Vec<mnemosyne_atomic::SectionImport> = serde_json::from_str(
            r#"[{"section_id":"sc-01","parent_doc":"M.md","title":"the sealed door"},
                {"section_id":"sc-02","parent_doc":"M.md","title":"the key"}]"#,
        )
        .expect("sections manifest parses");
        mnemosyne_atomic::import_sections(&mut store, &sidecar, &sections).expect("sections");

        let facts: mnemosyne_atomic::FactsManifest = serde_json::from_str(
            r#"{
              "frames": [{"frame_id": "gt", "description": "what is so"}],
              "entity_kinds": [
                {"kind_id": "character", "description": "a person"},
                {"kind_id": "quest", "description": "an errand"}
              ],
              "entities": [
                {"entity_id": "hero", "kind": "character", "description": "the hero"},
                {"entity_id": "q-door", "kind": "quest", "description": "open the sealed door"},
                {"entity_id": "q-other", "kind": "quest", "description": "the errand before it"}
              ],
              "predicates": [
                {"predicate_id": "pursues", "object_kind": "entity", "description": "who leads a quest"},
                {"predicate_id": "requires", "object_kind": "entity", "description": "a quest gated by another"},
                {"predicate_id": "opened_by", "object_kind": "fact", "description": "the knowledge that opens a quest"},
                {"predicate_id": "debunked_by", "object_kind": "fact", "description": "the knowledge that ends a rumour"},
                {"predicate_id": "holds", "object_kind": "entity", "description": "who carries what"}
              ],
              "facts": [
                {"fact_id":"f-key","frame":"gt","entities":["hero"],
                 "claim":"the hero learns the key is in the well","canon_from":"sc-02","evidence":["sc-02"]},
                {"fact_id":"f-rumour","frame":"gt","entities":["hero"],
                 "claim":"the hero hears the door was never sealed at all","canon_from":"sc-02",
                 "evidence":["sc-02"]},
                {"fact_id":"f-debunk","frame":"gt","entities":["q-door","hero"],
                 "claim":"the tale of the unsealed door dies at the door itself","canon_from":"sc-01",
                 "evidence":["sc-01"],
                 "typed":{"subject":"q-door","predicate":"debunked_by","object":{"kind":"fact","id":"f-rumour"}}},
                {"fact_id":"f-pursue","frame":"gt","entities":["hero","q-door"],
                 "claim":"the hero takes on the sealed door","canon_from":"sc-01","evidence":["sc-01"],
                 "typed":{"subject":"hero","predicate":"pursues","object":{"kind":"entity","id":"q-door"}}},
                {"fact_id":"f-opens","frame":"gt","entities":["q-door","hero"],
                 "claim":"the sealed door opens to whoever knows where the key lies","canon_from":"sc-01",
                 "evidence":["sc-01"],
                 "typed":{"subject":"q-door","predicate":"opened_by","object":{"kind":"fact","id":"f-key"}}},
                {"fact_id":"f-requires","frame":"gt","entities":["q-door","q-other"],
                 "claim":"the sealed door waits on the earlier errand","canon_from":"sc-01","evidence":["sc-01"],
                 "typed":{"subject":"q-door","predicate":"requires","object":{"kind":"entity","id":"q-other"}}},
                {"fact_id":"f-holds","frame":"gt","entities":["q-door","hero"],
                 "claim":"the door's errand rests with the hero","canon_from":"sc-01","evidence":["sc-01"],
                 "typed":{"subject":"q-door","predicate":"holds","object":{"kind":"entity","id":"hero"}}}
              ],
              "disclosure_plans": [{"telling_id":"t1","description":"the reader's telling"}]
            }"#,
        )
        .expect("facts manifest parses");
        mnemosyne_atomic::import_facts(&mut store, &sidecar, &facts).expect("facts");

        let declared = StaticOverrides {
            // BOTH shapes declared, so the filter is what separates them.
            quest_precondition_predicates: vec!["opened_by".to_string(), "requires".to_string()],
            ..StaticOverrides::default()
        };
        let quests =
            QuestProjection::from_workspace(root, "t1", None, &declared).expect("the store reads");
        let door = quests.quest("q-door").expect("the pursued quest projects");
        assert_eq!(
            door.preconditions,
            vec!["f-key".to_string()],
            "only the FACT-object claim is a checkable precondition"
        );

        // The undeclared-predicate arm, and the one that has to be FACT-shaped:
        // `debunked_by = f-rumour` names this same quest with the same object
        // shape a precondition has, so the ONLY thing keeping it out is the
        // consumer's policy. An entity-shaped claim here would have been caught
        // by the shape filter and proved nothing.
        assert!(
            !door.preconditions.iter().any(|p| p == "f-rumour"),
            "a fact-shaped claim under an undeclared predicate is not a precondition"
        );

        // And declaring NOTHING reads no preconditions at all — the empty policy
        // is a policy, not a store that happens to be empty.
        let silent = QuestProjection::from_workspace(root, "t1", None, &StaticOverrides::default())
            .expect("the store reads");
        assert!(silent
            .quest("q-door")
            .expect("the quest still projects")
            .preconditions
            .is_empty());
    }

    /// Round 773 — the build-time seam's whole claim on the quest axis: a
    /// projection that went out through `to_parts` and came back through the
    /// INFALLIBLE `from_parts` answers every public query identically, INCLUDING
    /// the completability gate, which is the one that matters — the gate is not
    /// baked (it is a rule evaluated at call time against a playable projection),
    /// so a baked quest layer must still be able to answer it.
    #[test]
    fn a_baked_quest_projection_answers_exactly_as_the_projected_one() {
        // Thick enough that every part-carrying field is POPULATED: two quests,
        // two roads, a completion with an actor and one without, prerequisites,
        // actors, and preconditions. An empty field would let a dropped one pass.
        let mut node = quest_node(
            "q-knot-1",
            "the first sin",
            &["q-salt"],
            vec![
                (
                    "main",
                    QuestState::Done,
                    vec![completion("f-confess", "sc-gut", Some("ent-eldest"))],
                ),
                (
                    "fork",
                    QuestState::Done,
                    vec![completion("f-confess", "sc-gut", None)],
                ),
            ],
        );
        node.actors = vec!["ent-jiun".to_string()];
        let live = QuestProjection::from_report(
            quest_report(vec![
                node,
                quest_node(
                    "q-salt",
                    "the salt debt",
                    &[],
                    vec![("main", QuestState::Open, Vec::new())],
                ),
            ]),
            &preconditions(&[("q-knot-1", &["f-clue"])]),
        );
        assert!(
            !live.quests()[0].actors.is_empty() && !live.quests()[0].preconditions.is_empty(),
            "fixture must carry actors and preconditions"
        );

        let baked = QuestProjection::from_parts(live.to_parts());
        assert_eq!(baked.telling(), live.telling());
        assert_eq!(baked.quests(), live.quests());
        for q in live.quests() {
            assert_eq!(baked.quest(&q.quest_id), live.quest(&q.quest_id));
        }

        // The gate agrees on a REAL verdict, not on two empty vectors: `main`
        // digs the clue before the gut, `fork` never does, so the same
        // one-violation answer must come out of both.
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
                            scene("sc-gut", "Gut", Vec::new()),
                        ],
                        vec![locator("f-clue", "sc-01", DisclosureMode::State)],
                    ),
                    (
                        "fork",
                        vec![
                            scene("sc-01", "Dawn", Vec::new()),
                            scene("sc-gut", "Gut", Vec::new()),
                        ],
                        Vec::new(),
                    ),
                ],
                ForkTreeReport::default(),
            ),
            &StaticOverrides::default(),
        )
        .unwrap();
        let verdict = live.completability(&playable);
        assert_eq!(verdict.len(), 1, "the fixture must produce a real verdict");
        assert_eq!(baked.completability(&playable), verdict);

        // And the emit is deterministic: the same parts twice over.
        assert_eq!(live.to_parts(), baked.to_parts());
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
