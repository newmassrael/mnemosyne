# Mnemosyne — Architecture & North Star

> **Status: forward-looking target.** This document fixes *where the
> architecture is going*, not where every crate is today. Current code is
> mid-convergence toward this target (see §5). When a decision here conflicts
> with current code, this document states the intent and the code is the debt
> to be paid — never the reverse.
>
> Human-facing companion docs: `GETTING_STARTED.md`, `docs/SCHEMA_GUIDE.md`.
> Audit trail / decision history: the atomic-store changelog (single source).

## 1. North Star

> **AI and humans co-author knowledge — specifications *and* narratives, where a
> specification is itself one kind of narrative — on a single bitemporal ·
> branchable · auditable typed-fact substrate. Media are swapped via adapters
> (design_doc / spec / ADR / fiction); domain meaning is plugged in (logic =
> SCE/SCXML, code = tree-sitter); and every artifact — readable docs, executable
> SCXML logic IR, generated code — is a *projection* of that one fact substrate.**

Mnemosyne is not a markdown tool. It is the **management / memory / version-
control layer** for evolving knowledge. Its sibling project
[`scxml-core-engine` (SCE)](../scxml-core-engine) supplies the **formal logic
layer**: extended SCXML (Forge kind-system) as a universal IR + multi-language
AOT codegen + the NL↔IR meta-layer between AI and humans. Together they form an
**AI–human co-authoring substrate**: SCE = "what the system *does*" (formal,
verifiable, executable); Mnemosyne = "what is *true when*, what *changed*, and
*why*" (versioned, branchable, auditable).

### 1.1 What "one substrate" means (stated so it is not re-derived)

- **A specification is one *instance* of a narrative, not a coordinate sibling.**
  Both are a consistency-managed, evolving fact-base whose artifacts must
  conform. "Does code match spec?" is the most tractable instance — the
  beachhead the substrate is hardened on first. A narrative manager embedded in
  a game engine is the encompassing target. **spec ⊂ narrative**; the North Star
  does not change as the medium widens.

- **Facts are multi-axis.** There is the actual / historical fact, and there is
  each agent's *understood* fact — distinct facts on distinct axes, **both
  true**. "A believes the king lives" and "the king is dead" is not a
  contradiction; it is two facts. Frames are never cross-validated for
  agreement, and an agent's behaviour derives from *its own* frame, not from
  ground truth.

- **`branch` is the epistemic-frame mechanism**, not version control: coexisting
  perspectival truths (agent A / agent B / ground truth), each internally
  consistent, never merged into one. This is why the bitemporal / branch
  foundation is load-bearing (§6 invariant 1), not speculative.

- **Validation is frame-scoped.** Within a frame, consistency is strict
  (violations reject). Across frames, disagreement is *data*, not a violation.
  The cross-frame invariant is: every actor's action derives from its own frame.

## 2. The three models are one system

| Model | What it is | Where it lives in the architecture |
|---|---|---|
| **Existing (substrate)** | bitemporal, branch, cascade, audit over typed facts | **Layer 0** — the fact core |
| **Narrative** | fiction + technical both first-class; branch / saga; many media | **Layer 1** — medium adapters |
| **Plugin** | domain meaning plugged in (SCE logic, code symbols) | **Layer 2** — capability plugins |

They are three faces of one statement: *manage one fact over time; swap the
medium with an adapter; plug in domain meaning; project everything else.*

## 3. Target architecture — Ports & Adapters (Hexagonal), 4 layers

```
┌─ Layer 0  CORE — bitemporal typed-fact substrate ─────────────────────┐
│  one canonical fact model (Section / ChangelogEntry / FrozenList /     │
│  CrossRef). Event-sourced: an append-only fact log is the SSOT.        │
│  branch · valid-time / transaction-time · cascade (incremental         │
│  projection) · proposal→gate→audit (every mutation = reviewed txn).    │
│  Knows nothing of markdown, SCXML, fiction, or specs. Pure domain.     │
└───────────────────────────────────────────────────────────────────────┘
        ▲ port: MediumAdapter            ▲ port: CapabilityPlugin
┌───────┴───────────────────┐   ┌────────┴──────────────────────────────┐
│ Layer 1  ADAPTERS          │   │ Layer 2  PLUGINS (domain meaning)      │
│ medium ↔ canonical facts   │   │ isolated via Transport                 │
│ design_doc (today)         │   │ in-process / MCP / CLI                 │
│ spec · ADR · fiction       │   │ tree-sitter (code symbols, exists)     │
│                            │   │ SCE (store/diff spec logic as SCXML)   │
└────────────────────────────┘   └────────────────────────────────────────┘
                       │ projection (cascade, incremental)
┌──────────────────────┴────────────────────────────────────────────────┐
│ Layer 3  VIEWS — projections of Layer-0 facts                           │
│ readable docs (EPUB + query) · logic IR (SCXML) · generated code        │
│ (SCE) · reports · Studio UI                                             │
└─────────────────────────────────────────────────────────────────────────┘
```

