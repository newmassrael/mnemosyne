# Mnemosyne — AI Agent Workflow Guide

This file is auto-read by Claude Code at every session start. When working
on the Mnemosyne project, this instruction takes precedence.

## Phase 0's *real* goal

**Make AI agents (Claude / future LLM) read + mutate spec efficiently.**

- ≠ human readability
- ≠ writing tutorials for newcomers
- ≠ making spec more *concise*
- ≠ "first-time readers can understand"

The atomic store + GENERATED.md = *audit trail + AI domain*. Density is the
essence; it is *not* written for human newcomers.

Human-facing surfaces = `GETTING_STARTED.md` / `SCHEMA_GUIDE.md` (separate
artifacts, already exist).

## Anti-patterns — *never recommend / proceed*

The following are *all* Phase 0 framing violations. Self-check before any
recommendation.

### ❌ "atomic store is dense, let's clean it up"
- atomic store = audit trail genre. Density is the essence.
- AI reads via DB query; humans don't read it from start to finish.
- *Reasonable termination point* = T3 reject = 0 + cross-ref consistency.
 Beyond that is an anti-pattern.

### ❌ "make it readable for first-time readers"
- That's `GETTING_STARTED.md` / `SCHEMA_GUIDE.md` territory; already exist.
- Not the purpose of the atomic store / GENERATED.md.

### ❌ "rewrite the body of a Round N entry to be shorter"
- Frozen ledger violation.
- Audit trail information loss.
- Round 19 frozen ledger principle carries stable.

### ❌ "split this paragraph because it looks long" (ignoring semantic preservation)
- LLMs can parse dense paragraphs fine.
- Splitting may damage *semantic layers*.

### ❌ "drive T3 warn / T4 info to 0"
- Round 138 tier mobility ratify carry: T3 reject = 0 enforced; T3 warn /
 T4 info = acceptable carry zone (author review discretion).
- Trying to drive to 0 = anti-pattern.

### ❌ "split atomic store across multiple files"
- Cross-ref graph fans out drastically.
- Single source-of-truth contract carries.

### ❌ "unify all Round entries to a standard template" (frozen zone)
- Retroactive change to existing entries = frozen ledger violation.
- Enforcing a template on *new* entries OK (Round 162 schema decomposition
 scope), but body mutation of existing entries = 0.

## ✅ Correct patterns — recommend path

- Improve AI query efficiency (e.g. indexed cache, faster lookup, multi-hop graph)
- Strengthen mutate API safety (e.g. T2 frozen ledger automation, atomic
 field validation)
- Cross-ref graph traversal efficiency (1-hop / multi-hop)
- Enforce frozen ledger consistency on the audit trail
- Add atomic fields to the DB schema (for semantic preservation enforcement)
- Remove hardcoding from production code (config-driven, external user path)
- Create new external-user-facing artifacts (`GETTING_STARTED.md` /
 `SCHEMA_GUIDE.md` etc., as *separate files*, not body mutation)
- Create human-facing dashboards (`STATUS.md` / `DECISIONS.md` etc., as
 *separate artifacts*, auto-generated via DB query)

## Cleanup hard limit

Reasonable termination point for cleanup:
- T3 reject = 0
- T1 cross-ref orphan = 0 (outside the known-stale ledger)
- round-trip mandatory = N/N (all configured docs)

Passing these 3 conditions = cleanup complete. Further *prose tidying*
attempts are anti-patterns.

## Self-check questions (run before any task)

If any of the following is *not* yes — anti-pattern suspected, *confirm
with user before proceeding*:

1. Does this work improve AI workflow efficiency / mutate API safety /
 query efficiency?
2. Is this work *separate* from human readability concerns?
3. Does this work *not touch* the frozen ledger zone (existing Round N
 entry bodies)?

If any answer is no — *confirm with user before starting*. Cleanup-loop
recurrence is a real risk zone.

## Decision flow — when *human readable* gap surfaces

If a human reports they cannot access information:
- → Create a *separate artifact* (`STATUS.md` / `DECISIONS.md` /
 `CHANGELOG_SUMMARY.md` / `FAQ.md` / `TUTORIAL.md` etc.)
- → *Not* body mutation of existing atomic store entries
- → Prefer DB-query auto-generation when possible (cascade auto-update
 Stage 4 alignment)

## Progress history location

The atomic store changelog_entries (post Round 251 MD-DELETION-RATIFY) =
single source of truth. On entering a new conversation, read the changelog
via `mnemosyne-cli query` first (Round 127 dogfood proof carries — direct
grep / Read of GENERATED.md not required).

## Mutate API enforcement (Round 127 carry)

All spec mutation routes through `mnemosyne-cli` mutate API:
- `append-changelog-entry-v2` (add a Round N entry, atomic)
- `set-section-intent` / `set-section-rationale` / etc. (atomic Section
 primitives)
- `add-section` / `add-cross-ref` (legacy markdown surgical insert,
 pre-251)
- `set-section-decision-status` (Active → Superseded)
- `set-section-body` (legacy markdown body update, T2 frozen ledger gate)

Direct `Edit` / `Write` on the atomic store JSON or generated artifacts =
0 calls enforced. Exception: explicit user *override grant* (Round 126
option (iii) escape hatch).
