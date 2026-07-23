//! The playable projection: read `report-playable-world` and index it into a
//! per-world, per-section stream of disclosed [`Line`]s plus the walk and fork
//! topology. The generalization of tide's `narrative.rs` — content-, telling-,
//! and presentation-agnostic.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use mnemosyne_core::MAIN_BRANCH;
use mnemosyne_validate::continuity::{ManuscriptFactEvent, PlayableWorldReport};

use crate::{
    CastMember, ChoiceEntityRef, Door, EngineError, EngineOverrides, Fork, Interactivity, Line,
    Passage, Rung, RungQuestionFault, SceneView,
};

/// Per-world, per-section disclosed narrative + the declared walk + fork
/// topology, projected from one `report-playable-world` read and configured with
/// the consumer's interactive layer. Everything narrative comes from the store;
/// the kernel adds no authoritative state.
#[derive(Debug, Clone)]
pub struct PlayableProjection {
    telling: String,
    by_world: HashMap<String, HashMap<String, Vec<Line>>>,
    walks: HashMap<String, Vec<String>>,
    titles: HashMap<String, String>,
    /// Section id -> the store-owned cast present there (Round 757, B1b). Keyed by
    /// section: presence is authored world-truth on the shared section, so it is
    /// world-independent (every world walking a section sees the same cast).
    cast: HashMap<String, Vec<CastMember>>,
    forks: Vec<Fork>,
    divergent_endings: HashSet<String>,
    interactivity: Interactivity,
    choice_entity_refs: Vec<ChoiceEntityRef>,
    /// Section id -> the resolved ask doors dug there (R759 P3c-2). Built once at
    /// construction: an anchored rung's question is resolved from the section's
    /// store `content_excerpt` (fail-loud), an un-anchored rung keeps its free
    /// question. World-independent (ladders + excerpts are both keyed by section),
    /// so `doors_at` appends these regardless of world.
    ask_doors: HashMap<String, Vec<Door>>,
}

impl PlayableProjection {
    /// Project the workspace store under `telling`, reading the playable world
    /// in-process (no JSON round-trip): the shared order resolution + the
    /// disclosure resolver, exactly as `report-playable-world` emits.
    ///
    /// # Errors
    ///
    /// [`EngineError::Projection`] if the underlying playable-world read fails
    /// (unregistered world, typo'd telling, unreadable store);
    /// [`EngineError::LocatorFactMissing`] if a locator dangles (a stale read).
    pub fn from_workspace(
        workspace_root: &Path,
        telling: &str,
        overrides: &impl EngineOverrides,
    ) -> Result<Self, EngineError> {
        let report =
            mnemosyne_ops::playable_world_report(workspace_root, None, None, None, telling)
                .map_err(|e| EngineError::Projection(e.to_string()))?;
        // The store's per-section content prose (R757 P3b), the second store
        // projection an anchored ladder question resolves against.
        let passages = crate::store_passages(workspace_root)?;
        Self::from_report_and_passages(report, overrides, passages)
    }

    /// Index an already-projected [`PlayableWorldReport`] — the pure testable core
    /// ([`Self::from_workspace`] is the store-reading wrapper). No store content
    /// prose is supplied here, so a rung that declares a `question_anchor` cannot
    /// resolve (fail-loud) — anchored questions are a [`Self::from_workspace`]
    /// concern (the store owns the prose); the report path renders only
    /// un-anchored questions.
    ///
    /// # Errors
    ///
    /// [`EngineError::LocatorFactMissing`] if any locator names a `fact_id` no
    /// `begins` event carries (a stale report), never a silent drop;
    /// [`EngineError::RungQuestionUnresolvable`] if any rung declares a
    /// `question_anchor` (unresolvable here — no store prose).
    pub fn from_report(
        report: PlayableWorldReport,
        overrides: &impl EngineOverrides,
    ) -> Result<Self, EngineError> {
        Self::from_report_and_passages(report, overrides, HashMap::new())
    }

