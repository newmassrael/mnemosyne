//! The authoring contract as machine-readable data (Round 587, R585 debt item
//! 1) — `describe-schema`.
//!
//! An external generate-gate-repair agent needs to know the authoring contract
//! (which registries exist, what a fact requires, the fixed vocabularies, the
//! narrative-rule classes, the quest encoding, the write-time invariants) to
//! self-serve BEFORE it writes a single fact. Today that knowledge lives only
//! in source doc-comments — the R585 dogfood (`../the-tide-that-counts`) had to
//! read `continuity.rs` to learn the quest convention. This module emits the
//! same contract as a serializable [`SchemaContract`]: a pure static
//! projection, store-independent (the contract is fixed; store CONTENTS are
//! `query`/`list-*`), medium-neutral by construction (ARCHITECTURE sec 6
//! invariant 4 — nothing fiction-shaped, valid for a novel / TRPG / spec
//! consumer alike).
//!
//! DRIFT GUARD — three tiers, honestly scoped (R592):
//! - COMPILE-guarded: the fixed vocabularies are built from the real core enums
//!   ([`DisclosureMode`] etc.) via `as_str()` + an exhaustive `match` for the
//!   per-value gloss, and the rule classes from the real
//!   [`crate::continuity::RuleClass`]. Adding an enum variant / rule class
//!   breaks the exhaustive match here (a compile error at the description
//!   site). The quest ids + `CURRENT_SCHEMA_VERSION` are single-sourced
//!   `pub(crate)`/`pub` constants.
//! - TEST-guarded: the fact field set is pinned to `FactImport`'s serde shape,
//!   the manifest WIRE FORMAT (every kind's JSON keys + the canon-order keys) is
//!   pinned to the real serde shapes by unit tests (Round 600), and the
//!   narrative-rules-FILE wire is pinned the same way (Round 605) — a
//!   renamed/added serde key fails the test until the wire prose names it. Every
//!   SERIALIZATION contract lives in this tier, not the hand-authored one.
//! - HAND-AUTHORED semantic prose (NOT auto-guarded): the registry and invariant
//!   *descriptions* are prose that PROJECTS the enforcement (the R576 "prose
//!   projects facts" posture) — a semantics change in a mutate primitive is not
//!   caught by a compiler here, so it must be reflected by hand. This is the one
//!   part that can drift; it is documentation of the enforcement, not a second
//!   source of it.

// Round 730 — `IntervalOp` LIFTED to core (shared by the interval rule + the
// DEBT-K parameter gate); imported from its canonical home, no longer from
// `crate::continuity`.
use mnemosyne_core::{DisclosureMode, IntervalOp, PayoffExpectation, PredicateObjectKind};
use serde::Serialize;

use crate::continuity::{
    ExclusiveKey, RuleClass, QUEST_PRED_COMPLETED_BY, QUEST_PRED_PURSUES, QUEST_PRED_REQUIRES,
};

/// The complete medium-neutral authoring contract (R587). Every field is a
/// static description of the substrate's shape, not any store's contents.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct SchemaContract {
    /// The store schema generation this contract describes
    /// ([`mnemosyne_atomic::CURRENT_SCHEMA_VERSION`]).
    pub schema_version: u32,
    /// One-paragraph framing of the fact model an agent is authoring into.
    pub overview: &'static str,
    /// The registries an id must be declared in before a fact may reference it
    /// (the fail-loud "register X first" contracts).
    pub registries: Vec<RegistrySpec>,
    /// The narrative-fact shape: required and optional fields.
    pub fact: FactSpec,
    /// The optional machine-readable subject–predicate–object leg of a fact.
    pub typed_claim: TypedClaimSpec,
    /// The fixed, substrate-defined vocabularies (the closed enums).
    pub vocabularies: Vec<Vocabulary>,
    /// The deterministic narrative-rule classes the continuity gate evaluates.
    pub narrative_rules: Vec<RuleClassSpec>,
    /// The quest authoring convention over the existing primitives (R559).
    pub quest_encoding: QuestEncoding,
    /// The write-time fail-loud invariants an author must satisfy (rejected at
    /// the mutate primitive, never a silent bad write).
    pub invariants: Vec<Invariant>,
    /// How the reference-integrity invariants are ALSO guarded out-of-band
    /// (Round 591) — so an agent knows a manual/out-of-band store edit that
    /// dangles a ref does not slip past. The AI-failure guardrails
    /// (hallucinated-ref, wrong-branch, orphan) are not a separate tool: they
    /// are these built-in invariants, re-checked by the continuity gate.
    pub invariant_enforcement: &'static str,
    /// The JSON WIRE FORMAT of the `import-facts` / `propose-verdict` batch
    /// manifest (Round 595, unattended-loop-experiment/v1 Finding 1). The field
    /// specs above give the SEMANTIC contract (names + types); this gives the
    /// SERIALIZATION an agent must emit — registry key names, the typed-object
    /// enum tagging (one wire tag per object_kind), the `first_at` tuple shape —
    /// plus a complete worked example. Without this an
    /// agent must reverse-engineer the serializer from parse errors.
    pub manifest_wire: ManifestWireSpec,
    /// The JSON WIRE FORMAT of the `import-sections` manifest (Round 906) — the
    /// STRUCTURE half of an authoring run, which this contract described nowhere
    /// until two blind authors (R904) independently guessed it and both guessed
    /// wrong in the same way. It is a BARE ARRAY, not an object, and its
    /// `parent_doc` is required and appeared zero times in this contract. Facts
    /// name sections in `canon_from` / `evidence`, so sections are authored
    /// FIRST; without this an author reverse-engineers them from parse errors.
    pub sections_wire: &'static str,
    /// How to author the KEYED SIDE TABLES (Round 909) — and, since Round 957,
    /// WHICH of them a manifest cannot reach is DERIVED from the manifest's own
    /// kind roster rather than asserted alongside it.
    ///
    /// A table outside that roster is reachable only through its own CLI verb;
    /// an author who guesses a manifest array for it gets `exit 0`, a receipt
    /// that never mentions their kind, and NOTHING BUILT (the deliberate
    /// unknown-key leniency, doing exactly what `manifest_wire.overview` warns
    /// it does).
    ///
    /// Round 957 — THIS FIELD SPENT A ROUND TEACHING THE MODEL THE WIRE HAD
    /// ALREADY KILLED. Round 956 wired `edge_costs` and `edge_guards` into the
    /// fact manifest, and this paragraph went on naming exactly those two as its
    /// examples of a silent no-op, closing with "nothing in a manifest will tell
    /// you so" — sending any author who came looking for them to the verbs. That
    /// is not a hypothetical cost: it is the misdirection that already produced
    /// two hand-written shell scripts from blind authors (R943) and left R936's
    /// five corpora at zero uses. Generating the claim from the roster is what
    /// stops a wire and its documentation from disagreeing again.
    pub side_table_wire: String,
    /// The canon ORDER a store needs to be RENDERABLE (Round 596,
    /// unattended-loop-experiment/v1 Finding 4) — a SEPARATE authoring artifact,
    /// NOT part of the fact manifest, that the read projections require. Without
    /// it `report-playthrough-manuscript` / `report-fork-tree` place nothing and
    /// the store is not playable; `report-authoring-frontier` surfaces every
    /// fact-bearing scene the order does not cover as an `unordered scenes` gap,
    /// and every section it does not position at all — empty ones included (Round
    /// 667) — as an `unplaced scenes` gap.
    pub canon_order: &'static str,
    /// How to encode a per-ROAD secret without leaking it (Round 601,
    /// unattended-loop-experiment/v2 gap B) — the `withhold` + `first_at` reveal
    /// idiom, and WHY a clean `report-authoring-frontier` does not certify a
    /// leak-free telling. A disclosure `mode` is world-INDEPENDENT (one decision
    /// per fact × telling); only `first_at` is per-world, so `state`/`hint`/`imply`
    /// discloses on every road — the trap two independent loop agents reached for.
    pub disclosure_encoding: &'static str,
    /// How to DECLARE a narrative rule so the continuity gate enforces it (Round
    /// 604, continuity-stress-experiment/v1 `surface_gap`) — the rule CLASSES
    /// above say what the gate CAN check; this gives the rules-FILE JSON wire, the
    /// `[continuity].rules_path` wiring, and the `interval_severity` opt-in. Without
    /// it a blind agent must reverse-engineer the rules file from parse errors +
    /// sweep candidate toml keys (a misspelled key is silently ignored; interval
    /// silently defaults to surface-only) — the three frictions the experiment hit.
    pub narrative_rules_wire: &'static str,
}

/// The JSON wire format of the batch manifest (Round 595) — the serialization,
/// not the semantics. Fully drift-guarded (Round 600): `example_json` parses
/// through the real [`mnemosyne_atomic::FactsManifest`] and its TRICKY shapes
/// are pinned by `manifest_example_parses_and_pins_wire_shape`; the `kinds`
/// key prose + `typed_object_wire` are pinned by
/// `manifest_wire_prose_names_every_serde_key`, which fails if the serializer
/// emits any key the prose does not name. So a serde rename cannot silently
/// leave this contract stale.
///
/// Round 906 — the ARRAY ROSTER is derived, not counted by hand. [`Self::kinds`]
/// is the single ordered list (apply order, which is semantic and not a serde
/// fact), `manifest_kinds_cover_every_facts_manifest_array` holds it set-equal to
/// [`mnemosyne_atomic::FactsManifest`]'s real serde keys, and [`Self::overview`]
/// is GENERATED from it — so the count and the roster cannot drift. They had:
/// `units` joined the manifest and this prose went on saying "seven ... frames,
/// branches, entity_kinds, entities, predicates, facts, disclosure_plans" for
/// thirteen rounds, which is how two blind authors both concluded a Quantity
/// could not be authored from a file (R904 gap 2).
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct ManifestWireSpec {
    /// The batch verbs this manifest is fed to.
    pub add_op: &'static str,
    /// The top-level object shape and the order kinds are applied in. GENERATED
    /// from [`Self::kinds`] (Round 906) — never hand-enumerated.
    pub overview: String,
    /// Per-kind serialized JSON key names (what the parser reads).
    pub kinds: Vec<KindWire>,
    /// The typed leg's object enum wire tagging — one wire tag per object_kind
    /// (Round 708 removed the `scalar`→`value` naming quirk with the value shape).
    pub typed_object_wire: &'static str,
    /// A complete, valid worked example: copy it and adapt. Parses through the
    /// real manifest parser (a test pins it, so it cannot silently drift).
    pub example_json: &'static str,
}

/// One kind's serialized JSON key names in the batch manifest (Round 595).
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct KindWire {
    /// The manifest array this describes (`frames` / `branches` / …).
    pub kind: &'static str,
    /// The serialized object's key names + shapes (the wire form, not prose).
    pub json_keys: &'static str,
}

/// One registry: an id space that must be populated before a fact references it.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct RegistrySpec {
    /// The `AtomicStore` field / registry name.
    pub name: &'static str,
    /// What the map is keyed by.
    pub key: &'static str,
    /// Which fact/claim field references a member (the fail-loud ref).
    pub referenced_by: &'static str,
    /// The mutate primitive that adds a member.
    pub add_op: &'static str,
    /// `true` when a typo silently escapes a rule (predicates) vs merely a bad
    /// ref (an entity kind); load-bearing ids get the strict registry contract.
    pub load_bearing: bool,
    /// Notes on the registry (e.g. a free-form kind field).
    pub description: &'static str,
}

/// One field of a struct in the contract.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct FieldSpec {
    pub name: &'static str,
    /// A description of the field's type (not a Rust path — an authoring hint).
    pub ty: &'static str,
    pub required: bool,
    pub description: &'static str,
}

/// The narrative-fact shape.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct FactSpec {
    pub description: &'static str,
    pub add_op: &'static str,
    pub fields: Vec<FieldSpec>,
}

/// The typed-claim (subject–predicate–object) contract.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct TypedClaimSpec {
    pub description: &'static str,
    pub subject: &'static str,
    pub predicate: &'static str,
    /// The object shapes (from [`PredicateObjectKind`]), each with its rule.
    pub object_shapes: Vec<EnumValue>,
}

/// One value of a fixed vocabulary (a closed enum variant).
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct EnumValue {
    pub value: &'static str,
    pub description: &'static str,
}

/// A fixed, substrate-defined vocabulary — a closed enum an author picks from.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct Vocabulary {
    pub name: &'static str,
    pub applies_to: &'static str,
    /// The default value serialized when omitted, if the enum has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<&'static str>,
    pub values: Vec<EnumValue>,
}

/// One narrative-rule class and its parameters.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct RuleClassSpec {
    pub class: &'static str,
    pub description: &'static str,
    pub parameters: Vec<FieldSpec>,
}

/// One typed predicate reserved by the quest convention.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct QuestPredicate {
    pub predicate: &'static str,
    pub role: &'static str,
    /// The expected object shape of this predicate's typed leg, as prose.
    pub object_shape: &'static str,
    /// Round 631 — the MACHINE-READABLE required object kind, the SSOT the
    /// validate-layer quest-shape guard reads. `None` = both kinds are allowed
    /// (`completed_by`, whose object is an entity actor OR a token discharger).
    ///
    /// Round 636 — this doc used to claim the prose `object_shape` and this
    /// field "cannot drift". NOTHING BOUND THEM, so it was the same unbacked
    /// drift-safety claim R629 had just been paid to delete three rounds
    /// earlier — a human reading the contract could be told "scalar" while the
    /// machine enforced entity, which is exactly how R620's consumer was misled.
    /// `quest_object_shape_prose_matches_the_enforced_kind` now binds them: the
    /// prose must NAME the kind this field enforces. The claim is true because a
    /// test makes it true, not because a comment says so.
    pub required_object_kind: Option<PredicateObjectKind>,
}

/// The quest authoring convention (R559) — a projection over existing
/// primitives, NOT new substrate: an author adopts these ids so the quest-graph
/// projection can read their store.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct QuestEncoding {
    pub description: &'static str,
    /// How the quest-graph projection IDENTIFIES a quest (R676): a quest is any
    /// entity in a quest predicate ROLE — the object of `pursues`, either
    /// endpoint of `requires`, or the subject of `completed_by`. There is NO
    /// `kind` marker; participation in the reserved relation is the sole signal.
    pub derivation: &'static str,
    pub predicates: Vec<QuestPredicate>,
    pub completion_rule: &'static str,
    pub state_derivation: &'static str,
}

/// One write-time fail-loud invariant.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct Invariant {
    pub name: &'static str,
    pub rule: &'static str,
    /// Where the check fires.
    pub enforced_at: &'static str,
}

