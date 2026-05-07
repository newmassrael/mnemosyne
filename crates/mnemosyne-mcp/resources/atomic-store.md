# The Atomic Store

## Location

Single JSON file: `docs/.atomic/workspace.atomic.json`. Configurable via
`[atomic] sidecar_path` in `mnemosyne.toml`.

## Shape

```json
{
  "schema_version": 1,
  "sections": {
    "<section_id>": {
      "intent": "...",
      "rationale_bullets": [...],
      "inputs_bullets": [...],
      "outputs_bullets": [...],
      "caveats_bullets": [...],
      "alternatives_rejected": [...],
      "impact_scope": ["§A", "§B"],
      "examples": [...]
    }
  },
  "changelog_entries": {
    "<entry_id>": {
      "decision_summary": "...",
      "changes_bullets": [...],
      "verification_bullets": [...],
      "impact_refs": ["§A"],
      "carry_forward_bullets": [...]
    }
  }
}
```

## Why this shape (genre = audit trail, not narrative)

The atomic store is a **dense, append-only audit ledger**. It is not
designed for sequential human reading. Density is the essence — every
field carries semantic weight, and history is preserved by accumulation.

Consequences:

- AI reads the store via **DB queries** (tools below), not by `Read`-ing
  the JSON top to bottom.
- The "looks dense / let's clean it up" instinct is wrong here. The
  store is meant to be dense. Human-facing readability lives in
  `docs/GENERATED.md` and external guide files.
- Audit trail integrity > prose tidiness. A `set_section_*` call appends
  to the store; existing entries stay frozen.

## How to read it

| Goal | Tool |
|---|---|
| Look up one section's full content | `query_section(section_id)` |
| List all section_ids in workspace | `list_sections()` |
| Find which entries cite a section | `query_section(.., include_changelog=true)` |
| Find related sections (1-hop crossref) | `query_section(.., include_related=true)` |

## How to mutate it

Always through typed primitives. Each tool corresponds to one atomic field:

| Field | Tool |
|---|---|
| Section.intent | `set_section_intent` |
| Section.rationale_bullets | `set_section_rationale` |
| Section.inputs_bullets | `set_section_inputs` |
| Section.outputs_bullets | `set_section_outputs` |
| Section.caveats_bullets (append) | `add_section_caveat` |
| Section.alternatives_rejected | `set_section_alternatives` |
| Section.impact_scope | `set_section_impact_scope` |
| Section.examples (append) | `add_section_example` |
| ChangelogEntry (new) | `append_changelog_entry_v2` |

## Direct JSON edits

**Forbidden by default**. The atomic store contract requires mutation
to route through validated primitives so that:

- T1 cross-ref orphan check runs.
- T2 frozen-ledger jaccard check runs.
- Every mutation is reflected in GENERATED.md atomically.

If a user *explicitly* grants an override, you may edit the JSON
directly. Otherwise, refuse and call the appropriate tool.

## Cascade auto-update

Every successful atomic mutation auto-regenerates `docs/GENERATED.md`
(template render → atomic write → byte-identical to a fresh
`generate_docs` call). The `verify_generated` tool exits 0 when in sync,
1 when stale — wire this into pre-commit hooks.
