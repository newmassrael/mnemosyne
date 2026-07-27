//! mnemosyne-engine — the default playable-projection kernel (P1).
//!
//! The Narrative Studio's read-side engine (design:
//! `claudedocs/mnemosyne-engine-design.md`). It consumes `report-playable-world`
//! (a pure read projection of the frozen fact store) and produces a per-world,
//! per-section stream of DISCLOSED narrative — every line provenance-bound to
//! the store fact it projects. The store is never mutated; this crate depends
//! only inward (`ops` -> `validate` -> `core`) and nothing depends back on it,
//! so it is presentation- and execution-agnostic (a renderer / a statechart
//! executor consume its output; it consumes neither).
//!
//! # The provenance contract (why invention is unrepresentable)
//!
//! [`Line`] is the ONLY carrier of narrative content, and it is
//! `#[non_exhaustive]` with no public constructor: a downstream presentation
//! crate can READ a line's `fact_id`/`text` but can never FABRICATE one from a
//! free string. So a renderer cannot put a sentence on the narrative surface
//! that no store fact backs — the class of bug that shipped through a consumer's
//! chrome template (the tide field report: an engine that "invented narrative"
//! with zero store backing) is a compile error here, not a test to remember.
//! This is the R643 detectable->unrepresentable doctrine moved to the engine's
//! type boundary.
//!
//! That promise was true of the RENDER side and false of the INGEST side until
//! Round 771. `from_report` was public and every type in a `PlayableWorldReport`
//! is a pub-field struct, so a downstream crate could hand the kernel a
//! fabricated world and receive real `Line`s minted from it — the guard sat one
//! layer above the hole (R764 found it while reviewing a request to make that
//! same boundary `Deserialize`). `from_report` is now crate-private, leaving
//! [`ProjectionParts`] as the single ingestion door: still plain data by
//! necessity (generated code cannot construct a `#[non_exhaustive]` type), but
//! ONE door instead of two, and one whose contents a build generates from the
//! store rather than a hand writes. What remains is what the threat model always
//! allowed: an author who edits their own build script forges their own game.
//!
//! The narrowing is proven by a PAIR of doctests, because a lone `compile_fail`
//! passes when the snippet fails for any reason at all — a typo'd path would
//! "prove" the guard while proving nothing. The positive half must compile, so a
//! wrong path breaks the suite instead of faking a pass:
//!
//! ```
//! // The ingestion door that IS public.
//! let _open = mnemosyne_engine::PlayableProjection::from_parts;
//! ```
//!
//! and the negative half must not, with the SAME path spelling:
//!
//! ```compile_fail
//! let _open = mnemosyne_engine::PlayableProjection::from_parts;
//! let _shut = mnemosyne_engine::PlayableProjection::from_report;
//! ```
//!
//! A withheld fact emits no locator, so it never becomes a [`Line`]: the store
//! filters disclosure additively, and this kernel never re-implements a
//! subtractive withhold filter.

mod gate;
mod overrides;
mod projection;
mod prose;
mod quest;
#[cfg(test)]
mod test_support;
mod types;

pub use gate::GateViolation;
pub use mnemosyne_core::{DisclosureMode, Modality, MAIN_BRANCH};
pub use mnemosyne_validate::continuity::QuestState;
pub use overrides::{DefaultOverrides, EngineOverrides, OverrideLoadError, StaticOverrides};
pub use projection::{fresh_disclosure, PlayableProjection, ProjectionParts, SectionLines};
pub use prose::{ContentAnchor, ContentSource, Locator, Passage, PrefixSlices, ProseError};
pub use quest::{
    QuestCompletionView, QuestGateViolation, QuestProjection, QuestView, QuestWorldView,
};
pub use types::{
    CastMember, CastPart, ChoiceEntityRef, Door, DoorPart, Fork, ForkPart, Interactivity, Line,
    LinePart, Rung, SceneView,
};

use std::fmt;

