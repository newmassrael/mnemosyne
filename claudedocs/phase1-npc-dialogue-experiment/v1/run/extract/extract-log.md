# Extract log — npc-dialogue-experiment/v1 re-extraction

Mirror, not editor. I read the three manuscripts (`run/render/world-report.md`,
`world-shelter.md`, `world-confront.md`) and the vocabulary, and recorded only
what the prose states on the page. This log records what the prose left
genuinely ambiguous, and the places where the prose puts a thing in a person's
mouth (or head) that — from the prose alone — they seem to have no way to know.

## Structure as extracted

- **Sections**: 32 (`sc-01`…`sc-32`). The three files share `sc-01`…`sc-15`
  verbatim (the spine); they then diverge per the vocabulary's `forks_at`:
  `sc-15`. report = `sc-16`…`sc-21`, shelter = `sc-22`…`sc-27`, confront =
  `sc-28`…`sc-32`.
- **Branch tagging**: facts from `sc-01`…`sc-15` carry no `branch` (spine /
  `main`). Facts from `sc-16`+ carry `report` / `shelter` / `confront` per the
  file they came from.
- **Counts**: 171 facts — 75 `gt`, 96 person-frame. 25 typed. Imports loaded
  clean (sections 32 created; facts 13 frames + 3 branches + 23 entities + 4
  predicates + 171 facts, 0 no-op, 0 reject).

## Things the prose states that the speaker/holder seems to have no way to know

These are recorded as the prose places them; flagged here, not fixed.

1. **`gt` omniscient interiors on the wreck-night spine (`sc-01`).** The
   narration states as ground truth that Orne "carried the lantern to the Ness
   that night, and shown what he had shown," that he "knew the Holm light was
   dark" and meant it never be known it "had been *made* dark," and Bryde's full
   interior of climbing/falling. No on-page witness reports these on the night;
   the prose presents them as omniscient narration. Recorded in `gt` because the
   prose frames them as actually-having-happened, but the *knowledge channel*
   (who could tell the reader) is the narrator alone — there is no in-world
   informant for the wreck-night interiors.

2. **`passenger` knows the supercargo is her kin and carried the deed/key
   (`sc-07`).** The prose says she "did not say" this and "kept it locked." It is
   her own knowledge, so it sits in her frame correctly — but on the SHELTER
   road this fact is never spoken aloud to anyone (the bargain keeps it sealed),
   so its only on-page channel to the reader there is, again, narration of her
   private knowledge.

3. **`gt` claim that the supercargo was murdered, on the SHELTER road
   (`sc-24`).** `f-s24-mate-walked-free` records, as ground truth, that Cass
   "had drowned a man for a paper." On this road Halsa explicitly "did not know —
   she never would, now" (also recorded). So the reader is told the murder
   happened by narration that no character on this road possesses. Recorded in
   `gt` (the prose asserts it as real) with the matching `cause` typed leg; the
   note is that on SHELTER this truth has no in-world knower at all.

4. **`mate` interior on the spine (`sc-11`).** "Cass meant to make [the claim]
   his own" — private intent, narrated. Placed in `mate` frame; no listener.

5. **`headman` interior on the spine (`sc-08`, `sc-10`).** The bidding "take
   what the sea gives" is reported by Dunna (`saltwife`, `sc-08`) as something
   the headman said, AND narrated as `gt` (`f-044`). Orne's belief that the boy
   found "only a drowned body" (`sc-10`) is narrated `gt` about Orne's mind.

## Genuinely ambiguous readings (recorded as best the prose supports)

1. **Wick's three tellings (`sc-04`).** The boy says "a man," then "a cask,"
   then "a lantern," and says he does not know which. I recorded all three as
   separate `boy`-frame claims (they are three things he said), plus Halsa's and
   Senna's reading that a real, frightening thing underlies them. The prose later
   (`sc-13`/`sc-14`) resolves the real thing to the lantern; I did NOT backfill
   sc-04 with that resolution — the earliest scene where the lantern is stated as
   the true find is sc-13 (boy frame) / sc-14 (gt, on holding it).

2. **Whose cask is short.** The factor's bill is one cask short (`sc-11`,
   factor frame). Dunna took "one cask" (`sc-08`). Halsa's spine reasoning
   (`f-067`, `sc-14`) explicitly says "Dunna's cask, or another's, it came to
   the same" — i.e. the prose does NOT, on the spine, firmly equate the short
   cask with Dunna's. On the report and confront roads Dunna's cask IS the one
   given back / set among the recovered casks and the count is "made whole," so
   there the identity is effectively stated. I left the spine fact at the
   prose's own hedge and let the branch facts assert the identity where the
   prose does.

3. **`whereabouts` of the lantern is multi-state across roads.** I recorded the
   lantern's location as a `whereabouts` typed leg at each scene the prose moves
   it: carried to the Ness (sc-01), hidden in the loft straw (sc-13), dug out /
   in Halsa's hands (sc-14, possession). On SHELTER it is given to Orne
   (possession, sc-23) and buried under the salt-house floor (whereabouts,
   sc-23). These are per-scene states in the same world-line, not contradictions.
   I did NOT record cross-road succession (each road is its own branch).

4. **`possession` of the tally.** Bryde keeps the tally-book (years of it), then
   tells Halsa to keep it (`sc-03`); I recorded Halsa taking it (possession,
   sc-03). Bryde later keeps a *second*, hidden true tally on the SHELTER road
   (`sc-27`) — recorded as a gt fact, but I did NOT add a possession typed leg
   for the hidden tally because the prose treats it as the keeper's account
   continuing, not a transfer.

5. **`lit` state vocabulary.** The vocabulary names the `lit` scalar as
   "burning, out, or shown to have been doused by a hand." I used the value
   strings `burning`, `out`, and `doused-by-a-hand` to match those three named
   states. The wreck-night light is `out` to Halsa's eye (sc-01, observed) and
   `doused-by-a-hand` as the gt/headman truth (sc-01 gt, sc-12 bryde) — recorded
   both, in their proper frames, since "out" is what is observed and
   "doused-by-a-hand" is the deeper stated cause.

6. **Hour-match inference vs statement.** The tally line "Light out" + matching
   hour (`sc-03`) is stated by the prose (Halsa reads it and "had only to set
   that hour against the hour the *Marran* had struck ... to see that they were
   the same"). I recorded the hour-match as a gt fact (`f-019`) because the
   prose states the two hours ARE the same, not merely that Halsa suspects it.

## Notes on frame assignment of dialogue

- A line of dialogue was recorded in the speaker's frame (per brief), even when
  the dialogue asserts a ground-truth event — e.g. Orne's confession on the
  confront road (`sc-29`) is `headman` frame, and ALSO has a `cause` typed leg,
  because the confession is the headman stating his own causation aloud. The
  parallel narrated gt of the same causation lives on the report road
  (`f-r18-orne-did-it-gt`, sc-18 gt) where the prose narrates it as fact behind
  his denial. The confront road has no separate narrated gt of the causation
  because there the headman states it himself; I did not duplicate it into gt.
- Where narration and a character's speech assert the same thing in the same
  scene, I recorded the gt fact AND the spoken fact separately only when both are
  actually on the page (e.g. sc-08 Dunna's spoken "gift of the sea" + the gt
  narration of the headman's bidding). I did not manufacture a gt twin for every
  line of dialogue.