### Textbook patterns applied
- **Ports & Adapters (Hexagonal):** Layer 0 is a pure domain; adapters and
  plugins sit at the boundary. The core defines the ports (`FactStore`,
  `MediumAdapter`, `CapabilityPlugin`); concrete implementations depend inward.
- **Dependency Inversion:** dependencies point toward the core abstractions,
  never outward toward formats or engines.
- **Single Source of Truth:** the canonical typed-fact log is the only truth.
  SCXML IR, generated code, the RocksDB index, reports — all derived.
- **Event Sourcing + bitemporal:** the append-only fact log *is* the event log
  (this is the existing frozen-ledger principle); current state is a fold;
  valid-time and transaction-time are separate axes.
- **CQRS:** the write side (proposal → gate → audit → log append) is distinct
  from the read side (cascade-recomputed projections).
- **Plugin process isolation:** trusted/fast meaning runs in-process; external,
  multi-language engines (SCE: Python/Rust/Go runtimes) run behind MCP/CLI
  transport — `core::Transport { InProcess | Mcp | Cli }` is exactly this seam.

## 4. Persistence — log is SSOT, RocksDB is the materialized index

The substrate is **event-sourced**:

```
SSOT  (git-native, reviewable)        RocksDB  (rebuildable, gitignored)
append-only fact log            ──→   materialized index keyed by
(canonical facts as JSON;             (branch_id, entity_id, valid_from)
 = frozen changelog ledger)           → fast point-in-time / branch / cascade
```

- The **fact log lives in git as text** — AI and humans review mutations by
  diffing it. This preserves the dogfood's core property (spec change review
  via version control) and *is* the event log.
- **RocksDB is a derived materialized view** rebuilt from the log: the 24-byte
  `(branch_id, entity_id, valid_from)` composite key + per-CF version history
  scans give the bitemporal / branch / cascade queries that a flat JSON file
  cannot serve at scale. Being gitignored is **correct** — a rebuildable index
  is never the source of truth.
- A flat JSON file works only because today's dogfood corpus is tiny (one
  design_doc, one branch). Branching bitemporal narrative needs the index.
- **Future (not a fork now):** at extreme corpus size, add log *snapshots* so
  the index need not replay the whole log on load. Optimization, not a redesign.

## 5. Current state vs target — the debt to converge

The fact model began as **two type definitions** — one for the live JSON
authoring store (`mnemosyne-atomic`), one for the RocksDB index codec
(`mnemosyne-facts`; the structs themselves moved to `mnemosyne-core` in R328).
They are now **connected in production**: `mnemosyne-atomic::project` (R329) folds
the live store into the canonical `*Fact` vocabulary, and that projection has two
real consumers — the warm read-side validation service (`mnemosyne-projection`,
R339) and the RocksDB materialized index (`mnemosyne-index::rebuild_index`,
R332). So the earlier *type*-level deviation is resolved for every projected
entity; what remains is routing live point queries through the index (convergence
B, below). (A third, Salsa per-entity face — `SectionInput` /
`ChangelogEntryInput` / `FrozenListInput` — was removed in R322; the coarse
snapshot engine that briefly succeeded it was itself retired in R338, leaving
`fine_grained.rs` as the sole cascade engine, consuming `*Fact` values directly.)

The two faces overlap far less than a "modeled twice" framing suggests, and the
right convergence differs per entity (R327 corrected this from an earlier
over-statement):

