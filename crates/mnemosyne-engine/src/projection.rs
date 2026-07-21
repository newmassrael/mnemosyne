//! The playable projection: read `report-playable-world` and index it into a
//! per-world, per-section stream of disclosed [`Line`]s plus the walk and fork
//! topology. The generalization of tide's `narrative.rs` — content-, telling-,
//! and presentation-agnostic.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use mnemosyne_core::MAIN_BRANCH;
use mnemosyne_validate::continuity::{ManuscriptFactEvent, PlayableWorldReport};

use crate::{Door, EngineError, EngineOverrides, Fork, Interactivity, Line, Rung, SceneView};

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
    forks: Vec<Fork>,
    divergent_endings: HashSet<String>,
    interactivity: Interactivity,
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
        Self::from_report(report, overrides)
    }

    /// Index an already-projected [`PlayableWorldReport`] — the testable core
    /// ([`Self::from_workspace`] is the store-reading wrapper).
    ///
    /// # Errors
    ///
    /// [`EngineError::LocatorFactMissing`] if any locator names a `fact_id` no
    /// `begins` event carries (a stale report), never a silent drop.
    pub fn from_report(
        report: PlayableWorldReport,
        overrides: &impl EngineOverrides,
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

        Ok(Self {
            telling,
            by_world,
            walks,
            titles,
            forks,
            divergent_endings,
            interactivity: overrides.interactivity().clone(),
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

        // Ask doors: the authored ladder rungs at this section, in authored
        // order. The leak gate (never construction) enforces that a rung's
        // reveal is a fact the store actually offers here.
        for rung in self.rungs_at(section) {
            doors.push(Door::Ask {
                question: rung.question.clone(),
                reveals: rung.reveals.clone(),
            });
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
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use mnemosyne_core::DisclosureMode;
    use mnemosyne_validate::continuity::ForkTreeReport;

    use crate::test_support::{begin, branch, journal_begin, locator, report, rung, scene};
    use crate::{
        DefaultOverrides, Door, EngineError, Interactivity, PlayableProjection, StaticOverrides,
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
            },
            journal_predicates: Vec::new(),
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
        };
        let filtered = PlayableProjection::from_report(build(), &overrides).unwrap();
        let lines = filtered.lines("main", "sc-01");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].fact_id, "f-prose");
    }
}
