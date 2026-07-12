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
    RuleClass, QUEST_ENTITY_KIND, QUEST_PRED_COMPLETED_BY, QUEST_PRED_PURSUES, QUEST_PRED_REQUIRES,
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
    /// enum tagging (incl. the `scalar` object_kind → `value` wire tag), the
    /// `first_at` tuple shape — plus a complete worked example. Without this an
    /// agent must reverse-engineer the serializer from parse errors.
    pub manifest_wire: ManifestWireSpec,
    /// The canon ORDER a store needs to be RENDERABLE (Round 596,
    /// unattended-loop-experiment/v1 Finding 4) — a SEPARATE authoring artifact,
    /// NOT part of the fact manifest, that the read projections require. Without
    /// it `report-playthrough-manuscript` / `report-fork-tree` place nothing and
    /// the store is not playable; `report-authoring-frontier` surfaces every
    /// fact-bearing scene the order does not cover as an `unordered scenes` gap.
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
    /// The typed leg's object enum wire tagging — INCLUDING the naming quirk
    /// that a `scalar` predicate object_kind serializes with the tag `value`.
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
    /// The two object shapes (from [`PredicateObjectKind`]), each with its rule.
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
    /// The expected object shape of this predicate's typed leg.
    pub object_shape: &'static str,
}

/// The quest authoring convention (R559) — a projection over existing
/// primitives, NOT new substrate: an author adopts these ids so the quest-graph
/// projection can read their store.
#[derive(Debug, Clone, Serialize)]
pub struct QuestEncoding {
    pub description: &'static str,
    /// The reserved `Entity.kind` value the quest-graph projection recognizes
    /// (kind is otherwise consumer-defined per ARCHITECTURE sec 6 inv4).
    pub entity_kind: &'static str,
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
            enforced: entity kinds and scalar values are consumer vocabulary (sec 6 inv4).",
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
             order declared, ALL of them), and the store cannot be rendered.",
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
             \"interval\", plus that class's legs: exclusive → \"per\": \"subject\" | \"object\"; \
             transition → \"allowed\": [ [from, to], … ] (scalar value pairs); interval → \
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
             loud, not silent.",
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
        overview: "A JSON object with six optional arrays applied in this order in ONE atomic \
             transaction: frames, branches, entities, predicates, facts, disclosure_plans. Later \
             kinds may reference earlier ones (a fact names a frame/branch/entity/section; a \
             disclosure override names a fact), so order matters — registries first, then facts, \
             then disclosure. Any array may be omitted (defaults to empty).",
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
                kind: "entities",
                json_keys: "{ \"entity_id\": string, \"kind\"?: string (free-form; the one \
                    reserved value is \"quest\"), \"description\"?: string }",
            },
            KindWire {
                kind: "predicates",
                json_keys: "{ \"predicate_id\": string, \"object_kind\": \"entity\"|\"scalar\", \
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
             id, \"object\": <tagged enum> }. The object is an INTERNALLY-TAGGED enum with two \
             variants: for a predicate whose object_kind is `entity`, write { \"kind\": \
             \"entity\", \"id\": entity id }; for a predicate whose object_kind is `scalar`, \
             write { \"kind\": \"value\", \"value\": string }. NOTE the deliberate naming: the \
             predicate's object_kind is spelled `scalar`, but the object's wire tag for that \
             shape is `value` (the object_KIND vs the runtime object shape) — write `value`, not \
             `scalar`. The subject and any entity-shaped object must ALSO appear in the fact's \
             `entities` list.",
        example_json: MANIFEST_EXAMPLE_JSON,
    }
}

