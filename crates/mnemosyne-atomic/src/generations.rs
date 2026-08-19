//! Round 1254 — the schema ladder, as data a program can answer from.
//!
//! Every on-disk shape change this store has made, in order, with what an old
//! store PAYS to cross it and the rewrite `load` runs to get it across.
//!
//! # Why it stopped being prose
//!
//! R1247 put the DISTANCE on every stale-generation refusal: a store that reads
//! as generation 23 against a build at 46 now says so. The distance is an upper
//! bound and not a diagnosis — most of those generations are additive and cost
//! an old store nothing — and the thing that would turn it into a diagnosis was
//! written down in a comment. Three things were true of that comment when this
//! module replaced it, and each is a reason:
//!
//! 1. It had HOLES nothing could see. Generations 18, 23 and 46 appeared
//!    nowhere in the file: `NarrativeFact.payoff_expectation` / `pays_off`
//!    (Round 442), `Branch.converges_from` (Round 532) and
//!    `AtomicStore.mutation_reasons` (Round 1024) each bumped the version and
//!    left no paragraph. `CURRENT_SCHEMA_VERSION` is derived from this table
//!    now, so a bump without a row is a build error rather than a silence.
//! 2. The same fact was written THREE times — here, in `load`'s hand-written
//!    `if on_disk_version < N` chain, and in that chain's own comments. The
//!    chain reads this table now; a row IS the dispatch.
//! 3. `Cost` and the migration are bound to each other at compile time: a row
//!    is [`Cost::Migrated`] exactly when it carries a rewrite. A taxonomy that
//!    could disagree with the code beside it is a taxonomy nobody can act on.
//!
//! # And then the costs were held to a store (Round 1255)
//!
//! Writing them down did not make them true: each was read off that
//! generation's own paragraph, and a row that said `Breaking` about a store this
//! build opens quite happily produces the confident wrong diagnosis the table
//! exists to end. Every row that claims an old store costs something now carries
//! the store that shows it — see [`Probe`] — and the first run of that law found
//! v24 was one of them. Its answer is [`Cost::Gated`], which the table did not
//! have.
//!
//! # What the paragraphs are
//!
//! Each row carries the reasoning that used to sit above `CURRENT_SCHEMA_VERSION`,
//! verbatim. It is audit prose and stays that way — what changed is that the
//! part a program must answer from is no longer only in it.

use serde_json::Value;

use crate::{
    migrate_disclosure_first_at_to_reveal, migrate_entity_kind_parent_to_parents,
    migrate_evidence_to_refs, migrate_normative_excerpt_to_wrapped,
};

/// What a store written before a generation pays to be read by a build after it.
///
/// THE FOUR ARE NOT DEGREES OF ONE THING. They are different answers to "can I
/// work with that file", which is the question a reader looking at a generation
/// distance actually has — and the fourth exists because a probe found that
/// "will not open" and "opens, and then refuses" had been recorded as the same
/// answer (Round 1255).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cost {
    /// It loads. The new field is absent and serde's default fills it, and a
    /// store that never uses the field re-serializes byte-identically.
    Additive,
    /// It loads, and `load` rewrites the raw JSON first. Nothing is asked of a
    /// person; the row carries the rewrite that does it.
    Migrated,
    /// It may REFUSE to load, and the record says what an author must do. Not
    /// "will refuse": several of these are breaking only for a store that
    /// actually carries the retired shape, and the paragraph says which.
    Breaking,
    /// It OPENS, and then the first write is refused until an author does
    /// something. A different answer from all three above, and one this table
    /// did not have until a probe went looking (Round 1255): v24 was recorded
    /// `Breaking` on the strength of its own paragraph, which says an
    /// unregistered entity kind is "a boundary REJECT" — and the boundary is
    /// the WRITE path, so this build opens such a store quite happily. A
    /// diagnosis that told its holder the file would not open would have been
    /// confidently wrong, which is the failure this table exists to end.
    Gated,
}

/// One generation of the on-disk schema.
pub struct Generation {
    /// The version this generation introduces. `from` is `to - 1` — the ladder
    /// is contiguous and a const assertion below holds it that way.
    pub to: u32,
    pub cost: Cost,
    /// The round that made it. Every one of these was verified against the
    /// atomic store before it was written here.
    pub round: u32,
    /// One line, for a reader who wants the list rather than the reasoning.
    pub what: &'static str,
    /// The raw-JSON rewrite `load` runs before the typed parse for a store older
    /// than `to`. `Some` exactly when `cost` is [`Cost::Migrated`].
    pub migrate: Option<fn(&mut Value)>,
    /// What holds this row's `cost` to something — see [`Probe`].
    pub probe: Probe,
}

