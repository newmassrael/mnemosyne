# Render-expand brief — render the new prologue, warm, never naming the figure

You are an adventure-prose author. An existing dungeon-delve road was already
written as warm played prose. A PROLOGUE has been added (the last delver's
investigation, before the party arrives) and one or two existing scenes were
touched. Render the NEW/affected scenes in the SAME warm voice so they splice in
seamlessly, and obey one hard rule about a withheld secret.

Read EXACTLY these and NO other file under `claudedocs/phase1-dnd-quest-experiment/`:
- this brief
- `/home/coin/mnemosyne/claudedocs/phase1-dnd-quest-experiment/v3/run/manuscripts/world-claim.md` (the expanded outline — what is true per scene, with disclosure annotations; the prologue scenes are sc-00a..)
- `/home/coin/mnemosyne/claudedocs/phase1-dnd-quest-experiment/v2/run/render/world-claim.md` (the existing warm prose — match its voice exactly)
- `/home/coin/mnemosyne/claudedocs/phase1-dnd-quest-experiment/v3/run/render/affected-scenes.txt` (the EXACT scenes to render)

## The hard rule (a withheld secret — do not break it)

The prologue shows the delver tracking a ROBED FIGURE who feeds the dead into the
floodwater. The figure's IDENTITY (who they are) is a SECRET the story does not
reveal until a much later scene (sc-14). In the prologue you must:
- write the figure ONLY as "the robed figure", "the hooded shape", "someone of
  the town" — NEVER name them, NEVER show their face, NEVER call them the
  shrine-keeper or tie them to the shrine by name;
- you may have the trail lead TOWARD the upper town / the shrine quarter, but the
  delver is cut off / caught / drowns BEFORE he confirms who it is — the reader
  must leave the prologue knowing the DEED and that the hunter got close, but NOT
  who the figure is.
A later check re-reads your prose and verifies a reader cannot identify the
villain from the prologue. If your prose lets the reader conclude it is the
shrine-keeper, it fails — keep the figure unnamed and the trail short of the
reveal.

## What to render

For each `## sc-NN` heading in `affected-scenes.txt` (the prologue scenes + any
touched existing scene), write warm played prose under the exact heading,
matching the v2 prose's voice, tense, register. The prologue is the delver's POV
— a lone hunter in the dark under-ways; give it its own quiet dread, consistent
with the book's voice. For the touched existing scene(s) (e.g. sc-14 where the
journal now pays off the prologue), render so the journal reads as the record of
the hunt the reader just witnessed.

Honor the render rules: cover each listed scene under its exact heading; say only
what each scene and its people know; the prologue is BEFORE the party, so only the
delver (and the narration) are present; never name the figure; no meta-commentary
or bracketed notes, only `## sc-NN` markers.

## Deliverables (v3/run/render/)

- `scenes/sc-NN.md` for each affected scene (just that scene under its heading).
- `render-log.md` — how you kept the v2 voice and the figure unnamed.

Reply with a SHORT summary: the scenes you rendered, approximate prologue word
count, and confirmation the figure is never named/face-shown. Do not paste the prose.