/// A complete, valid `import-facts` manifest — the copy-and-adapt template
/// (Round 595). Exercises every kind and the load-bearing serialization quirks:
/// a fork branch (`forks_from` string), a scalar typed object (`kind`:`value`),
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
    { "predicate_id": "state", "object_kind": "scalar", "description": "an item's state" }
  ],
  "facts": [
    {
      "fact_id": "f-setup", "frame": "ground-truth",
      "claim": "the relic lies in the vault", "canon_from": "sc-01",
      "evidence": ["sc-01"], "entities": ["e-relic"],
      "payoff_expectation": "expected",
      "typed": { "subject": "e-relic", "predicate": "state",
                 "object": { "kind": "value", "value": "hidden" } }
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
                EITHER a fork (forks_from a parent at a canon point, inheriting its prefix) XOR \
                a confluence (converges_from >= 2 parents at their merge points) — a forest by \
                construction. FORK-LINEAGE TRAP (Round 601, the dangling two independent loop \
                agents hit): a fork inherits the parent's prefix, so a pre-fork trunk setup is \
                `in` every fork's world-line — but the BARE parent (no fork continuing it) stays \
                its OWN world-line, a DEAD PREFIX that still carries those trunk setups. Forking \
                BOTH roads off `main` and never continuing bare `main` leaves `main` a dead \
                prefix whose trunk `expected` setups have no payoff THERE and dangle (surfaced \
                per-world by `report-payoff-coverage` / `report-authoring-frontier`). Continue \
                `main` AS one of the roads (fork only the OTHER off it), or pay the trunk setups \
                off before the fork — do not leave a bare pre-fork trunk carrying live setups.",
        },
        RegistrySpec {
            name: "entities",
            key: "entity id",
            referenced_by: "NarrativeFact.entities[] + TypedClaim.subject / entity-shaped object",
            add_op: "add-entity",
            load_bearing: false,
            description: "The retrieval key for entity-scoped verification (all facts about X — \
                a character, location, item, faction). `Entity.kind` is a FREE-FORM \
                consumer-defined tag (nothing medium-shaped enforced, sec 6 inv4); the ONE \
                reserved value is `quest` (see quest_encoding).",
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
                shape (entity | scalar), enforced on every typed leg.",
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
    ]
}

