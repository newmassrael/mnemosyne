//! The playable projection: read `report-playable-world` and index it into a
//! per-world, per-section stream of disclosed [`Line`]s plus the walk and fork
//! topology. The generalization of tide's `narrative.rs` — content-, telling-,
//! and presentation-agnostic.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use mnemosyne_core::MAIN_BRANCH;
use mnemosyne_validate::continuity::{ManuscriptFactEvent, PlayableWorldReport};

use crate::{
    CastMember, CastPart, ChoiceEntityRef, ContentAnchor, Door, DoorPart, EngineError,
    EngineOverrides, Fork, ForkPart, Interactivity, Line, LinePart, Passage, PrefixSlices, Rung,
    RungQuestionFault, SceneView,
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
    /// `world -> section -> the fact ids the consumer's journal policy routed OUT
    /// of the prose stream there` (Round 787). Disjoint from `by_world` by
    /// construction — a disclosed locator becomes a [`Line`] or lands here, never
    /// both — so together they are everything the telling offers at a spot, which
    /// is what [`Self::offers`] resolves. EMPTY for a consumer that declares no
    /// journal predicates, which is every consumer but the first game.
    journal_offers: HashMap<String, HashMap<String, Vec<String>>>,
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
    /// TEST-ONLY since Round 771, and gated rather than merely narrowed because
    /// that is what it now is: taking it off the public API (see the crate docs
    /// for why — it was the ingestion hole under a render-side guarantee) left it
    /// with no caller outside tests, which the dead-code lint said plainly. A
    /// consumer supplies a whole projection through
    /// [`ProjectionParts`](crate::ProjectionParts) instead.
    ///
    /// # Errors
    ///
    /// [`EngineError::LocatorFactMissing`] if any locator names a `fact_id` no
    /// `begins` event carries (a stale report), never a silent drop;
    /// [`EngineError::RungQuestionUnresolvable`] if any rung declares a
    /// `question_anchor` (unresolvable here — no store prose).
    #[cfg(test)]
    pub(crate) fn from_report(
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
        let mut journal_offers: HashMap<String, HashMap<String, Vec<String>>> = HashMap::new();
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
            let mut routed: HashMap<String, Vec<String>> = HashMap::new();
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
                // world prose — route them out of the line stream. The fail-loud
                // join above runs FIRST, so a stale journal locator is still
                // caught. They are RECORDED rather than dropped (Round 787):
                // the telling still offers the fact here, and a gate that asks
                // what the audience can KNOW must not read a stream that answers
                // what the audience is SHOWN.
                if let Some(typed) = &begin.typed {
                    if journal.contains(&typed.predicate) {
                        routed
                            .entry(locator.scene.clone())
                            .or_default()
                            .push(locator.fact_id.clone());
                        continue;
                    }
                }
                by_section
                    .entry(locator.scene.clone())
                    .or_default()
                    .push(Line::from_disclosed(locator, begin));
            }

            for ids in routed.values_mut() {
                ids.sort();
                ids.dedup();
            }
            journal_offers.insert(world_name.clone(), routed);
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
            let questions = resolve_ladder_questions(section, rungs, &passages)?;
            let resolved = rungs
                .iter()
                .zip(questions)
                .map(|(rung, question)| Door::Ask {
                    question,
                    reveals: rung.reveals.clone(),
                })
                .collect();
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
            journal_offers,
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

    /// Does the telling OFFER `fact_id` to the audience at this spot (Round 787)?
    ///
    /// This is the knowledge axis, and it is deliberately not [`Self::lines`].
    /// A fact is offered where it has a locator — where the audience meets it —
    /// and the consumer's
    /// [`journal_predicates`](EngineOverrides::journal_predicates) policy decides
    /// only which STREAM carries it, never whether it was met. So the split the
    /// kernel draws is: **`lines` is what the audience is SHOWN, `offers` is what
    /// the audience can KNOW**, and every gate asking whether knowledge is
    /// reachable in time must resolve through here.
    ///
    /// Withholding still narrows this, which is the narrowing that was always
    /// intended: a withheld fact emits no locator, so it is offered nowhere and
    /// this answers `false` for it at every spot.
    ///
    /// Round 778 recorded the alternative reading — that "offered" is
    /// disclosed-as-prose — as an honest limitation, and it produced a FALSE
    /// [`QuestGateViolation::PreconditionUnreachable`](crate::QuestGateViolation::PreconditionUnreachable)
    /// for any consumer routing a precondition fact into its journal. A gate's
    /// verdict must be about the store's facts, not about a consumer's rendering
    /// policy.
    ///
    /// Answers `false` for an unknown world or section — an accessor reporting
    /// "nothing here" is an answer, not a verdict (the Round 776 distinction).
    /// A GATE must establish the world separately, as
    /// [`QuestProjection::completability`](crate::QuestProjection::completability)
    /// does with [`Self::knows_world`].
    #[must_use]
    pub fn offers(&self, world: &str, section: &str, fact_id: &str) -> bool {
        self.offered_at(world, section).any(|id| id == fact_id)
    }

    /// Every fact id the telling offers at this spot — the prose stream and the
    /// journal-routed ids, which are disjoint by construction (Round 787).
    ///
    /// The ONE place that computes that union. [`Self::offers`] is this plus a
    /// membership test, and the ladder gate's `earliest_offer` index is this
    /// scanned in walk order — so the two gates cannot drift into two readings of
    /// "offered", which is the shape Round 779 removed one layer down.
    ///
    /// Crate-private: the public knowledge-axis surface is the predicate. A
    /// consumer needing to enumerate has not asked, and an enumeration is the
    /// harder thing to take back.
    pub(crate) fn offered_at(&self, world: &str, section: &str) -> impl Iterator<Item = &str> {
        self.lines(world, section)
            .iter()
            .map(|line| line.fact_id())
            .chain(
                self.journal_offers
                    .get(world)
                    .and_then(|sections| sections.get(section))
                    .map_or(&[][..], Vec::as_slice)
                    .iter()
                    .map(String::as_str),
            )
    }

    /// The one spot bundled as a [`SceneView`]: disclosed `lines` plus the LIVE
    /// `doors` for a reader who already holds `known` (fork choices + examine
    /// objects + authored ladder rungs, from the projection's configured
    /// interactivity). Narrative content is exclusively the provenance-bound
    /// `lines`; a door reveals `fact_id`s, never free text.
    ///
    /// It took `known` from Round 779 onward, and the reason is the shape of the
    /// bug it removed rather than a new feature. This handed out UNFILTERED doors
    /// while [`live_doors_at`](Self::live_doors_at) filtered them, so the same
    /// datum had two public read paths under two different invariants and the
    /// dead-door class R762/R763 closed was reachable through the kernel's own
    /// most convenient accessor — the one a renderer reaches for first. A shared
    /// rule that one caller can decline is not a shared rule. There is now a
    /// single filtered path, so the guarantee holds by construction instead of by
    /// the caller picking the careful function.
    ///
    /// A reader with no session state passes an empty set, which says "this
    /// reader holds nothing" rather than leaving the question unasked — and
    /// filters nothing, since freshness is a set difference.
    #[must_use]
    pub fn scene(&self, world: &str, section: &str, known: &HashSet<String>) -> SceneView {
        SceneView::new(
            section.to_string(),
            self.title(section).map(str::to_string),
            self.lines(world, section).to_vec(),
            self.live_doors_at(world, section, known),
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

    /// Every world-line this projection carries, sorted (Round 776) — the
    /// AUTHORITATIVE set, and the list a consumer runs its per-world gates over.
    ///
    /// It existed only privately until now, and the first consumer paid for that:
    /// with no way to ask, it rebuilt the list by walking the fork graph from
    /// `main` through [`walk`](Self::walk) + [`forks_at`](Self::forks_at). That is
    /// a DIFFERENT definition — "fork-reachable from the trunk" rather than "in
    /// the projection" — so a registered world-line that no fork edge points at
    /// would never have been gated, and nothing could have said so. This is the
    /// mistake R766 refused to make on the prose axis, where the slicer exposes
    /// [`in_prose_order`](crate::PrefixSlices::in_prose_order) precisely so an
    /// ordered caller never re-implements the search and drifts from it; the world
    /// set had no such door.
    ///
    /// A world with an EMPTY walk is still a world and is listed: the store
    /// registered it, and "carries no scene under this telling" is an answer, not
    /// an absence. That distinction is what [`gate`](Self::gate) turns on.
    #[must_use]
    pub fn worlds(&self) -> Vec<&str> {
        let mut out: Vec<&str> = self.walks.keys().map(String::as_str).collect();
        out.sort_unstable();
        out
    }

    /// Does this projection carry `world`? The membership half of
    /// [`worlds`](Self::worlds), reading the same map so the enumeration and the
    /// test can never disagree.
    ///
    /// Deliberately NOT `walk(world).is_empty()`: `walks` holds an entry for every
    /// world in the report, so a registered world whose walk is empty is KNOWN
    /// with nothing on it, while an unknown name has no entry at all. The gate
    /// must tell those apart — one is a clean verdict, the other is a question it
    /// cannot answer.
    pub(crate) fn knows_world(&self, world: &str) -> bool {
        self.walks.contains_key(world)
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
    /// inclusive. Sorted, unique. The field-report sec 5 discourse-order
    /// invariant: an entity is referenceable at a spot only once the player has
    /// MET it (it appeared in a disclosed line there or earlier), so a consumer's
    /// choice may name only these — a reference to anything else is the
    /// field-report parallel-identity class, caught by
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

    /// The doors at a spot that are LIVE for a reader who already holds `known`
    /// (R762 P4b — the disclose-freshness rule for the doors the kernel BUILDS).
    /// A disclose-door (`Examine`/`Ask`) is kept only while it still discloses a
    /// fact the reader does not hold ([`fresh_disclosure`] of its
    /// [`discloses`](Door::discloses) is non-empty); a `Fork` door passes through
    /// unchanged, because a navigational door discloses no fact and its liveness
    /// (not-taken) is the consumer's, never disclosure-relative. This is why the
    /// tide field-report sec 5 dead-door class cannot recur through the kernel's
    /// own doors: a door that would surface nothing new is not offered, and since
    /// Round 779 there is no public path that skips this — [`scene`](Self::scene)
    /// used to be one, which made the claim in this sentence false for as long as
    /// it stood.
    ///
    /// A consumer applies the SAME [`fresh_disclosure`] to the doors IT builds
    /// (tide's `go` door, whose reachable facts the kernel never sees), so one
    /// rule governs every disclose-door and the invariant cannot be half-enforced.
    ///
    /// Read-side (R643/R337): `known` is a QUERY COORDINATE like `world`/
    /// `section` — `f(frozen_store, coords) -> view`. The projection stores no
    /// session state and computes no successor; a door filtered out here is not
    /// mutated away, and a different `known` yields a different answer from the
    /// same immutable projection.
    #[must_use]
    pub fn live_doors_at(&self, world: &str, section: &str, known: &HashSet<String>) -> Vec<Door> {
        self.doors_at(world, section)
            .into_iter()
            .filter(|door| match door {
                // Navigational: discloses nothing; its liveness is the consumer's
                // (not-taken), so it is never dropped by the disclosure filter.
                Door::Fork { .. } => true,
                // Disclose-door: live iff it still surfaces a not-known fact.
                Door::Examine { .. } | Door::Ask { .. } => {
                    !fresh_disclosure(door.discloses(), known).is_empty()
                }
            })
            .collect()
    }
}

/// The fresh disclosure a door offers a reader who already holds `known` — the
/// door's [`discloses`](Door::discloses) set minus what the reader already knows,
/// preserving the door's own order. An empty result means the door is DEAD (it
/// would surface nothing new).
///
/// THE universal disclose-freshness rule (R762 P4b). The kernel applies it to its
/// own `Examine`/`Ask` doors in
/// [`live_doors_at`](PlayableProjection::live_doors_at); a consumer applies the
/// SAME function to the reveal-set of a door IT builds (tide's `go` door, whose
/// destination-reachable facts the kernel never holds), so every disclose-door —
/// kernel-built or consumer-built — obeys one definition of "live" and the
/// half-enforced-invariant class (a builder that forgets the known-check) is
/// structurally impossible.
///
/// Read-side: a pure set-difference over a query coordinate; no state, no store
/// access. It does not price or record the disclosure (a consumer's own reveal
/// path — tide's `reveal_probe` — remains the sole writer of `known`); it only
/// answers "what here is still new?".
#[must_use]
pub fn fresh_disclosure<'a>(reveals: &'a [String], known: &HashSet<String>) -> Vec<&'a str> {
    reveals
        .iter()
        .filter(|r| !known.contains(r.as_str()))
        .map(String::as_str)
        .collect()
}

/// Resolve a rung's rendered question, fail-loud (R759 P3c-2). An UN-ANCHORED rung
/// renders its free `question` verbatim (interactive chrome). An ANCHORED rung's
/// question is the section's store `content_excerpt` text — and the declared
/// anchor MUST match that excerpt's anchor, so a false provenance claim (a rung
/// pointing at prose the section does not hold, or a section with no excerpt) is
/// [`EngineError::RungQuestionUnresolvable`], never a silent fall-back to the free
/// string. This is the store-resolution that makes a fabricated door label
/// inexpressible for a provenance-bound consumer.
/// One world's baked narrative: each section paired with the lines disclosed
/// there, in section order (Round 769).
pub type SectionLines = Vec<(String, Vec<LinePart>)>;

/// One world's baked journal-routed offers: each section paired with the fact ids
/// the consumer's journal policy took out of the prose stream there, in section
/// order (Round 787). Empty for a consumer that declares no journal predicates.
pub type SectionJournalOffers = Vec<(String, Vec<String>)>;

/// A whole [`PlayableProjection`] as plain data (Round 769) — what a build-time
/// bake writes out and reads back.
///
/// Every field is already resolved: the store read, the locator join, and the
/// ladder-question resolution all happened when this was produced. So
/// [`PlayableProjection::from_parts`] takes no `Result` — there is nothing left
/// to check, because there is no untrusted input left to check it against. That
/// is the whole shape of the build-time move (R764): the checks do not get
/// deleted, they get discharged where the store is, and what remains at runtime
/// is data the compiler has already type-checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionParts {
    /// The telling this projection was resolved under.
    pub telling: String,
    /// `world -> section -> the disclosed lines there`.
    pub by_world: Vec<(String, SectionLines)>,
    /// `world -> its declared scene walk`.
    pub walks: Vec<(String, Vec<String>)>,
    /// `section -> its skeleton title`.
    pub titles: Vec<(String, String)>,
    /// `section -> the store-owned cast standing there`.
    pub cast: Vec<(String, Vec<CastPart>)>,
    /// The cross-world choice graph.
    pub forks: Vec<ForkPart>,
    /// World-lines whose ending diverges.
    pub divergent_endings: Vec<String>,
    /// The consumer's interactive layer, as it was at bake time.
    pub interactivity: Interactivity,
    /// Declared choice-to-entity references.
    pub choice_entity_refs: Vec<ChoiceEntityRef>,
    /// `section -> the ask doors dug there, questions ALREADY resolved`.
    pub ask_doors: Vec<(String, Vec<DoorPart>)>,
    /// `world -> section -> the fact ids the journal policy routed out of the
    /// prose stream` (Round 787). Empty for a consumer with no journal policy;
    /// carried because [`PlayableProjection::offers`] reads it, so a bake that
    /// dropped it would answer a different question from the live projection.
    pub journal_offers: Vec<(String, SectionJournalOffers)>,
}