    /// The store-aware core: index a report AND the store's per-section content
    /// prose, so an anchored ladder question ([`Rung::question_anchor`]) resolves
    /// against the section's [`Passage`]. Both public constructors delegate here —
    /// [`Self::from_report`] with no passages, [`Self::from_workspace`] with the
    /// live store's. Crate-internal: the only injection point for passages is the
    /// real store read (or an in-crate test), never a downstream fabrication.
    ///
    /// # Errors
    ///
    /// [`EngineError::LocatorFactMissing`] for a stale locator;
    /// [`EngineError::RungQuestionUnresolvable`] if an anchored rung's question
    /// prose is not backed by the section's store excerpt.
    pub(crate) fn from_report_and_passages(
        report: PlayableWorldReport,
        overrides: &impl EngineOverrides,
        passages: HashMap<String, Passage>,
    ) -> Result<Self, EngineError> {
        let journal = overrides.journal_predicates();
        let PlayableWorldReport {
            telling,
            fork_tree,
            worlds,
        } = report;
        let mut by_world = HashMap::new();
        let mut walks = HashMap::new();
        let mut titles = HashMap::new();
        let mut cast: HashMap<String, Vec<CastMember>> = HashMap::new();

        for (world_name, world) in worlds {
            walks.insert(
                world_name.clone(),
                world
                    .manuscript
                    .scenes
                    .iter()
                    .map(|s| s.section.clone())
                    .collect(),
            );

            // `begins` = what each fact IS (claim/frame/entities/typed). The
            // seat comes from the locator, not from here (`canon_from` is where
            // the novel first says it, not where the player meets it). Empty
            // section slots are kept: a fact-less spot is still a spot (a part
            // epigraph carries `begins: []`).
            let mut facts: HashMap<String, ManuscriptFactEvent> = HashMap::new();
            let mut by_section: HashMap<String, Vec<Line>> = HashMap::new();
            for scene in &world.manuscript.scenes {
                if !scene.title.is_empty() {
                    titles
                        .entry(scene.section.clone())
                        .or_insert_with(|| scene.title.clone());
                }
                by_section.entry(scene.section.clone()).or_default();
                // Presence is world-independent (authored on the shared section),
                // so the first world to carry a section fixes its cast; later
                // worlds see identical scene_cast. Provenance-bound via
                // `CastMember::from_presence`.
                cast.entry(scene.section.clone()).or_insert_with(|| {
                    scene
                        .scene_cast
                        .iter()
                        .map(CastMember::from_presence)
                        .collect()
                });
                for begin in &scene.begins {
                    facts.insert(begin.fact_id.clone(), begin.clone());
                }
            }

            // `locators` = where the audience MEETS each fact under this
            // telling. Withheld facts are absent (they emit no locator), so
            // there is no subtractive filter here. A locator whose fact is
            // missing is a stale report — a hard error, never a silent drop.
            for locator in &world.locators {
                let begin =
                    facts
                        .get(&locator.fact_id)
                        .ok_or_else(|| EngineError::LocatorFactMissing {
                            world: world_name.clone(),
                            fact_id: locator.fact_id.clone(),
                        })?;
                // Journal facts (quest legs) are the game's own ledger, not
                // world prose — route them out of the line stream (still
                // queryable elsewhere). The fail-loud join above runs FIRST, so
                // a stale journal locator is still caught.
                if let Some(typed) = &begin.typed {
                    if journal.contains(&typed.predicate) {
                        continue;
                    }
                }
                by_section
                    .entry(locator.scene.clone())
                    .or_default()
                    .push(Line::from_disclosed(locator, begin));
            }

            by_world.insert(world_name, by_section);
        }

        // A divergent ending = a world that forks and never reconverges (no one
        // merges FROM it). A converge edge names its parents; a branch listed as
        // some converge's parent feeds back into a confluence, so it is NOT a
        // divergent ending. Derived from the fork tree, never hardcoded.
        let converge_parents: HashSet<&str> = fork_tree
            .branches
            .iter()
            .flat_map(|b| b.converges.iter().map(|e| e.parent.as_str()))
            .collect();
        let divergent_endings: HashSet<String> = fork_tree
            .branches
            .iter()
            .filter(|b| b.fork.is_some() && !converge_parents.contains(b.branch_id.as_str()))
            .map(|b| b.branch_id.clone())
            .collect();
        let forks = fork_tree
            .branches
            .into_iter()
            .filter_map(|b| {
                b.fork
                    .map(|edge| Fork::new(edge.at, edge.parent, b.branch_id, b.description))
            })
            .collect();

        // Resolve the ask doors ONCE, fail-loud: an anchored rung's question is
        // the section's store prose (never the free string), a bare rung keeps its
        // authored question. Building here (not lazily in `doors_at`) keeps the
        // read path infallible and localizes the provenance failure to construction.
        let mut ask_doors: HashMap<String, Vec<Door>> = HashMap::new();
        for (section, rungs) in &overrides.interactivity().ladders {
            let mut resolved = Vec::with_capacity(rungs.len());
            for rung in rungs {
                resolved.push(Door::Ask {
                    question: resolve_rung_question(section, rung, &passages)?,
                    reveals: rung.reveals.clone(),
                });
            }
            ask_doors.insert(section.clone(), resolved);
        }

        Ok(Self {
            telling,
            by_world,
            walks,
            titles,
            cast,
            forks,
            divergent_endings,
            interactivity: overrides.interactivity().clone(),
            choice_entity_refs: overrides.choice_entity_refs().to_vec(),
            ask_doors,
        })
    }