fn disclosure_mode_values() -> Vec<EnumValue> {
    // Exhaustive `match` — an added DisclosureMode variant fails to compile HERE
    // (and in the variant array below), the single-source drift guard.
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
    [
        DisclosureMode::Withhold,
        DisclosureMode::State,
        DisclosureMode::Hint,
        DisclosureMode::Imply,
    ]
    .into_iter()
    .map(|m| EnumValue {
        value: m.as_str(),
        description: gloss(m),
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
    [PayoffExpectation::Unmarked, PayoffExpectation::Expected]
        .into_iter()
        .map(|p| EnumValue {
            value: p.as_str(),
            description: gloss(p),
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
            PredicateObjectKind::Scalar => {
                "the object leg is an opaque consumer-vocabulary \
                value string (`alive`, `undead`) — never enumerated by the substrate"
            }
        }
    }
    [PredicateObjectKind::Entity, PredicateObjectKind::Scalar]
        .into_iter()
        .map(|k| EnumValue {
            value: k.as_str(),
            description: gloss(k),
        })
        .collect()
}

fn rule_class_specs() -> Vec<RuleClassSpec> {
    // Exhaustive `match` over the real RuleClass — an added class fails to
    // compile HERE, so a new rule can never go undescribed.
    fn spec(c: RuleClass) -> RuleClassSpec {
        match c {
            RuleClass::Exclusive => RuleClassSpec {
                class: "exclusive",
                description: "At most one co-holding value per subject (`per: subject` — \
                    location exclusivity) or one holder per object (`per: object` — \
                    conservation/custody) within one (frame x world). Overlapping typed legs \
                    that violate this are a continuity-gate reject.",
                parameters: vec![FieldSpec {
                    name: "per",
                    ty: "`subject` | `object`",
                    required: true,
                    description: "Which typed leg the rule keys on.",
                }],
            },
            RuleClass::Transition => RuleClassSpec {
                class: "transition",
                description: "Rides the in-frame succession edge: a successor and predecessor \
                    both typed with the same subject+predicate must form a declared `(from, \
                    to)` transition. Succession IS the declared adjacency; unchained \
                    same-subject pairs are surfaced, never gated.",
                parameters: vec![FieldSpec {
                    name: "allowed",
                    ty: "[from, to][] (scalar value pairs)",
                    required: true,
                    description: "The permitted state transitions.",
                }],
            },
            RuleClass::Interval => RuleClassSpec {
                class: "interval",
                description: "A scalar/arithmetic relation over numeric typed legs, same \
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
    [
        RuleClass::Exclusive,
        RuleClass::Transition,
        RuleClass::Interval,
    ]
    .into_iter()
    .map(spec)
    .collect()
}

fn quest_encoding() -> QuestEncoding {
    QuestEncoding {
        description: "A quest is the NARRATIVE instance of the substrate's universal \
            tracked-obligation pattern, PROJECTED from existing primitives — no new substrate. \
            An author adopts these reserved ids so `report-quest-graph` can read the store; the \
            projection derives per-world open/done, prerequisites, and giver locators.",
        entity_kind: QUEST_ENTITY_KIND,
        predicates: vec![
            QuestPredicate {
                predicate: QUEST_PRED_PURSUES,
                role: "an actor entity (subject) LEADS the quest (object) — the quest's actors.",
                object_shape: "entity (the quest)",
            },
            QuestPredicate {
                predicate: QUEST_PRED_REQUIRES,
                role: "a quest (subject) is gated by another quest (object) that must complete \
                    first — the declarative prerequisite; the canon order proves the timing.",
                object_shape: "entity (the prerequisite quest)",
            },
            QuestPredicate {
                predicate: QUEST_PRED_COMPLETED_BY,
                role: "a quest (subject) is DISCHARGED by an actor (object) on a road — the \
                    carrying fact also `pays_off` the quest's giving setup.",
                object_shape: "entity or scalar value (the discharger)",
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
                (entity vs scalar); a scalar value must be non-empty.",
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
                plan by typed tuple, so a decision on an untyped fact is un-gateable.",
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
        let dm: Vec<_> = vocab("disclosure_mode")
            .values
            .iter()
            .map(|v| v.value)
            .collect();
        assert_eq!(dm, ["withhold", "state", "hint", "imply"]);
        assert_eq!(vocab("disclosure_mode").default, Some("withhold"));
        let pe: Vec<_> = vocab("payoff_expectation")
            .values
            .iter()
            .map(|v| v.value)
            .collect();
        assert_eq!(pe, ["unmarked", "expected"]);
        let pok: Vec<_> = vocab("predicate_object_kind")
            .values
            .iter()
            .map(|v| v.value)
            .collect();
        assert_eq!(pok, ["entity", "scalar"]);

        // Every enum value carries a non-empty gloss.
        for v in &c.vocabularies {
            for val in &v.values {
                assert!(!val.description.is_empty(), "empty gloss on {}", val.value);
            }
        }

        // The three rule classes are described.
        let classes: Vec<_> = c.narrative_rules.iter().map(|r| r.class).collect();
        assert_eq!(classes, ["exclusive", "transition", "interval"]);

        // The quest ids are the real projection constants (single-sourced).
        assert_eq!(c.quest_encoding.entity_kind, QUEST_ENTITY_KIND);
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
        // the scalar typed object serializes with the tag `value` (the quirk).
        let setup = &m.facts[0];
        assert_eq!(setup.payoff_expectation.as_deref(), Some("expected"));
        match &setup.typed.as_ref().expect("setup has a typed leg").object {
            mnemosyne_core::TypedObject::Value { value } => assert_eq!(value, "hidden"),
            other => panic!("scalar object must be the Value variant, got {other:?}"),
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
            entities: vec![mnemosyne_atomic::EntityImport {
                entity_id: "e".into(),
                kind: "character".into(),
                description: "d".into(),
            }],
            predicates: vec![mnemosyne_atomic::PredicateImport {
                predicate_id: "p".into(),
                object_kind: "scalar".into(),
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
                    object: mnemosyne_core::TypedObject::Value { value: "v".into() },
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