impl PlayableProjection {
    /// Emit this projection as plain data (Round 769) — the bake half of the
    /// build-time seam. Deterministic: every map is emitted in sorted key order,
    /// so the same store produces byte-identical generated source and a rebuild
    /// that changed nothing changes nothing.
    #[must_use]
    pub fn to_parts(&self) -> ProjectionParts {
        fn sorted<T: Clone>(map: &HashMap<String, T>) -> Vec<(String, T)> {
            let mut out: Vec<(String, T)> = map
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect();
            out.sort_by(|a, b| a.0.cmp(&b.0));
            out
        }
        let mut divergent_endings: Vec<String> = self.divergent_endings.iter().cloned().collect();
        divergent_endings.sort();
        ProjectionParts {
            telling: self.telling.clone(),
            by_world: sorted(&self.by_world)
                .into_iter()
                .map(|(world, sections)| {
                    (
                        world,
                        sorted(&sections)
                            .into_iter()
                            .map(|(section, lines)| {
                                (section, lines.iter().map(Line::to_part).collect())
                            })
                            .collect(),
                    )
                })
                .collect(),
            walks: sorted(&self.walks),
            titles: sorted(&self.titles),
            cast: sorted(&self.cast)
                .into_iter()
                .map(|(section, members)| {
                    (section, members.iter().map(CastMember::to_part).collect())
                })
                .collect(),
            forks: self.forks.iter().map(Fork::to_part).collect(),
            divergent_endings,
            interactivity: self.interactivity.clone(),
            choice_entity_refs: self.choice_entity_refs.clone(),
            ask_doors: sorted(&self.ask_doors)
                .into_iter()
                .map(|(section, doors)| (section, doors.iter().map(Door::to_part).collect()))
                .collect(),
            journal_offers: sorted(&self.journal_offers)
                .into_iter()
                .map(|(world, sections)| (world, sorted(&sections)))
                .collect(),
        }
    }