/// Build the authoring contract (R587). Pure — no store, no order, no I/O.
pub fn describe_schema() -> SchemaContract {
    // Built before the literal so the side-table paragraph can be DERIVED from
    // the manifest roster (Round 957) rather than written out beside it.
    let registries = registries();
    let manifest_wire = manifest_wire();
    let side_table_wire = verb_only_wire(&registries, &manifest_wire);
    SchemaContract {
        schema_version: mnemosyne_atomic::CURRENT_SCHEMA_VERSION,
        overview: "A store is a set of multi-axis narrative FACTS (ARCHITECTURE sec 1.1): each \
            fact is one atomic CLAIM held within exactly one epistemic FRAME (who believes it) \
            on one world-line BRANCH (which quest-path/playthrough world), over a canon-time \
            extent evidenced by structure SECTIONS (the medium's discourse order — chapters / \
            scenes). Frames are sparse and non-privileged: the absence of a fact in a frame is \
            *unrecorded*, never *false* (ground-truth is one frame among many). Facts are \
            append-only for in-world change (a changed belief is a SUCCESSOR fact, not an edit). \
            An optional TYPED leg gives a fact a machine-readable subject-predicate-object \
            reading, authored in the same act as the prose (never NLP-derived) — the typed \
            subset is what the deterministic rule gate covers. Nothing fiction-shaped is \
            enforced: entity kinds, token vocabularies, and units are consumer vocabulary (sec 6 inv4).",
        registries,
        fact: fact_spec(),
        typed_claim: typed_claim_spec(),
        vocabularies: vocabularies(),
        narrative_rules: rule_class_specs(),
        quest_encoding: quest_encoding(),
        invariants: invariants(),
        invariant_enforcement:
            "The reference-integrity invariants (registered frame / branch / entity / predicate \
             refs, canon_from / evidence section refs, and supersedes_in_frame / pays_off / \
             conflict edge targets) are enforced BOTH at the mutate primitive (write-time) AND \
             re-checked out-of-band by the continuity gate (`validate-continuity`): a manual or \
             out-of-band store edit that dangles a ref fails the gate, and `FactCanonOffBranch` \
             flags a fact on the wrong world-line. So the deterministic AI-failure guardrails \
             (hallucinated-ref, wrong-branch, orphan) ARE these hard invariants — not a separate \
             optional check. `propose-verdict` runs the same gate over a candidate batch and \
             returns each as an actionable violation.",
        manifest_wire,
        sections_wire:
            "Creating the structure SECTIONS a fact's `canon_from` / `evidence` point at — the \
             FIRST authoring step, because a fact naming an unregistered section is rejected. \
             Two routes, and the registry's `add_op` names the SINGLE one: \
             `add-section --section <id> --parent-doc <doc-id> --title <text> [--parent <p>]` \
             creates one section per call, which is the right shape when structure arrives a \
             scene at a time. For a whole document use the bulk route instead — \
             `import-sections --manifest <file>`, and the file is a BARE JSON ARRAY, NOT an object \
             with a `\"sections\"` key: [ { \"section_id\": string (the store key), \"parent_doc\": \
             string (REQUIRED — the document this section belongs to; there is no default, and an \
             omitted one is a parse error, not an empty string), \"title\": string, \
             \"parent_section\"?: string (a section id declared EARLIER in this array — order \
             matters, a child follows its parent), \"coverage_expectation\"?: \
             \"normative\"|\"out_of_scope_here\"|\"informational\" (omitted = normative; \
             `out_of_scope_here` = in the mirrored document but not implemented by THIS consumer, \
             revisitable; `informational` = INHERENTLY non-implementable prose, terminology or \
             overview — the distinction is load-bearing, since marking an intended-but-unbuilt \
             feature `informational` is a false declaration no later binding can correct. The \
             pre-R422 tag `informative` is NOT accepted and NOT aliased — it fails loudly), \
             \"normative_excerpt\"?: { \"text\": string, \"anchor_url\": string, \
             \"source_revision\": string, \"text_sha256\"?: string (computed if omitted) } }, … ]. \
             Applied as ONE atomic transaction with a 3-way per-entry classification: absent → \
             create, byte-identical → no-op skip (so a re-run is idempotent), present-but-different \
             → the WHOLE manifest rejects (no silent overwrite — supersede and re-create to \
             revise). A manifest of only no-ops does not write, and reports `0 created`. NOTE the \
             asymmetry with the fact manifest: this one is a bare array of REQUIRED-field objects, \
             that one is an object of optional arrays.",
        side_table_wire,
        canon_order:
            "The canon ORDER — the discourse sequence of the sections — is a SEPARATE artifact \
             from the fact manifest, and a store needs it to be RENDERABLE: the read projections \
             (`report-playthrough-manuscript`, `report-fork-tree`, and any render / pinion \
             consumer) place a fact only at a section the order reaches. It is a JSON edge graph \
             { \"edges\": [[from-section, to-section], …], \"branches\": { branch-id: [[from, \
             to], …] } } — the main trunk in `edges`, each fork/branch's own edges under \
             `branches`, pinned via `[continuity].canon_order_path` (or passed with `--order`). \
             Authoring the facts is NOT enough: until the order covers every fact-bearing scene, \
             `report-authoring-frontier` reports those scenes as `unordered scenes` (with no \
             order declared, ALL of them), and the store cannot be rendered. THE ORDER IS ALSO \
             THE ONLY PLACE A SECTION IS PLACED, so registering one does not put it on a road: \
             the frontier's `unplaced scenes` is every section the order does not position \
             (fact-bearing or empty), and `validate-continuity` prints `order_nodes=<n>/<total> \
             sections` with a notice naming the shortfall when an order is declared. Being unplaced is NOT an error \
             — a section may be unplaced YET, the same forward-declared mode the canon-coordinate \
             checks tolerate — so it is reported and never gated.",
        disclosure_encoding:
            "WHEN THE READER LEARNS A FACT — the general axis, not only a secrecy device (Round \
             912). A fact's `canon_from` is when the claim becomes TRUE in the world; `evidence` is \
             a prior establishing scene; and the reader's discovery is a THIRD thing, pinned here \
             by `first_at`. So an ordinary, unsecret fact that holds from chapter one and is only \
             found out in chapter nine is authored `canon_from` at chapter one with a `first_at` at \
             chapter nine — NOT by citing chapter nine as evidence, which is a forward reference and \
             fails the gate (see the fact's `evidence` field). Two blind authors (R910) reached for \
             the evidence field because this surface described itself only as follows. \
             Encoding a per-ROAD secret, and the shape that does NOT encode one. A telling's \
             disclosure `mode` is world-INDEPENDENT (one decision per fact × telling), and so is \
             `surface`; only `first_at` is per-world. So a fact set to `state`/`hint`/`imply` is \
             disclosed on EVERY world-line — reaching for `state` to reveal a secret on one road \
             LEAKS it on the others. THE OBVIOUS REPAIR REVEALS IT ON NO ROAD, and this paragraph \
             taught it as the idiom for as long as the shape existed: `withhold` plus a per-world \
             `first_at` discloses nothing, anywhere. A withheld fact is dropped before it can seat \
             a locator, so no line renders at the pin on ANY world-line, and the premature-leak \
             gate reads any match of a withheld fact as a leak whatever the pin says. \
             `report-disclosure-coverage` names this exact shape an INERT reveal pin, and two \
             blind authors (Round 943) wrote it independently meaning `hidden until here, then \
             told`. TO TELL A FACT LATE on a road the reader walks, the mode must DISCLOSE: \
             `state` (or `hint`/`imply`) plus `surface.scene` at the scene they should learn it. \
             The SEAT is what moves a disclosure — `surface.scene` when authored, the fact's own \
             `canon_from` otherwise; `first_at` is a premature-leak CONSTRAINT on the re-extracted \
             prose (a first-reached trigger SET, Round 752) and moves no line. TO KEEP A FACT OFF \
             ANOTHER ROAD ENTIRELY, put the FACT on that branch: a branch-scoped fact is disclosed \
             only on the world-lines that contain it, and that is the per-road mechanism this \
             substrate has. IMPORTANT: a clean `report-authoring-frontier` does NOT certify a \
             correct telling — the frontier counts any fact carrying an override as `planned` and \
             never reads prose, so frontier-clean is necessary-but-NOT-sufficient. A premature \
             leak or early reveal is a RENDER property, caught only by the render-acceptance gates \
             (the disclosure leak gate + `report-playthrough-manuscript --telling`) over the \
             re-extracted prose; run those before trusting a telling — they are in scope, not an \
             afterthought.",
        narrative_rules_wire:
            "Declaring a narrative RULE so the continuity gate ENFORCES it — the rule CLASSES \
             (above) say what the gate CAN check; this is how to TURN A RULE ON. Rules live in a \
             SEPARATE file (like the canon order, NOT the fact manifest): a JSON object { \
             \"schema\"?: \"narrative-rules/v1\", \"comment\"?: string (a free-text annotation \
             slot), \"rules\": [ … ] } where each rule is { \"id\": \
             string (unique — it names the finding), \"predicate\": <predicate id> (the KEYED / \
             left typed leg, for every class), \"class\": \"exclusive\" | \"transition\" | \
             \"interval\", plus that class's legs: exclusive → \"per\": \"subject\" | \"object\" \
             + \"containment\"?: <predicate id> (Round 714: makes exclusivity REFINEMENT-AWARE \
             — two co-holding values one of which transitively `contains` the other refine one \
             location, not conflict; omit for literal-value exclusivity); \
             transition → \"adjacency\": <predicate id> (Round 697: its facts ARE the edges \
             — `adjacent(a,b)` admits (a,b); this is how movement between PLACES is gated, the \
             store-native map — the edges are FACTS, not a file list) + \"undirected\"?: bool \
             (Round 924 — this declares EDGE SYMMETRY and nothing else, so choose it by what you \
             need to say about the WAY: true = one fact admits both directions, and the two can \
             then never differ; absent/false = one fact is one way, which is what lets a way cost \
             more upward than downward. That a two-way road is then two facts is a PRICE, not a \
             reason — the fact count says nothing about the way. It does NOT declare \
             whether your rule is a map or a lifecycle, and no check reads it as though it did) \
             + \"containment\"?: <predicate id> (Round 716, \
             superseding R703's grouping model: its facts are `contains(container, contained)` and \
             they PARTITION the map into SCOPES — an adjacency edge may only join SIBLINGS (same \
             direct container), a non-sibling edge is `adjacency_cross_scope`, and a container \
             LEAVES its own scope by being a NODE in its parent's (a portal). Also turns on the \
             per-scope completeness/leak checks — EXCEPT `map_invented_place`, which additionally \
             needs the ADJACENCY predicate to declare a leg kind, since only that says what a \
             place IS (Round 934; declare neither leg and the class is unaskable and emits \
             nothing, which the gate names rather than passing over). Round 913/925, the STEP \
             side of the same \
             declaration, which the edge rule above does not imply: a succession between a \
             container and a place inside it is a crossing and needs NO EDGE AT ALL, and a CHAIN \
             of crossings is ONE move judged between its outer endpoints — so routing a forbidden \
             step through a container does not license it. Omit for a flat map). Any declared \
             containment \
             predicate \
             (a transition map's OR an exclusive rule's) is Round-715 integrity-checked to form a \
             TREE per (frame, world): at most one direct container per place, and acyclic. \
             interval → \
             \"right\": <predicate id>, \"op\": \"ge\"|\"le\"|\"eq\"|\"gt\"|\"lt\", \"bound\": { \
             \"const\": number } | { \"predicate\": <predicate id> } (a TAGGED object, never a \
             bare number). The relation is `value(left) − value(right) op bound`; the two operands \
             (and a predicate bound) must share ONE unit — a `Quantity{n,unit}` carries a \
             registered unit, a bare numeric token has none, and MISMATCHED units (day vs hour, or \
             typed vs bare) are SURFACED as `interval_unverifiable`, never subtracted as raw \
             numbers (Round 718); a `const` bound is read in the difference's own unit (it has no \
             unit slot). The parser is fail-loud on unknown or class-mismatched legs (a \
             transition carrying `per`, or a bare-number `bound`, rejects). WIRE the file via \
             `[continuity].rules_path = \"<file>\"` in mnemosyne.toml (+ an optional \
             `rules_sha256` pin, like the canon order); `--rules <file>` overrides it. Authoring \
             the file IS the opt-in — the gate is off until it is wired, and a MISSPELLED \
             `[continuity]` key is rejected (fail-loud) so a typo cannot silently leave the rules \
             unloaded. IMPORTANT: exclusive + transition violations gate at `[continuity].severity` \
             (default reject), but INTERVAL violations are SURFACE-ONLY by default (a timeline gap \
             can be a deliberate authored time-bend) — set `[continuity].interval_severity = \
             \"reject\"` to make an interval rule actually GATE, else it is reported but never \
             fails the gate. When interval rules are declared with the class OFF, \
             `validate-continuity` prints a NOTICE naming their count so the ungated state is \
             loud, not silent. And when ZERO rules are declared at all it prints a NOTICE saying \
             exactly that: a gate that evaluated NOTHING must never read the same as a gate that \
             PASSED.",
    }
}

/// The wire format of the batch manifest (Round 595). The `example_json` is the
/// SSOT an agent copies; the per-kind key notes name the exact serialized keys
/// (which differ from the semantic field names in a few load-bearing places —
/// `forks_from` is a bare string, the typed object is a tagged enum, `first_at`
/// is a list of per-world reveal triggers `{branch, coords, threshold?}`).
fn manifest_wire() -> ManifestWireSpec {
    let kinds = manifest_kinds();
    ManifestWireSpec {
        add_op: "import-facts (apply) / propose-verdict (dry-run gate) — both read this manifest",
        overview: manifest_overview(&kinds),
        kinds,
        typed_object_wire: TYPED_OBJECT_WIRE,
        example_json: MANIFEST_EXAMPLE_JSON,
    }
}

/// Round 906 — the manifest overview, GENERATED from the kind roster. The count
/// word and the ordered list were hand-written and drifted (see
/// [`ManifestWireSpec`]); here they are one expression over the single roster,
/// so an added kind updates both or neither.
fn manifest_overview(kinds: &[KindWire]) -> String {
    let names: Vec<&str> = kinds.iter().map(|k| k.kind).collect();
    format!(
        "A JSON object with {} optional arrays applied in this order in ONE atomic transaction: \
         {}. Later kinds may reference earlier ones (an entity names an entity_kind; a Quantity \
         object names a unit; a fact names a frame/branch/entity/section; a disclosure override \
         names a fact), so order matters — registries first, then facts, then disclosure. Any \
         array may be omitted (defaults to empty). UNKNOWN KEYS ARE IGNORED, DELIBERATELY: every \
         field is optional and the parser neither rejects nor reports a key it does not know, so a \
         MISSPELLED kind (or a key from a shape this manifest never had) parses cleanly and builds \
         NOTHING — `exit 0`, zero rows. That leniency is load-bearing (a reader can ask \"does this \
         file parse\" separately from \"does it build anything\"), which is exactly why the roster \
         above is authoritative: a correct guess and a typo are byte-identical at the parse, so the \
         only way to know a kind exists is that it is named here. Sections are NOT in this manifest \
         — see the sections wire; author them first, since facts name them.",
        names.len(),
        names.join(", ")
    )
}

/// The ordered roster of manifest arrays: APPLY order (semantic — later kinds
/// reference earlier ones), which is not recoverable from serde. Round 906 holds
/// this set-equal to the real `FactsManifest` serde keys, so the roster is
/// complete by test even though the ORDER is authored.
fn manifest_kinds() -> Vec<KindWire> {
    vec![
            KindWire {
                kind: "frames",
                json_keys: "{ \"frame_id\": string, \"description\"?: string }",
            },
            KindWire {
                kind: "branches",
                json_keys: "{ \"branch_id\": string, \"description\"?: string, \"forks_from\"?: \
                    string (a PARENT BRANCH id, e.g. \"main\" — a bare string, NOT an object), \
                    \"forks_at\"?: string (a section id), \"converges_from\"?: [ {\"branch\": \
                    string, \"at\": string}, … ] } — a branch is a fork (forks_from + forks_at) \
                    XOR a confluence (converges_from)",
            },
            KindWire {
                kind: "entity_kinds",
                json_keys: "{ \"kind_id\": string, \"parents\"?: [string, …] (registered \
                    entity_kind ids declared EARLIER in this array — R732 kind-inheritance tree, \
                    R738 a DAG / multiple inheritance; a rule scoped to ANY ancestor then accepts \
                    this subkind, so a `magic-sword` with parents [`weapon`,`magic-item`] \
                    satisfies both), \"description\"?: string } — the consumer's entity-kind \
                    vocabulary (character/place/item/quest/…); members are the consumer's, never \
                    core's",
            },
            KindWire {
                kind: "units",
                json_keys: "{ \"unit_id\": string, \"description\"?: string } — the unit registry \
                    (R706) a `quantity` typed object's \"unit\" must be a member of. A Quantity IS \
                    authorable from a manifest: declare the unit here, then write { \"kind\": \
                    \"quantity\", \"n\": …, \"unit\": <unit_id> } on the fact's typed leg. This \
                    array existed for thirteen rounds while the overview said seven; two blind \
                    authors read that roster and both reported a Quantity could not be file-\
                    authored (R904), which is why the roster is generated now",
            },
            KindWire {
                kind: "entities",
                json_keys: "{ \"entity_id\": string, \"kind\"?: string (a REGISTERED entity_kind \
                    id, not free text — declare it in entity_kinds first; omit = unspecified), \
                    \"description\"?: string }",
            },
            KindWire {
                kind: "predicates",
                json_keys: "{ \"predicate_id\": string, \"object_kind\": \
                    \"entity\"|\"token\"|\"quantity\"|\"fact\", \"object_tokens\"?: [string, …] \
                    (REQUIRED non-empty under object_kind=token — the closed vocabulary), \
                    \"subject_kind\"?: entity_kind, \"object_entity_kind\"?: entity_kind, \
                    \"description\"?: string }",
            },
            KindWire {
                kind: "facts",
                json_keys: "{ \"fact_id\": string, \"frame\": string, \"claim\": string, \
                    \"canon_from\": string (section id), \"evidence\": [section id, …] (>= 1), \
                    \"branch\"?: string (omit for main), \"canon_to\"?: string, \"entities\"?: \
                    [entity id, …], \"payoff_expectation\"?: \"expected\"|\"unmarked\", \
                    \"pays_off\"?: [fact id, …], \"supersedes_in_frame\"?: fact id, \
                    \"conflicts_with\"?: [fact id, …], \"typed\"?: TypedClaim (see \
                    typed_object_wire), \"quote\"?: string }",
            },
            KindWire {
                kind: "edge_costs",
                json_keys: "{ \"fact_id\": fact id (the `adjacent(a, b)` EDGE fact the cost \
                    attaches to), \"n\": positive integer (0 or negative is a free teleport and \
                    rejects), \"unit\": unit id (REGISTERED; add it to `units` first) } — Round \
                    956. Until this wire existed the cost side table was reachable ONLY by the \
                    `add-edge-cost` verb, so a file-only authoring could not touch it at all — \
                    and the hand-written `side-tables.sh` scripts under \
                    `phase1-map-corpus-experiment` are the tree's own record of that gap (R959). \
                    Mnemosyne stores the number \
                    and NEVER adds two \
                    costs together — units are consumer vocabulary, so summing them is the \
                    consumer's arithmetic (invariant 4).",
            },
            KindWire {
                kind: "edge_guards",
                json_keys: "{ \"fact_id\": fact id (the EDGE fact this guard gates), \
                    \"conditions\": [fact id, …] (>= 1; each must EXIST and none may be the edge \
                    itself — a guard is a SET, ANDed by default), \"threshold\"?: integer (K-of-N: \
                    omitted = AND over every condition, `1..=len`; `k == len` normalizes to AND, \
                    and 0 or `k > len` reject) } — Round 956, same verb-only history as \
                    `edge_costs`. THE MANIFEST IS THE DECLARATION, NOT A PATCH: re-importing an \
                    entry with `threshold` dropped returns that guard to AND rather than keeping \
                    the old k. Mnemosyne NEVER evaluates whether a guard holds — the declaration \
                    is checked, the evaluation is the consumer's (the Round 712 layering line).",
            },
            KindWire {
                kind: "disclosure_plans",
                json_keys: "{ \"telling_id\": string, \"default_mode\"?: \
                    \"withhold\"|\"state\"|\"hint\"|\"imply\" (omitted = withhold), \
                    \"description\"?: string, \"overrides\"?: [ { \"fact_id\": string, \"mode\": \
                    string, \"first_at\"?: [ { \"branch\": branch id, \"coords\": [section id, …] \
                    (>= 1; the first-reached trigger SET — a non-linear reader reveals the fact at \
                    the EARLIEST coord reached), \"threshold\"?: integer (K-of-N; omitted = \
                    first-reached, 2..=len selects the k-th-earliest, len = last-reached) }, … ] (a \
                    list of per-world reveal triggers), \"surface\"?: { \"scene\": section id, \
                    \"object\"?: entity id } (Round 955 — WHERE the reader meets the fact, and the \
                    only slot that moves it. The seat is DERIVED (R643): omit `surface` and it is \
                    the fact's own `canon_from`; author one and it OVERRIDES that, and the \
                    `map_locator` a runtime dereferences resolves your `scene` against the world's \
                    walk. `first_at` is the other axis and moves nothing — it pins WHEN a withheld \
                    fact is revealed, which is the confusion Round 947 measured on authored data. \
                    BOTH LEGS ARE REGISTRY REFS, not free text: `scene` must be a registered \
                    section and `object` a registered entity, and either unregistered is a write \
                    REJECT. Seating a fact EARLIER than its own `canon_from` puts a truth on the \
                    page before it is true — legal to write, and counted by \
                    `report-authoring-frontier` as `disclosures_seated_before_truth`.) } ] }",
            },
    ]
}

/// The typed leg's object enum wire tagging (Round 595, Round 708).
const TYPED_OBJECT_WIRE: &str =
    "A fact's optional `typed` leg is { \"subject\": entity id, \"predicate\": predicate \
             id, \"object\": <tagged enum> }. The object is an INTERNALLY-TAGGED enum with four \
             registered variants matching the predicate's object_kind (Round 708 removed the \
             free-text `scalar`/`value` shape — every machine-slot object is now enumerable; free \
             text lives only in the prose `claim`): for `entity`, write { \"kind\": \"entity\", \
             \"id\": entity id }; for `token` (R705), write { \"kind\": \"token\", \"token\": \
             string } where the token MUST be a member of the predicate's declared object_tokens \
             (a token outside the closed set rejects); for `quantity` (R706), write { \"kind\": \
             \"quantity\", \"n\": integer, \"unit\": unit id } where `unit` MUST be a registered \
             unit (add-unit first; an unregistered unit rejects); for `fact` (R707), write \
             { \"kind\": \"fact\", \"id\": fact id } referencing another fact of this store \
             (existence checked in phase 2 against store + same-manifest staged; self-reference \
             rejects; the fact cannot be retracted while referenced). The subject and any \
             entity-shaped object must ALSO appear in the fact's `entities` list.";

/// A complete, valid `import-facts` manifest — the copy-and-adapt template
/// (Round 595). Exercises every kind and the load-bearing serialization quirks:
/// a fork branch (`forks_from` string), a token typed object (`kind`:`token`),
/// an entity typed object (`kind`:`entity`), a setup/payoff pair, and a
/// disclosure override with a `first_at` `[branch, section]` pin. Section ids
/// are illustrative — serde does not check them (the store validator does). A
/// unit test parses this through the real [`mnemosyne_atomic::FactsManifest`]
/// and pins its contents, so a wire-format change breaks the build here.
const MANIFEST_EXAMPLE_JSON: &str = r#"{
  "frames": [
    { "frame_id": "ground-truth" },
    { "frame_id": "scout", "description": "the scout's belief" }
  ],
  "branches": [
    { "branch_id": "road-b", "forks_from": "main", "forks_at": "sc-03" }
  ],
  "entities": [
    { "entity_id": "e-scout", "kind": "character" },
    { "entity_id": "e-relic", "kind": "item" }
  ],
  "predicates": [
    { "predicate_id": "held_by", "object_kind": "entity", "description": "custody" },
    { "predicate_id": "state", "object_kind": "token", "object_tokens": ["hidden", "taken"], "description": "an item's state" }
  ],
  "facts": [
    {
      "fact_id": "f-setup", "frame": "ground-truth",
      "claim": "the relic lies in the vault", "canon_from": "sc-01",
      "evidence": ["sc-01"], "entities": ["e-relic"],
      "payoff_expectation": "expected",
      "typed": { "subject": "e-relic", "predicate": "state",
                 "object": { "kind": "token", "token": "hidden" } }
    },
    {
      "fact_id": "f-payoff", "frame": "ground-truth", "branch": "road-b",
      "claim": "the scout takes the relic", "canon_from": "sc-04",
      "evidence": ["sc-04"], "entities": ["e-scout", "e-relic"],
      "pays_off": ["f-setup"],
      "typed": { "subject": "e-relic", "predicate": "held_by",
                 "object": { "kind": "entity", "id": "e-scout" } }
    }
  ],
  "disclosure_plans": [
    {
      "telling_id": "default", "default_mode": "withhold",
      "description": "the reader reconstructs by default",
      "overrides": [
        { "fact_id": "f-setup", "mode": "state", "first_at": [ { "branch": "road-b", "coords": ["sc-04"] } ] }
      ]
    }
  ]
}"#;

