# Render brief (Stage 2) — tell the Holm's story in scenes, in the people's own voices

You are a storyteller. You are given a finished story bible for a shut-in coastal
mystery — the people, what is really true, what each person knows and believes, the
scene-by-scene events of three different roads the story can take, and what each road
withholds until its reveal. Your job is to **write it as story** — warm, peopled
scenes a reader would not put down, told largely THROUGH THE PEOPLE TALKING.

**Tell the best story you can.** That is the whole job. Do not think of yourself as
transcribing a list of facts or filling in an outline; think of yourself as the one
who finally tells, in full, a story you know cold. The bible is what you know; the
scenes are you telling it.

**Read ONLY this file and the story inputs you are pointed to (the per-road outlines
in `run/manuscripts/` and the cast/knowledge notes you are given). Do not open any
other file under `claudedocs/phase1-npc-dialogue-experiment/`** — those are experiment
internals and would bias you. Work in `run/render/`.

## What you are given

- **One outline per road** (`run/manuscripts/world-report.md`,
  `world-shelter.md`, `world-confront.md`). Each is that road's events in order,
  scene by scene (`sc-01`, `sc-02`, …). Every beat is tagged with a POINT OF VIEW in
  parentheses — `(gt)` is what is actually true; `(halsa)`, `(officer)`, `(pilot)`,
  `(salt-wife)`, … are what THAT PERSON knows or believes at that point. Some beats
  are marked WITHHELD until a later reveal scene — that is the story's mystery; honor
  it (see below).
- The three roads SHARE their opening scenes (the wreck-night and the first days) and
  DIVERGE at the fork into three different aftermaths. Render all three.

## The voices — this is the heart of it

This is a house of distinct people, not one narrator with a crowd of extras. Make the
reader able to tell who is speaking with the dialogue tags hidden:

- **Give each significant person a voice of their own** — rhythm, diction, what they
  reach for and what they will not say. The riding-officer does not talk like the
  salt-wife; the old pilot does not talk like the boy; the headman speaks for the Holm
  and chooses his words; the stranded factor's man is a mainlander among islanders.
  Let those differences live in HOW they speak, not in narrator labels.
- **Keep each voice CONSISTENT** across all their scenes and across all three roads.
  A person's way of speaking, their temper, what they care about, should hold from
  their first scene to their last — the same person on every road.
- **Let people speak only from what they know.** Each person's lines must come from
  THAT person's point of view in the outline — what they witnessed, were told, or
  could reasonably guess. A person must not say (or knowingly imply) a thing only
  someone else saw, or a thing from a road they are not on. When someone is wrong,
  let them be confidently wrong in their own voice. This is not a constraint on the
  drama — it is the drama: a houseful of people each speaking their own fragment.

## The mystery — withhold what the outline withholds

The central truth of the wreck-night (the WITHHELD beats) must NOT be stated, by
anyone, before its reveal scene on that road. Before the reveal, characters may
suspect it, circle it, lie about it, fear it — but no line may plainly tell the reader
the withheld solution early. At the reveal scene, let it land. (Different roads reveal
— or never reveal — different things; follow each road's outline.) This is just good
mystery-telling; write it as such.

## Keep the Holm peopled to the last scene

The early scenes are crowded; do not let the late scenes — each road's aftermath after
the fork — empty out to two or three principals talking in a room. The Holm is a
living place on every road: the salt-wife, the boy, the pilot, the widow, the factor's
man, the headman and the rest are still present and still themselves in the aftermath,
still speaking their own fragment as the road closes. If the outline gives a secondary
person a beat in a tail scene, let them speak it in their own voice. A road should be
as peopled at its end as at its start.

## Form

- Write **one file per road**: `run/render/world-report.md`, `world-shelter.md`,
  `world-confront.md`.
- Inside each file, mark every scene with a heading line exactly `## sc-NN` (the same
  scene ids as the outline), then the scene's prose beneath it, in canon order. Keep
  the scene ids — they anchor the story to its bible.
- Warm, readable narrative prose carried by dialogue. Description and interiority are
  welcome where they serve the scene; but this is a story told largely through people
  talking, so let the dialogue do the heavy lifting of character, knowledge, and
  movement.
- Do not add an author's note, a key, a summary, a cast list, or any out-of-story
  scaffolding — only the scenes. Do not invent NEW plot facts that contradict the
  bible (you may add ordinary texture — weather, gesture, the smell of the salt-pans —
  that the bible does not speak to); the spine of who-did-what, who-knows-what, and
  what-each-road-decides is fixed by the bible.

## Deliverable

`run/render/world-report.md`, `world-shelter.md`, `world-confront.md`, plus a short
`run/render/render-log.md` noting how you approached the voices and anything you found
hard. Tell it well.
