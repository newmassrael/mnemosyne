//! Round 835/836 predecessor — Round 834's design, built: every place a `String`
//! can live under `AtomicStore`, at ANY depth, must be classified by which gate
//! covers it.
//!
//! # Why this exists
//!
//! `every_side_table_detector_is_wired_into_the_aggregate` already forces a
//! human to classify a new field — by destructuring `AtomicStore` with no `..`,
//! so adding a field stops the build. Round 833 found what that cannot see:
//! `AtomicSection::ladder` (Round 765) carried four registry refs NESTED inside
//! an existing field, so no top-level field was ever added, the destructure
//! never stopped compiling, and the guard reported complete for sixty-eight
//! rounds. A completeness guard that watches one depth reports "complete" about
//! the depth it does not watch.
//!
//! # Why the obvious fix is not the fix
//!
//! "Just recurse the types" is dead on measurement, not on argument. Round 834
//! measured 52 reachable types carrying 91 String-typed fields, and WHICH are
//! registry refs is not written in any type: `SectionLadder::carrier` and
//! `AtomicSection::intent` are both `Option<String>`, one an entity ref and one
//! prose; `LadderRung::needs` and `AtomicSection::rationale_bullets` are both
//! `Vec<String>`, one a list of fact ids and one a list of sentences. No
//! mechanical walk can decide. A human must, once per field — and this makes the
//! moment of deciding unavoidable instead of invisible.
//!
//! # Why the classification is a PATH, not a boolean
//!
//! Coverage is not single-source. `AtomicSection::superseded_by` is guarded, but
//! by neither the write path nor a `*_violations` detector: `project.rs` emits it
//! as a cross-ref and the orphan scan resolves it (measured — a section
//! superseded by a phantom id exits 1 with `atomic orphan new`). A table that
//! only knew about detectors would demand a redundant one. So each field records
//! WHICH gate covers it.
//!
//! # What this does NOT do
//!
//! It catches an UNCLASSIFIED field, never a MISCLASSIFIED one. A field recorded
//! as `NotARef` that later becomes a ref by convention is still silent. The
//! difference from Round 833 is that a line already exists for someone to
//! change; that defect had no such line anywhere.

use std::collections::{BTreeMap, BTreeSet};

/// Which gate covers a String-bearing field — the question the table answers.
///
/// Round 834's design assumed TWO paths. Filling the table found FIVE, which is
/// the strongest argument for having built it: a classification that offered
/// only "detector or not a ref" would have forced three quarters of these fields
/// into a wrong answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Coverage {
    /// Re-checked at the scan boundary by a `*_violations` detector wired into
    /// `store_registry_violations` — the baseline gate.
    Detector,
    /// Projected as a cross-ref by `project.rs` and resolved by the orphan scan
    /// — also the baseline gate, by a different route. `superseded_by` is here,
    /// measured: a section superseded by a phantom id exits 1 with
    /// `atomic orphan new`.
    Projection,
    /// Checked by `scan_continuity` — a DIFFERENT command (`validate-continuity`)
    /// from the baseline gate, so a workspace that runs only the baseline does
    /// not get these. `pays_off`, `conflicts_with` and `supersedes_in_frame` are
    /// here, each with its own violation variant.
    Continuity,
    /// A mirror of an audit field whose drift is gated by the publishable/audit
    /// divergence check, not by a ref scan. The value is a copy, so its
    /// referential integrity is the audit field's.
    Divergence,
    /// The write path validates it and NO re-check was found by inspection.
    ///
    /// This is the Round 833 class, recorded rather than papered over: an
    /// out-of-band edit to one of these is invisible to every gate. Each is a
    /// candidate for a boundary twin, and listing them is what turns this table
    /// from a formality into an inventory. "No re-check found" is a statement
    /// about a reading, not a proof — confirming each is follow-on work.
    WritePathOnly,
    /// Not a registry ref. The reason is the payload: it is what a future reader
    /// re-examines when the field's meaning drifts.
    NotARef,
}

