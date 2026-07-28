//! The provenance-bound value types the kernel hands the presentation layer.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use mnemosyne_atomic::ScenePresence;
use mnemosyne_core::{ContentAnchor, DisclosureMode, Modality};
use mnemosyne_validate::continuity::{ManuscriptFactEvent, MapLocator};

/// A single disclosed narrative unit — the ONLY carrier of narrative content to
/// the presentation layer, and every one is provenance-bound.
///
/// # The provenance contract (invention is unrepresentable)
///
/// A downstream crate can READ a line through its accessors but can never
/// synthesize one whose `fact_id` names no store fact:
/// - the fields are **crate-private** (`pub(crate)`) — a downstream crate cannot
///   name them, so it cannot build one with a struct literal, and cannot
///   overwrite them on a clone either (closing the clone-and-mutate forgery a
///   `#[non_exhaustive]`-only guard would have missed);
/// - there is no public constructor, no `Default`, no `Deserialize`;
/// - the sole constructor is [`Line::from_disclosed`] (crate-private), which
///   builds from a real `(MapLocator, ManuscriptFactEvent)` pair whose `fact_id`
///   has already joined against the store's `begins`.
///
/// So a renderer can never surface a sentence no store fact backs — invention is
/// a compile error, not a test to remember (R643 detectable->unrepresentable at
/// the type boundary). Struct-literal construction does not compile:
///
/// ```compile_fail
/// use mnemosyne_engine::{DisclosureMode, Line};
/// // `Line` is #[non_exhaustive], so a struct literal from another crate does
/// // not compile. The values are `todo!()` ON PURPOSE: `!` coerces to any type,
/// // so the only thing this can fail on is the guard, whatever the fields are
/// // declared to hold. Round 795 changed those types and found the earlier
/// // version of this test failing on four type mismatches instead — a
/// // `compile_fail` that would have stayed green with the guard removed.
/// let _ = Line {
///     fact_id: todo!(),
///     text: todo!(),
///     mode: DisclosureMode::State,
///     frame: todo!(),
///     entities: todo!(),
///     carrier: None,
///     typed_predicate: None,
///     quote: None,
///     count: None,
/// };
/// ```
///
/// That one proves the `#[non_exhaustive]` half. The PRIVACY half is the
/// clone-and-overwrite below — a real seed line is freely available (every
/// `SceneView.lines` hands them out), but its content cannot be mutated, and this
/// is the test that reports `E0616`, field is private:
///
/// ```compile_fail
/// // `text` is crate-private, so overwriting it on an owned clone does not
/// // compile — a downstream crate cannot fake a line's content.
/// fn forge(seed: &mnemosyne_engine::Line) -> String {
///     let mut forged = seed.clone();
///     forged.text = todo!();
///     forged.text.into_owned()
/// }
/// ```
///
/// # Styling hooks (the semantic axes a themed renderer keys off)
///
/// A `Line` exposes the store's fact-level SEMANTIC axes through accessors so a
/// downstream renderer maps them to visual style (color, weight, letter-spacing)
/// WITHOUT the kernel owning a pixel: [`mode`](Line::mode) (tone), [`frame`](Line::frame)
/// / [`is_belief`](Line::is_belief) (world truth vs a character's voice),
/// [`entities`](Line::entities), [`quote`](Line::quote) (verbatim vs paraphrase),
/// [`count`](Line::count) (multiplicity), [`typed_predicate`](Line::typed_predicate)
/// (quest legs). The visual mapping and any theme OVERRIDE live in the
/// presentation layer, never here — the kernel supplies meaning, the renderer
/// supplies looks.
///
/// # How the strings are held (Round 795)
///
/// Every string here is a `Cow<'static, str>` and the entity list a
/// `Cow<'static, [Cow<'static, str>]>`, which is an IMPLEMENTATION DETAIL — the
/// accessors below still hand out `&str`, so no consumer learns the
/// representation. What it buys is that a BAKED line points at the literals in
/// the binary instead of copying them, while [`Line::from_disclosed`] keeps
/// owning because it derives its text at run time. One type with two ownership
/// modes rather than two types, the Round 785 decision.
///
/// The list is a `Cow` and not a `Vec<Cow<'static, str>>` because Round 794
/// measured the difference and it is the whole remainder: a `Vec` spine costs one
/// allocation per line whatever the strings do, which is 80% of the win rather
/// than 100%. It costs nothing at the API, since both forms expose exactly the
/// same accessor.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Line {
    /// Provenance — the `narrative_facts` key this line projects.
    pub(crate) fact_id: Cow<'static, str>,
    /// The authored claim from the store (the fact's own words).
    pub(crate) text: Cow<'static, str>,
    /// `state`/`hint`/`imply` — never [`DisclosureMode::Withhold`] (a withheld
    /// fact emits no locator, so it never reaches here).
    pub(crate) mode: DisclosureMode,
    /// Whose knowledge — the store's epistemic frame. `"ground-truth"` = the
    /// world asserts it; anything else = a named character believes/says it
    /// (see [`Line::is_belief`]). May be empty when the store left it unframed.
    pub(crate) frame: Cow<'static, str>,
    /// The entities the store attached to this fact (people/objects/places
    /// mixed; splitting them by kind is the consumer's job via its registries).
    pub(crate) entities: Cow<'static, [Cow<'static, str>]>,
    /// The diegetic carrier the disclosure rides on (`surface.object`), when an
    /// authored surface names one; often `None`.
    pub(crate) carrier: Option<Cow<'static, str>>,
    /// The typed-claim predicate (e.g. `pursues`/`requires`/`completed_by`) when
    /// this fact is a typed leg — surfaced so a consumer can route quest-journal
    /// facts out of the prose stream without the kernel guessing a policy (that
    /// policy is a consumer override, never a kernel default).
    pub(crate) typed_predicate: Option<Cow<'static, str>>,
    /// The store's verbatim quote for this fact, when authored (vs the
    /// paraphrased `text`/claim) — a styling axis a renderer may set in
    /// quotation treatment. `None` = no authored quote.
    pub(crate) quote: Option<Cow<'static, str>>,
    /// The asserted multiplicity riding this fact (R731 `fact_counts`), when
    /// authored — a renderer may annotate it (e.g. "×3"). Never summed; `None`
    /// = no authored multiplicity.
    pub(crate) count: Option<i64>,
}

