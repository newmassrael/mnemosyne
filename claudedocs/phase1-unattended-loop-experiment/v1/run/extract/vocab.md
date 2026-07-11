# Extractor vocabulary — "The Ban Tower" (road: descend)

The id vocabulary ONLY, for re-extracting facts from the rendered prose. You are
blind to the original facts, their claims, and their disclosure — map only what
the prose itself states or a character on the page plainly knows.

## Frames (whose knowledge)
- `ground-truth` — what actually happened (author omniscience; use ONLY for what the prose states as fact, not what a character believes)
- `relief` — Wren Calloway, the relief lookout (the POV)
- `senior` — Tomas Hale, the missing senior lookout
- `dispatch` — Marla Vane, valley dispatch
- `ranger` — Dell Ossory, the district ranger
- `hiker` — Sella Roan, the stranded hiker

## Entities
- characters: `e-wren`, `e-tomas`, `e-marla`, `e-dell`, `e-sella`
- locations: `e-tower`, `e-westdraw`, `e-scree`
- items: `e-log`, `e-torn-page`, `e-radio`, `e-rope`, `e-fire-ring`
- quest: `q-westdraw`

## Predicates (typed leg, optional)
- `life_state` (scalar): a character's life state (e.g. alive / dead)
- `signal_kind` (scalar): what the smoke/fire is (e.g. wildfire / distress-signal)
- `pursues` (entity): actor pursues a quest
- `completed_by` (entity): quest discharged by an actor

## World-line
- one fork: `road-descend` forks from `main` at scene `sc-08-west-draw` (the
  choice to go down). You are extracting the `road-descend` world-line.