/// The sources the type graph is walked over. `include_str!` rather than a
/// runtime read, so a moved or renamed file is a COMPILE error — the first
/// attempt at this measurement read two `lib.rs` files, silently saw a third of
/// the graph, and reported the missing types as external primitives.
const SOURCES: &[(&str, &str)] = &[
    ("atomic/lib.rs", include_str!("../src/lib.rs")),
    (
        "core/lib.rs",
        include_str!("../../mnemosyne-core/src/lib.rs"),
    ),
    (
        "core/narrative.rs",
        include_str!("../../mnemosyne-core/src/narrative.rs"),
    ),
    (
        "core/fact.rs",
        include_str!("../../mnemosyne-core/src/fact.rs"),
    ),
    (
        "core/content_anchor.rs",
        include_str!("../../mnemosyne-core/src/content_anchor.rs"),
    ),
    (
        "core/scene.rs",
        include_str!("../../mnemosyne-core/src/scene.rs"),
    ),
    (
        "core/section_ref.rs",
        include_str!("../../mnemosyne-core/src/section_ref.rs"),
    ),
];

/// The registry id types (Round 838/839). A field of one of these is a ref whose
/// TARGET REGISTRY the compiler now holds, so it can no longer be misclassified
/// as to ref-ness — but WHICH gate checks it is still a human answer, so it stays
/// in the table. Dropping a field from here the moment it gains a type would
/// trade one blind spot for another.
///
/// This list grows by one line per migration round; the endgame is that it
/// replaces the `String` scan entirely and `NotARef` disappears.
const REF_ID_TYPES: &[&str] = &[
    "UnitId",
    "ParameterId",
    "PredicateId",
    "EntityKindId",
    "FrameId",
    "EntityId",
];

/// Container names that are never a type to walk into.
const CONTAINERS: &[&str] = &[
    "String", "Vec", "Option", "BTreeMap", "BTreeSet", "HashMap", "HashSet", "Cow", "Box",
];

/// One type's body: the `(place, type-text)` pairs it declares.
///
/// `place` is a struct field name, an enum struct-variant field name, or a
/// tuple-variant name. Enums are walked too, because they carry refs —
/// `TypedObject::Entity { id }` is an entity id and `Locator::Prefix(String)` is
/// not, which is the same undecidability one level in.
fn type_bodies() -> BTreeMap<String, Vec<(String, String)>> {
    let mut out: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for (_label, src) in SOURCES {
        let lines: Vec<&str> = src.lines().collect();
        let mut i = 0;
        while i < lines.len() {
            let line = lines[i].trim_end();
            let name = line
                .strip_prefix("pub struct ")
                .or_else(|| line.strip_prefix("pub enum "))
                .filter(|rest| rest.ends_with('{'))
                .map(|rest| rest.trim_end_matches('{').trim().to_string());
            let Some(name) = name else {
                i += 1;
                continue;
            };
            let mut body: Vec<(String, String)> = Vec::new();
            i += 1;
            // A type block ends at the first column-0 `}` — nested braces are
            // indented, the same shape the detector-wiring tripwire relies on.
            while i < lines.len() && !lines[i].starts_with('}') {
                let t = lines[i].trim();
                if !t.starts_with("//") && !t.starts_with("#[") {
                    // Order matters: an inline struct-variant (`Entity { id:
                    // String },`) also contains a `:` and would otherwise be
                    // read as one malformed field and dropped. It was, on the
                    // first run — `TypedObject::Entity.id` is an entity ref and
                    // went missing, which is this file's own failure mode
                    // appearing inside the file that exists to prevent it.
                    let inline = inline_struct_variant(t);
                    if !inline.is_empty() {
                        body.extend(inline);
                    } else if let Some((place, ty)) = named_field(t) {
                        body.push((place, ty));
                    } else if let Some((place, ty)) = tuple_variant(t) {
                        body.push((place, ty));
                    }
                }
                i += 1;
            }
            out.insert(name, body);
        }
    }
    out
}

