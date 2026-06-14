# Extraction brief — read ONLY this file and the one story file named in your task

## Firewall (binding)

Work only inside `claudedocs/phase1-disclosure-craft-experiment/v2/`. Use ONLY
this brief and the single story file your task names. Do NOT read any other file
in this repository — no changelog, no design docs, no runbook, no manifest, no
other story, no `mnemosyne-cli ... query --list-changelog`. Do not ask why this
task exists. Save your output only where this brief says.

## What you are doing

You are re-extracting the GROUND-TRUTH facts a piece of prose **explicitly
states**, into a Mnemosyne fact store, using a FIXED vocabulary. You type only
what the text says in plain words — never what it merely hints, implies, or
leaves for the reader to infer. Every fact cites the scene it came from and a
short verbatim quote as evidence.

## The fixed vocabulary (use these ids EXACTLY — do not invent new ones)

**Frames** (whose-knowledge a fact belongs to):
- `gt` — ground truth: what the narrative establishes as actually having
  happened (a confirmed reveal, a stated fact of the world).
- `hale` — the investigator's working theory (a belief he holds, not yet
  confirmed true by the narrative).
- `pike` — a particular character's false belief.

Extract **`gt` facts only** for this task. Skip a statement if the prose frames
it as a character's unconfirmed theory/suspicion rather than an established
truth.

**Entities** (the `--entities` list AND typed subject/object use these ids):
- `crane` (person — Dr. Aurel Crane, the director, found dead)
- `pike` (person — Onslow Pike, the bursar)
- `junia` (person — Junia Frost, assistant astronomer)
- `hale` (person — Lewin Hale, insurance adjuster)
- `climber` (person — the UNNAMED figure who climbed the iron stair at 3:14;
  the identity is the story's central mystery)
- `dome-key` (object — the spare dome key reported lost)
- `telegram` (object — the forged patron's telegram of recall)
- `letter` (object — Crane's unsent resignation letter)
- `night-log` (object — the night-log for the observation)
- `account` (object — the instrument account)
- `clock` (object — the meridian clock stopped at 3:14)
- `plate-camera` (object — the plate-camera and its blank plate)

**Predicates** (`--typed-predicate`; each fact may carry ONE typed claim):
- `identity` (object = an entity id) — the real identity of an
  otherwise-unnamed figure. Use for "the climber was X": subject `climber`,
  object-entity the named person. ONLY when the prose plainly NAMES who climbed.
- `key-custody` (object = an entity id) — who holds the spare dome key. subject
  `dome-key`, object-entity the holder.
- `knows-dismissal` (object = scalar `yes`|`no`) — whether Crane knew he was
  being dismissed/recalled. subject `crane`. Use `no` only when the prose plainly
  establishes he never knew (e.g., the recall telegram never reached him).
- `lifestate` (object = scalar, e.g. `dead`) — alive/dead state. subject the
  person.
- `location` (object = scalar, e.g. `gallery`) — where a character physically
  was on the death-night. subject the person.

A fact does NOT need a typed claim — but a fact that matches one of the five
predicates above SHOULD carry it (that is the point of this extraction). Facts
with no matching predicate: record them with `--entities` and a `--claim`, no
typed leg.

## Scene coordinates

The story file has `## sc-XX` headings (e.g. `## sc-01`, `## sc-17r`,
`## sc-09b`). Use that heading's id verbatim as the fact's `--canon-from` and
`--evidence`. A fact belongs to the scene where the prose STATES it (the
discourse position), not where it was true in backstory. If a conclusion is
only made explicit at a late reveal scene, its canon-from is that late scene —
do NOT back-date it to where the event happened.

## How to record each fact

Start from the skeleton store (it already holds the vocabulary, zero facts):

```
cp skeleton.atomic.json <OUT>            # your task names <OUT>
mnemosyne-cli add-fact --fact <unique-id> --frame gt \
  --claim "<one sentence in your own words of what the prose states>" \
  --canon-from <sc-XX> --evidence <sc-XX> \
  --entities <id,id,...> \
  [--typed-subject <id> --typed-predicate <id> (--typed-object-entity <id> | --typed-object-value <scalar>)] \
  --quote "<short verbatim phrase from the scene>" \
  --sidecar <OUT>
```

(Use the workspace `mnemosyne-cli` already on PATH. `--typed-subject` must also
appear in `--entities`.)

Rules:
- EXPLICIT text only. If the scene does not plainly state who climbed, do NOT
  emit an `identity` fact for that scene — silence is correct, not a gap to fill.
- One fact per distinct stated conclusion. Aim for the load-bearing facts of
  each scene, not every descriptive sentence.
- Cover EVERY `## sc-XX` scene in the file (all world-line limbs present).
- Never guess which "version" of the story this is; just extract what is on the
  page.

## Deliver

Write the single store your task names (e.g. `extract/A.reextract.atomic.json`).
At the end, print: the number of facts you added, and a list of every
`identity` / `knows-dismissal` typed fact you emitted with its `--canon-from`
(scene) so the count can be checked. Nothing else.
