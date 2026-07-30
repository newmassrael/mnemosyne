# dnd-quest fixture — the authored map seam, at the current schema

`facts.json` here is the ONE input `dnd_quest_map_seam_smoke.rs` needs that the
frozen experiment record cannot supply. Everything else the rebuild reads —
`sections.json`, `order.json`, `narrative-rules.json` — is read straight out of
`claudedocs/phase1-dnd-quest-experiment/v3/run/author/`, unchanged and uncopied,
because a byte-identical second home would be free to drift.

## Why it exists

The R562/R566 dnd-quest experiment is the only store that ever AUTHORED
quest-giver surfaces — the `DisclosureSurface {scene, object}` half of the map
seam. Every other store, including the first playable consumer's, exercises the
seam only through the derived canon-coordinate seat.

That store stopped loading and nobody noticed for ~150 rounds, because nothing
in the workspace ever loaded it. The manifest is at schema 23; the removals it
trips over landed one at a time; and the store copies that would have shown it
(`store.atomic.json`, `pin.atomic.json`) are gitignored, so the only tracked
inputs could not rebuild it either.

The migration below is therefore half the repair. The other half is the smoke
test: a fixture that CI never loads is a fixture that rots again.

## Provenance

Derived once, mechanically, from two frozen artifacts in
`claudedocs/phase1-dnd-quest-experiment/v3/run/author/`:

| source | what came from it |
|---|---|
| `facts.json` (tracked) | every frame, branch, entity, predicate, and fact |
| `pin.atomic.json` (gitignored) | the `delve` disclosure plan, verbatim |

Neither was modified: they are the blind author's record and the experiment's
sha-pinned evidence.

## What the migration changed, and nothing else

1. **5 `object_kind: "scalar"` predicates → `token`** (R708 removed
   `PredicateObjectKind::Scalar`), each with the closed `object_tokens`
   vocabulary DERIVED from the objects the author actually wrote:
   `cause_of_rising` · `warden_disposition` · `betrayal` · `rising_state` ·
   `reach_rule`. That set is a floor — what this store asserts — not a claim
   about what the predicate may legally take. A later road wanting a new value
   must declare it, which is the point of the closure.
2. **18 `{kind: "value", value}` objects → `{kind: "token", token}`** (R708
   removed `TypedObject::Value`).
3. **`entity_kinds` declared** — `item` · `person` · `place` · `quest`, derived
   from the authored entities. Entity kind became a registry ref rather than
   free text (R732/R738), so the four the author used now need declaring.
4. **The `delve` disclosure plan added** — 7 overrides, 4 of them the
   quest-giver surfaces, with `first_at` moved from the schema-23 map shape
   `{branch: section}` to the all-primitive `[{branch, coords[]}]` list.

Point 4 is not a re-wire. **The tracked audit manifest never carried a
disclosure plan at all** — the blind author wrote its plan directly into its own
store, so the 4 surfaces that are the map seam's only authored instance existed
in two gitignored files and nowhere else. This file is now their only tracked
home, which is the rot the round found rather than the one it went looking for.

## What it proves when it loads

`report-quest-graph --telling delve` reproduces what the ledger recorded for
R569 exactly: `q-key`, `q-delver`, and `q-reliquary` giving-bound, and `q-main`
honestly `unresolved` on its split encoding — no heuristic rescue. All 4
surfaces resolve to MapLocators on all 4 world-lines, 0 unplaced and 0
undecidable.

It also supplies an instance the substrate did not otherwise have: the quest
`open` verdict — the one a quest LOG renders — appears 5 times here. The first
playable consumer's store yields none (its manuscripts are finished, not
save-states), so before this fixture that verdict rested on unit fixtures alone.