/// A fault projecting the playable world. Fail-loud: the kernel never silently
/// drops a locator or invents a fallback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    /// A locator named a `fact_id` absent from its world's `begins` events —
    /// the playable-world read is stale relative to the store (regenerate it).
    /// A dangling pointer is a hard error, never a silent drop.
    LocatorFactMissing {
        /// The world-line whose locator dangled.
        world: String,
        /// The `fact_id` the locator named but no `begins` event carried.
        fact_id: String,
    },
    /// The underlying `report-playable-world` projection failed (an unregistered
    /// world, a typo'd telling, an unreadable store).
    Projection(String),
    /// A ladder rung declared a `question_anchor` (R759 P3c-2) but the engine
    /// could not resolve the question prose from the section's store
    /// `content_excerpt` — the rung claims provenance the store does not back.
    /// Fail-loud: a manuscript-less consumer cannot render a fabricated door
    /// label. Raised at projection, never a silent fallback to the free string.
    RungQuestionUnresolvable {
        /// The section the rung is dug at.
        section: String,
        /// The content anchor the rung declared for its question prose.
        anchor: ContentAnchor,
        /// Why the anchor did not resolve.
        reason: RungQuestionFault,
    },
}

/// Why a [`Rung`]'s `question_anchor` did not resolve against the store
/// ([`EngineError::RungQuestionUnresolvable`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RungQuestionFault {
    /// The section carries no store `content_excerpt`, so there is no prose to
    /// anchor the question to.
    SectionHasNoExcerpt,
    /// The section HAS an excerpt, but the rung's anchor does not locate inside
    /// it (Round 767) — a different document, a prefix the prose does not carry,
    /// a prefix naming more than one place, or the same hold declared twice. The
    /// [`ProseError`] says which; the rung's provenance claim is false either way.
    /// Boxed so carrying the cause does not widen every `Result` on the read path
    /// (`clippy::result_large_err`): the fault is rare, the return is not.
    Unlocatable(Box<ProseError>),
    /// The ladder's declared rung order is not the order its anchors occur in the
    /// section's prose (Round 767) — the author's nth hold is not the nth
    /// passage, so the numbering does not describe the scene as written. A
    /// LADDER invariant, deliberately not the generic slicer's (R766).
    OutOfProseOrder,
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineError::LocatorFactMissing { world, fact_id } => write!(
                f,
                "locator points at `{fact_id}` but world `{world}` has no such begins event \
                 — playable-world is stale relative to the store"
            ),
            EngineError::Projection(msg) => {
                write!(f, "playable-world projection failed: {msg}")
            }
            EngineError::RungQuestionUnresolvable {
                section,
                anchor,
                reason,
            } => {
                let why = match reason {
                    RungQuestionFault::SectionHasNoExcerpt => {
                        "the section has no store content excerpt to bind the question to"
                            .to_string()
                    }
                    RungQuestionFault::Unlocatable(err) => {
                        format!("it does not locate in the section's content excerpt: {err}")
                    }
                    RungQuestionFault::OutOfProseOrder => {
                        "the ladder's declared rung order is not the order its anchors occur in \
                         the section's prose"
                            .to_string()
                    }
                };
                write!(
                    f,
                    "rung at `{section}` declares question anchor `{}` but {why} \
                     — the question prose is not store-backed",
                    anchor.source
                )
            }
        }
    }
}

impl std::error::Error for EngineError {}

/// Read every registered entity's declared kind from the store — `entity_id ->
/// kind` over the whole registry.
///
/// The raw kind registry, for a consumer that needs the whole map: which entities
/// have screen names, which are places, which its own registries must agree with.
/// A read-through of [`mnemosyne_ops::entity_kinds`] so a consumer talks to its
/// kernel rather than past it into `ops`.
///
/// This said "[`Interactivity::objects`] is FED, not derived here" until Round
/// 768, which is no longer true and was never load-bearing: the objects axis IS
/// derived from this registry now, by [`store_interactivity`]. The consumer gate
/// that used to prove its hand-fed set equalled the store's `kind:object` set is
/// what showed the derivation was exact in the first place.
///
/// # Errors
///
/// [`EngineError::Projection`] if the store (or its sidecar) cannot be read.
pub fn store_entity_kinds(
    workspace_root: &std::path::Path,
) -> Result<std::collections::BTreeMap<String, String>, EngineError> {
    mnemosyne_ops::entity_kinds(workspace_root, None)
        .map_err(|e| EngineError::Projection(e.to_string()))
}