impl Line {
    const GROUND_TRUTH: &'static str = "ground-truth";

    /// Is this a character's belief/report rather than ground truth? The store
    /// keeps a believed-fact and its ground-truth counterpart as DISTINCT facts;
    /// a renderer that flattens the two robs the player of the distinction (a
    /// character's guess vs the world's fact).
    #[must_use]
    pub fn is_belief(&self) -> bool {
        !self.frame.is_empty() && self.frame != Self::GROUND_TRUTH
    }

    /// Is this the world's ground truth (not a character's belief/report)? The
    /// symmetric styling axis to [`Line::is_belief`] — a themed renderer sets
    /// truth and hearsay apart. An unframed line counts as ground truth.
    #[must_use]
    pub fn is_ground_truth(&self) -> bool {
        self.frame.is_empty() || self.frame == Self::GROUND_TRUTH
    }

    /// Provenance — the `narrative_facts` key this line projects.
    #[must_use]
    pub fn fact_id(&self) -> &str {
        &self.fact_id
    }

    /// The authored claim from the store (the fact's own words).
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The disclosure tone (`state`/`hint`/`imply`; never `withhold`).
    #[must_use]
    pub fn mode(&self) -> DisclosureMode {
        self.mode
    }

    /// The epistemic frame (`"ground-truth"` or a character's frame; may be
    /// empty). See [`Line::is_belief`] / [`Line::is_ground_truth`].
    #[must_use]
    pub fn frame(&self) -> &str {
        &self.frame
    }

