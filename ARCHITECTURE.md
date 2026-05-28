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

> **AI and humans co-author knowledge — specifications *and* narratives — on a
> single bitemporal · branchable · auditable typed-fact substrate. Media are
> swapped via adapters (design_doc / spec / ADR / fiction); domain meaning is
> plugged in (logic = SCE/SCXML, code = tree-sitter); and every artifact —
> readable docs, executable SCXML logic IR, generated code — is a *projection*
> of that one fact substrate.**

Mnemosyne is not a markdown tool. It is the **management / memory / version-
control layer** for evolving knowledge. Its sibling project
[`scxml-core-engine` (SCE)](../scxml-core-engine) supplies the **formal logic
layer**: extended SCXML (Forge kind-system) as a universal IR + multi-language
AOT codegen + the NL↔IR meta-layer between AI and humans. Together they form an
**AI–human co-authoring substrate**: SCE = "what the system *does*" (formal,
verifiable, executable); Mnemosyne = "what is *true when*, what *changed*, and
*why*" (versioned, branchable, auditable).

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
│ readable docs (GENERATED.md) · logic IR (SCXML) · generated code        │
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
  `GENERATED.md`, SCXML IR, generated code, the RocksDB index — all derived.
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

The biggest deviation from this architecture is a **triplicated fact model**:
the same domain concept is modeled up to three times.

| Concept | `mnemosyne-atomic` (JSON, live) | `mnemosyne-facts` (RocksDB codec) | `mnemosyne-cascade` (Salsa) |
|---|---|---|---|
| Section | `AtomicSection` | `SectionFact` | `SectionInput` + snapshot |
| ChangelogEntry | `AtomicChangelogEntry` | `ChangelogEntryFact` | `ChangelogEntryInput` |
| FrozenList | (in `AtomicStore`) | `FrozenListFact` | `FrozenListInput` |
| CrossRef | (in `AtomicSection`) | `CrossRefFact` | — |

Consequences today: two unreconciled persistence models (JSON atomic store =
live; RocksDB store = built-but-orphaned), a write path that bridged them only
through a now-removed broken `commit` hashing stub, and a tier-gate concept that
exists both in `validate-workspace` (real) and in `mnemosyne-server` (stub).

The substrate components are **well-built and kept** — `store` is a correct
bitemporal/branch KV; `cascade` is a correct incremental-projection seed. They
are not dead code; they are *not yet wired*.

### Convergence sequence (each step independently verifiable)
- **A — unify the fact model (keystone).** One canonical fact set carrying
  serde (log), byte codec (index), and Salsa input. Everything depends on this.
- **B — RocksDB as materialized index.** Project the log into the composite-key
  store; route queries through it instead of full-JSON scan.
- **C — cascade as incremental projection.** Replace full re-render with Salsa
  incremental recompute on log change.
- **D — unify the write path.** Atomic mutate primitives + proposal→gate→audit
  reconcile into one command path (append log → update index → cascade).

### Canonical fact-model boundary (A's keystone decision, R323–R324)

`A` splits along a strict Layer-0 / Layer-1 line so the core stays
domain-agnostic (§1):

- **Layer 0 — canonical skeleton (`mnemosyne-core`).** The bitemporal identity
  `FactKey { branch_id, entity_id, valid_from }` (landed R323) plus the
  medium-neutral attributes every fact has regardless of medium: `title`,
  parent links, `decision_status`, and cross-refs. This is all the core knows.
- **Layer 1 — medium content (design_doc adapter).** The rich design_doc fields
  — `intent`, `rationale`, `inputs`/`outputs`, `caveats`, `alternatives`,
  `examples`, `normative_excerpt`, `implementations`, `publishable_*` — are
  *shaped by the design_doc medium* (a fiction or ADR section carries different
  content) and live in the Layer-1 `MediumAdapter` payload, never in Layer 0.

`AtomicSection` today conflates skeleton + content in one struct. The A3 code
round splits it: skeleton → `mnemosyne-core`, content → the design_doc adapter.
The on-disk `workspace.atomic.json` serde representation must stay byte-identical
across the split (the round-trip gate is the guard). This is what lets new media
(fiction, ADR, spec) become first-class adapters without the core ever learning
what a "rationale" or a "normative excerpt" is.

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