    /// The telling this projection was cut for.
    #[must_use]
    pub fn telling(&self) -> &str {
        &self.telling
    }

    /// The disclosed narrative for one world-line at one section, in manuscript
    /// order. Empty when nothing is disclosed there (or the world/section is
    /// unknown).
    #[must_use]
    pub fn lines(&self, world: &str, section: &str) -> &[Line] {
        self.by_world
            .get(world)
            .and_then(|w| w.get(section))
            .map_or(&[][..], Vec::as_slice)
    }

    /// The one spot bundled as a [`SceneView`]: disclosed `lines` plus `doors`
    /// (fork choices + examine objects + authored ladder rungs, from the
    /// projection's configured interactivity). Narrative content is exclusively
    /// the provenance-bound `lines`; a door reveals `fact_id`s, never free text.
    #[must_use]
    pub fn scene(&self, world: &str, section: &str) -> SceneView {
        SceneView::new(
            section.to_string(),
            self.title(section).map(str::to_string),
            self.lines(world, section).to_vec(),
            self.doors_at(world, section),
        )
    }

    /// The interactive affordances at one spot: fork choices (store-native) +
    /// examine doors (a disclosed line's entity that is a registered object) +
    /// ask doors (the authored ladder rungs at this section). Deterministic
    /// order: forks, then examine (object-sorted), then rungs (authored order).
    #[must_use]
    pub(crate) fn doors_at(&self, world: &str, section: &str) -> Vec<Door> {
        let mut doors = Vec::new();

        for fork in self.forks_at(section, world) {
            doors.push(Door::Fork {
                world: fork.world.clone(),
                label: fork.label.clone(),
            });
        }

        // Examine doors: an offered fact whose entity is a registered object.
        // Group the offered fact_ids by object (object-sorted for determinism);
        // the reveals are a subset of the disclosed lines, so they cannot leak.
        let mut by_object: BTreeMap<&str, Vec<String>> = BTreeMap::new();
        for line in self.lines(world, section) {
            for entity in &line.entities {
                if self.interactivity.objects.contains(entity) {
                    by_object
                        .entry(entity.as_str())
                        .or_default()
                        .push(line.fact_id.clone());
                }
            }
        }
        for (object, reveals) in by_object {
            doors.push(Door::Examine {
                object: object.to_string(),
                reveals,
            });
        }

        // Ask doors: the ladder rungs at this section, in authored order, with
        // their questions already resolved at construction (R759 P3c-2 — an
        // anchored question is the section's store prose, fail-loud there; a bare
        // one is un-anchored chrome). The leak gate (never construction) still
        // enforces that a rung's reveal is a fact the store offers here.
        if let Some(ask) = self.ask_doors.get(section) {
            doors.extend(ask.iter().cloned());
        }

        doors
    }

    /// The authored ladder rungs at one section (empty when the spot has no
    /// ladder) — the crate-internal accessor the gate reads for the leak /
    /// reachability / precondition checks.
    pub(crate) fn rungs_at(&self, section: &str) -> &[Rung] {
        self.interactivity
            .ladders
            .get(section)
            .map_or(&[][..], Vec::as_slice)
    }

    /// Whether the configured interactive layer is PARTIAL (a free fallback
    /// reveals a ladder spot's remainder) rather than MODAL — the gate reads it to
    /// decide whether the offered-fact-unreachable check applies. See
    /// [`Interactivity::free_investigate`](crate::Interactivity::free_investigate).
    pub(crate) fn free_investigate(&self) -> bool {
        self.interactivity.free_investigate
    }

    /// The store-declared walk for a world-line — the section sequence the
    /// player's own traversal must match (the reachability yardstick). Empty for
    /// an unknown world.
    #[must_use]
    pub fn walk(&self, world: &str) -> &[String] {
        self.walks.get(world).map_or(&[][..], Vec::as_slice)
    }

    /// The main trunk's walk — the spine every world-line shares (each differs
    /// only in what is true at each spot, not in the section order).
    #[must_use]
    pub fn spine(&self) -> &[String] {
        self.walk(MAIN_BRANCH)
    }