    /// The entities the store attached to this fact.
    ///
    /// # Why an iterator and not a slice (Round 795)
    ///
    /// This is the one accessor whose form the Round 785 design left open, on the
    /// grounds that it needs the call sites. With them in hand there is exactly
    /// one caller outside this crate, so ergonomics did not decide it — the
    /// design's own commitment did. `Cow` is meant to stay an implementation
    /// detail, and `&[Cow<'static, str>]` would publish it in the signature of
    /// the most-read accessor on the most-read type; every consumer would then
    /// match on a representation the kernel reserves the right to change.
    ///
    /// An iterator of `&str` hides it completely, and hides it for BOTH holdings:
    /// Round 794 verified by compilation that a `Vec<Cow<..>>` and a
    /// `Cow<'static, [Cow<..>]>` yield the identical `&str` sequence, so this
    /// signature does not pin the field either. It also keeps Round 786's
    /// property, since a `&'static self` yields `&'static str` items by the same
    /// elision that made the handle work.
    ///
    /// `ExactSizeIterator` so a caller that only wants the count does not collect.
    pub fn entities(&self) -> impl ExactSizeIterator<Item = &str> {
        self.entities.iter().map(AsRef::as_ref)
    }

    /// The diegetic carrier the disclosure rides on (`surface.object`).
    #[must_use]
    pub fn carrier(&self) -> Option<&str> {
        self.carrier.as_deref()
    }

    /// The typed-claim predicate when this fact is a typed leg.
    #[must_use]
    pub fn typed_predicate(&self) -> Option<&str> {
        self.typed_predicate.as_deref()
    }

    /// The store's verbatim quote for this fact, when authored.
    #[must_use]
    pub fn quote(&self) -> Option<&str> {
        self.quote.as_deref()
    }

    /// The asserted multiplicity riding this fact, when authored.
    #[must_use]
    pub fn count(&self) -> Option<i64> {
        self.count
    }

    /// Build a line from a disclosed `(locator, begin)` pair. Crate-private: the
    /// only path to a `Line`, and it always carries a real joined `fact_id`. The
    /// caller has already confirmed `begin.fact_id == locator.fact_id` via the
    /// begins index.
    pub(crate) fn from_disclosed(locator: &MapLocator, begin: &ManuscriptFactEvent) -> Self {
        Self {
            fact_id: Cow::Owned(locator.fact_id.clone()),
            text: Cow::Owned(begin.claim.clone()),
            mode: locator.mode,
            frame: Cow::Owned(begin.frame.clone()),
            entities: Cow::Owned(begin.entities.iter().cloned().map(Cow::Owned).collect()),
            carrier: locator.object.clone().map(Cow::Owned),
            typed_predicate: begin
                .typed
                .as_ref()
                .map(|t| Cow::Owned(t.predicate.clone())),
            quote: begin.quote.clone().map(Cow::Owned),
            count: begin.count,
        }
    }
}

/// A character present in a scene (Round 757, B1b) — projected from the store's
/// authored `scene_cast`, the ONLY cast source a consumer reads. Provenance-bound
/// like [`Line`]: the fields are crate-private with no public constructor, and the
/// sole ctor [`CastMember::from_presence`] builds from a real store
/// [`ScenePresence`], so a downstream crate READS who is present but can never
/// FABRICATE a presence (the field-report parallel-identity class is unrepresentable
/// here too). The authored `modality`/`can_answer` are the store's world-truth
/// (never engine-re-derived), and `quote` is the manuscript excerpt proving the
/// presence. Struct-literal construction from another crate does not compile:
///
/// ```compile_fail
/// use mnemosyne_engine::{CastMember, Modality};
/// let _ = CastMember {
///     entity: "ent-invented".to_string(),
///     modality: Modality::Observed,
///     can_answer: true,
///     quote: "the engine made this up".to_string(),
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct CastMember {
    /// The store entity id present in the scene.
    pub(crate) entity: String,
    /// The authored evidentiary stance behind the presence (world-truth).
    pub(crate) modality: Modality,
    /// The authored judgment: can this presence answer the reckoner's questions?
    pub(crate) can_answer: bool,
    /// The manuscript quote proving the presence (the store excerpt text).
    pub(crate) quote: String,
}

impl CastMember {
    /// The store entity id present in the scene.
    #[must_use]
    pub fn entity(&self) -> &str {
        &self.entity
    }