    /// Ingest baked parts (Round 769) — the read half, and INFALLIBLE by
    /// construction rather than by optimism.
    ///
    /// There is no `Result` because there is no check left to run: a locator join
    /// that could dangle already happened, a ladder question that could fail to
    /// resolve already resolved, and a store that could be unreadable was read at
    /// bake time. A consumer built this way does no validation at startup because
    /// it takes in no input that could be wrong — the artifact is generated from
    /// the store by its own build and type-checked by its own compiler.
    #[must_use]
    pub fn from_parts(parts: ProjectionParts) -> Self {
        Self {
            telling: parts.telling,
            by_world: parts
                .by_world
                .into_iter()
                .map(|(world, sections)| {
                    (
                        world,
                        sections
                            .into_iter()
                            .map(|(section, lines)| {
                                (section, lines.into_iter().map(Line::from_part).collect())
                            })
                            .collect(),
                    )
                })
                .collect(),
            walks: parts.walks.into_iter().collect(),
            titles: parts.titles.into_iter().collect(),
            cast: parts
                .cast
                .into_iter()
                .map(|(section, members)| {
                    (
                        section,
                        members.into_iter().map(CastMember::from_part).collect(),
                    )
                })
                .collect(),
            forks: parts.forks.into_iter().map(Fork::from_part).collect(),
            divergent_endings: parts.divergent_endings.into_iter().collect(),
            interactivity: parts.interactivity,
            choice_entity_refs: parts.choice_entity_refs,
            ask_doors: parts
                .ask_doors
                .into_iter()
                .map(|(section, doors)| (section, doors.into_iter().map(Door::from_part).collect()))
                .collect(),
            journal_offers: parts
                .journal_offers
                .into_iter()
                .map(|(world, sections)| (world, sections.into_iter().collect()))
                .collect(),
        }
    }
}