| Concept | `mnemosyne-atomic` (JSON, live) | `mnemosyne-facts` (index codec) | Convergence |
|---|---|---|---|
| Section | `AtomicSection` = `SectionSkeleton` + design_doc content | `SectionFact` embeds `SectionSkeleton` | **done** (R325/R326): one shared skeleton |
| CrossRef | inline `AtomicSection.impact_scope` | first-class `CrossRefFact` relation rows | adapter-divergent by design (R326); projected at index build |
| ChangelogEntry | `AtomicChangelogEntry` — audit + publishable bullet halves, keyed by prose `entry_id` | `ChangelogEntryFact` — scalar `round_number` / `summary` | **settled (R329/R330)**: `project_changelog_entry_facts` parses `round_number` from the prose key and takes `summary` from the audit `decision_summary`; R330 dropped the unsourced `appended_at`. The two still share *no* struct — the canonical scalar shape was defined by the projection consumer, as planned |
| FrozenList | *none* — frozen-ledger is a behavioral semantic (the `FrozenLedger` mutate-reject), not a stored entity | `FrozenListFact` + cascade `FrozenListRecord` | forward-looking substrate; no live counterpart to unify (YAGNI until a real consumer) |

So **convergence A (unify the fact model) is complete for the only entity that
had genuine cross-face duplication — Section.** ChangelogEntry has no shared
skeleton to lift today: its atomic face carries medium-shaped design_doc content
keyed by prose, while its fact face is pure scalars that exist nowhere in the
live store. Forcing a shared struct now would either strand a one-embedder type
or demand a frozen-ledger schema migration — both premature ahead of the consumer
that defines the right shape. That consumer is **convergence B** (the
projection). FrozenList has no atomic representation at all, so there is nothing
to reconcile until a real frozen-list consumer exists.

Other consequences today: the RocksDB index is materialized from the atomic log
by `mnemosyne-index` (R332) and exercised through its admin binary, but the live
`query` / `validate` / `ops` read paths still scan the JSON store / in-memory
projection rather than routing point queries through the index — that routing is
convergence B's remaining half. A tier-gate concept exists both in
`validate-workspace` (real) and `mnemosyne-server` (Phase-0 stub); the write path
that once bridged them through a broken `commit` hashing stub was removed in R321.

The substrate components are **well-built**, and the read side is now **wired**:
`store` is a correct bitemporal/branch KV; `cascade` (the `fine_grained` Salsa
engine since R338) drives the warm validation projection (R339). They are not
dead code.

### Convergence sequence (each step independently verifiable)
- **A — unify the fact model. _Done._** One canonical skeleton carrying both
  serde (the JSON log) and the byte codec (the RocksDB index), for every entity
  that has two genuine faces. R323 hoisted `FactKey`; R325 made the live model
  compose `SectionSkeleton`; R326 made the index codec encode the same
  `SectionSkeleton`. Section was the only such entity — ChangelogEntry's faces
  share no fields and FrozenList has no atomic face, so their reconciliation is
  owned by B (the projection that defines the canonical shape), not a
  pre-emptive shared struct (R327).
