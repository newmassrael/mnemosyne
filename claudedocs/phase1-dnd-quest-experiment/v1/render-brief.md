# Render brief (Stage 2) — play a slice of the adventure as warm prose

You are an adventure-prose author. You will be given STRUCTURAL OUTLINES of a
dungeon-delve — a shared opening (the trunk) and ONE chosen road through to its
ending — each a sequence of scenes with, per scene, what is true, who is
present, what they know, and which quest-threads are in play. Your job is to
write it out as warm, played PROSE: the kind of scene-by-scene narration a good
game would give a player living the adventure.

**Read ONLY this file and the outline files you are given. Do not open any other
file under `claudedocs/phase1-dnd-quest-experiment/`.** Work in `run/render/`;
leave one prose file per outline.

## What to write

For each scene (each `## sc-NN` heading in the outline, in order), write the
scene as it is PLAYED: the place, what the party sees and does, the people they
meet and what is said, the turn the scene takes. Warm and grounded — a told
adventure, not a bulleted summary. Carry the QUESTS as a player would feel them:
when a quest is offered, let it be offered in the fiction (someone asks, a door
is found locked); when one is finished, let the finish land as an event.

## The rules that keep it honest

- **Cover every scene in the outline, in order, under its `## sc-NN` heading.**
  Keep the headings exactly as given so the scenes stay aligned.
- **Say only what THIS scene and its people know.** The outline tells you, per
  scene, what each person knows and what is still hidden. Do not let the
  narration or any character state a truth the outline marks as not-yet-known on
  this road — a secret revealed at scene 25 must not be voiced at scene 8, and a
  character must not speak knowledge they had no way to come by. Write the
  not-knowing as real: characters act on what they believe, including when they
  are wrong.
- **Stay on this road.** You are writing ONE road. Do not pull in events that
  happen only on a different road of the fork.
- Write in a consistent register throughout. Distinct, consistent voices for
  the party and the NPCs. No meta-commentary, no headings beyond the `## sc-NN`
  scene markers, no answer keys, no bracketed notes — just the played scenes.

## Deliverables (in `run/render/`)

- One `world-<road>.md` per outline (e.g. `world-trunk.md` + `world-parley.md`,
  or whatever the outlines are named) — the played prose, `## sc-NN` headings
  preserved.
- `render-log.md` — a short note on the voice choices you made and anything in
  the outline you found ambiguous.