/// The evidence behind a row's [`Cost`], and the absence of it, said out loud.
///
/// ROUND 1255 — WHAT MAKES `Breaking` A MEASUREMENT RATHER THAN A LABEL. When
/// R1254 built this table the costs were read off each generation's own
/// paragraph, and nothing held them to it. A row that says `Breaking` about a
/// store this build opens quite happily produces a confident wrong diagnosis —
/// which is the failure the table exists to END, one level up. So a breaking row
/// carries the shape it retired, and a law loads it.
// NOT `PartialEq`, and the compiler said why: a variant holds a function
// pointer, and comparing those "does not produce meaningful results since their
// addresses are not guaranteed to be unique". Nothing here compares probes —
// every reader of this type MATCHES on it — so the derive was a claim the type
// could not honour, standing where a reader would take it for one it could.
#[derive(Debug, Clone, Copy)]
pub enum Probe {
    /// Not a breaking generation: there is no refusal to exhibit. What such a
    /// row claims — that an old store simply loads — is held by the law that
    /// loads a bare store at every non-breaking generation.
    NotBreaking,
    /// A PAIR, and the pair is the whole point. `retired` is a store body
    /// carrying the shape this generation removed, and `load` must refuse it;
    /// `control` is the same store with that shape taken out, and `load` must
    /// ACCEPT it. Without the control a refusal proves only that the fixture was
    /// unparseable, which every malformed fixture also proves.
    Pair {
        retired: &'static str,
        control: &'static str,
    },
    /// A [`Cost::Gated`] generation: `opens` is a store this build READS, and
    /// `refused_by` is the write that will not go through until an author
    /// repairs it. Both halves are asserted, because either alone is the
    /// misreading — "it refuses" without the open is what v24 was recorded as,
    /// and "it opens" without the refusal is what `Additive` would have said.
    Boundary {
        opens: &'static str,
        refused_by: fn(&mut crate::AtomicStore, &std::path::Path) -> bool,
    },
    /// A breaking generation with no fixture, and WHY. A hole somebody can read
    /// is one somebody can close; a row silently unprobed is a cost nobody
    /// checked wearing the same face as one that was.
    Unprobed(&'static str),
}

/// v23→v24's boundary: registering an entity under a kind the store's registry
/// does not name.
///
/// A NAMED FUNCTION AND NOT A CLOSURE, because a `const` table can hold a plain
/// `fn` and a closure would have to be built at run time — the same reason
/// `migrate` holds one. It returns whether the write was REFUSED, so the law
/// reads as the claim: the boundary refuses.
fn an_unregistered_kind_is_refused_at_the_write_boundary(
    store: &mut crate::AtomicStore,
    sidecar: &std::path::Path,
) -> bool {
    crate::add_entity(store, sidecar, "e-2", "person", "").is_err()
}

/// THE LADDER. Ordered, contiguous, and the source of both
/// [`CURRENT_SCHEMA_VERSION`] and `load`'s migration dispatch.
pub const GENERATIONS: &[Generation] = &[
    // Schema version 2 (Round 273): Phase 1A entry — adds
    // AtomicStore.inventory_entries.
    Generation {
        to: 2,
        cost: Cost::Additive,
        round: 273,
        what: "adds AtomicStore.inventory_entries",
        migrate: None,
        probe: Probe::NotBreaking,
    },
    // Schema version 3 (Round 287): outline lift — adds AtomicSection.title /
    // .parent_doc / .parent_section (title-from-workspace-pending carry
    // closure). Pre-v3 sections deserialize with empty title/parent_doc +
    // parent_section=None via #[serde(default)]; Phase I backfill migration
    // populates them from workspace markdown-derived Section data.
    Generation {
        to: 3,
        cost: Cost::Additive,
        round: 287,
        what: "adds AtomicSection.title / .parent_doc / .parent_section",
        migrate: None,
        probe: Probe::NotBreaking,
    },
    // Schema version 4 (Round 294): publishable / audit body split on
    // AtomicChangelogEntry. Pre-v4 entries deserialize with empty publishable_*
    // fields via #[serde(default)]; the v3→v4 migration in `AtomicStore::load`
    // clones audit_* into publishable_* per entry so the default render shape
    // stays byte-identical until R295 setters explicitly diverge them.
    //
    // THE REWRITE IS NOT A RAW-JSON ONE and so is not carried here: it runs on
    // the TYPED store after the parse, because it copies field to field rather
    // than reshaping what serde must read. `Cost::Migrated` is about the raw
    // rewrite this table dispatches; this generation still costs a person
    // nothing, which is what `Additive` says.
    Generation {
        to: 4,
        cost: Cost::Additive,
        round: 294,
        what: "publishable / audit body split on AtomicChangelogEntry",
        migrate: None,
        probe: Probe::NotBreaking,
    },
    // Schema version 5 (Round 387): Path B `Section.implementations[] = {file,
    // symbol}` became `Section.bindings[] = {file, symbol, kind}` (typed
    // trace-link edge; BindingKind = Implements | References). Unlike v3→v4 (a
    // *content transform* that must run imperatively in `load`), v4→v5 is a pure
    // field-rename + new-field-default, which serde expresses idiomatically and
    // declaratively — so there is deliberately NO `schema_version < 5` arm in
    // `load`: the renamed `bindings` field reads the old `implementations` JSON
    // key via #[serde(alias)] and each legacy binding (no `kind` on disk)
    // defaults to `Implements` via #[serde(default)]. Behavior-preserving,
    // because coverage counted every binding before the split. The inferred
    // defaults are NOT silently blessed: `AtomicStore::kind_migration_report`
    // (surfaced by the CLI `report-binding-migration` verb) lists them while
    // `schema_version < 5`, i.e. before the first save bumps the version.
    Generation {
        to: 5,
        cost: Cost::Additive,
        round: 387,
        what: "Section.implementations[] became Section.bindings[] with a typed kind",
        migrate: None,
        probe: Probe::NotBreaking,
    },
    // v5→v6 (Round 389) adds `AtomicSection.coverage_expectation` (Normative |
    // Informative). Like v4→v5 (and unlike v3→v4's content transform), this is a
    // pure new-field default, expressed declaratively: a pre-v6 store has no
    // `coverage_expectation` key, so serde `#[serde(default)]` fills `Normative`
    // — which preserves the Round 269 coverage axiom exactly (every section
    // expected coverage before the split). So there is deliberately NO
    // `schema_version < 6` arm in `load`. The default is the conservative no-op,
    // not a silently-blessed claim, so it needs no migration report (contrast
    // v4→v5's `kind = Implements` default, which was a reviewable claim surfaced
    // by `report-binding-migration`).
    Generation {
        to: 6,
        cost: Cost::Additive,
        round: 389,
        what: "adds AtomicSection.coverage_expectation",
        migrate: None,
        probe: Probe::NotBreaking,
    },
    // v6→v7 adds `AtomicSection.epub_locator` (EPUB-SSOT pointer, R393). Same
    // declarative new-field-default pattern: a pre-v7 store has no
    // `epub_locator` key, serde `#[serde(default)]` fills `None` (no EPUB
    // mirrored) — byte-identical on disk, no behavior change. So there is
    // deliberately NO `schema_version < 7` arm in `load`. The locator is a
    // derived pointer (set by `import-epub-anchors`), not an authored value, so
    // no migration report is needed.
    Generation {
        to: 7,
        cost: Cost::Additive,
        round: 393,
        what: "adds AtomicSection.epub_locator",
        migrate: None,
        probe: Probe::NotBreaking,
    },
    // v7→v8 adds `NormativeExcerpt.text_sha256` (R402). Same declarative
    // new-field-default pattern: a pre-v8 excerpt has no `text_sha256` key,
    // serde `#[serde(default)]` fills "" — byte-identical behavior, no
    // `schema_version < 8` arm. Unlike `epub_locator`, an empty hash IS a
    // reviewable gap (the excerpt's `text` is not yet revalidatable against an
    // EPUB), so it is surfaced by `excerpt_hash_backfill_report` /
    // `report-excerpt-hash-backfill` — a schema-independent work-list (the gap
    // persists across saves until the excerpt is re-imported from an EPUB via
    // `import_epub_excerpts`).
    Generation {
        to: 8,
        cost: Cost::Additive,
        round: 402,
        what: "adds NormativeExcerpt.text_sha256",
        migrate: None,
        probe: Probe::NotBreaking,
    },
    // v8→v9 adds `AtomicSection.verification_expectation` (Dedicated |
    // ByConstruction, R413). Same declarative new-field-default pattern as v5→v6
    // coverage_expectation: a pre-v9 store has no `verification_expectation`
    // key, serde `#[serde(default)]` fills `Dedicated` — but because the
    // VerificationMissing gate is OFF unless `severity_verification` is
    // explicitly configured, an unclassified store gates identically to before
    // (no verify violations). So there is deliberately NO `schema_version < 9`
    // arm in `load`, and no migration report is needed.
    Generation {
        to: 9,
        cost: Cost::Additive,
        round: 413,
        what: "adds AtomicSection.verification_expectation",
        migrate: None,
        probe: Probe::NotBreaking,
    },
    // v9→v10 adds `AtomicStore.confirmation_events` (max-rigor confirmation
    // subsystem, R416) — a top-level append-only collection mirroring
    // `changelog_entries`. Same declarative new-field-default pattern: a pre-v10
    // store has no `confirmation_events` key, serde `#[serde(default)]` fills an
    // empty map — no behavior change (nothing reads the events until the R418
    // predicate / R419 gate land, and that gate is opt-in). So there is
    // deliberately NO `schema_version < 10` arm in `load`, and no migration
    // report is needed.
    Generation {
        to: 10,
        cost: Cost::Additive,
        round: 416,
        what: "adds AtomicStore.confirmation_events",
        migrate: None,
        probe: Probe::NotBreaking,
    },
    // v10→v11 widens `AtomicSection.coverage_expectation` from 2-state
    // (Normative | Informative) to 3-state (Normative | OutOfScopeHere |
    // Informational, R421). The `informative` alias was REMOVED (R422 clean
    // break): a store still carrying that tag fails to load LOUDLY (an unknown
    // enum tag errors — no silent drop), so a consumer migrates `informative` →
    // `out_of_scope_here` deliberately before bumping. New 3-state stores gate
    // identically to the old 2-state (both OutOfScopeHere and Informational
    // leave the coverage axiom, exactly as Informative did). SCE's 50
    // `informative` sections migrate this way on rev bump.
    Generation {
        to: 11,
        cost: Cost::Breaking,
        round: 421,
        what: "coverage_expectation widens to 3 states and the `informative` tag is retired",
        migrate: None,
        probe: Probe::Pair {
            retired: r#"{"sections": {"s-1": {"skeleton": {}, "coverage_expectation": "informative"}}}"#,
            control: r#"{"sections": {"s-1": {"skeleton": {}, "coverage_expectation": "informational"}}}"#,
        },
    },
    // v11→v12 adds `AtomicStore.frames` + `AtomicStore.narrative_facts` (Phase
    // 1A narrative fact entity, Round 430) — two top-level collections mirroring
    // the v9→v10 confirmation_events placement. Same declarative
    // new-field-default pattern: a pre-v12 store has no `frames` /
    // `narrative_facts` keys, serde `#[serde(default)]` fills empty maps — no
    // behavior change (nothing reads them until the continuity gate lands, and
    // that gate is opt-in). So there is deliberately NO `schema_version < 12`
    // arm in `load`, and no migration report is needed.
    Generation {
        to: 12,
        cost: Cost::Additive,
        round: 430,
        what: "adds AtomicStore.frames + AtomicStore.narrative_facts",
        migrate: None,
        probe: Probe::NotBreaking,
    },
    // v12→v13 adds `NarrativeFact.branch` (world-line branch axis, Round 433 —
    // design sec 7.9 axis 2). Declarative serde default: a pre-v13 fact has no
    // `branch` key, serde fills `MAIN_BRANCH`, and serialization skips the
    // default — a single-world store round-trips byte-identical. Conflict
    // scoping and succession widen from `frame` to `(frame, branch)` (guardrail
    // B-2 key-widening); a store that never names a branch gates exactly as
    // before. So there is deliberately NO `schema_version < 13` arm in `load`,
    // and no migration report is needed.
    Generation {
        to: 13,
        cost: Cost::Additive,
        round: 433,
        what: "adds NarrativeFact.branch",
        migrate: None,
        probe: Probe::NotBreaking,
    },
    // v13→v14 adds `AtomicStore.branches` (world-line branch registry, Round
    // 436) — the frames-registry symmetry the R433 minimal pin deferred: branch
    // refs now fail loud at the write path (`MAIN_BRANCH` ∪ registry) instead of
    // free-form strings, closing the write-side-typo-creates-a-world gap the
    // session review surfaced. Same declarative new-field-default pattern: a
    // pre-v14 store has no `branches` key, serde fills an empty map, and a
    // single-world store (every fact on the default branch) loads and gates
    // exactly as before. So there is deliberately NO `schema_version < 14` arm
    // in `load`, and no migration report is needed.
    Generation {
        to: 14,
        cost: Cost::Additive,
        round: 436,
        what: "adds AtomicStore.branches",
        migrate: None,
        probe: Probe::NotBreaking,
    },
    // v14→v15 adds `AtomicStore.entities` + `NarrativeFact.entities` (narrative
    // entity axis, Round 437 — design sec 7.10 gap 4, pulled live by the
    // AAA/pinion consumer: entity-scoped verification needs a retrieval key).
    // Same declarative new-field-default pattern: pre-v15 stores load with an
    // empty registry and entity-less facts, and a fact that names no entity
    // serializes no `entities` key — byte-stable round-trip. So there is
    // deliberately NO `schema_version < 15` arm in `load`, and no migration
    // report is needed.
    Generation {
        to: 15,
        cost: Cost::Additive,
        round: 437,
        what: "adds AtomicStore.entities + NarrativeFact.entities",
        migrate: None,
        probe: Probe::NotBreaking,
    },
    // v15→v16 adds `Branch.forks_from` (world-line fork point, Round 438 — the
    // shared-history half of the branch axis the R433 minimal pin deferred,
    // surfaced as session-review tension 1: without it a branching story lost
    // its pre-divergence facts on the branch view). `None` = standalone world
    // (pre-fork semantics preserved exactly); ancestry is a forest by
    // construction (parent must already be registered; fork is immutable after
    // registration). Declarative serde default — pre-v16 stores load with
    // fork-less branches and gate identically. So there is deliberately NO
    // `schema_version < 16` arm in `load`, and no migration report is needed.
    Generation {
        to: 16,
        cost: Cost::Additive,
        round: 438,
        what: "adds Branch.forks_from",
        migrate: None,
        probe: Probe::NotBreaking,
    },
    // v16→v17 changes `NarrativeFact.conflicts_with` from bare target ids to
    // [`ConflictAssertion`] rows pinning the target's claim sha256 at judgment
    // time (Round 439 — session-review tension 2: an amend of the target must
    // not leave recorded semantic judgments silently trusted). CLEAN BREAK, no
    // compat shim (pre-release rule): no committed consumer store carries a
    // conflict edge yet, and an old-shape store fails to load LOUDLY (string
    // where a struct is expected) rather than silently dropping the pin. The
    // hash is computed by the primitives, never caller-supplied (R404), and is
    // NEVER auto-refreshed — `scan_continuity` surfaces a stale pin as
    // `ConflictEdgeStale`, and re-affirmation = amending the edge-owning fact
    // (its outbound judgments restamp as the amender's fresh assertions).
    Generation {
        to: 17,
        cost: Cost::Breaking,
        round: 439,
        what: "NarrativeFact.conflicts_with becomes ConflictAssertion rows with a pinned sha256",
        migrate: None,
        probe: Probe::Pair {
            retired: r#"{"narrative_facts": {"f-1": {"frame": "gt", "claim": "c", "canon_from": "s-1", "evidence": [], "conflicts_with": ["f-2"]}}}"#,
            control: r#"{"narrative_facts": {"f-1": {"frame": "gt", "claim": "c", "canon_from": "s-1", "evidence": []}}}"#,
        },
    },
    // v17→v18 adds `NarrativeFact.payoff_expectation` + `NarrativeFact.pays_off`
    // (setup/payoff coverage, Round 442): an optional expectation on a setup
    // fact and the identity edges that pay it, so a dangling setup is reportable
    // per world rather than inferred. Additive, same declarative pattern as the
    // bumps around it — an `Option<String>` and a `Vec<String>` under serde
    // defaults, so a pre-v18 fact has neither key and round-trips
    // byte-identically, and a store that never declares an expectation reports
    // nothing dangling.
    //
    // WRITTEN IN ROUND 1254, TWELVE HUNDRED ROUNDS LATE. This generation bumped
    // the version and left no paragraph at all, which is one of the three holes
    // that made this table a table.
    Generation {
        to: 18,
        cost: Cost::Additive,
        round: 442,
        what: "adds NarrativeFact.payoff_expectation + NarrativeFact.pays_off",
        migrate: None,
        probe: Probe::NotBreaking,
    },
    // v18→v19 adds `AtomicStore.predicates` (the 4th registry) and
    // `NarrativeFact.typed` (the optional TypedClaim leg, Round 446 — design
    // sec 7.12 step 2: the machine-readable subject–predicate–object reading
    // authored in the same act as the prose claim, never NLP-derived). Both
    // declarative serde defaults (empty map / `None`, skip-serialized) — every
    // pre-v19 store loads unchanged and stays byte-stable. So there is
    // deliberately NO `schema_version < 19` arm in `load`, and no migration
    // report is needed.
    Generation {
        to: 19,
        cost: Cost::Additive,
        round: 446,
        what: "adds AtomicStore.predicates and NarrativeFact.typed",
        migrate: None,
        probe: Probe::NotBreaking,
    },
    // v19→v20 added the `ConfirmationClaim::FactEvidence { fact_id }` variant
    // (the R481 LLM-verdict drift target).
    Generation {
        to: 20,
        cost: Cost::Additive,
        round: 481,
        what: "adds the ConfirmationClaim::FactEvidence variant",
        migrate: None,
        probe: Probe::NotBreaking,
    },
    // v20→v21 REMOVES it (Round 485 — the all-deterministic redesign R484:
    // R483's blind acceptance falsified the LLM-verdict approach, drift moved to
    // the deterministic typed-substantiation scan, and no-legacy-carry retires
    // the dead variant in the same change). No canonical store ever carried a
    // `fact_evidence` event (the dogfood store's only such events lived in a
    // throwaway grading copy), so the removal loses no data; a v21 store has no
    // `fact_evidence` events and the monotonic bump records the variant's
    // retirement. No migration arm needed.
    //
    // BREAKING FOR A STORE THAT CARRIES ONE, which is the sense this table uses:
    // the typed parse refuses an unknown variant rather than dropping it. The
    // paragraph above is why no such store is known to exist.
    Generation {
        to: 21,
        cost: Cost::Breaking,
        round: 485,
        what: "removes the ConfirmationClaim::FactEvidence variant",
        migrate: None,
        probe: Probe::Pair {
            retired: r#"{"confirmation_events": {"e-1": {"claim": {"kind": "fact_evidence", "fact_id": "f-1"}, "confirmer": {"kind": "tool", "id": "t", "version": "1"}, "method": "linkage_check", "authoring_run": "a", "confirming_run": "b", "verdict": "confirm", "rationale": "r", "timestamp": "2026-01-01"}}}"#,
            control: r#"{"confirmation_events": {"e-1": {"claim": {"kind": "section_completeness", "section_id": "s-1"}, "confirmer": {"kind": "tool", "id": "t", "version": "1"}, "method": "linkage_check", "authoring_run": "a", "confirming_run": "b", "verdict": "confirm", "rationale": "r", "timestamp": "2026-01-01"}}}"#,
        },
    },
    // v21→v22 adds `AtomicStore.disclosure_plans` (the disclosure/discourse
    // layer, Round 506 — design sec 7.24): a top-level registry of named
    // tellings over the fact base, mirroring the v9→v10 confirmation_events /
    // v11→v12 narrative_facts placement. Same declarative new-field-default
    // pattern: a pre-v22 store has no `disclosure_plans` key, serde
    // `#[serde(default)]` fills an empty map — no behavior change (nothing reads
    // the plans until the `--telling` carrier + the render-acceptance gates run,
    // and those are out-of-band render-loop tools, not validate-workspace). So
    // there is deliberately NO `schema_version < 22` arm in `load`, and no
    // migration report is needed.
    Generation {
        to: 22,
        cost: Cost::Additive,
        round: 506,
        what: "adds AtomicStore.disclosure_plans",
        migrate: None,
        probe: Probe::NotBreaking,
    },
    // v22→v23 adds `Branch.converges_from` (Round 532) — the inverse of
    // `forks_from`: a branch may declare two or more parents it converges from,
    // so a confluence is declarable and visible in the fork tree instead of
    // being a shape the registry could not express. Additive, the same
    // declarative pattern as `forks_from` one generation over: a `Vec` under
    // serde defaults, absent on a pre-v23 branch, and a store whose branches
    // never converge round-trips byte-identically.
    //
    // WRITTEN IN ROUND 1254, SEVEN HUNDRED ROUNDS LATE — the second of the three
    // generations the prose ladder never mentioned.
    Generation {
        to: 23,
        cost: Cost::Additive,
        round: 532,
        what: "adds Branch.converges_from",
        migrate: None,
        probe: Probe::NotBreaking,
    },
    // v23→v24 adds `AtomicStore.entity_kinds` and turns `Entity.kind` from FREE
    // TEXT into a registry ref (Round 661's machine-slot rule reaching the slot
    // that round missed: it counted the entity ID as registered and never looked
    // at the kind INSIDE the record). This one IS breaking, unlike the bumps
    // above: a pre-v24 store whose entities carry kinds has NO registry, and a
    // non-empty kind that does not resolve is a boundary REJECT. That is
    // deliberate and is not a compat carry — an unregistered kind is exactly the
    // defect being closed, so it fails loud rather than passing on a default.
    // Migration is one `add-entity-kind` per distinct kind, measured at 5 for
    // the live 109-entity corpus and 2 for the tracked experiment store. There
    // is no `schema_version < 24` arm in `load` for the same reason: silently
    // back-filling the registry from the kinds already in the file would
    // "migrate" a typo into a registered vocabulary and defeat the gate.
    Generation {
        to: 24,
        cost: Cost::Gated,
        round: 669,
        what: "adds AtomicStore.entity_kinds; Entity.kind becomes a registry ref",
        migrate: None,
        probe: Probe::Boundary {
            opens: r#"{"entities": {"e-1": {"entity_id": "e-1", "kind": "person"}}}"#,
            refused_by: an_unregistered_kind_is_refused_at_the_write_boundary,
        },
    },
    // v24→v25 adds `Predicate.subject_kind` / `Predicate.object_entity_kind`
    // (Round 701 — the spatial-map G1 endpoint-kind gate declared on the
    // predicate, enforced at the fact write path). Same declarative
    // new-field-default pattern as v21→v22: both fields are `Option`,
    // `#[serde(default, skip_serializing_if = Option::is_none)]`, so a pre-v25
    // predicate has neither key and loads as `None` (= no constraint = the prior
    // behavior), and a constraint-free predicate serializes byte-identically.
    // The gate is a WRITE-path check only — no fact is re-validated on load — so
    // there is deliberately NO `schema_version < 25` arm and no migration
    // report. The monotonic bump is the guard against a STALE older binary
    // reading a v25 store, silently dropping the two unknown fields on save, and
    // erasing the map gate: `schema_version > CURRENT_SCHEMA_VERSION` rejects
    // that newer store loudly instead.
    Generation {
        to: 25,
        cost: Cost::Additive,
        round: 701,
        what: "adds Predicate.subject_kind / Predicate.object_entity_kind",
        migrate: None,
        probe: Probe::NotBreaking,
    },
    // v25→v26 adds `Predicate.object_tokens` (Round 705 — the closed token
    // vocabulary for `object_kind=token`). Same declarative pattern: `BTreeSet`,
    // `#[serde(default, skip_serializing_if = BTreeSet::is_empty)]`, so a
    // non-token predicate has no key and serializes byte-identically, and no
    // fact is re-validated on load. The stale-binary silent-drop risk that
    // motivated the v24→v25 bump is even weaker here — `object_tokens` only ever
    // accompanies the NEW `object_kind=token` / `TypedObject::Token` variants,
    // which a pre-R705 binary already rejects loudly (serde unknown variant
    // `token`), so it cannot reach the drop-on-save path — but the bump is kept
    // for audit consistency (the codebase notes every on-disk shape change) and
    // to keep the `> CURRENT` guard monotone.
    Generation {
        to: 26,
        cost: Cost::Additive,
        round: 705,
        what: "adds Predicate.object_tokens",
        migrate: None,
        probe: Probe::NotBreaking,
    },
    // v26→v27 adds `AtomicStore.units` (Round 706 — the unit registry) and the
    // `TypedObject::Quantity { n, unit }` object shape (`object_kind=quantity`).
    // `units` is a top-level `BTreeMap` under `#[serde(default)]`, empty on
    // older stores. As with `object_tokens`, the silent-drop risk is weak: a
    // Quantity object only appears under the NEW `object_kind=quantity` /
    // `TypedObject::Quantity` variants, which a pre-R706 binary rejects loudly
    // (serde unknown variant `quantity`), so a quantity store cannot reach the
    // drop-on-save path of an old binary. The bump keeps the `> CURRENT` guard
    // monotone and the audit trail of on-disk shape changes complete.
    Generation {
        to: 27,
        cost: Cost::Additive,
        round: 706,
        what: "adds AtomicStore.units and the TypedObject::Quantity object shape",
        migrate: None,
        probe: Probe::NotBreaking,
    },
    // v27→v28 adds the `TypedObject::Fact { id }` object shape
    // (`object_kind=fact`). No new top-level field — a Fact object is a
    // fact-identity ref (like conflicts_with / pays_off), validated in phase 2
    // and delete-guarded, not a registry. A pre-R707 binary rejects a `fact`
    // object loudly (serde unknown variant), so no silent drop; the bump keeps
    // the `> CURRENT` guard monotone and the shape-change audit complete.
    Generation {
        to: 28,
        cost: Cost::Additive,
        round: 707,
        what: "adds the TypedObject::Fact object shape",
        migrate: None,
        probe: Probe::NotBreaking,
    },
    // v28→v29 REMOVES the free-text `TypedObject::Value` shape +
    // `object_kind=scalar` (Round 708 — the object-shape-closure arc's terminus:
    // every machine-slot object is now registered/enumerable, free text lives
    // ONLY in the prose `claim`). This is a BREAKING removal, not an additive
    // bump: a store still carrying `{kind:value}` objects fails the strict
    // parse, and `load` turns that into the NAMED migration work-list
    // (`removed_value_shape_error` → `RemovedValueShape`) rather than a silent
    // drop or a cryptic serde error (the R625 brick lesson). The migration is
    // executable: each value object re-authors to a token / quantity / fact
    // object, or its text moves to the fact's `claim`. There is deliberately no
    // `schema_version < 29` back-fill arm — silently guessing a shape for free
    // text would defeat the closure the removal exists to enforce.
    Generation {
        to: 29,
        cost: Cost::Breaking,
        round: 708,
        what: "removes the TypedObject::Value / object_kind=scalar free-text shape",
        migrate: None,
        probe: Probe::Pair {
            retired: r#"{"narrative_facts": {"f-1": {"frame": "gt", "claim": "c", "canon_from": "s-1", "evidence": [], "typed": {"subject": "e-1", "predicate": "p-1", "object": {"kind": "value", "text": "free text"}}}}}"#,
            control: r#"{"narrative_facts": {"f-1": {"frame": "gt", "claim": "c", "canon_from": "s-1", "evidence": []}}}"#,
        },
    },
    // v29→v30 adds `AtomicStore.edge_costs` (Round 709 design → DEBT-J build —
    // the map edge-cost side-table). Additive: a top-level `BTreeMap` under
    // `#[serde(default)]`, empty on older stores, no fact re-validated on load.
    // A pre-DEBT-J binary reading a v30 store drops the unknown field on save
    // (the v24→v25 silent-drop class), which the monotone `> CURRENT` guard
    // rejects loudly instead. No migration arm needed (the field is new, not a
    // shape change to existing data).
    Generation {
        to: 30,
        cost: Cost::Additive,
        round: 709,
        what: "adds AtomicStore.edge_costs",
        migrate: None,
        probe: Probe::NotBreaking,
    },
    // v30→v31 adds `AtomicStore.edge_guards` (Round 717 design → Round 720 build
    // — the map edge-GUARD side-table: a place-access condition on an adjacency
    // edge, keyed by the edge fact id, value = the CONDITION fact id it
    // requires). Additive, same class as edge_costs (a top-level `BTreeMap`
    // under `#[serde(default)]`, empty on older stores); no migration arm.
    Generation {
        to: 31,
        cost: Cost::Additive,
        round: 720,
        what: "adds AtomicStore.edge_guards",
        migrate: None,
        probe: Probe::NotBreaking,
    },
    // v31→v32 changes `edge_guards`' VALUE from a single condition `String` to a
    // `BTreeSet<String>` — a SET of required conditions (AND; Round 721 design →
    // Round 722 build). A SHAPE change, so the bump is real; but NO migration
    // code is needed — the live store's edge_guards is `{}`, and an empty map
    // deserializes identically as `BTreeMap<String, BTreeSet<String>>`. No
    // populated v31 store exists (edge_guards landed empty in R720), so serde's
    // refusal to coerce a bare `"c"` into `["c"]` is moot; the monotone
    // `> CURRENT` guard stops an old binary misreading a populated set.
    Generation {
        to: 32,
        cost: Cost::Breaking,
        round: 722,
        what: "edge_guards' value becomes a set of conditions",
        migrate: None,
        probe: Probe::Pair {
            retired: r#"{"edge_guards": {"f-1": "c-1"}}"#,
            control: r#"{"edge_guards": {"f-1": {"conditions": ["c-1"]}}}"#,
        },
    },
    // v32→v33 changes `edge_guards`' VALUE from a bare `BTreeSet<String>` to the
    // `EdgeGuard` struct — a condition set PLUS an optional K-of-N `threshold`
    // (Round 723). A SHAPE change; the bump is real. NO bare-array back-fill
    // adapter (review F1 — YAGNI by the same "no populated store" reasoning as
    // v32: edge_guards is `{}` in Mnemosyne, no game/tide store carries
    // store-native guards). The wire form is the OBJECT shape only; an old v32
    // bare-array value would fail the struct parse LOUD (no silent guess), and
    // the monotone `> CURRENT` guard stops an old binary misreading a threshold
    // guard's object as `Some(len)` AND.
    Generation {
        to: 33,
        cost: Cost::Breaking,
        round: 723,
        what: "edge_guards' value becomes the EdgeGuard struct with a threshold",
        migrate: None,
        probe: Probe::Pair {
            retired: r#"{"edge_guards": {"f-1": ["c-1"]}}"#,
            control: r#"{"edge_guards": {"f-1": {"conditions": ["c-1"]}}}"#,
        },
    },
    // v33→v34 adds `AtomicStore.parameters` (a numeric-meter registry) +
    // `AtomicStore.parameter_deltas` (a per-beat signed-delta side-table) —
    // Round 728 design → Round 729 build, DEBT-K. Additive: two top-level
    // `BTreeMap`s under `#[serde(default)]`, empty on older stores, no fact
    // re-validated on load. A pre-DEBT-K binary reading a v34 store drops the
    // unknown fields on save (the silent-drop class), which the monotone
    // `> CURRENT` guard rejects loudly instead. No migration arm (new fields,
    // not a shape change to existing data).
    Generation {
        to: 34,
        cost: Cost::Additive,
        round: 729,
        what: "adds AtomicStore.parameters + AtomicStore.parameter_deltas",
        migrate: None,
        probe: Probe::NotBreaking,
    },
    // v34→v35 adds `AtomicStore.parameter_gates` (a per-choice
    // numeric-threshold gate side-table) — Round 728 design → Round 730 build,
    // DEBT-K (the CHOICE half: gaps 1+2). Additive: a top-level `BTreeMap` under
    // `#[serde(default)]`, empty on older stores, no fact re-validated on load.
    // Same class as parameter_deltas — a pre-R730 binary reading a v35 store
    // drops the unknown field on save, which the monotone `> CURRENT` guard
    // rejects loudly instead. No migration arm (a new field, not a shape change
    // to existing data).
    Generation {
        to: 35,
        cost: Cost::Additive,
        round: 730,
        what: "adds AtomicStore.parameter_gates",
        migrate: None,
        probe: Probe::NotBreaking,
    },
    // v35→v36 adds `AtomicStore.fact_counts` (a per-fact multiset-count
    // side-table) — Round 731 build, DEBT-L (the distinct part of
    // multiset/quantity custody: a positive count bound to a custody fact, which
    // singular `holds` cannot express). Additive: a top-level `BTreeMap<String,
    // i64>` under `#[serde(default)]`, empty on older stores, no fact
    // re-validated on load. Same class as parameter_gates — a pre-R731 binary
    // reading a v36 store drops the unknown field on save, which the monotone
    // `> CURRENT` guard rejects loudly instead. No migration arm (a new field,
    // not a shape change to existing data).
    Generation {
        to: 36,
        cost: Cost::Additive,
        round: 731,
        what: "adds AtomicStore.fact_counts",
        migrate: None,
        probe: Probe::NotBreaking,
    },
    // v36→v37 adds `EntityKind.parent` (an optional registered-kind ref forming
    // a single-parent kind INHERITANCE TREE) — Round 732 build, DEBT-M
    // (Inform-style kind-subtree rule scope: "a weapon is a kind of thing" so
    // thing-scoped rules accept weapons, which flat kinds cannot express).
    // Additive: an `Option<String>` under `#[serde(default)]`, None on older
    // stores (0 parent links ⇒ every subtree is a singleton ⇒ identical to
    // today). A pre-R732 binary reading a v37 store drops the field on save,
    // which the monotone `> CURRENT` guard rejects loudly instead. No migration
    // arm (a new field, not a shape change to existing data).
    Generation {
        to: 37,
        cost: Cost::Additive,
        round: 732,
        what: "adds EntityKind.parent",
        migrate: None,
        probe: Probe::NotBreaking,
    },
    // v37→v38 GENERALISES `EntityKind.parent: Option<String>` to `parents:
    // BTreeSet<String>` — Round 738, the R661 kind-tree extension (single-parent
    // TREE → multiple-inheritance DAG: a `magic-sword` is BOTH a `weapon` and a
    // `magic-item`). This is a SHAPE CHANGE to an existing field (rename +
    // retype), not an additive one, so it DOES need a migration arm: a v37 store
    // may carry `"parent": "thing"` on a kind, which the retyped struct would
    // silently DROP on read (EntityKind has no `deny_unknown_fields`) — data
    // loss. `load` therefore runs `migrate_entity_kind_parent_to_parents` on the
    // raw JSON when the on-disk `schema_version < 38`, rewriting each
    // `"parent": "x"` to `"parents": ["x"]` BEFORE the typed parse.
    // Backward-compat is exact: `None` / absent ⇒ an empty set ⇒ a root kind (a
    // flat registry, unchanged). A pre-R738 binary reading a v38 store hits the
    // monotone `> CURRENT` guard.
    Generation {
        to: 38,
        cost: Cost::Migrated,
        round: 738,
        what: "EntityKind.parent generalises to parents (a set)",
        migrate: Some(migrate_entity_kind_parent_to_parents),
        probe: Probe::NotBreaking,
    },
    // v38→v39 GENERALISES `DisclosureOverride.first_at`'s per-world VALUE from a
    // single ordinal coord (`String`) to a `DisclosureReveal` { coords:
    // BTreeSet<String>, threshold: Option<usize> } — Round 752, the R751
    // first-reached-of-a-set trigger (the non-linear-render pull; the edge-guard
    // shape mirror). This is a SHAPE CHANGE to an existing map value, not an
    // additive one, so it needs a migration arm: a v38 store carries `first_at:
    // {branch: "sc-NN"}`, which the retyped struct fails to parse (a string
    // where an object is expected). `load` runs
    // `migrate_disclosure_first_at_to_reveal` on the raw JSON when the on-disk
    // `schema_version < 39`, rewriting each `"sc-NN"` to `{"coords": ["sc-NN"],
    // "threshold": null}` BEFORE the typed parse. Backward-compat is exact: the
    // single ordinal becomes a one-coord first-reached trigger (the same
    // effective pin). A pre-R752 binary reading a v39 store hits the monotone
    // `> CURRENT` guard.
    Generation {
        to: 39,
        cost: Cost::Migrated,
        round: 752,
        what: "DisclosureOverride.first_at's value becomes a DisclosureReveal",
        migrate: Some(migrate_disclosure_first_at_to_reveal),
        probe: Probe::NotBreaking,
    },
    // v39→v40 adds `AtomicSection.content_excerpt` (Round 756, P3a) — the
    // store-owned narrative-prose provenance anchor (a Layer-0 `ContentAnchor` +
    // a projected-text cache + a drift hash), generalizing `normative_excerpt`
    // to narrative sections. Same declarative new-field-default pattern as
    // v6→v7 epub_locator / v8 excerpt hash: a pre-v40 section has no
    // `content_excerpt` key, serde `#[serde(default)]` fills `None`, and
    // serialization skips it when `None` — byte-identical behavior for any store
    // that has not ingested a narrative excerpt. So there is deliberately NO
    // `schema_version < 40` arm in `load`, and no migration report is needed.
    Generation {
        to: 40,
        cost: Cost::Additive,
        round: 756,
        what: "adds AtomicSection.content_excerpt",
        migrate: None,
        probe: Probe::NotBreaking,
    },
    // v40→v41 adds `AtomicSection.scene_cast` (Round 757, B0) — the store-owned
    // scene-presence list (who is in a scene + the authored modality/can_answer
    // + a manuscript `ContentExcerpt` proving each presence), so a consumer
    // reads the cast of a scene from the store instead of a parallel identity
    // space. Same declarative new-field-default pattern as v39→v40
    // content_excerpt: a pre-v41 section has no `scene_cast` key, serde
    // `#[serde(default)]` fills an empty `Vec`, and serialization skips it when
    // empty — byte-identical behavior for any store that has not ingested scene
    // presence. So there is deliberately NO `schema_version < 41` arm in `load`,
    // and no migration report is needed.
    Generation {
        to: 41,
        cost: Cost::Additive,
        round: 757,
        what: "adds AtomicSection.scene_cast",
        migrate: None,
        probe: Probe::NotBreaking,
    },
    // v41→v42 CONVERGES `NormativeExcerpt` onto the one provenance substrate
    // `ContentExcerpt` (Round 759, P3c): the flat `{text, anchor_url,
    // source_revision, text_sha256}` becomes `{excerpt: {anchor, text,
    // text_sha256}, anchor_url, source_revision}`, so spec + narrative share ONE
    // text/sha validator. This is a SHAPE CHANGE to an existing field (the new
    // `excerpt` sub-object has no serde default), so it needs a migration arm: a
    // v41 store carries the flat shape, which the retyped struct fails to parse
    // (missing field `excerpt`). `load` runs
    // `migrate_normative_excerpt_to_wrapped` on the raw JSON when the on-disk
    // `schema_version < 42`, reshaping each `normative_excerpt` and synthesizing
    // `excerpt.anchor` from the Section's `epub_locator` (if present) ELSE from
    // `anchor_url` (`Locator::Prefix(text)`) via the SAME
    // `normative_excerpt_anchor` the constructor uses — no field dropped, no
    // data invented, never panics. Backward-compat is exact: the same text +
    // hash + upstream origin, now positioned under one substrate. A pre-R759
    // binary reading a v42 store hits the monotone `> CURRENT` guard.
    Generation {
        to: 42,
        cost: Cost::Migrated,
        round: 759,
        what: "NormativeExcerpt converges onto the ContentExcerpt substrate",
        migrate: Some(migrate_normative_excerpt_to_wrapped),
        probe: Probe::NotBreaking,
    },
    // v42→v43 adds `AtomicSection.ladder` (Round 765) — the store-owned
    // interactive ladder: a carrier entity + ordered rungs, each a
    // `Locator::Prefix` coordinate into the section's OWN `content_excerpt` plus
    // the `needs`/`reveals` gate, so a consumer stops holding a parallel
    // sub-section coordinate space the kernel cannot see. Same declarative
    // new-field-default pattern as v39→v40 content_excerpt and v40→v41
    // scene_cast: a pre-v43 section has no `ladder` key, serde
    // `#[serde(default)]` fills `None`, and serialization skips it when `None` —
    // byte-identical behavior for any store that has not ingested a ladder. So
    // there is deliberately NO `schema_version < 43` arm in `load`, and no
    // migration report is needed. A pre-R765 binary reading a v43 store hits the
    // monotone `> CURRENT` guard.
    Generation {
        to: 43,
        cost: Cost::Additive,
        round: 765,
        what: "adds AtomicSection.ladder",
        migrate: None,
        probe: Probe::NotBreaking,
    },
    // v43→v44 moves `NarrativeFact.evidence` from `Vec<String>` to
    // `Vec<EvidenceRef>` (Round 806) — each evidence ref now carries the sha256
    // of the prose the author affirms they judged the claim against, so a claim
    // whose evidence moved is detectable instead of silently outliving it. A
    // typed parse of a v43 store would FAIL (a string where an object is
    // expected), so `load` runs `migrate_evidence_to_refs` on the raw JSON when
    // `schema_version < 44`; it writes an EMPTY affirmation, never the live
    // excerpt hash (seeding would assert reviews that never happened). A
    // pre-R806 binary reading a v44 store hits the monotone `> CURRENT` guard.
    Generation {
        to: 44,
        cost: Cost::Migrated,
        round: 806,
        what: "NarrativeFact.evidence becomes EvidenceRef rows carrying a sha256",
        migrate: Some(migrate_evidence_to_refs),
        probe: Probe::NotBreaking,
    },
    // v44→v45 adds `AtomicChangelogEntry.population_census` (Round 979 — what
    // the recorded population said at the moment an entry was appended, so a
    // later round inherits DATA rather than a sentence). Same declarative
    // new-field-default pattern as v40→v41 and v42→v43: the field is a `Vec`
    // with `#[serde(default, skip_serializing_if = "Vec::is_empty")]`, so a
    // pre-v45 entry has no key, loads as empty, and re-serializes
    // byte-identically. There is deliberately NO `schema_version < 45` arm and
    // no migration report — back-filling it would mean writing today's counts
    // under yesterday's round, which is the stale-baseline defect the field
    // exists to end. A pre-R979 binary reading a v45 store hits the monotone
    // `> CURRENT` guard rather than silently dropping the key on save.
    Generation {
        to: 45,
        cost: Cost::Additive,
        round: 979,
        what: "adds AtomicChangelogEntry.population_census",
        migrate: None,
        probe: Probe::NotBreaking,
    },
    // v45→v46 adds `AtomicStore.mutation_reasons` (Round 1024) — the reason a
    // mutating primitive was given, kept instead of validated and discarded, so
    // WHY the store changed is a ledger row rather than a sentence in a commit
    // message. Additive: a top-level `Vec` under `#[serde(default)]`, empty on
    // older stores, no fact re-validated on load. A pre-R1024 binary reading a
    // v46 store drops the unknown field on save, which the monotone `> CURRENT`
    // guard rejects loudly instead.
    //
    // WRITTEN IN ROUND 1254, TWO HUNDRED ROUNDS LATE, and the one that showed
    // the ladder had stopped tracking the constant: the prose ended at v45 while
    // the build wrote v46, and nothing anywhere could tell.
    Generation {
        to: 46,
        cost: Cost::Additive,
        round: 1024,
        what: "adds AtomicStore.mutation_reasons",
        migrate: None,
        probe: Probe::NotBreaking,
    },
];

/// The store schema generation the current binary writes and validates against
/// (bumped on a breaking shape change). Public so the medium-neutral authoring
/// contract (`describe-schema`, R587) can report which generation it describes.
///
/// DERIVED FROM THE LADDER, and that is the point (Round 1254): a bump without a
/// row is now a build that does not compile, rather than a version constant that
/// moved while the record stood still. Which is what had happened — v46 shipped
/// in Round 1024 and the prose ladder ended at v45.
pub const CURRENT_SCHEMA_VERSION: u32 = GENERATIONS[GENERATIONS.len() - 1].to;

/// The ladder is contiguous, starts at 2, and every `Migrated` row carries the
/// rewrite that makes it one.
///
/// A CONST ASSERTION AND NOT A TEST, because both properties are about the table
/// as written rather than about anything that happens at run time: a gap, a
/// duplicate, a row out of order, or a `Migrated` row with nothing to run is a
/// compile error at the point somebody writes it.
const fn ladder_holds(g: &[Generation]) -> bool {
    if g.is_empty() || g[0].to != 2 {
        return false;
    }
    let mut i = 0;
    while i < g.len() {
        if g[i].to != i as u32 + 2 {
            return false;
        }
        let migrated = matches!(g[i].cost, Cost::Migrated);
        if migrated != g[i].migrate.is_some() {
            return false;
        }
        // ROUND 1255 — and the same pairing on the costs that claim an old
        // store is not simply carried. A `Breaking` or `Gated` row must exhibit
        // that, or say why it does not; a row that claims neither must not
        // carry a probe, because a probe beside `Additive` is a refusal nothing
        // in the table is asserting.
        let costs_work = matches!(g[i].cost, Cost::Breaking | Cost::Gated);
        if costs_work == matches!(g[i].probe, Probe::NotBreaking) {
            return false;
        }
        // AND THE TWO SHAPES OF PROBE ARE NOT INTERCHANGEABLE. `Boundary` says
        // the store OPENS and a write is refused; `Pair` says the store does
        // not open. Reading one as the other is exactly the misclassification
        // v24 carried, so the pairing is held here rather than left to a
        // reader.
        let gated = matches!(g[i].cost, Cost::Gated);
        if gated != matches!(g[i].probe, Probe::Boundary { .. }) {
            return false;
        }
        i += 1;
    }
    true
}

const _: () = assert!(
    ladder_holds(GENERATIONS),
    "the schema ladder must be contiguous from 2, every Migrated row must carry its rewrite, \
     and every Breaking row must carry a probe or say why it has none"
);

/// The generations a store written at `on_disk` must cross to be read by this
/// build. Empty when the store is current or newer.
pub fn crossed_by(on_disk: u32) -> &'static [Generation] {
    let first = GENERATIONS
        .iter()
        .position(|g| g.to > on_disk)
        .unwrap_or(GENERATIONS.len());
    &GENERATIONS[first..]
}

