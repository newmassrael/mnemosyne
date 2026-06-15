# Extractor brief (Stage 3) — re-extract the facts a reader would take from the prose

You are a careful reader. You will be given three story manuscripts (three roads of
the same shut-in coastal mystery) and a VOCABULARY (a list of the people/objects and
the relation-types that appear in this world, with ids). Your job is to read the prose
and record, as structured facts, **exactly what the prose tells a reader** — scene by
scene, point of view by point of view — using the given ids.

**Read ONLY this file, the manuscripts in `run/render/`, and the vocabulary file you
are given (`run/extract/vocab.md`). Do not open any other file under
`claudedocs/phase1-npc-dialogue-experiment/`** — especially not any original fact base,
plan, or outline. You must extract from the PROSE ALONE; that is the whole point. Work
in `run/extract/`.

## What "exactly what the prose tells a reader" means

- **EXPLICIT only.** Record a fact only if the prose actually states or shows it on the
  page. Do NOT infer what is probably true, what is foreshadowed, or what you suspect.
  If a scene only hints, record the hint as the belief/suspicion it is, in the suspecter's
  point of view — not as established truth.
- **Point of view.** Tag each fact with the FRAME it belongs to, using the vocabulary's
  frame ids: `gt` for what the prose presents as actually-having-happened (narration the
  reader is meant to take as real), and a person's frame id for what THAT person says,
  knows, or believes in the prose. A line of dialogue is a fact in the speaker's frame.
- **Scene coordinate.** Set each fact's `canon_from` to the `## sc-NN` heading of the
  scene where the prose states it. Use the scene ids exactly as the manuscripts mark
  them. Record a fact at the EARLIEST scene where the prose actually states it.
- **World-line.** Each manuscript is one road: `world-report.md` → branch `report`,
  `world-shelter.md` → `shelter`, `world-confront.md` → `confront`. Tag a fact with the
  branch of the file it came from — UNLESS it appears in the shared opening scenes that
  are identical across all three files, in which case omit `branch` (it is on the spine).
- **Typed claims.** When a fact is about one of the relation-types in the vocabulary
  (e.g. a `possession` relation between a person and an object), record it `typed` with
  the matching predicate id and entity ids from the vocabulary. This is how your reading
  is checked against the bible — use the given ids precisely; do not invent new ones for
  things the vocabulary already names.

## Produce a re-extracted store (same tooling as an author)

Work in `run/extract/`. Build a fresh store from your reading:

1. Empty seed `reextracted.atomic.json`:
   ```json
   {"sections":{},"changelog_entries":{},"inventory_entries":{},"confirmation_events":{},"frames":{},"branches":{},"entities":{},"predicates":{},"narrative_facts":{},"disclosure_plans":{},"schema_version":23}
   ```
2. `sections.json` — one section per distinct `## sc-NN` you saw across the manuscripts.
   `import-sections --manifest sections.json --sidecar reextracted.atomic.json`.
3. `facts.json` — the frames / branches / entities / predicates you used (copy the ids
   from the vocabulary) and your extracted `facts` array (each with `frame`, optional
   `branch`, `claim`, `canon_from`, `evidence:[its own scene]`, `entities`, and `typed`
   where it applies). `import-facts --manifest facts.json --sidecar reextracted.atomic.json`.
   (One atomic transaction; if it rejects, fix and re-run.)

Use the branch fork structure from the vocabulary's `forks_at` so your spine/branch
split matches the scene ids.

## Deliverable (in `run/extract/`)

- `reextracted.atomic.json` (the re-extracted store), `sections.json`, `facts.json`.
- `extract-log.md` — note anything the prose left genuinely ambiguous, and any place a
  scene seemed to state a thing in one person's mouth that (from the prose alone) they
  seemed to have no way to know. Record it; do not fix it. You are a mirror, not an
  editor.

Extract faithfully and completely. Record what the prose says — no more (do not infer)
and no less (do not omit a stated fact).
