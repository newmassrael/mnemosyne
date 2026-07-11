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
//! DRIFT GUARD: the fixed vocabularies are built from the real core enums
//! ([`DisclosureMode`] etc.) via `as_str()` + an exhaustive `match` for the
//! per-value gloss, and the rule classes from the real
//! [`crate::continuity::RuleClass`]. Adding an enum variant / rule class breaks
//! the exhaustive match here (a compile error at the description site), so the
//! contract cannot silently fall behind the code. The quest-convention ids are
//! the same `pub(crate)` constants the projection reads, single-sourced.

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
    }
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
                EITHER a fork (forks_from a parent at a canon point, inheriting its prefix) XOR \
                a confluence (converges_from >= 2 parents at their merge points) — a forest by \
                construction.",
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
    }
}
