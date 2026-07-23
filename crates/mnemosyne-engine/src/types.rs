//! The provenance-bound value types the kernel hands the presentation layer.

use std::collections::{HashMap, HashSet};

use mnemosyne_atomic::ScenePresence;
use mnemosyne_core::{DisclosureMode, Modality};
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
/// // The fields are crate-private (and `Line` is #[non_exhaustive]), so a
/// // struct literal from another crate does not compile.
/// let _ = Line {
///     fact_id: "f-invented".to_string(),
///     text: "the engine made this up".to_string(),
///     mode: DisclosureMode::State,
///     frame: String::new(),
///     entities: Vec::new(),
///     carrier: None,
///     typed_predicate: None,
///     quote: None,
///     count: None,
/// };
/// ```
///
/// Nor does clone-and-overwrite — a real seed line is freely available (every
/// `SceneView.lines` hands them out), but its content cannot be mutated:
///
/// ```compile_fail
/// // `text` is crate-private, so overwriting it on an owned clone does not
/// // compile — a downstream crate cannot fake a line's content.
/// fn forge(seed: &mnemosyne_engine::Line) -> String {
///     let mut forged = seed.clone();
///     forged.text = "the engine made this up".to_string();
///     forged.text
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
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Line {
    /// Provenance — the `narrative_facts` key this line projects.
    pub(crate) fact_id: String,
    /// The authored claim from the store (the fact's own words).
    pub(crate) text: String,
    /// `state`/`hint`/`imply` — never [`DisclosureMode::Withhold`] (a withheld
    /// fact emits no locator, so it never reaches here).
    pub(crate) mode: DisclosureMode,
    /// Whose knowledge — the store's epistemic frame. `"ground-truth"` = the
    /// world asserts it; anything else = a named character believes/says it
    /// (see [`Line::is_belief`]). May be empty when the store left it unframed.
    pub(crate) frame: String,
    /// The entities the store attached to this fact (people/objects/places
    /// mixed; splitting them by kind is the consumer's job via its registries).
    pub(crate) entities: Vec<String>,
    /// The diegetic carrier the disclosure rides on (`surface.object`), when an
    /// authored surface names one; often `None`.
    pub(crate) carrier: Option<String>,
    /// The typed-claim predicate (e.g. `pursues`/`requires`/`completed_by`) when
    /// this fact is a typed leg — surfaced so a consumer can route quest-journal
    /// facts out of the prose stream without the kernel guessing a policy (that
    /// policy is a consumer override, never a kernel default).
    pub(crate) typed_predicate: Option<String>,
    /// The store's verbatim quote for this fact, when authored (vs the
    /// paraphrased `text`/claim) — a styling axis a renderer may set in
    /// quotation treatment. `None` = no authored quote.
    pub(crate) quote: Option<String>,
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
    #[must_use]
    pub fn entities(&self) -> &[String] {
        &self.entities
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
            fact_id: locator.fact_id.clone(),
            text: begin.claim.clone(),
            mode: locator.mode,
            frame: begin.frame.clone(),
            entities: begin.entities.clone(),
            carrier: locator.object.clone(),
            typed_predicate: begin.typed.as_ref().map(|t| t.predicate.clone()),
            quote: begin.quote.clone(),
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

/// One authored step of a ladder — a question whose answer reveals a store
/// fact, optionally gated behind preconditions. A CONSUMER INPUT (authored
/// data), so it is plainly constructible: the provenance guarantee is that the
/// leak gate rejects a `reveals` the store does not offer, not that the rung is
/// unconstructible.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Rung {
    /// The authored question (the door label; novel-anchored upstream by the
    /// consumer's authoring pipeline, not re-checked here).
    pub question: String,
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
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
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