    /// The index of `section` on the spine, if present.
    #[must_use]
    pub fn cursor_of(&self, section: &str) -> Option<usize> {
        self.spine().iter().position(|s| s == section)
    }

    /// The store section title for a section id (world-independent).
    #[must_use]
    pub fn title(&self, section: &str) -> Option<&str> {
        self.titles.get(section).map(String::as_str)
    }

    /// Is this world-line a divergent ending (a fork that never reconverges)?
    /// Such a world has no canon trunk prose past its fork, so a renderer falls
    /// back to that world's own fact-claims. Derived from the fork tree.
    #[must_use]
    pub fn is_divergent_ending(&self, world: &str) -> bool {
        self.divergent_endings.contains(world)
    }

    /// The forks that open at `section` while walking `world` (their parent).
    #[must_use]
    pub fn forks_at(&self, section: &str, world: &str) -> Vec<&Fork> {
        self.forks
            .iter()
            .filter(|f| f.at == section && f.parent == world)
            .collect()
    }

    /// The entities the discourse has DISCLOSED at-or-before `section` on
    /// `world`'s walk (Round 757, B1) — the union of every disclosed
    /// [`Line`]'s entities from the world's first spot through `section`,
    /// inclusive. Sorted, unique. The §5 discourse-order invariant: an entity is
    /// referenceable at a spot only once the player has MET it (it appeared in a
    /// disclosed line there or earlier), so a consumer's choice may name only
    /// these — a reference to anything else is the field-report parallel-identity
    /// class, caught by
    /// [`GateViolation::ChoiceReferencesUndisclosedEntity`](crate::GateViolation::ChoiceReferencesUndisclosedEntity).
    /// Empty when `world` does not walk `section` (there is no at-or-before).
    /// A withheld fact emits no line, so a withheld entity is not referenceable —
    /// disclosure is additive here too.
    #[must_use]
    pub fn referenceable_entities(&self, world: &str, section: &str) -> Vec<String> {
        let walk = self.walk(world);
        let Some(pos) = walk.iter().position(|s| s == section) else {
            return Vec::new();
        };
        let mut set: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
        for s in &walk[..=pos] {
            for line in self.lines(world, s) {
                for entity in &line.entities {
                    set.insert(entity.as_str());
                }
            }
        }
        set.into_iter().map(String::from).collect()
    }

    /// The consumer-declared choice→entity references (Round 757, B1) — the gate
    /// reads these to enforce the disclosure invariant on interactive choices.
    pub(crate) fn choice_entity_refs(&self) -> &[ChoiceEntityRef] {
        &self.choice_entity_refs
    }

    /// The store-owned cast present at `section` (Round 757, B1b) — WHO is in the
    /// scene, with the authored `modality`/`can_answer` and a provenance quote,
    /// projected from `AtomicSection.scene_cast`. The ONLY cast source a consumer
    /// reads (instead of building its own presence space the kernel cannot see —
    /// the field-report class). World-independent: presence is authored on the
    /// shared section, so every world walking `section` sees the same cast. Empty
    /// for a section with no authored presence (or an unknown section).
    #[must_use]
    pub fn cast_at(&self, section: &str) -> &[CastMember] {
        self.cast.get(section).map_or(&[][..], Vec::as_slice)
    }
}