    /// The authored evidentiary stance behind the presence.
    #[must_use]
    pub fn modality(&self) -> Modality {
        self.modality
    }

    /// The authored judgment: can this presence answer questions?
    #[must_use]
    pub fn can_answer(&self) -> bool {
        self.can_answer
    }

    /// The manuscript quote proving the presence.
    #[must_use]
    pub fn quote(&self) -> &str {
        &self.quote
    }

    /// Build a cast member from a store scene presence. Crate-private: the only
    /// path to a `CastMember`, and it always carries a real store `ScenePresence`
    /// (its excerpt already sha-pinned at ingestion), so a consumer cannot invent
    /// who is present.
    pub(crate) fn from_presence(p: &ScenePresence) -> Self {
        Self {
            entity: p.entity.clone(),
            modality: p.modality,
            can_answer: p.can_answer,
            quote: p.excerpt.text.clone(),
        }
    }
}

/// One branch point, derived verbatim from the store's fork tree — the kernel
/// reads topology, never invents it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Fork {
    /// The section the choice opens at.
    pub at: String,
    /// The world-line this forks FROM (usually the `main` trunk).
    pub parent: String,
    /// The world-line this leads TO.
    pub world: String,
    /// The authored choice label (the branch description); may be empty.
    pub label: String,
}

impl Fork {
    pub(crate) fn new(at: String, parent: String, world: String, label: String) -> Self {
        Self {
            at,
            parent,
            world,
            label,
        }
    }
}

/// What the presentation may render at one spot: narrative content is
/// EXCLUSIVELY `lines` (each provenance-bound), and interactive affordances are
/// EXCLUSIVELY `doors` (each provenance-bound). Chrome labels/status are a
/// separate consumer type, never narrative.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SceneView {
    /// The store section id this scene projects.
    pub section: String,
    /// The store section title, when authored.
    pub title: Option<String>,
    /// The disclosed narrative stream for this spot — provenance-only.
    pub lines: Vec<Line>,
    /// The interactive affordances at this spot — provenance-only (fork
    /// navigation or store-fact reveals; never free content).
    pub doors: Vec<Door>,
}

impl SceneView {
    pub(crate) fn new(
        section: String,
        title: Option<String>,
        lines: Vec<Line>,
        doors: Vec<Door>,
    ) -> Self {
        Self {
            section,
            title,
            lines,
            doors,
        }
    }
}

/// An interactive affordance at a spot. Every variant is provenance-bound: a
/// door navigates the store's fork topology or reveals store facts — never free
/// content. Like [`Line`] it is `#[non_exhaustive]`: a renderer READS the doors
/// the kernel derived; the narrative a door reveals resolves to [`Line`]s, so an
/// invented sentence has no slot even here.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Door {
    /// A branch choice — navigates to another world-line (from the fork tree).
    Fork {
        /// The world-line this choice leads to.
        world: String,
        /// The authored choice label (the branch description).
        label: String,
    },
    /// Examine a diegetic object — reveals the offered facts that name it. The
    /// reveals are a subset of the spot's disclosed lines (provenance-bound by
    /// construction: an examine door can never leak an unoffered fact).
    Examine {
        /// The examinable entity id.
        object: String,
        /// The offered `fact_id`s examining it reveals.
        reveals: Vec<String>,
    },
    /// Ask an authored question — a ladder rung. Reveals the answer fact
    /// (provenance enforced by the leak gate, not by construction).
    Ask {
        /// The authored question (the rung's prompt / door label).
        question: String,
        /// The `fact_id` the answer reveals.
        reveals: String,
    },
}

impl Door {
    /// The `fact_id`s this door would disclose to the reader. `Examine` returns
    /// the facts examining its object reveals; `Ask` returns its single answer
    /// fact (a one-element slice); `Fork` returns an empty slice (a navigational
    /// door discloses no fact — its liveness is not-taken, never disclosure).
    ///
    /// This is the uniform surface the disclose-freshness rule reads
    /// ([`fresh_disclosure`](crate::fresh_disclosure), R762 P4b): every
    /// disclose-door — the kernel's own and a consumer's — obeys ONE liveness
    /// definition (a door is LIVE only if it discloses a not-yet-known fact), so
    /// the half-enforced-invariant class (a door-builder that forgets the
    /// known-check, the tide field-report sec 5 dead-`go`-door) cannot recur.
    #[must_use]
    pub fn discloses(&self) -> &[String] {
        match self {
            Door::Fork { .. } => &[],
            Door::Examine { reveals, .. } => reveals,
            Door::Ask { reveals, .. } => std::slice::from_ref(reveals),
        }
    }
}

