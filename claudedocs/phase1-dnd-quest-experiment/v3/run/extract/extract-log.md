# Extract log — re-extraction of `world-claim.md` (CLAIM road, with PROLOGUE)

Independent re-reading per `v1/extractor-brief.md`. Only `extractor-brief.md`,
`v3/run/render/world-claim.md`, and `v3/run/extract/vocab.md` were read. No
author fact base was opened.

## Build summary

- Store: `reextracted.atomic.json` (schema 23), built from the empty seed +
  `sections.json` (`import-sections`) + `facts.json` (`import-facts`).
- **36 scenes** extracted as sections, in `## sc-NN` order: prologue
  `sc-00a..sc-00d`, then the main spine `sc-01..sc-22`, then the CLAIM road
  `sc-23c..sc-32c`.
- **86 facts** extracted (40 with a typed subject-predicate-object leg).
- Registries: 12 frames, 1 branch (`claim`, forks from `main` at `sc-22`),
  25 entities, 9 predicates — all from `vocab.md` only.
- Branch discipline: every fact at `sc-22` and after carries `branch:"claim"`;
  every prologue and pre-fork spine fact carries no branch tag (audited
  programmatically: 0 violations).

## Method notes / frame choices

- `gt` = what the narration states as simply true (e.g. the robed figure tips
  a body into the sluice; the crown wakes the dead; the fork choice).
- `delver` (Coombe) = his prologue knowledge and his stated conclusions (the
  dead are *brought*; the figure is *living* and *of the town*; his journal
  charge to "name the hand").
- Named-party frames (`fighter`/`wizard`/`rogue`/`cleric`), plus `reeve`,
  `wisewoman`, `shrinekeeper`, `lanternboy`, `rival`, `warden` used for what
  each character knows/believes/says in their own scenes.
- Spoken facts are recorded as known at the scene they are voiced (Maeve's
  living-hand lore at sc-06; Pip's incense/shrine-work report at sc-13;
  Lysa reading the journal's naming of Vane at sc-14).
- Typed legs only where the prose makes a clear subject-predicate-object
  claim with a vocab predicate (who holds the crown/key/journal, who pursues
  which quest, what the cause/disposition/rising-state/rule is). Otherwise
  the row is left untyped rather than forcing a reading.

## Audit answers

### 1. Does the PROLOGUE ever NAME or let you identify WHO the robed figure is (before sc-14)?

**No.** The prose explicitly and repeatedly withholds the identity. The figure
is only ever "a hooded shape, all in a long robe ... the face of it lost in the
hood and turned away besides" (sc-00b). Coombe never closes the gap: sc-00c has
him lose the trail "a hand's breadth from the having of it ... the turn of a
face, the say of a name," and sc-00d states it outright — "He could not name the
hand. He had not seen the face ... where the name should go he wrote only that
the figure was someone of the town."

What the prologue **does** commit to (and I recorded as `delver`/`gt` facts) is
narrower than an identification:
- the figure is a *living person*, not a thing of the deep (sc-00b);
- it is *someone of the town* (sc-00b/sc-00c/sc-00d);
- after its work it *climbs up*, on a course bent toward "the upper reaches ...
  the upper town ... the holy quarter above" (sc-00c).

That climb toward "the holy quarter" is a directional hint that *points* in the
shrine's direction, but the prose never names the shrine-keeper, never shows a
face, and Coombe himself refuses to "write down a guess as if it were a
knowing." So: the figure cannot be identified as Brother Vane from the prologue.
The reader is given a witnessed *deed* and a witnessed *direction of escape*, and
an explicit, named *blank* where the identity should be.

### 2. At sc-14 the journal names the villain. Is that naming now GROUNDED in an investigation the reader has SEEN (the prologue), or still a conclusion from a found document with no shown path?

**Partly grounded, but the final link is still an unshown leap.** The prologue
now puts a real, *seen* investigation behind the journal: the reader has watched
Coombe find the fresh dead (sc-00a), witness the robed figure tip a body into the
sluice (sc-00b), follow it climbing toward the upper town / holy quarter
(sc-00c), and write the hunt down before drowning (sc-00d). So when sc-13–sc-14
recover that same journal and Lysa reads it, the "watch kept, a path followed,
the source ... run down at last" (sc-14) is no longer pure off-page assertion —
the reader saw most of that hunt happen.

**However**, the one step the journal supplies at sc-14 — "It had led him to the
shrine. To the shrine-keeper. To Brother Vane." — is exactly the step the
prologue explicitly says Coombe *never took*. He lost the trail before any face
or name (sc-00c/sc-00d) and left the name a deliberate blank. So the journal as
the reader saw it being written could not contain the name "Vane." The naming at
sc-14 therefore still arrives as a conclusion the found document asserts past the
point the shown investigation actually reached. The prologue grounds the *deed,
the method, and the direction*; it does **not** ground the *final identification*,
which the journal nonetheless delivers as a settled name.

I recorded the sc-14 naming as a `wizard`-frame fact (what Lysa reads/concludes
from the journal), not as `gt` and not as `delver` prologue knowledge, precisely
because the prose attributes the name to the document, not to anything the
prologue showed.

### 3. Anywhere else a character knows something the text gave them no path to?

- **sc-14, the journal naming Vane (the main one).** As in Q2: the journal, as
  the reader watched it being written, stopped at "someone of the town" with the
  name left blank (sc-00d). At sc-14 the same journal is read out as having run
  the tracking "to ground at ... Brother Vane." The text shows no scene in which
  Coombe got from the lost trail to the name; the named conclusion exceeds the
  shown investigation. (Recorded under `wizard` frame, fact `f-14-01`.)

- **sc-04 / sc-07 — Brother Vane's true motive (narration, not a character).**
  The `gt` narration states Vane "already had what he wanted and meant to keep
  building it" (sc-04) and that his reliquary "was no seal ... the power he had
  bent a lifetime and a season of the drowned dead toward gathering" (sc-07).
  This is omniscient narration asserting his guilt before any in-world evidence;
  no *character* is given a path to it here. I recorded these as `gt` facts
  (`f-04-02`, `f-07-03`) because the narration states them outright, but they are
  the narrator's knowledge, not earned by any character on the page.

- **sc-06 → sc-13, Maeve's "incense / shrine-work" smell.** Maeve asserts the new
  tithe "smells like shrine-work" (sc-06). The prose gives her no shown path for
  having smelled the underground sluice-tithe from her hut "furthest out"; it is
  delivered as the wise-woman simply knowing. I recorded it strictly as
  `wisewoman`-frame belief (sc-06) and as Pip *reporting* it (sc-13), not as
  ground truth, since the text never substantiates how she knows.

These three are flagged as honest gaps. The first (sc-14) is the load-bearing
one: even with the prologue added, the villain's *name* still arrives from a
document a step beyond where the reader watched the investigation stop.