/// The FILES this workspace's projections read (Round 772) — the discovered
/// config, the atomic sidecar it names, the canon-order file it declares. A
/// read-through of [`mnemosyne_ops::projection_inputs`], so a consumer talks to
/// its kernel rather than past it into `ops`.
///
/// A build-time bake declares these to cargo (`cargo:rerun-if-changed`) so a
/// store edit regenerates the artifact. The list is RESOLVED rather than assumed
/// — see the `ops` doc for the staleness R770 shipped by assuming it.
///
/// # Errors
///
/// [`EngineError::Projection`] if the config cannot be read or the sidecar
/// cannot be resolved.
pub fn projection_inputs(
    workspace_root: &std::path::Path,
) -> Result<Vec<std::path::PathBuf>, EngineError> {
    mnemosyne_ops::projection_inputs(workspace_root, None)
        .map_err(|e| EngineError::Projection(e.to_string()))
}

/// Provenance-bound narrative prose FROM THE STORE (R757 P3b) — `section_id ->
/// Passage`. Reads each section's `content_excerpt` (R756 P3a) via
/// [`mnemosyne_ops::section_content_excerpts`] and projects it with
/// `Passage::from_excerpt` (the store-cache model), so a manuscript-less consumer
/// (a generic renderer, pinion) gets prose bound to its manuscript anchor WITHOUT
/// the engine holding the manuscript. The excerpt was sha-pinned at ingestion and
/// is trusted like a [`Line`]; drift is `scan_content_drift`'s separate offline
/// guard. Sections with no excerpt are omitted.
///
/// This is the store-owned generalization of a per-consumer anchor file (R756 P3):
/// the narrative prose anchor + projected text live in the store's Section, so the
/// engine hands any consumer the same provenance-bound `Passage` with no manuscript
/// and no anchor file of its own.
///
/// # Errors
///
/// [`EngineError::Projection`] if the store (or its sidecar) cannot be read.
pub fn store_passages(
    workspace_root: &std::path::Path,
) -> Result<std::collections::HashMap<String, Passage>, EngineError> {
    let excerpts = mnemosyne_ops::section_content_excerpts(workspace_root, None)
        .map_err(|e| EngineError::Projection(e.to_string()))?;
    Ok(excerpts
        .into_iter()
        .map(|(id, ex)| (id, Passage::from_excerpt(&ex)))
        .collect())
}

