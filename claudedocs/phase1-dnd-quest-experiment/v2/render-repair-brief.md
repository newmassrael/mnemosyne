# Render-repair brief — re-render only the deficient scenes, warm

You are an adventure-prose author doing a TARGETED re-render. An existing road of
a dungeon-delve was already written as warm played prose; a few scenes were
flagged as deficient and the underlying structure of those scenes has been
repaired. Re-render ONLY those scenes from the repaired outline, in the same
warm voice, so they splice seamlessly into the existing prose.

Read EXACTLY these and NO other file under `claudedocs/phase1-dnd-quest-experiment/`:
- this brief
- `/home/coin/mnemosyne/claudedocs/phase1-dnd-quest-experiment/v2/run/manuscripts/world-claim.md` (the REPAIRED structural outline of the CLAIM road — what is now true per scene, with disclosure annotations)
- `/home/coin/mnemosyne/claudedocs/phase1-dnd-quest-experiment/v1/run/render/world-claim.md` (the EXISTING warm prose — match its voice exactly)
- `/home/coin/mnemosyne/claudedocs/phase1-dnd-quest-experiment/v2/run/render/affected-scenes.txt` (the EXACT list of scene ids to re-render — re-render THESE and only these)

## What to do

For each `## sc-NN` heading listed in `affected-scenes.txt`, write a NEW warm
prose version of that scene from the repaired outline, matching the voice,
tense, register, and character voices of the existing v1 prose so a reader cannot
tell the seam. The repairs you are realizing in prose (the outline now supports
them — render them as lived scenes, do not announce them):
- the delver's journal now reads as his recorded INVESTIGATION reaching the
  shrine-keeper (a tracked case), not a verdict dropped in;
- the rogue SHARES with the party the incense detail he overheard from the
  wise-woman, and the wizard's case against the shrine-keeper rests on the
  journal + the shared tithe-lore + that shared clue;
- the wizard names the three real roads consistently (unmake / claim / oath),
  the oath being the third, not a fourth thing;
- the CLAIM ending COMMITS: the party's grasp for the crown delivered the Reach
  to the schemer who betrayed them — a specific, earned doom — rather than "the
  same end whichever way you turned it".

Honor the same rules as the original render: cover each listed scene under its
exact `## sc-NN` heading; say only what each scene and its people know; never
voice a withheld truth before its scene; stay on the CLAIM road; distinct
consistent voices; no meta-commentary, no bracketed notes, only the scene
markers.

## Deliverables (in v2/run/render/)

- `scenes/sc-NN.md` for EACH affected scene (just that scene's prose, its
  `## sc-NN` heading preserved) — OR a single `repaired-scenes.md` with each
  affected scene under its heading. The orchestrator will splice these into v1's
  prose; do NOT re-emit the unaffected scenes.
- `render-log.md` — a short note on how you matched the v1 voice and any
  ambiguity.

When done, reply with a SHORT summary: which scenes you re-rendered and a line on
how you kept the voice continuous. Do not paste the full prose.