/// The keyed side tables a fact manifest CANNOT reach (Round 957) — derived by
/// differencing the registry roster against the manifest's own kinds, never
/// hand-listed beside it.
///
/// `sections` is excluded deliberately: it is outside the fact manifest too, but
/// it has its own file wire ([`SchemaContract::sections_wire`]), so calling it
/// verb-only would be its own false claim.
///
/// Deriving this is the entire point of the function. Round 956 wired two of
/// these tables and the prose next to them went on calling them verb-only,
/// which is the shape that sends a blind author to a hand-written shell script.
fn verb_only_registries<'a>(
    registries: &'a [RegistrySpec],
    manifest: &ManifestWireSpec,
) -> Vec<&'a str> {
    let wired: std::collections::BTreeSet<&str> = manifest.kinds.iter().map(|k| k.kind).collect();
    registries
        .iter()
        .map(|r| r.name)
        .filter(|n| *n != "sections" && !wired.contains(n))
        .collect()
}

/// The side-table authoring paragraph (Round 909 content, Round 957 derivation).
///
/// The opening claim names [`verb_only_registries`] and nothing else, so a table
/// that gains a manifest wire leaves the claim in the same change that wires it.
/// The verb reference block below stays hand-written: those argument strings are
/// CLI-parser knowledge, and inventing them for verbs this round did not read
/// would be the same class of false documentation this round exists to remove.
fn verb_only_wire(registries: &[RegistrySpec], manifest: &ManifestWireSpec) -> String {
    let named = verb_only_registries(registries, manifest)
        .iter()
        .map(|n| format!("`{n}`"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "Reached ONLY through their own verb, keyed by an existing fact: {named}. Writing one of \
         THOSE as a manifest array is a SILENT NO-OP: the manifest parses, `exit 0`, and the \
         receipt simply never mentions that kind, because unknown manifest keys are ignored by \
         design (see the manifest overview). That list is DERIVED from the manifest's own kind \
         roster (Round 957), so wiring one of these tables retires its place here in the same \
         change, instead of leaving this paragraph to contradict the wire. \
         `edge_costs` AND `edge_guards` ARE NO LONGER AMONG THEM (Round 956): both are fact-manifest \
         arrays now — their rows, with the shapes, are in the roster above — and both keep their \
         verbs, so a file and a verb are two doors onto one enforcement. This paragraph asserted \
         the opposite for as long as the wire was missing, and that was not a harmless silence — \
         a table reachable only by verb is a table a file-only authoring cannot reach, whatever \
         this document says about it. \
         The verbs, with their arguments: \
         `add-edge-cost --fact <adjacent-fact-id> --n <positive-int> --unit <registered-unit>` \
         (travel time on one edge; the unit must be registered first, and n must be POSITIVE \
         — 0 is a free teleport). \
         `add-edge-guard --fact <adjacent-fact-id> --condition <condition-fact-id>` (call it \
         once per condition; the set is ANDed) plus \
         `set-edge-guard-threshold --fact <edge-fact-id> (--threshold <k> | --clear)` for \
         K-of-N. \
         `add-parameter --parameter <id>`, then \
         `add-parameter-delta --fact <beat-fact-id> --parameter <registered-id> --delta \
         <nonzero-int>` and \
         `add-parameter-gate --fact <choice-fact-id> --parameter <registered-id> --op \
         <ge|le|eq|gt|lt> --threshold <int>`. \
         `add-fact-count --fact <fact-id> --count <positive-int>`. \
         Each verb takes `[--sidecar <path>] [--json]` like every other mutate. So an authoring \
         run that must express \"this journey takes longer\" or \"this way is shut until \
         something is true\" now says BOTH in the fact manifest — the tables named at the top of \
         this paragraph are what still needs the second artifact."
    )
}

fn registries() -> Vec<RegistrySpec> {
    vec![
        RegistrySpec {
            name: "sections",
            key: "section id",
            referenced_by:
                "NarrativeFact.canon_from / canon_to / evidence[] (the canon coordinate space)",
            add_op: "add-section",
            load_bearing: true,
            description: "The structure / discourse-order space (chapters, scenes). Canon \
                coordinates ARE section ids — a fact's canon_from and every evidence ref must \
                name an existing section, so scenes are authored before the facts set in them. \
                Shared with the spec side (a section is the medium-neutral structural unit).",
        },
        RegistrySpec {
            name: "frames",
            key: "frame id",
            referenced_by: "NarrativeFact.frame (exactly one, mandatory)",
            add_op: "add-frame",
            load_bearing: true,
            description: "Epistemic frames — whose belief a fact records. `ground-truth` is a \
                non-privileged entry registered like any other; a believed-fact and its \
                ground-truth counterpart are DISTINCT facts, never one fact with two frames.",
        },
        RegistrySpec {
            name: "branches",
            key: "branch id",
            referenced_by: "NarrativeFact.branch (optional; defaults to `main`)",
            add_op: "add-branch",
            load_bearing: true,
            description: "World-line branches — divergent quest-path/playthrough worlds. `main` \
                is the default axis, known by construction and never registered. A branch is \
                EITHER a fork (forks_from a parent at a canon point) XOR a confluence \
                (converges_from >= 2 parents at their merge points). \
                \
                TWO AXES, AND THEY ARE DUALS (Rounds 612 + 614) — a world-line has FACTS (what \
                is true in it) and a ROAD (which scenes it travels). At a FORK: facts are CUT at \
                the departure point, and the road is OVERRIDDEN (a branch that declares its own \
                next scene at a shared coordinate replaces the inherited one). At a MERGE: facts \
                INTERSECT (a confluence continues only what EVERY incoming road carried, so the \
                path-independent trunk survives and each parent's EXCLUSIVE middle is dropped), \
                while roads UNION (a coordinate is travelled if EITHER incoming road travels it). \
                Consequences you must author for: a fork off a CONFLUENCE still inherits the \
                whole pre-merge trunk; and AUTHORING A MERGE RELOCATES TRUNK OWNERSHIP — the \
                scenes after the merge now belong to the confluence branch, not to `main`, so a \
                later divergent line forking off `main` inherits them THROUGH the confluence \
                (MNEMO-GAP-003). \
                \
                THE ROAD IS DECLARED IN THE CANON ORDER, NOT HERE. The order file's top-level \
                `edges` ARE `main`'s road; each entry under `branches` is THAT world-line's own \
                road segment. A branch that declares NO segment RIDES ITS LINEAGE'S ROAD ON — so \
                its ENDING is the trunk's ending. That is correct for a world-line that diverges \
                only in FACTS, and WRONG for a DIVERGENT ENDING: until such a branch declares its \
                road (`\"branches\": {\"ending\": [[\"<fork-point>\", \"<its own scene>\"]]}`), \
                `validate-render-fidelity` cannot tell its ending from the trunk's and \
                `validate-continuity` names it under `undeclared_roads`. A branch's segment must \
                ATTACH to the road it rides in on (start it AT or before where it leaves the \
                parent's road) — an edge whose source the branch never reaches can never be \
                travelled and is REJECTED at load. A merge edge may be declared from either side \
                (on the parent, or on the confluence, one per parent); a confluence's merge edge \
                from a sibling never puts that sibling's exclusive scene on YOUR road, because a \
                scene is only travelled if the world can actually GET there. \
                \
                A fact's `canon_from` must be ON its branch's road (else `FactCanonOffBranch`), \
                and so must every scene it cites in `evidence` — 'could this world have SEEN that \
                scene, by now?' is a ROAD question (Round 615), so citing a sibling's exclusive \
                scene is rejected even though the shared order can reach it. \
                \
                FORK-LINEAGE TRAP (Round 601, the dangling two independent loop agents hit): a \
                fork inherits the parent's prefix, so a pre-fork trunk setup is `in` every fork's \
                world-line — but the BARE parent (no fork continuing it) stays its OWN \
                world-line, a DEAD PREFIX that still carries those trunk setups. Forking BOTH \
                roads off `main` and never continuing bare `main` leaves `main` a dead prefix \
                whose trunk `expected` setups have no payoff THERE and dangle (surfaced per-world \
                by `report-payoff-coverage` / `report-authoring-frontier`). Continue `main` AS \
                one of the roads (fork only the OTHER off it), or pay the trunk setups off before \
                the fork — do not leave a bare pre-fork trunk carrying live setups.",
        },
        RegistrySpec {
            name: "entities",
            key: "entity id",
            referenced_by: "NarrativeFact.entities[] + TypedClaim.subject / entity-shaped object",
            add_op: "add-entity",
            load_bearing: false,
            description: "The retrieval key for entity-scoped verification (all facts about X — \
                a character, location, item, faction). `Entity.kind` is a consumer-defined tag \
                (a registered entity-kind ref, sec 6 inv4); there is NO reserved kind value — \
                quests are DERIVED from quest predicate roles, not a `kind` marker (R676, see \
                quest_encoding).",
        },
        RegistrySpec {
            name: "predicates",
            key: "predicate id",
            referenced_by: "TypedClaim.predicate",
            add_op: "add-predicate",
            load_bearing: true,
            description: "Typed-claim predicates. LOAD-BEARING: narrative rules key off a \
                predicate id, so a typo would silently escape its rule — hence a strict \
                registry (unlike the free-form entity kind). Each predicate declares its object \
                shape (entity | token | quantity | fact — R708 removed free-text scalar), enforced \
                on every typed leg; a `token` predicate also declares a closed `object_tokens` \
                vocabulary the object must be a member of, a `quantity` object's unit must be a \
                registered unit, and a `fact` object references another fact (phase-2 existence + \
                delete-guard).",
        },
        RegistrySpec {
            name: "units",
            key: "unit id",
            referenced_by: "TypedObject::Quantity.unit",
            add_op: "add-unit",
            load_bearing: false,
            description: "Units of measure for the `quantity` object shape (R706) — `day`, \
                `minute`, `metre`. Consumer vocabulary (invariant 4: core never enumerates \
                them, the R700 place-kind lesson one axis over); the substrate enforces only \
                THAT a Quantity's unit is registered, fail-loud — a bare unit string would \
                drift `min`/`minute`/`분`. Declared via add-unit before a Quantity uses it.",
        },
        RegistrySpec {
            name: "edge_costs",
            key: "adjacent fact id",
            referenced_by: "keyed BY the adjacent fact; handed back by report-transition-map \
                (R875) on its edge — the DERIVED travel-time computation (tide_budget − Σcost) \
                this once pointed at as \"future\" is the CONSUMER's, never ours: it needs a \
                domain number core must not know (R711), which is why the plain carriage read \
                is the one that exists",
            add_op: "add-edge-cost",
            load_bearing: false,
            description: "Map EDGE COSTS (R709 → DEBT-J) — keyed by the adjacent(a,b) fact id, \
                value = a number + registered unit (the Quantity shape). A SIDE-TABLE, not a \
                reified fact: the cost is frame-invariant edge metadata (owner-invented map \
                minutes, no evidence), so it needs no per-fact frame/branch. Fail-loud: the fact \
                must exist, the cost must be POSITIVE (G3 — 0 is a free teleport), the unit \
                registered. retract-fact cascade-drops the cost, so it never dangles.",
        },
        RegistrySpec {
            name: "edge_guards",
            key: "adjacent (edge) fact id",
            referenced_by: "keyed BY the adjacent (edge) fact; VALUE = a condition SET plus an \
                optional K-of-N threshold — handed back by report-transition-map (R875) on its \
                edge, and evaluated by the consumer (pinion runtime) which ANDs the conditions \
                (or counts >= the threshold), never by Mnemosyne",
            add_op: "add-edge-guard",
            load_bearing: false,
            description: "Map EDGE GUARDS (R717/721 design → R720/722, K-of-N threshold R723) — a \
                PLACE-ACCESS condition on an adjacency edge, keyed by the edge fact id, value = an \
                EdgeGuard: the SET of CONDITION fact ids the edge REQUIRES (\"this passage requires \
                the key AND low tide\") plus an optional K-of-N `threshold`. threshold None = \
                require ALL (AND, the default + canonical); Some(k) = at least k of them (1<=k<len, \
                set via set-edge-guard-threshold, k==len normalizes to None). The consumer \
                evaluates each condition and ANDs them (or counts >= k); OR is authored as MULTIPLE \
                guarded edges to the same target (never a stored boolean expression tree — the \
                layering line; negation stays named-deferred). A SIDE-TABLE like edge_costs: the \
                LINK is frame-invariant edge metadata, each CONDITION is a real fact. Mnemosyne \
                holds the DECLARATION and integrity-checks ONLY that the edge and EVERY condition \
                resolve (a per-member dangling-ref check) + that 1<=k<=len — it NEVER evaluates \
                whether the guard holds now (the consumer's playthrough job). So the AUTHOR puts \
                the branch outcomes in the store (the got-condition world-line and the without \
                one, as forked branches); the game only evaluates the booleans and follows the \
                branch. add-edge-guard adds one condition (call N times); set-edge-guard-threshold \
                sets/clears k; remove-edge-guard-condition drops one (the key is deleted when the \
                set empties, and it REFUSES a drop below k); remove-edge-guard drops the whole set. \
                retract-fact cascade-drops the set with its edge and REFUSES to retract a \
                referenced condition; validate-continuity flags a guard on a non-edge \
                (edge_guard_not_an_edge).",
        },
        RegistrySpec {
            name: "parameters",
            key: "parameter id",
            referenced_by: "parameter_deltas reference a registered parameter (parameter_gates \
                joins them in R730)",
            add_op: "add-parameter",
            load_bearing: false,
            description: "Numeric PARAMETER registry (R728 design → R729 build, DEBT-K) — the \
                consumer's accumulating meters (`affection`, `karma`, `gold`, an RPG stat). \
                Consumer vocabulary (invariant 4: core never enumerates them, the R700/R706 \
                lesson one axis over); the substrate enforces only THAT a parameter in use is \
                registered, fail-loud — a bare parameter string would drift \
                `affection`/`affinity`/`호감도`. Declared via add-parameter before a delta or \
                gate names it. Like `units`, EMPTY does not pass.",
        },
        RegistrySpec {
            name: "parameter_deltas",
            key: "beat fact id",
            referenced_by: "keyed BY the beat fact; VALUE = a map from parameter id to a SIGNED \
                delta — read by the consumer (a VN/RPG runtime), which accumulates the running \
                sum along a playthrough, never by Mnemosyne",
            add_op: "add-parameter-delta",
            load_bearing: false,
            description: "Per-beat SIGNED parameter DELTAS (R728 design → R729 build, DEBT-K) — \
                keyed by the fact id of the beat that grants the change, value = parameter id -> \
                signed delta (`+2` a gift, `-1` an insult; one beat may move several meters). A \
                SIDE-TABLE like edge_costs, not a reified fact: the delta is frame-invariant \
                game-mechanic ground truth (which branch it applies on is captured by the \
                branch-scoped fact key; the VALUE is invariant). Fail-loud: the fact must exist, \
                the parameter be registered, and the delta be NON-ZERO — re-checked at the scan \
                boundary too (the parity-complete edge_guard precedent, not the n>0-blind \
                edge_cost one). Signed deltas are the weighted/negative axis K-of-N cannot \
                express. retract-fact cascade-drops the deltas, so none dangles. Mnemosyne holds \
                the authored delta; it NEVER computes a running sum (the consumer's playthrough \
                job — the layering line).",
        },
        RegistrySpec {
            name: "parameter_gates",
            key: "choice (edge) fact id",
            referenced_by: "keyed BY the choice fact; VALUE = {parameter, op, threshold} — read by \
                the consumer (a VN/RPG runtime), which accumulates the meter along a playthrough \
                and compares it to the threshold, never by Mnemosyne",
            add_op: "add-parameter-gate",
            load_bearing: false,
            description: "Per-CHOICE numeric-threshold GATES (R728 design → R730 build, DEBT-K) — \
                keyed by the choice edge fact id, value = a ParameterGate {parameter (a registered \
                meter), op (ge|le|eq|gt|lt), threshold (signed int)}. The axis K-of-N (edge_guards \
                threshold) cannot express: a signed/weighted meter compared to a numeric threshold \
                (\"romance route unlocks if affection >= 4\"). Because the gate references the meter \
                DIRECTLY (via the parameters registry), the boolean-proxy silent hole is \
                UNREPRESENTABLE — there is no disconnected \"sufficient\" fact to leave stale when \
                the meter drops. Rides ANY real fact — NO map-edge check (unlike edge_guards' \
                edge_guard_not_an_edge): a meter-gated route unlock is a NARRATIVE branch choice, \
                not necessarily a spatial place-to-place move. Fail-loud: the fact must exist, the \
                parameter be registered — re-checked at the scan boundary too (the parity-complete \
                edge_guard precedent). The threshold has NO bound (0/negative legal); satisfiability \
                is the consumer's model, never Mnemosyne's (NO reachability verdict). retract-fact \
                cascade-drops the gate. Mnemosyne holds the DECLARATION; it NEVER accumulates the \
                meter or evaluates whether the gate holds now (the consumer's playthrough job — the \
                layering line).",
        },
        RegistrySpec {
            name: "fact_counts",
            key: "fact id",
            referenced_by: "keyed BY the fact; VALUE = a positive int count — read by the consumer \
                (a VN/RPG runtime) as the multiset size, never summed or evaluated by Mnemosyne",
            add_op: "add-fact-count",
            load_bearing: false,
            description: "Per-fact multiset COUNTS (R731 build, DEBT-L) — keyed by a fact id, value \
                = a POSITIVE count (the multiset multiplicity: holds(A, potion) count 5 = A holds \
                FIVE potions). The DISTINCT part of multiset/quantity custody (currency 100 gold is \
                a DEBT-K global meter): a count bound to a SPECIFIC custody fact, which singular \
                holds cannot express and a global meter cannot express per-holder. A SIDE-TABLE like \
                edge_costs, not a reified fact: the count is frame-invariant metadata; a bare int \
                (no unit — the thing counted is the fact's OBJECT leg). Because it is keyed BY the \
                fact, retract-fact CASCADE-DROPS it — the orphaned-count silent hole (a stray count \
                fact surviving its custody retract) is UNREPRESENTABLE. Rides ANY fact — NO \
                custody-predicate check: anchoring to the per:object Exclusive rule is semantically \
                INVERTED (a multiset count is meaningful for FUNGIBLE items, which are exactly the \
                ones NOT under exclusivity; a unique token has count 1). Fail-loud: the fact must \
                exist, the count be POSITIVE — re-checked at the scan boundary too (the \
                parity-complete edge_guard precedent). Mnemosyne holds the count; it NEVER evaluates \
                the multiset (the consumer's job — the layering line; singular-custody stays the \
                per:object Exclusive rule's, unchanged).",
        },
        RegistrySpec {
            name: "disclosure_plans",
            key: "telling id",
            referenced_by: "the `--telling` carrier + the render-acceptance gates",
            add_op: "add-disclosure-plan",
            load_bearing: false,
            description: "Named TELLINGS over one fact base (the North-Star 'one substrate, \
                many tellings'): a default disclosure mode + sparse per-fact overrides selecting \
                what the reader learns, when (per world-line), in what mode. A render property, \
                NOT a store-integrity invariant — checked by the render-acceptance gates over \
                re-extracted prose, never by validate-workspace.",
        },
    ]
}

fn fact_spec() -> FactSpec {
    FactSpec {
        description: "One multi-axis narrative fact — an atomic, falsifiable claim (one \
            assertion, not an entity dossier). Append-only by genre for in-world change.",
        add_op: "add-fact (or import-facts for a batch; both route through one validator)",
        fields: vec![
            FieldSpec {
                name: "frame",
                ty: "frame id",
                required: true,
                description: "The epistemic frame this claim is held in (exactly one).",
            },
            FieldSpec {
                name: "claim",
                ty: "string",
                required: true,
                description: "The claim held in this frame — atomic, one assertion. Primary and \
                    always required (the typed leg is an optional machine reading of it).",
            },
            FieldSpec {
                name: "canon_from",
                ty: "section id",
                required: true,
                description: "The canon coordinate (structure-section) where this claim starts \
                    holding — the medium's discourse order.",
            },
            FieldSpec {
                name: "evidence",
                ty: "section id[] (>= 1), each AT OR BEFORE `canon_from`",
                required: true,
                description: "Structure sections evidencing the claim. At least one — a claim \
                    without provenance is unauditable. ROUND 912: evidence is a BACKREFERENCE, \
                    not a place where the claim is shown. Every ref must be reachable AND PRIOR in \
                    this fact's own world-line — at or before its `canon_from` — and a forward \
                    reference is `evidence_unreachable` at the continuity gate (R522). \
                    `canon_to` DOES NOT WIDEN THIS: a fact holding sc-01..sc-05 still may not cite \
                    sc-03, because the comparison is against `canon_from` alone. IF WHAT YOU MEAN \
                    IS \"true from here, but the reader only finds out later\", THIS IS THE WRONG \
                    FIELD — leave `canon_from` where the claim becomes true, cite a prior \
                    establishing scene here, and pin the reader's discovery with a disclosure \
                    `first_at` (see disclosure_encoding; it is the general when-does-the-reader-\
                    learn-this axis, not only a secrecy device). Two blind authors (R910) reached \
                    for a forward `evidence` ref instead and produced 43 findings between them.",
            },
            FieldSpec {
                name: "branch",
                ty: "branch id",
                required: false,
                description: "The world-line (defaults to `main`). Conflict scoping and \
                    in-frame succession are both (frame, branch)-scoped.",
            },
            FieldSpec {
                name: "entities",
                ty: "entity id[]",
                required: false,
                description: "The entities this claim is about — the retrieval key. A typed \
                    leg's subject/entity-object must also appear here.",
            },
            FieldSpec {
                name: "canon_to",
                ty: "section id",
                required: false,
                description: "Explicit canon end for a belief that ends WITHOUT an in-frame \
                    successor; omit when a successor exists (the end derives from it).",
            },
            FieldSpec {
                name: "payoff_expectation",
                ty: "payoff_expectation enum",
                required: false,
                description: "`expected` marks the fact a setup (Chekhov's gun) whose payoff \
                    coverage the report classifies per world; default `unmarked`.",
            },
            FieldSpec {
                name: "pays_off",
                ty: "fact id[]",
                required: false,
                description: "Setup fact ids this fact pays off (the backward pointer; the \
                    setup is written first and never touched when paid). Targets must exist.",
            },
            FieldSpec {
                name: "supersedes_in_frame",
                ty: "fact id",
                required: false,
                description: "The in-frame predecessor this claim replaces — the mechanism for \
                    time-indexed belief change (same frame enforced).",
            },
            FieldSpec {
                name: "conflicts_with",
                ty: "fact id[] (recorded judgments)",
                required: false,
                description: "Recorded contradiction edges (never derived from claim text). \
                    Each pins the target claim's hash at judgment time (computed by the \
                    primitive) so a later amend surfaces the judgment as stale.",
            },
            FieldSpec {
                name: "typed",
                ty: "TypedClaim (subject, predicate, object)",
                required: false,
                description: "The optional machine-readable reading of the claim (see \
                    typed_claim). Absence means prose-only — partial coverage is the design.",
            },
            FieldSpec {
                name: "quote",
                ty: "string",
                required: false,
                description: "Optional verbatim medium quote backing the claim; its sha256 is \
                    computed by the primitive (content-drift detectable offline).",
            },
        ],
    }
}

fn typed_claim_spec() -> TypedClaimSpec {
    TypedClaimSpec {
        description: "The optional machine-readable leg: binary subject-predicate-object, \
            authored WITH the prose (never NLP-derived). The typed subset is what the \
            deterministic rule gate covers; the prose claim stays primary.",
        subject: "a registered entity id that MUST also be a member of the fact's entities list \
            (a typed leg never silently widens the retrieval key).",
        predicate: "a registered predicate id — its declared object_kind fixes the object shape.",
        object_shapes: predicate_object_kind_values(),
    }
}

/// The fixed vocabularies, each built from the real core enum (drift-guarded by
/// the exhaustive `match` in its `*_values` helper).
/// Round 629 — WHICH enums the contract publishes is an editorial choice, so
/// this list is hand-picked and is the ONE residual the variant oracle cannot
/// close: there is no way to enumerate "every enum in the crate", and not every
/// enum belongs in an authoring contract. What the oracle guarantees is that a
/// vocabulary listed here can never be SHORT. A vocabulary that is missing
/// ENTIRELY is still possible — that is a judgment, and R629 paid two of them
/// (`interval_op` / `exclusive_key`, 7 variants that existed only as hand-typed
/// strings inside a prose blob). Named here rather than left silent, because a
/// list that looks complete is what taught a real consumer that seven present
/// capabilities were absent (R620).
fn vocabularies() -> Vec<Vocabulary> {
    vec![
        Vocabulary {
            name: "disclosure_mode",
            applies_to: "DisclosurePlan.default_mode + DisclosureOverride.mode",
            default: Some(DisclosureMode::default().as_str()),
            values: disclosure_mode_values(),
        },
        Vocabulary {
            name: "payoff_expectation",
            applies_to: "NarrativeFact.payoff_expectation",
            default: Some(PayoffExpectation::default().as_str()),
            values: payoff_expectation_values(),
        },
        Vocabulary {
            name: "predicate_object_kind",
            applies_to: "Predicate.object_kind (fixes a predicate's typed-object shape)",
            default: None,
            values: predicate_object_kind_values(),
        },
        // Round 629 — these two existed ONLY as hand-typed strings inside the
        // narrative-rules prose ("op": "ge"|"le"|…, "per": "subject"|"object").
        // 7 variants the authority never published as vocabulary, in the class
        // R620 convicted: an author reading the contract could not enumerate
        // them, and nothing tied the prose to the enums.
        Vocabulary {
            name: "interval_op",
            applies_to: "the `op` leg of an interval narrative rule \
                (value(left) − value(right) ⋈op⋈ bound)",
            default: None,
            values: interval_op_values(),
        },
        Vocabulary {
            name: "exclusive_key",
            applies_to: "the `per` leg of an exclusive narrative rule \
                (which typed leg the at-most-one rule keys on)",
            default: None,
            values: exclusive_key_values(),
        },
    ]
}

fn interval_op_values() -> Vec<EnumValue> {
    // Exhaustive `match` forces a gloss; the enumeration derives (R629).
    fn gloss(o: IntervalOp) -> &'static str {
        match o {
            IntervalOp::Ge => "the difference must be at least the bound",
            IntervalOp::Le => "the difference must be at most the bound",
            IntervalOp::Eq => "the difference must equal the bound exactly",
            IntervalOp::Gt => "the difference must exceed the bound",
            IntervalOp::Lt => "the difference must fall short of the bound",
        }
    }
    serde_variants::<IntervalOp>()
        .iter()
        .map(|tag| EnumValue {
            value: tag,
            description: gloss(variant_from_tag::<IntervalOp>(tag)),
        })
        .collect()
}

