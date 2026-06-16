# Extractor brief — re-extract facts from the played prose

You are a careful reader. You will be given PROSE manuscripts (the played scenes
of a dungeon-delve) and a small VOCABULARY file (`vocab.md`) listing the allowed
entity / predicate / frame IDS and the fork structure ONLY. Your job is to read
the prose and write down, as a fact base, ONLY what the prose EXPLICITLY states —
no inference, no filling gaps, no knowledge from outside the text.

**Read ONLY this file, the prose manuscripts you are given, and `vocab.md`. Do
not open any other file under `claudedocs/phase1-dnd-quest-experiment/`** — in
particular there is an author's fact base you must NOT see; the whole point is an
independent re-reading.

## Method

- Go scene by scene, in the order the `## sc-NN` headings appear. For each fact
  the prose STATES, record it with `canon_from` = the `## sc-NN` heading it
  appears under.
- Record a fact ONLY if the prose says it outright. If the prose only hints or
  implies something, do NOT record it as a fact — this is a test of what the
  prose actually commits to. If a character SPEAKS a fact (says who did the
  thing, names the cause), record it as known at that scene.
- Use the frame for whose knowledge it is: ground truth (`gt`) for what the
  narration presents as simply true; a person's frame for what that person
  knows/believes/says.
- Use ONLY the ids in `vocab.md`. Tag each fact's `branch` with the road the
  manuscript is (the vocab names the roads and their fork scene). Type a fact
  (`typed:{subject,predicate,object}`) whenever the prose makes a clear
  subject-predicate-object claim using a vocab predicate (who holds the crown,
  who did the deed, who pursues which quest).

## Deliverables (in `run/extract/`)

- `reextracted.atomic.json` — a schema-23 store built from your re-extraction
  (the empty seed + your `sections.json` + `facts.json` imported), OR the
  `sections.json` + `facts.json` themselves if you cannot run the tool. canon_from
  = the prose `## sc-NN` markers.
- `extract-log.md` — anything the prose left genuinely ambiguous, and any place a
  character voiced knowledge that seemed to come from nowhere in the text.
