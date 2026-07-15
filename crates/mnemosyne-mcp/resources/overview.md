# Mnemosyne — Overview for AI Agents

## What it is

Mnemosyne is an **AI-first authoring substrate**: a store of typed facts an
agent can read, mutate, and validate without silently corrupting structure or
losing audit history. It carries **two halves over one substrate**, and you
almost certainly need to know which half you are in:

- **The spec half** — design-doc lifecycle: sections, an append-only
  changelog, cross-refs, code bindings, verification. Gated by
  `validate-workspace`.
- **The narrative half** — authoring a *story world*: frames (whose belief),
  branches (which world-line), entities, predicates, narrative facts,
  disclosure plans (what the reader/player is told, and when), quests, and a
  playable-world projection. Gated by **`validate-continuity`** — NOT by
  `validate-workspace`, which never looks at it.

Both persist in the atomic store (`docs/.atomic/workspace.atomic.json`), the
single directly-validated source of truth post Round 400. Humans read spec
content from a committed **EPUB** and the changelog via `mnemosyne-cli query`
(the markdown-rendered GENERATED.md was removed in Round 400). Every mutation
goes through a typed primitive (e.g. `set_section_intent`) which validates
against tier rules (T1/T2/T3/T4) before persisting.

**Do not infer the schema from this document.** Call `describe_schema` — it
emits the authoring contract (record types, fact shape, fixed vocabularies,
rule classes, quest encoding, write-time invariants) derived from the code,
so it cannot go stale the way a prose summary can. This page once claimed the
store had "4 typed entities, closed-form"; a consumer believed it and rebuilt
the narrative half in Python.

## Why this shape

Plain markdown editing by AI is unsafe at scale:
- A bad regex collapses bullet structure.
- A `## Heading` rename silently breaks 200 cross-refs.
- An "improvement" rewrites a frozen ledger entry and loses history.

The atomic store + tier rules turn each of those failures into a typed
reject *before* the mutation lands.

## What works

Counts are deliberately absent here — a hand-maintained tally is what drifted
last time. Ask the code:

- `describe_schema` — the authoring contract (both halves), derived, static.
- `mnemosyne-cli --help` — every dispatched verb, gated against the dispatch
  by `tests/help_covers_dispatch_smoke.rs`.
- `list_changelog` — the decision ledger, newest last. Round entries live
  here, NOT in `list_sections` (which lists spec sections only).

Load-bearing surfaces, by half:

- **Spec** — T1 prose cross-ref orphan reject + T2 atomic frozen-ledger
  reject; T3/T4 style checks (advisory), store-direct; 5-language code emit
  (Rust authoritative; Kotlin / Python / C++ / Protobuf reference);
  spec↔code binding + verification/confirmation ledger.
- **Narrative** — frame-scoped continuity gate (`validate-continuity`);
  declarative narrative rules (`Exclusive` / `Transition` / `Interval`);
  setup/payoff coverage; disclosure plans + leak/fidelity gates; fork tree,
  playable-world and quest-graph projections; typing/edge discovery reports
  that package work for an LLM proposer.

## Concepts you must internalize

Read these resources in order:

1. `mnemosyne://concepts/atomic-store` — what the store is, why dense audit
2. `mnemosyne://concepts/frozen-ledger` — append-only invariant
3. `mnemosyne://concepts/tier-rules` — T1/T2/T3/T4 hierarchy
4. `mnemosyne://concepts/anti-patterns` — what NOT to do (critical)
5. `mnemosyne://concepts/schema-guide` — `mnemosyne.toml` schema
6. `mnemosyne://concepts/workflow` — how a typical session looks

These pages are prose, and prose goes stale. If you are about to conclude
"Mnemosyne has no way to record X", call `describe_schema` FIRST and search
the ledger (`list_changelog`) for the project's own word for X — the axis is
often there under a name you did not guess (world state = `Exclusive` rules
on typed claims; unlock = the quest `requires` predicate; what-the-player-was-
told = disclosure plans; causality = the setup/`pays_off` edge). Seven of
seven such conclusions by the first playable consumer were wrong.

## Identity reminder

**You are not editing markdown.** You are appending to a typed audit log
(the atomic store). If you find yourself reaching for `Edit` or `Write`
on the atomic store JSON, stop — the correct action is a Mnemosyne tool
call.