/// The consumer's interactive layer, DERIVED FROM THE STORE (Round 768) — the
/// half of [`Interactivity`] that is world structure rather than consumer policy.
///
/// Before this, a consumer built its own `Interactivity` from its own files and
/// handed it in, which meant the interactive layer lived in a coordinate space
/// the kernel could not see and, worse, that baking the projection at build time
/// required the consumer to run its own derivation in a build script. Both halves
/// come from the store now:
///
/// - `objects` = every registered entity of kind `object`. This was already exact
///   without anyone relying on it: the first consumer's own gate asserts its
///   object-id set EQUALS the store's `kind:object` set in both directions, so
///   reading it here reproduces that set by construction rather than by
///   agreement. Object NAMES stay the consumer's — a name is a word of the novel,
///   not world structure.
/// - `ladders` = each section's authored [`SectionLadder`](mnemosyne_atomic::SectionLadder)
///   (R765), expanded into the kernel's rung model. Only DIALOGUE ladders (a
///   carrier is named) become ask-ladders: an observation ladder's facts are
///   opened by examining its object, which is the objects axis, and offering them
///   as questions would put one fact behind two doors.
///
/// A declared rung that reveals N facts becomes N kernel rungs sharing one
/// question ANCHOR — the kernel prices a rung per fact, the author writes one
/// hold. The question text is not resolved here: the anchor travels into
/// [`PlayableProjection::from_workspace`], which resolves every ladder against
/// its section's own prose (R767). So this function reads coordinates and the
/// projection reads prose, the same split R765 drew in the store.
///
/// `free_investigate` is left at its default: it is policy about how a consumer's
/// own investigate verb behaves, not world structure, so the caller sets it —
/// `Interactivity { free_investigate: true, ..store_interactivity(root)? }`.
///
/// # Errors
///
/// [`EngineError::Projection`] if the store (or its sidecar) cannot be read.
pub fn store_interactivity(workspace_root: &std::path::Path) -> Result<Interactivity, EngineError> {
    let objects = store_entity_kinds(workspace_root)?
        .into_iter()
        .filter(|(_, kind)| kind == "object")
        .map(|(id, _)| id)
        .collect();
    let declared = mnemosyne_ops::section_ladders(workspace_root, None)
        .map_err(|e| EngineError::Projection(e.to_string()))?;
    let ladders = declared
        .into_iter()
        // An observation ladder (no carrier) is opened by examining its rungs'
        // objects, so it is the objects axis and never an ask-ladder.
        .filter(|(_, ladder)| ladder.carrier.is_some())
        .filter_map(|(section, ladder)| {
            let rungs: Vec<Rung> = ladder
                .rungs
                .iter()
                .flat_map(|declared| {
                    declared.reveals.iter().map(move |fact| Rung {
                        // Never rendered: an anchored rung's label is resolved
                        // from the section's prose at projection (R767). Empty
                        // rather than a plausible-looking placeholder, so a bug
                        // that skipped resolution would show as a blank door
                        // rather than as an invented sentence.
                        question: String::new(),
                        question_anchor: Some(declared.anchor.clone()),
                        reveals: fact.clone(),
                        needs: declared.needs.clone(),
                    })
                })
                .collect();
            // A dialogue ladder whose every rung reveals nothing carries no gate
            // and no door; it is prose, and prose is already the projection's.
            (!rungs.is_empty()).then_some((section, rungs))
        })
        .collect();
    Ok(Interactivity {
        objects,
        ladders,
        free_investigate: false,
    })
}

#[cfg(test)]
mod tests {
    use super::{store_entity_kinds, store_interactivity, store_passages};
    use tempfile::TempDir;

