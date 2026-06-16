# Vocabulary — allowed IDs and fork structure ONLY

Use ONLY these ids when you record facts. Do not invent ids. (This file lists the
identifiers and the fork; it tells you nothing about the story's content — that is
what you read from the prose.)

## Frames (whose knowledge a fact is)
- `gt` — ground truth (what the narration presents as simply true)
- `sera`, `halden`, `bearer` — Founding-Age people's points of view
- `ode`, `marn`, `pell`, `tamsin` — Withering-Age people's points of view

## Entities (people, places, props)
- places: `cistern-steps`, `loom-quarter`, `wend-fields`
- props: `cistern-vent`, `boundary-stone`, `gorge-sapling`, `wend-pipe`, `cistern`, `ford`
- persons: `sera-person`, `halden-person`, `bearer-person`, `ode-person`, `marn-person`, `pell-person`, `tamsin-person`

## Predicates (for typed facts)
- `condition` — the state a thing is in (e.g. a cistern's condition: rotting / sealed / clean). Type as `{subject:<entity>, predicate:"condition", object:{kind:"literal", value:"<state>"}}`.
- `possession` — who/what holds a thing.

## Branches (world-lines) and the fork
- Two world-lines branch at scene **sc-26** (the fork): `planted` and `barren`.
- The prose you are given is ONE reading: the shared Founding scenes (`sc-01`..`sc-26`)
  then the `planted` future (`sc-27p`..`sc-40p`).
- Tag a fact's `branch`:
  - Founding scenes `sc-01`..`sc-26` → branch `main` (the shared spine, before the fork). Omit branch or use `main`.
  - Withering scenes ending in `p` (`sc-27p`..`sc-40p`) → branch `planted`.
- `canon_from` for each fact = the `## sc-NN` heading it appears under in the prose.