/// What stands between a store and this build, in one line, for a refusal that
/// has already said how far behind the store is.
///
/// THE DISTANCE IS AN UPPER BOUND AND THIS IS THE DIAGNOSIS. Most generations
/// are additive and cost an old store nothing; a reader who is told only "23
/// generations behind" has to read the source to find out whether that means any
/// work at all. So the breaking ones are NAMED, and the rest are counted.
pub fn crossing_note(on_disk: u32) -> String {
    let crossing = crossed_by(on_disk);
    if crossing.is_empty() {
        return String::new();
    }
    let count = |cost: Cost| crossing.iter().filter(|g| g.cost == cost).count();
    let (migrated, breaking, gated) = (
        count(Cost::Migrated),
        count(Cost::Breaking),
        count(Cost::Gated),
    );
    let mut note = format!(
        "; crossing {} generation(s): {} additive, {migrated} migrated on load, \
         {breaking} that may refuse to open, {gated} that open and refuse the first write",
        crossing.len(),
        crossing.len() - migrated - breaking - gated,
    );
    // THE ONES THAT COST SOMETHING ARE NAMED, in ladder order, and each says
    // WHICH of the two ways it bites. The additive and migrated ones are
    // counted: a holder has nothing to do about either, and naming twenty of
    // them would bury the four that matter.
    for g in crossing
        .iter()
        .filter(|g| matches!(g.cost, Cost::Breaking | Cost::Gated))
    {
        note.push_str(&format!(
            " — v{} (Round {}) {}{}",
            g.to,
            g.round,
            g.what,
            if g.cost == Cost::Gated {
                " [opens; the write is what is refused]"
            } else {
                ""
            }
        ));
    }
    note
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::AtomicStore;

    #[test]
    fn the_current_version_is_the_last_rung() {
        assert_eq!(
            CURRENT_SCHEMA_VERSION,
            GENERATIONS.last().expect("a non-empty ladder").to
        );
        // NOT A TAUTOLOGY WITH THE CONST ABOVE, which is how it is DEFINED. This
        // is the number itself: a bump that lands without a row cannot compile,
        // and this says which number the ladder currently reaches, so a reader
        // comparing it with a store's `schema_version` is comparing two things
        // that were derived the same way.
        assert_eq!(CURRENT_SCHEMA_VERSION, 46);
    }

    #[test]
    fn a_store_at_the_current_generation_crosses_nothing() {
        assert!(crossed_by(CURRENT_SCHEMA_VERSION).is_empty());
        assert!(crossed_by(CURRENT_SCHEMA_VERSION + 1).is_empty());
        assert_eq!(crossing_note(CURRENT_SCHEMA_VERSION), "");
    }

    #[test]
    fn the_generations_a_store_crosses_are_the_ones_after_it() {
        let crossing = crossed_by(44);
        assert_eq!(
            crossing.iter().map(|g| g.to).collect::<Vec<_>>(),
            vec![45, 46]
        );
        assert!(crossed_by(1).len() == GENERATIONS.len());
    }

    /// THE ANSWER R1247 COULD NOT GIVE. A store at generation 23 is 23
    /// generations behind, and that number is an upper bound: what it actually
    /// costs is four rungs out of twenty-three, and this is where a reader
    /// learns which — and, since R1255, which of the two ways each one bites.
    #[test]
    fn the_note_names_what_costs_something_and_counts_the_rest() {
        let note = crossing_note(23);
        assert!(note.contains("crossing 23 generation(s)"), "{note}");
        // v29, v32 and v33 refuse to open; v24 OPENS and refuses the write,
        // which is the distinction a probe found rather than a paragraph.
        assert!(note.contains("3 that may refuse to open"), "{note}");
        assert!(
            note.contains("1 that open and refuse the first write"),
            "{note}"
        );
        assert!(note.contains("v29 (Round 708)"), "{note}");
        assert!(
            note.contains("removes the TypedObject::Value"),
            "a rung that costs something says what it did: {note}"
        );
        assert!(
            note.contains("v24 (Round 669)")
                && note.contains("[opens; the write is what is refused]"),
            "the gated rung must not read as one that will not open: {note}"
        );
        // And the migrated ones are counted rather than named: they cost a
        // person nothing, which is the whole distinction.
        assert!(note.contains("4 migrated on load"), "{note}");
    }

    /// A store body at `version`, written where `load` will find it.
    fn store_at(dir: &std::path::Path, version: u32, body: &str) -> std::path::PathBuf {
        let mut doc: serde_json::Value =
            serde_json::from_str(body).expect("a probe body is a JSON object");
        doc["schema_version"] = serde_json::json!(version);
        let path = dir.join(format!("store-{version}.json"));
        std::fs::write(&path, serde_json::to_vec_pretty(&doc).expect("serialize")).expect("write");
        path
    }

    /// THE LAW N153 ASKED FOR: every row's `cost` is exercised, and the ones
    /// that are not say so.
    ///
    /// A breaking row carries the shape it retired plus the same store without
    /// it. `load` must REFUSE the first and ACCEPT the second — the control is
    /// what makes the refusal attributable to the retired shape rather than to
    /// a fixture nobody could parse, which is the way a probe silently stops
    /// probing. A non-breaking row claims an old store simply loads, and a bare
    /// store at the generation below it is that claim.
    #[test]
    fn every_cost_is_one_a_store_exhibits() {
        let tmp = TempDir::new().expect("tempdir");
        let mut probed = 0usize;
        let mut unprobed: Vec<(u32, &str)> = Vec::new();
        for g in GENERATIONS {
            let before = g.to - 1;
            match &g.probe {
                Probe::NotBreaking => {
                    // The claim is that an old store LOADS. The bare store is
                    // the weakest form of it and the only one that is true of
                    // every additive generation at once: what the new field's
                    // absence does is exactly what a store without it does.
                    let path = store_at(tmp.path(), before, "{}");
                    AtomicStore::load(&path).unwrap_or_else(|e| {
                        panic!(
                            "v{}: a {:?} generation must open a store from the one below it: {e}",
                            g.to, g.cost
                        )
                    });
                }
                Probe::Pair { retired, control } => {
                    let bad = store_at(tmp.path(), before, retired);
                    let err = AtomicStore::load(&bad).err().unwrap_or_else(|| {
                        panic!(
                            "v{} (Round {}) is recorded Breaking and this build opened a store \
                             carrying the shape it retired — the cost is wrong, and a refusal \
                             that names it would be a confident wrong diagnosis",
                            g.to, g.round
                        )
                    });
                    std::fs::remove_file(&bad).expect("clear the fixture");
                    let good = store_at(tmp.path(), before, control);
                    AtomicStore::load(&good).unwrap_or_else(|_| {
                        panic!(
                            "v{}: the CONTROL must load, or the refusal above is about a fixture \
                             nobody could parse rather than about the retired shape. The refusal \
                             was: {err}",
                            g.to
                        )
                    });
                    std::fs::remove_file(&good).expect("clear the fixture");
                    probed += 1;
                }
                Probe::Boundary { opens, refused_by } => {
                    // BOTH HALVES, because either alone is a misreading. The
                    // store OPENS — v24 was recorded `Breaking` and this is the
                    // measurement that said otherwise — and the write is what
                    // does not go through.
                    let path = store_at(tmp.path(), before, opens);
                    let mut store = AtomicStore::load(&path).unwrap_or_else(|e| {
                        panic!(
                            "v{}: a Gated generation OPENS such a store; if it no longer does, \
                             the cost is Breaking and the probe is the wrong shape: {e}",
                            g.to
                        )
                    });
                    assert!(
                        refused_by(&mut store, &path),
                        "v{} (Round {}) is recorded Gated and the write went through — the store \
                         opens and nothing is refused, which is what Additive says",
                        g.to,
                        g.round
                    );
                    std::fs::remove_file(&path).expect("clear the fixture");
                    probed += 1;
                }
                Probe::Unprobed(why) => unprobed.push((g.to, why)),
            }
        }
        // NON-VACUITY, and the count is printed rather than asserted at a
        // number: a table where every breaking row had drifted to `Unprobed`
        // would satisfy every assertion above while exhibiting nothing.
        assert!(
            probed > 0,
            "no breaking generation was exhibited — this test asserted nothing"
        );
        println!(
            "{probed} generation(s) that cost something were exhibited against a real store; \
             {} recorded a cost nothing here shows",
            unprobed.len()
        );
        for (to, why) in &unprobed {
            println!("v{to}: a cost this law does not exhibit — {why}");
        }
    }

    #[test]
    fn every_migrated_generation_is_one_load_can_dispatch() {
        // The const assertion holds the pairing; this holds the POPULATION —
        // that there is at least one of each kind, so the note above is a
        // sentence about something and the dispatch below is not empty.
        let mut additive = 0;
        let mut migrated = 0;
        let mut breaking = 0;
        let mut gated = 0;
        for g in GENERATIONS {
            match g.cost {
                Cost::Additive => additive += 1,
                Cost::Migrated => {
                    migrated += 1;
                    assert!(
                        g.migrate.is_some(),
                        "v{} is migrated with nothing to run",
                        g.to
                    );
                }
                Cost::Breaking => breaking += 1,
                Cost::Gated => gated += 1,
            }
        }
        assert!(additive > 0 && migrated > 0 && breaking > 0 && gated > 0);
        assert_eq!(
            additive + migrated + breaking + gated,
            GENERATIONS.len(),
            "the match above is the whole population, so a new cost must be counted here"
        );
    }
}