/// Resolve a rung's rendered question, fail-loud (R759 P3c-2). An UN-ANCHORED rung
/// renders its free `question` verbatim (interactive chrome). An ANCHORED rung's
/// question is the section's store `content_excerpt` text — and the declared
/// anchor MUST match that excerpt's anchor, so a false provenance claim (a rung
/// pointing at prose the section does not hold, or a section with no excerpt) is
/// [`EngineError::RungQuestionUnresolvable`], never a silent fall-back to the free
/// string. This is the store-resolution that makes a fabricated door label
/// inexpressible for a provenance-bound consumer.
fn resolve_rung_question(
    section: &str,
    rung: &Rung,
    passages: &HashMap<String, Passage>,
) -> Result<String, EngineError> {
    let Some(anchor) = &rung.question_anchor else {
        return Ok(rung.question.clone());
    };
    let passage = passages
        .get(section)
        .ok_or_else(|| EngineError::RungQuestionUnresolvable {
            section: section.to_string(),
            anchor: anchor.clone(),
            reason: RungQuestionFault::SectionHasNoExcerpt,
        })?;
    if passage.anchor() != anchor {
        return Err(EngineError::RungQuestionUnresolvable {
            section: section.to_string(),
            anchor: anchor.clone(),
            reason: RungQuestionFault::AnchorMismatch,
        });
    }
    Ok(passage.text().to_string())
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use mnemosyne_core::DisclosureMode;
    use mnemosyne_validate::continuity::ForkTreeReport;

    use crate::test_support::{
        begin, branch, cast_scene, journal_begin, locator, presence, report, rung, scene,
    };
    use crate::{
        ContentAnchor, DefaultOverrides, Door, EngineError, Interactivity, Locator, Modality,
        Passage, PlayableProjection, Rung, RungQuestionFault, StaticOverrides,
    };

    #[test]
    fn projects_disclosed_lines_at_the_locator_seat() {
        let r = report(
            "main",
            vec![
                scene(
                    "sc-01",
                    "Dawn",
                    vec![begin(
                        "f-a",
                        "the tide pulls out",
                        "ground-truth",
                        &["tide"],
                    )],
                ),
                scene(
                    "sc-02",
                    "Noon",
                    vec![begin(
                        "f-b",
                        "Bunok guesses a name",
                        "frame-bunok",
                        &["bunok"],
                    )],
                ),
            ],
            vec![
                locator("f-a", "sc-01", DisclosureMode::State),
                locator("f-b", "sc-02", DisclosureMode::Hint),
            ],
            ForkTreeReport::default(),
        );
        let proj = PlayableProjection::from_report(r, &DefaultOverrides::default()).unwrap();

        let sc01 = proj.lines("main", "sc-01");
        assert_eq!(sc01.len(), 1);
        assert_eq!(sc01[0].fact_id, "f-a");
        assert_eq!(sc01[0].text, "the tide pulls out");
        assert_eq!(sc01[0].mode, DisclosureMode::State);
        assert_eq!(sc01[0].entities, vec!["tide".to_string()]);
        assert!(!sc01[0].is_belief());

        let sc02 = proj.lines("main", "sc-02");
        assert_eq!(sc02[0].fact_id, "f-b");
        assert!(sc02[0].is_belief()); // frame-bunok != ground-truth
        assert!(sc01[0].is_ground_truth());
        assert!(!sc02[0].is_ground_truth());
        assert_eq!(sc01[0].quote, None); // styling hooks default when unauthored
        assert_eq!(sc01[0].count, None);

        assert_eq!(
            proj.walk("main"),
            &["sc-01".to_string(), "sc-02".to_string()]
        );
        assert_eq!(proj.title("sc-01"), Some("Dawn"));
        assert_eq!(proj.cursor_of("sc-02"), Some(1));
        assert_eq!(proj.telling(), "reader");

        let view = proj.scene("main", "sc-01");
        assert_eq!(view.section, "sc-01");
        assert_eq!(view.title.as_deref(), Some("Dawn"));
        assert_eq!(view.lines.len(), 1);
        assert!(view.doors.is_empty()); // no fork tree, no interactivity
    }

    #[test]
    fn a_stale_locator_is_a_hard_error_never_a_silent_drop() {
        let r = report(
            "main",
            vec![scene(
                "sc-01",
                "Dawn",
                vec![begin("f-a", "x", "ground-truth", &[])],
            )],
            vec![locator("f-ghost", "sc-01", DisclosureMode::State)],
            ForkTreeReport::default(),
        );
        let err = PlayableProjection::from_report(r, &DefaultOverrides::default()).unwrap_err();
        assert_eq!(
            err,
            EngineError::LocatorFactMissing {
                world: "main".into(),
                fact_id: "f-ghost".into(),
            }
        );
    }

    #[test]
    fn a_withheld_fact_never_becomes_a_line() {
        // `f-secret` is in `begins` but has NO locator (withheld facts emit
        // none). The kernel never re-adds it via a subtractive filter.
        let r = report(
            "main",
            vec![scene(
                "sc-01",
                "Dawn",
                vec![
                    begin("f-open", "spoken aloud", "ground-truth", &[]),
                    begin("f-secret", "the hidden truth", "ground-truth", &[]),
                ],
            )],
            vec![locator("f-open", "sc-01", DisclosureMode::State)],
            ForkTreeReport::default(),
        );
        let proj = PlayableProjection::from_report(r, &DefaultOverrides::default()).unwrap();
        let lines = proj.lines("main", "sc-01");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].fact_id, "f-open");
        assert!(lines.iter().all(|l| l.fact_id != "f-secret"));
    }

    #[test]
    fn forks_and_divergent_endings_derive_from_the_fork_tree() {
        let fork_tree = ForkTreeReport {
            branches: vec![
                branch("flee", "run for the causeway", "main", "sc-02", &[]),
                branch("weave", "stay and count", "main", "sc-02", &["flee"]),
            ],
            ..Default::default()
        };
        let r = report(
            "main",
            vec![scene("sc-02", "Noon", Vec::new())],
            Vec::new(),
            fork_tree,
        );
        let proj = PlayableProjection::from_report(r, &DefaultOverrides::default()).unwrap();

        let forks = proj.forks_at("sc-02", "main");
        assert_eq!(forks.len(), 2);
        assert!(forks
            .iter()
            .any(|f| f.world == "flee" && f.label == "run for the causeway"));

        // `flee` forks and IS a converge parent of `weave` -> NOT divergent.
        // `weave` forks and no one converges from it -> a divergent ending.
        assert!(!proj.is_divergent_ending("flee"));
        assert!(proj.is_divergent_ending("weave"));
        // A world that never forks is not a divergent ending.
        assert!(!proj.is_divergent_ending("main"));
    }

    #[test]
    fn scene_derives_fork_examine_and_ask_doors() {
        let build = || {
            report(
                "main",
                vec![scene(
                    "sc-01",
                    "Dawn",
                    vec![
                        begin(
                            "f-table",
                            "the tide table hangs there",
                            "ground-truth",
                            &["tide-table"],
                        ),
                        begin("f-name", "the name is Yeonggeun", "ground-truth", &[]),
                    ],
                )],
                vec![
                    locator("f-table", "sc-01", DisclosureMode::State),
                    locator("f-name", "sc-01", DisclosureMode::Hint),
                ],
                ForkTreeReport {
                    branches: vec![branch("flee", "run", "main", "sc-01", &[])],
                    ..Default::default()
                },
            )
        };

        // Configured with interactivity: fork + examine + ask doors.
        let overrides = StaticOverrides {
            interactivity: Interactivity {
                objects: HashSet::from(["tide-table".to_string()]),
                ladders: HashMap::from([(
                    "sc-01".to_string(),
                    vec![rung("Whose name?", "f-name", &[])],
                )]),
                free_investigate: false,
            },
            journal_predicates: Vec::new(),
            quest_precondition_predicates: Vec::new(),
            choice_entity_refs: Vec::new(),
        };
        let proj = PlayableProjection::from_report(build(), &overrides).unwrap();
        let view = proj.scene("main", "sc-01");

        assert!(view.doors.contains(&Door::Fork {
            world: "flee".into(),
            label: "run".into(),
        }));
        assert!(view.doors.contains(&Door::Examine {
            object: "tide-table".into(),
            reveals: vec!["f-table".into()],
        }));
        assert!(view.doors.contains(&Door::Ask {
            question: "Whose name?".into(),
            reveals: "f-name".into(),
        }));
        assert_eq!(view.doors.len(), 3);

        // With no interactivity (DefaultOverrides): only the native fork door.
        let bare = PlayableProjection::from_report(build(), &DefaultOverrides::default()).unwrap();
        assert_eq!(
            bare.scene("main", "sc-01").doors,
            vec![Door::Fork {
                world: "flee".into(),
                label: "run".into(),
            }]
        );
    }

    // ---- R759 P3c-2: the anchored ladder question resolves from the store ----

    /// A one-section report carrying the `f-name` fact the ladder reveals.
    fn name_report() -> mnemosyne_validate::continuity::PlayableWorldReport {
        report(
            "main",
            vec![scene(
                "sc-01",
                "Dawn",
                vec![begin(
                    "f-name",
                    "the name is Yeonggeun",
                    "ground-truth",
                    &[],
                )],
            )],
            vec![locator("f-name", "sc-01", DisclosureMode::Hint)],
            ForkTreeReport::default(),
        )
    }

    /// The anchor the section's store excerpt actually carries.
    fn store_anchor() -> ContentAnchor {
        ContentAnchor {
            source: "MANUSCRIPT.md".into(),
            locator: Locator::Prefix("지운은".into()),
        }
    }

    /// `sc-01`'s store content prose — a provenance-bound [`Passage`] the anchored
    /// question resolves against (the P3b store-cache model, built in-crate).
    fn sc01_passages() -> HashMap<String, Passage> {
        let excerpt = mnemosyne_atomic::ContentExcerpt {
            anchor: store_anchor(),
            text: "지운은 그 이름을 물었다.".into(),
            text_sha256: String::new(),
        };
        HashMap::from([("sc-01".to_string(), Passage::from_excerpt(&excerpt))])
    }

    /// A `sc-01` ladder whose one rung carries a FABRICATED free question plus the
    /// given `question_anchor` — so a passing test proves the store prose wins.
    fn anchored_ladder(question_anchor: Option<ContentAnchor>) -> StaticOverrides {
        StaticOverrides {
            interactivity: Interactivity {
                objects: HashSet::new(),
                ladders: HashMap::from([(
                    "sc-01".to_string(),
                    vec![Rung {
                        question: "FABRICATED — a free label that must never be shown".into(),
                        question_anchor,
                        reveals: "f-name".into(),
                        needs: Vec::new(),
                    }],
                )]),
                free_investigate: false,
            },
            journal_predicates: Vec::new(),
            quest_precondition_predicates: Vec::new(),
            choice_entity_refs: Vec::new(),
        }
    }

    #[test]
    fn anchored_rung_question_renders_the_store_excerpt_not_the_free_string() {
        let proj = PlayableProjection::from_report_and_passages(
            name_report(),
            &anchored_ladder(Some(store_anchor())),
            sc01_passages(),
        )
        .expect("the declared anchor matches the section's excerpt");
        let view = proj.scene("main", "sc-01");
        // The rendered question IS the store prose.
        assert!(view.doors.contains(&Door::Ask {
            question: "지운은 그 이름을 물었다.".into(),
            reveals: "f-name".into(),
        }));
        // The fabricated free string is NEVER rendered.
        assert!(!view.doors.iter().any(|d| matches!(
            d,
            Door::Ask { question, .. } if question.contains("FABRICATED")
        )));
    }

    #[test]
    fn anchored_rung_question_with_no_section_excerpt_is_rejected() {
        // No passages injected — the section has no store prose to bind to.
        let err = PlayableProjection::from_report_and_passages(
            name_report(),
            &anchored_ladder(Some(store_anchor())),
            HashMap::new(),
        )
        .unwrap_err();
        assert_eq!(
            err,
            EngineError::RungQuestionUnresolvable {
                section: "sc-01".into(),
                anchor: store_anchor(),
                reason: RungQuestionFault::SectionHasNoExcerpt,
            }
        );
    }

    #[test]
    fn anchored_rung_question_with_a_mismatched_anchor_is_rejected() {
        // The rung claims an anchor the section's excerpt does not carry — a false
        // provenance claim, rejected rather than silently resolved to the excerpt.
        let wrong = ContentAnchor {
            source: "MANUSCRIPT.md".into(),
            locator: Locator::Prefix("존재하지".into()),
        };
        let err = PlayableProjection::from_report_and_passages(
            name_report(),
            &anchored_ladder(Some(wrong.clone())),
            sc01_passages(),
        )
        .unwrap_err();
        assert_eq!(
            err,
            EngineError::RungQuestionUnresolvable {
                section: "sc-01".into(),
                anchor: wrong,
                reason: RungQuestionFault::AnchorMismatch,
            }
        );
    }

    #[test]
    fn un_anchored_rung_keeps_its_free_question_even_when_store_prose_exists() {
        // question_anchor: None -> the free string IS the label; injected store
        // prose does not override un-anchored interactive chrome.
        let proj = PlayableProjection::from_report_and_passages(
            name_report(),
            &anchored_ladder(None),
            sc01_passages(),
        )
        .expect("an un-anchored rung never resolves against the store");
        let view = proj.scene("main", "sc-01");
        assert!(view.doors.contains(&Door::Ask {
            question: "FABRICATED — a free label that must never be shown".into(),
            reveals: "f-name".into(),
        }));
    }

    #[test]
    fn journal_predicate_facts_are_routed_out_of_the_prose_lines() {
        let build = || {
            report(
                "main",
                vec![scene(
                    "sc-01",
                    "Dawn",
                    vec![
                        begin("f-prose", "a plain world fact", "ground-truth", &[]),
                        journal_begin("f-quest", "pursues the vault key", "pursues"),
                    ],
                )],
                vec![
                    locator("f-prose", "sc-01", DisclosureMode::State),
                    locator("f-quest", "sc-01", DisclosureMode::State),
                ],
                ForkTreeReport::default(),
            )
        };

        // No journal policy: both facts are prose lines.
        let all = PlayableProjection::from_report(build(), &DefaultOverrides::default()).unwrap();
        assert_eq!(all.lines("main", "sc-01").len(), 2);

        // `pursues` declared journal: the quest leg leaves the prose stream.
        let overrides = StaticOverrides {
            interactivity: Interactivity::default(),
            journal_predicates: vec!["pursues".to_string()],
            quest_precondition_predicates: Vec::new(),
            choice_entity_refs: Vec::new(),
        };
        let filtered = PlayableProjection::from_report(build(), &overrides).unwrap();
        let lines = filtered.lines("main", "sc-01");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].fact_id, "f-prose");
    }

    #[test]
    fn referenceable_entities_accumulate_disclosed_entities_in_walk_order() {
        // R757 B1 — the §5 discourse-order invariant. `ghost` is authored on a
        // WITHHELD fact (no locator), so it is never disclosed and never
        // referenceable — disclosure is additive here too (non-vacuous exclusion).
        let r = report(
            "main",
            vec![
                scene(
                    "sc-01",
                    "Dawn",
                    vec![begin("f-a", "the tide", "ground-truth", &["tide", "jiun"])],
                ),
                scene(
                    "sc-02",
                    "Noon",
                    vec![begin("f-b", "Bunok arrives", "ground-truth", &["bunok"])],
                ),
                scene(
                    "sc-03",
                    "Dusk",
                    vec![begin("f-c", "a hidden thing", "ground-truth", &["ghost"])],
                ),
            ],
            vec![
                locator("f-a", "sc-01", DisclosureMode::State),
                locator("f-b", "sc-02", DisclosureMode::State),
                // f-c is WITHHELD — no locator, so `ghost` never becomes a line.
            ],
            ForkTreeReport::default(),
        );
        let proj = PlayableProjection::from_report(r, &DefaultOverrides::default()).unwrap();
        // At sc-01: only sc-01's disclosed entities (sorted, unique).
        assert_eq!(
            proj.referenceable_entities("main", "sc-01"),
            vec!["jiun".to_string(), "tide".to_string()]
        );
        // At sc-02: cumulative through sc-02.
        assert_eq!(
            proj.referenceable_entities("main", "sc-02"),
            vec!["bunok".to_string(), "jiun".to_string(), "tide".to_string()]
        );
        // At sc-03: STILL only bunok/jiun/tide — `ghost` (withheld) is excluded.
        assert_eq!(
            proj.referenceable_entities("main", "sc-03"),
            vec!["bunok".to_string(), "jiun".to_string(), "tide".to_string()]
        );
        // A section this world does not walk: empty (no at-or-before).
        assert!(proj.referenceable_entities("main", "sc-99").is_empty());
    }

    #[test]
    fn cast_at_projects_the_store_scene_cast_provenance_bound() {
        // R757 B1b — the store `scene_cast` projected as provenance-bound
        // `CastMember`s: entity + authored modality/can_answer + the manuscript
        // quote, in stored order.
        let r = report(
            "main",
            vec![
                cast_scene(
                    "sc-01",
                    "Dawn",
                    vec![begin("f-a", "the tide", "ground-truth", &["tide"])],
                    vec![
                        presence(
                            "ent-jongdeuk",
                            Modality::Observed,
                            true,
                            "종득은 문간에 서 있었다.",
                        ),
                        presence("ent-driver", Modality::Told, false, "운전기사가 왔다더라."),
                    ],
                ),
                scene(
                    "sc-02",
                    "Noon",
                    vec![begin("f-b", "a plain fact", "ground-truth", &[])],
                ),
            ],
            vec![
                locator("f-a", "sc-01", DisclosureMode::State),
                locator("f-b", "sc-02", DisclosureMode::State),
            ],
            ForkTreeReport::default(),
        );
        let proj = PlayableProjection::from_report(r, &DefaultOverrides::default()).unwrap();
        let cast = proj.cast_at("sc-01");
        assert_eq!(cast.len(), 2);
        assert_eq!(cast[0].entity(), "ent-jongdeuk");
        assert_eq!(cast[0].modality(), Modality::Observed);
        assert!(cast[0].can_answer());
        assert_eq!(cast[0].quote(), "종득은 문간에 서 있었다.");
        assert_eq!(cast[1].entity(), "ent-driver");
        assert_eq!(cast[1].modality(), Modality::Told);
        assert!(!cast[1].can_answer());
        // A scene with no authored presence, and an unknown section: empty cast.
        assert!(proj.cast_at("sc-02").is_empty());
        assert!(proj.cast_at("sc-99").is_empty());
    }
}