    /// Round 768 — a store holding one DIALOGUE ladder (a carrier, two holds, the
    /// second opening two facts), one OBSERVATION ladder (no carrier), and two
    /// entity kinds.
    fn laddered_store(root: &std::path::Path) {
        std::fs::write(
            root.join("mnemosyne.toml"),
            "[workspace]\nroot = \".\"\n\n[atomic]\nsidecar_path = \"store.json\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join("store.json"),
            r#"{"schema_version":43,"frames":{},"narrative_facts":{},
               "entities":{"ent-post":{"kind":"object"},"ent-weir":{"kind":"place"}},
               "sections":{
                 "d01-nat":{"ladder":{"carrier":"ent-jongdeuk","rungs":[
                   {"anchor":{"source":"M.md","locator":{"Prefix":"이름을"}},
                    "reveals":["f-name"]},
                   {"anchor":{"source":"M.md","locator":{"Prefix":"물때를"}},
                    "needs":["f-name"],"reveals":["f-tide","f-count"]}
                 ]}},
                 "d02-nat":{"ladder":{"rungs":[
                   {"anchor":{"source":"M.md","locator":{"Prefix":"말뚝을"}},
                    "reveals":["f-post"],"object":"ent-post"}
                 ]}}
               }}"#,
        )
        .unwrap();
    }

    #[test]
    fn store_interactivity_derives_objects_and_dialogue_ladders_from_the_store() {
        // Round 768 — the interactive layer stops being consumer-built. Objects
        // are the store's kind:object entities; ladders are the store's authored
        // holds expanded to the kernel's per-fact rung model.
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        laddered_store(root);
        let interactivity = store_interactivity(root).expect("kernel derives the layer");

        // Objects are exactly the store's kind:object — a place is not examinable
        // furniture, and nothing here is fed in by a consumer.
        assert_eq!(
            interactivity.objects,
            std::collections::HashSet::from(["ent-post".to_string()])
        );

        // The OBSERVATION ladder (no carrier) is not an ask-ladder: its fact is
        // opened by examining ent-post, and offering it as a question too would
        // put one fact behind two doors.
        assert!(
            !interactivity.ladders.contains_key("d02-nat"),
            "an observation ladder must not become an ask-ladder"
        );

        // The DIALOGUE ladder fans out per revealed fact: 1 + 2 facts = 3 rungs,
        // in declared order, each carrying its hold's anchor and needs.
        let rungs = &interactivity.ladders["d01-nat"];
        assert_eq!(rungs.len(), 3);
        let reveals: Vec<&str> = rungs.iter().map(|r| r.reveals.as_str()).collect();
        assert_eq!(reveals, vec!["f-name", "f-tide", "f-count"]);
        assert!(rungs[0].needs.is_empty());
        assert_eq!(rungs[1].needs, vec!["f-name".to_string()]);
        assert_eq!(rungs[2].needs, vec!["f-name".to_string()]);
        // The two rungs of the SECOND hold share ONE anchor — they are the same
        // hold, priced twice. The projection's resolver must therefore tolerate a
        // repeated anchor (it dedupes) even though the store rejects a duplicated
        // one within a single declaration.
        assert_eq!(rungs[1].question_anchor, rungs[2].question_anchor);
        assert_ne!(rungs[0].question_anchor, rungs[1].question_anchor);
        // No question text is invented here: the label comes from the section's
        // own prose at projection, so a bug that skipped resolution shows as a
        // blank door rather than as a sentence nobody wrote.
        assert!(rungs.iter().all(|r| r.question.is_empty()));

        // free_investigate is the consumer's knob, not world structure.
        assert!(!interactivity.free_investigate);
    }

    /// The kernel's read-through of the store's entity registry — `id -> kind`
    /// for the whole registry, what a consumer validates its own kind registry
    /// against. (The read lives in `mnemosyne_ops::entity_kinds`; this proves the
    /// kernel re-export tide's object/place gates call.)
    #[test]
    fn store_entity_kinds_reads_the_registry_through_the_kernel() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::write(
            root.join("mnemosyne.toml"),
            "[workspace]\nroot = \".\"\n\n[atomic]\nsidecar_path = \"store.json\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join("store.json"),
            r#"{"schema_version":39,"sections":{},"frames":{},"narrative_facts":{},
               "entities":{"ent-post":{"kind":"object"},"ent-weir":{"kind":"place"}}}"#,
        )
        .unwrap();
        let kinds = store_entity_kinds(root).expect("kernel reads the store kinds");
        assert_eq!(kinds.get("ent-post").map(String::as_str), Some("object"));
        assert_eq!(kinds.get("ent-weir").map(String::as_str), Some("place"));
        assert_eq!(kinds.len(), 2);
    }

    /// R757 P3b — the kernel projects the store's `content_excerpt`s into
    /// provenance-bound `Passage`s, so a manuscript-less consumer gets narrative
    /// prose FROM THE STORE. A section with no excerpt yields no passage (no
    /// invented prose), the fail-loud omission.
    #[test]
    fn store_passages_projects_content_excerpts_from_the_store() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::write(
            root.join("mnemosyne.toml"),
            "[workspace]\nroot = \".\"\n\n[atomic]\nsidecar_path = \"store.json\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join("store.json"),
            r#"{"schema_version":41,"frames":{},"narrative_facts":{},"entities":{},
               "sections":{
                 "d01-nat":{"content_excerpt":{
                   "anchor":{"source":"MANUSCRIPT.md","locator":{"Prefix":"지운은"}},
                   "text":"지운은 둑에 발을 올렸다.","text_sha256":""}},
                 "d02-nat":{}
               }}"#,
        )
        .unwrap();
        let passages = store_passages(root).expect("kernel reads store excerpts");
        // The section WITH an excerpt projects a provenance-bound passage (text +
        // its manuscript anchor, both from the store — no manuscript needed here).
        let p = passages.get("d01-nat").expect("d01-nat has a passage");
        assert_eq!(p.text(), "지운은 둑에 발을 올렸다.");
        assert_eq!(p.anchor().source, "MANUSCRIPT.md");
        // The section WITHOUT an excerpt is omitted — the kernel invents nothing.
        assert!(!passages.contains_key("d02-nat"));
        assert_eq!(passages.len(), 1);
    }
}