/// One authored step of a ladder — a question whose answer reveals a store
/// fact, optionally gated behind preconditions. A CONSUMER INPUT (authored
/// data), so it is plainly constructible: the provenance guarantee is that the
/// leak gate rejects a `reveals` the store does not offer, not that the rung is
/// unconstructible.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Rung {
    /// The authored question (the door label). The UN-ANCHORED fallback: when
    /// [`question_anchor`](Self::question_anchor) is `None` this string is the
    /// rendered label as-is (pure interactive chrome, asserting no manuscript
    /// prose). When an anchor IS present the engine resolves the label from the
    /// store excerpt instead and this string is never shown.
    pub question: String,
    /// R759 P3c-2 — an optional content-SSOT anchor binding this rung's question
    /// prose to the section's store `content_excerpt`. When `Some`, the engine
    /// RESOLVES the rendered question from that excerpt at projection and FAILS
    /// LOUD ([`EngineError::RungQuestionUnresolvable`](crate::EngineError::RungQuestionUnresolvable))
    /// if it does not resolve — the declared anchor MUST match the section's
    /// excerpt — so a manuscript-less consumer (a generic renderer, pinion) cannot
    /// fabricate a ladder door label. This extends R755 fork-2 ("a rendered unit
    /// is provenance-bound to a `fact_id` OR a content-anchor") to the ladder
    /// question, the last bare-`String` authored-prose surface in the engine.
    /// A CONSUMER INPUT (Deserialize, like [`reveals`](Self::reveals)): the
    /// guarantee is RESOLUTION, not unconstructibility (`Rung` must stay
    /// Deserialize for `StaticOverrides::load`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question_anchor: Option<ContentAnchor>,
    /// The `fact_id` this rung's answer discloses — provenance-checked by the
    /// leak gate against the facts the store offers at the spot.
    pub reveals: String,
    /// `fact_id`s that must be diggable at-or-before this spot for the rung to
    /// open (the precondition gate; empty = unconditional).
    pub needs: Vec<String>,
}

/// A consumer-declared reference from an interactive CHOICE to a store entity
/// (Round 757, B1) — "at `section`, my `choice` offers/names `entity`". The
/// consumer declares these so the kernel can gate them: a choice may only name an
/// entity the discourse has already DISCLOSED at-or-before its spot
/// ([`PlayableProjection::referenceable_entities`](crate::PlayableProjection::referenceable_entities)),
/// which makes a hand-built parallel-identity choice — the field-report class
/// where a consumer offered strangers the player never met — a fail-loud
/// [`GateViolation::ChoiceReferencesUndisclosedEntity`](crate::GateViolation::ChoiceReferencesUndisclosedEntity)
/// for ANY consumer that declares its refs (the `journal_predicates` contract:
/// the kernel enforces, the consumer declares). A CONSUMER INPUT (authored data),
/// so it is plainly constructible; the guarantee is that the gate rejects an
/// undisclosed reference, not that the ref is unconstructible.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ChoiceEntityRef {
    /// The section the choice is offered at.
    pub section: String,
    /// The store entity id the choice names (must be disclosed at-or-before this
    /// section on the walk).
    pub entity: String,
    /// The choice's label — carried for the diagnostic (which choice leaked); not
    /// gated.
    pub choice: String,
}