fn resolve_ladder_questions(
    section: &str,
    rungs: &[Rung],
    passages: &HashMap<String, Passage>,
) -> Result<Vec<String>, EngineError> {
    // Un-anchored rungs keep their authored question (interactive chrome the
    // store never claimed). If NONE is anchored there is no store prose to
    // consult, so a section without an excerpt is not yet a fault.
    // DEDUPED, and that is load-bearing (Round 768): the kernel prices a rung per
    // revealed fact while an author writes one hold, so a hold that opens three
    // facts arrives as three rungs sharing ONE anchor. They are the same hold, and
    // the slicer — which rightly refuses a duplicated coordinate from a single
    // declaration (R766) — must see it once. Order is preserved, so the sequence
    // judged against the prose is the sequence of distinct HOLDS.
    let mut anchored: Vec<&ContentAnchor> = Vec::new();
    for anchor in rungs
        .iter()
        .filter_map(|rung| rung.question_anchor.as_ref())
    {
        if !anchored.contains(&anchor) {
            anchored.push(anchor);
        }
    }
    if anchored.is_empty() {
        return Ok(rungs.iter().map(|rung| rung.question.clone()).collect());
    }

    let fault =
        |anchor: &ContentAnchor, reason: RungQuestionFault| EngineError::RungQuestionUnresolvable {
            section: section.to_string(),
            anchor: anchor.clone(),
            reason,
        };
    let passage = passages
        .get(section)
        .ok_or_else(|| fault(anchored[0], RungQuestionFault::SectionHasNoExcerpt))?;

    // Round 767 — the anchor no longer has to EQUAL the section's excerpt anchor.
    // That rule (R761) made a hold part-way down a scene inexpressible: a rung
    // question IS a line of the section's prose, not the whole section, so a
    // provenance-bound consumer had to declare no anchor at all and the guarantee
    // never reached a real ladder. The anchor must now name the SAME document and
    // LOCATE within its text — which is the R766-tightened slicer's job, so an
    // ambiguous or duplicated hold is refused there rather than guessed at here.
    let owned: Vec<ContentAnchor> = anchored.iter().map(|a| (*a).clone()).collect();
    let slices = PrefixSlices::new(&passage.anchor().source, passage.text(), &owned)
        .map_err(|err| fault(anchored[0], RungQuestionFault::Unlocatable(Box::new(err))))?;

    // Order is the ladder's own invariant (R766 deliberately kept it out of the
    // generic slicer): the nth declared hold must be the nth passage, or the
    // author's numbering does not describe the scene they wrote.
    let prose_order = slices.in_prose_order();
    if prose_order.len() == owned.len() && prose_order != owned.as_slice() {
        let at = owned
            .iter()
            .zip(prose_order)
            .position(|(declared, actual)| declared != actual)
            .unwrap_or(0);
        return Err(fault(&owned[at], RungQuestionFault::OutOfProseOrder));
    }

    rungs
        .iter()
        .map(|rung| {
            let Some(anchor) = &rung.question_anchor else {
                return Ok(rung.question.clone());
            };
            let slice = Passage::resolve(anchor.clone(), &slices)
                .map_err(|err| fault(anchor, RungQuestionFault::Unlocatable(Box::new(err))))?;
            // A hold is named by the LINE it starts at: the slice runs to the next
            // hold and so carries whatever follows the question, which is the
            // scene's business and not the door's label.
            Ok(slice
                .text()
                .lines()
                .next()
                .unwrap_or_default()
                .trim_end()
                .to_string())
        })
        .collect()
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
        fresh_disclosure, ContentAnchor, DefaultOverrides, Door, EngineError, Interactivity,
        Locator, Modality, Passage, PlayableProjection, Rung, RungQuestionFault, StaticOverrides,
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

        let view = proj.scene("main", "sc-01", &HashSet::new());
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
        let view = proj.scene("main", "sc-01", &HashSet::new());

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
            bare.scene("main", "sc-01", &HashSet::new()).doors,
            vec![Door::Fork {
                world: "flee".into(),
                label: "run".into(),
            }]
        );
    }

    // ---- R762 P4b: the disclose-freshness rule ----

    #[test]
    fn discloses_reports_each_door_kinds_disclosure() {
        // A navigational door discloses no fact; a disclose-door discloses its
        // reveals (Examine the whole set, Ask a one-element slice).
        let fork = Door::Fork {
            world: "flee".into(),
            label: "run".into(),
        };
        assert!(fork.discloses().is_empty());

        let examine = Door::Examine {
            object: "tide-table".into(),
            reveals: vec!["f-a".into(), "f-b".into()],
        };
        assert_eq!(examine.discloses(), ["f-a".to_string(), "f-b".to_string()]);

        let ask = Door::Ask {
            question: "Whose name?".into(),
            reveals: "f-name".into(),
        };
        assert_eq!(ask.discloses(), ["f-name".to_string()]); // one-element slice
    }

    #[test]
    fn fresh_disclosure_is_the_not_known_subset_in_order() {
        let reveals = vec!["f-a".to_string(), "f-b".to_string(), "f-c".to_string()];
        // Nothing known: the whole set, in the door's own order.
        assert_eq!(
            fresh_disclosure(&reveals, &HashSet::new()),
            vec!["f-a", "f-b", "f-c"]
        );
        // The middle one known: dropped, order preserved.
        let known = HashSet::from(["f-b".to_string()]);
        assert_eq!(fresh_disclosure(&reveals, &known), vec!["f-a", "f-c"]);
        // All known: empty -> the door is dead.
        let all = HashSet::from(["f-a".to_string(), "f-b".to_string(), "f-c".to_string()]);
        assert!(fresh_disclosure(&reveals, &all).is_empty());
    }

    /// The kernel's own disclose-doors (`Examine`/`Ask`) are dropped once the
    /// reader holds all they would reveal, while a `Fork` always survives — the
    /// tide field-report sec 5 dead-`go`-door made structurally unofferable through
    /// the doors the kernel builds. `known` is a bare query coordinate: the same
    /// immutable projection answers three different `known` sets.
    #[test]
    fn live_doors_at_drops_a_fully_known_disclose_door_but_keeps_fork() {
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
        let r = report(
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
        );
        let proj = PlayableProjection::from_report(r, &overrides).unwrap();

        let fork = Door::Fork {
            world: "flee".into(),
            label: "run".into(),
        };
        let examine = Door::Examine {
            object: "tide-table".into(),
            reveals: vec!["f-table".into()],
        };
        let ask = Door::Ask {
            question: "Whose name?".into(),
            reveals: "f-name".into(),
        };

        // Nothing known: all three doors are live (the stateless `scene` set).
        let all_live = proj.live_doors_at("main", "sc-01", &HashSet::new());
        assert_eq!(all_live.len(), 3);
        assert!(all_live.contains(&fork));
        assert!(all_live.contains(&examine));
        assert!(all_live.contains(&ask));

        // `f-table` known: the examine door is DEAD (its only reveal is held),
        // the ask (`f-name`) stays live, and the fork always passes.
        let known_table = HashSet::from(["f-table".to_string()]);
        let live = proj.live_doors_at("main", "sc-01", &known_table);
        assert!(live.contains(&fork));
        assert!(live.contains(&ask));
        assert!(
            !live.contains(&examine),
            "a fully-known examine door is not offered"
        );
        assert_eq!(live.len(), 2);

        // Both facts known: both disclose-doors are dead, only the fork survives
        // — the field-report sec 5 dead-door is unofferable while navigation
        // remains.
        let known_both = HashSet::from(["f-table".to_string(), "f-name".to_string()]);
        let nav_only = proj.live_doors_at("main", "sc-01", &known_both);
        assert_eq!(nav_only, vec![fork.clone()]);

        // Round 779 — and `scene` gives the SAME answer, because it is the same
        // path. It used to hand out the unfiltered set, which made this crate's
        // own claim (that a dead door cannot be offered through the kernel's
        // doors) false through the accessor a renderer reaches for first: a
        // shared rule one caller can decline is not a shared rule. Asserted at
        // every `known` this test already established, so a future short-cut back
        // to the unfiltered set fails here rather than in a consumer's chrome.
        for known in [HashSet::new(), known_table, known_both] {
            assert_eq!(
                proj.scene("main", "sc-01", &known).doors,
                proj.live_doors_at("main", "sc-01", &known),
                "the bundled scene must not be a second, laxer door path"
            );
        }
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
        let view = proj.scene("main", "sc-01", &HashSet::new());
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
    fn anchored_rung_question_the_prose_does_not_carry_is_rejected() {
        // The rung claims a prefix the section's excerpt does not carry — a false
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
                reason: RungQuestionFault::Unlocatable(Box::new(
                    crate::ProseError::PrefixNotFound {
                        source: "MANUSCRIPT.md".into(),
                        prefix: "존재하지".into(),
                    }
                )),
            }
        );
    }

    /// A multi-line `sc-01` excerpt — a scene with THREE holds in it, which is
    /// what the R761 anchor-equality rule could not address (Round 767).
    fn sc01_multi_line_passages() -> HashMap<String, Passage> {
        let excerpt = mnemosyne_atomic::ContentExcerpt {
            anchor: store_anchor(),
            text: "지운은 둑에 섰다.\n이름을 물었다.\n종득은 답하지 않았다.\n물때를 물었다.\n\
                   종득은 셈을 폈다."
                .into(),
            text_sha256: String::new(),
        };
        HashMap::from([("sc-01".to_string(), Passage::from_excerpt(&excerpt))])
    }

    fn at(prefix: &str) -> ContentAnchor {
        ContentAnchor {
            source: "MANUSCRIPT.md".into(),
            locator: Locator::Prefix(prefix.into()),
        }
    }

    /// A `sc-01` ladder of N rungs, each with a FABRICATED free question and the
    /// given anchor, so a passing test proves the store prose won.
    fn ladder_of(anchors: &[ContentAnchor]) -> StaticOverrides {
        StaticOverrides {
            interactivity: Interactivity {
                objects: HashSet::new(),
                ladders: HashMap::from([(
                    "sc-01".to_string(),
                    anchors
                        .iter()
                        .map(|anchor| Rung {
                            question: "FABRICATED — a free label that must never be shown".into(),
                            question_anchor: Some(anchor.clone()),
                            reveals: "f-name".into(),
                            needs: Vec::new(),
                        })
                        .collect(),
                )]),
                free_investigate: false,
            },
            journal_predicates: Vec::new(),
            quest_precondition_predicates: Vec::new(),
            choice_entity_refs: Vec::new(),
        }
    }

    #[test]
    fn a_rung_anchored_part_way_down_a_scene_renders_that_line() {
        // Round 767 — THE capability R761 lacked. Under anchor-equality the only
        // expressible anchor was the section's own, so a hold mid-scene could not
        // be declared at all and a provenance-bound consumer had to pass None.
        // Here two rungs take hold at two different lines of one excerpt, and each
        // door's label is ITS line — not the free string, not the whole excerpt,
        // and not the first line for both.
        let proj = PlayableProjection::from_report_and_passages(
            name_report(),
            &ladder_of(&[at("이름을"), at("물때를")]),
            sc01_multi_line_passages(),
        )
        .expect("a mid-scene hold resolves against the section's own prose");
        let labels: Vec<String> = proj
            .scene("main", "sc-01", &HashSet::new())
            .doors
            .into_iter()
            .filter_map(|door| match door {
                Door::Ask { question, .. } => Some(question),
                _ => None,
            })
            .collect();
        assert_eq!(
            labels,
            vec!["이름을 물었다.".to_string(), "물때를 물었다.".to_string()],
            "each hold is named by the line it starts at"
        );
    }

    #[test]
    fn a_baked_projection_answers_exactly_as_the_projected_one() {
        // Round 769 — the build-time seam's whole claim, stated as a test: a
        // projection that went out through `to_parts` and came back through the
        // INFALLIBLE `from_parts` answers every public query identically. If this
        // holds, moving the projection to build time costs a consumer nothing but
        // the store read it wanted to stop paying.
        // A world thick enough that every part-carrying field is POPULATED —
        // an empty `cast` or `forks` would let a dropped field pass unnoticed.
        let mut fork_tree = ForkTreeReport::default();
        fork_tree
            .branches
            .push(branch("dark", "돌아선다", "main", "sc-01", &[]));
        fork_tree.branch_count = 1;
        let thick = crate::test_support::report_worlds(
            vec![
                (
                    "main",
                    vec![cast_scene(
                        "sc-01",
                        "Dawn",
                        vec![
                            begin(
                                "f-name",
                                "the name is Yeonggeun",
                                "ground-truth",
                                &["ent-a"],
                            ),
                            journal_begin("f-leg", "pursues the vault key", "pursues"),
                        ],
                        vec![presence(
                            "ent-jongdeuk",
                            Modality::Observed,
                            true,
                            "종득은 섰다.",
                        )],
                    )],
                    vec![
                        locator("f-name", "sc-01", DisclosureMode::Hint),
                        locator("f-leg", "sc-01", DisclosureMode::State),
                    ],
                ),
                (
                    // The SAME scene_cast: presence is authored on the shared
                    // section, so every world walking it sees one cast. Giving
                    // one world a cast and the other none would be a state the
                    // store cannot produce, and the projection (first world to
                    // carry a section fixes its cast) would read the empty one.
                    "dark",
                    vec![cast_scene(
                        "sc-01",
                        "Dawn",
                        vec![
                            begin("f-name", "the name is Yeonggeun", "ground-truth", &[]),
                            journal_begin("f-leg", "pursues the vault key", "pursues"),
                        ],
                        vec![presence(
                            "ent-jongdeuk",
                            Modality::Observed,
                            true,
                            "종득은 섰다.",
                        )],
                    )],
                    vec![
                        locator("f-name", "sc-01", DisclosureMode::State),
                        locator("f-leg", "sc-01", DisclosureMode::State),
                    ],
                ),
            ],
            fork_tree,
        );
        let live = PlayableProjection::from_report_and_passages(
            thick,
            &StaticOverrides {
                // Round 787 — the fixture routes a fact out of the prose stream so
                // the journal-offer axis is CARRIED across the seam here. Without
                // it that field is empty on both sides and the round trip agrees
                // about nothing.
                journal_predicates: vec!["pursues".to_string()],
                ..ladder_of(&[at("이름을"), at("물때를")])
            },
            sc01_multi_line_passages(),
        )
        .expect("the live projection");
        // The fixture must actually exercise the fields, or the test proves
        // nothing about them.
        assert!(!live.cast_at("sc-01").is_empty(), "fixture must carry cast");
        assert!(
            !live.forks_at("sc-01", "main").is_empty(),
            "fixture must carry a fork"
        );
        assert!(
            live.offers("main", "sc-01", "f-leg")
                && !live
                    .lines("main", "sc-01")
                    .iter()
                    .any(|l| l.fact_id == "f-leg"),
            "fixture must carry a journal-routed offer — offered, and not a line"
        );
        let baked = PlayableProjection::from_parts(live.to_parts());

        assert_eq!(baked.telling(), live.telling());
        assert_eq!(baked.spine(), live.spine());
        // Round 776 — the world list is DERIVED from the projection rather than
        // hand-written here, which is the same correction the accessor exists for:
        // a hand list silently stops covering a world the fixture gains.
        assert_eq!(baked.worlds(), live.worlds());
        assert_eq!(
            live.worlds(),
            vec!["dark", "main"],
            "the fixture's two roads"
        );
        for world in live.worlds() {
            assert_eq!(baked.walk(world), live.walk(world));
            assert_eq!(
                baked.is_divergent_ending(world),
                live.is_divergent_ending(world)
            );
            for section in live.walk(world) {
                assert_eq!(baked.lines(world, section), live.lines(world, section));
                assert_eq!(baked.title(section), live.title(section));
                assert_eq!(baked.cursor_of(section), live.cursor_of(section));
                assert_eq!(baked.cast_at(section), live.cast_at(section));
                assert_eq!(
                    baked.forks_at(section, world),
                    live.forks_at(section, world)
                );
                assert_eq!(
                    baked.scene(world, section, &HashSet::new()),
                    live.scene(world, section, &HashSet::new())
                );
                assert_eq!(
                    baked.referenceable_entities(world, section),
                    live.referenceable_entities(world, section)
                );
                // Round 787 — the knowledge axis crosses the seam too. Compared
                // over the LIVE offers rather than a hand-written fact list, for
                // the reason `worlds()` is derived above.
                for fact_id in live.offered_at(world, section) {
                    assert!(
                        baked.offers(world, section, fact_id),
                        "baked lost an offer: {world}/{section}/{fact_id}"
                    );
                }
                let known = HashSet::new();
                assert_eq!(
                    baked.live_doors_at(world, section, &known),
                    live.live_doors_at(world, section, &known)
                );
            }
        }
        // The ask doors survive with their RESOLVED labels: baking happens after
        // resolution, so the store prose is in the artifact and the runtime never
        // re-resolves it (nor could it — it has no store).
        assert!(baked
            .scene("main", "sc-01", &HashSet::new())
            .doors
            .contains(&Door::Ask {
                question: "이름을 물었다.".into(),
                reveals: "f-name".into(),
            }));
        // And the emit is deterministic: the same projection parts twice over.
        assert_eq!(live.to_parts(), baked.to_parts());
    }

    #[test]
    fn one_hold_priced_per_fact_resolves_rather_than_reading_as_a_duplicate() {
        // Round 768 — the kernel prices a rung per revealed fact while an author
        // writes ONE hold, so a hold opening two facts arrives as two rungs
        // sharing one anchor. The slicer rightly refuses a duplicated coordinate
        // from a single declaration (R766), so the resolver must see the distinct
        // HOLDS: it dedupes. Without that, `store_interactivity`'s very first
        // multi-fact hold would have failed as DuplicateAnchor.
        let proj = PlayableProjection::from_report_and_passages(
            name_report(),
            &ladder_of(&[at("이름을"), at("물때를"), at("물때를")]),
            sc01_multi_line_passages(),
        )
        .expect("two rungs at one hold are the same hold, not a duplicated coordinate");
        let labels: Vec<String> = proj
            .scene("main", "sc-01", &HashSet::new())
            .doors
            .into_iter()
            .filter_map(|door| match door {
                Door::Ask { question, .. } => Some(question),
                _ => None,
            })
            .collect();
        assert_eq!(
            labels,
            vec![
                "이름을 물었다.".to_string(),
                "물때를 물었다.".to_string(),
                "물때를 물었다.".to_string(),
            ],
            "each priced rung carries its hold's line"
        );
    }

    #[test]
    fn a_ladder_declared_out_of_prose_order_is_rejected() {
        // Round 767 — the LADDER invariant the generic slicer deliberately does
        // not carry (R766): the nth declared hold must be the nth passage, or the
        // author's numbering does not describe the scene they wrote.
        let err = PlayableProjection::from_report_and_passages(
            name_report(),
            &ladder_of(&[at("물때를"), at("이름을")]),
            sc01_multi_line_passages(),
        )
        .unwrap_err();
        assert_eq!(
            err,
            EngineError::RungQuestionUnresolvable {
                section: "sc-01".into(),
                anchor: at("물때를"),
                reason: RungQuestionFault::OutOfProseOrder,
            }
        );
        // ...and the SAME two holds in prose order are accepted, so the reject is
        // the order and not the anchors.
        assert!(PlayableProjection::from_report_and_passages(
            name_report(),
            &ladder_of(&[at("이름을"), at("물때를")]),
            sc01_multi_line_passages(),
        )
        .is_ok());
    }

    #[test]
    fn a_rung_anchored_ambiguously_is_rejected_not_guessed() {
        // Round 767 — the R766 tightening reaching its first real caller: a prefix
        // naming two lines of the scene resolves to neither. Before R766 this
        // silently took the first.
        let err = PlayableProjection::from_report_and_passages(
            name_report(),
            &ladder_of(&[at("종득은")]),
            sc01_multi_line_passages(),
        )
        .unwrap_err();
        assert_eq!(
            err,
            EngineError::RungQuestionUnresolvable {
                section: "sc-01".into(),
                anchor: at("종득은"),
                reason: RungQuestionFault::Unlocatable(Box::new(
                    crate::ProseError::PrefixAmbiguous {
                        source: "MANUSCRIPT.md".into(),
                        prefix: "종득은".into(),
                        occurrences: 2,
                    }
                )),
            }
        );
        // Extending the prefix to name one line is accepted — the ambiguity was
        // real, not a refusal to anchor on a recurring name.
        assert!(PlayableProjection::from_report_and_passages(
            name_report(),
            &ladder_of(&[at("종득은 셈을")]),
            sc01_multi_line_passages(),
        )
        .is_ok());
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
        let view = proj.scene("main", "sc-01", &HashSet::new());
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
        // R757 B1 — the field-report sec 5 discourse-order invariant. `ghost` is
        // authored on a WITHHELD fact (no locator), so it is never disclosed and
        // never referenceable — disclosure is additive too (non-vacuous exclusion).
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
