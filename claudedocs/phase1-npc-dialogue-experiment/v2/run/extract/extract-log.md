# Extract log — npc-dialogue-experiment/v2 re-extraction

Read: extractor-brief.md (method), vocab.md (ids), and the three manuscripts
`R-world-report.md`, `R-world-shelter.md`, `R-world-confront.md`. Nothing else opened.

## Structure observed

- Shared spine = `sc-01` .. `sc-15`, **byte-identical** across all three files.
  All spine facts recorded with `branch` omitted (root `main`).
- `report` road = `sc-16`..`sc-21` (post-fork).
- `shelter` road = `sc-22`..`sc-27` (post-fork).
- `confront` road = `sc-28`..`sc-32` (post-fork).
- 32 distinct sections, one section per heading.

## Totals

- 159 facts: 36 `gt` (narration the reader is meant to take as real), 123 person-frame.
- Branch split: 66 spine, 34 report, 30 shelter, 29 confront.
- 21 typed facts (possession 9, lit 5, cause 4, whereabouts 3).

## Genuine ambiguities the prose leaves

1. **The hour is never a number.** The tally line is "Light out" + "the hour
   beside it" (sc-03); the prose says only that this hour equals the hour the
   Marran struck. No clock value is on the page. Recorded as the equality the
   prose states, not as a numeric `whereabouts`/time value.

2. **Which islander's cask is "the short cask."** The factor's count is one
   cask short (sc-11). Dunna's hidden cask is the one the prose follows, but
   Halsa's own reasoning at sc-14 says "Dunna's cask, or another's, it came to
   the same." The identity of the short cask as Dunna's is only confirmed at
   the resolutions (report sc-18 Dunna gives it up; shelter sc-25/confront sc-31
   she keeps/returns it). On the spine it is left as "one cask short" + "Dunna
   took one cask"; the prose does not on the spine assert they are the same cask.
   I recorded them as the separate statements the prose makes and did not fuse
   them into a single identity fact before the road where the prose does.

3. **What Orne carries "against his ribs" on the wreck-night (sc-01).** The
   prose says only "a thing under his coat ... that he meant no living soul
   should ever see," and that he "had carried the lantern to the Ness." Whether
   the thing against his ribs IS the lantern is never said outright on the page
   (the lantern by then has been shown and left on the Ness; he stands at the
   chapel wall after). I recorded the lantern-to-Ness action and the hidden
   "thing" as two separate stated facts and did not equate them, because the
   prose does not.

4. **Maon's "I read it in the lie of her timber" (sc-09 / confront sc-28) vs.
   ground truth.** The pilot infers the steered-to-a-light conclusion from the
   wreckage. Recorded entirely in `pilot` frame as his reading/claim, not as
   `gt`. The matching `gt` (Orne showed the light) rests on sc-01/sc-18/sc-29,
   not on the pilot's inference.

## Recorded-but-flagged: a thing said in a mouth that (from prose alone) had no
## evident way to know it

- **Ysolt knows the supercargo was held under — confront sc-30, recorded in
  `passenger` frame.** Ysolt presses the mate: "The sea takes a man, mate. It
  doesn't unknot a cord from a dead man's neck and leave you the key. ... Where
  is the lading-bill. ... A man does not get a paper off a drowning kinsman's
  coat by grieving him." She arrives at the murder by *deduction from the
  missing key/bill*, which the prose does support (she had a second key; the
  first went with her kin). So the inference is grounded. What the prose does
  NOT explain is how Ysolt knows the mate *came out of the water with* the bill
  at all — she was a passenger off the wreck, and the bill has been hidden
  against Cass's body since sc-11. On the page no scene shows her learning the
  mate held the bill before she names it on the bar. Recorded as her sc-30
  claim; not fixed. (A mirror, not an editor.)

- **The boy's frame at confront sc-29 / report sc-19** asserts the lantern was
  "the false light ... shown from the Ness." Wick only ever found a dry shuttered
  lantern on the bar (sc-04/sc-13); the Ness-connection is Halsa's and the
  pilot's reading. In Wick's own lines he claims only *finding* it; I kept his
  facts to what he says he found, and attributed the "false light from the Ness"
  naming to Halsa's frame (sc-29 f-135), not the boy's.

## Notes on typing

- `possession` used for the single-holder objects as they change hands: lantern
  (headman→hidden→headman→buried, or →officer's table), chest (passenger→officer
  in report), tally (→halsa), short cask (saltwife→officer/factor depending on
  road), lading-bill (→mate). Recorded the holder only at scenes where the prose
  states the holding/transfer.
- `lit` used for the Holm light states: out (sc-01), doused-by-a-hand (sc-12,
  Bryde's frame), burning (each road's closing scene).
- `cause` used for the two load-bearing wreck-night truths the prose states as
  gt or owns in confession: Orne = the false light / made wreck; the mate =
  holding the supercargo under. On the spine the mate's `cause` is recorded at
  sc-15 as gt (the prose states it outright there as the hidden deeper truth);
  Orne's confession `cause` is recorded again in `headman` frame at confront
  sc-29 where he owns it aloud.
- `whereabouts` used for the lantern's stated locations: hidden in the loft
  straw (sc-13 gt), buried under the salt-house floor (shelter sc-23 gt); and
  the chest opened on the bar (confront sc-30).

## Import result

- `import-sections`: 32 created, 0 no-op. Clean.
- `import-facts`: 13 frames + 3 branches + 23 entities + 4 predicates + 159
  facts created, 0 no-op, 0 reject. One atomic transaction, accepted first run.
