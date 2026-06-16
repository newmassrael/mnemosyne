# Extract log — re-reading of `world-claim.md` (CLAIM road)

Independent re-extraction. Recorded ONLY what the prose states outright; no
inference, no gap-filling, no outside knowledge. 32 scenes, 87 facts.

## Knowledge that seems to come from nowhere in the text

These are the places where a character (or the narration in a character's
frame) voices a thing the prose has not laid a textual path to. Flagged per
the brief; NOT silently "fixed."

1. **sc-14 — the journal naming Brother Vane.** The delver's journal is read
   out (frame `wizard`) as having "tracked the fresh tithes ... to their
   source" and the source is named as Brother Vane. The prose presents this
   as written in the dead delver's own journal. Within the text this is the
   single biggest leap: the journal *names* the shrine-keeper. It is stated
   outright (so recorded as `r-048`, frame `wizard` — what the journal says),
   but the prose never shows *how* the delver came to identify Vane
   specifically. The naming arrives as a finished conclusion in the journal.
   Recorded as stated; the chain behind it is not in the text.

2. **sc-14 — Lysa's synthesis citing "the incense Pip had heard."** Lysa
   (frame `wizard`) is narrated assembling the case partly from "the incense
   Pip had heard the old woman mutter" (sc-06). But sc-06 states plainly that
   "Pip alone ... heard" Maeve's incense remark, "said nothing, and stepped
   out." The prose never shows Pip telling the others. So Lysa's frame draws
   on a clue the text says was never shared. Recorded sc-06 as Pip-only
   (`r-029`) and recorded Lysa's synthesis as her frame at sc-14 (`r-049`),
   without inventing a hand-off scene. The gap between "Pip alone heard / said
   nothing" and "Lysa uses it" is genuine and left in the log rather than
   papered over.

3. **sc-18 / sc-20 — the "three ends + a fourth (parley)" framing.** Lysa
   (sc-18) names the keeping's three ends (unmake / claim / leave to him) and
   then "a fourth thing ... the shade could be reasoned with." sc-20 then has
   Orrek's *own* shade (frame `warden`) "know" the binding "had only three
   ends — its unmaking, its claiming, or an oath sworn that would loose the
   burden." The two passages count the endings differently (sc-18 treats
   parley as a fourth thing; sc-20 folds the oath into the third of three).
   Both are stated; I recorded each in its own scene/frame as written
   (`r-059`/`r-060` for Lysa, `r-065` for the warden) and did not reconcile
   the count.

## Genuinely ambiguous places (recorded conservatively)

- **Quest entities (`q-main`, `q-key`, `q-delver`, `q-reliquary`).** The vocab
  supplies these quest entities and the entity-predicates `pursues` /
  `requires` / `completed_by`. The prose states the *acts* (Calder charges
  the party; Tam asks Pip to find his master; Pip takes the key-errand; Henna
  takes the reliquary errand). I typed `pursues`/`completed_by` legs where the
  prose states a person taking on or discharging a named errand, but I could
  NOT type the `requires` leg for `q-main`/`q-delver` with the vocab as given:
  `requires` declares `object_kind=entity`, and the prose's content of those
  charges ("find the thing that wakes the dead and end it" / "find his master
  and bring word") is a scalar description, not one of the vocab entities.
  Those two charges are therefore recorded as untyped claims (`r-012`,
  `r-023`). Only `vault-door requires key` (sc-16) had a clean entity object
  and was typed (`r-053`).

- **`q-delver` completion (sc-13).** The prose states Pip found the delver's
  body and recognized him as Tam's master — "He was not lost. He was dead."
  The errand was "find him ... and bring me word." The party finds him but the
  prose never shows them bringing Tam word (and sc-31c/sc-32c state outright
  that no one ever does). I typed `q-delver completed_by delver` (the finding,
  `r-046`) because the prose states the find, but I did NOT record the errand
  as discharged-to-Tam — sc-31c/sc-32c explicitly state the boy was never
  told. Recorded that non-delivery as `r-085` / `r-087`.

- **Who "holds" the crown on the claim road (sc-23c onward).** The prose
  states Doran is the one who wades out, takes it, bears it up (sc-23c,
  sc-27c), but also says repeatedly "the crown was theirs" / "the four of
  them" held it. I typed `possession` to `fighter` (Doran) where the prose
  names him as the bearer specifically (`r-071`, `r-076`) and kept the
  "theirs / the party" sense in the untyped claim text, rather than inventing
  a party-collective entity (none in vocab).

- **sc-28c reliquary completion.** "Henna remembered the errand ... she meant
  to keep her word ... She put the Hollow Crown into Brother Vane's keeping."
  I typed `q-reliquary completed_by cleric` (`r-079`) because the prose states
  Henna fulfilling the errand by the act of handing it over. Whether handing
  the crown to its schemer "counts" as completing the errand is a question the
  prose answers in the affirmative for the *errand-as-asked* ("the reliquary
  errand fulfilled, the dark thing passed into holy hands") even as it shows
  the betrayal next scene. Recorded the stated fulfillment; the irony lives in
  the following scenes' own facts.

- **sc-30c — `reach_rule`.** The prose hedges deliberately: "Whether it had
  stayed on the party's brow or ... passed to Brother Vane's, the shape of the
  thing did not change: ... the dead answered to whoever wore the iron." On
  the claim road the manuscript has actually shown the crown passing to Vane
  (sc-29c). I typed the stated, frame-`gt` claim that the *crown* rules the
  Reach (`r-082`, `r-086`), and separately recorded Vane's own stated intent
  to rule (`r-081`, frame `shrinekeeper`). I did NOT type a `reach_rule`
  pinning the final ruler to Vane as ground truth, because sc-30c explicitly
  declines to pin it to a person and frames the rule as belonging to "whoever
  wore the iron" / the crown.

## Notes on frame choice

- `gt` used for what the narration states as simply true (the drowning, the
  Hollow Crown as cause in sc-01, the crown changing hands, the dead obeying).
- Character frames used for what a person senses/says/believes: `reeve`
  (curse), `wisewoman` (living-hand tithe, incense), `cleric` (the leash, the
  weariness, the oath she could swear), `wizard` (the journal read-out, the
  runes, the crown's nature), `rogue` (the hidden stair, the tools, the key,
  recognizing the master), `warden` (what Orrek's shade knows),
  `shrinekeeper` (Vane's stated intent at the betrayal), `lanternboy` (Tam's
  account and request), `rival` (Skell's wants).
- sc-01's "It was the Hollow Crown that did it — though no one in the town
  above could have named it so" is recorded as `gt` (`r-007`): the narration
  asserts it as true while explicitly flagging that the townsfolk do not know
  it. The clause about no-one-naming-it is left as prose colour, not a
  separate fact.