fn exclusive_key_values() -> Vec<EnumValue> {
    fn gloss(k: ExclusiveKey) -> &'static str {
        match k {
            ExclusiveKey::Subject => {
                "key on the SUBJECT leg: at most one co-holding OBJECT value per \
                subject. That is location exclusivity when the predicate reads \
                `at(person, place)` — one place per person."
            }
            ExclusiveKey::Object => {
                "key on the OBJECT leg: at most one co-holding SUBJECT value per \
                object. That is custody/conservation ONLY when the predicate is \
                written holder-first, `holds(holder, thing)` — then the object IS \
                the thing and this is one holder per thing. Write the same \
                relation thing-first, `held_by(thing, holder)`, and this value \
                means one THING per holder, the opposite rule under the same \
                name. MEASURED AT ROUND 965, not supposed: two corpora then on \
                record declared `held_by` with OPPOSITE `per` values and both \
                called it one-holder-per-thing, so read the legs, never the \
                role words."
            }
        }
    }
    serde_variants::<ExclusiveKey>()
        .iter()
        .map(|tag| EnumValue {
            value: tag,
            description: gloss(variant_from_tag::<ExclusiveKey>(tag)),
        })
        .collect()
}

/// Round 629 — THE variant oracle. `serde`'s derive ALREADY wrote every
/// variant's published tag down, and hands the list over through the
/// `Deserializer::deserialize_enum(name, variants, visitor)` **trait
/// signature** — an API contract, not an error-message format we parse. So the
/// contract's vocabulary is DERIVED from the same generator that produces the
/// wire, in the wire's own spelling, with no second derive macro and no hand
/// list to drift.
///
/// This replaces four hardcoded arrays whose comments claimed the compiler
/// forced them. It did not (Round 629 proved it: a 4th `RuleClass` variant, its
/// exhaustive matches satisfied, compiled clean with 293 tests green while
/// `describe-schema` silently omitted it). The exhaustive `match` in each
/// caller's `gloss`/`spec` forces a DESCRIPTION per variant — that part was
/// always true; nothing forced the ENUMERATION, which is what this fixes.
///
/// Do not "simplify" this to `T::as_str()`: that is a hand-written mirror whose
/// doc claims to match the serde representation and is enforced by nothing.
///
/// Round 644 — `pub(crate)` so the from_tag/as_str round-trip parity pins can
/// reach the one variant oracle instead of standing up a second one.
pub(crate) fn serde_variants<T>() -> &'static [&'static str]
where
    T: for<'de> serde::Deserialize<'de>,
{
    use serde::de::{Deserializer, Visitor};

    struct Capture(Option<&'static [&'static str]>);

    /// Deserialization is ABORTED the moment the list is captured — we want the
    /// contract, never a value; this error is the abort signal, not a failure.
    #[derive(Debug)]
    struct Captured;
    impl std::fmt::Display for Captured {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "variant list captured")
        }
    }
    impl std::error::Error for Captured {}
    impl serde::de::Error for Captured {
        fn custom<M: std::fmt::Display>(_: M) -> Self {
            Captured
        }
    }

    impl<'de> Deserializer<'de> for &mut Capture {
        type Error = Captured;
        fn deserialize_enum<V: Visitor<'de>>(
            self,
            _name: &'static str,
            variants: &'static [&'static str],
            _visitor: V,
        ) -> Result<V::Value, Captured> {
            self.0 = Some(variants);
            Err(Captured)
        }
        fn deserialize_any<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value, Captured> {
            Err(Captured)
        }
        serde::forward_to_deserialize_any! {
            bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char str string bytes
            byte_buf option unit unit_struct newtype_struct seq tuple
            tuple_struct map struct identifier ignored_any
        }
    }

    let mut capture = Capture(None);
    let _ = T::deserialize(&mut capture);
    let variants = capture
        .0
        .expect("serde reports the variant list for every derived enum (this type is not one)");
    // F5 vacuity guard (R510): a capturer that silently returned an EMPTY list
    // would make every vocabulary read as "no values" and every downstream
    // check pass vacuously — the failure mode this oracle exists to prevent.
    assert!(
        !variants.is_empty(),
        "variant oracle returned an empty list — a vacuous contract is worse than a stale one"
    );
    variants
}

/// Round 629 — the serde-reported tag back to its variant, so a caller can hand
/// the variant to its exhaustive `gloss`/`spec` match without a hand-written
/// tag-to-variant table (which would be the drift this oracle removes, moved).
fn variant_from_tag<T>(tag: &'static str) -> T
where
    T: for<'de> serde::Deserialize<'de>,
{
    use serde::de::IntoDeserializer;
    let de: serde::de::value::StrDeserializer<'static, serde::de::value::Error> =
        tag.into_deserializer();
    T::deserialize(de).expect("a serde-reported tag always deserializes back to its variant")
}

fn disclosure_mode_values() -> Vec<EnumValue> {
    // The exhaustive `match` forces a DESCRIPTION for every variant; the
    // ENUMERATION is derived (R629), not hand-listed as it was until R628.
    fn gloss(m: DisclosureMode) -> &'static str {
        match m {
            DisclosureMode::Withhold => {
                "never told; the reader reconstructs it (the default — \
                the sparse-frame ethos on disclosure, the Dark-Souls hidden-lore extreme)"
            }
            DisclosureMode::State => "told outright",
            DisclosureMode::Hint => "partially signalled",
            DisclosureMode::Imply => {
                "realised via an object/environment (the Dark-Souls \
                item-text)"
            }
        }
    }
    serde_variants::<DisclosureMode>()
        .iter()
        .map(|tag| EnumValue {
            value: tag,
            description: gloss(variant_from_tag::<DisclosureMode>(tag)),
        })
        .collect()
}

fn payoff_expectation_values() -> Vec<EnumValue> {
    fn gloss(p: PayoffExpectation) -> &'static str {
        match p {
            PayoffExpectation::Unmarked => {
                "the author has not marked the fact a setup (default \
                — unrecorded, never an assertion that it is not a setup)"
            }
            PayoffExpectation::Expected => {
                "a setup whose payoff should become visible in every \
                world where the setup is; dangling until then (a report finding, never gated)"
            }
        }
    }
    serde_variants::<PayoffExpectation>()
        .iter()
        .map(|tag| EnumValue {
            value: tag,
            description: gloss(variant_from_tag::<PayoffExpectation>(tag)),
        })
        .collect()
}

fn predicate_object_kind_values() -> Vec<EnumValue> {
    fn gloss(k: PredicateObjectKind) -> &'static str {
        match k {
            PredicateObjectKind::Entity => {
                "the object leg names a registered entity that is \
                also a member of the fact's entities list (locations, custody targets)"
            }
            PredicateObjectKind::Token => {
                "the object leg is a member of the predicate's CLOSED, declared \
                vocabulary (`object_tokens`) — enumerable, so the substrate can answer \
                what values this predicate takes; a token outside the set is rejected"
            }
            PredicateObjectKind::Quantity => {
                "the object leg is a number + a REGISTERED unit \
                (`{kind:quantity, n, unit}`) — the amount slot for timeline/measurement \
                facts; `n` is an exact integer, `unit` a ref into the units registry \
                (add-unit first, invariant 4), an unregistered unit is rejected"
            }
            PredicateObjectKind::Fact => {
                "the object leg REFERENCES another fact of this store \
                (`{kind:fact, id}`) — a typed fact-ref (e.g. `opened_by`); existence is \
                checked in PHASE 2 against store + staged (a same-manifest forward ref is \
                legal), self-reference is rejected, and the delete path refuses to orphan it"
            }
        }
    }
    serde_variants::<PredicateObjectKind>()
        .iter()
        .map(|tag| EnumValue {
            value: tag,
            description: gloss(variant_from_tag::<PredicateObjectKind>(tag)),
        })
        .collect()
}