/// `Variant { a: T, b: U },` — an enum struct-variant written on one line, whose
/// fields are keyed `Variant.a`. Empty when the line is not one.
fn inline_struct_variant(t: &str) -> Vec<(String, String)> {
    let Some((head, rest)) = t.split_once('{') else {
        return Vec::new();
    };
    let head = head.trim();
    if head.is_empty() || !head.starts_with(|c: char| c.is_ascii_uppercase()) {
        return Vec::new();
    }
    let Some((inner, _)) = rest.rsplit_once('}') else {
        return Vec::new();
    };
    inner
        .split(',')
        .filter_map(|part| named_field(part.trim()))
        .map(|(f, ty)| (format!("{head}.{f}"), ty))
        .collect()
}

/// `pub field: Type,` (struct) or `field: Type,` (enum struct-variant).
fn named_field(t: &str) -> Option<(String, String)> {
    let t = t.strip_prefix("pub ").unwrap_or(t);
    let (place, ty) = t.split_once(':')?;
    let place = place.trim();
    if place.is_empty() || !place.chars().all(|c| c.is_ascii_lowercase() || c == '_') {
        return None;
    }
    Some((
        place.to_string(),
        ty.trim().trim_end_matches(',').to_string(),
    ))
}

/// `Variant(Type),` — a tuple variant, keyed by the variant name.
fn tuple_variant(t: &str) -> Option<(String, String)> {
    let (place, rest) = t.split_once('(')?;
    let place = place.trim();
    if place.is_empty() || !place.starts_with(|c: char| c.is_ascii_uppercase()) {
        return None;
    }
    let ty = rest.rsplit_once(')')?.0;
    Some((place.to_string(), ty.to_string()))
}

/// Type names mentioned in a type-text, minus the containers.
fn referenced(ty: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut cur = String::new();
    for c in ty.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            cur.push(c);
        } else {
            if cur.starts_with(|c: char| c.is_ascii_uppercase()) && !CONTAINERS.contains(&&*cur) {
                out.insert(cur.clone());
            }
            cur.clear();
        }
    }
    if cur.starts_with(|c: char| c.is_ascii_uppercase()) && !CONTAINERS.contains(&&*cur) {
        out.insert(cur);
    }
    out
}

/// Every `(Type, place)` reachable from `AtomicStore` whose declared type
/// mentions `String`, at any depth.
fn derived_pairs() -> BTreeSet<(String, String)> {
    let bodies = type_bodies();
    let root = bodies
        .get("AtomicStore")
        .expect("AtomicStore must parse — the walk has no root otherwise");
    let mut frontier: Vec<String> = root
        .iter()
        .flat_map(|(_, ty)| referenced(ty))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut pairs: BTreeSet<(String, String)> = BTreeSet::new();
    while let Some(ty_name) = frontier.pop() {
        if !seen.insert(ty_name.clone()) {
            continue;
        }
        let Some(body) = bodies.get(&ty_name) else {
            continue; // not one of ours (an external or primitive type)
        };
        for (place, ty) in body {
            // A String place, OR a place already migrated to a ref id type —
            // both must stay classified by covering gate.
            if ty.contains("String") || REF_ID_TYPES.iter().any(|t| ty.contains(t)) {
                pairs.insert((ty_name.clone(), place.clone()));
            }
            for next in referenced(ty) {
                if !seen.contains(&next) {
                    frontier.push(next);
                }
            }
        }
    }
    pairs
}

