# Workflow — How a Typical Session Looks

This is the canonical session pattern for AI agents working with
Mnemosyne.

## Session start

1. Read concept resources you haven't internalized yet
   (`mnemosyne://concepts/*`).
2. Run `validate_workspace` to surface the current baseline (orphans,
   style violations, round-trip status).
3. If the user asks "what's the state?", surface the metrics from (2)
   plus the section topology via `list_sections`.

## Before any mutation

1. Run `query_section(<section_id>, include_related=true,
   include_changelog=true)` on the section you intend to touch.
2. Verify the section exists and isn't strong-carry / frozen.
3. Read the `decision_status` — `"Active"` only; `"Superseded"`
   sections must not be edited (they're historical record).

## Session-level mutation pattern

Each user request typically maps to:

```
plan → tool calls → validate_workspace → report
```

Specifically:

1. **Plan** the change in conversation (which section, which field,
   what content).
2. Call the **typed primitive** (e.g. `set_section_intent`).
3. Call **`validate_workspace`** to confirm no new T1/T2 violations.
4. Report metrics: orphan delta, T3 warn delta, round-trip status.

Skipping step 3 is the most common failure — the user gets confirmation
"done!" and the next session discovers a T1 reject.

## Cross-doc references

When authoring new content that references another section:

- Within the same doc: `§N` (e.g. `§2.4`).
- Across docs (default_doc): `§N` (resolves to default_doc fallback).
- Across docs (non-default): `[text](other.md#anchor)` markdown link.

The parser auto-classifies these into `RefKind::Decision`, `Impl`, or
`CrossDoc`. You don't author the kind; you author the markdown form.

## Adding a new ChangelogEntry

1. Pick a monotonic `entry_id` (e.g. next `Round N` or `ADR-NNNN`).
2. Provide:
   - `decision_summary` — 1 sentence headline
   - `changes_bullets` — what changed (file paths, primitives, etc.)
   - `verification_bullets` — how the change was validated
   - `impact_refs` — `§A,§B` of affected sections
   - `carry_forward_bullets` — anything pending for next round
3. Call `append_changelog_entry_v2`.
4. Run `validate_workspace`.

## When validate_workspace fails

| Metric | Diagnosis |
|---|---|
| New T1 orphan | Cross-ref target missing — fix §N or add the target section |
| T2 frozen-ledger violation | You touched an existing ChangelogEntry — append a new one instead |
| Round-trip mandatory failure | Parse → emit → re-parse not byte-equal — typically heading/list formatting drift |
| T3 warn count rose | New style violation — fix the prose or accept the warning |

## Pre-commit integration

Recommend that the user install a git pre-commit hook that runs:

```bash
mnemosyne-cli verify-generated && mnemosyne-cli validate-workspace
```

Exit 0 = workspace consistent. Non-zero = mutation needed before
commit.

## Don't do this

- Don't `Read` the atomic JSON. Use `query_section`.
- Don't `Edit` GENERATED.md. Mutate via tools; cascade auto-update will
  refresh it.
- Don't append a ChangelogEntry without `impact_refs`. Empty
  impact_refs are nearly always a sign of incomplete planning.
- Don't run `validate_workspace` once at the start and assume that's
  enough. Run it after every mutation.