fn rule_class_specs() -> Vec<RuleClassSpec> {
    // The exhaustive `match` forces a SPEC for every class; the ENUMERATION is
    // derived (R629). Until R628 this carried a comment claiming the compiler
    // forced the hand-written array below it — R629 disproved that by adding a
    // 4th variant, satisfying the matches, and watching the contract omit it
    // with 293 tests green. The `class` tag now comes from serde, so it cannot
    // disagree with the wire either.
    fn spec(c: RuleClass) -> RuleClassSpec {
        match c {
            RuleClass::Exclusive => RuleClassSpec {
                class: "exclusive",
                description: "At most one co-holding value on the leg the rule does NOT key \
                    on, per value of the leg it does, within one (frame x world). \
                    `per: subject` keys on the subject (one object value per subject); \
                    `per: object` keys on the object (one subject value per object). Which \
                    of those is location exclusivity and which is custody depends on how \
                    YOUR predicate orders its legs, not on the rule's name — see \
                    `exclusive_key`. Overlapping typed legs that violate this are a \
                    continuity-gate reject.",
                parameters: vec![
                    FieldSpec {
                        name: "per",
                        ty: "`subject` | `object`",
                        required: true,
                        description: "Which typed leg the rule keys on.",
                    },
                    FieldSpec {
                        name: "containment",
                        ty: "predicate id (optional)",
                        required: false,
                        description: "Round 714: makes exclusivity REFINEMENT-AWARE. The \
                            predicate whose facts are `contains(container, contained)`. Two \
                            co-holding non-keyed values that are COMPARABLE in this containment \
                            order (one transitively contains the other, e.g. `at(p, classroom)` \
                            and `at(p, school)` with classroom in school) REFINE one location — \
                            a finer + a coarser statement of the same place — so the overlap is \
                            NOT flagged. Evaluated holds_at-scoped at the co-hold point in the \
                            pair's frame-world. Omit for literal-value exclusivity.",
                    },
                ],
            },
            RuleClass::Transition => RuleClassSpec {
                class: "transition",
                description: "Rides the in-frame succession edge: a successor and predecessor \
                    both typed with the same subject+predicate must form an ALLOWED `(from, \
                    to)` step. Succession says WHICH pairs are steps; the declared map says \
                    which steps are allowed, and the two are not the same relation — a step is \
                    judged at the deepest scope where its endpoints are comparable, so siblings \
                    need an adjacency EDGE while entering or leaving a container needs NO EDGE \
                    AT ALL (see `containment`). Unchained same-subject pairs are surfaced, \
                    never gated.",
                parameters: vec![
                    FieldSpec {
                        name: "adjacency",
                        ty: "predicate id (the edge source)",
                        required: true,
                        description: "Round 697 (store-native map): the predicate whose FACTS \
                            are the edges — `adjacent(a, b)`, read from the store. This is how \
                            movement between PLACES is gated; the edges are store facts, not a \
                            file list. Each edge's legs are OBJECT KEYS (a registered entity id \
                            when the adjacency predicate's `object_kind` is `entity`, else the \
                            token value).",
                    },
                    FieldSpec {
                        name: "undirected",
                        ty: "bool (default false)",
                        required: false,
                        description: "Round 697: EDGE SYMMETRY, and Round 924: that is ALL it \
                            declares. CHOOSE IT BY WHAT YOU NEED TO SAY ABOUT THE WAY. true = an \
                            `adjacent(a, b)` fact admits BOTH (a, b) and (b, a), and the two \
                            directions can then never differ. Absent/false = one fact is ONE WAY, \
                            which is what lets a way cost more upward than downward — `edge_costs` \
                            keys on the FACT, so a symmetric edge carries exactly one cost for \
                            both directions and there is no second place to put the other number \
                            — and what keeps `alive → dead` from admitting the reverse (the same \
                            knob serves both). THAT A TWO-WAY ROAD IS THEN TWO FACTS IS A PRICE, \
                            NOT A REASON: the fact count is the one consideration that says \
                            nothing about the way, and picking symmetry to halve it trades a \
                            capability for bookkeeping. THIS DOES NOT DECLARE WHETHER YOUR RULE \
                            IS A MAP OR A LIFECYCLE. Two checks used to read it that way and were \
                            wrong about the maps they exempted (Round 918); nothing reads it that \
                            way now — a map's islands are named and its unreachable pairs counted \
                            whichever value you set, and you set it purely by whether one fact \
                            means one direction or two.",
                    },
                    FieldSpec {
                        name: "containment",
                        ty: "predicate id (optional)",
                        required: false,
                        description: "Round 716 (per-scope map; supersedes the Round 703 grouping \
                            model): the predicate whose facts are `contains(container, contained)`. \
                            Its facts form a per-(frame, world) containment TREE, and that tree \
                            PARTITIONS the adjacency edges into SCOPES: an edge may only join \
                            SIBLINGS — two places with the SAME direct container (the root scope \
                            counts). An edge between non-siblings is `adjacency_cross_scope`, \
                            evaluated per canon point (a place that MOVES makes the same edge \
                            sibling early and cross-scope late). A container is NOT a search-key \
                            you must not walk on — it PARTICIPATES AS A NODE in its parent's scope, \
                            and that is how you leave it: a PORTAL is the container's own edge to \
                            its siblings, not an edge from something inside it to something \
                            outside. So model a village of walled districts as districts adjacent \
                            to each other (or to a shared road), never as a house in one district \
                            adjacent to a house in the next. Wiring the predicate also turns on the \
                            completeness/leak checks PER SCOPE, UNION-over-canon-points: a \
                            place-kind entity off every scope at every point is `map_invented_\
                            place`, a contained thing that is never a node nor itself a container \
                            is `map_contained_off_map`, and an undirected scope whose sub-graph is \
                            disconnected at every point is `map_disconnected`. \
                            ROUND 934 — TWO OF THOSE THREE ARE TURNED ON BY THIS DECLARATION; \
                            `map_invented_place` NEEDS A SECOND ONE. It asks whether every PLACE \
                            is on the map, and only your ADJACENCY PREDICATE can say what a place \
                            is: the kind comes from that predicate's `subject_kind` / \
                            `object_entity_kind` (either leg, both read), never from a hardcoded \
                            \"place\". Declare neither and the store cannot be asked which \
                            entities are places, so the class emits nothing — the run then reads \
                            exactly like one where every place was on the map. It is NOT a \
                            violation to leave the kinds off, and nothing here rejects you for \
                            it; `validate-continuity` NAMES the rule and predicate whose \
                            completeness went unevaluated, because a gate that evaluated nothing \
                            must never read like a gate that passed. THIS WAS MEASURED AT ROUND \
                            934, not supposed: three of the six corpora then recorded — one \
                            author per arm across three arms — declared no leg kind \
                            at all and were never told. A container-less map \
                            degenerates to ONE root scope (the flat Round 702/703 behaviour). \
                            KNOWN LIMIT: two mutually-unreachable TOP-LEVEL containers produce no \
                            finding. Omit for a flat map. \
                            ROUND 909 — THE MAP IS POSSIBILITY, NOT ITINERARY, and this is the \
                            distinction an author reaches for and does not find. To write a place \
                            that EXISTS but that nobody in the story ever visits, put it ON the \
                            map (give it an edge) and simply never travel it in the canon order: \
                            being unvisited is a property of the telling, not of the map. Leaving \
                            it OFF the map instead — no edges — is `map_invented_place`, and \
                            naming it inside a container without edges is that PLUS \
                            `map_contained_off_map`. All three were measured on one authored town: \
                            of the three ways to encode \"spoken of but never reached\", exactly \
                            one passes, and the two that fail are the ones the phrase suggests. \
                            ROUND 913 — HOW A SUBJECT ENTERS A CONTAINER, which is the question two \
                            blind authors (R910) each raised unprompted and each answered unaided. \
                            Just write the step. A succession is judged AT THE DEEPEST SCOPE WHERE \
                            ITS ENDPOINTS ARE COMPARABLE, because the map declares two relations \
                            and a step may use either: places sharing a direct container need an \
                            EDGE between them; a pair where one transitively CONTAINS the other is \
                            a descent (entering) or an ascent (leaving) and needs no edge at all; \
                            anything else is judged between the two ancestors that ARE siblings, so \
                            a step from a gate to a market inside a district is licensed by the \
                            gate-to-district edge you already drew. That lift is a CHECK, not a \
                            licence — reaching a shrine inside a palace from a gate that is not \
                            adjacent to the palace is `rule_transition_invalid`, and the finding \
                            names the lifted pair, which is the edge you could author. Adding an \
                            edge from a container to something inside it is still \
                            `adjacency_cross_scope`: the step model changed here, the edge model \
                            did not. ROUND 925 — A CHAIN OF CROSSINGS IS ONE MOVE. Consecutive \
                            crossings by one subject (out of a room, into the courtyard, into \
                            another room) are judged TOGETHER, between the last place before the \
                            chain and the first place after it, because a crossing changes the \
                            GRAIN of the claim and not the position — so a chain of them moves \
                            nobody either. Routing a step through a container therefore does NOT \
                            license it: if no edge joins the two rooms, saying `room-a` -> \
                            `courtyard` -> `room-b` is the same finding as saying `room-a` -> \
                            `room-b`, and the finding names the whole route. A chain that ends \
                            BACK WHERE IT STARTED needs no edge at all — nobody moved. THE \
                            ALTERNATIVE TELLING, still legal (Round 911): let the \
                            COARSE fact (`at` the container) CO-HOLD across the whole visit — one \
                            fact whose canon extent spans it — while the FINE facts inside succeed \
                            each other normally. That overlap is legal only because a \
                            refinement-aware exclusive rule (one declaring `containment`, Round \
                            714) reads a coarser and a finer statement of one position as \
                            refinement rather than conflict; drop that `containment` and the same \
                            corpus is `rule_exclusive_overlap`. THE LIMIT, stated: only a DECLARED \
                            crossing is checked. Leave the crossing undeclared — the ellipsis of \
                            untold travel, which is legitimate — and the pair appears in \
                            `unchained_state_pairs`, surfaced and never gated, so a corpus that \
                            never says how a subject got inside is taken on trust. WHY THIS \
                            CHANGED: before Round 913 the crossing was not wrong but UNSAYABLE, \
                            and across four blind authorings nobody declared one — one author \
                            called 6 of 13 places unreachable and wrote none of them, reporting it \
                            as the shape the map model forced.",
                    },
                ],
            },
            RuleClass::Interval => RuleClassSpec {
                class: "interval",
                description: "A numeric/arithmetic relation over numeric typed legs, same \
                    subject: value(left_predicate) - value(right) `op` bound. Expresses \
                    constraints the equality/exclusivity gates cannot; a non-numeric operand is \
                    surfaced (interval_unverifiable), never silently passed.",
                parameters: vec![
                    FieldSpec {
                        name: "right",
                        ty: "predicate id (the second operand)",
                        required: true,
                        description: "The right operand, resolved on the same subject.",
                    },
                    FieldSpec {
                        name: "op",
                        ty: "`ge` | `le` | `eq` | `gt` | `lt`",
                        required: true,
                        description: "The comparison operator.",
                    },
                    FieldSpec {
                        name: "bound",
                        ty: "a TAGGED object: { \"const\": number } | { \"predicate\": <predicate \
                             id> }",
                        required: true,
                        description: "Round 907: the right-hand bound, and it is NEVER a bare \
                            number — `\"bound\": 5` is a PARSE ERROR (`invalid type: integer, \
                            expected struct IntervalBoundWire`). Write { \"const\": 5 } for a \
                            literal, read in the difference's own unit (a const has no unit slot), \
                            or { \"predicate\": <id> } for a bound resolved on the same subject as \
                            the operands — an inherited rule fact such as `min-ratify-gap-days`. \
                            That predicate must yield a NUMERIC object (a `quantity`, or a bare \
                            numeric token); a non-numeric operand is surfaced as \
                            `interval_unverifiable`, never subtracted.",
                    },
                ],
            },
        }
    }
    serde_variants::<RuleClass>()
        .iter()
        .map(|tag| RuleClassSpec {
            class: tag,
            ..spec(variant_from_tag::<RuleClass>(tag))
        })
        .collect()
}

/// Round 631 — the quest predicates and their REQUIRED object kind, read by the
/// validate-layer quest-shape guard (`continuity::check_quest_predicate_shapes`)
/// so a store cannot hold a `requires`/`pursues` fact with a non-entity object where
/// the contract declares an entity. Derived from the ONE contract in
/// `quest_encoding` — the guard shares the SSOT with `describe-schema`, never a
/// second hardcoded list (the R629 drift class). `None` = both kinds allowed.
pub(crate) fn quest_predicate_object_kinds(
) -> impl Iterator<Item = (&'static str, Option<PredicateObjectKind>)> {
    quest_encoding()
        .predicates
        .into_iter()
        .map(|p| (p.predicate, p.required_object_kind))
}

fn quest_encoding() -> QuestEncoding {
    QuestEncoding {
        description: "A quest is the NARRATIVE instance of the substrate's universal \
            tracked-obligation pattern, PROJECTED from existing primitives — no new substrate. \
            An author adopts these reserved ids so `report-quest-graph` can read the store; the \
            projection derives per-world open/done, prerequisites, and giver locators.",
        derivation: "A quest is any entity occupying a quest predicate ROLE — the object of \
            `pursues`, either endpoint of `requires`, or the subject of `completed_by`. There is \
            NO `kind` marker (R676): the reserved predicates are the sole signal, and an entity \
            used as both a quest and an actor is a fail-loud reversed/mis-typed slot.",
        predicates: vec![
            QuestPredicate {
                predicate: QUEST_PRED_PURSUES,
                role: "an actor entity (subject) LEADS the quest (object) — the quest's actors.",
                object_shape: "entity (the quest)",
                required_object_kind: Some(PredicateObjectKind::Entity),
            },
            QuestPredicate {
                predicate: QUEST_PRED_REQUIRES,
                role: "a quest (subject) is gated by another quest (object) that must complete \
                    first — the declarative prerequisite; the canon order proves the timing.",
                object_shape: "entity (the prerequisite quest)",
                required_object_kind: Some(PredicateObjectKind::Entity),
            },
            QuestPredicate {
                predicate: QUEST_PRED_COMPLETED_BY,
                role: "a quest (subject) is DISCHARGED by an actor (object) on a road — the \
                    carrying fact also `pays_off` the quest's giving setup.",
                object_shape: "entity or token (the discharger)",
                required_object_kind: None,
            },
        ],
        completion_rule: "A quest's GIVING setup is a `payoff_expectation: expected` fact that \
            the quest's OWN `completed_by` fact `pays_off` (strict-combined, R569 — no \
            scene-proximity bridge, so two quests completing at one scene never bleed givings). \
            A quest with no such binding is `unresolved` (surfaced, not dropped).",
        state_derivation: "open/done is DERIVED per world-line from the R442 payoff coverage of \
            the giving fact — paid here = done, dangling here = open, not visible here = unknown \
            — never stored. Executable quest lifecycle/guards are SCE/pinion's, not modeled here \
            (the declarative-vs-executable line).",
    }
}

fn invariants() -> Vec<Invariant> {
    let mutate = "mutate primitive (write-time reject)";
    vec![
        Invariant {
            name: "registered-frame",
            rule: "NarrativeFact.frame must name a registered frame id (add-frame first).",
            enforced_at: mutate,
        },
        Invariant {
            name: "registered-branch",
            rule: "a non-default NarrativeFact.branch must name a registered branch id \
                (add-branch first); a write-side typo must never silently create a world.",
            enforced_at: mutate,
        },
        Invariant {
            name: "registered-entities",
            rule: "every NarrativeFact.entities ref must name a registered entity — no blanks, \
                no duplicates.",
            enforced_at: mutate,
        },
        Invariant {
            name: "evidence-provenance",
            rule: "evidence has >= 1 ref and canon_from / canon_to / every evidence ref must \
                name an existing section (a claim without provenance is unauditable). EXISTENCE is \
                what the write path checks; ORDER is checked by the continuity gate, which requires \
                every evidence ref to be reachable and AT OR BEFORE `canon_from` in this fact's \
                world-line — so a manifest carrying a forward reference IMPORTS CLEANLY and fails \
                the gate as `evidence_unreachable` (Round 912).",
            enforced_at: mutate,
        },
        Invariant {
            name: "typed-subject-listed",
            rule: "a TypedClaim.subject must be a registered entity AND a member of the fact's \
                entities list (a typed leg never silently widens the retrieval key); an \
                entity-shaped object obeys the same registered-and-listed rule.",
            enforced_at: mutate,
        },
        Invariant {
            name: "registered-predicate",
            rule: "a TypedClaim.predicate must name a registered predicate id (load-bearing — \
                rules key off it).",
            enforced_at: mutate,
        },
        Invariant {
            name: "object-shape-match",
            rule: "the typed object's shape must match the predicate's declared object_kind \
                (entity | token | quantity | fact); a token must be a declared-vocabulary \
                member, a quantity's unit a registered unit, and a fact ref must resolve.",
            enforced_at: mutate,
        },
        Invariant {
            name: "same-frame-succession",
            rule: "supersedes_in_frame must name an existing fact in the SAME frame; no \
                self-reference. Cross-branch succession is legitimate only along fork/confluence \
                lineage.",
            enforced_at: mutate,
        },
        Invariant {
            name: "pays-off-exists",
            rule: "every pays_off ref must name an existing setup fact — no self-reference, no \
                duplicates (a payoff resolves an existing setup).",
            enforced_at: mutate,
        },
        Invariant {
            name: "branch-forest",
            rule: "a branch is EITHER a fork (forks_from) XOR a confluence (converges_from, >= 2 \
                parents); every parent must be pre-registered and not the branch itself; forks \
                are immutable after registration — acyclic by construction.",
            enforced_at: mutate,
        },
        Invariant {
            name: "disclosure-needs-typed",
            rule: "a `withhold` mode OR any first_at timing pin requires the targeted fact to \
                carry a typed claim — the premature-leak gate matches re-extracted prose to the \
                plan by typed tuple, so a decision on an untyped fact is un-gateable. Holds on \
                every write authority (Round 626): amend-fact cannot drop the typed leg out \
                from under a live one either — clear the decision first (remove-disclosure).",
            enforced_at: mutate,
        },
        Invariant {
            name: "disclosure-ref-integrity",
            rule: "a fact carrying a disclosure decision under any telling cannot be retracted \
                (Round 626) — clear each decision first (remove-disclosure --telling <id> \
                --fact <id>). Set-disclosure refuses a decision on an absent fact, so the \
                delete path must not create one from the far side; an override with mode \
                state/hint/imply and no first_at pin is re-checked by NO gate, so its orphan \
                would be silent. Clearing is not neutral: the fact then rides the plan's \
                default_mode (default `withhold`).",
            enforced_at: mutate,
        },
        Invariant {
            name: "content-hashes-computed",
            rule: "quote_sha256 and a conflict's target_claim_sha256 are computed by the \
                primitive, never caller-supplied — out-of-band drift stays detectable offline.",
            enforced_at: mutate,
        },
    ]
}

/// Round 929 — the SUPERSEDED tellings of how a subject gets inside a container,
/// in ONE list read by every surface that can carry one.
///
/// R911 answered "do not model it as a succession at all" and R913 falsified
/// that: the crossing IS a declarable, checked step. The sentence therefore may
/// not survive anywhere an author reads, and there are THREE such places — the
/// rule-class prose, the rules-file wire, and the repair hint a rejected author
/// is handed. R917 found the list scanning the first two and `verdict.rs`
/// carrying a single hand-written substring of its own, so R911's answer could
/// return there reworded past it. One list, three readers: a phrase added here
/// is checked on every surface at once, which is the property a second copy
/// cannot have.
///
/// This catches a stale sentence RETURNING VERBATIM. It cannot catch a rewording,
/// and it is not asked to — that is the job of the POSITIVE claim pins beside
/// each use, which fail when the claim they name is inverted or deleted.
#[cfg(test)]
pub(crate) const SUPERSEDED_CROSSING_TELLINGS: &[&str] = &[
    "must not be walked on",
    "a search-key, not a position",
    "Entry is NOT a succession",
    "no intermediate state exists",
    "do not model it as a",
    "Succession IS the declared adjacency",
];

#[cfg(test)]
mod tests {
    use super::*;

    /// EVERY manifest array has a contract entry — asked of the TYPE, not of a
    /// hand-list.
    ///
    /// Round 956, and it is a defect this round's own change surfaced. Its
    /// sibling `manifest_wire_prose_names_every_serde_key` checks that a
    /// DECLARED kind's prose names every key of it, and nothing at all checked
    /// that a kind was declared: deleting the whole `edge_costs` row left the
    /// suite green. A manifest array with no contract entry is invisible to
    /// `describe-schema`, which is the document every blind author reads, so an
    /// authoring surface can ship and stay unreachable in practice — which is
    /// the exact condition this round exists to end.
    #[test]
    fn every_manifest_array_has_a_contract_entry() {
        let empty = serde_json::to_value(mnemosyne_atomic::FactsManifest {
            frames: vec![],
            branches: vec![],
            entity_kinds: vec![],
            units: vec![],
            entities: vec![],
            predicates: vec![],
            facts: vec![],
            edge_costs: vec![],
            edge_guards: vec![],
            disclosure_plans: vec![],
        })
        .expect("a manifest serializes");
        let keys: Vec<String> = empty
            .as_object()
            .expect("a struct serializes to an object")
            .keys()
            .cloned()
            .collect();
        assert!(!keys.is_empty(), "no manifest keys — nothing asserted");
        let described: Vec<&str> = manifest_kinds().iter().map(|k| k.kind).collect();
        let missing: Vec<&String> = keys
            .iter()
            .filter(|k| !described.contains(&k.as_str()))
            .collect();
        assert!(
            missing.is_empty(),
            "manifest array(s) with no entry in the authoring contract, so an \
             author reading `describe-schema` never learns the slot exists: \
             {missing:?}"
        );
        println!("{} manifest array(s), all described: {keys:?}", keys.len());
    }

    /// The contract names a disclosure surface's two legs as the REGISTRY REFS
    /// the write path rejects on, and not as bare strings.
    ///
    /// Round 955. Both legs are checked at write time against `sections` and
    /// `entities` — that enforcement has tests of its own in `mnemosyne-atomic`
    /// — and until this round the contract every blind author reads described
    /// them as `string`, two slots away from a `first_at` that spells out
    /// `branch id` and `[section id, …]`. An author reading `string` has been
    /// told the slot takes free text, and the only thing that corrects them is
    /// a reject they have to earn by running.
    ///
    /// Pinned here rather than left to prose review because the failure mode is
    /// silent reversion: nothing else in this tree reads the sentence.
    #[test]
    fn the_contract_types_a_disclosure_surface_as_registry_refs() {
        let wire = manifest_kinds()
            .into_iter()
            .find(|k| k.kind == "disclosure_plans")
            .expect("the contract describes disclosure_plans");
        let surface = wire
            .json_keys
            .split_once("\"surface\"")
            .expect("the disclosure wire describes a surface")
            .1;
        assert!(
            surface.contains("\"scene\": section id"),
            "the surface scene is a registered section, and the contract must \
             say so: {surface}"
        );
        assert!(
            surface.contains("\"object\"?: entity id"),
            "the surface object is a registered entity, and the contract must \
             say so: {surface}"
        );
        // The negative half: `string` here is exactly what this round removed,
        // so a revert must be red rather than merely unpinned.
        assert!(
            !surface.starts_with("?: {\"scene\": string"),
            "the surface legs are back to bare strings: {surface}"
        );
    }

    /// Round 636 — the published quest object rule is stated twice (machine +
    /// prose); this binds them, making R631's "cannot drift" claim true.
    #[test]
    fn quest_object_shape_prose_matches_the_enforced_kind() {
        // Round 636 — the quest object rule is stated TWICE in the published
        // contract: `object_shape` (prose, for the human/AI reading it) and
        // `required_object_kind` (machine, what the validate guard enforces).
        // R631 asserted in a doc comment that they "cannot drift" and bound them
        // with NOTHING — the same unbacked drift-safety claim R629 was paid to
        // delete. This is the binding. Drift here is not cosmetic: the contract
        // is the authority R620 designated, so prose saying "scalar" while the
        // machine enforces entity is precisely how that consumer was misled.
        for p in describe_schema().quest_encoding.predicates {
            match p.required_object_kind {
                // The prose must NAME the kind the machine enforces.
                Some(kind) => assert!(
                    p.object_shape.contains(kind.as_str()),
                    "quest predicate `{}`: enforced kind `{}` is absent from its published \
                     prose `{}` — the contract would teach a shape the guard rejects",
                    p.predicate,
                    kind.as_str(),
                    p.object_shape
                ),
                // `None` = both kinds legal; the prose must not read as a single
                // fixed shape, so it has to name BOTH (completed_by: "entity or
                // token"). Without this arm the None case is unpinned and the
                // test would be half-vacuous. (Round 708 — the second shape is
                // `token`, the free-text scalar having been removed.)
                None => {
                    let names_both = PredicateObjectKind::Entity.as_str();
                    let names_token = PredicateObjectKind::Token.as_str();
                    assert!(
                        p.object_shape.contains(names_both) && p.object_shape.contains(names_token),
                        "quest predicate `{}` accepts BOTH kinds, but its prose `{}` does not \
                         name both — an author would read one shape as the only legal one",
                        p.predicate,
                        p.object_shape
                    );
                }
            }
        }
    }