/// The consumer-authored interactive layer over a store: per-section ladders
/// (authored Q&A) plus the set of examinable objects. The kernel OPERATES on
/// it; loading it (from files or a trait) is a consumer override built in a
/// later phase. `Default` = no interactivity (only fork doors, all narrative
/// shown directly).
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Interactivity {
    /// Section id -> the authored rung chain dug at that spot.
    pub ladders: HashMap<String, Vec<Rung>>,
    /// Entity ids that are examinable diegetic objects.
    pub objects: HashSet<String>,
    /// Does a ladder gate only the facts behind its rungs/objects (a PARTIAL
    /// ladder), or does entering a ladder spot hide everything not behind a door
    /// (a MODAL ladder)?
    ///
    /// A partial consumer keeps a free fallback that reveals whatever no door
    /// claimed — tide's `investigate` action reveals the spot's remainder, so a
    /// fact is never stranded. For such a consumer the offered-fact-unreachable
    /// check ("does every offered fact have a door?") does not apply: the free
    /// fallback IS the door. Set `true` to declare a partial layer and suppress
    /// that check; leak (a rung reveals an unoffered fact) and precondition timing
    /// still gate.
    ///
    /// Default `false` = modal: the strict check runs (the batteries-included
    /// assumption that a ladder replaces free reading, so a door-less offered fact
    /// is stranded). A modal consumer that forgets a door still fails loud.
    #[serde(default)]
    pub free_investigate: bool,
}

/// A [`Line`] as plain data (Round 769) — the emit/ingest shape of the one type
/// a downstream crate may READ but never FABRICATE.
///
/// `Line` keeps crate-private fields on purpose (invention is unrepresentable),
/// which also means generated code in another crate cannot construct one. So a
/// baked projection carries THIS instead: a pub-field mirror the engine converts
/// back inside the crate, where the private constructor is reachable. The guard
/// it does not weaken is the one that matters — a RENDERER still cannot mint a
/// sentence, because a renderer holds a `Line` and never a part. The guard it
/// does move is the ingestion one, which was already open at the report boundary
/// (R764 weak point 1): a build-time forger and a report forger are the same
/// forger, and neither is the threat model.
#[derive(Debug, Clone, PartialEq, Eq)]
///
/// The fields are `Cow<'static, _>` since Round 795, mirroring [`Line`]: a BAKED
/// part carries `Cow::Borrowed` of the literals the emitter wrote, and
/// [`Line::to_part`] on a live line yields `Cow::Owned`. The two compare equal by
/// CONTENT, which is what keeps the live-to-baked round trip an equality rather
/// than a shape check — and is also why the gate that guards the borrow has to
/// assert the DISCRIMINANT rather than the value.
pub struct LinePart {
    /// Provenance — the `narrative_facts` key this line projects.
    pub fact_id: Cow<'static, str>,
    /// The authored claim from the store.
    pub text: Cow<'static, str>,
    /// How the telling surfaces it; never [`DisclosureMode::Withhold`].
    pub mode: DisclosureMode,
    /// Whose knowledge this is (the store's epistemic frame).
    pub frame: Cow<'static, str>,
    /// The store entities the fact names.
    pub entities: Cow<'static, [Cow<'static, str>]>,
    /// The diegetic carrier the disclosure rides on.
    pub carrier: Option<Cow<'static, str>>,
    /// The typed leg's predicate, when the fact carries one.
    pub typed_predicate: Option<Cow<'static, str>>,
    /// The authored quote backing the fact.
    pub quote: Option<Cow<'static, str>>,
    /// The asserted multiplicity riding the fact.
    pub count: Option<i64>,
}

/// A [`CastMember`] as plain data (Round 769) — the emit/ingest mirror, for the
/// same reason as [`LinePart`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastPart {
    /// The store entity present in the scene.
    pub entity: String,
    /// The authored evidentiary stance behind the presence.
    pub modality: Modality,
    /// Whether this presence can answer the reckoner's questions.
    pub can_answer: bool,
    /// The manuscript quote proving the presence.
    pub quote: String,
}

impl Line {
    /// Ingest a [`LinePart`] — crate-private, so the only way a part becomes a
    /// `Line` is through the engine (Round 769).
    pub(crate) fn from_part(part: LinePart) -> Self {
        Self {
            fact_id: part.fact_id,
            text: part.text,
            mode: part.mode,
            frame: part.frame,
            entities: part.entities,
            carrier: part.carrier,
            typed_predicate: part.typed_predicate,
            quote: part.quote,
            count: part.count,
        }
    }