/// THE TABLE. One line per String-bearing place reachable from the store,
/// naming the gate that covers it — or, for `NotARef`, why it is not one.
///
/// Bootstrapped from the deriver rather than by hand: Round 834 hand-listed 26
/// types and got 68 pairs where the derivation finds 52 and 91, missing a
/// quarter INCLUDING `TypedClaim` and `EvidenceRef`. A hand list restated the
/// tree and drifted from it inside the round about hand lists drifting, so this
/// table was filled in from the failure message this test prints.
#[rustfmt::skip]
const CLASSIFIED: &[(&str, &str, Coverage, &str)] = &[
    ("AtomicChangelogEntry", "carry_forward_bullets", Coverage::NotARef, "audit prose"),
    ("AtomicChangelogEntry", "changes_bullets", Coverage::NotARef, "audit prose"),
    ("AtomicChangelogEntry", "decision_summary", Coverage::NotARef, "audit prose"),
    ("AtomicChangelogEntry", "impact_refs", Coverage::Projection, "section refs - validate_atomic_store orphan_entry_refs"),
    ("AtomicChangelogEntry", "publishable_carry_forward_bullets", Coverage::Divergence, "mirror of the audit field"),
    ("AtomicChangelogEntry", "publishable_changes_bullets", Coverage::Divergence, "mirror of the audit field"),
    ("AtomicChangelogEntry", "publishable_decision_summary", Coverage::Divergence, "mirror of the audit field"),
    ("AtomicChangelogEntry", "publishable_impact_refs", Coverage::Divergence, "mirror of the audit field's refs"),
    ("AtomicChangelogEntry", "publishable_verification_bullets", Coverage::Divergence, "mirror of the audit field"),
    ("AtomicChangelogEntry", "verification_bullets", Coverage::NotARef, "audit prose"),
    ("AtomicSection", "caveats_bullets", Coverage::NotARef, "authored prose"),
    ("AtomicSection", "impact_scope", Coverage::Projection, "section refs, projected as cross-refs"),
    ("AtomicSection", "inputs_bullets", Coverage::NotARef, "authored prose"),
    ("AtomicSection", "intent", Coverage::NotARef, "authored prose"),
    ("AtomicSection", "outputs_bullets", Coverage::NotARef, "authored prose"),
    ("AtomicSection", "rationale_bullets", Coverage::NotARef, "authored prose"),
    ("AtomicSection", "resolved_by", Coverage::Projection, "section ref, projected as a cross-ref"),
    ("AtomicSection", "superseded_by", Coverage::Projection, "section ref, projected as a cross-ref (measured: exits 1)"),
    ("Binding", "file", Coverage::NotARef, "a workspace-relative path"),
    ("Binding", "symbol", Coverage::NotARef, "a code symbol name"),
    ("Branch", "description", Coverage::NotARef, "authored prose (the choice label)"),
    ("BranchFork", "at", Coverage::Detector, "section ref - branch_ref_violations"),
    ("BranchFork", "branch", Coverage::Detector, "branch ref - branch_ref_violations"),
    ("ConfirmationClaim", "SectionCompleteness.section_id", Coverage::WritePathOnly, "section ref; no re-check found"),
    ("ConfirmationClaim", "file", Coverage::NotARef, "a workspace-relative path"),
    ("ConfirmationClaim", "section_id", Coverage::WritePathOnly, "section ref; no re-check found"),
    ("ConfirmationClaim", "symbol", Coverage::NotARef, "a code symbol name"),
    ("ConfirmationEvent", "authoring_run", Coverage::NotARef, "an opaque run identifier"),
    ("ConfirmationEvent", "confirming_run", Coverage::NotARef, "an opaque run identifier"),
    ("ConfirmationEvent", "rationale", Coverage::NotARef, "authored prose"),
    ("ConfirmationEvent", "timestamp", Coverage::NotARef, "an ISO timestamp"),
    ("Confirmer", "id", Coverage::NotARef, "the confirming tool's identity"),
    ("Confirmer", "version", Coverage::NotARef, "the confirming tool's version"),
    ("ConflictAssertion", "target", Coverage::Continuity, "fact ref - ConflictTargetMissing"),
    ("ContentAnchor", "source", Coverage::NotARef, "a document name, not a registry key"),
    ("ContentExcerpt", "text", Coverage::NotARef, "projected prose"),
    ("DisclosureOverride", "first_at", Coverage::Detector, "branch keys - disclosure_ref_violations"),
    ("DisclosurePlan", "description", Coverage::NotARef, "authored prose"),
    ("DisclosurePlan", "overrides", Coverage::Detector, "fact-id keys - disclosure_ref_violations"),
    ("DisclosureReveal", "coords", Coverage::Detector, "section refs - disclosure_ref_violations"),
    ("DisclosureSurface", "object", Coverage::Detector, "EntityId (R844) - disclosure_ref_violations"),
    ("DisclosureSurface", "scene", Coverage::Detector, "section ref - disclosure_ref_violations"),
    ("EdgeCost", "unit", Coverage::Detector, "UnitId (R839) - edge_cost_violations"),
    ("EdgeGuard", "conditions", Coverage::Detector, "fact refs - edge_guard_violations"),
    ("Entity", "description", Coverage::NotARef, "authored prose"),
    ("Entity", "kind", Coverage::Detector, "entity-kind ref - unregistered_entity_kinds"),
    ("EntityKind", "description", Coverage::NotARef, "authored prose"),
    ("EntityKind", "parents", Coverage::Detector, "entity-kind refs - entity_kind_parent_violations"),
    ("EpubLocator", "cfi", Coverage::NotARef, "an EPUB coordinate"),
    ("EpubLocator", "fragment", Coverage::NotARef, "an EPUB coordinate"),
    ("EpubLocator", "spine_href", Coverage::NotARef, "an EPUB spine path"),
    ("EvidenceRef", "section", Coverage::Detector, "section ref - fact_registry_refs Evidence facet"),
    ("ExampleBlock", "code", Coverage::NotARef, "authored content"),
    ("ExampleBlock", "language", Coverage::NotARef, "a language tag"),
    ("Frame", "description", Coverage::NotARef, "authored prose"),
    ("InventoryEntry", "reason", Coverage::NotARef, "authored prose"),
    ("InventoryEntry", "section_ref", Coverage::WritePathOnly, "section ref; no re-check found"),
    ("InventoryEntry", "source", Coverage::NotARef, "the declaring artifact"),
    ("LadderRung", "needs", Coverage::Detector, "fact refs - ladder_ref_violations (R833)"),
    ("LadderRung", "object", Coverage::Detector, "EntityId (R844) - ladder_ref_violations (R833)"),
    ("LadderRung", "reveals", Coverage::Detector, "fact refs - ladder_ref_violations (R833)"),
    ("Locator", "Cfi", Coverage::NotARef, "coordinate text"),
    ("Locator", "Prefix", Coverage::NotARef, "coordinate text"),
    ("NarrativeFact", "branch", Coverage::Detector, "branch ref - fact_registry_refs Branch facet"),
    ("NarrativeFact", "canon_from", Coverage::Detector, "section ref - fact_registry_refs CanonFrom facet"),
    ("NarrativeFact", "canon_to", Coverage::Detector, "section ref - fact_registry_refs CanonTo facet"),
    ("NarrativeFact", "claim", Coverage::NotARef, "the authored assertion itself"),
    ("NarrativeFact", "entities", Coverage::Detector, "EntityId (R844) - fact_registry_refs Entity facet"),
    ("NarrativeFact", "frame", Coverage::Detector, "FrameId (R843) - fact_registry_refs Frame facet"),
    ("NarrativeFact", "pays_off", Coverage::Continuity, "fact refs - PayoffTargetMissing"),
    ("NarrativeFact", "quote", Coverage::NotARef, "authored prose"),
    ("NarrativeFact", "supersedes_in_frame", Coverage::Continuity, "fact ref - SuccessionTargetMissing"),
    ("NormativeExcerpt", "anchor_url", Coverage::NotARef, "upstream provenance, not a store key"),
    ("NormativeExcerpt", "source_revision", Coverage::NotARef, "upstream provenance, not a store key"),
    ("Parameter", "description", Coverage::NotARef, "authored prose"),
    ("ParameterGate", "parameter", Coverage::Detector, "ParameterId (R839) - parameter_gate_violations"),
    ("Predicate", "description", Coverage::NotARef, "authored prose"),
    ("Predicate", "object_entity_kind", Coverage::Detector, "entity-kind ref - predicate_kind_ref_violations"),
    ("Predicate", "object_tokens", Coverage::NotARef, "the declared vocabulary, not a ref into one"),
    ("Predicate", "subject_kind", Coverage::Detector, "entity-kind ref - predicate_kind_ref_violations"),
    ("RejectedAlternative", "alternative", Coverage::NotARef, "authored prose"),
    ("RejectedAlternative", "reason", Coverage::NotARef, "authored prose"),
    ("ScenePresence", "entity", Coverage::WritePathOnly, "EntityId (R844); no re-check found"),
    ("SectionLadder", "carrier", Coverage::Detector, "EntityId (R844) - ladder_ref_violations (R833)"),
    ("SectionSkeleton", "parent_doc", Coverage::NotARef, "a document label, not a registry key"),
    ("SectionSkeleton", "parent_section", Coverage::WritePathOnly, "section ref; no re-check found"),
    ("SectionSkeleton", "title", Coverage::NotARef, "authored prose"),
    ("TypedClaim", "predicate", Coverage::Detector, "predicate ref - TypedPredicate facet"),
    ("TypedClaim", "subject", Coverage::Detector, "EntityId (R844) - TypedSubject facet"),
    ("TypedObject", "Entity.id", Coverage::Detector, "EntityId (R844) - TypedObject facet"),
    ("TypedObject", "Fact.id", Coverage::WritePathOnly, "phase-2 fact ref (store union staged); excluded from the facets"),
    ("TypedObject", "Quantity.unit", Coverage::Detector, "UnitId (R839) - TypedUnit facet"),
    ("TypedObject", "Token.token", Coverage::WritePathOnly, "checked against the predicate's declared tokens; excluded from the facets"),
    ("Unit", "description", Coverage::NotARef, "authored prose"),
];