    /// The contract is complete and internally consistent: every declared
    /// section is present, the vocabularies mirror the core enums exactly (the
    /// drift guard's positive assertion), and the quest ids are the real
    /// constants.
    #[test]
    fn contract_is_complete_and_matches_source() {
        let c = describe_schema();
        assert_eq!(c.schema_version, mnemosyne_atomic::CURRENT_SCHEMA_VERSION);

        // Every narrative registry is described.
        let reg: Vec<_> = c.registries.iter().map(|r| r.name).collect();
        for expected in [
            "sections",
            "frames",
            "branches",
            "entities",
            "predicates",
            "disclosure_plans",
            // Round 729 (DEBT-K) — the meter economy registries (F5: assert the
            // new describe-schema entries land, since side-table docs are manual).
            "parameters",
            "parameter_deltas",
            // Round 730 (DEBT-K) — the choice-gate side-table.
            "parameter_gates",
        ] {
            assert!(reg.contains(&expected), "registry `{expected}` missing");
        }

        // The vocabularies mirror the core enums (value-for-value), so the
        // exhaustive-match drift guard is observably in force.
        let vocab = |name: &str| {
            c.vocabularies
                .iter()
                .find(|v| v.name == name)
                .unwrap_or_else(|| panic!("vocabulary `{name}` missing"))
        };
        // Round 629 — these three vocabularies used to be pinned to hardcoded
        // string arrays. That was a SECOND mirror of the producers' own
        // hardcoded arrays: both sides agreed at the stale value, so adding an
        // enum variant left the contract silently short AND the test green
        // (proven — a 4th RuleClass compiled with 293 tests passing and no
        // mention in the contract). The producers now DERIVE from serde, which
        // makes a membership assertion here tautological; pinning it again
        // would only move the hand-list into the test.
        //
        // What is NOT tautological, and is pinned instead: every vocabulary is
        // non-empty (an oracle that silently returned `[]` would make every
        // reader see "no values" and pass vacuously — the R510 F5 class), and
        // the vocabulary agrees with the enum's own `as_str()`, whose doc
        // CLAIMS "matches the serde representation" while nothing enforced it.
        // `as_str` is live in production (receipts, CLI/MCP json), so that
        // claim drifting is a real defect, not a hypothetical.
        for name in [
            "disclosure_mode",
            "payoff_expectation",
            "predicate_object_kind",
        ] {
            assert!(!vocab(name).values.is_empty(), "vocabulary `{name}` empty");
        }
        assert_eq!(vocab("disclosure_mode").default, Some("withhold"));

        fn as_str_matches_serde<T, F>(vocab_values: &[EnumValue], as_str: F)
        where
            T: for<'de> serde::Deserialize<'de> + Copy,
            F: Fn(T) -> &'static str,
        {
            for v in vocab_values {
                let variant = variant_from_tag::<T>(v.value);
                assert_eq!(
                    as_str(variant),
                    v.value,
                    "`as_str()` disagrees with the serde tag the contract publishes"
                );
            }
        }
        as_str_matches_serde::<DisclosureMode, _>(&vocab("disclosure_mode").values, |m| m.as_str());
        as_str_matches_serde::<PayoffExpectation, _>(&vocab("payoff_expectation").values, |p| {
            p.as_str()
        });
        as_str_matches_serde::<PredicateObjectKind, _>(
            &vocab("predicate_object_kind").values,
            |k| k.as_str(),
        );

        // Every enum value carries a non-empty gloss.
        for v in &c.vocabularies {
            for val in &v.values {
                assert!(!val.description.is_empty(), "empty gloss on {}", val.value);
            }
        }

        // Round 629 — the rule classes are DERIVED from the enum's serde tags,
        // so a count/membership pin here would be the same second mirror. What
        // is pinned: the set is non-empty (vacuity), and every class the
        // contract publishes round-trips through serde — i.e. the `class` tag
        // is the wire spelling, not a hand-typed lookalike.
        assert!(
            !c.narrative_rules.is_empty(),
            "no rule classes described — a vacuous contract"
        );
        for r in &c.narrative_rules {
            let _: RuleClass = variant_from_tag(r.class);
        }

        // The quest ids are the real projection constants (single-sourced).
        // R676 — no `entity_kind` marker; the contract advertises the derivation.
        assert!(
            c.quest_encoding.derivation.contains("predicate")
                && !c.quest_encoding.derivation.contains("kind\":\"quest"),
            "quest contract must advertise role-derivation, not a kind marker"
        );
        let preds: Vec<_> = c
            .quest_encoding
            .predicates
            .iter()
            .map(|p| p.predicate)
            .collect();
        assert_eq!(
            preds,
            [
                QUEST_PRED_PURSUES,
                QUEST_PRED_REQUIRES,
                QUEST_PRED_COMPLETED_BY
            ]
        );

        // The invariant set is non-empty and every entry names where it fires.
        assert!(!c.invariants.is_empty());
        for inv in &c.invariants {
            assert!(
                !inv.enforced_at.is_empty(),
                "invariant `{}` has no locus",
                inv.name
            );
        }

        // The out-of-band enforcement note (R591) records the continuity re-check.
        assert!(c.invariant_enforcement.contains("continuity"));
    }