    /// Emit this line as plain data (Round 769) — what a build-time bake writes.
    #[must_use]
    pub fn to_part(&self) -> LinePart {
        LinePart {
            fact_id: self.fact_id.clone(),
            text: self.text.clone(),
            mode: self.mode,
            frame: self.frame.clone(),
            entities: self.entities.clone(),
            carrier: self.carrier.clone(),
            typed_predicate: self.typed_predicate.clone(),
            quote: self.quote.clone(),
            count: self.count,
        }
    }
}

impl CastMember {
    /// Ingest a [`CastPart`] — crate-private (Round 769).
    pub(crate) fn from_part(part: CastPart) -> Self {
        Self {
            entity: part.entity,
            modality: part.modality,
            can_answer: part.can_answer,
            quote: part.quote,
        }
    }

    /// Emit this cast member as plain data (Round 769).
    #[must_use]
    pub fn to_part(&self) -> CastPart {
        CastPart {
            entity: self.entity.clone(),
            modality: self.modality,
            can_answer: self.can_answer,
            quote: self.quote.clone(),
        }
    }
}

/// A [`Fork`] as plain data (Round 770) — the emit/ingest mirror.
///
/// `Fork` is `#[non_exhaustive]`, which stops a downstream crate from writing a
/// struct literal. That guard is right for a type a renderer READS, and it is
/// also why generated code cannot build one: a baked projection therefore
/// carries this instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForkPart {
    /// The section the choice opens at.
    pub at: String,
    /// The world-line the choice forks from.
    pub parent: String,
    /// The world-line it leads to.
    pub world: String,
    /// The authored choice label.
    pub label: String,
}

/// A [`Door`] as plain data (Round 770) — the emit/ingest mirror, for the same
/// reason as [`ForkPart`]: `Door` is `#[non_exhaustive]`, so a downstream crate
/// cannot name its variants in a literal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DoorPart {
    /// A branch choice.
    Fork {
        /// The world-line this choice leads to.
        world: String,
        /// The authored choice label.
        label: String,
    },
    /// Examine a diegetic object.
    Examine {
        /// The examinable entity id.
        object: String,
        /// The offered `fact_id`s examining it reveals.
        reveals: Vec<String>,
    },
    /// Ask a ladder question, its prose ALREADY resolved.
    Ask {
        /// The resolved question — the door's label.
        question: String,
        /// The `fact_id` the answer reveals.
        reveals: String,
    },
}

impl Fork {
    /// Ingest a [`ForkPart`] — crate-private (Round 770).
    pub(crate) fn from_part(part: ForkPart) -> Self {
        Self {
            at: part.at,
            parent: part.parent,
            world: part.world,
            label: part.label,
        }
    }

    /// Emit this fork as plain data (Round 770).
    #[must_use]
    pub fn to_part(&self) -> ForkPart {
        ForkPart {
            at: self.at.clone(),
            parent: self.parent.clone(),
            world: self.world.clone(),
            label: self.label.clone(),
        }
    }
}

impl Door {
    /// Ingest a [`DoorPart`] — crate-private (Round 770).
    pub(crate) fn from_part(part: DoorPart) -> Self {
        match part {
            DoorPart::Fork { world, label } => Door::Fork { world, label },
            DoorPart::Examine { object, reveals } => Door::Examine { object, reveals },
            DoorPart::Ask { question, reveals } => Door::Ask { question, reveals },
        }
    }

    /// Emit this door as plain data (Round 770).
    #[must_use]
    pub fn to_part(&self) -> DoorPart {
        match self {
            Door::Fork { world, label } => DoorPart::Fork {
                world: world.clone(),
                label: label.clone(),
            },
            Door::Examine { object, reveals } => DoorPart::Examine {
                object: object.clone(),
                reveals: reveals.clone(),
            },
            Door::Ask { question, reveals } => DoorPart::Ask {
                question: question.clone(),
                reveals: reveals.clone(),
            },
        }
    }
}
