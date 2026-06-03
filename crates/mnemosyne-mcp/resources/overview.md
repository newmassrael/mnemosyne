# Mnemosyne — Overview for AI Agents

## What it is

Mnemosyne is a design-doc lifecycle infrastructure. Its mission is to make
**LLM-driven markdown management** feasible at production scale: an AI agent
must be able to read, mutate, and validate a project's design docs without
silently corrupting structure or losing audit history.

The shape that achieves this is the **atomic store** (post Round 400 the
single directly-validated source of truth):

- The atomic store (`docs/.atomic/workspace.atomic.json`) is the SSOT —
  typed structured records (Section / ChangelogEntry / FrozenList / CrossRef)
  with append-only audit semantics. Humans read the spec content from a
  committed **EPUB** and the changelog via `mnemosyne-cli query` (the
  markdown-rendered GENERATED.md was removed in Round 400).
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
- T1 prose cross-ref orphan reject + T2 atomic frozen-ledger reject
- T3/T4 style checks (advisory), store-direct over the atomic store
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
(the atomic store). If you find yourself reaching for `Edit` or `Write`
on the atomic store JSON, stop — the correct action is a Mnemosyne tool
call.