#[test]
fn every_string_field_under_the_store_is_classified() {
    let derived = derived_pairs();

    // NON-VACUITY FLOORS. A parser that quietly stops matching reports an empty
    // derivation, and an empty derivation is a subset of any table — it would
    // pass while checking nothing. The floors are set below Round 834's measured
    // 52 types / 91 pairs so ordinary growth does not trip them, and far above
    // zero so a broken parse cannot.
    assert!(
        derived.len() >= 80,
        "the derivation found only {} String-bearing places; Round 834 measured 91, \
         so the parser has stopped matching rather than the tree having shrunk",
        derived.len()
    );
    let types: BTreeSet<&str> = derived.iter().map(|(t, _)| t.as_str()).collect();
    assert!(
        types.len() >= 20,
        "only {} types carry String places — the walk is not reaching the graph",
        types.len()
    );

    let classified: BTreeSet<(String, String)> = CLASSIFIED
        .iter()
        .map(|(t, f, _, _)| ((*t).to_string(), (*f).to_string()))
        .collect();

    // Unclassified: a String place exists that nobody has decided about. This is
    // the Round 833 case — a ref-bearing field arriving at a depth no guard
    // watched.
    let unclassified: Vec<&(String, String)> = derived
        .iter()
        .filter(|p| !classified.contains(*p))
        .collect();
    assert!(
        unclassified.is_empty(),
        "{} String place(s) under AtomicStore are unclassified. Add each to \
         CLASSIFIED with the gate that covers it (Detector / Projection) or with \
         NotARef and the reason:\n{}",
        unclassified.len(),
        unclassified
            .iter()
            .map(|(t, f)| format!("    (\"{t}\", \"{f}\", Coverage::???, \"why\"),"))
            .collect::<Vec<_>>()
            .join("\n")
    );

    // Stale: a table entry for a place that no longer exists. The Round 783 rule
    // — a declared exclusion matching nothing is folklore, and folklore is what
    // this table would otherwise decay into.
    let stale: Vec<&(String, String)> = classified
        .iter()
        .filter(|p| !derived.contains(*p))
        .collect();
    assert!(
        stale.is_empty(),
        "{} CLASSIFIED entry(ies) name a place that no longer exists — delete them:\n{}",
        stale.len(),
        stale
            .iter()
            .map(|(t, f)| format!("    {t}.{f}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// The table's own shape: no duplicate keys, and every `NotARef` carries a
/// reason. A blank reason is the classification that will be re-derived wrongly
/// by the next reader, which is the failure this whole file exists to prevent.
#[test]
fn the_classification_table_is_well_formed() {
    let mut seen: BTreeSet<(&str, &str)> = BTreeSet::new();
    for (t, f, cov, why) in CLASSIFIED {
        assert!(
            seen.insert((t, f)),
            "{t}.{f} is classified twice — two answers for one place"
        );
        if *cov == Coverage::NotARef {
            assert!(
                !why.trim().is_empty(),
                "{t}.{f} is declared NotARef with no reason; the reason IS the entry"
            );
        }
    }
}