    /// Round 644 — the from_tag/as_str pair on a manually-labelled enum is TWO
    /// hand-written mirrors of serde's own tag list, and only `as_str` is
    /// compiler-forced (its `match` is exhaustive). `from_tag` is a `&str` match
    /// ending `_ => None`, so a NEW variant serializes through serde but silently
    /// fails to parse back — nothing forces the arm, exactly the class R629
    /// convicted for `describe-schema`'s vocabularies.
    ///
    /// This binds BOTH directions of all six pairs to the one variant oracle
    /// (`serde_variants`, the serde-derived list): for every published tag,
    /// `as_str(from_tag(tag)) == tag` AND `from_tag` accepts it. Injecting a new
    /// variant without extending its `from_tag` fails here; the pins are DERIVED,
    /// so they carry no second copy of the vocabulary to drift (R622).
    ///
    /// `as_str` on the three atomic enums takes `self` by value (they are
    /// `Copy`), so each is exercised through a closure that names the method.
    #[test]
    fn from_tag_and_as_str_round_trip_through_the_serde_oracle() {
        use mnemosyne_atomic::{ConfirmMethod, ConfirmerKind, Verdict};

        /// For every serde tag: `from_tag` parses it, and `as_str` of the parsed
        /// variant returns the same tag. Fails if either mirror omits a variant.
        fn round_trip<T>(from_tag: impl Fn(&str) -> Option<T>, as_str: impl Fn(T) -> &'static str)
        where
            T: Copy + for<'de> serde::Deserialize<'de>,
        {
            let tags = serde_variants::<T>();
            assert!(!tags.is_empty(), "vacuous: the oracle reported no variants");
            for tag in tags {
                let parsed = from_tag(tag)
                    .unwrap_or_else(|| panic!("from_tag rejects the published serde tag `{tag}`"));
                assert_eq!(
                    as_str(parsed),
                    *tag,
                    "as_str disagrees with the serde tag `{tag}` from_tag parsed it into",
                );
            }
        }

        round_trip(ConfirmerKind::from_tag, ConfirmerKind::as_str);
        round_trip(ConfirmMethod::from_tag, ConfirmMethod::as_str);
        round_trip(Verdict::from_tag, Verdict::as_str);
        round_trip(PredicateObjectKind::from_tag, PredicateObjectKind::as_str);
        round_trip(PayoffExpectation::from_tag, PayoffExpectation::as_str);
        round_trip(DisclosureMode::from_tag, DisclosureMode::as_str);
    }

    /// Round 660 — THE WRITE SURFACE GETS AN ORACLE, because R659 measured
    /// "the compiler will catch a new variant" FALSE for the second time
    /// (R625 was the first).
    ///
    /// The compiler forces every READER of [`TypedObject`] — an added variant
    /// breaks every `match` over it — and forces ZERO WRITERS, because
    /// `from_exclusive_args` matches `(Option<String>, Option<String>)`, NOT
    /// `TypedObject`. Adding a variant cannot break a function that only
    /// CONSTRUCTS the type: its input never changed, and its 2x2 match is
    /// already exhaustive. So the arity `2` is a HAND COPY of the variant
    /// count — the exact shape R448 consolidated ("both surfaces had
    /// hand-rolled copies") without ever deriving. R659 proved the cost: a
    /// variant wired into all 8 forced reader sites builds clean, clippy
    /// clean, 975/975 green, and passes the pre-commit hook, while being
    /// UNCONSTRUCTIBLE from the CLI, MCP, and the published JsonSchema — the
    /// half that carries the whole value.
    ///
    /// This closes the loop, and every link is derived or compiler-forced:
    /// 1. add a variant -> the DERIVED oracle grows, the surface does not,
    ///    THIS TEST FAILS;
    /// 2. the only fix is a new `from_exclusive_args` parameter -> its arity
    ///    changes -> EVERY call site (CLI, MCP) breaks the build;
    /// 3. so the author must reach the flag and the arg field to compile.
    ///
    /// The oracle is `PredicateObjectKind`, NOT `TypedObject`, and that is a
    /// measured constraint rather than a preference: [`serde_variants`]
    /// captures the list through `deserialize_enum`, which serde calls only
    /// for EXTERNALLY-tagged enums. `TypedObject` is `#[serde(tag = "kind")]`
    /// — internally tagged — so it routes `deserialize_any` and the capture
    /// never fires (it panics "this type is not one"). Every type the oracle
    /// feeds today is a plain unit enum for that reason. Read this before
    /// trying to point it at a data-carrying enum.
    ///
    /// Pointing it at `PredicateObjectKind` catches the direction that was
    /// actually UNGUARDED. The other direction is already compiler-forced: a
    /// bare `TypedObject::Fact` breaks `build_typed_claim`'s (object, kind)
    /// match. But a bare `PredicateObjectKind::Fact` breaks NOTHING — and
    /// that is precisely R659's proof: `add-predicate --object-kind fact` was
    /// ACCEPTED and PERSISTED while no fact could ever satisfy it, with the
    /// whole suite green.
    #[test]
    fn every_declared_object_kind_is_satisfiable_from_the_arg_surface() {
        use mnemosyne_core::TypedObject;

        /// Does this object satisfy that declared kind? EXHAUSTIVE with no
        /// wildcard on purpose (R624/R658): a new variant on either enum
        /// breaks THIS match, so the author is stopped here rather than
        /// shipping a green half-wire.
        fn conforms(object: &TypedObject, kind: PredicateObjectKind) -> bool {
            match (object, kind) {
                (TypedObject::Entity { .. }, PredicateObjectKind::Entity) => true,
                (TypedObject::Token { .. }, PredicateObjectKind::Token) => true,
                (TypedObject::Quantity { .. }, PredicateObjectKind::Quantity) => true,
                (TypedObject::Fact { .. }, PredicateObjectKind::Fact) => true,
                (TypedObject::Entity { .. }, PredicateObjectKind::Token)
                | (TypedObject::Entity { .. }, PredicateObjectKind::Quantity)
                | (TypedObject::Entity { .. }, PredicateObjectKind::Fact)
                | (TypedObject::Token { .. }, PredicateObjectKind::Entity)
                | (TypedObject::Token { .. }, PredicateObjectKind::Quantity)
                | (TypedObject::Token { .. }, PredicateObjectKind::Fact)
                | (TypedObject::Quantity { .. }, PredicateObjectKind::Entity)
                | (TypedObject::Quantity { .. }, PredicateObjectKind::Token)
                | (TypedObject::Quantity { .. }, PredicateObjectKind::Fact)
                | (TypedObject::Fact { .. }, PredicateObjectKind::Entity)
                | (TypedObject::Fact { .. }, PredicateObjectKind::Token)
                | (TypedObject::Fact { .. }, PredicateObjectKind::Quantity) => false,
            }
        }

        // THE SURFACE, measured: every arg combination the CLI flags
        // (`--typed-object-entity` / `--typed-object-token` /
        // `--typed-object-quantity-n` + `--typed-object-quantity-unit` /
        // `--typed-object-fact`) and the MCP fields can actually send (Round 708
        // removed the free-text value arg).
        let buildable: Vec<TypedObject> = [
            TypedObject::from_exclusive_args(Some("e".to_string()), None, None, None),
            TypedObject::from_exclusive_args(None, Some("t".to_string()), None, None),
            TypedObject::from_exclusive_args(None, None, Some((1, "u".to_string())), None),
            TypedObject::from_exclusive_args(None, None, None, Some("f".to_string())),
        ]
        .into_iter()
        .flatten()
        .collect();
        assert!(
            !buildable.is_empty(),
            "vacuous: the arg surface built no object at all"
        );

        // THE ORACLE, derived from the type — never a hand-list, because a
        // hand-list here would be a copy of the class this test kills.
        let declared = serde_variants::<PredicateObjectKind>();
        assert!(
            !declared.is_empty(),
            "vacuous: the oracle reported no variants"
        );

        for tag in declared {
            let kind = PredicateObjectKind::from_tag(tag)
                .unwrap_or_else(|| panic!("from_tag rejects its own serde tag `{tag}`"));
            assert!(
                buildable.iter().any(|o| conforms(o, kind)),
                "object_kind `{tag}` is DECLARED but NO arg combination can build an \
                 object that satisfies it — `add-predicate --object-kind {tag}` would be \
                 accepted and persisted, and no fact could ever use it (R659 measured \
                 exactly this, green). The compiler cannot see it: `from_exclusive_args` \
                 matches (Option, Option), not the enum, so it stays exhaustive while the \
                 CLI flag, the MCP field, and the published JsonSchema go unwired. Give \
                 the constructor a parameter for the new shape — the arity change then \
                 forces every call site to be reached."
            );
        }
    }

    /// The contract serializes to JSON (the machine-readable deliverable).
    #[test]
    fn contract_serializes_to_json() {
        let c = describe_schema();
        let json = serde_json::to_string_pretty(&c).expect("serialize");
        assert!(json.contains("\"schema_version\""));
        assert!(json.contains("quest_encoding"));
        assert!(json.contains("\"withhold\""));
        // Round 595/596 — the wire format + canon-order contract must ship.
        assert!(json.contains("manifest_wire"));
        assert!(json.contains("canon_order"));
        // Round 601 — the disclosure-encoding idiom must ship (gap B).
        assert!(json.contains("disclosure_encoding"));
        // Round 604 — the rules-file authoring surface must ship (surface_gap).
        assert!(json.contains("narrative_rules_wire"));
    }

    /// Round 604/605 (continuity-stress-experiment/v1 `surface_gap`; review F2) —
    /// the rules-file wire is a SERIALIZATION contract, so its describe-schema
    /// prose is REFLECTION-pinned to the real serde structs (mirroring
    /// `manifest_wire_prose_names_every_serde_key`): every key a fully-populated
    /// `NarrativeRulesWire` emits must be named (quoted) in the prose, so a serde
    /// rename in `continuity.rs` fails the build until the prose is updated. This
    /// replaces the earlier substring self-check (a wire format belongs in the
    /// TEST-guarded tier, not hand-authored tier-3). The wiring keys +
    /// interval opt-in are prose (not file serde keys), asserted separately.
    #[test]
    fn narrative_rules_wire_prose_names_every_serde_key() {
        fn assert_documented(value: &serde_json::Value, prose: &str) {
            match value {
                serde_json::Value::Object(map) => {
                    for (k, v) in map {
                        assert!(
                            prose.contains(&format!("\"{k}\"")),
                            "rules-wire serde key `{k}` is not named in narrative_rules_wire prose"
                        );
                        assert_documented(v, prose);
                    }
                }
                serde_json::Value::Array(items) => {
                    items.iter().for_each(|v| assert_documented(v, prose))
                }
                _ => {}
            }
        }
        let prose = describe_schema().narrative_rules_wire;
        assert_documented(
            &crate::continuity::narrative_rules_wire_sample_json(),
            prose,
        );
        // Wiring + gating are prose, not serde keys of the file.
        assert!(prose.contains("rules_path") && prose.contains("rules_sha256"));
        assert!(
            prose.contains("interval_severity"),
            "the interval gate opt-in"
        );
        assert!(
            prose.contains("exclusive")
                && prose.contains("transition")
                && prose.contains("interval")
        );
    }

    /// Round 601 (unattended-loop-experiment/v2 gap B + Finding 2) — the two
    /// hand-authored prose fixes must carry their load-bearing concepts: the
    /// disclosure-encoding paragraph names both halves of the pair and the
    /// frontier-is-not-the-leak-gate caveat; the `branches` registry names the
    /// dead-prefix dangling trap. Prose (tier-3, not serde-guarded), so this
    /// pins the concepts an agent must find, not the exact wording.
    ///
    /// Round 966 — THIS COMMENT USED TO CALL THE PAIR "the `withhold`+`first_at`
    /// reveal",
    /// which is the shape Round 946 measured as INERT and Round 947 sharpened.
    /// The assertions below never said the paragraph RECOMMENDS the pair — only
    /// that both words appear, which a paragraph naming the pair as the trap
    /// satisfies too. So the repair left every assertion standing and only the
    /// sentence describing them had to go; the sibling test below is the one
    /// that holds the prose to what the classifier actually does.
    #[test]
    fn disclosure_encoding_and_fork_lineage_trap_are_documented() {
        let c = describe_schema();
        let enc = c.disclosure_encoding;
        assert!(enc.contains("withhold"), "names the withhold default");
        assert!(enc.contains("first_at"), "names the per-world reveal pin");
        assert!(
            enc.contains("leak") || enc.contains("LEAK"),
            "warns about the leak"
        );
        assert!(
            enc.contains("report-authoring-frontier"),
            "states frontier-clean is not sufficient"
        );
        let branches = c
            .registries
            .iter()
            .find(|r| r.name == "branches")
            .expect("branches registry present");
        assert!(
            branches.description.contains("dead prefix")
                || branches.description.contains("DEAD PREFIX"),
            "branches names the dead-prefix trap"
        );
        assert!(
            branches.description.contains("dangle"),
            "branches names the dangling consequence"
        );
    }

    /// Round 615 — the `branches` contract must carry the WORLD-LINE MODEL a blind
    /// author cannot otherwise self-serve. Rounds 612/614 changed what a fork and a
    /// merge MEAN (facts intersect at a merge, roads union; a merge relocates trunk
    /// ownership onto the confluence; a branch that declares no road inherits the
    /// trunk's ENDING), and none of it was documented — an author reading only
    /// `describe-schema` would have authored a divergent ending whose terminal gates
    /// silently measured the trunk's. Prose (tier-3, not serde-guarded), so this pins
    /// the CONCEPTS an agent must find, never the wording.
    #[test]
    fn branches_contract_carries_the_road_and_merge_model() {
        let c = describe_schema();
        let b = &c
            .registries
            .iter()
            .find(|r| r.name == "branches")
            .expect("branches registry present")
            .description;
        // the two axes and their DUAL behaviour at a merge
        assert!(b.contains("ROAD"), "names the road axis");
        assert!(
            b.contains("INTERSECT") && b.contains("UNION"),
            "facts intersect at a merge, roads union — the duality is the model"
        );
        // the GAP-003 lesson an author must plan for
        assert!(
            b.contains("RELOCATES TRUNK OWNERSHIP"),
            "authoring a merge moves the post-merge scenes onto the confluence"
        );
        // where the road is declared, and what an UNDECLARED road costs
        assert!(
            b.contains("`edges` ARE `main`'s road"),
            "the base edges are main's road segment, not a global coordinate pool"
        );
        assert!(
            b.contains("undeclared_roads") && b.contains("validate-render-fidelity"),
            "an undeclared road means the terminal gates measure the TRUNK's ending"
        );
        // the road is also what `evidence` is checked against (R615)
        assert!(
            b.contains("evidence"),
            "citing a scene this world never travels is rejected"
        );
    }

    /// Round 592 — the fact-shape DRIFT GUARD: the described fact fields must
    /// equal `FactImport`'s serde field set (plus `fact_id`, which is the map
    /// key, described via the fact's `add_op` rather than as a field). Adding a
    /// field to `FactImport` fails this test until `describe-schema` describes
    /// it — closing the one place the contract could silently fall behind the
    /// real batch shape.
    #[test]
    fn fact_fields_match_fact_import_serde_shape() {
        use std::collections::BTreeSet;
        // FactImport serializes every field (no skip_serializing_if), so a
        // sample instance yields the full field set.
        let sample = mnemosyne_atomic::FactImport {
            fact_id: "x".into(),
            frame: "f".into(),
            branch: None,
            entities: vec![],
            claim: "c".into(),
            canon_from: "s".into(),
            canon_to: None,
            evidence: vec![],
            conflicts_with: vec![],
            supersedes_in_frame: None,
            payoff_expectation: None,
            pays_off: vec![],
            typed: None,
            quote: None,
        };
        let value = serde_json::to_value(&sample).unwrap();
        let import_fields: BTreeSet<String> = value.as_object().unwrap().keys().cloned().collect();

        let mut described: BTreeSet<String> = describe_schema()
            .fact
            .fields
            .iter()
            .map(|f| f.name.to_string())
            .collect();
        // fact_id is the map key, not a body field — described via add_op.
        described.insert("fact_id".to_string());

        assert_eq!(
            import_fields, described,
            "describe-schema fact fields drifted from FactImport's serde shape"
        );
    }

    /// Round 595 — the WIRE-FORMAT drift guard (unattended-loop Finding 1): the
    /// worked example must parse through the real `FactsManifest` and carry the
    /// shapes it advertises. Renaming a serialized key or the typed-object tag
    /// breaks this — a required key fails to parse; an optional one drops to its
    /// default and a content assertion fires. This pins the serialization the
    /// contract now documents so an agent never again reverse-engineers it.
    #[test]
    fn manifest_example_parses_and_pins_wire_shape() {
        let example = describe_schema().manifest_wire.example_json;
        let m: mnemosyne_atomic::FactsManifest = serde_json::from_str(example)
            .expect("manifest example must parse through the real FactsManifest parser");
        assert_eq!(m.frames.len(), 2);
        assert_eq!(m.branches.len(), 1);
        assert_eq!(m.branches[0].forks_from.as_deref(), Some("main"));
        assert_eq!(m.branches[0].forks_at.as_deref(), Some("sc-03"));
        assert_eq!(m.entities.len(), 2);
        assert_eq!(m.predicates.len(), 2);
        assert_eq!(m.facts.len(), 2);
        // the token typed object serializes with the tag `token` (Round 708 —
        // the free-text scalar/value shape was removed).
        let setup = &m.facts[0];
        assert_eq!(setup.payoff_expectation.as_deref(), Some("expected"));
        match &setup.typed.as_ref().expect("setup has a typed leg").object {
            mnemosyne_core::TypedObject::Token { token } => assert_eq!(token, "hidden"),
            other => panic!("state object must be the Token variant, got {other:?}"),
        }
        // the entity typed object serializes with the tag `entity` + `id`.
        let payoff = &m.facts[1];
        assert_eq!(payoff.pays_off, vec!["f-setup".to_string()]);
        match &payoff
            .typed
            .as_ref()
            .expect("payoff has a typed leg")
            .object
        {
            mnemosyne_core::TypedObject::Entity { id } => assert_eq!(id, "e-scout"),
            other => panic!("entity object must be the Entity variant, got {other:?}"),
        }
        // the disclosure override's first_at is a per-world reveal trigger
        // (branch + a coord SET + optional threshold, Round 752).
        assert_eq!(m.disclosure_plans.len(), 1);
        let overrides = &m.disclosure_plans[0].overrides;
        assert_eq!(overrides.len(), 1);
        assert_eq!(overrides[0].fact_id, "f-setup");
        assert_eq!(overrides[0].first_at.len(), 1);
        assert_eq!(overrides[0].first_at[0].branch, "road-b");
        assert_eq!(overrides[0].first_at[0].coords, vec!["sc-04".to_string()]);
        assert_eq!(overrides[0].first_at[0].threshold, None);
    }

    /// Round 909 — every registry an author may write to must have a documented
    /// WAY to write to it, and which way is derived, not asserted.
    ///
    /// A registry is reachable either as a manifest array or through its own
    /// verb. The contract listed all of them and documented the wire for only
    /// one route, so `edge_costs` and `edge_guards` — the two the map brief needs
    /// for "some journeys take longer" and "one way is shut until something is
    /// true" — had no authoring path anywhere in it. Measured on a real store:
    /// writing them as manifest arrays parses, exits 0, and builds nothing.
    ///
    /// The oracle is the contract's own two lists differenced against each
    /// other, so a registry added tomorrow joins the obligation by itself.
    #[test]
    fn every_registry_has_a_documented_authoring_path() {
        use std::collections::BTreeSet;
        let c = describe_schema();
        let manifest_kinds: BTreeSet<&str> = c.manifest_wire.kinds.iter().map(|k| k.kind).collect();

        // The registries a manifest cannot reach — derived, never listed here.
        let side: Vec<&RegistrySpec> = c
            .registries
            .iter()
            .filter(|r| !manifest_kinds.contains(r.name))
            .collect();
        assert!(
            side.len() >= 5,
            "only {} registries fell outside the manifest — the difference broke, and an \
             empty difference would assert nothing",
            side.len()
        );

        for r in side {
            // `sections` has its own file wire; everything else is a keyed side
            // table reached by verb.
            let home: &str = if r.name == "sections" {
                c.sections_wire
            } else {
                &c.side_table_wire
            };
            assert!(
                home.contains(r.add_op),
                "registry `{}` is not a manifest array, so `{}` is the only way to write it — \
                 and no wire prose names that verb. An author meets this registry in the list, \
                 guesses a manifest array, and is answered with exit 0 and nothing built.",
                r.name,
                r.add_op
            );
        }

        // The silent-no-op consequence must be stated where the guess is made,
        // not only where the verbs are listed.
        assert!(
            c.side_table_wire.contains("SILENT NO-OP"),
            "the failure mode is silence, so the contract has to say so"
        );
    }

    /// Round 957 — WHICH tables are verb-only is derived from the manifest
    /// roster, so wiring one retires the claim in the same change.
    ///
    /// Round 956 gave `edge_costs` and `edge_guards` a fact-manifest wire and
    /// this paragraph went on naming exactly those two as its examples of a
    /// silent no-op, closing with "nothing in a manifest will tell you so" —
    /// pointing every author who came looking straight back at the verbs. The
    /// R909 gate above could not see it, because that check is ONE-DIRECTIONAL:
    /// it requires each verb-only registry's verb to be NAMED here, and nothing
    /// forbade the prose from calling a manifest array verb-only. Measured on a
    /// scratch store before this round changed anything: a facts manifest
    /// carrying `edge_costs` and `edge_guards` imports them (`1 edge-costs +
    /// 1 edge-guard-conditions created`) and `report-transition-map` hands both
    /// back on the edge — so the wire worked and only the document was wrong.
    #[test]
    fn the_verb_only_claim_follows_the_manifest_roster_and_not_a_hand_list() {
        let regs = registries();
        let roster_now = manifest_wire();

        let claimed = verb_only_registries(&regs, &roster_now);
        assert!(
            claimed.len() >= 4,
            "only {} table(s) fell outside the manifest — an empty difference would assert \
             nothing at all",
            claimed.len()
        );

        // `sections` sits outside the fact manifest too, but it has a file wire
        // of its own, so calling it verb-only would be this paragraph's own
        // false claim. This fires if the exclusion is dropped.
        assert!(
            !claimed.contains(&"sections"),
            "`sections` has its own file wire and must never be called verb-only"
        );

        // THE DISCRIMINATING ARM. The same registries differenced against a
        // roster WITHOUT the two arrays Round 956 wired IS the pre-956 world,
        // and the claim has to move between the two. Asserting that today's
        // claim omits them would be vacuous on its own — the function filters
        // on exactly that — so what is checked is that the ROSTER is what
        // decides.
        let mut roster_pre_956 = manifest_wire();
        roster_pre_956
            .kinds
            .retain(|k| k.kind != "edge_costs" && k.kind != "edge_guards");
        let claimed_pre = verb_only_registries(&regs, &roster_pre_956);
        for t in ["edge_costs", "edge_guards"] {
            assert!(
                claimed_pre.contains(&t),
                "off the roster, `{t}` must be claimed verb-only — otherwise the difference is \
                 not what decides and this is a hand list wearing a derivation"
            );
            assert!(
                !claimed.contains(&t),
                "on the roster today, `{t}` must NOT be claimed verb-only — this is the state \
                 that put these tables out of a file-only authoring's reach (R956)"
            );
        }

        // The rendered paragraph must EMBED the derived list, not merely compute
        // it — the Round 956 failure shape of a value set once and never read.
        let wire = verb_only_wire(&regs, &roster_now);
        for name in &claimed {
            assert!(
                wire.contains(&format!("`{name}`")),
                "derived table `{name}` never reaches the rendered paragraph"
            );
        }
        let wire_pre = verb_only_wire(&regs, &roster_pre_956);
        assert!(
            wire_pre.contains("`edge_costs`") && wire_pre.contains("`edge_guards`"),
            "the pre-956 rendering must name the tables its roster cannot reach"
        );
        assert_ne!(
            wire, wire_pre,
            "the paragraph must differ between the two rosters, or nothing here is derived"
        );

        // Exactly one statement of the consequence: a second one is a second
        // home for one datum, and the section HEADING that carried one asserted
        // `verb-only` for a whole round after it stopped being true.
        assert_eq!(
            wire.matches("SILENT NO-OP").count(),
            1,
            "the consequence belongs in the derived claim and nowhere else"
        );
    }

    /// Round 912 — the `evidence` ordering rule and the field that actually
    /// carries "the reader finds out later", both pinned to prose an author meets
    /// where they are looking.
    ///
    /// R910's two blind authors produced 43 `evidence_unreachable` findings
    /// between them by citing scenes LATER than `canon_from`. The check is right
    /// — `evidence` is defined as a backreference (R522) — but the contract said
    /// so nowhere an author would look: the field description said "provenance",
    /// the invariant said "must name an existing section", and the ordering rule
    /// appeared once, as a clause inside a paragraph about forks and sibling
    /// world-lines. Measured: zero occurrences of at-or-before / prior /
    /// backreference / forward reference in the whole rendered contract.
    #[test]
    fn evidence_says_it_must_be_prior_and_points_at_the_field_that_is_not() {
        let c = describe_schema();
        let ev = c
            .fact
            .fields
            .iter()
            .find(|f| f.name == "evidence")
            .expect("evidence field present");
        let prose = format!("{} {}", ev.ty, ev.description);

        assert!(
            prose.contains("AT OR BEFORE") || prose.contains("at or before"),
            "the ordering requirement belongs in the field's own description"
        );
        assert!(
            prose.contains("evidence_unreachable"),
            "name the finding the author will be handed"
        );
        // The sharp case: the extent does not widen the window. A fact holding
        // sc-01..sc-05 still may not cite sc-03.
        assert!(
            prose.contains("canon_to") && prose.contains("DOES NOT WIDEN"),
            "a reader who knows about canon_to will assume the span is the window; \
             it is compared against canon_from alone"
        );
        // And the redirection, which is the half that makes the rule survivable.
        assert!(
            prose.contains("first_at"),
            "an author who wanted `where the reader finds out` must be sent to the \
             field that means that, or they will come back to this one"
        );

        // The disclosure surface has to accept that traffic: it described itself
        // as a secrecy device only, so a non-secret late reveal looked out of scope.
        let d = c.disclosure_encoding;
        assert!(
            d.contains("WHEN THE READER LEARNS A FACT"),
            "the general axis must be stated before the secrecy idiom"
        );
        assert!(
            d.contains("canon_from") && d.contains("evidence"),
            "the three-way distinction (true / established / discovered) is the \
             confusion this fixes, so all three have to appear"
        );
    }

    /// Round 907 — the interval `bound` prose is pinned to what the REAL rules
    /// parser accepts, not to a word list.
    ///
    /// The rule-class parameter said the bound was "a literal number, or a third
    /// scalar predicate id". Both halves were wrong and each was wrong in a
    /// different way: a bare number is a PARSE ERROR (the wire is a tagged
    /// object), and `scalar` is the `object_kind` R708 removed — so the more
    /// prominent of the contract's two descriptions of one field contradicted
    /// the other and used dead vocabulary while doing it. An author following it
    /// writes `"bound": 5` and is refused at load.
    ///
    /// No key-set or vocabulary-alternation oracle reaches this: `scalar` was an
    /// ADJECTIVE, not an enumerated value, and "a literal number" names no
    /// identifier at all. What IS decidable is the behaviour, so the test runs
    /// both spellings through the parser and holds the prose to the outcome.
    #[test]
    fn interval_bound_prose_matches_what_the_rules_parser_accepts() {
        // Through the real public entry point, on a real file — the same call
        // `validate-continuity` makes.
        fn parse(bound: &str) -> Result<(), String> {
            let json = format!(
                r#"{{"rules":[{{"id":"r","predicate":"p","class":"interval",
                   "right":"q","op":"ge","bound":{bound}}}]}}"#
            );
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().join("rules.json");
            std::fs::write(&path, json).expect("write rules");
            crate::continuity::load_narrative_rules(&path, None).map(|_| ())
        }
        // The discriminating pair — without both halves the assertion is vacuous.
        assert!(
            parse("5").is_err(),
            "a bare-number bound must be refused; if this ever parses, the prose \
             below is what needs changing, not this test"
        );
        assert!(
            parse(r#"{"const":5}"#).is_ok(),
            "the tagged const bound is the documented spelling and must parse"
        );
        assert!(
            parse(r#"{"predicate":"min-gap"}"#).is_ok(),
            "the tagged predicate bound is the documented spelling and must parse"
        );

        let c = describe_schema();
        let bound = &c
            .narrative_rules
            .iter()
            .find(|r| r.class == "interval")
            .expect("interval rule class present")
            .parameters
            .iter()
            .find(|p| p.name == "bound")
            .expect("bound parameter present");
        let prose = format!("{} {}", bound.ty, bound.description);
        // Both spellings the parser accepts must be named where an author looks.
        for tag in ["\"const\"", "\"predicate\""] {
            assert!(
                prose.contains(tag),
                "the bound parameter does not name the {tag} wire tag, which is one of \
                 the only two shapes the parser accepts"
            );
        }
        // And the shape it refuses must not be offered, in either surface.
        for surface in [prose.as_str(), c.narrative_rules_wire] {
            assert!(
                !surface.contains("a literal number,"),
                "the contract still offers a bare-number bound, which is a parse error"
            );
            assert!(
                !surface.contains("scalar"),
                "`scalar` is the object_kind R708 removed; it must not appear in the \
                 authoring contract"
            );
        }
    }

    /// Round 906 — the ROSTER guard, derived from the type. The hand-written
    /// sample below can only check the kinds it happens to build; nothing asked
    /// whether the contract MENTIONS every kind at all. `units` was added to
    /// `FactsManifest` and this contract went on advertising "seven" arrays for
    /// thirteen rounds — two blind authors (R904) read the short roster and both
    /// concluded, in a sealed report, that a Quantity could not be authored from
    /// a file. The oracle is the real serde key set, so a kind cannot be added
    /// without being described.
    #[test]
    fn manifest_kinds_cover_every_facts_manifest_array() {
        use std::collections::BTreeSet;
        // Every `FactsManifest` field is `#[serde(default)]`, so an empty object
        // parses and re-serializes to exactly the manifest's array names.
        let empty: mnemosyne_atomic::FactsManifest = serde_json::from_str("{}")
            .expect("every FactsManifest field is #[serde(default)] — the documented leniency");
        let arrays: BTreeSet<String> = serde_json::to_value(empty)
            .expect("FactsManifest serializes")
            .as_object()
            .expect("a struct serializes to an object")
            .keys()
            .cloned()
            .collect();

        let wire = describe_schema().manifest_wire;
        let described: BTreeSet<String> = wire.kinds.iter().map(|k| k.kind.to_string()).collect();
        assert_eq!(
            arrays, described,
            "describe-schema's kind roster drifted from FactsManifest's real arrays — every array \
             an author may write needs a KindWire, and a KindWire for an array that no longer \
             exists sends them at a shape the parser will ignore"
        );

        // The overview is GENERATED from that roster, so the count word and the
        // ordered list follow it. Assert the generation, not a copy of the list.
        let overview = &wire.overview;
        for name in &arrays {
            assert!(
                overview.contains(name.as_str()),
                "manifest overview does not name the `{name}` array"
            );
        }
        assert!(
            overview.contains(&format!("with {} optional arrays", arrays.len())),
            "the overview must state the REAL array count ({}) — it said `seven` while the type \
             held eight",
            arrays.len()
        );
        // The leniency that makes the roster load-bearing (see the test below).
        assert!(
            overview.contains("UNKNOWN KEYS ARE IGNORED"),
            "an author cannot discover a misspelled kind from the parser, so the contract must say \
             the parse is silent"
        );
    }

    /// Round 906 — the manifest's unknown-key tolerance is INTENDED, not a bug,
    /// and this pins it as behaviour rather than leaving it as prose. It is the
    /// reason the roster above is the only way to learn a kind exists: a correct
    /// guess and a typo produce byte-identical results at the parse (R904 gap 3).
    /// `evidence_replay_smoke::classify` depends on this to separate "does the
    /// file parse" from "does the file build anything".
    #[test]
    fn manifest_tolerates_unknown_keys_and_builds_nothing() {
        let typo = r#"{ "unti": [ { "unit_id": "minute" } ], "no_such_kind": 3 }"#;
        let m: mnemosyne_atomic::FactsManifest = serde_json::from_str(typo)
            .expect("unknown keys are ignored — the documented, load-bearing leniency");
        // Parsed cleanly AND built nothing: the two questions the contract must
        // keep separate.
        assert!(m.units.is_empty(), "a misspelled kind builds no rows");
        assert!(m.frames.is_empty() && m.facts.is_empty() && m.entities.is_empty());
        // The correctly-spelled key is what differs — the discriminating input
        // this pair exists to hold (without it the assertion above is vacuous).
        let correct = r#"{ "units": [ { "unit_id": "minute" } ] }"#;
        let m: mnemosyne_atomic::FactsManifest =
            serde_json::from_str(correct).expect("the real key parses");
        assert_eq!(m.units.len(), 1, "the spelling is the ONLY difference");
    }

    /// Round 906 — the SECTIONS wire is a serialization contract, so its prose is
    /// reflection-pinned to `SectionImport` (the `manifest_wire_prose_names_every_serde_key`
    /// discipline). Until this round the contract described the sections manifest
    /// NOWHERE — `parent_doc` appeared zero times — and two blind authors (R904)
    /// independently guessed an object with a `sections` key, both rejected by a
    /// loader that wants a bare array. A guess that two isolated readers make the
    /// same way is a contract gap, not two mistakes.
    #[test]
    fn sections_wire_prose_names_every_serde_key() {
        fn assert_documented(value: &serde_json::Value, prose: &str) {
            match value {
                serde_json::Value::Object(map) => {
                    for (k, v) in map {
                        assert!(
                            prose.contains(&format!("\"{k}\"")),
                            "SectionImport serde key `{k}` is not named in the sections-wire prose"
                        );
                        assert_documented(v, prose);
                    }
                }
                serde_json::Value::Array(items) => {
                    items.iter().for_each(|v| assert_documented(v, prose))
                }
                _ => {}
            }
        }
        // Fully populated: every optional key present, so none escapes the walk
        // (the empty-array lesson one type over).
        let sample = mnemosyne_atomic::SectionImport {
            section_id: "s".into(),
            parent_doc: "d".into(),
            title: "t".into(),
            parent_section: Some("p".into()),
            normative_excerpt: Some(mnemosyne_atomic::NormativeExcerptImport {
                text: "x".into(),
                anchor_url: "u".into(),
                source_revision: "r".into(),
                text_sha256: "h".into(),
            }),
            coverage_expectation: mnemosyne_core::CoverageExpectation::Informational,
        };
        let prose = describe_schema().sections_wire;
        assert_documented(&serde_json::to_value(&sample).unwrap(), prose);

        // A key-set walk checks that every KEY is named; it says nothing about
        // the VALUES a closed-enum key accepts. The first draft of this prose
        // offered `"normative"|"informative"` — the two-state vocabulary R422
        // removed and R870 had already found hand-retyped in five surfaces — and
        // every key-level assertion above passed on it. So the closed vocabulary
        // is pinned to the enum too.
        for tag in serde_variants::<mnemosyne_core::CoverageExpectation>() {
            assert!(
                prose.contains(&format!("\"{tag}\"")),
                "coverage_expectation value `{tag}` is not named in the sections-wire prose"
            );
        }
        assert!(
            mnemosyne_core::CoverageExpectation::from_tag("informative").is_none(),
            "the prose claims `informative` is refused; hold it to that"
        );

        // The two shape facts a reader cannot get from the key list, and that
        // both blind authors got wrong: it is a BARE ARRAY, and `parent_doc` is
        // required (no default — an omitted one is a parse error).
        assert!(
            prose.contains("BARE JSON ARRAY"),
            "the top-level shape is the guess that failed twice"
        );
        assert!(
            serde_json::from_str::<Vec<mnemosyne_atomic::SectionImport>>(
                r#"[{"section_id":"s","title":"t"}]"#
            )
            .is_err(),
            "parent_doc must really be required — else this prose overstates the loader"
        );
        assert!(
            prose.contains("REQUIRED"),
            "the required-ness of parent_doc must be stated, not implied by ordering"
        );
    }

    /// Round 924 — the contract must tell an author that `undirected` declares
    /// EDGE SYMMETRY, and must not tell them it selects a genre.
    ///
    /// Both surfaces said the true value was "the MAP" and the false value "a
    /// state machine". Two checks read the field that way, and R918 measured what
    /// it cost: the four corpora on record AT THAT TIME all declared
    /// `undirected: false` — because a road per direction is how an author says a
    /// way costs more upward than downward — so four MAPS were exempted from the
    /// connectivity walk and the route count. That count is dated on purpose: it
    /// stopped being true at Round 943 and the undated version of it shipped in
    /// the contract for twenty-five rounds (Round 969). The behaviour is pinned in
    /// `continuity.rs` (`genre_is_not_inferable_from_any_declared_field`); what is
    /// pinned here is that the author is not told the retired story, because the
    /// author's copy is the one that produced the corpora.
    #[test]
    fn undirected_declares_symmetry_and_never_the_genre() {
        let c = describe_schema();
        let undirected = &c
            .narrative_rules
            .iter()
            .find(|r| r.class == "transition")
            .expect("transition rule class present")
            .parameters
            .iter()
            .find(|p| p.name == "undirected")
            .expect("undirected parameter present")
            .description;

        assert!(
            undirected.contains("EDGE SYMMETRY"),
            "say what the field declares"
        );
        assert!(
            undirected.contains("DOES NOT DECLARE WHETHER YOUR RULE IS A MAP OR A LIFECYCLE"),
            "and say what it does not, since the retired gloss is what an author \
             would otherwise infer from `true = undirected`"
        );
        // The retired gloss itself, in BOTH surfaces that carried it — a
        // structured parameter description and the wire prose beside it.
        let retired = ["the undirected MAP", "a state machine like"];
        for surface in [&undirected[..], c.narrative_rules_wire] {
            for phrase in retired {
                assert!(
                    !surface.contains(phrase),
                    "`undirected` is edge symmetry (R924), but the contract still \
                     glosses it as `{phrase}`"
                );
            }
        }
    }

    /// Round 906 — the transition rule's `containment` prose must describe the
    /// R716 model the gate actually enforces, not the R703 one it replaced. The
    /// contract said a container is "a search-key, not a position" that "must not
    /// be walked on" while `scan_spatial_map` had, since R716, treated containment
    /// as a SCOPE BOUNDARY with the container participating as a node in its
    /// parent's scope. Two blind authors (R904) quoted the stale sentence back in
    /// their sealed reports and were rejected by the live gate for obeying it:
    /// 24 of 39 findings were `adjacency_cross_scope`. Tier-3 prose, so this pins
    /// the CONCEPTS and the emitted violation names, never the wording.
    #[test]
    fn transition_containment_documents_the_scope_boundary_model() {
        let c = describe_schema();
        let containment = &c
            .narrative_rules
            .iter()
            .find(|r| r.class == "transition")
            .expect("transition rule class present")
            .parameters
            .iter()
            .find(|p| p.name == "containment")
            .expect("containment parameter present")
            .description;

        // The model: siblings-only edges, and the portal that replaces the
        // forbidden cross-container edge.
        assert!(
            containment.contains("SIBLINGS"),
            "an edge may only join places sharing a direct container"
        );
        assert!(
            containment.contains("PORTAL") || containment.contains("portal"),
            "how a container is left — the half an author cannot infer from the reject"
        );
        assert!(
            containment.contains("NODE in its parent's scope"),
            "the container/node dichotomy dissolving is the R716 correction"
        );
        // The violation an author will actually be handed.
        assert!(
            containment.contains("adjacency_cross_scope"),
            "name the finding, so a rejected author can search for it"
        );
        // Round 911 — and the question two blind authors each asked unprompted:
        // how a subject gets INSIDE. Round 913 changed the ANSWER (the crossing
        // is now a declarable, checked step), so these pins move with it: the
        // rule an author needs, the finding they may be handed, the co-hold that
        // remains legal as the alternative, and the limit that only a DECLARED
        // crossing is checked.
        assert!(
            containment.contains("DEEPEST SCOPE WHERE"),
            "the step rule is the answer to `how do I get inside`; state it"
        );
        assert!(
            containment.contains("descent") && containment.contains("ascent"),
            "an author searching for the crossing rule needs the vocabulary the \
             finding and the docs use"
        );
        // Round 929 — and the CLAIM those two words are attached to, which is the
        // half R917 found unpinned. The guard above asserts the words while its
        // own message named the claim, so keeping both words and inverting the
        // sentence to "STILL REQUIRES an edge between the container and the place
        // inside it" passed the whole suite — and the contract would then be
        // instructing an author to write the exact edge the gate rejects as
        // `adjacency_cross_scope`. Measured by injection, not read off the code.
        //
        // The pin carries the ASCENT clause and not just the trailing phrase, and
        // that is not caution: the first version of this guard pinned "needs no
        // edge at all" alone, and R925's equal-endpoint sentence three lines below
        // ("a chain that ends BACK WHERE IT STARTED needs no edge at all")
        // satisfied it — the inverted crossing rule still passed. A claim pin that
        // another claim can satisfy is a word pin wearing the right message.
        assert!(
            containment.contains("or an ascent (leaving) and needs no edge at all"),
            "entering and leaving need NO edge — the half an author cannot infer, \
             and the half an inverted sentence silently reverses"
        );
        assert!(
            containment.contains("rule_transition_invalid"),
            "the lift is a check: name what an author will be handed when it fails"
        );
        assert!(
            containment.contains("CO-HOLD"),
            "the alternative telling stays legal and stays stated"
        );
        assert!(
            containment.contains("unchained_state_pairs"),
            "the limit is that only a DECLARED crossing is checked; say so"
        );
        // Round 925 — the run rule, pinned as CLAIMS rather than as words. A
        // guard that pins the word `chain` passes on prose that says the
        // opposite; these three fail if the sentence is inverted, dropped or
        // softened, which is the R917 debt-3 lesson about prose guards.
        assert!(
            containment.contains("A CHAIN OF CROSSINGS IS ONE MOVE"),
            "an author who reads that each crossing is judged alone will launder a \
             forbidden step through a container"
        );
        assert!(
            containment.contains("does NOT license it"),
            "the consequence is the whole point: routing through a container licenses nothing"
        );
        assert!(
            containment.contains("BACK WHERE IT STARTED needs no edge at all"),
            "the equal-endpoint half must be stated too, or an author who steps out and back \
             will expect a reject and rewrite correct prose"
        );
        // The superseded model must not be re-stated anywhere in the contract —
        // including Round 911's own answer, which Round 913 falsified. An author
        // told the crossing cannot be a step will not write one. Round 929 — the
        // list itself now lives in ONE place and the repair hint is scanned with
        // it too; see [`SUPERSEDED_CROSSING_TELLINGS`].
        //
        // Round 930 — and the RULE-CLASS description is the third contract
        // surface, which nothing scanned until now. That is not a hypothetical
        // gap: `Succession IS the declared adjacency` was sitting there, R917
        // named it, and adding the phrase to the list above changed NOTHING until
        // this line was added, because the sentence lives on the one surface the
        // scan never read.
        let class_prose = c
            .narrative_rules
            .iter()
            .find(|r| r.class == "transition")
            .expect("transition rule class present")
            .description;
        let surfaces = [c.narrative_rules_wire, containment, class_prose];
        for s in surfaces {
            for phrase in SUPERSEDED_CROSSING_TELLINGS.iter().copied() {
                assert!(
                    !s.contains(phrase),
                    "the R703 grouping model is superseded by R716, but the contract still says \
                     `{phrase}`"
                );
            }
        }
        // Round 934 — the contract used to attribute ALL THREE completeness
        // findings to wiring `containment`, and that misattribution is worse
        // than silence: every one of the six recorded authors DID wire
        // containment, so the three who declared no leg kind had every reason
        // to believe completeness was on. Pin the CLAIM, not a word: a
        // rewording that drops which declaration turns `map_invented_place` on
        // must fail here. The phrase names the adjacency predicate because that
        // is the half the old sentence got wrong — `containment` is right there
        // in the same paragraph and would satisfy any looser pin (the R929
        // trap, where a claim pin rode a substring its own neighbour supplied).
        assert!(
            containment.contains("only your ADJACENCY PREDICATE can say what a place"),
            "the contract must say WHICH declaration turns `map_invented_place` on; \
             attributing it to `containment` is what three of six blind authors read"
        );
        assert!(
            containment.contains("the class emits nothing"),
            "and it must say what happens when neither leg is declared, or an author \
             reads an unaskable class as a clean one"
        );
        assert!(
            c.narrative_rules_wire
                .contains("EXCEPT `map_invented_place`"),
            "the wire states the same exception, or an author who reads only it \
             concludes containment alone turns every completeness check on"
        );
        // The rules-file wire carries the same correction (the same fact is
        // described in two places; they may not disagree).
        assert!(
            c.narrative_rules_wire.contains("adjacency_cross_scope"),
            "the rules-wire prose must name the same finding as the rule-class prose"
        );
        // Round 930 — and the STEP side, which the wire described only for EDGES.
        // An author reading the wire alone learned that containment partitions the
        // map and nothing about crossings, so the edge rule ("an adjacency edge
        // may only join SIBLINGS") was the whole of what they knew — which is
        // exactly the R911 dead end R913 falsified, reachable by reading one
        // surface instead of the other.
        assert!(
            c.narrative_rules_wire
                .contains("crossing and needs NO EDGE AT ALL"),
            "the wire must carry the crossing rule too, or an author who reads only \
             it concludes a crossing has to be an edge"
        );
        assert!(
            c.narrative_rules_wire
                .contains("CHAIN of crossings is ONE move"),
            "and the run rule, for the same reason the rule-class prose carries it"
        );
        // The class prose and the wire must agree that a succession is not itself
        // the map relation — the conflation R930 removed, pinned as the claim so
        // it cannot come back in other words.
        assert!(
            class_prose.contains("Succession says WHICH pairs are steps")
                && class_prose.contains("not the same relation"),
            "succession supplies the pairs and the map licenses them; a contract \
             that says they are one relation tells an author a crossing must be an \
             edge: {class_prose}"
        );
    }

    /// Round 600 (session review, Findings 1 + 3): extend the drift guard from
    /// the worked example to the KEY PROSE. Every JSON key the serializer emits
    /// for any manifest kind — including the nested confluence / typed / surface
    /// shapes — must be NAMED (quoted) in the wire prose, and the canon-order
    /// prose must name the real `CanonOrderFile` keys. Before this, keys present
    /// only in prose (`converges_from`, `canon_to`, `surface`, …) and never in
    /// the guarded example could be renamed in serde without breaking a test,
    /// leaving `describe-schema` to hand an agent a stale wire contract.
    #[test]
    fn manifest_wire_prose_names_every_serde_key() {
        // Every OBJECT key in `value` (recursively) must appear quoted in `prose`.
        fn assert_documented(value: &serde_json::Value, prose: &str) {
            match value {
                serde_json::Value::Object(map) => {
                    for (k, v) in map {
                        assert!(
                            prose.contains(&format!("\"{k}\"")),
                            "serde key `{k}` is not named in the wire prose"
                        );
                        assert_documented(v, prose);
                    }
                }
                serde_json::Value::Array(items) => {
                    items.iter().for_each(|v| assert_documented(v, prose))
                }
                _ => {}
            }
        }

        let w = describe_schema().manifest_wire;
        let mut prose = String::from(w.typed_object_wire);
        for k in &w.kinds {
            prose.push_str(k.json_keys);
        }

        // A fully-populated manifest exercising every optional key + nested shape
        // (the shape only — serde does not validate ids, so these need not be a
        // valid store).
        let manifest = mnemosyne_atomic::FactsManifest {
            edge_costs: vec![mnemosyne_atomic::EdgeCostImport {
                fact_id: "f".into(),
                n: 1,
                unit: "u".into(),
            }],
            edge_guards: vec![mnemosyne_atomic::EdgeGuardImport {
                fact_id: "f".into(),
                conditions: vec!["f0".into()],
                threshold: Some(1),
            }],
            frames: vec![mnemosyne_atomic::FrameImport {
                frame_id: "gt".into(),
                description: "d".into(),
            }],
            branches: vec![mnemosyne_atomic::BranchImport {
                branch_id: "b".into(),
                description: "d".into(),
                forks_from: Some("main".into()),
                forks_at: Some("s".into()),
                converges_from: vec![mnemosyne_atomic::BranchConvergeImport {
                    branch: "b".into(),
                    at: "s".into(),
                }],
            }],
            entity_kinds: vec![mnemosyne_atomic::EntityKindImport {
                kind_id: "character".into(),
                parents: vec![],
                description: "d".into(),
            }],
            units: vec![mnemosyne_atomic::UnitImport {
                unit_id: "minute".into(),
                description: "d".into(),
            }],
            entities: vec![mnemosyne_atomic::EntityImport {
                entity_id: "e".into(),
                kind: "character".into(),
                description: "d".into(),
            }],
            predicates: vec![mnemosyne_atomic::PredicateImport {
                predicate_id: "p".into(),
                object_kind: mnemosyne_core::PredicateObjectKind::Token,
                subject_kind: None,
                object_entity_kind: None,
                object_tokens: vec!["v".into()],
                description: "d".into(),
            }],
            facts: vec![mnemosyne_atomic::FactImport {
                fact_id: "f".into(),
                frame: "gt".into(),
                branch: Some("b".into()),
                entities: vec!["e".into()],
                claim: "c".into(),
                canon_from: "s".into(),
                canon_to: Some("s".into()),
                evidence: vec!["s".into()],
                conflicts_with: vec!["f0".into()],
                supersedes_in_frame: Some("f0".into()),
                payoff_expectation: Some("expected".into()),
                pays_off: vec!["f0".into()],
                typed: Some(mnemosyne_core::TypedClaim {
                    subject: "e".into(),
                    predicate: "p".into(),
                    object: mnemosyne_core::TypedObject::Token { token: "v".into() },
                }),
                quote: Some("q".into()),
            }],
            disclosure_plans: vec![mnemosyne_atomic::DisclosurePlanImport {
                telling_id: "t".into(),
                default_mode: Some("withhold".into()),
                description: "d".into(),
                overrides: vec![mnemosyne_atomic::DisclosureOverrideImport {
                    fact_id: "f".into(),
                    mode: "state".into(),
                    // Round 752 — exercise every reveal key (branch/coords/
                    // threshold) so the wire-prose drift guard checks them all.
                    first_at: vec![mnemosyne_atomic::DisclosureRevealImport {
                        branch: "b".into(),
                        coords: vec!["s".into(), "s2".into()],
                        threshold: Some(2),
                    }],
                    surface: Some(mnemosyne_atomic::DisclosureSurfaceImport {
                        scene: "s".into(),
                        object: Some("o".into()),
                    }),
                }],
            }],
        };
        // Round 906 — the sample must POPULATE every array before its element
        // keys can be checked, and that is exactly where this guard went quiet:
        // `units` joined `FactsManifest`, the struct literal below stopped
        // compiling, and the repair was `units: vec![]` — which recurses into
        // nothing and asks nothing. An empty array is not a checked array, so
        // failing here is the only way an added kind reaches the prose.
        let value = serde_json::to_value(&manifest).unwrap();
        for (kind, arr) in value.as_object().unwrap() {
            assert!(
                arr.as_array().is_some_and(|a| !a.is_empty()),
                "the wire-prose sample leaves `{kind}` EMPTY, so no key of it is checked — \
                 populate it here (an empty array silently exempts a whole kind)"
            );
            assert_documented(arr, &prose);
        }

        // Finding 3: the canon-order prose names the real `CanonOrderFile`
        // STRUCTURAL keys. `branches` is a data-keyed map (its keys are branch
        // ids, not field names), so check only the top-level fields — an empty
        // map avoids recursing into data keys.
        let order = crate::continuity::CanonOrderFile {
            edges: vec![["a".to_string(), "b".to_string()]],
            branches: std::collections::BTreeMap::new(),
            ..Default::default()
        };
        let canon = describe_schema().canon_order;
        for key in serde_json::to_value(&order)
            .unwrap()
            .as_object()
            .unwrap()
            .keys()
        {
            assert!(
                canon.contains(&format!("\"{key}\"")),
                "canon-order structural key `{key}` is not named in the canon-order prose"
            );
        }
    }

    /// Every `exclusive_key` gloss names BOTH legs, in the enum's own words.
    ///
    /// The failure this exists for is not a missing sentence but a role word
    /// standing in for a leg. Until Round 965 the `object` gloss read "at most
    /// one HOLDER per OBJECT (one holder per thing)": it named the leg it keys
    /// on and described the other leg by the role it plays when the predicate
    /// happens to be written holder-first. An author who writes the same
    /// relation the other way round — `held_by(thing, holder)` instead of
    /// `holds(holder, thing)` — reads that sentence, agrees with it, and gets
    /// the inverted rule. Measured over every exclusive rule on record: the
    /// predicate `held_by` appears with `per: subject` in one corpus and
    /// `per: object` in another, and both name the rule one-holder-per-thing.
    ///
    /// So the checkable property is that each gloss names both legs, and the
    /// leg names are not typed here — they ARE the variants, read off the enum.
    #[test]
    fn every_exclusive_key_gloss_names_both_legs() {
        let keys = super::exclusive_key_values();
        assert_eq!(
            keys.len(),
            2,
            "this check reads `both legs` as `every variant`, which is only the \
             right rule while `ExclusiveKey` is a pair. A third key needs the \
             property restated, not this assertion relaxed."
        );
        for key in &keys {
            let text = key.description.to_lowercase();
            for other in &keys {
                assert!(
                    text.contains(other.value),
                    "the `{}` gloss never names the `{}` leg, so it can only be \
                     describing that leg by the ROLE it plays under one \
                     predicate ordering — the exact reading that put `held_by` \
                     on record with opposite `per` values in two corpora. \
                     Gloss: {:?}",
                    key.value,
                    other.value,
                    key.description
                );
            }
        }
    }
}