- **B — RocksDB as materialized index. _Projection + materialization landed
  (R329/R330/R332); live query-routing scale-gated._** The projection is written:
  `project_*_facts` folds the atomic log into `*Fact` values (R329) and
  `mnemosyne-index::rebuild_index` persists them under the composite key (R332);
  ChangelogEntry's scalar shape was settled by that consumer (R330), and the
  bitemporal substrate is now exercised end-to-end (rebuild → point query) by the
  index admin binary. The index is **not yet the live query source** — the
  `query` / `validate` paths still scan the JSON store, which the warm in-memory
  projection (C) already serves at dogfood scale. Routing point queries through
  the durable index is the remaining half, deferred until corpus scale needs the
  composite-key lookup a flat scan cannot give (§4, invariant #3). The index
  stays a *derived, rebuildable* view — never a second authoritative store.
- **C — cascade as incremental projection.** _Status half done (R335);
  architecture decided (R337)._ `SectionRecord` already carries the typed
  `Option<DecisionStatus>`. The remaining incremental half is a **read-side
  projection service** — a warm Salsa host behind the §3 Transport seam — built
  jointly with D. The audit's two open questions (R336) are resolved in
  *Incremental projection architecture* below: engine = `fine_grained.rs`
  (coarse `runtime.rs` retired). The validation projection landed (R339–R341);
  the former markdown-render target was retired with the markdown-doc model
  (R400), so C's read side now serves validation + reports + the future Studio,
  not a markdown render.
- **D — unify the write path.** Atomic mutate primitives + proposal→gate→audit
  reconcile into one command path: append log (RocksDB-free) → notify the
  read-side projection service → incremental index update + cascade recompute. D
  shares C's read-side driver and is **co-designed with C** (R337), not a later
  sequential step.

### Incremental projection architecture (C + D keystone, R337)

R336 surfaced two questions for C; R337 resolves them and the C/D split, derived
from the cost-no-object textbook + Phase-0 mandate (AI reads/mutates spec
efficiently):

- **CQRS split.** The write side (the `cli` / `ops` mutate primitives) appends to
  the git-native fact log — the SSOT — and stays **RocksDB-free** (R328). The
  read side owns the RocksDB index and the Salsa cascade DB and *projects*. They
  never share a process edge that drags RocksDB into authoring; they meet at the
  `core::Transport { InProcess | Mcp | Cli }` seam (§3).
- **Warm host, because Salsa memoization is in-process.** Incremental recompute
  pays off only in a *live* process — a one-shot CLI rebuilds cold every
  invocation. So C is a **read-side projection service**, not a new edge from the
  authoring CLI. The host binary is bound when the first warm consumer is built;
  MCP (the AI's long-running entry point) is the natural first home, and a
  standalone `mnemosyne-server` daemon stays deferred until a consumer needs it
  (YAGNI, invariant #3). The `cli` keeps its one-shot full-rebuild path for
  human / CI use — correct for that scale, and it keeps authoring RocksDB-free.
- **Engine = `fine_grained.rs`.** The genuine per-entity `#[salsa::input]` design
  is C's engine; the coarse `runtime.rs` (monolithic `snapshot_payload`,
  non-incremental by construction) is retired at C's implementation
  (no-legacy-carry), and its measurement test moves to the fine-grained engine.
- **Layer-1 content can feed the projection engine as Salsa inputs.** The engine
  is a Layer-3-producing *read model*, not the domain-agnostic Layer-0 core, so
  supplying it the design_doc content (intent / rationale / …) a richer view needs
  does not breach invariant #4. Validation needs only the Layer-0 skeleton; a
  Layer-1 read model (e.g. the Studio views) adds the Layer-1 inputs.
- **Persisted vs in-process state.** The RocksDB index (B) is the durable,
  cross-process read model, updated by applying only the log delta. Within a warm
  session, Salsa memoizes the recompute. Both are derived and rebuildable — the
  log remains the single source of truth.
- **Sequencing (no half-finished).** (1) A **validation** projection through the
  warm service first — Layer-0 only, the cheapest real projection, proving the
  warm-host + RocksDB-free split end-to-end (walking skeleton); landed R339–R341.
  (2) The original step 2 was an incremental markdown render to `GENERATED.md`;
  that target was removed with the markdown-doc model (R400). The next Layer-1
  read projection is the Studio views, built with their consumer.

### Canonical fact-model boundary (A's keystone, R323–R326)

`A` splits along a strict Layer-0 / Layer-1 line so the core stays
domain-agnostic (§1):

- **Layer 0 — canonical scalar skeleton (`mnemosyne-core`).** The bitemporal
  identity `FactKey { branch_id, entity_id, valid_from }` (R323) and the
  medium-neutral *scalar* `SectionSkeleton` (R325; scoped to scalars in R326):
  `title`, parent links, `decision_status`. These serialize identically across
  every adapter, which is what makes the type safely shareable.
- **Layer 1 — medium content (design_doc adapter = `mnemosyne-atomic`).** The
  rich design_doc fields — `intent`, `rationale`, `inputs`/`outputs`, `caveats`,
  `alternatives`, `examples`, `normative_excerpt`, `bindings`,
  `publishable_*` — are *shaped by the design_doc medium* (a fiction or ADR
  section carries different content) and stay in the adapter, never in Layer 0.

Cross-refs are deliberately **not** in the shared skeleton: they are
adapter-divergent — the JSON log stores them inline as `AtomicSection.impact_scope`,
the index stores them as first-class `CrossRefFact` relation rows — so each
adapter owns its own representation (a shared value object holds only what every
embedder persists the same way). The supersession forward-pointer
(`AtomicSection.superseded_by`, R342) joins `impact_scope` as a second inline
adapter cross-ref, projected to a `decision`-kind `CrossRefFact` — see *Supersession
cross-ref convergence* below.

R325–R326 unified the Section fact: `AtomicSection` embeds
`mnemosyne_core::SectionSkeleton` via `#[serde(flatten)]` (byte-identical JSON),
and `SectionFact` embeds the same `SectionSkeleton` behind a full-fidelity byte
codec — the log and the index now share one skeleton definition for Section.
ChangelogEntry and FrozenList do **not** get a copy of this treatment (R327): a
shared skeleton is the right tool only when both faces already persist the same
scalars, as Section's did. ChangelogEntry's faces share no fields and FrozenList
has no atomic face, so their canonical shape is settled by convergence B (the
projection consumer), not lifted pre-emptively. R335 completed convergence C's
status half: the cascade Salsa input (`SectionRecord`) now carries the typed
`Option<DecisionStatus>` directly, retiring the `as_str` + None→"active" string
bridge (the lone surviving `as_str` caller is the read-side `SectionView` string
projection). The skeleton discipline is what lets new media (fiction, ADR, spec)
become first-class adapters without the core ever learning what a "rationale" or
a "normative excerpt" is.

### Supersession cross-ref convergence (R342)

A Superseded section forward-points to the decision that replaced it. The
atomic-axis gate (`atomic_section_supersede_state_reject`) encodes the invariant
*Superseded ⟹ an outbound `decision`/`impl` cross-ref exists*. At R342 the
pointer had **no structural home**: `set_section_decision_status` validated
`--superseding §M`'s *presence* and then discarded it, so the only surviving
trace of the replacement target was whatever `§M` citation the author typed into
free prose, recovered only by the markdown re-parse (itself since removed in
R400). The warm read-side projection (R339), which reads **only** the atomic
store, could never see a `decision`-kind ref and therefore over-flagged *every*
Superseded section. That was a single-source-of-truth break: the supersession
relation lived in the markdown projection, not the canonical store.

**Decision — model the pointer as a first-class adapter cross-ref field**
`AtomicSection.superseded_by: Option<String>`, placed beside `impact_scope` and
following the identical adapter-divergent pattern (inline in the JSON log,
projected to a `CrossRefFact` at index build). `set_section_decision_status`
becomes the single write path: it stores the target on the `→ Superseded`
transition and clears it on `→ Active`/`→ Removed`, so the
`decision_status`/`superseded_by` pair cannot drift. `project_cross_ref_facts`
emits a `ref_kind = "decision"` relation for the pointer; the fine-grained
cascade already accepts `decision`/`impl` outbound refs
(`section_decision_violation`), so the engine needs no change and the projection
stops over-flagging. The atomic-axis gate now reads `superseded_by` from the
store (the SSOT) instead of re-parsed markdown; the relation is a stored fact,
surfaced through `query` and any read-side view rather than living only in prose.

**Rejected alternatives.** *(A) A `SectionSkeleton` scalar* next to
`decision_status`: would force the pointer into the index codec, producing a
double representation (once in `SectionFact.skeleton`, once as the projected
`CrossRefFact`) and contradicting the scalar-only skeleton boundary above.
*(C) A general first-class cross-ref store* (an `AtomicSection` cross-ref vector
with typed `ref_kind`): the broader "adapter-divergent cross-refs" convergence,
deferred as YAGNI — supersession is the only divergent ref with a live consumer,
and a cardinality-1 lifecycle-coupled pointer does not need a general relation
collection. `impact_scope` keeps its own field; supersession keeps its own.

## 6. Anti-drift invariants

1. **Never delete the bitemporal foundation** (`store / facts / cascade /
   server`). It is the North Star's substrate, not dead weight. "Unused by
   today's dogfood" ≠ "wrong long-term." Converge onto it; do not amputate it.
2. **The log is the SSOT; everything else is a projection.** Never reintroduce
   a second authoritative store. RocksDB stays a rebuildable index.
3. **Don't pre-build speculative upper layers.** The SCE plugin's IR-exchange
   protocol, the narrative branch/saga API, and `MediumAdapter` methods beyond
   what a concrete medium needs are designed *when a real consumer exists* — not
   ahead of it. YAGNI gates the upper layers; it does not justify deleting the
   foundation.
4. **Core stays domain-agnostic.** Markdown, SCXML, fiction, and spec knowledge
   live in adapters/plugins, never in Layer 0.
5. **Phase 0 reaches the target foundation; Phase 1 builds narrative + SCE on
   top of it.** Get the substrate textbook-correct first.
