# Mnemosyne — Overview for AI Agents

## What it is

Mnemosyne is a design-doc lifecycle infrastructure. Its mission is to make
**LLM-driven markdown management** feasible at production scale: an AI agent
must be able to read, mutate, and validate a project's design docs without
silently corrupting structure or losing audit history.

The shape that achieves this is **atomic-store + GENERATED.md**:

- The atomic store (`docs/.atomic/workspace.atomic.json`) is the
  source of truth — typed structured records (Section / ChangelogEntry
  / FrozenList / CrossRef) with append-only audit semantics.
- `docs/GENERATED.md` is the sole *human-readable* artifact, deterministically
  rendered from the atomic store.
- Every mutation goes through a typed primitive (e.g.
  `set_section_intent`) which validates against tier rules (T1/T2/T3/T4)
  before persisting.

## Why this shape

Plain markdown editing by AI is unsafe at scale:
- A bad regex collapses bullet structure.
- A `## Heading` rename silently breaks 200 cross-refs.
- An "improvement" rewrites a frozen ledger entry and loses history.

The atomic store + tier rules turn each of those failures into a typed
reject *before* the mutation lands.

## Phase 0 (current) — what works

- 4 typed entities (Section / ChangelogEntry / FrozenList / CrossRef)
- 14 atomic mutate primitives
- T1 cross-ref orphan reject + T2 frozen-ledger jaccard reject
- T3/T4 style checks (advisory)
- Round-trip validation (parse → emit → re-parse → typed-fact diff = ∅)
- 5-language code emit (Rust authoritative; Kotlin / Python / C++ / Protobuf reference)

## Concepts you must internalize

Read these resources in order:

1. `mnemosyne://concepts/atomic-store` — what the store is, why dense audit
2. `mnemosyne://concepts/frozen-ledger` — append-only invariant
3. `mnemosyne://concepts/tier-rules` — T1/T2/T3/T4 hierarchy
4. `mnemosyne://concepts/anti-patterns` — what NOT to do (critical)
5. `mnemosyne://concepts/schema-guide` — `mnemosyne.toml` schema
6. `mnemosyne://concepts/workflow` — how a typical session looks

## Identity reminder

**You are not editing markdown.** You are appending to a typed audit log
that *renders into* markdown. If you find yourself reaching for `Edit`
or `Write` on `docs/GENERATED.md` or the atomic JSON, stop — the
correct action is a Mnemosyne tool call.
