# Workflow — How a Typical Session Looks

This is the canonical session pattern for AI agents working with
Mnemosyne.

## Session start

1. Read concept resources you haven't internalized yet
   (`mnemosyne://concepts/*`).
2. Run `validate_workspace` to surface the current baseline (prose
   orphans, style violations, atomic orphan refs).
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
4. Report metrics: orphan delta, T3 warn delta, atomic orphan refs.

Skipping step 3 is the most common failure — the user gets confirmation
"done!" and the next session discovers a T1 reject.

## Section references

Reference another section by its id: `§N` (e.g. `§2.4`). Verify the target
exists first — `list_sections` is the section space.

(The `[workspace] docs` / `default_doc` multi-doc markdown model, and the
default_doc fallback for cross-doc `§N`, were removed in Round 400 with
GENERATED.md. The store is the single directly-validated artifact; there is
no doc list to resolve against.)

## Adding a new ChangelogEntry

1. Pick a monotonic `entry_id` (e.g. next `Round N` or `ADR-NNNN`).
2. Provide:
   - `decision_summary` — 1 sentence headline
   - `changes_bullets` — what changed (file paths, primitives, etc.)
   - `verification_bullets` — how the change was validated
   - `impact_refs` — `§A,§B` of affected sections
   - `carry_forward_bullets` — anything pending for next round
3. Call `append_changelog_entry`.
4. Run `validate_workspace`.

## When validate_workspace fails

| Metric | Diagnosis |
|---|---|
| New T1 orphan | Cross-ref target missing — fix §N or add the target section |
| T2 frozen-ledger violation | You touched an existing ChangelogEntry — append a new one instead |
| Round-trip mandatory failure | Parse → emit → re-parse not byte-equal — typically heading/list formatting drift |
| T3 warn count rose | New style violation — fix the prose or accept the warning |

## Pre-commit integration

This repo ships its hooks under `.githooks/` — install with
`git config core.hooksPath .githooks`. For an adopting project, the gate to run
depends on **which half you are in**:

```bash
mnemosyne-cli validate-workspace     # the SPEC half: sections, changelog, cross-refs, bindings
mnemosyne-cli validate-continuity    # the NARRATIVE half: frame-scoped continuity + declared rules
```

Exit 0 = consistent. Non-zero = mutation needed before commit.

**`validate-workspace` is NOT the narrative gate** — it never looks at facts,
frames, branches or disclosure. A consumer authoring a playable story ran only
`validate-workspace`, saw it green, and believed their world was checked; the
gate that actually checks it is `validate-continuity` (plus
`validate-disclosure-leak` / `validate-render-fidelity` for a telling). Run the
one that matches what you are authoring; run both if you author both.

(`verify-generated`, which this page recommended until R622, was removed in
Round 400 along with the GENERATED.md model. It has not existed for ~220
rounds.)

## Don't do this

- Don't `Read` the atomic JSON. Use `query_section`.
- Don't `Edit` the atomic store JSON. Mutate via the typed tools.
- Don't append a ChangelogEntry without `impact_refs`. Empty
  impact_refs are nearly always a sign of incomplete planning.
- Don't run `validate_workspace` once at the start and assume that's
  enough. Run it after every mutation.
