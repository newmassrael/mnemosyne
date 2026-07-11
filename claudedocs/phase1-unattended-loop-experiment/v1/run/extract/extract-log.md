# Re-extraction log — world-line `road-descend`

Blind extraction from `render/world-road-descend.md` only, using `extract/vocab.md`
for ids. Notes on judgement calls and ambiguities.

## Withholding respected (revealed-truth NOT recorded early)

- **Tomas dead** — the prose only STATES his death as ground-truth at
  `sc-11d-on-the-scree` ("she found Tomas Hale ... He lay at the foot of the
  scree the way a man lies who has ... not got up"). Earlier scenes give only
  absence: `sc-02` = the cab is empty and "of Tomas there was nothing at all"
  (recorded as `e-tomas absent`, f-08); `sc-05` = Wren deliberately withholds
  the word, "Missing ... Only missing" (recorded in the `relief` frame, f-22).
  So the ground-truth `life_state = dead` fact (f-46) has `canon_from = sc-11d`.
- **The fire/smoke was real** — contested through sc-04/sc-05 (Tomas swears
  smoke; dispatch reads dead ground). Ground-truth confirmation is withheld
  until `sc-10d` when Wren puts her hand to the cold ash (f-38). Dispatch's
  "no fire" is kept in the `dispatch` frame (f-21), Tomas's "smoke" in the
  `senior` frame (f-17).
- **The handline is Tomas's rope** — at `sc-09d` the prose only states a rigged
  handline exists (ground-truth f-34) and that Wren infers "someone" rigged it
  (relief f-35). It is not identified as Tomas's rope until `sc-11d` (f-44).
- **Sella Roan exists / who is down there** — before sc-10d Wren only *believes*
  a person is below (relief f-31). Sella as a real, named, living person is
  first stated at `sc-10d` (f-39), so all Sella facts have `canon_from = sc-10d`.

## Frame assignments

- `ground-truth` used only for what the narration asserts as fact.
- `relief` (Wren) used for her inferences/expectations/beliefs: f-05, f-12,
  f-18, f-22, f-25, f-26, f-30, f-31, f-35. Note f-25/f-26 are Wren's deductions
  about Tomas's actions/beliefs from the empty hooks — kept in her frame, not
  ground-truth, since the narration presents them as her reading of the room.
- `senior` (Tomas): f-17, his belief he saw smoke ("he swore he had seen").
- `dispatch` (Marla): f-21, her position that nothing is burning.
- `ranger` (Dell): f-28, his expectation the tower stays manned. The protocol
  itself (f-27) is narrated as an existing rule, so it is ground-truth.

## Ambiguities / low-confidence calls

- **f-15 / f-16 entity for "the valley / dispatch"**: vocab gives only `e-marla`
  as the dispatch character; there is no separate entity for the district office
  or the valley cameras, so those facts use `e-marla` as the dispatch actor.
- **f-17 typed `signal_kind = smoke`**: Tomas reported "smoke"; vocab's example
  values are wildfire / distress-signal. "smoke" is what he actually logged, so
  I used it as the perceived kind in his frame rather than forcing wildfire.
  The ground-truth kind (distress-signal) is on f-41.
- **f-42 "only one man had seen"**: the prose says "on the whole ridge only one
  man had seen" — read as Tomas being the sole one who saw Sella's smoke.
- **f-22 not typed**: Wren's "only missing" is an epistemic stance, not a clean
  alive/dead scalar, so it is left as a plain claim in the relief frame rather
  than typed `life_state`.
- **q-westdraw**: treated as the confirm-the-column / find-the-person quest.
  `pursues` attached to Wren at her commitment (f-18, sc-04); `completed_by`
  attached to Wren at the rescue (f-53). Tomas also went down for it and died,
  but the discharge of the quest (survivor brought out) is Wren's.
- **Fork**: `road-descend` diverges at `sc-08-west-draw` ("Then she went back
  inside for her rope"); the descend-only scenes carry the `d` suffix
  (sc-09d..sc-12d). Extraction covers sc-01 through sc-12d, this world-line only.
