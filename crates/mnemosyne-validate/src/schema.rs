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

use mnemosyne_core::{DisclosureMode, PayoffExpectation, PredicateObjectKind};
use serde::Serialize;

use crate::continuity::{
    ExclusiveKey, IntervalOp, RuleClass, QUEST_PRED_COMPLETED_BY, QUEST_PRED_PURSUES,
    QUEST_PRED_REQUIRES,
};

/// The complete medium-neutral authoring contract (R587). Every field is a
/// static description of the substrate's shape, not any store's contents.
#[derive(Debug, Clone, Serialize)]
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
#[derive(Debug, Clone, Serialize)]
pub struct ManifestWireSpec {
    /// The batch verbs this manifest is fed to.
    pub add_op: &'static str,
    /// The top-level object shape and the order kinds are applied in.
    pub overview: &'static str,
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
pub struct KindWire {
    /// The manifest array this describes (`frames` / `branches` / …).
    pub kind: &'static str,
    /// The serialized object's key names + shapes (the wire form, not prose).
    pub json_keys: &'static str,
}

/// One registry: an id space that must be populated before a fact references it.
#[derive(Debug, Clone, Serialize)]
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
pub struct FieldSpec {
    pub name: &'static str,
    /// A description of the field's type (not a Rust path — an authoring hint).
    pub ty: &'static str,
    pub required: bool,
    pub description: &'static str,
}

/// The narrative-fact shape.
#[derive(Debug, Clone, Serialize)]
pub struct FactSpec {
    pub description: &'static str,
    pub add_op: &'static str,
    pub fields: Vec<FieldSpec>,
}

/// The typed-claim (subject–predicate–object) contract.
#[derive(Debug, Clone, Serialize)]
pub struct TypedClaimSpec {
    pub description: &'static str,
    pub subject: &'static str,
    pub predicate: &'static str,
    /// The object shapes (from [`PredicateObjectKind`]), each with its rule.
    pub object_shapes: Vec<EnumValue>,
}

/// One value of a fixed vocabulary (a closed enum variant).
#[derive(Debug, Clone, Serialize)]
pub struct EnumValue {
    pub value: &'static str,
    pub description: &'static str,
}

/// A fixed, substrate-defined vocabulary — a closed enum an author picks from.
#[derive(Debug, Clone, Serialize)]
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
pub struct RuleClassSpec {
    pub class: &'static str,
    pub description: &'static str,
    pub parameters: Vec<FieldSpec>,
}

/// One typed predicate reserved by the quest convention.
#[derive(Debug, Clone, Serialize)]
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
pub struct Invariant {
    pub name: &'static str,
    pub rule: &'static str,
    /// Where the check fires.
    pub enforced_at: &'static str,
}

/// Build the authoring contract (R587). Pure — no store, no order, no I/O.
pub fn describe_schema() -> SchemaContract {
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
        registries: registries(),
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
        manifest_wire: manifest_wire(),
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
            "Encoding a per-ROAD secret without leaking it — the `withhold` + `first_at` reveal \
             idiom. A telling's disclosure `mode` is world-INDEPENDENT (one decision per fact × \
             telling); only `first_at` is per-world. So a fact set to `state`/`hint`/`imply` is \
             disclosed on EVERY world-line — reaching for `state` to reveal a secret on one road \
             LEAKS it on the others. To reveal a fact on the reveal-road yet keep it hidden \
             elsewhere, leave the mode `withhold` and pin `first_at` for THAT road only (e.g. \
             `first_at: [[reveal-road, reveal-scene]]`); a road with no pin stays withheld. The \
             secrecy is the withhold default; the reveal is the per-world timing pin — never a \
             non-withhold mode. IMPORTANT: a clean `report-authoring-frontier` does NOT certify a \
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
             (true = an edge admits both directions, the map; absent/false = one-way, a state \
             machine like `alive → dead`) + \"containment\"?: <predicate id> (Round 703: its \
             facts are `contains(region, node)` — turns on the G2 completeness/leak checks over \
             containers; omit for a map with no containers); interval → \
             \"right\": <predicate id>, \"op\": \"ge\"|\"le\"|\"eq\"|\"gt\"|\"lt\", \"bound\": { \
             \"const\": number } | { \"predicate\": <predicate id> } (a TAGGED object, never a \
             bare number). The parser is fail-loud on unknown or class-mismatched legs (a \
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
/// is a list of `[branch, section]` pairs).
fn manifest_wire() -> ManifestWireSpec {
    ManifestWireSpec {
        add_op: "import-facts (apply) / propose-verdict (dry-run gate) — both read this manifest",
        overview: "A JSON object with seven optional arrays applied in this order in ONE atomic \
             transaction: frames, branches, entity_kinds, entities, predicates, facts, \
             disclosure_plans. Later kinds may reference earlier ones (an entity names an \
             entity_kind; a fact names a frame/branch/entity/section; a disclosure override names \
             a fact), so order matters — registries first, then facts, then disclosure. Any array \
             may be omitted (defaults to empty).",
        kinds: vec![
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
                json_keys: "{ \"kind_id\": string, \"description\"?: string } — the consumer's \
                    entity-kind vocabulary (character/place/item/quest/…); members are the \
                    consumer's, never core's",
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
                kind: "disclosure_plans",
                json_keys: "{ \"telling_id\": string, \"default_mode\"?: \
                    \"withhold\"|\"state\"|\"hint\"|\"imply\" (omitted = withhold), \
                    \"description\"?: string, \"overrides\"?: [ { \"fact_id\": string, \"mode\": \
                    string, \"first_at\"?: [ [branch id, section id], … ] (a list of 2-element \
                    [branch, section] arrays), \"surface\"?: {\"scene\": string, \"object\"?: \
                    string} } ] }",
            },
        ],
        typed_object_wire:
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
             entity-shaped object must ALSO appear in the fact's `entities` list.",
        example_json: MANIFEST_EXAMPLE_JSON,
    }
}

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
        { "fact_id": "f-setup", "mode": "state", "first_at": [ ["road-b", "sc-04"] ] }
      ]
    }
  ]
}"#;

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
            referenced_by: "keyed BY the adjacent fact; read by the future derived travel-time \
                computation (tide_budget − Σcost) — not referenced by any other row",
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
                ty: "section id[] (>= 1)",
                required: true,
                description: "Structure sections evidencing the claim. At least one — a claim \
                    without provenance is unauditable.",
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
                "at most one co-holding value per SUBJECT (location exclusivity: \
                one place per person)"
            }
            ExclusiveKey::Object => {
                "at most one holder per OBJECT (conservation/custody: one \
                holder per thing)"
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
                description: "At most one co-holding value per subject (`per: subject` — \
                    location exclusivity) or one holder per object (`per: object` — \
                    conservation/custody) within one (frame x world). Overlapping typed legs \
                    that violate this are a continuity-gate reject.",
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
                    both typed with the same subject+predicate must form an adjacent `(from, \
                    to)` step. Succession IS the declared adjacency; unchained \
                    same-subject pairs are surfaced, never gated.",
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
                        description: "Round 697: edge symmetry. true = an `adjacent(a, b)` fact \
                            admits BOTH (a, b) and (b, a) — the undirected MAP, so one fact per \
                            edge is the SSOT. Absent/false = one-way, a state machine (`alive → \
                            dead` must not admit the reverse).",
                    },
                    FieldSpec {
                        name: "containment",
                        ty: "predicate id (optional)",
                        required: false,
                        description: "Round 703 (store-native map): the predicate whose facts are \
                            `contains(region, node)` — a region (a container: a search-key, not a \
                            position) and the map nodes it holds. Wiring it turns on the G2 \
                            completeness/leak invariant: every place-kind entity must be a node or \
                            a container; a container must not be walked on; a region contains only \
                            real nodes. Omit for a map with no containers.",
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
                        ty: "a literal number, or a third scalar predicate id",
                        required: true,
                        description: "The right-hand bound of the comparison.",
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
                name an existing section (a claim without provenance is unauditable).",
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

#[cfg(test)]
mod tests {
    use super::*;

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
    /// disclosure-encoding idiom names the `withhold`+`first_at` reveal and the
    /// frontier-is-not-the-leak-gate caveat; the `branches` registry names the
    /// dead-prefix dangling trap. Prose (tier-3, not serde-guarded), so this
    /// pins the concepts an agent must find, not the exact wording.
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
        // the disclosure override's first_at is a [branch, section] pair.
        assert_eq!(m.disclosure_plans.len(), 1);
        let overrides = &m.disclosure_plans[0].overrides;
        assert_eq!(overrides.len(), 1);
        assert_eq!(overrides[0].fact_id, "f-setup");
        assert_eq!(
            overrides[0].first_at,
            vec![["road-b".to_string(), "sc-04".to_string()]]
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
                description: "d".into(),
            }],
            units: vec![],
            entities: vec![mnemosyne_atomic::EntityImport {
                entity_id: "e".into(),
                kind: "character".into(),
                description: "d".into(),
            }],
            predicates: vec![mnemosyne_atomic::PredicateImport {
                predicate_id: "p".into(),
                object_kind: "token".into(),
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
                    first_at: vec![["b".into(), "s".into()]],
                    surface: Some(mnemosyne_atomic::DisclosureSurfaceImport {
                        scene: "s".into(),
                        object: Some("o".into()),
                    }),
                }],
            }],
        };
        // Recurse into each kind's array (the six top-level array names are the
        // well-known kinds, documented in the overview + FACTS_MANIFEST_SHAPE).
        let value = serde_json::to_value(&manifest).unwrap();
        for arr in value.as_object().unwrap().values() {
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
}
