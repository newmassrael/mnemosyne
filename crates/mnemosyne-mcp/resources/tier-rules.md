# Tier Rules — T1 / T2 / T3 / T4

The validator runs four severity tiers. Each tier has different reject
power and a different recovery path.

## T1 — Semantic (always reject)

Cross-ref integrity. A mutation is rejected if it would create an
**orphan cross-ref** (a `§N` reference whose target does not exist in
the workspace).

Lookup priority (intra → workspace → atomic store):

1. Atomic store: §N exists in `atomic_id_set`.
2. Not there → **reject**.

(The multi-doc markdown model — `[workspace] docs` / `default_doc` and
its cross-doc fallback — was removed in Round 400. The store is the
sole section space, so the lookup has one step.)

Recovery: either fix the §N reference, or add the target section
first.

## T2 — Structural (always reject)

Three rules:

- **changelog_entry_append_only**: existing ChangelogEntry sub_bullets
  cannot be removed or modified (jaccard inclusion).
- **frozen_ledger_jaccard**: same check at the 5-atomic-field
  granularity (Round 161+).
- **frozen_list_membership_delta**: FrozenList membership changes
  require a new ChangelogEntry attachment.

Recovery: append a new ChangelogEntry instead of editing.

## T3 — Convention (warn-only by default)

Style rules — `max_paragraph_length`, `max_sentence_length`,
`bullet_list_preference`, `terminology_consistency`, etc. Configurable
thresholds via `[style] thresholds` in `mnemosyne.toml`.

Reject power can be flipped per-rule if a project wants strict style.
Default = warn so an external project's existing prose isn't rejected
on day 1.

Run via `style_check` tool. T3 produces *warnings* in the validation
report; the workspace still passes overall.

## T4 — Informational (info-only)

Suggestions. Bullet-list preferences over run-on paragraphs, anchor
conventions, etc. Never reject. May be silenced.

## Hard-case exemption

A section can be marked **HARD_CASE** to exempt it from T3/T4 rules
where the rule would mechanically misclassify (e.g. a code-fence
example that legitimately exceeds `max_paragraph_length`).

## Tier mobility ratify (Round 138)

Tier classification is fixed at audit time. Once a violation is logged
as T3-warn, it stays T3-warn even if a future round redesigns the rule.
The rationale: tier mobility would invalidate cross-round comparisons.

## Tool-level surface

| Tool | Tiers it runs |
|---|---|
| `validate_workspace` | T1 + T2 + T3/T4 (store-direct) |
| `style_check` | T3 + T4 only |
| `set_section_*` mutate | T1 + T2 pre-write |
| `append_changelog_entry` | T1 + T2 pre-write |
